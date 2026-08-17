# Design Baseline `BL-2026-05-01-A`

> **HISTORICAL SNAPSHOT — SUPERSEDED.** This is the design baseline as issued at the 2026-05-01 Phase-5 handoff. It was superseded on 2026-07-13 by [`BL-2026-07-B`](design-baseline-2026-07.md) (the v0.2.0 steady state). The file is preserved **unedited** below this banner: its crate paths (pre-split `crates/transync` monolith), contract inventory, and topology are historical record, not current structure. For the current authority hierarchy and live contract surfaces, read `design-baseline-2026-07.md`.

This is the implementation-ready baseline for `transync` MVP-D (Rust library + thin CLI + JS demo). Implementation slices may begin against this baseline. A change to any artifact named below is a Design Change Record under `docs/project/design-change-records/`, not an in-place edit.

## Identity

| Field                    | Value                                              |
|--------------------------|----------------------------------------------------|
| Baseline ID              | `BL-2026-05-01-A`                                  |
| Issued                   | 2026-05-01 (Phase 5 handoff)                       |
| Scope                    | MVP-D — Rust library + CLI + vanilla JS demo       |
| Stub manifest version    | generated (45 `STUB` rows; 0 `DEFERRED` rows)      |
| Phase 4 hard gate        | met — see "Validation evidence" below              |
| Active design phase      | 5 (Validation & Handoff — complete)                |

## Scope status

- **In scope (locked):** SCN-01 through SCN-14 per `docs/architecture/scenario-matrix.md`. The fourteen scenarios are the complete MVP behavior surface.
- **Out of scope (deferred to post-MVP):** WASM rendering (track C); sentence-level sub-anchors (NG3); live-edit re-anchoring (NG4); MDX, raw HTML, YAML frontmatter, math (NG2); disk-backed cache; glossary editor UX; `transync-anthropic` and `transync-local-llama` provider crates; streaming translation; headless-browser SCN-13 automation. Each is documented in `docs/architecture/mvp-scope.md` §DEFERRED and in `docs/project/intake.md` §"Explicit Non-Goals".

## Contract inventory (frozen at this baseline)

| Contract                                            | Where defined                                         | Stability |
|-----------------------------------------------------|-------------------------------------------------------|-----------|
| `Translator` trait + `TranslationBatch` / `TranslationBatchResult` / `UnitResult` / `TranslatorError` | `docs/architecture/contracts.md` §1; `crates/transync/src/llm.rs` | stable from `0.1.0`; field additions allowed only with `#[non_exhaustive]` |
| Profile TOML (`slug`, `version`, `[system]`, `[constraints]`, `[batching]`, `[[glossary]]`) | `contracts.md` §2; `crates/transync-cli/profiles/default.toml` | unknown sections/keys ignored with warning; key removal bumps `version` |
| `AlignmentMap` JSON wire format                     | `contracts.md` §3; `crates/transync/src/align.rs`     | `schema_version: "1.0.0"` — semver |
| Annotated HTML attributes (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`) | `contracts.md` §4; `crates/transync/src/render/attrs.rs` | wire form is kebab-case; JS engine reads `data-sync-id` only |
| Retry / fallback policy parameters                  | `contracts.md` §5; `crates/transync/src/pipeline/retry.rs` | defaults locked: `max_per_unit_validation_retries=2`, `max_oversize_split_retries_per_batch=3`, `max_per_batch_provider_retries=1` |
| CLI argument contract + exit codes 0..5             | `contracts.md` §6; `crates/transync-cli/src/translate_cmd.rs` + `error.rs` | stable from `0.1.0` |
| `transync-openai` constructor + `from_env()` resolution order | `contracts.md` §7; `crates/transync-openai/src/lib.rs` | `OPENAI_API_KEY` / `TRANSYNC_OPENAI_MODEL` / `TRANSYNC_OPENAI_BASE_URL` |
| Renderer output shape (two pane fragments + templated `index.html` shell) | ADR-0006; `crates/transync-cli/src/output.rs` + `web/index.html.tpl` | `--html-out/{index,source,target}.html + alignment.json + sync.js` |
| `BlockId` format `<kind>-<NNNN>`                    | ADR-0005; `crates/transync/src/id.rs`                 | single global counter; separate `source_hash: u64` |

## Architecture decisions

| ADR  | Title                                              | Status    |
|------|----------------------------------------------------|-----------|
| 0001 | Block-level alignment as the only sync currency    | Accepted  |
| 0002 | HTTP-free core with `Translator` trait + provider crates | Accepted  |
| 0003 | Cargo workspace with provider crates as separate workspace members | Accepted  |
| 0004 | Comrak as the GFM parser                           | Accepted  |
| 0005 | `BlockId` format `<kind>-<NNNN>` with separate `source_hash: u64` | Accepted  |
| 0006 | Renderer output shape — two pane fragments plus shell | Accepted  |

No ADRs are superseded at this baseline. No DCRs are open.

## Pointers (the baseline document set)

| What                                              | Where |
|---------------------------------------------------|-------|
| Module map (scenario → component coverage)        | `docs/implementation/module-map.md` |
| Rough schema (every wire / durable type)          | `docs/architecture/rough-schema.md` |
| Source-of-truth table                             | `docs/architecture/source-of-truth-table.md` |
| Persistence and file lifecycle                    | `docs/architecture/persistence-and-files.md` |
| Wire / process-crossing contracts                 | `docs/architecture/contracts.md` |
| Scenario matrix (SCN-01..14 with verification keys) | `docs/architecture/scenario-matrix.md` |
| MVP scope (in scope + DEFERRED list)              | `docs/architecture/mvp-scope.md` |
| Skeleton plan (workspace topology + gen order)    | `docs/project/skeleton-plan.md` |
| Implementation slice checklists (SL-00..SL-14)    | `docs/project/implementation-slice-checklists.md` |
| Stub manifest (45 STUB rows, status `generated`)  | `docs/project/stub-manifest.md` |
| Brainstorming spec (intent of record)             | `docs/superpowers/specs/2026-05-01-transync-design.md` |

## Repo / workspace topology (frozen at this baseline)

```
crates/transync          # core library, HTTP-free; 24 source files
crates/transync-openai   # default Translator impl; 5 source files
crates/transync-cli      # binary (translate, serve); 5 source files + assets
web/                     # workspace-level demo source-of-truth
scripts/smoke.sh         # phase-4 hard-gate driver (kept for SL-00..SL-14 use)
```

12 fixture `.md` files live in `crates/transync/tests/fixtures/`. The integration test suite is one binary (`tests/scenarios.rs`) aggregating 12 `scn_NN_*.rs` smoke tests; the CLI binary has its own integration smoke at `crates/transync-cli/tests/cli_smoke.rs` gated on the `test-stub-provider` Cargo feature.

## Validation evidence (Phase 4 hard gate, replayable from clean checkout)

```
$ cargo build --workspace                                            # clean
$ cargo test --workspace                                             # 12 scenario tests pass
$ cargo test -p transync-cli --features test-stub-provider           # cli_translate_smoke passes
$ ./scripts/smoke.sh                                                 # exit 0; 7 outputs land in $WORKDIR
```

The single warning (`ExitCode::ArgumentError` and `AllUnitsFellBack` unused) is intentional — those variants are reserved for SL-12's exit-code coverage.

## Implementation handoff

- Implementation track may begin at any time against this baseline.
- The dependency graph in `implementation-slice-checklists.md` §"Slice dependency graph" lists which slices may run in parallel.
- Each slice closes its rows in `stub-manifest.md` (status flips from `generated` to `closed` with the closing commit hash).
- Per-slice CHANGELOG entries land under `[Unreleased]` as work merges; the next release tag is the implementation track's call.

## Sign-off

User review of Phase 4 skeleton: 2026-05-01.
Baseline issued by `/design-first-architecture` Phase 5: 2026-05-01.
