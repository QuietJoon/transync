//! SCN-01 — Heading + paragraph translation.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-01:
//! 1. `transync::translate(source, opts, MockTranslator::passthrough)` returns `Ok(_)`.
//! 2. `output.alignment_map.blocks.len() == 4` (one H1 + three paragraphs).
//! 3. First block has `block_kind == "heading-1"`.
//! 4. Every alignment row has `fallback_status == translated`.
//! 5. Reparsing `output.translated_document` yields the same kind sequence.
//!
//! TRACE: SCN-01
//! TRACE: SL-01

use crate::common::mock_translator::MockTranslator;
use transync::{FallbackStatus, translate};

#[tokio::test]
async fn smoke_scn_01() {
    let source = include_str!("../fixtures/scn-01-headings-and-paragraphs.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-01 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert!(!output.translated_document.is_empty());

    // (2) Exactly four sync-relevant blocks.
    assert_eq!(
        output.alignment_map.blocks.len(),
        4,
        "expected 4 alignment rows (1 H1 + 3 paragraphs), got {}",
        output.alignment_map.blocks.len(),
    );

    // (3) First row is the H1.
    let first = &output.alignment_map.blocks[0];
    assert_eq!(
        first.block_kind, "heading-1",
        "expected first block heading-1"
    );
    assert_eq!(first.source_block_id.0, "h1-0001");

    // Remaining three are paragraphs in source order.
    for (i, row) in output.alignment_map.blocks.iter().enumerate().skip(1) {
        assert_eq!(row.block_kind, "paragraph", "row {} should be paragraph", i);
    }

    // (4) Every row was accepted as translated under the passthrough mock.
    for row in &output.alignment_map.blocks {
        assert_eq!(
            row.fallback_status,
            FallbackStatus::Translated,
            "block {} should be translated",
            row.source_block_id,
        );
    }

    // (5) Reparse the regenerated MD and check the kind sequence matches.
    let kinds = reparse_kinds(&output.translated_document);
    let expected = vec!["heading-1", "paragraph", "paragraph", "paragraph"];
    assert_eq!(
        kinds, expected,
        "reparsed kind sequence diverged from source"
    );

    // Render must include data-sync-id on every block.
    for row in &output.alignment_map.blocks {
        let needle = format!("data-sync-id=\"{}\"", row.source_block_id);
        assert!(
            output.annotated_target_html.contains(&needle),
            "target HTML missing data-sync-id for {}",
            row.source_block_id,
        );
    }
}

/// Reparse the regenerated MD with Comrak and emit a flat list of
/// kebab-case block-kind names in source order.
fn reparse_kinds(md: &str) -> Vec<&'static str> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    opts.extension.tasklist = true;
    let root = comrak::parse_document(&arena, md, &opts);
    let mut kinds = Vec::new();
    for child in root.children() {
        match child.data.borrow().value {
            NodeValue::Heading(ref h) => match h.level {
                1 => kinds.push("heading-1"),
                2 => kinds.push("heading-2"),
                3 => kinds.push("heading-3"),
                4 => kinds.push("heading-4"),
                5 => kinds.push("heading-5"),
                6 => kinds.push("heading-6"),
                _ => {}
            },
            NodeValue::Paragraph => kinds.push("paragraph"),
            NodeValue::Table(_) => kinds.push("table"),
            NodeValue::CodeBlock(_) => kinds.push("code-block"),
            NodeValue::List(_) => kinds.push("list"),
            NodeValue::BlockQuote => kinds.push("blockquote"),
            NodeValue::ThematicBreak => kinds.push("thematic-break"),
            _ => {}
        }
    }
    kinds
}
