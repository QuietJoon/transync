# Manual Log

## 2026-09-07

* **Out-of-band edits, not a Sync.** The bundle was changed twice by ordinary
  repository work rather than by a run of the manual skill, and this entry
  exists so the log does not silently skip them. No page was regenerated, no
  page was added or removed, and the tally is unchanged at 17 (1 tutorial,
  8 how-to, 4 reference, 4 explanation).
  * 2026-09-06 (`c5de190`): three stale claims corrected in place, found by the
    review-0010/0011 decision gate.
  * 2026-09-07 (`18c1ab4`, DCR-0053): three frontmatter source paths followed
    the `parser` → `intake::markdown` rename in `transync-syntax`. Paths only —
    `crates/transync/tests/manual_source_refs.rs` fails on a source reference
    that does not resolve, so they had to move in the same commit as the
    module.
* **Known asymmetry, not fixed here:** `how-to/user/en/index.md` is the only
  quadrant sub-index in the bundle, is not listed in `index.md`, and duplicates
  four entries the manifest already carries — so a fifth user how-to would land
  in the manifest and silently not in it. Whether to complete the pattern, drop
  the file, or leave it is a bundle-structure decision for the manual pipeline's
  owner, not for a drift sweep.

## 2026-08-13
* **Sync**: All 12 existing pages were stale — the codebase had advanced far
  past the bundle, and four pages were wrong at the premise rather than
  merely behind. `transync serve` had become a real loopback HTTP server, so
  the operator how-to built on "serve is a deferred stub" was rewritten from
  the title down; the `Translator` reference and the custom-provider how-to
  both printed method signatures missing the cancellation parameter, so their
  code samples would not compile; the profile schema still called
  `scope = "section"` rejected and unimplemented after it shipped, and never
  mentioned `[[glossary]].sections` at all. Regenerated all 12, and added 5
  new pages for capabilities no page covered: the disk-backed cache (how-to),
  run cancellation (how-to), the alignment-map schema (reference), glossary
  resolution and section-boundary batching (explanation). 17 pages total
  (1 tutorial, 8 how-to, 4 reference, 4 explanation) across
  user/operator/developer.
* `how-to/user/en/diagnose-a-translation-run.md` was **conflicted** — someone
  had hand-added the exit-6/7 provider-taxonomy rows without updating
  `synced_hash`. Resolved by owner decision: replace with a full rewrite.
* Drift was established by reading sources directly, not from git: the
  repository's history was reset to a single initial commit during this run,
  so per-source commit comparison was unavailable and no commit hash is cited
  anywhere in this bundle.
* Korean `ko/` mirrors exist for all 17 pages, written by a translation
  pipeline, not by this skill. 16 track their `en/` sibling; the mirror of
  `diagnose-a-translation-run.md` is stale because its `en/` source was
  rewritten after the pipeline ran.
* Findings: 22 filed as verification tickets (3 marked blocked pending an
  owner decision), 1 declined-but-recurring.

## 2026-08-08
* **Sync**: `manual/` existed with 4 How-To pages from an interrupted
  bootstrap (no `index.md`, `log.md`, or any Tutorial/Reference/Explanation
  page). Regenerated all 4 stale pages against current sources — including
  fixing a never-computed `synced_hash: PLACEHOLDER_HASH` bug on
  `write-a-translation-profile.md` and an inaccurate hardcoded
  `schema_version=1.0.0` (should be `1.2.0`) on `serve-the-demo-bundle.md` —
  and added 8 new pages closing every dangling link plus the missing
  quadrants: 1 tutorial, 3 more how-to guides, 3 reference pages, 2
  explanation pages. Created `index.md` (first time), `log.md` (this file,
  first time), and a sub-index at `how-to/user/en/index.md` (3 concept
  docs). 12 pages total (1 tutorial, 6 how-to, 3 reference, 2 explanation)
  across user/operator/developer. Findings: 2 (tickets ticgit:12f945bc,
  ticgit:e04a84cd).
