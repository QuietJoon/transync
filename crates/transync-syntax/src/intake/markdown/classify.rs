//! Comrak `NodeValue` → [`BlockKind`], plus the labels a node that has no
//! kind gets instead.
//!
//! Everything here is a pure projection of one AST node: no IDs, no ranges,
//! no walker state. The walker's match arms decide *which* projection applies
//! and where the result is emitted; this module owns *what* the answer is.
//! That split is what keeps the structural policy — block-level image
//! promotion, "a list's ordered-ness lives on the List, not the item" — in
//! one readable place instead of interleaved with ID allocation.

use crate::id::BlockKind;
use comrak::nodes::{AstNode, NodeCodeBlock, NodeValue};

/// Heading level → kind, or `None` for a level Comrak should never produce.
///
/// The level→kind direction. Its inverse, [`BlockKind::heading_level`], is
/// single-homed on the enum itself so the two cannot drift apart silently.
pub(super) fn heading_kind(level: u8) -> Option<BlockKind> {
    match level {
        1 => Some(BlockKind::Heading1),
        2 => Some(BlockKind::Heading2),
        3 => Some(BlockKind::Heading3),
        4 => Some(BlockKind::Heading4),
        5 => Some(BlockKind::Heading5),
        6 => Some(BlockKind::Heading6),
        _ => None,
    }
}

/// Paragraph → [`BlockKind::Image`] or [`BlockKind::Paragraph`].
///
/// OI-0002: an image-only paragraph (one or more images with nothing but
/// whitespace between them — the badge-row form included) is a block-level
/// image: `BlockKind::Image`, rendered as a `<figure>` sync anchor. Comrak
/// has no block-level image node, so this is where the promotion happens.
/// Paragraphs mixing images with prose stay paragraphs.
pub(super) fn paragraph_kind<'a>(node: &'a AstNode<'a>) -> BlockKind {
    if paragraph_is_image_only(node) {
        BlockKind::Image
    } else {
        BlockKind::Paragraph
    }
}

/// Fenced/indented code block → kind, carrying the info string when there is
/// one. An empty info string is `None`, not `Some("")`, so regen re-emits a
/// bare fence rather than a fence with a trailing space.
///
/// `fenced` records how the SOURCE spelled the block, and is load-bearing
/// rather than informational: an indented block's comrak `Sourcepos` starts
/// after the columns of block structure it consumed, so `markdown::emit` moves
/// its range back over that indent and `unit::payload::assemble` synthesizes
/// the fenced wire payload. Both read this flag, so discarding it disarms
/// both — and what that cost depends on the indent's depth (ti 457e51). A
/// four-space block failed `fragment_reparse` three times and fell back
/// untranslated, which is loud and self-reporting. A block indented eight
/// columns or more sliced to something that was *itself* a valid indented
/// code block, so it passed every per-unit layer, was accepted on attempt
/// one, and was then regenerated into an unclosed fence that swallowed the
/// following paragraph — contained only by `validate::full_reparse` and the
/// DCR-0004 cascade. See `markdown::emit::snap_indented_code_start` for the
/// two mechanisms in full; the second one is silent.
pub(super) fn code_block_kind(code: &NodeCodeBlock) -> BlockKind {
    let info = if code.info.is_empty() {
        None
    } else {
        Some(code.info.clone())
    };
    BlockKind::CodeBlock {
        info,
        fenced: code.fenced,
    }
}

/// A `List`'s child → a list-item kind, or `None` for a child that is not an
/// item at all.
///
/// `ordered` comes from the owning `List` node on purpose: a `TaskItem` AST
/// node carries no list-type marker of its own, so the item cannot answer
/// this question about itself.
pub(super) fn list_item_kind(value: &NodeValue, ordered: bool) -> Option<BlockKind> {
    match value {
        NodeValue::Item(_) => Some(BlockKind::ListItem {
            ordered,
            task: None,
        }),
        NodeValue::TaskItem(marker) => {
            let task = match marker {
                Some('x') | Some('X') => Some(true),
                _ => Some(false),
            };
            Some(BlockKind::ListItem { ordered, task })
        }
        _ => None,
    }
}

/// A top-level node the pipeline does not model as a translatable kind →
/// [`BlockKind::Skipped`] carrying its machine label.
pub(super) fn skipped_kind(value: &NodeValue) -> BlockKind {
    BlockKind::Skipped {
        label: skipped_label(value).to_string(),
    }
}

/// The warning that accompanies a [`skipped_kind`] emission (R0008-0013):
/// the node is preserved, not dropped, and the note says so. Lives beside the
/// label tables it reads rather than inline in the walker's catch-all arm.
pub(super) fn skip_warning(value: &NodeValue) -> String {
    format!(
        "top-level {} is not translatable: it is preserved verbatim in the \
         translated Markdown and rendered as an inert escaped placeholder \
         in both panes",
        node_kind_label(value)
    )
}

/// Machine label for a `Skipped` node's underlying source kind, stored on
/// the `BlockKind::Skipped { label }` variant and surfaced via the DOM
/// `data-skipped` marker attribute (A6). The prose [`node_kind_label`] is
/// kept separately for warning text. Raw HTML blocks no longer reach here:
/// spec 2026-08-03 §3.1 gives them their own `BlockKind::Html` arm above
/// the walker's catch-all.
fn skipped_label(value: &NodeValue) -> &'static str {
    match value {
        NodeValue::FootnoteDefinition(_) => "footnote-definition",
        NodeValue::FrontMatter(_) => "front-matter",
        _ => "unsupported",
    }
}

/// Human-readable label for a source node the parser does not model as a
/// sync anchor. Names the common cases explicitly and derives a short name
/// from the Debug form otherwise. R0008-0013.
fn node_kind_label(value: &NodeValue) -> String {
    match value {
        NodeValue::FootnoteDefinition(_) => "footnote definition".to_string(),
        NodeValue::FrontMatter(_) => "front matter".to_string(),
        other => {
            let dbg = format!("{other:?}");
            let name = dbg.split(['(', ' ', '{']).next().unwrap_or("node");
            format!("unsupported node `{name}`")
        }
    }
}

/// Whether a paragraph node contains only images (at least one), with
/// nothing but whitespace text and soft/line breaks between them. Such
/// a paragraph is a block-level image per `contracts.md` §3/§4
/// (`BlockKind::Image`, `<figure>` sync wrapper). OI-0002.
fn paragraph_is_image_only<'a>(node: &'a AstNode<'a>) -> bool {
    let mut saw_image = false;
    for child in node.children() {
        match &child.data.borrow().value {
            NodeValue::Image(_) => saw_image = true,
            NodeValue::Text(t) if t.trim().is_empty() => {}
            NodeValue::SoftBreak | NodeValue::LineBreak => {}
            _ => return false,
        }
    }
    saw_image
}

// OI-0002: block-level image detection.
#[cfg(test)]
mod image_block_tests {
    use super::super::parse;
    use crate::id::BlockKind;

    fn kinds(src: &str) -> Vec<BlockKind> {
        parse(src)
            .expect("parses")
            .blocks
            .iter()
            .map(|b| b.kind.clone())
            .collect()
    }

    #[test]
    fn image_only_paragraph_becomes_image_block() {
        let kinds = kinds("![alt text](img.png)\n");
        assert_eq!(kinds, vec![BlockKind::Image]);
    }

    #[test]
    fn badge_row_of_images_becomes_image_block() {
        let kinds = kinds("![a](x.svg) ![b](y.svg)\n");
        assert_eq!(kinds, vec![BlockKind::Image]);
    }

    #[test]
    fn paragraph_mixing_image_and_prose_stays_paragraph() {
        let kinds = kinds("See ![icon](i.png) for details.\n");
        assert_eq!(kinds, vec![BlockKind::Paragraph]);
    }

    #[test]
    fn linked_image_paragraph_stays_paragraph() {
        // A link wrapping an image is interaction content, not a plain
        // figure; keep the paragraph fallback (still a valid sync anchor).
        let kinds = kinds("[![badge](b.svg)](https://ci.example)\n");
        assert_eq!(kinds, vec![BlockKind::Paragraph]);
    }

    #[test]
    fn image_block_ids_use_img_prefix_and_round_trip() {
        let mut doc = parse("intro\n\n![alt](img.png)\n\noutro\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let img = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Image))
            .expect("image block present");
        assert!(img.block_id.0.starts_with("img-"), "got {}", img.block_id.0);
    }
}
