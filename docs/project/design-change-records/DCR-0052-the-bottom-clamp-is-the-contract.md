---
type: DCR
title: The bottom clamp is the contract — a browser test that asserted the dead zone is corrected, not the engine
description: engine.spec.js test l went red at 2ca092f and was filed as a priority-1 anchor capture — a duplicate of a listed id resolving to its second occurrence after a reflow recompute. Three measurements say otherwise: with the duplicate removed entirely the follower still lands on the same pixel, with the pre-clamp sync.js served under the unchanged test it lands at 0, and a duplicate appended after mount still pairs with its first occurrence. The mechanism is OI-0047's new bottom clamp, whose destination coincided with the planted duplicate's top. The clamp stands, the assertion is rewritten out of the pane's geometry, and test l gains the two recompute legs the suite never had.
tags: [change, project-control, DCR-0052]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-05T00:00:00Z
status: stable
---

# DCR-0052: The bottom clamp is the contract

- **Date:** 2026-09-05
- **Source:** ticket `8cd7ba` (P1, filed as an OI-0035 impostor surface)
- **Affected contracts:** `docs/architecture/contracts.md` §4a — which
  described the anchor set and the reflow budget and said nothing about what
  the engine answers past the end of the anchored region
- **Affected records:** `docs/project/open-issues.md` OI-0047 gains a dated
  note. It is not rewritten; it remains correct as written for its own date.
- **Engine behaviour changed:** none. `web/js/sync.js` and its welded twin
  gain one explanatory comment and nothing else.

**Numbering note.** DCR numbering is independent of ADR numbering. DCR-0035–0039
remain reserved by the HTML→HTML waves; this is the next free number after
DCR-0051.

## What this record is for

A priority-1 security ticket was filed against a defect that does not exist,
against a number that two different mechanisms produce identically. The ticket
is not careless — its measurement is correct, its arithmetic is correct, and its
reading of the arithmetic is the one the fixture invites. What it lacked was a
control. This record exists so the next reader of `activeBlockWithProgress` does
not re-file it, and so the method failure is written down where the method is.

## What was measured, and what it was read as

`web/tests/engine.spec.js` test `l` builds five 150 px anchors per pane in a
200 px source pane, plants a DUPLICATE `e-0001` at the bottom of the follower —
offsetTop 750, which is exactly where `e-0005` ends — then appends an
`x-8888` the alignment map does not claim to both panes and forces a reflow
recompute. It drives the reader 4 px into `x-8888`'s band and asserted the
follower had not moved:

```js
expect(await scrollTopOf(page, TGT)).toBeLessThan(50);
```

Observed: **746**, deterministically. Read as: the partner lookup for `e-0001`
answered with the second occurrence — the planted duplicate at 750, less the
engine's 4 px reference offset — which would mean the first-occurrence policy
holds at mount and is lost in the reflow recompute. That would be a real
invariant-1 break, and worse than the residual `sync.js` documents about itself,
because a duplicate sitting *after* the genuine anchor would capture the
follower with no attacker timing required.

## Why that reading is wrong

Three measurements, each against the shipped CLI-emitted bundle:

1. **Remove the planted duplicate entirely.** The follower still lands at 746.
   Nothing about a duplicate is load-bearing in the observation.
2. **Serve `51f0d93`'s `sync.js` under the unchanged test.** The follower lands
   at 0 — the assertion passes. The engine changed, not the test.
3. **Append a duplicate `e-0002` after mount, force a recompute, drive onto its
   band.** The follower pairs with the FIRST copy. The policy survives the
   recompute, which is the thing the ticket says it does not.

The mechanism is OI-0047's bottom clamp, added to `activeBlockWithProgress` in
commit `2ca092f` — the same commit in which the test went red. The reader sits
`REFERENCE_OFFSET_PX` past `e-0005`'s bottom, so no claimed anchor straddles the
reference line and none is below it; the clamp answers `e-0005` at `progress: 1`;
and `handleScroll` drives the follower to the partner's `e-0005` bottom,
`600 + 150 - 4`. That is 746. The planted duplicate's top is 750, and 750 less
the same reference offset is also 746. **The two candidate explanations differ by
nothing at all in the observed number**, which is why a control rather than
sharper arithmetic was what the diagnosis needed.

The first-occurrence policy never had a second home. `collectAnchors` gates both
structures it fills — the document-order scan array and the partner lookup — and
the reflow recompute calls it, with the row gate live and only the *warnings*
suppressed. That was true before this ticket and is true after it.

## The method failure worth recording

The ticket's exoneration of biome swapped `web/js/sync.js`, `web/tests/engine.spec.js`
and `web/tests/support/harness.js` to their `HEAD` versions and observed an
identical failure. The conclusion drawn — "pre-existing, the formatter is not
implicated, the suite was already red at `51f0d93`" — does not follow, because
`engine.spec.js` imports `/sync.js` from the **generated bundle**, which is the
CLI-embedded twin at `crates/transync-cli/web/sync.js` compiled in through
`include_str!`. Swapping `web/js/sync.js` changes nothing that test executes.
The experiment held the engine constant while believing it had varied it, so an
identical result was the only outcome available to it.

The generalization: an A/B over a welded pair must vary the copy the harness
actually loads, and the way to know which one that is, is to change it and watch
the test move. A control that cannot fail is not a control.

## Decision

**The clamp is the contract. The assertion is corrected.**

- The clamp stands. Freezing the follower once the reference line passes the
  last anchor is the defect OI-0047 was filed for, and it is reachable by any
  pane with a viewport of unanchored content below its last anchor — an
  ordinary paragraph does it, `data-sync-id` or not.
- `toBeLessThan(50)` is not weakened; it is **replaced by the geometry**. It
  asserted the follower had not moved AT ALL, which conflated "the late anchor
  is inert" with "the follower stayed home" — one observation standing for two
  properties, only one of which the test is about. The expectation now names the
  clamp's destination, computed from the pane, and separately asserts that the
  destination is above the late anchor's own copy. That is a strictly finer
  discriminator: it rejects 904 (the late anchor driving) *and* every other
  value, where the old bound accepted a 50 px band around a number the engine no
  longer produces.
- Test `l` gains the two legs the ticket asked for and the suite did not have: a
  duplicate that arrives AFTER mount still pairs with its first occurrence
  across a recompute, and the scan array still drops it — the second only
  observable through the clamp, since `lastEnding` is the one consumer of the
  scan array that a late duplicate can move.

Each leg is falsification-tested against a deliberately doctored engine served
from the fixture: the policy applied at mount only lands the follower at 700,
the scan array keeping every duplicate lands it at 146, and the row gate skipped
on reflow lands it at 904 — against 746 green. A test whose failure mode is
unmeasured is the shape this ticket was.

## Alternatives rejected

- **Revert or narrow the clamp so the old bound passes.** Rejected: the only
  behaviour that satisfies `< 50` here is the dead zone, and narrowing the clamp
  to fire on unanchored content but not on unanchored content that happens to
  carry an unclaimed `data-sync-id` would put a second opinion about the row
  gate inside the scan — the defect class this session spent itself removing.
- **Leave the suite red and report.** Rejected: a manual suite with no CI is
  already carried by whoever remembers to run it, and a red the reader is
  expected to know is benign is a suite that stops being run.
- **Weaken the bound to `< 800`.** Rejected as the thing the ticket rightly
  forbade — it would keep the test passing while still not saying what the test
  is for.

## What stays open

Nothing from this ticket. The residual `sync.js` documents about itself — an
impostor carrying a **listed** id that PRECEDES the genuine anchor, in a pane
transync did not produce — is untouched by any of this, and remains recorded in
`contracts.md` §4a as the blind spot the render-side strip covers.
