# transync MVP Intake

Transcribed from the brainstorming spec at `docs/superpowers/specs/2026-05-01-transync-design.md`. Anything later phases need beyond this lives in the brainstorming spec or in `references/draft.md`.

> **Correction (2026-08-08, R0002-0073).** Two statements below describe a server
> that has never run: the **File/Blob Lifecycle** row for *Web UI assets*
> ("Served by `transync serve` from RAM") and the **System Shape** process-model
> bullet ("`transync serve` runs a short-lived static HTTP server on
> `127.0.0.1`"). `transync serve` shipped **DEFERRED (STUB-061)**: it parses
> `--rendered` / `--bind` / `--port`, prints the bind address it would have used,
> and exits non-zero (`5`, `Other`) so a script cannot mistake the print for a
> listening server. Nothing is served from the binary's memory. The demo shell,
> the sync engine and the sanitizer are compiled into the binary and **written to
> disk** by `translate --html-out`; that directory is then served by an external
> static HTTP server (`python3 -m http.server`, per `web/SMOKE.md`, which is also
> what `scripts/test-browser.sh` drives). The bundle is self-contained, so any
> origin serving those files works. This is a deferral, not a cancellation: the
> real loopback server is commissioned as a post-0.3.0 roadmap ticket (`b791d6`).
> `contracts.md` §6 carries the live contract.
>
> **Follow-up (2026-08-09).** `b791d6` landed, so the **System Shape** bullet is
> now true as written: `transync serve` does run a short-lived static HTTP
> server on `127.0.0.1` for the demo page. The **File/Blob Lifecycle** row is
> still wrong, and for the reason given above rather than a superseded one:
> the server serves from **disk**, reading the directory `translate --html-out`
> wrote, and nothing is served out of the binary's memory. "Bound to binary
> lifetime" is wrong the same way — the bundle outlives the process that wrote
> it and the process that serves it.
>
> The same *Web UI assets* row names `include_dir!` as the embedding mechanism.
> That was the intake's plan; the shipped code uses three `include_str!`
> constants in `crates/transync-cli/src/output.rs` and never took an `include_dir`
> dependency. The canonical storage the row gives — `crates/transync-cli/web/` —
> is correct.
>
> This intake is a Phase-1 transcription, so the text stands and the correction is
> appended.

## Product Goal

- **Goal:** A Rust library that translates a GFM-compatible Markdown document with a consumer-supplied LLM and re-emits a regenerated translated Markdown plus an alignment map. Source and target panes synchronize **by semantic block ID**, not by scroll percentage.
- **Local run target:** clean-checkout-local. `cargo build --workspace` and `cargo run -p transync-cli -- translate ...` work from a fresh clone with only a Rust toolchain and an OpenAI-compatible API key.
- **Definition of "working":** All MVP scenarios `SCN-01` through `SCN-14` pass end-to-end on a developer machine; `cargo test --workspace` is green; the JS demo synchronizes scroll on the SCN-13 fixture.

## Actors and Roles

| Actor                  | What they do |
|------------------------|--------------|
| Library consumer       | Calls `transync::translate(source, options, translator)`. Owns auth, retries, tracing of LLM calls. |
| `Translator` impl      | Consumer-supplied or `transync-openai`. Handles batch translation requests; honors the structural contract. |
| `transync-cli` binary  | End-user CLI. Translates a single `.md` file → translated MD + alignment JSON + annotated HTML. Bundles a default profile. |
| JS demo (browser)      | Loads annotated HTML + alignment map, mounts dual panes, drives `IntersectionObserver`-based block-ID sync. |
| LLM provider           | OpenAI Responses API in v1 (or any OpenAI-compatible endpoint via `base_url` override). |
| End user               | Runs the CLI, optionally edits a profile TOML, opens the demo page. |

## Mandatory MVP Scenarios

The trigger / input / expected-output prose for each scenario lives in §11 of the brainstorming spec and is canonicalized in `docs/architecture/scenario-matrix.md` per the design-first template. The summary below names all SCNs; do not treat it as the canonical contract.

Fourteen MVP scenarios in seven groups:

| ID     | Group       | Scenario                                                                                            | Critical |
|--------|-------------|-----------------------------------------------------------------------------------------------------|----------|
| SCN-01 | Parse       | Heading + paragraph translation; regenerated MD reparses                                            | Yes      |
| SCN-02 | Tables      | Whole-block GFM table translation; column count + alignment preserved                               | Yes      |
| SCN-03 | Tables      | Oversized table → row-window fallback with header context                                           | Yes      |
| SCN-04 | Code        | Fenced code block; fence regenerated safely; translated comments accepted                           | Yes      |
| SCN-05 | Lists       | Nested list with task items; topology + checkbox state preserved                                    | Yes      |
| SCN-06 | Blockquotes | Blockquote with nested paragraphs; container preserved                                              | Yes      |
| SCN-07 | Validation  | ID/column mismatch → retry with stricter prompt → success                                           | Yes      |
| SCN-08 | Fallback    | Persistent failure → `fallback_status: fallback_source` in map; sync anchor present                 | Yes      |
| SCN-09 | Security    | Prompt-injection-shaped source content treated as data; schema-valid output                         | Yes      |
| SCN-10 | Batching    | Long doc split into N batches by token budget; partial-resume works                                 | Yes      |
| SCN-11 | Language    | `source-language=auto` → detected language echoed                                                   | Yes      |
| SCN-12 | CLI         | `transync translate` end-to-end on a real doc → 4 output files written                              | Yes      |
| SCN-13 | Sync        | JS demo: scrolling either pane drives the other by block ID; no oscillation                        | Yes      |
| SCN-14 | Reparse     | Full regenerated doc reparses; anchor count + order match source                                    | Yes      |

## Explicit Non-Goals (MVP)

- WASM rendering — *Post-MVP track C.*
- Sentence-level sub-anchors — *Draft NG3.*
- Live-edit re-anchoring — *Draft NG4.*
- MDX, raw HTML, YAML frontmatter, math syntax — *Draft NG2.*
- Disk-backed translation cache — *Per design, in-memory is correct for v1.*
- Glossary editor UX — *Post-MVP.*
- Additional providers (`transync-anthropic`, `transync-local-llama`) — *OpenAI only in MVP; trait in place for later.*
- Streaming translation — *Post-MVP.*

## Persistent State

| Entity / State              | Why it must persist                                                          | Source of truth |
|-----------------------------|------------------------------------------------------------------------------|-----------------|
| Profile TOML (when supplied)| User-editable translation behavior — system prompt body + glossary           | User-supplied path; CLI `--profile` flag |
| API key                     | Authenticate against the LLM provider                                        | `OPENAI_API_KEY` env or `transync-openai` constructor argument |
| CLI output artifacts        | Translated MD, alignment JSON, annotated HTML are the deliverables           | User-supplied output paths |

**Not persisted:** translation cache (process-lifetime in-memory only), per-batch retry history, validation reports beyond the in-memory `ValidationReport` returned to the caller. CLI consumers may serialize the report themselves; the library does not write it to disk.

## File / Blob Lifecycle

| File class            | Ingress                                       | Canonical storage                          | Generated outputs                                | Cleanup / retention |
|-----------------------|-----------------------------------------------|--------------------------------------------|--------------------------------------------------|---------------------|
| Source `.md`          | User-supplied via `--input`                   | User filesystem                            | None (read-only)                                 | User-managed |
| Profile TOML          | User-supplied via `--profile`, optional       | User filesystem; default ships embedded    | None                                             | User-managed |
| Translated `.md`      | Written by CLI                                | User-supplied via `--output`               | The translated document                          | User-managed |
| Alignment JSON        | Written by CLI                                | User-supplied via `--map`                  | Source-target block correspondence               | User-managed |
| Annotated HTML pair   | Written by CLI                                | User-supplied directory via `--html-out`   | `source.html` + `target.html` + alignment       | User-managed |
| JS sync engine        | Embedded in CLI binary or shipped in `web/`   | `web/js/sync.js` in the repo               | Loaded by the demo HTML                          | Bound to release artifact |
| Web UI assets         | Embedded at compile time via `include_dir!`   | `crates/transync-cli/web/`                 | Served by `transync serve` from RAM              | Bound to binary lifetime |

## Integrations

| Integration                                          | Required for MVP? | Real or deferred? | Notes |
|------------------------------------------------------|-------------------|-------------------|-------|
| OpenAI Responses API (Structured Outputs)            | MVP yes           | Real              | `transync-openai` crate; `tiktoken-rs` for budgets |
| OpenAI-compatible proxies (Azure, OpenRouter, …)     | MVP yes (free)    | Real (via `base_url`) | Same `transync-openai` impl; consumer overrides URL |
| Other LLM providers (`transync-anthropic` etc.)      | Post-MVP          | Deferred — trait in place | Implement `Translator` in a sibling crate |
| Browser rendering                                    | MVP yes           | Real              | Vanilla JS demo + DOMPurify; no framework, no build step |
| WASM renderer                                        | Post-MVP track C  | Deferred          | Renderer is structured to compile to `wasm32-unknown-unknown` later |

## System Shape

- **Single workspace, multi-crate Rust library + thin binary.** Three crates in MVP: `transync` (core, HTTP-free), `transync-cli` (binary), `transync-openai` (default `Translator` impl).
- **Process model:** the library is in-process. `transync-cli` runs as a one-shot CLI for `translate`; `transync serve` runs a short-lived static HTTP server on `127.0.0.1` for the demo page.
- **Communication style:**
  - Library API: Rust function calls + `async-trait` `Translator`.
  - CLI ↔ filesystem: read input MD, write translated MD + alignment JSON + annotated HTML.
  - JS demo: loads alignment map JSON + annotated HTML; uses `IntersectionObserver` per pane.
- **Languages and major frameworks:**
  - Rust: `comrak`, `serde`/`serde_json`, `thiserror`, `tracing`, `async-trait`, `tokio` (async runtime), `clap` (CLI), `tiktoken-rs` (in `transync-openai`), `async-openai` or `reqwest` (in `transync-openai`).
  - JS: vanilla ESM, `DOMPurify` for HTML sanitization. No framework, no bundler.
- **Contract format:** Rust types in `transync` are the source of truth for `TranslationBatch` / `TranslationBatchResult`. The `Translator` trait crosses the LLM boundary; the alignment map JSON shape crosses the renderer boundary. Both are versioned by the alignment-map `schema_version` field.
- **Verification preference:** unit tests per crate + integration tests for the full pipeline against fixture documents in `crates/transync/tests/fixtures/`. JS sync engine has Playwright-driven smoke tests post-MVP; for MVP, SCN-13 verifies via manual smoke run.

## Critical Libraries / Systems / Scenarios

| Item                              | Why critical                                       | Required negative behavior |
|-----------------------------------|----------------------------------------------------|---------------------------|
| Comrak GFM parser                 | Source of truth for block boundaries + reparse     | Reparse of regenerated MD must succeed; if Comrak rejects, regeneration is buggy. |
| `Translator` trait contract       | The LLM boundary; only contract callers see        | Trait must remain object-safe + `async-trait`-compatible; changes are breaking. |
| AlignmentMap JSON schema          | Renderer ↔ JS sync contract                        | Schema changes must bump `schema_version`; old maps must be rejected with a clear error. |
| Validation layer                  | Prevents silent corruption on LLM rewrites         | Failed validation must trigger retry / fallback, never accept-and-render. |
| Prompt template                   | Defends against prompt injection from source       | Source content rendered as data section; system prompt asserts non-instruction status. |

## Estimation Preference

**Dependency-ordered slices with risk notes** (default).

## Open Items

### Resolved in Phase 2

| Item                                                                                       | Resolved by |
|--------------------------------------------------------------------------------------------|-------------|
| Exact `BlockId` string format                                                              | ADR-0005 — kind-prefixed sequential `<kind>-<NNNN>` with separate `source_hash: u64` field |
| Profile TOML schema                                                                        | `docs/architecture/contracts.md` §2 — `slug` + `version` + `[system]` + `[constraints]` + `[batching]` + `[[glossary]]` |
| Annotated HTML attribute naming                                                            | `docs/architecture/contracts.md` §4 — `data-sync-id` / `data-block-kind` / `data-order` / `data-fallback` / `data-parent-id` |
| `AlignmentMap` on-disk JSON shape + `schema_version`                                       | `docs/architecture/contracts.md` §3 — `schema_version 1.0.0`; `rough-schema.md` §11 |
| Retry policy parameters (max retries per unit; max splits per oversize)                    | `docs/architecture/contracts.md` §5 — defaults locked: `max_per_unit_validation_retries=2`, `max_oversize_split_retries_per_batch=3`, `max_per_batch_provider_retries=1` |
| Default OpenAI model + token budget defaults                                               | `docs/architecture/mvp-scope.md` defaults table — `gpt-5-chat-latest`, `target_output_tokens=8000`, `max_units_per_batch=8` |

### Resolved in Phase 3

| Item                                                                                       | Resolved by |
|--------------------------------------------------------------------------------------------|-------------|
| Smoke fixture set (`.md` files exercising all 14 SCNs)                                     | `docs/project/skeleton-plan.md` §6 — 12 fixtures specified, with `scn-03-table-large.md` and `scn-10-long-document.md` generated programmatically by `tests/common/fixture_gen.rs` |
| Whether the renderer emits one HTML doc with two pane sections, or two separate HTML files | ADR-0006 — two pane fragments (`source.html` + `target.html`) plus a templated `index.html` shell, locked as the renderer output shape |

All Phase-1 open items are now closed. Phase 4 may proceed once the user approves the Phase-3 plan.
