# Documentation Log

Newest-first journal of changes to the `docs/` bundle itself. Changes to what
the documents *say* belong in the documents; this file records the shape of the
bundle — files added, removed, retired, or re-typed.

## 2026-10-10

* **DCR-0059 and DCR-0060 added** under `project/design-change-records/`: the
  OpenAI model-generation floor and Vertex AI express mode. The bundle index
  links both.

## 2026-10-06

* **Five archive files added**, each holding text that the documentation
  compaction (DCR-0054) moved verbatim out of a living document:
  `decisions/archive/adr-amendments.md` (the ADR amendment sections),
  `architecture/archive/contracts-history.md` (the `contracts.md` recitals and
  narratives), `project/release-checklist-archive.md`,
  `Developer_Guide-archive.md` and `Troubleshooting-archive.md`. The bundle
  index links each. `backlog-archive.md` received the backlog entries whose
  resolution was verified.

## 2026-09-23

* **DCR-0058 added** under `project/design-change-records/` for the Gemini
  Developer and Vertex adapter. The bundle index links it.

## 2026-09-16

* **`backlog-archive.md` added.** `backlog.md` is a Register, and the kind rule
  sends a closed entry verbatim to an archive in the commit that closes it —
  but the backlog had no archive to send one to, so entries had been
  accumulating past their resolution. The first move is
  `section-batch-coalescing`, retired by DCR-0057.
* **DCR-0057 added** under `project/design-change-records/`.

## 2026-09-15

* **Bundle adopted.** `docs/` became a conformant OKF v0.2 bundle (DCR-0055).
  `index.md` gained `okf_version: "0.2"` and is now the reserved bundle-root
  index; this journal was created; and `project-index.md` was added as the
  curated human entry point, which the reserved index must never absorb.
* **Frontmatter backfilled** on the 38 concept documents that carried none —
  the five top-level reader docs, seven under `architecture/`, `backlog.md`,
  `implementation/module-map.md`, eleven under `project/`, four archived DCRs
  and the nine design specs. `generated.by` is the migration sentinel
  `unknown/unknown` on all 38: pre-OKF documents recorded no producer, and the
  migrator is an editor rather than the author of the words.
* **Records and archives received frontmatter too.** The bundle's own kind rule
  makes a Record's *words* write-once; a metadata header is neither a rewrite of
  those words nor an appended note, and OKF holds a retired record to the same
  `type` requirement as a live one. They also stay linked from the index's
  *Archive* section, which this bundle's profile requires
  (`docs_index_drift.rs`) though OKF does not.
* **DCR-0056 added** under `project/design-change-records/`. The bundle index
  links it.
