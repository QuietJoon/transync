//! Full-document rescan: the HTML layer-6 twin of [`full_reparse`](super::full_reparse).
//!
//! `reparse_full` re-reads a regenerated *Markdown* document under comrak and
//! compares its top-level shape to the source IR. This module does the same
//! job for a regenerated *HTML* document — with the scanner, never comrak:
//! comrak over an HTML document produces *some* node sequence and can pass
//! while checking nothing, which is why spec §7/§12 make this gate a hard
//! precondition for any HTML translation run.
//!
//! Four checks, in spec §7's order:
//!
//! 1. **Document-wide ordered tag ledger** — `tag_inventory(regenerated)`
//!    must equal `tag_inventory(source_text)`. The ledger is an ordered
//!    `Vec`, so dropped/duplicated/reordered markup anywhere fires — inside
//!    blocks and in gap bytes alike (pass-through wrappers live in gaps, and
//!    the ledger still tokenizes them).
//! 2. **Fresh segmentation** — re-run the intake segmenter
//!    (`transync_syntax::intake::html::parse`) over the regenerated document
//!    and compare block count and kind-label sequence against
//!    `source_doc.blocks` in order. Written to D9's ruling: `<li>` is the
//!    block, so both sides carry per-item entries and attribution is
//!    per-`<li>`, never per-list (the validation area's original whole-`<ul>`
//!    projection is superseded — spec §7). Attribution mirrors
//!    `full_reparse::attribute_offenders`: each fresh block's start offset is
//!    mapped into the regen `BlockOffsets`; a source block owning ≠ 1 fresh
//!    block is the offender.
//! 3. **Gap byte-identity** — every inter-block gap, the preamble and the
//!    tail, compared byte-for-byte. This is the check with no Markdown twin.
//!    It exists precisely to close the ledger's two MEASURED blind spots:
//!    `<!DOCTYPE>` and comments produce no ledger entry — `scan_tags`
//!    yields only a `Skip` for them, and `tag_inventory` filters `Skip` —
//!    so a dropped doctype or comment is ledger-invisible; but it is a gap
//!    byte, and gaps must be verbatim. In HTML, gaps carry meaning (head, scripts,
//!    structural wrappers); this check re-reads them from the *output*,
//!    which is the whole point of a layer-6 gate.
//! 4. **Boundary sanity** — every block has a target range; ranges are
//!    in-bounds against the actual output length, on char boundaries,
//!    monotone and non-overlapping: cheap re-verification of regen's
//!    bookkeeping. Check 3 defers an unreadable gap here (the fault is the
//!    range, not the bytes), and this check's last step proves the deferral
//!    was total.
//!
//! The checks interlock: a corruption the ledger cannot see (tagless bytes)
//! either changes the block set (check 2) or changes gap bytes (check 3),
//! and a corruption that would make a gap unreadable is a range fault
//! (check 4).
//!
//! # Honestly recorded residuals (spec §7; accepted limitation §13 item 7)
//!
//! - **Two same-kind, identical-tag-skeleton blocks whose texts were swapped
//!   by an engine fault pass all four checks.** This is exact parity with
//!   the shipped `reparse_full`'s blindness to two swapped paragraphs —
//!   parity, not regression. Per-unit layer 3 already ledger-checks each
//!   accepted splice individually, which bounds the fault surface to regen's
//!   assembly ordering; checks 3 + 4 jointly cover any block whose neighbors
//!   differ. Pinned by `swapped_same_skeleton_texts_pass_by_documented_parity`.
//! - **Attribute values are not tokenized** (`scan_tags` skips attribute
//!   internals; RCDATA/raw-text contents are never tokenized). Compensating
//!   controls: splice never edits inside tags by construction,
//!   translated-text escaping is pinned by test in `transync-html`, and the
//!   SCN-16 acceptance criterion ("untouched markup byte-identical outside
//!   text nodes") tests it end-to-end.
//! - **`scan_tags` is not a general HTML parser** — fine, because both sides
//!   of every comparison use the *same* scanner: the check is
//!   self-consistency of one tokenizer opinion, the same rule the
//!   architecture enforces for comrak.
//!
//! On rejection, returns the SAME [`ReparseFailure`] the Markdown gate
//! returns — that shared shape is what keeps the three-stage fallback
//! cascade in `pipeline::finalize` format-blind and untouched.
//!
//! TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)
//! TRACE: DCR-0036

use crate::align::ByteRange;
use crate::id::{BlockId, SourceFormat};
use crate::parser::Document;
use crate::regen::BlockOffsets;
use crate::validate::full_reparse::ReparseFailure;
use transync_html::{TagToken, scan_tags, tag_inventory};

/// Rescan the regenerated HTML and verify it against the source IR.
///
/// `offsets` is the regenerator's per-source-block byte-range map, used the
/// same two ways `reparse_full` uses it: to attribute a divergence back to
/// the source block that owns the divergent output bytes, and (check 4) as
/// the bookkeeping under re-verification. See the module doc for the four
/// checks and the recorded residuals.
///
/// TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)
pub fn full_rescan_html(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    debug_assert_eq!(
        source_doc.format,
        SourceFormat::Html,
        "full_rescan_html is the HTML gate; Markdown documents take reparse_full — \
         the finalize dispatch is the enforcement, this assert is the tripwire",
    );
    check_tag_ledger(source_doc, regenerated, offsets)?;
    check_fresh_segmentation(source_doc, regenerated, offsets)?;
    let deferred = check_gap_bytes(source_doc, regenerated, offsets)?;
    check_boundaries(source_doc, regenerated, offsets, &deferred)
}

/// Check 1 (spec §7): the document-wide ordered tag ledger.
fn check_tag_ledger(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    let source_inv = tag_inventory(&source_doc.source_text);
    let regen_inv = tag_inventory(regenerated);
    if source_inv == regen_inv {
        return Ok(());
    }
    // First divergence over the aligned prefix; if one stream is a strict
    // prefix of the other, the divergence sits at the shorter length.
    let i = source_inv
        .iter()
        .zip(regen_inv.iter())
        .position(|(s, r)| s != r)
        .unwrap_or_else(|| source_inv.len().min(regen_inv.len()));
    Err(ReparseFailure {
        reason: format!(
            "tag inventory diverged at token {i}: source {}, regenerated {} \
             (source has {} tags, regenerated has {})",
            source_inv.get(i).map(String::as_str).unwrap_or("<end>"),
            regen_inv.get(i).map(String::as_str).unwrap_or("<end>"),
            source_inv.len(),
            regen_inv.len(),
        ),
        divergent_source_blocks: ledger_offenders(source_doc, regenerated, offsets, i),
    })
}

/// The byte span of the `i`-th NON-Skip token — index-aligned with
/// `tag_inventory`'s output, which filters `Skip` the same way. Exhaustive
/// match by charter: no `_ =>`, no `matches!`.
fn non_skip_span(tokens: &[TagToken], i: usize) -> Option<(usize, usize)> {
    tokens
        .iter()
        .filter_map(|t| match t {
            TagToken::Open { span, .. } => Some(*span),
            TagToken::Close { span, .. } => Some(*span),
            TagToken::Skip { .. } => None,
        })
        .nth(i)
}

/// Attribution for check 1 (plan-defined; the spec specifies attribution
/// only for check 2): prefer the regenerated side's divergent token — the
/// concrete byte the output got wrong — mapped through the regen offsets;
/// fall back to the source side's token against source ranges when the
/// regenerated stream is the shorter one (a deletion at or past its end).
fn ledger_offenders(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
    i: usize,
) -> Vec<BlockId> {
    if let Some(span) = non_skip_span(&scan_tags(regenerated), i) {
        let ranges: Vec<(BlockId, ByteRange)> = source_doc
            .blocks
            .iter()
            .filter_map(|b| offsets.0.get(&b.block_id).map(|r| (b.block_id.clone(), *r)))
            .collect();
        return owning_or_flanking(&ranges, span.0);
    }
    if let Some(span) = non_skip_span(&scan_tags(&source_doc.source_text), i) {
        let ranges: Vec<(BlockId, ByteRange)> = source_doc
            .blocks
            .iter()
            .map(|b| (b.block_id.clone(), b.source_range))
            .collect();
        return owning_or_flanking(&ranges, span.0);
    }
    Vec::new()
}

/// The block whose (doc-ordered) range contains `offset`, or — when the
/// offset falls in a gap — the flanking block(s). Deterministic; empty only
/// when `ranges` is empty.
fn owning_or_flanking(ranges: &[(BlockId, ByteRange)], offset: usize) -> Vec<BlockId> {
    for (id, r) in ranges {
        if offset >= r.start && offset < r.end {
            return vec![id.clone()];
        }
    }
    let before = ranges.iter().rev().find(|(_, r)| r.end <= offset);
    let after = ranges.iter().find(|(_, r)| r.start > offset);
    match (before, after) {
        (Some((b, _)), Some((a, _))) => vec![b.clone(), a.clone()],
        (Some((b, _)), None) => vec![b.clone()],
        (None, Some((a, _))) => vec![a.clone()],
        (None, None) => Vec::new(),
    }
}

/// Check 2 (spec §7): fresh segmentation, written to D9's per-`<li>` ruling.
/// The regenerated document goes back through THE intake — the same
/// segmenter that produced `source_doc`, named directly per wave 3's
/// no-re-export rule — and the block count and kind-label sequence must
/// match `source_doc.blocks` in order. No `walk::normalize_top_level` here:
/// both sides are already per-item (D9), and `walk` is the Markdown
/// pairing, never called on an HTML document (spec §7).
fn check_fresh_segmentation(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    let fresh = transync_syntax::intake::html::parse(regenerated);
    let common = source_doc.blocks.len().min(fresh.blocks.len());
    for i in 0..common {
        let s = source_doc.blocks[i].kind.wire_str();
        let r = fresh.blocks[i].kind.wire_str();
        if s != r {
            let suspects = attribute_offenders(source_doc, &fresh, offsets);
            return Err(ReparseFailure {
                reason: format!(
                    "fresh segmentation: block {i}: source kind {s} != regenerated kind {r} \
                     (at source block `{}`)",
                    source_doc.blocks[i].block_id.0,
                ),
                divergent_source_blocks: if suspects.is_empty() {
                    vec![source_doc.blocks[i].block_id.clone()]
                } else {
                    suspects
                },
            });
        }
    }
    if fresh.blocks.len() != source_doc.blocks.len() {
        let suspects = attribute_offenders(source_doc, &fresh, offsets);
        let fallback_seed = source_doc
            .blocks
            .get(common)
            .or_else(|| source_doc.blocks.last())
            .map(|b| vec![b.block_id.clone()])
            .unwrap_or_default();
        return Err(ReparseFailure {
            reason: format!(
                "fresh segmentation found {} block(s); source has {}",
                fresh.blocks.len(),
                source_doc.blocks.len(),
            ),
            divergent_source_blocks: if suspects.is_empty() {
                fallback_seed
            } else {
                suspects
            },
        });
    }
    Ok(())
}

/// Check 2's attribution, mirroring `full_reparse::attribute_offenders`:
/// map each fresh block's start offset into the regen `BlockOffsets`; a
/// source block owning ≠ 1 fresh block is the offender. Per `<li>`, never
/// per group — D9 makes every source entry a single block, so the Markdown
/// twin's collapsed-list group fallback has no counterpart here. Walks
/// `source_doc.blocks` in order, so the returned vec is deterministic. A
/// source block with no offsets entry is skipped here (check 4 will name
/// it); comparisons only — this helper must never slice or panic.
fn attribute_offenders(
    source_doc: &Document,
    fresh: &Document,
    offsets: &BlockOffsets,
) -> Vec<BlockId> {
    let mut suspects: Vec<BlockId> = Vec::new();
    for block in &source_doc.blocks {
        let Some(range) = offsets.0.get(&block.block_id) else {
            continue;
        };
        // OI-0042: `fresh` is `intake::html::parse` output, whose ranges
        // follow a monotone cursor (`debug_assert_block_invariants`), so
        // `source_range.start` is non-decreasing and the count in
        // `[range.start, range.end)` is the gap between two partition
        // points — O(log N) instead of a scan per source block.
        // `saturating_sub` keeps this function's no-panic promise: `range`
        // comes from the bookkeeping, which check 4 has not yet vetted for
        // inversion.
        let lo = fresh
            .blocks
            .partition_point(|f| f.source_range.start < range.start);
        let hi = fresh
            .blocks
            .partition_point(|f| f.source_range.start < range.end);
        let owned = hi.saturating_sub(lo);
        if owned != 1 {
            suspects.push(block.block_id.clone());
        }
    }
    suspects
}

/// Check 3 (spec §7): gap byte-identity, preamble and tail included. Gap
/// `g` sits before block `g`; `g == blocks.len()` is the tail. A gap whose
/// endpoints cannot be sliced is DEFERRED to check 4 — the fault is the
/// range, not the bytes — and the deferral is total: every cause of an
/// unreadable gap (missing entry; inverted, out-of-bounds or
/// non-char-boundary range) is one of check 4's predicates, and check 4's
/// last step hard-errors on any leftover, so no gap can fall between the
/// two checks. Comparisons use `str::get`, never indexing: this function
/// must not panic on any input.
fn check_gap_bytes(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<Vec<usize>, ReparseFailure> {
    let n = source_doc.blocks.len();
    let mut deferred: Vec<usize> = Vec::new();
    for g in 0..=n {
        let (s_start, before) = if g == 0 {
            (0, None)
        } else {
            let b = &source_doc.blocks[g - 1];
            (b.source_range.end, Some(b.block_id.clone()))
        };
        let (s_end, after) = if g == n {
            (source_doc.source_text.len(), None)
        } else {
            let b = &source_doc.blocks[g];
            (b.source_range.start, Some(b.block_id.clone()))
        };
        let t_start = if g == 0 {
            Some(0)
        } else {
            offsets
                .0
                .get(&source_doc.blocks[g - 1].block_id)
                .map(|r| r.end)
        };
        let t_end = if g == n {
            Some(regenerated.len())
        } else {
            offsets
                .0
                .get(&source_doc.blocks[g].block_id)
                .map(|r| r.start)
        };
        // Source ranges from the intake are ordered by its debug-asserted
        // invariants; a hand-built document could invert them, so the
        // source side is `.get()`-guarded too and an unreadable source gap
        // defers exactly like an unreadable target gap (check 4's leftover
        // arm then reports it — the one deferral its block-range predicates
        // cannot independently re-derive).
        let src_gap = if s_start <= s_end {
            source_doc.source_text.get(s_start..s_end)
        } else {
            None
        };
        let tgt_gap = match (t_start, t_end) {
            (Some(a), Some(b)) if a <= b => regenerated.get(a..b),
            _ => None,
        };
        let (Some(src_gap), Some(tgt_gap)) = (src_gap, tgt_gap) else {
            deferred.push(g);
            continue;
        };
        if src_gap != tgt_gap {
            let named = |x: &Option<BlockId>| -> String {
                x.as_ref()
                    .map(|id| format!("`{}`", id.0))
                    .unwrap_or_else(|| {
                        if g == 0 {
                            "start of document".to_string()
                        } else {
                            "end of document".to_string()
                        }
                    })
            };
            return Err(ReparseFailure {
                reason: format!(
                    "gap {g} (between {} and {}) differs: source {} byte(s), regenerated {} byte(s)",
                    named(&before),
                    named(&after),
                    src_gap.len(),
                    tgt_gap.len(),
                ),
                divergent_source_blocks: before.into_iter().chain(after).collect(),
            });
        }
    }
    Ok(deferred)
}

/// Check 4 (spec §7): boundary sanity over the regen bookkeeping — every
/// block has a target range; every range is in-bounds against the ACTUAL
/// output length, on char boundaries, monotone and non-overlapping in
/// document order. Runs last, exactly as `reparse_full`'s third scan does:
/// the earlier checks give its indices meaning. The final leftover arm
/// makes deviation 2's deferral contract checkable in code.
fn check_boundaries(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
    deferred: &[usize],
) -> Result<(), ReparseFailure> {
    let mut prev_end = 0usize;
    let mut prev_id: Option<BlockId> = None;
    for block in &source_doc.blocks {
        let Some(range) = offsets.0.get(&block.block_id) else {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` has no target range in the regen bookkeeping",
                    block.block_id.0,
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        };
        if range.start > range.end || range.end > regenerated.len() {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range {}..{} is not in-bounds for output length {}",
                    block.block_id.0,
                    range.start,
                    range.end,
                    regenerated.len(),
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        }
        if !regenerated.is_char_boundary(range.start) || !regenerated.is_char_boundary(range.end) {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range {}..{} splits a char",
                    block.block_id.0, range.start, range.end,
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        }
        if range.start < prev_end {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range starts at {} before block {} ends at {} \
                     (overlap or disorder)",
                    block.block_id.0,
                    range.start,
                    prev_id
                        .as_ref()
                        .map(|id| format!("`{}`", id.0))
                        .unwrap_or_else(|| "<none>".to_string()),
                    prev_end,
                ),
                divergent_source_blocks: prev_id
                    .iter()
                    .cloned()
                    .chain(std::iter::once(block.block_id.clone()))
                    .collect(),
            });
        }
        prev_end = range.end;
        prev_id = Some(block.block_id.clone());
    }
    if let Some(g) = deferred.first() {
        // Reachable only if the totality argument above is wrong — e.g. a
        // hand-built SOURCE range made a source-side gap unreadable. A hard
        // error, not a debug_assert: reaching it means the bookkeeping
        // model itself is broken, and the cascade still needs an Err.
        return Err(ReparseFailure {
            reason: format!(
                "boundary: gap {g} was unreadable although every block range passed \
                 the boundary predicates",
            ),
            divergent_source_blocks: Vec::new(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// The one segmenter, both sides (wave 3's seam; see the module doc for
    /// why core names it directly rather than through a re-export).
    fn parse_html(src: &str) -> Document {
        transync_syntax::intake::html::parse(src)
    }

    /// Identity regen: byte-identical output + one target range per block
    /// (wave 3's theorem, re-asserted here because every corruption helper
    /// below starts from it).
    fn identity(doc: &Document) -> (String, BlockOffsets) {
        let (out, offsets) = crate::regen::regenerate(doc, &HashMap::new());
        assert_eq!(out, doc.source_text, "wave 3 identity theorem");
        assert_eq!(offsets.0.len(), doc.blocks.len());
        (out, offsets)
    }

    /// Identity output with `cut` deleted, offsets re-derived: a range fully
    /// before the cut is unchanged; fully after shifts left by the cut's
    /// length; a range containing the cut shrinks; a range inside the cut
    /// collapses to an empty range at the cut point — the "dropped block"
    /// shape `full_reparse`'s dropped-html test hand-built.
    fn identity_minus(doc: &Document, cut: std::ops::Range<usize>) -> (String, BlockOffsets) {
        let (out, offsets) = identity(doc);
        let mut cropped = String::new();
        cropped.push_str(&out[..cut.start]);
        cropped.push_str(&out[cut.end..]);
        let len = cut.end - cut.start;
        let map = |p: usize| -> usize {
            if p <= cut.start {
                p
            } else if p >= cut.end {
                p - len
            } else {
                cut.start
            }
        };
        let mut shifted = BlockOffsets::default();
        for (id, r) in offsets.0 {
            shifted.0.insert(
                id,
                ByteRange {
                    start: map(r.start),
                    end: map(r.end),
                },
            );
        }
        (cropped, shifted)
    }

    /// Identity output with `insert` spliced in at `at` (a gap or in-block
    /// position), offsets re-derived by shifting every boundary at or after
    /// `at` right by the insertion's length.
    fn identity_plus(doc: &Document, at: usize, insert: &str) -> (String, BlockOffsets) {
        let (out, offsets) = identity(doc);
        let mut grown = String::new();
        grown.push_str(&out[..at]);
        grown.push_str(insert);
        grown.push_str(&out[at..]);
        let map = |p: usize| -> usize { if p < at { p } else { p + insert.len() } };
        let mut shifted = BlockOffsets::default();
        for (id, r) in offsets.0 {
            shifted.0.insert(
                id,
                ByteRange {
                    start: map(r.start),
                    end: map(r.end),
                },
            );
        }
        (grown, shifted)
    }

    /// Identity output with two non-overlapping regions (`a` before `b`)
    /// swapped, offsets remapped positionally: the block whose source range
    /// IS `a` follows its bytes to `b`'s old position and vice versa; blocks
    /// between the two shift by the length difference; blocks outside are
    /// unchanged (the total length is). Panics if any block range straddles
    /// a swapped region — the fixtures below are chosen so none does.
    fn swap_regions(doc: &Document, a: ByteRange, b: ByteRange) -> (String, BlockOffsets) {
        assert!(a.end <= b.start, "a must precede b");
        let src = &doc.source_text;
        let mut out = String::new();
        out.push_str(&src[..a.start]);
        let b_new = ByteRange {
            start: out.len(),
            end: out.len() + (b.end - b.start),
        };
        out.push_str(&src[b.start..b.end]);
        out.push_str(&src[a.end..b.start]);
        let a_new = ByteRange {
            start: out.len(),
            end: out.len() + (a.end - a.start),
        };
        out.push_str(&src[a.start..a.end]);
        out.push_str(&src[b.end..]);
        assert_eq!(out.len(), src.len());
        let delta = (b.end - b.start) as isize - (a.end - a.start) as isize;
        let mut offsets = BlockOffsets::default();
        for blk in &doc.blocks {
            let r = blk.source_range;
            let mapped = if r == a {
                a_new
            } else if r == b {
                b_new
            } else if r.end <= a.start || r.start >= b.end {
                r
            } else if r.start >= a.end && r.end <= b.start {
                ByteRange {
                    start: (r.start as isize + delta) as usize,
                    end: (r.end as isize + delta) as usize,
                }
            } else {
                panic!("swap_regions: block range straddles a swapped region");
            };
            offsets.0.insert(blk.block_id.clone(), mapped);
        }
        (out, offsets)
    }

    /// Small inline fixture. Blocks, in order (derived by hand from spec
    /// §4's classification table): `<p>alpha</p>` → Paragraph; the naked
    /// `intro text` run → Paragraph (rule T); two `<li>` → ListItem ×2 (D9).
    /// The `<div>`/`<ul>` markup and the comment are gap bytes; the comment
    /// is a `Skip` span and produces NO ledger token — which is what the
    /// dropped-comment test in Task 3 leans on.
    const SRC_SMALL: &str = "<div>\n<p>alpha</p>\nintro text\n<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n<!-- note -->\n</div>\n";

    /// The wave-3 fixture, shared by relative include exactly as wave 3's
    /// own syntax-side tests share it — one file, two consumers, no drift.
    const SCN_16: &str = include_str!("../../../transync/tests/fixtures/scn-16-html-document.html");

    fn kinds(doc: &Document) -> Vec<&'static str> {
        doc.blocks.iter().map(|b| b.kind.wire_str()).collect()
    }

    #[test]
    fn identity_regen_accepts_over_the_scn_16_fixture() {
        let doc = parse_html(SCN_16);
        assert_eq!(doc.format, SourceFormat::Html);
        assert_eq!(doc.blocks.len(), 14, "the wave-3 designed block set");
        let (out, offsets) = identity(&doc);
        full_rescan_html(&doc, &out, &offsets)
            .expect("byte-identical regen must pass all four checks");
    }

    #[test]
    fn a_dropped_element_block_is_rejected_by_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        assert_eq!(
            kinds(&doc),
            vec!["paragraph", "paragraph", "list-item", "list-item"],
            "fixture sanity: the designed block set",
        );
        // Cut the whole `<p>alpha</p>` element — its `p`/`/p` tokens leave
        // the ledger, so check 1 fires. (Its block also dissolves, so check
        // 2 WOULD fire too — spec §11's "trips checks 1+2"; check 1 is
        // first in order, so it reports. The check-2-only route is the
        // dissolved-run test below.)
        let p = doc.blocks[0].source_range;
        let (out, offsets) = identity_minus(&doc, p.start..p.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a regen that dropped an element block must be rejected");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        assert!(
            !err.divergent_source_blocks.is_empty(),
            "check 1 must seed the cascade"
        );
    }

    #[test]
    fn a_reordered_block_pair_is_rejected_by_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        // Swap the `<p>` element's bytes with the first `<li>` element's.
        // The ledger is ORDERED, so `p,/p,…,li,/li` vs `li,/li,…,p,/p`
        // diverges at the first swapped token (spec §11's reordered red:
        // both the ledger and the label sequence fire; the ledger is first).
        let (out, offsets) =
            swap_regions(&doc, doc.blocks[0].source_range, doc.blocks[2].source_range);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a regen that reordered two blocks must be rejected");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        let named: Vec<&str> = err
            .divergent_source_blocks
            .iter()
            .map(|id| id.0.as_str())
            .collect();
        assert!(
            named.contains(&doc.blocks[0].block_id.0.as_str())
                || named.contains(&doc.blocks[2].block_id.0.as_str()),
            "attribution must name a swapped block; got {named:?}",
        );
    }

    #[test]
    fn tag_inventory_drift_inside_a_block_is_rejected() {
        let doc = parse_html(SRC_SMALL);
        // Insert a phantom `<em>` pair inside the first `<li>`'s text — the
        // kind of markup a faulty splice could invent. Ledger-visible,
        // segmentation-invisible (an `<em>` is PHRASING and mints no block).
        let li = doc.blocks[2].source_range;
        let li_text = doc.source_text[li.start..li.end].to_string();
        assert!(li_text.contains("one"), "fixture sanity");
        let at = li.start + doc.source_text[li.start..li.end].find("one").unwrap();
        let (out, offsets) = identity_plus(&doc, at, "<em>");
        let (out, offsets) = {
            // close it after the word so the fragment stays balanced — the
            // drift must be caught by INVENTORY inequality, not by luck.
            let close_at = at + "<em>".len() + "one".len();
            let mut o2 = BlockOffsets::default();
            let map = |p: usize| -> usize { if p < close_at { p } else { p + "</em>".len() } };
            for (id, r) in offsets.0 {
                o2.0.insert(
                    id,
                    ByteRange {
                        start: map(r.start),
                        end: map(r.end),
                    },
                );
            }
            let mut s = String::new();
            s.push_str(&out[..close_at]);
            s.push_str("</em>");
            s.push_str(&out[close_at..]);
            (s, o2)
        };
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("invented markup must be rejected by the ledger");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[2].block_id.clone()],
            "the token span sits inside li-…'s target range, so attribution \
             must name exactly that block",
        );
    }

    #[test]
    fn a_dissolved_anonymous_run_is_rejected_by_fresh_segmentation_not_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        // Cut ONLY the rule-T run's bytes (`intro text`) — no tag leaves
        // the ledger, but the block dissolves. With later blocks following,
        // the divergence surfaces at the LABEL arm: index 1 is `paragraph`
        // on the source side and `list-item` on the fresh side (everything
        // shifted up). This is the check-2-only red: with check 2 removed
        // the twin would accept a document that silently lost a paragraph.
        let run = doc.blocks[1].source_range;
        assert_eq!(
            &doc.source_text[run.start..run.end],
            "intro text",
            "fixture sanity"
        );
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dissolved anonymous run must be rejected");
        assert!(
            !err.reason.contains("tag inventory"),
            "the ledger cannot see tagless bytes; got: {err}",
        );
        assert!(err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(err.reason.contains("paragraph"), "got: {err}");
        assert!(err.reason.contains("list-item"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[1].block_id.clone()],
            "the dissolved run's empty target range owns 0 fresh blocks — \
             per-block attribution (D9), never a group",
        );
    }

    #[test]
    fn a_dissolved_trailing_run_hits_the_count_arm() {
        // The count arm's own red: when the dissolved run is the LAST
        // block, the label prefix stays clean and only the count diverges —
        // "fresh segmentation found 1 block(s); source has 2".
        let doc = parse_html("<p>x</p>\n<div>tail note</div>\n");
        assert_eq!(
            kinds(&doc),
            vec!["paragraph", "paragraph"],
            "fixture sanity"
        );
        let run = doc.blocks[1].source_range;
        assert_eq!(
            &doc.source_text[run.start..run.end],
            "tail note",
            "fixture sanity"
        );
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dissolved trailing run must be rejected");
        assert!(err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(
            err.reason.contains("found 1 block(s); source has 2"),
            "got: {err}"
        );
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[1].block_id.clone()],
            "got: {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn attribution_is_per_li_not_per_list() {
        // D9's ruling, pinned structurally: both sides of check 2 carry one
        // entry PER `<li>`, so the label sequence over SRC_SMALL ends
        // `…, "list-item", "list-item"` — never a collapsed `"list"` entry
        // like `walk::normalize_top_level` produces on the Markdown side.
        // The twin never imports `walk` (gated by grep in Step 6), so the
        // superseded whole-`<ul>` projection has no code to hide in.
        let doc = parse_html(SRC_SMALL);
        let (out, _offsets) = identity(&doc);
        let fresh = parse_html(&out);
        assert_eq!(kinds(&fresh), kinds(&doc));
        assert_eq!(kinds(&fresh)[2..4], ["list-item", "list-item"]);
    }

    #[test]
    fn swapped_same_skeleton_texts_pass_by_documented_parity() {
        // Residual 1 (module doc; spec §13 item 7): two same-kind,
        // identical-tag-skeleton blocks whose TEXTS were swapped pass all
        // four checks — exact parity with `reparse_full`'s blindness to two
        // swapped paragraphs. Characterized so the residual is a tested
        // fact, not a forgotten one: if a future check closes it, this pin
        // flips and the closure DCR retires it deliberately.
        let doc = parse_html("<p>alpha</p>\n<p>beta</p>\n");
        let (out, offsets) =
            swap_regions(&doc, doc.blocks[0].source_range, doc.blocks[1].source_range);
        // Positional re-map: block 0's range must describe the FIRST region
        // again (the engine-fault shape: right bytes, wrong owner).
        let mut positional = BlockOffsets::default();
        positional.0.insert(
            doc.blocks[0].block_id.clone(),
            *offsets.0.get(&doc.blocks[1].block_id).unwrap(),
        );
        positional.0.insert(
            doc.blocks[1].block_id.clone(),
            *offsets.0.get(&doc.blocks[0].block_id).unwrap(),
        );
        full_rescan_html(&doc, &out, &positional)
            .expect("parity with reparse_full: the documented blind spot accepts");
    }
    #[test]
    fn a_dropped_doctype_is_ledger_invisible_but_not_gap_invisible() {
        // THE decoration-proof (spec §7 check 3, §12 wave 4): the wave-0
        // MEASURED fact is that `<!DOCTYPE html>` produces no inventory
        // entry — only a `Skip`, which `tag_inventory` filters — so
        // check 1 cannot see it leave; it minted no block (wave
        // 3's doctype-only pin), so check 2 cannot either; the offsets
        // below are coherent, so check 4 passes. ONLY check 3 stands
        // between this corruption and a silent doctype-less output. This
        // test MUST fail against the two-check twin — that red is recorded
        // evidence that check 3 is a gate, not decoration.
        let doc = parse_html(SCN_16);
        let cut_len = "<!DOCTYPE html>\n".len();
        assert!(
            doc.source_text.starts_with("<!DOCTYPE html>\n"),
            "fixture sanity"
        );
        let (out, offsets) = identity_minus(&doc, 0..cut_len);
        let err =
            full_rescan_html(&doc, &out, &offsets).expect_err("a dropped doctype must be rejected");
        assert!(!err.reason.contains("tag inventory"), "got: {err}");
        assert!(!err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(err.reason.contains("gap"), "got: {err}");
        assert!(err.reason.contains("start of document"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[0].block_id.clone()],
            "the preamble's flanking block is the first block (the title)",
        );
    }

    #[test]
    fn a_dropped_comment_is_ledger_invisible_but_not_gap_invisible() {
        // The second MEASURED blind spot: comments are Skip spans and leave
        // no ledger token. SRC_SMALL's `<!-- note -->` sits in the tail gap
        // (after the last block), so this also covers the tail leg of
        // check 3's "including preamble and tail".
        let doc = parse_html(SRC_SMALL);
        let at = doc
            .source_text
            .find("<!-- note -->")
            .expect("fixture sanity");
        let (out, offsets) = identity_minus(&doc, at..at + "<!-- note -->".len());
        let err =
            full_rescan_html(&doc, &out, &offsets).expect_err("a dropped comment must be rejected");
        assert!(err.reason.contains("gap"), "got: {err}");
        assert!(err.reason.contains("end of document"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks.last().unwrap().block_id.clone()],
            "the tail's flanking block is the last block",
        );
    }

    #[test]
    fn text_moved_between_a_block_and_a_gap_is_still_caught() {
        // The interlock the module doc claims: no tagless corruption
        // escapes, because it either moves the block set (check 2) or moves
        // gap bytes (check 3). Dissolve the rule-T run AND inject
        // whitespace into a different gap (whitespace mints no block, so
        // the injection alone would be check-3-only). Ledger: clean — no
        // tag moved. The dissolved run fires check 2 first; had the run
        // survived, the dirtied gap would have fired check 3. Either path
        // implicates the run's neighborhood.
        let doc = parse_html(SRC_SMALL);
        let run = doc.blocks[1].source_range;
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        // Splice noise into the gap AFTER the (now empty) run — between the
        // run's collapse point and `<ul>`: three spaces mint no block.
        let (out, offsets) = {
            let at = out.find("<ul>").expect("fixture sanity");
            let mut o2 = BlockOffsets::default();
            let map = |p: usize| -> usize { if p < at { p } else { p + 3 } };
            for (id, r) in offsets.0 {
                o2.0.insert(
                    id,
                    ByteRange {
                        start: map(r.start),
                        end: map(r.end),
                    },
                );
            }
            let mut s = String::new();
            s.push_str(&out[..at]);
            s.push_str("   ");
            s.push_str(&out[at..]);
            (s, o2)
        };
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("whitespace injected into a gap must be rejected");
        // Which check fires first depends on the dissolved run: check 2
        // sees 3 blocks vs 4 BEFORE check 3 reads the gaps — both paths are
        // honest; assert the run is implicated either way.
        assert!(
            err.divergent_source_blocks
                .contains(&doc.blocks[1].block_id),
            "got: {err} / {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn overlapping_target_ranges_are_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        // Pull block 2's start back inside block 1's range: the gap between
        // them inverts (unreadable → deferred by check 3), and check 4's
        // monotonicity predicate names both blocks.
        let b1_end = offsets.0.get(&doc.blocks[1].block_id).unwrap().end;
        let r2 = offsets.0.get_mut(&doc.blocks[2].block_id).unwrap();
        r2.start = b1_end - 1;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("overlapping ranges must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(
            err.divergent_source_blocks
                .contains(&doc.blocks[2].block_id),
            "got: {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn a_missing_offsets_entry_is_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        offsets.0.remove(&doc.blocks[2].block_id);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a block with no target range must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(err.reason.contains("no target range"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[2].block_id.clone()]
        );
    }

    #[test]
    fn an_out_of_bounds_range_is_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        let last = doc.blocks.last().unwrap().block_id.clone();
        offsets.0.get_mut(&last).unwrap().end = out.len() + 1;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("an out-of-bounds range must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert_eq!(err.divergent_source_blocks, vec![last]);
    }

    #[test]
    fn a_range_endpoint_inside_a_char_is_a_boundary_fault() {
        // The char-boundary predicate's OWN red — and the module's one
        // non-ASCII fixture, which is exactly why it exists: every other
        // fixture here is ASCII, where `is_char_boundary` can never say no.
        // Deleting the predicate would turn NO other test red: the
        // unreadable gap would still be deferred by check 3 and swept up by
        // check 4's leftover arm — but with the leftover's reason and EMPTY
        // attribution, leaving the cascade nothing to downgrade. This pin
        // holds the honest diagnosis. (The `é` is written `\u{e9}` so no
        // editor or tool can silently normalize the fixture into a
        // decomposed `e`-plus-combining form whose first byte IS a
        // boundary.)
        let doc = parse_html("<p>caf\u{e9}</p>\n<p>tail</p>\n");
        assert_eq!(
            kinds(&doc),
            vec!["paragraph", "paragraph"],
            "fixture sanity"
        );
        let (out, mut offsets) = identity(&doc);
        // Park block 0's end one byte into the two-byte `é`: the map stays
        // present, in-bounds and monotone, so only this predicate can name
        // the fault. Check 3 defers the now-unreadable gap after block 0
        // (`regenerated.get` returns `None` at a non-boundary endpoint),
        // and check 4 then reports the range that caused the deferral —
        // deviation 2's contract, exercised end-to-end for real.
        let split = out.find('\u{e9}').expect("fixture sanity") + 1;
        assert!(!out.is_char_boundary(split), "fixture sanity: inside the é");
        offsets.0.get_mut(&doc.blocks[0].block_id).unwrap().end = split;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a range endpoint inside a char must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(err.reason.contains("splits a char"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[0].block_id.clone()]
        );
    }

    #[test]
    fn garbage_bookkeeping_never_panics() {
        // The twin's no-panic rule: every offsets-driven slice is
        // `.get()`-guarded, every lookup is `Option`-handled. Whatever the
        // fault, the answer is `Err`, never an unwind — the cascade needs a
        // diagnosis, not a crash (invariant 6: never silently corrupt, and
        // never die).
        let doc = parse_html(SRC_SMALL);
        let (out, _) = identity(&doc);
        let mut garbage = BlockOffsets::default();
        for (i, b) in doc.blocks.iter().enumerate() {
            garbage.0.insert(
                b.block_id.clone(),
                ByteRange {
                    start: usize::MAX - i,
                    end: 7,
                },
            );
        }
        // These two calls die at check 1 — truncation and emptiness change
        // the tag inventory — so they prove the twin never panics BEFORE
        // rejecting, and nothing more: on these paths the garbage ranges
        // are dead weight that no check ever slices by.
        let truncated = &out[..out.len() / 2];
        assert!(full_rescan_html(&doc, truncated, &garbage).is_err());
        assert!(full_rescan_html(&doc, "", &garbage).is_err());
        // THE call that does the work: the UN-truncated identity output
        // sails through checks 1 and 2 (byte-identical, so the inventory
        // and the fresh segmentation both match), which forces the
        // usize::MAX ranges into check 3 — the only code in the twin that
        // slices by the bookkeeping's ranges. Every interior gap's
        // `regenerated.get(..)` comes back `None` and defers (an unguarded
        // index there would unwind and turn this red), and the walk ends in
        // the tail gap's honest byte-inequality `Err`. Check 4 never slices
        // at all — its predicates are arithmetic plus `is_char_boundary`,
        // which is total — so check 3's guards are the whole no-panic
        // surface this pin exists to hold.
        assert!(full_rescan_html(&doc, &out, &garbage).is_err());
    }

    /// OI-0042: check 2's attribution over an INVERTED bookkeeping range,
    /// driven directly. `garbage_bookkeeping_never_panics` cannot cover
    /// this — its working call sails THROUGH check 2, and this helper runs
    /// only when check 2 fails — so the "must never slice or panic"
    /// promise above needs its own red. The linearized form subtracts two
    /// `partition_point`s over `fresh.blocks`, and an inverted query puts
    /// the upper one BELOW the lower: the subtraction must saturate to the
    /// conservative "owns nothing, so it is a suspect" answer rather than
    /// wrap (release) or panic (overflow checks). Check 4 is the predicate
    /// that names an inverted range, and it runs after this.
    #[test]
    fn attribute_offenders_survives_an_inverted_bookkeeping_range() {
        let doc = parse_html("<p>alpha</p>\n<p>bravo</p>\n");
        assert_eq!(
            kinds(&doc),
            vec!["paragraph", "paragraph"],
            "fixture sanity"
        );
        let (out, mut offsets) = identity(&doc);
        let fresh = parse_html(&out);
        assert!(
            attribute_offenders(&doc, &fresh, &offsets).is_empty(),
            "identity regen: every source block owns exactly one fresh block"
        );

        let r = *offsets
            .0
            .get(&doc.blocks[0].block_id)
            .expect("block 0 has a range");
        assert!(
            r.start < r.end,
            "fixture sanity: a non-empty range to invert"
        );
        offsets.0.insert(
            doc.blocks[0].block_id.clone(),
            ByteRange {
                start: r.end,
                end: r.start,
            },
        );
        assert_eq!(
            attribute_offenders(&doc, &fresh, &offsets),
            vec![doc.blocks[0].block_id.clone()],
            "an inverted range owns nothing, which makes its block a suspect"
        );
    }
}
