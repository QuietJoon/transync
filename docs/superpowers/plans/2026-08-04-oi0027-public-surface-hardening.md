# OI-0027 Public-Surface Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the 0.2.0 breaking window by curating the facade to an explicit, compiler-enforced public surface, hardening evolvable types with `#[non_exhaustive]`, migrating the three live internal-path consumers, and pinning the surface with a drift test against contracts.md.

**Architecture:** Approved spec: `docs/superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md` (read it before your task; each task cites its sections). Order matters: the `#[non_exhaustive]` attributes MUST land in the same commit as the construction-site rewrites (clippy's `field_reassign_with_default` fires on default-then-assign against an exhaustive struct — probe-confirmed 2026-08-04 — and is suppressed only for cross-crate `#[non_exhaustive]` structs), then the sibling repo migrates immediately (its own hook runs `clippy -D warnings` too), then the remaining migrations, the closure, the drift test, chores/records. **Execution order: 1, 6, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13** (Task 7 is folded into Task 1). The transync tree is green at every commit; resp-translator is red only in the minutes between Task 1 and Task 6.

**Tech Stack:** Rust workspace (5 crates), path-dep sibling repo `/Volumes/Common/QJoon/resp-translator`, contracts doc scrape test in plain std.

## Global Constraints

- Run tests as `cargo test --workspace -- --test-threads=4` — never raise the thread cap.
- NEVER set/override `CARGO_TARGET_DIR` or pass `--target-dir`.
- Temp files only under `/Volumes/Temp/claude/`.
- `crates/transync-syntax` must gain **no `[features]` table and no `transync-core` dependency (dev-dependencies included)** — either breaks the standing wasm gate.
- Do not hardcode library default values at consumer call sites — unspecified fields flow from the library's `Default` (owner directive 2026-08-04).
- No pure-formatting edits; `cargo fmt` owns style. Run `cargo fmt` before each commit.
- Korean `*.ko.md` files are out of scope.
- Long-lived docs (contracts.md, DCR, open-issues): no line-number references.
- Each task ends with: `cargo fmt` → `cargo clippy --all-targets -- -D warnings` → `cargo test --workspace -- --test-threads=4` → commit. The wasm gate runs in the pre-commit hook automatically.
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: `#[non_exhaustive]` wave + construction-site sweep (one atomic commit)

Spec §2.4 + §3.1–§3.2, merged (was Tasks 1+7). WHY ATOMIC: clippy's `field_reassign_with_default` (style, warn-by-default → error under `-D warnings`) fires on `let x = Default::default(); x.f = …` against an *exhaustive* struct and is suppressed for cross-crate `#[non_exhaustive]` structs (probe-confirmed 2026-08-04 on clippy 1.97). Attribute-first alone breaks every FRU site (E0639-class); sweep-first alone fails the clippy gate. Same commit = both safe.

**Files:**
- Modify: `crates/transync/tests/common/mod.rs` (add helper)
- Modify: every workspace file matched by the grep in Step 1 (scenario tests, `boundary_v02.rs`, `error_fixtures.rs`, `reader_honesty.rs`, `scenarios/inline_protection.rs`, `scenarios/scn_10_long_document.rs`, `scenarios/scn_15_html_blocks.rs`, `crates/transync-cli/src/translate_cmd.rs`, `crates/transync-openai/tests/live_smoke.rs`)
- Modify: `crates/transync-core/src/error.rs`, `crates/transync-core/src/llm.rs`, `crates/transync-syntax/src/error.rs`, `crates/transync-core/src/lib.rs`, `crates/transync-core/src/profile.rs` (attributes)
- Modify: `docs/architecture/contracts.md` (§1 stability bullet)

**Interfaces:**
- Produces: `common::opts_for(target_language: &str) -> transync::TranslateOptions` — used by later scenario edits; Tasks 2 and 3 touch two of the same files and depend on this task being done first. `#[non_exhaustive]` on `TransyncError`, `TranslatorError`, `ParseError`, `TranslateOptions`, `ProfileMetadata`, `ProfileConstraints`, `ProfileBatching`, `ProfileRender` — Task 6 (which runs IMMEDIATELY after this task) migrates the sibling repo onto it.

- [ ] **Step 1: Enumerate the sites**

Run: `grep -rn "\.\.TranslateOptions::default()\|\.\.Default::default()" crates/ --include='*.rs' | grep -v "BlockConstraints"`

Expected: ~27 construction sites (mechanics census). Three additional grep hits that are NOT literals (two fn return types in `boundary_v02.rs`, one doc comment in `scn_11`) stay untouched. Note `BlockConstraints { …, ..BlockConstraints::default() }` sites (e.g. `unit.rs`) are NOT in scope — only `TranslateOptions`.

- [ ] **Step 2: Add the shared helper to `crates/transync/tests/common/mod.rs`**

```rust
use transync::TranslateOptions;

/// Canonical scenario options: everything default except the required
/// target language. Default-then-assign, not FRU, so this keeps
/// compiling when `TranslateOptions` becomes `#[non_exhaustive]`
/// (OI-0027).
pub fn opts_for(target_language: &str) -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.target_language = target_language.to_string();
    opts
}
```

- [ ] **Step 3: Rewrite the scenario-binary sites (files mounted by `tests/scenarios.rs` — they can see `crate::common`)**

Sites whose literal sets ONLY `target_language`:
```rust
// before
let opts = TranslateOptions { target_language: "ko".to_string(), ..Default::default() };
// after
let opts = crate::common::opts_for("ko");
```
Sites that set more fields (e.g. `scn_10`, `inline_protection` set `profile`/`max_concurrent_batches`/…):
```rust
// before
let opts = TranslateOptions { target_language: "ko".into(), max_concurrent_batches: 1, ..Default::default() };
// after
let mut opts = crate::common::opts_for("ko");
opts.max_concurrent_batches = 1;
```
Remove now-unused `TranslateOptions` imports where the helper replaces the last use (rustc will tell you via unused-import warnings under `-D warnings`).

- [ ] **Step 4: Rewrite the standalone-binary sites (no `common` access) as plain default-then-assign**

`boundary_v02.rs` (2), `error_fixtures.rs` (2), `reader_honesty.rs` (2), `crates/transync-openai/tests/live_smoke.rs` (2):
```rust
let mut opts = TranslateOptions::default();
opts.target_language = "ko".to_string();
// …plus whatever other fields the old literal set, one assignment each
```
`crates/transync-cli/src/translate_cmd.rs` (1 site, already `let mut opts = TranslateOptions { … }`): convert the literal body to `TranslateOptions::default()` followed by one assignment per field the literal set.

- [ ] **Step 5: Verify zero remaining FRU sites**

Run the Step-1 grep again. Expected: only the two fn-return-type hits and the doc-comment hit remain.

- [ ] **Step 6: Add `#[non_exhaustive]` to the three error enums**

Directly above each `pub enum` (below the doc comment, adjacent to the derive like `CacheError` does):
- `TransyncError` in `crates/transync-core/src/error.rs`
- `TranslatorError` in `crates/transync-core/src/llm.rs`
- `ParseError` in `crates/transync-syntax/src/error.rs`

Do NOT touch `TransyncError::stable_code()` — it stays an exhaustive in-crate match with no `_` arm (that is the device forcing a stable code per new variant). Add one doc sentence to each enum: `/// Non-exhaustive: new variants may be added in minor releases; match with a wildcard arm.` For `TransyncError` add additionally: `/// Every variant carries a stable machine code — see [`TransyncError::stable_code`] and contracts.md §1.`

- [ ] **Step 7: Add `#[non_exhaustive]` to the five structs**

`TranslateOptions` (`crates/transync-core/src/lib.rs`) — plus a doc addendum on the struct:
```rust
/// Non-exhaustive: construct via default-then-assign —
/// `let mut opts = TranslateOptions::default(); opts.target_language = "ko".into();`
/// Struct-literal and functional-update construction are not available
/// outside this crate.
```
`ProfileMetadata`, `ProfileConstraints`, `ProfileBatching`, `ProfileRender` (`crates/transync-core/src/profile.rs`) — same one-line doc note each ("construct via `Default` then assign, or deserialize").

- [ ] **Step 8: Flip contracts.md §1's stability bullet**

Rewrite the bullet that currently reads "`TranslatorError` … is **not** `#[non_exhaustive]` today … becomes true only once `#[non_exhaustive]` lands … (tracked in OI-0027)" to state the landed reality: `TranslatorError`, `TransyncError`, and `ParseError` are `#[non_exhaustive]`; variant additions are non-breaking; consumers must keep a wildcard arm; variant *removals* remain breaking. Note the same attribute on `TranslateOptions`/`ProfileMetadata`/`ProfileConstraints`/`ProfileBatching`/`ProfileRender` with the default-then-assign construction pattern. No line-number references.

- [ ] **Step 9: Gates + commit**

Run: `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --workspace -- --test-threads=4`
Expected: all green (335 passed baseline, no removals). NOTE: the clippy gate passing is itself the probe repeated in vivo — default-then-assign sites are lint-suppressed because the structs are now cross-crate non_exhaustive.
```bash
git add -A crates/ docs/architecture/contracts.md
git commit -m "feat!: #[non_exhaustive] error enums + options/profile structs; construction via default-then-assign (OI-0027)"
```
The sibling repo `/Volumes/Common/QJoon/resp-translator` is now temporarily broken (its FRU + full literal fail against the attributes) — execute Task 6 IMMEDIATELY after this commit.

---

### Task 2: scn_10 migrates run_pipeline → translate_with_cache

Spec §2.1. Signature-identical swap; the only behavioral delta (empty-`target_language` guard) is unreachable here — every site sets `"ko"`.

**Files:**
- Modify: `crates/transync/tests/scenarios/scn_10_long_document.rs`

**Interfaces:**
- Consumes: `transync::translate_with_cache(source: &str, opts: &TranslateOptions, translator: &T, cache: &dyn Cache) -> Result<TranslationOutput, TransyncError>` (already public).
- Produces: zero `run_pipeline` references outside `transync-core` — Task 9's `pub(crate) mod pipeline` depends on it.

- [ ] **Step 1: Swap the import**

Delete `use transync::pipeline::run_pipeline;`. Ensure `translate_with_cache` is in the existing `use transync::{…}` list.

- [ ] **Step 2: Rename all seven call sites**

`run_pipeline(` → `translate_with_cache(` — arguments unchanged, verbatim.

- [ ] **Step 3: Verify**

Run: `grep -rn "run_pipeline" crates/transync/tests/` — expected: no hits.
Run: `cargo test -p transync --test scenarios -- --test-threads=4 scn_10`
Expected: PASS (same assertions, same pipeline underneath).

- [ ] **Step 4: Gates + commit**

```bash
git add crates/transync/tests/scenarios/scn_10_long_document.rs
git commit -m "test: scn_10 exercises translate_with_cache through the facade (OI-0027)"
```

---

### Task 3: live_smoke reaches engine internals via dev-dependencies

Spec §2.3. `transync-openai`'s live smoke test uses `parser::parse` and `unit::{build_batches, html_outcomes}`; after the closure those facade paths die. Dev-deps declare the internal reach honestly and survive Task 9.

**Files:**
- Modify: `crates/transync-openai/Cargo.toml`
- Modify: `crates/transync-openai/tests/live_smoke.rs`

**Interfaces:**
- Consumes: `transync_syntax::parser::parse(&str) -> Result<Document, ParseError>`; `transync_core::unit::{build_batches, html_outcomes}` (same items, new paths — the `Document` type is identical on both paths because core re-exports syntax's).
- Produces: zero `transync::parser` / `transync::unit` references anywhere outside core — Task 9 depends on it.

- [ ] **Step 1: Add dev-dependencies**

In `crates/transync-openai/Cargo.toml` under `[dev-dependencies]` (mirror the style of the existing `transync` dependency entry — path + same version key shape):
```toml
transync-core = { path = "../transync-core" }
transync-syntax = { path = "../transync-syntax" }
```
Do NOT touch `[dependencies]` — these are test-only.

- [ ] **Step 2: Migrate the paths in `live_smoke.rs`**

- `transync::parser::parse(` → `transync_syntax::parser::parse(` (both sites)
- `transync::unit::build_batches(` → `transync_core::unit::build_batches(`
- `transync::unit::html_outcomes(` → `transync_core::unit::html_outcomes(`

- [ ] **Step 3: Verify compile (live tests are ignored by default — compile is the gate)**

Run: `cargo test -p transync-openai --test live_smoke --no-run`
Expected: compiles. Then `grep -rn "transync::parser\|transync::unit" crates/ --include='*.rs' | grep -v transync-core/src` — expected: no hits.

- [ ] **Step 4: Gates + commit**

```bash
git add crates/transync-openai/
git commit -m "test: live_smoke reaches parser/unit via engine dev-deps (OI-0027)"
```

---

### Task 4: Root re-export additions + path migrations

Spec §1.3. Additive root aliases first, then migrate the module-path users so Task 9 can demote the module re-exports.

**Files:**
- Modify: `crates/transync-core/src/lib.rs` (root re-exports)
- Modify: `crates/transync-openai/src/client.rs`, `crates/transync/tests/common/mock_translator.rs`, `crates/transync/tests/scenarios/scn_10_long_document.rs`, `crates/transync-cli/tests/sync_js_drift.rs`

**Interfaces:**
- Produces: root paths `transync::{ParseError, GlossaryScope, ALIGNMENT_SCHEMA_VERSION}` — Task 9's facade list and Task 11's drift test name them.

- [ ] **Step 1: Extend core's root re-exports in `crates/transync-core/src/lib.rs`**

```rust
pub use error::{ParseError, TransyncError};              // was: pub use error::TransyncError;
pub use transync_syntax::align::ALIGNMENT_SCHEMA_VERSION; // new line
```
and add `GlossaryScope` to the existing `pub use llm::{…}` list (alphabetical position, after `GlossaryExtractionRequest`).

- [ ] **Step 2: Migrate the module-path users to root aliases**

- `crates/transync-openai/src/client.rs`: `use transync::id::BlockId;` → `use transync::BlockId;`; the inline `transync::id::BlockKind::Paragraph` → `transync::BlockKind::Paragraph`.
- `crates/transync/tests/common/mock_translator.rs`: `use transync::id::BlockId;` → `use transync::BlockId;`.
- `crates/transync/tests/scenarios/scn_10_long_document.rs`: same `transync::id::` → root swap.
- `crates/transync-cli/tests/sync_js_drift.rs`: `transync::align::ALIGNMENT_SCHEMA_VERSION` → `transync::ALIGNMENT_SCHEMA_VERSION` (the code use AND the doc-comment mention a few lines above it).

- [ ] **Step 3: Verify**

Run: `grep -rn "transync::id::\|transync::align::" crates/ --include='*.rs' | grep -v "transync-core/src\|transync-syntax/src"` — expected: no hits.
Run: `cargo test --workspace -- --test-threads=4` — green.

- [ ] **Step 4: Commit**

```bash
git add crates/
git commit -m "feat: root aliases for ParseError/GlossaryScope/ALIGNMENT_SCHEMA_VERSION; migrate module-path users (OI-0027)"
```

---

### Task 5: OutputBudgetWarning moves batch → validate

Spec §1.2. The type is tier-(a) (element of `ValidationReport.output_budget_warnings`); its module is about to go private. No consumer names the type today, so the move is invisible downstream.

**Files:**
- Modify: `crates/transync-core/src/batch.rs` (remove struct + Display; import from validate)
- Modify: `crates/transync-core/src/validate.rs` (add struct + Display; drop its `crate::batch::OutputBudgetWarning` import)
- Modify: `crates/transync-core/src/lib.rs` (root re-export)

**Interfaces:**
- Produces: `transync_core::validate::OutputBudgetWarning` + root alias `transync_core::OutputBudgetWarning` `{ unit_id: BlockId, estimated_output_tokens: u32, ceiling_tokens: u32 }`, `derive(Debug, Clone, serde::Serialize)` + `Display`. Task 9's facade list and Task 11's drift test name the root alias.

- [ ] **Step 1: Move the type**

Cut from `batch.rs` and paste into `validate.rs` (near the `ValidationReport` definition), byte-identical including doc comments and the `Display` impl:
```rust
/// A unit whose estimated response size exceeds the per-batch output ceiling
/// (D1 §2.3). Estimation is heuristic and the provider may still succeed, so
/// this is a WARNING, never an abort — the only remaining over-ceiling shape
/// after output-aware packing is a single unit whose own `est_out` exceeds
/// the target (it gets its own batch and may terminally abort the run at the
/// provider).
///
/// TRACE: SCN-10
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputBudgetWarning {
    pub unit_id: BlockId,
    pub estimated_output_tokens: u32,
    /// The raw `target_output_tokens`, not the reserved effective target.
    pub ceiling_tokens: u32,
}

impl std::fmt::Display for OutputBudgetWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "unit {} estimated output ~{} tokens exceeds the per-batch output ceiling of {} \
             (profile [batching] target_output_tokens); the provider may truncate the response \
             and abort the run",
            self.unit_id, self.estimated_output_tokens, self.ceiling_tokens
        )
    }
}
```
(`validate.rs` already has `BlockId` in scope; if not, add the import.)

- [ ] **Step 2: Fix every referencing site**

Run: `grep -rn "OutputBudgetWarning" crates/ --include='*.rs'` and fix each:
- `batch.rs`: add `use crate::validate::OutputBudgetWarning;`; `output_budget_warnings()` body unchanged.
- `validate.rs`: delete its old `use crate::batch::OutputBudgetWarning;` (or equivalent path in the report field/type references).
- Any `pipeline.rs` references follow the same import swap.
- `crates/transync-core/src/lib.rs`: add `OutputBudgetWarning` to the `pub use validate::{…}` list (alphabetical, after `BatchFault`).

- [ ] **Step 3: Gates + commit**

Run: `cargo test --workspace -- --test-threads=4` — green (Display/serde output byte-identical, so no snapshot churn).
```bash
git add crates/transync-core/src/
git commit -m "refactor: OutputBudgetWarning lives with ValidationReport in validate (OI-0027)"
```

---

### Task 6: resp-translator migration (sibling repo — commit there, not here)

Spec §2.2. **Runs IMMEDIATELY after Task 1** (execution order 1, 6, 2, 3, …): Task 1's attributes break this repo's FRU + full-literal sites, and this repo's own pre-commit hook runs `cargo clippy --workspace --all-targets -- -D warnings`, so the default-then-assign rewrites below only pass its hook once the structs are non_exhaustive (lint suppression, see Task 1's rationale). Work in `/Volumes/Common/QJoon/resp-translator`.

**Files:**
- Modify: `/Volumes/Common/QJoon/resp-translator/bins/copy-transfer-mcp/src/translation.rs`
- Modify: `/Volumes/Common/QJoon/resp-translator/bins/copy-transfer-mcp/src/translation/profile_bridge.rs`

**Interfaces:**
- Consumes: `transync::translate_with_cache` (public since 0.2 dev), `ProfileMetadata::default()` (= `default_profile()`).
- Produces: a resp-translator commit; zero `transync::pipeline::` references and zero transync-struct literals in that repo.

- [ ] **Step 1: Swap the pipeline entry in `translation.rs`**

```rust
// before
transync::pipeline::run_pipeline(&envelope.message_text, &opts, translator_ref, cache_ref)
// after
transync::translate_with_cache(&envelope.message_text, &opts, translator_ref, cache_ref)
```
Also update the module-header TRACE comment in `profile_bridge.rs` that names `transync::pipeline::run_pipeline` as the consumer — it now names `transync::translate_with_cache`.

- [ ] **Step 2: Rewrite the `ProfileMetadata` literal in `profile_bridge.rs` (`to_transync_profile`)**

Replace the whole 9-field literal (including the `auto_glossary: None` / `render: ProfileRender::default()` lines and their comment) with:
```rust
    // Default-then-assign: fields MCP does not own (auto_glossary, render,
    // and any future additions) flow from transync's default profile
    // instead of being restated here.
    let mut meta = ProfileMetadata::default();
    meta.slug = p.name.clone();
    meta.version = format!("v{}", p.schema_version);
    meta.prompt_body = p.system.prompt.clone();
    meta.glossary = p.glossary.clone();
    meta.constraints = constraints;
    meta.batching = batching;
    meta.load_warnings = load_warnings;
    meta
```
Remove the now-unused `ProfileRender` import if nothing else uses it. Behavior-preservation check (spec §2.2): the embedded `crates/transync-core/profiles/default.toml` has `auto_glossary` and `[render]` commented out, so `default()` yields `None` / `ProfileRender::default()` — identical to the deleted hardcodes. Verify by reading that TOML.

- [ ] **Step 3: Rewrite the `TranslateOptions` FRU in `to_transync_options`**

```rust
    let mut opts = TranslateOptions::default();
    opts.source_language = "auto".into();
    opts.target_language = p.target_language.clone();
    opts.model_id = pipeline_cfg.model.clone();
    opts
```

- [ ] **Step 4: Verify in that repo**

Run (from `/Volumes/Common/QJoon/resp-translator`): `cargo check --workspace` then `cargo test --workspace -- --test-threads=4`.
Expected: green. The `profile_metadata_carries_template_and_glossary` and `options_carry_target_language_and_model` tests pin exactly the fields we assign.

- [ ] **Step 5: Commit (in resp-translator)**

```bash
git add bins/copy-transfer-mcp/src/translation.rs bins/copy-transfer-mcp/src/translation/profile_bridge.rs
git commit -m "migrate: transync curated surface — translate_with_cache + default-then-assign construction

transync OI-0027 hides pipeline::run_pipeline and marks TranslateOptions/
ProfileMetadata #[non_exhaustive]. Behavioral delta: an empty profile
target_language now fails fast with stable_code \"internal\" instead of
proceeding (profiles always set it today).

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: FOLDED INTO TASK 1 — no work here

The 2026-08-04 pre-flight probe showed clippy's `field_reassign_with_default` makes the attributes and the construction-site sweep inseparable (see Task 1's rationale). All of this task's former content — the three enum attributes, the five struct attributes, and the contracts.md §1 stability flip — lives in Task 1 Steps 6–9. Do not dispatch an implementer for Task 7; there is nothing to do.

---

### Task 8: TranslationUnit constructor + `#[non_exhaustive]`

Spec §3.3–§3.4. TDD: test first.

**Files:**
- Modify: `crates/transync-core/src/llm.rs` (attribute + impl + unit test)
- Modify: `crates/transync-openai/src/client.rs` (fixture migration)
- Modify: `docs/architecture/contracts.md` (§1 TranslationUnit paragraph)

**Interfaces:**
- Produces:
  - `TranslationUnit::new(unit_id: BlockId, block_kind: BlockKind, input_mode: InputMode, source_payload: String, source_hash: u64) -> Self`
  - `fn with_context(self, BlockContext) -> Self`, `fn with_constraints(self, BlockConstraints) -> Self`, `fn with_batch_id(self, BatchId) -> Self`, `fn with_retry(self, RetryContext) -> Self`
  - Task 11's drift test names `TranslationUnit` (root) — unchanged path.

- [ ] **Step 1: Write the failing test (in `llm.rs`'s `#[cfg(test)] mod tests`, create the module if absent)**

```rust
#[test]
fn translation_unit_constructor_defaults_match_first_dispatch_shape() {
    let u = TranslationUnit::new(
        BlockId("p-0001".to_string()),
        BlockKind::Paragraph,
        InputMode::TextFragment,
        "hello".to_string(),
        42,
    );
    assert_eq!(u.unit_id.0, "p-0001");
    assert_eq!(u.source_hash, 42);
    assert_eq!(u.batch_id, BatchId::new(0)); // batcher placeholder semantics
    assert!(u.retry.is_none());              // first-dispatch contract
    let with = u
        .with_batch_id(BatchId::new(7))
        .with_retry(RetryContext { attempt: 1, rejected_by: None, reason: None });
    assert_eq!(with.batch_id, BatchId::new(7));
    assert_eq!(with.retry.as_ref().map(|r| r.attempt), Some(1));
}
```
(If `RetryContext`'s fields differ from `{attempt, rejected_by, reason}`, mirror the real definition — it is in the same file.)

- [ ] **Step 2: Run it — expect FAIL** (`no function or associated item named 'new'`)

Run: `cargo test -p transync-core translation_unit_constructor -- --test-threads=4`

- [ ] **Step 3: Implement**

Add `#[non_exhaustive]` above `pub struct TranslationUnit` (below its doc comment) plus a doc addendum: `/// Non-exhaustive: construct via [\`TranslationUnit::new\`] + \`with_*\`; struct literals are in-crate only.` Then:

```rust
impl TranslationUnit {
    /// The five required-by-essence fields are positional; the rest
    /// default (`context`/`constraints` to their `Default`s, `batch_id`
    /// to the `BatchId::new(0)` placeholder the batcher overwrites per
    /// chunk, `retry` to `None` per the first-dispatch contract).
    /// `source_hash` is required deliberately: it is a `CacheKey` axis
    /// with no meaningful zero — a defaulted hash would silently alias
    /// cache entries.
    pub fn new(
        unit_id: BlockId,
        block_kind: BlockKind,
        input_mode: InputMode,
        source_payload: String,
        source_hash: u64,
    ) -> Self {
        Self {
            unit_id,
            block_kind,
            input_mode,
            source_payload,
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash,
            batch_id: BatchId::new(0),
            retry: None,
        }
    }

    pub fn with_context(mut self, context: BlockContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_constraints(mut self, constraints: BlockConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_batch_id(mut self, batch_id: BatchId) -> Self {
        self.batch_id = batch_id;
        self
    }

    pub fn with_retry(mut self, retry: RetryContext) -> Self {
        self.retry = Some(retry);
        self
    }
}
```
In-crate literal constructions (unit.rs, retry.rs, validate tests, prompt tests…) stay as-is — `#[non_exhaustive]` does not restrict the defining crate.

- [ ] **Step 4: Run the test — expect PASS**, then migrate the one external literal

`crates/transync-openai/src/client.rs` `fixture_batch()` — the 9-field `TranslationUnit { … }` literal becomes:
```rust
TranslationUnit::new(
    BlockId("p-0001".to_string()),
    transync::BlockKind::Paragraph,
    transync::InputMode::TextFragment,
    "Hello world.".to_string(),
    11,
)
```
(carry over the fixture's actual payload/hash values verbatim; append `.with_context(…)`/`.with_constraints(…)` only if the old literal set non-default values — census says it used `::default()` for both, so no `with_*` calls are needed; the old literal's `batch_id`/`retry` values were the placeholder/None, also defaults).

- [ ] **Step 5: Update contracts.md §1**

Replace the "`TranslationUnit` stays exhaustive by policy (external mock-translator tests construct it)…" sentence with: `TranslationUnit` is `#[non_exhaustive]`; external construction goes through `TranslationUnit::new(unit_id, block_kind, input_mode, source_payload, source_hash)` + `with_context`/`with_constraints`/`with_batch_id`/`with_retry`, so field additions are non-breaking. In the same bullet, state the exhaustive-by-policy set (spec §3.4): `UnitResult` and `TranslationBatchResult` stay exhaustive because downstream translators *produce* them (field additions are breaking-by-policy); `TranslationBatch` is produced by core only, by policy; `GlossaryEntry`'s serde shape is third-party wire format (new fields must be `#[serde(default)]`).

- [ ] **Step 6: Gates + commit**

Run: `cargo clippy --all-targets -- -D warnings && cargo test --workspace -- --test-threads=4`
```bash
git add crates/ docs/architecture/contracts.md
git commit -m "feat: TranslationUnit::new + with_* constructor; struct goes non_exhaustive (OI-0027)"
```

---

### Task 9: The closure — core goes pub(crate), facade goes explicit

Spec §1.1, §1.3, §1.5, §5 (core half). The big one. All consumers were migrated in Tasks 1–6; this task makes the boundary real.

**Files:**
- Modify: `crates/transync-core/src/lib.rs` (module block + crate doc)
- Modify: `crates/transync-core/src/llm.rs`, `crates/transync-core/src/profile.rs` (doc de-links)
- Modify: `crates/transync/src/lib.rs` (full rewrite)

**Interfaces:**
- Consumes: everything Tasks 2–5 produced (no external `run_pipeline`/`transync::parser`/`transync::unit`/`transync::id::`/`transync::align::` users; `OutputBudgetWarning` root alias).
- Produces: the curated facade — the exact list Task 11's drift test pins. `transync::{pipeline,parser,unit,batch,validate,regen,render,error,id,align,transync_syntax}` paths no longer exist (`unit` survives only as a hidden path via `transync_core`).

- [ ] **Step 1: Rewrite core's module block (`crates/transync-core/src/lib.rs`)**

```rust
pub(crate) use transync_syntax::align;
pub(crate) mod batch;
pub mod cache;
pub(crate) mod error;
pub(crate) use transync_syntax::id;
pub mod llm;
pub(crate) use transync_syntax::parser;
pub(crate) mod pipeline;
pub mod profile;
pub(crate) use transync_syntax::regen;
pub(crate) use transync_syntax::render;
// Internal unit-construction engine. `pub` only because transync-openai's
// live-smoke test reaches it cross-crate via a dev-dependency (OI-0027
// spec §2.3); not part of the curated API.
#[doc(hidden)]
pub mod unit;
pub(crate) mod validate;
```
The root `pub use` item lists below it are UNCHANGED (they keep working over `pub(crate)` modules because the items themselves are `pub` — the standard facade pattern). If rustc reports any of the `pub(crate) use transync_syntax::…` aliases as unused (`-D warnings`), delete that alias and fix the internal references to use the other path — do not silence with `#[allow]`.

- [ ] **Step 2: Fix core's crate-level and item docs that link into now-private modules**

- Crate doc: `[`validate::BatchFault`]` → `[`BatchFault`]`; the "All wire types under [`llm`], [`align`], [`profile`], [`cache`]" line → "All wire types under [`llm`], [`profile`], [`cache`] and the alignment-map types re-exported at the crate root."
- `translate` doc: "Wraps [`pipeline::run_pipeline`] with a fresh in-memory cache." → "Runs the internal pipeline with a fresh in-memory cache."
- `TranslateOptions::model_id` doc: `[`batch::encoder_for`]` → `` `encoder_for` `` (plain code text, twice if linked twice).
- `llm.rs` `Translator::tokenizer_hint` doc: `[`crate::batch::encoder_for`]` → plain `` `encoder_for` ``.
- `profile.rs` `ProfileBatching` doc: `[`crate::batch::DEFAULT_OUTPUT_EXPANSION_FACTOR`]` → plain `` `DEFAULT_OUTPUT_EXPANSION_FACTOR` ``.
- `llm.rs` `Translator::extract_glossary` doc's `[`crate::profile::merge_auto_glossary`]` STAYS a live link (`profile` remains public).

- [ ] **Step 3: Rewrite the facade (`crates/transync/src/lib.rs`) in full**

```rust
//! `transync` — the curated public facade (semver firewall) over the
//! engine crates.
//!
//! Everything a consumer may name is re-exported here explicitly —
//! the supported surface is this file, mirrored by
//! `docs/architecture/contracts.md` §0 and pinned by
//! `tests/public_surface.rs`. Engine internals (`transync-core`'s
//! pipeline/batching/validation modules, `transync-syntax`'s
//! parser/renderer) are not reachable through this crate; advanced
//! consumers depend on the engine crates directly.
//!
//! TRACE: ADR-0002
//! TRACE: ADR-0003
//! TRACE: OI-0027

// Curated modules: the Translator/provider contract (`llm`, incl. the
// provider-SDK `llm::prompt`), profile handling, and the cache trait.
pub use transync_core::{cache, llm, profile};

#[cfg(feature = "test-stub")]
pub use transync_core::test_stub;

// Entry points and their option/result types.
pub use transync_core::{
    FullReparseFailure, TranslateOptions, TranslationOutput, translate, translate_with_cache,
};

// Errors.
pub use transync_core::{ParseError, TransyncError};

// Cache cluster.
pub use transync_core::{Cache, CacheError, CacheKey, InMemoryCache};

// Translator-contract cluster.
pub use transync_core::{
    BatchId, BlockConstraints, BlockContext, DEFAULT_MAX_AUTO_GLOSSARY_TERMS, GlossaryEntry,
    GlossaryExtractionRequest, GlossaryScope, InputMode, ListTopologyEntry,
    MAX_EXTRACTION_SOURCE_BYTES, OutputKind, ProviderFingerprint, RetryContext, TableAlign,
    TokenizerHint, TranslationBatch, TranslationBatchResult, TranslationUnit, Translator,
    TranslatorError, UnitResult,
};

// Profile types.
pub use transync_core::{MergedGlossary, ProfileError, ProfileMetadata, merge_auto_glossary};

// Alignment-map wire types.
pub use transync_core::{
    ALIGNMENT_SCHEMA_VERSION, AlignmentBlock, AlignmentMap, ByteRange, FallbackStatus,
    GeneratorMeta, SyncRole, ValidationSummary,
};

// Block identity.
pub use transync_core::{BlockId, BlockKind};

// Validation-report family.
pub use transync_core::{
    AttemptOutcome, AutoGlossaryReport, AutoGlossaryStatus, BatchFault, OutputBudgetWarning,
    UnitValidationRecord, VALIDATION_SCHEMA_VERSION, ValidationLayer, ValidationReport,
};
```
Both old lines (`pub use transync_core::*;` and `pub use transync_syntax;`) are gone.

- [ ] **Step 4: Gates**

Run: `cargo clippy --all-targets -- -D warnings && cargo test --workspace -- --test-threads=4`
Expected: green. Compile failures here mean a consumer slipped through the census — fix by migrating THAT consumer (root alias or engine dev-dep), never by re-widening the facade.
Then: `grep -rn "pub use transync_core::\*\|pub use transync_syntax;" crates/transync/src/` — no hits.

- [ ] **Step 5: Commit**

```bash
git add crates/
git commit -m "feat!: compiler-enforced facade curation — core internals go pub(crate), facade goes explicit (OI-0027)"
```

---

### Task 10: Syntax narrowings + rustdoc zero-warning gate

Spec §1.4, §5 (syntax half). The three DCR-0017 narrowing candidates plus all seven doc de-links.

**Files:**
- Modify: `crates/transync-syntax/src/htmlseg.rs`, `crates/transync-syntax/src/outcome.rs`, `crates/transync-syntax/src/walk.rs`, `crates/transync-syntax/src/render.rs`

**Interfaces:**
- Produces: `htmlseg::balance_fragment`, `outcome::is_translatable`, `walk::label_for` are `pub(crate)`; `cargo doc --no-deps` is warning-free for all three library crates (a Task 13 exit criterion).

- [ ] **Step 1: Narrow the three items**

- `htmlseg.rs`: `pub fn balance_fragment` → `pub(crate) fn balance_fragment` (sole consumer is `render`, same crate).
- `outcome.rs`: `#[doc(hidden)] pub fn is_translatable` → `pub(crate) fn is_translatable` (drop the now-redundant `#[doc(hidden)]`; sole consumer is this file).
- `walk.rs`: `pub fn label_for` → `pub(crate) fn label_for` (no cross-crate consumer).

- [ ] **Step 2: De-link the seven rustdoc warnings (refdefs precedent: `[`x`]` → `` `x` ``, prose otherwise untouched)**

1. `htmlseg.rs` module doc: `[`rewriter_settings`]` → `` `rewriter_settings` ``
2. `htmlseg.rs` `extract` doc: `[`scan`]` → `` `scan` ``
3–5. `htmlseg.rs` `splice` doc paragraph: `[`scan`]`, `[`rewriter_settings`]`, `[`wanted_text_type`]` → plain code text (one edit, three links)
6. `render.rs` module doc: `[`node_inner_html`]` → `` `node_inner_html` ``
7. `walk.rs` `node_label` doc ("the AST-side twin of [`label_for`]"): `[`label_for`]` → `` `label_for` `` (created by Step 1's narrowing)

- [ ] **Step 3: Verify the doc gate**

Run: `cargo doc --no-deps -p transync-syntax -p transync-core -p transync 2>&1 | grep -i "warning" ; echo "exit: $?"`
Expected: no warning lines (grep exit 1). If new warnings surfaced from Task 9's hides, de-link them the same way.
Run: `cargo clippy --all-targets -- -D warnings && cargo test --workspace -- --test-threads=4` — green. The wasm gate runs in the hook at commit.

- [ ] **Step 4: Commit**

```bash
git add crates/transync-syntax/
git commit -m "refactor: narrow balance_fragment/is_translatable/label_for; zero rustdoc warnings (OI-0027)"
```

---

### Task 11: contracts.md §0 + the surface-drift test

Spec §4. The pin that keeps Tasks 1–10 true.

**Files:**
- Modify: `docs/architecture/contracts.md` (new §0 before §1; extend §1's prompt-module list)
- Create: `crates/transync/tests/public_surface.rs`
- Modify: `crates/transync-core/src/llm/prompt.rs` (doc notes)
- Modify: `crates/transync/Cargo.toml` (drop the now-unused `transync-syntax` dependency — Task 9 removed its only reference)

**Interfaces:**
- Consumes: the Task 9 facade list, Task 4/5 root aliases, Task 1/8 attributes.
- Produces: the standing drift gate; contracts.md §0 as the documented surface.

- [ ] **Step 1: Add prompt-text doc notes (spec §4.2 last bullet)**

In `llm/prompt.rs`, on `EXTRACTION_SYSTEM_PROMPT` and in the module doc: `/// The prompt/instruction *text* is not a contract — only the schema object shape and the parse behavior are (contracts.md §1). Wording may change in any release.`

- [ ] **Step 2: Write contracts.md `## 0. Public Rust surface` (insert before `## 1.`)**

Prose preamble: this table IS the supported surface; it mirrors `crates/transync/src/lib.rs` and is enforced bidirectionally by `crates/transync/tests/public_surface.rs`; tier (c) internals (`pipeline`, `parser`, `unit`, `batch`, `validate`, `regen`, `render` and the whole of `transync-syntax`) are deliberately absent — advanced consumers depend on the engine crates directly. `transync::test_stub` (feature `test-stub`) is test-only surface, listed for completeness but excluded from the drift table below because it is feature-gated.

Then the table, one row per path (kind ∈ fn/struct/enum/trait/const/module; tier ∈ a/b). The `path` column values are EXACTLY these 78 strings (the test scrapes them):

```
| path | kind | tier |
|---|---|---|
| `transync::translate` | fn | a |
| `transync::translate_with_cache` | fn | a |
| `transync::TranslateOptions` | struct | a |
| `transync::TranslationOutput` | struct | a |
| `transync::FullReparseFailure` | enum | a |
| `transync::TransyncError` | enum | a |
| `transync::ParseError` | enum | a |
| `transync::Cache` | trait | a |
| `transync::CacheError` | enum | a |
| `transync::CacheKey` | struct | a |
| `transync::InMemoryCache` | struct | a |
| `transync::cache` | module | a |
| `transync::Translator` | trait | a |
| `transync::TranslatorError` | enum | a |
| `transync::TranslationBatch` | struct | a |
| `transync::TranslationBatchResult` | struct | a |
| `transync::TranslationUnit` | struct | a |
| `transync::UnitResult` | struct | a |
| `transync::OutputKind` | enum | a |
| `transync::RetryContext` | struct | a |
| `transync::BatchId` | struct | a |
| `transync::BlockConstraints` | struct | a |
| `transync::BlockContext` | struct | a |
| `transync::InputMode` | enum | a |
| `transync::ListTopologyEntry` | struct | a |
| `transync::TableAlign` | enum | a |
| `transync::TokenizerHint` | enum | a |
| `transync::ProviderFingerprint` | struct | a |
| `transync::GlossaryEntry` | struct | a |
| `transync::GlossaryScope` | enum | a |
| `transync::GlossaryExtractionRequest` | struct | a |
| `transync::DEFAULT_MAX_AUTO_GLOSSARY_TERMS` | const | a |
| `transync::MAX_EXTRACTION_SOURCE_BYTES` | const | a |
| `transync::llm` | module | a |
| `transync::llm::HeadingSnippet` | struct | a |
| `transync::llm::NeighborSnippet` | struct | a |
| `transync::llm::HtmlSegmentConstraints` | struct | a |
| `transync::llm::ListDelimiter` | enum | a |
| `transync::ProfileMetadata` | struct | a |
| `transync::ProfileError` | enum | a |
| `transync::MergedGlossary` | struct | a |
| `transync::merge_auto_glossary` | fn | a |
| `transync::profile` | module | a |
| `transync::profile::ProfileConstraints` | struct | a |
| `transync::profile::ProfileBatching` | struct | a |
| `transync::profile::ProfileRender` | struct | a |
| `transync::profile::load_profile` | fn | a |
| `transync::profile::default_profile` | fn | a |
| `transync::AlignmentMap` | struct | a |
| `transync::AlignmentBlock` | struct | a |
| `transync::ALIGNMENT_SCHEMA_VERSION` | const | a |
| `transync::ByteRange` | struct | a |
| `transync::FallbackStatus` | enum | a |
| `transync::GeneratorMeta` | struct | a |
| `transync::SyncRole` | enum | a |
| `transync::ValidationSummary` | struct | a |
| `transync::BlockId` | struct | a |
| `transync::BlockKind` | enum | a |
| `transync::ValidationReport` | struct | a |
| `transync::ValidationLayer` | enum | a |
| `transync::AttemptOutcome` | struct | a |
| `transync::UnitValidationRecord` | struct | a |
| `transync::BatchFault` | struct | a |
| `transync::OutputBudgetWarning` | struct | a |
| `transync::AutoGlossaryReport` | struct | a |
| `transync::AutoGlossaryStatus` | enum | a |
| `transync::VALIDATION_SCHEMA_VERSION` | const | a |
| `transync::llm::prompt` | module | b |
| `transync::llm::prompt::SCHEMA_NAME` | const | b |
| `transync::llm::prompt::build_user_prompt` | fn | b |
| `transync::llm::prompt::schema_object_for` | fn | b |
| `transync::llm::prompt::parse_batch_output` | fn | b |
| `transync::llm::prompt::EXTRACTION_SCHEMA_NAME` | const | b |
| `transync::llm::prompt::EXTRACTION_SYSTEM_PROMPT` | const | b |
| `transync::llm::prompt::build_extraction_user_prompt` | fn | b |
| `transync::llm::prompt::extraction_schema_object` | fn | b |
| `transync::llm::prompt::parse_extraction_output` | fn | b |
| `transync::profile::render_prompt_body` | fn | b |
```
(78 rows — count them when done; the tier-b subsection prose repeats the "prompt text is not a contract" note.) Also add the `stable_code()` vocabulary to §1 (spec §4.2): a small list of the seven codes (`parse_failed`, `provider_error`, `validation_failed`, `regen_failed`, `profile_failed`, `alignment_failed`, `internal`) marked as an inter-process contract — additions require a new stable code; strings never change meaning. Also extend §1's prompt-module sentence to name all nine `llm::prompt` items (it currently names four).

- [ ] **Step 3: Write `crates/transync/tests/public_surface.rs`**

Part 1 — compile-as-assertion (module `first_class`): one `pub use` per §0 row, grouped by path prefix. Root rows via `pub use transync::{…};`, `transync::llm::…` rows via `pub use transync::llm::{…};`, prompt rows via `pub use transync::llm::prompt::{…};`, profile rows via `pub use transync::profile::{…};`. Module rows (`transync::cache`, `transync::llm`, `transync::profile`, `transync::llm::prompt`) via `pub use transync::cache;` etc. Wrap the module in `#[allow(unused_imports)]`.

Part 2+3 — the doc scrape:
```rust
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Every §0 row path, verbatim. Mirrors both the `first_class` module
/// above and contracts.md §0 — the test below fails on any drift.
const DOCUMENTED: &[&str] = &[
    "transync::translate",
    "transync::translate_with_cache",
    // … all 80 rows from §0, byte-identical, same order as the table …
];

fn repo_root() -> PathBuf {
    // Same idiom as docs_index_drift.rs.
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn section_zero() -> String {
    let text = fs::read_to_string(repo_root().join("docs/architecture/contracts.md"))
        .expect("contracts.md must exist");
    let start = text.find("## 0.").expect("contracts.md must carry §0");
    let rest = &text[start..];
    let end = rest[3..].find("\n## ").map(|i| i + 3).unwrap_or(rest.len());
    rest[..end].to_string()
}

fn table_paths() -> BTreeSet<String> {
    section_zero()
        .lines()
        .filter(|l| l.starts_with("| `transync::"))
        .map(|l| {
            let cell = l.trim_start_matches("| `");
            cell[..cell.find('`').expect("row path must close its backtick")].to_string()
        })
        .collect()
}

#[test]
fn surface_table_matches_the_documented_list() {
    let table = table_paths();
    let documented: BTreeSet<String> = DOCUMENTED.iter().map(|s| s.to_string()).collect();
    let undocumented: Vec<_> = documented.difference(&table).collect();
    let orphaned: Vec<_> = table.difference(&documented).collect();
    assert!(
        undocumented.is_empty() && orphaned.is_empty(),
        "surface drift — missing from contracts.md §0: {undocumented:?}; \
         documented but not exported: {orphaned:?}"
    );
}
```

Part 4 — negative assertion (path-segment comparison, NOT substring — `render_prompt_body` must pass):
```rust
#[test]
fn hidden_modules_are_not_documented_as_surface() {
    let forbidden = [
        "pipeline", "parser", "unit", "batch", "validate", "regen", "render",
        "transync_syntax", "htmlseg", "outcome", "walk",
    ];
    for path in table_paths() {
        for seg in path.split("::") {
            assert!(
                !forbidden.contains(&seg),
                "contracts.md §0 documents hidden module path: {path}"
            );
        }
    }
}
```

Part 5 — the GeneratorMeta guard (DCR-0017 M6):
```rust
#[test]
fn generator_version_matches_the_facade_crate() {
    let map = transync::AlignmentMap::default();
    assert_eq!(
        map.generator.version,
        env!("CARGO_PKG_VERSION"),
        "GeneratorMeta stamps transync-syntax's version (DCR-0017 M6); \
         it must stay in lockstep with the facade via version.workspace"
    );
    assert_eq!(map.generator.name, "transync");
}
```

- [ ] **Step 4: Run the new test to verify it passes AND that it catches drift**

Run: `cargo test -p transync --test public_surface -- --test-threads=4` — PASS.
Then prove the gate bites: temporarily delete one row from §0, rerun, expect `surface_table_matches_the_documented_list` FAIL naming the row; restore it, rerun, PASS.

- [ ] **Step 4b: Drop the facade's unused `transync-syntax` dependency**

Task 9 removed `pub use transync_syntax;` — the facade no longer references the crate anywhere. Delete the `transync-syntax` entry from `crates/transync/Cargo.toml` `[dependencies]`. The workspace build plus this task's compile-as-assertion test prove the facade builds without it. If compilation reveals a hidden need (e.g. a feature forward), report it rather than silently re-adding.

- [ ] **Step 5: Gates + commit**

Run: `cargo clippy --all-targets -- -D warnings && cargo test --workspace -- --test-threads=4`
```bash
git add crates/transync/tests/public_surface.rs crates/transync/Cargo.toml docs/architecture/contracts.md crates/transync-core/src/llm/prompt.rs
git commit -m "test+docs: contracts.md §0 surface table + bidirectional drift gate + M6 version guard (OI-0027)"
```

---

### Task 12: Hook JS-section back-port + standing rustdoc gate in smoke.sh

Spec §6, plus an owner-approved addition (2026-08-04): the wave drove `cargo doc` to zero warnings, but nothing enforces it going forward — smoke.sh gains the gate line. The active `.git/hooks/pre-commit` carries an improved JS section the tracked copy lacks. Make the tracked copy canonical, then reinstall.

**Files:**
- Modify: `scripts/hooks/pre-commit`
- Modify: `scripts/smoke.sh`

**Interfaces:** none (infra chore).

- [ ] **Step 1: Back-port**

Run `diff scripts/hooks/pre-commit .git/hooks/pre-commit` to see the delta. Replace the tracked copy's JS block (the `if [ -f package.json ]; then … pnpm/npx … prettier/eslint/tsc … fi` section) with the active copy's version verbatim: the `js_dir` discovery loop over `. web`, the `run_js_tool()` helper (skip-with-notice when a tool is missing), biome-first with prettier+eslint fallback, and the `tsconfig.json`-guarded `tsc --noEmit`. KEEP the tracked copy's header lines (`tracked under version control — R0008-0058` and the `Install with: scripts/install-hooks.sh` note) and every non-JS section (Rust gates, wasm gate) untouched.

- [ ] **Step 2: Reinstall + verify convergence**

Run: `scripts/install-hooks.sh && diff scripts/hooks/pre-commit .git/hooks/pre-commit`
Expected: no diff output (identical copies).

- [ ] **Step 2b: Add the standing rustdoc gate to smoke.sh (owner-approved 2026-08-04)**

Append to the gate sequence in `scripts/smoke.sh`, matching the file's existing echo/run style:
```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p transync-syntax -p transync-core -p transync
```
Smoke-only — the pre-commit hook stays as-is (a doc build on every commit would stretch the hook; smoke.sh is the fuller gate). Verify by running `scripts/smoke.sh` once to completion: it must pass, with the doc line visibly executed.

- [ ] **Step 3: Verify the hook still runs end-to-end + commit**

```bash
git add scripts/hooks/pre-commit scripts/smoke.sh
git commit -m "chore: hook JS-section back-port (DCR-0017 handover) + standing rustdoc gate in smoke.sh"
```
The commit itself exercises the hook — watch its output for the `[pre-commit]` JS lines (SKIP notices are the expected shape while no JS linter is installed).

---

### Task 13: Records + final gates

Spec §8. Close the books: DCR-0018, OI-0027 RESOLVED, DCR-0005 note, status/phase-state, and the full gate suite.

**Files:**
- Create: `docs/project/design-change-records/archive/DCR-0018-public-surface-curation.md`
- Modify: `docs/project/open-issues.md` (OI-0027 entry)
- Modify: `docs/project/design-change-records/DCR-0005-facade-and-core-crate-split.md` (dated note)
- Modify: `docs/project/status.md`, `docs/project/phase-state.yaml`
- Check: `docs/implementation/module-map.md` (stale "public surface" comments)

**Interfaces:**
- Consumes: everything; write records against `git log` reality, not the plan.

- [ ] **Step 1: Write DCR-0018 (follow DCR-0017's structure: Context / Change / Migration list / Guards / Verification)**

Must cover, with the actual commit hashes from `git log --oneline`: the closure map (which modules went `pub(crate)`, `unit`'s `#[doc(hidden)]` + dev-dep rationale, the three syntax narrowings); the explicit facade list replacing the glob and the removal of `pub use transync_syntax;`; the `OutputBudgetWarning` relocation; the root-alias additions (`ParseError`, `GlossaryScope`, `ALIGNMENT_SCHEMA_VERSION`, `OutputBudgetWarning`); the `#[non_exhaustive]` set (three enums + five structs + `TranslationUnit`) and the constructor; the exhaustive-by-policy set; the migration list (scn_10, live_smoke dev-deps, FRU sweep incl. the `opts_for` helper, id/align path migrations, **the resp-translator commit — cite its hash from that repo**, with the empty-`target_language` behavioral delta); the drift test's five parts; the seven rustdoc de-links; the hook back-port. State the invariants: alignment schema stays `1.2.0`, `VALIDATION_SCHEMA_VERSION` stays `2`, `CacheKey` and both `sync.js` copies untouched. No line-number references.

- [ ] **Step 2: Resolve OI-0027 in `docs/project/open-issues.md`**

Status → `**RESOLVED (2026-08-04)**` with a Resolution block in the house style (mirror OI-0028's): resolving record DCR-0018 + the approved spec path; note the two owner-decision corrections recorded there (the FRU parenthetical was unsound → default-then-assign; `TranslateOutput` → `TranslationOutput`); check off the three Required Actions; add a Verification checklist (facade explicit ✓, attributes ✓, drift test ✓, doc warnings 0 ✓, sibling repo migrated ✓, full gates ✓ — each with the date).

- [ ] **Step 3: DCR-0005 dated note**

Append one dated paragraph: 2026-08-04 — the facade firewall this DCR created is now *curated* (explicit re-export list, compiler-enforced tier boundary, drift-tested against contracts.md §0); see DCR-0018.

- [ ] **Step 4: Update `status.md` and `phase-state.yaml`**

- `phase-state.yaml`: append `DCR-0018-public-surface-curation` to `closed_change_records`; `last_updated: 2026-08-04-oi0027-surface-curation-landed`; prepend a `notes:` paragraph summarizing the wave (mirror the DCR-0017 paragraph's density); update the "Next in sequence" sentence to "0.2.0 release".
- `status.md`: current-phase section gains the same summary; OI-0027 moves to resolved; next action = 0.2.0 release.
- `module-map.md`: grep for `public surface` / `pub use transync_core::*` mentions; update any now-false statement (e.g. the facade file-tree comment).

- [ ] **Step 5: Final full gate run**

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings \
  && cargo test --workspace -- --test-threads=4 \
  && cargo check -p transync-syntax --target wasm32-unknown-unknown \
  && cargo doc --no-deps -p transync-syntax -p transync-core -p transync \
  && scripts/smoke.sh && scripts/test-browser.sh
```
Expected: everything green; doc build warning-free; Playwright 8/8. Also from `/Volumes/Common/QJoon/resp-translator`: `cargo test --workspace -- --test-threads=4` — green.

- [ ] **Step 6: Commit**

```bash
git add docs/
git commit -m "docs: DCR-0018 public-surface curation — OI-0027 RESOLVED, window-closing records"
```

---

## Baseline numbers (for reviewers)

- Test baseline at plan time: 335 passed / 0 failed / 3 ignored workspace-wide (post-DCR-0017). Task 8 adds +1 (constructor test), Task 11 adds +3 (drift/negative/M6 tests). No test removals anywhere in this plan.
- `grep -c "non_exhaustive"` across `crates/`: 2 before (TokenizerHint, CacheError) → 11 after (adds 3 enums + 5 structs + TranslationUnit).
