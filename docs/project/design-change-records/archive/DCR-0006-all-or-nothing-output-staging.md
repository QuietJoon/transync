---
type: DCR
title: CLI outputs commit all-or-nothing
description: The CLI's per-file atomic writes became one staged fileset commit; the documented mixed-output partial state on exit 4 is gone except for rename-pass failures.
tags: [change, project-control, DCR-0006]
status: deprecated
---

# DCR-0006: CLI outputs commit all-or-nothing

- **Date:** 2026-07-10
- **Source:** Review 0006, Issue R0006-0012 (user-approved fix route) (review archived and removed)
- **Affected ADRs:** none directly. Steady-state docs updated:
  `docs/architecture/contracts.md` §6 (exit-code 4 wording) and
  `docs/architecture/persistence-and-files.md` §atomic-write.

## What Changed

Previously each output (`out.md`, `--map`, then each `--html-out` bundle
file) was written atomically **per file, sequentially**; a mid-sequence
failure exited 4 leaving a documented mixed set of outputs.

Now `translate` serializes and preflights everything up front, then commits
the entire output set through `output::write_fileset_atomic`: stage all
payloads as `<name>.tmp.<pid>` (fsynced) → rename all → best-effort parent
fsyncs. A failure during staging removes every temp and leaves every target
untouched. Temps append to the full file name (`out.md.tmp.<pid>`) so
same-stem siblings cannot collide; the `--html-out` foreign-file guard also
recognizes and removes both the current and the legacy temp-name schemes.

## Why

"Some outputs may exist" on exit 4 forced consumers to treat every non-zero
exit as potentially-corrupt state. Shrinking the mixed-set window to the
final rename pass makes exit 4 mean "your outputs are as before" in
practice. Details in contracts.md §6.

## Affected Areas

- `crates/transync-cli/src/output.rs` (`write_fileset_atomic`,
  `html_bundle_files`, `preflight_html_out`; `write_atomic` and
  `write_html_bundle` removed)
- `crates/transync-cli/src/translate_cmd.rs` (single commit point)
- `docs/architecture/contracts.md` §6, `docs/architecture/persistence-and-files.md`

## Migration / Follow-up

- Consumers that probed for the old partial state on exit 4 can rely on
  untouched targets unless the stderr message names a rename failure.
- `web/SMOKE.md` and scripts needed no change (they consume final names).

## Terminology update (2026-07-13, external review P1-6)

The concept this DCR named "all-or-nothing" is renamed to **staged fileset
commit** in the steady-state docs and code (the mechanism is unchanged). The
rename is honesty: a failure inside the final rename pass *can* still leave a
mixed set of old and new targets, so "all-or-nothing" over-promised. The
retitled section is `docs/architecture/persistence-and-files.md`
§staged-fileset-commit (formerly §atomic-write); `output.rs` TRACE strings and
contracts.md §6 track the new anchor. This DCR's own historical wording above
is left intact.

The same review shipped `--out-dir` (P1-6), which extends the staged-commit
discipline to a whole directory published with one rename (backup-and-swap for
an existing target). Contracts.md §6 and persistence-and-files.md carry the
exact semantics.
