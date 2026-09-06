//! Per-block html extraction outcome and the translatability predicates
//! derived from it (spec §3.2).
//!
//! Syntax-side because the whole closure is syntax-side: the block IR, the
//! `transync-html` extract pass, and the byte-range payload slice.
//! `transync-core` consumes these from batching, alignment, and the pipeline
//! report so all three agree on which html blocks became units.
//!
//! TRACE: DCR-0017

use crate::id::{BlockId, BlockKind, Spelling};
use crate::intake::markdown::{Block, Document};
use std::collections::HashMap;

/// Per-block extraction outcome for **HTML-spelled** blocks (spec §3.2).
/// Computed once per run by [`html_outcomes`] and threaded to batching,
/// alignment, and the report so the three stay in agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlOutcome {
    /// Extraction produced ≥1 segment — a normal translation unit.
    Unit,
    /// Zero translatable segments — preserved, live-rendered, uncounted.
    PreservedZeroSegment,
    /// The rewriter errored — placeholder presentation, uncounted. The
    /// message is surfaced into `ValidationReport::skipped_source_nodes` so
    /// the degrade is visible rather than silent.
    ExtractionFailed(String),
}

/// Run `transync_html::extract` over every **HTML-spelled** block once
/// and record what it produced (spec §3.2).
///
/// Computing the map once per run is what keeps every html-aware stage in
/// agreement: `unit::build_batches` and `has_translatable_blocks` decide what
/// becomes a unit, `align::build_alignment_map` derives each row's
/// `fallback_status` and whether it counts, and the pipeline turns the
/// non-`Unit` outcomes into `ValidationReport::skipped_source_nodes` rows —
/// so a block's row, its counters, and its presentation cannot disagree.
/// `regen` is deliberately not a consumer — it keys off the validated
/// payloads instead.
pub fn html_outcomes(doc: &Document) -> HashMap<BlockId, HtmlOutcome> {
    let mut out = HashMap::new();
    for block in &doc.blocks {
        if !matches!(block.spelling, Spelling::Html { .. }) {
            continue;
        }
        let payload = block_payload(doc, block);
        let outcome = match transync_html::extract(&payload) {
            Ok(segs) if segs.texts.is_empty() => HtmlOutcome::PreservedZeroSegment,
            Ok(_) => HtmlOutcome::Unit,
            Err(e) => HtmlOutcome::ExtractionFailed(e),
        };
        out.insert(block.block_id.clone(), outcome);
    }
    out
}

/// Whether a block kind participates in translation. Thematic breaks and
/// images carry no translatable text; `Skipped` nodes are preserved
/// verbatim and rendered as inert placeholders, so they are never batched
/// (A3, invariant 7). An HTML-spelled block is kind-level translatable
/// whatever its kind — whether a given html *block* becomes a unit is decided
/// per block by [`is_translatable_block`].
///
/// TRACE: SCN-01..SCN-06
pub(crate) fn is_translatable(kind: &BlockKind) -> bool {
    !matches!(
        kind,
        BlockKind::ThematicBreak | BlockKind::Image | BlockKind::Skipped { .. }
    )
}

/// Per-block translatability (spec §3.2): the extraction outcome decides for
/// an HTML-spelled block, kind-level for a Markdown-spelled one. MUST stay in
/// exact agreement with `unit::build_batches` and `align::build_alignment_map`.
// pub only because the crate boundary forces it — not curated API (DCR-0017 hands this list to OI-0027)
#[doc(hidden)]
pub fn is_translatable_block(block: &Block, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> bool {
    // Spec §7: for an HTML-spelled block the extraction outcome decides,
    // whatever its semantic kind — an HTML document's `<h1>` is a `Heading1`
    // and still translates through the segment engine, and its `<hr>` is a
    // `ThematicBreak` that extracts zero segments and lands
    // `PreservedZeroSegment`. The kind-level exclusions below are therefore
    // the MARKDOWN arm; asking them first would have excluded a
    // ThematicBreak-kinded HTML block before the outcome map ever saw it,
    // which is the same answer by a route that stops being right.
    if matches!(block.spelling, Spelling::Html { .. }) {
        return matches!(html_outcomes.get(&block.block_id), Some(HtmlOutcome::Unit));
    }
    is_translatable(&block.kind)
}

/// Whether the document contains at least one block `unit::build_batches`
/// would turn into a translation unit. Exactly the predicate that decides
/// whether `build_batches` returns an empty vec, so callers can skip
/// per-run preflight work (OI-0026's glossary extraction) on a document
/// that will produce no batches. `html_outcomes` must be the same map the
/// matching `build_batches` call receives.
///
/// TRACE: OI-0026
// pub only because the crate boundary forces it — not curated API (DCR-0017 hands this list to OI-0027)
#[doc(hidden)]
pub fn has_translatable_blocks(
    doc: &Document,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> bool {
    doc.blocks
        .iter()
        .any(|b| is_translatable_block(b, html_outcomes))
}

// pub only because the crate boundary forces it — not curated API (DCR-0017 hands this list to OI-0027)
#[doc(hidden)]
pub fn block_payload(doc: &Document, block: &Block) -> String {
    let mut start = block.source_range.start.min(doc.source_text.len());
    let mut end = block.source_range.end.min(doc.source_text.len()).max(start);
    // `R0001-0029` in the removed `reviews/reviewed/0001.md` (not the live
    // round's 0029, which is about `Retry-After`): defensive snap to UTF-8
    // char boundaries so a
    // surprising source-position offset (e.g. land in the middle of a
    // multi-byte char) cannot panic the slice. We snap start FORWARD
    // and end BACKWARD; if the range collapses, we return an empty
    // payload rather than a panic.
    let bytes = doc.source_text.as_bytes();
    while start < bytes.len() && !doc.source_text.is_char_boundary(start) {
        start += 1;
    }
    end = end.max(start);
    while end > start && !doc.source_text.is_char_boundary(end) {
        end -= 1;
    }
    doc.source_text[start..end].to_string()
}

// D5: a `<title>` is real translatable content — the single highest-value
// string on many pages — so it gets a real unit and a real row. The kind-level
// predicate says so by NOT excluding it; this pins that the omission is the
// decision and not an oversight, beside the two kinds that ARE excluded.
#[cfg(test)]
mod title_translatability_tests {
    use super::*;

    #[test]
    fn a_title_is_kind_level_translatable_and_the_excluded_kinds_still_are_not() {
        assert!(is_translatable(&BlockKind::Title));
        assert!(!is_translatable(&BlockKind::ThematicBreak));
        assert!(!is_translatable(&BlockKind::Image));
    }
}
