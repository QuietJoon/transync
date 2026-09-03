//! THE HTML document intake: `source_text` → block IR (spec 2026-08-20 §4).
//!
//! One linear pass over [`scan_tags`]' token stream, steered by
//! [`element_extents`]' precomputed pairing verdicts — the crate-external
//! half of `transync-html`'s single pairing discipline. This module decides
//! what a block *is* (classification, kinds, ids, sections, `ast_path`);
//! the mechanics crate has no opinion about that, and this module has no
//! opinion about pairing: it never re-pairs a tag, it reads the extents.
//!
//! **Classification — five classes, default STOP (spec §4):**
//! - PASS-THROUGH: markup becomes inter-block gap bytes; segmentation
//!   descends.
//! - STOP → semantic kind: the element becomes a leaf block; nothing is
//!   emitted inside one.
//! - VERBATIM: never a block; the bytes stay gap.
//! - PHRASING: absorbed into anonymous text runs; never a boundary.
//! - DEFAULT-STOP → [`BlockKind::Html`]: `summary`, `dt`, `dd`, `address`,
//!   `dialog`, `iframe`, `noscript`, `template`, `legend`, custom elements,
//!   future elements — the safe direction: over-stopping yields a coarser
//!   block, under-stopping would leak text into untranslated gap (D4).
//!
//! **Ranges are the whole element, tags included** (spec §4): void elements
//! are the tag span, anonymous runs are the trimmed run, forced closes run
//! to the boundary with trailing ASCII whitespace excluded. Whole-element
//! ranges are what `outcome::block_payload`, layer 3's
//! `HtmlSegmentConstraints.source_bytes`, `regen`'s fallback splice and
//! `source_hash` all want.
//!
//! **Identity by construction:** every emitted range starts at a `<`, ends
//! after a `>` or at an ASCII-whitespace trim, advances a monotone cursor,
//! and never nests — so `regen::regenerate(doc, &empty)` reproduces the
//! input byte-for-byte, which is wave 3's acceptance and this module's
//! `debug_assert_block_invariants` tripwire (named, not linked: the fn is
//! `pub(crate)` and this module doc is public).
//!
//! TRACE: ADR-0025

use crate::id::{BlockId, BlockKind, SourceFormat, Spelling};
use crate::parser::ranges::ByteRange;
use crate::parser::sections::SectionStack;
use crate::parser::{AstPath, Block, Document};
use std::collections::HashMap;
use transync_html::{ElementExtent, TagToken, element_extents, scan_tags};

/// PASS-THROUGH (spec §4): markup → gap bytes. `head` is handled by the
/// walker (it is the element that OPENS head mode) and — per D9 — `ul`,
/// `ol`, `menu`, `dl` pass through **to their items**.
const PASS_THROUGH: &[&str] = &[
    "html", "body", "div", "section", "article", "main", "nav", "aside", "header", "footer",
    "hgroup", "figure", "details", "form", "fieldset", "search", "center", "ul", "ol", "menu",
    "dl",
];

/// VERBATIM (spec §4): never a block; bytes stay gap. Extraction already
/// excludes script/style text by `TextType`; these have no reader-visible
/// anchor surface. ("Everything in head mode except `title`" is the mode
/// rule in the walker, not a name in this list.)
const VERBATIM: &[&str] = &["script", "style", "link", "meta", "base"];

/// PHRASING (spec §4, the recorded widening of D4): absorbed into text
/// runs, never a boundary. Without it `Hello <b>world</b> out there`
/// shatters into three blocks and the sentence crosses translation units.
const PHRASING: &[&str] = &[
    "a", "abbr", "audio", "b", "bdi", "bdo", "br", "button", "canvas", "cite", "code", "data",
    "del", "dfn", "em", "embed", "i", "img", "input", "ins", "kbd", "label", "mark", "math",
    "object", "optgroup", "option", "output", "picture", "q", "rp", "rt", "ruby", "s", "samp",
    "select", "small", "span", "strong", "sub", "sup", "svg", "textarea", "time", "u", "var",
    "video", "wbr",
];

fn is_pass_through(name: &str) -> bool {
    PASS_THROUGH.contains(&name)
}

fn is_verbatim(name: &str) -> bool {
    VERBATIM.contains(&name)
}

fn is_phrasing(name: &str) -> bool {
    PHRASING.contains(&name)
}

/// STOP → semantic kind (spec §4), for the kinds that need no context.
/// `li` (needs its parent), `hr` (void), `title` (needs head mode) and the
/// img-only run (rule T's exception) are the walker's; every name in no
/// class is DEFAULT-STOP → [`BlockKind::Html`].
fn semantic_stop_kind(name: &str) -> Option<BlockKind> {
    Some(match name {
        "h1" => BlockKind::Heading1,
        "h2" => BlockKind::Heading2,
        "h3" => BlockKind::Heading3,
        "h4" => BlockKind::Heading4,
        "h5" => BlockKind::Heading5,
        "h6" => BlockKind::Heading6,
        // `figcaption` is prose with a paragraph's shape (spec §4).
        "p" | "figcaption" => BlockKind::Paragraph,
        // Whole element, one leaf block — invariant 3's posture.
        "table" => BlockKind::Table,
        // `fenced` is vacuous under Html spelling: both consumers of
        // `fenced == false` sit behind `Spelling::Markdown` dispatch
        // (spec §4).
        "pre" => BlockKind::CodeBlock {
            info: None,
            fenced: true,
        },
        "blockquote" => BlockKind::Blockquote,
        _ => return None,
    })
}

/// Parse an HTML document into the block IR. Infallible: `scan_tags` and
/// `element_extents` are total, malformed input force-closes with
/// [`Document::warnings`] notes (spec §4), and NUL is normalized — the same
/// `normalize_source` the Markdown intake applies, so both intakes mean the
/// same string by "source bytes". The CLI's fallible boundary (exit-2
/// `InputReadFailure`, spec §9) is file IO, which is not this function.
///
/// Identity contract, pinned by `tests/html_intake_identity.rs`:
/// `regen::regenerate(&parse(s), &HashMap::new()).0 == parse(s).source_text`.
pub fn parse(source: &str) -> Document {
    let normalized = crate::parser::normalize_source(source);
    let mut walk = HtmlWalk::new(&normalized);
    walk.walk();
    let HtmlWalk {
        blocks, warnings, ..
    } = walk;
    let doc = Document {
        source_text: normalized.into_owned(),
        format: SourceFormat::Html,
        blocks,
        warnings,
        // No CommonMark context exists, so no link-reference definitions.
        ref_defs: String::new(),
    };
    debug_assert_block_invariants(&doc);
    doc
}

/// Regen's three requirements, checked where they are created (spec §4):
/// source order, non-overlap, in-bounds offsets on UTF-8 boundaries.
/// Unreachable through [`parse`] by construction — every range starts on an
/// ASCII `<`, ends after an ASCII `>` or an ASCII-whitespace trim, and
/// follows a monotone cursor — so this is a debug tripwire, not a
/// production failure mode. `Document` is pub-fields, so the firing
/// behaviour is pinned by calling this directly on a hand-built bad
/// document (the `warn_on_empty_list_item_range` precedent).
pub(crate) fn debug_assert_block_invariants(doc: &Document) {
    let mut prev_end = 0usize;
    for block in &doc.blocks {
        let ByteRange { start, end } = block.source_range;
        debug_assert!(
            start <= end,
            "{}: inverted range {start}..{end}",
            block.block_id
        );
        debug_assert!(
            start >= prev_end,
            "{}: starts at {start}, inside or before its predecessor (which ends at \
             {prev_end}) — blocks must be in source order and non-overlapping",
            block.block_id,
        );
        debug_assert!(
            end <= doc.source_text.len(),
            "{}: range end {end} is past the end of source_text ({})",
            block.block_id,
            doc.source_text.len(),
        );
        debug_assert!(
            doc.source_text.is_char_boundary(start) && doc.source_text.is_char_boundary(end),
            "{}: range {start}..{end} splits a UTF-8 char",
            block.block_id,
        );
        prev_end = end;
    }
}

/// One open pass-through container.
struct Container {
    name: String,
    /// Whole-element end: the close tag's end, or `content_end` when the
    /// extents walk closed it implicitly / left it open at EOF.
    end: usize,
    /// The container's own `ast_path` (ancestors' child ordinals + its own).
    path: Vec<usize>,
    /// Whether the source closed it with a real end tag.
    explicitly_closed: bool,
}

/// Rule T's per-run accumulators: spans excluded from the naked-byte test.
#[derive(Default)]
struct RunTags {
    /// `Skip` spans (comments, CDATA, doctype/bogus comments) in the run.
    skips: Vec<(usize, usize)>,
    /// Phrasing/absorbed tag spans in the run.
    tags: Vec<(usize, usize)>,
    /// `img` open-tag spans in the run — rule T's exception.
    imgs: Vec<(usize, usize)>,
}

/// Everything one walk of one document accumulates (the `WalkState` shape,
/// for the second intake).
struct HtmlWalk<'s> {
    source: &'s str,
    extents: Vec<ElementExtent>,
    /// Extent index by open-tag start offset — `scan_tags` and
    /// `element_extents` see the same open tags at the same offsets, so
    /// this lookup is total for every extent-backed `Open` token.
    extent_at: HashMap<usize, usize>,
    blocks: Vec<Block>,
    warnings: Vec<String>,
    /// The one global ordinal (spec §4): `BlockId::new(kind.id_code(), n)`.
    counter: u32,
    /// THE heading-scope stack, shared with the Markdown intake.
    sections: SectionStack,
    containers: Vec<Container>,
    /// One child-ordinal counter per open level; `[0]` is the root level.
    child_counts: Vec<usize>,
    /// Where the current anonymous run began (rule T).
    seg_start: usize,
    run: RunTags,
    /// Bytes before this offset are inside an already-consumed leaf.
    cursor: usize,
    /// Head mode is active for tokens starting before this offset.
    head_until: Option<usize>,
    /// Tokens starting before this offset are inside an open PHRASING
    /// element's extent and are absorbed whole (deviation 6): this is what
    /// keeps `<svg><title>` out of head-mode `Title` and keeps a phrasing
    /// element's descendants inside one run.
    phrasing_hold: usize,
}

/// A token's byte span, whatever kind it is.
///
/// `Skip` carries two more facts since ti `c1f9a8` — which region kind it is
/// and whether it was terminated — and this intake reads neither: an
/// unterminated trailing region is gap either way (rule T trims whole `Skip`
/// spans), and repairing one is `balance_fragment`'s job at a wrapper seam,
/// not an intake's. The variant list is still matched exhaustively, so a new
/// TOKEN would break this build, which is the part that must not be hidden.
fn token_span(tok: &TagToken) -> (usize, usize) {
    match tok {
        TagToken::Open { span, .. }
        | TagToken::Close { span, .. }
        | TagToken::Skip { span, .. } => *span,
    }
}

impl<'s> HtmlWalk<'s> {
    fn new(source: &'s str) -> Self {
        let extents = element_extents(source);
        let extent_at = extents
            .iter()
            .enumerate()
            .map(|(i, e)| (e.open.0, i))
            .collect();
        HtmlWalk {
            source,
            extents,
            extent_at,
            blocks: Vec::new(),
            warnings: Vec::new(),
            counter: 0,
            sections: SectionStack::default(),
            containers: Vec::new(),
            child_counts: vec![0],
            seg_start: 0,
            run: RunTags::default(),
            cursor: 0,
            head_until: None,
            phrasing_hold: 0,
        }
    }

    /// Whole-element end for extent `idx`.
    fn element_end(&self, idx: usize) -> usize {
        let e = &self.extents[idx];
        e.close.map_or(e.content_end, |c| c.1)
    }

    /// The one pass (spec §4's "one stack walk"): tokens in source order,
    /// extents consulted positionally.
    fn walk(&mut self) {
        for tok in scan_tags(self.source) {
            let (tstart, tend) = token_span(&tok);
            if tstart < self.cursor {
                continue; // inside a consumed leaf: nothing is emitted there
            }
            if let Some(limit) = self.head_until {
                let body_open = matches!(&tok, TagToken::Open { name, .. } if name == "body");
                if tstart < limit && !body_open {
                    self.head_token(&tok, tend);
                    continue;
                }
                // Past the head — or a <body> arriving inside a head whose
                // </head> is missing (plan deviation 3): the MODE ends here;
                // the still-open head container is the extents' business.
                self.head_until = None;
            }
            if tstart < self.phrasing_hold {
                self.absorb(&tok);
                continue;
            }
            match &tok {
                TagToken::Skip { span, .. } => self.run.skips.push(*span),
                TagToken::Close { name, span } => {
                    if is_phrasing(name) {
                        self.run.tags.push(*span);
                    } else {
                        // Enclosing (or orphan) close: a hard boundary.
                        self.flush_run(span.0);
                        self.pop_closed(span.1);
                        self.seg_start = span.1;
                    }
                }
                TagToken::Open {
                    name,
                    self_closing,
                    span,
                } => {
                    self.open(name, *self_closing, *span);
                }
            }
        }
        // Rule T never runs in head mode (spec §4: everything in head mode
        // except `title` is VERBATIM gap) — INCLUDING at EOF. On a page
        // truncated inside an unclosed <head> (`head_until` still reaching
        // `source.len()`), the tail is head-mode gap, not a paragraph.
        if !self
            .head_until
            .is_some_and(|limit| limit >= self.source.len())
        {
            self.flush_run(self.source.len());
        }
        for c in std::mem::take(&mut self.containers) {
            if !c.explicitly_closed && c.end >= self.source.len() {
                self.warnings.push(format!(
                    "html intake: <{}> was still open at end of input and was force-closed",
                    c.name,
                ));
            }
        }
    }

    /// Head mode (spec §4): only `title` stops inside `<head>`; everything
    /// else — meta, link, base, script, style, stray text, anything — is
    /// VERBATIM gap. Never rule T.
    fn head_token(&mut self, tok: &TagToken, tend: usize) {
        match tok {
            TagToken::Open { name, span, .. } => {
                if let Some(&idx) = self.extent_at.get(&span.0) {
                    if name == "title" {
                        // Emitted in source order, EMPTY section_path — a
                        // title opens no scope (spec §4).
                        self.emit_extent_leaf(BlockKind::Title, *span, Vec::new());
                    } else {
                        self.cursor = self.cursor.max(self.element_end(idx));
                    }
                }
                // Void head furniture (<meta>, <link>, <base>) has no
                // extent; its tag bytes are already gap.
            }
            TagToken::Close { name, span } => {
                if !is_phrasing(name) {
                    // `</head>` itself lands here and pops by the extents'
                    // verdict; the mode then ends at its recorded limit.
                    self.pop_closed(span.1);
                }
            }
            TagToken::Skip { .. } => {}
        }
        self.seg_start = self.seg_start.max(tend).max(self.cursor);
    }

    /// Absorb a token into the current run without letting its name steer
    /// classification — used inside an open phrasing element's extent.
    fn absorb(&mut self, tok: &TagToken) {
        match tok {
            TagToken::Skip { span, .. } => self.run.skips.push(*span),
            TagToken::Open { name, span, .. } => {
                self.run.tags.push(*span);
                if name == "img" {
                    self.run.imgs.push(*span);
                }
                if let Some(&idx) = self.extent_at.get(&span.0) {
                    let end = self.element_end(idx);
                    self.phrasing_hold = self.phrasing_hold.max(end);
                }
            }
            TagToken::Close { span, .. } => self.run.tags.push(*span),
        }
    }

    /// An `Open` token at segmentation level: classify and act.
    ///
    /// The `self_closing` flag is deliberately unread, and since ti `490d97`
    /// wave 1 that is a statement about delegation rather than about HTML: a
    /// start tag's `/` is honoured in exactly the two places HTML honours it
    /// — inside foreign content, and on the `<svg>`/`<math>` start tags that
    /// enter it — and everywhere else it is a parse error the parser ignores,
    /// so `<div/>` OPENS. `element_extents` already models all of that, so
    /// reading the flag here could only produce a second, worse opinion.
    fn open(&mut self, name: &str, _self_closing: bool, span: (usize, usize)) {
        if is_phrasing(name) {
            self.run.tags.push(span);
            // `img` is the name the scanner reports; a source `<image>` in
            // HTML content arrives here already renamed (ti `e923ef`, the
            // way HTML's tree builder does it), with its span still covering
            // the original bytes — so the exception covers both spellings
            // and identity is untouched.
            if name == "img" {
                self.run.imgs.push(span);
            }
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                self.phrasing_hold = self.phrasing_hold.max(end);
            }
            return;
        }

        // Every non-phrasing open is a hard boundary for rule T.
        self.flush_run(span.0);

        if name == "head" {
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                let closed = self.extents[idx].close.is_some();
                self.enter_container(name, end, closed);
                self.head_until = Some(end);
            }
            self.seg_start = span.1;
            return;
        }
        if is_pass_through(name) {
            // A self-closing `<div/>` is NOT closed and DOES mint an extent
            // (ti `490d97` wave 1), so this branch takes the container path
            // for it too and the author's own end tag is its real closer.
            // A pass-through name with no extent cannot arise in HTML
            // content — none of them is void — so the `if let` is the
            // foreign-content carve-out, where the bytes are pure gap.
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                let closed = self.extents[idx].close.is_some();
                self.enter_container(name, end, closed);
            }
            self.seg_start = span.1;
            return;
        }
        if is_verbatim(name) {
            // script/style hold raw text and have extents; link/meta/base
            // are void. Either way: no block, bytes stay gap.
            if let Some(&idx) = self.extent_at.get(&span.0) {
                self.cursor = self.cursor.max(self.element_end(idx));
            }
            self.seg_start = self.seg_start.max(span.1).max(self.cursor);
            return;
        }
        if name == "hr" {
            // Void STOP: the tag span is the block (spec §4).
            let sp = self.sections.current_path();
            self.emit(
                BlockKind::ThematicBreak,
                sp,
                ByteRange {
                    start: span.0,
                    end: span.1,
                },
            );
            self.seg_start = span.1;
            return;
        }

        // STOP → semantic kind, or DEFAULT-STOP → BlockKind::Html.
        let kind = self.stop_kind(name);
        if let Some(level) = kind.heading_level() {
            // Opening/closing SectionStack scopes exactly as Markdown
            // headings do — which is what keeps `partition_by_section`
            // working unchanged (spec §4).
            let sp = self.sections.close_through(level);
            let id = self.emit_extent_leaf(kind, span, sp);
            self.sections.open_scope(id, level);
        } else {
            let sp = self.sections.current_path();
            self.emit_extent_leaf(kind, span, sp);
        }
    }

    /// The kind a stopping element takes. `li` is `ListItem` only as the
    /// direct child of a list container (D9); `title` reaches here only
    /// OUTSIDE head mode, where it is nobody's semantic kind; everything
    /// unnamed is DEFAULT-STOP.
    fn stop_kind(&self, name: &str) -> BlockKind {
        if name == "li" {
            if let Some(c) = self.containers.last()
                && matches!(c.name.as_str(), "ul" | "ol" | "menu")
            {
                return BlockKind::ListItem {
                    ordered: c.name == "ol",
                    task: None,
                };
            }
            return BlockKind::Html;
        }
        semantic_stop_kind(name).unwrap_or(BlockKind::Html)
    }

    /// Emit a leaf whose range is the WHOLE element, tags included
    /// (spec §4): close-tag end when closed; boundary minus trailing ASCII
    /// whitespace when force-closed; the open-tag span alone when the
    /// element has no extent at all.
    ///
    /// That last case is narrow, and since ti `490d97` wave 1 it is NOT
    /// "a self-closing spelling": `<div/>` opens and gets an extent. What
    /// reaches it is a void name in HTML content (`<hr>` takes its own
    /// branch; the others are not STOP names) and a self-closing tag inside
    /// foreign content or an `<svg>`/`<math>` root — the two places HTML
    /// honours the flag.
    fn emit_extent_leaf(
        &mut self,
        kind: BlockKind,
        open_span: (usize, usize),
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let extent = self
            .extent_at
            .get(&open_span.0)
            .map(|&i| self.extents[i].clone());
        let (range, consumed_end, forced) = match &extent {
            Some(ex) => match ex.close {
                Some((_, close_end)) => (
                    ByteRange {
                        start: open_span.0,
                        end: close_end,
                    },
                    close_end,
                    false,
                ),
                None => {
                    let end = trim_ascii_ws_back(self.source, ex.content_end, open_span.1);
                    (
                        ByteRange {
                            start: open_span.0,
                            end,
                        },
                        ex.content_end,
                        true,
                    )
                }
            },
            None => (
                ByteRange {
                    start: open_span.0,
                    end: open_span.1,
                },
                open_span.1,
                false,
            ),
        };
        let id = self.emit(kind, section_path, range);
        if forced {
            let name = &extent.as_ref().expect("forced implies an extent").name;
            self.warnings.push(if consumed_end >= self.source.len() {
                format!(
                    "html intake: <{name}> ({id}) was still open at end of input and was \
                     force-closed"
                )
            } else {
                format!(
                    "html intake: <{name}> ({id}) has no end tag and was closed implicitly \
                     at byte {consumed_end}"
                )
            });
        }
        self.cursor = self.cursor.max(consumed_end);
        self.seg_start = self.cursor;
        id
    }

    /// Push ONE block: bump the one global ordinal, mint the kind-prefixed
    /// id, hash the exact range bytes, stamp `Spelling::Html { block_type:
    /// None }` (spec §3's format invariant) and the emission-order
    /// `ast_path`.
    fn emit(&mut self, kind: BlockKind, section_path: Vec<BlockId>, range: ByteRange) -> BlockId {
        self.counter += 1;
        let id = BlockId::new(kind.id_code(), self.counter);
        let hash = crate::id::source_hash_bytes(&self.source.as_bytes()[range.start..range.end]);
        let ast_path = self.next_child_path();
        self.blocks.push(Block {
            block_id: id.clone(),
            kind,
            spelling: Spelling::Html { block_type: None },
            source_range: range,
            source_hash: hash,
            section_path,
            ast_path: AstPath(ast_path),
        });
        id
    }

    /// The child-index convention that mirrors comrak's (spec §4): every
    /// emitted block and every entered container takes the next ordinal at
    /// its level, so an `<li>`'s path is its list container's path plus the
    /// item ordinal — `walk::normalize_top_level`'s prefix collapse stays
    /// total and adjacent lists can never merge.
    fn next_child_path(&mut self) -> Vec<usize> {
        let counter = self
            .child_counts
            .last_mut()
            .expect("the root level always exists");
        let ordinal = *counter;
        *counter += 1;
        let mut path = self
            .containers
            .last()
            .map(|c| c.path.clone())
            .unwrap_or_default();
        path.push(ordinal);
        path
    }

    fn enter_container(&mut self, name: &str, end: usize, explicitly_closed: bool) {
        let path = self.next_child_path();
        self.containers.push(Container {
            name: name.to_string(),
            end,
            path,
            explicitly_closed,
        });
        self.child_counts.push(0);
    }

    /// Pop every container the extents say has ended by `close_end` —
    /// positional, so an enclosing `</section>` pops the containers it
    /// implicitly closed without this module ever re-pairing a name.
    ///
    /// One retention rule guards the EOF warnings: a container that was
    /// never explicitly closed and whose extent runs to `source.len()`
    /// (wave 0's contract — unclosed at EOF means `close: None` and
    /// `content_end == html.len()`) is NOT popped here, even by a close
    /// tag whose own end reaches `source.len()`. Without it, a document
    /// whose last byte is a close tag (`…</body>`, `…</html>` — most real
    /// pages) satisfies `c.end <= close_end` as `len <= len` for every
    /// still-open ancestor, pops them all, and the EOF warning loop
    /// iterates an empty vec — the unclosed `<head>` pin would be
    /// guaranteed red and real pages would lose the warning silently. A
    /// container implicitly closed mid-document is untouched by the rule:
    /// its `content_end` sits before `source.len()`, so it still pops
    /// (and, per deviation 4, still pops silently).
    fn pop_closed(&mut self, close_end: usize) {
        while self.containers.last().is_some_and(|c| {
            c.end <= close_end && (c.explicitly_closed || c.end < self.source.len())
        }) {
            self.containers.pop();
            self.child_counts.pop();
        }
    }

    /// Rule T (spec §4, as amended by plan deviation 5): the bytes since
    /// the last hard boundary, evaluated as one anonymous run. At least
    /// one non-whitespace char outside tag and `Skip` spans (U+FEFF
    /// stripped) → ONE `Paragraph`; textless with one or more `img`
    /// elements in the run → ONE `Image` block — phrasing wrappers
    /// (`<a>`, `<picture>`, `<span>`) are absorbed markup, so a linked
    /// image keeps its sync anchor instead of dissolving into gap, the
    /// exception's own stated rationale; textless with no `img` at all →
    /// gap: nothing translatable exists, nothing is lost, bytes preserved
    /// verbatim.
    fn flush_run(&mut self, boundary: usize) {
        let start = self.seg_start;
        self.seg_start = boundary;
        let RunTags { skips, tags, imgs } = std::mem::take(&mut self.run);
        if start >= boundary {
            return;
        }
        let (start, end) = trim_run(self.source, start, boundary, &skips);
        if start >= end {
            return;
        }
        if any_naked_char(
            self.source,
            start,
            end,
            &[tags.as_slice(), skips.as_slice()],
        ) {
            let sp = self.sections.current_path();
            self.emit(BlockKind::Paragraph, sp, ByteRange { start, end });
        } else if !imgs.is_empty() {
            // Textless — every non-whitespace byte is inside an absorbed
            // tag span or a Skip span — and at least one img: ONE Image
            // block, wrappers included (deviation 5's amended reading;
            // §11's two-adjacent-img case verbatim). A textless run with
            // NO img falls through to gap.
            let sp = self.sections.current_path();
            self.emit(BlockKind::Image, sp, ByteRange { start, end });
        }
    }
}

/// U+FEFF as UTF-8 — stripped before rule T's byte test and trimmed from
/// run edges: a leading BOM must not mint a paragraph (spec §4).
const BOM: &[u8] = "\u{feff}".as_bytes();

/// Trim leading/trailing ASCII whitespace, U+FEFF, and whole `Skip` spans
/// from a run (spec §4's "range trimmed of leading/trailing whitespace +
/// `Skip` spans + BOM"). Every step crosses ASCII bytes, a whole BOM, or a
/// whole `Skip` span, so the result stays on UTF-8 boundaries.
fn trim_run(
    source: &str,
    mut start: usize,
    mut end: usize,
    skips: &[(usize, usize)],
) -> (usize, usize) {
    let bytes = source.as_bytes();
    loop {
        if start < end && bytes[start].is_ascii_whitespace() {
            start += 1;
        } else if start + BOM.len() <= end && &bytes[start..start + BOM.len()] == BOM {
            start += BOM.len();
        } else if let Some(&(_, e)) = skips.iter().find(|&&(s, e)| s == start && e <= end) {
            start = e;
        } else {
            break;
        }
    }
    loop {
        if end > start && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        } else if end >= start + BOM.len() && &bytes[end - BOM.len()..end] == BOM {
            end -= BOM.len();
        } else if let Some(&(s, _)) = skips.iter().find(|&&(s, e)| e == end && s >= start) {
            end = s;
        } else {
            break;
        }
    }
    (start, end)
}

/// Is there a non-whitespace, non-U+FEFF char in `[start, end)` covered by
/// NONE of the span lists? This is rule T's byte test, and the reason
/// `TagToken::Skip` exists: without it a doctype-only gap would mint a
/// phantom paragraph (spec §4, MEASURED).
fn any_naked_char(source: &str, start: usize, end: usize, cover: &[&[(usize, usize)]]) -> bool {
    source[start..end].char_indices().any(|(rel, c)| {
        let at = start + rel;
        let covered = cover
            .iter()
            .any(|spans| spans.iter().any(|&(s, e)| s <= at && at < e));
        !covered && !c.is_whitespace() && c != '\u{feff}'
    })
}

/// Walk `end` back over ASCII whitespace, never past `floor` (the open
/// tag's end): a forced-close range runs "up to the boundary token,
/// trailing whitespace excluded" (spec §4).
fn trim_ascii_ws_back(source: &str, mut end: usize, floor: usize) -> usize {
    let bytes = source.as_bytes();
    while end > floor && end <= bytes.len() && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}

// Rule T (spec 2026-08-20 §4): anonymous text runs. Each test names the
// sentence it pins.
#[cfg(test)]
mod rule_t_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src)
            .blocks
            .iter()
            .map(|b| b.kind.wire_str())
            .collect()
    }

    fn payloads_of(src: &str) -> Vec<String> {
        let doc = parse(src);
        doc.blocks
            .iter()
            .map(|b| doc.source_text[b.source_range.start..b.source_range.end].to_string())
            .collect()
    }

    /// "a maximal run … that contains at least one non-whitespace byte
    /// outside tag and Skip spans … becomes one Paragraph. A textless run
    /// is gap."
    #[test]
    fn whitespace_only_and_bom_only_runs_are_gap() {
        assert!(parse("<div>\n\t \n</div>\n").blocks.is_empty());
        assert!(parse("<div>\u{feff}</div>").blocks.is_empty());
        assert!(parse("\u{feff}\n").blocks.is_empty());
    }

    /// "U+FEFF stripped before the test — a leading BOM must not mint a
    /// paragraph", and the trim excludes it from the range.
    #[test]
    fn a_bom_is_stripped_before_the_test_and_trimmed_from_the_range() {
        let doc = parse("<div>\u{feff}Hello</div>");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(
            &doc.source_text[doc.blocks[0].source_range.start..doc.blocks[0].source_range.end],
            "Hello",
        );
    }

    /// MEASURED in the spec: pre-wave-0, `<!DOCTYPE html>` produced no
    /// token AND no skip — ordinary text to the scanner — so without
    /// `TagToken::Skip` this gap would mint a phantom paragraph. The Skip
    /// span's exclusion from the byte test is the token's reason to exist.
    #[test]
    fn a_doctype_only_gap_mints_no_block_because_skip_spans_are_excluded() {
        assert!(parse("<!DOCTYPE html>\n").blocks.is_empty());
        assert!(
            parse("<!-- a note -->\n<!-- another -->\n")
                .blocks
                .is_empty()
        );
        assert_eq!(kinds_of("<!DOCTYPE html>\n<p>x</p>\n"), vec!["paragraph"]);
    }

    /// "a textless run whose only non-whitespace content is ONE OR MORE
    /// img elements becomes ONE Image block" — §11 pins the two-image case
    /// because an earlier draft stated the rule two ways.
    #[test]
    fn one_or_more_imgs_in_a_textless_run_are_one_image_block() {
        assert_eq!(
            kinds_of("<div><img src=\"a.png\" alt=\"a\"></div>"),
            vec!["image"]
        );
        let src = "<div>\n<img src=\"a.png\"> <img src=\"b.png\">\n</div>";
        assert_eq!(
            kinds_of(src),
            vec!["image"],
            "two adjacent imgs: ONE block, not two, not gap"
        );
        assert_eq!(
            payloads_of(src),
            vec!["<img src=\"a.png\"> <img src=\"b.png\">"]
        );
    }

    /// An img beside real text is ordinary phrasing content of a paragraph
    /// — the counter-case that keeps deviation 5's amended exception from
    /// swallowing prose: one naked byte and the run is a Paragraph, linked
    /// or not.
    #[test]
    fn an_img_with_real_text_is_still_a_paragraph() {
        assert_eq!(
            kinds_of("<div><img src=\"a.png\"> the caption</div>"),
            vec!["paragraph"]
        );
        assert_eq!(
            kinds_of("<div><a href=\"/x\"><img src=\"a.png\"></a> the caption</div>"),
            vec!["paragraph"],
        );
    }

    /// Deviation 5's amended reading, justified by spec §4's own rationale
    /// ("so images keep a sync anchor instead of dissolving into gap") and
    /// PHRASING's definition ("never a boundary"): a linked image is ONE
    /// Image block, wrapper included; two adjacent linked images are still
    /// ONE block (the §11 two-image rule); phrasing noise beside an img no
    /// longer degrades the anchor; and a textless run with NO img at all
    /// remains gap.
    #[test]
    fn a_linked_image_keeps_its_anchor_as_one_image_block() {
        let src = "<div><a href=\"/home\"><img src=\"logo.png\" alt=\"logo\"></a></div>";
        assert_eq!(kinds_of(src), vec!["image"]);
        assert_eq!(
            payloads_of(src),
            vec!["<a href=\"/home\"><img src=\"logo.png\" alt=\"logo\"></a>"],
        );
        let two = "<div><a href=\"/a\"><img src=\"a.png\"></a> <a href=\"/b\"><img src=\"b.png\"></a></div>";
        assert_eq!(
            kinds_of(two),
            vec!["image"],
            "two adjacent linked images: ONE block"
        );
        assert_eq!(
            kinds_of("<div><b> </b><img src=\"a.png\"></div>"),
            vec!["image"],
            "phrasing noise beside an img keeps the anchor (the pre-amendment reading made this gap)",
        );
        assert!(
            parse("<div><b> </b><span></span></div>").blocks.is_empty(),
            "a textless run with no img at all is still gap",
        );
    }

    /// "without it `Hello <b>world</b> out there` at div level shatters
    /// into three blocks" — the PHRASING class, absorbed into ONE run.
    #[test]
    fn phrasing_markup_is_absorbed_into_one_paragraph() {
        let src = "<div>Hello <b>world</b> out there</div>";
        assert_eq!(kinds_of(src), vec!["paragraph"]);
        assert_eq!(payloads_of(src), vec!["Hello <b>world</b> out there"]);
    }

    /// "RCData counts as text (a bare <textarea> has translatable
    /// content)" — its content is never tokenized, so it is naked bytes.
    #[test]
    fn rcdata_counts_as_text() {
        assert_eq!(
            kinds_of("<div><textarea>a < b</textarea></div>"),
            vec!["paragraph"]
        );
    }

    /// Rule T tests BYTES, not decoded text: `&nbsp;` is six non-whitespace
    /// bytes, so the run mints a paragraph. Whether it holds translatable
    /// segments is extraction's question, in wave 5 — not intake's.
    #[test]
    fn an_entity_spelled_run_is_text_by_the_byte_test() {
        assert_eq!(kinds_of("<div>&nbsp;</div>"), vec!["paragraph"]);
    }

    /// "a custom element mid-sentence still stops (D4 verbatim), so
    /// `Price: <my-price/> today` splits the sentence — loud rather than
    /// silent" — accepted for now; §14's named review item.
    ///
    /// **TWO blocks, not the three this wave's plan predicted.** The plan
    /// was written 2026-08-20, when `walk_elements` read a start tag's `/`
    /// the way XML means it: `<my-price/>` minted no extent, so the
    /// DEFAULT-STOP block was the tag span alone and ` today` became a
    /// third block — a paragraph sitting OUTSIDE the element that a browser
    /// says contains it. Since ti `490d97` wave 1 the slash is honoured
    /// only where HTML honours it, so `<my-price/>` opens, `</div>` closes
    /// it implicitly, and its whole extent — trailing text included — is
    /// one DEFAULT-STOP block. The spec sentence still holds: the sentence
    /// is still severed at the custom element, which is what §14 carries.
    /// What changed is that the severed tail is no longer misattributed.
    #[test]
    fn a_custom_element_mid_sentence_still_stops_and_splits_the_sentence() {
        let src = "<div>Price: <my-price/> today</div>";
        assert_eq!(kinds_of(src), vec!["paragraph", "html"]);
        assert_eq!(payloads_of(src), vec!["Price:", "<my-price/> today"]);
        // The element has no end tag, so the force-close is reported rather
        // than silently absorbing the tail.
        let doc = parse(src);
        assert_eq!(doc.warnings.len(), 1, "{:?}", doc.warnings);
        assert!(
            doc.warnings[0].contains("closed implicitly"),
            "{}",
            doc.warnings[0]
        );
    }
}

// The five-class table (spec 2026-08-20 §4), pinned per named element.
#[cfg(test)]
mod classification_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src)
            .blocks
            .iter()
            .map(|b| b.kind.wire_str())
            .collect()
    }

    /// STOP → semantic kind: h1–h6, p, figcaption, table, pre, blockquote,
    /// hr.
    #[test]
    fn stop_elements_become_their_semantic_kinds() {
        assert_eq!(
            kinds_of("<h1>a</h1><h2>b</h2><h3>c</h3><h4>d</h4><h5>e</h5><h6>f</h6>"),
            vec![
                "heading-1",
                "heading-2",
                "heading-3",
                "heading-4",
                "heading-5",
                "heading-6"
            ],
        );
        assert_eq!(
            kinds_of("<p>a</p><figcaption>b</figcaption>"),
            vec!["paragraph", "paragraph"]
        );
        assert_eq!(
            kinds_of("<table><tr><td>x</td></tr></table>"),
            vec!["table"]
        );
        assert_eq!(
            kinds_of("<blockquote><p>q</p></blockquote>"),
            vec!["blockquote"]
        );
        assert_eq!(kinds_of("<hr>"), vec!["thematic-break"]);
        let doc = parse("<pre>let x = 1;</pre>");
        assert!(
            matches!(
                &doc.blocks[0].kind,
                BlockKind::CodeBlock {
                    info: None,
                    fenced: true
                }
            ),
            "spec §4: pre → CodeBlock {{ info: None, fenced: true }}, got {:?}",
            doc.blocks[0].kind,
        );
    }

    /// PASS-THROUGH, the full list: markup becomes gap, content stops
    /// inside. Each wrapper must contribute zero blocks of its own.
    #[test]
    fn every_pass_through_element_is_gap_around_its_content() {
        for wrapper in [
            "html", "body", "div", "section", "article", "main", "nav", "aside", "header",
            "footer", "hgroup", "figure", "details", "form", "fieldset", "search", "center",
        ] {
            let src = format!("<{wrapper}><p>inner</p></{wrapper}>");
            assert_eq!(kinds_of(&src), vec!["paragraph"], "for <{wrapper}>");
        }
    }

    /// D9: `ul`, `ol`, `menu`, `dl` pass through TO THEIR ITEMS. An
    /// `<ol>`'s items are ordered; `dt`/`dd` are DEFAULT-STOP; an `<li>`
    /// that is not a list child is nobody's semantic kind.
    #[test]
    fn list_containers_pass_through_to_their_items() {
        let doc = parse("<ul><li>a</li><li>b</li></ul>");
        assert!(
            doc.blocks.iter().all(|b| matches!(
                b.kind,
                BlockKind::ListItem {
                    ordered: false,
                    task: None
                }
            )),
            "{:?}",
            doc.blocks.iter().map(|b| &b.kind).collect::<Vec<_>>(),
        );
        let doc = parse("<ol><li>a</li></ol>");
        assert!(matches!(
            doc.blocks[0].kind,
            BlockKind::ListItem {
                ordered: true,
                task: None
            }
        ));
        let doc = parse("<menu><li>a</li></menu>");
        assert!(matches!(
            doc.blocks[0].kind,
            BlockKind::ListItem {
                ordered: false,
                task: None
            }
        ));
        assert_eq!(
            kinds_of("<dl><dt>t</dt><dd>d</dd></dl>"),
            vec!["html", "html"]
        );
        assert_eq!(kinds_of("<div><li>stray</li></div>"), vec!["html"]);
    }

    /// DEFAULT-STOP → BlockKind::Html: summary, dt, dd, address, dialog,
    /// iframe, noscript, template, legend, custom elements. `<template>`
    /// needs no special case — it stops (zero segments is wave 5's story).
    #[test]
    fn the_default_stop_set_stops_as_block_kind_html() {
        for src in [
            "<summary>s</summary>",
            "<address>a</address>",
            "<dialog>d</dialog>",
            "<iframe src=\"x\"></iframe>",
            "<noscript><p>n</p></noscript>",
            "<template><p>t</p></template>",
            "<legend>l</legend>",
            "<x-widget>custom</x-widget>",
        ] {
            assert_eq!(kinds_of(src), vec!["html"], "for {src}");
        }
    }

    /// VERBATIM: script, style, link, meta, base — never a block, bytes
    /// stay gap, and "script content cannot reach a run because script is
    /// VERBATIM and a hard boundary".
    #[test]
    fn verbatim_elements_are_never_blocks() {
        assert_eq!(
            kinds_of("<script>if (a < b) { emit(\"</div>\"); }</script>\n<p>x</p>"),
            vec!["paragraph"],
        );
        assert_eq!(
            kinds_of("<style>main > p { color: red }</style>\n<p>x</p>"),
            vec!["paragraph"]
        );
        assert_eq!(
            kinds_of("<link rel=\"a\" href=\"b\">\n<meta name=\"c\">\n<base href=\"/\">\n<p>x</p>"),
            vec!["paragraph"],
        );
    }
}

// Head mode and the D5 title (spec §4's <title> paragraph).
#[cfg(test)]
mod head_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src)
            .blocks
            .iter()
            .map(|b| b.kind.wire_str())
            .collect()
    }

    /// "title in head mode → Title", with an EMPTY section_path — a page
    /// title opens no scope, and no glossary section-selector can mean it.
    #[test]
    fn the_title_stops_only_in_head_mode_and_opens_no_scope() {
        let doc = parse("<head><title>Page &amp; title</title></head><body><p>x</p></body>");
        assert_eq!(doc.blocks[0].kind.wire_str(), "title");
        assert!(doc.blocks[0].section_path.is_empty());
        assert_eq!(doc.blocks[1].kind.wire_str(), "paragraph");
        // Outside head mode the same element is DEFAULT-STOP:
        assert_eq!(kinds_of("<title>loose</title>"), vec!["html"]);
        // <svg><title> is phrasing-held run content — never Title:
        assert_eq!(
            kinds_of("<div><svg><title>chart</title></svg> labelled</div>"),
            vec!["paragraph"],
        );
    }

    /// "everything in head mode except title" is VERBATIM gap, and "a
    /// duplicate title yields two honest rows".
    #[test]
    fn everything_else_in_head_is_gap_and_a_duplicate_title_is_two_honest_blocks() {
        let doc = parse(
            "<head><meta charset=\"utf-8\"><style>p{}</style><title>a</title><title>b</title></head>",
        );
        assert_eq!(
            doc.blocks
                .iter()
                .map(|b| b.kind.wire_str())
                .collect::<Vec<_>>(),
            vec!["title", "title"],
        );
    }

    /// Plan deviation 3: the pairing table has no head-closes-at-body rule,
    /// so on a page that omits </head> the MODE ends at the first <body>
    /// open — the body must not dissolve into head-mode gap.
    #[test]
    fn a_missing_head_close_does_not_swallow_the_body() {
        let doc = parse("<head><title>t</title><body><p>real content</p></body>");
        assert_eq!(
            doc.blocks
                .iter()
                .map(|b| b.kind.wire_str())
                .collect::<Vec<_>>(),
            vec!["title", "paragraph"],
        );
        assert!(
            doc.warnings.iter().any(|w| w.contains("<head>")),
            "the truly-unclosed head is still reported at EOF: {:?}",
            doc.warnings,
        );
    }

    /// Rule T must not leak into head mode at EOF: a page truncated inside
    /// an unclosed <head> — exactly the shape a cut-off download produces —
    /// ends with stray head text, and that tail is head-mode VERBATIM gap
    /// (spec §4: everything in head mode except title), never a Paragraph.
    /// Byte-identity survives either way; the BLOCK SET is what this pins.
    #[test]
    fn trailing_text_inside_an_unclosed_head_is_gap_not_a_paragraph() {
        let doc = parse("<head><title>t</title>junk");
        assert_eq!(
            doc.blocks
                .iter()
                .map(|b| b.kind.wire_str())
                .collect::<Vec<_>>(),
            vec!["title"],
            "the trailing head text must not mint a paragraph: {:?}",
            doc.warnings,
        );
        let (out, _) = crate::regen::regenerate(&doc, &std::collections::HashMap::new());
        assert_eq!(out, doc.source_text, "the tail stays verbatim gap");
        assert!(
            doc.warnings.iter().any(|w| w.contains("<head>")),
            "the unclosed head still warns at EOF: {:?}",
            doc.warnings,
        );
    }
}

// "Implicit closes, force-closes on unclosed leaves, and EOF force-close
// each append a Document::warnings note — that channel is documented as
// not parse-only." (spec §4)
#[cfg(test)]
mod warning_tests {
    use super::*;

    /// An implicit close (the second <p> pops the first) and a force-close
    /// by the enclosing </div> each leave a note naming the block.
    #[test]
    fn an_implicit_close_and_a_force_close_each_leave_a_note() {
        let doc = parse("<div><p>alpha<p>bravo</div>");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.warnings.len(), 2, "{:?}", doc.warnings);
        assert!(
            doc.warnings[0].contains("p-0001") && doc.warnings[0].contains("closed implicitly"),
            "{}",
            doc.warnings[0],
        );
        assert!(doc.warnings[1].contains("p-0002"), "{}", doc.warnings[1]);
        // The force-closed ranges exclude trailing whitespace and the
        // boundary tokens (spec §4 "ranges"):
        let p1 = &doc.source_text[doc.blocks[0].source_range.start..doc.blocks[0].source_range.end];
        assert_eq!(p1, "<p>alpha");
    }

    /// EOF force-close: the leaf and the still-open container are each
    /// reported, and nothing panics.
    #[test]
    fn eof_force_close_warns_for_the_leaf_and_the_container() {
        let doc = parse("<div><p>tail with no closers");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].kind.wire_str(), "paragraph");
        assert!(
            doc.warnings
                .iter()
                .any(|w| w.contains("p-0001") && w.contains("end of input")),
            "{:?}",
            doc.warnings,
        );
        assert!(
            doc.warnings
                .iter()
                .any(|w| w.contains("<div>") && w.contains("end of input")),
            "{:?}",
            doc.warnings,
        );
    }

    /// Spec §12 wave 3 acceptance: "an unclosed fragment force-closing with
    /// a warning rather than a panic" — generalized to genuinely hostile
    /// shapes. Identity must survive every one of them, because identity is
    /// a range-bookkeeping property, not a well-formedness property.
    #[test]
    fn genuinely_broken_input_parses_without_panicking_and_round_trips() {
        for src in [
            "</p></div><p>orphans first",
            "<p><div></span><p>text",
            "<ul><li><table><tr>x",
            "<<<>>><p>&</p>",
            "<title>rcdata swallows <div> everything",
            "<head><head><title>t</title>",
        ] {
            let doc = parse(src);
            let (out, _) = crate::regen::regenerate(&doc, &std::collections::HashMap::new());
            assert_eq!(out, doc.source_text, "for {src:?}");
        }
    }
}

// The spec's intake tripwire ("A debug assertion pins order/non-overlap at
// intake"), pinned by tripping it: Document is pub-fields, so the bad
// documents below are constructible outside parse, and the assertion must
// name the block and the violation.
#[cfg(test)]
mod invariant_tests {
    use super::*;

    fn block(ordinal: u32, start: usize, end: usize) -> Block {
        Block {
            block_id: BlockId::new("p", ordinal),
            kind: BlockKind::Paragraph,
            spelling: Spelling::Html { block_type: None },
            source_range: ByteRange { start, end },
            source_hash: 0,
            section_path: Vec::new(),
            ast_path: AstPath(vec![0]),
        }
    }

    fn doc_with(source: &str, blocks: Vec<Block>) -> Document {
        Document {
            source_text: source.to_string(),
            format: SourceFormat::Html,
            blocks,
            warnings: Vec::new(),
            ref_defs: String::new(),
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "inside or before its predecessor")]
    fn the_assertion_trips_on_overlapping_ranges() {
        debug_assert_block_invariants(&doc_with("abcdefgh", vec![block(1, 0, 5), block(2, 3, 8)]));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "splits a UTF-8 char")]
    fn the_assertion_trips_on_a_non_boundary_offset() {
        // Byte 2 is inside the three-byte '한'.
        debug_assert_block_invariants(&doc_with("a한글b", vec![block(1, 0, 2)]));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "past the end of source_text")]
    fn the_assertion_trips_on_an_out_of_bounds_range() {
        debug_assert_block_invariants(&doc_with("short", vec![block(1, 0, 99)]));
    }

    /// The assertion is quiet on what parse actually builds — otherwise
    /// every test above this line would already have tripped it, but say
    /// so explicitly once.
    #[test]
    fn the_assertion_is_quiet_on_parse_output() {
        let doc = parse("<h1>a</h1><p>b</p><ul><li>c</li></ul>");
        debug_assert_block_invariants(&doc);
        assert_eq!(doc.blocks.len(), 3);
    }
}

// Ids, sections and paths (spec §4 "Ids, hashes, paths").
#[cfg(test)]
mod id_and_path_tests {
    use super::*;

    /// "opening/closing SectionStack scopes exactly as Markdown headings
    /// do" — same close-through-then-open rule, same section_path shapes.
    #[test]
    fn headings_open_and_close_section_scopes_exactly_as_markdown_headings_do() {
        let doc = parse("<h1>A</h1><p>one</p><h2>B</h2><p>two</p><h1>C</h1><p>three</p>");
        let paths: Vec<Vec<&str>> = doc
            .blocks
            .iter()
            .map(|b| b.section_path.iter().map(|id| id.0.as_str()).collect())
            .collect();
        assert_eq!(
            paths,
            vec![
                vec![],
                vec!["h1-0001"],
                vec!["h1-0001"],
                vec!["h1-0001", "h2-0003"],
                vec![],
                vec!["h1-0005"],
            ],
        );
    }

    /// "an <li> takes its list container's path + item ordinal, exactly the
    /// Markdown List-arm convention" — the prefix is shared within one list
    /// and differs across adjacent lists.
    #[test]
    fn list_items_carry_the_container_path_plus_their_ordinal() {
        let doc = parse("<ul><li>a</li><li>b</li></ul><ul><li>c</li></ul>");
        let paths: Vec<&[usize]> = doc.blocks.iter().map(|b| &b.ast_path.0[..]).collect();
        assert_eq!(paths, vec![&[0, 0][..], &[0, 1][..], &[1, 0][..]]);
    }

    /// "the same global-ordinal scheme … the pipeline's existing
    /// assign_block_ids is idempotent over it".
    #[test]
    fn assign_block_ids_is_idempotent_over_the_html_intake() {
        let mut doc = parse("<h1>t</h1><p>a</p><ul><li>b</li></ul><hr>");
        let before: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert_eq!(before, vec!["h1-0001", "p-0002", "li-0003", "hr-0004"]);
        crate::id::assign_block_ids(&mut doc);
        let after: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert_eq!(before, after);
    }

    /// The hash covers the WHOLE element, attributes included, "so
    /// <p class=\"a\">x</p> and <p>x</p> cannot alias in the cache"
    /// (spec §4 "ranges").
    #[test]
    fn the_source_hash_covers_the_whole_element_attributes_included() {
        let a = parse("<p class=\"a\">x</p>");
        let b = parse("<p>x</p>");
        assert_ne!(a.blocks[0].source_hash, b.blocks[0].source_hash);
        assert_eq!(
            a.blocks[0].source_hash,
            crate::id::source_hash_bytes("<p class=\"a\">x</p>".as_bytes()),
        );
    }
}
