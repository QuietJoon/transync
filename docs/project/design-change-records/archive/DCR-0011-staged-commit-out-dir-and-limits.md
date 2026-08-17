---
type: DCR
title: Staged fileset commit naming, --out-dir atomic directory publish, and CLI admission limits
description: The output contract is renamed to "staged fileset commit" (honest about phase-2 rename exposure); a new --out-dir mode publishes the complete output set via one directory rename; CLI-boundary input caps land per the amended ADR-0016.
tags: [change, project-control, DCR-0011]
status: deprecated
---

# DCR-0011: Staged fileset commit, --out-dir publish, CLI admission limits

- **Date:** 2026-07-13
- **Source:** External review 2026-07 (EXT-2026-07 P1-6 + the CLI half of P1-7)
- **Affected ADRs:** docs/decisions/0016-whole-document-in-memory.md (amended — admission-control argument accepted; core whole-document stance stands); DCR-0006 carries a terminology-update note (its history unchanged)

## What Changed

- **Honest naming (P1-6):** the multi-path output contract formerly called
  "all-or-nothing" is now the **staged fileset commit** in contracts §6,
  `persistence-and-files.md`, and `output.rs` doc comments. The rename makes
  the documented limitation prominent: phase 1 (stage + fsync every payload)
  is all-or-nothing; phase 2 (renames) can still leave a mixed set on a
  mid-pass failure. Multiple arbitrary output paths cannot be one
  filesystem transaction.
- **`--out-dir <dir>` (P1-6):** publishes the complete output set — `out.md`,
  `alignment.json`, `validation-report.json`, `html/` bundle — by staging
  into a sibling directory and swapping it in with a single rename
  (backup-and-rollback when the target exists; `--force` required when the
  target does not look like a prior transync out-dir). Conflicts with
  `--output`/`--map`/`--html-out`, which remain the best-effort multi-path
  mode.
- **Admission limits (P1-7):** `--max-input-bytes` (default 64 MiB) on
  `--input`, fixed 4 MiB caps on `--profile`/`--system-prompt-file`; all
  reads take `limit + 1` bytes so metadata is never trusted alone;
  exceeding a cap is a deterministic exit-2 error naming the limit.

## Why

The reviewer was right twice: "all-or-nothing" overpromised what multiple
independent paths can deliver, and ADR-0016's "a cap guards nothing"
conflated streaming with admission control — a limit+1 read rejects an
accidental multi-gigabyte file cheaply and deterministically before any
allocation, without contradicting the whole-document-in-memory
architecture for accepted inputs.

## Affected Areas

- `crates/transync-cli/src/{translate_cmd.rs,output.rs}`
- `docs/architecture/contracts.md` §6, `docs/architecture/persistence-and-files.md`
- ADR-0016 amendment; DCR-0006 terminology note

## Migration / Follow-up

- Scripts/consumers keying on the phrase "all-or-nothing" in docs should
  read "staged fileset commit"; behavior of the multi-path mode is
  unchanged.
- `--out-dir` is additive; existing flag combinations work as before.
