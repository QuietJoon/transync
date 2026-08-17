# Implementation Slice Checklists

Each slice is a coherent body of implementation work that turns one or more `STUB` paths into real behavior for a specific scenario from `docs/architecture/scenario-matrix.md`. Slices are listed in dependency order, with risk notes, but execution order is not rigid — slices that share no STUB rows in `stub-manifest.md` may be implemented in parallel (and `phase-state.yaml.implementation.allowed_parallel_slices` will list them when they are).

*Path note (2026-08-04): every `SL-00`..`SL-14` slice below is **closed**, and
the `transync::…` module paths in their "Modules touched" and "Done when"
bullets are **as of MVP completion (2026-05-01)** — they are the acceptance
criteria as they were verified, and are deliberately not rewritten. Two
migrations have happened since: DCR-0017 moved `parser` (+`ranges`/`refdefs`),
`id`, `regen`, `align`, `render` (+`attrs`), and `htmlseg` into
`crates/transync-syntax`, and DCR-0018 made the engine modules unreachable
through the `transync` facade (`transync::parser::parse` is now
`transync_syntax::parser::parse`; `transync::pipeline::run_pipeline` is now
`transync::translate_with_cache`). `docs/implementation/module-map.md` carries
the live layout and `contracts.md` §0 the curated surface.*

## Slice numbering

`SL-NN` where `NN` corresponds to the governing `SCN-NN`. SL-12 spans CLI + provider; SL-13 spans the JS demo; SL-14 is mostly leverage from earlier slices. SL-00 is foundation work shared across all later slices.

---

## SL-00 — Foundation (no governing SCN; prerequisite)

**Governing scenario:** none — this slice establishes the substrate.
**Modules touched:** `transync::{parser, id, unit, regen}` (real, not STUB), `transync::error`, `tests/common/mock_translator.rs`, `tests/common/fixture_gen.rs`.
**Contracts touched:** none cross-process — IR shapes only.
**Persistence:** none.
**Files:** generates `scn-03-table-large.md` and `scn-10-long-document.md` from the `fixture_gen` module.
**Integrations:** none.
**Negative paths mandatory?** Yes — `parse` must reject MDX-shaped input cleanly; `assign_block_ids` must assign deterministic IDs across two parses of the same source.
**Done when:**
- `transync::parser::parse` returns a populated `Document` with non-empty `blocks` and a usable `ast` for any of the 12 fixtures.
- `transync::id::assign_block_ids` produces stable kind-prefixed IDs (`h2-0001`, `p-0002`, …); a fresh parse of the same source yields the same ID sequence.
- `transync::id::source_hash_block` returns a non-zero `u64` derived from the canonical block bytes.
- `transync::regen::regenerate` returns the source unchanged when given a no-op `validated` slice.
- `MockTranslator` exposes `passthrough`, `rejects_then_accepts`, `always_fails_unit`, `recording`, and `oversize_simulator` modes used by SL-01..SL-11.
- `tests/common/fixture_gen.rs` generates `scn-03-table-large.md` and `scn-10-long-document.md` from a `cargo test --features regen-fixtures` invocation; outputs are byte-stable across runs.

---

## SL-01 — Headings + paragraphs

**Governing scenario:** SCN-01.
**Modules touched:** `transync::{unit, batch, validate, regen, render, pipeline}`; uses `MockTranslator::passthrough`.
**Contracts touched:** `Translator` trait input/output shape (single-batch, single-unit-each path).
**Persistence:** none.
**Files:** `tests/fixtures/scn-01-headings-and-paragraphs.md` is the input.
**Integrations:** none.
**Negative paths mandatory?** No — happy path only; SL-07 covers retries, SL-08 covers fallback.
**Done when:**
- `transync::translate(source, opts, MockTranslator::passthrough)` returns `Ok(TranslationOutput)` for SCN-01's fixture.
- `output.translated_document` parses to the same kind sequence as the source IR (heading + 3 paragraphs).
- `output.alignment_map.blocks` has exactly 4 entries with `fallback_status="translated"`.
- `tests/scenarios/scn_01_headings_and_paragraphs.rs` asserts (a) block count = 4, (b) first block has `kind=heading-1`, (c) reparse succeeds.

---

## SL-02 — GFM table whole-block

**Governing scenario:** SCN-02.
**Modules touched:** `transync::{unit (FullTableMarkdown InputMode), validate::per_kind, regen}`.
**Contracts touched:** `BlockConstraints.must_preserve_table_columns`, `must_preserve_table_alignment`.
**Persistence:** none.
**Files:** `tests/fixtures/scn-02-table-small.md`.
**Integrations:** none.
**Negative paths mandatory?** Yes — a Mock that returns 3 columns must be rejected by per-kind validation.
**Done when:**
- A 4-column input round-trips with 4 columns and the same alignment row.
- Per-kind validator returns a `Schema`/`PerKindShape` rejection on column-count mismatch.
- `tests/scenarios/scn_02_table_small.rs` asserts column count and alignment preservation against the reparsed translated MD.

---

## SL-03 — Oversized-table row-window fallback

**Governing scenario:** SCN-03.
**Modules touched:** `transync::{batch, regen, unit::split, pipeline::merge}` — `regen` carries both the whole-block table round-trip and the `split_table_rows` / `regenerate_table` pair. *(Updated 2026-08-09, DCR-0026: row-window splitting — the `unit` `TableRowWindow` InputMode and the per-window `regen` reassembly — SHIPPED with SL-100..SL-105; STUB-017 closed with it.)*
**Contracts touched:** whole-block table unit via `InputMode::FullTableMarkdown`, and `InputMode::TableRowWindow { parent_block_id, window_index, window_count }` for the split path. *(Updated 2026-08-09, DCR-0026: the row-window contract is shipped, not deferred — its fields are in-process bookkeeping for reassembly and are never serialized into a prompt, see `contracts.md` §1. STUB-029's consumer-side oversize-split hook is unrelated and was deleted by DCR-0018.)*
**Persistence:** none.
**Files:** `tests/fixtures/scn-03-table-large.md` (200 rows).
**Integrations:** none — the slice uses `MockTranslator::{recording,always_fails_unit}`. *(Updated 2026-08-09, DCR-0026: the row-window path is no longer deferred, and the scenario drives it with a real output ceiling instead of simulating one.)*
**Negative paths mandatory?** Yes since DCR-0026: a window that exhausts its retries must contribute its own source rows and mark the block `partially_translated` (OQ-1(b)), not cost the table. A whole-block validation failure still falls back the entire table block through the SL-08 path.
**Done when:**
- Under a ceiling the table cannot fit, it is dispatched as **two or more** `table_row_window` units — each a complete GFM table carrying the real header — and `pipeline::merge` reassembles them into one table before regen (DCR-0026, SL-100..SL-105). A table that fits the ceiling, or a run under `--table-strategy whole-block`, still round-trips through the whole-block path as before.
- All 200 rows appear in the regenerated table in source order.
- The split is **packing, not retrying** (ADR-0009 untouched): it happens once, before round one, and `ValidationReport.total_retries` counts each window's own validation retries and nothing else. There is no reactive re-split on any provider signal — rejected, not deferred.
- `tests/scenarios/scn_03_table_large.rs` asserts what was **dispatched** (>= 2 windows, each a four-column table starting with the source header) and what came back (column count == 4, row count == 200, in source order; one alignment row for the table, marked `translated`, and no window id anywhere in the map).

---

## SL-04 — Code-block fence regeneration

**Governing scenario:** SCN-04.
**Modules touched:** `transync::{regen (safe fence sizing), validate::per_kind (info-string preservation)}`.
**Contracts touched:** `BlockKind::CodeBlock { info }`, `BlockConstraints.must_preserve_code_fence_info`.
**Persistence:** none.
**Files:** `tests/fixtures/scn-04-code-block.md`.
**Integrations:** none.
**Negative paths mandatory?** Yes — body containing 4 backticks must trigger ≥5-backtick fence.
**Done when:**
- Output fence is at least one backtick longer than the longest run of backticks in the body.
- Info string (`rust`) is preserved byte-for-byte.
- Translated comments are accepted; code identifiers are preserved unless explicitly altered by the model.
- `tests/scenarios/scn_04_code_block.rs` asserts fence length and info preservation.

---

## SL-05 — Nested list with task items

**Governing scenario:** SCN-05.
**Modules touched:** `transync::{validate::per_kind (list topology), regen (list reserialization)}`.
**Contracts touched:** `BlockKind::ListItem { ordered, task }`, `BlockConstraints.must_preserve_list_topology`.
**Persistence:** none.
**Files:** `tests/fixtures/scn-05-nested-list.md`.
**Integrations:** none.
**Negative paths mandatory?** Yes — collapsing depth-3 to depth-2 must be rejected by `per_kind`.
**Done when:**
- Source depth-3 structure survives in the translated MD (reparse yields the same `(depth, ordered, task)` tuple sequence).
- Checked/unchecked task markers preserved per item.
- Ordered/unordered marker types preserved.
- `tests/scenarios/scn_05_nested_list.rs` asserts the topology tuple sequence.

---

## SL-06 — Blockquote container

**Governing scenario:** SCN-06.
**Modules touched:** `transync::{validate::per_kind, regen}`.
**Contracts touched:** none new — blockquote children re-use existing per-kind paths.
**Persistence:** none.
**Files:** `tests/fixtures/scn-06-blockquote.md`.
**Integrations:** none.
**Negative paths mandatory?** No — happy path; the children are validated by their own per-kind validators.
**Done when:**
- Translated blockquote contains the same number of nested children in the same order with the same kinds.
- `tests/scenarios/scn_06_blockquote.rs` asserts container shape.

---

## SL-07 — Validation retry → success

**Governing scenario:** SCN-07.
**Modules touched:** `transync::{validate, pipeline::retry}`; uses `MockTranslator::rejects_then_accepts`.
**Contracts touched:** `TranslateOptions.max_per_unit_validation_retries`.
**Persistence:** none.
**Files:** `tests/fixtures/scn-07-validation-retry.md`.
**Integrations:** none.
**Negative paths mandatory?** Yes — this slice *is* the negative-path slice for SL-02.
**Done when:**
- The mock is invoked twice; the retried unit is resubmitted **verbatim** as a single-unit batch — the prior rejection reason is deliberately **not** included, because injecting it into `source_payload` makes a faithful provider echo it back and then fails full-document reparse (ADR-0009). *(Historical intent was a stricter prompt scope carrying the prior rejection reason; that was revised per ADR-0009 — the reason-bearing retry-hint side channel is a deferred feature.)*
- Final output is the corrected (3-column) table.
- `ValidationReport.per_unit[<unit>].attempts` has exactly 2 entries; first has a non-`None` `rejected_by`.
- `tests/scenarios/scn_07_validation_retry.rs` asserts attempt count and final acceptance.

---

## SL-08 — Persistent failure → fallback to source

**Governing scenario:** SCN-08.
**Modules touched:** `transync::{pipeline::retry, align (fallback_status), render (data-fallback)}`; uses `MockTranslator::always_fails_unit("p-0003")`.
**Contracts touched:** `AlignmentMap.blocks[*].fallback_status="fallback_source"`; `data-fallback="fallback_source"` HTML attribute.
**Persistence:** none.
**Files:** `tests/fixtures/scn-08-fallback.md`.
**Integrations:** none.
**Negative paths mandatory?** Yes — this slice *is* the persistent-failure slice.
**Done when:**
- After `max_per_unit_validation_retries` exhausts, the failed unit's `fallback_status` is `fallback_source`.
- The remainder of the document translates normally.
- The rendered target HTML for that block carries `data-fallback="fallback_source"`.
- The sync anchor (`data-sync-id`) is still present on the fallback block so JS sync still finds it.
- `tests/scenarios/scn_08_fallback.rs` asserts all four of the above.

---

## SL-09 — Prompt-injection treated as data

**Governing scenario:** SCN-09.
**Modules touched:** `transync::{profile (system-prompt rendering), unit (BlockContext), validate::schema}`; uses `MockTranslator::recording` to capture the rendered system prompt and source payload.
**Contracts touched:** Profile TOML `[system].prompt`; the data-not-instructions clause must remain at the head of the rendered prompt.
**Persistence:** none.
**Files:** `tests/fixtures/scn-09-prompt-injection.md`.
**Integrations:** recording stub provider only.
**Negative paths mandatory?** Yes — this is the security slice.
**Done when:**
- The rendered system prompt contains the verbatim "data, not instructions" clause from the default profile.
- The recorded `source_payload` for the offending paragraph contains the literal injection string (it is data; it is not pre-stripped).
- The translated output does not contain `PWNED`.
- Schema validation passes on the structured-output path even when the source payload contains imperative phrasing.
- `tests/scenarios/scn_09_prompt_injection.rs` asserts all four.

---

## SL-10 — Long document → many batches; partial-resume

**Governing scenario:** SCN-10.
**Modules touched:** `transync::{batch (token-budget grouping), pipeline (resume from last-completed-batch)}`.
**Contracts touched:** `[batching].target_output_tokens`, `[batching].max_units_per_batch`.
**Persistence:** none — resume state is in-memory across the test (single process; cache holds completed unit results).
**Files:** `tests/fixtures/scn-10-long-document.md`.
**Integrations:** none — `MockTranslator::passthrough` plus a forced `panic!` injected into the test's wrapping translator at batch index N/2.
**Negative paths mandatory?** Yes — partial-resume *is* the negative path here.
**Done when:**
- The fixture splits into ≥ 4 batches under `target_output_tokens=8000` for the default profile.
- A run that fails at batch N/2 and is restarted (with the same in-memory cache) produces byte-identical final output to a fresh successful run, excepting `detected_source_language`: that field is populated only on runs that dispatched >=1 batch, so a fully-cached restart reports `null` (pinned by `partial_resume_detected_language_is_dispatch_only`).
- `tests/scenarios/scn_10_long_document.rs` asserts batch count and resume equivalence.

*Note (2026-08-09, DCR-0028): the `detected_source_language` exception in the
second bullet is retired.* It was narrowed (R0007-0005) because the per-unit
cache stored no document-level record, so a fully-cached restart had no
envelope to report a detection from. The `Cache` trait now carries one — written
from the live run's qualifying envelope, replayed only by a run that dispatched
zero provider batches — so a fully-cached restart reports the same detection and
the byte-identical guarantee covers the whole alignment map. The pinning test
was rewritten and renamed to `partial_resume_replays_the_detected_language`.
This slice stays closed; the note records what moved under it.

---

## SL-11 — `source-language=auto` echoed

**Governing scenario:** SCN-11.
**Modules touched:** `transync::{align (detected_source_language), pipeline}`; uses `MockTranslator::passthrough` configured to set `detected_source_language="fr"` on the result.
**Contracts touched:** `TranslationBatchResult.detected_source_language`, `AlignmentMap.detected_source_language`.
**Persistence:** none.
**Files:** `tests/fixtures/scn-11-language-auto.md` (French source).
**Integrations:** none.
**Negative paths mandatory?** No.
**Done when:**
- `TranslateOptions { source_language: "auto", ... }` propagates as `"auto"` into the batch.
- `AlignmentMap.detected_source_language == "fr"` after the run.
- `TranslationOutput.detected_source_language == "fr"` is also surfaced to the caller.
- `tests/scenarios/scn_11_language_auto.rs` asserts both surfaces.

---

## SL-12 — CLI end-to-end

**Governing scenario:** SCN-12.
**Modules touched:** `transync-cli::{main, translate_cmd, output, error}`; `transync-openai::*` (real, with `OPENAI_API_KEY`) **or** the in-process `MockTranslator` under the `test-stub-provider` Cargo feature.
**Contracts touched:** CLI argument contract (§6 of `contracts.md`); atomic-write contract (§Persistence atomic-write); annotated HTML attribute set; ADR-0006 directory layout.
**Persistence:** the atomic outputs to `--output`, `--map`, and the six files inside `--html-out` (all committed in one staged fileset commit).
**Files:** input `tests/fixtures/scn-14-full.md`; outputs in a tmp dir.
**Integrations:** OpenAI Responses API in the live job; `MockTranslator` in CI default and in `scripts/smoke.sh`.
**Negative paths mandatory?** Yes — exit codes 1..4 must each have a covering test:
  - 1: missing `--input`
  - 2: malformed source
  - 3: every unit fell back (handled with `MockTranslator::always_fails_all`)
  - 4: write failure (simulated by passing a non-writable `--output` parent dir)
**Done when:**
- `cargo run -p transync-cli --features test-stub-provider -- translate --input ... --output ... --map ... --html-out ... --target-language ko` exits 0.
- All four output paths exist and are non-empty; each output has the correct atomic-write contract (`.tmp.<pid>` is gone; `rename` succeeded).
- `out.md` reparses; `out.json` validates against the AlignmentMap schema (`schema_version: "1.0.0"`).
- `out/source.html` and `out/target.html` are fragment-shaped (no `<html>` wrapper) and contain `data-sync-id` on every sync-relevant block.
- All five exit-code paths have integration tests in `crates/transync-cli/tests/cli_smoke.rs`.

---

## SL-13 — JS demo synchronizes panes

**Governing scenario:** SCN-13.
**Modules touched:** `web/js/sync.js`, `transync-cli/web/index.html.tpl` (the demo shell).
**Contracts touched:** Annotated HTML attribute contract (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`); AlignmentMap JSON `schema_version: "1.0.0"`.
**Persistence:** none.
**Files:** consumes the SL-12 output bundle from `transync serve --rendered <dir>`. *(Correction 2026-08-08: that prerequisite was never met, not even historically. `transync serve` was already DEFERRED — STUB-061 — when this slice closed, and it still binds no socket; SL-13 was in fact verified against an external static server, `python3 -m http.server` per `web/SMOKE.md`, as `scripts/test-browser.sh` still is. The row records the planned prerequisite rather than the one acceptance actually used. The real loopback server is deferred, not cancelled — post-0.3.0 roadmap ticket `b791d6`; `contracts.md` §6 carries the live contract.)* *(Follow-up 2026-08-09: `b791d6` landed. `transync serve` binds a socket now, so the prerequisite this row records is met at last — and `scripts/test-browser.sh` serves SCN-13 with it rather than with an external server, which makes the acceptance actually used and the one planned the same thing for the first time. The 2026-08-08 correction above still describes the slice's own history correctly and stands.)*
**Integrations:** browser (manual smoke); no headless automation in MVP.
**Negative paths mandatory?** Manual checks only:
  - Programmatic-scroll lock prevents oscillation when both panes are scrolled near-simultaneously.
  - Resize preserves correspondence (re-runs `getBoundingClientRect` reads, does not re-mount).
**Done when:**
- The demo shell mounts both fragments into named slots and loads `alignment.json` once.
- Scrolling either pane drives the other to the matching `data-sync-id` block within one animation frame.
- A 50 px continuous scroll over 1 s does not produce more than one sync transition per ~6 frames (hysteresis works).
- `web/js/sync.js` is < 500 LOC, framework-free, ESM-only.
- Manual smoke checklist (in `web/SMOKE.md`, written in this slice) is checked off.

---

## SL-14 — Full-document reparse

**Governing scenario:** SCN-14.
**Modules touched:** `transync::{regen, validate::full_reparse}`.
**Contracts touched:** none new — wraps existing surfaces.
**Persistence:** none.
**Files:** any fixture that has produced a `TranslationOutput`; `tests/fixtures/scn-14-full.md` is the canonical input.
**Integrations:** none.
**Negative paths mandatory?** Yes — a deliberately mangled regenerated MD (e.g. mismatched code fence) must fail `full_reparse`.
**Done when:**
- Re-parsing `output.translated_document` via Comrak yields a block-kind sequence equal to the source IR's sequence.
- The reparsed block count equals the source block count.
- `validate::full_reparse` is exercised once at the end of `pipeline::run_pipeline` and rejects malformed regenerations.
- `tests/scenarios/scn_14_full.rs` asserts both equalities and the negative-path rejection.

---

## Slice dependency graph

```
SL-00 ──┬── SL-01 ──┬── SL-07 ─── SL-08
        │           │
        ├── SL-02 ──┤
        ├── SL-03 ──┤
        ├── SL-04 ──┤
        ├── SL-05 ──┤
        ├── SL-06 ──┤
        ├── SL-09 ──┤
        ├── SL-10 ──┤
        ├── SL-11 ──┤
        └── SL-14 ──┘

SL-01..SL-06 + SL-14 ──── SL-12 ──── SL-13
                              ↑
                        SL-07, SL-08 (so the CLI can surface fallbacks)
```

- SL-00 must complete before any other slice begins.
- SL-01..SL-06, SL-09, SL-10, SL-11, SL-14 are mutually independent after SL-00 — `phase-state.yaml.implementation.allowed_parallel_slices` should list them as parallel-safe once SL-00 closes.
- SL-07 depends on SL-02 (the rejection target is a column-count mismatch).
- SL-08 depends on SL-07 (shares the retry state machine).
- SL-12 depends on SL-01..SL-08 + SL-14 because the CLI exercises the full pipeline.
- SL-13 depends on SL-12 because it consumes the CLI output bundle.

## What is *not* a slice

These are out of scope for MVP and have no slice number:

- WASM track-C renderer.
- `transync-anthropic`, `transync-local-llama` provider crates.
- Disk-backed cache.
- Streaming translation.
- Sentence-level sub-anchors.
- Live-edit re-anchoring.
- Headless-browser SCN-13 automation.

If any of these become in-scope post-MVP they will get a new slice number outside the SL-01..SL-14 range (`SL-100`+) and a Design Change Record.
