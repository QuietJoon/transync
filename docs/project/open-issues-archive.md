# Open Issues — Archive

Entries moved from `docs/project/open-issues.md` after their implementation impact had lapsed. Kept for audit and historical context. Do not add new OPEN issues here.

> **Note added 2026-08-07 (ticket `4af82bbc`) — which review round this file
> cites.** Every `R0001-NNNN` id below is from the **2026-05-02** Review 0001,
> whose file was removed in `bb93b68` (`reviews/reviewed/0001.md`) — not the
> `reviews/0001.md` on disk today, which numbers a different 52 findings over
> the same `0001`–`0052` range. Most entries say so on their own `Source:`
> line with the older `(review archived and removed)` spelling; this note
> covers the ids that appear only in an entry's body, such as OI-0014's
> `R0001-0010` / `0009` (the `--model` and `--base-url` flags, not the live
> round's findings of those numbers). `reviews/README.md` carries the round
> registry and the retired round's finding index.
>
> **Extended 2026-09-03 (ti `bdf8d981`).** The same applies to the
> `R0002`/`R0003`/`R0004` ids below: they are all from the **2026-05** rounds,
> because this file's entries predate the three gated rounds of 2026-08-07/07/11
> that reused those numbers. A bare id written after that date means the
> **2026-08** round (`reviews/README.md` rule 4). Numbers settle most of them
> without any marker — `R0004-0001` is the 2026-05 external consumer report by
> rule, since that round had exactly one finding — and this note covers the rest.

***

> Archived 2026-07-11. Reason: resolved security fix (DOMPurify sanitize-before-mount) — audit history.

## OI-0001: Demo shells mount fetched HTML via unsanitized `innerHTML`

- **Source:** R0001-0001 + R0001-0095 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** DOMPurify 3.2.6 vendored at `web/vendor/purify.min.js` with a byte-identical CLI-embedded copy (`crates/transync-cli/web/purify.min.js`, lockstep enforced by `purify_js_workspace_and_embedded_are_byte_identical`). Both shells sanitize fetched fragments before `innerHTML` and fail closed with an on-page error when `window.DOMPurify` is missing. The CLI bundle is now six files (`purify.min.js` added; ADR-0006 + persistence-and-files updated); `web/SMOKE.md` checklist updated.

### Problem

Both demo shells (`web/index.html` and `crates/transync-cli/web/index.html.tpl`) assign `fetch().then(r => r.text())` results directly via `el.innerHTML = src`. No DOMPurify is loaded. The MVP scope and design spec explicitly require DOMPurify before mount; `web/SMOKE.md` currently waives this on the grounds that fragments are produced by transync's own renderer.

### Impact

In the current configuration the renderer is the only producer of these fragments and the renderer escapes attributes / strips raw HTML, so the practical injection surface is narrow. The risk window opens when (a) a third-party renderer is plugged in or (b) a malicious source document somehow reaches the HTML stage with raw HTML enabled.

### Required Actions

1. Decide between vendoring DOMPurify under `web/vendor/` (preferred) vs. switching from `innerHTML` to a `DOMParser` + manual mounting path.
2. Apply the chosen mitigation to both `web/index.html` and `crates/transync-cli/web/index.html.tpl`.
3. Update `web/SMOKE.md` to require DOMPurify in the visual-smoke checklist.

### Verification

- [ ] Mitigation applied to both shells
- [ ] `web/SMOKE.md` updated
- [ ] Manual smoke confirms unchanged demo behavior

### Related

- `docs/superpowers/specs/2026-05-01-transync-design.md` §security
- `docs/architecture/mvp-scope.md`

***

> Archived 2026-07-11. Reason: resolved behavior change; records the linked-images-stay-paragraphs trap.

## OI-0002: Image blocks modeled but not parsed as block-level anchors

- **Source:** R0001-0030 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** Option (a) implemented. The parser promotes image-only paragraphs (including badge rows; images + whitespace only) to `BlockKind::Image`; the renderer emits a `<figure data-sync-id="img-…" data-block-kind="image">` wrapper with the bare `<img>` inside; alignment rows carry `block_kind: image`, `sync_role: anchor`, `fallback_status: preserved` (never batched). Linked images (`[![…](…)](…)`) intentionally stay paragraphs. Pinned by `parser::image_block_tests` and the SCN-14 kind-sequence walk.

### Problem

`BlockKind::Image` exists in the IR and `contracts.md` §3 lists `image` as a sync_role, but Comrak's `NodeValue::Image` is inline-only and the parser default branch skips it. Documents containing block-level images (a paragraph whose only child is an image) currently render as paragraphs with no `image` sync role.

### Impact

Block-level images cannot receive the documented `image` kind or `<figure>` sync wrapper. Functional impact is small — the paragraph fallback still yields a working sync anchor — but the documented contract is not actually delivered.

### Required Actions

1. Decide between (a) detecting "image-paragraph" patterns in the parser and emitting `BlockKind::Image`, or (b) removing `Image` from the MVP contract and updating `contracts.md`.
2. If (a): emit `<figure>`-shaped HTML in the renderer, and define how figure captions interact with sync.
3. If (b): drop the `BlockKind::Image` variant + alignment-row support.

### Verification

- [ ] Implementation matches the chosen path
- [ ] `samples/demo-complex.md` image renders with the expected sync_role

### Related

- `docs/architecture/contracts.md` §3
- `docs/architecture/contracts.md` §4

***

> Archived 2026-07-11. Reason: resolved; records profile-vs-caller precedence and the folded unknown-key warning-channel decision.

## OI-0003: Profile `[constraints]` and `[batching]` sections discarded

- **Source:** R0001-0013 + R0001-0004 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** `[constraints]` and `[batching]` are typed (`ProfileConstraints` / `ProfileBatching` on `ProfileMetadata`). The constraint booleans render as prompt policy lines (cache identity follows via the prompt hash); `default_table_strategy` is validated with a fallback warning for the reserved row-window value; `[batching].max_units_per_batch` seeds the batcher default with caller override; `target_output_tokens`/`max_split_retries` remain advisory (documented). The folded R0006-0080/0081/0082 warning-channel decision is implemented: the loader collects path-qualified unknown-key warnings on `ProfileMetadata.load_warnings`, emits `tracing` warnings, and the CLI prints them — contracts §2, the Cookbook, and the design-baseline promise now all agree. Pinned by `profile::oi_0003_tests` (5 tests).
- **Historical progress:** Glossary threaded (commit `99a0065`); UserPromptUnit constraint/context hints (commit `fa8b2b0`).

### Problem

`ProfileToml` parses `[constraints]` and `[batching]` as `toml::Value` and discards them when building `ProfileMetadata`. The CLI's `--profile <path>` documented knobs (`preserve_code_identifiers`, `default_table_strategy`, `target_output_tokens`, `max_units_per_batch`, etc.) currently have no effect.

Glossary, the third sub-section flagged by the same finding, has been plumbed end-to-end. Constraints and batching remain.

### Impact

Documented profile knobs do not influence translations or batching. `[batching].max_units_per_batch` in particular looks like it would tune the same dial as `TranslateOptions::max_units_per_batch` but does not.

### Required Actions

1. Model `[constraints]` and `[batching]` in `ProfileMetadata` as typed structs.
2. Thread `[constraints]` into per-block `BlockConstraints` when more permissive than the inferred ones.
3. Thread `[batching]` defaults into `TranslateOptions` when the caller did not override.
4. Update `docs/Profile_Cookbook.md` with the new effective fields.

### Verification

- [ ] Profile constraints alter validator behavior
- [ ] Profile batching alters batch sizing
- [ ] Cookbook recipes updated

### Related

- `docs/architecture/contracts.md` §2
- `docs/Profile_Cookbook.md`
- **Folded in (2026-07-10, gate for Review 0006):** R0006-0080/0081/0082 — contracts.md promises unknown-profile-key warnings, the Cookbook says keys are silently ignored, and the loader has no warning channel. Resolve together with the typed `[constraints]`/`[batching]` work: add a warnings channel to the loader API (or weaken the contract + design-baseline wording via a DCR), then reconcile all three docs.

***

> Archived 2026-07-11. Reason: resolved; leaf-model / reserved-hierarchy rationale cited by the Developer Guide.

## OI-0005: Parser carries unconsumed metadata fields

- **Source:** R0001-0036, R0001-0038, R0001-0039, R0001-0040 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing — "they're used next step")
- **Status:** RESOLVED (2026-07-10)
- **Resolution (per sub-item):**
  - R0001-0036 (re-key staleness): **fixed** — `assign_block_ids` now rewrites `parent_id`, `section_path`, and `hierarchy` through the old→new map; pinned by `id::rekey_tests`.
  - R0001-0038 (`AstPath` unconsumed): **consumed** — the full-reparse normalizer groups list runs by path prefix and the renderer's DCR-0007 list grouping does the same.
  - R0001-0039 (`doc.hierarchy` unconsumed): **kept-reserved** — doc-marked as the hierarchical-alignment staging field, now id-consistent under re-keying; per-block `section_path` remains the consumed model.
  - R0001-0040 (nearest-scope attachment): **documented as intended** — `Section::children` lists the nearest scope's blocks; ancestor membership derives from `section_path`, which carries the full heading chain.

### Problem

Cluster of "code claims more than it does" findings in the parser:
- `assign_block_ids` only overwrites `block_id`, leaving `parent_id` / `section_path` / `hierarchy` stale after re-keying.
- `AstPath` is populated in `parser.rs` but no downstream code reads it.
- `doc.hierarchy` is computed but unconsumed; per-block `section_path` is the only section model used downstream.
- Section hierarchy attaches non-heading blocks only to the nearest heading scope, not to ancestor sections.

### Impact

Cosmetic for MVP (nothing breaks). The fields exist as stage-setting for future features the user has flagged as upcoming work — leaving them in place ensures the next pass (AST-based regeneration, hierarchical alignment) can land without re-introducing parser walker state.

### Required Actions

Once the dependent feature lands, either consume the fields or remove them.

### Related

- `docs/decisions/0005-block-id-format.md`

***

> Archived 2026-07-11. Reason: resolved; paired with DCR-0008 (its Source).

## OI-0006: JS sync engine diverges from ADR-0001's documented algorithm

- **Source:** R0001-0064 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing — "related to architecture, track only")
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** DCR-0008 records the divergence; ADR-0001 is amended in place to point at `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md` as the authoritative algorithm. The optional IntersectionObserver revisit is noted in DCR-0008's follow-up (tied to OI-0016's profiling gate).

### Problem

ADR-0001 specifies the active-block algorithm as `IntersectionObserver` with intersection ratio + viewport-center distance + hysteresis. The shipping `web/js/sync.js` uses scroll listeners + a top-edge heuristic + per-frame `lerp` (the smooth-scroll spec at `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md` already supersedes parts of this).

### Impact

The implementation works (and is the actively-used behavior across all SCN-13 demos) but the ADR no longer matches reality. New contributors reading ADR-0001 will reach the wrong mental model.

### Required Actions

1. Write a Design Change Record describing the divergence and rationale.
2. Update ADR-0001 to reference the smooth-scroll spec as the authoritative algorithm.
3. (Optional, post-track-only judgment) revisit whether IntersectionObserver-based active-block detection should replace or augment the current scroll-listener approach.

### Related

- `docs/decisions/0001-block-level-alignment-as-sync-currency.md`
- `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`

***

> Archived 2026-07-11. Reason: resolved; provider test-coverage closure evidence cited by the stub manifest.

## OI-0009: OpenAI client request body is untested

- **Source:** R0001-0081 (Review 0001) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** `client.rs` now carries a 12-test module covering both request-body builders (incl. the `store: false` retention pin), `schema_object_for` min/maxItems, strict-mode required-property completeness (ADR-0008 pin), `parse_batch_output` happy/error paths, `build_endpoint` (default, `/v1` proxy dedup, query/fragment stripping), constraint-hint serialization + injection framing, and the API dispatch heuristic. `cargo test -p transync-openai` runs 12 tests.

### Problem

`crates/transync-openai/` has zero unit tests in `cargo test --workspace`. `build_request_body`, `build_user_prompt`, `schema_object_for`, and the response parser are pure functions and easy to test, but currently rely entirely on live-API smoke for verification.

### Required Actions

1. Add unit tests for `build_chat_request_body` / `build_responses_request_body` shape.
2. Add unit tests for `parse_batch_output` against known-good envelope shapes (Chat Completions + Responses).
3. Add unit tests for `schema_object_for(Some(N))` so a regression on `minItems`/`maxItems` is caught at `cargo test`.

### Verification

- [ ] `cargo test -p transync-openai` runs at least 8 tests.

### Related

- `crates/transync-openai/src/client.rs`

***

> Archived 2026-07-11. Reason: resolved; paired with DCR-0007 (its Source).

<!-- Entry Template (copy below line for new entries) -->

## OI-0010: Consecutive list items render as separate one-item lists

- **Source:** R0003-0027 (Review 0003) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** DCR-0007. The renderer groups consecutive items of one source list (same `ast_path` prefix) inside a single unattributed `<ul>`/`<ol>` and moves the sync attributes onto real `<li>` elements; marker-type changes start a new group. The folded R0006-0042 metadata contradiction is resolved the same way: `sync_role` for list items is now `anchor` (they are the primary anchors; no list-level row exists). Pinned by `render::list_grouping_tests` (5 tests).

### Problem

The renderer emits each top-level list-item as its own `<div data-sync-id="li-NNNN">` wrapper, with Comrak output for that one item — which reparses as a single-item `<ul>`. The result for `- a\n- b\n` is `<div><ul><li>a</li></ul></div><div><ul><li>b</li></ul></div>` instead of one shared `<ul>` with two `<li>`s.

### Impact

Visual styling that targets list-spacing breaks across consecutive items. Screen readers may also read each item as its own list. Functional sync is unaffected because each item still has a stable `data-sync-id`.

### Required Actions

1. Group consecutive list-items in `render_source` / `render_target` into a single `<ul>` / `<ol>` parent that wraps the per-item `data-sync-id` divs (or move the sync-id to `<li>` itself).
2. Re-run SCN-05 visual smoke.
3. **Folded in (2026-07-10, gate for Review 0006):** R0006-0042 — alignment metadata marks list items `ChildOnly` while the renderer emits them as top-level sync anchors; resolve the metadata (ChildOnly vs TopLevel) together with whichever grouping option is chosen, so map and DOM agree.

***

> Archived 2026-07-11. Reason: resolved; abort-keeps-progress precedent cited by open OI-0017.

## OI-0011: One provider error cancels in-flight concurrent batches

- **Source:** R0003-0006 (Review 0003) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** `run_pipeline` now collects every batch outcome instead of `try_collect`: in-flight batches run to completion, successes land in the cache (so a retried run only re-dispatches the failed remainder), and the run still fails with the lowest-input-index error after all batches settle. Partial-success policy decided as "abort but keep cached progress" — no public-API change.

### Problem

`pipeline.rs::run_pipeline` uses `futures::stream::buffer_unordered(...).try_collect()`. The `try_collect` future cancels all in-flight batches on the first `Err`. For a transient `Network` error this is wasteful — successful batches that already returned are dropped on the floor.

### Impact

Wall-clock recovery from a partial failure is worse than necessary. A document-level cache (OI-style external) would partially mitigate this since successful batches would land in the cache and skip on retry.

### Required Actions

1. Switch from `try_collect` to `collect` of `Result<...>`, then partition successful and failed batches; surface the first error after collecting all.
2. Decide policy for "partial success" — return early with what we have, or always abort if any batch failed?

***

> Archived 2026-07-11. Reason: resolved; separate validation-report rationale cited by persistence-and-files.md.

## OI-0012: CLI alignment JSON omits per-unit validation report

- **Source:** R0003-0024 (Review 0003) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** `--validation-report <path>` writes the per-unit report (attempt log, rejection reasons, provider warnings, full-reparse fallbacks, provider retries) as JSON, committed in the same all-or-nothing fileset as the other outputs; no alignment-map schema bump needed. Documented in contracts.md §6; pinned by `cli_validation_report_output`.

### Problem

`out.json` (the CLI's alignment-map output) carries `validation_summary` (counts) but not the per-unit `validation_report` (attempt log + rejection reasons). Library-API callers receive the report on `TranslationOutput.validation_report` but CLI users have no way to see why a unit fell back without re-running.

### Required Actions

1. Either embed `validation_report` in the alignment-map JSON (with a `schema_version` bump to 1.1.0), or add a separate `--validation-report <path>` CLI flag.

***

> Archived 2026-07-11. Reason: resolved doc-drift audit record cited by the stub manifest.

## OI-0014: source-of-truth-table.md drift vs. live code

- **Source:** R0003-0053..0056 (Review 0003) (review archived and removed)
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track)
- **Status:** RESOLVED (2026-07-10)
- **Resolution:** Gate session for Reviews 0006/0007 reconciled every flagged row against live code: token-estimation and default-profile ownership corrected to `transync-core` (R0006-0056/0057), the DOMPurify row now states sanitization is absent/waived and links OI-0001 (R0006-0058), and the `--model`/`--base-url` rows were verified against the now-implemented flag > env > default resolution (R0006-0009).

### Problem

`docs/architecture/source-of-truth-table.md` claims:
- Token estimation belongs to the provider crate (it lives in `transync` core via tiktoken-rs).
- Default profile belongs to the CLI (it's embedded in `transync` core via `include_str!`).
- DOMPurify is used for HTML safety (it isn't — see OI-0001).
- CLI passes `--model` / `--base-url` into the provider (it does NOW after R0001-0010 / 0009 fixes; the table was correct prospectively but hadn't been verified against shipped code).

### Required Actions

1. Reconcile each table row with the live code; either correct the row or fix the code.

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-05 (DCR-0019) — the refactor / perf debt cluster was fully consumed by the internal-quality wave.

## OI-0008: Refactor / perf debt cluster

- **Source:** R0001-0057, R0001-0069, R0001-0076..R0001-0080, R0001-0096 (Review 0001) (review archived and removed — the 2026-05-02 round, `reviews/reviewed/0001.md`, not today's `reviews/0001.md`; see `reviews/README.md`). Every `R0001-` id in this entry, including the bullets below, is from that round.
- **Date:** 2026-05-04
- **Decision:** ACCEPT (track per Phase 2 routing — value-vs-cost call deferred)
- **Status:** **RESOLVED 2026-08-05 (DCR-0019)** — every bullet group consumed by the OI-0008 + OI-0033 internal-quality wave (11 commits, `b1c3c7f`..`73fd331`). Previously narrowed 2026-08-04 (DCR-0017), which cleared the two renderer items.

### Problem

Aggregate of refactor / perf items the gate flagged as low-impact and deferable. Each bullet group with the commit that closed it:

- ~~Retry/fallback policy split across no-op helpers (R0001-0057).~~ **RESOLVED 2026-08-05 (`f82fb37`)** — the decision core is now `crates/transync-core/src/pipeline/policy.rs`, a **pure** module (approach A1: no `tokio`, no `Cache`, no `Translator`, nothing `async`) owning the per-unit content-fault counter, the per-batch schema-fault round counter, the provider-transient budget, the dispatch ordinal, and the backoff schedule; `process_one_batch` keeps every provider call, cache write, sleep, and log line and asks the policy object at each branch. Twelve decision tests now run without a mock translator. The extraction surfaced two facts recorded in DCR-0019: `ValidationReport.total_retries` was conflating batch-schema-fault rounds and rejected cache hits with content retries (**fixed**, the wave's second sanctioned behavior change), and `max_per_batch_provider_retries` is charged per **dispatch round** despite its per-batch name (**preserved verbatim**, ticket `294ddabd`). *Earlier note 2026-08-04 (DCR-0018): the three no-op helpers themselves were deleted as closure fallout — that removed the scaffolding, not the debt; this bullet is what removed the debt.*
- ~~Renderer reparses every block individually (R0001-0069).~~ **CLEARED 2026-08-04 (DCR-0017)** — the AST-direct rewrite does **one** Comrak parse per pane (2 per run instead of `2 × (N_blocks + N_ol_runs)`) and dropped the O(N_blocks × |ref_defs|) reference-definition append entirely.
- ~~`strip_outer_wrapper` is brittle string parsing (R0001-0096).~~ **CLEARED 2026-08-04 (DCR-0017)** — `strip_outer_wrapper` (all arms plus its dead fallback path), `block_inner_html`, and `ordered_list_start` were **deleted**; the renderer formats the paired AST node instead of rewriting Comrak's HTML. The rework also surfaced and fixed a latent bug the string surgery hid (4-space-indented code blocks rendered as `<p>` because the pre-reparse `md.trim()` destroyed the indent).
- ~~`run_pipeline` mixes multiple concerns (R0001-0076).~~ **RESOLVED 2026-08-05 (`70730dc`)** — split into `pipeline/{dispatch,finalize,report}.rs` plus the policy module, with `run_pipeline` reduced to a thin phase sequence over them. Verified as pure motion by a normalized line-multiset diff (zero production lines on either side) and an identical test-name multiset. Alignment-map assembly stayed inline deliberately — it has no existing signature, so lifting it would have been design, not motion; recorded as an open seam. *That seam is closed as of 2026-08-05 (backlog item `alignment-map-assembly-lift`): the four statements moved verbatim into `pipeline::report::assemble_alignment_map`, so the reporting phase owns both of its products; behavior preservation checked by the workspace suite plus a 141-file byte-diff of the CLI output tree across all fourteen fixtures.*
- ~~The parser walker carries too many mutable cross-cutting parameters (R0001-0077).~~ **RESOLVED 2026-08-05 (`094c1c4`, `73fd331`)** — full scope, subtractive first: `Document::hierarchy` (a whole extra pass with allocations, read by nothing) and `Block::parent_id` (provably always `None`) were **deleted**, simplifying **six** tautological filter sites; `emit`, the section-scope algorithm, and the `heading_level` mapping each collapsed to one home; then `visit`/`walk_children` restructured onto a `WalkState` across `parser/{options,classify,emit,sections}.rs`, deleting three `#[allow(clippy::too_many_arguments)]` and the `emit_here!` macro. The wire field `AlignmentBlock.parent_id` stays at schema 1.2.0, always null; byte-stability proven by regenerating 112 CLI output files from the pre-change tree and diffing.
- ~~Unit construction mixes multiple concerns (R0001-0078).~~ **RESOLVED 2026-08-05 (`f10b070`, `8b339f3`)** — the structural-fingerprint oracle moved to `crates/transync-core/src/structure.rs` (+ `structure/labels.rs`), which removes the inversion where `validate::per_kind` imported its oracle from the unit builder; then `unit/payload.rs` took the per-kind strategy and `unit/budget.rs` the budget resolution, collapsing two of the three "caller value differing from the built-in default wins" copies into one helper. `unit::build_batches` and `unit::html_outcomes` keep their exact paths.
- ~~The OpenAI client mixes multiple concerns (R0001-0079).~~ **RESOLVED 2026-08-05 (`3587d86`, `5686387`)** — `client.rs` split into `client/{classify,transport,endpoint,dispatch,chat,responses}.rs` plus a top-level `tokenizer.rs`, behind ONE surface abstraction (`SurfaceRequest` + `round_trip`) shared by translation and glossary extraction. The HTTP-status → `ProviderError` classification table — the pipeline's retry-policy input, and previously **untested** — was pinned tests-first (29 unresolved-name compile errors as the RED step) before being moved character-for-character. The dead `pagination` module went with it.
- ~~The CLI translate command mixes multiple concerns (R0001-0080).~~ **RESOLVED 2026-08-05 (`8b1a434`)** — split into `translate_cmd/{args,input,provider,publish,report}.rs` behind an `execute(args, reporter) -> Result<RunSummary, CliFailure>` seam that collapses fifteen scattered exit-code sites into one table, and one file-set builder that replaces the two duplicated `OutputTarget` arms. Byte-stability verified by running the pre-refactor and post-refactor binaries over seven invocations: output trees, stderr, and exit codes identical.

### Impact

Future-maintainability concern; none of these items ever affected correctness (SCN-01..SCN-15 passed throughout). The original routing rule was "address as part of the next concrete feature that touches the relevant module", and two waves honored it — DCR-0017 cleared the two renderer items because the crate split had to touch the renderer anyway.

That rule ran out of road: the next concrete feature is **Track C**, which ships `transync-syntax` to the browser. Landing this debt after Track C would mean refactoring code already compiled into a WASM artifact with a JS integration on top; landing it before means Track C starts from a parser that does one walk instead of two and exposes a `WalkState` rather than a nine-argument function pair. The owner accepted the double-work risk against Track C's own IR-boundary design, mitigated by the wave's IR freeze rule (beyond the two field deletions, `Document`/`Block`/`Section` shapes are frozen).

### Verification

- [x] `cargo test --workspace -- --test-threads=4` — **385 passed / 0 failed / 3 ignored**, up from the pre-wave 340; zero test removals with one sanctioned 1-for-1 swap (2026-08-05)
- [x] `crates/transync/tests/public_surface.rs` green and byte-identical to tag `v0.2.0` — the wave never touched the curated surface (2026-08-05)
- [x] fmt / clippy `-D warnings` / wasm32 gate / `scripts/smoke.sh` (incl. the rustdoc gate) / Playwright SCN-13 **8/8** (2026-08-05)
- [x] `resp-translator` `cargo check --workspace` green against this HEAD, no migration needed (2026-08-05)

### Related

- DCR-0017 (cleared the two renderer items 2026-08-04), DCR-0018 (deleted the three empty retry hooks as closure fallout), **DCR-0019** (this resolution)
- Tickets filed from the wave and left open: `294ddabd` (per-round vs per-batch provider budget), `69b9b3db` (a fourth heading kind→level mapping in `unit/payload.rs`) — *(2026-08-05: **both resolved**, so neither is open. `294ddabd` in `1436304` by route (a): `BatchPolicy::begin_dispatch_round` no longer resets `provider_attempts`, so the budget is charged per batch across the whole retry ladder — recorded in ADR-0009's amendment, `contracts.md` §5, and the CHANGELOG's `[Unreleased]` Fixed entry, with the first end-to-end transient-retry tests. `69b9b3db` in `7572c45`: `constraints_for` seeds `must_preserve_heading_level` from `BlockKind::heading_level()`, leaving `id.rs` the single kind→level home. The narrative above stays as written — it records what was true when the wave landed.)*

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-09 (ticket `d3acc3`) — single-file packaging re-affirmed and ID identity made a normative, tested schema-1.x invariant.

## OI-0015: Browser sync module mixes four concerns

- **Source:** R0006-0050 (Review 0006) (review archived and removed)
- **Date:** 2026-07-10
- **Decision:** ACCEPT (track — user routing in the Review 0006 gate)
- **Status:** **RESOLVED (2026-08-09, ticket `d3acc3`)**
- **Resolution:** both open questions answered, in the pass this issue was
  waiting for. **(1) Single-file packaging is RE-AFFIRMED, not split.** The
  trigger fired — the reflow hooks of OI-0024 item 1 are the substantive JS
  feature — and the answer is that the split is not worth its price:
  `web/js/sync.js` is mirrored byte-for-byte into `crates/transync-cli/web/sync.js`
  under `sync_js_drift.rs`, and `web/` is a no-build, framework-free tree, so a
  source boundary would multiply the mirror *and* the asset list every consumer
  ships. The re-affirmation is recorded in the module's own doc-comment (so the
  next reader meets it in the file) and in DCR-0008's dated note. **(2)
  ID-identity pairing is now DOCUMENTED and enforced**, not merely observed:
  identical `data-sync-id` is a stated invariant of alignment schema 1.x
  (ADR-0001 amendment 2026-08-09, `contracts.md` §3), the map's source/target
  indirection is reserved for a future divergence revision rather than routed
  through today, and a schema-1.x map whose row contradicts the identity is
  refused instead of mounted-but-unsyncable. That also resolves Review 0003's
  `R0003-0002`, which found the two consumer code paths disagreeing about which
  id the target side is keyed by — they now read the same one. Pinned by
  Playwright `engine.spec.js` test `g`.

### Problem

`web/js/sync.js` (mirrored byte-for-byte into `crates/transync-cli/web/sync.js`) carries the runtime sync engine, alignment-map validation, a fetch helper, and a storage helper in one module.

### Impact

Maintainability only — all scenarios pass. Splitting means shipping multiple JS assets, updating the CLI-embedded mirror, and keeping the `sync_js_drift.rs` byte-equality test satisfied, so the change should ride on the next substantive JS feature rather than land alone.

### Required Actions

1. ~~When the next JS feature lands, split the module (engine vs. helpers) or explicitly re-affirm the single-file packaging.~~ — **DONE 2026-08-09: re-affirmed.**
2. ~~Keep both copies byte-identical (test already enforces).~~ — **standing; both copies moved together in `d3acc3`'s commit and `sync_js_drift.rs` is green.**
3. ~~**ID-identity pairing is undocumented (DR-2026-07):** the JS runtime pairs source/target anchors by *identical* `data-sync-id` (`sync.js` `handleScroll`), silently ignoring the alignment map's `source_block_id` / `target_block_id` indirection — the map could express a non-identity pairing that the engine would not honor. When the module split happens, either route pairing through the map or document ID-identity as a normative, tested schema-1.x invariant so the two cannot silently disagree.~~ — **DONE 2026-08-09**, by the second of the two offered routes: documented as normative and tested. The engine now also *refuses* a map that contradicts it, so "the map could express a non-identity pairing the engine would not honor" has stopped being a silent state.

### Verification

- [x] Code change applied — `web/js/sync.js` + the byte-identical CLI twin, one commit.
- [x] Tests pass — Playwright `engine.spec.js` `g`, the `sync_js_drift.rs` byte-equality and schema-mirror tests.
- [x] No regressions observed — full browser suite green.

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-09 — all four items shipped, the last with DCR-0028's `Cache` metadata seam and `--cache-dir`.

## OI-0017: Cache identity and eviction design (provider namespace, staged commit, Hard-failure rollback)

- **Source:** R0008-0002 + R0008-0003 + R0008-0004 (Review 0008); R0002-0026 (Review 0002, folded by archive cleanup) (review archived and removed)
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — user routing in the Review 0008 gate)
- **Status:** **RESOLVED (2026-08-09)** — all four items shipped.
- **Resolution of item 4 (2026-08-09, DCR-0028 / ADR-0021, slices SL-111 + SL-114):** document-level metadata rides the `Cache` seam. `DocumentMetaKey` / `DocumentMeta` and two **defaulted** trait methods (`get_document_meta` / `put_document_meta`) let the pipeline write the run's detection from a live qualifying envelope and replay it on a run that dispatched **zero** provider batches — replay compensates for elided calls, it never overrides what a live provider said. The seam, not one backend, was the right home: the fix works for any `Cache` a consumer passes to `translate_with_cache`, and a defaulted method leaves every existing implementation compiling with its pre-v0.4.0 behavior. `DiskCache` (SL-112/SL-113) makes the record durable across processes, and `transync translate --cache-dir` (SL-114) makes the scenario literal: `cli_cache_dir_persists_progress_and_detected_language` runs the binary twice against one cache directory with `--source-language auto` and asserts the second run — whose stub provider would have answered a *different* language — publishes a byte-identical output set, detected language included. SCN-10's partial-resume guarantee lost its one carve-out with it.
- **Partial resolution (2026-07-13, EXT-2026-07 P1-4 / v0.2 boundary):** items 1–3 shipped — `Translator::fingerprint()` + `CacheKey.provider_fingerprint`/`validation_schema_version` (item 1); `Cache` trait v2 gained `evict`, and the pipeline evicts exactly the reparse-cascade-downgraded keys (item 2) and the blocks implicated in a `FullReparseFailure::Hard` error (item 3) — OI-0011's keep-progress behavior preserved. Remaining scope is item 4 only (detected-source-language caching), deferred to the disk-backed-cache design — now DCR-0028.

### Problem

Four converging gaps in the cache design, all requiring the same `Cache`-trait / public-API decision:

1. **No translator/endpoint identity in the key** (R0008-0002): `CacheKey` carries `model_id` but neither a translator namespace nor the base URL / API surface, so a cache shared across two `Translator` instances could return the other provider's output.
2. **Units are cached before the full-document structural gate** (R0008-0003): `process_one_batch` writes accepted units to cache before `finalize_regen_with_reparse_policy`; a unit later downgraded by the reparse cascade stays cached, so a shared-cache re-run replays the collectively-bad translation instead of a fresh attempt. Output is never corrupted (the cascade re-runs each pass) — only re-run freshness suffers.
3. **`FullReparseFailure::Hard` leaves the cache populated** (R0008-0004): a Hard run returns `Err` with no rollback, so subsequent runs against a shared cache deterministically replay the failing state.
4. **Detected source language is not cached** (R0002-0026): the per-unit cache stores no document-level metadata, so a fully-cache-hit `--source-language auto` run reports `detected_source_language: None` (documented in README/rough-schema, but the cache design should decide whether to persist it).

No shipped caller is exposed today: the CLI builds a fresh per-run cache, only one `Translator` ships, and the default policy is `FallbackPerBlock`. The reviewer's blanket "commit only after full reparse" would revert OI-0011's settled "abort but keep cached progress"; the defensible fix is targeted eviction, which needs an `evict`/namespace extension on the `Cache` trait.

### Impact

Only materializes when an external caller shares a long-lived cache across translators or opts into `Hard` — the exact scenario the future disk-backed-cache work targets.

### Required Actions

1. When designing the disk-backed cache (the trait's stated purpose): add a caller-supplied cache namespace / provider fingerprint to `TranslateOptions` or the `Translator` contract (R0008-0002).
2. Same design point: add targeted eviction (`evict(&CacheKey)` or generation tags) and evict reparse-downgraded keys after the cascade (R0008-0003) and on `Hard` failure (R0008-0004) — without reverting OI-0011's keep-progress-on-abort behavior.
3. Same design point: decide whether document-level metadata (detected source language) is cached so fully-resumed `auto` runs keep reporting it (R0002-0026).

### Related

- OI-0011 [archived] (settled: abort keeps cached progress)
- `docs/architecture/rough-schema.md` §14 (cache key contract)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-07 — the re-open marker fired as an owner posture update; `--strict-csp` is the per-run escape hatch and the default is unchanged.

## OI-0018: Bundle ships no CSP; remote source images load in the viewer

- **Source:** R0008-0009 + R0008-0051 (Review 0008) (review archived and removed)
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — user routing in the Review 0008 gate: "drop, but track")
- **Status:** RESOLVED (2026-08-07) — **posture amended, default unchanged**
- **Resolution:** Ticket `14307f84` — required action 2 (the opt-in escape hatch) shipped as `transync translate --strict-csp`; required action 1 (CSP by default, both shells) is **explicitly not taken** and is not pending.

### Problem

Neither demo shell sets a Content-Security-Policy. Remote image URLs in the untrusted source Markdown are emitted verbatim and fetched when a generated bundle is opened, which can disclose viewer IP/timing to the image host (tracking-pixel pattern); a CSP would also be defense-in-depth behind DOMPurify.

### Impact

Accepted by owner decision: transync is usually not the document's first consumer — source documents are typically already-downloaded local copies, so their images are expected to be local/already-fetched resources, and a `img-src 'self' data:` CSP would visibly break legitimately-rendered remote images. Privacy exposure is limited to opening a bundle generated from a hostile remote-image document.

### Owner posture update (2026-08-06)

The default posture above **stands**: bundles ship no CSP, and a run without the new flag emits a byte-identical bundle to every prior version. What changed is that users translating sensitive documents — or handing a bundle to a third party, the scenario the original re-open marker named — now get an escape hatch instead of having to choose between the default and a source edit. The flag is per run, so the tradeoff is chosen where it is known.

### Required Actions

1. ~~If the posture changes (e.g. bundles are shared with third parties), add this CSP meta to both shells (`web/index.html`, `crates/transync-cli/web/index.html.tpl`)~~ — **not taken.** The posture did not change; the flag did. The static shells under `web/` are dev demos serving repo-local fixtures, and their posture is untouched.
2. ~~Alternatively, offer an opt-in `--strict-csp` bundle flag so privacy-sensitive users can choose the tradeoff per run.~~ **DONE (2026-08-07)** — `--strict-csp` stamps `default-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'` (the policy the archived Review 0008 patch proposed, unchanged) into the emitted `index.html`'s `<head>`, in both bundle modes. `frame-ancestors` / `sandbox` / `report-uri` are omitted because a `<meta>`-delivered policy ignores them. Contract: `contracts.md` §6 "Bundle Content-Security-Policy". Pinned by three CLI-smoke tests (policy present, default output byte-identical apart from the element, `--out-dir` covered) and two unit tests over the shell assembler.

### Related

- OI-0001 [archived] (DOMPurify sanitize-before-mount, resolved)
- Invariant 7 (source Markdown is untrusted)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-07-13 — provider response-size cap and output-token ceiling shipped (EXT-2026-07 P1-7).

## OI-0019: Provider requests/responses lack resource bounds

- **Source:** R0008-0012 + R0008-0031 (Review 0008) (review archived and removed)
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — user routing in the Review 0008 gate)
- **Status:** RESOLVED (2026-07-13)
- **Resolution:** EXT-2026-07 P1-7 — `post_json` enforces a 32 MiB response ceiling (Content-Length precheck + bounded chunk accumulation; overflow is a terminal error) and `profile.batching.target_output_tokens` is now the live output ceiling (`max_completion_tokens` on Chat Completions, `max_output_tokens` on Responses; omitted when unset). CLI-side admission caps (`--max-input-bytes`, 4 MiB aux-file caps) landed in the same wave (ADR-0016 amended).

### Problem

1. **Unbounded response buffering** (R0008-0012): `post_json` buffers the entire response body via `response.bytes()`; a misbehaving compatible endpoint can force arbitrarily large allocation. (Error-body *diagnostics* are now capped at 512 bytes — R0008-0034 — but the buffer itself is not.)
2. **No output-token ceiling** (R0008-0031): neither API request sets `max_output_tokens` / `max_completion_tokens`; profile `target_output_tokens` is advisory-only (documented, STUB-manifest), so a runaway generation behind a custom proxy is unbounded in cost.

### Impact

Local CLI talking to `api.openai.com` or a user-chosen base URL — worst case is the user's own process memory / API bill. Cost-control hardening, not a correctness bug.

### Required Actions

1. Add a streaming byte ceiling (e.g. 32 MB) in `post_json`, erroring with a clear diagnostic when exceeded.
2. Thread an effective output budget (from `profile.batching.target_output_tokens` or a new option) into both request bodies.

### Related

- STUB-manifest `target_output_tokens` advisory note
- R0008-0037 (schema payload bounds — dropped: negligible effect)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-06 — `Cargo.lock` is committed (`6cf4164`) and the `wasm-bindgen` pin is kept alongside it.

## OI-0020: Committed Cargo.lock vs. floating dependency resolution

- **Source:** R0008-0056 (Review 0008) (review archived and removed)
- **Filed as (2026-07-11):** "No committed Cargo.lock (blocked by machine-global gitignore)" — kept verbatim here; the heading was retitled 2026-08-07, after the lockfile landed, so that it stops asserting a state the tree contradicts.
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — user routing in the Review 0008 gate)
- **Status:** RESOLVED (2026-08-06)
- **Resolution:** Commit `6cf4164` — `.gitignore` gained a repo-local `!Cargo.lock` un-ignore overriding the machine-global rule, and `Cargo.lock` is tracked (`git ls-files --error-unmatch Cargo.lock` succeeds). That is the first of the Required Action's two alternatives, taken one day after the 2026-08-05 deferral and reversing it; the `wasm-bindgen = "=0.2.126"` pin stays **alongside** the lock, not instead of it. The 2026-08-06 note below records what the lock covers, what the pin still covers, and the one part of the original gitignore concern that was declined rather than fixed.

### Problem

The workspace ships a CLI binary but has no committed `Cargo.lock`, so dependency resolution can drift across clean checkouts and silently exceed the declared Rust MSRV, whatever it is at the time. The lockfile is absent because the owner's machine-global gitignore excludes `Cargo.lock`, not by project intent.

### Impact

Unreproducible builds for any second consumer of the repo; MSRV claims unverifiable.

### Required Actions

1. Owner decision: add a repo-local `!Cargo.lock` un-ignore (and `git add -f Cargo.lock`) overriding the global policy, or record that this repo intentionally floats dependencies. — **Owner decision 2026-08-05 (Track C): pin, do not lock.** This issue **stays OPEN**; see the note below for what the decision costs. — **Reversed and done 2026-08-06 (`6cf4164`): the un-ignore, not the floating-dependency record.** The 2026-08-05 sentence stands as that day's decision; the 2026-08-06 note below is what replaced it.

**Note (2026-08-05, ADR-0019 / DCR-0020) — the risk is now concrete and sits on a build path.** Track C added `wasm-bindgen = "=0.2.126"` as an **exact** pin, because wasm-bindgen requires crate↔CLI version identity: a resolution drift there does not degrade, it breaks the build outright, and the repair (`cargo install wasm-bindgen-cli@<v>` from source) needs a network this machine measured as restricted. Three facts sharpen the original entry:

- **The binding constraint was not where it was expected.** The pre-implementation analysis assumed `slug → comrak` (a wasm32-side edge) held wasm-bindgen down. It declares only a caret `"0.2"` and constrains nothing. The real edge is **host-side**: `reqwest → js-sys` inside `transync-openai`, where `js-sys 0.3.97` **exact-pins** `wasm-bindgen = "=0.2.120"`. Because cargo resolves one wasm-bindgen for the whole workspace lock, a host-side pin was holding the *wasm32* tree down.
- **Reaching 0.2.126 needed a seven-package update**, not the one-liner assumed: `wasm-bindgen` + macro/macro-support/shared, `js-sys` 0.3.97→0.3.103, `web-sys` 0.3.97→0.3.103, `wasm-bindgen-futures` 0.4.70→0.4.76. `reqwest` itself did not move and every `transync-openai` test stayed green, so the bump is behaviorally inert — but **that resolution exists only in the gitignored `Cargo.lock`**.
- **The failure mode on a clean checkout.** A future wasm-bindgen release moves fresh `js-sys` resolutions; a clean checkout can then land on a `js-sys` whose own exact pin **conflicts with our `=0.2.126`** — a hard resolver failure originating in a `reqwest` bump that looks entirely unrelated to wasm. Recovery is a matching `-p js-sys` update, or finally committing the lock.

**Note (2026-08-06, commit `6cf4164`) — the lock is committed; the pin is kept anyway.** The owner took the recovery the 2026-08-05 note names last — "or finally committing the lock" — reversing the previous day's decision: `.gitignore` carries a repo-local `!Cargo.lock` un-ignore (its comment names this issue) and the lockfile is in the tree. The seven-package resolution the 2026-08-05 note says "exists only in the gitignored `Cargo.lock`" is now a tracked file — `wasm-bindgen 0.2.126`, `js-sys 0.3.103`, `web-sys 0.3.103`, `wasm-bindgen-futures 0.4.76` are all readable in it — so the clean-checkout failure mode no longer has to be survived by luck. The **Impact** above is answered on both counts: a second consumer of the repo resolves what this machine resolved, and the MSRV claim is now checkable rather than asserted (the root manifest's floor argument turns on `lol_html 2.9.0` by version, and that version is pinned in the tree).

**The `=0.2.126` pin is not made redundant by the lock, and must not be loosened.** A lockfile binds only the resolves that read it: `cargo update`, a regenerated lockfile, and any build after the lock is deleted all re-resolve from the manifests, where a caret range would float straight past the CLI. And wasm-bindgen's requirement is crate↔**CLI** identity — `wasm-bindgen-cli` is a host tool installed outside the workspace, so no lockfile can pin that half at all. The pin holds the crate side still so the host side can be matched; the lock records a resolution that satisfies the pin. The root `Cargo.toml` says the same beside the pin, and `docs/project/release-checklist.md` step 18 owns the release-time consequence (a version bump rewrites the workspace entries in the lock, and that diff belongs in the release-prep commit).

**What survives the fix, stated rather than left implied.**

- **The `CLAUDE.md` half was declined, not fixed.** The same machine-global gitignore that hid `Cargo.lock` also hides this repo's `CLAUDE.md`, and the owner **declined** that half on 2026-08-06 — recorded in `6cf4164`'s commit message and in the `.gitignore` comment beside the un-ignore. `CLAUDE.md` stays untracked **by choice**, so a clean checkout does not get it. This is outside the filed scope above (the Required Actions are about the lockfile), and it is named here so the surviving half of the original gitignore concern lives in the record rather than only in a commit message.
- **Nothing gates the lock's freshness.** No check runs `--locked` — neither the pre-commit hook nor `scripts/smoke.sh` does — and cargo rewrites the lockfile in place, so a manifest change shows up as an uncommitted `Cargo.lock` diff rather than as a failure. Keeping the committed lock current is a human step (release-checklist step 18), not a gate.

### Related

- Workspace `rust-version` (MSRV 1.88 since 2026-08-07, ticket `23e76a`; 1.85 when this issue was filed)
- **ADR-0019** (decision 2 — pin over lock, with this failure mode recorded); its pin-over-lock half is **superseded** — see ADR-0019's 2026-08-07 amendment / **DCR-0020**
- `.gitignore` (the `!Cargo.lock` un-ignore and the owner decision in its comment) and the root `Cargo.toml` comment beside `wasm-bindgen = "=0.2.126"`
- `docs/project/release-checklist.md` step 18 (the lock diff belongs in the release-prep commit; the pin must not be loosened for a release)
- `docs/backlog.md` (the 2026-08-06 decision line, including the declined `CLAUDE.md` half)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-07-13 — the blockquote fingerprint recurses one level; the deeper residual is tracked in `docs/backlog.md`.

## OI-0022: Structural-fingerprint depth residuals

- **Source:** R0003-0033 + R0003-0037 + R0003-0038 (Review 0003) — consolidated by archive cleanup (review archived and removed)
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — archive-cleanup consolidation)
- **Status:** RESOLVED (2026-07-13)
- **Resolution:** EXT-2026-07 P2-9 — `Preserved` results are byte-compared against the source (mismatch = retryable rejection); `ListTopologyEntry` fingerprints ordered-list marker facts (`start`, `delimiter`, `tight`); blockquote fingerprints recurse one level (nested list marker facts + item markers, nested quote child kinds). The renderer also preserves `<ol start>` (DCR-0007 limitation closed).

### Problem

Residual depth gaps in per-kind validation after the R0008-0014/0015 fingerprint extensions:

1. **`Preserved` is not proven** (R0003-0033): a provider returning `output_kind: preserved` is accepted without checking `translated_payload == source_payload`, so modified text can masquerade as preserved.
2. **List fingerprint omits marker details** (R0003-0037): ordered-list start numbers, delimiter style (`.` vs `)`), and loose/tight spacing are not fingerprinted; `(depth, ordered, task, child_kinds)` is checked.
3. **Blockquote fingerprint is direct-children-only** (R0003-0038): nested list topology or heading levels *inside* a blockquote are unchecked below the first level.

### Impact

A misbehaving provider can alter these sub-block properties without tripping validation. Rendering stays safe (DOMPurify + reparse layers); the risk is silent structural drift within a block.

### Required Actions

1. Byte-compare `Preserved` payloads against the source and downgrade mismatches (cheap, highest value).
2. Extend `ListTopologyEntry` with start/delimiter/looseness if drift is observed in practice.
3. Recurse the blockquote fingerprint one level (or reuse the list fingerprint inside quotes) if drift is observed in practice.

### Verification

- [ ] `preserved`-labeled modified payload is rejected by a new test
- [ ] Marker/nesting extensions covered by per_kind tests when implemented

### Related

- R0008-0014 / R0008-0015 (child-kind fingerprints, fixed 2026-07-11)
- ADR-0012 (inline content stays LLM-owned — this OI is about *structural* sub-block properties)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-09 (ticket `d3acc3`) — the forward-drift warning landed 2026-07-14 and the reflow-recompute hooks landed here.

## OI-0024: Browser sync residuals (reflow hooks, version-drift warning)

- **Source:** R0003-0044 (Review 0003); R0002-0090 (Review 0002) — consolidated by archive cleanup (review archived and removed)
- **Date:** 2026-07-11
- **Decision:** ACCEPT (track — archive-cleanup consolidation)
- **Status:** **RESOLVED (2026-08-09, ticket `d3acc3`)** — item 2 had already landed 2026-07-14; item 1 closes here
- **Partial resolution (2026-07-14, OI-0024 item 2):** `loadAlignment` now emits a `console.warn` when a same-major map carries a newer minor/patch than the engine's `KNOWN_SCHEMA` (`1.0.0`) — forward-compat drift is accepted but visible, replacing the silent `console.debug`. Byte-mirrored to the CLI copy (`sync_js_drift` green) and pinned by Playwright test `g` (`web/tests/scn13.spec.js`). Item 1 (reflow-recalculation hooks) remains deferred into OI-0015's module-split pass.
- **Resolution (2026-08-09, OI-0024 item 1):** `mountSync` observes three reflow signals — a `ResizeObserver` on **both** panes, `document.fonts.ready`, and `load`/`error` on `<img>` elements inside either pane (capture phase; neither event bubbles) — and on any of them re-collects both anchor sets and re-runs the *last driving* pane's scroll handler, coalesced into a single animation frame. That does both halves of the item: the cached block lists are replaced rather than merely re-measured, and the follower is put back under the reader instead of waiting for the next scroll event. The caller-driven destroy-and-remount the docstring used to demand is now needed only for an actual HTML **replacement**, which is not a reflow; `controller.refresh()` requests the same recompute for a layout change no observer reports. Byte-mirrored to the CLI copy (`sync_js_drift` green) and pinned by Playwright `engine.spec.js` tests `h` (resize re-drive + anchor-cache refresh) and `i` (image-load re-drive).
- **Follow-on (2026-08-12, R0004-0087):** a fourth signal joined the three above — a `<details>` toggle inside either pane, raised by the toggle mirror itself. It is the one *content* reflow the engine causes (mirroring a disclosure reflows the partner pane) and the pane box never changes, so none of the three original observers reported it and the follower waited for the next scroll. Pinned by `engine.spec.js` test `j`, and byte-mirrored to the CLI copy as always. This does not reopen the issue: item 1 was closed by the recompute mechanism, and this adds an entry point to it.

### Problem

1. ~~**No reflow recalculation hooks**~~ (R0003-0044) — **RESOLVED 2026-08-09:** `sync.js` wires scroll/wheel/touch/pointer/key listeners but no `ResizeObserver`, `document.fonts.ready`, or image-load hooks. Per-frame geometry reads mitigate most drift, but cached block lists can go stale across major reflows (the docstring pushes destroy-and-remount onto the caller). (The Playwright suite's major-reflow test, 2026-07-13, pins the current per-frame-geometry behavior so a regression here is now caught.)
2. ~~**Silent minor/patch acceptance**~~ (R0002-0090) — **RESOLVED 2026-07-14:** `loadAlignment` warns (`console.warn`) on a newer-minor/patch, same-major map instead of the silent `console.debug`; still accepted (forward-compat).

### Impact

Cosmetic-to-moderate UX: sync accuracy can degrade after dynamic reflows until remount; forward-schema drift is invisible to integrators.

### Required Actions

1. ~~Evaluate a `ResizeObserver`-driven cache refresh when OI-0015's module split happens (same file, same pass).~~ — **DONE 2026-08-09**, and it went past "evaluate": all three hooks are built, and the recompute re-drives as well as re-collects.
2. ~~Emit `console.warn` when minor/patch is newer than the engine's known version.~~ — **DONE 2026-07-14.**

### Verification

- [x] Code change applied — `web/js/sync.js` + the byte-identical CLI twin, one commit.
- [x] Tests pass — Playwright `engine.spec.js` `h` and `i`; `sync_js_drift.rs` green.
- [x] No regressions observed — full browser suite green, including the 2026-07-13 major-reflow test that pins the live-geometry behavior this builds on.

### Related

- OI-0015 (sync-module separation of concerns — action 1 folded into that pass, and both closed there 2026-08-09)
- OI-0016 (per-frame scan perf — an offset cache would now hang off this recompute)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-03 (DCR-0014) — the auto-extracted candidate-glossary preflight shipped.

## OI-0026: No cross-batch terminology consistency mechanism

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — owner decision on mechanism pending)
- **Status:** RESOLVED (2026-08-03)
- **Resolution:** OI-2026-08 wave / DCR-0014 Part A — owner chose mechanism (a), the **auto-extracted candidate glossary**. A defaulted `Translator::extract_glossary` preflight (`Ok(None)` = unsupported) runs once per run after ID assignment; `profile::merge_auto_glossary` merges the harvest into the run's glossary with **static entries winning**, scope forced to `GlobalAcrossDocument` (ADR-0014), a 24-term cap (`DEFAULT_MAX_AUTO_GLOSSARY_TERMS`), and sanitize bounds (80/200/200 chars) that act as the invariant-7 prompt-stuffing guard. A `Cow<TranslateOptions>` shadow in `run_pipeline` makes prompts, the `TranslationBatch.glossary` wire field, **and** cache identity derive from one effective profile, so the merged terms enter `CacheKey` twice through the existing `glossary_hash` + `profile_prompt_hash` — **no `CacheKey` or `Cache`-trait change, no `VALIDATION_SCHEMA_VERSION` bump**. Extraction failure/unsupported/empty-document all degrade to static-only (explicitly *not* an ADR-0017 terminal event) and are key-identical to an opted-out run. Default **OFF**: resolution flag > profile `auto_glossary` > `false`, with CLI `--auto-glossary` / `--no-auto-glossary`. Outcome rides on `ValidationReport.auto_glossary: Option<AutoGlossaryReport>` (absent when off, so opted-out report JSON is byte-identical). First-occurrence pinning was evaluated and rejected — it is undefined under concurrent dispatch, corrupts cache identity, breaks ADR-0009's verbatim-retry comparability, and has nothing to harvest resolutions from. Tests: `auto_glossary_merges_into_prompts_and_batches`, `auto_glossary_static_entry_wins_on_conflict`, `auto_glossary_extraction_failure_degrades_not_aborts`, `auto_glossary_unsupported_default_is_reported`, `auto_glossary_changes_cache_identity`, `auto_glossary_disabled_makes_no_extraction_call`, `auto_glossary_skips_document_without_translatable_blocks`, `merge_auto_glossary_caps_dedupes_sanitizes_and_forces_scope`, `report_json_omits_auto_glossary_when_absent`.
- **Follow-up (ti `dca5bf`, 2026-08-10):** the "no `Cache`-trait change" above described *this* design and stays the right description of it — the merged terms still enter `CacheKey` only through `glossary_hash` + `profile_prompt_hash`. What changed since is one level up: the preflight *call* is now itself cacheable, through two more defaulted `Cache` methods and a `GlossaryExtractionKey` keyed on the assembled extraction prompt's bytes, so a fully-cache-hit enabled run makes zero provider calls instead of one. `VALIDATION_SCHEMA_VERSION` still does not move, and a replayed harvest re-runs `merge_auto_glossary` against the run's own profile, so every rule above holds unchanged. See contracts.md §5a *The cached glossary preflight* and DCR-0028's 2026-08-10 appended note.

### Problem

Concurrent batches share no runtime state: each is translated independently, so a term the provider renders one way in batch A can be rendered differently in batch B. The static profile glossary is the *only* consistency mechanism, and it only covers terms the author knew to list up front. On a document with ~8 blocks per batch, a term first seen in batch 2 can drift by batch 3 with nothing to pull it back.

### Impact

Terminology inconsistency in the translated output for any term not in the profile glossary — a quality defect, not a correctness or structural one. Grows with document length and concurrency.

### Required Actions

1. ~~Owner decision on the mechanism: auto-extracted candidate glossary (first pass harvests salient terms, second pass pins them), first-occurrence pinning (thread earlier resolutions forward as hints), or accept-with-docs (declare the profile glossary the sole supported mechanism and document the limitation).~~ — **DONE 2026-08-03:** mechanism (a) shipped; (b) rejected with reasons recorded in DCR-0014; (c) ruled out by owner instruction.

### Verification

- [x] Every batch of a multi-batch run carries the same merged glossary in prompt and wire field (`auto_glossary_merges_into_prompts_and_batches`, 2026-08-03)
- [x] Static profile entries win on source-term conflict; the extractor's variant never reaches the prompt (`auto_glossary_static_entry_wins_on_conflict`, 2026-08-03)
- [x] Extraction failure / unsupported translator completes the run with a recorded status and static-glossary output (`auto_glossary_extraction_failure_degrades_not_aborts`, `auto_glossary_unsupported_default_is_reported`, 2026-08-03)
- [x] Differing effective glossaries do not share cache entries (`auto_glossary_changes_cache_identity`, 2026-08-03)
- [x] Default-off runs make no extraction call and keep the pre-wave cache key + report JSON (`auto_glossary_disabled_makes_no_extraction_call`, `report_json_omits_auto_glossary_when_absent`, 2026-08-03)

### Related

- ADR-0014 (glossary scope) — the static-glossary surface this extends; upheld by forcing extracted entries to global scope
- ADR-0017 (batch-terminal abort) — deliberately *not* engaged: a failed extraction degrades, it does not abort
- DCR-0014 (auto-glossary preflight + batch-fault fairness) — the resolving record
- Invariant 2 (LLM owns content decisions)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-04 (DCR-0018) — the public-surface curation shipped and closed the next breaking window.

## OI-0027: Public-surface hardening for the next breaking window

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — do at the next sanctioned breaking window); **owner decision 2026-08-03:** the window is the still-open 0.2.0 release — execute as the window-closing step (see Owner decision below)
- **Status:** **RESOLVED (2026-08-04)**
- **Resolution:** **DCR-0018** — public-surface curation, implementing the owner-approved spec `docs/superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md` against a 13-task plan (amended in flight, `f1c714f`).
  - **The facade is an explicit list and the boundary is compiler-enforced.** `crates/transync/src/lib.rs` dropped `pub use transync_core::*;` and `pub use transync_syntax;` for a curated cluster-grouped re-export list, and the facade's `transync-syntax` dependency went with them. `transync-core`'s tier-(c) modules (`batch`, `error`, `pipeline`, `validate`) and the five re-exported syntax aliases (`align`, `id`, `parser`, `regen`, `render`) are `pub(crate)`; `cache`, `llm`, and `profile` stay `pub` as tier (a)/(b). **One exception:** `unit` is `#[doc(hidden)] pub` because `transync-openai`'s offline `live_smoke` fixture-shape test reaches it cross-crate — that reach is now declared as engine **dev-dependencies** rather than borrowed from the facade.
  - **`#[non_exhaustive]` on nine items** (workspace count 2 → 11): the three error enums (`TranslatorError`, `TransyncError`, `ParseError`), the four profile structs (`ProfileMetadata`, `ProfileConstraints`, `ProfileBatching`, `ProfileRender`), `TranslateOptions`, and `TranslationUnit`. Construction is **default-then-assign** everywhere except `TranslationUnit`, which got `TranslationUnit::new(unit_id, block_kind, input_mode, source_payload, source_hash)` + `with_context`/`with_constraints`/`with_batch_id`/`with_retry` (`Default` is meaningless for it, and `source_hash` is a `CacheKey` axis with no honest zero). The exhaustive-by-policy set — `UnitResult`, `TranslationBatchResult`, `TranslationBatch`, `GlossaryEntry` — is stated in contracts.md §1 and unchanged.
  - **`translate_with_cache` is the promoted entry point.** `pipeline::run_pipeline` is unreachable; `scn_10` moved onto the facade path. Not a bare alias: `translate_with_cache` preflights `opts.target_language` and fails fast on an empty value.
  - **The drift gate exists and welds three artifacts.** `contracts.md` gained `## 0. Public Rust surface` — a 78-row table that **is** the surface — and `crates/transync/tests/public_surface.rs` welds it to the code in six parts (compile-assertion re-export module, `DOCUMENTED` mirror, table ↔ `DOCUMENTED` scrape equality, `first_class` ↔ `DOCUMENTED` self-scrape equality, segment-based negative test over the hidden module names, and a DCR-0017 **M6** `generator.version` guard). Any one of the three going out of step with another is a red test or a red compile; a `pub use` added to the facade's `lib.rs` **alone** is the one direction still unchecked, deliberately, pending a rustdoc-JSON diff. §1 additionally pins the seven-code `stable_code()` vocabulary as an append-only inter-process contract.
    - **Update (2026-08-06, backlog `public-surface-widening-gate`):** that last direction is no longer unchecked. A **fourth** artifact was welded in — `lib_rs_exports_nothing_the_documented_list_omits` scrapes `crates/transync/src/lib.rs`, resolves each re-export to the name it publishes, and asserts every one carries a §0 row (feature-gated `transync::test_stub` excepted, via a named `FEATURE_GATED` constant). Dependency-free by design, as the OI-0027 spec required; it panics on any top-level `pub` form it cannot read rather than skipping it, and asserts the reverse inclusion so a blind scrape cannot pass vacuously. Only surface reaching consumers *without being named in `lib.rs`* is still rustdoc-JSON territory. See DCR-0018's 2026-08-06 addendum.
  - **DCR-0017's handover list is closed.** The three narrowing candidates (`htmlseg::balance_fragment`, `outcome::is_translatable`, `walk::label_for`) are `pub(crate)`, and `cargo doc --no-deps` is warning-free for all three library crates — kept true by a **standing** `RUSTDOCFLAGS="-D warnings"` gate in `scripts/smoke.sh`.
  - **Owner-decision corrections recorded in DCR-0018.** (a) The 2026-08-03 decision's parenthetical "construction via `..Default::default()`" was **unsound** — functional-record update is illegal cross-crate on a `#[non_exhaustive]` struct; the owner re-decided **default-then-assign** on 2026-08-04. (b) Tier (a)'s "`TranslateOutput`" is a naming slip for **`TranslationOutput`**; no type was renamed.
  - **Sibling repo migrated the same day:** `/Volumes/Common/QJoon/resp-translator` commit `8b6dbd5` (branch `main`) — `run_pipeline` → `translate_with_cache` plus default-then-assign construction. **Behavioral delta:** an empty profile `target_language` now fails fast with `stable_code` `"internal"`.
  - **Closure fallout, resolved with zero test removals and zero new `#[allow]`s:** `pipeline::retry`'s three no-op hooks (`retry_validation`, `oversize_split`, `fallback_to_source`), `batch::group_by_unit_count` (and the pre-existing `#[allow(dead_code)]` that hid it), and `ValidatedBatch::batch_id_str` were **deleted** (greppable tombstone comment left behind); `batch::estimate_unit_tokens` and `validate::schema::check_schema` became `#[cfg(test)]`. **OI-0008 still owns the retry/fallback redesign** — deleting empty hooks is not the policy fix.
  - **Discovery:** the tracked pre-commit hook was **not live** — `core.hooksPath` was unset, so only `.git/hooks/pre-commit` ran. `scripts/install-hooks.sh` set `core.hooksPath=scripts/hooks`; the `.git/hooks` copy is now inert but kept byte-identical.
  - **Invariants held:** alignment schema stays `1.2.0`, `VALIDATION_SCHEMA_VERSION` stays `2`, `CacheKey` and both `sync.js` copies untouched.

### Problem

Three coupled public-API-hygiene gaps, all cheapest to fix inside a breaking window:

1. **The facade is a blanket re-export.** `crates/transync/src/lib.rs` is `pub use transync_core::*` over an all-`pub` core, so *item-level* breakage in core leaks straight through the semver firewall the facade is supposed to be. Curate the re-export set, and decide explicitly whether `transync::pipeline::run_pipeline`, `transync::profile::*`, and similar internals are supported API or accidental surface.
2. **`TranslatorError` / `TransyncError` are not `#[non_exhaustive]`** while contracts.md §1 promised "variant additions are non-breaking." Strictly per cargo semver, adding a variant to a plain enum is breaking. The contract text was corrected this wave (see DR-2026-07 item 5 / contracts.md §1); the *code* still needs `#[non_exhaustive]` to make the additive promise true.
3. **`TranslateOptions` / `TranslationUnit` are exhaustive by policy** (external mock-translator tests construct them), so every new field costs a migration. Decide whether that policy is worth its recurring cost or whether these should become `#[non_exhaustive]` with a builder.

### Impact

Every future field/variant addition is a breaking change or a leak; the firewall under-delivers. No runtime defect today.

### Required Actions

1. ~~Curate the facade re-exports; document the supported-API boundary.~~ — **DONE 2026-08-04:** explicit re-export list in `crates/transync/src/lib.rs`, documented as contracts.md §0 (78 rows) and pinned bidirectionally by `crates/transync/tests/public_surface.rs`. Engine modules are `pub(crate)`, so the boundary is a compiler fact, not a doc claim.
2. ~~Add `#[non_exhaustive]` to `TranslatorError` / `TransyncError` (matches the corrected contract §1).~~ — **DONE 2026-08-04:** both, plus `ParseError`, so §1's additive-variant promise is true for every error enum the facade exposes.
3. ~~Owner decision on `TranslateOptions` / `TranslationUnit` exhaustiveness vs a builder.~~ — **DONE 2026-08-04:** both become `#[non_exhaustive]`. `TranslateOptions` (and the four profile structs) construct by default-then-assign — **not** `..Default::default()`, which is illegal cross-crate on a `#[non_exhaustive]` struct; `TranslationUnit` gets `new` + `with_*` because `Default` would give it a zero `source_hash` and silently alias cache entries.

### Verification

- [x] The facade re-exports an explicit curated list; `pub use transync_core::*;`, `pub use transync_syntax;`, and the facade's `transync-syntax` dependency are all gone (2026-08-04)
- [x] `#[non_exhaustive]` present on the three error enums, the four profile structs, `TranslateOptions`, and `TranslationUnit` — workspace count 2 → 11 (2026-08-04)
- [x] `crates/transync/tests/public_surface.rs` welds **three** artifacts to each other — contracts.md §0, the test's `DOCUMENTED` constant, and the test's `first_class` compile assertion — so any one drifting from another is a red test (the two scrape-diffs) or a red compile (a §0 row the facade does not export). It also keeps the tier-(c) module names out of §0 by path segment, and pins DCR-0017 M6 (2026-08-04). **Not** covered, by design: a `pub use` added to `crates/transync/src/lib.rs` alone still widens the surface silently — nothing enumerates the facade's exports mechanically. That last direction is human-enforced by the three-place editing obligation plus a manual pre-release read-through of `lib.rs`, and its eventual mechanical answer is a rustdoc-JSON diff (`cargo public-api` or equivalent). **Superseded 2026-08-06** — a fourth scrape now welds `lib.rs` in too; see the Resolution update above.
- [x] `cargo doc --no-deps` emits **zero** warnings for `transync-syntax`, `transync-core`, and `transync`, kept true by the standing `RUSTDOCFLAGS="-D warnings"` line in `scripts/smoke.sh` (2026-08-04)
- [x] Sibling repo `resp-translator` migrated and green — commit `8b6dbd5`, with the empty-`target_language` behavioral delta recorded (2026-08-04)
- [x] Full gate set green: fmt, `clippy --all-targets -D warnings`, `cargo test --workspace -- --test-threads=4` (**340 passed / 0 failed / 3 ignored**), the wasm32 check, `scripts/smoke.sh`, and the Playwright SCN-13 suite 8/8 (2026-08-04)

### Owner decision (2026-08-03)

The owner accepted the curation proposal from the 2026-08-03 design discussion in full:

*(Two statements below were corrected during execution — see the Resolution block above: `TranslateOutput` in item 1 is a naming slip for `TranslationOutput`, and item 3's "construction via `..Default::default()`" was unsound and was re-decided as default-then-assign on 2026-08-04. The text is left as decided, with the corrections recorded rather than silently rewritten.)*

1. **Three-tier boundary.** (a) *First-class supported API:* `translate()` + `TranslateOptions`/`TranslateOutput`, the `Translator` contract types (`TranslationBatch`, `UnitResult`, `OutputKind`, `TranslatorError`, `RetryContext`, `GlossaryEntry`, `ProviderFingerprint`, `TokenizerHint`), the `Cache` trait + `CacheKey`/`CacheError`/`InMemoryCache`, `TransyncError`, the alignment-map wire types, and the `ValidationReport` family. (b) *Provider-SDK layer*, deliberately public and documented as the provider-implementor surface: `llm::prompt` (build/parse/schema) and `profile::render_prompt_body` — with an explicit doc note that prompt *text* is not a contract (only schema/parse shape is, per contracts.md §1). (c) *Hidden:* `parser`/`id`/`unit`/`batch`/`regen`/`render`/`validate` internals and `pipeline::run_pipeline`.
2. **`translate_with_cache(source, opts, translator, cache)` is promoted to the facade** as first-class API (long-lived cache sharing is a legitimate external use case and the OI-0017 disk-cache entry point); scenario tests move onto it, preserving DCR-0005's "verify the public surface through the facade" value.
3. **`#[non_exhaustive]`** on `TranslatorError`/`TransyncError`/`ParseError` and on `TranslateOptions` (construction via `..Default::default()`); `TranslationUnit` gets a builder/constructor helper instead (`Default` is meaningless for it — required fields are its essence).
4. A **surface-drift test** (facade re-export list ↔ contracts.md public-surface documentation) lands with the curation.

**Sequencing (owner-approved):** HTML-content translation feature (the last major surface addition; **LANDED 2026-08-04** — ADR-0018 / DCR-0016) → OI-0028 Option B crate split (which shakes the surface again) → this curation → **0.2.0 release**. The hardening is the window-closing ritual; a 0.3 breaking window becomes unnecessary.

### Related

- **DCR-0018** (the resolving record — closure map, `#[non_exhaustive]` set, migration list, drift gate, gates)
- DCR-0005 (facade + core split — the firewall this hardens; gained a dated note 2026-08-04: the firewall is now *curated*)
- contracts.md §0 (the surface table this wave created) and §1 (Translator trait, corrected in DR-2026-07, extended here)
- OI-0028 (**RESOLVED 2026-08-04** — its crate split landed first and handed forward the widened-surface inventory this wave closed)
- OI-0008 (retry/fallback policy redesign — **still open**; this wave deleted the three empty `pipeline::retry` hooks but changed no policy)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-04 (DCR-0017) — the `transync-syntax` split gave the render path a standing wasm32 compile gate.

## OI-0028: WASM render path has no compile path

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — schedule with Track C / dialect work); **owner decision 2026-08-03:** Track C (in-browser WASM rendering) is confirmed as a real goal — execute **Option B (crate split)** inside the open 0.2.0 window (see Owner decision below)
- **Status:** **RESOLVED (2026-08-04)**
- **Resolution:** **DCR-0017** — `transync-syntax` crate split + AST-direct renderer, implementing the owner-approved spec `docs/superpowers/specs/2026-08-04-transync-syntax-split-design.md` (v2) against a 9-task plan.
  - **The compile path exists and is enforced.** `crates/transync-syntax` is the workspace's fifth member and owns `parser` (+`ranges`/`refdefs`), `id`, `regen`, `render` (+`attrs`), `align`, `htmlseg`, plus the two modules the split created — `outcome` (the relocated `HtmlOutcome` closure) and `walk` (the shared top-level normalization) — and `error::ParseError`. `transync-core` keeps `pipeline`/`unit`/`batch`/`llm`/`validate`/`cache`/`profile`. It declares **no `[features]`** (a feature there would reintroduce the unification trap Option B exists to avoid) and **no core-directed dev-dependency** (that would force the gate down to `--lib` and lose test-target coverage).
  - **Standing gate.** `cargo check -p transync-syntax --target wasm32-unknown-unknown` runs in **both** pre-commit hook copies (`scripts/hooks/pre-commit`, tracked; `.git/hooks/pre-commit`, the active untracked copy) and in `scripts/smoke.sh`. The hook **fails loudly** with a `rustup target add wasm32-unknown-unknown` hint when the target is missing rather than silently skipping; `smoke.sh` runs it unconditionally. This gate — not the canary below — is what keeps the property true as manifests float.
  - **Canary re-run (2026-08-04, the split's first gate, per the validity rule below).** The full dependency set compiles for `wasm32-unknown-unknown`: comrak **0.27.0**, serde **1.0.228**, serde_json **1.0.149**, siphasher **1.0.2**, thiserror **1.0.69**, lol_html **2.9.0**, htmlize **1.1.0**. Compile-only; nothing executed on a WASM host.
  - **No consumer path broke.** `transync-core` re-exports the moved modules, so `transync::parser::parse`, `transync::id::BlockId`, `transync::align::ALIGNMENT_SCHEMA_VERSION`, `transync::error::ParseError`, `transync::unit::html_outcomes`, and the root aliases all still resolve. The facade also gained `pub use transync_syntax;`. The intentional signature breaks (`regen::regenerate`, `align::build_alignment_map`) are enumerated in DCR-0017's migration list and ride the open 0.2.0 window.
  - **The renderer rework rode along as designed**, clearing OI-0008's two renderer items (see that entry) and fixing a latent indented-code-block bug. Accepted byte-shape changes (loose lists in both panes, loose task items) are recorded in DCR-0017.
  - **Records:** ADR-0003 dated amendment (five members; `transync-core → transync-syntax`), ADR-0006 / ADR-0007 dated mechanism amendments, DCR-0005 / DCR-0007 dated notes, ADR-0018 path note. **The dialect trait was deliberately NOT instantiated** — DCR-0005's deferral condition (a real second dialect supplying constraints) is still unmet; this split created the crate boundary only.
  - **Handed forward:** the widened-surface inventory (the `#[doc(hidden)]` items the crate boundary forced public, three narrowing candidates — `htmlseg::balance_fragment`, `outcome::is_translatable`, `walk::label_for` — and six rustdoc warnings where public docs link private siblings) goes to **OI-0027**, which is the next step. **OI-0033** was filed from the Guard-1 residual probe.

**Note (2026-08-05) — Track C proper has landed; this issue stays RESOLVED.**
Required Action 2 recorded that "Track C proper (wasm-bindgen entry points, JS
integration, browser demo) is separate post-0.2.0 work". That work is now
shipped, in **ADR-0019 / DCR-0020** (six commits, `94d7e39`..`1d04135`), and
the deferral clause is discharged rather than reopened — this issue's scope was
the *compile path*, which has been true and gated since 2026-08-04.

- **The gate widened rather than moved.** Both pre-commit hook copies and
  `scripts/smoke.sh` now run
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`.
  The hook keeps its loud rustup-target failure and stays rustup-only — the new
  host prerequisites (wasm-pack, binaryen ≥ 121) live in `scripts/smoke.sh` via
  `scripts/build-wasm.sh`, which also fails loudly, never skips.
- **`crates/transync-syntax` is untouched by the wave** — still no
  `[features]`, still no `transync-core` dependency, and
  `git diff 94d7e39..HEAD -- crates/transync-syntax` is empty. The wasm-bindgen
  entry points live in a **sixth** member, `crates/transync-wasm`, which
  depends on `transync-syntax` alone (charter, plus the `getrandom` trap in
  core's `wasm32` tree).
- **The compile path is now an executed path, not only a compiled one.** The
  browser renders both panes from the same Rust renderer, byte-identically to
  the CLI's fragments — pinned as a standing Playwright gate — where every
  claim before this wave was compile-only.
- **The canary-validity rule below is superseded in practice for this crate
  set.** The gate now covers both crates on every commit, so resolver drift
  surfaces at the hook rather than at a re-run canary.

### Problem

The architecture reserves a WASM rendering path (CLAUDE.md tech-stack layering; Track C), but `transync-core` cannot compile to `wasm32` today: it unconditionally inherits `tokio` with `rt-multi-thread` (a `compile_error!` on `wasm32`) and pulls in `tiktoken-rs` for a single backoff-sleep path. Rendering does not need either. There is no feature gate that lets the renderer build without the runtime.

### Impact

The reserved WASM-in-browser rendering option is not actually reachable; a second Markdown parser in JS (which the invariants forbid) would be the only alternative if a browser renderer were needed.

### Required Actions

1. ~~Feature-gate the runtime/tokenizer dependencies (or split a render-only crate) so the renderer compiles to `wasm32`.~~ — **DONE 2026-08-04:** split, not feature-gated (Option B). `crates/transync-syntax`.
2. ~~Schedule with Track C / dialect work; the renderer's per-block-reparse and `strip_outer_wrapper` rework (OI-0008 items) ride the same effort and should land together.~~ — **DONE 2026-08-04:** both landed in the same wave (DCR-0017); OI-0008 narrowed accordingly. Track C proper (wasm-bindgen entry points, JS integration, browser demo) is separate post-0.2.0 work by owner decision — this issue was scoped to the *compile path*, which now exists.

### Verification

- [x] `cargo check -p transync-syntax --target wasm32-unknown-unknown` succeeds, and the line is present in both pre-commit hook copies plus `scripts/smoke.sh` (2026-08-04)
- [x] Hook fails loudly (does not skip) when the `wasm32-unknown-unknown` rustup target is absent (2026-08-04)
- [x] `transync-syntax` declares no `[features]` and no core-directed dev-dependency (2026-08-04)
- [x] Full pre-existing gate set green — fmt, clippy, `cargo test --workspace`, CLI stub suite, Playwright SCN-13, drift tests — at every task boundary (2026-08-04)
- [x] No downstream path regression: `boundary_v02`, `docs_index_drift`, `cli_smoke`, the SCN-01..15 scenario suites, and `transync-openai`'s `live_smoke.rs` compile and pass untouched (2026-08-04)

### Owner decision (2026-08-03)

Track C is confirmed as an actual goal, and the owner chose **Option B — crate split** over in-crate feature gates (Option A):

- A base crate (working name `transync-syntax`) takes `parser`/`id`/`regen`/`render`/`align` plus the shared IR types (`Block`, `BlockKind`, `Document`, `ByteRange`, …); `transync-core` sits on top with `pipeline`/`batch`/`llm`/`validate`/`cache`. All base-crate dependencies (comrak, serde, and — after the HTML feature — `lol_html`) are WASM-clean.
- Rationale over Option A: the boundary is **compiler-enforced** — `cargo check -p transync-syntax --target wasm32-unknown-unknown` becomes a standing gate line, with no feature-unification trap and no `#[cfg]` sprawl through mixed files — and the split is the natural landing zone for the DCR-0005-deferred dialect-trait seam.
- The **renderer rework rides the same effort** (OI-0008: per-block comrak reparse and `strip_outer_wrapper` retirement): shipping the current renderer to WASM would export the O(blocks) reparse cost to the browser, so AST-direct rendering is effectively a precondition. Three deferred debts (dialect seam, renderer rework, WASM target) share one incision line and land together.
- The facade keeps re-exporting everything at unchanged paths (curated per OI-0027), so consumers absorb the type moves without path breakage.

**Sequencing (owner-approved):** after the HTML-content translation feature, before the OI-0027 curation, inside the open 0.2.0 window: HTML feature → this split + renderer rework → OI-0027 curation → 0.2.0 release. **The HTML feature LANDED 2026-08-04** (ADR-0018 / DCR-0016), so this split is now the next step.

**Dependency-set note (2026-08-03 canary):** `lol_html` 2.9.0 + `htmlize` 1.1.0 — the two dependencies the HTML-content translation feature added to `transync-core` (ADR-0018 / DCR-0016) — **compile for `wasm32-unknown-unknown`**. This was a compile-only canary run before implementation began; nothing was executed on a WASM host, and runtime behavior is unverified. The base-crate split's dependency set therefore stays WASM-clean: `htmlseg` (and the `parser`/`id`/`regen`/`render`/`align` modules that will move with it) adds no non-WASM dependency to `transync-syntax`. **Canary validity:** the result holds for the versions the resolver picked on 2026-08-03 — `lol_html` **2.9.0** and `htmlize` **1.1.0**. The manifests float (`"2"` / `"1"`, and there is no committed lockfile per OI-0020), so a resolver bump can change the transitive set: **re-run the canary on resolver drift**, and again as the first gate of the split itself. *(Update 2026-08-07: `Cargo.lock` has been committed since `6cf4164` — OI-0020 RESOLVED — so the manifests no longer float across clean checkouts. The rule stands, but its trigger is now a deliberate lock update rather than any fresh resolve.)*

### Related

- **DCR-0017** (the resolving record — split topology, migration list, renderer rework, guards, gates)
- **ADR-0019 / DCR-0020** (2026-08-05 — Track C proper: `crates/transync-wasm`, the build pipeline, the browser demo, and the CLI-vs-browser parity gate; see the dated note above)
- OI-0008 (renderer per-block reparse; brittle `strip_outer_wrapper` — **cleared** by the same effort, 2026-08-04)
- OI-0027 (**RESOLVED 2026-08-04**, DCR-0018 — the curation ran after this split and closed the widened-surface inventory it handed forward: the three narrowings landed and the rustdoc warnings are gone, under a standing gate)
- OI-0033 (lone-CR `LineOffsets` desync — filed 2026-08-04 from this wave's Guard-1 residual probe; pre-existing and orthogonal)
- ADR-0003 (amended 2026-08-04: five members, `transync-core → transync-syntax`)
- DCR-0005 (dialect trait still deferred — this split created the boundary only)
- CLAUDE.md tech-stack layering (WASM path); Track C

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-03 (DCR-0015) — the provider-neutral prompt assembly was lifted into core.

## OI-0029: Provider #2 preparation (deduplicate provider-neutral assembly)

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — act only when a second provider lands); **deferral overridden by the owner 2026-08-03** (do it now, before provider #2)
- **Status:** RESOLVED (2026-08-03)
- **Resolution:** OI-2026-08 wave / DCR-0015 Part A. The provider-neutral assembly moved to the new `transync-core::llm::prompt` (file-as-module child of `llm`), exposing `SCHEMA_NAME`, `build_user_prompt`, `schema_object_for`, and `parse_batch_output(text, &BatchId)`; the hint/envelope structs moved **private**. `transync-openai` keeps only dispatch, transport, and the two surface wrappers — the `call_*` signatures (including `reasoning_effort`) are untouched, and request JSON, prompt text, schema JSON, and error variants/messages are byte-identical. Byte-identity is **pinned by four goldens** under `llm/prompt/golden/`, generated from the **pre-lift** `client.rs` as the implementation's first action and asserted by `golden_user_prompt_first_dispatch` / `golden_user_prompt_retry` / `golden_schema_objects`; the independent wave review rebuilt the pre-lift assembly and confirmed byte-identity. Shape issue 1 (tokenizer): new `#[non_exhaustive] llm::TokenizerHint { O200kBase, Cl100kBase }` plus a defaulted `Translator::tokenizer_hint()` (`None` = the legacy OpenAI-model-name heuristic), resolved through `batch::resolve_encoder`; `TransyncOpenAI` overrides it with rules mirroring `encoder_for`, so packing is unchanged. The hint is deliberately excluded from `fingerprint()` / `CacheKey` — batching shape never participates in content identity. Shape issue 2 (dual-homed model identity) is resolved **by documentation**: the `Translator` instance is the authority (its `fingerprint()` covers the real model/endpoint/surface) and `TranslateOptions::model_id` is demoted to an advisory tokenizer-fallback label + cache-key axis — a mismatch only *over*-distinguishes, which is the safe direction. Recorded in both rustdocs and contracts.md §1. Tests: the three golden pins, the migrated prompt/schema/parse suites, `encoder_for_hint_maps_both_variants`, `resolve_encoder_prefers_the_hint_over_the_model_name`, `resolve_encoder_falls_back_to_the_model_heuristic`, `tokenizer_hint_matches_dispatch_generations`.

### Problem

Roughly 800 lines of provider-*neutral* prompt/hint assembly live inside `transync-openai` (constraint hints, glossary rendering, retry-context framing, schema object construction). A second provider crate would duplicate all of it. Two smaller shape issues compound it: the tokenizer-selection heuristic (`encoder_for`) is OpenAI-model-shaped, and model identity is dual-homed (`opts.model_id` vs the provider's own `ModelId`).

### Impact

None today (one provider ships). Materializes as duplication + drift the moment a second provider is written.

### Required Actions

1. ~~When provider #2 lands — **not before** — lift the provider-neutral assembly into `transync-core` or a `provider-common` crate.~~ — **DONE 2026-08-03** (owner overrode the "not before" condition): lifted into `transync-core::llm::prompt`; no new crate needed, since ADR-0003's DAG already delivers core to every provider through the facade.
2. ~~At the same time, generalize the tokenizer-selection heuristic and reconcile the dual-homed model identity.~~ — **DONE 2026-08-03:** `TokenizerHint` + defaulted `Translator::tokenizer_hint()`; model identity resolved as "fingerprint = authority, `model_id` = advisory".

### Verification

- [x] Lifted prompt / schema output is byte-identical to the pre-lift provider output (four goldens generated pre-lift + independent review reconstruction, 2026-08-03)
- [x] Provider request bodies, endpoints, and envelope extraction unchanged (provider-side suites stayed in place and green, 2026-08-03)
- [x] Provider-declared tokenizer hint beats the model-name heuristic; `None` keeps the legacy path (`resolve_encoder_*`, `tokenizer_hint_matches_dispatch_generations`, 2026-08-03)
- [x] Model-identity authority documented in code and contracts.md §1 (2026-08-03)

### Related

- ADR-0002 (HTTP-free core with `Translator` trait) — upheld: the lift moves code toward the HTTP-free core
- ADR-0003 (workspace with provider crates) — upheld: no new crate; the facade already re-exports core to providers
- ADR-0010 (tokenizer panic contract) — upheld by `encoder_for_hint`
- DCR-0015 (prompt lift + tokenizer hint + RTL + live smoke) — the resolving record
- OI-0017 (cache identity design) — still owns any future `CacheKey` reshaping, which is why `model_id` was demoted rather than removed

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-03 (DCR-0015) — live-endpoint evidence became one double-gated, machine-asserted command.

## OI-0030: Live-endpoint evidence is manual-only

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — owner decision on scheduled/gated live smoke)
- **Status:** RESOLVED (2026-08-03)
- **Resolution:** OI-2026-08 wave / DCR-0015 Part C — evidence is now **automated-but-gated**: one command, machine-asserted, double-gated. `crates/transync-openai/tests/live_smoke.rs` holds two `#[ignore]`d `tokio` round-trips, one per API surface (`live_chat_surface_round_trip`, default `gpt-4o-mini` → Chat Completions; `live_responses_surface_round_trip`, default `gpt-5-mini` → Responses; both models env-overridable, `TRANSYNC_OPENAI_BASE_URL` honored). Both gates are required — the `#[ignore]` keeps them out of every default suite, and a self-skip guard demands `TRANSYNC_LIVE_SMOKE=1` **and** a non-empty `OPENAI_API_KEY`, so even a blanket `--include-ignored` run without the opt-in is a no-op skip rather than a failure or a network call. The gate decision is a pure `decide_gate` predicate unit-tested **offline** (`gate_requires_both_opt_in_and_api_key`); no `std::env::set_var` anywhere. Each test is a full `translate()` round-trip, so every validation layer is the assertion; the checks are structural (unit count, matching anchor counts, `fallback_source < total_units`, non-empty output differing from the source) to survive model nondeterminism. The fixture's shape is pinned offline by `live_source_fixture_has_the_expected_shape` at **4** units — heading + paragraph + **two** list items: the D2 design said 3, which was a design-side miscount, because the IR has no list-*container* kind (DCR-0007's leaf-block model). `scripts/smoke-live-gate.sh` (0755) is the human opt-in wrapper (`chat|responses|all`, `--test-threads=1`, refuses without a key, cross-references the interactive `smoke-live.sh`). **Scheduled** CI live smoke stays out of scope — this repo has no CI; revisit if CI ever lands. No live call was made in this wave: verification was `cargo test -p transync-openai --no-run`, the normal suite (both tests listed `ignored`), and `bash -n`.

### Problem

The only evidence the code works against a real endpoint is manual (`scripts/smoke-live*.sh`, run by hand with a key). Automated scenario coverage and the Playwright suite both run entirely against stubs. Nothing catches a regression that only shows up against a live provider (schema drift, header changes, model-behavior shifts).

### Impact

Live-only regressions surface only when someone manually runs the smoke script — potentially long after they land.

### Required Actions

1. ~~Owner decision on whether to add a scheduled or gate-triggered live smoke (cost: an API key in CI, per-run token spend) or to keep live verification manual and document it as such.~~ — **DONE 2026-08-03:** gate-triggered (double-gated, human-invoked) shipped; scheduled CI explicitly out of scope while the repo has no CI.

### Verification

- [x] Gated live tests compile and are invisible to the default suite (`cargo test -p transync-openai --no-run`; both listed `ignored`, 2026-08-03)
- [x] Gate predicate refuses to call the network without both the opt-in and a key, asserted offline (`gate_requires_both_opt_in_and_api_key`, 2026-08-03)
- [x] Fixture unit count pinned offline so a fixture edit fails locally, not mid-flight against a paid endpoint (`live_source_fixture_has_the_expected_shape`, 2026-08-03)
- [x] Wrapper script syntax-checked and executable (`bash -n scripts/smoke-live-gate.sh`, mode 0755, 2026-08-03)
- [ ] First actual live run recorded (date + models) in the CHANGELOG at the next release touching `transync-openai` / `llm::prompt` / the output schema / batching — the release-gate step, not a wave deliverable

### Related

- OI-0023 (test-coverage gaps — resolved; this is the live-endpoint remainder)
- DCR-0015 (prompt lift + tokenizer hint + RTL + live smoke) — the resolving record
- `scripts/smoke-live-gate.sh` (new, machine-asserted) and `scripts/smoke-live*.sh` (interactive browser path, unchanged)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-03 (DCR-0014) — per-batch schema-fault fairness shipped.

## OI-0031: A per-batch schema fault spends every batch-mate's per-unit retry budget

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track)
- **Status:** RESOLVED (2026-08-03)
- **Resolution:** OI-2026-08 wave / DCR-0014 Part B — a hybrid of both options the issue offered, because they answer different halves. **Salvage** (who is innocent): `validate/schema.rs` gains `SchemaClassification` + `classify()`, and `validate_batch` is now **total over the requested units** — a unit with exactly one returned result row is validated through the per-unit layers and accepted in round 1 exactly as if the batch had been clean (`ValidatedBatch.batch_fault: Option<BatchFault>` carries the attribution). **Separate budget** (who pays): requested ids missing from the result, or returned more than once (all copies discarded), become `Schema`-layer offender rows re-dispatched **verbatim** with an ADR-0009 `RetryContext`, charged to the new `TranslateOptions.max_per_batch_schema_retries` (default 2, **charged once per round** rather than per offender, library-only — symmetric with `max_per_unit_validation_retries`, since the CLI exposes batching/output knobs, not retry-policy internals). Bookkeeping splits into `dispatch_counter` (attempt numbering / `RetryContext.attempt`), `unit_fault_counter` (per-unit content budget) and `batch_fault_rounds`, so the two budgets drain independently and pre-wave histories are numerically unchanged. Foreign (unrequested) ids are discarded and **charged to nobody**. Duplicate ids in the *request* are a caller-side contract violation `build_batches` cannot produce, so `process_one_batch` preflights them into a loud terminal `TransyncError::Validation` instead of silently burning everyone's budget. Reporting: `AttemptOutcome.batch_fault: bool` (skip-serialized when false, so pre-wave report JSON is byte-identical) and `ValidationReport.batch_schema_faults: u32`. Termination is provably bounded: rounds per input batch `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`. ADR-0009 gained a dated amendment; ADR-0017 is untouched (schema faults are validation-layer events, not provider-terminal errors). Tests: `batch_fault_spares_innocent_units` (fails under the old blanket rejection), `fully_hostile_empty_result_terminates`, `duplicate_result_id_charges_only_the_duplicated_unit`, `foreign_ids_are_discarded_without_charges`, `mixed_batch_and_unit_faults_drain_separate_budgets`, `offender_keeps_full_unit_budget_after_batch_fault`, `recovering_offender_ends_translated`, `request_duplicate_ids_abort_loudly`, `attempt_outcome_batch_fault_is_skipped_when_false`, plus the `classify` / `validate_batch` unit suites and `check_schema_first_error_strings_are_unchanged`.

### Problem

A whole-batch schema failure is charged against every batch-mate's *per-unit* validation retry budget even though the fault is per-*batch*. A provider that repeatedly drops or malforms one unit therefore burns the innocent units in the same batch through their retries too, so they land in `fallback_source` alongside the genuinely-failing unit — clustering fallbacks around one bad unit rather than isolating it.

### Impact

Diagnostics + output quality on a misbehaving provider: fallback clusters mislead attribution and degrade blocks that would have translated fine in isolation. No structural corruption (fallbacks are honest source).

### Required Actions

1. ~~Distinguish a per-batch schema fault from a per-unit failure so a batch-level fault does not consume innocent units' per-unit budgets (e.g. re-dispatch the batch minus the offending unit, or charge batch faults to a separate budget).~~ — **DONE 2026-08-03:** both, hybridized — innocents are salvaged in round 1 (not re-sent, which would waste calls reproducing results already in hand) and only the implicated units are re-dispatched, on a separate bounded per-batch budget.

### Verification

- [x] Innocent batch-mates translate in round 1 with exactly one attempt row while only the dropped unit falls back (`batch_fault_spares_innocent_units`, 2026-08-03)
- [x] A fully hostile provider terminates at the batch bound with every unit at honest `fallback_source` (`fully_hostile_empty_result_terminates`, 2026-08-03)
- [x] The two budgets drain independently; a batch fault does not consume the offender's content budget (`mixed_batch_and_unit_faults_drain_separate_budgets`, `offender_keeps_full_unit_budget_after_batch_fault`, 2026-08-03)
- [x] Report rows distinguish batch faults, and pre-wave rows serialize byte-identically (`attempt_outcome_batch_fault_is_skipped_when_false`, 2026-08-03)
- [x] Existing retry / report / cache suites pass unchanged (`check_schema` message parity preserved, 2026-08-03)

### Related

- ADR-0009 (bounded retry policy) — amended 2026-08-03: a second bounded budget beside the per-unit one
- ADR-0017 (batch-terminal abort) — untouched: schema faults are validation-layer, not provider-terminal
- DCR-0014 (auto-glossary preflight + batch-fault fairness) — the resolving record
- contracts.md §5 / §5a (validation tiers)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-03 (DCR-0015) — per-pane `dir="rtl"` stamping shipped.

## OI-0032: RTL target languages render left-to-right

- **Source:** DR-2026-07 design review
- **Date:** 2026-07-27
- **Decision:** ACCEPT (track — owner scope decision)
- **Status:** RESOLVED (2026-08-03)
- **Resolution:** OI-2026-08 wave / DCR-0015 Part B — RTL is **in scope**, signalled by explicit control with a best-effort table as the default. Precedence: `--target-direction {rtl|ltr|auto}` > profile `[render].target_direction` (new additive `ProfileRender` on `ProfileMetadata`, unknown values warn at load and normalize to auto) > `auto`. `auto` matches the label's ASCII-lowercased first `-`/`_` token against a 15-entry primary-subtag table (`ar arc ckb dv fa he iw ji nqo ps sd syr ug ur yi`; the grandfathered `iw`/`ji` are included, `ku` is deliberately excluded — Kurmanji is Latin-script, Sorani is `ckb`). RTL stamps ` dir="rtl"` on the pane; **LTR emits nothing** (the HTML default), which is what keeps existing ko/ja/en bundles byte-identical — a wave-review finding that the template had gained an explanatory comment (shipped inside every generated `index.html`) was fixed by removing the comment, leaving only the two placeholder insertions. Direction is **pane-level, not per-block** (no per-block language metadata exists, and the map is presentation-free), and the sync engine is direction-agnostic (`offsetTop`/`offsetHeight`), so **`sync.js` and both byte-mirrored copies are untouched**. The source pane auto-resolves from the resolved source label; there is no `--source-direction` flag in this wave. **Bundle-only:** `out.md` and the alignment map are untouched and the schema stays `1.1.0`; `<html>` keeps only `lang` (stamping `dir` there would flip the themer/legend chrome). `html_bundle_files` gained `source_dir_attr` / `target_dir_attr` parameters feeding two new template placeholders. ADR-0013 gained a dated amendment recording this as a presentation-layer hint over labels that stay opaque for prompt, cache, map, and validation — with the documented best-effort limit that expressive labels ("Korean (formal)", "العربية") stay LTR and the explicit flag/profile key is the authoritative path. Tests: `auto_rtl_table`, `expressive_labels_stay_ltr`, `explicit_mode_wins`, `resolve_mode_precedence`, `render_target_direction_loads_and_normalizes`, `cli_rtl_target_language_stamps_target_pane` (with the ko byte-stability control), `cli_target_direction_flag_overrides_table`, `cli_profile_render_direction_applies_and_flag_beats_it`.

### Problem

The rendered bundle stamps the target `lang` on the shell but never sets `dir="rtl"`, so a right-to-left target language (Arabic, Hebrew, …) renders left-to-right. Language labels are opaque (ADR-0013), so the engine has no built-in RTL detection to key off.

### Impact

RTL target output is visually wrong in the demo bundle. Whether RTL is in scope at all is undecided (see mvp-scope.md).

### Required Actions

1. ~~Owner scope decision: is RTL rendering in scope? If yes, decide how direction is signaled given opaque language labels (explicit profile/CLI flag vs a detection table).~~ — **DONE 2026-08-03:** in scope; signalled by **both** — an explicit flag/profile key as the authoritative path, with a best-effort subtag table as the `auto` default.

### Verification

- [x] RTL target label stamps `dir="rtl"` on the target pane only; an LTR control run emits no `dir=` at all (`cli_rtl_target_language_stamps_target_pane`, 2026-08-03)
- [x] Explicit flag beats the table in both directions; profile key applies and the flag beats it (`cli_target_direction_flag_overrides_table`, `cli_profile_render_direction_applies_and_flag_beats_it`, 2026-08-03)
- [x] The hint's best-effort limit is documented and pinned (`expressive_labels_stay_ltr`, 2026-08-03)
- [x] `out.md`, alignment map (schema `1.1.0`), and both `sync.js` mirrors unchanged (2026-08-03)

### Related

- ADR-0013 (opaque language labels) — amended 2026-08-03: presentation-layer direction hint; labels stay opaque for prompt / cache / map / validation
- DCR-0015 (prompt lift + tokenizer hint + RTL + live smoke) — the resolving record
- mvp-scope.md (RTL note updated from "undecided" to shipped behavior + documented limits)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-05 (DCR-0019) — the line table counts a lone CR as a boundary, under the owner-decided Support posture.

## OI-0033: `LineOffsets` counts only `\n`, so lone-CR sources desync sourcepos→byte mapping

- **Source:** discovered 2026-08-04 during the DCR-0017 crate-split wave, by the probe written to test Guard 1's spec-listed residual (a)
- **Date:** 2026-08-04
- **Decision:** ACCEPT (track — pre-existing defect, orthogonal to the split that found it)
- **Status:** **RESOLVED 2026-08-05 (DCR-0019)** — posture **Support**, owner-decided 2026-08-05; fixed in `b1c3c7f`, pinned and closed out in `09040f9`

### Problem

`transync-syntax::parser::ranges::LineOffsets::new` builds its line table by
scanning for `b'\n'` only. A source that uses **lone `CR`** (`\r` with no
following `\n`) as its line terminator — classic Mac line endings, or a
mixed-ending document — therefore produces a line table with **fewer entries
than the document has lines**, while comrak's `Sourcepos` counts the CR-only
breaks as real line boundaries. Every `pos_to_byte` lookup past the first lone
CR resolves against the wrong line, so `byte_range_for` returns a byte range
that does not correspond to the node.

Downstream, the damage lands on the payload slices built from those ranges:
observed outcomes are an **empty slice** (start and end collapse onto the same
clamped offset) and an **item-swallowing slice** (one block's range extends
over the next block's bytes). Both are silent — the mapping is arithmetic, not
a parse, so nothing errors.

**How it was found.** The DCR-0017 spec listed as Guard-1 residual (a) "units
whose `expected_list_topology` came back `None`", on the premise that
`inspect_list_topology` can fail on a payload that does not reparse as an item
subtree. Probing that premise **disproved it** for well-formed input: comrak
anchors an `Item`'s sourcepos at the **marker**, so a payload sliced from a
real list item always reparses as an item subtree and `None` never comes back.
The one construction that *does* produce a `None`-yielding payload is a
mis-sliced one — and this CR defect is the only route to a mis-slice found so
far. That is the whole reason it is recorded: it is the surviving premise
behind a residual otherwise documented as void.

### Impact

**Correctness, on an input class nothing in the repo currently covers.** For a
lone-CR document: block payloads can be empty or can swallow the following
block, which means garbage sent to the LLM, mis-attributed alignment rows, and
— because a mis-sliced list-item payload can defeat `inspect_list_topology` —
the one live route to Guard 1's otherwise-void residual (a). No shipped
fixture, sample, or scenario uses lone CR, and `\r\n` is **unaffected** (the
`\n` is still found; the `\r` is merely inside the preceding line's bytes),
so there is no known reachable failure on today's inputs. Severity is Medium
rather than High only because of that: the defect is real, the exposure is
not yet demonstrated on a supported input.

### Required Actions

1. ~~Decide the posture explicitly: **normalize** lone CR at intake (cheapest,
   but mutates source bytes, which collides with the byte-verbatim
   round-trip guarantees in `regen`/`htmlseg`), **support** it by counting
   `\r` not followed by `\n` as a line break in `LineOffsets::new` (matches
   CommonMark, which treats lone CR as a line ending), or **reject** it at
   intake with a clear error (honest, and cheapest to prove).~~ —
   **DONE 2026-08-05: Support**, owner-decided. Normalize was rejected because
   mutating source bytes collides head-on with the byte-verbatim round-trip
   guarantee that `regen` and `htmlseg` exist to keep; reject was rejected
   because it would make transync stricter than CommonMark. Support is the only
   option where the parser agrees with the parser — comrak already counts lone
   CR, and the defect was only ever that *our* line table did not.
2. ~~Whichever is chosen, add a lone-CR fixture and pin the behavior — an empty
   or item-swallowing payload slice must be impossible or must be a hard
   error, never silent.~~ — **DONE 2026-08-05.** `LineOffsets::new` gained
   exactly one arm (a `\r` whose next byte is not `\n`), naming comrak's
   `strings::is_line_end_char` as the authority in its doc comment. Fixtures
   are **inline `&str` constants, not checked-in `.md` files**: no
   `.gitattributes` protects CR bytes here, so an editor or an `autocrlf`
   setting could silently rewrite a fixture into a test that passes vacuously.
   Six tests landed in `ranges.rs` (which previously had none) plus a
   parse-level pin asserting exact ranges, exact slices, and the
   non-empty/non-overlapping/increasing property as a loop — the last of which
   fails wholesale pre-fix. A defensive parse warning now fires when a
   translatable **list-item** block has an empty computed range at a
   non-degenerate source position, so the next sourcepos quirk is loud instead
   of silently disarming `per_kind::check_list`.
3. ~~Re-check Guard 1's residual (a) afterwards: if lone CR becomes
   unreachable, the residual is genuinely void and DCR-0017's note can say so
   without the caveat.~~ — **DONE 2026-08-05.** DCR-0017's note and the
   mirroring comment in `validate/full_reparse.rs` now read "void
   unqualified", backed by a direct assertion rather than an inference.

### Verification

- [x] Posture decided and recorded — **Support**, owner-decided 2026-08-05 (spec §1; DCR-0019 Part A)
- [x] Lone-CR payload slices asserted correct: `lone_cr_document_slices_every_block_to_its_own_bytes` pins kinds, exact ranges, exact slices, and the structural loop, plus `warnings.is_empty()` — pre-fix this document also emitted the ref-defs "unattributed source text between blocks" note, so the test pins the observable symptom, not only the arithmetic (2026-08-05)
- [x] `\r\n` behavior pinned unchanged by the same test module — CR-aware offsets are **byte-identical** to the old LF-only table for every LF and CRLF document, so the fix is behavior-preserving by construction on the entire current corpus (2026-08-05)
- [x] Guard 1 residual (a) re-assessed and **closed**: `lone_cr_list_items_carry_list_topology_constraints` parses a lone-CR list document and requires every list-item unit to carry a non-empty payload *and* `Some(expected_list_topology)` — the residual's exact predicate (2026-08-05)

### Probe evidence (comrak 0.27, recorded because it is why the fix is that narrow)

A line-boundary sweep over `alpha<CH>bravo` gave a maximum sourcepos `end.line`
of **2** for LF, CR, and CRLF, and **1** for VT (U+000B), FF (U+000C), NEL
(U+0085), LS (U+2028), PS (U+2029), and NUL. None of those is a line ending, so
neither `str::lines()` (splits on `\n` only) nor any Unicode line-break API is
admissible — both would mis-count against comrak. On the CR-only probe document
comrak reported a maximum `end.line` of **6** against an offsets table of length
**1**; that gap was the defect. The same probe found the sibling defect now
filed as **OI-0034**.

### Related

- DCR-0017 (the wave that found it; Guard 1 and the disproven residual (a))
- **DCR-0019** (the resolution; Part A)
- ADR-0004 (comrak as the GFM parser — sourcepos is comrak's, the line table is ours; **upheld**, and the two now agree on line endings)
- OI-0022 (structural-fingerprint depth residuals — the other place sourcepos precision matters)
- **OI-0034** (the column-side sibling, filed by the same probe)

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-06 — NUL is normalized to U+FFFD at intake, before comrak sees the document.

## OI-0034: comrak's NUL→U+FFFD substitution desyncs byte columns

- **Source:** discovered 2026-08-05 by the OI-0033 mechanics probe (the same sweep that established comrak's line-ending rule)
- **Date:** 2026-08-05
- **Decision:** ACCEPT (track — pre-existing defect, orthogonal to the line-ending fix that found it; non-fatal today)
- **Status:** **RESOLVED 2026-08-06** — posture **Normalize at intake**, owner-decided 2026-08-06; ticket `743d27f0`, fixed in `e75e815`
- **Resolution:** `transync-syntax::parser::parse` substitutes U+FFFD for every NUL **before** comrak is handed the document, so comrak's byte columns and `parser::ranges`' line table count the same bytes by construction. The normalized string is what becomes `Document::source_text`, and therefore what every `source_range`, every `source_hash`, the alignment map's `document_id`, regen's fallback bytes, and both rendered panes index. Support was rejected because there is nothing for the range code to agree with — comrak has already discarded the NUL by the time it reports a position, so "account for the substitution width" means reimplementing comrak's preprocessing in the range mapper and keeping the copy in step forever. Reject was rejected for the same reason it was for OI-0033: CommonMark *requires* the substitution, so refusing the input would make transync stricter than the spec it implements.

### Problem

Comrak replaces a NUL byte (`U+0000`) in the source with the replacement
character **U+FFFD**, which is **three** bytes in UTF-8. Its `Sourcepos`
columns are byte columns counted over the *substituted* text, so every column
past a NUL on the same line is reported **two bytes too far**.

The probe case, verbatim: `"alpha\0bravo\n"` is 12 bytes, but comrak reports
the paragraph as `1:1-1:13` — a column beyond the line's actual byte length.
`transync-syntax::parser::ranges` maps those columns onto the *original*
bytes, so the mapping is off by two per preceding NUL on the line.

This is the **column** sibling of OI-0033's **line** defect. Unlike lone CR,
NUL is not a line ending (probe-verified: `max end.line = 1`), so the line
table is correct and only the within-line offset drifts.

### Impact

**Non-fatal today, by accident rather than by design.** `pos_to_byte` clamps
the resolved offset to the line's content end, and `byte_range_for` clamps
again to the source length, so the observed effect on the probe case is **one
extra byte** in the computed slice rather than an out-of-bounds panic or a
neighbor-swallowing range. No shipped fixture, sample, or scenario contains a
NUL byte, and a NUL in a Markdown source document is pathological input in the
first place.

The reason it is filed rather than ignored: the *class* of sourcepos quirks is
open (SCN-15 already ships a legitimately empty html range, and OI-0033 was the
line-side instance), and this one is currently held harmless by clamping — a
defense that exists for a different reason and could be tightened away by
someone who does not know it is load-bearing here.

**Correction (2026-08-06), from the resolving probe.** "One extra byte" was
wrong, and wrong in the *safer* direction: on the whole current corpus a
block-level range was not off at all. A block's `Sourcepos` always ends where
its line's content ends, and `pos_to_byte` clamps a drifted column back onto
exactly that offset — so the clamp was returning the *correct* answer, not an
approximately-correct one, for every block shape probed (paragraph, ATX heading
with and without a closing sequence, list item, fenced code block, and lines
with two NULs). The drift is only observable at a column strictly inside a
line, which today means an *inline* node's position. That makes the original
severity assessment right for the wrong reason: the exposure was not "one byte
of slop" but "an exact answer produced by a mechanism that has no idea it is
producing it", one inline-position consumer away from becoming real. Recorded
because the resolving fixture had to be built around it — a NUL fixture with no
trailing whitespace passes before and after the fix and proves nothing.

### Required Actions

1. ~~Decide the posture, the same three-way choice OI-0033 faced: **support**
   (make the column mapping account for comrak's substitution width, which
   means the range code must know which bytes comrak rewrote), **normalize**
   (strip or replace NUL at intake — mutates source bytes, so it collides with
   the same byte-verbatim round-trip guarantee that ruled normalization out for
   OI-0033), or **reject** (refuse a source containing NUL at intake, which is
   defensible for a text-document pipeline and is the cheapest to prove).~~ —
   **DONE 2026-08-06: Normalize**, owner-decided. The round-trip objection that
   ruled normalization out for lone CR does not transfer: that guarantee is
   about `Document::source_text`, and the substitution happens before that
   field exists, so `regen` still reproduces the parsed document byte for byte.
   Support would mean carrying comrak's preprocessing rules in the range mapper
   and keeping the copy in step; reject would refuse input CommonMark §2.3
   requires a parser to accept.
2. ~~Whichever is chosen, add an inline NUL fixture — **not** a checked-in `.md`
   file, for the same reason OI-0033's fixtures are inline — and pin the
   behavior: the computed slice must be correct or the input must be a hard
   error, never a silently-off-by-N range.~~ — **DONE 2026-08-06.** Six tests:
   four in `parser::nul_tests` (all RED without the substitution) and two in
   `parser::ranges::tests`. The fixture is an inline `&str` and its trailing
   spaces are load-bearing — see the correction above.
3. ~~State explicitly in `parser/ranges.rs` that the clamps are load-bearing for
   this case, or remove the need for them by fixing the mapping.~~ —
   **DONE 2026-08-06: need removed.** `pos_to_byte`'s doc now says what the
   clamp is and is not — a guard against an out-of-range column, never a
   correction for a systematically wrong one — and names this issue as the role
   it no longer has. The module doc states the equality both fixes serve:
   comrak's columns and the line table must count the same bytes, closed at the
   table end for lone CR and at the intake end for NUL.

### Verification

- [x] Posture decided and recorded — **Normalize at intake**, owner-decided 2026-08-06 (`parser::normalize_source`; `parse`'s and `Document::source_text`'s docs carry it) (2026-08-06)
- [x] NUL fixture added inline; computed slices asserted correct: `a_nul_source_parses_as_its_normalized_self` pins kinds, exact ranges and exact slices for the block containing the NUL *and* both blocks past it, plus `warnings.is_empty()`; `columns_past_the_nul_resolve_exactly_rather_than_by_the_clamp` resolves comrak's own inline column through `ranges::byte_range_for` and asserts the result is **strictly interior** to its line, so the clamp demonstrably did not produce it (2026-08-06)
- [x] Non-NUL column behavior pinned unchanged: `normalization_is_a_fixed_point_and_a_no_op_without_a_nul` asserts a NUL-free source is returned untouched and borrowed, and that parsing the already-normalized spelling yields identical ranges; the pre-existing lone-CR, CRLF, LF and table-edge tests are unchanged and green (2026-08-06)
- [x] The clamp's role documented **and** made unnecessary — both, in `parser/ranges.rs`; `ranges::tests::raw_nul_columns_drift` keeps the drift arithmetic itself on the record so a future regression has something to fail against (2026-08-06)
- [x] The normalized text is THE source downstream: `the_normalized_text_is_the_source_for_ids_hashes_and_regen` asserts the block IDs, each block's `source_hash` over the normalized bytes, and that `regen::regenerate` with nothing accepted reproduces the normalized document exactly (2026-08-06)

### Residual

~~A NUL that arrives in a **translated payload** rather than in the source is not
covered: `regen` splices provider bytes verbatim, so `out.md` could carry a raw
NUL while the rendered target pane shows U+FFFD for it. Nothing desyncs —
`validate::full_reparse` resolves only *start* columns of top-level nodes, which
are never preceded by a NUL on their line — so this is an output-consistency
asymmetry, not a mapping defect. Ticketed as `d06c4349`, out of scope here by
the resolving ticket's own scope line.~~ — **CLOSED 2026-08-07** (ticket
`d06c4349`).

The posture is **fail the unit**, decided under owner delegation and
deliberately *not* the intake posture this issue took: normalizing a payload
inside validation would change what "the payload bytes the provider returned"
means for the `Cache` and for the `ValidationReport`, and both are supposed to
be literal. So `validate::schema::check_payload_bytes` rejects a `U+0000`
before any content layer runs, at the `schema` layer (`batch_fault` absent —
the flag, not the layer, is what marks an envelope fault), and the unit takes
the ordinary ADR-0009 retry-then-fallback path; a fallback splices the block's
own source bytes, which are NUL-free by *this* issue's fix. An html unit's
payload is judged on its **decoded** segments, since that is what regen
splices and the wire form holds the byte escaped.

The two sides now make one statement — **`out.md` never contains a NUL** —
recorded in contracts.md §3 next to the source-side sentence this issue wrote.
Eight tests, four of them RED without the check, including a `run_pipeline`
case whose failure message is the defect itself ("out.md must never carry a
NUL").

### Related

- **OI-0033** (the line-side sibling; the probe that found both, and the posture precedent — same goal, the other mechanism)
- DCR-0019 (the wave that filed this; spec §1 last bullet — filed, not fixed)
- ADR-0004 (comrak as the GFM parser — the substitution is comrak's, the byte mapping is ours; **upheld**, and the two now count the same bytes)
- OI-0022 (structural-fingerprint depth residuals — the other place sourcepos precision matters)

***

> Archived 2026-08-23. Reason: resolved at both layers (route (c), DCR-0033) — audit history, and the record of a residual that is accepted rather than fixed.

## OI-0035: A `data-sync-id` injected through raw HTML can pre-claim a real block's anchor

- **Source:** R0002-0018 (Review 0002) (review archived and removed)
- **Date:** 2026-08-08
- **Decision:** ACCEPT (track — the mechanism is confirmed, but which layer should close it is a design choice)
- **Status:** RESOLVED (2026-08-23)
- **Resolution:** Route **(c) — both layers**, owner-ratified in the 2026-08-20 HTML→HTML design (spec §8) and landed as ti `490d97` wave 1 (DCR-0033; the two code commits are `eedc9e3` and `0972fa2`, 2026-08-22). **Required action 1** is answered by the route itself. **Action 2:** the renderer's Markdown `BlockKind::Html` success arm — the only live route by which source-controlled markup reaches a Markdown pane, since panes render with `unsafe_ = false` — strips the six reserved attribute names (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`, `data-skipped`) case-insensitively from every open tag in the block's own bytes before the wrapper writes ours, and `mountSync` builds its anchor set from the validated rows whose `sync_role` is not `non-sync` while `collectAnchors` — the single choke point for both panes at mount and at every reflow recompute — skips what that set does not contain. An unlisted anchor is inert forever, including one inserted after mount, which closes the 2026-08-09 correction's residual as well. Both `sync.js` copies moved in one commit; `sync_js_drift.rs` is the weld. The strip is **pane-only**: `out.md` keeps the author's bytes. **Action 3:** the browser suite gained an end-to-end case over a second `--html-out` bundle whose fixture's raw HTML claims the id of the paragraph that follows it (`web/tests/scn13.spec.js`), plus engine-direct cases (`web/tests/engine.spec.js`) and a Rust unit at the render call site. **Honest residual, accepted:** neither layer alone — and no DOM-visible discriminator — defeats an in-pane impostor carrying a *listed* id placed ahead of the genuine anchor, because `collectAnchors` is first-occurrence-wins. The render strip makes that case unreachable in panes transync produces; the engine gate makes every *unlisted* id inert in any pane, whoever produced it. A *listed* impostor in a pane transync did not produce is reached by neither, and that is the accepted boundary of route (c). **Measured while closing this, correcting an inference:** DOMPurify's default really does keep `data-*` attributes, verified against the vendored build the bundle ships. **A wave-0 defect this wave's own call site surfaced** — the balancer's walk reading a self-closing slash the way XML means it, and the strip welding bytes at its cut — was fixed in `d146a53` and recorded in DCR-0032's 2026-08-23 amendment; the render half depends on that fix, because the strip alone over the old walk traded an impostor anchor for a swallowed one.

### Problem

The browser sync engine builds its anchor sets by collecting **every**
`data-sync-id` in each pane's DOM. Alignment rows are used for validation and
warnings, but they are not the source of the anchor set, so membership in the
map does not gate what can become a scroll driver or target.

Half of that is a recorded decision and is not in question: ID-identity
pairing (the runtime pairs anchors by identical `data-sync-id` rather than
routing through the map's source/target indirection) is normative under
`d3acc3` / OI-0015. The unrecorded half is what this entry is for. Source
Markdown is untrusted data (architectural invariant 7), and raw HTML blocks
are translatable, structurally-owned content that reaches the rendered pane.
A `data-sync-id` attribute written into a source document therefore survives
the default DOMPurify configuration and lands in the DOM as an anchor that
the engine cannot distinguish from one the renderer emitted — so it can
**pre-claim the id of a real block** and become that block's scroll driver.

### Impact

Scroll synchronization can be steered by document content rather than by the
alignment map: the wrong pane region tracks the reader, or a genuine block's
anchor is shadowed. It is a correctness-of-presentation failure, not a data
loss or code-execution one — DOMPurify still bounds what markup renders.
Reachability requires a source document containing crafted raw HTML, which
invariant 7 says to expect rather than to rule out.

### Required Actions

1. Decide the layer, which is the reason this is tracked rather than fixed:
   (a) strip or namespace `data-*` attributes at sanitize time so injected
   anchors never reach the DOM; (b) build the anchor sets from validated
   alignment rows so DOM anchors outside the map are inert; or (c) both, if
   defense in depth is wanted at the render boundary and the engine boundary.
2. Implement the chosen layer in `web/js/sync.js` **and** its byte-identical
   embedded CLI twin in the same commit; the drift tests weld the pair.
3. Extend the browser suite (`scripts/test-browser.sh`) with a document whose
   raw HTML carries a `data-sync-id` colliding with a real block id.

### Verification

- [x] Code change applied — `crates/transync-syntax/src/render.rs` (render half); `web/js/sync.js` + `crates/transync-cli/web/sync.js` (engine half, one commit, byte-identical)
- [x] Tests pass — workspace suite and browser suite green; `sync_js_drift.rs` green
- [x] No regressions observed — SCN-13, the wasm demo and the engine-direct suite all green with no expectation edits outside the new cases

### Related

- `d3acc3` / OI-0015 — the ID-identity pairing decision this does **not**
  reopen; its packaging half is settled. **(Landed 2026-08-09; it changes
  nothing here on purpose.)** The anchor sets are still built by
  `pane.querySelectorAll("[data-sync-id]")`, so DOM membership still decides
  what can drive scroll and map membership still does not — every one of
  actions (a), (b) and (c) below is exactly as open as it was. Two facts for
  whoever routes it. The pass made route **(b)** slightly *cheaper*: reflow
  recompute re-collects anchors, so `collectAnchors` is now the single choke
  point every anchor set on either pane passes through, at mount and on every
  reflow, and it already takes a per-call policy argument — a map-membership
  gate lands there once and covers both. And the pass touched the existing
  duplicate-id warning that is today's only signal of a shadowing attempt: the
  *recompute's* re-collection is deliberately quiet (a resize drag would
  otherwise replay one warning per frame), while the **mount-time** warning —
  the one an injected anchor in the initially rendered document actually trips
  — is unchanged, as is the first-occurrence-wins policy that decides which of
  two same-id anchors survives. No reflow signal can introduce an anchor.
  **Correction (2026-08-09), same day, from this pass's own review:** the
  sentence that stood here — *"so nothing became reachable that was not
  reachable before"* — overstated that. Reflow inserts no anchor, but the quiet
  recompute **activates** one that entered the DOM after mount. Before this
  pass such an anchor stayed inert until a `destroy()` + re-mount, which
  re-runs the duplicate-id audit; now the next reflow signal — an `<img>` load
  is enough, and `controller.refresh()` schedules the same quiet recompute —
  folds it into the live anchor set with the duplicate warning suppressed and
  no audit at any point (`engine.spec.js` `h` phase 2 drives a post-mount
  `e-0006` with no re-mount). Under **this entry's** threat model — injection
  carried by the rendered source document, therefore present at mount — the
  mount-time audit still fires and exposure is unchanged. What is weaker is the
  doctrine the layer choice leans on: activation no longer implies an audited
  mount, so "mount is the audit point" is now an argument **for** route (b),
  not a substitute for it.
- ADR-0018 — raw HTML as translatable, structurally-owned content.
- DCR-0022 — the Review 0002 hardening pass that routed this to tracking.
- DCR-0033 — the closure: the render-side strip, the engine-side row gate, and the residual neither closes alone.

***

> Archived 2026-08-13. Reason: RESOLVED 2026-08-10 — a sparse allow-listed subset no longer counts as a genuine `--out-dir`.

## OI-0036: A sparse allow-listed subset counts as a genuine `--out-dir`

- **Source:** R0002-0028 (Review 0002) (review archived and removed)
- **Date:** 2026-08-08
- **Decision:** ACCEPT (track — co-decide with the sibling guard question)
- **Status:** RESOLVED (2026-08-10)
- **Resolution:** Decided together with ticket `66339b`, as required action 1 asked, and by the first of the three candidate shapes: `publish_out_dir` stages a `.transync-out-dir` **ownership marker** into every tree it publishes, and `ensure_out_dir_replaceable` now asks two separate questions — "is anything in here not transync's?" (the allow-list, which also gained the sibling ticket's staging-temp recognition) and "did transync publish this?" (the marker, or the **complete** published set for a target written before the marker existed). A directory holding only `out.md` fails the second question and is refused, which is this issue; a directory holding transync's own crash residue passes both, which is `66339b`. `contracts.md` §6, `persistence-and-files.md`, `Developer_Guide.md` and `Troubleshooting.md` carry the rule.

### Problem

`ensure_out_dir_replaceable` treats a directory as a transync bundle — and so
replaceable without `--force` — when its entries are a **subset** of the
allow-listed bundle names. A user directory that happens to contain only
`out.md` therefore qualifies: it is moved aside and its backup recursively
removed, with no `--force` and no prompt.

The operator did aim `--out-dir` at that directory, which is what keeps this
below the destructive findings the same review produced. But "holds one
allow-listed name" is a weaker test than "is a bundle this tool produced",
and the guard reads as though it were the stronger one.

### Impact

A directory holding unrelated work whose only entry matches a bundle name is
replaced without the `--force` confirmation the guard exists to require.
Recovery is the backup, which the same run then removes on success.

### Required Actions

1. Decide the guard policy **together with ticket `66339b`** (an `--out-dir`
   target is treated as foreign when a staging temp sits at its top level).
   They are the same question — how strictly "is this my bundle?" should be
   answered — approached from opposite directions, and deciding them apart
   risks two inconsistent answers in one function.
2. Candidate shapes to weigh: require a marker file written by publication;
   require the full bundle set rather than a subset; or require at least one
   *distinctive* member (`alignment.json` + `html/`) rather than any single
   allow-listed name.
3. Implement in `crates/transync-cli/src/output.rs`, keeping
   `scan_bundle_dir`'s temp-file recognition and the republish path working —
   no existing bundle may start demanding `--force`.

### Verification

- [x] Code change applied
- [x] Tests pass (if applicable)
- [x] No regressions observed

### Related

- Ticket `66339b` — the sibling guard question; decided together, as required.
- DCR-0021 — the publication lock and honest-replacement record.
- DCR-0022 — the Review 0002 hardening pass that routed this to tracking.
