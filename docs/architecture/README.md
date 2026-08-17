# transync — Architecture Overview

This document is the entry point to the architecture artifacts. It names the components, describes the data flow, and points to the deeper documents for each concern.

## Authority

Document authority is governed by the hierarchy in `../project/design-baseline-2026-07.md` (baseline `BL-2026-07-B`): `contracts.md` plus the scenario and Playwright test suites define intended behavior; this document and its siblings define steady-state structure; ADRs carry rationale; DCRs carry historical deltas; code is implementation evidence. On conflict, the higher authority wins and the lower document gets fixed.

## What this is

A Rust library that translates GFM-compatible Markdown documents using a consumer-supplied LLM, then emits:
1. **Regenerated translated Markdown** that reparses cleanly under the same dialect.
2. **An alignment map** linking source blocks to target blocks by stable ID.
3. **Annotated HTML** for both panes, ready for browser-side block-level scroll sync.

The application — not the LLM — owns block IDs, table shape, list topology, and code-fence placement. The LLM is constrained to translation judgment alone.

## Components

| Component                | Crate / location                  | Responsibility |
|--------------------------|-----------------------------------|----------------|
| Markdown parser          | `crates/transync-syntax` (`parser` mod) | GFM AST + IR with block IDs and source ranges |
| Block indexer            | `crates/transync-syntax` (`id` mod)    | Stable ID assignment + source hashing |
| HTML segment engine      | `crates/transync-syntax` (`htmlseg` mod) | Text-segment extraction / positional splice-back inside raw-HTML blocks (`lol_html`, one pinned `Settings` for both passes) + render-path fragment balancing (ADR-0018 / DCR-0016) |
| Translation unit builder | `crates/transync-core` (`unit` mod)    | Context-rich units; section path + neighbor snippets |
| Batch manager            | `crates/transync-core` (`batch` mod + `unit::section`) | Section-coherent packing: the unit list is partitioned at every heading (`unit::section`), then each section is packed by the sequential token-budget packer (consumer-driven via `TranslateOptions`). No batch straddles a section boundary; an oversize section splits inside itself (DCR-0027) |
| Translator trait         | `crates/transync-core` (`llm` mod)     | HTTP-free contract: `translate_batch(batch, cancel) -> result` + `fingerprint()` for cache namespacing; the `cancel` token is the run's (DCR-0024, contracts.md §5b) |
| Translation cache        | `crates/transync-core` (`cache` mod)   | `Cache` trait v2 (`get`/`put`/`evict`, fallible); `CacheKey` identity; provisional results, hits re-validated (ADR-0015) |
| Validator                | `crates/transync-core` (`validate` mod)| Layered: schema → IDs → per-kind → fragment reparse → inline protection → full reparse |
| Regenerator              | `crates/transync-syntax` (`regen` mod) | Splice translated fragments + reserialize tables / fences |
| Alignment map generator  | `crates/transync-syntax` (`align` mod) | Source-target block map with fallback status |
| Renderer                 | `crates/transync-syntax` (`render` mod) | GFM AST → annotated HTML with sync attributes, one whole-document parse per pane (DCR-0017); raw-HTML blocks render live (auto-balanced) or as the escaped failure placeholder |
| Shared top-level walk    | `crates/transync-syntax` (`walk` mod)  | One normalization of the top-level node sequence (list collapsing, kind labels, per-list item counts) consumed by both the render zip and `validate::full_reparse` (DCR-0017) |
| Facade crate             | `crates/transync`                 | Semver firewall: an **explicitly curated** re-export list over `transync-core`, enumerated in `contracts.md` §0 and pinned by `tests/public_surface.rs`. Engine internals are deliberately not reachable through it (DCR-0005 / DCR-0017 / DCR-0018) |
| OpenAI Translator impl   | `crates/transync-openai`          | Default `Translator` impl: model-driven Chat/Responses dispatch + Structured Outputs |
| Anthropic Translator impl | `crates/transync-anthropic`      | Second in-tree `Translator` impl: the Anthropic Messages API — one endpoint, so no `Api` axis — with structured outputs via `output_config.format` (DCR-0029, contracts.md §8). The facade re-exports neither provider; a consumer names a provider crate directly (ADR-0002) |
| CLI                      | `crates/transync-cli`             | `transync translate` + `transync serve` |
| JS sync engine           | `web/js/sync.js`                  | Scroll-listener + reference-line active-block sync by block ID; smooth-lerp partner follow with a programmatic-scroll lock |

## Pipeline state machine

Temporal order matters: per-unit validation (including fragment reparse and inline protection) runs *before* regeneration; the full-document reparse runs *after* regeneration and is the document-level gate with three policy branches. Cache writes happen at provisional acceptance; cache evictions happen at document-level disqualification (contracts §5a).

```
source.md
    │
    ▼
┌──────────────────┐   ┌──────────────────┐   ┌───────────────────────────────┐
│ parse (Comrak)   │──▶│ assign block IDs │──▶│ build units + batches         │
│ GFM AST → IR     │   │ + source hashes  │   │ (partition at every heading,  │
└──────────────────┘   └──────────────────┘   │ then sequential token-budget  │
                                              │ packing WITHIN each section)  │
                                              └──────────────┬────────────────┘
                                                             ▼
                                              ┌───────────────────────────────┐
                                    hit ┌─────│ per-unit cache lookup         │
                                        │     │ (CacheKey; hits re-validated  │
                                        │     │ through the per-unit layers,  │
                                        │     │ ADR-0015)                     │
                                        │     └──────────────┬────────────────┘
                                        │                    │ miss
                                        │                    ▼
                                        │     ┌───────────────────────────────┐
                                        │     │ concurrent batch dispatch     │
                                        │     │ (Translator::translate_batch; │
                                        │     │ fingerprint() namespaces the  │
                                        │     │ cache)                        │
                                        │     └──────────────┬────────────────┘
                                        │                    ▼
                                        │     ┌───────────────────────────────┐
                                        │     │ per-unit layered validation   │
                                        │     │ schema / ID-set               │
                                        │     │  → per-kind shape             │
                                        │     │  → fragment reparse           │
                                        │     │  → inline protection          │
                                        │     └───────┬──────────────┬────────┘
                                        │      reject │              │ pass
                                        │             ▼              │
                                        │  ┌────────────────────┐    │
                                        │  │ bounded retry:     │    │
                                        │  │ verbatim unit      │──┐ │  (re-dispatch →
                                        │  │ resubmission +     │  │ │   re-validate)
                                        │  │ RetryContext side  │◀─┘ │
                                        │  │ channel (ADR-0009) │    │
                                        │  └─────────┬──────────┘    │
                                        │            │ budget        │
                                        │            │ exhausted     │
                                        │            ▼               ▼
                                        │  ┌────────────────┐  ┌───────────────────┐
                                        │  │ fallback_source│  │ provisional result│
                                        │  │ (marked in     │  │ → cache put       │
                                        │  │ alignment map) │  └─────────┬─────────┘
                                        │  └───────┬────────┘            │
                                        │          │                     │
                                        └──────────┴──────────┬──────────┘
                                                              ▼
                                              ┌───────────────────────────────┐
                                              │ regenerate full translated    │
                                              │ Markdown                      │
                                              └──────────────┬────────────────┘
                                                              ▼
                                              ┌───────────────────────────────┐
                                              │ FULL-document reparse         │
                                              │ (document-level gate, §5a)    │
                                              └─┬──────────┬─────────┬────────┘
                                            ok  │    Hard  │         │ recoverable
                                                │          ▼         ▼
                                                │  ┌──────────────┐ ┌────────────────────────┐
                                                │  │ Err + evict  │ │ FallbackPerBlock:      │
                                                │  │ implicated   │ │ attributed → widened   │
                                                │  │ cache keys   │ │ neighbors → FallbackAll│
                                                │  └──────────────┘ │ cascade; downgraded    │
                                                │                   │ units evicted from     │
                                                │                   │ cache. FallbackAll:    │
                                                │                   │ every unit →           │
                                                │                   │ fallback_source        │
                                                │                   └───────────┬────────────┘
                                                │◀──────────────────────────────┘
                                                ▼
┌──────────────────┐   ┌──────────────────┐   ┌───────────────────────────────┐
│ align            │──▶│ render           │──▶│ TranslationOutput             │
│ (alignment map)  │   │ (annotated HTML, │   │  • translated_document        │
└──────────────────┘   │ both panes)      │   │  • alignment_map              │
                       └──────────────────┘   │  • annotated_*_html           │
                                              │  • validation_report          │
                                              └──────────────┬────────────────┘
                                                             ▼
                                              ┌───────────────────────────────┐
                                              │ JS sync engine (browser)      │
                                              └───────────────────────────────┘
```

## Architectural invariants

These are the rules every implementation slice must respect. (Earlier phrasings in the brainstorming spec §3 and `references/draft.md` §3 are historical; this list is current.)

1. **Block ID is the only sync currency.** No reliance on heading text, slugs, line numbers, or scroll percentage.
2. **LLM owns content; application owns structure.**
3. **Tables translate as whole blocks**, never cell-by-cell. Since DCR-0026 a table whose estimated response exceeds the run's output ceiling is split at *packing* time into header-carrying row windows — each window is itself a complete GFM table — and merged back into one block before regeneration, so no window id ever reaches the alignment map or the DOM. `[constraints].default_table_strategy` selects it (`"row-window-first"`, the shipped default profile's value, vs `"whole-block"`, which every unset or unrecognized value resolves to); the CLI exposes it as `--table-strategy`. Isolated cell translation stays rejected, not deferred.
4. **Code blocks are full fenced-block units;** Rust validates and safely re-wraps the fence.
5. **Layered validation** — schema → ID set → per-kind shape → fragment reparse → inline protection → full-document reparse — with retry then fallback to source. Unit validity is two-tier: per-unit acceptance is *provisional* (and cached); only survival of the post-regeneration full-document reparse is *final* (contracts §5a).
6. **Source content is data, not instructions.**
7. **Static-document assumption.** ID survival across edits is a non-goal.
8. **Browser must not parse Markdown independently.** Use Rust-generated annotated HTML or Rust WASM. Parser divergence breaks anchor stability. Since 2026-08-04 (DCR-0017) the WASM half of that sentence has a compile path: the whole syntax layer lives in `crates/transync-syntax`. **Since 2026-08-05 (ADR-0019 / DCR-0020) it is an executed path too**: `crates/transync-wasm` ships wasm-bindgen entry points and `web/demo-wasm.html` renders both panes in the browser with the same Rust renderer — byte-identically to the CLI's fragments, pinned as a standing Playwright gate. The gate line is now `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown` in the tracked pre-commit hook — `scripts/hooks/pre-commit`, the only copy, which `scripts/install-hooks.sh` points `core.hooksPath` at while deleting any legacy `.git/hooks` shadow — and in `scripts/smoke.sh`. What remains deliberately undone is **CLI bundle integration**: the module is ~41× the entire JS payload and a bundle already ships the rendered HTML that its view mode reproduces.

## Where to look next

| You want to know about…                  | Read |
|------------------------------------------|------|
| What's in scope vs deferred              | `mvp-scope.md` |
| The mandatory scenarios                  | `scenario-matrix.md` |
| Source-of-truth + ownership              | `source-of-truth-table.md` |
| Persistence + file lifecycle             | `persistence-and-files.md` |
| Wire/process-crossing contracts          | `contracts.md` |
| Rough wire-type schemas                  | `rough-schema.md` |
| Module map + scenario coverage           | `../implementation/module-map.md` |
| Phase-3 skeleton plan + slice checklists | `../project/skeleton-plan.md`, `../project/implementation-slice-checklists.md` |
| Initial STUB inventory                   | `../project/stub-manifest.md` |
| The active baseline + authority hierarchy | `../project/design-baseline-2026-07.md` |
| What is open, deferred or blocked right now | `../backlog.md` — the cross-source index; `../project/open-issues.md` holds the authoritative detail for `OI-` entries |
| ADRs (full decision set — foundations + reject rationales) | `../decisions/` (see `../index.md` for the list) |
| Which review round an `R000N-NNNN` citation means | `../../reviews/README.md` — round registry + citation convention (two rounds numbered themselves `0001`) |
| The brainstorming narrative (historical) | `../superpowers/specs/2026-05-01-transync-design.md` |
| The original design draft (historical)   | `../../references/draft.md` |
