---
type: DCR
title: The v0.5.0 blocker clearance — the review-0009 cluster is dispositioned, and the arc does not reach a tag
description: The ten open issues filed from Review 0009 (OI-0039..OI-0048) are all dispositioned, four of them by a recorded deferral with an objective self-firing re-trigger rather than by doing the work. OI-0039 adopts biome format-only. The arc's own output then re-opened the gate it was clearing: the checking apparatus OI-0046 asked for found a live anchor-injection defect on its first execution, and the first run of the browser suite since the biome adoption found a second one that was already red at HEAD.
tags: [change, project-control, DCR-0049]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-05T00:00:00Z
status: stable
---

# DCR-0049: The v0.5.0 blocker clearance

- **Date:** 2026-09-04 / 2026-09-05
- **Source:** the review-0009 cluster OI-0039..OI-0048 (filed 2026-08-26), plus
  the owner's six decision-required rulings of 2026-09-04
- **Affected contracts:** `docs/architecture/contracts.md` §1 (a 22nd stable
  code, `invalid_options`), §2 (`prompt_html` skeleton), §4a (the recompute
  frame budget), §6 (the `out.md | out.html` allow-list and exit-6's causes),
  §7 (`offline()`)
- **Affected records:** OI-0039..OI-0048 dispositioned in
  `docs/project/open-issues.md`; eleven notes and two new entries in
  `docs/backlog.md`

## What this record is for

The release condition in `docs/project/phase-state.yaml` is a **union**:
v0.5.0 ships only after every registered task and every open issue is resolved,
*excluding those explicitly deferred*. That wording makes "defer with a
recorded decision" a first-class route to clearing a blocker — and four of the
ten took it. This record says which four, on what evidence, and what has to
happen for each deferral to expire, so that a later reader can tell a decision
from a promise.

## The ten dispositions

| Issue | Disposition |
|---|---|
| OI-0039 | biome adopted **format-only**; the lint half deferred with a re-trigger |
| OI-0040 | prefilter + tripwire; the seam member deferred |
| OI-0041 | partial — the rest declined on measurement or deferred |
| OI-0042 | all three sites fixed, the CLI twin included |
| OI-0043 | `output.rs` split 4,005 -> 411 lines, proven a pure move |
| OI-0044 | **still OPEN** — the trim member fixed; the unbounded read and the poisonable write remain |
| OI-0045 | fixed |
| OI-0046 | all three holes closed; hole (3) found a live defect |
| OI-0047 | fixed |
| OI-0048 | fixed; R0009-0052 deferred |

**OI-0044 is not cleared and this record does not claim it is.** Two of its
three members (R0009-0080 unbounded read, R0009-0081 poisonable write) are
open, so the register is at **one** open issue, not zero. An earlier recount in
the session keyed on resolution dates and reported zero; that was wrong, and
the summary table's own `OPEN` row is the correct reading.

## OI-0039: biome is adopted for format only

The measurement decided the shape. Run over every JS blob version in the
repository's reachable history, biome's linter produced 62 diagnostics, **61 of
which are one style preference** (`complexity/useOptionalChain`) whose every
fix rewrites a defensive guard inside the sync engine. Adopting the lint half
would therefore have meant either 61 mechanical rewrites of guard code in the
one file the product's defining scenario depends on, or 61 suppressions. Both
are decisions with their own consequences, so the lint half is recorded as
separate and still open rather than taken as a drive-by.

`biome.jsonc` is the adoption record, and its **presence** — not a probe of
`node_modules` — is what tells the pre-commit hook this repository has a
JavaScript format gate. That inversion is the point of the fix: OI-0039 was
filed because a gate exited 0 while validating nothing, so a config in the tree
with no installed formatter is now a **failure**, not a shrug.

The corpus is **git's, not a directory walk**. The old leg did `cd web` and
walked `.`, which never saw `crates/transync-cli/web/sync.js` or
`benchmark/scroll-frame/profile.mjs` — 2 of the 11 hand-written files, 22% of
the bytes — while it *did* read the vendored `purify.min.js` and the gitignored
`wasm/` build output. The tool is resolved under `web/`; the corpus comes from
`git ls-files '*.js' '*.mjs' '*.cjs'` less the two byte-pinned minified vendor
copies, and the hook runs from the repository root.

Three cases that used to pass now fail explicitly: an **empty** file list, a
**declared-but-absent** tool (a broken install, not an absent decision), and an
**orphaned config**. The end-of-run summary states that no lint rule ran, so a
green commit means the browser code is formatted — not that it was linted.

### The deferral's re-trigger had already fired

The lint-half deferral was first written against the trigger "the first commit
that already moves `web/js/sync.js` and its CLI twin together". That condition
was **already met in the same commit that wrote it** — OI-0047's fix moves both
copies, because `crates/transync-cli/tests/sync_js_drift.rs` welds them
byte-identical. A deferral whose trigger has already fired is vacuous, so the
owner was asked and chose option A': adopt the format half now, and let the
lint half stand as its own decision with no self-firing trigger attached.

## The arc did not reach a tag, and the reason is the arc's own output

This is the part worth recording. Two of the ten dispositions produced the
checking apparatus that then found live defects:

1. **OI-0046 hole (3)** asked for generated-input properties over the HTML
   mechanics. On their **first execution** they found `balance_fragment`'s
   orphan-deletion pass welding a literal `<` onto its neighbour — an
   anchor-injection route reachable from untrusted source Markdown (ti
   `fdd989`, fixed and verified; four harms, 1.32M generated inputs and 9,214
   Chromium cases). The measured reason the 41-golden corpus could not find it:
   **not one of the 38 pinned edge cases produces a single orphan close tag.**
   Adversarial verification of that fix then found three further routes that
   are *not* the weld (ti `9b4d66`, `307283`, `895fb7`) plus the reason the
   harness could not see them (ti `ec235f`): its oracles are built from the
   functions under test, so a divergence between this crate's stacks and a
   browser's is invisible by construction.

2. **The biome adoption** made the browser suite worth re-running, and its
   first run since found `web/tests/engine.spec.js` test `l` red — the
   duplicate-anchor first-occurrence policy holds at mount and is lost in the
   reflow recompute path (ti `8cd7ba`). Swapping the three JS files to their
   `HEAD` versions reproduces it identically, so **the formatter is exonerated
   and the suite was already red at `51f0d93`**. It went unnoticed because the
   repository has no CI and this suite is manual: the standing "gates green"
   claim covered the workspace (1291/0), the CLI stub suite (217/0) and wasm,
   and did not include it.

So the honest state at the end of this arc is **not** "ready to tag". It is:
the register's review-0009 cluster is dispositioned, one issue (OI-0044)
remains open, five tickets are open, and Review 0010's 93 findings are
untriaged.

## Consequences

* Good, because four blockers cleared through a recorded decision instead of
  speculative work, and each names what would reopen it.
* Good, because the two gates this arc built or restored both earned their keep
  immediately — each found a live defect on its first execution, one of them a
  security route.
* Bad, because the release date moved: the apparatus found more than it
  cleared, which is the correct outcome for a gate and an inconvenient one for
  a tag.
* Bad, because a manual browser suite was red for an unknown number of commits.
  ti `8cd7ba` carries the defect; the absence of CI that let it hide is
  recorded in `docs/backlog.md` and is not fixed here.

## Verification

- `cargo test --workspace -- --test-threads=4` — **1318 passed, 0 failed**, 47
  suites, `CARGO_EXIT=0`.
- `cargo clippy --all-targets -- -D warnings` — exit 0, zero warnings.
- `cargo clippy --all-targets --all-features -- -D warnings` (the hook's exact
  form) — exit 0, zero warnings.
- `cargo fmt --all` — exit 0.
- `biome format` over the explicit 11-file git-derived corpus — exit 0,
  no fixes applied.
- `scripts/test-browser.sh` — **41 passed, 1 failed**; the failure is ti
  `8cd7ba`, reproduced identically on `HEAD`'s JS and therefore pre-existing.
