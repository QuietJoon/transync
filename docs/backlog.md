# Backlog — WIP / deferred / open items

Maintained by the `/reopen` sweep. Last sweep: **2026-08-26**; last full update:
**2026-08-26** (the review-0009 track register). Three sweeps have run since the
2026-08-07 refresh this line used to date itself from, and each one's markers are dated
in place on the entries it touched: **2026-08-24** (`e89b537` — the index was missing 18
of 24 open tickets), **2026-08-25** (`7a1eaa0` — twelve deferrals audited, and the census
below became answerable) and **2026-08-26** (`e663422` — ten entries for thirty-six
review-0009 findings: the thirty-five the owner routed `track`, plus R0009-0014).
That 2026-08-07 refresh was derived from
`ti list --all` and `git log`, not from this file's own previous text — the review-0001
arc produced several tickets whose root cause was one document restating another, and the
index must not become one. Every sweep since has held to that rule.
Sources: `docs/project/`, `docs/decisions/`, `docs/architecture/`, DCRs, specs, code
markers, `docs/investigation/` (untracked and git-ignored since 2026-08-08 — present on
disk and read as a source, but not version-controlled) and `reviews/` (partly tracked:
round 0001 and `README.md` are committed, later rounds are left untracked until the gate
archives them; see `untracked-analysis-bundles` below).

**The v0.5.0 release gate — the rule this file is the census for (owner, stated
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
- **The window is not the gate.** `0.5.0-dev` is the open breaking window (`ff788f7`,
  2026-08-24); this condition is what closes it. Nothing is released from an open window.

**Why the rule lives here.** The 0.4.0 window was declared used-and-closed while six
wave-0/1 finding tickets sat unindexed — including `ticgit:e77173bb`, a *live*
`contracts.md` §4a break — and the 2026-08-24 sweep then measured that failure properly:
**18 of the 24 open tickets were missing** from the very file `docs/project/status.md`
calls "the index of open items". A gate phrased over "every open issue" is worth exactly
as much as the census that answers it, which is why every entry below has to be in one of
three states — counting, deferred-with-a-recorded-condition, or resolved/historical — and
why an entry that is in none of them is a defect in this file, not an ambiguity.

**Census (2026-08-26).** Of the **64** entries below: **34 count against the gate**,
**23 are explicitly deferred with a recorded condition**, **7 are resolved or historical**.
The 2026-08-25 deferral audit (`7a1eaa0`) established **24 / 23 / 7** over the 54 entries
that existed then; the ten review-0009 entries `e663422` added the next day
(OI-0039..OI-0048) are all open work, and are the whole of the difference. Re-derive these
figures on each sweep rather than trusting them — an entry counts unless it carries a
resolution or a dated deferral line. The release-side half of this rule is
`docs/project/release-checklist.md` step 2a, which is where a release actually consults
the census.

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

**TicGit state (2026-08-08, after the Review-0002 gate):** 91 tickets, 78 closed,
**thirteen open**. Those thirteen are the six commissioned roadmap tickets in Type 1
(`fc0304`, `43cfb4`, `d3acc3`, `b791d6`, `bda471`, `f12b8b`), the three ticketed design
questions in Type 2 (`40e2a5`, `66339b`, `1347b4`), and four filed against the API
surface and its consumers: `1a85f3` (`TranslatorError::Other` conflates five terminal
causes), `43331a` (deadline/cancellation on `Translator` — DCR-0009's stated revisit
condition has fired now that a long-running service consumer exists), `2d8a9a`
(release-checklist step 8 knows only one path consumer), and `92abf5` (a rolled-back run
can still remove an empty directory a peer just locked). Two Type 2 entries carry an
`OI-` id rather than a ticket: the gate registers documents only, so they wait for
`reopen`'s selection gate.
Of the 34 tickets tagged `review-0001`, 33 are closed; the one survivor is Type 2's
`40e2a5`.

**TicGit state superseded (2026-08-24, `e89b537`).** The snapshot above is kept as the
2026-08-08 record and is not rewritten; it is no longer the current count. The 2026-08-24
sweep found **24** open tickets, all 24 now indexed here — 13 Type 1, 8 Type 2, 3 Type 3 —
and filed three more entries (OI-0037 `148fcf83`, OI-0038 `30a744f1`, OI-0016 `dd21ad59`).
Correlation between the two registers depends on the `ticgit:<8-hex>` spelling, not on
`` ti `xxxxxx` ``: an entry written the short way is re-filed as untracked by the next
sweep. Read the census above for the current answer to the release gate; read `ti list
--all` for the current queue.

Types:

- **Type 1** — not blocked and needs no prior decision/discussion; can be picked up as-is.
- **Type 2** — needs a decision or discussion next (posture, scope, or design choice).
- **Type 3** — blocked, with the blocking reason stated.

---

## Type 1 — actionable now

### commissioned-roadmap-post-0.3.0 — **ALL SIX LANDED (2026-08-09/10)**

- **Description:** Six feature-scale work items the owner commissioned on 2026-08-06
  (decisions delegated to the controller and recorded in each ticket, tagged
  `owner-decision,roadmap`, priority 5). Each was actionable as-is. Four opened with a DCR
  before code per repo convention — `fc0304` and `bda471` with SL slice numbers too —
  while `d3acc3` and `b791d6` were already decided and started at implementation.
  **All six have since landed**, the last on 2026-08-10, so nothing in this entry is open
  work; it stays here as the roadmap's record. Recorded order, each with what it shipped:
  1. `fc0304` — the oversize row-window splitter (STUB-017 cluster): no single unit may
     abort a run. Scheduled FIRST. **Landed 2026-08-09** (DCR-0026, slices
     SL-100..SL-105): every obligation its DCR owed is discharged. Row-window batching
     carries header context — `unit::split` replaces an oversize table, in place and in
     document order, with windows that are each a complete GFM table holding the real
     header and delimiter rows, so invariant 3's ban on isolated cells holds by
     construction. The split is **packing, not retrying**, and happens exactly once
     before round one, which leaves ADR-0009's verbatim-resubmission budget untouched
     rather than amended. `regen::regenerate_table` is the plug-in point the windows
     reassemble through, and `pipeline::merge` runs between result aggregation and
     regeneration so a window id never reaches `regen::regenerate`, the alignment map or
     the DOM. Oversize *non-table* units are excluded, each with its ground stated in the
     record. Of the two advisory knobs, `default_table_strategy` became **effective**
     (the shipped default profile carries `row-window-first`) and `max_split_retries` was
     **removed** — a deterministic packing-time split has no split retry to bound, and a
     profile still carrying it now gets the ordinary unknown-key load warning. SCN-03
     stopped being stub-only: `scn_03_table_large.rs` drives the live path and asserts
     that two or more windows were dispatched, that they reassemble into one 200×4 table,
     and that the alignment map holds exactly one `Translated` row for it and no window
     id. STUB-017 closed with it.
  2. `43cfb4` — section-aware batching. Scheduled SECOND. **Landed 2026-08-09**
     (DCR-0027, slices SL-106..SL-109): batches are section-coherent,
     `[[glossary]].scope = "section"` is accepted end-to-end with a `sections`
     selector, and its `ProfileError::Unsupported` rejection retired at all three
     gates — which retired the Type-3 `glossary-section-scope` item in the same
     motion, as commissioned.
  3. `d3acc3` — the sync-engine pass. Scheduled THIRD. **Landed 2026-08-09:**
     single-file packaging re-affirmed (recorded in the module doc-comment and
     DCR-0008's dated note), ID-identity pairing documented as a normative
     schema-1.x invariant and *enforced* — a map whose row contradicts it is
     refused, which is also the `R0003-0002` resolution — and all three reflow
     hooks (ResizeObserver on both panes, `document.fonts.ready`, image
     `load`/`error`) built, re-collecting the anchor sets and re-driving the
     follower. OI-0015 and OI-0024 both close, and the Type-3
     `oi-0024-reflow-hooks` item is retired below.
  4. `b791d6` — `transync serve` becomes a real loopback static server (STUB-061
     un-deferred). Scheduled FOURTH. **Landed 2026-08-09:** the server binds
     `127.0.0.1:7470`, serves `GET`/`HEAD` only, confines every request to the served
     root in two layers (per-segment percent-decode, then canonicalize-and-contain,
     which is the one that sees a symlink escape), types responses from a fixed
     extension table, and shuts down cleanly on Ctrl-C/SIGTERM. Both statements it was
     scheduled to retire are retired — the exit-5 contract in `contracts.md` §6 and the
     "deferred stub — nothing binds" paragraph the Developer Guide's CLI reference
     gained in `a5ffe50` — along with STUB-061 itself and every `python3 -m
     http.server` the smoke paths reached for. `scripts/test-browser.sh` now serves
     SCN-13 with it, so the browser suite is evidence for the shipped server.
  5. `bda471` — `transync-anthropic` commissioned as the second provider crate
     (anthropic first, local-llama later); its landing supplies half of the Type-3
     `provider-capability-set` revisit condition. **Landed 2026-08-10** as DCR-0029,
     slices SL-115..SL-119: a rostered, publishable member implementing `Translator`
     over the Messages API, with construction-time configuration identity, a
     required output ceiling, a schema-profile pass for the provider's narrower
     dialect, an honestly-mapped stop-reason table, and a keyless end-to-end test
     that drives `transync::translate` over a loopback socket. One taxonomy variant
     rode along — `TranslatorError::ContextWindowExceeded` — and the core trait did
     not move, which is ADR-0002's promise exercised rather than merely restated.
     The `provider-capability-set` entry below is marked awake accordingly. The live
     leg is written and gated but **never executed**: there is no `ANTHROPIC_API_KEY`
     in this environment.
  6. `f12b8b` — the disk-backed cache design pass; its DCR must resolve OI-0017's one
     remaining item (detected-language persistence) explicitly rather than re-defer it,
     unblocking that Type-3 item. Its scope also absorbs three Review-0003 findings the
     gate routed here on 2026-08-09 rather than fixing standalone, because this pass owns
     eviction semantics and would re-decide any interim answer: `InMemoryCache` has **no
     capacity bound** (R0003-0048 — the routed one), its lookup cost (R0003-0049) and its
     key representation/interning (R0003-0051) are the same conversation, and R0003-0050
     was already a direct duplicate of this commission.
- **Background:** All six originate from the 2026-08-05 sweep's Type-2 section; the
  owner's 2026-08-06 ruling converted them from "needs decision" to "commissioned,
  scheduled post-v0.3.0". **All six have landed and all six are `closed` / `resolved` in
  TicGit** (re-checked 2026-08-16 with `ti show` on each id) — the entry survives here as
  the roadmap's record, not as open work. The "none has been started — all six are
  `state: new` and unassigned" note this line used to carry described the 2026-08-07
  refresh and was already stale by 2026-08-10.

_The 2026-08-05 sweep's Type-1 work is fully resolved: its nine items landed 2026-08-05/06
(`0f137ed`..`3c9bc15`). The review-0001 arc that followed ran waves C–H over 2026-08-06/07
and closed out on 2026-08-07 with a documentation-accuracy tail — nine tickets fixing docs
that asserted what the code or a neighbouring document did not say, one cache/CLI fix
(`aa21e8e`, the `--source-language AUTO` sentinel), and the git-object-loss incident
`7a7feb`, repaired and then parked by the owner. v0.3.0 was cut at `30d467c` and tagged
locally only — no remote, nothing published — so post-release CHANGELOG material goes under
`[Unreleased]`. `reviews/reviewed/0001-dispositions.md` carries the per-finding table for the 52
findings of `reviews/reviewed/0001.md`; `ti list --status closed --tag review-0001` enumerates the
tickets; `CHANGELOG.md` carries the per-ticket entries under `[0.3.0]` and `[Unreleased]`._

### html-walk-foreign-content-breakout — **`e77173`, a live §4a break, and wave 3's precondition**

- **Type:** 1
- **Verified:** yes — measured 2026-08-23 through the shipped DOMPurify mount by the
  wave-1 fix review, and again by the 2026-08-24 design review.
- **Sources:** ticgit:e77173bb, `docs/architecture/contracts.md` §4a (the carve-out),
  `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`
  (2026-08-23 amendment)
- **First seen:** 2026-08-22 · **Last seen:** 2026-08-24

#### Description

`walk_elements` models neither HTML's foreign-content **breakout** tags nor its
integration points. `<div class="wrap"><svg><div>x</svg></div>` followed by a block
mounts that block's anchor **inside** the wrapper rather than as a direct child of
`<main>` — the §4a rule the engine's `offsetTop` math rests on. Reachable from
untrusted source Markdown through a type-6 html block, which invariant 7 says to
assume hostile.

**The self-closing slash is not the mechanism.** The *unflagged* `<svg><div>x</svg>`
does the same thing: `div` is on HTML's breakout list, so a browser pops the `<svg>`
at the `<div>` and continues in HTML content, while the walk closes it at `</svg>`
and passes the fragment through. So this is not wave 1's fix leaking — it is an older
gap that fix neither introduced nor worsened (`balance_fragment` output is
byte-identical old-versus-new for every one of these inputs).

#### Background — why it is scheduled rather than parked

The ticket's own words put the fix-or-accept decision **before wave 3 lands**, and the
2026-08-24 design review found it had no owner: `e77173`, "breakout" and
`foreignObject` appear **zero** times in the wave 3–7 plans, in `status.md`, in
`open-issues.md`, and in this file. It lived in three places, none on an execution
path — which is what this entry exists to end.

Two waves are exposed. **Wave 3** consumes `element_extents` — the same walk — and its
deviation 6 makes the extents binding on phrasing-run boundaries, so on breakout inputs
the intake draws block boundaries a browser disagrees with, blesses an identity corpus
over them, and a later fix then re-derives block sets, **ids and section paths**. Ids
are this project's only sync currency; re-deriving them is not a code change. **Wave 6**
mounts every HTML-document block through the same balance pipe, and wave 7's browser
gate has no case that would catch it.

**Disposition: fix it, between wave 2 closing and wave 3 starting.** It is a live
correctness break today, its cost grows discontinuously at wave 3's blessing, and
ti `48f3c6` records that wave 1's stack-scoped foreign bit makes the modelling cheap
now that the bit exists — the expensive prerequisite is already paid. Not folded into
wave 2: that is the breaking window, and an orthogonal walk defect does not belong in
its diff. Wave 1's `transync-html` fix is the precedent for a scoped,
separately-reviewed fix between waves.

The ticket suggests the corpus entry that would have caught it:
`("selfclose-breakout", "<div class=\"w\"><svg><div/>x</svg></div>")` — the balanced
golden corpus has no foreign-content breakout entry at all, which is why five
`selfclose-*` entries could not see this.

### transync-html-wave-0-1-findings — five tickets from the wave 0/1 reviews

- **Type:** 1
- **Verified:** yes — each reproduced by the review that filed it.
- **Sources:** ticgit:95f55b3a, ticgit:415cdb7f, ticgit:4882ac7d, ticgit:2e2453dd, ticgit:48f3c6d8
- **First seen:** 2026-08-22 · **Last seen:** 2026-08-24

#### Description

| ticket | finding | class |
|---|---|---|
| `95f55b` | `balance_fragment` appends a closer that lands **inside** an unterminated trailing comment/CDATA/tag, so a re-balance adds another. 15,726 violations / 200k fuzz iterations | pre-existing, widened by wave 1's fix; **latent** — balance runs once on the render path, and the mount outcome is identical either way |
| `415cdb` | `collect_reserved_attr_spans` over-deletes across a stray `=`: `<div =data-sync-id="x">y` → `<div =>y`, moving a **non-reserved** attribute | pre-existing; over-deletion, so the impostor-anchor property holds (51-case probe, zero under-deletions) |
| `4882ac` | `image` is a documented `VOID_ELEMENTS` exclusion with no test or golden anywhere | test gap; the decision is held only by prose |
| `2e2453` | `scan_tags` enters raw-text state for `script`/`style`/`textarea`/`title` even inside `svg`/`math`, where a browser does not | scanner-level, pre-existing |
| `48f3c6` | a void name used as a real foreign element keeps `is_void` globally | deliberate trade, recorded |

#### Background

These were filed rather than fixed because each is pre-existing and orthogonal to the
wave that found it. `415cdb` additionally **contradicts a shipped contract sentence** —
`contracts.md` §4's "no element name and no other attribute moves" — so it now carries a
carve-out beside that sentence, the way §4a's breakout carve-out was added. `48f3c6` and
`2e2453` are carve-outs the same foreign-content fix as the entry above would naturally
sweep in, which is a reason to schedule that fix rather than five separate ones.

---

---

### git-object-loss-residual-blobs

- **Type:** 1
- **Verified:** partly — reopen verified 2026-08-24: `git diff --stat HEAD` now succeeds; the two named blobs still answer `could not get object info`
- **Sources:** ticgit:494a754c
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

Two blobs the ticket names are still unreadable in `.git`, but they are no longer HEAD blobs and the symptom the ticket is titled after — `git diff` over the whole tree aborting — no longer reproduces.

#### Background

Filed when a plain `git diff --stat` aborted with `fatal: unable to read 1b7a866f…`. The 2026-08-17 restart at `59ce8df` re-created those paths as fresh blobs, so the unreadable objects became unreferenced debris rather than tree content. Nothing at HEAD depends on them and `git fsck` is clean. It is type 1 because the remaining action is a decision-free re-scope or close, not a repair — there is nothing left to recover, and the objects cannot be reconstructed. Left open rather than closed here because this command never closes a ticket it did not file. The related standing record is `ticgit:6b43008a`.

### developer-guide-names-two-of-three-browser-specs

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:729ec8ec
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`docs/Developer_Guide.md` describes the browser suite as SCN-13 plus the wasm demo, but `web/tests/` holds a third spec — `engine.spec.js`, `sync.js`'s mount contract driven over a bare two-pane rig — which the same runner executes.

#### Background

`playwright.config.js` sets `testDir: "./tests"` and `scripts/test-browser.sh` ends in a bare `pnpm exec playwright test`, so all three specs run. The weld `docs_browser_suite_drift.rs` requires living documents to name the spec files rather than publish a count, and it can only pin what the document names. A spec the guide omits is therefore a spec no weld protects: it could be deleted and both the document and the test would stay green. The fix is a one-line documentation edit plus confirming the weld then covers all three. Type 1 — nothing to decide, no prerequisite.

### playwright-outside-the-runner-validates-a-stale-bundle

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:ed2e73c9
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

A bare `pnpm exec playwright test` from `web/` skips the rebuild that `scripts/test-browser.sh` performs, so it validates whatever bundle the last full run left behind.

#### Background

The script rebuilds the stub CLI, regenerates every bundle and rebuilds the wasm module on each invocation. The inner loop is documented in the wave plans as a legitimate speed-up with the caveat that any pass must be re-confirmed by a full run — but nothing enforces the caveat, and a green inner-loop run looks identical to a green full run. Pre-existing and suite-wide; surfaced by the adversarial review of wave 1 Task 2. Type 1 because the shape of the fix is settled: make the fast path announce what it did not rebuild, or make it refuse when the bundle is older than its inputs.

### cancellation-tests-assert-wall-clock-bounds

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:d4178267
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

Two tests in `crates/transync/tests/cancellation.rs` end in a 5-second wall-clock assertion against 30 s and 600 s regression signatures, and flake under load.

#### Background

The bound exists to prove a sleep was raced against a cancellation token rather than waited out. It is a real property worth pinning, but wall-clock is the wrong instrument on a machine where this session measured `syspolicyd` parking fresh binaries at `_dyld_start` for 30–40 minutes and five foreign cargo processes contending for one target dir. A flake here reads as a cancellation regression, which is the most expensive possible false positive: it points an investigator at the retry policy. Type 1 — the fix is to assert on the observable (the token was observed, the provider was dropped) rather than on elapsed time.

### scratch-scripts-fall-back-to-tmpdir

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:13a73be0
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

Five scripts prefer `/Volumes/Temp/claude/<name>` and silently fall back to `${TMPDIR:-/tmp}/<name>` when that parent is absent.

#### Background

`test-browser.sh`, `smoke.sh`, `smoke-live.sh`, `smoke-live-long.sh` and one more all carry the same two-step rule. The workspace's standing rule is that every temporary artifact lives under `/Volumes/Temp/claude/` and that an unreachable volume is a stop-and-ask, not a fallback — precisely because a silent fallback puts build artifacts somewhere nobody is looking and nobody cleans. The fallback also makes the failure invisible: a run that should have halted instead succeeds against a different directory, and the operator learns nothing. Type 1 — replace the fallback with a refusal that names the missing path.

### transync-html-token-pin-reads-another-crates-fixtures

> **RESOLVED 2026-09-04 (`ticgit:4fb858`).** Option 1 — the corpus is vendored into the crate: `crates/transync-html/tests/fixtures/{scn-15-html-blocks,scn-14-full,reader-honesty}.md` are byte-verbatim copies, sha256-verified, so the pin no longer reaches through `CARGO_MANIFEST_DIR/../..` into a sibling crate. It was also the only admissible option: `publish = false` contradicts wave 0's roster decision, and the feature-gate variant this entry recorded is **barred outright** — `transync-html` may never carry a `[features]` table (CLAUDE.md, and the crate's own manifest header), because that would break the standing two-package `wasm32` gate. The ticket's own hold ("do not act during ti 490d97 waves 0-7 — the fixture freeze holds for the duration") expired when wave 7 landed.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:4fb85819
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`crates/transync-html`'s `token_stream_pin.rs` reads fixtures from `crates/transync/tests/fixtures/` via `CARGO_MANIFEST_DIR/../..`, which a published tarball does not contain.

#### Background

The crate carries no `publish = false`, so it sits on the publication roster. A tarball ships the crate's own `tests/` but not another member's fixtures, so the pin cannot run from it. Nothing breaks today only because nothing has published from a tarball. The second consequence is worse in daily use: when the fixtures move, the pin fails with a file-not-found that reads like a corpus regression rather than a path problem, so the red misdiagnoses itself. Type 1 — either vendor the fixtures the pin needs into the crate, or gate the pin behind a feature that a tarball run does not enable.

### sync-role-for-catch-all-would-mis-classify-a-new-kind

- **Type:** 1
- **Verified:** yes — gated finding register; **wave 2 Task 6 demonstrated it live**
- **Sources:** ticgit:9ffb97ee
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`align::sync_role_for` ends in `_ => SyncRole::Anchor`, so any `BlockKind` nobody named silently anchors — and `SyncRole` is wire-visible, deciding whether a row gets a DOM anchor and whether the engine counts it synchronizable.

#### Background

This is no longer a hypothesis. Wave 2 Task 6 added `BlockKind::Title` and captured the catch-all swallowing it: the trap test failed with `left: Anchor, right: NonSync` before the explicit arm was written. That red is the ticket's claim, reproduced. Wave 2 worked around it deliberately rather than fixing it, because removing a catch-all is a change to every kind's dispatch and did not belong in a breaking-window wave. The related and sharper instance is `ticgit:18b9c34b`, where the renderer decides the same question from a literal match instead of asking this function at all. Type 1 — the approach is settled: enumerate the arms and delete the catch-all, letting the compiler demand a decision per variant.

### render-block-decides-anchoring-without-asking-sync-role-for

- **Type:** 1
- **Verified:** yes — reopen verified 2026-08-24: `grep -c sync_role_for crates/transync-syntax/src/render.rs` returns 0
- **Sources:** ticgit:18b9c34b
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`render_block` enforces the non-sync-implies-no-anchor rule with a literal `matches!(kind, BlockKind::ThematicBreak)` rather than by consulting `sync_role_for`, so a `Title` block would get a DOM anchor its own alignment row denies.

#### Background

Verified: the renderer never calls the role function at all. Unreachable at wave 2's HEAD because nothing mints a `BlockKind::Title` yet — the Markdown intake cannot, and the HTML intake that will is wave 3's — so wave 2's suite is green on the merits rather than by luck. Wave 3 mints titles and wave 6 builds the HTML panes, which is where such a block first reaches a renderer on a live path. Wave 6 must extend the guard or state why a literal list is the right shape. Deriving it is the better fix and matches what wave 1 already did one layer up, when it made `synchronizableRowCount` derive from `synchronizableRowIds(...)` precisely so the mount predicate and the anchor predicate could not drift. Type 1 — the approach is settled and the deadline is wave 6.

### scan-tags-tag-name-and-end-tag-open-divergences

- **Type:** 1
- **Verified:** yes — gated finding register; owner ruled both in scope for a later task
- **Sources:** ticgit:e20490fe
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

Two deliberate divergences from HTML survive in `scan_tags`: one plants a real attribute the strip never examines, the other invents structure a browser never mints.

#### Background

Wave 0 task 8 (`ti 549b20`) aligned `AttrState::Outside` with the browser and closed a measured strip bypass. The owner's 2026-08-21 ruling named these two as out of scope for that task and asked for them to be filed together, because they share a fix. Both are on the untrusted-source path that architectural invariant 7 says to assume hostile, and one is explicitly security-shaped: an attribute the strip never examines is an attribute the strip cannot remove. Type 1 — the owner already ruled, so nothing is undecided; what remains is the work. Related foreign-content gaps that the same stack-scoped bit would sweep in: `ticgit:2e2453dd`, `ticgit:48f3c6d8`, `ticgit:e77173bb`.

### manual-still-calls-serve-a-deferred-stub

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:0eee5c55
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`manual/` still describes `transync serve` as a deferred stub that never binds, two weeks after the real loopback static file server shipped.

#### Background

`transync serve` became real on 2026-08-09 (ticket `b791d6`); `contracts.md` §6 records that it supersedes the deferred-stub contract that stood from SL-13. The `docs/` bundle moved with it. `manual/` is derived one-way from `docs/` and did not. Found while working the exit-code tables, whose scope was table content only — this needs whole sections rewritten, which is a different job. The reason it went unnoticed is `ticgit:7e2fd068`: no weld machine-checks the manual's CLI reference, so a section can describe software that no longer exists and nothing fails. Type 1 — the correct text already exists in `docs/`; this is a derivation the manual pipeline owes.

### provider-payload-intake-guard (OI-0037)

- **Type:** 1
- **Verified:** yes — Review 0003 findings (R0003-0004, R0003-0005, R0003-0088), gate-accepted, user-routed track
- **Sources:** ticgit:148fcf83, `docs/project/open-issues.md#OI-0037`
- **First seen:** 2026-08-09
- **Last seen:** 2026-08-09

#### Description

Source Markdown enters through `parser::intake` and its depth pre-scan
(ticket `07844d`); provider-returned payloads call `comrak::parse_document`
directly at five sites (`validate/fragment_reparse.rs`, `validate/inline.rs`,
`validate/per_kind.rs`, `structure.rs`, `validate/full_reparse.rs`), so the
ceiling that exists because stack exhaustion is an uncatchable abort governs
what the user wrote but not what the model returned. Route provider payloads
through one guarded entry point and convert a refusal into `ReparseFailure`
with attributed fallback ids, so the existing retry-then-fallback machinery
carries it.

#### Background

Type 1, not 2: the approach is settled (one guarded entry point, refusal ->
`ReparseFailure`) and nothing has to land first — it is tracked rather than
fixed only because it spans five call sites and has **no reachable failure
today**. The reviewer's stack-exhaustion impact was refuted by the measured,
regression-pinned record in `parser/depth.rs`: comrak 0.27's block parse is
iterative, every untrusted-tree walker here was made iterative for exactly
this path, and memory is linear in the 32 MiB response cap. What survives is
that the safety rests on comrak internals rather than on anything this
repository asserts — a dependency upgrade could reintroduce a recursive block
parse with nothing going red. R0003-0088 is the test gap and cannot land
before the guard.


### pre-network-recomputation (OI-0041)

> **RESOLVED 2026-09-04 (partial); six of eight declined on this entry's own severity verdict, two deferred.** `unit/split.rs` plans in one pass and carries the sliced parent with the plan — one `windows_of` per unit, one fewer whole comrak parse per splitting table, and **a panic site deleted** because the invariant now rides the signature. `ProfileMetadata::default` is documented rather than restructured: the panic is unreachable for any caller of a built crate, and each structural alternative is worse (a `build.rs` duplicates `load_profile`'s rules; Rust literals violate the single-source rule; a `Default` that stops equalling `default_profile()` hands out an empty system prompt). R0009-0049 / R0009-0050 **deferred** — their fix needs a new field on `GlossaryEntry`, a tier-(a) public type *without* `#[non_exhaustive]` whose serde shape is operator-edited profile TOML, i.e. a breaking wire change for a loop this entry records as dwarfed by the tiktoken encode beside it. **Re-trigger: the next breaking window already moving `GlossaryEntry`.** **Does not count against the v0.5.0 gate.**

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0044, R0009-0045, R0009-0046,
  R0009-0047, R0009-0049, R0009-0050, R0009-0051, R0009-0073), gate-accepted,
  user-routed track
- **Sources:** docs/project/open-issues.md#OI-0041, reviews/reviewed/0009.md#R0009-0044,
  reviews/reviewed/0009.md#R0009-0045, reviews/reviewed/0009.md#R0009-0046,
  reviews/reviewed/0009.md#R0009-0047, reviews/reviewed/0009.md#R0009-0049,
  reviews/reviewed/0009.md#R0009-0050, reviews/reviewed/0009.md#R0009-0051,
  reviews/reviewed/0009.md#R0009-0073
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

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

#### Background

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

### degraded-regen-cascade-scans (OI-0042)

> **RESOLVED 2026-09-04.** All three sites linearized — including `validate/full_rescan_html.rs`'s twin, written 2026-09-03, eight days after the entry, and covered here rather than given its own row because splitting twins across two entries is how one of them rots. `widen_to_neighbors` also stopped comparing `String` where `usize` was available, which is why it was 8× its sibling: 26 ms / 245 ms / 1.9 s at 5,000 / 20,000 / 50,000 blocks, and N is the caller's document rather than this repository's — which is what moved it off the "cold path, therefore negligible" reading.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0033, R0009-0079),
  gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0042, reviews/reviewed/0009.md#R0009-0033,
  reviews/reviewed/0009.md#R0009-0079
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

Two scans that are worse than linear, both on the path that runs only after
regeneration has already failed. `validate/full_reparse.rs`'s
`attribute_offenders` filters the whole `regen_top` collection once per entry,
with both collections sized by the document's top-level block count; its two
call sites are inside terminal error branches of `reparse_full` that return
`Err` immediately. `pipeline/finalize.rs`'s `widen_to_neighbors` does a linear
`BlockId` string comparison per seed (`top_level.iter().position(|id| *id == seed)`)
over the whole identity walk, and is reached only at stage 2 of the cascade,
after `reparse_full` failed **and** the stage-1 re-regeneration failed too.

#### Background

Type 1: both fixes are settled and unblocked — one `HashMap<&BlockId, usize>`
for the seed lookup, and a sorted sweep or binary search over `regen_top` by
`start` for the offender scan. The review filed both as Medium hot-path
problems; verification placed both on a cold failure path that runs at most a
handful of times per run, up to three through the DCR-0004 cascade. Recording
that is the entry's main value — the next reader of the review should not
re-raise these as throughput issues. Verification also corrected the proposed
fix for R0009-0033: containment there is **range**-based (`top.start` inside the
entry's offset span), not id-based, so an ID lookup table is the wrong shape.

### module-size-versus-inline-tests (OI-0043)

> **RESOLVED 2026-09-04.** `output.rs` 4,005 → 411 lines across five concern modules; the other three files close as **measured non-issues**, which is what this entry already suspected. The split is proven pure — an independent review found 119/119 item bodies and 49/49 test bodies **byte-exact** and the code string-literal multiset identical at 746/746, so the staging → fsync → rename → rollback ordering behind `--out-dir`'s all-or-nothing guarantee cannot have moved.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0068, R0009-0069,
  R0009-0070, R0009-0071), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0043, reviews/reviewed/0009.md#R0009-0068,
  reviews/reviewed/0009.md#R0009-0069, reviews/reviewed/0009.md#R0009-0070,
  reviews/reviewed/0009.md#R0009-0071
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

Four files the review called oversized concern bundles from raw `wc -l`.
Re-split on `#[cfg(test)]` with brace tracking, three of the four are mostly
inline test code: `transync-html/src/lib.rs` is 2,550 lines but 1,242
production; `transync-core/src/profile.rs` is 4,035 but 1,738; and
`transync-core/src/pipeline.rs` is 4,981 but only 1,457 — 71% tests, with
`dispatch`, `finalize`, `merge`, `policy`, `report` and `retry` already living
in their own files. The exception is `transync-cli/src/output.rs`: 3,967 lines,
**2,041 of them production**, with only `lock.rs` extracted, so target
resolution, staging, bundle rendering and filesystem policy genuinely share one
module.

#### Background

Type 1: the approach for each is settled — split `output.rs` under the repo's
file-as-module convention, and for the other two move the inline test modules
out (deciding per module between `tests/`, which loses access to private items,
and `src/<name>/tests.rs`, which keeps it). No behavioural defect anywhere; all
four are Low. The value of the verified table is that it stops the wrong work:
acting on the raw counts would mean re-cutting `pipeline.rs`'s orchestration,
which is already cut. R0009-0068 is a **re-raise of a known open item** —
DCR-0032's "Handed forward" section already names the file-as-module split, and
`crates/transync/tests/docs_ownership_drift.rs` encodes it as a `CRATE_ROOTS`
floor of 0, "a state nobody has decided against rather than a shape anyone
chose" — so it rides DCR-0032 and should be sequenced behind the HTML→HTML
waves still landing in that crate rather than racing them.

### disk-cache-log-edges (OI-0044)

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0080, R0009-0081,
  R0009-0082), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0044, reviews/reviewed/0009.md#R0009-0080,
  reviews/reviewed/0009.md#R0009-0081, reviews/reviewed/0009.md#R0009-0082
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

One finding on each of `crates/transync-core/src/cache/disk.rs`'s three paths.
Replay: `scan_log` reads a physical line with `.read_until(b'\n', &mut line)`
into an uncapped `Vec<u8>` and only decides the header afterwards, so a foreign
file with no newline is materialized whole before rejection — contradicting the
function's own claim that "peak memory is the index it is building plus one
record". Write: `buffer_record` returns `Err` with the `BufWriter<File>` still
installed in `DiskState`, and the next `put`/`evict` writes onto it; there is no
last-good-offset and `truncate_to` is never called from the write path. Trim:
`trim_to_budget` measures `header_bytes() + meta_bytes` plus entries but drops
only entries, so when `oldest_first` is exhausted the loop ends with the budget
still exceeded — while DCR-0028 §4 promises entries are dropped "until within
budget" and the public `max_bytes` doc names no exemption.

#### Background

Type 1: each fix shape is settled — a floor plus a doc sentence for the trim, a
last-good offset or a poisoned writer for the write, `Read::take` (or a
corrected doc claim) for the read — and nothing must land first. Verification
narrowed two of the three substantially and sharpened the third. The write
poisoning is **one welded line, not open-ended corruption**: sub-capacity
records self-heal because `BufWriter::flush_buf` re-queues the remainder, so
damage needs a record at or above the 8 KiB buffer; the result is an unparseable
line `scan_log` skips with a warning, and a truncated JSON object concatenated
with a whole one cannot deserialize, so **no wrong value is ever served** — the
cost is two lost entries, inside DCR-0028's stated degrade-to-re-translation
envelope. The unbounded read's security framing was already answered: ADR-0022
puts a local writer to the cache path outside the threat model, naming
R0004-0024 — this same memory concern — by number. The trim is **worse than
filed**: if header plus `meta_bytes` alone exceeds `max_bytes`, every open drops
every unit entry and still forces a compaction, so the cache permanently retains
nothing across opens. No test covers a failing writer.

### serve-body-and-authority-reporting (OI-0045)

> **RESOLVED 2026-09-04.** All three fixed, each with a test proven red against pre-fix code. One correction to this entry: it says `contracts.md` carries the `[::1]` claim in prose and must move with the code — it does not. §6's authority sentence is family-agnostic; the enumeration lived in `docs/Developer_Guide.md` and `docs/Quick_Start.md`, both now corrected.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0002, R0009-0008,
  R0009-0009), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0045, reviews/reviewed/0009.md#R0009-0002,
  reviews/reviewed/0009.md#R0009-0008, reviews/reviewed/0009.md#R0009-0009
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

Three residual `transync serve` findings after the Review 0009 fix pass. In
`serve_cmd/conn.rs`, the streaming branch announces `len` and then discards the
count `tokio::io::copy` returns (`.map(|_| ())`), so a file past the 8 MiB
in-memory limit that shrinks mid-response sends fewer bytes than its
`Content-Length`; the in-memory branch, by contrast, announces
`body.len() as u64`, and `read_capped`'s doc comment reasons only about a file
that *grows*. In `serve_cmd/host.rs`, one `is_unspecified()` branch pushes both
loopback families without consulting `local.ip()`, so a `0.0.0.0` bind — which
is IPv4-only — advertises `[::1]:port` in startup output and 421 bodies; and
derived entries plus every `--allow-host` go into one un-normalized `Vec`, so
`--bind 127.0.0.1 --allow-host localhost` prints `localhost:7470` twice.

#### Background

Type 1: three small, settled fixes, nothing blocked. All three were filed above
their verified severity. The truncated-body case reaches only the >8 MiB branch
— the serve test's own comment says "Nothing in a bundle is this big" — and the
client sees a short read on a `Connection: close` socket rather than silent
corruption. The IPv6 advertisement costs guidance accuracy and one unreachable
allowlist entry; no verdict changes, because a client using `http://[::1]:port/`
never reaches the socket. The duplicate authority is cosmetic: lookup is
`self.answered.iter().any(...)`. Two constraints the work must respect: the
`::` case is **correct as written** (a dual-stack `::` listener does answer at
`127.0.0.1`), so only the IPv4 branch changes — and the fix is three edits, the
code plus the test that pins the wrong expectation plus the `contracts.md`
sentence that repeats it in prose; and the dedup must be order-preserving
retain-first, because `the_authorities_print_the_way_they_are_typed`
deliberately pins ordering.

### parser-intake-markdown-rename

- **Description:** The format seam is asymmetric on purpose — `intake::html` sits beside
  `parser`, not beside a sibling `intake::markdown` — because the rename is mechanical
  and large: ~43 references in `transync-syntax`, ~74 in `transync-core`, the wasm
  import, `public_surface.rs`'s hidden-module pin, and four documents, for **zero**
  behaviour change. Unblocked, pick-up-as-is; the paying wave's diff must be *only* the
  rename, or the review cannot tell the move from a change.
- **Background:** ti `490d97` wave 3 deviation 1; DCR-0035. Registered by wave 7's
  closure so the deferral survives outside the DCR.

## Type 2 — needs decision / discussion next

The open design questions here carry an `OI-` id rather than a ticket: a review gate
registers documents only, so each waits for `reopen`'s selection gate to offer it and
file a ticket for whatever is picked. The ticketed questions this section once listed —
`40e2a5`, `66339b`, `1347b4` — were all resolved on 2026-08-10 and are gone from it.

`sync-anchor-injection-via-raw-html` (OI-0035) asked which layer should own anchor
trust in the browser — resolved 2026-08-23 (route (c), both layers; ti `490d97` wave 1,
DCR-0033); its entry below is retained for the record. `out-dir-sparse-bundle-subset` (OI-0036) was settled with `66339b`
by the marker-file rule and is retained only for its record. `provider-payload-intake-guard`
(OI-0037) sits in Type 1 below, since its approach is settled.
`warm-cache-run-needs-no-credentials` (OI-0038) is the newest, from Review 0004, and is
the one that genuinely needs a product answer before any code.

### publish-lock-nested-tree-boundary (ticket `40e2a5`)

> **reopen 2026-08-24: ticgit:40e2a5 is RESOLVED.** Verified with `ti show 40e2a5 --json`. Kept for audit; it is not open work and does not count against the v0.5.0 gate.

- **Description:** DCR-0021's publication lock serializes two runs publishing *into* the
  same directory, but `--out-dir X` locks the directory that **holds** `X` (where the
  staging and backup siblings live) while a files-mode publish into `X`
  (`--output X/out.md --map X/alignment.json`) locks `X` itself — different inodes, so the
  nested pair never serializes, and the operator can end up with a directory mixing two
  runs. An inode-based lock cannot close it: after the `--out-dir` run swaps its staged
  tree in, a waiter holds a lock on a directory that is no longer at that path. The
  choice: (a) name-keyed leases in the parent directory so the two modes serialize, with
  a test covering the two-mode race, or (b) accept the boundary permanently and say so in
  `contracts.md` §6 beside the existing "Scope" bullet, naming the operator-side
  mitigation (do not aim two runs at one tree through different modes).
- **Background:** Filed 2026-08-06 by the agent verifying ticket `f41f16`
  (R0001-0034/0035/0036); DCR-0021 already records it under "Known boundary", so the
  decision is whether that stays the end state. It is the last `review-0001`-tagged
  ticket still open.
- **Landed 2026-08-10 (option a, by a different key):** the boundary is closed, not
  accepted. Neither mode's lock moved; each reaches one directory further, to where the
  other already is — a fileset commit also locks the deepest level that *exists* on the
  way to a destination it has to create (`claim_anchor`; for a destination that already
  exists that is the destination itself, so ordinary runs are unchanged), and an
  `--out-dir` publish also locks the target and its `html/` (`published_dirs_inside`).
  Name-keyed leases in the parent were the routed sketch and were declined for a reason
  stated in DCR-0021's appended note: they make every files-mode publication write a lock
  marker into the parent of its destination — `--output out.md` in `~/project` marking
  `~` — and fail where a parent is unwritable but the destination is not. Locking a
  target the swap renames away is sound because of ticket `8792b7`'s waiter-side
  revalidation, which lands with it. Residual, stated in `contracts.md` §6: a run
  publishing *deeper* inside a target than `html/`.
- **Review fix round, 2026-08-10:** the inner set (`published_dirs_inside`) was read
  before the lock was taken and never revalidated, so a replace that started before its
  target existed held nothing inside it even after the target appeared. It is now re-read
  under the lock and the whole set retaken until it stops changing
  (`lock_publication_tree`, bounded). A second, narrower residual is stated with the
  first: a directory that appears inside the target after that last re-read, put there by
  something the replace is not serialized with. Both are ticket `cbbc4e`.
- **Both residuals settled 2026-08-10 (ticket `cbbc4e`), by ordering rather than by a
  third claim:** no finite pair of claims meets at every depth — a replace claims a fixed,
  shallow set while a publication claims a point that can be arbitrarily deep — so none
  was added. Claiming upward has no principled root short of `/`; claiming downward is
  unbounded under `--force`, litters a tree the guard may then refuse, and is a TOCTOU;
  and "no lock file beneath the target" is not "no publisher beneath it" (`8792b7`). What
  bounds it instead: a publication reads its anchor *before* it creates anything, so a
  peer aiming under an `--out-dir` target claims the target or a level above it and waits;
  and `ensure_out_dir_replaceable` now runs a **second** time with the staged tree in
  hand, so a directory that appeared inside the target while the bundle was being written
  makes the replace refuse instead of renaming it away. Making that argument true also
  required tightening the guard's shape recognition — the bundle allow-list and both
  staging-temp recognizers matched on name alone, so a *directory* named `index.html` or
  `out.md.tmp.<pid>` was walked past the check and into a backup `remove_dir_all` that
  needed no `--force`. Accepted rather than open, and stated in `contracts.md` §6: the
  `--force` path, which waives the guard by request, and the few syscalls between the
  second guard pass and the rename.

### out-dir-foreign-staging-temp (ticket `66339b`)

> **reopen 2026-08-24: ticgit:66339b is RESOLVED.** Verified with `ti show 66339b --json`. Kept for audit; not open work.

- **Description:** `ensure_out_dir_replaceable` admits only `out.md`, `alignment.json`,
  `validation-report.json`, `html` and the `.transync-publish.lock` marker at an
  `--out-dir` target's top level; any other name makes the target foreign and the publish
  exits 4 until `--force` is passed. A crashed files-mode run into that same directory
  leaves exactly such a name behind — `write_fileset_atomic` stages each payload as
  `<path>.tmp.<pid>` inside the destination — so the entry being refused can be
  transync's own residue. `scan_bundle_dir` and the nested `html/` scan already recognize
  that suffix (`tmp_pid_suffix`); the top-level loop does not. The choice: (a) leave it,
  which is what `contracts.md` §6 says today and what `docs/Troubleshooting.md` now tells
  the operator (`a5ffe50`), or (b) give the top-level loop the same tmp-pid recognition —
  one rule instead of two, and one fewer `--force` recommendation — at the cost of
  widening what an `--out-dir` publish will `remove_dir_all` without asking, which owes
  the same "is nested user data reachable?" argument the EXT-2026-07 review-fix made for
  `html/`.
- **Background:** Found 2026-08-07 by ticket `3b90c1` while deriving the `--force` rule
  from `output.rs`. The behavior matches `contracts.md` §6 as written, so this is a
  proposal, not a defect.
- **Landed 2026-08-10 (option b, with OI-0036's marker paying for it):** the top-level
  loop got the tmp-pid recognition, and the widening it owed is paid by the second
  question the guard now asks — the `.transync-out-dir` ownership marker (or the complete
  published set) — so tolerating transync's own residue never widens what gets replaced
  in a directory transync did not publish. Decided in one change with OI-0036 directly
  below, as both entries required.

### out-dir-sparse-bundle-subset (OI-0036)

> **reopen 2026-08-24: OI-0036 lives in `open-issues-archive.md`,** i.e. resolved and archived by `indy-review-prune`. Kept for audit; not open work.

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0028), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues-archive.md#OI-0036`
- **First seen:** 2026-08-08
- **Last seen:** 2026-08-08

#### Description

`ensure_out_dir_replaceable` treats a directory as a transync bundle — and so
replaceable without `--force` — when its entries are a **subset** of the allow-listed
bundle names. A user directory that happens to contain only `out.md` therefore
qualifies: it is moved aside and its backup recursively removed, with no `--force` and
no prompt. The operator did aim `--out-dir` at that directory, which is what keeps this
below the destructive findings the same review produced, but "holds one allow-listed
name" is a weaker test than "is a bundle this tool produced", and the guard reads as
though it were the stronger one.

#### Background

Low severity at HEAD (the reviewer filed it Medium). It is deliberately parked beside
`out-dir-foreign-staging-temp` (`66339b`) directly above: both are the same question —
how strictly "is this my bundle?" should be answered — approached from opposite
directions, in the same function. Deciding them apart risks two inconsistent answers.
Candidate shapes: require a marker file written by publication; require the full bundle
set rather than a subset; or require at least one *distinctive* member
(`alignment.json` + `html/`).

**Landed 2026-08-10** (first candidate shape, with the second as its fallback), in one
change with `66339b` as both entries required. `publish_out_dir` stages a
`.transync-out-dir` ownership marker into every tree it publishes, and the guard asks
"is anything in here not transync's?" and "did transync publish this?" separately; a
target with no marker still passes on the **complete** published set, so bundles written
before the marker existed keep republishing. `docs/project/open-issues-archive.md#OI-0036` is
RESOLVED.

### sync-anchor-injection-via-raw-html (OI-0035)

> **reopen 2026-08-24: OI-0035 was RESOLVED 2026-08-23 and archived** (route (c), DCR-0033). Verified: 0 hits in `open-issues.md`, 1 in `open-issues-archive.md`. Kept for audit; not open work.

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0018), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues-archive.md#OI-0035` (RESOLVED 2026-08-23, archived — was `open-issues.md` until then)
- **Accepted residual (ti `490d97` wave 7, DCR-0039):** recorded in DCR-0033 and not
  restated here beyond the one fact the note above omits — an impostor carrying a
  *listed* id ahead of the genuine anchor still defeats the engine layer alone; the
  render strip is what closes it for panes transync produces. Wave 7 corrected the
  Type-2 preamble above, which had gone on framing this as an open design question.
- **First seen:** 2026-08-08
- **Last seen:** 2026-08-08

#### Description

The browser sync engine builds its anchor sets by collecting **every** `data-sync-id` in
each pane's DOM; alignment rows validate and warn but are not the source of the anchor
set, so map membership does not gate what can become a scroll driver or target. Source
Markdown is untrusted (architectural invariant 7) and raw HTML blocks are translatable,
structurally-owned content that reaches the rendered pane — so a `data-sync-id` written
into a source document survives the default DOMPurify configuration and can **pre-claim
a real block's id**.

#### Background

Low severity at HEAD; DOMPurify still bounds what markup renders, so the failure is
steering, not execution. Half of the reviewer's finding is the recorded `d3acc3` /
OI-0015 ID-identity decision and is not in question — this entry is only the unrecorded
half. It needs a decision before code because the fix could live at the sanitizer
(strip or namespace `data-*`), at the engine (build anchors from validated rows), or at
both; and whichever is chosen must move `web/js/sync.js` and its byte-identical embedded
CLI twin in one commit. *(2026-08-09: `d3acc3` landed and left all three routes open —
the anchor sets still come from `querySelectorAll("[data-sync-id]")`. It did make the
engine route cheaper: reflow recompute re-collects anchors through the same
`collectAnchors`, which is now the single place every anchor set passes through, at mount
and on every reflow, and it already takes a per-call policy argument.)*

### warm-cache-run-needs-no-credentials (OI-0038)

- **Type:** 2
- **Verified:** yes — Review 0004 finding (R0004-0069), gate-accepted, user-routed track
- **Sources:** ticgit:30a744f1, `docs/project/open-issues.md#OI-0038`
- **First seen:** 2026-08-13
- **Last seen:** 2026-08-13

#### Description

The pipeline constructs its `Translator` — and so demands an API key — before
the cache is consulted, so a run in which every unit is a cache hit and zero
provider calls would be made still cannot start without credentials. DCR-0028's
own tests prove such a run exists (they assert zero dispatches on a second
identical run); what no record says is whether that run is meant to be possible
**offline**.

#### Background

Type 2, not 1: the product question comes first and decides the fix's shape —
is an offline run against a fully warm cache a supported scenario? Only then
does the mechanism follow (lazy `Translator` construction, or resolution at
first dispatch), and with it a semantics that does not exist today: what a run
that started without credentials does when it takes a cache **miss**. That
belongs with ADR-0009's retry/fallback contract rather than being invented at
the call site. Low severity — the failure is a clean refusal to start, not
wrong output — but it withholds the disk cache's most valuable property
(a document is paid for once) exactly where it is most wanted.

### docs-index-orphaned-trees (ticket `1347b4`) — **RESOLVED 2026-08-10**

- **Description:** `docs/index.md` calls itself the "single entry point for everything
  under `docs/`" and tells its maintainer to link every new doc "so nothing becomes
  orphaned", but one tracked non-Korean doc was absent from it: this file. (The
  fourteen-file `docs/investigation/` bundle was the rest of the gap until 2026-08-08,
  when the owner untracked it — an untracked bundle is not the index's to carry, so only
  this file remained.) It was not linked from `docs/architecture/README.md` either, so it
  was unreachable from the entry point.
  `crates/transync/tests/docs_index_drift.rs` passed because it enforced only
  `docs/decisions/*.md` and the DCRs, while its module doc restated the header's broader
  claim. The choice: (a) index them and widen the drift test to the header's rule, or
  (b) narrow the header to the trees the index actually covers and align the test's own
  doc — not both.
- **Resolution (2026-08-10):** the owner routed it to **(a)**. This file and
  `docs/project/git-history-loss-2026-08-10.md` are linked from `docs/index.md`, this
  file also from `docs/architecture/README.md`'s "Where to look next", and the drift test
  now requires **every** Markdown document under `docs/` at any depth. Its exclusions are
  the ones the index states to a reader — `*.ko.md` siblings, git-ignored bundles under
  `docs/`, `index.md` itself, non-Markdown files — and the git-ignored set is read out of
  `.gitignore` instead of hardcoded, so re-tracking `docs/investigation/` requires its
  files again in the same edit. Two tests were added beside the widened one: one asserting
  the required set reaches past the two record trees, one asserting the stated exclusions
  are the only exclusions.
- **Background:** Found 2026-08-07 while refreshing this file (`9c709cc`), checking that
  the refreshed index of open items is reachable from the documentation entry point. It
  is a judgment call about what the index is for, which is why it is a decision rather
  than a one-line edit.

The ten items below were reviewed by the owner on 2026-08-06 and **deferred as a group**
("이번에는 보류" — hold for now). They stay listed so the next sweep re-surfaces them;
none should be picked up without a fresh owner decision. None of the ten was ruled on
again, ticketed, or otherwise moved during the arc that followed.

### streaming-translation _(owner-deferred 2026-08-06)_

- **Description:** The pipeline is whole-batch request/response only; no
  incremental/streaming delivery path exists. Post-MVP feature wave; needs product/design
  discussion before any work.
- **Background:** mvp-scope DEFERRED table; "not a slice" list.

### sentence-level-sub-anchors _(owner-deferred 2026-08-06)_

- **Description:** Sync anchors are block-level only (draft NG3). Listed in the DEFERRED —
  not permanently-rejected — table, so picking it up is a posture decision with large
  design implications (invariant 1 names block ID as the only sync currency).
- **Background:** Draft non-goal NG3 retained as deferred in mvp-scope; also named by
  ADR-0001 as the finer-grained alternative it declined.

### mdx-frontmatter-math-support _(owner-deferred 2026-08-06)_

- **Description:** MDX, YAML frontmatter, and math syntax are unparsed/unsupported (draft
  NG2), retained in the DEFERRED table. A posture decision, and technically entangled with
  the dialect-trait item (Type 3).
- **Background:** Draft non-goal NG2; mvp-scope DEFERRED table and scenario-matrix
  out-of-scope list.

### glossary-editor-ux _(owner-deferred 2026-08-06)_

- **Description:** Glossaries are hand-edited profile TOML; no editor tooling exists.
  Post-MVP product decision.
- **Background:** mvp-scope DEFERRED table ("post-MVP feature wave").

### html-attribute-text-translation _(owner-deferred 2026-08-06)_

- **Description:** HTML attribute text (`alt`/`title`/`aria-label`) is not extracted for
  translation. A scope decision on widening the segment-extraction contract.
- **Background:** DCR-0016 / ADR-0018 follow-up list (2026-08-03 design, §9).
- **Cross-reference (ti `490d97` wave 7):** now also the HTML-document intake's recorded
  limitation — ADR-0025 names the visible consequence (a shared link previews in the
  source language). The owner gate is unchanged; the population grew from raw-HTML
  islands to whole documents.

### phrasing-custom-element-mid-sentence-review

> **RESOLVED 2026-09-04 (`ticgit:84bf37`) — the owner ACCEPTED the current behaviour.** The re-confirmation ran on the shipped intake over 42 real pages (10.1 MB, 8,879 blocks, 346 custom-element start tags) across two disjoint corpora, the second deliberately covering the generator classes the first missed (Sphinx, MkDocs, Docusaurus, Jekyll, Hugo, GitBook, WordPress, Ghost, Notion, Confluence, gov.uk, MediaWiki, two production email templates): **zero mid-sentence splits.** The measurement also narrowed the hazard — the split fires only in prose **not wrapped in an element**; inside `<p>`, a heading or an `<li>` the wrapping element is the block and the custom element is interior. Twelve errata landed in the spec recording that, including in §4's PHRASING bullet and §13 limitation 5, which both stated the split unconditionally. The untaken lever now lives at `phrasing-extension-list-for-custom-elements`, deferred at filing.

- **Description:** A custom element mid-sentence **stops** segmentation (D4's DEFAULT-STOP
  default), so one sentence can cross three translation units. Accepted *for now* with
  the PHRASING widening — and the owner **explicitly required re-confirmation against
  real output**. Candidate lever if the review rejects it: a per-run or per-profile
  phrasing-extension list, designed then, not now.
- **Measured 2026-09-04 (ti `490d97` wave 7), and it is NARROWER than §14.1 states.**
  Three shapes through the shipped `intake::html`, captured in
  `/Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/`:
  - **Wrapped in `<p>` — no split.** `<p>The Pro plan costs <price-tag>29</price-tag>
    per month.</p>` is **one** `paragraph` block: the `<p>` is the element block and the
    custom element lives inside its extent. Three such sentences yielded
    `paragraph paragraph paragraph`, not nine blocks.
  - **Naked prose — the split fires.** The same sentence directly inside a `<div>`
    yields `p-0002 html-0003 p-0004`: run, stopped element, run. This is the §14.1
    hazard, reproduced.
  - **Known phrasing is absorbed either way.** `<em>` keeps its run whole.
  So the hazard is confined to prose that is **not wrapped in an element** — which real
  pages rarely are. §14.1's own example (`Price: <my-price/> today`) is written in the
  naked form without saying that the form is what makes it split.
- **Background:** spec `2026-08-20-html-to-html-translation-design.md` §14.1 and §13.5;
  ADR-0025 (D4). Wave 7 staged the evidence and filed the ticket; the ruling is the
  owner's and is not wave 7's to take.
- **Sources:** ticgit:84bf37 (filed 2026-09-04 with the measurement above and three
  options: amend §14.1 to state the wrapping condition, add a phrasing-extension list,
  or leave it).

### html-details-fold-reproduction _(owner-deferred 2026-08-06)_

- **Description:** Interleaved `<details>` regions in raw-HTML blocks always render
  visible; collapsed/expanded state is not reproduced. Scoped out of v1 by decision 5 —
  reviving it is a scope decision.
- **Background:** DCR-0016 follow-up list.

### html-nested-in-list-blockquote-protection _(owner-deferred 2026-08-06)_

- **Description:** HTML blocks nested inside list items/blockquotes are protected only by
  coarse child-kind topology labels — the LLM can still mutate tag text there. Closing the
  accepted v1 gap needs an extraction-inside-containers design.
- **Background:** DCR-0016 follow-up list ("an accepted v1 gap").

### intra-block-progress-indicator _(owner-deferred 2026-08-06)_

- **Description:** Within very long blocks (large tables, long code blocks) sync feels
  coarse; an optional intra-block progress indicator is the drafted mitigation
  (references/draft.md §18.2). Post-MVP UX decision.
- **Background:** ADR-0001's accepted "Bad" consequence of block-level sync currency.

### browser-sync-ux-polish _(owner-deferred 2026-08-06)_

- **Description:** Three small accepted MVP simplifications in the sync engine: no
  hysteresis on active-block selection (rapid flicks may briefly pick a neighbor), a
  single fixed partner-pane easing policy (20%/frame lerp), and `SMOOTHING_FACTOR`
  hardcoded rather than a `mountSync` option. Each is small; whether any is wanted is a UX
  priority decision.
- **Background:** web/SMOKE.md "Known limitations (post-MVP)"; 2026-05-03 smooth-scroll
  spec's deferred list.

### in-browser-retranslation _(owner-deferred 2026-08-06)_

- **Description:** The wasm demo edits and re-renders locally but cannot re-run
  translation in the browser — `transync-core` cannot compile to wasm32 (tokio/tiktoken).
  Enabling it needs a network boundary or a second crate split; scoped as its own future
  project.
- **Background:** Track C design spec §9 (2026-08-05), explicitly framed as future work.

_Resolved since the 2026-08-05 sweep, and removed from this section (each carries its
record elsewhere; a bare six-hex id below is a TicGit ticket, a seven-hex one a commit):
`oi-0015` decided and commissioned as ticket `d3acc3` (single-file re-affirmed,
ID-identity to become normative, reflow hooks in the same pass) — that pass **ran
2026-08-09**, so `open-issues-archive.md` now carries OI-0015 (and OI-0024) as RESOLVED;
`oi-0018` **RESOLVED 2026-08-07**
as the opt-in `--strict-csp` flag (ticket `14307f`), default posture unchanged, and
`261e2c5` then corrected the three places that described the flag as confining a bundle
to its folder — CSP `'self'` scopes to the serving **origin**, so the guarantee is a
remote-load block, not a sandbox around the directory; `oi-0020` **RESOLVED 2026-08-06**
— `Cargo.lock` is committed (`6cf4164`, a repo-local `!Cargo.lock` un-ignore) and the
`wasm-bindgen = "=0.2.126"` pin is kept alongside it, while the `CLAUDE.md` half of the
same machine-global-gitignore concern was explicitly **declined**, so `CLAUDE.md` — not
the lockfile — stays untracked by owner choice; its entry still read OPEN until ticket
`d0cd98` corrected the record (`f3f52ac`); `oi-0034` and its payload-side residual
resolved (normalize at intake, ticket `743d27`; fail-the-unit for payload NUL, ticket
`d06c43` — `out.md` never contains a NUL, contracts.md §3);
`default-profile-korean-glossary` resolved — examples commented out (ticket `e7ae21`);
`cache-identity-batch-cohort` resolved — prompt-relevant identity, `CacheKey` gains
`instruction_hash` (ticket `c02f69`, 0.3.0 window); `next-release-cut` decided — v0.3.0
after the waves and smoke (owner, 2026-08-06), cut at `30d467c`;
`untracked-analysis-bundles` settled 2026-08-08 (owner) the other way: the
`docs/investigation/` bundle is untracked again, reversing the 2026-08-06 decision that
committed it in `727d3bc`, because it is regenerated from the code rather than authored,
so each regeneration produced a diff carrying no decision — the files stay on disk.
`reviews/` was not reversed with it: round 0001, its dispositions, its patch and
`README.md` stay committed as the dated record they were made, and a later round joins
them when the gate archives it;
`wasm-demo-schema-triple-mirror` decided 2026-08-06 under owner delegation — the
test-pinned mirrors ARE the end state (each of the three JS copies is pinned by a cargo
test; a build-time injection was declined because it would put a build step in front of
the deliberately no-build `web/` demos); the doc-hidden `build_batches` gate shipped
(ticket `0ed6eb`, commits `296ce67` + `721ba93`)._

---

### validation-report-schema-bump-for-document-level-entry

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:52109b22
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`ValidationReport::skipped_source_nodes` now carries a document-level entry alongside the per-block notes it carried before, which changes the ordering contract `contracts.md` §3a states — and `VALIDATION_REPORT_SCHEMA_VERSION` stayed at `1.1.0`.

#### Background

The precedent for the 1.0.0 → 1.1.0 bump (DCR-0026) was exactly 'a new row shape a 1.0.0 consumer never met'. This is a new entry shape by the same description, so either the precedent applies and the constant owes a bump, or the precedent is narrower than it reads and the difference should be written down. A consumer that iterates the array positionally is the one that would break. Type 2 because it is a versioning decision with a real cost either way: bumping obliges consumers, and not bumping weakens what the version number promises. Nobody can start until that is chosen.

### review-round-registry-number-collision

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:bdf8d981
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`reviews/README.md` is the resolver for bare finding ids and states that rounds 0002 and up never collide. Three gated 2026-08 rounds reused 0002, 0003 and 0004, and 508 live citations depend on the rule that is now false.

#### Background

The registry's rule 4 says verbatim that each number was used once, so `R0002-…` through `R0008-…` are unambiguous bare and need no marker. That is what makes a bare citation resolvable. With three numbers reused, a bare `R0002-0047` has two possible referents and a reader has no way to tell which. The collision was observed live on 2026-08-24, when `reviews/0002.md` existed again as a fresh `STATUS: CLAIMED` stub; that transient file is gone today, but the reused numbers and the citations that depend on them are not. Type 2 and tagged owner-decision because the repair is a choice between renumbering the newer rounds, qualifying every affected citation, or amending the rule and accepting ambiguity — each with a different cost across 508 citations.

### wrapper-ruling-leaves-named-default-stop-elements-exposed

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:d4bce239
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

The owner's 2026-08-21 wrapper ruling closed the custom-element anchor hole, but it is keyed on elements unknown to spec §4's tables — and DOMPurify also removes some elements those tables do name.

#### Background

The ruling fixed a measured defect: a block whose outermost element is unknown to the tables now takes a transparent `<div{attrs}>` wrapper instead of having the anchor injected into its own open tag, because the sanitizer was removing the element and the anchor with it. Verified against the vendored purify 3.2.6. The residue is that 'unknown to our tables' and 'removed by the sanitizer' are different sets, and the ruling covers only the first. A named DEFAULT-STOP element the sanitizer strips still loses its anchor. Type 2 and owner-decision because widening the rule means either deriving it from the sanitizer's behaviour — which the project has elsewhere refused, on the ground that a second opinion about HTML is how anchors drift — or enumerating a second list that must be maintained against a vendored dependency.

### html-to-html-document-translation-epic

- **Type:** 2
- **Verified:** yes — owner-commissioned roadmap ticket
- **Sources:** ticgit:490d9712
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

The HTML→HTML feature: a second intake producing the same block IR, delivered as eight waves. **COMPLETE 2026-09-04** — all eight waves (0–7) landed, recorded in DCR-0032 through DCR-0039.

#### Background

This was the umbrella the HTML work ran under, recorded here because a backlog that omits the largest open item in the tree is not a census. **All eight waves landed** — DCR-0032 (wave 0) through DCR-0039 (wave 7, 2026-09-04) — and the executed order was 0 → 2 → 3 → 4 → 5 → 6 → 7, corrected from the spec's original graph by wave 4's deviation 1. Retained for the record; it is no longer open work and does not count against the v0.5.0 gate. What the feature deliberately left open has its own entries: `phasing-custom-element-mid-sentence-review` (the owner's §14.1 re-confirmation, `ticgit:84bf37`), `markdown-island-reclassification`, `html-oversize-leaf-block-split` and `parser-intake-markdown-rename`.

### bin-only-crates-are-outside-the-rustdoc-gate

- **Type:** 1
- **Verified:** yes — measured 2026-09-04 during OI-0043's `output.rs` split: nine broken intra-doc links,
  four of them in `crates/transync-cli/src/output/lock.rs`, **a file the split never opened**, because its
  `super::X` references silently became sibling references when the concerns moved out.
- **Sources:** OI-0043's resolution (2026-09-04)
- **First seen:** 2026-09-04 · **Last seen:** 2026-09-04

#### Description

Nothing in this repository would ever have reported those nine links. `scripts/lib/rustdoc-gate.sh` excludes
`transync-cli` by design — its own comment reads "`transync-cli` is absent on purpose — it is bin-only, with
no public API to document" — and `cargo clippy --all-targets -- -D warnings` does not check intra-doc links
at all. So a bin-only crate's rustdoc can rot without limit, and a refactor in one file can break doc links
in a file it never touched. A tenth break was latent behind `#[cfg(not(unix))]` and would have surfaced only
on a Windows build.

#### Background

Distinct from OI-0046's rustdoc **completeness** sub-item, which asks whether every *library* member is
inside the gate; this asks whether bin-only members should be doc-checked at all. The cheap shape is a
`cargo doc -p transync-cli --no-deps --document-private-items` leg with `-D warnings`, which is what
diagnosed these nine — it costs one more `cargo doc` invocation and needs no new dependency. The reason it
was excluded was "no public API to document", which is true and beside the point: `--document-private-items`
is how a developer reads this crate, and that is the reader the links are for.

### phrasing-extension-list-for-custom-elements

- **Type:** 2
- **Verified:** yes — measured 2026-09-04 over 42 real pages (10.1 MB, 8,879 blocks, 346 custom-element
  start tags): **zero** mid-sentence splits
- **Sources:** ticgit:84bf37 (RESOLVED 2026-09-04 — the owner accepted the current behaviour), spec
  `2026-08-20-html-to-html-translation-design.md` §14.1 / §13.5, ADR-0025 (D4)
- **First seen:** 2026-09-04 · **Last seen:** 2026-09-04

> **Deferred at filing, 2026-09-04, by owner ruling.** `84bf37` was closed by **accepting** the current
> behaviour on the measurement above; this entry carries only the untaken lever, and it is recorded so the
> option survives outside the closed ticket. **Does not count against the v0.5.0 gate.** **Re-trigger: a
> real page whose prose sits directly inside a container rather than in `<p>`/heading/`<li>` AND whose
> sentences are cut by a custom element** — the shape the 42-page corpus did not contain.

#### Description

An unknown element is DEFAULT-STOP (ADR-0025 D4), so a custom element used mid-sentence flushes the text run
before it and starts a new one after. In prose **not wrapped in an element** that severs one sentence into
`p-…` / `html-…` / `p-…` translation units. Wrapped in `<p>`, a heading or an `<li>` the wrapping element is
the block and the custom element is interior, so the sentence stays one block — which is why the measured
corpus shows zero splits. The candidate lever, if the naked case ever matters, is a per-run or per-profile
phrasing-extension list letting an operator declare `price-tag`, `fa-icon` and friends as phrasing.

#### Background

The prohibition that constrains any design here: **unknown elements must not become PHRASING by default.**
DOMPurify removes an unknown element and every attribute riding it, which is why wave 6's 2026-08-21 ruling
puts such a block's anchor on a transparent `<div>` wrapper; treating one as inline phrasing would put its
text inside a run whose markup the sanitizer then deletes.

### git-history-lost-twice-standing-record

- **Type:** 2
- **Verified:** partly — reopen verified 2026-08-24: 48 commits and a v0.4.0 tag now exist, so the stated symptom is stale; the loss itself is permanent
- **Sources:** ticgit:6b43008a
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

The pre-restart history is unreachable and always will be. The ticket's stated symptom — exactly one commit, no tags — has been overtaken by subsequent work.

#### Background

Verified: `git rev-list --count HEAD` is 48, `git tag` prints `v0.4.0`, `git fsck` is clean, and the root commit is `59ce8df723b4`, the 2026-08-17 restart rather than the original root. So half the claim stands permanently and half no longer reproduces. `docs/project/git-history-loss-2026-08-17.md` and its 2026-08-10 sibling record both events. Type 2 rather than type 1 because there is nothing to implement: what remains is a decision about what this ticket is *for*. Read as a defect it is unreproducible and should close; read as the standing record that this object store has now lost history twice — and that `/Volumes/Common` is therefore not reliable storage — it should stay open and be re-titled. That is an owner call, not a maintenance one.

### js-lint-gate-exit-status-is-dishonest (OI-0039)

> **RESOLVED 2026-09-04 — format half adopted, lint half deferred with a re-trigger.** `biome.jsonc` at the repo root (formatter on, linter and assist **off**, `@biomejs/biome` pinned exactly at 2.5.12 because formatter output is not semver-covered); 457 lines reformatted across 10 files; the hook's JS leg rewritten to a `git ls-files` corpus invoked **from the repo root**, so it reaches the CLI `sync.js` twin and `profile.mjs` — the 22% the old `cd web` scope structurally could not see — and an empty file list is now an explicit FAIL. Two things landed first as fixes rather than decisions: the printed remediation no longer instructs an action that makes the next commit fail with 180 errors, and a tool **declared** in `web/package.json` but not installed now fails instead of skipping. Taken now because the deferral's own re-trigger — "the first commit that already moves both `sync.js` copies together" — is OI-0047's fix, so deferring would have re-triggered immediately. **Lint half deferred; re-trigger: a JS defect reaching a shipped bundle that a lint rule would have caught** (measured basis: 62 diagnostics over all 31 historical blob versions, 61 of them one style preference). **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 finding (R0009-0014), gate-accepted,
  user-routed **fix**; the visibility half landed in `88964df` and the
  exit-status half was deliberately refused there as a decision above a fix
  route
- **Sources:** docs/project/open-issues.md#OI-0039, reviews/reviewed/0009.md#R0009-0014
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

`scripts/hooks/pre-commit` — the only hook copy, the one `core.hooksPath`
points at — runs a JavaScript/TypeScript leg over `web/` whose `run_js_tool`
helper prints a SKIP line and `return 0` when the tool binary is absent.
`web/node_modules/.bin/` holds exactly one entry, `playwright`;
`web/package.json`'s only devDependency is `@playwright/test`; and there is no
`web/tsconfig.json`. **Every probe therefore misses**, and the leg's
contribution to the exit status is always zero. Measured by running the hook
against a scratch tree mirroring `web/` with no `Cargo.toml`:
`SKIP: prettier not installed in web`, `SKIP: eslint not installed in web`,
`HOOK_EXIT=0`. The consequence is that `web/js/sync.js` — 52,996 bytes, the
sync engine — plus `web/js/wasm-demo.js`, `web/playwright.config.js` and the
three spec files ship with zero format or lint coverage, and the hook is the
only automatic gate this repository has.

#### Background

Type 2, and this is the whole point of the entry: the fixing agent did the
visibility half and **refused the substantive half on the record**. The hook now
ends with a framed block reading `THE JAVASCRIPT/TYPESCRIPT GATE DID NOT RUN.`,
naming the missing tools and stating that "a successful exit below covers the
Rust gates ONLY". The status is still 0. Making it honest requires choosing
between two routes, each with a named cost. **(a) Adopt and pin a linter** — the
hook already has a biome-first branch, so `pnpm add -D @biomejs/biome` alone
makes it fire, but the price is conforming roughly 190 KB of existing JavaScript
including `sync.js`, whose byte-identical twin at `crates/transync-cli/web/sync.js`
is pinned by `crates/transync-cli/tests/sync_js_drift.rs` — any reformat is a
two-file commit or that test goes red. **(b) Fail only on staged JavaScript** —
narrower, but it blocks work in progress, stopping the first developer to touch
a JS file mid-feature. Verification also narrowed the finding itself: the
reviewer's "as if the language gate ran" is wrong (two SKIP lines with the exact
remediation have always printed, and skip-with-notice is DCR-0018's recorded
design, which both 2026-08-20 wave plans call "normal output here, not a
failure"), and there is no correctness exposure because Playwright covers
browser behaviour. Do **not** close this by improving the message again — the
message is already as loud as a message can be; what is unresolved is the
status.

### repeated-parsing-on-the-accepted-path (OI-0040)

> **RESOLVED 2026-09-04 (partial), and the residual is deferred with a SELF-FIRING trigger.** The `[`-prefilter landed at both `inline_inventory` sites; the shared-AST **seam** is deferred, and the deferral carries its own tripwire rather than a promise: `build_batches` now raises one run-level `tracing::warn` when `unit_count × |ref_defs|` passes 6,000,000 bytes, naming `Document.ref_defs` as the knob. **Re-trigger: that warning firing on a real run.** Measured: +1,756 ms on a 1,289 ms baseline at 5,000 units × 20 KB pool; the prefilter recovers 51.8% at this corpus's bracket density and 0% at link-reference house style, which is why the residual is real rather than closed. Also corrected here: this entry's sentence that R0009-0034 and R0009-0037 "are **not blocked** by anything and should ride OI-0037" is **dead** — OI-0037 resolved 2026-09-01 the other way (it shared a *guard*, not an artifact), so those two are re-homed onto OI-0040. **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0031, R0009-0034,
  R0009-0035, R0009-0037, R0009-0043, R0009-0065), gate-accepted, user-routed
  track
- **Sources:** docs/project/open-issues.md#OI-0040, reviews/reviewed/0009.md#R0009-0031,
  reviews/reviewed/0009.md#R0009-0034, reviews/reviewed/0009.md#R0009-0035,
  reviews/reviewed/0009.md#R0009-0037, reviews/reviewed/0009.md#R0009-0043,
  reviews/reviewed/0009.md#R0009-0065
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

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

#### Background

Type 2: three of the six cannot be started without a decision, so the cluster
tie-breaks up. R0009-0031's fix pushes an arena lifetime through
`transync-syntax`'s public render signature, because `parser::parse` owns its
arena and returns owned IR. R0009-0043 must **not** be fixed by carrying the
validated splice into regen: the two calls take different inputs — validation
uses `constraints.html.source_bytes` from `outcome::block_payload`, regen slices
`doc.source_text` via `parser::ranges::clamped_char_bounds` — and `validate.rs`
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

### checking-apparatus-holes (OI-0046)

> **RESOLVED 2026-09-04, and sub-item 3 paid for itself immediately.** (1) The rustdoc completeness loop moved into the shared `rustdoc-gate.sh`, so the hook gained it — its candid deferral trigger had **already fired** (`Developer_Guide.md`'s gate command omitted `transync-lang`). (2) The Playwright port got an env fallback. (3) The "existing coverage suffices" demonstration **failed on measurement**: 8/8 historical divergence classes are pinned, but **not one of the 38 pinned edge cases produces a single orphan close tag**, so `balance_fragment`'s orphan-deletion pass — the first thing it does — had zero corpus coverage; six entries carry a literal `<` and none carries one alongside an orphan closer. The two ingredients existed separately and never together, which is the combinatorial gap a hand-picked corpus cannot close. `generative_properties.rs` (840 lines, nine-line `splitmix64` over four pinned seeds, `std` only, no dependency, no `cargo-fuzz`, no `#[ignore]` — because the hook is the only automatic gate and a coverage-guided harness would run at no venue) found a **live anchor-injection defect on its first execution**, now `ticgit:fdd989`. 400,000 generated inputs over 4 seeds: 399,872 satisfy every property, 128 fail, all one class.

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0015, R0009-0018,
  R0009-0067), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0046, reviews/reviewed/0009.md#R0009-0015,
  reviews/reviewed/0009.md#R0009-0018, reviews/reviewed/0009.md#R0009-0067
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

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

#### Background

Type 2: two of the three need a choice before code. The browser harness needs a
decision about how a second concurrent run gets **both** its port and its
workdir — a port-only change would leave the destructive resource shared — and
the missing generative harness needs its shape chosen (in-crate property tests,
a `cargo-fuzz` target, or a differential oracle against a browser parse) and its
home decided, given there is no CI and the hook is the only automatic gate.
R0009-0015 alone is settled and cheap. Verification narrowed it twice: the
smoke/hook split is **documented and deliberate** (`rustdoc-gate.sh` says "What
keeps it total is the completeness check in `scripts/smoke.sh`") and there is no
live gap today, since all eight members with a `src/lib.rs` are in
`RUSTDOC_GATE_CRATES` — but the check is a pure bash glob with no cargo cost, so
moving it into the shared library closes the window for free. The fuzz item has
the strongest expected value in the entry, and it is not speculative: this file
already records "15,726 violations / 200k fuzz iterations" behind ticket
`95f55b` — someone fuzzed this crate ad hoc, found a real defect, and did not
keep the harness. R0009-0017, the sibling foreign-server finding, was fixed in
`88964df`; build on it rather than duplicating it.

### sync-engine-dead-zone-and-quiet-reflow (OI-0047)

> **RESOLVED 2026-09-04.** The bottom clamp landed and the two drift diagnostics now re-run on every coalesced reflow recompute, **latched** so a warning fires only when the verdict changes; the refresh-only alternative was rejected because a diagnostic that fires only on an explicit call is one nobody sees. Both `sync.js` copies moved in one commit. `contracts.md` §4a is amended for the consequence: its "one property read per pane at mount, never per frame" budget is now one read per pane **per recompute frame**, and that widening is the contract.

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0021, R0009-0025),
  gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0047, reviews/reviewed/0009.md#R0009-0021,
  reviews/reviewed/0009.md#R0009-0025
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

Two findings in `web/js/sync.js`. `activeBlockWithProgress` returns `null` once
the reference line (`scrollTop + REFERENCE_OFFSET_PX`, with the constant at 4)
passes the last anchor's bottom, and `handleScroll` bails on `null` — so the
follower pane freezes. Measured against a stub DOM with three anchors ending at
y=900 and 1200px of unanchored trailing content: `scrollTop=896` gives
`{"id":"p-3","progress":1}`, `scrollTop=897` gives `null`, and so does
`scrollTop=1500`. Separately, `recompute` calls
`collectAnchors(pane, label, rowIds, true)` — quiet — and calls neither
`warnMapDomDrift` nor `warnOffsetParentDrift`: after mutating a mounted pane to
hold a duplicate id, an unlisted id and a block whose `offsetParent` is a
`<figure>`, `controller.refresh()` emitted **0** warnings where a fresh mount of
the same DOM emitted **3**.

#### Background

Type 2: the clamp is settled, but the diagnostics question is not.
`contracts.md` §4a states a mount-time budget — "One property read per pane at
mount, never per frame" — so re-running the probes on every coalesced reflow
frame changes a documented budget rather than merely adding a call; the
candidate shapes are re-running only on the explicit `controller.refresh()`, or
re-running and latching so a warning fires only when the verdict changes.
Verification narrowed both members and corrected the review's paths. Neither
shipped shell can scroll into the dead zone: it needs more than a viewport of
unanchored content **inside the scroll box**, and shipped panes carry only the
sanitized `<main>` plus 12px of padding while the sole non-anchoring kinds are
`ThematicBreak` and `Title` — a third-party pane with a tall in-pane footer can.
Half the review's recommendation is already implemented (a viewport above the
first anchor already returns it at progress 0), so what is missing is the
symmetric bottom clamp, about three lines. On the quiet reflow, what is lost is
**reporting, not behaviour** — the `rowIds` gate is not quiet, so an unlisted
anchor stays inert and a duplicate keeps its first occurrence — and part of the
silence is deliberate and recorded: §4a says the skip warnings are "silent on
reflow" by design. What no record covers is that `warnMapDomDrift` and
`warnOffsetParentDrift` never re-run at all. The review's third path,
`crates/transync-wasm/demo/sync.js`, does not exist; the byte-identical twin is
`crates/transync-cli/web/sync.js`, pinned by
`crates/transync-cli/tests/sync_js_drift.rs`, so either fix is a two-file commit.

### boundary-checks-weaker-than-contract (OI-0048)

> **RESOLVED 2026-09-04 (R0009-0052 deferred).** Policy: the library refuses where a real caller can reach, and records with a named trigger where none can yet. R0009-0053 landed as `TransyncError::InvalidOptions` with the new stable code `invalid_options`; **the reachability argument inverted in flight** — both roster consumers guard `target_language`, which first read as "unreachable", but the guards exist *because* transync reported the cause as `internal`, so they are downstream compensation for an upstream defect and naming the cause is what retires them. R0009-0075 landed; R0009-0078 landed as its doc half only. R0009-0052 **deferred**: `ProfileConstraints` is `#[non_exhaustive]`, so no external caller can build the unrecognized value. **Re-trigger: a caller-built `ProfileConstraints` becomes constructible, or a consumer reports a silently-whole-block table.** **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0052, R0009-0053,
  R0009-0075, R0009-0078), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0048, reviews/reviewed/0009.md#R0009-0052,
  reviews/reviewed/0009.md#R0009-0053, reviews/reviewed/0009.md#R0009-0075,
  reviews/reviewed/0009.md#R0009-0078
- **Unfiled:** gate registers documents only — queue it through reopen's selection gate
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

#### Description

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

#### Background

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

## Type 3 — blocked

Every blocking reason below was re-checked against the code and the arc's commits at the
2026-08-07 refresh: **none was removed then**, so nothing moved out of Type 3 at that
sweep — three have since (the notes below), and
`oi-0017-detected-language-caching` is a fourth: its named unlock, ticket `f12b8b`, landed
2026-08-09 as DCR-0028, so that entry is marked RESOLVED in place below rather than left
carrying a gate that no longer holds. `provider-capability-set` is a fifth, and the
one to read carefully: its stated condition has **fully fired**, on both limbs —
`fc0304` / DCR-0026 shipped the oversize split on 2026-08-09, and `bda471` / DCR-0029
landed a second in-tree provider on 2026-08-10 — so the entry below is marked
**awake** rather than blocked, and it has no blocking reason left to state. Awake is not
resolved: what the two landings produced is the first real evidence, and the entry
records that the evidence points both ways. With no block left it is a Type-2 item
(needs a decision), marked as such in place rather than moved, per this file's
convention for an entry whose block lifts. _(Corrected 2026-08-16: this paragraph
previously read "half its stated condition fired … the remaining half (`fc0304`) is
still outstanding", which was wrong when it was written — DCR-0026 had been on disk
since 2026-08-09, a day before the `bda471` landing it was reacting to.)_

_Removed 2026-08-08: **`translator-deadline-cancellation`** — the one entry whose block did
lift. Its stated condition ("a long-running service consumer appearing") fired when
`dynwebserver` shipped as a tokio daemon holding transync behind an HTTP endpoint, so the
deferral was re-decided rather than left to expire: ticket `43331a`, **DCR-0024**, which
supersedes DCR-0009's YAGNI entry. This is what a named revisit condition is for, and it is
the shape the remaining Type-3 entries should be read against — their conditions are still
unmet._

_Removed 2026-08-09: **`glossary-section-scope`** — `[[glossary]].scope = "section"` was
rejected with `ProfileError::Unsupported` at three gates (the loader, the translate
boundary, and the `#[doc(hidden)]` `unit::build_batches` entry point, which was fallible
for that reason alone — ticket `0ed6eb`). Its stated block was section-aware batching, and
ticket `43cfb4` / **DCR-0027** built it: batches are section-coherent, an entry names its
sections with a `sections` selector matched against the heading stack, and all three gates
retired **together** with the error variant and with `build_batches`'s fallibility, exactly
as the entry required. ADR-0014's own revisit trigger fired to get here; the entry's
condition is met, not waived._

_Removed 2026-08-09: **`oi-0024-reflow-hooks`** — `sync.js` wired scroll/wheel/touch/
pointer/key listeners and nothing else, so a resized window, a late webfont or a late
image left the two panes describing different places until the reader scrolled again, and
the docstring pushed destroy-and-remount onto the caller. Its stated block was the
commissioned sync-engine pass, and ticket `d3acc3` landed it: a `ResizeObserver` on both
panes, `document.fonts.ready`, and capture-phase `load`/`error` on `<img>` inside either
pane, all coalescing into one animation frame that re-collects both anchor sets and
re-runs the last driving pane's scroll handler. The block was coverage, and the coverage
paid out._

### section-batch-coalescing _(owner-deferred to 2027, 2026-08-09)_

- **Description:** Section-coherent batching (DCR-0027, ticket `43cfb4`) confines every
  batch to one section, so a document of many small sections dispatches **one batch per
  section** where the old packer coalesced them — more requests, more latency, more
  per-request envelope cost, on exactly the heading-rich documents (reference pages, API
  docs, FAQs) where it hurts most. The relaxation is **cohort-restricted coalescing**:
  adjacent *whole* sections may share a batch when their effective glossaries are
  identical, so a co-batched section reads the same prompt it would have read alone —
  glossary exactness is preserved and no section's units are separated. For a
  glossary-free document every section shares one cohort, which restores the old batch
  count almost exactly. Shape if built: an opt-in `[batching].coalesce_sections` knob,
  default off.
- **Background:** DCR-0027's open question OQ-A. Strict confinement is the literal reading
  of the commissioned acceptance criteria, and its sanctioned exception splits a section
  *across* batches — it never merges two sections *into* one — so coalescing needs its own
  decision rather than riding the slice. The asymmetry is why (a) shipped first: (b) is
  cheap to add later and impossible to remove quietly once defaults depend on it.
- **Blocked by:** owner decision 2026-08-09 — **deferred until 2027**. A date rather than
  a condition because the natural condition ("real request-count pain observed") needs a
  consumer running heading-rich documents at volume and no such measurement exists yet;
  the date guarantees the question is re-asked, and the condition, if it fires first, is
  the reason to re-ask early. **No TicGit ticket, deliberately** — TicGit is the queue for
  work someone means to start, and a 2027 deferral is the opposite; this entry is the
  tracker.

### oi-0016-active-block-scan-perf

- **Sources:** ticgit:dd21ad59, `docs/project/open-issues.md#OI-0016`

- **Description:** `activeBlockWithProgress` linearly scans all blocks every RAF frame; a
  sorted-offset cache + binary search is the named alternative, needing reflow
  invalidation and a non-monotonic-layout policy.
- **Background:** OI-0016 (Review 0006, R0006-0090).
- **Blocked by:** explicit profiling gate — "optimize only when profiling shows
  scroll-frame overruns"; no such evidence exists.

### oi-0017-detected-language-caching — **RESOLVED 2026-08-09**

- **Description:** A fully-cache-hit `--source-language auto` run reported
  `detected_source_language: None` because the per-unit cache stored no document-level
  metadata.
- **Background:** OI-0017's one remaining item (R0002-0026), narrowed 2026-07-13 when its
  three siblings shipped (`Translator::fingerprint()` + `CacheKey` provider/validation
  identity, `Cache::evict`, Hard-failure eviction). The 0.3.0 window's `instruction_hash`
  addition and the `AUTO`/`auto` sentinel canonicalization (`aa21e8e`) both touched cache
  identity without touching this.
- **Unblocked and resolved:** the named unlock landed. Ticket `f12b8b` / **DCR-0028** put
  the record on the `Cache` seam itself — `DocumentMetaKey` / `DocumentMeta` and two
  defaulted trait methods, written from a live qualifying envelope and replayed only on a
  run that dispatched zero provider batches (SL-111), made durable across processes by
  `DiskCache` (SL-112/SL-113) and exercised end-to-end by `transync translate --cache-dir`
  (SL-114). `open-issues-archive.md` carries OI-0017 as RESOLVED.

### nested-block-ast-splicing

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: dated owner-delegated decision (2026-08-06): invest at nested-editing time; no consumer needs the splice today. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** The regen splice walks only top-level source ranges; nested
  list/blockquote units ride parent payload coverage. SL-05/SL-06 name AST-aware
  recursive splicing as the successor — required before any finer-grained nested-unit
  editing.
- **Background:** `regen.rs` module-doc limitation; investigation bundle's
  constraints-and-debt table.
- **Blocked by:** decided 2026-08-06 (controller under owner delegation): invest **at
  nested-editing time**, not before — the splice has no current consumer that needs it,
  so the item waits for a nested-editing feature to be commissioned.

### deep-blockquote-fingerprint-depth (R0001-0008 residual)

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: OI-0022's own gate — “extend if drift is observed in practice”; no drift observed. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

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

### html-same-parent-segment-grouping

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: telemetry gate — build only if segment-count rejections dominate ValidationReport data. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** Grouping same-parent HTML text segments via opaque placeholder tokens
  to reduce segment-count rejections; recorded fast-follow, not built.
- **Background:** DCR-0016 decision 6.
- **Blocked by:** telemetry gate — build only if segment-count rejections dominate
  ValidationReport data; no such data yet.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.

### html-native-array-wire-field

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: telemetry gate — build only if double-encoding proves a top validation-rejection cause. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** An additive native-array wire field for HTML payloads as an
  inner-escaping escape hatch; recorded, not built.
- **Background:** DCR-0016 decision 7.
- **Blocked by:** telemetry gate — build only if double-encoding proves a top
  validation-rejection cause.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.

### html-br-normalization-guard

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: DCR-0016's named revisit trigger, behind a telemetry gate. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** The byte-verbatim comparison guard is the named revisit point if models
  normalizing `<br>` vs `<br/>` becomes a top inline-rejection cause.
- **Background:** DCR-0016 revisit trigger.
- **Blocked by:** telemetry gate — no rejection-cause instrumentation evidence.
- **Population note (ti `490d97` wave 7):** every block of an HTML document now rides the
  segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far
  more traffic to fire on.

### per-kind-expansion-factors

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: DCR-0012's deferred refinement, behind an evidence gate. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** Output-aware batching uses one global expansion factor (2.0) for all
  block kinds; per-kind factors (code ≈ 1.0) were deliberately deferred to keep one knob.
- **Background:** DCR-0012 deferred refinement.
- **Blocked by:** evidence gate — revisit only if code-heavy documents demonstrably
  over-split painfully.

### provider-capability-set — **AWAKE: condition fully fired, no block left → Type 2**

- **Description:** No capability descriptor on providers (what strategies/limits a
  provider supports).
- **Background:** DCR-0009 YAGNI list.
- **Unblocked (was: stated revisit condition — a second in-tree provider or the
  oversize-split feature shipping):** **BOTH LIMBS HAVE FIRED.** `fc0304` / DCR-0026
  shipped the oversize row-window split on 2026-08-09, and `bda471` / DCR-0029 landed
  `crates/transync-anthropic` on 2026-08-10, so a second in-tree provider now exists.
  Nothing blocks this entry any more; it is **awake and ready for a decision**, which
  makes it Type-2 work sitting in the Type-3 section — marked here rather than moved.
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

### intersection-observer-active-block

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: rides OI-0016's profiling gate, now ticketed as dd21ad59. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** Whether to replace/augment the scroll-listener engine with an
  IntersectionObserver-based active-block pick.
- **Background:** DCR-0008 open judgment call (OI-0006 third action).
- **Blocked by:** deferred to a future performance pass — rides OI-0016's profiling gate.

### dialect-trait

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: explicit YAGNI condition — a real second dialect must supply constraints first. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

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

### nested-anchor-scheme

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: the hierarchical-alignment revision is uncommissioned; no active demand. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** `sync_role: child-only` and `data-parent-id` are reserved wire values
  never emitted; the `Section` IR type is shape-frozen with no producer, reserved for the
  hierarchical-alignment revision expected to project it from the heading stack.
- **Background:** Archived OI-0005; reserved through DCR-0017 and DCR-0019 precisely to
  allow this without a schema break.
- **Blocked by:** the hierarchical-alignment revision has not been commissioned; no active
  demand.

### wasm-in-cli-bundle

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: ADR-0019's recorded size economics (~41× the bundle's whole JS payload). **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** The wasm renderer ships only as the standalone demo; CLI `--html-out`
  bundle integration is deliberately excluded.
- **Background:** Track C scope across DCR-0017/ADR-0019/DCR-0020.
- **Blocked by:** ADR-0019's recorded size economics — the module is ~41× the bundle's
  entire JS payload for capability the bundle already has; revisit only if that ratio
  changes dramatically.

### live-edit-reanchoring

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: contradicts architectural invariant 8; needs a baseline-level design revision. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** Block-ID survival across structural source edits (insert/delete blocks
  with stable IDs) is unsupported; the wasm demo deliberately never restructures either
  document.
- **Background:** Draft non-goal NG4; mvp-scope DEFERRED table ("still deferred after
  2026-08-05").
- **Blocked by:** contradicts architectural invariant 8 ("document is assumed static") —
  needs a baseline-level design revision before any implementation.

### template-webcomponent-extraction

> **Deferral audited 2026-08-25 (reopen).** Explicitly deferred, and the condition is recorded: stated gate — “revisit only on demonstrated need”; none demonstrated. **Does not count against the v0.5.0 gate**, which excludes work deferred with a recorded decision. Re-counts the moment its named condition fires.

- **Description:** Text inside `<template>` elements (and web-component content) is
  bucketed with `script`/`style` and left untranslated (`transync-html`'s
  segment scan tracks `template_depth` specifically to exclude it).
- **Background:** DCR-0016 / 2026-08-03 design §9.
- **Blocked by:** stated gate — "revisit only on demonstrated need"; none demonstrated.
- **Cross-reference (ti `490d97` wave 7):** an HTML document's `<template>` is
  DEFAULT-STOP in the new intake — a `BlockKind::Html` block extracting zero segments,
  honest `PreservedZeroSegment` row — so the exclusion now has an anchor-bearing
  spelling too. Gate unchanged.

### no-cli-surface-selects-the-second-provider

- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -c transync-anthropic crates/transync-cli/Cargo.toml` returns 0
- **Blocked by:** an owner decision on whether the CLI links a second provider at all (ADR-0002 currently says compile-time choice)
- **Sources:** ticgit:473dd1e0
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`transync-anthropic` implements `Translator` and reads its three environment variables, but `transync-cli` does not depend on the crate, so no flag, environment variable or profile key can select it.

#### Background

This is a dependency-graph fact rather than a missing-argument fact: the binary does not link the crate, so a flag could not reach it even if one existed. The user-visible trap is that someone who already has `ANTHROPIC_API_KEY` exported can reasonably conclude the CLI will use it, and it silently will not — the run goes to OpenAI. ADR-0002 records that the CLI's provider is a compile-time choice, so the current state is intentional, but the manual and the crate's presence together imply otherwise. Blocked on a decision: adding the dependency widens the CLI's build surface and its credential story, and the alternative is documenting the asymmetry loudly enough that nobody is caught by it.

### no-machine-check-of-the-manual-cli-reference

- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -c manual crates/transync-cli/tests/docs_cli_flags_drift.rs` returns 0
- **Blocked by:** a decision on whether the guard belongs on `manual/`'s generated output or on the generator that produces it
- **Sources:** ticgit:7e2fd068
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`docs_cli_flags_drift.rs` guards `contracts.md` §6 and `docs/Developer_Guide.md` only. `manual/reference/user/en/cli.md` is a third full CLI reference with no machine guard.

#### Background

It has already fallen two flags behind (`--cache-dir`, `--table-strategy`) and kept a whole `transync serve` section describing a deferred stub for days after the real server shipped, with nothing failing. `ticgit:0eee5c55` is that same drift, still open — so this gap has a demonstrated cost, not a theoretical one. The reason it is blocked rather than ready is that `manual/` is generated one-way from `docs/` by `write-diataxis-manual`, so a weld pointed at it would pin generated output: either the generator must guarantee the reference, or the weld must be placed at the generator rather than at its product. That is a design decision about where the guard belongs.

### disk-cache-capacity-policy-unreachable-from-the-cli

- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -rc DiskCacheOptions crates/transync-cli/src/` sums to 0
- **Blocked by:** an owner decision between adding a CLI surface and documenting the policy as library-only
- **Sources:** ticgit:650bbba6
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

#### Description

`DiskCacheOptions` carries a byte budget and an entry cap; no CLI surface reaches either, so every `--cache-dir` run takes the defaults with no way to change them.

#### Background

Verified at HEAD: the type is not mentioned anywhere in the CLI's sources. The ticket itself frames this as a decision rather than a defect, and names the two acceptable outcomes — a way to set the policy from the CLI, or an explicit statement in the CLI reference and the DCR that the defaults are fixed for CLI runs and the policy is a library-only knob. Either closes it honestly; leaving it silent does not, because a user who finds `DiskCacheOptions` in the API docs has no way to learn it is unreachable from the binary they are running. Blocked on that choice.

### markdown-island-reclassification

- **Description:** A Markdown document's raw-HTML islands stay `BlockKind::Html` with
  `html-…` ids even where a semantic equivalent exists (an HTML `<table>` island is not a
  `Table`). Reclassifying them moves block **ids**, and with the ids go alignment rows,
  DOM anchors, and the `block_kind` cache axis — every consumer keyed on either.
- **Background:** ADR-0025 D11; spec §13.9.
- **Blocked by:** a schema-2.x-shaped window. The corpus-stability acceptance criterion
  (a document's ids do not move within a minor line) forbids it in any 1.x window, so
  this is not a "when there is time" item — it needs the window first.

### html-oversize-leaf-block-split

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
