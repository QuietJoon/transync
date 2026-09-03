//! Wave 3's spine (spec 2026-08-20 §12): an HTML document goes in and comes
//! out byte-identical, with a real block set and real ids — no LLM, no
//! pipeline, no core. `regen::regenerate` copies inter-block gaps verbatim
//! and splices `source_text[range]` for every block absent from the accepted
//! map, so byte-identity over `&HashMap::new()` is precisely the statement
//! that the intake's ranges are in source order, non-overlapping and in
//! bounds — the three invariants the intake debug-asserts.
//!
//! A byte-identity assertion ALONE is unfalsifiable against the obvious
//! stub: a `parse` that finds zero blocks makes `regenerate` reproduce the
//! whole source as one gap, trivially. Every test here therefore pairs the
//! round trip with a block-set (kind-sequence) assertion in the same test;
//! the pair is the falsifiable unit. Do not add identity-only tests.

use std::collections::HashMap;
use transync_syntax::id::{SourceFormat, Spelling};
use transync_syntax::parser::Document;
use transync_syntax::{intake, regen};

/// The spec-mandated fixture (§11 row 1). Its canonical home is the SCN
/// suite — wave 5's scenario test consumes the SAME file — so this crate
/// reaches it relatively rather than keeping a copy that could drift.
const SCN_16: &str = include_str!("../../transync/tests/fixtures/scn-16-html-document.html");
const MARKETING: &str = include_str!("fixtures/html-corpus/marketing-page.html");
const DOCS_FRAGMENT: &str = include_str!("fixtures/html-corpus/docs-fragment.html");

/// Inline, never checked-in files: a lone CR, a BOM and a NUL are exactly
/// the bytes an editor, a filter, or an EOL-normalizing tool is liable to
/// mangle on the way to disk (the OI-0033 fixture precedent), and the test
/// would then pass vacuously.
const CRLF_PAGE: &str = "<div>\r\n<p>CRLF line one</p>\r\n<p>CRLF line two</p>\r\n</div>\r\n";
const LONE_CR_PAGE: &str = "<p>alpha</p>\r<p>bravo</p>\r";
const BOM_PAGE: &str = "\u{feff}<p>after the BOM</p>\n";
const DOCTYPE_ONLY: &str = "<!DOCTYPE html>\n";
const PLAIN_TEXT: &str = "Just words, no tags at all.\n";

/// Parse, regenerate against the empty map, assert byte-identity, and hand
/// the document back for block-set assertions — which every caller MUST
/// make: identity alone passes on a zero-block stub (see the module doc).
fn round_trip(name: &str, source: &str) -> Document {
    let doc = intake::html::parse(source);
    let (out, offsets) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(
        out, doc.source_text,
        "{name}: regenerate(parse_html(doc), &empty) must be byte-identical"
    );
    assert_eq!(
        offsets.0.len(),
        doc.blocks.len(),
        "{name}: one offset per block"
    );
    doc
}

fn kinds(doc: &Document) -> Vec<&'static str> {
    doc.blocks.iter().map(|b| b.kind.wire_str()).collect()
}

#[test]
fn the_scn_16_fixture_round_trips_byte_identical_with_the_designed_block_set() {
    let doc = round_trip("scn-16", SCN_16);
    assert_eq!(
        doc.source_text, SCN_16,
        "no NUL, so parse must not copy-modify"
    );
    assert_eq!(
        kinds(&doc),
        vec![
            "title",
            "heading-1",
            "list-item",
            "list-item",
            "heading-2",
            "paragraph",
            "image",
            "blockquote",
            "heading-2",
            "table",
            "code-block",
            "thematic-break",
            "html",
            "paragraph",
        ],
    );
    let ids: Vec<&str> = doc.blocks.iter().map(|b| b.block_id.0.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "title-0001",
            "h1-0002",
            "li-0003",
            "li-0004",
            "h2-0005",
            "p-0006",
            "img-0007",
            "q-0008",
            "h2-0009",
            "t-0010",
            "c-0011",
            "hr-0012",
            "html-0013",
            "p-0014",
        ],
        "emission-order stamping with the one global ordinal (spec §4)",
    );
    assert_eq!(doc.format, SourceFormat::Html);
    assert!(
        doc.blocks
            .iter()
            .all(|b| matches!(b.spelling, Spelling::Html { block_type: None })),
        "format == Html implies every spelling is Html {{ block_type: None }} (spec §3)",
    );
    assert_eq!(
        doc.warnings.len(),
        1,
        "exactly the unclosed <p>: {:?}",
        doc.warnings
    );
    assert!(doc.warnings[0].contains("p-0014"), "{}", doc.warnings[0]);
    // §11's pinned case: two adjacent <img> tags are ONE Image block.
    let img = &doc.blocks[6];
    let payload = &doc.source_text[img.source_range.start..img.source_range.end];
    assert_eq!(
        payload,
        "<img src=\"badge-build.svg\" alt=\"build\"> <img src=\"badge-docs.svg\" alt=\"docs\">",
    );
}

#[test]
fn a_messy_marketing_page_round_trips_and_the_pairing_discipline_owns_its_imgs() {
    let doc = round_trip("marketing-page", MARKETING);
    assert_eq!(
        kinds(&doc),
        vec![
            "title",
            "heading-1",
            "paragraph",
            "paragraph",
            "paragraph",
            "html",
            "html",
            "list-item",
            "list-item",
            "html",
            "html",
            "html",
            "paragraph",
            "paragraph",
        ],
    );
    // The img pair follows an UNCLOSED <p>, so the extents put both imgs
    // INSIDE that paragraph — rule T's exception never sees them, and no
    // Image block may exist in this document.
    assert!(!kinds(&doc).contains(&"image"));
    let p2 = &doc.blocks[3];
    let payload = &doc.source_text[p2.source_range.start..p2.source_range.end];
    assert!(payload.starts_with("<p>No reparse"), "{payload}");
    assert!(payload.contains("src=two.png"), "{payload}");
    assert_eq!(
        doc.warnings.len(),
        4,
        "two implicit <p> closes + two implicit <li> closes: {:?}",
        doc.warnings,
    );
}

#[test]
fn an_unclosed_fragment_force_closes_with_warnings_and_still_round_trips() {
    let doc = round_trip("docs-fragment", DOCS_FRAGMENT);
    assert_eq!(
        kinds(&doc),
        vec![
            "heading-1",
            "paragraph",
            "list-item",
            "list-item",
            "list-item",
            "code-block",
            "html",
            "html",
            "paragraph",
        ],
    );
    use transync_syntax::id::BlockKind;
    assert!(
        doc.blocks[2..5].iter().all(|b| matches!(
            b.kind,
            BlockKind::ListItem {
                ordered: true,
                task: None
            }
        )),
        "an <ol>'s items are ordered (D9)",
    );
    // Three implicit <li> closes, the EOF-force-closed <p>, and the two
    // containers (<section>, <div>) still open at end of input.
    assert_eq!(doc.warnings.len(), 6, "{:?}", doc.warnings);
    assert!(doc.warnings.iter().any(|w| w.contains("end of input")));
}

#[test]
fn edge_documents_round_trip() {
    let doc = round_trip("crlf", CRLF_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"]);

    // Spans are direct byte offsets — no line table exists on this path, so
    // the lone-CR sourcepos hazard (OI-0033) structurally cannot recur.
    let doc = round_trip("lone-cr", LONE_CR_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"]);

    let doc = round_trip("bom", BOM_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph"]);
    assert_eq!(
        doc.blocks[0].source_range.start,
        "\u{feff}".len(),
        "the BOM stays in the inter-block gap, exactly as on the Markdown side",
    );

    let doc = round_trip("doctype-only", DOCTYPE_ONLY);
    assert!(
        doc.blocks.is_empty(),
        "a doctype-only gap mints NO block (spec §12 wave 3): {:?}",
        doc.blocks,
    );

    let doc = round_trip("empty", "");
    assert!(doc.blocks.is_empty());

    let doc = round_trip("plain-text", PLAIN_TEXT);
    assert_eq!(
        kinds(&doc),
        vec!["paragraph"],
        "a tagless file is one text-heavy block set — D8's explicitly-requested shape",
    );
}

#[test]
fn a_nul_bearing_document_round_trips_to_its_normalized_self() {
    let src = "<p>alpha\0bravo</p>\n";
    let doc = intake::html::parse(src);
    assert_eq!(
        doc.source_text, "<p>alpha\u{fffd}bravo</p>\n",
        "the SAME normalize_source contract as the Markdown intake (spec §4)",
    );
    let (out, _) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(
        out, doc.source_text,
        "identity is over Document::source_text"
    );
}
