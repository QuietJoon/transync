---
type: DCR
title: Oversize tables split into header-carrying row windows at packing time
description: A table whose estimated response exceeds the output ceiling no longer aborts the run — the packer splits it into row-window units that each carry the real header, validates every window through the normal layers, and Rust reassembles one table before regeneration. The split is packing, not retrying; default_table_strategy becomes effective and max_split_retries is removed. Design-first record for ticket fc0304 (STUB-017 cluster); slices SL-100..SL-105.
tags: [change, project-control, DCR-0026]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-09T00:00:00Z
status: stable
---

# DCR-0026: Oversize tables split into header-carrying row windows at packing time

- **Date:** 2026-08-09
- **Source:** ticket `fc0304` — owner decision 2026-08-06 to build the STUB-017
  cluster, scheduled first of the six commissioned post-0.3.0 roadmap items.
- **Design-first.** Unlike DCR-0023/0024/0025 this record precedes the code: it is
  the commissioning DCR the repo convention requires before implementation, and its
  slices (SL-100..SL-105, the first use of the reserved SL-100+ range) are the
  implementation plan. Living documents (`contracts.md`, `scenario-matrix.md`,
  module docs) keep describing shipped behavior and are edited by the slice that
  ships each change, not by this record.
- **Affected ADRs:** `docs/decisions/0017-batch-terminal-failures-abort-the-run.md`
  (appended note 2026-08-09 — the "boundary document" consequence narrows to
  non-table kinds; nothing in its decision moves) and
  `docs/decisions/0009-reject-retry-policy-changes.md` (appended note 2026-08-09 —
  a split is packing, not retrying; the verbatim rule is untouched). No new ADR:
  this record introduces no new principle — it executes ADR-0017's stated
  prevention-side doctrine on the one case prevention could not yet reach.
- **Affected contracts (edited by the slices, listed here as the map):**
  `contracts.md` §1 (the `table_row_window` payload semantics stop being
  "reserved"), §2 (`default_table_strategy` effective; `max_split_retries` row
  deleted), §3a (window rows in the validation report; report schema minor bump),
  §5 (the "advisory only / DEFERRED" paragraph replaced by the splitter's rules),
  §6 (`--table-strategy` flag; preflight remediation wording). Plus
  `scenario-matrix.md` SCN-03 and `stub-manifest.md` STUB-017.
- **Breaking.** Carried by **v0.4.0**, under the open breaking window:
  `transync::InputMode::TableRowWindow`'s fields are redefined, a `ProfileBatching`
  field is removed, and the embedded default profile changes behavior for runs that
  today abort. No §0 row is added or removed, so `public_surface.rs` does not move.

## The problem being solved

Output-aware packing (DCR-0012) can shrink a batch but cannot shrink a unit. A
single block whose estimated response exceeds the effective output ceiling is
packed alone, flagged by the `OutputBudgetWarning` preflight, dispatched anyway,
truncated by the provider, and the run aborts whole (ADR-0017) — with raising
`--target-output-tokens` the only remedy. ADR-0017 records this as the accepted
"boundary document" cost *until the deferred oversize split lands*. This DCR is
that split, for the one kind whose sub-structure the application already owns and
validates: **tables**. Everything about it is prevention-side — the split happens
before the first provider call, deterministically, and the abort semantics for
whatever still cannot fit are unchanged.

## What changes

### 1. The split: packing-time, output-axis, tables only

`unit::build_batches` gains one step between unit construction and instruction
assembly: when the profile sets `[batching].target_output_tokens` **and** the
effective table strategy is `row-window-first`, every `BlockKind::Table` unit
whose estimated response exceeds the **effective output target** is replaced, in
place and in document order, by row-window units. The effective target is the raw
ceiling minus `OUTPUT_ENVELOPE_RESERVE_TOKENS` floored at 1 — the same number the
packer and the preflight derive, and with this a **third** derivation site exists,
so the computation moves into one shared helper all three call. The estimate uses
the same encoder (`Translator::tokenizer_hint`, else the `model_id` heuristic) and
the same resolved `output_expansion_factor` as the packer, so the three sites
cannot disagree about what "oversize" means.

The split runs **once per run, before round one**. It is never reactive: a
provider-side oversize signal after packing remains a terminal error exactly as
ADR-0017 settled it. (A reactive re-split was considered and rejected — it would
require the per-batch fallback rung ADR-0017 explicitly rejected, and it would
make packing nondeterministic across retry rounds, which R0001-0012's re-packing
rules assume it is not.)

When the ceiling is unset, or the strategy is `whole-block`, or the block is not a
table, nothing changes: the unit ships whole, the preflight warns if it is
oversize, and the run aborts at the provider as today.

### 2. What a window is

A window is a **complete, valid GFM table**: the source table's header row, its
delimiter row, and one contiguous run of body rows in source order. The header
rides with every window — this is architectural invariant 3's sanctioned shape
(row windows carrying header context), and nothing below ever produces an
isolated cell or a headerless row fragment.

- **Source-side slicing** is a `transync-syntax` concern:
  `regen::split_table_rows(source: &str)` parses the block with comrak and returns
  the header line, the delimiter line, and the body-row lines, or `None` when the
  payload does not parse as a single table (the splitter then leaves the unit
  whole and the preflight warning stands).
- **Sizing** is greedy: body rows accumulate into a window until adding the next
  row would push the window's estimated response (header + delimiter + rows,
  encoded, times the expansion factor, plus the per-unit output overhead) past
  the effective target. Minimum one body row per window. A window that exceeds
  the target even with one row (a single giant row) is kept as-is: the preflight
  names it and ADR-0017's abort applies — that floor is the design's stated
  boundary, not a defect.
- **Unit shape.** Each window is an ordinary `TranslationUnit`:
  - `unit_id`: `<parent-id>.wNN` — the parent's ADR-0005 id, a literal `.w`, and
    a zero-padded two-digit window ordinal (widening on demand past 99, like
    ADR-0005's numbers). `.` cannot occur in an ADR-0005 id, so collision with a
    real block id is impossible. Window ids appear on the wire and in the
    validation report only — **never** in the alignment map or the DOM.
  - `block_kind`: `Table`. `source_payload`: the window's markdown.
  - `input_mode`: `TableRowWindow { parent_block_id: BlockId, window_index: u32,
    window_count: u32 }` — the variant's fields are **redefined** (the reserved
    `header_markdown`/`rows_markdown` shape was never produced and dies unused;
    breaking, rides the open window). The payload carries the mini-table; the
    variant carries only what merge and eviction need, which also makes the whole
    split plan reconstructible from the batches — `build_batches`' signature does
    not change.
  - `constraints`: from `inspect_table` over the **window** payload — same column
    count and alignment as the source by construction, `must_preserve_table_row_count`
    equal to the window's own body-row count.
  - `context`: the parent block's `BlockContext`, unchanged. `source_hash`: the
    parent's.
- **The wire.** Providers already receive the `"table_row_window"` label
  (`llm::prompt::input_mode_label` has emitted it since the variant was reserved);
  its payload contract is now stated: a complete GFM table, translated whole,
  same rules as `full_table_markdown`. The user-message instruction gains one
  conditional clause for row-window units (header repeats across sibling windows
  for context; translate the whole mini-table; keep terminology consistent),
  assembled by `llm::prompt` and reserved off the packing budget on a
  document-level fact — the exact mechanism the html-segment clause uses
  (`InstructionVariant`), and covered by the `instruction_hash` cache axis
  automatically. A tier-(b) provider that calls `build_user_prompt` needs no code
  change.

### 3. Validation: per window, plus one merge check

Every window passes through the normal layers unchanged — schema/ID echo, per-kind
table checks against the window's own constraints, fragment reparse (a window is a
table, so it reparses as one), inline protection. Retries are per window (see §5).

One check is added at merge time: the reassembled table is re-inspected
(`inspect_table`) and must match the **source block's** column count, alignment,
and total body-row count before the parent enters `accepted`. The merged block
then faces the full-document reparse like every other block. Layered validation
(invariant 5) is preserved end to end: no window result reaches the document
without both its own fragment proof and the whole-table proof.

### 4. Reassembly: `regenerate_table` gets its real body

A new pipeline step between `aggregate_batch_results` and
`finalize_regen_with_reparse_policy` groups window results by `parent_block_id`
and calls the retained plug-in point, which stops being an identity function:

```rust
// transync-syntax, regen — STUB-017's exit condition
pub fn regenerate_table(windows: &[&str]) -> String
```

Window 0's translated output is taken verbatim; each subsequent window contributes
its lines **after** its delimiter row. Window 0's translated header is canonical;
later windows' headers were context and are discarded, so divergent header
translations across windows are harmless by construction. (Purely textual
splicing is safe here because every input already passed fragment reparse as a
single table and the merged result is re-inspected per §3.)

The merged parent `ValidatedUnit` **replaces** its window entries in `accepted` —
window ids must never reach `regen::regenerate`, the alignment map, or the
renderer; the alignment map still carries exactly one row per source block and its
schema version does not move. Bookkeeping that follows from this:

- **Report.** Windows keep their own `UnitValidationRecord` rows — they are real
  dispatches with real attempts — alongside the parent's row carrying the merged
  `final_status`. `order_report_by_document` ranks a window at its parent's
  document index (id-string tiebreak orders `t-0007` before `t-0007.w01`), so
  windows list directly under their parent. `total_retries` counts window retries;
  this is a new row shape a consumer may meet, so
  `VALIDATION_REPORT_SCHEMA_VERSION` takes a **minor** bump and §3a documents the
  window-id convention. (`VALIDATION_SCHEMA_VERSION` — the cache axis — does
  **not** bump: no existing payload's semantics change, whole-block cache entries
  stay valid, and window payloads mint fresh keys through the ordinary
  prompt-bytes rule.)
- **Status math.** All windows `Translated` → parent `Translated`. Any window
  `PartiallyTranslated` → parent `PartiallyTranslated`. Any window terminally
  failed → **open question OQ-1** below.
- **Merge engine fault.** If the merged fragment fails the §3 check even though
  every window passed (an engine bug, not a model fault), the parent takes the
  DCR-0016 direct-fallback shape: `fallback_source` with `rejected_by` and
  `rejection_reason` both `None`, a row on the warnings channel, no retry burned —
  and every window's cache key is evicted so the state is not replayed.
- **Eviction.** A full-reparse cascade downgrade or `FullReparseFailure::Hard`
  that names the parent block evicts **every window key** of that parent (the
  parent itself was never dispatched and has no key). The parent→window mapping
  comes from the `TableRowWindow` fields.
- **Cache.** Windows cache individually under the standard identity rule — the
  prompt bytes are the key, nothing new is added. Consequence, stated: window
  boundaries follow the ceiling, the expansion factor, and the encoder, so
  changing any of them reshapes windows and orphans their entries. That is the
  ordinary cost of a content axis and is accepted.

### 5. The retry budget: a split is packing, not retrying

This is the ADR-0009 interplay the commission requires stated exactly:

- **A split is not a retry.** It happens before round one, changes no in-flight
  unit, and consumes no budget — it is the same kind of act as
  `group_by_token_budget` deciding batch boundaries.
- **From birth, a window is an ordinary unit.** Its scope and payload are fixed
  for the whole run. A validation failure re-dispatches **that window verbatim**
  — same payload, same scope, `RetryContext` side channel only — charged to
  `max_per_unit_validation_retries` per window. Batch-fault and transient budgets
  apply to windows exactly as to any unit. The §5 round bound
  `1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries` holds
  with `U` counting windows.
- **There is no re-split.** No failure, provider signal, or retry round ever
  changes a unit's scope after packing. ADR-0009's verbatim-resubmission rule is
  therefore untouched rather than amended.

### 6. The knobs: one becomes effective, one is removed

- **`[constraints].default_table_strategy` becomes effective.**
  `"row-window-first"` enables the packing-time split; `"whole-block"` disables it
  (today's behavior: preflight warning, provider abort). The "reserved" load
  warning is deleted; the unknown-value warning stays. The embedded default
  profile flips to `"row-window-first"`: the only runs whose behavior changes are
  runs that today abort — the split converts a guaranteed failure into a
  translated document and changes nothing for any table that fits. The knob is
  not a cache axis (the window payloads it produces are, via the ordinary rule).
- **`[batching].max_split_retries` is removed** — the field, its `default.toml`
  line, its allowlist entry (an old profile carrying it gets the standard
  unknown-key load warning), and its contracts rows. Under a deterministic
  packing-time split there is no "split retry" for it to bound, and the
  commission forbids leaving it parsed-and-ignored. Removal, not repurposing: a
  knob whose name promises retry semantics must not silently become a window
  count.
- **CLI:** `--table-strategy <whole-block|row-window-first>` joins the DCR-0012
  batching flags with the same precedence (flag > profile > built-in default),
  and the preflight's remediation text names it (alongside
  `--target-output-tokens`) when the flagged unit is a table.

### 7. Oversize NON-table units: documented exclusions, per kind

The split covers tables only. Every other kind is excluded **by decision**, each
on its own ground, and keeps today's semantics exactly (preflight names the block;
ADR-0017 abort; remedy is raising the ceiling or splitting the source):

| Kind | Decision | Ground |
|---|---|---|
| Paragraph / heading text | excluded | Sub-paragraph boundaries are sentence segmentation — a **content** decision, which invariant 2 assigns to the LLM, not the application. Rust owns no structural seam inside a paragraph. |
| Code block | excluded | Invariant 4: the LLM receives and returns the **whole** fenced block; a fence fragment is not a valid unit and safe re-fencing is defined only over whole blocks. |
| List item | excluded | Units are already per item; `expected_list_topology` is a whole-unit constraint, and an item's subtree has no seam that preserves it. |
| Blockquote | excluded | `expected_blockquote_children` is a whole-container constraint; splitting the container breaks the child-sequence proof. |
| Html block | excluded (evidence-gated follow-up) | Segment-subset windows would need splice-by-parts in `htmlseg`; DCR-0016's splice contract is all-segments-at-once. Revisit only on evidence of real oversize html blocks. |
| ThematicBreak / Image / Skipped | n/a | Never translatable units. |

Consequence for ADR-0017: its "boundary document" bad-consequence **narrows** to
these kinds; it is not deleted. The appended note on that ADR records exactly
this.

### 8. SCN-03 becomes live-verifiable

The scenario stops being satisfied by the whole-block workaround. The rewritten
`scn_03_table_large.rs` sets an output ceiling low enough that the 200-row table
must split, runs the passthrough mock, and asserts: more than one
`table_row_window` unit was dispatched (recording translator), the regenerated
table has 200 body rows × 4 columns in source order, the table's alignment row is
`Translated`, and the alignment schema version is unchanged. A negative path
exercises a window that exhausts its retries and asserts the OQ-1 outcome. The
scenario-matrix SCN-03 row is rewritten and its "stub-verified-only until the
oversize-split lands" footnote is retired by the slice that ships this.

## Open question — OQ-1: what a terminally-failed window does to the table

Two defensible shapes with materially different consequences. **This DCR
deliberately does not pick**; the owner decides, and SL-103/SL-105 implement the
answer. Nothing in SL-100..SL-102 or SL-104 depends on it.

- **(a) All-or-nothing.** Any window that finalizes `fallback_source` makes the
  **whole table** `fallback_source` — the emitted block is the pristine source
  table. Preserves the strict reading that `fallback_source` means "this block is
  the source content, exactly." Cost: up to N−1 successfully translated windows
  are discarded on a 200-row table because one window failed.
- **(b) Partial merge.** Failed windows contribute their **source** rows to the
  merged table; the parent is marked `PartiallyTranslated` — a status that already
  exists on every wire surface (alignment map §3, `data-fallback` attribute §4,
  `ValidationSummary` tally, report). Preserves the paid-for majority; the failed
  windows are named in the report. Cost: the mixed rows are marked at block level
  but are not visually distinguishable row-by-row in raw Markdown.
- **Recommendation: (b).** The project already sanctions honest per-unit
  degradation with a marker (invariant 6); the window is the unit here, and
  `PartiallyTranslated` is the existing marker for exactly this state. ADR-0017's
  "quietly-degraded document" objection targeted **unmarked** degradation; both
  shapes here are marked. (a) discards work without adding any honesty (b) lacks.

### Implementation deviation — `source_hash` is the window's own, 2026-08-09 (appended note)

§2 lists a window's `source_hash` as **the parent's**. Implemented as the hash
of the **window's own payload bytes** instead, because §4's own requirement —
"windows cache individually under the standard identity rule" — is impossible
under the field as §2 wrote it.

`CacheKey` has no unit-id axis (deliberately: that is the cross-block dedup the
cache exists for), and `cache.rs` states the content rule as *if it changed the
prompt bytes the model saw for this unit, it is identity*, with `source_hash` +
`block_kind` covering "the unit's own body". Windows of one table share their
parent's kind, context, language labels, profile and instruction, so
`source_hash` is the only axis left that can separate them. With the parent's
hash they collide: **every window after the first reads window 1's entry out of
the run's own cache**, and the merged table repeats window 1's rows. This was
observed, not theorized — the first end-to-end SCN-03 run produced a 200-row
table in which twelve windows resolved to two distinct payloads.

Hashing the window's own bytes is the same statement the whole-block path
already makes (one unit, one body, one hash) and changes nothing else: the
merge, the eviction mapping, and the report all key off ids, and the parent's
own hash is never needed after packing. Pinned by
`unit::split::tests::each_window_hashes_its_own_body_so_the_cache_cannot_alias_them`.

### OQ-1 resolved — (b), 2026-08-09 (appended note)

**Implemented as (b)**, this record's own recommendation, in SL-103. The owner
had not answered when the implementation slices ran, and SL-103/SL-105 could
not be built without an answer; taking the recommendation rather than inventing
a third shape is the choice that leaves this record the thing the next reader
can trust. **If the owner prefers (a), the reversal is local**:
`pipeline::merge::merge_status` plus the source-rows arm of `merge_row_windows`
in `crates/transync-core/src/pipeline/merge.rs`, and the SCN-03 negative-path
assertions.

Two details the fork did not spell out, settled the same way and for the same
reason — the marker must be true:

- **All windows failing is `fallback_source`, not `partially_translated`.** The
  block then really is its source content exactly, which is the strict reading
  `fallback_source` promises and is also what the whole-block path produces for
  the same table. It carries `accepted_payload: None`, the whole-block
  convention regen relies on to splice source bytes byte-identically (the
  DCR-0004 cascade's "structurally identical by construction" argument).
- **Any other mixture is `partially_translated`**, including a mix of
  `translated` and `preserved` windows. Unanimity carries; nothing else does.

The failed windows are named on the parent's report warnings channel, so
"which rows are source" is answerable from the artifact even though raw
Markdown cannot distinguish them row by row — the cost (b) was accepted with.

## Rules the implementer must not violate

1. **Never an isolated cell, never a headerless fragment.** Every window is a
   complete GFM table carrying the real header (invariant 3). Whole-block stays
   the shape for every table that fits the ceiling.
2. **The split is deterministic and happens exactly once, pre-dispatch.** No
   re-splitting on any signal; no scope change to any unit after packing
   (ADR-0009). Retries of a window are verbatim.
3. **Window ids never reach `regen::regenerate`, the alignment map, or the DOM.**
   One alignment row per source block; `data-sync-id` vocabulary is unchanged
   (invariant 1).
4. **One "effective output target" helper.** Packer, preflight, and splitter must
   call the same derivation with the same encoder and factor; a fourth private
   copy is the bug class this repo keeps paying for.
5. **`transync-syntax` stays clean:** no `[features]`, no `transync-core`
   dependency (dev-deps included); `split_table_rows` / `regenerate_table` are
   pure functions over strings + comrak. The wasm gate
   (`cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`)
   stays green on every slice.
6. **§0 weld:** no facade row changes are expected; if a slice does add or remove
   an exported name, `contracts.md` §0 and `public_surface.rs` move in the same
   commit.
7. **ADR-0017 is not relitigated.** No fallback rung, no policy flag for
   provider-terminal errors; the abort stays the terminal behavior for everything
   this DCR does not split.
8. **Living docs move with the slice that ships the behavior**; this DCR and the
   ADR notes are the only records that describe the future.
9. No version bumps or tags; CHANGELOG entries go under `[Unreleased]`. Every
   `cargo test` invocation carries `-- --test-threads=4`.

## Out of scope

- Splitting any non-table kind (each excluded above with its ground).
- Reactive / provider-signal splitting — **rejected**, not deferred (§1).
- Input-axis-triggered splitting: the trigger is the output ceiling only, the
  axis whose overflow provably aborts; the input target is a soft packing cap.
- Row-window batching for tables that fit the ceiling.
- Section-aware batching (its own commissioned ticket) and per-kind expansion
  factors (Type 3, evidence-gated).
- `htmlseg` splice-by-parts (the html exclusion's follow-up).

## Implementation slices

Each slice is independently landable and testable, lands with its own tests plus
the living-doc edits for what it ships, and runs the full standing gates.

- **SL-100 — syntax layer: slice and reassemble.**
  `transync-syntax::regen::split_table_rows` (comrak-verified header/delimiter/
  body-row slicing; `None` for non-tables) and the real
  `regenerate_table(windows: &[&str]) -> String` body (window 0 verbatim; later
  windows minus header+delimiter), retiring the STUB-017 identity marker.
  Property tests: split→merge over the identity translation reproduces the source
  table for tables with escaped pipes, multibyte cells, and mixed alignment.
  No pipeline wiring; wasm gate proves the crate rules hold.
- **SL-101 — core types and wire.** Redefine
  `InputMode::TableRowWindow { parent_block_id, window_index, window_count }`;
  the row-window instruction clause in `llm::prompt` (`InstructionVariant` grows
  the document-level has-row-window fact, reserved like the html clause); packer
  envelope pricing covers the clause; contracts §1 states the payload semantics.
  Tests: instruction assembly + estimator pricing; schema/parse behavior, never
  string equality (tier-(b) carve-out).
- **SL-102 — the splitter.** The shared effective-output-target helper; the
  packing-time split in `unit::build_batches` (after profile normalization,
  before instruction assembly), gated on the ceiling and on
  `default_table_strategy` — which becomes **effective** in this slice while the
  shipped default stays `whole-block` until SL-104. Window unit construction
  (ids, constraints via `inspect_table`, parent context). Tests: no split when
  ceiling unset / strategy whole-block / table fits; deterministic window
  boundaries; giant-single-row window still warns; windows pack through
  `group_by_token_budget` like ordinary units.
- **SL-103 — merge and bookkeeping.** *(Blocked on OQ-1.)* The post-aggregation
  merge step: group by `parent_block_id`, `regenerate_table`, the §3 merged-
  fragment check, parent `ValidatedUnit` replacing window entries, OQ-1 policy,
  merge-fault direct fallback + eviction, parent-downgrade → window-key eviction,
  window report rows + document-order ranking, `VALIDATION_REPORT_SCHEMA_VERSION`
  minor bump, contracts §3a/§5 edits. Tests: all-success merge; per-window
  retry-then-success; OQ-1 outcome; merge-fault shape (`rejected_by: None` +
  warning); eviction targeting.
- **SL-104 — knobs and CLI.** Default profile flips to `row-window-first`;
  `max_split_retries` removed everywhere (field, default.toml, allowlist,
  contracts §2/§5); `--table-strategy` CLI flag; preflight remediation names the
  strategy for table units; contracts §6. Tests: precedence flag > profile >
  default; removed-knob load warning; CLI stub suite.
- **SL-105 — SCN-03 live path and record closure.** *(Negative path blocked on
  OQ-1.)* The rewritten `scn_03_table_large.rs` (split actually exercised, ≥2
  windows, 200×4 round-trip, alignment row Translated) plus the negative path;
  scenario-matrix SCN-03 row rewritten and the stub-verified-only footnote
  retired; `stub-manifest.md` STUB-017 row closed with this DCR and the slice
  numbers; CHANGELOG under `[Unreleased]`.

## OQ-1 ratified — (b), by the owner, 2026-08-09 (appended note)

The owner ratified **(b)**, the shape SL-103 implemented and this record
recommended: a terminally-failed window contributes its **source** rows and the
parent table is marked `PartiallyTranslated`.

This closes the loop the previous note left open. That note recorded the
implementer taking this record's own recommendation because SL-103/SL-105 could
not be built without an answer; the decision is now the owner's, not a
default, and the two boundary cases settled with it stand as written —
all-windows-failed is `fallback_source` with `accepted_payload: None`, and
every other mixture, including translated+preserved, is `PartiallyTranslated`.

The reversal points named in the previous note are no longer a pending option
and should be read as history rather than as an offer.

## The other aliasing case — a window versus a *whole* table, 2026-08-10 (appended note)

The `source_hash` deviation note above closed window-versus-window aliasing.
Ticket `5f7942`, raised from dynwebserver, found the complementary case, and it
was this record's own §2 shape that made it reachable.

A window's payload is a **complete** table, deliberately shaped exactly like
the payload a whole table sends — same header, same delimiter row, no trailing
newline. So a small whole table and the opening window of a big one that starts
with the same row are byte-identical, and the window's own-bytes `source_hash`
(the deviation above) equals the small table's. `block_kind` is `table` for
both. A window inherits its parent's `BlockContext`, so two tables flanked by
neighbors whose 120-character summaries match agree on `context_hash`. And a
first window small enough to be packed beside that small table agrees on all
three batch-scoped axes — including `instruction_hash`, because the row-window
clause rides on the batch, not on the unit. Every axis agreed; the two prompts
differed only in the per-unit `input_mode` label, which was not an axis.

Fixed by adding `input_mode` to `CacheKey` in the v0.4.0 window (contracts.md
§5a, *The mode axis*), not by changing anything this record specified. Nothing
above is retracted: the deviation note's reasoning about windows of one table
still holds exactly as written — they share a mode label, so `source_hash`
remains the only axis that separates *them*.

The reachable document is pinned by
`pipeline::run_level_tests::a_row_window_and_a_whole_table_are_not_one_entry`.
