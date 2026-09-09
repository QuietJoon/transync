//! Differential proof of the **tokenizer-level** divergences from the retired
//! `transync-syntax::htmlseg` — how bytes become tokens.
//!
//! Companion to `divergence_from_htmlseg.rs`, which carries the mechanism's
//! full rationale. The short version, because a reader must not have to open
//! the other file to trust this one:
//!
//! - `tests/reference/htmlseg_1d6f19d.rs` is `git show
//!   1d6f19d^:crates/transync-syntax/src/htmlseg.rs`, **byte-for-byte**.
//!   `1d6f19d` extracted the HTML mechanics into `transync-html`, so its parent
//!   is the last tree in which `htmlseg` was the shipping code.
//! - It compiles unmodified because it never referenced its own crate: its
//!   whole dependency surface is `lol_html`, `htmlize` and `std`.
//! - Its own ~44 tests ride along inside the vendored module **in this binary
//!   too** and must keep passing. That is the provenance self-check: if they
//!   fail, the reference has stopped being the code it claims to be and every
//!   verdict here is void.
//!
//! # The two channels, and why these two
//!
//! Old `htmlseg`'s `scan_tags` was **private**, so no test may call it. Two
//! observable channels reach it, and both are load-bearing rather than
//! convenient:
//!
//! - [`inventories`] — `tag_inventory`, the lowercase tag-name sequence
//!   `transync-core`'s validation **layer 3** compares before and after a
//!   splice to decide whether the markup skeleton survived. A divergence
//!   visible here is a divergence that can accept or reject a translation.
//! - [`balanced`] — `balance_fragment`, the render-path auto-balancer whose
//!   output is what a **pane** contains. A divergence visible here is a
//!   divergence in what a reader sees, and in whether the sync wrapper's own
//!   `</div>` survives (`contracts.md` §4a).
//!
//! `balance_fragment` was `pub(crate)` in `htmlseg`, not absent: because the
//! vendored file is compiled *into this test crate*, `pub(crate)` resolves
//! here and the comparison is available. It is not part of the four-item
//! external surface, so a claim proven only through it is a divergence in the
//! render path rather than in the published API — which is exactly what the
//! records say those claims are.
//!
//! Where a claim's subject **postdates** the extraction — anything reached
//! through `strip_reserved_sync_attrs`, which `htmlseg` never had — there is
//! no old behaviour to diverge from. Those are marked `NOT_COMPARABLE` in the
//! report and pinned here as current behaviour anyway, because the behaviour is
//! worth pinning even when the comparison is not available.
//!
//! # Claims NOT re-proven here
//!
//! DCR-0050's first two — `RAW_TEXT_ELEMENTS` gaining `xmp`/`iframe`/
//! `noembed`/`noframes`, and `comment_end` learning `--!>` — are already
//! executed in `divergence_from_htmlseg.rs`
//! (`iframe_content_was_scanned_as_markup_and_is_now_raw_text` and
//! `the_bang_comment_close_hid_a_live_div_and_no_longer_does`). Duplicating
//! them would double-count the ledger they exist to make checkable.
//!
//! TRACE: ti 549b20, ti e20490, ti 415cdb, ti 2e2453, ti bebebe
//! TRACE: DCR-0032, DCR-0042, DCR-0050

#[allow(dead_code, clippy::all, clippy::pedantic)]
#[path = "reference/htmlseg_1d6f19d.rs"]
mod htmlseg_old;

use transync_html as new;

/// The same input through both `tag_inventory`s: `(old, new)`.
fn inventories(html: &str) -> (Vec<String>, Vec<String>) {
    (htmlseg_old::tag_inventory(html), new::tag_inventory(html))
}

/// The same input through both `balance_fragment`s: `(old, new)`.
fn balanced(html: &str) -> (String, String) {
    (
        htmlseg_old::balance_fragment(html),
        new::balance_fragment(html),
    )
}

/// The vendoring's self-check, stated as a test rather than left to the
/// reader: the reference must still do the job it did, through both channels
/// this file reads. If this fails, nothing else in the file means anything.
#[test]
fn the_vendored_reference_still_runs_on_both_channels() {
    let got = htmlseg_old::extract("<div><p>hello</p></div>").expect("old extract runs");
    assert_eq!(
        got.texts,
        vec!["hello".to_string()],
        "the vendored pre-extraction engine must still extract the one text node \
         in a trivial block; if this fails the reference is not the code it claims \
         to be and every verdict in this file is void"
    );
    assert_eq!(
        htmlseg_old::tag_inventory("<div>x</div>"),
        vec!["div".to_string(), "/div".to_string()],
        "the old inventory channel must still work"
    );
    assert_eq!(
        htmlseg_old::balance_fragment("<div><span>x"),
        "<div><span>x</span></div>",
        "the old balancer channel must still work"
    );
}

// ---------------------------------------------------------------------------
// ti 549b20 / DCR-0032 (2026-08-21): the three divergences that lived in one
// `AttrState::Outside` arm.
// ---------------------------------------------------------------------------

/// **Claim 1a (ti 549b20, DCR-0032 2026-08-21 amendment):** `scan_tags`'
/// `AttrState::Outside` arm was rewritten to tokenize **a stray quote** the
/// way a browser does. The landing commit states the rule: "a bare quote is a
/// name byte" — HTML's attribute-name state appends `"` to the attribute NAME
/// (a parse error, not a value opener), so the tag still ends at the first
/// `>`. Old `htmlseg` entered `AttrState::Quoted` there and hunted for a
/// matching quote, so every byte up to it — `>` included — was swallowed into
/// a phantom attribute value.
///
/// `<div a"><b>">` is the smallest input that exhibits it. It needs four
/// parts and no more: an attribute name byte (`a`) so the quote is *bare*
/// rather than a value's opener, the bare quote, a real element (`<b>`) in the
/// region the phantom value swallows, and a second quote plus `>` so the OLD
/// reading terminates its tag — without that last pair old runs off EOF and
/// the reading would be indistinguishable from claim 1c's.
///
/// Establishes: old buried a live `<b>` inside a phantom quoted attribute
/// value and reported it to nobody; new ends the `div` at the first `>` and
/// `<b>` is markup, which is what Chromium was measured to do.
#[test]
fn a_bare_quote_between_attributes_is_a_name_byte_not_a_value_opener() {
    let html = r#"<div a"><b>">"#;
    let (old, new_) = inventories(html);

    assert_ne!(
        old, new_,
        "ti 549b20 claims this input's reading changed; if old and new now agree, \
         either the fix was reverted or the vendored reference is not htmlseg"
    );
    assert_eq!(
        old,
        vec!["div".to_string()],
        "old htmlseg read the bare quote as a value opener, so `<b>` sat inside a \
         phantom attribute value and never became a token: {old:?}"
    );
    assert_eq!(
        new_,
        vec!["div".to_string(), "b".to_string()],
        "a bare quote is an attribute-NAME byte, so the `div` tag ends at the first \
         `>` and `<b>` is live markup — the browser reading. If this fails, the \
         scanner has gone back to opening a value on a bare quote and every byte \
         after one is hidden from layer 3, the strip and the balancer: {new_:?}"
    );
}

/// **Claim 1b (ti 549b20, DCR-0032 2026-08-21 amendment):** the same arm was
/// rewritten to tokenize **a stray `=`** the way a browser does. The landing
/// commit states the rule and the bit that carries it: "equals opens a value
/// only after a consumed attribute name (`has_attr_name` is the bit that tells
/// HTML's before-attribute-name state apart from attribute-name)". A `=` in
/// before-attribute-name position starts an attribute *named* `=`, and — the
/// observable half — a quote after it joins that NAME instead of opening a
/// value.
///
/// Why the discriminating input must contain `="`: with any non-quote byte
/// after the stray `=`, old's `BeforeValue`→`Unquoted` path and new's
/// attribute-name path end the tag at the very same `>`, so the two readings
/// are indistinguishable through every channel. The divergence *is* "does a
/// quote after a stray `=` open a value".
///
/// The control in the second half is what makes this a test of the stray-`=`
/// rule rather than of claim 1a's bare quote: spelling one attribute-name
/// byte before the `=` (`a="`) puts HTML in attribute-name state, where `=`
/// legitimately opens a quoted value — and there old and new agree exactly.
///
/// Establishes: old swallowed `<b>` into a value opened by a stray `=`; new
/// ends the tag at the first `>`; and the difference is conditioned on
/// `has_attr_name`, not on the quote alone.
#[test]
fn a_stray_equals_does_not_open_a_value_but_a_named_one_does() {
    let stray = r#"<div ="><b>">"#;
    let (old, new_) = inventories(stray);
    assert_eq!(
        old,
        vec!["div".to_string()],
        "old htmlseg went to BeforeValue on the stray `=` and then to Quoted on the \
         `\"`, so `<b>` was inside an attribute value: {old:?}"
    );
    assert_eq!(
        new_,
        vec!["div".to_string(), "b".to_string()],
        "before-attribute-name: the `=` starts a name, the quote joins it, and the \
         tag ends at the first `>` — so `<b>` is live markup: {new_:?}"
    );

    // The control: one name byte before the `=` makes it a REAL value opener,
    // and there is nothing to diverge about.
    let named = r#"<div a="><b>">"#;
    let (old_named, new_named) = inventories(named);
    assert_eq!(
        old_named, new_named,
        "with an attribute name consumed first, `=` opens a quoted value in both \
         implementations and `<b>` is inside it — this control is what pins the \
         divergence to the has_attr_name bit rather than to the quote"
    );
    assert_eq!(
        new_named,
        vec!["div".to_string()],
        "`a=\"><b>\"` is one attribute with a value containing `><b>`: {new_named:?}"
    );
}

/// **Claim 1c (ti 549b20, DCR-0032 2026-08-21 amendment):** the same arm was
/// rewritten to tokenize **an unterminated tag** the way a browser does. The
/// landing commit: "a tag with no closing angle bracket before EOF is pushed
/// as `TagToken::Skip` spanning to end of input before the scan stops",
/// because "a browser abandons a tag cut off before its `>` — it mints no
/// element and no attributes".
///
/// `<div>a<span` is the smallest input: one open element that will owe a
/// closer, and one tag cut off at EOF.
///
/// This claim's cost is **invisible to layer 3** and the landing commit says
/// so ("`tag_inventory` filters Skip by construction, so the layer-3 ledger is
/// untouched") — so the test asserts that agreement rather than pretending to
/// a divergence there, and reads the render channel for the real one.
///
/// Establishes: the two inventories are identical; the two *panes* are not.
/// Old left the truncated `<span` in the fragment and then appended `</div>`
/// AFTER it, where a browser reads those six bytes as part of the truncated
/// tag's attribute list — the wrapper's closer swallowed, `contracts.md` §4a.
/// New names the region a `SkipKind::UnterminatedTag`, deletes it (a browser
/// abandons it, so terminating would invent structure), and the appended
/// closer lands in markup.
#[test]
fn an_unterminated_tag_is_abandoned_rather_than_left_for_the_closer_to_fall_into() {
    let html = "<div>a<span";

    let (old_inv, new_inv) = inventories(html);
    assert_eq!(
        old_inv, new_inv,
        "the inventory channel cannot see this claim — Skip carries no name — and \
         the record says as much; a difference here would mean the layer-3 ledger \
         moved, which ti 549b20 explicitly did not do"
    );

    let (old, new_) = balanced(html);
    assert_eq!(
        old, "<div>a<span</div>",
        "old htmlseg emitted no token for the truncated tag and left its bytes in \
         place, so the appended closer landed inside them: {old}"
    );
    assert_eq!(
        new_, "<div>a</div>",
        "the abandoned tag's bytes are deleted and the closer is real markup. If \
         this fails, a fragment ending mid-tag can again bury the wrapper's own \
         `</div>` in an attribute list: {new_}"
    );

    // The substance behind the render difference: the region is NAMED, and it
    // is named as the thing a browser abandons.
    let tokens = new::scan_tags(html);
    let last = tokens.last().expect("the truncated tag produces a token");
    match last {
        new::TagToken::Skip {
            span,
            kind,
            terminated,
        } => {
            assert_eq!(*kind, new::SkipKind::UnterminatedTag);
            assert!(!*terminated, "a tag cut off at EOF is never terminated");
            assert_eq!(
                *span,
                (6, html.len()),
                "the region must cover the `<` of the truncated tag through EOF, or \
                 the balancer's truncate lands in the wrong place"
            );
        }
        other => panic!("expected a Skip region for the truncated tag, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// ti e20490 (2026-09-01): the TAG-NAME and END-TAG-OPEN states, and the one
// shared `tag_name_end`.
// ---------------------------------------------------------------------------

/// **Claim 2a (ti e20490, DCR-0032 2026-08-21 amendment):** HTML's TAG NAME
/// state "consumes every byte that is not whitespace, `/` or `>` into the
/// element name — quotes and `=` included — so `<divq"x=" data-sync-id="v">`
/// is an element named `divq"x="` carrying a real `data-sync-id`". Old
/// `htmlseg` stopped the name at the first byte outside `[A-Za-z0-9:-]` and
/// attribute-walked the remainder, which "buried that plant inside a phantom
/// quoted value".
///
/// The record's own example is the input, and it is minimal for the claim: the
/// name must contain a byte the old scanner rejected (`"`), the following
/// bytes must look like a quoted value to the old walk (`x="`), and the real
/// attribute must sit after the name's true end.
///
/// Establishes both halves of the boundary disagreement directly, as element
/// names: `divq` versus `divq"x="`. The strip half is **not comparable** —
/// `strip_reserved_sync_attrs` postdates the extraction — but is pinned below,
/// because it is the reason the boundary mattered: with the browser's name
/// boundary the reserved attribute is reachable and removed.
#[test]
fn the_tag_name_state_consumes_quotes_and_equals_into_the_element_name() {
    let html = r#"<divq"x=" data-sync-id="v">"#;
    let (old, new_) = inventories(html);

    assert_eq!(
        old,
        vec!["divq".to_string()],
        "old htmlseg ended the name at the first `\"`, so everything after it was \
         an attribute list and `data-sync-id` sat inside a phantom quoted value: \
         {old:?}"
    );
    assert_eq!(
        new_,
        vec![r#"divq"x=""#.to_string()],
        "HTML's tag-name state runs to the first whitespace, so the element is \
         named `divq\"x=\"` and the space begins a REAL attribute list. If this \
         fails, the scanner and `walk_attrs` disagree about the name boundary \
         again and a live anchor can hide behind one: {new_:?}"
    );

    // NOT_COMPARABLE half, pinned: with the browser's boundary the reserved
    // attribute is a name the strip examines, so it is cut. `htmlseg` had no
    // strip at all, so there is no old reading to compare this to.
    assert_eq!(
        new::strip_reserved_sync_attrs(html),
        r#"<divq"x=">"#,
        "the plant must be reachable as an attribute NAME; only the pane path's \
         DOMPurify mount ever caught the survivor, and a consumer that does not \
         sanitize never inherits that"
    );
}

/// **Claim 2b (ti e20490, DCR-0032 2026-08-21 amendment):** HTML's
/// END-TAG-OPEN state — `</` before anything that is not an ASCII letter opens
/// a **bogus comment** running to the first `>`, minting no element and no
/// attributes. The source records the cost of the old plain-text
/// fall-through: "the scanner emitted an `Open` for `</1 <div>x` and the
/// balancer then owed it a `</div>` a browser reads as orphan junk".
///
/// `</1 <div>x` is the record's own example and is minimal: `</` then a
/// non-letter, then an element inside the bogus comment's extent.
///
/// Establishes: old minted a live `div` out of bytes a browser passes over,
/// and then invented a closer for it in the pane; new mints nothing and leaves
/// the fragment alone.
#[test]
fn end_tag_open_before_a_non_letter_is_a_bogus_comment_not_an_element() {
    let html = "</1 <div>x";
    let (old, new_) = inventories(html);

    assert_eq!(
        old,
        vec!["div".to_string()],
        "old htmlseg stepped one byte past the `<` and then tokenized `<div>` as a \
         real start tag: {old:?}"
    );
    assert!(
        new_.is_empty(),
        "the bogus comment runs from `</` to the first `>`, which is the `>` of \
         `<div>`, so there is no markup in this fragment at all: {new_:?}"
    );

    let (old_pane, new_pane) = balanced(html);
    assert_eq!(
        old_pane, "</1 <div>x</div>",
        "the invented `div` earned an invented closer: {old_pane}"
    );
    assert_eq!(
        new_pane, html,
        "nothing was opened, so nothing is owed and the author's bytes pass \
         through: {new_pane}"
    );
}

/// **Claim 3 (ti 415cdb, 2026-09-01) — NOT_COMPARABLE, pinned.** "A stray `=`
/// where an attribute name would start BEGINS that name, per HTML's
/// before-attribute-name state."
///
/// The comparison is unavailable, and this test is what establishes that
/// rather than asserting it. ti 415cdb changed `walk_attrs`, which serves
/// `strip_reserved_sync_attrs` and `tag_attr_value` — neither existed in
/// `htmlseg`. The tokenizer half of the same rule had already landed with ti
/// 549b20 (claim 1b above), and the residual is strip-only: the first
/// assertion here shows old and new agreeing exactly on the record's own input
/// through the one channel both implementations have. The landing commit's own
/// framing matches ("root cause is drift between two attribute walks";
/// `scan_tags` was already aligned and "this one was never aligned with it").
///
/// So the behaviour is pinned instead. Both directions, because the record
/// insists on both: over-deletion was the defect (`<div =data-sync-id="x">y`
/// became `<div =>y`, mutating a NON-reserved attribute), and over-correcting
/// would be an under-deletion — the direction that leaves a live anchor
/// standing.
#[test]
fn a_stray_equals_before_an_attribute_name_is_a_strip_only_rule() {
    let html = r#"<div =data-sync-id="x">y"#;

    let (old, new_) = inventories(html);
    assert_eq!(
        old, new_,
        "the tokenizer agrees on this input — with no quote directly after the \
         stray `=`, old's Unquoted path and new's attribute-name path end the tag \
         at the same `>` — which is what makes ti 415cdb's residual strip-only and \
         its verdict NOT_COMPARABLE rather than a divergence from htmlseg"
    );
    assert_eq!(old, vec!["div".to_string()]);

    assert_eq!(
        new::strip_reserved_sync_attrs(html),
        html,
        "a browser reads ONE junk attribute named `=data-sync-id` here, so there is \
         no reserved attribute present and nothing may be cut. If this fails, the \
         strip is again mutating a non-reserved attribute, which its narrowed \
         guarantee says never happens"
    );

    assert_eq!(
        new::strip_reserved_sync_attrs(r#"<div =junk data-sync-id="x">y"#),
        "<div =junk>y",
        "and the rule must not blind the strip to a REAL reserved attribute beside \
         the junk one — over-correction here is an under-deletion, the direction \
         that actually leaves a live anchor in the pane"
    );

    // `walk_attrs` has a SECOND consumer that does reach the tokenizer —
    // `tag_attr_value`, which `child_content_mode` asks for `annotation-xml`'s
    // `encoding` (claim 5c below). So the NOT_COMPARABLE verdict has to survive
    // that route too, and it does: with the browser's rule the attribute is
    // named `=encoding`, no integration point is entered, and the CDATA section
    // ends at `]]>` — which is the reading old htmlseg had for every encoding,
    // because it had no integration-point rule at all. Both walk_attrs
    // consumers therefore agree with old on this input.
    let via_mode = r#"<math><annotation-xml =encoding="text/html"><![CDATA[x><b>]]>"#;
    let (old_mode, new_mode) = inventories(via_mode);
    assert_eq!(
        old_mode, new_mode,
        "the stray-`=` rule changes nothing old htmlseg could observe, on either of \
         walk_attrs' two routes"
    );
    assert_eq!(
        new_mode,
        vec!["math".to_string(), "annotation-xml".to_string()],
        "`=encoding` is a junk attribute name, so this is not an HTML integration \
         point and the `<b>` inside the CDATA section is character data: {new_mode:?}"
    );
}

// ---------------------------------------------------------------------------
// ti 2e2453 / DCR-0042 (2026-09-01): raw text and RCDATA are HTML-CONTENT
// states.
// ---------------------------------------------------------------------------

/// **Claim 4a (ti 2e2453 / DCR-0042):** "raw text and RCDATA are the
/// HTML-CONTENT states they are, so inside `<svg>`/`<math>`
/// `script`/`style`/`textarea`/`title` are ordinary foreign elements whose
/// contents are markup". Old `htmlseg` asked the NAME only — its
/// `is_raw_text(&name)` had no context term at all — so a `<title>` anywhere
/// entered RCDATA.
///
/// `<svg><title><div>x</div></title></svg>` is minimal: the foreign root, one
/// of the four names, and one element inside it whose fate is the whole
/// question. (`<svg><title>` is also an SVG **HTML integration point**, which
/// is why the `div` inside it is HTML content and genuinely live in a browser.)
///
/// Establishes: old hid `<div>`/`</div>` inside RCDATA; new reads them as the
/// markup a browser reads.
#[test]
fn raw_text_names_inside_foreign_content_hold_markup() {
    let html = "<svg><title><div>x</div></title></svg>";
    let (old, new_) = inventories(html);

    assert_eq!(
        old,
        vec![
            "svg".to_string(),
            "title".to_string(),
            "/title".to_string(),
            "/svg".to_string(),
        ],
        "old htmlseg entered RCDATA on the NAME `title`, so the `div` inside it was \
         never markup: {old:?}"
    );
    assert_eq!(
        new_,
        vec![
            "svg".to_string(),
            "title".to_string(),
            "div".to_string(),
            "/div".to_string(),
            "/title".to_string(),
            "/svg".to_string(),
        ],
        "inside foreign content `title` is an ordinary element and its contents are \
         markup. If this fails, everything planted inside `<svg><title>` is invisible \
         to the scanner again — including a live `data-sync-id`: {new_:?}"
    );
}

/// **Claim 4b (ti 2e2453 / DCR-0042):** the same sentence's second half —
/// inside `<svg>`/`<math>` those names' "self-closing `/` is honoured".
/// HTML ignores the flag on a raw-text start tag in HTML content, which is
/// what old `htmlseg` did unconditionally; foreign content is one of the two
/// places HTML honours it.
///
/// `<svg><title/>` is minimal, and the *balancer* is the channel that shows
/// it: whether the element opened is precisely whether a closer is owed. The
/// inventories cannot see it — a self-closing start tag is still one `Open` —
/// so this claim is proven in the pane, not in the layer-3 ledger.
///
/// Establishes: old treated `<title/>` as an opened RCDATA element and
/// invented a `</title>` for it (and swallowed everything up to a `</title>`
/// that never comes, per the second assertion); new honours the flag, so
/// nothing is owed.
#[test]
fn a_self_closing_raw_text_name_inside_foreign_content_opens_nothing() {
    let (old, new_) = balanced("<svg><title/>");
    assert_eq!(
        old, "<svg><title/></title></svg>",
        "old htmlseg pushed the element anyway — its balancer had an explicit \
         `|| is_raw_text(name)` term — and invented a closer: {old}"
    );
    assert_eq!(
        new_, "<svg><title/></svg>",
        "the `/` is honoured in foreign content, so `<title/>` opens nothing and no \
         `</title>` is invented in the pane: {new_}"
    );

    // The cost of not honouring it, on the inventory channel: old's RCDATA run
    // never ends, so the rest of the fragment stops being markup.
    let (old_inv, new_inv) = inventories("<svg><title/>a</svg>");
    assert_eq!(
        old_inv,
        vec!["svg".to_string(), "title".to_string()],
        "old htmlseg entered RCDATA on `<title/>` and hunted for a `</title>` that \
         is not there, so the author's `</svg>` was swallowed: {old_inv:?}"
    );
    assert_eq!(
        new_inv,
        vec!["svg".to_string(), "title".to_string(), "/svg".to_string(),],
        "with the flag honoured the `</svg>` is the real closer it looks like: \
         {new_inv:?}"
    );
}

/// **Claim 4c (ti 2e2453 / DCR-0042):** the same change "closed a second
/// `strip_reserved_sync_attrs` bypass: a `data-sync-id` planted inside
/// `<svg><title>` is live in a browser and used to survive the strip".
///
/// The strip is **not comparable** — `htmlseg` never had one — so the test
/// proves the *mechanism* of the bypass through the channel that is comparable
/// and pins the current strip beside it. The mechanism is decisive on its own:
/// the strip walks `scan_tags`' `Open` tokens, and old `scan_tags` emitted no
/// `Open` for this `div` at all, so no strip built on those tokens could have
/// reached the attribute regardless of how it was written.
#[test]
fn a_reserved_anchor_inside_svg_title_is_reachable_by_the_strip() {
    let html = r#"<svg><title><div data-sync-id="p-0001">x</div></title></svg>"#;

    let (old, new_) = inventories(html);
    assert!(
        !old.contains(&"div".to_string()),
        "old htmlseg emitted no Open token for the planted `div`, so the attribute \
         was unreachable from the token stream by construction: {old:?}"
    );
    assert!(
        new_.contains(&"div".to_string()),
        "the plant is a token now, which is the precondition for stripping it: \
         {new_:?}"
    );

    assert_eq!(
        new::strip_reserved_sync_attrs(html),
        "<svg><title><div>x</div></title></svg>",
        "and it is stripped. If this fails, an author-supplied anchor survives into \
         a mounted pane, where the engine's 'ours are the only sync attributes in \
         this DOM' stops being a construction"
    );
}

// ---------------------------------------------------------------------------
// ti bebebe / DCR-0050 (2026-09-05). (a) RAW_TEXT_ELEMENTS and (b) `--!>` are
// already executed in `divergence_from_htmlseg.rs`; (c) and (d) are here.
// ---------------------------------------------------------------------------

/// **Claim 5c (ti bebebe / DCR-0050):** "`tag_attr_value` decodes character
/// references through `htmlize::unescape_attribute` before
/// `child_content_mode` compares them, so `<annotation-xml
/// encoding="text&#47;html">` is the HTML integration point a browser makes
/// it."
///
/// The observable consequence chosen here is the **CDATA terminator**, because
/// it is the one that differs by content mode with no other rule in the way:
/// in foreign content `<![CDATA[…]]>` is a real CDATA section ending at
/// `]]>`, and in HTML content the same bytes open a bogus comment ending at
/// the FIRST `>`. So `<![CDATA[x><b>]]>` contains a live `<b>` if and only if
/// the parser is in HTML content there.
///
/// The test is a triple, and the triple is what makes it a test of the
/// *decode* rather than of integration points in general:
///
/// 1. `encoding="text/html"` — the literal spelling. New reads HTML content.
/// 2. `encoding="text&#47;html"` — the claim's spelling. New must read HTML
///    content too, which it can only do by decoding the reference before
///    comparing.
/// 3. `encoding="application/mathml+xml"` — not an integration point. New
///    stays in MathML, and agrees with old.
///
/// Old `htmlseg` has no integration-point concept at all: its `foreign_depth`
/// counts `<svg>`/`<math>` roots only, so it reads case 2 as foreign content
/// exactly as it reads case 3. That is why case 2's divergence from old is
/// attributable to the decode specifically — before DCR-0050 the crate
/// compared the raw bytes, landed in MathML, and matched old.
#[test]
fn annotation_xml_encoding_is_decoded_before_the_integration_point_test() {
    fn html_for(enc: &str) -> String {
        format!(r#"<math><annotation-xml encoding="{enc}"><![CDATA[x><b>]]>"#)
    }
    let foreign = vec!["math".to_string(), "annotation-xml".to_string()];
    let html_content = vec![
        "math".to_string(),
        "annotation-xml".to_string(),
        "b".to_string(),
    ];

    // (1) The literal spelling: an HTML integration point, so the CDATA bytes
    // are a bogus comment ending at the first `>` and `<b>` is live.
    let literal = html_for("text/html");
    let (old_lit, new_lit) = inventories(&literal);
    assert_eq!(
        old_lit, foreign,
        "old read it as a CDATA section: {old_lit:?}"
    );
    assert_eq!(
        new_lit, html_content,
        "an HTML integration point puts these bytes in HTML content, where they are \
         a bogus comment: {new_lit:?}"
    );

    // (2) THE CLAIM: the entity-encoded spelling must read the same as (1).
    let encoded = html_for("text&#47;html");
    let (old_enc, new_enc) = inventories(&encoded);
    assert_eq!(
        old_enc, foreign,
        "old htmlseg has no integration-point rule, so this is foreign content to \
         it: {old_enc:?}"
    );
    assert_eq!(
        new_enc, html_content,
        "`text&#47;html` IS `text/html` to a browser, so this is the same HTML \
         integration point as (1) and the `<b>` is live. If this fails, the crate \
         is comparing raw attribute bytes again and an entity-spelled encoding \
         hides a live element from the scanner, the strip and the balancer: \
         {new_enc:?}"
    );
    assert_eq!(
        new_enc, new_lit,
        "the two spellings must be indistinguishable after the decode"
    );

    // (3) The control: an encoding that is NOT an integration point keeps
    // MathML, and there new agrees with old — so the divergence in (1) and (2)
    // is the integration-point reading, and (2)'s is the decode.
    let other = html_for("application/mathml+xml");
    let (old_other, new_other) = inventories(&other);
    assert_eq!(
        old_other, foreign,
        "old is foreign content here too: {old_other:?}"
    );
    assert_eq!(
        new_other, foreign,
        "not an integration point, so MathML holds, the CDATA section ends at `]]>` \
         and the `<b>` is character data — the reading old had for every encoding: \
         {new_other:?}"
    );
}

/// **Claim 5d (ti bebebe / DCR-0050):** "`<plaintext>` in HTML content became
/// a `SkipKind::PlainText` region covering the start tag and every byte after
/// it … so the region is DELETED and DCR-0050 records why keeping the tag, or
/// keeping the bytes after it, is each a break rather than a repair." HTML's
/// PLAINTEXT state has no exit at all: no end tag, no character sequence,
/// nothing but EOF leaves it.
///
/// `<div><plaintext></div>` is minimal and is the source's own example: one
/// element that owes a closer, the `plaintext` start tag, and one end tag
/// after it whose fate is the claim. In a mounted pane that trailing `</div>`
/// stands for the **sync wrapper's own** closer.
///
/// Establishes three things at once: old read the following `</div>` as a tag
/// (new does not); the region is named, covers the start tag, and runs to EOF;
/// and the balancer DELETES it — old passed the whole fragment through
/// unchanged, which is what let a browser turn the wrapper's closer and every
/// following block into text (`contracts.md` §4a, invariant 7).
#[test]
fn plaintext_makes_every_following_byte_text_and_the_region_is_deleted() {
    let html = "<div><plaintext></div>";

    let (old, new_) = inventories(html);
    assert_eq!(
        old,
        vec![
            "div".to_string(),
            "plaintext".to_string(),
            "/div".to_string(),
        ],
        "old htmlseg walked div → plaintext → both closed at `</div>`: {old:?}"
    );
    assert_eq!(
        new_,
        vec!["div".to_string()],
        "every byte from the `<plaintext>` start tag onward is TEXT in a browser, so \
         the only tag in this fragment is the `div`: {new_:?}"
    );

    // The span covers the START TAG as well as the text after it — the point
    // DCR-0050 argues, because keeping the tag would leave the tokenizer
    // switch standing in the pane.
    let tokens = new::scan_tags(html);
    match tokens.last().expect("plaintext produces a token") {
        new::TagToken::Skip {
            span,
            kind,
            terminated,
        } => {
            assert_eq!(*kind, new::SkipKind::PlainText);
            assert!(
                !*terminated,
                "HTML's PLAINTEXT state has no exit, so the region is never terminated"
            );
            assert_eq!(
                *span,
                (5, html.len()),
                "the region must start at the `<` of `<plaintext>` and run to EOF; a \
                 span that starts after the tag would leave the switch in the pane"
            );
        }
        other => panic!("expected a PlainText Skip region, got {other:?}"),
    }

    let (old_pane, new_pane) = balanced(html);
    assert_eq!(
        old_pane, html,
        "old htmlseg passed the fragment through unchanged, leaving the tokenizer \
         switch in the pane: {old_pane}"
    );
    assert_eq!(
        new_pane, "<div></div>",
        "DELETED, not terminated — there are no bytes that close PLAINTEXT — and the \
         `div` is then closed honestly. If this fails, one `<plaintext>` in an html \
         block can again turn the sync wrapper's own `</div>` into text: {new_pane}"
    );
}

/// **Claim 5d, second half:** the `plaintext` rule is an **HTML-content** rule
/// like voidness (ti 48f3c6) and raw text (ti 2e2453) — "the switch is a
/// tree-construction rule of the 'in body' insertion mode, and foreign content
/// has none, so `<svg><plaintext>` is an ordinary foreign element".
/// `plaintext` is not a breakout tag, so nothing pulls it out of `<svg>` first.
///
/// The test is the boundary of claim 5d rather than a restatement of it: it is
/// what stops the region from being widened to every context, and old and new
/// AGREE here, so this arm is not one of the divergences.
#[test]
fn plaintext_is_an_html_content_rule_only() {
    let html = "<svg><plaintext>a</plaintext>";
    let (old, new_) = inventories(html);
    assert_eq!(
        old, new_,
        "inside foreign content `plaintext` is an ordinary element in BOTH \
         implementations — this arm is a boundary on the fix, not a divergence"
    );
    assert_eq!(
        new_,
        vec![
            "svg".to_string(),
            "plaintext".to_string(),
            "/plaintext".to_string(),
        ],
        "its contents are markup and its author's end tag is a real closer: {new_:?}"
    );
}

/// **Claim 6 (DCR-0050's deliberate exclusion):** "`noscript` is deliberately
/// still out, because HTML makes it raw text only under the scripting flag,
/// which is context rather than a name."
///
/// This is a claim about what the code deliberately does NOT do, so the test
/// exists to stop the exclusion being "fixed" by accident. It is not a
/// divergence: old and new agree, and the test asserts that too, so the
/// nineteen-divergence ledger cannot absorb it.
///
/// Ticket `16721e90` argues the exclusion is nonetheless a divergence from
/// browsers (every pane this crate feeds is mounted by JavaScript, so the
/// scripting flag IS set there). That argument is not this test's business:
/// if it is accepted, this test is the one that must be deliberately changed,
/// which is the whole point of pinning it.
#[test]
fn noscript_is_deliberately_not_raw_text() {
    assert!(
        !new::is_raw_text("noscript"),
        "the name predicate must keep answering the NAME question; the scripting \
         flag is context and belongs to the caller"
    );

    let html = "<noscript><div>x</div></noscript>";
    let (old, new_) = inventories(html);
    assert_eq!(
        old, new_,
        "old and new agree on `noscript`, so this is a pinned deliberate choice and \
         not one of the recorded divergences"
    );
    assert_eq!(
        new_,
        vec![
            "noscript".to_string(),
            "div".to_string(),
            "/div".to_string(),
            "/noscript".to_string(),
        ],
        "its contents are scanned as markup. If this fails, someone added `noscript` \
         to RAW_TEXT_ELEMENTS — read ti 16721e90 and DCR-0050 first: the change is \
         defensible but it is a DECISION, not a bug fix, and it moves the ledger"
    );
}
