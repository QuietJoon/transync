---
type: DCR
title: The docs bundle adopts OKF v0.2
description: "docs/ carried no conformance standard, so 39 of its 128 Markdown files — 38 concept documents and the root index — had no frontmatter, and nothing could say so. This record makes docs/ a conformant OKF v0.2 bundle — index.md becomes the reserved bundle root, log.md the change journal, project-index.md the curated human entry point — backfills frontmatter on the 38 concept documents, gives index.md only okf_version, and settles that a Record's write-once rule governs its words rather than its metadata header."
tags: [documentation, project-control, DCR-0055, DCR-0054]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-15T13:41:50Z
status: stable
---

# DCR-0055: The docs bundle adopts OKF v0.2

- **Date:** 2026-09-15.
- **Source:** a `/check-missing` audit, which found `docs/project-index.md`
  absent and `docs/index.md` carrying no `okf_version`; owner decision to
  adopt rather than to keep recording the gap.
- **Affected documents:** `docs/index.md` (living) gains `okf_version` and a
  rewritten header; `docs/log.md` and `docs/project-index.md` are new; 38
  concept documents gain frontmatter. The superseded statement is in
  `docs/project/phase-state-notes.md`, the archived run log; it stays verbatim,
  and this record supersedes it.
- **Behaviour changed:** none. No crate, wire format, CLI surface or contract
  moves. One test changes: `crates/transync/tests/docs_index_drift.rs`.

**Numbering note.** Next free number after DCR-0054.

## What was wrong

The phase-state run log (now `phase-state-notes.md`) recorded the position
plainly: *"only `manual/` declares `okf_version`; `docs/` has no conformance
standard to violate."* That was true, and it was the problem. `manual/` is a
conformant OKF v0.2 bundle with a root index, a journal and frontmatter on
every page. `docs/` — the larger and more load-bearing tree — had 39 of its
128 Markdown files with no frontmatter at all, 38 concept documents and the
root index, and no gate that could notice. "No standard to violate" is not a property
of a healthy tree; it is the absence of the thing that would have caught the
drift.

## The decision

`docs/` is a conformant **OKF v0.2** bundle. Three files carry reserved roles:

| File | Role | Frontmatter |
|---|---|---|
| `docs/index.md` | bundle-root index — machine-facing navigation of what exists | `okf_version: "0.2"` and nothing else |
| `docs/log.md` | bundle change journal, newest-first | none |
| `docs/project-index.md` | curated human entry point (`type: Documentation Index`) | normal concept frontmatter |

`index.md` and `project-index.md` are **not** the same document and neither
absorbs the other. The reserved index is regenerable and lists everything; the
curated index is hand-written, says what to read in what order, and may describe
artifacts that do not exist yet.

## Records keep their words; metadata is not words

DCR-0054 makes a Record write-once: *"Its words never change and nothing is ever
appended to it."* OKF requires a non-empty `type` on every concept document,
retired ones included. Read literally against each other, the two rules say a
retired record must both gain frontmatter and never be touched.

They do not actually collide. DCR-0054 governs a record's **words** — its
argument, its evidence, its conclusion — and forbids the appended correction and
the stacked amendment that motivated it. A frontmatter block is a metadata
header: it is *prepended*, not appended, it makes no claim about the subject
matter, and it changes no sentence. So the four archived DCRs, the nine design
specs and `open-issues-archive.md` receive frontmatter like any other concept
document.

What does **not** change: OKF never demands an index entry for a retired
record, and it accepts one that points into an archive. This bundle's profile
is stricter. `docs_index_drift.rs` requires every document under `docs/` to be
linked, archives included, so a retired record stays linked under the index's
*Archive* section, as the compaction spec's wave 1 set up. An archive keeps
its entries findable; it is never read for current truth.

## Provenance of the backfill

All 38 backfilled documents carry `generated.by: unknown/unknown`. This is the
format's migration sentinel and it is the honest value: these documents predate
OKF here, recorded no producer, and the migrator edited metadata rather than
authoring the words. Writing a model identifier there would claim authorship of
text the model did not write. `project-index.md` is new prose and names its
actual producer.

## The test that moved

`docs_index_drift.rs` required every `.md` under `docs/` to be linked from the
index, excluding, besides the files and bundles `.gitignore` ignores, only the
index itself. `log.md` is reserved and is not a
concept document, so it would have been demanded as an orphan. The exemption is
now a two-element reserved set (`OKF_RESERVED`) rather than a single
self-reference, and the companion test that asserts the exclusions are the only
exclusions moved with it. The exclusion is keyed on the reserved *names*, which
is sound because OKF forbids a concept document from taking either name.

## What this does not do

- It does not adopt OKF's reference agent or any SDK. The format only.
- It adds no validator to the test suite. `docs_index_drift.rs` still checks
  linkage and link resolution; nothing yet asserts that every concept document
  has a non-empty `type`. That gate is worth having and is not in this record.
