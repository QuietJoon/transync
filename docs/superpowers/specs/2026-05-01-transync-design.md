# transync — MVP Design Spec

> **HISTORICAL — ORIGINAL BRAINSTORMING NARRATIVE.** This is the 2026-05-01 brainstorming output, preserved unedited below this banner. It describes the design as first conceived — an `IntersectionObserver` sync engine (replaced by the scroll-listener engine, DCR-0008), a working `serve` command, content-only code units (now full fenced-block units), and the pre-split workspace (`crates/transync` monolith, split by DCR-0005) — all since superseded. Its self-description as "the source-of-truth narrative" no longer holds: document authority is governed by the hierarchy in `docs/project/design-baseline-2026-07.md`, where this file ranks as historical narrative only.

**Status:** approved 2026-05-01
**Brainstorming session:** 2026-05-01
**Companion baseline:** to be assigned at Phase 5 handoff (`BL-2026-05-01-A` placeholder)

This is the brainstorming output that frozes the v1 design before any production code lands. It is the source-of-truth narrative; the structured Phase 1+ artifacts (`mvp-scope`, `scenario-matrix`, ADRs) refine specific facets in the format the design-first-architecture skill expects.

## 1. Product Goal

`transync` is a Rust library that translates a GFM-compatible Markdown document with a consumer-supplied LLM and re-emits a regenerated translated Markdown document plus an alignment map. Source and target panes can be rendered side-by-side and synchronized **by semantic block ID**, not by scroll percentage.

The primary differentiator vs ad-hoc dual-pane translators (e.g. the LLM-Trans reference) is structural integrity: the application — not the LLM — owns block boundaries, IDs, table shape, list topology, and code-fence placement. The LLM is constrained to translation judgment alone.

## 2. MVP Cut

**MVP track = D** (per Q1 brainstorming): Rust library + a thin `transync` CLI binary + a vanilla-JS demo page. WASM rendering is **post-MVP** (track C).

The library is HTTP-free. The LLM call is owned by the consumer through a `Translator` trait; a default `transync-openai` implementation ships in a sibling crate so the CLI runs out of the box.

## 3. Architectural Invariants (non-negotiable)

These match `references/draft.md` and the project-level `CLAUDE.md`. Implementation must not violate any of them:

1. **Block ID is the only sync currency.** Same `block_id` flows through Rust IR → LLM request → LLM response → regenerated Markdown → DOM anchors → JS sync engine. No reliance on heading text, generated slugs, line numbers, or scroll percentage.
2. **LLM owns content; application owns structure.** The LLM may translate, preserve, or partially translate any text. It must not change unit IDs, outer block kind, table column count, list topology, code-fence delimiters, or heading level.
3. **Tables translate as whole blocks.** Row-window batching with header context is the fallback for oversized tables. Cell-isolated translation is forbidden.
4. **Code blocks are content-only units.** Rust regenerates the fence (choosing safe length) and preserves the info string. The LLM never returns a fenced block envelope.
5. **Layered validation.** Schema → ID set equality → per-block-kind shape → fragment reparse → full-document reparse + anchor count/order. Any layer can reject; rejection triggers retry then fallback.
6. **Retry then fallback to source.** Persistent failure must surface as `fallback_status: fallback_source` in the alignment map, not as silent corruption.
7. **Source content is data, not instructions.** System prompt declares this; output is schema-validated regardless. Raw HTML in v1 is disabled / escaped / rejected.
8. **Static-document assumption.** Path-based + sequential IDs with source hashes are sufficient. Cross-edit ID survival is a non-goal.

## 4. Workspace Layout

```
transync/                          # Cargo workspace root
├── Cargo.toml                     # [workspace] members = ["crates/*"]
├── crates/
│   ├── transync/                  # core library — HTTP-free, no LLM dep
│   ├── transync-cli/              # `transync` binary (D-track MVP)
│   └── transync-openai/           # OpenAI Responses-API Translator impl
├── web/
│   ├── index.html                 # dual-pane demo page
│   └── js/sync.js                 # vanilla JS sync engine
├── docs/{architecture,decisions,project,superpowers}/
├── references/draft.md            # original architecture reference
└── CHANGELOG.md
```

Future provider crates land alongside `transync-openai` (e.g. `transync-anthropic`, `transync-local-llama`) — never folded into the core crate.

## 5. Public API (`transync` crate)

### 5.1 Pipeline (high-level — the 95% API)

```rust
pub async fn translate(
    source: &str,
    options: &TranslateOptions,
    translator: &impl Translator,
) -> Result<TranslationOutput, TransyncError>;
```

`TranslationOutput`:
- `translated_markdown: String` — full regenerated GFM Markdown
- `alignment_map: AlignmentMap` — source-target block correspondence + fallback status
- `annotated_source_html: String` — source HTML with sync attributes
- `annotated_target_html: String` — target HTML with sync attributes
- `validation_report: ValidationReport` — per-unit pass/retry/fallback record
- `fallback_count: usize` — convenience surface from the report

### 5.2 Step layer (escape hatch)

Each phase is a public function for advanced consumers, deterministic testing, and post-MVP streaming use cases:

```rust
pub fn parse(source: &str) -> Result<Document, ParseError>;
pub fn build_units(doc: &Document, options: &TranslateOptions) -> Vec<TranslationBatch>;
pub fn validate(batch: &TranslationBatch, result: &TranslationBatchResult) -> ValidatedBatch;
pub fn regenerate(doc: &Document, validated: &[ValidatedBatch]) -> (String, AlignmentMap);
pub fn render(doc: &Document, alignment: &AlignmentMap) -> RenderedHtml;
```

### 5.3 `Translator` trait

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
    ) -> Result<TranslationBatchResult, TranslatorError>;
}
```

The core `transync` crate has zero `reqwest`/`async-openai` dependency. Consumer owns auth, retries at the network layer, observability.

### 5.4 Data shapes

`TranslationBatch` carries:
- `units: Vec<TranslationUnit>`
- `source_language: String` (`"auto"` allowed)
- `target_language: String`
- `glossary: Vec<GlossaryEntry>`
- `profile: ProfileMetadata` (slug + version + body)

`TranslationUnit`:
- `unit_id: BlockId` (string-ish, stable ID from §11 of `references/draft.md`)
- `block_kind: BlockKind` (Heading | Paragraph | ListItem | Table | CodeBlock | Blockquote | …)
- `input_mode: InputMode` (text fragment | full table MD | code content | …)
- `source_payload: String`
- `context: BlockContext` (section path, neighbor snippets)
- `constraints: BlockConstraints` (column count, language, indentation policy, …)
- `source_hash: u64`
- `batch_id: BatchId`

`TranslationBatchResult`: per-unit `{ unit_id, output_kind, translated_payload, optional warnings }` where `output_kind ∈ {Translated, Preserved, PartiallyTranslated, FailedNeedsFallback}`.

## 6. `transync-openai` Crate

- Implements `Translator` against the OpenAI Responses API + Structured Outputs.
- Uses `tiktoken-rs` for token estimation; pagination + split-on-oversize ported from the LLM-Trans reference pattern.
- Configurable `base_url` lets consumers point at OpenAI-compatible proxies (Azure OpenAI, OpenRouter, local llama.cpp servers, etc.).
- Bring-your-own `OPENAI_API_KEY` (env-default; constructor accepts an explicit value).
- Emits structured tracing spans for retries and split events.

## 7. CLI (`transync-cli`)

```
transync translate \
  --input doc.md \
  --output doc.ko.md \
  --map alignment.json \
  --html-out site/ \
  --target-language ko \
  [--source-language en|auto] \
  [--profile profile.toml] \
  [--model gpt-5-chat-latest]

transync serve --rendered site/   # static 127.0.0.1 server for the demo
```

A minimal default profile ships embedded in the CLI binary so a clean checkout can run end-to-end without authoring a profile file.

## 8. JS Sync Engine (`web/js/sync.js`)

- Vanilla ESM, no framework, no build step.
- Reads alignment-map JSON injected into the page (or fetched from a sibling URL).
- Mounts annotated source HTML + annotated target HTML into two pane containers.
- `IntersectionObserver` per pane against `[data-sync-id]` anchors.
- Active block selection: intersection ratio → viewport-center distance → source order tiebreak → hysteresis.
- Programmatic-scroll lock + RAF batching to suppress feedback loops.
- Recomputes anchor positions on `resize`, font-load, image-load, collapsible expansion.
- Target ≈ 300 LOC.

DOMPurify is used to sanitize the annotated HTML pre-mount (matches LLM-Trans precedent and §20.4 of the draft).

## 9. Renderer

- Rust → annotated HTML in v1.
- Each sync-relevant block emits `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback` attributes.
- Comrak is the chosen GFM parser (AST-oriented; matches §4.4 of the draft). pulldown-cmark is rejected for v1 because it forces event-stream bookkeeping that complicates ID stability.
- WASM rendering is **DEFERRED** to post-MVP track C. The renderer is structured so the same crate can compile to `wasm32-unknown-unknown` later without a refactor.

## 10. Caching

- In-memory only in v1.
- Composite key: `(source_hash, target_lang, profile_version, model_id, block_kind)`.
- Process-lifetime; no cross-run persistence.
- Disk-backed and shared cache are **DEFERRED**.

## 11. MVP Scenarios

The numbered list below is canonicalized in `docs/architecture/scenario-matrix.md` with explicit Trigger / Input / Expected-Output / Verification columns. The summary is the contract; the matrix is the test harness reference.

| ID     | Group        | Scenario                                                                                            |
| ------ | ------------ | --------------------------------------------------------------------------------------------------- |
| SCN-01 | Parse        | Heading + paragraph translation; regenerated Markdown reparses                                      |
| SCN-02 | Tables       | GFM table translated whole-block; column count + alignment preserved                                |
| SCN-03 | Tables       | Oversized table → row-window fallback with header context                                           |
| SCN-04 | Code         | Fenced code block; fence regenerated safely; comments may be translated                             |
| SCN-05 | Lists        | Nested list with task items; topology + checkbox state preserved                                    |
| SCN-06 | Blockquotes  | Blockquote with nested paragraphs; container preserved                                              |
| SCN-07 | Validation   | ID-mismatch / column-mismatch → retry with stricter prompt → success                                |
| SCN-08 | Fallback     | Persistent failure → `fallback_status: fallback_source` in alignment map; sync anchor still present |
| SCN-09 | Security     | Prompt-injection-shaped source content treated as data; output schema-valid                         |
| SCN-10 | Batching     | Long doc split into N batches by token budget; partial-resume works                                 |
| SCN-11 | Language     | `source-language=auto` → detected language echoed in the result                                     |
| SCN-12 | CLI          | `transync translate` end-to-end on a real doc → 4 output files written                              |
| SCN-13 | Sync         | JS demo: scrolling either pane drives the other by block ID, no oscillation                         |
| SCN-14 | Reparse      | Full regenerated doc reparses; anchor count + order match source                                    |

## 12. DEFERRED (out of scope, architecturally provisioned)

- WASM build (post-MVP track C)
- `transync-anthropic` / `transync-local-llama` provider crates
- Disk-backed translation cache
- Streaming translation (per-batch SSE)
- Sentence-level sub-anchors (NG3 of draft)
- Live-edit re-anchoring (NG4)
- MDX, raw HTML, YAML frontmatter, math syntax (NG2)
- Glossary editor UX

## 13. Default Settings

- **Parser:** Comrak
- **Profile format:** TOML (matches LLM-Trans / resp-translator)
- **Renderer error policy:** strict — failed fragments use the retry/fallback chain; never silent corruption
- **JS demo HTML sanitization:** DOMPurify
- **License:** MIT
- **Edition:** Rust 2024 (where stabilized) / 2021 fallback if 2024 features are not needed

## 14. Open Items (deferred to later phases)

| Item                                                              | Resolution phase | Risk if unresolved                     |
| ----------------------------------------------------------------- | ---------------- | -------------------------------------- |
| Exact `BlockId` string format (sequential vs path-based vs hybrid) | Phase 2          | ID drift across reparses               |
| Profile TOML schema (sections, glossary shape)                    | Phase 2          | CLI / library cannot consume profiles  |
| Annotated HTML attribute names (`data-sync-id` etc.)              | Phase 2          | Sync engine cannot bind                |
| AlignmentMap on-disk JSON shape                                   | Phase 2          | Post-render consumers cannot integrate |
| Default OpenAI model + token budgets                              | Phase 1          | CLI defaults pick wrong model          |
| Smoke fixture set (sample `.md` files exercising all 14 SCNs)     | Phase 3          | SCN coverage cannot be verified        |
| Whether the renderer emits one HTML doc with both panes or two    | Phase 3          | Demo wiring shape                      |

## 15. References

- `references/draft.md` — original architecture document (sections cited inline above).
- `/Volumes/Common/QJoon/LLM-API/LLM-Trans/` — UX and pipeline reference. Do not mirror its sentence-level pairing.
- `/Volumes/Common/QJoon/resp-translator/` — workspace + design-first-architecture artifact reference. Mirror the artifact shapes; project goals differ.
