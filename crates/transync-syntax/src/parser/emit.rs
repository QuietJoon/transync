//! The single home for block emission: ID allocation, byte-range capture,
//! source hashing, and the empty-list-item guard.
//!
//! Every block in the IR is pushed from here. The walker's match arms decide
//! the kind and the path; nothing else touches the ordinal counter or the
//! block vector. That is what makes block-ID assignment — invariant 1's sync
//! currency — a one-file property.

use super::{AstPath, Block, WalkState};
use crate::id::{BlockId, BlockKind, Spelling};
use crate::parser::ranges::{self, ByteRange};
use comrak::nodes::AstNode;

impl WalkState<'_> {
    /// Push ONE block into the IR: bump the ordinal, mint the kind-prefixed
    /// ID, resolve `node`'s byte range, hash the bytes it slices to, and run
    /// the empty-list-item guard. Returns the new ID (the Heading arm opens
    /// its section scope with it).
    ///
    /// The single home for ID allocation, range capture and source hashing.
    /// The `NodeValue::List` arm used to carry a byte-copy of this body — the
    /// shape in which any change to hashing or clamping gets made once and
    /// missed once — and the guard was called from two places at two
    /// different points relative to the push. Here it fires exactly once per
    /// emission, always before the block lands.
    ///
    /// Two parameters exist precisely because the List arm differs from every
    /// other caller, and collapsing them was what forced the duplicate:
    /// `node` is the node whose source range the block owns (the `Item`, not
    /// the `List`), and `ast_path` is the block's OWN path (the `List`'s path
    /// plus the item's index), which is not the walker's current path.
    ///
    /// The `source_range` stored is the resolved range as
    /// [`snap_indented_code_start`] leaves it; only the slice fed to the hash
    /// is clamped to the source bounds. Both were true of the two copies this
    /// replaces. The hash is computed *after* the snap, so it covers an
    /// indented code block's complete spelling rather than its dedented body.
    pub(super) fn emit(
        &mut self,
        node: &AstNode<'_>,
        kind: BlockKind,
        spelling: Spelling,
        section_path: Vec<BlockId>,
        ast_path: Vec<usize>,
    ) -> BlockId {
        self.counter += 1;
        let id = BlockId::new(kind.id_code(), self.counter);
        let range = snap_indented_code_start(node, &kind, self.source, self.line_offsets);
        let start = range.start.min(self.source.len());
        let end = range.end.min(self.source.len()).max(start);
        let hash = crate::id::source_hash_bytes(&self.source.as_bytes()[start..end]);
        warn_on_empty_list_item_range(node, &kind, &id, range, &mut self.warnings);
        self.blocks.push(Block {
            block_id: id.clone(),
            kind,
            spelling,
            source_range: range,
            source_hash: hash,
            section_path,
            ast_path: AstPath(ast_path),
        });
        id
    }

    /// [`WalkState::emit`] for the common case: this node, at the walker's
    /// current path, **spelled Markdown**. Only the `List` arm needs the
    /// general form, because its items own a path the walker never stands on,
    /// and only the `HtmlBlock` arm needs [`WalkState::emit_html_here`],
    /// because it is the one arm whose spelling is not Markdown.
    ///
    /// The spelling is written here rather than derived from the kind on
    /// purpose (decision D2): deriving it would re-create the conflation the
    /// split removes, and it would collapse the day an intake emits a
    /// semantic kind with a non-Markdown spelling — which is the whole point
    /// of the axis.
    pub(super) fn emit_here(
        &mut self,
        node: &AstNode<'_>,
        kind: BlockKind,
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let ast_path = self.ast_path.clone();
        self.emit(node, kind, Spelling::Markdown, section_path, ast_path)
    }

    /// The ONE place a raw-HTML island's two axes are stamped together.
    ///
    /// `NodeValue::HtmlBlock` is the only Markdown-intake arm that produces a
    /// non-Markdown spelling, and the CommonMark block type it carries has to
    /// land on the *spelling*, not on the kind — the kind's job is semantics,
    /// and an island has none the Markdown intake is willing to guess at
    /// (decision D11: reclassifying `html-0007` to `t-0007` would move the
    /// block id, and with it the alignment row, the DOM anchor and the cache
    /// axis). Keeping both stamps in one function is what stops a future arm
    /// from setting one and forgetting the other.
    pub(super) fn emit_html_here(
        &mut self,
        node: &AstNode<'_>,
        block_type: u8,
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let ast_path = self.ast_path.clone();
        self.emit(
            node,
            BlockKind::Html,
            Spelling::Html {
                block_type: Some(block_type),
            },
            section_path,
            ast_path,
        )
    }
}

/// [`ranges::byte_range_for`], with one correction: an INDENTED code block's
/// start is moved back over the block-structure indent comrak consumed.
///
/// Comrak reports an indented code block's `Sourcepos` after the columns of
/// block structure it consumed, and only on the block's first line — every
/// subsequent line of the block is inside the range with its indent intact.
/// Slicing the unsnapped range therefore hands back a payload whose first
/// line alone is dedented, and that misses in **two different ways** depending
/// on how deep the source indent is (ti 457e51). Both are re-opened by
/// reverting this snap, and only the first one is loud:
///
/// - **Four columns.** What is left cannot reparse as a code block at all, so
///   `validate::fragment_reparse` rejected the unit on every attempt and the
///   block settled as `fallback_source` — three provider calls spent to
///   translate nothing, with the four columns left outside the range copied
///   back as verbatim inter-block text ahead of the regenerated fence. A cost
///   and coverage defect that reports itself (invariant 6's loud fallback).
/// - **Eight columns, or any deeper indent.** Removing four columns still
///   leaves a four-column indent, so the slice is *itself* a valid indented
///   code block. It passed every per-unit layer and was **accepted on attempt
///   one**; regeneration then re-fenced it while the four consumed columns sat
///   OUTSIDE the block's range and were copied back as gap bytes, producing an
///   indented fence marker followed by an **unclosed** fence that swallowed
///   the following paragraph. Nothing per-unit objected — only
///   `validate::full_reparse` and the DCR-0004 cascade stood between that and
///   corrupt output. This one is **silent**.
///
/// So "the pre-fix behavior was a loud fallback" is true of four-space blocks
/// and false of deeper ones; a maintainer weighing a revert is weighing a
/// silent output-corruption path, not just a wasted retry budget.
///
/// The snap walks back over the indent — the run of spaces and tabs
/// immediately preceding the reported start, bounded by the start of the line
/// — never "start minus four bytes". Comrak counts consumed *columns*, so the
/// indent is four bytes for four spaces, one byte for a tab, and four of eight
/// for a doubly-indented block whose other four columns are content; only a
/// walk over the actual whitespace run is right for all three, and the
/// line-start bound is what keeps it from reaching the preceding line.
///
/// **Why a walk rather than an unconditional snap to column 1.** Comrak's
/// column is a byte offset within the line, and the bytes it skips before a
/// block are not always indentation: a document-leading UTF-8 BOM is skipped
/// for block structure while its three bytes are still counted in the column,
/// so `"\u{feff}    code\n"` reports its code block at `1:8`. A blind snap to
/// column 1 pulls the BOM *inside* the block's range, and since an accepted
/// translation replaces exactly those bytes with a regenerated fence, it would
/// **delete the BOM** from the output — while a BOM ahead of a paragraph, a
/// heading or a fenced block survives untouched in the inter-block gap. The
/// walk stops at the first byte that is not a space or a tab, so the BOM stays
/// outside the block where every other kind already leaves it. Pinned by
/// `parser::indented_code_tests::the_snap_stops_at_a_document_leading_bom` and
/// `regen::indented_code_regen_tests::a_leading_bom_survives_an_accepted_indented_block`.
///
/// `end` needs no correction: comrak reports it at column 0 of the line after
/// the block's last content line, which `byte_range_for` already resolves to
/// the byte past that line's terminator — so an interior blank line is inside
/// the range and a trailing one is not.
///
/// Fenced blocks and every other kind pass through unchanged: their sourcepos
/// already starts at the block's first byte.
///
/// TRACE: SCN-04
fn snap_indented_code_start(
    node: &AstNode<'_>,
    kind: &BlockKind,
    source: &str,
    line_offsets: &ranges::LineOffsets<'_>,
) -> ByteRange {
    let range = ranges::byte_range_for(node, line_offsets);
    if !matches!(kind, BlockKind::CodeBlock { fenced: false, .. }) {
        return range;
    }
    let start_line = node.data.borrow().sourcepos.start.line;
    let line_start = line_offsets.pos_to_byte(start_line, 1);
    let bytes = source.as_bytes();
    let mut start = range.start.min(bytes.len());
    while start > line_start && matches!(bytes[start - 1], b' ' | b'\t') {
        start -= 1;
    }
    ByteRange {
        start: start.min(range.start),
        ..range
    }
}

/// Defensive guard: a list item that occupies a real region of the source
/// must slice to a non-empty payload.
///
/// An empty `source_range` under a **non-degenerate** `Sourcepos` means our
/// line table and comrak disagree about where the item lives — the exact
/// shape of the lone-CR desync fixed in [`ranges::LineOffsets`] (OI-0033).
/// That disagreement is silent everywhere downstream: `outcome::block_payload`
/// hands back `""`, `unit::build_batches` dispatches the empty unit anyway,
/// and `structure::inspect_list_topology("")` yields `None`, which turns
/// `validate::per_kind::check_list` into an unconditional `Ok(())` — the
/// route DCR-0017 records as Guard-1 residual (a). Warning here makes the
/// *next* sourcepos quirk of that class visible instead of letting it disarm
/// a structural validator.
///
/// **Scoped strictly to list items, by design.** Raw-HTML blocks legitimately
/// produce empty ranges today: SCN-15's `html-0008` comment reports an end
/// position *before* its start, and those bytes survive as verbatim
/// inter-block gap bytes (pinned by that scenario's byte-identity assertion).
/// Widening this guard past `ListItem` would fire on a shipped fixture.
///
/// The guard is unreachable on the current corpus — it exists to catch a
/// future regression, so there is no production path that constructs the
/// warning today; its firing behavior is pinned by calling it directly from
/// the inline tests rather than by adding a test-only seam to `parse`.
///
/// TRACE: OI-0033
fn warn_on_empty_list_item_range(
    node: &AstNode<'_>,
    kind: &BlockKind,
    id: &BlockId,
    range: ByteRange,
    warnings: &mut Vec<String>,
) {
    if !matches!(kind, BlockKind::ListItem { .. }) || range.start != range.end {
        return;
    }
    let pos = node.data.borrow().sourcepos;
    // A degenerate position spans no source region at all, so an empty range
    // is the honest answer for it rather than evidence of a desync.
    if (pos.start.line, pos.start.column) == (pos.end.line, pos.end.column) {
        return;
    }
    warnings.push(format!(
        "list item {id} has an empty source range at a non-degenerate source \
         position ({}:{}-{}:{}): its payload is empty, so structural list \
         validation cannot check it",
        pos.start.line, pos.start.column, pos.end.line, pos.end.column,
    ));
}

// The `ast_path` contract every downstream grouping rule reads: a list item
// carries its owning `List` node's path plus its own index within that list,
// while every other block carries the walker's path for its own node. Pinned
// because `walk::normalize_top_level` collapses list runs by comparing item
// paths' PREFIXES and the renderer opens one `<ul>`/`<ol>` per run (DCR-0007) —
// a one-element shift in either direction silently merges or splits lists.
#[cfg(test)]
mod ast_path_tests {
    use super::super::parse;

    fn paths(src: &str) -> Vec<(&'static str, Vec<usize>)> {
        parse(src)
            .expect("parses")
            .blocks
            .iter()
            .map(|b| (b.kind.wire_str(), b.ast_path.0.clone()))
            .collect()
    }

    #[test]
    fn list_items_carry_the_owning_list_path_plus_their_own_index() {
        assert_eq!(
            paths("# T\n\npara\n\n- a\n- b\n\n* c\n\n1. one\n2. two\n"),
            vec![
                ("heading-1", vec![0]),
                ("paragraph", vec![1]),
                ("list-item", vec![2, 0]),
                ("list-item", vec![2, 1]),
                ("list-item", vec![3, 0]),
                ("list-item", vec![4, 0]),
                ("list-item", vec![4, 1]),
            ],
            "each item = its List node's path + the item index; a marker change \
             starts a new List and therefore a new prefix",
        );
    }

    #[test]
    fn nested_and_task_items_keep_the_outer_list_prefix() {
        // A sub-list lives inside its parent item's source range and is never
        // visited, so only the OUTER items get rows — with the outer list's path.
        assert_eq!(
            paths("- a\n  - a1\n  - a2\n- b\n"),
            vec![("list-item", vec![0, 0]), ("list-item", vec![0, 1])],
        );
        assert_eq!(
            paths("intro\n\n- [ ] todo\n- [x] done\n"),
            vec![
                ("paragraph", vec![0]),
                ("list-item", vec![1, 0]),
                ("list-item", vec![1, 1]),
            ],
        );
    }

    #[test]
    fn non_list_blocks_carry_their_own_walker_path() {
        assert_eq!(
            paths("> quote\n\n```\ncode\n```\n\n---\n\n<div>x</div>\n"),
            vec![
                ("blockquote", vec![0]),
                ("code-block", vec![1]),
                ("thematic-break", vec![2]),
                ("html", vec![3]),
            ],
        );
    }
}

// OI-0033 scope pins for `warn_on_empty_list_item_range`: it must fire for a
// list item whose range collapses under a real source position, and stay
// silent for everything else — including the shipped SCN-15 html fixture whose
// range legitimately collapses. The parse-level pins for the CR fix itself
// live beside `parse` in `parser.rs`.
#[cfg(test)]
mod empty_range_guard_tests {
    use super::super::{options::gfm_options, parse};
    use super::warn_on_empty_list_item_range;
    use crate::id::{BlockId, BlockKind};
    use crate::parser::ranges::ByteRange;
    use comrak::nodes::NodeValue;

    #[test]
    fn empty_html_range_does_not_trip_the_list_item_guard() {
        // SCN-15 ships a comment block whose sourcepos end precedes its start,
        // so its byte range legitimately collapses to empty. The guard is
        // scoped to list items precisely so that fixture stays silent.
        let src = "intro\n\n<!-- maintainer note: invisible -->\n\n- alpha\n- bravo\n";
        let doc = parse(src).expect("parses");
        let html = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Html))
            .expect("html block present");
        assert_eq!(
            html.source_range.start, html.source_range.end,
            "this test is only meaningful while the comment's range is empty",
        );
        assert!(
            !doc.warnings
                .iter()
                .any(|w| w.contains("empty source range")),
            "the empty html range must not warn: {:?}",
            doc.warnings,
        );
    }

    #[test]
    fn list_item_guard_fires_only_for_an_empty_range_at_a_real_position() {
        // The guard is unreachable through `parse` on the current corpus (that
        // is the point of the fix), so exercise it directly rather than adding
        // a test-only seam for injecting a corrupted `LineOffsets`.
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, "- alpha\n", &gfm_options());
        let item = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::Item(_)))
            .expect("list item node");
        let li = BlockKind::ListItem {
            ordered: false,
            task: None,
        };
        let id = BlockId::new("li", 7);
        let mut warnings: Vec<String> = Vec::new();

        // Correctly sliced: silent.
        warn_on_empty_list_item_range(
            item,
            &li,
            &id,
            ByteRange { start: 0, end: 7 },
            &mut warnings,
        );
        assert!(
            warnings.is_empty(),
            "a real range must not warn: {warnings:?}"
        );

        // Desynced: empty range under the item's real (non-degenerate) position.
        warn_on_empty_list_item_range(
            item,
            &li,
            &id,
            ByteRange { start: 8, end: 8 },
            &mut warnings,
        );
        assert_eq!(warnings.len(), 1, "expected one warning, got {warnings:?}");
        assert!(
            warnings[0].contains("li-0007"),
            "the warning must name the block: {}",
            warnings[0],
        );

        // Same empty range, non-list-item kind: out of scope by design.
        warn_on_empty_list_item_range(
            item,
            &BlockKind::Html,
            &id,
            ByteRange { start: 8, end: 8 },
            &mut warnings,
        );
        assert_eq!(
            warnings.len(),
            1,
            "only list items are in scope: {warnings:?}",
        );
    }
}
