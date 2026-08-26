//! Reassembling a split table from its translated row windows (DCR-0026 §4).
//!
//! The splitter (`unit::split`) replaced one oversize table unit with several
//! window units before round one. Everything between here and there treated
//! those windows as ordinary units — they were dispatched, validated, retried
//! and cached individually — and this module is where they stop being units
//! and become a block again.
//!
//! It runs between `aggregate_batch_results` and
//! `finalize_regen_with_reparse_policy`, which is the one point where every
//! window has a final per-unit verdict and nothing downstream has yet read
//! `accepted`. That ordering is load-bearing: **a window id must never reach
//! `regen::regenerate`, the alignment map, or the DOM** (DCR-0026 rule 3). The
//! alignment map still carries exactly one row per source block, and its
//! schema version does not move.
//!
//! What the merge does, per parent table:
//!
//! 1. Take each window's accepted payload, or — when that window finalized
//!    `fallback_source` — its own **source** rows (OQ-1(b), resolved
//!    2026-08-09).
//! 2. `regen::regenerate_table` splices them: window 0 whole, later windows
//!    minus their header and delimiter rows.
//! 3. Re-inspect the merged table and require the **source** block's column
//!    count, alignment, and total body-row count. Layered validation
//!    (invariant 5) holds end to end: no window result reaches the document
//!    without both its own fragment proof and this whole-table proof.
//! 4. Replace the window entries in `accepted` with one parent entry, whose
//!    status is the merge of its windows'.
//!
//! TRACE: SCN-03
//! TRACE: DCR-0026

use crate::FallbackStatus;
use crate::id::BlockId;
use crate::llm::{InputMode, TranslationBatch};
use crate::parser::Document;
use crate::structure::inspect_table;
use crate::validate::ValidatedUnit;
use std::collections::HashMap;
use transync_syntax::regen::regenerate_table;

/// One window of a split table, as the run packed it.
struct Window {
    unit_id: BlockId,
    /// The window's own source markdown — a complete mini-table. This is what
    /// a terminally-failed window contributes to the merge, which is why the
    /// plan keeps it rather than re-deriving it: the row boundaries are the
    /// packer's and are not recoverable from the source alone.
    source_payload: String,
}

/// The run's row-window split, read back off the packed batches.
///
/// Reconstructible rather than threaded through: `unit::build_batches` returns
/// batches, not a plan, and every fact the merge needs is on the window units
/// themselves ([`InputMode::TableRowWindow`]). That is why the variant carries
/// `parent_block_id` and the ordinals — see DCR-0026 §2.
#[derive(Default)]
pub(crate) struct SplitPlan {
    /// Parent block id → its windows, in window order.
    by_parent: HashMap<BlockId, Vec<Window>>,
}

impl SplitPlan {
    /// Read the plan off `batches`. Empty — and every operation a no-op — for
    /// a run in which nothing split, which is the common case.
    pub(crate) fn of_batches(batches: &[TranslationBatch]) -> Self {
        let mut by_parent: HashMap<BlockId, Vec<(u32, Window)>> = HashMap::new();
        for unit in batches.iter().flat_map(|b| &b.units) {
            if let InputMode::TableRowWindow {
                parent_block_id,
                window_index,
                ..
            } = &unit.input_mode
            {
                by_parent.entry(parent_block_id.clone()).or_default().push((
                    *window_index,
                    Window {
                        unit_id: unit.unit_id.clone(),
                        source_payload: unit.source_payload.clone(),
                    },
                ));
            }
        }
        // Batches are dispatched concurrently and a parent's windows can be
        // spread across them, so source row order is restored from the
        // ordinal rather than assumed from iteration order.
        Self {
            by_parent: by_parent
                .into_iter()
                .map(|(parent, mut windows)| {
                    windows.sort_by_key(|(index, _)| *index);
                    (parent, windows.into_iter().map(|(_, w)| w).collect())
                })
                .collect(),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.by_parent.is_empty()
    }

    /// Every window unit id of `parent`, in source row order. Empty for a
    /// block that was never split.
    ///
    /// This is the parent→window mapping eviction needs: a split parent was
    /// never dispatched and has no cache key of its own, so a full-reparse
    /// downgrade or `Hard` failure naming it must evict its windows' keys
    /// instead (DCR-0026 §4).
    pub(crate) fn window_ids_of(&self, parent: &BlockId) -> Vec<BlockId> {
        self.by_parent
            .get(parent)
            .map(|ws| ws.iter().map(|w| w.unit_id.clone()).collect())
            .unwrap_or_default()
    }
}

/// What a merge pass found, for the caller to act on.
pub(crate) struct MergeOutcome {
    /// Parents whose merged fragment failed the whole-table check even though
    /// every window passed its own. The caller evicts these parents' window
    /// cache keys so the state is not replayed.
    pub(crate) faulted_parents: Vec<BlockId>,
    /// The window entries this pass removed from `accepted`, in source row
    /// order per parent.
    ///
    /// They leave `accepted` because regen must never see a window id
    /// (DCR-0026 rule 3), but they are real dispatches with real attempts, so
    /// they keep their own `UnitValidationRecord` rows — handed back here for
    /// `build_validation_report` rather than left in the map regen reads
    /// (DCR-0026 §4).
    pub(crate) window_records: Vec<ValidatedUnit>,
}

/// Replace every split table's window entries in `accepted` with one merged
/// parent entry.
///
/// A no-op when `plan` is empty. Window entries are always removed, whatever
/// the outcome — leaving one behind would put a window id in front of
/// `regen::regenerate`, which is the one thing this module exists to prevent.
///
/// TRACE: DCR-0026
pub(crate) fn merge_row_windows(
    doc: &Document,
    plan: &SplitPlan,
    accepted: &mut HashMap<BlockId, ValidatedUnit>,
) -> MergeOutcome {
    let mut faulted_parents = Vec::new();
    let mut window_records: Vec<ValidatedUnit> = Vec::new();
    if plan.is_empty() {
        return MergeOutcome {
            faulted_parents,
            window_records,
        };
    }
    // Only a split parent's source is ever read below, and `block_payload`
    // materializes an owned `String` per block it is asked about. Filtering on
    // the plan first keeps a document with one split table from allocating a
    // copy of every other block's markdown (R0009-0072). The early return
    // above already spares a run in which nothing split; this spares the run in
    // which something did.
    let source_of: HashMap<&BlockId, String> = doc
        .blocks
        .iter()
        .filter(|b| plan.by_parent.contains_key(&b.block_id))
        .map(|b| (&b.block_id, transync_syntax::outcome::block_payload(doc, b)))
        .collect();

    for (parent, windows) in &plan.by_parent {
        let entries: Vec<Option<ValidatedUnit>> = windows
            .iter()
            .map(|w| accepted.remove(&w.unit_id))
            .collect();
        let Some(source) = source_of.get(parent) else {
            // The parent block is not in this document — unreachable through
            // `run_pipeline`, which builds the plan from this document's own
            // batches. Nothing to merge into, and the windows are already
            // out of `accepted`, so the block simply falls back to source.
            continue;
        };

        let mut statuses: Vec<FallbackStatus> = Vec::with_capacity(windows.len());
        let mut payloads: Vec<String> = Vec::with_capacity(windows.len());
        let mut warnings: Vec<String> = Vec::new();
        for (window, entry) in windows.iter().zip(entries) {
            match entry {
                // A window that finalized with a payload contributes it.
                Some(vu) => {
                    // The report keeps the window's own verdict, unchanged by
                    // whatever the merge decides about the block.
                    window_records.push(vu.clone());
                    warnings.extend(vu.warnings);
                    match vu.accepted_payload {
                        Some(payload) => {
                            statuses.push(vu.final_status);
                            payloads.push(payload);
                        }
                        // `fallback_source` carries no payload by convention
                        // (regen splices source bytes for it), so the window's
                        // own source rows are what it contributes — OQ-1(b).
                        None => {
                            statuses.push(FallbackStatus::FallbackSource);
                            payloads.push(window.source_payload.clone());
                        }
                    }
                }
                // Defensive: every dispatched unit finalizes, so a missing
                // entry is unreachable. Treated as a failed window rather
                // than dropped, so the merged table keeps every source row.
                None => {
                    statuses.push(FallbackStatus::FallbackSource);
                    payloads.push(window.source_payload.clone());
                }
            }
        }

        let merged_status = merge_status(&statuses);
        // Every window failed: the block IS its source, exactly, and the
        // whole-block convention for that is `accepted_payload: None` — regen
        // then splices the source bytes byte-identically, which is what the
        // DCR-0004 cascade's "structurally identical by construction"
        // reasoning rests on.
        if merged_status == FallbackStatus::FallbackSource {
            accepted.insert(
                parent.clone(),
                ValidatedUnit {
                    unit_id: parent.clone(),
                    final_status: FallbackStatus::FallbackSource,
                    accepted_payload: None,
                    rejected_by: None,
                    rejection_reason: None,
                    warnings: crate::validate::bounded_warnings(&warnings),
                },
            );
            continue;
        }

        let refs: Vec<&str> = payloads.iter().map(String::as_str).collect();
        let merged = regenerate_table(&refs);
        if !merged_matches_source(&merged, source) {
            // DCR-0026 §4: the merged fragment failed the whole-table check
            // even though every window passed its own — an engine fault, not a
            // model fault. It takes the DCR-0016 direct-fallback shape: no
            // rejecting layer, no reason, no retry burned, and the caller
            // evicts every window key so the state is not replayed.
            tracing::warn!(
                target: "transync::pipeline",
                "row-window merge for {parent} did not reproduce the source table's shape; \
                 falling back to the source table and evicting its {} window cache entries",
                windows.len(),
            );
            warnings.push(format!(
                "row-window merge for {parent} produced a table that does not match the \
                 source block's shape; the source table is emitted instead"
            ));
            faulted_parents.push(parent.clone());
            accepted.insert(
                parent.clone(),
                ValidatedUnit {
                    unit_id: parent.clone(),
                    final_status: FallbackStatus::FallbackSource,
                    accepted_payload: None,
                    rejected_by: None,
                    rejection_reason: None,
                    warnings: crate::validate::bounded_warnings(&warnings),
                },
            );
            continue;
        }

        if merged_status == FallbackStatus::PartiallyTranslated
            && statuses.contains(&FallbackStatus::FallbackSource)
        {
            let failed: Vec<&str> = windows
                .iter()
                .zip(&statuses)
                .filter(|(_, s)| **s == FallbackStatus::FallbackSource)
                .map(|(w, _)| w.unit_id.0.as_str())
                .collect();
            warnings.push(format!(
                "row window(s) {} fell back to source rows; the rest of {parent} is translated",
                failed.join(", ")
            ));
        }

        accepted.insert(
            parent.clone(),
            ValidatedUnit {
                unit_id: parent.clone(),
                final_status: merged_status,
                accepted_payload: Some(merged),
                rejected_by: None,
                rejection_reason: None,
                warnings: crate::validate::bounded_warnings(&warnings),
            },
        );
    }

    MergeOutcome {
        faulted_parents,
        window_records,
    }
}

/// The parent's status from its windows' (DCR-0026 §4, OQ-1(b)).
///
/// Unanimity carries: all-translated is translated, all-preserved is
/// preserved, and all-failed is `fallback_source` — the block really is its
/// source content exactly, which is the strict reading `fallback_source`
/// promises and is also what the whole-block path would have produced.
/// Anything mixed is `PartiallyTranslated`, the existing marker for exactly
/// this state (invariant 6): the paid-for windows are kept, the failed ones
/// contribute their source rows, and the block says so on every wire surface.
fn merge_status(statuses: &[FallbackStatus]) -> FallbackStatus {
    let first = match statuses.first() {
        Some(s) => *s,
        None => return FallbackStatus::FallbackSource,
    };
    if statuses.iter().all(|s| *s == first)
        && matches!(
            first,
            FallbackStatus::Translated | FallbackStatus::Preserved | FallbackStatus::FallbackSource
        )
    {
        return first;
    }
    FallbackStatus::PartiallyTranslated
}

/// The merged fragment must be one table with the **source** block's column
/// count, per-column alignment, and total body-row count.
///
/// This is the merge-time check DCR-0026 §3 adds on top of the per-window
/// layers. It is deliberately taken through `inspect_table`, the same oracle
/// `unit::payload` fingerprints the source with and `validate::per_kind`
/// re-derives from a translated payload, so the three cannot drift.
fn merged_matches_source(merged: &str, source: &str) -> bool {
    match (inspect_table(merged), inspect_table(source)) {
        (Some(m), Some(s)) => m == s,
        // A source that does not inspect as a table cannot have been split
        // (the splitter refuses it), so this is unreachable; a merged
        // fragment that does not is exactly what this check is for.
        _ => false,
    }
}

// DCR-0026 SL-103. The merge's four outcomes — clean, per-window partial,
// all-failed, and the engine-fault path — plus the property that no window id
// survives it.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::unit::{build_batches, html_outcomes};
    use crate::validate::ValidatedUnit;

    fn table(rows: usize) -> String {
        let mut src = String::from("| id | name | kind | note |\n|---|---|---|---|");
        for i in 1..=rows {
            src.push_str(&format!(
                "\n| {i} | item number {i} | widget | a note about item {i} |"
            ));
        }
        src
    }

    /// The run's own batching, with a profile that splits.
    fn split_run(src: &str) -> (Document, Vec<TranslationBatch>) {
        let mut doc = crate::parser::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let mut profile = crate::profile::default_profile();
        profile.constraints.default_table_strategy = Some("row-window-first".to_string());
        profile.batching.target_output_tokens = Some(400);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &html_outcomes(&doc));
        (doc, batches)
    }

    /// `accepted` as a run would have it just before the merge: one entry per
    /// window, each carrying `payload_of(window_source)`.
    fn accepted_from(
        batches: &[TranslationBatch],
        mut verdict: impl FnMut(&BlockId, &str) -> ValidatedUnit,
    ) -> HashMap<BlockId, ValidatedUnit> {
        batches
            .iter()
            .flat_map(|b| &b.units)
            .map(|u| (u.unit_id.clone(), verdict(&u.unit_id, &u.source_payload)))
            .collect()
    }

    fn translated(id: &BlockId, payload: &str) -> ValidatedUnit {
        ValidatedUnit {
            unit_id: id.clone(),
            final_status: FallbackStatus::Translated,
            accepted_payload: Some(payload.to_string()),
            rejected_by: None,
            rejection_reason: None,
            warnings: Vec::new(),
        }
    }

    fn failed(id: &BlockId) -> ValidatedUnit {
        ValidatedUnit {
            unit_id: id.clone(),
            final_status: FallbackStatus::FallbackSource,
            accepted_payload: None,
            rejected_by: Some(crate::validate::ValidationLayer::PerKindShape),
            rejection_reason: Some("shape".to_string()),
            warnings: Vec::new(),
        }
    }

    /// The happy path: every window comes back, the table is one block again,
    /// and no window id survives into what regen will see.
    #[test]
    fn every_window_translated_merges_into_one_translated_parent() {
        let src = format!("{}\n", table(40));
        let (doc, batches) = split_run(&src);
        let plan = SplitPlan::of_batches(&batches);
        assert!(!plan.is_empty(), "the fixture must actually split");

        let mut accepted = accepted_from(&batches, translated);
        let outcome = merge_row_windows(&doc, &plan, &mut accepted);
        assert!(outcome.faulted_parents.is_empty());

        assert_eq!(accepted.len(), 1, "one entry, for the source block");
        // The window entries left `accepted` — regen must not see them — but
        // came back for the report, which keeps their own verdicts.
        assert_eq!(
            outcome
                .window_records
                .iter()
                .map(|vu| vu.unit_id.0.clone())
                .collect::<Vec<String>>(),
            plan.window_ids_of(&doc.blocks[0].block_id)
                .iter()
                .map(|id| id.0.clone())
                .collect::<Vec<String>>(),
        );
        let parent = &doc.blocks[0].block_id;
        let vu = accepted
            .get(parent)
            .expect("the parent replaced its windows");
        assert_eq!(vu.final_status, FallbackStatus::Translated);
        assert!(
            !accepted.keys().any(|k| k.0.contains(".w")),
            "no window id may survive the merge (DCR-0026 rule 3)"
        );
        // The identity translation must reproduce the source table exactly.
        assert_eq!(vu.accepted_payload.as_deref(), Some(table(40).as_str()));
    }

    /// OQ-1(b): one terminally-failed window costs its own rows, not the
    /// table. The rest of the paid-for translation survives and the block is
    /// marked.
    #[test]
    fn a_failed_window_contributes_source_rows_and_marks_the_parent() {
        let src = format!("{}\n", table(40));
        let (doc, batches) = split_run(&src);
        let plan = SplitPlan::of_batches(&batches);
        let parent = doc.blocks[0].block_id.clone();
        let window_ids = plan.window_ids_of(&parent);
        assert!(window_ids.len() > 2, "need a middle window to fail");
        let victim = window_ids[1].clone();

        // Every window translates to an uppercased copy of itself except the
        // victim, which fails: the merged table must then hold the victim's
        // rows verbatim and everyone else's uppercased.
        let mut accepted = accepted_from(&batches, |id, payload| {
            if *id == victim {
                failed(id)
            } else {
                translated(id, &payload.to_uppercase())
            }
        });
        let outcome = merge_row_windows(&doc, &plan, &mut accepted);
        assert!(outcome.faulted_parents.is_empty(), "this is not a fault");

        let vu = accepted.get(&parent).expect("merged parent");
        assert_eq!(
            vu.final_status,
            FallbackStatus::PartiallyTranslated,
            "the existing marker for exactly this state"
        );
        let merged = vu.accepted_payload.as_deref().expect("a merged payload");
        // Row counts and shape survive; the failed window's rows are source.
        let (cols, _, rows) = inspect_table(merged).expect("a table");
        assert_eq!((cols, rows), (4, 40));
        assert!(
            merged.contains("| item number 1 |") || merged.contains("item number"),
            "the failed window's source rows are in the table: {merged}"
        );
        assert!(
            merged.contains("ITEM NUMBER"),
            "and the translated windows' rows are too: {merged}"
        );
        assert!(
            vu.warnings.iter().any(|w| w.contains(&victim.0)),
            "the failed window is named: {:?}",
            vu.warnings
        );
    }

    /// Every window failed: the block is its source exactly, so it is
    /// `fallback_source` with no payload — the whole-block convention, which
    /// the DCR-0004 cascade's byte-identity reasoning depends on.
    #[test]
    fn every_window_failing_makes_the_parent_a_plain_source_fallback() {
        let src = format!("{}\n", table(40));
        let (doc, batches) = split_run(&src);
        let plan = SplitPlan::of_batches(&batches);
        let mut accepted = accepted_from(&batches, |id, _| failed(id));

        let outcome = merge_row_windows(&doc, &plan, &mut accepted);
        assert!(outcome.faulted_parents.is_empty());
        let vu = accepted
            .get(&doc.blocks[0].block_id)
            .expect("merged parent");
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            vu.accepted_payload, None,
            "a source fallback carries no payload; regen splices the source bytes"
        );
    }

    /// The engine-fault path: every window passed its own layers, yet the
    /// merged table does not have the source's shape. Direct fallback, no
    /// rejecting layer, and the parent is named for eviction.
    #[test]
    fn a_merged_table_that_lost_the_sources_shape_faults_without_blaming_a_layer() {
        let src = format!("{}\n", table(40));
        let (doc, batches) = split_run(&src);
        let plan = SplitPlan::of_batches(&batches);
        let parent = doc.blocks[0].block_id.clone();
        let victim = plan.window_ids_of(&parent)[1].clone();

        // A payload that is a perfectly good three-column table on its own —
        // it would pass a per-window fragment reparse — but makes the merged
        // table inconsistent.
        let mut accepted = accepted_from(&batches, |id, payload| {
            if *id == victim {
                translated(id, "| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |")
            } else {
                translated(id, payload)
            }
        });
        let outcome = merge_row_windows(&doc, &plan, &mut accepted);

        assert_eq!(outcome.faulted_parents, vec![parent.clone()]);
        let vu = accepted.get(&parent).expect("merged parent");
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert_eq!(vu.accepted_payload, None);
        assert_eq!(
            (vu.rejected_by, vu.rejection_reason.as_deref()),
            (None, None),
            "an engine fault blames no validation layer (DCR-0016 direct fallback)"
        );
        assert!(
            vu.warnings.iter().any(|w| w.contains("does not match")),
            "the fault is on the warnings channel: {:?}",
            vu.warnings
        );
    }

    /// The plan is read back off the batches, so a parent's windows are
    /// ordered by their ordinal even when packing spread them across batches.
    #[test]
    fn the_plan_restores_source_row_order_across_batches() {
        let src = format!("{}\n", table(40));
        let (doc, mut batches) = split_run(&src);
        // Shuffle the batches AND the units inside them; the plan must not
        // care.
        batches.reverse();
        for b in &mut batches {
            b.units.reverse();
        }
        let plan = SplitPlan::of_batches(&batches);
        let parent = doc.blocks[0].block_id.clone();
        let ids = plan.window_ids_of(&parent);
        let mut sorted = ids.clone();
        sorted.sort_by_key(|id| id.0.clone());
        assert_eq!(ids, sorted, "windows come back in source row order");

        let mut accepted = accepted_from(&batches, translated);
        merge_row_windows(&doc, &plan, &mut accepted);
        assert_eq!(
            accepted
                .get(&parent)
                .and_then(|vu| vu.accepted_payload.as_deref()),
            Some(table(40).as_str()),
            "rows land in source order however the batches were shuffled"
        );
    }

    /// A run in which nothing split pays nothing and changes nothing.
    #[test]
    fn a_run_without_windows_is_untouched() {
        let mut doc = crate::parser::parse("a paragraph\n\n| a | b |\n|---|---|\n| 1 | 2 |\n")
            .expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &html_outcomes(&doc));
        let plan = SplitPlan::of_batches(&batches);
        assert!(plan.is_empty());

        let mut accepted = accepted_from(&batches, translated);
        let before: Vec<BlockId> = {
            let mut k: Vec<BlockId> = accepted.keys().cloned().collect();
            k.sort_by_key(|id| id.0.clone());
            k
        };
        merge_row_windows(&doc, &plan, &mut accepted);
        let after: Vec<BlockId> = {
            let mut k: Vec<BlockId> = accepted.keys().cloned().collect();
            k.sort_by_key(|id| id.0.clone());
            k
        };
        assert_eq!(before, after);
    }
}
