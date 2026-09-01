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
//! `transync_syntax::intake::html` module — and this crate is the mechanics
//! layer underneath it. The name says "html" because the capability is HTML,
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
/// The rule is **global** — foreign content included. Appending a close tag
/// for a void name is worse than leaving one out, because HTML's end-tag-`br`
/// rule turns an emitted `</br>` back into a fresh `<br>`: the balancer would
/// mint structure instead of repairing it. The cost is that a real foreign
/// `<svg><link>…</link>` still has its closer dropped as an orphan; that
/// trade is deliberate.
///
/// `image` is deliberately NOT here. `svg:image` is a real, closable foreign
/// element, so calling it void would delete an author's `</image>` closers.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

/// Elements whose content HTML tokenizes as raw text / RCDATA. HTML ignores
/// the self-closing flag on them: `<textarea/>` opens RCDATA and swallows
/// every later sibling as text. The balancer therefore treats a self-closing
/// start tag for these as OPEN so a close tag is appended, and [`scan_tags`]
/// enters raw-text state for all four regardless of the flag (DCR-0016
/// Part D; R0002-0020 brought textarea/title into the tokenizer half, which
/// had covered only script and style).
const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style", "textarea", "title"];

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
/// `title`)? A browser never tokenizes their content as markup, and HTML
/// ignores the self-closing flag on them (DCR-0016 Part D).
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
pub(crate) fn collapse_blank_lines(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    let kept: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            let blank = l.chars().all(|c| c == ' ' || c == '\t');
            !(blank && *i > 0 && *i + 1 < n)
        })
        .map(|(_, l)| *l)
        .collect();
    kept.join("\n")
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
    /// section (either terminator mode), a bogus comment (`<!…>` / `<?…>`),
    /// or a tag left unterminated at EOF (ti 549b20 — a browser abandons a
    /// tag cut off before its `>`, minting no element and no attributes, so
    /// the bytes are a passed-over region, not markup).
    /// It carries no name because it has none — what it carries is the byte
    /// range, which is the thing an intake needs in order to trim the
    /// anonymous runs between elements. Neither `tag_inventory` nor
    /// `balance_fragment` reads it.
    Skip { span: (usize, usize) },
}

/// Minimal tag tokenizer for balancing: understands comments, CDATA
/// sections, the raw-text and RCDATA states of every [`is_raw_text`] element,
/// and quoted attribute values. NOT a general HTML parser — it is one
/// self-consistent opinion about HTML tokenization, shared by the render-path
/// balancer (spec 2026-08-03 §3.4), the layer-3 inventory check, and the HTML
/// intake to come.
pub fn scan_tags(html: &str) -> Vec<TagToken> {
    let bytes = html.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    // Inside a raw-text / RCDATA element: only its own close tag ends it.
    let mut raw_until: Option<String> = None;
    // R0003-0067: how many `<svg>` / `<math>` elements are open — HTML's
    // "foreign content", the only place a `<![CDATA[` is a CDATA section.
    // A stack-only balancer cannot track the adjusted current node, so this
    // counts the two integration-point roots and nothing else; that is the
    // same deliberate bound `implicitly_closes` documents.
    let mut foreign_depth = 0usize;

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
            let end = html[i..]
                .find("-->")
                .map(|p| i + p + 3)
                .unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
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
            let terminator = if foreign_depth > 0 { "]]>" } else { ">" };
            let end = html[i..]
                .find(terminator)
                .map(|p| i + p + terminator.len())
                .unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
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
            let end = html[i..].find('>').map(|p| i + p + 1).unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
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
        if bytes.get(name_start).is_some_and(u8::is_ascii_alphabetic) {
            j += 1;
            while j < bytes.len()
                && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-' || bytes[j] == b':')
            {
                j += 1;
            }
        }
        if j == name_start {
            i += 1; // "<" not followed by a tag name — plain text
            continue;
        }
        let name = html[name_start..j].to_ascii_lowercase();
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
            tokens.push(TagToken::Skip {
                span: (start, bytes.len()),
            });
            break;
        }
        let span = (start, j + 1);
        if closing {
            if is_foreign_root(&name) {
                foreign_depth = foreign_depth.saturating_sub(1);
            }
            tokens.push(TagToken::Close { name, span });
        } else {
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
            // silently editing what the reader sees. The balancer already
            // treated all four as raw-text (`is_raw_text` in
            // `balance_fragment`) — the tokenizer's disagreement with it was
            // the whole defect (DCR-0016 Part D argues from exactly this
            // consistency).
            if is_raw_text(&name) {
                raw_until = Some(name.clone());
            }
            // A self-closing `<svg/>` opens nothing: foreign content is the
            // one place HTML honours the marker.
            if is_foreign_root(&name) && !self_closing {
                foreign_depth += 1;
            }
            tokens.push(TagToken::Open {
                name,
                self_closing,
                span,
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
/// **Not every tag mints one.** A void element (`img`, `br`, `hr`, …) is never
/// pushed onto the walk's stack, so it produces **no** `ElementExtent` at all
/// — it has no content and nothing to close. Neither does a self-closing tag
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
/// **`title` is deliberately absent.** SVG's `<title>` is an HTML integration
/// point by the spec, but [`scan_tags`] enters raw-text state for that name
/// unconditionally, so its contents never reach this walk as markup at all.
/// Listing it here would claim a fidelity the tokenizer below does not
/// provide. That divergence is one layer down and is recorded on `e77173`
/// rather than papered over here.
fn is_svg_html_integration_point(name: &str) -> bool {
    name.eq_ignore_ascii_case("foreignObject") || name.eq_ignore_ascii_case("desc")
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

/// The content mode for the children of `name`, opened inside `parent`.
fn child_content_mode(
    parent: ContentMode,
    name: &str,
    html: &str,
    span: (usize, usize),
) -> ContentMode {
    if name.eq_ignore_ascii_case("svg") {
        return ContentMode::Svg;
    }
    if name.eq_ignore_ascii_case("math") {
        return ContentMode::MathMl;
    }
    match parent {
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
fn walk_elements(html: &str) -> Walk {
    let tokens = scan_tags(html);
    let mut extents: Vec<ElementExtent> = Vec::new();
    // (name, index into `extents`, mode its CHILDREN are parsed in) for each
    // element still open. The third field is a STACK property, and it is a
    // [`ContentMode`] rather than the bool this walk carried until `e77173`:
    // foreign content contains islands of HTML — `foreignObject`, `desc`, the
    // MathML text integration points — so "am I foreign" is not answerable by
    // inheriting a flag downward. It is deliberately not `scan_tags`'
    // `foreign_depth`: that is a saturating counter with no stack scoping,
    // adequate only for choosing a CDATA terminator.
    let mut open_stack: Vec<(String, usize, ContentMode)> = Vec::new();
    let mut orphan_closes: Vec<(usize, usize)> = Vec::new();

    for tok in &tokens {
        match tok {
            TagToken::Open {
                name,
                self_closing,
                span,
            } => {
                // Before the push, and for void elements too: `<hr>` closes a
                // paragraph it never joins (R0002-0061).
                let implied = implicitly_closes(name);
                while open_stack
                    .last()
                    .is_some_and(|(top, _, _)| implied.contains(&top.as_str()))
                {
                    let (_, idx, _) = open_stack.pop().expect("just inspected the top");
                    extents[idx].content_end = span.0;
                }
                // HTML honours the self-closing flag in exactly TWO places:
                // inside foreign content, and on the `<svg>`/`<math>` start
                // tags that enter it. Everywhere else the `/` is a parse
                // error the parser IGNORES — the element opens, and the
                // author's end tag is a real closer rather than an orphan
                // the balancer deletes. Modelling it the way XML means it
                // cost exactly that deletion (ti 490d97 wave 1); before
                // that, `<div/>y</div>` lost its `</div>` and the fragment
                // went on to consume the sync wrapper's own one.
                //
                // Read AFTER the implied-close pops, so the parent is the
                // element this tag actually lands in.
                let mut parent_mode = open_stack.last().map_or(ContentMode::Html, |(_, _, m)| *m);

                // A breakout tag tears the parser back out to HTML before it
                // is processed: pop foreign elements until the thing we are
                // landing in parses its children as HTML. An integration
                // point already does, which is why this stops at one
                // (`<svg><foreignObject><div>` nests; `<svg><g><div>` does
                // not). Each popped element is closed implicitly, exactly as
                // an implied close does it (`e77173`).
                if parent_mode != ContentMode::Html && breaks_out_of_foreign(name, html, *span) {
                    while open_stack
                        .last()
                        .is_some_and(|(_, _, m)| *m != ContentMode::Html)
                    {
                        let (_, idx, _) = open_stack.pop().expect("just inspected the top");
                        extents[idx].content_end = span.0;
                    }
                    parent_mode = open_stack.last().map_or(ContentMode::Html, |(_, _, m)| *m);
                }

                let in_foreign = parent_mode != ContentMode::Html;
                // Raw text sits ABOVE the foreign rule on purpose:
                // `scan_tags` enters raw-text state for these four names
                // unconditionally, so the appended closer is what keeps
                // `balance_fragment` idempotent under its own re-scan
                // (DCR-0016 Part D).
                let honours_flag = !is_raw_text(name) && (in_foreign || is_foreign_root(name));
                if !is_void(name) && !(*self_closing && honours_flag) {
                    extents.push(ElementExtent {
                        name: name.clone(),
                        depth: open_stack.len(),
                        open: *span,
                        close: None,
                        content_end: html.len(),
                    });
                    open_stack.push((
                        name.clone(),
                        extents.len() - 1,
                        child_content_mode(parent_mode, name, html, *span),
                    ));
                }
            }
            TagToken::Close { name, span } => {
                if let Some(pos) = open_stack.iter().rposition(|(t, _, _)| t == name) {
                    // Everything above `pos` is closed implicitly by this tag.
                    for (_, idx, _) in open_stack.drain(pos + 1..) {
                        extents[idx].content_end = span.0;
                    }
                    let (_, idx, _) = open_stack.pop().expect("rposition found it");
                    extents[idx].close = Some(*span);
                    extents[idx].content_end = span.0;
                } else {
                    // Orphan close tag: dropping it is what keeps the sync
                    // wrapper's own `</div>` safe (spec 2026-08-03 §3.4).
                    orphan_closes.push(*span);
                }
            }
            // Comments, CDATA and bogus comments are not structure.
            TagToken::Skip { .. } => {}
        }
    }

    let unclosed = open_stack.into_iter().map(|(name, _, _)| name).collect();
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
/// text are content, and RCDATA / comment interiors are never tokenized by
/// [`scan_tags`] in the first place, so the strip cannot reach into them.
/// Borrows when nothing matched.
///
/// Malformed markup is tokenized the way a browser tokenizes it where the
/// two were measured to disagree (ti 549b20): a bare quote between
/// attributes is a NAME byte, never a value opener; a stray `=` in
/// attribute-name position STARTS an attribute named `=` rather than
/// opening a value, so the strip neither misses a plant hidden behind one
/// nor deletes text a browser paints after the tag's real end; and a tag
/// left unterminated at EOF is a passed-over [`TagToken::Skip`] region a
/// browser abandons. That exhausts the known divergences in the regions
/// this fix touched; two remain one state earlier — HTML's tag-name state
/// consumes `=` and quote bytes into the ELEMENT name until the first
/// whitespace, where this scanner ends the name earlier, and HTML's
/// end-tag-open state opens a bogus comment on `</` before a non-letter,
/// where this scanner sees plain text and keeps tokenizing — both
/// recorded in DCR-0032's 2026-08-21 amendment. Neither yields a
/// live-anchor construction through the pane path: a diverging element
/// name can never match a sanitizer's allowlist, and a browser mints no
/// element at all from a bogus comment's interior.
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

/// Byte ranges of the reserved attributes inside one open tag, appended to
/// `out` in ascending order. Walks HTML's attribute states directly rather
/// than reusing [`AttrState`], which answers a different question (where the
/// tag ends).
/// One attribute [`walk_attrs`] found: its lowercased name, where the name
/// starts, one past the whole `name="value"` run, and the value's own bytes
/// if it has a value.
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
/// deletes what it reports and `walk_elements` classifies foreign content by
/// it, so a disagreement would be a security question and a structure question
/// at once.
///
/// `span` is a [`TagToken::Open`] span, so `scan_tags` has already proved the
/// tag is well-formed enough to have a name.
fn walk_attrs(html: &str, span: (usize, usize), mut f: impl FnMut(AttrHit<'_>)) {
    let bytes = html.as_bytes();
    let (start, end) = span;
    // `end` is one past the `>`; never look at the `>` itself.
    let limit = end.saturating_sub(1);

    // Step over `<` and the tag name — `scan_tags` already proved one is here.
    let mut i = start + 1;
    while i < limit && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b':') {
        i += 1;
    }

    while i < limit {
        if bytes[i].is_ascii_whitespace() || bytes[i] == b'/' {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < limit && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' && bytes[i] != b'/' {
            i += 1;
        }
        if i == name_start {
            i += 1; // a stray `=`: not a name, and the walk must not stall
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

/// The value of `wanted` on this start tag, if it has one. Used for
/// `annotation-xml`, an HTML integration point only at two `encoding` values.
fn tag_attr_value(html: &str, span: (usize, usize), wanted: &str) -> Option<String> {
    let mut out = None;
    walk_attrs(html, span, |attr| {
        if out.is_none() && attr.name.eq_ignore_ascii_case(wanted) {
            out = attr.value.map(|(a, b)| html[a..b].to_string());
        }
    });
    out
}

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

/// Render-path auto-balancing (spec §3.4): tags opened but never closed in
/// the fragment are closed at its end, and orphan close tags are DROPPED.
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

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    for (s, e) in walk.orphan_closes {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    for name in walk.unclosed.iter().rev() {
        out.push_str("</");
        out.push_str(name);
        out.push('>');
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

    /// `Skip` reaches neither shipped consumer: `tag_inventory` filters it
    /// out and the balancer ignores it.
    #[test]
    fn skip_tokens_are_invisible_to_both_shipped_consumers() {
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

    /// The `Skip` spans of `html`, in document order.
    fn skips(html: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Skip { span } = token {
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
    /// The last arm is the new one: `in_foreign` is a STACK property, so past
    /// `</svg>` the flag is ignored again, and a fix that latched a counter
    /// instead would leave that `<span/>` unpushed.
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
        // `divc"` (its tag-name state eats the quote; that one-byte remainder
        // is the divergence already recorded in DCR-0032's 2026-08-21
        // amendment, ti e20490). Moving off ["div"] at all is the defect.
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

    /// Guards the ORDER of the push rule: `is_void` is checked first and
    /// globally, so the four parser-voids `VOID_ELEMENTS` gained in ti 490d97
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
        // A close tag whose element was already closed implicitly is an
        // orphan and goes the way of every orphan — dropped. A browser would
        // have turned it into the same phantom `<p></p>` this fix removes.
        assert_eq!(
            balance_fragment("<p>a<div>b</div>c</p>"),
            "<p>a<div>b</div>c"
        );
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
        assert_eq!(balance_fragment("<p>a</p><div "), "<p>a</p><div ");
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
