---
type: DCR
title: Review 0002 hardening — the render seam becomes fallible, and three guards stop trusting their input
description: The renderer, the wasm structural gate and the publication guards each stopped trusting an input a recorded design assumed was well-formed; the render seam's signatures changed as a result.
tags: [change, project-control, DCR-0022]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-08T00:00:00Z
status: stable
---

# DCR-0022: Review 0002 hardening — the render seam becomes fallible, and three guards stop trusting their input

- **Date:** 2026-08-08
- **Source:** Review 0002, issues R0002-0010, R0002-0011, R0002-0014, R0002-0002, R0002-0029 (review archived and removed)
- **Affected ADRs:** `docs/decisions/0006-renderer-output-shape.md` (updated — 2026-08-08 amendment), `docs/decisions/0019-wasm-demo-layer.md` (unchanged; its posture held — see below), `docs/project/design-change-records/DCR-0020-track-c-wasm-demo.md` (updated), `docs/project/design-change-records/DCR-0021-publication-lock-and-honest-replacement.md` (updated)

## What Changed

**The render seam is fallible.** ADR-0006's implementation-seam list said
`render_source` / `render_target` take a document and an alignment map and
return a `String`; its 2026-08-04 amendment restated that the signatures were
kept. Both now return a `Result`. The renderer had treated a caller-supplied
alignment map as well-formed in two ways that failed silently: a repeated
`source_block_id` let whichever row the `HashMap` indexed last decide an id's
attributes and byte ranges, and a block with no row was skipped outright —
no anchor, no placeholder, no warning — which contradicts contracts.md §4a's
per-row anchor-survival guarantee. Both are producer defects that
`align::build_alignment_map` cannot commit, so they were reachable only
through the one caller that supplies its own map: `transync-wasm`'s view mode.

**The wasm structural gate compares shape, not size.** DCR-0020 shipped
`check_top_level_structure` as a top-level *count* comparison. A same-count
kind substitution therefore reshuffled content under the wrong sync anchors
with no `structure_warning` — a hole in the announced-degrade promise the gate
exists to keep. It now compares normalized label sequences plus per-list item
counts, through `transync-syntax`'s shared `walk` module rather than a private
copy.

**Publication stopped trusting the directory it was handed.** DCR-0021 gave
publication an exclusive per-directory lock. Under it, three guards changed:
rollback removes only directories it created *and* that are still empty (a
`remove_dir_all` could delete a woken peer's committed output); staging and
backup siblings carry a random per-run token instead of a bare pid, so every
sibling a run deletes is one it made; and the destination preflight now runs
*before* the provider call, with every guard still re-run authoritatively
under the lock at publication time.

**ADR-0019's posture held and is worth recording as unchanged.** Three
findings (R0002-0009, R0002-0012, R0002-0013) argued the wasm *engine* should
validate what its caller supplies. It does not, deliberately: ADR-0019 and
DCR-0020 place the gates in the demo, and the engine is stateless with the
same caller supplying both source and payload, so no trust boundary is
crossed. Those three were rejected. The two renderer findings above were not
the same argument — they are inside a seam that promises anchor survival.

## Why

A guard that silently accepts malformed input is indistinguishable from one
that has nothing to guard. Each change here makes a promise the design already
made — anchor survival, announced degradation, publication that deletes only
its own work — actually enforceable at the seam that makes it.

## Affected Areas

- `crates/transync-syntax/src/render.rs`, `crates/transync-syntax/src/parser.rs`
- `crates/transync-wasm/src/engine.rs`
- `crates/transync-cli/src/output.rs`, `crates/transync-cli/src/translate_cmd/publish.rs`
- `docs/architecture/contracts.md` (§4a, §6), `docs/implementation/module-map.md`

## Migration / Follow-up

- The render seam sits in `contracts.md` §0 tier (c), not the curated facade,
  so the signature change moved no §0 row. It was made under the owner's
  2026-08-07 ruling that compatibility is not a consideration — transync has
  never been released and its only consumers are two path-dependent sibling
  checkouts, which recompile.
- Both sibling consumers (`resp-translator`, `dynwebserver`) treat the
  alignment map as opaque JSON and call no renderer constructor, so neither is
  expected to need migration; release-checklist step 8 verifies that against
  the commit to be tagged.
- The next release is **v0.4.0**, not a patch: this DCR's signature change,
  plus the `TranslatorError` taxonomy and `Translator` cancellation work queued
  behind it, are breaking under 0.x semver.
- `R0002-0018` (raw-HTML-injected `data-sync-id` anchors can pre-claim a real
  block's id) and `R0002-0028` (a sparse allow-listed subset counts as a
  genuine out-dir) were routed to tracking, not fixed; they are registered in
  `docs/project/open-issues.md` and `docs/backlog.md`.
