//! SCN-06 — Blockquote container.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-06:
//! - Translated blockquote container shape preserved.
//! - Same number of nested children in the same order with the same kinds.
//!
//! TRACE: SCN-06
//! TRACE: SL-06

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_06() {
    let source = include_str!("../fixtures/scn-06-blockquote.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-06 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.2.0");
    assert!(!output.translated_document.is_empty());

    // The fixture has exactly one blockquote at the top level. Its
    // children-kind sequence must round-trip.
    let src_children = first_blockquote_children(source).expect("fixture has a blockquote");
    let regen_children = first_blockquote_children(&output.translated_document)
        .expect("regenerated MD must still contain the blockquote");
    assert_eq!(
        src_children, regen_children,
        "blockquote child kinds must round-trip",
    );
}

fn first_blockquote_children(md: &str) -> Option<Vec<String>> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.tasklist = true;
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    for child in root.children() {
        if matches!(child.data.borrow().value, NodeValue::BlockQuote) {
            let kinds: Vec<String> = child
                .children()
                .filter_map(|n| match &n.data.borrow().value {
                    NodeValue::Heading(h) => Some(format!("heading-{}", h.level)),
                    NodeValue::Paragraph => Some("paragraph".to_string()),
                    NodeValue::Table(_) => Some("table".to_string()),
                    NodeValue::CodeBlock(_) => Some("code-block".to_string()),
                    NodeValue::List(_) => Some("list".to_string()),
                    NodeValue::BlockQuote => Some("blockquote".to_string()),
                    NodeValue::ThematicBreak => Some("thematic-break".to_string()),
                    _ => None,
                })
                .collect();
            return Some(kinds);
        }
    }
    None
}
