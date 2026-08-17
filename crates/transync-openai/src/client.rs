//! Thin wrapper around the OpenAI HTTP APIs with model-driven dispatch
//! between Chat Completions and Responses.
//!
//! - Chat Completions (`POST {base_url}/v1/chat/completions`) is used
//!   for everything by default and is required for `*-chat-*` aliases
//!   like `gpt-5-chat-latest`.
//! - Responses (`POST {base_url}/v1/responses`) is used for the
//!   o-series reasoning models (`o1*`, `o3*`, `o4*`) and for non-chat
//!   GPT-5 snapshots. Set `TRANSYNC_OPENAI_API=chat|responses` to
//!   override the heuristic explicitly.
//!
//! Both transports send the **same** strict JSON schema
//! describing `TranslationBatchResult`; the wrapping field name differs
//! (`response_format.json_schema` for chat vs. `text.format` for
//! responses). The model's output JSON string is extracted from each
//! API's envelope shape and parsed back into the
//! `transync::Translator` contract.
//!
//! OI-0029: the provider-neutral half — user-prompt + hint assembly, the
//! Structured Output schema object, and the output-envelope parser — lives
//! in `transync::llm::prompt`, shared with every other provider so the
//! prompt wording cannot drift away from what core's validators enforce.
//! What remains here is genuinely OpenAI-shaped: model-name dispatch, the
//! two surface-specific request/response wrappers, and transport.
//!
//! # Layout (OI-0008 / `R0001-0079` in the removed `reviews/reviewed/0001.md`)
//!
//! This file is the *flow*; every mechanism it composes lives in a private
//! submodule. Those submodules are named below in plain code spans, not
//! intra-doc links: they are crate-private, so linking to them from this
//! public module's docs is exactly what the rustdoc gate rejects, and the
//! link would render as bare text anyway.
//!
//! - `dispatch` — model name → [`Api`]
//! - `chat` / `responses` — one module per HTTP surface, each owning
//!   its wire DTOs, both body builders (translation + extraction) and its
//!   envelope reader
//! - `endpoint` — the endpoint paths, base-URL normalization, path joining
//! - `transport` — bearer-authenticated POST with a response-size cap
//! - `classify` — status/`reqwest` failure → `ProviderError` (the
//!   retryable-vs-terminal table)
//!
//! What is left here is `SurfaceRequest` plus `round_trip` — likewise
//! private, likewise unlinked: **one** request/response shape that both
//! use cases ride. Translation and glossary extraction differ only in
//! which body builder shapes the request and which parser reads the
//! result — never in how the call is made or how its failures are
//! reported.
//!
//! # An injected client carries its own posture
//!
//! [`call_api`], [`call_chat_completions_api`], [`call_responses_api`] and
//! [`call_glossary_extraction`] take whatever `reqwest::Client` the caller
//! hands them, and two of that client's settings are load-bearing here. Both
//! are settled at *client build* time and neither can be re-decided per
//! request, so neither is something this module can impose on a client it
//! did not build:
//!
//! - **The request budget is the caller's**, which is what `transport`'s
//!   docs have always said. `crate::build_http_client` puts a 10-second
//!   connect timeout inside a 120-second request timeout on the clients this
//!   crate builds; an injected client with neither can hang for as long as
//!   the far end keeps the socket open.
//! - **The redirect policy is the caller's on the same terms** (`R0004-0001`).
//!   This adapter's credential rides as a bearer in `Authorization`, which
//!   *is* in the fixed set `reqwest` strips on a cross-origin hop, so the api
//!   key does not travel — but the **request body** does, and that body is
//!   the document being translated together with the prompt built around it.
//!   `crate::build_http_client` sets `redirect::Policy::none()`, the same
//!   posture the Anthropic adapter sets for the stronger reason its custom
//!   `x-api-key` header gives it; an injected client should set it too.
//!
//! Prefer [`crate::TransyncOpenAI`], which builds and holds a client with
//! both already decided.
//!
//! TRACE: SCN-12
//! TRACE: OI-0029

mod chat;
mod classify;
mod dispatch;
mod endpoint;
mod responses;
mod transport;

use crate::{ModelId, ReasoningEffort};
use secrecy::SecretString;
use serde::Serialize;
use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, TranslationBatch, TranslationBatchResult,
    TranslatorError, prompt,
};
use url::Url;

use endpoint::{CHAT_COMPLETIONS_PATH, RESPONSES_PATH, build_endpoint};
use transport::post_json;

pub(crate) use dispatch::api_from_env_or_model;
pub use dispatch::{Api, ParseApiError, api_for_model};

// The base URL and the endpoint paths belong to `endpoint`, which owns every
// rule about how a URL is built; re-exported here because `lib.rs` names the
// default when it decides whether a run is talking to OpenAI itself.
pub(crate) use endpoint::DEFAULT_BASE_URL;

// REQUEST_TIMEOUT is now configured on the shared `reqwest::Client`
// owned by `TransyncOpenAI`; see `crate::build_http_client`.

/// Output ceiling for an extraction call. A term list is small, and a fixed
/// cap keeps a runaway generation bounded without touching the profile's
/// translation ceiling (`[batching].target_output_tokens`), which sizes
/// batch responses, not this one.
///
/// TRACE: OI-0026
const EXTRACTION_MAX_OUTPUT_TOKENS: u32 = 4096;

/// The translation output ceiling a request will actually carry, read off the
/// batch's profile on core's own terms.
///
/// One place rather than two, because both surfaces send the same knob under
/// different names — `max_completion_tokens` on Chat Completions,
/// `max_output_tokens` on Responses — and a rule about what the knob *means*
/// must not be able to hold on one surface and not the other.
///
/// **A `Some(0)` is normalized to absent** (the sibling of `R0004-0016`,
/// closed on the Anthropic adapter first). Core's `profile::normalize_batching`
/// already maps zero to `None` — "not an output budget; ignored … exactly as
/// if the key were absent" — and it runs at both `load_profile` and
/// `unit::build_batches`, the latter owning the `ProfileMetadata` cloned onto
/// every `TranslationBatch`, so the pipeline never hands one down. A caller
/// that builds a `TranslationBatch` itself and calls this adapter directly
/// bypasses both. Absent on these surfaces means *omit the field*, so that is
/// what zero becomes: the narrow bypass then behaves the way the configured
/// door already does instead of spending a round trip to be told a zero
/// ceiling leaves nothing to answer in.
///
/// Only zero is normalized here. A nonzero-but-tiny ceiling produces a valid
/// request and a loud, terminal exhausted-ceiling diagnostic, which is the
/// designed behavior and a different failure entirely.
fn translation_output_ceiling(batch: &TranslationBatch) -> Option<u32> {
    match batch.profile.batching.target_output_tokens {
        // Zero names no budget; see this function's docs.
        Some(0) => None,
        other => other,
    }
}

// ---------------------------------------------------------------------------
// The one shared request shape
// ---------------------------------------------------------------------------

/// A request body together with the surface that shaped it.
///
/// This is the whole surface abstraction: the variant decides the endpoint
/// path and which envelope reader parses the reply, and nothing else in the
/// flow needs to know which API it is talking to. `#[serde(untagged)]` makes
/// the wrapper transparent on the wire — the bytes are exactly the inner
/// body's, which is what keeps the per-surface request pins in
/// [`chat`]/[`responses`] authoritative.
#[derive(Serialize)]
#[serde(untagged)]
enum SurfaceRequest {
    Chat(chat::Request),
    Responses(responses::Request),
}

impl SurfaceRequest {
    /// The endpoint path this body must be POSTed to.
    fn path(&self) -> &'static str {
        match self {
            SurfaceRequest::Chat(_) => CHAT_COMPLETIONS_PATH,
            SurfaceRequest::Responses(_) => RESPONSES_PATH,
        }
    }

    /// Pull the model's JSON string out of the reply, using the envelope
    /// reader belonging to the surface this request went to.
    fn extract_output(&self, bytes: &[u8]) -> Result<String, TranslatorError> {
        match self {
            SurfaceRequest::Chat(_) => chat::extract_output(bytes),
            SurfaceRequest::Responses(_) => responses::extract_output(bytes),
        }
    }
}

/// The single request/response round-trip: resolve the endpoint, POST the
/// body, read the model's output string back out of the surface's envelope.
///
/// Both use cases below go through here, so endpoint construction,
/// transport, and malformed-envelope reporting exist exactly once.
async fn round_trip(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    request: SurfaceRequest,
) -> Result<String, TranslatorError> {
    let endpoint = build_endpoint(base_url, request.path())?;
    let bytes = post_json(http, api_key, &endpoint, &request).await?;
    request.extract_output(&bytes)
}

// ---------------------------------------------------------------------------
// Use case 1: batch translation
// ---------------------------------------------------------------------------

/// Top-level dispatcher for a caller that holds no adapter. Picks the
/// right OpenAI HTTP surface for the configured model (or the explicit
/// `TRANSYNC_OPENAI_API` override) and forwards to the shared translation
/// flow.
///
/// Resolution happens **per call** here, because a bare function has
/// nowhere to remember it. [`crate::TransyncOpenAI`] does not use this
/// path: it resolves the surface at construction so its
/// `fingerprint()` and its requests cannot name different surfaces
/// (R0001-0031). Callers that want a pinned surface should say so with
/// [`call_chat_completions_api`] / [`call_responses_api`].
///
/// **`http` brings its own timeouts and its own redirect policy** — see this
/// module's docs.
///
/// TRACE: SCN-12
/// TRACE: contracts.md §1
/// TRACE: contracts.md §7
pub async fn call_api(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<TranslationBatchResult, TranslatorError> {
    let api = api_from_env_or_model(&model.0);
    translate_on(api, http, api_key, base_url, model, reasoning_effort, batch).await
}

/// Issue one Chat Completions API call with Structured Outputs.
///
/// **`http` brings its own timeouts and its own redirect policy** — see this
/// module's docs.
///
/// TRACE: SCN-12
pub async fn call_chat_completions_api(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<TranslationBatchResult, TranslatorError> {
    translate_on(
        Api::ChatCompletions,
        http,
        api_key,
        base_url,
        model,
        reasoning_effort,
        batch,
    )
    .await
}

/// Issue one Responses API call with Structured Outputs.
///
/// **`http` brings its own timeouts and its own redirect policy** — see this
/// module's docs.
///
/// TRACE: SCN-12
pub async fn call_responses_api(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<TranslationBatchResult, TranslatorError> {
    translate_on(
        Api::Responses,
        http,
        api_key,
        base_url,
        model,
        reasoning_effort,
        batch,
    )
    .await
}

/// Translate one batch on an already-chosen surface: shape it, send it,
/// parse the batch result.
///
/// `pub(crate)` so [`crate::TransyncOpenAI`] can hand in the surface it
/// resolved at construction rather than going through [`call_api`], which
/// would re-read the environment (R0001-0031).
pub(crate) async fn translate_on(
    api: Api,
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<TranslationBatchResult, TranslatorError> {
    let request = match api {
        Api::ChatCompletions => {
            SurfaceRequest::Chat(chat::translation_body(model, reasoning_effort, batch)?)
        }
        Api::Responses => {
            SurfaceRequest::Responses(responses::translation_body(model, reasoning_effort, batch)?)
        }
    };
    let output_text = round_trip(http, api_key, base_url, request).await?;
    prompt::parse_batch_output(&output_text, &batch.batch_id)
}

// ---------------------------------------------------------------------------
// Use case 2: candidate-glossary extraction preflight (OI-0026)
//
// Same two surfaces, same transport, same envelope readers (so refusal and
// `incomplete` handling comes free); only the schema, the prompts, the
// output cap, and the final parse differ. The provider-neutral halves —
// system/user prompt, schema object, response parser — live in
// `transync::llm::prompt`.
// ---------------------------------------------------------------------------

/// Issue the one-per-run candidate-glossary extraction call, choosing the
/// surface the way [`call_api`] does — per call, from the environment or
/// the model name. The same caveat applies: [`crate::TransyncOpenAI`] uses
/// the private `extract_glossary_on` with its construction-time surface
/// instead (R0001-0031).
///
/// Errors are returned to the pipeline, which degrades (warn + static
/// glossary only) instead of aborting — see
/// `transync::llm::Translator::extract_glossary`.
///
/// **`http` brings its own timeouts and its own redirect policy** — see this
/// module's docs.
///
/// TRACE: SCN-09
/// TRACE: OI-0026
pub async fn call_glossary_extraction(
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    req: &GlossaryExtractionRequest,
) -> Result<Vec<GlossaryEntry>, TranslatorError> {
    let api = api_from_env_or_model(&model.0);
    extract_glossary_on(api, http, api_key, base_url, model, reasoning_effort, req).await
}

/// Extract a glossary on an already-chosen surface — the extraction twin
/// of [`translate_on`], so both of an adapter's call paths ride the one
/// surface it resolved at construction.
pub(crate) async fn extract_glossary_on(
    api: Api,
    http: &reqwest::Client,
    api_key: &SecretString,
    base_url: Option<&Url>,
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    req: &GlossaryExtractionRequest,
) -> Result<Vec<GlossaryEntry>, TranslatorError> {
    let request = match api {
        Api::ChatCompletions => {
            SurfaceRequest::Chat(chat::extraction_body(model, reasoning_effort, req)?)
        }
        Api::Responses => {
            SurfaceRequest::Responses(responses::extraction_body(model, reasoning_effort, req)?)
        }
    };
    let output_text = round_trip(http, api_key, base_url, request).await?;
    prompt::parse_extraction_output(&output_text)
}

/// Offline fixtures shared by this module's tests and the per-surface
/// modules' — one copy, so both surfaces are pinned against identical input.
#[cfg(test)]
mod fixtures {
    use transync::BlockId;
    use transync::llm::{BatchId, GlossaryExtractionRequest, TranslationBatch, TranslationUnit};
    use transync::profile::{default_profile, render_prompt_body};

    /// Minimal batch for the surface-wrapper tests. OI-0029: the richly
    /// populated hint fixture moved to `transync_core::llm::prompt`'s test
    /// module together with the assembly it exercises — nothing in these
    /// modules inspects per-unit hints, only the request envelopes around
    /// them.
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
                // The old literal stamped the enclosing batch's id (not the
                // `BatchId::new(0)` placeholder), so keep it explicit.
                .with_batch_id(BatchId::new(7)),
            ],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: profile.glossary.clone(),
            profile,
        }
    }

    /// OI-0026: minimal extraction request for the surface-wrapper tests.
    /// The prompt/schema/parse halves are pinned in
    /// `transync_core::llm::prompt`; what is provider-shaped is the envelope
    /// around them.
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

// OI-0009: unit tests for the pure request/response plumbing. These run
// under plain `cargo test --workspace` — no API key, no network.
//
// The single-surface request/envelope pins live next to their surface in
// `chat.rs` and `responses.rs`; what stays here is what is genuinely about
// *both* surfaces at once.
#[cfg(test)]
mod tests {
    use super::*;
    use fixtures::{fixture_batch, fixture_extraction_request};

    #[test]
    fn reasoning_effort_uses_surface_specific_request_shapes() {
        let batch = fixture_batch();
        let chat = chat::translation_body(
            &ModelId::new("gpt-5-chat-latest"),
            Some(ReasoningEffort::Medium),
            &batch,
        )
        .expect("chat body builds");
        let chat = serde_json::to_value(&chat).expect("chat body serializes");
        assert_eq!(chat["reasoning_effort"], "medium");
        assert!(chat.get("reasoning").is_none());

        let responses = responses::translation_body(
            &ModelId::new("gpt-5.6-terra"),
            Some(ReasoningEffort::Medium),
            &batch,
        )
        .expect("responses body builds");
        let responses = serde_json::to_value(&responses).expect("responses body serializes");
        assert_eq!(responses["reasoning"]["effort"], "medium");
        assert!(responses.get("reasoning_effort").is_none());
    }

    #[test]
    fn reasoning_effort_is_omitted_when_unconfigured() {
        let batch = fixture_batch();
        let chat = chat::translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("chat body builds");
        let responses = responses::translation_body(&ModelId::new("gpt-5.6-terra"), None, &batch)
            .expect("responses body builds");
        assert!(
            serde_json::to_value(chat)
                .expect("chat body serializes")
                .get("reasoning_effort")
                .is_none()
        );
        assert!(
            serde_json::to_value(responses)
                .expect("responses body serializes")
                .get("reasoning")
                .is_none()
        );
    }

    /// OI-0026: both surfaces carry the extraction schema under their own
    /// wrapper, announce the extraction schema name, opt out of retention
    /// (Responses), and cap output at [`EXTRACTION_MAX_OUTPUT_TOKENS`]
    /// regardless of the profile's translation ceiling. Reasoning effort
    /// keeps its surface-specific placement.
    #[test]
    fn extraction_request_bodies_use_surface_specific_shapes() {
        let req = fixture_extraction_request();

        let chat = chat::extraction_body(
            &ModelId::new("gpt-5-chat-latest"),
            Some(ReasoningEffort::Medium),
            &req,
        )
        .expect("chat body builds");
        let chat = serde_json::to_value(&chat).expect("chat body serializes");
        assert_eq!(chat["messages"][0]["role"], "system");
        assert_eq!(chat["messages"][1]["role"], "user");
        assert_eq!(chat["response_format"]["type"], "json_schema");
        assert_eq!(
            chat["response_format"]["json_schema"]["name"],
            "GlossaryExtraction"
        );
        assert_eq!(chat["response_format"]["json_schema"]["strict"], true);
        assert_eq!(
            chat["response_format"]["json_schema"]["schema"]["properties"]["terms"]["maxItems"], 9,
            "the request's term cap must reach the provider schema"
        );
        assert_eq!(chat["reasoning_effort"], "medium");
        assert!(chat.get("reasoning").is_none());
        assert_eq!(chat["max_completion_tokens"], EXTRACTION_MAX_OUTPUT_TOKENS);

        let responses = responses::extraction_body(
            &ModelId::new("gpt-5.6-terra"),
            Some(ReasoningEffort::Medium),
            &req,
        )
        .expect("responses body builds");
        let responses = serde_json::to_value(&responses).expect("responses body serializes");
        assert_eq!(responses["input"][0]["role"], "developer");
        assert_eq!(responses["input"][1]["role"], "user");
        assert_eq!(responses["text"]["format"]["type"], "json_schema");
        assert_eq!(responses["text"]["format"]["name"], "GlossaryExtraction");
        assert_eq!(
            responses["text"]["format"]["schema"]["properties"]["terms"]["maxItems"],
            9
        );
        assert_eq!(responses["reasoning"]["effort"], "medium");
        assert!(responses.get("reasoning_effort").is_none());
        assert_eq!(responses["max_output_tokens"], EXTRACTION_MAX_OUTPUT_TOKENS);
        assert_eq!(
            responses["store"], false,
            "the extraction request carries a document excerpt and must opt out of retention"
        );

        // Unconfigured effort is omitted on both surfaces, as in translation.
        let chat_plain = chat::extraction_body(&ModelId::new("gpt-5-chat-latest"), None, &req)
            .expect("chat body builds");
        assert!(
            serde_json::to_value(chat_plain)
                .expect("serializes")
                .get("reasoning_effort")
                .is_none()
        );
        let responses_plain =
            responses::extraction_body(&ModelId::new("gpt-5.6-terra"), None, &req)
                .expect("responses body builds");
        assert!(
            serde_json::to_value(responses_plain)
                .expect("serializes")
                .get("reasoning")
                .is_none()
        );
    }

    /// OI-0008 / `R0001-0079` in the removed `reviews/reviewed/0001.md`:
    /// the [`SurfaceRequest`] wrapper carries *which*
    /// surface shaped a body and must add nothing to the wire — otherwise
    /// the per-surface body pins in `chat.rs`/`responses.rs` would stop
    /// describing what actually gets POSTed.
    #[test]
    fn surface_request_wrapper_is_transparent_on_the_wire() {
        let batch = fixture_batch();

        let chat = chat::translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("chat body builds");
        let bare = serde_json::to_string(&chat).expect("chat body serializes");
        let wrapped =
            serde_json::to_string(&SurfaceRequest::Chat(chat)).expect("wrapped body serializes");
        assert_eq!(wrapped, bare);

        let responses = responses::translation_body(&ModelId::new("o3-mini"), None, &batch)
            .expect("responses body builds");
        let bare = serde_json::to_string(&responses).expect("responses body serializes");
        let wrapped = serde_json::to_string(&SurfaceRequest::Responses(responses))
            .expect("wrapped body serializes");
        assert_eq!(wrapped, bare);
    }

    /// The endpoint a request goes to is decided by the surface that shaped
    /// it, not by a second dispatch at the call site.
    #[test]
    fn surface_request_path_follows_the_surface() {
        let batch = fixture_batch();
        let chat = SurfaceRequest::Chat(
            chat::translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
                .expect("chat body builds"),
        );
        assert_eq!(chat.path(), CHAT_COMPLETIONS_PATH);
        let responses = SurfaceRequest::Responses(
            responses::translation_body(&ModelId::new("o3-mini"), None, &batch)
                .expect("responses body builds"),
        );
        assert_eq!(responses.path(), RESPONSES_PATH);
    }
}
