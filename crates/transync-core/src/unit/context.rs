//! BlockContext composition: section path + neighbor snippets.
//!
//! TRACE: SCN-09

use crate::id::{BlockId, BlockKind};
use crate::llm::{BlockContext, HeadingSnippet, NeighborSnippet};
use crate::parser::{Block, Document};
use std::collections::HashMap;

const NEIGHBOR_SUMMARY_LEN: usize = 120;

/// Document-wide lookups shared by every `build_context` call.
///
/// R0006-0029 / R0006-0030: the heading snippets and the document title
/// are identical for every unit, so they are computed once per
/// document (in `unit::build_batches`) instead of once per unit. Since
/// `R0001-0022` (`reviews/0001.md`) that matters more than it did: a snippet
/// costs a Markdown parse (`heading_plain_text`), so re-deriving one per
/// section-path entry per unit would reparse the same headings thousands of
/// times in a large document.
pub struct ContextIndex<'a> {
    /// Heading id → the snippet a `section_path` entry for it gets. Only
    /// headings are indexed: a `section_path` element that is not a
    /// heading cannot exist (`parser::sections` stamps heading ids only),
    /// and no other lookup reads the map.
    headings: HashMap<&'a BlockId, HeadingSnippet>,
    document_title: Option<String>,
}

impl ContextIndex<'_> {
    /// The plain text of the heading with this id, or `None` when the id is
    /// not a heading of this document.
    ///
    /// DCR-0027's section partition needs it: a heading's own `section_path`
    /// excludes itself, so building the section's heading **stack** — the
    /// identity glossary selectors are matched against — means appending the
    /// opening heading's own text to its ancestors'. Reading it from this map
    /// rather than re-deriving it is what keeps the opener and the ancestors
    /// spelled the same way; `heading_plain_text` also costs a Markdown parse,
    /// and this map already paid it once per heading.
    ///
    /// TRACE: DCR-0027
    pub(crate) fn heading_text(&self, id: &BlockId) -> Option<&str> {
        self.headings.get(id).map(|h| h.text.as_str())
    }
}

/// Build the shared per-document context index: one snippet per heading id,
/// so a `section_path` hop is a lookup instead of a rescan of `doc.blocks`
/// (`R0001-0042` in the removed `reviews/reviewed/0001.md` — an earlier round
/// than today's `reviews/0001.md`, which spells that id differently).
pub fn build_index(doc: &Document) -> ContextIndex<'_> {
    let headings: HashMap<&BlockId, HeadingSnippet> = doc
        .blocks
        .iter()
        .filter_map(|b| {
            b.kind.heading_level().map(|level| {
                (
                    &b.block_id,
                    HeadingSnippet {
                        level,
                        text: heading_plain_text(doc, b),
                    },
                )
            })
        })
        .collect();
    ContextIndex {
        headings,
        document_title: document_title(doc),
    }
}

/// The document's title: the plain text of its **`Title` block** when it has
/// one, else of its **first level-1 heading**, else `None`.
///
/// The `Title` preference is explicit, and it has to be (ti 490d97 wave 2):
/// [`BlockKind::Title`]'s `heading_level()` is `None` — a page title is not a
/// heading level — so "the first block with a heading level" would never find
/// it, and this `.find` matched `Heading1` and nothing else before the arm
/// below existed. A Markdown document has no `Title` block, so its answer is
/// unchanged, which is what `without_a_title_block_the_first_h1_still_wins`
/// pins.
///
/// This is the one place the title rule lives. The provider reads it as
/// [`BlockContext::document_title`] on every unit, and the pipeline hands the
/// same value to its caller on `TranslationOutput` so a front end (the CLI's
/// HTML bundle, ti 0f26b5) titles the document with what the model was told it
/// is called — one heading, one rendering of it, wherever it surfaces.
///
/// Rendering it costs one extra Comrak parse of one heading per **document**,
/// which is why [`build_index`] calls this instead of reading its own snippet
/// map: the per-unit cost R0006-0029 / `R0001-0042` in the removed
/// `reviews/reviewed/0001.md` care about is untouched, and a second
/// expression of "first H1, as prose" would be a rule that can drift.
///
/// The text is what the parser left after consuming markers and inline
/// delimiters, so it is prose, not source — but it is *untrusted* prose
/// (invariant 7) and it may be empty (`#` alone). A consumer that renders it
/// escapes it and decides for itself what an empty title means.
pub(crate) fn document_title(doc: &Document) -> Option<String> {
    doc.blocks
        .iter()
        .find(|b| matches!(b.kind, BlockKind::Title))
        .or_else(|| {
            doc.blocks
                .iter()
                .find(|b| matches!(b.kind, BlockKind::Heading1))
        })
        .map(|b| heading_plain_text(doc, b))
}

/// Build the `BlockContext` for a block at a given index.
///
/// Populates `section_path` from the surrounding heading scopes, and
/// captures preceding/following block snippets so the provider has enough
/// context to translate idioms accurately.
///
/// TRACE: SCN-09
pub fn build_context(doc: &Document, block_index: usize, index: &ContextIndex<'_>) -> BlockContext {
    let block = match doc.blocks.get(block_index) {
        Some(b) => b,
        None => return BlockContext::default(),
    };

    let section_path = block
        .section_path
        .iter()
        .filter_map(|id| index.headings.get(id).cloned())
        .collect();

    let preceding_block = block_index
        .checked_sub(1)
        .and_then(|i| doc.blocks.get(i))
        .map(|b| neighbor(b, doc));
    let following_block = doc.blocks.get(block_index + 1).map(|b| neighbor(b, doc));

    BlockContext {
        section_path,
        preceding_block,
        following_block,
        document_title: index.document_title.clone(),
    }
}

/// The heading's own text, as prose: every structural marker removed by the
/// parser, every content byte kept.
///
/// The markers are the parser's to recognize, not a trim's. An ATX opener,
/// an ATX closing sequence, a setext `====`/`----` underline and every inline
/// delimiter are syntax Comrak consumes, so walking the reparsed heading's
/// inline descendants removes all of them and nothing else — while a `#` that
/// is *content* survives untouched, because the parser never saw it as a
/// marker. That this snippet and [`document_title`] must both read as prose
/// is the older ask of `R0001-0041` / `R0001-0098` in the removed
/// `reviews/reviewed/0001.md`; that round numbered its findings up to 0100,
/// so neither id is the one today's `reviews/0001.md` carries under the same
/// spelling.
///
/// `R0001-0022` / `R0001-0023` (`reviews/0001.md`): this used to be
/// `body.trim_start_matches('#').trim()` over the source slice, which
/// deleted content hashes (a setext heading reading `#hashtag` reached the
/// provider as `hashtag`), kept the closing ATX marker (`# Foo #` → `Foo #`),
/// and left emphasis, links and code spans in a string this comment called
/// clean prose. Both effects also moved `pipeline::context_hash`, so they
/// changed cache identity as well as what the model read.
///
/// ti 490d97 wave 5 (spec §6): an **Html-spelled** block's plain text is
/// `transync_html::extract` over its source slice — texts joined with one
/// space, trimmed. The one existing HTML opinion, entity-decoded, spelled
/// exactly as the model sees the wire segments (the DCR-0027 property: the
/// glossary's section selectors and the prompt's section_path must read the
/// heading the same way). Running comrak here instead collapses every
/// Html-spelled heading to the empty string — an `HtmlBlock` is a leaf with
/// no inline children to walk — so the heading stacks say nothing and
/// context identity degrades. The branch lives HERE —
/// inside the one function `build_index`'s headings map is built from — so
/// the map, `section_path`, and `document_title` cannot hold three
/// projections. Extraction failure degrades to the empty string, exactly
/// what the comrak arm yields for a fragment it cannot read: a heading
/// context that says nothing rather than one that lies in markup.
fn heading_plain_text(doc: &Document, b: &Block) -> String {
    match b.spelling {
        crate::id::Spelling::Markdown => inline_plain_text(&text_from_range(doc, b), &doc.ref_defs),
        crate::id::Spelling::Html { .. } => transync_html::extract(&text_from_range(doc, b))
            .map(|segs| segs.texts.join(" ").trim().to_string())
            .unwrap_or_default(),
    }
}

/// Flatten one block-level Markdown fragment to the plain text of its
/// inlines: literal text, code-span literals, and a space for each line
/// break. Emphasis/link/image wrappers contribute their own text through the
/// descendant walk; raw inline HTML tags and footnote references are markup,
/// not prose, so they contribute nothing.
///
/// `ref_defs` is [`Document::ref_defs`], appended before the parse for the
/// same reason `validate::inline` appends it: a link-reference definition
/// lives at document level, so `[text][ref]` in an isolated fragment would
/// otherwise stay unresolved and leak its brackets into the "plain" text.
/// Definitions produce no nodes of their own, and only the fragment's own
/// first block is walked, so the pool can never contribute text.
fn inline_plain_text(fragment: &str, ref_defs: &str) -> String {
    use comrak::nodes::NodeValue;

    let arena = comrak::Arena::new();
    let opts = crate::parser::comrak_options();
    let appended;
    let to_parse: &str = if ref_defs.is_empty() {
        fragment
    } else {
        appended = format!("{fragment}\n\n{ref_defs}");
        appended.as_str()
    };
    let root = comrak::parse_document(&arena, to_parse, &opts);

    let Some(first_block) = root.children().next() else {
        return String::new();
    };
    let mut out = String::new();
    for node in first_block.descendants() {
        match &node.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            _ => {}
        }
    }
    out.trim().to_string()
}

fn neighbor(b: &Block, doc: &Document) -> NeighborSnippet {
    NeighborSnippet {
        kind: b.kind.clone(),
        summary: text_from_range_truncated(doc, b, NEIGHBOR_SUMMARY_LEN),
    }
}

fn text_from_range(doc: &Document, block: &Block) -> String {
    let mut start = block.source_range.start.min(doc.source_text.len());
    let mut end = block.source_range.end.min(doc.source_text.len()).max(start);
    // R0006-0031: same defensive UTF-8 char-boundary snap as
    // `transync_syntax::outcome::block_payload` — a mid-char sourcepos
    // offset must degrade to a shorter snippet, never panic the slice.
    while start < doc.source_text.len() && !doc.source_text.is_char_boundary(start) {
        start += 1;
    }
    end = end.max(start);
    while end > start && !doc.source_text.is_char_boundary(end) {
        end -= 1;
    }
    doc.source_text[start..end].trim().to_string()
}

fn text_from_range_truncated(doc: &Document, block: &Block, max_chars: usize) -> String {
    let raw = text_from_range(doc, block);
    if raw.chars().count() <= max_chars {
        raw
    } else {
        raw.chars().take(max_chars).collect()
    }
}

/// R0001-0022 / R0001-0023: heading context is the parsed heading's inline
/// text. Markers vanish because the parser consumed them; content that looks
/// like a marker survives because it never was one.
#[cfg(test)]
mod heading_text_tests {
    use super::*;

    /// The snippet the first heading of `md` contributes to a section path.
    fn heading_text(md: &str) -> String {
        let doc = crate::parser::parse(md).expect("parses");
        let b = doc
            .blocks
            .iter()
            .find(|b| b.kind.heading_level().is_some())
            .expect("a heading block");
        heading_plain_text(&doc, b)
    }

    #[test]
    fn setext_heading_keeps_a_leading_content_hash() {
        // R0001-0022's defect, with the repro that actually triggers it: no
        // ATX marker is present at all, so `trim_start_matches('#')` ate a
        // content byte and the provider read `hashtag`.
        assert_eq!(heading_text("#hashtag\n========\n"), "#hashtag");
    }

    #[test]
    fn atx_heading_keeps_a_content_hash_after_the_marker() {
        assert_eq!(heading_text("# #hashtag\n"), "#hashtag");
    }

    #[test]
    fn atx_closing_marker_is_removed() {
        // R0001-0023: the trim only looked at the front, so `Foo #` shipped.
        assert_eq!(heading_text("# Foo #\n"), "Foo");
        assert_eq!(heading_text("### Deep ###\n"), "Deep");
    }

    #[test]
    fn inline_syntax_is_flattened_to_prose() {
        // R0001-0023: emphasis, link and code-span syntax all survived the
        // trim verbatim, contradicting the function's own contract.
        assert_eq!(
            heading_text("# **Bold** [Docs](https://example.com) and `code_span()`\n"),
            "Bold Docs and code_span()",
        );
        assert_eq!(heading_text("## ~~Old~~ *new* name\n"), "Old new name");
        assert_eq!(heading_text("## ![Logo](l.png) Title\n"), "Logo Title");
    }

    #[test]
    fn inline_raw_html_tags_are_markup_not_prose() {
        assert_eq!(heading_text("# Press <kbd>X</kbd> now\n"), "Press X now");
    }

    #[test]
    fn setext_underline_is_not_part_of_the_snippet() {
        // The case the pre-R0001-0022 implementation already handled by
        // dropping the last line; it is structural now, not a string rule.
        assert_eq!(heading_text("Overview\n========\n"), "Overview");
        assert_eq!(heading_text("Sub heading\n-----------\n"), "Sub heading");
    }

    #[test]
    fn multi_line_setext_heading_joins_on_the_line_break() {
        assert_eq!(
            heading_text("Long heading\ncontinued here\n==============\n"),
            "Long heading continued here",
        );
    }

    #[test]
    fn reference_link_in_a_heading_resolves_through_the_document_pool() {
        // The definition lives at document level, so an isolated fragment
        // parse would leave `[the docs][d]` bracketed in "plain" text.
        assert_eq!(
            heading_text("# See [the docs][d]\n\nbody\n\n[d]: https://example.com\n"),
            "See the docs",
        );
    }

    #[test]
    fn content_bytes_survive_byte_for_byte() {
        assert_eq!(
            heading_text("# 한국어 제목 — 100% 준비\n"),
            "한국어 제목 — 100% 준비",
        );
    }

    #[test]
    fn degenerate_fragment_yields_an_empty_snippet() {
        assert_eq!(inline_plain_text("", ""), "");
        assert_eq!(inline_plain_text("   \n", ""), "");
    }
}

/// The document title and every section-path entry come from the same
/// extraction — one heading, one rendering of it, wherever it surfaces.
#[cfg(test)]
mod context_index_tests {
    use super::*;

    const DOC: &str = "\
# The `transync` **Guide** #

intro

Design *notes*
--------------

body paragraph
";

    /// Index of the block whose source excerpt is exactly `text`.
    fn block_at(doc: &Document, text: &str) -> usize {
        doc.blocks
            .iter()
            .position(|b| text_from_range(doc, b) == text)
            .unwrap_or_else(|| panic!("no block reading {text:?}"))
    }

    #[test]
    fn document_title_uses_the_same_extraction_as_section_path() {
        let doc = crate::parser::parse(DOC).expect("parses");
        let index = build_index(&doc);
        let ctx = build_context(&doc, block_at(&doc, "body paragraph"), &index);

        assert_eq!(ctx.document_title.as_deref(), Some("The transync Guide"));
        let path: Vec<(u8, &str)> = ctx
            .section_path
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(
            path,
            vec![(1, "The transync Guide"), (2, "Design notes")],
            "both heading styles reach the provider as prose, with levels intact",
        );
    }

    /// The rule is the *first* level-1 heading and nothing else: a document
    /// that opens with prose is still titled by the H1 further down, a second
    /// H1 does not retitle it, and a document whose only headings are deeper
    /// has no title at all (its consumer supplies its own fallback — ti
    /// 0f26b5).
    #[test]
    fn the_title_is_the_first_level_1_heading_and_only_that() {
        let doc = crate::parser::parse("intro\n\n# First\n\nbody\n\n# Second\n").expect("parses");
        assert_eq!(document_title(&doc).as_deref(), Some("First"));

        let deep = crate::parser::parse("## Only a subheading\n\nbody\n").expect("parses");
        assert_eq!(document_title(&deep), None);

        let empty = crate::parser::parse("body only\n").expect("parses");
        assert_eq!(document_title(&empty), None);
    }

    /// An empty title and no title are two different states, and both are
    /// reachable from real source: a bare `#` is a level-1 heading whose
    /// text is empty, so the rule yields `Some("")`. They stay distinct all
    /// the way out — the user prompt sends `"document_title":""` for one and
    /// omits the field for the other, and `pipeline::context_hash` gives
    /// them different cache identities (ti 53d495 backstop).
    #[test]
    fn a_bare_hash_is_an_empty_title_not_an_absent_one() {
        let doc = crate::parser::parse("#\n\nbody\n").expect("parses");
        assert!(
            matches!(doc.blocks[0].kind, BlockKind::Heading1),
            "a bare `#` parses as a level-1 heading"
        );
        assert_eq!(document_title(&doc).as_deref(), Some(""));

        let index = build_index(&doc);
        let ctx = build_context(&doc, block_at(&doc, "body"), &index);
        assert_eq!(ctx.document_title.as_deref(), Some(""));
        assert_eq!(
            ctx.section_path
                .iter()
                .map(|h| (h.level, h.text.as_str()))
                .collect::<Vec<_>>(),
            vec![(1, "")],
            "the untitled heading still scopes the block it encloses"
        );
    }

    #[test]
    fn neighbor_summaries_stay_verbatim_source() {
        // Deliberate contrast: a neighbor snippet is a source excerpt of any
        // block kind, paired with its `kind`. Only heading *context* is prose.
        let doc = crate::parser::parse(DOC).expect("parses");
        let index = build_index(&doc);
        let ctx = build_context(&doc, block_at(&doc, "intro"), &index);
        assert_eq!(
            ctx.preceding_block.map(|n| n.summary),
            Some("# The `transync` **Guide** #".to_string()),
        );
    }
}

// ti 490d97 wave 2 (spec §3 / §6): `document_title` matched `BlockKind::Heading1`
// and nothing else — not "the first block with a heading level" — so a `Title`
// block does NOT enter the document title for free, and extending it is
// explicit work. These pin the selection rule; the TEXT projection for a real
// `<title>…</title>` slice is wave 5's, and the third test says so out loud
// rather than leaving the gap to be discovered.
#[cfg(test)]
mod document_title_tests {
    use super::*;
    use crate::id::{SourceFormat, Spelling};
    use crate::parser::AstPath;
    use crate::parser::ranges::ByteRange;

    /// A hand-built HTML document: a `Title` block over the first line, an H1
    /// over the last, gap in between. Hand-built because no intake emits a
    /// `Title` until wave 3, and the H1 is there so the test can tell the new
    /// rule from the old one.
    fn hand_built_titled_document(title_slice: &str) -> Document {
        let source_text = format!("{title_slice}\n\n# Heading one\n");
        let h1_start = title_slice.len() + 2;
        let end = source_text.len();
        Document {
            source_text,
            format: SourceFormat::Html,
            blocks: vec![
                Block {
                    block_id: BlockId::new("title", 1),
                    kind: BlockKind::Title,
                    spelling: Spelling::Html { block_type: None },
                    source_range: ByteRange {
                        start: 0,
                        end: title_slice.len(),
                    },
                    source_hash: 0,
                    section_path: Vec::new(),
                    ast_path: AstPath(vec![0]),
                },
                Block {
                    block_id: BlockId::new("h1", 2),
                    kind: BlockKind::Heading1,
                    spelling: Spelling::Html { block_type: None },
                    source_range: ByteRange {
                        start: h1_start,
                        end,
                    },
                    source_hash: 0,
                    section_path: Vec::new(),
                    ast_path: AstPath(vec![1]),
                },
            ],
            ..Document::default()
        }
    }

    #[test]
    fn a_title_block_wins_the_document_title_over_the_first_h1() {
        let doc = hand_built_titled_document("Doc name");
        assert_eq!(
            document_title(&doc).as_deref(),
            Some("Doc name"),
            "D5 / spec §6: an HTML run's document title is its <title>, not \
             the first heading the page happens to contain",
        );
    }

    #[test]
    fn without_a_title_block_the_first_h1_still_wins() {
        // The Markdown rule, unchanged — this is the regression guard on the
        // half that must not move.
        let doc =
            crate::parser::parse("intro\n\n# Heading one\n\n# Heading two\n").expect("parses");
        assert_eq!(document_title(&doc).as_deref(), Some("Heading one"));
    }

    /// Wave 2 pinned this exact case at `Some("")` — `heading_plain_text` ran
    /// comrak over the slice, comrak saw one HtmlBlock node with no inline
    /// children, and a real <title> element projected to empty prose. Wave 5
    /// replaces the projection for Html-spelled blocks with
    /// `transync_html::extract` (spec §6): the one existing HTML opinion,
    /// entity-decoded, spelled exactly as the model sees the wire segments.
    #[test]
    fn a_real_title_element_projects_to_its_extracted_text() {
        let doc = hand_built_titled_document("<title>Doc name</title>");
        assert_eq!(document_title(&doc).as_deref(), Some("Doc name"));
    }

    /// §6: an HTML <h1>'s heading snippet is extracted plain text. Before
    /// this wave the comrak projection COLLAPSED it instead: `<h1 …>` is a
    /// type-6 HTML block, one leaf HtmlBlock node with no inline children
    /// to walk, so every Html-spelled heading projected to "" — the same
    /// mechanism wave 2's title pin records. The hazard is collapse, not
    /// markup leakage: distinct headings dedupe to the same empty
    /// section_path entry, the prompt loses the section story, and the
    /// section_path half of context identity goes silent (neighbor
    /// snippets, verbatim by design, were the only place a heading still
    /// spoke). Entity-decoded, because that is how the model sees the wire
    /// segments (DCR-0027's spelled-the-same property).
    #[test]
    fn an_html_headings_snippet_is_extracted_prose_not_tags() {
        let mut doc = transync_syntax::intake::html::parse(
            "<h1 id=\"top\">Anchors &amp; panes</h1>\n<p>body text</p>\n",
        );
        crate::id::assign_block_ids(&mut doc);
        let index = build_index(&doc);
        let ctx = build_context(&doc, 1, &index);
        let path: Vec<(u8, &str)> = ctx
            .section_path
            .iter()
            .map(|h| (h.level, h.text.as_str()))
            .collect();
        assert_eq!(
            path,
            vec![(1, "Anchors & panes")],
            "extracted, entity-decoded prose — no '<', no '&amp;'",
        );
        assert_eq!(
            ctx.document_title.as_deref(),
            Some("Anchors & panes"),
            "no Title block, so the first heading wins, through the same projection",
        );
    }

    /// Final confirmation ii (spec §6), the recorded REJECTION: neighbor
    /// snippets stay verbatim source excerpts for both formats. Projecting
    /// them through `extract` was proposed and rejected — it would break the
    /// Markdown pin above this module for a prompt-nicety, and a Markdown
    /// neighbor summary is already raw source markup on the same terms. Do
    /// not re-propose it; this test is the tombstone.
    #[test]
    fn an_html_neighbor_summary_stays_verbatim_source() {
        let mut doc = transync_syntax::intake::html::parse(
            "<h1 id=\"top\">Anchors &amp; panes</h1>\n<p>body text</p>\n",
        );
        crate::id::assign_block_ids(&mut doc);
        let index = build_index(&doc);
        let ctx = build_context(&doc, 1, &index);
        assert_eq!(
            ctx.preceding_block.map(|n| n.summary),
            Some("<h1 id=\"top\">Anchors &amp; panes</h1>".to_string()),
            "verbatim markup, tags and entity spelling included — deliberately \
             NOT the extracted prose the heading stack carries",
        );
    }
}
