# OI-0027 Public-Surface Hardening — Design Spec

- **Date:** 2026-08-04
- **Status:** owner-approved design (approach 1; sections S1–S6 approved 2026-08-04)
- **Issue:** OI-0027 (`docs/project/open-issues.md`) — the 0.2.0 window-closing step
- **Inherits:** DCR-0017 Part B widened-surface inventory (three narrowing candidates,
  six rustdoc warnings, `GeneratorMeta` M6 constraint) and the hook JS-section back-port
  chore. The other two DCR-0017 chores (facade `Cargo.toml` header, Playwright loose-list
  target-pane pin) already landed in `3026d1a` / `255c35f`.
- **Sequencing:** HTML feature (landed) → transync-syntax split (landed) → **this wave** →
  0.2.0 release. The release itself is out of scope here.

## 0. Owner decisions

### 0.1 Standing decision (2026-08-03, `open-issues.md` OI-0027)

1. Three-tier boundary: (a) first-class supported API, (b) provider-SDK layer
   (`llm::prompt`, `profile::render_prompt_body`) documented with a "prompt *text* is not
   a contract" note, (c) hidden internals (`parser`/`id`/`unit`/`batch`/`regen`/`render`/
   `validate` and `pipeline::run_pipeline`).
2. `translate_with_cache` promoted to first-class; scenario tests move onto it.
3. `#[non_exhaustive]` on `TranslatorError`/`TransyncError`/`ParseError` and
   `TranslateOptions`; `TranslationUnit` gets a builder/constructor helper.
4. A surface-drift test (facade re-export list ↔ contracts.md) lands with the curation.

### 0.2 Clarifications decided 2026-08-04 (this design)

1. **resp-translator migrates in the same wave** (its production `run_pipeline` call, its
   `ProfileMetadata` literal, its `TranslateOptions` FRU). Call sites must not restate
   library defaults as literals — unspecified fields flow from the library's `Default`.
2. **`TranslateOptions` strategy = `#[non_exhaustive]` + default-then-assign.** The
   standing decision's parenthetical ("construction via `..Default::default()`") is
   unsound: a non-exhaustive struct cannot be built by any struct expression from another
   crate, *functional-update syntax included* (E0639). All 28 downstream construction
   sites are FRU and must be rewritten to `let mut o = TranslateOptions::default(); o.x = …`.
   A builder is *not* added now; it can be added additively later if ergonomics demand it.
3. **`ProfileMetadata` + `ProfileConstraints`/`ProfileBatching`/`ProfileRender` also get
   `#[non_exhaustive]`** (not in the standing list; motivated by the E0063 breakage
   resp-translator absorbed on 2026-08-04 when `auto_glossary`/`render` were added).
4. **The facade's `pub use transync_syntax;` is removed.** Consumers that need syntax
   internals depend on `transync-syntax` directly — the split's stated philosophy.
5. **Approach = compiler-enforced closure** (approach 1): core tier-(c) modules go
   `pub(crate)`, the facade glob becomes an explicit re-export list. Doc-only hiding
   (approach 2) and a `cargo-public-api` snapshot gate (approach 3) were rejected — the
   former closes tier (c) on paper only; the latter adds an install-dependent gate, the
   "silently skips" failure mode DCR-0017 explicitly rejected. `cargo public-api` remains
   available as a *manual* release-ritual step, documented, never a test.

### 0.3 Naming correction

The standing decision's tier (a) names `TranslateOutput`; no such type exists. The type is
**`TranslationOutput`** (6 fields). Records are corrected; the type is not renamed.

## 1. Closure map

### 1.1 `transync-core` — module dispositions

| module | disposition | notes |
|---|---|---|
| `batch` | `pub(crate) mod` | Seals the `tiktoken_rs::CoreBPE` leak (three public fns returned a foreign type). `OutputBudgetWarning` **moves to `validate`** first — see §1.2. |
| `validate` | `pub(crate) mod` | Root re-exports (ValidationReport family) are unchanged — re-exporting `pub` items from a private module is the standard facade pattern. Gains `OutputBudgetWarning`. |
| `error` | `pub(crate) mod` | `TransyncError` stays root-re-exported; `ParseError` is added to the root re-exports. |
| `pipeline` | `pub(crate) mod` | After the §2 migrations. `pipeline::retry`'s four public no-op stubs become crate-private with it. |
| `unit` | **`#[doc(hidden)] pub mod`** | The one module that cannot fully close: `live_smoke.rs` consumes `build_batches`/`html_outcomes` cross-crate via a dev-dependency on `transync-core` (§2.3). Same pattern as `htmlseg` — pub only because a crate boundary forces it, hidden because it is not API. |
| `llm` (incl. `llm::prompt`) | `pub mod` (tiers a+b) | All nine `llm::prompt` items stay public — `transync-openai` consumes every one. Doc notes added: prompt/instruction *text* is not a contract (only schema/parse shape is). |
| `profile` | `pub mod` | Consumed by module path from the CLI, resp-translator, and the openai test fixture; carries tier-(b) `render_prompt_body`. Structs harden per §3. |
| `cache` | `pub mod` (tier a) | Unchanged. |
| `test_stub` | `pub mod` behind `test-stub` feature | CLI consumer; documented as test-only surface in contracts.md §0. |

### 1.2 `OutputBudgetWarning` relocation

`batch::OutputBudgetWarning` is the element type of tier-(a)
`ValidationReport.output_budget_warnings`; resp-translator reads three of its fields and
the CLI uses its `Display`. It moves to `validate` (where the report lives) and joins the
root re-exports. No consumer names the type today, so the move itself breaks nothing.
The `output_budget_warnings(…)` computation fn stays in `batch` (crate-private) and
imports the type from `validate`, reversing the direction of the current
`validate → batch` type import.

### 1.3 Syntax re-exports in core — demote, don't delete

`pub use transync_syntax::{align, id, parser, regen, render};` becomes
**`pub(crate) use`** — core-internal `crate::parser::…`/`crate::regen::…` paths keep
compiling; the external `transync::parser`/`transync::id`/`transync::align`/
`transync::regen`/`transync::render` paths disappear. Root aliases cover the wire types:

- kept: `BlockId`, `BlockKind`, `AlignmentBlock`, `AlignmentMap`, `ByteRange`,
  `FallbackStatus`, `GeneratorMeta`, `SyncRole`, `ValidationSummary`
- **added:** `ALIGNMENT_SCHEMA_VERSION` (sole consumer `sync_js_drift.rs` migrates to the
  root alias), `ParseError`, `GlossaryScope` (type of a pub field on tier-(a)
  `GlossaryEntry`, previously nameable only as `transync::llm::GlossaryScope` — the
  module path also remains valid), `OutputBudgetWarning` (post-move)

### 1.4 `transync-syntax` narrowings (DCR-0017 candidates, confirmed by census)

`htmlseg::balance_fragment`, `outcome::is_translatable`, `walk::label_for` →
`pub(crate)`. All three have only same-crate consumers. The `label_for` narrowing
creates a seventh rustdoc warning (`node_label`'s doc links it) — de-linked in §5.

### 1.5 Facade — explicit curation

`crates/transync/src/lib.rs` drops both lines (`pub use transync_core::*;` and
`pub use transync_syntax;`) and gains the explicit list:

- **modules:** `pub use transync_core::{cache, llm, profile};` plus
  `#[cfg(feature = "test-stub")] pub use transync_core::test_stub;`
- **functions:** `translate`, `translate_with_cache`
- **root types:** `TranslateOptions`, `TranslationOutput`, `FullReparseFailure`
- **cache cluster:** `Cache`, `CacheError`, `CacheKey`, `InMemoryCache`
- **errors:** `TransyncError`, `ParseError`
- **Translator-contract cluster (root aliases, as today plus `GlossaryScope`):**
  `BatchId`, `BlockConstraints`, `BlockContext`, `DEFAULT_MAX_AUTO_GLOSSARY_TERMS`,
  `GlossaryEntry`, `GlossaryExtractionRequest`, `GlossaryScope`, `InputMode`,
  `ListTopologyEntry`, `MAX_EXTRACTION_SOURCE_BYTES`, `OutputKind`,
  `ProviderFingerprint`, `RetryContext`, `TableAlign`, `TokenizerHint`,
  `TranslationBatch`, `TranslationBatchResult`, `TranslationUnit`, `Translator`,
  `TranslatorError`, `UnitResult`
- **profile root aliases:** `MergedGlossary`, `ProfileError`, `ProfileMetadata`,
  `merge_auto_glossary`
- **alignment wire:** `AlignmentBlock`, `AlignmentMap`, `ALIGNMENT_SCHEMA_VERSION`,
  `ByteRange`, `FallbackStatus`, `GeneratorMeta`, `SyncRole`, `ValidationSummary`
- **id wire:** `BlockId`, `BlockKind`
- **validation family:** `AttemptOutcome`, `AutoGlossaryReport`, `AutoGlossaryStatus`,
  `BatchFault`, `OutputBudgetWarning`, `UnitValidationRecord`,
  `VALIDATION_SCHEMA_VERSION`, `ValidationLayer`, `ValidationReport`

Satellite types stay nameable through the public `llm` module without root aliases:
`HeadingSnippet`, `NeighborSnippet`, `HtmlSegmentConstraints`, `ListDelimiter`, and the
nine `llm::prompt` items. After this change, an accidental new `pub` in core no longer
leaks through the facade; adding surface becomes a deliberate facade edit — the semver
firewall doing its job.

## 2. Migrations

### 2.1 `scn_10_long_document.rs`

Import + seven `run_pipeline` calls → `translate_with_cache`, name-for-name (signatures
are identical; both take `(source, &TranslateOptions, &T, &dyn Cache)`).

### 2.2 resp-translator (separate repo, committed there)

1. `bins/copy-transfer-mcp/src/translation.rs` — `transync::pipeline::run_pipeline(…)` →
   `transync::translate_with_cache(…)`. Behavioral delta, recorded in the migration note:
   an empty/whitespace `target_language` now returns `TransyncError` with
   `stable_code() == "internal"` instead of proceeding. The profile bridge always sets it,
   so the delta is inert today — and fail-fast is the better behavior.
2. `bins/copy-transfer-mcp/src/translation/profile_bridge.rs` — the nine-field
   `ProfileMetadata` literal → default-then-assign. **The hardcoded
   `auto_glossary: None` / `render: ProfileRender::default()` lines are deleted**;
   unspecified fields flow from `ProfileMetadata::default()` (= `default_profile()`), and
   the call site assigns only the fields it owns (slug, version, prompt_body, glossary,
   constraints, batching, load_warnings). Behavior-preservation check before landing:
   `default_profile()`'s `auto_glossary` and `render` must equal the values the bridge
   previously hardcoded (`None` / `ProfileRender::default()`) — true at HEAD; the
   migration must not change what the bridge produces.
3. Same file, `to_transync_options` — the `TranslateOptions` FRU → default-then-assign.

### 2.3 `live_smoke.rs` (transync-openai)

`transync-openai` gains **dev-dependencies** on `transync-syntax` and `transync-core`
(dev-deps do not enter the published dependency graph and do not affect the wasm gate,
which constrains only `transync-syntax`'s own manifest). The test then uses
`transync_syntax::parser::parse` and `transync_core::unit::{build_batches, html_outcomes}`
directly — an honest manifest-level declaration that this test reaches into engine
internals. The `Document` type is identical on both paths (core re-exports syntax's).

### 2.4 `TranslateOptions` FRU sites (28)

- ~20 facade scenario/test sites share the shape `{ target_language: "ko", ..default }`
  → one shared helper in the facade `tests/common` (constructed via default-then-assign;
  call sites needing more fields mutate the returned value).
- Remaining sites (boundary/error_fixtures/reader_honesty/inline_protection extras, CLI
  `translate_cmd.rs` — already `let mut`, openai `live_smoke.rs`, resp-translator
  `to_transync_options`) rewrite individually to default-then-assign.

### 2.5 Path migrations

- `transync::id::{BlockId, BlockKind}` module-path users (four sites: openai client test
  ×2 incl. one inline `transync::id::BlockKind::Paragraph`, facade `mock_translator`,
  `scn_10`) → root aliases.
- `sync_js_drift.rs` → `transync::ALIGNMENT_SCHEMA_VERSION`.
- `scn_15_html_blocks.rs`'s `transync::cache::InMemoryCache::default()` keeps working
  (`cache` stays public) — no change required.

## 3. `#[non_exhaustive]` set + `TranslationUnit` constructor

### 3.1 Enums — free by census

`TransyncError`, `TranslatorError`, `ParseError` get the attribute. Every cross-crate
match already carries a wildcard (`_`, a binding arm, or `matches!`), and thiserror's
generated impls plus in-crate exhaustive matches are unaffected — zero migrations.
`TransyncError::stable_code()` **stays an exhaustive in-crate match with no `_` arm**:
that is the device that forces a stable code for every new variant. Its seven return
strings are an inter-process contract (resp-translator's listener hard-codes them with no
cargo edge); contracts.md gains the code vocabulary as a documented, versioned list.

### 3.2 Structs

`TranslateOptions`, `ProfileMetadata`, `ProfileConstraints`, `ProfileBatching`,
`ProfileRender` get the attribute. Migrations per §2.2/§2.4; every other consumption path
of the profile structs is `Deserialize` + `Default`, which the attribute does not touch.

### 3.3 `TranslationUnit`

Gets `#[non_exhaustive]` **and** a constructor helper (per the standing decision — for
this type `Default` is meaningless because the required fields are its essence):

```rust
TranslationUnit::new(unit_id, block_kind, input_mode, source_payload, source_hash)
    .with_context(ctx)        // default: BlockContext::default()
    .with_constraints(c)      // default: BlockConstraints::default()
    .with_batch_id(b)         // default: BatchId::new(0) — today's placeholder semantics,
                              // the batcher overwrites it per chunk
    .with_retry(r)            // default: None — first-dispatch contract
```

`source_hash` is positional-required deliberately: it is a `CacheKey` axis with no
meaningful zero — a defaulted `0` would silently alias every unit's cache entry. In-crate
literal constructions remain legal and unchanged. The single external literal (the openai
`#[cfg(test)]` fixture) migrates to the constructor.

### 3.4 Explicitly exhaustive by policy (documented in contracts.md)

- `UnitResult`, `TranslationBatchResult` — *produced* by every downstream mock/provider
  translator (eight impls in the census); `#[non_exhaustive]` would break all of them.
  Field additions are breaking-by-policy.
- `TranslationBatch` — produced by core only, by policy; documented rather than enforced
  (enforcement would break only the openai prompt-test fixture, not worth a helper today).
- `GlossaryEntry` — embedded in resp-translator's own `Serialize + Deserialize` public
  type and its operator-edited profile TOML; its serde shape is third-party wire format.
  Field additions must be `#[serde(default)]` and are breaking-by-policy for literals.

## 4. Surface-drift test + contracts.md

### 4.1 `crates/transync/tests/public_surface.rs` (new)

1. **Compile-as-assertion:** a module whose body is `use transync::{…};` naming the full
   curated surface (§1.5 list + tier-(b) `llm::prompt` items and
   `profile::render_prompt_body`), `#[allow(unused_imports)]`. Compilation is the proof
   that every documented path resolves *through the facade*; a regression fails with a
   name-resolution error naming the missing item.
2. **`const DOCUMENTED: &[&str]`** mirroring (1).
3. **Doc-table scrape:** contracts.md gains a `§0 Public Rust surface` table; the test
   parses its rows (reusing the `docs_index_drift.rs` `repo_root()` idiom) and asserts
   `BTreeSet` equality with `DOCUMENTED` in both directions — orphans and dead rows both
   fail.
4. **Negative doc assertion:** no §0 row may name `pipeline`, `parser`, `unit`, `batch`,
   `validate`, `regen`, `render`, or `transync_syntax` — keeps the doc honest about
   tier (c).
5. **`GeneratorMeta` guard (DCR-0017 M6):** assert
   `AlignmentMap::default().generator.version == env!("CARGO_PKG_VERSION")` (facade's
   version). Fails the instant any workspace crate pins its own version, converting M6's
   silent-skew hazard into a red test. No signature or wire change; the caller-supplied
   `GeneratorMeta` redesign was considered and rejected as overkill now that
   `build_alignment_map` is no longer facade-reachable.

### 4.2 contracts.md edits

- New **§0 Public Rust surface**: the curated table (grouped as in §1.5), plus a
  test-only note for `test_stub` and a tier-(b) subsection for `llm::prompt` (all nine
  items named — this also fixes the existing drift where only four were listed) and
  `profile::render_prompt_body`.
- §1 stability text flipped: `#[non_exhaustive]` on the three enums is now true; the
  "additive-variant promise" holds. `TranslationUnit`'s "exhaustive by policy" paragraph
  is replaced by the §3.3 constructor policy. §3.4's exhaustive-by-policy set is stated.
- `stable_code()` vocabulary table added (the seven codes), marked as an inter-process
  contract.
- Prompt-text-is-not-a-contract note extended to the extraction items
  (`EXTRACTION_SYSTEM_PROMPT` in particular).

## 5. rustdoc hygiene

- The six inherited syntax warnings: de-link per the `parser/refdefs` precedent — four
  doc-block edits (`htmlseg` module doc, `extract`, `splice`, `render` module doc),
  `[`x`]` → `` `x` ``, zero API change. Plus the seventh created by the `label_for`
  narrowing (`walk::node_label`'s doc).
- Core links into newly-hidden modules, de-linked or retargeted in the same pass:
  `TranslateOptions::model_id` doc → `batch::encoder_for` (prose);
  `translate` doc → `pipeline::run_pipeline` (prose, "the internal pipeline");
  crate root doc `validate::BatchFault` → root alias `[BatchFault]`;
  `Translator::tokenizer_hint` doc → `crate::batch::encoder_for` (prose);
  `ProfileBatching` doc → `crate::batch::DEFAULT_OUTPUT_EXPANSION_FACTOR` (prose).
  (`Translator::extract_glossary` → `crate::profile::merge_auto_glossary` stays a live
  link — `profile` remains public.)
- **Gate:** `cargo doc --no-deps` warning-free for `transync-syntax`, `transync-core`,
  and `transync` is a completion criterion of this wave.

## 6. Inherited chore — hook JS-section back-port

The active `.git/hooks/pre-commit` carries an improved JS section (package-root discovery
under `web/`, biome-first with prettier/eslint fallback, skip-with-notice when a tool is
not installed) that the tracked `scripts/hooks/pre-commit` lacks. Back-port it so the
tracked copy is canonical, then refresh the active copy via `scripts/install-hooks.sh` so
both are identical. The wasm-gate lines in both copies are untouched.

## 7. Verification plan

- Full standing gate set at every landing point: `cargo fmt`, `cargo clippy --all-targets
  -- -D warnings`, `cargo test --workspace -- --test-threads=4`, the wasm gate
  (`cargo check -p transync-syntax --target wasm32-unknown-unknown`), `scripts/smoke.sh`,
  Playwright SCN-13 suite.
- New: `public_surface.rs` green (all five parts); `cargo doc --no-deps` warning-free for
  the three library crates.
- resp-translator: `cargo check` + its test suite green in that repo after §2.2, committed
  there with a migration-note commit message.
- Structural greps as review aids (not tests): no `pub use transync_core::*`, no
  `pub use transync_syntax` in the facade; no `tiktoken_rs` type in any public signature.

## 8. Records

- **DCR-0018** documenting this wave: the closure map, the migration list (including the
  resp-translator commit), the non_exhaustive set, and the drift test.
- **OI-0027 → RESOLVED** with a verification checklist mirroring §7.
- **DCR-0005** dated note: the facade firewall is now curated (this wave realizes its
  design intent).
- contracts.md §0/§1 edits are part of the work itself (§4.2).
- `docs/project/status.md`, `docs/project/phase-state.yaml` updated at landing.
- `docs/implementation/module-map.md` re-checked for stale "public surface" comments.

## 9. Out of scope

- `transync-syntax`'s own crate surface (its comrak-type leaks — `comrak_options`,
  `node_label`, `direct_item_count`, `byte_range_for`) — Track C's concern.
- Track C proper (wasm-bindgen entry points, browser integration).
- `pipeline::retry` stub redesign and the other OI-0008 residual refactors (the stubs
  become invisible with `pipeline`; the refactor debt remains tracked in OI-0008).
- OI-0033 (lone-CR `LineOffsets`).
- The 0.2.0 release ritual itself (version pin, changelog, publish decision) — the next
  step after this wave lands.
- Alignment schema (stays 1.2.0), `VALIDATION_SCHEMA_VERSION` (stays 2), `CacheKey`,
  both `sync.js` copies: all unchanged.
