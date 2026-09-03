//! SCN-04 — Code-block fence regeneration + comment translation.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-04:
//! - Output fence is at least one backtick longer than the longest run of
//!   backticks in the body.
//! - Info string (`rust`) is preserved byte-for-byte.
//!
//! TRACE: SCN-04
//! TRACE: SL-04

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_04() {
    let source = include_str!("../fixtures/scn-04-code-block.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-04 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert!(!output.translated_document.is_empty());

    // Locate the code block in the regenerated output.
    let (fence_len, info, body) = extract_first_code_block(&output.translated_document)
        .expect("regenerated MD should contain a single code block");

    assert_eq!(info, "rust", "info string must round-trip byte-for-byte");

    let max_run = longest_backtick_run(&body);
    assert!(
        fence_len > max_run,
        "fence length {fence_len} must be > longest body run ({max_run})",
    );
    assert!(
        max_run >= 3,
        "SCN-04 fixture should contain a 3-backtick run inside the body (got {max_run})",
    );
}

fn extract_first_code_block(md: &str) -> Option<(usize, String, String)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    for child in root.children() {
        if let NodeValue::CodeBlock(c) = &child.data.borrow().value {
            return Some((c.fence_length, c.info.clone(), c.literal.clone()));
        }
    }
    None
}

fn longest_backtick_run(s: &str) -> usize {
    let mut max = 0usize;
    let mut cur = 0usize;
    for c in s.chars() {
        if c == '`' {
            cur += 1;
            if cur > max {
                max = cur;
            }
        } else {
            cur = 0;
        }
    }
    max
}

// ---------------------------------------------------------------------------
// ti `457e51` — indented (four-space) code blocks.
//
// SCN-04 covered *fenced* code only, which is why an indented block could
// burn three provider attempts per run and come back untranslated without a
// single test noticing. The fixture below carries both indented spellings the
// ticket names:
//
// | id       | kind      | source spelling                                  |
// |----------|-----------|--------------------------------------------------|
// | `p-0001` | paragraph | intro                                            |
// | `c-0002` | code      | four-space indent, body holds a ``` run           |
// | `p-0003` | paragraph | separator (an indented block may not follow a     |
// |          |           | blank line after another one — CommonMark would   |
// |          |           | merge the two into a single block)                |
// | `c-0004` | code      | eight-space indent, single line                   |
// | `p-0005` | paragraph | outro                                            |
//
// TRACE: SCN-04
// TRACE: ti 457e51
// ---------------------------------------------------------------------------

const INDENTED: &str = include_str!("../fixtures/scn-04-indented-code.md");

/// Every code block reachable from a regenerated document, in source order:
/// `(fenced, info, literal)`.
fn code_blocks(md: &str) -> Vec<(bool, String, String)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    root.children()
        .filter_map(|child| match &child.data.borrow().value {
            NodeValue::CodeBlock(c) => Some((c.fenced, c.info.clone(), c.literal.clone())),
            _ => None,
        })
        .collect()
}

/// Top-level node labels of a document, in source order. Enough to say
/// "the regenerated document has the source's shape" without welding to the
/// IR's own kind names.
fn top_level_labels(md: &str) -> Vec<&'static str> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);
    root.children()
        .map(|child| match &child.data.borrow().value {
            NodeValue::Paragraph => "paragraph",
            NodeValue::CodeBlock(_) => "code-block",
            NodeValue::Heading(_) => "heading",
            NodeValue::List(_) => "list",
            NodeValue::BlockQuote => "blockquote",
            NodeValue::HtmlBlock(_) => "html",
            NodeValue::ThematicBreak => "thematic-break",
            _ => "other",
        })
        .collect()
}

/// ti `457e51`: a passthrough run over indented code must cost nothing extra
/// and must translate both blocks.
///
/// The four-space block is the ticket's headline defect (three attempts,
/// `fallback_source`, `fragment reparse kind mismatch: expected code-block,
/// got paragraph`). The eight-space block fails by the *other* mechanism —
/// its dedented slice is itself a valid indented code block, so it is
/// ACCEPTED on attempt one and then corrupts `out.md`, which the
/// full-document reparse catches and cascades. `full_reparse_fallbacks` being
/// empty is that second symptom's assertion.
#[tokio::test]
async fn smoke_scn_04_indented_code() {
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(INDENTED, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the indented-code fixture");

    // 1. Nothing fell back, and nothing was retried.
    let statuses: Vec<(&str, transync::FallbackStatus)> = output
        .alignment_map
        .blocks
        .iter()
        .map(|b| (b.source_block_id.0.as_str(), b.fallback_status))
        .collect();
    assert!(
        statuses
            .iter()
            .all(|(_, s)| *s == transync::FallbackStatus::Translated),
        "every block must translate; got {statuses:?}\n\
         full_reparse_fallbacks: {:?}\n--- out.md ---\n{}",
        output.validation_report.full_reparse_fallbacks,
        output.translated_document,
    );
    assert_eq!(
        output.alignment_map.validation_summary.retried_units,
        0,
        "an indented block must not burn a retry; per-unit log: {:?}",
        output
            .validation_report
            .per_unit
            .iter()
            .map(|r| (r.unit_id.0.as_str(), r.attempts.len()))
            .collect::<Vec<_>>(),
    );
    assert_eq!(
        output.alignment_map.validation_summary.fallback_source, 0,
        "no unit may settle at fallback_source",
    );

    // 2. The eight-space symptom: the post-regen full-document reparse must
    //    find nothing to downgrade.
    assert!(
        output.validation_report.full_reparse_fallbacks.is_empty(),
        "regenerated document must reparse to the source shape; downgrades: {:?}\n--- out.md ---\n{}",
        output.validation_report.full_reparse_fallbacks,
        output.translated_document,
    );

    // 3. The regenerated document keeps the source's top-level shape, with
    //    both code blocks re-emitted FENCED (the accepted-path spelling).
    assert_eq!(
        top_level_labels(&output.translated_document),
        vec![
            "paragraph",
            "code-block",
            "paragraph",
            "code-block",
            "paragraph"
        ],
        "out.md:\n{}",
        output.translated_document,
    );

    let blocks = code_blocks(&output.translated_document);
    assert_eq!(blocks.len(), 2, "out.md:\n{}", output.translated_document);

    let (fenced_a, info_a, literal_a) = &blocks[0];
    assert!(*fenced_a, "an accepted indented block is re-emitted fenced");
    assert_eq!(info_a, "", "an indented block has no info string to carry");
    assert_eq!(
        literal_a, "let s = \"```\";\nprintln!(\"{}\", s);\n",
        "the four-space block's content is its CommonMark dedent",
    );

    let (fenced_b, info_b, literal_b) = &blocks[1];
    assert!(*fenced_b, "an accepted indented block is re-emitted fenced");
    assert_eq!(info_b, "");
    assert_eq!(
        literal_b, "    indented code line\n",
        "eight spaces = four of block structure + four of CONTENT; the \
         second four survive into the fenced body",
    );
}

/// Invariant 6 (retry then fallback, never silent corruption) against the
/// widened source range: a unit the provider refuses outright still splices
/// its own source bytes back, so an indented block that falls back keeps its
/// INDENTED spelling — the re-fencing is the accepted path's behavior only.
///
/// Single-block source on purpose: over the shared fixture the eight-space
/// block is legitimately translated (and therefore fenced) in the same run,
/// so "out.md is byte-identical to the source" is only a true statement about
/// a document whose every code block fell back.
#[tokio::test]
async fn indented_code_that_falls_back_keeps_its_indented_spelling() {
    const SRC: &str = "Intro paragraph.\n\n    let x = 1;\n\nOutro paragraph.\n";

    let translator = MockTranslator::always_fails_unit(transync::BlockId::new("c", 2));
    let opts = crate::common::opts_for("ko");

    let output = translate(SRC, &opts, &translator)
        .await
        .expect("pipeline returns Ok");

    let row = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "c-0002")
        .expect("the indented block is an anchored row");
    assert_eq!(
        row.fallback_status,
        transync::FallbackStatus::FallbackSource,
        "a provider-refused unit settles at fallback_source",
    );

    let record = output
        .validation_report
        .per_unit
        .iter()
        .find(|r| r.unit_id.0 == "c-0002")
        .expect("the indented block has an attempt log");
    assert_eq!(
        record.attempts.len(),
        3,
        "one dispatch + the default two validation retries: {:?}",
        record.attempts,
    );

    assert_eq!(
        output.translated_document, SRC,
        "fallback splices the source bytes verbatim — indent included",
    );
}
