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
    /// Block-level raw HTML (spec 2026-08-03). Translatable via segment
    /// extraction; `block_type` is the CommonMark HTML block type (1–7)
    /// and drives type-conditional splice normalization.
    Html {
        block_type: u8,
    },
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
            // Raw HTML blocks get their own `html` prefix — no collision with
            // h1..h6, p, t, c, li, q, hr, img, x.
            BlockKind::Html { .. } => "html",
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
            // The CommonMark block type is an internal splice detail, not a
            // wire distinction: every raw HTML block is `"html"` on the wire.
            BlockKind::Html { .. } => "html",
            // Static wire value; the per-node label is carried on the DOM
            // marker attribute (A6), not in the alignment row's block_kind.
            BlockKind::Skipped { .. } => "skipped",
        }
    }
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
