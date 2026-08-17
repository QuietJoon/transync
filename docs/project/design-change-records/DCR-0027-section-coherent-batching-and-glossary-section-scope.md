---
type: DCR
title: Batches become section-coherent, and the glossary section scope becomes real
description: The batcher partitions units at heading boundaries and packs each section on its own, falling back to sequential packing inside a section that exceeds the budget; the per-batch envelope reserve prices each section's own compiled prompt. That makes per-section glossary filtering cheap, so scope = "section" gains a sections selector matched against the heading stack, renders only into its sections' prompts, and its three-gate ProfileError::Unsupported rejection is retired end to end. Design-first record for ticket 43cfb4; slices SL-106..SL-109.
tags: [change, project-control, DCR-0027]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-09T00:00:00Z
status: stable
supersedes: decisions/archive/0014-section-scoped-glossary-renders-globally.md
---

# DCR-0027: Batches become section-coherent, and the glossary section scope becomes real

- **Date:** 2026-08-09
- **Source:** ticket `43cfb4` — owner decision 2026-08-06 to build section-aware
  batching, scheduled second of the six commissioned post-0.3.0 roadmap items,
  immediately after `fc0304` (DCR-0026) because both reshape the batcher. The same
  commission implements `[[glossary]].scope = "section"` acceptance end to end,
  which unblocks the Type-3 backlog item **`glossary-section-scope`** — that
  backlog entry is retired by the slice that lands the acceptance (SL-109 closes
  the record).
- **Design-first.** This record precedes the code; its slices (SL-106..SL-109,
  continuing DCR-0026's SL-100+ range) are the implementation plan. Living
  documents keep describing shipped behavior and are edited by the slice that
  ships each change, not by this record.
- **Affected ADRs:** `docs/decisions/archive/0014-section-scoped-glossary-renders-globally.md`
  (appended note 2026-08-09, committed with this record — its own revisit trigger
  fires: section-aware batching provides the reliable unit→section mapping at
  batch-assembly time it named, so the time-boxed rejection is re-admitted on the
  design below). No new ADR: this record introduces no new principle — it
  executes the revisit plan ADR-0014 wrote down in 2026-07, and the batching
  policy is the enabling mechanism that plan presupposed.
- **Affected records beyond the ADR:** the EXT-2026-07 wave-close baseline
  `docs/project/design-baseline-2026-07.md` (its pipeline line carries
  "section-awareness deferred"; snapshot doc, so it gets a dated appended note —
  scheduled into SL-109, when the sentence stops being true).
- **Affected contracts (edited by the slices, listed here as the map):**
  `contracts.md` §2 (the `[[glossary]]` schema gains `sections`; the scope bullet's
  three-gate rejection paragraph is replaced by the filtering semantics; the
  claimed-once rule and the auto-glossary merge rule are refined; the §2
  "what the budget covers" paragraph moves to per-section pricing), §5 (the
  packing paragraphs state the grouping policy), §5a (the two profile-derived
  hashes become batch-scoped). Plus `scenario-matrix.md` SCN-09/SCN-10 rows,
  `mvp-scope.md`'s deferral line, `batch.rs`'s module doc (its "There is NO
  section logic here" header), `docs/architecture/README.md`'s two batcher
  labels, `docs/Developer_Guide.md`'s scope-rejection paragraph, and
  `docs/backlog.md`'s `glossary-section-scope` Type-3 entry.
- **Breaking.** Carried by **v0.4.0**, under the open breaking window:
  `GlossaryEntry` gains a public field (struct literals break),
  `ProfileError::Unsupported` and `ProfileMetadata::ensure_supported` are
  removed, `unit::build_batches` (`#[doc(hidden)]`) becomes infallible, and the
  default packing behavior changes for every heading-bearing document. No §0 row
  is expected to move — `ProfileError` keeps its type row and methods are not
  rows — but the weld rule stands regardless: any §0 change lands with
  `crates/transync/tests/public_surface.rs` in the same commit.

## The problem being solved

Two halves, deliberately one commission because the second is gated on the first:

1. **The batcher is sequential and section-blind.** `batch::group_by_token_budget`
   packs units in document order until a budget or the unit cap binds, so batches
   freely straddle `##`/`###` boundaries — the module doc says so in capitals.
   Section-aware grouping has been designed-in-name-only since the MVP
   (`mvp-scope.md` defers it; ADR-0014 as amended names it as its revisit
   trigger).
2. **`scope = "section"` is rejected, not implemented.** A section-scoped
   glossary term rendered into every batch's prompt would steer every section —
   the misleading behavior EXT-2026-07 P0-3 reversed — so `load_profile`, the
   translate boundary, and `unit::build_batches` all raise
   `ProfileError::Unsupported` for it (three gates; ticket `0ed6eb`, backlog
   `glossary-section-scope`). The rejection was always a time-boxed deferral:
   ADR-0014 says to re-admit the scope the moment a reliable unit→section mapping
   exists at batch-assembly time.

The mapping has in fact existed at the *parser* level all along —
`parser::Block.section_path` is the heading-id stack, and every unit's
`BlockContext.section_path` already crosses the wire and feeds `context_hash`.
What was missing is a batcher that keeps a batch inside one section, so that
"which glossary entries apply to this batch's prompt" has a single well-defined
answer.

## What changes

### 1. The grouping policy — precise enough to test

`unit::build_batches` stops handing the whole unit list to one
`group_by_token_budget` call and instead partitions first, packs second:

- **P1 — Partition.** Units, in document order, are partitioned into
  **sections**: a new section starts at unit index 0 and at every unit whose
  `block_kind` is a heading (any level 1–6). **A heading unit belongs to the
  section it opens.** This rule is stated at the unit level because the parser
  stamps a heading's own `section_path` *excluding* itself
  (`sections::close_through` is captured before `open_scope`); raw
  path-equality would dangle every heading off the section it closes. A
  section's **heading stack** — the identity everything below keys on — is the
  stack in effect for its body blocks: the opening heading's own path plus that
  heading; for the preamble section (units before any heading), the empty
  stack.
- **P2 — Confinement.** Every batch's units come from exactly one section.
  Batches never straddle a section boundary — including the whole-section
  coalescing case, which is the open question OQ-A below and is **not**
  implemented by this DCR.
- **P3 — Packing within a section.** Inside one section, packing is today's
  greedy dual-budget sequential algorithm (`group_by_token_budget`, unchanged),
  with the per-section envelope reserve of §3. Testable consequence: a section
  whose units' estimates plus the reserve fit the input target, whose estimated
  responses fit the effective output target, and whose unit count fits
  `max_units_per_batch`, yields **exactly one batch**.
- **P4 — The oversize-section fallback.** A section that cannot fit one batch
  yields several, all confined to that section — this is the commissioned
  fallback ("today's sequential packing WITHIN that section"), and it is the
  only way a section's units are ever separated. `OutputBudgetWarning` and the
  single-oversize-unit ADR-0017 abort semantics are untouched.
- **P5 — Determinism.** The partition is a pure function of the unit list; it
  runs once per run, before round one, and nothing downstream re-partitions
  (ADR-0009: packing stays deterministic across retry rounds; the retry packer
  re-packs *within* a batch, and a batch is single-section by P2).
- **P6 — Batch ids.** Batches keep their global document-order numbering
  (`BatchId` 1..N across sections); nothing about batch identity changes shape.

### 2. Composition with DCR-0026 — split first, partition second

The row-window splitter and the section partition compose by **ordering**, not
by interaction:

- `split::split_oversize_tables` keeps running where it runs today — on the
  flat unit list, before instruction assembly and before any packing. Window
  units inherit the parent's `context` unchanged (DCR-0026 §2), so **every
  window carries the parent table's `section_path` and lands in the parent's
  section** by construction. A section-coherent batch that would contain an
  oversize table never sees it: the splitter already replaced it with windows
  by the time the partition looks.
- The two mechanisms answer different axes: the splitter is **output-axis,
  tables only** (a unit too big to answer); the P4 fallback is **any-axis,
  section-level** (a section too big to co-batch). An oversize section
  containing an oversize table gets both, in that order: the table becomes
  windows, then the section's units — windows included — pack sequentially
  within the section.
- The instruction envelope keeps its DCR-0026 shape: `InstructionVariant` is
  computed once per run from `DocumentFacts::of(&units)` **after** the split,
  and both membership-driven clauses (html, row-window) stay document-level
  facts (owner decision on ticket `aa92d6`). Section awareness adds **no**
  instruction clause — the glossary is system-prompt-side.

### 3. The reserve arithmetic moves with the section (the `aa92d6` interplay)

Ticket `aa92d6` settled that everything shipping once per batch is *encoded*
from the real strings, and that clauses depending on batch membership are
priced on a document-level fact because membership is what packing decides.
Section cohorts do **not** have that circularity: a unit's section is known
*before* packing, from `section_path`. So the reserve gets more exact, not more
conservative:

- `envelope_input_tokens` keeps its contract, but the `system_prompt` it
  encodes becomes **the section's own compiled prompt** (§5): one
  `BatchBudget` per section, differing from its siblings only in
  `system_prompt` (and the reserve derived from it). The instruction envelope,
  the two labels, and every per-unit estimate are unchanged.
- The retry packer already prices from `batch.profile.prompt_body`
  (`pipeline/dispatch.rs`, the per-batch re-pack) — with per-section profiles
  riding each batch, the same-budget-both-rounds rule (R0001-0012) holds with
  **zero** seam change there: a batch is single-section, so its budget is the
  same object in round one and round N.
- The containment test (`the_reserve_plus_the_per_unit_estimates_bound_the_whole_request`)
  extends to assert per section: each batch's reserve bounds the request built
  from **that batch's** profile.

### 4. Glossary section scoping — the semantics that make `scope = "section"` mean something

`GlossaryEntry` gains the selector the scope always lacked:

```toml
[[glossary]]
source = "cell"
target = "셀"
scope  = "section"
sections = ["Tables", "Spreadsheet Reference"]
```

- **G1 — Shape.** `sections` is an array of heading-text selectors
  (`#[serde(default)] pub sections: Vec<String>` on `GlossaryEntry`; TOML/JSON
  key `sections`, added to the `[[glossary]]` key allowlist). For
  `scope = "section"` it is required non-empty; for `scope = "global"` a
  non-empty `sections` draws a path-qualified load warning and is cleared
  (normalize-and-warn, the house style for keys that cannot mean what they
  say).
- **G2 — Selector normalization.** Selectors are compared trimmed and
  case-folded — the same identity `glossary_key` already uses for source
  terms. An empty/whitespace selector is dropped with a warning; a
  `scope = "section"` entry whose selector list ends up empty is dropped with
  a warning (it cannot mean what it says). Duplicate selectors within one
  entry are deduped with a warning. All of this lives in
  `profile::normalize_glossary`, so the existing three warn-door structure
  (loader records; translate boundary and `unit::build_batches` normalize) is
  inherited rather than rebuilt.
- **G3 — Applicability (heading-stack matching).** A section-scoped entry
  **applies to a section** iff any heading in that section's heading stack
  (§1 P1 — the opening heading included, levels ignored) matches any of the
  entry's selectors under G2's normalization. Matching the *stack* rather than
  the innermost heading gives subsection inheritance by construction: a term
  scoped to `"Installation"` applies inside `### Windows` under
  `## Installation`, because `Installation` is on the stack. Applicability is
  defined per **section**, not per unit — the opening heading unit itself is
  translated under its section's cohort even though its own wire
  `section_path` excludes it (P1 states why).
- **G4 — Claimed-once, relaxed exactly as far as scopes allow.** The rule
  stays "one claim per place the claims can meet":
  - global vs global: today's rule — first wins, later dropped with the
    repeat-vs-conflict warning.
  - section vs section, same case-folded source term: legal only while their
    normalized selector sets are **disjoint**; a later entry sharing a
    selector with an earlier same-term entry is dropped with a warning
    (selector overlap is document-independent, so the loader can and does
    settle it).
  - global vs section, same term: **both load** — this is the override
    pattern the scope exists for (a global default rendering plus a
    section-specific one).
- **G5 — Cohort resolution (render-time precedence).** The **effective
  glossary of a section** = every global entry + every section-scoped entry
  that applies (G3), resolved per case-folded source term: an applicable
  **section-scoped entry beats the global one** (no warning — that is the
  designed override); among several applicable section-scoped entries (nested
  headings can make disjoint selectors both match one stack), the first in
  profile order wins and the shadowing is warned. Auto-merged extracted
  entries sit after static entries in profile order, as today.
- **G6 — Auto-glossary merge refinement.** `merge_auto_glossary` keeps
  static-wins, sharpened by scope: an extracted (always-global, ADR-0014)
  term is dropped only when a static **global** entry claims it; a static
  **section-scoped** claim coexists with the extracted global — G5 then gives
  the static entry its sections and the extracted one everywhere else, which
  preserves both intents instead of letting one section's term erase document
  terminology.
- **G7 — The no-match advisory.** A section-scoped entry that applies to no
  section of the document being translated is named once per run on the
  `transync::profile` tracing target, at the `unit::build_batches` door (the
  door that computes cohorts). Advisory, not a gate: the profile is
  document-independent and the entry may be meant for a sibling document.
- **G8 — What the model sees.** A rendered glossary bullet is unchanged
  (`- "source" → "target"` with the optional note): scope and selectors are
  **never** rendered — filtering replaces annotation, which is exactly the
  reversal ADR-0014's amendment demanded (the advisory-suffix option is dead,
  not resurrected).

### 5. Cohort rendering — per-section compiled prompts

`render_prompt_body` stops being called once per run with the full glossary:

- A **cohort** is a distinct effective glossary (G5) among the run's sections.
  For each distinct cohort, the prompt is compiled **once** — from the rewound
  template each time, so the compile-idempotence and rewind rules (R0001-0014,
  ti 28110f) hold per cohort — and shared by every section with that cohort.
  Memoization is behavioral, not just performance: G5's shadowing warning is
  emitted at cohort render, so render-once-per-cohort is what keeps the
  exactly-once warning rule (R0001-0032) holding by construction.
- `TranslationBatch` keeps its exact shape; what changes is that
  `batch.profile` and `batch.glossary` now carry **the batch's section's**
  compiled profile and effective entry list. Tier-(b) providers need no code
  change: they read `batch.profile.prompt_body` today and keep doing so.
- A run with **no** section-scoped entries has exactly one cohort whose
  effective glossary is the full list — the compiled prompt is byte-identical
  to today's, every batch carries identical profiles, and (with §6) every
  cache key is unchanged. The new machinery is invisible until a profile uses
  the scope.
- The run-level warnings stay run-level and unduplicated: `normalize_glossary`
  and the control-character advisory run once over the **full** entry list at
  their existing doors; the stacked-prompt check runs once over the incoming
  body (a stacked template is stacked in every cohort — checking per cohort
  would repeat the diagnosis).

### 6. Cache identity — the two profile-derived hashes become batch-scoped

§5a's rule is "if it changed the prompt bytes the model saw for this unit, it
is identity." Per-cohort prompts mean the prompt bytes now vary by section, so
the two axes that cover them move from run scope to batch scope — by the same
mechanism `instruction_hash` already uses (ti c02f69: batch-scoped, computed
once per batch, handed into `key_for`):

- `CacheKeyContext` drops its run-level `profile_prompt_hash` and
  `glossary_hash` fields; a per-batch **cohort digest** carries them instead,
  derived **from what the batch actually carries** — `profile_prompt_hash`
  over `batch.profile.prompt_body` bytes, `glossary_hash` over
  `batch.glossary` under the existing injective length-prefixed encoding
  (source, target, note presence — unchanged; selectors are **not** hashed,
  because they never reach prompt bytes, and §5a's rule excludes them
  directly). Deriving from the batch rather than re-running the filter is what
  makes filter-logic drift structurally impossible: there is no second
  derivation to disagree with.
- `CacheKey`'s struct does not change, and `VALIDATION_SCHEMA_VERSION` does
  not bump: no existing payload's semantics change. Migration is zero by
  construction — a run without section-scoped entries produces byte-identical
  prompts and therefore identical keys (§5), and a run *with* them could not
  previously exist (the profile was rejected at load).
- Consequence, stated: two sections with identical effective glossaries share
  prompt bytes, so their units can share cache entries — correct under the
  rule. Two units differing only in *section membership under different
  cohorts* stop sharing — also correct: the model reads different glossaries.
  `context_hash` already separated most such pairs (it covers `section_path`);
  the cohort digest closes the case `context_hash` cannot see — the opening
  heading unit, whose own wire `section_path` excludes the heading that
  selects its cohort (P1/G3).

### 7. Retiring the rejection end to end

The re-admission ADR-0014 promised, done as removal rather than as a disabled
gate (the DCR-0026 `max_split_retries` discipline — dead surface must not stay
parsed-and-ignored):

- `profile::reject_reserved_glossary_scope` is deleted.
- `ProfileMetadata::ensure_supported` is deleted — it exists only to raise this
  rejection; a trivially-`Ok` method would be a dishonest surface.
- `ProfileError::Unsupported` is deleted — nothing else raises it (verified:
  the only constructor is `reject_reserved_glossary_scope`). `TranslatorError::
  Unsupported` and `AutoGlossaryStatus::Unsupported` are different types and
  are untouched.
- The translate-boundary gate call in `run_pipeline` goes with it, and
  `unit::build_batches` — fallible **only** for this gate (ticket `0ed6eb`) —
  becomes infallible: `Vec<TranslationBatch>` instead of
  `Result<_, ProfileError>`. `#[doc(hidden)]`, not curated, breaking window
  open.
- All three gates retire **in the same slice** (SL-108), which is the slice
  that makes filtering effective — the backlog entry's own condition ("all of
  which the unblocking slice must retire together").
- The rejection's regression tests (including the corrected-hint pin from
  2026-08-04) are replaced by acceptance tests: the same profiles now load,
  and the entries take effect only in their sections.
- `contracts.md` §2's scope bullet is rewritten from "rejected at three gates"
  to the G1–G8 semantics; §0 and `public_surface.rs` are checked in the same
  commit (no row is expected to move, but the weld is verified, never
  assumed).

## Open question — OQ-A: may whole sections ever share a batch?

Two defensible shapes with materially different consequences. **This DCR
implements strict confinement (P2) and deliberately does not settle the
relaxation**; the owner decides whether a follow-up is wanted. Nothing in
SL-106..SL-109 depends on the answer — coalescing would be a pure packing
relaxation on top of P1–P5.

- **(a) Strict (implemented).** Every batch ⊆ one section, always. This is the
  literal reading of the commissioned acceptance ("batches never straddle a
  section boundary unless a single section exceeds the budget" — and the
  sanctioned exception splits a section *across* batches, it never merges two
  sections *into* one). Cost: a document of many small sections dispatches one
  batch per section — more requests, more latency, more per-request envelope
  cost — where today's packer would have coalesced them. The per-request
  system prompt itself was always paid per batch; what grows is the batch
  count.
- **(b) Cohort-restricted coalescing.** Adjacent **whole** sections may share
  a batch when their effective glossaries (G5) are identical — which preserves
  glossary exactness (co-batched sections read the same prompt they would have
  read alone) and keeps every section's units together, but such a batch does
  contain a section boundary, so it fails the acceptance sentence read
  literally. For a glossary-free document this restores today's batch counts
  almost exactly (all sections share the one cohort).
- **Recommendation: ship (a), ask the owner about (b) as an opt-in knob**
  (e.g. `[batching].coalesce_sections`) once real request-count pain is
  observed. (a) is what the acceptance says; (b) is cheap to add later and
  impossible to remove quietly once defaults depend on it.

## Rules the implementer must not violate

1. **A batch never straddles a section boundary** (P2), and a section's units
   are separated only by the P4 fallback. No unit is ever reordered —
   document order is preserved within and across sections.
2. **The partition is deterministic and happens exactly once, pre-dispatch**,
   after `split_oversize_tables` and before any packing. No re-partitioning on
   any signal; retries re-dispatch verbatim within their batch (ADR-0009).
3. **A section-scoped term must never reach a prompt outside its sections**
   (ADR-0014's whole point), and scope/selectors are never rendered as prompt
   text — filtering, not annotation (G8).
4. **One applicability predicate.** G3's matching lives in one place
   (`profile`), used by cohort computation alone; cache identity derives from
   the batch's carried bytes (§6), never from a second run of the filter.
   A private re-derivation is the bug class DCR-0026 rule 4 names.
5. **Exactly-once warnings** (R0001-0032): normalization warnings at the
   existing doors over the full list; G5 shadowing warnings only at the
   memoized cohort render; G7 once per run.
6. **Cohort renders start from the rewound template** every time
   (R0001-0014 / ti 28110f); a compiled body is never re-compiled on top of
   itself.
7. **`transync-syntax` is untouched.** Everything here is core-side;
   `parser::sections` already provides the stack. No `[features]`, no
   `transync-core` edge, and the wasm gate
   (`cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`)
   stays green on every slice.
8. **§0 weld:** no facade row is expected to move; if a slice does move one,
   `contracts.md` §0 and `public_surface.rs` move in the same commit. The weld
   is never deleted.
9. **ADR-0017 and DCR-0026 are not relitigated:** the oversize-unit abort, the
   splitter's trigger, and the instruction envelope's document-level facts all
   keep their shapes; §2 and §3 above are the complete interaction.
10. **Living docs move with the slice that ships the behavior**; this DCR and
    the ADR-0014 note are the only records that describe the future. No
    version bumps or tags; CHANGELOG entries go under `[Unreleased]`. Every
    `cargo test` invocation carries `-- --test-threads=4`.

## Out of scope

- **Whole-section coalescing** (OQ-A) — recorded above, not implemented.
- Any new CLI flag or `[batching]` knob: section coherence is the behavior,
  not an option. (A coalescing knob is OQ-A's follow-up if the owner wants it.)
- Level-aware or path-expression selectors (`"Install > Windows"`, globs,
  regex): G3 is deliberately text-at-any-depth; richer selectors are a
  compatible extension if evidence demands them.
- Per-section batching *of table row windows* beyond what §2 gives for free,
  and every DCR-0026 out-of-scope item (they stay out).
- Glossary editor UX (owner-deferred on the ticket) and per-kind expansion
  factors (Type 3, evidence-gated).
- Section-scoped **auto**-glossary extraction: extracted entries stay global
  (ADR-0014); G6 only refines the merge's conflict rule.
- Report/alignment schema changes: neither artifact grows section fields;
  `VALIDATION_REPORT_SCHEMA_VERSION` and the alignment schema do not move.

## Implementation slices

Each slice is independently landable and testable, lands with its own tests
plus the living-doc edits for what it ships, and runs the full standing gates
(fmt, clippy, wasm gate, rustdoc, workspace tests with `-- --test-threads=4`).

- **SL-106 — glossary model and normalization.** `GlossaryEntry.sections` +
  the `[[glossary]]` allowlist key; G1/G2 normalization rules and warnings in
  `normalize_glossary` (inheriting the three-door structure); G4's relaxed
  claimed-once; the G3 applicability predicate + effective-glossary resolver
  (G5) as pure, heavily-tested `profile` functions. The three gates **stay
  up** — `scope = "section"` still rejects, so shipped behavior is unchanged;
  the machinery lands testable underneath. Contracts §2 documents the new key
  as reserved-but-parsed.
- **SL-107 — section-coherent packing.** The P1 partition in
  `unit::build_batches` (after `split_oversize_tables`, before instruction
  assembly); per-section `BatchBudget`s and the per-section envelope reserve
  (§3); P3/P4 packing per section. Behavior change: batches stop straddling
  sections for every run. Tests: P1 edge cases (preamble, heading-belongs-to-
  its-section, h1→h3 jumps, list-item/blockquote units staying put), the P3
  one-batch-per-fitting-section property, the P4 fallback, window units
  landing in the parent table's section, determinism, and the extended
  containment test. Living docs: `batch.rs` module doc, contracts §5,
  `mvp-scope.md`, `architecture/README.md`, SCN-10 row.
- **SL-108 — cohorts, cache identity, and gate retirement.** Per-cohort
  memoized rendering (§5) with batches carrying their section's profile and
  effective glossary; G5 precedence + shadowing warnings; G6 merge
  refinement; G7 advisory; the batch-scoped cohort digest replacing the two
  run-level hashes in `CacheKeyContext` (§6); retire the rejection end to end
  (§7: helper, `ensure_supported`, `ProfileError::Unsupported`, boundary
  call, `build_batches` infallibility) with the §0 weld checked in the same
  commit. End-to-end test: a profile with a section-scoped entry loads,
  translates, and a recording translator proves the entry's bullet appears in
  exactly its sections' prompts and no others; cache-identity tests pin the
  single-cohort run byte-identical to today and the heading-unit cohort case
  (§6). Living docs: contracts §2/§5a, `Developer_Guide.md`.
- **SL-109 — scenario and record closure.** The scenario-level test (a
  sectioned document, a section-scoped + global override pair, batches never
  straddle, term applied only in scope); `scenario-matrix.md` SCN-09/SCN-10
  rows updated; the dated appended note on `design-baseline-2026-07.md` (the
  EXT-2026-07 wave record); `docs/backlog.md` retires the
  `glossary-section-scope` Type-3 entry; CHANGELOG under `[Unreleased]`.

## Amendment (2026-08-09) — the shadowing warning dedups on a pair, not on the cohort memo

*Appended, not a rewrite. The design stands. One mechanism it named could not
deliver the property it was named for, so SL-108 (`8afb830`) shipped a
different one; this note is the record of the deviation, which until now lived
only in the slice's off-repo implementation report.*

- **What §5 and rule 5 said.** §5: "G5's shadowing warning is emitted at cohort
  render, so render-once-per-cohort is what keeps the exactly-once warning rule
  (R0001-0032) holding by construction." Rule 5: "G5 shadowing warnings only at
  the memoized cohort render." Both are **superseded** by this note.
- **Why the memo cannot carry it.** A cohort is a *set of surviving entries*,
  and two sections can arrive at the same set by different routes — one where a
  section-scoped entry was shadowed, one where it never applied at all. Take
  `glossary[0]` scoped to `Guide` and `glossary[1]` scoped to `Tables`, both
  claiming `cell`, over a document whose sections are `# Guide` then
  `## Tables`. `## Tables` resolves to `[0]` *with* a shadowing; `# Guide`
  resolves to `[0]` with none — and `# Guide` packs first, so the memo is
  already warm when the shadowing occurs and the render that would have spoken
  never runs. Under the §5 mechanism the warning is not deduped, it is **lost**:
  the rule fails in the direction that hides a real finding.
- **What ships instead.** `profile::effective_glossary` **returns**
  `ShadowedEntry { index, winner, message }` values rather than emitting them,
  and `unit::build_batches` — which already walks the sections in document
  order — dedups on the `(loser, winner)` pair through a `said_shadowings` set
  before emitting. Exactly-once now holds on the pair the finding is *about*,
  rather than on the cohort, a coarser key that does not contain it. The memo
  keeps its other job unchanged (compile each distinct prompt once), and every
  remaining clause of §5 stands: cohort keying, the seeded full-glossary
  cohort, the rewind rule, and the run-level warnings that stay run-level.
- **Rule 5 as shipped.** Exactly-once warnings (R0001-0032): normalization
  warnings at the existing doors over the full list; **G5 shadowing warnings
  once per `(shadowed, winner)` pair, deduped by the caller that walks the
  sections — never by the cohort memo**; G7 once per run.
- **Test.** `unit`'s
  `a_shadowed_entry_is_named_once_however_many_sections_share_the_cohort` uses
  exactly the route-difference fixture above, so the superseded mechanism
  cannot be reintroduced without a red test.

## OQ-A settled — (a) stands, (b) deferred to 2027, 2026-08-09 (appended note)

**Owner decision.** Strict confinement — every batch inside one section — is
the end state for now, and cohort-restricted coalescing is **deferred until
2027** rather than declined. The recommendation this record made ("ship (a),
ask about (b) once real request-count pain is observed") is accepted with a
date attached instead of an open-ended "later".

Nothing changes in the code: (a) is what SL-106..SL-109 shipped. What changes
is the status of (b) — it is now **tracked work with a revisit date**, carried
in `docs/backlog.md` under Type 3 as `section-batch-coalescing`, not an open
question living only in this record.

Why a date rather than a condition, when this repo usually prefers conditions:
the natural condition ("real request-count pain is observed") needs a consumer
running heading-rich documents at volume, and no such measurement exists yet.
A date guarantees the question is re-asked; the condition, if it fires first,
is the reason to re-ask early. Both are recorded on the backlog entry.

No TicGit ticket was filed, deliberately. TicGit is the queue for work someone
means to start; a 2027 deferral is the opposite, and filing it would hand
`ti-pick-next` a task nobody chose. The backlog entry is the tracker, and
`reopen` will offer it at its selection gate when the date comes near.

## Amendment (2026-08-12) — G2's identity gains canonical normalization

*Appended, not a rewrite. Review 0004 finding `R0004-0080`, ticket `ad8b54e4`.*

- **What G2 said.** "Selectors are compared **trimmed and case-folded** — the
  same identity `glossary_key` already uses for source terms." That named the
  right property and one step short of it: `to_lowercase` is Unicode-aware but
  purely per-scalar, so it folds case without settling *composition*. A heading
  whose accented letter is written as base + combining mark and a selector whose
  is written precomposed are the same string under Unicode canonical
  equivalence, and they folded to two different keys — the entry applied
  nowhere, silently, and a term in the same shape claimed nothing.
- **What it says now.** The identity is trimmed, case-folded and normalized to
  **NFC**, in that order, in one private `profile::canonical_key` that both
  `section_key` and `glossary_key` call. NFC runs last because case mapping
  preserves canonical equivalence but not the composed form, so composing first
  would leave the fold's output unnormalized again; composing last makes the
  function idempotent on its own output, which is what lets both sides of every
  comparison run through it.
- **Nothing else in this record moves.** G3's stack matching, G4's claimed-once
  rule, G5's resolution and §5's cohorts are all expressed *in terms of* this
  identity, so they inherit the fix without restatement — and a profile whose
  text is already NFC (every ASCII profile, and every editor-typed one) folds to
  exactly the keys it folded to before.
- **§6 consequence.** For a profile that this actually moves, the run's cohorts
  can change, which changes the compiled prompt and therefore the two
  profile-derived cache hashes: those entries re-key once. The CHANGELOG entry
  for 2026-08-12 states it; the disk log grammar does not move, so an existing
  log replays cleanly and simply misses.
