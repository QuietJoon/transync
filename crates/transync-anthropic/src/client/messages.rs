//! The Messages API wire shape: request builders and the envelope reader.
//!
//! Everything this provider needs for *both* use cases lives here — the wire
//! DTOs, [`translation_body`] and [`extraction_body`] (which build the same
//! [`Request`] and differ only in prompts, schema and output ceiling), and
//! [`extract_output`], the single reader of a Messages reply. `super`'s flow
//! reaches past those three functions for nothing.
//!
//! Four things are this API's, not the OpenAI adapter's:
//!
//! - the **system prompt is a top-level field**, so `messages` holds exactly
//!   one user turn rather than a system/user pair;
//! - the schema rides under **`output_config.format`**, and goes through the
//!   [`super::schema`] profile pass on the way (the old top-level
//!   `output_format` parameter is deprecated on this API and is never sent);
//! - **`max_tokens` is required** — there is no "omit it and take the
//!   provider default" path, which is why [`Ceiling`] exists;
//! - **no `thinking` parameter is ever sent.** The legacy budget-token form
//!   is rejected on current models and the explicit `disabled` form is
//!   rejected on part of the family and effort-gated on another part, so
//!   *not sending it* is the only choice valid across the whole family.
//!   `output_config.effort` is the sanctioned depth knob instead.
//!
//! TRACE: DCR-0029
//! TRACE: contracts.md §8

use serde::{Deserialize, Serialize};
use transync::llm::{GlossaryExtractionRequest, TranslationBatch, TranslatorError, prompt};

use crate::error::{ProviderError, map_provider_error, truncate_diagnostic};
use crate::{DEFAULT_MAX_OUTPUT_TOKENS, Effort, ModelId};

use super::EXTRACTION_MAX_OUTPUT_TOKENS;
use super::schema::to_provider_dialect;

/// The output ceiling a request carried, and where it came from.
///
/// It is carried rather than recomputed because the ceiling diagnostic has
/// to name **the value that was sent and its origin**: on this API the
/// ceiling always rides in the request, so `stop_reason: "max_tokens"` can
/// fire on a run whose operator configured nothing, and "raise the knob you
/// never set" is not a remediation anyone can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Ceiling {
    /// `[batching].target_output_tokens` supplied it.
    Configured(u32),
    /// Nothing configured it, so [`DEFAULT_MAX_OUTPUT_TOKENS`] rode out.
    Default(u32),
    /// The glossary preflight's own fixed cap, which the profile knob does
    /// not move.
    ExtractionPreflight(u32),
}

impl Ceiling {
    /// The number that went on the wire as `max_tokens`.
    fn value(self) -> u32 {
        match self {
            Self::Configured(n) | Self::Default(n) | Self::ExtractionPreflight(n) => n,
        }
    }

    /// The clause naming where the sent value came from.
    fn origin(self) -> String {
        match self {
            Self::Configured(_) => "it came from [batching].target_output_tokens".to_string(),
            Self::Default(n) => format!(
                "nothing configured it, so this crate's DEFAULT_MAX_OUTPUT_TOKENS of {n} rode out \
                 — max_tokens is a required field on this API, so a ceiling is always sent"
            ),
            Self::ExtractionPreflight(_) => "it is the glossary preflight's own fixed cap, which \
                 [batching].target_output_tokens does not move"
                .to_string(),
        }
    }
}

/// Shape one translation batch as a Messages-API request.
///
/// **A `target_output_tokens` of zero is normalized to the default ceiling,
/// not sent** (R0004-0016). Core's `profile::normalize_batching` already maps
/// `Some(0)` to `None` — "not an output budget; ignored … exactly as if the
/// key were absent" — and it runs at both `load_profile` and
/// `unit::build_batches`, so the pipeline never hands one down. A caller that
/// builds a [`TranslationBatch`] itself and calls this adapter directly
/// bypasses both, and `max_tokens: 0` is not merely a useless ceiling on this
/// API: paired with the `output_config.format` this adapter always sends, the
/// provider rejects the request outright. Zero is normalized here on core's
/// own terms — absent means [`Ceiling::Default`], because `max_tokens` is
/// required and something must ride — so the narrow bypass behaves the way
/// the configured door already does instead of buying a remote 400.
pub(super) fn translation_body(
    model: &ModelId,
    effort: Option<Effort>,
    batch: &TranslationBatch,
) -> Result<Shaped, TranslatorError> {
    let ceiling = match batch.profile.batching.target_output_tokens {
        // Zero names no budget; see this function's docs.
        Some(0) | None => Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
        Some(configured) => Ceiling::Configured(configured),
    };
    Ok(Shaped {
        body: Request {
            model: model.0.clone(),
            max_tokens: ceiling.value(),
            // The profile's prompt body is the system prompt, carried at the
            // top level rather than as a `messages[0]` turn. The adapter adds
            // no framing of its own around it: the untrusted-data framing
            // (invariant 7) is the shared prompt's job.
            system: batch.profile.prompt_body.clone(),
            messages: vec![Message {
                role: "user",
                content: prompt::build_user_prompt(batch)?,
            }],
            output_config: OutputConfig {
                format: OutputFormat {
                    ty: "json_schema",
                    schema: to_provider_dialect(prompt::schema_object_for(Some(batch.units.len()))),
                },
                effort,
            },
        },
        ceiling,
    })
}

/// Shape the one-per-run candidate-glossary preflight as a Messages-API
/// request. OI-0026.
pub(super) fn extraction_body(
    model: &ModelId,
    effort: Option<Effort>,
    req: &GlossaryExtractionRequest,
) -> Result<Shaped, TranslatorError> {
    let ceiling = Ceiling::ExtractionPreflight(EXTRACTION_MAX_OUTPUT_TOKENS);
    Ok(Shaped {
        body: Request {
            model: model.0.clone(),
            max_tokens: ceiling.value(),
            system: prompt::EXTRACTION_SYSTEM_PROMPT.to_string(),
            messages: vec![Message {
                role: "user",
                content: prompt::build_extraction_user_prompt(req)?,
            }],
            output_config: OutputConfig {
                format: OutputFormat {
                    ty: "json_schema",
                    schema: to_provider_dialect(prompt::extraction_schema_object(req.max_terms)),
                },
                effort,
            },
        },
        ceiling,
    })
}

/// A request body together with the ceiling it declares.
///
/// The pair travels together because the reply's reader needs the second
/// half to diagnose the first: `stop_reason: "max_tokens"` is only
/// actionable if the operator is told which number was enforced and who
/// chose it.
pub(super) struct Shaped {
    pub(super) body: Request,
    pub(super) ceiling: Ceiling,
}

/// Read a Messages reply: deserialize the envelope, then pull the model's
/// JSON string out of it.
///
/// The only place the envelope is parsed, so every use case reports a
/// malformed envelope, an exhausted ceiling, a policy stop, a refusal, a
/// context-window overflow and a missing body identically.
pub(super) fn extract_output(bytes: &[u8], ceiling: Ceiling) -> Result<String, TranslatorError> {
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|e| {
        map_provider_error(ProviderError::Malformed(format!(
            "could not parse Messages envelope: {e}"
        )))
    })?;
    output_from_envelope(envelope, ceiling).map_err(map_provider_error)
}

/// Extract the model's JSON string from a Messages envelope.
///
/// **`stop_reason` is read before any content, and that ordering is a rule
/// rather than a habit**: on this API a refusal can arrive with an *empty*
/// `content` array, so code that reaches for the first block unconditionally
/// reports a missing body for what the provider already labelled a decline.
/// The same ordering is what makes a truncated answer an exhausted ceiling
/// instead of malformed JSON one layer down.
///
/// Every stop reason this reader acts on is **terminal** — none is one the
/// pipeline re-dispatches — and each is terminal for its own reason:
///
/// - `max_tokens`: the ceiling rides in the request, so ADR-0009's verbatim
///   resubmission truncates identically.
/// - `refusal`: the resubmission is verbatim and identical content is what
///   the safety layer acted on, so every attempt stops the same way.
/// - `model_context_window_exceeded`: the request did not fit, and it will
///   not fit again.
/// - `stop_sequence` / `tool_use` / `pause_turn`: unreachable by
///   construction, since this adapter sends no stop sequences and declares
///   no tools. If one arrives, *unknown* is the honest answer, so it takes
///   the sanctioned catch-all with the reason named. `pause_turn` in
///   particular is a **resumable** state the taxonomy deliberately cannot
///   express, and expressing it would be a server-tools feature this crate
///   does not have.
///
/// `end_turn`, an absent field, and any vocabulary this reader does not
/// model all proceed to extraction, so a reply that does carry a usable body
/// is never rejected on the strength of a label.
fn output_from_envelope(envelope: Envelope, ceiling: Ceiling) -> Result<String, ProviderError> {
    if let Some(err) = stop_reason_error(&envelope, ceiling) {
        return Err(err);
    }
    // The model's JSON rides in the first **non-empty** `text`-typed block.
    // Thinking blocks, when a model emits them, precede it — they are skipped
    // here, never parsed and never logged.
    //
    // R0004-0018: emptiness is part of *finding* the answer, not a check
    // applied after one was chosen. Selecting the first `text` block and only
    // then filtering emptiness reported `no non-empty text content block` for
    // `[{text: ""}, {text: "{…}"}]` — a reply that carried the answer in its
    // second block. That is a valid response turned into a malformed one: it
    // burns a content retry, and on repeat falls the unit back to source.
    // Skipping empties during selection costs nothing and cannot reach past a
    // block that does carry text.
    envelope
        .content
        .into_iter()
        .filter(|block| block.ty == "text")
        .find_map(|block| block.text.filter(|text| !text.is_empty()))
        .ok_or_else(|| {
            ProviderError::Malformed(
                "Messages response carried no non-empty text content block".to_string(),
            )
        })
}

/// Map a `stop_reason` that cannot carry a complete answer onto a
/// [`ProviderError`]; `None` means extraction may proceed.
fn stop_reason_error(envelope: &Envelope, ceiling: Ceiling) -> Option<ProviderError> {
    match envelope.stop_reason.as_deref() {
        Some("max_tokens") => Some(ProviderError::OutputCeilingExhausted(format!(
            "Messages output incomplete (stop_reason: max_tokens): the max_tokens ceiling of {} \
             was exhausted before the answer was complete — {}. On this API max_tokens caps \
             thinking and answer together, so reasoning shares the budget; a short batch can \
             exhaust it. On a translation batch, raise [batching].target_output_tokens \
             (CLI --target-output-tokens)",
            ceiling.value(),
            ceiling.origin(),
        ))),
        // `stop_details` is populated **only** under a refusal, so it is read
        // only here — and its `category` is the sole evidence of *which*
        // layer declined. A category means the provider's safety layer named
        // a policy; its absence means the model itself declined without one.
        // The split is a stated heuristic: both are terminal with no
        // remediation, so a misdrawn boundary costs a name and never a
        // behavior, and the two causes stay distinct rather than flattened.
        Some("refusal") => {
            let details = envelope.stop_details.as_ref();
            let explanation = details
                .and_then(|d| d.explanation.as_deref())
                .filter(|e| !e.is_empty())
                .map(|e| truncate_diagnostic(e.as_bytes()))
                .unwrap_or_else(|| NO_REFUSAL_DETAIL.to_string());
            match details
                .and_then(|d| d.category.as_deref())
                .filter(|c| !c.is_empty())
            {
                Some(category) => Some(ProviderError::ContentFiltered(format!(
                    "Messages generation stopped by the provider's safety layer \
                     (stop_reason: refusal, stop_details.category: {}): the reply was ended by a \
                     content policy, not by a transport fault or a schema mismatch — {explanation}",
                    truncate_diagnostic(category.as_bytes()),
                ))),
                None => Some(ProviderError::ModelRefused(format!(
                    "model refused to translate (stop_reason: refusal, no \
                     stop_details.category): {explanation}"
                ))),
            }
        }
        Some("model_context_window_exceeded") => Some(ProviderError::ContextWindowExceeded(
            "Messages request rejected (stop_reason: model_context_window_exceeded): input plus \
             requested output did not fit the model's context window. The remediation is the \
             batching configuration ([batching] token budget / max_units_per_batch), not the \
             output ceiling"
                .to_string(),
        )),
        Some(unreachable @ ("stop_sequence" | "tool_use" | "pause_turn")) => {
            Some(ProviderError::Other(format!(
                "Messages generation stopped with stop_reason: {unreachable}, which a request \
                 this adapter shaped cannot elicit — it sends no stop sequences and declares no \
                 tools"
            )))
        }
        _ => None,
    }
}

/// What a refusal with no explanation reports in place of one. The refusal
/// itself is the fact worth carrying; its wording is optional.
const NO_REFUSAL_DETAIL: &str = "no refusal detail";

#[derive(Serialize)]
pub(super) struct Request {
    model: String,
    /// Required by this API — there is no omit-and-default path, which is
    /// the whole reason [`Ceiling`] exists.
    max_tokens: u32,
    /// Top-level, not a `messages[0]` turn: that is where this API puts the
    /// system prompt.
    system: String,
    messages: Vec<Message>,
    output_config: OutputConfig,
}

#[derive(Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Serialize)]
struct OutputConfig {
    format: OutputFormat,
    /// Omitted when unconfigured, so the provider applies its own default.
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<Effort>,
}

#[derive(Serialize)]
struct OutputFormat {
    #[serde(rename = "type")]
    ty: &'static str,
    schema: serde_json::Value,
}

#[derive(Deserialize)]
struct Envelope {
    #[serde(default)]
    stop_reason: Option<String>,
    /// Populated **only** under `stop_reason: "refusal"`; `null` for every
    /// other stop reason, which is why it is guarded rather than read.
    #[serde(default)]
    stop_details: Option<StopDetails>,
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct StopDetails {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    explanation: Option<String>,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    ty: String,
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::fixtures::{fixture_batch, fixture_extraction_request};
    use crate::client::schema;

    fn model() -> ModelId {
        ModelId::new("claude-opus-5")
    }

    fn body_of(shaped: &Shaped) -> serde_json::Value {
        serde_json::to_value(&shaped.body).expect("request body serializes")
    }

    fn envelope(value: serde_json::Value) -> Envelope {
        serde_json::from_value(value).expect("envelope parses")
    }

    /// The translation request's shape, pinned whole: the system prompt is a
    /// top-level field, `messages` is one user turn, the schema rides under
    /// `output_config.format`, and `max_tokens` is present.
    #[test]
    fn translation_request_body_shape() {
        let batch = fixture_batch();
        let shaped = translation_body(&model(), None, &batch).expect("body builds");
        let v = body_of(&shaped);

        assert_eq!(v["model"], "claude-opus-5");
        assert_eq!(
            v["system"], batch.profile.prompt_body,
            "the system prompt is a top-level field on this API"
        );
        assert_eq!(
            v["messages"]
                .as_array()
                .expect("messages is an array")
                .len(),
            1,
            "one user turn, not a system/user pair: {v}"
        );
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["output_config"]["format"]["type"], "json_schema");
        assert!(v["output_config"]["format"]["schema"].is_object());
        assert!(
            v.get("output_format").is_none(),
            "the deprecated top-level parameter must never be sent: {v}"
        );
    }

    /// The one parameter this crate must never send, in either body.
    /// Sending any `thinking` form would make the adapter's validity a
    /// function of the model name, which is a per-model capability table
    /// this crate refuses to carry.
    #[test]
    fn no_body_ever_carries_a_thinking_parameter() {
        let batch = fixture_batch();
        let req = fixture_extraction_request();
        for (what, v) in [
            (
                "translation",
                body_of(&translation_body(&model(), Some(Effort::Max), &batch).expect("builds")),
            ),
            (
                "extraction",
                body_of(&extraction_body(&model(), Some(Effort::Max), &req).expect("builds")),
            ),
        ] {
            assert!(
                v.get("thinking").is_none(),
                "{what}: no thinking parameter is valid across the whole model family: {v}"
            );
        }
    }

    /// The profile's ceiling rides out verbatim when it is set.
    #[test]
    fn a_configured_ceiling_rides_out_as_max_tokens() {
        let batch = fixture_batch();
        assert_eq!(
            batch.profile.batching.target_output_tokens,
            Some(8000),
            "precondition: the fixture profile configures a ceiling"
        );
        let shaped = translation_body(&model(), None, &batch).expect("body builds");
        assert_eq!(shaped.ceiling, Ceiling::Configured(8000));
        assert_eq!(body_of(&shaped)["max_tokens"], 8000);
    }

    /// …and when it is not, the crate default rides out instead — because
    /// `max_tokens` is required, there is no third option.
    #[test]
    fn an_unset_ceiling_sends_the_crate_default_rather_than_omitting_the_field() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = None;
        let shaped = translation_body(&model(), None, &batch).expect("body builds");
        assert_eq!(shaped.ceiling, Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS));
        assert_eq!(body_of(&shaped)["max_tokens"], DEFAULT_MAX_OUTPUT_TOKENS);
        assert_eq!(DEFAULT_MAX_OUTPUT_TOKENS, 16_384);
    }

    /// R0004-0016: a zero ceiling names no budget, so it takes the same road
    /// an absent one does rather than riding out as `max_tokens: 0`.
    ///
    /// Core's `profile::normalize_batching` already maps `Some(0)` to `None`
    /// at both `load_profile` and `unit::build_batches`, so the pipeline never
    /// hands one down; a caller assembling a `TranslationBatch` itself and
    /// calling the adapter directly bypasses both. On this API that bypass is
    /// not merely wasteful — `max_tokens: 0` alongside the `output_config.format`
    /// this adapter always sends is refused by the provider — so the request
    /// is fixed here instead of being sent to be rejected.
    #[test]
    fn a_zero_ceiling_is_normalized_to_the_default_rather_than_sent() {
        let mut batch = fixture_batch();
        batch.profile.batching.target_output_tokens = Some(0);
        let shaped = translation_body(&model(), None, &batch).expect("body builds");
        assert_eq!(
            shaped.ceiling,
            Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
            "zero must be treated exactly as the key being absent"
        );
        assert_eq!(
            body_of(&shaped)["max_tokens"],
            DEFAULT_MAX_OUTPUT_TOKENS,
            "max_tokens: 0 must never reach the provider"
        );
    }

    /// The extraction preflight keeps its own fixed cap, which the profile
    /// knob does not move.
    #[test]
    fn extraction_keeps_its_own_ceiling() {
        let req = fixture_extraction_request();
        let shaped = extraction_body(&model(), None, &req).expect("body builds");
        assert_eq!(
            shaped.ceiling,
            Ceiling::ExtractionPreflight(EXTRACTION_MAX_OUTPUT_TOKENS)
        );
        let v = body_of(&shaped);
        assert_eq!(v["max_tokens"], EXTRACTION_MAX_OUTPUT_TOKENS);
        assert_eq!(v["system"], prompt::EXTRACTION_SYSTEM_PROMPT);
        let user: serde_json::Value =
            serde_json::from_str(v["messages"][0]["content"].as_str().expect("user content"))
                .expect("user message is the JSON payload");
        assert_eq!(user["document"], "The agent invokes the tool.");
        assert_eq!(user["max_terms"], 9);
    }

    /// Effort goes under `output_config`, and is omitted entirely when
    /// unconfigured so the provider applies its own default.
    #[test]
    fn effort_is_placed_under_output_config_and_omitted_when_unset() {
        let batch = fixture_batch();
        let with =
            body_of(&translation_body(&model(), Some(Effort::Xhigh), &batch).expect("builds"));
        assert_eq!(with["output_config"]["effort"], "xhigh");
        assert!(
            with.get("effort").is_none(),
            "effort is nested, never top-level: {with}"
        );

        let without = body_of(&translation_body(&model(), None, &batch).expect("builds"));
        assert!(
            without["output_config"].get("effort").is_none(),
            "an unconfigured effort must be omitted: {without}"
        );
    }

    /// Both bodies send the shared schema through the profile pass — there
    /// is no prompt-coaxed-JSON mode, and no path that sends the raw object.
    #[test]
    fn both_bodies_carry_the_shared_schema_in_the_providers_dialect() {
        let batch = fixture_batch();
        let req = fixture_extraction_request();
        let bodies = [
            (
                "translation",
                body_of(&translation_body(&model(), None, &batch).expect("builds")),
                to_provider_dialect(prompt::schema_object_for(Some(batch.units.len()))),
            ),
            (
                "extraction",
                body_of(&extraction_body(&model(), None, &req).expect("builds")),
                to_provider_dialect(prompt::extraction_schema_object(req.max_terms)),
            ),
        ];
        for (what, v, expected) in bodies {
            let sent = &v["output_config"]["format"]["schema"];
            assert_eq!(
                sent, &expected,
                "{what}: the profiled shared object is what goes out"
            );
            let keys = schema::all_keys(sent);
            for keyword in schema::unsupported_keywords() {
                assert!(
                    !keys.iter().any(|k| k == keyword),
                    "{what}: {keyword} must not reach the wire"
                );
            }
            assert_eq!(sent["additionalProperties"], false);
            assert!(sent["required"].is_array());
        }
    }

    /// The reader's first rule: `stop_reason` before content. A refusal with
    /// an **empty** content array is the shape that proves it — reaching for
    /// the first block first would report a missing body for a decline the
    /// provider already labelled.
    #[test]
    fn a_refusal_with_no_content_is_read_as_a_refusal() {
        let err = output_from_envelope(
            envelope(serde_json::json!({ "stop_reason": "refusal", "content": [] })),
            Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
        )
        .expect_err("a refusal is an error");
        assert!(
            matches!(&err, ProviderError::ModelRefused(s) if s.contains("stop_reason: refusal")),
            "got {err:?}"
        );
    }

    /// The refusal split, on the only evidence the API gives: a
    /// `stop_details.category` means the safety layer named a policy;
    /// its absence means the model itself declined.
    #[test]
    fn the_refusal_split_turns_on_the_stop_details_category() {
        let filtered = output_from_envelope(
            envelope(serde_json::json!({
                "stop_reason": "refusal",
                "stop_details": { "category": "cyber", "explanation": "policy text" },
                "content": []
            })),
            Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
        )
        .expect_err("a filtered refusal is an error");
        match &filtered {
            ProviderError::ContentFiltered(s) => {
                assert!(
                    s.contains("cyber") && s.contains("policy text"),
                    "got {s:?}"
                );
                assert!(
                    s.contains("not by a transport fault or a schema mismatch"),
                    "the diagnosis must deny the two readings an empty body invites: {s:?}"
                );
            }
            other => panic!("a categorized refusal must be a policy stop, got {other:?}"),
        }

        // No category — and an explicitly null one, which deserializes the
        // same way — is the model declining without a policy label.
        for shape in [
            serde_json::json!({ "stop_reason": "refusal", "content": [] }),
            serde_json::json!({
                "stop_reason": "refusal",
                "stop_details": { "category": null, "explanation": "I can't help with that." },
                "content": []
            }),
        ] {
            let refused =
                output_from_envelope(envelope(shape), Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS))
                    .expect_err("a refusal is an error");
            assert!(
                matches!(&refused, ProviderError::ModelRefused(_)),
                "an uncategorized refusal must stay the model's own decline, got {refused:?}"
            );
        }

        // The two causes must not share a taxonomy code — that separation is
        // the whole reason the split exists.
        assert_ne!(
            map_provider_error(filtered).stable_code(),
            map_provider_error(ProviderError::ModelRefused("x".into())).stable_code()
        );
    }

    /// A refusal explanation is provider-controlled text, so it is capped at
    /// the same 512-byte diagnostic excerpt an error body gets.
    #[test]
    fn a_refusal_explanation_is_capped() {
        let huge = "r".repeat(200_000);
        let err = output_from_envelope(
            envelope(serde_json::json!({
                "stop_reason": "refusal",
                "stop_details": { "explanation": huge },
                "content": []
            })),
            Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
        )
        .expect_err("a refusal is an error");
        let ProviderError::ModelRefused(s) = err else {
            panic!("expected a refusal");
        };
        assert!(
            s.len() < 1024,
            "the explanation must be capped, got {} bytes",
            s.len()
        );
        assert!(s.contains("truncated"), "got {s:?}");
    }

    /// An exhausted ceiling names the value that was sent, its origin, and
    /// that reasoning shares the budget — the last clause matters because a
    /// short batch can exhaust a ceiling on this API for reasons that have
    /// nothing to do with the answer's length.
    #[test]
    fn an_exhausted_ceiling_names_the_sent_value_its_origin_and_the_thinking_share() {
        let cases = [
            (Ceiling::Configured(8000), "[batching].target_output_tokens"),
            (
                Ceiling::Default(DEFAULT_MAX_OUTPUT_TOKENS),
                "DEFAULT_MAX_OUTPUT_TOKENS",
            ),
            (
                Ceiling::ExtractionPreflight(EXTRACTION_MAX_OUTPUT_TOKENS),
                "glossary preflight's own fixed cap",
            ),
        ];
        for (ceiling, expected_origin) in cases {
            let err = output_from_envelope(
                envelope(serde_json::json!({
                    "stop_reason": "max_tokens",
                    "content": [{ "type": "text", "text": "{\"units\":[{\"unit_id\"" }]
                })),
                ceiling,
            )
            .expect_err("a truncated answer is an error");
            let ProviderError::OutputCeilingExhausted(s) = &err else {
                panic!("{ceiling:?}: expected an explicit ceiling error, got {err:?}");
            };
            assert!(s.contains("stop_reason: max_tokens"), "{ceiling:?}: {s:?}");
            assert!(
                s.contains(&ceiling.value().to_string()),
                "{ceiling:?}: the enforced value must be named: {s:?}"
            );
            assert!(
                s.contains(expected_origin),
                "{ceiling:?}: the origin must be named: {s:?}"
            );
            assert!(
                s.contains("thinking and answer together"),
                "{ceiling:?}: reasoning shares the budget and the operator must be told: {s:?}"
            );
            assert_eq!(
                map_provider_error(
                    output_from_envelope(
                        envelope(serde_json::json!({ "stop_reason": "max_tokens", "content": [] })),
                        ceiling
                    )
                    .expect_err("still an error"),
                )
                .stable_code(),
                "provider_output_ceiling_exhausted"
            );
        }
    }

    /// A context-window overflow must never borrow the ceiling's name: the
    /// remediation is the batching configuration, and pointing at the output
    /// knob is the actively-harmful classification DCR-0023 exists to remove.
    #[test]
    fn a_context_window_stop_names_the_batching_knobs_not_the_output_ceiling() {
        let err = output_from_envelope(
            envelope(serde_json::json!({
                "stop_reason": "model_context_window_exceeded",
                "content": []
            })),
            Ceiling::Configured(8000),
        )
        .expect_err("an overflow is an error");
        let ProviderError::ContextWindowExceeded(s) = &err else {
            panic!("expected a context-window error, got {err:?}");
        };
        assert!(s.contains("model_context_window_exceeded"), "got {s:?}");
        assert!(
            s.contains("max_units_per_batch"),
            "the remediation is the batching configuration: {s:?}"
        );
        assert!(
            !matches!(err, ProviderError::OutputCeilingExhausted(_)),
            "it must not be reported as an exhausted output ceiling"
        );
    }

    /// The three stop reasons a request this adapter shaped cannot elicit
    /// take the sanctioned catch-all with the reason named — *unknown* is the
    /// honest answer, and `pause_turn` in particular is a resumable state the
    /// taxonomy deliberately cannot express.
    #[test]
    fn unreachable_stop_reasons_take_the_catch_all_and_name_themselves() {
        for reason in ["stop_sequence", "tool_use", "pause_turn"] {
            let err = output_from_envelope(
                envelope(serde_json::json!({
                    "stop_reason": reason,
                    "content": [{ "type": "text", "text": "{}" }]
                })),
                Ceiling::Configured(8000),
            )
            .expect_err("an unmodelled stop is an error");
            assert!(
                matches!(&err, ProviderError::Other(s) if s.contains(reason)),
                "{reason}: got {err:?}"
            );
            assert_eq!(map_provider_error(err).stable_code(), "provider_error");
        }
    }

    /// Every other stop reason proceeds to extraction, so a reply that does
    /// carry a usable body is never rejected on the strength of a label this
    /// reader does not model.
    #[test]
    fn end_turn_absent_and_unknown_stop_reasons_all_extract() {
        for stop_reason in [
            serde_json::json!("end_turn"),
            serde_json::Value::Null,
            serde_json::json!("some_gateway_reason"),
        ] {
            let env = envelope(serde_json::json!({
                "stop_reason": stop_reason.clone(),
                "content": [{ "type": "text", "text": "{\"a\":1}" }]
            }));
            assert_eq!(
                output_from_envelope(env, Ceiling::Configured(8000)).expect("extracts"),
                "{\"a\":1}",
                "stop_reason {stop_reason} must not block extraction"
            );
        }
    }

    /// Thinking blocks precede the answer when a model emits them. They are
    /// skipped, never parsed — the first *text* block is the answer.
    #[test]
    fn a_thinking_block_before_the_answer_is_skipped() {
        let env = envelope(serde_json::json!({
            "stop_reason": "end_turn",
            "content": [
                { "type": "thinking", "thinking": "…", "signature": "sig" },
                { "type": "text", "text": "{\"a\":1}" },
                { "type": "text", "text": "trailing" }
            ]
        }));
        assert_eq!(
            output_from_envelope(env, Ceiling::Configured(8000)).expect("extracts"),
            "{\"a\":1}",
            "the first text block carrying text is the answer"
        );
    }

    /// R0004-0018: an **empty** leading text block must not mask an answer
    /// that arrives in a later one. Emptiness is part of finding the answer,
    /// not a verdict on the block that was already chosen — selecting first
    /// and filtering after reported "no non-empty text content block" for a
    /// reply that plainly carried one, which costs a content retry and, on
    /// repeat, falls the unit back to source.
    #[test]
    fn an_empty_leading_text_block_does_not_mask_a_later_answer() {
        for content in [
            serde_json::json!([
                { "type": "text", "text": "" },
                { "type": "text", "text": "{\"a\":1}" }
            ]),
            // …and with the provider's other leading-block shapes mixed in:
            // a thinking block, a text block whose `text` is absent
            // altogether, and more than one empty.
            serde_json::json!([
                { "type": "thinking", "thinking": "…", "signature": "sig" },
                { "type": "text" },
                { "type": "text", "text": "" },
                { "type": "text", "text": "{\"a\":1}" },
                { "type": "text", "text": "trailing" }
            ]),
        ] {
            let env = envelope(serde_json::json!({
                "stop_reason": "end_turn",
                "content": content.clone(),
            }));
            assert_eq!(
                output_from_envelope(env, Ceiling::Configured(8000))
                    .unwrap_or_else(|e| panic!("{content} must extract, got {e:?}")),
                "{\"a\":1}",
                "an empty leading block must be skipped, not treated as the answer: {content}"
            );
        }
    }

    /// A body with no usable text — no blocks at all, only non-text blocks,
    /// or an empty text block — is malformed, and says so rather than
    /// borrowing a stop reason's diagnosis.
    #[test]
    fn a_reply_with_no_usable_text_is_malformed() {
        for shape in [
            serde_json::json!({ "stop_reason": "end_turn", "content": [] }),
            serde_json::json!({
                "stop_reason": "end_turn",
                "content": [{ "type": "thinking", "thinking": "…" }]
            }),
            serde_json::json!({
                "stop_reason": "end_turn",
                "content": [{ "type": "text", "text": "" }]
            }),
            serde_json::json!({ "stop_reason": "end_turn" }),
        ] {
            let err = output_from_envelope(envelope(shape.clone()), Ceiling::Configured(8000))
                .expect_err("no usable text is an error");
            assert!(
                matches!(&err, ProviderError::Malformed(s) if s.contains("text content block")),
                "{shape}: got {err:?}"
            );
        }
    }

    /// Bytes that are not an envelope at all are malformed, and the reader
    /// says which envelope it failed to parse.
    #[test]
    fn unparseable_bytes_are_malformed() {
        let err = extract_output(b"not json", Ceiling::Configured(8000))
            .expect_err("garbage is an error");
        assert!(
            matches!(&err, TranslatorError::MalformedResponse(s)
                if s.contains("could not parse Messages envelope")),
            "got {err:?}"
        );
    }
}
