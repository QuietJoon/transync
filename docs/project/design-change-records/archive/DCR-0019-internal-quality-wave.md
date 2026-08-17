---
type: DCR
title: OI-0008 + OI-0033 internal-quality wave — CR-aware line table, pure retry/fallback policy module, six concern splits
description: Eleven commits resolving the last two internal-quality issues. Two deliberate behavior changes — LineOffsets now counts a lone CR as a line boundary (matching comrak), and ValidationReport.total_retries counts content retries only — plus behavior-preserving motion: a pure pipeline::policy decision core (A1), run_pipeline split into dispatch/finalize/report, unit.rs split into a structure oracle plus payload/budget strategies, the OpenAI client split into transport/classify/endpoint/dispatch/chat/responses/tokenizer behind one surface abstraction, translate_cmd split behind an execute() seam that collapses fifteen exit-code sites into one table, and the parser walker restructured onto a WalkState after Document::hierarchy and Block::parent_id were deleted. Resolves OI-0008 and OI-0033; files OI-0034.
tags: [change, project-control, DCR-0019]
status: deprecated
---

# DCR-0019: OI-0008 + OI-0033 internal-quality wave

- **Date:** 2026-08-05
- **Source:** the OI-0008 + OI-0033 internal-quality wave, implementing the
  owner-approved design spec
  `docs/superpowers/specs/2026-08-05-oi0008-0033-internal-quality-design.md`
  (approach **A1**; sections S1–S8 approved 2026-08-05) against the 12-task
  plan `docs/superpowers/plans/2026-08-05-oi0008-0033-internal-quality.md`.
  **Eleven commits on `master`, `b1c3c7f`..`73fd331`.** No new ADR: the
  decisions this executes are the owner's three 2026-08-05 calls — OI-0033
  posture = **Support**, OI-0008 scope = **full** (including the parser
  `visit` restructure, accepting the double-work risk against Track C's
  IR-boundary design), retry/fallback architecture = **A1, pure policy
  module**.
- **Resolves:** **OI-0008** (refactor / perf debt cluster — every remaining
  bullet group consumed) and **OI-0033** (lone-CR `LineOffsets` desync).
- **Files:** **OI-0034** (comrak's NUL→U+FFFD substitution desyncs byte
  *columns*) — found by the same probe, deliberately not fixed here.
- **Affected ADRs:** `docs/decisions/0004-comrak-as-gfm-parser.md`
  (**upheld** — the fix makes *our* line table agree with comrak's own
  `strings::is_line_end_char`; one parser, now with one line-ending rule);
  `docs/decisions/0009-reject-retry-policy-changes.md` (**upheld, and both
  amendments with it** — the bounded per-unit budget, the verbatim
  resubmission rule, the 2026-07 `RetryContext` side channel, and the
  2026-08-03 second per-batch schema budget all keep their meaning and their
  defaults. `pipeline/retry.rs` still owns `retry_validation_unit` and
  `MAX_RETRY_REASON_BYTES`; the extraction moved the *decision* into
  `pipeline/policy.rs`, not the side channel, and the policy module's tests
  pin the ADR's arithmetic — `1 + max_per_batch_schema_retries + units ×
  max_per_unit_validation_retries` — as an explicit termination bound);
  `docs/decisions/0016-whole-document-in-memory.md` (**upheld**, dated path
  note — its `resolve_profile` pointer now names `translate_cmd/input.rs`);
  `docs/decisions/0008-reject-optional-warnings-in-strict-schema.md`
  (**upheld**, dated path note — the strict-schema/lenient-parser asymmetry it
  defends is unchanged, but its `client.rs` pointer no longer holds in either
  half: `schema_object_for` had already left the provider crate in DCR-0015's
  prompt lift, and this wave's client split moved the provider-side envelope
  readers to `client/chat.rs` and `client/responses.rs`);
  `docs/decisions/0005-block-id-format.md` (**upheld** — its "recovered via
  `parent_id` and `section_path` in the alignment map" reasoning is about the
  *wire* fields, which are unchanged). **DCR-0017** gained a dated note in
  commit `09040f9`: its Guard-1 residual (a) is now void **unqualified**.

## What Changed

The wave's behavior-change budget was **exactly two** items, both listed in
spec §0. Everything else is behavior-preserving motion, and each task carried
its own proof of that (line-multiset diffs, byte-identical output trees,
identical test-name multisets).

### Part A — OI-0033: `LineOffsets` agrees with comrak (behavior change 1)

Commits `b1c3c7f` (the fix) and `09040f9` (the integration pins).

- **One arm added to `LineOffsets::new`:** push a boundary for a `b'\r'`
  whose next byte is not `b'\n'`. Comrak's rule and nothing more. The doc
  comment names the authority — comrak's `strings::is_line_end_char` matches
  `10 | 13` and consumes a terminator as optional `\r` followed by optional
  `\n`, so `\r\n`, lone `\r`, and lone `\n` are each exactly one terminator.
- **Probe evidence (comrak 0.27), recorded because it is the reason the arm
  is that narrow.** A line-boundary sweep over `alpha<CH>bravo` showed
  `max sourcepos end.line = 2` for LF, CR, and CRLF, and `= 1` for **VT
  (U+000B), FF (U+000C), NEL (U+0085), LS (U+2028), PS (U+2029), and NUL**.
  None of those is a line ending, so neither `str::lines()` (splits on `\n`
  only) nor any Unicode line-break API is admissible — both would mis-count
  against comrak. On the CR-only probe document comrak reported a maximum
  `end.line` of **6** against an offsets table of length **1**; that gap is
  the whole defect.
- **Byte-identical on the entire current corpus.** The new arm fires only
  where a lone `\r` exists, so the offsets vector for every LF and CRLF
  document is unchanged — behavior-preserving by construction, which is why
  the fix could land without a schema or fixture migration.
- **The duplicated line-end cap became one terminator-aware helper.**
  `pos_to_byte` and `byte_range_for` each carried their own derivation of
  "where does this line's content stop"; both now call one private helper
  that returns the offset of the *first* byte of the line's terminator, so a
  CRLF line's cap excludes the `\r` too. Not reachable today (the probe found
  zero over-column hits on block nodes — the only over-column cases are
  inline `SoftBreak`/`LineBreak`, and `byte_range_for` is called exclusively
  on block nodes), but the duplication was a drift hazard and the old
  `next_start - 1` cap was loose.
  - **Plan deviation, deliberate:** the plan sketched the helper as a
    function taking the source text. No call site holds the source, so that
    signature was unworkable; the implementation stores a parallel
    `line_content_ends` vector built in the same pass. The cost is one extra
    `usize` per line, noted as a revisit-only-if-wasm-footprint-matters item
    rather than a defect.
- **Tests are inline `&str` constants, never a checked-in fixture.** No
  `.gitattributes` protects CR bytes in this repo, so an editor or an
  `autocrlf` setting could silently rewrite a `.md` fixture into a test that
  passes vacuously; a fixture file would also create a `.ko.md` mirror
  obligation. Six tests landed in `ranges.rs`, which previously had none.
  - **One of the plan's own tests did not discriminate, and was
    strengthened.** `lone_cr_at_eof_is_a_boundary` **passed pre-fix**: for
    `"just one line\r"`, line 2 is out of range in the old table, and the
    out-of-range branch resolves to the source length — which happens to be
    the right answer. The bug hid behind its own failure mode. Both original
    assertions were kept and one that actually discriminates was added (the
    boundary must exist in the offsets table, `[0, 14]` where the old code
    produced `[0]`), so the test now expresses the same intent *and* fails
    without the fix.
- **Parse-level pin.** `lone_cr_document_slices_every_block_to_its_own_bytes`
  asserts kinds, exact ranges, exact slices, and the structural property
  (non-empty, non-overlapping, increasing) as a loop — the last of which
  fails wholesale pre-fix, because every block after the first collapsed onto
  the same clamped offset. It also asserts `doc.warnings.is_empty()`: pre-fix
  this document emitted the ref-defs "unattributed source text between
  blocks" note, so the test pins the observable *symptom*, not only the
  arithmetic.
  - **The probe beat the plan's prose on one value.** The plan listed the
    third list item's slice as `"- charlie"`; comrak's sourcepos for that
    item ends at the *next* line's column 0, so the terminator-inclusive
    slice `"- charlie\r"` is correct. Re-derived independently against the
    committed terminator-aware helper, and confirmed not to be a CR artifact
    — the LF spelling of the same document produces the identical shape.
- **Defensive guard for the open class.** A translatable **list-item** block
  whose computed range is empty at a *non-degenerate* source position now
  emits a document warning naming the block, instead of silently disarming
  `per_kind::check_list`. Scoped strictly to list items: SCN-15's comment
  block legitimately slices to an empty range and must not trip it.
- **Guard-1 residual (a) closed.**
  `lone_cr_list_items_carry_list_topology_constraints` parses a lone-CR list
  document and requires every list-item unit to carry a non-empty payload
  *and* `Some(expected_list_topology)` — the residual's exact predicate,
  asserted directly rather than inferred. DCR-0017's residual note and the
  mirroring comment in `validate/full_reparse.rs` dropped their lone-CR
  caveat in the same commit and now read "void unqualified".

### Part B — A pure retry/fallback policy module, and `total_retries` (behavior change 2)

Commit `f82fb37`. Approach **A1**: the decision core is extracted; all I/O
stays in the orchestrator.

- **`pipeline/policy.rs` is pure, and provably so.** Its complete import list
  is `crate::TranslateOptions`, `crate::id::BlockId`,
  `crate::validate::AttemptOutcome`, `std::collections::HashMap`, and
  `std::time::Duration`. No `tokio`, no `Cache`, no `Translator`, no
  `TranslatorError`, nothing `async`. The transient-error *predicate*
  (`Network | RateLimited`) deliberately stayed at the orchestrator's `match`
  so that no provider type reaches the module — which is why
  `on_transient_error` takes an `Option<Duration>` rather than the error.
- **What it owns:** the per-unit content-fault counter, the per-batch
  schema-fault round counter, the provider-transient attempt counter, the
  per-unit dispatch ordinal, and the budget values lifted from
  `TranslateOptions`. `process_one_batch` keeps the provider calls, the cache
  get/put/evict, the sleep, and the attempt-row emission; every branch that
  used to embed policy now asks the policy object.
- **Four API deltas from the plan's sketch**, each forced by the real data
  flow and each preserving the no-I/O property:
  1. **`record_dispatch(&BlockId) -> u32`** — the plan's field list omitted
     the dispatch counter, but `unit_disposition` must return `attempt`
     (`dispatch_n + 1`) and the orchestrator independently needs `dispatch_n`
     for the attempt row. The ordinal is now bumped and returned by one
     method, and `unit_disposition` `debug_assert!`s that it is non-zero, so
     the once-per-unit-per-round ordering contract fails loudly in tests
     rather than silently emitting `attempt: 1`.
  2. **`begin_dispatch_round()`** — load-bearing, not decoration. Today
     `translate_with_provider_retries` is called once per retry round and
     each call started from a fresh local counter, i.e.
     `max_per_batch_provider_retries` is charged **per dispatch round**, not
     per batch. Making the policy's counter plainly batch-cumulative would
     have been a *silent third* behavior change, so the reset is explicit and
     documented at the method.
     **The knob's name and doc say "per-batch"; the code has always charged
     per round.** That mismatch is preserved verbatim and filed as TicGit
     ticket **`294ddabd`** with both resolution options and the missing
     end-to-end transient test written up — the wave's behavior-change budget
     was already spent.
  3. **`batch_fault_rounds()`** — a read-only accessor, the minimum surface
     for the two things the orchestrator still owns: the schema-fault warn
     log's `(n/m batch-fault round(s) used)` field, and
     `BatchResult.batch_schema_fault_rounds`.
  4. **`content_retry_count(&[AttemptOutcome]) -> u32` lives in
     `policy.rs`**, not inside `build_validation_report`. It is retry
     semantics, and homing it in the pure module bought it an eight-case
     truth-table test with no mock pipeline. Since Part C it is a
     `report.rs → policy.rs` call; both are `pub(crate)`.
- **`ValidationReport.total_retries` now counts content retries only.**
  The old computation was `attempts.len() - 1`, which also counted
  batch-schema-fault rounds and rejected-cache-hit re-validations —
  contradicting the claim, written in **both** `pipeline.rs` and
  `validate.rs`, that `batch_schema_faults` is "counted apart from
  `total_retries`". The docs were right; the code changed.
  - **TDD evidence:** the three report-level tests were written against the
    old code first. Exactly the two discriminating cases failed at the old
    value `1` (`a_batch_fault_round_is_not_a_content_retry`,
    `a_rejected_cache_hit_is_not_a_content_retry`), while the positive case
    already passed — which is the point: the fix must not move it.
  - **No existing test pinned the old conflated value.** The only two
    assertions on the field (`scn_07_validation_retry`,
    `inline_protection`) live in scenarios with no batch fault and no cache
    hit, where the old and new semantics coincide.
  - **Sanctioned observable delta:** a `validation-report.json` from a run
    that took a batch-schema fault or a rejected cache hit now reports a
    **lower** `total_retries` than it did at v0.2.0. `batch_schema_faults`
    and `provider_retries` are unchanged, and the three counters now sum
    without double-counting.
- **A coverage gap closed on the way.** `translate_with_provider_retries` had
  **no** test at all: the transient predicate, the 30 s `Retry-After` cap,
  the `200 ms << min(attempts-1, 5)` schedule capped at 5 s, and the counter
  were all unverified. The schedule and the budget arithmetic are now pinned
  purely; the predicate itself remains at the orchestrator's `match` and is
  still unpinned end-to-end (also covered by ticket `294ddabd`).
- **Twelve policy tests + three report-semantics tests**, all runnable
  without a mock translator — which is the whole argument for A1.

### Part C — The six split maps

Every split is `pub(crate)` (or module-private) and file-as-module; no
`mod.rs` anywhere. Inline `#[cfg(test)]` tests moved **with** the code they
pin.

**1. `pipeline.rs` → four phases** (commits `f82fb37`, `70730dc`)

| before | after |
|---|---|
| `pipeline.rs` (3,717 lines), `pipeline/retry.rs` | `pipeline.rs` (1,439) — `run_pipeline`, the glossary preflight (`resolve_auto_glossary` + helpers), `annotate_with_preflight`, the cache helpers, `CacheKeyContext` |
| | `pipeline/dispatch.rs` (1,592) — `BatchResult`, `process_one_batch`, `translate_with_provider_retries` |
| | `pipeline/finalize.rs` (474) — `finalize_regen_with_reparse_policy`, `regen_pass`, `widen_to_neighbors`, `downgrade_units`, `fall_back_all`, `collect_validated` |
| | `pipeline/report.rs` (548) — `Aggregated`, `aggregate_batch_results`, `build_validation_report` |
| | `pipeline/policy.rs` (506) — Part B |
| | `pipeline/retry.rs` — role unchanged (ADR-0009 side channel) |

- **Pure-motion proof, not test colour.** A normalized line-multiset diff of
  the pre-split file against the four post-split files (dropping blanks,
  indentation, `use` lines, `mod` declarations, module docs, and the
  `pub(crate) ` prefix) leaves **zero production-code lines** on either side:
  only `use`-block continuation lines, three new `#[cfg(test)]` wrappers, and
  module-header prose. Independently, `cargo test --workspace -- --list`
  reported the same count before and after, with the leaf-test-name multiset
  diffing to nothing — only module paths moved.
- One pre-existing test module was genuinely mixed (reparse-policy tests plus
  dispatch-, report-, and run-level pins) and split four ways, each half
  landing next to its subject.
- **Deliberately NOT extracted: alignment-map assembly.** It is still inline
  in `run_pipeline` and has no existing function signature, so lifting it
  would have been *design*, not motion — outside a wave whose review question
  is "is this pure motion?". Recorded as an open seam in *Follow-up*.

**2. `unit.rs` → the structural-fingerprint oracle leaves** (commit `f10b070`)

| before | after |
|---|---|
| `unit.rs` — `ListFacts`, `inspect_list_topology`, `inspect_blockquote_children`, `inspect_table`, `block_node_kind_label`, `item_child_kinds`, `blockquote_child_label` (38% of the file) | `structure.rs` (195) — `ListFacts` + the three `inspect_*` oracles |
| | `structure/labels.rs` (91) — the three label helpers |

- **The point is the inversion it removes:** `validate/per_kind.rs` used to
  import its structural oracle from the *unit builder*. Both of its import
  sites (module-level and test-level) now name `crate::structure`, so the
  validator no longer depends on the thing it validates.
- Move verified mechanically: the non-blank line multiset of the vacated
  region equals the two new files' bodies once module docs and three
  scaffolding lines are excluded. The only substantive change to a moved line
  is `fn` → `pub(crate) fn` on the three label helpers, now that they sit one
  module deeper.
- `ListFacts` stayed private — minimal visibility upheld.

**3. `unit.rs` → payload strategy + budget resolution** (commit `8b339f3`)

| before | after |
|---|---|
| `unit.rs` (666 lines at v0.2.0), `unit/context.rs` | `unit.rs` (272) — `build_batches` as thin orchestration: build units → early return → resolve profile → resolve budget → pack → stamp batch ids |
| | `unit/payload.rs` (194) — `assemble(doc, block) -> (payload, InputMode, BlockConstraints)`, absorbing `input_mode_for`, `constraints_for`, and the previously-inlined html arm |
| | `unit/budget.rs` (242) — `resolve(...) -> BatchBudget`, plus ONE `caller_wins` sentinel helper |
| | `unit/context.rs` — unchanged role |

- **The sentinel duplication is genuinely gone.** The "caller value differing
  from the built-in default wins" rule existed as three near-identical
  copies; two were true sentinel copies and both now call one helper. The
  third — the output ceiling — has no caller-side counterpart in
  `TranslateOptions`, so it stays a straight profile read with a comment
  saying why no sentinel applies. Equivalence of the collapse is exact: the
  old copies ended `.unwrap_or(opts.<field>)` and the helper ends
  `.unwrap_or(default_value)`, and that branch is reachable only when the two
  are equal.
- **Path preservation held:** `unit::build_batches` and `unit::html_outcomes`
  keep their exact paths — `transync-openai`'s `live_smoke` fixture-shape
  test reaches them by dev-dependency, and that reach is the sole reason
  `unit` is `pub` at all (DCR-0018).
- **An undocumented coupling became documented.** The empty-document early
  return stays *ahead* of profile and budget resolution, now under an
  `ORDERING IS DELIBERATE` comment: resolving the profile first would still
  return no batches, but it would surface a malformed-profile failure on a
  document that never needed a profile.

**4. `transync-openai`'s client → transport / classification / surfaces** (commits `3587d86`, `5686387`)

| before | after |
|---|---|
| `client.rs` (1,272 lines), `pagination.rs` | `client.rs` (491) — flow only: `SurfaceRequest`, `round_trip`, `translate_on`, `call_glossary_extraction`, the path/limit consts |
| | `client/classify.rs` (340) — `provider_error_for_status`, `map_reqwest_error`, `oversize_response_error`, `truncate_diagnostic`, `parse_retry_after` |
| | `client/transport.rs` (95) — `post_json`, reduced to send + size cap + accumulate |
| | `client/endpoint.rs` (75) — `build_endpoint` |
| | `client/dispatch.rs` (100) — `Api`, `api_for_model`, `api_from_env_or_model` |
| | `client/chat.rs` (338) — Chat DTOs, both body builders, output extraction, schema descriptors |
| | `client/responses.rs` (346) — the same for Responses |
| | `tokenizer.rs` (35) — `tokenizer_hint_for_model`, out of the HTTP client entirely |
| | `pagination.rs` — **deleted** (Part E) |

- **Tests first, and genuinely RED.** The HTTP-status → `ProviderError`
  classification table was the client's only unpinned piece *and* the input
  the core pipeline's retry policy consumes. It was reachable only over a
  socket, so the tests were written into a `classify.rs` that contained
  nothing but `#[cfg(test)] mod tests`: **29 compile errors, every one an
  unresolved name**. No assertion could have been satisfied by the
  pre-existing code, because there was no callable classifier. Only then was
  the table moved — character-for-character.
- **Ten new classification pins**, including three the table had quietly
  earned: 401/403 carry a **bare** body excerpt with no `HTTP <status>:`
  prefix (a real asymmetry, now pinned); the classification is also pinned
  *through* `map_provider_error`, i.e. as the pipeline actually sees it; and
  `truncate_diagnostic` is pinned to walk back to a char boundary when a cut
  lands inside a multi-byte character — a latent panic-shaped edge with no
  prior coverage.
- **One surface abstraction, and no trait.** Translation used two whole call
  functions while glossary extraction re-inlined the entire per-surface flow
  inside one `match`, duplicating endpoint construction, transport, and error
  mapping. Both converge on
  `SurfaceRequest::{Chat, Responses}` + `round_trip(http, key, base, request)`
  = build endpoint → `post_json` → `request.extract_output(bytes)`, with two
  thin flows riding it. An enum plus free functions was sufficient: there is
  no third implementor and no generic call site, and the enum keeps the wire
  types concrete. `Api`, `api_for_model`, and `api_from_env_or_model` are
  re-exported from `client.rs`, so every existing
  `transync_openai::client::…` path resolves unchanged.

**5. `transync-cli`'s translate command → six files behind one seam** (commit `8b1a434`)

| before | after |
|---|---|
| `translate_cmd.rs` (1,025 lines; a 311-line `run`) | `translate_cmd.rs` (385) — `TranslateArgs` (clap, verbatim), `CliFailure`, `RunSummary`, a 23-line `run`, and `execute` |
| | `translate_cmd/args.rs` (273) — flag validation/resolution + `OutputTarget` |
| | `translate_cmd/input.rs` (112) — `read_capped`, profile resolution, their error types |
| | `translate_cmd/provider.rs` (78) — both cfg-gated `translator_for_run` variants + the stub echo side channel |
| | `translate_cmd/publish.rs` (168) — ONE output-set builder + commit; the OI-0032 direction resolution |
| | `translate_cmd/report.rs` (241) — the `Reporter` quiet gate, diagnostics, verbose tally, glossary summary |

- **The one structural change: the `execute` seam.**
  `async fn execute(args: &TranslateArgs, reporter: &Reporter) -> Result<RunSummary, CliFailure>`
  with `CliFailure { code: ExitCode, message: String }`; `run` is now build
  `Reporter` → `match execute()` → report-then-exit. All **fifteen** former
  `return ExitCode::… as i32` sites became `CliFailure` values constructed
  inside `execute`, and no submodule names an exit code at all — `input.rs`
  keeps its existing error split and `publish.rs` returns its own
  `PublishError`, both mapped to codes and message texts in `execute`'s `Err`
  arms.
  - **The seam takes `reporter`, not `args` alone** as the plan sketched:
    diagnostics are emitted *during* the run, not only after it, so the
    reporter has to be inside. That is a deviation from the plan's signature
    and is recorded here rather than silently absorbed.
- **The two `OutputTarget` arms collapsed.** Both were ~40-line near-copies;
  the per-mode decision is now one `Destinations` value (`markdown`, `map`,
  `report`, `bundle_dir`, `commit`), after which alignment-map
  serialization, report serialization, bundle assembly, file-list build, and
  commit each exist exactly once. ~90 duplicated lines are gone.
  - **Correction to the plan:** the plan said the `--html-out` preflight
    belongs to the Dir path. It does not — it guards the **Files** path;
    `--out-dir` guards its whole target inside its own publish routine. The
    actual behavior was preserved and the plan's claim is wrong, recorded
    here so the next reader does not "restore" it.
- **Byte-stability probe beyond the suite.** The binary was built at the
  pre-refactor commit and at the refactor, then run over seven invocations
  against `scn-14-full.md` — individual outputs, an `--out-dir` publish, a
  missing input (exit 2), a forced provider failure (exit 3), two argument
  errors (exit 1), and a `--quiet` run. **Output trees, stderr, and exit
  codes are byte-identical.** The 19 `cli_smoke.rs` subprocess tests were not
  touched.

**6. `transync-syntax`'s parser walker** (commits `094c1c4`, `73fd331`)

| before | after |
|---|---|
| `parser.rs` (641 lines): IR types, `parse`, `build_hierarchy`, comrak options, the classification tables, a nine-argument `walk_children`/`visit` pair, an `emit` closure plus the `List` arm's inline re-implementation, an `emit_here!` macro, a free `current_section_path` | `parser.rs` (490) — the IR (`Document`/`Block`/`AstPath`/`Section`), a thin `parse`, `WalkState<'s>` + `walk_children` + `visit` (the one match) |
| | `parser/options.rs` (35) — `gfm_options`, `comrak_options` |
| | `parser/classify.rs` (198) — `heading_kind`, `paragraph_kind` (+ the image-only promotion), `code_block_kind`, `list_item_kind`, `skipped_kind`, `skip_warning`, and both private label tables |
| | `parser/emit.rs` (292) — the ONE `emit` (+ `emit_here`), and the OI-0033 empty-range guard |
| | `parser/sections.rs` (49) — `SectionStack`: `current_path` / `close_through` / `open_scope` |
| | `parser/ranges.rs`, `parser/refdefs.rs` — unchanged homes |

- **`WalkState<'s>`** (`source`, `line_offsets`, `blocks`, `counter`,
  `sections`, `ast_path`, `warnings`) turns both nine-parameter free
  functions into methods. Deleted for free: **three**
  `#[allow(clippy::too_many_arguments)]` attributes, the
  `#[allow(clippy::ptr_arg)]` on `visit`, the `emit_here!` macro (it existed
  only to hide nine positional arguments), and the free
  `current_section_path` (absorbed into `SectionStack`). The only surviving
  mention of `too_many_arguments` in the crate is prose explaining why
  `WalkState` exists.
- Side-effect order is preserved arm for arm. The Heading arm still
  classifies *before* touching the section stack; the List arm is still the
  one arm that needs the general `emit`, because its items own a path the
  walker never stands on.
- **Seventeen `#[test]` functions before, seventeen after**, with the sorted
  name lists diffing to empty; bodies are byte-identical and only `use`
  headers were rewritten for the new module depth.

### Part D — The subtractive slice, and the wire invariance that let it happen

Commit `094c1c4`. This is the half of the parser work that *deletes* rather
than moves.

- **`Document::hierarchy` is gone**, together with `build_hierarchy` and the
  `assign_block_ids` re-key pass over it. The field's own doc admitted "no
  pipeline consumer yet"; its only readers were that pass and that pass's
  test. It cost a full extra walk plus a `HashMap` and two `Vec<BlockId>`
  allocations on **every parse** — precisely the code path Track C ships to
  the browser.
  - **`Section` survives with its shape frozen** (the spec's IR freeze rule),
    now with no producer, and carries a note saying why: a future OI-0005
    hierarchical-alignment revision should *project* it from the surviving
    heading stack rather than re-derive the scope rule a second time — which
    is exactly the drift `build_hierarchy` embodied.
- **`Block::parent_id` is gone**, provably always `None`: `walk_children` was
  called exactly once, from `parse`, with `None`, and `visit` never recursed.
  The `parent_block_id` parameter that threaded through both nine-argument
  signatures purely to be discarded went with it.
  - **The exploration digest listed five tautological filter sites; there are
    SIX.** `walk.rs::normalize_top_level` was not in the list. The sites:
    `parser/refdefs::gap_ranges`, `regen::top_level_blocks` (kept as a named
    concept), `render::is_top_level` (the function deleted with its one call
    site), `render/attrs::write_attrs` (the `data-parent-id` conditional
    deleted), `align.rs` (two target-range `or_else` fallbacks and two
    `parent_id:` writes), and `walk::normalize_top_level`. Missing one would
    have been a compile error rather than a silent bug, but the undercount is
    recorded so the next census starts from six.
  - **Consequential core-side sites** the deletion forces, outside the plan's
    file list: `test_fixtures.rs`, two test-side filters plus one dead
    sub-case in `pipeline/finalize.rs`, three test-side filters in
    `validate/full_reparse.rs`, and one synthetic `Block` literal in
    `unit.rs`.
- **The WIRE field stays, and the bytes did not move.**
  `AlignmentBlock.parent_id` remains at schema **1.2.0**, now written as a
  literal `None` at both construction sites and documented as RESERVED and
  always null. The in-crate and facade assertions that read it (`align.rs`,
  `tests/reader_honesty.rs`, `scn_14_full`) are untouched — they now pin the
  wire invariant rather than an IR projection, which is the better thing for
  them to pin.
  - **`render/attrs`' reads of the wire field were removed** rather than
    rewired. This is contract-aligned: `contracts.md` §4a already states that
    `data-parent-id` is RESERVED and that **no rendered block is ever emitted
    with one**. The removal is observable only for a hand-crafted,
    deserialized alignment map carrying a non-null `parent_id` driven through
    the `transync-syntax`-direct render path — a shape the pipeline cannot
    produce and the contract already forbids.
  - **Byte-stability proof.** The stub-provider CLI was run over **every**
    `crates/transync/tests/fixtures/*.md`, emitting `--output`, `--map`, and
    a full `--html-out` bundle — **112 output files**. The tree was then
    stashed back to the pre-change state, the same 112 files regenerated, and
    `diff -r` run: **identical, byte for byte**. That set covers the
    translated Markdown, the alignment JSON (with `parent_id` present and
    `null` on every row), and each fixture's `index.html` / `source.html` /
    `target.html` / `alignment.json` / `sync.js`. `data-parent-id` appears
    nowhere in the rendered HTML, before or after.
- **ONE `emit`.** The `List` arm used to re-implement the emit closure
  inline, with a subtly different `ast_path`; one free function now serves
  both, taking `node` and `ast_path` **explicitly** (the old closure
  *captured* both, which is exactly why the List arm could not reuse it).
  - **The `ast_path` contract was pinned FIRST.** Three tests capturing
    today's values were written and run green against untouched `emit` code
    before a line of the unification: list items carry the owning `List`
    node's path plus their own index, a marker-type change starts a new
    prefix, a nested sub-list is never visited, and every non-list block
    carries the walker's own path. This is the contract
    `walk::normalize_top_level` reads when it collapses a list run by
    comparing item-path prefixes, and the renderer reads when it opens one
    `<ul>`/`<ol>` per run (DCR-0007). A one-element shift either way silently
    merges or splits lists — invisible to a smoke test, loud to these three.
  - The OI-0033 guard now fires exactly once per emission, always pre-push,
    from inside `emit`; previously the two emission paths called it at two
    different points relative to the push. Warning content and vector order
    are unchanged.
- **ONE section-scope algorithm.** `build_hierarchy`'s independent pop-while
  re-derivation died with the field, leaving `visit`'s Heading arm — homed in
  `parser/sections.rs` by the next commit — as the only implementation in the
  workspace.
- **ONE `heading_level` mapping — with one honest residue.**
  `BlockKind::heading_level()` now lives in `transync-syntax/src/id.rs`, in
  the same `impl` as `id_code` and `wire_str` because it is the same kind of
  thing: a projection that must be extended in lockstep with the enum. It is
  `#[doc(hidden)]` (workspace-internal, not curated surface, and the facade
  does not re-export it). `parser.rs`'s private copy died with
  `build_hierarchy`, and `transync-core`'s `unit/context.rs` deleted its
  cross-crate byte-copy. **A fourth mapping survives** in
  `unit/payload.rs::constraints_for`; it is literal-singleness debt, not a
  second algorithm, and is filed as TicGit ticket **`69b9b3db`**.
  *(2026-08-05, ticket `69b9b3db` resolved in `7572c45`: it no longer survives.
  `constraints_for` seeds `must_preserve_heading_level` from
  `BlockKind::heading_level()` — the six-arm ladder is gone, `id.rs` is the
  single kind→level home, and the `llm/prompt` golden fixtures are
  byte-untouched.)*

### Part E — Two deletions, one of them a crate-surface removal

Commit `3587d86`.

- **`transync-openai::pagination` is deleted** — module, `split_oversize`,
  and the `pub mod` line. It was an explicit DEFERRED no-op with **zero**
  workspace callers; its consumer-side counterpart (`retry::oversize_split`)
  was already deleted by the OI-0027 curation. This **is** a
  `transync-openai` crate-surface removal made after the 0.2.0 window closed,
  and it is deliberate, per spec §5: `contracts.md` §7 (the provider
  contract) never covered it, a census of the `resp-translator` sibling
  repository shows zero use, the workspace publishes to no registry, and
  keeping a dead public stub alive purely for semver optics contradicts the
  purpose of this wave. **The deferral itself is not retracted** — the
  oversize row-window split stays tracked in `stub-manifest.md` and
  `contracts.md` §5, which is where a deferral belongs, rather than in dead
  code.
- **The `_unused_for_typing` stub is deleted.** `BatchId` is still needed by
  `client.rs`'s test fixture, so its import moved into the test module.
  `pipeline/retry.rs`'s tombstone comment lost its now-dangling `pagination`
  reference.

### Part F — Gates

Every task ran the full standing gate set at its own boundary, through the
pre-commit hook (`--no-verify` was never used). The wave-closing run:

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --workspace -- --test-threads=4` — **385 passed / 0 failed /
  3 ignored** (summed `test result:` lines), against the pre-wave baseline of
  **340 / 0 / 3**. Per binary: `transync-syntax` 108, `transync-core` 186
  (+1 ignored golden-regen helper), `transync-openai` 37 + 2 live-smoke
  (+2 ignored live), the facade's 21 scenario + 6 `boundary_v02` +
  4 `public_surface` + 2 `error_fixtures` + 2 `reader_honesty` +
  2 `docs_index_drift`, and the CLI's 12 + 3 drift.
- `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`
  — 12 + **19 `cli_smoke`** + 3 = 34 passed, 0 failed.
- `cargo check -p transync-syntax --target wasm32-unknown-unknown` — clean.
  The four new `parser/` submodules added no `[features]` table and no
  `transync-core` dependency, so the standing gate is intact.
- `scripts/smoke.sh` — OK, **including** the standing
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` gate: the twenty-plus
  modules created by this wave are documented warning-free.
- `scripts/test-browser.sh` (Playwright SCN-13) — **8/8**.
- `crates/transync/tests/public_surface.rs` — green and **byte-identical to
  tag `v0.2.0`**; `git diff v0.2.0..HEAD -- crates/transync/tests/public_surface.rs`
  is empty. That is the wave's proof that it never touched the curated
  surface.
- `/Volumes/Common/QJoon/resp-translator` — `cargo check --workspace` green
  against this HEAD. No migration was needed: nothing the sibling names
  moved.

### Invariants held

**Nothing on the wire moved.** The alignment-map schema stays **`1.2.0`**
(including the always-null `parent_id` row field), `VALIDATION_SCHEMA_VERSION`
stays **`2`**, `CacheKey`'s identity and shape are untouched, and **both
`sync.js` copies** were not modified. The curated facade surface — the 78-row
`contracts.md` §0 table and its three welded mirrors — is unchanged, and
`transync-syntax` still declares no `[features]` and no dependency on
`transync-core`, dev-dependencies included.

**Zero test removals**, with **one sanctioned swap**, named in the plan's
Global Constraints before dispatch:
`id.rs::rekey_tests::rekeying_remaps_section_path_and_hierarchy` →
`rekeying_renumbers_the_id_sequence_and_remaps_section_path`. Same fixture,
same mutation, and *stronger* on the surviving contract — it asserts the whole
assigned id sequence (gapless ordinals in source order, correct kind prefix
per block) where the original asserted only its two endpoints, and it keeps
the `section_path`-points-at-the-new-heading-id assertion verbatim. Only the
`hierarchy`-lookup half was dropped, because its subject no longer exists.

**The IR freeze rule was honored.** Beyond deleting `Document::hierarchy` and
`Block::parent_id`, the shapes of `Document`, `Block`, and `Section` are
unchanged. Track C decides serde-vs-DTO for the JS boundary; the IR must not
be reshaped twice.

## Why

OI-0008 had been open since 2026-05-04 under an explicit rule — "address as
part of the next concrete feature that touches the relevant module" — and two
waves had honored it: DCR-0017 cleared the two renderer items because the
crate split had to touch the renderer anyway. That rule ran out of road. The
next concrete feature is **Track C**, and Track C ships `transync-syntax` to
the browser. Landing the debt *after* Track C would mean refactoring code that
is already compiled into a WASM artifact with a JS integration on top of it;
landing it before means Track C starts from a parser that does one walk
instead of two, has no dead hierarchy pass, and exposes a `WalkState` rather
than a nine-argument function pair. The double-work risk against Track C's own
IR-boundary design is real and was accepted by the owner; the IR freeze rule
is the mitigation.

**A1 over the alternatives** for retry/fallback because the debt was never
"the policy is wrong" — it was "the policy is unreadable and untestable, being
spread across a `process_one_batch` that also owns provider calls, cache
writes, sleeps, and log lines." Moving the I/O would have been a rewrite;
moving the *decisions* is a lift. The evidence that it was the right cut is
that twelve decision tests now run without a mock translator, and that the
extraction is what surfaced the `total_retries` contradiction and the
per-round provider-budget quirk — neither of which was visible while the
policy was interleaved with the I/O.

**`total_retries` changed instead of the docs** because two independent doc
sites already stated the intended semantics, no test pinned the old value, and
the conflated number was actively misleading: a run that hit one batch-schema
fault reported a content retry that never happened. Fixing the counter makes
`total_retries`, `batch_schema_faults`, and `provider_retries` sum without
double-counting, which is what a report is for.

**Support over normalize or reject for OI-0033** because normalization mutates
source bytes, and byte-verbatim round-trip is what `regen` and `htmlseg`
guarantee — the cheapest posture collides head-on with the strongest
invariant. Reject would have been honest but would have made transync stricter
than CommonMark, which treats lone CR as a line ending. Support is the only
option where the parser agrees with the parser: comrak already counts lone CR,
and the defect was only ever that *our* line table did not.

**The parser deletions are the wave's highest-value change per line.**
`hierarchy` was a full extra pass with allocations, computed on every parse,
read by nothing. `parent_id` was a field threaded through two nine-argument
signatures to be unconditionally `None`, which made five — six — filter sites
into tautologies that *looked* like real invariant checks. Deleting a field
that is always `None` is not a micro-optimization; it removes an entire class
of "wait, when is this `Some`?" from every future reader, including the
reader who will port this walker to a WASM boundary.

## Affected Areas

- `crates/transync-syntax/src/parser/ranges.rs` — CR arm, terminator-aware cap helper, first inline test module
- `crates/transync-syntax/src/parser.rs` — lone-CR parse pin; `hierarchy`/`build_hierarchy` deleted; `parent_id` deleted; `WalkState` + thin `parse`; `Section` frozen with a projection note
- `crates/transync-syntax/src/parser/{options,classify,emit,sections}.rs` (new) — comrak options; kind classification + label tables; the one `emit` + the OI-0033 guard; `SectionStack`
- `crates/transync-syntax/src/id.rs` — `BlockKind::heading_level()` single home; the sanctioned re-key test swap
- `crates/transync-syntax/src/{align,regen,render,walk}.rs`, `src/render/attrs.rs`, `src/parser/refdefs.rs` — the six `parent_id` filter sites; the `data-parent-id` conditional and `render::is_top_level` deleted; wire `parent_id` written as a literal `None`
- `crates/transync-core/src/pipeline.rs` — `run_pipeline` as a thin phase sequence; glossary preflight and cache helpers retained
- `crates/transync-core/src/pipeline/{policy,dispatch,finalize,report}.rs` (new) — the decision core; the per-batch loop; the reparse cascade; the report builder
- `crates/transync-core/src/pipeline/retry.rs` — role unchanged; dangling `pagination` reference removed
- `crates/transync-core/src/structure.rs`, `src/structure/labels.rs` (new) — the structural-fingerprint oracle, out of the unit builder
- `crates/transync-core/src/unit.rs`, `src/unit/{payload,budget}.rs` (new) — thin `build_batches`; the per-kind strategy; budget resolution with one sentinel helper
- `crates/transync-core/src/unit/context.rs` — cross-crate `heading_level` byte-copy deleted
- `crates/transync-core/src/validate.rs`, `src/validate/per_kind.rs`, `src/validate/full_reparse.rs` — `total_retries` field doc; oracle imports repointed to `crate::structure`; residual-(a) comment amended
- `crates/transync-core/src/{llm,test_fixtures}.rs` — doc path fix; `parent_id` skip removed
- `crates/transync-openai/src/client.rs` — flow only; `SurfaceRequest` + `round_trip`
- `crates/transync-openai/src/client/{classify,transport,endpoint,dispatch,chat,responses}.rs` (new), `src/tokenizer.rs` (new)
- `crates/transync-openai/src/pagination.rs` — **deleted**; `src/lib.rs` — `pub mod pagination` removed, `mod tokenizer` added
- `crates/transync-cli/src/translate_cmd.rs` — `execute` seam, `CliFailure`, thin `run`
- `crates/transync-cli/src/translate_cmd/{args,input,provider,publish,report}.rs` (new)
- Records: this DCR (new), `DCR-0017` (dated residual-(a) note), `docs/project/open-issues.md` (OI-0008 RESOLVED, OI-0033 RESOLVED, **OI-0034** filed), `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/index.md`, `CHANGELOG.md`
- Living docs: `docs/implementation/module-map.md` (layout tree gains this wave's submodules, loses `pagination.rs`), `docs/decisions/0016-whole-document-in-memory.md` and `docs/decisions/0008-reject-optional-warnings-in-strict-schema.md` (dated path notes), `docs/architecture/rough-schema.md` (IR sketch loses the two deleted fields), `docs/architecture/mvp-scope.md` and `docs/project/stub-manifest.md` (the oversize-split deferral no longer names a live module), `docs/Developer_Guide.md` (`parent_id` is a wire field, not an IR field)
- Tickets filed, not fixed: **`294ddabd`** (provider budget charged per dispatch round while the knob says per batch), **`69b9b3db`** (the fourth kind→level mapping in `unit/payload.rs`) — *(2026-08-05: both were fixed later the same day, `294ddabd` in `1436304` and `69b9b3db` in `7572c45`; see the dated notes in Part D and below.)*

**Deliberately not rewritten:** the dated snapshots that describe the
pre-wave code — the bodies of earlier DCRs, the specs and plans under
`docs/superpowers/`, the resolved-issue blocks in `open-issues.md`, and
`stub-manifest.md`'s historical STUB rows — keep their as-of-that-date
wording, following the precedent DCR-0017 and DCR-0018 set. Only live
DEFERRED rows and live path claims were touched.

### Discriminating tests

OI-0033 (`transync-syntax`): the six `ranges.rs` tests (lone-CR offsets and
slices, CRLF offsets unchanged, LF offsets unchanged, the mixed CR/LF
item-swallow case, lone CR at EOF, and the cap helper directly),
`lone_cr_document_slices_every_block_to_its_own_bytes` (kinds, exact ranges,
exact slices, the non-empty/non-overlapping/increasing loop, and
`warnings.is_empty()`), `empty_html_range_does_not_trip_the_list_item_guard`
and `list_item_guard_fires_only_for_an_empty_range_at_a_real_position` (the
guard's scope from both sides), and — in `transync-core` —
`lone_cr_list_items_carry_list_topology_constraints`, which is Guard-1
residual (a) asserted as its own predicate.

Policy (`transync-core::pipeline::policy`): twelve tests, of which four carry
the argument —
`offender_redispatch_does_not_consume_the_units_content_budget` (the whole
separation: one batch-fault round plus the full content ladder = four
dispatches), `the_provider_budget_resets_at_each_dispatch_round` (the
preserved per-round quirk, pinned so a future "cleanup" is a red test),
`content_retries_exclude_first_dispatch_batch_faults_and_cache_rows` (the
eight-case truth table), and `max_rounds_is_the_documented_termination_bound`.
Report semantics: `one_content_retry_counts_once`,
`a_batch_fault_round_is_not_a_content_retry`,
`a_rejected_cache_hit_is_not_a_content_retry` — the last two were the RED
pair.

Client (`transync-openai::client::classify`): eleven, of which
`classification_maps_onto_the_retry_surface_the_pipeline_consumes` is the one
that matters — it pins the table *through* `map_provider_error`, i.e. as
`pipeline::dispatch` actually branches on it — plus
`auth_statuses_classify_as_auth_with_bare_body` (the prefix asymmetry) and
`truncate_diagnostic_caps_and_stays_char_safe` (the multi-byte cut).

Parser (`transync-syntax::parser::emit`): the three `ast_path` pins, written
green against untouched code before the `emit` unification.

CLI (`transync-cli::translate_cmd::report`):
`pipeline_diagnostics_order_and_glossary_gating` — emission order, and that a
successful preflight stays silent outside `--verbose`.

Unchanged-by-design suites that prove the motion was motion: all 21 scenario
suites, `boundary_v02`'s retry-context assertions, the 19 `cli_smoke`
subprocess tests, `public_surface.rs`, `sync_js_drift`, `docs_index_drift`,
and the Playwright SCN-13 suite.

## Migration / Follow-up

The 0.2.0 breaking window is **closed**, and this wave respected it: the
curated facade surface is byte-identical to the tag. Two consumer-visible
notes:

- **`validation-report.json`'s `total_retries` can read lower** for a run that
  took a batch-schema-fault round or a rejected cache hit. This is the
  documented semantics finally being true; `batch_schema_faults` and
  `provider_retries` are unchanged, and a run with neither reports the same
  number as before.
- **`transync_openai::pagination` is gone.** Zero in-tree and zero
  sibling-repo callers; see Part E for why this one removal was taken outside
  the window. The oversize-split deferral it stood for is still tracked in
  `stub-manifest.md` and `contracts.md` §5.
- **Engine-internal signatures moved freely** — `pipeline`, `unit`,
  `validate`, `parser`, and the client's internals are `pub(crate)` or
  module-private since DCR-0018, so nothing a curated consumer can name
  changed. A consumer depending on `transync-core` or `transync-syntax`
  *directly* accepts their weaker stability promise and should expect this
  wave's module paths to differ.

Follow-ups deliberately left open:

- **Alignment-map assembly is still inline in `run_pipeline`.** It was the one
  candidate `pipeline/report.rs` did not absorb, because it has no existing
  signature and lifting it would have been design rather than motion. It is an
  open seam and the natural first move for whoever next touches the report
  phase.
- **Ticket `294ddabd`** — `max_per_batch_provider_retries` is documented
  per-batch and charged per dispatch round. Either the doc or the code should
  move; the missing end-to-end transient-retry test is written up in the same
  ticket.
- **Ticket `69b9b3db`** — the fourth heading kind→level mapping in
  `unit/payload.rs::constraints_for` should fold into
  `BlockKind::heading_level()`.
  *(2026-08-05, ticket resolved in `7572c45`: it did fold. `constraints_for`
  seeds `must_preserve_heading_level` from `BlockKind::heading_level()` and
  gained a table test over all fifteen kinds; the goldens did not move. See the
  dated note in Part D.)*
- **Minor items deferred with reasons, from the task reviews:** the redundant
  clamp inside `byte_range_for`; the `line_content_ends` parallel vector's
  per-line memory (revisit only if the wasm footprint matters); a
  once-per-round `debug_assert` on batch-fault admission (a protocol asymmetry
  against `record_dispatch`); the schema-fault warn log reading the options
  budget directly where an accessor would single-home it; and the placement of
  the path constants left in `client.rs`.
- **`Section` has no producer.** It is kept shape-frozen for OI-0005's
  hierarchical-alignment revision, which should project it from the heading
  stack rather than re-derive the scope rule.
- **OI-0034** (new) — comrak's NUL→U+FFFD substitution makes byte *columns*
  exceed the source line length. Non-fatal today because of clamping; filed,
  not fixed.
- **Track C** (wasm-bindgen entry points, JS integration, browser demo) is the
  next wave, and is the reason this one happened first.

## Note (2026-08-05) — `begin_dispatch_round`'s reset is gone

Ticket `294ddabd` (short id `294dda`), listed above as a deliberate follow-up,
was resolved the same day by route (a): the code moved to the documents, not
the other way round. `max_per_batch_provider_retries` is now charged to the
whole input batch — `BatchPolicy::begin_dispatch_round` no longer resets
`provider_attempts`, so a batch spends at most that many transient retries
across its entire retry ladder.

Two statements in this DCR's API-delta discussion of
`begin_dispatch_round()` were true for this wave and are no longer true of the
code: the method's purpose is no longer "to make that reset explicit" (there is
no reset), and the mismatch between the knob's per-batch name and its per-round
charging is no longer preserved. The method itself survives, still load-bearing:
it now opens the round and returns its 1-based ordinal, which `process_one_batch`
checks against `max_rounds` (the orchestrator's own local round counter is gone).
A third statement is superseded with them: Part B's coverage-gap bullet ends
"the predicate itself … is still unpinned end-to-end (also covered by ticket
`294ddabd`)". The ticket did cover it — `pipeline::dispatch`'s
`provider_retry_budget_tests` now drive real `RateLimited` failures through the
dispatch path across two rounds (fast, via a 1 ms `Retry-After`, with no clock
injection), so the transient predicate is pinned where the orchestrator branches
on it.
Everything else recorded here — the pure-policy A1 shape, the no-I/O import list,
the other three API deltas — is unchanged.

The behavior change is recorded in the CHANGELOG under `[Unreleased]` and in
**ADR-0009**'s 2026-08-05 amendment, which owns the retry-ladder semantics.

## Note (2026-08-05) — the five deferred minors are closed

The "minor items deferred with reasons" bullet above is consumed (backlog item
`dcr-0019-minor-cleanups`). All five were taken as zero-behavior-change work;
the corpus-level proof is in the commits. Two of the rationales recorded above
were wrong, and this note corrects them rather than leaving the reader to trust
them:

- **The redundant clamp in `byte_range_for` is gone.** Two of its three clamps
  were provable no-ops — `pos_to_byte` already clamps to the same line's
  content end (so `line_end_cap.max(end_inclusive)` was always
  `line_end_cap`), and a content end never exceeds the source length. The one
  remaining clamp is the one OI-0034 leans on.
- **The `line_content_ends` parallel vector is gone, and its deferral reason
  does not survive.** It was filed as "revisit only if the wasm footprint
  matters"; the cost is *runtime* memory (one `usize` per line, transient,
  alongside a comrak arena an order of magnitude larger), and the module size
  is unaffected either way — so the wasm framing named the wrong quantity. The
  substantive claim, "no call site retains the source", is also false: `parse`
  holds the source it walks and `validate::full_reparse` holds the string it
  built the table from. `LineOffsets` now borrows that string, so the cap is
  derived from the next line start (terminator length 2 exactly for `\r\n` —
  the same two arms `new` already has) instead of being materialized. The
  lone-CR rule is still stated once, and the borrow makes "these offsets belong
  to this source" a type-level fact.
- **The batch-fault admission asymmetry is gone.** `admit_batch_fault_round`
  and `unit_disposition` now debug-assert the round protocol on both sides, the
  way `record_dispatch` already was; the check was verified discriminating by
  making the admission conditional and watching the suite go red.
- **The schema-fault warn log reads its ceiling through
  `BatchPolicy::max_batch_fault_rounds()`**, so "n of m rounds used" comes from
  the module that owns the budget rather than pairing the policy's counter with
  the orchestrator's copy of the options.
- **The endpoint paths and `DEFAULT_BASE_URL` moved into
  `client/endpoint.rs`**, next to the `/v1` de-duplication rule that only means
  anything against them; `client.rs` keeps a `pub(crate)` re-export so `lib.rs`
  is untouched.

The follow-up list's remaining open *action* is the alignment-map assembly
still inline in `run_pipeline` (backlog item `alignment-map-assembly-lift`);
both tickets it named are closed, and **OI-0034** stays filed and open.

## Note (2026-08-05) — the alignment-map open seam is closed

The *Migration / Follow-up* bullet "**Alignment-map assembly is still inline in
`run_pipeline`**" is consumed (backlog item `alignment-map-assembly-lift`).
`pipeline/report.rs` now owns it as `assemble_alignment_map`, so the reporting
phase has both of its products in one module and `run_pipeline` calls it the
way it already calls `build_validation_report`.

The bullet's reasoning was that lifting it "would have been *design*, not
motion, because it has no existing signature". That held for a wave whose
review question was "is this pure motion?", and the design it deferred turned
out to be one signature: the four statements move verbatim, and the interface
is `(doc, final_validated, offsets, opts, detected_source_language,
html_outcomes, validation_report) -> AlignmentMap` — every argument already a
local in `run_pipeline` at that point. The one ordering fact the extraction
makes explicit rather than incidental is now stated on the function: the
`validation_report` argument must be the **composed** report, because
`retried_units` is derived from its per-unit attempt log.

Behavior preservation was checked three ways: the workspace suite (**406
passed / 0 failed / 3 ignored**), a byte-diff of the CLI's full output tree —
`out.md`, the alignment map, and the `--html-out` bundle — regenerated by the
pre-change and post-change binaries over all fourteen checked-in fixtures
(**141 files, zero diff**), and a differential probe of
`ValidationSummary.retried_units` on the one scenario that actually retries.
That probe exposed a coverage gap rather than a regression — the field had no
non-zero assertion anywhere, so a derivation replaced by a hardcoded zero would
have passed — and SCN-07 now pins it at 2 of its 3 units.

## Note (2026-08-06) — OI-0034 is resolved, and its impact estimate was wrong

Two statements above are superseded. The *Migration / Follow-up* bullet
"**OI-0034** (new) … filed, not fixed" and the 2026-08-05 note's closing "**OI-0034**
stays filed and open" both described this wave's state, not the current one:
**OI-0034 is RESOLVED (2026-08-06, ticket `743d27f0`, commit `e75e815`)** under the
owner-decided posture **Normalize at intake**. `transync-syntax::parser::parse`
substitutes U+FFFD for every NUL before comrak is handed the document, so comrak's byte
columns and `parser::ranges`' line table count the same bytes by construction, and the
normalized string is what becomes `Document::source_text` — and therefore what every
`source_range`, every `source_hash`, the alignment map's `document_id`, regen's fallback
bytes and both rendered panes index.

It is the *same goal* as this wave's OI-0033 fix — make the parser agree with the parser —
reached from the other end. Lone CR is content comrak keeps, so the line table moved to
meet it; NUL is content comrak has already discarded by the time it reports a position, so
there is nothing for a line table to agree with and the input had to move instead. The
byte-verbatim round-trip objection this wave recorded against normalizing lone CR does not
transfer: that guarantee is about `Document::source_text`, and the substitution happens
before that field exists.

**The deferred-minor note's last line needs one correction.** "The one remaining clamp is
the one OI-0034 leans on" was true when written and is now false in both halves — nothing
leans on it, and what it was doing was misdescribed. The resolving probe found that the
clamp had been returning the **correct** answer for every block shape tested (paragraph,
ATX heading with and without a closing sequence, list item, fenced code block, multi-NUL
lines), not an approximately-correct one: a block's `Sourcepos` always ends where its
line's content ends, which is exactly where `pos_to_byte` clamps. The drift was only ever
observable at a column strictly *inside* a line — an inline node's position — which is why
the resolving fixture carries trailing whitespace and why a NUL fixture without it would
have passed before and after the fix. `pos_to_byte`'s doc now states what that clamp is
and is not for, and `parser::ranges::tests::raw_nul_columns_drift` keeps the drift
arithmetic on the record.

One residual is ticketed and deliberately out of scope: a NUL arriving in a **translated
payload** still reaches `out.md` verbatim while the pane renders U+FFFD for it
(`d06c4349`). Nothing desyncs — `validate::full_reparse` resolves only start columns of
top-level nodes, which no NUL can precede — so it is an output-consistency asymmetry, not
a mapping defect.
