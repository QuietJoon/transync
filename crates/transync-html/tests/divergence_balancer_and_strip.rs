//! Differential proof for the BALANCER and the STRIP claims — executed, not read.
//!
//! Companion to `divergence_from_htmlseg.rs`, which carries the vendoring
//! rationale in full. The short version, because every verdict below depends on
//! it: `tests/reference/htmlseg_1d6f19d.rs` is
//! `git show 1d6f19d^:crates/transync-syntax/src/htmlseg.rs` **byte-for-byte**,
//! `1d6f19d` being the commit that extracted the HTML mechanics into
//! `transync-html`. So the vendored module is the last shipping tree in which
//! `htmlseg` was the code, and a disagreement between it and `transync_html`
//! today is a real old-vs-new divergence rather than a story about one.
//!
//! # What this file settles, and what it cannot
//!
//! The claims in this slice come from `docs/architecture/contracts.md` §4a and
//! `DCR-0047`, and they split cleanly in two once you ask which of them
//! `htmlseg` could even have been wrong about:
//!
//! * `balance_fragment` **existed** in `htmlseg` — as `pub(crate)`, therefore
//!   reachable from a test inside the same crate, which is what an integration
//!   test that `#[path]`-includes the module is. So the balancer claims are
//!   genuinely comparable, function to function, and they are executed that way
//!   below. `the_pre_extraction_engine_did_have_a_fragment_balancer` pins that
//!   premise so a future reader does not have to take it on trust.
//! * `strip_reserved_sync_attrs` **did not exist**, in any form, through any
//!   channel a caller had. `the_pre_extraction_engine_had_no_reserved_attribute_strip`
//!   proves that by execution over *every* entry point the old module offered.
//!   Claims about the strip are therefore `NOT_COMPARABLE`: they are fixes to
//!   code `htmlseg` never had, and DCR-0032's own phrase for it is that the
//!   strip "landed ahead of its callers". They are still pinned here, because
//!   the behaviour is worth pinning even when the comparison is not available.
//!
//! # How a "swallowed closer" is judged
//!
//! Nowhere below is the assertion merely "the two outputs differ". A closer is
//! *buried* when a browser-faithful reading of the balanced bytes cannot see it
//! as an end tag, so the test asks exactly that: it runs the CURRENT scanner
//! (`transync_html::tag_inventory`, the same inventory `transync-core`'s
//! validation layer 3 compares) over the OLD balancer's output and shows the
//! appended `/div` is absent — and present over the new balancer's output. That
//! is "a buried closer closes nothing", stated as an executable question.
//!
//! TRACE: ti 3f0879ef
//! TRACE: ti 95f55b, ti c1f9a8 (DCR-0047), ti 490d97 wave 1, ti 415cdb
//! TRACE: docs/architecture/contracts.md §4a

#[allow(dead_code, clippy::all, clippy::pedantic)]
#[path = "reference/htmlseg_1d6f19d.rs"]
mod htmlseg_old;

use std::borrow::Cow;
use transync_html as new;

/// The six names `RESERVED_SYNC_ATTRS` holds. Restated here on purpose: the
/// constant is private, and a test that imported it could not notice the set
/// silently shrinking. If a name is added to the crate and not to this list,
/// `the_strip_never_leaves_a_reserved_name_in_an_open_tag` simply stops
/// covering it — which is why the list is visible rather than borrowed.
const RESERVED: &[&str] = &[
    "data-sync-id",
    "data-block-kind",
    "data-order",
    "data-fallback",
    "data-parent-id",
    "data-skipped",
];

/// Both balancers' output for one input, old first.
fn balanced(html: &str) -> (String, String) {
    (
        htmlseg_old::balance_fragment(html),
        new::balance_fragment(html),
    )
}

/// Does a browser-faithful reading of `html` see `name` as an END tag?
///
/// This is the whole "is the closer buried?" question. It deliberately uses the
/// CURRENT scanner for both sides: the old scanner's opinion of the old
/// balancer's output is self-consistent by construction, and self-consistency
/// is not the property at issue — agreeing with a browser is.
fn closer_is_visible(html: &str, name: &str) -> bool {
    let want = format!("/{name}");
    new::tag_inventory(html).contains(&want)
}

/// A minimal two-block pane: one sync wrapper around `block`, then a second
/// wrapper that is the next block's anchor. This is the shape `contracts.md`
/// §4a constrains — "every following block is a DIRECT CHILD" — so a break in
/// it is the break the records describe, not an analogy for one.
fn pane(block: &str) -> String {
    format!("<div data-sync-id=\"b1\">{block}</div><div data-sync-id=\"b2\">y</div>")
}

/// Nesting depth of the element whose open tag carries `data-sync-id="<id>"`,
/// or `None` when a browser-faithful walk mints no such element at all.
///
/// `None` and `Some(depth > 0)` are the two shapes of the §4a break and they
/// are genuinely different harms: `None` means the anchor was swallowed into a
/// passed-over region and never exists in the DOM, `Some(1)` means it exists
/// but hangs inside the previous block's wrapper.
fn anchor_depth(pane: &str, id: &str) -> Option<usize> {
    let needle = format!("data-sync-id=\"{id}\"");
    new::element_extents(pane)
        .into_iter()
        .find(|e| pane[e.open.0..e.open.1].contains(&needle))
        .map(|e| e.depth)
}

// ---------------------------------------------------------------------------
// Section 0 — provenance, and the structural partition this slice exists to
// settle. These three tests are the premises every verdict below rests on.
// ---------------------------------------------------------------------------

/// The vendoring's self-check. If the reference cannot still do the job it
/// shipped doing, it is not the code it claims to be and nothing else in this
/// file means anything. `extract` is the narrowest end-to-end exercise of the
/// old engine's `lol_html` rewriter path.
#[test]
fn the_vendored_reference_still_extracts() {
    let got = htmlseg_old::extract("<div><p>hello</p></div>").expect("old extract runs");
    assert_eq!(
        got.texts,
        vec!["hello".to_string()],
        "the vendored pre-extraction engine must still extract the one text node \
         in a trivial block; if this fails the reference has stopped being \
         `1d6f19d^:crates/transync-syntax/src/htmlseg.rs` and every verdict in \
         this file is void"
    );
}

/// **The positive half of the structural question.** `htmlseg` DID have a
/// fragment balancer — `pub(crate) fn balance_fragment` — so the ti `95f55b`
/// and ti `c1f9a8` claims are comparable old-vs-new, and the tests below are
/// entitled to compare them directly rather than settle for pinning.
///
/// The assertion is the balancer's defining act on the simplest possible input:
/// an element opened and never closed earns an appended end tag. Establishing
/// this is what licenses `CONFIRMED` (rather than `NOT_COMPARABLE`) for every
/// balancer claim in this file.
#[test]
fn the_pre_extraction_engine_did_have_a_fragment_balancer() {
    assert_eq!(
        htmlseg_old::balance_fragment("<div><span>x"),
        "<div><span>x</span></div>",
        "the reference's own `balance_fragment` must be callable and must append \
         closers; if it is not, the balancer claims in this file are \
         NOT_COMPARABLE rather than CONFIRMED and the verdicts must change"
    );
}

/// **The negative half of the structural question, and this file's most
/// load-bearing test.** `strip_reserved_sync_attrs` did not exist in
/// `htmlseg` — so ti `490d97` wave 1's strip half and ti `415cdb` are not
/// divergences *from `htmlseg`* at all. DCR-0032's phrase is that the strip
/// "landed ahead of its callers".
///
/// Absence is proven the same way anything else here is: by execution. The old
/// module offered exactly five entry points a caller could reach —
/// `extract`, `splice` (identity and translating), `tag_inventory`, and the
/// crate-visible `balance_fragment` — and this feeds a block carrying TWO
/// reserved attributes through all of them. Every one hands the attributes
/// back. There is no channel through which `htmlseg` removed a reserved name,
/// so there is nothing for the strip to have diverged from.
#[test]
fn the_pre_extraction_engine_had_no_reserved_attribute_strip() {
    let block = "<div data-sync-id=\"x\" data-order=\"1\">y</div>";

    // (1) extract — reads text only, so the attributes cannot even be reported.
    let seg = htmlseg_old::extract(block).expect("old extract runs");
    assert_eq!(seg.texts, vec!["y".to_string()]);

    // (2) splice, identity: byte-verbatim, attributes included.
    let identity = htmlseg_old::splice(block, &["y".to_string()], 6).expect("old splice runs");
    assert_eq!(
        identity, block,
        "an identity splice through the old engine returned the block's own bytes"
    );

    // (3) splice, translating: only the text node moves.
    let translated = htmlseg_old::splice(block, &["z".to_string()], 6).expect("old splice runs");
    assert_eq!(
        translated,
        "<div data-sync-id=\"x\" data-order=\"1\">z</div>"
    );

    // (4) tag_inventory — names only, and unchanged by the attributes.
    assert_eq!(
        htmlseg_old::tag_inventory(block),
        vec!["div".to_string(), "/div".to_string()]
    );

    // (5) balance_fragment — the one crate-visible function that rewrites bytes.
    assert_eq!(
        htmlseg_old::balance_fragment(block),
        block,
        "the old balancer passed a balanced fragment through untouched, reserved \
         attributes and all"
    );

    // Every old channel that emits bytes still carries both names…
    for (channel, out) in [
        ("splice/identity", identity.as_str()),
        ("splice/translated", translated.as_str()),
        ("balance_fragment", block),
    ] {
        assert!(
            out.contains("data-sync-id=\"x\"") && out.contains("data-order=\"1\""),
            "{channel} kept the reserved attributes — as it must, since htmlseg \
             had no strip; if this ever fails, htmlseg DID have strip-shaped \
             behaviour and the NOT_COMPARABLE verdicts in this file are wrong"
        );
    }

    // …and the current crate removes both. That contrast is the partition: the
    // strip is new capability, not a changed reading of old capability.
    assert_eq!(
        new::strip_reserved_sync_attrs(block),
        "<div>y</div>",
        "transync-html's strip removes the whole reserved namespace; htmlseg had \
         no function that did this and no channel through which it happened"
    );
}

// ---------------------------------------------------------------------------
// Section 1 — ti `95f55b`: a closer the region would swallow is WITHHELD, and
// the idempotence that bought.
// ---------------------------------------------------------------------------

/// **Claim (ti `95f55b`, contracts.md §4a):** "a closer that would land inside
/// an unterminated trailing comment, CDATA section, bogus comment, raw-text run
/// or tag is **not** appended … because a swallowed closer buys nothing and
/// feeds unbounded re-balance growth: 15,726 violations across 200,000 fuzz
/// iterations before the fix."
///
/// One input per region kind, each the smallest that exhibits the claim: an
/// open `<div>` (so a closer is genuinely owed) plus the shortest byte sequence
/// that opens the region and runs to EOF.
///
/// The assertion is not "the outputs differ". For each kind it is: the closer
/// the OLD balancer appended is invisible to a browser-faithful reading of its
/// own output, and the closer the NEW balancer produces is visible. That is
/// "a buried closer closes nothing", executed.
#[test]
fn a_closer_the_trailing_region_would_swallow_is_no_longer_appended() {
    // (input, the element whose closer is owed, which region kind it opens)
    let cases = [
        ("<div>x<!--", "div", "unterminated comment"),
        (
            "<svg><![CDATA[x",
            "svg",
            "unterminated CDATA section in foreign content",
        ),
        ("<div>x<!foo", "div", "bogus comment (`<!` + non-`--`)"),
        ("<div>x<?pi", "div", "bogus comment (`<?`)"),
        (
            "<div>x</",
            "div",
            "bogus comment (`</` before a non-letter)",
        ),
        ("<div>x<span", "div", "tag cut off before its `>`"),
        (
            "<div>x<div class=\"y",
            "div",
            "tag cut off inside a quoted value",
        ),
    ];

    for (html, owed, kind) in cases {
        let (old, new_) = balanced(html);

        assert_ne!(
            old, new_,
            "{kind}: the record claims this input's balanced output changed \
             (old={old:?} new={new_:?})"
        );
        assert!(
            old.ends_with(&format!("</{owed}>")),
            "{kind}: the OLD balancer must be shown appending the closer at all, \
             or there is nothing to have withheld: {old:?}"
        );
        assert!(
            !closer_is_visible(&old, owed),
            "{kind}: the closer the old balancer appended is BURIED — a \
             browser-faithful scan of {old:?} sees no `</{owed}>`, so it closed \
             nothing. If this assertion fails, the old output was fine and \
             ti 95f55b's premise is wrong"
        );
        assert!(
            closer_is_visible(&new_, owed),
            "{kind}: transync-html's balanced output must carry a closer a \
             browser can see: {new_:?}"
        );
    }
}

/// **Claim (ti `95f55b`):** "each re-balance added another" — the fix bought
/// IDEMPOTENCE, so re-balancing an already-balanced fragment must not keep
/// growing it.
///
/// The growth half is executed on the old engine and the fixed-point half on
/// the new one, over the same inputs, so the test discriminates rather than
/// merely asserting the good half.
///
/// **Scope, measured rather than assumed.** The unbounded growth is observable
/// for the kinds the old *scanner* also modelled — an unterminated comment, an
/// unterminated CDATA section in foreign content, and a cut-off tag. It is NOT
/// observable for the bogus-comment kinds or for `<plaintext>`, because the old
/// scanner did not recognize those regions at all: it read their bytes as plain
/// text, so its own re-scan saw the appended `</div>` as a real end tag and
/// stopped. Those kinds bury the closer *in a browser* without growing the
/// fragment — which is exactly why ti `c1f9a8` had to be a second ticket about
/// the §4a break rather than a footnote to this one (see section 2).
#[test]
fn rebalancing_a_balanced_fragment_no_longer_grows_it() {
    // (input, does the OLD balancer grow on re-balance?)
    let cases = [
        ("<div>x<!--", true),
        ("<svg><![CDATA[x", true),
        ("<div>x<span", true),
        ("<div>x<div class=\"y", true),
        ("<div>x<!foo", false),
        ("<div>x<?pi", false),
        ("<div>x</", false),
        ("<div>x<plaintext>y", false),
    ];

    for (html, old_grows) in cases {
        let (old, new_) = balanced(html);
        let old_again = htmlseg_old::balance_fragment(&old);
        let new_again = new::balance_fragment(&new_);

        assert_eq!(
            new_again, new_,
            "transync-html's balancer must be a FIXED POINT on its own output \
             for {html:?}; it is not, so a pane re-balanced twice would keep \
             accumulating closers: {new_:?} -> {new_again:?}"
        );

        if old_grows {
            assert!(
                old_again.len() > old.len(),
                "the old balancer must be shown GROWING for {html:?}, or ti \
                 95f55b's 'each re-balance added another' has no witness here: \
                 {old:?} -> {old_again:?}"
            );
        } else {
            assert_eq!(
                old_again, old,
                "the old balancer is a fixed point for {html:?} — its scanner \
                 never recognized this region, so its own re-scan read the \
                 appended closer as markup. Recorded so the growth claim's \
                 scope is measured, not assumed: {old:?} -> {old_again:?}"
            );
        }
    }
}

/// **Claim (ti `95f55b`, contracts.md §4a) — the "raw-text run" item.** The
/// five-kind list names a "raw-text run" alongside the comment, CDATA,
/// bogus-comment and tag kinds.
///
/// Executed, that item does not hold: for a fragment ending inside a raw-text
/// or RCDATA run, the appended closer is the very thing that ENDS the run, so
/// it is markup, it is kept, and old and new agree byte-for-byte on all seven
/// raw-text names the current crate recognizes. There is no observable
/// withholding for this kind, and the current `SkipKind` enum has no `RawText`
/// variant to carry one — consistent with DCR-0047, which names **four** region
/// kinds and omits raw text.
///
/// This test therefore records a REFUTATION of one item in a five-item list,
/// not of the claim as a whole: the other four are confirmed above.
#[test]
fn a_raw_text_run_never_actually_withholds_a_closer() {
    // Every raw-text/RCDATA name the current crate treats as such, plus the
    // nesting that would be needed to bury the appended closer if it could be.
    let cases = [
        "<div>x<script>y",
        "<div>x<style>y",
        "<div>x<textarea>y",
        "<div>x<title>y",
        "<div>x<xmp>y",
        "<div>x<iframe>y",
        "<div>x<noembed>y",
        "<div>x<noframes>y",
        "<div>x<textarea>",
        "<textarea><div>x",
        "<div>x<textarea>y<!--",
        "<div>x<textarea>y<span",
    ];

    for html in cases {
        let (old, new_) = balanced(html);
        assert_eq!(
            old, new_,
            "no raw-text case produces a withheld closer: old and new agree on \
             {html:?}. If this ever differs, the 'raw-text run' item in ti \
             95f55b's five-kind list has finally acquired a witness and this \
             test's REFUTED verdict must be revisited"
        );
        assert!(
            closer_is_visible(&new_, "div") || !html.starts_with("<div"),
            "the appended raw-text closer ends the run, so the `</div>` behind \
             it is real markup: {new_:?}"
        );
    }

    // The one raw-text-adjacent input where the two DO differ is not a raw-text
    // region at all: `</textarea ` exits the run and then runs out of bytes, so
    // the region is `SkipKind::UnterminatedTag` — the kind already confirmed
    // above. Pinned here so the refutation above is not mistaken for a gap.
    let (old, new_) = balanced("<div>x<textarea>y</textarea ");
    assert_eq!(
        old, "<div>x<textarea>y</textarea </textarea></div>",
        "old buried its appended `</textarea>` inside the unterminated END tag"
    );
    assert_eq!(
        new_, "<div>x<textarea>y</textarea></div>",
        "new deletes the abandoned end tag and appends closers a browser sees"
    );
}

// ---------------------------------------------------------------------------
// Section 2 — ti `c1f9a8` / DCR-0047: the fragment is REPAIRED, and that is
// what closes the `contracts.md` §4a direct-child break.
// ---------------------------------------------------------------------------

/// **Claim (ti `c1f9a8` / DCR-0047):** the region is "**terminated** where a
/// browser terminates it … or **deleted** where a browser abandons it", with
/// `TagToken::Skip` carrying kind and terminated-ness "so the balancer never
/// classifies bytes itself".
///
/// Pinned as exact output bytes, because the *choice* between terminate and
/// delete is the claim — "they differ from old" would not distinguish a
/// terminated comment from a deleted one. The four rows are DCR-0047's own
/// two-bless movement table, plus the `<plaintext>` row DCR-0050 added.
#[test]
fn an_unterminated_trailing_region_is_terminated_or_deleted_as_a_browser_resolves_it() {
    // (input, repaired output, and which of the two acts applied)
    let cases = [
        ("<div>x<!--", "<div>x<!----></div>", "terminated with `-->`"),
        (
            "<div>x<?pi",
            "<div>x<?pi></div>",
            "bogus comment terminated with `>`",
        ),
        (
            "<svg><text><![CDATA[y",
            "<svg><text><![CDATA[y]]></text></svg>",
            "CDATA section terminated with `]]>`",
        ),
        (
            "<div>x<p",
            "<div>x</div>",
            "cut-off tag DELETED (a browser abandons it)",
        ),
        (
            "<div>x<plaintext>y",
            "<div>x</div>",
            "PLAINTEXT region DELETED — HTML's one exitless state has no terminator",
        ),
    ];

    for (html, want, act) in cases {
        assert_eq!(
            new::balance_fragment(html),
            want,
            "{act}: the repair's exact bytes are the claim. A different answer \
             here means the balancer chose the other act, or invented a third"
        );
        assert_ne!(
            htmlseg_old::balance_fragment(html),
            want,
            "{act}: the old balancer must NOT already produce this, or there \
             is no divergence to confirm"
        );
    }
}

/// **Claim (ti `c1f9a8` / DCR-0047, contracts.md §4a):** the repair "closes a
/// `contracts.md` §4a direct-child break", because "three of those four kinds
/// end at the **first `>`** — and in a mounted pane the next `>` is the sync
/// wrapper's own `</div>`. So the wrapper closed inside the region, the wrapper
/// stayed open, and every following block nested inside this block's wrapper."
///
/// This is the claim that costs something, so it is executed on the actual pane
/// shape rather than on a bare fragment: one wrapper around the balanced block,
/// then the next block's anchor as a sibling. §4a requires that anchor to be a
/// DIRECT CHILD — depth 0 in this two-block pane.
///
/// Both shapes of the break are asserted and distinguished. A comment or a
/// `<plaintext>` region swallows everything after it, so the next anchor does
/// not exist in the DOM at all (`None`). The three `>`-terminated kinds let it
/// exist but at depth 1, inside the previous block's wrapper.
#[test]
fn the_next_blocks_anchor_stops_nesting_inside_the_previous_wrapper() {
    // (block, expected old anchor depth — None means swallowed outright)
    let cases = [
        (
            "<div>x<!--",
            None,
            "unterminated comment swallows the rest of the pane",
        ),
        (
            "<div>x<plaintext>y",
            None,
            "PLAINTEXT swallows the rest of the pane (HTML never leaves the state)",
        ),
        (
            "<div>x<!foo",
            Some(1),
            "bogus comment ends at the wrapper's own `>`",
        ),
        (
            "<div>x</",
            Some(1),
            "`</` bogus comment ends at the wrapper's own `>`",
        ),
        (
            "<div>x<span",
            Some(1),
            "cut-off tag ends at the wrapper's own `>`",
        ),
    ];

    for (block, old_depth, why) in cases {
        let (old, new_) = balanced(block);
        let old_pane = pane(&old);
        let new_pane = pane(&new_);

        assert_eq!(
            anchor_depth(&old_pane, "b2"),
            old_depth,
            "{why}: the OLD balancer's pane must exhibit the §4a break for \
             {block:?} — pane={old_pane:?}. Without this the break has no \
             witness and the claim is unproven, not confirmed"
        );
        assert_eq!(
            anchor_depth(&new_pane, "b2"),
            Some(0),
            "{why}: after the repair the next block's anchor must be a DIRECT \
             CHILD of the pane root, which is what contracts.md §4a requires. \
             A depth above 0 means the wrapper is still open and every later \
             anchor is nested; `None` means it was swallowed entirely. \
             pane={new_pane:?}"
        );
        // And the block's own wrapper must actually close, not merely coexist.
        let own = new::element_extents(&new_pane)
            .into_iter()
            .find(|e| new_pane[e.open.0..e.open.1].contains("data-sync-id=\"b1\""))
            .expect("the block's own wrapper exists");
        assert!(
            own.close.is_some() && own.content_end < new_pane.find("b2").unwrap(),
            "{why}: the block's wrapper must close BEFORE the next anchor \
             begins; extent={own:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Section 3 — ti `490d97` wave 1, the strip half. NOT_COMPARABLE (see
// `the_pre_extraction_engine_had_no_reserved_attribute_strip`); the current
// behaviour the record describes is pinned regardless.
// ---------------------------------------------------------------------------

/// **Claim (ti `490d97` wave 1, contracts.md §4a):** the strip "coalesces
/// adjacent cuts and replaces the run with one space wherever deleting it would
/// weld bytes together. Before that rule, `<div data-sync-id="a b="c">x`
/// stripped to `<divc">x` and the inventory moved from `["div"]` to
/// `["divc"]`."
///
/// The record's example is the smallest input that exhibits the weld, and it
/// exhibits it for a reason worth stating: the value `"a b="` swallows the
/// space, so the reserved attribute's run ends on a NAME byte (`c`) rather than
/// on whitespace or `>`.
///
/// The assertion carries both halves the record states. The seam rule's output
/// is pinned byte-exact, and the counterfactual — the record's own pre-rule
/// string — is fed to the CURRENT scanner to show the tag inventory really does
/// move off `["div"]`, which is what would have broken validation layer 3.
#[test]
fn the_strip_seam_rule_keeps_the_element_name_off_the_inventory() {
    let src = "<div data-sync-id=\"a b=\"c\">x";
    let got = new::strip_reserved_sync_attrs(src);

    assert_eq!(
        got, "<div c\">x",
        "the coalesced run must be replaced by ONE U+0020, not deleted: \
         deleting welds `c` onto `div`"
    );
    assert_eq!(
        new::tag_inventory(&got),
        vec!["div".to_string()],
        "the strip must not move the tag inventory validation layer 3 compares"
    );

    // The record's own pre-rule output, read by the current scanner. The record
    // says `["divc"]`; under today's tag-name boundary (ti `e20490`, which made
    // a bare quote a NAME byte) the same bytes read `["divc\""]`. Either way the
    // substance holds: the element is no longer a `div`.
    let pre_rule = "<divc\">x";
    let welded = new::tag_inventory(pre_rule);
    assert_ne!(
        welded,
        vec!["div".to_string()],
        "the pre-rule output must be shown moving the inventory, or the seam \
         rule is protecting nothing"
    );
    assert_eq!(
        welded,
        vec!["divc\"".to_string()],
        "the welded element's name — the record writes `divc`, from the \
         pre-`e20490` name boundary; today's browser-faithful boundary reads \
         `divc\"`. If this changes, the counterfactual is still wrong, just \
         differently wrong"
    );
}

/// **Claim (ti `490d97` wave 1):** "adjacent cuts are coalesced and the run is
/// replaced by one U+0020 instead of removed" — one space for the whole run,
/// and only where its absence would change the parse.
///
/// Four rows, one per branch of the seam rule, so a regression in any single
/// branch fails a named case rather than a lump. The `/`-before-`>` row is the
/// one whose consequence is structural: welding `/` onto `>` SETS a
/// self-closing flag the source never carried.
#[test]
fn adjacent_cuts_coalesce_into_exactly_one_space() {
    let cases = [
        (
            "<div data-sync-id=\"x\">y",
            "<div>y",
            "the well-formed case still strips byte-exact — no space added",
        ),
        (
            "<div data-sync-id=\"a\" data-order=\"b\" x=\"1\">y",
            "<div x=\"1\">y",
            "two adjacent cuts, next byte is whitespace: no space added",
        ),
        (
            "<div/data-sync-id=\"a\" data-order=\"b\">x",
            "<div/ >x",
            "prev is `/` and next is `>`: ONE space, or the tag becomes \
             self-closing. Coalescing is what makes this one space rather than \
             a plain delete of the second cut (which would give `<div/>`)",
        ),
        (
            "<div DATA-SYNC-ID=X>y",
            "<div>y",
            "matching is case-insensitive on the NAME",
        ),
    ];

    for (src, want, why) in cases {
        assert_eq!(new::strip_reserved_sync_attrs(src), want, "{why}");
    }

    // The strip must also leave alone what only LOOKS reserved: a reserved name
    // appearing as a VALUE is content, and borrowing (not copying) is the
    // observable proof that no cut was collected at all.
    let inert = "<div class=\"data-sync-id\">y";
    let got = new::strip_reserved_sync_attrs(inert);
    assert!(
        matches!(got, Cow::Borrowed(_)) && got == inert,
        "a reserved name in a value is content: the strip must borrow, which \
         proves it collected no cut: {got:?}"
    );
}

// ---------------------------------------------------------------------------
// Section 4 — ti `415cdb`: the one measured over-deletion exception.
// NOT_COMPARABLE for the same reason section 3 is.
// ---------------------------------------------------------------------------

/// **Claim (ti `415cdb`, contracts.md §4a):** "across a stray `=` the strip
/// over-deleted, so `strip_reserved_sync_attrs("<div =data-sync-id=\"x\">y")`
/// returned `<div =>y` where a browser folds those bytes into **one junk
/// attribute named `=data-sync-id`** — nothing reserved present, yet the cut
/// ran and a non-reserved attribute moved."
///
/// So the fixed behaviour is that the input comes back UNCHANGED, and
/// "unchanged" is asserted in the strongest available form: `Cow::Borrowed`,
/// which can only happen when the strip collected no cut at all. Pinning the
/// output string alone would not distinguish "found nothing to cut" from "cut
/// and put the bytes back".
///
/// The test also has to discriminate: a strip that simply refused to touch
/// anything would pass a bare "unchanged" assertion. So the same tag with a
/// genuinely reserved attribute in the same position is stripped, proving the
/// refusal is name-based.
#[test]
fn a_stray_equals_begins_an_attribute_name_so_nothing_is_cut() {
    let src = "<div =data-sync-id=\"x\">y";
    let got = new::strip_reserved_sync_attrs(src);

    assert!(
        matches!(got, Cow::Borrowed(_)),
        "`=data-sync-id` is ONE attribute name, and it is not in the reserved \
         set, so the strip must collect no cut and borrow: {got:?}"
    );
    assert_eq!(got, src, "and therefore return the source bytes untouched");

    // The recorded over-deletion, for contrast: it removed 15 bytes of the
    // author's own (non-reserved) attribute.
    let over_deleted = "<div =>y";
    assert_ne!(
        got, over_deleted,
        "the pre-fix output must differ from today's, or ti 415cdb fixed nothing"
    );
    assert!(
        over_deleted.len() < src.len(),
        "the direction of the recorded defect was OVER-deletion: the pre-fix \
         output is shorter than its input"
    );

    // Discrimination: the same position, a genuinely reserved name, IS cut.
    assert_eq!(
        new::strip_reserved_sync_attrs("<div data-sync-id=\"x\">y"),
        "<div>y",
        "the refusal above must be about the NAME, not about the strip having \
         gone inert"
    );
}

/// **Claim (ti `415cdb`, contracts.md §4a):** "The direction was always safe
/// (over-deletion, never under-deletion: a 51-case browser equivalence probe
/// found 50/51 equal, that one over-deletion, and **zero** under-deletions), so
/// the impostor-anchor property was never at risk."
///
/// The 51-case figure itself is not reproducible here — it required a browser as
/// the oracle, and these tests must not need one — so the record's measurement
/// is reported UNFALSIFIABLE. What IS executable is the property the figure was
/// evidence *for*, on today's code: **no under-deletion**. If the strip ever
/// leaves a reserved name inside an element open tag, an author's attribute
/// reaches the mounted DOM as an anchor the engine cannot tell from the
/// renderer's own — the impostor-anchor break OI-0035 route (c) exists to
/// prevent.
///
/// The detector (`live_reserved_names`) is deliberately independent of the
/// strip's own attribute walk, which is where the under-deletion risk lives
/// (ti `e20490` was exactly a name-boundary bug). It reads only `scan_tags`'
/// `Open` spans and applies one rule: an attribute NAME starts after whitespace
/// or `/` and ends at `=`, whitespace, `/` or `>`. Nothing about quoting,
/// duplicate names or attribute order.
///
/// The test is guarded against being vacuous in both directions: the same
/// detector is run on the UNSTRIPPED sources and must find the plants there.
/// That guard earned its place — a first, cruder detector (plain substring over
/// the open tag) reported `<div =data-sync-id="v">` as an under-deletion, which
/// it is not: a browser folds those bytes into one junk attribute NAMED
/// `=data-sync-id`, so `[data-sync-id]` does not select it. That is ti
/// `415cdb`'s own point, and a detector that cannot tell the two apart cannot
/// test it.
#[test]
fn the_strip_never_leaves_a_reserved_name_in_an_open_tag() {
    // Malformed shapes crossed with each reserved name: quoted, unquoted and
    // valueless spellings, a stray `=` and a phantom quoted value in the tag
    // name (ti `e20490`'s case), a `/` where the separator would be, inside
    // foreign content and inside an SVG `<title>` (ti `2e2453`'s case), a tag
    // left unterminated at EOF (ti `549b20`'s case), and duplicates so a
    // coalesced run is exercised too.
    let shapes: &[&str] = &[
        "<div {N}=\"v\">y",
        "<div {N}=v>y",
        "<div {N}>y",
        "<div {N} >y",
        "<div/{N}=\"v\">y",
        "<div a=\"1\" {N}=\"v\" b=\"2\">y",
        "<div {N}=\"a b=\"c\">y",
        "<divq\"x=\" {N}=\"v\">y",
        "<div ={N}=\"v\" {N}=\"w\">y",
        "<div {N}=\"v\" {N}=\"w\">y",
        "<DIV {N}=\"v\">y",
        "<svg><rect {N}=\"v\"/></svg>",
        "<svg><title><b {N}=\"v\">y</b></title></svg>",
        "<div {N}=\"v\"",
        "<div {N}=\"v\"><span {N}=\"w\">y</span></div>",
    ];

    let mut checked = 0usize;
    let mut sources_with_a_live_plant = 0usize;
    for name in RESERVED {
        for shape in shapes {
            let src = shape.replace("{N}", name);
            let stripped = new::strip_reserved_sync_attrs(&src);
            checked += 1;

            // Positive control: the plant must be LIVE before the strip runs,
            // or this row proves nothing about the strip.
            if !live_reserved_names(&src).is_empty() {
                sources_with_a_live_plant += 1;
            }

            let survivors = live_reserved_names(&stripped);
            assert!(
                survivors.is_empty(),
                "UNDER-DELETION: {survivors:?} survived as live attribute \
                 name(s).\n\
                 source:   {src:?}\n\
                 stripped: {stripped}\n\
                 This is the impostor-anchor break: an author's reserved \
                 attribute reaching the mounted DOM as an anchor the sync \
                 engine cannot distinguish from the renderer's own \
                 (contracts.md §4a, OI-0035 route (c))."
            );

            // The strip must also be a fixed point: a second pass finding more
            // to cut would mean the first pass under-deleted.
            assert_eq!(
                new::strip_reserved_sync_attrs(&stripped),
                stripped,
                "the strip must be idempotent; a second pass that changes bytes \
                 means the first left a reserved attribute behind: {src:?}"
            );
        }
    }

    assert_eq!(
        checked,
        RESERVED.len() * shapes.len(),
        "every reserved name must be crossed with every shape; a shrinking \
         count means the corpus stopped covering the namespace"
    );
    // 14 of the 15 shapes plant a live attribute. The exception is
    // `<div {N}="v"` — a tag cut off at EOF, which a browser ABANDONS, so
    // `scan_tags` reports it as a `Skip` region and it mints no attribute at
    // all. That row still belongs in the corpus: it is the case where the
    // strip must NOT act, and the detector agreeing is the point.
    assert_eq!(
        sources_with_a_live_plant,
        14 * RESERVED.len(),
        "the detector must find the plants in the UNSTRIPPED sources, or the \
         assertion above is vacuous and would pass on a strip that did nothing"
    );
}

/// Reserved attribute names that are LIVE in `html` — i.e. would be selectable
/// as `[data-sync-id]` and friends in a mounted DOM.
///
/// One rule, applied to `scan_tags`' `Open` spans only: a name starts after
/// whitespace or `/` and ends at `=`, whitespace, `/` or `>`. That is enough to
/// separate the three things a naive substring search conflates — a real
/// attribute, a longer junk name that merely contains a reserved one
/// (`=data-sync-id`), and a reserved name sitting inside a quoted VALUE, which
/// is content.
fn live_reserved_names(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    for token in new::scan_tags(html) {
        let new::TagToken::Open { span, .. } = token else {
            continue;
        };
        let tag = html[span.0..span.1].to_ascii_lowercase();
        let bytes = tag.as_bytes();
        for name in RESERVED {
            let mut from = 0usize;
            while let Some(rel) = tag[from..].find(name) {
                let at = from + rel;
                let end = at + name.len();
                let starts_a_name = at
                    .checked_sub(1)
                    .is_some_and(|i| bytes[i].is_ascii_whitespace() || bytes[i] == b'/');
                let ends_the_name = bytes
                    .get(end)
                    .is_some_and(|c| c.is_ascii_whitespace() || matches!(c, b'=' | b'/' | b'>'));
                if starts_a_name && ends_the_name {
                    found.push((*name).to_string());
                }
                from = end;
            }
        }
    }
    found
}
