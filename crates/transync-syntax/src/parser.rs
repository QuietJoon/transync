//! GFM parser. Wraps Comrak (per ADR-0004) and produces the in-memory
//! `Document` IR with stable block IDs assigned in source-order traversal.
//!
//! This file owns the IR types, [`parse`], and the walk itself — `WalkState`
//! plus the one match over Comrak's node kinds. Everything the walk delegates
//! to lives beside it, one concern per file: `options` (THE Comrak
//! configuration), `depth` (THE block-nesting ceiling and its pre-scan),
//! `classify` (node → `BlockKind`, and the labels a node without a kind gets
//! instead), `emit` (ID allocation, byte ranges, source hashing, the
//! empty-range guard), `sections` (THE heading-scope stack).
//! Each match arm is therefore a routing decision — which projection, at
//! which path — and nothing else.
//!
//! Refusal note: [`parse`] declines a source nested deeper than
//! [`MAX_BLOCK_NESTING_DEPTH`] — 128 block containers — with
//! [`ParseError::TooDeeplyNested`], before Comrak is called. Untrusted source
//! can nest a container per byte, and a walker that runs out of stack aborts
//! the host process rather than returning an error, so the depth is declined
//! rather than survived. `parser::depth` holds the bound, the pre-scan, and
//! the measurements the number was chosen from.
//!
//! Ownership note: the Comrak `Arena` lives only inside [`parse`]. The
//! resulting `Document` carries the block list, source ranges, and source
//! hashes; downstream regeneration reparses on demand from the same
//! source string. This keeps the IR independent of Comrak's borrow graph.
//!
//! Intake note: [`parse`] normalizes NUL before Comrak sees the document —
//! see `normalize_source` — and `Document::source_text` is the normalized
//! string. Every byte offset in the IR, and everything derived from one,
//! indexes *that* string.
//!
//! TRACE: ADR-0004

mod classify;
mod depth;
mod emit;
mod options;
pub mod ranges;
pub mod refdefs;
mod sections;

use crate::error::ParseError;
use crate::id::{BlockId, BlockKind};
use comrak::nodes::{AstNode, NodeValue};
use ranges::{ByteRange, LineOffsets};
use sections::SectionStack;
use std::borrow::Cow;

/// The canonical GFM Comrak configuration. Re-exported from `parser::options`
/// so `parser::comrak_options` — the path regen, render, and every
/// `transync-core` reparse call — keeps resolving.
pub use options::comrak_options;

/// THE block-nesting ceiling and its pre-scan, from `parser::depth`.
/// [`parse`] applies them before Comrak sees the source.
pub use depth::{MAX_BLOCK_NESTING_DEPTH, check_nesting_depth};

/// In-memory document IR. Internal type — fields evolve without a wire bump.
///
/// TRACE: SCN-01
/// TRACE: ADR-0004
#[derive(Debug, Clone, Default)]
pub struct Document {
    /// The document as parsed — the caller's bytes after `normalize_source`,
    /// which is the same string Comrak was handed. Every `source_range` in
    /// `blocks`, every `source_hash`, the alignment map's `document_id`, and
    /// regen's fallback bytes index this field, never the caller's original
    /// `&str`. The two differ only for a source containing NUL.
    pub source_text: String,
    pub blocks: Vec<Block>,
    /// Human-readable notes a reader of the run should see, in emission
    /// order.
    ///
    /// [`parse`] fills it with notes about top-level source nodes the pipeline
    /// does not model as a translatable kind (footnote definitions, front
    /// matter, …). They are preserved verbatim in the regenerated Markdown and
    /// rendered as inert, escaped placeholders anchored in both panes
    /// (invariant 7); surfaced so the handling is visible instead of silent
    /// (R0008-0013).
    ///
    /// The channel is not parse-only: a later stage may append a note about
    /// the document as a whole before the field is forwarded to
    /// `ValidationReport::skipped_source_nodes` — `transync-core` appends the
    /// html-dominance note here (ti `13e145`). Nothing on this channel is ever
    /// a refusal; a stage that cannot proceed returns an error instead.
    pub warnings: Vec<String>,
    /// Concatenated link-reference-definition text ([`refdefs::extract`])
    /// recovered from the inter-block gaps in source order. Empty when the
    /// document defines none. CommonMark definitions leave no AST node and a
    /// single-block fragment parse resolves nothing, so this pool is appended
    /// to a fragment before reparse (render) and before the inline-inventory
    /// parse (validate) to give comrak-native reference resolution. Internal
    /// field — evolves without a wire bump (design D2 §B).
    pub ref_defs: String,
}

/// One sync-relevant block in source order.
///
/// Every block is top-level: the walker never recurses, so a list item or a
/// blockquote is a LEAF unit whose `source_range` already covers its nested
/// content (`R0001-0003` in the removed `reviews/reviewed/0001.md`; see
/// `reviews/README.md` for why a retired-round id names its file). There is
/// deliberately no `parent_id` — it was
/// always `None`, and the wire field `align::AlignmentBlock::parent_id`
/// keeps that fact on the contract (`contracts.md` §3/§4a).
///
/// TRACE: ADR-0005
#[derive(Debug, Clone)]
pub struct Block {
    pub block_id: BlockId,
    pub kind: BlockKind,
    pub source_range: ByteRange,
    pub source_hash: u64,
    pub section_path: Vec<BlockId>,
    pub ast_path: AstPath,
}

/// Index path into the AST. List items carry the owning `List` node's
/// path plus their own index, which is what lets consumers group
/// sibling items of one source list: `validate::full_reparse` collapses
/// list runs by path prefix, and `render::render_fragment` opens one
/// shared `<ul>`/`<ol>` per run (DCR-0007). OI-0005: consumed.
///
/// TRACE: SCN-14
#[derive(Debug, Clone, Default)]
pub struct AstPath(pub Vec<usize>);

/// Heading-derived hierarchy section.
///
/// `children` lists the blocks of the NEAREST enclosing heading scope
/// only — by design (OI-0005): ancestor membership is derivable from
/// each block's `section_path`, which carries the full heading chain.
///
/// **No producer today.** The eagerly-built `Document::hierarchy` this
/// type populated had no pipeline consumer and cost an extra pass plus a
/// `HashMap` on every parse, so it was deleted; `Block::section_path` —
/// stamped by the ONE heading-scope stack (`parser::sections`) — is the
/// section model the pipeline actually reads. The shape survives so a future
/// hierarchical-alignment revision (OI-0005) can project it from that
/// same stack instead of re-deriving the scope rule a second time.
///
/// TRACE: SCN-09
#[derive(Debug, Clone)]
pub struct Section {
    pub heading_id: BlockId,
    pub level: u8,
    pub children: Vec<BlockId>,
}

/// Substitute U+FFFD for every NUL byte, borrowing when there is none.
///
/// CommonMark §2.3 requires a parser to replace U+0000 with the REPLACEMENT
/// CHARACTER, and Comrak does exactly that — *before* it records any
/// `Sourcepos`. Its byte columns are therefore counted over text in which
/// each one-byte NUL has become a three-byte U+FFFD, while
/// [`ranges::LineOffsets`] maps those columns onto the string it was handed.
/// Feed the two different strings and the mapping drifts two bytes per
/// preceding NUL on the line (OI-0034).
///
/// Doing the substitution here is the owner-decided posture (2026-08-06):
/// **normalize at intake**, so Comrak and `ranges` count the same bytes and
/// the agreement is structural rather than a property the clamps in
/// [`ranges::LineOffsets::pos_to_byte`] happen to hide. It is the same shape of fix as the
/// lone-CR desync (OI-0033) — make the parser agree with the parser — but the
/// other one of the two admissible mechanisms: lone CR is *content* Comrak
/// preserves, so the line table moved to meet it; NUL is content Comrak has
/// already thrown away by the time it reports a position, so there is nothing
/// for a line table to agree with and the input must move instead.
///
/// The byte-verbatim round-trip guarantee `regen` and `htmlseg` keep — the
/// reason normalization was rejected for lone CR — is unaffected in
/// substance: it is a guarantee about `Document::source_text`, and this
/// substitution happens before that field exists. What changes for a
/// NUL-bearing source is that `out.md` carries U+FFFD instead of the NUL —
/// which is what the rendered panes have always shown, because they render
/// through Comrak.
fn normalize_source(source: &str) -> Cow<'_, str> {
    if source.as_bytes().contains(&0) {
        Cow::Owned(source.replace('\0', "\u{FFFD}"))
    } else {
        Cow::Borrowed(source)
    }
}

/// The ONE guarded way this crate hands raw Markdown to Comrak.
///
/// Two entry points feed Comrak: [`parse`], which builds the IR out of the
/// source document, and `render::render_fragment`, which parses whichever
/// pane's Markdown it is about to render (the target pane's Markdown never
/// went through [`parse`] — it arrives already translated). Routing both
/// through this function is what stops them growing separate opinions about
/// the nesting ceiling or about NUL: a document accepted as a source is
/// accepted as a target, and refused the same way.
///
/// Returns the string to hand Comrak — `source` itself when it holds no NUL,
/// an owned copy with U+FFFD substituted when it does. CommonMark §2.3
/// requires that substitution and Comrak performs it internally *before* it
/// records any position, so doing it here is what keeps Comrak's byte columns
/// and this crate's line table counting the same bytes (`normalize_source`,
/// private, carries the full argument).
///
/// TRACE: OI-0034
pub fn intake(source: &str) -> Result<Cow<'_, str>, ParseError> {
    // First, and on the caller's own bytes: a tree too deep to walk must not
    // be built, and NUL normalization cannot change how deep it nests.
    check_nesting_depth(source)?;
    Ok(normalize_source(source))
}

/// Parse GFM Markdown into the internal IR. Block IDs are assigned in
/// source order during the walk; [`crate::id::assign_block_ids`] is then
/// idempotent.
///
/// `source` is normalized by `normalize_source` first, and it is the
/// normalized string — not the caller's — that becomes
/// [`Document::source_text`] and that every byte range in the IR indexes.
///
/// Refuses a source nested deeper than [`MAX_BLOCK_NESTING_DEPTH`] with
/// [`ParseError::TooDeeplyNested`], before anything is allocated or parsed;
/// see `parser::depth` for why the ceiling exists and how its value was
/// chosen.
///
/// TRACE: SCN-01..SCN-14
/// TRACE: ADR-0004
/// TRACE: OI-0034
pub fn parse(source: &str) -> Result<Document, ParseError> {
    let normalized = intake(source)?;
    let source: &str = &normalized;

    let arena = comrak::Arena::new();
    let opts = options::gfm_options();
    let root = comrak::parse_document(&arena, source, &opts);
    let line_offsets = LineOffsets::new(source);

    let mut state = WalkState::new(source, &line_offsets);
    state.walk_children(root);
    let (blocks, mut warnings) = state.finish();

    // Recover document-level link-reference definitions from the inter-block
    // gaps (design D2 §B). Any noded-gap warnings join the skipped-node
    // warnings so both surface through `Document::warnings`.
    let (ref_defs, mut ref_warnings) = refdefs::extract(source, &blocks);
    warnings.append(&mut ref_warnings);

    Ok(Document {
        source_text: normalized.into_owned(),
        blocks,
        warnings,
        ref_defs,
    })
}

/// Everything one walk of one document accumulates.
///
/// The walk used to thread these seven values through two nine-parameter
/// signatures, both of which needed `#[allow(clippy::too_many_arguments)]`
/// to compile clean. Bundling them lets `emit` and the section stack hang off
/// the state as methods, which is what keeps every arm of [`WalkState::visit`]
/// down to a kind and a path.
struct WalkState<'s> {
    /// The document being walked. Only `emit` reads it (to hash the bytes a
    /// block slices to).
    source: &'s str,
    /// Line-start table for `Sourcepos` → byte-offset resolution.
    line_offsets: &'s LineOffsets<'s>,
    /// Blocks in source order — the walk's product.
    blocks: Vec<Block>,
    /// Ordinal for the next block ID. Bumped only by `emit`.
    counter: u32,
    /// Open heading scopes; stamps every block's `section_path`.
    sections: SectionStack,
    /// The walker's current position in the AST, one index per depth level.
    ast_path: Vec<usize>,
    /// Skipped-node notes, surfaced through `Document::warnings`.
    warnings: Vec<String>,
}

impl<'s> WalkState<'s> {
    fn new(source: &'s str, line_offsets: &'s LineOffsets<'s>) -> Self {
        WalkState {
            source,
            line_offsets,
            blocks: Vec::new(),
            counter: 0,
            sections: SectionStack::default(),
            ast_path: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Consume the state, yielding what `parse` assembles into a `Document`.
    fn finish(self) -> (Vec<Block>, Vec<String>) {
        (self.blocks, self.warnings)
    }

    fn walk_children<'a>(&mut self, parent: &'a AstNode<'a>) {
        for (i, child) in parent.children().enumerate() {
            self.ast_path.push(i);
            self.visit(child);
            self.ast_path.pop();
        }
    }

    fn visit<'a>(&mut self, node: &'a AstNode<'a>) {
        let value = node.data.borrow().value.clone();

        match value {
            NodeValue::Heading(h) => {
                let Some(kind) = classify::heading_kind(h.level) else {
                    return;
                };
                // Close existing scopes >= this heading's level, capture the
                // surviving section path, then open the new scope.
                let sp = self.sections.close_through(h.level);
                let id = self.emit_here(node, kind, sp);
                self.sections.open_scope(id, h.level);
            }
            NodeValue::Paragraph => {
                let sp = self.sections.current_path();
                // OI-0002 lives in `classify`: an image-only paragraph is
                // promoted to a block-level `BlockKind::Image`.
                let kind = classify::paragraph_kind(node);
                self.emit_here(node, kind, sp);
            }
            NodeValue::Table(_) => {
                let sp = self.sections.current_path();
                self.emit_here(node, BlockKind::Table, sp);
                // Table rows/cells are not separate sync anchors.
            }
            NodeValue::CodeBlock(c) => {
                let sp = self.sections.current_path();
                self.emit_here(node, classify::code_block_kind(&c), sp);
            }
            NodeValue::List(list) => {
                // The List node itself is not a sync anchor; iterate its
                // children directly so we can emit each Item / TaskItem
                // with the parent list's ordered-ness, even when the child
                // is a TaskItem (the TaskItem AST node carries no list-type
                // marker of its own — that lives on the owning List).
                //
                // Items in this leaf model are leaf translation units —
                // their source_range covers nested paragraphs / sublists,
                // so we don't recurse beyond the Item itself. See
                // `R0001-0003` in the removed `reviews/reviewed/0001.md` for
                // the rationale.
                let list_ordered = matches!(list.list_type, comrak::nodes::ListType::Ordered);
                for (item_index, child) in node.children().enumerate() {
                    let value = child.data.borrow().value.clone();
                    let Some(kind) = classify::list_item_kind(&value, list_ordered) else {
                        continue;
                    };
                    let sp = self.sections.current_path();
                    // The item's own path is the owning List's path plus the
                    // item index — NOT the walker's current path, which stops at
                    // the List (the items are never `visit`ed). `walk` groups a
                    // list run by exactly this prefix, so the extra element is
                    // load-bearing; `ast_path_tests` pins it. This is the one
                    // arm that needs the general `emit` rather than `emit_here`.
                    let item_path = {
                        let mut p = self.ast_path.clone();
                        p.push(item_index);
                        p
                    };
                    self.emit(child, kind, sp, item_path);
                }
            }
            NodeValue::Item(_) | NodeValue::TaskItem(_) => {
                // Reachable only if a List walker external to this match
                // arm dispatches Item/TaskItem directly. The NodeValue::List
                // arm above owns the standard path; this fallback keeps
                // visit() total in case Comrak produces an orphan Item.
                let kind = BlockKind::ListItem {
                    ordered: false,
                    task: None,
                };
                let sp = self.sections.current_path();
                // `emit` runs the empty-range guard itself, so this arm no longer
                // re-reads the pushed block to call it a second time.
                self.emit_here(node, kind, sp);
            }
            NodeValue::BlockQuote => {
                // Blockquote is a leaf translation unit — its source_range
                // already covers all > -prefixed content. Per-kind validation
                // walks the payload via inspect_blockquote_children.
                let sp = self.sections.current_path();
                self.emit_here(node, BlockKind::Blockquote, sp);
            }
            NodeValue::ThematicBreak => {
                let sp = self.sections.current_path();
                self.emit_here(node, BlockKind::ThematicBreak, sp);
            }
            NodeValue::HtmlBlock(h) => {
                // Spec 2026-08-03 §3.1: block-level raw HTML is translatable.
                // Segment extraction happens at unit construction, not here.
                let sp = self.sections.current_path();
                self.emit_here(
                    node,
                    BlockKind::Html {
                        block_type: h.block_type,
                    },
                    sp,
                );
            }
            _ => {
                // R0008-0013: a top-level block node we don't model as a
                // translatable kind (footnote definition, front matter, …; raw
                // HTML has its own arm above). Emit it as a `Skipped` block so it
                // anchors in both
                // panes as an inert, escaped placeholder (invariant 7) instead of
                // silently vanishing. `emit` gives it a `source_range`,
                // `source_hash`, `section_path`, and `ast_path` for free — regen
                // then splices its source bytes verbatim.
                let sp = self.sections.current_path();
                self.emit_here(node, classify::skipped_kind(&value), sp);
                self.warnings.push(classify::skip_warning(&value));
            }
        }
    }
}

#[cfg(test)]
mod skipped_node_tests {
    use super::*;

    #[test]
    fn skipped_block_id_round_trips_assign_block_ids() {
        // No node maps to Skipped under the current comrak options (html
        // became a real kind); pin the re-key path synthetically.
        let mut doc = parse("real paragraph\n").expect("parses");
        doc.blocks.insert(
            0,
            Block {
                block_id: BlockId::new("x", 99),
                kind: BlockKind::Skipped {
                    label: "unsupported".to_string(),
                },
                source_range: ByteRange { start: 0, end: 0 },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: AstPath(Vec::new()),
            },
        );
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "x-0001");
        assert_eq!(doc.blocks[1].block_id.0, "p-0002");
    }

    #[test]
    fn ordinary_document_has_no_skip_warnings() {
        let doc = parse("# Title\n\nbody paragraph\n\n- a\n- b\n").expect("parses");
        assert!(
            doc.warnings.is_empty(),
            "no skip warnings for a fully-modeled document, got {:?}",
            doc.warnings,
        );
    }
}

// Spec 2026-08-03 §3.1: raw HTML blocks are a first-class translatable
// kind with the CommonMark block type recorded for splice normalization.
#[cfg(test)]
mod html_block_tests {
    use super::*;

    #[test]
    fn top_level_html_block_becomes_block_kind_html_with_type() {
        let doc = parse("<div class=\"note\">side note</div>\n\nreal paragraph\n").expect("parses");
        let html = &doc.blocks[0];
        assert!(
            matches!(&html.kind, BlockKind::Html { block_type } if *block_type == 6),
            "expected Html type 6, got {:?}",
            html.kind
        );
        assert_eq!(html.block_id.0, "html-0001", "html id prefix");
        assert_eq!(doc.blocks[1].block_id.0, "p-0002");
        assert!(
            doc.warnings.is_empty(),
            "html blocks are translatable — no skip warning: {:?}",
            doc.warnings
        );
    }

    #[test]
    fn pre_block_records_type_1() {
        let doc = parse("<pre>\nascii art\n</pre>\n").expect("parses");
        assert!(
            matches!(&doc.blocks[0].kind, BlockKind::Html { block_type } if *block_type == 1),
            "got {:?}",
            doc.blocks[0].kind
        );
    }

    #[test]
    fn html_block_ids_round_trip_assign_block_ids() {
        let mut doc = parse("<div>x</div>\n\npara\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "html-0001");
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "html-0001", "idempotent");
    }

    #[test]
    fn interleaved_details_region_parses_as_multiple_fragments() {
        // Spec §1 "fragment reality": blank lines split type-6 regions.
        let src = "<details>\n<summary>More</summary>\n\nBody paragraph.\n\n</details>\n";
        let doc = parse(src).expect("parses");
        let kinds: Vec<&str> = doc.blocks.iter().map(|b| b.kind.wire_str()).collect();
        assert_eq!(kinds, vec!["html", "paragraph", "html"]);
    }
}

// The block-nesting ceiling, at the level that matters: `parse` refusing a
// document rather than a downstream walker aborting the process. The bound
// itself, and the pre-scan's own behavior, are pinned in `parser::depth`.
#[cfg(test)]
mod nesting_depth_tests {
    use super::*;

    /// `levels` blockquote markers on one line — one byte per level, which
    /// is what makes this the cheapest deep document to write and the
    /// reason a byte-count limit would not have caught it: twenty thousand
    /// levels is twenty kilobytes.
    fn blockquotes(levels: usize) -> String {
        format!("{} quoted\n", ">".repeat(levels))
    }

    /// One list level per line, two more columns of indentation each time.
    fn nested_list(levels: usize) -> String {
        (0..levels).fold(String::new(), |mut s, level| {
            s.push_str(&"  ".repeat(level));
            s.push_str("- item\n");
            s
        })
    }

    /// The same, written with *empty* items — a bare `-` at end of line. It
    /// is a list item under CommonMark and comrak nests into it exactly the
    /// same way, so the guard has to see it (Review-0002).
    fn nested_empty_items(levels: usize) -> String {
        (0..levels).fold(String::new(), |mut s, level| {
            s.push_str(&"  ".repeat(level));
            s.push_str("-\n");
            s
        })
    }

    #[test]
    fn a_document_past_the_ceiling_is_refused_with_a_typed_error() {
        // The ticket's own reproduction: one line, ~20 KB, twenty thousand
        // nested blockquotes. It used to reach comrak; now it does not.
        let err = parse(&blockquotes(20_000)).expect_err("20,000 levels must be refused");
        let ParseError::TooDeeplyNested { depth, limit } = err else {
            panic!("expected TooDeeplyNested, got {err:?}");
        };
        assert_eq!(limit, MAX_BLOCK_NESTING_DEPTH);
        assert!(depth > limit, "depth {depth} must exceed the limit {limit}");

        // And the line-per-level list shape, which no per-line ceiling in
        // comrak constrains.
        assert!(
            matches!(
                parse(&nested_list(5_000)),
                Err(ParseError::TooDeeplyNested { .. })
            ),
            "5,000 nested list levels must be refused",
        );

        // Review-0002: the same list spelled with empty items. The scan used
        // to demand whitespace after a marker, so every one of these lines
        // scored zero and the document walked straight through the guard into
        // a tree 601 nodes deep at 300 levels.
        assert!(
            matches!(
                parse(&nested_empty_items(300)),
                Err(ParseError::TooDeeplyNested { .. })
            ),
            "300 nested empty list items must be refused",
        );
    }

    #[test]
    fn a_document_at_the_ceiling_still_parses() {
        let doc = parse(&blockquotes(MAX_BLOCK_NESTING_DEPTH)).expect("128 levels parse");
        assert_eq!(
            doc.blocks
                .iter()
                .map(|b| b.kind.wire_str())
                .collect::<Vec<_>>(),
            vec!["blockquote"],
            "the whole nest is one leaf blockquote unit",
        );

        let doc = parse(&nested_list(MAX_BLOCK_NESTING_DEPTH)).expect("128 list levels parse");
        assert_eq!(
            doc.blocks.len(),
            1,
            "the outermost item owns its whole nested payload",
        );

        // Review-0002: the empty-item spelling nests just as deep, so the
        // scan has to count it — and counting it must not cost the shapes
        // just under the ceiling.
        let doc = parse(&nested_empty_items(MAX_BLOCK_NESTING_DEPTH))
            .expect("128 empty list levels parse");
        assert_eq!(
            doc.blocks.len(),
            1,
            "the outermost empty item owns its whole nested payload",
        );
    }

    #[test]
    fn documents_anyone_would_write_are_untouched() {
        // The guard runs on every parse, so the thing that must not happen
        // is a false refusal. These are the shapes with the most container
        // syntax per line in the repo's own scenario corpus.
        for src in [
            "# Title\n\nA paragraph.\n",
            "- a\n  - b\n    - c\n      - d\n",
            "> quoted\n> > deeper\n> > > deeper still\n",
            "1. one\n   1. one one\n      - bullet\n",
            "| a | b |\n| - | - |\n| 1 | 2 |\n",
            "```rust\nfn main() {\n    let deeply = vec![vec![vec![1]]];\n}\n```\n",
            "<div>\n  <p>\n    <span>indented html</span>\n  </p>\n</div>\n",
        ] {
            assert!(parse(src).is_ok(), "must still parse: {src:?}");
        }
    }
}

// OI-0033: a lone `\r` is a CommonMark line ending and comrak counts it as
// one, so `ranges::LineOffsets` must too. Before the fix every block after
// the first lone CR resolved to an out-of-range line and collapsed to an
// empty range at EOF. This is the parse-level pin for that; the scope pins
// for the empty-range guard live beside the guard in `parser::emit`.
#[cfg(test)]
mod lone_cr_tests {
    use super::*;

    /// The OI-0033 investigation's "CR-only list only" probe document.
    /// Byte layout: `intro para` 0..10, `\r\r` 10..12, `- alpha` 12..19,
    /// `\r` 19, `- bravo` 20..27, `\r` 27, `- charlie` 28..37, `\r\r` 37..39,
    /// `tail para` 39..48, `\r` 48 — 49 bytes.
    const CR_LIST: &str = "intro para\r\r- alpha\r- bravo\r- charlie\r\rtail para\r";

    #[test]
    fn lone_cr_document_slices_every_block_to_its_own_bytes() {
        let doc = parse(CR_LIST).expect("parses");

        let kinds: Vec<&str> = doc.blocks.iter().map(|b| b.kind.wire_str()).collect();
        assert_eq!(
            kinds,
            vec![
                "paragraph",
                "list-item",
                "list-item",
                "list-item",
                "paragraph"
            ],
            "one paragraph, three items, one paragraph",
        );

        let ranges: Vec<(usize, usize)> = doc
            .blocks
            .iter()
            .map(|b| (b.source_range.start, b.source_range.end))
            .collect();
        assert_eq!(
            ranges,
            vec![(0, 10), (12, 19), (20, 27), (28, 38), (39, 48)],
            "probe `oi0033.md` §2 'CR-only list only' CR-aware offsets",
        );

        let payloads: Vec<&str> = doc
            .blocks
            .iter()
            .map(|b| &doc.source_text[b.source_range.start..b.source_range.end])
            .collect();
        assert_eq!(
            payloads,
            vec![
                "intro para",
                "- alpha",
                "- bravo",
                // The last item's sourcepos ends at the *next* line's column
                // 0, so its range carries the terminator — identical to the
                // LF spelling of the same document, not a CR artifact.
                "- charlie\r",
                "tail para",
            ],
        );

        // Non-empty, non-overlapping, strictly increasing — the structural
        // property that fails wholesale under the pre-fix desync (every block
        // after the first collapsed to `49..49`).
        let mut prev_end = 0usize;
        for b in &doc.blocks {
            assert!(
                b.source_range.start < b.source_range.end,
                "{} has an empty range {:?}",
                b.block_id,
                b.source_range,
            );
            assert!(
                b.source_range.start >= prev_end,
                "{} overlaps the previous block ({:?} after end {prev_end})",
                b.block_id,
                b.source_range,
            );
            prev_end = b.source_range.end;
        }

        // The pre-fix desync's one observable symptom was refdefs' "unattributed
        // source text between blocks" note, because the gaps swelled to cover
        // the real content. Correct slicing leaves only terminator bytes in the
        // gaps, so the document parses clean.
        assert!(doc.warnings.is_empty(), "got {:?}", doc.warnings);
    }
}

// OI-0034: Comrak substitutes a three-byte U+FFFD for every one-byte NUL
// before it records a `Sourcepos`, so a line table built over the raw bytes
// disagrees with its columns past the first NUL on a line. `parse` performs
// the substitution itself (`normalize_source`), which makes the two count the
// same bytes and makes the normalized string THE source everything downstream
// indexes. These are the parse-level pins for that posture.
#[cfg(test)]
mod nul_tests {
    use super::*;
    use comrak::nodes::NodeValue;
    use std::collections::HashMap;

    /// Inline `&str` constants, never a checked-in `.md` file — the OI-0033
    /// fixture precedent, and doubly so here: a NUL byte is exactly what an
    /// editor, a filter, or a `git` clean pass is liable to drop on the way to
    /// disk, and the test would then pass vacuously.
    ///
    /// The three trailing spaces are load-bearing. They put the paragraph's
    /// inline end column strictly *inside* the line, which is the only place a
    /// drifted column is observable: a column that overshoots the line's
    /// content end is clamped back onto it by [`ranges::LineOffsets`], and a
    /// block-level `Sourcepos` always ends at the line's content end, so a
    /// block range alone cannot tell an exact mapping from a rescued one.
    const NUL_DOC: &str = "alpha\0bravo   \n\n# head\0ing\n\ntail para\n";

    /// `NUL_DOC` after `normalize_source` — three bytes where each NUL was.
    const NORMALIZED: &str = "alpha\u{FFFD}bravo   \n\n# head\u{FFFD}ing\n\ntail para\n";

    #[test]
    fn a_nul_source_parses_as_its_normalized_self() {
        let doc = parse(NUL_DOC).expect("parses");

        assert_eq!(
            doc.source_text, NORMALIZED,
            "the normalized string is the document",
        );
        assert!(
            !doc.source_text.contains('\0'),
            "no NUL survives into the IR",
        );

        let kinds: Vec<&str> = doc.blocks.iter().map(|b| b.kind.wire_str()).collect();
        assert_eq!(kinds, vec!["paragraph", "heading-1", "paragraph"]);

        let ranges: Vec<(usize, usize)> = doc
            .blocks
            .iter()
            .map(|b| (b.source_range.start, b.source_range.end))
            .collect();
        assert_eq!(
            ranges,
            vec![(0, 16), (18, 30), (32, 41)],
            "offsets into the normalized text: each NUL costs two extra bytes, \
             so every range past one moves",
        );

        let payloads: Vec<&str> = doc
            .blocks
            .iter()
            .map(|b| &doc.source_text[b.source_range.start..b.source_range.end])
            .collect();
        assert_eq!(
            payloads,
            vec![
                "alpha\u{FFFD}bravo   ",
                // Past the first NUL, on a line carrying a NUL of its own —
                // both drifts compound here, and neither shows.
                "# head\u{FFFD}ing",
                "tail para",
            ],
        );

        assert!(doc.warnings.is_empty(), "got {:?}", doc.warnings);
    }

    #[test]
    fn columns_past_the_nul_resolve_exactly_rather_than_by_the_clamp() {
        let doc = parse(NUL_DOC).expect("parses");
        let line_offsets = LineOffsets::new(&doc.source_text);

        // Comrak's own columns, over the same bytes the table was built from.
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(&arena, &doc.source_text, &options::gfm_options());
        let text = root
            .descendants()
            .find(|n| matches!(n.data.borrow().value, NodeValue::Text(_)))
            .expect("the first paragraph has an inline text node");
        let range = ranges::byte_range_for(text, &line_offsets);

        assert_eq!(
            &doc.source_text[range.start..range.end],
            "alpha\u{FFFD}bravo",
            "the inline run resolves to its own bytes, three past where a \
             one-byte NUL would have put its tail",
        );

        // The whole point of the trailing spaces: the resolved end is strictly
        // inside the line, so the content-end clamp is provably not what
        // produced it. Feed the table the raw-NUL spelling instead and this
        // same column lands on a space (`ranges::tests::raw_nul_columns_drift`).
        let line_len = doc.source_text.lines().next().expect("a first line").len();
        assert_eq!(line_len, 16);
        assert!(
            range.end < line_len,
            "range {range:?} must not be the clamp's answer ({line_len})",
        );
    }

    #[test]
    fn the_normalized_text_is_the_source_for_ids_hashes_and_regen() {
        let mut doc = parse(NUL_DOC).expect("parses");
        crate::id::assign_block_ids(&mut doc);

        let ids: Vec<&str> = doc.blocks.iter().map(|b| b.block_id.0.as_str()).collect();
        assert_eq!(ids, vec!["p-0001", "h1-0002", "p-0003"]);

        for b in &doc.blocks {
            assert_eq!(
                b.source_hash,
                crate::id::source_hash_bytes(
                    &NORMALIZED.as_bytes()[b.source_range.start..b.source_range.end]
                ),
                "{} hashes the normalized bytes",
                b.block_id,
            );
        }

        // Nothing accepted: regen falls every block back to its own source
        // bytes, which must reproduce the document exactly. That is where the
        // substitution becomes visible in `out.md` — U+FFFD, which is what the
        // rendered panes have always shown, instead of the NUL.
        let (regenerated, _) = crate::regen::regenerate(&doc, &HashMap::new());
        assert_eq!(regenerated, NORMALIZED);
    }

    #[test]
    fn normalization_is_a_fixed_point_and_a_no_op_without_a_nul() {
        // Parsing the already-normalized spelling gives the same document, so
        // a second pass through `parse` cannot move an offset.
        let from_raw = parse(NUL_DOC).expect("parses");
        let from_normalized = parse(NORMALIZED).expect("parses");
        assert_eq!(from_raw.source_text, from_normalized.source_text);
        let ranges = |d: &Document| -> Vec<(usize, usize)> {
            d.blocks
                .iter()
                .map(|b| (b.source_range.start, b.source_range.end))
                .collect()
        };
        assert_eq!(ranges(&from_raw), ranges(&from_normalized));

        // A source with no NUL is handed through untouched (and borrowed).
        let plain = "# Title\n\npara one\n\n- a\n- b\n";
        assert_eq!(parse(plain).expect("parses").source_text, plain);
        assert!(matches!(normalize_source(plain), Cow::Borrowed(_)));
        assert!(matches!(normalize_source(NUL_DOC), Cow::Owned(_)));
    }
}

// ti `457e51`: indented (four-space / tab) code blocks.
//
// Comrak reports an indented code block's sourcepos AFTER the columns of
// block structure it consumed, so `outcome::block_payload` — which slices
// `Block::source_range` — handed back the DEDENTED first line. That defect
// had TWO mechanisms, and only the shallower one was loud. A four-space
// block's slice can only reparse as a paragraph, so
// `validate::fragment_reparse` rejected it three times and it fell back
// untranslated. A block indented eight columns or more sliced to something
// that is *itself* a valid indented code block: it passed every per-unit
// layer, was ACCEPTED on attempt one, and was then re-fenced with the four
// consumed columns still outside its range — an unclosed fence that
// swallowed the following paragraph, caught only by `validate::full_reparse`
// and the DCR-0004 cascade. Anything that says the pre-fix behavior was
// uniformly "rejected three times and fell back" is describing the four-space
// class alone.
//
// These pin the facts the fix has to produce: the slice is the block's
// COMPLETE indented spelling (moved back over the block-structure indent, so
// a tab is one byte and eight spaces keep their content indent), the walk
// stops at a byte that is not indentation (a document-leading BOM), and the
// kind records how the source spelled the block.
#[cfg(test)]
mod indented_code_tests {
    use super::*;

    /// `(wire kind, payload slice)` for every block, in source order.
    fn blocks(src: &str) -> Vec<(&'static str, String)> {
        let doc = parse(src).expect("parses");
        doc.blocks
            .iter()
            .map(|b| (b.kind.wire_str(), crate::outcome::block_payload(&doc, b)))
            .collect()
    }

    /// The payload of the single code block in `src`.
    fn code_payload(src: &str) -> String {
        let doc = parse(src).expect("parses");
        let block = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
            .expect("the fixture has a code block");
        crate::outcome::block_payload(&doc, block)
    }

    #[test]
    fn a_four_space_block_slices_to_its_full_indented_spelling() {
        assert_eq!(
            blocks("Intro.\n\n    line one\n    line two\n\nOutro.\n"),
            vec![
                ("paragraph", "Intro.".to_string()),
                ("code-block", "    line one\n    line two\n".to_string()),
                ("paragraph", "Outro.".to_string()),
            ],
            "the slice must carry the block-structure indent on EVERY line, \
             the first one included — comrak's sourcepos starts past it",
        );
    }

    #[test]
    fn an_eight_space_block_keeps_the_four_columns_of_content_indent() {
        // Four columns are block structure; the other four are content, and
        // the slice must keep all eight. For THIS spelling a "start minus four
        // bytes" snap would land on the line start too — it is the tab case
        // below that breaks it, and a leading BOM that makes the walk (rather
        // than a jump to column 1) the right rule.
        assert_eq!(
            code_payload("Intro.\n\n        indented code line\n\nOutro.\n"),
            "        indented code line\n",
        );
    }

    #[test]
    fn a_tab_indented_block_slices_from_column_one() {
        // The tab is ONE byte that opens FOUR columns. A "start minus four
        // bytes" snap would reach back into the preceding blank line; walking
        // back over the whitespace run lands exactly on the tab and stops
        // there, because the line start bounds it.
        assert_eq!(
            code_payload("Intro.\n\n\tindented code line\n\nOutro.\n"),
            "\tindented code line\n",
        );
    }

    /// The snap walks back over INDENTATION, not over "whatever comrak's
    /// column counted", and a document-leading UTF-8 BOM is the case that
    /// tells the two apart.
    ///
    /// Comrak skips the BOM for block structure but still counts its three
    /// bytes in the column, so `"\u{feff}    code\n"` reports its code block
    /// at `1:8` — four columns of indent on top of three BOM bytes. An
    /// unconditional snap to column 1 would pull the BOM INSIDE the block's
    /// range, and because an accepted translation replaces exactly the block's
    /// range with a regenerated fence, that deletes the BOM from the output.
    /// Every other first-block kind leaves the BOM in the inter-block gap,
    /// where it survives; an indented code block must not be the exception.
    #[test]
    fn the_snap_stops_at_a_document_leading_bom() {
        const BOM: &str = "\u{feff}";

        let src = format!("{BOM}    indented code line\n\nOutro.\n");
        let doc = parse(&src).expect("parses");
        let code = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
            .expect("the fixture has a code block");
        assert_eq!(
            code.source_range.start,
            BOM.len(),
            "the BOM is not indentation: the walk must stop on it, leaving it \
             in the inter-block gap where every other kind leaves it",
        );
        assert_eq!(
            crate::outcome::block_payload(&doc, code),
            "    indented code line\n",
            "the payload is still the block's complete indented spelling",
        );
        assert_eq!(
            code.source_hash,
            crate::id::source_hash_bytes("    indented code line\n".as_bytes()),
            "the hash follows the range, BOM excluded",
        );

        // The comparison that makes the rule a rule: a BOM ahead of a
        // paragraph or a fenced block is outside that block too.
        for other in [
            format!("{BOM}Intro paragraph.\n"),
            format!("{BOM}```\nfenced\n```\n"),
        ] {
            let doc = parse(&other).expect("parses");
            assert_eq!(
                doc.blocks[0].source_range.start,
                BOM.len(),
                "a BOM is outside the first block for {other:?}",
            );
        }
    }

    #[test]
    fn a_fenced_block_is_untouched_by_the_snap() {
        assert_eq!(
            blocks("Intro.\n\n```rust\nlet x = 1;\n```\n\nOutro.\n"),
            vec![
                ("paragraph", "Intro.".to_string()),
                ("code-block", "```rust\nlet x = 1;\n```".to_string()),
                ("paragraph", "Outro.".to_string()),
            ],
            "a fenced block already starts at column 1 — its range must not move",
        );
    }

    /// OPEN QUESTION resolved against comrak, not assumed: what does an
    /// indented block with an INTERIOR BLANK LINE report as its end?
    ///
    /// Answer (comrak, this workspace's pin): sourcepos `3:5 - 6:0` for the
    /// source below — the end is line 6 **column 0**, i.e. the start of the
    /// line AFTER the last content line, exactly the same shape as a
    /// single-line block's end. `ranges::byte_range_for` resolves that to the
    /// byte just past the final content line's `\n`, so the interior blank
    /// line is INSIDE the range and the trailing blank line is NOT. The
    /// snap therefore has to move only `start`; `end` is already right.
    #[test]
    fn an_interior_blank_line_stays_inside_the_block_and_the_trailing_one_does_not() {
        assert_eq!(
            code_payload("Intro.\n\n    a\n\n    b\n\nOutro.\n"),
            "    a\n\n    b\n",
        );
    }

    /// The kind must record HOW the source spelled the block, because that
    /// is what `unit::payload::assemble` branches on to decide whether the
    /// wire payload needs synthesizing.
    ///
    /// Was written against the derived `Debug` string so it would compile on
    /// the pre-fix tree; now that `BlockKind::CodeBlock` carries `fenced`,
    /// it asserts the field directly as its own doc comment instructed.
    #[test]
    fn the_kind_records_whether_the_source_spelled_the_block_fenced() {
        let kind_of = |src: &str| -> BlockKind {
            let doc = parse(src).expect("parses");
            doc.blocks
                .iter()
                .find(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
                .expect("the fixture has a code block")
                .kind
                .clone()
        };

        for src in [
            "Intro.\n\n    indented code line\n\nOutro.\n",
            "Intro.\n\n        indented code line\n\nOutro.\n",
            "Intro.\n\n\tindented code line\n\nOutro.\n",
        ] {
            let kind = kind_of(src);
            assert!(
                matches!(
                    kind,
                    BlockKind::CodeBlock {
                        fenced: false,
                        info: None
                    }
                ),
                "an indented block must record fenced: false, got {kind:?} for {src:?}",
            );
        }

        let kind = kind_of("Intro.\n\n```rust\nlet x = 1;\n```\n\nOutro.\n");
        assert!(
            matches!(kind, BlockKind::CodeBlock { fenced: true, .. }),
            "a fenced block must record fenced: true, got {kind:?}",
        );
    }

    /// The hash is a `CacheKey` axis, so it has to cover the bytes the unit
    /// actually carries. It is computed from the stored range, so widening
    /// the range must widen what is hashed — not leave the hash pinned to
    /// the dedented body.
    #[test]
    fn the_source_hash_covers_the_full_indented_spelling() {
        let src = "Intro.\n\n    line one\n    line two\n\nOutro.\n";
        let doc = parse(src).expect("parses");
        let block = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::CodeBlock { .. }))
            .expect("the fixture has a code block");
        assert_eq!(
            block.source_hash,
            crate::id::source_hash_bytes("    line one\n    line two\n".as_bytes()),
        );
    }
}
