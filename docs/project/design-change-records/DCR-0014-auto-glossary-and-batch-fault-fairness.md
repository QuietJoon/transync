---
type: DCR
title: Auto-extracted candidate glossary and per-batch schema-fault fairness
description: Two pipeline-quality mechanisms. (A) An opt-in preflight provider call harvests recurring source terminology and merges it into the run's glossary (static entries win, scope forced Global per ADR-0014), reaching every batch's prompt and both glossary-sensitive cache-key components through one effective profile — no CacheKey change. (B) A batch-envelope schema fault is decomposed per unit: innocent batch-mates validate and accept in the round they arrived, and the implicated units are re-dispatched on a new bounded per-batch budget instead of spending their neighbours' content-retry budgets. Amends ADR-0009's retry policy additively.
tags: [change, project-control, DCR-0014]
status: active
---

# DCR-0014: Auto-glossary preflight + batch-fault fairness

- **Date:** 2026-08-03
- **Source:** OI-resolution wave **OI-2026-08**; design D1 (`Pipeline Quality: OI-0026 + OI-0031`); resolves OI-0026 and OI-0031
- **Affected ADRs:** `docs/decisions/0009-reject-retry-policy-changes.md` (dated amendment — a *second*, per-batch retry budget is added beside the per-unit one; verbatim resubmission and the per-unit bound are unchanged); `docs/decisions/archive/0014-section-scoped-glossary-renders-globally.md` (upheld, not amended — extracted entries are forced to `GlobalAcrossDocument` so extraction cannot resurrect section scoping past the loader gate); `docs/decisions/0017-batch-terminal-failures-abort-the-run.md` (explicitly **not** engaged — see below)

## What Changed

### Part A — OI-0026: auto-extracted candidate glossary

Before this wave, concurrent batches shared no terminology state: the static
profile `[[glossary]]` was the *only* cross-batch consistency mechanism, so a
term first seen in batch 2 could drift by batch 3 with nothing to pull it back.

The mechanism that shipped is a **run-constant preflight**, chosen over
first-occurrence pinning. Pinning was rejected on four grounds, recorded here
because it is the obvious alternative and will be re-proposed: it is only
well-defined under sequential dispatch (so it fights `max_concurrent_batches`
head-on, making terminology a function of network jitter); it makes a unit's
translation depend on mutable dispatch-time state, which either poisons
`CacheKey` with completion order or lets a shared cache replay a translation
produced under a different pinned set; it breaks ADR-0009's premise that
retries are comparable verbatim resubmissions; and there is nothing to harvest
resolutions *from* — `UnitResult` is an opaque payload, so deriving
source→target term pairs needs either a second model call per batch (strictly
more cost) or Rust-side string alignment that second-guesses the model
(against invariant 2).

- **Defaulted trait method.** `Translator::extract_glossary(&GlossaryExtractionRequest)
  -> Result<Option<Vec<GlossaryEntry>>, TranslatorError>` (`llm.rs`), with a
  default body returning `Ok(None)`. `Ok(None)` means "this translator does
  not support extraction"; `Ok(Some(vec![]))` means "supported, found nothing
  salient" — the distinction is deliberate and surfaces in the report. A
  defaulted method keeps every existing implementor (the echo/failure stubs,
  external mocks) compiling and correct, and keeps `run_pipeline`'s signature
  and the single-translator identity intact: the extractor *is* the
  translator, so `fingerprint()` needs no new axis. A separate
  `GlossaryExtractor` trait or an extra `run_pipeline` parameter was rejected
  as public-surface growth on the exact seam OI-0027 wants to shrink; reusing
  `translate_batch` with a synthetic batch was rejected because it mis-fires
  schema validation, ID-set equality, and the batch-identity check, and the
  translation schema cannot express a term list.
- **Request shape.** `GlossaryExtractionRequest { source_text, source_truncated,
  source_language, target_language, existing_terms, max_terms }`. The pipeline
  pre-truncates `source_text` at `MAX_EXTRACTION_SOURCE_BYTES` (128 KiB,
  char-boundary safe) and flags the partial harvest via `source_truncated`;
  `existing_terms` are the static glossary's source terms, so the model does
  not spend term slots on already-pinned vocabulary. `max_terms` defaults to
  `DEFAULT_MAX_AUTO_GLOSSARY_TERMS` (24) — the real cost lever is not the one
  extraction call but the ~≤600 tokens of glossary section that then rides
  **every** batch's system prompt.
- **Merge policy** — `profile::merge_auto_glossary(static_entries, extracted,
  max_terms) -> MergedGlossary { entries, accepted, dropped_conflicts,
  dropped_invalid }`. Per extracted entry, in order: sanitize (trim both
  terms; drop when either is empty, when `source_term` exceeds 80 chars or
  `target_term` exceeds 200 chars; truncate `note` to 200 chars) → **static
  wins** on a case-folded source-term collision (`dropped_conflicts`) →
  dedupe within the extraction (first occurrence wins) → **force
  `GlossaryScope::GlobalAcrossDocument`** (ADR-0014) → cap at `max_terms`.
  Output order is every static entry first, then accepted extracted entries
  in provider order — a stable prefix so prompt rendering and hash
  composition stay predictable.
  The length bounds are not cosmetic: they are the **invariant-7
  prompt-stuffing guard**. The glossary is rendered into the trusted system
  prompt, so a hostile document must not be able to steer the extractor into
  emitting paragraph-sized "terms" that function as injected instructions.
  The cap is re-applied in core even though the provider schema already
  stamps `maxItems` — core does not trust the provider.
- **One effective profile, two existing hashes.** `run_pipeline` resolves the
  preflight immediately after `id::assign_block_ids` and shadows its options
  with a `Cow<TranslateOptions>`: `Cow::Borrowed` when the feature is off or
  degraded, `Cow::Owned` carrying the merged profile when a harvest landed.
  Everything downstream — `build_batches`, `CacheKeyContext::for_run`,
  `key_for` — reads the *same* effective profile, so prompt text, the
  `TranslationBatch.glossary` wire field, and cache identity agree **by
  construction** rather than by convention. Cache identity therefore needed
  **no new `CacheKey` field and no `Cache`-trait change**: the merged entries
  enter the key twice through fields that already exist —
  `CacheKey.glossary_hash` (the raw `(source, target, note)` triples) and
  `CacheKey.profile_prompt_hash` (the rendered prompt body, which appends the
  glossary section). Two runs with different effective glossaries cannot
  collide. `VALIDATION_SCHEMA_VERSION` is likewise **not** bumped: the
  per-unit prompt contract and `UnitResult` semantics are unchanged, and the
  bump discipline explicitly excludes content the key already covers.
- **Degradation is not an abort.** `Ok(None)` → `status: Unsupported`; `Err`
  → a `tracing::warn!` plus `status: Failed` with a 512-byte-truncated
  diagnostic; a document with no translatable block → `status: Skipped` with
  no call made. All three proceed on the static glossary and produce output
  identical to an opted-out run — which also means they are *key-identical*
  to an opted-out run, so cached static-glossary translations stay reusable
  across the degraded states. This is deliberately **not** an ADR-0017
  batch-terminal event: no batch was dispatched, output language coverage is
  unaffected, and the degraded state equals the shipped baseline. Extraction
  gets no transport-retry loop in v1 — it is advisory, and backoff would add
  worst-case latency to every enabled run for marginal benefit.
- **Opt-in surface, default OFF.** `TranslateOptions.auto_glossary:
  Option<bool>` → profile top-level `auto_glossary` → built-in `false`;
  `Option<bool>` rather than `bool` so an explicit opt-out is expressible
  over a profile that enables it (avoiding the documented
  "explicit-default-is-indistinguishable-from-unset" wart of
  `max_units_per_batch`). CLI: `--auto-glossary` / `--no-auto-glossary`
  (clap-conflicting pair). Off by default because it costs one extra provider
  call per run *including a fully cache-warm one* (keys cannot be known
  before the merge), taxes every batch's prompt, and can vary run-to-run at
  nonzero sampling temperature — which lowers warm-cache hit rates without
  ever threatening correctness (over-distinguishing is the safe direction).
  The drift it prevents starts mattering at ≥3 batches, so a long-document
  project profile is where it belongs.
- **Observability.** `ValidationReport.auto_glossary: Option<AutoGlossaryReport>`
  (`skip_serializing_if = "Option::is_none"`, so opted-out runs emit
  byte-identical report JSON to the pre-wave baseline).
  `AutoGlossaryReport { status, accepted_terms, dropped_conflicts,
  dropped_invalid, source_truncated, error, terms }` with
  `AutoGlossaryStatus ∈ { extracted, unsupported, failed, skipped }`. The CLI
  prints a one-line summary under `--verbose`, and surfaces a `Failed`
  extraction on stderr for **every** non-quiet run — silently degrading an
  explicitly requested feature would be misleading.

**Recorded deviation from the D1 design: where the extraction wire contract
lives.** D1 specified the extraction prompts, schema object, schema name, and
output parser as `transync-openai` additions, because D1 was written before —
and independently of — the OI-0029 prompt lift (DCR-0015). Once the lift
landed in the same wave, honoring D1 literally would have created a second
provider-owned wire contract in the crate the lift had just emptied of
provider-neutral assembly. The implementation therefore adapted D1: the whole
provider-neutral half of the extraction contract lives in
`transync-core::llm::prompt` beside its translation sibling —
`EXTRACTION_SCHEMA_NAME`, `EXTRACTION_SYSTEM_PROMPT`, the instruction const,
`build_extraction_user_prompt`, `extraction_schema_object`, and
`parse_extraction_output`. Only genuinely OpenAI-specific pieces stayed
provider-side: `client::call_glossary_extraction`, the two surface-specific
request-body builders, and the fixed `EXTRACTION_MAX_OUTPUT_TOKENS` (4096)
output cap that bounds a runaway term list without touching the profile's
translation ceiling. This is the intended end state, not a compromise: a
second provider inherits the data-framing discipline and the strict schema for
free, exactly as it does for translation.

### Part B — OI-0031: batch faults stop spending batch-mates' budgets

Before this wave, any schema-layer finding (a requested id missing from the
result, an id returned twice, a foreign id, a duplicated *request* id) made
`validate_batch` blanket-reject **every requested unit** with the same reason.
`process_one_batch` then charged every unit's retry counter, re-dispatched all
of them, and eventually accepted all of them as `fallback_source` — so a
provider that deterministically drops one unit dragged every batch-mate
through the full ladder into fallback. That is the OI's "fallback cluster
around one bad unit".

The fix is a hybrid of the two options the OI offered, because they answer
different halves of the problem: **salvage** decides who is innocent, and a
**separate budget** decides who pays.

- **Classification.** `validate/schema.rs` gains
  `SchemaClassification { request_duplicates, missing, duplicated, foreign }`
  plus `classify(batch, result)`, `is_clean()`, `offenders()` (missing ∪
  duplicated) and `summary()`. Every vector is sorted by id string so reasons
  and reports are stable. `check_schema` is retained with its exact signature
  and message strings, reimplemented over `classify` and preserving today's
  first-error precedence (request-dup → result-dup → foreign → missing), so
  its existing tests and any external caller are untouched
  (`check_schema_first_error_strings_are_unchanged`).
- **Salvage.** `validate_batch` is now **total over the requested units**:
  every requested id appears in `ValidatedBatch.units` exactly once,
  regardless of how the provider mangled the reply. A unit with exactly one
  returned row is *innocent* and flows through `validate_unit` (per-kind →
  fragment reparse → inline) exactly as if the batch had been clean, and an
  accepted one is cached and finalized in round 1. This is safe because every
  content and structure property is still independently proven for that unit —
  a sibling's missing row says nothing about a present payload. The literal
  reading of "re-dispatch the batch minus the offender" (discard the
  innocents' results and re-send them) was rejected: it burns provider calls
  to reproduce results already in hand.
- **Offenders and the new budget.** Requested ids that are missing, or
  returned more than once (every copy discarded — there is no principled way
  to pick one), become `ValidationLayer::Schema` rejections with no payload
  and are re-dispatched **verbatim** with a non-content `RetryContext`
  (ADR-0009's side channel, `rejected_by: Some(Schema)`), charged to the new
  `TranslateOptions.max_per_batch_schema_retries` (default **2**). The charge
  is **once per round** in which a fault with ≥1 offender was observed, not
  once per offender, so an entirely empty response costs one budget point
  rather than N. Once the budget is spent the offenders finalize as honest
  `fallback_source`. Foreign ids are discarded and charged to **nobody** — no
  requested unit is at fault and there is nothing to re-dispatch — surfaced
  via `tracing::warn!` and `BatchFault.foreign_ids`.
  The separation is the whole point: a dropped unit's *content* was never
  judged, so spending its content budget on the provider's formatting failure
  would conflate "this provider can't format this batch" with "this provider
  can't translate this block", and would leave a twice-dropped unit with
  fewer real translation attempts than a merely-mistranslated one.
- **Attribution type.** `ValidatedBatch` gains
  `batch_fault: Option<BatchFault>`, where `BatchFault { reason, offenders,
  foreign_ids, malformed_request }` carries exactly the routing information
  the pipeline needs. `ValidationLayer::IdSet` stays reserved/unused; its doc
  comment now points at `BatchFault`.
- **Counter split.** `process_one_batch`'s single `attempt_counter` becomes
  `dispatch_counter` (bumped whenever a unit receives an `AttemptOutcome` row
  from a network round — drives `attempt_number` and `RetryContext.attempt`),
  `unit_fault_counter` (bumped only on a unit-scope rejection — drives the
  per-unit budget), and the scalar `batch_fault_rounds` per input batch. For
  any history with no batch fault the two maps coincide, so attempt
  numbering, the `retried_units` summary, and every pre-wave test expectation
  are unchanged.
- **Termination bound.** Each round ends in exactly one of: no retry units
  (break); ≥1 unit-fault retry (that unit's fault counter strictly increases,
  bounded by the per-unit budget, and exhausted units are finalized, never
  re-queued); ≥1 offender re-dispatch (`batch_fault_rounds` strictly
  increases, bounded by the per-batch budget). Total rounds per input batch
  are therefore `≤ 1 + max_per_batch_schema_retries + U ×
  max_per_unit_validation_retries`. A fully hostile provider (always empty,
  always duplicated, always foreign-only) terminates in
  `1 + max_per_batch_schema_retries` calls per batch with every unit at
  honest `fallback_source` — pinned by `fully_hostile_empty_result_terminates`.
- **One deliberate hard edge.** Duplicate `unit_id`s in the **request** are a
  caller-side contract violation that `build_batches` can never produce, and
  re-dispatching an identical malformed request is futile by construction.
  `process_one_batch` now preflights request-id uniqueness *before* the loop
  and returns `Err(TransyncError::Validation(...))` naming the ids — a loud
  abort replacing the old silent burn-everyone's-budget-then-fallback.
  Unreachable through `run_pipeline`; `validate_batch` keeps a defensive
  total behavior (all units rejected with the legacy message,
  `malformed_request: true`) for direct library callers.
- **Reporting.** `AttemptOutcome.batch_fault: bool`
  (`skip_serializing_if = "std::ops::Not::not"`, so every pre-wave row
  serializes byte-identically) distinguishes "charged to the batch budget"
  from "this unit's own output was bad". `ValidationReport.batch_schema_faults:
  u32` totals the charged rounds across all batches, plumbed like
  `provider_retries` through `BatchResult` / `Aggregated`.
- **ADR-0017 is untouched.** Schema faults are *validation*-layer events with
  a fallback path, not provider/transport-terminal errors. A `TranslatorError`
  still aborts the run, and the `result.batch_id != current_batch.batch_id`
  identity check stays terminal.

## Why

Both halves fix a **fairness** defect in how the pipeline attributes blame.

Part A: the profile glossary can only cover terms the author knew to list up
front, so on any document long enough to need several batches the engine had
no answer for drift — and the only alternatives that observe terms *in
context* all require serializing dispatch or corrupting cache identity. A
run-constant preflight buys consistency without touching concurrency, the
retry contract, or the cache key's shape, and its failure mode is exactly the
behavior that shipped before it existed.

Part B: charging a per-*batch* envelope fault to per-*unit* content budgets
punishes units whose output was never even examined, and it produces
misleading diagnostics — a fallback cluster reads as "several blocks are hard
to translate" when the truth is "the provider mangled one response envelope".
Salvage keeps valid work, and a second bounded budget keeps the guarantee the
first one was making: a unit gets its full complement of *content* attempts
before it is allowed to degrade.

## Affected Areas

- `crates/transync-core/src/llm.rs` — `GlossaryExtractionRequest`,
  `DEFAULT_MAX_AUTO_GLOSSARY_TERMS`, `MAX_EXTRACTION_SOURCE_BYTES`, defaulted
  `Translator::extract_glossary`
- `crates/transync-core/src/llm/prompt.rs` — extraction system prompt,
  instruction, `build_extraction_user_prompt`, `extraction_schema_object`,
  `EXTRACTION_SCHEMA_NAME`, `parse_extraction_output` (see the recorded
  deviation above)
- `crates/transync-core/src/lib.rs` — `TranslateOptions.auto_glossary`,
  `TranslateOptions.max_per_batch_schema_retries`, `model_id`/retry rustdoc,
  re-exports (`GlossaryExtractionRequest`, `MergedGlossary`,
  `merge_auto_glossary`, `AutoGlossaryReport`, `AutoGlossaryStatus`,
  `BatchFault`, the two new consts), module-doc bullets
- `crates/transync-core/src/profile.rs` — `ProfileMetadata.auto_glossary`,
  `ProfileToml` mirror + `TOP` known-key list, `merge_auto_glossary` +
  `MergedGlossary`, sanitize bounds
- `crates/transync-core/src/unit.rs` — `has_translatable_blocks`
- `crates/transync-core/src/validate/schema.rs` — `SchemaClassification`,
  `classify`, `request_duplicate_message`, `check_schema` reimplemented over
  the classification
- `crates/transync-core/src/validate.rs` — `ValidatedBatch.batch_fault`,
  `BatchFault`, salvage flow, `AttemptOutcome.batch_fault`,
  `ValidationReport.batch_schema_faults` + `.auto_glossary`,
  `AutoGlossaryReport` / `AutoGlossaryStatus`
- `crates/transync-core/src/pipeline.rs` — `resolve_auto_glossary` + the
  `Cow<TranslateOptions>` shadow, request-uniqueness preflight, counter split
  and offender routing, `batch_schema_fault_rounds` aggregation
- `crates/transync-openai/src/lib.rs` + `src/client.rs` —
  `extract_glossary` override, `call_glossary_extraction`, the two extraction
  request-body builders, `EXTRACTION_MAX_OUTPUT_TOKENS`
- `crates/transync-cli/src/translate_cmd.rs` — `--auto-glossary` /
  `--no-auto-glossary`, flag resolution, `--verbose` summary + stderr notice
- `crates/transync-core/profiles/default.toml`,
  `crates/transync-cli/profiles/default.toml` — commented `auto_glossary` key
- `docs/architecture/contracts.md` §1 / §2 / §5 / §5a / §6

### Discriminating tests

Part A: `auto_glossary_merges_into_prompts_and_batches`,
`auto_glossary_static_entry_wins_on_conflict`,
`auto_glossary_extraction_failure_degrades_not_aborts`,
`auto_glossary_unsupported_default_is_reported`,
`auto_glossary_changes_cache_identity`,
`auto_glossary_disabled_makes_no_extraction_call`,
`auto_glossary_skips_document_without_translatable_blocks`,
`extraction_source_truncation_is_char_safe`,
`failed_status_diagnostic_is_bounded`,
`report_json_omits_auto_glossary_when_absent`,
`merge_auto_glossary_caps_dedupes_sanitizes_and_forces_scope`,
`auto_glossary_profile_key_parses_and_unknown_key_warning_absent`,
`has_translatable_blocks_matches_batch_emptiness`,
`extraction_schema_stamps_max_items_and_strict_required`,
`extraction_user_prompt_frames_document_as_data`,
`parse_extraction_output_happy_path_and_rejects_invalid_json`,
`extraction_request_bodies_use_surface_specific_shapes`,
`extraction_chat_body_carries_data_framed_document`.

Part B: `batch_fault_spares_innocent_units` (the discriminating test — fails
under the pre-wave blanket rejection), `fully_hostile_empty_result_terminates`,
`duplicate_result_id_charges_only_the_duplicated_unit`,
`foreign_ids_are_discarded_without_charges`,
`mixed_batch_and_unit_faults_drain_separate_budgets`,
`offender_keeps_full_unit_budget_after_batch_fault`,
`recovering_offender_ends_translated`, `request_duplicate_ids_abort_loudly`,
`attempt_outcome_batch_fault_is_skipped_when_false`,
`clean_report_counts_no_batch_schema_faults`, plus the `classify` /
`validate_batch` unit suites (`missing_ids_are_classified_and_sorted`,
`duplicated_result_rows_are_classified_not_missing`,
`foreign_ids_are_classified_without_offenders`, `missing_and_foreign_combine`,
`request_duplicates_are_named`, `dropped_row_rejects_only_its_own_unit`,
`duplicated_row_discards_both_copies`,
`foreign_row_is_dropped_and_named_without_offenders`,
`request_duplicate_rejects_every_unit_with_legacy_message`).

## Migration / Follow-up

Breaking / wire changes ride the **still-open 0.2.0 window**. All of the
struct-field additions have `Default` or serde defaults, so
`..Default::default()` callers are unaffected; only exhaustive struct literals
break.

- `TranslateOptions` gains `auto_glossary: Option<bool>` (default `None`) and
  `max_per_batch_schema_retries: u32` (default `2`).
- `ValidatedBatch` gains `batch_fault: Option<BatchFault>` (derives
  `Default`).
- `AttemptOutcome` gains `batch_fault: bool` — `Serialize`-only and
  skip-serialized when false, so report JSON is unchanged until a batch fault
  actually occurs.
- `ValidationReport` gains `batch_schema_faults: u32` and
  `auto_glossary: Option<AutoGlossaryReport>`; both are JSON-additive, and
  `auto_glossary` is absent entirely on opted-out runs.
- `ProfileMetadata` gains `auto_glossary: Option<bool>` (serde-defaulted; the
  TOML key is additive and its absence keeps today's behavior).
- **Behavior change for direct `process_one_batch`/`validate_batch` callers:**
  a request batch carrying duplicate `unit_id`s is now a loud
  `TransyncError::Validation` abort in the pipeline instead of a silent
  all-fallback outcome. Unreachable through `run_pipeline`, whose batcher
  cannot emit duplicate ids.
- `CacheKey`, the `Cache` trait, `VALIDATION_SCHEMA_VERSION`, and the
  alignment-map schema (`1.1.0`) are all **unchanged**.
- Nothing needs to be done to keep pre-wave behavior: auto-glossary is off
  unless asked for, and a clean provider response never produces a
  `BatchFault`.

### Note (2026-08-06) — one of the two profile paths above no longer exists

*Affected Areas* lists both `crates/transync-core/profiles/default.toml` and
`crates/transync-cli/profiles/default.toml` as carrying the commented
`auto_glossary` key. Only the `transync-core` half was ever `include_str!`d,
and the CLI-side file — referenced by no source, test or script — was deleted
on 2026-08-06 (Review-0001 finding R0001-0041) after its comments had already
drifted from the core copy. The commented key this record added is unchanged
and still lives in the surviving file.

### Note (2026-08-07) — a failed extraction lost its `tracing::warn!`

*Degradation is not an abort* says `Err` produces "a `tracing::warn!` plus
`status: Failed`", and *Observability* says the CLI surfaces a `Failed`
extraction on stderr for every non-quiet run. Both were true when this record
was written, and together they became a defect once the reference binary
installed a `tracing` subscriber (R0001-0032, ticket `7f922fa1`): the two
sentences then landed on the same stderr in the same run, one fact told twice
and told apart only by a level prefix. Ticket `33e178` kept the CLI's copy and
deleted the library's. The
`Err` arm now raises no `tracing` record at all — the only degradation in
`transync-core` that raises none — because the CLI line is unconditional where
a log record follows the level filter, and OI-0026's requirement is that an
explicitly requested feature's degradation be visible *without* `--verbose`.
`status: Failed`, its 512-byte-truncated diagnostic, and everything else this
record specifies are unchanged; `Unsupported` keeps its `tracing::info!`.
contracts.md §6 carries the standing rule and this exception.
