//! Layered validation orchestrator.
//!
//! [`validate_batch`] owns the per-batch layers, in order: schema
//! (ID-set equality, plus the payload-byte rule the JSON schema cannot
//! express — no NUL, ti d06c43) → per-kind shape → fragment reparse →
//! inline protection (link/image destinations and policy-gated inline
//! code spans; EXT-2026-07 P1-5, ADR-0012 amendment). The final layer —
//! the full-document reparse — is the pipeline's final gate: it runs
//! once after regeneration (see [`full_reparse`] and
//! `pipeline::finalize::finalize_regen_with_reparse_policy`), not per batch. See
//! `docs/architecture/contracts.md` §5.
//!
//! TRACE: SCN-07
//! TRACE: SCN-14

pub mod fragment_reparse;
pub mod full_reparse;
pub mod inline;
pub mod per_kind;
pub mod schema;

use crate::FallbackStatus;
use crate::id::BlockId;
use crate::llm::{GlossaryEntry, OutputKind, TranslationBatch, TranslationBatchResult, UnitResult};
use serde::Serialize;
use std::collections::HashMap;

/// Result of validating one batch — what the regenerator will splice in.
///
/// `units` is **total over the requested units**: every unit the batch
/// asked for appears exactly once, even when the provider dropped or
/// duplicated its row (OI-0031). Result rows for ids that were never
/// requested are discarded and named in [`ValidatedBatch::batch_fault`].
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Default)]
pub struct ValidatedBatch {
    // OI-0027: the former `batch_id_str` field was dropped — with `validate`
    // crate-private the compiler could see nothing ever read it. Callers that
    // need the batch id have the `TranslationBatch` they passed in.
    pub units: Vec<ValidatedUnit>,
    /// OI-0031: the batch-level schema fault observed on this attempt, if
    /// any. Carries the attribution the pipeline needs to charge the
    /// *per-batch* schema budget instead of the offenders' per-unit
    /// content-retry budgets.
    pub batch_fault: Option<BatchFault>,
}

/// Batch-level schema fault: the attribution that routes budget charging
/// (per-batch, not per-unit).
///
/// A schema fault is a statement about the provider's *envelope*, not
/// about any unit's content: a dropped row means the unit was never
/// judged. Charging it to that unit's content-retry budget would conflate
/// "the provider can't format this batch" with "the provider can't
/// translate this block", and would leave a twice-dropped unit with fewer
/// real content attempts than a unit that was merely mistranslated.
///
/// TRACE: SCN-07
/// TRACE: OI-0031
#[derive(Debug, Clone)]
pub struct BatchFault {
    /// Human-readable summary composed from the classification
    /// (`schema::SchemaClassification::summary`).
    pub reason: String,
    /// Requested units to re-dispatch **without** a per-unit charge:
    /// missing from the result, or returned more than once (every copy
    /// discarded). Sorted by id string.
    pub offenders: Vec<BlockId>,
    /// Discarded result ids that were never requested. Charged to nobody —
    /// no requested unit is at fault and there is nothing to re-dispatch.
    pub foreign_ids: Vec<String>,
    /// Caller-side request malformation (duplicate ids in the *request*).
    /// The pipeline preflights this and aborts before dispatching, so it
    /// is only reachable through a direct `validate_batch` call; that
    /// defensive path still rejects every unit rather than guessing.
    pub malformed_request: bool,
}

/// Per-unit acceptance/rejection decision after the layered validators ran.
///
/// `rejected_by` carries the layer that rejected the attempt; the pipeline
/// uses it to decide whether to retry. `final_status` reports the
/// *current attempt's* outcome — the orchestration layer may flip this
/// to `Translated` after a successful retry.
///
/// TRACE: SCN-08
#[derive(Debug, Clone)]
pub struct ValidatedUnit {
    pub unit_id: BlockId,
    pub final_status: FallbackStatus,
    pub accepted_payload: Option<String>,
    pub rejected_by: Option<ValidationLayer>,
    pub rejection_reason: Option<String>,
    /// Provider-side warnings forwarded from `UnitResult.warnings`. Empty
    /// for accepted-without-warnings units; non-empty when the provider
    /// flagged a soft issue (truncation hint, partial language detection,
    /// rate-limit warning, etc.) that didn't fail the request.
    ///
    /// **Bounded** ([`bounded_warnings`], R0002-0068): this is
    /// provider-authored text that reaches the validation-report JSON and
    /// the cache verbatim, so it is capped in both count and per-message
    /// bytes on the way in.
    pub warnings: Vec<String>,
}

/// Run the layered validators against one batch result.
///
/// Phase ownership of the layers, in order:
/// - Schema (ID set equality; per-unit payload bytes — no NUL, ti d06c43) — SL-01
/// - Per-kind shape                  — SL-02..SL-06 (table cols, list topology, fence info, heading level, blockquote children)
/// - Fragment reparse                 — SL-02..SL-06 (reparse the unit payload as the same kind)
/// - Inline protection                — EXT-2026-07 P1-5 (link/image destination identity, including reference-style links resolved via `ref_defs`, + policy-gated inline code-span identity; ADR-0012 amendment)
///
/// Full-document reparse runs once at the end of the pipeline (SL-14).
///
/// `ref_defs` is the document's link-reference-definition pool
/// ([`crate::parser::refdefs`]), forwarded to the inline layer so
/// reference-style links resolve to real destinations (design D2 §B4). Pass
/// `""` when the document defines none.
///
/// **Schema faults do not taint batch-mates (OI-0031).** A dropped,
/// duplicated, or invented result row is decomposed per unit: units with
/// exactly one returned row are *innocent* and flow through the per-unit
/// layers exactly as if the batch had been clean, because every content
/// and structure property is still independently proven for them. Only the
/// implicated units are rejected, and the fault itself is reported on
/// [`ValidatedBatch::batch_fault`] so the pipeline can charge it to the
/// per-batch schema budget.
///
/// TRACE: SCN-07
/// TRACE: OI-0031
pub fn validate_batch(
    batch: &TranslationBatch,
    result: &TranslationBatchResult,
    ref_defs: &str,
) -> ValidatedBatch {
    let cls = schema::classify(batch, result);

    // A malformed *request* cannot be salvaged or re-dispatched (an
    // identical resubmission is futile by construction), so keep the
    // pre-OI-0031 defensive behavior: reject every unit with the legacy
    // diagnostic. `run_pipeline` never reaches this — `process_one_batch`
    // preflights request-id uniqueness and aborts loudly instead.
    if !cls.request_duplicates.is_empty() {
        let unique = batch
            .units
            .iter()
            .map(|u| &u.unit_id)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let reason = schema::request_duplicate_message(batch.units.len(), unique);
        return ValidatedBatch {
            units: batch
                .units
                .iter()
                .map(|u| ValidatedUnit {
                    unit_id: u.unit_id.clone(),
                    final_status: FallbackStatus::FallbackSource,
                    accepted_payload: None,
                    rejected_by: Some(ValidationLayer::Schema),
                    rejection_reason: Some(reason.clone()),
                    warnings: Vec::new(),
                })
                .collect(),
            batch_fault: Some(BatchFault {
                reason,
                offenders: Vec::new(),
                foreign_ids: cls.foreign.clone(),
                malformed_request: true,
            }),
        };
    }

    let unit_kind_lookup: HashMap<&BlockId, &crate::llm::TranslationUnit> =
        batch.units.iter().map(|u| (&u.unit_id, u)).collect();
    // Index the result rows once: the salvage path needs "how many rows did
    // this requested id get?" per unit, and a per-unit scan would be
    // O(N*M) on large batches (R0006-0051's lesson).
    let mut result_rows: HashMap<&BlockId, Vec<&UnitResult>> = HashMap::new();
    for r in &result.units {
        result_rows.entry(&r.unit_id).or_default().push(r);
    }

    // EXT-2026-07 P1-5: the inline-protection layer is gated on the
    // profile's `[constraints]` — except the raw-inline-HTML tag guard,
    // which is always-on (spec 2026-08-03 §4.3); the batch already
    // carries them.
    let policy = &batch.profile.constraints;
    // Request order, so `units` is total over the requested set and
    // independent of however the provider ordered (or mangled) its reply.
    let units: Vec<ValidatedUnit> = batch
        .units
        .iter()
        .map(|u| match result_rows.get(&u.unit_id).map(Vec::as_slice) {
            Some([r]) => validate_unit(&unit_kind_lookup, policy, r, ref_defs),
            // Offenders: the provider dropped the row, or returned several
            // and left no principled way to pick one. Either way this
            // unit's *content* was never judged — the pipeline re-dispatches
            // it on the per-batch budget, and finalizes it as fallback only
            // once that budget is spent.
            Some(_) => offender_unit(u, "duplicated in provider result (batch schema fault)"),
            None => offender_unit(u, "missing from provider result (batch schema fault)"),
        })
        .collect();

    let batch_fault = (!cls.is_clean()).then(|| BatchFault {
        reason: cls.summary(),
        offenders: cls.offenders().cloned().collect(),
        foreign_ids: cls.foreign.clone(),
        malformed_request: false,
    });

    ValidatedBatch { units, batch_fault }
}

/// A requested unit implicated by a batch-level schema fault: rejected at
/// the `Schema` layer with no payload, so the pipeline either re-dispatches
/// it on the per-batch budget or finalizes it as an honest
/// `fallback_source`. OI-0031.
fn offender_unit(unit: &crate::llm::TranslationUnit, reason: &str) -> ValidatedUnit {
    ValidatedUnit {
        unit_id: unit.unit_id.clone(),
        final_status: FallbackStatus::FallbackSource,
        accepted_payload: None,
        rejected_by: Some(ValidationLayer::Schema),
        rejection_reason: Some(reason.to_string()),
        warnings: Vec::new(),
    }
}

/// Bound a provider diagnostic before it lands in the validation report
/// (a hostile or misbehaving provider can produce an arbitrarily long
/// message). Char-safe, mirroring the provider client's own cap
/// (`transync-openai`'s `client::classify::truncate_diagnostic`).
/// OI-0026; R0008-0034.
///
/// R0003-0046: `MAX` bounds the retained **prefix**. A truncated message is
/// that prefix plus a short `"… (N bytes total, truncated)"` marker, so the
/// emitted string runs slightly past `MAX` — by design, since the marker is
/// what distinguishes a cut message from a short one.
pub(crate) fn truncate_diagnostic(text: &str) -> String {
    /// Bytes of the original message kept; see the truncation marker note above.
    const MAX: usize = 512;
    if text.len() <= MAX {
        return text.to_string();
    }
    let mut end = MAX;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… ({} bytes total, truncated)", &text[..end], text.len())
}

/// How many provider warnings one unit may carry into the report.
///
/// R0002-0068. Generous next to any honest provider (a soft issue is one
/// or two sentences about one block), and the count is what a hostile one
/// would otherwise scale without limit — the per-message cap alone bounds
/// each string, not the vector.
const MAX_UNIT_WARNINGS: usize = 8;

/// Bound the provider-authored warnings of one unit, in both directions
/// a provider controls: [`MAX_UNIT_WARNINGS`] messages, each capped by
/// [`truncate_diagnostic`].
///
/// R0002-0068: `warnings` is model-authored text (a schema field, so
/// invariant 7's untrusted source can shape it) that every validation arm
/// forwards verbatim into `ValidationReport`'s JSON artifact and into the
/// cached `UnitResult`. It had no cap of its own — only the provider's
/// response ceiling — while the sibling provider-authored strings
/// (`rejection_reason`, the retry `reason`, the auto-glossary diagnostic)
/// have been capped since R0008-0034. This closes the last one.
///
/// The overflow is *named*, not silently dropped: a trailing line reports
/// how many messages were discarded, so a report reader can tell a
/// two-warning unit from a two-thousand-warning one.
pub(crate) fn bounded_warnings(warnings: &[String]) -> Vec<String> {
    let mut out: Vec<String> = warnings
        .iter()
        .take(MAX_UNIT_WARNINGS)
        .map(|w| truncate_diagnostic(w))
        .collect();
    if warnings.len() > MAX_UNIT_WARNINGS {
        out.push(format!(
            "… ({} further provider warnings dropped; {} total)",
            warnings.len() - MAX_UNIT_WARNINGS,
            warnings.len()
        ));
    }
    out
}

fn validate_unit(
    lookup: &HashMap<&BlockId, &crate::llm::TranslationUnit>,
    policy: &crate::profile::ProfileConstraints,
    result: &UnitResult,
    ref_defs: &str,
) -> ValidatedUnit {
    // One bounding, at the door: every arm below forwards these, and the
    // report and the cache both keep what they forward (R0002-0068).
    let warnings = bounded_warnings(&result.warnings);

    // R0002-0081: ONE rejection shape for every layer below. A rejected
    // attempt is always the same statement — this unit, fallback status, no
    // payload handed on, the provider's warnings forwarded — and only the
    // layer and the reason differ. The struct literal used to be written out
    // once per layer, which the compiler checks for field *presence* but not
    // for field *values*: a seventh arm that quietly kept `accepted_payload`
    // or claimed `Translated` would have compiled. Now there is nothing to
    // keep in step.
    let reject = |layer: ValidationLayer, reason: String| ValidatedUnit {
        unit_id: result.unit_id.clone(),
        final_status: FallbackStatus::FallbackSource,
        accepted_payload: None,
        rejected_by: Some(layer),
        rejection_reason: Some(reason),
        warnings: warnings.clone(),
    };

    let unit = match lookup.get(&result.unit_id) {
        Some(u) => u,
        None => {
            // Defensive only: since OI-0031, `validate_batch` calls this
            // exclusively for *requested* units, so a foreign row never
            // reaches here (it is discarded and named in
            // `BatchFault::foreign_ids`). Kept total rather than panicking,
            // for any future caller that re-enters this seam.
            return reject(ValidationLayer::Schema, "unknown unit_id in result".into());
        }
    };

    let initial_status = match result.output_kind {
        OutputKind::Translated => FallbackStatus::Translated,
        OutputKind::Preserved => FallbackStatus::Preserved,
        OutputKind::PartiallyTranslated => FallbackStatus::PartiallyTranslated,
        OutputKind::FailedNeedsFallback => FallbackStatus::FallbackSource,
    };

    if matches!(initial_status, FallbackStatus::FallbackSource) {
        // Provider explicitly opted out of this unit. Mark the attempt
        // as a Provider-layer rejection so the pipeline's retry budget
        // applies — a deterministic provider stays at fallback after
        // each retry, but a non-deterministic one may recover, and
        // either way we never retry past max_per_unit_validation_retries.
        return reject(
            ValidationLayer::Provider,
            "provider returned FailedNeedsFallback".into(),
        );
    }

    // ti d06c43: the one byte `out.md` never carries. Ahead of every content
    // layer because the shape layers cannot see it — they read the payload
    // through comrak, which substitutes U+FFFD before a node exists
    // (CommonMark §2.3) — and because the `Preserved` proof below is the one
    // check that would notice, and would blame the wrong thing.
    if let Err(reason) = schema::check_payload_bytes(&unit.block_kind, &result.translated_payload) {
        return reject(ValidationLayer::Schema, reason);
    }

    // EXT-2026-07 P2-9 (OI-0022): "preserved" is a claim the provider
    // returned the source unchanged. Prove it — a byte-for-byte mismatch
    // means the provider mutated content while asserting it did not, which
    // silently corrupts the output. Reject it like any other per-kind
    // failure so the retry budget applies; a deterministic provider stays
    // rejected and falls back to source, never emitting the mismatched
    // payload as if it were an untouched original.
    if matches!(result.output_kind, OutputKind::Preserved)
        && result.translated_payload != unit.source_payload
    {
        return reject(
            ValidationLayer::PerKindShape,
            "output_kind=preserved but payload differs from source".into(),
        );
    }

    if let Err(reason) = per_kind::check(&unit.constraints, &unit.block_kind, result) {
        return reject(ValidationLayer::PerKindShape, reason);
    }
    if let Err(reason) = fragment_reparse::reparse_fragment(&unit.block_kind, result) {
        return reject(ValidationLayer::FragmentReparse, reason);
    }
    // EXT-2026-07 P1-5: link/image destinations, policy-gated inline code
    // spans, and always-on raw inline HTML tags are operational metadata
    // (ADR-0012 amendment). Runs after fragment reparse — the most
    // expensive per-unit layer (two arena parses), and its diagnostics
    // assume a payload that already parsed as the expected kind. A count
    // differ (model added/dropped a link) is a retryable rejection like any
    // other; the pipeline retries verbatim and falls back to source after
    // the budget is spent.
    if let Err(reason) = inline::check_inline(policy, unit, result, ref_defs) {
        return reject(ValidationLayer::Inline, reason);
    }

    // Spec 2026-08-03 §4.2 layer 3: run the real splice as a check, so a
    // block that cannot be reassembled is never accepted. A failure here is
    // an ENGINE fault, not a model fault — the payload already passed the
    // shape layers — so it degrades straight to fallback (§3.3) with
    // `rejected_by: None`: the pipeline's retry loop keys on
    // `rejected_by.is_some()`, and an identical resubmission cannot fix a
    // rewriter that could not process this block. The cause rides in
    // `warnings`, which the report surfaces for fallback units, and the
    // `FallbackSource` status keeps the payload out of the cache.
    //
    // The check's input is `constraints.html.source_bytes` (the
    // UTF-8-snapped block payload), while regen splices the raw
    // `source_range` slice. A boundary-weird divergence between the two
    // therefore cannot make this layer lie about regen's success — it
    // surfaces as regen's own defensive source-bytes fallback, which keeps
    // `out.md` honest rather than corrupt.
    if let Some(h) = &unit.constraints.html {
        // Deliberately NOT the `reject` shape above: this is the one arm
        // that is not a rejection at all — no layer, no reason, so the
        // pipeline's `rejected_by.is_some()` retry loop skips it (§3.3).
        // Provider warnings are kept (every other layer forwards them) and
        // the cause is appended, so a truncation hint that explains the
        // fault is not lost on the way to the report.
        let direct_fallback = |msg: String| {
            // The engine's own cause is appended *after* the bound, so a
            // provider that filled the vector cannot push it out.
            let mut warnings = warnings.clone();
            warnings.push(msg);
            ValidatedUnit {
                // Provably equal to `unit.unit_id` (the ID layer ran first);
                // sourced from `result` for consistency with every sibling arm.
                unit_id: result.unit_id.clone(),
                final_status: FallbackStatus::FallbackSource,
                accepted_payload: None,
                rejected_by: None,
                rejection_reason: None,
                warnings,
            }
        };
        match serde_json::from_str::<Vec<String>>(&result.translated_payload) {
            // `per_kind::check_html` already rejected malformed JSON; defensive.
            Err(e) => {
                return direct_fallback(format!("html splice precheck: payload unparsable: {e}"));
            }
            Ok(segs) => match transync_html::splice(
                &h.source_bytes,
                &segs,
                transync_html::BlankLinePolicy::from_commonmark_html_block_type(h.block_type),
            ) {
                Err(e) => return direct_fallback(format!("html splice failed: {e}")),
                Ok(spliced) => {
                    if transync_html::tag_inventory(&spliced)
                        != transync_html::tag_inventory(&h.source_bytes)
                    {
                        return direct_fallback(
                            "html splice check: tag inventory diverged from source (engine fault)"
                                .to_string(),
                        );
                    }
                }
            },
        }
    }

    ValidatedUnit {
        unit_id: result.unit_id.clone(),
        final_status: initial_status,
        accepted_payload: Some(result.translated_payload.clone()),
        rejected_by: None,
        rejection_reason: None,
        warnings,
    }
}

/// Per-attempt diagnostic returned to the caller in the [`ValidationReport`].
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Serialize)]
pub struct AttemptOutcome {
    pub attempt_number: u32,
    pub rejected_by: Option<ValidationLayer>,
    pub rejection_reason: Option<String>,
    /// OI-0031: this attempt failed because of a batch-level schema fault
    /// (the provider dropped or duplicated this unit's row), not because of
    /// anything about this unit's own output — so it was charged to the
    /// per-batch schema budget
    /// ([`crate::TranslateOptions::max_per_batch_schema_retries`]), leaving
    /// the unit's content-retry budget intact.
    ///
    /// Skipped when `false` so every pre-OI-0031 row serializes
    /// byte-identically (this type is `Serialize`-only).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub batch_fault: bool,
}

/// Per-unit attempt log.
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Serialize)]
pub struct UnitValidationRecord {
    pub unit_id: BlockId,
    pub attempts: Vec<AttemptOutcome>,
    pub final_status: FallbackStatus,
    /// Provider-side warnings attached to the accepted attempt
    /// (forwarded from `ValidatedUnit::warnings`). R0006-0026.
    pub warnings: Vec<String>,
}

/// Schema version of the durable **validation-report JSON artifact** — the
/// object the CLI publishes as the root of `--validation-report <path>` and
/// of `--out-dir`'s `validation-report.json` (contracts.md §3a).
///
/// Semver, read exactly like `align::ALIGNMENT_SCHEMA_VERSION`: patch bumps
/// add fields, minor bumps may add enumerated values or rename an optional
/// field behind an alias, major bumps are breaking. Consumers MUST reject an
/// unknown major and MUST accept an unknown minor/patch of the same major.
///
/// **Not** `VALIDATION_SCHEMA_VERSION`. That one is a `CacheKey` axis over
/// the prompt/payload contract and bumps for reasons — prompt framing,
/// `InputMode` payload semantics, `UnitResult` field semantics — that leave
/// this artifact's shape untouched; conversely, re-versioning this artifact
/// orphans no cache entry. The two constants are independent by design and
/// neither implies the other.
///
/// Curated facade surface, tier (a), as
/// `transync::VALIDATION_REPORT_SCHEMA_VERSION` (contracts.md §0) — symmetric
/// with `align::ALIGNMENT_SCHEMA_VERSION`, which the same table already
/// carries. The two say different things and a consumer needs both: **this
/// constant is what the linked build emits**, so a Rust caller can branch on it
/// at compile time, while the **serialized `schema_version` field is what a
/// given file declares**, which is the only honest discriminator for a report
/// read off disk (possibly written by another version). Reading the constant to
/// interpret someone else's artifact is the one misuse to avoid.
///
/// History:
/// - 1.0.0 (2026-08): the first versioned report (R0001-0027). A report with
///   no `schema_version` key at all predates this and reads as "pre-1.0.0".
/// - 1.1.0 (2026-08, DCR-0026): additive — `per_unit` can now carry rows for
///   **table row windows** as well as blocks. A window row's `unit_id` is
///   `<parent-block-id>.wNN`, it lists directly under its parent's row in the
///   document ordering, and its attempts and retries are its own. Every field
///   keeps its meaning and no key was added or removed, so a 1.0.0 reader
///   parses a 1.1.0 report; what it must not assume any more is that every
///   `unit_id` names a block in the alignment map (contracts.md §3a).
pub const VALIDATION_REPORT_SCHEMA_VERSION: &str = "1.1.0";

/// Caller-facing validation report (returned in [`crate::TranslationOutput`]).
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct — new fields may be added in minor releases.
///
/// As a durable JSON artifact it is versioned by its own `schema_version`
/// field (contracts.md §3a); `Default` stamps the current version, so every
/// report the engine composes carries it.
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct ValidationReport {
    /// Schema version of this report **as a durable JSON artifact**
    /// (contracts.md §3a). Semver: consumers MUST reject an unknown major
    /// and MUST accept an unknown minor/patch of the same major. Distinct
    /// from the `VALIDATION_SCHEMA_VERSION` cache-key axis, which versions
    /// the prompt/payload contract rather than this artifact's shape.
    ///
    /// R0001-0027
    pub schema_version: String,
    /// Per-unit attempt log, in **document (source) order** — the order
    /// `crate::parser::Document::blocks` traverses, not the order the
    /// `unit_id` strings sort in (OI-0021 item 2).
    pub per_unit: Vec<UnitValidationRecord>,
    /// Re-dispatches of a unit's **own content** across all batches — the
    /// per-unit validation-retry budget's consumption
    /// ([`crate::TranslateOptions::max_per_unit_validation_retries`]).
    ///
    /// A unit's first content attempt is not a retry, and two other kinds of
    /// attempt row are excluded because they are not content re-dispatches:
    /// batch-envelope faults (counted by `batch_schema_faults`) and the
    /// `attempt_number: 0` cache-hit re-validation row, which charges no
    /// budget (ADR-0015).
    pub total_retries: u32,
    pub total_fallbacks: u32,
    /// Units the post-regen full-document reparse downgraded to
    /// `FallbackSource` (DCR-0004 cascade). These downgrades happen
    /// after the per-attempt log is closed, so they are reported here
    /// rather than as synthetic [`AttemptOutcome`] rows. R0006-0006.
    ///
    /// In **document (source) order**, the same order `per_unit` uses —
    /// not the order the `unit_id` strings sort in (R0001-0028).
    pub full_reparse_fallbacks: Vec<BlockId>,
    /// Transient provider-transport retries (network / rate-limit)
    /// across all batches. Distinct from `total_retries`, which counts
    /// validation-rejection retries. R0006-0024.
    ///
    /// The honest observed total, bounded by
    /// `max_per_batch_provider_retries × input batch count`: that budget is
    /// charged to the whole batch, so a batch's retry ladder cannot refund it
    /// by opening another dispatch round (ti 294dda).
    pub provider_retries: u32,
    /// OI-0031: batch-level schema faults charged across all batches — one
    /// per round in which the provider mangled the response envelope
    /// (dropped or duplicated rows) and the implicated units were
    /// re-dispatched on the per-batch budget. Distinct from `total_retries`:
    /// these rounds cost the offenders' batch budget, not their per-unit
    /// content-retry budget, and they never consume an innocent
    /// batch-mate's. Bounded by
    /// `max_per_batch_schema_retries × batch count`.
    #[serde(default)]
    pub batch_schema_faults: u32,
    /// Notes about top-level source nodes the pipeline does not model as a
    /// translatable kind (footnote definitions, front matter, …). They
    /// are preserved verbatim in the translated Markdown and rendered as
    /// inert, escaped placeholders anchored in both panes (invariant 7);
    /// forwarded from [`crate::parser::Document::warnings`] so the handling
    /// is surfaced rather than silent (R0008-0013). Because it mirrors
    /// `Document::warnings` wholesale, other parser-level source notes ride
    /// along too (e.g. the refdefs "unattributed source text between blocks"
    /// backstop).
    ///
    /// Spec 2026-08-03 §3.2 added a second, unit-construction-time producer on
    /// the same channel: an html block that yielded no translatable segment,
    /// or whose segment extraction failed, appends a note here. Ticket
    /// `13e145` added a third, and the first that is about the **document**
    /// rather than one of its nodes: a run whose translatable mass is
    /// overwhelmingly raw HTML says so, because handing an HTML document to a
    /// Markdown pipeline corrupts it quietly rather than loudly
    /// (`unit::html_dominance_warning` carries the argument). Order is
    /// parser notes, then that document-level note, then the html notes in id
    /// order (contracts.md §3a). Name unchanged and wire-compatible through
    /// all three.
    #[serde(default)]
    pub skipped_source_nodes: Vec<String>,
    /// Units whose estimated response size exceeds the per-batch output
    /// ceiling (D1 §2.3). A prevention/diagnosis signal, never an abort:
    /// the provider may still succeed, but if it truncates, the run aborts —
    /// this names the at-risk block(s) up front. Empty when the profile
    /// leaves `[batching].target_output_tokens` unset.
    #[serde(default)]
    pub output_budget_warnings: Vec<OutputBudgetWarning>,
    /// OI-0026: outcome of the auto-glossary extraction preflight.
    /// `None` — and absent from the serialized report — when the feature
    /// is off, so an opted-out run's report carries no `auto_glossary` key
    /// at all and the field was purely additive on the wire when it landed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_glossary: Option<AutoGlossaryReport>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self {
            schema_version: VALIDATION_REPORT_SCHEMA_VERSION.to_string(),
            per_unit: Vec::new(),
            total_retries: 0,
            total_fallbacks: 0,
            full_reparse_fallbacks: Vec::new(),
            provider_retries: 0,
            batch_schema_faults: 0,
            skipped_source_nodes: Vec::new(),
            output_budget_warnings: Vec::new(),
            auto_glossary: None,
        }
    }
}

/// A unit whose estimated response size exceeds the per-batch output ceiling
/// (D1 §2.3). Estimation is heuristic and the provider may still succeed, so
/// this is a WARNING, never an abort — the only remaining over-ceiling shape
/// after output-aware packing is a single unit whose own `est_out` exceeds
/// the target (it gets its own batch and may terminally abort the run at the
/// provider).
///
/// TRACE: SCN-10
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputBudgetWarning {
    pub unit_id: BlockId,
    pub estimated_output_tokens: u32,
    /// The raw `target_output_tokens`, not the reserved effective target.
    pub ceiling_tokens: u32,
}

impl std::fmt::Display for OutputBudgetWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unit {} estimated output ~{} tokens exceeds the per-batch output ceiling of {} \
             (profile [batching] target_output_tokens); the provider may truncate the response \
             and abort the run",
            self.unit_id, self.estimated_output_tokens, self.ceiling_tokens
        )
    }
}

/// OI-0026: outcome of the auto-glossary preflight. `None` on
/// [`ValidationReport::auto_glossary`] when the feature is disabled.
///
/// TRACE: SCN-09
/// TRACE: OI-0026
#[derive(Debug, Clone, Serialize)]
pub struct AutoGlossaryReport {
    pub status: AutoGlossaryStatus,
    /// Extracted entries merged into the run's glossary.
    pub accepted_terms: u32,
    /// Extracted entries dropped because a static profile entry already
    /// pinned the same source term (static wins).
    pub dropped_conflicts: u32,
    /// Extracted entries dropped as empty, oversize, duplicated within the
    /// extraction, or past the term cap.
    pub dropped_invalid: u32,
    /// Whether the source excerpt sent to the extractor was truncated at
    /// [`crate::llm::MAX_EXTRACTION_SOURCE_BYTES`] (a partial harvest).
    pub source_truncated: bool,
    /// 512-byte-truncated provider diagnostic when
    /// `status == AutoGlossaryStatus::Failed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The merged-in entries, for full provenance in report consumers.
    pub terms: Vec<GlossaryEntry>,
}

/// Why [`AutoGlossaryReport`] looks the way it does.
///
/// `Unsupported` and `Failed` are deliberately distinct: the first means
/// the translator does not implement
/// [`crate::llm::Translator::extract_glossary`] at all, the second that it
/// tried and errored. Both degrade to the static glossary — and therefore
/// to the same cache identity as a disabled run — but only `Failed` is a
/// symptom worth chasing.
///
/// They also differ in how they are announced, on purpose (ti 33e178).
/// `Unsupported` is named on the `transync::pipeline` `tracing` target;
/// `Failed` is named on **this report and nowhere else** — it is the one
/// degradation the library raises no `tracing` record for, because the
/// reference CLI prints it unconditionally and a record would be a second
/// copy of the same sentence. A consumer that wants the failure reads
/// [`AutoGlossaryReport::status`] and [`AutoGlossaryReport::error`].
///
/// TRACE: OI-0026
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoGlossaryStatus {
    /// The extractor ran and its harvest was merged (possibly empty).
    Extracted,
    /// The translator does not implement extraction (`Ok(None)`).
    Unsupported,
    /// The extractor errored; the run proceeded on the static glossary.
    Failed,
    /// Enabled, but the document had no translatable block, so no call
    /// was made.
    Skipped,
}

/// Which layer rejected an attempt.
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLayer {
    /// The response envelope, and the payload bytes the envelope's JSON
    /// schema cannot police. Two shapes reach a report row through this
    /// variant, and [`AttemptOutcome::batch_fault`] is the discriminator:
    /// a **batch-envelope** fault (a requested unit's row dropped or
    /// returned more than once) sets it, and a **per-unit payload-byte**
    /// rejection — today exactly one rule, a NUL (`U+0000`), which a JSON
    /// string may legally hold and `out.md` may not (ti d06c43) — does not.
    Schema,
    /// Reserved: ID-set equality is enforced inside the `Schema` layer
    /// (`schema::classify`), so no attempt is tagged `IdSet` today.
    /// Attribution for an ID-set fault rides on [`BatchFault`] and
    /// [`AttemptOutcome::batch_fault`] instead of a distinct layer, so
    /// existing consumers keep matching on `Schema`. R0006-0041, OI-0031.
    IdSet,
    PerKindShape,
    FragmentReparse,
    /// Inline-protection layer: link/image destination identity and
    /// policy-gated inline code-span identity (EXT-2026-07 P1-5;
    /// ADR-0012 amendment). Serialized as "inline".
    Inline,
    /// Reserved: the full-document reparse runs once at pipeline end
    /// and reports through `ValidationReport.full_reparse_fallbacks`,
    /// not through per-attempt outcomes. R0006-0041.
    FullReparse,
    /// Provider opted the unit out via `OutputKind::FailedNeedsFallback`.
    /// Treated as a retryable rejection so the per-unit retry budget
    /// applies — see `validate_unit` for the rationale.
    ///
    /// TRACE: SCN-08
    Provider,
}

impl ValidationLayer {
    /// Stable wire label for this layer, mirroring `BlockKind::wire_str`.
    /// MUST equal the serde `snake_case` rename of each variant — pinned
    /// by `wire_str_matches_serde` so the two never drift. Used by the
    /// provider client to frame a re-dispatched unit's `retry` hint.
    ///
    /// EXT-2026-07 P0-2
    pub fn wire_str(self) -> &'static str {
        match self {
            Self::Schema => "schema",
            Self::IdSet => "id_set",
            Self::PerKindShape => "per_kind_shape",
            Self::FragmentReparse => "fragment_reparse",
            Self::Inline => "inline",
            Self::FullReparse => "full_reparse",
            Self::Provider => "provider",
        }
    }
}

/// Generation counter for everything a cached `UnitResult`'s validity
/// depends on that the layered validators cannot re-check on a hit:
/// the provider prompt contract (instruction text, injection framing,
/// hint fields), `InputMode` payload semantics, and `UnitResult` wire
/// semantics. Participates in `CacheKey`, so a bump orphans (never
/// corrupts) old entries.
///
/// Bump when:
/// - the user-prompt instruction/framing changes in a way that alters
///   what a trustworthy result looks like (e.g. injection-guard fixes);
/// - an `InputMode`'s payload meaning changes (e.g. the FullCodeBlock
///   content-only migration, `R0001-0026` in the removed
///   `reviews/reviewed/0001.md`);
/// - `UnitResult` field semantics change.
///
/// Do NOT bump for validator tightening alone — cache hits are
/// re-validated through `validate_batch` on every run (ADR-0015), so
/// per-kind/schema tightening already rejects stale entries.
///
/// No eviction is needed on a bump — a new version yields a new key — but
/// a bump is not free. Under [`crate::cache::InMemoryCache`] the orphans
/// die with the process; under [`crate::cache::DiskCache`] (`transync
/// translate --cache-dir`, DCR-0028 / ADR-0021) they are durable,
/// already-paid-for provider output in an operator's cache directory, and
/// nothing recognizes them as orphans — they stay *live* entries until the
/// capacity budget drops them oldest-written-first. Price a borderline bump
/// as re-buying every affected entry from the provider, in every cache
/// directory that exists.
///
/// This is **not** the validation-report artifact's version — that is
/// `VALIDATION_REPORT_SCHEMA_VERSION` (contracts.md §3a), an independent
/// axis. A change to the report's JSON shape is none of the three bump
/// triggers above and therefore does not orphan a single cache entry.
///
/// History:
/// - 2 (2026-08): HTML-content translation — new `html_segments` payload
///   semantics, the html prompt instruction, and the always-on inline
///   raw-HTML tag guard (spec 2026-08-03 §6 Cache row). html entries could
///   not pre-exist; the bump guards paragraph-family units against
///   pre-guard cached results. Recorded at the time as costless because the
///   only shipped cache was in-memory; that ceased to hold when `DiskCache`
///   shipped (DCR-0028 / ADR-0021, 2026-08) — see the bump-cost paragraph
///   above.
///
/// EXT-2026-07 P1-4
pub const VALIDATION_SCHEMA_VERSION: u32 = 2;

// EXT-2026-07 P0-2: `ValidationLayer::wire_str` is the hand-written
// mirror of the serde `snake_case` rename; pin them so a variant rename
// on one side can never silently diverge from the other.
#[cfg(test)]
mod wire_str_tests {
    use super::ValidationLayer::*;

    #[test]
    fn wire_str_matches_serde() {
        for layer in [
            Schema,
            IdSet,
            PerKindShape,
            FragmentReparse,
            Inline,
            FullReparse,
            Provider,
        ] {
            let serde = serde_json::to_value(layer).expect("serializes");
            assert_eq!(
                serde_json::Value::String(layer.wire_str().to_string()),
                serde,
                "wire_str disagrees with serde for {layer:?}"
            );
        }
    }
}

// R0001-0027: the validation report is published as the root object of a
// durable JSON artifact, so it declares its own schema version. Before this
// it had none, and a consumer had nothing to branch on.
#[cfg(test)]
mod report_schema_version_tests {
    use super::*;

    /// Every report the engine composes starts from `Default`, so stamping
    /// the version there is what makes the field unconditional.
    #[test]
    fn default_report_stamps_the_current_version() {
        let report = ValidationReport::default();
        assert_eq!(report.schema_version, VALIDATION_REPORT_SCHEMA_VERSION);
    }

    /// A report read off disk is discriminated by its serialized field, not
    /// by the reader's constant (contracts.md §3a) — so the field has to be
    /// there, whatever the constant says.
    #[test]
    fn report_json_declares_its_schema_version() {
        let v = serde_json::to_value(ValidationReport::default()).expect("serializes");
        assert_eq!(
            v["schema_version"], VALIDATION_REPORT_SCHEMA_VERSION,
            "the published report must carry a version discriminator: {v}"
        );
    }

    /// Consumers branch on the major and tolerate unknown minor/patch
    /// (contracts.md §3a), which is only possible if the string is semver.
    #[test]
    fn the_version_is_semver_shaped() {
        let parts: Vec<&str> = VALIDATION_REPORT_SCHEMA_VERSION.split('.').collect();
        assert_eq!(
            parts.len(),
            3,
            "expected major.minor.patch, got {VALIDATION_REPORT_SCHEMA_VERSION:?}"
        );
        assert!(
            parts
                .iter()
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())),
            "every component must be numeric: {VALIDATION_REPORT_SCHEMA_VERSION:?}"
        );
    }
}

// EXT-2026-07 P2-9 (OI-0022): a `Preserved` claim is proven by comparing
// the returned payload against the unit's source.
#[cfg(test)]
mod preserved_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, OutputKind, TranslationUnit, UnitResult,
    };
    use std::collections::HashMap;

    fn paragraph_unit(source: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: source.to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn preserved_result(payload: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId("p-0001".to_string()),
            output_kind: OutputKind::Preserved,
            translated_payload: payload.to_string(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn preserved_matching_source_is_accepted() {
        let u = paragraph_unit("Hello world.");
        let lookup: HashMap<&BlockId, &TranslationUnit> =
            std::iter::once((&u.unit_id, &u)).collect();
        let policy = crate::profile::ProfileConstraints::default();
        let vu = validate_unit(&lookup, &policy, &preserved_result("Hello world."), "");
        assert!(
            vu.rejected_by.is_none(),
            "an untouched payload passes: {vu:?}"
        );
        assert!(matches!(vu.final_status, FallbackStatus::Preserved));
        assert_eq!(vu.accepted_payload.as_deref(), Some("Hello world."));
    }

    #[test]
    fn preserved_mismatch_is_rejected() {
        let u = paragraph_unit("Hello world.");
        let lookup: HashMap<&BlockId, &TranslationUnit> =
            std::iter::once((&u.unit_id, &u)).collect();
        // Provider claims "preserved" but returns different bytes — a
        // silent-corruption vector that must be caught and fall back.
        let policy = crate::profile::ProfileConstraints::default();
        let vu = validate_unit(&lookup, &policy, &preserved_result("안녕하세요."), "");
        assert_eq!(vu.rejected_by, Some(ValidationLayer::PerKindShape));
        assert!(matches!(vu.final_status, FallbackStatus::FallbackSource));
        assert!(vu.accepted_payload.is_none());
        assert!(
            vu.rejection_reason
                .as_deref()
                .unwrap_or_default()
                .contains("preserved but payload differs from source"),
            "reason: {:?}",
            vu.rejection_reason
        );
    }
}

// ti d06c43: the payload-byte rule wired through the full `validate_batch`
// path. `schema::tests` pins the predicate itself; these pin its seat — the
// layer it reports, its precedence over the content layers, and that no
// payload carrying the byte is ever handed to regen.
#[cfg(test)]
mod nul_payload_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, HtmlSegmentConstraints, InputMode,
        TranslationBatch, TranslationBatchResult, TranslationUnit, UnitResult,
    };
    use crate::profile::default_profile;

    const NUL: char = '\0';

    fn batch_of(unit: TranslationUnit) -> TranslationBatch {
        TranslationBatch {
            batch_id: BatchId::new(1),
            units: vec![unit],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        }
    }

    fn paragraph_unit(source: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: source.to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn result_for(
        unit: &TranslationUnit,
        kind: OutputKind,
        payload: &str,
    ) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: vec![UnitResult {
                unit_id: unit.unit_id.clone(),
                output_kind: kind,
                translated_payload: payload.to_string(),
                warnings: Vec::new(),
            }],
        }
    }

    /// The whole contract in one assertion set: the byte rejects, it rejects
    /// at the schema layer with a reason the report can carry, and the
    /// payload is dropped rather than passed on — `accepted_payload: None`
    /// is what keeps it out of regen (which splices only what it is given)
    /// and out of the cache.
    #[test]
    fn a_nul_payload_is_rejected_and_never_handed_on() {
        let unit = paragraph_unit("Hello world.");
        let batch = batch_of(unit.clone());
        let result = result_for(&unit, OutputKind::Translated, &format!("안녕{NUL}하세요."));

        let vb = validate_batch(&batch, &result, "");
        let vu = &vb.units[0];
        assert_eq!(vu.rejected_by, Some(ValidationLayer::Schema));
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
        assert!(
            vu.rejection_reason
                .as_deref()
                .unwrap_or_default()
                .contains("NUL byte (U+0000)"),
            "reason: {:?}",
            vu.rejection_reason
        );
        assert!(
            vb.batch_fault.is_none(),
            "a payload-byte rejection is this unit's own fault, not the \
             envelope's — the batch budget must not be charged"
        );
    }

    /// Precedence, and why it is not cosmetic: `Preserved` is proven by
    /// comparing against the source, and the source cannot hold a NUL
    /// (OI-0034), so a NUL-bearing "preserved" payload also fails that
    /// comparison. Without the byte check running first, the report would
    /// blame the wrong thing and a retry hint would ask the provider to fix
    /// a fidelity problem it does not have.
    #[test]
    fn a_preserved_payload_with_a_nul_is_named_as_the_nul() {
        let unit = paragraph_unit("Hello world.");
        let batch = batch_of(unit.clone());
        let result = result_for(&unit, OutputKind::Preserved, &format!("Hello{NUL} world."));

        let vu = &validate_batch(&batch, &result, "").units[0];
        let reason = vu.rejection_reason.clone().unwrap_or_default();
        assert!(reason.contains("NUL byte (U+0000)"), "reason: {reason}");
        assert!(
            !reason.contains("preserved but payload differs"),
            "the NUL check must win the race with the preserved proof: {reason}"
        );
    }

    /// An html unit's wire payload is a JSON array, so the byte arrives
    /// escaped and only a decode can see it — and regen decodes, so this is
    /// the shape that would otherwise splice a raw NUL into `out.md`.
    #[test]
    fn an_html_payload_whose_segment_decodes_to_a_nul_is_rejected() {
        let source = "<p>one</p>";
        let segs = transync_html::extract(source).expect("source block extracts");
        let mut unit = paragraph_unit(source);
        unit.unit_id = BlockId::new("html", 1);
        unit.block_kind = BlockKind::Html;
        unit.input_mode = InputMode::HtmlSegments;
        unit.source_payload = serde_json::to_string(&segs.texts).expect("json");
        unit.constraints = BlockConstraints {
            html: Some(HtmlSegmentConstraints {
                segment_count: segs.texts.len() as u32,
                segment_labels: segs.labels,
                source_bytes: source.to_string(),
                block_type: 6,
            }),
            ..BlockConstraints::default()
        };
        let batch = batch_of(unit.clone());
        let payload = r#"["하\u0000나"]"#;
        assert!(
            !payload.contains('\0'),
            "the wire payload carries the ESCAPE; a raw-byte scan sees nothing"
        );
        let result = result_for(&unit, OutputKind::Translated, payload);

        let vu = &validate_batch(&batch, &result, "").units[0];
        assert_eq!(vu.rejected_by, Some(ValidationLayer::Schema));
        assert!(vu.accepted_payload.is_none());
        assert!(
            vu.rejection_reason
                .as_deref()
                .unwrap_or_default()
                .contains("html segment 0"),
            "the reason names the offending segment: {:?}",
            vu.rejection_reason
        );
    }
}

// EXT-2026-07 P1-5: the inline-protection layer is wired through the full
// `validate_batch` path — the profile constraints reach it and a tampered
// destination tags the attempt with `ValidationLayer::Inline`.
#[cfg(test)]
mod inline_layer_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, OutputKind, TranslationBatch,
        TranslationBatchResult, TranslationUnit, UnitResult,
    };
    use crate::profile::default_profile;

    #[test]
    fn inline_rejection_tags_layer_inline() {
        // The default profile pledges `preserve_urls = true`, so a swapped
        // link destination must reject through the Inline layer.
        let unit = TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "See [the docs](https://example.com/a).".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        };
        let batch = TranslationBatch {
            batch_id: BatchId::new(1),
            units: vec![unit],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        };
        let result = TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: vec![UnitResult {
                unit_id: BlockId("p-0001".to_string()),
                output_kind: OutputKind::Translated,
                translated_payload: "[문서](https://evil.example/x)를 보라.".to_string(),
                warnings: Vec::new(),
            }],
        };
        let validated = validate_batch(&batch, &result, "");
        assert_eq!(validated.units.len(), 1);
        assert_eq!(
            validated.units[0].rejected_by,
            Some(ValidationLayer::Inline)
        );
        assert!(matches!(
            validated.units[0].final_status,
            FallbackStatus::FallbackSource
        ));
    }

    // Design D2 §B4 / C10(f): a translated reference LABEL rejects through
    // the full `validate_batch` path with `ValidationLayer::Inline`, proving
    // `ref_defs` reaches the inline layer.
    #[test]
    fn broken_reference_label_rejects_through_validate_batch() {
        let unit = TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "See [the docs][ref] for more.".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        };
        let batch = TranslationBatch {
            batch_id: BatchId::new(1),
            units: vec![unit],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        };
        let result = TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: vec![UnitResult {
                unit_id: BlockId("p-0001".to_string()),
                output_kind: OutputKind::Translated,
                // The label `ref` was translated to `참조`, undefined in the
                // pool → resolution breaks on the translated side only.
                translated_payload: "자세한 내용은 [문서][참조].".to_string(),
                warnings: Vec::new(),
            }],
        };
        let ref_defs = "[ref]: https://example.com/r\n";
        let validated = validate_batch(&batch, &result, ref_defs);
        assert_eq!(validated.units.len(), 1);
        assert_eq!(
            validated.units[0].rejected_by,
            Some(ValidationLayer::Inline),
            "a broken reference label must reject through the Inline layer",
        );
        assert!(matches!(
            validated.units[0].final_status,
            FallbackStatus::FallbackSource
        ));
    }
}

// Spec 2026-08-03 §4.2 layer 3: the post-splice structural check runs the
// real splice engine as a validator and, per §3.3, degrades a splice-engine
// fault straight to fallback instead of feeding the retry loop.
#[cfg(test)]
mod html_splice_layer_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, HtmlSegmentConstraints, InputMode, OutputKind,
        TranslationBatch, TranslationBatchResult, TranslationUnit, UnitResult,
    };
    use crate::profile::default_profile;

    /// A one-unit html batch over `source`, plus a result row carrying
    /// `payload`. `constraints.html` is built the way `unit::build_batches`
    /// builds it (real `transync_html::extract` over the same bytes), so the
    /// splice check sees production-shaped inputs.
    fn html_batch_and_result(
        source: &str,
        payload: &str,
    ) -> (TranslationBatch, TranslationBatchResult) {
        let segs = transync_html::extract(source).expect("source block extracts");
        let unit = TranslationUnit {
            unit_id: BlockId::new("html", 1),
            block_kind: BlockKind::Html,
            input_mode: InputMode::HtmlSegments,
            source_payload: serde_json::to_string(&segs.texts).expect("json"),
            context: BlockContext::default(),
            constraints: BlockConstraints {
                html: Some(HtmlSegmentConstraints {
                    segment_count: segs.texts.len() as u32,
                    segment_labels: segs.labels,
                    source_bytes: source.to_string(),
                    block_type: 6,
                }),
                ..BlockConstraints::default()
            },
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        };
        let batch = TranslationBatch {
            batch_id: BatchId::new(1),
            units: vec![unit],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        };
        let result = TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: vec![UnitResult {
                unit_id: BlockId::new("html", 1),
                output_kind: OutputKind::Translated,
                translated_payload: payload.to_string(),
                warnings: Vec::new(),
            }],
        };
        (batch, result)
    }

    /// Overwrite the claimed segment count so layer 2 can be steered
    /// independently of what the source block really contains.
    fn batch_with_count(mut batch: TranslationBatch, count: u32) -> TranslationBatch {
        batch
            .units
            .iter_mut()
            .filter_map(|u| u.constraints.html.as_mut())
            .for_each(|h| h.segment_count = count);
        batch
    }

    // Spec §4.2 layer 3 + §3.3: a splice-engine failure is not a model
    // fault — direct fallback, rejected_by None (no verbatim retry burn).
    #[test]
    fn splice_engine_failure_falls_back_directly_without_retry_marker() {
        // Force the engine to fail by giving constraints whose source_bytes
        // disagree with the payload segment count (splice's own count guard).
        let (batch, result) = html_batch_and_result(
            "<p>one</p><p>two</p>", // 2 kept segments
            "[\"하나\"]",           // 1 translation → splice count mismatch
        );
        // per-kind must not reject first: claim segment_count = 1 so layer 2
        // passes and the mismatch surfaces inside the splice engine.
        let vb = validate_batch(&batch_with_count(batch, 1), &result, "");
        let vu = &vb.units[0];
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
        assert!(vu.rejected_by.is_none(), "direct fallback — not retryable");
        assert!(
            vu.warnings.iter().any(|w| w.contains("splice")),
            "cause recorded: {:?}",
            vu.warnings
        );
    }

    #[test]
    fn clean_html_splice_is_accepted_with_wire_payload() {
        let (batch, result) = html_batch_and_result("<p>one</p>", "[\"하나\"]");
        let vb = validate_batch(&batch, &result, "");
        let vu = &vb.units[0];
        assert_eq!(vu.final_status, FallbackStatus::Translated);
        assert_eq!(
            vu.accepted_payload.as_deref(),
            Some("[\"하나\"]"),
            "accepted_payload stays the WIRE payload — regen re-splices (spec §3.3); \
             this is also what keeps cached entries wire-shaped"
        );
    }
}

// OI-0031: `validate_batch` decomposes a batch-level schema fault per unit
// instead of blanket-rejecting the batch. These tests pin the layer's own
// contract (the pipeline-level budget behavior lives in
// `pipeline::dispatch::batch_fault_tests`).
#[cfg(test)]
mod batch_fault_layer_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, OutputKind, TranslationBatch,
        TranslationBatchResult, TranslationUnit, UnitResult,
    };
    use crate::profile::default_profile;

    fn unit(id: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: format!("source of {id}"),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn batch_of(ids: &[&str]) -> TranslationBatch {
        TranslationBatch {
            batch_id: BatchId::new(1),
            units: ids.iter().map(|id| unit(id)).collect(),
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        }
    }

    fn row(id: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId(id.to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: format!("번역 {id}"),
            warnings: Vec::new(),
        }
    }

    fn result_of(rows: Vec<UnitResult>) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: rows,
        }
    }

    fn unit_by<'a>(v: &'a ValidatedBatch, id: &str) -> &'a ValidatedUnit {
        v.units
            .iter()
            .find(|u| u.unit_id.0 == id)
            .unwrap_or_else(|| panic!("no validated unit for {id}"))
    }

    /// A dropped row rejects only its own unit; the batch-mates' payloads
    /// still flow through the per-unit layers and are accepted.
    #[test]
    fn dropped_row_rejects_only_its_own_unit() {
        let batch = batch_of(&["p-0001", "p-0002", "p-0003"]);
        let result = result_of(vec![row("p-0002"), row("p-0003")]);
        let v = validate_batch(&batch, &result, "");

        assert_eq!(v.units.len(), 3, "units stay total over the REQUESTED set");
        for id in ["p-0002", "p-0003"] {
            let u = unit_by(&v, id);
            assert_eq!(u.rejected_by, None, "{id} is innocent: {u:?}");
            assert_eq!(u.accepted_payload.as_deref(), Some(&*format!("번역 {id}")));
        }
        let offender = unit_by(&v, "p-0001");
        assert_eq!(offender.rejected_by, Some(ValidationLayer::Schema));
        assert!(offender.accepted_payload.is_none());
        assert!(matches!(
            offender.final_status,
            FallbackStatus::FallbackSource
        ));

        let bf = v.batch_fault.expect("a fault is reported");
        assert!(!bf.malformed_request);
        assert_eq!(bf.offenders.len(), 1);
        assert_eq!(bf.offenders[0].0, "p-0001");
        assert!(bf.foreign_ids.is_empty());
        assert!(bf.reason.contains("missing from result"), "{}", bf.reason);
    }

    /// Both copies of a duplicated row are discarded — picking one would be
    /// a coin flip on content — and only that unit is implicated.
    #[test]
    fn duplicated_row_discards_both_copies() {
        let batch = batch_of(&["p-0001", "p-0002"]);
        let mut dup = row("p-0001");
        dup.translated_payload = "다른 번역".to_string();
        let result = result_of(vec![row("p-0001"), dup, row("p-0002")]);
        let v = validate_batch(&batch, &result, "");

        assert_eq!(v.units.len(), 2);
        assert_eq!(unit_by(&v, "p-0002").rejected_by, None);
        let offender = unit_by(&v, "p-0001");
        assert!(offender.accepted_payload.is_none());
        assert!(
            offender
                .rejection_reason
                .as_deref()
                .unwrap_or_default()
                .contains("duplicated in provider result"),
            "{offender:?}"
        );
        let bf = v.batch_fault.expect("a fault is reported");
        assert_eq!(bf.offenders.len(), 1);
    }

    /// An unrequested row is dropped from `units` (it maps to no requested
    /// unit) and named on the fault — with no offender, so nobody is charged.
    #[test]
    fn foreign_row_is_dropped_and_named_without_offenders() {
        let batch = batch_of(&["p-0001"]);
        let result = result_of(vec![row("p-0001"), row("zz-9999")]);
        let v = validate_batch(&batch, &result, "");

        assert_eq!(v.units.len(), 1, "only requested units appear");
        assert_eq!(unit_by(&v, "p-0001").rejected_by, None);
        let bf = v.batch_fault.expect("a fault is reported");
        assert!(bf.offenders.is_empty(), "no requested unit is at fault");
        assert_eq!(bf.foreign_ids, vec!["zz-9999".to_string()]);
    }

    /// A clean 1:1 result carries no fault at all.
    #[test]
    fn clean_batch_has_no_fault() {
        let batch = batch_of(&["p-0001", "p-0002"]);
        let result = result_of(vec![row("p-0002"), row("p-0001")]);
        let v = validate_batch(&batch, &result, "");
        assert!(v.batch_fault.is_none());
        assert!(v.units.iter().all(|u| u.rejected_by.is_none()));
    }

    /// §B.6 test 8 (defensive half) — a duplicated id in the REQUEST is a
    /// caller contract violation: nothing can be salvaged, so every unit is
    /// rejected with the legacy diagnostic and the fault says why. The
    /// pipeline never gets here (`process_one_batch` preflights and aborts).
    #[test]
    fn request_duplicate_rejects_every_unit_with_legacy_message() {
        let batch = batch_of(&["p-0001", "p-0001", "p-0002"]);
        let result = result_of(vec![row("p-0001"), row("p-0002")]);
        let v = validate_batch(&batch, &result, "");

        assert_eq!(v.units.len(), 3, "one row per request entry");
        for u in &v.units {
            assert_eq!(u.rejected_by, Some(ValidationLayer::Schema));
            assert_eq!(
                u.rejection_reason.as_deref(),
                Some("duplicate unit_id in request: 3 units but 2 unique IDs"),
                "the pre-OI-0031 message is preserved"
            );
        }
        let bf = v.batch_fault.expect("a fault is reported");
        assert!(bf.malformed_request);
        assert!(bf.offenders.is_empty(), "re-dispatch would be futile");
    }
}

#[cfg(test)]
mod provider_warning_bound_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, InputMode, TranslationBatch,
        TranslationBatchResult, TranslationUnit,
    };
    use crate::profile::default_profile;

    fn unit() -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "Hello world.".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn batch_of(unit: TranslationUnit) -> TranslationBatch {
        TranslationBatch {
            batch_id: BatchId::new(1),
            units: vec![unit],
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        }
    }

    fn result_with(
        kind: OutputKind,
        payload: &str,
        warnings: Vec<String>,
    ) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: BatchId::new(1),
            detected_source_language: None,
            units: vec![UnitResult {
                unit_id: BlockId("p-0001".to_string()),
                output_kind: kind,
                translated_payload: payload.to_string(),
                warnings,
            }],
        }
    }

    /// R0002-0068: a provider's warnings reach the report JSON and the
    /// cache verbatim, so they are bounded on the way in — in count and in
    /// per-message bytes alike. Both arms are checked, because the cap is
    /// applied once at the door rather than restated per arm: an accepted
    /// unit and a rejected one must be bounded the same.
    #[test]
    fn provider_warnings_are_bounded_in_count_and_in_bytes() {
        let flood: Vec<String> = (0..500)
            .map(|i| format!("{i}:{}", "x".repeat(5000)))
            .collect();

        for (kind, payload) in [
            (OutputKind::Translated, "안녕하세요."),
            // Rejected at the per-kind layer: `preserved` that is not the
            // source (EXT-2026-07 P2-9).
            (OutputKind::Preserved, "not the source"),
        ] {
            let batch = batch_of(unit());
            let vb = validate_batch(&batch, &result_with(kind, payload, flood.clone()), "");
            let vu = &vb.units[0];

            assert!(
                vu.warnings.len() <= MAX_UNIT_WARNINGS + 1,
                "{kind:?}: {} warnings survived",
                vu.warnings.len()
            );
            for w in &vu.warnings {
                assert!(
                    w.len() < 700,
                    "{kind:?}: a single warning kept {} bytes",
                    w.len()
                );
            }
            assert!(
                vu.warnings
                    .last()
                    .expect("bounded, not emptied")
                    .contains("492 further provider warnings dropped"),
                "{kind:?}: the overflow is named, not silently swallowed: {:?}",
                vu.warnings.last()
            );
        }
    }

    /// The bound is a ceiling, not a rewrite: a provider that says one
    /// ordinary thing is quoted exactly, in order.
    #[test]
    fn warnings_under_the_bound_are_forwarded_verbatim() {
        let said = vec![
            "output may be truncated".to_string(),
            "source language uncertain".to_string(),
        ];
        let batch = batch_of(unit());
        let vb = validate_batch(
            &batch,
            &result_with(OutputKind::Translated, "안녕하세요.", said.clone()),
            "",
        );
        assert_eq!(vb.units[0].warnings, said);
    }

    /// A multi-byte message is cut at a character boundary, not through
    /// one — the report is JSON, and half a codepoint is not a string.
    #[test]
    fn a_capped_warning_stays_char_safe() {
        let hangul = "가".repeat(4000);
        let batch = batch_of(unit());
        let vb = validate_batch(
            &batch,
            &result_with(OutputKind::Translated, "안녕하세요.", vec![hangul]),
            "",
        );
        let w = &vb.units[0].warnings[0];
        assert!(w.contains("truncated"), "{w:?}");
        assert!(w.starts_with('가'), "{w:?}");
    }
}

// ti `457e51`: the layered validators against an INDENTED code block's unit.
//
// No validator changes for this fix — `expected_label` already answers
// "code-block" and `per_kind::check_code` already enforces the two-direction
// info rule. What these pin is that the *new payload shape* satisfies them:
// an echoed synthesized fence is accepted outright (no retry, no fallback),
// and the info-string rule still bites against that shape.
#[cfg(test)]
mod indented_code_layer_tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::id::BlockKind;
    use crate::llm::{BatchId, OutputKind, TranslationBatch, TranslationBatchResult, UnitResult};
    use crate::profile::default_profile;
    use crate::unit::{build_batches, html_outcomes};

    const SRC: &str = "Intro.\n\n    let s = \"```\";\n    println!(\"{}\", s);\n\nSeparator.\n\n        indented code line\n\nOutro.\n";

    /// The real batch the pipeline would dispatch for `SRC`, narrowed to its
    /// code units — so the payload under test is whatever
    /// `unit::payload::assemble` actually produces, never a hand-written
    /// stand-in.
    fn code_batch() -> TranslationBatch {
        let mut doc = crate::parser::parse(SRC).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let units: Vec<_> = build_batches(&doc, &opts, None, &outcomes)
            .iter()
            .flat_map(|b| b.units.clone())
            .filter(|u| matches!(u.block_kind, BlockKind::CodeBlock { .. }))
            .map(|mut u| {
                u.batch_id = BatchId::new(1);
                u
            })
            .collect();
        assert_eq!(units.len(), 2, "the fixture has two code blocks");
        TranslationBatch {
            batch_id: BatchId::new(1),
            units,
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: Vec::new(),
            profile: default_profile(),
        }
    }

    fn echo(batch: &TranslationBatch, rewrite: impl Fn(&str) -> String) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: batch.batch_id.clone(),
            detected_source_language: None,
            units: batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: rewrite(&u.source_payload),
                    warnings: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn an_echoed_indented_code_payload_passes_every_layer() {
        let batch = code_batch();
        let result = echo(&batch, |p| p.to_string());
        let vb = validate_batch(&batch, &result, "");

        for vu in &vb.units {
            assert_eq!(
                vu.rejected_by,
                None,
                "{} rejected at {:?}: {:?} (payload {:?})",
                vu.unit_id,
                vu.rejected_by,
                vu.rejection_reason,
                batch
                    .units
                    .iter()
                    .find(|u| u.unit_id == vu.unit_id)
                    .map(|u| u.source_payload.as_str()),
            );
            assert_eq!(vu.final_status, FallbackStatus::Translated);
            assert!(vu.accepted_payload.is_some());
        }
        assert!(vb.batch_fault.is_none());
    }

    /// R0003-0043's two-direction info rule, re-pinned against the new
    /// payload shape: an indented source block has NO info string, so a
    /// returned payload that invents one is a structural change and must
    /// reject at `PerKindShape`. (This one already holds on the pre-fix
    /// tree — it is a regression guard for the payload swap, not a red
    /// test.)
    #[test]
    fn a_returned_info_string_is_still_rejected_for_an_indented_block() {
        let batch = code_batch();
        let result = echo(&batch, |_| "```rust\nlet x = 1;\n```".to_string());
        let vb = validate_batch(&batch, &result, "");

        for vu in &vb.units {
            assert_eq!(
                vu.rejected_by,
                Some(ValidationLayer::PerKindShape),
                "{}: {:?}",
                vu.unit_id,
                vu.rejection_reason,
            );
            assert!(
                vu.rejection_reason
                    .as_deref()
                    .unwrap_or_default()
                    .contains("code fence info changed"),
                "reason: {:?}",
                vu.rejection_reason,
            );
            assert!(vu.accepted_payload.is_none());
        }
    }
}
