//! Differential proof of the **tree-construction** divergences from the retired
//! `htmlseg`: which element is open, and what closes it.
//!
//! `CLAUDE.md`'s `transync-html` paragraph, `docs/architecture/contracts.md`
//! §4a/§4b, DCR-0041, DCR-0043 and DCR-0051 all assert that this crate's walk
//! and balancer answer "which element does this closer close, and what is still
//! open at the end?" **differently from the pre-extraction `htmlseg`, on
//! purpose**, and that "a difference from it in any of the nineteen is the
//! **fix**, not a regression to restore". Those sentences are what stops the
//! next maintainer "repairing" a deliberate browser-fidelity change back into a
//! `contracts.md` §4a break.
//!
//! Until now they rested on prose. This file replaces the prose with
//! **execution**: every claim below is an assertion run against both
//! implementations at once.
//!
//! # Provenance
//!
//! `tests/reference/htmlseg_1d6f19d.rs` is
//! `git show 1d6f19d^:crates/transync-syntax/src/htmlseg.rs`, byte-for-byte.
//! `1d6f19d` is the extraction commit, so its parent is the last tree in which
//! `htmlseg` was the shipping code. Its own ~44 tests ride along inside the
//! vendored module and must keep passing: if they ever fail, the reference has
//! stopped being the code it claims to be and every verdict here is void.
//!
//! # The channel, and one correction to the received account
//!
//! The received account of the extraction says old `htmlseg` exposed exactly
//! four public items (`HtmlSegments`, `extract`, `splice`, `tag_inventory`) and
//! had **no** `balance_fragment`, so balanced output had to be observed
//! indirectly through an identity `splice`. **That is not what the vendored
//! bytes say.** Old `htmlseg` had `pub(crate) fn balance_fragment` — crate-
//! private, not absent — and old `splice` never called it (the render path
//! did). So an identity `splice` is *not* a channel onto the old balancer at
//! all, while `pub(crate)` inside a vendored module IS visible to this test
//! crate. Every balancer claim below is therefore proven on the **direct**
//! old-versus-new comparison of `balance_fragment`, which is a far sharper
//! instrument than the one the briefing described. `tag_inventory` (validation
//! layer 3's channel) and `element_extents` are used where the question is
//! about tokens or about which elements were opened.
//!
//! `element_extents` genuinely postdates the extraction — old `htmlseg` has no
//! such function — so where it is the only way to see *which* element a closer
//! closed, it pins current behaviour rather than establishing a divergence, and
//! the docstring says so.
//!
//! # What each test must do
//!
//! Assert the SUBSTANCE, not merely that two outputs differ: which closer was
//! deleted, which was invented, which element stayed open. A test that only
//! shows inequality would not tell a future reader whether the difference is
//! the fix or the regression.
//!
//! TRACE: ti 490d97 wave 1, ti e77173 / DCR-0041, ti 48f3c6 / DCR-0043
//! TRACE: DCR-0051 (ti 9b4d66, 307283, 895fb7, da6bb5, bb961a, a7e625)
//! TRACE: contracts.md §4a, §4b; ti 525bef (the deliberate residual)

#[allow(dead_code, clippy::all, clippy::pedantic)]
#[path = "reference/htmlseg_1d6f19d.rs"]
mod htmlseg_old;

use transync_html as new;

/// What one input's execution established on the balancer channel.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// The two balancers disagree on this input.
    Divergent,
    /// They agree — whatever the record describes, it is not observable here.
    Identical,
}

/// Balance one fragment with both implementations: `(old, new)`.
fn balancers(html: &str) -> (String, String) {
    (
        htmlseg_old::balance_fragment(html),
        new::balance_fragment(html),
    )
}

fn verdict(html: &str) -> Verdict {
    let (old, new_) = balancers(html);
    if old == new_ {
        Verdict::Identical
    } else {
        Verdict::Divergent
    }
}

/// Did the current walk record an element of this name as *opened*?
///
/// `element_extents` postdates the extraction, so this answers a question about
/// today's crate only. It is how "the tag opened an element" is separated from
/// "the tag's bytes survived", which the balancer's output alone cannot tell
/// apart.
fn opens_element(html: &str, name: &str) -> bool {
    new::element_extents(html).iter().any(|e| e.name == name)
}

// ---------------------------------------------------------------------------
// Provenance self-check
// ---------------------------------------------------------------------------

/// The vendoring's self-check for **this** file's channel: the old balancer
/// must still be the old balancer. `<div><span>x` is the narrowest fragment
/// that exercises push, no-pop and the appended-closer tail, and its expected
/// value is copied from the vendored module's own
/// `unclosed_tags_are_closed_at_the_end` test.
///
/// If this fails, the reference is not the code it claims to be and every
/// verdict in this file is void.
#[test]
fn the_vendored_reference_still_balances() {
    assert_eq!(
        htmlseg_old::balance_fragment("<div><span>x"),
        "<div><span>x</span></div>",
        "the vendored pre-extraction balancer must still close a trivially \
         unclosed fragment; if it does not, the reference has stopped being \
         `htmlseg` and nothing else here means anything"
    );
}

// ---------------------------------------------------------------------------
// Claim 1 — ti 490d97 wave 1 (2026-08-23): the self-closing slash is honoured
// in only the two places HTML honours it.
// ---------------------------------------------------------------------------

/// **Claim (ti `490d97` wave 1, 2026-08-23; contracts.md §4a "Opened means
/// what a browser means by it"):** HTML honours a start tag's self-closing `/`
/// in exactly two places — inside foreign content, and on the `<svg>`/`<math>`
/// start tags that enter it. Everywhere else the slash is a parse error the
/// parser ignores, so "a self-closing spelling of a non-void tag **counts as
/// opened** and the author's matching end tag is its real closer rather than an
/// orphan". Before the fix "the walk never pushed one, the author's `</div>`
/// was deleted as an orphan, and the fragment reached the pane still open".
///
/// `<div/>x</div>` is the smallest input that exhibits it: one non-void tag in
/// the self-closing spelling, one author-written closer, no strip and no
/// attributes involved — the record's own "reachable from a plain author-written
/// `<div/>`".
///
/// The assertion establishes both halves: that the two disagree, and that the
/// disagreement is precisely the author's `</div>` — deleted by `htmlseg`,
/// honoured as a real closer now.
#[test]
fn a_self_closed_div_now_opens_so_the_authors_closer_is_no_longer_deleted() {
    let html = "<div/>x</div>";
    let (old, new_) = balancers(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );
    assert_eq!(
        old, "<div/>x",
        "old htmlseg read the slash the way XML means it: the `<div/>` was never \
         pushed, so the author's `</div>` matched nothing and was DELETED as an \
         orphan — and the fragment then reached the pane with the div still open"
    );
    assert_eq!(
        new_, html,
        "transync-html ignores the slash in HTML content, so the div opens and \
         the author's own `</div>` closes it: the fragment is already balanced \
         and nothing is added or removed. If this ever regresses to `<div/>x`, \
         the pane's own wrapper `</div>` gets consumed and the next block's \
         anchor mounts inside this block — contracts.md §4a's direct-child break"
    );
    assert!(
        new::element_extents(html)
            .iter()
            .any(|e| e.name == "div" && e.close == Some((7, 13))),
        "the substance, stated on the walk rather than on the bytes: the author's \
         closer at 7..13 is that div's REAL close span, not an orphan"
    );
}

/// **Claim (same record):** the slash is honoured in **only** those two places.
///
/// This is the intra-crate discriminator for the word "only", and it needs
/// `element_extents` (which postdates the extraction) because the balancer's
/// bytes cannot distinguish "opened and closed by the author" from "never
/// opened". Three readings, one input each:
///
/// * `<div/>` in HTML content — slash ignored, element OPENS (claim 1 above).
/// * `<g/>` inside `<svg>` — foreign content, slash honoured, so the element
///   opens and closes at its own tag and records no extent, exactly like a void.
/// * `<svg/>` — the entering tag itself, slash honoured, so no `svg` extent and
///   the trailing `</svg>` has nothing to close.
///
/// The last of the three is where old and new AGREE (`htmlseg` self-closed it
/// for XML reasons, this crate for HTML's), which is reported rather than
/// hidden: not every clause of a record is a divergence.
#[test]
fn the_self_closing_slash_is_honoured_only_in_foreign_content_and_on_the_root() {
    assert!(
        opens_element("<div/>x</div>", "div"),
        "HTML content: the slash is a parse error the parser ignores, so the \
         element opens"
    );
    assert!(
        !opens_element("<svg><g/>x</svg>", "g"),
        "foreign content: the slash IS honoured, so `<g/>` opens and closes at \
         its own tag and mints no extent"
    );
    assert_eq!(
        new::balance_fragment("<svg><g/>x</svg>"),
        "<svg><g/>x</svg>",
        "and the fragment therefore needs no repair: no `</g>` is invented"
    );

    assert!(
        !opens_element("<svg/>x</svg>", "svg"),
        "the entering tag is the second honoured place: `<svg/>` opens no \
         foreign content at all"
    );
    assert_eq!(
        verdict("<svg/>x</svg>"),
        Verdict::Identical,
        "and this is the clause where the two agree — htmlseg honoured the slash \
         on every tag, so it happened to be right about this one. Reported, not \
         claimed as a divergence"
    );
}

/// **Claim (same record), read against `htmlseg`'s own words:** the vendored
/// module contains `a_slash_outside_any_attribute_value_still_self_closes`,
/// which asserts `balance_fragment("<span/>tail") == "<span/>tail"` under the
/// comment *"Non-void: only the self-closing flag can keep this balanced."*
/// That test is green in this run — it is one of the ~44 that ride along — so
/// the old contract is not being paraphrased here, it is being executed.
///
/// The fix inverts exactly that assertion. This test is the inversion, and it
/// exists separately from the `<div/>` case because the `<div/>` case could be
/// dismissed as being about the breakout list; `span` is on no list at all.
#[test]
fn a_self_closed_span_now_earns_the_closer_the_old_test_pinned_away() {
    let html = "<span/>tail";
    let (old, new_) = balancers(html);

    assert_eq!(
        old, "<span/>tail",
        "this is the vendored test's own expected value, re-executed here"
    );
    assert_eq!(
        new_, "<span/>tail</span>",
        "and this is the fix: the span is open at the end of the fragment, so \
         the balancer closes it there rather than letting it run into the \
         wrapper. `htmlseg`'s comment — only the self-closing flag can keep this \
         balanced — was a statement about XML, not about HTML"
    );
}

// ---------------------------------------------------------------------------
// Claim 2 — ti e77173 / DCR-0041 (2026-09-01): a content mode per open element,
// integration points, and foreign-content breakout.
// ---------------------------------------------------------------------------

/// **Claim (ti `e77173` / DCR-0041; contracts.md §4a "The foreign-content
/// carve-out is closed"):** a three-valued content mode per open element
/// replaced a single inherited `in_foreign` bool, and a breakout start tag pops
/// foreign elements. The record's exemplar is
/// `<div class="wrap"><svg><div>x</svg></div>`, and the claim is that this
/// closed a **live** §4a break: "a browser pops the `<svg>` at the `<div>`,
/// leaving that div open in HTML content, where it swallows the anchor of the
/// block that follows … The old walk closed the div at `</svg>`, saw a balanced
/// fragment, and passed it through with nothing to repair."
///
/// The assertion establishes all three halves — that `htmlseg` passed it
/// through untouched, that this crate appends exactly the one closer the
/// wrapper needs, and (the §4a harm, made executable without a browser) that
/// `htmlseg`'s output is **not a fixed point** of the browser-faithful
/// balancer: something in it is still open.
#[test]
fn a_breakout_div_inside_svg_no_longer_leaves_the_wrapper_unclosed() {
    let html = "<div class=\"wrap\"><svg><div>x</svg></div>";
    let (old, new_) = balancers(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );
    assert_eq!(
        old, html,
        "old htmlseg matched `</svg>` by name, which popped the inner div with \
         it: the stack came out empty, the fragment was declared balanced, and \
         it was passed through with NOTHING repaired"
    );
    assert_eq!(
        new_, "<div class=\"wrap\"><svg><div>x</svg></div></div>",
        "`div` is a breakout tag, so the `<svg>` is popped at it and the inner \
         div opens in HTML content, where the author's `</div>` closes it — \
         leaving only the wrapper div open, which the balancer closes. The \
         appended `</div>` IS the fix"
    );

    // The §4a harm itself, executable without a browser: run the
    // browser-faithful balancer over htmlseg's own output. A fragment with
    // nothing left open is a fixed point; this one is not.
    assert_ne!(
        new::balance_fragment(&old),
        old,
        "htmlseg's output still leaves an element open, so mounted in a pane it \
         consumes the sync wrapper's own `</div>` and the following block's \
         anchor stops being a direct child of `<main>` (contracts.md §4a)"
    );
    assert_eq!(
        new::balance_fragment(&new_),
        new_,
        "while this crate's output is a fixed point — nothing outlives the \
         fragment"
    );
}

/// **Claim (DCR-0041, "The self-closing flag was never the mechanism"):** "the
/// unflagged `<svg><div>x</svg>` does exactly the same thing, because `div` is
/// on the breakout list. A fix that handled only the flagged spelling would
/// have closed half of it."
///
/// So the flagged and unflagged spellings must agree with each other in the new
/// crate, and both must differ from `htmlseg`. Testing them as a pair is the
/// point: either one alone leaves the record's warning unchecked.
#[test]
fn the_self_closing_spelling_was_never_the_breakout_mechanism() {
    let unflagged = "<div class=\"wrap\"><svg><div>x</svg></div>";
    let flagged = "<div class=\"wrap\"><svg><div/>x</svg></div>";

    for html in [unflagged, flagged] {
        let (old, new_) = balancers(html);
        assert_eq!(
            old, html,
            "htmlseg passed {html:?} through untouched — the break is present in \
             both spellings"
        );
        assert_eq!(
            new_,
            format!("{html}</div>"),
            "and this crate repairs both the same way, by appending the one \
             `</div>` the wrapper needs. If only one spelling were repaired, the \
             fix would be half a fix (DCR-0041)"
        );
    }
}

/// **Claim (DCR-0041 exemplar 1):** "`<svg><foreignObject><div/>x` leaves that
/// div **open** in a browser: `foreignObject` re-enters HTML content, where the
/// self-closing flag is the parse error it always is. The walk treated it as
/// ordinary foreign content and closed the div." And its contrast, from the
/// same record: "which is why `<svg><foreignObject><div>` nests and
/// `<svg><g><div>` does not."
///
/// Both inputs are needed. The integration point is only meaningful against an
/// ordinary foreign element, or the test would pass on a crate that had simply
/// stopped treating anything as foreign.
#[test]
fn an_html_integration_point_returns_its_children_to_html_content() {
    let inside = "<svg><foreignObject><div/>x";
    let (old, new_) = balancers(inside);

    assert_eq!(
        old, "<svg><foreignObject><div/>x</foreignobject></svg>",
        "htmlseg read foreignObject as ordinary foreign content, so the \
         `<div/>` self-closed and earned no closer"
    );
    assert_eq!(
        new_, "<svg><foreignObject><div/>x</div></foreignobject></svg>",
        "the integration point returns its children to HTML content, where the \
         slash is ignored — so the div is open at the end and the balancer owes \
         it a `</div>`. The extra closer is the fix"
    );

    // The contrast: an ordinary foreign element does NOT nest a breakout div —
    // it is popped at it, and the div lands at top level.
    let outside = "<svg><g><div/>x";
    assert_eq!(
        new::balance_fragment(outside),
        "<svg><g><div/>x</div>",
        "`g` is no integration point, so `<div>` tears the parser back out to \
         HTML: the svg and the g are already closed at the breakout and only the \
         div needs a closer. htmlseg appended `</g></svg>` instead"
    );
    assert_eq!(
        htmlseg_old::balance_fragment(outside),
        "<svg><g><div/>x</g></svg>",
        "htmlseg's answer to the same input, for the record"
    );
    assert!(
        new::element_extents(outside)
            .iter()
            .any(|e| e.name == "div" && e.depth == 0),
        "and the div is at DEPTH 0 — outside the svg entirely, which is where a \
         browser puts it"
    );
}

/// **Claim (DCR-0041; contracts.md §4a):** "`annotation-xml` does so at exactly
/// its two HTML `encoding` values" — `text/html` and `application/xhtml+xml`,
/// "and at no other value".
///
/// The word doing the work is *exactly*, so the test needs all three readings:
/// both HTML values (children stay nested) and one non-HTML value (the `<div>`
/// is a breakout tag and tears out of MathML). `htmlseg` had no content model at
/// all, so it is **insensitive to the attribute** — that insensitivity is
/// asserted too, because it is what makes the new sensitivity a divergence
/// rather than a coincidence.
#[test]
fn annotation_xml_returns_children_to_html_at_exactly_its_two_html_encodings() {
    let html_encodings = [
        "<math><annotation-xml encoding=\"text/html\"><div>x</div></annotation-xml></math>",
        "<math><annotation-xml encoding=\"application/xhtml+xml\"><div>x</div></annotation-xml></math>",
    ];
    for html in html_encodings {
        assert_eq!(
            new::balance_fragment(html),
            html,
            "at an HTML encoding the children are HTML content, so the `<div>` \
             nests, the author's own closers suffice and the fragment is a fixed \
             point"
        );
        assert!(
            new::element_extents(html)
                .iter()
                .any(|e| e.name == "div" && e.depth == 2),
            "and the div really is nested two deep, inside the annotation-xml"
        );
    }

    let other = "<math><annotation-xml encoding=\"foo\"><div>x</div></annotation-xml></math>";
    let (old, new_) = balancers(other);
    assert_eq!(
        new_, "<math><annotation-xml encoding=\"foo\"><div>x</div>",
        "at any other value the children are MathML, so `<div>` is a breakout \
         tag: the math and the annotation-xml are popped at it and the trailing \
         `</annotation-xml></math>` close nothing and are deleted"
    );
    assert!(
        new::element_extents(other)
            .iter()
            .any(|e| e.name == "div" && e.depth == 0),
        "the div is at top level, not inside the annotation-xml"
    );
    assert_eq!(
        old, other,
        "htmlseg's reading is identical for all three encodings — it matched \
         `</annotation-xml>` and `</math>` by name and never read the attribute \
         at all. That is why the sensitivity above is a divergence"
    );
    assert_eq!(
        htmlseg_old::balance_fragment(html_encodings[0]),
        html_encodings[0],
        "same bytes out for the HTML encoding, confirming the insensitivity \
         rather than inferring it"
    );
}

// ---------------------------------------------------------------------------
// Claim 3 — ti 48f3c6 / DCR-0043 (2026-09-01): voidness is an HTML-content rule.
// ---------------------------------------------------------------------------

/// **Claim (ti `48f3c6` / DCR-0043):** "`is_void` decided whether a start tag
/// opens an element, globally — foreign content included. HTML's void list is an
/// HTML-content rule … So `<svg><link>a</link>` is a genuine, closable SVG
/// element, and the author's `</link>` was classified an orphan and **deleted**
/// from the pane — the balancer editing what the reader sees, in exactly the
/// direction it exists to prevent."
///
/// `<svg><link>a</link>` is the record's own smallest input. The assertion
/// establishes the deletion (old) against the real closer (new), and then uses
/// `element_extents` for the half the bytes cannot show: that in HTML content
/// `link` still opens **nothing**, so this is a context rule and not a change to
/// the void list.
#[test]
fn a_void_name_inside_foreign_content_opens_and_keeps_its_authors_closer() {
    let html = "<svg><link>a</link>";
    let (old, new_) = balancers(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );
    assert_eq!(
        old, "<svg><link>a</svg>",
        "htmlseg applied the void list inside foreign content, so `<link>` was \
         never pushed and the author's `</link>` was DELETED as an orphan — six \
         bytes of the reader's pane removed by the balancer"
    );
    assert_eq!(
        new_, "<svg><link>a</link></svg>",
        "in foreign content there is no void list: the link is a real, closable \
         SVG element, the author's `</link>` is its closer, and only the svg is \
         owed one"
    );
    assert!(
        opens_element(html, "link"),
        "the walk records the link as an opened element"
    );
    assert!(
        !opens_element("<div><link>a</link>", "link"),
        "and in HTML content it still opens nothing — DCR-0043 changed the \
         CONTEXT the void list is consulted in, not the list. `is_void` stays a \
         name-only predicate"
    );
}

/// **Claim (DCR-0043, "Why it stayed open, and why that reason expired"):** the
/// hazard that kept voidness global was the balancer appending `</br>`, which
/// HTML turns back into a fresh `<br>`. DCR-0041 retired it: "`br` is in
/// `FOREIGN_BREAKOUT_TAGS`, so `<svg><br>` tears out of foreign content
/// **before** the tag is processed and lands in HTML content, where `is_void`
/// still wins and nothing is ever pushed."
///
/// And its 2026-09-05 amendment records the reverse asymmetry: "`</br>` is now
/// never deleted as an orphan either. A browser turns it into a `<br>` START
/// tag, so removing the author's bytes removes a line break the reader would
/// have seen."
///
/// `<svg><br></br>` exhibits both in one input, which is why it is the one
/// chosen: the hazard (is a `</br>` invented?) and the amendment (is the
/// author's `</br>` kept?).
#[test]
fn the_br_hazard_that_kept_voidness_global_is_retired_by_breakout() {
    let html = "<svg><br></br>";
    let (old, new_) = balancers(html);

    assert_eq!(
        old, "<svg><br></svg>",
        "htmlseg deleted the author's `</br>` as an orphan and closed the svg"
    );
    assert_eq!(
        new_, html,
        "the `<br>` breaks OUT of foreign content first, so it is void again and \
         nothing is pushed — and the svg was popped at the breakout, so no \
         `</svg>` is owed either. The author's `</br>` stays because a browser \
         makes a `<br>` of it"
    );
    assert!(
        !new_.contains("</br></br>") && !opens_element(html, "br"),
        "and the hazard itself: no `</br>` is ever INVENTED, because the br is \
         never on the stack to be closed. If this fails, the balancer is minting \
         line breaks the source never had (DCR-0043's recorded trade)"
    );
}

// ---------------------------------------------------------------------------
// Claim 4 — DCR-0051 (2026-09-05): five at once.
// ---------------------------------------------------------------------------

/// **Claim 4(a) (DCR-0051 §1, ti `307283` / ti `bb961a`):** "In-body's 'any
/// other end tag' walks down from the current node and **stops at the first
/// special element**, ignoring the token. Without it, `</b>` in
/// `<div><b><div></b></div>` closed the inner `div`, the balancer called the
/// fragment balanced, and the pane's own `</div>` was consumed."
///
/// `<div><b><div></b></div>` alone does not discriminate: both implementations
/// leave the *bytes* untouched, differing only in what they append. The
/// smallest input that makes the divergence visible in the bytes is that
/// fragment with the author's own outer `</div>` written out —
/// `<div><b><div></b></div></div>` — because `htmlseg`, having spent both divs
/// on the `</b>`, runs out and DELETES the author's last closer.
///
/// Both inputs are asserted: the record's own, for the appended closers, and
/// the extended one, for the deletion.
#[test]
fn an_other_end_tag_stops_at_the_first_special_element_and_closes_nothing() {
    // The record's own input: same bytes out of both, different tails.
    let recorded = "<div><b><div></b></div>";
    let (old, new_) = balancers(recorded);
    assert_eq!(
        old, recorded,
        "htmlseg matched `</b>` by name, which popped the `b` AND the inner div \
         sitting on top of it; the outer div was then closed by the author's \
         `</div>` and the stack came out empty — 'balanced', with the pane's own \
         wrapper `</div>` left to be consumed"
    );
    assert_eq!(
        new_, "<div><b><div></b></div></b></div>",
        "the `</b>` closes NOTHING (the inner div is special, so the search stops \
         there and the token is ignored), the author's `</div>` closes the inner \
         div, and the `b` and the outer div are still open — so the balancer owes \
         `</b></div>`"
    );

    // The extended input, where the divergence reaches the author's bytes.
    let extended = "<div><b><div></b></div></div>";
    let (old_x, new_x) = balancers(extended);
    assert_eq!(
        old_x, "<div><b><div></b></div>",
        "htmlseg had already spent both divs, so the author's SECOND `</div>` \
         matched nothing and was deleted from the pane"
    );
    assert_eq!(
        new_x, extended,
        "this crate keeps all three closers: `</b>` inert, then one `</div>` per \
         open div. The fragment is a fixed point and no author byte moves"
    );
}

/// **Claim 4(b) (DCR-0051 §2, ti `307283` / R0010-0039, ti `da6bb5`):** "HTML's
/// **four scopes** … Table scope is what makes `<div><table>a</div>` leave the
/// table open: `table` terminates the search, a browser ignores the closer, and
/// the crate used to pop the table and let the pane's own `</div>` be ignored
/// the same way."
///
/// `<div><table>a</div>` is the record's own input and the smallest one: one
/// scope terminator, one closer that must be ignored because of it.
#[test]
fn table_scope_ignores_the_closer_and_leaves_the_table_open() {
    let html = "<div><table>a</div>";
    let (old, new_) = balancers(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );
    assert_eq!(
        old, html,
        "htmlseg popped the table at the nearest name match, so `</div>` closed \
         the div, the stack came out empty and the fragment was passed through \
         with nothing repaired — and in a pane the wrapper's own `</div>` was \
         then ignored the same way, foster-parenting the NEXT block's anchor \
         inside this block's wrapper (measured `parent-div`, R0010-0039)"
    );
    assert_eq!(
        new_, "<div><table>a</div></table></div>",
        "`div` is not in table scope because `table` terminates the search, so \
         the closer is ignored and its bytes are inert; both elements are still \
         open, and the appended `</table></div>` is what keeps the fragment from \
         reaching into the wrapper"
    );
    assert_eq!(
        new::balance_fragment(&new_),
        new_,
        "and the repaired fragment is a fixed point"
    );
}

/// **Claim 4(b), the other three scopes (DCR-0051 §2):** the four scopes decide
/// "the block-level end tags, `li`, `h1`–`h6` and the table ones".
///
/// One input per remaining scope, each built the same way — a scope terminator
/// (`table`) between the closer and its target:
///
/// * list-item scope, `</li>`;
/// * heading scope, `</h1>`.
///
/// Plus the control that keeps this honest: `<ul><li><div></li>`, where the
/// element in between is **not** a terminator, so the closer works and old and
/// new agree. Without the control, a crate that had simply stopped honouring
/// `</li>` would pass.
#[test]
fn list_item_and_heading_scopes_are_terminated_by_a_table_too() {
    let li = "<ul><li><table></li>";
    let (old_li, new_li) = balancers(li);
    assert_eq!(
        old_li, "<ul><li><table></li></ul>",
        "htmlseg matched `</li>` by name straight through the table"
    );
    assert_eq!(
        new_li, "<ul><li><table></li></table></li></ul>",
        "`li` is not in list-item scope past a `table`, so the closer is ignored \
         and all three elements are still open"
    );

    let h1 = "<div><h1><table></h1>";
    let (old_h1, new_h1) = balancers(h1);
    assert_eq!(
        old_h1, "<div><h1><table></h1></div>",
        "same name match for the heading end tag"
    );
    assert_eq!(
        new_h1, "<div><h1><table></h1></table></h1></div>",
        "and the same scope answer: ignored, everything still open"
    );

    let control = "<ul><li><div></li>";
    assert_eq!(
        verdict(control),
        Verdict::Identical,
        "the control: a `div` is no scope terminator, so `</li>` still closes the \
         li through it and both implementations agree. This is what makes the two \
         cases above about SCOPE rather than about `</li>` having been broken"
    );
    assert_eq!(
        new::balance_fragment(control),
        "<ul><li><div></li></ul>",
        "and the control's shared answer, stated rather than left implicit"
    );
}

/// **Claim 4(b), the implied-end-tag half (DCR-0051 §2, ti `da6bb5`):** "Button
/// scope is ti `da6bb5`'s second half: `<ul>` closes an open `<p>` *in button
/// scope*, which reaches through the `<em>` sitting on top of it. The crate
/// closed only the innermost frame." Also stated in contracts.md §4b: "HTML's
/// **implied end tags** applied by scope, in the one stack, so `<p><em><ul>`
/// closes the paragraph through the `<em>`."
///
/// `<p><em><ul>` is the record's own input. The old `implicitly_closes` table
/// only popped a name sitting at the TOP of the stack — its own docstring says
/// so — so the `<em>` blocked it.
#[test]
fn an_implied_close_by_scope_reaches_through_the_element_above_it() {
    let html = "<p><em><ul>";
    let (old, new_) = balancers(html);

    assert_eq!(
        old, "<p><em><ul></ul></em></p>",
        "htmlseg's implied close only fired while the name sat at the top of the \
         stack, so the `<em>` blocked it: p, em and ul were all still open and \
         all three earned closers"
    );
    assert_eq!(
        new_, "<p><em><ul></ul>",
        "the `<ul>` closes the paragraph in BUTTON scope, which reaches through \
         the `em` and takes it with it — so only the ul is open and only `</ul>` \
         is owed. Two fewer invented closers in the pane"
    );
}

/// **Claim 4(c) (DCR-0051 §3, ti `9b4d66` / ti `895fb7`):** "`</p>` and `</br>`
/// are **breakout end tags**: HTML lists them alongside the breakout START
/// tags, so they pop out of `<svg>`/`<math>` and are then reprocessed by the
/// HTML rules."
///
/// `<svg></p>x` and `<svg></br>x` are the smallest inputs: one foreign root, one
/// breakout end tag, nothing else. `htmlseg` deleted each closer as an orphan
/// and then closed the svg it should never still have been inside.
#[test]
fn close_p_and_close_br_are_breakout_end_tags_that_leave_foreign_content() {
    for html in ["<svg></p>x", "<svg></br>x"] {
        let (old, new_) = balancers(html);
        assert_eq!(
            old, "<svg>x</svg>",
            "htmlseg found no match for the closer, DELETED it, and then closed \
             the svg at the end of the fragment — the author's bytes edited and \
             the foreign root held open past where a browser leaves it \
             ({html:?})"
        );
        assert_eq!(
            new_, html,
            "the closer pops the svg before it is reprocessed, so nothing is \
             open at the end and nothing is owed — and the closer's own bytes \
             stay, because a browser makes content of each ({html:?})"
        );
    }
}

/// **Claim 4(c), the record's own chain (DCR-0051 §3):** "This is what
/// `<p><ul><svg></p><title>…` really does in a browser, and the crate reached
/// the same reading by accident and lost it the moment the balancer deleted the
/// `</p>` as an orphan."
///
/// The claim's phrasing matters and is worth quoting because it is easy to
/// misread: the divergence is **not** in how `<title>` is tokenized here. Both
/// implementations read it as RCDATA on this input — `htmlseg` because
/// `title` is unconditionally raw text, this crate because the `</p>` put the
/// parser back in HTML content — and the identical `tag_inventory` proves it.
/// The divergence is the `</p>` itself, and the appended `</svg>` that follows
/// from deleting it. That is the reading tested.
#[test]
fn the_breakout_close_p_is_what_makes_the_following_title_html_rcdata() {
    let html = "<p><ul><svg></p><title>";

    assert_eq!(
        htmlseg_old::tag_inventory(html),
        new::tag_inventory(html),
        "the token streams agree, so nothing here is about tokenizing `<title>`; \
         both read it as RCDATA and the `<title>`'s contents are text to both"
    );

    let (old, new_) = balancers(html);
    assert_eq!(
        old, "<p><ul><svg><title></title></svg></ul>",
        "htmlseg had already popped the `p` at the `<ul>`, so the author's \
         `</p>` matched nothing and was DELETED — and with the svg still on the \
         stack it also invented a `</svg>`. The right reading of `<title>` was an \
         accident it then threw away"
    );
    assert_eq!(
        new_, "<p><ul><svg></p><title></title></ul>",
        "the `</p>` is a breakout end tag: its bytes stay, it pops the svg, and \
         no `</svg>` is invented. `<title>` is HTML RCDATA on purpose rather than \
         by accident"
    );
}

/// **Claim 4(d) (DCR-0051 §4, ti `da6bb5`; contracts.md §4b):** "`caption`,
/// `col`, `colgroup`, `frame`, `tbody`, `td`, `tfoot`, `th`, `thead` and `tr`
/// outside a table are a parse error the parser IGNORES — no element, no frame.
/// Pushing one made the balancer append a `</td>` or `</tr>` to a fragment HTML
/// needs none for, which is inventing structure in a pane."
///
/// `<div><td>x` is the smallest input that shows the invented closer, and
/// `<table><td>x` is the control that keeps the claim narrow: *outside* a table
/// the tag is ignored, inside one it opens an element as it always did.
#[test]
fn a_td_outside_a_table_opens_no_element_and_earns_no_invented_closer() {
    let outside = "<div><td>x";
    let (old, new_) = balancers(outside);

    assert_eq!(
        old, "<div><td>x</td></div>",
        "htmlseg pushed a frame for the ignored start tag, so the balancer \
         invented a `</td>` — structure in the reader's pane that no source \
         document and no browser has"
    );
    assert_eq!(
        new_, "<div><td>x</div>",
        "the start tag is ignored, so there is no frame and no closer: only the \
         div is owed one"
    );
    assert!(
        !opens_element(outside, "td"),
        "and no `td` element is recorded at all"
    );

    let inside = "<table><td>x";
    assert_eq!(
        verdict(inside),
        Verdict::Identical,
        "the control: inside a table the same tag DOES open an element, and both \
         implementations agree. Without this, a crate that had simply dropped \
         `td` everywhere would pass"
    );
    assert_eq!(
        new::balance_fragment(inside),
        "<table><td>x</td></table>",
        "and the shared answer is that both closers are owed"
    );
}

/// **Claim 4(e) (DCR-0051 §5, ti `a7e625`):** "`mglyph` and `malignmark` are the
/// two start tags HTML's tree-construction dispatcher does *not* hand to the
/// HTML rules inside a MathML text integration point, so they stay MathML and a
/// `<script>` beneath one is a foreign element rather than a raw-text run. …
/// The crate read `<script>` as raw text, made `<div>` its text content, and
/// appended four closers a browser then ignores."
///
/// Two inputs, differing by exactly one tag, which is what makes this about the
/// exception rather than about raw text in general:
///
/// * `<math><mi><script><div>` — `mi` IS an integration point, so its children
///   are HTML, so `<script>` really is raw text and the `<div>` is text. Old and
///   new agree here.
/// * `<math><mi><mglyph><script><div>` — `mglyph` keeps its children in MathML,
///   so `<script>` is an ordinary foreign element and the `<div>` is markup that
///   breaks out to the `mi`.
///
/// Chromium's measured answer for the second is quoted in the record:
/// `<math><mi><mglyph><script></script></mglyph><div></div></mi></math>` with
/// `["math","mi","div"]` open.
#[test]
fn mglyph_and_malignmark_keep_their_children_in_mathml() {
    // The control: without the exception, `mi` hands its children to HTML and a
    // `<script>` beneath it is raw text. Both implementations agree.
    let control = "<math><mi><script><div>";
    assert_eq!(
        verdict(control),
        Verdict::Identical,
        "inside a text integration point the script IS raw text in both"
    );
    assert_eq!(
        new::balance_fragment(control),
        "<math><mi><script><div></script></mi></math>",
        "the `<div>` is script text, so it earns no closer, and three are owed"
    );
    assert!(
        !new::tag_inventory(control).iter().any(|t| t == "div"),
        "and the `<div>` is not a token at all there"
    );

    // The exception, for both names the dispatcher excepts.
    for (html, expected) in [
        (
            "<math><mi><mglyph><script><div>",
            "<math><mi><mglyph><script><div></div></mi></math>",
        ),
        (
            "<math><mi><malignmark><script><div>",
            "<math><mi><malignmark><script><div></div></mi></math>",
        ),
    ] {
        let (old, new_) = balancers(html);
        assert!(
            old.contains("</script>"),
            "htmlseg read the script as a raw-text run and invented a `</script>` \
             for it, plus closers for the mglyph/malignmark frame: {old:?}"
        );
        assert!(
            new::tag_inventory(html).iter().any(|t| t == "div"),
            "the `<div>` beneath the excepted element is MARKUP, not script \
             text — it is a token here and is not one in the control above"
        );
        assert_eq!(
            new_, expected,
            "the excepted element keeps its children in MathML, so the script is \
             an ordinary foreign element and the `<div>` breaks out to the `mi` — \
             matching Chromium's measured \
             `<math><mi><mglyph><script></script></mglyph><div></div></mi></math>` \
             with math/mi/div open. Three closers, not four, and none of them \
             `</script>`"
        );
    }
}

// ---------------------------------------------------------------------------
// Claim 5 — the narrowing of "orphan close tags are dropped".
// ---------------------------------------------------------------------------

/// **Claim (contracts.md §4b; DCR-0051 "An end tag that closes nothing is not
/// automatically an orphan"):** the line is the pane's.
///
/// * "**`Orphan`, deleted** — the search ran off the bottom of *this
///   fragment's* stack with nothing to stop it. … `<svg><g></div>` is the shape:
///   measured, the `</div>` closes the wrapper."
/// * "**`Inert`, kept** — a special element or a scope terminator *the fragment
///   itself contributes* stopped the search … deleting them would edit a
///   reader's pane for nothing."
///
/// So the narrowing has to be shown as a **pair**, or it reads as "the balancer
/// stopped deleting things". `<svg><g></div>` is the deletion that survived the
/// narrowing (and where old and new agree); `<div><table>a</div>` and the
/// `foreignObject` chain are the two keeps.
#[test]
fn a_closer_stopped_inside_the_fragment_is_kept_and_one_running_off_the_bottom_is_deleted() {
    // Still deleted, and both implementations agree it must be.
    let orphan = "<svg><g></div>";
    let (old, new_) = balancers(orphan);
    assert_eq!(
        old, new_,
        "the narrowing did not touch this case: a closer whose search runs off \
         the bottom of the fragment's own stack would reach the sync wrapper, so \
         it is still deleted"
    );
    assert_eq!(
        new_, "<svg><g></g></svg>",
        "the `</div>` bytes are gone and the two foreign elements are closed. If \
         this ever keeps the `</div>`, `<svg><g></div>` closes the pane's own \
         wrapper (measured, DCR-0051)"
    );

    // Kept: a scope terminator the fragment itself contributes.
    assert!(
        new::balance_fragment("<div><table>a</div>").contains("a</div></table>"),
        "the `</div>` in `<div><table>a</div>` is stopped by the fragment's own \
         `table` — a browser stops at the same frame, so the bytes are inert \
         there too and deleting them would edit the pane on an approximation"
    );

    // Kept: a special element the fragment itself contributes, on the input the
    // 400,000-case deep property run found (DCR-0051 §3).
    let chain = "<svg><foreignObject><p></foreignObject></svg></div>";
    let (old_c, new_c) = balancers(chain);
    assert_eq!(
        old_c, "<svg><foreignObject><p></foreignObject></svg>",
        "htmlseg name-matched `</foreignobject>` and `</svg>`, popping the `p` \
         with them, and then deleted the trailing `</div>` — leaving the `<p>` \
         open in the bytes it emitted"
    );
    assert_eq!(
        new_c, "<svg><foreignObject><p></foreignObject></svg></div></p></foreignobject></svg>",
        "here the foreign end tags close nothing (the current node is the HTML \
         `p`, which is special, so the search stops there) and neither does the \
         `</div>` — all three sets of bytes are inert and kept, and the balancer \
         closes what is really open. The `</div>` never reaches the wrapper, \
         which is §4a's break arriving THROUGH the repair meant to prevent it"
    );
}

/// **Claim (contracts.md §4b; DCR-0051):** "`</p>` and `</br>` are never
/// orphans at all: a browser turns each into content (`<p>a<div>b</div>c</p>`
/// renders `<p>a</p><div>b</div>c<p></p>`), so deleting one removes something
/// the author's own bytes render."
///
/// The record's own measured input is used for `</p>` — it is the one a browser
/// was actually asked about — plus the barest possible `x</br>y`, where there is
/// no open element anywhere and the closer still survives.
#[test]
fn close_p_and_close_br_are_never_orphans_because_a_browser_makes_content_of_each() {
    let recorded = "<p>a<div>b</div>c</p>";
    let (old, new_) = balancers(recorded);
    assert_eq!(
        old, "<p>a<div>b</div>c",
        "htmlseg had popped the `p` implicitly at the `<div>`, so the author's \
         `</p>` matched nothing and was deleted — removing the empty paragraph a \
         browser renders from it"
    );
    assert_eq!(
        new_, recorded,
        "the fragment is a fixed point: the `</p>` is kept and nothing is \
         appended. This deliberately reverses one line of R0002-0061, which was \
         about the balancer APPENDING a second `</p>`; deleting one the author \
         wrote is the different act of editing the pane"
    );

    let bare = "x</br>y";
    let (old_b, new_b) = balancers(bare);
    assert_eq!(
        old_b, "xy",
        "htmlseg deleted a `</br>` with nothing open anywhere — and a browser \
         mints a `<br>` from it, so a line break the reader would have seen was \
         removed"
    );
    assert_eq!(new_b, bare, "kept, for exactly that reason");
}

// ---------------------------------------------------------------------------
// Claim 6 — the deliberate absence: no adoption agency algorithm.
// ---------------------------------------------------------------------------

/// **Claim (contracts.md §4b "What is NOT modelled"; DCR-0051; ti `525bef`):**
/// "The adoption agency algorithm and the list of active formatting elements. A
/// formatting end tag goes through 'any other end tag' instead. The appended
/// redundant closer is a no-op in a browser, but *reconstruction* is not covered
/// by that argument: a browser rebuilds a `<b>` when it inserts the next
/// element, and that frame can stop an appended closer. Measured residual: 8 of
/// 10,000 generated fragments still break §4a this way, down from 1,358."
///
/// This test makes the **accepted residual executable** rather than only
/// narrated. `<b><div></b></div>` is the classic AAA input and the core of ti
/// `525bef`'s second census exemplar: a browser reparents, closing the `<b>`
/// and reconstructing it inside the div; this crate keeps the frame and appends
/// one redundant `</b>`-shaped closer.
///
/// Two things are pinned, and the second is the sharper one:
///
/// * the shape of the appended closer, so a future implementation of AAA fails
///   here loudly and deliberately rather than silently changing panes;
/// * that a formatting closer can still cost the author bytes —
///   `<b><i></b></i>` loses its `</i>` in **both** implementations, because
///   without an active-formatting-element list there is no reparenting to keep
///   it for. That is the residual's real shape, and it is not a divergence: it
///   is a thing both versions get wrong the same way.
///
/// The census figure itself (8 of 10,000) is **not** asserted here: it is
/// produced by `web/tests/html-oracle.spec.js` with Chromium in the loop, and
/// these tests must run without a browser. What is asserted is the mechanism the
/// figure counts.
#[test]
fn the_absent_active_formatting_element_list_is_pinned_rather_than_narrated() {
    let aaa = "<b><div></b></div>";
    let (old, new_) = balancers(aaa);

    assert_eq!(
        old, "<b><div></b>",
        "htmlseg name-matched the `</b>`, popping the div with it, and then \
         deleted the author's `</div>`"
    );
    assert_eq!(
        new_, "<b><div></b></div></b>",
        "this crate ignores the `</b>` (the div is special), closes the div with \
         the author's own closer, and appends ONE redundant `</b>`-shaped closer \
         for the formatting frame a browser has already moved. That appended \
         closer is the accepted residual — if this value changes, the adoption \
         agency algorithm has been implemented (ti 525bef) or the residual has \
         moved, and contracts.md §4b's census must be re-measured against \
         Chromium in the same commit"
    );
    assert_eq!(
        new::balance_fragment(&new_),
        new_,
        "the residual is at least stable: the output is a fixed point, so a pane \
         re-balanced twice does not grow"
    );

    // The sharper half: no reparenting means a formatting closer can still cost
    // the author bytes, in both implementations.
    let reparent = "<b><i></b></i>";
    assert_eq!(
        verdict(reparent),
        Verdict::Identical,
        "not a divergence — a shared limitation"
    );
    assert_eq!(
        new::balance_fragment(reparent),
        "<b><i></b>",
        "the `</b>` closes the `b` and takes the non-special `i` with it, so the \
         author's `</i>` runs off the bottom and is deleted. A browser reparents \
         instead (`<b><i></i></b><i></i>`), so the `</i>` is content there. This \
         is what 'the adoption agency algorithm is deliberately still absent' \
         costs, executed"
    );
}
