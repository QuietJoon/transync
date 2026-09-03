//! Report assembly — per-batch fold, `ValidationReport` composition, and
//! alignment-map assembly.
//!
//! [`aggregate_batch_results`] folds every [`BatchResult`] the dispatch phase
//! produced into the run-level maps, [`build_validation_report`] composes
//! those maps into the report [`super::run_pipeline`] returns,
//! [`order_report_by_document`] puts every block list in that report into
//! document order once the run-level fields are stamped, and
//! [`assemble_alignment_map`] projects the finalized per-unit statuses onto
//! the syntax layer's map builder. What counts as a content retry is
//! [`super::policy`]'s decision ([`super::policy::content_retry_count`]);
//! this module only assembles.
//!
//! TRACE: SCN-07

use super::dispatch::BatchResult;
use super::policy;
use crate::FallbackStatus;
use crate::align::{AlignmentMap, build_alignment_map};
use crate::id::BlockId;
use crate::unit::HtmlOutcome;
use crate::validate::{
    AttemptOutcome, UnitValidationRecord, ValidatedBatch, ValidatedUnit, ValidationReport,
};
use std::collections::HashMap;

/// Batch results folded into pipeline-level state.
pub(crate) struct Aggregated {
    pub(crate) accepted: HashMap<BlockId, ValidatedUnit>,
    pub(crate) attempts: HashMap<BlockId, Vec<AttemptOutcome>>,
    pub(crate) detected_source_language: Option<String>,
    pub(crate) provider_retries: u32,
    /// OI-0031: batch-schema-fault rounds summed across all batches.
    pub(crate) batch_schema_faults: u32,
    /// DCR-0028 §3: how many batches reached the provider. Zero means the run
    /// was served entirely from cache — the only shape whose reported detection
    /// may come from a stored document-metadata record.
    pub(crate) provider_batches_dispatched: usize,
}

/// Fold per-batch results into the pipeline-level maps. The `accepted`
/// map is unique per BlockId because each unit appears in exactly one
/// batch.
pub(crate) fn aggregate_batch_results(batch_results: Vec<BatchResult>) -> Aggregated {
    let mut accepted: HashMap<BlockId, ValidatedUnit> = HashMap::new();
    let mut attempts: HashMap<BlockId, Vec<AttemptOutcome>> = HashMap::new();
    let mut detections: Vec<(usize, String)> = Vec::new();
    let mut provider_retries: u32 = 0;
    let mut batch_schema_faults: u32 = 0;
    let mut provider_batches_dispatched: usize = 0;

    for br in batch_results {
        if br.dispatched_provider_call {
            provider_batches_dispatched += 1;
        }
        for vu in br.accepted {
            accepted.insert(vu.unit_id.clone(), vu);
        }
        for (id, log) in br.attempts {
            attempts.entry(id).or_default().extend(log);
        }
        // R0003-0045: saturating, for the same reason the per-batch counters
        // they fold are — these are reported numbers, and a wrapped total
        // reads as a clean run.
        provider_retries = provider_retries.saturating_add(br.provider_retries);
        batch_schema_faults = batch_schema_faults.saturating_add(br.batch_schema_fault_rounds);
        if let Some(lang) = br.detected_source_language {
            detections.push((br.input_index, lang));
        }
    }

    // The detection from the lowest-indexed batch wins (input order,
    // not completion order) for determinism. R0006-0021: batches can
    // disagree; the winner stands, but a conflict is worth surfacing.
    detections.sort_by_key(|(idx, _)| *idx);
    let detected_source_language = detections.first().map(|(_, lang)| lang.clone());
    if let Some(first) = &detected_source_language
        && detections.iter().any(|(_, lang)| lang != first)
    {
        tracing::warn!(
            target: "transync::pipeline",
            "conflicting detected_source_language across batches: keeping {first:?} (lowest batch index); all observed: {:?}",
            detections.iter().map(|(_, l)| l.as_str()).collect::<Vec<_>>()
        );
    }

    Aggregated {
        accepted,
        attempts,
        detected_source_language,
        provider_retries,
        batch_schema_faults,
        provider_batches_dispatched,
    }
}

/// Compose a `ValidationReport` from the accepted map + per-unit attempt log.
///
/// The composed report is **unordered**: `accepted` is a `HashMap`, so
/// `per_unit` comes out in whatever order iteration produced.
/// [`order_report_by_document`] is what puts every block list into document
/// order, and it runs once, after [`super::run_pipeline`] has stamped the
/// run-level fields (`full_reparse_fallbacks` among them).
///
/// TRACE: SCN-07
pub(crate) fn build_validation_report(
    accepted: &HashMap<BlockId, ValidatedUnit>,
    attempts: &HashMap<BlockId, Vec<AttemptOutcome>>,
    extra_units: &[ValidatedUnit],
) -> ValidationReport {
    let mut report = ValidationReport::default();

    // DCR-0026: `extra_units` are the row-window entries the merge removed
    // from `accepted` — real dispatches with real attempts, which regen must
    // not see but the report must. They are folded into the counters like any
    // other unit (a window's retries and fallbacks are its own), and
    // `order_report_by_document` files each under its parent.
    for vu in accepted.values().chain(extra_units.iter()) {
        let id = &vu.unit_id;
        let unit_attempts = attempts.get(id).cloned().unwrap_or_default();
        // OI-0008: content retries only. A batch-fault round is charged to
        // `batch_schema_faults` and a cache-hit re-validation row charges no
        // budget at all, so neither is a retry of this unit's content.
        // R0003-0045: saturating totals — see `aggregate_batch_results`.
        report.total_retries = report
            .total_retries
            .saturating_add(policy::content_retry_count(&unit_attempts));
        if matches!(vu.final_status, FallbackStatus::FallbackSource) {
            report.total_fallbacks = report.total_fallbacks.saturating_add(1);
        }
        report.per_unit.push(UnitValidationRecord {
            unit_id: id.clone(),
            attempts: unit_attempts,
            final_status: vu.final_status,
            warnings: vu.warnings.clone(),
        });
    }

    report
}

/// Put **every** block list in the report into document (source) order.
///
/// OI-0021 item 2 established the rule for `per_unit`; R0001-0028 found its
/// sibling `full_reparse_fallbacks` still sorted lexically by the `BlockId`
/// string, which groups by kind prefix (`c-`, `h1-`, `li-`, `p-`) and stops
/// tracking the document. One pass now owns both, so the next list added to
/// the report has exactly one place to join and cannot re-open the gap.
///
/// `doc.blocks` is the canonical source-order traversal (see `Block` /
/// `id::assign_block_ids`), so a block's index within it is the authoritative
/// order key. Ties cannot occur (indices are unique); the id-string tiebreak
/// only kicks in for an entry with no matching source block, which keeps the
/// order deterministic instead of arbitrary should that ever happen.
///
/// Runs after the run-level fields are stamped, because
/// `full_reparse_fallbacks` is one of them.
///
/// TRACE: SCN-07
pub(crate) fn order_report_by_document(
    doc: &crate::parser::Document,
    report: &mut ValidationReport,
) {
    let doc_order: HashMap<&BlockId, usize> = doc
        .blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (&b.block_id, i))
        .collect();
    // DCR-0026: a row-window unit is not a block, so it has no index of its
    // own — it ranks at its PARENT's, which lists a table's windows directly
    // under it (the id-string tiebreak below then orders `t-0007` before
    // `t-0007.w01` before `t-0007.w02`). The convention is parsed by
    // `unit::split::parent_id_of`, the same module that writes it.
    let rank = |id: &BlockId| {
        doc_order.get(id).copied().unwrap_or_else(|| {
            crate::unit::split::parent_id_of(id)
                .and_then(|parent| doc_order.get(&parent).copied())
                .unwrap_or(usize::MAX)
        })
    };

    report.per_unit.sort_by(|a, b| {
        rank(&a.unit_id)
            .cmp(&rank(&b.unit_id))
            .then_with(|| a.unit_id.0.cmp(&b.unit_id.0))
    });
    report
        .full_reparse_fallbacks
        .sort_by(|a, b| rank(a).cmp(&rank(b)).then_with(|| a.0.cmp(&b.0)));
}

// DCR-0026: a row window is a real dispatch with real attempts, so it keeps
// its own report row — and that row has to land under its parent's, not at the
// end of the report where an unknown id would otherwise sort.
#[cfg(test)]
mod window_row_order_tests {
    use super::order_report_by_document;
    use crate::FallbackStatus;
    use crate::id::BlockId;
    use crate::validate::{UnitValidationRecord, ValidationReport};

    fn row(id: &str) -> UnitValidationRecord {
        UnitValidationRecord {
            unit_id: BlockId(id.to_string()),
            attempts: Vec::new(),
            final_status: FallbackStatus::Translated,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn window_rows_list_directly_under_their_parent() {
        let mut doc =
            crate::parser::parse("first paragraph\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\ntail\n")
                .expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let table = doc.blocks[1].block_id.0.clone();
        assert!(table.starts_with("t-"), "the fixture's middle block");

        let mut report = ValidationReport {
            // Deliberately scrambled, and with the window rows split apart —
            // the sort is what has to put them back.
            per_unit: vec![
                row(&format!("{table}.w03")),
                row("p-0003"),
                row(&format!("{table}.w01")),
                row("p-0001"),
                row(&format!("{table}.w02")),
                // An id that names no block AND is not a window: it still
                // sorts last, which is the pre-existing behavior.
                row("t-9999"),
            ],
            ..ValidationReport::default()
        };
        order_report_by_document(&doc, &mut report);
        let ids: Vec<&str> = report
            .per_unit
            .iter()
            .map(|r| r.unit_id.0.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "p-0001",
                format!("{table}.w01").as_str(),
                format!("{table}.w02").as_str(),
                format!("{table}.w03").as_str(),
                "p-0003",
                "t-9999",
            ],
            "windows rank at their parent's document index, ordered among \
             themselves by the id-string tiebreak"
        );
    }
}

/// Assemble the run's alignment map from the finalized units.
///
/// The report phase's second product: it projects each unit's *finalized*
/// status into the status map [`build_alignment_map`] keys its rows off,
/// then patches in the one summary field the syntax layer cannot see —
/// `retried_units`, a property of the validation report's attempt log rather
/// than of the document. `validation_report` must therefore already be the
/// composed report ([`build_validation_report`] plus the run-level fields
/// [`super::run_pipeline`] stamps onto it).
///
/// DCR-0019 left this inline in `run_pipeline` as an open seam; it lives here
/// with the rest of the reporting phase.
///
/// TRACE: SCN-07
pub(crate) fn assemble_alignment_map(
    doc: &crate::parser::Document,
    final_validated: &[ValidatedBatch],
    offsets: &crate::regen::BlockOffsets,
    opts: &crate::TranslateOptions,
    detected_source_language: Option<String>,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
    validation_report: &ValidationReport,
) -> AlignmentMap {
    // Spec §3.2: align keys each row off the status the pipeline finalized for
    // that unit — fallbacks included, since a fallback unit's row must report
    // `fallback_source` rather than the synthesized `preserved`.
    let statuses: HashMap<BlockId, FallbackStatus> = final_validated
        .iter()
        .flat_map(|batch| batch.units.iter())
        .map(|unit| (unit.unit_id.clone(), unit.final_status))
        .collect();
    // Task-4 review: `build_alignment_map`'s completeness obligation — every
    // block `is_translatable_block` accepts has a finalized status here, so no
    // translatable row can silently synthesize `preserved` — was doc-only.
    // Debug-only and cheap: one pass over the blocks, one hash lookup each.
    debug_assert!(
        doc.blocks
            .iter()
            .filter(|b| transync_syntax::outcome::is_translatable_block(b, html_outcomes))
            .all(|b| statuses.contains_key(&b.block_id)),
        "every translatable block must carry a finalized status into build_alignment_map",
    );
    let mut alignment_map = build_alignment_map(
        doc,
        &statuses,
        offsets,
        &opts.source_language,
        &opts.target_language,
        detected_source_language,
        html_outcomes,
    );
    // `R0001-0025` in the removed `reviews/reviewed/0001.md` — not the live
    // round's 0025, which `llm.rs` cites for the list-depth `u8` overflow:
    // ValidationSummary.retried_units was never populated.
    // The validation report tracks every attempt per unit; a unit is
    // "retried" if it accumulated more than one network attempt
    // (attempt_number > 1; cache hits at attempt_number 0 don't count).
    alignment_map.validation_summary.retried_units = validation_report
        .per_unit
        .iter()
        .filter(|r| r.attempts.iter().any(|a| a.attempt_number > 1))
        .count() as u32;
    alignment_map
}

// OI-0021 item 2: the report's document-order contract, driven end-to-end
// through `run_pipeline`.
#[cfg(test)]
mod report_order_tests {
    // The moved test keeps the exact scope it had in `pipeline.rs`.
    use crate::llm::{OutputKind, TranslationBatch, TranslationBatchResult};
    use crate::pipeline::*;

    // OI-0021 item 2: `ValidationReport.per_unit` must be ordered by document
    // (source) order, not by the lexicographic `unit_id` string. On a
    // mixed-kind document the two orders diverge — sorting by id groups by kind
    // prefix (c-, h1-, li-, p-), which no longer tracks the document — so this
    // fixture fails under the old id-string sort and passes under the fix.
    #[tokio::test]
    async fn validation_report_per_unit_is_in_document_order() {
        use crate::cache::InMemoryCache;

        // Passthrough translator: echo every unit's source back unchanged so
        // all units validate and land in `per_unit`.
        struct PassthroughTranslator;
        #[async_trait::async_trait]
        impl Translator for PassthroughTranslator {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let units = batch
                    .units
                    .iter()
                    .map(|u| UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::Preserved,
                        translated_payload: u.source_payload.clone(),
                        warnings: Vec::new(),
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        // Interleaved kinds: heading, paragraph, code, list items, heading,
        // paragraph. Document order is h1, p, code, li, li, h2, p; the ids the
        // pipeline assigns (kind-prefixed over a source-ordered counter) are
        // h1-0001, p-0002, c-0003, li-0004, li-0005, h2-0006, p-0007, whose
        // lexicographic order (c-, h1-, h2-, li-, li-, p-, p-) is a different
        // sequence — so this fixture distinguishes the two sorts.
        let src = "\
# Alpha

first paragraph

```rust
let x = 1;
```

- item one
- item two

## Beta

second paragraph
";
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };

        // Expected document order, taken from the parser's source-order block
        // list — the authoritative order the report must match.
        let mut doc = parse(src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let expected: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert!(
            expected.len() >= 6,
            "fixture should produce several interleaved blocks, got {expected:?}"
        );

        let translator = PassthroughTranslator;
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let report_order: Vec<String> = out
            .validation_report
            .per_unit
            .iter()
            .map(|r| r.unit_id.0.clone())
            .collect();

        // The report is exactly document order.
        assert_eq!(
            report_order, expected,
            "per_unit must be in document (source) order"
        );

        // Guard: the old lexicographic-by-id sort would have produced a
        // different sequence, so the assertion above genuinely fails under the
        // old behaviour rather than passing vacuously.
        let mut lexicographic = report_order.clone();
        lexicographic.sort();
        assert_ne!(
            report_order, lexicographic,
            "fixture must distinguish document order from id-string order"
        );
    }

    // R0001-0028: `full_reparse_fallbacks` is `per_unit`'s sibling and was
    // still sorted with `a.0.cmp(&b.0)` — the lexicographic id sort OI-0021
    // removed from `per_unit`. Both lists now come out of the one
    // `order_report_by_document` pass.
    #[tokio::test]
    async fn full_reparse_fallbacks_are_in_document_order() {
        use crate::cache::InMemoryCache;

        /// Echoes every unit, except that the second list item comes back
        /// indented by two spaces. That payload is a valid one-item list on
        /// its own (`per_kind::check_list` and the fragment reparse both
        /// accept it), but spliced after the first item it becomes a
        /// SUB-list — so only the post-regen full-document reparse catches
        /// it, which is exactly the channel `full_reparse_fallbacks` reports.
        struct IndentTheSecondItem;
        #[async_trait::async_trait]
        impl Translator for IndentTheSecondItem {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let units = batch
                    .units
                    .iter()
                    .map(|u| {
                        if u.unit_id.0 == "li-0005" {
                            UnitResult {
                                unit_id: u.unit_id.clone(),
                                output_kind: OutputKind::Translated,
                                translated_payload: format!("  {}", u.source_payload),
                                warnings: Vec::new(),
                            }
                        } else {
                            UnitResult {
                                unit_id: u.unit_id.clone(),
                                output_kind: OutputKind::Preserved,
                                translated_payload: u.source_payload.clone(),
                                warnings: Vec::new(),
                            }
                        }
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        // The same interleaved fixture the `per_unit` test uses: document
        // order h1-0001, p-0002, c-0003, li-0004, li-0005, h2-0006, p-0007,
        // whose id-string order (c-, h1-, h2-, li-, li-, p-, p-) differs.
        let src = "\
# Alpha

first paragraph

```rust
let x = 1;
```

- item one
- item two

## Beta

second paragraph
";
        // `FallbackAll` makes the downgrade set the whole document, so the
        // fallback list carries every kind at once — the mixed-kind set the
        // lexical sort mis-ordered.
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            full_reparse_failure: crate::FullReparseFailure::FallbackAll,
            ..Default::default()
        };

        let mut doc = parse(src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let expected: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();

        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(src, &opts, &IndentTheSecondItem, cache_dyn)
            .await
            .expect("FallbackAll degrades, never aborts");

        let fallback_order: Vec<String> = out
            .validation_report
            .full_reparse_fallbacks
            .iter()
            .map(|id| id.0.clone())
            .collect();

        // Every unit was healthy before the reparse gate ran, so the whole
        // document is reported — in document order.
        assert_eq!(
            fallback_order, expected,
            "full_reparse_fallbacks must be in document (source) order"
        );

        // Guard: the fixture genuinely distinguishes the two orders, so the
        // assertion above cannot pass under the old lexicographic sort.
        let mut lexicographic = fallback_order.clone();
        lexicographic.sort();
        assert_ne!(
            fallback_order, lexicographic,
            "fixture must distinguish document order from id-string order"
        );

        // R0001-0027: the same report declares the artifact's schema version.
        assert_eq!(
            out.validation_report.schema_version,
            crate::validate::VALIDATION_REPORT_SCHEMA_VERSION,
        );
    }
}

// OI-0008: `ValidationReport.total_retries` counts CONTENT retries only —
// re-dispatches of a unit's own content. The two other things that put an
// extra row in the attempt log are counted elsewhere and must not inflate it:
// a batch-fault redispatch round is `batch_schema_faults`' business
// (OI-0031), and the cache-hit re-validation row (attempt_number 0, ADR-0015)
// is not a network attempt and charges no budget at all. Before this the
// field was `attempts.len() - 1`, which conflated all three and contradicted
// both `pipeline`'s and `validate`'s own documentation of the split.
// R0003-0045: the run-level counters are work-driven — one increment per real
// round trip or per validated unit — so a wrap needs a run that cannot
// physically happen. They are nonetheless *reported* numbers, and a wrapped
// total reads as a clean run, which is the one failure mode telemetry must not
// have. The fold is exercised directly here because no constructible pipeline
// run reaches the top of `u32`.
#[cfg(test)]
mod counter_saturation_tests {
    use super::*;

    fn batch_result(input_index: usize, provider_retries: u32, faults: u32) -> BatchResult {
        BatchResult {
            input_index,
            accepted: Vec::new(),
            attempts: Vec::new(),
            detected_source_language: None,
            provider_retries,
            batch_schema_fault_rounds: faults,
            dispatched_provider_call: true,
        }
    }

    #[test]
    fn folding_batch_counters_saturates_rather_than_wrapping() {
        let aggregated = aggregate_batch_results(vec![
            batch_result(0, u32::MAX, u32::MAX - 1),
            batch_result(1, 7, 5),
        ]);
        assert_eq!(
            aggregated.provider_retries,
            u32::MAX,
            "an overflowing retry total pins at the top instead of restarting at zero"
        );
        assert_eq!(aggregated.batch_schema_faults, u32::MAX);
    }
}

#[cfg(test)]
mod total_retries_semantics_tests {
    use super::*;
    // The moved tests keep the exact scope they had in `pipeline.rs`.
    use crate::cache::InMemoryCache;
    use crate::llm::{OutputKind, TranslationBatch, TranslationBatchResult};
    use crate::pipeline::*;
    use std::sync::atomic::Ordering;

    const SRC: &str = "alpha paragraph\n\nbravo paragraph\n";
    /// The lexicographically first id — the one the stubs single out.
    const TARGET: &str = "p-0001";

    fn opts() -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        }
    }

    fn translated(u: &TranslationUnit) -> UnitResult {
        UnitResult {
            unit_id: u.unit_id.clone(),
            output_kind: OutputKind::Translated,
            translated_payload: format!("{} 번역", u.source_payload),
            warnings: Vec::new(),
        }
    }

    fn failed(u: &TranslationUnit) -> UnitResult {
        UnitResult {
            unit_id: u.unit_id.clone(),
            output_kind: OutputKind::FailedNeedsFallback,
            translated_payload: String::new(),
            warnings: Vec::new(),
        }
    }

    fn ok(batch: TranslationBatch, units: Vec<UnitResult>) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        }
    }

    fn attempts_of<'a>(out: &'a TranslationOutput, id: &str) -> &'a [AttemptOutcome] {
        &out.validation_report
            .per_unit
            .iter()
            .find(|r| r.unit_id.0 == id)
            .unwrap_or_else(|| panic!("no report row for {id}"))
            .attempts
    }

    /// Drops the target's result row on round 1 only, then translates
    /// everything: exactly one batch-fault round, zero content retries.
    #[derive(Default)]
    struct DropOnceTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for DropOnceTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let units = batch
                .units
                .iter()
                .filter(|u| !(round == 1 && u.unit_id.0 == TARGET))
                .map(translated)
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// A mangled response envelope costs the batch budget, not the unit's
    /// content budget — and so it is not a content retry either.
    #[tokio::test]
    async fn a_batch_fault_round_is_not_a_content_retry() {
        let opts = opts();
        let translator = DropOnceTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let rows = attempts_of(&out, TARGET);
        assert_eq!(
            rows.len(),
            2,
            "one dropped round, then a clean one: {rows:?}"
        );
        assert!(rows[0].batch_fault);
        assert!(!rows[1].batch_fault);

        assert_eq!(
            out.validation_report.batch_schema_faults, 1,
            "the round is counted — on its own budget's channel"
        );
        assert_eq!(
            out.validation_report.total_retries, 0,
            "…and nowhere else: the unit's content was never re-dispatched"
        );
    }

    /// Rejects the target's own output on round 1, then translates it.
    #[derive(Default)]
    struct FailOnceTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for FailOnceTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let units = batch
                .units
                .iter()
                .map(|u| {
                    if round == 1 && u.unit_id.0 == TARGET {
                        failed(u)
                    } else {
                        translated(u)
                    }
                })
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// The positive case the field exists for: one genuine re-dispatch of a
    /// unit's own content counts exactly once.
    #[tokio::test]
    async fn one_content_retry_counts_once() {
        let opts = opts();
        let translator = FailOnceTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let rows = attempts_of(&out, TARGET);
        assert_eq!(rows.len(), 2, "{rows:?}");
        assert!(rows.iter().all(|r| !r.batch_fault));
        assert_eq!(out.validation_report.batch_schema_faults, 0);
        assert_eq!(out.validation_report.total_retries, 1);
    }

    /// Translates everything cleanly, first try.
    #[derive(Default)]
    struct CleanTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for CleanTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let units = batch.units.iter().map(translated).collect();
            Ok(ok(batch, units))
        }
    }

    /// ADR-0015: a cache hit that fails re-validation is evicted and the
    /// pristine unit re-enters the dispatch loop with clean counters. Its
    /// `attempt_number: 0` row charges no budget, so the fresh dispatch is
    /// the unit's FIRST content attempt, not a retry.
    #[tokio::test]
    async fn a_rejected_cache_hit_is_not_a_content_retry() {
        let src = "hello world\n";
        let opts = opts();
        let translator = CleanTranslator::default();

        // Derive the exact key the pipeline will use for the sole unit.
        let mut doc = parse(src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let batches = build_batches(&doc, &opts, None, &crate::unit::html_outcomes(&doc));
        let batch = batches.first().expect("at least one batch");
        let unit = batch.units.first().expect("at least one unit").clone();
        let key_ctx = CacheKeyContext::for_run(&opts, translator.fingerprint());
        // ti c02f69: the instruction axis comes from the batch the packer
        // built, exactly as `process_one_batch` derives it.
        let key = key_ctx.key_for(
            &opts,
            &unit,
            InstructionDigest::for_batch(batch),
            CohortDigest::for_batch(batch),
        );

        // Poison it with an entry that cannot survive re-validation.
        let cache = InMemoryCache::new();
        cache
            .put(
                key.clone(),
                UnitResult {
                    unit_id: unit.unit_id.clone(),
                    output_kind: OutputKind::FailedNeedsFallback,
                    translated_payload: String::new(),
                    warnings: Vec::new(),
                },
            )
            .expect("seed");

        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let rows = attempts_of(&out, TARGET);
        assert_eq!(
            rows.len(),
            2,
            "the rejected hit, then one dispatch: {rows:?}"
        );
        assert_eq!(rows[0].attempt_number, 0, "the cache-hit row");
        assert_eq!(rows[1].attempt_number, 1, "the unit's first real attempt");
        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            1,
            "exactly one provider round"
        );
        assert_eq!(out.validation_report.total_retries, 0);
    }
}

// Spec §3.2: an html block with nothing to translate is never silent — the
// run reports it on the same reader-honesty channel as parser skips, its row
// is `preserved`, and it stays out of the unit counters. Row, counter, and
// warning come from one outcome map, so they cannot disagree.
#[cfg(test)]
mod html_outcome_report_tests {
    // The moved tests keep the exact scope they had in `pipeline.rs`.
    use crate::llm::{OutputKind, TranslationBatch, TranslationBatchResult};
    use crate::pipeline::*;

    #[tokio::test]
    async fn zero_segment_html_block_is_reported_preserved_and_uncounted() {
        use crate::cache::InMemoryCache;

        // Echo translator: return every unit's source unchanged so the
        // paragraph validates and the html accounting is the only variable.
        struct EchoAll;
        #[async_trait::async_trait]
        impl Translator for EchoAll {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let units = batch
                    .units
                    .iter()
                    .map(|u| UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::Preserved,
                        translated_payload: u.source_payload.clone(),
                        warnings: Vec::new(),
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        // A comment-only html block: extraction reaches no text node, so no
        // unit is built for it (`HtmlOutcome::PreservedZeroSegment`).
        let src = "<!-- note -->\n\npara\n";
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(src, &opts, &EchoAll, cache_dyn)
            .await
            .expect("pipeline completes");

        let notes = &out.validation_report.skipped_source_nodes;
        assert!(
            notes.iter().any(|w| w.contains("no translatable text")),
            "the zero-segment html block must be reported: {notes:?}",
        );

        let html_row = out
            .alignment_map
            .blocks
            .iter()
            .find(|b| b.block_kind == "html")
            .expect("the html block keeps an alignment row");
        assert_eq!(html_row.fallback_status, FallbackStatus::Preserved);
        assert_eq!(
            out.alignment_map.validation_summary.total_units, 1,
            "only the paragraph is a unit — the zero-segment html block is uncounted",
        );
    }
}

// ti 13e145: the html-dominance note is a *document*-level entry on the same
// reader-honesty channel, and it is a note — the run still completes and still
// writes its outputs. These pin both halves: that it reaches the report at all,
// and that it does not fire on the ordinary document.
#[cfg(test)]
mod html_dominance_report_tests {
    use crate::llm::{OutputKind, TranslationBatch, TranslationBatchResult};
    use crate::pipeline::*;

    /// Echoes every unit's source payload back, so accounting is the only
    /// variable. Same shape as the zero-segment test's translator above.
    struct EchoAll;
    #[async_trait::async_trait]
    impl Translator for EchoAll {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let units = batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Preserved,
                    translated_payload: u.source_payload.clone(),
                    warnings: Vec::new(),
                })
                .collect();
            Ok(TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
    }

    async fn notes_for(src: &str) -> Vec<String> {
        use crate::cache::InMemoryCache;
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(src, &opts, &EchoAll, cache_dyn)
            .await
            .expect("the note never aborts the run");
        out.validation_report.skipped_source_nodes
    }

    #[tokio::test]
    async fn an_html_dominated_document_still_completes_and_says_so() {
        let block = format!("<div>\n{}\n</div>\n\n", "x".repeat(400));
        let src = format!("{block}{block}Prose.\n");
        let notes = notes_for(&src).await;
        assert!(
            notes.iter().any(|n| n.contains("raw HTML blocks")),
            "the document-level note must reach the report: {notes:?}",
        );
        assert!(
            notes.iter().any(|n| n.contains("490d97")),
            "and it must cite the ticket: {notes:?}",
        );
        assert!(
            notes.iter().any(|n| n.contains("input_format")),
            "the note must name the library entry point (spec §6; ti d990b6's \
             vocabulary rule): {notes:?}",
        );
        assert!(
            notes.iter().all(|n| !n.contains("not implemented")),
            "the feature exists; the note must not lie in that direction: {notes:?}",
        );
        assert!(
            notes.iter().all(|n| !n.contains("--")),
            "never CLI vocabulary — no flag has a name here (ti d990b6): {notes:?}",
        );
    }

    #[tokio::test]
    async fn an_ordinary_document_gains_no_note() {
        let src = "# Title\n\nA paragraph of prose that carries the document.\n";
        assert!(
            notes_for(src).await.is_empty(),
            "no note on a document the guard is not about",
        );
    }
}
