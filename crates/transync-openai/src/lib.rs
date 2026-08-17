//! `transync-openai` — default `Translator` implementation with
//! model-driven dual dispatch: Chat Completions for `*chat*` models
//! (including the default `gpt-5-chat-latest`), the Responses API for
//! o-series and non-chat GPT-5 snapshots. Set
//! `TRANSYNC_OPENAI_API=chat|responses` to override the heuristic —
//! it is read **once, when the adapter is constructed** (R0001-0031) — or
//! pin one instance with [`TransyncOpenAI::with_api`], which needs no
//! process-global state at all. Both surfaces send the same strict
//! Structured Outputs schema.
//!
//! TRACE: ADR-0002
//! TRACE: contracts.md §7

pub mod client;
pub mod error;
mod tokenizer;

use std::fmt;
use std::str::FromStr;

use secrecy::SecretString;
use serde::Serialize;
use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, ProviderFingerprint, TokenizerHint, TranslationBatch,
    TranslationBatchResult, Translator, TranslatorError,
};
use url::Url;

/// The HTTP surface an adapter talks to, and the error its [`FromStr`]
/// raises — at the crate root because [`TransyncOpenAI::with_api`] takes an
/// [`Api`]. Same types as [`client::Api`] / [`client::ParseApiError`]: that
/// module still owns them, together with the [`client::api_for_model`]
/// heuristic that picks a surface from a model name.
pub use client::{Api, ParseApiError};

/// Identifier of an OpenAI-compatible model.
///
/// **This is the authoritative model identity for a run** (OI-0029): it is
/// what goes on the wire, what `fingerprint()` covers, and what
/// `tokenizer_hint()` is derived from. `transync::TranslateOptions::model_id`
/// is only an advisory label for tokenizer fallback and one cache-key axis —
/// see its docs. Keep the two equal (the CLI resolves the model once and
/// passes the same string to both); a mismatch is safe but wasteful.
///
/// TRACE: contracts.md §7
#[derive(Debug, Clone)]
pub struct ModelId(pub String);

impl ModelId {
    /// Wrap an identifier verbatim, unchecked — [`TransyncOpenAI::new`]'s
    /// counterpart at the model axis. [`Self::parse`] is the checked one.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// The checked constructor: refuse an identifier that names no model, or
    /// that names one with whitespace around it.
    ///
    /// An empty — or whitespace-only — identifier is
    /// [`ConfigError::EmptyModel`]. Nothing downstream can recover from one:
    /// the string is what goes on the wire, so the provider answers a
    /// request that names no model with a remote 400, and the
    /// [`client::api_for_model`] heuristic meanwhile picks a surface out of
    /// a blank name. Both are the configuration's fault and both are cheaper
    /// to learn here (R0002-0037).
    ///
    /// An identifier with **surrounding whitespace** is
    /// [`ConfigError::PaddedModel`] (R0003-0009). The identifier that passes
    /// is stored **verbatim**, which is what forces the choice: the stored
    /// string is simultaneously the wire value and a
    /// [`TransyncOpenAI::fingerprint`] axis, so trimming would silently send
    /// something other than what the configuration says while `" gpt-5 "`
    /// and `"gpt-5"` would still be two cache namespaces for one model.
    /// Rejecting says so at construction instead, where the visible fix is
    /// to the configuration.
    ///
    /// [`TransyncOpenAI::try_new`] applies these same rules to the model it
    /// is handed, so a checked adapter cannot hold an unusable one, and
    /// [`TransyncOpenAI::from_env`] applies them to `TRANSYNC_OPENAI_MODEL`.
    ///
    /// TRACE: contracts.md §7
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

/// Reasoning effort accepted by OpenAI reasoning-capable models.
///
/// The adapter deliberately supports the union used across GPT-5 generations:
/// older models can use `minimal` or `none` depending on their snapshot, while
/// GPT-5.6+ additionally accepts `max`. OpenAI remains the authority on which
/// effort values a particular model supports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Minimal,
    None,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

impl ReasoningEffort {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::None => "none",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
            Self::Max => "max",
        }
    }
}

impl fmt::Display for ReasoningEffort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "unsupported reasoning effort {0:?}; expected minimal, none, low, medium, high, xhigh, or max"
)]
pub struct ParseReasoningEffortError(String);

impl FromStr for ReasoningEffort {
    type Err = ParseReasoningEffortError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "minimal" => Ok(Self::Minimal),
            "none" => Ok(Self::None),
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "xhigh" => Ok(Self::Xhigh),
            "max" => Ok(Self::Max),
            _ => Err(ParseReasoningEffortError(value.to_owned())),
        }
    }
}

/// Configuration error raised by the checked constructors —
/// [`TransyncOpenAI::try_new`], [`TransyncOpenAI::from_env`] and
/// [`ModelId::parse`].
///
/// [`TransyncOpenAI::new`] cannot raise it: it validates nothing and
/// returns `Self` (R0001-0033).
///
/// TRACE: contracts.md §7
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("OPENAI_API_KEY environment variable not set")]
    MissingApiKey,

    /// R0003-0010: the api key was empty *or nothing but whitespace* — the
    /// second is as unusable as the first and used to pass, to be discovered
    /// as a 401 after a request had already gone out. The key itself is
    /// never named here.
    #[error("api_key was empty")]
    EmptyApiKey,

    /// R0002-0037: the model identifier was empty or whitespace-only, so it
    /// names no model to call.
    #[error("model identifier was empty")]
    EmptyModel,

    /// R0003-0009: the model identifier carries surrounding whitespace. It
    /// names a model, but not the one the configuration appears to say —
    /// [`ModelId::parse`] records why this is refused rather than trimmed.
    /// Debug-formatted so the padding is visible in the message.
    #[error("model identifier has surrounding whitespace: {0:?}")]
    PaddedModel(String),

    #[error("malformed base URL: {0}")]
    MalformedUrl(String),
}

/// Default OpenAI-backed `Translator`. Posts to `{base_url}/v1/responses`
/// with Structured Outputs.
///
/// Holds a single shared `reqwest::Client` so connection pooling +
/// keep-alive amortize across many concurrent batch calls — important
/// for `transync-core`'s parallel-dispatch path, which fires up to
/// `max_concurrent_batches` requests at once.
///
/// **Configuration identity is fixed at construction.** Every
/// output-affecting axis this adapter owns — model, base URL, reasoning
/// effort, and the HTTP surface — is a field, so [`Self::fingerprint`]
/// and every request it issues read the *same* values for the life of the
/// instance. The surface in particular is resolved once, from
/// `TRANSYNC_OPENAI_API` if set and the model-name heuristic otherwise
/// (R0001-0031); mutating that variable afterwards cannot make the
/// fingerprint name one surface while the requests go to the other.
/// [`Self::with_api`] is how a caller *chooses* that axis without the
/// process-global variable — it consumes and returns the adapter, so it
/// too lands before anything can observe the instance.
///
/// TRACE: contracts.md §7
pub struct TransyncOpenAI {
    api_key: SecretString,
    model: ModelId,
    base_url: Option<Url>,
    reasoning_effort: Option<ReasoningEffort>,
    /// R0001-0031: the HTTP surface, resolved at construction and never
    /// re-read. The single source for both `fingerprint()` and dispatch.
    api: client::Api,
    /// The total per-request budget this instance's `http` was built with.
    /// Kept alongside the client because a built [`reqwest::Client`] cannot
    /// be asked for it in any structured form — the same reason
    /// [`http_client_builder_with`] exists — and [`Self::request_timeout`] is
    /// what lets a host report the budget it actually got.
    request_timeout: std::time::Duration,
    http: reqwest::Client,
}

impl TransyncOpenAI {
    /// Construct directly from caller-supplied credentials, unchecked.
    /// Validates **nothing**: an empty api key produces an HTTP 401 at
    /// call time, and an unusable `base_url` a transport error, each
    /// blamed on the request instead of on the configuration behind it.
    /// [`Self::try_new`] is the canonical constructor — it refuses both
    /// at construction. Reach for this one only when the credentials
    /// were validated upstream.
    ///
    /// Resolves the HTTP surface here (see [`Self::api`]), so
    /// `TRANSYNC_OPENAI_API` must be set *before* this call to take
    /// effect on this instance.
    ///
    /// Validating nothing is not the same as saying nothing: a `base_url`
    /// that reaches a **non-loopback** host over cleartext `http` earns a
    /// one-per-process `tracing::warn` naming what travels in the clear
    /// (R0002-0005). It is emitted *here*, in the one constructor the other
    /// two delegate to, so every way of building an adapter is covered by one
    /// call site. Nothing is refused by it, so the contract above holds.
    ///
    /// TRACE: contracts.md §7
    pub fn new(api_key: SecretString, model: ModelId, base_url: Option<Url>) -> Self {
        if let Some(url) = &base_url {
            warn_once_on_cleartext_base_url(url);
        }
        Self {
            api: client::api_from_env_or_model(&model.0),
            api_key,
            model,
            base_url,
            reasoning_effort: None,
            request_timeout: REQUEST_TIMEOUT,
            http: build_http_client(),
        }
    }

    /// The canonical constructor: build the adapter only if its
    /// configuration can be used.
    ///
    /// An empty — or whitespace-only — `api_key` is
    /// [`ConfigError::EmptyApiKey`] (R0003-0010: whitespace authenticates
    /// nothing, and used to be found out as a 401 one request later); an
    /// empty or whitespace-only `model` is [`ConfigError::EmptyModel`] and a
    /// padded one is [`ConfigError::PaddedModel`] ([`ModelId::parse`] owns
    /// both rules); a `base_url` whose scheme is
    /// neither `http` nor `https`, or that carries no host, is
    /// [`ConfigError::MalformedUrl`]. All are refused here rather than
    /// deferred to the first request. Construction itself is [`Self::new`]'s,
    /// which this delegates to once the checks pass.
    ///
    /// A `base_url` that reaches a **non-loopback** host over cleartext
    /// `http` is accepted — an intranet OpenAI-compatible gateway is a real
    /// deployment — but earns a one-per-process `tracing::warn` naming what
    /// travels in the clear (R0002-0005). Loopback hosts are silent. That
    /// warning is [`Self::new`]'s, inherited by delegating to it.
    ///
    /// TRACE: contracts.md §7
    pub fn try_new(
        api_key: SecretString,
        model: ModelId,
        base_url: Option<Url>,
    ) -> Result<Self, ConfigError> {
        use secrecy::ExposeSecret;
        // R0003-0010: whitespace is not a credential. `"   "` passed the
        // emptiness check and failed later as a 401 — a remote answer about
        // the request, for a fault that was visible in the configuration.
        // The key is never logged or echoed, here or in the error.
        if api_key.expose_secret().trim().is_empty() {
            return Err(ConfigError::EmptyApiKey);
        }
        // R0002-0037 / R0003-0009: one rule for "this identifier names a
        // model", stated once on `ModelId` and reused here rather than
        // restated.
        let model = ModelId::parse(model.0)?;
        if let Some(url) = &base_url {
            validate_base_url(url)?;
        }
        Ok(Self::new(api_key, model, base_url))
    }

    /// Apply an explicit reasoning effort to subsequent requests.
    ///
    /// Chat Completions receives `reasoning_effort`; Responses receives the
    /// equivalent nested `reasoning.effort` object.
    pub fn with_reasoning_effort(mut self, effort: ReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }

    /// Set the total per-request HTTP budget for **this instance**,
    /// overriding the 120-second default.
    ///
    /// The budget spans the whole request, first byte sent to last byte
    /// read, and is spent again on every bounded provider retry. The
    /// 10-second connect budget keeps its own separate clock and is not
    /// affected (R0002-0042) — a caller that lengthens the request budget
    /// still fails fast on a connect that was never going to complete.
    ///
    /// **Why a builder and not a constructor argument.** A host that
    /// exposes its own timeout knob could not honor it: the budget was a
    /// private const, so a configured 600s waited on a request this crate
    /// killed at 120s, and the host's own watchdog documented a premise
    /// ("the provider's timeout fires first") that was false for every
    /// value above the default. Model, base URL, reasoning effort and the
    /// HTTP surface were all expressible per instance; this was the one
    /// budget that was not.
    ///
    /// **Not part of the fingerprint, deliberately.** Every other
    /// per-instance axis feeds [`Self::fingerprint`] because it changes
    /// what the provider returns. A timeout changes only *whether* a
    /// result arrives in time, never its content, so folding it into the
    /// cache namespace would split entries between hosts that merely tune
    /// their patience — two runs with the same model, prompt and glossary
    /// would stop sharing work for no semantic reason.
    ///
    /// Rebuilding the client here (rather than overriding per request)
    /// keeps the dispatch path untouched: `client::call_*` still takes one
    /// `&reqwest::Client` and asks nothing about budgets.
    ///
    /// TRACE: contracts.md §7
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

    /// Pin the HTTP surface for **this instance**, overriding whatever the
    /// constructor resolved from `TRANSYNC_OPENAI_API` or the model-name
    /// heuristic.
    ///
    /// The environment variable is process-global, so it cannot express
    /// "this adapter on Chat Completions, that one on Responses" — a host
    /// wanting both at once had only `std::env::set_var` between the two
    /// constructions, which is `unsafe` under edition 2024 and racy against
    /// every other thread. This makes the surface configuration, like
    /// `model` and `base_url` (ticket `42c8e6d3`).
    ///
    /// The surface is a plain field read by *both* [`Self::fingerprint`]
    /// and every request path, so pinning it moves the cache namespace with
    /// it — a pinned instance can neither reuse nor poison entries written
    /// under the other surface. Order does not matter: this and
    /// [`Self::with_reasoning_effort`] set independent fields.
    ///
    /// ```
    /// # use secrecy::SecretString;
    /// # use transync_openai::{Api, ModelId, TransyncOpenAI};
    /// // `o3-mini` would go to the Responses API by heuristic; this host
    /// // is talking to a proxy that only implements Chat Completions.
    /// let provider = TransyncOpenAI::new(
    ///     SecretString::new("sk-…".to_string().into()),
    ///     ModelId::new("o3-mini"),
    ///     None,
    /// )
    /// .with_api(Api::ChatCompletions);
    /// assert_eq!(provider.api(), Api::ChatCompletions);
    /// ```
    pub fn with_api(mut self, api: client::Api) -> Self {
        self.api = api;
        self
    }

    /// Construct from environment variables per `contracts.md` §7.
    ///
    /// Reads `OPENAI_API_KEY`, honors `TRANSYNC_OPENAI_MODEL` and
    /// `TRANSYNC_OPENAI_BASE_URL` when present. Like [`Self::new`] — which
    /// it reaches through [`Self::try_new`] — it also resolves
    /// `TRANSYNC_OPENAI_API` once, here.
    ///
    /// **The environment path applies exactly [`Self::try_new`]'s rules**,
    /// because it *is* [`Self::try_new`] once the three variables are read
    /// (R0003-0011): the key it found must not be blank, the model it found
    /// goes through [`ModelId::parse`], and the base URL it found is
    /// validated the same way, cleartext warning included. Only the
    /// fallbacks are this function's own: a `TRANSYNC_OPENAI_MODEL` that is
    /// empty *or whitespace-only* names no model, so it takes the default
    /// rather than riding out as one (R0002-0037) — while a variable that
    /// *does* name a model, with whitespace around it, is
    /// [`ConfigError::PaddedModel`] rather than a silently different model
    /// than the direct constructor would have used.
    ///
    /// TRACE: contracts.md §7
    pub fn from_env() -> Result<Self, ConfigError> {
        let key = std::env::var("OPENAI_API_KEY").map_err(|_| ConfigError::MissingApiKey)?;
        let model = model_from_env_value(std::env::var("TRANSYNC_OPENAI_MODEL").ok())?;
        let base_url = match std::env::var("TRANSYNC_OPENAI_BASE_URL").ok() {
            Some(s) if !s.is_empty() => {
                Some(Url::parse(&s).map_err(|e| ConfigError::MalformedUrl(e.to_string()))?)
            }
            _ => None,
        };
        Self::try_new(SecretString::new(key.into()), model, base_url)
    }

    /// The OpenAI HTTP surface this instance was built to call.
    ///
    /// Resolved once at construction — `TRANSYNC_OPENAI_API` when set to a
    /// recognized value, the [`client::api_for_model`] heuristic otherwise
    /// — or set outright by [`Self::with_api`], and read from here by both
    /// [`Self::fingerprint`] and every request. Exposed so a caller can log
    /// or assert which surface a configured adapter will actually use,
    /// instead of re-deriving it and hoping the two derivations agree
    /// (R0001-0031).
    pub fn api(&self) -> client::Api {
        self.api
    }
}

/// The model [`TransyncOpenAI::from_env`] resolves from
/// `TRANSYNC_OPENAI_MODEL`, or the built-in default when the variable names
/// nothing.
///
/// The whole environment-side model rule, in one place and callable without a
/// process-global variable (which is why it is a function and not three lines
/// inside `from_env`): an unset variable, and one holding nothing but
/// whitespace, both take the default rather than riding out as the model on
/// the wire (R0002-0037); anything else goes through [`ModelId::parse`], so
/// the padding rule that governs the direct constructor governs this entry
/// point too (R0003-0011) instead of the environment being the weaker door.
fn model_from_env_value(raw: Option<String>) -> Result<ModelId, ConfigError> {
    match raw.filter(|s| !s.trim().is_empty()) {
        Some(id) => ModelId::parse(id),
        None => Ok(ModelId::new(DEFAULT_MODEL)),
    }
}

/// The model [`TransyncOpenAI::from_env`] uses when `TRANSYNC_OPENAI_MODEL`
/// names nothing.
const DEFAULT_MODEL: &str = "gpt-5-chat-latest";

/// Reject a custom base URL that is not an `http`/`https` URL with a host,
/// or that carries userinfo, so a misconfiguration (`file:`, `ftp:`, a
/// scheme-less string that parses as an opaque URL, an embedded credential)
/// fails fast at construction rather than as a confusing transport error at
/// request time (R0008-0033).
///
/// **Userinfo is refused rather than ignored** (R0004-0023, found on the
/// sibling adapter and mirrored here because the defect is the same shape).
/// This provider authenticates with `Authorization: Bearer`, so
/// `https://user:pass@gateway/` names no credential this crate would use —
/// but the endpoint builder clears only the query, the fragment and the
/// path, so the userinfo survives onto the wire, where `reqwest` turns it
/// into an `Authorization: Basic` header that *collides with* the bearer this
/// adapter sets. The URL is also the one part of the configuration that gets
/// echoed into logs and diagnostics, so an embedded password leaks where the
/// api key deliberately never does. There is no legitimate use to preserve,
/// which is what makes refusing it cheaper than sanitizing it. The message
/// names the shape and never the value.
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
             provider authenticates with the Authorization: Bearer header, and an embedded \
             credential would collide with it and surface wherever the URL is logged"
                .to_string(),
        ));
    }
    Ok(())
}

/// Whether a base URL would carry the bearer credential in cleartext to a
/// host that is not this machine — `http` scheme, non-loopback host.
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

/// R0002-0005: `http` to a **loopback** host is silent — that is how an
/// ollama or litellm gateway on the same box is addressed, and nothing
/// leaves the machine. `http` to any other host is still *allowed*, because
/// a plain-http gateway on a trusted intranet is a legitimate deployment and
/// refusing it would break real ones, but it is announced once per process:
/// the API key rides on every request as a bearer token and the document
/// content rides in every request body, so both are readable — and
/// alterable — by anything on the path.
///
/// One warning per process, not one per adapter: a host that builds many
/// adapters against the same gateway has one misconfiguration, not many.
///
/// A plain flag rather than a [`std::sync::Once`] so the tests can put it back
/// and assert the emission itself; the exactly-once guarantee is the same
/// (whoever wins the `swap` emits, everyone else returns), and no caller
/// blocks behind the one that is emitting.
static CLEARTEXT_BASE_URL_WARNED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Emit [`CLEARTEXT_BASE_URL_WARNED`]'s warning if this base URL earns it.
///
/// Called from [`TransyncOpenAI::new`] and **only** from there:
/// `try_new` and `from_env` both end in `new`, so one call site covers every
/// way of building an adapter and none of them can drift from the others.
/// Announcing is not validating — nothing here refuses anything — so `new`'s
/// recorded "validates nothing" contract (R0001-0033) is intact.
fn warn_once_on_cleartext_base_url(base_url: &Url) {
    use std::sync::atomic::Ordering;
    if !is_cleartext_to_remote_host(base_url) {
        return;
    }
    if CLEARTEXT_BASE_URL_WARNED.swap(true, Ordering::Relaxed) {
        return;
    }
    tracing::warn!(
        target: "transync::openai",
        "base URL {:?} uses cleartext http to a non-loopback host: the API key travels as a bearer token and the document content as the request body, both readable and alterable by anything on the network path; use https unless the whole path is trusted",
        base_url.host_str().unwrap_or_default()
    );
}

#[async_trait::async_trait]
impl Translator for TransyncOpenAI {
    /// Live OpenAI call with Structured Outputs. Goes to either
    /// `/v1/chat/completions` or `/v1/responses` per [`Self::api`] — the
    /// surface this instance resolved at construction from the model name
    /// (see [`client::api_for_model`]) or from `TRANSYNC_OPENAI_API`.
    ///
    /// Cancellation (DCR-0024): the round-trip is raced against `cancel`, and
    /// losing that race drops the `reqwest` future — which is what actually
    /// aborts the in-flight request; there is no other abort handle on a
    /// `reqwest` call. The pipeline would drop this future anyway, so what
    /// honoring the token adds is a *typed* answer
    /// ([`TranslatorError::Cancelled`]) for the caller that drives this
    /// adapter directly, and one that arrives without waiting out
    /// [`Self::request_timeout`].
    ///
    /// TRACE: SCN-12
    /// TRACE: contracts.md §1
    /// TRACE: DCR-0024
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
            r = client::translate_on(
                self.api,
                &self.http,
                &self.api_key,
                self.base_url.as_ref(),
                &self.model,
                self.reasoning_effort,
                &batch,
            ) => r,
        }
    }

    /// Cache-namespace identity covering model + effective base URL +
    /// resolved API surface + reasoning effort — every output-affecting configuration axis
    /// of this instance (R0008-0002). The configured base-URL string is
    /// used verbatim (not a normalized endpoint): equivalent-but-
    /// differently-spelled URLs producing distinct fingerprints only
    /// over-invalidates, which is the safe direction.
    ///
    /// Every axis is read from a field, [`Self::api`] included, so the
    /// namespace this returns is the configuration the requests actually
    /// run under — not a second, independently-derived guess at it
    /// (R0001-0031). That holds for a surface pinned by
    /// [`Self::with_api`] exactly as it does for a resolved one.
    ///
    /// EXT-2026-07 P1-4
    fn fingerprint(&self) -> ProviderFingerprint {
        let base = self
            .base_url
            .as_ref()
            .map(url::Url::as_str)
            .unwrap_or(client::DEFAULT_BASE_URL);
        let api = self.api.as_str();
        let effort = self
            .reasoning_effort
            .map(ReasoningEffort::as_str)
            .unwrap_or("default");
        ProviderFingerprint::new("openai", &[&self.model.0, base, api, effort])
    }

    /// OI-0026: harvest a candidate glossary in one preflight call, on the
    /// same surface (and with the same reasoning effort) this instance
    /// uses for translation — literally [`Self::api`], not a second
    /// derivation of it — extraction quality scales with the same
    /// knob, and `fingerprint()` already covers effort, so no fingerprint
    /// change is needed: the harvested terms reach the cache key through
    /// the glossary/prompt hashes, not the provider namespace.
    ///
    /// `Ok(Some(_))` even for an empty harvest — this provider *does*
    /// support extraction, which the report distinguishes from the trait
    /// default's `Ok(None)`. Errors are advisory: the pipeline records the
    /// failure on `ValidationReport.auto_glossary` and proceeds on the static
    /// glossary.
    ///
    /// Cancellation (DCR-0024): raced like [`Self::translate_batch`]. The
    /// preflight is the one provider call a run makes *before* any batch, so
    /// it is also the one whose stall a cancelled run would notice first.
    ///
    /// TRACE: SCN-09
    /// TRACE: OI-0026
    /// TRACE: DCR-0024
    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &transync::CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let entries = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TranslatorError::Cancelled),
            r = client::extract_glossary_on(
                self.api,
                &self.http,
                &self.api_key,
                self.base_url.as_ref(),
                &self.model,
                self.reasoning_effort,
                req,
            ) => r?,
        };
        Ok(Some(entries))
    }

    /// Declare the tiktoken encoder for THIS instance's model so batch
    /// budgeting stops inferring it from the advisory
    /// `TranslateOptions::model_id` (OI-0029). Mirrors core's legacy
    /// heuristic exactly, so CLI runs — which hand the same resolved model
    /// string to both — pack identically to before.
    ///
    /// TRACE: SCN-10
    /// TRACE: OI-0029
    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        Some(tokenizer::tokenizer_hint_for_model(&self.model.0))
    }
}

/// DNS + TCP + TLS, on its own budget (R0002-0042). Far above any reachable
/// handshake and far below [`REQUEST_TIMEOUT`], so it can only bite on a
/// connect that was never going to complete.
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// The whole request, from the first byte sent to the last byte read.
const REQUEST_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// The configuration [`build_http_client`] builds from. Split out from it
/// because a built [`reqwest::Client`] does not report its connect timeout in
/// any form — its `Debug` prints the total timeout and not that one — while
/// the builder does, which is what lets the two budgets be pinned by a test
/// instead of taken on faith (R0002-0042).
///
/// Two budgets, not one: without [`CONNECT_TIMEOUT`], a black-holed connect —
/// a firewall that drops SYNs, a stale DNS record — spends the entire
/// [`REQUEST_TIMEOUT`] before a single byte is sent, and spends it again on
/// every bounded provider retry.
/// The connect budget is deliberately NOT parameterized: it guards a
/// connect that was never going to complete, which is orthogonal to how
/// long a host is willing to wait for a response (R0002-0042). Callers
/// that want the default budget pass [`REQUEST_TIMEOUT`].
///
/// **Redirects are not followed** (R0004-0001). This adapter's credential
/// rides in `Authorization: Bearer`, which `reqwest` *does* strip on a
/// cross-origin redirect, so this crate was never the one that leaked — the
/// sibling `transync-anthropic` was, because its provider's `x-api-key` is a
/// custom header and `reqwest` strips only its own fixed sensitive set. The
/// policy is set here anyway so the two authenticated provider clients have
/// one deliberate posture instead of one of them being safe by accident of
/// which header its provider chose, and so a later credential change on this
/// side cannot silently re-open the hole. Nothing legitimate is lost: this
/// adapter posts to one endpoint it builds itself, and `reqwest` would
/// rewrite a followed 301/302/303 into a bodyless `GET` anyway. The
/// unfollowed `3xx` reaches
/// `client::classify::provider_error_for_status`, which names it terminal.
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
        SecretString::new("sk-test".to_string().into())
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

    #[test]
    fn reasoning_effort_parses_cross_generation_values() {
        for value in ["minimal", "none", "low", "medium", "high", "xhigh", "max"] {
            let effort: ReasoningEffort = value.parse().expect("supported effort");
            assert_eq!(effort.as_str(), value);
        }
        assert!("turbo".parse::<ReasoningEffort>().is_err());
    }

    #[test]
    fn try_new_rejects_non_http_base_url() {
        // R0008-0033: a `file:`/scheme-less base URL fails fast.
        let url = Url::parse("file:///etc/passwd").unwrap();
        let result = TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(url));
        assert!(
            matches!(result, Err(ConfigError::MalformedUrl(_))),
            "non-http base URL must be rejected"
        );
    }

    #[test]
    fn try_new_accepts_https_base_url() {
        let url = Url::parse("https://proxy.example/v1").unwrap();
        assert!(
            TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(url)).is_ok()
        );
    }

    /// R0004-0023 (found on the sibling adapter, mirrored here): a base URL
    /// carrying userinfo is a misconfiguration with no legitimate use — the
    /// credential is the bearer header and nothing else — and the endpoint
    /// builder preserves userinfo onto the wire, where `reqwest` turns it
    /// into an `Authorization: Basic` that collides with that bearer. Both
    /// halves are refused: `user@`, `:password@`, and the full pair. The
    /// diagnostic names the shape and never the value.
    #[test]
    fn try_new_rejects_a_base_url_carrying_userinfo() {
        for embedded in [
            "https://user@gateway.example/v1",
            "https://:hunter2@gateway.example/v1",
            "https://user:hunter2@gateway.example/v1",
            "http://user:hunter2@127.0.0.1:8080/v1",
        ] {
            let url = Url::parse(embedded).expect("test URL parses");
            let refused =
                TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(url))
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
            TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(clean)).is_ok(),
            "the rule must refuse userinfo and nothing else"
        );
    }

    /// R0002-0037: an identifier that names no model is a *configuration*
    /// error raised here, not a remote 400 after a request went out — and
    /// whitespace is no more of a model name than emptiness is.
    #[test]
    fn try_new_rejects_a_model_that_names_nothing() {
        for blank in ["", " ", "\t\n"] {
            let result = TransyncOpenAI::try_new(key(), ModelId::new(blank), None);
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
        let model = ModelId::parse("gpt-5-chat-latest").expect("a real model id is accepted");
        assert_eq!(model.0, "gpt-5-chat-latest");
        assert!(TransyncOpenAI::try_new(key(), model, None).is_ok());
    }

    /// R0003-0009: `ModelId::parse`'s own contract says a padded identifier is
    /// refused rather than trimmed — because the stored string is at once the
    /// wire value and a fingerprint axis — and the code used to store it. A
    /// padded id reached the provider as a remote 400 and fingerprinted as a
    /// cache namespace of its own.
    #[test]
    fn model_id_parse_rejects_surrounding_whitespace() {
        for padded in [
            " gpt-5-chat-latest",
            "gpt-5-chat-latest ",
            " gpt-5-chat-latest ",
            "\tgpt-5-chat-latest\n",
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
                    TransyncOpenAI::try_new(key(), ModelId::new(padded), None),
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

    /// R0003-0010: whitespace authenticates nothing. `"   "` passed the
    /// emptiness check and was found out as a 401 one request later — a
    /// remote answer about the request, for a fault visible in the
    /// configuration.
    #[test]
    fn try_new_rejects_a_whitespace_only_api_key() {
        for blank in ["", " ", "\t\n  "] {
            let result = TransyncOpenAI::try_new(
                SecretString::new(blank.to_string().into()),
                ModelId::new("gpt-5-chat-latest"),
                None,
            );
            assert!(
                matches!(result, Err(ConfigError::EmptyApiKey)),
                "an api key of {blank:?} must be refused at construction"
            );
        }
    }

    /// R0003-0011: the environment door applies the same model rule as the
    /// direct one. A variable that names nothing still falls back to the
    /// default (R0002-0037); a variable that names a model with whitespace
    /// around it is refused instead of quietly becoming a different model —
    /// and a different cache namespace — than `try_new` would have accepted.
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
        let model = model_from_env_value(Some("o3-mini".to_string())).expect("a real id is used");
        assert_eq!(model.0, "o3-mini");
        assert!(
            matches!(
                model_from_env_value(Some(" o3-mini ".to_string())),
                Err(ConfigError::PaddedModel(_))
            ),
            "the environment must not be the weaker door"
        );
    }

    /// R0002-0005: cleartext `http` is refused for nobody — an intranet
    /// gateway is a real deployment — but the adapter must be able to tell
    /// "never leaves this machine" from "crosses a network in the clear",
    /// because only the second one puts the bearer token on the wire
    /// unencrypted. `https` is never the risk, whatever the host.
    #[test]
    fn cleartext_risk_is_recognized_only_off_the_loopback() {
        let _turn = cleartext_turn();
        for silent in [
            "http://localhost:11434",
            "http://LOCALHOST/v1",
            "http://localhost./v1",
            "http://ollama.localhost/v1",
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
                TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(url))
                    .is_ok(),
                "{warned} must still construct — the warning is not a refusal"
            );
        }
    }

    /// R0002-0005, the other half: the predicate above decides *what* earns a
    /// warning, this pins that the warning is actually emitted and that the
    /// constructors are wired to the thing that emits it. Deleting the single
    /// call site — or moving it somewhere a constructor can miss — turns this
    /// red, which the pure predicate's test cannot do.
    ///
    /// `from_env` is not driven here because it reads process-global
    /// environment variables that a parallel test binary cannot own; it needs
    /// no separate pin, because it reaches the warning the only way anything
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
                TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), Some(url));
            }),
            ("try_new", |url| {
                TransyncOpenAI::try_new(key(), ModelId::new("gpt-5-chat-latest"), Some(url))
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
            assert_eq!(target, "transync::openai", "{name}: {message}");
            for named in ["proxy.example", "bearer token", "https"] {
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
                TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), Some(remote()));
            }
            for silent in ["https://proxy.example/v1", "http://localhost:11434/v1"] {
                let url = Url::parse(silent).expect("test URL parses");
                TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), Some(url));
            }
        });
        assert_eq!(
            captured.taken().len(),
            1,
            "five adapters against one gateway are one misconfiguration, not five"
        );
    }

    /// R0002-0042: the two budgets exist and are distinct. The *behavior* is
    /// only observable against a host that black-holes SYNs, which no
    /// portable test can conjure — a bound-but-unaccepting socket completes
    /// the handshake out of the accept backlog and so exercises the request
    /// timeout instead, passing with or without the connect budget. So this
    /// pins the configuration where reqwest reports it: the builder's own
    /// `Debug`, which names both timeouts where a built `Client`'s names only
    /// the total one. Deleting either budget turns this red; so does a
    /// reqwest upgrade that renames the fields, which is the failure mode to
    /// want — loud rather than silently vacuous.
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
    /// assertion that the default was replaced. Set here for posture parity
    /// with `transync-anthropic`, whose provider's credential is a custom
    /// header `reqwest` would not strip on a cross-origin hop; the
    /// behavioral half lives in `client::transport`'s two-socket test.
    #[test]
    fn the_http_client_follows_no_redirects() {
        for budget in [REQUEST_TIMEOUT, std::time::Duration::from_secs(600)] {
            let configured = format!("{:?}", http_client_builder_with(budget));
            assert!(
                configured.contains("redirect_policy: Policy(None)"),
                "the two provider clients must refuse redirects identically: {configured}"
            );
        }
    }

    /// `with_timeout` must reach the client it claims to configure — the
    /// whole point of the builder is that a host's configured budget stops
    /// being advisory. Pinned at the builder's `Debug` for the same reason
    /// as the test above: a built `Client` will not report the connect
    /// budget, so this is where both are visible at once.
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

        let adapter = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None)
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
        let adapter = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None);
        assert_eq!(adapter.request_timeout(), REQUEST_TIMEOUT);
    }

    /// The timeout is deliberately NOT a fingerprint axis: it changes
    /// whether a result arrives in time, never its content. Folding it in
    /// would split the cache between hosts that merely tune their patience,
    /// so two adapters differing only in budget must keep sharing entries.
    #[test]
    fn the_timeout_is_not_part_of_the_cache_namespace() {
        let patient = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None)
            .with_timeout(std::time::Duration::from_secs(600));
        let hasty = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None)
            .with_timeout(std::time::Duration::from_secs(5));
        assert_eq!(
            patient.fingerprint(),
            hasty.fingerprint(),
            "a timeout must not fragment the cache namespace",
        );
    }

    /// EXT-2026-07 P1-4: the fingerprint distinguishes model, effective
    /// base URL, and resolved API surface, and is stable across calls.
    #[test]
    fn fingerprint_covers_model_base_url_and_api() {
        // Different model (and, incidentally, API surface: chat vs
        // responses) → different fingerprint.
        let chat = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None);
        let responses = TransyncOpenAI::new(key(), ModelId::new("o3-mini"), None);
        assert_ne!(chat.fingerprint(), responses.fingerprint());

        // Same model, custom base URL → different fingerprint.
        let custom = TransyncOpenAI::new(
            key(),
            ModelId::new("gpt-5-chat-latest"),
            Some(Url::parse("https://proxy.example/v1").unwrap()),
        );
        assert_ne!(chat.fingerprint(), custom.fingerprint());

        // Reasoning effort affects outputs and therefore the cache namespace.
        let medium = TransyncOpenAI::new(key(), ModelId::new("gpt-5-chat-latest"), None)
            .with_reasoning_effort(ReasoningEffort::Medium);
        assert_ne!(chat.fingerprint(), medium.fingerprint());

        // Stable across two calls on the same instance.
        assert_eq!(chat.fingerprint(), chat.fingerprint());

        // The default-base instance carries the canonical base URL.
        assert!(
            chat.fingerprint()
                .as_str()
                .contains(client::DEFAULT_BASE_URL)
        );
    }

    /// R0001-0031: the surface named in the fingerprint is the field the
    /// request paths dispatch on — one stored value, read twice, never
    /// re-derived. Env-independent on purpose: whatever
    /// `TRANSYNC_OPENAI_API` happens to say in the ambient environment, the
    /// two readings must agree. The end-to-end proof that they also survive
    /// a mid-run mutation lives in `tests/api_surface_identity.rs`, which
    /// owns its own process.
    #[test]
    fn fingerprint_names_the_stored_surface() {
        for model in ["gpt-5-chat-latest", "o3-mini", "gpt-4o", "gpt-5.6-terra"] {
            let t = TransyncOpenAI::new(key(), ModelId::new(model), None);
            let expected = match t.api() {
                client::Api::ChatCompletions => "chat",
                client::Api::Responses => "responses",
            };
            let printed = t.fingerprint();
            // Each part is framed by its byte length (R0002-0007), in the
            // order ⟨provider⟩⟨model⟩⟨base⟩⟨api⟩⟨effort⟩ — so the surface
            // is named as e.g. `4\u{1F}chat`, and a model or base URL that
            // happens to *spell* that cannot be mistaken for it.
            let expected_field = format!("{}\u{1F}{expected}", expected.len());
            assert!(
                printed.as_str().contains(&expected_field),
                "{model}: fingerprint {:?} must name the stored surface {expected:?}",
                printed.as_str()
            );
            // …and the accessor is not itself re-deriving on each call.
            assert_eq!(t.api(), t.api());
        }
    }

    /// The reasoning-effort builder rebuilds the struct; the resolved
    /// surface must survive that move rather than being silently re-read.
    #[test]
    fn with_reasoning_effort_preserves_the_resolved_surface() {
        let plain = TransyncOpenAI::new(key(), ModelId::new("o3-mini"), None);
        let before = plain.api();
        let with_effort = plain.with_reasoning_effort(ReasoningEffort::Low);
        assert_eq!(with_effort.api(), before);
    }

    /// Ticket `42c8e6d3`: pinning the surface per instance beats whatever
    /// the constructor resolved, in both directions, and does not depend on
    /// what `TRANSYNC_OPENAI_API` says in the ambient environment — which
    /// is the whole point, so the assertion must not read it either.
    #[test]
    fn with_api_pins_the_surface_per_instance() {
        for target in [Api::ChatCompletions, Api::Responses] {
            for model in ["gpt-5-chat-latest", "o3-mini", "gpt-4o", "gpt-5.6-terra"] {
                let pinned = TransyncOpenAI::new(key(), ModelId::new(model), None).with_api(target);
                assert_eq!(pinned.api(), target, "{model} pinned to {target}");

                // Independent fields: pinning survives the other builder,
                // in either order.
                let then_effort = TransyncOpenAI::new(key(), ModelId::new(model), None)
                    .with_api(target)
                    .with_reasoning_effort(ReasoningEffort::Low);
                let then_api = TransyncOpenAI::new(key(), ModelId::new(model), None)
                    .with_reasoning_effort(ReasoningEffort::Low)
                    .with_api(target);
                assert_eq!(then_effort.api(), target);
                assert_eq!(then_api.api(), target);
                assert_eq!(then_effort.fingerprint(), then_api.fingerprint());
            }
        }
    }

    /// Ticket `42c8e6d3`: the cache namespace follows the pin. Two adapters
    /// identical but for the pinned surface must not share entries, and a
    /// pin that merely restates what the heuristic already chose must not
    /// split them — the namespace tracks the surface, not how it was
    /// decided.
    #[test]
    fn with_api_moves_the_cache_namespace_with_the_surface() {
        // `gpt-4o` resolves to Chat Completions by heuristic (rule 4).
        let heuristic = TransyncOpenAI::new(key(), ModelId::new("gpt-4o"), None);
        assert_eq!(heuristic.api(), Api::ChatCompletions);

        let restated =
            TransyncOpenAI::new(key(), ModelId::new("gpt-4o"), None).with_api(Api::ChatCompletions);
        assert_eq!(
            restated.fingerprint(),
            heuristic.fingerprint(),
            "pinning the surface the heuristic already picked must not re-namespace"
        );

        let flipped =
            TransyncOpenAI::new(key(), ModelId::new("gpt-4o"), None).with_api(Api::Responses);
        assert_ne!(
            flipped.fingerprint(),
            heuristic.fingerprint(),
            "a pinned surface must namespace apart from the resolved one"
        );

        // …and the fingerprint names the pin, as a whole length-framed
        // field, not the surface the model name would have implied.
        assert!(flipped.fingerprint().as_str().contains(&format!(
            "{}\u{1F}{}",
            Api::Responses.as_str().len(),
            Api::Responses.as_str()
        )));
    }

    /// OI-0029: the declared encoder hint covers the same generations the
    /// API-dispatch heuristic knows about, and unknown names degrade to the
    /// documented `cl100k_base` approximation. A `*-chat-*` alias rides on
    /// Chat Completions but is still an o200k-era model — the two
    /// heuristics are deliberately independent.
    #[test]
    fn tokenizer_hint_matches_dispatch_generations() {
        for model in [
            "o3-mini",
            "gpt-5.6-terra",
            "gpt-5-chat-latest",
            "gpt-4o-mini",
        ] {
            let t = TransyncOpenAI::new(key(), ModelId::new(model), None);
            assert_eq!(
                t.tokenizer_hint(),
                Some(TokenizerHint::O200kBase),
                "{model} must report the o200k encoder"
            );
        }
        for model in ["gpt-4-turbo", "some-vendor/model-x"] {
            let t = TransyncOpenAI::new(key(), ModelId::new(model), None);
            assert_eq!(
                t.tokenizer_hint(),
                Some(TokenizerHint::Cl100kBase),
                "{model} must fall back to the cl100k approximation"
            );
        }
    }
}
