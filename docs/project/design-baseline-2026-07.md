# Design Baseline `BL-2026-07-B`

This is the steady-state baseline for `transync` v0.2.0, issued at the close of the 2026-07 external-review wave (EXT-2026-07). It declares the current system: which documents are authoritative for what, and which contract surfaces are live. A change to any artifact named below is a Design Change Record under `docs/project/design-change-records/`, not an in-place edit.

## Identity

| Field           | Value                                                        |
|-----------------|--------------------------------------------------------------|
| Baseline ID     | `BL-2026-07-B`                                               |
| Issued          | 2026-07-13 (external-review wave close)                      |
| Workspace       | v0.2.0 — `transync-core` (engine) / `transync` (facade = semver firewall) / `transync-openai` / `transync-cli` |
| Scope           | MVP-D steady state — Rust library + CLI + vanilla JS demo    |
| Supersedes      | `BL-2026-05-01-A` (2026-05-01) — historical                  |

## Supersession

`BL-2026-05-01-A` is historical. Its record (`docs/project/design-baseline.md`) is preserved unedited under a supersession banner; its contract inventory, workspace topology (pre-split `crates/transync` monolith), and validation evidence describe the system as of the 2026-05-01 design handoff, not the system as shipped. Where the two baselines disagree, this document governs.

## Authority hierarchy

When documents conflict, the higher authority wins and the lower document must be fixed:

1. **Intended behavior** — `docs/architecture/contracts.md`, the scenario test suite (`crates/transync/tests/scenarios.rs` + `tests/scenarios/`, SCN-01..14, run through the facade), and the headless Playwright suite (`web/tests/`, driven by `scripts/test-browser.sh`, primary SCN-13 evidence).
2. **Steady-state structure** — the current architecture docs: `docs/architecture/README.md`, `mvp-scope.md` (as amended), `rough-schema.md`, `persistence-and-files.md`, `source-of-truth-table.md`.
3. **Rationale** — ADRs (`docs/decisions/0001`..`0016`, all with frontmatter; amendments are recorded in-file, e.g. ADR-0014's 2026-07-13 amendment).
4. **Historical deltas** — DCRs (`docs/project/design-change-records/`; DCR-0001..0004 archived, DCR-0005..0011 live — DCR-0009/0010/0011 record this wave).
5. **Implementation evidence** — the code itself.

Historical narratives (`docs/superpowers/specs/2026-05-01-transync-design.md`, `references/draft.md`, `docs/project/design-baseline.md`) sit outside this hierarchy: they explain where the design came from and are never fixed to match the present.

## Contract inventory (live at this baseline)

*Paths are as of this baseline's issue date (2026-07-13) and are not rewritten as code moves — the contracts themselves are what this table pins. Since then the DCR-0017 crate split (2026-08-04) moved `align.rs` and `render/attrs.rs` to `crates/transync-syntax/src/`; both contracts (`AlignmentMap` JSON, the annotated-HTML attribute set) are unchanged, and the facade paths `transync::align` / `transync::render::attrs` still resolve. `docs/implementation/module-map.md` carries the live layout. Amendment (2026-08-07, ti c02f69): `CacheKey` gained one axis, `instruction_hash` — the instruction the unit's batch assembled — in the sanctioned 0.3.0 surface window; contracts.md §5a carries the identity principle it implements.*

| Contract                                            | Where defined                                         | Notes |
|-----------------------------------------------------|-------------------------------------------------------|-------|
| `Translator` trait v2 — `translate_batch` + `fingerprint()` | `contracts.md` §1; `crates/transync-core/src/llm.rs` | `fingerprint()` returns the `ProviderFingerprint` that namespaces cache identity |
| `RetryContext` (`attempt`, `rejected_by`, `reason`) | `contracts.md` §5; `crates/transync-core/src/llm.rs`  | non-content side channel on `TranslationUnit`; unit payload resubmitted verbatim (ADR-0009) |
| `Cache` trait v2 — `get` / `put` / `evict`, fallible (`CacheError`) | `contracts.md` §5a; `crates/transync-core/src/cache.rs`; ADR-0015 | cache errors degrade, never abort; `InMemoryCache` clears once and resumes on poison |
| `CacheKey`                                          | `crates/transync-core/src/cache.rs`                   | provider fingerprint + validation-schema version + source hash + langs + profile version/prompt hash/glossary hash + model + block kind + context hash; hits re-validated per ADR-0015 |
| `AlignmentMap` JSON wire format                     | `contracts.md` §3; `crates/transync-core/src/align.rs` | `schema_version: "1.0.0"` — semver |
| Profile TOML (`slug`, `version`, `[system]`, `[constraints]`, `[batching]`, `[[glossary]]`) | `contracts.md` §2; `crates/transync-core/profiles/default.toml` | glossary scope `conditional-on-section` is rejected at load until section-aware batching lands (ADR-0014, amended 2026-07-13) |
| Annotated HTML attribute contract                   | `contracts.md` §4, §4a; `crates/transync-core/src/render/attrs.rs` | incl. the outer-wrapper positioning contract (DCR-0003) |
| Retry / fallback policy + provisional-vs-final unit validity | `contracts.md` §5, §5a; `crates/transync-core/src/pipeline/retry.rs` | provisional results cached; document-level disqualification evicts by `BlockId` |
| CLI argument contract + exit codes 0..7             | `contracts.md` §6; `crates/transync-cli/src/translate_cmd.rs` + `error.rs` | incl. `--out-dir` (staged fileset commit; one atomic rename onto a fresh target, crash-safe replacement of an existing one — DCR-0021) and `--max-input-bytes` (64 MiB default, limit+1 read; 4 MiB aux-file caps); `6`/`7` split the provider taxonomy by remediation (ti `e62b59`, v0.4.0 — append-only, `0`–`5` unchanged) |
| `transync-openai` constructor + `from_env()`        | `contracts.md` §7; `crates/transync-openai/src/lib.rs` | model-driven Chat/Responses dispatch; 32 MiB response cap; `target_output_tokens` is a live output ceiling |

## Pipeline shape at this baseline

Parse (Comrak) → assign block IDs → build units + batches (sequential token-budget packing; section-awareness deferred) → per-unit cache lookup → concurrent batch dispatch → layered per-unit validation (schema/ID-set → per-kind shape → fragment reparse → inline protection) → bounded verbatim retry with `RetryContext` → fallback to source after budget → provisional results cached → regenerate → full-document reparse with three policy branches (`Hard` / `FallbackPerBlock` cascade / `FallbackAll`) and targeted cache eviction → align → render. The state machine is drawn in `docs/architecture/README.md`.

## Sign-off

External-review items P0-1 (single architectural baseline) and P0-2 (current pipeline diagram) executed 2026-07-13. DCR-0009 (v0.2 boundary), DCR-0010 (inline protection), and DCR-0011 (staged commit / --out-dir / limits) record the wave's individual design deltas.

## Appended note — 2026-08-09 (DCR-0027, ticket `43cfb4`)

Two sentences of this record stopped being true. It is a dated snapshot, so they stay as written and this note carries what replaced them:

- **Pipeline shape.** "sequential token-budget packing; section-awareness deferred" is now *partition at every heading, then sequential token-budget packing WITHIN each section*. A batch never straddles a section boundary; a section too large for one batch splits inside itself, and that is the only thing that separates one section's units. A document with no heading packs bit-identically to this baseline. `contracts.md` §5 carries the policy.
- **Profile TOML.** The glossary-scope note — "`conditional-on-section` is rejected at load until section-aware batching lands (ADR-0014, amended 2026-07-13)" — has been discharged: that batching landed, the scope loads and is filtered per section against a `sections` selector matched on the heading stack, and the `ProfileError::Unsupported` all three gates raised was **removed** in v0.4.0. `contracts.md` §2 carries the semantics; ADR-0014 carries its own 2026-08-09 note.

Two knock-on facts worth recording beside them: `CacheKey`'s `profile_prompt_hash` and `glossary_hash` moved from run scope to **batch** scope (per-section prompts mean the prompt bytes vary within a run; the axes are derived from what the batch carries), and `unit::build_batches` became infallible — it was fallible only to raise the retired refusal. EXT-2026-07 P0-3 and P2 are both closed by this: P0-3's rejection was always the time-boxed half of the deferral, and P2's honest label ("this module has no section logic") is no longer the honest one.
