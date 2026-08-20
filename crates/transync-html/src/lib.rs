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
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
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
pub fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

/// Does `tag` hold raw text / RCDATA (`script`, `style`, `textarea`,
/// `title`)? A browser never tokenizes their content as markup, and HTML
/// ignores the self-closing flag on them (DCR-0016 Part D).
pub fn is_raw_text(tag: &str) -> bool {
    RAW_TEXT_ELEMENTS.contains(&tag)
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
    let collapse = matches!(blank_lines, BlankLinePolicy::Collapse);
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
    // (one Settings source, one text-type filter), so it is a debug check
    // rather than a new production failure mode.
    debug_assert_eq!(
        node_index.get(),
        plan.len(),
        "splice phase B counted a different number of text nodes than phase A"
    );

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
    /// Between attributes, or after a quoted value: `/` here is a marker.
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
    /// section (either terminator mode), or a bogus comment (`<!…>` / `<?…>`).
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
        while j < bytes.len() {
            let c = bytes[j];
            match attr {
                AttrState::Outside => match c {
                    b'>' => break,
                    b'"' | b'\'' => {
                        attr = AttrState::Quoted(c);
                        self_closing = false;
                    }
                    b'=' => {
                        attr = AttrState::BeforeValue;
                        self_closing = false;
                    }
                    b'/' => self_closing = true,
                    _ => self_closing = false,
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
            break; // unterminated tag: leave as-is
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
    let tokens = scan_tags(html);
    let mut open_stack: Vec<String> = Vec::new();
    let mut drop_spans: Vec<(usize, usize)> = Vec::new();

    for tok in &tokens {
        match tok {
            TagToken::Open {
                name, self_closing, ..
            } => {
                // Before the push, and for void elements too: `<hr>` closes a
                // paragraph it never joins.
                let implied = implicitly_closes(name);
                while open_stack
                    .last()
                    .is_some_and(|top| implied.contains(&top.as_str()))
                {
                    open_stack.pop();
                }
                // HTML ignores `/` on raw-text/RCDATA start tags: `<textarea/>`
                // still opens RCDATA and swallows every later sibling —
                // including the sync wrapper's own `</div>` — and DOMPurify
                // keeps `<textarea>`, so the pane tail collapses into a form
                // control. Treating them as open makes the balancer append the
                // close tag, and matches `scan_tags`, which already enters
                // raw-text state for `<script/>`/`<style/>` (DCR-0016 Part D).
                if !is_void(name) && (!self_closing || is_raw_text(name)) {
                    open_stack.push(name.clone());
                }
            }
            TagToken::Close { name, span } => {
                if let Some(pos) = open_stack.iter().rposition(|t| t == name) {
                    open_stack.truncate(pos);
                } else {
                    // Orphan close tag: dropping it is what keeps the sync
                    // wrapper's own </div> safe (spec §3.4).
                    drop_spans.push(*span);
                }
            }
            // Comments, CDATA and bogus comments are not structure: the
            // balancer must neither open nor close on them.
            TagToken::Skip { .. } => {}
        }
    }

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    for (s, e) in drop_spans {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    for name in open_stack.iter().rev() {
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

    #[test]
    fn self_closing_non_raw_text_elements_stay_closed() {
        // The void/self-closing invariant is unchanged for everything outside
        // the raw-text set: `<br/>` and `<span/>` get no appended close tag.
        assert_eq!(balance_fragment("<br/>after"), "<br/>after");
        assert_eq!(balance_fragment("<span/>after"), "<span/>after");
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

    #[test]
    fn a_slash_outside_any_attribute_value_still_self_closes() {
        // Void, so it must not be pushed for two independent reasons.
        assert_eq!(balance_fragment("<br/>"), "<br/>");
        // Non-void: only the self-closing flag can keep this balanced.
        assert_eq!(balance_fragment("<span/>tail"), "<span/>tail");
        assert_eq!(
            balance_fragment("<img src=\"x.png\"/><i>y"),
            "<img src=\"x.png\"/><i>y</i>"
        );
        // A detached slash is NOT a self-closing marker (HTML reconsumes it
        // in before-attribute-name state).
        assert_eq!(balance_fragment("<span / >tail"), "<span / >tail</span>");
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
}
