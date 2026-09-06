//! HTML mechanics for transync: tag scanning, the pairing discipline,
//! element extents, text-segment extraction/splice, and render-path fragment
//! balancing (`lol_html`).
//!
//! One pinned Settings source (`rewriter_settings`) feeds every pass so the
//! extract pass and the splice pass can never disagree on coalescing or drop
//! decisions.
//!
//! **Structural intake does not live here.** Classification, `BlockKind`
//! assignment, block ids and `ast_path` are `transync-syntax`'s job — the
//! `transync_syntax::intake::html` module wave 3 will add; it does not exist
//! yet — and this crate is the mechanics layer underneath it. The name says "html" because the capability is HTML,
//! not because the crate decides what an HTML block *is*.
//!
//! **The module formerly called `htmlseg` inside `transync-syntax` is this
//! crate.** Records dated before 2026-08-20 — ADR-0018, ADR-0003,
//! `docs/project/stub-manifest.md`, `docs/project/status.md`,
//! `docs/project/open-issues-archive.md`,
//! `docs/project/implementation-slice-checklists.md`, and the investigation
//! bundle — name `htmlseg`, and they are dated records that are not
//! rewritten. A reader who greps for `htmlseg` and finds nothing is looking
//! at this.
//!
//! Spec: docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md
//! §5. The original engine spec, still the authority on the extract/splice
//! algorithm, is
//! docs/superpowers/specs/2026-08-03-html-content-translation-design.md
//! §3.2–§3.4.
//!
//! TRACE: ADR-0018
//! TRACE: DCR-0016
//! TRACE: DCR-0032

use lol_html::html_content::{ContentType, TextType};
use lol_html::{
    DocumentContentHandlers, ElementContentHandlers, EndTagHandler, HtmlRewriter, MemorySettings,
    Selector, Settings, doc_text, element,
};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// HTML void elements: they never get an end tag, so pushing them onto the
/// open-element stack would poison the parent label of every later text node.
///
/// Eighteen names: the thirteen on HTML's current *syntax* list, plus five
/// whose elements the spec dropped while the tree-construction rule that
/// makes them void survived — `basefont`, `bgsound`, `frame`, `keygen`,
/// `param`. All five measured never-open in headless Chromium (`<keygen>x`
/// leaves `x` a SIBLING, not a child). The four beyond `param` joined when
/// [`walk_elements`] began pushing self-closing non-void tags: without them,
/// `<keygen/>` would newly earn an appended `</keygen>` a browser never
/// mints.
///
/// The rule applies **in HTML content only** (ti `48f3c6`, DCR-0043). It used
/// to be global, to keep the balancer from ever appending a `</br>` — HTML's
/// end-tag-`br` rule turns one back into a fresh `<br>`, so the balancer would
/// mint structure instead of repairing it — at the cost of dropping a real
/// foreign `<svg><link>…</link>`'s closer as an orphan. That trade expired
/// when [`walk_elements`] learned breakout (DCR-0041): `br` is a breakout tag,
/// so `<svg><br>` leaves foreign content before the tag is processed and meets
/// this list in HTML content after all. Five names are in that position —
/// `br`, `embed`, `hr`, `img`, `meta` — and
/// `a_void_name_that_is_also_a_breakout_tag_never_opens_inside_foreign_content`
/// pins the set.
///
/// `image` is deliberately NOT here, and the reason is the tree builder's
/// rather than this list's: **HTML has no void element called `image`.** "A
/// start tag whose tag name is `image`" is handled by rewriting the token's
/// name to `img` and reprocessing it, so `<image>x` leaves `x` a sibling
/// because `img` is void, not because `image` is.
///
/// [`scan_tags`] performs that rename since ti `e923ef`, which is what makes
/// this exclusion correct rather than merely defensible: the name never
/// reaches this list, so the list stays a list of void NAMES. A real, closable
/// `<svg><image>…</image>` is untouched for two independent reasons — the
/// rename is an HTML-content rule, and voidness is not consulted in foreign
/// content at all (ti `48f3c6`).
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// Elements whose content HTML tokenizes as raw text / RCDATA **in HTML
/// content**. HTML ignores the self-closing flag on them there:
/// `<textarea/>` opens RCDATA and swallows every later sibling as text. The
/// balancer therefore treats a self-closing start tag for these as OPEN so a
/// close tag is appended, and [`scan_tags`] enters raw-text state for all of
/// them regardless of the flag (DCR-0016 Part D; R0002-0020 brought
/// textarea/title into the tokenizer half, which had covered only script and
/// style).
///
/// Inside `<svg>`/`<math>` none of that applies — they are ordinary foreign
/// elements whose contents are markup and whose `/` is honoured (ti
/// `2e2453`, DCR-0042).
///
/// **Eight names since R0010-0031 (DCR-0050), not four.** The generic
/// raw-text element parsing algorithm is reached from "in body" for `xmp`,
/// `iframe` and `noembed`, and from "in head" for `noframes`, exactly as it
/// is for `style`; leaving them out read their contents as markup. The harm
/// is the pairing one, not a cosmetic one: `<div><iframe></div></iframe>` is
/// ONE CommonMark type-6 html block (`div` and `iframe` are both type-6
/// names), the walk popped `iframe` at `</div>` and deleted the author's
/// `</iframe>` as an orphan, and the emitted `<div><iframe></div>` then let a
/// browser's iframe raw text swallow the sync wrapper's own `</div>` and
/// every following block to EOF — `contracts.md` §4a, reachable from
/// untrusted source Markdown (invariant 7).
///
/// `xmp` is in [`implicitly_closes`]' paragraph-closing set as well, and both
/// facts hold at once: it closes an open `<p>` *and* its contents are raw
/// text. Being on this list does not remove it from that one.
///
/// **`noscript` is deliberately NOT here.** HTML makes it generic raw text
/// only *when the scripting flag is enabled*, which is a property of the
/// parsing context and not of the name — and this predicate answers the NAME
/// question, leaving context to the caller, exactly as [`is_void`] does. The
/// case is real (every pane this crate feeds is mounted by JavaScript, so the
/// flag is set there) and is filed rather than decided here.
const RAW_TEXT_ELEMENTS: &[&str] = &[
    "script", "style", "textarea", "title", "xmp", "iframe", "noembed", "noframes",
];

/// Pinned memory ceiling for the rewriter (spec §3.2 "pinned Settings").
/// Generous for real README blocks; the test hook lowers it to force the
/// error path deterministically.
const DEFAULT_MAX_MEMORY_BYTES: usize = 4 * 1024 * 1024;

/// One text node of the source block, in document order.
#[derive(Debug, Clone)]
pub(crate) struct NodeRecord {
    pub decoded: String,
    pub label: String,
    pub kept: bool,
}

/// The kept text nodes of a block, paired with their parent-element labels.
#[derive(Debug, Clone, Default)]
pub struct HtmlSegments {
    pub texts: Vec<String>,
    pub labels: Vec<String>,
}

/// Is `tag` an HTML void element? Void elements never get an end tag, so a
/// stack walker must not push them and a balancer must not close them.
///
/// **In HTML content.** Foreign content has no void elements at all: "any
/// other start tag" inserts a foreign element there and only the self-closing
/// flag pops it, so `<svg><link>a</link>` is a genuine closable SVG element
/// (ti `48f3c6`). Like [`is_raw_text`], this answers the NAME question and
/// leaves the context to the caller; [`scan_tags`] is the one that pairs the
/// two.
///
/// **ASCII-case-insensitive**, because HTML tag names are: `<BR>` names the
/// same element as `<br>`, and a helper that answered `false` for the first
/// would tell an external caller to push a void tag onto its open-element
/// stack. Every in-crate caller already hands over a lowercased name
/// ([`scan_tags`] and the `lol_html` pass both normalize at the point they
/// read the name), so the fold decides nothing here and everything for a
/// caller outside this crate — which is who the `pub` is for (R0009-0064).
pub fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS
        .iter()
        .any(|name| name.eq_ignore_ascii_case(tag))
}

/// Does `tag` hold raw text / RCDATA (`script`, `style`, `textarea`,
/// `title`, `xmp`, `iframe`, `noembed`, `noframes`) **in HTML content**? A
/// browser never tokenizes their content as markup there, and HTML ignores
/// the self-closing flag on them (DCR-0016 Part D; the last four since
/// R0010-0031 / DCR-0050).
///
/// The qualifier is load-bearing (ti `2e2453`). These states are entered by
/// the tree construction stage from the "in body" and "in head" insertion
/// modes; foreign content has no such rule, so inside `<svg>`/`<math>` all
/// eight are ordinary foreign elements whose contents are markup and whose
/// self-closing `/` is honoured. This predicate answers the NAME question
/// only — the caller owns the context, and [`scan_tags`] is the one that
/// pairs the two.
///
/// ASCII-case-insensitive for the same reason as [`is_void`]: `<SCRIPT>` is
/// raw text, and answering otherwise would invite a caller to scan its
/// contents for markup (R0009-0064).
pub fn is_raw_text(tag: &str) -> bool {
    RAW_TEXT_ELEMENTS
        .iter()
        .any(|name| name.eq_ignore_ascii_case(tag))
}

/// The two elements that switch an HTML parser into *foreign content*, where
/// a `<![CDATA[` is a real CDATA section rather than a bogus comment
/// (R0003-0067). Their descendants are foreign too, but a flat scanner
/// cannot know the adjusted current node — counting these roots is the
/// honest bound.
fn is_foreign_root(tag: &str) -> bool {
    matches!(tag, "svg" | "math")
}

/// Text types that carry translatable content. `RCData` covers `<textarea>`
/// and `<title>`; `ScriptData`/`RawText`/`PlainText` (script, style, xmp,
/// noscript, …) are deliberately excluded, and comments/CDATA never reach a
/// text handler at all.
fn wanted_text_type(t: TextType) -> bool {
    matches!(t, TextType::Data | TextType::RCData)
}

/// The single source of rewriter Settings for every pass over an HTML block
/// (spec §3.2 design note 1). Handlers vary per pass; everything else —
/// encoding, memory ceiling, strictness — is pinned here so the extract pass
/// and the splice pass can never disagree on how the document is tokenized.
fn rewriter_settings<'h, 's>(
    element_content_handlers: Vec<(Cow<'s, Selector>, ElementContentHandlers<'h>)>,
    document_content_handlers: Vec<DocumentContentHandlers<'h>>,
    max_bytes: usize,
) -> Settings<'h, 's> {
    Settings {
        element_content_handlers,
        document_content_handlers,
        memory_settings: MemorySettings {
            max_allowed_memory_usage: max_bytes,
            // `Arena::new` debug-asserts that the preallocation fits inside
            // `max_allowed_memory_usage`; clamping keeps lol_html's default
            // preallocation in production while letting the low-cap test hook
            // return `Err` instead of panicking.
            preallocated_parsing_buffer_size: MemorySettings::default()
                .preallocated_parsing_buffer_size
                .min(max_bytes),
        },
        ..Settings::new()
    }
}

/// Uniform rewriter-failure text. The phase tag is the only thing that
/// separates a mid-stream failure from a finalization failure, and both ride
/// the same degrade path (spec §3.2 extraction failure, §3.3 splice failure),
/// so triage depends on the message.
fn rewriter_err(phase: &str, e: impl std::fmt::Display) -> String {
    format!("html rewriter failed ({phase}): {e}")
}

/// Walks `block` and returns one record per text node in document order,
/// including the whitespace-only nodes that [`extract`] drops (`kept: false`)
/// and the `<template>` content that spec §9 puts in the same bucket as
/// script/style.
pub(crate) fn scan(block: &str) -> Result<Vec<NodeRecord>, String> {
    scan_with_memory_cap(block, DEFAULT_MAX_MEMORY_BYTES)
}

pub(crate) fn scan_with_memory_cap(
    block: &str,
    max_bytes: usize,
) -> Result<Vec<NodeRecord>, String> {
    scan_chunks(&[block], max_bytes)
}

/// [`scan`] over an explicit sequence of `write()` chunks. Production always
/// hands over a single chunk; the multi-chunk form exists so the coalescing
/// contract (an entity cut in half by a write boundary) can be pinned by test
/// instead of hoped for.
fn scan_chunks(chunks: &[&str], max_bytes: usize) -> Result<Vec<NodeRecord>, String> {
    let records: Rc<RefCell<Vec<NodeRecord>>> = Rc::new(RefCell::new(Vec::new()));
    let stack: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let pending: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    // Spec §9: `<template>` text is NOT extracted — same bucket as
    // script/style. lol_html's lexer delivers template contents as
    // `TextType::Data`, so `wanted_text_type` cannot see the difference; the
    // element handler has to tell the text handler when it is inside one.
    // A depth counter (not a bool) so nested templates close correctly, and
    // the decrement is tied to the same pop-by-name check the label stack
    // uses — an outer end tag that truncates a `<template>` off the stack
    // leaves the depth elevated, which is what browsers do too (the rest of
    // the block stays inside template content).
    let template_depth: Rc<Cell<usize>> = Rc::new(Cell::new(0));

    let records_t = records.clone();
    let stack_t = stack.clone();
    let pending_t = pending.clone();
    let stack_e = stack.clone();
    let template_t = template_depth.clone();
    let template_e = template_depth.clone();

    let settings = rewriter_settings(
        vec![element!("*", move |el| {
            let name = el.tag_name().to_ascii_lowercase();
            // spec §3.2 step 6: never push void/self-closing tags — they get
            // no end tag, and would poison later labels.
            if is_void(&name) || el.is_self_closing() {
                return Ok(());
            }
            let stack_end = stack_e.clone();
            let template_end = template_e.clone();
            let popped = name.clone();
            let on_end: EndTagHandler<'static> = Box::new(move |_end| {
                // Pop by name so mismatched markup cannot corrupt labels of
                // later text nodes.
                let mut s = stack_end.borrow_mut();
                if let Some(pos) = s.iter().rposition(|t| *t == popped) {
                    s.truncate(pos);
                    if popped == "template" {
                        template_end.set(template_end.get().saturating_sub(1));
                    }
                }
                Ok(())
            });
            // Only push once the matching pop is guaranteed to be registered;
            // `end_tag_handlers()` is `None` for elements that cannot have
            // content, and an unpaired push would leak into later labels.
            if let Some(handlers) = el.end_tag_handlers() {
                handlers.push(on_end);
                if name == "template" {
                    template_e.set(template_e.get() + 1);
                }
                stack_e.borrow_mut().push(name);
            }
            Ok(())
        })],
        vec![doc_text!(move |t| {
            if wanted_text_type(t.text_type()) {
                pending_t.borrow_mut().push_str(t.as_str());
                if t.last_in_text_node() {
                    // Entities can straddle chunk boundaries, so decode only
                    // after the whole text node has been coalesced.
                    let raw = std::mem::take(&mut *pending_t.borrow_mut());
                    let decoded = htmlize::unescape(raw.as_str()).into_owned();
                    // The drop decision is made on the DECODED form: an
                    // `&nbsp;`-only node decodes to whitespace and is dropped.
                    // Template content is dropped outright (spec §9).
                    let kept = template_t.get() == 0 && !decoded.chars().all(char::is_whitespace);
                    let label = stack_t
                        .borrow()
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "fragment".to_string());
                    records_t.borrow_mut().push(NodeRecord {
                        decoded,
                        label,
                        kept,
                    });
                }
            }
            Ok(())
        })],
        max_bytes,
    );

    let mut rewriter = HtmlRewriter::new(settings, |_chunk: &[u8]| {});
    for chunk in chunks {
        rewriter
            .write(chunk.as_bytes())
            .map_err(|e| rewriter_err("write", e))?;
    }
    // `end` consumes the rewriter, which drops the handlers and with them the
    // `Rc` clones captured above.
    rewriter.end().map_err(|e| rewriter_err("end", e))?;

    Ok(Rc::try_unwrap(records)
        .map(RefCell::into_inner)
        .unwrap_or_else(|rc| rc.borrow().clone()))
}

/// The kept records of `scan`, split into parallel text/label vectors.
pub fn extract(block: &str) -> Result<HtmlSegments, String> {
    let records = scan(block)?;
    let mut out = HtmlSegments::default();
    for r in records.into_iter().filter(|r| r.kept) {
        out.texts.push(r.decoded);
        out.labels.push(r.label);
    }
    Ok(out)
}

/// Whether a translated segment's interior blank lines survive the splice.
///
/// The knob exists because the *host format* decides, not the markup: a blank
/// line terminates a CommonMark HTML block of type 6/7 at reparse, splitting
/// the block and breaking its anchor. Nothing else about splicing cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlankLinePolicy {
    /// Interior blank lines in a translated segment are collapsed — the
    /// Markdown-host rule for CommonMark HTML block types 6 and 7.
    Collapse,
    /// Blank lines pass through. Correct for CommonMark types 1–5 (`<pre>`,
    /// `<textarea>` and friends, whose content a collapse would damage) and
    /// for every block of an HTML document, where nothing ever Markdown-
    /// reparses the output.
    Keep,
}

impl BlankLinePolicy {
    /// The one home of the CommonMark mapping: 6 | 7 → [`BlankLinePolicy::Collapse`],
    /// everything else → [`BlankLinePolicy::Keep`]. Byte-for-byte the
    /// `matches!(block_type, 6 | 7)` that `splice` computed inline before ti
    /// 490d97 wave 0.
    ///
    /// The domain is CommonMark's 1–7 plus the `0` a caller with no
    /// CommonMark context has; values outside it resolve to `Keep` rather
    /// than panicking, because the safe arm is the one that touches nothing.
    pub fn from_commonmark_html_block_type(block_type: u8) -> Self {
        if matches!(block_type, 6 | 7) {
            BlankLinePolicy::Collapse
        } else {
            BlankLinePolicy::Keep
        }
    }
}

/// Remove interior blank lines (CommonMark definition: a line containing
/// only spaces/tabs). Only [`splice`] under [`BlankLinePolicy::Collapse`] calls this —
/// type-1 blocks (`<pre>`, `<textarea>`) keep their blank lines (spec §3.3).
///
/// A line ends at `\r\n`, a lone `\r`, or a `\n` — CommonMark §2.1's rule,
/// and the one `transync_syntax::parser::ranges::LineOffsets` already counts
/// with, because the reader this protects against is comrak (R0010-0053).
/// Splitting on `\n` alone left the `\r` of a CRLF blank line *inside* the
/// candidate slice, where the spaces/tabs test rejects it, and made a
/// lone-CR segment one single line with no interior at all — so both
/// spellings kept the blank separator that terminates the host's type-6/7
/// html block at reparse and breaks the block's anchor, which is the whole
/// reason this function exists.
///
/// Terminators are re-emitted from the source rather than normalized: this
/// runs on a provider's translated segment, and rewriting its line endings
/// is content editing, not blank-line collapse.
pub(crate) fn collapse_blank_lines(text: &str) -> String {
    // (content, terminator) per line. The final entry's terminator is empty:
    // a text ending in a terminator gets a trailing empty line, exactly the
    // trailing piece `split('\n')` used to produce.
    let bytes = text.as_bytes();
    let mut lines: Vec<(&str, &str)> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < bytes.len() {
        let term = match bytes[i] {
            b'\n' => 1,
            // A `\r` immediately before a `\n` is that one terminator's
            // first byte, never a boundary of its own.
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
            b'\r' => 1,
            _ => 0,
        };
        if term == 0 {
            i += 1;
            continue;
        }
        lines.push((&text[start..i], &text[i..i + term]));
        i += term;
        start = i;
    }
    lines.push((&text[start..], ""));

    let n = lines.len();
    let mut out = String::with_capacity(text.len());
    for (idx, (content, terminator)) in lines.iter().enumerate() {
        let blank = content.chars().all(|c| c == ' ' || c == '\t');
        if blank && idx > 0 && idx + 1 < n {
            continue;
        }
        out.push_str(content);
        out.push_str(terminator);
    }
    out
}

/// Splice translated segments back into the block positionally
/// (spec §3.3). Phase A re-runs `scan` for the node-level plan (kept/
/// dropped + decoded source text); phase B streams the block through the
/// same `rewriter_settings` and the same `wanted_text_type` filter,
/// replacing kept nodes whose translation differs from the decoded source
/// (identity-skip) and passing everything else through byte-verbatim.
pub fn splice(
    block: &str,
    translated: &[String],
    blank_lines: BlankLinePolicy,
) -> Result<String, String> {
    let plan = scan(block)?;
    let kept_count = plan.iter().filter(|r| r.kept).count();
    if kept_count != translated.len() {
        return Err(format!(
            "html segment count mismatch at splice: block has {kept_count} kept segments, got {} translations",
            translated.len()
        ));
    }

    // Node-level actions, in text-node order: None = leave untouched
    // (dropped node OR identity translation); Some(text) = replace.
    //
    // Exhaustive on purpose (ti 490d97 wave 0 Task 3 review): `matches!`
    // would hide a third variant behind an implicit `_ => false` and make it
    // mean Keep by accident. A new policy must stop the compiler here.
    let collapse = match blank_lines {
        BlankLinePolicy::Collapse => true,
        BlankLinePolicy::Keep => false,
    };
    let mut ti = 0usize;
    let actions: Vec<Option<String>> = plan
        .iter()
        .map(|r| {
            if !r.kept {
                return None;
            }
            let t = &translated[ti];
            ti += 1;
            if *t == r.decoded {
                None // identity-skip: source bytes (entities included) pass through
            } else if collapse {
                Some(collapse_blank_lines(t))
            } else {
                Some(t.clone())
            }
        })
        .collect();

    let output: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::with_capacity(block.len())));
    let out_sink = output.clone();

    // Phase-B cursor. It advances on exactly the same signal phase A used —
    // `wanted_text_type` gating plus `last_in_text_node()` — so the two
    // passes cannot disagree on which text node index they are looking at.
    // A borrowed `Cell` (not `Rc<RefCell<_>>`) is enough: the handler's borrow
    // ends when `end()` drops the rewriter, and `Cell::get` only needs `&`, so
    // the final count stays readable for the agreement assertion below.
    let node_index = Cell::new(0usize);
    let cursor = &node_index;
    let mut replaced_current = false;

    let settings = rewriter_settings(
        vec![],
        vec![doc_text!(move |t| {
            if wanted_text_type(t.text_type()) {
                let idx = cursor.get();
                if let Some(Some(replacement)) = actions.get(idx) {
                    if replaced_current {
                        t.remove();
                    } else {
                        // First chunk of a replaced node carries the whole
                        // translation; ContentType::Text escapes < > &
                        // (spec §3.3).
                        t.replace(replacement, ContentType::Text);
                        replaced_current = true;
                    }
                }
                if t.last_in_text_node() {
                    cursor.set(idx + 1);
                    replaced_current = false;
                }
            }
            Ok(())
        })],
        DEFAULT_MAX_MEMORY_BYTES,
    );

    let mut rewriter = HtmlRewriter::new(settings, move |chunk: &[u8]| {
        out_sink.borrow_mut().extend_from_slice(chunk);
    });
    rewriter
        .write(block.as_bytes())
        .map_err(|e| rewriter_err("write", e))?;
    // `end` consumes the rewriter, releasing the output-sink `Rc` clone.
    rewriter.end().map_err(|e| rewriter_err("end", e))?;

    // Pass agreement is the whole basis of positional splicing: if phase B
    // saw a different number of text nodes than phase A, every action after
    // the divergence landed on the wrong node. Unreachable by construction
    // (one Settings source, one text-type filter) — but the release failure
    // mode of an unchecked divergence is SILENT, not loud: `actions.get(idx)`
    // simply answers `None` past the end, so a short phase B drops the tail of
    // the translation and a long one shifts every later replacement onto the
    // wrong node, both with a successful `Ok`. So it is a real error rather
    // than a `debug_assert` (R0009-0066): a refusal here rides the same
    // splice-failure degrade path the count mismatch above already uses, and
    // the block falls back to source instead of shipping mis-spliced text.
    let seen = node_index.get();
    if seen != plan.len() {
        return Err(format!(
            "html splice pass disagreement: phase B saw {seen} text nodes, phase A planned {}",
            plan.len()
        ));
    }

    let bytes = Rc::try_unwrap(output)
        .map(RefCell::into_inner)
        .unwrap_or_else(|rc| rc.borrow().clone());
    String::from_utf8(bytes).map_err(|e| format!("spliced html is not utf-8: {e}"))
}

/// Where inside a start tag's attribute list the scanner is. HTML only treats
/// `/` as a self-closing marker outside an attribute value, so an unquoted
/// value such as `href=https://example.com/` must not be misread as one.
#[derive(Debug, Clone, Copy)]
enum AttrState {
    /// Between attributes, or after a quoted value: `/` here is a marker,
    /// a bare `"` / `'` is an ordinary name byte, and `=` opens a value
    /// only after a consumed attribute name — a stray `=` starts an
    /// attribute NAMED `=` instead (ti 549b20; both measured against a
    /// real browser).
    Outside,
    /// After `=`, before the value starts.
    BeforeValue,
    /// Inside a `"`/`'` delimited value.
    Quoted(u8),
    /// Inside an unquoted value: `/` here is data.
    Unquoted,
}

/// One tag-shaped region [`scan_tags`] recognized, in document order.
///
/// Exhaustive by policy: no `#[non_exhaustive]`, and no in-crate match may
/// use a `_ =>` arm. R0002-0020 and R0003-0066 were both phantom tokens
/// caught because nothing hid behind one.
#[derive(Debug, Clone)]
pub enum TagToken {
    /// A start tag. `span` covers `<` through `>` inclusive-exclusive.
    Open {
        name: String,
        self_closing: bool,
        span: (usize, usize),
    },
    /// An end tag. `span` covers `</` through `>` inclusive-exclusive.
    Close { name: String, span: (usize, usize) },
    /// A region the scanner recognizes and steps over: a comment, a CDATA
    /// section (either terminator mode), a bogus comment — `<!…>` / `<?…>`,
    /// and since ti `e20490` also `</` before a non-letter, which HTML's
    /// end-tag-open state sends to a bogus comment running to the first `>`
    /// (`</>` alone is discarded whole and mints nothing at all) —
    /// a tag left unterminated at EOF (ti 549b20 — a browser abandons a
    /// tag cut off before its `>`, minting no element and no attributes, so
    /// the bytes are a passed-over region, not markup), or — since R0010-0032
    /// / DCR-0050 — a `<plaintext>` start tag in HTML content and everything
    /// after it, which HTML's one exitless tokenizer state makes text.
    /// It carries no name because it has none. What it carries is the byte
    /// range — the thing an intake needs in order to trim the anonymous runs
    /// between elements — plus, since ti `c1f9a8`, the two facts the scanner
    /// knew at the moment it produced the region and nothing downstream can
    /// recover: which KIND of region it is, and whether it was TERMINATED.
    ///
    /// `tag_inventory` still ignores it. `balance_fragment` does not, and that
    /// is the point: a fragment ending inside an unterminated region needs
    /// repairing at the wrapper seam, and deciding where a comment ends is a
    /// question `scan_tags` already answers. Having the balancer re-derive it
    /// from the bytes would be a second opinion about exactly that — the shape
    /// ti `415cdb`, ti `e20490` and ti `2e2453` each were.
    Skip {
        span: (usize, usize),
        kind: SkipKind,
        /// `false` when the region ran to EOF without its terminator. Only the
        /// LAST token of a fragment can be usefully unterminated; an earlier
        /// one means the scanner stopped, which cannot happen.
        terminated: bool,
    },
}

/// Which passed-over region a [`TagToken::Skip`] names.
///
/// The distinction exists so [`balance_fragment`] can repair an unterminated
/// one the way a browser resolves it, without classifying bytes itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipKind {
    /// `<!-- … -->`.
    Comment,
    /// `<![CDATA[ … ]]>` in foreign content, where it really is a CDATA
    /// section. In HTML content the same bytes open a bogus comment, and the
    /// scanner reports that as [`Self::BogusComment`] — the terminator differs,
    /// so the kinds must.
    CdataSection,
    /// `<!…>`, `<?…>`, or `</` before a non-letter: HTML's bogus-comment
    /// state, running to the first `>`.
    BogusComment,
    /// A tag cut off before its `>`. Never terminated by construction — a
    /// terminated tag is an `Open` or a `Close`.
    UnterminatedTag,
    /// A `<plaintext>` start tag in HTML content **and every byte after it**
    /// (R0010-0032 / DCR-0050). Never terminated by construction, and unlike
    /// the other three that is not an accident of where the fragment stopped:
    /// HTML's PLAINTEXT state has **no exit** — no end tag, no character
    /// sequence, nothing but EOF ends it — so the region runs to the end of
    /// the fragment by definition and is always the last token.
    ///
    /// The span deliberately covers the start tag as well as the text. The
    /// element is real in a browser, but keeping it and dropping only its
    /// contents would leave the tag in the pane, still switching the
    /// tokenizer, still swallowing the sync wrapper's own `</div>`; and
    /// dropping only the tag would hand the bytes after it back to HTML's
    /// tokenizer as markup, after [`strip_reserved_sync_attrs`] had already
    /// passed over them as the text a browser makes of them — the
    /// strip-then-balance bypass OI-0046 was, one region larger. One region,
    /// one repair.
    PlainText,
}

impl SkipKind {
    /// The bytes that would close this region, or `None` when closing it is
    /// not what a browser does.
    ///
    /// Two kinds answer `None`, for two different reasons, and both land on
    /// the same repair — deletion, the act the balancer already performs on an
    /// orphan close tag:
    ///
    /// * `UnterminatedTag`, where "terminate it" would be the wrong repair
    ///   twice over. A browser at EOF-inside-a-tag **abandons** the tag — it
    ///   mints no element and no attributes — so appending `>` would invent
    ///   structure rather than recover it. And no single terminator even
    ///   works: a tag cut inside a quoted value (`<div class="x`) needs the
    ///   quote closed first, and guessing that is parsing.
    /// * `PlainText`, where there is no terminator to append at all. HTML's
    ///   PLAINTEXT state is the one tokenizer state with no exit, so a browser
    ///   does not *abandon* this region — it never leaves it. That is a third
    ///   case for ti `c1f9a8`'s "terminated where a browser terminates it,
    ///   deleted where a browser abandons it", and DCR-0050 records the
    ///   decision rather than letting the code invent a rule: a region a
    ///   browser will not leave cannot be embedded in a pane at all, so it is
    ///   deleted, and the alternatives (escaping the tail, keeping the tag)
    ///   are argued and rejected there.
    pub fn terminator(self) -> Option<&'static str> {
        match self {
            Self::Comment => Some("-->"),
            Self::CdataSection => Some("]]>"),
            Self::BogusComment => Some(">"),
            Self::UnterminatedTag | Self::PlainText => None,
        }
    }
}

/// Where HTML's **tag-name state** ends: it consumes every byte that is not
/// whitespace, `/` or `>` into the element name — quotes and `=` included.
///
/// ONE definition, deliberately. `scan_tags` decides where a tag's name stops
/// and `walk_attrs` decides where its attributes start, and those are the same
/// boundary: when they disagreed, `<divq"x=" data-sync-id="v">` ended its name
/// at `divq` for the strip while a browser read the whole `divq"x="`, so the
/// reserved attribute landed inside a phantom quoted value the strip never
/// examined as a name and a live anchor survived (ti `e20490`). Fixing only
/// the tokenizer left that hole open, because the strip does not go through
/// it.
fn tag_name_end(bytes: &[u8], first_name_byte: usize, limit: usize) -> usize {
    let mut j = first_name_byte;
    while j < limit && !bytes[j].is_ascii_whitespace() && bytes[j] != b'/' && bytes[j] != b'>' {
        j += 1;
    }
    j
}

/// Where the comment that opened at `open` (the `<` of its `<!--`) ends —
/// one past its closing bytes — or `None` when the fragment runs out first.
///
/// ONE definition, for the same reason [`tag_name_end`] is one: "where does a
/// comment end" is a question [`scan_tags`] answers and [`balance_fragment`]
/// acts on, and a second reader of these bytes would be the defect shape ti
/// `415cdb`, ti `e20490` and ti `2e2453` each were.
///
/// **Two closing forms, not one (R0010-0049 / DCR-0050).** HTML's comment-end
/// state closes on `>` — the familiar `-->` — and on `!` it goes to
/// comment-end-BANG, where a `>` also closes the comment (an
/// incorrectly-closed-comment parse error; the token is still emitted). So
/// `--!>` closes a comment in every browser, and reading only `-->` made
/// `<!-- a --!>` + `<div>y` + `<!-- b -->` — ONE CommonMark type-2 html block
/// — scan as a single terminated comment: the `<div>` was never seen, the
/// balancer owed nothing, and the still-open `div` swallowed the sync
/// wrapper's own `</div>` in the pane (`contracts.md` §4a, invariant 7).
///
/// The two searches start at different offsets, and the asymmetry is HTML's:
///
/// * `-->` is searched from the `<` itself, so the opener's own dashes can
///   supply it. That is what models the ABRUPT-CLOSING rules without a state
///   machine — comment-start and comment-start-dash both close on `>`, so
///   `<!-->` and `<!--->` are complete empty comments, and searching from the
///   `<` lands on exactly their last three bytes.
/// * `--!>` is searched from after the opener, because comment-end-bang is
///   reachable only THROUGH comment-end, and the opener's `--` never enters
///   it. `<!--!>` is therefore not a closed comment (comment-start reconsumes
///   the `!` as data), and a search from the `<` would have claimed it was.
fn comment_end(html: &str, open: usize) -> Option<usize> {
    const OPENER: usize = "<!--".len();
    let dashes = html[open..].find("-->").map(|p| open + p);
    let bang = html[open + OPENER..]
        .find("--!>")
        .map(|p| open + OPENER + p);
    match (dashes, bang) {
        (Some(d), Some(b)) if b < d => Some(b + "--!>".len()),
        (Some(d), _) => Some(d + "-->".len()),
        (None, Some(b)) => Some(b + "--!>".len()),
        (None, None) => None,
    }
}

/// One token from [`scan_tags_with_state`], together with the tree state the
/// scanner had to keep in order to produce it.
///
/// HTML's tokenizer is not standalone: the tree construction stage feeds back
/// into it, and *which content mode the adjusted current node is in* is the
/// piece of feedback this crate cannot do without — it decides whether
/// `<script>` opens a raw-text run at all (ti `2e2453`) and whether a
/// self-closing `/` is honoured. So the scanner, not the walk, owns the
/// content-mode stack, and [`walk_elements`] reads its verdict off these
/// fields rather than recomputing one. A second opinion about where foreign
/// content begins and ends is the same defect shape as ti `415cdb` and ti
/// `e20490`, one region larger.
struct ScannedTag {
    token: TagToken,
    /// How many frames came off THE stack **before** this token was processed:
    /// HTML's implied end tags at a start tag, and the pops that a breakout
    /// start tag — or a breakout `</p>` / `</br>` — performs to leave foreign
    /// content. [`walk_elements`] records each of them as implicitly closed at
    /// this token's own start offset, exactly as it used to record the pops it
    /// computed for itself.
    pre_pops: usize,
    /// What processing the token then did to the stack.
    effect: StackEffect,
}

/// The one mutation a token makes to THE open-element stack, after
/// [`ScannedTag::pre_pops`] have been applied.
///
/// This enum is why there is only one stack discipline in this crate.
/// `walk_elements` used to re-derive "did this close something, and what"
/// from the token stream with its own `rposition` search, which is how the
/// two ended up popping differently from each other AND from a browser
/// (ti `9b4d66`, `307283`, `895fb7`, `da6bb5`, `bb961a`). The scanner has to
/// reach this verdict anyway — it decides the content mode the next byte is
/// tokenized in — so the walk reads it here rather than voting a second time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StackEffect {
    /// A start tag that inserted an element.
    Push,
    /// An end tag that closed `n` frames. The DEEPEST of the `n` is the
    /// element this tag closed; the rest were closed implicitly by it.
    Close(usize),
    /// The token moved nothing and its bytes STAY. Two shapes reach here: a
    /// `Skip` region, which is not structure at all, and an end tag a browser
    /// turns into content rather than into a pop — `</p>` mints an empty
    /// paragraph and `</br>` mints a `<br>`, so deleting either silently edits
    /// what the reader sees.
    Inert,
    /// The token moved nothing and its bytes must GO: an end tag whose search
    /// ran out of frames on THIS fragment's stack. In a mounted pane the
    /// search would continue into the sync wrapper's own `<div>`, so leaving
    /// the bytes is how `contracts.md` §4a's direct-child rule breaks.
    Orphan,
}

impl ScannedTag {
    /// A token that opens nothing and closes nothing: a skipped region.
    fn inert(token: TagToken) -> Self {
        ScannedTag {
            token,
            pre_pops: 0,
            effect: StackEffect::Inert,
        }
    }
}

/// The content mode a tag lands in, given the open-element stack.
fn current_mode(stack: &[Frame]) -> ContentMode {
    stack.last().map_or(ContentMode::Html, |f| f.child)
}

/// Minimal tag tokenizer for balancing: understands comments, CDATA
/// sections, the raw-text and RCDATA states every [`is_raw_text`] element
/// opens **in HTML content**, and quoted attribute values. NOT a general HTML
/// parser — it is one self-consistent opinion about HTML tokenization, shared
/// by the render-path balancer (spec 2026-08-03 §3.4), the layer-3 inventory
/// check, and the HTML intake to come.
pub fn scan_tags(html: &str) -> Vec<TagToken> {
    scan_tags_with_state(html)
        .into_iter()
        .map(|t| t.token)
        .collect()
}

/// [`scan_tags`] with the scanner's tree state still attached — the form
/// [`walk_elements`] consumes.
fn scan_tags_with_state(html: &str) -> Vec<ScannedTag> {
    let bytes = html.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    // Inside a raw-text / RCDATA element: only its own close tag ends it.
    let mut raw_until: Option<String> = None;
    // THE open-element stack. Not "the scanner's" — there is one, and
    // [`walk_elements`] replays the mutations reported on each [`ScannedTag`]
    // instead of keeping a second one.
    //
    // Each frame carries both content modes because the two answer different
    // questions and HTML asks both. `child` decides how the NEXT byte is
    // tokenized (raw text, a CDATA terminator, whether a `/` is honoured) and
    // is what [`current_mode`] reports; `own` is the element's own namespace,
    // which decides whether an end tag is dispatched to foreign content at
    // all, and whether this frame is in HTML's **special** category or
    // terminates a scope search. A single mode per frame cannot express both:
    // `<svg><desc>` has `own == Svg` and `child == Html`, and reading the
    // wrong one of those is ti `bb961a`.
    //
    // Until DCR-0051 this stack applied NO implied end tags, on the recorded
    // grounds that an extra frame "cannot change the answer" because its child
    // mode equals its parent's. The reasoning was about the extra frame's own
    // mode and never about `truncate` popping the frames ABOVE it — and that
    // is what it cost: `<p><ul><svg></p>` left a stale `p` at the bottom, the
    // close tag found it, and the `<svg>` came off with it (ti `9b4d66`).
    let mut open_elements: Vec<Frame> = Vec::new();

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if let Some(close_name) = &raw_until {
            // Only the matching close tag ends raw-text state. The probe
            // compares BYTES: `html[i..][..want.len()]` would panic when a
            // multibyte char straddles the window. HTML also requires a
            // delimiter right after the name, otherwise `</scripty>` would
            // end a `<script>` body (and get deleted as an orphan close).
            let want = format!("</{close_name}");
            let rest = &bytes[i..];
            let delimited = matches!(
                bytes.get(i + want.len()),
                Some(c) if c.is_ascii_whitespace() || *c == b'/' || *c == b'>'
            );
            if delimited
                && rest.len() >= want.len()
                && rest[..want.len()].eq_ignore_ascii_case(want.as_bytes())
            {
                raw_until = None; // fall through: tokenize the close tag
            } else {
                i += 1;
                continue;
            }
        }
        if html[i..].starts_with("<!--") {
            // Both of HTML's closing forms, decided in one place — see
            // [`comment_end`] for why `--!>` is one of them and why the two
            // searches begin at different offsets.
            let closer = comment_end(html, i);
            let end = closer.unwrap_or(html.len());
            tokens.push(ScannedTag::inert(TagToken::Skip {
                span: (i, end),
                kind: SkipKind::Comment,
                terminated: closer.is_some(),
            }));
            i = end;
            continue;
        }
        // R0003-0067: `<![CDATA[ … ]]>`. A browser never tokenizes what is
        // inside one as markup — in foreign content it is character data
        // that ends at `]]>`, and in HTML content the bytes open a *bogus
        // comment* that ends at the first `>`. The scanner had neither
        // state, so `<b>` inside an `<svg>`'s CDATA became a real open tag:
        // `balance_fragment` appended an invented `</b>` to the pane, and
        // `tag_inventory` carried a phantom the escaped translation could
        // not match. The terminator differs by context, so the depth
        // decides which one ends the skip.
        if html[i..].starts_with("<![CDATA[") {
            let terminator = if current_mode(&open_elements) != ContentMode::Html {
                "]]>"
            } else {
                ">"
            };
            let closer = html[i..].find(terminator);
            let end = closer
                .map(|p| i + p + terminator.len())
                .unwrap_or(html.len());
            // The kind follows the terminator, not the opening bytes: in HTML
            // content `<![CDATA[` opens a BOGUS COMMENT ending at the first
            // `>`, and calling it a CDATA section here would hand the balancer
            // the wrong repair.
            let kind = if terminator == "]]>" {
                SkipKind::CdataSection
            } else {
                SkipKind::BogusComment
            };
            tokens.push(ScannedTag::inert(TagToken::Skip {
                span: (i, end),
                kind,
                terminated: closer.is_some(),
            }));
            i = end;
            continue;
        }
        // HTML's bogus-comment state. `<!` that is neither a comment nor a
        // CDATA section, and `<?`, run to the FIRST `>` and are not markup —
        // the same rule R0003-0066 and R0003-0067 applied one region further
        // in. Before wave 0 the scanner had no such state: it stepped past
        // `<!` a byte at a time, so `<!doctype html>` produced no token at
        // all (the `!` fails the tag-open state's ASCII-letter test below)
        // and tag-shaped bytes inside a bogus comment tokenized as markup.
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            let closer = html[i..].find('>');
            let end = closer.map(|p| i + p + 1).unwrap_or(html.len());
            tokens.push(ScannedTag::inert(TagToken::Skip {
                span: (i, end),
                kind: SkipKind::BogusComment,
                terminated: closer.is_some(),
            }));
            i = end;
            continue;
        }
        let start = i;
        let mut j = i + 1;
        let closing = j < bytes.len() && bytes[j] == b'/';
        if closing {
            j += 1;
        }
        let name_start = j;
        // R0003-0066: HTML's tag-open state admits an ASCII LETTER and
        // nothing else, so a name may not START with a digit. Accepting one
        // tokenized ordinary prose — `<1>`, `<2026 rows>` — as markup, and
        // the invention was expensive twice over, exactly as it was for
        // RCDATA in R0002-0020: `balance_fragment` appended a `</1>` a
        // browser reads as junk, and `tag_inventory` reported a phantom
        // `1` for the SOURCE that the spliced translation (whose replaced
        // text is escaped) never reproduces — so layer 3 called an engine
        // fault and dropped a perfectly good translation to fallback_source.
        //
        // R0002-0060: `:` belongs to the NAME, not to the attribute list.
        // Stopping at it read `<div:x>` as a `div`, and the balancer then
        // appended a `</div>` that pops the sync wrapper's own `<div>` at
        // parse time — the block's tail spills outside its anchor, which is
        // precisely what balancing exists to prevent. A leading `:` is still
        // not a name, which the letter-only first byte now says directly
        // (`<:x>` stays plain text via the `j == name_start` check below).
        // ti `e20490`: HTML's TAG NAME state consumes every byte that is not
        // whitespace, `/` or `>` into the element name — quotes and `=`
        // included — so `<divq"x=" data-sync-id="v">` is an element named
        // `divq"x="` carrying a real `data-sync-id`. Stopping the name at the
        // first byte outside `[A-Za-z0-9:-]` attribute-walked the remainder
        // instead, which buried that plant inside a phantom quoted value
        // `collect_reserved_attr_spans` never examines as a name — so the
        // strip left a live anchor standing. The `:` rule above is subsumed
        // by this loop rather than special-cased; the leading-letter test is
        // what still keeps `<1>` and `<:x>` out of markup.
        if bytes.get(name_start).is_some_and(u8::is_ascii_alphabetic) {
            j = tag_name_end(bytes, name_start, bytes.len());
        }
        if j == name_start {
            if closing {
                // HTML's END-TAG-OPEN state: `</` before anything that is not
                // an ASCII letter is a parse error. `</>` is discarded whole
                // (missing-end-tag-name); anything else opens a BOGUS COMMENT
                // running to the first `>`, or to EOF if there is none. Either
                // way a browser mints no element and no attributes, so the
                // plain-text fall-through here invented structure: the scanner
                // emitted an `Open` for `</1 <div>x` and the balancer then
                // owed it a `</div>` a browser reads as orphan junk
                // (R0003-0066 / R0002-0020's class, ti `e20490`).
                //
                // Reserved-name-shaped bytes inside such a region cannot
                // become live attributes, so this direction is fail-safe: the
                // cost was invented structure, never a leaked anchor.
                let mut k = j;
                while k < bytes.len() && bytes[k] != b'>' {
                    k += 1;
                }
                let end = (k + 1).min(bytes.len());
                if bytes.get(j) != Some(&b'>') {
                    tokens.push(ScannedTag::inert(TagToken::Skip {
                        span: (start, end),
                        kind: SkipKind::BogusComment,
                        terminated: k < bytes.len(),
                    }));
                }
                i = end;
                continue;
            }
            i += 1; // "<" not followed by a tag name — plain text
            continue;
        }
        let mut name = html[name_start..j].to_ascii_lowercase();
        // Scan to the closing '>' through HTML's attribute states. `/` sets
        // the self-closing flag only in `Outside` state and only if `>`
        // follows immediately; anything else clears it again.
        let mut attr = AttrState::Outside;
        let mut self_closing = false;
        // ti 549b20: has the current attribute consumed a NAME byte since
        // the last boundary (tag name, `/`, or a completed value)? HTML
        // opens a value on `=` only from the attribute-name /
        // after-attribute-name states; a stray `=` in before-attribute-name
        // STARTS an attribute named `=` instead, and a quote after it joins
        // that name. This bit tells the two apart — whitespace does not
        // reset it (after-attribute-name), a completed value or a `/` does
        // (after-attribute-value-quoted and self-closing-start-tag both
        // reconsume in before-attribute-name).
        let mut has_attr_name = false;
        while j < bytes.len() {
            let c = bytes[j];
            match attr {
                AttrState::Outside => match c {
                    b'>' => break,
                    // ti 549b20: a bare quote is a parse error that joins
                    // the attribute NAME in a browser — never a value
                    // opener. Before the fix it opened a phantom Quoted
                    // state, and one stray quote ran the scan off EOF and
                    // hid everything after it from every consumer.
                    b'"' | b'\'' => {
                        self_closing = false;
                        has_attr_name = true;
                    }
                    b'=' => {
                        self_closing = false;
                        if has_attr_name {
                            // attribute-name / after-attribute-name: `=`
                            // ends the name and opens the value. The
                            // ordinary `name=value` shape lands here.
                            attr = AttrState::BeforeValue;
                            has_attr_name = false;
                        } else {
                            // before-attribute-name: `=` is a parse error
                            // that STARTS an attribute whose name is `=`
                            // (measured in headless Chromium: `<div =">`
                            // is one attribute named `="` and the tag ends
                            // at the first `>`). No value state — the
                            // quote after it joins the NAME.
                            has_attr_name = true;
                        }
                    }
                    b'/' => {
                        // self-closing-start-tag: anything but `>`
                        // reconsumes in before-attribute-name, so the
                        // name track resets with it.
                        self_closing = true;
                        has_attr_name = false;
                    }
                    _ if c.is_ascii_whitespace() => self_closing = false,
                    _ => {
                        self_closing = false;
                        has_attr_name = true;
                    }
                },
                AttrState::BeforeValue => match c {
                    b'>' => break,
                    b'"' | b'\'' => attr = AttrState::Quoted(c),
                    _ if c.is_ascii_whitespace() => {}
                    // Anything else — `/` included — begins an unquoted value.
                    _ => attr = AttrState::Unquoted,
                },
                AttrState::Quoted(q) => {
                    if c == q {
                        attr = AttrState::Outside;
                    }
                }
                AttrState::Unquoted => match c {
                    b'>' => break,
                    _ if c.is_ascii_whitespace() => attr = AttrState::Outside,
                    _ => {}
                },
            }
            j += 1;
        }
        if j >= bytes.len() {
            // ti 549b20: an unterminated tag runs to EOF — there are no
            // bytes past it by definition — but the old bare `break` left
            // the DOCUMENT loop with no token, so a caller could not tell
            // a passed-over region from a scanned one. Name it instead: a
            // browser abandons a tag truncated at EOF (no element, no
            // attributes), so `Skip` — recognized and stepped over, not
            // markup — is exactly what it is.
            tokens.push(ScannedTag::inert(TagToken::Skip {
                span: (start, bytes.len()),
                kind: SkipKind::UnterminatedTag,
                // Unterminated by construction: this arm is only reached
                // when the scan ran off the end looking for `>`.
                terminated: false,
            }));
            break;
        }
        let span = (start, j + 1);
        if closing {
            // HTML's end-tag handling, in the ONE place that keeps the stack —
            // see [`end_tag_effect`] for which of its rules are modelled and
            // which are not. This used to be a bare `rposition` + `truncate`
            // whose comment called it "the same pop HTML's 'any other end tag'
            // performs". It was not: HTML's version stops at the first SPECIAL
            // element, its block-level end tags ask about SCOPE first, and
            // `</p>` and `</br>` leave foreign content before either applies.
            // Each of those three is a ticket in this family.
            let effect = end_tag_effect(&open_elements, &name);
            for _ in 0..(effect.breakout + effect.close) {
                open_elements.pop();
            }
            tokens.push(ScannedTag {
                token: TagToken::Close { name, span },
                pre_pops: effect.breakout,
                effect: if effect.close > 0 {
                    StackEffect::Close(effect.close)
                } else if effect.delete {
                    StackEffect::Orphan
                } else {
                    StackEffect::Inert
                },
            });
        } else {
            // How many frames this token takes off the stack before it is
            // processed at all: the foreign-content breakout, then HTML's
            // implied end tags. Both are recorded on the token so the walk can
            // replay them; neither is re-derived there.
            let mut pre_pops = 0usize;

            // Which rules the token is dispatched to. Normally the current
            // node's child mode — but HTML's tree-construction dispatcher has
            // two start-tag exceptions inside foreign content, and both are
            // measured divergences rather than pedantry:
            //
            // * `mglyph` and `malignmark` are the two names a MathML text
            //   integration point does NOT hand to the HTML rules, so they
            //   stay MathML and a `<script>` beneath one is a foreign element
            //   rather than a raw-text run (ti `a7e625`).
            // * an `<svg>` start tag inside a MathML `annotation-xml` IS handed
            //   to the HTML rules, whatever the element's `encoding`, so it
            //   opens a real SVG element.
            let mut parent_mode = match open_elements.last() {
                Some(f)
                    if f.own == ContentMode::MathMl
                        && is_mathml_text_integration_point(&f.name)
                        && (name == "mglyph" || name == "malignmark") =>
                {
                    ContentMode::MathMl
                }
                Some(f)
                    if f.own == ContentMode::MathMl
                        && f.name.eq_ignore_ascii_case("annotation-xml")
                        && name == "svg" =>
                {
                    ContentMode::Html
                }
                _ => current_mode(&open_elements),
            };

            // A breakout start tag tears the parser out of foreign content
            // BEFORE it is processed, so the mode this tag is read in — and
            // therefore everything below — is the one it lands in after the
            // pop, not the one it was written inside (DCR-0041).
            if parent_mode != ContentMode::Html && breaks_out_of_foreign(&name, html, span) {
                while open_elements
                    .last()
                    .is_some_and(|f| f.child != ContentMode::Html)
                {
                    open_elements.pop();
                    pre_pops += 1;
                }
                parent_mode = current_mode(&open_elements);
            }

            // ti `e923ef`: HTML's one tag-name substitution, and it is an
            // HTML-CONTENT rule like the three before it. The "in body"
            // insertion mode handles a start tag named `image` by rewriting the
            // token's name to `img` and reprocessing it, so `<image>x` is an
            // empty `img` with `x` as its SIBLING — `image` is not a void
            // element, `img` is, and the voidness follows the rename rather
            // than the name the author typed.
            //
            // Doing it here rather than in the walk is what keeps one rule in
            // one place: `is_void`, the mode stack and the strip all read the
            // renamed token, so none of them needs its own opinion about
            // `image`. The alternative — teaching the walk that `image` is void
            // — would have been a second opinion about voidness, which is the
            // shape ti `415cdb`, ti `e20490` and ti `2e2453` each were.
            //
            // Foreign content is excluded, and that is not a carve-out but the
            // same spec rule: "any other start tag" there inserts a foreign
            // element, and SVG has a real `<image>`. So `<svg><image>a</image>`
            // keeps the author's name and its closer, exactly as ti `48f3c6`
            // left voidness and ti `2e2453` left raw text.
            //
            // END tags are deliberately untouched. The substitution is defined
            // on start tags only; a browser does not rename `</image>`, and it
            // matches no open element either way.
            if parent_mode == ContentMode::Html && name == "image" {
                name = "img".to_string();
            }

            // R0010-0032 / DCR-0050: HTML's PLAINTEXT state, the one state a
            // tokenizer never leaves. "A start tag whose tag name is
            // `plaintext`" (in body) inserts the element and switches the
            // tokenizer to PLAINTEXT, and no end tag, character sequence or
            // insertion mode brings it back — only EOF. So every byte from
            // this tag onward is TEXT in a browser, and the scanner said
            // markup: `<div><plaintext></div>` walked as div → plaintext →
            // both closed at `</div>` and passed through unchanged, while a
            // browser turned the sync wrapper's own `</div>` and every
            // following block into text (`contracts.md` §4a, invariant 7).
            //
            // The whole rest of the fragment, this start tag included, is one
            // `Skip` and the scan stops — see [`SkipKind::PlainText`] for why
            // the tag is inside the span rather than emitted as an `Open`
            // beside it, and DCR-0050 for why the balancer deletes the region
            // instead of terminating it.
            //
            // An HTML-CONTENT rule, like voidness (ti `48f3c6`), raw text (ti
            // `2e2453`) and the `image` rename (ti `e923ef`) above it: the
            // switch is a tree-construction rule of the "in body" insertion
            // mode, and foreign content has none, so `<svg><plaintext>` is an
            // ordinary foreign element. `plaintext` is not a breakout tag, so
            // nothing pulls it out of `<svg>` first.
            if parent_mode == ContentMode::Html && name == "plaintext" {
                tokens.push(ScannedTag::inert(TagToken::Skip {
                    span: (start, bytes.len()),
                    kind: SkipKind::PlainText,
                    // Unterminated by construction: HTML has no bytes that
                    // end this state.
                    terminated: false,
                }));
                break;
            }

            // ti `da6bb5`, the OVER-open direction. In the "in body" insertion
            // mode a `caption` / `col` / `colgroup` / `frame` / `tbody` / `td`
            // / `tfoot` / `th` / `thead` / `tr` start tag is a parse error the
            // parser IGNORES outright — no element, no frame. Pushing one made
            // `balance_fragment` append a `</td>` or `</tr>` to a fragment
            // HTML needs none for, which is inventing structure in a pane:
            // exactly the direction [`implicitly_closes`]' own docstring says
            // this walk must not take. Inside a real table these are a
            // different insertion mode's tags and open normally, which is what
            // the table-scope test asks.
            //
            // `head`, `body`, `html` and `frameset` are on the same spec list
            // and are DELIBERATELY not here — see the residual note on
            // [`start_tag_is_ignored`].
            if parent_mode == ContentMode::Html && start_tag_is_ignored(&open_elements, &name) {
                tokens.push(ScannedTag {
                    token: TagToken::Open {
                        name,
                        self_closing,
                        span,
                    },
                    pre_pops,
                    effect: StackEffect::Inert,
                });
                i = j + 1;
                continue;
            }

            // HTML's implied end tags, applied HERE rather than in the walk
            // (ti `9b4d66`) and by SCOPE rather than at the top of the stack
            // (ti `da6bb5`). Foreign content has no such rule — "any other
            // start tag" simply inserts a foreign element — so the gate is the
            // same one raw text, voidness and the `image` rename already carry.
            if parent_mode == ContentMode::Html {
                let popped = implied_start_tag_pops(&open_elements, &name);
                for _ in 0..popped {
                    open_elements.pop();
                }
                pre_pops += popped;
            }

            // A `/` on a raw-text/RCDATA start tag is ignored by real HTML
            // parsers, so the state starts either way; scanning that content
            // for tags would invent tokens out of `a < b`.
            //
            // R0002-0020: EVERY `is_raw_text` element, not just script and
            // style. `<textarea>` and `<title>` hold RCDATA, which a browser
            // never tokenizes as markup — so scanning them invented tags out
            // of user-visible text, and the invention was expensive twice
            // over: `tag_inventory` reported a phantom `/p` for the source
            // but not for the (correctly escaped) spliced translation, so
            // layer 3 declared an engine fault and dropped a perfectly good
            // translation back to source; and on the live render path the
            // same phantom was DELETED from the pane as an orphan close tag,
            // silently editing what the reader sees.
            //
            // ti `2e2453`: "every" means every one in HTML CONTENT. Raw text
            // and RCDATA are entered by the tree construction stage, from the
            // "in body" insertion mode; foreign content has no such rule, so
            // inside `<svg>`/`<math>` these four are ordinary foreign
            // elements whose contents ARE markup. R0002-0020's motivation is
            // untouched by the qualifier — it was about the tokenizer
            // disagreeing with the balancer over user-visible RCDATA text,
            // all of it in HTML content — and the fix is narrower than the
            // rule it refines, not a revert of it.
            if parent_mode == ContentMode::Html && is_raw_text(&name) {
                raw_until = Some(name.clone());
            }

            // HTML honours the self-closing flag in exactly two places:
            // inside foreign content, and on the `<svg>`/`<math>` start tags
            // that enter it. This condition used to carry a `!is_raw_text`
            // term as well, because the scanner entered raw text inside
            // foreign content and an unhonoured flag was what kept the
            // appended closer visible to the re-scan (DCR-0016 Part D). It
            // was never load-bearing in HTML content — no raw-text name is a
            // foreign root, so the term could not fire there — and inside
            // foreign content it was the bug: `<svg><script/>x` left `x` a
            // sibling in a browser and swallowed it here.
            let honours_flag = parent_mode != ContentMode::Html || is_foreign_root(&name);
            // ti `48f3c6`: voidness is an HTML-content rule too. Foreign
            // content has no void elements — "any other start tag" inserts a
            // foreign element and only the self-closing flag pops it — so
            // `<svg><link>a</link>` is a genuine closable SVG element whose
            // end tag is a real closer, not an orphan the balancer deletes.
            //
            // The hazard that kept `is_void` global was `</br>`: HTML's
            // end-tag-`br` rule turns an appended one back into a fresh
            // `<br>`, so the balancer would MINT structure. It cannot reach
            // here. `br` is in `FOREIGN_BREAKOUT_TAGS`, so `<svg><br>` tears
            // out of foreign content before this line runs and lands in HTML
            // content with `parent_mode == Html`. Same for the other four
            // void names that are also breakout tags: `embed`, `hr`, `img`,
            // `meta`. DCR-0041's breakout model is what retired the trade.
            let void_here = parent_mode == ContentMode::Html && is_void(&name);
            let opens_element = !void_here && !(self_closing && honours_flag);
            let own_mode = own_content_mode(parent_mode, &name);
            let child_mode = child_content_mode(own_mode, &name, html, span);
            if opens_element {
                open_elements.push(Frame {
                    name: name.clone(),
                    own: own_mode,
                    child: child_mode,
                });
            }
            tokens.push(ScannedTag {
                token: TagToken::Open {
                    name,
                    self_closing,
                    span,
                },
                pre_pops,
                effect: if opens_element {
                    StackEffect::Push
                } else {
                    StackEffect::Inert
                },
            });
        }
        i = j + 1;
    }
    tokens
}

/// Lowercase tag-name sequence (`div`, `/div`, …) in document order — the
/// layer-3 belt-and-braces check that a splice never changed the markup
/// skeleton (spec §4.2 post-splice structural check).
pub fn tag_inventory(html: &str) -> Vec<String> {
    scan_tags(html)
        .into_iter()
        .filter_map(|t| match t {
            TagToken::Open { name, .. } => Some(name),
            TagToken::Close { name, .. } => Some(format!("/{name}")),
            TagToken::Skip { .. } => None,
        })
        .collect()
}

/// Elements a start tag closes *implicitly* — HTML's optional-end-tag rules
/// (R0002-0061), as much of them as a stack-only balancer can honestly keep.
///
/// A returned name is popped from the open stack only while it sits at the
/// TOP: the real algorithm walks down through non-special elements, and
/// guessing at that from a flat stack would close more than HTML does. The
/// bound is deliberate — over-closing invents structure, under-closing costs
/// at most the phantom this table exists to remove.
///
/// `p`'s closers are the block-level start tags that close an open paragraph
/// in the "in body" insertion mode; the rest are the classic pairs (list
/// items, definition lists, table rows and cells, select options, ruby
/// annotations).
pub fn implicitly_closes(name: &str) -> &'static [&'static str] {
    const P: &[&str] = &["p"];
    match name {
        "li" => &["li", "p"],
        "dd" | "dt" => &["dd", "dt", "p"],
        "tr" => &["td", "th", "tr"],
        "td" | "th" => &["td", "th"],
        "tbody" | "tfoot" | "thead" => &["td", "th", "tr", "tbody", "tfoot", "thead"],
        "option" => &["option"],
        "optgroup" => &["option", "optgroup"],
        "rp" | "rt" => &["rp", "rt"],
        // The "closes an open `<p>`" set. `hr` is here AND void: it closes a
        // paragraph without ever being pushed itself.
        "address" | "article" | "aside" | "blockquote" | "center" | "details" | "dialog"
        | "dir" | "div" | "dl" | "fieldset" | "figcaption" | "figure" | "footer" | "form"
        | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" | "hgroup" | "hr" | "listing"
        | "main" | "menu" | "nav" | "ol" | "p" | "pre" | "search" | "section" | "summary"
        | "table" | "ul" | "xmp" => P,
        _ => &[],
    }
}

/// One element the [`element_extents`] walk found, in source order by its
/// open tag.
///
/// **Not every tag mints one.** A void element (`img`, `br`, `hr`, …) in HTML
/// content is never pushed onto the walk's stack, so it produces **no**
/// `ElementExtent` at all — it has no content and nothing to close. Inside
/// foreign content it does mint one, because there are no void elements there
/// (ti `48f3c6`); the five void names that are also breakout tags never reach
/// that case, since they leave foreign content before they are processed. Neither does a self-closing tag
/// in the two places HTML actually honours that flag: inside foreign content,
/// and on the `<svg>` / `<math>` start tags that enter it. "Inside foreign
/// content" is narrower than "inside an `<svg>`" — SVG's `foreignObject` and
/// `desc` and MathML's text integration points put their children back in HTML
/// content, and a breakout start tag leaves foreign content altogether
/// (DCR-0041). A consumer that
/// needs "the element around these bytes" must handle the empty case rather
/// than assuming one extent per tag. (ti 490d97 wave 0 Task 5 review; wave 6's
/// pane derivation depends on it — a block whose only element is a lone
/// `<img>` has zero extents, which is why it takes the transparent wrapper
/// rather than self-injection.)
///
/// **A self-closing spelling of an ordinary HTML tag DOES mint one** (ti
/// 490d97 wave 1). Outside those two carve-outs the `/` is a parse error the
/// parser ignores and the element opens, so `<div/>y</div>` is one `div`
/// extent whose `close` is the author's own end tag. This paragraph used to
/// say "and a self-closing tag outside raw-text/RCDATA", which read the flag
/// the way XML means it: the walk never pushed such a tag, the author's end
/// tag matched nothing on the stack, and [`balance_fragment`] deleted it as
/// an orphan.
///
/// **A truncated tag is not an element either.** A tag with no closing `>`
/// before EOF is a [`TagToken::Skip`], not an `Open` (ti 549b20): it mints
/// no extent, exactly as a browser abandons a tag cut off at EOF. An
/// element whose open tag is complete but whose end tag never arrives is
/// different — it still gets an extent with `close: None` and
/// `content_end == html.len()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementExtent {
    /// Lowercased tag name, exactly as [`scan_tags`] reports it.
    pub name: String,
    /// Nesting depth at the open tag; 0 at top level.
    pub depth: usize,
    /// Byte range of the open tag, `<` through `>`.
    pub open: (usize, usize),
    /// Byte range of the end tag, or `None` when the element was closed
    /// implicitly (HTML's optional end tags, or mis-nesting recovery — a
    /// `</b>` that closes an open `<i>` beneath it) or left unclosed at EOF.
    pub close: Option<(usize, usize)>,
    /// Byte offset where the element's content ends: its end tag's start, its
    /// implicit closer's start, or `html.len()`.
    pub content_end: usize,
}

/// Which content mode an element's **children** are parsed in.
///
/// HTML has three, and the difference is not decorative: it decides whether a
/// start tag's self-closing `/` is honoured, and whether a `<div>` nests or
/// tears the parser back out to HTML. `walk_elements` carries one of these per
/// open element rather than the single `in_foreign` bool it used to, because a
/// bool cannot express the thing that made `e77173` wrong — that foreign
/// content can contain islands of HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContentMode {
    Html,
    Svg,
    MathMl,
}

/// SVG's **HTML integration points**: their children are HTML content again,
/// so `<svg><foreignObject><div/>x` leaves the `div` OPEN — the flag is a
/// parse error the parser ignores, exactly as at top level. Measured in real
/// Chromium (ti `490d97` wave 1); modelling `foreignObject` as ordinary
/// foreign content is what made the walk close it (`e77173`).
///
/// **`title` is the spec's third one, and it is listed here as of ti
/// `2e2453`.** It could not be before: [`scan_tags`] entered raw-text state
/// for that name unconditionally, so an SVG title's interior never reached
/// this walk as markup at all, and listing it would have claimed a fidelity
/// the tokenizer one layer down did not provide. The tokenizer now enters
/// that state only in HTML content, so `<svg><title>a<b>c</b></title>` mints
/// the real `<b>` a browser mints, and the claim is one the whole crate makes
/// together.
fn is_svg_html_integration_point(name: &str) -> bool {
    ["foreignObject", "desc", "title"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
}

/// MathML's **text integration points**: `<math><mi><div/>` opens the `div`
/// for the same reason `foreignObject` does.
fn is_mathml_text_integration_point(name: &str) -> bool {
    ["mi", "mo", "mn", "ms", "mtext"]
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
}

/// The HTML start tags that force a parser **out** of foreign content
/// entirely, popping open foreign elements until it is back in HTML. Without
/// this, `<svg><div>x</div></svg>` reads as a `div` nested inside the `svg`,
/// and every extent from the `div` onward carries a depth and a parent that
/// no browser agrees with.
///
/// The list is the spec's, in its order. `font` is **not** here because it
/// breaks out only when it carries `color`, `face` or `size` — that one is
/// attribute-conditional and is answered by [`breaks_out_of_foreign`].
const FOREIGN_BREAKOUT_TAGS: &[&str] = &[
    "b",
    "big",
    "blockquote",
    "body",
    "br",
    "center",
    "code",
    "dd",
    "div",
    "dl",
    "dt",
    "em",
    "embed",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "hr",
    "i",
    "img",
    "li",
    "listing",
    "menu",
    "meta",
    "nobr",
    "ol",
    "p",
    "pre",
    "ruby",
    "s",
    "small",
    "span",
    "strong",
    "strike",
    "sub",
    "sup",
    "table",
    "tt",
    "u",
    "ul",
    "var",
];

/// Does this start tag tear the parser out of foreign content?
fn breaks_out_of_foreign(name: &str, html: &str, span: (usize, usize)) -> bool {
    if FOREIGN_BREAKOUT_TAGS
        .iter()
        .any(|n| name.eq_ignore_ascii_case(n))
    {
        return true;
    }
    // `font` breaks out only when it carries one of these three; a bare
    // `<font>` inside `<svg>` is an ordinary foreign element.
    name.eq_ignore_ascii_case("font") && tag_has_any_attr(html, span, &["color", "face", "size"])
}

/// The content mode an element is **itself** in — its namespace, as opposed
/// to the one its children are read in.
///
/// `<svg>` and `<math>` are the only two start tags that ENTER foreign
/// content, and they do it from HTML content only: inside foreign content
/// "any other start tag" inserts an element in the namespace of the adjusted
/// current node, so `<math><svg>` is a **MathML** element that happens to be
/// named `svg`. Measured (DCR-0051): Chromium parses `<math><svg><desc><div>`
/// with the `div` at TOP LEVEL — `desc` under a MathML `svg` is not an SVG
/// HTML integration point, so the breakout tag leaves foreign content
/// entirely. Reading the name without the parent claimed an HTML island that
/// is not there.
fn own_content_mode(parent: ContentMode, name: &str) -> ContentMode {
    match parent {
        ContentMode::Html if name.eq_ignore_ascii_case("svg") => ContentMode::Svg,
        ContentMode::Html if name.eq_ignore_ascii_case("math") => ContentMode::MathMl,
        other => other,
    }
}

/// The content mode for the children of `name`, an element whose own
/// namespace is `own`.
fn child_content_mode(
    own: ContentMode,
    name: &str,
    html: &str,
    span: (usize, usize),
) -> ContentMode {
    match own {
        ContentMode::Svg if is_svg_html_integration_point(name) => ContentMode::Html,
        ContentMode::MathMl if is_mathml_text_integration_point(name) => ContentMode::Html,
        // `annotation-xml` is an HTML integration point only when its
        // `encoding` is `text/html` or `application/xhtml+xml`.
        ContentMode::MathMl if name.eq_ignore_ascii_case("annotation-xml") => {
            match tag_attr_value(html, span, "encoding") {
                Some(enc)
                    if enc.eq_ignore_ascii_case("text/html")
                        || enc.eq_ignore_ascii_case("application/xhtml+xml") =>
                {
                    ContentMode::Html
                }
                _ => ContentMode::MathMl,
            }
        }
        other => other,
    }
}

// ---------------------------------------------------------------------------
// THE open-element stack discipline (DCR-0051)
// ---------------------------------------------------------------------------

/// One frame of THE open-element stack.
///
/// Two modes, because HTML asks two different questions of an open element
/// and answering both from one field is what ti `bb961a` cost. `child` is
/// what the TOKENIZER needs — does `<script>` open a raw-text run here, does
/// `<![CDATA[` end at `]]>` or at the first `>`, is a `/` honoured — and it
/// is what [`current_mode`] reports. `own` is the element's NAMESPACE, and it
/// decides three tree-construction questions the tokenizer never asks: is an
/// end tag dispatched to the foreign-content rules, is this frame in HTML's
/// **special** category, and does it terminate a scope search.
///
/// `<svg><desc>` is the pair that separates them: `own == Svg`, `child ==
/// Html`.
#[derive(Debug, Clone)]
struct Frame {
    /// Lowercased tag name, exactly as [`scan_tags`] reports it.
    name: String,
    /// The namespace the element itself is in.
    own: ContentMode,
    /// The content mode its children are read in.
    child: ContentMode,
}

/// HTML's **special** category, in the spec's own order.
///
/// Its one job here is in-body's "any other end tag", which walks down from
/// the current node and stops dead at the first special element instead of
/// closing whatever nearest frame happens to share the name. Without it
/// `<div><b><div></b>` closed the inner `div` at `</b>` (ti `307283`) and
/// `<svg><desc><div></desc>` closed back into foreign content (ti `bb961a`).
///
/// The foreign members of the category — MathML's `mi`/`mo`/`mn`/`ms`/`mtext`
/// /`annotation-xml` and SVG's `foreignObject`/`desc`/`title` — are not
/// listed here because they are exactly the names [`is_integration_point`]
/// already answers, and they are ALSO the foreign scope terminators. One
/// list, two questions; a second copy is the defect shape this whole change
/// is about.
const SPECIAL_HTML_ELEMENTS: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "search",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

/// In-body's block-level end tags: the ones HTML answers with a SCOPE search
/// and a pop-until-popped, rather than with "any other end tag"'s
/// stop-at-the-first-special walk.
///
/// The difference is not cosmetic. `<div><section></div>` closes both — the
/// `section` is walked straight through, because a scope search only stops at
/// a scope TERMINATOR — while "any other end tag" would have stopped at it and
/// ignored the token. `body`, `html` and `form` ride along: HTML gives each of
/// them a rule of its own that this crate does not model (they change
/// insertion mode, or consult the form pointer), and a scope-bounded pop is
/// what an intake reading a whole HTML document needs from them anyway.
const BLOCK_END_TAGS: &[&str] = &[
    "address",
    "applet",
    "article",
    "aside",
    "blockquote",
    "body",
    "button",
    "center",
    "dd",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "dt",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "header",
    "hgroup",
    "html",
    "listing",
    "main",
    "marquee",
    "menu",
    "nav",
    "object",
    "ol",
    "pre",
    "search",
    "section",
    "summary",
    "ul",
];

/// The start tags in-body IGNORES outright (ti `da6bb5`), minus the four this
/// crate deliberately keeps.
///
/// **The residual, recorded rather than left to be rediscovered.** HTML's list
/// also carries `head`, `body`, `html` and `frameset`, and they are not here
/// because this crate has no insertion modes: `intake::html` walks whole HTML
/// DOCUMENTS through [`element_extents`], where those four are processed in
/// "before head" / "in head" / "after head" and really do open elements. A
/// fragment carrying a second `<body>` therefore still opens one here where a
/// browser ignores it — the over-open direction, which appends a closer rather
/// than losing one.
fn start_tag_is_ignored(stack: &[Frame], name: &str) -> bool {
    matches!(
        name,
        "caption" | "col" | "colgroup" | "frame" | "tbody" | "td" | "tfoot" | "th" | "thead" | "tr"
    ) && !matches!(in_scope(stack, &[&"table"], Scope::Table), Search::Found(_))
}

/// Is this frame one of the foreign elements HTML treats as an island of HTML
/// — MathML's text integration points and `annotation-xml`, SVG's
/// `foreignObject` / `desc` / `title`?
///
/// The same six-and-three names are HTML's foreign **special** elements and
/// its foreign **scope terminators**, so both questions read this one answer.
fn is_integration_point(f: &Frame) -> bool {
    match f.own {
        ContentMode::Html => false,
        ContentMode::MathMl => {
            is_mathml_text_integration_point(&f.name)
                || f.name.eq_ignore_ascii_case("annotation-xml")
        }
        ContentMode::Svg => is_svg_html_integration_point(&f.name),
    }
}

/// Is this frame in HTML's special category?
fn is_special(f: &Frame) -> bool {
    if f.own != ContentMode::Html {
        return is_integration_point(f);
    }
    SPECIAL_HTML_ELEMENTS.contains(&f.name.as_str())
}

/// Which of HTML's four scope flavours a search uses. They differ only in
/// what stops them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Normal,
    ListItem,
    Button,
    Table,
}

/// Does this frame stop a scope search?
fn terminates_scope(f: &Frame, scope: Scope) -> bool {
    if scope == Scope::Table {
        // Table scope's list is short and has no foreign members at all, which
        // is what makes `</td>` reach past a `<div>` foster-parented into a
        // table while `</div>` cannot reach past the table itself.
        return f.own == ContentMode::Html
            && matches!(f.name.as_str(), "html" | "table" | "template");
    }
    if f.own != ContentMode::Html {
        return is_integration_point(f);
    }
    if matches!(
        f.name.as_str(),
        "applet" | "caption" | "html" | "table" | "td" | "th" | "marquee" | "object" | "template"
    ) {
        return true;
    }
    match scope {
        Scope::ListItem => matches!(f.name.as_str(), "ol" | "ul"),
        Scope::Button => f.name == "button",
        Scope::Normal | Scope::Table => false,
    }
}

/// How a downward search over the stack ended.
///
/// The two failure answers are NOT interchangeable, and telling them apart is
/// what lets the balancer keep bytes it used to delete. A fragment is mounted
/// INSIDE the sync wrapper's own `<div>`, so a browser's search continues past
/// the bottom of this stack and into that wrapper — `RanOut` is therefore the
/// case where leaving the bytes lets a `</div>` close the wrapper, which is
/// `contracts.md` §4a's direct-child break. `Blocked` stopped at a frame the
/// fragment itself contributes, ABOVE anything the wrapper adds, so a browser
/// mounting the pane stops in the same place and the bytes are inert there
/// too. Deleting those was the old behaviour and it was not free: an
/// approximation is not a browser's verdict, and `</b>` in
/// `<div><b><div></b></div>` is a token this crate declines to act on while a
/// browser reparents through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Search {
    /// The frame at this index matched.
    Found(usize),
    /// A scope terminator — or, for "any other end tag", a special element —
    /// stopped the search inside this fragment.
    Blocked,
    /// The stack ran out first.
    RanOut,
}

/// The nearest frame named by `names` that a `scope` search reaches.
///
/// The target test comes first, exactly as HTML orders its two steps: that is
/// why `</table>` finds the `table` a scope search would otherwise be stopped
/// by.
fn in_scope(stack: &[Frame], names: &[&&str], scope: Scope) -> Search {
    for (i, f) in stack.iter().enumerate().rev() {
        if f.own == ContentMode::Html && names.iter().any(|n| **n == f.name) {
            return Search::Found(i);
        }
        if terminates_scope(f, scope) {
            return Search::Blocked;
        }
    }
    Search::RanOut
}

/// What an end tag does to the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EndTagEffect {
    /// Frames popped by foreign content's breakout rule for `</p>` and
    /// `</br>`, before the HTML rules see the token.
    breakout: usize,
    /// Frames the token then closed; the deepest of them is the element it
    /// closed. Zero means it closed nothing.
    close: usize,
    /// Delete the bytes: the token moved nothing HERE, and in a mounted pane
    /// the search would have carried on into the sync wrapper's own `<div>`.
    delete: bool,
}

/// HTML's end-tag handling, as much of it as a stack of names can honestly
/// keep — the ONE answer both [`scan_tags_with_state`] and [`walk_elements`]
/// use (DCR-0051).
///
/// **What is modelled**, each because a browser measurement said the old
/// `rposition` + `truncate` was wrong about it:
///
/// * Foreign content's own dispatch. `</p>` and `</br>` are BREAKOUT end tags
///   there — HTML lists them with the breakout start tags — so they pop out of
///   `<svg>`/`<math>` before anything else and are then reprocessed by the
///   HTML rules (ti `9b4d66`, ti `895fb7`). Every other end tag walks down the
///   foreign run looking for its own name and falls through to the HTML rules
///   at the first HTML-namespace frame.
/// * "Any other end tag"'s **special-element guard** (ti `307283`, ti
///   `bb961a`).
/// * **Scope** for the block-level end tags, for `li`, for `h1`..`h6` and for
///   the table ones — including the table-scope terminators that make
///   `<div><table>a</div>` leave the table open (R0010-0039).
/// * `</p>` out of button scope minting an empty paragraph rather than
///   closing anything, and `</br>` minting a `<br>`. Both are `Inert` rather
///   than `Orphan`: a browser turns them into content, so deleting them edits
///   what the reader sees.
///
/// **What is NOT modelled, and why the omission is safe.** The **adoption
/// agency algorithm** is absent: a formatting end tag is routed through "any
/// other end tag" instead. Where the two differ, AAA removes the formatting
/// element from the stack and inserts a clone deeper, while this keeps the
/// frame — so the walk holds a formatting frame a browser has already moved,
/// and the balancer appends one redundant `</b>`-shaped closer. That closer is
/// a no-op in a browser (AAA drops an end tag whose formatting element is not
/// on the stack), a formatting element is never special and never terminates a
/// scope, and the direction is the safe one: an extra frame appends a closer
/// rather than losing one. `<div><b><div></b></div>` and
/// `<div><b><div></b></div></div>` are both measured balanced in Chromium
/// under this approximation.
fn end_tag_effect(stack: &[Frame], name: &str) -> EndTagEffect {
    let mut breakout = 0usize;
    if stack.last().is_some_and(|f| f.own != ContentMode::Html) {
        if name == "br" || name == "p" {
            // "While the current node is not a MathML text integration point,
            // an HTML integration point, or an element in the HTML namespace,
            // pop" — which is precisely "while this frame's children are not
            // read as HTML".
            breakout = stack
                .iter()
                .rev()
                .take_while(|f| f.child != ContentMode::Html)
                .count();
        } else {
            // Foreign content's "any other end tag", in the spec's own step
            // ORDER, which is load-bearing rather than pedantic: the name test
            // applies to the current node and to each FOREIGN node below it,
            // and the namespace test comes first on every step after the
            // first. Testing the name at an HTML-namespace frame instead let
            // `<svg><foreignObject><p></foreignObject></svg>…</div>` walk
            // straight past the `foreignObject` — a scope terminator — and
            // match the sync wrapper's own `<div>`, which is `contracts.md`
            // §4a's break arriving through the repair meant to prevent it.
            let mut i = stack.len() - 1;
            loop {
                if stack[i].name == name {
                    return EndTagEffect {
                        breakout: 0,
                        close: stack.len() - i,
                        delete: false,
                    };
                }
                if i == 0 {
                    // Ran off the bottom. In a mounted pane the next frame
                    // down is the wrapper, an HTML element, so this lands in
                    // the spec's step 7 exactly as the break below does.
                    break;
                }
                i -= 1;
                if stack[i].own == ContentMode::Html {
                    // Step 7: hand the token to the HTML rules, on the whole
                    // stack.
                    break;
                }
            }
        }
    }
    let live = &stack[..stack.len() - breakout];
    let html_effect = html_end_tag_effect(live, name);
    EndTagEffect {
        breakout,
        close: html_effect.close,
        // A breakout already moved frames, so the bytes are structure whatever
        // the HTML rules then made of the token.
        delete: html_effect.delete && breakout == 0,
    }
}

/// The "in body" half of [`end_tag_effect`].
fn html_end_tag_effect(stack: &[Frame], name: &str) -> EndTagEffect {
    let pop = |i: usize| EndTagEffect {
        breakout: 0,
        close: stack.len() - i,
        delete: false,
    };
    let drop_it = EndTagEffect {
        breakout: 0,
        close: 0,
        delete: true,
    };
    let keep = EndTagEffect {
        breakout: 0,
        close: 0,
        delete: false,
    };
    let scoped = |names: &[&&str], scope: Scope| match in_scope(stack, names, scope) {
        Search::Found(i) => pop(i),
        Search::Blocked => keep,
        Search::RanOut => drop_it,
    };
    match name {
        // Out of button scope HTML inserts an empty `<p>` and closes it, so
        // the token is content rather than a pop — and never reaches the
        // wrapper, whichever way the search ended.
        "p" => match in_scope(stack, &[&"p"], Scope::Button) {
            Search::Found(i) => pop(i),
            Search::Blocked | Search::RanOut => keep,
        },
        // `</br>` is a parse error a browser treats as a `<br>` START tag.
        // Nothing is closed and nothing may be deleted.
        "br" => keep,
        "li" => scoped(&[&"li"], Scope::ListItem),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            scoped(&[&"h1", &"h2", &"h3", &"h4", &"h5", &"h6"], Scope::Normal)
        }
        "table" | "caption" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th" => {
            scoped(&[&name], Scope::Table)
        }
        n if BLOCK_END_TAGS.contains(&n) => scoped(&[&name], Scope::Normal),
        _ => {
            for (i, f) in stack.iter().enumerate().rev() {
                if f.own == ContentMode::Html && f.name == name {
                    return pop(i);
                }
                if is_special(f) {
                    // Blocked inside the fragment: a browser mounting the pane
                    // stops at this same frame, so the bytes stay.
                    return keep;
                }
            }
            drop_it
        }
    }
}

/// How many frames a START tag closes implicitly, before it is inserted.
///
/// [`implicitly_closes`] stays the one table of NAMES; this is the one place
/// that applies it, and applying it by scope rather than at the top of the
/// stack is ti `da6bb5`'s second half. `<p><em><ul>` closes the paragraph in a
/// browser — the `<ul>` start tag says "if the stack has a `p` element in
/// BUTTON scope, close a p element", which reaches straight through the `<em>`
/// — and this crate reached only the innermost frame, so the `p` survived with
/// `em` on top of it.
fn implied_start_tag_pops(stack: &[Frame], name: &str) -> usize {
    let mut popped = 0usize;

    // HTML's `li` / `dd` / `dt` start-tag loop, which is neither a scope
    // search nor a top-of-stack test: it walks down and stops at the first
    // special element that is not an `address`, a `div` or a `p`.
    let loop_names: &[&str] = match name {
        "li" => &["li"],
        "dd" | "dt" => &["dd", "dt"],
        _ => &[],
    };
    if !loop_names.is_empty() {
        for (i, f) in stack.iter().enumerate().rev() {
            if f.own == ContentMode::Html && loop_names.contains(&f.name.as_str()) {
                popped = stack.len() - i;
                break;
            }
            if is_special(f) && !matches!(f.name.as_str(), "address" | "div" | "p") {
                break;
            }
        }
    }

    // The classic pairs HTML really does decide at the top of the stack: table
    // rows and cells, select options, ruby annotations. Read out of
    // `implicitly_closes` rather than restated, minus the two shapes the rest
    // of this function owns.
    while let Some(f) = stack[..stack.len() - popped].last() {
        let closes_top = f.own == ContentMode::Html
            && implicitly_closes(name)
                .iter()
                .any(|n| *n != "p" && !loop_names.contains(n) && *n == f.name);
        if closes_top {
            popped += 1;
        } else {
            break;
        }
    }

    // "If the stack of open elements has a p element in button scope, then
    // close a p element" — the rule every block-level start tag in
    // `implicitly_closes`' `P` set carries.
    if implicitly_closes(name).contains(&"p")
        && let Search::Found(i) = in_scope(&stack[..stack.len() - popped], &[&"p"], Scope::Button)
    {
        popped = stack.len() - i;
    }

    popped
}

/// The result of the one stack walk over [`scan_tags`]' token stream.
struct Walk {
    extents: Vec<ElementExtent>,
    /// Orphan close tags, in document order — the spans the balancer deletes.
    orphan_closes: Vec<(usize, usize)>,
    /// Names still open at EOF, outermost first.
    unclosed: Vec<String>,
}

/// THE stack walk. [`element_extents`] reads its structure and
/// [`balance_fragment`] reads its repairs; there is exactly one of it, for
/// the same reason there is exactly one Markdown parser — a second walk would
/// be a second opinion about HTML structure, and the two would drift.
///
/// **Since DCR-0051 it holds no opinion at all.** It used to keep its own
/// `open_stack` and its own `rposition` search, and to apply
/// [`implicitly_closes`] where the scanner deliberately did not — two stacks,
/// popping differently from each other and from a browser, which is the single
/// root cause behind ti `9b4d66`, `307283`, `895fb7`, `da6bb5` and `bb961a`.
/// [`scan_tags_with_state`] has to keep the real stack anyway (the content
/// mode of the next byte depends on it), so every mutation is decided there
/// and reported on the token, and this function REPLAYS it while recording
/// what the scanner has no use for: extents, close spans and orphan spans.
///
/// The parallel stack below therefore carries no mode and does no searching —
/// only the name and the extent index each frame belongs to.
fn walk_elements(html: &str) -> Walk {
    let tokens = scan_tags_with_state(html);
    let mut extents: Vec<ElementExtent> = Vec::new();
    let mut open_stack: Vec<(String, usize)> = Vec::new();
    let mut orphan_closes: Vec<(usize, usize)> = Vec::new();

    for tag in &tokens {
        let (span, opened_name) = match &tag.token {
            TagToken::Open { name, span, .. } => (*span, Some(name)),
            TagToken::Close { span, .. } => (*span, None),
            TagToken::Skip { span, .. } => (*span, None),
        };

        // Implied end tags and foreign-content breakouts, replayed. Each frame
        // the scanner took off before processing the token was closed by it
        // implicitly, so its content ends where the token begins — the same
        // record an explicit closer's siblings get below.
        for _ in 0..tag.pre_pops {
            let (_, idx) = open_stack
                .pop()
                .expect("the scanner popped this frame from the same stack");
            extents[idx].content_end = span.0;
        }

        match tag.effect {
            StackEffect::Push => {
                let name = opened_name.expect("only a start tag pushes a frame");
                extents.push(ElementExtent {
                    name: name.clone(),
                    depth: open_stack.len(),
                    open: span,
                    close: None,
                    content_end: html.len(),
                });
                open_stack.push((name.clone(), extents.len() - 1));
            }
            StackEffect::Close(n) => {
                // Everything above the match is closed implicitly by this tag;
                // the deepest of the `n` is the element it actually closed.
                for _ in 1..n {
                    let (_, idx) = open_stack.pop().expect("the scanner counted these frames");
                    extents[idx].content_end = span.0;
                }
                let (_, idx) = open_stack.pop().expect("the scanner counted these frames");
                extents[idx].close = Some(span);
                extents[idx].content_end = span.0;
            }
            // A skipped region, an ignored start tag, or an end tag a browser
            // turns into content rather than into a pop. Not structure, and
            // not the balancer's to delete either.
            StackEffect::Inert => {}
            // Dropping it is what keeps the sync wrapper's own `</div>` safe
            // (spec 2026-08-03 §3.4).
            StackEffect::Orphan => orphan_closes.push(span),
        }
    }

    let unclosed = open_stack.into_iter().map(|(name, _)| name).collect();
    Walk {
        extents,
        orphan_closes,
        unclosed,
    }
}

/// Every element in `html`, in source order by open tag, with its nesting
/// depth, both tag spans and its content end.
///
/// The classification of what those elements *mean* is not here — that is
/// `transync-syntax`'s intake. This is the structure the intake walks.
pub fn element_extents(html: &str) -> Vec<ElementExtent> {
    walk_elements(html).extents
}

/// The reserved sync-attribute namespace: the four `render::attrs` writes,
/// plus `data-skipped` (the placeholder label) and `data-parent-id` (reserved
/// and never emitted, contracts.md §4a). We own this namespace in DOM we
/// mount, and nowhere else.
const RESERVED_SYNC_ATTRS: &[&str] = &[
    "data-sync-id",
    "data-block-kind",
    "data-order",
    "data-fallback",
    "data-parent-id",
    "data-skipped",
];

/// Remove every `RESERVED_SYNC_ATTRS` attribute from `html`'s element open
/// tags, case-insensitively, taking each one's leading whitespace with it —
/// except where that whitespace was the only token separation left.
///
/// That exception is the **seam rule** (ti 490d97 wave 1). Adjacent removals
/// are coalesced into one run first, and a run is replaced by a single U+0020
/// rather than deleted when the first surviving byte is a name byte, `=` or a
/// quote — reachable only in malformed markup, where the plain delete welded
/// `<div data-sync-id="a b="c">x` into `<divc">x` and moved the tag inventory
/// off `["div"]` — and when deleting would weld a preceding `/` onto the `>`
/// and SET a self-closing flag the source never carried. Nowhere else:
/// `<div data-sync-id="x">` still strips to `<div>` byte-exact.
///
/// (Plain backticks, not an intra-doc link: `RESERVED_SYNC_ATTRS` is private
/// and this fn is `pub`, so a link would trip rustdoc's
/// `private_intra_doc_links` lint — warn-by-default, but the pre-commit
/// rustdoc gate runs `-D warnings` over `transync-html` without
/// `--document-private-items`, which makes it a hard commit block. Same rule
/// wave 3's plan states for `intake::html`. Do not "restore" the link.)
///
/// This is **pane-only** (OI-0035 route (c), render half): strip-then-inject
/// is what makes "ours are the only sync attributes in this DOM" a
/// construction rather than a scan. Published output keeps the author's
/// bytes — their `data-sync-id` is their content.
///
/// Stripping DOES change rendered appearance for markup that borrowed this
/// namespace's own presentation: the shipped bundle shell tints
/// `[data-fallback=…]` and `pre[data-skipped]`, so an author's copy of one
/// loses that tint. That is correct — the presentation is engine-owned — but
/// "stripping never changes rendered appearance, because attributes do not
/// paint" was an overclaim, corrected in ti 490d97 wave 1. What holds is
/// narrower: only names in the reserved namespace are removed, only inside
/// element open tags, and no element name and no other attribute moves.
///
/// Only names are matched, and only inside an open tag: attribute values and
/// text are content, and comment interiors — plus the raw-text / RCDATA
/// interiors [`scan_tags`] recognizes — are never tokenized in the first
/// place, so the strip cannot reach into them. "Recognizes" is the operative
/// word since ti `2e2453`: those states are HTML-content states, so inside
/// `<svg>`/`<math>` a `<title>` holds markup, and a reserved attribute
/// planted there is a LIVE attribute a browser honours — so the strip reaches
/// it, as it must. Borrows when nothing matched.
///
/// Malformed markup is tokenized the way a browser tokenizes it where the
/// two were measured to disagree (ti 549b20): a bare quote between
/// attributes is a NAME byte, never a value opener; a stray `=` in
/// attribute-name position STARTS an attribute named `=` rather than
/// opening a value, so the strip neither misses a plant hidden behind one
/// nor deletes text a browser paints after the tag's real end; and a tag
/// left unterminated at EOF is a passed-over [`TagToken::Skip`] region a
/// browser abandons. The two that remained one state earlier — HTML's
/// tag-name state consuming `=` and quote bytes into the ELEMENT name, and
/// its end-tag-open state opening a bogus comment on `</` before a
/// non-letter, both recorded in DCR-0032's 2026-08-21 amendment as ti
/// `e20490` — are closed: `tag_name_end` is now the one name boundary
/// shared by the scanner and this strip, and `</` before a non-letter is a
/// passed-over [`TagToken::Skip`] region that mints no element.
///
/// The tag-name half was NOT harmless while it stood, and the record here
/// used to say it was. `<divq"x=" data-sync-id="v">` is one element named
/// `divq"x="` carrying a real `data-sync-id`; ending the name early buried
/// that plant in a phantom quoted value this strip never examines as a
/// name, so a live anchor survived. Only the pane path's DOMPurify mount
/// dropped it, for a reason — no allowlisted element name contains those
/// bytes — that a consumer which does not sanitize never inherits.
pub fn strip_reserved_sync_attrs(html: &str) -> Cow<'_, str> {
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for token in scan_tags(html) {
        // Exhaustive, and every arm named (wave 0's standing constraint): a
        // token variant added later must stop the compiler here rather than
        // slip past an `if let`'s implicit else and leave its attributes
        // unstripped.
        match token {
            TagToken::Open { span, .. } => collect_reserved_attr_spans(html, span, &mut cuts),
            // A close tag carries no attribute list. A skipped region —
            // comment, CDATA, bogus comment, or a tag left unterminated at
            // EOF (ti 549b20) — never mints an element in a browser, so
            // reserved-name-shaped bytes inside one cannot become live
            // attributes; leaving them unstripped is fail-safe, not an
            // oversight.
            TagToken::Close { .. } | TagToken::Skip { .. } => {}
        }
    }
    if cuts.is_empty() {
        return Cow::Borrowed(html);
    }

    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    let mut k = 0usize;
    while k < cuts.len() {
        // Coalesce the maximal run of ADJACENT cuts before judging the seam.
        // Adjacency is exact: each cut's leading-whitespace backover consumes
        // the whole run down to the previous cut's end, so two reserved
        // attributes separated only by whitespace produce touching spans.
        // Judging per-cut instead would read bytes that lie inside a
        // neighbouring cut — `<div/data-sync-id="a" data-order="b">` would
        // see whitespace after the first cut, delete plainly, and produce
        // `<div/>`, a self-closing flag the author never wrote.
        let cut_start = cuts[k].0;
        let mut cut_end = cuts[k].1;
        while k + 1 < cuts.len() && cuts[k + 1].0 == cut_end {
            k += 1;
            cut_end = cuts[k].1;
        }
        out.push_str(&html[cursor..cut_start]);
        // Both indices are inside the tag by construction: the backover stops
        // at `span.0 + 1`, and every `attr_end` is clamped to the `>`'s own
        // index. So neither read can leave the tag or the string.
        let prev = bytes[cut_start - 1];
        let next = bytes[cut_end];
        // Deleting the run also deletes the token separation it carried. Put
        // ONE space back exactly where its absence would change the parse:
        //
        // - next is whitespace or `/` — a separator survives; a space here
        //   would be a second one.
        // - next is `>` and prev is not `/` — the well-formed case;
        //   `<div data-sync-id="x">` must still strip to `<div>` byte-exact.
        // - next is `>` and prev IS `/` — deleting would weld `/` onto `>`
        //   and SET a self-closing flag the source never had. Measured: on a
        //   foreign root that changes the tree (`<svg/>y</svg>` is an empty
        //   svg with `y` outside it, `<svg/ >y</svg>` is an open svg
        //   containing `y`).
        // - anything else (a name byte, `=`, a quote — reachable only in
        //   malformed markup) — deleting welds the following bytes onto
        //   whatever precedes. Measured: `<div data-sync-id="a b="c">x`
        //   became `<divc">x`, an element a browser names `divc"`, moving
        //   the tag inventory off `["div"]`.
        let needs_separator = match next {
            b'>' => prev == b'/',
            b'/' => false,
            n if n.is_ascii_whitespace() => false,
            _ => true,
        };
        if needs_separator {
            out.push(' ');
        }
        cursor = cut_end;
        k += 1;
    }
    out.push_str(&html[cursor..]);
    Cow::Owned(out)
}

/// One attribute [`walk_attrs`] found: its name exactly as the source spells
/// it, where the name starts, one past the whole `name="value"` run, and the
/// value's own bytes if it has a value.
///
/// The name is **not** folded. Every consumer folds for itself —
/// `collect_reserved_attr_spans` lowercases before matching the reserved set,
/// and the two attribute readers use `eq_ignore_ascii_case` — because the
/// spans this record carries index the source bytes, and a folded copy could
/// not.
struct AttrHit<'a> {
    name: &'a str,
    name_start: usize,
    attr_end: usize,
    value: Option<(usize, usize)>,
}

/// Walk the attributes of ONE start tag, in source order.
///
/// There is exactly one of these for the same reason there is exactly one
/// stack walk: a second attribute reader would be a second opinion about
/// where an attribute ends, and the two would drift. `strip_reserved_sync_attrs`
/// deletes what it reports and `scan_tags_with_state` classifies foreign
/// content by it — `breaks_out_of_foreign` reads `font`'s attributes and
/// `child_content_mode` reads `annotation-xml`'s `encoding` — so a
/// disagreement would be a security question and a structure question at
/// once.
///
/// `span` is a [`TagToken::Open`] span, so `scan_tags` has already proved the
/// tag is well-formed enough to have a name.
fn walk_attrs(html: &str, span: (usize, usize), mut f: impl FnMut(AttrHit<'_>)) {
    let bytes = html.as_bytes();
    let (start, end) = span;
    // `end` is one past the `>`; never look at the `>` itself.
    let limit = end.saturating_sub(1);

    // Step over `<` and the tag name — `scan_tags` already proved one is here,
    // and [`tag_name_end`] is the same boundary it used, so attributes begin
    // exactly where the tokenizer says they do.
    let mut i = tag_name_end(bytes, start + 1, limit);

    while i < limit {
        if bytes[i].is_ascii_whitespace() || bytes[i] == b'/' {
            i += 1;
            continue;
        }
        let name_start = i;
        // HTML's "before attribute name" state: a `=` HERE is a parse error
        // that starts a new attribute whose NAME begins with that `=` — it is
        // not a separator and it is not skipped. So `<div =data-sync-id="x">`
        // carries one junk attribute named `=data-sync-id`, and no reserved
        // attribute is present at all.
        //
        // This walk used to restart the name after such an `=`, which found a
        // `data-sync-id` that a browser never sees and cut it, leaving
        // `<div =>y` — mutating a NON-reserved attribute and contradicting the
        // strip's stated guarantee that nothing but reserved names moves. The
        // direction was over-deletion, never a missed impostor (ti `415cdb`;
        // reviewer A's 51-case browser probe: 50/51 equal, one over-deletion,
        // zero under-deletions).
        if bytes[i] == b'=' {
            i += 1;
        }
        while i < limit && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' && bytes[i] != b'/' {
            i += 1;
        }
        if i == name_start {
            i += 1; // nothing consumable here; the walk must not stall
            continue;
        }
        let name_end = i;

        // The optional `= value`, in HTML's three value shapes.
        let mut j = i;
        while j < limit && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        let mut attr_end = i;
        let mut value = None;
        if j < limit && bytes[j] == b'=' {
            j += 1;
            while j < limit && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < limit && (bytes[j] == b'"' || bytes[j] == b'\'') {
                let quote = bytes[j];
                j += 1;
                let value_start = j;
                while j < limit && bytes[j] != quote {
                    j += 1;
                }
                value = Some((value_start, j));
                attr_end = (j + 1).min(limit);
            } else {
                let value_start = j;
                while j < limit && !bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                value = Some((value_start, j));
                attr_end = j;
            }
        }

        f(AttrHit {
            name: &html[name_start..name_end],
            name_start,
            attr_end,
            value,
        });
        i = attr_end;
    }
}

/// Does this start tag carry any of `wanted`? Used for `font`, which leaves
/// foreign content only when it has `color`, `face` or `size`.
fn tag_has_any_attr(html: &str, span: (usize, usize), wanted: &[&str]) -> bool {
    let mut found = false;
    walk_attrs(html, span, |attr| {
        if wanted.iter().any(|w| attr.name.eq_ignore_ascii_case(w)) {
            found = true;
        }
    });
    found
}

/// The value of `wanted` on this start tag, if it has one, **decoded**. Used
/// for `annotation-xml`, an HTML integration point only at two `encoding`
/// values.
///
/// Decoding is not a nicety here, it is the comparison's premise (R0010-0034 /
/// DCR-0050). A browser decodes character references in an attribute value
/// before anything reads it, so `encoding="text&#47;html"` IS `text/html` and
/// `<math><annotation-xml encoding="text&#47;html"><div/>x` is an HTML
/// integration point in Chromium. Comparing the raw source slice made it
/// MathML content here, which honoured the `<div/>`'s slash and appended only
/// `</annotation-xml></math>` — while a browser had opened the `div` (the
/// slash is a parse error it ignores in HTML content) and then ignored both
/// appended end tags, because in-body's "any other end tag" stops at the
/// special `div`. The fragment ends with three elements still open, so the
/// sync wrapper's own `</div>` closes the `div` and every following block
/// nests inside this one's wrapper — `contracts.md` §4a, reachable from
/// untrusted source Markdown through a type-6 html block (invariant 7).
///
/// The decode lives HERE and not in [`walk_attrs`], and the split is the
/// crate's usual one: `walk_attrs` reports byte SPANS, because
/// `strip_reserved_sync_attrs` cuts bytes out of the source with them and a
/// decoded string has no offsets to cut. This is the one accessor that hands
/// a caller a value to *read*, so it is the one place the decode belongs.
///
/// `htmlize::unescape_attribute` rather than `unescape`: HTML's
/// character-reference state has an attribute-value branch (the
/// ambiguous-ampersand rule — a named reference with no semicolon followed by
/// `=` or an alphanumeric stays literal), and that branch is the one a
/// browser applies to the bytes this function reads. The crate already
/// depends on the same table for text nodes in `scan_chunks`, so this is one
/// decoder used in two contexts rather than a second opinion about entities.
fn tag_attr_value(html: &str, span: (usize, usize), wanted: &str) -> Option<String> {
    let mut out = None;
    walk_attrs(html, span, |attr| {
        if out.is_none() && attr.name.eq_ignore_ascii_case(wanted) {
            out = attr
                .value
                .map(|(a, b)| htmlize::unescape_attribute(&html[a..b]).into_owned());
        }
    });
    out
}

/// Byte ranges of the reserved attributes inside one open tag, appended to
/// `out` in ascending order.
///
/// It does not walk the attribute states itself: DCR-0041 collapsed this onto
/// the one shared [`walk_attrs`], so the strip and the scanner cannot disagree
/// about where an attribute begins or ends.
fn collect_reserved_attr_spans(html: &str, span: (usize, usize), out: &mut Vec<(usize, usize)>) {
    let bytes = html.as_bytes();
    let start = span.0;
    walk_attrs(html, span, |attr| {
        if RESERVED_SYNC_ATTRS.contains(&attr.name.to_ascii_lowercase().as_str()) {
            // Take the leading whitespace along, so removing an attribute
            // does not leave a double space behind.
            let mut cut_start = attr.name_start;
            while cut_start > start + 1 && bytes[cut_start - 1].is_ascii_whitespace() {
                cut_start -= 1;
            }
            out.push((cut_start, attr.attr_end));
        }
    });
}

/// Would deleting `bytes[run_start..run_end]` — one coalesced run of orphan
/// close tags — change how a byte that SURVIVES the deletion tokenizes?
///
/// This is the balancer's half of the **seam rule** ti `490d97` wave 1 gave
/// `strip_reserved_sync_attrs` ("replace the run with one space wherever
/// deleting it would weld bytes together"). Both functions cut bytes out of a
/// fragment, so both owe the same invariant, and until OI-0046's generative
/// run found it only one of them held it:
///
/// **Deleting an orphan must not change how any surviving byte tokenizes.**
///
/// An orphan span is a complete `Close` token, so it never sits inside another
/// token: the byte before it and the byte after it are both in HTML's DATA
/// state (or foreign content, which tokenizes tags identically). Raw text and
/// RCDATA cannot contain one — only the element's own end tag is a token there
/// — and neither can a comment, CDATA section or bogus comment, whose
/// interiors [`scan_tags`] never tokenizes. So there are exactly two bytes
/// whose meaning depends on what FOLLOWS them, and they are the whole hazard:
///
/// * **`<`** enters TAG OPEN, where `!`, `?`, `/` or an ASCII letter forms a
///   token and anything else leaves the `<` a literal character. A `<`
///   immediately before an orphan is necessarily one of those literals: the
///   byte after it is the orphan's own `<`, and `<<` leaves the first one a
///   character. Welding it mints markup out of text: `<</b>x` became `<x`,
///   `a<</b>!-- …` minted an unterminated comment that swallows the pane,
///   `a<</b>/div>tail` minted an end tag that closes the sync wrapper early,
///   and `<</b>div data-sync-id="p-0009">` minted a live element carrying a
///   live anchor out of bytes the strip had correctly passed over as text
///   (invariant 7 / OI-0035 / DCR-0033 — the composition was the bypass, not
///   either layer).
/// * **`&`** enters CHARACTER REFERENCE, where `#` or an alphanumeric can
///   still form one. No tag comes of it, so this half is content fidelity
///   rather than a `contracts.md` §4a break — but it is a real edit to what
///   the reader sees. In `a&</b>amp;` the `&` is a literal character (`&<`
///   names no reference), so the six characters `a`, `&`, `a`, `m`, `p`, `;`
///   reach the page; the plain delete made the bytes one reference and left
///   two, `a` and `&`. The invariant above is about tokenization rather than
///   about tags, so it covers this too.
///
/// Nothing else can weld. Every other byte in data position is a character
/// both before and after the cut, and the byte after the run begins whatever
/// it began before — the run it followed was a token boundary either way.
///
/// The separator is one U+0020 and it is always safe: a cut sits in character
/// position, so the space is character data and can never become markup.
///
/// It is also not free, and the cost is worth stating. A browser renders
/// `<</b>x` as the two characters `<x` — the `<` is data, the end tag closes
/// nothing and is dropped — where this rule renders three, `< x`. Escaping
/// the `<` to `&lt;` would render exactly two, and is still the wrong trade:
/// it rewrites a byte that is NOT in the span being removed (and, for the `&`
/// case, would rewrite the `&` itself), which turns the balancer into an
/// editor of surviving content instead of a function that removes orphans.
/// The strip already chose one space for the same hazard, and one rule for
/// welding in this crate is worth more than one rendered space in a fragment
/// that was malformed to begin with.
fn orphan_cut_would_weld(bytes: &[u8], run_start: usize, run_end: usize) -> bool {
    // A run at either end of the fragment has nothing to weld to. `<` and `&`
    // at EOF are literal characters, so a trailing cut is free.
    let Some(prev) = run_start.checked_sub(1).map(|i| bytes[i]) else {
        return false;
    };
    let Some(&next) = bytes.get(run_end) else {
        return false;
    };
    match prev {
        b'<' => matches!(next, b'!' | b'?' | b'/') || next.is_ascii_alphabetic(),
        b'&' => next == b'#' || next.is_ascii_alphanumeric(),
        _ => false,
    }
}

/// Render-path auto-balancing (spec §3.4): tags opened but never closed in
/// the fragment are closed at its end, and orphan close tags are DROPPED.
///
/// "DROPPED" is a *replacement* rather than a plain deletion at one kind of
/// seam (OI-0046): a run of orphan spans becomes a single U+0020 wherever
/// removing its bytes would weld the surviving bytes into markup they were
/// not. `orphan_cut_would_weld` carries the rule and the reason, and the
/// invariant the two of them establish is that deleting an orphan never
/// changes how a surviving byte tokenizes — which is also what makes this
/// function a fixed point on its own output.
///
/// The other two passes cannot weld, and it is worth saying why rather than
/// leaving it to be re-derived. An appended closer begins with `<`, and a `<`
/// before a `<` stays the literal character it was (HTML's tag-open state
/// emits it and reconsumes the second one), so a fragment ending in a literal
/// `<` is safe to close. A truncation removes a suffix, and no byte can weld
/// onto what is no longer there.
///
/// (Plain backticks, not an intra-doc link: that helper is private and this fn
/// is `pub`, so a link would trip rustdoc's `private_intra_doc_links` lint,
/// which the pre-commit rustdoc gate runs as `-D warnings`. Same note as on
/// `strip_reserved_sync_attrs`; do not "restore" the link.)
///
/// "Closed at its end" is conditional since ti `95f55b`: a closer that
/// would land inside an unterminated trailing comment, CDATA section,
/// bogus comment, raw-text run or tag is **not** kept. The append is
/// re-scanned and truncated when the scanner cannot see it as markup,
/// because bytes buried in such a region close nothing and each re-balance
/// would add another copy without limit.
/// `out.md` never sees this — it exists so an unbalanced fragment cannot
/// consume the sync wrapper's own `</div>` and swallow later anchors.
///
/// R0002-0061: elements whose end tag HTML makes optional are closed
/// implicitly by the start tag that ends them (see [`implicitly_closes`]),
/// rather than accumulating on the stack until the fragment ends. Without
/// that, `<p>a<p>b` collected two open `p`s and earned two appended `</p>`s
/// — the second of which a browser turns into a phantom empty `<p></p>`,
/// structure that appears in the pane and in no source document. Fragments
/// whose optional end tags are all written out are unaffected: the implicit
/// close pops exactly what the explicit one would have.
pub fn balance_fragment(html: &str) -> String {
    let walk = walk_elements(html);
    let bytes = html.as_bytes();

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    // The NET bytes the orphan pass took out before the trailing region — the
    // deletions minus the separators the seam rule put back — so the repair
    // below can map that region's `html` offset onto `out`. It is a net figure
    // rather than a count of deleted bytes because a cut can be replaced by a
    // space instead of removed (see `orphan_cut_would_weld`).
    let mut shift = 0usize;
    let cuts = &walk.orphan_closes;
    let mut k = 0usize;
    while k < cuts.len() {
        // Coalesce the maximal run of ADJACENT orphan spans before judging the
        // seam, for the reason `strip_reserved_sync_attrs` does: judging each
        // cut alone would read the byte before it, which for the second of two
        // touching cuts is a byte this pass has already deleted. `<</b></i>x`
        // would then see the `>` of `</b>` as its predecessor, call the seam
        // safe, and weld the literal `<` onto `x` anyway.
        let run_start = cuts[k].0;
        let mut run_end = cuts[k].1;
        while k + 1 < cuts.len() && cuts[k + 1].0 == run_end {
            k += 1;
            run_end = cuts[k].1;
        }
        out.push_str(&html[cursor..run_start]);
        if orphan_cut_would_weld(bytes, run_start, run_end) {
            out.push(' ');
            // Every orphan span is at least `</a>`, so the run is four bytes
            // or more and the net shift stays positive.
            shift += run_end - run_start - 1;
        } else {
            shift += run_end - run_start;
        }
        cursor = run_end;
        k += 1;
    }
    out.push_str(&html[cursor..]);

    // ti `c1f9a8`: a fragment that ENDS inside an unterminated region is
    // repaired here, before anything else, because it is the one defect the
    // rest of this function cannot reach.
    //
    // The harm is bigger than "the trailing bytes look odd", and bigger than
    // the ticket said. An unterminated COMMENT swallows everything after it,
    // and so does a `SkipKind::PlainText` region (R0010-0032 / DCR-0050),
    // which is why that one is deleted rather than closed — there are no bytes
    // that close it. The other three kinds end at the first `>` — and in a
    // mounted pane the next `>` is the sync wrapper's OWN `</div>`. So the wrapper closes
    // inside the region, the wrapper stays open, and every following block
    // nests inside this block's wrapper: the `contracts.md` §4a direct-child
    // break that wave 1 (`<div/>`) and DCR-0041 (`<svg><div>`) were each fixed
    // for as live breaks, and the harm spec §3.4 gives as the balancer's whole
    // reason to exist. Reachable from ordinary source Markdown: a type-6 html
    // block ends at a blank line, so `<div>x<!--` + blank line + a paragraph
    // is one html block followed by a real anchor.
    //
    // The scanner decides WHAT the region is and WHETHER it closed; this only
    // acts on the answer. Re-deriving either from the bytes here would be a
    // second opinion about where a comment ends — ti `415cdb`, ti `e20490` and
    // ti `2e2453` were each exactly that shape, one region smaller.
    if let Some(TagToken::Skip {
        span,
        kind,
        terminated: false,
    }) = scan_tags(html).last()
    {
        // Only a region running to the very end can swallow what follows.
        if span.1 >= html.len() {
            match kind.terminator() {
                // Close it where a browser closes it. A browser emits a
                // comment left open at EOF, so terminating preserves the
                // author's content rather than inventing any.
                Some(t) => out.push_str(t),
                // Delete instead. Two kinds arrive here and
                // `SkipKind::terminator` carries both reasons: a browser
                // ABANDONS a tag cut off at EOF (no element, no attributes),
                // and it never LEAVES a PLAINTEXT region at all, so neither
                // has bytes that would close it. Deletion is the act this
                // function already performs on an orphan closer, and for
                // PLAINTEXT it is the decision DCR-0050 records rather than
                // an invented third arm.
                None => out.truncate(span.0 - shift),
            }
        }
    }

    if walk.unclosed.is_empty() {
        return out;
    }

    let seam = out.len();
    for name in walk.unclosed.iter().rev() {
        out.push_str("</");
        out.push_str(name);
        out.push('>');
    }

    // A fragment can END inside an unterminated comment, CDATA section,
    // bogus comment, raw-text run or tag — an author may close an HTML block
    // mid-`<!--`, and invariant 7 says that input is expected, not
    // exceptional. The closers just appended then land INSIDE that region,
    // where they are not markup at all: a re-scan hides them, the elements
    // read unclosed again, and the next balance appends another copy without
    // limit (ti `95f55b`: 15,726 violations across 200,000 fuzz iterations).
    //
    // Ask rather than classify. Deciding here where a comment ends would be a
    // second opinion about exactly what `scan_tags` already decides, and two
    // such opinions drifting is what ti `415cdb` was. So put the question to
    // the scanner: if the appended bytes come back as close tokens they
    // closed something and belong; if the scanner cannot see them they are
    // swallowed, and adding them accomplishes nothing but growth.
    //
    // This keeps the append where it IS load-bearing: `</textarea>` ends the
    // raw-text run it would otherwise be buried in, so it comes back visible
    // and stays.
    let mut appended_is_markup = false;
    for token in scan_tags(&out) {
        match token {
            TagToken::Close { span, .. } => {
                if span.0 >= seam {
                    appended_is_markup = true;
                }
            }
            // Nothing appended above spells an open tag or a skip region, so
            // one past the seam is not ours. Named rather than hidden behind
            // a `_` arm, per this module's exhaustiveness policy.
            TagToken::Open { .. } | TagToken::Skip { .. } => {}
        }
    }
    if !appended_is_markup {
        out.truncate(seam);
    }
    out
}

// Spec 2026-08-03 §3.2: ordered extraction algorithm — coalesce per text
// node, decode after coalescing, drop whitespace-only on the decoded form.
#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn segments_are_collected_in_document_order_with_parent_labels() {
        let block = "<details><summary>Click me</summary><p>Body &amp; soul</p></details>";
        let segs = extract(block).expect("extracts");
        assert_eq!(
            segs.texts,
            vec!["Click me".to_string(), "Body & soul".to_string()]
        );
        assert_eq!(segs.labels, vec!["summary".to_string(), "p".to_string()]);
    }

    #[test]
    fn script_style_and_comment_content_is_never_collected() {
        let block =
            "<div><script>var x = 'no';</script><style>.a{}</style><!-- hidden -->visible</div>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["visible".to_string()]);
        assert_eq!(segs.labels, vec!["div".to_string()]);
    }

    /// Spec §9: template text is not extracted — same bucket as script/style.
    /// lol_html reports it as `TextType::Data`, so the open-element stack is
    /// the only thing that can tell it apart.
    #[test]
    fn template_content_is_never_collected() {
        let block = "<template><div>Hello template</div></template>";
        let segs = extract(block).expect("extracts");
        assert!(
            segs.texts.is_empty(),
            "template text leaked into the payload: {:?}",
            segs.texts
        );
        // The node still appears in the scan — splice needs one record per
        // text node in both passes, just with kept: false.
        let records = scan(block).expect("scans");
        assert_eq!(records.len(), 1);
        assert!(!records[0].kept);
        assert_eq!(records[0].decoded, "Hello template");
    }

    #[test]
    fn template_nested_in_a_div_suppresses_only_its_own_text() {
        let block = "<div>before<template><p>inside</p></template>after</div>";
        let segs = extract(block).expect("extracts");
        assert_eq!(
            segs.texts,
            vec!["before".to_string(), "after".to_string()],
            "only the text outside the template is translatable"
        );
        assert_eq!(segs.labels, vec!["div".to_string(), "div".to_string()]);
    }

    #[test]
    fn nbsp_only_text_node_is_dropped_on_the_decoded_form() {
        let block = "<p>&nbsp;</p><p>real</p>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["real".to_string()]);
    }

    #[test]
    fn entities_are_decoded_after_coalescing() {
        // A large text node forces lol_html to deliver multiple chunks in
        // some configurations; correctness must not depend on chunking, so
        // scan() must coalesce before decoding either way.
        let block = "<p>a &lt;tag&gt; and &copy; sign</p>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["a <tag> and © sign".to_string()]);
    }

    #[test]
    fn top_level_text_outside_any_element_is_captured_as_fragment() {
        let block = "</div>\norphan tail prose";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["\norphan tail prose".to_string()]);
        assert_eq!(segs.labels, vec!["fragment".to_string()]);
    }

    #[test]
    fn pre_content_is_extracted_with_pre_label() {
        let block = "<pre>line one\n\nline two</pre>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["line one\n\nline two".to_string()]);
        assert_eq!(segs.labels, vec!["pre".to_string()]);
    }

    #[test]
    fn zero_segment_block_yields_empty_extraction() {
        let block = "<img src=\"a.png\"><hr><!-- badges -->";
        let segs = extract(block).expect("extracts");
        assert!(segs.texts.is_empty());
    }

    #[test]
    fn scan_reports_dropped_nodes_and_agrees_with_extract() {
        let block = "<p>&nbsp;</p><p>kept</p>";
        let records = scan(block).expect("scans");
        assert_eq!(records.len(), 2);
        assert!(!records[0].kept);
        assert!(records[1].kept);
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts.len(), records.iter().filter(|r| r.kept).count());
    }

    #[test]
    fn rewriter_error_surfaces_as_err() {
        // The memory cap is the one deterministic way to make the rewriter
        // fail; the production degrade path (spec §3.2) rides this Err.
        let big = format!("<p>{}</p>", "x".repeat(64 * 1024));
        assert!(scan_with_memory_cap(&big, 16).is_err());
    }

    #[test]
    fn entity_split_across_two_writes_is_coalesced_before_decoding() {
        // Two `write()` calls cut `&copy;` in half. Decoding per chunk would
        // yield "a &co" + "py; b"; coalescing first yields the one true text
        // node. This deterministically pins the coalesce-then-decode order
        // that the positional splice alignment depends on.
        let records =
            scan_chunks(&["<p>a &co", "py; b</p>"], DEFAULT_MAX_MEMORY_BYTES).expect("scans");
        assert_eq!(records.len(), 1, "one text node, not one per write");
        assert_eq!(records[0].decoded, "a © b");
    }
}

// Spec 2026-08-20 §5: the CommonMark blank-line rule has exactly one home.
#[cfg(test)]
mod policy_tests {
    use super::*;

    /// `matches!(block_type, 6 | 7)` is what `splice` computed inline before
    /// wave 0, and this constructor is now its only home. Every other type —
    /// including the `0` a non-CommonMark host has no value for, and anything
    /// outside CommonMark's 1–7 domain — keeps its blank lines.
    #[test]
    fn the_commonmark_mapping_is_six_and_seven_and_nothing_else() {
        for t in 0u8..=7 {
            let expected = if t == 6 || t == 7 {
                BlankLinePolicy::Collapse
            } else {
                BlankLinePolicy::Keep
            };
            assert_eq!(
                BlankLinePolicy::from_commonmark_html_block_type(t),
                expected,
                "block type {t}"
            );
        }
        assert_eq!(
            BlankLinePolicy::from_commonmark_html_block_type(200),
            BlankLinePolicy::Keep,
            "out of CommonMark's domain entirely: Keep, never a panic"
        );
    }
}

// Spec 2026-08-20 §5: the token stream grows the two things the HTML intake
// needs, and stays inert for the two consumers that ship today.
#[cfg(test)]
mod token_tests {
    use super::*;

    #[test]
    fn an_open_tag_span_slices_back_to_its_own_bytes() {
        let html = "<p>x</p><img src=\"a.png\"/>";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 2, "two open tags: {opens:?}");
        assert_eq!(opens[0].0, "p");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<p>");
        assert_eq!(opens[1].0, "img");
        assert_eq!(&html[opens[1].1.0..opens[1].1.1], "<img src=\"a.png\"/>");
    }

    #[test]
    fn comments_cdata_and_bogus_comments_become_skip_tokens() {
        let comment = "<!-- note --><p>x</p>";
        assert_eq!(skips(comment), vec![(0, 13)]);
        assert_eq!(&comment[0..13], "<!-- note -->");

        // Foreign content: the section ends at `]]>`.
        let foreign = "<svg><![CDATA[<b>]]></svg>";
        let foreign_skips = skips(foreign);
        assert_eq!(foreign_skips.len(), 1);
        assert_eq!(
            &foreign[foreign_skips[0].0..foreign_skips[0].1],
            "<![CDATA[<b>]]>"
        );

        // HTML content: the same bytes are a bogus comment ending at the
        // first `>`.
        let in_html = "<p><![CDATA[</b>]]></p>";
        let html_skips = skips(in_html);
        assert_eq!(html_skips.len(), 1);
        assert_eq!(&in_html[html_skips[0].0..html_skips[0].1], "<![CDATA[</b>");

        // The doctype: `!` fails the tag-open state's ASCII-letter test, so
        // before wave 0 this produced no token AND no skip — the region was
        // stepped over as text. It is a bogus comment, and now it says so.
        let doctype = "<!doctype html>\n<p>x</p>";
        assert_eq!(skips(doctype), vec![(0, 15)]);
        assert_eq!(&doctype[0..15], "<!doctype html>");
    }

    /// The bogus-comment state is a tokenizer change, not just a new token.
    /// Before wave 0 the scanner stepped past `<!` a byte at a time and read
    /// the `<div>` inside as markup — so `balance_fragment` appended a
    /// `</div>` a browser never asked for.
    #[test]
    fn tag_shaped_bytes_inside_a_bogus_comment_are_not_markup() {
        assert!(tag_inventory("<! <div> >").is_empty());
        assert_eq!(balance_fragment("<! <div> >"), "<! <div> >");
        assert!(tag_inventory("<?xml version=\"1.0\"?>").is_empty());
    }

    /// `Skip` never reaches `tag_inventory`, which filters it out — and, since
    /// ti `c1f9a8`, it DOES reach the balancer, which repairs an unterminated
    /// trailing one. This test's subject is the other case, and the one that
    /// has to keep holding: a region that is properly TERMINATED is inert for
    /// both consumers, so the repair cannot reach ordinary markup.
    ///
    /// (Renamed from `skip_tokens_are_invisible_to_both_shipped_consumers`,
    /// whose claim `c1f9a8` made false for the balancer.)
    #[test]
    fn terminated_skip_regions_stay_inert_for_both_shipped_consumers() {
        let html = "<!doctype html><div><!-- c -->text</div>";
        assert_eq!(tag_inventory(html), vec!["div", "/div"]);
        assert_eq!(balance_fragment(html), html);
        assert_eq!(skips(html).len(), 2, "the doctype and the comment");
    }

    /// ti 549b20: HTML's before-attribute-name / attribute-name states make
    /// a quote not preceded by `=` part of the attribute NAME. Opening a
    /// phantom quoted value here desynchronized the scanner from every
    /// browser and hid the rest of the document from every consumer.
    #[test]
    fn a_bare_quote_in_a_tag_is_a_name_byte_not_a_value_opener() {
        let html = "<div \">x";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 1, "one open tag: {opens:?}");
        assert_eq!(opens[0].0, "div");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<div \">");
    }

    /// ti 549b20, measured in headless Chromium: a stray `=` in
    /// before-attribute-name state STARTS an attribute named `=`, and the
    /// quote after it joins that NAME — no value state is entered, so the
    /// browser ends this tag at the FIRST `>`. The ordinary `name=value`
    /// shape reaches `=` from a consumed name and still opens the value.
    #[test]
    fn a_stray_equals_does_not_open_a_value() {
        let html = "<div =\"> data-sync-id=\"v\">x</div>";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 1, "one open tag: {opens:?}");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<div =\">");
        // The ordinary shape is untouched: `=` after a NAME opens the
        // value, and a `>` inside that value stays data.
        let ok = "<div a=\">\" b>x";
        let spans: Vec<(usize, usize)> = scan_tags(ok)
            .into_iter()
            .filter_map(|t| match t {
                TagToken::Open { span, .. } => Some(span),
                TagToken::Close { .. } | TagToken::Skip { .. } => None,
            })
            .collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(&ok[spans[0].0..spans[0].1], "<div a=\">\" b>");
    }

    /// ti 549b20: the old exit left the document loop with no token, so a
    /// caller could not tell a passed-over suffix from a scanned one. A
    /// browser abandons a tag truncated at EOF — no element, no attributes —
    /// and the scanner now says so in the stream.
    #[test]
    fn an_unterminated_tag_is_a_skip_to_eof_not_a_silent_abort() {
        let html = "<p>a</p><div class=\"x";
        assert_eq!(skips(html), vec![(8, 21)]);
        assert_eq!(&html[8..21], "<div class=\"x");
        // The ledger is unaffected by construction: `tag_inventory` filters
        // `Skip` out, so the region never mints an inventory entry.
        assert_eq!(tag_inventory(html), vec!["p", "/p"]);
    }

    /// R0010-0049: HTML's comment-end-BANG state closes a comment at `--!>`
    /// (an incorrectly-closed-comment parse error — the token is still
    /// emitted), so a scanner that knows only `-->` reads the markup after one
    /// as comment text.
    ///
    /// The boundary cases are what keep the second terminator from being a
    /// blunt instrument, and they are why [`comment_end`] searches the two
    /// forms from different offsets: comment-end-bang is reachable only
    /// THROUGH comment-end, which the opener's own `--` never enters.
    #[test]
    fn a_comment_closes_at_the_bang_form_too() {
        let html = "<!-- a --!><div>y<!-- b -->";
        assert_eq!(skips(html), vec![(0, 11), (17, 27)]);
        assert_eq!(&html[0..11], "<!-- a --!>");
        assert_eq!(
            tag_inventory(html),
            vec!["div"],
            "the div after the bang-closed comment is real markup"
        );
        // `<!--!>` is NOT closed: comment-start reconsumes the `!` as data, so
        // comment-end-bang is never reached. Searching the bang form from the
        // `<` would have claimed it was.
        assert_eq!(skips("<!--!>x"), vec![(0, 7)]);
        // With one more `--` in front of it, the state IS reached.
        assert_eq!(skips("<!----!>x"), vec![(0, 8)]);
        // The abrupt-closing forms are unmoved: comment-start and
        // comment-start-dash both close on `>`, which is what searching `-->`
        // from the `<` models.
        assert_eq!(skips("<!-->x"), vec![(0, 5)]);
        assert_eq!(skips("<!--->x"), vec![(0, 6)]);
        // Whichever form comes FIRST ends the comment.
        assert_eq!(skips("<!-- a --> b --!> c"), vec![(0, 10)]);
    }

    /// R0010-0032: HTML's PLAINTEXT state has no exit, so `<plaintext>` and
    /// every byte after it is one passed-over region rather than markup — the
    /// start tag included, because a `Skip` beside a live `Open` would be two
    /// tokens over the same bytes and the balancer could repair only one of
    /// them (see [`SkipKind::PlainText`]).
    #[test]
    fn plaintext_makes_the_rest_of_the_fragment_one_region() {
        let html = "<div><plaintext></div><p>x</p>";
        assert_eq!(skips(html), vec![(5, html.len())]);
        assert_eq!(
            tag_inventory(html),
            vec!["div"],
            "nothing after `<plaintext>` is markup, so nothing after it counts"
        );
        // An HTML-content rule: inside foreign content `plaintext` is an
        // ordinary element and the state is never entered.
        assert!(skips("<svg><plaintext></div>").is_empty());
        assert_eq!(
            tag_inventory("<svg><plaintext></div>"),
            vec!["svg", "plaintext", "/div"]
        );
        // RCDATA wins where it already ran: a browser in `<title>` does not
        // tokenize the tag at all.
        assert!(skips("<title><plaintext></title>").is_empty());
    }

    /// The `Skip` spans of `html`, in document order.
    fn skips(html: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Skip { span, .. } = token {
                out.push(span);
            }
        }
        out
    }
}

// Spec 2026-08-20 §5: one stack walk, two readers.
#[cfg(test)]
mod extent_tests {
    use super::*;

    #[test]
    fn extents_are_in_source_order_with_depth_and_both_tag_spans() {
        let html = "<div><p>a</p><p>b</p></div>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div", "p", "p"]);
        assert_eq!(ex[0].depth, 0);
        assert_eq!(ex[1].depth, 1);
        assert_eq!(ex[2].depth, 1);
        assert_eq!(&html[ex[1].open.0..ex[1].open.1], "<p>");
        let inner_close = ex[1].close.expect("the first <p> is closed");
        assert_eq!(&html[inner_close.0..inner_close.1], "</p>");
        assert_eq!(&html[ex[1].open.1..ex[1].content_end], "a");
        let outer_close = ex[0].close.expect("the div is closed");
        assert_eq!(&html[outer_close.0..outer_close.1], "</div>");
        assert_eq!(&html[ex[0].open.1..ex[0].content_end], "<p>a</p><p>b</p>");
    }

    #[test]
    fn an_implicitly_closed_element_has_no_close_span_and_ends_at_its_closer() {
        let html = "<ul><li>a<li>b</ul>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["ul", "li", "li"]);
        assert!(ex[1].close.is_none(), "the first <li> is closed implicitly");
        assert_eq!(&html[ex[1].open.1..ex[1].content_end], "a");
        assert!(ex[2].close.is_none(), "the second <li> is closed by </ul>");
        assert_eq!(&html[ex[2].open.1..ex[2].content_end], "b");
    }

    #[test]
    fn an_unclosed_element_runs_to_end_of_input() {
        let html = "<div><span>x";
        let ex = element_extents(html);
        assert_eq!(ex.len(), 2);
        assert!(ex[0].close.is_none());
        assert_eq!(ex[0].content_end, html.len());
        assert!(ex[1].close.is_none());
        assert_eq!(ex[1].content_end, html.len());
    }

    #[test]
    fn void_elements_mint_no_extent_and_a_flagged_non_void_one_does() {
        assert!(element_extents("<br><img src=\"a.png\"/>").is_empty());
        // …but a self-closing raw-text start tag DOES open one, exactly as
        // the balancer treats it: HTML ignores `/` there (DCR-0016 Part D).
        let ex = element_extents("<textarea/>");
        assert_eq!(ex.len(), 1);
        assert_eq!(ex[0].name, "textarea");
        assert!(ex[0].close.is_none());
        // ti 490d97 wave 1. The old name said "and self-closing", which was
        // an XML reading of the flag: outside foreign content HTML ignores it
        // and the element opens, so a flagged non-void tag mints an extent
        // like any other. Every assertion above survived the fix unchanged —
        // its only flagged tags are a VOID `img` and a RAW-TEXT `textarea`,
        // the two carve-outs — which is why the lie needed a positive arm
        // rather than a rewrite.
        let ex = element_extents("<span/>x");
        assert_eq!(ex.len(), 1, "a flagged non-void tag opens: {ex:?}");
        assert_eq!(ex[0].name, "span");
        assert!(ex[0].close.is_none());
    }

    /// ti 490d97 wave 1: the author's own end tag is the extent's `close`,
    /// not an orphan the balancer deletes.
    #[test]
    fn a_flagged_non_void_element_owns_the_close_tag_that_follows_it() {
        let html = "<div/>y</div>";
        let ex = element_extents(html);
        assert_eq!(ex.len(), 1, "{ex:?}");
        assert_eq!(ex[0].name, "div");
        assert_eq!(ex[0].depth, 0);
        assert_eq!(&html[ex[0].open.0..ex[0].open.1], "<div/>");
        let close = ex[0].close.expect("the author's </div> closes it");
        assert_eq!(&html[close.0..close.1], "</div>");
        assert_eq!(&html[ex[0].open.1..ex[0].content_end], "y");
    }

    /// The carve-out, from the extents side: foreign content and the
    /// `<svg>`/`<math>` roots are the two places HTML really does honour the
    /// flag, so a flagged tag there mints nothing. Every arm but the last was
    /// green before the wave-1 fix too — they guard it from over-reaching.
    /// The last arm is the new one: being inside foreign content is a STACK
    /// property, so past `</svg>` the flag is ignored again, and a fix that
    /// latched a counter instead would leave that `<span/>` unpushed. The
    /// walk carried that as an `in_foreign` bool until `e77173` made it a
    /// per-element [`ContentMode`], and `2e2453` moved the stack itself down
    /// into the scanner; the property this arm guards is unchanged.
    #[test]
    fn a_flagged_tag_in_foreign_content_mints_no_extent() {
        let names: Vec<String> = element_extents("<svg><rect/><circle/></svg>")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["svg"], "rect and circle are empty siblings");
        assert!(
            element_extents("<svg/>after").is_empty(),
            "the foreign ROOT honours its own flag"
        );
        assert!(element_extents("<math/>x").is_empty());
        // Nested foreign descendants inherit the bit through the stack.
        let names: Vec<String> = element_extents("<svg><g><rect/></g></svg>")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["svg", "g"]);
        // …and it is a stack property, not a latch: past `</svg>` the flag
        // is ignored again.
        let names: Vec<String> = element_extents("<svg><rect/></svg><span/>x")
            .into_iter()
            .map(|e| e.name)
            .collect();
        assert_eq!(names, vec!["svg", "span"]);
    }

    fn names(html: &str) -> Vec<String> {
        element_extents(html).into_iter().map(|e| e.name).collect()
    }

    /// `e77173`: foreign content contains ISLANDS of HTML. Inside SVG's
    /// `foreignObject` / `desc` the parser is back in HTML content, so a
    /// self-closing `/` is the parse error it always is out here and the
    /// element OPENS. Measured in real Chromium: `<svg><foreignObject><div/>x`
    /// leaves the `div` open.
    ///
    /// Before this, the walk inherited one `in_foreign` bool down the stack,
    /// so `foreignObject` looked like any other foreign element and the `div`
    /// minted no extent at all.
    #[test]
    fn an_html_integration_point_returns_its_children_to_html_content() {
        assert_eq!(
            names("<svg><foreignObject><div/>x"),
            // `scan_tags` lowercases names, so the extent is `foreignobject`
            // even though the source spells it camelCase.
            vec!["svg", "foreignobject", "div"],
            "foreignObject re-enters HTML, so the div's slash is ignored"
        );
        assert_eq!(names("<svg><desc><div/>x"), vec!["svg", "desc", "div"]);
        // The contrast that makes it a rule rather than a special case: an
        // ordinary foreign element does NOT re-enter HTML.
        assert_eq!(
            names("<svg><g><rect/>x"),
            vec!["svg", "g"],
            "rect stays foreign, so its slash still closes it"
        );
    }

    /// `e77173`: the breakout set tears the parser out of foreign content
    /// before the tag is processed, popping the open foreign elements. Without
    /// it `<svg><g><div>` reads as a div nested two deep inside an svg, a
    /// shape no browser produces.
    #[test]
    fn a_breakout_tag_leaves_foreign_content_before_it_opens() {
        let extents = element_extents("<svg><g><div>x</div></svg>");
        let shape: Vec<(String, usize)> =
            extents.iter().map(|e| (e.name.clone(), e.depth)).collect();
        assert_eq!(
            shape,
            vec![
                ("svg".to_string(), 0),
                ("g".to_string(), 1),
                ("div".to_string(), 0)
            ],
            "the div breaks out to top level; svg and g are closed implicitly"
        );
        // Both foreign elements end where the breakout tag starts, and
        // neither gets an end tag it never had.
        let div_open = "<svg><g>".len();
        assert_eq!(extents[0].content_end, div_open, "svg ends at the div");
        assert_eq!(extents[1].content_end, div_open, "g ends at the div");
        assert!(extents[0].close.is_none() && extents[1].close.is_none());

        // A void breakout tag pops just the same, then mints nothing itself.
        assert_eq!(names("<svg><g><br>"), vec!["svg", "g"]);
        // A tag that is NOT in the set stays inside foreign content.
        assert_eq!(names("<svg><g><rect>x"), vec!["svg", "g", "rect"]);
    }

    /// `font` is the one breakout tag that depends on its attributes, so it is
    /// the one that proves the walk reads them rather than guessing.
    #[test]
    fn font_breaks_out_of_foreign_content_only_with_color_face_or_size() {
        assert_eq!(
            names("<svg><font>x</font></svg>")
                .into_iter()
                .zip(
                    element_extents("<svg><font>x</font></svg>")
                        .iter()
                        .map(|e| e.depth)
                )
                .map(|(n, d)| format!("{n}@{d}"))
                .collect::<Vec<_>>(),
            vec!["svg@0", "font@1"],
            "a bare font is an ordinary foreign element"
        );
        let flagged = element_extents("<svg><font color=\"red\">x</font></svg>");
        assert_eq!(
            flagged
                .iter()
                .map(|e| (e.name.as_str(), e.depth))
                .collect::<Vec<_>>(),
            vec![("svg", 0), ("font", 0)],
            "color makes it break out, so the font lands at top level"
        );
    }

    /// `e77173` was a **live `contracts.md` §4a break**, not a benign extent
    /// divergence, and this is the shape that made it one (DCR-0032's
    /// 2026-08-23 correction, measured through the shipped DOMPurify mount).
    ///
    /// `div` is on the breakout list, so a browser pops the `<svg>` at the
    /// `<div>` and leaves that div OPEN in HTML content — where it swallows
    /// the anchor of the block that follows and mounts it under `DIV.wrap`
    /// instead of `<main>`. The walk used to close the div at `</svg>` and
    /// hand the fragment through unchanged, so the balancer saw nothing to
    /// repair. Reachable from untrusted source Markdown via a type-6 html
    /// block, which is why the self-closing flag was never the mechanism: the
    /// unflagged `<svg><div>x</svg>` does exactly the same thing.
    #[test]
    fn a_breakout_div_inside_svg_cannot_swallow_the_anchor_that_follows() {
        for src in [
            "<div class=\"wrap\"><svg><div/>x</svg></div>",
            // The flag is not the mechanism — the unflagged spelling too.
            "<div class=\"wrap\"><svg><div>x</svg></div>",
        ] {
            let extents = element_extents(src);
            let shape: Vec<(&str, usize)> =
                extents.iter().map(|e| (e.name.as_str(), e.depth)).collect();
            assert_eq!(
                shape,
                vec![("div", 0), ("svg", 1), ("div", 1)],
                "the inner div breaks out of the svg and becomes wrap's child, \
                 not the svg's: {src}"
            );

            // The balancer repairs what the walk now sees, so the fragment
            // cannot leave an element open past its own end.
            let balanced = balance_fragment(src);
            let after = walk_elements(&balanced);
            assert!(
                after.unclosed.is_empty(),
                "nothing may outlive the fragment: {balanced}"
            );
            assert!(
                after.orphan_closes.is_empty(),
                "and it may not carry a closer for something never opened: {balanced}"
            );
            assert_eq!(
                balance_fragment(&balanced),
                balanced,
                "idempotent under its own re-scan: {balanced}"
            );
        }
    }

    /// MathML has two kinds of island: the text integration points, and
    /// `annotation-xml` at exactly two `encoding` values.
    #[test]
    fn mathml_islands_return_their_children_to_html_content() {
        // A text integration point: `foo` is not a breakout tag, so the only
        // thing that can open it is being back in HTML content.
        assert_eq!(names("<math><mi><foo/>x"), vec!["math", "mi", "foo"]);
        assert_eq!(
            names("<math><mrow><foo/>x"),
            vec!["math", "mrow"],
            "mrow is ordinary MathML, so foo's slash still closes it"
        );
        // annotation-xml, both ways.
        assert_eq!(
            names("<math><annotation-xml encoding=\"text/html\"><foo/>x"),
            vec!["math", "annotation-xml", "foo"]
        );
        assert_eq!(
            names("<math><annotation-xml encoding=\"application/xhtml+xml\"><foo/>x"),
            vec!["math", "annotation-xml", "foo"]
        );
        assert_eq!(
            names("<math><annotation-xml><foo/>x"),
            vec!["math", "annotation-xml"],
            "no encoding: still MathML, so the slash closes foo"
        );
        assert_eq!(
            names("<math><annotation-xml encoding=\"image/svg+xml\"><foo/>x"),
            vec!["math", "annotation-xml"],
            "an encoding that is not one of the two is not an island"
        );
    }

    #[test]
    fn an_orphan_close_tag_mints_no_extent() {
        let ex = element_extents("</details><p>x</p>");
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["p"],
            "the orphan closes nothing and opens nothing"
        );
    }

    #[test]
    fn skipped_regions_are_not_structure() {
        let ex = element_extents("<!doctype html><div><!-- c -->x</div>");
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div"]);
    }

    /// The walk is SHARED (spec §5): the balancer's "still open at EOF" set
    /// and the extents' "no close span" set are the same computation, and a
    /// divergence here means a second copy crept in.
    #[test]
    fn the_walk_is_shared_with_the_balancer() {
        let html = "<div><span>x";
        // Bound to a local first: the `&str`s below borrow out of the Vec, so
        // calling `element_extents` inline would drop it at the end of the
        // statement (E0716).
        let ex = element_extents(html);
        let unclosed: Vec<&str> = ex
            .iter()
            .filter(|e| e.close.is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(unclosed, vec!["div", "span"]);
        assert_eq!(balance_fragment(html), "<div><span>x</span></div>");
    }

    /// ti 549b20. `<div ">` is a complete open tag — the quote is a name
    /// byte — and it minted nothing before the fix because the scanner
    /// aborted the whole scan instead.
    #[test]
    fn a_stray_quote_tag_mints_an_extent_and_an_unterminated_tag_does_not() {
        let html = "<div \"> <p>x</p>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div", "p"]);
        assert!(ex[0].close.is_none(), "the div is unclosed at EOF");
        assert_eq!(ex[0].content_end, html.len());
        // A tag with no `>` at all is a Skip: no element, exactly as a
        // browser abandons it. An intake caller sees the passed-over region
        // in the token stream, not a phantom element here.
        assert!(element_extents("<div class=\"x").is_empty());
    }

    /// ti `2e2453`, divergence 1 — measured in real Chromium. In foreign
    /// content `title` is an ordinary foreign element, not RCDATA, so its
    /// interior is markup and the `<b>` is a real element. That is also what
    /// finally lets SVG's `title` be listed as the HTML integration point the
    /// spec says it is: inside it the children are HTML content, so a
    /// self-closing `/` there is the parse error it is at top level.
    #[test]
    fn an_svg_title_holds_markup_and_is_an_html_integration_point() {
        let html = "<svg><title>a<b>c</b></title>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["svg", "title", "b"]);
        assert_eq!(
            tag_inventory(html),
            vec!["svg", "title", "b", "/b", "/title"]
        );
        assert_eq!(
            balance_fragment("<svg><title><div/>x"),
            "<svg><title><div/>x</div></title></svg>"
        );
    }

    /// ti `2e2453`, divergence 2 — measured in real Chromium. Foreign content
    /// is one of the two places HTML honours the self-closing flag, and
    /// `script` is not exempt there: the script is empty and `x` is its
    /// sibling. The scanner used to enter raw text and swallow `x` to EOF,
    /// which cost the fragment a `</script>` closing an element a browser
    /// never opened.
    #[test]
    fn a_self_closing_script_inside_foreign_content_swallows_nothing() {
        let html = "<svg><script/>x";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["svg"]);
        assert_eq!(balance_fragment(html), "<svg><script/>x</svg>");
    }

    /// The opposite direction, and the reason the scanner grew a mode STACK
    /// rather than the `foreign_depth` counter it replaced: inside an HTML
    /// integration point the children are HTML content again, so raw text is
    /// entered there exactly as it is at top level. A `foreign_depth > 0`
    /// suppression would have read this as foreign and tokenized the `<b>`.
    #[test]
    fn raw_text_is_still_entered_inside_an_html_integration_point() {
        let html = "<svg><foreignObject><script>a<b>c</script></foreignObject></svg>";
        assert_eq!(
            tag_inventory(html),
            vec![
                "svg",
                "foreignobject",
                "script",
                "/script",
                "/foreignobject",
                "/svg"
            ]
        );
        assert_eq!(balance_fragment(html), html);
    }

    /// HTML content is untouched by the qualifier: the four names still open
    /// raw text / RCDATA at top level, and the `/` on them is still ignored
    /// (DCR-0016 Part D), so `<textarea/>` still earns its closer.
    #[test]
    fn raw_text_in_html_content_is_untouched_by_the_foreign_rule() {
        assert_eq!(balance_fragment("<textarea/>"), "<textarea/></textarea>");
        assert_eq!(
            tag_inventory("<title>a < b </i> c</title>"),
            vec!["title", "/title"]
        );
    }

    /// **What this pins changed at DCR-0051, and the name is kept on purpose**
    /// so DCR-0042's reference to it keeps resolving.
    ///
    /// It used to be the falsifier for a claim in `scan_tags_with_state`'s own
    /// comment: that the scanner could skip HTML's optional end tags because an
    /// extra frame of an ordinary HTML element "reports the mode the one below
    /// it would". Every word of that was true and the conclusion was not — it
    /// reasoned about the extra frame's own mode and never about `truncate`
    /// popping the frames ABOVE it, which is ti `9b4d66`. There is one stack
    /// now and it applies implied closes, so there is no stale frame for this
    /// test to be a claim about. (The comment also cited the test under a name
    /// it has never had, `implicitly_closed_names_never_change_the_content_mode`
    /// — a pin that could not have gone red because it did not exist.)
    ///
    /// Two things it still pins, and both are worth keeping:
    ///
    /// * `KEYS` is closed under [`implicitly_closes`], so this list really does
    ///   reach every name the table can pop and cannot silently stop covering
    ///   it.
    /// * No implicitly-closed name changes the content mode. The live
    ///   direction is the one ti `2e2453` walked — adding a name to
    ///   `is_svg_html_integration_point` (it added `title`) that is also an
    ///   optional-end-tag closer — and it would now mean an implied pop moving
    ///   the tokenizer's mode out from under it, which is a stranger thing than
    ///   the stale frame it used to mean.
    #[test]
    fn no_implicitly_closed_name_changes_the_content_mode() {
        const KEYS: &[&str] = &[
            "li",
            "dd",
            "dt",
            "tr",
            "td",
            "th",
            "tbody",
            "tfoot",
            "thead",
            "option",
            "optgroup",
            "rp",
            "rt",
            "address",
            "article",
            "aside",
            "blockquote",
            "center",
            "details",
            "dialog",
            "dir",
            "div",
            "dl",
            "fieldset",
            "figcaption",
            "figure",
            "footer",
            "form",
            "h1",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "header",
            "hgroup",
            "hr",
            "listing",
            "main",
            "menu",
            "nav",
            "ol",
            "p",
            "pre",
            "search",
            "section",
            "summary",
            "table",
            "ul",
            "xmp",
        ];
        let mut popped_any = false;
        for key in KEYS {
            let popped = implicitly_closes(key);
            assert!(
                !popped.is_empty(),
                "`{key}` is listed here as a closer but pops nothing"
            );
            for name in popped {
                popped_any = true;
                // Closed under the table, so KEYS really does reach every
                // name the walk can pop implicitly.
                assert!(
                    KEYS.contains(name),
                    "`{name}` is popped implicitly but is not in KEYS — this list \
                     no longer covers the table"
                );
                for parent in [ContentMode::Html, ContentMode::Svg, ContentMode::MathMl] {
                    assert_eq!(
                        child_content_mode(parent, name, "", (0, 0)),
                        parent,
                        "`{name}` is closed implicitly AND changes the content mode"
                    );
                }
            }
        }
        assert!(popped_any, "the table popped nothing at all");
    }

    /// ti `48f3c6`. `<svg><link>` is a genuine, closable SVG element — foreign
    /// content has no void elements — so the author's `</link>` is a real
    /// closer. `is_void` winning globally made it an orphan, and the balancer
    /// DELETED it from the pane: an edit to what the reader sees, in the one
    /// direction the balancer exists to prevent.
    #[test]
    fn a_void_name_used_as_a_real_foreign_element_keeps_its_closer() {
        let html = "<svg><link>a</link>b</svg>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["svg", "link"]);
        assert!(
            ex[1].close.is_some(),
            "the author's </link> closes the link"
        );
        assert_eq!(balance_fragment(html), html);
        // And the other half: an unclosed one is now owed a closer.
        assert_eq!(
            balance_fragment("<svg><link>a"),
            "<svg><link>a</link></svg>"
        );
    }

    /// The property the fix rests on, pinned rather than its example.
    ///
    /// The trade recorded on `48f3c6` was that pushing void names inside
    /// foreign content risks appending `</br>`, which HTML's end-tag-`br`
    /// rule turns back into a fresh `<br>` — the balancer MINTING structure
    /// instead of repairing it, which is worse than the bug being fixed. No
    /// void name can reach that case: the five that are also breakout tags
    /// leave foreign content before they are processed, and `br` is one of
    /// them. DCR-0041's breakout model is what retired the hazard, so this
    /// test fails if anyone removes a name from `FOREIGN_BREAKOUT_TAGS`.
    #[test]
    fn a_void_name_that_is_also_a_breakout_tag_never_opens_inside_foreign_content() {
        let both: Vec<&str> = VOID_ELEMENTS
            .iter()
            .copied()
            .filter(|n| breaks_out_of_foreign(n, "", (0, 0)))
            .collect();
        assert_eq!(
            both,
            vec!["br", "embed", "hr", "img", "meta"],
            "the void names that can never be foreign elements moved — \
             re-check the `</br>` hazard before accepting this"
        );
        for name in both {
            let html = format!("<svg><{name}>x");
            assert_eq!(
                balance_fragment(&html),
                html,
                "`{name}` broke out of foreign content, so it stays void and \
                 earns no closer"
            );
        }
    }

    /// Voidness is untouched in HTML content, which is where every consumer
    /// of this crate spends nearly all of its time.
    #[test]
    fn voidness_still_wins_in_html_content() {
        assert_eq!(balance_fragment("<div><link>a"), "<div><link>a</div>");
        assert!(
            element_extents("<p><br>x</p>")
                .iter()
                .all(|e| e.name != "br")
        );
    }

    /// ti `4882ac`. `image`'s absence from `VOID_ELEMENTS` was a documented
    /// decision that nothing pinned — no unit test, no corpus entry, no
    /// scenario. A contributor comparing the list against HTML's void
    /// elements finds `image` missing, sees nothing saying why, and adds it.
    /// This is the falsifiable form of the decision.
    ///
    /// The reason is the tree builder's, not the syntax list's: HTML has no
    /// void element called `image`. "A start tag whose tag name is `image`"
    /// is handled by REWRITING the token's name to `img` and reprocessing, so
    /// `<image>x` leaves `x` a sibling because `img` is void — measured in
    /// headless Chromium. This crate performs no such rewrite, so the literal
    /// name is not void here.
    #[test]
    fn image_is_not_void_because_html_renames_it_rather_than_voiding_it() {
        assert!(
            !is_void("image"),
            "`image` is not on HTML's void list — `img`, the name the parser \
             rewrites it to, is"
        );
        assert!(is_void("img"));
        assert!(!VOID_ELEMENTS.contains(&"image"));
    }

    /// ti `e923ef`. `<image>x` is an empty `img` with `x` as its SIBLING,
    /// because HTML's "in body" insertion mode rewrites the token's name to
    /// `img` and reprocesses it. The voidness follows the rename, not the name
    /// the author typed — which is why `image` stays out of `VOID_ELEMENTS`
    /// (ti `4882ac`) and this is still right.
    ///
    /// Before the rename the element OPENED here and `x` read as its content,
    /// so `element_extents` answered `image` where a browser says nothing at
    /// all. It was inert for both shipped consumers — `tag_inventory` compared
    /// source against splice and both spelled it `image`, and the appended
    /// `</image>` was an end tag a browser matches to nothing — and it would
    /// have mis-parented content for wave 3's intake, which asks this walk
    /// "what element surrounds these bytes".
    #[test]
    fn an_image_start_tag_is_renamed_to_img_and_is_therefore_void() {
        assert!(
            element_extents("<image>x").is_empty(),
            "an empty img mints no extent"
        );
        assert_eq!(tag_inventory("<image>x"), vec!["img"]);
        // The author's bytes are untouched: the rename is in the TOKEN, so the
        // balancer simply owes nothing.
        assert_eq!(balance_fragment("<image>x"), "<image>x");
        // And `img` itself is unchanged by any of this.
        assert_eq!(tag_inventory("<img>x"), vec!["img"]);
    }

    /// The other half, and the fourth time this crate has drawn the same line:
    /// the substitution is an HTML-CONTENT rule. In foreign content "any other
    /// start tag" inserts a foreign element, and SVG has a real `<image>`, so
    /// the author's name and their closer both survive there.
    #[test]
    fn a_foreign_image_keeps_its_own_name_and_its_closer() {
        let svg = "<svg><image>a</image>b</svg>";
        assert_eq!(
            tag_inventory(svg),
            vec!["svg", "image", "/image", "/svg"],
            "svg:image is a real element, not a renamed img"
        );
        assert_eq!(balance_fragment(svg), svg);
        // A breakout tag returns us to HTML content, where the rename applies
        // again — the stack decides, not the nearest `<svg>`.
        assert_eq!(
            tag_inventory("<svg><div><image>x"),
            vec!["svg", "div", "img"]
        );
    }
}

// Spec 2026-08-20 §8, OI-0035 route (c), render half.
#[cfg(test)]
mod strip_tests {
    use super::*;

    #[test]
    fn every_reserved_attribute_is_removed_case_insensitively() {
        let html = "<div data-sync-id=\"p-0001\" DATA-Block-Kind='paragraph' data-order=3 \
                    data-fallback=\"none\" data-parent-id=\"x\" data-skipped=\"html-block\">t</div>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div>t</div>");
    }

    #[test]
    fn non_reserved_attributes_and_content_survive_untouched() {
        let html = "<a href=\"https://example.com/?a=1&amp;b=2\" data-sync-id=\"p-0001\" \
                    title=\"data-sync-id\">data-sync-id</a>";
        assert_eq!(
            strip_reserved_sync_attrs(html),
            "<a href=\"https://example.com/?a=1&amp;b=2\" title=\"data-sync-id\">data-sync-id</a>",
            "only NAMES are matched: the value and the text are content"
        );
    }

    #[test]
    fn markup_with_nothing_reserved_is_borrowed_not_rebuilt() {
        let html = "<p class=\"x\">hello</p>";
        assert!(matches!(strip_reserved_sync_attrs(html), Cow::Borrowed(_)));
        assert_eq!(strip_reserved_sync_attrs(html), html);
    }

    #[test]
    fn a_reserved_attribute_inside_rcdata_or_a_comment_is_text_not_markup() {
        // `scan_tags` never tokenizes RCDATA or comment content, so the strip
        // cannot reach in and silently edit what the reader sees — the exact
        // failure R0002-0020 was.
        let textarea = "<textarea><div data-sync-id=\"p-0001\"></textarea>";
        assert_eq!(strip_reserved_sync_attrs(textarea), textarea);
        let comment = "<!-- <div data-sync-id=\"p-0001\"> -->";
        assert_eq!(strip_reserved_sync_attrs(comment), comment);
    }

    /// The other side of that rule, and the reason the qualifier in
    /// `is_raw_text`'s name-only contract matters (ti `2e2453`). Inside
    /// foreign content `<title>` is an ordinary foreign element, so a browser
    /// reads this `data-sync-id` as a LIVE attribute on a real `div`. The
    /// scanner used to read the same bytes as RCDATA text, so the strip
    /// walked past them and left a planted anchor standing in a pane whose
    /// whole premise is that ours are the only sync attributes in it.
    ///
    /// Same shape as ti `e20490`, one region out: a divergence about where a
    /// tokenizer state begins, cashed out as a surviving anchor.
    #[test]
    fn a_reserved_attribute_inside_an_svg_title_is_markup_and_is_stripped() {
        assert_eq!(
            strip_reserved_sync_attrs(
                "<svg><title><div data-sync-id=\"p-0001\">x</div></title></svg>"
            ),
            "<svg><title><div>x</div></title></svg>"
        );
        // The HTML-content spelling is unchanged: still RCDATA, still text.
        let html_title = "<title><div data-sync-id=\"p-0001\"></title>";
        assert_eq!(strip_reserved_sync_attrs(html_title), html_title);
    }

    #[test]
    fn self_closing_and_close_tags_are_handled() {
        assert_eq!(
            strip_reserved_sync_attrs("<img data-sync-id=\"i-1\" src=\"a.png\"/>"),
            "<img src=\"a.png\"/>"
        );
        // A close tag carries no attributes: nothing to do, nothing to break.
        assert_eq!(strip_reserved_sync_attrs("</div>"), "</div>");
    }

    #[test]
    fn stripping_never_changes_the_tag_inventory() {
        // Layer 3's skeleton check must see the same thing before and after.
        // The guarantee is not free, and it is not "because attributes are
        // not structure": the CUT carries the token separation an attribute
        // stood in for, so it is the seam rule below that makes the sentence
        // true (ti 490d97 wave 1).
        let html = "<div data-sync-id=\"p-0001\"><b data-order=\"2\">x</b></div>";
        let stripped = strip_reserved_sync_attrs(html);
        assert_eq!(tag_inventory(&stripped), tag_inventory(html));
        assert_eq!(stripped, "<div><b>x</b></div>");

        // The weld: a misplaced closing quote leaves a NAME byte as the first
        // survivor after the cut. Deleting the separator moved the inventory
        // from ["div"] to ["divc"] — a browser names the welded element
        // `divc"` (its tag-name state eats the quote). That used to be a
        // one-byte remainder — DCR-0032's 2026-08-21 amendment filed it as
        // ti e20490 — and is not one now: `tag_name_end` eats the quote too,
        // so the scanner names the element exactly as a browser does. Moving
        // off ["div"] at all is the defect this guards.
        for weld in [
            "<div data-sync-id=\"a b=\"c\">x",
            "<div data-order=\"a b=\"c\">x",
        ] {
            let stripped = strip_reserved_sync_attrs(weld);
            assert_eq!(
                tag_inventory(&stripped),
                tag_inventory(weld),
                "the strip welded bytes: {stripped}"
            );
            assert_eq!(tag_inventory(&stripped), vec!["div"]);
        }

        // The residue: a reserved attribute sitting directly behind a stray
        // `/` (the shape wave 1's own call site generates). Deleting it welds
        // `/` onto `>` and SETS a flag the source never carried.
        for residue in [
            "<div/data-sync-id=\"p-0003\">x</div>",
            "<svg/data-sync-id=\"x\">y</svg>",
        ] {
            let stripped = strip_reserved_sync_attrs(residue);
            assert_eq!(
                tag_inventory(&stripped),
                tag_inventory(residue),
                "the strip minted a self-closing flag: {stripped}"
            );
        }
    }

    /// ti 490d97 wave 1, measured in headless Chromium. The cut took the
    /// removed attribute's leading whitespace UNCONDITIONALLY, so when the
    /// next surviving byte was a name byte it welded onto the tag name:
    /// `<div data-sync-id="a b="c">x` became `<divc">x`. One U+0020 in the
    /// cut's place reproduces the pre-strip parse — element `div`, junk
    /// attribute `c"` — exactly.
    #[test]
    fn a_cut_that_would_weld_bytes_leaves_one_space() {
        assert_eq!(
            strip_reserved_sync_attrs("<div data-sync-id=\"a b=\"c\">x"),
            "<div c\">x"
        );
        // One space, never two: the run is replaced, not padded.
        assert_eq!(
            strip_reserved_sync_attrs("<div  data-sync-id=\"a b=\"c\">x"),
            "<div c\">x"
        );
    }

    /// ti `e20490` divergence 1, and the reason it had a security shape: the
    /// plant is a REAL attribute to a browser, so the strip must see it as a
    /// name and cut it.
    ///
    /// HTML's tag-name state consumes bytes up to whitespace, `/` or `>` into
    /// the element name, so `<divq"x=" data-sync-id="v">` is an element named
    /// `divq"x="` carrying a live `data-sync-id`. The scanner used to end the
    /// name at `divq` and attribute-walk the rest, which buried the plant
    /// inside a phantom quoted value that `collect_reserved_attr_spans` never
    /// examines as a name — so the strip left the anchor standing.
    ///
    /// The pane path survived that only because DOMPurify drops an element
    /// whose name carries a byte no allowlisted name has. This test does not
    /// depend on the sanitizer, because a consumer that does not mount
    /// through one does not inherit it.
    #[test]
    fn a_divergent_tag_name_no_longer_hides_a_reserved_attribute_from_the_strip() {
        let planted = "<divq\"x=\" data-sync-id=\"v\">z";
        let stripped = strip_reserved_sync_attrs(planted);
        assert!(
            !stripped.contains("data-sync-id"),
            "the plant is a real attribute to a browser and must be cut: {stripped}"
        );
        // The element name itself is not the strip's business — only the
        // reserved attribute moves.
        assert!(
            stripped.contains("divq\"x=\""),
            "the element name must survive verbatim: {stripped}"
        );
    }

    /// ti `e20490` divergence 2: `</` before a non-letter is a parse error
    /// opening a bogus comment to the first `>`, so a browser mints ZERO
    /// elements. The scanner used to fall through to plain text and keep
    /// tokenizing, which invented an element and earned it a closer the
    /// browser reads as orphan junk.
    #[test]
    fn an_end_tag_before_a_non_letter_mints_no_element() {
        for src in ["</1 <div>x", "</ <div>x", "</= <div>x"] {
            assert!(
                tag_inventory(src).is_empty(),
                "a bogus comment mints no element: {src} -> {:?}",
                tag_inventory(src)
            );
            assert_eq!(
                balance_fragment(src),
                src,
                "and so it is owed no closer: {src}"
            );
        }
        // `</>` is discarded whole (missing-end-tag-name), not turned into a
        // comment — so it too mints nothing, by a different spec rule.
        assert!(tag_inventory("</>x").is_empty());
        assert_eq!(balance_fragment("</>x"), "</>x");
        // The control: a real end tag still closes its element.
        assert_eq!(tag_inventory("<div>x</div>"), vec!["div", "/div"]);
    }

    /// ti `415cdb`: a `=` where an attribute NAME would start is a parse error
    /// that begins a name with that `=`, not a separator to step over. So
    /// Chromium reads `<div =data-sync-id="x">` as one junk attribute named
    /// `=data-sync-id` — there is no reserved attribute present and nothing
    /// to strip.
    ///
    /// This walk used to restart the name after the `=`, find a
    /// `data-sync-id` no browser sees, and cut it — leaving `<div =>y`. That
    /// mutates a NON-reserved attribute, which is exactly what the strip's
    /// narrowed guarantee says never happens.
    ///
    /// The direction was always over-deletion: reviewer A's 51-case browser
    /// equivalence probe found this one case and zero under-deletions, so no
    /// live `data-sync-id` ever survived the strip and the impostor-anchor
    /// property held throughout.
    #[test]
    fn a_stray_equals_begins_an_attribute_name_instead_of_being_skipped() {
        for src in [
            "<div =data-sync-id=\"x\">y",
            // The same rule with the reserved name spelled second.
            "<div =data-order=\"3\">y",
        ] {
            assert_eq!(
                strip_reserved_sync_attrs(src),
                src,
                "no reserved attribute is present, so nothing may be cut: {src}"
            );
        }

        // The rule must not blind the strip to a real one sitting beside the
        // junk attribute — over-correction here would be an UNDER-deletion,
        // which is the direction that actually breaks the anchor property.
        assert_eq!(
            strip_reserved_sync_attrs("<div =junk data-sync-id=\"x\">y"),
            "<div =junk>y"
        );
    }

    /// ti 490d97 wave 1. Two seams that are not welds:
    ///
    /// - a cut whose run ends at `>` with a `/` in front of it would create a
    ///   `/>` adjacency the input never had. Measured, that changes the tree
    ///   on a foreign root: `<svg/>y</svg>` is an EMPTY svg with `y` outside
    ///   it, while `<svg/ >y</svg>` is an open svg containing `y`.
    /// - adjacent cuts must be coalesced first. Judging per-cut reads bytes
    ///   inside the neighbouring cut and produces `<div/>x`, the wrong tree.
    #[test]
    fn a_cut_that_would_mint_a_self_closing_flag_leaves_one_space() {
        assert_eq!(
            strip_reserved_sync_attrs("<div/data-sync-id=\"p-0003\">x</div>"),
            "<div/ >x</div>"
        );
        assert_eq!(
            strip_reserved_sync_attrs("<svg/data-sync-id=\"x\">y</svg>"),
            "<svg/ >y</svg>"
        );
        assert_eq!(
            strip_reserved_sync_attrs("<div/data-sync-id=\"a\" data-order=\"b\">x"),
            "<div/ >x",
            "the cuts merge into one run before the seam is judged"
        );
    }

    /// The other half of the seam rule, and the one that has to stay
    /// byte-exact: a well-formed strip emits NO space. Every existing
    /// expectation in this module depends on it, and a rule that padded
    /// unconditionally would move all of them.
    #[test]
    fn a_well_formed_strip_leaves_no_space_behind() {
        assert_eq!(
            strip_reserved_sync_attrs("<div data-sync-id=\"x\">t</div>"),
            "<div>t</div>"
        );
        // Run ends at whitespace: the survivor already has its separator.
        assert_eq!(
            strip_reserved_sync_attrs("<div data-sync-id=\"x\" class=\"a\">t"),
            "<div class=\"a\">t"
        );
        // Run ends at `/` with no `/` in front: the flag was already there.
        assert_eq!(
            strip_reserved_sync_attrs("<img data-sync-id=\"x\"/>"),
            "<img/>"
        );
    }

    /// ti 549b20, measured: DOMPurify 3.2.6 (the vendored copy) sanitizes
    /// this to `<div> <p data-sync-id="v">x</p></div>` — a live P#v anchor —
    /// while the pre-fix scanner saw zero tokens and stripped nothing.
    #[test]
    fn a_stray_quote_cannot_hide_a_planted_sync_attr() {
        let html = "<div \"> <p data-sync-id=\"v\">x</p>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div \"> <p>x</p>");
    }

    /// ti 549b20, measured: DOMPurify keeps this plant as a live DIV#v. The
    /// stray quote after the closed value is a name byte, so the scanner now
    /// reads the tag the way the browser does and the strip reaches the
    /// plant.
    #[test]
    fn a_doubled_quote_cannot_hide_a_planted_sync_attr() {
        let html = "<div a=\"x\"\" data-sync-id=\"v\">y</div>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div a=\"x\"\">y</div>");
    }

    /// ti 549b20, measured in headless Chromium: the browser makes one
    /// attribute named `="`, ends the div at the FIRST `>`, and paints
    /// ` data-sync-id="v">x` as TEXT — live_sync_ids is empty. The strip
    /// must never delete bytes a browser renders. This test is vacuously
    /// green against the wholly-broken scanner (zero tokens, borrow); its
    /// red arrives at the task's midpoint, where fixing only the quote and
    /// unterminated-tag divergences turns the bypass into a DELETION — a
    /// content mutation, not a bypass — which is exactly why the stray-`=`
    /// arm is fixed in the same task.
    #[test]
    fn text_the_browser_paints_is_never_cut_by_the_strip() {
        let html = "<div =\"> data-sync-id=\"v\">x</div>";
        assert_eq!(strip_reserved_sync_attrs(html), html);
    }
}

// Spec §3.3: identity-skip splice, entity-drift acceptance, type-conditional
// blank-line collapse. Spec §3.4: render-side auto-balancing.
#[cfg(test)]
mod splice_tests {
    use super::*;

    #[test]
    fn identity_segments_splice_byte_identical_including_entities() {
        // &nbsp; and &copy; would NOT survive a naive decode/re-escape
        // round-trip; the identity skip preserves them byte-exactly.
        let block = "<p>Caf&eacute; &copy; 2026&nbsp;&mdash; <b>bold &amp; true</b></p>";
        let segs = extract(block).expect("extracts");
        let out = splice(
            block,
            &segs.texts,
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(out, block, "identity splice must be byte-identical");
    }

    #[test]
    fn translated_segment_is_replaced_and_specials_are_escaped() {
        let block = "<summary>Click to expand</summary>";
        let out = splice(
            block,
            &["펼치기 <&>".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(out, "<summary>펼치기 &lt;&amp;&gt;</summary>");
    }

    #[test]
    fn entity_drift_for_translated_segments_is_pinned() {
        // Spec §6 entities row: genuinely translated segments re-escape only
        // < > & — a translated segment loses named-entity forms.
        let block = "<p>one&nbsp;two</p>";
        let out = splice(
            block,
            &["eins\u{a0}zwei".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(out, "<p>eins\u{a0}zwei</p>");
    }

    #[test]
    fn blank_lines_collapse_for_type_6_but_not_type_1() {
        let block = "<div>text</div>";
        let translated = vec!["line1\n\nline2".to_string()];
        let out6 = splice(
            block,
            &translated,
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert!(
            !out6.contains("\n\n"),
            "type 6 must collapse blank lines: {out6}"
        );

        let pre = "<pre>a\n\nb</pre>";
        let pre_segs = extract(pre).expect("extracts");
        let out1 = splice(
            pre,
            &pre_segs.texts,
            BlankLinePolicy::from_commonmark_html_block_type(1),
        )
        .expect("splices");
        assert_eq!(out1, pre, "type 1 keeps interior blank lines");
    }

    #[test]
    fn type_1_keeps_blank_lines_inside_a_translated_segment() {
        // The identity leg above cannot see the collapse gate at all —
        // identity-skip short-circuits first — so a gate that wrongly read
        // `1 | 6 | 7` would still pass it. A NON-identity translation is what
        // actually pins the type-1 exemption (spec §3.3).
        let pre = "<pre>a\n\nb</pre>";
        let out = splice(
            pre,
            &["가\n\n나".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(1),
        )
        .expect("splices");
        assert_eq!(out, "<pre>가\n\n나</pre>");
    }

    #[test]
    fn whitespace_only_line_counts_as_blank_for_collapse() {
        let block = "<div>text</div>";
        let out = splice(
            block,
            &["a\n \nb".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert!(
            !out.contains("\n \n"),
            "whitespace-only line must collapse: {out:?}"
        );
    }

    /// R0010-0053. The collapse existed to keep a blank line out of a type-6/7
    /// html block, and comrak counts `\r\n` and a lone `\r` as line endings
    /// too (OI-0033) — so a segment that separated its lines the classic-Mac
    /// or Windows way used to keep the separator that splits the block and
    /// breaks its anchor. Terminators are re-emitted verbatim: collapsing
    /// blank lines is not a licence to rewrite the segment's line endings.
    #[test]
    fn collapse_honours_cr_and_crlf_line_endings() {
        // The LF spelling is unchanged, byte for byte.
        assert_eq!(collapse_blank_lines("a\n\nb"), "a\nb");

        assert_eq!(collapse_blank_lines("a\r\n\r\nb"), "a\r\nb");
        assert_eq!(collapse_blank_lines("a\r\rb"), "a\rb");
        // Whitespace-only counts as blank under every terminator.
        assert_eq!(collapse_blank_lines("a\r\n \r\nb"), "a\r\nb");
        assert_eq!(collapse_blank_lines("a\r \rb"), "a\rb");
        // Mixed terminators: the `\r` of a CRLF is not a boundary of its own.
        assert_eq!(collapse_blank_lines("a\r\n\rb"), "a\r\nb");
        // Leading and trailing blank lines survive, as they always have.
        assert_eq!(collapse_blank_lines("\r\na\r\n"), "\r\na\r\n");

        let out = splice(
            "<div>text</div>",
            &["a\r\n\r\nb".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert!(
            !out.contains("\r\n\r\n"),
            "a CRLF blank line must collapse: {out:?}"
        );
    }

    #[test]
    fn wrong_segment_count_is_an_error() {
        let block = "<p>one</p><p>two</p>";
        let err = splice(
            block,
            &["only-one".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .unwrap_err();
        assert!(err.contains("segment count"), "got: {err}");
    }

    #[test]
    fn dropped_nodes_agree_between_extract_and_splice_passes() {
        // The &nbsp;-only node is dropped by BOTH passes (same scan()), so
        // one translated segment maps to the kept node — never off-by-one.
        let block = "<p>&nbsp;</p><p>kept</p>";
        let out = splice(
            block,
            &["유지".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(out, "<p>&nbsp;</p><p>유지</p>");
    }

    #[test]
    fn template_bearing_block_splices_byte_identical() {
        // Template text is `kept: false` in BOTH passes (one shared scan), so
        // the suppressed node passes through byte-verbatim and the one real
        // segment still lands on the right node (spec §9).
        let block = "<div>before<template><p>inside &amp; hidden</p></template>after</div>";
        let out = splice(
            block,
            &["before".to_string(), "after".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(out, block, "identity splice must be byte-identical");

        let translated = splice(
            block,
            &["앞".to_string(), "뒤".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(
            translated, "<div>앞<template><p>inside &amp; hidden</p></template>뒤</div>",
            "template content is untouched while its siblings translate"
        );
    }

    #[test]
    fn replacement_lands_on_the_right_node_past_dropped_and_filtered_nodes() {
        // The load-bearing pass-agreement test: only the LAST segment is
        // translated, so a phase-A/phase-B counter divergence over the
        // comment, the ScriptData node, the &nbsp;-only node or the
        // whitespace-only node between </b> and <i> would land the
        // replacement on the wrong text node instead of being invisible.
        let block = "<div><!-- c --><script>a < b;</script><p>&nbsp;</p>\
                     <b>keep&amp;me</b> <i>target</i></div>";
        let segs = extract(block).expect("extracts");
        assert_eq!(
            segs.texts,
            vec!["keep&me".to_string(), "target".to_string()]
        );
        let mut translated = segs.texts.clone();
        *translated.last_mut().expect("two segments") = "대상".to_string();
        let out = splice(
            block,
            &translated,
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(
            out,
            "<div><!-- c --><script>a < b;</script><p>&nbsp;</p>\
             <b>keep&amp;me</b> <i>대상</i></div>"
        );
    }
}

// Spec §3.4: auto-balancing exists only on the render path.
#[cfg(test)]
mod balance_tests {
    use super::*;

    #[test]
    fn unclosed_div_is_closed_at_fragment_end() {
        assert_eq!(
            balance_fragment("<div align=\"center\">\n<b>Hero</b>"),
            "<div align=\"center\">\n<b>Hero</b></div>"
        );
    }

    #[test]
    fn orphan_close_tag_is_dropped() {
        assert_eq!(balance_fragment("</details>"), "");
        assert_eq!(balance_fragment("tail</div>text"), "tailtext");
    }

    /// OI-0046: the seam rule, on the balancer's side of it. Same shape as
    /// `strip_tests::a_cut_that_would_weld_bytes_leaves_one_space`, and the
    /// four inputs are the four harms `generative_properties.rs::
    /// deleting_an_orphan_no_longer_welds_a_literal_angle_bracket` measures
    /// end to end.
    #[test]
    fn an_orphan_cut_that_would_weld_bytes_leaves_one_space() {
        // A literal `<` welded onto a name byte minted a tag.
        assert_eq!(balance_fragment("<</b>x"), "< x");
        // Onto `!` it minted a comment that swallows the rest of the pane.
        assert_eq!(
            balance_fragment("a<</b>!-- swallow everything"),
            "a< !-- swallow everything"
        );
        // Onto `/` it minted an end tag.
        assert_eq!(balance_fragment("a<</b>/div>tail"), "a< /div>tail");
        // Onto `?` it minted a bogus comment.
        assert_eq!(balance_fragment("a<</b>?pi>tail"), "a< ?pi>tail");
        // One space, never two: adjacent orphans are one coalesced run, and
        // the run is replaced rather than padded per cut.
        assert_eq!(balance_fragment("<</b></i>x"), "< x");
        // `&` is the other byte whose meaning depends on what follows it. No
        // tag comes of it, but the reader's text changes: the source shows
        // six characters (the `&` is literal, since `&<` names no reference)
        // and the welded reference would show two.
        assert_eq!(balance_fragment("a&</b>amp;"), "a& amp;");
    }

    /// The other half of the seam rule: it fires only where deleting really
    /// would change a token, so an ordinary orphan is still removed
    /// byte-exactly. Without this the rule would pad every fragment with
    /// spaces the author never wrote.
    #[test]
    fn an_orphan_cut_that_welds_nothing_is_still_deleted_byte_exactly() {
        for (src, want) in [
            ("tail</div>text", "tailtext"),
            // `<` before the cut, but nothing markup-shaped after it.
            ("<</b> x", "< x"),
            ("<</b><div>y", "<<div>y</div>"),
            ("<</b>", "<"),
            ("a&</b> b", "a& b"),
            ("a&</b>-b", "a&-b"),
            // The cut is not adjacent to the literal `<` at all.
            ("< x</b>y", "< xy"),
        ] {
            assert_eq!(balance_fragment(src), want, "input {src:?}");
        }
    }

    /// The separator and ti `c1f9a8`'s trailing repair in one fragment, which
    /// is where the arithmetic could go wrong: that repair maps an `html`
    /// offset onto the output, so the offset has to be shifted by the cut's
    /// NET size — four bytes deleted minus one space put back — rather than
    /// by the bytes the cut removed.
    #[test]
    fn a_separator_shifts_the_trailing_repair_by_the_net_cut() {
        // `<` literal, `</b>` orphan, `div ` text, `<p` a tag cut off at EOF.
        // The orphan run is replaced by one space and the abandoned tag is
        // truncated away; a shift of 4 instead of 3 would eat the `v ` too.
        assert_eq!(balance_fragment("<</b>div <p"), "< div ");
        // The truncation itself never welds: it removes a suffix, and the `<`
        // it leaves at the end is a literal character exactly as it was.
        assert_eq!(balance_fragment("a<<p"), "a<");
        // Nor does an appended closer, because it begins with `<` and `<<`
        // leaves the first one literal.
        assert_eq!(balance_fragment("<div>x<"), "<div>x<</div>");
        // The same seam with a terminator-bearing region instead: the repair
        // appends rather than truncates, so it does not read the shift, but
        // the separator must still be there.
        assert_eq!(balance_fragment("<</b>div <!--x"), "< div <!--x-->");
    }

    #[test]
    fn balanced_fragment_is_untouched() {
        let html = "<details><summary>ok</summary></details>";
        assert_eq!(balance_fragment(html), html);
    }

    /// DCR-0016 Part D: HTML ignores `/` on raw-text/RCDATA start tags, so a
    /// self-closing `<textarea/>` opens RCDATA and swallows every later
    /// sibling — including the sync wrapper's `</div>`. DOMPurify keeps
    /// `<textarea>`, so the pane tail would collapse into a form control.
    /// The load-bearing assertion is that a close tag is appended.
    #[test]
    fn self_closing_raw_text_elements_still_get_a_close_tag() {
        assert_eq!(balance_fragment("<textarea/>"), "<textarea/></textarea>");
        assert_eq!(balance_fragment("<script/>"), "<script/></script>");
        assert_eq!(balance_fragment("<style/>"), "<style/></style>");
        assert_eq!(balance_fragment("<title/>"), "<title/></title>");
    }

    #[test]
    fn balanced_raw_text_elements_are_untouched() {
        let html = "<textarea>x</textarea>";
        assert_eq!(balance_fragment(html), html);
        let script = "<script>var a = 1;</script>";
        assert_eq!(balance_fragment(script), script);
    }

    /// ti 490d97 wave 1. This test used to be called
    /// `self_closing_non_raw_text_elements_stay_closed` and asserted that
    /// `<span/>after` gets no appended close tag. Its premise WAS the defect:
    /// HTML honours the self-closing flag in exactly two places — foreign
    /// content and the `<svg>`/`<math>` start tags — and everywhere else the
    /// `/` is a parse error the parser ignores. Measured in headless
    /// Chromium, `<span/>after` parses to `<span>after</span>`, so the
    /// balancer owes it a closer. The `<br/>` arm is unchanged: void wins
    /// over everything.
    #[test]
    fn only_void_and_foreign_tags_are_closed_by_their_own_slash() {
        assert_eq!(balance_fragment("<br/>after"), "<br/>after");
        assert_eq!(balance_fragment("<span/>after"), "<span/>after</span>");
        // Foreign content and the foreign roots keep the XML reading, because
        // there HTML really does.
        assert_eq!(
            balance_fragment("<svg><rect/><circle/></svg>"),
            "<svg><rect/><circle/></svg>"
        );
        assert_eq!(balance_fragment("<svg/>after"), "<svg/>after");
        assert_eq!(balance_fragment("<math/>after"), "<math/>after");
    }

    /// ti `95f55b`, then ti `c1f9a8`. A fragment can end INSIDE an
    /// unterminated comment, CDATA section, bogus comment or tag — an author
    /// can close an HTML block mid-`<!--`, and invariant 7 says that input is
    /// expected rather than exceptional.
    ///
    /// `95f55b` stopped the balancer appending a closer the region would
    /// swallow, because a buried closer closes nothing and each re-balance
    /// added another (15,726 violations in 200,000 fuzz iterations). That made
    /// the function idempotent and left the fragment unrepaired.
    ///
    /// `c1f9a8` repairs it: the region is TERMINATED where a browser
    /// terminates it, or DELETED where a browser abandons it, and only then
    /// are closers appended. The idempotence property is unchanged and is
    /// still asserted by balancing twice — it is now achieved by fixing the
    /// fragment rather than by declining to.
    ///
    /// Why it matters more than the trailing bytes looking odd: only an
    /// unterminated COMMENT swallows everything after it. The other kinds end
    /// at the first `>`, which in a mounted pane is the sync wrapper's own
    /// `</div>` — so the wrapper closed inside the region, stayed open, and
    /// every following block nested inside this one's wrapper. That is
    /// `contracts.md` §4a's direct-child break.
    #[test]
    fn an_unterminated_trailing_region_is_repaired_before_closers_are_appended() {
        for (src, want) in [
            // Terminated where a browser terminates it, then the div closes.
            ("<div>x<!--", "<div>x<!----></div>"),
            ("<div>x<?pi", "<div>x<?pi></div>"),
            ("<div>x<![CDATA[y", "<div>x<![CDATA[y></div>"),
            // A tag cut at EOF is ABANDONED by a browser — no element, no
            // attributes — so the faithful repair is deletion, not `>`.
            ("<div>x<p", "<div>x</div>"),
            // Reachable only since wave 1, because the walk now pushes a
            // flagged non-void tag.
            ("<div/>x<!--", "<div/>x<!----></div>"),
            ("<span/>t<!--", "<span/>t<!----></span>"),
        ] {
            let once = balance_fragment(src);
            assert_eq!(once, want, "repair for: {src}");
            assert_eq!(
                balance_fragment(&once),
                once,
                "and balancing is stable under its own re-scan: {src}"
            );
        }
    }

    /// The repair is confined to the LAST token. A terminated region in the
    /// middle of a fragment is untouched, and a fragment with no unterminated
    /// tail is byte-identical — which is what keeps this change invisible to
    /// every shipped fixture.
    #[test]
    fn a_terminated_region_is_never_touched() {
        for src in [
            "<div><!-- c -->x</div>",
            "<div>x<![CDATA[y]]></div>",
            "<!doctype html><p>a</p>",
            "<div><!-- a --><!-- b --></div>",
        ] {
            assert_eq!(balance_fragment(src), src, "unchanged: {src}");
        }
    }

    /// The other half of the same rule: an appended closer that ENDS the
    /// region it would otherwise be buried in is real markup and must still
    /// be appended. `</textarea>` terminates raw text, so it comes back
    /// visible to the scanner and stays — dropping it would leave the
    /// fragment able to consume the sync wrapper's own closer, which is the
    /// harm `balance_fragment` exists to prevent.
    #[test]
    fn a_closer_that_terminates_the_region_is_still_appended() {
        assert_eq!(
            balance_fragment("<div><textarea>x"),
            "<div><textarea>x</textarea></div>"
        );
        let once = balance_fragment("<div><textarea>x");
        assert_eq!(balance_fragment(&once), once, "and it is idempotent");
    }

    /// ti 490d97 wave 1, the defect in one line: the author's closing tag.
    /// The walk refused to push a flagged non-void tag, so `</div>` matched
    /// nothing on the stack, became an orphan, and was DELETED — and on the
    /// pane path the still-open fragment then consumed the sync wrapper's own
    /// `</div>`, nesting the next block's anchor inside the html block's
    /// wrapper (contracts.md §4a wants it a direct child of `<main>`).
    #[test]
    fn a_flagged_non_void_tag_opens_and_keeps_its_own_closer() {
        let html = "<div class=\"a\">\n<div/>y</div>\n</div>";
        assert_eq!(
            balance_fragment(html),
            html,
            "both closers are real; neither is an orphan"
        );
        // Reachable with no strip at all: `<div/>` is a JSX habit, and it
        // tripped this from the day the walk existed.
        assert_eq!(balance_fragment("<div/>y</div>"), "<div/>y</div>");
        assert_eq!(balance_fragment("<span/>tail"), "<span/>tail</span>");
        // Implied closes compose with the new push: one appended `</p>`, not
        // two, and not zero.
        assert_eq!(balance_fragment("<p/>a"), "<p/>a</p>");
        assert_eq!(balance_fragment("<p/>a<p/>b"), "<p/>a<p/>b</p>");
    }

    /// R0009-0064: the two `pub` tag predicates answer HTML's question, and
    /// HTML tag names are ASCII-case-insensitive. In-crate every caller has
    /// already lowercased, so only an external caller can observe this — and
    /// for that caller a `false` on `BR` means pushing a void element onto an
    /// open-element stack, which poisons the parent label of every later text
    /// node. Non-tags stay `false` in every casing.
    #[test]
    fn the_tag_predicates_ignore_ascii_case() {
        for spelling in ["br", "BR", "Br"] {
            assert!(is_void(spelling), "{spelling} is a void element");
        }
        for spelling in ["script", "SCRIPT", "ScRiPt", "TEXTAREA", "Title"] {
            assert!(is_raw_text(spelling), "{spelling} holds raw text/RCDATA");
        }
        for spelling in ["div", "DIV", "", "b r", "brr"] {
            assert!(!is_void(spelling), "{spelling} is not void");
            assert!(!is_raw_text(spelling), "{spelling} is not raw text");
        }
    }

    /// Guards the ORDER of the push rule: `is_void` is checked first, so the
    /// four parser-voids `VOID_ELEMENTS` gained in ti 490d97
    /// wave 1 (`basefont`, `bgsound`, `frame`, `keygen`, all measured
    /// never-open in Chromium) mint no appended close. The flagged arms were
    /// green before that fix — landing the walk change WITHOUT the void
    /// extension turns each of them red, which is what they are here for. The
    /// unflagged arm was red before it: `<keygen>x` used to earn a
    /// `</keygen>` a browser never mints.
    #[test]
    fn the_parser_voids_earn_no_appended_close_for_a_flagged_spelling() {
        assert_eq!(balance_fragment("<keygen/>x"), "<keygen/>x");
        assert_eq!(balance_fragment("<basefont/>x"), "<basefont/>x");
        assert_eq!(balance_fragment("<bgsound/>x"), "<bgsound/>x");
        assert_eq!(balance_fragment("<frame/>x"), "<frame/>x");
        // Unflagged too — they were never open elements to begin with.
        assert_eq!(balance_fragment("<keygen>x"), "<keygen>x");
    }

    /// The composed pane chain, in the order `render.rs` calls it:
    /// `balance_fragment(&strip_reserved_sync_attrs(md))`. Each half is
    /// pinned on its own above; this is the seam between them, which is where
    /// the wave-1 defect actually reached a reader.
    #[test]
    fn the_pane_chain_survives_a_reserved_attribute_behind_a_slash() {
        let md = "<div/data-sync-id=\"p-0003\">x</div>";
        assert_eq!(
            balance_fragment(&strip_reserved_sync_attrs(md)),
            "<div/ >x</div>",
            "the strip must not mint a flag, and the walk must not drop the \
             author's closer"
        );
        // The foreign root is where minting one would change the TREE, not
        // just the extents.
        let svg = "<svg/data-sync-id=\"x\">y</svg>";
        assert_eq!(
            balance_fragment(&strip_reserved_sync_attrs(svg)),
            "<svg/ >y</svg>"
        );
    }

    #[test]
    fn void_elements_do_not_accumulate_open_tags() {
        let html = "<p>a<br>b<img src=\"x.png\"></p>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn comments_and_script_content_are_not_scanned_for_tags() {
        let html = "<!-- <div> --><script>if (a < b) { s = \"</div>\"; }</script>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn nested_unclosed_tags_close_in_reverse_order() {
        assert_eq!(
            balance_fragment("<div><span>x"),
            "<div><span>x</span></div>"
        );
    }

    #[test]
    fn non_ascii_raw_text_does_not_split_a_char_boundary() {
        // The raw-text close-tag probe compares bytes, not str slices: a
        // multibyte char straddling the probe window must not panic.
        let html = "<script>var s = \"日本語 < x\";</script>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn raw_text_exits_only_on_a_delimited_close_tag() {
        // `</scripty>` is not `</script>`. Without HTML's post-name delimiter
        // check the scanner leaves raw-text state early and balance_fragment
        // DELETES those bytes as an orphan close tag.
        let html = "<script>var s = \"</scripty>\";</script>";
        assert_eq!(balance_fragment(html), html);
        assert_eq!(tag_inventory(html), vec!["script", "/script"]);
    }

    #[test]
    fn unquoted_attribute_value_ending_in_slash_is_not_self_closing() {
        // Per HTML tokenization the trailing `/` is part of the unquoted
        // value. Misreading it as a self-closing marker leaves <a> off the
        // open stack, turns the real </a> into an "orphan", and DELETES it —
        // producing exactly the unclosed tag balance_fragment exists to stop.
        let html = "<a href=https://example.com/>Example</a>";
        assert_eq!(balance_fragment(html), html);
        assert_eq!(tag_inventory(html), vec!["a", "/a"]);
    }

    /// R0002-0020: `<textarea>` and `<title>` hold RCDATA — a browser never
    /// tokenizes their content as markup — so the tag-like bytes below are
    /// TEXT. Scanning them invented a `/p` that the balancer then DELETED
    /// from the pane as an orphan close tag, editing what the reader sees.
    #[test]
    fn rcdata_content_is_not_scanned_for_tags() {
        let textarea = "<textarea>Use </p> to close a paragraph</textarea>";
        assert_eq!(tag_inventory(textarea), vec!["textarea", "/textarea"]);
        assert_eq!(balance_fragment(textarea), textarea);

        let title = "<title>a < b </i> c</title>";
        assert_eq!(tag_inventory(title), vec!["title", "/title"]);
        assert_eq!(balance_fragment(title), title);

        // The close-tag probe is still delimited and still name-exact, so
        // RCDATA ends where a browser ends it and nowhere earlier.
        let nested = "<textarea></textareax> still text</textarea>";
        assert_eq!(tag_inventory(nested), vec!["textarea", "/textarea"]);
    }

    /// R0002-0060: a colonized name is scanned WHOLE. Reading `<div:x>` as a
    /// `div` made the balancer append a `</div>`, which at parse time pops
    /// the sync wrapper's own `<div>` — the rest of the block spills outside
    /// its anchor, the exact failure balancing exists to prevent.
    #[test]
    fn a_namespaced_tag_name_is_scanned_whole() {
        assert_eq!(tag_inventory("<div:x>y"), vec!["div:x"]);
        let balanced = balance_fragment("<div:x>y");
        assert_eq!(balanced, "<div:x>y</div:x>");
        assert!(
            !balanced.contains("</div>"),
            "an appended </div> would close the sync wrapper: {balanced}"
        );
        assert_eq!(
            tag_inventory("<svg:rect></svg:rect>"),
            vec!["svg:rect", "/svg:rect"]
        );
        // A leading colon is not a tag name (HTML's tag-open state wants a
        // letter), so this stays plain text — as it always did.
        assert!(tag_inventory("<:x>y").is_empty());
        assert_eq!(balance_fragment("<:x>y"), "<:x>y");
    }

    /// R0002-0061: HTML's optional end tags are generated implicitly by the
    /// start tag that ends them. Without that, every unclosed one piled onto
    /// the open stack and earned its own appended close — and a browser
    /// turns each surplus close into a phantom empty element that appears in
    /// the pane and in no source document.
    #[test]
    fn optional_end_tags_do_not_accumulate_phantom_closes() {
        // One appended `</p>`, not two.
        assert_eq!(balance_fragment("<p>a<p>b"), "<p>a<p>b</p>");
        // A block-level start tag closes an open paragraph too...
        assert_eq!(balance_fragment("<p>a<div>b</div>"), "<p>a<div>b</div>");
        // ...including a void one, which closes without ever being pushed.
        assert_eq!(balance_fragment("<p>a<hr>"), "<p>a<hr>");
        // `<br>` is void and does NOT close a paragraph.
        assert_eq!(balance_fragment("<p>a<br>b"), "<p>a<br>b</p>");
        // List items, table rows/cells, definition lists, select options.
        assert_eq!(
            balance_fragment("<ul><li>a<li>b"),
            "<ul><li>a<li>b</li></ul>"
        );
        assert_eq!(
            balance_fragment("<table><tr><td>a<td>b"),
            "<table><tr><td>a<td>b</td></tr></table>"
        );
        assert_eq!(
            balance_fragment("<dl><dt>a<dd>b"),
            "<dl><dt>a<dd>b</dd></dl>"
        );
        assert_eq!(
            balance_fragment("<select><option>a<option>b"),
            "<select><option>a<option>b</option></select>"
        );
        // Explicitly closed fragments are untouched: the implicit close pops
        // exactly what the explicit one would have.
        let explicit = "<p>a</p><p>b</p><ul><li>x</li><li>y</li></ul>";
        assert_eq!(balance_fragment(explicit), explicit);
        // A close tag whose element was already closed implicitly used to be
        // an orphan and to go the way of every orphan — dropped, on the
        // grounds that a browser would turn it into "the same phantom `<p></p>`
        // this fix removes". It does turn it into one, and measurement is what
        // changed the verdict (DCR-0051): `<p>a<div>b</div>c</p>` renders
        // `<p>a</p><div>b</div>c<p></p>` in Chromium, so that paragraph is the
        // author's own bytes rendering, not structure the balancer invented.
        // R0002-0061 was about the balancer APPENDING a second `</p>`; deleting
        // one the author wrote is the different act of editing the pane. So
        // `</p>` — and `</br>`, which mints a `<br>` the same way — is never an
        // orphan now, and this fragment is a fixed point instead.
        let author_wrote_it = "<p>a<div>b</div>c</p>";
        assert_eq!(balance_fragment(author_wrote_it), author_wrote_it);
        // Nesting is unaffected: an inner list opens inside its item.
        assert_eq!(
            balance_fragment("<ul><li>a<ul><li>b"),
            "<ul><li>a<ul><li>b</li></ul></li></ul>"
        );
    }

    /// R0003-0066: HTML's tag-open state admits an ASCII LETTER and nothing
    /// else, so `<1>` is prose, not markup. Reading it as a tag cost the same
    /// two things R0002-0020 cost for RCDATA: an invented `</1>` appended to
    /// the pane, and an orphan `</1>` DELETED from it.
    #[test]
    fn a_digit_leading_tag_name_is_plain_text() {
        assert!(tag_inventory("<1>y").is_empty());
        assert_eq!(balance_fragment("<1>y"), "<1>y");
        assert!(tag_inventory("Rows <2026 total> here").is_empty());
        assert_eq!(
            balance_fragment("Rows <2026 total> here"),
            "Rows <2026 total> here"
        );
        // A close tag is held to the same rule — and an orphan close is
        // exactly what the balancer deletes, so this arm is content loss.
        assert!(tag_inventory("</1>").is_empty());
        assert_eq!(balance_fragment("</1>"), "</1>");
        // Digits are still name bytes AFTER the first one: `<h1>` is a tag,
        // and so is a hyphenated custom element.
        assert_eq!(tag_inventory("<h1>x</h1>"), vec!["h1", "/h1"]);
        assert_eq!(tag_inventory("<x-2>y</x-2>"), vec!["x-2", "/x-2"]);
    }

    /// R0003-0067: a browser never tokenizes what is inside a `<![CDATA[` as
    /// markup — in foreign content it is character data ending at `]]>`, in
    /// HTML content it is a bogus comment ending at the first `>`.
    #[test]
    fn cdata_sections_are_not_scanned_for_tags() {
        let svg = "<svg><text><![CDATA[<b>bold</b>]]></text></svg>";
        assert_eq!(tag_inventory(svg), vec!["svg", "text", "/text", "/svg"]);
        assert_eq!(balance_fragment(svg), svg);
        // The orphan-close arm: this used to be deleted from the pane,
        // silently editing character data the reader sees.
        let orphan = "<svg><![CDATA[</b>]]></svg>";
        assert_eq!(tag_inventory(orphan), vec!["svg", "/svg"]);
        assert_eq!(balance_fragment(orphan), orphan);
        // MathML is foreign content too.
        let math = "<math><![CDATA[<mi>x</mi>]]></math>";
        assert_eq!(tag_inventory(math), vec!["math", "/math"]);
        // In HTML content the same bytes are a bogus comment, so the close
        // tag inside is not markup there either.
        let html = "<p><![CDATA[</b>]]></p>";
        assert_eq!(tag_inventory(html), vec!["p", "/p"]);
        assert_eq!(balance_fragment(html), html);
        // Foreign content ends with its root: past `</svg>` the bogus-comment
        // reading is back.
        assert_eq!(
            tag_inventory("<svg></svg><![CDATA[<i>]]>"),
            vec!["svg", "/svg"]
        );
        // A self-closing `<svg/>` opens no foreign content, so the section
        // after it is a bogus comment and the `<i>` inside is not a tag.
        assert_eq!(tag_inventory("<svg/><![CDATA[<i>]]>"), vec!["svg"]);
        // An unterminated section consumes the rest rather than looping.
        assert_eq!(tag_inventory("<svg><![CDATA[<b>"), vec!["svg"]);
    }

    /// ti 490d97 wave 1 retitled this from
    /// `a_slash_outside_any_attribute_value_still_self_closes`. The scanner
    /// half is unchanged — a slash outside a value still SETS the flag — but
    /// "self-closes" was the walk's old conclusion, and it now holds only for
    /// void and foreign tags. The `<span/>tail` arm moved with the fix; every
    /// other arm here is untouched.
    #[test]
    fn a_slash_outside_any_attribute_value_sets_the_flag() {
        // Void, so it must not be pushed for two independent reasons.
        assert_eq!(balance_fragment("<br/>"), "<br/>");
        // Non-void HTML content: the flag is set and then IGNORED.
        assert_eq!(balance_fragment("<span/>tail"), "<span/>tail</span>");
        assert_eq!(
            balance_fragment("<img src=\"x.png\"/><i>y"),
            "<img src=\"x.png\"/><i>y</i>"
        );
        // A detached slash is NOT a self-closing marker (HTML reconsumes it
        // in before-attribute-name state). Same output as the flagged
        // spelling now, for a different reason — which is the point: the
        // browser cannot tell them apart either.
        assert_eq!(balance_fragment("<span / >tail"), "<span / >tail</span>");
    }

    /// ti 549b20, measured against the vendored DOMPurify 3.2.6: a browser
    /// parses `<div ">` as an open div and the sanitized DOM closes it at
    /// fragment end. The old scanner saw zero tokens here and returned the
    /// input unchanged — disagreeing with the DOM this function exists to
    /// protect.
    #[test]
    fn a_stray_quote_no_longer_hides_an_unclosed_div_from_the_balancer() {
        assert_eq!(
            balance_fragment("<div \"> <p>x</p>"),
            "<div \"> <p>x</p></div>"
        );
        // An unterminated tag is a Skip, not structure: nothing to balance,
        // before the fix and after it.
        // ti `c1f9a8`: the unterminated tag is now DELETED rather than left
        // in place — a browser abandons a tag cut off at EOF, so the bytes
        // mint nothing and carrying them to the pane only risks the next `>`
        // being the wrapper's own.
        assert_eq!(balance_fragment("<p>a</p><div "), "<p>a</p>");
    }

    /// `contracts.md` §4a as one question, mirroring
    /// `generative_properties.rs::wrapper_survives`: mount the balanced
    /// fragment the way `render::html_pane` mounts it and ask whether the
    /// wrapper's own `</div>` still closes the wrapper.
    ///
    /// The two copies exist because that one lives across the crate boundary
    /// in an integration test, which cannot see a `#[cfg(test)]` item in here.
    /// They ask the same question of the same public function, so there is one
    /// answer and two callers, not two opinions.
    const WRAPPER_OPEN: &str = "<div data-sync-id=\"p-0001\">";

    fn wrapper_survives(balanced: &str) -> bool {
        let pane = format!("{WRAPPER_OPEN}{balanced}</div>");
        element_extents(&pane).first().is_some_and(|e| {
            e.name == "div"
                && e.depth == 0
                && e.open == (0, WRAPPER_OPEN.len())
                && e.close == Some((pane.len() - "</div>".len(), pane.len()))
        })
    }

    /// R0010-0031 — the raw-text element set was short by four names, and
    /// `iframe` is the one CommonMark hands to an attacker directly.
    ///
    /// `<div><iframe></div></iframe>` is ONE type-6 html block: `div` and
    /// `iframe` are both type-6 start-condition names, so an ordinary source
    /// Markdown file can carry it (invariant 7). A browser's iframe holds
    /// generic RAW TEXT, so the `</div>` inside it is text and the author's
    /// `</iframe>` is the real closer. The scanner read both as markup, the
    /// walk popped `iframe` at `</div>`, and the balancer then DELETED
    /// `</iframe>` as an orphan — emitting a fragment whose iframe is still
    /// open, which in a pane swallows the sync wrapper's own `</div>` and
    /// every following block to EOF.
    #[test]
    fn a_raw_text_element_beyond_the_original_four_keeps_its_own_closer() {
        assert_eq!(
            balance_fragment("<div><iframe></div></iframe>"),
            "<div><iframe></div></iframe></div>",
            "the `</div>` is iframe raw text, so the author's `</iframe>` is \
             the closer and the outer div is what needs closing"
        );
        assert!(
            wrapper_survives(&balance_fragment("<div><iframe></div></iframe>")),
            "and the pane's own </div> closes the wrapper (contracts.md §4a)"
        );
        // The other three names of the finding, each on its own route.
        assert_eq!(
            balance_fragment("<xmp></p></xmp>"),
            "<xmp></p></xmp>",
            "an `</p>` inside xmp is text, not an orphan to delete"
        );
        assert_eq!(
            balance_fragment("<noembed></div></noembed>"),
            "<noembed></div></noembed>",
            "and a `</div>` inside noembed is text, not an orphan to delete"
        );
        assert_eq!(
            balance_fragment("<noframes></p></noframes>"),
            "<noframes></p></noframes>"
        );
        // The qualifier the four new names inherit is real: inside foreign
        // content they are ordinary elements whose contents ARE markup (ti
        // `2e2453`, DCR-0042), so the `<g>` here is a real element that earns
        // its own closer rather than iframe text.
        assert_eq!(
            balance_fragment("<svg><iframe><g>x"),
            "<svg><iframe><g>x</g></iframe></svg>",
            "inside <svg> an iframe is an ordinary foreign element"
        );
    }

    /// R0010-0032 — HTML's PLAINTEXT state, the one a tokenizer never leaves.
    ///
    /// `<div><plaintext></div>` walked as div → plaintext → both closed at
    /// `</div>` and passed through byte-identical, while a browser turns the
    /// wrapper's own `</div>`, the next block's anchor and the rest of the
    /// pane into text. There are no bytes that end the state, so the region is
    /// DELETED rather than terminated — the decision DCR-0050 records, and the
    /// reason the start tag is inside the deleted span: leaving it would leave
    /// the switch, and deleting only it would hand the bytes after it back to
    /// the tokenizer as markup after the strip had passed over them as text.
    #[test]
    fn a_plaintext_start_tag_no_longer_swallows_the_wrapper() {
        assert_eq!(
            balance_fragment("<div><plaintext></div>"),
            "<div></div>",
            "the region is deleted and the div earns a real closer"
        );
        assert!(
            wrapper_survives(&balance_fragment("<div><plaintext></div>")),
            "so the pane's own </div> closes the wrapper (contracts.md §4a)"
        );
        // The paragraph a browser closes implicitly at `<plaintext>` is closed
        // here too, by the same appended closer the deletion leaves owing.
        assert_eq!(balance_fragment("<p>a<plaintext>b"), "<p>a</p>");
        // A whole fragment that is nothing but the state.
        assert_eq!(balance_fragment("<plaintext>x</plaintext>"), "");
        // An impostor anchor written INTO the region cannot reach the pane,
        // which is what makes the strip's pass-over safe rather than a bypass
        // (invariant 7 / OI-0035): the composition is what ships.
        let planted = "<div><plaintext><div data-sync-id=\"p-0009\">y";
        let chained = balance_fragment(&strip_reserved_sync_attrs(planted));
        assert_eq!(chained, "<div></div>");
        assert_eq!(
            strip_reserved_sync_attrs(&chained).into_owned(),
            chained,
            "asked through the strip itself: nothing in the reserved namespace \
             is left for it to cut"
        );
        // HTML content only, like voidness (ti `48f3c6`) and raw text (ti
        // `2e2453`): the switch is an "in body" tree-construction rule, and
        // `plaintext` is not a breakout tag, so inside <svg> it is an ordinary
        // foreign element.
        assert_eq!(
            balance_fragment("<svg><plaintext>x"),
            "<svg><plaintext>x</plaintext></svg>"
        );
    }

    /// R0010-0034 — a browser decodes an attribute value before it reads it,
    /// so `encoding="text&#47;html"` makes `annotation-xml` an HTML
    /// integration point.
    ///
    /// Comparing the raw slice made it MathML content, which honoured the
    /// `<div/>`'s slash and left the `div` unopened; the balancer then owed
    /// only `</annotation-xml></math>`. A browser had opened the `div` (the
    /// slash is a parse error it ignores in HTML content) and ignores both of
    /// those end tags, because in-body's "any other end tag" stops at the
    /// special `div` — so the fragment reached the pane with three elements
    /// open, and `annotation-xml` is a SCOPE terminator, which makes the
    /// wrapper's own `</div>` a token a browser ignores outright.
    #[test]
    fn an_encoded_integration_point_encoding_is_read_the_way_a_browser_reads_it() {
        let html = "<math><annotation-xml encoding=\"text&#47;html\"><div/>x";
        assert_eq!(
            balance_fragment(html),
            "<math><annotation-xml encoding=\"text&#47;html\"><div/>x\
             </div></annotation-xml></math>",
            "the div is HTML content, so it opens and is owed a closer"
        );
        assert!(wrapper_survives(&balance_fragment(html)));
        // The plain spelling was always right and stays byte-identical.
        assert_eq!(
            balance_fragment("<math><annotation-xml encoding=\"text/html\"><div/>x"),
            "<math><annotation-xml encoding=\"text/html\"><div/>x\
             </div></annotation-xml></math>"
        );
        // Anti-vacuity: the decode did not simply make every `annotation-xml`
        // an integration point. An encoding that is not one of HTML's two
        // still leaves MathML content, where the self-closing `/` IS honoured
        // — so `<rect/>` opens nothing and only the two foreign closers are
        // owed. (`<rect/>` rather than `<div/>` here because `div` is a
        // BREAKOUT tag: it pops both foreign frames and lands in HTML content
        // either way, which is the same answer for both encodings and would
        // have made this arm prove nothing.)
        assert_eq!(
            balance_fragment("<math><annotation-xml encoding=\"image&#47;svg+xml\"><rect/>x"),
            "<math><annotation-xml encoding=\"image&#47;svg+xml\"><rect/>x\
             </annotation-xml></math>"
        );
        assert_eq!(
            balance_fragment("<math><annotation-xml encoding=\"text&#47;html\"><rect/>x"),
            "<math><annotation-xml encoding=\"text&#47;html\"><rect/>x\
             </rect></annotation-xml></math>",
            "and at the decoded HTML encoding the same `/` is the parse error \
             HTML ignores, so the element opens"
        );
    }

    /// R0010-0049 — HTML closes a comment at `--!>` as well as at `-->`.
    ///
    /// `<!-- a --!>` + `<div>y` + `<!-- b -->` is ONE type-2 html block, so
    /// this arrives from ordinary source Markdown (invariant 7). Reading only
    /// `-->` made the whole block one terminated comment: the `<div>` was
    /// never tokenized, the balancer owed nothing, and the still-open div
    /// consumed the sync wrapper's own `</div>` in the pane.
    #[test]
    fn a_bang_terminated_comment_no_longer_hides_the_markup_after_it() {
        let html = "<!-- a --!><div>y<!-- b -->";
        assert_eq!(
            balance_fragment(html),
            "<!-- a --!><div>y<!-- b --></div>",
            "the first comment ends at `--!>`, so the div is real and unclosed"
        );
        assert!(wrapper_survives(&balance_fragment(html)));
        // The unterminated form still terminates with the canonical bytes, and
        // they still close it: `--!` + `-->` walks comment-end-bang →
        // comment-end-dash → comment-end → close.
        assert_eq!(
            balance_fragment("<div>x<!-- a --!"),
            "<div>x<!-- a --!--></div>"
        );
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn tag_inventory_lists_open_and_close_tags_in_order() {
        assert_eq!(
            tag_inventory("<div><b>x</b></div>"),
            vec!["div", "b", "/b", "/div"]
        );
    }

    #[test]
    fn identity_splice_preserves_tag_inventory() {
        let block = "<details><summary>s</summary><p>p</p></details>";
        let segs = extract(block).expect("extracts");
        let out = splice(
            block,
            &segs.texts,
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(tag_inventory(&out), tag_inventory(block));
    }

    /// R0002-0020's layer-3 half, end to end. lol_html reads RCDATA the way
    /// a browser does, so the extracted segment carries the tag-like bytes
    /// as text and the splice escapes them. When `tag_inventory` scanned
    /// that content as markup, only the SOURCE side reported the phantom
    /// `/p` — the inventories diverged, `validate`'s post-splice check
    /// called it an engine fault, and a perfectly good translation fell back
    /// to source. Both sides must see the same two tags.
    #[test]
    fn translating_rcdata_content_preserves_the_tag_inventory() {
        let block = "<textarea>Use </p> to close a paragraph</textarea>";
        let segs = extract(block).expect("extracts");
        assert_eq!(
            segs.texts,
            vec!["Use </p> to close a paragraph".to_string()],
            "the tag-like bytes are RCDATA text, extracted whole",
        );
        let out = splice(
            block,
            &["단락을 닫으려면 </p> 를 씁니다".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(tag_inventory(&out), tag_inventory(block));
        assert_eq!(tag_inventory(block), vec!["textarea", "/textarea"]);
    }

    /// R0003-0066's layer-3 half, the same phantom shape as the RCDATA case
    /// above. lol_html reads `<1>` as text (HTML's tag-open state wants a
    /// letter), so the splice escapes it — while the scanner counted a `1`
    /// open tag on the SOURCE side only. The inventories diverged, layer 3
    /// called an engine fault, and a correct translation was dropped to
    /// fallback_source.
    #[test]
    fn translating_text_that_looks_like_a_digit_tag_preserves_the_tag_inventory() {
        let block = "<p>Row <1> is the header</p>";
        let segs = extract(block).expect("extracts");
        assert_eq!(
            segs.texts,
            vec!["Row <1> is the header".to_string()],
            "the tag-like bytes are ordinary text, extracted whole",
        );
        let out = splice(
            block,
            &["<1> 행이 헤더입니다".to_string()],
            BlankLinePolicy::from_commonmark_html_block_type(6),
        )
        .expect("splices");
        assert_eq!(tag_inventory(&out), tag_inventory(block));
        assert_eq!(tag_inventory(block), vec!["p", "/p"]);
    }

    /// ti 549b20: the inventory is the layer-3 ledger, and the Skip-to-EOF
    /// token must be filtered by construction — verified here rather than
    /// assumed, because a phantom entry there fails good translations.
    #[test]
    fn the_ledger_never_carries_a_skipped_suffix() {
        assert_eq!(
            tag_inventory("<div \"> <p data-sync-id=\"v\">x</p>"),
            vec!["div", "p", "/p"]
        );
        assert_eq!(tag_inventory("<p>a</p><div class=\"x"), vec!["p", "/p"]);
    }
}

// ---------------------------------------------------------------------------
// Tree construction: the five open-element-stack tickets, and their oracle
// ---------------------------------------------------------------------------

/// The regression set for ti `9b4d66`, `307283`, `895fb7`, `da6bb5`, `bb961a`
/// and `a7e625` — the family whose one root cause was that this crate kept two
/// open-element stacks and popped both of them differently from a browser.
///
/// **Every expected value in this module was MEASURED in headless Chromium
/// before it was written**, through the same `innerHTML`-on-a-detached-`div`
/// fragment parse the shipped shells mount a pane with, and the measurement is
/// quoted beside each one. Asserting this crate's answer against this crate's
/// answer is the blind spot ti `ec235f` exists to close (see
/// `web/tests/html-oracle.spec.js`); these tests are the cheap in-crate half of
/// that record, and the browser oracle is the half that can contradict it.
#[cfg(test)]
mod tree_construction_tests {
    use super::*;

    /// The pane mount, the shape `contracts.md` §4a is a claim about: the
    /// block's balanced fragment inside its own sync wrapper.
    fn pane(fragment: &str) -> String {
        format!("<div data-sync-id=\"p-0001\">{fragment}</div>")
    }

    /// §4a's question: does the wrapper's own `</div>` still close the
    /// wrapper, or did the fragment eat it? Asked of the crate's walk, which
    /// is what the balancer acts on — the browser half is the oracle spec's.
    fn wrapper_survives(fragment: &str) -> bool {
        let pane = pane(fragment);
        element_extents(&pane).iter().any(|e| {
            e.name == "div"
                && e.depth == 0
                && e.close == Some((pane.len() - "</div>".len(), pane.len()))
        })
    }

    /// The bytes a reader's browser is handed, in the order `render.rs` calls
    /// them: `balance_fragment(&strip_reserved_sync_attrs(md))`.
    fn chained(md: &str) -> String {
        balance_fragment(&strip_reserved_sync_attrs(md))
    }

    // -- ti `9b4d66` --------------------------------------------------------

    /// ti `9b4d66`. One CommonMark type-6 html block, reachable from untrusted
    /// source Markdown, that used to put a live impostor `data-sync-id` in a
    /// reader's DOM.
    const IMPOSTOR: &str =
        "<p><ul><svg></p><title><div data-sync-id=\"p-0002\">impostor</div></title></svg>";

    /// `</p>` is a BREAKOUT end tag in foreign content — HTML groups it with
    /// `</br>` and with the breakout START tags — so it pops the `<svg>` and
    /// leaves `<title>` an ordinary HTML RCDATA element whose interior is
    /// text. Chromium, measured on `IMPOSTOR`:
    ///
    /// ```text
    /// <p></p><ul><svg></svg><p></p><title>&lt;div data-sync-id="p-0002"&gt;impostor&lt;/div&gt;</title></ul>
    /// ```
    ///
    /// The walk agreed by accident and the SCANNER did not: its `open_elements`
    /// carried a `p` frame the walk had already closed implicitly at `<ul>`,
    /// so `</p>` found that stale frame, `truncate`d the `<svg>` away with it,
    /// and read `<title>` as HTML RCDATA for the wrong reason. The balancer
    /// then deleted the `</p>` as an orphan, and on the deleted bytes nothing
    /// left foreign content at all: `<title>` became an SVG element, its
    /// interior became markup, and the plant became a live anchor.
    #[test]
    fn a_breakout_end_tag_survives_the_balancer_and_keeps_the_impostor_text() {
        let chained = chained(IMPOSTOR);
        assert!(
            chained.contains("</p>"),
            "the `</p>` that leaves foreign content is structure, not an \
             orphan — deleting it puts `<title>` back inside `<svg>`\n  \
             chained = {chained:?}"
        );
        assert_eq!(
            strip_reserved_sync_attrs(&chained).into_owned(),
            chained,
            "the pane chain minted a reserved sync attribute the strip had \
             already cleared (P6 / OI-0035 route (c))"
        );
        assert!(wrapper_survives(&chained));
    }

    // -- ti `307283` --------------------------------------------------------

    /// ti `307283` case 1. Chromium leaves the OUTER `div` open here — the
    /// adoption agency algorithm reparents `<b>` into the inner `div`, so the
    /// author's `</div>` closes the inner one. Measured: `openAtEnd` is
    /// `["div"]`, and the balanced form below is `[]`.
    ///
    /// The fix is not the adoption agency algorithm (see the module's
    /// residual note) but the guard that precedes it: in-body's "any other end
    /// tag" walks down from the current node and **stops at the first special
    /// element**, and the inner `div` is special, so `</b>` is ignored.
    #[test]
    fn a_misnested_formatting_end_tag_no_longer_closes_the_block_above_it() {
        let out = balance_fragment("<div><b><div></b></div>");
        assert_eq!(out, "<div><b><div></b></div></b></div>");
        assert!(wrapper_survives(&out));
        assert_eq!(balance_fragment(&out), out);
    }

    /// ti `307283` case 2, the more serious shape: the balancer CREATED the
    /// §4a break by deleting a closer the author wrote and a browser needs.
    /// Chromium: `<div><b><div></b></div></div>` ends with nothing open.
    #[test]
    fn the_balancer_no_longer_deletes_the_closer_a_browser_needs() {
        let input = "<div><b><div></b></div></div>";
        assert_eq!(balance_fragment(input), input);
        assert!(wrapper_survives(input));
    }

    /// R0010-0039, folded into ti `307283`: in-body's `</div>` rule asks
    /// whether a `div` is in SCOPE, and `table`, `td` and `th` all terminate a
    /// scope search. Chromium ignores both `</div>`s below and leaves the
    /// table open; the walk popped the table at the nearest name match, called
    /// the fragment balanced, and let the pane's own `</div>` be ignored the
    /// same way — so the next block's anchor was foster-parented INSIDE this
    /// block's wrapper (measured `parent-div`).
    ///
    /// Both balanced forms below were measured as `openAtEnd == []`.
    #[test]
    fn a_div_end_tag_a_table_puts_out_of_scope_leaves_the_table_open() {
        assert_eq!(
            balance_fragment("<div><table>a</div>"),
            "<div><table>a</div></table></div>"
        );
        assert_eq!(
            balance_fragment("<div><table><tr><td>a</div>"),
            "<div><table><tr><td>a</div></td></tr></table></div>"
        );
        for input in ["<div><table>a</div>", "<div><table><tr><td>a</div>"] {
            let out = balance_fragment(input);
            assert!(wrapper_survives(&out), "{out:?}");
            assert_eq!(balance_fragment(&out), out);
        }
    }

    // -- ti `895fb7` --------------------------------------------------------

    /// ti `895fb7`. The first call used to append `</math>` and the second to
    /// delete it, so the pane's bytes depended on how many times the repair
    /// ran. One stack is what fixes it: the `<div ">` breakout pops the
    /// `<math>` on the SAME stack the scanner tokenizes from, so there is no
    /// closer to owe. Chromium on the input: `openAtEnd == ["ul","div"]`.
    #[test]
    fn balance_fragment_is_a_fixed_point_on_stale_frame_input() {
        let input = "<p/><ul><math></p><div \">";
        let once = balance_fragment(input);
        assert_eq!(once, "<p/><ul><math></p><div \"></div></ul>");
        assert_eq!(balance_fragment(&once), once);
        assert!(wrapper_survives(&once));
    }

    // -- ti `bb961a` --------------------------------------------------------

    /// ti `bb961a`. `<desc>` is an SVG HTML integration point, so the `<div>`
    /// inside it is an HTML element — and in-body's "any other end tag" stops
    /// at it, because `div` is special. Chromium therefore IGNORES the
    /// `</desc>` and is still in HTML content, where `<![CDATA[` opens a bogus
    /// comment ending at the first `>`:
    ///
    /// ```text
    /// <svg><desc><div><!--[CDATA[x--></div></desc></svg>
    /// ```
    ///
    /// The walk popped the nearest name match instead, landed back in foreign
    /// content, and read one CDATA section to EOF — so everything after it was
    /// text to the strip and markup to the browser, and a planted
    /// `data-sync-id` reached a live DOM.
    #[test]
    fn an_ignored_desc_end_tag_keeps_the_fragment_in_html_content() {
        // The `</desc>` STAYS: a browser ignores it, so deleting it would edit
        // the reader's bytes for nothing, and the balanced form below parses to
        // the same DOM Chromium gives the input above.
        let out = balance_fragment("<svg><desc><div></desc><![CDATA[x");
        assert_eq!(out, "<svg><desc><div></desc><![CDATA[x></div></desc></svg>");
        assert!(wrapper_survives(&out));
        assert_eq!(balance_fragment(&out), out);
    }

    /// The generated exemplar the oracle found `bb961a` on, whole. Its harm
    /// was two at once: a live `data-sync-id` in the DOM (`reserved:dom-only`)
    /// and a `parent-td` §4a break.
    #[test]
    fn the_generated_bb961a_exemplar_leaks_no_anchor_and_keeps_the_wrapper() {
        let input = "<!doctype html><blockquote><div><span></blockquote><a href=\"x\">\
                     <div>y</a><div>z</a><svg><desc><div></desc><![CDATA[y<p><table><tr>\
                     <td>c<?pi<!doctype html><div/data-sync-id=\"p-1\"></>";
        let chained = chained(input);
        assert_eq!(
            strip_reserved_sync_attrs(&chained).into_owned(),
            chained,
            "a reserved sync attribute survived into the pane chain"
        );
        assert!(wrapper_survives(&chained), "chained = {chained:?}");
        assert_eq!(balance_fragment(&chained), chained);
    }

    // -- ti `da6bb5` --------------------------------------------------------

    /// ti `da6bb5`, the over-open direction. In-body IGNORES a stray
    /// `<td>`/`<th>`/`<tr>`/`<tbody>`/`<caption>`/`<col>`/`<colgroup>` start
    /// tag outright, so pushing a frame for one made the balancer append a
    /// closer HTML needs none for — inventing structure in a pane, which
    /// `implicitly_closes`' own docstring says the walk must not do.
    ///
    /// Chromium on the oracle's exemplar: `openAtEnd == ["font","li"]`, DOM
    /// `x<font><li>&amp;</li></font>` — no `td`, no `tr`.
    #[test]
    fn a_table_cell_start_tag_outside_a_table_opens_nothing() {
        assert!(element_extents("<td>x</tr>").is_empty());
        // `</tr>` goes — nothing on this stack stops that search, so in a
        // mounted pane it would run on into the wrapper — while `</g>` stays,
        // stopped at the special `<li>` a browser stops at too. Measured DOM,
        // identical for input and output: `x<font><li>&amp;</li></font>`.
        assert_eq!(
            balance_fragment("<td>x</tr><font><li></g>&amp;<!doctype html>"),
            "<td>x<font><li></g>&amp;<!doctype html></li></font>"
        );
        // …and inside a real table it still opens, because "in table" is a
        // different insertion mode. Chromium: `["table","tbody","tr","td"]`.
        assert_eq!(
            balance_fragment("<table><td>a"),
            "<table><td>a</td></table>"
        );
    }

    /// ti `da6bb5`'s second sub-mechanism: HTML closes an open `<p>` by
    /// BUTTON SCOPE, which reaches through the `<em>` sitting on top of it,
    /// and this crate closed only the innermost frame. Chromium on the
    /// oracle's exemplar ends with nothing open.
    #[test]
    fn an_implied_paragraph_close_reaches_through_the_formatting_element_above_it() {
        // Chromium: `<p><em></em></p><ul><li><em>q</em></li></ul>` — the `<em>`
        // is reconstructed inside the `<li>` rather than closed, which is the
        // adoption-agency residual and costs nothing here. `</em>` stays
        // (stopped at the special `<li>`); `</link>` and `</math>` go.
        let out = balance_fragment("</link><p/><em><ul><li>q</em></li></ul></math>");
        assert_eq!(out, "<p/><em><ul><li>q</em></li></ul>");
        assert!(wrapper_survives(&out));
        assert_eq!(balance_fragment(&out), out);
    }

    // -- ti `a7e625` --------------------------------------------------------

    /// ti `a7e625`, CONFIRMED by measurement rather than accepted from the
    /// spec trace. `mglyph` and `malignmark` are the two start tags HTML's
    /// tree-construction dispatcher does NOT hand to the HTML rules inside a
    /// MathML text integration point, so they stay MathML — and a `<script>`
    /// under one is a foreign element, not a raw-text run. Chromium:
    ///
    /// ```text
    /// <math><mi><mglyph><script></script></mglyph><div></div></mi></math>
    /// ```
    ///
    /// with `openAtEnd == ["math","mi","div"]`; `malignmark` measures the
    /// same. The crate read `<script>` as raw text, made `<div>` its text
    /// content, and appended four closers a browser then ignores.
    #[test]
    fn mglyph_and_malignmark_keep_their_children_in_mathml() {
        for name in ["mglyph", "malignmark"] {
            let input = format!("<math><mi><{name}><script><div>");
            assert_eq!(
                balance_fragment(&input),
                format!("{input}</div></mi></math>")
            );
        }
    }

    /// Found by the 400_000-input deep property run while DCR-0051 was being
    /// written, and pinned because it is the one shape where getting HTML's
    /// step ORDER wrong turns the repair into the break it exists to prevent.
    ///
    /// Foreign content's "any other end tag" name-tests the current node and
    /// each FOREIGN node below it, and checks the namespace BEFORE the name on
    /// every step after the first. Testing the name at an HTML-namespace frame
    /// as well let this fragment's `</div>` walk past the `foreignObject` — a
    /// scope terminator — and match the sync wrapper's own `<div>`.
    ///
    /// Chromium leaves `["svg","foreignobject","annotation-xml","p"]` open on
    /// the input, `[]` on the balanced form, and mounts the following block's
    /// anchor as a direct child of `<main>`; input and output parse to the
    /// same DOM.
    #[test]
    fn a_foreign_end_tag_search_stops_at_the_first_html_frame_not_at_a_name() {
        let input = "<![CDATA[y]]><![CDATA[<b>x</b>]]><svg><foreignObject><p></foreignObject></svg>\
                     <! <div> >div<i><p>x</i></p></div><annotation-xml encoding=\"text&#47;html\"><p>";
        let out = balance_fragment(input);
        assert_eq!(
            out,
            "<![CDATA[y]]><![CDATA[<b>x]]><svg><foreignObject><p></foreignObject></svg>\
             <! <div> >div<i><p>x</i></p></div><annotation-xml encoding=\"text&#47;html\">\
             <p></p></annotation-xml></foreignobject></svg>"
        );
        assert!(wrapper_survives(&out), "out = {out:?}");
        assert_eq!(balance_fragment(&out), out);
        // The same terminator, asked directly: `foreignObject` is a scope
        // terminator, so a `</div>` beneath it reaches nothing and is inert
        // rather than orphaned. Chromium agrees — it leaves `svg`,
        // `foreignObject` and `p` open and ignores the closer.
        let inert = "<svg><foreignObject><p></foreignObject></svg></div>";
        assert_eq!(
            balance_fragment(inert),
            format!("{inert}</p></foreignobject></svg>")
        );
    }

    /// A nested `<svg>` inside `<math>` is a MathML element — foreign
    /// content's "any other start tag" inserts in the namespace of the
    /// adjusted current node — so `desc` under it is NOT an HTML integration
    /// point and the `<div>` breaks all the way out. Chromium:
    /// `<math><svg><desc></desc></svg></math><div></div>`, `openAtEnd ==
    /// ["div"]`.
    #[test]
    fn a_nested_svg_inside_math_stays_in_mathml_content() {
        assert_eq!(
            balance_fragment("<math><svg><desc><div>"),
            "<math><svg><desc><div></div>"
        );
    }
}
