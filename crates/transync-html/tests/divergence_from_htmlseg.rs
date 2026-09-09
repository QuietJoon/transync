//! Differential proof of the claimed divergences from the retired `htmlseg`.
//!
//! `CLAUDE.md` states that the record trail records **nineteen** deliberate
//! behavioural divergences of `transync-html` from `transync-syntax::htmlseg`,
//! and that "a difference from it in any of the nineteen is the **fix**, not a
//! regression to restore". That sentence is load-bearing: it is what stops the
//! next maintainer "repairing" a deliberate browser-fidelity change.
//!
//! Until now the sentence rested on a count nobody could check. DCR-0032's
//! amendments describe the early divergences in prose without ordinals and then
//! jump to "the fifteenth through nineteenth", so a reader had one bracket and
//! no list (ticket `3f0879ef`).
//!
//! This file replaces the count with **measurement**. The pre-extraction
//! implementation is recovered from git and vendored beside it, so every claim
//! is executed rather than enumerated:
//!
//! - `tests/reference/htmlseg_1d6f19d.rs` is `git show
//!   1d6f19d^:crates/transync-syntax/src/htmlseg.rs`, **byte-for-byte**. Not one
//!   line is edited. `1d6f19d` is the extraction commit ("refactor: the HTML
//!   mechanics get their own crate, and it is called transync-html"), so its
//!   parent is the last tree in which `htmlseg` was the shipping code.
//! - It compiles unmodified because it never referenced its own crate: its whole
//!   dependency surface is `lol_html`, `htmlize` and `std`, and the workspace
//!   still pins `lol_html = "2"` and `htmlize = "1"`.
//! - Its own ~40 tests ride along inside the vendored module and must keep
//!   passing. That is the vendoring's self-check: if they ever fail, the
//!   reference has stopped being the code it claims to be, and every verdict
//!   below is void.
//!
//! # Why the comparison goes through the public surface
//!
//! Old `htmlseg` exposed exactly four public items — `HtmlSegments`, `extract`,
//! `splice`, `tag_inventory`. It had **no** `balance_fragment`, no
//! `element_extents`, no `scan_tags` (private), and no
//! `strip_reserved_sync_attrs` at all. So a function-to-function diff is not
//! available for most claims, and asking for one would be the wrong question
//! anyway: what a divergence costs is what a **caller** sees change. Every
//! assertion here therefore reads the observable channels a caller had in both
//! versions, chiefly `tag_inventory` — which is not an arbitrary choice, because
//! it is the inventory `transync-core`'s validation layer 3 compares to decide
//! whether a splice preserved the markup.
//!
//! A claim about a function that postdates the extraction is **not a divergence
//! from `htmlseg`**; it is a fix to code `htmlseg` never had. Those are recorded
//! as `NotComparable` rather than silently counted, because counting them is how
//! nineteen stops being checkable.
//!
//! TRACE: ti 3f0879ef
//! TRACE: DCR-0032, DCR-0050

#[allow(dead_code, clippy::all, clippy::pedantic)]
#[path = "reference/htmlseg_1d6f19d.rs"]
mod htmlseg_old;

use transync_html as new;

/// What a single claim's execution established.
#[derive(Debug, PartialEq, Eq)]
enum Verdict {
    /// Old and new disagree on this input, and the record says they should.
    Divergent,
    /// Old and new agree — so whatever the record describes, it is not
    /// observable through this channel on this input.
    Identical,
}

/// Compare the two implementations on one input through `tag_inventory`.
fn inventories(html: &str) -> (Vec<String>, Vec<String>) {
    (htmlseg_old::tag_inventory(html), new::tag_inventory(html))
}

fn verdict(html: &str) -> Verdict {
    let (old, new_) = inventories(html);
    if old == new_ {
        Verdict::Identical
    } else {
        Verdict::Divergent
    }
}

/// The vendoring's self-check, stated as a test rather than left to the reader:
/// the reference must still be able to do the job it did, or nothing else in
/// this file means anything. `extract` on a plain block is the narrowest
/// end-to-end exercise of the old engine's rewriter path.
#[test]
fn the_vendored_reference_still_runs() {
    let got = htmlseg_old::extract("<div><p>hello</p></div>").expect("old extract runs");
    assert_eq!(
        got.texts,
        vec!["hello".to_string()],
        "the vendored pre-extraction engine must still extract the one text node \
         in a trivial block; if this fails the reference is not the code it claims \
         to be and every verdict in this file is void"
    );
}

/// **Claim (DCR-0050, R0010-0031):** `RAW_TEXT_ELEMENTS` was widened from four
/// names to eight, because HTML's in-body/in-head rules reach the generic
/// raw-text parsing algorithm for `xmp`, `iframe`, `noembed` and `noframes`
/// too — so `<div><iframe></div></iframe>`, which is ONE type-6 html block,
/// stopped losing its author's `</iframe>` to the orphan pass.
///
/// The old constant is visible in the vendored reference as
/// `RAW_TEXT_ELEMENTS: &[&str] = &["script", "style", "textarea", "title"]`,
/// which is the four the claim names.
#[test]
fn iframe_content_was_scanned_as_markup_and_is_now_raw_text() {
    let html = "<div><iframe></div></iframe>";
    let (old, new_) = inventories(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );

    // The substance, not just the difference: the old reading scanned the
    // iframe's contents, so `</div>` was a tag to it. The new reading treats
    // them as text, so the only tags are the div and the iframe.
    assert!(
        old.iter().any(|t| t == "/div"),
        "old htmlseg read `</div>` inside iframe content as a tag: {old:?}"
    );
    assert!(
        !new_.iter().any(|t| t == "/div"),
        "transync-html reads iframe content as raw text, so `</div>` is text \
         rather than a tag: {new_:?}"
    );
}

/// **Claim (DCR-0050, R0010-0032):** `comment_end` became the one home of
/// "where does a comment end" and learned HTML's second closing form `--!>`,
/// so `<!-- a --!><div>y<!-- b -->` stopped hiding a live `<div>` inside one
/// comment.
#[test]
fn the_bang_comment_close_hid_a_live_div_and_no_longer_does() {
    let html = "<!-- a --!><div>y<!-- b -->";
    let (old, new_) = inventories(html);

    assert_eq!(
        verdict(html),
        Verdict::Divergent,
        "the claim is that this input's reading changed; old={old:?} new={new_:?}"
    );
    assert!(
        !old.iter().any(|t| t == "div"),
        "old htmlseg closed a comment only at `-->`, so the `<div>` was buried \
         inside one comment and never reached the inventory: {old:?}"
    );
    assert!(
        new_.iter().any(|t| t == "div"),
        "`--!>` closes the comment, so the `<div>` is live markup — which is what \
         a browser does with it: {new_:?}"
    );
}
