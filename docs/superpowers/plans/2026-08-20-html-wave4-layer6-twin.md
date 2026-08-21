# HTML→HTML Wave 4 — The Layer-6 Twin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An HTML translation run can be **trusted to fail loudly**. `validate::full_rescan_html` — the layer-6 twin of `reparse_full` — lands with its four checks (ordered tag ledger, fresh segmentation, gap byte-identity, boundary sanity), `finalize_regen_with_reparse_policy` gains **the one format branch** that dispatches on `Document.format`, and the three-stage fallback cascade is provably untouched. No HTML translation run becomes reachable: that is wave 5's entry point and wave 6's flag, and this plan's acceptance re-checks it mechanically.

**Architecture:** One new module, `crates/transync-core/src/validate/full_rescan_html.rs`, sibling of `full_reparse.rs`, returning the **same** `ReparseFailure { reason, divergent_source_blocks }` — which is the whole reason the cascade does not move: `downgrade_units` consumes only the id list, `widen_to_neighbors` consumes the id list plus `regen::top_level_blocks` (which is literally `blocks.iter()` — format-blind), and `fall_back_all` consumes neither field. The twin re-reads the **output**: check 1 compares `transync_html::tag_inventory` over the whole regenerated document against the whole source (ordered `Vec`, so dropped/duplicated/reordered markup anywhere fires, gaps included); check 2 re-runs the intake segmenter — `transync_syntax::intake::html::parse`, wave 3's seam — over the regenerated document and compares the block count and `wire_str()` label sequence against `doc.blocks` in order, written to **D9's per-`<li>` ruling** with attribution that mirrors `attribute_offenders` (fresh-block start offsets mapped into the regen `BlockOffsets`; a source block owning ≠ 1 fresh block is the offender, per block, never per group); check 3 memcmps every inter-block gap **including the preamble and the tail** — the check with no Markdown twin, existing precisely to close the ledger's two MEASURED blind spots (`<!DOCTYPE>` and comments produce no **ledger entry** — since wave 0's bogus-comment state they scan as `Skip` tokens, which `tag_inventory` filters — so a dropped doctype or comment is ledger-invisible but is a gap byte); check 4 re-verifies regen's bookkeeping (every block has a target range; ranges in-bounds against the **actual output length**, on char boundaries, monotone, non-overlapping). The checks interlock: a corruption that dodges the ledger (tagless bytes) either moves a block count (check 2) or moves gap bytes (check 3), and a corruption that would make a gap unreadable is exactly a range fault (check 4). The cascade's terminal rung stays sound for HTML **because of wave 3's identity theorem**: `regenerate(doc, &empty)` is byte-identical to the source, so a full-fallback regen passes the twin trivially — `fall_back_all`'s "structurally identical by construction" claim extends to HTML with no new code.

**Tech Stack:** Rust 2024 workspace (rustc ≥ 1.88), `transync-html` (wave 0: `tag_inventory`, `scan_tags`, `TagToken::{Open, Close, Skip}` with spans), the wave-2 IR (`Spelling`, `SourceFormat`, `Block.spelling`, `Document.format`), the wave-3 intake (`transync_syntax::intake::html::parse`), `wasm32-unknown-unknown` check gate (untouched by this wave but run by the hook), tracked pre-commit hook (fmt + clippy + wasm gate + rustdoc gate).

**Spec:** docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md — §7 is this wave in full; §12's "Wave 4" entry names the deliverable and acceptance; §13 item 7 records the accepted blind spots.

**Depends on:** **wave 0, complete** (`docs/superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md` — this plan consumes `pub fn tag_inventory(html: &str) -> Vec<String>` and `pub fn scan_tags(html: &str) -> Vec<TagToken>` with `TagToken::{Open { name, self_closing, span }, Close { name, span }, Skip { span }}`), **wave 2, complete** (`docs/superpowers/plans/2026-08-20-html-wave2-ir-split.md` — `Document.format: SourceFormat`, `Block.spelling: Spelling`, the re-keyed regen splice arm that decodes an Html-spelled block's accepted payload as a JSON segment array), and **wave 3, complete** (`docs/superpowers/plans/2026-08-20-html-wave3-intake-round-trip.md` — `pub fn parse(source: &str) -> Document` in `transync-syntax::intake::html`, **infallible** per its deviation 2, plus the SCN-16 fixture and the identity theorem). Task 1 Step 2 is a hard gate on all three. See "The §12 parallelism ruling" below for why wave 3 is a dependency here although the spec calls wave 4 "parallel with 3".

**What this wave must NOT do:** make an HTML translation run reachable. Spec §7/§12's hard rule — *no HTML translation run ships before the twin exists* — cuts both ways: the twin must land, and nothing else may move. After this wave, `translate()` / `translate_with_cache()` still take `source: &str` and `run_pipeline` still builds its document through exactly one constructor, `let mut doc = parse(source)?` (`crate::parser::parse`, the Markdown intake, which stamps `format: SourceFormat::Markdown`); `TranslateOptions` has no input-format field (wave 5/6), the CLI has no `--input-format` flag (wave 6), and no public entry point accepts a caller-built `Document`. The dispatcher's Html arm is therefore reachable only from this crate's own tests until wave 5 — and the acceptance section checks that mechanically, not by assertion. **This plan touches no file in `transync-syntax`, `transync-html`, `transync-cli`, `transync-wasm`, or `web/` at all**, and inside `transync-core` it touches exactly two source files (`validate.rs` gains a `pub mod` line and one doc sentence; `pipeline/finalize.rs` gains the dispatcher and tests) plus one new module. `pipeline.rs`, `report.rs`, `full_reparse.rs` and `walk.rs` are **byte-untouched**, and the acceptance diff proves it.

---

## The §12 parallelism ruling (read before Task 1)

Spec §12 says wave 4 "runs parallel with 3" and is "developable against hand-built `Document`s before intake is complete." Spec §7's check 2 says the twin **re-runs the intake segmenter over the regenerated document** — wave 3's `intake::html::parse`. Both cannot be fully true: checks 1, 3 and 4 are developable against hand-built documents, but check 2 *calls the intake*, and a twin without check 2 is not the gate §7 specifies.

**This plan takes reading (a): the waves run sequentially in this execution — 0 → 2 → 3 → 4 — and wave 4 calls `intake::html::parse` directly.** Reasons, in order of weight:

1. **The alternative is the shape this project already rejected.** Reading (b) — a segmenter seam (function parameter or trait) so `full_rescan_html` is testable against a stub — creates a seam with exactly one real implementation, whose stub-driven tests would prove properties of the stub, not of the gate. Wave 3's deviation 2 rejected precisely this shape for the intake's own `Result` ("an unreachable `Err` variant would be untested code claiming to be a gate"), and the architecture's one-HTML-opinion rule (spec §5: `element_extents` "never a second copy … a second opinion about HTML structure, the sin the architecture forbids") applies with full force to a stub segmenter: check 2's entire value is that the *same* segmenter reads both sides.
2. **The parallelism was a scheduling affordance, not a design requirement.** §12's dependency graph puts both 3 and 4 after 2; "parallel with 3" said they *may* overlap, not that they must. In this execution wave 3 lands first, so the affordance costs nothing to decline. Wave 3's own plan anticipated the race and reduced the overlap to one shared artifact — the DCR number: wave 3's Task 6 Step 1 carries a renumber rule, while this plan pins 0036 unconditionally and STOPs to reconcile on a collision (Task 5 Step 1 here) — reservations that stay consistent in every landing order.
3. **The honest residue is recorded, not smoothed over.** §12's "developable against hand-built `Document`s before intake is complete" remains true for checks 1, 3 and 4 and for the finalize branch; it is false for check 2. That partial truth is a spec amendment owed: **Task 5's DCR-0036 records it as a §14-style post-implementation item** (amend §12's wave-4 entry to "parallel with 3 except check 2, which consumes the intake; in the executed ordering wave 4 followed wave 3"), following wave 3's deviation-5 pattern — the spec file itself is **not** edited in this wave.

Consequences threaded through the plan: Task 1's precondition gate includes wave 3; the twin's tests build source documents through the real intake (`intake::html::parse`) rather than hand-assembled `Block` literals wherever a real document serves, hand-corrupting the *regenerated* side instead — the same house style `full_reparse.rs`'s own tests use (`bad_regen` + hand-built offsets).

One consequence of wave 3's plan that binds here: its Notes section says **"Do not add `intake` re-exports to `transync-core` or the facade — wave 5 decides how core reaches the intake."** This plan complies: `full_rescan_html.rs` names `transync_syntax::intake::html::parse` **directly at its one call site**, exactly as `full_reparse.rs` already names `transync_syntax::walk` directly rather than through a crate-private re-export. No `lib.rs` edit, no new `pub(crate) use`.

---

## Global Constraints

- **Acceptance gate for the whole wave:** the full workspace suite green with **zero fixture or expectation edits**. Every pre-existing test — `full_reparse.rs`'s ~15, `finalize.rs`'s `reparse_policy_tests` and `list_item_count_pipeline_tests`, the SCN corpus — passes with **zero edits**; that is half of the "Markdown runs still take `reparse_full`" proof (Task 4 carries the other, positive half).
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`. This wave never touches either package, but the pre-commit hook runs the gate on every commit; it must stay green.
- `transync-syntax` gains **no `[features]` table and no `transync-core` dependency** — trivially satisfied here (no `transync-syntax` file and no `Cargo.toml` is touched; verify in the acceptance diff), stated because it is the standing charter.
- Every test run: `cargo test -p <crate> -- --test-threads=4`; workspace runs: `cargo test --workspace -- --test-threads=4`. **Never raise the cap.**
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append `echo "CARGO_EXIT=$?"`, and inspect the file as a **separate** step. A pipeline reports the last stage's status, so a failing suite reads as a pass, and `tail -N` over filtered lines drops early failures.
- **If a number is offered as evidence, capture it to a file.** Test counts, grep counts, exit codes, diff stats — every number a task's expected-result line names must exist in a file under the temp dir, not only in the transcript.
- Lint gates (the pre-commit hook enforces them): `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`. Run `cargo fmt --all` **before** committing, so the hook's fmt check is a confirmation, not a discovery.
- **`git commit --no-verify` is never used.** A commit step's expected result is that the hook runs fmt, clippy, the wasm gate and the rustdoc gate, and all four pass. If the hook blocks, fix the cause.
- **Stage exact paths only** — never `git add -A`, never `git add <directory>`. Every commit step below names its files.
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave4/` — never `/tmp`, never `/private/tmp`, never `$TMPDIR`, never the OS default. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user**.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`. If a cargo command fails because the target dir is unreachable, stop and ask.
- **Every workspace enum stays exhaustively matched: no `_ =>` catch-all arm and no `matches!(x, Variant)`** over `SourceFormat`, `Spelling`, `TagToken`, `BlockKind` or any other workspace enum in code this wave writes — `matches!` expands to a match with an implicit `_ => false`, so a variant added later silently takes the `false` branch instead of stopping the compiler (wave 0's constraint, restated because this wave writes the `SourceFormat` dispatch match and a `TagToken` filter, and both must name every arm). The one sanctioned narrow use: matching non-workspace types (`char`, `u8`, std types) is not covered.
- **`grep -c` exits non-zero on a zero count** — the precondition and gate greps below read the *printed count*, not the exit code; do not run them under `set -e`, and a count **above 1 is fine** (a doc comment naming a symbol beside its definition is not a defect — absence is).
- **`docs/index.md` is NOT touched by this plan — the controller owns that file this wave.** The plan file's own link is the controller's atomic move-and-link step; DCR-0036's link line is handed to the controller in Task 5 (with its exact text), never added here. Between the DCR file's creation and the controller's link, `docs_index_drift` is expected red naming DCR-0036's file — **plus, possibly, this plan's own file** while the controller's move-and-link is still pending (the same two-name allowance Task 5 Step 7 and the Notes state); any name outside those two is real drift and a STOP.
- No pure-formatting edits. Where this plan shows wrapped Rust, run `cargo fmt --all` afterwards and take the formatter's answer.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, cite, or create them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `crates/transync-core/src/validate/full_rescan_html.rs` | **create** | THE layer-6 twin: `full_rescan_html(source_doc, regenerated, offsets) -> Result<(), ReparseFailure>`, four checks in spec §7's order, attribution helpers, the three §7 residuals as module-doc text, and the module tests (spec §11 row 3 names `crates/transync-core/src/validate/` module tests as their location) |
| `crates/transync-core/src/validate.rs` | modify | one `pub mod full_rescan_html;` line beside `pub mod full_reparse;`, and the orchestrator module-doc sentence gains the format dispatch |
| `crates/transync-core/src/pipeline/finalize.rs` | modify | the private `layer6_gate` dispatcher (THE one format branch), the three `reparse_full` call sites rerouted through it, and the new dispatch/cascade tests. **`downgrade_units`, `widen_to_neighbors`, `fall_back_all`, `regen_pass`, `collect_validated` and every existing test: byte-untouched** |
| `docs/architecture/contracts.md` | modify (Task 5) | §5a "Unit validity" item 2 gains the format-dispatch sentences (spec §10 assigns "§5a — the layer-6 format dispatch" to the wave that makes it true; the cache no-axis paragraph is **wave 5's**, not this wave's) |
| `docs/implementation/module-map.md` | modify (Task 5) | the `validate/` tree gains the `full_rescan_html.rs` row; "`validate` (+ its five layers)" becomes six |
| `docs/architecture/source-of-truth-table.md` | modify (Task 5) | the `transync-core::validate` row's Notes name both layer-6 gates and the dispatch (`docs_ownership_drift` welds only the crate-root `lib.rs` module lists — three roots today: `transync-syntax`, `transync-core`, `transync-html` — and this wave adds no crate-root module — the row edit is honesty, not weld-forced) |
| `docs/project/design-change-records/DCR-0036-html-layer6-twin.md` | **create** (Task 5) | the wave's record, including the §12 parallelism amendment recorded as owed |
| `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml` | modify (Task 5) | routine per-wave records |
| `CLAUDE.md` | modify (Task 5) | the `transync-core` module-split list's `validate` row gains the dispatch clause |

**Not touched, deliberately:** `crates/transync-core/src/validate/full_reparse.rs` (**byte-untouched** — the strongest form of "ReparseFailure's shape is unchanged"; the twin imports the type, it does not redefine or modify it); `crates/transync-core/src/pipeline.rs` (the Hard-arm `"full reparse failed: {reason}"` mapping and the eviction loop consume the same `ReparseFailure` either gate returns — unchanged); `crates/transync-core/src/pipeline/report.rs` (verified during planning: it imports no `ReparseFailure` at all — its `full_reparse_fallbacks` field is computed in `pipeline.rs` from `FallbackStatus` alone); `transync_syntax::walk` (spec §7: "it is the *Markdown* layer-6/renderer pairing and must simply never be called on an HTML document — enforced by the format branch in `finalize`, not by adding arms"; the twin never imports it, gated in Task 2); every `Cargo.toml` and `Cargo.lock` (`transync-core` already depends on `transync-html` — `validate.rs`'s layer-3 check calls `transync_html::tag_inventory` today — and on `transync-syntax`); every fixture (this wave creates **no** fixture: its tests use inline documents plus the wave-3 SCN-16 fixture via a relative `include_str!`, the same sharing mechanism wave 3 established); `docs/index.md` (controller-owned, see Global Constraints); `docs/architecture/scenario-matrix.md` (SCN-16's row is wave 7's).

---

## Deviations from the spec

Four, declared here and nowhere else.

1. **The §12 parallelism claim is resolved to sequential execution (reading (a)), and the spec amendment is recorded as owed.** Full argument above ("The §12 parallelism ruling"). Check 2 consumes `intake::html::parse`; a segmenter seam with one real implementation is the "untested code claiming to be a gate" shape wave 3's deviation 2 rejected, and a stub segmenter would be a second HTML opinion. DCR-0036 (Task 5) carries the owed amendment to §12's wave-4 entry; the spec file is not edited in this wave.
2. **Check 3 defers an unreadable gap to check 4 instead of failing on it.** Spec §7 lists the four checks in order and this plan keeps that order — but a gap whose *target* endpoints cannot be sliced (missing offsets entry, inverted range, out of bounds, non-char-boundary) is a **bookkeeping** fault, which is check 4's vocabulary, not a byte-inequality, which is check 3's. So check 3 compares every gap it can read and returns the ordinals of the ones it could not; check 4's systematic predicates then fail on the range fault that made each unreadable. The deferral is **total by construction** — every cause of an unreadable gap is one of check 4's predicates (proved in the code comment at the deferral site) — and belt-and-braces: check 4's last step hard-errors if any deferred ordinal survived its predicates, so no gap can fall between the two checks. Without this split, any incoherent offsets map would surface as a misattributed "gap differs" failure and a pure boundary red would be unreachable.
3. **Attribution for checks 1, 3 and 4 is plan-defined.** Spec §7 specifies attribution only for check 2 ("mirrors `attribute_offenders`"). `ReparseFailure.divergent_source_blocks` seeds the cascade's stage 1, so an empty list costs two futile regen+rescan rounds before stage 3; this plan therefore attributes all four checks — check 1 maps the first-divergence token's byte span to its owning or flanking block(s); check 3 names the divergent gap's flanking blocks (preamble → the first block, tail → the last); check 4 names the offending block (plus its predecessor on an order/overlap fault). All deterministic, all falling back to an empty list only when genuinely unmappable — which the cascade already handles (stage 1 no-ops, stages 2–3 still terminate in honest fallback, never corrupt output; invariant 6).
4. **No `docs/index.md` step exists anywhere in this plan** — a declared deviation from the wave-2/-3 house form, whose Task 1 verified the plan link and whose records task added the DCR link. The controller owns `docs/index.md` this wave: it moves this plan to `docs/superpowers/plans/2026-08-20-html-wave4-layer6-twin.md` and links it atomically, and Task 5 Step 7 hands it DCR-0036's exact link line instead of editing the file. The expected weld state between DCR creation and the controller's link — `docs_index_drift` red naming exactly `DCR-0036-html-layer6-twin.md` — is written into Task 5, so nobody "fixes" it by hand.

---

### Task 1: Preconditions, baseline, and the recorded green "before"

**Files:**
- Test: none. This task's product is a recorded green "before" and three verified precondition waves.

**Interfaces:**
- Consumes from wave 0: `transync_html::{tag_inventory, scan_tags, TagToken}` — `pub fn tag_inventory(html: &str) -> Vec<String>` (Open → `"name"`, Close → `"/name"`, `Skip` filtered out), `pub fn scan_tags(html: &str) -> Vec<TagToken>` with `Open { name: String, self_closing: bool, span: (usize, usize) }`, `Close { name: String, span: (usize, usize) }`, `Skip { span: (usize, usize) }`.
- Consumes from wave 2: `transync_syntax::id::SourceFormat` (`Markdown | Html`, `Copy`, `#[default] Markdown`), `Document.format: SourceFormat`, `Block.spelling: Spelling`, and regen's re-keyed splice arm (`Spelling::Html { block_type }` + translated ⇒ JSON-decode the payload as `Vec<String>` and `transync_html::splice` it; on decode/splice failure, source bytes).
- Consumes from wave 3: `transync_syntax::intake::html`'s `pub fn parse(source: &str) -> Document` (**infallible**, deviation 2 of that plan), the SCN-16 fixture at `crates/transync/tests/fixtures/scn-16-html-document.html`, and the identity theorem (`regenerate(parse(fixture), &HashMap::new())` byte-identical).
- Produces: `/Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt`, the commit the acceptance section diffs against.

- [ ] **Step 1: Create the wave's temp directory.**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave4/gate
```
Expected: no output, exit 0. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user** — do not fall back to `/tmp`.

- [ ] **Step 2: Hard precondition gate — waves 0, 2 AND 3 must be COMPLETE.** Every code block below constructs `Document`s through `intake::html::parse` and relies on wave 2's field shapes; written against a partial tree, the whole plan is wrong.
```bash
grep -c 'pub fn tag_inventory' crates/transync-html/src/lib.rs
grep -c 'pub fn scan_tags' crates/transync-html/src/lib.rs
grep -c 'Skip { span' crates/transync-html/src/lib.rs
ls docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md
grep -c 'pub enum Spelling' crates/transync-syntax/src/id.rs
grep -c 'pub enum SourceFormat' crates/transync-syntax/src/id.rs
grep -c 'pub spelling: Spelling' crates/transync-syntax/src/parser.rs
grep -c 'pub format: SourceFormat' crates/transync-syntax/src/parser.rs
grep -c '0.5.0-dev' Cargo.toml
ls docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md
grep -c 'pub fn parse' crates/transync-syntax/src/intake/html.rs
ls crates/transync/tests/fixtures/scn-16-html-document.html
ls docs/project/design-change-records/DCR-0035-html-intake-identity-round-trip.md
grep -c 'transync-html' crates/transync-core/Cargo.toml
```
Expected: every count **≥ 1** and every `ls` path echoed. **Any `0`, or any missing file, means a prerequisite wave is unfinished — STOP.** Reading notes: `grep -c` exits non-zero on a zero count, so do not run these under `set -e`; counts above 1 are fine. The last grep matters because the twin calls `transync_html::tag_inventory` from `transync-core` — the dependency edge is wave 0's repoint and must already exist (today `validate.rs`'s layer-3 check uses it).

- [ ] **Step 3: Capture the baseline, bare-to-file — including wave 3's identity suite, which the cascade's terminal-rung argument rests on.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-cli.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-wasm.txt
cargo test -p transync-syntax --test html_intake_identity -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-identity.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-identity.txt
```
Then, as a **separate** step, read the last line of each file. Expected: `CARGO_EXIT=0` in all four and no line containing `FAILED`. If any is non-zero, STOP — the tree was not green before this wave. One tolerated exception: if `docs_index_drift` is the sole red and its file list names **only** this plan's own file, the controller's atomic move-and-link has not completed — hand back to the controller and wait; do not edit `docs/index.md`.

- [ ] **Step 4: Record the baseline commit.**
```bash
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt
cat /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt
```
Expected: one 40-character SHA. Every "untouched" claim in the acceptance section diffs against this commit.

---

### Task 2: The twin's spine — the module, the signature, checks 1 + 2, and the staged reds

**Files:**
- Create: `crates/transync-core/src/validate/full_rescan_html.rs`
- Modify: `crates/transync-core/src/validate.rs`

**Interfaces:**
- Produces, for Task 3, Task 4 and wave 5:
  ```rust
  // crates/transync-core/src/validate/full_rescan_html.rs
  pub fn full_rescan_html(
      source_doc: &Document,
      regenerated: &str,
      offsets: &BlockOffsets,
  ) -> Result<(), ReparseFailure>;
  ```
  The `ReparseFailure` is `crate::validate::full_reparse::ReparseFailure` — **imported, never redefined**: `pub struct ReparseFailure { pub reason: String, pub divergent_source_blocks: Vec<BlockId> }` (read from `full_reparse.rs` during planning; that file is byte-untouched this wave).
- Consumes: `crate::parser::Document` (fields used: `source_text`, `blocks`, `format`), `crate::regen::BlockOffsets` (`pub struct BlockOffsets(pub HashMap<BlockId, ByteRange>)`), `crate::align::ByteRange` (`{ pub start: usize, pub end: usize }`, `Copy`, half-open), `crate::id::{BlockId, SourceFormat}`, `BlockKind::wire_str()`, `transync_html::{tag_inventory, scan_tags, TagToken}`, and — directly, per wave 3's no-re-export rule — `transync_syntax::intake::html::parse`.
- **Reason-string vocabulary is a contract of this task:** every twin reason begins with one of four markers — `"tag inventory"`, `"fresh segmentation"`, `"gap "`, `"boundary"` — and **never** contains `reparse_full`'s phrases (`"regenerated block count"`, `"source kind"` appears only inside the `"fresh segmentation: …"` composite, `"list at block"` never). Task 4's routing proofs and the pipeline's Hard-arm logs distinguish the two gates by these strings.

- [ ] **Step 1: Declare the module and write the file with its doc, imports, and tests — NO implementation yet.** In `crates/transync-core/src/validate.rs`, immediately after `pub mod full_reparse;` add:
```rust
pub mod full_rescan_html;
```
Create `crates/transync-core/src/validate/full_rescan_html.rs` with the module doc, the imports, and the test module below (the helpers and tests are complete; the functions they call do not exist yet — that is Step 2's red). One spelling in the doc's first line is deliberate: the `full_reparse` link is written `[`full_reparse`](super::full_reparse)` because this module imports only `ReparseFailure`, not the sibling module, so a bare `[`full_reparse`]` would be a dead intra-doc link — unlinted today only because `validate` is `pub(crate)` and the rustdoc gate documents public items, and dead in anyone's face the day that changes. Do not "simplify" it back. (`validate.rs`'s own module doc in Step 7 keeps the bare form — there `full_reparse` is a child module, in scope, and the link resolves.)
```rust
//! Full-document rescan: the HTML layer-6 twin of [`full_reparse`](super::full_reparse).
//!
//! `reparse_full` re-reads a regenerated *Markdown* document under comrak and
//! compares its top-level shape to the source IR. This module does the same
//! job for a regenerated *HTML* document — with the scanner, never comrak:
//! comrak over an HTML document produces *some* node sequence and can pass
//! while checking nothing, which is why spec §7/§12 make this gate a hard
//! precondition for any HTML translation run.
//!
//! Four checks, in spec §7's order:
//!
//! 1. **Document-wide ordered tag ledger** — `tag_inventory(regenerated)`
//!    must equal `tag_inventory(source_text)`. The ledger is an ordered
//!    `Vec`, so dropped/duplicated/reordered markup anywhere fires — inside
//!    blocks and in gap bytes alike (pass-through wrappers live in gaps, and
//!    the ledger still tokenizes them).
//! 2. **Fresh segmentation** — re-run the intake segmenter
//!    (`transync_syntax::intake::html::parse`) over the regenerated document
//!    and compare block count and kind-label sequence against
//!    `source_doc.blocks` in order. Written to D9's ruling: `<li>` is the
//!    block, so both sides carry per-item entries and attribution is
//!    per-`<li>`, never per-list (the validation area's original whole-`<ul>`
//!    projection is superseded — spec §7). Attribution mirrors
//!    `full_reparse::attribute_offenders`: each fresh block's start offset is
//!    mapped into the regen `BlockOffsets`; a source block owning ≠ 1 fresh
//!    block is the offender.
//! 3. **Gap byte-identity** — every inter-block gap, the preamble and the
//!    tail, compared byte-for-byte. This is the check with no Markdown twin.
//!    It exists precisely to close the ledger's two MEASURED blind spots:
//!    `<!DOCTYPE>` and comments produce no ledger entry — `scan_tags`
//!    yields only a `Skip` for them, and `tag_inventory` filters `Skip` —
//!    so a dropped doctype or comment is ledger-invisible; but it is a gap
//!    byte, and gaps must be verbatim. In HTML, gaps carry meaning (head, scripts,
//!    structural wrappers); this check re-reads them from the *output*,
//!    which is the whole point of a layer-6 gate.
//! 4. **Boundary sanity** — every block has a target range; ranges are
//!    in-bounds against the actual output length, on char boundaries,
//!    monotone and non-overlapping: cheap re-verification of regen's
//!    bookkeeping. Check 3 defers an unreadable gap here (the fault is the
//!    range, not the bytes), and this check's last step proves the deferral
//!    was total.
//!
//! The checks interlock: a corruption the ledger cannot see (tagless bytes)
//! either changes the block set (check 2) or changes gap bytes (check 3),
//! and a corruption that would make a gap unreadable is a range fault
//! (check 4).
//!
//! # Honestly recorded residuals (spec §7; accepted limitation §13 item 7)
//!
//! - **Two same-kind, identical-tag-skeleton blocks whose texts were swapped
//!   by an engine fault pass all four checks.** This is exact parity with
//!   the shipped `reparse_full`'s blindness to two swapped paragraphs —
//!   parity, not regression. Per-unit layer 3 already ledger-checks each
//!   accepted splice individually, which bounds the fault surface to regen's
//!   assembly ordering; checks 3 + 4 jointly cover any block whose neighbors
//!   differ. Pinned by `swapped_same_skeleton_texts_pass_by_documented_parity`.
//! - **Attribute values are not tokenized** (`scan_tags` skips attribute
//!   internals; RCDATA/raw-text contents are never tokenized). Compensating
//!   controls: splice never edits inside tags by construction,
//!   translated-text escaping is pinned by test in `transync-html`, and the
//!   SCN-16 acceptance criterion ("untouched markup byte-identical outside
//!   text nodes") tests it end-to-end.
//! - **`scan_tags` is not a general HTML parser** — fine, because both sides
//!   of every comparison use the *same* scanner: the check is
//!   self-consistency of one tokenizer opinion, the same rule the
//!   architecture enforces for comrak.
//!
//! On rejection, returns the SAME [`ReparseFailure`] the Markdown gate
//! returns — that shared shape is what keeps the three-stage fallback
//! cascade in `pipeline::finalize` format-blind and untouched.
//!
//! TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)
//! TRACE: DCR-0036

use crate::align::ByteRange;
use crate::id::{BlockId, SourceFormat};
use crate::parser::Document;
use crate::regen::BlockOffsets;
use crate::validate::full_reparse::ReparseFailure;
use transync_html::{TagToken, scan_tags, tag_inventory};
```
and then, at the bottom of the same file, the test module. Its helpers are the file's whole test vocabulary; write them exactly:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// The one segmenter, both sides (wave 3's seam; see the module doc for
    /// why core names it directly rather than through a re-export).
    fn parse_html(src: &str) -> Document {
        transync_syntax::intake::html::parse(src)
    }

    /// Identity regen: byte-identical output + one target range per block
    /// (wave 3's theorem, re-asserted here because every corruption helper
    /// below starts from it).
    fn identity(doc: &Document) -> (String, BlockOffsets) {
        let (out, offsets) = crate::regen::regenerate(doc, &HashMap::new());
        assert_eq!(out, doc.source_text, "wave 3 identity theorem");
        assert_eq!(offsets.0.len(), doc.blocks.len());
        (out, offsets)
    }

    /// Identity output with `cut` deleted, offsets re-derived: a range fully
    /// before the cut is unchanged; fully after shifts left by the cut's
    /// length; a range containing the cut shrinks; a range inside the cut
    /// collapses to an empty range at the cut point — the "dropped block"
    /// shape `full_reparse`'s dropped-html test hand-built.
    fn identity_minus(doc: &Document, cut: std::ops::Range<usize>) -> (String, BlockOffsets) {
        let (out, offsets) = identity(doc);
        let mut cropped = String::new();
        cropped.push_str(&out[..cut.start]);
        cropped.push_str(&out[cut.end..]);
        let len = cut.end - cut.start;
        let map = |p: usize| -> usize {
            if p <= cut.start {
                p
            } else if p >= cut.end {
                p - len
            } else {
                cut.start
            }
        };
        let mut shifted = BlockOffsets::default();
        for (id, r) in offsets.0 {
            shifted.0.insert(
                id,
                ByteRange {
                    start: map(r.start),
                    end: map(r.end),
                },
            );
        }
        (cropped, shifted)
    }

    /// Identity output with `insert` spliced in at `at` (a gap or in-block
    /// position), offsets re-derived by shifting every boundary at or after
    /// `at` right by the insertion's length.
    fn identity_plus(doc: &Document, at: usize, insert: &str) -> (String, BlockOffsets) {
        let (out, offsets) = identity(doc);
        let mut grown = String::new();
        grown.push_str(&out[..at]);
        grown.push_str(insert);
        grown.push_str(&out[at..]);
        let map = |p: usize| -> usize { if p < at { p } else { p + insert.len() } };
        let mut shifted = BlockOffsets::default();
        for (id, r) in offsets.0 {
            shifted.0.insert(
                id,
                ByteRange {
                    start: map(r.start),
                    end: map(r.end),
                },
            );
        }
        (grown, shifted)
    }

    /// Identity output with two non-overlapping regions (`a` before `b`)
    /// swapped, offsets remapped positionally: the block whose source range
    /// IS `a` follows its bytes to `b`'s old position and vice versa; blocks
    /// between the two shift by the length difference; blocks outside are
    /// unchanged (the total length is). Panics if any block range straddles
    /// a swapped region — the fixtures below are chosen so none does.
    fn swap_regions(doc: &Document, a: ByteRange, b: ByteRange) -> (String, BlockOffsets) {
        assert!(a.end <= b.start, "a must precede b");
        let src = &doc.source_text;
        let mut out = String::new();
        out.push_str(&src[..a.start]);
        let b_new = ByteRange {
            start: out.len(),
            end: out.len() + (b.end - b.start),
        };
        out.push_str(&src[b.start..b.end]);
        out.push_str(&src[a.end..b.start]);
        let a_new = ByteRange {
            start: out.len(),
            end: out.len() + (a.end - a.start),
        };
        out.push_str(&src[a.start..a.end]);
        out.push_str(&src[b.end..]);
        assert_eq!(out.len(), src.len());
        let delta = (b.end - b.start) as isize - (a.end - a.start) as isize;
        let mut offsets = BlockOffsets::default();
        for blk in &doc.blocks {
            let r = blk.source_range;
            let mapped = if r == a {
                a_new
            } else if r == b {
                b_new
            } else if r.end <= a.start || r.start >= b.end {
                r
            } else if r.start >= a.end && r.end <= b.start {
                ByteRange {
                    start: (r.start as isize + delta) as usize,
                    end: (r.end as isize + delta) as usize,
                }
            } else {
                panic!("swap_regions: block range straddles a swapped region");
            };
            offsets.0.insert(blk.block_id.clone(), mapped);
        }
        (out, offsets)
    }

    /// Small inline fixture. Blocks, in order (derived by hand from spec
    /// §4's classification table): `<p>alpha</p>` → Paragraph; the naked
    /// `intro text` run → Paragraph (rule T); two `<li>` → ListItem ×2 (D9).
    /// The `<div>`/`<ul>` markup and the comment are gap bytes; the comment
    /// is a `Skip` span and produces NO ledger token — which is what the
    /// dropped-comment test in Task 3 leans on.
    const SRC_SMALL: &str = "<div>\n<p>alpha</p>\nintro text\n<ul>\n<li>one</li>\n<li>two</li>\n</ul>\n<!-- note -->\n</div>\n";

    /// The wave-3 fixture, shared by relative include exactly as wave 3's
    /// own syntax-side tests share it — one file, two consumers, no drift.
    const SCN_16: &str =
        include_str!("../../../transync/tests/fixtures/scn-16-html-document.html");

    fn kinds(doc: &Document) -> Vec<&'static str> {
        doc.blocks.iter().map(|b| b.kind.wire_str()).collect()
    }

    #[test]
    fn identity_regen_accepts_over_the_scn_16_fixture() {
        let doc = parse_html(SCN_16);
        assert_eq!(doc.format, SourceFormat::Html);
        assert_eq!(doc.blocks.len(), 14, "the wave-3 designed block set");
        let (out, offsets) = identity(&doc);
        full_rescan_html(&doc, &out, &offsets)
            .expect("byte-identical regen must pass all four checks");
    }

    #[test]
    fn a_dropped_element_block_is_rejected_by_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        assert_eq!(
            kinds(&doc),
            vec!["paragraph", "paragraph", "list-item", "list-item"],
            "fixture sanity: the designed block set",
        );
        // Cut the whole `<p>alpha</p>` element — its `p`/`/p` tokens leave
        // the ledger, so check 1 fires. (Its block also dissolves, so check
        // 2 WOULD fire too — spec §11's "trips checks 1+2"; check 1 is
        // first in order, so it reports. The check-2-only route is the
        // dissolved-run test below.)
        let p = doc.blocks[0].source_range;
        let (out, offsets) = identity_minus(&doc, p.start..p.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a regen that dropped an element block must be rejected");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        assert!(
            !err.divergent_source_blocks.is_empty(),
            "check 1 must seed the cascade",
        );
    }

    #[test]
    fn a_reordered_block_pair_is_rejected_by_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        // Swap the `<p>` element's bytes with the first `<li>` element's.
        // The ledger is ORDERED, so `p,/p,…,li,/li` vs `li,/li,…,p,/p`
        // diverges at the first swapped token (spec §11's reordered red:
        // both the ledger and the label sequence fire; the ledger is first).
        let (out, offsets) =
            swap_regions(&doc, doc.blocks[0].source_range, doc.blocks[2].source_range);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a regen that reordered two blocks must be rejected");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        let named: Vec<&str> = err
            .divergent_source_blocks
            .iter()
            .map(|id| id.0.as_str())
            .collect();
        assert!(
            named.contains(&doc.blocks[0].block_id.0.as_str())
                || named.contains(&doc.blocks[2].block_id.0.as_str()),
            "attribution must name a swapped block; got {named:?}",
        );
    }

    #[test]
    fn tag_inventory_drift_inside_a_block_is_rejected() {
        let doc = parse_html(SRC_SMALL);
        // Insert a phantom `<em>` pair inside the first `<li>`'s text — the
        // kind of markup a faulty splice could invent. Ledger-visible,
        // segmentation-invisible (an `<em>` is PHRASING and mints no block).
        let li = doc.blocks[2].source_range;
        let li_text = doc.source_text[li.start..li.end].to_string();
        assert!(li_text.contains("one"), "fixture sanity");
        let at = li.start + doc.source_text[li.start..li.end].find("one").unwrap();
        let (out, offsets) = identity_plus(&doc, at, "<em>");
        let (out, offsets) = {
            // close it after the word so the fragment stays balanced — the
            // drift must be caught by INVENTORY inequality, not by luck.
            let close_at = at + "<em>".len() + "one".len();
            let mut o2 = BlockOffsets::default();
            let map = |p: usize| -> usize { if p < close_at { p } else { p + "</em>".len() } };
            for (id, r) in offsets.0 {
                o2.0.insert(id, ByteRange { start: map(r.start), end: map(r.end) });
            }
            let mut s = String::new();
            s.push_str(&out[..close_at]);
            s.push_str("</em>");
            s.push_str(&out[close_at..]);
            (s, o2)
        };
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("invented markup must be rejected by the ledger");
        assert!(err.reason.contains("tag inventory"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[2].block_id.clone()],
            "the token span sits inside li-…'s target range, so attribution \
             must name exactly that block",
        );
    }

    #[test]
    fn a_dissolved_anonymous_run_is_rejected_by_fresh_segmentation_not_the_ledger() {
        let doc = parse_html(SRC_SMALL);
        // Cut ONLY the rule-T run's bytes (`intro text`) — no tag leaves
        // the ledger, but the block dissolves. With later blocks following,
        // the divergence surfaces at the LABEL arm: index 1 is `paragraph`
        // on the source side and `list-item` on the fresh side (everything
        // shifted up). This is the check-2-only red: with check 2 removed
        // the twin would accept a document that silently lost a paragraph.
        let run = doc.blocks[1].source_range;
        assert_eq!(&doc.source_text[run.start..run.end], "intro text", "fixture sanity");
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dissolved anonymous run must be rejected");
        assert!(
            !err.reason.contains("tag inventory"),
            "the ledger cannot see tagless bytes; got: {err}",
        );
        assert!(err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(err.reason.contains("paragraph"), "got: {err}");
        assert!(err.reason.contains("list-item"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[1].block_id.clone()],
            "the dissolved run's empty target range owns 0 fresh blocks — \
             per-block attribution (D9), never a group",
        );
    }

    #[test]
    fn a_dissolved_trailing_run_hits_the_count_arm() {
        // The count arm's own red: when the dissolved run is the LAST
        // block, the label prefix stays clean and only the count diverges —
        // "fresh segmentation found 1 block(s); source has 2".
        let doc = parse_html("<p>x</p>\n<div>tail note</div>\n");
        assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"], "fixture sanity");
        let run = doc.blocks[1].source_range;
        assert_eq!(&doc.source_text[run.start..run.end], "tail note", "fixture sanity");
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dissolved trailing run must be rejected");
        assert!(err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(err.reason.contains("found 1 block(s); source has 2"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[1].block_id.clone()],
            "got: {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn attribution_is_per_li_not_per_list() {
        // D9's ruling, pinned structurally: both sides of check 2 carry one
        // entry PER `<li>`, so the label sequence over SRC_SMALL ends
        // `…, "list-item", "list-item"` — never a collapsed `"list"` entry
        // like `walk::normalize_top_level` produces on the Markdown side.
        // The twin never imports `walk` (gated by grep in Step 6), so the
        // superseded whole-`<ul>` projection has no code to hide in.
        let doc = parse_html(SRC_SMALL);
        let (out, _offsets) = identity(&doc);
        let fresh = parse_html(&out);
        assert_eq!(kinds(&fresh), kinds(&doc));
        assert_eq!(kinds(&fresh)[2..4], ["list-item", "list-item"]);
    }

    #[test]
    fn swapped_same_skeleton_texts_pass_by_documented_parity() {
        // Residual 1 (module doc; spec §13 item 7): two same-kind,
        // identical-tag-skeleton blocks whose TEXTS were swapped pass all
        // four checks — exact parity with `reparse_full`'s blindness to two
        // swapped paragraphs. Characterized so the residual is a tested
        // fact, not a forgotten one: if a future check closes it, this pin
        // flips and the closure DCR retires it deliberately.
        let doc = parse_html("<p>alpha</p>\n<p>beta</p>\n");
        let (out, offsets) =
            swap_regions(&doc, doc.blocks[0].source_range, doc.blocks[1].source_range);
        // Positional re-map: block 0's range must describe the FIRST region
        // again (the engine-fault shape: right bytes, wrong owner).
        let mut positional = BlockOffsets::default();
        positional
            .0
            .insert(doc.blocks[0].block_id.clone(), *offsets.0.get(&doc.blocks[1].block_id).unwrap());
        positional
            .0
            .insert(doc.blocks[1].block_id.clone(), *offsets.0.get(&doc.blocks[0].block_id).unwrap());
        full_rescan_html(&doc, &out, &positional)
            .expect("parity with reparse_full: the documented blind spot accepts");
    }
}
```
Reading note for the implementer: `swap_regions` + the positional re-map in the parity pin together produce a *coherent* offsets map (monotone, tiling) whose block↔bytes pairing is wrong — that is the fault class the residual describes. Do not "fix" the test by attributing blocks to their followed bytes; the point is that no check can tell.

- [ ] **Step 2: Watch the compile red.**
```bash
cargo test -p transync-core full_rescan -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t2-red-compile.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t2-red-compile.txt
```
Expected: `CARGO_EXIT=101`, and the file contains `error[E0425]: cannot find function \`full_rescan_html\` in this scope` at each test call site (the module, doc and imports resolve; only the function is missing). This is the wiring red; the behavioural reds are Step 4's.

- [ ] **Step 3: Land the signature as an accept-everything stub.** Above the test module, add:
```rust
/// Rescan the regenerated HTML and verify it against the source IR.
///
/// `offsets` is the regenerator's per-source-block byte-range map, used the
/// same two ways `reparse_full` uses it: to attribute a divergence back to
/// the source block that owns the divergent output bytes, and (check 4) as
/// the bookkeeping under re-verification. See the module doc for the four
/// checks and the recorded residuals.
///
/// TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)
pub fn full_rescan_html(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    debug_assert_eq!(
        source_doc.format,
        SourceFormat::Html,
        "full_rescan_html is the HTML gate; Markdown documents take reparse_full — \
         the finalize dispatch is the enforcement, this assert is the tripwire",
    );
    let _ = (regenerated, offsets);
    Ok(())
}
```
- [ ] **Step 4: Watch the behavioural reds.** Re-run the Step 2 command into `t2-red-stub.txt`. Expected: `CARGO_EXIT=101`; `identity_regen_accepts_over_the_scn_16_fixture`, `attribution_is_per_li_not_per_list` and `swapped_same_skeleton_texts_pass_by_documented_parity` **pass** (the stub accepts everything — including, honestly, the pins that SHOULD accept); the five reject tests **fail**, each with `` called `Result::expect_err()` on an `Ok` value `` under its own message (e.g. `a regen that dropped an element block must be rejected`). Five named reds, zero mystery.

- [ ] **Step 5: Implement checks 1 and 2 and their attribution.** Replace the stub body (keep the `debug_assert_eq!`) with the check sequence — Task 3 adds checks 3 and 4 to the same sequence:
```rust
    check_tag_ledger(source_doc, regenerated, offsets)?;
    check_fresh_segmentation(source_doc, regenerated, offsets)?;
    Ok(())
```
and add, below the public function:
```rust
/// Check 1 (spec §7): the document-wide ordered tag ledger.
fn check_tag_ledger(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    let source_inv = tag_inventory(&source_doc.source_text);
    let regen_inv = tag_inventory(regenerated);
    if source_inv == regen_inv {
        return Ok(());
    }
    // First divergence over the aligned prefix; if one stream is a strict
    // prefix of the other, the divergence sits at the shorter length.
    let i = source_inv
        .iter()
        .zip(regen_inv.iter())
        .position(|(s, r)| s != r)
        .unwrap_or_else(|| source_inv.len().min(regen_inv.len()));
    Err(ReparseFailure {
        reason: format!(
            "tag inventory diverged at token {i}: source {}, regenerated {} \
             (source has {} tags, regenerated has {})",
            source_inv.get(i).map(String::as_str).unwrap_or("<end>"),
            regen_inv.get(i).map(String::as_str).unwrap_or("<end>"),
            source_inv.len(),
            regen_inv.len(),
        ),
        divergent_source_blocks: ledger_offenders(source_doc, regenerated, offsets, i),
    })
}

/// The byte span of the `i`-th NON-Skip token — index-aligned with
/// `tag_inventory`'s output, which filters `Skip` the same way. Exhaustive
/// match by charter: no `_ =>`, no `matches!`.
fn non_skip_span(tokens: &[TagToken], i: usize) -> Option<(usize, usize)> {
    tokens
        .iter()
        .filter_map(|t| match t {
            TagToken::Open { span, .. } => Some(*span),
            TagToken::Close { span, .. } => Some(*span),
            TagToken::Skip { .. } => None,
        })
        .nth(i)
}

/// Attribution for check 1 (plan-defined; the spec specifies attribution
/// only for check 2): prefer the regenerated side's divergent token — the
/// concrete byte the output got wrong — mapped through the regen offsets;
/// fall back to the source side's token against source ranges when the
/// regenerated stream is the shorter one (a deletion at or past its end).
fn ledger_offenders(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
    i: usize,
) -> Vec<BlockId> {
    if let Some(span) = non_skip_span(&scan_tags(regenerated), i) {
        let ranges: Vec<(BlockId, ByteRange)> = source_doc
            .blocks
            .iter()
            .filter_map(|b| offsets.0.get(&b.block_id).map(|r| (b.block_id.clone(), *r)))
            .collect();
        return owning_or_flanking(&ranges, span.0);
    }
    if let Some(span) = non_skip_span(&scan_tags(&source_doc.source_text), i) {
        let ranges: Vec<(BlockId, ByteRange)> = source_doc
            .blocks
            .iter()
            .map(|b| (b.block_id.clone(), b.source_range))
            .collect();
        return owning_or_flanking(&ranges, span.0);
    }
    Vec::new()
}

/// The block whose (doc-ordered) range contains `offset`, or — when the
/// offset falls in a gap — the flanking block(s). Deterministic; empty only
/// when `ranges` is empty.
fn owning_or_flanking(ranges: &[(BlockId, ByteRange)], offset: usize) -> Vec<BlockId> {
    for (id, r) in ranges {
        if offset >= r.start && offset < r.end {
            return vec![id.clone()];
        }
    }
    let before = ranges.iter().rev().find(|(_, r)| r.end <= offset);
    let after = ranges.iter().find(|(_, r)| r.start > offset);
    match (before, after) {
        (Some((b, _)), Some((a, _))) => vec![b.clone(), a.clone()],
        (Some((b, _)), None) => vec![b.clone()],
        (None, Some((a, _))) => vec![a.clone()],
        (None, None) => Vec::new(),
    }
}

/// Check 2 (spec §7): fresh segmentation, written to D9's per-`<li>` ruling.
/// The regenerated document goes back through THE intake — the same
/// segmenter that produced `source_doc`, named directly per wave 3's
/// no-re-export rule — and the block count and kind-label sequence must
/// match `source_doc.blocks` in order. No `walk::normalize_top_level` here:
/// both sides are already per-item (D9), and `walk` is the Markdown
/// pairing, never called on an HTML document (spec §7).
fn check_fresh_segmentation(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure> {
    let fresh = transync_syntax::intake::html::parse(regenerated);
    let common = source_doc.blocks.len().min(fresh.blocks.len());
    for i in 0..common {
        let s = source_doc.blocks[i].kind.wire_str();
        let r = fresh.blocks[i].kind.wire_str();
        if s != r {
            let suspects = attribute_offenders(source_doc, &fresh, offsets);
            return Err(ReparseFailure {
                reason: format!(
                    "fresh segmentation: block {i}: source kind {s} != regenerated kind {r} \
                     (at source block `{}`)",
                    source_doc.blocks[i].block_id.0,
                ),
                divergent_source_blocks: if suspects.is_empty() {
                    vec![source_doc.blocks[i].block_id.clone()]
                } else {
                    suspects
                },
            });
        }
    }
    if fresh.blocks.len() != source_doc.blocks.len() {
        let suspects = attribute_offenders(source_doc, &fresh, offsets);
        let fallback_seed = source_doc
            .blocks
            .get(common)
            .or_else(|| source_doc.blocks.last())
            .map(|b| vec![b.block_id.clone()])
            .unwrap_or_default();
        return Err(ReparseFailure {
            reason: format!(
                "fresh segmentation found {} block(s); source has {}",
                fresh.blocks.len(),
                source_doc.blocks.len(),
            ),
            divergent_source_blocks: if suspects.is_empty() { fallback_seed } else { suspects },
        });
    }
    Ok(())
}

/// Check 2's attribution, mirroring `full_reparse::attribute_offenders`:
/// map each fresh block's start offset into the regen `BlockOffsets`; a
/// source block owning ≠ 1 fresh block is the offender. Per `<li>`, never
/// per group — D9 makes every source entry a single block, so the Markdown
/// twin's collapsed-list group fallback has no counterpart here. Walks
/// `source_doc.blocks` in order, so the returned vec is deterministic. A
/// source block with no offsets entry is skipped here (check 4 will name
/// it); comparisons only — this helper must never slice or panic.
fn attribute_offenders(
    source_doc: &Document,
    fresh: &Document,
    offsets: &BlockOffsets,
) -> Vec<BlockId> {
    let mut suspects: Vec<BlockId> = Vec::new();
    for block in &source_doc.blocks {
        let Some(range) = offsets.0.get(&block.block_id) else {
            continue;
        };
        let owned = fresh
            .blocks
            .iter()
            .filter(|f| f.source_range.start >= range.start && f.source_range.start < range.end)
            .count();
        if owned != 1 {
            suspects.push(block.block_id.clone());
        }
    }
    suspects
}
```
- [ ] **Step 6: Watch Task 2's tests go green, and gate the module's imports.**
```bash
cargo test -p transync-core full_rescan -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t2-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t2-green.txt
grep -c 'use transync_syntax::walk' crates/transync-core/src/validate/full_rescan_html.rs
grep -c 'normalize_top_level(' crates/transync-core/src/validate/full_rescan_html.rs
grep -c 'use comrak' crates/transync-core/src/validate/full_rescan_html.rs
grep -c 'comrak::' crates/transync-core/src/validate/full_rescan_html.rs
grep -c 'pub struct' crates/transync-core/src/validate/full_rescan_html.rs
```
Expected: `CARGO_EXIT=0` with all 8 tests passing; then `0` for all five greps — no `walk` import and no `normalize_top_level` call (the doc comments *mention* the word `walk` to forbid it, which is why the gate greps for the import and the call, not the word), no comrak import or path (the module doc names comrak twice while forbidding it — same distinction), and no `pub struct` (no new failure type; `ReparseFailure` is imported). Remember `grep -c` exits 1 on a zero count — the `0` printed IS the pass. Capture the five counts:
```bash
{ grep -c 'use transync_syntax::walk' crates/transync-core/src/validate/full_rescan_html.rs; grep -c 'normalize_top_level(' crates/transync-core/src/validate/full_rescan_html.rs; grep -c 'use comrak' crates/transync-core/src/validate/full_rescan_html.rs; grep -c 'comrak::' crates/transync-core/src/validate/full_rescan_html.rs; grep -c 'pub struct' crates/transync-core/src/validate/full_rescan_html.rs; } > /Volumes/Temp/claude/ti490d97-wave4/gate/t2-import-gates.txt 2>&1
```

- [ ] **Step 7: Edit the orchestrator doc, format, lint, and commit.** In `crates/transync-core/src/validate.rs`, replace the module-doc sentence
```
//! The final layer —
//! the full-document reparse — is the pipeline's final gate: it runs
//! once after regeneration (see [`full_reparse`] and
//! `pipeline::finalize::finalize_regen_with_reparse_policy`), not per batch. See
//! `docs/architecture/contracts.md` §5.
```
with
```
//! The final layer —
//! the full-document gate — runs once after regeneration and dispatches on
//! `Document.format`: [`full_reparse`] for Markdown, [`full_rescan_html`]
//! for HTML (ti 490d97 wave 4; see
//! `pipeline::finalize::finalize_regen_with_reparse_policy`), not per
//! batch. See `docs/architecture/contracts.md` §5.
```
Then:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave4/gate/t2-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t2-clippy.txt
git add crates/transync-core/src/validate.rs crates/transync-core/src/validate/full_rescan_html.rs
git commit -m "feat(validate): the layer-6 twin's spine — the ledger and the fresh segmentation

full_rescan_html re-reads the regenerated HTML with the same scanner the
splice used (check 1: ordered tag inventory, document-wide) and with the
same segmenter the intake used (check 2: block count + kind-label sequence,
per-<li> under D9, attribution mirroring attribute_offenders through the
regen offsets). It returns the SAME ReparseFailure reparse_full returns,
which is what will keep the cascade format-blind when finalize dispatches.
Checks 3 and 4 land next, red-first: the doctype test must fail against
this commit, proving check 3 is a gate and not decoration.

TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
Expected: hook green (fmt, clippy, wasm gate, rustdoc gate).

---

### Task 3: Checks 3 + 4 — the ledger's blind spots, and the bookkeeping

**Files:**
- Modify: `crates/transync-core/src/validate/full_rescan_html.rs`

**Interfaces:**
- Consumes Task 2's helpers (`identity`, `identity_minus`, `parse_html`, `SRC_SMALL`, `SCN_16`) and produces the completed four-check sequence. Nothing outside the module changes.

- [ ] **Step 1: Write the check-3 and check-4 tests — RED FIRST.** Append to the existing `mod tests`:
```rust
    #[test]
    fn a_dropped_doctype_is_ledger_invisible_but_not_gap_invisible() {
        // THE decoration-proof (spec §7 check 3, §12 wave 4): the wave-0
        // MEASURED fact is that `<!DOCTYPE html>` produces no inventory
        // entry — only a `Skip`, which `tag_inventory` filters — so
        // check 1 cannot see it leave; it minted no block (wave
        // 3's doctype-only pin), so check 2 cannot either; the offsets
        // below are coherent, so check 4 passes. ONLY check 3 stands
        // between this corruption and a silent doctype-less output. This
        // test MUST fail against the Task 2 commit (checks 1+2 only) — that
        // red is recorded evidence that check 3 is a gate, not decoration.
        let doc = parse_html(SCN_16);
        let cut_len = "<!DOCTYPE html>\n".len();
        assert!(doc.source_text.starts_with("<!DOCTYPE html>\n"), "fixture sanity");
        let (out, offsets) = identity_minus(&doc, 0..cut_len);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dropped doctype must be rejected");
        assert!(!err.reason.contains("tag inventory"), "got: {err}");
        assert!(!err.reason.contains("fresh segmentation"), "got: {err}");
        assert!(err.reason.contains("gap"), "got: {err}");
        assert!(err.reason.contains("start of document"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks[0].block_id.clone()],
            "the preamble's flanking block is the first block (the title)",
        );
    }

    #[test]
    fn a_dropped_comment_is_ledger_invisible_but_not_gap_invisible() {
        // The second MEASURED blind spot: comments are Skip spans and leave
        // no ledger token. SRC_SMALL's `<!-- note -->` sits in the tail gap
        // (after the last block), so this also covers the tail leg of
        // check 3's "including preamble and tail".
        let doc = parse_html(SRC_SMALL);
        let at = doc.source_text.find("<!-- note -->").expect("fixture sanity");
        let (out, offsets) = identity_minus(&doc, at..at + "<!-- note -->".len());
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a dropped comment must be rejected");
        assert!(err.reason.contains("gap"), "got: {err}");
        assert!(err.reason.contains("end of document"), "got: {err}");
        assert_eq!(
            err.divergent_source_blocks,
            vec![doc.blocks.last().unwrap().block_id.clone()],
            "the tail's flanking block is the last block",
        );
    }

    #[test]
    fn text_moved_between_a_block_and_a_gap_is_still_caught() {
        // The interlock the module doc claims: no tagless corruption
        // escapes, because it either moves the block set (check 2) or moves
        // gap bytes (check 3). Dissolve the rule-T run AND inject
        // whitespace into a different gap (whitespace mints no block, so
        // the injection alone would be check-3-only). Ledger: clean — no
        // tag moved. The dissolved run fires check 2 first; had the run
        // survived, the dirtied gap would have fired check 3. Either path
        // implicates the run's neighborhood.
        let doc = parse_html(SRC_SMALL);
        let run = doc.blocks[1].source_range;
        let (out, offsets) = identity_minus(&doc, run.start..run.end);
        // Splice noise into the gap AFTER the (now empty) run — between the
        // run's collapse point and `<ul>`: three spaces mint no block.
        let (out, offsets) = {
            let at = out.find("<ul>").expect("fixture sanity");
            let mut o2 = BlockOffsets::default();
            let map = |p: usize| -> usize { if p < at { p } else { p + 3 } };
            for (id, r) in offsets.0 {
                o2.0.insert(id, ByteRange { start: map(r.start), end: map(r.end) });
            }
            let mut s = String::new();
            s.push_str(&out[..at]);
            s.push_str("   ");
            s.push_str(&out[at..]);
            (s, o2)
        };
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("whitespace injected into a gap must be rejected");
        // Which check fires first depends on the dissolved run: check 2
        // sees 3 blocks vs 4 BEFORE check 3 reads the gaps — both paths are
        // honest; assert the run is implicated either way.
        assert!(
            err.divergent_source_blocks.contains(&doc.blocks[1].block_id),
            "got: {err} / {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn overlapping_target_ranges_are_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        // Pull block 2's start back inside block 1's range: the gap between
        // them inverts (unreadable → deferred by check 3), and check 4's
        // monotonicity predicate names both blocks.
        let b1_end = offsets.0.get(&doc.blocks[1].block_id).unwrap().end;
        let r2 = offsets.0.get_mut(&doc.blocks[2].block_id).unwrap();
        r2.start = b1_end - 1;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("overlapping ranges must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(
            err.divergent_source_blocks.contains(&doc.blocks[2].block_id),
            "got: {:?}",
            err.divergent_source_blocks,
        );
    }

    #[test]
    fn a_missing_offsets_entry_is_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        offsets.0.remove(&doc.blocks[2].block_id);
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a block with no target range must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(err.reason.contains("no target range"), "got: {err}");
        assert_eq!(err.divergent_source_blocks, vec![doc.blocks[2].block_id.clone()]);
    }

    #[test]
    fn an_out_of_bounds_range_is_a_boundary_fault() {
        let doc = parse_html(SRC_SMALL);
        let (out, mut offsets) = identity(&doc);
        let last = doc.blocks.last().unwrap().block_id.clone();
        offsets.0.get_mut(&last).unwrap().end = out.len() + 1;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("an out-of-bounds range must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert_eq!(err.divergent_source_blocks, vec![last]);
    }

    #[test]
    fn a_range_endpoint_inside_a_char_is_a_boundary_fault() {
        // The char-boundary predicate's OWN red — and the module's one
        // non-ASCII fixture, which is exactly why it exists: every other
        // fixture here is ASCII, where `is_char_boundary` can never say no.
        // Deleting the predicate would turn NO other test red: the
        // unreadable gap would still be deferred by check 3 and swept up by
        // check 4's leftover arm — but with the leftover's reason and EMPTY
        // attribution, leaving the cascade nothing to downgrade. This pin
        // holds the honest diagnosis. (The `é` is written `\u{e9}` so no
        // editor or tool can silently normalize the fixture into a
        // decomposed `e`-plus-combining form whose first byte IS a
        // boundary.)
        let doc = parse_html("<p>caf\u{e9}</p>\n<p>tail</p>\n");
        assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"], "fixture sanity");
        let (out, mut offsets) = identity(&doc);
        // Park block 0's end one byte into the two-byte `é`: the map stays
        // present, in-bounds and monotone, so only this predicate can name
        // the fault. Check 3 defers the now-unreadable gap after block 0
        // (`regenerated.get` returns `None` at a non-boundary endpoint),
        // and check 4 then reports the range that caused the deferral —
        // deviation 2's contract, exercised end-to-end for real.
        let split = out.find('\u{e9}').expect("fixture sanity") + 1;
        assert!(!out.is_char_boundary(split), "fixture sanity: inside the é");
        offsets.0.get_mut(&doc.blocks[0].block_id).unwrap().end = split;
        let err = full_rescan_html(&doc, &out, &offsets)
            .expect_err("a range endpoint inside a char must be rejected");
        assert!(err.reason.contains("boundary"), "got: {err}");
        assert!(err.reason.contains("splits a char"), "got: {err}");
        assert_eq!(err.divergent_source_blocks, vec![doc.blocks[0].block_id.clone()]);
    }

    #[test]
    fn garbage_bookkeeping_never_panics() {
        // The twin's no-panic rule: every offsets-driven slice is
        // `.get()`-guarded, every lookup is `Option`-handled. Whatever the
        // fault, the answer is `Err`, never an unwind — the cascade needs a
        // diagnosis, not a crash (invariant 6: never silently corrupt, and
        // never die).
        let doc = parse_html(SRC_SMALL);
        let (out, _) = identity(&doc);
        let mut garbage = BlockOffsets::default();
        for (i, b) in doc.blocks.iter().enumerate() {
            garbage.0.insert(
                b.block_id.clone(),
                ByteRange { start: usize::MAX - i, end: 7 },
            );
        }
        // These two calls die at check 1 — truncation and emptiness change
        // the tag inventory — so they prove the twin never panics BEFORE
        // rejecting, and nothing more: on these paths the garbage ranges
        // are dead weight that no check ever slices by.
        let truncated = &out[..out.len() / 2];
        assert!(full_rescan_html(&doc, truncated, &garbage).is_err());
        assert!(full_rescan_html(&doc, "", &garbage).is_err());
        // THE call that does the work: the UN-truncated identity output
        // sails through checks 1 and 2 (byte-identical, so the inventory
        // and the fresh segmentation both match), which forces the
        // usize::MAX ranges into check 3 — the only code in the twin that
        // slices by the bookkeeping's ranges. Every interior gap's
        // `regenerated.get(..)` comes back `None` and defers (an unguarded
        // index there would unwind and turn this red), and the walk ends in
        // the tail gap's honest byte-inequality `Err`. Check 4 never slices
        // at all — its predicates are arithmetic plus `is_char_boundary`,
        // which is total — so check 3's guards are the whole no-panic
        // surface this pin exists to hold.
        assert!(full_rescan_html(&doc, &out, &garbage).is_err());
    }
```
- [ ] **Step 2: Watch the reds.**
```bash
cargo test -p transync-core full_rescan -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t3-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t3-red.txt
```
Expected: `CARGO_EXIT=101`, **seven named reds**. The two blind-spot tests and the four boundary tests fail with `` called `Result::expect_err()` on an `Ok` value `` (checks 1+2 pass those corruptions — the recorded proof that checks 3 and 4 are gates). `garbage_bookkeeping_never_panics` fails **only its third assertion**, the identity-output call: the two-check twin returns `Ok` for a byte-identical output no matter how deranged the bookkeeping is. Its first two calls hold even now — truncation and emptiness already fail check 1's inventory comparison — so a green first half is **correct**, not a deviation to record; nor is it coverage, because those two calls never reach any code that slices by the garbage ranges (the test's own comments carry this split). A *panic* anywhere in this round instead of an assertion failure is a Task 2 defect, fix it there. `text_moved_between_a_block_and_a_gap…` already rejects via check 2 (the dissolved run), so its attribution assertion may already pass — either observed state is acceptable **if recorded**. Task 2's eight tests stay green.

- [ ] **Step 3: Implement checks 3 and 4.** Replace the two-check sequence in `full_rescan_html` with:
```rust
    check_tag_ledger(source_doc, regenerated, offsets)?;
    check_fresh_segmentation(source_doc, regenerated, offsets)?;
    let deferred = check_gap_bytes(source_doc, regenerated, offsets)?;
    check_boundaries(source_doc, regenerated, offsets, &deferred)
```
and add:
```rust
/// Check 3 (spec §7): gap byte-identity, preamble and tail included. Gap
/// `g` sits before block `g`; `g == blocks.len()` is the tail. A gap whose
/// endpoints cannot be sliced is DEFERRED to check 4 — the fault is the
/// range, not the bytes — and the deferral is total: every cause of an
/// unreadable gap (missing entry; inverted, out-of-bounds or
/// non-char-boundary range) is one of check 4's predicates, and check 4's
/// last step hard-errors on any leftover, so no gap can fall between the
/// two checks. Comparisons use `str::get`, never indexing: this function
/// must not panic on any input.
fn check_gap_bytes(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<Vec<usize>, ReparseFailure> {
    let n = source_doc.blocks.len();
    let mut deferred: Vec<usize> = Vec::new();
    for g in 0..=n {
        let (s_start, before) = if g == 0 {
            (0, None)
        } else {
            let b = &source_doc.blocks[g - 1];
            (b.source_range.end, Some(b.block_id.clone()))
        };
        let (s_end, after) = if g == n {
            (source_doc.source_text.len(), None)
        } else {
            let b = &source_doc.blocks[g];
            (b.source_range.start, Some(b.block_id.clone()))
        };
        let t_start = if g == 0 {
            Some(0)
        } else {
            offsets
                .0
                .get(&source_doc.blocks[g - 1].block_id)
                .map(|r| r.end)
        };
        let t_end = if g == n {
            Some(regenerated.len())
        } else {
            offsets
                .0
                .get(&source_doc.blocks[g].block_id)
                .map(|r| r.start)
        };
        // Source ranges from the intake are ordered by its debug-asserted
        // invariants; a hand-built document could invert them, so the
        // source side is `.get()`-guarded too and an unreadable source gap
        // defers exactly like an unreadable target gap (check 4's leftover
        // arm then reports it — the one deferral its block-range predicates
        // cannot independently re-derive).
        let src_gap = if s_start <= s_end {
            source_doc.source_text.get(s_start..s_end)
        } else {
            None
        };
        let tgt_gap = match (t_start, t_end) {
            (Some(a), Some(b)) if a <= b => regenerated.get(a..b),
            _ => None,
        };
        let (Some(src_gap), Some(tgt_gap)) = (src_gap, tgt_gap) else {
            deferred.push(g);
            continue;
        };
        if src_gap != tgt_gap {
            let named = |x: &Option<BlockId>| -> String {
                x.as_ref()
                    .map(|id| format!("`{}`", id.0))
                    .unwrap_or_else(|| {
                        if g == 0 { "start of document".to_string() } else { "end of document".to_string() }
                    })
            };
            return Err(ReparseFailure {
                reason: format!(
                    "gap {g} (between {} and {}) differs: source {} byte(s), regenerated {} byte(s)",
                    named(&before),
                    named(&after),
                    src_gap.len(),
                    tgt_gap.len(),
                ),
                divergent_source_blocks: before.into_iter().chain(after).collect(),
            });
        }
    }
    Ok(deferred)
}

/// Check 4 (spec §7): boundary sanity over the regen bookkeeping — every
/// block has a target range; every range is in-bounds against the ACTUAL
/// output length, on char boundaries, monotone and non-overlapping in
/// document order. Runs last, exactly as `reparse_full`'s third scan does:
/// the earlier checks give its indices meaning. The final leftover arm
/// makes deviation 2's deferral contract checkable in code.
fn check_boundaries(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
    deferred: &[usize],
) -> Result<(), ReparseFailure> {
    let mut prev_end = 0usize;
    let mut prev_id: Option<BlockId> = None;
    for block in &source_doc.blocks {
        let Some(range) = offsets.0.get(&block.block_id) else {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` has no target range in the regen bookkeeping",
                    block.block_id.0,
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        };
        if range.start > range.end || range.end > regenerated.len() {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range {}..{} is not in-bounds for output length {}",
                    block.block_id.0, range.start, range.end, regenerated.len(),
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        }
        if !regenerated.is_char_boundary(range.start) || !regenerated.is_char_boundary(range.end) {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range {}..{} splits a char",
                    block.block_id.0, range.start, range.end,
                ),
                divergent_source_blocks: vec![block.block_id.clone()],
            });
        }
        if range.start < prev_end {
            return Err(ReparseFailure {
                reason: format!(
                    "boundary: block `{}` target range starts at {} before block {} ends at {} \
                     (overlap or disorder)",
                    block.block_id.0,
                    range.start,
                    prev_id
                        .as_ref()
                        .map(|id| format!("`{}`", id.0))
                        .unwrap_or_else(|| "<none>".to_string()),
                    prev_end,
                ),
                divergent_source_blocks: prev_id
                    .iter()
                    .cloned()
                    .chain(std::iter::once(block.block_id.clone()))
                    .collect(),
            });
        }
        prev_end = range.end;
        prev_id = Some(block.block_id.clone());
    }
    if let Some(g) = deferred.first() {
        // Reachable only if the totality argument above is wrong — e.g. a
        // hand-built SOURCE range made a source-side gap unreadable. A hard
        // error, not a debug_assert: reaching it means the bookkeeping
        // model itself is broken, and the cascade still needs an Err.
        return Err(ReparseFailure {
            reason: format!(
                "boundary: gap {g} was unreadable although every block range passed \
                 the boundary predicates",
            ),
            divergent_source_blocks: Vec::new(),
        });
    }
    Ok(())
}
```
- [ ] **Step 4: Watch everything go green, then format, lint, commit.**
```bash
cargo test -p transync-core full_rescan -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t3-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t3-green.txt
```
Expected: `CARGO_EXIT=0`, all 16 tests passing. Then:
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave4/gate/t3-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t3-clippy.txt
git add crates/transync-core/src/validate/full_rescan_html.rs
git commit -m "feat(validate): the twin's gap and boundary checks close the ledger's blind spots

A dropped <!DOCTYPE> or comment leaves no tag_inventory entry (each scans
as a Skip token, which the ledger filters — wave 0 MEASURED) and mints no
block, so checks 1 and 2 pass it — the recorded red against the previous
commit is the proof. Check 3 reads every gap,
preamble and tail included, from the OUTPUT; check 4 re-verifies the
bookkeeping (presence, bounds against the actual output length, char
boundaries, monotone non-overlap) and hard-errors if check 3's deferral
left anything uncovered, so no gap can fall between the two.

TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: The one format branch in `finalize`, and the gates that prove nothing else moved

**Files:**
- Modify: `crates/transync-core/src/pipeline/finalize.rs`

**Interfaces:**
- Consumes: `full_rescan_html` (Tasks 2–3); `crate::id::SourceFormat` (wave 2); the **unchanged** `finalize_regen_with_reparse_policy` signature, quoted from the file as read during planning:
  ```rust
  pub(crate) fn finalize_regen_with_reparse_policy(
      doc: &crate::parser::Document,
      accepted: &mut HashMap<BlockId, ValidatedUnit>,
      policy: FullReparseFailure,
  ) -> Result<
      (
          String,
          crate::regen::BlockOffsets,
          Vec<crate::validate::ValidatedBatch>,
      ),
      crate::validate::full_reparse::ReparseFailure,
  >
  ```
  and `ValidatedUnit { unit_id: BlockId, final_status: FallbackStatus, accepted_payload: Option<String>, rejected_by: Option<ValidationLayer>, rejection_reason: Option<String>, warnings: Vec<String> }` (quoted from `validate.rs`).
- **The cascade-untouched contract, verified during planning and re-verified here:** the three `reparse_full` call sites in this function (initial, post-stage-1, post-stage-2) are the ONLY lines that change. `downgrade_units` consumes `failure.divergent_source_blocks.iter()` only; `widen_to_neighbors` consumes the seed list plus `crate::regen::top_level_blocks(&doc.blocks)`, whose body is `blocks.iter()` (format-blind — read from `regen.rs`); `fall_back_all` consumes no failure field; `reason` reaches only `tracing::warn!` and the Hard-arm `Err` that `pipeline.rs` maps to `"full reparse failed: {reason}"`. None of those five functions is edited.

- [ ] **Step 1: Write the dispatch tests — RED FIRST.** Append a new test module to `finalize.rs` (below the existing ones; the existing `reparse_policy_tests` and `list_item_count_pipeline_tests` are **not edited**):
```rust
// ti 490d97 wave 4: the ONE format branch (spec §7). These tests drive
// `finalize_regen_with_reparse_policy` directly — the pub(crate) seam — with
// intake-built HTML documents and hand-built accepted maps, because no run
// path can construct a `format == Html` document until wave 5's entry point
// and wave 6's flag exist. That unreachability is this wave's hard rule, and
// the acceptance section checks it mechanically.
#[cfg(test)]
mod format_dispatch_tests {
    use super::*;
    use crate::parser::parse;
    use crate::test_fixtures::{EVIL_SOURCE_MD, evil_accepted};

    /// One `<p>` element block and one rule-T anonymous run. The run is the
    /// lever: a whitespace-only translated segment splices cleanly (segment
    /// count 1 == 1; no tags on either side, so per-unit layer 3's
    /// inventory check passes it too — this exact shape reaches regen in a
    /// real wave-5 run), but the resulting run is textless and dissolves
    /// under rule T. Only the layer-6 twin can see that.
    const HTML_SRC: &str = "<div>\n<p>keep</p>\nnaked run text\n</div>\n";

    fn html_doc() -> crate::parser::Document {
        transync_syntax::intake::html::parse(HTML_SRC)
    }

    fn unit(id: &BlockId, payload: Option<String>) -> ValidatedUnit {
        ValidatedUnit {
            unit_id: id.clone(),
            final_status: crate::FallbackStatus::Translated,
            accepted_payload: payload,
            rejected_by: None,
            rejection_reason: None,
            warnings: Vec::new(),
        }
    }

    /// Accepted map: the `<p>` honestly translated, the run "translated" to
    /// whitespace. Payloads are JSON segment arrays — the shape wave 2's
    /// re-keyed regen arm decodes for every Html-spelled block.
    fn accepted_with_dissolving_run(
        doc: &crate::parser::Document,
    ) -> HashMap<BlockId, ValidatedUnit> {
        let p_id = doc.blocks[0].block_id.clone();
        let run_id = doc.blocks[1].block_id.clone();
        let mut accepted = HashMap::new();
        accepted.insert(
            p_id.clone(),
            unit(&p_id, Some(serde_json::to_string(&vec!["유지"]).unwrap())),
        );
        accepted.insert(
            run_id.clone(),
            unit(&run_id, Some(serde_json::to_string(&vec![" "]).unwrap())),
        );
        accepted
    }

    #[test]
    fn an_html_document_takes_the_twin_and_the_twin_names_the_dissolved_run() {
        let doc = html_doc();
        assert_eq!(doc.blocks.len(), 2, "fixture sanity: <p> + rule-T run");
        let mut accepted = accepted_with_dissolving_run(&doc);
        let failure = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::Hard,
        )
        .expect_err("Hard must surface the twin's failure");
        // The twin's vocabulary, never reparse_full's: the dissolved run is
        // a fresh-segmentation count divergence.
        assert!(failure.reason.contains("fresh segmentation"), "got: {}", failure.reason);
        assert!(
            !failure.reason.contains("regenerated block count"),
            "reparse_full's phrase must not appear — got: {}",
            failure.reason,
        );
        assert!(
            failure
                .divergent_source_blocks
                .contains(&doc.blocks[1].block_id),
            "got: {:?}",
            failure.divergent_source_blocks,
        );
    }

    #[test]
    fn fallback_per_block_restores_the_run_and_keeps_the_honest_translation() {
        let doc = html_doc();
        let run_id = doc.blocks[1].block_id.clone();
        let mut accepted = accepted_with_dissolving_run(&doc);
        let (out, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::FallbackPerBlock,
        )
        .expect("FallbackPerBlock degrades, never aborts (invariant 6)");
        // Stage 1 downgrades exactly the dissolved run; the re-regen
        // splices its SOURCE bytes back and the twin passes.
        assert!(out.contains("유지"), "the honest translation survives:\n{out}");
        assert!(out.contains("naked run text"), "the run is source bytes again:\n{out}");
        let vu = accepted.get(&run_id).expect("run unit still present");
        assert_eq!(vu.final_status, crate::FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
        assert_eq!(
            accepted.get(&doc.blocks[0].block_id).unwrap().final_status,
            crate::FallbackStatus::Translated,
            "the innocent neighbor is NOT downgraded at stage 1",
        );
    }

    #[test]
    fn fallback_all_returns_the_source_bytes_for_an_html_document() {
        // Stage 3's "structurally identical by construction" claim rests on
        // wave 3's identity theorem for HTML — pinned here at the finalize
        // seam: all-fallback regen IS the source document, byte-identical.
        let doc = html_doc();
        let mut accepted = accepted_with_dissolving_run(&doc);
        let (out, _offsets, _final) = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::FallbackAll,
        )
        .expect("FallbackAll always succeeds");
        assert_eq!(out, HTML_SRC);
        for vu in accepted.values() {
            assert_eq!(vu.final_status, crate::FallbackStatus::FallbackSource);
            assert!(vu.accepted_payload.is_none());
        }
    }

    #[test]
    fn a_markdown_document_still_takes_reparse_full_not_the_twin() {
        // The POSITIVE half of §12's "Markdown runs provably still take
        // reparse_full" (the zero-edit green suite is the other half):
        // reparse_full's count-drift phrase appears, and the twin's
        // vocabulary provably does not. The reason strings are the two
        // gates' distinguishing marks — see Task 2's vocabulary contract.
        let doc = parse(EVIL_SOURCE_MD).expect("source parses");
        let mut accepted = evil_accepted(&doc);
        let failure = finalize_regen_with_reparse_policy(
            &doc,
            &mut accepted,
            crate::FullReparseFailure::Hard,
        )
        .expect_err("the evil fixture diverges under reparse_full");
        assert!(
            failure.reason.contains("regenerated block count"),
            "got: {}",
            failure.reason,
        );
        assert!(!failure.reason.contains("tag inventory"), "got: {}", failure.reason);
        assert!(!failure.reason.contains("fresh segmentation"), "got: {}", failure.reason);
    }
}
```
- [ ] **Step 2: Watch the reds.**
```bash
cargo test -p transync-core format_dispatch -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t4-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t4-red.txt
```
Expected: `CARGO_EXIT=101`. Before the branch exists, finalize routes the HTML document into `reparse_full` — comrak over HTML, the exact hazard §7 names — so the three HTML tests fail on their **assertions**, not on compiles: `an_html_document_takes_the_twin…` fails its `reason.contains("fresh segmentation")` assert (the observed reason carries `reparse_full`'s kind/count vocabulary — its exact text is comrak's business and is not pinned here); `fallback_per_block_restores…` fails `out.contains("유지")` (the comrak-driven cascade over-downgrades to FallbackAll, so the honest translation is gone — the "worse than ungated" behavior, observed live); `fallback_all_returns…` may already pass (FallbackAll never consults the gate — record whichever is observed). `a_markdown_document_still_takes_reparse_full…` already passes (nothing routed Markdown anywhere else yet). The red file is the recorded proof that the branch, not luck, is what makes an HTML run trustworthy.

- [ ] **Step 3: Land the dispatcher — THE one format branch.** In `finalize.rs`, add below `finalize_regen_with_reparse_policy`:
```rust
/// THE one format branch (ti 490d97 wave 4, spec §7): the document-level
/// layer-6 gate, dispatched on `Document.format` — comrak's reparse for
/// Markdown, the scanner rescan for HTML. Both return the same
/// [`crate::validate::full_reparse::ReparseFailure`], which is what keeps
/// the three-stage cascade above format-blind: `downgrade_units` and
/// `widen_to_neighbors` consume only `divergent_source_blocks` (over
/// `regen::top_level_blocks` = `blocks.iter()`), `fall_back_all` consumes
/// neither field, and `reason` reaches only the logs and the Hard-arm
/// error. `walk`/comrak never see an HTML document — this branch is the
/// enforcement (spec §7: "not by adding arms").
fn layer6_gate(
    doc: &crate::parser::Document,
    regenerated: &str,
    offsets: &crate::regen::BlockOffsets,
) -> Result<(), crate::validate::full_reparse::ReparseFailure> {
    match doc.format {
        crate::id::SourceFormat::Markdown => {
            crate::validate::full_reparse::reparse_full(doc, regenerated, offsets)
        }
        crate::id::SourceFormat::Html => {
            crate::validate::full_rescan_html::full_rescan_html(doc, regenerated, offsets)
        }
    }
}
```
The match is exhaustive over `SourceFormat` with **both arms named** — no `_ =>` (Global Constraints; a third format added later must stop the compiler here). Then reroute the three call sites — each currently reading
```rust
crate::validate::full_reparse::reparse_full(doc, &md, &offsets)
```
(one in the initial gate, one after stage 1, one after stage 2) — to
```rust
layer6_gate(doc, &md, &offsets)
```
**Nothing else in the function changes**: the `md` local keeps its name (it holds HTML on an HTML run; renaming it is wave-5/6 churn this wave declines), and `downgrade_units` / `widen_to_neighbors` / `fall_back_all` / `regen_pass` / `collect_validated` are byte-untouched.

- [ ] **Step 4: Watch everything go green.**
```bash
cargo test -p transync-core -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t4-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t4-green.txt
```
Expected: `CARGO_EXIT=0` — the four new tests AND every pre-existing test (`reparse_policy_tests`, `list_item_count_pipeline_tests`, the `full_reparse` suite, the pipeline suites) with zero edits. Any pre-existing red is a defect in this wave's code, never a reason to touch the old test.

- [ ] **Step 5: The cascade-untouched gate — mechanical, not asserted.**
```bash
git diff "$(cat /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt)"..HEAD -- crates/transync-core/src/validate/full_reparse.rs crates/transync-core/src/pipeline/report.rs crates/transync-core/src/pipeline.rs > /Volumes/Temp/claude/ti490d97-wave4/gate/t5-untouched.txt 2>&1
echo "GIT_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t5-untouched.txt
git diff "$(cat /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt)"..HEAD -- crates/transync-core/src/pipeline/finalize.rs > /Volumes/Temp/claude/ti490d97-wave4/gate/t5-finalize-hunks.txt 2>&1
```
Expected: `t5-untouched.txt` is an **empty diff** (`GIT_EXIT=0`, no hunks — `full_reparse.rs` byte-untouched is the strongest form of "`ReparseFailure`'s shape is unchanged", and `report.rs`/`pipeline.rs` untouched is the cascade-and-report half). Inspect `t5-finalize-hunks.txt` as a separate step: its only hunks are the `layer6_gate` function, the three one-line call-site edits, and the new test module — **no hunk touches `downgrade_units`, `widen_to_neighbors`, `fall_back_all`, `regen_pass`, or `collect_validated`**. Also pin the shape textually:
```bash
{ grep -c 'pub struct ReparseFailure' crates/transync-core/src/validate/full_reparse.rs; grep -c 'pub reason: String' crates/transync-core/src/validate/full_reparse.rs; grep -c 'pub divergent_source_blocks: Vec<BlockId>' crates/transync-core/src/validate/full_reparse.rs; } > /Volumes/Temp/claude/ti490d97-wave4/gate/t5-shape.txt 2>&1
```
Expected: `1` / `1` / `1`.

- [ ] **Step 6: The scope-leak gate — `translate()` cannot reach `format == Html`.**
```bash
grep -rn 'intake::html' crates/transync-core/src --include='*.rs' > /Volumes/Temp/claude/ti490d97-wave4/gate/t5-scope.txt 2>&1
grep -c 'input_format' crates/transync-core/src/lib.rs >> /Volumes/Temp/claude/ti490d97-wave4/gate/t5-scope.txt 2>&1; echo "grep_exit=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t5-scope.txt
grep -n 'let mut doc = parse(source)' crates/transync-core/src/pipeline.rs >> /Volumes/Temp/claude/ti490d97-wave4/gate/t5-scope.txt 2>&1
```
Expected, reading the file as a separate step: `intake::html` appears **only** in `validate/full_rescan_html.rs` (the twin's check 2 + its tests) and `pipeline/finalize.rs`'s `format_dispatch_tests` module — never in `pipeline.rs`, `unit*`, `llm*`, or `lib.rs`; `input_format` count is `0` (with `grep_exit=1`, the zero-count exit — that is the good outcome); and `run_pipeline`'s single document constructor is still `let mut doc = parse(source)?` — the Markdown intake, which stamps `format: Markdown`. Together with wave 6 owning the only CLI flag and wave 5 owning the only library entry, that is what makes "the Html arm is test-only until wave 5" a mechanical fact. **If any of these greps shows otherwise — an `intake::html` call on a run path, an input-format option, a second document constructor — that is a stop-the-wave finding: report it before continuing.**

- [ ] **Step 7: Format, lint, commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave4/gate/t4-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t4-clippy.txt
git add crates/transync-core/src/pipeline/finalize.rs
git commit -m "feat(pipeline): the layer-6 gate dispatches on Document.format — the one branch

layer6_gate sends Markdown to reparse_full and HTML to full_rescan_html;
the three call sites in finalize_regen_with_reparse_policy route through
it and nothing else moves — the cascade consumes only reason +
divergent_source_blocks, so it is format-blind by shape, and the diff
proves downgrade_units/widen_to_neighbors/fall_back_all are byte-
untouched. The recorded red shows what the branch prevents: comrak over
an HTML document produced a passing-shaped cascade that threw away an
honest translation. No HTML run is reachable: translate() still has
exactly one document constructor (the Markdown intake) and no
input-format option exists until waves 5/6.

TRACE: ti 490d97 wave 4 (spec 2026-08-20 §7)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Records — DCR-0036 and the routine files (no `docs/index.md` step)

**Files:**
- Create: `docs/project/design-change-records/DCR-0036-html-layer6-twin.md`
- Modify: `docs/architecture/contracts.md`, `docs/implementation/module-map.md`, `docs/architecture/source-of-truth-table.md`, `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`, `CLAUDE.md`
- **NOT modified: `docs/index.md`** — deviation 4; the controller owns it this wave. Step 7 hands over the link line.

**Interfaces:**
- Consumes: the finished wave. Produces: the record set spec §10 assigns per landed wave (DCR; routine CHANGELOG/status/phase-state/module map; contracts §5a's layer-6 dispatch sentence — the §5a **cache** paragraph is wave 5's and must NOT be written here), plus the §12 amendment owed by this plan's parallelism ruling.

- [ ] **Step 1: Confirm the DCR number.** Record numbers are reserved per wave, not taken from the disk (wave 2's rule): wave 0 holds DCR-0032, wave 1 holds DCR-0033 by name in its own plan, wave 2 holds DCR-0034, wave 3 holds DCR-0035 — **this wave takes DCR-0036** even if 0033 or 0035 is not on disk yet.
```bash
ls docs/project/design-change-records/ | grep -c 'DCR-0036'
```
Expected: `0` (`grep -c` exits 1 on a zero count — that is the *good* outcome here). If it prints `1` or more, an unplanned record took the number: STOP and reconcile with the controller before renumbering anything.

- [ ] **Step 2: Write the DCR.** Create `docs/project/design-change-records/DCR-0036-html-layer6-twin.md`, opening with the house OKF frontmatter (`type: DCR`, `tags: [change, project-control, DCR-0036]`, matching DCR-0035's field set exactly — copy its frontmatter shape, not its text). Body sections, in the house order:
  - **What changed:** `transync-core::validate::full_rescan_html` — the HTML layer-6 twin (spec §7): document-wide ordered tag ledger, fresh segmentation through `transync_syntax::intake::html::parse` written to D9's per-`<li>` ruling with `attribute_offenders`-mirroring attribution, gap byte-identity including preamble and tail, boundary sanity against the actual output length; and the ONE format branch — `layer6_gate` in `pipeline::finalize` — dispatching `reparse_full` for Markdown, `full_rescan_html` for HTML. `ReparseFailure`'s shape is unchanged (`full_reparse.rs` is byte-untouched in the wave's diff), so the DCR-0004 three-stage cascade is untouched by construction and by diff.
  - **Why:** wave 4 of ti `490d97` — the gate. Spec §7/§12's hard rule: no HTML translation run ships before the twin, because comrak over an HTML document produces some node sequence and can pass while checking nothing — an HTML run gated by `reparse_full` is worse than ungated. The wave's recorded red demonstrates it: pre-branch, the comrak-driven cascade over an HTML document discarded an honest translation on its way to FallbackAll.
  - **The §12 parallelism amendment, owed (this plan's deviation 1):** §12 calls wave 4 "parallel with 3" and "developable against hand-built `Document`s before intake is complete"; check 2 consumes the intake, so that holds only for checks 1/3/4 and the finalize branch. In the executed ordering wave 4 followed wave 3 and calls `intake::html::parse` directly; a segmenter seam was rejected as a one-implementation stub gate (the same rule as wave 3's deviation 2) and a second HTML opinion. **Post-implementation review item (§14-style): amend spec §12's wave-4 entry to say so.** The spec file was deliberately not edited in this wave.
  - **The §7 blind-spot wording amendment, owed (recorded exactly like the §12 item above — wave 3's deviation-5 shape):** spec §7's check-3 rationale still says "`<!DOCTYPE>` and comments produce no token in `scan_tags`". Since wave 0 landed the bogus-comment state they DO produce a token — a `Skip` — and what they leave none of is a **ledger entry**, because `tag_inventory` filters `Skip`. The conclusion §7 draws is unchanged and still load-bearing (ledger-invisible, gap-visible — hence check 3); only the stated mechanism is stale. **Post-implementation review item (§14-style): amend the sentence to the ledger-entry form.** The spec file was deliberately not edited in this wave.
  - **The residuals, recorded (spec §7; §13 item 7):** swapped same-skeleton texts pass all four checks — exact parity with `reparse_full`'s blindness, pinned by `swapped_same_skeleton_texts_pass_by_documented_parity`; attribute values are untokenized, compensated by splice-never-edits-inside-tags, pinned escaping, and the SCN-16 end-to-end criterion; the scanner is one self-consistent tokenizer opinion, not a general parser — both sides of every comparison use it.
  - **Evidence:** the staged red/green gate files under `/Volumes/Temp/claude/ti490d97-wave4/gate/` — `t2-red-compile.txt` / `t2-red-stub.txt` (the twin's reds), `t3-red.txt` (the decoration-proof: dropped doctype/comment accepted by checks 1+2 alone), `t4-red.txt` (comrak-over-HTML observed live), `t4-green.txt` (workspace-shape green, zero fixture edits), `t5-untouched.txt` (empty diff over `full_reparse.rs`/`report.rs`/`pipeline.rs`), `t5-scope.txt` (no run path reaches the Html arm).
  - **Consequences / hand-forwards:** wave 5 makes `translate()` reach `format == Html` (entry point, prompt, cache separation, `html_dominance_warning` re-text) and inherits one wording residual recorded here: the pipeline's Hard-arm mapping `"full reparse failed: {reason}"` (pinned out-of-crate by the facade's `boundary_v02::hard_failure_maps_error_and_evicts_implicated_keys`; an earlier draft of this line named a `hard_policy_maps_error_and_evicts` that has never existed in the tree, and `finalize.rs`'s comment carried the same wrong name until it was corrected on 2026-08-20 — this prose is the last copy, and it is the one that would otherwise land in a shipped record) will prefix the twin's reasons too — "reparse" is then loose for HTML; re-word or re-pin **in the wave the entry point lands**, never silently. Wave 6 adds the flag, wire fields and `out.html`; wave 7 the browser gate.

- [ ] **Step 3: contracts.md §5a — the layer-6 format dispatch.** In `docs/architecture/contracts.md`, in **### 5a. Unit validity: provisional vs final**, item 2 currently reads:
```
2. **Final (document-level):** the post-regeneration full-document reparse (SCN-14). Only a result that survives regeneration into the whole document is final. The DCR-0004 cascade may downgrade provisionally-valid units to `fallback_source` (attributed blocks → widened neighbors → all); downgraded units are reported in `ValidationReport.full_reparse_fallbacks`.
```
Replace it with:
```
2. **Final (document-level):** the post-regeneration full-document gate (SCN-14). Only a result that survives regeneration into the whole document is final. Since ti `490d97` wave 4 (DCR-0036) the gate dispatches on `Document.format`: a Markdown run takes the comrak reparse (`validate::full_reparse`), an HTML run takes the scanner rescan (`validate::full_rescan_html` — ordered tag ledger, fresh segmentation per D9, gap byte-identity including preamble and tail, boundary sanity). Both return the same `ReparseFailure` shape, so the DCR-0004 cascade below is format-blind: it may downgrade provisionally-valid units to `fallback_source` (attributed blocks → widened neighbors → all); downgraded units are reported in `ValidationReport.full_reparse_fallbacks`.
```
Do **not** touch §5a's cache paragraphs — the no-axis paragraph is wave 5's record.

- [ ] **Step 4: module-map, source-of-truth table, CLAUDE.md.** Three exact edits:
  - `docs/implementation/module-map.md`, in the `validate/` tree, change the last row's connector and append the new row — from:
```
    │   └── full_reparse.rs             # parse the regenerated full doc; anchor count, label
    │                                   #   sequence, per-list item count (DCR-0017 Guard 1)
```
    to:
```
    │   ├── full_reparse.rs             # parse the regenerated full doc; anchor count, label
    │   │                               #   sequence, per-list item count (DCR-0017 Guard 1)
    │   └── full_rescan_html.rs         # the HTML layer-6 twin: ordered tag ledger, fresh
    │                                   #   segmentation (D9 per-<li>), gap byte-identity,
    │                                   #   boundary sanity; dispatched from finalize on
    │                                   #   Document.format (DCR-0036)
```
    and in the "Scenario → component coverage" preamble change `` `validate` (+ its five layers) `` to `` `validate` (+ its six layers) `` — a count that stops being true is the wave-0 "six members" lesson.
  - `docs/architecture/source-of-truth-table.md`, the `transync-core::validate` row's Notes: change `Layered checks (schema → IDs → per-kind → fragment reparse → full reparse).` to `Layered checks (schema → IDs → per-kind → fragment reparse → full document gate: full reparse for Markdown, full rescan for HTML, dispatched on Document.format in pipeline::finalize — DCR-0036).` (No new table row: `docs_ownership_drift` welds the crate-root `lib.rs` module lists — its `CRATE_ROOTS` names three roots today: `transync-syntax`, `transync-core`, and wave 0's `transync-html` — and this wave adds a child module under `validate/`, not a crate-root one — verified against the test's `CRATE_ROOTS` during planning.)
  - `CLAUDE.md`, the `transync-core` module-split list's `validate` row: change `` - `validate` — layered validators (schema, IDs, per-kind, fragment reparse, inline protection, full reparse) `` to `` - `validate` — layered validators (schema, IDs, per-kind, fragment reparse, inline protection); the document-level layer-6 gate dispatches on `Document.format` — `full_reparse` (comrak) for Markdown, `full_rescan_html` (scanner: ledger, fresh segmentation, gap bytes, boundaries) for HTML ``.

- [ ] **Step 5: CHANGELOG.** Under `## [Unreleased]`'s `### Added` (after the wave-3 entry), append:
```markdown
- The HTML layer-6 twin (`transync-core::validate::full_rescan_html`, ti `490d97` wave 4, spec §7, DCR-0036): the post-regeneration gate for HTML documents — document-wide ordered tag ledger, fresh segmentation re-run through the intake (per-`<li>`, D9), gap byte-identity including preamble and tail (closing the ledger's measured `<!DOCTYPE>`/comment blind spots), and boundary sanity. `finalize` dispatches on `Document.format`; `ReparseFailure` is unchanged, so the three-stage fallback cascade is untouched. No HTML translation run is reachable yet — the entry point is wave 5, the CLI flag wave 6.
```

- [ ] **Step 6: `docs/project/status.md` and `docs/project/phase-state.yaml`.** Record wave 4 as landed in exactly the shape waves 0–3 recorded theirs (a per-wave line/entry under the ti `490d97` work record): wave 4 — the layer-6 twin, DCR-0036, demonstrable outcome "an HTML run can be trusted to fail loudly; the gate precedes any real run". Do not mark the feature further along than it is: waves 5–7 remain open, and any "next action" line must now name **wave 5 (units, context, prompt — the entry point)** as the next step, with the standing hard rule discharged: the twin exists, so wave 5 may proceed.

- [ ] **Step 7: Hand the index link to the controller — do NOT edit `docs/index.md`.** Give the controller this exact line for insertion after the DCR-0035 line (numeric order):
```markdown
- [DCR-0036 — The HTML layer-6 twin (ti 490d97 wave 4): `full_rescan_html` gates HTML regen output, and `finalize` dispatches on `Document.format`](project/design-change-records/DCR-0036-html-layer6-twin.md)
```
Until the controller lands it, `docs_index_drift` is expected red naming exactly `DCR-0036-html-layer6-twin.md`. Verify that the red is exactly that, bare-to-file:
```bash
cargo test -p transync --test docs_index_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave4/gate/t6-index.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/t6-index.txt
```
Expected while the controller's edit is pending: `CARGO_EXIT` non-zero with the failure naming **only** the DCR file (and possibly this plan's own file if the controller's move-and-link has not completed). Any other name is real drift — STOP. After the controller confirms, re-run into `t6-index-after.txt` and expect `CARGO_EXIT=0`.

- [ ] **Step 8: Commit the records.**
```bash
git add docs/project/design-change-records/DCR-0036-html-layer6-twin.md \
        docs/architecture/contracts.md docs/implementation/module-map.md \
        docs/architecture/source-of-truth-table.md \
        CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml CLAUDE.md
git commit -m "docs(records): DCR-0036 — the layer-6 twin landed, the gate precedes the run

The record carries the four checks, the untouched-cascade proof, the three
residuals, the recorded comrak-over-HTML red, and two owed items: the spec
§12 parallelism amendment (check 2 consumes the intake, so wave 4 followed
wave 3 in this execution) and the Hard-arm 'full reparse failed' prefix
wording, deferred to the wave the HTML entry point lands. The DCR's
docs/index.md link is the controller's edit, handed over, not made here.

TRACE: ti 490d97 wave 4
TRACE: DCR-0036

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
Note: the pre-commit hook runs fmt, clippy, the wasm gate and the rustdoc gate — none of which is `docs_index_drift` — so this commit lands cleanly while the index link is pending. That is by design, and Step 7's capture is the honest record of the interim red.

---

## Wave acceptance — check all six before declaring wave 4 done

Spec §12 wave 4's acceptance items, plus this plan's own gates, each with its evidence:

1. **Dropped / reordered / inventory-drift each rejected** — `a_dropped_element_block_is_rejected_by_the_ledger`, `a_reordered_block_pair_is_rejected_by_the_ledger`, `tag_inventory_drift_inside_a_block_is_rejected`, green in `t3-green.txt`; their staged reds in `t2-red-stub.txt`. The beyond-floor reds: the two check-2-only dissolved runs (label arm and count arm), the two check-3 blind-spot cases, the four check-4 boundary cases (overlap, missing entry, out-of-bounds, split char), and the no-panic pin's identity-output call.
2. **Check 3 is a gate, not decoration** — `a_dropped_doctype_is_ledger_invisible_but_not_gap_invisible` and the comment twin were **observed red** against the Task 2 commit (checks 1+2 only): `t3-red.txt` is the proof that removing check 3 un-gates a real corruption.
3. **Markdown runs provably still take `reparse_full`** — both halves: the whole pre-existing suite green with **zero edits** (`t4-green.txt`, and the acceptance diff below shows no test file edits outside the two named modules), and the positive routing proof `a_markdown_document_still_takes_reparse_full_not_the_twin` asserting `reparse_full`'s reason vocabulary present and the twin's absent.
4. **`ReparseFailure` shape unchanged, cascade untouched** — `t5-untouched.txt` is an empty diff over `full_reparse.rs`, `report.rs` and `pipeline.rs`; `t5-finalize-hunks.txt` shows no hunk in the five cascade/projection functions; `t5-shape.txt` pins the two-field struct. (Planning-time verification, restated for the reviewer: `downgrade_units` consumes the id list, `widen_to_neighbors` consumes ids + `top_level_blocks` = `blocks.iter()`, `fall_back_all` consumes neither failure field, and `report.rs` imports no `ReparseFailure` at all.)
5. **No HTML translation run became reachable** — `t5-scope.txt`: `intake::html` only in the twin and the dispatch tests; no `input_format` in `lib.rs`; `run_pipeline`'s single constructor is the Markdown intake. Plus the whole-tree containment diff:
```bash
git diff --stat "$(cat /Volumes/Temp/claude/ti490d97-wave4/gate/baseline-commit.txt)"..HEAD -- crates/transync-syntax crates/transync-html crates/transync-cli crates/transync-openai crates/transync-anthropic crates/transync-wasm crates/transync web 'crates/*/Cargo.toml' Cargo.toml Cargo.lock > /Volumes/Temp/claude/ti490d97-wave4/gate/accept-containment.txt 2>&1
echo "GIT_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave4/gate/accept-containment.txt
```
   Expected: an **empty diff** — this wave changed nothing outside `crates/transync-core` and the named docs files.
6. **The final green, bare-to-file** — `cargo test --workspace -- --test-threads=4` into `accept-workspace.txt` with `CARGO_EXIT=0`; the wasm gate into `accept-wasm.txt` with `CARGO_EXIT=0` (string unchanged); `docs_index_drift` green **after** the controller's link (`t6-index-after.txt`), `docs_ownership_drift` green (no top-level module moved).

## Spec §7 coverage map

| Spec §7 statement | Where in this plan |
|---|---|
| `full_rescan_html(source_doc, regenerated, offsets) -> Result<(), ReparseFailure>`, same scanner the splice used | Task 2 Steps 3+5 (signature, `tag_inventory`/`scan_tags` imports) |
| Check 1: document-wide ordered ledger, gaps included | Task 2 Step 5 `check_tag_ledger`; dropped/reordered/drift reds |
| Check 2: fresh segmentation, D9's per-`<li>` ruling (whole-`<ul>` projection superseded), attribution mirrors `attribute_offenders` | Task 2 Step 5 `check_fresh_segmentation` + `attribute_offenders`; `attribution_is_per_li_not_per_list`; the dissolved-run red |
| Check 3: gap byte-identity, preamble + tail, closes the two MEASURED ledger blind spots | Task 3 Steps 1+3; the doctype/comment reds observed against the checks-1+2 commit |
| Check 4: boundary sanity against the actual output length | Task 3 Step 3 `check_boundaries`; the four boundary reds; deviation 2's totality contract in code |
| The three residuals, honestly recorded | the module doc (Task 2 Step 1), the parity pin, DCR-0036 |
| Cascade: one format branch in `finalize`; `ReparseFailure` unchanged; `downgrade_units`/`widen_to_neighbors`/`fall_back_all` untouched; `top_level_blocks` = `blocks.iter()` | Task 4 Steps 3+5; acceptance items 4–5 |
| Retry/fallback: invariant 6 survives mechanically; every cascade arm ends `Ok`, never corrupt output | Task 4's three cascade tests (stage-1 recovery keeps the honest neighbor; FallbackAll = source bytes via wave 3's identity theorem) |
| `walk`/comrak never called on an HTML document — enforced by the branch, not by arms | the twin's import gates (Task 2 Step 6); `layer6_gate`'s doc |
| Hard rule: no HTML translation run before the twin | the scope-leak gate (Task 4 Step 6, acceptance 5) and this plan's "What this wave must NOT do" |

## Notes for the implementer

- **Fix bugs where they live.** Task 3's and Task 4's tests are gates over Task 2's implementation and finalize's dispatch; a red pin is a code defect in this wave's own modules — never weaken a pin, never edit a pre-existing test, never touch a fixture.
- **The reason-string vocabulary is load-bearing.** `"tag inventory"`, `"fresh segmentation"`, `"gap "`, `"boundary"` vs `reparse_full`'s `"regenerated block count"` / `"source kind … != regenerated kind"` / `"list at block"` — Task 4's routing proofs distinguish the gates by these strings. If you re-word a reason, update its assertions in the same commit and say why; do not let the two gates' vocabularies converge.
- **`grep -c` exits non-zero on a zero count** — the gate greps read the printed count, not the exit code; do not run them under `set -e`.
- **The expected block sets were hand-derived** from SRC_SMALL/HTML_SRC and spec §4's classification table, cross-checked against wave 3's own hand-derived SCN-16 sequence (14 blocks, `title` first, the badge pair as ONE `image`). If a fixture-sanity assertion disagrees with what the landed intake produces, STOP and re-derive against spec §4 — one of the two was misread; do not adjust either side to make the other pass.
- **The wave's reds, named honestly:** one compile red (`E0425`, Task 2 Step 2 — wiring), one stub red round (Task 2 Step 4 — the four ledger/segmentation rejections), the decoration-proof round (Task 3 Step 2 — blind spots + boundaries + the no-panic pin's identity-output call against the two-check twin), and the dispatch round (Task 4 Step 2 — comrak-over-HTML observed live). Do not skip a red "to save a compile": each is recorded evidence a specific check or branch is load-bearing.
- **If `transync_html::splice` of a whitespace-only segment behaves unexpectedly** (Task 4's `[" "]` payload — chosen over `[""]` to avoid any empty-string edge in the rewriter), the honest fallback in the test is a payload that dissolves the run some other way, not a weakened assertion; measure first (`splice("naked run text", &[" ".into()], Keep)` in a scratch test), then adapt, and record what was measured in the commit body.
- **`md` stays `md`** in finalize — it holds HTML on an HTML run; the rename is wave-5/6 churn and renaming it here would bloat the diff the cascade-untouched gate inspects.
- **The controller owns `docs/index.md` this wave** (deviation 4). If `docs_index_drift` is red naming anything other than this plan's file or DCR-0036, that is real drift: STOP and report, do not link it yourself.
