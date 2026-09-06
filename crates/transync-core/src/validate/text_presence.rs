//! Layer 2, content half: did the translation keep any text a reader can see?
//!
//! Every other per-unit layer is **structural and text-blind**. `per_kind`
//! compares a heading's level, a table's `(cols, rows, alignments)`, a code
//! block's info string, a list's node-kind fingerprint; `fragment_reparse`
//! compares kind labels; `inline` compares link destinations and code-span
//! multisets; layer 6 compares block counts, tag ledgers and gap bytes.
//! Not one of them asks whether the words survived. So a provider that
//! returns a payload keeping every structural fact and none of the text
//! ships as `translated`, with `fallback_status: translated`, and no warning
//! anywhere — the exact outcome architectural invariant 6 ("never silently
//! corrupt the output") forbids.
//!
//! This module is the one place that asks. It holds THE definition of
//! "renders nothing" ([`is_invisible`]) and applies it on both intakes, so
//! the two can never drift the way `per_kind`'s old whitespace guard drifted
//! from `intake::html`'s rule T (ti `c887bc`).
//!
//! # The rule is SCOPED, and the scope is what makes the predicate safe
//!
//! Reject only when **the source carried visible text and the translation
//! carries none**. Never on the translation alone.
//!
//! That scope is not politeness, it is what lets [`is_invisible`] be
//! aggressive. A `<td>&#8203;</td>` layout shim, a zero-width-joiner-only
//! emoji fragment, an empty-bodied code block, a thematic break — each is a
//! legitimate unit with no visible text of its own, and an unconditional
//! predicate would reject its faithful echo and burn the unit's whole retry
//! budget on the way to a pointless fallback. Under the scope those units
//! have no visible source text either, so the check never fires on them and
//! the membership question stops being a decision about false positives.
//! Over-inclusion in the predicate costs nothing; under-inclusion is a miss.
//!
//! # Two intakes, two different things to read, one predicate
//!
//! The asymmetry is entity handling, and it comes from the splice:
//!
//! - **`HtmlSegments`** — compare the segment STRINGS as they stand.
//!   `transync_html::extract` decodes source segments (`htmlize::unescape`),
//!   and `splice` re-escapes a genuinely translated one (`ContentType::Text`
//!   escapes `<`, `>`, `&`), so a provider's `"&nbsp;"` reaches the page as
//!   the six visible characters `&nbsp;` rather than as U+00A0. Both sides
//!   are therefore already "the text as it will render", and decoding here
//!   would invent an erasure the splice prevents.
//! - **Markdown** — compare comrak's DECODED text. Nothing re-escapes on
//!   this path: `&nbsp;` in a Markdown payload is resolved by comrak at
//!   inline parse and reaches the output as U+00A0. A byte-level test on the
//!   payload would be bypassed by `&nbsp;` / `&#8203;` / `&#xFEFF;`, all of
//!   which are pure ASCII in the payload. So this half reads the parsed AST,
//!   not the bytes.
//!
//! Raw markup is deliberately NOT text: an `HtmlInline`/`HtmlBlock` node
//! contributes nothing, so a paragraph "translated" to `<b></b>` is a
//! rejection. Tag identity belongs to `inline`; this layer only counts words.
//!
//! # What this layer is not
//!
//! It is not a translation-quality judge. A payload of `**` renders two
//! visible asterisks, so it passes — pinned by
//! `visible_garbage_is_not_this_layers_business`. Rejecting it would mean
//! deciding which visible characters are "really" text, which is a content
//! decision and belongs to the model (architectural invariant 2).
//!
//! TRACE: ti c887bc
//! TRACE: R0003-0042

use crate::llm::{InputMode, TranslationUnit, UnitResult};

/// Does `c` occupy no visual space?
///
/// Two halves, deliberately sourced differently:
///
/// 1. `char::is_whitespace` — the 25 Unicode `White_Space` codepoints
///    (U+00A0 and U+3000 included). Taken from `std` rather than vendored so
///    it tracks the compiler's Unicode version; a second table for this half
///    would be a second opinion.
/// 2. The explicit list below — codepoints that render nothing but are NOT
///    `White_Space`, so half 1 cannot see them. This is where the old
///    whitespace guard's gap lived.
///
/// The list is hand-written rather than a `unicode_categories`-style
/// `Other_Format` test, for two reasons. It is not **sufficient**: U+2800
/// BRAILLE PATTERN BLANK is `So` and the Hangul fillers are `Lo`, yet all
/// three render as blank. And a vendored category table pins an older Unicode
/// version than `std`'s, so the two halves would disagree about codepoints
/// assigned in between — exactly the drift this module exists to prevent.
///
/// Over-inclusion is safe by construction; see the module docs on scope.
pub(crate) fn is_invisible(c: char) -> bool {
    if c.is_whitespace() {
        return true;
    }
    matches!(
        c,
        '\u{00ad}'                  // SOFT HYPHEN
        | '\u{034f}'                // COMBINING GRAPHEME JOINER
        | '\u{061c}'                // ARABIC LETTER MARK
        | '\u{115f}' | '\u{1160}'   // HANGUL CHOSEONG / JUNGSEONG FILLER
        | '\u{17b4}' | '\u{17b5}'   // KHMER VOWEL INHERENT AQ / AA
        | '\u{180b}'..='\u{180f}'   // MONGOLIAN FVS 1-4, VOWEL SEPARATOR
        | '\u{200b}'..='\u{200f}'   // ZWSP, ZWNJ, ZWJ, LRM, RLM
        | '\u{202a}'..='\u{202e}'   // bidi embedding / override controls
        | '\u{2060}'..='\u{206f}'   // WORD JOINER .. deprecated format chars
        | '\u{2800}'                // BRAILLE PATTERN BLANK
        | '\u{3164}'                // HANGUL FILLER
        | '\u{fe00}'..='\u{fe0f}'   // VARIATION SELECTOR-1 .. -16
        | '\u{feff}'                // ZERO WIDTH NO-BREAK SPACE (BOM)
        | '\u{ffa0}'                // HALFWIDTH HANGUL FILLER
        | '\u{fff9}'..='\u{fffb}'   // interlinear annotation controls
        | '\u{1d173}'..='\u{1d17a}' // musical symbol format controls
        | '\u{e0000}'..='\u{e0fff}' // tag chars, variation selectors supplement
    )
}

/// Is there one character in `s` that a reader can see?
pub(crate) fn has_visible_text(s: &str) -> bool {
    s.chars().any(|c| !is_invisible(c))
}

/// The layer. Retryable on rejection, like every other per-unit layer: an
/// erasure is a provider fault, the correct answer is stated in the message,
/// and a provider that keeps erasing falls back to source after the budget.
pub fn check(unit: &TranslationUnit, result: &UnitResult) -> Result<(), String> {
    match unit.input_mode {
        InputMode::HtmlSegments => check_segments(&unit.source_payload, &result.translated_payload),
        // Exhaustive on purpose: a new intake mode must stop the compiler
        // here and be classified, not inherit the Markdown reading of its
        // payload by accident.
        InputMode::TextFragment
        | InputMode::FullTableMarkdown
        | InputMode::TableRowWindow { .. }
        | InputMode::FullCodeBlock { .. }
        | InputMode::ListItemContent
        | InputMode::BlockquoteContent => {
            check_markdown(&unit.source_payload, &result.translated_payload)
        }
    }
}

/// `HtmlSegments`: per SEGMENT, not per block. A block with ten segments of
/// which one is erased is still an erasure, and naming the index is what
/// makes the message actionable.
fn check_segments(source_payload: &str, translated_payload: &str) -> Result<(), String> {
    let Ok(source): Result<Vec<String>, _> = serde_json::from_str(source_payload) else {
        // `unit::payload::assemble` builds this payload with
        // `serde_json::to_string(&Vec<String>)`, so a parse failure means a
        // hand-built unit — a caller bug, not a provider fault, and the
        // wrong thing to charge to the retry budget.
        debug_assert!(
            false,
            "HtmlSegments unit's source_payload is not a JSON array of strings"
        );
        return Ok(());
    };
    let Ok(translated): Result<Vec<String>, _> = serde_json::from_str(translated_payload) else {
        // Defensive: `per_kind::check_html` runs first and rejects malformed
        // JSON with a better reason.
        return Ok(());
    };
    if source.len() != translated.len() {
        // Defensive for the same reason — the count check is `check_html`'s.
        return Ok(());
    }
    for (i, (src, out)) in source.iter().zip(translated.iter()).enumerate() {
        if has_visible_text(src) && !has_visible_text(out) {
            return Err(format!(
                "html segment {i} renders no visible text (empty, whitespace-only, \
                 or zero-width-only) while the source segment carried text; \
                 echo the source segment to preserve it"
            ));
        }
    }
    Ok(())
}

/// Markdown: whole payload, against comrak's decoded text.
fn check_markdown(source_payload: &str, translated_payload: &str) -> Result<(), String> {
    if !has_visible_text(&rendered_text(source_payload)) {
        return Ok(());
    }
    if has_visible_text(&rendered_text(translated_payload)) {
        return Ok(());
    }
    Err(
        "the translated payload renders no visible text (empty, whitespace-only, \
         or zero-width-only) while the source payload carried text; echo the \
         source payload to preserve it"
            .to_string(),
    )
}

/// Every character of `payload` that reaches the reader as TEXT.
///
/// Text-bearing nodes only. Excluded on purpose:
///
/// - a code block's **info string** — `check_code` owns it, and counting it
///   would make this test unfalsifiable for every fenced payload (`rust` is
///   always there);
/// - **link destinations and titles** — `inline` owns them, and a URL is not
///   prose;
/// - **raw HTML** (`HtmlInline` / `HtmlBlock`) — markup, not text, which is
///   what makes a paragraph erased to `<b></b>` a rejection here.
///
/// A payload past the nesting ceiling yields no text. `validate_unit`'s door
/// already refused it with a better reason, so this is unreachable in the
/// pipeline; returning empty keeps the function total, and empty text is the
/// permissive answer rather than a rejection this layer cannot justify.
fn rendered_text(payload: &str) -> String {
    use comrak::nodes::NodeValue;

    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let Ok(root) = crate::markdown::guarded_parse(&arena, payload, &opts) else {
        return String::new();
    };

    let mut out = String::new();
    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::CodeBlock(cb) => out.push_str(&cb.literal),
            NodeValue::Math(m) => out.push_str(&m.literal),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{BatchId, BlockConstraints, BlockContext, OutputKind, TranslationUnit};

    /// `block_kind` is deliberately fixed and never read: this layer keys on
    /// the intake MODE, for the same reason ti `490d97` wave 2 re-keyed
    /// `check_html` off the kind — an HTML document's `<p>` is a
    /// `BlockKind::Paragraph` and still ships a JSON segment array, so a
    /// kind-keyed dispatch would hand its JSON to comrak.
    fn unit(mode: InputMode, source_payload: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("x-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: mode,
            source_payload: source_payload.to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn result(payload: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId("x-0001".to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: payload.to_string(),
            warnings: Vec::new(),
        }
    }

    /// Assert an erasure and hand back the message.
    #[track_caller]
    fn rejects(mode: InputMode, source: &str, translated: &str) -> String {
        match check(&unit(mode, source), &result(translated)) {
            Err(reason) => reason,
            Ok(()) => panic!("must be rejected: source {source:?} -> {translated:?}"),
        }
    }

    #[track_caller]
    fn passes(mode: InputMode, source: &str, translated: &str) {
        if let Err(reason) = check(&unit(mode, source), &result(translated)) {
            panic!("must pass: source {source:?} -> {translated:?}: {reason}");
        }
    }

    fn md() -> InputMode {
        InputMode::TextFragment
    }

    fn code() -> InputMode {
        InputMode::FullCodeBlock {
            language_info: Some("rust".to_string()),
        }
    }

    fn segs(v: &[&str]) -> String {
        serde_json::to_string(v).expect("Vec<&str> serializes")
    }

    // ---------------------------------------------------------------- the predicate

    /// Both halves of [`is_invisible`], one codepoint at a time. The second
    /// group is the gap the old whitespace guard had: not one of them is
    /// `char::is_whitespace`, and each renders nothing.
    #[test]
    fn the_predicate_membership_is_pinned() {
        for c in [
            ' ', '\t', '\n', '\r', '\u{000b}', '\u{00a0}', '\u{2007}', '\u{3000}',
        ] {
            assert!(
                c.is_whitespace(),
                "fixture: U+{:04X} is White_Space",
                c as u32
            );
            assert!(is_invisible(c), "U+{:04X} must be invisible", c as u32);
        }
        for c in [
            '\u{00ad}',  // SOFT HYPHEN
            '\u{034f}',  // COMBINING GRAPHEME JOINER
            '\u{061c}',  // ARABIC LETTER MARK
            '\u{115f}',  // HANGUL CHOSEONG FILLER
            '\u{180e}',  // MONGOLIAN VOWEL SEPARATOR
            '\u{200b}',  // ZERO WIDTH SPACE
            '\u{200c}',  // ZERO WIDTH NON-JOINER
            '\u{200d}',  // ZERO WIDTH JOINER
            '\u{200e}',  // LEFT-TO-RIGHT MARK
            '\u{202d}',  // LEFT-TO-RIGHT OVERRIDE
            '\u{2060}',  // WORD JOINER
            '\u{2066}',  // LEFT-TO-RIGHT ISOLATE
            '\u{2800}',  // BRAILLE PATTERN BLANK
            '\u{3164}',  // HANGUL FILLER
            '\u{fe0f}',  // VARIATION SELECTOR-16
            '\u{feff}',  // ZERO WIDTH NO-BREAK SPACE
            '\u{ffa0}',  // HALFWIDTH HANGUL FILLER
            '\u{fffb}',  // INTERLINEAR ANNOTATION TERMINATOR
            '\u{e0041}', // TAG LATIN CAPITAL LETTER A
        ] {
            assert!(
                !c.is_whitespace(),
                "the whole point: U+{:04X} is NOT White_Space, which is why \
                 the old guard missed it",
                c as u32,
            );
            assert!(is_invisible(c), "U+{:04X} must be invisible", c as u32);
        }
        // Visible, and must stay visible — over-inclusion is safe under the
        // scope but these are the characters real translations are made of.
        for c in [
            'a', '가', '漢', '*', '#', '-', '|', '`', '·', '•', '\u{fffd}', '☺', '🇰',
        ] {
            assert!(!is_invisible(c), "U+{:04X} renders", c as u32);
        }
    }

    // ---------------------------------------------------------------- markdown: per kind
    //
    // Every payload below was MEASURED shipping as `translated` against the
    // pre-fix code (ti `c887bc`): each keeps the structural fact its kind's
    // per-kind arm compares, so `per_kind`, `fragment_reparse`, `inline` and
    // layer 6 all pass it. The rule the table shows: the more markup a kind
    // carries, the more the attacker must keep — and keeping it is always
    // sufficient.

    #[test]
    fn paragraph_erasures_are_rejected() {
        const SRC: &str = "The quick brown fox jumps over the lazy dog.";
        for out in [
            "",                                 // nothing at all
            " ",                                // the case the old guard did catch
            "\u{200b}",                         // ZWSP
            "\u{200c}",                         // ZWNJ
            "\u{200d}",                         // ZWJ
            "\u{2060}",                         // WORD JOINER
            "\u{00ad}",                         // SOFT HYPHEN
            "\u{feff}",                         // BOM
            " \u{feff}",                        // one space in front: not comrak's BOM strip
            "\u{feff}\u{feff}",                 // doubled: the strip takes only the first
            "\u{2800}", // BRAILLE PATTERN BLANK — neither White_Space nor Cf
            "&nbsp;",   // ASCII in the payload, U+00A0 after comrak
            "&#8203;",  // decimal ZWSP
            "&#xfeff;", // hex BOM
            "\u{200b}\u{2060}\u{00ad}\u{feff}", // a run of them
        ] {
            let msg = rejects(md(), SRC, out);
            assert!(
                msg.contains("no visible text") && msg.contains("echo the source payload"),
                "{out:?}: the message must state the erasure and the fix: {msg}",
            );
        }
    }

    /// A heading's level is what `check_heading` compares, and `##` alone
    /// satisfies it: `<h2></h2>` is a level-2 heading with no text.
    #[test]
    fn heading_erasures_are_rejected() {
        const SRC: &str = "## Installing the toolchain";
        for out in [
            "##",
            "## ",
            "## \u{200b}",
            "## \u{2060}",
            "## \u{feff}",
            "## &nbsp;",
            "## &#8203;",
            "## \u{2800}",
            "##\n",
        ] {
            rejects(md(), SRC, out);
        }
    }

    /// `check_table` compares `(cols, rows, alignments)`. Blank cells change
    /// none of the three.
    #[test]
    fn table_erasures_are_rejected() {
        const SRC: &str = "| Option | Meaning |\n| --- | --- |\n| `-v` | verbose |";
        for out in [
            "|  |  |\n| --- | --- |\n|  |  |",
            "| \u{200b} | \u{200b} |\n| --- | --- |\n| \u{200b} | \u{200b} |",
            "| &nbsp; | &nbsp; |\n| --- | --- |\n| &nbsp; | &nbsp; |",
            "| \u{feff} | \u{2800} |\n| --- | --- |\n| \u{00ad} | \u{2060} |",
        ] {
            rejects(InputMode::FullTableMarkdown, SRC, out);
        }
    }

    /// `check_code` compares the info string, in both directions. Keeping
    /// `rust` and dropping the body satisfies it — and the info string is
    /// deliberately not counted as text, or this check could never fire on a
    /// fenced payload.
    #[test]
    fn code_block_erasures_are_rejected() {
        const SRC: &str = "```rust\nfn main() { println!(\"hi\"); }\n```";
        for out in [
            "```rust\n```",
            "```rust\n\n```",
            "```rust\n\u{200b}\n```",
            "```rust\n\u{feff}\n```",
            "```rust\n   \n```",
        ] {
            rejects(code(), SRC, out);
        }
    }

    /// `check_list` compares a node-kind-label fingerprint: one item with one
    /// paragraph. An item whose paragraph is invisible has the same one.
    #[test]
    fn list_item_erasures_are_rejected() {
        const SRC: &str = "- install the toolchain";
        for out in [
            "- \u{200b}",
            "- \u{feff}",
            "- &nbsp;",
            "- &#8203;",
            "- \u{2800}",
            "-",
        ] {
            rejects(InputMode::ListItemContent, SRC, out);
        }
    }

    #[test]
    fn blockquote_erasures_are_rejected() {
        const SRC: &str = "> a quoted remark";
        for out in ["> \u{200b}", "> \u{feff}", "> &nbsp;", "> \u{2800}", ">"] {
            rejects(InputMode::BlockquoteContent, SRC, out);
        }
    }

    /// A table row window is a whole GFM table under exactly the table rules
    /// (DCR-0026), so it must be read the same way — not left to inherit a
    /// default by accident.
    #[test]
    fn a_table_row_window_is_read_as_a_table() {
        let mode = InputMode::TableRowWindow {
            parent_block_id: BlockId("t-0001".to_string()),
            window_index: 0,
            window_count: 2,
        };
        rejects(
            mode.clone(),
            "| Option | Meaning |\n| --- | --- |\n| `-v` | verbose |",
            "|  |  |\n| --- | --- |\n|  |  |",
        );
        passes(
            mode,
            "| Option | Meaning |\n| --- | --- |\n| `-v` | verbose |",
            "| 옵션 | 뜻 |\n| --- | --- |\n| `-v` | 자세히 |",
        );
    }

    /// The constraint that reshaped the fix: `&nbsp;` and `&#8203;` are pure
    /// ASCII **in the payload bytes**, so any byte-level predicate over
    /// `translated_payload` passes them and comrak resolves them afterwards.
    /// Reading the parsed text is what closes it — pinned here as a property
    /// of the payload, not just as another row in the tables above.
    #[test]
    fn the_entity_bypass_is_closed_on_the_markdown_path() {
        for payload in ["&nbsp;", "&#8203;", "&#xfeff;", "&#160;", "&zwnj;"] {
            assert!(
                payload.is_ascii() && has_visible_text(payload),
                "fixture: {payload:?} is visible ASCII as BYTES — a byte-level \
                 check cannot see the erasure",
            );
            rejects(md(), "real prose here", payload);
        }
    }

    // ---------------------------------------------------------------- html segments

    #[test]
    fn html_segment_erasures_are_rejected() {
        let src = segs(&["Click me", "Body text"]);
        for out in [
            segs(&["Click me", ""]),
            segs(&["Click me", " "]),
            segs(&["Click me", "\u{feff}"]),
            segs(&["Click me", "\u{200b}"]),
            segs(&["Click me", "\u{2060}"]),
            segs(&["Click me", "\u{00ad}"]),
            segs(&["Click me", "\u{2800}"]),
            segs(&["Click me", "\u{00a0}\u{00a0}"]),
            segs(&["", ""]),
        ] {
            let msg = rejects(InputMode::HtmlSegments, &src, &out);
            assert!(
                msg.contains("echo the source segment"),
                "the message must state the fix: {msg}",
            );
        }
    }

    /// Per segment, and the index is named — a ten-segment block with one
    /// erased segment is an erasure, and "which one" is the actionable part.
    #[test]
    fn the_rejection_names_the_erased_segment() {
        let src = segs(&["alpha", "beta", "gamma"]);
        let msg = rejects(
            InputMode::HtmlSegments,
            &src,
            &segs(&["알파", "\u{200b}", "감마"]),
        );
        assert!(msg.contains("segment 1"), "got: {msg}");
        let msg = rejects(
            InputMode::HtmlSegments,
            &src,
            &segs(&["\u{feff}", "베타", "감마"]),
        );
        assert!(msg.contains("segment 0"), "got: {msg}");
    }

    /// The intake asymmetry, and it is counterintuitive enough to pin: on the
    /// HTML path an entity is NOT an erasure. `splice` writes a genuinely
    /// translated segment as `ContentType::Text`, which escapes `&`, so
    /// `"&nbsp;"` reaches the page as the six visible characters `&nbsp;`.
    /// Decoding here would invent an erasure the splice prevents — which is
    /// why this half reads the strings and the Markdown half reads the AST.
    #[test]
    fn an_html_segment_entity_is_not_an_erasure() {
        passes(
            InputMode::HtmlSegments,
            &segs(&["Click me"]),
            &segs(&["&nbsp;"]),
        );
        passes(
            InputMode::HtmlSegments,
            &segs(&["Click me"]),
            &segs(&["&#8203;"]),
        );
    }

    // ---------------------------------------------------------------- the scope

    /// The scope, stated as the false positives it prevents. Each source here
    /// legitimately has no visible text of its own, so an UNCONDITIONAL
    /// predicate would reject its faithful echo and spend the unit's whole
    /// retry budget reaching a fallback that changes nothing.
    #[test]
    fn a_source_without_visible_text_is_never_rejected() {
        // The `<td>&#8203;</td>` layout shim: extract keeps it (its drop test
        // is `all char::is_whitespace`, and ZWSP is not whitespace), so it
        // reaches the model and comes back invisible either way.
        passes(
            InputMode::HtmlSegments,
            &segs(&["\u{200b}"]),
            &segs(&["\u{feff}"]),
        );
        passes(
            InputMode::HtmlSegments,
            &segs(&["\u{200d}"]),
            &segs(&["\u{200d}"]),
        );
        // Markdown counterparts.
        passes(md(), "\u{200b}", "\u{feff}");
        passes(md(), "&#8203;", "&nbsp;");
        // An empty-bodied fenced block, echoed.
        passes(code(), "```rust\n```", "```rust\n```");
        // A thematic break carries no text at all, and never has (its
        // alignment row has been text-free since schema 1.0).
        passes(md(), "---", "---");
        // An image with no alt text: the destination is `inline`'s business,
        // and there is no prose to lose.
        passes(md(), "![](diagram.png)", "![](diagram.png)");
    }

    #[test]
    fn faithful_translations_pass_on_every_kind() {
        passes(md(), "The quick brown fox.", "빠른 갈색 여우.");
        passes(md(), "## Installing", "## 설치하기");
        passes(
            InputMode::FullTableMarkdown,
            "| Option | Meaning |\n| --- | --- |\n| `-v` | verbose |",
            "| 옵션 | 뜻 |\n| --- | --- |\n| `-v` | 자세히 |",
        );
        passes(
            code(),
            "```rust\nfn main() { println!(\"hi\"); }\n```",
            "```rust\nfn main() { println!(\"안녕\"); }\n```",
        );
        passes(InputMode::ListItemContent, "- install it", "- 설치하기");
        passes(InputMode::BlockquoteContent, "> a remark", "> 한마디");
        passes(
            InputMode::HtmlSegments,
            &segs(&["Click me", "Body text"]),
            &segs(&["클릭", "본문"]),
        );
        // A code block whose body is only an identifier the profile protects:
        // still text.
        passes(
            code(),
            "```\nCARGO_TARGET_DIR\n```",
            "```\nCARGO_TARGET_DIR\n```",
        );
    }

    /// The boundary, pinned so it stays a decision rather than an accident.
    /// `**` renders two visible asterisks; so does `##` used as a paragraph.
    /// Both are bad translations and neither is an erasure, and deciding
    /// which visible characters are "really" text is a content decision that
    /// belongs to the model (architectural invariant 2). This layer counts
    /// words; it does not grade them.
    #[test]
    fn visible_garbage_is_not_this_layers_business() {
        passes(md(), "The quick brown fox.", "**");
        passes(md(), "The quick brown fox.", "...");
        passes(md(), "The quick brown fox.", "TODO");
        // The one that looks like an erasure and is not: a code SPAN's
        // literal is text, even when it is punctuation.
        passes(md(), "The quick brown fox.", "`|`");
    }

    /// Raw markup is not text. A paragraph "translated" into empty tags has
    /// lost every word, and `inline`'s tag inventory only compares counts —
    /// so if this layer counted `HtmlInline` as text, `<b></b>` would ship.
    #[test]
    fn empty_markup_is_not_text() {
        rejects(md(), "The quick brown fox.", "<b></b>");
        rejects(md(), "The quick brown fox.", "<span>\u{200b}</span>");
        passes(md(), "The quick brown fox.", "<b>여우</b>");
    }

    /// Totality at the seams. None of these is reachable through the
    /// pipeline — `per_kind::check_html` rejects malformed JSON and a count
    /// mismatch first, and the door refuses a past-ceiling payload — but this
    /// function must not panic or invent a rejection for a caller that
    /// arrives another way.
    #[test]
    fn malformed_input_defers_to_the_layer_that_owns_it() {
        // Not JSON at all: `check_html`'s message is the better one.
        passes(InputMode::HtmlSegments, &segs(&["a"]), "not json");
        // Count mismatch: `check_html` owns it.
        passes(InputMode::HtmlSegments, &segs(&["a", "b"]), &segs(&["가"]));
    }
}
