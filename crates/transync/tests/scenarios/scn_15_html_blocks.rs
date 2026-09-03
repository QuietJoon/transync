//! SCN-15 — HTML-content translation end-to-end (spec 2026-08-03 §7).
//!
//! Fixture structure (`fixtures/scn-15-html-blocks.md`), verified against the
//! real parse — ids follow the single document counter, so they shift if the
//! fixture gains or loses a block:
//!
//! | id          | kind      | html block type                    | outcome      |
//! |-------------|-----------|------------------------------------|--------------|
//! | `h1-0001`   | heading-1 | —                                  | unit         |
//! | `p-0002`    | paragraph | — (inline `<kbd>`)                 | unit         |
//! | `html-0003` | html      | 6 (`<details>` open + `<summary>`) | unit         |
//! | `p-0004`    | paragraph | — (hidden markdown body)           | unit         |
//! | `html-0005` | html      | 6 (`</details>` orphan close)      | zero-segment |
//! | `html-0006` | html      | 6 (`<div align>` hero)             | unit         |
//! | `html-0007` | html      | 6 (`<table>`)                      | unit         |
//! | `html-0008` | html      | 2 (comment)                        | zero-segment |
//! | `html-0009` | html      | 1 (`<pre>`, interior blank line)   | unit         |
//! | `p-0010`    | paragraph | —                                  | unit         |
//!
//! Four html blocks are unit-backed and two are zero-segment, so
//! `total_units` is 8 (four non-html + four html) — the counting rule of
//! spec §5.
//!
//! Two comrak sourcepos quirks are visible in this fixture and are the reason
//! the run also emits two refdefs "unattributed source text between blocks"
//! notes (`parser::refdefs`'s documented backstop):
//! `html-0008`'s comment reports an end position *before* its start, so its
//! byte range collapses to empty; and `html-0009`'s type-1 `<pre>` reports an
//! end at its last content line, leaving the `</pre>` line outside the block
//! range. Both sets of bytes survive as verbatim inter-block gap bytes — the
//! byte-identity assertion below is what pins that — and the renderer's
//! per-anchor tag balancing still closes `<pre>` in both panes. The
//! assertions here deliberately do not encode either quirk.
//!
//! TRACE: SCN-15

use crate::common::mock_translator::MockTranslator;
use transync::{FallbackStatus, TranslateOptions, translate, translate_with_cache};

fn opts() -> TranslateOptions {
    crate::common::opts_for("ko")
}

const SRC: &str = include_str!("../fixtures/scn-15-html-blocks.md");

#[tokio::test]
async fn smoke_scn_15() {
    let translator = MockTranslator::passthrough();
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");

    // Every html-kind row is an anchor with block_kind "html".
    let html_rows: Vec<_> = output
        .alignment_map
        .blocks
        .iter()
        .filter(|r| r.block_kind == "html")
        .collect();
    assert!(
        html_rows.len() >= 5,
        "details-open, orphan-close, hero, table, comment, pre: {html_rows:?}"
    );

    // Counting rule (spec §5): zero-segment rows preserved+uncounted.
    let zero_rows: Vec<_> = html_rows
        .iter()
        .filter(|r| r.fallback_status == FallbackStatus::Preserved)
        .collect();
    assert!(
        !zero_rows.is_empty(),
        "comment + orphan-close are zero-segment"
    );
    let unit_rows = html_rows.len() - zero_rows.len();
    let s = &output.alignment_map.validation_summary;
    let non_html_units = output
        .alignment_map
        .blocks
        .iter()
        .filter(|r| {
            r.block_kind != "html" && r.block_kind != "thematic-break" && r.block_kind != "image"
        })
        .count();
    assert_eq!(
        s.total_units as usize,
        non_html_units + unit_rows,
        "unit-backed html blocks count; zero-segment ones do not"
    );

    // Passthrough == identity: out.md must be byte-identical (identity skip
    // keeps entity/markup bytes; spec §3.3).
    assert_eq!(output.translated_document, SRC, "identity round-trip");

    // Live render: real <details> in both panes, no data-skipped for it.
    for html in [&output.annotated_source_html, &output.annotated_target_html] {
        assert!(html.contains("<details>"), "live details:\n{html}");
        assert!(
            html.contains("<summary>Click to expand</summary>"),
            "live summary"
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "no placeholder on success path"
        );
    }

    // Reader-honesty warnings for the zero-segment blocks.
    assert!(
        output
            .validation_report
            .skipped_source_nodes
            .iter()
            .any(|w| w.contains("no translatable text")),
        "zero-segment warning present: {:?}",
        output.validation_report.skipped_source_nodes
    );
}

#[tokio::test]
async fn translated_segments_are_spliced_and_markup_survives() {
    let translator = MockTranslator::tamper_payload("Click to expand", "펼치기");
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");
    assert!(
        output
            .translated_document
            .contains("<summary>펼치기</summary>"),
        "segment translated inside intact markup:\n{}",
        output.translated_document
    );
    assert!(
        output
            .translated_document
            .contains("<div align=\"center\">"),
        "markup preserved by construction"
    );
}

#[tokio::test]
async fn failed_html_unit_falls_back_to_escaped_placeholder() {
    // Find the details-open fragment's id first (structure-derived).
    let probe = MockTranslator::passthrough();
    let probe_out = translate(SRC, &opts(), &probe).await.expect("Ok");
    let details_id = probe_out
        .alignment_map
        .blocks
        .iter()
        .find(|r| r.block_kind == "html" && r.fallback_status == FallbackStatus::Translated)
        .map(|r| r.source_block_id.clone())
        .expect("a unit-backed html row exists");

    let translator = MockTranslator::always_fails_unit(details_id.clone());
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");
    let row = output
        .alignment_map
        .blocks
        .iter()
        .find(|r| r.source_block_id == details_id)
        .expect("row");
    assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
    let needle = format!("data-sync-id=\"{details_id}\"");
    let target = &output.annotated_target_html;
    let pos = target.find(&needle).expect("anchor present");
    let window = &target[pos.saturating_sub(120)..(pos + 200).min(target.len())];
    assert!(
        window.contains("data-skipped=\"html-block\""),
        "fallback html renders the placeholder:\n{window}"
    );
    // out.md keeps the source bytes verbatim for the failed block.
    assert!(
        output
            .translated_document
            .contains("<summary>Click to expand</summary>")
    );
}

#[tokio::test]
async fn html_units_cache_and_replay_without_provider_calls() {
    // Spec §6 cache row: wire-shaped payloads cache; hits re-validate and
    // re-splice.
    let cache = transync::cache::InMemoryCache::default();
    let t1 = MockTranslator::recording();
    let first = translate_with_cache(SRC, &opts(), &t1, &cache)
        .await
        .expect("Ok");
    let calls_first = t1.call_count();
    assert!(calls_first > 0);

    let t2 = MockTranslator::recording();
    let second = translate_with_cache(SRC, &opts(), &t2, &cache)
        .await
        .expect("Ok");
    assert_eq!(
        t2.call_count(),
        0,
        "full cache replay — html units included"
    );
    assert_eq!(first.translated_document, second.translated_document);
}
