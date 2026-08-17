//! Byte-range recovery from Comrak nodes.
//!
//! Comrak's `Sourcepos` is 1-indexed line + byte-column. We compute byte
//! offsets into the original source string by precomputing the line-start
//! offsets once per document, then resolving (line, col) → offset.
//!
//! That arithmetic is only meaningful while Comrak's columns and this table
//! count the *same bytes*. Two ways they can disagree have been found, and
//! both are closed at the two ends of the same equality: lone CR is a line
//! ending Comrak honours, so [`LineOffsets::new`] honours it too (OI-0033);
//! NUL is a byte Comrak replaces with a three-byte U+FFFD before it records
//! any position, so `parser::parse` replaces it first (OI-0034). Nothing in
//! this module needs to know about the second — that is the point of doing it
//! at intake — but the clamps below are no longer what holds it together.
//!
//! TRACE: SCN-01

use serde::{Deserialize, Serialize};

/// Half-open `[start, end)` byte range into the original source string.
///
/// Used both in the in-memory IR and on the wire (alignment map JSON).
///
/// TRACE: SCN-01
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByteRange {
    pub start: usize,
    pub end: usize,
}

/// Clamp `range` into `source` and snap both ends to `char` boundaries —
/// `start` forward, `end` backward and never past `start`. The returned pair
/// is always sliceable: `start <= end <= source.len()`, both on boundaries.
///
/// Byte ranges reach the engine from two places that are not equally
/// trustworthy. A [`crate::parser::Document`] built by `parse` carries
/// comrak positions, which are boundary-valid by construction — for those
/// this is the identity. But `Document` and `Block` are `pub` with `pub`
/// fields, so a caller can hand-build a range that splits a multi-byte char,
/// and `&source[start..end]` panics on one. `render::block_text` had grown
/// this loop for exactly that reason; `regen::regenerate` clamped
/// numerically and sliced anyway (R0003-0059).
///
/// `regen::regenerate` is now the only caller (R0003-0060). The renderer
/// went the other way: it has an error channel, so it *refuses* a range that
/// is not a slice (`render::RenderError::UnusableRange`) instead of coercing
/// one into whatever bytes survive — coercion there meant a block rendering
/// as empty, truncated or a character short under its own correct anchor.
/// `regenerate` is infallible by design and has no such channel, so for it
/// the snap remains the safe answer: the alternative is a panic.
pub(crate) fn clamped_char_bounds(source: &str, range: ByteRange) -> (usize, usize) {
    let mut start = range.start.min(source.len());
    let mut end = range.end.min(source.len()).max(start);
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end > start && !source.is_char_boundary(end) {
        end -= 1;
    }
    // Snapping `start` FORWARD can carry it past an `end` that was already
    // in range, which is a collapse to empty, not an inverted slice.
    (start, end.max(start))
}

/// Byte offset of the start of every line in the source. Index `i` is the
/// byte offset of line `i+1` (lines are 1-indexed in `Sourcepos`).
///
/// Lines are split on **Comrak's** rule, not on `\n` alone: comrak's
/// `strings::is_line_end_char` accepts `\n` *or* `\r`, and its reader
/// consumes an optional `\r` followed by an optional `\n`, so `\r\n` is one
/// terminator and a lone `\r` (classic-Mac line ending, CommonMark §2.1) is
/// also one. Nothing else is a line ending — not U+2028/U+2029, NEL, VT, FF,
/// or NUL. Counting `\n` alone would desync every `Sourcepos` → byte mapping
/// for a lone-CR document (OI-0033).
///
/// TRACE: SCN-01
pub struct LineOffsets<'a> {
    pub offsets: Vec<usize>,
    /// The exact source `offsets` was built from, borrowed so every query
    /// resolves against the bytes it describes and cannot be handed a
    /// different string. It is also what makes a line's terminator *length*
    /// recoverable at query time — see [`Self::line_content_end`].
    source: &'a str,
}

impl<'a> LineOffsets<'a> {
    /// Precompute line offsets for `source`.
    ///
    /// TRACE: SCN-01
    pub fn new(source: &'a str) -> Self {
        let bytes = source.as_bytes();
        let mut offsets = Vec::with_capacity(bytes.len() / 40 + 1);
        offsets.push(0);
        for (i, b) in bytes.iter().enumerate() {
            match *b {
                b'\n' => offsets.push(i + 1),
                // A `\r` immediately before a `\n` is that one terminator's
                // first byte, never a boundary of its own.
                b'\r' if bytes.get(i + 1) != Some(&b'\n') => offsets.push(i + 1),
                _ => {}
            }
        }
        Self { offsets, source }
    }

    /// End-of-content cap for a 1-indexed `line`: the offset of the first
    /// byte of that line's terminator, or the source end for the final
    /// unterminated line and for any line past the end of the table.
    ///
    /// This is the single derivation of the cap; `pos_to_byte` and
    /// `byte_range_for` both clamp through it so the two cannot drift.
    /// Unlike a plain `next_line_start - 1` it is terminator-aware, so a
    /// CRLF line caps before its `\r` rather than on it.
    ///
    /// The terminator's length is read back off the source instead of being
    /// materialized into a second per-line table (DCR-0019 shipped that
    /// table; this is its deferred-minor follow-up). The next line starts one
    /// past the terminator's last byte, and by [`Self::new`]'s own rule that
    /// terminator is two bytes exactly when it is `\r\n` — so the two arms
    /// here mirror the two arms there, and no parallel vector has to be kept
    /// in step with `offsets`.
    ///
    /// TRACE: SCN-01
    fn line_content_end(&self, line: usize) -> usize {
        let bytes = self.source.as_bytes();
        // Line 0 does not exist; `pos_to_byte` resolves it to 0, so cap
        // it at 0 too rather than letting it reach into line 1.
        let Some(idx) = line.checked_sub(1) else {
            return 0;
        };
        // No line start after it: `line` is the final, unterminated line (it
        // may be empty) or past the end of the table. Its content runs to the
        // end of the source either way.
        let Some(next_start) = self.offsets.get(idx + 1).copied() else {
            return bytes.len();
        };
        // Offsets are produced by `new`, so `next_start` is in 1..=len; the
        // saturating/min pair only keeps a caller-corrupted `offsets` (the
        // field is `pub`) from indexing out of bounds.
        let last = next_start.saturating_sub(1).min(bytes.len());
        match last.checked_sub(1) {
            Some(prev) if bytes.get(last) == Some(&b'\n') && bytes[prev] == b'\r' => prev,
            _ => last,
        }
    }

    /// Resolve a 1-indexed `(line, col)` pair to a byte offset. `col` is a
    /// 1-indexed byte column within the line.
    ///
    /// `col` is clamped to the byte length of the line itself, so a
    /// surprising `col` value (e.g. one past the line's terminator) cannot
    /// resolve into the terminator or the next line's first byte.
    ///
    /// That clamp is a guard against an out-of-range column, **not** a
    /// correction for a systematically wrong one. It used to be what kept
    /// Comrak's NUL column drift non-fatal (OI-0034) — a role it was never
    /// written for, and one it discharged only because a block-level
    /// `Sourcepos` happens to end where a line's content ends. The drift is
    /// removed at intake now (`parser::parse` normalizes NUL away before
    /// Comrak sees it), so nothing depends on this clamp being lucky.
    ///
    /// The clamp is what makes the result meaningful, but it runs *after*
    /// the add, so the add is saturating (R0001-0045). This is public API in
    /// a public module: `col` is whatever an arbitrary caller passes, not
    /// only comrak's own small sourcepos columns, and `offsets` is a `pub`
    /// field a caller can corrupt. An unchecked `line_start + col` panics on
    /// `usize::MAX` in a checked build and — worse — wraps to a small,
    /// in-range offset in a release build, where the clamp would then happily
    /// pass it through as a *valid-looking* byte position. Saturating first
    /// makes both cases land on the same place an oversized column always
    /// lands: the line's content end.
    ///
    /// TRACE: SCN-01
    pub fn pos_to_byte(&self, line: usize, col: usize) -> usize {
        if line == 0 {
            return 0;
        }
        let line_start = self
            .offsets
            .get(line - 1)
            .copied()
            .unwrap_or(self.source.len());
        let absolute = line_start.saturating_add(col.saturating_sub(1));
        absolute.min(self.line_content_end(line))
    }
}

/// Resolve a Comrak AST node to its byte range in the original source.
///
/// Comrak's `Sourcepos.end` is **inclusive** at the byte-column granularity,
/// so we add one to the end column to produce a half-open `[start, end)`
/// range. Empty ranges (start == end) are coerced to `start..start` to
/// avoid producing invalid slices.
///
/// TRACE: SCN-01
pub fn byte_range_for(
    node: &comrak::nodes::AstNode<'_>,
    line_offsets: &LineOffsets<'_>,
) -> ByteRange {
    let pos = node.data.borrow().sourcepos;
    let start = line_offsets.pos_to_byte(pos.start.line, pos.start.column);
    // Sourcepos.end is inclusive — add one byte so the range is exclusive.
    // Then clamp the +1 to the end of the same source line so we don't
    // pull in the terminator, or the leading byte of the *following* block,
    // when the sourcepos end points at the last byte of the current line.
    //
    // That cap is the only clamp needed: `pos_to_byte` already clamps its
    // own result to the same line's content end, and a content end never
    // exceeds the source length.
    //
    // The `+ 1` is saturating for the same reason `pos_to_byte`'s add is
    // (R0001-0045). It cannot overflow today — its input is already clamped
    // to at most the source length — but that leaves the safety of this line
    // resting on a clamp two calls away rather than on the line itself.
    let end_inclusive = line_offsets.pos_to_byte(pos.end.line, pos.end.column);
    let end = end_inclusive
        .saturating_add(1)
        .min(line_offsets.line_content_end(pos.end.line));
    ByteRange {
        start,
        end: end.max(start),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONE_CR: &str = "# Title\r\rpara one\r\r- a\r- b\r";
    const CRLF: &str = "# Title\r\n\r\npara one\r\n\r\n- a\r\n- b\r\n";
    const LF: &str = "# Title\n\npara one\n\n- a\n- b\n";

    #[test]
    fn lone_cr_lines_are_boundaries() {
        let lo = LineOffsets::new(LONE_CR);
        // comrak sees 6 content lines; the table must have an entry per line
        // (probe: offsets [0, 8, 9, 18, 19, 23, 27]).
        assert_eq!(lo.pos_to_byte(3, 1), 9, "line 3 starts at 'para one'");
        assert_eq!(
            &LONE_CR[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1],
            "para one"
        );
        assert_eq!(
            &LONE_CR[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 3) + 1],
            "- a"
        );
        assert_eq!(
            &LONE_CR[lo.pos_to_byte(6, 1)..lo.pos_to_byte(6, 3) + 1],
            "- b"
        );
    }

    #[test]
    fn crlf_offsets_are_unchanged_by_the_cr_arm() {
        // CRLF is one terminator: the \r arm must NOT fire before a \n.
        let lo = LineOffsets::new(CRLF);
        assert_eq!(
            &CRLF[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1],
            "para one"
        );
        assert_eq!(&CRLF[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 3) + 1], "- a");
    }

    #[test]
    fn lf_offsets_are_unchanged() {
        let lo = LineOffsets::new(LF);
        assert_eq!(
            &LF[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1],
            "para one"
        );
    }

    #[test]
    fn mixed_cr_lf_does_not_swallow_the_next_block() {
        let src = "# Title\n\npara one\r\rsecond para\n\n- a\n- b\n";
        let lo = LineOffsets::new(src);
        // Pre-fix, line 5 resolved into '- a' (the item-swallow). Post-fix it
        // must resolve to 'second para' (probe: CR-aware range 19..30).
        assert_eq!(
            &src[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 11) + 1],
            "second para"
        );
    }

    #[test]
    fn lone_cr_at_eof_is_a_boundary() {
        let lo = LineOffsets::new("just one line\r");
        // The table itself must carry the boundary: without it, `pos_to_byte`
        // only lands on 14 by the out-of-range-line-resolves-to-EOF accident.
        assert_eq!(lo.offsets, vec![0, 14], "probe: CR-aware offsets [0, 14]");
        assert_eq!(lo.pos_to_byte(1, 1), 0);
        assert_eq!(lo.pos_to_byte(2, 1), 14, "line 2 starts at EOF");
    }

    #[test]
    fn crlf_cap_excludes_the_carriage_return() {
        // A too-large column must clamp to the line's *content* end (the
        // offset of the terminator's first byte), not onto the CR of a CRLF.
        let lo = LineOffsets::new(CRLF);
        assert_eq!(
            lo.pos_to_byte(1, 99),
            7,
            "'# Title' ends at byte 7 (the \\r)"
        );
        assert_eq!(lo.line_content_end(1), 7);
        // Lone-CR and LF lines keep today's cap exactly.
        assert_eq!(LineOffsets::new(LONE_CR).line_content_end(1), 7);
        assert_eq!(LineOffsets::new(LF).line_content_end(1), 7);
        // The final, unterminated line caps at the source end.
        let lo = LineOffsets::new("abc");
        assert_eq!(lo.line_content_end(1), 3);
        assert_eq!(lo.pos_to_byte(1, 99), 3);
    }

    /// The cap is derived from the next line start rather than materialized,
    /// so pin the three edges that derivation has to get right on its own:
    /// a terminated final line (the table's last entry is the empty line
    /// *after* it), an empty source (one entry, no next start), and a line
    /// past the end of the table.
    #[test]
    fn cap_is_derived_correctly_at_the_table_edges() {
        let lo = LineOffsets::new("abc\n");
        assert_eq!(lo.offsets, vec![0, 4]);
        assert_eq!(lo.line_content_end(1), 3, "caps before the \\n");
        assert_eq!(lo.line_content_end(2), 4, "the empty trailing line");
        assert_eq!(lo.line_content_end(9), 4, "past the table");

        let lo = LineOffsets::new("");
        assert_eq!(lo.offsets, vec![0]);
        assert_eq!(lo.line_content_end(1), 0);
        assert_eq!(lo.pos_to_byte(1, 1), 0);

        // A CRLF-terminated final line: two terminator bytes, one boundary.
        let lo = LineOffsets::new("abc\r\n");
        assert_eq!(lo.offsets, vec![0, 5]);
        assert_eq!(lo.line_content_end(1), 3, "caps before the \\r");
        assert_eq!(lo.line_content_end(2), 5);

        // A leading terminator: line 1 is empty, and the `\n` at offset 0 has
        // no preceding byte to test for a `\r`.
        let lo = LineOffsets::new("\nabc");
        assert_eq!(lo.offsets, vec![0, 1]);
        assert_eq!(lo.line_content_end(1), 0);
        assert_eq!(lo.line_content_end(2), 4);
    }

    /// R0001-0045. `pos_to_byte` is public API on a public module, so `col`
    /// is whatever an arbitrary caller passes — not only comrak's own small
    /// sourcepos columns. The line-start + column add must therefore survive
    /// `usize::MAX` instead of overflowing before the clamp gets a chance to
    /// run (a debug-build panic; a wrap to a *small* in-range offset in
    /// release, which is worse — it silently slices the wrong bytes).
    #[test]
    fn an_absurd_column_saturates_instead_of_overflowing() {
        let lo = LineOffsets::new(LF);
        // Inside the table: the clamp still wins, exactly as for col = 99.
        assert_eq!(lo.pos_to_byte(1, usize::MAX), 7, "'# Title' content end");
        assert_eq!(lo.pos_to_byte(3, usize::MAX), lo.line_content_end(3));
        // Past the table: `line_start` falls back to the source length, and
        // the add must not push past it.
        assert_eq!(lo.pos_to_byte(99, usize::MAX), LF.len());
        // Both terminator styles, so the CRLF cap is exercised too.
        assert_eq!(LineOffsets::new(CRLF).pos_to_byte(1, usize::MAX), 7);
        assert_eq!(LineOffsets::new(LONE_CR).pos_to_byte(1, usize::MAX), 7);
        // The empty source has nowhere to go but 0.
        assert_eq!(LineOffsets::new("").pos_to_byte(1, usize::MAX), 0);
    }

    /// OI-0034. `parser::parse` hands this table the *normalized* document,
    /// in which every NUL has already become the three-byte U+FFFD Comrak
    /// would have substituted itself. This pins what that buys: a column past
    /// the replacement character resolves to the exact byte, and it does so
    /// strictly *inside* the line, where the content-end clamp is provably
    /// not the thing that produced the answer.
    #[test]
    fn columns_past_a_replacement_character_resolve_exactly() {
        // The line Comrak counts columns in, and the line the table is built
        // over, are the same string — that is the whole fix.
        const NORMALIZED: &str = "alpha\u{FFFD}bravo   \nnext line\n";
        let lo = LineOffsets::new(NORMALIZED);
        assert_eq!(lo.line_content_end(1), 16, "'alpha␣bravo   ' is 16 bytes");

        // Comrak's columns over this line: 'b' of bravo is column 9 (5 + 3),
        // its 'o' is column 13. Both are past the substitution.
        assert_eq!(lo.pos_to_byte(1, 9), 8);
        assert_eq!(
            &NORMALIZED[lo.pos_to_byte(1, 9)..lo.pos_to_byte(1, 13) + 1],
            "bravo",
        );
        assert_eq!(
            &NORMALIZED[lo.pos_to_byte(1, 1)..lo.pos_to_byte(1, 13) + 1],
            "alpha\u{FFFD}bravo",
        );
        assert!(
            lo.pos_to_byte(1, 13) + 1 < lo.line_content_end(1),
            "the resolved end is interior, so the clamp did not produce it",
        );

        // The line after the substitution is unaffected: a NUL is not a line
        // ending (probe-verified in OI-0033), so only within-line columns
        // were ever at stake.
        assert_eq!(lo.offsets, vec![0, 17, 27]);
        assert_eq!(
            &NORMALIZED[lo.pos_to_byte(2, 1)..lo.pos_to_byte(2, 9) + 1],
            "next line"
        );
    }

    /// The same line spelled with the raw NUL Comrak never lets a line table
    /// see any more — kept because it is the arithmetic that made the
    /// normalization necessary, and it is the shape a future regression would
    /// take. Comrak's column 13 means the last byte of `bravo`; over the raw
    /// bytes it lands two further on, in the trailing whitespace, with no
    /// clamp anywhere near it.
    #[test]
    fn raw_nul_columns_drift() {
        const RAW: &str = "alpha\0bravo   \nnext line\n";
        let lo = LineOffsets::new(RAW);
        assert_eq!(lo.line_content_end(1), 14, "'alpha\\0bravo   ' is 14 bytes");
        assert_eq!(
            &RAW[lo.pos_to_byte(1, 1)..lo.pos_to_byte(1, 13) + 1],
            "alpha\0bravo  ",
            "two bytes of drift, well inside the line",
        );
    }

    /// `offsets` is a `pub` field, so a caller can hand `pos_to_byte` a line
    /// start that is nowhere near the source. `line_content_end` already
    /// guards its own indexing against that; the add in front of it must be
    /// just as unimpressed.
    #[test]
    fn a_corrupted_line_table_cannot_overflow_the_add_either() {
        let mut lo = LineOffsets::new(LF);
        lo.offsets[0] = usize::MAX;
        assert_eq!(lo.pos_to_byte(1, 1), 7, "clamped back onto line 1");
        assert_eq!(lo.pos_to_byte(1, usize::MAX), 7);
    }
}
