# MVP Scope

## Track

**Track D** (per Q1 brainstorming):

> Rust library + thin CLI binary + vanilla-JS demo. JS sync ships as an unpolished demo, not a supported package.

Track C (WASM rendering for browser-only consumers) is **post-MVP**.
**Shipped 2026-08-05 (ADR-0019 / DCR-0020)** as a *web demo*, not as a track
swap: `crates/transync-wasm` wraps `transync-syntax` for the browser and
`web/demo-wasm.html` renders both panes locally (plus a translated-block edit
loop), while the CLI's `--html-out` bundle deliberately carries **no** wasm.
The MVP track is still **D** — the wasm demo is an unpolished demo on the same
terms as the JS sync engine, and it is not a supported package.

## In Scope (MVP)

### Core library — `crates/transync`

- GFM parsing via Comrak (ADR-0004).
- Block IR with stable IDs and source ranges (ID format finalized Phase 2).
- Translation unit construction with section path + neighbor snippets.
- Section-coherent, sequential token-budget batch grouping (DCR-0027) — the unit list is partitioned at every heading, and each section is then packed in document order until the budget or the `max_units_per_batch` cap is hit; the consumer or `TranslateOptions` configures the token budget. **A batch never straddles a `##`/`###` boundary**; a section too large for one batch packs into several, all inside that section, which is the only thing that separates one section's units. Whole-section coalescing (two small sections sharing a batch) is deliberately not done — see DCR-0027 OQ-A.
- HTTP-free `Translator` trait (ADR-0002). The core crate has zero `reqwest` / `async-openai` dependency.
- Layered validation: schema → ID set → per-block-kind shape → fragment reparse → visible-text presence → inline protection → full-doc reparse.
- Retry then fallback to source; never silent corruption.
- Markdown regeneration via Comrak AST reserialization with safe fence regeneration for code blocks and structural reserialization for tables.
- Alignment map generation in a stable JSON shape (`schema_version` field; finalized Phase 2).
- Annotated HTML renderer with `data-sync-id` / `data-block-kind` / `data-order` / `data-fallback` attributes (attribute names finalized Phase 2).
- Translation cache keyed by `(provider_fingerprint, validation_schema_version, source_hash, source_lang, target_lang, profile_version, profile_prompt_hash, glossary_hash, model_id, block_kind, input_mode, context_hash, instruction_hash)` — thirteen fields; `CacheKey` in `crates/transync-core/src/cache.rs` is the live definition and `contracts.md` §5a carries the identity argument. `InMemoryCache` is still the default, but the cache is no longer in-memory-only: `DiskCache` shipped 2026-08-09 (DCR-0028 / ADR-0021), which is also why the key set is exhaustive by policy.

### Default provider — `crates/transync-openai`

- `Translator` impl with model-driven dispatch across both the OpenAI Chat Completions and Responses APIs + Structured Outputs (DCR-0005).
- `tiktoken-rs`-based token estimation (shipped). Provider-side batch pagination / split-on-oversize resubmit is **deferred** — tracked in `contracts.md` §5 and `stub-manifest.md`, no longer as code: the `pagination::split_oversize` no-op stub was deleted 2026-08-05 (DCR-0019) after its consumer-side counterpart went in the v0.2.0 curation. The deferral stands; only the dead placeholder is gone.
- Configurable `base_url` for OpenAI-compatible proxies (Azure OpenAI, OpenRouter, local llama.cpp servers).
- Bring-your-own `OPENAI_API_KEY`.

### CLI — `crates/transync-cli`

- `transync translate --input <md> --output <md> --map <json> --html-out <dir> --target-language <lang> [--source-language auto] [--profile <toml>] [--model <id>]`.
- `transync serve --rendered <dir>` — a loopback static file server for the bundle: `127.0.0.1:7470` by default, `GET`/`HEAD` only, path-confined to the served root (no traversal, no symlink escape), a fixed content-type table, Ctrl-C shutdown. It shipped **DEFERRED (STUB-061)** — a placeholder that always exited nonzero — and became real on 2026-08-09 (ticket `b791d6`); `contracts.md` §6 carries the contract. Any other static server still works, since the bundle is self-contained.
- A minimal default profile is embedded so a clean checkout runs end-to-end without a profile file.

### JS demo — `web/`

- `index.html` + `web/js/sync.js` (vanilla ESM, no framework, no build step).
- Loads alignment map + annotated HTML; mounts dual panes; drives RAF-coalesced scroll-listener block-ID sync (DCR-0008).
- DOMPurify-sanitizes annotated HTML pre-mount.
- Programmatic-scroll lock + RAF batching to suppress feedback loops.
- Reads anchor geometry per-frame during scroll (`offsetTop` / `offsetHeight` each RAF), which absorbs most incremental reflow. Dedicated recompute hooks **shipped 2026-08-09** (ticket `d3acc3`, closing OI-0024 item 1): a `ResizeObserver` on both panes, `document.fonts.ready`, and capture-phase `load`/`error` on `<img>` inside either pane, each coalescing into one animation frame that re-collects both anchor sets and re-drives the follower from the last driving pane. Collapsible expansion joined them as a fourth signal (R0004-0087, 2026-08-12): the `<details>` toggle mirror raises the recompute as well as mirroring, because a disclosure is a content reflow no observer reports and the two panes' disclosures rarely grow by the same height. `controller.refresh()` remains the escape hatch for a layout change none of the four covers. Caller-driven destroy-and-remount is now required only when a caller **replaces** a pane's HTML, which is not a reflow.
- Pairs the two panes by **identical `data-sync-id`** — a normative schema-1.x invariant since 2026-08-09 (ADR-0001 amendment; `contracts.md` §3), with the alignment map's source/target indirection reserved for a future divergence revision. A map whose row contradicts it is refused.

## MVP scenarios

`SCN-01` through `SCN-14`. See `scenario-matrix.md` for the canonical contract (Trigger / Input / Expected Output / Verification). `SCN-15` and `SCN-16` are in the same matrix but are **post-MVP** coverage — SCN-15 for the HTML-content translation wave below, SCN-16 for HTML→HTML document translation (ADR-0025, ti `490d97`) — not MVP gates.

## Post-MVP feature waves

Recorded here so the scope document stays a truthful description of the shipped
system rather than of the 2026-05-01 MVP alone.

### HTML-content translation — shipped 2026-08-04 (ADR-0018 / DCR-0016)

Block-level raw HTML moved from "preserved, escaped placeholder" (DCR-0013) to a
**translatable kind**. The application extracts entity-decoded text segments with
`lol_html`, sends the LLM only an ordered JSON array of strings (tags never
appear in the translatable payload; neighbor-context summaries may carry raw
markup as untrusted context data), and splices translations back per text node —
so markup is preserved by construction and an echoed segment is byte-identical to
its source.
Successful blocks **live-render** in both panes inside the standard `<div>` sync
wrapper through the shells' DOMPurify fail-closed mount; the escaped
`<pre data-skipped="html-block">` placeholder is now the **failure** presentation
only. Inline raw-HTML tags gained an **always-on** verbatim tag-identity guard.
Alignment schema `1.1.0` → `1.2.0` (additive `html` `block_kind`);
`VALIDATION_SCHEMA_VERSION` `1` → `2`. Coverage: `SCN-15` end-to-end plus
Playwright test **h** in `web/tests/scn13.spec.js`. This amends **invariant 7**
in `CLAUDE.md`.

Accepted limits (documented, not defects — the full list is in DCR-0016 Part D):

- **`<details>` fold not reproduced.** CommonMark ends a type-6/7 HTML block at
  the first blank line, so an interleaved `<details>` region is *several*
  fragments; the render path auto-balances each one, which yields a live
  translated `<summary>` above an **always-visible** body.
- **Per-segment quality ceiling.** Text-node granularity means a sentence split
  by inline tags is translated in pieces; word-order reflow cannot cross a
  segment boundary. Mitigated by a prompt instruction, not eliminated.
- **Entity-form drift in `out.md`.** A genuinely translated segment re-escapes
  only `<`, `>`, `&`, so `&nbsp;` becomes a literal U+00A0. Identity-skipped
  (echoed) segments are exempt and stay byte-exact.
- **Attribute text is never translated** (`alt`, `title`, `aria-label`), and
  HTML nested inside list items or blockquotes is covered only by the coarse
  child-kind topology labels.
- **Zero-segment blocks may render as nothing** (comment-only, `script`-only —
  the sanitizer strips them at mount, matching GitHub). An info-level warning
  row keeps the disappearance visible in the report.

### `transync-syntax` crate split + AST-direct renderer — shipped 2026-08-04 (DCR-0017)

Structural, not a feature: the syntax layer (`parser`, `id`, `regen`, `render`,
`align`, `htmlseg`, plus the new `outcome` and `walk` modules) became the
workspace's fifth member, `crates/transync-syntax`, which compiles for
`wasm32-unknown-unknown` behind a standing gate. `transync-core` keeps the
pipeline and re-exports the moved modules, so no `transync::…` path moved.
This resolves **OI-0028** — the *compile path* for track C, not track C itself
(see the DEFERRED table below) — and clears **OI-0008**'s two renderer items:
the renderer now does **one** whole-document Comrak parse per pane instead of
reparsing every block as an isolated fragment, and `strip_outer_wrapper`'s
string surgery is gone.

Accepted output changes (the full list is in DCR-0017):

- **Loose lists render loose in both panes.** A source list whose items are
  blank-line separated now renders `<li><p>…</p></li>`, matching CommonMark;
  per-item fragment reparse used to flatten it to tight. Source-driven —
  `validate::per_kind` still rejects translation-introduced looseness.
- **Loose task items** keep Comrak's `<input …/>`-then-`<p>` shape verbatim.
- **Bugfix:** 4-space-indented code blocks used to render as `<p>` (the
  pre-reparse `md.trim()` destroyed the indent that made them code blocks).
- Attribute names/order, wrapper elements, the one-block-per-line shape,
  `out.md`, the alignment map, and both `sync.js` copies are unchanged.

## Default settings (locked at Phase 1)

| Setting                       | Value |
|-------------------------------|-------|
| Markdown dialect              | GitHub Flavored Markdown |
| Parser                        | Comrak |
| Profile format                | TOML |
| Renderer error policy         | strict — failed fragments use retry/fallback chain; never silent corruption |
| HTML sanitization (JS demo)   | DOMPurify |
| Default OpenAI model          | `gpt-5-chat-latest` (override via `TRANSYNC_OPENAI_MODEL` or `--model`; if your project lacks access, try `gpt-4o-2024-11-20`) |
| Default request max tokens    | `12_000` (alias-tunable) |
| Default `target_output_tokens`| `8_000` per page |
| Default max segments per page | `8` (low for MVP, raise after fixture-driven tuning) |
| Default split retries         | `3` per oversized page |
| License                       | MIT (single) |
| Rust edition                  | `2024` (workspace pins `rust-version = "1.88"` — let-chains, not the edition, set the floor; `transync-cli` pins 1.89 for the std file lock) |

## Out of scope (DEFERRED)

| Item                                                | Reason |
|-----------------------------------------------------|--------|
| WASM renderer **in the CLI bundle**                 | Post-MVP track C. **Advanced 2026-08-04 (DCR-0017):** the renderer's crate (`transync-syntax`) compiles for `wasm32-unknown-unknown` under a standing gate. **Landed 2026-08-05 (ADR-0019 / DCR-0020) for the web demo only:** `crates/transync-wasm` ships wasm-bindgen entry points and `web/demo-wasm.html` renders both panes in the browser. What stays deferred — deliberately, not for lack of time — is **bundle integration**: the module is ~41× the entire 42 KB JS payload, and a `--html-out` bundle already ships the rendered HTML that the module's view mode reproduces. The ADR-0006 six-file contract is unchanged. |
| `transync-anthropic` / `transync-local-llama` crates| **`transync-anthropic` LANDED 2026-08-10 (DCR-0029, ticket `bda471`, SL-115..SL-119).** It is a workspace member implementing `Translator` over the Anthropic Messages API (`TransyncAnthropic`), with the same offline/live test split as the OpenAI adapter (`tests/offline_end_to_end.rs`, `tests/live_smoke.rs`); it is the first evidence for ADR-0002's claim that a provider costs the core trait nothing. The facade does not re-export it and the reference CLI stays OpenAI-backed, by DCR-0029's scope. `transync-local-llama` **stays deferred**: trait in place; that sibling crate lands later. |
| Disk-backed translation cache                       | **RESOLVED 2026-08-09 (DCR-0028 / ADR-0021).** In-memory was correct for v1 and `InMemoryCache` is still the default; what changed is that the deferral's own premise — resp-translator ADR-0002's "results are not durably valuable" — does not hold here, where an entry is paid-for provider output over a whole document. `transync::DiskCache` ships a versioned JSON-lines log behind the unchanged lookup seam, `transync translate --cache-dir` opens it, and §5b's "the paid-for progress lives in the cache" promise now survives the process. |
| Streaming translation (per-batch SSE)               | Post-MVP |
| Sentence-level sub-anchors                          | Draft NG3 |
| Live-edit re-anchoring                              | Draft NG4. **Still deferred after 2026-08-05 (DCR-0020):** the wasm demo edits **per-block payloads** of the translated side only, so the block set is structurally preserved — identity comes from parsing the source document, which the demo never edits. ID survival across *structural* edits remains a non-goal (invariant 8). |
| MDX / YAML frontmatter / math syntax                | Draft NG2 |
| Raw HTML                                            | **Block-level raw HTML shipped 2026-08-04** (ADR-0018 / DCR-0016) — see "Post-MVP feature waves" above. Inline raw HTML is guarded, not translated; MDX/frontmatter/math stay NG2 |
| Glossary editor UX                                  | Post-MVP |
| Playwright-driven JS sync tests                     | **Shipped 2026-07-13** (EXT-2026-07 P2-10): `scripts/test-browser.sh` runs `web/tests/scn13.spec.js` headless; manual smoke kept as fallback |
| Korean / Japanese sibling docs (`*.ko.md`)          | Out of scope per global feedback memory |

## Accepted scope limitations (DR-2026-07)

Two limitations surfaced by the 2026-07 design review, recorded here so they
are not mistaken for bugs:

- **Tail-of-document alignment.** The sync engine aligns panes on a *top*
  reference line (the active block's top edge). When the source and target
  panes have asymmetric total heights — inevitable, since translation changes
  content length — a top-reference design cannot simultaneously align both the
  tops *and* the bottoms of the two documents; the last blocks drift relative
  to each other near the very end of a scroll. This is **inherent** to
  top-reference scroll sync, not a defect, and is **accepted** — bottom
  alignment would require a second reference mode that fights the top one.
- **RTL target rendering — in scope since 2026-08-03 (OI-0032 / DCR-0015),
  with a documented best-effort limit.** The bundle now stamps
  ` dir="rtl"` on a pane when direction resolves to RTL; LTR emits no
  attribute at all (the HTML default), so existing ko/ja/en bundles are
  byte-identical. Precedence is `--target-direction` > profile
  `[render].target_direction` > `auto`, where `auto` matches the target
  label's primary subtag against a 15-entry RTL table. Because labels stay
  opaque (ADR-0013, amended), the table is a **presentation hint, not
  detection**: an expressive label ("Korean (formal)", "العربية") is not
  tag-shaped and therefore resolves LTR — the explicit flag or profile key is
  the authoritative path for those. Direction is pane-level and bundle-only:
  `out.md` and the alignment map carry none of it (direction drove no schema bump), and
  the sync engine is direction-agnostic, so `sync.js` is untouched.

## Definition of "MVP working"

- `cargo build --workspace` clean from a fresh checkout with only a Rust toolchain.
- `cargo test --workspace` green.
- All 14 scenarios pass per `scenario-matrix.md`.
- `cargo run -p transync-cli -- translate ...` produces translated MD + alignment JSON + annotated HTML for a representative sample document.
- The output bundle, served by `transync serve` (an external static server also works — the bundle is self-contained), makes SCN-13 sync work in a browser — verified by the automated headless suite `scripts/test-browser.sh` (`web/tests/scn13.spec.js`), which drives that same server, with the `web/SMOKE.md` manual checklist as fallback. *(This bullet said "served by an external static server … `transync serve` is DEFERRED per STUB-061 and exits nonzero" until 2026-08-09, ticket `b791d6`, when the deferral closed.)*
