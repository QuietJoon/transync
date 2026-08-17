# transync-syntax Crate Split + AST-Direct Renderer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract `transync-syntax` (parser/id/regen/render/align/htmlseg/outcome + ParseError) as a wasm32-compilable base crate beneath `transync-core`, and replace the per-block-reparse renderer with whole-document AST-direct rendering.

**Architecture:** Move the six syntax-side modules into a new fifth workspace member; dissolve the three boundary inversions (HtmlOutcome closure relocation, neutral `regenerate`/`build_alignment_map` signatures); keep every existing path resolving via module re-exports in core; then rework the renderer to one comrak parse per pane with a per-kind emission table, guarded by a shared normalization/item-count helper consumed by both the renderer and `validate::full_reparse`.

**Tech Stack:** Rust workspace (comrak 0.27, lol_html 2.9, htmlize 1.1, siphasher, serde/serde_json, thiserror), wasm32-unknown-unknown check gate.

**Authoritative spec:** `docs/superpowers/specs/2026-08-04-transync-syntax-split-design.md` (v2, commit 92761c9). Where this plan and the spec disagree, the spec wins — stop and flag it.

## Global Constraints

- Temp files ONLY under `/Volumes/Temp/claude/` (never `/tmp` directly, never `/private/tmp`).
- NEVER set/override `CARGO_TARGET_DIR`; never pass `--target-dir`. If the target volume is unreachable, stop and ask.
- Every test run: `-- --test-threads=4`. Both cargo gate suites at every task boundary: `cargo test --workspace -- --test-threads=4` AND `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`.
- Lint gate (pre-commit hook enforces): `cargo fmt --all -- --check` + `cargo clippy --all-targets --all-features -- -D warnings`.
- The tree must be GREEN at every task boundary — module re-exports exist precisely so intermediate states compile.
- No pure-formatting edits. Korean `*.ko.md` files: never read/edit/cite.
- Commits: small, logically coherent; end every message with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- transync-syntax gets NO `[features]` block and NO core-directed dev-dependency (either would defeat the wasm gate — spec §2.1/§3).
- comrak stays `{ workspace = true }` in BOTH syntax and core (shared resolved version; parser's public signatures carry comrak types).
- Owner latitude (spec header): path preservation is the default, not an obligation; improvement-driven breaking changes are allowed — record every intentional break in the task report for DCR-0017's migration list.
- Context digests from the design exploration (dependency edges with file:line, render architecture, manifests/hooks verbatim) live at `/Volumes/Temp/claude/oi0028-context/{depgraph,renderside,touchpoints}.md` — implementers of Tasks 2–7 should read the relevant digest section before moving code.

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `Cargo.toml` (workspace) | modify | add `crates/transync-syntax` to explicit members |
| `crates/transync-syntax/Cargo.toml` | **create** | base-crate manifest (comrak, serde, serde_json, siphasher, lol_html, htmlize, thiserror) |
| `crates/transync-syntax/src/lib.rs` | **create** | module decls + curated re-exports |
| `crates/transync-syntax/src/{parser.rs, parser/, id.rs, regen.rs, render.rs, render/, align.rs, htmlseg.rs}` | **moved** | verbatim moves (git mv) + boundary edits |
| `crates/transync-syntax/src/error.rs` | **create** | `ParseError` (moved from core error.rs) |
| `crates/transync-syntax/src/outcome.rs` | **create** | the six relocated unit.rs items |
| `crates/transync-syntax/src/walk.rs` | **create** | shared normalization + direct-item-count helper |
| `crates/transync-core/src/lib.rs` | modify | module decls → `pub use transync_syntax::…` re-exports |
| `crates/transync-core/src/{unit.rs, pipeline.rs, validate.rs, validate/full_reparse.rs, error.rs}` | modify | cross-crate consumption + neutral-signature call sites + Guard 1 |
| `crates/transync/Cargo.toml` + `src/lib.rs` | modify | gains transync-syntax dep + `pub use transync_syntax;` |
| `scripts/hooks/pre-commit`, `.git/hooks/pre-commit`, `scripts/smoke.sh` | modify | wasm gate lines |
| `.claude/settings.local.json` | modify | `Bash(cargo check:*)` allowlist |
| docs (DCR-0017, ADR-0003/0006/0007 + DCR-0005/0007 amendments, sweep) | modify/create | records per spec §7 |

---

### Task 1: Canary, crate scaffold, gates

**Files:**
- Create: `crates/transync-syntax/Cargo.toml`, `crates/transync-syntax/src/lib.rs`
- Modify: `Cargo.toml` (workspace members), `crates/transync/Cargo.toml` + `crates/transync/src/lib.rs`, `scripts/hooks/pre-commit`, `.git/hooks/pre-commit`, `scripts/smoke.sh`, `.claude/settings.local.json`

**Interfaces:**
- Produces: an empty-but-gated `transync-syntax` crate every later task moves code into; the standing wasm gate.

- [ ] **Step 1: Dependency canary (spec §5 first-task rule)** — under `/Volumes/Temp/claude/syntax-wasm-probe/`, `cargo init`, depend on the FULL syntax dep set at the workspace's currently-resolved versions (read Cargo.lock on disk): `comrak`, `serde` (derive), `serde_json`, `siphasher`, `lol_html`, `htmlize` (unescape), `thiserror`; `main.rs` references one symbol from each; `cargo check --target wasm32-unknown-unknown` (target is installed; if the check fails, STOP and report BLOCKED — the whole split's premise needs re-examination). Record versions + verdict.

- [ ] **Step 2: Scaffold the crate**

`crates/transync-syntax/Cargo.toml`:
```toml
# transync-syntax: the wasm32-compilable base crate — GFM parsing, block IR
# and IDs, regeneration/splicing, alignment maps, HTML rendering, and the
# lol_html segment engine. transync-core sits on top with the pipeline.
# No [features] and no core-directed dev-dependency, ever: both would
# defeat the standing wasm gate (spec 2026-08-04 §2.1/§3).
# TRACE: ADR-0003 (amended)
# TRACE: DCR-0017

[package]
name         = "transync-syntax"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
repository.workspace   = true
description  = "GFM block IR, parser, renderer, and alignment maps for transync (wasm32-compilable base crate)."

[dependencies]
comrak      = { workspace = true }
serde       = { workspace = true }
serde_json  = { workspace = true }
siphasher   = { workspace = true }
thiserror   = { workspace = true }
lol_html    = { workspace = true }
htmlize     = { workspace = true }
```

`src/lib.rs` (initial):
```rust
//! transync-syntax: the wasm32-compilable base crate for transync.
//!
//! Owns everything syntax-side: GFM parsing into the block IR, block IDs,
//! Markdown regeneration/splicing, alignment-map construction, annotated
//! HTML rendering, and the lol_html segment engine. `transync-core` builds
//! the LLM pipeline on top. Gate: this crate must always pass
//! `cargo check -p transync-syntax --target wasm32-unknown-unknown`.
//!
//! TRACE: DCR-0017
```

Workspace `Cargo.toml`: insert `"crates/transync-syntax",` FIRST in the members list (dependency order convention). Facade: add `transync-syntax = { path = "../transync-syntax" }` to `crates/transync/Cargo.toml` `[dependencies]` and `pub use transync_syntax;` under the existing glob in `crates/transync/src/lib.rs` (with a one-line comment: direct base-crate naming, spec §2.3).

- [ ] **Step 3: Gates.** In BOTH `scripts/hooks/pre-commit` and `.git/hooks/pre-commit` (byte-identical logic today — keep them so), after the clippy line inside the `if [ -f Cargo.toml ]` block:
```bash
  if rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
    echo "[pre-commit] cargo check -p transync-syntax --target wasm32-unknown-unknown"
    cargo check -p transync-syntax --target wasm32-unknown-unknown || fail=1
  else
    echo "[pre-commit] FAIL: wasm32-unknown-unknown target missing — run: rustup target add wasm32-unknown-unknown" >&2
    fail=1
  fi
```
In `scripts/smoke.sh`, next to the existing `cargo build --workspace` line, add the same check unconditionally (no rustup guard — the hard gate assumes a full toolchain):
```bash
cargo check -p transync-syntax --target wasm32-unknown-unknown
```
`.claude/settings.local.json`: add `"Bash(cargo check:*)"` to the allow list (match the existing entry style).

- [ ] **Step 4: Verify** — `cargo check -p transync-syntax --target wasm32-unknown-unknown` (empty crate passes; deps compile for wasm), then both cargo gate suites (nothing else changed → green), fmt + clippy.

- [ ] **Step 5: Commit** — `deps/scaffold: transync-syntax base crate + standing wasm32 gate (spec §2.1, §5)` (+ canary verdict line in the body, + trailer).

---

### Task 2: Move the leaf cluster — id, parser(+ranges,+refdefs), ParseError

**Files:**
- Move (git mv): `crates/transync-core/src/id.rs`, `src/parser.rs`, `src/parser/` → same paths under `crates/transync-syntax/src/`
- Create: `crates/transync-syntax/src/error.rs`
- Modify: `crates/transync-syntax/src/lib.rs`, `crates/transync-core/src/lib.rs`, `crates/transync-core/src/error.rs`

**Interfaces:**
- Produces: `transync_syntax::{id, parser, error::ParseError}`; core paths `crate::id::…`/`crate::parser::…` keep resolving via module re-exports.
- Consumes: nothing from later tasks.

**Digest reference:** `/Volumes/Temp/claude/oi0028-context/depgraph.md` §1 (parser/id edges: the ONLY non-mover edge is `ParseError`).

- [ ] **Step 1: Move + wire.** `git mv` the three paths. `transync-syntax/src/error.rs`:
```rust
//! Parse-side error type. `transync-core`'s `TransyncError` wraps it via
//! `#[from]` — the cross-crate From derive works unchanged.

/// Errors from [`crate::parser::parse`].
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("comrak parse failed: {0}")]
    Comrak(String),

    #[error("unsupported syntax: {0}")]
    Unsupported(String),
}
```
Delete the `ParseError` enum from core `error.rs`; change its variant to `Parse(#[from] transync_syntax::error::ParseError)` and add `pub use transync_syntax::error::ParseError;` (path `transync::error::ParseError` keeps resolving). Syntax `lib.rs` gains:
```rust
pub mod error;
pub mod id;
pub mod parser;
```
Core `lib.rs`: delete `pub mod id;` / `pub mod parser;`, add in their alphabetical slots:
```rust
pub use transync_syntax::id;
pub use transync_syntax::parser;
```
(Module re-exports, NOT globs — `crate::parser::ranges::…`/`crate::parser::refdefs` must keep resolving.) The `pub use id::{BlockId, BlockKind};` root re-export line becomes `pub use transync_syntax::id::{BlockId, BlockKind};`.
Inside the moved files: `use crate::error::ParseError` (parser.rs:14) stays valid (syntax now has its own `error`); `crate::id::…` self-references stay valid. `parser/refdefs.rs`: convert the rustdoc intra-doc links naming `crate::validate::…`/`crate::regen`/`crate::render` into plain-code-span text (they can't resolve from syntax; the regen/render ones will resolve again after Tasks 4–5 but keep them plain for symmetry — note it in the DCR).

- [ ] **Step 2: Compile-fix sweep.** `cargo check --workspace` names any residue (expected: none — depgraph verified core consumes parser/id only via `crate::id`/`crate::parser` paths, which the re-exports preserve; `test_fixtures.rs`/`profile.rs` test uses resolve the same way).

- [ ] **Step 3: Gates** — both cargo suites + wasm check (now compiles comrak into the gate) + fmt + clippy.

- [ ] **Step 4: Commit** — `refactor: move parser/id/ParseError into transync-syntax (spec §2.2–§2.3)`.

---

### Task 3: Move htmlseg + create the outcome module

**Files:**
- Move: `crates/transync-core/src/htmlseg.rs` → `crates/transync-syntax/src/htmlseg.rs`
- Create: `crates/transync-syntax/src/outcome.rs`
- Modify: syntax `lib.rs`, core `lib.rs`, core `unit.rs`, core `validate.rs`, core `pipeline.rs`, core `align.rs` (import lines only), `crates/transync-core/Cargo.toml`

**Interfaces:**
- Produces: `transync_syntax::htmlseg` (`#[doc(hidden)] pub mod`; items `extract`, `splice`, `tag_inventory`, `HtmlSegments{texts,labels}`, `balance_fragment` become `pub`); `transync_syntax::outcome::{HtmlOutcome, html_outcomes}` (pub, real API) + `{block_payload, is_translatable, is_translatable_block, has_translatable_blocks}` (`pub` + `#[doc(hidden)]`); core `unit` re-exports `pub use transync_syntax::outcome::{HtmlOutcome, html_outcomes};` (the `transync::unit::html_outcomes` path that live_smoke uses keeps resolving).

**Digest reference:** depgraph.md §2 (htmlseg consumers both sides), §3 (the six items' closure).

- [ ] **Step 1: Move htmlseg.** In syntax `lib.rs`: `#[doc(hidden)] pub mod htmlseg;` with a comment (internal engine — consumed by regen/render here and unit/validate in core; not curated API, OI-0027 gets the list). Inside htmlseg.rs, widen `pub(crate)` → `pub` on: `extract`, `splice`, `tag_inventory`, `HtmlSegments`, `balance_fragment` (regen/render consume it after Tasks 4–5 from within syntax, but pub is needed for core's transitional call in render until Task 5 — simpler: pub now, note in DCR). `scan`, `scan_chunks`, `scan_with_memory_cap`, `collapse_blank_lines`, `NodeRecord` stay `pub(crate)` (their only consumers are htmlseg's own tests, which move with the file).

- [ ] **Step 2: Create outcome.rs** — move VERBATIM from core `unit.rs` (unit.rs:191–296 region; exact bodies, keep every doc comment incl. the `# Panics` section on nothing — the `# Panics` section lives on `build_batches`, which stays): `HtmlOutcome` (enum + derives + docs), `html_outcomes`, `is_translatable`, `is_translatable_block`, `has_translatable_blocks`, `block_payload`. Adjust internals: `crate::htmlseg::extract` still resolves (same crate now); imports become `use crate::id::{BlockId, BlockKind}; use crate::parser::{Block, Document}; use std::collections::HashMap;`. Visibility: `HtmlOutcome`/`html_outcomes` plain `pub` with their existing docs; the other four `pub` + `#[doc(hidden)]` + a one-line comment (`// pub only because the crate boundary forces it — not curated API (DCR-0017 hands this list to OI-0027)`). Syntax lib.rs: `pub mod outcome;`.

- [ ] **Step 3: Rewire core.** core `lib.rs`: `pub(crate) mod htmlseg;` deleted; DO NOT re-export htmlseg from core (core-internal callers switch to `transync_syntax::htmlseg::…` directly — cleaner than a shim, allowed by owner latitude; the facade never exposed it). core `unit.rs`: delete the six items; add `pub use transync_syntax::outcome::{HtmlOutcome, html_outcomes};` (path contract) and `use transync_syntax::outcome::is_translatable_block;` where `build_batches` uses it; `build_batches`'s payload construction imports `transync_syntax::outcome::block_payload`. core `validate.rs` (378, 381–382, 862) and `unit.rs` (69, 221): `crate::htmlseg::` → `transync_syntax::htmlseg::`. core `align.rs` (still in core until Task 4): `use crate::unit::HtmlOutcome;` keeps resolving via the unit re-export; `crate::unit::is_translatable_block` at align.rs:172 → `transync_syntax::outcome::is_translatable_block`. `pipeline.rs`/`profile.rs` `crate::unit::html_outcomes` call sites keep resolving via the re-export (verify, don't touch).

- [ ] **Step 4: Gates** (both cargo suites + wasm + fmt/clippy). live_smoke compiles via `cargo test -p transync-openai --no-run` (the `transync::unit::html_outcomes` path must still resolve — this is the review's should-fix #4 pin).

- [ ] **Step 5: Commit** — `refactor: move htmlseg + HtmlOutcome closure into transync-syntax (spec §2.2, §3.1)`.

---

### Task 4: Neutral signatures; move regen + align

**Files:**
- Move: `crates/transync-core/src/regen.rs`, `src/align.rs` → syntax
- Modify: syntax `lib.rs`, core `lib.rs`, core `pipeline.rs`, core `validate/full_reparse.rs` (import lines), core `test_fixtures.rs` (imports)

**Interfaces:**
- Produces (spec §3.2/§3.3, breaking — record in DCR migration list):
  - `transync_syntax::regen::regenerate(doc: &Document, accepted: &HashMap<BlockId, String>) -> (String, BlockOffsets)`; `pub fn top_level_blocks(...)` (widened from pub(crate));
  - `transync_syntax::align::build_alignment_map(doc: &Document, statuses: &HashMap<BlockId, FallbackStatus>, offsets: &BlockOffsets, source_language: &str, target_language: &str, detected_source_language: Option<String>, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> AlignmentMap`;
  - core `lib.rs`: `pub use transync_syntax::{regen, align};` + root value re-exports rewired (`pub use transync_syntax::align::{AlignmentBlock, AlignmentMap, ByteRange, FallbackStatus, GeneratorMeta, SyncRole, ValidationSummary};`).
- Consumes: `outcome::HtmlOutcome` (Task 3).

**Digest reference:** depgraph.md §3 (inversion inventory verbatim — collect_translations reads unit_id+accepted_payload only; align reads unit_id+final_status + two language strings), §8 (the seven test helpers that dissolve).

- [ ] **Step 1: Neutralize regen.** In regen.rs (before moving): delete `use crate::validate::ValidatedBatch;`; change `regenerate`'s second parameter to `accepted: &HashMap<BlockId, String>`; delete `collect_translations` (the map IS the parameter now); the call site match becomes:
```rust
        let (payload, translated) = match accepted.get(&block.block_id) {
            Some(p) => (p.clone(), true),
            None => (doc.source_text[bstart..bend].to_string(), false),
        };
```
Rewrite the `html_regen_tests` `validated()` helper to build the map directly:
```rust
    fn accepted_map(unit_id: &str, payload: Option<&str>) -> HashMap<BlockId, String> {
        payload
            .map(|p| HashMap::from([(BlockId(unit_id.to_string()), p.to_string())]))
            .unwrap_or_default()
    }
```
(and drop the now-unused `FallbackStatus` argument from the three tests — a fallback unit is simply absent from the map, which is exactly regen's real contract).

- [ ] **Step 2: Neutralize align.** In align.rs (before moving): replace `use crate::TranslateOptions; use crate::unit::HtmlOutcome; use crate::validate::ValidatedBatch;` with `use crate::outcome::HtmlOutcome;`; new signature per Interfaces; the `by_id` loop is deleted (the `statuses` parameter IS `by_id`); `opts.source_language.clone()`/`opts.target_language.clone()` become `source_language.to_string()`/`target_language.to_string()`. Rewrite the two test call sites to pass `&HashMap::new()` / hand-built status maps and `"auto", "ko"` literals (the helpers already synthesize statuses — depgraph §8 item 5–6).

- [ ] **Step 3: Move both files** (git mv), syntax lib.rs `pub mod regen; pub mod align;`, widen `top_level_blocks` to `pub` with a doc line (consumed by core's pipeline). Core lib.rs: swap module decls for re-exports; rewire the root value re-export list to `transync_syntax::align::{…}` (identical names — `crate::FallbackStatus` at validate.rs/test_fixtures.rs/pipeline.rs keeps resolving).

- [ ] **Step 4: Fix core call sites.** `pipeline.rs` `regen_pass` builds the payload map (this replaces `collect_translations`'s job, core-side):
```rust
fn regen_pass(
    doc: &transync_syntax::parser::Document,
    accepted: &HashMap<BlockId, ValidatedUnit>,
) -> (String, transync_syntax::regen::BlockOffsets, Vec<crate::validate::ValidatedBatch>) {
    let final_validated = collect_validated(doc, accepted);
    let payloads: HashMap<BlockId, String> = accepted
        .iter()
        .filter_map(|(id, vu)| vu.accepted_payload.clone().map(|p| (id.clone(), p)))
        .collect();
    let (md, offsets) = regenerate(doc, &payloads);
    (md, offsets, final_validated)
}
```
`build_alignment_map` call site (pipeline.rs ~319) builds the status map from `final_validated` (unit_id → final_status) and passes `&opts.source_language, &opts.target_language`. `validate/full_reparse.rs`'s `crate::align::ByteRange`/`crate::regen::BlockOffsets` imports keep resolving via the module re-exports (verify only).

- [ ] **Step 5: Gates + commit** — `refactor: neutral regen/align signatures; move both into transync-syntax (spec §3.2–§3.3)`.

---

### Task 5: Move render (+attrs), tests made syntax-local

**Files:**
- Move: `crates/transync-core/src/render.rs`, `src/render/` → syntax
- Modify: syntax `lib.rs`, core `lib.rs`, core `pipeline.rs` (import line only)

**Interfaces:**
- Produces: `transync_syntax::render::{render_source, render_target, fallback_status_str}` (signatures unchanged this task); core `pub use transync_syntax::render;`.
- Consumes: neutral `build_alignment_map` (Task 4), `outcome::html_outcomes` (Task 3), `htmlseg::balance_fragment` (same crate now).

- [ ] **Step 1: Rewrite render's four test helpers to syntax-local form** (they are the last core-type users in the moving set — depgraph §8 items 1–4): `render(src)` and siblings call the NEUTRAL `build_alignment_map(&doc, &HashMap::new(), &BlockOffsets::default(), "auto", "ko", None, &html_outcomes(&doc))`; `render_with_status(src, status)` builds `statuses: HashMap<BlockId, FallbackStatus>` directly for each html block id (replacing its `ValidatedUnit`/`ValidatedBatch` construction — the map is what align consumes now); `refmap_render_tests` calls `regenerate(&doc, &HashMap::new())`. No `TranslateOptions` anywhere in the file afterward (grep-verify).

- [ ] **Step 2: Move** (git mv), syntax lib.rs `pub mod render;`, core lib.rs re-export swap, `crate::htmlseg::balance_fragment` at render.rs:195 stays valid (same crate). `pipeline.rs:29`'s `use crate::render::{render_source, render_target};` keeps resolving via the re-export.

- [ ] **Step 3: Gates + commit** — `refactor: move render into transync-syntax; render tests syntax-local (spec §2.2)`.

---

### Task 6: Shared walk helper + full_reparse item-count check (Guard 1)

**Files:**
- Create: `crates/transync-syntax/src/walk.rs`
- Modify: syntax `lib.rs`, core `validate/full_reparse.rs`, core `pipeline.rs` (only if imports shift)

**Interfaces:**
- Produces:
```rust
// transync_syntax::walk
/// One normalized top-level entry: consecutive ListItem blocks sharing an
/// ast_path prefix collapse into a single "list" entry (the same rule the
/// renderer's zip and validate::full_reparse both build on — ONE home,
/// spec §4.1).
pub struct NormalizedEntry {
    pub label: &'static str,
    pub sources: Vec<BlockId>,
}
pub fn normalize_top_level(doc: &Document) -> Vec<NormalizedEntry>;
/// Direct Item|TaskItem children of a List node — nested lists' items are
/// NOT counted (spec §4.3).
pub fn direct_item_count<'a>(list: &'a comrak::nodes::AstNode<'a>) -> usize;
```
- Consumes: `parser::Document`, `id::{BlockId, BlockKind}`.

- [ ] **Step 1: Extract `normalize_top_level`.** Core `validate/full_reparse.rs` owns the current implementation (`normalize_for_top_level`, private `Normalized{label, sources}`, `label_for`). Move the normalization walk + `label_for` into `walk.rs` (label_for's `BlockKind` match is syntax-typed — it belongs there), keeping the collapse-by-`ast_path`-prefix rule byte-identical. `full_reparse.rs` consumes `transync_syntax::walk::{normalize_top_level, NormalizedEntry}` and deletes its local copies; its `Normalized` uses swap to `NormalizedEntry` (same fields). Behavior change: none — pin by the existing full_reparse inline tests staying green untouched.

- [ ] **Step 2: `direct_item_count`** (in walk.rs):
```rust
pub fn direct_item_count<'a>(list: &'a comrak::nodes::AstNode<'a>) -> usize {
    use comrak::nodes::NodeValue;
    list.children()
        .filter(|c| {
            matches!(
                c.data.borrow().value,
                NodeValue::Item(_) | NodeValue::TaskItem(_)
            )
        })
        .count()
}
```
With walk.rs inline tests: plain list = N; task list = N; a list whose item CONTAINS a nested list counts only the outer items; empty non-list node = 0.

- [ ] **Step 3: Guard 1 in full_reparse.** After the existing label-sequence compare succeeds, add the item-count pass: for each `(i, entry)` where `entry.label == "list"`, the reparsed top-level node at index `i` is a `List` — compare `direct_item_count(node)` against `entry.sources.len()`; on mismatch return `Err(ReparseFailure { reason: format!("list at block {i}: source has {} items, regenerated has {}", entry.sources.len(), got), divergent_source_blocks: entry.sources.clone() })`. (This slots into the existing failure/attribution machinery — the cascade downgrades exactly the list's item blocks.)

- [ ] **Step 4: Tests (spec §6 — MUST bypass per_kind).** In full_reparse's inline test module, call `reparse_full` DIRECTLY with crafted `regenerated_md`: (a) source `- a\n- b\n` vs regenerated `- a\n- b\n- c\n` → Err containing "items"; (b) merge (regenerated one item) → Err; (c) unchanged → Ok; (d) task-list variant `- [ ] a\n- [x] b\n` count 2 → Ok, split → Err. Plus ONE pipeline-level test (in pipeline.rs's inline tests, minimal inline Translator per existing pattern) for the None-constraint residual: a source list item whose payload defeats `inspect_list_topology` (e.g. an item whose payload slice reparses as a bare paragraph) and whose translation splits into two items — asserts the run completes with that block `fallback_source` (Guard 1 caught what per_kind could not).

- [ ] **Step 5: Gates + commit** — `feat: shared walk helper + full_reparse per-list item-count check (spec §4.3)`.

---

### Task 7: AST-direct renderer (R1)

**Files:**
- Modify: `crates/transync-syntax/src/render.rs` (the rework), `crates/transync-syntax/src/walk.rs` (only if the zip needs a helper tweak — keep ONE home)

**Interfaces:**
- Consumes: `walk::normalize_top_level` semantics (the zip pairs rows the same way), `htmlseg::balance_fragment`, comrak 0.27 (`format_html` on arbitrary nodes; `WriteWithLast` resets per call).
- Produces: same public signatures (`render_source`, `render_target`); `strip_outer_wrapper`, `block_inner_html`, `ordered_list_start` DELETED.

**Digest reference:** renderside.md §3–§4 (why one parse suffices + the comrak formatter surface, verbatim), and spec §4.1's emission table — the load-bearing content of this task; read both before coding.

- [ ] **Step 1: Write the equivalence/regression tests FIRST** (render.rs inline, plus updates to existing pins):

```rust
// Spec §4.1/§4.4: AST-direct rendering — per-kind emission + assembly.
#[cfg(test)]
mod ast_direct_tests {
    use super::*;

    #[test]
    fn code_block_content_survives_whole_node_formatting() {
        let html = render("```rust\nfn main() {}\n```\n");
        assert!(html.contains("<pre><code class=\"language-rust\">"), "{html}");
        assert!(html.contains("fn main() {}"), "code content present:\n{html}");
    }

    #[test]
    fn table_keeps_its_element_and_tbody() {
        let html = render("| a | b |\n|---|---|\n| 1 | 2 |\n");
        for needle in ["<table>", "<tbody>", "</tbody>", "</table>"] {
            assert!(html.contains(needle), "missing {needle}:\n{html}");
        }
    }

    #[test]
    fn blockquote_keeps_its_element() {
        let html = render("> quoted\n");
        assert!(html.contains("<blockquote>"), "{html}");
    }

    #[test]
    fn task_item_checkbox_survives() {
        let html = render("- [x] done\n- [ ] todo\n");
        assert_eq!(html.matches("type=\"checkbox\"").count(), 2, "{html}");
        assert!(html.contains("checked"), "{html}");
    }

    #[test]
    fn loose_source_list_renders_loose_in_the_pane() {
        // v2 spec §4.4: source-driven looseness now faithful (old per-item
        // reparse under-reported it).
        let html = render("- a\n\n- b\n");
        assert!(html.contains("<p>a</p>"), "loose items gain <p>:\n{html}");
    }

    #[test]
    fn tight_item_with_nested_list_gets_the_cr_newline() {
        // The cr()-emulation case: tight paragraph then nested list.
        let html = render("- top\n  - nested\n");
        assert!(html.contains("top\n<ul>"), "newline before nested list:\n{html}");
    }

    #[test]
    fn wrapper_close_stays_on_the_block_line() {
        // Trailing-newline trim: scn_08's same-line pin depends on this.
        let html = render("| a |\n|---|\n| 1 |\n");
        let line = html
            .lines()
            .find(|l| l.contains("data-sync-id=\"t-"))
            .expect("table line");
        assert!(line.ends_with("</div>"), "one line per block:\n{line}");
    }

    #[test]
    fn reference_links_resolve_without_the_append() {
        let html = render("See [docs][ref].\n\n[ref]: https://example.com/r\n");
        assert!(html.contains("<a href=\"https://example.com/r\""), "{html}");
    }
}
```
(`render(src)` is the Task-5 helper. Existing pins to UPDATE in the same commit where shapes legitimately changed: none of the list_grouping/html/skipped pins should move — verify; if a pin fails, judge whether the new byte shape is the spec-accepted change (loose lists, inter-child newlines) and update ONLY those, documenting each.)

- [ ] **Step 2: Run them** — the loose-list, cr-newline tests FAIL against the old renderer (it under-reports looseness); the rest pass (characterizing). Confirm which fail and why before rework.

- [ ] **Step 3: The rework.** Replace `render_fragment`'s internals:

```rust
fn render_fragment(
    doc: &Document,
    alignment: &AlignmentMap,
    pane_md: &str,
) -> String
```
(callers: `render_source` passes `&doc.source_text`, `render_target` passes `translated_md` — the closure indirection dissolves; `block_text` stays for the Guard-2 degrade arm only).

Body outline (real code, adapt names to the file):
1. `let arena = comrak::Arena::new(); let opts = parser::comrak_options(); let root = comrak::parse_document(&arena, pane_md, &opts);`
2. `let top: Vec<&AstNode> = root.children().collect();`
3. Walk `alignment.blocks` exactly as today (skip non-top-level/missing), maintaining `node_idx`; group ListItem runs by `(tag, ast_path-prefix)` as today, but the group's `<ol start>` comes from the CURRENT node's `NodeValue::List(list)` (`list.start`, `list.list_type`), and all items in the run pair with that one List node's `Item|TaskItem` children (an item iterator advanced per row).
4. Per row, emission per the spec §4.1 table:
   - Heading/Paragraph: `format_children(node)`;
   - Image: `format_children(paragraph_node)` into `<figure>`;
   - ListItem: if the paired item is `TaskItem(symbol)`, first push `<input type="checkbox" disabled="" />` / `<input type="checkbox" checked="" disabled="" /> ` per comrak's exact byte shape (copy it from vendored html.rs:1021-1038 so DOM equality holds), then `format_children(item_node)`;
   - Table/CodeBlock/Blockquote: `format_node_whole(node)` (single `comrak::format_html(node, …)` call), trailing `\n` trimmed;
   - Html/Skipped/ThematicBreak: today's bypass arms verbatim (md text via `block_text(pane_md, range)` — Html needs the pane bytes for `balance_fragment`); the node still advances `node_idx`.
5. `format_children(node)`: for each child, `comrak::format_html(child, &opts, &mut buf)`; between children, if the buffer is non-empty and does not end with `\n`, push `'\n'` first (cr() emulation); after the loop trim ONE trailing `\n`.
6. Guard 2 (defensive, spec §4.3): if `node_idx` runs past `top.len()` or a row's expected kind cannot pair with the current node's value, emit that block via the OLD byte-range path (`block_text` + `html_escape` for safety on non-bypass kinds) and DO NOT advance `node_idx` further for that row. NO `debug_assert` here — the spec's §6 synthetic-mismatch test must exercise this arm gracefully. Add that test to `ast_direct_tests`:
```rust
    #[test]
    fn zip_mismatch_degrades_to_escaped_bytes_never_panics() {
        // Guard 2 (spec §4.3): a hand-built alignment row claiming a kind
        // the parsed md does not have at that position must degrade, not
        // panic, and the anchor must survive.
        let mut doc = parser::parse("plain paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let mut map = crate::align::build_alignment_map(
            &doc,
            &std::collections::HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );
        map.blocks[0].block_kind = "table".to_string(); // lie about the kind
        let html = render_source(&doc, &map);
        assert!(html.contains("data-sync-id=\"p-0001\""), "anchor survives:\n{html}");
    }
```
(Adapt the mismatch trigger to whatever the zip actually keys on — if it pairs by `block.kind` rather than the row's `block_kind` string, force the mismatch by parsing DIFFERENT md for the pane than the doc, e.g. `render_fragment` via `render_target` with a hand-made `translated_md`/offsets that reparse to a different kind sequence; the load-bearing assertions are no-panic + anchor-present.)
7. Delete `strip_outer_wrapper`, `block_inner_html`, `ordered_list_start`, the `ref_defs` parameter threading (render_block signature loses `ref_defs`; `render_fragment` no longer reads `doc.ref_defs`).

- [ ] **Step 4: Run the full render module + workspace suites**; update the spec-accepted pins (each documented). Run the browser suite (`./scripts/test-browser.sh`) — DOM-level assertions must stay green.

- [ ] **Step 5: Commit** — `feat: whole-document AST-direct renderer — per-kind emission, cr-emulation, Guard 2 (spec §4)`.

---

### Task 8: Records (spec §7)

**Files:**
- Create: `docs/project/design-change-records/archive/DCR-0017-transync-syntax-split.md`
- Modify: `docs/decisions/0003-cargo-workspace-with-provider-crates.md` (dated amendment), `0006`/`0007` (dated mechanism amendments), `docs/project/design-change-records/DCR-0005…md` + `DCR-0007…md` (notes), `docs/project/open-issues.md` (OI-0028 RESOLVED; OI-0008 narrowed), `docs/architecture/contracts.md` (§4/§4a renderer mechanism + loose-list shape), `docs/architecture/{README,mvp-scope,source-of-truth-table,module-map→docs/implementation/module-map.md,rough-schema if touched}`, `README.md`, `CLAUDE.md` (module-split list — on-disk only, untracked), `docs/Developer_Guide.md`, `docs/index.md`, `CHANGELOG.md`, `docs/project/{status.md,phase-state.yaml}`
- Content requirements: DCR-0017 records the split, the neutral signatures + full migration list (every intentional break: `regenerate`/`build_alignment_map` signatures, widened items with doc(hidden) inventory for OI-0027, htmlseg module path change for core-internal callers, refdefs de-linked docs), the renderer rework + accepted shape changes (source-driven loose lists, both panes), Guard 1 defense-in-depth framing + residuals, the wasm gate lines + hook guard posture + canary result, and the explicit dialect-trait NON-instantiation (DCR-0005 deferral condition unmet). OI-0008 narrowing names the two cleared items and keeps four (parser-walker moved-not-refactored noted). status.md road-to-0.2.0: split marked landed, next = OI-0027.
- Gates: `cargo test -p transync --test docs_index_drift -- --test-threads=4` (2 tests) + full workspace once.
- Commit — `docs: DCR-0017 + amendments — transync-syntax split, AST-direct renderer, OI-0028 resolved`.

---

### Task 9: Full gate + conformance sweep

- [ ] Verification stack in order: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings`; `cargo clippy --all-targets --all-features -- -D warnings`; `cargo test --workspace -- --test-threads=4`; `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`; `cargo check -p transync-syntax --target wasm32-unknown-unknown`; `./scripts/test-browser.sh && ./scripts/test-browser.sh` (8/8 twice); `bash scripts/smoke.sh` (the hard gate now contains the wasm line).
- [ ] Conformance sweep: every spec §2–§7 requirement mapped to a commit/test (spec §6's new-test list each named to a real test); `grep -rn "use crate::validate\|use crate::unit\|use crate::pipeline" crates/transync-syntax/src` is EMPTY (no upward edges); `grep -rn "dev-dependencies" crates/transync-syntax/Cargo.toml` EMPTY; `git status` clean except known untracked.
- [ ] Report: commits, gate results, migration list completeness, deviations.

---

## Execution notes

- Tasks strictly ordered; every boundary green (module re-exports are what make the intermediate states compile).
- If a step's expected state disagrees with reality, READ the file and adapt mechanics — the spec's behavior contract is the invariant, not this plan's line guesses; record every adaptation.
- Fable-model subagents: max 2 concurrent (owner policy). Implementation subagents: model `opus` (owner memory).
