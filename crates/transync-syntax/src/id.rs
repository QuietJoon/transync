//! Stable block identifier and block-kind enum.
//!
//! TRACE: ADR-0005
//! TRACE: ADR-0001

use serde::{Deserialize, Serialize};
use siphasher::sip::SipHasher13;
use std::hash::Hasher;

/// Stable, kind-prefixed sequential ID — `<kind>-<NNNN>`.
///
/// Format and constraints are documented in ADR-0005. The format is
/// stable across the entire pipeline (Rust IR → LLM contract → alignment
/// map → DOM `data-sync-id` attribute).
///
/// TRACE: ADR-0005
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlockId(pub String);

impl BlockId {
    /// Compose an ID from kind code + ordinal. Zero-pads to four digits.
    ///
    /// TRACE: ADR-0005
    pub fn new(kind_code: &str, ordinal: u32) -> Self {
        Self(format!("{kind_code}-{ordinal:04}"))
    }
}

impl std::fmt::Display for BlockId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// GFM block-kind enum. Wire form (kebab-case) is the same as
/// `docs/architecture/contracts.md` §3.
///
/// TRACE: SCN-01..SCN-06
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum BlockKind {
    #[serde(rename = "heading-1")]
    Heading1,
    #[serde(rename = "heading-2")]
    Heading2,
    #[serde(rename = "heading-3")]
    Heading3,
    #[serde(rename = "heading-4")]
    Heading4,
    #[serde(rename = "heading-5")]
    Heading5,
    #[serde(rename = "heading-6")]
    Heading6,
    Paragraph,
    Table,
    /// A code block, either spelled fenced (```` ``` ````/`~~~`) or spelled
    /// with a four-column indent.
    CodeBlock {
        /// The fence's info string, or `None` for a bare fence and for every
        /// indented block (which cannot carry one).
        info: Option<String>,
        /// How the SOURCE spelled the block. `false` means an indented
        /// (four-column) block, whose `source_range` is moved back over the
        /// block-structure indent so the slice carries the block's complete
        /// indented spelling, and whose wire payload
        /// `unit::payload::assemble` therefore has to synthesize as a fenced
        /// block. Deliberately **not** `#[serde(default)]`: no shipped
        /// artifact serializes this enum (alignment rows and cache keys carry
        /// [`BlockKind::wire_str`] strings, and `TranslationUnit` is not
        /// `Serialize`), so a loud deserialize failure on an old payload beats
        /// a silent wrong guess about the source's spelling.
        fenced: bool,
    },
    ListItem {
        ordered: bool,
        task: Option<bool>,
    },
    Blockquote,
    ThematicBreak,
    Image,
    /// An HTML document's `<title>` — real translatable content that is
    /// **not page content** (decision D5).
    ///
    /// It gets a real block, a real translation unit and a real alignment row,
    /// and it gets **no DOM anchor**: the browser chrome renders it, not the
    /// pane, so there is nothing in the pane to anchor. Its row therefore
    /// carries `sync_role: "non-sync"` — the shape a thematic break has had
    /// since schema 1.0 — which is why `align::sync_role_for` has an
    /// **explicit** arm for this variant. It had to be written by hand when
    /// that function still ended in `_ => SyncRole::Anchor`, which would have
    /// answered the opposite; ti `9ffb97` made the function exhaustive, so a
    /// variant added after this one cannot inherit a role nobody chose. This amends architectural invariant 1:
    /// the DOM-anchor leg of the chain is conditional on being page content,
    /// not on the kind.
    ///
    /// `heading_level()` is `None`: a page title is not a heading level, it
    /// opens no section scope, and `partition_by_section` puts it in the
    /// preamble section — correct, since no glossary section selector can
    /// mean a page title. `unit::context::document_title` prefers it over the
    /// first `Heading1` by an explicit arm, for the same reason.
    ///
    /// TRACE: ADR-0025
    Title,
    /// HTML content with **no semantic equivalent** — a `div`, a custom
    /// element, an unknown or future tag. Translatable via segment extraction
    /// (ADR-0018).
    ///
    /// The variant carried `block_type: u8` until ti `490d97` wave 2. That
    /// field was 100 % spelling metadata — its only consumers were splice
    /// normalization and `HtmlSegmentConstraints` — so it moved to
    /// [`Spelling::Html`], leaving this variant with one meaning instead of
    /// two ("no semantic classification" *and* "spelled as HTML").
    ///
    /// On the Markdown path every raw-HTML island still lands here, and that
    /// is deliberate (decision D11): reclassifying `html-0007` to `t-0007`
    /// would move the block id, and with it the alignment row, the DOM anchor
    /// and the `block_kind` cache axis. The narrowed meaning binds the HTML
    /// intake, where a `<table>` really is a [`BlockKind::Table`].
    Html,
    /// A top-level source node the pipeline does not model as a translatable
    /// kind (footnote definition, front matter, …). Never a
    /// translation unit; regen splices its source bytes verbatim; render
    /// emits an escaped, inert placeholder anchored in both panes
    /// (invariant 7).
    Skipped {
        /// Machine label of the underlying node: "footnote-definition",
        /// "front-matter", or "unsupported". `"html-block"` is NOT among
        /// them since the 2026-08-03 spec — raw HTML is [`BlockKind::Html`]
        /// now, and `parser::classify`'s label table can no longer return
        /// it. The literal string survives only as the `data-skipped` attribute
        /// value the renderer stamps on a *degraded* html block's escaped
        /// placeholder, which is not this field.
        label: String,
    },
}

impl BlockKind {
    /// Returns the kind-prefix code used in [`BlockId`] strings.
    ///
    /// TRACE: ADR-0005
    pub fn id_code(&self) -> &'static str {
        match self {
            BlockKind::Heading1 => "h1",
            BlockKind::Heading2 => "h2",
            BlockKind::Heading3 => "h3",
            BlockKind::Heading4 => "h4",
            BlockKind::Heading5 => "h5",
            BlockKind::Heading6 => "h6",
            BlockKind::Paragraph => "p",
            BlockKind::Table => "t",
            BlockKind::CodeBlock { .. } => "c",
            BlockKind::ListItem { .. } => "li",
            BlockKind::Blockquote => "q",
            BlockKind::ThematicBreak => "hr",
            BlockKind::Image => "img",
            // An HTML document's page title. No collision with h1..h6, p, t,
            // c, li, q, hr, img, html, x.
            BlockKind::Title => "title",
            // Raw HTML blocks get their own `html` prefix — no collision with
            // h1..h6, p, t, c, li, q, hr, img, x.
            BlockKind::Html => "html",
            // Skipped nodes get their own `x` prefix — no collision with
            // h1..h6, p, t, c, li, q, hr, img (A1).
            BlockKind::Skipped { .. } => "x",
        }
    }

    /// Heading depth 1–6, or `None` for every non-heading kind.
    ///
    /// THE kind→level mapping. It previously existed twice — once in
    /// `parser::build_hierarchy`'s private helper and once as a byte-copy in
    /// `transync-core`'s `unit::context`, across a crate boundary where no
    /// compiler check could catch a drift. It lives beside [`id_code`] and
    /// [`wire_str`] because it is the same thing they are: a projection of
    /// the kind enum that must be extended in lockstep when a variant is
    /// added.
    ///
    /// [`id_code`]: BlockKind::id_code
    /// [`wire_str`]: BlockKind::wire_str
    ///
    /// `#[doc(hidden)]`: workspace-internal helper, not curated public
    /// surface — the facade does not re-export it (contracts.md §0).
    #[doc(hidden)]
    pub fn heading_level(&self) -> Option<u8> {
        match self {
            BlockKind::Heading1 => Some(1),
            BlockKind::Heading2 => Some(2),
            BlockKind::Heading3 => Some(3),
            BlockKind::Heading4 => Some(4),
            BlockKind::Heading5 => Some(5),
            BlockKind::Heading6 => Some(6),
            _ => None,
        }
    }

    /// Wire (kebab-case) form used in JSON and in the `data-block-kind`
    /// HTML attribute.
    ///
    /// TRACE: SCN-12
    pub fn wire_str(&self) -> &'static str {
        match self {
            BlockKind::Heading1 => "heading-1",
            BlockKind::Heading2 => "heading-2",
            BlockKind::Heading3 => "heading-3",
            BlockKind::Heading4 => "heading-4",
            BlockKind::Heading5 => "heading-5",
            BlockKind::Heading6 => "heading-6",
            BlockKind::Paragraph => "paragraph",
            BlockKind::Table => "table",
            BlockKind::CodeBlock { .. } => "code-block",
            BlockKind::ListItem { .. } => "list-item",
            BlockKind::Blockquote => "blockquote",
            BlockKind::ThematicBreak => "thematic-break",
            BlockKind::Image => "image",
            BlockKind::Title => "title",
            // The CommonMark block type is an internal splice detail, not a
            // wire distinction: every raw HTML block is `"html"` on the wire.
            BlockKind::Html => "html",
            // Static wire value; the per-node label is carried on the DOM
            // marker attribute (A6), not in the alignment row's block_kind.
            BlockKind::Skipped { .. } => "skipped",
        }
    }
}

/// How the source **spelled** a block — the axis orthogonal to [`BlockKind`].
///
/// [`BlockKind`] says what a block *is* (a level-2 heading, a table, a list
/// item); this says how the source *wrote* it. The two were conflated until ti
/// `490d97` wave 2: `BlockKind::Html { block_type }` meant both "no semantic
/// classification" and "spelled as HTML", which is harmless while every
/// document is Markdown and wrong the moment an HTML document's `<h1>` has to
/// be a heading. Splitting the axes is what lets that `<h1>` be
/// [`BlockKind::Heading1`] and keep `h1-0001`, its section scope, and its
/// heading context, while still translating through the segment engine.
///
/// [`Copy`] on purpose: it is two words at most, every consumer wants it by
/// value, and a `Block` field that had to be borrowed would make the dispatch
/// sites noisier for nothing.
///
/// TRACE: ADR-0025
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spelling {
    /// GFM syntax. Every consumer that reparses the block under comrak —
    /// `validate::fragment_reparse`, `validate::full_reparse`, the Markdown
    /// renderer, `unit::context`'s heading projection — lives behind this arm.
    Markdown,
    /// Raw HTML markup, translated through text-segment extraction and
    /// positional splice-back (ADR-0018). The markup itself never reaches the
    /// model.
    Html {
        /// The CommonMark HTML block type (1–7) comrak reported, when the
        /// block is a raw-HTML **island inside a Markdown document**. It is
        /// the only input the blank-line splice policy takes.
        ///
        /// `None` for every block of an **HTML document**, where no CommonMark
        /// context exists and blank-line collapse must never run: collapsing
        /// exists because a blank line terminates a CommonMark HTML block of
        /// type 6/7 at reparse, and nothing ever Markdown-reparses an HTML
        /// document.
        block_type: Option<u8>,
    },
}

impl Spelling {
    /// THE spelling→splice-policy mapping, and the only place the
    /// HTML-document case is decided.
    ///
    /// An island defers to [`BlankLinePolicy::from_commonmark_html_block_type`]
    /// — wave 0's one home for the `6 | 7` rule. A block of an HTML document
    /// answers [`BlankLinePolicy::Keep`], because "a blank line terminates
    /// this block" is a statement about the *host format* and an HTML document
    /// has no such rule.
    ///
    /// Takes the `Option<u8>` rather than `&self` so the caller that has
    /// already destructured `Spelling::Html { block_type }` in a match arm can
    /// use it without a second match and without an `expect` on the
    /// [`Spelling::Markdown`] arm, which never splices.
    ///
    /// [`BlankLinePolicy::from_commonmark_html_block_type`]: transync_html::BlankLinePolicy::from_commonmark_html_block_type
    /// [`BlankLinePolicy::Keep`]: transync_html::BlankLinePolicy::Keep
    pub fn blank_line_policy_for(block_type: Option<u8>) -> transync_html::BlankLinePolicy {
        match block_type {
            Some(t) => transync_html::BlankLinePolicy::from_commonmark_html_block_type(t),
            None => transync_html::BlankLinePolicy::Keep,
        }
    }
}

/// Which intake produced a document — the plain two-value format label.
///
/// Distinct from [`Spelling`] and doing a different job. Spelling is
/// **per block**, and it has to be: the Markdown parser's `HtmlBlock` arm
/// interleaves HTML-spelled blocks with Markdown-spelled ones inside one
/// document, so no document-level bit can carry that axis. This label is
/// per *document*, and it exists so a format-committed consumer can **refuse**
/// the wrong document instead of silently producing garbage — comrak over an
/// HTML document yields *some* node sequence and would pass a check that is
/// checking nothing.
///
/// Invariant, established at intake: `Html` ⟹ every block's spelling is
/// `Spelling::Html { block_type: None }`; `Markdown` ⟹ spellings are mixed,
/// and every HTML island carries `Some(t)`.
///
/// The wire form is kebab-case (`"markdown"` / `"html"`). It reaches the wire
/// in a later wave as the alignment map's `input_format` and each row's
/// `source_format`; the spelling is fixed here so those two cannot be spelled
/// differently when they arrive.
///
/// [`Default`] is [`SourceFormat::Markdown`] because [`crate::parser::Document`]
/// derives `Default` and an empty document was a Markdown document before this
/// field existed. Every producer that is not the Markdown intake sets the
/// field explicitly.
///
/// TRACE: ADR-0025
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFormat {
    #[default]
    Markdown,
    Html,
}

/// Walk the document and re-affirm sequential IDs. Idempotent on the
/// output of [`crate::parser::parse`], which already assigns IDs in
/// source order. Re-running it after manual mutation lets callers
/// re-key the blocks list (used by future SL-10 partial-resume code).
///
/// OI-0005 (`R0001-0036` in the removed `reviews/reviewed/0001.md`):
/// re-keying also rewrites the one stored
/// cross-reference the IR still carries — `section_path` — through the
/// old→new map, so a mutated-then-re-keyed document stays
/// self-consistent. IDs unknown to the map are left untouched.
///
/// TRACE: SCN-01..SCN-14
/// TRACE: ADR-0005
pub fn assign_block_ids(doc: &mut crate::parser::Document) {
    use std::collections::HashMap;

    let mut counter: u32 = 0;
    let mut remap: HashMap<BlockId, BlockId> = HashMap::with_capacity(doc.blocks.len());
    for block in doc.blocks.iter_mut() {
        counter += 1;
        let new_id = BlockId::new(block.kind.id_code(), counter);
        let old_id = std::mem::replace(&mut block.block_id, new_id.clone());
        remap.insert(old_id, new_id);
    }

    let renew = |id: &mut BlockId| {
        if let Some(new_id) = remap.get(id) {
            *id = new_id.clone();
        }
    };
    for block in doc.blocks.iter_mut() {
        block.section_path.iter_mut().for_each(renew);
    }
}

/// Compute the SipHash-1-3 integrity hash for a block's canonical bytes
/// (the byte slice between the block's `source_range`).
///
/// TRACE: ADR-0005
pub fn source_hash_block(doc: &crate::parser::Document, block_index: usize) -> Option<u64> {
    let block = doc.blocks.get(block_index)?;
    let start = block.source_range.start.min(doc.source_text.len());
    let end = block.source_range.end.min(doc.source_text.len()).max(start);
    Some(source_hash_bytes(&doc.source_text.as_bytes()[start..end]))
}

/// SipHash-1-3 over an arbitrary byte slice. Used for both block-level
/// hashes and the alignment-map `document_id` field.
///
/// TRACE: ADR-0005
pub fn source_hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = SipHasher13::new_with_keys(0, 0);
    hasher.write(bytes);
    hasher.finish()
}

// OI-0005: re-keying keeps cross-references consistent.
#[cfg(test)]
mod rekey_tests {
    use super::*;

    #[test]
    fn rekeying_renumbers_the_id_sequence_and_remaps_section_path() {
        let src = "# Title\n\nfirst\n\nsecond\n";
        let mut doc = crate::parser::parse(src).expect("parses");

        // Simulate a caller mutation: drop the block between the heading
        // and the last paragraph, then re-key.
        doc.blocks.remove(1);
        assign_block_ids(&mut doc);

        // The whole assigned sequence, not just its endpoints: ordinals are
        // gapless and source-ordered after the removal, and each keeps its
        // own kind prefix.
        let ids: Vec<&str> = doc.blocks.iter().map(|b| b.block_id.0.as_str()).collect();
        assert_eq!(
            ids,
            vec!["h1-0001", "p-0002"],
            "re-keying renumbers from 1 in source order, kind prefix per block",
        );

        // The surviving stored cross-reference must follow the renumbering.
        let heading_id = doc.blocks[0].block_id.clone();
        assert_eq!(
            doc.blocks[1].section_path,
            vec![heading_id],
            "section_path must point at the heading's NEW id",
        );
    }

    #[test]
    fn rekeying_parse_output_is_idempotent() {
        let src = "# T\n\npara\n\n- a\n- b\n";
        let mut doc = crate::parser::parse(src).expect("parses");
        let before: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assign_block_ids(&mut doc);
        let after: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert_eq!(
            before, after,
            "assign_block_ids is idempotent on parse output"
        );
    }
}

// ti 490d97 wave 2 (spec §3, decision D2): semantic kind and source spelling
// are orthogonal axes. These pin the new vocabularies before anything
// consumes them — the projections a later variant or a later intake must
// extend in lockstep.
#[cfg(test)]
mod spelling_tests {
    use super::*;
    use transync_html::BlankLinePolicy;

    #[test]
    fn spelling_is_copy_and_carries_the_commonmark_type_only_for_an_island() {
        let island = Spelling::Html {
            block_type: Some(6),
        };
        // `Copy`, so a consumer that reads it does not move it out of a Block.
        let copied = island;
        assert_eq!(copied, island);
        assert_ne!(island, Spelling::Html { block_type: None });
        assert_ne!(island, Spelling::Markdown);
    }

    #[test]
    fn the_splice_policy_is_the_spellings_to_decide_and_it_is_total() {
        // An island inside a Markdown document takes the CommonMark rule,
        // which wave 0 moved into its one home.
        for t in 0u8..=7 {
            assert_eq!(
                Spelling::blank_line_policy_for(Some(t)),
                BlankLinePolicy::from_commonmark_html_block_type(t),
                "an island of type {t} must defer to the CommonMark mapping",
            );
        }
        // A block of an HTML document has no CommonMark context, and nothing
        // ever Markdown-reparses the output, so a blank line terminates
        // nothing: Keep, always.
        assert_eq!(
            Spelling::blank_line_policy_for(None),
            BlankLinePolicy::Keep,
            "collapse is a CommonMark rule; an HTML document has no CommonMark",
        );
        // The two agree on the sentinel `constraints.html.block_type` carries
        // for an HTML-document unit, which is what lets `validate`'s layer 3
        // reach the same answer from a `u8` it cannot distinguish from a
        // missing value.
        assert_eq!(
            BlankLinePolicy::from_commonmark_html_block_type(0),
            Spelling::blank_line_policy_for(None),
        );
    }

    #[test]
    fn source_format_wire_form_is_kebab_case_in_both_directions() {
        assert_eq!(
            serde_json::to_string(&SourceFormat::Markdown).expect("serializes"),
            "\"markdown\"",
        );
        assert_eq!(
            serde_json::to_string(&SourceFormat::Html).expect("serializes"),
            "\"html\"",
        );
        assert_eq!(
            serde_json::from_str::<SourceFormat>("\"html\"").expect("deserializes"),
            SourceFormat::Html,
        );
        // The default is what an empty `Document` means, and it is the format
        // every document had before this type existed.
        assert_eq!(SourceFormat::default(), SourceFormat::Markdown);
    }
}
