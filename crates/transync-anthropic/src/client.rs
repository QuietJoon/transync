//! The Messages API client: one HTTP surface, two use cases.
//!
//! `POST {base_url}/v1/messages` is the whole surface. There is no dispatch
//! layer here because there is nothing to dispatch between — what the OpenAI
//! adapter spends on choosing a surface, this module does not spend at all.
//!
//! # Layout
//!
//! This file is the *flow*; every mechanism it composes lives in a private
//! submodule, named below in plain code spans rather than intra-doc links
//! because they are crate-private and linking to them from a public module's
//! docs is exactly what the rustdoc gate rejects:
//!
//! - `messages` — the wire DTOs, both body builders (translation +
//!   extraction) and the single envelope reader
//! - `schema` — the deterministic schema-profile pass that renders a shared
//!   Structured-Output schema object into this provider's narrower dialect
//! - `endpoint` — the endpoint path, base-URL normalization, path joining
//! - `transport` — `x-api-key`-authenticated POST with the two body caps
//! - `classify` — status/`reqwest` failure → `ProviderError`, the
//!   retryable-vs-terminal table
//!
//! What is left here is `round_trip` — likewise private, likewise unlinked
//! for the reason above — plus the two use cases: **one**
//! request/response shape that both ride. Translation and glossary
//! extraction differ only in which builder shapes the request and which
//! parser reads the result — never in how the call is made or how its
//! failures are reported.
//!
//! The provider-neutral half — user-prompt assembly, the schema objects
//! themselves, and the response parsers — lives in `transync::llm::prompt`
//! and is **called, never reproduced** (tier (b), contracts.md §0). What
//! remains here is genuinely Anthropic-shaped.
//!
//! # An injected client carries its own posture
//!
//! [`translate_on`] and [`extract_glossary_on`] take whatever
//! `reqwest::Client` the caller hands them, and two of that client's
//! settings are load-bearing here. Both are settled at *client build* time
//! and neither can be re-decided per request, so neither is something this
//! module can impose on a client it did not build:
//!
//! - **The request budget is the caller's**, which is what `transport`'s
//!   docs have always said. `crate::build_http_client` puts a 10-second
//!   connect timeout inside a 120-second request timeout on the clients this
//!   crate builds; an injected client with neither can hang for as long as
//!   the far end keeps the socket open.
//! - **The redirect policy is the caller's on the same terms, and here that
//!   is a credential rule** (`R0004-0001`). The api key rides in the custom
//!   `x-api-key` header, which `reqwest` does **not** strip on a cross-origin
//!   hop — it strips only `Authorization`, `Cookie`, `Proxy-Authorization`
//!   and `Www-Authenticate`. A client left on `reqwest`'s default
//!   follow-up-to-ten policy therefore hands the key, and the document being
//!   translated, to whatever origin a `3xx` names.
//!   `crate::build_http_client` sets `redirect::Policy::none()`; an injected
//!   client should set it too.
//!
//! Prefer [`crate::TransyncAnthropic`], which builds and holds a client with
//! both already decided.
//!
//! TRACE: DCR-0029
//! TRACE: SCN-12
//! TRACE: contracts.md §8

mod classify;
mod endpoint;
mod messages;
mod schema;
mod transport;

use secrecy::SecretString;
use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, TranslationBatch, TranslationBatchResult,
    TranslatorError, prompt,
};
use url::Url;

use crate::{Effort, ModelId};
use endpoint::build_endpoint;
use messages::Shaped;
use transport::post_json;

/// Output ceiling for an extraction call. A term list is small, and a fixed
/// cap keeps a runaway generation bounded without touching the profile's
/// translation ceiling (`[batching].target_output_tokens`), which sizes
/// batch responses and not this one.
///
/// TRACE: OI-0026
const EXTRACTION_MAX_OUTPUT_TOKENS: u32 = 4096;

/// The single request/response round-trip: resolve the endpoint, POST the
/// body, read the model's output string back out of the envelope.
///
/// Both use cases go through here, so endpoint construction, transport, and
/// stop-reason reporting exist exactly once — and the ceiling the request
/// declared reaches the reader that has to diagnose it.
async fn round_trip(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    request: Shaped,
) -> Result<String, TranslatorError> {
    let endpoint = build_endpoint(base_url)?;
    let bytes = post_json(http, api_key, &endpoint, &request.body).await?;
    messages::extract_output(&bytes, request.ceiling)
}

/// Translate one batch: shape it, send it, parse the batch result.
///
/// Public as the crate's lower-level entry point, for a caller that holds no
/// adapter. **Unlike the OpenAI adapter's `call_api`, this cannot disagree
/// with an adapter's `fingerprint()`**: that function re-resolves the HTTP
/// surface from a process-global variable on every call, which is exactly why
/// `TransyncOpenAI` refuses to use it. Here every axis is an argument and
/// nothing is read from the environment, so the free function and an adapter
/// built with the same axes issue byte-identical requests. Prefer
/// [`crate::TransyncAnthropic`] anyway — it is what implements `Translator`,
/// and it decides the axes once instead of at every call site.
///
/// **`http` brings its own timeouts and its own redirect policy**, and a
/// client that follows redirects can carry the `x-api-key` header to another
/// origin — see this module's docs.
///
/// TRACE: SCN-12
pub async fn translate_on(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    effort: Option<Effort>,
    batch: &TranslationBatch,
) -> Result<TranslationBatchResult, TranslatorError> {
    let request = messages::translation_body(model, effort, batch)?;
    let output_text = round_trip(http, api_key, base_url, request).await?;
    prompt::parse_batch_output(&output_text, &batch.batch_id)
}

/// Issue the one-per-run candidate-glossary extraction preflight.
///
/// Same transport, same envelope reader (so every stop reason is reported
/// identically); only the schema, the prompts, the output cap and the final
/// parse differ.
///
/// Errors are returned to the pipeline, which degrades (warn + static
/// glossary only) instead of aborting — see
/// `transync::llm::Translator::extract_glossary`.
///
/// **`http` brings its own timeouts and its own redirect policy**, and a
/// client that follows redirects can carry the `x-api-key` header to another
/// origin — see this module's docs.
///
/// TRACE: SCN-09
/// TRACE: OI-0026
pub async fn extract_glossary_on(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    effort: Option<Effort>,
    req: &GlossaryExtractionRequest,
) -> Result<Vec<GlossaryEntry>, TranslatorError> {
    let request = messages::extraction_body(model, effort, req)?;
    let output_text = round_trip(http, api_key, base_url, request).await?;
    prompt::parse_extraction_output(&output_text)
}

/// Offline fixtures shared by this module's tests and the submodules' — one
/// copy, so every pin runs against identical input.
#[cfg(test)]
mod fixtures {
    use transync::BlockId;
    use transync::llm::{BatchId, GlossaryExtractionRequest, TranslationBatch, TranslationUnit};
    use transync::profile::{default_profile, render_prompt_body};

    /// Minimal batch for the request-shape tests. Nothing in this crate
    /// inspects per-unit hints — the richly populated hint fixture lives with
    /// the assembly it exercises, in core's `llm::prompt` tests (OI-0029) —
    /// so what is pinned here is the envelope around them.
    pub(super) fn fixture_batch() -> TranslationBatch {
        let profile = render_prompt_body(&default_profile(), "en", "ko");
        TranslationBatch {
            batch_id: BatchId::new(7),
            units: vec![
                TranslationUnit::new(
                    BlockId("p-0001".to_string()),
                    transync::BlockKind::Paragraph,
                    transync::InputMode::TextFragment,
                    "Hello world.".to_string(),
                    42,
                )
                .with_batch_id(BatchId::new(7)),
            ],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: profile.glossary.clone(),
            profile,
        }
    }

    /// Minimal extraction request for the request-shape tests (OI-0026).
    pub(super) fn fixture_extraction_request() -> GlossaryExtractionRequest {
        GlossaryExtractionRequest {
            source_text: "The agent invokes the tool.".to_string(),
            source_truncated: false,
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            existing_terms: vec!["agent".to_string()],
            max_terms: 9,
        }
    }
}
