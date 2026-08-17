# OI-0008 + OI-0033 Internal-Quality Wave — Design Spec

- **Date:** 2026-08-05
- **Status:** owner-approved design (approach A1; sections S1–S8 approved 2026-08-05)
- **Issues:** OI-0008 (refactor debt cluster — R0001-0057 retry/fallback policy, R0001-0076..0080
  concern-mixing), OI-0033 (lone-CR `LineOffsets` desync) — `docs/project/open-issues.md`
- **Owner decisions (2026-08-05):** OI-0033 posture = **Support** (make the line table match
  comrak); OI-0008 scope = **full**, including the parser `visit` restructure (the double-work
  risk against Track C's IR-boundary design is accepted); retry/fallback architecture = **A1,
  pure policy module** (decision core extracted, I/O stays in the orchestrator).
- **Window status:** post-0.2.0 — the breaking window is CLOSED. Everything here is internal
  (`pub(crate)` reshaping, `transync-syntax`'s own crate surface, bug fix). The facade §0
  surface, the alignment schema (1.2.0), `VALIDATION_SCHEMA_VERSION` (2), `CacheKey`, and both
  `sync.js` copies are untouched; the `public_surface.rs` drift gate must stay green unmodified.
- **Exploration evidence:** three-reader workflow 2026-08-05 (retry-policy map, concern-mixing
  map with per-site rankings, OI-0033 mechanics with comrak probes). Probes confirmed comrak
  0.27 counts lone CR as a line ending (`strings.rs::is_line_end_char` matches `10 | 13`;
  terminator consumed as optional-`\r` + optional-`\n`) and that CR-aware offsets are
  byte-identical to today's for every LF/CRLF document.

## 0. Goals and behavior-change budget

Two issues, one wave. The behavior-change budget is exactly **two** deliberate changes:

1. **Lone-CR sources now slice correctly** (OI-0033 Support): previously every block after the
   first lone CR got an empty or neighbor-swallowing payload, silently.
2. **`ValidationReport.total_retries` counts content retries only** (S2): today
   `build_validation_report` computes `attempts.len() - 1`, which also counts batch-schema-fault
   rounds and rejected-cache-hit re-validations — contradicting the documented claim (in both
   `pipeline.rs` and `validate.rs` doc text) that `batch_schema_faults` is "counted apart from
   `total_retries`". The docs are right; the code changes. No test pins the old value; new tests
   pin the new one.

Everything else is behavior-preserving code motion, verified by the existing suite
(340 passed / 0 failed / 3 ignored at HEAD `555cd70`, Playwright 8/8, smoke.sh incl. the
standing rustdoc gate).

## 1. OI-0033 — Support: make `LineOffsets` agree with comrak (S1)

`crates/transync-syntax/src/parser/ranges.rs`:

- `LineOffsets::new` gains exactly one arm — push a boundary for `b'\r'` whose next byte is not
  `b'\n'` — mirroring comrak's rule and nothing more (probe-verified: VT, FF, NEL, U+2028,
  U+2029, NUL are NOT line endings; `str::lines()` and Unicode line-break APIs must not be
  used). The doc comment names comrak's `is_line_end_char` as the authority. Because the arm
  fires only where a lone `\r` exists, the offsets vector is byte-identical for every LF/CRLF
  document — behavior-preserving by construction on the entire current corpus.
- The `line_end_cap` derivation currently duplicated between `pos_to_byte` and
  `byte_range_for` is extracted into one shared, terminator-length-aware helper (subtract the
  real terminator length, so a CRLF line's cap excludes the `\r` too). Not reachable today
  (probe: zero over-column hits on block nodes), but the duplication is a drift hazard and the
  cap looseness an ambiguity.
- New `#[cfg(test)] mod tests` in `ranges.rs` (the file has none): lone-CR offsets + slices,
  CRLF offsets unchanged, LF offsets unchanged, mixed CR/LF item-swallow case, lone-CR at EOF.
  Fixtures are inline `&str` constants — NO checked-in `.md` fixture (no `.gitattributes`
  protects CR bytes; editors/autocrlf can silently rewrite them into a test that passes
  vacuously; fixture files also create `.ko.md` mirror obligations).
- Parse-level assertion in `parser.rs`'s existing inline-test style: a lone-CR document yields
  non-empty, non-overlapping, in-order `source_range`s for every block.
- **Defensive guard:** a translatable **list-item** block whose source position is
  non-degenerate but whose payload comes back empty produces a parse warning (scoped to list
  items — SCN-15 legitimately ships an empty html range that must not trip it). This makes the
  next sourcepos quirk loud instead of silently disarming `per_kind::check_list`.
- **Guard-1 residual (a) closure:** a test asserts that for a lone-CR list document every
  `li-*` unit's `constraints.expected_list_topology` is `Some(_)` (the exact residual
  predicate). After it passes, DCR-0017's residual note and the mirroring comment in
  `validate/full_reparse.rs` drop their lone-CR caveat and read "residual (a) is void"
  unqualified.
- **New OI filed (not fixed here):** comrak's NUL→U+FFFD substitution makes byte columns exceed
  the source line length (a 1-byte NUL reported as 3 bytes wide) — a column-desync sibling of
  OI-0033, non-fatal today because of clamping.

## 2. Retry/fallback policy module (S2 — R0001-0057, approach A1)

New `crates/transync-core/src/pipeline/policy.rs`, a **pure decision core** with no I/O:

- Owns the budget state currently scattered through `process_one_batch`: the per-unit fault
  counter, the per-batch schema-fault round counter, the provider-transient attempt counter,
  and the budget values lifted from `TranslateOptions`.
- Exposes decision methods returning data, one per decision point in the exploration's map:
  unit disposition after validation (accept / retry-unit / redispatch-offender / finalize as
  fallback), batch-fault round admission, transient-error handling returning
  `Option<Duration>` (the backoff computation — `Retry-After` capped at 30 s, else the
  exponential `200ms << min(attempts-1, 5)` capped at 5 s — moves here as a pure function),
  and the documented termination bound
  (`1 + max_per_batch_schema_retries + units × max_per_unit_validation_retries`).
- `process_one_batch` keeps the I/O and logging: provider calls, cache get/put/evict, sleep,
  attempt-row emission. Every branch that today embeds policy asks the policy object instead.
- `pipeline/retry.rs` keeps `retry_validation_unit` (the ADR-0009 side-channel builder) and
  `MAX_RETRY_REASON_BYTES` unchanged.
- **`total_retries` semantics fix** per §0 item 2, with new policy-level unit tests pinning:
  a batch-schema-fault round does not increment it; a rejected cache-hit re-validation does
  not increment it; a per-unit content retry does. `batch_schema_faults` and
  `provider_retries` reporting is unchanged.
- The policy module gets direct unit tests for every decision method — the whole point of A1
  is that these no longer need a mock translator to exercise.

## 3. `run_pipeline` phase split (S3 — R0001-0076)

`pipeline.rs` (~3,700 lines) splits into orchestration plus submodules, all `pub(crate)`:

- `pipeline/dispatch.rs` — concurrency fan-out, the per-batch loop (`process_one_batch`),
  cache partition/re-validation, batch-id identity check, retry-batch construction.
- `pipeline/finalize.rs` — regen, the DCR-0004 full-reparse cascade
  (`finalize_regen_with_reparse_policy`, widen/escalate, `fall_back_all`), post-cascade
  eviction and fallback attribution.
- `pipeline/report.rs` — `build_validation_report`, alignment-map assembly, summary tallies.
- `pipeline/policy.rs` — §2. `pipeline/retry.rs` — unchanged role.
- `pipeline.rs` itself keeps `run_pipeline` as a thin sequence over those phases plus the
  glossary preflight and output-budget preflight it already fronts.
- Inline `#[cfg(test)]` tests move WITH the code they pin (zero test removals). File-as-module
  layout throughout; never `mod.rs`.

## 4. `unit.rs` split (S4 — R0001-0077)

- The structural-fingerprint oracle (38% of the file) moves to
  `crates/transync-core/src/structure.rs` + `structure/labels.rs`: `ListFacts`,
  `inspect_list_topology`, `inspect_blockquote_children`, `inspect_table`,
  `block_node_kind_label`, `item_child_kinds`, `blockquote_child_label`.
  `validate/per_kind.rs` imports `crate::structure::*` — removing the current inversion where
  the validator depends on the unit builder.
- `unit/payload.rs` — the per-kind (payload, input_mode, constraints) strategy, absorbing
  `input_mode_for`, `constraints_for`, and the inlined html-segment arm.
- `unit/budget.rs` — profile + input-token + unit-cap + output-cap resolution into
  `BatchBudget`, with ONE shared sentinel helper replacing the three copies of the
  "caller value differing from the built-in default wins" rule.
- `unit.rs` keeps `build_batches` as thin orchestration. **Path preservation:**
  `unit::build_batches` and `unit::html_outcomes` keep their exact paths — transync-openai's
  live_smoke fixture test reaches them via dev-dependency (the sole reason `unit` is `pub`).
- The empty-document early return currently happens BEFORE profile/budget resolution; the two
  batch-emptiness↔`has_translatable_blocks` agreement tests pin the outcome but not the
  ordering. The split preserves the ordering and adds a comment naming it as deliberate
  (a malformed profile must not surface on an empty document).

## 5. OpenAI client split (S5 — R0001-0079)

`crates/transync-openai/src/`:

- `client/dispatch.rs` (`Api`, `api_for_model`, `api_from_env_or_model`),
  `client/chat.rs` (Chat DTOs + translation/extraction body builders + `extract_chat_output`),
  `client/responses.rs` (same for Responses), `client/transport.rs` (`post_json` reduced to
  send + size cap + accumulation ONLY), `client/classify.rs` (HTTP-status/reqwest →
  `ProviderError` classification, `parse_retry_after`, `truncate_diagnostic`,
  `oversize_response_error`), `client/endpoint.rs` (`build_endpoint`), and `tokenizer.rs`
  (`tokenizer_hint_for_model` leaves the HTTP client entirely).
- **One surface abstraction** (an enum plus per-use-case free functions is sufficient — no
  trait needed): translation currently uses two whole call functions while glossary extraction
  re-inlines the entire per-surface flow inside one `match`, duplicating endpoint construction,
  transport, and error mapping. Both flows converge on the same shape; a third surface or use
  case becomes additive.
- **Test-first constraint:** the status-classification table (408/409/425 → retryable
  Transport, 429 → `RateLimitedAfter`, 401/403 → auth, oversize → terminal) is currently
  UNTESTED and is the pipeline's retry-policy input. Its pinning tests are written BEFORE the
  extraction, and the table's behavior must not change (S2's policy consumes it as-is).
- Delete the dead `_unused_for_typing` stub and the dead `pagination.rs` module
  (`split_oversize` — an explicit DEFERRED no-op with zero workspace callers; its core-side
  counterpart was already deleted by OI-0027). The oversize-split deferral remains tracked in
  the stub manifest / contracts §5, not by dead code. Note this IS a `transync-openai`
  crate-surface removal post-window: deliberate — contracts §7 (the provider contract) never
  covered it, the resp-translator census shows zero use, the workspace publishes to no
  registry, and keeping a dead public stub alive purely for semver optics contradicts this
  wave's purpose. Recorded in DCR-0019.

## 6. CLI `translate_cmd` split (S6 — R0001-0080)

`crates/transync-cli/src/`:

- `translate_cmd/args.rs` (flag validation/resolution helpers + `OutputTarget`),
  `translate_cmd/input.rs` (`read_capped`, profile resolution + error types),
  `translate_cmd/provider.rs` (both cfg-gated `translator_for_run` variants + the stub echo
  side channel), `translate_cmd/publish.rs` (ONE file-set builder replacing the two ~40-line
  duplicated `OutputTarget` arms; the arms collapse to a commit-function choice),
  `translate_cmd/report.rs` (`format_auto_glossary_summary` plus the currently-inline
  skipped-node / budget-warning / verbose-tally renderers as pure functions).
- **The one structural change:** `run` gains an inner
  `async fn execute(args) -> Result<Summary, CliFailure>` where `CliFailure` carries
  `(ExitCode, String)`; the outer `run` does report-then-exit. This collapses the ~15 scattered
  exit-code decision sites into one table and makes the command testable in-process.
- Exit codes, output shapes, and flag precedence are all pinned externally by the 19
  `cli_smoke.rs` subprocess tests — the refactor is verifiable end-to-end without new tests.
  `output.rs` (zero inline tests) is out of bounds this wave.

## 7. Parser walker rework (S7 — R0001-0078, full scope)

**Subtractive slice** (all consumer-visible only inside the workspace; the facade is
unaffected — `transync-syntax`'s own crate surface is workspace-internal by the split's
design):

- Delete `Document::hierarchy` + `build_hierarchy` + the `assign_block_ids` re-key pass — its
  own doc admits "no pipeline consumer yet"; it costs a full extra pass + allocations on every
  parse, i.e. exactly the code path Track C ships to the browser.
- Delete `Block::parent_id` (provably always `None`: `walk_children` is called once with
  `None` and `visit` never recurses) and simplify the five now-tautological filter sites
  (`refdefs`, `regen`, `render`, `render/attrs`, `align`). The WIRE field
  `AlignmentBlock.parent_id` stays — schema 1.2.0 pins it, and it is already always `null`,
  so emitted bytes are unchanged.
- Unify `emit`: the List arm currently re-implements the emit closure inline (with a subtly
  different `ast_path`); one free `emit` function serves both.
- ONE section-scope algorithm: `visit`'s heading-stack and `build_hierarchy`'s independent
  re-derivation collapse (the latter dies with `hierarchy`; the survivor is the single home).
- ONE `heading_level` mapping: today it exists three times (level→kind and kind→level in
  `parser.rs`, plus a byte-copy of the latter in core's `unit/context.rs`); the syntax crate
  exports one and core consumes it.

**`visit` restructure** (owner-approved despite Track C adjacency):

- `parser/options.rs` (comrak config), `parser/classify.rs` (NodeValue→`BlockKind` incl.
  image-only promotion and both label tables), `parser/emit.rs` (the one emit),
  `parser/sections.rs` (the one heading-scope stack); `parser.rs` keeps the IR types and a
  thin `parse`.
- The 9-parameter `walk_children`/`visit` pair collapses into a `WalkState<'a>` struct —
  removing both `#[allow(clippy::too_many_arguments)]`.
- **IR freeze rule:** beyond the two field deletions above, `Document`/`Block`/`Section`
  shapes are frozen this wave — Track C decides serde-vs-DTO for the JS boundary, and the IR
  must not be reshaped twice.
- Wasm-gate rules hold throughout: no `[features]`, no `transync-core` dependency
  (dev-dependencies included) in `transync-syntax`; new submodules are free.

## 8. Verification plan (S8)

- Full standing gates at every landing point: fmt, clippy `-D warnings`,
  `cargo test --workspace -- --test-threads=4`, wasm gate, smoke.sh (incl. rustdoc gate),
  Playwright SCN-13.
- The suite count may only GROW (new ranges/policy/classify tests); zero test removals —
  inline tests move with their code.
- `public_surface.rs` (facade drift gate) green UNMODIFIED — proof the wave never touched the
  curated surface.
- resp-translator: `cargo check --workspace` green (no surface change expected; check-only).
- Behavior-preservation spot pins: scenario suite SCN-01..15 byte-stable; `boundary_v02`
  retry-context assertions unchanged; CLI smoke exit codes unchanged.
- New-behavior pins: lone-CR fixture tests (§1), `total_retries` semantics tests (§2),
  classification-table tests (§5).

## 9. Records

- **DCR-0019** documenting the wave (structure maps before/after, the two behavior changes,
  the policy-module architecture, migration notes for `transync-syntax` field deletions).
- **OI-0008 → RESOLVED** (all bullet groups consumed: R0001-0057 by §2, R0001-0076 by §3,
  R0001-0077 by §4, R0001-0079 by §5, R0001-0080 by §6, R0001-0078 by §7).
- **OI-0033 → RESOLVED** (§1; Guard-1 residual (a) closure recorded; DCR-0017 note and
  `full_reparse.rs` comment amended).
- **New OI filed:** NUL column desync (§1 last bullet).
- `status.md` / `phase-state.yaml` updated at landing; CHANGELOG `[Unreleased]` gains the
  wave's entries (post-0.2.0, additive).

## 10. Out of scope

- Track C proper (wasm-bindgen entry points, JS integration, browser demo) — next wave; this
  wave deliberately shrinks and de-duplicates the code path it will ship.
- `output.rs` internals (untested; do not touch), `web/` JS, any facade-surface change, any
  wire-schema change, OI-0015/0016 browser-side perf items, the deferred oversize row-window
  split (stays a documented deferral).
- The NUL column desync (filed, not fixed).

## 11. Dated note

*(2026-08-07 — appended, nothing above rewritten. Ticket `81b28a00`. **The two
review ids this spec attaches to §4 and §7 are swapped.** Every `R0001-` id
here belongs to the retired 2026-05-02 Review 0001, recoverable with `git show
bb93b68^:reviews/reviewed/0001.md` and indexed in `reviews/README.md`. That
round titles `R0001-0077` "Parser traversal has too many mutable cross-cutting
parameters" and `R0001-0078` "Unit construction mixes payload extraction,
profile rendering, batching, and structural inspection". So §4 (`unit.rs`
split) answers `R0001-0078`, §7 (parser walker rework) answers `R0001-0077`,
and §9's "OI-0008 → RESOLVED" list reads `R0001-0078` by §4 and `R0001-0077`
by §7. Only the labels are wrong: every section's scope, approach and
acceptance criteria stand as written, and the wave landed the work they
describe. `docs/project/open-issues.md` and `docs/project/status.md`, the
living records, carry the corrected pairing.)*
