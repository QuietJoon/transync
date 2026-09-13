---
type: DCR
title: Four document kinds — a document's kind decides how it changes
description: Living documents grew by appending dated notes instead of being edited in place, because a rule written for records was applied to them and the other half of release-checklist step 23 never got a mechanism. This record gives every tracked document one of four kinds — living, register, record, archive — and gives each kind one rule for how it changes. Records become write-once. ADR decision bodies become living, which is the one place this overrides the MADR template. A ratchet test ships green over the tree as it stands and forbids regrowth.
tags: [change, project-control, documentation, DCR-0054]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-13T00:00:00Z
status: active
---

# DCR-0054: Four document kinds

- **Date:** 2026-09-13.
- **Source:** ticket `d4b142dd`; the design spec
  `docs/superpowers/specs/2026-09-13-docs-compaction-design.md`, owner-ratified
  the same day with six settled decisions.
- **Affected documents:** `reviews/README.md` rule 5 and
  `docs/project/release-checklist.md` steps 20b and 23 are **replaced** by this
  record's rules — all three are living documents, so they are edited in place
  rather than annotated. `CLAUDE.md` gains the rule under *Conventions*.
- **Affected ADRs:** none directly. The MADR template's "append an amendment
  section with date and rationale" is overridden for this repository's ADRs;
  see *The one override* below.
- **Behaviour changed:** none. No crate, wire format, CLI surface or contract
  moves.

**Numbering note.** DCR numbering is independent of ADR numbering. This is the
next free number after DCR-0053.

## What this record is for

Measured across 5,651,227 bytes of tracked English Markdown: roughly 13.7% is
amendment scaffolding and 54.6% is dead history. The worst living documents are
`docs/architecture/contracts.md` (31% scaffolding), `docs/index.md` (27%) and
`CLAUDE.md`, whose 6,900-byte dated divergence chain is loaded into every agent
turn.

The cause is not carelessness. It is a good rule applied one category too wide.

`reviews/README.md` rule 5 said dated records are not rewritten to comply, and
that where their ids need a round, **a dated note is appended saying so**. That
is right for a record. Release-checklist step 23 said the same thing and then
added the other half — "living documents are edited in place" — but that half
never got a mechanism, no gate, no trigger, nothing. So the appending half won
everywhere, and `contracts.md` grew 145 stacked amendments inside its own
normative clauses.

Four registers make the same mistake from the other direction: `open-issues.md`,
`docs/backlog.md`, the DCR archive banner rule and release-checklist section F
each prescribe rotation, and nothing runs any of them. `ti close` touches
TicGit only; the pre-commit hook runs no docs gate. Fourteen of nineteen
open-issue entries are resolved and still inline; no DCR has been archived since
DCR-0019.

So the fix is not "write less". It is to say what each document **is**, and give
each kind exactly one rule for how it changes.

## The four kinds

This text is the rule. It is reproduced verbatim in `CLAUDE.md` under
*Conventions*, and it replaces `reviews/README.md` rule 5.

> ## Document kinds
>
> Every tracked document is one of four kinds; the kind decides how it changes.
>
> - **Living** — `CLAUDE.md`, `README.md`, `docs/architecture/**`, `docs/*.md`,
>   `docs/implementation/**`, `docs/index.md`, `status.md`,
>   `release-checklist.md`, `phase-state.yaml`, and an ADR's decision body. Says
>   what is true now. Edit it in place, in the change's own commit; cite the
>   record as `(DCR-00NN)` and stop. No dates, no "since", no "used to", no
>   corrections, no appended notes, no struck-through items, no stacked
>   censuses.
> - **Register** — `open-issues.md`, `docs/backlog.md`, `CHANGELOG.md`'s
>   `[Unreleased]` and latest release. Holds open work. An entry moves verbatim
>   to its archive in the commit that closes it, once its resolution is verified
>   against code or a shipped artifact — never on the label alone.
> - **Record** — DCRs, released `CHANGELOG` sections, specs, archived issues.
>   **Write-once.** Its words never change and nothing is ever appended to it. A
>   later change is a new DCR; the reader finds it through `docs/index.md`.
> - **Archive** — `**/archive/`, `*-archive.md`. Verbatim, append-only, never
>   summarized back into a living document, never read for current truth.
>
> Text leaving a living document goes into an archive file or into a commit
> pushed to `origin` — never "into git history".

## Why records are write-once, and what that costs

Write-once is stronger than the rule it replaces. Rule 5 allowed a dated note to
be appended to a record; this does not.

The cost is real and is accepted deliberately: **nothing in a record can ever be
compacted.** Deleting an existing amendment block is a rewrite, so the 134,538
bytes of amendment blocks already inside the DCR family, and the 24 "Appended,
not a rewrite" preambles, stay exactly as written. Rotating a DCR into
`design-change-records/archive/` recovers no bytes at all; it only moves the
file. Write-once stops sixty-four amendment blocks becoming eighty. It does not
shrink them.

What it buys is that a record means one thing. A reader who opens DCR-0032 today
cannot tell which sentences were written in August and which were grafted on in
September without reading the dates; under write-once, a record is a photograph
of one moment and the successor is a separate file. That is also the property
the records themselves already claim — "keeps the words it was written with" —
so write-once is rule 5 taken literally rather than a departure from it.

## The one override

An **ADR's decision body is living**, and its amendment sections are not.

This contradicts the MADR template, which says to append an amendment section
with date and rationale. The override is deliberate and is confined to that one
sentence.

The reason is the authority hierarchy in `docs/project/design-baseline-2026-07.md`,
which this repository already follows: rank 3 is *rationale — ADRs*, and the
hierarchy's own rule is that "when documents conflict, the higher authority wins
and **the lower document must be fixed**". An ADR whose Decision Outcome is
false is a lower document that was never fixed. Today the Decision Outcome is
literally false in ADR-0003, ADR-0015 and ADR-0016, while eight `## Amendment`
sections below ADR-0006 quietly correct the seams list at the top that still
reads as current.

So: the decision body states the current rule; the amendment prose moves
verbatim to `docs/decisions/archive/adr-amendments.md`; a `## History` list
keeps one line per event, under 160 characters, so the record of *when* survives
without the essay. ADR ids never change, no ADR file moves, and `status:` is
untouched — it is the only machine-readable archive signal in the tree.

## What enforces it

`crates/transync/tests/docs_lifecycle_drift.rs`, a fifth member of the
docs-drift family, beside `docs_index_drift`, `docs_gate_claims_drift`,
`docs_ownership_drift` and `docs_browser_suite_drift`.

It is a **ratchet, not a cleanup**, and it ships green over the tree as it
stands:

- a living document **not** on `CONSTRUCTS_ALLOWED_TODAY` may not gain an
  amendment construct;
- a document **on** that list which has become clean must come off it, so the
  list only ever shrinks;
- a budgeted document may not exceed its byte budget, and a budget more than
  8 KiB above the real size is stale and must be lowered;
- `open-issues.md` may not hold more resolved entries than its high-water mark,
  and when it holds fewer, the mark comes down.

Sixteen files are on the allowlist today — `contracts.md` and fifteen ADRs — each
with the wave that takes it off. Twelve documents carry budgets.

It is a test and not a pre-commit check because the hook runs fmt, clippy, the
`wasm32` gate, the rustdoc gate and biome, and **no docs gate at all**. The real
docs gate is `cargo test --workspace -- --test-threads=2`.

## What this record does not decide

- **It does not decide ticket status.** Moving an entry to an archive closes
  nothing; a resolved label is not evidence of resolution, and an entry whose
  evidence is absent or conflicting stays in the live register with its
  uncertainty visible.
- **It does not reach the four self-declared dated snapshots** —
  `design-baseline-2026-07.md`, `design-baseline.md`, `stub-manifest.md`,
  `implementation-slice-checklists.md` — nor the three out-of-hierarchy
  historical narratives. Their staleness is their content, and
  `exit_code_docs_drift.rs` asserts two of them still carry their obsolete
  exit-code range, "because rewriting them would destroy the record they exist
  to keep".
- **It does not renumber anything.** `contracts.md` section numbers are cited
  253 times in `crates/`, 104 in `CHANGELOG.md` and 24 in `web/`; every heading
  stays byte-identical.
- **It does not edit the owner's skills.** `indy-review-gate`, `reopen` and
  `indy-review-cleanup` write the shape this record removes; the replacement
  wording is reported in the design spec for the owner to apply.
