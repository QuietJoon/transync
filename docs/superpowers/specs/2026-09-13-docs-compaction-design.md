# Documentation Compaction — Design Spec

- **Date:** 2026-09-13
- **Status:** **owner-ratified design** (six decisions settled 2026-09-13). Implementation is **not
  yet authorized** — this is the document that gate reviews. No repository content outside this spec
  and its `docs/index.md` link has been changed.
- **Problem:** maintained documents grew by appending amendments rather than editing in place.
  Measured across 5,651,227 B of tracked English Markdown: ~13.7% amendment scaffolding, ~54.6%
  dead history.
- **Owner decisions (2026-09-13, settled — do not relitigate):** (1) records are **fully
  write-once** — nothing may be added to a dated record after it is written; (2) **ADR bodies
  consolidate in place**, an explicit override of the MADR template's "append an amendment section",
  justified by the authority hierarchy ranking ADR rationale as current truth that "must be fixed";
  (3) the **13 executed implementation plans are deleted** after relocation of unique content and
  after the deleting commit is proven on `origin/master`; (4) **the owner adjusts the conflicting
  skills** — this plan touches nothing under `~/.claude`; (5) this spec lives in
  `docs/superpowers/specs/`; (6) `origin/master` is the authoritative ref and `branch.master.merge`
  is repointed to it.
- **Amends:** `reviews/README.md` rule 5 (replacement text in §6); `release-checklist.md` steps 23
  and 20b (§5, wave 0). Both are authorized by DCR-0054, which wave 0 writes.
- **Method:** eight parallel document-family surveys plus a governance pass (authority order,
  archive policy, machine-enforced constraints, citation shapes, git durability), two independent
  design approaches, and a synthesis. Every headline figure in §1 was re-verified by hand against
  the working tree; figures that remain estimates are labelled as such.
- **Mode:** produced under the `compact-docs` skill in assessment/proposal mode, which changes no
  project files or tracker state.

## 0. What the decisions changed, against the first draft

### 0.1 Write-once makes DCRs a placement problem only

"Fully write-once" removes the last mechanism by which a DCR, a released `CHANGELOG` section, or an
archived open-issue entry could be edited. Those classes therefore **cannot be compacted by rewriting
at all** — not even to delete an existing amendment block, because deleting is a rewrite.

This is consistent with the `compact-docs` rule for the case *"Accepted source records must remain
intact"*: the treatment is **improve the existing derived current view**, not touch the records.

Consequences:

- The 134,538 B of existing DCR amendment blocks and the 24 "Appended, not a rewrite" preambles
  **stay exactly as written**. Wave 6 becomes pure rotation — moving whole files into the archive
  directory that already exists — and recovers **0 bytes** from the tree.
- Write-once is forward-looking: it stops the 64 blocks from becoming 80. It does not shrink them.
- The derived current view that replaces them is the one that already exists: `docs/index.md` plus
  the `contracts.md` normative text. Nothing new is invented to hold it.
- ADRs are the deliberate exception, by decision (2): the authority hierarchy ranks ADR rationale as
  **current truth** ("the lower document must be fixed"), so an ADR decision body is a *living*
  document that happens to live in a record-shaped file. The override is exactly this and no wider.

### 0.2 Reuse the existing archive convention and the existing gate family

The draft proposed a new `docs/archive/**` tree and a new `scripts/docs-lint.sh`. The skill forbids
introducing a documentation framework to shorten a document, and requires reusing an existing
effective view or generator. Verified in-tree:

- `docs/decisions/archive/` — exists.
- `docs/project/design-change-records/archive/` — exists.
- `docs/project/open-issues-archive.md` — exists.
- `crates/transync/tests/docs_{index,gate_claims,ownership,browser_suite}_drift.rs` — an existing
  four-test doc-gate family that runs inside `cargo test --workspace`.
- `docs/archive/` — **does not exist**, and will not be created.

So: destinations follow the per-family `archive` convention already in use, and enforcement is a
fifth member of the existing drift-test family (`docs_lifecycle_drift.rs`), not a new shell lint.
The pre-commit hook is left alone — it does not run the doc tests today, and the real gate is
`cargo test --workspace -- --test-threads=4`.

### 0.3 A label is not evidence of resolution

The skill is explicit: *"A closed label, passing build, missing symbol, newer date, or backlink alone
does not establish completion or supersession."*

The draft's wave 2 moved 14 `**Status:** RESOLVED` open-issue entries on the strength of the label.
That is now a per-entry verification step: for each entry, confirm the obligation is actually
discharged in code or in a shipped artifact, and name the clause checked. An entry whose evidence is
absent or conflicting **stays in the live register with its uncertainty visible**, per the skill's
*"Keep the uncertainty and remaining work visible; continue independent compaction."*

Storage treatment decides nothing about ticket status. Archiving an entry does not close its TicGit
ticket, and does not assert the work is done.

---

## 1. Findings (measured, re-verified by hand)

Corpus: **5,651,227 B** of tracked English Markdown (`git ls-files '*.md'` minus `*.ko.md` minus
`manual/**/ko/`).

Across the eight surveyed families: amendment scaffolding **~13.7%**, dead history **~54.6%**.
Those headline numbers are honest but not actionable on their own; the split is what matters.

| bucket | bytes | nature | recoverable? |
|---|---:|---|---|
| Rotation failure — registers whose own rules say the text should have left | ~582 K | mechanical | yes, from live reading |
| Amendment scaffolding in living documents | ~250 K | judgement | yes |
| Accreted records (DCRs) | ~330 K | placement | no — write-once |
| Spent scaffolding — 13 executed plans | 1,506 K | referenced only by the index that is forced to | yes, by deletion |
| Frozen by constraint — baselines, `stub-manifest`, slice checklists | ~116 K | protected, asserted by test | no |

Hand-verified figures:

| claim | verified value |
|---|---|
| executed plans, total | **1,505,956 B** (27% of tracked English Markdown) |
| `CHANGELOG.md` total | **577,645 B** |
| `CHANGELOG` `[0.1.0]`–`[0.4.0]` | **482,602 B** = 83.5%, all behind annotated tags |
| `CHANGELOG` `[0.5.0]` | 76,089 B — released, therefore write-once, therefore out of scope until 0.6.0 |
| `open-issues.md` | **19 entries**, 14 labelled RESOLVED |
| `contracts.md` §N citations in `crates/` | **253** |
| …in `CHANGELOG.md` | **104** |
| …in `web/` | **24** |
| `contracts.md` headings (`##`/`###`) | 18 — every one an anchor |
| `master` vs `origin/master` | **5 ahead** |
| `master` vs `origin/main` (the configured upstream) | **29 ahead** → `origin/main` is 24 behind `origin/master` |
| repository root commit | `59ce8df` (2026-08-17), 164 commits total |

### The twelve worst files

| file | bytes | scaffold / dead | mechanism |
|---|---:|---|---|
| `CHANGELOG.md` | 577,645 | 4% / 79% | per-commit prose journal: 163 `###` entries against 164 commits, mean 3,446 B |
| `docs/architecture/contracts.md` | 306,911 | 31% / 17% | each ticket appended a decision record *into* the normative clause: bold DCR headline, "It read, until that date:" recital (14 of them, 22,984 B), fuzz anecdote, rejected alternative |
| `docs/backlog.md` | 218,902 | 19% / 48% | 43 entries carry a RESOLVED banner over an unchanged present-tense body; 4 stacked censuses |
| `plans/…wave0-transync-html-crate` | 212,381 | 8% / 84% | frozen copies of shipped code |
| `plans/…wave6-panes-wire-cli` | 180,014 | 16% / 69% | same |
| `docs/project/open-issues-archive.md` | 136,774 | 25% / 71% | entries moved at full template width (~4.7 KB each) |
| `docs/project/open-issues.md` | 121,257 | 11% / 73% | own line-4 rotation rule not run since 2026-08-23 |
| `docs/project/status.md` | 120,762 | 8% / 80% | two append-only ledgers inside a status page; 17 of 20 "immediate" actions struck through |
| `docs/project/phase-state.yaml` | 88,287 | 7% / 83% | one `project.notes:` scalar = 94% of the file, which `validate-project-state.py` never reads |
| `docs/decisions/0006-renderer-output-shape.md` | 25,491 | 39% / 8% | eight `## Amendment` sections (17,848 B = 70%), each opening "*Appended, not a rewrite*", all correcting the same stale seams list |
| `CLAUDE.md` | 29,032 | 7% / 28% | loaded every agent turn; one 10,082 B line; a 6,900 B dated divergence chain |
| `docs/index.md` | 40,835 | 27% / 19% | 20,610 B of link annotations restating each target's own title |

---

## 2. Root causes

1. **"Append a dated note, never rewrite" was applied to living documents.** Written for records
   (`reviews/README.md` rule 5, the MADR template, release-checklist step 23). Step 23's other half —
   living docs are edited in place — never got a mechanism, so the appending half won everywhere.
2. **Rotation rules exist with no trigger.** `open-issues.md` line 4, the DCR archive banner rule,
   `backlog.md`'s own verification convention. `ti close` touches TicGit only; the hook runs no doc
   gate; the checklist has no sweep step. Result: 14 of 19 OI entries RESOLVED in the live file,
   0 DCRs archived since DCR-0019, 17 of 20 "immediate" actions done and struck.
3. **Narrating the change instead of stating the result**, with no size or shape budget anywhere.
4. **Bookkeeping about the register inside the register** — and release-checklist **step 20b
   mandates it**: "a new dated census line beneath the previous one — never by editing the previous
   one".
5. **Deletion felt like destruction, and there was nowhere else to put text.** Three git-loss events
   in ten days made "it's in history" false, and the only in-tree destination was
   `open-issues-archive.md`.

---

## 3. Constraints the plan obeys

Machine-enforced:

- **Never renumber `contracts.md` sections.** 253 citations in `crates/`, 104 in `CHANGELOG.md`,
  24 in `web/`. Headings are anchors; all 18 stay byte-identical.
- `docs_index_drift.rs` fails on any orphan or dead link under `docs/`. Every file created, moved or
  deleted updates `docs/index.md` in the same commit.
- `docs_gate_claims_drift.rs` is **proximity-based**: every `--no-verify` must share a paragraph with
  "never"; every `CI` token must share a segment with "no CI". Compaction can break co-location
  without changing a claim. Seven files are guarded.
- `docs_cli_flags_drift.rs` and `exit_code_docs_drift.rs` pin the literal headings
  `## 6. CLI argument contract`, `## CLI reference`, `### Exit codes`, and parse the first fenced
  block under two of them.
- `docs_ownership_drift.rs` asserts three retired phrases are **absent** — they may not reappear even
  inside a quoted historical note.
- `docs_browser_suite_drift.rs` requires the five browser spec filenames to stay named.
- 261 `manual/` frontmatter `resource:` paths resolve into `docs/`, including **5 DCRs**
  (0020, 0025, 0026, 0027, 0029) and **7 ADRs** (0001, 0002, 0009, 0017, 0018, 0019, archive-0014).
  Moving any of them breaks Korean pages this assistant may not edit.

Protected content, not bloat:

- The four self-declared dated snapshots: `design-baseline-2026-07.md`, `design-baseline.md`,
  `stub-manifest.md`, `implementation-slice-checklists.md`. Two are additionally asserted by
  `exit_code_docs_drift.rs` to still carry their obsolete exit-code range, with the reason recorded
  in the test: *"rewriting them would destroy the record they exist to keep."*
- The three out-of-hierarchy historical narratives: `docs/superpowers/specs/2026-05-01-transync-design.md`,
  `references/draft.md`, `docs/project/design-baseline.md`.

Durability:

- **Git is not a backup here.** Root commit `59ce8df` (2026-08-17); three loss events in ten days;
  the cause was never removed; both salvage archives are damaged and on the same synced volume.
  Every removed byte is permanently destroyed unless it lands in a tracked archive file **or** in a
  commit pushed to `origin/master` before the next operation.
- No `git gcx` / `gc` / `prune` at any point during the campaign — prune deletes the unreachable pool
  that `fsck --lost-found` recovers from.
- `git fsck --no-progress --connectivity-only` before and after every wave.

---

## 4. Representative questions, fixed before any rewrite

Per the skill, these are the acceptance test. They are fixed now so they cannot be adjusted to fit
the result.

1. *"What exactly does `--html-out` write, and what is guaranteed atomic?"* — answerable from
   ADR-0006's decision body alone, including the exception that `transync serve` is real, without
   reading a single amendment.
2. *"May `transync-syntax` gain a `[features]` table?"* — answerable from `CLAUDE.md` in one place,
   with the reason (`getrandom` via tokio/tiktoken breaks the wasm32 link), in under 10 lines.
3. *"Which open issues are unfinished right now, and what is blocking each?"* — answerable from
   `open-issues.md` without skipping past resolved entries, with each entry's evidence visible.
4. *"Why is the adoption agency algorithm absent, and what is the residual?"* — answerable from
   `contracts.md` §4b, with the ticket id, without reading the nineteen-divergence chain.
5. *"What did `contracts.md` §4a say before DCR-0041 changed it?"* — answerable by topic or by
   original DCR id from the history sibling, and distinguishable at a glance from current rules.
6. *"Is OI-0027 resolved?"* — answerable with actual evidence, not a label.

---

## 5. The waves

Standing conventions for every wave: claim a ticket via `ti-pick-next`;
`git fsck --no-progress --connectivity-only` before and after; the gate is

```
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/transync/gate/<wave>.txt 2>&1
cargo_rc=$?
printf 'CARGO_EXIT=%s\n' "$cargo_rc" >> /Volumes/Temp/claude/transync/gate/<wave>.txt
exit "$cargo_rc"
```

run in the foreground, never piped; `docs/index.md` updated in the same commit as any file
created/moved/deleted; push to `origin master` after every commit; Korean siblings regenerated only
through the `doc-translation` skill's `doc-translate` binary, never by hand.

Classes: **(i)** mechanical, assertion-neutral · **(ii)** judgement · **(iii)** needs a DCR first.

### Wave 0 — safety and governance · class (iii) · 0 bytes recovered (adds ~8 KB)

| action | detail |
|---|---|
| Settle the remote | Push the 5 commits to `origin/master`. Confirm `origin/master` is authoritative and decide whether `origin/main` is fast-forwarded or retired; `branch.master.merge` currently points at the stale one. |
| Working tree | Commit or stash `docs/backlog.md` and the two `manual/**/ko/` files; decide the fate of untracked `reviews/.claims/` and `reviews/.gating/`. |
| `DCR-0054` | Records the four document kinds, the write-once rule for records, the decision-(2) ADR override, the destinations, and the rotation triggers. This is the DCR that authorizes waves 4–7. |
| `crates/transync/tests/docs_lifecycle_drift.rs` | Fifth member of the existing drift family. Fails on a new amendment construct in a living document, on a RESOLVED entry left in a live register, and on a living document exceeding its recorded byte budget. Seeded green with a shrink-only allowlist and a downward-only budget file. |
| `release-checklist.md` | Step 23 rewritten to carry the "edit living docs in place" half; step 20b changed from "never by editing the previous one" to "the census replaces the previous census"; a sweep sub-step added to section F. |
| `reviews/README.md` rule 5 | Replaced with the write-once text (§6 below). Note: this file is **untracked on this machine** yet holds the rule that disambiguates 1,720 `R000N-####` citations — it must be committed before anything depends on it. |

### Wave 1 — agent-facing · class (i)+(ii) · CLAUDE.md 29,032 → ≤14,000; index.md 40,835 → ~22,000

`CLAUDE.md`: delete the 6,900 B dated divergence chain — but only after checking, clause by clause,
that each divergence is stated in the DCR it names. Note DCR-0032 never states the total "nineteen";
that total exists only in `CLAUDE.md`, so it is written once into `contracts.md` §4b before the chain
goes. Three standing rules are buried in the chain and must survive verbatim in the rewritten line:
the fix-not-regression rule, `noscript` deliberately not raw text, and the adoption-agency absence
with its 8/10,000 residual and ticket `525bef`. Also preserved: the `[features]`/`transync-core`
prohibition and the "the gate's command string must not be changed to name `transync-html`" rule.

`docs/index.md`: annotations capped at 100 characters; `*(archived — superseded by …)*` suffixes
shortened to `*(archived)*`; an `## Archive` section added so archived files remain linked (the
drift test requires every non-excluded doc to be linked).

### Wave 2 — registers · class (i), with per-entry verification · ≈ −365 K live

| source | destination | rule |
|---|---|---|
| `open-issues.md` 14 RESOLVED entries | `open-issues-archive.md` (exists) | verbatim, with an `> Archived DATE. Reason:` banner. **Each entry's resolution verified against code or a shipped artifact first**, naming the clause checked. Unverifiable → stays live with its uncertainty visible. |
| `status.md` wave-log chain (34,684 B) + Phase 1–5 tables (8,719 B) + struck actions (~43 K) | `docs/project/status-archive.md` (new, matching the `open-issues-archive.md` naming already in use) | verbatim |
| `phase-state.yaml` `project.notes:` scalar (82,873 B) | `docs/project/phase-state-notes.md` (new, same family) | verbatim inside a fence; ≤20 lines of current notes remain in the YAML |
| `backlog.md` 43 resolved entries | collapse in place to heading + banner + `### Verification` + `### Notes`, both verbatim | `/reopen` owns `### Notes` |
| `backlog.md` 4 stacked censuses | 1 | step 20b must be changed in wave 0 first |
| `release-checklist.md` 7 "is spent" paragraphs (6,283 B) | deleted whole-paragraph | whole paragraphs only — the proximity gate guards this file |

Census re-derived in the same commit, per constraint: `ti list --open`, plus OI OPEN and DEFERRED
counts. `docs/backlog.md` and `open-issues.md` are the v0.5.0 release-gate input (checklist step 2a).

**Resurrection risk.** `/indy-review-cleanup` re-creates OI entries from `reviews/reviewed/`, and
`/reopen` re-adds backlog entries whose source still holds them. After wave 2 the owner dry-runs
both; `git diff --stat` must show zero re-adds. This is the concrete form of decision (4).

### Wave 3 — CHANGELOG and reader docs · class (i) · 577,645 → ~95,700

Move `[0.1.0]`–`[0.4.0]` bodies (482,602 B) verbatim to `CHANGELOG-v0.1.0-v0.4.0.md` at the repo
root. **Keep each `## [x.y.z]` heading in `CHANGELOG.md`** with a one-line pointer, so the
heading↔link-stanza invariant and the `[Unreleased]` check still hold. The 104 `contracts.md §N`
citations travel with the text and remain resolvable.

`[0.5.0]` (76,089 B) is a released section and therefore write-once — untouched until 0.6.0 rotates
it. The per-entry length rule is forward-only.

Reader docs: `Developer_Guide.md` v0.2/v0.4 migration notes (4,299 B) → the archive file under their
matching release; `Troubleshooting.md`'s three-loss narrative → two pointers at the incident records
that already hold it; `module-map.md` "used to" tree comments (~3 K) deleted, `(DCR-00NN)` markers
kept. All three are proximity-gated files: whole-paragraph edits only, and no paragraph containing
`--no-verify` or a `CI` token is reflowed.

### Wave 4 — `contracts.md` · class (ii), under DCR-0054 · 306,911 → ~160,000

Mechanical floor first: the 14 "It read, until that date" recitals (22,984 B) move verbatim to
`docs/architecture/contracts-history.md`, each under a heading naming its section and its DCR, so
question 5 above is answerable by topic or by original id. Provenance leads shorten to `(DCR-00NN)`
(23,915 B).

Then the judgement layer (~147 K): incident stories, dates, "deliberately/on purpose" restated beyond
one clause, rejected-alternative litigation. Fuzz and browser-oracle measurements are **relocated,
not deleted** — they are the evidence behind normative claims.

Kept absolutely: every heading, every normative sentence, every id, every `Pinned by <test>` weld.

**Verification is a claim inventory, not a diff.** Before each section, extract every bold-lead and
every sentence containing `must|never|always|is contractual|is rejected` to a file. After, each one
must be present verbatim (whitespace-normalized) either in the head or in the history sibling. One
commit per section.

### Wave 5 — ADRs · class (iii), authorized by decision (2) · ≈ −59 K

Eight ADRs (0006, 0002, 0009, 0013, 0003, 0012, 0016, 0015). The decision body is rewritten to state
the current rule; the amendment sections (78,846 B) move verbatim to
`docs/decisions/archive/adr-amendments.md`; a `## History` list keeps one ≤160-character line per
event. Six stale `description:` frontmatter fields are corrected. `status:` is untouched — it is the
only machine-readable archive signal in the tree.

ADR ids do not change, so the 517 `ADR-00NN` citations outside `docs/` and the 7 manual `resource:`
paths are unaffected. No ADR file moves.

### Wave 6 — DCR rotation · class (i), placement only · 0 bytes from the tree

25–31 of the 41 active DCRs move to `design-change-records/archive/` (which exists) with an
`> Archived DATE. Reason:` banner and `status: deprecated`. Bodies are **not touched** — write-once.
The active set becomes roughly 0040–0048, 0050, 0051, 0053, plus any carrying an undischarged
follow-up.

**DCR-0020, 0025, 0026, 0027 and 0029 do not move** until the owner re-runs `write-diataxis-manual`,
because `manual/` `resource:` paths point at them on both English and Korean pages.

Where the owner is willing, this wave runs *through* `/indy-review-prune` rather than by hand, to
satisfy its Phase 0a gate.

### Wave 7 — executed plans · class (iii), authorized by decision (3) · −1,505,956 B

Order matters:

1. Relocate wave0's ~2.4 KB correction to four unrewritable commit messages — its only unique
   content, and the sole home of that correction — to `docs/project/git-history-loss-2026-08-17.md`,
   which is already the record for exactly this class of fact.
2. Verify no other plan holds content that exists nowhere else (grep each plan's unique assertions
   against its DCR and the CHANGELOG).
3. Delete the 13 files; update `docs/index.md` in the same commit.
4. Push, then confirm with `git branch -r --contains <sha>` that the deleting commit is on
   `origin/master` **before the next wave begins**. This is the only durable record of the deleted
   bytes, and this repository has destroyed history twice.

`docs/superpowers/specs/` is untouched — it holds live design content, including the two specs cited
from Rust source.

---

## 6. The steady-state rule

Goes into `CLAUDE.md` under Conventions, verbatim into DCR-0054, and replaces `reviews/README.md`
rule 5. Enforced by `crates/transync/tests/docs_lifecycle_drift.rs`, so it runs inside
`cargo test --workspace` and `scripts/smoke.sh`.

> ## Document kinds
>
> Every tracked document is one of four kinds; the kind decides how it changes.
>
> - **Living** — `CLAUDE.md`, `README.md`, `docs/architecture/**`, `docs/*.md`,
>   `docs/implementation/**`, `docs/index.md`, `status.md`, `release-checklist.md`,
>   `phase-state.yaml`, and an ADR's decision body. Says what is true now. Edit it in place, in the
>   change's own commit; cite the record as `(DCR-00NN)` and stop. No dates, no "since", no "used
>   to", no corrections, no appended notes, no struck-through items, no stacked censuses.
> - **Register** — `open-issues.md`, `docs/backlog.md`, `CHANGELOG.md`'s `[Unreleased]` and latest
>   release. Holds open work. An entry moves verbatim to its archive in the commit that closes it,
>   once its resolution is verified against code or a shipped artifact — never on the label alone.
> - **Record** — DCRs, released `CHANGELOG` sections, specs, archived issues. **Write-once.** Its
>   words never change and nothing is ever appended to it. A later change is a new DCR; the reader
>   finds it through `docs/index.md`.
> - **Archive** — `**/archive/`, `*-archive.md`. Verbatim, append-only, never summarized back into a
>   living document, never read for current truth.
>
> Text leaving a living document goes into an archive file or into a commit pushed to `origin` —
> never "into git history".

### Replacement wording for the owner's skills (decision 4)

Reported, not applied — nothing under `~/.claude` is edited.

| skill | line to change | replacement |
|---|---|---|
| `indy-review-gate` | creates OI + backlog entries at full template width | cap a new entry at ~1.5 KB: statement, evidence, obligation. Detail lives in the review round. |
| `reopen` | "keeps resolved entries for audit"; stacks a dated census | move a resolved entry to the archive in the sweep that resolves it; the census **replaces** the previous census |
| `indy-review-cleanup` | re-creates OI entries from `reviews/reviewed/` | skip an id already present in `open-issues-archive.md` |
| MADR template | "Append an amendment section with date and rationale" | "Edit the decision body to state the current rule; add one ≤160-character line to `## History`" |

---

## 7. Totals, honestly stated

| measure | value |
|---|---|
| live documents shrink by | ≈ 1.05–1.2 MB (−55–60% of the touched heads) |
| tree shrinks by | ≈ 1.8 MB, almost all of it wave 7 |
| relocated, not removed | ≈ 1.0 MB into tracked archive files |
| recovered from records | **0** — write-once |

Archiving relocates; it does not shrink the repository. The only true deletions are wave 7's plans,
wave 2's struck action lines, and wave 4's judgement layer.

## 8. What this does not fix

- **Archived record bodies.** DCR-0032's four amendment blocks, every archived DCR's `Part A–F`
  narration, and the 204 commit hashes that do not resolve in this object store stay as written.
  Write-once stops growth; it does not shrink history.
- **`CHANGELOG [0.5.0]`** (76 K of essays) until 0.6.0 rotates it.
- **The four frozen snapshots** (~116 K) — by constraint and by test.
- **`reviews/reviewed/0009–0011`** (179,693 B) — `/indy-review-cleanup`'s job, and blocked until
  `reviews/README.md` carries a per-finding index for those rounds (160 ids, 952 citation sites).
- **`manual/` prose drift** beyond path existence — only `write-diataxis-manual` regeneration fixes
  it, and that is also what unblocks moving the 5 manual-cited DCRs.
- **The baseline's stale enumerations** (ADR 0001..0016, DCR 0005..0011, SCN-01..14) — protected
  text. `docs/architecture/README.md` and `settled-questions.md` restate the ranks correctly and are
  the living pointers readers should use.
- **Prose that is long because it is true** — `contracts.md`'s legitimately long normative
  paragraphs do not become history by being long.

## 9. Costs not yet measured

- **Korean retranslation.** Translation state is keyed by per-block sha256, so every compacted block
  becomes a retranslation job. `doc-translate --prepare-changes` must be run for a count before
  wave 1, and `.translate/registry.yaml`'s `exclude:` key should gain the archive paths.
- **Wave 4's judgement layer** is the only wave whose reduction is an estimate rather than a
  measurement, and the only one where a silent meaning change is possible. It is also the wave with
  no test coverage over most of its content — the claim inventory is the entire safety net.
