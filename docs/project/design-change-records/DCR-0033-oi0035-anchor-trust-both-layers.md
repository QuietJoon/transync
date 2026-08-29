---
type: DCR
title: An anchor is trusted because the alignment map claims it, and because the renderer put it there
description: OI-0035 closed at both layers (route (c)). The Markdown pane's html-block arm strips the reserved sync-attribute namespace out of the block's own bytes before writing ours, and mountSync takes its anchor set from the validated alignment rows so an unlisted data-sync-id is inert forever, including one inserted after mount. The residual neither layer closes alone is recorded rather than glossed.
tags: [change, project-control, DCR-0033]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-23T00:00:00Z
status: stable
---

# DCR-0033: An anchor is trusted because the map claims it, and because we put it there

- **Date:** 2026-08-23 — this record. The two code commits landed **2026-08-22**:
  `eedc9e3` (the render half) and `0972fa2` (the engine half, both `sync.js`
  copies in one commit).
- **Source:** ticket `490d97`, **wave 1** of the eight in
  `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12.
  Wave 1 is parallel to waves 2–5 and independent of every wave after it: it
  landed **before** the HTML pane producer that would have widened the exposure
  exists.
- **Post-implementation**: the record follows the code it describes and lands as
  the wave's closing commit. Six commits carry the wave — `1299280` (the plan's
  own baseline correction), `eedc9e3` (render half), `0972fa2` (engine half),
  then the wave-0 defect this wave's first call site surfaced, `644e4f2` (the
  corpus learns it) / `d146a53` (the fix) / `a7af73f` (DCR-0032's amendment),
  and `c5ff3df` (the two fix reviews' document findings, plus the corpus gate's
  pathspec).
- **The plan said 2026-08-20 throughout and the tree says otherwise.** That was
  the date the plan was written, not the date anything landed;
  `git log --date=short` is the check. Every date in this record and in the five
  documents it moves is the measured one.
- **No ADR in this wave.** ADR-0025 records the twelve HTML→HTML decisions as
  one architectural commitment and lands in wave 2 (spec §12).
- **Affected contracts:** `docs/architecture/contracts.md` §4 (the
  reserved-namespace rule, plus the corrected "reads `[data-sync-id]` only"
  sentence) and §4a (the anchor-set paragraph and its residual). Both were
  written by the code commits above; this record does not restate them.

## Not breaking, and not schema-visible

No §0 surface item moves. The alignment wire is unchanged and
`ALIGNMENT_SCHEMA_VERSION` stays `1.2.0`. What changes is the bytes a pane
carries and which of them the engine will act on — neither is a published
contract of the `transync` facade, and both are named in `contracts.md`.

## The mechanism, now measured rather than inferred

Spec §15 item 3 flagged DOMPurify's `ALLOW_DATA_ATTR` default as *inferred*. It
was measured in this wave against the vendored build the bundle actually ships
(`web/tests/scn13.spec.js`, test `m`):
`DOMPurify.sanitize('<div data-sync-id="p-0003">x</div>')` returns the attribute
intact. The sanitizer is not, and was never, the layer that closed this — which
matters, because the render half's *necessity* rested on that inference for as
long as it stayed one.

## Route (c), and why not (a) or (b) alone

(a) sanitize-time stripping of `data-*` would take the renderer's own anchors
with it, or need a sanitizer configuration transync does not own in a
third-party mount. (b) the engine gate alone leaves the
listed-id-ahead-of-the-genuine-anchor case open. Two layers, each covering the
other's blind spot.

## The render half

The Markdown path's `BlockKind::Html` **success** arm — the only live route by
which source-controlled markup reaches a Markdown pane, since panes render with
`unsafe_ = false` — now emits
`balance_fragment(strip_reserved_sync_attrs(md))`. Six names
(`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`,
`data-parent-id`, `data-skipped`), case-insensitive, element open tags only.

The **failure** arm is deliberately untouched: its payload is HTML-escaped, so
an impostor attribute there is text, not an anchor.

The strip is **pane-only**. `out.md` keeps the author's bytes, because their
`data-sync-id` is their content; this namespace is owned only in DOM transync
mounts. And `strip_reserved_sync_attrs` itself shipped in wave 0 (DCR-0032)
with no call site, precisely so this wave would not write it in the old crate
and move it a commit later.

## The call site that surfaced a wave-0 defect, and where the fix is recorded

Wave 1's first and only call site is one composed expression, and **both halves
of it were wrong before this wave ran**. The strip's cut welded bytes together
(`<div/data-sync-id="x">` stripped to `<div/>` — a self-closing flag the author
never wrote), and `balance_fragment`'s walk read that slash the way XML means
it, so the author's own `</div>` matched nothing on the stack, was deleted as an
orphan, and the still-open fragment consumed the sync wrapper's `</div>` —
mounting the next block's anchor inside the html-block wrapper, which
`contracts.md` §4a forbids.

**The fix and its full rationale live in DCR-0032's 2026-08-23 amendment**, and
that is their correct home: these are wave 0's functions, the defect predates
wave 1 entirely (a plain author-written `<div/>` trips it, with no strip and no
HTML translation involved), and repairing only the strip's residue would have
left the authored hole open. This record does not re-litigate it. What belongs
here is the dependency: **route (c)'s render half rests on that fix.** The strip
alone, over the pre-fix walk, would have traded an impostor anchor for a
swallowed one — the same §4a break by a different route. Evidence, one test per
layer: `crates/transync-syntax/src/render.rs`'s
`a_self_closing_html_block_does_not_swallow_the_next_anchor`, and
`web/tests/scn13.spec.js`'s
`n — a self-closing spelling of a non-void tag does not swallow the next anchor`.

One sequencing note while the cross-reference is open: DCR-0032's *Migration /
follow-up* list says "Waves 1–7 are unstarted", which was true when it was
written on 2026-08-21 and stopped being true when this wave landed — wave 1
landed 2026-08-23, and waves 2–7 remain unstarted. That record is closed and
correct as written; this sentence is the amendment it needs, not a new section
in it.

## The engine half

`mountSync` builds the id set from rows whose `sync_role !== "non-sync"`, and
`synchronizableRowCount` is now **derived from that set** — so the predicate
gating the mount and the predicate gating the anchors are one function rather
than two that happen to agree.

`collectAnchors` — the single choke point for both panes at mount and at every
reflow recompute — skips what the set does not contain, warns at mount under the
existing first-five-then-a-tally policy, and stays quiet on reflow. **This
closes the 2026-08-09 correction's residual**: an anchor entering the DOM after
mount used to be folded into the live set by the next quiet recompute with no
audit at any point; it is now inert forever. First-occurrence-wins among listed
ids is unchanged.

So is R0002-0047's refusal, and keeping it took a deliberate decision rather
than no change at all. Whether the panes carry anchors is a question about the
**DOM**, so it is answered by an ungated `querySelectorAll` and asked *before*
the gated collection. Counting collected anchors would have made
`anchorCount > 0 && synchronizableRowCount === 0` unsatisfiable — the maps that
refusal exists to refuse are exactly the maps whose id set is empty — and a
title-only map over anchored panes would have mounted vacuously. **The gate
governs what may drive scroll, not what may be counted.**

## The residual, stated plainly

Neither the gate nor any DOM-visible discriminator can defeat an in-pane
impostor carrying a **listed** id that precedes the genuine anchor in document
order. The render strip is what makes that case unreachable in panes transync
produces; the gate is what makes every **unlisted** id inert in any pane,
whoever produced it. A future producer outside this repository can still hand
the engine a pane with a listed impostor, and the engine will drive from it.
That is the accepted boundary of route (c), also recorded as spec §13 item 8.

## Evidence

- A Rust unit at the render call site,
  `html_block_impostor_sync_attributes_never_reach_the_pane`.
- An end-to-end browser case over a second `--html-out` bundle built from
  `crates/transync/tests/fixtures/oi-0035-impostor-anchor.md`, whose html block
  claims the id of the paragraph that follows it: `web/tests/scn13.spec.js`
  asserts the bundle's `source.html` carries exactly one claimant and the
  mounted pane exactly one element — and guards both against passing vacuously
  on the escaped-placeholder arm.
- Engine-direct browser cases in `web/tests/engine.spec.js`: an unlisted anchor
  cannot drive the follower and is named once per pane at mount; a listed
  duplicate still keeps its first occurrence, and a post-mount arrival stays
  inert through the reflow that used to activate it.
- The R0002-0047 refusal proved intact by the sharpened `engine.spec.js` `b`,
  which now asserts that the refusal message names the anchors the **panes**
  actually carry — the assertion that goes red if the gate is ever wired into
  that count.
- The full workspace suite and the full browser suite green;
  `sync_js_drift.rs` green over two byte-identical copies.

## Welds

`crates/transync-cli/tests/sync_js_drift.rs`, unchanged, is what makes "both
copies moved in one commit" checkable — full byte identity by `std::fs::read`,
not a structural check.

The browser suite's OI-0035 leg lives in `scripts/test-browser.sh` beside the
wasm leg, inside the scratch fixture directory only; the six-file `--html-out`
bundle contract is untouched.

`contracts.md` §4 (the reserved-namespace rule plus the corrected "reads
`[data-sync-id]` only" sentence) and §4a (the anchor-set paragraph with the
residual) carry the rules.

## Handed forward

The second call site — the HTML pane derivation's strip-then-inject — is
**wave 6's**, and spec §8 already fixes its position: strip before injection, so
the same "ours are the only ones" construction holds for HTML panes. Nothing in
this wave forecloses it, and nothing in wave 6 needs to revisit these two.

## Amendment (2026-08-29) — "Six commits carry the wave" is right; the seven-hash enumeration beside it is not

*Appended, not a rewrite. The header bullet above stands exactly as written,
count and hashes both — this says which half of it to believe.*

The **Post-implementation** bullet says "Six commits carry the wave" and then
lists **seven** hashes: `1299280`, `eedc9e3`, `0972fa2`, `644e4f2`, `d146a53`,
`a7af73f`, `c5ff3df`. All seven are real commits and all seven are real work.
The disagreement is only about which wave `1299280` belongs to.

**It is wave 0's, not wave 1's.** DCR-0032's own amendment tabulates wave 0's
commits and lists `1299280` (2026-08-22) as **"the wave's close"**. Excluding
it, this record's list is exactly six — so the count is correct, DCR-0032 agrees
with the count, and the **enumeration is the odd one out**: it swept up the
preceding commit because that commit's baseline correction was the thing wave 1
started from.

The six that carry wave 1 are therefore `eedc9e3` (render half), `0972fa2`
(engine half), `644e4f2` (the corpus learns the wave-0 defect), `d146a53` (the
fix), `a7af73f` (DCR-0032's amendment) and `c5ff3df` (the two fix reviews'
document findings plus the corpus gate's pathspec). `1299280` is the commit wave
1 built on, and reading it as wave 1's would double-count it against DCR-0032.
