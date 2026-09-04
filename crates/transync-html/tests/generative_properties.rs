//! Generated inputs for the hand-rolled tokenizer, walk and balancer
//! (OI-0046 sub-item 3 / R0009-0067).
//!
//! # Why this shape, and not a `cargo-fuzz` target
//!
//! **There is no CI in this repository** — `crates/transync/tests/
//! docs_gate_claims_drift.rs::the_repository_still_has_no_ci` is green — and
//! the pre-commit hook is the only automatic gate. The hook compiles test
//! targets (`clippy --all-targets`) and executes none; `scripts/smoke.sh` and
//! release-checklist step 5 run `cargo test --workspace`. A coverage-guided
//! harness would therefore run at **no** venue, and this repository already has
//! two measured records of what that becomes: `benchmark/lang-detect/` (one
//! `RESULTS.md`, never re-run) and `scripts/smoke-live-gate.sh anthropic`
//! (written, double-gated, never executed here). So the bounded run below is a
//! plain `#[test]` — no `#[ignore]`, no dependency, no lockfile entry — and the
//! deep run sits behind the double gate `regenerate_goldens` established in
//! `token_stream_pin.rs`.
//!
//! # Why generated inputs at all, given `token_stream_pin.rs`
//!
//! The pin is byte-exact and stronger than any property **on the inputs it
//! holds** — but it holds 41 of them, and every edge-case entry was *added by*
//! the defect it now guards, so the corpus is a record of divergences already
//! found rather than a search for the next one. What was missing was novel
//! input. The corpus is deliberately **not** re-read here: it lives in
//! `token_stream_pin.rs` and a second copy of that list would be a second
//! chance to miss an entry (`scripts/lib/rustdoc-gate.sh`'s argument, one crate
//! over). The three document-scale fixtures under `tests/fixtures/` *are* read,
//! because they are files rather than a literal list.
//!
//! # Determinism
//!
//! `splitmix64` over pinned seeds, `std` only. Same inputs on every run, on
//! every host — so a red here is a defect, never a flake, and it names the
//! exact input. This is the property `proptest` would have traded away, and in
//! a repository whose evidence culture is byte-exact goldens and two-bless
//! protocols the trade is the wrong way round.
//!
//! # What the first run found
//!
//! A live defect, on its first execution: `balance_fragment`'s orphan-deletion
//! pass welded a literal `<` onto the byte after the span it deleted, minting
//! markup — up to a live `data-sync-id` — out of untrusted text. It was
//! blessed BROKEN here first, in the protocol `token_stream_pin.rs` documents
//! (record today's wrong answer, fix, re-bless, review the diff between the
//! blessings), and it was the one class the bounded property run **excluded**.
//!
//! It is fixed. `deleting_an_orphan_no_longer_welds_a_literal_angle_bracket`
//! below is the same test with the same four inputs and the repaired answers,
//! and the exclusion is gone: no generated input is skipped now, so P1–P6 run
//! over the weld class like any other. A property that has to skip the bug it
//! found is not a property.
//!
//! TRACE: OI-0046 (R0009-0067)
//! TRACE: ti 95f55b
//! TRACE: ADR-0018 (architectural invariant 7)

use std::collections::BTreeSet;
use std::path::PathBuf;

use transync_html::{
    ElementExtent, SkipKind, TagToken, balance_fragment, element_extents, is_raw_text, is_void,
    scan_tags, strip_reserved_sync_attrs, tag_inventory,
};

// ---------------------------------------------------------------------------
// The generator
// ---------------------------------------------------------------------------

/// `splitmix64`. Nine lines of `std`, in place of a dev-dependency and the
/// nondeterminism that comes with one.
struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// The alphabet, one entry per decision the scanner, the walk or the balancer
/// makes — not a random byte soup. Random bytes reach the interesting states
/// too rarely to be worth a second of gate time; these atoms reach every one of
/// them, and `the_generator_reaches_every_state_the_september_fixes_lived_in`
/// is the assertion that they still do rather than the hope that they do.
///
/// The last four are not tags at all (`div`, `/div>`, `!-- z`,
/// `data-sync-id="p-9">`). They are there because a divergence can be created
/// by *deleting* bytes as well as by reading them: they are the right-hand side
/// of a weld, and they are what found OI-0046's defect. Removing one of them
/// would take the weld class out of the generator, which
/// `the_properties_hold_over_generated_input`'s weld census now refuses.
const ATOMS: &[&str] = &[
    // Ordinary HTML content.
    "<div>",
    "</div>",
    "<span>",
    "</span>",
    "<p>",
    "</p>",
    "<li>",
    "<ul>",
    "</ul>",
    "<b>",
    "</b>",
    "<i>",
    "</i>",
    // Void names, including the four parser-voids of ti 490d97 wave 1.
    "<br>",
    "<br/>",
    "<img src=\"x\">",
    "<hr>",
    "<meta>",
    "<keygen>",
    "<keygen/>",
    "<basefont>",
    // Self-closing spellings of non-void names (ti 490d97 wave 1).
    "<div/>",
    "<span/>",
    "<p/>",
    "<div / >",
    // Raw text and RCDATA (R0002-0020, ti 2e2453).
    "<script>",
    "</script>",
    "<style>",
    "</style>",
    "<textarea>",
    "</textarea>",
    "<title>",
    "</title>",
    "<script/>",
    "<textarea/>",
    "</scripty>",
    // Foreign roots and foreign content (DCR-0041 / 0042 / 0043).
    "<svg>",
    "</svg>",
    "<svg/>",
    "<math>",
    "</math>",
    "<math/>",
    "<rect/>",
    "<circle>",
    "</circle>",
    "<g>",
    "</g>",
    "<mrow>",
    // Integration points, including `annotation-xml` at and off its two
    // encodings.
    "<foreignObject>",
    "</foreignObject>",
    "<desc>",
    "<mi>",
    "</mi>",
    "<mtext>",
    "<annotation-xml encoding=\"text/html\">",
    "<annotation-xml>",
    "<annotation-xml encoding=\"image/svg+xml\">",
    // Breakout, and `font`'s attribute-conditional breakout.
    "<font>",
    "<font color=\"r\">",
    "</font>",
    "<foo/>",
    "<foo>",
    "</foo>",
    // Voidness in foreign content (ti 48f3c6) and the rename (ti e923ef).
    "<link>",
    "</link>",
    "<image>",
    "</image>",
    "<embed>",
    // The attribute machine's parse errors (ti 549b20, e20490, 415cdb).
    "<div \">",
    "<div a=\"x\"\">",
    "<div =\">",
    "<div=x>",
    "<div\"x>",
    "</div\"x>",
    "<a href=https://example.com/>",
    "<div/data-sync-id=\"p-1\">",
    "<div data-sync-id=\"p-2\">",
    "<div DATA-Order=\"3\">",
    "<div data-block-kind=\"paragraph\" class=\"k\">",
    "<div:x>",
    "</div:x>",
    // The tag-open and end-tag-open states (R0003-0066, ti e20490).
    "<1>",
    "<2026 rows>",
    "</1 >",
    "</1",
    "</ ",
    "</>",
    "<:x>",
    "< div>",
    "<",
    // Skipped regions, terminated and not (ti 95f55b, c1f9a8).
    "<!-- c -->",
    "<!--",
    "<!-- <div> -->",
    "<![CDATA[y]]>",
    "<![CDATA[y",
    "<![CDATA[<b>x</b>]]>",
    "<!doctype html>",
    "<!x>",
    "<!x",
    "<! <div> >",
    "<?pi>",
    "<?pi",
    // Tags cut off at EOF.
    "<div",
    "<div class=\"x",
    "<p",
    "<div /",
    "<div a=",
    // Text, including bytes the scanner must not read as markup.
    "x",
    "a < b",
    "\n\n",
    "  ",
    "e\u{0301}\u{2192}",
    "&amp;",
    "]]>",
    "-->",
    ">",
    // Not tags: the right-hand side of a weld.
    "div",
    "/div>",
    "!-- z",
    "data-sync-id=\"p-9\">",
];

/// Stale-frame PHRASES: whole multi-tag *shapes*, drawn like an atom
/// (ti `ec235f`).
///
/// `ATOMS` is one entry per decision the scanner, the walk or the balancer
/// makes, and concatenating ten of them uniformly does reach every one of
/// those decisions. What it cannot reach is a **shape**.
/// `<div><b><div></b></div>` is six particular atoms in one particular order:
/// at one chance in `ATOMS.len()` per draw that is about `10^-12` per input,
/// so the bounded run would need more iterations than this repository has
/// hours. And a shape is exactly where the four routes found by the
/// adversarial verification of ti `fdd989` lived — the finding that opened
/// ti `ec235f` named "the generated space did not reach the stale-frame
/// shapes" as one of its two gaps. So the shapes are drawn directly rather
/// than hoped for.
///
/// Four classes, each named by the coverage state it reaches and each
/// required by `the_generator_reaches_every_state_the_september_fixes_lived_in`
/// — a phrase list that quietly stopped producing a class would otherwise
/// leave this whole section decorative:
///
/// * `stale-frame:formatting-crosses-block` — a formatting element opened
///   inside one block and closed inside another. HTML answers these with the
///   adoption agency algorithm, which reparents nodes and leaves a **clone**
///   of the formatting element on the stack; this crate's walk closes the
///   nearest matching open frame and pops what sat above it. The two answers
///   differ, and tickets `307283` / `895fb7` are that difference.
/// * `stale-frame:breakout-inside-foreign` — a breakout tag, an integration
///   point or a raw-text name met inside `<svg>` / `<math>`, i.e. the
///   content-mode model DCR-0041 / 0042 / 0043 built. Ticket `9b4d66` is a
///   surviving disagreement in it.
/// * `stale-frame:non-innermost-close` — an end tag whose name matches a
///   frame that is not the innermost one, so something has to be popped
///   unclosed. HTML decides that with its scope rules; the walk decides it
///   with a nearest-match search.
/// * `stale-frame:implied-close` — the [`transync_html::implicitly_closes`]
///   pairs, spelled as real sequences: a `<p>` followed by a block-level
///   start tag, and the `<li>` / `<dt>` / `<dd>` / `<tr>` / `<td>` runs.
///
/// **Two families are deliberately absent, and that is a bound rather than an
/// oversight.** `<select>` / `<option>` and `<ruby>` / `<rt>` / `<rp>` are
/// `implicitly_closes` entries too, but their start tags put a browser into
/// insertion modes ("in select", ruby's own annotation handling) that this
/// crate models *not at all* — a flat stack has no notion of them. Generating
/// them would measure the size of that absence rather than the four classes
/// this ticket is about, and it would do it by flooding the browser oracle's
/// divergence census with one class nobody is working on. `<table>` is
/// included despite entering "in table" for the opposite reason: the ticket
/// names `<li>`/`<td>`/`<tr>` sequences explicitly, and foster parenting is
/// how a browser answers them.
const PHRASES: &[(&str, &str)] = &[
    // A formatting element crossing a block boundary. The first three are the
    // shapes tickets 307283 / 895fb7 own; the rest walk the same route through
    // `i`, `a` (which HTML gives its own extra rule), `em`, `font` and a
    // nested pair.
    ("stale-frame:formatting-crosses-block", "<b><div></b></div>"),
    (
        "stale-frame:formatting-crosses-block",
        "<div><b><div></b></div>",
    ),
    ("stale-frame:formatting-crosses-block", "<b><div></b>"),
    ("stale-frame:formatting-crosses-block", "<i><p>x</i></p>"),
    (
        "stale-frame:formatting-crosses-block",
        "<a href=\"x\"><div>y</a><div>z</a>",
    ),
    (
        "stale-frame:formatting-crosses-block",
        "<em><ul><li>q</em></li></ul>",
    ),
    (
        "stale-frame:formatting-crosses-block",
        "<font color=\"r\"><p></font></p>",
    ),
    (
        "stale-frame:formatting-crosses-block",
        "<b><i><div></b></i>",
    ),
    // Breakout tags, integration points and raw-text names inside foreign
    // content. The `<title>` pair is the shape ticket 9b4d66 lives in, and the
    // planted `data-sync-id` is there because that ticket's harm is an
    // impostor anchor rather than a lost closer.
    ("stale-frame:breakout-inside-foreign", "<svg><p>x</svg></p>"),
    (
        "stale-frame:breakout-inside-foreign",
        "<math><div>y</math></div>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<svg><foreignObject><p></foreignObject></svg>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<svg><title><div data-sync-id=\"p-3\">z</div></title></svg>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<math><mtext><b>q</mtext></math>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<svg><g><br></g></svg>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<svg><desc><div></desc>",
    ),
    (
        "stale-frame:breakout-inside-foreign",
        "<p><ul><svg></p><title><div>k</div></title></svg>",
    ),
    // An end tag matching a frame that is not the innermost one.
    (
        "stale-frame:non-innermost-close",
        "<div><span></div></span>",
    ),
    ("stale-frame:non-innermost-close", "<ul><li><b></ul>"),
    ("stale-frame:non-innermost-close", "<div><p><span></div>"),
    (
        "stale-frame:non-innermost-close",
        "<blockquote><div><span></blockquote>",
    ),
    ("stale-frame:non-innermost-close", "<table><tr><td></table>"),
    // The `implicitly_closes` pairs as sequences rather than as a table
    // lookup.
    ("stale-frame:implied-close", "<p><div></p>"),
    ("stale-frame:implied-close", "<p><ul><li>a<li>b</ul>"),
    ("stale-frame:implied-close", "<tr><td>a<td>b"),
    ("stale-frame:implied-close", "<td>x</tr>"),
    ("stale-frame:implied-close", "<dl><dt>a<dd>b</dl>"),
    ("stale-frame:implied-close", "<p><table><tr><td>c"),
];

/// One draw in this many reaches for a `PHRASES` entry instead of an `ATOMS`
/// entry. At `MAX_ATOMS` 10 that is about 0.7 phrases per input and roughly
/// half of all inputs carrying at least one — enough that every phrase is
/// drawn some hundreds of times over the bounded run, and few enough that the
/// atom-only space `the_generator_reaches_every_state_the_september_fixes_lived_in`
/// depends on is diluted by an eighth rather than replaced.
const PHRASE_ODDS: usize = 8;

/// The seeds. Pinned, because a moving seed makes a red unreproducible and
/// turns this file into the flake the deterministic generator exists to avoid.
const SEEDS: &[u64] = &[
    0x5EED_0046_0000_0001,
    0x5EED_0046_0000_0002,
    0xC1F9_A800_95F5_5B00,
    0xE204_9000_2E24_5300,
];

/// Inputs per seed for the bounded run. 4 x 4_000 = 16_000 inputs, ~1 s in a
/// debug `cargo test --workspace`. Sized to be affordable on **every** smoke
/// run rather than impressive once; the deep run below is where the iteration
/// count goes.
const BOUNDED_PER_SEED: usize = 4_000;

const MAX_ATOMS: usize = 10;

fn synth(rng: &mut Rng) -> String {
    let n = 1 + rng.below(MAX_ATOMS);
    let mut s = String::new();
    for _ in 0..n {
        // One draw in `PHRASE_ODDS` takes a whole stale-frame shape instead of
        // a single atom (ti `ec235f`). Two draws rather than one, so the atom
        // stream is diluted but not displaced.
        if rng.below(PHRASE_ODDS) == 0 {
            s.push_str(PHRASES[rng.below(PHRASES.len())].1);
        } else {
            s.push_str(ATOMS[rng.below(ATOMS.len())]);
        }
    }
    s
}

// ---------------------------------------------------------------------------
// Oracles — public API only, no second opinion about HTML
// ---------------------------------------------------------------------------

/// Names still open at the end of `html`: `walk_elements`'s `unclosed`,
/// reconstructed from the public surface. `content_end` starts at `html.len()`
/// and every closer — explicit, implied, or a breakout pop — overwrites it, so
/// an extent that still carries the initial value is one nothing closed.
fn unclosed(html: &str, extents: &[ElementExtent]) -> Vec<String> {
    extents
        .iter()
        .filter(|e| e.close.is_none() && e.content_end == html.len())
        .map(|e| e.name.clone())
        .collect()
}

/// Close-tag spans that closed nothing: `walk_elements`'s `orphan_closes`.
/// Every non-orphan `Close` becomes exactly one extent's `close`, so the ones
/// no extent claims are the orphans.
fn orphan_spans(html: &str) -> Vec<(usize, usize)> {
    let claimed: BTreeSet<(usize, usize)> = element_extents(html)
        .iter()
        .filter_map(|e| e.close)
        .collect();
    scan_tags(html)
        .into_iter()
        .filter_map(|t| match t {
            TagToken::Close { span, .. } => Some(span),
            TagToken::Open { .. } | TagToken::Skip { .. } => None,
        })
        .filter(|s| !claimed.contains(s))
        .collect()
}

/// Every `<` the scanner did **not** fold into a token — the ones a browser
/// emits as a literal character. `<<div>` has one; `<div>` has none.
fn literal_angles(html: &str) -> Vec<usize> {
    let covered: Vec<(usize, usize)> = scan_tags(html)
        .into_iter()
        .map(|t| match t {
            TagToken::Open { span, .. }
            | TagToken::Close { span, .. }
            | TagToken::Skip { span, .. } => span,
        })
        .collect();
    html.bytes()
        .enumerate()
        .filter(|(_, b)| *b == b'<')
        .map(|(i, _)| i)
        .filter(|i| !covered.iter().any(|(s, e)| i >= s && i < e))
        .collect()
}

/// `balance_fragment`'s first pass as it stood BEFORE OI-0046's fix: delete
/// every orphan close tag by concatenating the bytes on either side.
///
/// Deliberately still a plain delete. It is the counterfactual the two uses
/// below need — "would the unfixed pass have welded here?" — and the shipped
/// pass no longer answers that question, because it now replaces such a run
/// with one U+0020 instead (`transync_html::balance_fragment`'s seam rule).
fn plainly_orphan_stripped(html: &str) -> String {
    let mut out = String::new();
    let mut cursor = 0usize;
    for (s, e) in orphan_spans(html) {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    out
}

/// Would a PLAIN deletion of the orphan close tags change which `<` bytes are
/// literal characters? If it would, those bytes are a weld site: the deletion
/// welds a literal `<` onto a new neighbour and the remaining bytes stop
/// meaning what they meant.
///
/// This was the **exclusion** the bounded run applied while the defect below
/// was blessed broken — every byte it covered was a byte the properties
/// stopped checking
/// (128 of 400_000 generated inputs, 0.032%). It is no longer an exclusion:
/// the balancer holds the seam rule now, so a weld site is ordinary input and
/// the run below checks it like any other. Two uses remain, and both need the
/// pre-fix behaviour rather than the fixed one:
///
/// * the coverage state `orphan-deletion-weld`, so a generator that stopped
///   reaching the class cannot quietly stop testing it;
/// * P4's strip clause, which asks whether the STRIP created a weld site its
///   input did not have — a defect in the strip's own seam rule (ti 490d97
///   wave 1) whether or not the balancer would now survive it.
fn is_a_weld_site(html: &str) -> bool {
    literal_angles(&plainly_orphan_stripped(html)).len() != literal_angles(html).len()
}

/// The sync wrapper `render::html_pane` puts around a block's balanced HTML.
const WRAPPER_OPEN: &str = "<div data-sync-id=\"p-0001\">";

/// `contracts.md` §4a, as one question: mount the balanced fragment the way the
/// pane mounts it, and ask whether the wrapper's own `</div>` still closes the
/// wrapper.
///
/// This is the harm, not a proxy for it. Every §4a break this crate has fixed —
/// wave 1's `<div/>`, DCR-0041's `<svg><div>`, ti c1f9a8's unterminated trailing
/// region — was a fragment that consumed this closer, leaving the wrapper open
/// and nesting the next block's anchor inside this block's wrapper.
fn wrapper_survives(balanced: &str) -> bool {
    let pane = format!("{WRAPPER_OPEN}{balanced}</div>");
    element_extents(&pane).first().is_some_and(|e| {
        e.name == "div"
            && e.depth == 0
            && e.open == (0, WRAPPER_OPEN.len())
            && e.close == Some((pane.len() - "</div>".len(), pane.len()))
    })
}

/// Every property that must hold of `html`, or the first one that does not.
fn check(html: &str) -> Result<(), String> {
    // P1 — idempotence. ti 95f55b: a closer buried in an unterminated region
    // closes nothing, and each re-balance added another (15,726 violations in
    // 200,000 iterations). The render path balances once, but a fragment whose
    // balanced form is not a fixed point is a fragment whose bytes do not mean
    // what the balancer thought they meant.
    let once = balance_fragment(html);
    let twice = balance_fragment(&once);
    if once != twice {
        return Err(format!(
            "balance_fragment is not idempotent\n  once  = {once:?}\n  twice = {twice:?}"
        ));
    }

    // P2 — nothing outlives the fragment, and it carries no closer for
    // something never opened. The balancer's whole job (spec 2026-08-03 §3.4).
    let extents = element_extents(&once);
    let still_open = unclosed(&once, &extents);
    if !still_open.is_empty() {
        return Err(format!(
            "balanced output leaves {still_open:?} open\n  balanced = {once:?}"
        ));
    }
    let orphans = orphan_spans(&once);
    if !orphans.is_empty() {
        return Err(format!(
            "balanced output carries orphan closers at {orphans:?}\n  balanced = {once:?}"
        ));
    }

    // P3 — the pane's own closer survives. contracts.md §4a.
    if !wrapper_survives(&once) {
        return Err(format!(
            "the sync wrapper's own </div> does not close the wrapper\n  \
             balanced = {once:?}\n  pane     = {:?}",
            format!("{WRAPPER_OPEN}{once}</div>")
        ));
    }

    // P4 — the strip is idempotent, preserves the tag skeleton, and does not
    // itself weld. The last clause is ti 490d97 wave 1's fix ("replace the run
    // with one space wherever deleting it would weld bytes together") asserted
    // over generated input rather than over the five literals that motivated
    // it.
    let stripped = strip_reserved_sync_attrs(html).into_owned();
    if strip_reserved_sync_attrs(&stripped).into_owned() != stripped {
        return Err(format!(
            "strip_reserved_sync_attrs is not idempotent\n  once = {stripped:?}"
        ));
    }
    if tag_inventory(&stripped) != tag_inventory(html) {
        return Err(format!(
            "the strip moved the tag inventory\n  before = {:?}\n  after  = {:?}",
            tag_inventory(html),
            tag_inventory(&stripped)
        ));
    }
    // A comparison, not a predicate: since OI-0046's fix removed the weld
    // exclusion, the input itself may be a weld site, and inheriting one is
    // not the strip's defect. Creating one is.
    if is_a_weld_site(&stripped) && !is_a_weld_site(html) {
        return Err(format!(
            "the strip created a weld the input did not have\n  stripped = {stripped:?}"
        ));
    }

    // P5 — the composed pane chain, in the order `render.rs` calls it:
    // `balance_fragment(&strip_reserved_sync_attrs(md))`. Each half is checked
    // above; this is the seam between them, which is where wave 1's defect
    // actually reached a reader.
    let chained = balance_fragment(&stripped);
    if balance_fragment(&chained) != chained {
        return Err(format!(
            "the pane chain is not idempotent\n  chained = {chained:?}"
        ));
    }
    if !wrapper_survives(&chained) {
        return Err(format!(
            "the pane chain lets the fragment consume the wrapper's </div>\n  \
             chained = {chained:?}"
        ));
    }
    // P6 — anchor injection, which is what OI-0046's harm 4 was. "Ours are
    // the only sync attributes in this DOM" is a construction, not a scan
    // (OI-0035 route (c) / DCR-0033): the strip clears the namespace and
    // `render::attrs` re-injects it. So nothing downstream of the strip may
    // hand an element one back — and the balancer did, by welding
    // `<</b>div data-sync-id="p-0009">` into a live `div` out of bytes the
    // strip had correctly passed over as text.
    //
    // Asked through the strip itself rather than with a second attribute
    // reader: if the chained bytes still carry a reserved attribute the strip
    // has something left to cut, so a borrow-shaped no-op is the whole
    // assertion.
    if strip_reserved_sync_attrs(&chained).into_owned() != chained {
        return Err(format!(
            "the pane chain minted a reserved sync attribute the strip had \
             already cleared\n  chained = {chained:?}"
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// The coverage demonstration
// ---------------------------------------------------------------------------

/// Which of the states the September 2026 fixes lived in did this input reach?
fn reached(html: &str, out: &mut BTreeSet<&'static str>) {
    for token in scan_tags(html) {
        match token {
            TagToken::Open {
                self_closing, name, ..
            } => {
                out.insert(if self_closing {
                    "open:flagged"
                } else {
                    "open:plain"
                });
                if name == "img" {
                    out.insert("name:img");
                }
            }
            TagToken::Close { .. } => {
                out.insert("close");
            }
            TagToken::Skip {
                kind, terminated, ..
            } => {
                out.insert(match (kind, terminated) {
                    (SkipKind::Comment, true) => "skip:comment:terminated",
                    (SkipKind::Comment, false) => "skip:comment:eof",
                    (SkipKind::CdataSection, true) => "skip:cdata:terminated",
                    (SkipKind::CdataSection, false) => "skip:cdata:eof",
                    (SkipKind::BogusComment, true) => "skip:bogus:terminated",
                    (SkipKind::BogusComment, false) => "skip:bogus:eof",
                    // Terminated by construction impossible: a terminated tag
                    // is an Open or a Close.
                    (SkipKind::UnterminatedTag, _) => "skip:unterminated-tag",
                });
            }
        }
    }

    if !literal_angles(html).is_empty() {
        out.insert("literal-angle-bracket");
    }
    if !orphan_spans(html).is_empty() {
        out.insert("orphan-close");
    }

    let extents = element_extents(html);
    if !unclosed(html, &extents).is_empty() {
        out.insert("unclosed-at-eof");
    }
    if extents
        .iter()
        .any(|e| e.close.is_none() && e.content_end < html.len())
    {
        out.insert("implied-or-breakout-close");
    }
    for e in &extents {
        // A void name that opened an element can only have done so in foreign
        // content — ti 48f3c6.
        if is_void(&e.name) {
            out.insert("void-name-opens-in-foreign");
        }
        // A raw-text name with a child element likewise: in HTML content its
        // interior is never markup — ti 2e2453.
        if is_raw_text(&e.name) && extents.iter().any(|c| c.depth == e.depth + 1) {
            out.insert("raw-text-name-holds-markup-in-foreign");
        }
        if e.open.1 <= html.len()
            && literal_angles(html)
                .iter()
                .any(|i| *i >= e.open.1 && *i < e.content_end)
            && is_raw_text(&e.name)
        {
            out.insert("raw-text-swallows-markup-in-html");
        }
        if [
            "foreignobject",
            "desc",
            "mi",
            "mo",
            "mn",
            "ms",
            "mtext",
            "annotation-xml",
        ]
        .contains(&e.name.as_str())
        {
            out.insert("integration-point");
        }
    }
    for w in extents.windows(2) {
        if w[0].name == "svg" || w[0].name == "math" {
            if w[1].depth == w[0].depth {
                out.insert("breakout-to-html");
            } else if w[1].depth == w[0].depth + 1 {
                out.insert("foreign-child");
            }
        }
    }
    if is_a_weld_site(html) {
        out.insert("orphan-deletion-weld");
    }

    // The stale-frame classes (ti `ec235f`). Read LEXICALLY, by looking for the
    // phrase's own bytes, and deliberately not through `scan_tags` /
    // `element_extents`: a shape is a fact about the generated string, and
    // asking the functions under test whether their input contains a shape
    // they mis-model is the self-reference this ticket exists to remove.
    // `synth` concatenates a phrase whole, so `contains` is exact.
    for (class, bytes) in PHRASES {
        if html.contains(bytes) {
            out.insert(class);
        }
    }
}

/// The states every state must be in for the bounded run above it to mean
/// anything: reached.
///
/// This is not decoration. All nine `transync-html` fixes of 2026-09-01 …
/// 09-03 lived in exactly these states, and a generator that does not reach
/// them is a green tick over unreached code — strictly worse than an honest
/// record that nothing generates input at all. `orphan-deletion-weld` is here
/// for the same reason, with its meaning inverted by OI-0046's fix: it used to
/// be the class the bounded run **excluded**, and it is now the class the
/// bounded run **covers**. Either way, a generator that stopped producing it
/// would make that arrangement silently vacuous.
#[test]
fn the_generator_reaches_every_state_the_september_fixes_lived_in() {
    let required = [
        // The tokenizer's own states.
        "open:plain",
        "open:flagged",
        "close",
        "literal-angle-bracket",
        // The four unterminated-region kinds, and the three that can also be
        // terminated (ti 95f55b, ti c1f9a8).
        "skip:comment:terminated",
        "skip:comment:eof",
        "skip:cdata:terminated",
        "skip:cdata:eof",
        "skip:bogus:terminated",
        "skip:bogus:eof",
        "skip:unterminated-tag",
        // The content-mode model (DCR-0041 / 0042 / 0043).
        "foreign-child",
        "breakout-to-html",
        "integration-point",
        "void-name-opens-in-foreign",
        "raw-text-name-holds-markup-in-foreign",
        "raw-text-swallows-markup-in-html",
        "name:img",
        // The walk's repairs.
        "orphan-close",
        "unclosed-at-eof",
        "implied-or-breakout-close",
        // The class the bounded run used to exclude and now checks (OI-0046).
        "orphan-deletion-weld",
        // The stale-frame shapes `PHRASES` was added to reach (ti `ec235f`).
        // These four are what the browser oracle in
        // `web/tests/html-oracle.spec.js` is pointed at, so a generator that
        // stopped producing one would leave that suite green over a class it
        // never sees.
        "stale-frame:formatting-crosses-block",
        "stale-frame:breakout-inside-foreign",
        "stale-frame:non-innermost-close",
        "stale-frame:implied-close",
    ];

    let mut seen: BTreeSet<&'static str> = BTreeSet::new();
    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for _ in 0..BOUNDED_PER_SEED {
            reached(&synth(&mut rng), &mut seen);
        }
    }

    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|k| !seen.contains(k))
        .collect();
    assert!(
        missing.is_empty(),
        "the generator no longer reaches {missing:?}. Every property run in \
         this file is only as good as this list: an unreached state is a green \
         tick over untested code. Add an atom to ATOMS that reaches it, or — if \
         the state stopped existing — delete it here in the same commit that \
         deleted it from `lib.rs`.\n  reached: {seen:?}"
    );
}

// ---------------------------------------------------------------------------
// The bounded run
// ---------------------------------------------------------------------------

#[test]
fn the_properties_hold_over_generated_input() {
    let mut checked = 0usize;
    // No longer an exclusion, just a census: OI-0046's fix made the weld class
    // ordinary input, so these are counted to keep the anti-vacuity assertion
    // below honest rather than skipped.
    let mut weld_sites = 0usize;
    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for _ in 0..BOUNDED_PER_SEED {
            let src = synth(&mut rng);
            if is_a_weld_site(&src) {
                weld_sites += 1;
            }
            checked += 1;
            if let Err(why) = check(&src) {
                panic!(
                    "generated input violates a property.\n  input = {src:?}\n  {why}\n\n\
                     This is a DEFECT, not a flake: the generator is a pinned-seed \
                     splitmix64, so this input is reproducible on every host. \
                     Reproduce it alone with a one-line `#[test]` before touching \
                     anything, then decide whether `lib.rs` or the property is wrong."
                );
            }
        }
    }

    // Anti-vacuity. Every generated input is now checked — there is no
    // exclusion left to swallow one — so the count is exact, and the weld
    // census must be non-zero or the class OI-0046 was about is being reported
    // as covered while nothing generates it.
    assert_eq!(
        checked,
        SEEDS.len() * BOUNDED_PER_SEED,
        "the loop did not visit every generated input"
    );
    assert!(
        weld_sites > 0,
        "no generated input is a weld site any more, so the properties are \
         reporting OI-0046's class as covered while nothing reaches it. Either \
         the generator stopped producing `<` before an orphan close tag — add \
         an atom back — or `is_a_weld_site` stopped describing the pre-fix \
         behaviour it is the counterfactual for"
    );
}

/// The same properties over the crate's three document-scale fixtures, which
/// are real scenario documents rather than generated ones. Read as files, so no
/// list is duplicated (ti 4fb858 put them in-crate for exactly this reason).
#[test]
fn the_properties_hold_over_the_document_scale_fixtures() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let names = [
        "scn-15-html-blocks.md",
        "scn-14-full.md",
        "reader-honesty.md",
    ];
    for name in names {
        let path = root.join("tests/fixtures").join(name);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
        assert!(
            src.contains('<'),
            "{name} carries no HTML at all, so it checks nothing here"
        );
        if let Err(why) = check(&src) {
            panic!("{name} violates a property.\n  {why}");
        }
    }
}

/// The deep run: 400_000 inputs. `#[ignore]`d **and** env-var interlocked, the
/// double gate `token_stream_pin.rs::regenerate_goldens` established, so no
/// ordinary run — `--include-ignored` included — spends four seconds on it.
///
/// `TRANSYNC_HTML_DEEP_PROPERTIES=1 cargo test -p transync-html --release \
///   --test generative_properties deep -- --ignored --test-threads=4`
#[test]
#[ignore = "400_000 inputs; run explicitly with --ignored"]
fn the_properties_hold_over_a_deep_generated_run() {
    if std::env::var("TRANSYNC_HTML_DEEP_PROPERTIES").as_deref() != Ok("1") {
        panic!("set TRANSYNC_HTML_DEEP_PROPERTIES=1 to confirm the deep run");
    }
    let mut weld_sites = 0usize;
    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for _ in 0..100_000 {
            let src = synth(&mut rng);
            if is_a_weld_site(&src) {
                weld_sites += 1;
            }
            if let Err(why) = check(&src) {
                panic!("generated input violates a property.\n  input = {src:?}\n  {why}");
            }
        }
    }
    // 128 of these were skipped before OI-0046's fix; they are checked now.
    println!("deep run: 400000 inputs, {weld_sites} of them weld sites, 0 excluded");
}

// ---------------------------------------------------------------------------
// The browser-oracle corpus (ti ec235f)
// ---------------------------------------------------------------------------
//
// Everything above this line shares one blind spot, and it is not fixable by
// generating more input. Every oracle in this file — `unclosed`,
// `orphan_spans`, `literal_angles`, `wrapper_survives` — is built out of
// `scan_tags` / `element_extents` / `balance_fragment`, i.e. out of the
// functions under test. Where this crate's stacks and a browser's tree
// construction DISAGREE, both sides of every comparison above carry the same
// disagreement, so it cannot be seen from here at all. That is the finding
// that opened ti `ec235f`.
//
// The independent oracle is a real browser, and the repository already ships
// one: headless Chromium under `web/tests/`, driven by
// `scripts/test-browser.sh`. It cannot be called from a `#[test]`, so the
// seam is a file: this emitter writes the generated inputs **together with
// this crate's answers**, and `web/tests/html-oracle.spec.js` computes
// Chromium's answers to the same three questions and compares. Which side is
// which is stated there, per comparison, and stated once more in
// `emit_browser_oracle_corpus`'s own docs below.
//
// There is **no CI** here (`crates/transync/tests/docs_gate_claims_drift.rs
// ::the_repository_still_has_no_ci`), so this runs on demand like the rest of
// the browser suite, and it is double-gated exactly like `regenerate_goldens`
// and the deep run above: `#[ignore]`d AND env-var interlocked, so an ordinary
// `cargo test --workspace` neither writes a file nor needs a browser
// installed.

/// The pane mount, in the shape `render::html_pane` produces and
/// `contracts.md` §4a governs: one `<main>`, every block a direct child of it.
/// The balanced fragment goes inside the first block's wrapper, and the second
/// block is the **following sibling** whose survival §4a is about — every §4a
/// break this crate has fixed was a fragment that ate the first wrapper's own
/// `</div>` and pulled that sibling inside it.
const PANE_MAIN_OPEN: &str = "<main>";
const PANE_BLOCK1_OPEN: &str = "<div data-sync-id=\"p-0001\">";
const PANE_BLOCK1_CLOSE: &str = "</div>";
/// The following block. It carries `data-oracle-next` as well as its real
/// `data-sync-id` because the corpus deliberately contains inputs that plant
/// an impostor `data-sync-id` (that is ticket `9b4d66`'s harm), and a selector
/// that an input can forge identifies nothing. No atom and no phrase can
/// produce this name.
const PANE_BLOCK2: &str = "<div data-sync-id=\"p-0002\" data-oracle-next=\"1\">next</div>";
const PANE_MAIN_CLOSE: &str = "</main>";

fn pane_mount(balanced: &str) -> String {
    format!(
        "{PANE_MAIN_OPEN}{PANE_BLOCK1_OPEN}{balanced}{PANE_BLOCK1_CLOSE}{PANE_BLOCK2}\
         {PANE_MAIN_CLOSE}"
    )
}

/// This crate's answer to §4a's question about `pane_mount(balanced)`: is the
/// following block's anchor still a direct child of the `<main>`?
///
/// Returned as a `(bool, note)` pair rather than a tri-state, because the
/// **bool** is what Chromium is asked and the note is only a diagnostic. The
/// three notes are `ok`, `absent` (no element begins where the second block's
/// open tag was written — the fragment swallowed it) and `depth-N` (an element
/// does begin there, but nested N levels deep instead of 1).
fn next_block_is_direct_child(balanced: &str) -> (bool, String) {
    let pane = pane_mount(balanced);
    let next_at =
        PANE_MAIN_OPEN.len() + PANE_BLOCK1_OPEN.len() + balanced.len() + PANE_BLOCK1_CLOSE.len();
    match element_extents(&pane).iter().find(|e| e.open.0 == next_at) {
        // depth 0 is the `<main>`; its direct children sit at depth 1.
        Some(e) if e.depth == 1 => (true, "ok".to_string()),
        Some(e) => (false, format!("depth-{}", e.depth)),
        None => (false, "absent".to_string()),
    }
}

/// Inputs per seed handed to the browser oracle — a deterministic PREFIX of
/// each seed's stream, so the browser corpus is a subset of exactly the inputs
/// the bounded run above checks rather than a second, differently-generated
/// population.
///
/// The cap is real and is **reported** rather than applied silently, both in
/// this emitter's stdout and in the corpus's own `generator` object, which
/// `web/tests/html-oracle.spec.js` prints: a bound nobody logs reads as
/// "covered everything". 4 x 2_500 = 10_000 cases, in the same order of
/// magnitude as the 9_214 Chromium cases the ti `fdd989` verification used to
/// find the four routes.
const BROWSER_ORACLE_PER_SEED: usize = 2_500;

/// Inputs on which this crate and Chromium are **known** to disagree today.
///
/// They are not generated — they are the four routes the adversarial
/// verification of ti `fdd989` found, and they are emitted FIRST and
/// separately so `web/tests/html-oracle.spec.js` can hold each one to an exact
/// recorded disagreement. Tickets `9b4d66`, `307283` and `895fb7` own the
/// fixes; this file and that spec own only seeing them. It is expected and
/// correct that Chromium contradicts this crate here — that is the evidence
/// the oracle can go red at all, which an oracle that has never disagreed with
/// anything cannot offer.
const KNOWN_DIVERGENT: &[&str] = &[
    // ti 9b4d66 — the balancer deletes the orphan `</p>` that had taken the
    // walk out of foreign content, which puts `<title>` back inside `<svg>`,
    // turns its interior back into markup, and revives an impostor
    // `data-sync-id` the strip had correctly read as text.
    "<p><ul><svg></p><title><div data-sync-id=\"p-0002\">impostor</div></title></svg>",
    // ti 307283 / 895fb7 — the adoption agency algorithm. HTML leaves a clone
    // of `<b>` on the stack inside the inner `<div>`, so the author's `</div>`
    // closes the INNER div and the outer one is still open at EOF; the walk
    // closes the nearest matching frame and reports nothing open, so the
    // balancer appends nothing and the pane's own `</div>` is consumed.
    "<div><b><div></b></div>",
    // The same shape with the closer the author actually wrote: a browser ends
    // balanced, and the walk deletes that closer as an orphan.
    "<div><b><div></b></div></div>",
    // ti 895fb7 — foreign content left by a breakout END tag, with an
    // unterminated tag behind it. This one also breaks P1 above, so the
    // bounded run would catch it if the generator reached it; the browser
    // oracle is what says WHICH of the two answers is HTML's.
    "<p/><ul><math></p><div \">",
];

/// JSON string escaping, `std` only — this crate has no `serde` and gains no
/// dev-dependency for one writer (the `no [features]`, no-new-dependency
/// discipline `PHRASES` and `Rng` are already written under). Non-ASCII passes
/// through as UTF-8, which JSON permits and both readers accept.
fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn json_str_array(items: &[String], out: &mut String) {
    out.push('[');
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_str(item, out);
    }
    out.push(']');
}

/// One corpus record: the input, and **this crate's** answers to the three
/// questions the browser is about to answer independently.
fn json_case(input: &str, out: &mut String) {
    let balanced = balance_fragment(input);
    let (direct, note) = next_block_is_direct_child(&balanced);
    // The pane chain in the order `render.rs` calls it — the bytes a reader's
    // browser is actually handed. P5/P6 above are about exactly this string.
    let chained = balance_fragment(&strip_reserved_sync_attrs(input));

    out.push_str("{\"input\":");
    json_str(input, out);
    out.push_str(",\"balanced\":");
    json_str(&balanced, out);
    out.push_str(",\"pane\":");
    json_str(&pane_mount(&balanced), out);
    out.push_str(",\"chained\":");
    json_str(&chained, out);
    // Crate side of comparison (a): the names `walk_elements` says are still
    // open at the end of the raw input, and at the end of the balanced output
    // (where P2 says there must be none).
    out.push_str(",\"crate_open_at_input\":");
    json_str_array(&unclosed(input, &element_extents(input)), out);
    out.push_str(",\"crate_open_at_balanced\":");
    json_str_array(&unclosed(&balanced, &element_extents(&balanced)), out);
    // Crate side of comparison (b): §4a direct-child survival.
    out.push_str(",\"crate_next_is_direct_child\":");
    out.push_str(if direct { "true" } else { "false" });
    out.push_str(",\"crate_next_note\":");
    json_str(&note, out);
    // Crate side of comparison (c): P6, asked exactly the way `check` asks it
    // — through the strip itself. `true` means the strip still has something
    // in the reserved namespace to cut, i.e. this crate is ALREADY reporting
    // a violation for this input; the browser's job is to say whether that
    // report is a live impostor anchor or a false alarm.
    out.push_str(",\"crate_chained_carries_reserved\":");
    let dirty = strip_reserved_sync_attrs(&chained).into_owned() != chained;
    out.push_str(if dirty { "true" } else { "false" });
    out.push('}');
}

/// Write the browser-oracle corpus.
///
/// Double-gated (`#[ignore]` + `TRANSYNC_HTML_ORACLE_CORPUS`) so an ordinary
/// `cargo test --workspace`, `--include-ignored` included, neither writes a
/// file nor pulls a browser into the default gate.
/// `scripts/test-browser.sh` runs it into the served fixture directory:
///
/// `TRANSYNC_HTML_ORACLE_CORPUS=<dir>/corpus.json cargo test -p transync-html \
///   --test generative_properties emit_browser_oracle_corpus \
///   -- --ignored --test-threads=4`
///
/// **What travels, and which side of the comparison it is.** Every field this
/// writes is THIS CRATE's answer — `balance_fragment`, `element_extents`. Not
/// one Chromium answer is written here, and the spec that reads the file
/// computes every one of its own from `DOMParser`. The two sides meet only in
/// the spec's comparison, which is the whole point: an oracle whose expected
/// value came from the code under test is the self-reference ti `ec235f` is
/// about.
#[test]
#[ignore = "writes the browser-oracle corpus; run through scripts/test-browser.sh"]
fn emit_browser_oracle_corpus() {
    let Ok(dest) = std::env::var("TRANSYNC_HTML_ORACLE_CORPUS") else {
        panic!(
            "set TRANSYNC_HTML_ORACLE_CORPUS to the file this should write. \
             `scripts/test-browser.sh` points it at the served fixture \
             directory; nothing else is expected to run this test."
        );
    };

    let mut out = String::new();
    out.push_str("{\"generator\":{\"seeds\":[");
    for (i, seed) in SEEDS.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("\"{seed:#018x}\""));
    }
    out.push_str(&format!(
        "],\"browser_per_seed\":{BROWSER_ORACLE_PER_SEED},\
         \"bounded_per_seed\":{BOUNDED_PER_SEED},\"max_atoms\":{MAX_ATOMS},\
         \"phrase_odds\":{PHRASE_ODDS},\"atoms\":{},\"phrases\":{}}}",
        ATOMS.len(),
        PHRASES.len(),
    ));

    out.push_str(",\"known_divergent\":[");
    for (i, input) in KNOWN_DIVERGENT.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        json_case(input, &mut out);
    }
    out.push_str("],\"generated\":[");
    let mut written = 0usize;
    for seed in SEEDS {
        let mut rng = Rng(*seed);
        for i in 0..BOUNDED_PER_SEED {
            let src = synth(&mut rng);
            // The prefix, not a sample: `synth` has to be driven through the
            // whole bounded stream anyway for the emitted cases to be the same
            // inputs the bounded run checks, so the tail is generated and
            // dropped rather than never produced.
            if i >= BROWSER_ORACLE_PER_SEED {
                continue;
            }
            if written > 0 {
                out.push(',');
            }
            json_case(&src, &mut out);
            written += 1;
        }
    }
    out.push_str("]}");

    std::fs::write(&dest, &out)
        .unwrap_or_else(|e| panic!("the corpus should be writable at {dest}: {e}"));

    let generated = SEEDS.len() * BOUNDED_PER_SEED;
    assert_eq!(written, SEEDS.len() * BROWSER_ORACLE_PER_SEED);
    println!(
        "browser-oracle corpus -> {dest}\n  \
         {} known-divergent + {written} generated cases, {} bytes\n  \
         BOUND: {written} of the {generated} inputs the bounded run checks are \
         browser-checked (the first {BROWSER_ORACLE_PER_SEED} of each of the \
         {} seeds); the other {} are generated and DROPPED here",
        KNOWN_DIVERGENT.len(),
        out.len(),
        SEEDS.len(),
        generated - written,
    );
}

// ---------------------------------------------------------------------------
// The defect the first run found — now the regression it became
// ---------------------------------------------------------------------------

/// The defect OI-0046's first generative run found, kept as its own
/// regression.
///
/// It was **blessed BROKEN** here first — the two-bless protocol
/// `token_stream_pin.rs` documents, in `#[test]` form: record today's answer,
/// fix, and review the diff between the blessings as the record of the
/// movement. The four inputs are unchanged; only the answers moved. That is
/// the record, so the harm-by-harm structure below stays even though the whole
/// class is now covered by `the_properties_hold_over_generated_input` (the
/// exclusion that used to hide it is gone).
///
/// # The defect
///
/// `balance_fragment`'s first pass deleted orphan close tags by concatenating
/// the bytes on either side. When the byte before the deleted span was a
/// **literal `<`** — one the scanner correctly did not read as markup, because
/// what followed it was not an ASCII letter — the deletion welded that `<`
/// onto its new neighbour, and bytes that were text in the source became
/// markup in the pane.
///
/// It was the same welding hazard `strip_reserved_sync_attrs` was fixed for in
/// ti 490d97 wave 1 ("coalesce adjacent cuts and replace the run with one
/// space wherever deleting it would weld bytes together", pinned by
/// `a_cut_that_would_weld_bytes_leaves_one_space`), in the other function that
/// cuts bytes out of a fragment. That fix did not travel; now it has, under
/// one invariant stated in `orphan_cut_would_weld`: **deleting an orphan must
/// not change how any surviving byte tokenizes.**
///
/// # Why it mattered
///
/// Four harms, all of them measured by the arms below, all reachable from
/// untrusted source Markdown through a type-6 html block (invariant 7):
///
/// 1. **The wrapper's own `</div>` was consumed.** `<</b>x` balanced to `<x`,
///    which in a pane parses as an element named `x</div` — `contracts.md`
///    §4a's direct-child break, the same one wave 1 and DCR-0041 were each
///    fixed for as live breaks. `<x` was not even a fixed point: re-balancing
///    it deleted the content.
/// 2. **A comment was minted that swallows the rest of the pane.**
/// 3. **An end tag was minted that closes the wrapper early**, putting the
///    block's tail outside its own anchor.
/// 4. **A `strip_reserved_sync_attrs` bypass.** The render path is
///    `balance_fragment(&strip_reserved_sync_attrs(md))`. The strip runs first
///    and correctly leaves `div data-sync-id="p-0009">` alone — it is text,
///    not a tag. The balancer then welded it into a live `<div>` carrying a
///    live `data-sync-id`. Same class as ti e20490 and ti 2e2453, one stage
///    later. Neither layer was wrong in isolation; the composition was.
///
/// # What the fix does to each of them
///
/// One U+0020 in place of the run, so the literal `<` keeps a non-name byte
/// after it and stays the literal character it was: `< x`, `a< !-- …`,
/// `a< /div>tail`, `< div data-sync-id="p-0009">planted`. HTML's tag-open
/// state needs an ASCII letter, so `< d` mints nothing — the planted bytes
/// render as the text they always were, and P6 in `check` asks the strip
/// itself to confirm no element carries the namespace back.
#[test]
fn deleting_an_orphan_no_longer_welds_a_literal_angle_bracket() {
    // Harm 1 — the wrapper's closer is no longer consumed, and the output is
    // a fixed point of the function that produced it.
    assert_eq!(
        balance_fragment("<</b>x"),
        "< x",
        "the literal `<` keeps a non-name byte after it and mints no tag"
    );
    assert_eq!(
        balance_fragment("< x"),
        "< x",
        "and `< x` is a fixed point, so the content survives a re-balance"
    );
    assert!(
        wrapper_survives(&balance_fragment("<</b>x")),
        "the pane's own </div> closes the wrapper"
    );

    // Harm 2 — no minted comment, so nothing swallows the rest of the pane.
    assert_eq!(
        balance_fragment("a<</b>!-- swallow everything"),
        "a< !-- swallow everything",
        "`< !` is not HTML's markup-declaration-open: the bytes stay text"
    );
    assert!(wrapper_survives(&balance_fragment(
        "a<</b>!-- swallow everything"
    )));
    assert!(
        !scan_tags(&balance_fragment("a<</b>!-- swallow everything"))
            .iter()
            .any(|t| matches!(t, TagToken::Skip { .. })),
        "and the scanner sees no comment region at all"
    );

    // Harm 3 — no minted end tag.
    assert_eq!(
        balance_fragment("a<</b>/div>tail"),
        "a< /div>tail",
        "`< /` is not HTML's end-tag-open: the bytes stay text"
    );
    assert!(wrapper_survives(&balance_fragment("a<</b>/div>tail")));

    // Harm 4 — the strip bypass. Each half was right; now the composition is
    // too.
    let planted = "<</b>div data-sync-id=\"p-0009\">planted";
    assert_eq!(
        strip_reserved_sync_attrs(planted).into_owned(),
        planted,
        "the strip is correct here, as it always was: those bytes are text, \
         not an attribute"
    );
    let pane_bytes = balance_fragment(&strip_reserved_sync_attrs(planted));
    assert_eq!(
        pane_bytes, "< div data-sync-id=\"p-0009\">planted",
        "and the balancer no longer mints a tag out of bytes the strip \
         passed: `< d` is text in HTML's tag-open state"
    );
    assert!(
        !scan_tags(&pane_bytes)
            .iter()
            .any(|t| matches!(t, TagToken::Open { .. })),
        "no element at all is minted, so no element carries the plant"
    );
    assert_eq!(
        strip_reserved_sync_attrs(&pane_bytes).into_owned(),
        pane_bytes,
        "asked through the strip itself: there is nothing left in the \
         reserved namespace for it to cut"
    );
    assert!(wrapper_survives(&pane_bytes));

    // The seam rule fires only where it must. This is the anti-vacuity arm
    // the `assert_ne!`s held while the test was blessed broken: a "fix" that
    // padded every orphan cut with a space would satisfy every assertion
    // above and silently edit every fragment in the corpus.
    assert_eq!(
        balance_fragment("tail</div>text"),
        "tailtext",
        "an ordinary orphan cut is still a plain deletion"
    );
    assert_eq!(
        balance_fragment("<</b> x"),
        "< x",
        "and a cut whose neighbour cannot extend the `<` is too — one space \
         here is the source's own, not the rule's"
    );
}
