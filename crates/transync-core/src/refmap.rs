//! The document's link-reference definitions, as comrak itself resolves them
//! (R0013-0003, DCR-0063).
//!
//! A CommonMark definition (`[label]: url "title"`) is document-level state
//! wherever it sits: in an inter-block gap, inside a blockquote or a list item
//! (CommonMark 0.31.2 example 218), or on a paragraph's own first lines ahead
//! of its text. comrak consumes every one of them into one parser-wide map
//! during block finalization and leaves no node behind, so a block's payload
//! parsed **alone** resolves nothing it does not define itself: `[text][ref]`
//! stays literal bracket text and contributes no destination to inline
//! protection. That is how a dropped reference used to pass: both sides of the
//! comparison were equally empty.
//!
//! [`ReferenceMap`] carries the whole document's answer into those fragment
//! parses through comrak's own `broken_link_callback`. A label the fragment
//! defines resolves locally, exactly as comrak would; any other label is looked
//! up in the map. No definition text is appended to anything, so a fragment
//! parse costs its own bytes and nothing per definition the document holds.
//!
//! **comrak answers every label question.** This module matches no labels; it
//! only chooses which spellings to ask about:
//!
//! 1. *Candidates* are every `[…]:` the text spells, read with the same
//!    bracket and backslash rules comrak's own label scanner applies. It is a
//!    deliberate superset — prose like `a[i]: x`, text inside a code block —
//!    because a candidate is only a question. A label continued across lines
//!    is also asked exactly as comrak's block parser holds it in the
//!    paragraph's text, container prefixes and indentation already stripped:
//!    comrak reads it back out of that text itself, once as a reference label
//!    and once as the literal of a code span
//!    ([`labels_as_comrak_reads_them`]).
//! 2. comrak *keys* each candidate (case fold, whitespace collapse) when it
//!    parses a probe `[][candidate]` with a callback that answers every lookup
//!    with the key it was handed, so each probe's link carries comrak's key
//!    for that spelling.
//! 3. comrak *resolves* each key against the document: the probe of the
//!    first spelling that produced the key is placed **ahead** of the
//!    document, where no unclosed fence or HTML block can swallow it, and its
//!    destination is read off the link it became. A candidate that is not a
//!    definition resolves to nothing and drops out.
//!
//! A key is never itself asked as a label: comrak's key is not always a label
//! comrak reads back to the same key (an edge of Unicode whitespace, an
//! escaping `\` left last, a case fold that outgrows the label scanner — see
//! `Label`), so the probe carries the spelling the source wrote. Nothing here
//! caps a spelling's length either: comrak's 1,000-byte label limit counts a
//! continued label after its container prefixes are stripped, so only
//! comrak's scanner, reading the probe, refuses a long one.
//!
//! A probe is one line (line breaks in a candidate become spaces, which
//! comrak's normalization treats exactly as it treats the break), so every
//! probe is exactly one paragraph and the probes are read back by position.
//!
//! No inline construct can hide a continued label from that reading: the code
//! span reading lays each paragraph holding one into a chain of code spans
//! from its first byte to its last such label, so no code span, inline HTML
//! or link title of the source's own can open there and run past a label's
//! `[`.
//!
//! [`definition_change`] asks the same three questions of a payload pair: which
//! definitions does each payload carry **itself**, and did the translation keep
//! every destination. A definition is text the provider can rewrite, delete or
//! inject without adding or removing a single inline node, so the destination
//! inventory alone cannot see it.
//!
//! The map is computed once per run inside `transync-core`; it is not a field
//! of `transync_syntax`'s `Document`, so the wasm `render_pair` path never pays
//! for a validation-only artifact. An HTML document has no reference
//! definitions and gets the empty map.
//!
//! TRACE: ADR-0012
//! TRACE: R0013-0003

use crate::markdown::Document;
use comrak::nodes::{AstNode, NodeValue};
use comrak::{BrokenLinkReference, ResolvedReference};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// comrak's floor on reference expansion: a parse stops resolving references
/// once the destinations and titles it has expanded pass
/// `max(input bytes, COMRAK_EXPANSION_FLOOR)` (comrak 0.27
/// `Parser::finalize_document`), which is what keeps a short document from
/// expanding one long URL a million times. A resolution this module adds
/// through the callback is charged against a budget of the same shape, so the
/// guard comrak gives a whole document is not lost by moving a lookup out of
/// its map.
pub(crate) const COMRAK_EXPANSION_FLOOR: usize = 100_000;

/// The expansion budget a fragment comparison charges each side: one value for
/// both, so a reference sequence the two sides share resolves to the same
/// point on both. Shaped like comrak's own (see [`COMRAK_EXPANSION_FLOOR`]).
pub(crate) fn expansion_budget(texts: &[&str]) -> usize {
    texts
        .iter()
        .map(|t| t.len())
        .fold(COMRAK_EXPANSION_FLOOR, usize::max)
}

/// The document's effective link-reference definitions: normalized label →
/// destination, first definition winning, exactly as comrak resolved them over
/// the whole document. Titles are not kept — no consumer compares one.
///
/// TRACE: R0013-0003
#[derive(Debug, Default, Clone)]
pub(crate) struct ReferenceMap {
    urls: HashMap<String, String>,
}

impl ReferenceMap {
    /// The map with no definitions: every fragment parse resolves only what
    /// the fragment defines itself.
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// The map for `doc`. A Markdown document's comes from its own source; an
    /// HTML document has no reference definitions.
    pub(crate) fn of(doc: &Document) -> Self {
        match doc.format {
            crate::id::SourceFormat::Markdown => Self::of_markdown(&doc.source_text),
            crate::id::SourceFormat::Html => Self::empty(),
        }
    }

    /// The map for a Markdown source: candidates, normalized by comrak,
    /// resolved by comrak against the whole source (module docs).
    pub(crate) fn of_markdown(source: &str) -> Self {
        let candidates = label_candidates(source);
        if candidates.is_empty() {
            return Self::empty();
        }
        let labels = keyed_labels(&candidates);
        let urls = probed_destinations(&labels, source);
        Self {
            urls: labels
                .into_iter()
                .zip(urls)
                .filter_map(|(label, url)| url.map(|u| (label.key, u)))
                .collect(),
        }
    }

    /// The destination `normalized` resolves to in the document, if any.
    #[cfg(test)]
    pub(crate) fn get(&self, normalized: &str) -> Option<&str> {
        self.urls.get(normalized).map(String::as_str)
    }

    /// Whether the document defines nothing.
    pub(crate) fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }

    /// Parse a fragment under the canonical options, through the depth guard,
    /// resolving a reference the fragment does not define from this map.
    ///
    /// `budget` caps the destination bytes the callback hands out
    /// ([`expansion_budget`]); a lookup past it resolves to nothing, as a
    /// lookup past comrak's own budget does.
    pub(crate) fn parse<'a>(
        &self,
        arena: &'a comrak::Arena<AstNode<'a>>,
        text: &str,
        budget: usize,
    ) -> Result<&'a AstNode<'a>, crate::error::ParseError> {
        if self.is_empty() {
            return crate::markdown::guarded_parse(arena, text, &crate::markdown::comrak_options());
        }
        let mut remaining = budget;
        let mut resolve = |r: BrokenLinkReference| {
            let url = self.urls.get(r.normalized)?;
            remaining = remaining.checked_sub(url.len())?;
            Some(ResolvedReference {
                url: url.clone(),
                title: String::new(),
            })
        };
        crate::markdown::guarded_parse(arena, text, &options_with(&mut resolve))
    }
}

/// The canonical options with `callback` as comrak's `broken_link_callback`.
///
/// comrak's field is an `Arc<Mutex<&mut dyn FnMut …>>` whatever the caller
/// needs; every parse here is synchronous and the options never leave the
/// stack frame that owns the closure, so the `Arc` is never shared across a
/// thread and clippy's `Send`/`Sync` concern does not arise.
#[allow(clippy::arc_with_non_send_sync)]
fn options_with<'c>(callback: comrak::BrokenLinkCallback<'c>) -> comrak::ComrakOptions<'c> {
    crate::markdown::comrak_options_with_broken_link_callback(Arc::new(Mutex::new(callback)))
}

/// The first definition a payload pair disagrees about, as a rejection
/// reason, or `None` when the translation carries exactly the definitions —
/// label for label, destination for destination — its source carried.
///
/// Only definitions a payload carries **itself** are compared; references to
/// the rest of the document are the destination inventory's business. Titles
/// are translatable and not compared. A definition needs `]:` in its text, so
/// a pair with none on either side — or two identical payloads — costs
/// nothing.
///
/// TRACE: R0013-0003
pub(crate) fn definition_change(source: &str, translated: &str) -> Option<String> {
    if source == translated || (!source.contains("]:") && !translated.contains("]:")) {
        return None;
    }
    let mut candidates = label_candidates(source);
    let mut seen: HashSet<String> = candidates.iter().cloned().collect();
    for c in label_candidates(translated) {
        if seen.insert(c.clone()) {
            candidates.push(c);
        }
    }
    if candidates.is_empty() {
        return None;
    }
    let labels = keyed_labels(&candidates);
    let before = probed_destinations(&labels, source);
    let after = probed_destinations(&labels, translated);
    labels
        .iter()
        .zip(before.iter().zip(after.iter()))
        .find(|(_, (s, t))| s != t)
        .map(|(label, (s, t))| {
            format!(
                "reference definition [{}] changed: source {}, translated {}",
                crate::validate::inline::clip(&label.key),
                describe(s.as_deref()),
                describe(t.as_deref()),
            )
        })
}

fn describe(destination: Option<&str>) -> String {
    match destination {
        Some(url) => format!("defines {}", crate::validate::inline::clip(url)),
        None => "defines nothing".to_string(),
    }
}

/// Every label `text` spells as `[…]:`, one line each, in order of first
/// appearance and without repeats ([`one_line`]) — and every label continued
/// across lines once more as comrak's paragraph text holds it
/// ([`labels_as_comrak_reads_them`]). Read with comrak's label rules: a
/// backslash escapes the byte after it, an unescaped `[` ends the attempt
/// (the scan restarts there), and an unescaped `]` ends the label. Scans are
/// disjoint, so the pass is linear in `text`; nothing caps a candidate's
/// length, because only comrak knows how many of its raw bytes the label
/// scanner's 1,000-byte limit counts.
fn label_candidates(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut push = |candidate: String| {
        if seen.insert(candidate.clone()) {
            out.push(candidate);
        }
    };
    let mut continued: Vec<Continued> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'[' {
            i += 1;
            continue;
        }
        let start = i + 1;
        let mut j = start;
        let mut close = None;
        while j < bytes.len() {
            match bytes[j] {
                b'\\' => j += 2,
                b'[' => break,
                b']' => {
                    close = Some(j);
                    break;
                }
                _ => j += 1,
            }
        }
        match close {
            Some(end) => {
                if bytes.get(end + 1) == Some(&b':') {
                    // `start` follows a `[` and `end` sits on a `]`: both are
                    // char boundaries.
                    let raw = &text[start..end];
                    if raw.contains(LINE_BREAK) {
                        continued.push(Continued {
                            open: i,
                            close: end,
                        });
                    }
                    push(one_line(raw));
                }
                i = end + 1;
            }
            // Stopped on a `[` (restart there) or at the end.
            None => i = j.max(i + 1),
        }
    }
    if !continued.is_empty() {
        for label in labels_as_comrak_reads_them(text, &continued) {
            push(label);
        }
    }
    out
}

const LINE_BREAK: [char; 2] = ['\r', '\n'];

/// `raw` on one line: each line break becomes a space, which comrak's
/// normalization treats exactly as it treats the break, and each
/// continuation line's indentation is dropped, as comrak drops it from a
/// paragraph's continuation line (a lazy one keeps it, which shortens this
/// spelling against comrak's and never changes its key). A container's `>`
/// markers stay; only [`labels_as_comrak_reads_them`] knows which bytes
/// comrak strips as one.
fn one_line(raw: &str) -> String {
    let mut lines = raw.split(LINE_BREAK);
    let mut out = lines.next().unwrap_or_default().to_string();
    for line in lines {
        out.push(' ');
        out.push_str(line.trim_start_matches([' ', '\t']));
    }
    out
}

/// A continued candidate: the byte offsets of its `[` and of the `]` that
/// closes it, with a line break between them.
#[derive(Debug, Clone, Copy)]
struct Continued {
    open: usize,
    close: usize,
}

/// The labels comrak reads for the `continued` candidates, each on one line,
/// container prefixes and indentation stripped exactly as comrak's block
/// parser strips them from a paragraph's text.
///
/// Two parses ask. The first writes `[]` ahead of each candidate's `[`, which
/// turns a definition into a full reference `[][label]` whose label comrak's
/// own scanner reads raw out of the paragraph text and hands to
/// `broken_link_callback`; the callback echoes it back as the destination,
/// and every link with no text — which the inserted `[]` makes it — reports
/// one label. A `[]` starts no block and keeps every paragraph holding a
/// candidate alive (a paragraph of definitions alone leaves no node), so the
/// same parse also says which lines each such paragraph spans and where its
/// text starts ([`by_paragraph`]).
///
/// An echo can be hidden: rewriting a definition as a reference makes the
/// rest of its paragraph inline text, and a code span, inline HTML or an
/// inline link's title opened there can run past a later label's `[`. The
/// second parse ([`labels_read_as_code`]) leaves no inline text in front of
/// any label: it reads every continued label as the literal of a code span.
///
/// Only a text with no unescaped bracket in it is echoed — what a label can
/// be — so the echoed texts never nest and hold at most `text`'s own bytes
/// between them, whatever the brackets in `text` do.
fn labels_as_comrak_reads_them(text: &str, continued: &[Continued]) -> Vec<String> {
    let mut asked = String::with_capacity(text.len() + 2 * continued.len());
    let mut from = 0;
    for c in continued {
        asked.push_str(&text[from..c.open]);
        asked.push_str("[]");
        from = c.open;
    }
    asked.push_str(&text[from..]);
    let mut echo = |r: BrokenLinkReference| {
        (!r.normalized.is_empty() && holds_no_bracket(r.original)).then(|| ResolvedReference {
            url: r.original.to_string(),
            title: String::new(),
        })
    };
    let arena = comrak::Arena::new();
    let Ok(root) = crate::markdown::guarded_parse(&arena, &asked, &options_with(&mut echo)) else {
        return Vec::new();
    };
    let mut labels = Vec::new();
    let mut paragraphs = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let lines = ParagraphLines {
            first: data.sourcepos.start.line,
            column: data.sourcepos.start.column,
            last: data.sourcepos.end.line,
        };
        match &data.value {
            // Every echo is bracket-free, as a label is; a textless inline
            // link's destination need not be, and a probe cannot carry it.
            NodeValue::Link(link)
                if node.first_child().is_none() && holds_no_bracket(&link.url) =>
            {
                labels.push(as_spelling(&link.url));
            }
            NodeValue::Paragraph => paragraphs.push(lines),
            NodeValue::Heading(heading) if heading.setext => paragraphs.push(lines),
            _ => {}
        }
    }
    let chains = by_paragraph(text, continued, &paragraphs);
    if !chains.is_empty() {
        labels.extend(labels_read_as_code(text, &chains));
    }
    labels
}

/// `label` as a probe can carry it: each line break a space, as comrak's
/// normalization treats it. comrak hands an echoed label over trimmed, edge
/// line breaks included, so a `\` that a trimmed byte followed would escape
/// the probe's `]`; one space goes back, the key trims it again, and the
/// byte comrak trimmed already counted against the label scanner's limit.
fn as_spelling(label: &str) -> String {
    let mut label = label.replace(LINE_BREAK, " ");
    if label.bytes().rev().take_while(|&b| b == b'\\').count() % 2 == 1 {
        label.push(' ');
    }
    label
}

/// A paragraph (or a setext heading, which is one) of the first parse: its
/// first and last lines and the 1-based byte column its text starts at, as
/// comrak's sourcepos gives them.
struct ParagraphLines {
    first: usize,
    column: usize,
    last: usize,
}

/// The continued candidates one paragraph holds, and the byte offset its
/// text starts at.
struct Chain {
    start: usize,
    labels: Vec<Continued>,
}

/// The offset each line of `text` starts at, lines split as comrak splits
/// them: at `\n`, at `\r\n` and at a `\r` alone.
fn line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\n' => starts.push(i + 1),
            b'\r' => {
                if bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
                starts.push(i + 1);
            }
            _ => {}
        }
        i += 1;
    }
    starts
}

/// `continued`, grouped by the paragraph of `paragraphs` that holds them, in
/// order. A candidate whose `]` lies past its paragraph's last line is no
/// label and is left out; so is a paragraph whose text would start anywhere
/// but at or ahead of its first candidate and past the previous chain, which
/// the first parse's sourcepos never gives — its echoes still count.
///
/// The first parse's lines are `text`'s lines (a `[]` holds no line break),
/// and a comrak column counts bytes from the line's start, a leading
/// byte-order mark included, so `line start + column − 1` is the offset in
/// `text` too: nothing is inserted ahead of a paragraph's first byte on its
/// first line.
fn by_paragraph(text: &str, continued: &[Continued], paragraphs: &[ParagraphLines]) -> Vec<Chain> {
    let starts = line_starts(text);
    let line_of = |offset: usize| starts.partition_point(|&s| s <= offset);
    let mut chains: Vec<Chain> = Vec::new();
    let mut current: Option<(usize, bool)> = None;
    let mut chained_to = 0;
    for &c in continued {
        let line = line_of(c.open);
        let Some(index) = paragraphs
            .partition_point(|p| p.first <= line)
            .checked_sub(1)
        else {
            continue;
        };
        let paragraph = &paragraphs[index];
        if paragraph.last < line_of(c.close) {
            continue;
        }
        if let Some((held, chained)) = current
            && held == index
        {
            if let (true, Some(chain)) = (chained, chains.last_mut()) {
                chain.labels.push(c);
                chained_to = c.close + 1;
            }
            continue;
        }
        let start = paragraph
            .first
            .checked_sub(1)
            .and_then(|line| starts.get(line))
            .zip(paragraph.column.checked_sub(1))
            .map(|(line_start, column)| line_start + column)
            .filter(|&s| chained_to <= s && s <= c.open && text.is_char_boundary(s));
        current = Some((index, start.is_some()));
        if let Some(start) = start {
            chains.push(Chain {
                start,
                labels: vec![c],
            });
            chained_to = c.close + 1;
        }
    }
    chains
}

/// comrak's longest code-span backtick string (`MAXBACKTICKS` in comrak
/// 0.27's inline parser): an opening string longer than this opens nothing.
const MAX_BACKTICKS: usize = 80;

/// The shortest backtick string `label` holds no run of, if one is short
/// enough to open a code span. A label comrak accepts holds at most 1,000
/// bytes, and runs of every length from 1 to 80 need 3,240 backticks, so
/// only a text that is no label goes without one.
fn absent_backtick_run(label: &str) -> Option<usize> {
    let mut held = [false; MAX_BACKTICKS + 1];
    for run in label.split(|c| c != '`') {
        if let Some(slot) = held.get_mut(run.len()) {
            *slot = true;
        }
    }
    (1..=MAX_BACKTICKS).find(|&length| !held[length])
}

/// The asked text of [`labels_read_as_code`], built span by span: every span
/// opens with `x` and its backtick string and is closed by the next span's
/// opening, or by [`CodeSpans::close`].
struct CodeSpans {
    asked: String,
    open: Option<usize>,
}

impl CodeSpans {
    fn close(&mut self) {
        if let Some(ticks) = self.open.take() {
            self.asked.extend(std::iter::repeat_n('`', ticks));
        }
    }

    fn start(&mut self, ticks: usize) {
        self.close();
        self.asked.push('x');
        self.asked.extend(std::iter::repeat_n('`', ticks));
        self.open = Some(ticks);
    }

    /// Text nobody reads back: its backticks become `x`, so a single
    /// backtick opens and closes it.
    fn stretch(&mut self, text: &str) {
        self.start(1);
        self.asked
            .extend(text.chars().map(|c| if c == '`' { 'x' } else { c }));
    }

    /// `[label]`, verbatim, between backtick strings it holds no run of.
    fn label(&mut self, bracketed: &str) {
        match absent_backtick_run(bracketed) {
            Some(ticks) => {
                self.start(ticks);
                self.asked.push_str(bracketed);
            }
            None => self.stretch(bracketed),
        }
    }
}

/// Every continued label of `chains`, read by comrak as the literal of a code
/// span.
///
/// Each chain's paragraph is laid into consecutive code spans from the byte
/// its text starts at to the `]` of its last continued candidate: one span
/// per candidate holding exactly `[label]`, and one per stretch of text
/// before and between them. Code span text is never parsed as inlines, and
/// the chain starts at the paragraph's first byte, so no inline construct
/// of the source's own opens anywhere up to the last label; the paragraph's
/// text now starts with `x`, so comrak strips no definition from its head
/// either. A span's backtick string must not occur inside it: a stretch's
/// backticks become `x` (its text is never read), and a label's span takes
/// the shortest string its label holds no run of ([`absent_backtick_run`]).
///
/// An `x` precedes every opening string, so none starts a line (where three
/// backticks would open a fenced code block) and none merges with the
/// closing string ahead of it; a closing string that starts a line closes a
/// stretch and is one backtick. Nothing written in — an `x`, a backtick
/// string, an `x` for a stretch's backtick — is a line break, a `|` or a
/// `\`, or starts a block, so the paragraphs keep the lines and table cells
/// the first parse found. comrak's literal is the paragraph text
/// between the strings with each line break a space, so a label span's
/// literal is `[`, the label exactly as comrak's definition scanner reads it,
/// and `]`. Any other code span shaped that way is one more harmless
/// candidate.
fn labels_read_as_code(text: &str, chains: &[Chain]) -> Vec<String> {
    let mut spans = CodeSpans {
        asked: String::with_capacity(text.len() + text.len() / 2),
        open: None,
    };
    let mut from = 0;
    for chain in chains {
        spans.asked.push_str(&text[from..chain.start]);
        let mut at = chain.start;
        for label in &chain.labels {
            if at < label.open {
                spans.stretch(&text[at..label.open]);
            }
            spans.label(&text[label.open..=label.close]);
            at = label.close + 1;
        }
        spans.close();
        from = at;
    }
    spans.asked.push_str(&text[from..]);
    let arena = comrak::Arena::new();
    let Ok(root) =
        crate::markdown::guarded_parse(&arena, &spans.asked, &crate::markdown::comrak_options())
    else {
        return Vec::new();
    };
    root.descendants()
        .filter_map(|n| match &n.data.borrow().value {
            NodeValue::Code(code) => code
                .literal
                .strip_prefix('[')
                .and_then(|l| l.strip_suffix(']'))
                .filter(|l| holds_no_bracket(l))
                .map(as_spelling),
            _ => None,
        })
        .collect()
}

/// Whether `text` holds no `[` or `]` that a backslash does not escape —
/// comrak's label scanner stops at either.
fn holds_no_bracket(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'[' | b']' => return false,
            _ => i += 1,
        }
    }
    true
}

/// A label as the probes carry it: the `spelling` a definition might use,
/// asked of comrak verbatim, and the `key` comrak normalized it to.
///
/// The probe always carries the spelling, never the key. comrak's key is not
/// always a label comrak reads back to itself: comrak trims only ASCII
/// whitespace from a label's edges but collapses any Unicode whitespace into
/// an ASCII space, so `[foo\u{a0}]` keys as `foo ` and `[][foo ]` asks for
/// `foo`; a `\` left last once the space after it is trimmed (`[a\ ]` keys as
/// `a\`) escapes the probe's own closing `]`; and case folding can grow a
/// label past the 1,000 bytes comrak's label scanner accepts.
struct Label {
    key: String,
    spelling: String,
}

/// One single-line probe per spelling, each its own paragraph.
fn probe_text<'s>(spellings: impl Iterator<Item = &'s str>) -> String {
    let mut text = String::new();
    for spelling in spellings {
        text.push_str("[][");
        text.push_str(spelling);
        text.push_str("]\n\n");
    }
    text
}

/// The empty-text link that opens `probe`'s paragraph, if the probe became
/// one. A probe that did not resolve stays text — and if its label happens to
/// hold a bare URL, the autolink extension may still find a link inside that
/// text, which is why only a childless link in first position counts.
fn probe_link<'a>(probe: &'a AstNode<'a>) -> Option<String> {
    let first = probe.first_child()?;
    match &first.data.borrow().value {
        NodeValue::Link(link) if first.first_child().is_none() => Some(link.url.clone()),
        _ => None,
    }
}

/// comrak's key for each candidate spelling, sorted by key, with the first
/// candidate that produced a key kept as its spelling.
///
/// Each probe is parsed with a callback that answers every lookup with the
/// key comrak hands it, so the link a probe becomes carries comrak's key for
/// that very spelling and the probes are read back by position. A spelling
/// comrak's label scanner refuses (or one that keys as nothing) becomes no
/// link and drops out. One spelling per key, because two spellings of one
/// label would each spend comrak's expansion budget on the same destination.
fn keyed_labels(candidates: &[String]) -> Vec<Label> {
    let mut keyed: BTreeMap<String, String> = BTreeMap::new();
    let mut answer = |r: BrokenLinkReference| {
        (!r.normalized.is_empty()).then(|| ResolvedReference {
            url: r.normalized.to_string(),
            title: String::new(),
        })
    };
    let arena = comrak::Arena::new();
    // Probes nest nothing, so the depth guard cannot refuse them.
    if let Ok(root) = crate::markdown::guarded_parse(
        &arena,
        &probe_text(candidates.iter().map(String::as_str)),
        &options_with(&mut answer),
    ) {
        for (probe, spelling) in root.children().zip(candidates) {
            if let Some(key) = probe_link(probe) {
                keyed.entry(key).or_insert_with(|| spelling.clone());
            }
        }
    }
    keyed
        .into_iter()
        .map(|(key, spelling)| Label { key, spelling })
        .collect()
}

/// For each label, the destination `body` defines for it, read off the probe
/// paragraphs placed ahead of `body`: a probe resolved when its paragraph
/// opens with the empty-text link it became ([`probe_link`]).
fn probed_destinations(labels: &[Label], body: &str) -> Vec<Option<String>> {
    let mut text = probe_text(labels.iter().map(|l| l.spelling.as_str()));
    // comrak drops a byte-order mark only from a document's first line, and
    // `body` is no longer first; drop it here so the probes leave `body`
    // parsed exactly as it parses alone.
    text.push_str(body.strip_prefix('\u{feff}').unwrap_or(body));
    let arena = comrak::Arena::new();
    let Ok(root) =
        crate::markdown::guarded_parse(&arena, &text, &crate::markdown::comrak_options())
    else {
        return vec![None; labels.len()];
    };
    let mut out: Vec<Option<String>> = root.children().take(labels.len()).map(probe_link).collect();
    out.resize(labels.len(), None);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(src: &str) -> ReferenceMap {
        ReferenceMap::of(&crate::markdown::parse(src).expect("parses"))
    }

    /// The destination `text`'s first link resolves to under `refs`.
    fn first_link(refs: &ReferenceMap, text: &str, budget: usize) -> Option<String> {
        let arena = comrak::Arena::new();
        let root = refs.parse(&arena, text, budget).expect("parses");
        root.descendants()
            .find_map(|n| match &n.data.borrow().value {
                NodeValue::Link(l) => Some(l.url.clone()),
                _ => None,
            })
    }

    #[test]
    fn definitions_inside_blocks_are_in_the_map() {
        let quoted = map("> [q]: https://q.example/\n>\n> holder\n");
        assert_eq!(quoted.get("q"), Some("https://q.example/"));
        let listed = map("- [l]: https://l.example/\n- item\n");
        assert_eq!(listed.get("l"), Some("https://l.example/"));
        let prefixed = map("[p]: https://p.example/\nIntro.\n");
        assert_eq!(prefixed.get("p"), Some("https://p.example/"));
        let nested = map("- > - [n]: https://n.example/\n");
        assert_eq!(nested.get("n"), Some("https://n.example/"));
        let gap = map("Prose.\n\n[g]: https://g.example/\n");
        assert_eq!(gap.get("g"), Some("https://g.example/"));
    }

    #[test]
    fn the_first_definition_wins_wherever_it_sits() {
        // The gap pool this replaces held only the later definition.
        let refs =
            map("> [ref]: https://a.example/\n\nSee [x][ref].\n\n[ref]: https://b.example/\n");
        assert_eq!(refs.get("ref"), Some("https://a.example/"));
    }

    #[test]
    fn labels_are_comrak_normalized() {
        let refs = map("[Foo   BAR]: /fb\n\n[a\\]b]: /esc\n\n> [multi\n> line]: /ml\n");
        assert_eq!(
            refs.get("foo bar"),
            Some("/fb"),
            "case-folded, whitespace collapsed"
        );
        assert_eq!(
            refs.get("a\\]b"),
            Some("/esc"),
            "an escaped bracket stays in the label"
        );
        assert_eq!(
            refs.get("multi line"),
            Some("/ml"),
            "a label continued inside a blockquote loses its `>` prefix"
        );
    }

    /// comrak's own answer: where `reference` goes when it sits in one
    /// document with `defs`, parsed whole, with no help from this module.
    fn native(defs: &str, reference: &str) -> Option<String> {
        first_link(
            &ReferenceMap::empty(),
            &format!("{reference}\n\n{defs}"),
            usize::MAX,
        )
    }

    /// Labels whose comrak key does not survive being parsed a second time as
    /// a label: comrak trims only ASCII whitespace from a label's edges but
    /// collapses Unicode whitespace into an ASCII space, an unescaped `\` ahead
    /// of the closing `]` escapes it, and case folding can grow a label past
    /// the 1,000 bytes the label scanner accepts. Each is asked of a gap, a
    /// blockquote and a list item, and the map must agree with comrak's
    /// whole-document resolution.
    const EDGE_LABELS: &[(&str, &str)] = &[
        ("foo\u{a0}", "[x][foo\u{a0}]"),
        ("\u{3000}참고", "[x][\u{3000}참고]"),
        ("a\u{2028}", "[x][a\u{2028}]"),
        ("a\\ ", "[x][a\\ ]"),
        ("a\\\\\\ ", "[x][a\\\\\\ ]"),
        ("Straße", "[x][STRASSE]"),
    ];

    #[test]
    fn a_label_comrak_cannot_reparse_from_its_key_still_resolves() {
        let grown = "\u{390}".repeat(300);
        let grown_ref = format!("[x][{grown}]");
        let cases = EDGE_LABELS
            .iter()
            .map(|(l, r)| ((*l).to_string(), (*r).to_string()))
            .chain([(grown.clone(), grown_ref)]);
        for (label, reference) in cases {
            for defs in [
                format!("[{label}]: https://example.com/d\n"),
                format!("> [{label}]: https://example.com/d\n>\n> holder\n"),
                format!("- [{label}]: https://example.com/d\n- item\n"),
            ] {
                let want = native(&defs, &reference);
                assert_eq!(
                    want.as_deref(),
                    Some("https://example.com/d"),
                    "comrak resolves {reference:?} against {defs:?}"
                );
                assert_eq!(
                    first_link(&map(&defs), &reference, expansion_budget(&[])),
                    want,
                    "{reference:?} against {defs:?}"
                );
            }
        }
    }

    #[test]
    fn a_label_continued_inside_a_container_resolves_as_comrak_reads_it() {
        for (defs, reference) in [
            ("> [multi\n> line]: /d\n", "[x][multi line]"),
            ("- [multi\n  line]: /d\n", "[x][multi line]"),
            ("> > [multi\n> > line]: /d\n", "[x][multi line]"),
            ("> [multi\nline]: /d\n", "[x][multi line]"),
            // The continuation's own `>` is indented past a marker, so it is
            // label text, not a container prefix.
            ("> [multi\n>     > line]: /d\n", "[x][multi > line]"),
            ("> [multi\\\n> line]: /d\n", "[x][multi\\ line]"),
            // A lazy continuation: four columns of indentation are too many
            // for a blockquote marker, so its `>` is label text.
            ("> [multi\n    > line]: /d\n", "[x][multi > line]"),
            // The third line keeps one marker for its container and one `>`
            // as label text; the second keeps both as markers.
            ("> > [a\n> > b\n>     > c]: /d\n", "[x][a b > c]"),
            // Inside a list item, a marker indented past the item's content
            // column by three spaces is still a marker.
            ("- > [multi\n     > line]: /d\n", "[x][multi line]"),
            // comrak trims the line break after the `\`, which would then
            // escape a probe's closing `]`.
            ("> [a\\\n> ]: /d\n", "[x][a\\ ]"),
            ("> [a\\\n> b\\\n>  ]: /d\n", "[x][a\\ b\\ ]"),
        ] {
            let want = native(defs, reference);
            assert_eq!(
                want.as_deref(),
                Some("/d"),
                "comrak resolves {reference:?} against {defs:?}"
            );
            assert_eq!(
                first_link(&map(defs), reference, expansion_budget(&[])),
                want,
                "{reference:?} against {defs:?}"
            );
        }
    }

    /// A label spread over many lines: 985 bytes once comrak has stripped
    /// its container prefixes and each continuation line's indentation —
    /// inside the 1,000 bytes comrak's label scanner accepts — though its
    /// raw text between the brackets runs far past that.
    fn long_label(line_break: &str) -> String {
        (0..58)
            .map(|i| format!("word{i:02}xxxxxxxxxx"))
            .collect::<Vec<_>>()
            .join(line_break)
    }

    /// The long label defined between blocks with indented continuation
    /// lines, inside a nested list item, and inside a deep blockquote.
    fn long_label_definitions(url: &str) -> [String; 3] {
        [
            format!("[{}]: {url}\n", long_label("\n        ")),
            format!("- outer\n  - [{}]: {url}\n", long_label("\n    ")),
            format!("> > > > [{}]: {url}\n", long_label("\n> > > > ")),
        ]
    }

    #[test]
    fn a_long_label_spread_over_lines_resolves_as_comrak_reads_it() {
        let reference = format!("[x][{}]", long_label(" "));
        for defs in long_label_definitions("https://example.com/d") {
            assert!(defs.len() > 1_100, "the raw label runs past 1,000 bytes");
            let want = native(&defs, &reference);
            assert_eq!(
                want.as_deref(),
                Some("https://example.com/d"),
                "comrak resolves the reference against {defs:?}"
            );
            assert_eq!(
                first_link(&map(&defs), &reference, expansion_budget(&[])),
                want,
                "{defs:?}"
            );
        }
    }

    /// Paragraphs of definitions whose labels continue across blockquote
    /// lines, where rewriting one definition as a reference would leave its
    /// title as inline text that opens a construct running past the next
    /// label's `[`: a code span, an inline link's or image's title, inline
    /// HTML, an HTML comment — and the same behind a single-line definition,
    /// across three labels, and behind a label comrak refuses.
    fn hidden_label_definitions() -> Vec<String> {
        vec![
            "> [a\n> a]: /a \"`\"\n> [b\n> c]: /b \"`\"\n".to_string(),
            "> [a\n> a]: /a \"[x](/u '\"\n> [b\n> c]: /b \"')\"\n".to_string(),
            "> [a\n> a]: /a \"![x](/u '\"\n> [b\n> c]: /b \"')\"\n".to_string(),
            "> [a\n> a]: /a \"<span title='\"\n> [b\n> c]: /b \"'>\"\n".to_string(),
            "> [a\n> a]: /a \"<!--\"\n> [b\n> c]: /b \"-->\"\n".to_string(),
            "> [a\n> a]: /a \"`\"\n> [s]: /s\n> [b\n> c]: /b \"`\"\n".to_string(),
            "> [a\n> a]: /a \"<!--\"\n> [d\n> d]: /d \"<!--\"\n> [b\n> c]: /b \"-->\"\n"
                .to_string(),
            // A title holding a candidate too long to be a label, whose own
            // text opens a code span.
            format!(
                "> [a\n> a]: /a \"[{}`\n> q]: t\"\n> [b\n> c]: /b \"`\"\n",
                "z".repeat(1_100)
            ),
        ]
    }

    #[test]
    fn no_inline_construct_can_hide_a_later_continued_label() {
        let mut shapes = hidden_label_definitions();
        shapes.extend(
            [
                // Lazy lines, other line endings, a tab after the marker,
                // nesting, and a stretch holding every backtick run length.
                "> [a\na]: /a \"`\"\n[b\n> c]: /b \"`\"\n",
                "> [a\r\n> a]: /a \"`\"\r\n> [b\r\n> c]: /b \"`\"\r\n",
                "> [a\r> a]: /a \"`\"\r> [b\r> c]: /b \"`\"\r",
                ">\t[a\n>\ta]: /a \"`\"\n>\t[b\n>\tc]: /b \"`\"\n",
                "- > [a\n  > a]: /a \"`\"\n  > [b\n  > c]: /b \"`\"\n",
                "> > > [a\n> > > a]: /a \"`\"\n> > > [b\n> > > c]: /b \"`\"\n",
            ]
            .map(str::to_string),
        );
        let runs = (1..=90).map(|n| "`".repeat(n)).collect::<Vec<_>>();
        shapes.push(format!(
            "> [a\n> a]: /a \"{}\"\n> [b\n> c]: /b \"`\"\n",
            runs.join(" ")
        ));
        for defs in &shapes {
            for label in ["a a", "b c"] {
                let reference = format!("[x][{label}]");
                let want = native(defs, &reference);
                assert!(
                    want.is_some(),
                    "comrak resolves {reference:?} against {defs:?}"
                );
                assert_eq!(
                    first_link(&map(defs), &reference, expansion_budget(&[])),
                    want,
                    "{reference:?} against {defs:?}"
                );
            }
        }
    }

    #[test]
    fn a_label_holding_many_backtick_runs_is_read_whole() {
        // Runs of every length from 1 to 40 (861 bytes with the separators):
        // a label comrak accepts, read between backtick strings of 41.
        let label = (1..=40)
            .map(|n| "`".repeat(n))
            .collect::<Vec<_>>()
            .join("x");
        let defs = format!("> [a\n> a]: /a \"`\"\n> [{label}\n> c]: /b \"`\"\n");
        let reference = format!("[x][{label} c]");
        assert_eq!(native(&defs, &reference).as_deref(), Some("/b"));
        assert_eq!(
            first_link(&map(&defs), &reference, expansion_budget(&[])).as_deref(),
            Some("/b")
        );
    }

    #[test]
    fn a_leading_byte_order_mark_does_not_move_a_paragraph_s_first_byte() {
        let defs = "\u{feff}> [a\n> a]: /a \"`\"\n> [b\n> c]: /b \"`\"\n";
        let refs = map(defs);
        assert_eq!(refs.get("a a"), Some("/a"));
        assert_eq!(refs.get("b c"), Some("/b"));
    }

    #[test]
    fn definition_change_sees_a_label_behind_an_inline_construct() {
        for source in hidden_label_definitions() {
            let source = format!("{source}>\n> holder");
            assert_eq!(
                definition_change(&source, &source.replace("holder", "보유자")),
                None,
                "{source:?}"
            );
            let reason =
                definition_change(&source, &source.replace("/b ", "https://evil.example/ "))
                    .unwrap_or_else(|| panic!("a rewritten definition is a change: {source:?}"));
            assert!(reason.contains("https://evil.example/"), "{reason}");
        }
    }

    #[test]
    fn definition_change_sees_a_long_label_spread_over_lines() {
        for (source, rewritten) in long_label_definitions("https://example.com/")
            .into_iter()
            .zip(long_label_definitions("https://evil.example/"))
        {
            let source = format!("{source}\nholder");
            assert_eq!(
                definition_change(&source, &source.replace("holder", "보유자")),
                None
            );
            let reason = definition_change(&source, &format!("{rewritten}\n보유자"))
                .unwrap_or_else(|| panic!("a rewritten definition is a change: {source:?}"));
            assert!(reason.contains("https://evil.example/"), "{reason}");
            assert!(
                definition_change(&source, "보유자").is_some(),
                "a deleted definition is a change: {source:?}"
            );
        }
    }

    #[test]
    fn definition_change_sees_labels_comrak_cannot_reparse_from_their_key() {
        for (label, _) in EDGE_LABELS {
            let source = format!("> [{label}]: https://example.com/\n>\n> holder");
            let kept = format!("> [{label}]: https://example.com/\n>\n> 보유자");
            assert_eq!(definition_change(&source, &kept), None, "{label:?}");
            let rewritten = format!("> [{label}]: https://evil.example/\n>\n> 보유자");
            let reason = definition_change(&source, &rewritten)
                .unwrap_or_else(|| panic!("{label:?}: a rewritten definition is a change"));
            assert!(reason.contains("https://evil.example/"), "{reason}");
            assert!(
                definition_change(&source, "> 보유자").is_some(),
                "{label:?}: a deleted definition is a change"
            );
            assert!(
                definition_change(
                    "Intro.",
                    &format!("[{label}]: https://evil.example/\n소개.")
                )
                .is_some(),
                "{label:?}: an injected definition is a change"
            );
        }
    }

    #[test]
    fn a_lookalike_that_comrak_does_not_read_as_a_definition_is_not_in_the_map() {
        assert!(map("```\n[x]: /in-code\n```\n").is_empty());
        assert!(map("Index a[i]: the value.\n").is_empty());
        assert!(
            map("Para\n[y]: /lazy\n").is_empty(),
            "a definition cannot interrupt a paragraph"
        );
    }

    #[test]
    fn an_unclosed_fence_cannot_swallow_the_probes() {
        let refs = map("[ref]: /u\n\n```\nunclosed\n[late]: /in-fence\n");
        assert_eq!(refs.get("ref"), Some("/u"));
        assert_eq!(refs.get("late"), None);
    }

    #[test]
    fn a_leading_byte_order_mark_does_not_hide_the_first_definition() {
        let refs = map("\u{feff}[ref]: /bom\n\nSee [x][ref].\n");
        assert_eq!(refs.get("ref"), Some("/bom"));
    }

    #[test]
    fn an_html_document_has_no_definitions() {
        let doc = transync_syntax::intake::html::parse("<p>[ref]: /u</p>\n");
        assert!(ReferenceMap::of(&doc).is_empty());
    }

    #[test]
    fn a_fragment_resolves_from_the_map_and_its_own_definitions_first() {
        let refs = map("> [ref]: https://doc.example/\n");
        let budget = expansion_budget(&[]);
        assert_eq!(
            first_link(&refs, "See [x][ref].", budget).as_deref(),
            Some("https://doc.example/")
        );
        assert_eq!(
            first_link(
                &refs,
                "See [x][ref].\n\n[ref]: https://local.example/",
                budget
            )
            .as_deref(),
            Some("https://local.example/"),
            "a label the fragment defines resolves locally, as comrak would"
        );
        assert_eq!(first_link(&refs, "See [x][nope].", budget), None);
        assert_eq!(
            first_link(&ReferenceMap::empty(), "See [x][ref].", budget),
            None
        );
    }

    #[test]
    fn callback_resolutions_are_charged_against_the_budget() {
        let url = format!("https://example.com/{}", "a".repeat(80));
        let refs = map(&format!("[r]: {url}\n"));
        let text = "[a][r] ".repeat(10);
        let count = |budget: usize| {
            let arena = comrak::Arena::new();
            let root = refs.parse(&arena, &text, budget).expect("parses");
            root.descendants()
                .filter(|n| matches!(n.data.borrow().value, NodeValue::Link(_)))
                .count()
        };
        assert_eq!(count(usize::MAX), 10);
        assert_eq!(
            count(url.len() * 3),
            3,
            "a lookup past the budget resolves to nothing"
        );
    }

    #[test]
    fn definition_change_sees_rewrites_deletions_and_injections() {
        let kept = "> [ref]: https://example.com/ \"Title\"\n>\n> holder";
        assert_eq!(definition_change(kept, kept), None);
        assert_eq!(
            definition_change(kept, "> [ref]: https://example.com/ \"제목\"\n>\n> 보유자"),
            None,
            "a title is translatable"
        );
        let rewritten = definition_change(kept, "> [ref]: https://evil.example/\n>\n> 보유자")
            .expect("rewritten");
        assert!(rewritten.contains("[ref]"), "{rewritten}");
        assert!(
            rewritten.contains("source defines https://example.com/"),
            "{rewritten}"
        );
        assert!(
            rewritten.contains("translated defines https://evil.example/"),
            "{rewritten}"
        );
        let deleted = definition_change(kept, "> 보유자").expect("deleted");
        assert!(deleted.contains("translated defines nothing"), "{deleted}");
        let injected =
            definition_change("Intro.", "[ref]: https://evil.example/\n소개.").expect("injected");
        assert!(injected.contains("source defines nothing"), "{injected}");
        let relabelled = definition_change(kept, "> [참조]: https://example.com/\n>\n> 보유자")
            .expect("a translated definition label breaks every reference to it");
        assert!(relabelled.contains("reference definition"), "{relabelled}");
        assert_eq!(
            definition_change("Index a[i]: x.", "인덱스 a[i]: 값."),
            None,
            "prose that spells `]:` defines nothing on either side"
        );
    }
}
