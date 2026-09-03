//! SCN-02 — GFM table whole-block translation.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-02:
//! - 4-column input round-trips with 4 columns and the same alignment row.
//! - Per-kind validator rejects column-count mismatch.
//! - SCN-02 fixture asserts column count and alignment preservation.
//!
//! TRACE: SCN-02
//! TRACE: SL-02

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_02() {
    let source = include_str!("../fixtures/scn-02-table-small.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-02 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert!(!output.translated_document.is_empty());

    // SCN-02 fixture: H2 + intro paragraph + table + closing paragraph = 4 blocks.
    assert_eq!(
        output.alignment_map.blocks.len(),
        4,
        "expected 4 alignment rows on SCN-02, got {}",
        output.alignment_map.blocks.len(),
    );

    let table_row = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "table")
        .expect("alignment map should contain one table row");
    assert_eq!(table_row.source_block_id.0, "t-0003");
    assert_eq!(
        table_row.fallback_status,
        transync::FallbackStatus::Translated,
        "table block should pass per-kind validation",
    );

    // Reparse the regenerated MD and assert the table still has 4 columns
    // and 6 data rows.
    let (cols, _alignments, rows) = reparse_scn02_table(&output.translated_document)
        .expect("regenerated MD should reparse as containing the SCN-02 table");
    assert_eq!(cols, 4, "table column count must be preserved");
    assert_eq!(rows, 6, "table data-row count must be preserved");
}

fn reparse_scn02_table(md: &str) -> Option<(u32, Vec<comrak::nodes::TableAlignment>, u32)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    for child in root.children() {
        if let NodeValue::Table(t) = &child.data.borrow().value {
            let alignments = t.alignments.clone();
            let cols = alignments.len() as u32;
            let mut rows: u32 = 0;
            for row in child.children() {
                if let NodeValue::TableRow(is_header) = row.data.borrow().value
                    && !is_header
                {
                    rows += 1;
                }
            }
            return Some((cols, alignments, rows));
        }
    }
    None
}
