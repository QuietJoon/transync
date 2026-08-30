---
type: ADR
title: The browser suite is Chromium-only, and that is accepted rather than unnoticed
description: sync.js is verified in Chromium alone while the shipped bundle opens in arbitrary browsers; the gap is accepted for now, with the conditions that would reopen it written down.
tags: [decision, ADR-0026]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-29T09:20:00Z
status: stable
---

# ADR: The browser suite is Chromium-only, and that is accepted rather than unnoticed

## Context and Problem Statement

Found in Review 0009 (Issue R0009-0020, Severity: Low — verification agreed
with the severity, which is unusual in this round: 55 of its 91 findings were
overstated).

Location: `web/playwright.config.js`, `projects:`.

The Playwright suite declares exactly one project:

```js
projects: [ { name: "chromium", use: { ...devices["Desktop Chrome"] } } ]
```

and the file's own header explains why — *"Chromium only, headless, single
worker (the scroll-timing assertions want a quiet CPU and one shared static
server)"*. `ls ~/Library/Caches/ms-playwright` confirms it: only
`chromium-1228` and `chromium_headless_shell-1228` are installed. No Firefox,
no WebKit.

The gap that makes this worth a record: **the `--html-out` bundle is opened by
end users in whatever browser they have.** `web/js/sync.js` rests on
`scrollTop`, `getBoundingClientRect`, `ResizeObserver` and `offsetTop`
geometry, none of which is exercised outside Chromium. A rendering difference
in any of them is a silent sync failure in a user's browser and a green suite
here.

`grep -rni chromium docs/` finds *descriptions* of the Chromium-only state in
`module-map.md` and `Developer_Guide.md` — but **no record weighing the risk**.
That absence is what this ADR closes: a reader could not previously tell
whether the single project was a decision or an accident.

## Decision Drivers

* The bundle's audience is arbitrary browsers; the suite's coverage is one.
* The scroll assertions are timing-sensitive and already run `workers: 1`.
* Neither Firefox nor WebKit is downloaded, and there is no CI to download them.
* A matrix that triples local runtime discourages running the suite at all,
  which costs more coverage than it buys.

## Considered Options

1. **Accept Chromium-only and record the conditions that would reopen it.**
2. Expand to `firefox` + `webkit` across the whole suite.
3. Scope a minimal cross-browser subset — the geometry-dependent specs only —
   and leave the rest Chromium-only.

## Decision Outcome

**ACCEPT (option 1), with option 3 named as the cheapest way back in.**

Expanding the full matrix now would roughly triple a suite that already runs
single-worker for timing reasons, on a machine with no CI and no other browser
engines installed. The realistic outcome is a suite people stop running, which
is worse coverage than a fast Chromium-only one.

What makes this an accepted trade rather than an oversight is that the
conditions to revisit are written down:

* **A sync defect reported in a non-Chromium browser.** One report is enough —
  it converts this from a hypothetical to a measured gap.
* **CI existing at all.** The runtime objection is about a developer's local
  loop; it does not apply to a machine that runs the suite unattended.
* **A change to the geometry assumptions** — anything touching `scrollTop`,
  `offsetTop`, `getBoundingClientRect` or `ResizeObserver` in `sync.js` — at
  which point option 3's minimal subset is the proportionate answer.

Status: Implemented (as a recorded acceptance; no code change).

### Implementation

No code changed. This record and the conditions above are the deliverable.
`web/playwright.config.js`'s header already states the *what*; this states the
*why* and the *when to revisit*.

## Consequences

* Good, because the suite stays fast enough that people run it, and the gap is
  now a known one with named triggers rather than an unexamined default.
* Good, because option 3 is pre-scoped: whoever reopens this does not start
  from a blank page.
* **Bad, because `sync.js`'s geometry assumptions remain unverified outside
  Chromium, and the failure mode is silent** — a user sees panes that do not
  follow each other, with nothing in any log. That cost is accepted here, not
  denied.
