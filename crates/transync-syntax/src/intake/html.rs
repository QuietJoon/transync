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
