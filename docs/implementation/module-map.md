# Module Map

Every mandatory scenario from `docs/architecture/scenario-matrix.md` mapped to the binaries / modules / persistence / files / integrations that implement it. This is the Phase 2 → Phase 3 hand-off table.

## Crate / module layout

```
crates/transync/                        # curated public facade (semver firewall); depends on
│                                       #   transync-core only (DCR-0018)
├── Cargo.toml
├── tests/
│   └── public_surface.rs               # standing drift gate: facade list <-> contracts.md §0
└── src/
    └── lib.rs                          # EXPLICIT curated re-export list — no globs; the
                                        #   supported surface, mirrored by contracts.md §0

crates/transync-html/                   # HTML mechanics: tag scan + element extents + segment
├── Cargo.toml                          #   extract/splice + fragment balancing (DCR-0032);
└── src/                                #   lol_html + htmlize only, no workspace-member dep
    └── lib.rs                          # tag scan + element extents + segment extract/splice + balancing (DCR-0032)

crates/transync-syntax/                 # wasm32-compilable base crate (DCR-0017); no [features]
├── Cargo.toml                          #   gate: cargo check --target wasm32-unknown-unknown
└── src/
    ├── lib.rs                          # module roots only; no pipeline types
    ├── parser.rs                       # IR types + thin parse; WalkState walker (DCR-0019)
    ├── parser/
    │   ├── options.rs                  # Comrak GFM option set
    │   ├── classify.rs                 # NodeValue → BlockKind + the label tables
    │   ├── emit.rs                     # THE one emit + the OI-0033 empty-range guard
    │   ├── sections.rs                 # SectionStack — the one heading-scope algorithm
    │   ├── ranges.rs                   # byte-range recovery from Comrak nodes; CR-aware
    │   │                               #   line table matching comrak (OI-0033)
    │   ├── refdefs.rs                  # link-reference-definition pool (DCR-0013)
    │   └── depth.rs                    # THE one guarded hand-off of raw Markdown to
    │                                   #   comrak: nesting-depth refusal before parse
    ├── id.rs                           # BlockId assignment + source_hash + the two vocabularies (BlockKind, Spelling/SourceFormat)
    ├── intake.rs                       # the format seam: one intake per source format (D3)
    ├── intake/
    │   └── html.rs                     # HTML document intake: five-class classification,
    │                                   #   rule T, ids/sections/ast_path (ti 490d97 wave 3)
    ├── outcome.rs                      # per-block HtmlOutcome closure (DCR-0017 §3.1)
    ├── walk.rs                         # ONE shared top-level normalization + per-list item
    │                                   #   count; consumed by render AND validate::full_reparse
    ├── regen.rs                        # source-splice regen + safe fence re-wrap + offset projection
    ├── align.rs                        # AlignmentMap construction + JSON serialization
    ├── render.rs                       # one whole-document parse per pane → annotated HTML
    ├── render/
    │   ├── attrs.rs                    # data-sync-id / data-block-kind / data-order / data-fallback
    │   └── html_pane.rs               # the HTML-source pane derivation: strip -> inject-or-wrap
    │                                  #   -> balance -> group li (ti 490d97 wave 6, spec §8)
    └── error.rs                        # ParseError

crates/transync-core/                   # pipeline on top of transync-syntax, HTTP-free
├── Cargo.toml
├── profiles/
│   └── default.toml                    # embedded default profile
└── src/
    ├── lib.rs                          # TranslateOptions, translate(), translate_with_cache();
    │                                   #   pub(crate) re-exports of syntax's
    │                                   #   parser/id/regen/render/align (DCR-0018). Only
    │                                   #   cache/llm/profile stay pub; unit is
    │                                   #   #[doc(hidden)] pub for transync-openai's live smoke
    ├── unit.rs                         # build_batches() as thin orchestration (DCR-0019)
    ├── unit/
    │   ├── payload.rs                  # per-kind (payload, input_mode, constraints) strategy
    │   ├── budget.rs                   # profile + token + cap resolution → BatchBudget
    │   ├── context.rs                  # section_path, neighbor snippets
    │   ├── split.rs                    # oversize tables → row windows, pre-packing (DCR-0026)
    │   └── section.rs                  # partition_by_section: the heading partition (DCR-0027)
    ├── structure.rs                    # structural-fingerprint oracle (list topology,
    │                                   #   blockquote children, table shape); consumed by
    │                                   #   unit AND validate::per_kind (DCR-0019)
    ├── structure/
    │   └── labels.rs                   # node-kind / item-child / blockquote-child labels
    ├── batch.rs                        # sequential grouping by input + output token budget,
    │                                   #   run once per section by `unit` (DCR-0027) — the
    │                                   #   partition is one level up; packs retry rounds too
    ├── llm.rs                          # Translator trait + types
    ├── llm/
    │   └── prompt.rs                   # provider-neutral prompt assembly + schema (DCR-0015)
    ├── validate.rs                     # layered validation orchestrator
    ├── validate/
    │   ├── schema.rs                   # ID set / shape checks
    │   ├── per_kind.rs                 # table column count, list topology, fence info, ...
    │   ├── inline.rs                   # destination / code-span / raw-tag protection
    │   ├── fragment_reparse.rs         # parse the translated payload as the same kind
    │   ├── full_reparse.rs             # parse the regenerated full doc; anchor count, label
    │   │                               #   sequence, per-list item count (DCR-0017 Guard 1)
    │   └── full_rescan_html.rs         # the HTML layer-6 twin: ordered tag ledger, fresh
    │                                   #   segmentation (D9 per-<li>), gap byte-identity,
    │                                   #   boundary sanity; dispatched from finalize on
    │                                   #   Document.format (DCR-0036)
    ├── pipeline.rs                     # thin phase sequence over the submodules below, plus
    │                                   #   the glossary + output-budget preflights (DCR-0019)
    ├── pipeline/
    │   ├── policy.rs                   # PURE retry/fallback decision core — budgets,
    │   │                               #   dispositions, backoff; no I/O, no async (DCR-0019)
    │   ├── dispatch.rs                 # concurrency fan-out, per-batch round loop (retry
    │                                   #   rounds re-packed by `batch`), cache partition
    │   ├── finalize.rs                 # regen + the DCR-0004 full-reparse cascade
    │   ├── report.rs                   # validation report + summary tallies, and the
    │   │                               #   alignment-map assembly (DCR-0019's open seam)
    │   ├── retry.rs                    # ADR-0009 RetryContext side channel (`contracts.md` §5)
    │   └── merge.rs                    # row-window reassembly: one table block again
    │                                   #   before regen, so a window id never reaches
    │                                   #   the alignment map or the DOM (DCR-0026)
    ├── cache.rs                        # Cache trait + CacheKey identity + InMemoryCache
    ├── cache/
    │   └── disk.rs                     # DiskCache — the disk-backed store (DCR-0028);
    │                                   #   a §0 tier-(a) export since v0.4.0
    ├── profile.rs                      # TOML loader + ProfileMetadata builder
    └── error.rs                        # TransyncError + sub-errors (Parse wraps syntax's ParseError)

crates/transync-lang/                   # the source-language GATE: "should a run start?"
├── Cargo.toml                          #   ONE dependency (whichlang), NO workspace member —
└── src/                                #   that absence keeps it from becoming a second answer
    ├── lib.rs                          # Language / Verdict / Basis / Reason / Gate; the decision order
    ├── script.rs                       # the free half: an exclusive-script codepoint test that
    │                                   #   only answers where counting is conclusive
    ├── backend.rs                      # the ONLY file naming whichlang; wildcard-free mapping
    └── containment.rs                  # cfg(test): the weld proving backend.rs is the only one

crates/transync-openai/                 # default Translator impl
├── Cargo.toml
└── src/
    ├── lib.rs                          # public surface (TransyncOpenAI)
    ├── client.rs                       # flow only: SurfaceRequest + round_trip (DCR-0019)
    ├── client/
    │   ├── dispatch.rs                 # Api enum + model→surface heuristic
    │   ├── chat.rs                     # Chat Completions DTOs, bodies, output extraction
    │   ├── responses.rs                # the same for the Responses API
    │   ├── transport.rs                # post_json: send + size cap + accumulate
    │   ├── classify.rs                 # HTTP status / reqwest → ProviderError; Retry-After
    │   └── endpoint.rs                 # base-URL normalization
    ├── tokenizer.rs                    # tokenizer_hint_for_model (no HTTP)
    └── error.rs                        # ProviderError → TranslatorError mapping

crates/transync-cli/                    # binary; no profiles/ dir — the default profile
│                                       #   is transync-core's, reached via default_profile()
├── Cargo.toml
├── web/                                # CLI's local copy of demo assets, embedded
│   ├── index.html.tpl
│   ├── purify.min.js                   # vendored DOMPurify; include_str!'d by output.rs into
│   │                                   #   every --html-out bundle (pinned against web/vendor/)
│   └── sync.js                         # symlink/copy from workspace web/js/sync.js (build-time)
└── src/
    ├── main.rs                         # clap dispatch
    ├── translate_cmd.rs                # `transync translate` — args, the execute() seam,
    │                                   #   and one exit-code table in `execute` (DCR-0019);
    │                                   #   its provider arm defers to error.rs (ti e62b59)
    ├── translate_cmd/
    │   ├── args.rs                     # flag validation/resolution + OutputTarget
    │   ├── input.rs                    # capped reads + profile resolution
    │   ├── provider.rs                 # translator selection (incl. the cfg-gated stub)
    │   ├── publish.rs                  # ONE output-set builder + commit
    │   └── report.rs                   # quiet gate, diagnostics, verbose tally
    ├── serve_cmd.rs                    # `transync serve` — args, bind, accept loop,
    │                                   #   signal shutdown (ti b791d6)
    ├── serve_cmd/
    │   ├── route.rs                    # THE path-confinement pass: per-segment decode,
    │   │                               #   then canonicalize-and-contain (symlink escape)
    │   ├── mime.rs                     # fixed extension -> content-type table, never sniffing
    │   ├── conn.rs                     # one request per connection: read head, answer, close
    │   └── host.rs                     # which authorities this server answers for; a
    │                                   #   loopback bind is not an access control
    ├── output.rs                       # atomic write helpers; --html-out templating
    ├── output/
    │   └── lock.rs                     # cross-process exclusion for publication
    │                                   #   (R0001-0034) — staging alone did not cover
    │                                   #   the rename pass
    ├── direction.rs                    # bundle text direction, stamped at the PANE
    │                                   #   level (OI-0032)
    ├── logging.rs                      # where the library's `tracing` events go in the
    │                                   #   reference binary
    └── error.rs                        # ExitCode 0..7 + THE TransyncError -> code table
                                        #   (6 = fix the config, 7 = skip the doc; ti e62b59)

crates/transync-wasm/                   # browser (wasm-bindgen) surface (ADR-0019 / DCR-0020)
├── Cargo.toml                          #   cdylib ONLY (an rlib holds codegen units open and
│                                       #   costs ~100 KB of fat-LTO scope), publish = false;
│                                       #   depends on transync-syntax ALONE (charter + getrandom trap
│                                       #   in core's wasm32 tree). Same gate as syntax:
│                                       #   cargo check -p transync-syntax -p transync-wasm
│                                       #   --target wasm32-unknown-unknown
└── src/
    ├── lib.rs                          # ONLY the three #[wasm_bindgen] wrappers:
    │                                   #   render_pair / rebuild / schema_version
    └── engine.rs                       # ALL logic + ALL tests; zero wasm-bindgen types, so it
                                        #   runs on the host. JSON string in, one JSON string
                                        #   out; edit mode runs regen -> align -> render whole

web/                                    # workspace-level demo source-of-truth
├── index.html                          # standalone demo shell (used outside CLI's --html-out flow)
├── demo-wasm.html                      # WASM render+edit demo shell (Track C); imports js/wasm-demo.js
├── js/
│   ├── sync.js                         # vanilla ESM sync engine
│   └── wasm-demo.js                    # demo controller: local render, per-block edit loop,
│                                       #   DOMPurify-gated mounts, five fail-closed boot gates
├── wasm/                               # build output of scripts/build-wasm.sh — GITIGNORED
│                                       #   (transync_wasm.js glue + transync_wasm_bg.wasm;
│                                       #    the glue resolves the module as its own sibling,
│                                       #    which is why they share one directory)
└── tests/                              # headless Playwright (scn13.spec.js + wasm.spec.js
                                        #   + engine.spec.js, sync.js's mount contract driven
                                        #   directly rather than through a shell)

scripts/
├── smoke.sh                            # build + wasm gate + tests + rustdoc gate + build-wasm + CLI e2e
├── build-wasm.sh                       # wasm-pack --no-opt + explicitly resolved binaryen
│                                       #   wasm-opt (cached 117 rejects current rustc output);
│                                       #   enforces the size budget; loud prereq failures
├── test-browser.sh                     # CLI fixture + wasm demo leg + Playwright
│                                       #   (web/tests/scn13.spec.js + engine.spec.js
│                                       #    + wasm.spec.js — every spec under web/tests/)
└── hooks/pre-commit                    # fmt, clippy, the two-crate wasm gate
```

## Scenario → component coverage

Module names in the "Crates / modules touched" column are **engine** modules —
`transync-syntax` owns `parser` (+`options`/`classify`/`emit`/`sections`/
`ranges`/`refdefs`/`depth`), `id`, `regen`, `render` (+`attrs`), `align`,
`outcome`, `walk`; `transync-core` owns `unit`
(+`payload`/`budget`/`context`/`split`/`section`), `structure` (+`labels`),
`batch`, `llm` (+`prompt`), `validate` (+ its six layers), `pipeline`
(+`policy`/`dispatch`/`finalize`/`report`/`retry`/`merge`), `cache` (+`disk`),
`profile`. Since DCR-0018 they are **not**
reachable through the `transync` facade (only `cache`, `llm`, and `profile`
are), so they are written bare rather than as `transync::…` paths.

`transync-wasm` owns no engine module: it is a **consumer** that composes
`parser` / `id` / `outcome` / `regen` / `align` / `render` behind a JSON-string
boundary for the browser (ADR-0019), which is why it never appears in the
column below.

| SCN     | Description                                                | Crates / modules touched                                                                                  | Files / fixtures                                       | External integrations                              |
|---------|------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------|--------------------------------------------------------|----------------------------------------------------|
| SCN-01  | Heading + paragraph translation                            | `{parser, id, unit, batch, validate, regen, render, pipeline}`, `MockTranslator`               | `tests/fixtures/scn-01-headings-and-paragraphs.md`     | none                                               |
| SCN-02  | Whole-block GFM table                                       | `{parser, unit, validate::per_kind, regen, render}`                                             | `scn-02-table-small.md`                                | none                                               |
| SCN-03  | Oversized table → header-carrying row windows (DCR-0026)    | `{unit::split, batch, validate::per_kind, pipeline::merge, regen::{split_table_rows,regenerate_table}}` — the table is split at packing time and reassembled before regen | `scn-03-table-large.md`                                | none                                               |
| SCN-04  | Code block fence regen + comment translation                | `{parser, regen (fence sizing), validate::per_kind}`                                            | `scn-04-code-block.md`                                 | none                                               |
| SCN-05  | Nested list with task items                                 | `{parser, validate::per_kind (list topology), regen}`                                           | `scn-05-nested-list.md`                                | none                                               |
| SCN-06  | Blockquote with nested paragraphs                           | `{parser, validate::per_kind, regen}`                                                           | `scn-06-blockquote.md`                                 | none                                               |
| SCN-07  | Validation retry → success                                  | `{validate, pipeline::policy, pipeline::dispatch, pipeline::retry}`, `MockTranslator (rejects-then-accepts)`                          | `scn-07-validation-retry.md`                           | none                                               |
| SCN-08  | Persistent failure → fallback                               | `{pipeline::policy, pipeline::finalize, pipeline::retry, align (fallback_status), render (data-fallback)}`, `MockTranslator (always-fails-one-unit)` | `scn-08-fallback.md`                          | none                                               |
| SCN-09  | Prompt injection in source content                          | `{profile, unit (BlockContext), validate::schema}`                                              | `scn-09-prompt-injection.md`                           | recording stub provider                            |
| SCN-10  | Long doc → many batches; partial-resume                     | `{batch, pipeline}`                                                                             | `scn-10-long-document.md`                              | none                                               |
| SCN-11  | `source-language=auto` → detected echoed                    | `{align (detected_source_language), pipeline}`, `MockTranslator (echoes detected)`              | `scn-11-language-auto.md`                              | none                                               |
| SCN-12  | CLI end-to-end produces 4 outputs                           | `transync-cli::{main, translate_cmd, output}`, `transync-openai::*`, the `transync` facade                 | `scn-14-full.md` reused as input                       | OpenAI Chat Completions / Responses on a human-invoked live run; the `test-stub-provider` echo `Translator` in every automated run — see the table below |
| SCN-13  | JS demo sync                                                | `web/js/sync.js`, `{render, align}`                                                                       | output of SCN-12; `web/tests/scn13.spec.js`            | headless Chromium via Playwright (`scripts/test-browser.sh`) |
| SCN-14  | Full-document reparse                                       | `{regen, validate::full_reparse}`                                                               | `scn-14-full.md`                                       | none                                               |
| SCN-15  | HTML-content translation end-to-end (post-MVP)              | `transync-html`, `{parser, unit (html_outcomes), validate::per_kind, regen, render, align}`, `web/js/sync.js` (toggle mirror) | `scn-15-html-blocks.md`                 | none (`MockTranslator`); browser leg via Playwright |

## Persistence / file touches per scenario

| SCN     | Reads                                            | Writes                                                                          |
|---------|--------------------------------------------------|---------------------------------------------------------------------------------|
| SCN-01..11, SCN-14, SCN-15 | Fixture `.md` (test-only)        | none (in-memory `TranslationOutput` returned)                                   |
| SCN-12  | `scn-14-full.md`, embedded default profile        | `out.md`, `out.json`, `<html-out>/{index.html,source.html,target.html,sync.js,purify.min.js,alignment.json}` (staged writes) |
| SCN-13  | output of SCN-12; `sync.js`                       | none by the product itself (the browser only fetches, and a static server must serve the bundle). Under `scripts/test-browser.sh` the harness regenerates that bundle into its own temp workdir (`TRANSYNC_FIXTURE_WORKDIR`) each run and serves it on loopback. |

## Integrations per scenario

| SCN     | Real integration in an automated run?                                                                                                  |
|---------|----------------------------------------------------------------------------------------------------------------------------------------|
| SCN-01..11, SCN-14, SCN-15 | No — drive via `MockTranslator`. Deterministic. SCN-15's browser leg rides the headless Playwright suite (test **h**).       |
| SCN-12  | Never automatically — there is no CI to run one (see SCN-13). The automated path is the `test-stub-provider` echo `Translator`, which returns content unchanged: that is what `scripts/smoke.sh` and `scripts/test-browser.sh` build their bundles with. The live path is human-invoked and double-gated — `crates/transync-openai/tests/live_smoke.rs` is `#[ignore]`d *and* self-skips unless `TRANSYNC_LIVE_SMOKE=1` and a non-empty `OPENAI_API_KEY` are both set; `scripts/smoke-live-gate.sh` is the wrapper that opts in. |
| SCN-13  | Real browser, no CI — the repository ships no CI workflow, so this runs locally. `scripts/test-browser.sh` is the primary automated check: it regenerates a stub-provider CLI bundle and runs `web/tests/scn13.spec.js` headless (Chromium via Playwright), with the `web/SMOKE.md` manual checklist kept as fallback — the phrasing `mvp-scope.md` uses, which records the suite as **shipped 2026-07-13**. It is step 6 of `docs/project/release-checklist.md`. |

## Internal interfaces (essentials)

These are the shapes other modules call into — live signatures, named by their **owning crate**. Most are engine-internal since DCR-0018: `transync-core` re-exports the `transync-syntax` modules `pub(crate)`, and the `transync` facade exposes none of them. Only `cache` and `profile` below are curated API (`transync::cache`, `transync::profile`); the pipeline is reached through `transync::translate` / `transync::translate_with_cache`. A consumer that needs an engine internal depends on `transync-core` / `transync-syntax` directly — see `contracts.md` §0.

```rust
// transync-syntax::parser
pub fn parse(source: &str) -> Result<Document, ParseError>;
// The ONE guarded way this crate hands raw Markdown to Comrak: nesting-depth
// check, then NUL normalization. `parse` and the renderer both go through it,
// so a document accepted as a source is accepted as a target (R0002-0059).
pub fn intake(source: &str) -> Result<Cow<'_, str>, ParseError>;

// transync-syntax::id
pub fn assign_block_ids(doc: &mut Document);
// `None` when `block_index` is out of range — the hash is of the block's
// clamped source slice, so there is no hash to give for a block that is not
// there. Callers must not treat absence as a zero hash.
pub fn source_hash_block(doc: &Document, block_index: usize) -> Option<u64>;

// transync-core::unit (#[doc(hidden)] pub — engine-internal)
// Infallible again since DCR-0027: it was fallible only to raise the reserved
// `scope = "section"` refusal (ti 0ed6eb), and that refusal —
// `ProfileMetadata::ensure_supported` and `ProfileError::Unsupported` with it —
// retired when the scope became effective. The profile gates it still applies
// for callers who batch without crossing the translate boundary (glossary
// normalization, the `[batching]` ranges) are warn-only, like every profile
// check past the loader.
pub fn build_batches(
    doc: &Document,
    opts: &TranslateOptions,
    tokenizer_hint: Option<TokenizerHint>,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Vec<TranslationBatch>;

// transync-core::validate (pub(crate))
// `ref_defs` is the document's link-reference-definition pool
// (transync-syntax::parser::refdefs), forwarded to the INLINE layer so
// reference-style links resolve to real destinations before their identity is
// compared (design D2 §B4). Pass "" when the document defines none.
pub fn validate_batch(
    batch: &TranslationBatch,
    result: &TranslationBatchResult,
    ref_defs: &str,
) -> ValidatedBatch;

// transync-syntax::regen
// Reconstructs the translated MD by SOURCE SPLICING: walk top-level blocks in
// source order, copy inter-block text verbatim, and splice each block's
// accepted payload at its source range; code blocks are re-wrapped with a safe
// fence. A block ABSENT from `accepted` falls back to its exact source slice —
// absence IS the fallback contract (DCR-0017 M1). Returns the MD plus per-block
// byte offsets into that output (BlockOffsets) for the alignment map.
pub fn regenerate(doc: &Document, accepted: &HashMap<BlockId, String>) -> (String, BlockOffsets);

// transync-syntax::align
// Neutral parameters (DCR-0017 M2/M3): statuses and languages arrive as plain
// data, not as &[ValidatedBatch] / &TranslateOptions, so the module carries no
// pipeline types. The CALLER owes completeness: every translatable block must
// have a status here or its row silently reads `preserved`.
pub fn build_alignment_map(
    doc: &Document,
    statuses: &HashMap<BlockId, FallbackStatus>,
    offsets: &BlockOffsets,
    source_language: &str,
    target_language: &str,
    detected_source_language: Option<String>,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> AlignmentMap;

// transync-syntax::render
// ONE Comrak parse per pane (DCR-0017): the top-level AST node sequence is
// zipped against the alignment rows through transync_syntax::walk, and each
// row's inner HTML comes from formatting the paired node (its children for the
// strip-kinds, the whole node for table / code block / blockquote). No
// per-block reparse, no HTML string surgery. render_source parses
// doc.source_text; render_target parses translated_md — both through
// parser::intake, so a pane meets the same nesting ceiling and NUL rule a
// source document meets (R0002-0059).
//
// Fallible since R0002-0010 / R0002-0011: the map must describe the doc —
// one row per block, no repeated source_block_id — or the pane is refused.
// A duplicate used to let the last row win; an uncovered block used to
// vanish from both panes with no anchor, placeholder or warning.
// R0003-0060 extends the refusal from map SHAPE to range VALUES: a range
// that runs backwards, past the pane's end, or into the middle of a UTF-8
// character used to be clamped and snapped into an empty / truncated /
// character-short block under a correct anchor. Each pane checks what it
// slices (source_range vs source_text, target_range vs translated_md); an
// empty in-bounds range is fine (build_alignment_map's 0..0, R0003-0055).
pub enum RenderError {
    DuplicateRow { .. }, UncoveredBlock { .. },
    UnusableRange { pane: &'static str, ids: String }, Intake(ParseError),
}
pub fn render_source(doc: &Document, alignment: &AlignmentMap)
    -> Result<String, RenderError>;
pub fn render_target(doc: &Document, translated_md: &str, alignment: &AlignmentMap)
    -> Result<String, RenderError>;

// transync::cache (curated API)
pub trait Cache: Send + Sync {
    fn get(&self, key: &CacheKey) -> Result<Option<UnitResult>, CacheError>;
    fn put(&self, key: CacheKey, value: UnitResult) -> Result<(), CacheError>;
    /// Remove one entry. Absent keys are a successful no-op.
    fn evict(&self, key: &CacheKey) -> Result<(), CacheError>;
    // Plus get_document_meta / put_document_meta (DocumentMetaKey ->
    // DocumentMeta, DCR-0028 §3). Both are DEFAULTED — get answers Ok(None),
    // put discards — so a backend that ignores metadata is pre-v0.4.0
    // behaviour: degraded, never wrong. The three above are the required ones.
}
pub struct InMemoryCache { /* Mutex<HashMap> */ }

// transync::profile (curated API)
pub fn load_profile(toml: &str) -> Result<ProfileMetadata, ProfileError>;
pub fn default_profile() -> ProfileMetadata;  // CLI calls this when no --profile

// transync-core::pipeline (pub(crate); the orchestrator that the curated
// transync::translate / transync::translate_with_cache entry points wrap —
// translate_with_cache additionally preflights an empty target_language)
pub async fn run_pipeline<T: Translator + ?Sized>(
    source: &str,
    opts: &TranslateOptions,
    translator: &T,
    cache: &dyn Cache,
) -> Result<TranslationOutput, TransyncError>;
```

## Phase 2 hard-gate evidence

| Phase 2 requirement                                                         | Where satisfied |
|-----------------------------------------------------------------------------|-----------------|
| Every mandatory scenario mapped to binaries/modules                         | "Scenario → component coverage" table above |
| Every mandatory scenario mapped to persistence/files                        | "Persistence / file touches per scenario" table |
| Every mandatory scenario mapped to integrations                             | "Integrations per scenario" table |
| Essential persistence interfaces exist                                      | `Cache` trait + `transync::profile` loader (this file); see also `persistence-and-files.md` |
| Essential file-storage interfaces exist                                     | `transync-cli::output` atomic-write helpers; see `persistence-and-files.md` §atomic-write |
| Rough schema outlines exist where durable state lives                       | `rough-schema.md` (Profile TOML, AlignmentMap JSON, all wire types) |
| Source-of-truth for every concern recorded                                  | `source-of-truth-table.md` |
| Contract pack written                                                       | `contracts.md` |
