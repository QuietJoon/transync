//! SCN-14 — Full-document reparse.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-14:
//! - Reparsing `output.translated_document` via Comrak yields the same
//!   block-kind sequence as the source IR (top-level).
//! - The reparsed block count equals the source top-level block count.
//!
//! TRACE: SCN-14
//! TRACE: SL-14

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_14() {
    let source = include_str!("../fixtures/scn-14-full.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-14 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert!(!output.translated_document.is_empty());

    // Top-level kind sequence equality: reparse the regenerated MD and
    // compare against the alignment map's top-level rows.
    let regen_kinds = reparse_top_level_kinds(&output.translated_document);
    let source_kinds = source_top_level_kinds(&output);
    assert_eq!(
        regen_kinds, source_kinds,
        "regenerated top-level kind sequence diverged from source IR",
    );
}

fn reparse_top_level_kinds(md: &str) -> Vec<String> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    opts.extension.tasklist = true;
    let root = comrak::parse_document(&arena, md, &opts);
    let mut kinds = Vec::new();
    for child in root.children() {
        let label: &'static str = match &child.data.borrow().value {
            NodeValue::Heading(h) => match h.level {
                1 => "heading-1",
                2 => "heading-2",
                3 => "heading-3",
                4 => "heading-4",
                5 => "heading-5",
                6 => "heading-6",
                _ => continue,
            },
            // OI-0002: mirror the parser's promotion rule — an
            // image-only paragraph (images + whitespace only) is a
            // block-level `image` in the IR.
            NodeValue::Paragraph => {
                let image_only = child.children().count() > 0
                    && child.children().all(|c| {
                        matches!(
                            &c.data.borrow().value,
                            NodeValue::Image(_) | NodeValue::SoftBreak | NodeValue::LineBreak
                        ) || matches!(
                            &c.data.borrow().value,
                            NodeValue::Text(t) if t.trim().is_empty()
                        )
                    })
                    && child
                        .children()
                        .any(|c| matches!(&c.data.borrow().value, NodeValue::Image(_)));
                if image_only { "image" } else { "paragraph" }
            }
            NodeValue::Table(_) => "table",
            NodeValue::CodeBlock(_) => "code-block",
            NodeValue::List(_) => "list",
            NodeValue::BlockQuote => "blockquote",
            NodeValue::ThematicBreak => "thematic-break",
            // Raw HTML blocks are first-class IR blocks (`BlockKind::Html`,
            // wire kind "html"). Without this arm they fall through to
            // `continue` and the comparison would be blind to them — the
            // regenerated document could drop an html block and still pass.
            NodeValue::HtmlBlock(_) => "html",
            _ => continue,
        };
        kinds.push(label.to_string());
    }
    kinds
}

/// Walk the alignment map's top-level rows (parent_id == None) and
/// produce a kind sequence comparable to the reparsed output. List items
/// collapse back to a single "list" group at the top level.
fn source_top_level_kinds(output: &transync::TranslationOutput) -> Vec<String> {
    let mut kinds: Vec<String> = Vec::new();
    for row in &output.alignment_map.blocks {
        if row.parent_id.is_some() {
            continue;
        }
        let mapped = if row.block_kind == "list-item" {
            "list".to_string()
        } else {
            row.block_kind.clone()
        };
        if mapped == "list" && matches!(kinds.last().map(String::as_str), Some("list")) {
            continue;
        }
        kinds.push(mapped);
    }
    kinds
}
