# Implementation Impact Report

This report tracks the implementation track's relationship to the design baseline.

## Current state

| Field                                      | Value                              |
|--------------------------------------------|------------------------------------|
| Synced baseline ID                         | `BL-2026-05-01-A`                  |
| Synced stub-manifest version               | `generated-2026-05-01`             |
| Sync status                                | `synced`                           |
| Open design change records                 | none                               |
| Last sync                                  | 2026-05-01 (initial)               |

## Initial sync (2026-05-01)

This is the first sync of the implementation track to the design baseline. The baseline was just published (`fae4d70` Phase 2 + `04b0734` Phase 3+4+5), so there is no prior implementation state to reconcile.

### Slice cutline

The following slices are eligible for parallel work after `SL-00` closes:

- SL-01 (SCN-01 — heading + paragraph)
- SL-02 (SCN-02 — table whole-block)
- SL-03 (SCN-03 — oversized-table row-window)
- SL-04 (SCN-04 — code-block fence regen)
- SL-05 (SCN-05 — nested list with task items)
- SL-06 (SCN-06 — blockquote container)
- SL-09 (SCN-09 — prompt-injection treated as data)
- SL-10 (SCN-10 — long-doc batching + partial-resume)
- SL-11 (SCN-11 — `source-language=auto`)
- SL-14 (SCN-14 — full-document reparse)

Sequenced after the parallel block:
- SL-07 (SCN-07 — validation retry; depends on SL-02)
- SL-08 (SCN-08 — fallback to source; depends on SL-07)
- SL-12 (SCN-12 — CLI end-to-end; depends on SL-01..08, SL-14)
- SL-13 (SCN-13 — JS demo sync; depends on SL-12)

### Blocked slices

None. The design baseline is stable; no DCRs are open.

### Critical / required-real integrations

- OpenAI Responses API via `transync-openai` — required real for SL-12's live job; CI default uses the in-process echo translator under `transync-cli/test-stub-provider` Cargo feature.
- Comrak GFM parser — required real from SL-00 onward.
- All other integrations are listed as deferred in `docs/architecture/mvp-scope.md` §DEFERRED and require no implementation work for MVP.

### Repairs at sync

- **CHANGELOG auto-repair:** not needed. `CHANGELOG.md` exists at the repo root with `[Unreleased]` section and the design-handoff entry naming `BL-2026-05-01-A`.
- **`docs/index.md` refresh:** not needed. The index was refreshed at design handoff and lists every artifact in `docs/`.

### Notes for future syncs

When a Design Change Record lands:
- list it under "Open design change records" above
- mark every impacted slice in `phase-state.yaml.implementation.blocked_slices`
- explain in this report which slices remain safe to continue and why
- bump `synced_baseline_id` only after the impacted slices re-implement against the new baseline
