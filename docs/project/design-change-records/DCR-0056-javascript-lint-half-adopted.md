---
type: DCR
title: The JavaScript lint half is adopted
description: DCR-0049 adopted biome format-only and left the lint half as a separate open decision, because 30 of the corpus's 36 diagnostics were one style rule whose every fix rewrites a defensive guard in the sync engine. This record turns the linter on, applies those rewrites, and verifies them against the browser suite rather than against an argument about equivalence.
tags: [browser, process, DCR-0056, DCR-0049, OI-0039]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-15T13:42:19Z
status: stable
---

# DCR-0056: The JavaScript lint half is adopted

- **Date:** 2026-09-15.
- **Source:** a `/check-missing` audit, which reported the pre-commit hook's
  JS leg as format-only; owner decision to close it.
- **Supersedes:** the still-open half of DCR-0049 / OI-0039 option A'. DCR-0049
  is a Record and is **not** edited; this record is its successor.
- **Behaviour changed:** none at runtime. The four JS files that changed are
  semantically unchanged; see *Equivalence* below.

**Numbering note.** Next free number after DCR-0055.

## What DCR-0049 deferred, and why

Option A' adopted biome for formatting and recorded the lint half as separate
and still open. The measurement behind that: enabling the linter produced 36
diagnostics over the 12-file corpus, and 30 of them were
`complexity/useOptionalChain` — a single style preference whose every fix
rewrites a defensive guard inside `sync.js`, the file the product's defining
scenario depends on. Adopting it would have meant 30 mechanical rewrites of
guard code or 30 suppressions, and DCR-0049 declined to take either as a
drive-by.

The deferral named a re-trigger: *"a JS defect reaching a shipped bundle that a
lint rule would have caught."* **That trigger has not fired.** This record does
not pretend otherwise. The decision to adopt now is the owner's, taken on the
grounds that a gate which exits 0 while validating nothing is worse than the
rewrites it was avoiding — not on the grounds that the recorded condition was
met.

## The decision

`biome.jsonc` sets `linter.enabled: true`. The assist actions stay off.
`scripts/hooks/pre-commit` runs `biome lint --error-on-warnings` alongside
`biome format`.

`--error-on-warnings` is load-bearing, not decoration. biome exits **0** on
warning-level diagnostics, and 31 of the 36 were warnings. Without the flag the
new leg would print every diagnostic and still pass — reproducing, in the lint
half, the exact defect OI-0039 was filed against in the format half.

## Equivalence

All 36 diagnostics are fixed; the corpus lints clean, so the gate fails on the
*next* diagnostic rather than on a backlog. The rewrites fall in four groups:

| Rule | Count | Shape |
|---|---|---|
| `complexity/useOptionalChain` | 30 | `!a \|\| a.b !== x` → `a?.b !== x`; `a && a.b` → `a?.b` |
| `style/useTemplate` | 3 | two adjacent template literals merged into one |
| `style/useExponentiationOperator` | 2 | `Math.pow(a, b)` → `a ** b` |
| `correctness/noUnusedVariables` | 1 | an unused binding in a test renamed `_logs` |

biome marks every `useOptionalChain` fix **unsafe**, and that warning is about
the general case: `a?.b === x` and `!a || a.b === x` disagree when `x` is
`undefined`. Every instance here is a `!==` comparison or a plain guard read,
where a nullish receiver yields `undefined` on both spellings and the branch is
taken either way. The one case worth naming explicitly is the schema gate
`if (!match || match[1] !== "1")` → `if (match?.[1] !== "1")`: `exec` returns
an array or `null`, and both spellings reject on `null`.

## Why the browser suite is the evidence

An argument about optional-chaining semantics is not verification of a scroll
engine. The rewrites were verified by running the headless suite —
`scripts/test-browser.sh`, 47 tests, all passing — which exercises SCN-13
dual-pane sync over the rewritten guards, the SCN-16 HTML-run bundle through the
shipped shell, `sync.js`'s mount-refusal contract driven directly, and the wasm
render+edit demo. Several of those tests drive precisely the refusal paths the
rewritten guards implement: an alien map, a non-array `blocks`, a duplicate row,
a reversed range, a missing `DOMPurify`.

The two `sync.js` copies stay byte-identical, as `sync_js_drift.rs` requires.

## What this does not do

- No type checking. Nothing runs `tsc`; there is no `tsconfig.json` in the JS
  root.
- The assist actions stay off, unchanged from DCR-0049.
- No rule was suppressed or disabled to reach a clean corpus. The default
  recommended set is what runs.
