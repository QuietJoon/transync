//! Full-document reparse: parse the regenerated Markdown end-to-end and
//! assert that the block-kind sequence, the block count, and each list's
//! item count match the source IR.
//!
//! On rejection, returns a [`ReparseFailure`] carrying both a
//! human-readable reason (with source-side `BlockId`s flanking the
//! divergence point) and the list of source `BlockId`s the pipeline
//! should mark as fallback before re-running regen.
//!
//! Divergence attribution uses the regenerator's per-block byte
//! offsets ([`BlockOffsets`]): each reparsed top-level block is mapped
//! back to the source block whose target byte range contains it. A
//! source block that owns 0 or ≥2 reparsed top-level blocks is the
//! offender.
//!
//! TRACE: SCN-14
//! TRACE: R0004-0001

use crate::id::BlockId;
use crate::parser::Document;
use crate::parser::ranges::LineOffsets;
use crate::regen::BlockOffsets;
use comrak::nodes::NodeValue;
use transync_syntax::walk::{NormalizedEntry, direct_item_count, node_label, normalize_top_level};

/// Diagnostic payload returned when [`reparse_full`] rejects.
///
/// `reason` is for logging; `divergent_source_blocks` is for the
/// pipeline's per-block fallback path (`FullReparseFailure::FallbackPerBlock`)
/// — these are the source-side block IDs to mark as `FallbackSource`
/// before re-regenerating.
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
#[derive(Debug, Clone)]
pub struct ReparseFailure {
    pub reason: String,
    pub divergent_source_blocks: Vec<BlockId>,
}

impl std::fmt::Display for ReparseFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// One reparsed top-level block: its kind label, its byte start in the
/// regenerated MD (for offset attribution), and the node itself (for the
/// per-list item count, spec §4.3).
struct RegenTop<'a> {
    label: &'static str,
    start: usize,
    node: &'a comrak::nodes::AstNode<'a>,
}

/// Reparse the regenerated MD and verify anchors match the source.
///
/// `offsets` is the regenerator's per-source-block byte-range map. It
/// is used to attribute each reparsed top-level block back to the
/// source block whose range contains it, so the failure payload's
/// `divergent_source_blocks` field names the actual offender(s) (block
/// that lost or gained top-level structure) — not just the seam at the
/// kind-mismatch index.
///
/// Three scans, in order: (1) kind divergence at a shared index, (2)
/// top-level count drift after a clean prefix, (3) per-list item count
/// (spec §4.3). Scan 3 runs last because it indexes both sides by the
/// same `i`, which only the first two scans passing makes meaningful.
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
pub fn reparse_full(
    source_doc: &Document,
    regenerated_md: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    let normalized_source = normalize_top_level(source_doc);
    let regen_lines = LineOffsets::new(regenerated_md);

    let arena = comrak::Arena::new();
    let parser_opts = crate::parser::comrak_options();
    let root = comrak::parse_document(&arena, regenerated_md, &parser_opts);

    // Reparsed top-level blocks, in document order.
    let mut regen_top: Vec<RegenTop<'_>> = Vec::new();
    for child in root.children() {
        // The reparse-side label mapping lives in `transync_syntax::walk`
        // alongside its source-side twin (`label_for`): the renderer's zip
        // pairs rows with nodes using the same two functions, and two
        // copies in two crates would drift. `None` = a node with no
        // normalized label (unproducible heading level) — skip it.
        let Some(label) = node_label(child) else {
            continue;
        };
        let pos = child.data.borrow().sourcepos;
        let start = regen_lines.pos_to_byte(pos.start.line, pos.start.column);
        regen_top.push(RegenTop {
            label,
            start,
            node: child,
        });
    }

    // First scan: kind divergence at a shared index.
    let common = normalized_source.len().min(regen_top.len());
    for i in 0..common {
        let s = normalized_source[i].label;
        let r = regen_top[i].label;
        if s != r {
            let neighbors = describe_neighbors(&normalized_source, i);
            let suspects = attribute_offenders(&normalized_source, &regen_top, offsets);
            return Err(ReparseFailure {
                reason: format!("block {i}: source kind {s} != regenerated kind {r}{neighbors}"),
                divergent_source_blocks: if suspects.is_empty() {
                    normalized_source[i].sources.clone()
                } else {
                    suspects
                },
            });
        }
    }

    // Second scan: count drift after a clean prefix.
    if regen_top.len() != normalized_source.len() {
        let neighbors = describe_neighbors(&normalized_source, common);
        let suspects = attribute_offenders(&normalized_source, &regen_top, offsets);
        let divergent_source_blocks = if suspects.is_empty() {
            normalized_source
                .get(common)
                .or_else(|| normalized_source.last())
                .map(|n| n.sources.clone())
                .unwrap_or_default()
        } else {
            suspects
        };
        return Err(ReparseFailure {
            reason: format!(
                "regenerated block count {} differs from source top-level count {}{neighbors}",
                regen_top.len(),
                normalized_source.len(),
            ),
            divergent_source_blocks,
        });
    }

    // Third scan (spec §4.3): per-list item count. The label sequence and
    // the top-level count already agree, so index `i` names the same block
    // on both sides — and a "list" label is only ever produced by a
    // `NodeValue::List`. A collapsed entry knows how many source
    // `ListItem` rows fed it; the reparsed `List` must expose exactly that
    // many DIRECT `Item`/`TaskItem` children.
    //
    // Defense in depth: `per_kind::check_list` already rejects naive
    // per-unit splits and merges, so what lands here are the residuals it
    // cannot see — units whose `expected_list_topology` came back `None`
    // (the check then no-ops entirely) and cross-unit splice-adjacency
    // effects, where individually-valid payloads merge or split at a regen
    // boundary without moving the top-level label sequence. It is also the
    // invariant the render zip's row↔item pairing needs.
    //
    // Note (2026-08-05): the `expected_list_topology == None` residual is now
    // void unqualified (DCR-0017 Guard 1, dated note). Its only known route
    // was a mis-sliced empty payload from the lone-CR sourcepos desync
    // (OI-0033), and `LineOffsets` has counted lone CR as a line boundary
    // since that fix; `unit::payload::lone_cr_topology_tests` asserts the predicate
    // directly, and `parser::emit`'s empty-range guard makes any future
    // quirk of that class loud. What this scan still owns is the cross-unit
    // splice-adjacency effect above, plus the render zip's pairing invariant.
    for (i, entry) in normalized_source.iter().enumerate() {
        if entry.label != "list" {
            continue;
        }
        let node = regen_top[i].node;
        debug_assert!(
            matches!(node.data.borrow().value, NodeValue::List(_)),
            "a \"list\" label is only emitted for NodeValue::List",
        );
        let got = direct_item_count(node);
        let expected = entry.sources.len();
        if got != expected {
            return Err(ReparseFailure {
                reason: format!(
                    "list at block {i}: source has {expected} items, regenerated has {got}"
                ),
                divergent_source_blocks: entry.sources.clone(),
            });
        }
    }

    Ok(())
}

/// Source-side block IDs that own ≠ 1 reparsed top-level block (0 =
/// lost, ≥ 2 = over-produced). Walks `normalized` in source order so
/// the returned vec is deterministic.
fn attribute_offenders(
    normalized: &[NormalizedEntry],
    regen_top: &[RegenTop<'_>],
    offsets: &BlockOffsets,
) -> Vec<BlockId> {
    let mut suspects: Vec<BlockId> = Vec::new();
    for entry in normalized {
        // Collapsed lists have multiple contributing IDs; use their
        // union range for containment.
        let mut span: Option<(usize, usize)> = None;
        for id in &entry.sources {
            if let Some(range) = offsets.0.get(id) {
                let (s, e) = span.unwrap_or((range.start, range.end));
                span = Some((s.min(range.start), e.max(range.end)));
            }
        }
        let Some((start, end)) = span else { continue };
        let owned = regen_top
            .iter()
            .filter(|top| top.start >= start && top.start < end)
            .count();
        if owned != 1 {
            // For a collapsed list we can't pin which item over- or
            // under-produced, so fall back the whole group.
            suspects.extend(entry.sources.iter().cloned());
        }
    }
    suspects
}

/// Format the source-side neighbors of the divergence point so the
/// error message names the blocks the consumer can act on.
fn describe_neighbors(normalized: &[NormalizedEntry], index: usize) -> String {
    let before = if index > 0 {
        normalized
            .get(index - 1)
            .and_then(|n| n.sources.last())
            .map(|id| id.0.as_str())
    } else {
        None
    };
    let at = normalized
        .get(index)
        .and_then(|n| n.sources.first())
        .map(|id| id.0.as_str());

    match (before, at) {
        (Some(b), Some(a)) => format!(" (between source blocks `{b}` and `{a}`)"),
        (Some(b), None) => format!(" (after source block `{b}`)"),
        (None, Some(a)) => format!(" (at source block `{a}`)"),
        (None, None) => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::BlockKind;
    use crate::parser::parse;
    use crate::regen::regenerate;

    /// Build a `BlockOffsets` for `regenerated_md == source.source_text`,
    /// i.e. each block's target range equals its source range. Suitable
    /// for tests that pass a hand-crafted `regenerated_md` matching
    /// the source layout, or that don't need precise attribution.
    fn dummy_offsets(doc: &Document) -> BlockOffsets {
        let mut offsets = BlockOffsets::default();
        for b in doc.blocks.iter() {
            offsets.0.insert(b.block_id.clone(), b.source_range);
        }
        offsets
    }

    /// `R0001-0083` in the removed `reviews/reviewed/0001.md`: confirm
    /// reparse_full rejects a regenerated document
    /// whose top-level structure diverges from the source.
    #[test]
    fn reparse_full_rejects_extra_top_level_block() {
        let src = "# Title\n\nsome text\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        // Inject an extra top-level paragraph that wasn't in the source IR.
        let bad_regen = "# Title\n\nsome text\n\nuninvited extra paragraph\n";
        let err = reparse_full(&doc, bad_regen, &offsets)
            .expect_err("regen with extra top-level block must be rejected");
        assert!(err.reason.contains("regenerated block count"), "got: {err}");
        // R0004-0001: the error must name the source-side neighbor so
        // consumers can locate the regression.
        assert!(err.reason.contains("`"), "got: {err}");
    }

    #[test]
    fn reparse_full_rejects_kind_substitution() {
        let src = "# Title\n\nsome text\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        // Replace the heading with a paragraph — same count, wrong kind
        // at position 0.
        let bad_regen = "Title\n\nsome text\n";
        let err = reparse_full(&doc, bad_regen, &offsets)
            .expect_err("kind-substituted regen must be rejected");
        assert!(err.reason.contains("kind"), "got: {err}");
        // R0004-0001: the divergent source block list must be populated
        // so the pipeline's per-block fallback can act on it.
        assert!(
            !err.divergent_source_blocks.is_empty(),
            "divergent_source_blocks should be populated"
        );
    }

    #[test]
    fn reparse_full_accepts_byte_identical_regen() {
        let src = "# Title\n\nbody paragraph\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        // The trivial passthrough case must accept; regression-guard
        // against an over-eager validator.
        reparse_full(&doc, src, &offsets).expect("byte-identical regen must accept");
    }

    /// Bonus observation #1 from R0004-0001: a list-item immediately
    /// followed by a paragraph. Comrak does NOT fold the paragraph
    /// into the list (loose-list is a within-list concern, not a
    /// list-then-paragraph one), so the normalizer should leave the
    /// two as distinct top-level entries and the round-trip should
    /// accept.
    #[test]
    fn reparse_full_accepts_list_item_then_paragraph() {
        let src = "- item one\n\nparagraph after\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        reparse_full(&doc, src, &offsets)
            .expect("list-item followed by paragraph should reparse to the same top-level shape");
    }

    /// Bonus observation #2 from R0004-0001: a code block whose source
    /// info string is empty regenerated as a code block with a
    /// non-empty info string. The kind label is still "code-block" on
    /// both sides, so the count + kind sequence should match. This
    /// guards against an over-eager future change that adds info-string
    /// equality to the reparse check (which would break the
    /// "translator may set the info string when source had none"
    /// contract).
    #[test]
    fn reparse_full_accepts_code_block_with_added_info_string() {
        let src = "```\nlet x = 1;\n```\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        let regen = "```rust\nlet x = 1;\n```\n";
        reparse_full(&doc, regen, &offsets)
            .expect("code-block with added info string should still match");
    }

    /// Spec §4.2(4): a document with a top-level raw HTML block round-trips
    /// `reparse_full` when regen is byte-identical to the source — both
    /// sides label the node "html" so the sequence aligns.
    #[test]
    fn reparse_full_accepts_raw_html_round_trip() {
        let src = "<div class=\"note\">n</div>\n\nreal paragraph\n";
        let doc = parse(src).expect("source parses");
        assert!(
            matches!(doc.blocks[0].kind, BlockKind::Html),
            "fixture must open with an Html block",
        );
        let offsets = dummy_offsets(&doc);
        reparse_full(&doc, src, &offsets)
            .expect("byte-identical regen with a raw HTML block must accept");
    }

    /// A7: a regen that drops the raw HTML block entirely is rejected,
    /// and the offending html block is named in the fallback list.
    #[test]
    fn reparse_full_rejects_dropped_html_block() {
        let src = "<div class=\"note\">n</div>\n\nreal paragraph\n";
        let doc = parse(src).expect("source parses");
        let html_id = doc.blocks[0].block_id.clone();
        assert!(matches!(doc.blocks[0].kind, BlockKind::Html));

        // The html block's target range collapsed to empty (dropped); only
        // the paragraph survives in the regenerated MD.
        let bad_regen = "real paragraph\n";
        let mut offsets = BlockOffsets::default();
        offsets.0.insert(
            html_id.clone(),
            crate::align::ByteRange { start: 0, end: 0 },
        );
        offsets.0.insert(
            doc.blocks[1].block_id.clone(),
            crate::align::ByteRange {
                start: 0,
                end: bad_regen.len(),
            },
        );

        let err = reparse_full(&doc, bad_regen, &offsets)
            .expect_err("a regen that drops the html block must be rejected");
        assert!(
            err.divergent_source_blocks.contains(&html_id),
            "the dropped html block must be attributed; got {:?}",
            err.divergent_source_blocks,
        );
    }

    /// Spec §4.3 (a): the regenerated list gained an item. The top-level
    /// label sequence is unchanged (`list` on both sides) and the count is
    /// unchanged (one aggregate entry vs one `List` node), so ONLY the
    /// per-list item count can see this. Called directly rather than
    /// through the pipeline because a naive per-unit split dies at
    /// `per_kind::check_list` and never reaches regen.
    #[test]
    fn reparse_full_rejects_list_item_split() {
        let src = "- a\n- b\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        let err = reparse_full(&doc, "- a\n- b\n- c\n", &offsets)
            .expect_err("a regenerated list with an extra item must be rejected");
        assert!(err.reason.contains("items"), "got: {err}");
        assert!(
            err.reason.contains("source has 2 items, regenerated has 3"),
            "got: {err}"
        );
        // The cascade must downgrade exactly the list's item blocks.
        let ids: Vec<&str> = err
            .divergent_source_blocks
            .iter()
            .map(|id| id.0.as_str())
            .collect();
        assert_eq!(ids, vec!["li-0001", "li-0002"], "got: {ids:?}");
    }

    /// Spec §4.3 (b): the mirror case — the regenerated list merged two
    /// source items into one.
    #[test]
    fn reparse_full_rejects_list_item_merge() {
        let src = "- a\n- b\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        let err = reparse_full(&doc, "- a b\n", &offsets)
            .expect_err("a regenerated list that merged two items must be rejected");
        assert!(
            err.reason.contains("source has 2 items, regenerated has 1"),
            "got: {err}"
        );
    }

    /// Spec §4.3 (c): the honest case still accepts — regression guard
    /// against an item count that mis-reads a well-formed list.
    #[test]
    fn reparse_full_accepts_unchanged_list_item_count() {
        let src = "- a\n- b\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        reparse_full(&doc, src, &offsets).expect("an unchanged two-item list must accept");
    }

    /// Spec §4.3 (d): a task list's items are `TaskItem` nodes, not
    /// `Item` — the count must see them, so the honest case accepts …
    #[test]
    fn reparse_full_accepts_unchanged_task_list_item_count() {
        let src = "- [ ] a\n- [x] b\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        reparse_full(&doc, src, &offsets).expect("an unchanged two-item task list must accept");
    }

    /// … and a split task list is rejected. (A count blind to `TaskItem`
    /// would read 0 items here and reject the honest case above instead.)
    #[test]
    fn reparse_full_rejects_task_list_item_split() {
        let src = "- [ ] a\n- [x] b\n";
        let doc = parse(src).expect("source parses");
        let offsets = dummy_offsets(&doc);
        let err = reparse_full(&doc, "- [ ] a\n- [x] b\n- [ ] c\n", &offsets)
            .expect_err("a regenerated task list with an extra item must be rejected");
        assert!(
            err.reason.contains("source has 2 items, regenerated has 3"),
            "got: {err}"
        );
    }

    /// Nested items belong to their own `List` node, so a source item that
    /// owns a sub-list still counts as ONE direct item — the guard must not
    /// fire on a document that merely has nesting.
    #[test]
    fn reparse_full_accepts_nested_sublist_round_trip() {
        let src = "- a\n  - a1\n  - a2\n- b\n";
        let doc = parse(src).expect("source parses");
        let top_items = doc
            .blocks
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::ListItem { .. }))
            .count();
        assert_eq!(top_items, 2, "only the outer items are top-level rows");
        let offsets = dummy_offsets(&doc);
        reparse_full(&doc, src, &offsets).expect("nesting is not an item-count divergence");
    }

    /// R0004-0001: when a source block over-produces (one source
    /// paragraph splices into two reparsed paragraphs), byte-offset
    /// attribution must point at the over-producer, not at a
    /// neighbor.
    #[test]
    fn attribution_finds_over_producing_source_block() {
        let doc = parse(crate::test_fixtures::EVIL_SOURCE_MD).expect("source parses");
        let first_id = doc
            .blocks
            .first()
            .map(|b| b.block_id.clone())
            .expect("first top-level block");
        let payloads: std::collections::HashMap<BlockId, String> =
            crate::test_fixtures::evil_units(&doc)
                .into_iter()
                .filter_map(|u| u.accepted_payload.map(|p| (u.unit_id, p)))
                .collect();
        let (regen_md, offsets) = regenerate(&doc, &payloads);
        let err = reparse_full(&doc, &regen_md, &offsets)
            .expect_err("over-producing payload must trigger rejection");
        assert!(
            err.divergent_source_blocks.contains(&first_id),
            "expected attribution to name the over-producer p-0001; got: {:?}",
            err.divergent_source_blocks
        );
    }
}
