//! Link-reference-definition extraction from the inter-block gaps.
//!
//! CommonMark link-reference definitions (`[label]: url "title"`) are
//! document-level state: comrak consumes them during block finalization and
//! leaves NO AST node — a paragraph whose definition prefixes are stripped
//! becomes blank and is detached. A single-block *fragment* parse therefore
//! resolves nothing, so `[text][ref]` renders as literal bracket text and
//! contributes no destination to inline protection.
//!
//! Definitions are line-anchored blocks that always survive verbatim into the
//! regenerated Markdown — they live in the inter-block gaps that
//! `regen::regenerate` copies byte-for-byte. This module recovers
//! them by walking those gaps at parse time and concatenating the
//! pure-definition ones into a pool. Appending the pool to a fragment before
//! reparse (`render`) or before the inline-inventory parse
//! (`validate::inline`) gives comrak-native label matching —
//! normalization, first-definition-wins duplicates, multiline titles — with
//! zero grammar reimplementation and zero HTML output (definitions render
//! nothing). See design D2 §B.
//!
//! Out of scope for appending (definitions cannot change a fragment's block
//! kind or shape, and reference links are inline-level either way):
//! `validate::fragment_reparse`, `validate::per_kind`,
//! `validate::full_reparse` (operates on the full `out.md`, which
//! already carries the definition lines), and `regen` (gap bytes are
//! copied verbatim). See design D2 §B5.
//!
//! Ordering dependency: Part A (a `Skipped` block for every unmodeled
//! top-level node) shrinks the gap inventory to whitespace + definitions, so
//! step 4's noded-gap warning is near-unreachable — it stays as a backstop
//! against comrak sourcepos quirks (design D2 §B2).
//!
//! TRACE: ADR-0012 (amendment §4)

use super::Block;

/// Extract the document's link-reference-definition pool from the gaps
/// between top-level blocks, in source order. Returns the concatenated
/// definition text (empty when the document defines none) plus warnings for
/// any gap that holds *noded* content the walk could not attribute to a
/// block (near-unreachable post-Part-A — see the module docs).
///
/// Appending the SAME pool to source and translated payloads before reparse
/// keeps the inline compare symmetric (design D2 §B4).
pub fn extract(source: &str, blocks: &[Block]) -> (String, Vec<String>) {
    let mut pool = String::new();
    let mut warnings: Vec<String> = Vec::new();

    for (start, end) in gap_ranges(source, blocks) {
        let gap = boundary_slice(source, start, end);
        if gap.trim().is_empty() {
            // Whitespace-only gap (the common `\n\n` separator) — fast path.
            continue;
        }
        if gap_is_pure_definitions(gap) {
            pool.push_str(gap.trim());
            pool.push('\n');
        } else {
            // Post-Part-A every noded top-level construct owns a block, so a
            // gap that parses to nodes indicates a sourcepos quirk: skip it
            // (worst case a reference in it renders literal — today's
            // behavior) and surface it so the misattribution is observable.
            warnings.push(
                "unattributed source text between blocks; reference definitions \
                 inside it will not resolve in the rendered panes"
                    .to_string(),
            );
        }
    }

    (pool, warnings)
}

/// Byte ranges of the inter-block gaps: `[0, first.start)`,
/// `[prev.end, next.start)` for each adjacent pair, and `[last.end, len)`.
///
/// Every block participates — the parser is leaf-block, so the block list is
/// exactly the set `regen::regenerate` splices, and these bytes are exactly
/// the ones it copies verbatim into `out.md`. `last_end` is tracked
/// monotonically so any overlapping or out-of-order range (defensive) never
/// yields a backwards gap.
fn gap_ranges(source: &str, blocks: &[Block]) -> Vec<(usize, usize)> {
    let len = source.len();
    let mut gaps: Vec<(usize, usize)> = Vec::new();
    let mut last_end = 0usize;
    for block in blocks.iter() {
        let bstart = block.source_range.start.min(len);
        let bend = block.source_range.end.min(len).max(bstart);
        if bstart > last_end {
            gaps.push((last_end, bstart));
        }
        last_end = bend.max(last_end);
    }
    if last_end < len {
        gaps.push((last_end, len));
    }
    gaps
}

/// Whether a gap parses to zero top-level AST nodes. Comrak consumes
/// reference definitions with no surviving node, so a gap of pure
/// definitions (plus whitespace) has an empty child set; any prose,
/// heading, or other construct yields at least one child.
fn gap_is_pure_definitions(gap: &str) -> bool {
    let arena = comrak::Arena::new();
    let opts = super::comrak_options();
    let root = comrak::parse_document(&arena, gap, &opts);
    root.children().next().is_none()
}

/// Slice `source[start..end]`, snapping `start` forward and `end` backward to
/// the nearest char boundaries (mirroring `render::block_text`) so a
/// sourcepos quirk can never panic the slice. Returns `""` if the range
/// collapses.
fn boundary_slice(source: &str, start: usize, end: usize) -> &str {
    let mut start = start.min(source.len());
    let mut end = end.min(source.len()).max(start);
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end > start && !source.is_char_boundary(end) {
        end -= 1;
    }
    if end <= start {
        return "";
    }
    &source[start..end]
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;

    // Helper: parse then extract from the real top-level blocks.
    fn pool(src: &str) -> (String, Vec<String>) {
        let doc = parse(src).expect("parses");
        super::extract(&doc.source_text, &doc.blocks)
    }

    #[test]
    fn definition_at_bottom_is_pooled() {
        let (pool, warnings) =
            pool("See [docs][ref] for details.\n\n[ref]: https://example.com/r\n");
        assert!(
            pool.contains("[ref]: https://example.com/r"),
            "trailing-gap definition must be pooled, got {pool:?}",
        );
        assert!(
            warnings.is_empty(),
            "no warnings expected, got {warnings:?}"
        );
    }

    #[test]
    fn definition_at_top_is_pooled() {
        let (pool, warnings) = pool("[ref]: https://example.com/r\n\nSee [docs][ref].\n");
        assert!(
            pool.contains("[ref]: https://example.com/r"),
            "leading-gap definition must be pooled, got {pool:?}",
        );
        assert!(
            warnings.is_empty(),
            "no warnings expected, got {warnings:?}"
        );
    }

    #[test]
    fn definition_between_blocks_is_pooled() {
        let (pool, warnings) =
            pool("# Heading\n\n[ref]: https://example.com/r\n\nSee [docs][ref].\n");
        assert!(
            pool.contains("[ref]: https://example.com/r"),
            "between-blocks definition must be pooled, got {pool:?}",
        );
        assert!(
            warnings.is_empty(),
            "no warnings expected, got {warnings:?}"
        );
    }

    #[test]
    fn multiline_title_definition_is_pooled() {
        // A definition whose title spans two lines is still a single
        // node-less block; the whole gap parses to zero children.
        let (pool, warnings) =
            pool("See [docs][ref].\n\n[ref]: https://example.com/r \"a\ntitle\"\n");
        assert!(
            pool.contains("https://example.com/r"),
            "multiline-title definition must be pooled, got {pool:?}",
        );
        assert!(
            warnings.is_empty(),
            "no warnings expected, got {warnings:?}"
        );
    }

    #[test]
    fn document_without_definitions_yields_empty_pool() {
        let (pool, warnings) = pool("# Heading\n\nJust prose, no references.\n\n- a\n- b\n");
        assert!(pool.is_empty(), "no definitions → empty pool, got {pool:?}");
        // Whitespace-only gaps between blocks must NOT warn.
        assert!(
            warnings.is_empty(),
            "whitespace gaps must not warn, got {warnings:?}"
        );
    }

    #[test]
    fn multiple_definitions_preserve_document_order() {
        let (pool, _) = pool(
            "[a]: https://example.com/a\n\nSee [x][a] and [y][b].\n\n[b]: https://example.com/b\n",
        );
        let ia = pool.find("[a]:").expect("a pooled");
        let ib = pool.find("[b]:").expect("b pooled");
        assert!(
            ia < ib,
            "pool preserves document order (first-wins), got {pool:?}"
        );
    }

    #[test]
    fn noded_gap_is_skipped_with_a_warning() {
        // Defensive step-4 path: hand a source whose trailing gap holds
        // prose the block slice does not cover, so `extract` sees an
        // unattributed noded gap. Achieved by passing only the FIRST block
        // of a two-paragraph document — the second paragraph is then an
        // unattributed trailing gap.
        let doc = parse("alpha paragraph\n\nbeta paragraph\n").expect("parses");
        let (pool, warnings) = super::extract(&doc.source_text, &doc.blocks[..1]);
        assert!(
            pool.is_empty(),
            "noded gap contributes nothing, got {pool:?}"
        );
        assert_eq!(
            warnings.len(),
            1,
            "one warning for the noded gap, got {warnings:?}"
        );
        assert!(
            warnings[0].contains("unattributed source text between blocks"),
            "warning names the unattributed gap, got {warnings:?}",
        );
    }
}
