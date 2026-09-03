//! Wave 3's "real ids and a real alignment map" leg (spec §12): the
//! existing downstream machinery — outcome, regen offsets, align, walk —
//! driven by the HTML intake's output, with the D5 title row's `non-sync`
//! role pinned end to end. No pipeline, no LLM, no core.

use std::collections::HashMap;
use transync_syntax::align::{self, SyncRole};
use transync_syntax::{id, intake, outcome, regen, walk};

const SCN_16: &str = include_str!("../../transync/tests/fixtures/scn-16-html-document.html");

#[test]
fn a_parsed_html_document_gets_a_full_alignment_map_with_a_non_sync_title_row() {
    let doc = intake::html::parse(SCN_16);
    let (out, offsets) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(out, doc.source_text);

    let outcomes = outcome::html_outcomes(&doc);
    let map =
        align::build_alignment_map(&doc, &HashMap::new(), &offsets, "en", "ko", None, &outcomes);

    assert_eq!(map.blocks.len(), doc.blocks.len(), "a row for every block");
    assert_eq!(
        map.document_id,
        format!("{:016x}", id::source_hash_bytes(doc.source_text.as_bytes())),
        "document_id stays source_hash_bytes(source_text) (spec §4)",
    );

    // The D5 title row: block_kind "title", sync_role non-sync, REAL ranges.
    let title = &map.blocks[0];
    assert_eq!(title.block_kind, "title");
    assert_eq!(
        title.sync_role,
        SyncRole::NonSync,
        "translated, aligned, anchor-less (D5)"
    );
    assert_eq!(title.source_range, doc.blocks[0].source_range);
    assert_eq!(
        title.target_range, doc.blocks[0].source_range,
        "identity regen: target ranges mirror source ranges byte for byte",
    );

    // Every row's target range is real — the empty-range fallback would
    // mean regenerate skipped a block.
    for row in &map.blocks {
        assert!(
            row.target_range.end > row.target_range.start,
            "{}: empty target_range",
            row.source_block_id.0,
        );
    }

    // The thematic break keeps its schema-1.0 non-sync shape; everything
    // anchored stays anchored.
    let hr = map
        .blocks
        .iter()
        .find(|r| r.block_kind == "thematic-break")
        .expect("hr row");
    assert_eq!(hr.sync_role, SyncRole::NonSync);
    let h1 = map
        .blocks
        .iter()
        .find(|r| r.block_kind == "heading-1")
        .expect("h1 row");
    assert_eq!(h1.sync_role, SyncRole::Anchor);
}

#[test]
fn the_prefix_collapse_is_total_over_html_list_items() {
    // walk::normalize_top_level's collapse rule reads ast_path prefixes;
    // the intake's comrak-convention paths must keep it total (spec §4):
    // same-list items collapse, adjacent lists never merge.
    let doc = intake::html::parse("<ul><li>a</li><li>b</li></ul><ul><li>c</li></ul>");
    let entries = walk::normalize_top_level(&doc);
    let shape: Vec<(&str, usize)> = entries.iter().map(|e| (e.label, e.sources.len())).collect();
    assert_eq!(shape, vec![("list", 2), ("list", 1)]);
}

#[test]
fn the_docs_fragment_normalizes_with_one_three_item_list_entry() {
    let doc = intake::html::parse(include_str!("fixtures/html-corpus/docs-fragment.html"));
    let entries = walk::normalize_top_level(&doc);
    let shape: Vec<(&str, usize)> = entries.iter().map(|e| (e.label, e.sources.len())).collect();
    assert_eq!(
        shape,
        vec![
            ("heading-1", 1),
            ("paragraph", 1),
            ("list", 3),
            ("code-block", 1),
            ("html", 1),
            ("html", 1),
            ("paragraph", 1),
        ],
    );
}
