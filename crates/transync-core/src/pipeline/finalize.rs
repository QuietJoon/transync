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
    doc: &crate::markdown::Document,
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
    let failure = match layer6_gate(doc, &md, &offsets) {
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
            let stage1_failure = match layer6_gate(doc, &md, &offsets) {
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
            let stage2_failure = match layer6_gate(doc, &md, &offsets) {
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
/// THE one format branch (ti 490d97 wave 4, spec §7): the document-level
/// layer-6 gate, dispatched on `Document.format` — comrak's reparse for
/// Markdown, the scanner rescan for HTML. Both return the same
/// [`crate::validate::full_reparse::ReparseFailure`], which is what keeps
/// the three-stage cascade above format-blind: `downgrade_units` and
/// `widen_to_neighbors` consume only `divergent_source_blocks` (over
/// `regen::top_level_blocks` = `blocks.iter()`), `fall_back_all` consumes
/// neither field, and `reason` reaches only the logs and the Hard-arm
/// error. `walk`/comrak never see an HTML document — this branch is the
/// enforcement (spec §7: "not by adding arms").
fn layer6_gate(
    doc: &crate::markdown::Document,
    regenerated: &str,
    offsets: &crate::regen::BlockOffsets,
) -> Result<(), crate::validate::full_reparse::ReparseFailure> {
    match doc.format {
        crate::id::SourceFormat::Markdown => {
            crate::validate::full_reparse::reparse_full(doc, regenerated, offsets)
        }
        crate::id::SourceFormat::Html => {
            crate::validate::full_rescan_html::full_rescan_html(doc, regenerated, offsets)
        }
    }
}

fn regen_pass(
    doc: &crate::markdown::Document,
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
fn widen_to_neighbors(doc: &crate::markdown::Document, seeds: &[BlockId]) -> Vec<BlockId> {
    let top_level: Vec<&BlockId> = crate::regen::top_level_blocks(&doc.blocks)
        .map(|b| &b.block_id)
        .collect();

    // OI-0042: one pass to index, then O(1) per seed — the linear
    // `position` scan made this O(seeds x blocks), and every comparison was
    // a `BlockId(String)`. `entry().or_insert` preserves `position`'s
    // FIRST-wins answer on a duplicate id; `collect()` would keep the last.
    let mut first_index: HashMap<&BlockId, usize> = HashMap::with_capacity(top_level.len());
    for (i, id) in top_level.iter().enumerate() {
        first_index.entry(*id).or_insert(i);
    }

    let mut indices = std::collections::BTreeSet::new();
    for seed in seeds {
        let Some(&i) = first_index.get(seed) else {
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
    doc: &crate::markdown::Document,
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
    doc: &crate::markdown::Document,
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
    use crate::markdown::parse;
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

    /// OI-0042: the linearized index lookup must keep `position`'s
    /// FIRST-wins answer on a duplicated id. `HashMap::entry().or_insert`
    /// is load-bearing here — `collect()` keeps the LAST index and would
    /// widen around the wrong neighbourhood.
    #[test]
    fn widen_to_neighbors_takes_the_first_of_a_duplicated_id() {
        let mut doc = parse("para A\n\npara B\n\npara C\n\npara D\n").expect("source parses");
        assert_eq!(doc.blocks.len(), 4, "fixture sanity");
        // `id::assign` never emits a duplicate, but `Document` is
        // pub-fields and this lookup must not depend on a uniqueness it is
        // not given.
        let dup = doc.blocks[0].block_id.clone();
        doc.blocks[2].block_id = dup.clone();
        let top: Vec<BlockId> = doc.blocks.iter().map(|b| b.block_id.clone()).collect();
        assert_eq!(
            widen_to_neighbors(&doc, std::slice::from_ref(&dup)),
            vec![top[0].clone(), top[1].clone()],
            "the FIRST occurrence and its one neighbour, not index 2's"
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

// ti 490d97 wave 4: the ONE format branch (spec §7). These tests drive
// `finalize_regen_with_reparse_policy` directly — the pub(crate) seam — with
// intake-built HTML documents and hand-built accepted maps, because no run
// path can construct a `format == Html` document until wave 5's entry point
// and wave 6's flag exist. That unreachability is this wave's hard rule, and
// the acceptance section checks it mechanically.
#[cfg(test)]
mod format_dispatch_tests {
    use super::*;
    use crate::markdown::parse;
    use crate::test_fixtures::{EVIL_SOURCE_MD, evil_accepted};

    /// One `<p>` element block and one rule-T anonymous run. The run is the
    /// lever: a whitespace-only translated segment splices cleanly (segment
    /// count 1 == 1; no tags on either side, so per-unit layer 3's
    /// inventory check passes it too — this exact shape reaches regen in a
    /// real wave-5 run), but the resulting run is textless and dissolves
    /// under rule T. Only the layer-6 twin can see that.
    const HTML_SRC: &str = "<div>\n<p>keep</p>\nnaked run text\n</div>\n";

    fn html_doc() -> crate::markdown::Document {
        transync_syntax::intake::html::parse(HTML_SRC)
    }

    fn unit(id: &BlockId, payload: Option<String>) -> ValidatedUnit {
        ValidatedUnit {
            unit_id: id.clone(),
            final_status: crate::FallbackStatus::Translated,
            accepted_payload: payload,
            rejected_by: None,
            rejection_reason: None,
            warnings: Vec::new(),
        }
    }

    /// Accepted map: the `<p>` honestly translated, the run "translated" to
    /// whitespace. Payloads are JSON segment arrays — the shape wave 2's
    /// re-keyed regen arm decodes for every Html-spelled block.
    fn accepted_with_dissolving_run(
        doc: &crate::markdown::Document,
    ) -> HashMap<BlockId, ValidatedUnit> {
        let p_id = doc.blocks[0].block_id.clone();
        let run_id = doc.blocks[1].block_id.clone();
        let mut accepted = HashMap::new();
        accepted.insert(
            p_id.clone(),
            unit(&p_id, Some(serde_json::to_string(&vec!["유지"]).unwrap())),
        );
        accepted.insert(
            run_id.clone(),
            unit(&run_id, Some(serde_json::to_string(&vec![" "]).unwrap())),
        );
        accepted
    }

    #[test]
    fn an_html_document_takes_the_twin_and_the_twin_names_the_dissolved_run() {
        let doc = html_doc();
        assert_eq!(doc.blocks.len(), 2, "fixture sanity: <p> + rule-T run");
        let mut accepted = accepted_with_dissolving_run(&doc);
        let failure = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::Hard,
        )
        .expect_err("Hard must surface the twin's failure");
        // The twin's vocabulary, never reparse_full's: the dissolved run is
        // a fresh-segmentation count divergence.
        assert!(
            failure.reason.contains("fresh segmentation"),
            "got: {}",
            failure.reason
        );
        assert!(
            !failure.reason.contains("regenerated block count"),
            "reparse_full's phrase must not appear — got: {}",
            failure.reason,
        );
        assert!(
            failure
                .divergent_source_blocks
                .contains(&doc.blocks[1].block_id),
            "got: {:?}",
            failure.divergent_source_blocks,
        );
    }

    #[test]
    fn fallback_per_block_restores_the_run_and_keeps_the_honest_translation() {
        let doc = html_doc();
        let run_id = doc.blocks[1].block_id.clone();
        let mut accepted = accepted_with_dissolving_run(&doc);
        let (out, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::FallbackPerBlock,
        )
        .expect("FallbackPerBlock degrades, never aborts (invariant 6)");
        // Stage 1 downgrades exactly the dissolved run; the re-regen
        // splices its SOURCE bytes back and the twin passes.
        assert!(
            out.contains("유지"),
            "the honest translation survives:\n{out}"
        );
        assert!(
            out.contains("naked run text"),
            "the run is source bytes again:\n{out}"
        );
        let vu = accepted.get(&run_id).expect("run unit still present");
        assert_eq!(vu.final_status, crate::FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
        assert_eq!(
            accepted.get(&doc.blocks[0].block_id).unwrap().final_status,
            crate::FallbackStatus::Translated,
            "the innocent neighbor is NOT downgraded at stage 1",
        );
    }

    #[test]
    fn fallback_all_returns_the_source_bytes_for_an_html_document() {
        // Stage 3's "structurally identical by construction" claim rests on
        // wave 3's identity theorem for HTML — pinned here at the finalize
        // seam: all-fallback regen IS the source document, byte-identical.
        let doc = html_doc();
        let mut accepted = accepted_with_dissolving_run(&doc);
        let (out, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::FallbackAll,
        )
        .expect("FallbackAll always succeeds");
        assert_eq!(out, HTML_SRC);
        for vu in accepted.values() {
            assert_eq!(vu.final_status, crate::FallbackStatus::FallbackSource);
            assert!(vu.accepted_payload.is_none());
        }
    }

    #[test]
    fn a_markdown_document_still_takes_reparse_full_not_the_twin() {
        // The POSITIVE half of §12's "Markdown runs provably still take
        // reparse_full" (the zero-edit green suite is the other half):
        // reparse_full's count-drift phrase appears, and the twin's
        // vocabulary provably does not. The reason strings are the two
        // gates' distinguishing marks — see Task 2's vocabulary contract.
        let doc = parse(EVIL_SOURCE_MD).expect("source parses");
        let mut accepted = evil_accepted(&doc);
        let failure = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::Hard,
        )
        .expect_err("the evil fixture diverges under reparse_full");
        assert!(
            failure.reason.contains("regenerated block count"),
            "got: {}",
            failure.reason
        );
        assert!(
            !failure.reason.contains("tag inventory"),
            "got: {}",
            failure.reason
        );
        assert!(
            !failure.reason.contains("fresh segmentation"),
            "got: {}",
            failure.reason
        );
    }
}
