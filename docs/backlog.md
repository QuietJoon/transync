---
type: "Reference"
title: "Backlog"
description: "Persistent register of WIP, deferred and open items found by the /reopen sweeps, grouped ready / needs-decision / blocked, and the census behind the v0.5.0 release gate"
tags: ["project", "process", "docs"]
generated:
  by: unknown/unknown
  at: 2026-09-15T13:39:50Z
---
# Backlog — WIP / deferred / open items

Maintained by the `/reopen` sweep. Last `/reopen` sweep: **2026-09-12** (full sweep — six new entries, six tickets filed; the previous full sweep was 2026-08-26); last in-place update: **2026-10-13** (the live-endpoint gate's results: one entry resolved and archived, one filed, and the census re-counted; earlier in-place passes 2026-09-05 `e5512b5`, 2026-09-06 `22f4efe`, 2026-09-07 `086ca35`, 2026-09-08 the documentation drift audit, 2026-09-12, 2026-10-10).
The 2026-08-07 refresh was derived from
`ti list --all` and `git log`, not from this file's own previous text — the review-0001
arc produced several tickets whose root cause was one document restating another, and the
index must not become one. Every sweep since has held to that rule.
Sources: `docs/project/`, `docs/decisions/`, `docs/architecture/`, DCRs, specs, code
markers, `docs/investigation/` (untracked and git-ignored since 2026-08-08 — present on
disk and read as a source, but not version-controlled) and `reviews/` (partly tracked:
round 0001 and `README.md` are committed, later rounds are left untracked until the gate
archives them; see `untracked-analysis-bundles` in the Type 2 removal note kept in
`docs/backlog-archive.md`).

**A second gate exists as of 2026-10-13, and this file is not yet the census for it.**
The owner stated v0.6.0's window-closing condition that day: *the manifest stays
`0.6.0-dev` for now; it goes to `0.6.0` once what is to be implemented is **complete and
verified**.* It is recorded where every such condition is checked — `release-checklist.md`
step 2a — and it adds a term v0.5.0's did not have: **verified**, not merely resolved.
What it does **not** yet say is which set "what is to be implemented" names: the three
items queued against the v0.6.0 window (OI-0051, OI-0052, OI-0054), or the whole
non-deferred union below, the way v0.5.0's condition did. **That is exactly the question
this file's census answers, so until it is settled the counts below carry no gate verdict
for v0.6.0** — they are a count of open work, which is what they were before any gate
existed. The v0.5.0 rule is kept verbatim underneath because it is the worked example of
how such a condition is read, and because its third bullet — the window is not the gate —
governs v0.6.0 unchanged.

**The v0.5.0 release gate — the rule this file was the census for (owner, stated
2026-08-24).** **`transync` bumps to 0.5.0 proper only after every registered task and
every open issue is resolved, excluding those explicitly deferred.** Three parts of that
sentence are load-bearing and none of them is optional:

- **"Registered" is the union of two registers, not either alone** — the TicGit queue
  (`ti list --all`) *and* the open-issue register (`docs/project/open-issues.md`, indexed
  here). An item closed in one and open in the other still counts against the gate.
- **"Explicitly deferred" means a *recorded* deferral with a named, falsifiable
  trigger** — not merely blocked, and not merely absent from anybody's queue. An entry
  whose blocking condition is written down is excluded, and **re-counts the moment that
  condition fires**. A substring search for the word `owner-deferred` is not a substitute
  for reading the entry; the 2026-08-25 audit reversed twelve findings that had been
  reported on exactly that basis.
- **The window is not the gate.** `0.5.0-dev` **was** the open breaking window (`ff788f7`,
  2026-08-24); this condition is what closed it. Nothing is released from an open window,
  and nothing was: the condition was met on 2026-09-05 and **v0.5.0 released the same day**
  (annotated tag `v0.5.0`), which closes the window. A further breaking change needs a new
  window and the owner decision that opens one. The rule stated here is unchanged and
  governs the next such number; only its subject has moved.

**Why the rule lives here.** The 0.4.0 window was declared used-and-closed while six
wave-0/1 finding tickets sat unindexed — including `ticgit:e77173bb`, a *live*
`contracts.md` §4a break — and the 2026-08-24 sweep then measured that failure properly:
**18 of the 24 open tickets were missing** from the very file `docs/project/status.md`
calls "the index of open items". A gate phrased over "every open issue" is worth exactly
as much as the census that answers it, which is why every entry below has to be in one of
three states — counting, deferred-with-a-recorded-condition, or resolved/historical — and
why an entry that is in none of them is a defect in this file, not an ambiguity.

**Census (2026-10-13b) — a count, not a sweep, and this is the current one.** Re-derived from
`ti list --open --json` and the section headings of this file; `docs/project/open-issues.md` was
not re-scanned and its figures below carry over from the 2026-10-10 count. No other source was
re-scanned either, so the last full sweep is still the 2026-09-12 one. **43 entries; 40 are live**
— each carrying at least one open TicGit ticket. By section: Type 1 holds 1 entry and it is live,
Type 2 holds 19 of which **17 are live**, Type 3 holds 23 of which **22 are live**. The same three
entries carry no open ticket: `anthropic-leg-outside-the-release-gate-trigger` and
`oversize-tombstones-point-at-a-closed-oi-0008` cite none, and
`phrasing-extension-list-for-custom-elements` cites only a closed one. **The two registers still
do not agree in both directions.** TicGit holds **72 open tickets** (18 new, 54 blocked, none in
progress). Forty are cited by an entry and **32 have no entry here**; those 32 were all filed
after the 2026-09-12 sweep, and the next sweep adopts them. Until it runs, the queue is
`ti list --open`, not this file. The open-issue register holds nine issues: **4 OPEN** (OI-0051
through OI-0054), **1 DEFERRED** (OI-0049) and four marked RESOLVED (OI-0040, OI-0041, OI-0046,
OI-0048) that stay in the register because each still has an open ticket against it. **What counts
against a gate is still the owner's to state**, and on 2026-10-13 a v0.6.0 condition was
stated — complete *and verified* — but its scope is not pinned, so **none of these figures is
a gate verdict**; the header says why. The **v0.6.0 breaking window is open** the same day
(DCR-0062; ticket `2d616b6a` closed), which is a separate thing: the window says what may ride
the release, the condition says whether it may happen.

**What moved since 2026-10-10, and it is all the live-endpoint gate.** The entry count is
unchanged because one entry left and one arrived. `anthropic-live-leg-never-run` is **RESOLVED**
and moved verbatim to `docs/backlog-archive.md`: the leg ran on 2026-10-13 and passed, against
`claude-haiku-4-5`, closing ti `40b29a9d` and a standing gap open since 2026-08-10. In its place,
Type 2 gains `anthropic-adapter-cannot-name-a-workspace` (ti `3e013e47`) — the limit that run
exposed and that its fix did not remove: the adapter sends no `anthropic-workspace-id`, so it
needs a workspace-scoped key. The second closure has no entry here and so moves only the
ticket count: ti `c5b1431a` asked whether Vertex express accepts the `x-goog-api-key` header
form, and the `google-vertex-express` leg answered yes. `v050-live-gate-unrun` is **deferred by the owner**, with a
trigger recorded on the entry: its remaining claim is narrower than its title — the run attests
the branch, not the released `v0.5.0` tree — and nothing depends on v0.5.0 carrying evidence of
its own.

**And one more moved the same day, for a different reason.** `unopened-breaking-window` is
**RESOLVED** and archived: the owner made the next version v0.6, which opens the window the entry
existed for (DCR-0062, ti `2d616b6a`). Nothing replaces it, which is why the entry total drops by
one here while the live-gate pair above left it unchanged.

Re-derive these figures on each sweep rather than trusting them — an entry counts unless it
carries a resolution or a dated deferral line. The release-side half of this rule is
`docs/project/release-checklist.md` step 2a, which is where a release actually consults
the census. Each census replaces the previous one (release-checklist step 20b): the superseded
censuses of 2026-08-26, 2026-09-05 (the v0.5.0 cut), 2026-09-08, 2026-09-12 and 2026-10-10, and
the TicGit snapshots of 2026-08-08 and 2026-08-24, are kept verbatim in
`docs/backlog-archive.md`. An entry moves there too, under its own heading, once its resolution is
verified against code or a shipped artifact.

The two tickets still tagged `deferred-v050` carry their re-triggers on the tickets themselves;
the v0.5.0-cut census stated them as ti `525bef` (no active-formatting-element list and no
adoption agency — residual measured at `sec4a:break-nested` 8 per 10,000 generated inputs and
`reserved:dom-only` **0**; re-triggers on the census rising above 8 on an unchanged `ATOMS`
alphabet, on any `reserved:dom-only` case at all, or on a route in this family being found by
hand rather than by the oracle) and ti `16721e` (`noscript` read as markup; re-triggers on the
oracle measuring a §4a break or anchor loss on a `noscript` route). OI-0049 (Review 0010's
sixteen non-blocking survivors) is likewise explicitly deferred; its three-way re-trigger is
recorded on the entry in `docs/project/open-issues.md`.

**Note (2026-08-17):** the `git log` half of that source set no longer exists. This
repository's object store was restarted for the second time on 2026-08-17
(`docs/project/git-history-loss-2026-08-17.md`; the first was
`docs/project/git-history-loss-2026-08-10.md`), so `git log` began again at the
2026-08-17 root `59ce8df`, and **every commit hash quoted below that predates the restart
names a commit outside this store** — that was every hash when this note was written, and
stopped being every hash as later in-place refreshes quoted post-restart commits
(`e89b537`, `88964df`, `59ce8df` itself), which resolve normally. Read it as the rule for
pre-restart hashes rather than as a count. Pre-restart entries
are dated records and stand as written — read the prose beside each hash rather than
trying to resolve it. The next sweep's sources are `ti list --all`, `docs/`, and the
code markers; a `git log` walk rejoins them only once this history has commits to walk.
Every item currently found lives here, keyed by topic; the authoritative
detail for OI-numbered items stays in `docs/project/open-issues.md` — this file is the
cross-source index.

Correlation between the two registers depends on the `ticgit:<8-hex>` spelling, not on
`` ti `xxxxxx` ``: an entry written the short way is re-filed as untracked by the next sweep.
Read the census above for the current answer to the release gate; read `ti list --all` for
the current queue.

Types:

- **Type 1** — not blocked and needs no prior decision/discussion; can be picked up as-is.
- **Type 2** — needs a decision or discussion next (posture, scope, or design choice).
- **Type 3** — blocked, with the blocking reason stated.

---

# Type 1 — Ready

## pre-network-recomputation (OI-0041)

> **RESOLVED 2026-09-04 (partial), and COMPLETED 2026-10-14 — the residual Required Action 1 landed, so nothing is now owed on this entry.** The `greedy_plan` per-row re-encode is gone: each piece is encoded once and their running sum guards the exact call, which still decides every split. **Split points are provably unchanged** — the sum is an upper bound on encoding the same bytes whole and `output_estimate` is monotone in its payload, so the guard may only skip an encode it did not need. That is load-bearing rather than incidental: a window boundary sets its `source_hash`, a `CacheKey` axis, so a moved split point would silently invalidate cached translations. The property is asserted over a corpus by `the_cheap_bound_never_undercounts` rather than assumed, and that test corrected the change — the bound was expected to be *exact* on a pre-token-boundary argument and is not, a merge crossing the join at 36 tokens whole against 37 summed. Workspace 1642 passed / 0 failed; commit `314d43f`. **`docs/project/open-issues.md` still gives “residual obligation (ti `b3e8d80c`)” as OI-0041's reason for not being archived; that reason is now spent, and this command does not edit that register.** Six of eight were declined on this entry's own severity verdict, two deferred.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0044, R0009-0045, R0009-0046,
  R0009-0047, R0009-0049, R0009-0050, R0009-0051, R0009-0073), gate-accepted,
  user-routed track
- **Sources:** docs/project/open-issues.md#OI-0041, reviews/reviewed/0009.md#R0009-0044,
  reviews/reviewed/0009.md#R0009-0045, reviews/reviewed/0009.md#R0009-0046,
  reviews/reviewed/0009.md#R0009-0047, reviews/reviewed/0009.md#R0009-0049,
  reviews/reviewed/0009.md#R0009-0050, reviews/reviewed/0009.md#R0009-0051,
  reviews/reviewed/0009.md#R0009-0073, ticgit:b3e8d80c
- **First seen:** 2026-08-26 · **Last seen:** 2026-10-14

### Description

Eight findings in `transync-core`'s once-per-run, pre-network phase, each
recomputing something it already had. Two edits cover all eight. In
`unit/split.rs`, `split_oversize_tables` prices the window plan twice —
`if !units.iter().any(|u| windows_of(…).is_some()) { return; }` immediately
followed by the same `windows_of` call in the loop — and `window_units` then
re-parses the parent a third time (`split_table_rows(&parent.source_payload)`,
a full comrak parse, under a comment reading `// Re-sliced rather than threaded
through:`). In `profile.rs`, `default_profile()` re-parses the embedded TOML
with no `OnceLock`, `load_profile` decodes the same text twice (typed
`toml::from_str`, then `toml_text.parse::<toml::Value>()` for the unknown-key
warnings), `entry_applies_to_section` allocates a fresh canonical `String` per
comparison, `effective_glossary` canonicalizes each term twice per section, and
`unit.rs` walks the glossary twice per section for two different questions.

### Background

Type 1: every fix shape is settled and nothing must land first. Six of the
eight were filed above their verified severity, and the register exists partly
to say so — all eight run before the first provider request and are invisible
next to one round trip; the glossary loop is dwarfed even locally by the
tiktoken encode in the same loop. Verification also corrected three claims that
would otherwise misdirect the work. R0009-0073's headline ("`SplitPlan` owns
cloned `TranslationUnit` values") is **refuted** — `merge.rs`'s `Window` holds
only `unit_id` and `source_payload`, and the copy is documented as deliberate —
but verification found a bigger clone the review walked past in the same file,
`window_records.push(vu.clone())` on a whole `ValidatedUnit`. R0009-0044's
double planning is bounded by `.any()`'s short-circuit, so a document with no
oversize table pays one pass; the unnamed and genuinely quadratic cost is
`greedy_plan` re-encoding the whole growing window payload once per body row.
And R0009-0051's two glossary passes must **not** be merged by reusing
`cohort_key`: `ever_applied` is DCR-0027's G7 obligation and must count entries
that applied but were shadowed, which `cohort_key` excludes. The sharpest thing
in the cluster is not a cost at all — `impl Default for ProfileMetadata` parses
TOML and can panic.

# Type 2 — Needs decision

Each entry here needs a decision or discussion before work can start: a posture, scope or
design choice nobody has taken yet, or an owner deferral whose blocker or re-trigger the entry
states. An entry with an `OI-` id or an `Unfiled:` line instead of a ticket waits for
`reopen`'s selection gate to offer it and file a ticket for whatever is picked, because a review
gate registers documents only. An entry whose resolution is verified against code or a shipped
artifact moves verbatim to `docs/backlog-archive.md`, naming the record that settled it.

Four entries carry a `RESOLVED` banner and stay because their open-issue entries do: OI-0041
(Type 1) and OI-0040, OI-0046 and OI-0048 below. Each has a residual obligation named on its
Sources line (`ticgit:b3e8d80c`, `ticgit:26238261`, `ticgit:a736f3fd`, `ticgit:830de884`).

The ten items below were reviewed by the owner on 2026-08-06 and **deferred as a group**
("이번에는 보류" — hold for now). They stay listed so the next sweep re-surfaces them;
none should be picked up without a fresh owner decision. None of the ten was ruled on
again, ticketed, or otherwise moved during the arc that followed.

## streaming-translation _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** The pipeline is whole-batch request/response only; no
  incremental/streaming delivery path exists. Post-MVP feature wave; needs product/design
  discussion before any work.
- **Background:** mvp-scope DEFERRED table; "not a slice" list.
- **Sources:** ticgit:7e42046d-07fd-4fdd-aa29-407039fd92cd
- **Last seen:** 2026-09-12

## sentence-level-sub-anchors _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** Sync anchors are block-level only (draft NG3). Listed in the DEFERRED —
  not permanently-rejected — table, so picking it up is a posture decision with large
  design implications (invariant 1 names block ID as the only sync currency).
- **Background:** Draft non-goal NG3 retained as deferred in mvp-scope; also named by
  ADR-0001 as the finer-grained alternative it declined.
- **Sources:** ticgit:ad9fb766-1cfa-4b53-a138-d5cb296293e0
- **Last seen:** 2026-09-12

## mdx-frontmatter-math-support _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** MDX, YAML frontmatter, and math syntax are unparsed/unsupported (draft
  NG2), retained in the DEFERRED table. A posture decision, and technically entangled with
  the dialect-trait item (Type 3).
- **Background:** Draft non-goal NG2; mvp-scope DEFERRED table and scenario-matrix
  out-of-scope list.
- **Sources:** ticgit:90e8846e-6e6b-4b82-93fa-8c3ba14d5d29
- **Last seen:** 2026-09-12

## glossary-editor-ux _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** Glossaries are hand-edited profile TOML; no editor tooling exists.
  Post-MVP product decision.
- **Background:** mvp-scope DEFERRED table ("post-MVP feature wave").
- **Sources:** ticgit:3e49630b-2c77-4504-9411-b7447e7f71e8
- **Last seen:** 2026-09-12

## html-attribute-text-translation _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** HTML attribute text (`alt`/`title`/`aria-label`) is not extracted for
  translation. A scope decision on widening the segment-extraction contract.
- **Background:** DCR-0016 / ADR-0018 follow-up list (2026-08-03 design, §9).
- **Cross-reference (ti `490d97` wave 7):** now also the HTML-document intake's recorded
  limitation — ADR-0025 names the visible consequence (a shared link previews in the
  source language). The owner gate is unchanged; the population grew from raw-HTML
  islands to whole documents.
- **Sources:** ticgit:60695aa0-0860-429e-831a-1b50a5227edf
- **Last seen:** 2026-09-12

## html-details-fold-reproduction _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** Interleaved `<details>` regions in raw-HTML blocks always render
  visible; collapsed/expanded state is not reproduced. Scoped out of v1 by decision 5 —
  reviving it is a scope decision.
- **Background:** DCR-0016 follow-up list.
- **Sources:** ticgit:6f58ab6a-09a6-44e3-980e-d7bc6b0e2253
- **Last seen:** 2026-09-12

## html-nested-in-list-blockquote-protection _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** HTML blocks nested inside list items/blockquotes are protected only by
  coarse child-kind topology labels — the LLM can still mutate tag text there. Closing the
  accepted v1 gap needs an extraction-inside-containers design.
- **Background:** DCR-0016 follow-up list ("an accepted v1 gap").
- **Sources:** ticgit:eb322221-e12d-4bc6-abdb-e5e85a3b2511
- **Last seen:** 2026-09-12

## intra-block-progress-indicator _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** Within very long blocks (large tables, long code blocks) sync feels
  coarse; an optional intra-block progress indicator is the drafted mitigation
  (references/draft.md §18.2). Post-MVP UX decision.
- **Background:** ADR-0001's accepted "Bad" consequence of block-level sync currency.
- **Sources:** ticgit:1b2e163c-92f6-478a-879c-4d7e345f81f7
- **Last seen:** 2026-09-12

## browser-sync-ux-polish _(owner-deferred 2026-08-06)_

> **One of the three claims below expired, and is corrected here — 2026-09-09, ticket
> `3f0879ef`'s experiment.** The easing policy has not been a **20%/frame** lerp since
> R0011-0094 (`e125a3a`, 2026-09-06): the factor is applied per elapsed **millisecond**, as
> `SMOOTHING_FACTOR` raised to `elapsed / NOMINAL_FRAME_MS`, with `MAX_FRAME_STEP_MS` capping the
> arrears one frame may claim — so a gesture settles in the same wall-clock time at 120 Hz as at
> 60 Hz, which is the defect the words "20%/frame" describe. **Single and fixed still hold; "per
> frame" does not.** This entry was written 2026-08-06 and the change landed a month later, and
> nobody re-read it — which is what a parked entry costs. All three claims are now pinned by
> `crates/transync/tests/parked_claims_sync_engine.rs`, so the next expiry is a failing assertion
> rather than another month of nobody looking.

- **Type:** 2
- **Description:** Three small accepted MVP simplifications in the sync engine: no
  hysteresis on active-block selection (rapid flicks may briefly pick a neighbor); a
  single fixed partner-pane easing policy — **time-based since 2026-09-06**, ~20% of the
  remaining gap per 16.7 ms of elapsed time rather than per frame, and still not variable
  by any caller; and `SMOOTHING_FACTOR` hardcoded rather than a `mountSync` option. Each is
  small; whether any is wanted is a UX priority decision.
- **Background:** web/SMOKE.md "Known limitations (post-MVP)"; 2026-05-03 smooth-scroll
  spec's deferred list. The hysteresis and `SMOOTHING_FACTOR` halves were re-executed
  2026-09-09 and both still hold: `activeBlockWithProgress` keeps no memory of its last
  choice, and `mountSync` takes three positional parameters with no options object.
- **Sources:** ticgit:4a9a93bc-a7ef-4a05-87e7-c73a330edea1
- **Last seen:** 2026-09-12

## in-browser-retranslation _(owner-deferred 2026-08-06)_

- **Type:** 2
- **Description:** The wasm demo edits and re-renders locally but cannot re-run
  translation in the browser — `transync-core` cannot compile to wasm32 (tokio/tiktoken).
  Enabling it needs a network boundary or a second crate split; scoped as its own future
  project.
- **Background:** Track C design spec §9 (2026-08-05), explicitly framed as future work.
- **Sources:** ticgit:27808ef5-6b50-40c2-b5c9-1057d8afb4fd
- **Last seen:** 2026-09-12

---

## repeated-parsing-on-the-accepted-path (OI-0040)

> **RESOLVED 2026-09-04 (partial), and the residual is deferred with a SELF-FIRING trigger.** The `[`-prefilter landed at both `inline_inventory` sites; the shared-AST **seam** is deferred, and the deferral carries its own tripwire rather than a promise: `build_batches` now raises one run-level `tracing::warn` when `unit_count × |ref_defs|` passes 6,000,000 bytes, naming `Document.ref_defs` as the knob. **Re-trigger: that warning firing on a real run.** Measured: +1,756 ms on a 1,289 ms baseline at 5,000 units × 20 KB pool; the prefilter recovers 51.8% at this corpus's bracket density and 0% at link-reference house style, which is why the residual is real rather than closed. Also corrected here: this entry's sentence that R0009-0034 and R0009-0037 "are **not blocked** by anything and should ride OI-0037" is **dead** — OI-0037 resolved 2026-09-01 the other way (it shared a *guard*, not an artifact), so those two are re-homed onto OI-0040. **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0031, R0009-0034,
  R0009-0035, R0009-0037, R0009-0043, R0009-0065), gate-accepted, user-routed
  track
- **Sources:** docs/project/open-issues.md#OI-0040, reviews/reviewed/0009.md#R0009-0031,
  reviews/reviewed/0009.md#R0009-0034, reviews/reviewed/0009.md#R0009-0035,
  reviews/reviewed/0009.md#R0009-0037, reviews/reviewed/0009.md#R0009-0043,
  reviews/reviewed/0009.md#R0009-0065, ticgit:26238261
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Six findings, one shape: no seam in this workspace carries a parsed artifact,
so every layer that needs an AST — or a spliced HTML fragment — builds its own
from the same bytes. Per attempt, one candidate payload goes to comrak up to
four times (`validate/per_kind.rs`'s `check_heading`,
`validate/fragment_reparse.rs`, `validate/inline.rs` → `inline_inventory`, and
`structure.rs` for table/list/blockquote kinds); none of the four accepts a
pre-parsed AST. `inline_inventory` additionally appends and reparses the
**whole-document** reference-definition pool for both sides of every unit
(`format!("{payload}\n\n{ref_defs}")`, with `ref_defs` coming straight from
`doc.ref_defs`), and `pipeline/dispatch.rs` recomputes the source side once per
retry round because `TranslationUnit` memoizes nothing. An accepted HTML unit is
spliced at least twice — once in `validate.rs` purely for a `tag_inventory`
comparison, then again in `regen.rs`, and up to three more times through
`pipeline/finalize.rs`'s repair ladder — and `transync_html::splice` itself runs
two complete `HtmlRewriter` passes over the same bytes. A WASM `rebuild_impl` is
**four** whole-document parses, not two.

### Background

Type 2: three of the six cannot be started without a decision, so the cluster
tie-breaks up. R0009-0031's fix pushes an arena lifetime through
`transync-syntax`'s public render signature, because `intake::markdown::parse` owns its
arena and returns owned IR. R0009-0043 must **not** be fixed by carrying the
validated splice into regen: the two calls take different inputs — validation
uses `constraints.html.source_bytes` from `outcome::block_payload`, regen slices
`doc.source_text` via `intake::markdown::ranges::clamped_char_bounds` — and `validate.rs`
documents the divergence as load-bearing ("cannot make this layer lie about
regen's success"), so sharing collapses a deliberate independent double-check.
R0009-0065's recommended one-scan staging is not implementable as written, since
lol_html's `TextChunk` exposes no source byte offsets. Verification found five
of the six overstated — microsecond-scale work on one block in a network-bound
pipeline — with exactly one exception that is worth doing on its own merit:
R0009-0035 is O(units × |ref_defs|) in both allocation and parse time, i.e.
quadratic in document size for a reference-definition-heavy document, and its
cheapest sound fix is a per-side prefilter (skip the append when the payload
contains no `[`), not the label-matching reimplementation the review proposed.
R0009-0034 and R0009-0037 are **not blocked** by anything and should ride
OI-0037, whose Required Action 1 already mandates funnelling exactly those
`comrak::parse_document` sites through one guarded entry point — the same
refactor with a second justification, not a prerequisite.

## checking-apparatus-holes (OI-0046)

> **RESOLVED 2026-09-04, and sub-item 3 paid for itself immediately.** (1) The rustdoc completeness loop moved into the shared `rustdoc-gate.sh`, so the hook gained it — its candid deferral trigger had **already fired** (`Developer_Guide.md`'s gate command omitted `transync-lang`). (2) The Playwright port got an env fallback. (3) The "existing coverage suffices" demonstration **failed on measurement**: 8/8 historical divergence classes are pinned, but **not one of the 38 pinned edge cases produces a single orphan close tag**, so `balance_fragment`'s orphan-deletion pass — the first thing it does — had zero corpus coverage; six entries carry a literal `<` and none carries one alongside an orphan closer. The two ingredients existed separately and never together, which is the combinatorial gap a hand-picked corpus cannot close. `generative_properties.rs` (840 lines, nine-line `splitmix64` over four pinned seeds, `std` only, no dependency, no `cargo-fuzz`, no `#[ignore]` — because the hook is the only automatic gate and a coverage-guided harness would run at no venue) found a **live anchor-injection defect on its first execution**, now `ticgit:fdd989`. 400,000 generated inputs over 4 seeds: 399,872 satisfy every property, 128 fail, all one class.

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0015, R0009-0018,
  R0009-0067), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0046, reviews/reviewed/0009.md#R0009-0015,
  reviews/reviewed/0009.md#R0009-0018, reviews/reviewed/0009.md#R0009-0067, ticgit:a736f3fd
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Three findings about what this repository's checks cover rather than about
shipped behaviour. The rustdoc **completeness** check — the loop that catches a
library member nobody added to the gate, ending in
`[smoke] FAIL: library member(s) outside the rustdoc gate` — exists only in
`scripts/smoke.sh`; the pre-commit hook sources `scripts/lib/rustdoc-gate.sh`
and fails only if `RUSTDOC_GATE_ARGS` is empty, so a new library crate can be
committed undocumented with the hook green. `web/playwright.config.js`
hard-codes `const HOST = "127.0.0.1"` and `const PORT = 4319` with no
`process.env` fallback, and `scripts/test-browser.sh` passes no port at all, so
two concurrent runs collide — and the port is not the only shared resource, since
that script also `rm -rf`s a single default `WORKDIR`. And there is no committed
fuzz, property or differential harness anywhere in the repository: a grep for
`proptest|quickcheck|arbitrary|cargo-fuzz|libfuzzer|afl` across the manifests
returns nothing, `find . -type d -name 'fuzz*'` returns nothing, and
`crates/transync-html/tests/` holds only a three-fixture golden pin, leaving a
hand-rolled tokenizer on the untrusted-input path validated by 79 handpicked
cases.

### Background

Type 2: two of the three need a choice before code. The browser harness needs a
decision about how a second concurrent run gets **both** its port and its
workdir — a port-only change would leave the destructive resource shared — and
the missing generative harness needs its shape chosen (in-crate property tests,
a `cargo-fuzz` target, or a differential oracle against a browser parse) and its
home decided, given there is no CI and the hook is the only automatic gate.
R0009-0015 alone is settled and cheap. Verification narrowed it twice: the
smoke/hook split is **documented and deliberate** (`rustdoc-gate.sh` says "What
keeps it total is the completeness check in `scripts/smoke.sh`") and there is no
live gap today, since all nine members with a `src/lib.rs` are in
`RUSTDOC_GATE_CRATES` — but the check is a pure bash glob with no cargo cost, so
moving it into the shared library closes the window for free. The fuzz item has
the strongest expected value in the entry, and it is not speculative: this file
already records "15,726 violations / 200k fuzz iterations" behind ticket
`95f55b` — someone fuzzed this crate ad hoc, found a real defect, and did not
keep the harness. R0009-0017, the sibling foreign-server finding, was fixed in
`88964df`; build on it rather than duplicating it.

## boundary-checks-weaker-than-contract (OI-0048)

> **RESOLVED 2026-09-04 (R0009-0052 deferred).** Policy: the library refuses where a real caller can reach, and records with a named trigger where none can yet. R0009-0053 landed as `TransyncError::InvalidOptions` with the new stable code `invalid_options`; **the reachability argument inverted in flight** — both roster consumers guard `target_language`, which first read as "unreachable", but the guards exist *because* transync reported the cause as `internal`, so they are downstream compensation for an upstream defect and naming the cause is what retires them. R0009-0075 landed; R0009-0078 landed as its doc half only. R0009-0052 **deferred**: `ProfileConstraints` is `#[non_exhaustive]`, so no external caller can build the unrecognized value. **Re-trigger: a caller-built `ProfileConstraints` becomes constructible, or a consumer reports a silently-whole-block table.** **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0052, R0009-0053,
  R0009-0075, R0009-0078), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0048, reviews/reviewed/0009.md#R0009-0052,
  reviews/reviewed/0009.md#R0009-0053, reviews/reviewed/0009.md#R0009-0075,
  reviews/reviewed/0009.md#R0009-0078, ticgit:830de884
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Four places where the library enforces less than its record implies, each
visible only to a consumer that is not the CLI. `profile.rs` resolves the table
strategy with `match … { Some("row-window-first") => RowWindowFirst, _ => WholeBlock }`
and warns about an unrecognized value only in `load_profile`, so a programmatic
caller who sets `Some("rowwindow")` on the `#[non_exhaustive]`
`ProfileConstraints` gets whole-block with no warning. `transync-core/src/lib.rs`
returns `TransyncError::Internal` for an empty `target_language`, which
`error.rs` maps to the wire code `internal` — the code reserved for engine
faults — for what is a caller input error. `transync-syntax/src/align.rs`
synthesizes `FallbackStatus::FallbackSource` for a block the status map does not
name, with no `tracing::warn` (unlike the missing-offset path right below it),
guarded only by a `debug_assert!` in `pipeline/report.rs`. And
`render.rs`'s `PaneCtx::new` performs exactly three checks — duplicate
`source_block_id`, `UncoveredBlock`, `range_fault` — while `Pane::range` reads
`block.source_range` for the source pane, so the **row's** `source_range` is
never read or bounds-checked even though row fields do reach the DOM.

### Background

Type 2: two of the four cannot be closed without a choice, and grouping them
prevents four inconsistent answers about how much a programmatic caller is owed.
R0009-0053 needs a new `TransyncError` variant plus a `stable_code()` row, which
means editing contracts.md §1 where it states "The complete set is **twenty**
codes" — a wire-vocabulary change, not a one-liner. R0009-0078 offers two routes
and neither is obviously right: assert `row.source_range == block.source_range`
in `PaneCtx::new`, or state in §4a that the source pane reads the block's range
and the row's field is advisory; §4a's current sentence — "Each pane is measured
against what it slices: `source_range` against the source text" — does not say
*whose*, which is the ambiguity. Verification also refused two of the review's
recommendations outright. Converting `default_table_strategy` to an enum
collides with the TOML wire shape and §0's frozen-field policy; the cheap fix is
to repeat the loader's unknown-value warning at the translate boundary, as
R0001-0020 did for prompt template variables — and the impact is loud but
**mis-diagnosed**, not silent, since an unsplit oversize table is still named by
`output_budget_warnings` and aborts at the provider under ADR-0017. Making
`build_alignment_map` strict is **not available**: `transync-wasm`'s
`engine.rs` documents that "a block absent from `statuses_json` gets the same
synthesized status the pipeline would give it" and
`omitted_unit_backed_html_row_is_fallback_source_not_translated` pins it, so
hardening belongs at the `report.rs` seam. R0009-0075's artifact is not
corrupted either — the row is labelled `fallback_source`, the least-integrity
status, and counted into `summary.fallback_source`; what is lost is the
distinction between "validated and rejected" and "dropped by a bug". Most of
R0009-0078 describes a documented boundary rather than a broken promise: §4a
enumerates the renderer's three refusals, the CLI never renders a foreign map,
the shipped JS gates two of the listed fields, `data-order` has no consumer in
`web/`, and the only live foreign-map path is the WASM view mode ADR-0023
assigns to the demo.

## provider-capability-set — **AWAKE: condition fully fired, no block left**

- **Type:** 2
- **Description:** No capability descriptor on providers (what strategies/limits a
  provider supports).
- **Background:** DCR-0009 YAGNI list.
- **Unblocked (was: stated revisit condition — a second in-tree provider or the
  oversize-split feature shipping):** **BOTH LIMBS HAVE FIRED.** `fc0304` / DCR-0026
  shipped the oversize row-window split on 2026-08-09, and `bda471` / DCR-0029 landed
  `crates/transync-anthropic` on 2026-08-10, so a second in-tree provider now exists.
  Nothing blocks this entry any more; it is **awake and ready for a decision**, which
  makes it Type-2 work. It was **moved into the Type 2 section on 2026-09-11** and filed
  as `ticgit:6fb6dab2` with a `Needs decision:` marker, because an entry whose stated
  revisit condition has fired is not blocked on anything and saying otherwise in the type
  field is what lets a fired condition sit unasked.
  It is not resolved: what the landings produced is the first real *evidence*, and it points
  both ways. In favour of a descriptor — the second adapter genuinely does differ
  in capability, not just in wire spelling: its output ceiling is **required**
  where the first's is optional, its structured-output schema dialect is narrower
  (a crate-private profile pass strips keywords the shared schema stamps), its
  effort vocabulary is a different set, and one stop reason
  (`model_context_window_exceeded`) has no counterpart on the shipped adapter at
  all. Against — every one of those was expressible as adapter-local behavior
  plus, in the one case that reached the seam, a new `TranslatorError` variant;
  the core trait did not move, which is exactly what ADR-0002 promised, and a
  descriptor would have bought none of it. The open question the *next* provider
  should settle is whether core ever needs to **branch** on a capability rather
  than let the adapter absorb it — nothing in DCR-0029 required that. Re-read
  DCR-0029's *Where it mirrors, and where it must differ* table before deciding.
  The other limb, `fc0304` / DCR-0026, adds one datum of its own rather than
  settling it: the split is a **core-side** decision that has to measure against an
  output ceiling, and the ceiling it reads is the profile's
  `[batching].target_output_tokens` (via `batch::effective_output_target`), not
  anything the provider declares. A capability descriptor is one place that number
  could have come from instead — which is an argument for a descriptor that the
  second-provider limb did not produce, and the sharpest concrete question to put to
  the decision.
- **Sources:** ticgit:6fb6dab2-a2ac-486c-b06b-932b6a4d9796
- **Last seen:** 2026-09-12

## disk-cache-meta-records-evictability

- **Type:** 2
- **Verified:** yes — reopen verified 2026-09-12 first-hand: `trim_to_budget` computes
  `unreclaimable = header_bytes() + replayed.meta_bytes` and drops only entries, with the exemption
  stated in the module's own doc comment. **Second pass 2026-09-13** (two adversarial lenses plus a
  completeness critic; every quote re-checked first-hand): all code, record and ticket quotes in
  ticket `9c26d90c` reproduce, and three arguments were refuted and corrected there — the declined
  capacity CLI lever *does* have a recorded revisit path (two falsifiable triggers on the
  `max_bytes` rustdoc and in `contracts.md`, with the floor precondition shipped 2026-09-03, so the
  residue is the two stale "needs OI-0044's floor first" sentences), the oldest-first hazard to a
  `doc_meta` record ignores the log's write order (harvest → entries → `doc_meta`, and compaction
  rewrites every document-scoped record after the entries), and a sixth route was missing (measure
  the budget against the reclaimable bytes only — R0009-0082's first Recommendation in code form).
  **Type contested:** lens A reads type 2 (the register calls the question open), lens B reads type 1
  (the exemption is a recorded decision — DCR-0028 §3/§4 plus the 2026-08-10 note, accepted by
  R0009-0082's own Decision reference — and the one sentence calling it open arrived inside the
  commit that closed the member). This pass recommends type 1 with the contest on record
- **Sources:** ticgit:9c26d90c, docs/project/open-issues-archive.md#OI-0044,
  docs/project/design-change-records/DCR-0028-disk-backed-cache-and-document-metadata.md
- **First seen:** 2026-09-12
- **Last seen:** 2026-09-13

### Description

The disk cache log holds two kinds of record: per-unit translation entries, and document-scoped
records (a run's detected source language, and the auto-glossary harvest). Only the first kind is
subject to the `max_bytes` byte budget. `trim_to_budget` drops entries alone and treats the header
plus `meta_bytes` as unreclaimable; `trim_to_budget`'s own doc comment calls the document-scoped
records **exempt**, and the public `DiskCacheOptions::max_bytes` rustdoc — the text an operator
actually reads — calls them "exempt by design". OI-0044 fixed the consequence of that in 2026-09-03 — a floor so the drop
loop stops once dropping everything still cannot get under budget — and then recorded, inside a
`RESOLVED` entry, a question it deliberately did not answer: *"Whether meta records should instead
become evictable remains a separate open question and was deliberately not answered by the floor."*
Nobody has reopened it since, and until this sweep it was in no register at all: OI-0044 is
resolved, and its own backlog entry is marked retained-for-the-record and does not mention it. The
2026-09-13 verification pass adds the other half of the picture: the exemption itself is a recorded
decision with a rationale — DCR-0028 §3 ("there is no metadata eviction"), §4 ("exempt from capacity
trim — they are tiny and superseded in place"), the 2026-08-10 note extending it to the harvest, and
R0009-0082's own Decision reference ("DCR-0028 accepts metadata/glossary retention; the contract
should state the resulting non-hard bound") — so what is unambiguously unfinished is the
documentation that gate owed, while whether keeping the exemption is still a live choice is exactly
what the two lenses disagree about.

### Background

The two record kinds are not symmetric, and that asymmetry is the whole decision. Losing a unit
entry costs one re-translation — squarely inside the "degrade to re-translation" envelope DCR-0028
already claims. Losing a document-scoped record costs something different in each of its two
cases: a dropped detection record makes a fully-warm offline rerun report a null source language,
which is the exact defect OI-0017 was filed for and DCR-0028 closed, and a dropped glossary harvest
is re-bought from the provider and may come back with a different term list — which moves
`glossary_hash`, which is a cache-key axis, which orphans that document's unit entries. So the
cheap-looking eviction has a cascade behind it that the expensive-looking exemption does not.
Against that, the exemption is what makes the floor necessary at all: an operator who sets
`max_bytes` small relative to accumulated meta *used to* get a discard-everything trim on every
open; since the 2026-09-03 floor the budget is ignored for that open with one warning instead, so
what is left below the floor is decaying entry capacity rather than a trim per open. DCR-0028 §4's
2026-09-03 amendment does name the exemption; the residue is its pre-amendment sentence and its
SL-113 slice line, which name `doc_meta` only while the code exempts both kinds, and `contracts.md`
§1, which states neither what the budget measures nor what eviction can reclaim. **Typing is
contested after the 2026-09-13 pass:** type 2 if the choice of which guarantee the byte budget makes
is still open, type 1 if DCR-0028 §3/§4 and the review gate already made it and only the documents
lag.

### Options

**Keep the exemption and let the public documentation be the whole answer** *(source: the shipped
state after OI-0044's floor, and R0009-0082's own Recommendation)* — no code moves, but it is not
free: three documentation residues have to be cleared (DCR-0028 §4's and SL-113's `doc_meta`-only
shorthand, `contracts.md` §1's silence on what the budget measures and can reclaim, and the two
stale "a lever needs OI-0044's floor first" sentences), and the record cost is a dated DCR-0028 §4
amendment note plus a sentence in `contracts.md` §1. Distinguishing point: it is the one route
under which `max_bytes` keeps meaning something other than what DCR-0028 §4 says it means.

**Make meta records evictable under the same oldest-first rule the entries follow** *(source:
OI-0044's own sentence)* — makes the budget literal. Blast radius is the cascade above: it
reintroduces the null-language rerun DCR-0028 closed, and a re-bought harvest can orphan unit
entries through `glossary_hash`. The 2026-09-13 pass narrows the first half: the log's write order is
harvest → entries → `doc_meta`, and compaction rewrites every document-scoped record after the
entries, so an oldest-first rule inheriting the entries' replay-position `seq` reaches a `doc_meta`
record only once that document's entries are already gone — which a subsequent dispatching run then
rewrites unless its detection latch held nothing. Needs a DCR-0028 amendment, since it reverses a stated design
property. Reversible in code, but not in the caches already trimmed.

**Give the meta records their own sub-budget inside `max_bytes`** *(proposed by reopen)* — bounds
both kinds without letting either starve the other; the most code of the five and the only one
adding a public knob, which is a §0 surface question and therefore a window question.

**Refuse a `max_bytes` below the unreclaimable floor at construction time** *(proposed by reopen)* —
smallest change that makes the failure loud instead of silent, and the only route that touches no
eviction semantics at all. It closes the operator-facing symptom without answering the question the
entry asks, which is either its virtue or its evasion depending on which the owner wants.

**Evict the glossary harvests only, keeping the detection records exempt** *(proposed by reopen,
research pass 2026-09-12)* — splits the two kinds on the ground that their costs differ. Needs the
same DCR amendment as full eviction but carries only half its cascade; distinguishing point is that
it is the only route derived from what the two record kinds actually cost rather than from what the
budget says.

**Measure the byte budget against only the reclaimable bytes (header plus entries), leaving the
document-scoped records neither counted nor evicted** *(proposed by reopen verification 2026-09-13;
the code form of R0009-0082's first Recommendation, "an evictable-unit budget")* — one comparison in
`trim_to_budget`, no record deleted, no public field, no ADR-0021 amendment, and the only route that
stops entry capacity decaying as documents accumulate. Its cost is what `max_bytes` promises: the
file may exceed it by the live document-scoped population, which is the "non-hard bound" the review
gate asked the contract to state. Both floor tests have to be revisited — an entries-only
measurement makes every budget satisfiable and breaks both, while a header-plus-entries measurement
keeps one passing for a stale reason.


## local-llama-provider-deferral-has-no-trigger

- **Type:** 2
- **Verified:** yes — reopen verified 2026-09-12 first-hand against
  `docs/architecture/mvp-scope.md`'s Out of scope (DEFERRED) table, whose row reads "stays
  deferred: trait in place; that sibling crate lands later"; **re-verified 2026-09-13** by the
  adversarial pass the filing sweep skipped — two independent lenses plus a completeness critic,
  both partly-confirmed, **type 2 upheld and uncontested**. Three material corrections, each
  re-reproduced before being encoded: the anthropic landing moved **five** shared places, not six
  (the root `[workspace.dependencies]` table deliberately has no entry for an adapter nothing
  depends on, and `workspace_publication.rs` fails one as `declared but unused`); ticket
  `6fb6dab2` lists **ten** options, not four; and a decision *was* taken — ticket `bda471`
  records "Anthropic first; local-llama later. Scheduled post-0.3.0." on 2026-08-06, a sequencing
  decision whose roadmap completed 2026-08-10, so what is missing is the trigger and not the
  decision. Both downstream consumers were read first-hand on this machine and neither pins
  `transync-anthropic`, so "no recorded demand" now rests on evidence rather than on an
  unperformed check. A sixth option was added, and Option 2 lost its claim that a live gate could
  be run here (no local model server is installed).
- **Sources:** ticgit:3887f02e, docs/architecture/mvp-scope.md, docs/project/intake.md,
  docs/decisions/0002-http-free-core-with-translator-trait.md
- **First seen:** 2026-09-12
- **Last seen:** 2026-09-13

### Description

A third provider crate, `transync-local-llama`, has been carried in the MVP scope document's
deferred table since the project's first design day. Its sibling on the same row,
`transync-anthropic`, landed on 2026-08-10 and the row records that; the local-model crate reads
only "stays deferred: trait in place; that sibling crate lands later." That is a deferral with no
trigger, no date and no recorded demand — the one shape this register exists to surface, because
nothing in it can ever fire and nothing in it can ever be falsified. It will be re-collected by
every sweep that reads the table, which is how it reached this entry.

### Background

ADR-0002 promises that a new provider is a purely additive sibling crate costing the core trait
nothing, and DCR-0029 is the only measured test of that promise: landing the second provider took
five slices and moved the core trait not at all, with one `TranslatorError` variant riding an open
window. So the cost of a third is known better than it was when the row was written, which is
precisely what makes the row answerable now and did not before. There is a live second-order
consequence, and it is why this is worth a decision rather than a shrug: the open
`provider-capability-set` ticket treats a third provider as the natural next data point for
whether a provider should describe its own limits, so one undecided row is propping up a second
undecided question. The research pass could find no ticket for the deferral itself. It also reported no recorded
demand from either named downstream consumer without having checked them; the 2026-09-13
verification pass checked both on disk and the premise survives — `/Volumes/Common/QJoon/resp-translator`
and `/Volumes/Common/QJoon/dynwebserver` both pin this workspace at tag `v0.5.0`, neither pins
`transync-anthropic`, `dynwebserver` already exposes a configurable `translate.base_url`, and
resp-translator's own scope lists Ollama as trait-reserved and out of scope for M1. The same pass
refuted the sharper claim that nobody had decided anything: ticket `bda471` records "Anthropic
first; local-llama later. Scheduled post-0.3.0." on 2026-08-06, so the owner fixed a sequence and
not a trigger — and the post-0.3.0 roadmap it pointed at completed on 2026-08-10, which is what
leaves the word "later" pointing at nothing. It also could not establish what the crate was ever
meant to be concretely — whether llama.cpp specifically, a named local runtime, or "some local
model" as a placeholder — and that ambiguity is itself part of what needs deciding. Type 2 because
nothing external is unmet: the question is whether the project wants this at all, and only a person
answers that. **This entry is not a request to build it.**

### Options

**Close the row as rejected and correct the roster to say two providers are the set** *(proposed by
reopen)* — smallest effort, and the only route that reduces the register. Blast radius is the seven
live documents that name the crate, plus `README.md` and `docs/Developer_Guide.md`, which say
further providers are siblings of the same shape and would contradict a closed set, plus the
`provider-capability-set` ticket, whose third-provider option would need re-pricing — and that
ticket lists **ten** options, so striking its Option 2 leaves nine. Reversible: a rejection can be reopened, and ADR-0002 means the trait does not move
either way.

**Commission it as a third provider crate on the anthropic precedent** *(source:
`docs/project/intake.md`, which names it as post-MVP with the trait in place)* — the largest
effort by far, measured rather than guessed at roughly the size of `transync-anthropic`. It is the
only route producing the third data point `provider-capability-set` wants. Needs its own DCR and
slice numbers; a published member is the hardest thing here to withdraw.

**Keep the deferral but attach a falsifiable trigger that fires on its own** *(proposed by reopen)* —
effort is one sentence; blast radius one row. It is the only route that leaves the answer open
without leaving it unanswerable, which is the defect this entry is actually about. Needs no record
beyond the row itself.

**Replace the row with a documented recipe for implementing the `Translator` trait out of tree**
*(proposed by reopen)* — moves the capability to the consumer instead of the roster. A how-to for
custom translators already exists in the manual, so this is largely a pointer plus a scope
sentence; distinguishing point is that it is the only route that answers the underlying user need
without adding a member.

**Ask the two named downstream consumers whether they want it and let the answer decide** *(proposed
by reopen, research pass 2026-09-12)* — the only route whose cost is someone else's time rather
than this repository's. It is no longer the only route that gathers evidence: the 2026-09-13
verification pass read both consumer repositories and found no request for a non-OpenAI-wire local
runtime, so what is left to ask is the narrow question the manifests cannot answer. It defers the
decision by design, which is what the other five are trying to stop.

**Resolve the row as shipped-plus-recipe: date-stamp it RESOLVED, naming `transync-openai`'s
`base_url` for OpenAI-wire local servers and the out-of-tree how-to for everything else**
*(proposed by reopen verification 2026-09-13)* — documentation only, and the only route whose claim
is already made in a shipped document: `README.md` says further providers are siblings of the same
shape and that a consumer names a provider crate directly. It differs from the rejection route in
not closing the set, and from the recipe route in asserting that the OpenAI-wire case is already
served. Needs an in-place edit of ADR-0002's Decision Outcome plus one dated `## History` line (DCR-0054) and a date on the row; no new ADR.

## anthropic-leg-outside-the-release-gate-trigger

- **Type:** 2
- **Verified:** yes — reopen verification 2026-09-13 first-hand: step 10's trigger diff names
  `crates/transync-openai`, `crates/transync-core/src/llm`, `llm.rs`, `batch.rs` and `unit`/`unit.rs`
  and does **not** name `crates/transync-anthropic`; step 11 names the two OpenAI legs by hand and
  mentions the third only parenthetically
- **Sources:** docs/project/release-checklist.md (step 10's trigger diff, step 11's parenthetical,
  the authority note at the top of the file),
  docs/project/design-change-records/archive/DCR-0015-prompt-lift-tokenizer-hint-rtl-live-smoke.md
  (the gate's trigger set), docs/project/status.md (the standing-gap bullet that carries the
  obligation instead), tickets `40b29a9d` and `342e028d` (both compose with this one; provenance,
  not this entry's ticket)
- **Unfiled:** surfaced by the 2026-09-13 reopen verification pass, after the 2026-09-12
  selection gate had already run, so it was never offered there. No ticket is owed until a
  human picks it at a gate
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

The release checklist decides whether the DCR-0015 Part C live-endpoint gate fires by diffing the
release range over five paths, and `crates/transync-anthropic` is not among them. Step 11 then
names the two OpenAI legs by hand, mentioning the third only parenthetically ("if the third leg is
ever run"). So no release range can ever make the `anthropic` leg due: it is outside the gate by
rule, not repeatedly skipped by it — which is why the obligation lives in `status.md` as a standing
gap instead of inside any release's checklist.

### Background

This is a defect in the ritual rather than in the adapter, and it is separable from whether a key
exists: widening the trigger set, and saying per leg whether a run or a recorded skip discharges
the gate, is one document edit that needs no credential. It also explains a pattern the project
keeps re-recording — three consecutive releases cut with the gate triggered and unrun — because for
the adapter the trigger arithmetic would never have fired at all. It is type 2 rather than type 1
because of an authority question that has to be settled first: the checklist's own note makes
editing it record-free, while changing a gate it names is a design change, and DCR-0015 (archived)
records this gate's trigger set by name — so whether the widening needs a dated appended note on
DCR-0015 Part C must be decided before the edit. It is filed apart from `40b29a9d` deliberately: it
is the mechanism that makes that ticket perpetual rather than its content, it is blocked on no
credential, it is wider than the Anthropic adapter, and it composes with whichever route the owner
picks for either blocked ticket rather than belonging inside one of them.

### Options

Routes named, not compared — the comparison belongs to a ticket if one is filed. Add
`crates/transync-anthropic` to step 10's trigger paths and make step 11 enumerate all three legs
with a per-leg discharge rule *(proposed by reopen verification 2026-09-13)*. Leave the trigger set
alone and move the obligation out of `status.md` into a dated checklist step that fires on adapter
changes only *(proposed by reopen verification 2026-09-13)*. Record the leg as permanently
out-of-gate, with the standing gap in `status.md` as its only home and a stated reason *(source:
the status quo, made explicit)*.

## oversize-tombstones-point-at-a-closed-oi-0008

- **Type:** 2
- **Verified:** yes — reopen verification 2026-09-13 first-hand: the tombstone in
  `crates/transync-core/src/pipeline/retry.rs` routes consumer-side oversize handling to OI-0008,
  STUB-029's note says the same, `docs/project/open-issues-archive.md` records OI-0008 as **RESOLVED
  2026-08-05 (DCR-0019)**, `docs/project/open-issues.md` carries no successor entry, and
  `docs/backlog.md` has zero OI-0008 hits
- **Sources:** crates/transync-core/src/pipeline/retry.rs (the `oversize_split` tombstone's OI-0008
  pointer), docs/project/stub-manifest.md (STUB-029's note),
  docs/project/open-issues-archive.md (OI-0008's status line and the dated Related list in the
  DCR-0018/OI-0027 entry), docs/project/open-issues.md (no successor entry),
  ticket `2183fbbb` (which flags it as out of its own scope; provenance, not this entry's ticket)
- **Unfiled:** surfaced by the 2026-09-13 reopen verification pass, after the 2026-09-12
  selection gate had already run, so it was never offered there. No ticket is owed until a
  human picks it at a gate
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

Two live artifacts route future consumer-side oversize handling to an issue the register closed.
The `retry.rs` tombstone says that if consumer-side oversize *handling* is ever wanted it belongs to
"OI-0008's retry/fallback redesign, not to a resurrected empty hook", and STUB-029's note says the
same in its own words — while the archive records OI-0008 as RESOLVED 2026-08-05 (DCR-0019). A
reader or agent following either pointer lands on a closed register entry with no way to tell
whether the redesign was abandoned, absorbed, or merely untracked.

### Background

The archive's one line calling the redesign "still open" sits inside DCR-0018/OI-0027's dated
Related list, so it is correct as written rather than a live status — which means nothing anywhere
tracks a retry/fallback redesign today, and the backlog has no entry for it either. This is type 2
rather than type 1 because the repair is not transcription: somebody has to choose whether the
forward pointer is deleted, re-aimed at ADR-0009 and `contracts.md` §5 as the standing policy, or
replaced by a newly filed open issue — and that choice states whether a retry/fallback redesign is
still wanted at all. It is filed apart from `2183fbbb` because that ticket declares it out of its
own scope and warns it must not be copied forward unexamined; folding it in would widen a settled
type-1 documentation fix into an owner question, and it touches different artifacts (a code comment
and STUB-029, not the three carrier rows).

### Options

Routes named, not compared — the comparison belongs to a ticket if one is filed. Delete the forward
pointer from both carriers and let ADR-0009 plus `contracts.md` §5 stand as the whole policy
*(proposed by reopen verification 2026-09-13)*. Re-aim both pointers at ADR-0009 and `contracts.md`
§5 explicitly, keeping the idea but naming a live record *(proposed by reopen verification
2026-09-13)*. File a successor open issue for the retry/fallback redesign and re-aim both pointers
at it *(source: the tombstone's own premise, that the redesign is a thing that may still be
wanted)*.

## anthropic-adapter-cannot-name-a-workspace

- **Type:** 2
- **Verified:** yes — observed first-hand 2026-10-13 against the live Messages API, not deduced.
  An organization-scoped `ANTHROPIC_API_KEY` drove `./scripts/smoke-live-gate.sh anthropic` to
  **`400 invalid_request_error`**: *“This API key is not scoped to a workspace, so this request
  must include the `anthropic-workspace-id` header with the ID of the workspace to use. Add the
  header, or use an API key that is scoped to a workspace.”* `crates/transync-anthropic/src/
  client/transport.rs` sets exactly two headers — `x-api-key` and `anthropic-version` — and
  `grep -i workspace` over the crate's sources matches nothing but prose.
- **Blocked by:** nothing external. It is a decision, not a prerequisite: the owner holds a
  workspace-scoped key, so the adapter works today and this buys reach rather than function.
- **Sources:** ticgit:3e013e47, the 2026-10-13 live-gate run (CHANGELOG `[Unreleased]`, README provider-evidence
  table, `docs/project/status-archive.md`'s completed standing-gap action), the archived entry
  `anthropic-live-leg-never-run` in `docs/backlog-archive.md`,
  `crates/transync-anthropic/src/lib.rs`, `crates/transync-anthropic/src/client/transport.rs`
- **First seen:** 2026-10-13
- **Last seen:** 2026-10-13

### Description

`transync-anthropic` cannot address an Anthropic **workspace**. It sends `x-api-key` and
`anthropic-version` and nothing else, so a key that is scoped to the organization rather than to
one workspace is refused before the model is reached. Anthropic's own refusal names two remedies
and this adapter supports only the second — use a workspace-scoped key. That is a real constraint
on who can adopt the crate, and it is invisible until the first request.

### Background

It is worth being exact about what this is and is not. It is **not** what blocked the live gate:
that entry is closed, because a workspace-scoped key passed unchanged on 2026-10-13. It is also
not a defect in request shaping — the same run's 400-rather-than-401 is the provider's own
statement that `x-api-key`, `anthropic-version`, the model name and the body are all well-formed.
What it is: an adapter that silently assumes one of the two key classes its provider issues.

Two sub-questions have to be settled together, which is what makes this a decision rather than a
ready task.

**Where the value enters.** `try_new(api_key, model, base_url)` is positional, so a fourth
parameter is breaking and would need a window. A `with_workspace_id` builder method alongside the
existing `with_timeout` is additive and needs none, and `from_env` would grow a fourth variable
beside `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` and `TRANSYNC_ANTHROPIC_BASE_URL`. The
builder road is the cheap one and is almost certainly right; it is named here so the choice is
recorded rather than assumed.

**Whether it rides the fingerprint.** This is the question that actually needs thought.
`ProviderFingerprint` feeds `CacheKey`, so anything included partitions the cache. A workspace is
a billing and access scope, not a model-behaviour one: the same model in two workspaces should
return interchangeable translations, which argues for excluding it, as `ANTHROPIC_VERSION` is
excluded. The counter-argument is that a workspace can carry its own model access and limits, so
a cache entry written under one may not be reproducible under another. Getting this wrong is
cheap in one direction (a redundant cache partition) and silent in the other (a replayed
translation attributed to the wrong scope).

### Options

**Add `with_workspace_id` plus a `TRANSYNC_ANTHROPIC_WORKSPACE_ID` read in `from_env`** — the
expected route. Additive, no window, one header conditional in `transport.rs`, and it makes the
crate usable with either key class. Needs a record, because it widens a published crate's surface,
and that record is where the fingerprint question is answered.

**Document the constraint and stop** — already half-done: the README's provider-evidence table
names it. Zero code, and honest, but it leaves a published adapter that refuses a credential class
its provider issues by default.

**Refuse an organization-scoped key at construction with a message naming the remedy** — composes
with either route above and is the smallest real improvement: the failure stays, but it stops
being a remote 400 and becomes a local error that says what to do. Its limit is that the key class
is not reliably readable from the key itself, so this may only be achievable as a better message
on the provider's 400 rather than as a pre-flight check.


## manual-prose-drift-unguarded

- **Type:** 2
- **Verified:** partly — reopen verified 2026-10-14 first-hand. **Instance (2) reproduces**:
  `manual/explanation/developer/en/validation-retry-fallback-model.md:4` says "seven independent layers"
  and `:78` says "a six-layer stack", against the page's own enumeration at :50-68, which lists seven.
  **Instances (1) and (3) were refuted.** (1) `implement-a-custom-translator.md:497-498` is literally true:
  the v0.4.0 and v0.5.0 windows *are* both used and closed, and the two breaks parked in `[Unreleased]` are
  `transync-syntax`-only and invisible through the facade — the v0.6.0 window being open does not falsify
  that sentence. (3) the compaction-temp claim did not reproduce as filed.
- **Sources:** ticgit:1e0ba235, manual/explanation/developer/en/validation-retry-fallback-model.md
- **First seen:** 2026-10-14 · **Last seen:** 2026-10-14

### Description

One manual page gives the validation stack two different layer counts, seven in its frontmatter
description and six in its body, and its own enumeration says seven. The surrounding question is larger
than the typo: `manual/` prose has **no machine guard**. `crates/transync/tests/manual_source_refs.rs:22-24`
says so outright — "this does **not** catch prose drift: a manual page that describes a flag wrongly, or
omits one, still passes here" — and `docs/backlog.md` already records that `manual/reference/user/en/cli.md`
is a third full CLI reference with no guard.

### Background

Type 2 rather than Type 1 because of the repair mechanism, not the content. These pages carry
`generated.by: codex/gpt-6`, and `docs/superpowers/specs/2026-09-13-docs-compaction-design.md:426` records
that `manual/` prose drift is fixed "only `write-diataxis-manual` regeneration". A hand edit would diverge
the page from its generator, so the decision owed is whether to regenerate the page, hand-edit it and
accept the divergence, or build the missing guard first. `manual/log.md` separately concedes the bundle is
already out of sync with its own `synced_hash` values, and nothing outside `manual/` reads that field — a
declared re-sync mechanism with no in-repo verifier.

## reviews-0012-0013-ungated

- **Type:** 2
- **Verified:** yes — reopen verified 2026-10-14 first-hand, and the first framing was **wrong**.
  `ls reviews/.gating/` returns 0009, 0010, 0011 only, while `ls reviews/.claims/` returns 0009..0013:
  rounds 0012 and 0013 were claimed and written but **never gated**. `git ls-files reviews/` returns
  `README.md` and nine paths under `reviews/reviewed/` — every tracked round file is an *archived* one.
  Finding counts verified mechanically: 27 distinct `R0012-` ids and 11 distinct `R0013-`, matching each
  file's `### ` header count. `grep -rl -E 'R0012-|R0013-' docs crates CHANGELOG.md` exits 1 with zero hits.
- **Sources:** ticgit:56dd1140, reviews/0012.md, reviews/0013.md, reviews/README.md,
  docs/project/status.md (the 2026-08-08 owner decision)
- **First seen:** 2026-10-14 · **Last seen:** 2026-10-14

### Description

Two independent review rounds read `STATUS: COMPLETED` and carry **38 substantiated findings** — 27
(4 high) and 11 (2 High). Neither has been gated: `reviews/.gating/` stops at 0011, `reviews/README.md`'s
round table stops at 0011, and `CHANGELOG.md` records the last gate as covering rounds 0010 and 0011. No
`R0012-` or `R0013-` id is cited anywhere in `docs/`, `crates/` or the CHANGELOG. They have been in that
state since 2026-09-19/20.

### Background

**This entry was filed as Type 1 on a false premise and is corrected here.** The original reading was that
the two files are untracked "by omission, not by design", inferred from `git check-ignore` exiting 1, and
that the obvious unblocked step was to commit them before a `git clean -fd` destroyed them.

Not gitignored is not the same as not deliberate. `docs/project/status.md` records an **owner decision of
2026-08-08**: under `reviews/`, the registry and the archived rounds in `reviews/reviewed/` are tracked,
"only the in-flight round files and the `.claims/` / `.gating/` state are not." The tracked-file listing
shows exactly that shape, and the lifecycle it implies is claimed → written at `reviews/NNNN.md`,
untracked → gated → archived into `reviews/reviewed/NNNN.md`, tracked and registered, which is what commit
`cc20f4f` did for 0010 and 0011. Rounds 0012 and 0013 sit at the ungated step, so their untracked state is
the recorded design rather than a defect, and committing them in place would contravene that decision and
assert a lifecycle position they have not reached.

The exposure is real but it is an **accepted** consequence of that design: an ungated round lives
untracked, so it is lost if the working tree is cleaned. That trade was made on 2026-08-08.

What survives is a gating backlog, not a tracking defect — and it is Type 2 because the choice is the
owner's and nothing an agent can settle: run `indy-review-gate` over the two rounds and disposition 38
findings, or declare them not owed and record that in `reviews/README.md` so the next reader does not
rediscover them. Sizing matters to the choice: the gate over rounds 0010 and 0011 covered 188 findings and
produced the `[Unreleased]` decision block, so 38 is a substantial but far smaller piece of work.

### Options

**Gate them** *(source: the lifecycle `reviews/README.md` and `cc20f4f` establish)* — runs
`indy-review-gate` over both rounds, dispositions each finding, then archives them to `reviews/reviewed/`
and registers them so their ids resolve. The only route that produces evidence about the 38 findings.

**Declare them not owed and archive the rounds** *(proposed by reopen)* — cheaper and honest if the
findings have been overtaken; needs the decision written into `reviews/README.md`, or the next sweep
rediscovers two completed rounds nobody has dispositioned.

**Leave them ungated** *(the status quo)* — costs nothing today and keeps the 38 findings one `git clean`
from deletion, which is the exposure the 2026-08-08 decision accepted for in-flight rounds but which
compounds the longer a *completed* round sits at that step.

# Type 3 — Blocked

Type-3 entries retired from this register are kept verbatim in
`docs/backlog-archive.md`, each under its own name.

## phrasing-extension-list-for-custom-elements

- **Type:** 3
- **Verified:** yes — measured 2026-09-04 over 42 real pages (10.1 MB, 8,879 blocks, 346 custom-element
  start tags): **zero** mid-sentence splits
- **Blocked by:** evidence, not a decision. The re-trigger quoted below is a concrete external
  prerequisite — **a real page whose prose sits directly inside a container rather than in
  `<p>`/heading/`<li>`, whose sentences are cut by a custom element** — and the 42-page corpus did
  not contain that shape. Nobody can choose their way past it; either such a page turns up or the
  lever stays untaken. *(Re-typed 2026-09-09 from Type 2. It had been filed as needing a decision,
  but the classifier's own tie-break is 3 > 2 > 1 and this has a stated external prerequisite, so it
  was inflating the needs-a-decision count with something nobody can decide yet.)*
- **Sources:** ticgit:84bf37 (RESOLVED 2026-09-04 — the owner accepted the current behaviour), spec
  `2026-08-20-html-to-html-translation-design.md` §14.1 / §13.5, ADR-0025 (D4)
- **First seen:** 2026-09-04 · **Last seen:** 2026-09-04

> **Deferred at filing, 2026-09-04, by owner ruling.** `84bf37` was closed by **accepting** the current
> behaviour on the measurement above; this entry carries only the untaken lever, and it is recorded so the
> option survives outside the closed ticket. **Does not count against the v0.5.0 gate.** **Re-trigger: a
> real page whose prose sits directly inside a container rather than in `<p>`/heading/`<li>` AND whose
> sentences are cut by a custom element** — the shape the 42-page corpus did not contain.

### Description

An unknown element is DEFAULT-STOP (ADR-0025 D4), so a custom element used mid-sentence flushes the text run
before it and starts a new one after. In prose **not wrapped in an element** that severs one sentence into
`p-…` / `html-…` / `p-…` translation units. Wrapped in `<p>`, a heading or an `<li>` the wrapping element is
the block and the custom element is interior, so the sentence stays one block — which is why the measured
corpus shows zero splits. The candidate lever, if the naked case ever matters, is a per-run or per-profile
phrasing-extension list letting an operator declare `price-tag`, `fa-icon` and friends as phrasing.

### Background

The prohibition that constrains any design here: **unknown elements must not become PHRASING by default.**
DOMPurify removes an unknown element and every attribute riding it, which is why wave 6's 2026-08-21 ruling
puts such a block's anchor on a transparent `<div>` wrapper; treating one as inline phrasing would put its
text inside a run whose markup the sanitizer then deletes.

## adoption-agency-and-active-formatting-elements

> **Registered 2026-09-08.** The ticket was open at the v0.5.0 cut and the file's own census named
> it, but it had no entry of its own — which is the 2026-08-24 lesson (18 of 24 open tickets
> unindexed) repeating on a smaller scale.

- **Type:** 3
- **Verified:** yes — filed by DCR-0051 before pinning it, on `web/tests/html-oracle.spec.js`'s own
  instruction; the residual is measured, not predicted
- **Blocked by:** an explicit owner deferral taken at the v0.5.0 cut (ticket tagged
  `deferred-v050`). Re-trigger: a `sec4a:break-nested` count that stops shrinking, or a route that
  puts an attacker-chosen `data-sync-id` into a live DOM through a reconstructed formatting element.
- **Sources:** ticgit:525bef96, `docs/architecture/contracts.md` §4b, DCR-0051
- **First seen:** 2026-09-04
- **Last seen:** 2026-09-12

### Description

`transync-html`'s `balance_fragment` models no **list of active formatting elements** and no
**adoption agency algorithm**. `end_tag_effect` records why the omission is safe for the *closer*
the balancer appends — an extra frame makes it append a redundant closer rather than lose a needed
one — but that reasoning does not cover **reconstruction**: when a browser inserts the next element
it reconstructs the active formatting elements, so a `<b>` the crate has closed reappears as a real
frame, and that frame is what the appended closers then have to get past.

### Background

DCR-0051 collapsed the crate's two open-element stacks into one and closed five divergences,
taking the oracle's agreement from 7,697 to 8,619 of 10,000 and `sec4a:break-nested` from 1,358 to
**8**. Those 8 are this ticket, and `contracts.md` §4b records the residual as accepted rather than
unnoticed. `open@input:order` is a divergence mechanism the census had never carried before.

## noscript-read-as-markup

> **Registered 2026-09-08**, for the same reason as the entry above.

- **Type:** 3
- **Verified:** yes — deferred out of ti `bebebe` / DCR-0050 with a stated reason
- **Blocked by:** an explicit owner deferral taken at the v0.5.0 cut (ticket tagged
  `deferred-v050`), and a design question it inherits: `is_raw_text` answers the **name** question
  and leaves the parsing context to its caller, which is the discipline ti `48f3c6` and ti `2e2453`
  established. `noscript` is raw text only when the scripting flag is enabled, which is context —
  so admitting it means giving the scanner a notion of context it deliberately does not have.
- **Sources:** ticgit:16721e90, DCR-0050
- **First seen:** 2026-09-04
- **Last seen:** 2026-09-12

### Description

`RAW_TEXT_ELEMENTS` carries eight names since ti `bebebe`; `noscript` is the ninth and was left
out on purpose. But the scripting flag is enabled in every context this crate's output is parsed
in — a pane is mounted by `pane.innerHTML = DOMPurify.sanitize(…)` from JavaScript, and a bundle's
`source.html` navigated to directly runs scripts too — so Chromium reads `<noscript>` contents as
text where `scan_tags` reads them as markup.

### Background

The divergence is real and measured; what is unsettled is where the fix belongs, because the
honest answer requires either a context parameter or an exception to the name-only rule.

## nested-block-ast-splicing

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: dated owner-delegated decision (2026-08-06): invest at nested-editing time; no consumer needs the splice today. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** The regen splice walks only top-level source ranges; nested
  list/blockquote units ride parent payload coverage. SL-05/SL-06 name AST-aware
  recursive splicing as the successor — required before any finer-grained nested-unit
  editing.
- **Background:** `regen.rs` module-doc limitation; investigation bundle's
  constraints-and-debt table.
- **Blocked by:** decided 2026-08-06 (controller under owner delegation): invest **at
  nested-editing time**, not before — the splice has no current consumer that needs it,
  so the item waits for a nested-editing feature to be commissioned.
- **Sources:** ticgit:9b18ba45-c2f3-40de-b14d-b26097dca7b8
- **Last seen:** 2026-09-12

## deep-blockquote-fingerprint-depth (R0001-0008 residual)

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: OI-0022's own gate — “extend if drift is observed in practice”; no drift observed. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Blockquote structural fingerprints recurse one level; structure deeper
  than one level below a blockquote can drift without validator rejection.
- **Background:** Review 0001 finding R0001-0008 — a bare `R0001-` id is the live
  `reviews/reviewed/0001.md` per `reviews/README.md`'s citation convention. OI-0022 is itself
  RESOLVED (2026-07-13): it recursed the blockquote fingerprint exactly one level, and
  `reviews/reviewed/0001-dispositions.md` routes R0001-0008 back here as `backlog-tracked`. The
  `ListTopologyEntry.depth` widening (ticket `0d8277`) removed the u8 saturation ceiling,
  which is a different residual and does not deepen the blockquote recursion.
- **Blocked by:** OI-0022's own gate — "extend if drift is observed in practice"; no
  observed drift.
- **Sources:** reviews/reviewed/0001.md#R0001-0008, ticgit:9fdf78fe-59dc-44a0-9f7c-59ce7de2d8fb
- **Last seen:** 2026-09-12

## html-same-parent-segment-grouping

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: telemetry gate — build only if segment-count rejections dominate ValidationReport data. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Grouping same-parent HTML text segments via opaque placeholder tokens
  to reduce segment-count rejections; recorded fast-follow, not built.
- **Background:** DCR-0016 decision 6.
- **Blocked by:** telemetry gate — build only if segment-count rejections dominate
  ValidationReport data; no such data yet.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.
- **Sources:** ticgit:523b259e-60f7-4751-b946-63ac1a4e4b7f
- **Last seen:** 2026-09-12

## html-native-array-wire-field

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: telemetry gate — build only if double-encoding proves a top validation-rejection cause. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** An additive native-array wire field for HTML payloads as an
  inner-escaping escape hatch; recorded, not built.
- **Background:** DCR-0016 decision 7.
- **Blocked by:** telemetry gate — build only if double-encoding proves a top
  validation-rejection cause.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.
- **Sources:** ticgit:83816e44-73c1-45a7-808f-cc4c5c471822
- **Last seen:** 2026-09-12

## html-br-normalization-guard

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: DCR-0016's named revisit trigger, behind a telemetry gate. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** The byte-verbatim comparison guard is the named revisit point if models
  normalizing `<br>` vs `<br/>` becomes a top inline-rejection cause.
- **Background:** DCR-0016 revisit trigger.
- **Blocked by:** telemetry gate — no rejection-cause instrumentation evidence.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.
- **Sources:** ticgit:594b3e21-72b1-443b-a76a-5993b1271064
- **Last seen:** 2026-09-12

## per-kind-expansion-factors

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: DCR-0012's deferred refinement, behind an evidence gate. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Output-aware batching uses one global expansion factor (2.0) for all
  block kinds; per-kind factors (code ≈ 1.0) were deliberately deferred to keep one knob.
- **Background:** DCR-0012 deferred refinement.
- **Blocked by:** evidence gate — revisit only if code-heavy documents demonstrably
  over-split painfully.
- **Sources:** ticgit:3ff89400-9b75-4514-96f4-010ea06d2b7a
- **Last seen:** 2026-09-12

## intersection-observer-active-block

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: rides OI-0016's profiling gate, now ticketed as dd21ad59. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Whether to replace/augment the scroll-listener engine with an
  IntersectionObserver-based active-block pick.
- **Background:** DCR-0008 open judgment call (OI-0006 third action).
- **Blocked by:** deferred to a future performance pass — rode OI-0016's profiling gate, which **fired 2026-09-02**: `benchmark/scroll-frame/RESULTS.md` found no scroll-frame overrun (ti `dd21ad59` closed). Still deferred, now on that measurement rather than on a pending gate; the re-trigger is unchanged — a measured overrun on a large document.
- **Sources:** ticgit:5676a268-9d91-4074-ad65-60c304fee169
- **Last seen:** 2026-09-12

## dialect-trait

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: explicit YAGNI condition — a real second dialect must supply constraints first. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** A `Dialect` trait + front-end crates abstracting the parser beyond
  GFM/comrak; the syntax-crate split created the boundary but deliberately not the trait.
- **Background:** DCR-0005 Migration/Follow-up, reaffirmed 2026-07-13 and by the
  2026-08-04 syntax-split spec.
- **Blocked by:** explicit YAGNI condition — a real second dialect must supply constraints
  before the trait is designed; none exists.
- **Evidence note (ti `490d97` wave 7):** a second front-end now exists in-tree —
  `intake::html` beside the comrak parser, a *module* seam, not a trait; ADR-0025 (D3)
  explicitly rejected a format-axis crate split. The YAGNI condition should be re-read
  against ADR-0025 at the next sweep rather than assumed still unmet; nothing in the
  HTML wave required core to branch on a dialect capability.
- **Sources:** ticgit:8e383504-b95c-4471-9bec-6d8ed67726b7
- **Last seen:** 2026-09-12

## nested-anchor-scheme

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: the hierarchical-alignment revision is uncommissioned; no active demand. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** `sync_role: child-only` and `data-parent-id` are reserved wire values
  never emitted; the `Section` IR type is shape-frozen with no producer, reserved for the
  hierarchical-alignment revision expected to project it from the heading stack.
- **Background:** Archived OI-0005; reserved through DCR-0017 and DCR-0019 precisely to
  allow this without a schema break.
- **Blocked by:** the hierarchical-alignment revision has not been commissioned; no active
  demand.
- **Sources:** ticgit:3ba61128-597e-4b2f-a10e-2d80b64a06e6
- **Last seen:** 2026-09-12

## wasm-in-cli-bundle

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: ADR-0019's recorded size economics (~41× the bundle's whole JS payload). **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** The wasm renderer ships only as the standalone demo; CLI `--html-out`
  bundle integration is deliberately excluded.
- **Background:** Track C scope across DCR-0017/ADR-0019/DCR-0020.
- **Blocked by:** ADR-0019's recorded size economics — the module is ~41× the bundle's
  entire JS payload for capability the bundle already has; revisit only if that ratio
  changes dramatically.
- **Sources:** ticgit:21b47815-6968-44ab-8198-cb07af829a1b
- **Last seen:** 2026-09-12

## live-edit-reanchoring

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: contradicts architectural invariant 8; needs a baseline-level design revision. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Block-ID survival across structural source edits (insert/delete blocks
  with stable IDs) is unsupported; the wasm demo deliberately never restructures either
  document.
- **Background:** Draft non-goal NG4; mvp-scope DEFERRED table ("still deferred after
  2026-08-05").
- **Blocked by:** contradicts architectural invariant 8 ("document is assumed static") —
  needs a baseline-level design revision before any implementation.
- **Sources:** ticgit:48cfcb19-b61b-46ef-a839-7241ad92b7bd
- **Last seen:** 2026-09-12

## template-webcomponent-extraction

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: stated gate — “revisit only on demonstrated need”; none demonstrated. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Type:** 3
- **Description:** Text inside `<template>` elements (and web-component content) is
  bucketed with `script`/`style` and left untranslated (`transync-html`'s
  segment scan tracks `template_depth` specifically to exclude it).
- **Background:** DCR-0016 / 2026-08-03 design §9.
- **Blocked by:** stated gate — "revisit only on demonstrated need"; none demonstrated.
- **Cross-reference (ti `490d97` wave 7):** an HTML document's `<template>` is
  DEFAULT-STOP in the new intake — a `BlockKind::Html` block extracting zero segments,
  honest `PreservedZeroSegment` row — so the exclusion now has an anchor-bearing
  spelling too. Gate unchanged.
- **Sources:** ticgit:6a8dcca4-868a-4ea2-aefa-d5ba55e9d380
- **Last seen:** 2026-09-12

## no-machine-check-of-the-manual-cli-reference

> **RESOLVED 2026-09-02** (ticket `7e2fd068`, commit `7355008`) — the owner chose option (c):
> `crates/transync/tests/manual_source_refs.rs` resolves every frontmatter source path in the
> manual bundle. The **CLI-flag prose** class this entry describes is deliberately *not* welded by
> that choice, so if it needs closing it is a new ticket rather than this one. Also: the Background
> below calls `ticgit:0eee5c55` "still open" — it closed 2026-09-01, and `docs/backlog-archive.md`'s
> `manual-still-calls-serve-a-deferred-stub` entry records that resolution.


- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -c manual crates/transync-cli/tests/docs_cli_flags_drift.rs` returns 0
- **Blocked by:** a decision on whether the guard belongs on `manual/`'s generated output or on the generator that produces it
- **Sources:** ticgit:7e2fd068 (closed), ticgit:3ea4d3a5 (the flag-prose class the resolution left unwelded)
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`docs_cli_flags_drift.rs` guards `contracts.md` §6 and `docs/Developer_Guide.md` only. `manual/reference/user/en/cli.md` is a third full CLI reference with no machine guard.

### Background

It has already fallen two flags behind (`--cache-dir`, `--table-strategy`) and kept a whole `transync serve` section describing a deferred stub for days after the real server shipped, with nothing failing. `ticgit:0eee5c55` is that same drift, still open — so this gap has a demonstrated cost, not a theoretical one. The reason it is blocked rather than ready is that `manual/` is generated one-way from `docs/` by `write-diataxis-manual`, so a weld pointed at it would pin generated output: either the generator must guarantee the reference, or the weld must be placed at the generator rather than at its product. That is a design decision about where the guard belongs.

## markdown-island-reclassification

- **Type:** 3
- **Description:** A Markdown document's raw-HTML islands stay `BlockKind::Html` with
  `html-…` ids even where a semantic equivalent exists (an HTML `<table>` island is not a
  `Table`). Reclassifying them moves block **ids**, and with the ids go alignment rows,
  DOM anchors, and the `block_kind` cache axis — every consumer keyed on either.
- **Background:** ADR-0025 D11; spec §13.9.
- **Blocked by:** a schema-2.x-shaped window. The corpus-stability acceptance criterion
  (a document's ids do not move within a minor line) forbids it in any 1.x window, so
  this is not a "when there is time" item — it needs the window first.
- **Sources:** ticgit:cac88159-af11-45fa-8c79-56b28da4f51e
- **Last seen:** 2026-09-12

## html-oversize-leaf-block-split

- **Type:** 3
- **Description:** A giant `<table>` or custom element that exceeds the token budget has
  no splitter on the HTML path. The design commits to the **shape** only — a packing-time
  segment-window split in the DCR-0026 mold, never intake descent (D4 rejected
  budget-driven block sets) — and to the shipped `(Table × Html)` exclusion from the GFM
  row-window splitter. Watch item riding with it (spec §15.5): if regen ever splices
  merged multi-element ranges, `BlankLinePolicy::Keep` and slice-agnostic extract/splice
  hold (measured), but layer 3's inventory check must then run over the same merged slice.
- **Background:** spec §15.4 and §15.5.
- **Blocked by:** demonstrated need. D9's `li` grouping removed the most common trigger
  (long lists), and no oversize-leaf abort has been observed on the corpus.
- **Sources:** ticgit:4262bc4e-3c49-48a2-96f9-b802fa311aab
- **Last seen:** 2026-09-12

## batch-payload-duplication (OI-0051)

- **Type:** 3
- **Verified:** yes — Review 0010 findings (R0010-0091, R0010-0092), gate-accepted, user-routed track
- **Blocked by:** a sanctioned breaking window. Both actions change
  `TranslationBatch`'s public field set, which is exhaustive by policy; the
  v0.5.0 window closed at the `v0.5.0` tag on 2026-09-05.
- **Sources:** docs/project/open-issues.md#OI-0051, reviews/reviewed/0010.md#R0010-0091,
  reviews/reviewed/0010.md#R0010-0092, ticgit:98b66817-0e55-4290-af6d-68aa2bb0e388
- **First seen:** 2026-09-06 · **Last seen:** 2026-09-12

### Description

`TranslationBatch` carries **both** `glossary: Vec<GlossaryEntry>` **and**
`profile: ProfileMetadata`, and `ProfileMetadata` has a `glossary` of its own.
The one construction site in `unit::build_batches` fills both from the same
cohort, so the two are byte-identical by construction and a test pins exactly
that. Separately, `profile` is owned per batch, and its largest member is
`prompt_body` — the whole compiled system prompt, cloned once per batch even
though DCR-0027 §5's cohort memo compiles it once.

### Background

Filed LOW/MEDIUM. The allocation saving is modest; the reason to do it is the
two-places-one-opinion class this project spent 2026-09-01 through 2026-09-05
removing. `pipeline` builds the glossary-sensitive `CacheKey` from
`batch.glossary` while the text the provider sees is `batch.profile.prompt_body`,
compiled from `profile.glossary`. They cannot disagree today; the first write to
one without the other makes a cache key that describes a glossary absent from the
prompt — a silent wrong-cache-hit. `Arc<ProfileMetadata>` plus removing the
duplicate field makes that unrepresentable. The reviewer's stronger claim, that
cohorts are recomputed per batch, is wrong — DCR-0027 §5 memoizes them.

## provider-constructor-header-validation (OI-0052)

- **Type:** 3
- **Verified:** yes — Review 0011 finding (R0011-0022), gate-accepted, user-routed track
- **Blocked by:** a sanctioned breaking window — the clean fix adds a
  `ConfigError` variant to an exhaustive-by-policy enum. Owner-deferred until the
  v0.6.0 window opens, and explicitly may be deferred again at that point.
- **Sources:** docs/project/open-issues.md#OI-0052, reviews/reviewed/0011.md#R0011-0022, ticgit:72339140-8b70-4a25-848b-c05499036cd9
- **First seen:** 2026-09-06 · **Last seen:** 2026-09-12

### Description

`try_new` validates only that the API key is non-empty. Conversion to a
`HeaderValue` is deferred to request time, so a key containing a newline, a NUL,
or any other byte `HeaderValue` refuses passes the constructor whose purpose is
to reject bad configuration, and then fails on every request afterwards.

### Background

Filed MEDIUM; the gate's own triage raised it as the round's single escalation
candidate, because it is the one finding whose correct fix was blocked purely by
the release boundary rather than by any disagreement about the defect. For a
long-lived service, `try_new` is a startup check that does not check the thing it
exists to check. Note the neighbouring `with_timeout` question was decided the
other way (ADR-0031): a budget is taken verbatim because a bad value fails
immediately and legibly, whereas an unusable header is a permanent per-request
failure with no construction-time signal.

## intake-markdown-intake-name-collision (OI-0054)

- **Type:** 3
- **Verified:** yes — found by the 2026-09-08 documentation drift audit; the collision is in the
  code and the misattribution was in three documents
- **Blocked by:** a sanctioned breaking window, *conditionally*. `intake::markdown` is a `pub mod`
  of the published `transync-syntax`, so renaming the function is breaking for a direct consumer,
  and v0.5.0 closed the window. The first required action is **not** blocked: check whether any
  external caller needs the function at all, because making it crate-private would remove the
  surface question and may make the rename non-breaking.
- **Sources:** docs/project/open-issues.md#OI-0054, ticgit:cdadffea, DCR-0053, DCR-0035
- **First seen:** 2026-09-08
- **Last seen:** 2026-09-12

### Description

The Markdown NUL/nesting guard **function** is reachable as `crate::intake::markdown::intake` — the
path says "intake" twice, and the module and the function share the word for two different things.
DCR-0053 created the collision when it renamed `parser` to `intake::markdown`, and left the
function alone on purpose: renaming a public function is a surface change rather than a move, and
folding it in would have broken that change's "the diff must be only the rename" condition.

### Background

The entry exists because the collision was recorded in three places as "owned by OI-0034", and that
attribution is **wrong**. OI-0034 is *comrak's NUL→U+FFFD substitution desyncs byte columns*,
RESOLVED 2026-08-06 and archived. It owns the NUL normalization the guard performs — which is what
every `TRACE: OI-0034` in the code correctly cites — but it has never owned a naming decision, and
resolved-and-archived it cannot acquire one. So the collision had no live owner. It rides the same
unopened window as `LineOffsets::offsets` (R0010-0023) and the DCR-0053 rename: all three are
invisible through the `transync` facade and breaking only for a direct `transync-syntax` consumer.

## bundle-shell-narrow-viewport-and-pane-headings (OI-0053)

- **Type:** 3
- **Verified:** yes — Review 0011 findings (R0011-0083, R0011-0084, R0011-0087), gate-accepted, user-routed track
- **Blocked by:** owner-deferred until the v0.6.0 window opens, and explicitly may
  be deferred again at that point; the work then still needs a scope decision —
  whether the shipped shells target narrow viewports at all.
- **Sources:** docs/project/open-issues.md#OI-0053, reviews/reviewed/0011.md#R0011-0083,
  reviews/reviewed/0011.md#R0011-0084, reviews/reviewed/0011.md#R0011-0087, ticgit:463f5c8b-396f-4fed-8884-969aed7b5754
- **First seen:** 2026-09-06 · **Last seen:** 2026-09-12

### Description

Every shell transync ships lays its two panes out as `grid-template-columns: 1fr
1fr` with no media query and no container query, while emitting a
`width=device-width` viewport meta that promises the opposite. The panes carry
`aria-label` (R0008-0049 / R0008-0050) but no visible caption, so a sighted
reader has no on-screen statement of which pane is source and which is target.

### Background

Filed LOW ×3, registered as **one** entry on purpose: this is a single product
question — does the bundle shell target narrow viewports at all? — not three
defects. Answering it decides all three findings, and splitting them invites the
bundle shell and the demo shell to drift apart, which is the failure mode
`sync_js_drift.rs` exists to prevent for the engine. A narrow-viewport commitment
also implies a viewport dimension in the browser matrix, which is Desktop-Chrome-
only under ADR-0026 — check that ADR's re-open conditions before the work starts.

## v050-live-gate-unrun

- **DEFERRED by the owner 2026-10-13** — *“not particularly important, skip it.”* This is a
  recorded deferral with a trigger, not an entry left unclassified: the header's three-state rule
  admits counting, deferred-with-a-recorded-condition, or resolved, and an entry in none of them
  is a defect in this file.
  **Why the deferral is defensible rather than merely instructed.** Most of what this entry
  warned about was answered on the same day it was deferred. The gate ran and four legs passed,
  so the *code* is known to survive a real provider round-trip; and `smoke-live-gate.sh` gained a
  `load_key_file` helper that reads each credential from its gitignored root file when the
  variable is unset, which removes “no session exports it” — the mechanism behind three
  consecutive unattested cuts — for every leg at once. What is left is the narrow retrospective
  claim: the run attests the branch, not the released `v0.5.0` tree, and nothing depends on
  v0.5.0 carrying its own evidence.
  **Re-counts if any of these fire:** a release is cut from the `v0.5.0` tag rather than from
  master; a provider-level defect is traced to the v0.5.0 tree; or a cut lands with the
  DCR-0015 Part C gate triggered and unrun again, which would show the key-file helper did not
  fix the pattern after all.
- **Type:** 3
- **Type contested (reopen verification 2026-09-13):** the two lenses split on the same evidence.
  Lens A keeps type 3 on the tie-break 3 > 2 with the blocker line corrected; lens B holds type 3
  unestablished, because the key is present and credit plus network are merely *unverified* rather
  than known unmet — which leaves the run half on the owner's spend decision (type 2) and the
  checklist-preflight half ready today (type 1), giving type 2 by tie-break. This pass sides with
  lens B; the field above is left at the filed value for the owner to settle, and the work is the
  same either way. Full reasoning: the ticket's Verification (2026-09-13) section.
- **Verified:** yes — reopen verified 2026-09-12 first-hand: the checklist's own trigger-path diff
  `git diff --shortstat v0.4.0..v0.5.0` over the four trigger paths reports
  "10 files changed, 1311 insertions(+), 84 deletions(-)", and no live-gate result line exists in
  the `[0.5.0]` CHANGELOG section. **Re-verified 2026-09-13** by the adversarial reopen pass (two
  lenses plus a completeness critic): every quoted record and all of the trigger arithmetic
  reproduced, and three of the body's arguments were refuted and corrected — the consumer pins, the
  type-3 premise, and OI-0030's unticked box as an open obligation.
- **Blocked by:** *(all three original blockers cleared 2026-10-13)* the owner authorized the spend,
  the key was refreshed — the 2026-05-03 one on disk had expired and returned `401
  invalid_api_key`, which is the one thing the contested-type argument below assumed away — and the
  session had network. `scripts/smoke-live-gate.sh` also no longer reads only the environment: a
  `load_key_file` helper now picks each credential up from its gitignored file at the repository
  root when the variable is unset, so “no session exports it” has stopped being a blocker for every
  leg at once. What remains is **narrower than the entry's title**: the run attests the *branch*,
  not the released `v0.5.0` tree, so v0.5.0 itself still ships unattested and can only be given
  evidence by a run against the tag.
- **Sources:** ticgit:342e028d, docs/project/status.md, CHANGELOG.md `[0.5.0]` and `[0.2.0]`,
  docs/project/phase-state.yaml, docs/project/release-checklist.md (steps 8 and 10-12),
  docs/Developer_Guide.md, resp-translator/Cargo.toml, dynwebserver/Cargo.toml,
  crates/transync-openai/src/client/dispatch.rs
- **First seen:** 2026-09-12
- **Last seen:** 2026-10-13

### Description

The released v0.5.0 tree carries no evidence that it can complete a single translation round-trip
against a real provider. The gate that would produce it — `scripts/smoke-live-gate.sh`, created by
DCR-0015 Part C and resolving OI-0030 — is triggered for this range and was never run against the
tag. **Partly answered 2026-10-13**, and the distinction is the whole of what is left: the gate ran
on the branch and `chat`, `responses` and `google-vertex-express` all passed, so the *code* is
known to survive a real round-trip — but the tree it ran against is dozens of commits past
`v0.5.0`, so it attests a tree nobody released. The two routes below that were about *obtaining a
run* are therefore spent; the one about *which tree the run attests* is not. All three
evidence carriers say so rather than hide it: the `[0.5.0]` CHANGELOG preamble reads **"Release
gate (OI-0030 / DCR-0015 Part C): TRIGGERED and NOT RUN."**, `phase-state.yaml` says the same, and
status.md still lists it among what remains after the tag. The second half is structural rather
than per-release: nothing in the checklist establishes a usable key before the tag, so a release
can only be gated in a session that happens to have one.

### Background

This repository has no continuous integration, and every other test in the tree talks to a stub, a
loopback socket or a golden file — so the live gate is the only thing proving a genuine provider
response survives the whole validation stack. The range is the wrong one to leave unchecked: it
moved the prompt bytes the model reads, the unit payload and context assembly, and the OpenAI
adapter itself — the three places a live provider can disagree with the stub standing in for it.
v0.5.0 is the **third consecutive release cut** in this position after v0.3.0 and v0.4.0, and the
one counter-example has to be stated precisely or it reads as an exoneration: v0.4.0 was also cut
without the gate, but the gate ran and passed later the same day against a tree identical to the
release-prep commit but for two CHANGELOG link lines, so that run does attest to the released code.
v0.3.0's did not, and its remedy has since decayed — the `[0.3.0]` section still instructs running
the gate "against this tag", but no `v0.3.0` tag exists in this object store after the 2026-08-17
history restart, so that promise can no longer be kept. Three mechanical facts a picker needs, all
measured 2026-09-12 and re-measured 2026-09-13: the working tree is **31 commits past the tag**
(29 at filing; the two additions are docs-only) with the trigger paths moved again (11 files, 149
insertions), so a run on the branch attests to a tree nobody released; the wrapper script itself
gained 69 lines after the tag, so the tagged copy's key checks are two bare `-z` emptiness tests
(lines 58 and 64) rather than `require_key`; but
the tagged test file is behaviourally identical to the branch's, its only post-tag change being the
`parser` → `intake::markdown` module rename, so a run at the tag asserts exactly what a branch run
would. The filed reason for type 3 — that the missing thing is neither a decision nor code — did
not survive verification: the key is on disk, so what is left is the owner's choice plus two
unverified conditions, and the classification is contested between 3 and 2 (see the Type contested
line above).

### Options

**Run the gate now against the tagged tree and record date and both model names in the `[0.5.0]`
entry** *(source: `docs/project/release-checklist.md` step 12)* — the only route producing evidence
about the released code. Effort is one command plus one dated CHANGELOG note; blast radius is a
released section, which the record rules say keeps its words and takes an appended dated note
rather than a rewrite. Costs real tokens against the owner's account.

**Fold it into the next release cut and annotate `[0.5.0]` as permanently unrun** *(proposed by
reopen)* — cheapest, and honest about what happened; it produces no evidence about v0.5.0 ever, and
both consumers are already building against that tag — all **seven** dependency edges moved to
`tag = "v0.5.0"` on 2026-09-05 (dynwebserver `9ea9711`) and 2026-09-06 (resp-translator `d104793`),
recorded in release-checklist step 8 — so they inherit the unattested tree today rather than later.
The filed body and this entry both said they still pinned `v0.4.0` on five edges, restating
status.md's dated "(verified 2026-09-05)" sentence as current; refuted 2026-09-13 against both
manifests.

**Make a usable key a precondition of the tag, so the gate blocks the cut** *(proposed by reopen)* —
the only route that changes the *next* release's behaviour and the only one addressing the pattern
rather than the instance. Effort is an edit to one living document; the checklist's own note says
editing it needs no DCR, and a new step must take a letter rather than a number because its numbers
are cited from outside. Composes with the first route rather than replacing it.

**Narrow the trigger paths so this range would not have fired** *(proposed by reopen)* — makes the
problem go away by redefining it, and forecloses the evidence permanently; it is the only route
that weakens a gate, and would amend DCR-0015 Part C.

**Run the two OpenAI legs against an OpenAI-compatible endpoint via the base-URL override**
*(proposed by reopen, research pass 2026-09-12)* — the script already validates
`TRANSYNC_OPENAI_BASE_URL` as an absolute http/https URL, so the plumbing exists. Distinguishing
point: it is the only route that can produce a real refusal without an OpenAI account, and the only
one whose evidence is about a different provider than the one the release names. One mechanic added
2026-09-13: `client::api_for_model` picks the surface from the model *name*, so the `responses` leg
is a second Chat Completions run unless the override name matches its heuristic (any `o<N>`, or
`gpt-<N>` with N ≥ 5, and no `chat` substring — DCR-0059) and the endpoint implements
`/v1/responses`.

**Name the existing local key source in the ritual, so a cut session can export the key the machine
already has** *(proposed by reopen verification 2026-09-13)* — the cheapest route and the only one
that treats "no API key was available in the release session" as a question the ritual never asked
rather than a resource it never had, which is the same shape step 2a was inserted for. Effort is one
edit to `release-checklist.md` step 11, which already shows `OPENAI_API_KEY=…` with a placeholder
and never says where the value comes from; blast radius is one living document and no code, and
`require_key` already rejects a padded or control-character-bearing value. It produces no evidence
about v0.5.0 and no block on the next cut, and naming where a key lives establishes neither credit
nor authorization to spend.
