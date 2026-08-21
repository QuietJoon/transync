//! Post-regen finalization — the DCR-0004 full-reparse fallback cascade.
//!
//! [`finalize_regen_with_reparse_policy`] regenerates the translated Markdown
//! from the accepted units and runs the full-document reparse gate, applying
//! the caller's [`FullReparseFailure`] policy when that gate rejects the
//! output. The staged widening (attributed blocks → their immediate top-level
//! neighbors → every block) lives here, together with the projection helpers
//! the regenerator consumes.
//!
//! TRACE: SCN-14
//! TRACE: R0004-0001

use crate::FallbackStatus;
use crate::FullReparseFailure;
use crate::id::BlockId;
use crate::regen::regenerate;
use crate::validate::{ValidatedBatch, ValidatedUnit};
use std::collections::HashMap;

/// Regenerate the translated MD and run the post-regen full-document
/// reparse, applying the caller's [`FullReparseFailure`] policy when
/// the reparse rejects the output.
///
/// On `FallbackPerBlock`, this runs the DCR-0004 three-stage cascade,
/// mutating `accepted` to flip divergent units to
/// `FallbackStatus::FallbackSource` (with `accepted_payload = None`, so
/// [`regenerate`] splices source bytes): stage 1 falls back exactly the
/// attributed blocks; if the reparse still fails, stage 2 widens to the
/// flagged blocks' immediate top-level neighbors; if that also fails,
/// stage 3 escalates to [`FullReparseFailure::FallbackAll`]. This arm
/// never returns `Err`.
///
/// On `FallbackAll`, this flips every accepted unit to fallback and
/// regenerates without re-checking — the result is the source bytes,
/// which are structurally identical to the source by construction.
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
pub(crate) fn finalize_regen_with_reparse_policy(
    doc: &crate::parser::Document,
    accepted: &mut HashMap<BlockId, ValidatedUnit>,
    policy: FullReparseFailure,
) -> Result<
    (
        String,
        crate::regen::BlockOffsets,
        Vec<crate::validate::ValidatedBatch>,
    ),
    crate::validate::full_reparse::ReparseFailure,
> {
    let (md, offsets, final_validated) = regen_pass(doc, accepted);
    let failure = match crate::validate::full_reparse::reparse_full(doc, &md, &offsets) {
        Ok(()) => return Ok((md, offsets, final_validated)),
        Err(f) => f,
    };

    match policy {
        // EXT-2026-07 P1-4 (R0008-0004): keep the failure typed so
        // `run_pipeline` can evict the implicated keys before mapping it
        // to the `"full reparse failed: {reason}"` error.
        FullReparseFailure::Hard => Err(failure),
        FullReparseFailure::FallbackPerBlock => {
            tracing::warn!(
                target: "transync::pipeline",
                "full reparse rejected output ({}); marking {} block(s) as fallback (stage 1: minimal) and re-running regen",
                failure.reason,
                failure.divergent_source_blocks.len()
            );
            downgrade_units(accepted, failure.divergent_source_blocks.iter());
            let (md, offsets, final_validated) = regen_pass(doc, accepted);
            let stage1_failure =
                match crate::validate::full_reparse::reparse_full(doc, &md, &offsets) {
                    Ok(()) => return Ok((md, offsets, final_validated)),
                    Err(f) => f,
                };

            // R0006: byte-offset attribution names the neighbor on
            // boundary contamination; widen so the actual culprit is
            // in scope. See DCR-0004.
            let widen_seeds = if stage1_failure.divergent_source_blocks.is_empty() {
                &failure.divergent_source_blocks
            } else {
                &stage1_failure.divergent_source_blocks
            };
            let widened = widen_to_neighbors(doc, widen_seeds);
            tracing::warn!(
                target: "transync::pipeline",
                "stage 1 still rejected ({}); widening fallback to {} block(s) (stage 2: neighbors) and re-running regen",
                stage1_failure.reason,
                widened.len()
            );
            downgrade_units(accepted, widened.iter());
            let (md, offsets, final_validated) = regen_pass(doc, accepted);
            let stage2_failure =
                match crate::validate::full_reparse::reparse_full(doc, &md, &offsets) {
                    Ok(()) => return Ok((md, offsets, final_validated)),
                    Err(f) => f,
                };

            tracing::warn!(
                target: "transync::pipeline",
                "stage 2 still rejected ({}); escalating to FallbackAll (stage 3) — every block returns source bytes",
                stage2_failure.reason
            );
            Ok(fall_back_all(doc, accepted))
        }
        FullReparseFailure::FallbackAll => {
            tracing::warn!(
                target: "transync::pipeline",
                "full reparse rejected output ({}); falling back all blocks to source bytes",
                failure.reason
            );
            Ok(fall_back_all(doc, accepted))
        }
    }
}

/// One regen-then-project pass over the current `accepted` map.
fn regen_pass(
    doc: &crate::parser::Document,
    accepted: &HashMap<BlockId, ValidatedUnit>,
) -> (
    String,
    crate::regen::BlockOffsets,
    Vec<crate::validate::ValidatedBatch>,
) {
    let final_validated = collect_validated(doc, accepted);
    // Spec §3.2: regen consumes accepted payloads only — a unit that fell
    // back has none, and its absence from the map is what makes regen splice
    // the source bytes.
    let payloads: HashMap<BlockId, String> = accepted
        .iter()
        .filter_map(|(id, vu)| vu.accepted_payload.clone().map(|p| (id.clone(), p)))
        .collect();
    let (md, offsets) = regenerate(doc, &payloads);
    (md, offsets, final_validated)
}

/// Stage 2 of `FallbackPerBlock`: seeds plus their immediate top-level
/// neighbors. A seed the document does not contain is silently skipped.
/// See DCR-0004 for the boundary-contamination rationale.
///
/// TRACE: R0006
fn widen_to_neighbors(doc: &crate::parser::Document, seeds: &[BlockId]) -> Vec<BlockId> {
    let top_level: Vec<&BlockId> = crate::regen::top_level_blocks(&doc.blocks)
        .map(|b| &b.block_id)
        .collect();

    let mut indices = std::collections::BTreeSet::new();
    for seed in seeds {
        let Some(i) = top_level.iter().position(|id| *id == seed) else {
            continue;
        };
        indices.insert(i);
        if i > 0 {
            indices.insert(i - 1);
        }
        if i + 1 < top_level.len() {
            indices.insert(i + 1);
        }
    }
    indices.into_iter().map(|i| top_level[i].clone()).collect()
}

/// Flip the named units to `FallbackStatus::FallbackSource` (with no
/// payload, so [`regenerate`] splices source bytes).
fn downgrade_units<'a>(
    accepted: &mut HashMap<BlockId, ValidatedUnit>,
    ids: impl IntoIterator<Item = &'a BlockId>,
) {
    for id in ids {
        if let Some(vu) = accepted.get_mut(id) {
            vu.final_status = FallbackStatus::FallbackSource;
            vu.accepted_payload = None;
        }
    }
}

/// Mark every accepted unit as `FallbackSource` and regenerate. Always
/// succeeds structurally — the result is the source document.
fn fall_back_all(
    doc: &crate::parser::Document,
    accepted: &mut HashMap<BlockId, ValidatedUnit>,
) -> (
    String,
    crate::regen::BlockOffsets,
    Vec<crate::validate::ValidatedBatch>,
) {
    for vu in accepted.values_mut() {
        vu.final_status = FallbackStatus::FallbackSource;
        vu.accepted_payload = None;
    }
    regen_pass(doc, accepted)
}

/// Project the per-unit `accepted` map into the doc's source-block order
/// so the regenerator sees one cohesive `ValidatedBatch`.
fn collect_validated(
    doc: &crate::parser::Document,
    accepted: &HashMap<BlockId, ValidatedUnit>,
) -> Vec<ValidatedBatch> {
    let mut units: Vec<ValidatedUnit> = Vec::new();
    for block in &doc.blocks {
        if let Some(vu) = accepted.get(&block.block_id) {
            units.push(vu.clone());
        }
    }
    if units.is_empty() {
        return Vec::new();
    }
    vec![ValidatedBatch {
        units,
        // Per-round provider-envelope attribution; the final projection is
        // assembled from already-finalized units, so there is no fault to
        // carry here.
        batch_fault: None,
    }]
}

#[cfg(test)]
mod reparse_policy_tests {
    use super::*;
    use crate::parser::parse;
    use crate::test_fixtures::{EVIL_SOURCE_MD, evil_accepted};

    #[test]
    fn hard_policy_returns_err_on_divergence() {
        let doc = parse(EVIL_SOURCE_MD).expect("source parses");
        let mut accepted = evil_accepted(&doc);
        // EXT-2026-07 P1-4: `finalize_…` now returns the typed
        // `ReparseFailure` on Hard so the pipeline can evict the
        // implicated keys before mapping it to a `TransyncError`. The
        // `"full reparse failed: {reason}"` string mapping is pinned
        // out-of-crate, by the facade's
        // `boundary_v02::hard_failure_maps_error_and_evicts_implicated_keys`
        // (it asserts the prefix on the `TransyncError::Validation` a Hard
        // run surfaces).
        let failure =
            finalize_regen_with_reparse_policy(&doc, &mut accepted, FullReparseFailure::Hard)
                .expect_err("Hard policy must surface the failure");
        assert!(
            failure.reason.contains("regenerated block count"),
            "got: {}",
            failure.reason
        );
        assert!(
            !failure.divergent_source_blocks.is_empty(),
            "expected implicated source blocks in the failure"
        );
    }

    #[test]
    fn fallback_per_block_recovers_and_marks_unit_fallback() {
        let doc = parse(EVIL_SOURCE_MD).expect("source parses");
        let mut accepted = evil_accepted(&doc);
        let first_id = doc
            .blocks
            .first()
            .map(|b| b.block_id.clone())
            .expect("first top-level block");

        let (md, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            FullReparseFailure::FallbackPerBlock,
        )
        .expect("FallbackPerBlock must recover");
        assert!(
            md.contains("para 1\n"),
            "regen should use source bytes for fallback block; got:\n{md}"
        );
        assert!(
            !md.contains("uninvited"),
            "evil payload must not survive fallback; got:\n{md}"
        );
        let vu = accepted.get(&first_id).expect("first unit still present");
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
    }

    /// R0006: widen_to_neighbors returns the seed plus its immediate
    /// preceding and following top-level blocks, in source order, with
    /// no duplicates.
    #[test]
    fn widen_to_neighbors_returns_immediate_neighbors() {
        let src = "para A\n\npara B\n\npara C\n\npara D\n";
        let doc = parse(src).expect("source parses");
        let top: Vec<BlockId> = doc.blocks.iter().map(|b| b.block_id.clone()).collect();
        assert_eq!(top.len(), 4);

        // Seed: middle block. Expect [prev, seed, next].
        let widened = widen_to_neighbors(&doc, std::slice::from_ref(&top[1]));
        assert_eq!(
            widened,
            vec![top[0].clone(), top[1].clone(), top[2].clone()]
        );

        // Seed: first block. Expect [seed, next] (no prev).
        let widened = widen_to_neighbors(&doc, std::slice::from_ref(&top[0]));
        assert_eq!(widened, vec![top[0].clone(), top[1].clone()]);

        // Seed: last block. Expect [prev, seed] (no next).
        let widened = widen_to_neighbors(&doc, std::slice::from_ref(&top[3]));
        assert_eq!(widened, vec![top[2].clone(), top[3].clone()]);

        // Multiple seeds with overlapping neighborhoods de-duplicate
        // and stay in source order.
        let widened = widen_to_neighbors(&doc, &[top[0].clone(), top[2].clone()]);
        assert_eq!(
            widened,
            vec![
                top[0].clone(),
                top[1].clone(),
                top[2].clone(),
                top[3].clone()
            ]
        );

        // Empty seed list returns empty.
        assert!(widen_to_neighbors(&doc, &[]).is_empty());

        // Unknown BlockId is silently skipped (not present in doc).
        let unknown = BlockId("p-9999".to_string());
        assert!(widen_to_neighbors(&doc, std::slice::from_ref(&unknown)).is_empty());

        // R0006's other skip case — a seed that is not a top-level block —
        // is unrepresentable: the parser is leaf-block, so every block in
        // `doc.blocks` is top-level and a list item is a top-level seed like
        // any other. The unknown-id assertion above is therefore the only
        // reachable skip path, and this documents why there is no second one.
        let items = parse("- a\n- b\n").expect("nested parses");
        let item_seed = items.blocks[0].block_id.clone();
        assert_eq!(
            widen_to_neighbors(&items, std::slice::from_ref(&item_seed)),
            items
                .blocks
                .iter()
                .map(|b| b.block_id.clone())
                .collect::<Vec<_>>(),
            "a list item is a top-level seed and widens to its neighbors",
        );
    }

    #[test]
    fn fallback_all_skips_translation() {
        let doc = parse(EVIL_SOURCE_MD).expect("source parses");
        let mut accepted = evil_accepted(&doc);
        let (md, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            FullReparseFailure::FallbackAll,
        )
        .expect("FallbackAll always succeeds");
        assert_eq!(md, EVIL_SOURCE_MD);
        for vu in accepted.values() {
            assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
            assert!(vu.accepted_payload.is_none());
        }
    }
}

// Spec §4.3: the residual the per-list item count exists for. `per_kind::
// check_list` compares each unit's payload topology against its own source,
// so it can only see a divergence a single payload carries on its face. It
// is blind to a SPLICE-ADJACENCY effect: two payloads that are each a valid
// one-item list on their own, but that merge into a differently-shaped list
// once regen concatenates them. The top-level label sequence and block count
// are both unchanged, so the first two reparse scans pass too — only the
// item count catches it, and the run still completes (honest fallback, never
// corrupt output).
#[cfg(test)]
mod list_item_count_pipeline_tests {
    use super::*;
    // The moved tests keep the exact scope they had in `pipeline.rs`.
    use crate::cache::InMemoryCache;
    use crate::llm::{OutputKind, TranslationBatch, TranslationBatchResult};
    use crate::pipeline::*;

    /// One two-item bullet list → units `li-0001`, `li-0002`.
    const SRC: &str = "- alpha\n- bravo\n";

    /// Translates both items, and indents the SECOND item's marker by two
    /// spaces. `"  - 브라보"` reparses on its own as a one-item list with the
    /// same topology fingerprint as its source (`per_kind::check_list` and
    /// the fragment reparse both accept it), but spliced after `"- 알파"` the
    /// two-space indent makes it a SUB-list of the first item: the
    /// regenerated document has one top-level `List` with one direct item.
    struct IndentSecondItem;
    #[async_trait::async_trait]
    impl Translator for IndentSecondItem {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let units = batch
                .units
                .iter()
                .map(|u| {
                    let payload = match u.unit_id.0.as_str() {
                        "li-0001" => "- 알파".to_string(),
                        "li-0002" => "  - 브라보".to_string(),
                        other => panic!("unexpected unit {other}"),
                    };
                    UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::Translated,
                        translated_payload: payload,
                        warnings: Vec::new(),
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

    #[tokio::test]
    async fn splice_adjacency_item_collapse_falls_back_and_completes() {
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &IndentSecondItem, cache_dyn)
            .await
            .expect("FallbackPerBlock degrades, never aborts");

        // Both payloads cleared every per-unit layer — the item-count guard,
        // not `per_kind`, is what caught this.
        for id in ["li-0001", "li-0002"] {
            let r = out
                .validation_report
                .per_unit
                .iter()
                .find(|r| r.unit_id.0 == id)
                .unwrap_or_else(|| panic!("no report row for {id}"));
            assert_eq!(
                r.attempts.len(),
                1,
                "{id} was never retried: {:?}",
                r.attempts
            );
            assert_eq!(
                r.attempts[0].rejected_by, None,
                "{id} must pass every per-unit validator: {:?}",
                r.attempts
            );
        }

        // The full-document gate downgraded exactly the list's item blocks…
        let mut fallbacks: Vec<&str> = out
            .validation_report
            .full_reparse_fallbacks
            .iter()
            .map(|id| id.0.as_str())
            .collect();
        fallbacks.sort_unstable();
        assert_eq!(fallbacks, vec!["li-0001", "li-0002"]);

        // …so both rows report the fallback honestly and the output is the
        // source list, structurally intact.
        for id in ["li-0001", "li-0002"] {
            let row = out
                .alignment_map
                .blocks
                .iter()
                .find(|b| b.source_block_id.0 == id)
                .unwrap_or_else(|| panic!("no alignment block for {id}"));
            assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
        }
        assert_eq!(out.translated_document, SRC);
    }
}
