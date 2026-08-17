//! Shared structural walk over top-level blocks: the ONE home for the
//! source-IR → reparsed-AST normalization rule and the per-list direct
//! item count.
//!
//! Both `transync-core`'s `validate::full_reparse` and this crate's
//! renderer build on the same fact: consecutive `BlockKind::ListItem`
//! rows that share an `ast_path` prefix are the items of ONE Comrak
//! `List` node, so they collapse into a single top-level entry. Keeping
//! two copies of that rule in two crates guarantees drift, so it lives
//! here and both consumers import it (spec 2026-08-04 §4.1).
//!
//! TRACE: DCR-0017
//! TRACE: SCN-14
//! TRACE: R0004-0001

use crate::id::{BlockId, BlockKind};
use crate::parser::Document;

/// One normalized top-level entry: a kind label plus the source-side
/// block IDs that contributed to it. Consecutive list-items collapse
/// into one entry — see [`normalize_top_level`].
#[derive(Debug, Clone)]
pub struct NormalizedEntry {
    pub label: &'static str,
    pub sources: Vec<BlockId>,
}

/// Project source top-level blocks into normalized entries that match
/// what Comrak emits when the regenerated MD is reparsed:
/// consecutive list-items collapse into a single `list` aggregate,
/// and each entry remembers its contributing source `BlockId`s so
/// the diagnostic + fallback paths can name them.
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
pub fn normalize_top_level(doc: &Document) -> Vec<NormalizedEntry> {
    let mut out: Vec<NormalizedEntry> = Vec::new();
    // Sibling items of one source List node share their ast_path
    // PREFIX — the parser stores each item as the owning List's path
    // plus the item's own index (R0006-0008) — so collapse only within
    // a single list: CommonMark starts a NEW list on a marker-type
    // change (`1.` vs `-`, `-` vs `*`, `.` vs `)`), and Comrak emits
    // separate top-level List nodes on reparse.
    let mut last_list_path: Option<Vec<usize>> = None;
    for block in doc.blocks.iter() {
        // `label_for` already normalizes `ListItem` to "list" (the label a
        // reparsed `List` node reports); there is no separate "list-item"
        // label to remap.
        let label = label_for(&block.kind);
        if label == "list" {
            let list_path: &[usize] = block
                .ast_path
                .0
                .split_last()
                .map_or(&[][..], |(_, prefix)| prefix);
            if last_list_path.as_deref() == Some(list_path)
                && let Some(last) = out.last_mut()
                && last.label == "list"
            {
                last.sources.push(block.block_id.clone());
                continue;
            }
            last_list_path = Some(list_path.to_vec());
        } else {
            last_list_path = None;
        }
        out.push(NormalizedEntry {
            label,
            sources: vec![block.block_id.clone()],
        });
    }
    out
}

/// The normalized top-level label for a source block kind. Must stay in
/// exact step with the reparse-side label match in
/// `transync_core::validate::full_reparse::reparse_full` — the two
/// sequences are compared element-wise.
///
/// For all but two kinds this label IS [`BlockKind::wire_str`], so it
/// delegates rather than restating those values (R0002-0085): the enum owns
/// the spelling, and only the places where a *reparse* genuinely disagrees
/// with the wire form are written out here. `heading-1..6`, `paragraph`,
/// `table`, `code-block`, `blockquote`, `thematic-break`, `skipped` and —
/// per spec §4.2(4) — `html` all round-trip unchanged, so a wire spelling
/// and a normalized label can no longer drift apart in silence. The
/// delegated values are still pinned as literals by this module's
/// `label_for_pins_every_kind` test, which is what keeps a `wire_str`
/// rename from moving them out from under the AST-side comparison.
///
/// TRACE: SCN-14
pub(crate) fn label_for(kind: &BlockKind) -> &'static str {
    match kind {
        // The two kinds whose reparsed node is NOT what the wire form says.
        // A source list-item is one item of a `List` node, and the whole
        // list normalizes to a single `list` entry (see
        // [`normalize_top_level`]) — never `list-item`.
        BlockKind::ListItem { .. } => "list",
        // An image-only paragraph is a wire-level `image` promotion
        // (OI-0002), but Comrak has no block-level image node: it reparses
        // as the `Paragraph` it was written as.
        BlockKind::Image => "paragraph",
        other => other.wire_str(),
    }
}

/// The normalized top-level label for a **reparsed Comrak node** — the
/// AST-side twin of `label_for`, and the other half of the same
/// one-home rule: the two mappings are compared element-wise, so they
/// cannot live in separate crates without drifting.
///
/// `None` means the node has no normalized label at all (a heading level
/// Comrak cannot produce); callers skip such a node.
///
/// Consumers: `transync_core::validate::full_reparse` (whole-sequence
/// comparison against [`normalize_top_level`]) and this crate's renderer
/// (per-row pairing of an alignment row with the node at its cursor).
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
pub fn node_label<'a>(node: &'a comrak::nodes::AstNode<'a>) -> Option<&'static str> {
    use comrak::nodes::NodeValue;
    Some(match &node.data.borrow().value {
        NodeValue::Heading(h) => match h.level {
            1 => "heading-1",
            2 => "heading-2",
            3 => "heading-3",
            4 => "heading-4",
            5 => "heading-5",
            6 => "heading-6",
            _ => return None,
        },
        NodeValue::Paragraph => "paragraph",
        NodeValue::Table(_) => "table",
        NodeValue::CodeBlock(_) => "code-block",
        NodeValue::List(_) => "list",
        NodeValue::BlockQuote => "blockquote",
        NodeValue::ThematicBreak => "thematic-break",
        // Spec §4.2(4): a spliced html block must label "html" on both
        // sides — the old catch-all bucketed it as "skipped", which
        // would fail-cascade every successfully translated block.
        NodeValue::HtmlBlock(_) => "html",
        // A7: an unmodeled top-level node (footnote definition, …) is a
        // Skipped block on the source side; label it the same here so
        // the round-trip aligns instead of silently dropping the node.
        _ => "skipped",
    })
}

/// Direct `Item` / `TaskItem` children of a `List` node.
///
/// Nested lists' items are deliberately NOT counted: a source list item
/// owning a sub-list contributes exactly one top-level `ListItem` row to
/// the IR, so only the outer items may be compared against
/// [`NormalizedEntry::sources`] (spec 2026-08-04 §4.3).
///
/// `TaskItem` counts alongside `Item` — with the tasklist extension on,
/// a checkbox item is a `TaskItem` node, and a task list's items are
/// list items like any other.
///
/// TRACE: SCN-14
pub fn direct_item_count<'a>(list: &'a comrak::nodes::AstNode<'a>) -> usize {
    use comrak::nodes::NodeValue;
    list.children()
        .filter(|c| {
            matches!(
                c.data.borrow().value,
                NodeValue::Item(_) | NodeValue::TaskItem(_)
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parse `md` and hand back the first top-level node, so the
    /// item-count tests can feed `direct_item_count` a real `List`.
    fn first_top_level<'a>(
        arena: &'a comrak::Arena<comrak::nodes::AstNode<'a>>,
        md: &str,
    ) -> &'a comrak::nodes::AstNode<'a> {
        let root = comrak::parse_document(arena, md, &crate::parser::comrak_options());
        root.children().next().expect("md has a top-level block")
    }

    /// Spec §4.3: a plain bullet list reports its item count.
    #[test]
    fn direct_item_count_counts_plain_items() {
        let arena = comrak::Arena::new();
        let list = first_top_level(&arena, "- a\n- b\n- c\n");
        assert_eq!(direct_item_count(list), 3);
    }

    /// Spec §4.3: TaskItem children count too — with the tasklist
    /// extension on, checkbox items are `TaskItem`, not `Item`, and a
    /// count that ignored them would read every task list as empty.
    #[test]
    fn direct_item_count_counts_task_items() {
        let arena = comrak::Arena::new();
        let list = first_top_level(&arena, "- [ ] a\n- [x] b\n");
        assert_eq!(direct_item_count(list), 2);
    }

    /// Spec §4.3: a nested list's items belong to their own `List` node;
    /// only the outer items are counted, because only they have a
    /// top-level `ListItem` row in the IR.
    #[test]
    fn direct_item_count_excludes_nested_list_items() {
        let arena = comrak::Arena::new();
        let list = first_top_level(&arena, "- a\n  - a1\n  - a2\n- b\n");
        assert_eq!(direct_item_count(list), 2);
    }

    /// A non-list node has no `Item` children — 0, not a panic.
    #[test]
    fn direct_item_count_of_non_list_is_zero() {
        let arena = comrak::Arena::new();
        let para = first_top_level(&arena, "just a paragraph\n");
        assert_eq!(direct_item_count(para), 0);
    }

    /// R0002-0085: `label_for` delegates to [`BlockKind::wire_str`] for
    /// every kind but `ListItem` and `Image`, so a wire-spelling change
    /// would otherwise move the normalized labels silently — and those
    /// labels are compared element-wise against `node_label`, whose values
    /// are hand-written literals that would NOT follow. Pinning the whole
    /// mapping as literals here is what makes the delegation safe: rename a
    /// wire label and this test names the break.
    #[test]
    fn label_for_pins_every_kind() {
        let cases: [(BlockKind, &str); 15] = [
            (BlockKind::Heading1, "heading-1"),
            (BlockKind::Heading2, "heading-2"),
            (BlockKind::Heading3, "heading-3"),
            (BlockKind::Heading4, "heading-4"),
            (BlockKind::Heading5, "heading-5"),
            (BlockKind::Heading6, "heading-6"),
            (BlockKind::Paragraph, "paragraph"),
            (BlockKind::Table, "table"),
            (
                BlockKind::CodeBlock {
                    info: None,
                    fenced: true,
                },
                "code-block",
            ),
            (
                BlockKind::ListItem {
                    ordered: false,
                    task: None,
                },
                "list",
            ),
            (BlockKind::Blockquote, "blockquote"),
            (BlockKind::ThematicBreak, "thematic-break"),
            (BlockKind::Image, "paragraph"),
            (BlockKind::Html { block_type: 6 }, "html"),
            (
                BlockKind::Skipped {
                    label: "unsupported".to_string(),
                },
                "skipped",
            ),
        ];
        for (kind, expected) in cases {
            assert_eq!(label_for(&kind), expected, "for {kind:?}");
        }
    }

    /// The reparse-side mapping must answer with the SAME labels
    /// `label_for` produces for the corresponding source kinds — that
    /// element-wise equality is the whole point of both `reparse_full`'s
    /// sequence check and the renderer's row↔node pairing.
    #[test]
    fn node_label_matches_the_source_side_labels() {
        let arena = comrak::Arena::new();
        let cases = [
            ("para\n", "paragraph"),
            ("# h\n", "heading-1"),
            ("###### h\n", "heading-6"),
            ("| a |\n|---|\n| 1 |\n", "table"),
            ("```\nx\n```\n", "code-block"),
            ("- a\n", "list"),
            ("> q\n", "blockquote"),
            ("---\n", "thematic-break"),
            ("<div>x</div>\n", "html"),
        ];
        for (md, expected) in cases {
            let node = first_top_level(&arena, md);
            assert_eq!(node_label(node), Some(expected), "for {md:?}");
        }
    }

    /// A7: an unmodeled top-level node labels "skipped" — the same label
    /// `label_for` gives `BlockKind::Skipped`, so the round-trip aligns.
    ///
    /// No node maps here under `parser::comrak_options()` (footnotes,
    /// front matter and description lists are all off), so the specimen is
    /// built synthetically — the same pattern the parser / render / align
    /// `Skipped` tests use.
    #[test]
    fn node_label_of_an_unmodeled_node_is_skipped() {
        use comrak::nodes::{Ast, AstNode, LineColumn, NodeValue};
        let arena: comrak::Arena<AstNode> = comrak::Arena::new();
        let node = arena.alloc(AstNode::new(std::cell::RefCell::new(Ast::new(
            NodeValue::FootnoteDefinition(comrak::nodes::NodeFootnoteDefinition {
                name: "1".to_string(),
                total_references: 0,
            }),
            LineColumn { line: 1, column: 1 },
        ))));
        assert_eq!(node_label(node), Some("skipped"));
    }

    /// The collapse rule: two items of one source list become ONE entry
    /// carrying both ids, and a marker-type change starts a second entry.
    #[test]
    fn normalize_collapses_one_list_and_splits_on_marker_change() {
        let mut doc = crate::parser::parse("- a\n- b\n\n* c\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let norm = normalize_top_level(&doc);
        let shape: Vec<(&str, usize)> = norm.iter().map(|e| (e.label, e.sources.len())).collect();
        assert_eq!(shape, vec![("list", 2), ("list", 1)]);
    }
}
