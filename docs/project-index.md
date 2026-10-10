---
type: "Documentation Index"
title: "Project Documentation Index"
description: "Curated entry point to the transync documentation: where to start for each kind of reader, the authority order between documents, and what is deliberately not written down"
tags: ["docs", "project", "process"]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-15T13:40:48Z
---
# Project Documentation Index

The hand-curated way in. [`index.md`](index.md) is the OKF bundle-root index and
lists *everything*; this file says what to read, in what order, and why.

## Start here

| If you want to… | Read |
|---|---|
| Build it and see two panes scroll together | [Quick Start](Quick_Start.md) |
| Use transync as a library or drive the CLI | [Developer Guide](Developer_Guide.md) |
| Understand how it works | [Architecture Overview](architecture/README.md) |
| Diagnose a run that failed or looks wrong | [Troubleshooting](Troubleshooting.md) |
| Make a translation read the way you want | [Profile Cookbook](Profile_Cookbook.md) |
| Make it faster | [Performance Tuning](Performance.md) |

## Authority order

When two documents disagree, [`project/design-baseline-2026-07.md`](project/design-baseline-2026-07.md)
(`BL-2026-07-B`) decides which one is authoritative and which one gets fixed.
Below it: the ADRs in [`decisions/`](decisions/) carry steady-state principles,
and the DCRs in [`project/design-change-records/`](project/design-change-records/)
carry delta events. A DCR is write-once — a later change is a *new* DCR, never an
edit to an old one.

`CLAUDE.md` at the repository root classifies every tracked document as Living,
Register, Record or Archive, and that kind decides how the document may change.
Read it before editing anything here.

## The shape of the architecture set

[`architecture/README.md`](architecture/README.md) is the door. From there:

* [MVP Scope](architecture/mvp-scope.md) — what is in, what is deliberately out.
* [Scenario Matrix](architecture/scenario-matrix.md) — SCN-01…SCN-16, each with the test layer that verifies it.
* [Contracts](architecture/contracts.md) — every wire- and process-crossing surface, normatively.
* [Source-of-Truth Table](architecture/source-of-truth-table.md) — which crate owns which concern.
* [Persistence and Files](architecture/persistence-and-files.md) — every file read or written, and who owns its lifecycle.

## Current state

* [Status](project/status.md) — where the project stands, and the standing gaps.
* [Open Issues](project/open-issues.md) — the live register of accepted, unresolved issues.
* [Backlog](backlog.md) — everything the `/reopen` sweeps found, classified.
* `project/phase-state.yaml` — machine-readable phase state; not an OKF concept, so it carries no frontmatter.

## Not written down, on purpose

* **Operational detail** — ID format, prompt versions, schema versions — lives in
  code and module docs, because a document that restates a constant drifts from it.
* **Per-developer setup** belongs in `.claude/settings.local.json`, not here.
* **Korean siblings** (`*.ko.md`) are generated and git-ignored; they are never
  the source of anything.
* **`investigation/`** is regenerated from the code and untracked. Treat it as a
  cache, never as a record.
