# Stub Manifest

**Version:** historical record (Phase 4 baseline, 2026-05-01); MVP closed 2026-05-03.

Phase 4 generated the STUB rows below — every "Source path" file existed at that time with a marker comment and the dummy behavior described. Implementation slices (`SL-00` … `SL-14`) closed each row in order, dropping the marker once the function carried real behavior. **Every STUB row is now closed**; the rows survive in this document as historical traceability between MVP scenarios, slices, and the code that retired the markers.

A separate set of rows lower in this file track the placeholders that were once **DEFERRED** — intentionally kept out of MVP scope. **None of them is still placeholder-shaped.** All 13 rows carry a dated Closed or Removed note, the last two on 2026-08-09 (STUB-017 and STUB-061), and a workspace grep for `STUB` markers over `crates/` and `web/js/` returns only historical citations in prose comments — never a marker on placeholder-shaped code. Those rows survive here as traceability, exactly like the STUB tables above.

## Marker types

- **`STUB`** — was an in-scope MVP placeholder; **all closed** as of MVP completion. The "STUB rows" tables below are historical.
- **`DEFERRED`** — was an out-of-scope placeholder; **all 13 rows are now closed or removed**, the last two on 2026-08-09 (the row-window splitter, STUB-017, and the real `transync serve`, STUB-061). See the DEFERRED section near the bottom of this file; a closed row keeps its place there with a dated note rather than being deleted, so that section is historical too.

## Conventions

- Marker IDs are stable for the project's lifetime: a row's ID is the cite used in code comments (`// STUB: STUB-014`) and in PR commit messages.
- "Dummy behavior" is the literal behavior Phase 4 wrote; it was enough to compile and to make the smoke test pass with placeholder values.
- "Closes by" is the implementation slice that deleted the marker. Closed rows are listed under the historical "STUB rows" tables; deferred-into-post-MVP work is listed under the DEFERRED section instead.
- Every row in the STUB tables was originally `MVP` scope; nothing in those tables is open.
- **Source paths are as of each row's date and are deliberately not rewritten** — the whole value of this file is the historical trace. Two path migrations have happened since: `crates/transync/src` → `crates/transync-core/src` on 2026-07-10 (DCR-0005), and `crates/transync-core/src` → `crates/transync-syntax/src` on 2026-08-04 for `parser` (+`ranges`/`refdefs`), `id`, `regen`, `align`, `render` (+`attrs`), and `htmlseg` (DCR-0017). Separately, the `transync::…` prefixes in the "Module" column stopped being *facade* paths on 2026-08-04 (DCR-0018): the facade re-exports an explicitly curated list and the engine modules named here are reachable only through `transync-core` / `transync-syntax` directly. `docs/implementation/module-map.md` carries the live layout and `contracts.md` §0 the curated surface.

## STUB rows — `transync` core (historical, all closed)

| ID       | Source                                            | Module                | Closes by | Contract / interface              | Dummy behavior                                                          | Exit condition |
|----------|---------------------------------------------------|-----------------------|-----------|-----------------------------------|-------------------------------------------------------------------------|----------------|
| STUB-001 | `crates/transync-core/src/parser.rs::parse`            | `transync::parser`    | SL-00     | `Document` IR (rough-schema §3)   | Returns `Document { source_text, blocks: vec![], ast: empty, hierarchy: vec![] }` | Real GFM parse populating `blocks` |
| STUB-002 | `crates/transync-core/src/parser/ranges.rs::byte_range_for` | `transync::parser::ranges` | SL-00 | `ByteRange`                       | Returns `ByteRange { start: 0, end: 0 }`                                | Returns AST node's source range |
| STUB-003 | `crates/transync-core/src/id.rs::assign_block_ids`     | `transync::id`        | SL-00     | `BlockId` format (ADR-0005)       | No-op (every block keeps `id = ""` placeholder)                          | Walks AST, assigns kind-prefixed sequential IDs |
| STUB-004 | `crates/transync-core/src/id.rs::source_hash_block`    | `transync::id`        | SL-00     | `source_hash: u64`                | Returns `0_u64`                                                          | SipHash-1-3 over canonical block bytes |
| STUB-005 | `crates/transync-core/src/unit.rs::build_batches`      | `transync::unit`      | SL-01     | `TranslationBatch` (rough-schema §8) | Returns `vec![]`                                                       | Constructs batches with units, contexts, constraints |
| STUB-006 | `crates/transync-core/src/unit/context.rs::build_context` | `transync::unit::context` | SL-01 | `BlockContext` (rough-schema §6)  | Returns `BlockContext::default()`                                        | Populates section path + neighbor snippets |
| STUB-007 | `crates/transync-core/src/batch.rs::group_by_budget`   | `transync::batch`     | SL-10     | `[batching].max_units_per_batch`   | Returns input chunked at 8 units (no token awareness)                   | Honors `target_output_tokens` heuristic |
| STUB-008 | `crates/transync-core/src/validate.rs::validate_batch` | `transync::validate`  | SL-01     | `ValidatedBatch`                   | Accepts every unit with `final_status=Translated`                        | Runs the six-layer validator and records rejections |
| STUB-009 | `crates/transync-core/src/validate/schema.rs::check_schema` | `transync::validate::schema` | SL-01 | `ValidationLayer::Schema`     | Returns `Ok(())`                                                         | Enforces ID set equality + no duplicates |
| STUB-010 | `crates/transync-core/src/validate/per_kind.rs::check_table` | `transync::validate::per_kind` | SL-02 | `BlockConstraints.must_preserve_table_columns` | Returns `Ok(())`                              | Column count + alignment match |
| STUB-011 | `crates/transync-core/src/validate/per_kind.rs::check_list` | `transync::validate::per_kind` | SL-05 | `BlockConstraints.must_preserve_list_topology` | Returns `Ok(())`                              | Depth + ordered/task tuple sequence equal |
| STUB-012 | `crates/transync-core/src/validate/per_kind.rs::check_code` | `transync::validate::per_kind` | SL-04 | `BlockConstraints.must_preserve_code_fence_info` | Returns `Ok(())`                            | Info string preserved; fence sized safely |
| STUB-013 | `crates/transync-core/src/validate/per_kind.rs::check_heading` | `transync::validate::per_kind` | SL-01 | `BlockConstraints.must_preserve_heading_level` | Returns `Ok(())`                              | Heading level == source |
| STUB-014 | `crates/transync-core/src/validate/fragment_reparse.rs::reparse_fragment` | `transync::validate::fragment_reparse` | SL-02 | `ValidationLayer::FragmentReparse` | Returns `Ok(())` | Reparses translated payload as the same kind |
| STUB-015 | `crates/transync-core/src/validate/full_reparse.rs::reparse_full` | `transync::validate::full_reparse` | SL-14 | `ValidationLayer::FullReparse` | Returns `Ok(())` | Reparses regenerated MD; counts + order match source |
| STUB-016 | `crates/transync-core/src/regen.rs::regenerate`        | `transync::regen`     | SL-01     | `(String, BlockOffsets)`           | Returns `(source_text.clone(), BlockOffsets::default())`                 | AST splice + safe fence regen + table reserialization |
| STUB-017 | `crates/transync-core/src/regen.rs::regenerate_table`  | `transync::regen`     | SL-02, SL-100 | GFM table reassembly           | Returns input unchanged                                                  | Reassembles from translated whole-block or row windows (both shipped — DCR-0026, 2026-08-09) |
| STUB-018 | `crates/transync-core/src/regen.rs::regenerate_code_fence` | `transync::regen` | SL-04     | Safe fence sizing                   | Returns input unchanged                                                  | Picks fence ≥ longest backtick run + 1 |
| STUB-019 | `crates/transync-core/src/align.rs::build_alignment_map` | `transync::align`   | SL-01     | `AlignmentMap` (`schema_version 1.0.0`, contracts §3) | Returns `AlignmentMap { schema_version: "1.0.0", blocks: vec![], ... }` | Populates blocks with source/target ranges + sync_role |
| STUB-020 | `crates/transync-core/src/render.rs::render_source`    | `transync::render`    | SL-01     | Annotated HTML (contracts §4, ADR-0006) | Returns `"<main></main>"`                                            | Emits annotated block-tree fragment for source |
| STUB-021 | `crates/transync-core/src/render.rs::render_target`    | `transync::render`    | SL-01     | Annotated HTML (contracts §4, ADR-0006) | Returns `"<main></main>"`                                            | Emits annotated block-tree fragment for target |
| STUB-022 | `crates/transync-core/src/render/attrs.rs::write_attrs` | `transync::render::attrs` | SL-01 | `data-sync-id` etc. (contracts §4) | Writes attribute names with empty values                                | Real values from `Block` + `AlignmentBlock` |
| STUB-023 | `crates/transync-core/src/cache.rs::InMemoryCache::get` | `transync::cache`    | SL-10     | `Cache` trait                      | Returns `None`                                                           | Hash lookup against composite key |
| STUB-024 | `crates/transync-core/src/cache.rs::InMemoryCache::put` | `transync::cache`    | SL-10     | `Cache` trait                      | No-op                                                                    | Stores `UnitResult` keyed by `CacheKey` |
| STUB-025 | `crates/transync-core/src/profile.rs::load_profile`    | `transync::profile`   | SL-09     | Profile TOML (contracts §2)        | Returns `default_profile()`                                              | Parses TOML, validates required fields |
| STUB-026 | `crates/transync-core/src/profile.rs::default_profile` | `transync::profile`   | SL-09     | Embedded default profile           | Returns `ProfileMetadata { slug: "default", version: "1.0.0", prompt_body: "" }` | Parses `crates/transync-cli/profiles/default.toml` via `include_str!` |
| STUB-027 | `crates/transync-core/src/pipeline.rs::run_pipeline`   | `transync::pipeline`  | SL-01     | Top-level orchestrator              | Calls each STUB sub-step in order; returns `TranslationOutput` with empty alignment map and translated MD = source | Real pipeline composing parser→units→batch→translate→validate→regen→align→render |
| STUB-028 | `crates/transync-core/src/pipeline/retry.rs::retry_validation` | `transync::pipeline::retry` | SL-07 | `max_per_unit_validation_retries` | First attempt accepted unconditionally                                  | Resubmits the failed unit verbatim (same prompt, same scope) — ADR-0009 |
| STUB-029 | `crates/transync-core/src/pipeline/retry.rs::oversize_split` | `transync::pipeline::retry` | SL-03 | `max_oversize_split_retries_per_batch` | No-op                                                                  | Halves and resubmits on oversize signal |
| STUB-030 | `crates/transync-core/src/pipeline/retry.rs::provider_retry` | `transync::pipeline::retry` | SL-12 | `max_per_batch_provider_retries` | No-op                                                                  | Bounded retry on `Network` / `RateLimited` |
| STUB-031 | `crates/transync-core/src/pipeline/retry.rs::fallback_to_source` | `transync::pipeline::retry` | SL-08 | `fallback_status: fallback_source` | No-op                                                              | Marks unit as fallback_source; alignment map records it |

## STUB rows — `transync-openai`

| ID       | Source                                                 | Module                           | Closes by | Contract / interface         | Dummy behavior                                                | Exit condition |
|----------|--------------------------------------------------------|----------------------------------|-----------|------------------------------|---------------------------------------------------------------|----------------|
| STUB-040 | `crates/transync-openai/src/lib.rs::TransyncOpenAI::new` | `transync_openai`              | SL-12     | constructor (contracts §7)   | Stores fields; no validation                                  | `try_new` validates `api_key` non-empty; `new` left unchecked |
| STUB-041 | `crates/transync-openai/src/lib.rs::TransyncOpenAI::from_env` | `transync_openai`         | SL-12     | env resolution (contracts §7) | Reads `OPENAI_API_KEY` only; falls back hard-coded values     | Honors `TRANSYNC_OPENAI_MODEL` and `TRANSYNC_OPENAI_BASE_URL` per spec |
| STUB-042 | `crates/transync-openai/src/lib.rs::<Translator for TransyncOpenAI>::translate_batch` | `transync_openai` | SL-12 | `Translator` trait (contracts §1) | Echoes payloads back as `OutputKind::Translated` units (no real API call) | Real Responses API call with Structured Outputs |
| STUB-043 | `crates/transync-openai/src/client.rs::call_responses_api` | `transync_openai::client`    | SL-12     | OpenAI Responses API          | Returns synthesized echo result                                | Real `async-openai` (or `reqwest`) call |
| STUB-044 | `crates/transync-openai/src/tokens.rs::estimate`       | `transync_openai::tokens`        | SL-10     | `tiktoken-rs`                 | Returns `0`                                                    | Real token count for the configured model |
| STUB-045 | `crates/transync-openai/src/pagination.rs::split_oversize` | `transync_openai::pagination` | SL-03 | oversize halving               | No-op (returns input unchanged)                                | Halves units and re-yields when over budget |
| STUB-046 | `crates/transync-openai/src/error.rs::map_provider_error` | `transync_openai::error`     | SL-12     | `TranslatorError` mapping     | Returns `TranslatorError::Other("STUB")`                        | Maps `async-openai` errors to the right `TranslatorError` variant |

## STUB rows — `transync-cli`

| ID       | Source                                              | Module                       | Closes by | Contract / interface           | Dummy behavior                                                       | Exit condition |
|----------|-----------------------------------------------------|------------------------------|-----------|--------------------------------|----------------------------------------------------------------------|----------------|
| STUB-060 | `crates/transync-cli/src/translate_cmd.rs::run`     | `transync_cli::translate_cmd` | SL-12    | CLI args (contracts §6) + ADR-0006 | Reads `--input`; calls `transync::translate` with the test stub provider; writes 4 output files via real atomic-write helper | Wires real provider when `--no-stub`; surfaces exit codes 0..5 |
| STUB-061 | `crates/transync-cli/src/serve_cmd.rs::run`         | `transync_cli::serve_cmd`    | SL-13     | demo HTTP                       | Binds `127.0.0.1:7470`; serves the requested directory with no MIME refinement | Adds correct content-types, range support, and graceful shutdown |
| STUB-062 | `crates/transync-cli/src/output.rs::write_html_bundle` | `transync_cli::output`     | SL-12     | ADR-0006 directory layout       | Writes the 5 files (each via the real atomic-write helper) but `index.html` body is the bare template with no substitutions | Substitutes `{{TRANSYNC_TITLE}}` and `{{ALIGNMENT_MAP_REL}}` in the embedded template |
| STUB-063 | `crates/transync-cli/web/index.html.tpl`            | (asset)                       | SL-12     | demo shell                      | Plain HTML with `<main id="source"></main><main id="target"></main><script src="sync.js"></script>` | Loads `alignment.json`, injects fragments, calls `mountSync(...)` |

The atomic-write primitive itself (`crates/transync-cli/src/output.rs::write_atomic`) is **not** a STUB — it is real in Phase 4 because every test bundle relies on it.

## STUB rows — JS demo

| ID       | Source                       | Module    | Closes by | Contract / interface              | Dummy behavior                                                  | Exit condition |
|----------|------------------------------|-----------|-----------|-----------------------------------|-----------------------------------------------------------------|----------------|
| STUB-080 | `web/js/sync.js::mountSync`  | (web)     | SL-13     | annotated HTML attrs (contracts §4) | Wires one `IntersectionObserver` per pane; logs active block to `console.debug`; does **not** scroll partner pane | Real partner-pane scroll via `scrollIntoView({ block: "nearest" })`, programmatic-scroll lock, hysteresis |
| STUB-081 | `web/js/sync.js::activeBlock` | (web)    | SL-13     | active-block heuristic            | Returns first `[data-sync-id]` whose `intersectionRatio > 0`     | Composite score: `intersectionRatio` + viewport-center distance + 6-frame hysteresis |
| STUB-082 | `web/js/sync.js::loadAlignment` | (web)  | SL-12     | AlignmentMap JSON                 | `fetch('alignment.json').then(r => r.json())` (no schema check)  | Validates `schema_version` major; rejects unknown majors |

## DEFERRED rows (Phase-3 seed-time state)

**None at seed time.** See `skeleton-plan.md` §10 — when this manifest was seeded, every post-MVP architectural extension was satisfied by *not generating that module* rather than by a placeholder. Phase-4 hardening later converted the remaining open rows to `DEFERRED`; the current set lives in "DEFERRED rows (post-MVP placeholders, retained for architectural symmetry)" below.

## Closed rows (implementation track)

The following rows were retired as their closing slice landed. The rows
remain in the table above for traceability; "Status" reflects the latest
state.

| ID       | Closed by      | Notes                                                                                         |
|----------|----------------|-----------------------------------------------------------------------------------------------|
| STUB-001 | SL-00 (2026-05-01) | `parser::parse` walks Comrak AST and emits real blocks with stable IDs                    |
| STUB-002 | SL-00 (2026-05-01) | `byte_range_for` resolves comrak `Sourcepos` → byte ranges via `LineOffsets`              |
| STUB-003 | SL-00 (2026-05-01) | `id::assign_block_ids` is now idempotent; parser pre-assigns IDs in source-order walk      |
| STUB-004 | SL-00 (2026-05-01) | `id::source_hash_block` returns SipHash-1-3 of canonical block bytes via `siphasher` crate |
| STUB-005 | SL-01 (2026-05-01) | `unit::build_batches` builds one TranslationUnit per sync-relevant block                  |
| STUB-006 | SL-01 (2026-05-01) | `unit::context::build_context` populates section path + neighbor snippets + doc title (SL-09 may refine glossary scoping) |
| STUB-008 | SL-01 (2026-05-01) | `validate::validate_batch` runs schema → per-kind dispatcher → fragment reparse           |
| STUB-009 | SL-01 (2026-05-01) | `validate::schema::check_schema` enforces unit-id set equality + no duplicates / extras    |
| STUB-013 | SL-01 (2026-05-01) | `validate::per_kind::check_heading` enforces level-equality on translated heading payload  |
| STUB-016 | SL-01 (2026-05-01) | `regen::regenerate` splices translated payloads into source at byte ranges (top-level blocks; SL-05/SL-06 will extend for nested) |
| STUB-019 | SL-01 (2026-05-01) | `align::build_alignment_map` populates blocks with ranges + fallback statuses + sync roles |
| STUB-020 | SL-01 (2026-05-01) | `render::render_source` emits annotated block-tree fragment via Comrak inline rendering    |
| STUB-021 | SL-01 (2026-05-01) | `render::render_target` mirrors `render_source` against the regenerated MD                 |
| STUB-022 | SL-01 (2026-05-01) | `render::attrs::write_attrs` writes the canonical attribute set with real values + parent-id |
| STUB-027 | SL-01 (2026-05-01) | `pipeline::run_pipeline` orchestrates parse → units → translate → validate → regen → align → render |
| STUB-010 | SL-02 (2026-05-01) | `validate::per_kind::check_table` reparses translated payload via Comrak; asserts column count + row count parity |
| STUB-014 | SL-02 (2026-05-01) | `validate::fragment_reparse::reparse_fragment` real comrak reparse with kind-equality check |
| STUB-012 | SL-04 (2026-05-01) | `validate::per_kind::check_code` extracts info string via Comrak and asserts byte-for-byte preservation |
| STUB-018 | SL-04 (2026-05-01) | `regen::regenerate_code_fence` picks fence ≥ longest backtick run + 1; regen detects code blocks and re-wraps |
| STUB-011 | SL-05 (2026-05-01) | `validate::per_kind::check_list` compares translated `(depth, ordered, task)` tuple sequence against source |
| STUB-025 | SL-09 (2026-05-01) | `profile::load_profile` parses TOML via the `toml` crate and validates required fields |
| STUB-026 | SL-09 (2026-05-01) | `profile::default_profile` includes the embedded `crates/transync-core/profiles/default.toml` and parses it |
| STUB-028 | SL-07 (2026-05-01) | `pipeline::retry::retry_validation_unit` resubmits the unit verbatim (ADR-0009 — no stricter-prompt escalation); the pipeline owns the bounded retry loop |
| STUB-031 | SL-08 (2026-05-01) | Fallback path lands inside `pipeline::run_pipeline` — `accepted` retains FallbackSource units; alignment map records it; render attaches `data-fallback="fallback_source"` |
| STUB-015 | SL-14 (2026-05-01) | `validate::full_reparse::reparse_full` reparses regenerated MD; pipeline runs it post-regen and surfaces failures via `tracing::warn` |
| STUB-007 | (no longer STUB) | `batch::group_by_budget` always honored `max_units_per_batch`; was never functionally STUB. Marker removed. The row's aspirational "Honors `target_output_tokens` heuristic" exit condition never described the batcher (whose per-batch budget is `TranslateOptions::target_input_tokens_per_batch`). `target_output_tokens` is now **shipped** — but as the provider *output* ceiling (`max_completion_tokens` / `max_output_tokens`) enforced in `transync-openai` per EXT P1-7 / OI-0019, not as a batching heuristic. See contracts §2/§7. |
| STUB-023 | SL-10 (2026-05-01) | `cache::InMemoryCache::get` returns `store.lock()?.get(key).cloned()` |
| STUB-024 | SL-10 (2026-05-01) | `cache::InMemoryCache::put` inserts into the locked HashMap |
| STUB-060 | SL-12 (2026-05-01) | `transync-cli::translate_cmd::run` surfaces exit codes 0, 1 (clap), 2 (read fail), 3 (all-fallback), 4 (write fail), 5 (other) |
| STUB-080 | SL-13 (2026-05-01) | `web/js/sync.js::mountSync` real partner-pane scroll + programmatic-scroll lock + RAF coalescing |
| STUB-081 | SL-13 (2026-05-01) | `activeBlock` returns the topmost element at-or-above a reference line, with last-active-id hysteresis |
| STUB-082 | SL-13 (2026-05-01) | `loadAlignment` validates `schema_version` major and rejects unknowns with a clear console warning |
| STUB-062 | ti 0f26b5 (2026-08-07) | The shell's slots carry resolved values, not placeholders: `{{TRANSYNC_TITLE}}` is `--title` > the source document's first H1 (as parser-plain text, the same value the provider read as `BlockContext.document_title`) > the `transync` literal, and `{{TRANSYNC_DOC_LANG}}` is the run's target language. Escaped, and filled in one pass so an untrusted heading cannot name another slot. `contracts.md` §6 "Bundle title and language" |
| STUB-063 | ti 0f26b5 (2026-08-07) | Pairs with STUB-062. The template it names has been a working shell since SL-13 (it fetches `alignment.json`, injects both fragments, mounts `sync.js`); its last placeholder-shaped slots are the two above |

## DEFERRED rows (post-MVP placeholders, retained for architectural symmetry)

Phase 4 hardening (2026-05-01) converted the remaining open rows to
explicit `DEFERRED` status. Each is justified below; none block the MVP
hard gate because the SCN-01..SCN-14 round-trip works without them under
the `test-stub-provider` Cargo feature.

| ID       | Source                                                         | Justification |
|----------|----------------------------------------------------------------|---------------|
| STUB-017 | `transync_syntax::regen::regenerate_table`                     | (**Closed 2026-08-09** — DCR-0026, ticket `fc0304`, slices SL-100..SL-105. `regenerate_table(&[&str])` reassembles a table from translated row windows and `split_table_rows` is its source-side inverse; `unit::split` produces the windows at packing time when `[constraints].default_table_strategy` is `row-window-first` — the shipped default — and a table's estimated response exceeds the output ceiling; `pipeline::merge` merges them back before regen. SCN-03 is live-verified: `scn_03_table_large.rs` asserts two or more windows were dispatched and that they reassemble into one 200x4 table. The original justification stands as history: row-window reassembly was post-MVP and the whole-block path in `regenerate` carried SCN-03's round-trip, with this function retained as the extension point — which is exactly where the splitter dropped in.) |
| STUB-029 | `transync::pipeline::retry::oversize_split`                    | (Removed 2026-08-04 — DCR-0018 **deleted** the `oversize_split` hook, along with `retry_validation` and `fallback_to_source`, when the OI-0027 module closure exposed all three as dead no-ops; a greppable tombstone comment preserves the rationale in `pipeline/retry.rs`. The original justification stands as history: provider-side oversize signal handling is post-MVP and no MockTranslator mode emits `Unsupported`/oversize, so the path was never reachable in CI. Consumer-side oversize handling, if it lands, will be designed with OI-0008's retry/fallback redesign rather than resurrected from an empty hook. STUB-045's provider-side splitter is unaffected.) |
| STUB-030 | `transync::pipeline::retry::provider_retry`                    | (Removed — R0006-0053 deleted the `provider_retry` hook. Transient HTTP / provider-error retries are implemented in the core pipeline's `translate_with_provider_retries` loop, which owns its own bounded budget + backoff; there is no separate deferred provider-retry hook.) |
| STUB-040 | `transync_openai::TransyncOpenAI::new`                         | (Closed — the live client is implemented. Note: `new` does NOT validate — only `try_new` fails fast on an empty `api_key`; `new` stores fields unchecked (an empty key surfaces as an HTTP 401 at call time). Remaining limitation: no live-mode integration tests — see OI-0009 [archived].) |
| STUB-042 | `transync_openai::TransyncOpenAI::translate_batch`             | (Closed — `translate_batch` performs the real API call with Structured Outputs; CI still exercises `EchoTranslator`. Remaining limitation: no live-mode integration tests — see OI-0009.) |
| STUB-043 | `transync_openai::client::call_responses_api`                  | (Closed — the real HTTP call is implemented via `reqwest`. Remaining limitation: no live-mode integration tests — see OI-0009.) |
| STUB-044 | `transync_openai::tokens::estimate` (deleted)                  | (Removed — the provider-side `tokens` module was deleted; token budgeting lives in `transync-core::batch` via `tiktoken-rs` (DCR-0005 / OI-0014 [archived]). No provider-side estimator remains.) |
| STUB-045 | `transync_openai::pagination::split_oversize`                  | (Removed 2026-08-05 — DCR-0019 **deleted** the whole `pagination` module, following STUB-029's consumer-side hook out the door for the same reason: an empty no-op with zero callers documents nothing. The deferral itself is unchanged and is now tracked where a deferral belongs — `contracts.md` §5 and this manifest — rather than in dead code. Provider-side oversize splitting, if it lands, will be designed alongside the retry/fallback policy module (`pipeline/policy.rs`) rather than resurrected from a stub.) |
| STUB-061 | `transync_cli::serve_cmd::run`                                 | (**Closed 2026-08-09** — ticket `b791d6`, the fourth commissioned post-0.3.0 roadmap item; no DCR, the owner un-deferred it on 2026-08-06. `serve` binds a socket and serves `--rendered` over HTTP: `GET`/`HEAD` only, loopback by default with a warning on any other bind, no directory listing, path confinement in two layers (per-segment percent-decode, then canonicalize-and-contain, which is the layer that sees a symlink escape), a fixed extension→content-type table with `application/octet-stream` for everything it does not name, `Connection: close` on every response, and a Ctrl-C/SIGTERM shutdown that drains and exits `0`. The exit condition this row set — "correct content-types, range support, and graceful shutdown" — is met on two of its three terms; **HTTP ranges were dropped by decision**, named out of scope in the ticket alongside TLS and caching sophistication, because nothing in a six-file bundle asks for them. `contracts.md` §6 carries the contract, `crates/transync-cli/tests/serve_static.rs` pins it, and `scripts/test-browser.sh` now serves SCN-13 with it. The original justification stands as history: a real static server was post-MVP and the SCN-13 smoke ran against `python3 -m http.server` per `web/SMOKE.md`.) |
| STUB-062 | `transync_cli::output::html_bundle_files` template substitution | (Closed 2026-08-07 — ti 0f26b5. Every slot now carries a value the run resolved. `{{TRANSYNC_DOC_LANG}}` is the target language — that half landed earlier, with the per-pane `lang` slots (R0008-0050); `{{TRANSYNC_TITLE}}` resolves `--title` > the source document's first H1 > the `transync` literal, the middle level being the same extraction the provider reads as `BlockContext.document_title`. The alignment map was NOT extended to carry it: the title rides pipeline state (`TranslationOutput::document_title`) to the bundle assembler, so no schema version moved. `contracts.md` §6 "Bundle title and language".) |
| STUB-063 | `transync_cli/web/index.html.tpl` placeholders                 | (Closed 2026-08-07 — ti 0f26b5. Paired with STUB-062: the template's remaining placeholder-shaped slots are the title and document language, both resolved above. The shell itself stopped being a stub at SL-13.) |
| STUB-041 | `transync_openai::TransyncOpenAI::from_env`                    | (Closed in Phase 4 hardening — now honors `TRANSYNC_OPENAI_MODEL` / `TRANSYNC_OPENAI_BASE_URL`.) |
| STUB-046 | `transync_openai::error::map_provider_error`                   | (Closed in Phase 4 hardening — maps every `ProviderError` variant to the right `TranslatorError`.) |

The MVP hard-gate-relevant code paths carry zero `STUB` markers — and since
2026-08-09 so does the rest of the workspace: **no placeholder remains**. Every
row above is closed or removed with a dated note. The only `STUB-` strings left
in `crates/` and `web/js/` are historical citations inside prose comments — the
deleted-hook tombstone in `pipeline/retry.rs`, the History note in
`serve_cmd.rs`, two comments in `regen.rs` recording what `regenerate_table`
used to do as STUB-017's placeholder, and the ticket reference at the top of
`crates/transync-cli/tests/serve_static.rs`.

## Status legend

- `planned` — listed here; will be generated in Phase 4
- `generated` — placeholder exists in code with the marker comment (post Phase 4 hard gate)
- `closed` — slice retired the marker; row recorded in "Closed rows" above

**All rows were `generated` as of Phase 4 completion (2026-05-01).** Phase-4 hard gate evidence:

```
$ cargo build --workspace                                            # clean (1 reserved-variant warning)
$ cargo test --workspace                                             # 12 scenario tests pass
$ cargo test -p transync-cli --features test-stub-provider           # cli_translate_smoke passes
$ ./scripts/smoke.sh                                                  # exit 0; 7 outputs landed
```

## Hard-gate evidence checklist

For Phase 4 to claim its hard gate, the manifest must:

- Have one row per `STUB` marker comment in the workspace.
- Have zero `STUB` markers in the workspace that are not in the manifest.
- Have every "Source" path resolve to a real file.
- Have every "Closes by" reference a slice in `implementation-slice-checklists.md`.
