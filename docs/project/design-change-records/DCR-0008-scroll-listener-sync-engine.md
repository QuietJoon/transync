---
type: DCR
title: Scroll-listener sync engine replaces the IntersectionObserver design
description: The shipped JS sync engine uses RAF-coalesced scroll listeners with a reference-line active-block pick and per-frame lerp, not the IntersectionObserver algorithm ADR-0001 originally specified.
tags: [change, project-control, DCR-0008]
status: active
---

# DCR-0008: Scroll-listener sync engine replaces the IntersectionObserver design

- **Date:** 2026-07-10
- **Source:** OI-0006 [archived] (R0001-0064); divergence shipped with the SCN-13 work and (review archived and removed)
  is specified in `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`
- **Affected ADRs:** docs/decisions/0001-block-level-alignment-as-sync-currency.md (updated)

## What Changed

ADR-0001 specified active-block detection via `IntersectionObserver`
(intersection ratio + viewport-center distance + hysteresis). The shipped
`web/js/sync.js` (mirrored into the CLI bundle) instead uses:

- RAF-coalesced `scroll` listeners per pane;
- a reference line a few pixels below the pane's visible top — the block
  straddling it is active, with a 0..1 progress fraction inside it
  (topmost-visible fallback);
- a per-frame lerp toward the partner's computed scrollTop (clamped to
  the reachable range) with a settle threshold;
- a per-pane programmatic-scroll lock (released by wheel / touch /
  pointer / keyboard input) instead of hysteresis.

The sync currency is unchanged: block IDs via `[data-sync-id]` only.

## Why

Continuous proportional tracking needs a stable per-frame position signal;
`IntersectionObserver` callbacks are threshold-based and too coarse for the
intra-block progress fraction the smooth-scroll spec requires. Full
rationale lives in the smooth-scroll spec; this DCR makes the delta visible
from the decision record so new contributors don't build the wrong mental
model (the exact failure OI-0006 recorded).

## Affected Areas

- `docs/decisions/0001-block-level-alignment-as-sync-currency.md` (amended
  in place)
- `web/js/sync.js` + `crates/transync-cli/web/sync.js` (no code change in
  this session — already the shipped behavior)
- `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md` (authoritative
  algorithm description)

## Migration / Follow-up

- OI-0006's optional third action — revisiting whether an
  IntersectionObserver-based pick should replace or augment the scroll
  listener — remains a judgment call for a future perf pass (see OI-0016
  for the related per-frame-scan note). No code follow-up otherwise.

## Note (2026-08-09, ticket `d3acc3`) — the engine now reacts to reflow, and its pairing rule is stated

The record above describes an engine that reacts to exactly one thing:
`scroll`. Two additions, neither of which changes the algorithm this DCR
decided.

1. **Three reflow signals join `scroll`** (OI-0024 item 1, closed here):
   a `ResizeObserver` on both panes, `document.fonts.ready`, and
   `load`/`error` on `<img>` elements inside either pane. On any of them the
   engine re-collects both anchor sets and re-runs the *last driving* pane's
   scroll handler, coalesced into one animation frame. The reference-line
   pick, the intra-block progress fraction, the per-frame lerp and the
   per-pane programmatic-scroll lock are untouched — a recompute is an extra
   *entry* into `handleScroll`, not a different `handleScroll`. What it
   replaces is the caller-driven destroy-and-remount the docstring demanded
   for a resized window, a late webfont or a late image. Replacing a pane's
   HTML is still a re-mount, because a replacement is not a reflow.
2. **Pairing by identical `data-sync-id` is normative** for alignment schema
   1.x (ADR-0001's 2026-08-09 amendment, `contracts.md` §3). This DCR's
   closing line — "the sync currency is unchanged: block IDs via
   `[data-sync-id]` only" — now has a stated companion: *which* id, on which
   side. The alignment map's source/target indirection is reserved for a
   future divergence revision and is not routed through; a schema-1.x map
   whose row contradicts the identity is refused (R0003-0002).

**Single-file packaging was re-affirmed, not revisited** (OI-0015, closed
here). The question OI-0015 held open was whether this engine should be split
into engine-vs-helpers modules once a substantive JS feature landed. The
feature landed — the reflow hooks above — and the answer is no: `web/js/sync.js`
is mirrored byte-for-byte into the CLI bundle under a drift test, and `web/`
is a no-build, framework-free tree, so a split would multiply both the mirror
and the assets every consumer ships to buy a source boundary neither needs.
The module doc-comment carries the re-affirmation so the next reader finds it
in the file rather than only in a record.

## Note (2026-08-12, R0004-0087) — a fourth reflow signal: the `<details>` toggle

The 2026-08-09 note above lists three reflow signals. There are four. A
`<details>` toggle inside either pane now raises the same recompute, from the
toggle mirror itself.

It is the odd one out twice over. It is a *content* reflow — the pane box is
unchanged, so the `ResizeObserver` on both panes says nothing, and no font and
no image are involved — and it is the only reflow the engine causes itself,
because mirroring a disclosure across the panes reflows the partner. Equal
growth on both sides would need no correction, but translated body text wraps
differently, so the two disclosures rarely grow by the same number of pixels
and the follower was left where the pre-toggle geometry had put it until the
reader scrolled again.

Nothing about the decision this DCR records changes: the recompute is still an
extra *entry* into `handleScroll`, the toggle mirror still mirrors exactly as
decision 9 specified, and the signal is coalesced into one animation frame like
every other. `controller.refresh()` keeps its purpose, minus one of its two
documented examples — a `<details>` opened by script fires a `toggle` like any
other and is now self-healing. Pinned by Playwright `engine.spec.js` test `j`.
