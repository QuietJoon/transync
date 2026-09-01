//! Annotated HTML renderer.
//!
//! Emits fragment HTML — a single `<main>` wrapping the block tree, with
//! sync attributes on each block. The CLI wraps this in a templated demo
//! shell (per ADR-0006); WASM consumers bring their own shell.
//!
//! Block layout precondition: consumers using `block.offsetTop`-based
//! scroll math MUST set `position: relative` (or any non-static value)
//! on their scrollable pane element so the pane is the offsetParent of
//! every block. The renderer deliberately does NOT stamp positioning
//! on `<main>` — doing so would make `<main>` the offsetParent instead
//! of the consumer's pane, and any padding the consumer applies to
//! the pane would silently shift `offsetTop` by that padding. The
//! contract is documented in `docs/architecture/contracts.md` §4a.
//! See R0005-0001 + DCR-0003.
//!
//! Rendering strategy (spec 2026-08-04 §4, AST-direct): **one** Comrak
//! parse per pane — the source pane parses `doc.source_text`, the target
//! pane the regenerated translated Markdown — and the resulting top-level
//! node sequence is zipped against the alignment rows through the shared
//! [`crate::walk`] normalization. Each block's wrapper element still comes
//! from its `BlockKind`; its inner HTML now comes from formatting the
//! *paired AST node* (its children for the strip-kinds, the whole node for
//! table / code-block / blockquote), so no HTML string surgery is needed
//! and Comrak's parent-dependent arms — list tightness, `<tbody>` — stay
//! context-faithful. See `node_inner_html` for the per-kind table.
//!
//! TRACE: ADR-0006
//! TRACE: contracts.md §4

pub mod attrs;

use crate::align::{AlignmentBlock, AlignmentMap, FallbackStatus};
use crate::id::{BlockId, BlockKind};
use crate::parser::ranges::ByteRange;
use crate::parser::{self, Block, Document};
use crate::walk::{self, NormalizedEntry};
use comrak::nodes::{AstNode, ListType, NodeValue};
use std::collections::HashMap;
use std::fmt::Write;

const FRAGMENT_OPEN: &str = "<main>\n";
const FRAGMENT_CLOSE: &str = "</main>\n";

/// Why the renderer refused to render a pane.
///
/// The first three variants name a *producer* defect in the alignment map:
/// every shipped producer ([`crate::align::build_alignment_map`]) emits
/// exactly one row per block of the document it was built from, with byte
/// ranges into the string that document was regenerated as — so a map that
/// repeats an id, omits one, or carries a range that is not a slice of this
/// pane was not built against this document. Rendering it anyway is what the
/// renderer used to do, and all three silences were invisible — a duplicate
/// let the LAST row decide an id's attributes and byte ranges, an uncovered
/// block vanished from both panes with no placeholder and no warning
/// (R0002-0010, R0002-0011), and a corrupt range was clamped, reordered and
/// snapped into whatever bytes happened to survive (R0003-0060). Refusing is
/// the only answer that cannot be mistaken for a clean render.
///
/// Non-exhaustive: new refusals may be added in minor releases.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RenderError {
    /// Two or more alignment rows claim the same `source_block_id`.
    #[error("alignment map repeats source_block_id: {ids}")]
    DuplicateRow {
        /// The repeated ids, sorted, deduplicated, and bounded.
        ids: String,
    },
    /// A block of the document has no alignment row at all.
    #[error("alignment map has no row for block: {ids}")]
    UncoveredBlock {
        /// The uncovered ids in document order, bounded.
        ids: String,
    },
    /// A byte range this pane would slice is not a slice of this pane's
    /// Markdown — reversed, past its end, or landing mid-character.
    ///
    /// The source pane measures [`Block::source_range`] against
    /// `doc.source_text`; the target pane measures
    /// [`AlignmentBlock::target_range`] against the `translated_md` it was
    /// handed. Refusing was chosen over the clamp-and-snap that preceded it
    /// (R0003-0060): a coerced range still renders, so a block whose row
    /// points at the wrong bytes presents truncated, empty or unrelated
    /// content under a correct-looking anchor.
    #[error("alignment map has an unusable {pane} byte range: {ids}")]
    UnusableRange {
        /// Which pane's Markdown the ranges were measured against —
        /// `"source"` or `"target"`.
        pane: &'static str,
        /// The offending ids, each with its offsets and its fault, in
        /// document order, bounded.
        ids: String,
    },
    /// The pane's Markdown was refused by [`parser::intake`] — the same
    /// guard [`parser::parse`] applies to a source document.
    #[error("pane Markdown refused at parser intake: {0}")]
    Intake(#[from] crate::error::ParseError),
}

/// At most this many ids are named in a [`RenderError`] message; the rest
/// are counted. A wholly foreign map can miss every block of a large
/// document, and an error message is a diagnostic, not a dump.
///
/// Deliberately smaller than the crate's other bounded id sample, the local
/// `SAMPLE` (8) in `align::build_alignment_map`'s incomplete-`BlockOffsets`
/// warning — the two are read side by side when diagnosing one mismatch, so
/// the difference is stated rather than left to guess. This one is spliced
/// into a one-line `Display` message that must also carry its own overflow
/// count (`(+N more)`) inline; that one is a structured `tracing` field
/// sitting beside separate `missing` / `total` fields which keep the
/// magnitude whatever the sample drops. Two knobs on two message shapes,
/// neither a contract — not one knob read twice.
const MAX_NAMED_IDS: usize = 5;

fn name_ids(ids: &[&str]) -> String {
    if ids.len() <= MAX_NAMED_IDS {
        return ids.join(", ");
    }
    format!(
        "{} (+{} more)",
        ids[..MAX_NAMED_IDS].join(", "),
        ids.len() - MAX_NAMED_IDS
    )
}

/// Render the source pane fragment.
///
/// One wrapper element per source block. The parser is leaf-block, so every
/// block IS a top-level DOM element: a list item, a blockquote, or a table
/// owns its whole nested payload, and Comrak's HTML for that node already
/// contains the descendants. Nothing is ever re-emitted as a sibling of its
/// own container — the duplicate-`<li>` shape that motivated the original
/// `parent_id == None` filter is unreachable by construction, which is why
/// the filter is gone rather than merely always-true.
///
/// Fallible since R0002-0010 / R0002-0011: `alignment` must describe `doc`
/// exactly — one row per block, no repeats — or the pane is refused rather
/// than rendered incomplete. See [`RenderError`].
///
/// TRACE: SCN-12
/// TRACE: ADR-0006
pub fn render_source(doc: &Document, alignment: &AlignmentMap) -> Result<String, RenderError> {
    render_fragment(doc, alignment, Pane::Source)
}

/// Render the target pane fragment from the regenerated translated MD.
/// Same block set, same refusals as [`render_source`], plus the
/// [`parser::intake`] guard on `translated_md` itself (R0002-0059).
///
/// TRACE: SCN-12
/// TRACE: ADR-0006
pub fn render_target(
    doc: &Document,
    translated_md: &str,
    alignment: &AlignmentMap,
) -> Result<String, RenderError> {
    render_fragment(doc, alignment, Pane::Target(translated_md))
}

/// Which pane is being rendered. Selects both the Markdown that gets
/// parsed (one parse per pane) and — for the bypass and Guard-2 degrade
/// arms, the only paths that still read raw bytes — which byte range of
/// it a block owns.
#[derive(Clone, Copy)]
enum Pane<'a> {
    /// The source document's own bytes; ranges come from the IR.
    Source,
    /// The regenerated translated Markdown; ranges come from the
    /// alignment row (`regen::BlockOffsets` fed them there).
    Target(&'a str),
}

impl<'a> Pane<'a> {
    fn md(&self, doc: &'a Document) -> &'a str {
        match self {
            Pane::Source => doc.source_text.as_str(),
            Pane::Target(md) => md,
        }
    }

    fn range(&self, row: &AlignmentBlock, block: &Block) -> ByteRange {
        match self {
            Pane::Source => block.source_range,
            Pane::Target(_) => row.target_range,
        }
    }

    /// Which pane a [`RenderError::UnusableRange`] measured against. Also
    /// names WHICH field is at fault, since the two panes read different
    /// ones: `source_range` off the block, `target_range` off the row.
    fn label(&self) -> &'static str {
        match self {
            Pane::Source => "source",
            Pane::Target(_) => "target",
        }
    }
}

/// Why `range` is not a slice of `md`, or `None` when it is one.
///
/// The three faults are checked in an order that keeps each check meaningful:
/// bounds before boundaries, because [`str::is_char_boundary`] answers
/// `false` for every index past the end and would otherwise report an
/// out-of-range offset as a mid-character one. `start > md.len()` needs no
/// arm of its own — with `start <= end` established, an out-of-range start
/// implies an out-of-range end.
///
/// An empty in-bounds range (`start == end`) is NOT a fault:
/// [`crate::align::build_alignment_map`] emits `0..0` for a block its
/// `BlockOffsets` did not cover, and says so through a warning (R0003-0055).
fn range_fault(md: &str, range: ByteRange) -> Option<&'static str> {
    if range.end < range.start {
        return Some("end precedes start");
    }
    if range.end > md.len() {
        return Some("end is past the pane's Markdown");
    }
    if !md.is_char_boundary(range.start) || !md.is_char_boundary(range.end) {
        return Some("offsets fall inside a UTF-8 character");
    }
    None
}

/// Shared fragment renderer: parses the pane's Markdown ONCE, walks the
/// document's normalized top-level entries in source order, and emits one
/// wrapper element per block with the paired AST node's HTML inside it.
///
/// The zip consumes [`crate::walk::normalize_top_level`] — the SAME
/// normalized view `validate::full_reparse` compares against — so the
/// grouping rule has exactly one home. OI-0010: consecutive list items
/// belonging to the same source list (they share their `ast_path` prefix)
/// collapse into ONE normalized `list` entry, which pairs with ONE Comrak
/// `List` node and renders as one shared `<ul>`/`<ol>` with the sync
/// attributes on each `<li>`. A marker-type change is a different
/// `ast_path` prefix, hence a second entry and a second list, so `- a` /
/// `* b` still produce two lists. The group element carries no *sync*
/// attributes — sync identity stays on the per-item `<li>` anchors
/// (contracts.md §4/§4a as amended by DCR-0007); its only attributes are
/// presentational, a non-1 `start` and the task-list class described on
/// [`render_list_group`].
///
/// Guard 2 (spec §4.3): `node_idx` only advances when the node at the
/// cursor actually carries the entry's normalized label. A row that
/// cannot pair degrades to its own byte range and leaves the cursor
/// where it was, so a single mismatch can never shift every later block.
fn render_fragment<'p>(
    doc: &'p Document,
    alignment: &'p AlignmentMap,
    pane: Pane<'p>,
) -> Result<String, RenderError> {
    let ctx = PaneCtx::new(doc, alignment, pane)?;

    // R0002-0059: the pane's Markdown meets the SAME intake the source
    // document met — `parser::intake`, hence the same nesting ceiling and
    // the same NUL rule — instead of going to Comrak unguarded. Only the
    // parse consumes the normalized string: `ctx.md` stays the caller's own
    // bytes, because the byte ranges the bypass and degrade arms slice
    // (`Block::source_range`, `AlignmentBlock::target_range`) index THOSE.
    // Substituting NUL for the parse changes no AST — Comrak performs the
    // same substitution internally, before it records a position — so the
    // guard is the part that was missing, not the normalization.
    let md_for_parse = parser::intake(ctx.md)?;

    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, &md_for_parse, &ctx.opts);
    let top: Vec<&AstNode<'_>> = root.children().collect();

    let mut out = String::new();
    out.push_str(FRAGMENT_OPEN);

    let mut node_idx = 0usize;
    for entry in walk::normalize_top_level(doc) {
        let node = top
            .get(node_idx)
            .copied()
            .filter(|n| walk::node_label(n) == Some(entry.label));

        if entry.label == "list" {
            render_list_group(&mut out, &entry, node, &ctx);
        } else if let Some((block, row)) = entry.sources.first().and_then(|id| ctx.pair(id)) {
            render_block(&mut out, block, row, node, &ctx);
        }

        if node.is_some() {
            node_idx += 1;
        }
    }
    out.push_str(FRAGMENT_CLOSE);
    Ok(out)
}

/// Everything the per-row emission needs that is constant across a pane:
/// the source-block and alignment-row indexes, which pane is being
/// rendered (with its Markdown), and the Comrak options the pane was
/// parsed with — the same options every node re-format must use.
struct PaneCtx<'p> {
    by_id: HashMap<&'p BlockId, &'p Block>,
    rows: HashMap<&'p BlockId, &'p AlignmentBlock>,
    pane: Pane<'p>,
    md: &'p str,
    opts: comrak::ComrakOptions<'static>,
}

impl<'p> PaneCtx<'p> {
    /// Index the document and the map, refusing a map that does not
    /// describe this document.
    ///
    /// Three refusals, all of them silences the emission path used to
    /// swallow:
    ///
    /// - **Duplicate rows** (R0002-0010). The index is a `HashMap` keyed by
    ///   `source_block_id`, so a repeated id used to mean "last row wins" —
    ///   one row's attributes, order and byte ranges quietly replaced by
    ///   another's, with no signal anywhere.
    /// - **Uncovered blocks** (R0002-0011). [`Self::pair`] answers `None`
    ///   for a block with no row and the caller skips it, so the block
    ///   disappeared from BOTH panes — no anchor, no placeholder, no
    ///   warning — while every other block still rendered, which is exactly
    ///   what an incomplete render must never look like.
    /// - **Unusable ranges** (R0003-0060). The 0002 pair drew the line at
    ///   map *shape*; a row whose range was reversed, out of bounds or
    ///   mid-character still rendered, because `block_text` clamped it,
    ///   reordered it and snapped it to boundaries. That kept the renderer
    ///   panic-free — and produced a truncated, empty or unrelated slice
    ///   under a correct-looking anchor on every path that reads raw bytes
    ///   (the html and skipped bypass arms and the Guard-2 degrade arm).
    ///   Checking the ranges here, once per pane, is what lets `block_text`
    ///   slice instead of coerce.
    ///
    /// Only the ranges THIS pane reads are checked, against the Markdown
    /// THIS pane slices: `source_range` off each block for the source pane,
    /// `target_range` off each row for the target pane. Every block is
    /// checked, not only the ones whose kind reads bytes today, because
    /// which rows take a byte-reading arm is not known until the pane's
    /// nodes are paired — Guard 2 can send any row there.
    ///
    /// Rows naming blocks this document does not have are NOT refused: they
    /// are inert (nothing ever looks them up, so neither does the range
    /// check) and a caller rendering a sub-document against a wider map is
    /// asking for something coherent.
    fn new(
        doc: &'p Document,
        alignment: &'p AlignmentMap,
        pane: Pane<'p>,
    ) -> Result<Self, RenderError> {
        let mut rows: HashMap<&'p BlockId, &'p AlignmentBlock> =
            HashMap::with_capacity(alignment.blocks.len());
        let mut duplicates: Vec<&str> = Vec::new();
        for row in alignment.blocks.iter() {
            if rows.insert(&row.source_block_id, row).is_some() {
                duplicates.push(row.source_block_id.0.as_str());
            }
        }
        if !duplicates.is_empty() {
            duplicates.sort_unstable();
            duplicates.dedup();
            return Err(RenderError::DuplicateRow {
                ids: name_ids(&duplicates),
            });
        }

        let missing: Vec<&str> = doc
            .blocks
            .iter()
            .filter(|b| !rows.contains_key(&b.block_id))
            .map(|b| b.block_id.0.as_str())
            .collect();
        if !missing.is_empty() {
            return Err(RenderError::UncoveredBlock {
                ids: name_ids(&missing),
            });
        }

        let md = pane.md(doc);
        let faults: Vec<String> = doc
            .blocks
            .iter()
            .filter_map(|block| {
                // Every block has a row — the check above just proved it —
                // so a miss here is impossible rather than skipped.
                let row = rows.get(&block.block_id)?;
                let range = pane.range(row, block);
                let why = range_fault(md, range)?;
                Some(format!(
                    "{} ({}..{}: {why})",
                    block.block_id.0, range.start, range.end
                ))
            })
            .collect();
        if !faults.is_empty() {
            let named: Vec<&str> = faults.iter().map(String::as_str).collect();
            return Err(RenderError::UnusableRange {
                pane: pane.label(),
                ids: name_ids(&named),
            });
        }

        Ok(PaneCtx {
            by_id: doc.blocks.iter().map(|b| (&b.block_id, b)).collect(),
            rows,
            md,
            pane,
            opts: parser::comrak_options(),
        })
    }

    /// The source block and alignment row for one normalized source ID.
    ///
    /// Both sides are present for every id [`crate::walk::normalize_top_level`]
    /// can produce — `by_id` is built from the same `doc.blocks` the entries
    /// come from, and [`Self::new`] refused any map that left a block
    /// uncovered. The `Option` survives so the emission path stays
    /// panic-free rather than asserting that check a second time per row.
    fn pair(&self, id: &BlockId) -> Option<(&'p Block, &'p AlignmentBlock)> {
        Some((*self.by_id.get(id)?, *self.rows.get(id)?))
    }
}

/// Emit one collapsed `list` entry: the shared group tag, then one `<li>`
/// per contributing row paired positionally with the `List` node's direct
/// `Item`/`TaskItem` children (`validate::full_reparse`'s Guard-1 item
/// count is what makes that pairing safe).
///
/// R0001-0026: a group holding at least one task row also carries
/// `class="contains-task-list"` — the GFM task-list marker that rendered
/// GitHub Markdown carries and that task-list styling and integrations key
/// on. This renderer is the only place it can come from: the pinned comrak
/// writes it nowhere (its `List` arm emits a bare `<ul>`/`<ol>`, with
/// `start` its only attribute, and its `TaskItem` arm a bare `<li>` plus
/// the checkbox `<input>`), and that build exposes no option to turn the
/// classes on. So the finding's "Comrak's own classes" is about the GFM
/// convention comrak's output is *read as*, not about bytes comrak ever
/// wrote — reconstructing the group here (DCR-0007) is what makes the
/// convention ours to keep, and the checkbox surviving alone is not the
/// same DOM. The class leads the group tag, ahead of `start`, so the two
/// presentational attributes have one fixed order; `<ol>` carries it too,
/// since a task list may be ordered. The per-row `class="task-list-item"`
/// half is [`task_item_class`].
///
/// Accepted gap (ticket `cfeb5df5`, weighed and accepted 2026-08-07 —
/// contracts.md §4/§4a states it, DCR-0007 records why): only this
/// reconstructed TOP-LEVEL group is marked. A task list nested inside an
/// item never reaches this code — it is a child node, so
/// [`node_inner_html`] hands it to comrak whole and it comes back
/// classless. Marking it would mean either bumping the parser every block
/// ID depends on or rendering nested lists here; neither is worth two CSS
/// classes, so downstream keys on the checkbox `<input>` instead.
fn render_list_group<'a>(
    out: &mut String,
    entry: &NormalizedEntry,
    node: Option<&'a AstNode<'a>>,
    ctx: &PaneCtx<'_>,
) {
    // EXT-2026-07 P2-9 (OI-0022): an ordered list that opens at a number
    // other than 1 needs an explicit `<ol start="N">` or the browser
    // renumbers from 1, silently dropping the source offset (DCR-0007's
    // known limitation, resolved). N and the tag now come straight off the
    // paired `List` node — no second parse of the first item's fragment.
    let list = node.and_then(|n| match &n.data.borrow().value {
        NodeValue::List(l) => Some((l.list_type, l.start)),
        _ => None,
    });
    let (tag, start) = match list {
        Some((ListType::Ordered, start)) => ("ol", start),
        Some((ListType::Bullet, _)) => ("ul", 1),
        // Guard 2: no paired `List` node. Every row degrades to its bytes,
        // but the group still opens so the `<li>` anchors stay inside a
        // list; the tag comes from the first row's own kind.
        None => {
            let ordered = entry
                .sources
                .first()
                .and_then(|id| ctx.by_id.get(id))
                .is_some_and(|b| matches!(b.kind, BlockKind::ListItem { ordered: true, .. }));
            (if ordered { "ol" } else { "ul" }, 1)
        }
    };
    let items: Vec<&'a AstNode<'a>> = node
        .map(|n| {
            n.children()
                .filter(|c| {
                    matches!(
                        c.data.borrow().value,
                        NodeValue::Item(_) | NodeValue::TaskItem(_)
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    // The task-list class rides the SAME signal the checkbox `<input>` does
    // — the paired `TaskItem` node — so the class is present exactly when at
    // least one row in this group renders a checkbox. On the Guard-2 degrade
    // path there is no paired node, hence no checkbox and no class: the two
    // stay in lockstep by construction rather than by agreement between two
    // derivations.
    let group_class = if items
        .iter()
        .any(|item| matches!(item.data.borrow().value, NodeValue::TaskItem(_)))
    {
        " class=\"contains-task-list\""
    } else {
        ""
    };
    if tag == "ol" && start != 1 {
        let _ = writeln!(out, "<{tag}{group_class} start=\"{start}\">");
    } else {
        let _ = writeln!(out, "<{tag}{group_class}>");
    }

    // Advance the item cursor per ROW (not per emitted `<li>`) so a row
    // the alignment map is missing cannot shift the rest of the run.
    for (i, id) in entry.sources.iter().enumerate() {
        if let Some((block, row)) = ctx.pair(id) {
            render_block(out, block, row, items.get(i).copied(), ctx);
        }
    }

    let _ = writeln!(out, "</{tag}>");
}

/// Emit one block: its sync wrapper plus the paired node's HTML.
///
/// `node` is `None` on the Guard-2 degrade path (and unused by the three
/// bypass arms, which present their own source bytes by design).
fn render_block<'a>(
    out: &mut String,
    block: &Block,
    align_block: &AlignmentBlock,
    node: Option<&'a AstNode<'a>>,
    ctx: &PaneCtx<'_>,
) {
    let kind = &block.kind;
    if matches!(kind, BlockKind::ThematicBreak) {
        // R0006-0043: thematic breaks are `SyncRole::NonSync` — they have
        // no sync identity, so the sync-attribute set (data-sync-id,
        // data-order, data-fallback) is omitted per ADR-0006; only
        // data-block-kind remains as a styling hook. The JS engine must
        // therefore never expect an <hr> anchor (its alignment row says
        // non-sync).
        //
        // That omission is no longer decided here (ti `18b9c3`).
        // `attrs::write_attrs` reads the ROW's `sync_role`, so every
        // non-sync kind gets the same treatment without a second literal
        // list to keep in step — this arm remains only because an `<hr>`
        // needs a different ELEMENT, not different attributes.
        //
        // <hr> is an HTML void element — no content, no closing tag.
        // The XML-style self-close (`<hr ... />`) is valid in HTML5 but
        // some serializers and validators flag it; emit the canonical
        // void-element form.
        let element = wrapper_element_for(kind);
        let attrs = attrs::write_attrs(align_block);
        let _ = writeln!(out, "<{element}{attrs}>");
        return;
    }
    if matches!(kind, BlockKind::Html) {
        // Spec §5: live render is a translated/preserved privilege; fallback
        // (and extraction-failure, which align marks fallback_source) keeps
        // the DCR-0013 escaped placeholder. Live output is auto-balanced
        // (spec §3.4) so an unbalanced fragment can never swallow sibling
        // sync wrappers after the DOMPurify innerHTML mount.
        //
        // Bypass arm (spec §4.2): Comrak is never involved — with
        // `unsafe_ = false` its HtmlBlock arm writes `<!-- raw HTML
        // omitted -->` — so this presentation reads the block's own bytes
        // out of the pane. The node still advanced the walk.
        let md = block_text(ctx.md, ctx.pane.range(align_block, block));
        let attrs = attrs::write_attrs(align_block);
        if matches!(align_block.fallback_status, FallbackStatus::FallbackSource) {
            let _ = writeln!(
                out,
                "<pre{attrs} data-skipped=\"html-block\">{}</pre>",
                html_escape(md),
            );
        } else {
            // OI-0035 route (c), render half (spec 2026-08-20 §8): strip the
            // reserved sync-attribute namespace out of the block's own bytes
            // before the wrapper writes ours. Strip-then-inject is what makes
            // "ours are the only sync attributes in this DOM" a construction
            // rather than a scan — the engine's row gate covers unlisted ids,
            // but a LISTED id planted ahead of the genuine anchor wins
            // first-occurrence-wins, and only this layer closes that.
            //
            // Pane-only. `out.md` keeps the author's bytes: their
            // `data-sync-id` is their content, and we own this namespace only
            // in DOM we mount. Stripping changes paint only for markup that
            // borrowed the engine-owned namespace's own presentation — the
            // bundle shell tints `[data-fallback=…]` and `pre[data-skipped]`
            // — which is ours to decide rather than the author's. ("Stripping
            // never changes rendered appearance, because attributes do not
            // paint" was an overclaim; corrected ti 490d97 wave 1.)
            //
            // The failure arm above needs none of this: its payload is
            // HTML-escaped, so an impostor attribute there is text.
            let _ = writeln!(
                out,
                "<div{attrs}>{}</div>",
                transync_html::balance_fragment(&transync_html::strip_reserved_sync_attrs(md)),
            );
        }
        return;
    }
    if let BlockKind::Skipped { label } = kind {
        // A6 / invariant 7: never render the payload live — Comrak with
        // `render.unsafe_ = false` would replace raw HTML with
        // `<!-- raw HTML omitted -->`, losing the content again. Bypass arm
        // (spec §4.2): emit the source bytes HTML-escaped inside an inert
        // `<pre>` placeholder that still carries the full sync-attribute
        // set, so the JS engine anchors it with zero JS changes. The
        // `data-skipped` marker names the underlying node kind for shell
        // styling / legend text.
        let md = block_text(ctx.md, ctx.pane.range(align_block, block));
        let attrs = attrs::write_attrs(align_block);
        let _ = writeln!(
            out,
            "<pre{attrs} data-skipped=\"{}\">{}</pre>",
            attrs::escape_attr(label),
            html_escape(md),
        );
        return;
    }
    let attrs = attrs::write_attrs(align_block);
    let inner = match node {
        Some(node) => node_inner_html(kind, node, &ctx.opts),
        // Guard 2 (spec §4.3): reachable, and not only in theory. The
        // pipeline never gets here — it renders only MD whose reparse
        // matched the source structure — but `transync-wasm`'s
        // `engine::rebuild_impl`, the browser demo's edit loop, renders
        // with no such precondition: a payload that changes a block's
        // topology (`# X\n\npara` typed over a paragraph) leaves the pane's
        // node sequence longer than the normalized entry sequence, so the
        // entry at the seam finds a differently-labelled node at the cursor.
        // The observed degrade: that block falls back to its byte range,
        // escaped (those bytes are Markdown, not HTML), so its anchor
        // survives and nothing panics — but the cursor deliberately does not
        // advance past the interloper node, so LATER entries with the same
        // label can pair with the wrong node, i.e. shifted content under
        // correct anchors until the payload is fixed. That is the designed
        // safe fallback rather than a defect; `rebuild_impl` compares the two
        // normalized top-level fingerprints — count, label sequence, and each
        // collapsed list's item count (R0002-0014: a same-count substitution
        // like plain text over a blockquote degrades identically) — and
        // returns a `structure_warning` so the demo can announce the degrade
        // instead of presenting it as a clean render.
        None => html_escape(block_text(ctx.md, ctx.pane.range(align_block, block)).trim()),
    };
    let element = wrapper_element_for(kind);
    let class = task_item_class(kind, node);
    let _ = writeln!(out, "<{element}{class}{attrs}>{inner}</{element}>");
}

/// R0001-0026: `class="task-list-item"` for a list row whose paired node is
/// a `TaskItem`, the empty string for every other row.
///
/// The paired `TaskItem` node is the same signal [`node_inner_html`] reads
/// to write the checkbox `<input>`, so class and checkbox are inseparable
/// by construction — including on the Guard-2 degrade path, where neither
/// is emitted because the row presents escaped source bytes, not a rendered
/// item. Reading the NODE rather than `BlockKind::ListItem`'s `task` field
/// is what keeps the target pane honest too: the source block still says
/// "task" for a row whose translated payload dropped its `[ ]` marker, and
/// such a row must not claim a class for a checkbox it no longer renders.
///
/// The class leads the sync attributes; `attrs::write_attrs` still owns the
/// sync set verbatim, so every anchor's attributes are byte-identical to
/// what they were before the class existed. GFM's third task-list class,
/// `task-list-item-checkbox` on the `<input>`, is deliberately not emitted:
/// [`node_inner_html`] reproduces the checkbox byte-for-byte as the pinned
/// comrak writes it, and a class there would make this checkbox differ from
/// every other checkbox comrak emits — the nested ones included.
fn task_item_class<'a>(kind: &BlockKind, node: Option<&'a AstNode<'a>>) -> &'static str {
    match (kind, node) {
        (BlockKind::ListItem { .. }, Some(node))
            if matches!(node.data.borrow().value, NodeValue::TaskItem(_)) =>
        {
            " class=\"task-list-item\""
        }
        _ => "",
    }
}

/// A block's inner HTML, straight out of Comrak's formatter — the spec §4.1
/// per-kind emission table:
///
/// | kinds | emission |
/// |---|---|
/// | Heading1–6, Paragraph, Image | the node's **children** (for `Image` the node IS the image-only paragraph, so its children are the bare `<img>`s the `<figure>` wrapper wants) |
/// | ListItem | the `Item`/`TaskItem` node's **children**, after the checkbox `<input>` prefix a `TaskItem` needs (Comrak writes that in the item's OPEN tag, which children-format never reaches) |
/// | Table, CodeBlock, Blockquote | the **whole node** — its own `<table>` / `<pre><code>` / `<blockquote>` element belongs inside our transparent `<div>`. `CodeBlock` is a childless leaf (the literal lives on the node) so children-format would emit nothing at all, and `Table`'s `</tbody>` is written only by the Table exit arm |
///
/// `Html`, `Skipped` and `ThematicBreak` never reach here: `render_block`
/// emits their bypass presentation and returns.
fn node_inner_html<'a>(kind: &BlockKind, node: &'a AstNode<'a>, opts: &comrak::Options) -> String {
    let mut buf: Vec<u8> = Vec::new();
    match kind {
        BlockKind::Table | BlockKind::CodeBlock { .. } | BlockKind::Blockquote => {
            // `format_html` renders exactly the argument subtree, opening
            // and closing tags included.
            let _ = comrak::format_html(node, opts, &mut buf);
        }
        BlockKind::ListItem { .. } => {
            let checked = match &node.data.borrow().value {
                NodeValue::TaskItem(symbol) => Some(symbol.is_some()),
                _ => None,
            };
            // Byte-identical to Comrak's own TaskItem open-tag arm, so the
            // DOM is the same whichever path produced it.
            match checked {
                Some(true) => buf
                    .extend_from_slice(b"<input type=\"checkbox\" checked=\"\" disabled=\"\" /> "),
                Some(false) => buf.extend_from_slice(b"<input type=\"checkbox\" disabled=\"\" /> "),
                None => {}
            }
            format_children(node, opts, &mut buf);
        }
        _ => format_children(node, opts, &mut buf),
    }
    let mut html = String::from_utf8_lossy(&buf).into_owned();
    // One-line-per-block `writeln!` shape: the wrapper closes right after
    // the block's own last byte (scn_08 pins the open tag's line).
    if html.ends_with('\n') {
        html.pop();
    }
    html
}

/// Append the HTML of `node`'s children to `buf` — the block's inner HTML,
/// with no string surgery on Comrak's output.
///
/// Each `format_html` call resets Comrak's `WriteWithLast`, so the newline
/// its `cr()` would have written before a block-level sibling is missing;
/// [`needs_cr`] puts back exactly those newlines — and only those, because
/// inline children must stay glued together (`**a**b` may not gain a line
/// break between the `<strong>` and the `b`).
fn format_children<'a>(node: &'a AstNode<'a>, opts: &comrak::Options, buf: &mut Vec<u8>) {
    for child in node.children() {
        if needs_cr(child) && !buf.is_empty() && buf.last() != Some(&b'\n') {
            buf.push(b'\n');
        }
        let _ = comrak::format_html(child, opts, buf);
    }
}

/// Whether Comrak's own whole-document walk would call `cr()` before this
/// node. Block-level nodes open with a block tag and do — except a
/// paragraph rendered tight, whose arm skips the open branch (and its
/// `cr()`) entirely. Inline nodes never do.
fn needs_cr<'a>(node: &'a AstNode<'a>) -> bool {
    let (is_block, is_paragraph) = {
        let data = node.data.borrow();
        (
            data.value.block(),
            matches!(data.value, NodeValue::Paragraph),
        )
    };
    is_block && !(is_paragraph && paragraph_is_tight(node))
}

/// Comrak's paragraph tightness rule, mirrored: the grandparent `List`'s
/// `tight` flag decides whether the `<p>` wrapper is emitted at all. The
/// parent links survive in the arena, so this is the same answer Comrak
/// itself computes — which is exactly why the AST-direct renderer reports
/// source-driven looseness faithfully where per-fragment reparse could not
/// (spec §4.4).
///
/// Options guard: comrak applies the same grandparent-`List` test to
/// `DescriptionTerm` paragraphs, which this mirror would mis-handle. The
/// description-list extension is off in [`crate::parser::comrak_options`], so
/// such nodes cannot occur today — revisit if that extension is enabled.
fn paragraph_is_tight<'a>(node: &'a AstNode<'a>) -> bool {
    node.parent()
        .and_then(|n| n.parent())
        .is_some_and(|grand| matches!(&grand.data.borrow().value, NodeValue::List(l) if l.tight))
}

fn wrapper_element_for(kind: &BlockKind) -> &'static str {
    // For block kinds whose Comrak HTML already includes the semantic
    // element (`<table>`, `<pre>`, `<ul><li>...`, `<blockquote>`), we use
    // a transparent `<div>` so the sync-id wrapper does not nest a
    // second copy of the same element.
    match kind {
        BlockKind::Heading1 => "h1",
        BlockKind::Heading2 => "h2",
        BlockKind::Heading3 => "h3",
        BlockKind::Heading4 => "h4",
        BlockKind::Heading5 => "h5",
        BlockKind::Heading6 => "h6",
        BlockKind::Paragraph => "p",
        BlockKind::Table => "div",
        BlockKind::CodeBlock { .. } => "div",
        // OI-0010: items render as real `<li>` anchors inside the shared
        // `<ul>`/`<ol>` group opened by `render_fragment`.
        BlockKind::ListItem { .. } => "li",
        BlockKind::Blockquote => "div",
        BlockKind::ThematicBreak => "hr",
        BlockKind::Image => "figure",
        // D5: a `<title>` is never in a pane — it has no anchor and no
        // presentation here, and the Markdown renderer never sees an HTML
        // document at all. A transparent `<div>` keeps the match total without
        // inventing a presentation for a block that has none.
        BlockKind::Title => "div",
        // Spec §5: a transparent `<div>` — same pattern as table / code-block
        // / blockquote, whose payload already carries its own semantic
        // elements. (The `Html` branch in `render_block` emits the wrapper
        // directly and returns; this arm keeps the match total.)
        BlockKind::Html => "div",
        // A6: `<pre>` preserves the source formatting of the escaped
        // placeholder, is visually distinct without CSS, and is inert.
        // (The `Skipped` branch in `render_block` emits the `<pre>`
        // directly and returns; this arm keeps the match total.)
        BlockKind::Skipped { .. } => "pre",
    }
}

/// Slice `source` by `range` — the pane's own bytes for the block, borrowed.
///
/// No clamping, no reordering, no boundary snapping: [`PaneCtx::new`]
/// checked every range this pane can reach ([`range_fault`]) and refused the
/// render outright if one was not a slice, so by the time a range gets here
/// it IS one (R0003-0060). This used to be the coercion site, and coercion
/// is what made a corrupt range look like content: a reversed range became an
/// empty block, an out-of-range one a truncated block, and a mid-character
/// one a block missing the character it split — each of them rendered under
/// the row's correct anchor, with nothing anywhere saying the bytes were not
/// the block's.
///
/// `get` rather than `&source[..]` keeps the residual — a future caller that
/// builds a `PaneCtx` some other way — a wrong answer instead of a panic, and
/// the `debug_assert` fails the test suite if that ever happens.
///
/// TRACE: SCN-12
fn block_text(source: &str, range: crate::align::ByteRange) -> &str {
    debug_assert!(
        range_fault(source, range).is_none(),
        "block_text got a range PaneCtx::new should have refused: {range:?}"
    );
    source.get(range.start..range.end).unwrap_or_default()
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Wire-form string for a `FallbackStatus`, matching the JSON enum.
///
/// TRACE: contracts.md §4
pub fn fallback_status_str(status: FallbackStatus) -> &'static str {
    match status {
        FallbackStatus::Translated => "translated",
        FallbackStatus::Preserved => "preserved",
        FallbackStatus::PartiallyTranslated => "partially_translated",
        FallbackStatus::FallbackSource => "fallback_source",
    }
}

// R0002-0010 / R0002-0011 / R0002-0059 / R0003-0060: the renderer's
// preconditions are checked, not assumed. Each test states the silence the
// check replaced.
#[cfg(test)]
mod map_integrity_tests {
    use super::*;
    use crate::align::build_alignment_map;
    use crate::id;

    /// Parse `src`, assign ids, and build the map every shipped producer
    /// would build for it — one row per block, in document order.
    fn doc_and_map(src: &str) -> (Document, AlignmentMap) {
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        (doc, map)
    }

    /// The same, but with the map built against the document's own
    /// regenerated Markdown, so every `target_range` is a real slice of the
    /// string handed to [`render_target`] — the precondition a caller of
    /// that function is required to keep. `doc_and_map`'s empty
    /// `BlockOffsets` cannot express a corrupt range: it makes every
    /// `target_range` `0..0`.
    fn doc_map_and_target(src: &str) -> (Document, AlignmentMap, String) {
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        let (translated, offsets) = crate::regen::regenerate(&doc, &HashMap::new());
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        (doc, map, translated)
    }

    /// The honest map renders — the checks refuse malformed maps, not
    /// working ones. Control for the two refusal tests below.
    #[test]
    fn a_map_built_from_the_document_still_renders() {
        let (doc, map) = doc_and_map("# Title\n\nbody\n");
        let html = render_source(&doc, &map).expect("a conformant map renders");
        assert!(html.contains("data-sync-id=\"h1-0001\""), "{html}");
        assert!(html.contains("data-sync-id=\"p-0002\""), "{html}");
    }

    /// R0002-0010: a repeated `source_block_id` used to mean "last row
    /// wins" — the row index is a `HashMap`, so the duplicate silently took
    /// over the id's attributes and byte ranges. The duplicate below carries
    /// a different `fallback_status`, which is what the old renderer would
    /// have stamped on the anchor; now the pane is refused instead.
    #[test]
    fn a_repeated_row_id_is_refused_instead_of_letting_the_last_row_win() {
        let (doc, mut map) = doc_and_map("# Title\n\nbody\n");
        let mut dup = map.blocks[0].clone();
        dup.fallback_status = FallbackStatus::FallbackSource;
        map.blocks.push(dup);

        let err = render_source(&doc, &map).expect_err("a repeated row id is refused");
        assert!(
            matches!(&err, RenderError::DuplicateRow { ids } if ids == "h1-0001"),
            "expected the repeated id to be named: {err}"
        );
        // The target pane refuses on the same grounds, before it parses
        // anything.
        assert!(matches!(
            render_target(&doc, "# Title\n\nbody\n", &map),
            Err(RenderError::DuplicateRow { .. })
        ));
    }

    /// R0002-0011: a block with no row rendered as *nothing* — no anchor, no
    /// placeholder, no warning — while every other block rendered normally,
    /// so an incomplete pane was indistinguishable from a complete one.
    #[test]
    fn a_block_with_no_row_is_refused_instead_of_vanishing_from_the_pane() {
        let (doc, mut map) = doc_and_map("# Title\n\nbody\n");
        let dropped = map.blocks.remove(1);
        assert_eq!(dropped.source_block_id.0, "p-0002");

        let err = render_source(&doc, &map).expect_err("an uncovered block is refused");
        assert!(
            matches!(&err, RenderError::UncoveredBlock { ids } if ids == "p-0002"),
            "expected the uncovered block to be named: {err}"
        );
    }

    /// The message names a bounded sample rather than dumping every id.
    #[test]
    fn refusal_messages_bound_the_id_list() {
        let src = "a\n\nb\n\nc\n\nd\n\ne\n\nf\n\ng\n";
        let (doc, mut map) = doc_and_map(src);
        map.blocks.clear();
        let err = render_source(&doc, &map).expect_err("nothing is covered");
        let RenderError::UncoveredBlock { ids } = &err else {
            panic!("expected UncoveredBlock: {err}");
        };
        assert!(ids.ends_with("(+2 more)"), "unbounded id list: {ids}");
        assert_eq!(ids.matches(", ").count(), MAX_NAMED_IDS - 1, "{ids}");
    }

    /// R0002-0059: the target pane's Markdown never went through
    /// [`parser::parse`], so it used to reach Comrak with neither the NUL
    /// normalization nor the nesting ceiling `parse` applies — a document
    /// the library refuses as a source was rendered as a target. Both entry
    /// points now share [`parser::intake`], so they answer alike.
    #[test]
    fn target_markdown_meets_the_same_intake_guard_as_a_source_document() {
        let (doc, map) = doc_and_map("shallow\n");
        let deep = "> ".repeat(crate::parser::MAX_BLOCK_NESTING_DEPTH + 1) + "too deep\n";

        // The control: this is exactly what the source-side entry point says.
        assert!(matches!(
            parser::parse(&deep),
            Err(crate::error::ParseError::TooDeeplyNested { .. })
        ));

        let err = render_target(&doc, &deep, &map).expect_err("the target pane refuses it too");
        assert!(
            matches!(
                &err,
                RenderError::Intake(crate::error::ParseError::TooDeeplyNested { .. })
            ),
            "expected the intake guard to fire: {err}"
        );
    }

    /// Name the offending ids out of an [`RenderError::UnusableRange`], or
    /// fail with what came back instead.
    fn unusable(err: &RenderError, expected_pane: &str) -> String {
        let RenderError::UnusableRange { pane, ids } = err else {
            panic!("expected UnusableRange: {err}");
        };
        assert_eq!(*pane, expected_pane, "wrong pane named: {err}");
        ids.clone()
    }

    /// R0003-0060, the reversed case. `block_text` used to clamp, reorder
    /// and snap, so a row whose `target_range` ran backwards rendered as an
    /// EMPTY html block — under the row's own anchor, with its
    /// `fallback_status` unchanged, and nothing anywhere saying the bytes
    /// were not the block's.
    ///
    /// The victim is an html row on purpose: html rows are exactly the ones
    /// the browser demo's own range gate skips (they are not editable —
    /// R0003-0078), so the renderer is the only place that can refuse them.
    #[test]
    fn a_reversed_target_range_is_refused_instead_of_rendering_as_an_empty_block() {
        let (doc, map, translated) = doc_map_and_target("<div>keep me</div>\n\nbody\n");
        // Control: the honest map presents the html block's own bytes.
        let html = render_target(&doc, &translated, &map).expect("the honest map renders");
        assert!(html.contains("keep me"), "{html}");

        let mut corrupt = map.clone();
        let row = &mut corrupt.blocks[0];
        assert_eq!(row.block_kind, "html", "the fixture's first block is html");
        let real = row.target_range;
        assert!(real.end > real.start, "the fixture row has real bytes");
        row.target_range = ByteRange {
            start: real.end,
            end: real.start,
        };

        let err = render_target(&doc, &translated, &corrupt)
            .expect_err("a reversed target_range is refused");
        let ids = unusable(&err, "target");
        assert!(ids.starts_with("html-0001 ("), "{ids}");
        assert!(ids.contains("end precedes start"), "{ids}");
    }

    /// R0003-0060, the out-of-bounds case: an end past the pane's Markdown
    /// used to clamp to its length, i.e. render whatever tail of the
    /// document happened to follow the block.
    #[test]
    fn a_target_range_past_the_end_is_refused_instead_of_clamping_to_the_tail() {
        let (doc, map, translated) = doc_map_and_target("para one\n\npara two\n");
        let mut corrupt = map.clone();
        corrupt.blocks[0].target_range = ByteRange {
            start: 0,
            end: translated.len() + 40,
        };

        let err = render_target(&doc, &translated, &corrupt)
            .expect_err("an out-of-bounds target_range is refused");
        let ids = unusable(&err, "target");
        assert!(ids.contains("end is past the pane's Markdown"), "{ids}");
    }

    /// R0003-0060, the mid-character case: snapping backward to a boundary
    /// silently dropped the character the range split — a whole Korean
    /// syllable, here — from a block that otherwise rendered normally.
    #[test]
    fn a_mid_character_target_range_is_refused_instead_of_dropping_the_character() {
        let (doc, map, translated) = doc_map_and_target("한국어 문단\n");
        let real = map.blocks[0].target_range;
        assert!(
            !translated.is_char_boundary(real.end - 1),
            "the fixture's last character is multi-byte"
        );

        let mut corrupt = map.clone();
        corrupt.blocks[0].target_range = ByteRange {
            start: real.start,
            end: real.end - 1,
        };

        let err = render_target(&doc, &translated, &corrupt)
            .expect_err("a mid-character target_range is refused");
        let ids = unusable(&err, "target");
        assert!(
            ids.contains("offsets fall inside a UTF-8 character"),
            "{ids}"
        );
    }

    /// The source pane checks its own ranges against its own bytes.
    /// `Document` and `Block` are `pub` with `pub` fields, so a hand-built
    /// document can carry a range `parse` would never produce — the same
    /// programmatic-misuse door `regen` closed by snapping (R0003-0059).
    /// Here it is refused rather than snapped, because the renderer has an
    /// error channel and regen does not.
    #[test]
    fn the_source_pane_refuses_a_hand_built_range_that_splits_a_character() {
        let (mut doc, map) = doc_and_map("한국어 문단\n");
        let real = doc.blocks[0].source_range;
        doc.blocks[0].source_range = ByteRange {
            start: real.start + 1,
            end: real.end,
        };

        let err = render_source(&doc, &map).expect_err("a split character is refused");
        let ids = unusable(&err, "source");
        assert!(
            ids.contains("offsets fall inside a UTF-8 character"),
            "{ids}"
        );
    }

    /// The refusal is about ranges that are not slices, NOT about ranges
    /// that are empty: [`build_alignment_map`] emits `0..0` for a block its
    /// `BlockOffsets` did not cover (and warns — R0003-0055), and that map
    /// must still render.
    #[test]
    fn an_empty_in_bounds_target_range_still_renders() {
        let (doc, map) = doc_and_map("# Title\n\nbody\n");
        assert!(
            map.blocks
                .iter()
                .all(|b| b.target_range == ByteRange::default()),
            "the empty-BlockOffsets map is the fixture for this test"
        );
        let html = render_target(&doc, "# Title\n\nbody\n", &map)
            .expect("an empty range is a slice, so it renders");
        assert!(html.contains("data-sync-id=\"h1-0001\""), "{html}");
    }

    /// The bound applies to the range list too — one entry per offending
    /// row, so a wholly foreign map cannot turn the message into a dump.
    #[test]
    fn the_unusable_range_message_is_bounded() {
        let src = "a\n\nb\n\nc\n\nd\n\ne\n\nf\n\ng\n";
        let (doc, mut map, translated) = doc_map_and_target(src);
        for row in map.blocks.iter_mut() {
            row.target_range = ByteRange { start: 1, end: 0 };
        }
        let err = render_target(&doc, &translated, &map).expect_err("every row is unusable");
        let ids = unusable(&err, "target");
        assert!(ids.ends_with("(+2 more)"), "unbounded id list: {ids}");
    }
}

// OI-0010: grouped list rendering.
#[cfg(test)]
mod list_grouping_tests {
    use super::*;
    use crate::align::build_alignment_map;
    use crate::id;

    pub(super) fn render(src: &str) -> String {
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        render_source(&doc, &map).expect("the map is built from this doc, so it covers every block")
    }

    #[test]
    fn consecutive_items_share_one_ul() {
        let html = render("- alpha\n- beta\n");
        assert_eq!(html.matches("<ul>").count(), 1, "one shared <ul>:\n{html}");
        assert_eq!(html.matches("</ul>").count(), 1);
        assert_eq!(
            html.matches("<li data-sync-id=\"li-").count(),
            2,
            "two <li> anchors:\n{html}"
        );
        assert!(!html.contains("<div data-sync-id=\"li-"));
    }

    #[test]
    fn ordered_items_share_one_ol() {
        let html = render("1. one\n2. two\n3. three\n");
        assert_eq!(html.matches("<ol>").count(), 1, "one shared <ol>:\n{html}");
        assert_eq!(html.matches("<li data-sync-id=\"li-").count(), 3);
    }

    #[test]
    fn ordered_list_preserves_nonone_start() {
        // EXT-2026-07 P2-9 (resolves DCR-0007's known limitation): a list
        // opening at 3 must render <ol start="3"> so the browser does not
        // silently renumber it from 1.
        let html = render("3. a\n4. b\n");
        assert!(
            html.contains("<ol start=\"3\">"),
            "start offset preserved:\n{html}"
        );
        assert!(
            !html.contains("<ol>"),
            "a non-1 start must not also emit a bare <ol>:\n{html}"
        );
        assert_eq!(html.matches("<li data-sync-id=\"li-").count(), 2);
    }

    #[test]
    fn ordered_list_starting_at_one_omits_start_attr() {
        // DCR-0007: the group element stays attribute-free in the common
        // case — only a non-1 start earns an explicit attribute.
        let html = render("1. a\n2. b\n");
        assert!(html.contains("<ol>"), "bare <ol> for start=1:\n{html}");
        assert!(
            !html.contains("start="),
            "no start attr for start=1:\n{html}"
        );
    }

    /// The start ordinal must reach the TARGET pane too. Both panes run the
    /// same renderer, but the target pane parses the *regenerated* Markdown,
    /// so the ordinal survives only if the accepted payload keeps its own
    /// marker — which `validate::per_kind::check_list` enforces upstream (it
    /// rejects a provider that renumbers). This pins the syntax-layer half:
    /// markers kept ⇒ both panes agree on `<ol start="3">`.
    #[test]
    fn ordered_start_reaches_the_target_pane_too() {
        let mut doc = parser::parse("3. a\n4. b\n").expect("parses");
        id::assign_block_ids(&mut doc);
        // A faithful translation translates the text and leaves the marker
        // alone; slicing the source keeps this test agnostic about how the
        // item's byte range treats its trailing newline.
        let accepted: HashMap<BlockId, String> = doc
            .blocks
            .iter()
            .map(|b| {
                let src = &doc.source_text[b.source_range.start..b.source_range.end];
                (
                    b.block_id.clone(),
                    src.replace('a', "가").replace('b', "나"),
                )
            })
            .collect();
        let (translated_md, offsets) = crate::regen::regenerate(&doc, &accepted);
        assert!(
            translated_md.starts_with("3. 가"),
            "regen must keep the source marker, or this test pins nothing: \
             {translated_md:?}"
        );
        let statuses: HashMap<BlockId, FallbackStatus> = accepted
            .keys()
            .map(|id| (id.clone(), FallbackStatus::Translated))
            .collect();
        let map = build_alignment_map(
            &doc,
            &statuses,
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );

        let source_html = render_source(&doc, &map)
            .expect("the map is built from this doc, so it covers every block");
        let target_html = render_target(&doc, &translated_md, &map)
            .expect("the map is built from this doc, so it covers every block");
        for (pane, html) in [("source", &source_html), ("target", &target_html)] {
            assert!(
                html.contains("<ol start=\"3\">"),
                "{pane} pane must keep the start offset:\n{html}"
            );
            assert!(
                !html.contains("<ol>"),
                "{pane} pane must not also emit a bare <ol>:\n{html}"
            );
        }
    }

    #[test]
    fn marker_change_starts_new_list() {
        // `-` then `*` are two CommonMark lists; ordered follows.
        let html = render("- a\n\n* b\n\n1. c\n");
        assert_eq!(html.matches("<ul>").count(), 2, "two <ul> groups:\n{html}");
        assert_eq!(html.matches("<ol>").count(), 1);
    }

    #[test]
    fn list_between_paragraphs_closes_cleanly() {
        let html = render("intro\n\n- a\n- b\n\noutro\n");
        let ul = html.find("<ul>").expect("ul opens");
        let ul_close = html.find("</ul>").expect("ul closes");
        let outro = html.find("outro").expect("outro rendered");
        assert!(
            ul < ul_close && ul_close < outro,
            "group closes before outro:\n{html}"
        );
    }

    /// R0001-0026: a task list carries the GFM semantic classes —
    /// `contains-task-list` on the group, `task-list-item` on each checkbox
    /// row — so styling and integrations keyed on them keep recognizing the
    /// reconstructed group. The plain row sharing the list stays classless:
    /// one task row is enough to mark the GROUP, never its neighbors.
    #[test]
    fn task_rows_carry_the_gfm_task_list_classes() {
        let html = render("- [x] done\n- [ ] todo\n- plain\n");
        assert_eq!(
            html.matches("<ul class=\"contains-task-list\">").count(),
            1,
            "the group tag is marked once:\n{html}"
        );
        assert!(
            !html.contains("<ul>"),
            "a marked group must not also emit a bare <ul>:\n{html}"
        );
        assert_eq!(
            html.matches("<li class=\"task-list-item\" data-sync-id=\"li-")
                .count(),
            2,
            "both checkbox rows marked, class ahead of the untouched sync set:\n{html}"
        );
        assert_eq!(
            html.matches("<li data-sync-id=\"li-").count(),
            1,
            "the plain row keeps the bare anchor:\n{html}"
        );
        // Class and checkbox are one signal, so the counts must agree.
        assert_eq!(html.matches("type=\"checkbox\"").count(), 2, "{html}");
    }

    /// Both of the group tag's presentational attributes at once, in the
    /// order [`render_list_group`] fixes: class first, then `start`.
    #[test]
    fn ordered_task_list_carries_the_class_before_the_start_offset() {
        let html = render("3. [ ] a\n4. [x] b\n");
        assert!(
            html.contains("<ol class=\"contains-task-list\" start=\"3\">"),
            "class leads, start follows:\n{html}"
        );
        assert_eq!(html.matches("type=\"checkbox\"").count(), 2, "{html}");
    }

    /// The other half of the acceptance: a list with no task row is
    /// unchanged — no class anywhere, on the group or on the rows.
    #[test]
    fn a_list_without_task_rows_gains_no_class() {
        let html = render("- a\n- b\n\n1. c\n2. d\n");
        assert!(
            !html.contains("class="),
            "a plain list carries no class at all:\n{html}"
        );
    }

    /// The classes must reach the TARGET pane too, which parses the
    /// *regenerated* Markdown — so they survive only if the accepted payload
    /// keeps its own `[ ]` / `[x]` marker. Same shape as the start-offset
    /// test above: real `regen` → `render_target`, translated payloads.
    #[test]
    fn task_classes_reach_the_target_pane_too() {
        let mut doc = parser::parse("- [x] a\n- [ ] b\n").expect("parses");
        id::assign_block_ids(&mut doc);
        let accepted: HashMap<BlockId, String> = doc
            .blocks
            .iter()
            .map(|b| {
                let src = &doc.source_text[b.source_range.start..b.source_range.end];
                (
                    b.block_id.clone(),
                    src.replace('a', "가").replace('b', "나"),
                )
            })
            .collect();
        let (translated_md, offsets) = crate::regen::regenerate(&doc, &accepted);
        assert!(
            translated_md.starts_with("- [x] 가"),
            "regen must keep the task marker, or this test pins nothing: \
             {translated_md:?}"
        );
        let statuses: HashMap<BlockId, FallbackStatus> = accepted
            .keys()
            .map(|id| (id.clone(), FallbackStatus::Translated))
            .collect();
        let map = build_alignment_map(
            &doc,
            &statuses,
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );

        let source_html = render_source(&doc, &map)
            .expect("the map is built from this doc, so it covers every block");
        let target_html = render_target(&doc, &translated_md, &map)
            .expect("the map is built from this doc, so it covers every block");
        for (pane, html) in [("source", &source_html), ("target", &target_html)] {
            assert!(
                html.contains("<ul class=\"contains-task-list\">"),
                "{pane} pane must mark the group:\n{html}"
            );
            assert_eq!(
                html.matches("<li class=\"task-list-item\" data-sync-id=\"li-")
                    .count(),
                2,
                "{pane} pane must mark both rows:\n{html}"
            );
        }
    }

    #[test]
    fn item_inner_has_no_nested_list_wrapper() {
        let html = render("- alpha\n");
        let li_start = html.find("<li data-sync-id").expect("li present");
        let li_end = html.find("</li>").expect("li closes");
        let inner = &html[li_start..li_end];
        assert!(
            !inner[inner.find('>').unwrap()..].contains("<ul>"),
            "no nested <ul> inside the item anchor:\n{html}"
        );
    }
}

// Spec §5: preserved/translated html blocks live-render inside the div
// wrapper; the escaped placeholder is fallback-only.
#[cfg(test)]
mod html_render_tests {
    use super::*;
    use crate::align::build_alignment_map;
    use crate::{id, parser};

    /// Render the source pane for `src` with every html block's alignment row
    /// forced to `status` by a synthetic status-map entry.
    ///
    /// Since spec §3.2 a text-bearing html block is a real translation unit, so
    /// `build_alignment_map` would report the internal-bug `fallback_source`
    /// for one absent from the status map. These tests are about presentation
    /// *per status* (spec §5), so the status is stated outright: `Preserved` is
    /// exactly what a unit whose translation echoed the source reports. The
    /// accepted payload is irrelevant here — the source pane renders the
    /// document's own bytes.
    fn render_with_status(src: &str, status: FallbackStatus) -> String {
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        let statuses: HashMap<BlockId, FallbackStatus> = doc
            .blocks
            .iter()
            .filter(|b| matches!(b.kind, BlockKind::Html))
            .map(|b| (b.block_id.clone(), status))
            .collect();
        let map = build_alignment_map(
            &doc,
            &statuses,
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        render_source(&doc, &map).expect("the map is built from this doc, so it covers every block")
    }

    #[test]
    fn preserved_html_block_live_renders_unescaped_and_balanced() {
        let html = render_with_status(
            "<div class=\"note\">side note</div>\n\npara\n",
            FallbackStatus::Preserved,
        );
        assert!(
            html.contains("data-sync-id=\"html-0001\""),
            "html anchor present:\n{html}"
        );
        assert!(
            html.contains("<div class=\"note\">side note</div>"),
            "content is live, not escaped:\n{html}"
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "no placeholder:\n{html}"
        );
    }

    #[test]
    fn unclosed_html_fragment_is_balanced_in_the_pane() {
        let html = render_with_status(
            "<div align=\"center\">\n<b>Hero</b>\n\npara\n",
            FallbackStatus::Preserved,
        );
        let open = html.matches("<div").count();
        let close = html.matches("</div>").count();
        assert_eq!(open, close, "balanced output:\n{html}");
    }

    /// Spec §7 render bullet, second balancing arm: an orphan close-tag
    /// fragment renders as an anchored but empty container. The balancer drops
    /// the stray `</details>`, which is what keeps it from closing — and so
    /// swallowing — a sibling sync wrapper. A visually empty anchor is benign
    /// in the shipped engine (spec §5).
    #[test]
    fn orphan_close_html_fragment_renders_empty_in_the_pane() {
        let html = render_with_status("</details>\n\npara\n", FallbackStatus::Preserved);
        assert!(
            html.contains("data-sync-id=\"html-0001\""),
            "the anchor still exists, just empty:\n{html}"
        );
        assert!(
            !html.contains("</details>"),
            "the orphan close tag must be dropped, never emitted:\n{html}"
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "still the live-render path, not the placeholder:\n{html}"
        );
    }

    /// Spec §5 (the other arm): a fallback html row — a rejected unit, or the
    /// extraction failure `align` now maps to `fallback_source` — keeps the
    /// DCR-0013 escaped placeholder instead of live-rendering.
    #[test]
    fn fallback_html_block_renders_the_escaped_placeholder() {
        let html = render_with_status(
            "<div class=\"note\">side note</div>\n\npara\n",
            FallbackStatus::FallbackSource,
        );
        assert!(
            html.contains("data-skipped=\"html-block\""),
            "fallback html block must carry the placeholder marker:\n{html}",
        );
        assert!(
            html.contains("&lt;div class=&quot;note&quot;&gt;"),
            "placeholder content must be HTML-escaped:\n{html}",
        );
        assert!(
            !html.contains("<div class=\"note\">"),
            "no live source HTML may survive on the fallback path:\n{html}",
        );
        // DCR-0013 pins the placeholder ELEMENT, not just the marker
        // attribute: the escaped source must land inside a `<pre>` so
        // whitespace is preserved and the text stays visibly inert.
        assert!(
            html.contains("<pre"),
            "DCR-0013 placeholder element is a <pre>:\n{html}",
        );
    }

    /// OI-0035 route (c), render half (spec 2026-08-20 §8). A `data-sync-id`
    /// written into a source raw-HTML block used to reach the pane verbatim,
    /// where the engine could not tell it from an anchor the renderer emitted
    /// — so it could pre-claim a real block's id and become that block's
    /// scroll driver. Source Markdown is untrusted data (invariant 7) and this
    /// arm is the only live route by which source-controlled markup reaches a
    /// Markdown pane, because panes render with `unsafe_ = false`.
    ///
    /// The specimen puts the impostor BEFORE the block it impersonates, which
    /// is the shape the engine layer cannot defeat on its own: a listed id
    /// preceding the genuine anchor wins `collectAnchors`' first-occurrence
    /// policy. This is the layer that closes it.
    #[test]
    fn html_block_impostor_sync_attributes_never_reach_the_pane() {
        let html = render_with_status(
            "<div data-sync-id=\"p-0002\" DATA-Order=\"99\" class=\"note\">side note</div>\n\npara\n",
            FallbackStatus::Preserved,
        );
        assert_eq!(
            html.matches("data-sync-id=\"p-0002\"").count(),
            1,
            "only the real p-0002 paragraph may claim p-0002; the impostor in \
             the html block's own bytes must not survive into the pane:\n{html}"
        );
        assert!(
            !html.contains("DATA-Order"),
            "the strip is case-insensitive over the whole reserved namespace:\n{html}"
        );
        assert!(
            html.contains("<div class=\"note\">side note</div>"),
            "everything outside the namespace is content and survives \
             untouched, including the block's own text:\n{html}"
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "still the live-render arm, not the escaped placeholder:\n{html}"
        );
    }

    /// ti 490d97 wave 1, the composed pane chain at its only call site:
    /// `balance_fragment(&strip_reserved_sync_attrs(md))`.
    ///
    /// The strip removes the impostor attribute and leaves `<div/>` — a
    /// self-closing spelling of a non-void tag. HTML honours that flag in
    /// exactly two places, foreign content and the `<svg>`/`<math>` start
    /// tags; everywhere else it is a parse error and the element OPENS. The
    /// walk used to read it the way XML means it, so it never pushed the
    /// tag, classified the author's own `</div>` an orphan, and the balancer
    /// DELETED it. The fragment then reached the pane still open, the
    /// wrapper's own `</div>` closed it instead, and the wrapper stayed open
    /// — so the next block's anchor mounted INSIDE the html block's wrapper,
    /// which contracts.md §4a forbids (every anchor a direct child of
    /// `<main>`, list items excepted).
    ///
    /// The depth count below is the assertion that matters: between the
    /// wrapper's own attribute and the paragraph's, every `<div` opened must
    /// be closed. Before the fix it was 2 opens against 1 close.
    #[test]
    fn a_self_closing_html_block_does_not_swallow_the_next_anchor() {
        let html = render_with_status(
            "<div data-sync-id=\"p-0002\"/>x</div>\n\npara\n",
            FallbackStatus::Preserved,
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "the assertions below are only meaningful on the live-render \
             arm:\n{html}"
        );
        assert!(
            html.contains("<div/>x</div></div>"),
            "the strip leaves a flagged tag and the balancer must keep the \
             author's closer, so the wrapper gets to close itself:\n{html}"
        );
        let wrapper = html
            .find("<div data-sync-id=\"html-0001\"")
            .expect("the html block's wrapper anchor");
        let para = html
            .find("data-sync-id=\"p-0002\"")
            .expect("the paragraph's anchor");
        assert!(wrapper < para, "document order:\n{html}");
        let between = &html[wrapper..para];
        assert_eq!(
            between.matches("<div").count(),
            between.matches("</div>").count(),
            "the paragraph's anchor sits inside an unclosed div — the html \
             block consumed the wrapper's own </div> (contracts.md \
             §4a):\n{html}"
        );
        // And the strip still did its own job on the same bytes.
        assert_eq!(
            html.matches("data-sync-id=\"p-0002\"").count(),
            1,
            "only the real paragraph may claim p-0002:\n{html}"
        );
    }
}

// A6 / invariant 7: a `Skipped` top-level node still renders as an inert,
// escaped `<pre>` placeholder — anchored, never live. No node maps to
// `Skipped` under the current comrak options (raw HTML became its own kind
// in the 2026-08-03 spec), so the specimen is constructed synthetically —
// same pattern as the parser / unit / align skipped tests.
#[cfg(test)]
mod skipped_render_tests {
    use super::*;
    use crate::align::build_alignment_map;
    use crate::id::BlockId;
    use crate::parser::ranges::ByteRange;
    use crate::parser::{AstPath, Block};
    use crate::{id, parser};

    /// Render a document that is one synthetic `Skipped` block (whose source
    /// range covers `payload` inside `src`) followed by the parsed blocks of
    /// `src`. Returns the source-pane fragment.
    fn render_with_synthetic_skipped(src: &str, payload: &str) -> String {
        let mut doc = parser::parse(src).expect("parses");
        let start = src.find(payload).expect("payload present in source");
        doc.blocks.insert(
            0,
            Block {
                block_id: BlockId::new("x", 99),
                kind: BlockKind::Skipped {
                    label: "unsupported".to_string(),
                },
                spelling: crate::id::Spelling::Markdown,
                source_range: ByteRange {
                    start,
                    end: start + payload.len(),
                },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: AstPath(Vec::new()),
            },
        );
        id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        render_source(&doc, &map).expect("the map is built from this doc, so it covers every block")
    }

    #[test]
    fn skipped_block_renders_as_anchored_escaped_placeholder() {
        // The payload lives inside a code span so comrak never sees raw
        // inline HTML in the paragraph — the `<pre>` placeholder is then the
        // only thing this test's assertions can be reading.
        let html = render_with_synthetic_skipped("text with `<b>raw</b>` inside\n", "<b>raw</b>");

        let pre_start = html.find("<pre").expect("placeholder <pre> present");
        let pre_end = html[pre_start..]
            .find("</pre>")
            .map(|i| pre_start + i)
            .expect("placeholder <pre> closes");
        let pre = &html[pre_start..pre_end];

        // Full sync-attribute set + the marker attr naming the node kind.
        assert!(
            pre.contains("data-sync-id=\"x-0001\""),
            "skipped block must carry a data-sync-id anchor:\n{html}",
        );
        assert!(
            pre.contains("data-block-kind=\"skipped\""),
            "skipped block-kind attribute missing:\n{html}",
        );
        assert!(
            pre.contains("data-order=\"0\""),
            "skipped block must carry its source order:\n{html}",
        );
        assert!(
            pre.contains("data-fallback=\"preserved\""),
            "a never-batched skipped block is honestly `preserved`:\n{html}",
        );
        assert!(
            pre.contains("data-skipped=\"unsupported\""),
            "data-skipped marker missing:\n{html}",
        );

        // Content appears escaped, never live, never dropped (invariant 7).
        assert!(
            pre.contains("&lt;b&gt;raw&lt;/b&gt;"),
            "placeholder content must be HTML-escaped:\n{html}",
        );
        assert!(
            !pre.contains("<b>raw</b>"),
            "no live source HTML may survive unescaped:\n{html}",
        );
        assert!(
            !html.contains("raw HTML omitted"),
            "content must not be dropped by comrak's unsafe filter:\n{html}",
        );
    }
}

// Design D2 §B3 / C9: reference-style links resolve in BOTH panes. The
// mechanism changed in DCR-0017 — each pane is now parsed as a WHOLE
// document, so comrak resolves the definitions natively and the old
// per-fragment `ref_defs` append is gone (spec §4.1). `doc.ref_defs`
// survives for `validate::inline` only. The outcome pinned here is
// unchanged, which is the point of keeping the test.
#[cfg(test)]
mod refmap_render_tests {
    use super::*;
    use crate::align::build_alignment_map;
    use crate::id;

    #[test]
    fn reference_link_renders_as_anchor_in_both_panes() {
        let src = "See [docs][ref].\n\n[ref]: https://example.com/r\n";
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        assert!(
            doc.ref_defs.contains("[ref]: https://example.com/r"),
            "parser must have pooled the definition: {:?}",
            doc.ref_defs,
        );
        // Passthrough regen (no accepted payloads) → every block falls back
        // to its source bytes, so out.md is byte-identical and the target
        // pane's fragment carries the same reference use.
        let (translated_md, offsets) = crate::regen::regenerate(&doc, &HashMap::new());
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );

        let source_html = render_source(&doc, &map)
            .expect("the map is built from this doc, so it covers every block");
        assert!(
            source_html.contains("<a href=\"https://example.com/r\""),
            "source pane must resolve the reference link:\n{source_html}",
        );

        let target_html = render_target(&doc, &translated_md, &map)
            .expect("the map is built from this doc, so it covers every block");
        assert!(
            target_html.contains("<a href=\"https://example.com/r\""),
            "target pane must resolve the reference link:\n{target_html}",
        );

        // The definition itself renders nothing; the bracket form is gone.
        assert!(
            !source_html.contains("[docs][ref]"),
            "resolved reference must not leak as literal brackets:\n{source_html}",
        );
    }
}

// Spec §4.1/§4.4: AST-direct rendering — per-kind emission + assembly.
#[cfg(test)]
mod ast_direct_tests {
    use super::list_grouping_tests::render;
    use super::*;
    use crate::align::build_alignment_map;
    use crate::id;

    #[test]
    fn code_block_content_survives_whole_node_formatting() {
        let html = render("```rust\nfn main() {}\n```\n");
        assert!(
            html.contains("<pre><code class=\"language-rust\">"),
            "{html}"
        );
        assert!(
            html.contains("fn main() {}"),
            "code content present:\n{html}"
        );
    }

    /// Regression (found by the old-vs-new equivalence diff): an INDENTED
    /// code block used to render as a paragraph. `block_inner_html` began
    /// with `md.trim()`, which stripped the four leading spaces that made
    /// it code, so the fragment reparsed as prose. Whole-node formatting
    /// reads the `CodeBlock` node itself and cannot lose the indent.
    #[test]
    fn indented_code_block_renders_as_code_not_paragraph() {
        let html = render("    indented code\n");
        assert!(html.contains("<pre><code>indented code"), "{html}");
        assert!(
            !html.contains("<p>indented code"),
            "an indented code block is not a paragraph:\n{html}"
        );
    }

    #[test]
    fn table_keeps_its_element_and_tbody() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |\n");
        for needle in ["<table>", "<tbody>", "</tbody>", "</table>"] {
            assert!(html.contains(needle), "missing {needle}:\n{html}");
        }
    }

    #[test]
    fn blockquote_keeps_its_element() {
        let html = render("> quoted\n");
        assert!(html.contains("<blockquote>"), "{html}");
    }

    #[test]
    fn task_item_checkbox_survives() {
        let html = render("- [x] done\n- [ ] todo\n");
        assert_eq!(html.matches("type=\"checkbox\"").count(), 2, "{html}");
        assert!(html.contains("checked"), "{html}");
    }

    /// The checkbox-meets-`cr()` interaction: Comrak's own whole-document
    /// output for a LOOSE task list is
    /// `<li><input … /> \n<p>one</p>\n</li>` — the checkbox rides the
    /// item's open tag, then the non-tight paragraph's `cr()` fires
    /// because the input's trailing space left the writer mid-line. The
    /// renderer reproduces both halves byte-for-byte.
    #[test]
    fn loose_task_item_keeps_the_checkbox_then_newline_shape() {
        let html = render("- [x] one\n\n- [ ] two\n");
        assert!(
            html.contains("<input type=\"checkbox\" checked=\"\" disabled=\"\" /> \n<p>one</p>"),
            "{html}"
        );
    }

    #[test]
    fn loose_source_list_renders_loose_in_the_pane() {
        // v2 spec §4.4: source-driven looseness now faithful (old per-item
        // reparse under-reported it).
        let html = render("- a\n\n- b\n");
        assert!(html.contains("<p>a</p>"), "loose items gain <p>:\n{html}");
    }

    /// Spec §4.4 pins the looseness change in **both** panes: the change is
    /// source-driven, so a loose source list must render loose in the target
    /// pane too. The source-pane half is the test above; this is the target
    /// half, which needs the real `regen` → `render_target` path because
    /// looseness lives in the regenerated Markdown's blank lines, not in any
    /// alignment field. Passthrough regen (no accepted payloads) keeps every
    /// block on its source bytes, so the target pane's own parse sees the
    /// same blank-line separation the source pane did.
    #[test]
    fn loose_source_list_renders_loose_in_the_target_pane_too() {
        let mut doc = parser::parse("- a\n\n- b\n").expect("parses");
        id::assign_block_ids(&mut doc);
        let (translated_md, offsets) = crate::regen::regenerate(&doc, &HashMap::new());
        assert!(
            translated_md.contains("- a\n\n- b"),
            "passthrough regen must preserve the blank-line separation that \
             makes the list loose, or this test pins nothing: {translated_md:?}",
        );
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        let target_html = render_target(&doc, &translated_md, &map)
            .expect("the map is built from this doc, so it covers every block");
        assert!(
            target_html.contains("<p>a</p>"),
            "target pane must render the loose list loose too:\n{target_html}",
        );
    }

    #[test]
    fn tight_item_with_nested_list_gets_the_cr_newline() {
        // The cr()-emulation case: tight paragraph then nested list.
        let html = render("- top\n  - nested\n");
        assert!(
            html.contains("top\n<ul>"),
            "newline before nested list:\n{html}"
        );
    }

    #[test]
    fn wrapper_close_stays_on_the_block_line() {
        // Trailing-newline trim: scn_08's same-line pin depends on this.
        //
        // Adapted from the brief's literal `line.ends_with("</div>")`, which
        // no renderer can satisfy for a table: every Comrak Table/TableRow/
        // TableCell arm calls `cr()`, so `<table>`'s own HTML is multi-line
        // in both the old fragment-reparse output and the new whole-node
        // output. What the trim actually guarantees — and what scn_08 needs
        // — is (a) the sync wrapper closes IMMEDIATELY after the block's
        // last byte, with no stray newline in between, and (b) exactly one
        // `writeln!` per block, so the open tag and its whole attribute set
        // share one line.
        let html = render("| a |\n|---|\n| 1 |\n");
        assert!(
            html.contains("</table></div>\n"),
            "no stray newline between the block and its wrapper close:\n{html}"
        );
        let line = html
            .lines()
            .find(|l| l.contains("data-sync-id=\"t-"))
            .expect("table line");
        assert!(
            line.ends_with("<table>"),
            "the whole open tag is on one line:\n{line}"
        );
        // A single-line block (paragraph) does close on its own line — the
        // one-line-per-block shape in its pure form.
        let para = render("just text\n");
        let para_line = para
            .lines()
            .find(|l| l.contains("data-sync-id=\"p-"))
            .expect("paragraph line");
        assert!(
            para_line.ends_with("</p>"),
            "one line per block:\n{para_line}"
        );
    }

    #[test]
    fn reference_links_resolve_without_the_append() {
        let html = render("See [docs][ref].\n\n[ref]: https://example.com/r\n");
        assert!(html.contains("<a href=\"https://example.com/r\""), "{html}");
    }

    #[test]
    fn zip_mismatch_degrades_to_escaped_bytes_never_panics() {
        // Guard 2 (spec §4.3): a row whose kind the pane's Markdown does not
        // have at that position must degrade, not panic, and the anchor must
        // survive. The zip pairs the source block's kind with the pane AST's
        // node label (not the alignment row's `block_kind` string), so the
        // mismatch is forced the way the brief's adapt note prescribes:
        // render the target pane from MD that reparses to a different kind
        // sequence than the document's rows claim.
        let mut doc = parser::parse("plain paragraph\n").expect("parses");
        id::assign_block_ids(&mut doc);
        let pane_md = "# not a paragraph\n";
        let mut offsets = crate::regen::BlockOffsets::default();
        offsets.0.insert(
            doc.blocks[0].block_id.clone(),
            crate::parser::ranges::ByteRange {
                start: 0,
                end: pane_md.len(),
            },
        );
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &offsets,
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        let html = render_target(&doc, pane_md, &map)
            .expect("the map is built from this doc, so it covers every block");
        assert!(
            html.contains("data-sync-id=\"p-0001\""),
            "anchor survives:\n{html}"
        );
        assert!(
            html.contains("# not a paragraph"),
            "degrades to the row's own byte range:\n{html}"
        );
    }
}

// ti `18b9c3`: the alignment row and the DOM must agree about the anchor.
#[cfg(test)]
mod non_sync_anchor_tests {
    use super::*;
    use crate::align::{SyncRole, build_alignment_map};
    use crate::id::BlockId;
    use crate::parser::ranges::ByteRange;
    use crate::parser::{AstPath, Block};
    use crate::{id, parser};

    /// A `Title` block through the real renderer, with its real alignment
    /// row, asserting both halves in ONE test so they cannot pass separately
    /// while disagreeing.
    ///
    /// Nothing mints `BlockKind::Title` yet — the Markdown intake cannot, and
    /// wave 3's HTML intake is unrun — so this synthesizes one the way
    /// `skipped_render_tests` synthesizes a `Skipped` block. That is the
    /// point: the divergence ti `18b9c3` found is unreachable today and
    /// arrives with wave 3, so the test has to reach forward to it. It takes
    /// the Guard-2 degrade path (no Comrak node carries the label `title`),
    /// which is the arm a Markdown-pane render would give it and is enough to
    /// exercise the attribute decision.
    #[test]
    fn a_title_block_renders_without_the_dom_anchor_its_row_denies() {
        let src = "para\n";
        let mut doc = parser::parse(src).expect("parses");
        doc.blocks.insert(
            0,
            Block {
                block_id: BlockId::new("x", 99),
                kind: BlockKind::Title,
                spelling: crate::id::Spelling::Markdown,
                source_range: ByteRange { start: 0, end: 4 },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: AstPath(Vec::new()),
            },
        );
        id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );

        let title_id = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Title))
            .map(|b| b.block_id.clone())
            .expect("the synthetic title survived id assignment");
        let row = map
            .blocks
            .iter()
            .find(|r| r.source_block_id == title_id)
            .expect("build_alignment_map covers every block");

        // Half one: the row.
        assert_eq!(
            row.sync_role,
            SyncRole::NonSync,
            "D5: a <title> is translated content but not PAGE content"
        );

        // Half two: the pane.
        let html = render_source(&doc, &map)
            .expect("the map is built from this doc, so it covers every block");
        assert!(
            !html.contains(&format!("data-sync-id=\"{title_id}\"")),
            "a non-sync row must not get a DOM anchor:\n{html}"
        );
        assert!(
            html.contains("data-block-kind=\"title\""),
            "the styling hook stays — only the sync set is omitted:\n{html}"
        );

        // Non-vacuity: the ordinary block beside it still anchors, so this
        // is not passing because the render produced nothing.
        let para_id = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, BlockKind::Paragraph))
            .map(|b| b.block_id.clone())
            .expect("the parsed paragraph is still there");
        assert!(
            html.contains(&format!("data-sync-id=\"{para_id}\"")),
            "the paragraph beside it anchors:\n{html}"
        );
    }

    /// The thematic break is the kind that reached the old literal test, and
    /// its output must not have moved: it was already right, and this change
    /// only moved WHERE the decision is taken.
    #[test]
    fn the_thematic_break_arm_still_emits_exactly_its_styling_hook() {
        let mut doc = parser::parse("a\n\n---\n\nb\n").expect("parses");
        id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        let html = render_source(&doc, &map).expect("covers every block");
        assert!(
            html.contains("<hr data-block-kind=\"thematic-break\">"),
            "byte-identical to the hand-written form it replaced:\n{html}"
        );
        assert!(
            !html.contains("data-sync-id=\"h-"),
            "no anchor for the break itself:\n{html}"
        );
    }
}
