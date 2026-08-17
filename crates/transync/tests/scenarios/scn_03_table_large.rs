//! SCN-03 — Oversized table → header-carrying row windows (DCR-0026).
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md`
//! SL-03, now exercised on the **live** path rather than around it:
//! - a 200-row table whose estimated response exceeds the output ceiling is
//!   dispatched as more than one `table_row_window` unit, each a complete GFM
//!   table carrying the real header (architectural invariant 3);
//! - the windows reassemble into ONE 200×4 table in source order;
//! - the alignment map still holds exactly one row for the table, marked
//!   `Translated`, under an unchanged schema version, and holds no window id;
//! - a window that exhausts its retries costs its own rows and not the table
//!   (DCR-0026 OQ-1(b)): the parent is `PartiallyTranslated`, every row still
//!   round-trips, and the failed window is named.
//!
//! Until DCR-0026 this scenario was satisfied by the whole-block path — the
//! splitter did not exist — and the scenario matrix carried a
//! "stub-verified only" footnote for it. Both are retired by this file.
//!
//! TRACE: SCN-03
//! TRACE: SL-03
//! TRACE: DCR-0026

use crate::common::fixture_gen::generate_scn_03;
use crate::common::mock_translator::MockTranslator;
use transync::llm::InputMode;
use transync::{BlockId, FallbackStatus, TranslateOptions, translate};

/// A ceiling low enough that the 200-row fixture cannot possibly fit under
/// it, so the split is forced rather than hoped for. Well above the 64-token
/// response-envelope reserve, so it is a legal ceiling.
const SPLITTING_CEILING: u32 = 1_200;

/// The run's options: the shipped default profile (which ships
/// `row-window-first` since DCR-0026) with a ceiling the fixture overruns.
fn splitting_opts() -> TranslateOptions {
    let mut opts = crate::common::opts_for("ko");
    let mut profile = transync::profile::default_profile();
    profile.batching.target_output_tokens = Some(SPLITTING_CEILING);
    opts.profile = Some(profile);
    opts
}

/// The table's alignment row, and the shape of the first table in the
/// regenerated Markdown.
fn first_table_shape(md: &str) -> Option<(u32, u32)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    for child in root.children() {
        if let NodeValue::Table(t) = &child.data.borrow().value {
            let cols = t.alignments.len() as u32;
            let mut rows: u32 = 0;
            for row in child.children() {
                if let NodeValue::TableRow(is_header) = row.data.borrow().value
                    && !is_header
                {
                    rows += 1;
                }
            }
            return Some((cols, rows));
        }
    }
    None
}

/// The positive path: the split really happens, and the document that comes
/// out is indistinguishable from the whole-block one.
#[tokio::test]
async fn smoke_scn_03() {
    let source = generate_scn_03(200);
    // Recording, so the assertion about what was DISPATCHED is made against
    // the batches the provider actually saw — not inferred from the output.
    let translator = MockTranslator::recording();
    let opts = splitting_opts();

    let output = translate(&source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-03 200-row table");

    // --- what went out on the wire ---------------------------------------
    let dispatched: Vec<transync::llm::TranslationUnit> = translator
        .recorded()
        .into_iter()
        .flat_map(|b| b.units)
        .collect();
    let windows: Vec<&transync::llm::TranslationUnit> = dispatched
        .iter()
        .filter(|u| matches!(u.input_mode, InputMode::TableRowWindow { .. }))
        .collect();
    assert!(
        windows.len() >= 2,
        "the oversize table must be dispatched as at least two row windows, got {}",
        windows.len()
    );
    assert!(
        !dispatched
            .iter()
            .any(|u| matches!(u.input_mode, InputMode::FullTableMarkdown)),
        "the whole-table unit is replaced by its windows, not sent beside them"
    );
    let parent = match &windows[0].input_mode {
        InputMode::TableRowWindow {
            parent_block_id,
            window_count,
            ..
        } => {
            assert_eq!(*window_count as usize, windows.len(), "every window ships");
            parent_block_id.clone()
        }
        other => panic!("expected a row window, got {other:?}"),
    };

    let mut window_rows = 0u32;
    for (i, w) in windows.iter().enumerate() {
        assert_eq!(w.unit_id.0, format!("{}.w{:02}", parent.0, i + 1));
        // Every window is a complete table with the real header — never an
        // isolated cell, never a headerless fragment.
        let (cols, rows) = first_table_shape(&w.source_payload).expect("a window is a table");
        assert_eq!(cols, 4, "window {i} carries all four columns");
        assert!(rows >= 1, "window {i} carries at least one body row");
        assert!(
            w.source_payload.starts_with("| Code"),
            "window {i} repeats the source header: {}",
            w.source_payload.lines().next().unwrap_or("")
        );
        window_rows += rows;
    }
    assert_eq!(
        window_rows, 200,
        "every source row is in exactly one window"
    );

    // --- what came back ---------------------------------------------------
    assert_eq!(output.alignment_map.schema_version, "1.2.0");
    let table_rows: Vec<&transync::AlignmentBlock> = output
        .alignment_map
        .blocks
        .iter()
        .filter(|b| b.block_kind == "table")
        .collect();
    assert_eq!(
        table_rows.len(),
        1,
        "the alignment map carries ONE row per source block, however many windows shipped"
    );
    assert_eq!(table_rows[0].source_block_id, parent);
    assert_eq!(
        table_rows[0].fallback_status,
        FallbackStatus::Translated,
        "every window translated, so the merged table is Translated",
    );
    assert!(
        !output
            .alignment_map
            .blocks
            .iter()
            .any(|b| b.source_block_id.0.contains(".w")),
        "a window id must never reach the alignment map"
    );

    let (cols, rows) = first_table_shape(&output.translated_document)
        .expect("regenerated MD should reparse as a table");
    assert_eq!(cols, 4, "table column count must be preserved");
    assert_eq!(rows, 200, "every one of the 200 rows must round-trip");
    // Source order, checked at both ends and across a window seam rather than
    // by row count alone.
    let body: Vec<&str> = output
        .translated_document
        .lines()
        .filter(|l| l.starts_with("| R-"))
        .collect();
    assert_eq!(body.len(), 200);
    for (i, line) in body.iter().enumerate() {
        assert!(
            line.contains(&format!("R-{:04}", i + 1)),
            "row {} is out of source order: {line}",
            i + 1
        );
    }

    // --- what the report says --------------------------------------------
    assert_eq!(
        output.validation_report.schema_version,
        transync::VALIDATION_REPORT_SCHEMA_VERSION
    );
    let report_ids: Vec<&str> = output
        .validation_report
        .per_unit
        .iter()
        .map(|r| r.unit_id.0.as_str())
        .collect();
    assert!(
        report_ids.iter().filter(|id| id.contains(".w")).count() == windows.len(),
        "every window keeps its own report row: {report_ids:?}"
    );
    // The windows are contiguous — they rank at their parent's document index.
    let first_window = report_ids
        .iter()
        .position(|id| id.contains(".w"))
        .expect("a window row");
    for offset in 0..windows.len() {
        assert!(
            report_ids[first_window + offset].contains(".w"),
            "a split table's report rows are contiguous: {report_ids:?}"
        );
    }

    // The checked-in skeleton fixture (12 rows) still round-trips whole under
    // the same profile: it fits the ceiling, so nothing splits.
    let skeleton = include_str!("../fixtures/scn-03-table-large.md");
    let skeleton_translator = MockTranslator::recording();
    translate(skeleton, &opts, &skeleton_translator)
        .await
        .expect("skeleton SCN-03 fixture should still round-trip");
    assert!(
        skeleton_translator
            .recorded()
            .iter()
            .flat_map(|b| &b.units)
            .all(|u| !matches!(u.input_mode, InputMode::TableRowWindow { .. })),
        "a table that fits the ceiling still ships whole"
    );
}

/// The negative path (DCR-0026 OQ-1, resolved (b)): one window exhausts its
/// retries. It costs its own rows, not the table — the paid-for windows
/// survive, every source row still round-trips, and the block says so.
#[tokio::test]
async fn scn_03_a_failed_window_costs_its_rows_not_the_table() {
    let source = generate_scn_03(200);
    let opts = splitting_opts();

    // Learn the window ids from a clean run first, so the fixture cannot go
    // stale if the boundaries move with the encoder or the factor.
    let probe = MockTranslator::recording();
    translate(&source, &opts, &probe)
        .await
        .expect("the probe run succeeds");
    let window_ids: Vec<BlockId> = probe
        .recorded()
        .into_iter()
        .flat_map(|b| b.units)
        .filter(|u| matches!(u.input_mode, InputMode::TableRowWindow { .. }))
        .map(|u| u.unit_id)
        .collect();
    assert!(window_ids.len() >= 3, "need a middle window to fail");
    let victim = window_ids[1].clone();

    let translator = MockTranslator::always_fails_unit(victim.clone());
    let output = translate(&source, &opts, &translator)
        .await
        .expect("a terminally-failed window must not abort the run");

    let table_row = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "table")
        .expect("the table still has exactly one alignment row");
    assert_eq!(
        table_row.fallback_status,
        FallbackStatus::PartiallyTranslated,
        "the existing marker for a partly-source block (invariant 6)"
    );

    // Every source row is still there, in order: the failed window
    // contributed its own SOURCE rows.
    let (cols, rows) = first_table_shape(&output.translated_document).expect("still a table");
    assert_eq!((cols, rows), (4, 200));
    let body: Vec<&str> = output
        .translated_document
        .lines()
        .filter(|l| l.starts_with("| R-"))
        .collect();
    for (i, line) in body.iter().enumerate() {
        assert!(line.contains(&format!("R-{:04}", i + 1)), "row {}", i + 1);
    }

    // The failed window is named — on its own report row, and on the parent's
    // warnings channel, so "which rows are source" is answerable from the
    // artifact.
    let parent_row = output
        .validation_report
        .per_unit
        .iter()
        .find(|r| r.unit_id == table_row.source_block_id)
        .expect("the parent has a report row");
    assert_eq!(parent_row.final_status, FallbackStatus::PartiallyTranslated);
    assert!(
        parent_row.warnings.iter().any(|w| w.contains(&victim.0)),
        "the failed window is named on the parent: {:?}",
        parent_row.warnings
    );
    let victim_row = output
        .validation_report
        .per_unit
        .iter()
        .find(|r| r.unit_id == victim)
        .expect("the failed window keeps its own row");
    assert_eq!(victim_row.final_status, FallbackStatus::FallbackSource);
    assert!(
        !victim_row.attempts.is_empty(),
        "and its own attempt log — a window's retries are its own"
    );

    // The tally counts blocks, not windows: one partially-translated block.
    assert_eq!(
        output.alignment_map.validation_summary.partially_translated,
        1
    );
}
