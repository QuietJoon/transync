# transync

A Rust library that does two things:

1. **Translates [GitHub Flavored Markdown](https://github.github.com/gfm/) documents** with a consumer-supplied LLM, preserving every structural invariant the markup carries (heading levels, table column counts and alignment, list topology, code-fence safety, blockquote nesting).
2. **Synchronizes the source and translated panes by semantic block ID**, not by scroll percentage — so the panes stay anchored across translation-induced length changes (CJK doubling source byte width, 50-row tables wrapping differently, code blocks staying fixed-height while their neighboring prose grows).

Use it as a library behind your own LLM provider, or through the bundled `transync` CLI: `transync translate` runs the pipeline end to end and publishes the whole output set — translated Markdown, alignment map, validation report and a ready-to-open side-by-side HTML bundle — into one directory, and `transync serve` hands that bundle to a browser on `127.0.0.1`.

**Status:** v0.4.0 — released and tagged 2026-08-20; depend on the annotated `v0.4.0` tag. Release history is in [`CHANGELOG.md`](CHANGELOG.md).

## Feature 1 — Translation pipeline

The application — not the LLM — owns block IDs, table shape, list topology, and code-fence placement. The LLM is constrained to translation judgment alone.

- **Block-aware GFM parsing** via Comrak. Each sync-relevant block (headings, paragraphs, tables, fenced code, list items, blockquotes, thematic breaks, block-level images) gets a stable kind-prefixed `BlockId` (`h1-0001`, `p-0042`, `t-0007`, …).
- **Per-block translation units** carry a `source_payload`, surrounding context (section path + neighbor snippets), and per-kind `BlockConstraints` (must-preserve column count, code-fence info string, list `(depth, ordered, task)` tuples, heading level, blockquote child sequence).
- **Pluggable provider** via the `Translator` trait — HTTP-free, async, object-safe. Two adapters ship in-tree, both on strict Structured Outputs: `transync-openai` (the default; model-driven dispatch across the OpenAI Chat Completions and Responses APIs, configured by `TRANSYNC_OPENAI_MODEL` / `TRANSYNC_OPENAI_BASE_URL`) and `transync-anthropic` (the Anthropic Messages API, one endpoint, configured by `TRANSYNC_ANTHROPIC_MODEL` / `TRANSYNC_ANTHROPIC_BASE_URL` — DCR-0029). Further providers (OpenRouter, local llama, Ollama, etc.) are siblings of the same shape, and a consumer names a provider crate directly — the facade re-exports none. The `transync` CLI is the exception: its provider is a compile-time choice and it links OpenAI only (ADR-0002).
- **Layered validation**: schema → ID-set equality → per-kind shape (table cols + alignment + row count, list topology, code-fence info, heading level, blockquote children) → fragment reparse → inline protection (link/image destinations; policy-gated code spans) → full-document reparse. Any layer rejecting triggers a bounded verbatim resubmission of the failed units — same prompt, same scope (ADR-0009); persistent failure falls back to source bytes with `fallback_status: fallback_source` recorded on the alignment map. Never silent corruption.
- **Profile TOML** with `[system].prompt` template (`{{source_language}}` / `{{target_language}}` substituted per call), `[constraints]`, `[batching]`, and `[[glossary]]` entries. CLI overrides via `--profile`, `--system-prompt`, or `--system-prompt-file`.
- **Long-doc batching** with token-budget heuristics + a cache keyed on all thirteen axes of `CacheKey` (`cache.rs`): the provider fingerprint, validation-schema version, the unit's source hash, source/target languages, profile version + prompt hash + glossary hash, model, block kind, input mode, context-hint hash, and the hash of the batch instruction the unit was translated under. A re-run against the same source skips every cached unit, reusing the cached translation for each; hits are re-validated through the per-unit layers, and units disqualified by the post-regeneration document gate are evicted by `BlockId`. Document-level metadata rides the same `Cache` seam (DCR-0028 / ADR-0021): a run that dispatched **zero** provider batches replays the recorded detected source language instead of reporting none, so a fully-cached `--source-language auto` re-run reproduces the previous output set byte for byte — detected language included, pinned by a CLI test over two runs on one `--cache-dir`. Replay never overrides a live provider answer, and a custom `Cache` that leaves the two defaulted metadata methods unimplemented simply keeps the old behavior; `InMemoryCache` and `DiskCache` both implement them.
- **Staged fileset commit** for the whole output set: each file is written to a `.tmp.<pid>` staging path, `fsync`ed, then swapped in with `rename`, so a failure while content is being staged touches no output; only a crash or I/O error during the final rename pass can leave a mixed set (reported on stderr, exit 4) — a *second run* cannot, because every directory a publication writes into is held under an exclusive OS file lock (`.transync-publish.lock`, left in place by design) for the whole staging-and-rename span. `--out-dir` publishes the entire bundle into one directory: one atomic rename onto a fresh target, and a crash-safe (not atomic — the old tree is moved aside first) replacement of an existing one. CLI exit codes 0..7 distinguish success / argument error / read failure / all-fallback / write failure / other / configuration rejected (fix the configuration and re-run) / document refused (the provider's content policy or the model declined — skip the document).

## Feature 2 — Block-level scroll sync

Source and translated panes follow each other smoothly even when their rendered heights diverge.

- **Stable `BlockId` flows through the entire pipeline** — Rust IR → LLM contract → regenerated Markdown → annotated HTML attribute (`data-sync-id`) → JS sync engine. The browser never reasons about scroll percentage.
- **Smooth proportional in-block following.** The active pane's reference line (4 px from the top) determines the active block and the fraction of it the user has scrolled past; the partner pane's `scrollTop` is set so the matching block shows the same proportional offset. A per-frame lerp (`SMOOTHING_FACTOR = 0.2`) eases motion so the partner glides toward the target rather than snapping.
- **No oscillation.** Per-pane programmatic-scroll lock (90 ms) absorbs the cascade scroll event each pane fires when we set its `scrollTop`, without blocking the other pane's user input.
- **Schema-versioned alignment map** (`schema_version: "1.2.0"`) — durable wire format between the renderer and the JS engine. Consumers MUST reject unknown majors.
- **Three runtime CSS themes** (Default / Document / Book) switched via a top-right `<select>`; choice persists in `localStorage`. Custom themes drop in as additional `body[data-theme="…"] .pane …` rules.
- **Vanilla JS, no framework, no build step.** The engine is `web/js/sync.js`; the CLI embeds a copy via `include_str!`. The sync engine reads only `data-sync-id` attributes — it never independently parses Markdown (per ADR-0001, parser divergence breaks anchor stability).

## How they fit together

```
   .md  ──▶  parse  ──▶  Document IR  ──▶  build batches  ──▶  Translator
                                                                    │
                                                                    ▼
                                                          per-batch result
                                                                    │
                              regenerated MD  ◀── regen ◀── validate (6 layers)
                                       │
                          ┌────────────┴────────────┐
                          ▼                         ▼
                    AlignmentMap                 source.html
                    (schema 1.2.0)               target.html
                          │                         │
                          └─────────┬───────────────┘
                                    ▼
                          web/js/sync.js (block-level scroll sync)
                                    ▼
                              browser demo
```

A consumer that only needs the translation half can ignore the rendered HTML and consume `output.translated_document` + `output.alignment_map` directly.

## Workspace layout

```
transync/                     # Cargo workspace root
├── crates/
│   ├── transync-html/        # HTML mechanics — tag scanning, element extents, fragment
│   │                        #   balancing, text-segment extract/splice; wasm32-clean
│   ├── transync-syntax/      # syntax layer — parser, IR + IDs, regen, render, align;
│   │                        #   sits on transync-html; wasm32-clean
│   ├── transync-core/        # translation pipeline on top — HTTP-free, no LLM dep
│   ├── transync/             # curated public facade — explicit re-export list (contracts.md §0)
│   ├── transync-cli/         # `transync` binary
│   ├── transync-openai/      # OpenAI Translator impl (Chat Completions + Responses dispatch)
│   ├── transync-anthropic/   # Anthropic Translator impl (Messages API, one endpoint — DCR-0029)
│   └── transync-wasm/        # browser surface over transync-syntax; `publish = false` (ADR-0019)
├── web/                      # vanilla JS demo (block-ID sync engine)
└── docs/                     # design artifacts (architecture, decisions, project)
```

## MVP track

**Track D**: Rust library + thin CLI + vanilla-JS demo. WASM rendering is post-MVP (track C). See `docs/architecture/mvp-scope.md` for the locked scope.

## Quick start

```bash
cargo build --workspace
cargo test --workspace
./scripts/smoke.sh                                # the full hard gate (see below)
OPENAI_API_KEY=sk-... ./scripts/test.sh           # live OpenAI + browser demo
```

`smoke.sh` is not the getting-started script: it builds the workspace,
runs the standing wasm gate, both test suites and the rustdoc gate,
then `scripts/build-wasm.sh`, and only then the CLI dry run. It needs
the `wasm32-unknown-unknown` rustup target, `wasm-pack` and binaryen ≥
121 on `PATH`, all of which fail loudly rather than skipping. The
no-API-key end-to-end run needs none of that — it is one `cargo run -p
transync-cli --features test-stub-provider -- translate …`.

Full walkthrough: **[`docs/Quick_Start.md`](docs/Quick_Start.md)**.

## Documentation

| Audience | Read |
|---|---|
| First-time user, "I just want to run it"            | [`docs/Quick_Start.md`](docs/Quick_Start.md) |
| Developer building against transync or extending it | [`docs/Developer_Guide.md`](docs/Developer_Guide.md) |
| Architecture overview + data flow                   | [`docs/architecture/README.md`](docs/architecture/README.md) |
| Wire / process-crossing contracts                   | [`docs/architecture/contracts.md`](docs/architecture/contracts.md) |
| Why each design decision was made                   | [`docs/decisions/`](docs/decisions/) (full ADR set) |
| Active design baseline + document authority order   | [`docs/project/design-baseline-2026-07.md`](docs/project/design-baseline-2026-07.md) |
| SCN-13 browser verification (Playwright suite is primary; manual checklist kept) | [`web/tests/`](web/tests/) via `scripts/test-browser.sh`; [`web/SMOKE.md`](web/SMOKE.md) |
| Full doc index                                      | [`docs/index.md`](docs/index.md) |

## License

MIT — see `LICENSE` at the repo root.
