//! Responses API — request shaping and envelope reading.
//!
//! The mirror of [`super::chat`]: same three entry points
//! ([`translation_body`], [`extraction_body`], [`extract_output`]), same
//! schema object, different wrapper field names and a different envelope
//! shape. `super`'s shared flow selects this module by
//! [`Api::Responses`](super::Api::Responses).
//!
//! The envelope shapes supported here:
//!
//! ```text
//!   { "output_text": "<json>" }                        // top-level shortcut
//!   { "output": [ { "type": "message",
//!                   "content": [ { "type": "output_text", "text": "<json>" } ]
//!                 } ] }                                // nested form
//! ```
//!
//! [`output_from_envelope`] reads the whole envelope before accepting any of
//! it — a refusal anywhere outranks text anywhere (`R0003-0006`) — and then
//! prefers the shortcut over the nested form so envelopes from either OpenAI
//! snapshot land cleanly.
//!
//! Split out of `client.rs` (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`).
//!
//! TRACE: SCN-12
//! TRACE: OI-0026
//! TRACE: OI-0029

use serde::{Deserialize, Serialize};
use transync::llm::{GlossaryExtractionRequest, TranslationBatch, TranslatorError, prompt};

use crate::error::{ProviderError, map_provider_error};
use crate::{ModelId, ReasoningEffort};

use super::classify::{NO_REFUSAL_DETAIL, truncate_diagnostic};
use super::{EXTRACTION_MAX_OUTPUT_TOKENS, translation_output_ceiling};

/// Shape one translation batch as a Responses-API request.
pub(super) fn translation_body(
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<Request, TranslatorError> {
    Ok(Request {
        model: model.0.clone(),
        input: vec![
            InputMessage {
                role: "developer",
                content: batch.profile.prompt_body.clone(),
            },
            InputMessage {
                role: "user",
                content: prompt::build_user_prompt(batch)?,
            },
        ],
        text: TextSpec {
            format: TextFormat {
                ty: "json_schema",
                name: prompt::SCHEMA_NAME,
                strict: true,
                schema: prompt::schema_object_for(Some(batch.units.len())),
            },
        },
        reasoning: reasoning_effort.map(|effort| Reasoning { effort }),
        // EXT-2026-07 P1-7 (OI-0019): profile `target_output_tokens` is
        // the enforced output ceiling on the Responses API. Omitted when
        // unset — and a zero is unset, which is why this reads the knob
        // through `translation_output_ceiling` rather than off the
        // profile. A ceiling too small to fit the batch surfaces as the
        // explicit `incomplete (reason: max_output_tokens)` error handled
        // in `output_from_envelope` (R0008-0036).
        max_output_tokens: translation_output_ceiling(batch),
        store: false,
    })
}

/// Shape the one-per-run candidate-glossary preflight as a Responses-API
/// request. OI-0026.
pub(super) fn extraction_body(
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    req: &GlossaryExtractionRequest,
) -> Result<Request, TranslatorError> {
    Ok(Request {
        model: model.0.clone(),
        input: vec![
            InputMessage {
                role: "developer",
                content: prompt::EXTRACTION_SYSTEM_PROMPT.to_string(),
            },
            InputMessage {
                role: "user",
                content: prompt::build_extraction_user_prompt(req)?,
            },
        ],
        text: TextSpec {
            format: TextFormat {
                ty: "json_schema",
                name: prompt::EXTRACTION_SCHEMA_NAME,
                strict: true,
                schema: prompt::extraction_schema_object(req.max_terms),
            },
        },
        reasoning: reasoning_effort.map(|effort| Reasoning { effort }),
        max_output_tokens: Some(EXTRACTION_MAX_OUTPUT_TOKENS),
        // Same no-retention posture as the translation path: the request
        // carries a whole-document excerpt.
        store: false,
    })
}

/// Read a Responses-API reply: deserialize the envelope, then pull the
/// model's JSON string out of it.
///
/// The only place the Responses envelope is parsed, so every use case
/// reports a malformed envelope, an `incomplete` status, and a refusal
/// identically.
pub(super) fn extract_output(bytes: &[u8]) -> Result<String, TranslatorError> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|e| {
        map_provider_error(ProviderError::Malformed(format!(
            "could not parse Responses-API envelope: {e}"
        )))
    })?;
    output_from_envelope(&envelope).map_err(map_provider_error)
}

/// Extract the model's JSON string from a Responses envelope. Reports a
/// non-success `status` and a refusal explicitly and concatenates every
/// `output_text` segment rather than returning only the first (R0008-0036).
///
/// **A refusal outranks text, wherever either arrives (`R0003-0006`,
/// `R0003-0019`).** The nested walk runs before *any* text is accepted — the
/// top-level `output_text` shortcut included — so an envelope carrying both
/// reports the refusal instead of translating what the model emitted beside
/// it. This is the Chat surface's rule (`R0002-0041`: "a refusal outranks
/// content, and that holds however the refusal arrives") applied to the
/// surface that was left asymmetric: extraction used to return the shortcut
/// before it had looked at the nested items at all, and to return the
/// concatenated nested text before it looked at the refusals it had just
/// collected. When the model both declines and emits something, the declining
/// is the honest report, and the something is by construction not a complete
/// answer.
///
/// The shortcut is still *preferred* over the nested text once nothing has
/// refused — that ordering is what makes both OpenAI snapshots land cleanly.
fn output_from_envelope(envelope: &Envelope) -> Result<String, ProviderError> {
    if let Some(status) = envelope.status.as_deref()
        && let Some(err) = non_success_status_error(status, envelope)
    {
        return Err(err);
    }
    // Walk the nested form first, concatenating every output_text segment and
    // collecting any refusal segments — the whole envelope is read before any
    // of it is accepted.
    let mut text = String::new();
    // The two are separate because a refusal segment carrying no usable string
    // is still a refusal: `saw_refusal` is the classification, `refusals` is
    // only the wording. Collapsing them into "did any wording arrive" is what
    // made an empty `refusal` extract as ordinary output on the Chat surface
    // (R0002-0041), and the same split is what keeps this surface honest.
    let mut saw_refusal = false;
    let mut refusals: Vec<&str> = Vec::new();
    for item in &envelope.output {
        if let OutputItem::Message { content } = item {
            for c in content {
                match c {
                    ContentItem::OutputText { text: t } => text.push_str(t),
                    ContentItem::Refusal { refusal } => {
                        saw_refusal = true;
                        if let Some(r) = refusal.as_deref().filter(|r| !r.is_empty()) {
                            refusals.push(r);
                        }
                    }
                    ContentItem::Other => {}
                }
            }
        }
    }
    if saw_refusal {
        // Empty strings were dropped above rather than joined, so a refusal
        // beside a detail-less one reads as "reason" and not "reason; " — and
        // when nothing had wording at all, the stand-in speaks instead of the
        // bare "model refused to translate: " this used to print (ti `594a7f`).
        let joined = refusals.join("; ");
        let detail = if joined.is_empty() {
            NO_REFUSAL_DETAIL
        } else {
            joined.as_str()
        };
        // R0008-0034's cap covers the refusal too (R0002-0039): a refusal
        // string is provider-controlled text on the same footing as the
        // `error` object, and it can be as large as the 32 MiB body cap
        // allows before it reaches stderr, the logs and the run's report
        // artifacts.
        return Err(ProviderError::ModelRefused(format!(
            "model refused to translate: {}",
            truncate_diagnostic(detail.as_bytes())
        )));
    }
    // Nothing refused: prefer the top-level shortcut when present and
    // non-empty, then the concatenated nested text.
    if let Some(t) = envelope.output_text.as_deref()
        && !t.is_empty()
    {
        return Ok(t.to_string());
    }
    if !text.is_empty() {
        return Ok(text);
    }
    // A status this transport does not recognize reached extraction and had
    // nothing to give: carry it (and any error object) into the diagnostic
    // instead of reporting a bare "no output_text" (R0001-0030). The status
    // is capped like every other provider-controlled string on this surface
    // (R0003-0016): the arm above names statuses this transport knows, but
    // this one prints whatever vocabulary a gateway invented, and unbounded
    // that is another 32 MiB path into stderr, the logs and the run's report.
    Err(ProviderError::Malformed(match envelope.status.as_deref() {
        Some(status) => format!(
            "Responses-API envelope had no output_text content (status: {}; {})",
            truncate_diagnostic(status.as_bytes()),
            envelope.error_detail()
        ),
        None => "Responses-API envelope had no output_text content".to_string(),
    }))
}

/// Map a Responses `status` that cannot carry a complete answer onto a
/// [`ProviderError`]; `None` means extraction may proceed.
///
/// R0001-0030: `output_from_envelope` used to branch on `incomplete` alone,
/// so a `failed` or `cancelled` response fell through to the generic "no
/// output_text" path and the provider's own status and error text were lost.
///
/// The variant chosen here **is** the retry decision (see
/// [`super::classify`]'s module doc): `Transport` reaches the pipeline as
/// `TranslatorError::Network` and is retried within
/// `max_per_batch_provider_retries`; anything else is terminal. The line
/// drawn is whether a **verbatim** resubmission (ADR-0009 — the retry
/// changes timing and nothing else) could plausibly come back different:
///
/// - `incomplete` — terminal. The ceiling that truncated the answer travels
///   in the request, so the same request truncates the same way; ADR-0017
///   keeps that a run-terminal condition. Which *terminal* variant it gets
///   is read off `incomplete_details.reason` (ti 1a85f3), because the two
///   documented reasons have opposite remediations: `max_output_tokens` is
///   an operator knob (`OutputCeilingExhausted`) and `content_filter` is a
///   policy stop nothing in the request moves (`ContentFiltered`). A reason
///   this adapter does not know — an absent one included — stays the
///   unclassified `Other` rather than borrowing a name that would send an
///   operator after the wrong knob. Today OpenAI delivers Responses filter
///   stops as `refusal` segments (§7), so the filter arm is the defensive
///   half of the mapping, for a gateway that uses the field instead.
/// - `failed` / `cancelled` — transient. The request was accepted and the
///   provider gave up on its own side; this is the envelope-level analogue
///   of a 5xx, and the bounded transport budget is exactly the instrument
///   for it. When the budget is spent the run still aborts (ADR-0017's
///   "exhausted transport retries"), now naming the provider's status.
/// - `queued` / `in_progress` — transient. A response object that has not
///   settled, which this transport never asks for (no streaming, no
///   background mode), so it is a provider anomaly, not a malformed body.
/// - `completed`, and any status this transport does not know, go to
///   extraction — an OpenAI-compatible gateway with its own status
///   vocabulary keeps working, and the fall-through diagnostic names the
///   status if there was nothing to extract.
fn non_success_status_error(status: &str, envelope: &Envelope) -> Option<ProviderError> {
    match status {
        "incomplete" => {
            let reason = envelope
                .incomplete_details
                .as_ref()
                .and_then(|d| d.reason.as_deref())
                .unwrap_or("unspecified");
            // The reason is provider-controlled text like the `error` object,
            // so it is capped at the same 512-byte excerpt before it reaches
            // stderr, the logs and the run's report artifacts (R0003-0015).
            // The *classification* below still reads the raw reason: the two
            // names it recognizes are far shorter than the cap, and matching
            // on the excerpt would make a diagnostic detail decide a variant.
            let detail = format!(
                "Responses output incomplete (reason: {})",
                truncate_diagnostic(reason.as_bytes())
            );
            Some(match reason {
                "max_output_tokens" => ProviderError::OutputCeilingExhausted(detail),
                "content_filter" => ProviderError::ContentFiltered(detail),
                _ => ProviderError::Other(detail),
            })
        }
        "failed" | "cancelled" | "queued" | "in_progress" => Some(ProviderError::Transport(
            format!("Responses status {status}: {}", envelope.error_detail()),
        )),
        _ => None,
    }
}

#[derive(Serialize)]
pub(super) struct Request {
    model: String,
    input: Vec<InputMessage>,
    text: TextSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: Option<Reasoning>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    /// The Responses API defaults to `store: true` (server-side
    /// retention, retrievable via `GET /v1/responses/{id}` for ~30
    /// days), unlike Chat Completions which defaults to `false`.
    /// Send `store: false` explicitly so both dispatch surfaces keep
    /// the same no-retention posture for document content.
    store: bool,
}

#[derive(Serialize)]
struct Reasoning {
    effort: ReasoningEffort,
}

#[derive(Serialize)]
struct InputMessage {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct TextSpec {
    format: TextFormat,
}

#[derive(Serialize)]
struct TextFormat {
    #[serde(rename = "type")]
    ty: &'static str,
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    output: Vec<OutputItem>,
    #[serde(default)]
    output_text: Option<String>,
    /// Status of the response object (`completed`, `incomplete`, `failed`,
    /// `cancelled`, `queued`, `in_progress`). R0008-0036 surfaced
    /// `incomplete` explicitly instead of degrading into a generic
    /// invalid-JSON error downstream; R0001-0030 extended that to every
    /// other status that cannot carry an answer — see
    /// [`non_success_status_error`], which owns both the wording and the
    /// retry decision.
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    incomplete_details: Option<IncompleteDetails>,
    /// The provider's error object, populated on a `failed` response.
    /// R0001-0030: modeled so the failure carries the provider's own code
    /// and message instead of an empty diagnostic.
    #[serde(default)]
    error: Option<ApiError>,
}

impl Envelope {
    /// The provider's own words for a non-success response, capped like
    /// every other provider-controlled diagnostic (R0008-0034), or a fixed
    /// stand-in when the envelope carried no usable `error` object.
    ///
    /// The cap is applied to the assembled detail, so it covers `code` and
    /// `message` alike: both are provider-controlled text that reaches
    /// stderr and logs, and capping only one of them would leave the other
    /// half of the same string unbounded.
    fn error_detail(&self) -> String {
        let Some(error) = self.error.as_ref() else {
            return NO_ERROR_DETAIL.to_string();
        };
        let detail = match (error.code.as_deref(), error.message.as_deref()) {
            (Some(code), Some(message)) => format!("{code}: {message}"),
            (Some(code), None) => code.to_string(),
            (None, Some(message)) => message.to_string(),
            (None, None) => return NO_ERROR_DETAIL.to_string(),
        };
        truncate_diagnostic(detail.as_bytes())
    }
}

/// What an error-less non-success envelope reports in place of a reason.
const NO_ERROR_DETAIL: &str = "no error detail";

#[derive(Deserialize)]
struct IncompleteDetails {
    #[serde(default)]
    reason: Option<String>,
}

/// The Responses `error` object. Both fields are optional: gateways vary,
/// and a missing one degrades the diagnostic rather than the parse.
#[derive(Deserialize)]
struct ApiError {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OutputItem {
    Message {
        #[serde(default)]
        content: Vec<ContentItem>,
    },
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ContentItem {
    OutputText {
        text: String,
    },
    Refusal {
        /// Optional, and defaulted, because the **segment** is the refusal —
        /// its wording is a detail. A `{"type": "refusal"}` with no string was
        /// a hard `serde` failure while this was required, so an envelope that
        /// plainly said the model declined came back as "could not parse
        /// Responses-API envelope": a misclassification, not just a thin
        /// diagnostic. Chat's refusal part has been `Option` for the same
        /// reason; see [`NO_REFUSAL_DETAIL`] for what fills the gap.
        #[serde(default)]
        refusal: Option<String>,
    },
    #[serde(other)]
    Other,
}

// OI-0009: unit tests for the pure request/response plumbing. These run
// under plain `cargo test --workspace` — no API key, no network.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::fixtures::fixture_batch;

    /// R0007-0010 pin: every Responses request must send `store: false`
    /// so document content is not retained server-side.
    #[test]
    fn responses_request_body_shape_and_store_false() {
        let batch = fixture_batch();
        let body = translation_body(&ModelId::new("o3-mini"), None, &batch).expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert_eq!(v["model"], "o3-mini");
        assert_eq!(
            v["store"], false,
            "Responses requests must opt out of retention"
        );
        assert_eq!(v["input"][0]["role"], "developer");
        assert_eq!(v["input"][1]["role"], "user");
        assert_eq!(v["text"]["format"]["type"], "json_schema");
    }

    /// EXT-2026-07 P1-7 (OI-0019): the profile's `target_output_tokens`
    /// rides out as `max_output_tokens` on the Responses API.
    #[test]
    fn responses_request_body_sets_max_output_tokens() {
        let batch = fixture_batch();
        assert_eq!(batch.profile.batching.target_output_tokens, Some(8000));
        let body = translation_body(&ModelId::new("o3-mini"), None, &batch).expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert_eq!(v["max_output_tokens"], 8000);
    }

    /// EXT-2026-07 P1-7: no ceiling set → the field is omitted entirely.
    #[test]
    fn responses_request_body_omits_max_output_tokens_when_none() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = None;
        let body = translation_body(&ModelId::new("o3-mini"), None, &batch).expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert!(
            v.get("max_output_tokens").is_none(),
            "unset ceiling must omit the field, got {v}"
        );
    }

    /// `R0004-0016`, the OpenAI sibling: a zero ceiling names no budget, so
    /// it takes the same road an absent one does — the field is omitted —
    /// rather than riding out as `max_output_tokens: 0`, which leaves the
    /// model nothing to answer in.
    ///
    /// The twin of the Chat-Completions pin: the knob is one knob, and a
    /// rule about what it means must not hold on one surface only.
    #[test]
    fn responses_request_body_omits_max_output_tokens_when_zero() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = Some(0);
        let body = translation_body(&ModelId::new("o3-mini"), None, &batch).expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert!(
            v.get("max_output_tokens").is_none(),
            "a zero ceiling must be normalized to absent, never sent, got {v}"
        );
    }

    /// R0008-0036: multiple output_text segments are concatenated in order.
    #[test]
    fn responses_output_concatenates_segments() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output": [{ "type": "message", "content": [
                { "type": "output_text", "text": "{\"a\":" },
                { "type": "output_text", "text": "1}" }
            ] }]
        }))
        .expect("envelope parses");
        assert_eq!(output_from_envelope(&env).expect("extracts"), "{\"a\":1}");
    }

    /// R0008-0036: an `incomplete` status surfaces the reason explicitly.
    #[test]
    fn responses_incomplete_is_error() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "incomplete",
            "incomplete_details": { "reason": "max_output_tokens" }
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("incomplete is an error");
        assert!(
            matches!(err, ProviderError::OutputCeilingExhausted(ref s) if s.contains("max_output_tokens")),
            "got {err:?}"
        );
    }

    /// ti 1a85f3: `incomplete` is one status with several causes, and the two
    /// documented ones have *opposite* remediations — raise the ceiling, or
    /// nothing the request can do. Each therefore gets its own variant and
    /// its own stable code; a reason this adapter does not know stays the
    /// unclassified catch-all rather than borrowing a name and sending an
    /// operator after the wrong knob.
    #[test]
    fn responses_incomplete_names_its_cause_from_the_reason() {
        let cases = [
            (
                Some("max_output_tokens"),
                "provider_output_ceiling_exhausted",
            ),
            (Some("content_filter"), "provider_content_filtered"),
            (Some("something_new"), "provider_error"),
            (None, "provider_error"),
        ];
        for (reason, expected) in cases {
            let mut body = serde_json::json!({ "status": "incomplete" });
            if let Some(reason) = reason {
                body["incomplete_details"] = serde_json::json!({ "reason": reason });
            }
            let env: Envelope = serde_json::from_value(body).expect("envelope parses");
            let mapped = map_provider_error(output_from_envelope(&env).expect_err("incomplete"));
            assert_eq!(
                mapped.stable_code(),
                expected,
                "reason {reason:?} must classify as {expected}, got {mapped:?}"
            );
            assert!(
                mapped.to_string().contains(reason.unwrap_or("unspecified")),
                "the reason must survive into the message, got {mapped:?}"
            );
        }
    }

    /// R0001-0030: a `failed` response reports its status and the provider's
    /// own error code and message, instead of falling through to the generic
    /// "no output_text" path.
    #[test]
    fn responses_failed_status_reports_the_provider_error() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "failed",
            "error": { "code": "server_error", "message": "the model failed to generate" }
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("a failed response is an error");
        match &err {
            ProviderError::Transport(s) => {
                assert!(s.contains("failed"), "must name the status, got {s:?}");
                assert!(
                    s.contains("server_error") && s.contains("the model failed to generate"),
                    "must carry the provider's error, got {s:?}"
                );
            }
            other => panic!("expected a transient Transport error, got {other:?}"),
        }
    }

    /// R0001-0030: `cancelled` is reported too, and an absent `error` object
    /// degrades the detail rather than the diagnostic.
    #[test]
    fn responses_cancelled_status_reports_without_an_error_object() {
        let env: Envelope = serde_json::from_value(serde_json::json!({ "status": "cancelled" }))
            .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("a cancelled response is an error");
        assert!(
            matches!(&err, ProviderError::Transport(s)
                if s.contains("cancelled") && s.contains("no error detail")),
            "got {err:?}"
        );
    }

    /// R0008-0034's 512-byte diagnostic cap covers the whole
    /// provider-controlled detail: `code` is as much the provider's text as
    /// `message` is, so a hostile or misbehaving gateway cannot flood stderr
    /// and logs through the field that is not the message.
    #[test]
    fn responses_error_detail_caps_the_code_as_well_as_the_message() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "failed",
            "error": { "code": "c".repeat(600), "message": "m".repeat(600) }
        }))
        .expect("envelope parses");
        let detail = env.error_detail();
        assert!(
            detail.len() < 600 && detail.contains("truncated"),
            "the assembled detail must be capped, got {} bytes",
            detail.len()
        );
        assert!(
            !detail.contains('m'),
            "the cap must bite inside the code, before the message, got {detail:?}"
        );

        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "failed",
            "error": { "code": "c".repeat(600) }
        }))
        .expect("envelope parses");
        let detail = env.error_detail();
        assert!(
            detail.len() < 600 && detail.contains("truncated"),
            "a code with no message is capped too, got {} bytes",
            detail.len()
        );
    }

    /// R0003-0015 / R0003-0016: the last two provider-controlled strings on
    /// this surface that reached an error uncapped — `incomplete_details.
    /// reason` and an unrecognized `status` — go through the same 512-byte
    /// excerpt as the `error` object and the refusal beside them. Both are
    /// gateway-supplied, and both end up in stderr, the logs and the run's
    /// report artifacts.
    #[test]
    fn responses_incomplete_reason_and_unknown_status_are_capped() {
        let huge = "z".repeat(200_000);

        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "incomplete",
            "incomplete_details": { "reason": huge.clone() }
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("incomplete is an error");
        let ProviderError::Other(reason_detail) = &err else {
            panic!("an unknown reason stays the unclassified cause, got {err:?}");
        };
        assert!(
            reason_detail.len() < 1024,
            "the reason must be capped, got {} bytes",
            reason_detail.len()
        );
        assert!(
            reason_detail.contains("incomplete") && reason_detail.contains("truncated"),
            "got {reason_detail:?}"
        );

        let env: Envelope =
            serde_json::from_value(serde_json::json!({ "status": huge })).expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("no output is still an error");
        let ProviderError::Malformed(status_detail) = &err else {
            panic!("an unknown status with nothing to extract is malformed, got {err:?}");
        };
        assert!(
            status_detail.len() < 1024,
            "the status must be capped, got {} bytes",
            status_detail.len()
        );
        assert!(
            status_detail.contains("no output_text content") && status_detail.contains("truncated"),
            "got {status_detail:?}"
        );
    }

    /// R0001-0030: the status table *is* the retry decision. `failed`,
    /// `cancelled` and the two unsettled statuses reach the pipeline as
    /// retryable `Network`; `incomplete` stays terminal, because the ceiling
    /// that truncated the answer rides in the request and a verbatim
    /// resubmission would truncate identically.
    #[test]
    fn responses_status_classification_matches_the_pipeline_retry_surface() {
        for status in ["failed", "cancelled", "queued", "in_progress"] {
            let env: Envelope = serde_json::from_value(serde_json::json!({ "status": status }))
                .expect("envelope parses");
            let err = output_from_envelope(&env).expect_err("non-success status");
            let mapped = map_provider_error(err);
            assert!(
                matches!(mapped, TranslatorError::Network(_)),
                "{status} must reach the pipeline as retryable Network, got {mapped:?}"
            );
        }
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "incomplete",
            "incomplete_details": { "reason": "max_output_tokens" }
        }))
        .expect("envelope parses");
        let mapped = map_provider_error(output_from_envelope(&env).expect_err("incomplete"));
        assert!(
            matches!(mapped, TranslatorError::OutputCeilingExhausted(_)),
            "incomplete must stay terminal, got {mapped:?}"
        );
    }

    /// A status this transport does not know still goes to extraction, so an
    /// OpenAI-compatible gateway with its own vocabulary keeps working — and
    /// when there is nothing to extract, the diagnostic names that status.
    #[test]
    fn responses_unknown_status_extracts_and_is_named_when_empty() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "succeeded",
            "output_text": "{\"a\":1}"
        }))
        .expect("envelope parses");
        assert_eq!(output_from_envelope(&env).expect("extracts"), "{\"a\":1}");

        let env: Envelope = serde_json::from_value(serde_json::json!({
            "status": "succeeded",
            "error": { "message": "nothing to say" }
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("no output is still an error");
        assert!(
            matches!(&err, ProviderError::Malformed(s)
                if s.contains("succeeded") && s.contains("nothing to say")),
            "got {err:?}"
        );
    }

    /// R0008-0036: a refusal content segment surfaces explicitly.
    #[test]
    fn responses_refusal_is_error() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output": [{ "type": "message", "content": [
                { "type": "refusal", "refusal": "nope" }
            ] }]
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("refusal is an error");
        assert!(matches!(err, ProviderError::ModelRefused(_)), "got {err:?}");
        assert_eq!(
            map_provider_error(err).stable_code(),
            "provider_model_refused",
            "ti 1a85f3: a refusal is its own cause on both surfaces"
        );
    }

    /// R0003-0006 / R0003-0019, pinned by R0003-0087: a refusal outranks text
    /// in the same envelope, whichever way the text arrives. Both shapes the
    /// old order let through: the top-level `output_text` shortcut, which
    /// returned before the nested items were looked at at all, and nested
    /// `output_text` segments beside a nested refusal, which were returned
    /// before the refusals just collected were checked. The Chat surface has
    /// answered this way since R0002-0041; this is the same rule on the
    /// surface that was left asymmetric.
    #[test]
    fn responses_refusal_outranks_text_in_the_same_envelope() {
        let refusal_content = serde_json::json!([
            { "type": "refusal", "refusal": "I can't help with that." }
        ]);
        for (shape, body) in [
            (
                "top-level shortcut beside a nested refusal",
                serde_json::json!({
                    "output_text": "{\"batch_id\":\"b1\"}",
                    "output": [{ "type": "message", "content": refusal_content }]
                }),
            ),
            (
                "nested text beside a nested refusal, same message",
                serde_json::json!({
                    "output": [{ "type": "message", "content": [
                        { "type": "output_text", "text": "{\"batch_id\":\"b1\"}" },
                        { "type": "refusal", "refusal": "I can't help with that." }
                    ] }]
                }),
            ),
            (
                "nested text and a refusal in separate messages",
                serde_json::json!({
                    "output": [
                        { "type": "message", "content": [
                            { "type": "output_text", "text": "{\"batch_id\":\"b1\"}" }
                        ] },
                        { "type": "message", "content": refusal_content }
                    ]
                }),
            ),
            (
                "everything at once, on a completed status",
                serde_json::json!({
                    "status": "completed",
                    "output_text": "{\"batch_id\":\"b1\"}",
                    "output": [
                        { "type": "message", "content": [
                            { "type": "output_text", "text": "{\"batch_id\":\"b1\"}" },
                            { "type": "refusal", "refusal": "I can't help with that." }
                        ] }
                    ]
                }),
            ),
        ] {
            let env: Envelope = serde_json::from_value(body).expect("envelope parses");
            let Err(err) = output_from_envelope(&env) else {
                panic!("{shape}: text beside a refusal must not extract as output");
            };
            assert!(
                matches!(&err, ProviderError::ModelRefused(s) if s.contains("refused to translate")),
                "{shape}: got {err:?}"
            );
            assert_eq!(
                map_provider_error(err).stable_code(),
                "provider_model_refused",
                "{shape}: a refusal keeps its own cause"
            );
        }
    }

    /// The other half of the same ordering: with nothing refusing, the
    /// top-level shortcut still wins over the nested text, and nested text is
    /// still read when the shortcut is absent or empty.
    #[test]
    fn responses_shortcut_still_precedes_nested_text_without_a_refusal() {
        let nested = serde_json::json!([{ "type": "message", "content": [
            { "type": "output_text", "text": "nested" }
        ] }]);
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output_text": "shortcut",
            "output": nested
        }))
        .expect("envelope parses");
        assert_eq!(output_from_envelope(&env).expect("extracts"), "shortcut");

        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output_text": "",
            "output": nested
        }))
        .expect("envelope parses");
        assert_eq!(output_from_envelope(&env).expect("extracts"), "nested");
    }

    /// ti `594a7f`: a refusal segment carrying no usable wording still reports
    /// the refusal *and* names the stand-in, exactly as the Chat surface has
    /// since R0002-0041. Three shapes had no wording to print, and each used
    /// to read differently:
    ///
    /// - `{"type": "refusal"}` — the field absent — did not reach the refusal
    ///   diagnosis at all; it failed `serde` and came back as a malformed
    ///   envelope, blaming the shape of a response that plainly said the model
    ///   declined;
    /// - `{"refusal": ""}` printed `model refused to translate: ` and stopped;
    /// - two empty ones printed the separator, `model refused to translate: ;`.
    ///
    /// The fourth case is the one that must NOT change: an empty segment
    /// beside a real one contributes nothing, not a dangling separator.
    #[test]
    fn responses_refusal_without_detail_names_the_stand_in() {
        for (shape, content) in [
            ("field absent", serde_json::json!([{ "type": "refusal" }])),
            (
                "empty string",
                serde_json::json!([{ "type": "refusal", "refusal": "" }]),
            ),
            (
                "two empty segments",
                serde_json::json!([
                    { "type": "refusal", "refusal": "" },
                    { "type": "refusal", "refusal": "" }
                ]),
            ),
            (
                "empty segment beside text",
                serde_json::json!([
                    { "type": "output_text", "text": "{\"batch_id\":\"b1\"}" },
                    { "type": "refusal", "refusal": "" }
                ]),
            ),
        ] {
            let env: Envelope = serde_json::from_value(serde_json::json!({
                "output": [{ "type": "message", "content": content }]
            }))
            .expect("envelope parses");
            let err = output_from_envelope(&env).expect_err("a refusal is an error");
            let ProviderError::ModelRefused(s) = &err else {
                panic!(
                    "{shape}: a detail-less refusal must still earn the refusal diagnosis, got {err:?}"
                );
            };
            assert_eq!(
                s,
                &format!("model refused to translate: {NO_REFUSAL_DETAIL}"),
                "{shape}: the stand-in speaks in place of the missing wording"
            );
            assert_eq!(
                map_provider_error(err).stable_code(),
                "provider_model_refused",
                "{shape}: a refusal keeps its own cause"
            );
        }

        // The wording that IS there is still the whole message: a detail-less
        // segment beside a real one adds no separator of its own.
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output": [{ "type": "message", "content": [
                { "type": "refusal", "refusal": "I can't help with that." },
                { "type": "refusal" },
                { "type": "refusal", "refusal": "" }
            ] }]
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("a refusal is an error");
        let ProviderError::ModelRefused(s) = &err else {
            panic!("expected a refusal error, got {err:?}");
        };
        assert_eq!(s, "model refused to translate: I can't help with that.");
    }

    /// R0002-0039: a refusal is provider-controlled text like the `error`
    /// object beside it, so it is capped at the same 512-byte diagnostic
    /// excerpt instead of expanding to whatever the 32 MiB body cap allowed.
    #[test]
    fn responses_refusal_diagnostic_is_capped() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "output": [{ "type": "message", "content": [
                { "type": "refusal", "refusal": "r".repeat(200_000) },
                { "type": "refusal", "refusal": "s".repeat(200_000) }
            ] }]
        }))
        .expect("envelope parses");
        let err = output_from_envelope(&env).expect_err("refusal is an error");
        match &err {
            ProviderError::ModelRefused(s) => {
                assert!(
                    s.len() < 1024,
                    "the refusal must be capped, got {} bytes",
                    s.len()
                );
                assert!(s.contains("refused to translate"), "got {s:?}");
                assert!(
                    s.contains("truncated"),
                    "the cut must be announced, got {s:?}"
                );
            }
            other => panic!("expected a refusal error, got {other:?}"),
        }
    }
}
