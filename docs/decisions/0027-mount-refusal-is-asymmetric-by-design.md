---
type: ADR
title: mountSync refuses a map no anchor can drive, but not panes no row can drive
description: The mount refusal fires on an unusable alignment map and deliberately not on unusable panes, because a blanket refusal would break the mount-empty-then-populate pattern refresh() exists to serve.
tags: [decision, ADR-0027]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-29T09:22:00Z
status: stable
---

# ADR: `mountSync` refuses a map no anchor can drive, but not panes no row can drive

## Context and Problem Statement

Found in Review 0009 (Issue R0009-0023, Severity: Medium — verification
confirmed the mechanism and judged the severity **overstated**).

Location: `web/js/sync.js`, `mountSync`'s refusal guard.

The guard refuses exactly one direction:

```js
if (anchorCount > 0 && synchronizableRowCount(alignmentMap) === 0)
```

— panes that carry anchors, paired with a map that claims none of them. The
mirror case is not refused. Reproduced directly: `mountSync` with a two-row map
over panes carrying only `ghost-1`/`ghost-2` **returns a live controller**
(`keys: destroy, refresh`) while emitting six warnings — four *"ignoring anchor
`ghost-N` … no alignment row claims it"* and two *"alignment map block `p-N`
has no DOM anchor in source + target pane"*.

So the return value reports success for a mount where **no row can ever drive
scroll**. The reviewer proposed symmetry: refuse both directions.

## Decision Drivers

* The return value is the only machine-readable signal a caller gets; six
  console warnings are not a substitute for it.
* But the console is **not** silent — a developer sees exactly what is wrong.
* `controller.refresh()` (`refresh: reflow.schedule`) exists to serve mounting
  panes *before* they are populated, then recollecting. Recollection re-runs
  `collectAnchors` against the same row ids.
* A blanket refusal at mount would break that pattern outright: the legitimate
  empty-then-populate caller gets `null` and has nothing to call `refresh()` on.

## Considered Options

1. **Keep the asymmetry and record why** — refuse an unusable *map*, warn on
   unusable *panes*.
2. Refuse both directions symmetrically.
3. Refuse both, and add a separate opt-in for the empty-then-populate caller.

## Decision Outcome

**ACCEPT the asymmetry (option 1).**

The two cases are not mirror images, which is what makes symmetry the wrong
instinct here. An alignment map that claims no anchor in populated panes is
**terminal** — the map is the contract, it cannot change after mount, and no
later event makes it usable. Panes that carry no matching anchors are
**transient** — that is precisely the state `refresh()` is built to resolve,
and DOM content arriving after mount is an ordinary lifecycle, not an error.

Refusing the transient case to match the terminal one would trade a real
supported pattern for a stronger-looking return value.

Two things the reviewer's argument got wrong, recorded so the case is not
relitigated from the same premises:

* It cited **DCR-0033** as authority. That record governs *anchor trust* — which
  ids the engine will act on — not mount-refusal policy. Citing it here is an
  argument from silence.
* It described the unrefused case as silent. It is not: six warnings name every
  ignored anchor and every unmatched row.

Option 3 was rejected as premature: it adds an API surface for a caller nobody
has reported, and R0002-0047's refusal already covers the case that actually
bit someone.

Status: Implemented (as a recorded acceptance; no code change).

### Implementation

No code changed. The asymmetry stands as written; this record supplies the
reasoning `sync.js`'s guard could not carry inline, and names the two premises
that make the symmetric argument fail.

## Consequences

* Good, because the empty-then-populate mount that `refresh()` exists for keeps
  working.
* Good, because the next reviewer to notice the asymmetry — and one will, it is
  visible in four lines of code — finds the reasoning instead of re-deriving it.
* **Bad, because `mountSync`'s return value still reports success for a mount
  that cannot drive scroll.** A caller checking only the return value learns
  nothing; it must read the console. That is the accepted cost, and the
  cheapest future improvement is a richer return value rather than a refusal —
  which would inform the caller without breaking the transient case.
