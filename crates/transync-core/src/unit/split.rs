//! Packing-time row-window split for oversize tables (DCR-0026).
//!
//! Output-aware packing (DCR-0012) can shrink a batch but cannot shrink a
//! unit, so a single block whose estimated response exceeds the output ceiling
//! is packed alone, flagged by the preflight, and aborts the whole run at the
//! provider (ADR-0017). This module is the one prevention-side answer to that,
//! for the one kind whose sub-structure the application already owns and
//! validates: a table is replaced, in place and in document order, by row
//! windows that each carry the real header.
//!
//! Three properties this module must keep, all of them load-bearing:
//!
//! - **A window is a complete GFM table.** Header row, delimiter row, one
//!   contiguous run of body rows — never an isolated cell, never a headerless
//!   fragment (architectural invariant 3). The slicing and the assembly are
//!   `transync_syntax::regen`'s, so this module never reasons about table
//!   syntax itself.
//! - **The split happens exactly once, before round one.** It is packing, not
//!   retrying: no failure, provider signal, or retry round ever re-scopes a
//!   unit afterwards, which is what leaves ADR-0009's verbatim-resubmission
//!   rule untouched rather than amended.
//! - **"Oversize" means one thing.** The ceiling, the reserve, the expansion
//!   factor and the encoder all come from the same helpers the packer and the
//!   preflight read (`batch::effective_output_target`,
//!   `batch::estimate_payload_output_tokens`, `budget::output_ceiling`), so
//!   the three sites cannot disagree (DCR-0026 rule 4).
//!
//! What is deliberately NOT here: any non-table kind (each excluded with its
//! ground in DCR-0026 §7), and any reactive re-split on a provider signal
//! (rejected, not deferred).
//!
//! TRACE: SCN-03
//! TRACE: DCR-0026

use crate::batch::{effective_output_target, estimate_payload_output_tokens, resolve_encoder};
use crate::id::{BlockId, BlockKind};
use crate::llm::{BatchId, InputMode, TokenizerHint, TranslationUnit};
use crate::profile::{ProfileMetadata, TableStrategy, resolve_table_strategy};
use tiktoken_rs::CoreBPE;
use transync_syntax::regen::{TableRows, split_table_rows};

/// Replace every oversize table unit with its row windows, in place and in
/// document order.
///
/// A no-op unless the run has an output ceiling AND the effective table
/// strategy is `row-window-first`; a no-op for every unit that is not a table,
/// fits the ceiling, does not parse as a single table, or has fewer than two
/// body rows to distribute. In each of those cases the unit ships whole and
/// the output-budget preflight still names it if it is over the ceiling —
/// ADR-0017's abort is unchanged for everything this does not split.
///
/// TRACE: DCR-0026
pub(crate) fn split_oversize_tables(
    units: &mut Vec<TranslationUnit>,
    model_id: &str,
    profile: &ProfileMetadata,
    tokenizer_hint: Option<TokenizerHint>,
) {
    if resolve_table_strategy(&profile.constraints) != TableStrategy::RowWindowFirst {
        return;
    }
    let Some(ceiling) = crate::unit::budget::output_ceiling(profile) else {
        return;
    };
    let target = effective_output_target(ceiling);
    let factor = crate::batch::resolve_expansion_factor(profile.batching.output_expansion_factor);
    let bpe = resolve_encoder(tokenizer_hint, model_id);

    // Only pay for the rebuild if something actually splits: the common
    // document has no oversize table at all.
    if !units
        .iter()
        .any(|u| windows_of(u, target, factor, &bpe).is_some())
    {
        return;
    }
    let mut out: Vec<TranslationUnit> = Vec::with_capacity(units.len() + 1);
    for unit in units.drain(..) {
        match windows_of(&unit, target, factor, &bpe) {
            Some(plan) => {
                tracing::debug!(
                    target: "transync::pipeline",
                    "table {} exceeds the effective output target ({target} tokens); \
                     splitting {} body rows into {} row windows",
                    unit.unit_id,
                    plan.iter().map(Vec::len).sum::<usize>(),
                    plan.len(),
                );
                out.extend(window_units(&unit, &plan));
            }
            None => out.push(unit),
        }
    }
    *units = out;
}

/// The row plan for `unit` — one `Vec<String>` of body rows per window — or
/// `None` when this unit is not split at all.
///
/// Every refusal here is a documented one, and each leaves the unit exactly as
/// it was: not a table; a table that fits; a payload that does not parse as
/// one table (`split_table_rows` refuses it, and mis-slicing it would be far
/// worse than not splitting); fewer than two body rows to distribute; and the
/// plan that came out with a single window anyway, which is the same unit
/// under a different id and would only cost the merge step work.
///
/// The single-giant-row case falls out of the last two: a table whose one row
/// is over the target keeps that row, and the preflight names it. That floor
/// is DCR-0026's stated boundary, not a defect.
fn windows_of(
    unit: &TranslationUnit,
    target: usize,
    factor: f64,
    bpe: &CoreBPE,
) -> Option<Vec<Vec<String>>> {
    if !matches!(unit.block_kind, BlockKind::Table) {
        return None;
    }
    // The same question `output_budget_warnings` asks about the same unit.
    if crate::batch::estimate_unit_output_tokens(unit, bpe, factor) <= target {
        return None;
    }
    let rows = split_table_rows(&unit.source_payload)?;
    if rows.body.len() < 2 {
        return None;
    }
    let plan = greedy_plan(&rows, &unit.unit_id, target, factor, bpe);
    (plan.len() > 1).then_some(plan)
}

/// Greedy sizing: body rows accumulate into a window until adding the next one
/// would push the window's estimated response past `target`. Minimum one body
/// row per window, so a row that does not fit on its own still gets a window
/// (and the preflight then names that window).
///
/// The estimate is taken over the window payload the unit will actually carry
/// — header and delimiter included, since they ride with every window — under
/// the id that window will actually ship, so nothing about the measurement is
/// a proxy for the thing measured.
fn greedy_plan(
    rows: &TableRows,
    parent_id: &BlockId,
    target: usize,
    factor: f64,
    bpe: &CoreBPE,
) -> Vec<Vec<String>> {
    let mut plan: Vec<Vec<String>> = Vec::new();
    let mut current: Vec<String> = Vec::new();
    for row in &rows.body {
        if current.is_empty() {
            current.push(row.clone());
            continue;
        }
        current.push(row.clone());
        let id = window_unit_id(parent_id, plan.len() as u32);
        if estimate_payload_output_tokens(&rows.window(&current), &id.0, bpe, factor) > target {
            // The row that broke the window starts the next one.
            let overflow = current.pop().expect("just pushed");
            plan.push(std::mem::take(&mut current));
            current.push(overflow);
        }
    }
    if !current.is_empty() {
        plan.push(current);
    }
    plan
}

/// One `TranslationUnit` per window, in source row order.
///
/// Each is an ordinary unit from birth (DCR-0026 §5): it carries its own
/// payload, its own constraints taken from that payload, and the parent's
/// context. Nothing about it is special-cased downstream until the merge step
/// reads [`InputMode::TableRowWindow`].
///
/// **`source_hash` is the WINDOW's own bytes, not the parent's** — a
/// deviation from DCR-0026 §2's field list, recorded as a dated note on that
/// record, and forced by §4's own requirement that windows cache
/// individually. `source_hash` is a `CacheKey` axis and, together with
/// `block_kind`, it is what covers "the unit's own body" under the cache's
/// content rule (`cache.rs`: if it changed the prompt bytes the model saw for
/// this unit, it is identity). The unit id is deliberately NOT an axis. So
/// windows sharing the parent's hash would share one cache key while carrying
/// different payloads and different context — every window after the first
/// would replay window 1's translation, silently duplicating rows. Hashing the
/// window's own bytes is the same statement the whole-block path makes: one
/// unit, one body, one hash.
fn window_units(parent: &TranslationUnit, plan: &[Vec<String>]) -> Vec<TranslationUnit> {
    // Re-sliced rather than threaded through: `windows_of` proved it slices,
    // and this keeps the plan a plain list of rows.
    let rows = split_table_rows(&parent.source_payload)
        .expect("the plan was built from this payload's own slice");
    let window_count = plan.len() as u32;
    plan.iter()
        .enumerate()
        .map(|(i, body)| {
            let payload = rows.window(body);
            let constraints = crate::unit::payload::constraints_for(&BlockKind::Table, &payload);
            TranslationUnit {
                unit_id: window_unit_id(&parent.unit_id, i as u32),
                block_kind: BlockKind::Table,
                input_mode: InputMode::TableRowWindow {
                    parent_block_id: parent.unit_id.clone(),
                    window_index: i as u32,
                    window_count,
                },
                source_hash: crate::id::source_hash_bytes(payload.as_bytes()),
                source_payload: payload,
                context: parent.context.clone(),
                constraints,
                // Overwritten per chunk by `build_batches`, like every unit's.
                batch_id: BatchId::new(0),
                retry: None,
            }
        })
        .collect()
}

/// A window's unit id: the parent's ADR-0005 id, a literal `.w`, and a
/// 1-based window ordinal zero-padded to two digits — widening on its own past
/// 99, the way ADR-0005's own numbers do.
///
/// `.` cannot occur in an ADR-0005 id, so a window id can never collide with a
/// real block id. It also sorts directly after its parent as a string, which
/// is what puts a window under its parent in the document-ordered validation
/// report.
pub(crate) fn window_unit_id(parent: &BlockId, window_index: u32) -> BlockId {
    BlockId(format!("{}.w{:02}", parent.0, window_index + 1))
}

/// The parent block id inside a window unit id, or `None` when `id` is an
/// ordinary block id.
///
/// The inverse of [`window_unit_id`], and its neighbor on purpose: the
/// validation report has to rank a window at its parent's document index, and
/// the id convention must be *written* and *read* in one place or the two
/// drift. Recognizing the shape rather than merely splitting on `.` is what
/// keeps a hypothetical future id containing a dot from being mistaken for a
/// window.
pub(crate) fn parent_id_of(id: &BlockId) -> Option<BlockId> {
    let (parent, ordinal) = id.0.rsplit_once(".w")?;
    if parent.is_empty() || ordinal.is_empty() || !ordinal.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(BlockId(parent.to_string()))
}

// DCR-0026 SL-102. The gate (does anything split at all?), the boundaries
// (where do the cuts fall?), and the shape of what comes out.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::llm::TranslationBatch;
    use crate::profile::default_profile;
    use crate::unit::{build_batches, html_outcomes};

    /// A table with `rows` body rows, four columns wide — SCN-03's shape.
    fn table(rows: usize) -> String {
        let mut src = String::from("| id | name | kind | note |\n|---|---|---|---|");
        for i in 1..=rows {
            src.push_str(&format!(
                "\n| {i} | item number {i} | widget | a note about item {i} |"
            ));
        }
        src
    }

    /// A profile that splits: `row-window-first` plus a ceiling low enough
    /// that the fixture table cannot fit under it.
    fn splitting_profile(ceiling: u32) -> ProfileMetadata {
        let mut p = default_profile();
        p.constraints.default_table_strategy = Some("row-window-first".to_string());
        p.batching.target_output_tokens = Some(ceiling);
        p
    }

    fn batches_of(src: &str, profile: ProfileMetadata) -> Vec<TranslationBatch> {
        let mut doc = crate::parser::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        };
        build_batches(&doc, &opts, None, &html_outcomes(&doc))
    }

    fn units_of(batches: &[TranslationBatch]) -> Vec<&TranslationUnit> {
        batches.iter().flat_map(|b| &b.units).collect()
    }

    /// The whole point: a table that today would be packed alone, flagged, and
    /// aborted at the provider is now several units, each a complete table.
    #[test]
    fn an_oversize_table_becomes_several_header_carrying_windows() {
        let src = format!("intro paragraph\n\n{}\n", table(40));
        let batches = batches_of(&src, splitting_profile(400));
        let units = units_of(&batches);

        let windows: Vec<&&TranslationUnit> = units
            .iter()
            .filter(|u| matches!(u.input_mode, InputMode::TableRowWindow { .. }))
            .collect();
        assert!(
            windows.len() > 1,
            "the table must split into more than one window, got {}",
            windows.len()
        );
        // Block ids are one source-order sequence across kinds (ADR-0005),
        // so the table after a paragraph is `t-0002`.
        assert!(
            !units.iter().any(|u| u.unit_id.0 == "t-0002"),
            "the whole-table unit is REPLACED, not kept beside its windows"
        );

        let mut seen_rows = 0u32;
        for (i, w) in windows.iter().enumerate() {
            assert_eq!(w.unit_id.0, format!("t-0002.w{:02}", i + 1));
            assert!(matches!(w.block_kind, BlockKind::Table));
            match &w.input_mode {
                InputMode::TableRowWindow {
                    parent_block_id,
                    window_index,
                    window_count,
                } => {
                    assert_eq!(parent_block_id.0, "t-0002");
                    assert_eq!(*window_index, i as u32);
                    assert_eq!(*window_count as usize, windows.len());
                }
                other => panic!("expected a row window, got {other:?}"),
            }
            // Every window is a complete four-column table with at least one
            // body row — invariant 3's shape, checked through the same
            // inspector the validator uses.
            let (cols, _, rows) =
                crate::structure::inspect_table(&w.source_payload).expect("a window is a table");
            assert_eq!(cols, 4, "window {i} lost a column");
            assert!(rows >= 1, "window {i} carries no body rows");
            assert_eq!(w.constraints.must_preserve_table_columns, Some(4));
            assert_eq!(
                w.constraints.must_preserve_table_row_count,
                Some(rows),
                "a window's row-count constraint is its OWN, not the parent's"
            );
            seen_rows += rows;
            // The parent's context rides along; the hash does NOT — see below.
            assert_eq!(
                w.context.section_path.len(),
                windows[0].context.section_path.len()
            );
        }
        assert_eq!(
            seen_rows, 40,
            "every source row lands in exactly one window"
        );

        // The paragraph is untouched, and the windows kept document order.
        assert_eq!(units[0].unit_id.0, "p-0001");
        assert_eq!(units[1].unit_id.0, "t-0002.w01");
    }

    /// Three gates, each closing on its own: no ceiling, whole-block strategy,
    /// or a table that fits. Any of them leaves the run bit-identical to the
    /// pre-DCR-0026 behavior.
    #[test]
    fn nothing_splits_without_a_ceiling_a_strategy_and_an_oversize_table() {
        let src = format!("{}\n", table(40));

        let no_ceiling = {
            let mut p = splitting_profile(400);
            p.batching.target_output_tokens = None;
            p
        };
        let whole_block = {
            let mut p = splitting_profile(400);
            p.constraints.default_table_strategy = Some("whole-block".to_string());
            p
        };
        let unset_strategy = {
            let mut p = splitting_profile(400);
            p.constraints.default_table_strategy = None;
            p
        };
        let nonsense_strategy = {
            let mut p = splitting_profile(400);
            p.constraints.default_table_strategy = Some("by-vibes".to_string());
            p
        };
        // A ceiling the whole table fits under: the strategy is on, and the
        // table still ships whole — row windows are for tables that do not fit.
        let roomy = splitting_profile(100_000);

        for (name, profile) in [
            ("no ceiling", no_ceiling),
            ("whole-block", whole_block),
            ("unset strategy", unset_strategy),
            ("unrecognized strategy", nonsense_strategy),
            ("a table that fits", roomy),
        ] {
            let batches = batches_of(&src, profile);
            let units = units_of(&batches);
            assert_eq!(units.len(), 1, "{name}: one unit");
            assert_eq!(units[0].unit_id.0, "t-0001", "{name}: the whole table");
            assert!(
                matches!(units[0].input_mode, InputMode::FullTableMarkdown),
                "{name}: still a whole-block table unit"
            );
        }
    }

    /// The boundaries are deterministic and every window that holds more than
    /// one row genuinely fits the target — the greedy rule, checked against
    /// the same estimator the preflight uses.
    #[test]
    fn window_boundaries_are_deterministic_and_respect_the_target() {
        let src = format!("{}\n", table(60));
        let first = batches_of(&src, splitting_profile(500));
        let second = batches_of(&src, splitting_profile(500));
        let ids = |bs: &[TranslationBatch]| -> Vec<String> {
            units_of(bs).iter().map(|u| u.unit_id.0.clone()).collect()
        };
        assert_eq!(ids(&first), ids(&second), "the split is deterministic");

        let bpe = resolve_encoder(None, &TranslateOptions::default().model_id);
        let factor = crate::batch::DEFAULT_OUTPUT_EXPANSION_FACTOR;
        let target = effective_output_target(500);
        let units = units_of(&first);
        assert!(units.len() > 2, "the fixture must really split");
        for u in &units {
            let est = crate::batch::estimate_unit_output_tokens(u, &bpe, factor);
            let rows = crate::structure::inspect_table(&u.source_payload)
                .expect("a table")
                .2;
            assert!(
                est <= target || rows == 1,
                "{} estimates {est} against a target of {target} with {rows} rows — \
                 only a single-row window may exceed it",
                u.unit_id
            );
        }
    }

    /// A payload the slicer refuses, and a table with one body row, both ship
    /// whole: the splitter never mis-slices and never emits a window it cannot
    /// justify. The preflight still names them, which is the documented floor.
    #[test]
    fn a_table_with_nothing_to_distribute_ships_whole() {
        // One enormous row: over the ceiling, and nothing to distribute.
        let giant = format!(
            "| a | b |\n|---|---|\n| {} | {} |\n",
            "x ".repeat(400),
            "y ".repeat(400)
        );
        let batches = batches_of(&giant, splitting_profile(200));
        let units = units_of(&batches);
        assert_eq!(units.len(), 1);
        assert!(matches!(units[0].input_mode, InputMode::FullTableMarkdown));

        // And it is still flagged, so the run is diagnosable rather than
        // silently oversize.
        let warnings = crate::batch::output_budget_warnings(
            &batches,
            &TranslateOptions::default().model_id,
            None,
        );
        assert_eq!(warnings.len(), 1, "the preflight still names it");
        assert_eq!(warnings[0].unit_id.0, "t-0001");
    }

    /// Windows pack through `group_by_token_budget` like ordinary units: the
    /// unit cap applies to them, and they never end up alone in a batch by
    /// virtue of being windows.
    #[test]
    fn windows_pack_through_the_ordinary_budget() {
        let src = format!("{}\n", table(40));
        let mut profile = splitting_profile(400);
        profile.batching.max_units_per_batch = Some(2);
        let batches = batches_of(&src, profile);
        assert!(
            batches.len() > 1,
            "the unit cap split the windows into batches"
        );
        for b in &batches {
            assert!(b.units.len() <= 2, "the cap applies to windows too");
        }
        // Batch ids are assigned per chunk, exactly as for any unit.
        for (i, b) in batches.iter().enumerate() {
            for u in &b.units {
                assert_eq!(u.batch_id, BatchId::new(i as u32 + 1));
            }
        }
    }

    /// The cache-identity property the whole split rests on: two windows of
    /// one table must never share a cache key.
    ///
    /// `CacheKey` has no unit-id axis on purpose (cross-block dedup), and the
    /// unit's body is covered by `source_hash` + `block_kind`. Windows share
    /// their parent's kind, context, language labels, profile and instruction,
    /// so `source_hash` is the ONLY axis left to separate them — and before it
    /// was the window's own bytes, window 2 read window 1's entry out of the
    /// run's own cache and the merged table repeated window 1's rows.
    #[test]
    fn each_window_hashes_its_own_body_so_the_cache_cannot_alias_them() {
        let src = format!("{}\n", table(40));
        let batches = batches_of(&src, splitting_profile(400));
        let units = units_of(&batches);
        let windows: Vec<&&TranslationUnit> = units
            .iter()
            .filter(|u| matches!(u.input_mode, InputMode::TableRowWindow { .. }))
            .collect();
        assert!(windows.len() > 1);

        let hashes: std::collections::HashSet<u64> =
            windows.iter().map(|w| w.source_hash).collect();
        assert_eq!(
            hashes.len(),
            windows.len(),
            "every window hashes to its own value"
        );
        for w in &windows {
            assert_eq!(
                w.source_hash,
                crate::id::source_hash_bytes(w.source_payload.as_bytes()),
                "and that value is the hash of the bytes it carries"
            );
        }
        // Two windows with identical bodies WOULD share a key, which is the
        // ordinary cross-block dedup and is correct.
        assert_eq!(
            crate::id::source_hash_bytes(windows[0].source_payload.as_bytes()),
            crate::id::source_hash_bytes(windows[0].source_payload.as_bytes()),
        );
    }

    /// The id convention: `.` cannot occur in an ADR-0005 id, the ordinal is
    /// 1-based and zero-padded to two digits, and it widens on its own.
    #[test]
    fn window_ids_are_collision_free_and_widen_past_ninety_nine() {
        let parent = BlockId("t-0007".to_string());
        assert_eq!(window_unit_id(&parent, 0).0, "t-0007.w01");
        assert_eq!(window_unit_id(&parent, 8).0, "t-0007.w09");
        assert_eq!(window_unit_id(&parent, 98).0, "t-0007.w99");
        assert_eq!(window_unit_id(&parent, 99).0, "t-0007.w100");
        assert_eq!(window_unit_id(&parent, 999).0, "t-0007.w1000");
        // Sorting a window directly after its parent is what the report's
        // document ordering leans on.
        let mut ids = vec![
            "t-0008".to_string(),
            window_unit_id(&parent, 1).0,
            "t-0007".to_string(),
            window_unit_id(&parent, 0).0,
        ];
        ids.sort();
        assert_eq!(ids, ["t-0007", "t-0007.w01", "t-0007.w02", "t-0008"]);
    }

    /// Writing the convention and reading it back are one pair, and the
    /// reader recognizes the shape rather than merely splitting on a
    /// separator — the validation report's document ordering depends on it
    /// answering `None` for anything that is not a window id.
    #[test]
    fn a_window_id_round_trips_through_its_parent() {
        for parent in ["t-0007", "t-0001", "t-9999"] {
            let p = BlockId(parent.to_string());
            for index in [0u32, 1, 98, 99, 999] {
                assert_eq!(parent_id_of(&window_unit_id(&p, index)), Some(p.clone()));
            }
        }
        for ordinary in [
            "t-0007",
            "p-0001",
            "h2-0003",
            "html-0001",
            "t-0007.",
            "t-0007.w",
            "t-0007.wxy",
            ".w01",
            "t-0007.window",
            "",
        ] {
            assert_eq!(
                parent_id_of(&BlockId(ordinary.to_string())),
                None,
                "{ordinary:?} is not a window id"
            );
        }
    }
}
