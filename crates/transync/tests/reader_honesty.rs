//! Reader-honesty rendering (design D2).
//!
//! Part A — anchored raw HTML blocks: spec 2026-08-03 §3.1 makes a raw HTML
//! block a first-class kind (`BlockKind::Html`), so it must (1) survive
//! verbatim into the regenerated Markdown and (2) appear LIVE-rendered
//! inside an anchored `<div>` in BOTH panes — the escaped `<pre>`
//! placeholder is now the fallback-only path (spec §5 / DCR-0013).
//!
//! Part B — document-level reference-link resolution: a `[text][ref]` whose
//! definition lives in an inter-block gap must render as a real `<a href>`
//! in both panes, and the definition line must survive verbatim into the
//! regenerated Markdown.
//!
//! TRACE: SCN-12
//! TRACE: R0008-0013
//! TRACE: ADR-0012 (amendment §4)

mod common;

use common::mock_translator::MockTranslator;
use transync::{TranslateOptions, translate};

#[tokio::test]
async fn raw_html_block_round_trips_live_rendered_and_anchored() {
    let source = include_str!("fixtures/reader-honesty.md");
    let translator = MockTranslator::passthrough();
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the reader-honesty fixture");

    // (1) The raw HTML block's source bytes survive verbatim in out.md. Since
    // spec §3.2 the block is a real unit whose payload is a JSON segment
    // array, so this now travels through the identity splice (§3.3): the
    // passthrough mock echoes the segments unchanged, every segment equals its
    // decoded source, and the splice therefore re-emits the source bytes.
    let source_block = source
        .split("\n\n")
        .find(|b| b.starts_with("<div class=\"note\">"))
        .expect("fixture carries the raw HTML block");
    assert!(
        output.translated_document.contains(source_block),
        "raw HTML block bytes must be preserved verbatim in the translated \
         Markdown:\n{}",
        output.translated_document,
    );

    // Exactly one html alignment row, with the `html-` id prefix. ID
    // numbering is pinned: the html block shares the ordinal counter, so it
    // lands at `html-0002` (after the `h1-0001` heading), shifting the
    // trailing paragraph to `p-0003`.
    let html_rows: Vec<_> = output
        .alignment_map
        .blocks
        .iter()
        .filter(|b| b.block_kind == "html")
        .collect();
    assert_eq!(
        html_rows.len(),
        1,
        "expected exactly one html alignment row"
    );
    let html_row = html_rows[0];
    assert_eq!(html_row.source_block_id.0, "html-0002", "pinned html id");
    assert_eq!(html_row.target_block_id.0, "html-0002");
    assert!(html_row.parent_id.is_none(), "html row has no parent");
    assert_eq!(
        html_row.fallback_status,
        transync::FallbackStatus::Translated,
        "a text-bearing html block is a real translation unit (spec §3.2), so \
         its row reports the unit's own status",
    );

    // Numbering shift: the paragraph after the html block is p-0003.
    assert!(
        output
            .alignment_map
            .blocks
            .iter()
            .any(|b| b.source_block_id.0 == "p-0003"),
        "the trailing paragraph must be p-0003 after the html block shifted \
         the ordinal counter",
    );

    // An html block is a modeled kind now, so it must NOT be reported as an
    // unmodeled/skipped source node any more (spec 2026-08-03 §3.1). The
    // whole report is empty for this fixture — no other unmodeled node and no
    // refdefs backstop note ride along.
    assert!(
        output.validation_report.skipped_source_nodes.is_empty(),
        "a raw HTML block must not be reported as a skipped source node:\n{:?}",
        output.validation_report.skipped_source_nodes,
    );

    // (2) Both panes carry the SAME anchor for the html block, rendered LIVE
    // inside the `<div>` wrapper — the escaped `<pre>` placeholder is now
    // reserved for the fallback path (spec §5).
    let anchor = "data-sync-id=\"html-0002\"";
    for (pane, html) in [
        ("source", &output.annotated_source_html),
        ("target", &output.annotated_target_html),
    ] {
        assert!(
            html.contains(anchor),
            "{pane} pane missing html anchor {anchor}:\n{html}",
        );
        assert!(
            html.contains("data-block-kind=\"html\""),
            "{pane} pane missing html block-kind attribute:\n{html}",
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "{pane} pane must not fall back to the escaped placeholder:\n{html}",
        );
        assert!(
            html.contains("<div class=\"note\">"),
            "{pane} pane must render the html block live:\n{html}",
        );
        assert!(
            !html.contains("&lt;div class="),
            "{pane} pane must not escape a preserved html block:\n{html}",
        );
        assert!(
            !html.contains("raw HTML omitted"),
            "{pane} pane must not drop the content via comrak's unsafe filter:\n{html}",
        );
    }
}

/// Part B (design D2 §B / C11): the fixture's `[project docs][ref]` reference
/// link — whose definition lives in a trailing inter-block gap — resolves to
/// a real `<a href>` in BOTH panes, and the definition line survives verbatim
/// into the regenerated Markdown.
#[tokio::test]
async fn reference_link_resolves_in_both_panes_and_definition_survives() {
    let source = include_str!("fixtures/reader-honesty.md");
    let translator = MockTranslator::passthrough();
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the reader-honesty fixture");

    // The definition line lives in an inter-block gap regen copies verbatim,
    // so it survives byte-for-byte into out.md — the pool source for the
    // target pane (design D2 §B1.3).
    assert!(
        output
            .translated_document
            .contains("[ref]: https://example.com/r"),
        "the reference definition must survive verbatim in the translated \
         Markdown:\n{}",
        output.translated_document,
    );

    // Both panes resolve the reference to a real anchor — never literal
    // bracket text (design D2 §B3).
    for (pane, html) in [
        ("source", &output.annotated_source_html),
        ("target", &output.annotated_target_html),
    ] {
        assert!(
            html.contains("<a href=\"https://example.com/r\""),
            "{pane} pane must render the reference link as a real <a href>:\n{html}",
        );
        assert!(
            !html.contains("[project docs][ref]"),
            "{pane} pane must not show the reference unresolved as literal \
             brackets:\n{html}",
        );
    }
}
