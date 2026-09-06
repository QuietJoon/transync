//! THE block-nesting ceiling, and the pre-scan that enforces it.
//!
//! Source Markdown is untrusted by construction (CLAUDE.md invariant 7), and
//! a Markdown block container costs one byte per level to write: `>>>>…` is
//! twenty thousand nested blockquotes in twenty kilobytes. Anything that
//! walks such a tree *recursively* runs out of stack, and a Rust stack
//! overflow is an **abort**, not a panic — a host embedding this library
//! cannot catch it and dies with it. So the depth has to be refused, and it
//! has to be refused before a tree that deep exists.
//!
//! [`check_nesting_depth`] is that refusal. [`crate::intake::markdown::parse`] runs it
//! on the caller's bytes before Comrak — or `normalize_source`, or anything
//! else — touches them, and returns
//! [`ParseError::TooDeeplyNested`](crate::error::ParseError::TooDeeplyNested)
//! instead of a `Document`.
//!
//! **Why a pre-scan and not a Comrak option.** Comrak 0.27 (the pinned
//! version) exposes no nesting or recursion limit: `Options` carries
//! `extension` / `parse` / `render` groups and none of the three has a depth
//! field. Its one internal ceiling — `MAX_LIST_DEPTH = 100` in
//! `open_new_blocks` — is neither configurable nor sufficient: it caps how
//! many list containers a *single line* may open, so a list nested one level
//! per line walks straight past it, and it does not constrain blockquotes at
//! all. A guard on this side is the only option, and it has to run first.
//!
//! **What the bound is measured against.** Measured on this workspace at the
//! 2 MiB stack a test thread gets: Comrak 0.27's own block parse is
//! iterative and survives 400,000 nested blockquotes and 5,000 nested list
//! levels without touching the ceiling. What aborts is a *recursive walker*
//! reading the tree afterwards — `transync-core`'s list-topology
//! fingerprinter died at 2,193 list levels on a 2 MiB stack and at 1,100 on
//! the 1 MiB a wasm module gets (it has since been made iterative). So
//! [`MAX_BLOCK_NESTING_DEPTH`] is set at 128: an order of magnitude above any
//! document written for a reader, and an order of magnitude *below* the
//! shallowest measured abort, on the smallest stack in the system.
//!
//! # The other nesting, and why it has no ceiling
//!
//! Block containers are one of the two ways Markdown nests. The other is
//! *inline* — nested emphasis, nested brackets, an image whose alt text is
//! itself nested — and this module bounds only the first. That is a measured
//! decision rather than an omission, and the pin named at the end of this
//! section is what stops it from quietly becoming an omission later
//! (ticket `f69e83`).
//!
//! **What was measured.** On this workspace, on the 256 KiB stack that pin
//! runs its walks on — a quarter of the 1 MiB a wasm module gets, and the
//! smallest stack anything here has been measured against — both
//! `transync-syntax` entry points survive an inline AST **2,097,153** levels
//! deep: [`crate::intake::markdown::parse`], and `render::render_source` driven through
//! it. Nothing on the path recurses. Comrak 0.27 keeps its inline delimiters
//! on a heap stack, both of its formatters (`html.rs`, `cm.rs`) walk an
//! explicit `Vec`, its `descendants()` iterator is edge-driven, and the four
//! genuinely recursive walks it does own — `collect_text` and the three
//! footnote passes — are unreachable under `markdown::comrak_options` (no
//! `header_ids`, no heading adapter, no footnote extension). So unlike the
//! block ceiling, inline nesting has no *shallowest measured abort* to sit an
//! order of magnitude below: it has no measured abort at all.
//!
//! **Why there is no bound to set.** [`MAX_BLOCK_NESTING_DEPTH`] is
//! enforceable because a cheap byte-level score is a genuine upper bound on
//! block depth. For inline nesting, no such score exists:
//!
//! - Scoring *runs* of `*` / `_` / `[` is not a bound at all. `*a *a *a …
//!   a* a* a*` reaches one emphasis level per repetition with no run longer
//!   than a single character, and `![![![ … ](u)](u)](u)` does the same with
//!   `!` and `[` alternating; both were measured at depth `n + 1` with a
//!   longest run of 1. A run-based ceiling would therefore miss the two
//!   shapes that nest hardest while refusing a `**********` rule — including
//!   one inside a fenced code block, which the pre-scan's deliberate
//!   blindness to fence interiors guarantees it would see and which cannot
//!   nest anything, because a fence's content is never inline-parsed.
//! - Scoring *totals* is a real bound — depth is at most half the delimiter
//!   count — but a useless one. Measured 2026-08-07 over the 123 Markdown
//!   files tracked in this repository (`.ko.md` excluded): the deepest inline
//!   AST any of them parses to is **3**, while the densest carries **3,860**
//!   delimiter characters and the busiest single line carries **183**. The
//!   proxy overstates the thing by three orders of magnitude before a code
//!   fence full of `snake_case` or JSON is even considered. A threshold low
//!   enough to constrain depth refuses ordinary documents; one high enough to
//!   stay quiet sits far above any depth that has aborted anything.
//!
//! An exact ceiling taken *after* the parse was weighed and declined as well:
//! it would spend a whole extra traversal on every parse, and it would refuse
//! documents that work today in exchange for protection against an abort no
//! measurement has produced.
//!
//! **What stands in a bound's place.** A regression pin —
//! `tests::inline_nesting_never_reaches_the_call_stack` — re-executes this
//! test binary once per probe shape, with `TRANSYNC_INLINE_PROBE_SHAPE` set,
//! and walks 50,000 levels of that shape on a 256 KiB stack inside the child.
//! A stack overflow is an abort, so an inline walk that started using the
//! call stack takes the *child* down, and the parent reads that as a non-zero
//! exit status: a red test rather than a dead test binary. Two things keep it
//! from passing vacuously — the child prints a line the parent requires, so a
//! child that ran no probe fails, and it asserts the inline depth its shape
//! actually reached, so a Comrak that stopped nesting fails too. And the two
//! ceilings are read together there: the child asserts that
//! [`check_nesting_depth`] scores each probe *zero*, because none of these
//! shapes carries a block-container prefix and the block guard must not be
//! what stops them.

use crate::error::ParseError;

/// Deepest block-container nesting [`crate::intake::markdown::parse`] accepts.
///
/// Counted the way a writer counts it: one per blockquote level, one per
/// list level. Comrak spends two AST nodes on a list level (the `List` and
/// the `Item`), so the deepest accepted document is at most `2 * 128 + 1`
/// AST nodes deep — still three orders of magnitude inside the stack of
/// every walker in the workspace.
///
/// No document written to be read comes near this. GFM's own rendering
/// stops distinguishing list indentation long before it, and Comrak's
/// internal per-line list ceiling is 100.
pub const MAX_BLOCK_NESTING_DEPTH: usize = 128;

/// Columns a tab is charged. CommonMark advances to the next multiple of
/// four, so four is the largest a single tab can be worth — the safe
/// direction for a bound that must never under-count.
const TAB_COLUMNS: usize = 4;

/// Columns the shallowest possible list level costs: a one-byte marker plus
/// one column of separation. An *empty* item — a bare marker at end of line —
/// costs the same, because CommonMark gives it a content indent of one column
/// past its marker. Continuation lines of a list `n` levels deep are therefore
/// indented at least `2 * n` columns, which is what lets indentation stand in
/// for the levels a line sits inside.
const MIN_LIST_COLUMNS: usize = 2;

/// Refuse a source whose block nesting could exceed
/// [`MAX_BLOCK_NESTING_DEPTH`].
///
/// Cheap — one linear pass over the bytes, no allocation, no parse — so
/// [`crate::intake::markdown::parse`] can afford to run it unconditionally, first.
/// Exposed rather than kept private so any other entry point that hands raw
/// Markdown to Comrak applies the same number instead of growing a second
/// opinion about it.
///
/// TRACE: ADR-0004
pub fn check_nesting_depth(source: &str) -> Result<(), ParseError> {
    let depth = nesting_depth_bound(source);
    if depth > MAX_BLOCK_NESTING_DEPTH {
        return Err(ParseError::TooDeeplyNested {
            depth,
            limit: MAX_BLOCK_NESTING_DEPTH,
        });
    }
    Ok(())
}

/// An upper bound on the block-container nesting depth `source` can parse
/// to, computed from line prefixes alone.
///
/// Per line, the leading run of container syntax — blockquote markers, list
/// markers, and the whitespace between them — is scanned once and scored:
///
/// ```text
/// bound(line) = blockquote markers
///             + list markers opened on the line
///             + min(indent columns / 2, deepest bound seen so far)
/// ```
///
/// and the answer is the largest score any line reaches.
///
/// **Why that is an upper bound.** A line can only open a block inside a
/// container whose prefix it matches — lazy continuation continues a
/// paragraph, it never opens anything — so the line that opens the deepest
/// node in the document carries, in its own prefix, one `>` per enclosing
/// blockquote and at least two indent columns per enclosing list level,
/// plus its own markers. The `min` with the running maximum is what keeps
/// indentation honest in the other direction: indentation can only stand
/// for list levels some *earlier* line actually opened, and that earlier
/// line's own score was at least as deep.
///
/// **Why it does not fire on real documents.** A line carrying neither a
/// blockquote marker nor a list marker scores at most the running maximum,
/// so it can never raise it. Deep indentation on its own — a wide code
/// block, pretty-printed HTML, a spreadsheet pasted into a fence — is
/// therefore inert. Raising the bound takes a line that both carries a
/// container marker *and* out-indents everything before it, one line per
/// level, 129 times over; or a single line with 129 `>` on it.
///
/// The estimate is deliberately not exact — it over-counts a `***`
/// thematic break as three list markers, and it reads container syntax
/// inside a fenced code block as if it were live. Exactness is not what
/// makes it safe: the margin is. A bound that is off by even a factor of
/// five still refuses everything an order of magnitude short of the
/// shallowest measured abort.
fn nesting_depth_bound(source: &str) -> usize {
    let mut deepest = 0usize;

    // CommonMark line endings are LF, CRLF, and lone CR (OI-0033); splitting
    // on either character covers all three and costs only an empty string
    // per CRLF, which scores zero.
    for line in source.split(['\n', '\r']) {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        let mut quotes = 0usize;
        let mut markers = 0usize;
        let mut columns = 0usize;

        while i < bytes.len() {
            match bytes[i] {
                b' ' => {
                    columns += 1;
                    i += 1;
                }
                b'\t' => {
                    columns += TAB_COLUMNS;
                    i += 1;
                }
                b'>' => {
                    quotes += 1;
                    i += 1;
                }
                _ => match list_marker_width(&bytes[i..]) {
                    Some(width) => {
                        markers += 1;
                        i += width;
                    }
                    // The prefix ended: everything from here is content.
                    None => break,
                },
            }
        }

        let inherited = (columns / MIN_LIST_COLUMNS).min(deepest);
        deepest = deepest.max(quotes + markers + inherited);
    }

    deepest
}

/// Width in bytes of the list marker at the head of `rest`, or `None` when
/// `rest` does not start with one.
///
/// A marker is a bullet (`-`, `*`, `+`) or an ordered marker (up to nine
/// digits then `.` or `)`), closed by a space, a tab, or the end of the
/// line. Something has to close it — that requirement is what keeps ordinary
/// prose (`-- so ...`) and `***` thematic breaks out of the count.
///
/// End of line closes a marker because a bare `-` on its own line **is** a
/// CommonMark list item — an empty one — and an empty item nests at the same
/// two columns per level as any other. Requiring whitespace here would let a
/// document of `2n` spaces plus a lone `-` open a list level per line while
/// scoring zero markers, and a line with no counted marker can never raise
/// the running maximum, so the whole document's bound would pin at zero.
fn list_marker_width(rest: &[u8]) -> Option<usize> {
    // `rest` is the tail of one line, so running out of bytes here is the end
    // of the line, not the end of the document.
    let closed_at = |at: usize| matches!(rest.get(at), None | Some(b' ' | b'\t'));
    // A marker closed by the end of the line has no closing byte to skip.
    let through = |at: usize| (at + 1).min(rest.len());
    match *rest.first()? {
        b'-' | b'*' | b'+' => closed_at(1).then(|| through(1)),
        b'0'..=b'9' => {
            // CommonMark allows at most nine digits; a tenth makes it prose.
            let digits = rest
                .iter()
                .take(9)
                .take_while(|b| b.is_ascii_digit())
                .count();
            match rest.get(digits) {
                Some(b'.' | b')') => closed_at(digits + 1).then(|| through(digits + 1)),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One line, `depth` blockquote markers — the cheapest deep document
    /// there is, one byte per level.
    fn blockquotes(depth: usize) -> String {
        format!("{} hi\n", ">".repeat(depth))
    }

    /// `depth` lines, one list level each, indented two columns per level.
    fn nested_list(depth: usize) -> String {
        (0..depth).fold(String::new(), |mut s, level| {
            s.push_str(&"  ".repeat(level));
            s.push_str("- item\n");
            s
        })
    }

    /// The same shape written with *empty* items — a bare `-` at end of line,
    /// which CommonMark opens a list item for just the same. It nests at the
    /// same two columns per level, so it is a list `depth` levels deep with
    /// no content anywhere in it.
    fn nested_empty_items(depth: usize) -> String {
        (0..depth).fold(String::new(), |mut s, level| {
            s.push_str(&"  ".repeat(level));
            s.push_str("-\n");
            s
        })
    }

    #[test]
    fn ordinary_documents_score_what_a_writer_would_count() {
        for (src, expected) in [
            ("", 0),
            ("# Title\n\nA paragraph.\n", 0),
            ("- a\n- b\n- c\n", 1),
            ("- a\n  - b\n    - c\n", 3),
            ("1. a\n   1. b\n", 2),
            ("> quoted\n", 1),
            ("> > twice\n", 2),
            (">>> thrice\n", 3),
            ("> - a\n>   - b\n", 3),
            ("| a | b |\n| - | - |\n| 1 | 2 |\n", 0),
        ] {
            assert_eq!(nesting_depth_bound(src), expected, "bound for {src:?}",);
        }
    }

    #[test]
    fn indentation_alone_never_raises_the_bound() {
        // A wide code block, pretty-printed HTML, an ASCII table — none of
        // them carry a container marker, so none of them can push the bound
        // up however far they are indented.
        let wide = format!("{}deeply indented code\n", " ".repeat(4000));
        assert_eq!(nesting_depth_bound(&wide), 0);

        let fenced = format!("```\n{}x\n```\n", " ".repeat(4000));
        assert_eq!(nesting_depth_bound(&fenced), 0);

        // Even after a real list has established some depth, indentation
        // cannot claim more than that list did.
        let mixed = format!("- a\n  - b\n\n{}x\n", " ".repeat(4000));
        assert_eq!(nesting_depth_bound(&mixed), 2);
    }

    #[test]
    fn the_bound_never_under_counts_the_three_attack_shapes() {
        for depth in [1usize, 2, 17, 128, 129, 500] {
            assert!(
                nesting_depth_bound(&blockquotes(depth)) >= depth,
                "blockquote depth {depth} under-counted",
            );
            assert!(
                nesting_depth_bound(&nested_list(depth)) >= depth,
                "list depth {depth} under-counted",
            );
            // Review-0002: the empty-item spelling used to score *zero* — the
            // scan demanded whitespace after a marker, so a lone `-` at end of
            // line counted for nothing, and a line with no counted marker
            // cannot raise the running maximum, which pinned the bound for the
            // whole document at zero however deep the list went.
            assert!(
                nesting_depth_bound(&nested_empty_items(depth)) >= depth,
                "empty-item depth {depth} under-counted",
            );
        }
    }

    #[test]
    fn the_limit_is_the_accept_reject_boundary() {
        // Exactly at the ceiling: accepted.
        let at = blockquotes(MAX_BLOCK_NESTING_DEPTH);
        assert!(
            check_nesting_depth(&at).is_ok(),
            "{MAX_BLOCK_NESTING_DEPTH} levels must parse"
        );

        // One past it: refused, and the error says both numbers.
        let past = blockquotes(MAX_BLOCK_NESTING_DEPTH + 1);
        let err = check_nesting_depth(&past).expect_err("one past the ceiling is refused");
        assert!(
            matches!(
                err,
                ParseError::TooDeeplyNested { depth, limit }
                    if depth == MAX_BLOCK_NESTING_DEPTH + 1 && limit == MAX_BLOCK_NESTING_DEPTH
            ),
            "got {err:?}",
        );

        // Same boundary for the line-per-level list shape, in both its
        // spellings — items with content, and empty items.
        assert!(check_nesting_depth(&nested_list(MAX_BLOCK_NESTING_DEPTH)).is_ok());
        assert!(check_nesting_depth(&nested_list(MAX_BLOCK_NESTING_DEPTH + 1)).is_err());
        assert!(check_nesting_depth(&nested_empty_items(MAX_BLOCK_NESTING_DEPTH)).is_ok());
        assert!(check_nesting_depth(&nested_empty_items(MAX_BLOCK_NESTING_DEPTH + 1)).is_err());
    }

    #[test]
    fn lone_cr_lines_are_lines() {
        // OI-0033: a lone CR ends a line, so a list nested across CR-separated
        // lines must score the same as its LF spelling — otherwise the whole
        // document reads as one line and the bound collapses.
        let lf = nested_list(40);
        let cr = lf.replace('\n', "\r");
        assert_eq!(nesting_depth_bound(&cr), nesting_depth_bound(&lf));
        assert_eq!(
            nesting_depth_bound(&lf.replace('\n', "\r\n")),
            nesting_depth_bound(&lf)
        );
    }

    #[test]
    fn a_marker_needs_something_to_close_it() {
        // Prose that merely starts with marker punctuation is not a list.
        assert_eq!(nesting_depth_bound("-no space here\n"), 0);
        assert_eq!(nesting_depth_bound("1.no space here\n"), 0);
        assert_eq!(nesting_depth_bound("1234567890. ten digits\n"), 0);
        assert_eq!(nesting_depth_bound("*emphasis* opens nothing\n"), 0);
        assert_eq!(nesting_depth_bound("***\n"), 0);
        assert_eq!(nesting_depth_bound("---\n"), 0);
        // …but the real forms are.
        assert_eq!(nesting_depth_bound("- item\n"), 1);
        assert_eq!(nesting_depth_bound("123456789. item\n"), 1);
        assert_eq!(nesting_depth_bound("1) item\n"), 1);
    }

    #[test]
    fn end_of_line_closes_a_marker_because_an_empty_item_is_an_item() {
        // Review-0002. Each of these is a CommonMark list item with no
        // content, and each opens a container comrak nests into, so each has
        // to score. The `\n`-less spellings matter too: the scan splits on
        // line endings, so the last line of a file without a trailing newline
        // hits exactly this arm.
        for src in ["-\n", "*\n", "+\n", "1.\n", "1)\n", "-", "1.", "  -\n"] {
            assert_eq!(nesting_depth_bound(src), 1, "bound for {src:?}");
        }
        // A ten-digit ordinal is still prose, closed or not.
        assert_eq!(nesting_depth_bound("1234567890.\n"), 0);
        // And a bare marker still cannot claim indentation no earlier line
        // paid for: the `min` with the running maximum holds.
        assert_eq!(nesting_depth_bound(&format!("{}-\n", " ".repeat(4000))), 1);
    }

    #[test]
    fn tabs_count_as_columns_not_as_bytes() {
        // A tab is worth up to four columns, so tab-indented list nesting
        // must not read as shallower than its space-indented spelling.
        let tabbed = "- a\n\t- b\n\t\t- c\n";
        assert!(
            nesting_depth_bound(tabbed) >= 3,
            "tabbed list under-counted"
        );
    }

    // ---- the inline ceiling, and the pin that stands in for it ------------
    //
    // See the "other nesting" section of this module's docs for why inline
    // nesting is measured rather than bounded. These are the two halves of
    // that decision: the block scan must stay blind to inline punctuation
    // (below), and the inline walks must stay off the call stack (further
    // below).

    #[test]
    fn the_block_scan_never_scores_inline_punctuation() {
        // The negative half of the inline decision. Every one of these is a
        // legitimate construct that a run-scoring inline ceiling would have
        // to refuse, and none of them nests a single block container — so
        // the block scan reads them all as zero, and must keep doing so.
        let mut cases: Vec<String> = vec![
            // A thematic break, and the ASCII rules people write instead.
            "***\n".to_string(),
            "___\n".to_string(),
            "*".repeat(200),
            format!("{}\n", "*".repeat(200)),
            format!("{}\n", "_".repeat(200)),
            // A separator line inside a fenced code block. The pre-scan does
            // not skip fence interiors on purpose, so an inline ceiling
            // sharing that stance would refuse this — and a fence's content
            // is never inline-parsed, so refusing it would buy nothing.
            format!("```\n{}\n```\n", "*".repeat(80)),
            format!("```text\n/{}/\n```\n", "*".repeat(60)),
            // A comment banner, as it appears in a C or Rust fence.
            format!("```c\n/{}\n```\n", "*".repeat(70)),
        ];
        // And the nesting shapes themselves: deep inline trees, zero blocks.
        cases.extend(PROBES.iter().map(|&(shape, _)| probe_source(shape)));

        for src in &cases {
            assert_eq!(
                nesting_depth_bound(src),
                0,
                "inline punctuation scored as block nesting: {:?}…",
                &src[..src.len().min(48)],
            );
        }
    }

    /// Set on a re-executed copy of this test binary to put it in probe-child
    /// mode; its value is the probe shape the child must run.
    const PROBE_SHAPE_ENV: &str = "TRANSYNC_INLINE_PROBE_SHAPE";

    /// Prefix of the one line a probe child prints on success. The parent
    /// requires it: a child that exits 0 without having run a probe would
    /// otherwise be indistinguishable from a child that survived one.
    const PROBE_OK: &str = "inline-probe ok: ";

    /// Levels of inline nesting every probe shape is built to.
    ///
    /// The ticket's own measurement used 50,000, and it is comfortably past
    /// the trap's threshold: measured on this workspace, the leanest possible
    /// recursive walk over the parsed tree — one `fn(node, &mut usize)`, about
    /// 30 bytes of frame per level in release — aborts at 8,536 levels on
    /// `PROBE_STACK_BYTES`. Debug frames (which is how these tests run) are
    /// several times larger, and a real walker's are larger still
    /// (`transync-core`'s recursive topology walk measured ~950 bytes per
    /// level), so every shape that nests overshoots the trap by at least
    /// 2.9× — 5.9× for the two that nest one level per repetition.
    const PROBE_LEVELS: usize = 50_000;

    /// Stack the child walks on: a quarter of the 1 MiB a wasm module gets.
    const PROBE_STACK_BYTES: usize = 256 * 1024;

    /// The probe shapes, each with the floor on the inline AST depth it must
    /// actually reach. The floor is what keeps the pin from passing
    /// vacuously — surviving a walk over a tree that turned out to be flat
    /// would prove nothing — and each one sits above the 8,536 levels at
    /// which even a minimal recursive walk dies on `PROBE_STACK_BYTES`.
    ///
    /// `brackets` is the exception and is listed at 1 on purpose: CommonMark
    /// forbids a link inside a link, so a run of `[` produces a flat tree
    /// however long it is. It is here because it is one of the ticket's three
    /// recorded probes and because it exercises the other structure that
    /// could go recursive — Comrak's bracket bookkeeping, 100,000 entries of
    /// it — rather than the tree walk.
    const PROBES: &[(&str, usize)] = &[
        ("emphasis-run", PROBE_LEVELS / 4),
        ("brackets", 1),
        ("image-alt", PROBE_LEVELS / 4),
        ("emphasis-spaced", PROBE_LEVELS / 2),
        ("image-nest", PROBE_LEVELS / 2),
    ];

    /// One probe shape as Markdown, `PROBE_LEVELS` deep.
    ///
    /// The three the ticket recorded are `emphasis-run`, `brackets` and
    /// `image-alt`; `emphasis-spaced` and `image-nest` were added because
    /// they reach twice the depth per delimiter and do it without a run
    /// longer than one character — which is the measurement that decided
    /// against a run-scoring ceiling.
    fn probe_source(shape: &str) -> String {
        let n = PROBE_LEVELS;
        match shape {
            // `*`×n, x, `*`×n — one delimiter run per side. Depth ≈ n/2.
            "emphasis-run" => format!("{}x{}\n", "*".repeat(n), "*".repeat(n)),
            // `[`×n, x, `]`×n. Depth 1; see `PROBES`.
            "brackets" => format!("{}x{}\n", "[".repeat(n), "]".repeat(n)),
            // An image whose alt text is the `emphasis-run` shape.
            "image-alt" => format!("![{}x{}](u)\n", "*".repeat(n), "*".repeat(n)),
            // The same nesting written with separated single delimiters: no
            // run anywhere is longer than one byte. Depth n + 1.
            "emphasis-spaced" => {
                let mut s = String::with_capacity(6 * n + 2);
                for _ in 0..n {
                    s.push_str("*a ");
                }
                s.push('x');
                for _ in 0..n {
                    s.push_str(" a*");
                }
                s.push('\n');
                s
            }
            // `![`×n, x, `](u)`×n — nested images, `!` and `[` alternating so
            // no run exceeds one byte either. Depth n + 1.
            "image-nest" => {
                let mut s = String::with_capacity(6 * n + 2);
                for _ in 0..n {
                    s.push_str("![");
                }
                s.push('x');
                for _ in 0..n {
                    s.push_str("](u)");
                }
                s.push('\n');
                s
            }
            other => panic!("no probe shape named {other:?}"),
        }
    }

    /// Longest chain of *inline* nodes in `md`'s parse — block containers
    /// reset the count, so this is inline nesting and nothing else.
    ///
    /// Walked on an explicit stack: the instrument must not be the thing that
    /// overflows.
    fn inline_depth(md: &str) -> usize {
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, md, &crate::intake::markdown::comrak_options());
        let mut deepest = 0usize;
        let mut stack = vec![(root, 0usize)];
        while let Some((node, depth)) = stack.pop() {
            let depth = if node.data.borrow().value.block() {
                0
            } else {
                depth + 1
            };
            deepest = deepest.max(depth);
            stack.extend(node.children().map(|child| (child, depth)));
        }
        deepest
    }

    /// Child half: build one shape, walk it on a small stack, report.
    ///
    /// Everything here runs inside a re-executed copy of the test binary, so
    /// an abort is this process's exit status and the parent is what turns it
    /// into a failure.
    fn run_inline_probe(shape: &str) {
        let floor = PROBES
            .iter()
            .find(|&&(name, _)| name == shape)
            .map(|&(_, floor)| floor)
            .unwrap_or_else(|| panic!("{PROBE_SHAPE_ENV} named an unknown shape: {shape:?}"));
        let md = probe_source(shape);

        // Both ceilings, read together: the block guard must not be what
        // stops these. None of the shapes carries a container prefix, so the
        // pre-scan scores every one of them zero and `parse` proceeds.
        assert_eq!(
            nesting_depth_bound(&md),
            0,
            "the block pre-scan scored the `{shape}` probe",
        );

        // The walk needs an owned label: it runs on a `'static` thread.
        let label = shape.to_string();
        let walk = std::thread::Builder::new()
            .stack_size(PROBE_STACK_BYTES)
            .name(format!("inline-probe-{shape}"))
            .spawn(move || {
                let depth = inline_depth(&md);
                assert!(
                    depth >= floor,
                    "the `{label}` probe parsed to inline depth {depth}, under its \
                     {floor} floor — surviving a walk over a tree this flat would \
                     prove nothing. If Comrak grew an inline nesting cap of its own, \
                     record it here rather than lowering the floor.",
                );

                let mut doc = crate::intake::markdown::parse(&md).expect("the probe parses");
                crate::id::assign_block_ids(&mut doc);
                let map = crate::align::build_alignment_map(
                    &doc,
                    &std::collections::HashMap::new(),
                    &crate::regen::BlockOffsets::default(),
                    "auto",
                    "ko",
                    None,
                    &crate::outcome::html_outcomes(&doc),
                );
                let html = crate::render::render_source(&doc, &map)
                    .expect("the map is built from this doc, so it covers every block");
                (depth, doc.blocks.len(), html.len())
            })
            .expect("spawn the small-stack inline walk");

        let (depth, blocks, html_len) = walk.join().expect("the probe's own assertions hold");
        println!("{PROBE_OK}{shape} depth={depth} blocks={blocks} html_len={html_len}");
    }

    /// Parent half: run each shape in its own child and read its exit status.
    fn spawn_inline_probe(shape: &str) {
        let exe = std::env::current_exe().expect("this test binary's own path");
        let child = std::process::Command::new(&exe)
            // A substring filter, so the child runs exactly this test and
            // nothing else. `--nocapture` is what lets `PROBE_OK` reach the
            // pipe; libtest's stdout is line-buffered, so a line printed
            // before an abort still arrives.
            .args([
                "inline_nesting_never_reaches_the_call_stack",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(PROBE_SHAPE_ENV, shape)
            .output()
            .unwrap_or_else(|err| panic!("could not re-run {exe:?} as a probe child: {err}"));
        let stdout = String::from_utf8_lossy(&child.stdout);
        let stderr = String::from_utf8_lossy(&child.stderr);

        assert!(
            child.status.success(),
            "the `{shape}` probe child failed ({status}); its own output is below. \
             `stack overflow, aborting` there is the finding this pin exists for — \
             an inline walk that started using the call stack, which no bound in \
             this module prevents — and anything else is one of the probe's own \
             assertions.\n\
             --- child stdout ---\n{stdout}--- child stderr ---\n{stderr}",
            status = child.status,
        );
        assert!(
            stdout.contains(&format!("{PROBE_OK}{shape} ")),
            "the `{shape}` probe child exited 0 without reporting `{PROBE_OK}` — it \
             ran no probe, so its success proves nothing. The child selects the probe \
             by this test's own name; a rename has to move with it.\n\
             --- child stdout ---\n{stdout}--- child stderr ---\n{stderr}",
        );
    }

    /// Inline nesting is unbounded on purpose, so this is what says it stays
    /// safe: 50,000 levels of each probe shape, walked on a 256 KiB stack, in
    /// a child process whose death is a failed assertion here rather than a
    /// dead test binary.
    ///
    /// Read the module docs above for the measurements this replaced a bound
    /// with. In one sentence: no byte-level score is an upper bound on inline
    /// depth, and no inline walk on the path has ever been measured to abort,
    /// so the thing worth pinning is that second fact.
    #[test]
    fn inline_nesting_never_reaches_the_call_stack() {
        match std::env::var(PROBE_SHAPE_ENV) {
            // Child: one shape, then out. Never spawns, so this cannot
            // recurse into a fork bomb.
            Ok(shape) => run_inline_probe(&shape),
            Err(_) => {
                for &(shape, _) in PROBES {
                    spawn_inline_probe(shape);
                }
            }
        }
    }
}
