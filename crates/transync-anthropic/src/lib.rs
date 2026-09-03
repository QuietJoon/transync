//! `transync-anthropic` — the second in-tree `Translator`, over the
//! Anthropic **Messages API** (`POST {base_url}/v1/messages`).
//!
//! One HTTP surface, not two: there is no `Api` type, no surface
//! environment variable and no `with_api` builder here, because the
//! provider has nothing for them to select. What the OpenAI adapter spends
//! on dual dispatch, this crate spends on the two things the provider does
//! differently — `x-api-key` plus a protocol-version header, and a
//! **required** output ceiling.
//!
//! Configuration identity is fixed at construction (the `a60f07` pattern):
//! model, base URL and effort are fields, so [`TransyncAnthropic::fingerprint`]
//! and every request it issues read the same values for the life of the
//! instance. The crate's **entire** environment-read surface is
//! `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` and
//! `TRANSYNC_ANTHROPIC_BASE_URL`, all three read inside
//! [`TransyncAnthropic::from_env`] and nowhere else.
//!
//! # The `transync` binary does not reach this crate
//!
//! **Setting those three variables changes no `transync` CLI run.** The binary
//! depends on `transync` and `transync-openai` only, so reaching this adapter
//! means depending on this crate from your own program. That is DCR-0029's
//! scope, re-affirmed as a standing decision by ti `473dd1` on 2026-09-03: a
//! `--provider` axis is real design work — provider-keyed default model, base
//! URL and environment-variable set, two `ModelId` types to route, and a
//! credential-free constructor to match `--offline` (DCR-0046) — and it wants
//! its own ticket if it is ever wanted.
//!
//! This paragraph exists because this file was the last place still silent
//! about it. The fact is stated in eight others — `README.md`, `CLAUDE.md`,
//! `contracts.md` §8, the CLI reference, the Developer Guide, `mvp-scope.md`,
//! the architecture overview and the root manifest's comment — and none of
//! them is what a crates.io or docs.rs reader sees first.
//!
//! TRACE: ti 473dd1
//! TRACE: ADR-0002
//! TRACE: DCR-0029
//! TRACE: contracts.md §8

pub mod client;
pub mod error;

use std::fmt;
use std::str::FromStr;

use secrecy::SecretString;
use serde::Serialize;
use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, ProviderFingerprint, TokenizerHint, TranslationBatch,
    TranslationBatchResult, Translator, TranslatorError,
};
use url::Url;

/// Where the Messages API lives when the caller configures no base URL.
///
/// Crate-root rather than inside the endpoint builder because
/// [`TransyncAnthropic::fingerprint`] names it: the fingerprint must carry
/// the *effective* base URL, and the effective one for an unconfigured
/// instance is this constant.
pub const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// The protocol version every request declares in its `anthropic-version`
/// header. A crate-wide constant, not a per-instance axis: two instances of
/// one crate build cannot disagree on it, which is why
/// [`TransyncAnthropic::fingerprint`] deliberately leaves it out (see that
/// method's docs).
pub const ANTHROPIC_VERSION: &str = "2023-06-01";

/// The output ceiling a translation request sends when
/// `[batching].target_output_tokens` configures none.
///
/// `max_tokens` is a **required** field on this API — there is no
/// omit-it-and-take-the-provider-default path the way there is on the OpenAI
/// surfaces — so a ceiling always rides in the request and this is the value
/// that rides when nobody chose one. 16 K is the non-streaming guidance
/// ceiling: large enough that a real batch answer fits, small enough that a
/// non-streaming request stays inside the HTTP budget. This adapter does not
/// stream.
///
/// Public because the exhausted-ceiling diagnostic names it: an operator
/// reading that error is being told a number they never configured, and this
/// is where the number is written down.
pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 16_384;

/// The model [`TransyncAnthropic::from_env`] uses when
/// `TRANSYNC_ANTHROPIC_MODEL` names nothing.
const DEFAULT_MODEL: &str = "claude-opus-5";

/// Identifier of an Anthropic model.
///
/// **This is the authoritative model identity for a run** (OI-0029), on the
/// same terms §7 records for the OpenAI adapter: it is what goes on the
/// wire and what [`TransyncAnthropic::fingerprint`] covers, while
/// `transync::TranslateOptions::model_id` stays the advisory label its own
/// docs describe. Keep the two equal; a mismatch is safe but wasteful.
///
/// The crate ships **no model registry** and validates no model names
/// beyond the blank/padding rules below: which models exist is the
/// provider's authority, and a wrong name comes back as a 404 —
/// `TranslatorError::ProviderRejected { status: Some(404), .. }`.
///
/// TRACE: contracts.md §8
#[derive(Debug, Clone)]
pub struct ModelId(pub String);

impl ModelId {
    /// Wrap an identifier verbatim, unchecked — [`TransyncAnthropic::new`]'s
    /// counterpart at the model axis. [`Self::parse`] is the checked one.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The checked constructor: refuse an identifier that names no model, or
    /// that names one with whitespace around it.
    ///
    /// The rules are §7's, verbatim, and for §7's reasons. An empty — or
    /// whitespace-only — identifier is [`ConfigError::EmptyModel`]: the
    /// string is what goes on the wire, so a blank one buys a remote 400 for
    /// a fault that was visible in the configuration. An identifier with
    /// **surrounding whitespace** is [`ConfigError::PaddedModel`], refused
    /// rather than trimmed because the stored string is simultaneously the
    /// wire value and a [`TransyncAnthropic::fingerprint`] axis — trimming
    /// would send something other than what the configuration says, while
    /// `" claude-opus-5 "` and `"claude-opus-5"` would still be two cache
    /// namespaces for one model.
    ///
    /// Padding means the **edges** only: interior whitespace is part of
    /// whatever name a gateway chose.
    ///
    /// TRACE: contracts.md §8
    pub fn parse(id: impl Into<String>) -> Result<Self, ConfigError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ConfigError::EmptyModel);
        }
        if id.trim() != id {
            return Err(ConfigError::PaddedModel(id));
        }
        Ok(Self(id))
    }
}

/// Reasoning effort, sent as `output_config.effort`.
///
/// The provider's own vocabulary, which is deliberately **not** the OpenAI
/// adapter's: there is no `minimal` and no `none` here, because this API
/// does not accept them. Omitting the field entirely is how a caller says
/// "provider default"; [`TransyncAnthropic::with_effort`] is how it says
/// anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl Effort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl fmt::Display for Effort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unsupported effort {0:?}; expected low, medium, high, xhigh, or max")]
pub struct ParseEffortError(String);

impl FromStr for Effort {
    type Err = ParseEffortError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            _ => Err(ParseEffortError(value.to_owned())),
        }
    }
}

/// Configuration error raised by the checked constructors —
/// [`TransyncAnthropic::try_new`], [`TransyncAnthropic::from_env`] and
/// [`ModelId::parse`].
///
/// [`TransyncAnthropic::new`] cannot raise it: it validates nothing and
/// returns `Self`.
///
/// TRACE: contracts.md §8
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("ANTHROPIC_API_KEY environment variable not set")]
    MissingApiKey,

    /// The api key was empty *or nothing but whitespace* — the second is as
    /// unusable as the first, and would otherwise be discovered as a 401
    /// after a request had already gone out. The key itself is never named
    /// here.
    #[error("api_key was empty")]
    EmptyApiKey,

    /// The model identifier was empty or whitespace-only, so it names no
    /// model to call.
    #[error("model identifier was empty")]
    EmptyModel,

    /// The model identifier carries surrounding whitespace. It names a
    /// model, but not the one the configuration appears to say —
    /// [`ModelId::parse`] records why this is refused rather than trimmed.
    /// Debug-formatted so the padding is visible in the message.
    #[error("model identifier has surrounding whitespace: {0:?}")]
    PaddedModel(String),

    #[error("malformed base URL: {0}")]
    MalformedUrl(String),
}

/// Anthropic-backed `Translator`. Posts to `{base_url}/v1/messages`
/// with structured outputs.
///
/// Holds a single shared `reqwest::Client` so connection pooling +
/// keep-alive amortize across many concurrent batch calls — important for
/// `transync-core`'s parallel-dispatch path, which fires up to
/// `max_concurrent_batches` requests at once.
///
/// **Configuration identity is fixed at construction.** Every
/// output-affecting axis this adapter owns — model, base URL, effort — is a
/// field, so [`Self::fingerprint`] and every request it issues read the
/// *same* values for the life of the instance. There is no surface axis to
/// resolve (the provider has one endpoint), which makes the invariant
/// simpler here than on the OpenAI side rather than weaker: nothing is read
/// from the environment after a constructor returns.
///
/// TRACE: contracts.md §8
pub struct TransyncAnthropic {
    /// The credential, held from construction and read by exactly one place:
    /// the transport, which puts it in the `x-api-key` header. It is stored
    /// (rather than passed per call) for the same reason the other axes are —
    /// the configuration is decided once — and it is deliberately **not** a
    /// [`Self::fingerprint`] axis: two adapters differing only in which key
    /// pays for the tokens produce the same translations, so splitting the
    /// cache namespace on it would buy nothing and leak key identity into a
    /// string that gets logged.
    ///
    api_key: SecretString,
    model: ModelId,
    base_url: Option<Url>,
    effort: Option<Effort>,
    /// The total per-request budget this instance's `http` was built with.
    /// Kept alongside the client because a built [`reqwest::Client`] cannot
    /// be asked for it in any structured form, which is also why
    /// [`http_client_builder_with`] exists; [`Self::request_timeout`] is
    /// what lets a host report the budget it actually got.
    request_timeout: std::time::Duration,
    http: reqwest::Client,
}

impl TransyncAnthropic {
    /// Construct directly from caller-supplied credentials, unchecked.
    /// Validates **nothing**: an empty api key produces an HTTP 401 at call
    /// time, and an unusable `base_url` a transport error, each blamed on
    /// the request instead of on the configuration behind it.
    /// [`Self::try_new`] is the canonical constructor — it refuses both at
    /// construction. Reach for this one only when the credentials were
    /// validated upstream.
    ///
    /// Validating nothing is not the same as saying nothing: a `base_url`
    /// that reaches a **non-loopback** host over cleartext `http` earns a
    /// one-per-process `tracing::warn` naming what travels in the clear. It
    /// is emitted *here*, in the one constructor the other two delegate to,
    /// so every way of building an adapter is covered by one call site.
    /// Nothing is refused by it, so the contract above holds.
    ///
    /// TRACE: contracts.md §8
    pub fn new(api_key: SecretString, model: ModelId, base_url: Option<Url>) -> Self {
        if let Some(url) = &base_url {
            warn_once_on_cleartext_base_url(url);
        }
        Self {
            api_key,
            model,
            base_url,
            effort: None,
            request_timeout: REQUEST_TIMEOUT,
            http: build_http_client(),
        }
    }

    /// The canonical constructor: build the adapter only if its
    /// configuration can be used.
    ///
    /// An empty — or whitespace-only — `api_key` is
    /// [`ConfigError::EmptyApiKey`] (whitespace authenticates nothing, and
    /// would be found out as a 401 one request later); an empty or
    /// whitespace-only `model` is [`ConfigError::EmptyModel`] and a padded
    /// one is [`ConfigError::PaddedModel`] ([`ModelId::parse`] owns both
    /// rules); a `base_url` whose scheme is neither `http` nor `https`, or
    /// that carries no host, is [`ConfigError::MalformedUrl`]. All are
    /// refused here rather than deferred to the first request. Construction
    /// itself is [`Self::new`]'s, which this delegates to once the checks
    /// pass — cleartext warning included.
    ///
    /// TRACE: contracts.md §8
    pub fn try_new(
        api_key: SecretString,
        model: ModelId,
        base_url: Option<Url>,
    ) -> Result<Self, ConfigError> {
        use secrecy::ExposeSecret;
        // Whitespace is not a credential: `"   "` would pass an emptiness
        // check and fail later as a 401 — a remote answer about the request,
        // for a fault visible in the configuration. The key is never logged
        // or echoed, here or in the error.
        if api_key.expose_secret().trim().is_empty() {
            return Err(ConfigError::EmptyApiKey);
        }
        // One rule for "this identifier names a model", stated once on
        // `ModelId` and reused here rather than restated.
        let model = ModelId::parse(model.0)?;
        if let Some(url) = &base_url {
            validate_base_url(url)?;
        }
        Ok(Self::new(api_key, model, base_url))
    }

    /// Construct from environment variables per `contracts.md` §8.
    ///
    /// **This function is the crate's entire environment-read surface.** It
    /// reads `ANTHROPIC_API_KEY` (required), `TRANSYNC_ANTHROPIC_MODEL`
    /// (optional; falls back to `claude-opus-5`) and
    /// `TRANSYNC_ANTHROPIC_BASE_URL` (optional; falls back to
    /// [`DEFAULT_BASE_URL`]) — three variables, and nothing anywhere else in
    /// the crate reads the environment at all.
    ///
    /// **It *is* [`Self::try_new`] once those variables are read**, so every
    /// rule above governs the environment door too; only the fallbacks below
    /// are its own. A `TRANSYNC_ANTHROPIC_MODEL` that is empty *or
    /// whitespace-only* names no model, so it takes the default rather than
    /// riding out as one — while a variable that *does* name a model, with
    /// whitespace around it, is [`ConfigError::PaddedModel`] rather than a
    /// silently different model than the direct constructor would have used.
    ///
    /// TRACE: contracts.md §8
    pub fn from_env() -> Result<Self, ConfigError> {
        let key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;
        let model = model_from_env_value(std::env::var("TRANSYNC_ANTHROPIC_MODEL").ok())?;
        let base_url = match std::env::var("TRANSYNC_ANTHROPIC_BASE_URL").ok() {
            Some(s) if !s.is_empty() => {
                Some(Url::parse(&s).map_err(|e| ConfigError::MalformedUrl(e.to_string()))?)
            }
            _ => None,
        };
        Self::try_new(SecretString::new(key.into()), model, base_url)
    }

    /// Apply an explicit reasoning effort to subsequent requests, sent as
    /// `output_config.effort` and omitted when unconfigured.
    ///
    /// This is the sanctioned depth knob on this provider, and the reason
    /// the crate never sends a `thinking` parameter: the legacy
    /// budget-token form is rejected on current models, and the explicit
    /// `disabled` form is rejected on part of the family and effort-gated on
    /// another part, so *not sending it* is the only choice valid across the
    /// whole family. Effort is an output-affecting axis and therefore a
    /// [`Self::fingerprint`] axis.
    pub fn with_effort(mut self, effort: Effort) -> Self {
        self.effort = Some(effort);
        self
    }

    /// Set the total per-request HTTP budget for **this instance**,
    /// overriding the 120-second default.
    ///
    /// The budget spans the whole request, first byte sent to last byte
    /// read, and is spent again on every bounded provider retry. The
    /// 10-second connect budget keeps its own separate clock and is not
    /// affected — a caller that lengthens the request budget still fails
    /// fast on a connect that was never going to complete.
    ///
    /// **Not part of the fingerprint, deliberately**, and it is the one
    /// per-instance field that is not. Every other axis feeds
    /// [`Self::fingerprint`] because it changes *what* the provider
    /// returns; a timeout changes only *whether* a result arrives in time,
    /// so folding it in would split the cache namespace between hosts that
    /// merely tune their patience.
    ///
    /// TRACE: contracts.md §8
    pub fn with_timeout(mut self, timeout: std::time::Duration) -> Self {
        self.request_timeout = timeout;
        self.http = build_http_client_with(timeout);
        self
    }

    /// The total per-request budget this adapter will spend — the value
    /// passed to [`Self::with_timeout`], else the 120-second default.
    ///
    /// Exposed so a host can log or reconcile the budget it actually got
    /// instead of assuming its own configuration was honored.
    pub fn request_timeout(&self) -> std::time::Duration {
        self.request_timeout
    }

    /// Cache-namespace identity covering model + effective base URL +
    /// effort — every output-affecting configuration axis of this instance.
    /// The configured base-URL string is used verbatim (not a normalized
    /// endpoint): equivalent-but-differently-spelled URLs producing distinct
    /// fingerprints only over-invalidates, which is the safe direction.
    ///
    /// **One axis fewer than the OpenAI adapter's, and one constant left
    /// out on purpose.** There is no surface axis because the provider has
    /// one endpoint. The `anthropic-version` header
    /// ([`ANTHROPIC_VERSION`]) is excluded because it is a crate-wide
    /// constant rather than a per-instance value — two instances of one
    /// crate build cannot disagree on it, and a crate release that changes
    /// it can state the cache consequence in its changelog, exactly as
    /// prompt-text revisions (also version-carried, also not fingerprinted)
    /// already do.
    ///
    /// ADR-0020 governs the whole axis set: this identity defends against
    /// *accidental* collision, and no adversarial hardening is added or
    /// owed.
    ///
    /// TRACE: contracts.md §8
    pub fn fingerprint(&self) -> ProviderFingerprint {
        let base = self
            .base_url
            .as_ref()
            .map(url::Url::as_str)
            .unwrap_or(DEFAULT_BASE_URL);
        let effort = self.effort.map(Effort::as_str).unwrap_or("default");
        ProviderFingerprint::new("anthropic", &[&self.model.0, base, effort])
    }

    /// Declare the token encoder batch budgeting should use — flatly
    /// `cl100k_base`, for every model this adapter can be pointed at.
    ///
    /// This is the *deliberate, documented approximation* the
    /// [`TokenizerHint`] docs ask a non-OpenAI provider to make, and it is
    /// stated here rather than left to the engine: `None` would send core
    /// back to its model-name heuristic over the advisory
    /// `TranslateOptions::model_id`, which is a guess about a tokenizer core
    /// does not know, made from a string that is not even authoritative.
    /// Budgets are soft estimate caps, so an approximation is acceptable —
    /// what is not acceptable is an *undeclared* one.
    ///
    /// TRACE: OI-0029
    /// TRACE: contracts.md §8
    pub fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        Some(TokenizerHint::Cl100kBase)
    }
}

/// The `Translator` implementation — ADR-0002's seam, exercised a second
/// time.
///
/// Every method here **delegates**: the two call paths to `client`, and
/// `fingerprint` / `tokenizer_hint` to the inherent methods of the same name
/// above. The inherent pair is what `contracts.md` §8 documents, so a caller
/// holding a concrete `TransyncAnthropic` reads its cache namespace without
/// importing the trait; the trait arms exist so the pipeline reads exactly
/// the same values through `&dyn Translator`. Rust resolves an inherent
/// method ahead of a trait method of the same name, which is what makes the
/// delegation below a call and not a loop.
#[async_trait::async_trait]
impl Translator for TransyncAnthropic {
    /// Live Messages-API call with structured outputs.
    ///
    /// Cancellation (DCR-0024): the round-trip is raced against `cancel`,
    /// **biased** so the token is polled first — an already-cancelled run
    /// therefore never issues the request at all, which is the guarantee an
    /// unbiased race cannot give. Losing the race drops the `reqwest`
    /// future, which is what actually aborts the in-flight request; there is
    /// no other abort handle on a `reqwest` call. The pipeline would drop
    /// this future anyway, so what honoring the token adds is a *typed*
    /// answer ([`TranslatorError::Cancelled`]) for the caller that drives
    /// this adapter directly, and one that arrives without waiting out
    /// [`Self::request_timeout`]. The token is observed, never cancelled.
    ///
    /// TRACE: SCN-12
    /// TRACE: DCR-0024
    /// TRACE: contracts.md §8
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
            r = client::translate_on(
                &self.http,
                &self.api_key,
                self.base_url.as_ref(),
                &self.model,
                self.effort,
                &batch,
            ) => r,
        }
    }

    fn fingerprint(&self) -> ProviderFingerprint {
        TransyncAnthropic::fingerprint(self)
    }

    /// OI-0026: harvest a candidate glossary in one preflight call, with the
    /// same model and the same effort this instance uses for translation —
    /// extraction quality scales with the same knob, and `fingerprint()`
    /// already covers effort, so no fingerprint change is needed: harvested
    /// terms reach the cache key through the glossary/prompt hashes, not the
    /// provider namespace.
    ///
    /// **The `max_terms` cap is enforced here, post-parse**, and that is not
    /// belt-and-braces: this provider's structured-output dialect rejects
    /// array-count constraints, so the schema-profile pass strips the
    /// `maxItems` the shared schema stamps and the provider is never told the
    /// bound. Truncating the parsed list is what keeps the
    /// `GlossaryExtractionRequest` contract true regardless. It is a
    /// truncation rather than a rejection because the cap is a *nudge* —
    /// core's merge already treats the harvest as advisory — and discarding a
    /// whole usable harvest over its length would degrade the run for no
    /// gain.
    ///
    /// `Ok(Some(_))` even for an empty harvest: this provider *does* support
    /// extraction, which the report distinguishes from the trait default's
    /// `Ok(None)`. Errors are advisory — the pipeline records the failure on
    /// `ValidationReport.auto_glossary` and proceeds on the static glossary.
    ///
    /// Cancellation (DCR-0024): raced like [`Self::translate_batch`]. The
    /// preflight is the one provider call a run makes *before* any batch, so
    /// it is also the one whose stall a cancelled run would notice first.
    /// Nothing is re-checked afterwards; the pipeline owns the post-call
    /// re-check.
    ///
    /// TRACE: SCN-09
    /// TRACE: OI-0026
    /// TRACE: DCR-0024
    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &transync::CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let mut entries = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TranslatorError::Cancelled),
            r = client::extract_glossary_on(
                &self.http,
                &self.api_key,
                self.base_url.as_ref(),
                &self.model,
                self.effort,
                req,
            ) => r?,
        };
        entries.truncate(usize::try_from(req.max_terms).unwrap_or(usize::MAX));
        Ok(Some(entries))
    }

    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        TransyncAnthropic::tokenizer_hint(self)
    }
}

/// The model [`TransyncAnthropic::from_env`] resolves from
/// `TRANSYNC_ANTHROPIC_MODEL`, or the built-in default when the variable
/// names nothing.
///
/// The whole environment-side model rule, in one place and callable without
/// a process-global variable (which is why it is a function and not three
/// lines inside `from_env`): an unset variable, and one holding nothing but
/// whitespace, both take the default rather than riding out as the model on
/// the wire; anything else goes through [`ModelId::parse`], so the padding
/// rule that governs the direct constructor governs this entry point too
/// instead of the environment being the weaker door.
fn model_from_env_value(raw: Option<String>) -> Result<ModelId, ConfigError> {
    match raw.filter(|s| !s.trim().is_empty()) {
        Some(id) => ModelId::parse(id),
        None => Ok(ModelId::new(DEFAULT_MODEL)),
    }
}

/// Reject a custom base URL that is not an `http`/`https` URL with a host,
/// or that carries userinfo, so a misconfiguration (`file:`, `ftp:`, a
/// scheme-less string that parses as an opaque URL, an embedded credential)
/// fails fast at construction rather than as a confusing transport error at
/// request time.
///
/// **Userinfo is refused rather than ignored** (R0004-0023). This provider
/// authenticates with the `x-api-key` header and nothing else, so
/// `https://user:pass@gateway/` names no credential this crate would use —
/// but `build_endpoint` clears only the query, the fragment and the path, so
/// the userinfo survives onto the wire, where `reqwest` turns it into an
/// `Authorization: Basic` header. That leaves a second, unmanaged credential
/// riding every request and makes the endpoint's identity ambiguous — and the
/// URL is the one part of the configuration that *does* get echoed, into
/// logs and diagnostics, so an embedded password leaks where the api key
/// deliberately never does. There is no legitimate use to preserve, which is
/// what makes refusing it cheaper than sanitizing it. The message names the
/// shape and never the value.
///
/// Not an ADR-0020 matter: that record scopes *adversarial hash collision*
/// out of the threat model. This is credential hygiene in the configuration
/// the operator wrote.
fn validate_base_url(base_url: &Url) -> Result<(), ConfigError> {
    if !matches!(base_url.scheme(), "http" | "https") {
        return Err(ConfigError::MalformedUrl(format!(
            "base URL scheme must be http or https, got {:?}",
            base_url.scheme()
        )));
    }
    if base_url.host_str().is_none() {
        return Err(ConfigError::MalformedUrl(
            "base URL must include a host".to_string(),
        ));
    }
    if !base_url.username().is_empty() || base_url.password().is_some() {
        return Err(ConfigError::MalformedUrl(
            "base URL must not carry userinfo (the user[:password]@ before the host): this \
             provider authenticates with the x-api-key header, and an embedded credential \
             would ride out as a second one and surface wherever the URL is logged"
                .to_string(),
        ));
    }
    Ok(())
}

/// Whether a base URL would carry the api key in cleartext to a host that
/// is not this machine — `http` scheme, non-loopback host.
///
/// Pure and separate from the emission below so the rule itself is testable
/// without a subscriber and without the one-shot guard.
fn is_cleartext_to_remote_host(base_url: &Url) -> bool {
    if base_url.scheme() != "http" {
        return false;
    }
    match base_url.host() {
        Some(url::Host::Ipv4(ip)) => !ip.is_loopback(),
        Some(url::Host::Ipv6(ip)) => !ip.is_loopback(),
        Some(url::Host::Domain(name)) => !is_loopback_name(name),
        // A host-less base URL never reaches here — `validate_base_url`
        // refuses it — and is in no case a cleartext hop to somewhere else.
        None => false,
    }
}

/// RFC 6761 §6.3: `localhost` and every name under it are reserved for the
/// loopback interface, so a request to one never leaves the machine. The
/// optional fully-qualifying trailing dot names the same host.
fn is_loopback_name(name: &str) -> bool {
    let name = name.strip_suffix('.').unwrap_or(name);
    name.eq_ignore_ascii_case("localhost")
        || name
            .rsplit_once('.')
            .is_some_and(|(_, last)| last.eq_ignore_ascii_case("localhost"))
}

/// `http` to a **loopback** host is silent — that is how a local
/// Anthropic-compatible gateway is addressed, and nothing leaves the
/// machine. `http` to any other host is still *allowed*, because a
/// plain-http gateway on a trusted intranet is a legitimate deployment and
/// refusing it would break real ones, but it is announced once per process:
/// the API key rides on every request in the `x-api-key` header and the
/// document content rides in every request body, so both are readable — and
/// alterable — by anything on the path.
///
/// One warning per process, not one per adapter: a host that builds many
/// adapters against the same gateway has one misconfiguration, not many.
///
/// A plain flag rather than a [`std::sync::Once`] so the tests can put it
/// back and assert the emission itself; the exactly-once guarantee is the
/// same (whoever wins the `swap` emits, everyone else returns), and no
/// caller blocks behind the one that is emitting.
static CLEARTEXT_BASE_URL_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Emit [`CLEARTEXT_BASE_URL_WARNED`]'s warning if this base URL earns it.
///
/// Called from [`TransyncAnthropic::new`] and **only** from there:
/// `try_new` and `from_env` both end in `new`, so one call site covers every
/// way of building an adapter and none of them can drift from the others.
/// Announcing is not validating — nothing here refuses anything — so `new`'s
/// recorded "validates nothing" contract is intact.
fn warn_once_on_cleartext_base_url(base_url: &Url) {
    use std::sync::atomic::Ordering;
    if !is_cleartext_to_remote_host(base_url) {
        return;
    }
    if CLEARTEXT_BASE_URL_WARNED.swap(true, Ordering::Relaxed) {
        return;
    }
    tracing::warn!(
        target: "transync::anthropic",
        "base URL {:?} uses cleartext http to a non-loopback host: the API key travels in the x-api-key header and the document content as the request body, both readable and alterable by anything on the network path; use https unless the whole path is trusted",
        base_url.host_str().unwrap_or_default()
    );
}

/// DNS + TCP + TLS, on its own budget. Far above any reachable handshake
/// and far below [`REQUEST_TIMEOUT`], so it can only bite on a connect that
/// was never going to complete.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The whole request, from the first byte sent to the last byte read.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The configuration [`build_http_client`] builds from. Split out from it
/// because a built [`reqwest::Client`] does not report its connect timeout
/// in any form — its `Debug` prints the total timeout and not that one —
/// while the builder does, which is what lets the two budgets be pinned by a
/// test instead of taken on faith.
///
/// Two budgets, not one: without [`CONNECT_TIMEOUT`], a black-holed connect
/// — a firewall that drops SYNs, a stale DNS record — spends the entire
/// [`REQUEST_TIMEOUT`] before a single byte is sent, and spends it again on
/// every bounded provider retry. The connect budget is deliberately NOT
/// parameterized: it guards a connect that was never going to complete,
/// which is orthogonal to how long a host is willing to wait for a
/// response.
///
/// **Redirects are not followed, and on this provider that is a credential
/// rule rather than a preference** (R0004-0001). The api key rides in the
/// custom **`x-api-key`** header, and `reqwest` strips only its own fixed
/// sensitive set — `Authorization`, `Cookie`, `Proxy-Authorization`,
/// `Www-Authenticate` — when a redirect crosses to another origin. A custom
/// header is not in that set, so under the default follow-up-to-ten policy a
/// compromised or misconfigured endpoint could answer `3xx` with a
/// `Location` on another origin and receive the key together with the
/// document body. `transync-openai` authenticates with `Authorization:
/// Bearer`, which *is* stripped, so only this crate leaked — and the two
/// crates now refuse redirects **identically** rather than one of them being
/// safe by accident of which header its provider chose.
///
/// Nothing legitimate is lost: this adapter posts to one endpoint it builds
/// itself, and a provider API answering a `3xx` to a translation POST is a
/// misconfiguration either way — `reqwest` would rewrite a followed 301/302/
/// 303 into a bodyless `GET` and the request would fail one hop later with a
/// worse diagnostic. The unfollowed `3xx` reaches
/// `client::classify::provider_error_for_status`, which names it terminal
/// (R0004-0021).
fn http_client_builder_with(request_timeout: std::time::Duration) -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(request_timeout)
        .redirect(reqwest::redirect::Policy::none())
}

/// The client every adapter instance holds, on the default budget.
fn build_http_client() -> reqwest::Client {
    build_http_client_with(REQUEST_TIMEOUT)
}

fn build_http_client_with(request_timeout: std::time::Duration) -> reqwest::Client {
    http_client_builder_with(request_timeout)
        .build()
        .expect("reqwest with rustls-tls should always build with these parameters")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> SecretString {
        SecretString::new("sk-ant-test".to_string().into())
    }

    /// `CLEARTEXT_BASE_URL_WARNED` is one flag for the whole test binary, so
    /// every test that builds an adapter against a cleartext remote URL
    /// consumes it. They take turns here instead of racing, which is what
    /// makes the emission assertions below deterministic under
    /// `--test-threads` greater than one.
    static CLEARTEXT_TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn cleartext_turn() -> std::sync::MutexGuard<'static, ()> {
        CLEARTEXT_TURN
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Put the one-per-process gate back so the next construction can be
    /// observed announcing. Test-only: no shipped path rearms it.
    fn rearm_cleartext_warning() {
        CLEARTEXT_BASE_URL_WARNED.store(false, std::sync::atomic::Ordering::Relaxed);
    }

    /// A `tracing` subscriber that keeps what was emitted on this thread,
    /// hand-rolled on `tracing` alone — `tracing-subscriber` belongs to the
    /// CLI by decision (R0001-0032) and no library member takes it, test
    /// targets included.
    #[derive(Clone, Default)]
    struct CapturedEvents(std::sync::Arc<std::sync::Mutex<Vec<(tracing::Level, String, String)>>>);

    impl CapturedEvents {
        fn taken(&self) -> Vec<(tracing::Level, String, String)> {
            std::mem::take(&mut *self.0.lock().unwrap_or_else(|p| p.into_inner()))
        }
    }

    impl tracing::Subscriber for CapturedEvents {
        fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
            true
        }

        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }

        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

        fn event(&self, event: &tracing::Event<'_>) {
            struct Message(String);
            impl tracing::field::Visit for Message {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "message" {
                        self.0 = format!("{value:?}");
                    }
                }
            }
            let mut message = Message(String::new());
            event.record(&mut message);
            self.0.lock().unwrap_or_else(|p| p.into_inner()).push((
                *event.metadata().level(),
                event.metadata().target().to_string(),
                message.0,
            ));
        }

        fn enter(&self, _: &tracing::span::Id) {}

        fn exit(&self, _: &tracing::span::Id) {}
    }

    /// The effort vocabulary is the provider's, not the OpenAI adapter's:
    /// five levels, and neither `minimal` nor `none` is one of them.
    #[test]
    fn effort_parses_the_providers_vocabulary_and_nothing_else() {
        for value in ["low", "medium", "high", "xhigh", "max"] {
            let effort: Effort = value.parse().expect("supported effort");
            assert_eq!(effort.as_str(), value);
            assert_eq!(
                serde_json::to_value(effort).expect("serializes"),
                serde_json::json!(value),
                "the wire form must be the provider's own token"
            );
        }
        for foreign in ["minimal", "none", "turbo", ""] {
            assert!(
                foreign.parse::<Effort>().is_err(),
                "{foreign:?} is not this provider's vocabulary"
            );
        }
    }

    #[test]
    fn try_new_rejects_non_http_base_url() {
        let url = Url::parse("file:///etc/passwd").unwrap();
        let result = TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(url));
        assert!(
            matches!(result, Err(ConfigError::MalformedUrl(_))),
            "non-http base URL must be rejected"
        );
    }

    #[test]
    fn try_new_accepts_https_base_url() {
        let url = Url::parse("https://proxy.example/v1").unwrap();
        assert!(TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(url)).is_ok());
    }

    /// R0004-0023: a base URL carrying userinfo is a misconfiguration with no
    /// legitimate use on this provider — the credential is `x-api-key` and
    /// nothing else — and `build_endpoint` preserves userinfo onto the wire,
    /// where `reqwest` turns it into a second, unmanaged `Authorization:
    /// Basic` header. Both halves are refused: `user@`, `:password@`, and the
    /// full pair. The diagnostic names the shape and never the value.
    #[test]
    fn try_new_rejects_a_base_url_carrying_userinfo() {
        for embedded in [
            "https://user@gateway.example/v1",
            "https://:hunter2@gateway.example/v1",
            "https://user:hunter2@gateway.example/v1",
            "http://user:hunter2@127.0.0.1:8080/v1",
        ] {
            let url = Url::parse(embedded).expect("test URL parses");
            let refused = TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(url))
                .err()
                .unwrap_or_else(|| panic!("{embedded} must be refused at construction"));
            let ConfigError::MalformedUrl(message) = refused else {
                panic!("{embedded} must be refused as a malformed URL, got {refused:?}");
            };
            assert!(
                message.contains("userinfo"),
                "{embedded}: the diagnostic must name the shape, got {message:?}"
            );
            assert!(
                !message.contains("hunter2"),
                "{embedded}: an embedded password must never be echoed, got {message:?}"
            );
        }

        // The same host without userinfo is still perfectly fine — the rule
        // refuses a credential, not a gateway.
        let clean = Url::parse("https://gateway.example/v1").expect("test URL parses");
        assert!(
            TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(clean)).is_ok(),
            "the rule must refuse userinfo and nothing else"
        );
    }

    /// An identifier that names no model is a *configuration* error raised
    /// here, not a remote 400 after a request went out — and whitespace is
    /// no more of a model name than emptiness is.
    #[test]
    fn try_new_rejects_a_model_that_names_nothing() {
        for blank in ["", " ", "\t\n"] {
            let result = TransyncAnthropic::try_new(key(), ModelId::new(blank), None);
            assert!(
                matches!(result, Err(ConfigError::EmptyModel)),
                "{blank:?} must be refused at construction"
            );
            assert!(
                matches!(ModelId::parse(blank), Err(ConfigError::EmptyModel)),
                "{blank:?} must be refused by the checked constructor too"
            );
        }
        // A real identifier survives verbatim — the stored string is both
        // the wire value and a fingerprint axis, so it is not rewritten.
        let model = ModelId::parse(DEFAULT_MODEL).expect("a real model id is accepted");
        assert_eq!(model.0, DEFAULT_MODEL);
        assert!(TransyncAnthropic::try_new(key(), model, None).is_ok());
    }

    /// A padded identifier is refused rather than trimmed — because the
    /// stored string is at once the wire value and a fingerprint axis, so
    /// trimming would send one thing while the configuration says another,
    /// and storing it would open a cache namespace of its own.
    #[test]
    fn model_id_parse_rejects_surrounding_whitespace() {
        for padded in [
            " claude-opus-5",
            "claude-opus-5 ",
            " claude-opus-5 ",
            "\tclaude-opus-5\n",
        ] {
            match ModelId::parse(padded) {
                Err(ConfigError::PaddedModel(reported)) => assert_eq!(
                    reported, padded,
                    "the message must show what was configured, unrewritten"
                ),
                other => panic!("{padded:?} must be refused, got {other:?}"),
            }
            assert!(
                matches!(
                    TransyncAnthropic::try_new(key(), ModelId::new(padded), None),
                    Err(ConfigError::PaddedModel(_))
                ),
                "{padded:?} must be refused at construction too"
            );
        }
        // Interior whitespace is not padding: it is part of whatever name a
        // gateway chose, and this rule is about the edges only.
        let inner = ModelId::parse("my gateway/model-1").expect("interior space is not padding");
        assert_eq!(inner.0, "my gateway/model-1");
    }

    /// Whitespace authenticates nothing: `"   "` would pass an emptiness
    /// check and be found out as a 401 one request later — a remote answer
    /// about the request, for a fault visible in the configuration.
    #[test]
    fn try_new_rejects_a_whitespace_only_api_key() {
        for blank in ["", " ", "\t\n  "] {
            let result = TransyncAnthropic::try_new(
                SecretString::new(blank.to_string().into()),
                ModelId::new(DEFAULT_MODEL),
                None,
            );
            assert!(
                matches!(result, Err(ConfigError::EmptyApiKey)),
                "an api key of {blank:?} must be refused at construction"
            );
        }
    }

    /// The environment door applies the same model rule as the direct one. A
    /// variable that names nothing falls back to the default; a variable
    /// that names a model with whitespace around it is refused instead of
    /// quietly becoming a different model — and a different cache namespace
    /// — than `try_new` would have accepted.
    ///
    /// Driven through the resolver rather than through `from_env`, which
    /// reads process-global variables that no test can set without racing
    /// every other thread.
    #[test]
    fn env_model_takes_the_default_or_the_one_padding_rule() {
        for names_nothing in [None, Some(""), Some("   "), Some("\t\n")] {
            let model = model_from_env_value(names_nothing.map(str::to_string))
                .expect("a variable that names nothing falls back");
            assert_eq!(
                model.0, DEFAULT_MODEL,
                "{names_nothing:?} must take the default"
            );
        }
        let model =
            model_from_env_value(Some("claude-haiku-4-5".to_string())).expect("a real id is used");
        assert_eq!(model.0, "claude-haiku-4-5");
        assert!(
            matches!(
                model_from_env_value(Some(" claude-haiku-4-5 ".to_string())),
                Err(ConfigError::PaddedModel(_))
            ),
            "the environment must not be the weaker door"
        );
    }

    /// Cleartext `http` is refused for nobody — an intranet gateway is a
    /// real deployment — but the adapter must be able to tell "never leaves
    /// this machine" from "crosses a network in the clear", because only the
    /// second one puts the api key on the wire unencrypted. `https` is never
    /// the risk, whatever the host.
    #[test]
    fn cleartext_risk_is_recognized_only_off_the_loopback() {
        let _turn = cleartext_turn();
        for silent in [
            "http://localhost:8080",
            "http://LOCALHOST/v1",
            "http://localhost./v1",
            "http://gateway.localhost/v1",
            "http://127.0.0.1:1234/v1",
            "http://127.9.9.9/v1",
            "http://[::1]:8080/v1",
            "https://proxy.example/v1",
            "https://10.0.0.5/v1",
        ] {
            let url = Url::parse(silent).expect("test URL parses");
            assert!(
                !is_cleartext_to_remote_host(&url),
                "{silent} must not be reported as a cleartext hop"
            );
        }
        for warned in [
            "http://proxy.example/v1",
            "http://10.0.0.5:8000/v1",
            "http://[2001:db8::1]/v1",
            "http://localhost.example.com/v1",
        ] {
            let url = Url::parse(warned).expect("test URL parses");
            assert!(
                is_cleartext_to_remote_host(&url),
                "{warned} must be reported as a cleartext hop"
            );
            // …and it is still accepted: nothing here rejects.
            assert!(
                TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(url)).is_ok(),
                "{warned} must still construct — the warning is not a refusal"
            );
        }
    }

    /// The other half: the predicate above decides *what* earns a warning,
    /// this pins that the warning is actually emitted and that the
    /// constructors are wired to the thing that emits it. Deleting the
    /// single call site — or moving it somewhere a constructor can miss —
    /// turns this red, which the pure predicate's test cannot do.
    ///
    /// `from_env` is not driven here because it reads process-global
    /// environment variables a parallel test binary cannot own; it needs no
    /// separate pin, because it reaches the warning the only way anything
    /// does, by ending in `new`.
    #[test]
    fn every_constructor_announces_a_cleartext_base_url_once() {
        let _turn = cleartext_turn();
        /// One way of building an adapter against a given base URL, so the
        /// assertions below can run over all of them rather than once each.
        type BuildAgainst = fn(Url);

        let remote = || Url::parse("http://proxy.example/v1").expect("test URL parses");
        let constructors: [(&str, BuildAgainst); 2] = [
            ("new", |url| {
                TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), Some(url));
            }),
            ("try_new", |url| {
                TransyncAnthropic::try_new(key(), ModelId::new(DEFAULT_MODEL), Some(url))
                    .expect("cleartext is announced, not refused");
            }),
        ];

        for (name, construct) in constructors {
            rearm_cleartext_warning();
            let captured = CapturedEvents::default();
            tracing::subscriber::with_default(captured.clone(), || construct(remote()));

            let events = captured.taken();
            assert_eq!(
                events.len(),
                1,
                "{name} must announce the cleartext hop exactly once, emitted {events:?}"
            );
            let (level, target, message) = &events[0];
            assert_eq!(*level, tracing::Level::WARN, "{name}: {message}");
            assert_eq!(target, "transync::anthropic", "{name}: {message}");
            for named in ["proxy.example", "x-api-key", "https"] {
                assert!(
                    message.contains(named),
                    "{name}: the warning must name {named:?}, said {message}"
                );
            }
        }

        // Once per process, not once per adapter — and never for a base URL
        // that carries nothing off the machine, however often it is built.
        rearm_cleartext_warning();
        let captured = CapturedEvents::default();
        tracing::subscriber::with_default(captured.clone(), || {
            for _ in 0..3 {
                TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), Some(remote()));
            }
            for silent in ["https://proxy.example/v1", "http://localhost:8080/v1"] {
                let url = Url::parse(silent).expect("test URL parses");
                TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), Some(url));
            }
        });
        assert_eq!(
            captured.taken().len(),
            1,
            "five adapters against one gateway are one misconfiguration, not five"
        );
    }

    /// The two budgets exist and are distinct. The *behavior* is only
    /// observable against a host that black-holes SYNs, which no portable
    /// test can conjure, so this pins the configuration where reqwest
    /// reports it: the builder's own `Debug`, which names both timeouts
    /// where a built `Client`'s names only the total one.
    #[test]
    fn the_http_client_bounds_the_connect_inside_the_request() {
        let configured = format!("{:?}", http_client_builder_with(REQUEST_TIMEOUT));
        assert!(
            configured.contains("connect_timeout: 10s"),
            "DNS + TCP + TLS must carry a budget of their own: {configured}"
        );
        assert!(
            configured.contains("timeout: 120s"),
            "the whole request must still be bounded: {configured}"
        );
        assert!(
            CONNECT_TIMEOUT < REQUEST_TIMEOUT,
            "the connect budget only helps while it lives inside the request budget"
        );
    }

    /// R0004-0001: the client follows no redirects, pinned where `reqwest`
    /// reports it — a builder prints `redirect_policy` only when it is *not*
    /// the default follow-up-to-ten, so the field's presence is itself the
    /// assertion that the default was replaced. The behavioral half (the key
    /// never reaching a redirect target) lives in `client::transport`'s
    /// two-socket test; this one guards the configuration every constructor
    /// goes through, including [`TransyncAnthropic::with_timeout`]'s rebuild.
    #[test]
    fn the_http_client_follows_no_redirects() {
        for budget in [REQUEST_TIMEOUT, std::time::Duration::from_secs(600)] {
            let configured = format!("{:?}", http_client_builder_with(budget));
            assert!(
                configured.contains("redirect_policy: Policy(None)"),
                "the api key rides in a custom header reqwest would not strip on a \
                 cross-origin hop, so no redirect may be followed: {configured}"
            );
        }
    }

    /// `with_timeout` must reach the client it claims to configure — the
    /// whole point of the builder is that a host's configured budget stops
    /// being advisory.
    #[test]
    fn with_timeout_replaces_the_request_budget_and_leaves_connect_alone() {
        let configured = format!(
            "{:?}",
            http_client_builder_with(std::time::Duration::from_secs(600))
        );
        assert!(
            configured.contains("timeout: 600s"),
            "the configured budget must reach the client: {configured}"
        );
        assert!(
            configured.contains("connect_timeout: 10s"),
            "the connect budget is a separate axis and must survive: {configured}"
        );

        let adapter = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_timeout(std::time::Duration::from_secs(600));
        assert_eq!(
            adapter.request_timeout(),
            std::time::Duration::from_secs(600),
            "the adapter must report the budget it was given, not the default",
        );
    }

    /// The default is unchanged for every caller that does not ask.
    #[test]
    fn an_adapter_built_without_the_builder_keeps_the_default_budget() {
        let adapter = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None);
        assert_eq!(adapter.request_timeout(), REQUEST_TIMEOUT);
    }

    /// The timeout is deliberately NOT a fingerprint axis: it changes
    /// whether a result arrives in time, never its content. Two adapters
    /// differing only in budget must keep sharing entries.
    #[test]
    fn the_timeout_is_not_part_of_the_cache_namespace() {
        let patient = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_timeout(std::time::Duration::from_secs(600));
        let hasty = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_timeout(std::time::Duration::from_secs(5));
        assert_eq!(
            patient.fingerprint(),
            hasty.fingerprint(),
            "a timeout must not fragment the cache namespace",
        );
    }

    /// The fingerprint distinguishes model, effective base URL and effort,
    /// is stable across calls, and namespaces apart from the OpenAI
    /// adapter's family label so a shared cache cannot replay one
    /// provider's output for the other's request.
    #[test]
    fn fingerprint_covers_model_base_url_and_effort() {
        let plain = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None);
        let other_model = TransyncAnthropic::new(key(), ModelId::new("claude-haiku-4-5"), None);
        assert_ne!(plain.fingerprint(), other_model.fingerprint());

        let custom = TransyncAnthropic::new(
            key(),
            ModelId::new(DEFAULT_MODEL),
            Some(Url::parse("https://proxy.example/v1").unwrap()),
        );
        assert_ne!(plain.fingerprint(), custom.fingerprint());

        // Effort affects outputs and therefore the cache namespace.
        let deliberate = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_effort(Effort::Xhigh);
        assert_ne!(plain.fingerprint(), deliberate.fingerprint());
        // …and two different efforts are two namespaces, not one.
        let cheaper = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_effort(Effort::Low);
        assert_ne!(deliberate.fingerprint(), cheaper.fingerprint());

        // Stable across two calls on the same instance.
        assert_eq!(plain.fingerprint(), plain.fingerprint());

        // The default-base instance carries the canonical base URL, and the
        // family label is this provider's.
        assert!(plain.fingerprint().as_str().contains(DEFAULT_BASE_URL));
        assert!(
            plain
                .fingerprint()
                .as_str()
                .starts_with(&format!("{}\u{1F}anthropic", "anthropic".len())),
            "the family label must namespace this provider apart: {:?}",
            plain.fingerprint().as_str()
        );
    }

    /// The `anthropic-version` header is a crate-wide constant, not a
    /// per-instance axis, so it must not appear in the namespace — two
    /// instances of one crate build cannot disagree on it, and a release
    /// that changes it states the consequence in its changelog.
    #[test]
    fn the_protocol_version_is_not_a_fingerprint_axis() {
        let adapter = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None);
        assert!(
            !adapter.fingerprint().as_str().contains(ANTHROPIC_VERSION),
            "a crate-wide constant must not fragment the cache namespace: {:?}",
            adapter.fingerprint().as_str()
        );
    }

    /// The encoder hint is this crate's *stated* approximation, flat across
    /// every model it can be pointed at — not `None`, which would send core
    /// back to guessing from the advisory `model_id` string.
    #[test]
    fn tokenizer_hint_is_the_documented_flat_approximation() {
        for model in [DEFAULT_MODEL, "claude-haiku-4-5", "some-gateway/model-x"] {
            let adapter = TransyncAnthropic::new(key(), ModelId::new(model), None);
            assert_eq!(
                adapter.tokenizer_hint(),
                Some(TokenizerHint::Cl100kBase),
                "{model} must declare the documented approximation, not defer to a guess"
            );
        }
    }

    /// The builders set independent fields, so applying them in either order
    /// lands on one configuration.
    #[test]
    fn the_builders_are_order_insensitive() {
        let effort_then_timeout = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_effort(Effort::Medium)
            .with_timeout(std::time::Duration::from_secs(42));
        let timeout_then_effort = TransyncAnthropic::new(key(), ModelId::new(DEFAULT_MODEL), None)
            .with_timeout(std::time::Duration::from_secs(42))
            .with_effort(Effort::Medium);
        assert_eq!(
            effort_then_timeout.fingerprint(),
            timeout_then_effort.fingerprint()
        );
        assert_eq!(
            effort_then_timeout.request_timeout(),
            timeout_then_effort.request_timeout()
        );
    }
}
