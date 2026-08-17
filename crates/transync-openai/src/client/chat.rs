//! Chat Completions — request shaping and envelope reading.
//!
//! Everything this surface needs for *both* use cases lives here: the wire
//! DTOs, [`translation_body`] and [`extraction_body`] (which build the same
//! [`Request`] and differ only in prompts, schema and output ceiling), and
//! [`extract_output`], the single reader of a Chat reply. `super`'s shared
//! flow selects this module by [`Api::ChatCompletions`](super::Api::ChatCompletions)
//! and reaches past those three functions for nothing.
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

/// Shape one translation batch as a Chat-Completions request.
pub(super) fn translation_body(
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    batch: &TranslationBatch,
) -> Result<Request, TranslatorError> {
    Ok(Request {
        model: model.0.clone(),
        messages: vec![
            Message {
                role: "system",
                content: batch.profile.prompt_body.clone(),
            },
            Message {
                role: "user",
                content: prompt::build_user_prompt(batch)?,
            },
        ],
        response_format: ResponseFormat {
            ty: "json_schema",
            json_schema: schema_descriptor(Some(batch.units.len())),
        },
        reasoning_effort,
        // EXT-2026-07 P1-7 (OI-0019): profile `target_output_tokens` is
        // the enforced output ceiling on Chat Completions. Omitted when
        // unset so the provider applies its own default — and a zero is
        // unset, which is why this reads the knob through
        // `translation_output_ceiling` rather than off the profile. A
        // ceiling too small to fit the batch comes back as
        // `finish_reason: "length"` and surfaces as the explicit
        // exhausted-ceiling error handled in `output_from_envelope`
        // (ticket 3c9741bf).
        max_completion_tokens: translation_output_ceiling(batch),
    })
}

/// Shape the one-per-run candidate-glossary preflight as a Chat-Completions
/// request. OI-0026.
pub(super) fn extraction_body(
    model: &ModelId,
    reasoning_effort: Option<ReasoningEffort>,
    req: &GlossaryExtractionRequest,
) -> Result<Request, TranslatorError> {
    Ok(Request {
        model: model.0.clone(),
        messages: vec![
            Message {
                role: "system",
                content: prompt::EXTRACTION_SYSTEM_PROMPT.to_string(),
            },
            Message {
                role: "user",
                content: prompt::build_extraction_user_prompt(req)?,
            },
        ],
        response_format: ResponseFormat {
            ty: "json_schema",
            json_schema: extraction_schema_descriptor(req.max_terms),
        },
        reasoning_effort,
        max_completion_tokens: Some(EXTRACTION_MAX_OUTPUT_TOKENS),
    })
}

/// Read a Chat-Completions reply: deserialize the envelope, then pull the
/// model's JSON string out of it.
///
/// The only place the Chat envelope is parsed, so every use case reports a
/// malformed envelope, an exhausted output ceiling, a provider content-policy
/// stop, a refusal, and a missing body identically.
pub(super) fn extract_output(bytes: &[u8]) -> Result<String, TranslatorError> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|e| {
        map_provider_error(ProviderError::Malformed(format!(
            "could not parse Chat-Completions envelope: {e}"
        )))
    })?;
    output_from_envelope(envelope).map_err(map_provider_error)
}

/// Extract the model's JSON string from a Chat-Completions envelope,
/// surfacing an exhausted output ceiling (ticket 3c9741bf), a provider
/// content-policy stop (ticket 0583a75a) and an explicit refusal (R0008-0035)
/// instead of a generic malformed-envelope error, and accepting content-part
/// arrays.
///
/// `finish_reason` is read **before** the message, mirroring the Responses
/// surface, where `status` decides whether extraction happens at all. A reply
/// the ceiling cut off — or the filter cut off — has nothing complete to give,
/// including a refusal string that was itself truncated, so the stop reason is
/// the honest diagnosis even when `message` carries something.
///
/// A refusal outranks content, and that holds however the refusal arrives:
/// `message.refusal` first, then a `refusal`-typed part inside a content array
/// (R0002-0041), which used to be dropped on the floor by the part filter —
/// text beside it was returned as ordinary output, and a refusal-only array
/// degraded into the generic missing-content error.
fn output_from_envelope(envelope: Envelope) -> Result<String, ProviderError> {
    let choice = envelope.choices.into_iter().next().ok_or_else(|| {
        ProviderError::Malformed("Chat-Completions response had no choices[0]".to_string())
    })?;
    if let Some(err) = finish_reason_error(choice.finish_reason.as_deref()) {
        return Err(err);
    }
    let message = choice.message;
    if let Some(refusal) = message.refusal.filter(|r| !r.is_empty()) {
        return Err(refusal_error(&refusal));
    }
    match message.content.map(Content::into_outcome) {
        Some(ContentOutcome::Refused(detail)) => Err(refusal_error(&detail)),
        Some(ContentOutcome::Text(text)) => Ok(text),
        Some(ContentOutcome::Empty) | None => Err(ProviderError::Malformed(
            "Chat-Completions response had no choices[0].message.content".to_string(),
        )),
    }
}

/// The one refusal diagnosis this surface reports, whichever field carried
/// the refusal.
///
/// `detail` is provider-controlled text on the same footing as an error body,
/// so it is capped at the same 512-byte diagnostic excerpt (R0008-0034 /
/// R0002-0039): unbounded, a hostile or misbehaving gateway could expand one
/// refusal to whatever the 32 MiB response cap allows and push all of it into
/// stderr, the logs, and the run's report artifacts.
fn refusal_error(detail: &str) -> ProviderError {
    ProviderError::ModelRefused(format!(
        "model refused to translate: {}",
        truncate_diagnostic(detail.as_bytes())
    ))
}

/// Map a Chat `finish_reason` that cannot carry a complete answer onto a
/// [`ProviderError`]; `None` means extraction may proceed.
///
/// The Responses surface has named an exhausted output ceiling explicitly
/// since R0008-0036 (`incomplete (reason: max_output_tokens)`). Chat
/// Completions read no `finish_reason` at all, so the same overshoot arrived
/// as a JSON body cut off mid-object and was diagnosed downstream by
/// `prompt::parse_batch_output` as `MalformedResponse` — the operator was told
/// the provider returned malformed JSON when in fact their own
/// `[batching].target_output_tokens` was too small, and the fix was not
/// discoverable from the message. The run was never *unsafe* (the truncated
/// attempt is rejected, so nothing invalid is cached or emitted, which is what
/// contracts.md §1 *Output ceiling and cache identity* relies on); only the
/// diagnosis was wrong.
///
/// Like `non_success_status_error` on the Responses surface, the variant
/// chosen here **is** the retry decision: neither variant is one the
/// pipeline re-dispatches, so both are terminal — which is what that
/// function already does for `incomplete`, the identical condition on the
/// other surface, terminal for the identical reason. The ceiling travels in
/// the request, so the verbatim resubmission ADR-0009 prescribes would
/// truncate the same way, and ADR-0017 keeps that run-terminal.
///
/// The variant chosen here is also the **taxonomy** decision (ti 1a85f3):
/// each reason gets its own `ProviderError` variant, so the two arrive
/// downstream as `TranslatorError::OutputCeilingExhausted` and
/// `TranslatorError::ContentFiltered` with distinct `stable_code()`s rather
/// than as two strings inside one `Other`. That matters because the
/// remediations are opposite — one names an operator knob, the other has
/// none — and a consumer used to have to string-match the constants below
/// to tell them apart.
///
/// `content_filter` (ticket 0583a75a) is the second reason acted on, and for
/// the same kind of argument. It is the provider stating that its own filter
/// ended generation; what comes back is a `message.content` that is absent,
/// null, or cut off where the filter fired, so before this it landed either on
/// [`output_from_envelope`]'s missing-body `Malformed` or on
/// `prompt::parse_batch_output`'s
/// `MalformedResponse` — both blaming the shape of the response for what the
/// provider had already labelled a policy stop. This is **not** the
/// `message.refusal` condition (R0008-0035): a refusal is the *model*
/// declining, arrives with `finish_reason: "stop"` and carries a `refusal`
/// string; a filter stop carries none. The classification is terminal on the
/// ADR-0009 argument again, in its strongest form: the retry
/// is a *verbatim* resubmission of identical content, and identical content is
/// what the filter acted on, so the same filter would fire on every attempt.
/// Retrying would spend the per-batch budget to arrive at the same stop, and
/// ADR-0017 makes the exhausted budget abort the run anyway — only later, and
/// under a message about attempts rather than about policy. If a future
/// provider ever made this outcome content-independent (say, a filter with a
/// nondeterministic threshold), that argument is the thing to revisit.
///
/// Every other stop reason — `stop`, `tool_calls`, an absent field, and any
/// vocabulary an OpenAI-compatible gateway invents — goes to extraction
/// unchanged, so a reply that does carry a usable body is never rejected on the
/// strength of a label this transport does not model. `tool_calls` and
/// `function_call` stay in that group deliberately: this adapter sends no
/// tools, so a request it shaped cannot elicit them.
///
/// TRACE: contracts.md §7
fn finish_reason_error(finish_reason: Option<&str>) -> Option<ProviderError> {
    match finish_reason {
        Some("length") => Some(ProviderError::OutputCeilingExhausted(
            CEILING_EXHAUSTED.to_string(),
        )),
        Some("content_filter") => {
            Some(ProviderError::ContentFiltered(CONTENT_FILTERED.to_string()))
        }
        _ => None,
    }
}

/// What an output-ceiling overshoot reports. Names the request parameter the
/// provider enforced *and* the profile key that set it, so the remediation is
/// readable off the error itself.
///
/// The remediation clause is scoped to the translation batch on purpose.
/// [`extract_output`] is the single reader for **both** bodies this module
/// builds, and only [`translation_body`] takes its ceiling from
/// `[batching].target_output_tokens`: [`extraction_body`] pins the
/// one-per-run glossary preflight at the fixed `EXTRACTION_MAX_OUTPUT_TOKENS`,
/// which that knob does not move. An unscoped "raise
/// `target_output_tokens`" would therefore be sound advice on a batch and a
/// misdirect on the preflight — where it is reachable despite the small
/// candidate list, because `reasoning_effort` rides on that request too and
/// reasoning tokens are billed against the same ceiling. The stop reason and
/// the enforced parameter are true on both paths, so only the fix is
/// qualified.
const CEILING_EXHAUSTED: &str = "Chat-Completions output incomplete (finish_reason: length): the \
     max_completion_tokens output ceiling was exhausted before the answer was complete — on a \
     translation batch, raise [batching].target_output_tokens (CLI --target-output-tokens)";

/// What a provider content-policy stop reports. Names the stop reason, says
/// whose decision ended the generation, and denies the two readings the old
/// diagnosis invited — a transport fault or a schema mismatch — because those
/// are exactly what the absent or half-written body used to be reported as.
///
/// It carries no remediation clause, unlike [`CEILING_EXHAUSTED`]. Nothing in
/// the request moves this outcome: no parameter this adapter sends turns the
/// filter off, and the batch is not resubmittable in a different shape (the
/// units are verbatim source, and ADR-0009 keeps the retry verbatim too). The
/// one thing that decided it is the content, so the content is what the
/// message points at — a claim that stays true whether the filter fired on the
/// source going out or on the translation coming back, which the envelope does
/// not distinguish.
const CONTENT_FILTERED: &str = "Chat-Completions generation stopped by the provider's content \
     filter (finish_reason: content_filter): the reply was cut short by a content policy, not by \
     a transport fault or a schema mismatch — the provider flagged this batch's content, so the \
     answer it returned is unusable by the provider's own decision";

#[derive(Serialize)]
pub(super) struct Request {
    model: String,
    messages: Vec<Message>,
    response_format: ResponseFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    ty: &'static str,
    json_schema: SchemaDescriptor,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
    /// Why the model stopped (`stop`, `length`, `content_filter`,
    /// `tool_calls`, …). Two of them are acted on: `length` is the Chat
    /// surface's report that the output ceiling was exhausted, the
    /// counterpart of the Responses envelope's
    /// `incomplete (reason: max_output_tokens)`; `content_filter` is the
    /// provider's filter ending generation, which has no Responses
    /// counterpart here because that surface delivers its filter stops as
    /// `refusal` content segments. Before ticket 3c9741bf this field was not
    /// read at all, so the overshoot reached `parse_batch_output` as a body
    /// cut off mid-JSON, and until ticket 0583a75a a filter stop reached it
    /// the same way — or died on the missing body. See
    /// [`finish_reason_error`], which owns both the wording and the retry
    /// decision.
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<Content>,
    #[serde(default)]
    refusal: Option<String>,
}

/// Chat `message.content` is usually a plain string, but compatible
/// endpoints may return an array of typed content parts. Accept both so a
/// content-part array is not misclassified as a malformed envelope
/// (R0008-0035).
#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Deserialize)]
struct ContentPart {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    text: Option<String>,
    /// R0002-0041: a `refusal`-typed part carries its string here, the way
    /// the Responses surface's `ContentItem::Refusal` does. Modeled so a
    /// refusal inside a content array is read rather than dropped by the
    /// text filter — `message.refusal` is the shape the real Chat
    /// Completions API uses, but a compatible gateway can put the refusal in
    /// the parts array instead, and the diagnosis should not depend on which.
    #[serde(default)]
    refusal: Option<String>,
}

/// What a `message.content` amounts to once flattened.
///
/// Three outcomes, not two, because a content array can carry a refusal —
/// which is neither usable text nor an absent body, and which must reach the
/// same diagnosis `message.refusal` gets.
enum ContentOutcome {
    Text(String),
    Refused(String),
    Empty,
}

impl Content {
    /// Flatten to the concatenated textual payload, a refusal, or nothing.
    ///
    /// A refusal part outranks text parts beside it, mirroring the sibling
    /// `message.refusal` field on this same surface, which is read before
    /// `message.content` for the same reason: when the model both declines
    /// and emits something, the declining is the honest report, and the
    /// something is by construction not a complete answer.
    fn into_outcome(self) -> ContentOutcome {
        match self {
            Content::Text(s) => {
                if s.is_empty() {
                    ContentOutcome::Empty
                } else {
                    ContentOutcome::Text(s)
                }
            }
            Content::Parts(parts) => {
                let mut text = String::new();
                let mut refusals: Vec<String> = Vec::new();
                let mut saw_refusal = false;
                for part in parts {
                    match part.ty.as_str() {
                        "text" | "output_text" => {
                            if let Some(t) = part.text {
                                text.push_str(&t);
                            }
                        }
                        "refusal" => {
                            saw_refusal = true;
                            // The string lives in `refusal`; `text` is the
                            // lenient fallback for a gateway that reuses the
                            // common field name for it.
                            if let Some(r) = part.refusal.or(part.text).filter(|r| !r.is_empty()) {
                                refusals.push(r);
                            }
                        }
                        _ => {}
                    }
                }
                if saw_refusal {
                    let joined = refusals.join("; ");
                    return ContentOutcome::Refused(if joined.is_empty() {
                        NO_REFUSAL_DETAIL.to_string()
                    } else {
                        joined
                    });
                }
                if text.is_empty() {
                    ContentOutcome::Empty
                } else {
                    ContentOutcome::Text(text)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Schema wrapping
//
// The provider-neutral request body (user prompt + hints), the schema
// object, and the output-envelope parser live in `transync::llm::prompt`
// (OI-0029); only the Chat-specific `{name, strict, schema}` wrapper below
// stays here. The Responses surface puts the very same schema object under
// `text.format` with its own wrapper — see `super::responses`.
// ---------------------------------------------------------------------------

/// JSON schema descriptor for Chat Completions' `response_format.json_schema`
/// field, which expects `{name, strict, schema}`. The *wrapper* is
/// Chat-specific; the embedded schema object comes from
/// `transync::llm::prompt` and is byte-identical to the one the Responses
/// surface puts under `text.format` (OI-0029).
///
/// TRACE: contracts.md §1
fn schema_descriptor(unit_count: Option<usize>) -> SchemaDescriptor {
    SchemaDescriptor {
        name: prompt::SCHEMA_NAME,
        strict: true,
        schema: prompt::schema_object_for(unit_count),
    }
}

/// Chat-specific `{name, strict, schema}` wrapper around the extraction
/// schema object, mirroring [`schema_descriptor`]. OI-0026.
fn extraction_schema_descriptor(max_terms: u32) -> SchemaDescriptor {
    SchemaDescriptor {
        name: prompt::EXTRACTION_SCHEMA_NAME,
        strict: true,
        schema: prompt::extraction_schema_object(max_terms),
    }
}

#[derive(Serialize)]
struct SchemaDescriptor {
    name: &'static str,
    strict: bool,
    schema: serde_json::Value,
}

// OI-0009: unit tests for the pure request/response plumbing. These run
// under plain `cargo test --workspace` — no API key, no network.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::fixtures::{fixture_batch, fixture_extraction_request};

    #[test]
    fn chat_request_body_shape() {
        let batch = fixture_batch();
        let body = translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert_eq!(v["model"], "gpt-5-chat-latest");
        assert_eq!(v["messages"][0]["role"], "system");
        assert_eq!(v["messages"][1]["role"], "user");
        assert_eq!(v["response_format"]["type"], "json_schema");
        assert_eq!(v["response_format"]["json_schema"]["strict"], true);
        assert!(v["response_format"]["json_schema"]["schema"].is_object());
    }

    /// EXT-2026-07 P1-7 (OI-0019): the profile's `target_output_tokens`
    /// rides out as `max_completion_tokens` on Chat Completions.
    #[test]
    fn chat_request_body_sets_max_completion_tokens() {
        let batch = fixture_batch();
        assert_eq!(batch.profile.batching.target_output_tokens, Some(8000));
        let body = translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert_eq!(v["max_completion_tokens"], 8000);
    }

    /// EXT-2026-07 P1-7: no ceiling set → the field is omitted entirely so
    /// the provider applies its own default.
    #[test]
    fn chat_request_body_omits_max_completion_tokens_when_none() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = None;
        let body = translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert!(
            v.get("max_completion_tokens").is_none(),
            "unset ceiling must omit the field, got {v}"
        );
    }

    /// `R0004-0016`, the OpenAI sibling: a zero ceiling names no budget, so
    /// it takes the same road an absent one does — the field is omitted —
    /// rather than riding out as `max_completion_tokens: 0`, which leaves
    /// the model nothing to answer in.
    ///
    /// Core normalizes zero to `None` at both of its gates, so this is only
    /// reachable through a caller-assembled `TranslationBatch` handed
    /// straight to the adapter. That is exactly the bypass the Anthropic
    /// adapter closed at its own boundary; the rule now holds on both
    /// surfaces of both crates.
    #[test]
    fn chat_request_body_omits_max_completion_tokens_when_zero() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = Some(0);
        let body = translation_body(&ModelId::new("gpt-5-chat-latest"), None, &batch)
            .expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert!(
            v.get("max_completion_tokens").is_none(),
            "a zero ceiling must be normalized to absent, never sent, got {v}"
        );
    }

    /// OI-0026: the extraction body embeds the document in the user message
    /// (as data), and the fixed system prompt frames it as such — the
    /// provider adds no framing of its own that could contradict core's.
    #[test]
    fn extraction_chat_body_carries_data_framed_document() {
        let req = fixture_extraction_request();
        let body = extraction_body(&ModelId::new("gpt-4o-mini"), None, &req).expect("body builds");
        let v = serde_json::to_value(&body).expect("serializes");
        assert_eq!(
            v["messages"][0]["content"],
            prompt::EXTRACTION_SYSTEM_PROMPT
        );
        let user: serde_json::Value =
            serde_json::from_str(v["messages"][1]["content"].as_str().expect("user content"))
                .expect("user message is the JSON payload");
        assert_eq!(user["document"], "The agent invokes the tool.");
        assert_eq!(user["max_terms"], 9);
        assert_eq!(user["existing_terms"], serde_json::json!(["agent"]));
    }

    /// R0008-0035: a content-part array (not a bare string) still yields the
    /// concatenated text.
    #[test]
    fn chat_output_from_content_parts() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "choices": [{ "message": { "content": [
                { "type": "text", "text": "{\"a\":" },
                { "type": "text", "text": "1}" }
            ] } }]
        }))
        .expect("envelope parses");
        assert_eq!(output_from_envelope(env).expect("extracts"), "{\"a\":1}");
    }

    /// One `finish_reason: "length"` envelope, built once, for the two tests
    /// below. The content is what such a reply actually carries: the
    /// schema-shaped answer, cut off mid-object.
    fn truncated_envelope() -> Envelope {
        serde_json::from_value(serde_json::json!({
            "choices": [{
                "finish_reason": "length",
                "message": { "content": "{\"batch_id\":\"b1\",\"units\":[{\"unit_id\"" }
            }]
        }))
        .expect("envelope parses")
    }

    /// Ticket 3c9741bf — the Chat mirror of `responses_incomplete_is_error`:
    /// a reply stopped at the output ceiling is reported as an exhausted
    /// ceiling instead of being handed on to `parse_batch_output` to fail as
    /// malformed JSON. The message names the request parameter the provider
    /// enforced and the profile key that set it, so the fix is readable off
    /// the error.
    #[test]
    fn chat_length_finish_reason_is_error() {
        let err = output_from_envelope(truncated_envelope()).expect_err("truncation is an error");
        match &err {
            ProviderError::OutputCeilingExhausted(s) => {
                assert!(
                    s.contains("finish_reason: length"),
                    "must name the stop reason, got {s:?}"
                );
                assert!(
                    s.contains("max_completion_tokens") && s.contains("target_output_tokens"),
                    "must name the ceiling and the knob behind it, got {s:?}"
                );
                // The same reader serves `extraction_body`, whose ceiling is
                // the fixed `EXTRACTION_MAX_OUTPUT_TOKENS` and does not move
                // with the profile knob, so the remediation must stay scoped
                // to the batch rather than promise a fix on every path.
                assert!(
                    s.contains("on a translation batch, raise"),
                    "the remediation must stay scoped to the batch, got {s:?}"
                );
            }
            other => panic!("expected an explicit ceiling error, got {other:?}"),
        }
    }

    /// Ticket 3c9741bf — and it must not reach the pipeline as
    /// `MalformedResponse`, which is what the truncated payload produced
    /// before. Terminal matches what the Responses surface already
    /// does with `incomplete` for the identical condition: the ceiling rides
    /// in the request, so a verbatim resubmission truncates identically.
    ///
    /// ti 1a85f3: the two surfaces now also agree on the *name*, so a
    /// consumer branching on the cause does not have to know which surface
    /// the run dispatched on.
    #[test]
    fn chat_length_finish_reason_is_terminal_like_responses_incomplete() {
        let mapped = map_provider_error(
            output_from_envelope(truncated_envelope()).expect_err("truncation is an error"),
        );
        assert!(
            matches!(mapped, TranslatorError::OutputCeilingExhausted(_)),
            "an exhausted ceiling must be terminal and not malformed, got {mapped:?}"
        );
        assert_eq!(mapped.stable_code(), "provider_output_ceiling_exhausted");
    }

    /// A `finish_reason: "content_filter"` envelope, built once for the tests
    /// below. `None` is the shape where the filter left no body at all — the
    /// key omitted, which `#[serde(default)]` and an explicit `null`
    /// deserialize into the same `Option::None`, so the two are one shape at
    /// this layer. `Some(_)` is the shape where the filter cut the
    /// schema-shaped answer off where it fired.
    fn filtered_envelope(content: Option<&str>) -> Envelope {
        let message = match content {
            Some(text) => serde_json::json!({ "content": text }),
            None => serde_json::json!({}),
        };
        serde_json::from_value(serde_json::json!({
            "choices": [{ "finish_reason": "content_filter", "message": message }]
        }))
        .expect("envelope parses")
    }

    /// Ticket 0583a75a — a provider content-policy stop is reported as one.
    /// Both shapes such a reply arrives in are covered: no body at all (which
    /// used to be the missing-`content` `Malformed`) and a body cut off where
    /// the filter fired (which used to reach `parse_batch_output` and fail as
    /// malformed JSON). Neither diagnosis was wrong about the *body*; both
    /// were wrong about *why*, blaming the response's shape for a decision
    /// the provider had already labelled.
    #[test]
    fn chat_content_filter_finish_reason_is_error() {
        for (shape, content) in [
            ("content absent", None),
            (
                "content partial",
                Some("{\"batch_id\":\"b1\",\"units\":[{\"unit_id\""),
            ),
        ] {
            let Err(err) = output_from_envelope(filtered_envelope(content)) else {
                panic!("{shape}: a content-policy stop must not reach extraction");
            };
            match &err {
                ProviderError::ContentFiltered(s) => {
                    assert!(
                        s.contains("finish_reason: content_filter"),
                        "{shape}: must name the stop reason, got {s:?}"
                    );
                    assert!(
                        s.contains("content filter") && s.contains("content policy"),
                        "{shape}: must name the provider's filter as the cause, got {s:?}"
                    );
                    // The whole point of the ticket: the operator must not
                    // read this as the transport or the schema misbehaving.
                    assert!(
                        s.contains("not by a transport fault or a schema mismatch"),
                        "{shape}: must not read as a transport or schema fault, got {s:?}"
                    );
                }
                other => panic!("{shape}: expected an explicit policy-stop error, got {other:?}"),
            }
        }
    }

    /// Ticket 0583a75a — and it is terminal, not a retry. The resubmission
    /// ADR-0009 prescribes is verbatim, and identical content is what the
    /// filter acted on, so every attempt would stop the same way.
    /// `MalformedResponse` — what both shapes produced before — happens to be
    /// terminal too, so this asserts the classification rather than a change
    /// in run outcome.
    ///
    /// ti 1a85f3: the classification is now `ContentFiltered`, and the code a
    /// process-boundary consumer reads is `provider_content_filtered`. That
    /// distinction is the reason the ticket exists — flattened into `Other`,
    /// a consumer had to advertise this stop as "try again", which is an
    /// invitation into an unbounded loop of paid, identical failures.
    #[test]
    fn chat_content_filter_finish_reason_is_terminal() {
        for content in [None, Some("{\"batch_id\":\"b1\"")] {
            let mapped = map_provider_error(
                output_from_envelope(filtered_envelope(content)).expect_err("filter stop errors"),
            );
            assert!(
                matches!(mapped, TranslatorError::ContentFiltered(_)),
                "a content-policy stop must be terminal and not malformed, got {mapped:?}"
            );
            assert_eq!(mapped.stable_code(), "provider_content_filtered");
        }
    }

    /// A filter stop is not the `message.refusal` condition (R0008-0035) and
    /// must not borrow its wording: a refusal is the *model* declining, and it
    /// comes back as `finish_reason: "stop"` with a `refusal` string, which
    /// still reaches the refusal branch untouched.
    ///
    /// ti 1a85f3: the two are now distinct in the **type** as well as in the
    /// wording, so telling them apart no longer requires reading the message.
    #[test]
    fn chat_refusal_and_content_filter_are_distinct_diagnoses() {
        let filtered = output_from_envelope(filtered_envelope(None)).expect_err("filter errors");
        assert!(
            matches!(&filtered, ProviderError::ContentFiltered(s) if !s.contains("refused")),
            "a filter stop must not be reported as a model refusal, got {filtered:?}"
        );
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "choices": [{
                "finish_reason": "stop",
                "message": { "refusal": "I can't help with that." }
            }]
        }))
        .expect("envelope parses");
        let refused = output_from_envelope(env).expect_err("refusal errors");
        assert!(
            matches!(&refused, ProviderError::ModelRefused(s)
                if s.contains("refused") && !s.contains("content_filter")),
            "a refusal must keep its own diagnosis, got {refused:?}"
        );
        assert_ne!(
            map_provider_error(filtered).stable_code(),
            map_provider_error(refused).stable_code(),
            "ti 1a85f3: the two causes must not share a stable code"
        );
    }

    /// Tickets 3c9741bf and 0583a75a: only `length` and `content_filter` are
    /// acted on. Every other stop reason — including one this transport does
    /// not model — still goes to extraction, so a gateway with its own
    /// vocabulary keeps working.
    #[test]
    fn chat_other_finish_reasons_still_extract() {
        for finish_reason in [
            serde_json::json!("stop"),
            serde_json::json!("tool_calls"),
            serde_json::json!("some_gateway_reason"),
            serde_json::Value::Null,
        ] {
            let env: Envelope = serde_json::from_value(serde_json::json!({
                "choices": [{
                    "finish_reason": finish_reason.clone(),
                    "message": { "content": "{\"a\":1}" }
                }]
            }))
            .expect("envelope parses");
            assert_eq!(
                output_from_envelope(env).expect("extracts"),
                "{\"a\":1}",
                "finish_reason {finish_reason} must not block extraction"
            );
        }
    }

    /// R0008-0035: an explicit refusal becomes a diagnostic error, not a
    /// generic malformed-envelope failure.
    #[test]
    fn chat_output_refusal_is_error() {
        let env: Envelope = serde_json::from_value(serde_json::json!({
            "choices": [{ "message": { "refusal": "I can't help with that." } }]
        }))
        .expect("envelope parses");
        let err = output_from_envelope(env).expect_err("refusal is an error");
        assert!(
            matches!(err, ProviderError::ModelRefused(ref s) if s.contains("refused")),
            "got {err:?}"
        );
        assert_eq!(
            map_provider_error(err).stable_code(),
            "provider_model_refused"
        );
    }

    /// R0002-0041: a refusal delivered as a **content part** reaches the same
    /// diagnosis `message.refusal` gets. Three shapes, one verdict:
    /// refusal-only (which used to degrade into the generic missing-content
    /// error), refusal beside text (where the text used to be returned as
    /// ordinary output), and a refusal part carrying no string of its own.
    ///
    /// The expected sentence is asserted whole, not merely searched for: the
    /// Responses surface asserts the identical one from the identical constant
    /// (ti `594a7f`), which is what makes "both surfaces read the same" a
    /// property a test can lose rather than a claim in a comment.
    #[test]
    fn chat_refusal_content_parts_are_not_dropped() {
        for (shape, parts, detail) in [
            (
                "refusal only",
                serde_json::json!([{ "type": "refusal", "refusal": "I can't help with that." }]),
                "I can't help with that.",
            ),
            (
                "refusal beside text",
                serde_json::json!([
                    { "type": "text", "text": "{\"batch_id\":\"b1\"}" },
                    { "type": "refusal", "refusal": "I can't help with that." }
                ]),
                "I can't help with that.",
            ),
            (
                "refusal with no string",
                serde_json::json!([{ "type": "refusal" }]),
                NO_REFUSAL_DETAIL,
            ),
            (
                "refusal with an empty string",
                serde_json::json!([{ "type": "refusal", "refusal": "" }]),
                NO_REFUSAL_DETAIL,
            ),
        ] {
            let env: Envelope = serde_json::from_value(serde_json::json!({
                "choices": [{ "message": { "content": parts } }]
            }))
            .expect("envelope parses");
            let Err(err) = output_from_envelope(env) else {
                panic!("{shape}: a refusal part must not extract as ordinary output");
            };
            let ProviderError::ModelRefused(s) = err else {
                panic!("{shape}: a refusal part must earn the refusal diagnosis");
            };
            assert_eq!(
                s,
                format!("model refused to translate: {detail}"),
                "{shape}: unexpected refusal diagnostic"
            );
        }
    }

    /// R0002-0039: a refusal is provider-controlled text, so it is capped at
    /// the same 512-byte diagnostic excerpt an error body gets, on both the
    /// `message.refusal` field and the content-part path.
    #[test]
    fn chat_refusal_diagnostic_is_capped() {
        let huge = "r".repeat(200_000);
        let envelopes = [
            (
                "message.refusal",
                serde_json::json!({ "refusal": huge.clone() }),
            ),
            (
                "refusal content part",
                serde_json::json!({ "content": [{ "type": "refusal", "refusal": huge.clone() }] }),
            ),
        ];
        for (shape, message) in envelopes {
            let env: Envelope =
                serde_json::from_value(serde_json::json!({ "choices": [{ "message": message }] }))
                    .expect("envelope parses");
            match output_from_envelope(env).expect_err("refusal is an error") {
                ProviderError::ModelRefused(s) => {
                    assert!(
                        s.len() < 1024,
                        "{shape}: the refusal must be capped, got {} bytes",
                        s.len()
                    );
                    assert!(
                        s.contains("refused to translate") && s.contains("truncated"),
                        "{shape}: got {s:?}"
                    );
                }
                other => panic!("{shape}: expected a refusal error, got {other:?}"),
            }
        }
    }
}
