# Backlog — WIP / deferred / open items

Maintained by the `/reopen` sweep. Last sweep: 2026-08-05; last full update: 2026-08-07
(the end of the review-0001 hardening arc). This refresh was derived from `ti list --all`
and `git log`, not from this file's own previous text — the arc produced several tickets
whose root cause was one document restating another, and the index must not become one.
Sources: `docs/project/`, `docs/decisions/`, `docs/architecture/`, DCRs, specs, code
markers, `docs/investigation/` (untracked and git-ignored since 2026-08-08 — present on
disk and read as a source, but not version-controlled) and `reviews/` (partly tracked:
round 0001 and `README.md` are committed, later rounds are left untracked until the gate
archives them; see `untracked-analysis-bundles` below).

**Note (2026-08-17):** the `git log` half of that source set no longer exists. This
repository's object store was restarted for the second time on 2026-08-17
(`docs/project/git-history-loss-2026-08-17.md`; the first was
`docs/project/git-history-loss-2026-08-10.md`), so `git log` prints exactly one commit
and **every commit hash quoted below names a commit outside this store**. The entries
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
`[Unreleased]`. `reviews/0001-dispositions.md` carries the per-finding table for the 52
findings of `reviews/0001.md`; `ti list --status closed --tag review-0001` enumerates the
tickets; `CHANGELOG.md` carries the per-ticket entries under `[0.3.0]` and `[Unreleased]`._

---

## Type 2 — needs decision / discussion next

The open design questions here carry an `OI-` id rather than a ticket: a review gate
registers documents only, so each waits for `reopen`'s selection gate to offer it and
file a ticket for whatever is picked. The ticketed questions this section once listed —
`40e2a5`, `66339b`, `1347b4` — were all resolved on 2026-08-10 and are gone from it.

`sync-anchor-injection-via-raw-html` (OI-0035) is about which layer should own anchor
trust in the browser. `out-dir-sparse-bundle-subset` (OI-0036) was settled with `66339b`
by the marker-file rule and is retained only for its record. `provider-payload-intake-guard`
(OI-0037) sits in Type 1 below, since its approach is settled.
`warm-cache-run-needs-no-credentials` (OI-0038) is the newest, from Review 0004, and is
the one that genuinely needs a product answer before any code.

### publish-lock-nested-tree-boundary (ticket `40e2a5`)

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

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0028), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues-archive.md#OI-0036`
- **Unfiled:** gate registers documents only — queue it through `reopen`'s selection gate
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

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0018), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues.md#OI-0035`
- **Unfiled:** gate registers documents only — queue it through `reopen`'s selection gate
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

### provider-payload-intake-guard (OI-0037)

- **Type:** 1
- **Verified:** yes — Review 0003 findings (R0003-0004, R0003-0005, R0003-0088), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues.md#OI-0037`
- **Unfiled:** gate registers documents only — queue it through `reopen`'s selection gate
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

### warm-cache-run-needs-no-credentials (OI-0038)

- **Type:** 2
- **Verified:** yes — Review 0004 finding (R0004-0069), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues.md#OI-0038`
- **Unfiled:** gate registers documents only — queue it through `reopen`'s selection gate
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

- **Description:** Blockquote structural fingerprints recurse one level; structure deeper
  than one level below a blockquote can drift without validator rejection.
- **Background:** Review 0001 finding R0001-0008 — a bare `R0001-` id is the live
  `reviews/0001.md` per `reviews/README.md`'s citation convention. OI-0022 is itself
  RESOLVED (2026-07-13): it recursed the blockquote fingerprint exactly one level, and
  `reviews/0001-dispositions.md` routes R0001-0008 back here as `backlog-tracked`. The
  `ListTopologyEntry.depth` widening (ticket `0d8277`) removed the u8 saturation ceiling,
  which is a different residual and does not deepen the blockquote recursion.
- **Blocked by:** OI-0022's own gate — "extend if drift is observed in practice"; no
  observed drift.

### html-same-parent-segment-grouping

- **Description:** Grouping same-parent HTML text segments via opaque placeholder tokens
  to reduce segment-count rejections; recorded fast-follow, not built.
- **Background:** DCR-0016 decision 6.
- **Blocked by:** telemetry gate — build only if segment-count rejections dominate
  ValidationReport data; no such data yet.

### html-native-array-wire-field

- **Description:** An additive native-array wire field for HTML payloads as an
  inner-escaping escape hatch; recorded, not built.
- **Background:** DCR-0016 decision 7.
- **Blocked by:** telemetry gate — build only if double-encoding proves a top
  validation-rejection cause.

### html-br-normalization-guard

- **Description:** The byte-verbatim comparison guard is the named revisit point if models
  normalizing `<br>` vs `<br/>` becomes a top inline-rejection cause.
- **Background:** DCR-0016 revisit trigger.
- **Blocked by:** telemetry gate — no rejection-cause instrumentation evidence.

### per-kind-expansion-factors

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

- **Description:** Whether to replace/augment the scroll-listener engine with an
  IntersectionObserver-based active-block pick.
- **Background:** DCR-0008 open judgment call (OI-0006 third action).
- **Blocked by:** deferred to a future performance pass — rides OI-0016's profiling gate.

### dialect-trait

- **Description:** A `Dialect` trait + front-end crates abstracting the parser beyond
  GFM/comrak; the syntax-crate split created the boundary but deliberately not the trait.
- **Background:** DCR-0005 Migration/Follow-up, reaffirmed 2026-07-13 and by the
  2026-08-04 syntax-split spec.
- **Blocked by:** explicit YAGNI condition — a real second dialect must supply constraints
  before the trait is designed; none exists.

### nested-anchor-scheme

- **Description:** `sync_role: child-only` and `data-parent-id` are reserved wire values
  never emitted; the `Section` IR type is shape-frozen with no producer, reserved for the
  hierarchical-alignment revision expected to project it from the heading stack.
- **Background:** Archived OI-0005; reserved through DCR-0017 and DCR-0019 precisely to
  allow this without a schema break.
- **Blocked by:** the hierarchical-alignment revision has not been commissioned; no active
  demand.

### wasm-in-cli-bundle

- **Description:** The wasm renderer ships only as the standalone demo; CLI `--html-out`
  bundle integration is deliberately excluded.
- **Background:** Track C scope across DCR-0017/ADR-0019/DCR-0020.
- **Blocked by:** ADR-0019's recorded size economics — the module is ~41× the bundle's
  entire JS payload for capability the bundle already has; revisit only if that ratio
  changes dramatically.

### live-edit-reanchoring

- **Description:** Block-ID survival across structural source edits (insert/delete blocks
  with stable IDs) is unsupported; the wasm demo deliberately never restructures either
  document.
- **Background:** Draft non-goal NG4; mvp-scope DEFERRED table ("still deferred after
  2026-08-05").
- **Blocked by:** contradicts architectural invariant 8 ("document is assumed static") —
  needs a baseline-level design revision before any implementation.

### template-webcomponent-extraction

- **Description:** Text inside `<template>` elements (and web-component content) is
  bucketed with `script`/`style` and left untranslated (`htmlseg.rs` tracks
  `template_depth` specifically to exclude it).
- **Background:** DCR-0016 / 2026-08-03 design §9.
- **Blocked by:** stated gate — "revisit only on demonstrated need"; none demonstrated.
