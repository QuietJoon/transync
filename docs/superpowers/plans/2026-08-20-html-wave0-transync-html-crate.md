# HTML→HTML Wave 0 — `transync-html` Crate Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the HTML mechanics out of `transync-syntax` into a new `transync-html` workspace member, extend the tag scanner with the two things the future HTML intake needs, and repoint every call site and weld — changing no behaviour the existing suite can observe.

**Architecture:** `crates/transync-syntax/src/htmlseg.rs` becomes `crates/transync-html/src/lib.rs` verbatim (a `git mv`, so the diff reads as a rename), losing `#[doc(hidden)] pub mod htmlseg` with **no re-export alias** — an alias would let the old path keep working and the migration would never finish. `splice`'s `block_type: u8` becomes a `BlankLinePolicy` whose one CommonMark constructor is the sole surviving home of the `matches!(block_type, 6 | 7)` rule. `TagToken::Open` gains the `span` `scan_tags` already computes and discards, a `TagToken::Skip { span }` variant names the comment / CDATA / bogus-comment regions the scanner passes over silently, `element_extents` is the `balance_fragment` stack walk refactored out from under it (never a second copy), and `strip_reserved_sync_attrs` lands here in its final home so wave 1 does not write it in the old crate and move it a commit later.

**Tech Stack:** Rust 2024 workspace (rustc ≥ 1.88), `lol_html` 2, `htmlize` 1, `comrak` 0.27, `serde`/`serde_json`, `toml` (dev), `wasm32-unknown-unknown` check gate, tracked pre-commit hook (fmt + clippy + wasm gate + rustdoc gate).

**Spec:** docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md

## Global Constraints

- **Acceptance gate for the whole wave:** the full workspace suite green with **zero fixture or expectation edits**. A fixture under `crates/*/tests/fixtures/` or an existing assertion's *expected value* changing is a plan failure, not a fix — stop and report.
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`. The new crate is covered transitively; do not add a package to that command.
- `transync-syntax` may gain **no `[features]` table** and **no `transync-core` dependency, dev-dependencies included** — either breaks the wasm gate that lives in `scripts/hooks/pre-commit` and `scripts/smoke.sh`.
- `transync-html` gets **no `[features]` table** and depends on `lol_html` + `htmlize` only.
- Every test run: `cargo test -p <crate> -- --test-threads=4`; workspace runs: `cargo test --workspace -- --test-threads=4`. Never raise the cap.
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append the exit code, and inspect the file as a *separate* step.
- Lint gates (the pre-commit hook enforces them): `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- **`git commit --no-verify` is never used.** A commit step's expected result is that the hook runs fmt, clippy, the wasm gate and the rustdoc gate, and all four pass. If the hook blocks, fix the cause.
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave0/` — never `/tmp`, never `/private/tmp`, never `$TMPDIR`. If `/Volumes/Temp/claude` is unreachable, stop and ask.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`. If a cargo command fails because the target dir is unreachable, stop and ask.
- No pure-formatting edits. Let `cargo fmt` own wrapping — where this plan shows wrapped code, run `cargo fmt --all` afterwards and take the formatter's answer.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, or cite them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **Every enum this crate publishes stays exhaustive** — `TagToken`, `BlankLinePolicy`, and anything later tasks add: no `#[non_exhaustive]`, and no `_ =>` catch-all arm in any in-crate match over one. The R0002-0020 / R0003-0066 phantom-token incidents were caught precisely because nothing hid behind one. **`matches!(x, Variant)` counts as a catch-all**: it expands to a match with an implicit `_ => false`, so a variant added later silently takes the `false` branch instead of stopping the compiler. Match exhaustively and name every arm. (This constraint originally said `TagToken` alone, which is how Task 3's `matches!(blank_lines, BlankLinePolicy::Collapse)` passed review as compliant; Task 5 Step 0 closes it. Matches over `char`, `u8` or `AttrState` are not covered — they are not this crate's enums and have no exhaustible variant set.)

---

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `crates/transync-html/Cargo.toml` | **create** | new member's manifest: `lol_html` + `htmlize`, no `[features]`, charter comment mirroring `transync-syntax`'s |
| `crates/transync-html/src/lib.rs` | **create** (`git mv` of `htmlseg.rs`) | the whole HTML mechanics layer: tag scanner, pairing predicates, element extents, segment extract/splice, blank-line policy, fragment balancing, reserved-attribute strip, and all in-crate tests |
| `crates/transync-html/tests/token_stream_pin.rs` | **create** | the characterization pin: `Open`/`Close` projection, `tag_inventory`, and `balance_fragment` frozen against goldens generated *before* the token change |
| `crates/transync-html/tests/goldens/token-stream.txt` | **create** (generated) | the frozen inventory + `Open`/`Close` stream, one section per corpus entry |
| `crates/transync-html/tests/goldens/balanced/*.txt` | **create** (generated) | the frozen `balance_fragment` output, one file per corpus entry |
| `Cargo.toml` (root) | modify | `members` gains the crate; `[workspace.dependencies]` gains `transync-html`; the publication-set comment six → seven; the versioning comment four → five |
| `Cargo.lock` | modify | new member; regenerated by cargo, staged by hand |
| `crates/transync-syntax/Cargo.toml` | modify | drops `lol_html` + `htmlize`, gains `transync-html`; header comment loses "the lol_html segment engine" |
| `crates/transync-syntax/src/lib.rs` | modify | drops `#[doc(hidden)] pub mod htmlseg;` and its comment block; crate-doc sentence renamed |
| `crates/transync-syntax/src/outcome.rs` | modify | `crate::htmlseg::extract` → `transync_html::extract`; module doc + `html_outcomes` doc renamed |
| `crates/transync-syntax/src/regen.rs` | modify | the `BlockKind::Html` splice call site (+ the `preserved_html_block_round_trips_byte_identical` test's `extract` call) |
| `crates/transync-syntax/src/render.rs` | modify | `crate::htmlseg::balance_fragment` → `transync_html::balance_fragment` |
| `crates/transync-syntax/src/parser.rs` | modify | one comment naming `htmlseg` |
| `crates/transync-core/Cargo.toml` | modify | gains `transync-html` |
| `crates/transync-core/src/unit/payload.rs` | modify | `transync_syntax::htmlseg::extract` → `transync_html::extract` |
| `crates/transync-core/src/validate.rs` | modify | the layer-3 splice + inventory call sites, and the two test helpers' `extract` calls |
| `crates/transync-core/src/validate/per_kind.rs` | modify | one comment naming `htmlseg::scan` |
| `crates/transync/tests/public_surface.rs` | modify | `forbidden` gains `"transync_html"` |
| `crates/transync/tests/workspace_publication.rs` | modify | `PUBLISHED_MEMBERS` six → seven, `transync-html` first |
| `crates/transync/tests/docs_ownership_drift.rs` | modify | `CRATE_ROOTS` gains a row and a per-root module floor |
| `scripts/lib/rustdoc-gate.sh` | modify | `RUSTDOC_GATE_CRATES` gains `transync-html` (smoke's completeness check fails the run otherwise) |
| `docs/index.md` | **verify** in Task 1, modify in Task 7 | this plan's link and the spec's are already present (the docs-index drift test requires every doc under `docs/` to be linked, so they had to land with the files themselves); Task 7 adds the DCR-0032 link |
| `docs/architecture/contracts.md` | modify | §0 tier-(c) prose gains the crate + the note that the facade re-exports nothing from it |
| `docs/architecture/source-of-truth-table.md` | modify | intro enumeration, the raw-HTML-unit row, and the out-of-scope list's raw-HTML ownership clause |
| `docs/architecture/README.md` | modify | the "HTML segment engine" component row |
| `docs/implementation/module-map.md` | modify | the crate tree, the module enumeration, and the SCN-15 scenario row |
| `docs/project/release-checklist.md` | modify | steps 17 and 19: four → five internal requirements, six → seven published members, the new row first, and the `transync-anthropic` row's `#3` cross-reference |
| `docs/Developer_Guide.md` | modify | the rustdoc-gate command block, the gate's prose crate list, and the "Workspace at a glance" crate tree |
| `docs/backlog.md` | modify | the still-open `template-webcomponent-extraction` entry's pointer at `htmlseg.rs` |
| `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md` | **create** | the wave's record |
| `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml` | modify | routine per-wave records |

---

### Task 1: Scaffold `transync-html`, move `htmlseg.rs` verbatim, repoint every call site, land every §5 weld

**Files:**
- Create: `crates/transync-html/Cargo.toml`
- Create (via `git mv`): `crates/transync-html/src/lib.rs`
- Modify: `Cargo.toml`, `Cargo.lock`, `crates/transync-syntax/Cargo.toml`, `crates/transync-syntax/src/lib.rs`, `crates/transync-syntax/src/outcome.rs`, `crates/transync-syntax/src/regen.rs`, `crates/transync-syntax/src/render.rs`, `crates/transync-syntax/src/parser.rs`, `crates/transync-core/Cargo.toml`, `crates/transync-core/src/unit/payload.rs`, `crates/transync-core/src/validate.rs`, `crates/transync-core/src/validate/per_kind.rs`, `crates/transync/tests/public_surface.rs`, `crates/transync/tests/workspace_publication.rs`, `crates/transync/tests/docs_ownership_drift.rs`, `scripts/lib/rustdoc-gate.sh`, `docs/architecture/contracts.md`, `docs/architecture/source-of-truth-table.md`, `docs/architecture/README.md`, `docs/implementation/module-map.md`, `docs/project/release-checklist.md`, `docs/Developer_Guide.md`, `docs/backlog.md`
- Verify only (already correct on disk, and staged in Step 18 so it rides this commit with the plan file it points at): `docs/index.md`
- Test: no new test file. **The existing suite is the test** — this task is characterized, not TDD-driven, and its red/green signal is the baseline capture in Step 1 versus the verification capture in Step 16.

**Interfaces:**
- Produces the crate `transync_html` with this public surface (everything else in the file stays `pub(crate)` or private):
  ```rust
  pub struct HtmlSegments { pub texts: Vec<String>, pub labels: Vec<String> }
  pub fn extract(block: &str) -> Result<HtmlSegments, String>;
  pub fn splice(block: &str, translated: &[String], block_type: u8) -> Result<String, String>;
  pub enum TagToken {
      Open { name: String, self_closing: bool },
      Close { name: String, span: (usize, usize) },
  }
  pub fn scan_tags(html: &str) -> Vec<TagToken>;
  pub fn tag_inventory(html: &str) -> Vec<String>;
  pub fn is_void(tag: &str) -> bool;
  pub fn is_raw_text(tag: &str) -> bool;
  pub fn implicitly_closes(name: &str) -> &'static [&'static str];
  pub fn balance_fragment(html: &str) -> String;
  ```
  `splice`'s third parameter is **still `block_type: u8` at the end of this task** — Task 3 changes it. `TagToken::Open` still has **no `span` field** and there is **no `Skip` variant** — Task 4 adds them. `element_extents` / `ElementExtent` (Task 5) and `strip_reserved_sync_attrs` (Task 6) do not exist yet.
- Stays private inside the crate: `scan`, `scan_chunks`, `scan_with_memory_cap`, `NodeRecord`, `collapse_blank_lines`, `rewriter_settings`, `rewriter_err`, `wanted_text_type`, `is_foreign_root`, `AttrState`, `VOID_ELEMENTS`, `RAW_TEXT_ELEMENTS`, `DEFAULT_MAX_MEMORY_BYTES`.
- Consumes: nothing from earlier tasks (this is the first).

- [ ] **Step 1: Baseline capture — the characterization "red".** A pure refactor has no failing test to write; its evidence is that a suite which was green before is byte-for-byte as green after, over fixtures nobody edited. Capture the before-state:
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave0/gate
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/baseline-cli.txt
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave0/gate/baseline-commit.txt
```
Then, as a **separate** step, read each file's last line. Expected: `CARGO_EXIT=0` in both, and no line containing `FAILED`. If either is non-zero, STOP — the tree was not green before this wave and nothing after this point can be attributed to the refactor.

**One trap this baseline is uniquely able to trip, and every future wave's plan inherits it.** `crates/transync/tests/docs_index_drift.rs` walks the *filesystem* under `docs/` and requires every `.md` it finds — including everything under `docs/superpowers/plans/` and `docs/superpowers/specs/` — to be linked from `docs/index.md`. A new document is therefore not inert until someone wires it up: **writing the file is itself what turns a weld red.** So the link must land in the same change that creates the document, never in a later step of the plan that document *is*; a plan whose own index link sits at step 15 of 18 has ordered a STOP against itself at step 1. This plan's file and its spec are both linked already — Step 15 verifies rather than adds them — which is the only reason this baseline can be green. If the baseline is red and the failure is `docs_index_drift`, do **not** defer it to Step 15: read the failure's file list, link what it names, and re-run.

- [ ] **Step 2: Create the new member's manifest.** `crates/transync-html/Cargo.toml`:
```toml
# transync-html: the HTML mechanics layer — tag scanning, the pairing
# discipline, element extents, text-segment extract/splice, and render-path
# fragment balancing. Extracted verbatim from `transync-syntax::htmlseg`
# (ti 490d97 wave 0; spec 2026-08-20 §5, decisions D3 and D10).
#
# Structural intake — classification, BlockKind assignment, ids, ast_path —
# is NOT here; it belongs to transync-syntax. This crate has no opinion about
# what a block is.
#
# No [features] and no workspace-member dependency, ever. This crate sits
# under `transync-syntax`, which must keep passing the standing gate
# `cargo check -p transync-syntax -p transync-wasm --target
# wasm32-unknown-unknown`; the gate's STRING does not change because the new
# crate is covered transitively, and it stays green because this dependency
# set is a subset of the one transync-syntax already carried.
# TRACE: ADR-0003 (amended)
# TRACE: ADR-0018
# TRACE: DCR-0032

[package]
name         = "transync-html"
version.workspace      = true
edition.workspace      = true
rust-version.workspace = true
license.workspace      = true
repository.workspace   = true
description  = "HTML mechanics for transync: tag scanning, element extents, text-segment extract/splice, and fragment balancing."

[dependencies]
# Segment extraction/splice rides lol_html's streaming rewriter; htmlize
# supplies the full named-entity decode table (lol_html has no decoder).
lol_html    = { workspace = true }
htmlize     = { workspace = true }
```

- [ ] **Step 3: Move the file with rename detection intact.**
```bash
mkdir -p crates/transync-html/src
git mv crates/transync-syntax/src/htmlseg.rs crates/transync-html/src/lib.rs
```
Expected: `git status --porcelain` shows `R  crates/transync-syntax/src/htmlseg.rs -> crates/transync-html/src/lib.rs`. Do **not** copy-and-delete; the rename is what makes the wave's diff reviewer-checkable.

- [ ] **Step 4: Replace the moved file's module doc with the crate doc (D10's two required sentences).** In `crates/transync-html/src/lib.rs`, replace lines 1–6 (the old `//! HTML segment extraction / splice engine (lol_html).` block) with:
```rust
//! HTML mechanics for transync: tag scanning, the pairing discipline,
//! element extents, text-segment extraction/splice, and render-path fragment
//! balancing (`lol_html`).
//!
//! One pinned Settings source (`rewriter_settings`) feeds every pass so the
//! extract pass and the splice pass can never disagree on coalescing or drop
//! decisions.
//!
//! **Structural intake does not live here.** Classification, `BlockKind`
//! assignment, block ids and `ast_path` are `transync-syntax`'s job — the
//! `transync_syntax::intake::html` module — and this crate is the mechanics
//! layer underneath it. The name says "html" because the capability is HTML,
//! not because the crate decides what an HTML block *is*.
//!
//! **The module formerly called `htmlseg` inside `transync-syntax` is this
//! crate.** Records dated before 2026-08-20 — ADR-0018, ADR-0003,
//! `docs/project/stub-manifest.md`, `docs/project/status.md`,
//! `docs/project/open-issues-archive.md`,
//! `docs/project/implementation-slice-checklists.md`, and the investigation
//! bundle — name `htmlseg`, and they are dated records that are not
//! rewritten. A reader who greps for `htmlseg` and finds nothing is looking
//! at this.
//!
//! Spec: docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md
//! §5. The original engine spec, still the authority on the extract/splice
//! algorithm, is
//! docs/superpowers/specs/2026-08-03-html-content-translation-design.md
//! §3.2–§3.4.
//!
//! TRACE: ADR-0018
//! TRACE: DCR-0016
//! TRACE: DCR-0032
```

- [ ] **Step 5: Open the visibility the spec's §5 names, and fix the one intra-doc link the widening breaks.** In `crates/transync-html/src/lib.rs`, five edits, all mechanical:

  1. `fn is_void(tag: &str) -> bool {` → add a doc line and `pub`:
```rust
/// Is `tag` an HTML void element? Void elements never get an end tag, so a
/// stack walker must not push them and a balancer must not close them.
pub fn is_void(tag: &str) -> bool {
```
  2. `fn is_raw_text(tag: &str) -> bool {` → add a doc line and `pub`:
```rust
/// Does `tag` hold raw text / RCDATA (`script`, `style`, `textarea`,
/// `title`)? A browser never tokenizes their content as markup, and HTML
/// ignores the self-closing flag on them (DCR-0016 Part D).
pub fn is_raw_text(tag: &str) -> bool {
```
  3. `enum TagToken {` → add a doc line and `pub`:
```rust
/// One tag-shaped region [`scan_tags`] recognized, in document order.
///
/// Exhaustive by policy: no `#[non_exhaustive]`, and no in-crate match may
/// use a `_ =>` arm. R0002-0020 and R0003-0066 were both phantom tokens
/// caught because nothing hid behind one.
#[derive(Debug, Clone)]
pub enum TagToken {
```
  4. `fn scan_tags(html: &str) -> Vec<TagToken> {` → `pub fn scan_tags(...)`, plus two doc-comment fixes the widening forces: replace `every [`RAW_TEXT_ELEMENTS`] entry` with `every [`is_raw_text`] element` (the const stays private, and a public item linking a private sibling is exactly what the rustdoc gate denies), and replace the closing sentence `only the render-path balancer (spec §3.4) and the layer-3 inventory check use it.` with `it is one self-consistent opinion about HTML tokenization, shared by the render-path balancer (spec 2026-08-03 §3.4), the layer-3 inventory check, and the HTML intake to come.`
  5. `pub(crate) fn balance_fragment(html: &str) -> String {` → `pub fn balance_fragment(...)`, and `fn implicitly_closes(name: &str) -> &'static [&'static str] {` → `pub fn implicitly_closes(...)`.

  Leave `pub(crate) fn scan`, `pub(crate) fn scan_with_memory_cap`, `pub(crate) fn collapse_blank_lines`, `pub(crate) struct NodeRecord` and every private item exactly as they are.

- [ ] **Step 6: Wire the member into the workspace, and move the two unwelded comments.** In the root `Cargo.toml`:

  - `members`: add `"crates/transync-html",` **first** in the list (dependency order).
  - The publication-set comment: `# Publication set (R0001-0040). Six members are ordinary crates.io packages,` → `Seven members`, and its dependency-order list `transync-syntax, transync-core, transync, transync-openai, transync-anthropic, transync-cli` gains `transync-html` at the front.
  - The `[workspace.dependencies]` versioning comment: `Bumping `[workspace.package] version` means bumping these four in the same edit;` → `these five`.
  - `[workspace.dependencies]`: add, immediately above the `transync-syntax` line so the table reads in dependency order:
```toml
transync-html   = { version = "0.4.0", path = "crates/transync-html" }
```
  Neither comment is welded by any test. A missed one is a permanently lying comment, which is why they are named individually.

- [ ] **Step 7: Move the two dependencies off `transync-syntax` and onto the two crates that now need the new member.**
  - `crates/transync-syntax/Cargo.toml`: delete the `lol_html` and `htmlize` lines with their three-line comment; add in their place:
```toml
# HTML-content translation (spec 2026-08-03 / 2026-08-20 §5): the mechanics
# — extract/splice, tag scanning, fragment balancing — live in their own
# member since ti 490d97 wave 0. `regen`, `render` and `outcome` call it.
transync-html = { workspace = true }
```
  Also fix the manifest's header comment: `alignment maps, HTML rendering, and the lol_html segment engine.` → `alignment maps, and HTML rendering. The lol_html segment engine moved to transync-html (DCR-0032).`
  - `crates/transync-core/Cargo.toml`: add after the `transync-syntax` entry:
```toml
# HTML mechanics (DCR-0032): `validate`'s layer-3 splice + tag-inventory
# check and `unit::payload`'s segment extraction call it directly.
transync-html = { workspace = true }
```

- [ ] **Step 8: Drop the module from `transync-syntax` — no alias.** In `crates/transync-syntax/src/lib.rs`, delete these six lines entirely:
```rust
// Internal engine, not curated API: the lol_html extract/splice/inventory
// routines are consumed by `regen`/`render` inside this crate and by
// `unit`/`validate` in `transync-core`, which is the only reason they are
// `pub` at all. OI-0027 curates the real surface and gets this list.
#[doc(hidden)]
pub mod htmlseg;
```
and in the crate doc, replace `//! HTML rendering, and the lol_html segment engine. `transync-core` builds` with:
```rust
//! HTML rendering. The lol_html segment engine moved to `transync-html`
//! (DCR-0032), which this crate depends on. `transync-core` builds
```
**Add no `pub use transync_html as htmlseg;`.** A hidden alias would let every old path keep compiling and the migration would never finish.

- [ ] **Step 9: Repoint the three `transync-syntax` call sites and its two stale comments.**
  - `src/outcome.rs`: `crate::htmlseg::extract(&payload)` → `transync_html::extract(&payload)`; module doc `the `htmlseg` extract pass` → `` the `transync-html` extract pass ``; `html_outcomes` doc `Run `htmlseg::extract` over every` → `` Run `transync_html::extract` over every ``.
  - `src/render.rs`: `crate::htmlseg::balance_fragment(md),` → `transync_html::balance_fragment(md),`.
  - `src/regen.rs`: `.and_then(|segs| crate::htmlseg::splice(source_bytes, &segs, *block_type).ok())` → `.and_then(|segs| transync_html::splice(source_bytes, &segs, *block_type).ok())`; and in the test `preserved_html_block_round_trips_byte_identical`, `crate::htmlseg::extract("<p>Caf&eacute;&nbsp;&copy;</p>")` → `transync_html::extract("<p>Caf&eacute;&nbsp;&copy;</p>")`.
  - `src/parser.rs`: the comment `` The byte-verbatim round-trip guarantee `regen` and `htmlseg` keep — the `` → `` …`regen` and `transync-html` keep — the ``.

- [ ] **Step 10: Repoint the `transync-core` call sites and its one stale comment.**
  - `src/unit/payload.rs`: `transync_syntax::htmlseg::extract(&raw)` → `transync_html::extract(&raw)`.
  - `src/validate.rs`, the layer-3 arm — three names in four lines:
```rust
            Ok(segs) => match transync_html::splice(&h.source_bytes, &segs, *block_type) {
                Err(e) => return direct_fallback(format!("html splice failed: {e}")),
                Ok(spliced) => {
                    if transync_html::tag_inventory(&spliced)
                        != transync_html::tag_inventory(&h.source_bytes)
```
  - `src/validate.rs`, the two test helpers: both occurrences of `transync_syntax::htmlseg::extract(source).expect("source block extracts")` → `transync_html::extract(source).expect("source block extracts")`, and the doc line `` (real `htmlseg::extract` over the same bytes) `` → `` (real `transync_html::extract` over the same bytes) ``.
  - `src/validate/per_kind.rs`: the comment `` R0003-0042: WHITESPACE-only, not just empty. `htmlseg::scan` drops `` → `` …the segment engine's scan drops ``. (`scan` is private in the new crate, so naming it as a path would be a lie about reachability.)

- [ ] **Step 11: Build the two crates and prove the move compiles.**
```bash
cargo check -p transync-html -p transync-syntax -p transync-core > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-check.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-check.txt
```
Read the file separately. Expected: `CARGO_EXIT=0`. If it reports `error[E0433]: failed to resolve: use of undeclared crate or module `htmlseg``, a call site was missed — grep for survivors:
```bash
grep -rn "htmlseg" crates/transync-syntax/src crates/transync-core/src crates/transync-html/src
```
must return **nothing**. (`crates/transync/tests/public_surface.rs` and `crates/transync/tests/docs_ownership_drift.rs` still name the retired string on purpose; Steps 12–13 settle those.)

- [ ] **Step 12: Weld — `public_surface.rs`.** In `crates/transync/tests/public_surface.rs`, in `hidden_modules_are_not_documented_as_surface`'s `forbidden` array, add `"transync_html",` immediately after `"transync_syntax",`. The check matches per `::` path segment, so the existing `"htmlseg"` entry cannot catch a `transync_html::…` path. Leave `"htmlseg"` in place — it costs nothing and still forbids the retired name.

- [ ] **Step 13: Weld — `workspace_publication.rs` and `docs_ownership_drift.rs`.**
  - `workspace_publication.rs`: `PUBLISHED_MEMBERS` gains `"transync-html",` as the **first** entry — a published `transync-syntax` cannot depend on an unpublished crate — and the doc comment above it stays accurate as written.
  - `docs_ownership_drift.rs`: replace the `CRATE_ROOTS` constant and the assert that reads it. Constant:
```rust
/// The crate roots whose module lists the table's intro enumerates, each with
/// the floor its own `lib.rs` must clear.
///
/// The floor is **per root** because `transync-html` is a single file: ti
/// 490d97 wave 0 moved `htmlseg.rs` in verbatim, and the file-as-module split
/// the spec permits is a later, separate decision. One shared floor of 5
/// would assert a shape nobody has chosen for that crate; the two roots above
/// it are what keep the anti-vacuity check meaningful.
const CRATE_ROOTS: &[(&str, &str, usize)] = &[
    ("transync-syntax", "crates/transync-syntax/src/lib.rs", 5),
    ("transync-core", "crates/transync-core/src/lib.rs", 5),
    ("transync-html", "crates/transync-html/src/lib.rs", 0),
];
```
  and, in `the_module_enumeration_names_every_module_both_crates_declare`:
```rust
    for (krate, rel, floor) in CRATE_ROOTS {
        let modules = declared_modules(&read_doc(rel));
        assert!(
            modules.len() >= *floor,
            "{rel} parsed to {} modules, below its floor of {floor} — the scan \
             broke, it did not get simpler",
            modules.len(),
        );
```
  Also fix the now-false comment inside `declared_modules`: `` // Only `cfg` hides a module; `#[doc(hidden)]` (`htmlseg`, `unit`) `` → `` // Only `cfg` hides a module; `#[doc(hidden)]` (`unit`) ``.

- [ ] **Step 14: Weld — the rustdoc gate's crate list.** In `scripts/lib/rustdoc-gate.sh`:
```bash
RUSTDOC_GATE_CRATES=(transync-html transync-syntax transync-core transync transync-openai transync-anthropic transync-wasm)
```
`scripts/smoke.sh` walks `crates/*/` and fails the run when a member with a `src/lib.rs` is not named there; the pre-commit hook sources the same file. Missing this makes smoke red, not the test suite, which is exactly the kind of gap that sits unnoticed.

- [ ] **Step 15: Weld — the seven documents, plus the two `docs/index.md` links that must already be there.**
  - `docs/index.md` — **verify, do not add.** Both links this wave depends on already sit in the "Brainstorming / specs" list. The check is two greps:
```bash
grep -c "superpowers/specs/2026-08-20-html-to-html-translation-design.md" docs/index.md
grep -c "superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md" docs/index.md
```
  Expected: `1` from each. The lines they match read (elided after the em dash — do not rewrite them, they are already correct):
```markdown
- [HTML→HTML Document Translation (2026-08-20)](superpowers/specs/2026-08-20-html-to-html-translation-design.md) — owner-ratified design spec …
- [HTML→HTML Wave 0 — the `transync-html` crate — implementation plan (2026-08-20)](superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md) — the 7-task plan for the pure-refactor wave …
```
  If either grep prints `0`, add the missing link **before** doing anything else and re-run Step 1's baseline: a doc under `docs/` that nothing links makes `docs_index_drift.rs` red, so Step 1's "the tree was green before this wave" was false and every comparison built on it is void. **The ordering rule, which every later wave's plan inherits: a document that makes a weld red must be linked in the same change that creates it — never in a later step of the plan it is part of.** That is why this bullet verifies instead of edits; Step 18 stages `docs/index.md` alongside this plan file so the link and the document it points at ride one commit, and Task 7 Step 5 is the one step that genuinely *edits* `docs/index.md`, for the DCR-0032 record.
  - `docs/architecture/contracts.md` §0, the tier-(c) paragraph: `` `pipeline`, `parser`, `unit`, `batch`, `validate`, `regen`, `render`, and the whole of `transync-syntax` are not reachable through the facade `` → `` `pipeline`, `parser`, `unit`, `batch`, `validate`, `regen`, `render`, the whole of `transync-syntax`, and the whole of `transync-html` (the HTML mechanics crate extracted from `transync-syntax::htmlseg` by DCR-0032 — **the facade re-exports nothing from it**) are not reachable through the facade ``. Extend the paragraph's closing sentence to name the third crate: `depends on `transync-core` / `transync-syntax` / `transync-html` directly and accepts their weaker stability promise.`
  - `docs/architecture/source-of-truth-table.md`, the intro paragraph: `` `align`, `htmlseg`, `id`, `outcome`, `parser`, `regen`, `render`, and `walk` live in `crates/transync-syntax` `` → `` `align`, `id`, `outcome`, `parser`, `regen`, `render`, and `walk` live in `crates/transync-syntax`; the HTML mechanics that were `transync-syntax::htmlseg` are their own member, `crates/transync-html`, which declares no modules of its own (DCR-0032) ``. In the "Which raw-HTML blocks become translation units" row, `` `html_outcomes` runs `htmlseg::extract` `` → `` `html_outcomes` runs `transync_html::extract` ``. And in the out-of-scope list near the end of the file — the bullet beginning `**Raw HTML left this list on 2026-08-04:**` — `` block-level raw HTML is a translatable kind owned by `transync-syntax::{parser, htmlseg, regen, render}` `` → `` block-level raw HTML is a translatable kind owned by `transync-html` plus `transync-syntax::{parser, regen, render}` ``. That third occurrence is welded by nothing and its ownership clause is **present tense**, so it is a claim about the tree as it stands, not a dated record — leave it and the file still names a module that no longer exists.
  - `docs/architecture/README.md`, the component table: `| HTML segment engine | `crates/transync-syntax` (`htmlseg` mod) |` → `| HTML mechanics engine | `crates/transync-html` |`.
  - `docs/implementation/module-map.md`, **three** edits, and the third is the one a `grep htmlseg` after the first two still finds: remove the `htmlseg.rs` node (and its continuation line) from the `transync-syntax` tree, add a sibling `crates/transync-html/` entry with `└── lib.rs  # tag scan + element extents + segment extract/splice + balancing (DCR-0032)`; drop `htmlseg` from the module enumeration in the "Scenario → component coverage" preamble — that enumeration wraps across lines, and the token to delete sits between `` `align`, `` and `` `outcome`, `` in `` `transync-syntax` owns … `align`, `htmlseg`, `outcome`, `walk`; `transync-core` owns … ``; and fix the **SCN-15 row** of the scenario table, whose module set still opens with the retired name — `` `{htmlseg, parser, unit (html_outcomes), validate::per_kind, regen, render, align}`, `web/js/sync.js` (toggle mirror) `` → `` `transync-html`, `{parser, unit (html_outcomes), validate::per_kind, regen, render, align}`, `web/js/sync.js` (toggle mirror) ``. The crate is named outside the braces because the braces hold *modules* by that table's own preamble, and this crate declares none — the SCN-12 row already uses that shape for `transync-cli` / `transync-openai`.
  - `docs/project/release-checklist.md`: step 17 — `the four requirements that shadow it` → `the five requirements`, `All seven members` → `All eight members`, `carries the four internal members` → `carries the five internal members`, and the parenthesized list gains `transync-html`. Step 19 — `Six members are ordinary crates.io packages` → `Seven members`, the table gains a first row `| 1 | `transync-html` | yes | the HTML mechanics layer, under the base crate (DCR-0032) |` with the following rows renumbered 2–7, `the four internal members` → `the five internal members`, and `It packages and verify-builds all six` → `all seven`. **The renumbering breaks a cross-reference inside the table itself**, and it breaks it silently — the `transync-anthropic` row's Why cell reads `depends on `transync` only, so it may publish any time after #3`, and `#3` is `transync` only while `transync-syntax` is `#1`. After the insert `transync` is `#4` and `#3` is `transync-core`, so the cell would state a publication order nobody decided. In the same edit: `so it may publish any time after #3` → `so it may publish any time after #4`. Nothing welds this table's numbering, which is exactly why it has to be done by hand and here.
  - `docs/Developer_Guide.md`: the gate command block becomes
```bash
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps \
  -p transync-html -p transync-syntax -p transync-core -p transync \
  -p transync-openai -p transync-anthropic -p transync-wasm   # standing rustdoc gate
```
  and the prose list `` — `transync-syntax`, `transync-core`, `transync`, `transync-openai`, `transync-wasm`. `` → `` — `transync-html`, `transync-syntax`, `transync-core`, `transync`, `transync-openai`, `transync-anthropic`, `transync-wasm`. `` (the prose had already fallen a member behind `rustdoc-gate.sh`; this brings both to the real list — note it wraps mid-list in the file, between `` `transync-core`, `` and `` `transync`, ``).
  **A third edit in the same file, which the gate command and the prose list both miss:** the ASCII crate tree under "## Workspace at a glance" still enumerates `transync-syntax`'s modules with `htmlseg` among them, and gains no line for the new member. Two changes there — the `transync-syntax` line loses the module: `` │   ├── transync-syntax/      syntax layer (parser, id, regen, render, align, htmlseg, outcome, walk); compiles for wasm32 under a standing gate `` → `` │   ├── transync-syntax/      syntax layer (parser, id, regen, render, align, outcome, walk); compiles for wasm32 under a standing gate ``, and a new line goes **above** it (dependency order, matching the roster elsewhere in this task), padded to the tree's 22-column name field:
```text
│   ├── transync-html/        HTML mechanics: tag scan, element extents, segment extract/splice, fragment balancing; lol_html + htmlize only (DCR-0032)
```
  This tree is welded by nothing, so a `grep -rn htmlseg docs/` after the wave is the only thing that would have caught it.

  **A third line in the same tree, pre-existing drift found while editing its neighbours.** The tree enumerates **six** members; the workspace has had **eight** since `transync-anthropic` landed on 2026-08-10 (DCR-0029), and it will have nine after this wave. Add the missing member too, on the line below `transync-openai/` (the roster order this task uses everywhere else), padded to the same 22-column name field:

```text
│   ├── transync-anthropic/   Translator impl; Anthropic Messages API. In-tree but on NO run path — transync-cli depends only on transync + transync-openai (ADR-0002 / DCR-0029)
```

  Fix it here rather than filing it: the line sits inside the block this step already rewrites, and leaving a known-wrong line while editing the one above it is precisely the drift shape this project keeps paying to find. The "no run path" clause is the fact a reader actually needs — it is why `ANTHROPIC_*` environment variables exist in-tree yet change no CLI run — and it matches the wording `CLAUDE.md`'s Project section already uses.
  - `docs/backlog.md`, the `### template-webcomponent-extraction` entry's **Description** line: `` (`htmlseg.rs` tracks `template_depth` specifically to exclude it) `` → `` (`transync-html`'s segment scan tracks `template_depth` specifically to exclude it) `` — the same phrasing Step 10 uses for `per_kind.rs`, and for the same reason: the function that holds that counter is private in the new crate, so naming it as a path would be a lie about reachability. This is a still-**open** deferred item whose Description is present tense and points a reader at a source file by name; after the `git mv` there is no `htmlseg.rs` to open. Only that parenthetical moves — the entry's `**Background:** DCR-0016 / 2026-08-03 design §9` line is a dated citation and stays exactly as written.

  **What deliberately does *not* change, so nobody "finishes the job" later and falsifies a record:** `docs/architecture/mvp-scope.md` keeps its `htmlseg` under the heading `### `transync-syntax` crate split + AST-direct renderer — shipped 2026-08-04 (DCR-0017)`. That sentence is past tense (`the syntax layer (…, `align`, `htmlseg`, …) **became** the workspace's fifth member`) and it is a dated shipped-record entry: on 2026-08-04 the module that moved into `transync-syntax` genuinely was called `htmlseg`, and the member count genuinely was five. Rewriting it would make the record wrong rather than current. Everything else outside this task's document list that still says `htmlseg` is in the same category — ADR-0018, ADR-0003, the DCRs, the archived DCRs, `stub-manifest.md`, `status.md`'s dated resolution notes, `open-issues-archive.md`, `implementation-slice-checklists.md`, `CHANGELOG.md`'s released entries, the three `htmlseg` mentions inside `phase-state.yaml`'s dated `project.notes` blocks (all of them `LANDED 2026-08-04` prose — Task 7 Step 4 appends to that block and touches none of them), the earlier dated plans and specs, and the git-ignored `docs/investigation/` bundle. Spec §12's D10 note is the standing rule: dated records are append-only and are not rewritten; the crate doc written in Step 4 is where a reader who greps `htmlseg` lands.

- [ ] **Step 16: Verify — the characterization gate.**
```bash
cargo fmt --all
cargo fmt --all -- --check > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-fmt.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-fmt.txt
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-wasm.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-cli.txt
```
Read all five files as a separate step. Expected: `CARGO_EXIT=0` in every one, no `FAILED` line anywhere. The wasm gate's string is the two-package one and did not change.

- [ ] **Step 17: Verify — zero fixture or expectation edits.** This is the wave's acceptance gate and it is a *checkable step*, not a hope:
```bash
git status --porcelain -- 'crates/*/tests/fixtures' 'web/tests' > /Volumes/Temp/claude/ti490d97-wave0/gate/t1-fixtures.txt
git diff --stat HEAD -- 'crates/*/tests/scenarios' >> /Volumes/Temp/claude/ti490d97-wave0/gate/t1-fixtures.txt
```
Expected: the file is **empty**. Any line at all means a fixture or a scenario expectation moved — STOP and report; the refactor changed behaviour.

- [ ] **Step 18: Commit.**
```bash
git add crates/transync-html Cargo.toml Cargo.lock \
  crates/transync-syntax/Cargo.toml crates/transync-syntax/src/lib.rs \
  crates/transync-syntax/src/outcome.rs crates/transync-syntax/src/regen.rs \
  crates/transync-syntax/src/render.rs crates/transync-syntax/src/parser.rs \
  crates/transync-core/Cargo.toml crates/transync-core/src/unit/payload.rs \
  crates/transync-core/src/validate.rs crates/transync-core/src/validate/per_kind.rs \
  crates/transync/tests/public_surface.rs crates/transync/tests/workspace_publication.rs \
  crates/transync/tests/docs_ownership_drift.rs scripts/lib/rustdoc-gate.sh \
  docs/index.md docs/architecture/contracts.md docs/architecture/source-of-truth-table.md \
  docs/architecture/README.md docs/implementation/module-map.md \
  docs/project/release-checklist.md docs/Developer_Guide.md docs/backlog.md \
  docs/superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md
git commit -m "refactor: the HTML mechanics get their own crate, and it is called transync-html

htmlseg.rs moves verbatim out of transync-syntax into a seventh workspace
member. No re-export alias: a hidden one would let every old path keep
compiling and the migration would never finish. transync-syntax drops
lol_html and htmlize; transync-syntax and transync-core both gain the new
member. Every weld the spec's §5 names rides this commit — the forbidden
path segment, the seven-member publication roster in dependency order, the
workspace dependency entry with its two unwelded comments, the ownership
drift roots, the rustdoc gate list, and the seven documents.

Behaviour is unchanged and the suite proves it: green with zero fixture or
expectation edits.

Spec: docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md §5
TRACE: ti 490d97 wave 0

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
Expected: the pre-commit hook prints its four lines (`cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`, `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p transync-html …`) and the commit succeeds. Never `--no-verify`.

---

### Task 2: The token-stream pin — freeze what the shipped consumers see, before touching the scanner

**Files:**
- Create: `crates/transync-html/tests/token_stream_pin.rs`
- Create (generated in Step 3): `crates/transync-html/tests/goldens/token-stream.txt`, `crates/transync-html/tests/goldens/balanced/*.txt`

**Interfaces:**
- Consumes from Task 1: `transync_html::{TagToken, balance_fragment, scan_tags, tag_inventory}`, with `TagToken` holding exactly `Open { name: String, self_closing: bool }` and `Close { name: String, span: (usize, usize) }`.
- Produces for Task 4: goldens generated from the **pre-`Skip`** code. Task 4's acceptance is that these files are byte-identical afterwards and appear in no diff.
- The pin's projection helper is deliberately written with `if let` rather than `match` so that **adding `TagToken::Skip` in Task 4 requires no edit to this file** — the projection's contract is "the two shapes the shipped consumers see", and a third variant must fall outside it by construction.

- [ ] **Step 1: Write the pin.** `crates/transync-html/tests/token_stream_pin.rs`:
```rust
//! The token-stream pin (spec 2026-08-20 §5).
//!
//! Wave 0 gives `TagToken::Open` a `span` and adds a `TagToken::Skip`
//! variant. Both are meant to be **inert** for the two consumers that ship
//! today — `tag_inventory` (validation layer 3) and `balance_fragment` (the
//! render path). "Meant to be" is not evidence, so this file is the evidence:
//! the `Open`/`Close` projection of `scan_tags`, `tag_inventory`'s output and
//! `balance_fragment`'s output are frozen against goldens generated from the
//! code as it stood BEFORE the token change, over the repository's existing
//! HTML-bearing fixtures.
//!
//! The goldens are regenerated only by running the `#[ignore]`d
//! `regenerate_goldens` below deliberately. **If the pin goes red, the token
//! change was not inert — do not re-bless the golden, find out which region
//! moved.**
//!
//! TRACE: ti 490d97 wave 0
//! TRACE: DCR-0032

use std::path::{Path, PathBuf};

use transync_html::{TagToken, balance_fragment, scan_tags, tag_inventory};

/// Repo root, derived from this crate's manifest dir
/// (`crates/transync-html` -> `../..`), the idiom `docs_index_drift.rs` uses.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens")
}

/// The repository's existing HTML-bearing fixtures. They are Markdown
/// documents; `scan_tags` is a byte scanner and does not care.
const FIXTURES: &[(&str, &str)] = &[
    (
        "scn-15-html-blocks",
        "crates/transync/tests/fixtures/scn-15-html-blocks.md",
    ),
    ("scn-14-full", "crates/transync/tests/fixtures/scn-14-full.md"),
    (
        "reader-honesty",
        "crates/transync/tests/fixtures/reader-honesty.md",
    ),
];

/// The regions the R0002-* / R0003-* incidents were about, plus the two
/// doctype spellings. Every one tokenizes the same way before and after
/// wave 0, which is what makes this golden a valid before/after comparison.
///
/// Two of them — `doctype-upper` and `doctype-lower` — *are* bare `<!…>`
/// regions, i.e. exactly the region where wave 0 changes tokenization. They
/// still cannot move this projection, because their interiors hold no
/// tag-shaped bytes: before wave 0 the scanner walked past them emitting
/// nothing (`!` fails the tag-open state's ASCII-letter test), after it they
/// emit one `Skip`, and the projection below drops both. What the corpus
/// deliberately does **not** contain is a bare `<!…>` whose interior *does*
/// hold tag-shaped bytes — `<! <div> >`, where the `<div>` used to tokenize
/// as markup and no longer does. That is the one real behaviour change in
/// the wave, so it is pinned by its own in-crate test rather than here
/// (`tag_shaped_bytes_inside_a_bogus_comment_are_not_markup`).
///
/// The `<![CDATA[…]]>` entries *do* hold tag-shaped bytes, and that is fine:
/// the CDATA branch, with both terminator modes, shipped with R0003-0067
/// long before this wave, so those regions were already skipped and their
/// tokenization does not move either.
const EDGE_CASES: &[(&str, &str)] = &[
    ("cdata-foreign", "<svg><text><![CDATA[<b>bold</b>]]></text></svg>"),
    ("cdata-html", "<p><![CDATA[</b>]]></p>"),
    (
        "rcdata-textarea",
        "<textarea>Use </p> to close a paragraph</textarea>",
    ),
    ("rcdata-title", "<title>a < b </i> c</title>"),
    ("script-raw-text", "<script>if (a < b) { s = \"</div>\"; }</script>"),
    ("digit-tag", "Rows <2026 total> here"),
    ("colon-name", "<div:x>y"),
    ("unquoted-slash", "<a href=https://example.com/>Example</a>"),
    ("optional-end-tags", "<ul><li>a<li>b"),
    ("comment", "<div><!-- hidden -->visible</div>"),
    ("doctype-upper", "<!DOCTYPE html>\n<p>after</p>"),
    ("doctype-lower", "<!doctype html>\n<p>after</p>"),
];

/// Every corpus entry as `(name, source)`, fixtures first.
fn corpus() -> Vec<(String, String)> {
    let root = repo_root();
    let mut out = Vec::new();
    for (name, rel) in FIXTURES {
        let path = root.join(rel);
        let src = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{rel} should be readable ({}): {e}", path.display()));
        out.push(((*name).to_string(), src));
    }
    for (name, src) in EDGE_CASES {
        out.push(((*name).to_string(), (*src).to_string()));
    }
    out
}

/// The stream every shipped consumer sees: `Open` and `Close`, nothing else.
///
/// Written as an `if let` chain, not a `match`, on purpose. This projection's
/// contract is "the two shapes that existed before wave 0", so a third
/// variant must fall outside it **without an edit to this file** — otherwise
/// the pin's own source would move in the same commit as the change it
/// exists to police. `Open.span` is deliberately unread here for the same
/// reason; Task 4's `an_open_tag_span_slices_back_to_its_own_bytes` covers it.
fn stream(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in scan_tags(html) {
        if let TagToken::Open {
            name, self_closing, ..
        } = &token
        {
            out.push(format!("O:{name}:{self_closing}"));
        } else if let TagToken::Close { name, span, .. } = &token {
            out.push(format!("C:{name}@{}..{}", span.0, span.1));
        }
    }
    out
}

/// The token golden, rendered from the corpus.
fn render_token_golden() -> String {
    let mut out = String::new();
    for (name, src) in corpus() {
        out.push_str(&format!("### {name}\n"));
        out.push_str(&format!("inventory: {}\n", tag_inventory(&src).join("|")));
        out.push_str(&format!("stream: {}\n", stream(&src).join("|")));
    }
    out
}

#[test]
fn the_open_close_stream_and_tag_inventory_are_byte_identical_to_the_golden() {
    let path = goldens_dir().join("token-stream.txt");
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} should be readable: {e}. Generate it with: cargo test -p \
             transync-html --test token_stream_pin -- --ignored --test-threads=4",
            path.display()
        )
    });

    // Anti-vacuity, both directions: a golden that lost its sections, or one
    // that never recorded a token, would make the compare pass for the wrong
    // reason.
    assert_eq!(
        expected.lines().filter(|l| l.starts_with("### ")).count(),
        FIXTURES.len() + EDGE_CASES.len(),
        "the golden does not cover the whole corpus — it was generated \
         against a different FIXTURES/EDGE_CASES set"
    );
    assert!(
        expected.lines().any(|l| l.starts_with("stream: O:")),
        "the golden records no open tag at all — the projection stopped \
         seeing tokens"
    );

    assert_eq!(
        render_token_golden(),
        expected,
        "the Open/Close stream or tag_inventory moved. Wave 0's token changes \
         (Open.span, TagToken::Skip) are meant to be inert for both shipped \
         consumers; this says they are not. Do NOT re-bless the golden."
    );
}

#[test]
fn balance_fragment_output_is_byte_identical_to_the_golden() {
    let dir = goldens_dir().join("balanced");
    let entries = corpus();
    assert!(!entries.is_empty(), "the corpus is empty");
    for (name, src) in entries {
        let path = dir.join(format!("{name}.txt"));
        let expected = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
        assert_eq!(
            balance_fragment(&src),
            expected,
            "balance_fragment moved for `{name}` — the render path's output \
             is not what it was before the token change"
        );
    }
}

/// Regenerate both goldens. `#[ignore]`d so no ordinary run rewrites a pin.
/// Run it deliberately, and only when the corpus itself changes — never to
/// turn a red pin green.
#[test]
#[ignore = "writes the goldens; run explicitly with --ignored"]
fn regenerate_goldens() {
    let dir = goldens_dir();
    std::fs::create_dir_all(dir.join("balanced")).expect("goldens/balanced should be creatable");
    std::fs::write(dir.join("token-stream.txt"), render_token_golden())
        .expect("token-stream.txt should be writable");
    for (name, src) in corpus() {
        std::fs::write(
            dir.join("balanced").join(format!("{name}.txt")),
            balance_fragment(&src),
        )
        .expect("a balanced golden should be writable");
    }
}
```

- [ ] **Step 2: Run it and see it fail — the goldens do not exist yet.**
```bash
cargo test -p transync-html --test token_stream_pin -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t2-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t2-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` and a `test result: FAILED.` line reading `0 passed; 2 failed; 1 ignored` (the generator is the ignored one), with two panics: `…/tests/goldens/token-stream.txt should be readable: No such file or directory (os error 2)` and `…/tests/goldens/balanced/scn-15-html-blocks.txt should be readable: No such file or directory (os error 2)`.

- [ ] **Step 3: Generate the goldens from the pre-`Skip` code.**
```bash
cargo test -p transync-html --test token_stream_pin -- --ignored --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t2-bless.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t2-bless.txt
```
Expected: `CARGO_EXIT=0` and a summary of the shape `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out`. Read that shape carefully, because it is not the shape of the other runs in this plan: `--ignored` runs **only** the ignored tests, so the two ordinary pins are reported as `filtered out`, not as `ignored`, and the `ignored` count is `0` precisely *because* the one `#[ignore]`d test ran. A summary reading `2 ignored` here would mean the generator did **not** run and no golden was written.

- [ ] **Step 4: Inspect the goldens by hand — this is the one moment their content is a decision rather than a fact.**
```bash
cat crates/transync-html/tests/goldens/token-stream.txt
ls crates/transync-html/tests/goldens/balanced/
```
Expected: 15 `### ` sections (3 fixtures + 12 edge cases); the `doctype-upper` and `doctype-lower` sections read `inventory: p|/p` and `stream: O:p:false|C:p@…` — the doctype itself contributes **nothing**, which is the pre-change fact the `Skip` variant is about to make visible without changing. 15 files under `balanced/`.

- [ ] **Step 5: Run it and see it pass.**
```bash
cargo test -p transync-html -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t2-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t2-green.txt
```
Expected: `CARGO_EXIT=0`, and the `token_stream_pin` line reads `test result: ok. 2 passed; 0 failed; 1 ignored`.

- [ ] **Step 6: Lint and format.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t2-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t2-clippy.txt
```
Expected: `CARGO_EXIT=0`.

- [ ] **Step 7: Commit.**
```bash
git add crates/transync-html/tests
git commit -m "test(transync-html): the token stream gets a pin before the scanner gets a variant

Open/Close projection, tag_inventory output and balance_fragment output are
frozen against goldens generated from the code as it stands NOW — before
Open gains a span and TagToken gains Skip. The projection is an if-let chain,
not a match, so the third variant falls outside it without editing the pin:
the instrument does not move in the same commit as the thing it measures.

Corpus: the three existing HTML-bearing fixtures plus the twelve regions the
R0002/R0003 incidents were about, plus both doctype spellings.

TRACE: ti 490d97 wave 0

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
Expected: the pre-commit hook's four gates pass and the commit succeeds.

---

### Task 3: `BlankLinePolicy` — the CommonMark `u8` leaves the crate's spine

**Files:**
- Modify: `crates/transync-html/src/lib.rs`
- Modify: `crates/transync-syntax/src/regen.rs`, `crates/transync-core/src/validate.rs`

**Interfaces:**
- Consumes from Task 1: `transync_html::splice(block: &str, translated: &[String], block_type: u8) -> Result<String, String>`, and the private `collapse_blank_lines`.
- Produces for every later task and wave:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum BlankLinePolicy { Collapse, Keep }
  impl BlankLinePolicy {
      pub fn from_commonmark_html_block_type(block_type: u8) -> Self;
  }
  pub fn splice(block: &str, translated: &[String], blank_lines: BlankLinePolicy)
      -> Result<String, String>;
  ```
- **MEASURED and load-bearing:** today's `matches!(block_type, 6 | 7)` is what runs `collapse_blank_lines`; type 6 collapse damages `<pre>` content and type 1 does not. Both Markdown call sites must keep passing exactly what they pass today, via the constructor — the mapping is not re-decided here, only relocated.

- [ ] **Step 1: Write the failing test for the constructor.** Append to `crates/transync-html/src/lib.rs`, after the existing `mod extract_tests` block:
```rust
// Spec 2026-08-20 §5: the CommonMark blank-line rule has exactly one home.
#[cfg(test)]
mod policy_tests {
    use super::*;

    /// `matches!(block_type, 6 | 7)` is what `splice` computed inline before
    /// wave 0, and this constructor is now its only home. Every other type —
    /// including the `0` a non-CommonMark host has no value for, and anything
    /// outside CommonMark's 1–7 domain — keeps its blank lines.
    #[test]
    fn the_commonmark_mapping_is_six_and_seven_and_nothing_else() {
        for t in 0u8..=7 {
            let expected = if t == 6 || t == 7 {
                BlankLinePolicy::Collapse
            } else {
                BlankLinePolicy::Keep
            };
            assert_eq!(
                BlankLinePolicy::from_commonmark_html_block_type(t),
                expected,
                "block type {t}"
            );
        }
        assert_eq!(
            BlankLinePolicy::from_commonmark_html_block_type(200),
            BlankLinePolicy::Keep,
            "out of CommonMark's domain entirely: Keep, never a panic"
        );
    }
}
```

- [ ] **Step 2: Run it and see it fail.**
```bash
cargo test -p transync-html -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t3-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t3-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` and a compile failure — `error[E0433]: failed to resolve: use of undeclared type `BlankLinePolicy`` (a compile error *is* the red; there is nothing to run yet).

- [ ] **Step 3: Add the type and its constructor.** In `crates/transync-html/src/lib.rs`, immediately above `collapse_blank_lines`:
```rust
/// Whether a translated segment's interior blank lines survive the splice.
///
/// The knob exists because the *host format* decides, not the markup: a blank
/// line terminates a CommonMark HTML block of type 6/7 at reparse, splitting
/// the block and breaking its anchor. Nothing else about splicing cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlankLinePolicy {
    /// Interior blank lines in a translated segment are collapsed — the
    /// Markdown-host rule for CommonMark HTML block types 6 and 7.
    Collapse,
    /// Blank lines pass through. Correct for CommonMark types 1–5 (`<pre>`,
    /// `<textarea>` and friends, whose content a collapse would damage) and
    /// for every block of an HTML document, where nothing ever Markdown-
    /// reparses the output.
    Keep,
}

impl BlankLinePolicy {
    /// The one home of the CommonMark mapping: 6 | 7 → [`BlankLinePolicy::Collapse`],
    /// everything else → [`BlankLinePolicy::Keep`]. Byte-for-byte the
    /// `matches!(block_type, 6 | 7)` that `splice` computed inline before ti
    /// 490d97 wave 0.
    ///
    /// The domain is CommonMark's 1–7 plus the `0` a caller with no
    /// CommonMark context has; values outside it resolve to `Keep` rather
    /// than panicking, because the safe arm is the one that touches nothing.
    pub fn from_commonmark_html_block_type(block_type: u8) -> Self {
        if matches!(block_type, 6 | 7) {
            BlankLinePolicy::Collapse
        } else {
            BlankLinePolicy::Keep
        }
    }
}
```

- [ ] **Step 4: Run it and see it pass.**
```bash
cargo test -p transync-html policy -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t3-green1.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t3-green1.txt
```
Expected: `CARGO_EXIT=0`, `test result: ok. 1 passed`.

- [ ] **Step 5: Change `splice`'s signature and the two in-crate uses of the old parameter.** In `crates/transync-html/src/lib.rs`:
  - the signature and the doc line:
```rust
pub fn splice(
    block: &str,
    translated: &[String],
    blank_lines: BlankLinePolicy,
) -> Result<String, String> {
```
  - inside, replace `let collapse = matches!(block_type, 6 | 7);` with:
```rust
    let collapse = matches!(blank_lines, BlankLinePolicy::Collapse);
```
  - in `collapse_blank_lines`'s doc, `` Only [`splice`] for block types 6/7 calls this `` → `` Only [`splice`] under [`BlankLinePolicy::Collapse`] calls this ``.

- [ ] **Step 6: Update the in-crate tests' third argument — assertions untouched.** In `mod splice_tests` and `mod inventory_tests`, every `splice(…, 6)` becomes `splice(…, BlankLinePolicy::from_commonmark_html_block_type(6))` and every `splice(…, 1)` becomes `splice(…, BlankLinePolicy::from_commonmark_html_block_type(1))`. Going *through the constructor* rather than naming the variant is deliberate: it keeps `blank_lines_collapse_for_type_6_but_not_type_1` and `type_1_keeps_blank_lines_inside_a_translated_segment` non-vacuous — they still pin the CommonMark mapping end to end, which is why the spec says these two move with the crate and stay real. **Change no expected value.**

- [ ] **Step 7: Update the two production call sites.**
  - `crates/transync-syntax/src/regen.rs`:
```rust
                match serde_json::from_str::<Vec<String>>(payload).ok().and_then(|segs| {
                    transync_html::splice(
                        source_bytes,
                        &segs,
                        transync_html::BlankLinePolicy::from_commonmark_html_block_type(
                            *block_type,
                        ),
                    )
                    .ok()
                }) {
```
  - `crates/transync-core/src/validate.rs`:
```rust
            Ok(segs) => match transync_html::splice(
                &h.source_bytes,
                &segs,
                transync_html::BlankLinePolicy::from_commonmark_html_block_type(*block_type),
            ) {
```
  Run `cargo fmt --all` afterwards and take the formatter's wrapping.

- [ ] **Step 8: Verify and commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t3-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t3-clippy.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t3-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t3-workspace.txt
git status --porcelain -- 'crates/*/tests/fixtures' > /Volumes/Temp/claude/ti490d97-wave0/gate/t3-fixtures.txt
```
Expected: `CARGO_EXIT=0` in both cargo files; `t3-fixtures.txt` empty.
```bash
git add crates/transync-html/src/lib.rs crates/transync-syntax/src/regen.rs crates/transync-core/src/validate.rs
git commit -m "refactor(transync-html): splice takes a blank-line policy, not a CommonMark u8

The u8 had exactly one effect inside splice — matches!(block_type, 6 | 7)
gating collapse_blank_lines — and it was the last CommonMark fact in a
format-neutral crate. It survives in one place now, the constructor, and both
Markdown call sites reach the same arm through it. The two type-named tests
keep calling the constructor rather than naming a variant, so they still pin
the mapping end to end: type 6 collapses, type 1 does not, and type 1 not
collapsing is measurable only on a NON-identity translation.

TRACE: ti 490d97 wave 0

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `TagToken::Open` gains `span`, `TagToken::Skip` names the passed-over regions

**Files:**
- Modify: `crates/transync-html/src/lib.rs`
- Test: the pin from Task 2 (`crates/transync-html/tests/token_stream_pin.rs`) plus new in-crate tests

**Interfaces:**
- Consumes from Task 1: `pub enum TagToken`, `pub fn scan_tags`, `pub fn tag_inventory`, `pub fn balance_fragment`; from Task 2: the frozen goldens.
- Produces for Tasks 5 and 6, and for wave 3's intake:
  ```rust
  pub enum TagToken {
      Open  { name: String, self_closing: bool, span: (usize, usize) },
      Close { name: String, span: (usize, usize) },
      Skip  { span: (usize, usize) },
  }
  ```
  `span` is `(start, end)`, half-open byte offsets into the `html` argument, covering the whole region including its delimiters. The `Open` span is **already computed** by `scan_tags` today (`let span = (start, j + 1);`, one line above the push) and merely discarded for the `Open` arm — this is a field addition, not new logic.
- **The bogus-comment state is a real tokenizer change, not just a new token.** HTML ends a `<!…>` / `<?…>` at the first `>`, so tag-shaped bytes inside one are not markup; today the scanner steps past `<!` one byte at a time and tokenizes them. The Task 2 corpus *does* contain bare `<!…>` regions — `doctype-upper` and `doctype-lower` — and they are still inert, because their interiors hold no tag-shaped bytes: before the change they produced no token at all, after it one `Skip`, and the pin's projection drops both. The three HTML-bearing fixtures carry a single `<!--` comment and no bare `<!` or `<?` anywhere (verified). So the pin must stay green. **If it goes red, STOP** — the corpus has acquired a bogus comment whose interior *is* tag-shaped, and the change is not inert there.

- [ ] **Step 0: Harden the golden-regeneration hatch before you touch the token stream.** Task 2's review found that `regenerate_goldens` is guarded by `#[ignore]` alone, while this repository already learned that `#[ignore]` is not enough: `regen_prompt_goldens` in `crates/transync-core/src/llm/prompt.rs` **panics** unless `TRANSYNC_REGEN_GOLDENS=1`. Without that interlock a `cargo test -p transync-html -- --include-ignored` silently rewrites the tracked goldens from *current* code and reports green — which is precisely the accident this pin exists to prevent, and it would happen in the very task that changes the token stream. Bring the hatch up to the house standard. In `crates/transync-html/tests/token_stream_pin.rs`, `regenerate_goldens`'s body gains a first statement, and its doc comment gains the invocation:
```rust
/// Regenerate both goldens. `#[ignore]`d **and** env-var interlocked so no
/// ordinary run — including `--include-ignored` — can rewrite a pin. Run it
/// deliberately, and only when the corpus itself changes — never to turn a
/// red pin green:
///
/// `TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4`
#[test]
#[ignore = "writes the goldens; run explicitly with --ignored"]
fn regenerate_goldens() {
    if std::env::var("TRANSYNC_REGEN_GOLDENS").as_deref() != Ok("1") {
        panic!("set TRANSYNC_REGEN_GOLDENS=1 to confirm intentional regeneration");
    }
```
  **Prove the interlock works, in both directions**, and capture each bare-to-file:
```bash
cargo test -p transync-html -- --include-ignored --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-interlock-blocked.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-interlock-blocked.txt
git status --porcelain -- crates/transync-html/tests/goldens >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-interlock-blocked.txt
```
  Expected: `CARGO_EXIT=101` with `regenerate_goldens` panicking on the env-var message, the three pin tests **passing**, and the `git status` line printing **nothing** — the goldens were not rewritten by a run that, before this step, would have rewritten them. That empty `git status` is the finding's proof, not the panic.

  Commit this step **on its own**, before the token change, so the hardening is not entangled with the change it protects against. Message subject: `test(transync-html): the golden hatch gets the interlock the prompt goldens already had`.

- [ ] **Step 1: Write the two failing tests.** Append to `crates/transync-html/src/lib.rs`, after `mod policy_tests`:
```rust
// Spec 2026-08-20 §5: the token stream grows the two things the HTML intake
// needs, and stays inert for the two consumers that ship today.
#[cfg(test)]
mod token_tests {
    use super::*;

    #[test]
    fn an_open_tag_span_slices_back_to_its_own_bytes() {
        let html = "<p>x</p><img src=\"a.png\"/>";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 2, "two open tags: {opens:?}");
        assert_eq!(opens[0].0, "p");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<p>");
        assert_eq!(opens[1].0, "img");
        assert_eq!(&html[opens[1].1.0..opens[1].1.1], "<img src=\"a.png\"/>");
    }

    #[test]
    fn comments_cdata_and_bogus_comments_become_skip_tokens() {
        let comment = "<!-- note --><p>x</p>";
        assert_eq!(skips(comment), vec![(0, 13)]);
        assert_eq!(&comment[0..13], "<!-- note -->");

        // Foreign content: the section ends at `]]>`.
        let foreign = "<svg><![CDATA[<b>]]></svg>";
        let foreign_skips = skips(foreign);
        assert_eq!(foreign_skips.len(), 1);
        assert_eq!(
            &foreign[foreign_skips[0].0..foreign_skips[0].1],
            "<![CDATA[<b>]]>"
        );

        // HTML content: the same bytes are a bogus comment ending at the
        // first `>`.
        let in_html = "<p><![CDATA[</b>]]></p>";
        let html_skips = skips(in_html);
        assert_eq!(html_skips.len(), 1);
        assert_eq!(&in_html[html_skips[0].0..html_skips[0].1], "<![CDATA[</b>");

        // The doctype: `!` fails the tag-open state's ASCII-letter test, so
        // before wave 0 this produced no token AND no skip — the region was
        // stepped over as text. It is a bogus comment, and now it says so.
        let doctype = "<!doctype html>\n<p>x</p>";
        assert_eq!(skips(doctype), vec![(0, 15)]);
        assert_eq!(&doctype[0..15], "<!doctype html>");
    }

    /// The bogus-comment state is a tokenizer change, not just a new token.
    /// Before wave 0 the scanner stepped past `<!` a byte at a time and read
    /// the `<div>` inside as markup — so `balance_fragment` appended a
    /// `</div>` a browser never asked for.
    #[test]
    fn tag_shaped_bytes_inside_a_bogus_comment_are_not_markup() {
        assert!(tag_inventory("<! <div> >").is_empty());
        assert_eq!(balance_fragment("<! <div> >"), "<! <div> >");
        assert!(tag_inventory("<?xml version=\"1.0\"?>").is_empty());
    }

    /// `Skip` reaches neither shipped consumer: `tag_inventory` filters it
    /// out and the balancer ignores it.
    #[test]
    fn skip_tokens_are_invisible_to_both_shipped_consumers() {
        let html = "<!doctype html><div><!-- c -->text</div>";
        assert_eq!(tag_inventory(html), vec!["div", "/div"]);
        assert_eq!(balance_fragment(html), html);
        assert_eq!(skips(html).len(), 2, "the doctype and the comment");
    }

    /// The `Skip` spans of `html`, in document order.
    fn skips(html: &str) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Skip { span } = token {
                out.push(span);
            }
        }
        out
    }
}
```

- [ ] **Step 2: Run them and see them fail.**
```bash
cargo test -p transync-html -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` and at least these two compile errors — `error[E0026]: variant `TagToken::Open` does not have a field named `span`` and `error[E0599]: no variant or associated item named `Skip` found for enum `TagToken``. A compile failure *is* the red; there is nothing to run yet.

- [ ] **Step 3: Add the `span` field and the `Skip` variant.** In `crates/transync-html/src/lib.rs`, the enum becomes:
```rust
pub enum TagToken {
    /// A start tag. `span` covers `<` through `>` inclusive-exclusive.
    Open {
        name: String,
        self_closing: bool,
        span: (usize, usize),
    },
    /// An end tag. `span` covers `</` through `>` inclusive-exclusive.
    Close {
        name: String,
        span: (usize, usize),
    },
    /// A region the scanner recognizes and steps over: a comment, a CDATA
    /// section (either terminator mode), or a bogus comment (`<!…>` / `<?…>`).
    /// It carries no name because it has none — what it carries is the byte
    /// range, which is the thing an intake needs in order to trim the
    /// anonymous runs between elements. Neither `tag_inventory` nor
    /// `balance_fragment` reads it.
    Skip {
        span: (usize, usize),
    },
}
```

- [ ] **Step 4: Emit the new token and the new field in `scan_tags`.** Four edits inside `scan_tags`:
  1. the comment branch:
```rust
        if html[i..].starts_with("<!--") {
            let end = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
            i = end;
            continue;
        }
```
  2. the CDATA branch (keep its existing comment block above it verbatim):
```rust
        if html[i..].starts_with("<![CDATA[") {
            let terminator = if foreign_depth > 0 { "]]>" } else { ">" };
            let end = html[i..]
                .find(terminator)
                .map(|p| i + p + terminator.len())
                .unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
            i = end;
            continue;
        }
```
  3. a new branch immediately after the CDATA one, before `let start = i;`:
```rust
        // HTML's bogus-comment state. `<!` that is neither a comment nor a
        // CDATA section, and `<?`, run to the FIRST `>` and are not markup —
        // the same rule R0003-0066 and R0003-0067 applied one region further
        // in. Before wave 0 the scanner had no such state: it stepped past
        // `<!` a byte at a time, so `<!doctype html>` produced no token at
        // all (the `!` fails the tag-open state's ASCII-letter test below)
        // and tag-shaped bytes inside a bogus comment tokenized as markup.
        if html[i..].starts_with("<!") || html[i..].starts_with("<?") {
            let end = html[i..].find('>').map(|p| i + p + 1).unwrap_or(html.len());
            tokens.push(TagToken::Skip { span: (i, end) });
            i = end;
            continue;
        }
```
  4. the `Open` push, which already has `span` in scope from the line `let span = (start, j + 1);`:
```rust
            tokens.push(TagToken::Open {
                name,
                self_closing,
                span,
            });
```

- [ ] **Step 5: Give both in-crate matches an explicit `Skip` arm — no `_ =>`.**
  - `tag_inventory` becomes:
```rust
pub fn tag_inventory(html: &str) -> Vec<String> {
    scan_tags(html)
        .into_iter()
        .filter_map(|t| match t {
            TagToken::Open { name, .. } => Some(name),
            TagToken::Close { name, .. } => Some(format!("/{name}")),
            TagToken::Skip { .. } => None,
        })
        .collect()
}
```
  - in `balance_fragment`'s `for tok in &tokens` match, add as the last arm:
```rust
            // Comments, CDATA and bogus comments are not structure: the
            // balancer must neither open nor close on them.
            TagToken::Skip { .. } => {}
```

- [ ] **Step 6: Run the new tests and see them pass.**
```bash
cargo test -p transync-html --lib -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-green.txt
```
Expected: `CARGO_EXIT=0`, no `FAILED`.

- [ ] **Step 7: Run the pin and prove the change was inert — the golden must not move.**
```bash
cargo test -p transync-html --test token_stream_pin -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-pin.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-pin.txt
git status --porcelain -- crates/transync-html/tests/goldens >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-pin.txt
```
Read the file. Expected: `CARGO_EXIT=0`, `test result: ok. 2 passed`, and **no** `git status` line for the goldens directory — the goldens are byte-identical and untouched. If the pin failed, do **not** run `--ignored` to re-bless: identify which corpus entry moved and why.

- [ ] **Step 8: Verify the workspace and commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-clippy.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t4-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t4-workspace.txt
```
Expected: `CARGO_EXIT=0` in both.
```bash
git add crates/transync-html/src/lib.rs
git commit -m "feat(transync-html): Open carries the span it already computed, and Skip names the regions

scan_tags computed the open tag's span one line above the push and threw it
away for the Open arm; it keeps it now. Skip { span } names the three regions
the scanner already stepped over silently — comments, CDATA in both
terminator modes, and a bogus comment, which is the state that did not exist
at all: `!` fails the tag-open state's letter test, so `<!doctype html>`
produced no token and no skip, and tag-shaped bytes inside a `<!…>` were read
as markup.

Both shipped consumers are unmoved and the pin says so: tag_inventory filters
Skip out, balance_fragment ignores it, and the goldens generated before this
commit are byte-identical after it.

TRACE: ti 490d97 wave 0

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: `element_extents` — the balancer's stack walk, refactored out from under it

**Files:**
- Modify: `crates/transync-html/src/lib.rs`

**Interfaces:**
- Consumes from Task 4: `TagToken::{Open, Close, Skip}` with spans on all three; `is_void`, `is_raw_text`, `implicitly_closes`.
- Produces for wave 3's HTML intake:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct ElementExtent {
      pub name: String,
      pub depth: usize,
      pub open: (usize, usize),
      pub close: Option<(usize, usize)>,
      pub content_end: usize,
  }
  pub fn element_extents(html: &str) -> Vec<ElementExtent>;
  ```
  `depth` is nesting depth at the open tag, 0 at top level. `close` is `None` for an element closed implicitly or unclosed at EOF. `content_end` is the byte offset where the element's content ends: its close tag's start, its implicit closer's start, or `html.len()`.
- **There is exactly one walk.** `balance_fragment` is re-expressed on top of the same private `walk_elements`; a second copy would be a second opinion about HTML structure, the sin the architecture forbids for comrak and forbids here for the same reason. `balance_fragment`'s output does not change — the Task 2 pin and the existing `mod balance_tests` are the proof.

- [ ] **Step 0: Close the `BlankLinePolicy` catch-all before adding another enum to this file.** Task 3's review found that `splice`'s `let collapse = matches!(blank_lines, BlankLinePolicy::Collapse);` hides an implicit `_ => false`: add a third variant and it silently means *keep blank lines*, with no compiler error at the one site that decides the behaviour. Two variants make that harmless today and a trap the moment the enum grows — which is exactly the shape the widened Global Constraint now forbids. In `crates/transync-html/src/lib.rs`, inside `splice`:
```rust
    // Node-level actions, in text-node order: None = leave untouched
    // (dropped node OR identity translation); Some(text) = replace.
    //
    // Exhaustive on purpose (ti 490d97 wave 0 Task 3 review): `matches!`
    // would hide a third variant behind an implicit `_ => false` and make it
    // mean Keep by accident. A new policy must stop the compiler here.
    let collapse = match blank_lines {
        BlankLinePolicy::Collapse => true,
        BlankLinePolicy::Keep => false,
    };
```
  **Prove it is behaviour-preserving rather than asserting it**, and capture bare-to-file:
```bash
cargo test -p transync-html -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-step0.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-step0.txt
git diff --stat HEAD -- crates/transync-html/tests/goldens >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-step0.txt
```
  Expected: `CARGO_EXIT=0`, every `splice` test still passing with **no expected value edited**, and the goldens line printing nothing. Commit this step **on its own**, before `element_extents`: subject `refactor(transync-html): the blank-line policy is matched exhaustively, not sampled`.

- [ ] **Step 1: Write the failing tests.** Append to `crates/transync-html/src/lib.rs`, after `mod token_tests`:
```rust
// Spec 2026-08-20 §5: one stack walk, two readers.
#[cfg(test)]
mod extent_tests {
    use super::*;

    #[test]
    fn extents_are_in_source_order_with_depth_and_both_tag_spans() {
        let html = "<div><p>a</p><p>b</p></div>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div", "p", "p"]);
        assert_eq!(ex[0].depth, 0);
        assert_eq!(ex[1].depth, 1);
        assert_eq!(ex[2].depth, 1);
        assert_eq!(&html[ex[1].open.0..ex[1].open.1], "<p>");
        let inner_close = ex[1].close.expect("the first <p> is closed");
        assert_eq!(&html[inner_close.0..inner_close.1], "</p>");
        assert_eq!(&html[ex[1].open.1..ex[1].content_end], "a");
        let outer_close = ex[0].close.expect("the div is closed");
        assert_eq!(&html[outer_close.0..outer_close.1], "</div>");
        assert_eq!(&html[ex[0].open.1..ex[0].content_end], "<p>a</p><p>b</p>");
    }

    #[test]
    fn an_implicitly_closed_element_has_no_close_span_and_ends_at_its_closer() {
        let html = "<ul><li>a<li>b</ul>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["ul", "li", "li"]);
        assert!(ex[1].close.is_none(), "the first <li> is closed implicitly");
        assert_eq!(&html[ex[1].open.1..ex[1].content_end], "a");
        assert!(ex[2].close.is_none(), "the second <li> is closed by </ul>");
        assert_eq!(&html[ex[2].open.1..ex[2].content_end], "b");
    }

    #[test]
    fn an_unclosed_element_runs_to_end_of_input() {
        let html = "<div><span>x";
        let ex = element_extents(html);
        assert_eq!(ex.len(), 2);
        assert!(ex[0].close.is_none());
        assert_eq!(ex[0].content_end, html.len());
        assert!(ex[1].close.is_none());
        assert_eq!(ex[1].content_end, html.len());
    }

    #[test]
    fn void_and_self_closing_elements_mint_no_extent() {
        assert!(element_extents("<br><img src=\"a.png\"/>").is_empty());
        // …but a self-closing raw-text start tag DOES open one, exactly as
        // the balancer treats it: HTML ignores `/` there (DCR-0016 Part D).
        let ex = element_extents("<textarea/>");
        assert_eq!(ex.len(), 1);
        assert_eq!(ex[0].name, "textarea");
        assert!(ex[0].close.is_none());
    }

    #[test]
    fn an_orphan_close_tag_mints_no_extent() {
        let ex = element_extents("</details><p>x</p>");
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["p"], "the orphan closes nothing and opens nothing");
    }

    #[test]
    fn skipped_regions_are_not_structure() {
        let ex = element_extents("<!doctype html><div><!-- c -->x</div>");
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div"]);
    }

    /// The walk is SHARED (spec §5): the balancer's "still open at EOF" set
    /// and the extents' "no close span" set are the same computation, and a
    /// divergence here means a second copy crept in.
    #[test]
    fn the_walk_is_shared_with_the_balancer() {
        let html = "<div><span>x";
        // Bind first: `element_extents` returns a Vec, and collecting
        // `&str`s out of a temporary drops it while they still borrow it
        // (E0716). The other six tests in this module already bind; this one
        // did not, and it was corrected during execution (ti 490d97 wave 0
        // Task 5, deviation 1) rather than left as a plan-shaped compile error.
        let ex = element_extents(html);
        let unclosed: Vec<&str> = ex
            .iter()
            .filter(|e| e.close.is_none())
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(unclosed, vec!["div", "span"]);
        assert_eq!(balance_fragment(html), "<div><span>x</span></div>");
    }
}
```

- [ ] **Step 2: Run them and see them fail.**
```bash
cargo test -p transync-html --lib -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-red.txt
```
Expected: `CARGO_EXIT=101` and `error[E0425]: cannot find function `element_extents` in this scope`, once per call site in the new module. A compile failure *is* the red.

- [ ] **Step 3: Add the type and the shared walk.** In `crates/transync-html/src/lib.rs`, immediately above the existing `balance_fragment`:
```rust
/// One element the [`element_extents`] walk found, in source order by its
/// open tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementExtent {
    /// Lowercased tag name, exactly as [`scan_tags`] reports it.
    pub name: String,
    /// Nesting depth at the open tag; 0 at top level.
    pub depth: usize,
    /// Byte range of the open tag, `<` through `>`.
    pub open: (usize, usize),
    /// Byte range of the end tag, or `None` when the element was closed
    /// implicitly (HTML's optional end tags) or left unclosed at EOF.
    pub close: Option<(usize, usize)>,
    /// Byte offset where the element's content ends: its end tag's start, its
    /// implicit closer's start, or `html.len()`.
    pub content_end: usize,
}

/// The result of the one stack walk over [`scan_tags`]' token stream.
struct Walk {
    extents: Vec<ElementExtent>,
    /// Orphan close tags, in document order — the spans the balancer deletes.
    orphan_closes: Vec<(usize, usize)>,
    /// Names still open at EOF, outermost first.
    unclosed: Vec<String>,
}

/// THE stack walk. [`element_extents`] reads its structure and
/// [`balance_fragment`] reads its repairs; there is exactly one of it, for
/// the same reason there is exactly one Markdown parser — a second walk would
/// be a second opinion about HTML structure, and the two would drift.
fn walk_elements(html: &str) -> Walk {
    let tokens = scan_tags(html);
    let mut extents: Vec<ElementExtent> = Vec::new();
    // (name, index into `extents`) for each element still open.
    let mut open_stack: Vec<(String, usize)> = Vec::new();
    let mut orphan_closes: Vec<(usize, usize)> = Vec::new();

    for tok in &tokens {
        match tok {
            TagToken::Open {
                name,
                self_closing,
                span,
            } => {
                // Before the push, and for void elements too: `<hr>` closes a
                // paragraph it never joins (R0002-0061).
                let implied = implicitly_closes(name);
                while open_stack
                    .last()
                    .is_some_and(|(top, _)| implied.contains(&top.as_str()))
                {
                    let (_, idx) = open_stack.pop().expect("just inspected the top");
                    extents[idx].content_end = span.0;
                }
                // HTML ignores `/` on raw-text/RCDATA start tags: `<textarea/>`
                // still opens RCDATA and swallows every later sibling
                // (DCR-0016 Part D).
                if !is_void(name) && (!self_closing || is_raw_text(name)) {
                    extents.push(ElementExtent {
                        name: name.clone(),
                        depth: open_stack.len(),
                        open: *span,
                        close: None,
                        content_end: html.len(),
                    });
                    open_stack.push((name.clone(), extents.len() - 1));
                }
            }
            TagToken::Close { name, span } => {
                if let Some(pos) = open_stack.iter().rposition(|(t, _)| t == name) {
                    // Everything above `pos` is closed implicitly by this tag.
                    for (_, idx) in open_stack.drain(pos + 1..) {
                        extents[idx].content_end = span.0;
                    }
                    let (_, idx) = open_stack.pop().expect("rposition found it");
                    extents[idx].close = Some(*span);
                    extents[idx].content_end = span.0;
                } else {
                    // Orphan close tag: dropping it is what keeps the sync
                    // wrapper's own `</div>` safe (spec 2026-08-03 §3.4).
                    orphan_closes.push(*span);
                }
            }
            // Comments, CDATA and bogus comments are not structure.
            TagToken::Skip { .. } => {}
        }
    }

    let unclosed = open_stack.into_iter().map(|(name, _)| name).collect();
    Walk {
        extents,
        orphan_closes,
        unclosed,
    }
}

/// Every element in `html`, in source order by open tag, with its nesting
/// depth, both tag spans and its content end.
///
/// The classification of what those elements *mean* is not here — that is
/// `transync-syntax`'s intake. This is the structure the intake walks.
pub fn element_extents(html: &str) -> Vec<ElementExtent> {
    walk_elements(html).extents
}
```

- [ ] **Step 4: Re-express `balance_fragment` on the shared walk.** Replace the whole body of `balance_fragment` (keep its existing doc comment verbatim — every word of it still holds) with:
```rust
pub fn balance_fragment(html: &str) -> String {
    let walk = walk_elements(html);

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    for (s, e) in walk.orphan_closes {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    for name in walk.unclosed.iter().rev() {
        out.push_str("</");
        out.push_str(name);
        out.push('>');
    }
    out
}
```

- [ ] **Step 5: Run the new tests and the whole existing balance suite.**
```bash
cargo test -p transync-html --lib -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-green.txt
```
Expected: `CARGO_EXIT=0`. Every test in `mod balance_tests` passes **with no assertion edited** — including `optional_end_tags_do_not_accumulate_phantom_closes`, `cdata_sections_are_not_scanned_for_tags` and `a_namespaced_tag_name_is_scanned_whole`, which are the ones a subtly-different walk would break.

- [ ] **Step 6: Run the pin — `balance_fragment`'s output must still be byte-identical.**
```bash
cargo test -p transync-html --test token_stream_pin -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-pin.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-pin.txt
git status --porcelain -- crates/transync-html/tests/goldens >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-pin.txt
```
Expected: `CARGO_EXIT=0` and no `git status` line for the goldens.

- [ ] **Step 7: Verify the workspace and commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-clippy.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t5-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t5-workspace.txt
git add crates/transync-html/src/lib.rs
git commit -m "feat(transync-html): element_extents is the balancer's walk, lifted out — not a second one

walk_elements is now THE stack walk; element_extents reads its structure and
balance_fragment reads its repairs. Lifting rather than copying is the point:
a second walk would be a second opinion about HTML structure, which is the
thing the architecture refuses for Markdown and refuses here for the same
reason. wave 3's intake consumes the extents for element boundaries and the
token stream for the runs between them.

balance_fragment's output is unchanged — mod balance_tests passes with no
assertion edited, and the token-stream pin's balanced goldens are
byte-identical.

TRACE: ti 490d97 wave 0

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: `strip_reserved_sync_attrs` — wave 1's render half lands in its final home

**Files:**
- Modify: `crates/transync-html/src/lib.rs`

**Interfaces:**
- Consumes from Task 4: `TagToken::Open { span, .. }` — the strip is the first consumer of the new field.
- Produces for wave 1 (OI-0035's render half) and wave 6 (the HTML pane derivation):
  ```rust
  pub fn strip_reserved_sync_attrs(html: &str) -> Cow<'_, str>;
  ```
  It removes, case-insensitively, the six reserved attribute names — `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`, `data-skipped` — from element **open tags** only, taking each removed attribute's leading whitespace with it. It borrows when nothing matched.
- The function ships here with its own tests and **no call site yet**; wiring it into `render.rs`'s `BlockKind::Html` success arm is wave 1's job. That ordering is deliberate: the spec reversed an earlier draft that put OI-0035 first, because the OI's render half calls this function, and landing OI-0035 first would mean writing it in `transync-syntax` and moving it one commit later.
- Attribute *values* and text content are never inspected — only names, and only inside an `Open` token's span. RCDATA and comment content therefore cannot be reached, because `scan_tags` never tokenizes them.

- [ ] **Step 0: Say the two things `ElementExtent`'s doc enforces but does not state.** Task 5's review found the struct documents `name` / `depth` / `open` / `close` / `content_end` while leaving two behaviours of the walk unwritten — and **wave 6 is told, in as many words, to "read the landed `ElementExtent` doc … and adjust to the landed contract"**, with its wrapper rule resting on the first of them. A downstream plan reading this doc today gets a contract that is narrower than the code. Two doc-only edits in `crates/transync-html/src/lib.rs`, no behaviour change:

  1. **Voids and self-closing tags mint no extent.** The rule lives only in `walk_elements`' guard (`if !is_void(name) && (!self_closing || is_raw_text(name))`) and in `extent_tests::void_and_self_closing_elements_mint_no_extent`. Add it to the struct's own doc, immediately after the first sentence:
```rust
/// One element the [`element_extents`] walk found, in source order by its
/// open tag.
///
/// **Not every tag mints one.** A void element (`img`, `br`, `hr`, …) and a
/// self-closing tag outside raw-text/RCDATA are never pushed onto the walk's
/// stack, so they produce **no** `ElementExtent` at all — they have no content
/// and nothing to close. A consumer that needs "the element around these
/// bytes" must handle the empty case rather than assuming one extent per tag.
/// (ti 490d97 wave 0 Task 5 review; wave 6's pane derivation depends on it —
/// a block whose only element is a lone `<img>` has zero extents, which is why
/// it takes the transparent wrapper rather than self-injection.)
```

  2. **`close: None` is wider than "optional end tags or EOF".** The drain branch also assigns `None` to elements closed by **mis-nesting recovery** — `<b><i></b>` leaves `i` with `close: None`, and `i` has no optional end tag. A reader could otherwise infer `close: None ⇒ optional-end-tag element`, which is false. Widen the field's parenthetical:
```rust
    /// Byte range of the end tag, or `None` when the element was closed
    /// implicitly (HTML's optional end tags, or mis-nesting recovery — a
    /// `</b>` that closes an open `<i>` beneath it) or left unclosed at EOF.
    pub close: Option<(usize, usize)>,
```

  **This is documentation only — no assertion, fixture or golden may move.** Prove it and capture:
```bash
cargo test -p transync-html -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-step0.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-step0.txt
git diff --stat HEAD -- crates/transync-html/tests/goldens >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-step0.txt
```
  Expected: `CARGO_EXIT=0` with the same test count Task 5 left (56), and the goldens line printing nothing. Commit this step **on its own**, before `strip_reserved_sync_attrs`: subject `docs(transync-html): ElementExtent says the two things its walk already enforces`.

- [ ] **Step 1: Write the failing tests.** Append to `crates/transync-html/src/lib.rs`, after `mod extent_tests`:
```rust
// Spec 2026-08-20 §8, OI-0035 route (c), render half.
#[cfg(test)]
mod strip_tests {
    use super::*;

    #[test]
    fn every_reserved_attribute_is_removed_case_insensitively() {
        let html = "<div data-sync-id=\"p-0001\" DATA-Block-Kind='paragraph' data-order=3 \
                    data-fallback=\"none\" data-parent-id=\"x\" data-skipped=\"html-block\">t</div>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div>t</div>");
    }

    #[test]
    fn non_reserved_attributes_and_content_survive_untouched() {
        let html = "<a href=\"https://example.com/?a=1&amp;b=2\" data-sync-id=\"p-0001\" \
                    title=\"data-sync-id\">data-sync-id</a>";
        assert_eq!(
            strip_reserved_sync_attrs(html),
            "<a href=\"https://example.com/?a=1&amp;b=2\" title=\"data-sync-id\">data-sync-id</a>",
            "only NAMES are matched: the value and the text are content"
        );
    }

    #[test]
    fn markup_with_nothing_reserved_is_borrowed_not_rebuilt() {
        let html = "<p class=\"x\">hello</p>";
        assert!(matches!(strip_reserved_sync_attrs(html), Cow::Borrowed(_)));
        assert_eq!(strip_reserved_sync_attrs(html), html);
    }

    #[test]
    fn a_reserved_attribute_inside_rcdata_or_a_comment_is_text_not_markup() {
        // `scan_tags` never tokenizes RCDATA or comment content, so the strip
        // cannot reach in and silently edit what the reader sees — the exact
        // failure R0002-0020 was.
        let textarea = "<textarea><div data-sync-id=\"p-0001\"></textarea>";
        assert_eq!(strip_reserved_sync_attrs(textarea), textarea);
        let comment = "<!-- <div data-sync-id=\"p-0001\"> -->";
        assert_eq!(strip_reserved_sync_attrs(comment), comment);
    }

    #[test]
    fn self_closing_and_close_tags_are_handled() {
        assert_eq!(
            strip_reserved_sync_attrs("<img data-sync-id=\"i-1\" src=\"a.png\"/>"),
            "<img src=\"a.png\"/>"
        );
        // A close tag carries no attributes: nothing to do, nothing to break.
        assert_eq!(strip_reserved_sync_attrs("</div>"), "</div>");
    }

    #[test]
    fn stripping_never_changes_the_tag_inventory() {
        // Attributes do not paint and they are not structure: layer 3's
        // skeleton check must see the same thing before and after.
        let html = "<div data-sync-id=\"p-0001\"><b data-order=\"2\">x</b></div>";
        let stripped = strip_reserved_sync_attrs(html);
        assert_eq!(tag_inventory(&stripped), tag_inventory(html));
        assert_eq!(stripped, "<div><b>x</b></div>");
    }
}
```

- [ ] **Step 2: Run them and see them fail.**
```bash
cargo test -p transync-html --lib -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-red.txt
```
Expected: `CARGO_EXIT=101` and `error[E0425]: cannot find function `strip_reserved_sync_attrs` in this scope`, once per call site in the new module. A compile failure *is* the red.

- [ ] **Step 3: Implement it.** In `crates/transync-html/src/lib.rs`, after `element_extents`:
```rust
/// The reserved sync-attribute namespace: the four `render::attrs` writes,
/// plus `data-skipped` (the placeholder label) and `data-parent-id` (reserved
/// and never emitted, contracts.md §4a). We own this namespace in DOM we
/// mount, and nowhere else.
const RESERVED_SYNC_ATTRS: &[&str] = &[
    "data-sync-id",
    "data-block-kind",
    "data-order",
    "data-fallback",
    "data-parent-id",
    "data-skipped",
];

/// Remove every `RESERVED_SYNC_ATTRS` attribute from `html`'s element open
/// tags, case-insensitively, taking each one's leading whitespace with it.
///
/// (Plain backticks, not an intra-doc link: `RESERVED_SYNC_ATTRS` is private
/// and this fn is `pub`, so a link would trip rustdoc's
/// `private_intra_doc_links` lint — warn-by-default, but the pre-commit
/// rustdoc gate runs `-D warnings` over `transync-html` without
/// `--document-private-items`, which makes it a hard commit block. Same rule
/// wave 3's plan states for `intake::html`. Do not "restore" the link.)
///
/// This is **pane-only** (OI-0035 route (c), render half): strip-then-inject
/// is what makes "ours are the only sync attributes in this DOM" a
/// construction rather than a scan. Published output keeps the author's
/// bytes — their `data-sync-id` is their content — and stripping never
/// changes rendered appearance, because attributes do not paint.
///
/// Only names are matched, and only inside an open tag: attribute values and
/// text are content, and RCDATA / comment interiors are never tokenized by
/// [`scan_tags`] in the first place, so the strip cannot reach into them.
/// Borrows when nothing matched.
pub fn strip_reserved_sync_attrs(html: &str) -> Cow<'_, str> {
    let mut cuts: Vec<(usize, usize)> = Vec::new();
    for token in scan_tags(html) {
        if let TagToken::Open { span, .. } = token {
            collect_reserved_attr_spans(html, span, &mut cuts);
        }
    }
    if cuts.is_empty() {
        return Cow::Borrowed(html);
    }

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    for (s, e) in cuts {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    Cow::Owned(out)
}

/// Byte ranges of the reserved attributes inside one open tag, appended to
/// `out` in ascending order. Walks HTML's attribute states directly rather
/// than reusing [`AttrState`], which answers a different question (where the
/// tag ends).
fn collect_reserved_attr_spans(html: &str, span: (usize, usize), out: &mut Vec<(usize, usize)>) {
    let bytes = html.as_bytes();
    let (start, end) = span;
    // `end` is one past the `>`; never look at the `>` itself.
    let limit = end.saturating_sub(1);

    // Step over `<` and the tag name — `scan_tags` already proved one is here.
    let mut i = start + 1;
    while i < limit && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'-' || bytes[i] == b':') {
        i += 1;
    }

    while i < limit {
        if bytes[i].is_ascii_whitespace() || bytes[i] == b'/' {
            i += 1;
            continue;
        }
        let name_start = i;
        while i < limit && !bytes[i].is_ascii_whitespace() && bytes[i] != b'=' && bytes[i] != b'/' {
            i += 1;
        }
        if i == name_start {
            i += 1; // a stray `=`: not a name, and the walk must not stall
            continue;
        }
        let name = html[name_start..i].to_ascii_lowercase();

        // The optional `= value`, in HTML's three value shapes.
        let mut j = i;
        while j < limit && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        let mut attr_end = i;
        if j < limit && bytes[j] == b'=' {
            j += 1;
            while j < limit && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < limit && (bytes[j] == b'"' || bytes[j] == b'\'') {
                let quote = bytes[j];
                j += 1;
                while j < limit && bytes[j] != quote {
                    j += 1;
                }
                attr_end = (j + 1).min(limit);
            } else {
                while j < limit && !bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                attr_end = j;
            }
        }

        if RESERVED_SYNC_ATTRS.contains(&name.as_str()) {
            // Take the leading whitespace along, so removing an attribute
            // does not leave a double space behind.
            let mut cut_start = name_start;
            while cut_start > start + 1 && bytes[cut_start - 1].is_ascii_whitespace() {
                cut_start -= 1;
            }
            out.push((cut_start, attr_end));
        }
        i = attr_end;
    }
}
```

- [ ] **Step 4: Run them and see them pass.**
```bash
cargo test -p transync-html --lib strip -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-green.txt
```
Expected: `CARGO_EXIT=0`, `test result: ok. 6 passed`.

- [ ] **Step 5: Verify the crate and the workspace.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-clippy.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t6-workspace.txt
git status --porcelain -- 'crates/*/tests/fixtures' crates/transync-html/tests/goldens > /Volumes/Temp/claude/ti490d97-wave0/gate/t6-fixtures.txt
```
Expected: `CARGO_EXIT=0` in both cargo files; `t6-fixtures.txt` empty.

- [ ] **Step 6: Commit.**
```bash
git add crates/transync-html/src/lib.rs
git commit -m "feat(transync-html): strip_reserved_sync_attrs lands in its final home

OI-0035's render half needs it and the HTML pane derivation needs it; both
are later waves, and both would otherwise have written it in transync-syntax
and moved it one commit afterwards. It ships here, tested, with no call site
yet — that is the ordering the spec reversed an earlier draft to get.

Names only, open tags only, case-insensitive, leading whitespace taken along,
borrowed when nothing matched. It cannot reach into RCDATA or a comment
because scan_tags never tokenizes their interiors, and it cannot move the tag
inventory because attributes are not structure.

TRACE: ti 490d97 wave 0
TRACE: OI-0035

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Records — DCR-0032, CHANGELOG, status, phase state

**Files:**
- Create: `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`
- Modify: `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/index.md`
- Modify **on disk only, never staged**: `CLAUDE.md` — it is **untracked in this repository by owner decision** (`.gitignore`: "CLAUDE.md stays ignored by the same owner decision", 2026-08-06 / OI-0020; `git log -- CLAUDE.md` is empty). Editing it is correct and necessary — that file is where every agent reads the repository's description of itself — but `git add CLAUDE.md` **aborts the whole staging command**, and `git add -f` would defy the decision. Edit it, verify it, do not stage it.

**Interfaces:**
- Consumes: everything Tasks 1–6 landed. The DCR number `DCR-0032` is the next free one (DCR-0031 is the highest on disk) and is what the code comments written in Tasks 1–6 already cite — the record must exist or those `TRACE: DCR-0032` lines point at nothing.
- Produces: nothing code-facing. **No ADR in this wave** — ADR-0025 records the twelve decisions and lands in wave 2 (spec §12); wave 0's crate doc carries the name-continuity note in the meantime.

- [ ] **Step 1: Write the DCR.** `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`, starting with OKF frontmatter in the house shape:
```markdown
---
type: DCR
title: The HTML mechanics become a workspace member, and the tag scanner grows the two things an HTML intake needs
description: htmlseg.rs moved verbatim out of transync-syntax into transync-html with no re-export alias; splice's CommonMark u8 became a BlankLinePolicy; TagToken::Open gained the span it already computed and TagToken::Skip named the regions the scanner passed over silently; element_extents was lifted out from under balance_fragment rather than copied; strip_reserved_sync_attrs landed in its final home ahead of its caller.
tags: [change, project-control, DCR-0032]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-20T00:00:00Z
status: stable
---

# DCR-0032: The HTML mechanics become a workspace member
```
Body sections, each of which must state a fact this wave actually established:
  - **Date / Source** — 2026-08-20, ticket `490d97`, wave 0 of the eight in `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12.
  - **Not breaking, and it ships regardless.** Nothing on the §0 surface moves; `transync-html` is tier (c) and the facade re-exports nothing from it. The wave is a pure refactor whose value does not depend on the rest of the feature landing.
  - **What moved, and why no alias.** The verbatim move; the deliberate absence of `pub use transync_html as htmlseg;` — a hidden alias would be the misleadingly-named thin wrapper D3 rejects, and it would let every old path keep compiling so the migration would never finish.
  - **The name, and the two notes it owes** (D10): `transync-html` over `transync-htmlseg` because "segment" under-describes a crate that also owns tokenization, the pairing discipline and element extents. The forward note (structural intake is `transync-syntax`'s, not this crate's) and the backward note (the module formerly called `htmlseg` is this crate) both live in the crate doc, because six shipped records still say `htmlseg` and dated records are not rewritten.
  - **The four non-mechanical changes**, each with what it replaced: the `BlankLinePolicy` signature (the `matches!(block_type, 6 | 7)` now has exactly one home, and every HTML-document splice will run `Keep`); the `Open.span` field (already computed, previously discarded); the `Skip` variant **and the bogus-comment state it required**, which is a real tokenizer change — HTML ends `<!…>` / `<?…>` at the first `>`, so tag-shaped bytes inside one are not markup, and before this wave `<!doctype html>` produced no token and no skip at all; and the visibility opening.
  - **Evidence.** The full workspace suite green with zero fixture or expectation edits; the two-package wasm gate exit 0, string unchanged; `workspace_publication.rs` green on a seven-member roster in dependency order; the new token-stream pin, generated before the token change and byte-identical after it.
  - **Welds that moved with it**, listed: the forbidden path segment, the roster, the workspace dependency entry and its two unwelded comments, the ownership-drift roots and their new per-root floor, the rustdoc-gate crate list, and the seven documents.
  - **Two spec sentences this wave made stale, recorded as owed rather than edited** (§14-style post-implementation items; the spec file is not touched in this wave, following the same convention wave 3's deviation 5 uses). Task 4 gave `<!…>` / `<?…>` a `TagToken::Skip`, so `<!DOCTYPE html>` now *does* produce a scanner token. Both sentences' **conclusions survive** — `tag_inventory` filters `Skip` (`TagToken::Skip { .. } => None`), so those regions remain invisible to the ledger — but the mechanism each states is now wrong, and both are load-bearing where they sit:
    - **§7, the layer-6 twin's check-3 rationale:** "`<!DOCTYPE>` and comments produce no token in `scan_tags`, so a dropped doctype or comment is ledger-invisible". The correct form is *no **ledger entry** — `tag_inventory` filters `Skip`*. This one matters most: it is the stated justification for the one check in the HTML twin that has no Markdown counterpart, so a reader who verifies the mechanism and finds it false has reason to doubt the check. Wave 4's plan carries the corrected wording into its own doc comments and repeats this amendment as owed.
    - **§4, rule T:** "**MEASURED: `<!DOCTYPE html>` produces no token *and no skip* in today's scanner**". This one is a *rationale for adding `Skip`*, not a live claim — "today's scanner" meant the pre-wave-0 scanner, and the sentence is correct about what it describes. It is listed anyway because the phrase reads as present-tense to anyone arriving after this wave; the amendment is a clarification ("in the pre-wave-0 scanner"), not a correction.

  - **Handed forward.** The file-as-module split the spec permits "later"; `ElementExtent`'s exact field shape, refined jointly when wave 3's consumer exists (spec §15 item 2); `strip_reserved_sync_attrs`' call sites, which are wave 1's and wave 6's.

- [ ] **Step 2: CHANGELOG.** The `## [Unreleased]` heading already exists above `## [0.4.0]`, and its reference link is already at the foot of the file (`[Unreleased]: …/compare/6fa4e88…HEAD`) — neither needs creating. What it holds today is a placeholder that these entries would contradict:
```markdown
Nothing yet — v0.4.0 closed the sanctioned breaking window opened after v0.3.0.
The next breaking change needs a new window; additive changes land here as they
come.
```
**Replace those three lines.** Adding entries beneath them leaves the file reading "Nothing yet" immediately above real entries, which is the failure this step exists to avoid. The replacement is one paragraph, keeping the window statement the placeholder was carrying, followed by the entries:
```markdown
v0.4.0 closed the sanctioned breaking window opened after v0.3.0, and
everything below it is additive — nothing on the `transync` facade's §0
surface moves. The next breaking change still needs a new window: wave 2 of
the HTML→HTML feature (spec `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
§12) is the one asking for it, against a future v0.5.0.

### Changed

- **The HTML mechanics moved to their own workspace member, `transync-html`** (ti `490d97` wave 0, DCR-0032). `transync-syntax::htmlseg` is gone with no re-export alias; `transync-syntax` and `transync-core` depend on the new crate, and `transync-syntax` no longer depends on `lol_html` or `htmlize`. Nothing on the `transync` facade's surface moves — the mechanics were always tier (c) engine internals. The publication roster is seven members.
- `transync_html::splice` takes a `BlankLinePolicy` instead of a CommonMark block-type `u8`; `BlankLinePolicy::from_commonmark_html_block_type` is the one surviving home of the `6 | 7` rule.

### Added

- `transync_html::element_extents` / `ElementExtent` — the balancer's stack walk lifted out from under it, for the HTML intake to come.
- `TagToken::Open` carries the byte span the scanner already computed; `TagToken::Skip { span }` names comments, CDATA sections and bogus comments, including the `<!doctype …>` region that previously produced no token at all.
- `transync_html::strip_reserved_sync_attrs` — removes the six reserved sync-attribute names from element open tags (OI-0035 route (c), render half; no call site yet).
```

- [ ] **Step 3: `docs/project/status.md` — two edits, both in sections that already exist.** There is no "current phase" list to append to; the file's landed-work record lives in `## Design Track` and `## Immediate Next Actions`.

  1. `## Design Track`, the `- Closed change records:` line. It opens `Closed change records: DCR-0001 through DCR-0031 (see `design-change-records/`; …` and its parenthetical ends `…, 0031 the 2026-08-16 indented-code normalize-on-translate (ticket `457e51`))`. Two changes on that one line: `DCR-0001 through DCR-0031` → `DCR-0001 through DCR-0032`, and the parenthetical gains a final item before its closing `)`:
```markdown
, 0032 the 2026-08-20 `transync-html` crate extraction + tag-scanner extension (ti `490d97` wave 0)
```
  2. `## Immediate Next Actions`, a new bullet placed immediately **after** the `**Action (roadmap, commissioned 2026-08-06):**` bullet (the one ending "**All six have landed**; the commissioned roadmap is complete. Each larger one opened with a DCR + SL number.") and **before** the `**Action:** Steady-state maintenance alongside:` bullet:
```markdown
- **Action (HTML→HTML feature, ti `490d97`):** ~~wave 0 — move the HTML mechanics into their own member and extend the tag scanner~~ **LANDED 2026-08-20** (DCR-0032): `transync-syntax::htmlseg` left verbatim for `transync-html` with **no** re-export alias, `splice` took a `BlankLinePolicy` in place of the CommonMark `u8`, `TagToken::Open` gained the span `scan_tags` already computed and `TagToken::Skip` named the comment / CDATA / bogus-comment regions, `element_extents` is `balance_fragment`'s stack walk lifted out from under it rather than a second copy, and `strip_reserved_sync_attrs` landed in its final home ahead of its callers. The publication roster is now **seven** members plus `transync-wasm` (`publish = false`). Pure refactor, and the acceptance evidence says so: the workspace suite green with zero fixture or expectation edits, the two-package wasm gate exit 0 with its string unchanged, and a token-stream pin whose goldens were generated before the scanner changed and are byte-identical after. **Waves 1–7 are unstarted** (`docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12; the dependency order is 0 → 2 → {3, 4} → 5 → 6 → 7, with wave 1 parallel to 2–5). **Wave 2 — the IR split — is the one that needs a sanctioned breaking window**, carries the version to `0.5.0-dev`, and blocks every wave after it; it is also the only part of the feature that cannot slip past a window.
```

- [ ] **Step 4: `docs/project/phase-state.yaml` — three edits, and one non-edit that matters.** Read the file first if you like, but do not go looking for a per-wave entry list: **there is none, and there is no per-entry `status:` key either.** The only two `status:` keys in the file are section-level — `design.status: handoff_complete` and `implementation.status: mvp_complete` — and **neither one moves**; wave 0 changes no phase. What the file actually records about landed work is a DCR slug in a list, plus prose in `project.notes`.

  1. `project.last_updated:` — `2026-08-20-v0.4.0-released` → `2026-08-20-ti490d97-wave0`.
  2. `design.closed_change_records:` — append one item, keeping the four-space list indentation, immediately after `    - DCR-0031-indented-code-normalize-on-translate`:
```yaml
    - DCR-0032-transync-html-crate-extraction
```
  `design.open_change_records: []` stays `[]`: the record lands closed, in the same commit as the work it describes.
  3. `project.notes:` is a `|` literal block of unformatted prose — no backticks, no markdown, `->` for arrows. Insert this paragraph after the line ending `Type 1).` (the end of the v0.4.0 block, which closes with the post-0.3.0 roadmap sentence) and before the line beginning `TRACK C WASM RENDER+EDIT DEMO LANDED 2026-08-05`, at the block's existing four-space indent:
```yaml
    TI 490d97 WAVE 0 LANDED 2026-08-20 (DCR-0032): the HTML mechanics left
    transync-syntax for a new workspace member, transync-html, with NO
    re-export alias - splice takes a BlankLinePolicy instead of a CommonMark
    u8, TagToken::Open carries the span scan_tags already computed,
    TagToken::Skip names the comment / CDATA / bogus-comment regions (the
    bogus-comment state is new: <!doctype html> previously produced no token
    at all), element_extents is balance_fragment's stack walk lifted out from
    under it, and strip_reserved_sync_attrs landed ahead of its callers. The
    publication roster is SEVEN members plus transync-wasm (publish = false).
    Pure refactor: workspace suite green with zero fixture or expectation
    edits, two-package wasm gate exit 0 with its string unchanged, goldens
    generated before the scanner change byte-identical after it. Waves 1-7 of
    the HTML->HTML feature are UNSTARTED; wave 2 is the IR split, needs a
    sanctioned breaking window, carries the version to 0.5.0-dev, and blocks
    everything after it.
```

- [ ] **Step 5: `docs/index.md`.** In the record list, add the DCR link beside DCR-0031 **in the shape the neighbouring entries use** — and note that the shape is load-bearing here, because the literal below is what a reviewer diffs against. Every one of the 31 neighbours is `- [DCR-NNNN — Title (ticket X)](path) — description`, with the number, an em dash, the title **and the ticket all inside the link text**. An earlier revision of this step supplied a literal using `DCR-NNNN: Title` that put the ticket nowhere, contradicting its own sentence — which forces an implementer to choose between the prose and the code block, and makes either choice a deviation. If they ever disagree again, the prose wins and the divergence is a finding:
```markdown
- [DCR-0032 — The HTML mechanics become a workspace member, and the record names what stopped being called htmlseg (ticket 490d97)](project/design-change-records/DCR-0032-transync-html-crate-extraction.md) — `htmlseg` becomes `transync-html`, the scanner grows `Open.span` and `TagToken::Skip`, and the balancer's walk becomes `element_extents`.
```

- [ ] **Step 6: `CLAUDE.md` — the file every future agent reads before touching this repository.** It is not a record of what happened; it is a description of what *is*, and after Tasks 1–6 four of its statements are false. Leaving them is worse than leaving a stale changelog entry: a stale record misinforms a reader who went looking, while a stale `CLAUDE.md` misinforms every agent that never went looking at all. Four edits, all in statements this wave made wrong.

  1. **The Project paragraph's opening.** It reads `The repository is a shipped Cargo workspace (v0.3.0), seven members:` — two errors in six words (the workspace has been at `0.4.0` since the release, and this wave adds the eighth member). Replace that clause with:
```markdown
The repository is a shipped Cargo workspace (v0.4.0), eight members:
```

  2. **The member enumeration, which does not mention the crate this wave created.** The list runs `crates/transync-syntax` … `crates/transync-wasm`. Insert `transync-html` at the head of it, before `crates/transync-syntax`, since it is the layer everything else sits on:
```markdown
`crates/transync-html` (the HTML mechanics — tag scanning, the pairing discipline, element extents, text-segment extract/splice, fragment balancing; extracted verbatim from `transync-syntax::htmlseg` by DCR-0032, with **no** re-export alias, and holding no opinion about what a block is),
```

  3. **Two counts inside the same sentence, both now wrong.** The paragraph ends `…the one member carrying `publish = false`, so the publication roster is the other six). Those two — `transync-syntax` and `transync-wasm` — are the only members that compile for `wasm32`.` The roster is **seven**, and `wasm32` is now **three** members, because `transync-syntax` depends on `transync-html` and the standing gate therefore builds it transitively — which is exactly why `transync-html`'s own `Cargo.toml` carries the no-`[features]`, no-workspace-member-dependency prohibition. Replace both:
```markdown
the one member carrying `publish = false`, so the publication roster is the other seven). Those three — `transync-html`, `transync-syntax` and `transync-wasm` — are the only members that compile for `wasm32`; `transync-html` is covered transitively, which is why the standing gate's command string does not name it and must not be changed to.
```

  4. **The `transync-syntax` module split still lists a module that is no longer in the crate.** Delete the `htmlseg` bullet from under *Module split in `crates/transync-syntax`*:
```markdown
  - `htmlseg` — raw-HTML text-segment extract/splice engine (`lol_html`), `#[doc(hidden)]`
```
  and add a `crates/transync-html` entry to the **Layout** section, immediately **before** the `crates/transync-wasm` bullet, so the layout list and the member list agree:
```markdown
- `crates/transync-html` (DCR-0032) — the HTML mechanics, one file (`lib.rs`): `scan_tags` / `TagToken`, `element_extents`, `balance_fragment`, `extract` / `splice` + `BlankLinePolicy`, `strip_reserved_sync_attrs`. **No `[features]` table and no workspace-member dependency, ever** — `transync-syntax` sits on top of it and must keep passing the two-package `wasm32` gate. The module formerly called `transync-syntax::htmlseg` is this crate; records dated before 2026-08-20 still say `htmlseg` and are correct as written.
```

  **Do not touch anything else in `CLAUDE.md`.** In particular the WASM bullet's existing prohibition on `transync-syntax` (`Never add a `[features]` table or a `transync-core` dependency (dev-dependencies included)…`) stays exactly as written — the new crate's own prohibition is stated in its Layout entry above, and rewording a rule that is still true is how a correct sentence acquires a bug.

  **Verify by grep, not by reading**, and capture:
```bash
{
  grep -c 'transync-html' CLAUDE.md
  grep -c 'seven members' CLAUDE.md
  grep -c 'htmlseg' CLAUDE.md
  grep -n 'v0\.3\.0' CLAUDE.md
} > /Volumes/Temp/claude/ti490d97-wave0/gate/t7-claude-md.txt 2>&1
```
  Expected: `transync-html` — **2 lines / 4 occurrences**. Note the distinction, because it is the trap here: **`grep -c` counts LINES, not matches**, and `CLAUDE.md`'s Project paragraph is a single long line, so the two edits inside it collapse into one counted line. Use `grep -o … | wc -l` if you want occurrences. **Do not add a fifth mention to make a line-count target come out** — that is adjusting the artifact to fit the gate. `seven members` **0**; `htmlseg` **≥ 2** (the two deliberate name-continuity mentions — the crate's provenance and the dated-records note — survive on purpose, so a reader who greps the old name still lands somewhere); and no `v0.3.0` line except inside a dated historical statement, if one exists. Note that `grep -c` **exits non-zero on a zero count**, so do not run this under `set -e` — the `seven members` line is *expected* to be 0 and would abort the block.

- [ ] **Step 7: Verify — the docs-drift welds are the gate here.**
```bash
cargo test -p transync --test docs_index_drift --test docs_ownership_drift --test docs_gate_claims_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t7-docs.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t7-docs.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave0/gate/t7-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t7-workspace.txt
```
Expected: `CARGO_EXIT=0` in both. A red `docs_index_drift` means a new document under `docs/` is unlinked — link it rather than excluding it.

- [ ] **Step 8: Run the aggregate hard gate once, then commit.**
```bash
./scripts/smoke.sh > /Volumes/Temp/claude/ti490d97-wave0/gate/t7-smoke.txt 2>&1
echo "SMOKE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t7-smoke.txt
```
Read the file separately. Expected: `SMOKE_EXIT=0`. This is the run that proves the rustdoc-gate completeness check accepts the seven-member list (Task 1 Step 14) — the workspace test suite never exercises it.
```bash
git add docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md \
  CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml docs/index.md
git commit -m "docs: wave 0 gets its record, and the record names what stopped being called htmlseg

DCR-0032 carries the extraction, the four non-mechanical changes inside it,
and the two notes the name transync-html owes — that structural intake is not
here, and that the module records dated before today call htmlseg is this.
Both notes also live in the crate doc, because a reader who greps for the old
name finds nothing and needs somewhere to land.

TRACE: ti 490d97 wave 0
TRACE: DCR-0032

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: `AttrState::Outside` aligns with the browser it was measured against — ti `549b20`'s bypass closes, the strip stops cutting painted text, and the pin records the movement

**Files:**
- Modify: `crates/transync-html/tests/token_stream_pin.rs` (the `EDGE_CASES` corpus learns the blind spot — three constructions — and its doc comment stops overclaiming)
- Modify (regenerated through the interlocked hatch, twice): `crates/transync-html/tests/goldens/token-stream.txt`
- Create (generated at the first bless, modified at the second — both through the interlocked hatch): `crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt`
- Create (generated; move at the first bless only): `crates/transync-html/tests/goldens/balanced/stray-quote-doubled.txt`, `crates/transync-html/tests/goldens/balanced/stray-equals.txt`
- Modify: `crates/transync-html/src/lib.rs` (the two ruled scanner edits, the third `Outside`-arm edit measured into scope, nine red-first tests, the doc edits the fixes force)
- Modify: `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md` (a dated amendment, appended — the record is committed at `796a97b`, since amended by `63bcbc0`; locate the final section by heading and append after it), `CHANGELOG.md` (a `### Fixed` entry under `[Unreleased]`)
- Tickets: `549b20` closed with evidence; one new remainder ticket filed via `ti new` for the two divergences that outlive this task (HTML's *tag name* and *end-tag-open* states, both named as deliberately unfixed in the owner's ruling comment on the ticket)
- Test: the Task 2 pin is the instrument, used against itself deliberately — this is the **first task in the wave in which running `regenerate_goldens` is legitimate**, and it runs exactly twice, both times through the `TRANSYNC_REGEN_GOLDENS=1` interlock, both times with the resulting diff reported as evidence

**Interfaces:**
- Consumes from Task 4: `TagToken::{Open, Close, Skip}` and `scan_tags`'s attribute-state machine; from Task 5: `walk_elements` / `element_extents` / `balance_fragment`; from Task 6: `strip_reserved_sync_attrs` + `collect_reserved_attr_spans`; from Task 7: the committed `DCR-0032-transync-html-crate-extraction.md`.
- Produces (no signature changes anywhere — the whole fix is behavioural, inside `scan_tags`; the one new piece of state is a local `bool`):
  - In `AttrState::Outside`, a bare `"` / `'` is an **ordinary name byte** (`self_closing = false`, `has_attr_name = true`, nothing more) — HTML's *before attribute name* / *attribute name* states make a quote not preceded by `=` part of the attribute name, never a value opener.
  - In `AttrState::Outside`, `=` opens a value **only after a consumed attribute name**. HTML's *before attribute name* state makes a stray `=` a parse error that **starts a new attribute whose name is `=`**, moving to *attribute name* state — where a following quote is a parse error appended to the **name**. The ordinary `name=value` shape reaches `=` from *attribute name* / *after attribute name* and is unaffected. A new local `has_attr_name: bool` is the bit that tells the two apart: set by any name byte (quotes included), persisting through whitespace (*after attribute name*), reset by entering a value and by `/` (HTML's *self-closing start tag* state reconsumes everything but `>` in *before attribute name*).
  - A tag with no `>` before EOF pushes `TagToken::Skip { span: (start, html.len()) }` and then stops the scan, instead of leaving the document loop silently. The passed-over region is **named**; callers can see it instead of losing the suffix without a trace.
- Owner's ruling (recorded as the owner's 2026-08-21 comment on ti `549b20`, which also resolves the sub-choice — the `Skip` token over a scan-incomplete signal — and names the two deliberately-unfixed divergences with their measurements), implemented exactly for the two lines it shows:
```text
AttrState::Outside, today:
  b'"' | b'\'' => attr = Quoted(c)      // browser: name bytes
  if j >= bytes.len() { break }         // exits DOCUMENT loop

After:
  b'"' | b'\'' => self_closing = false  // ordinary name byte
  unterminated  => push Skip{span:(start,len)}, then stop
```
- **The third edit is inside the ruling, not an expansion of it — the reasoning is stated here so a reviewer can check it rather than take it.** During this task's derivation a sibling divergence was traced in the same arm, and the coordinator then **measured** it in headless Chromium — `<div ="> data-sync-id="v">x</div>` parses as:
```json
{
  "parsed_html": "<div =\"=\"\"> data-sync-id=\"v\">x</div>",
  "div_attrs":   ["=\"=\"\""],
  "div_text":    " data-sync-id=\"v\">x",
  "live_sync_ids": []
}
```
  The browser makes one attribute named `="`, ends the div at the **first** `>`, and paints ` data-sync-id="v">x` as **text**; `live_sync_ids` is empty, so this is *not* a live-anchor bypass — but a scanner that opens a value on the stray `=` swallows that first `>`, and the strip then **deletes text a browser paints**: a content mutation on adversarial input, the class invariant 6 exists to forbid. Three reasons this lands here and not in a follow-up ticket:
  1. **It is inside the ruling.** The chosen option is "align `AttrState::Outside` with HTML", and the `b'=' => BeforeValue` transition *is* `AttrState::Outside`. The ruling's preview quoted the quote line because that is the line the bypass ran through, not because the ruling was scoped to it. Fixing one divergence in the arm while knowingly leaving a sibling is the harder position to justify.
  2. **The blast-radius window opens exactly once.** This task already re-blesses goldens through the interlocked hatch and already reviews a tokenization diff. Deferring the sibling costs a second corpus commit, a second re-bless, a second blast-radius argument and a second review later, for a change that shares this one's surface.
  3. **Leaving it weakens the fix's own rationale.** The DCR amendment's sentence is "the scanner tokenizes like the browser in this state"; a known residual in that same state falsifies the sentence as it is written.
  The two-stage fix sequence below (Step 6 lands the ruled pair, Step 7 the `=` arm) exists to **capture the midpoint**: with only the ruled pair applied, the strip's behaviour on the measured construction is a *deletion* — that red run is the in-task, reviewer-checkable proof of point 3.
- **Blast radius over the four `scan_tags` consumers, stated up front and pinned below.** On well-formed input nothing moves anywhere — the fixes are reachable only through a bare quote in attribute position, a stray `=` in attribute-name position, or a `>`-less tail, and Step 0 re-proves the existing corpus and the three fixtures contain none of the three.
  1. `tag_inventory` — **unaffected by construction**: `TagToken::Skip { .. } => None` in its `filter_map` (verified in Step 0, pinned by a new test rather than assumed). Stray-markup inputs now contribute the entries a browser would see where before they contributed nothing.
  2. `walk_elements` → `element_extents` — stray-markup inputs now mint extents where before they minted none (`<div ">` and `<div =">` are complete open tags). A truncated tag (`<div class="x` at EOF) minted no extent before (the scan aborted having emitted nothing) and mints none now (`Skip` is not structure) — **the `Skip`-to-EOF change does not alter what wave 6 sees for an unclosed fragment**: an element whose open tag is complete still gets `close: None`, `content_end == html.len()`. The struct doc gains that sentence because wave 6 reads the doc as binding.
  3. `walk_elements` → `balance_fragment` — the one consumer whose *output bytes* change: `<div "> <p …>x</p>` now gains a `</div>` where before the input passed through unchanged. **Correct, and pinned as such**: DOMPurify 3.2.6 (the repository's own vendored copy, measured in ti `549b20`) closes the same div. The stray-`=` construction carries its own explicit `</div>`, so the balancer changes nothing for it — that inertness is pinned too.
  4. `strip_reserved_sync_attrs` — both directions close: the two bypass constructions now **lose** their planted `data-sync-id`, and the stray-`=` construction **keeps** every byte the browser paints as text (the plant sits outside the tag the fixed scanner ends at the first `>`, exactly where the browser ends it). Reserved-shaped bytes inside a `Skip` region stay unstripped **and that is fail-safe**: a browser mints no element and no attributes from a bogus comment or an EOF-truncated tag — the match-arm comment is updated to say so instead of claiming the bytes cannot exist.
- One line of the wave-acceptance checklist is superseded by this task, on purpose: "goldens byte-identical to the ones generated before the token change" now reads with one carve-out — the **three** stray-markup entries added here moved **inside this task, through the hatch, with the diff reviewed**; the fifteen wave-0-era entries are still bound by the original wording and must be byte-identical.

**Task-local constraints (restated — each one binds every step below):**
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave0/` — never `/tmp`, `/private/tmp`, or `$TMPDIR`. If the volume is unreachable, stop and ask.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`.
- Every `cargo test` capped: `-- --test-threads=4`. Never raise it.
- Every capture runs **bare**, redirected to a file, with the command line echoed as **line 1** and `echo "CARGO_EXIT=$?"` appended; the file is inspected as a *separate* step. Never `| grep | tail` on a test run.
- **`git commit --no-verify` is never used.** The pre-commit hook (fmt, clippy, wasm gate, rustdoc gate) takes **~15 minutes under load**, which exceeds the 10-minute foreground tool cap: every commit therefore runs inside a **single detached background chain** (one commit per background run, ~60-minute background budget), which writes its completion marker **last** and writes `REFUSED_AT=<stage>` to the marker on any failed gate. Do not poll for the chain with `pgrep -f` — wait for the completion notification or for the marker file to exist.
- Stage **exact paths only** — never `git add -A`, never a directory. The working tree carries other agents' in-flight edits (at plan time: `.gitignore`, `crates/transync-core/src/pipeline/finalize.rs`, `docs/index.md`, two wave plans); none of them may ride a Task 8 commit. If a workspace run fails in files outside this task's touch set, that is `REFUSED_AT=workspace-foreign`: report it, do not fix or revert another agent's work.
- `crates/transync/tests/cancellation.rs` is load-flaky (ticket `d41782`): if it trips, preserve the red capture under a `-LOADFLAKE-` name, re-run once, report **both** files, and never adjust the assertion.
- **No `_ =>` catch-all and no `matches!(x, Variant)` over this crate's enums** (`TagToken`, `BlankLinePolicy`) anywhere in the crate — the carve-out is by *enum ownership* (`char`, `u8`, `AttrState`, `Cow` are not this crate's exhaustible enums), **not** by test position; there is no test-code carve-out. The rewritten `Outside` arm below keeps every `u8` arm named anyway, including the quote and `=` arms the fixes are about.
- **`regenerate_goldens` may be run in this task — the first task in the wave where that is true.** It is `#[ignore]`d **and** interlocked on `TRANSYNC_REGEN_GOLDENS=1` (Task 4 Step 0). It runs exactly twice here: once to bless the *broken* behaviour after the corpus learns the gap (Step 2), once to re-bless after the full fix (Step 9) — and the second bless is preceded by reviewing the red pin's diff, never used to avoid reviewing it. Both resulting diffs are reported.
- Korean `*.ko.*` siblings (including the `.ko.txt` files beside the goldens) are out of scope: never read, edit, stage, or regenerate them — the owner handles them. Exact-path staging is what keeps them out of every commit.

- [ ] **Step 0: Preconditions — verify the five things this task is told not to assume.** All read-only; capture the session in `/Volumes/Temp/claude/ti490d97-wave0/gate/t8-preflight.txt` (append each command's output under its echoed command line).
  1. **The ticket is open and unclaimed, and the ruling is on it.** `ti show 549b20` — expected `Status: open`, `State: new`, tags including `490d97` and `owner-decision`, and the owner's 2026-08-21 ruling comment attached (option 1; the `Skip`-token sub-choice; the two deliberately-unfixed divergences with their measurements). If the ruling comment is absent, the ticket is closed, or another agent holds it, STOP.
  2. **The corpus is blind to the class.** In `crates/transync-html/tests/token_stream_pin.rs`, read the twelve `EDGE_CASES` entries and confirm: no entry contains a `"` or `'` reachable in `AttrState::Outside` (`script-raw-text`'s quotes sit inside raw-text content, which `raw_until` skips byte-by-byte and never feeds to the attribute loop; `unquoted-slash` has no quotes at all; the two doctypes take the bogus-comment branch and never reach the attribute loop), no entry has a `=` in attribute-name position (every `=` follows a consumed name), and no entry has a tag without a closing `>`. Also `grep -c 'stray-' crates/transync-html/tests/token_stream_pin.rs` — expected `0`.
  3. **The three fixtures are equally blind**, so their goldens cannot move:
```bash
for f in crates/transync/tests/fixtures/scn-15-html-blocks.md \
         crates/transync/tests/fixtures/scn-14-full.md \
         crates/transync/tests/fixtures/reader-honesty.md; do
  echo "== $f"
  perl -0777 -ne 'while (/<[A-Za-z][^>]*["\x27=][^>]*>/gs) { my $t = $&; $t =~ s/\n/\\n/g; print "  TAGQ: $t\n" }' "$f"
done
```
  Expected: exactly three `TAGQ:` lines — `<div align="center">` (twice) and `<div class="note">` — every quote preceded by `=` and every `=` preceded by a name, i.e. the paths the fixes do not touch. Any other hit is a STOP: the fixture corpus changed since this task was planned, and the golden-movement prediction below is void until re-derived.
  4. **`tag_inventory` filters `Skip` and the fix sites are byte-intact.** In `crates/transync-html/src/lib.rs`: `grep -n 'TagToken::Skip { .. } => None'` — exactly one hit, inside `tag_inventory`; `grep -c 'has_attr_name'` — expected `0` (the bit does not exist yet); and read the `AttrState::Outside` arm and the post-attribute-loop exit directly, confirming they still read `b'"' | b'\'' => { attr = AttrState::Quoted(c); self_closing = false; }`, `b'=' => { attr = AttrState::BeforeValue; self_closing = false; }`, and `break; // unterminated tag: leave as-is`. If either site has drifted from the shapes quoted in the Interfaces block, STOP and re-derive the edits before writing anything.
  5. **DCR-0032 is committed** (`796a97b`, since amended by `63bcbc0`) **with its final section on disk**: `grep -n '^## Migration / follow-up' docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md` — expected one hit. Step 12 appends after that section's content; it does not depend on any other wording in the file.
  6. `mkdir -p /Volumes/Temp/claude/ti490d97-wave0/gate`.

- [ ] **Step 1: Teach the corpus the blind spot — all three constructions in one commit.** In `crates/transync-html/tests/token_stream_pin.rs`, four edits: the corpus, its doc comment, and the two re-bless prohibitions (module doc, hatch doc) that would otherwise contradict Step 9's sanctioned second bless.
  1. Append the three constructions at the end of `EDGE_CASES`, immediately before the closing `];` — the two from ti `549b20`'s write-up plus the stray-`=` construction the coordinator measured:
```rust
    ("stray-quote-bare", "<div \"> <p data-sync-id=\"v\">x</p>"),
    ("stray-quote-doubled", "<div a=\"x\"\" data-sync-id=\"v\">y</div>"),
    ("stray-equals", "<div =\"> data-sync-id=\"v\">x</div>"),
];
```
  2. The doc comment above `EDGE_CASES` currently opens with a sentence the new entries would make false (`Every one tokenizes the same way before and after wave 0…`). Scope it to the entries it was written about, and append the two-bless protocol. Replace the first paragraph:
```rust
/// The regions the R0002-* / R0003-* incidents were about, plus the two
/// doctype spellings, plus (task 8) the three stray-markup constructions
/// of ti 549b20. The twelve wave-0-era entries tokenize the same way
/// before and after wave 0's token change, which is what made this golden
/// a valid before/after comparison for that change.
```
  and append, after the existing `<![CDATA[…]]>` paragraph (the one ending `their tokenization does not move either.`):
```rust
///
/// The three `stray-*` entries are different in kind (ti 549b20, task 8):
/// they are the corpus learning a blind spot. Until task 8 no entry held a
/// bare quote in attribute position, a stray `=` in attribute-name
/// position, or an unterminated tag, so the pin could not see the
/// scanner's stray-markup divergences from a real browser. Their goldens
/// are blessed twice, deliberately: the commit that adds them blesses the
/// BROKEN scanner's behaviour — empty stream, input passed through the
/// balancer unchanged — and the fix re-blesses them through the same
/// interlocked hatch, so the diff between the two blessings is the
/// reviewable record of exactly how tokenization changed. Unlike every
/// entry above, they exist because their tokenization moved inside wave 0.
```
  3. **The module doc's re-bless prohibition names its one sanctioned exception** — as written ("If the pin goes red … do not re-bless the golden") it contradicts Step 9, where the corpus is unchanged, the scanner moved, and the red pin is deliberately re-blessed. In the `//!` module doc, replace:
```rust
//! change was not inert — do not re-bless the golden, find out which region
//! moved.**
```
  with:
```rust
//! change was not inert — do not re-bless the golden, find out which region
//! moved.** The one sanctioned exception is a deliberate, reviewed tokenizer
//! change landed through the two-bless protocol the `stray-*` entries
//! document (ti 549b20, task 8): bless the broken behaviour first, fix,
//! re-bless, and review the diff between the blessings as the record of the
//! movement. The prohibition on blessing a red pin *instead of* reviewing it
//! stands everywhere else.
```
  4. **The hatch's own doc admits the same bounded case** — "only when the corpus itself changes — never to turn a red pin green" forbids Step 9's second bless as written. On `regenerate_goldens`, replace:
```rust
/// deliberately, and only when the corpus itself changes — never to turn a
/// red pin green:
```
  with:
```rust
/// deliberately, and only when the corpus itself changes or a deliberate,
/// reviewed tokenizer change lands through the two-bless protocol (ti
/// 549b20, task 8) — never to turn a red pin green as a shortcut past
/// reviewing what moved:
```

- [ ] **Step 2: See the pin red for the right reason, then bless the broken behaviour.** Three foreground captures, each in the echoed-command-line-1 form.
  1. The red — the corpus outgrew the golden:
```bash
echo 'cargo test -p transync-html --test token_stream_pin -- --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-corpus-red.txt
cargo test -p transync-html --test token_stream_pin -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-corpus-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-corpus-red.txt
```
  Read the file separately. Expected: `CARGO_EXIT=101`, `0 passed; 2 failed; 1 ignored`, and the two failures for the *right* reasons — the token test on its anti-vacuity guard (`the golden does not cover the whole corpus — it was generated against a different FIXTURES/EDGE_CASES set`, left `15`, right `18`), the balanced test on `…/goldens/balanced/stray-quote-bare.txt should be readable: No such file or directory (os error 2)`. Any *other* failure shape means an entry above the new three moved — STOP.
  2. The first bless, through the interlock (**sanctioned: the corpus itself changed** — the one legitimate reason the Task 2 notes name):
```bash
echo 'TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless1.txt
TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless1.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless1.txt
```
  Expected: `CARGO_EXIT=0`, `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out` — Task 2 Step 3's shape note applies verbatim: `2 ignored` here would mean the generator did **not** run.
  3. Inspect what the bless wrote — this is the moment the *broken* behaviour becomes a recorded fact:
```bash
git status --porcelain -- crates/transync-html/tests/goldens
tail -9 crates/transync-html/tests/goldens/token-stream.txt | od -c | head -16
cat crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt
cat crates/transync-html/tests/goldens/balanced/stray-quote-doubled.txt
cat crates/transync-html/tests/goldens/balanced/stray-equals.txt
```
  Expected, exactly: `git status` shows ` M …/token-stream.txt`, `?? …/balanced/stray-quote-bare.txt`, `?? …/balanced/stray-quote-doubled.txt`, `?? …/balanced/stray-equals.txt` and **nothing else** (in particular no `.ko.txt` line and no pre-existing `balanced/*.txt` line — the generator rewrites those fifteen byte-identically). The tail of `token-stream.txt` is the three new sections with **empty** projections — `inventory:` and `stream:` each followed by one space and a newline (the `od` output makes the trailing spaces checkable):
```text
### stray-quote-bare
inventory: 
stream: 
### stray-quote-doubled
inventory: 
stream: 
### stray-equals
inventory: 
stream: 
```
  All three balanced files hold their input byte-verbatim, no trailing newline: `<div "> <p data-sync-id="v">x</p>`, `<div a="x"" data-sync-id="v">y</div>`, `<div ="> data-sync-id="v">x</div>` — the wholly-broken scanner emits **zero tokens** for each (the phantom quote state runs every one of them off EOF), so the balancer passes all three through untouched and the strip sees nothing. Then re-run the pin (same capture form, `t8-corpus-green.txt`): expected `CARGO_EXIT=0`, `2 passed; 0 failed; 1 ignored`.

- [ ] **Step 3: Commit A — the corpus learning a gap is its own reviewable act.** One detached background chain, one commit, marker written last:
```bash
cd /Volumes/Common/QJoon/transync || exit 1
G=/Volumes/Temp/claude/ti490d97-wave0/gate
M="$G/t8-commitA.marker"
mkdir -p "$G"; rm -f "$M"
run() {
  local stage="$1"; shift
  echo "$*" > "$G/t8a-$stage.txt"
  "$@" >> "$G/t8a-$stage.txt" 2>&1
  echo "CARGO_EXIT=$?" >> "$G/t8a-$stage.txt"
  [ "$(tail -n 1 "$G/t8a-$stage.txt")" = "CARGO_EXIT=0" ] || { echo "REFUSED_AT=$stage" > "$M"; exit 1; }
}
run fmt cargo fmt --all
run clippy cargo clippy --all-targets --all-features -- -D warnings
run crate cargo test -p transync-html -- --test-threads=4
git add crates/transync-html/tests/token_stream_pin.rs \
  crates/transync-html/tests/goldens/token-stream.txt \
  crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt \
  crates/transync-html/tests/goldens/balanced/stray-quote-doubled.txt \
  crates/transync-html/tests/goldens/balanced/stray-equals.txt
git commit -m "test(transync-html): the corpus learns the stray-markup blind spot before the scanner learns the fix

EDGE_CASES gains the three constructions from ti 549b20 - a bare quote in
attribute position, a doubled quote after a closed value, and a stray
equals sign before a quote, the last one measured in headless Chromium -
and the goldens are blessed from the scanner as it stands, which is
BROKEN on all three: empty token stream, input passed through the
balancer unchanged, planted sync attribute invisible to the strip.
Blessing the broken behaviour first is the point: the fix re-blesses in
a later commit, and the diff between the two blessings is the reviewable
record of exactly how tokenization changed.

TRACE: ti 490d97 wave 0
TRACE: ti 549b20

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" > "$G/t8a-commit.txt" 2>&1
echo "COMMIT_EXIT=$?" >> "$G/t8a-commit.txt"
[ "$(tail -n 1 "$G/t8a-commit.txt")" = "COMMIT_EXIT=0" ] || { echo "REFUSED_AT=pre-commit-hook" > "$M"; exit 1; }
echo "COMPLETED=t8-commitA COMMIT=$(git rev-parse HEAD)" > "$M"
```
Expected: the marker reads `COMPLETED=t8-commitA COMMIT=<hash>`; `t8a-crate.txt` shows `62 passed` for the lib and `2 passed; 1 ignored` for the pin. If the marker reads `REFUSED_AT=…`, read that stage's capture, fix the cause, re-run the whole chain — never `--no-verify`.

- [ ] **Step 4: Write the nine red-first tests.** All in `crates/transync-html/src/lib.rs`, each in the module that owns its consumer. **Change no existing assertion.** One of the nine — the strip's content-preservation test — is *vacuously green* until Step 6 and earns its red at the midpoint; that is stated on the test and re-stated in Step 5, so nobody mistakes the sequencing for an error.
  1. In `mod token_tests` (after `skip_tokens_are_invisible_to_both_shipped_consumers`, before the `skips` helper):
```rust
    /// ti 549b20: HTML's before-attribute-name / attribute-name states make
    /// a quote not preceded by `=` part of the attribute NAME. Opening a
    /// phantom quoted value here desynchronized the scanner from every
    /// browser and hid the rest of the document from every consumer.
    #[test]
    fn a_bare_quote_in_a_tag_is_a_name_byte_not_a_value_opener() {
        let html = "<div \">x";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 1, "one open tag: {opens:?}");
        assert_eq!(opens[0].0, "div");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<div \">");
    }

    /// ti 549b20, measured in headless Chromium: a stray `=` in
    /// before-attribute-name state STARTS an attribute named `=`, and the
    /// quote after it joins that NAME — no value state is entered, so the
    /// browser ends this tag at the FIRST `>`. The ordinary `name=value`
    /// shape reaches `=` from a consumed name and still opens the value.
    #[test]
    fn a_stray_equals_does_not_open_a_value() {
        let html = "<div =\"> data-sync-id=\"v\">x</div>";
        let mut opens: Vec<(String, (usize, usize))> = Vec::new();
        for token in scan_tags(html) {
            if let TagToken::Open { name, span, .. } = token {
                opens.push((name, span));
            }
        }
        assert_eq!(opens.len(), 1, "one open tag: {opens:?}");
        assert_eq!(&html[opens[0].1.0..opens[0].1.1], "<div =\">");
        // The ordinary shape is untouched: `=` after a NAME opens the
        // value, and a `>` inside that value stays data.
        let ok = "<div a=\">\" b>x";
        let spans: Vec<(usize, usize)> = scan_tags(ok)
            .into_iter()
            .filter_map(|t| match t {
                TagToken::Open { span, .. } => Some(span),
                TagToken::Close { .. } | TagToken::Skip { .. } => None,
            })
            .collect();
        assert_eq!(spans.len(), 1);
        assert_eq!(&ok[spans[0].0..spans[0].1], "<div a=\">\" b>");
    }

    /// ti 549b20: the old exit left the document loop with no token, so a
    /// caller could not tell a passed-over suffix from a scanned one. A
    /// browser abandons a tag truncated at EOF — no element, no attributes —
    /// and the scanner now says so in the stream.
    #[test]
    fn an_unterminated_tag_is_a_skip_to_eof_not_a_silent_abort() {
        let html = "<p>a</p><div class=\"x";
        assert_eq!(skips(html), vec![(8, 21)]);
        assert_eq!(&html[8..21], "<div class=\"x");
        // The ledger is unaffected by construction: `tag_inventory` filters
        // `Skip` out, so the region never mints an inventory entry.
        assert_eq!(tag_inventory(html), vec!["p", "/p"]);
    }
```
  2. In `mod extent_tests` (after `the_walk_is_shared_with_the_balancer`):
```rust
    /// ti 549b20. `<div ">` is a complete open tag — the quote is a name
    /// byte — and it minted nothing before the fix because the scanner
    /// aborted the whole scan instead.
    #[test]
    fn a_stray_quote_tag_mints_an_extent_and_an_unterminated_tag_does_not() {
        let html = "<div \"> <p>x</p>";
        let ex = element_extents(html);
        let names: Vec<&str> = ex.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["div", "p"]);
        assert!(ex[0].close.is_none(), "the div is unclosed at EOF");
        assert_eq!(ex[0].content_end, html.len());
        // A tag with no `>` at all is a Skip: no element, exactly as a
        // browser abandons it. An intake caller sees the passed-over region
        // in the token stream, not a phantom element here.
        assert!(element_extents("<div class=\"x").is_empty());
    }
```
  3. In `mod strip_tests` (after `stripping_never_changes_the_tag_inventory`):
```rust
    /// ti 549b20, measured: DOMPurify 3.2.6 (the vendored copy) sanitizes
    /// this to `<div> <p data-sync-id="v">x</p></div>` — a live P#v anchor —
    /// while the pre-fix scanner saw zero tokens and stripped nothing.
    #[test]
    fn a_stray_quote_cannot_hide_a_planted_sync_attr() {
        let html = "<div \"> <p data-sync-id=\"v\">x</p>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div \"> <p>x</p>");
    }

    /// ti 549b20, measured: DOMPurify keeps this plant as a live DIV#v. The
    /// stray quote after the closed value is a name byte, so the scanner now
    /// reads the tag the way the browser does and the strip reaches the
    /// plant.
    #[test]
    fn a_doubled_quote_cannot_hide_a_planted_sync_attr() {
        let html = "<div a=\"x\"\" data-sync-id=\"v\">y</div>";
        assert_eq!(strip_reserved_sync_attrs(html), "<div a=\"x\"\">y</div>");
    }

    /// ti 549b20, measured in headless Chromium: the browser makes one
    /// attribute named `="`, ends the div at the FIRST `>`, and paints
    /// ` data-sync-id="v">x` as TEXT — live_sync_ids is empty. The strip
    /// must never delete bytes a browser renders. This test is vacuously
    /// green against the wholly-broken scanner (zero tokens, borrow); its
    /// red arrives at the task's midpoint, where fixing only the quote and
    /// unterminated-tag divergences turns the bypass into a DELETION — a
    /// content mutation, not a bypass — which is exactly why the stray-`=`
    /// arm is fixed in the same task.
    #[test]
    fn text_the_browser_paints_is_never_cut_by_the_strip() {
        let html = "<div =\"> data-sync-id=\"v\">x</div>";
        assert_eq!(strip_reserved_sync_attrs(html), html);
    }
```
  4. In `mod balance_tests` (after `a_slash_outside_any_attribute_value_still_self_closes`):
```rust
    /// ti 549b20, measured against the vendored DOMPurify 3.2.6: a browser
    /// parses `<div ">` as an open div and the sanitized DOM closes it at
    /// fragment end. The old scanner saw zero tokens here and returned the
    /// input unchanged — disagreeing with the DOM this function exists to
    /// protect.
    #[test]
    fn a_stray_quote_no_longer_hides_an_unclosed_div_from_the_balancer() {
        assert_eq!(
            balance_fragment("<div \"> <p>x</p>"),
            "<div \"> <p>x</p></div>"
        );
        // An unterminated tag is a Skip, not structure: nothing to balance,
        // before the fix and after it.
        assert_eq!(balance_fragment("<p>a</p><div "), "<p>a</p><div ");
    }
```
  5. In `mod inventory_tests` (after `translating_text_that_looks_like_a_digit_tag_preserves_the_tag_inventory`):
```rust
    /// ti 549b20: the inventory is the layer-3 ledger, and the Skip-to-EOF
    /// token must be filtered by construction — verified here rather than
    /// assumed, because a phantom entry there fails good translations.
    #[test]
    fn the_ledger_never_carries_a_skipped_suffix() {
        assert_eq!(
            tag_inventory("<div \"> <p data-sync-id=\"v\">x</p>"),
            vec!["div", "p", "/p"]
        );
        assert_eq!(tag_inventory("<p>a</p><div class=\"x"), vec!["p", "/p"]);
    }
```

- [ ] **Step 5: Run them and see exactly eight fail.**
```bash
echo 'cargo test -p transync-html --lib -- --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-red.txt
cargo test -p transync-html --lib -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-red.txt
```
Read the file separately. Expected: `CARGO_EXIT=101`, `test result: FAILED. 63 passed; 8 failed` (62 existing + the vacuously-green preservation test pass; eight of the nine new tests fail), each failure for its predicted reason — these are runtime assertion failures, not compile errors, because every function already exists:
  - `token_tests::a_bare_quote_in_a_tag_is_a_name_byte_not_a_value_opener` — `one open tag: []` (the old scanner enters `Quoted`, runs to EOF, aborts: zero tokens).
  - `token_tests::a_stray_equals_does_not_open_a_value` — `one open tag: []` (same abort: the phantom `Quoted` from the old quote arm re-opens on the quote after `v` and runs off EOF).
  - `token_tests::an_unterminated_tag_is_a_skip_to_eof_not_a_silent_abort` — left `[]`, right `[(8, 21)]` (no `Skip` exists on the abort path).
  - `extent_tests::a_stray_quote_tag_mints_an_extent_and_an_unterminated_tag_does_not` — left `[]`, right `["div", "p"]`.
  - `balance_tests::a_stray_quote_no_longer_hides_an_unclosed_div_from_the_balancer` — left `"<div \"> <p>x</p>"` (unchanged), right with the appended `</div>`.
  - `inventory_tests::the_ledger_never_carries_a_skipped_suffix` — left `[]`, right `["div", "p", "/p"]`.
  - `strip_tests::a_stray_quote_cannot_hide_a_planted_sync_attr` — left is the input byte-unchanged, plant intact.
  - `strip_tests::a_doubled_quote_cannot_hide_a_planted_sync_attr` — left is the input byte-unchanged, plant intact.
  And `strip_tests::text_the_browser_paints_is_never_cut_by_the_strip` **passes — vacuously**: the wholly-broken scanner emits zero tokens for its input, so the strip borrows. Its non-vacuous red is Step 6's midpoint capture; its anti-vacuity moment is there, not here. Any *ninth* failure, or any of the eight failing differently, is a STOP — the scanner or a consumer is not in the state the plan derived from.

- [ ] **Step 6: The ruled pair — and the midpoint capture that proves the third edit belongs here.** Two edits in `crates/transync-html/src/lib.rs`, exactly the two lines the ruling shows, then a run whose red is evidence.
  1. **The quote arm.** In `scan_tags`, in the `AttrState::Outside` match, replace:
```rust
                    b'"' | b'\'' => {
                        attr = AttrState::Quoted(c);
                        self_closing = false;
                    }
```
  with:
```rust
                    b'"' | b'\'' => self_closing = false,
```
  (Step 7 rewrites this whole arm again, comments included — this intermediate form exists to measure the ruled pair in isolation.)
  2. **The unterminated exit.** Still in `scan_tags`, replace:
```rust
        if j >= bytes.len() {
            break; // unterminated tag: leave as-is
        }
```
  with:
```rust
        if j >= bytes.len() {
            // ti 549b20: an unterminated tag runs to EOF — there are no
            // bytes past it by definition — but the old bare `break` left
            // the DOCUMENT loop with no token, so a caller could not tell
            // a passed-over region from a scanned one. Name it instead: a
            // browser abandons a tag truncated at EOF (no element, no
            // attributes), so `Skip` — recognized and stepped over, not
            // markup — is exactly what it is.
            tokens.push(TagToken::Skip {
                span: (start, bytes.len()),
            });
            break;
        }
```
  3. **The midpoint run — capture it; its red is the in-task justification for Step 7:**
```bash
echo 'cargo test -p transync-html --lib -- --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-mid-red.txt
cargo test -p transync-html --lib -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-mid-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-mid-red.txt
```
  Read the file separately. Expected: `CARGO_EXIT=101`, `test result: FAILED. 69 passed; 2 failed` — the seven bypass-class tests all green, and exactly these two red:
  - `strip_tests::text_the_browser_paints_is_never_cut_by_the_strip` — **left `"<div =\">>x</div>"`**: with only the ruled pair applied, the scanner ends the stray-`=` tag at the *second* `>` (the phantom value swallowed the first), and the strip **deleted** ` data-sync-id="v"` — bytes headless Chromium renders as text — leaving the swallowed first `>` behind beside the tag's own, hence the doubled `>>`. The bypass became a content mutation. This capture is the measured, reviewer-checkable form of the scope argument in the Interfaces block: stopping at the ruled pair leaves the same `Outside` arm browser-divergent in a way that now *edits what the reader sees*, which invariant 6 forbids.
  - `token_tests::a_stray_equals_does_not_open_a_value` — left `"<div =\"> data-sync-id=\"v\">"` (span `(0, 26)`), right `"<div =\">"`: the tag end overshoots the browser's by exactly the swallowed `>`.
  Preserve `t8-mid-red.txt`; it rides the final report and the DCR amendment cites it.

- [ ] **Step 7: The `=` arm — HTML's before-attribute-name rule lands, and the docs the fixes force.** All in `crates/transync-html/src/lib.rs`. This step rewrites the whole `Outside` arm, subsuming Step 6's quote edit — the two-stage sequence existed to capture the midpoint, not because the arm is edited twice by accident.
  1. **The state bit.** Replace:
```rust
        let mut attr = AttrState::Outside;
        let mut self_closing = false;
```
  with:
```rust
        let mut attr = AttrState::Outside;
        let mut self_closing = false;
        // ti 549b20: has the current attribute consumed a NAME byte since
        // the last boundary (tag name, `/`, or a completed value)? HTML
        // opens a value on `=` only from the attribute-name /
        // after-attribute-name states; a stray `=` in before-attribute-name
        // STARTS an attribute named `=` instead, and a quote after it joins
        // that name. This bit tells the two apart — whitespace does not
        // reset it (after-attribute-name), a completed value or a `/` does
        // (after-attribute-value-quoted and self-closing-start-tag both
        // reconsume in before-attribute-name).
        let mut has_attr_name = false;
```
  2. **The whole `Outside` arm.** Replace the arm as Step 6 left it (quote arm `b'"' | b'\'' => self_closing = false,`, `=` arm entering `BeforeValue` unconditionally) with:
```rust
                AttrState::Outside => match c {
                    b'>' => break,
                    // ti 549b20: a bare quote is a parse error that joins
                    // the attribute NAME in a browser — never a value
                    // opener. Before the fix it opened a phantom Quoted
                    // state, and one stray quote ran the scan off EOF and
                    // hid everything after it from every consumer.
                    b'"' | b'\'' => {
                        self_closing = false;
                        has_attr_name = true;
                    }
                    b'=' => {
                        self_closing = false;
                        if has_attr_name {
                            // attribute-name / after-attribute-name: `=`
                            // ends the name and opens the value. The
                            // ordinary `name=value` shape lands here.
                            attr = AttrState::BeforeValue;
                            has_attr_name = false;
                        } else {
                            // before-attribute-name: `=` is a parse error
                            // that STARTS an attribute whose name is `=`
                            // (measured in headless Chromium: `<div =">`
                            // is one attribute named `="` and the tag ends
                            // at the first `>`). No value state — the
                            // quote after it joins the NAME.
                            has_attr_name = true;
                        }
                    }
                    b'/' => {
                        // self-closing-start-tag: anything but `>`
                        // reconsumes in before-attribute-name, so the
                        // name track resets with it.
                        self_closing = true;
                        has_attr_name = false;
                    }
                    _ if c.is_ascii_whitespace() => self_closing = false,
                    _ => {
                        self_closing = false;
                        has_attr_name = true;
                    }
                },
```
  The `BeforeValue`, `Quoted` and `Unquoted` arms are untouched: `has_attr_name` is already `false` whenever they run, because entering `BeforeValue` reset it. (This match is over `u8`, not one of this crate's enums, so its `_` arms are exempt from the catch-all constraint; every byte the fixes are about is a named arm regardless.) The split of the old `_ => self_closing = false` into a whitespace guard and a name-byte arm changes nothing for `self_closing` — both still clear it — and only whitespace's *non*-participation in the name track is new.
  3. **`AttrState::Outside`'s variant doc** gains the new facts. Replace:
```rust
    /// Between attributes, or after a quoted value: `/` here is a marker.
```
  with:
```rust
    /// Between attributes, or after a quoted value: `/` here is a marker,
    /// a bare `"` / `'` is an ordinary name byte, and `=` opens a value
    /// only after a consumed attribute name — a stray `=` starts an
    /// attribute NAMED `=` instead (ti 549b20; both measured against a
    /// real browser).
```
  4. **`TagToken::Skip`'s doc** gains the fourth region. Replace its first sentence (`/// A region the scanner recognizes and steps over: a comment, a CDATA` … `or a bogus comment (`<!…>` / `<?…>`).`) with:
```rust
    /// A region the scanner recognizes and steps over: a comment, a CDATA
    /// section (either terminator mode), a bogus comment (`<!…>` / `<?…>`),
    /// or a tag left unterminated at EOF (ti 549b20 — a browser abandons a
    /// tag cut off before its `>`, minting no element and no attributes, so
    /// the bytes are a passed-over region, not markup).
```
  The rest of the variant doc (`It carries no name…`) stands as written.
  5. **The strip's match-arm comment** stops claiming the bytes cannot exist. In `strip_reserved_sync_attrs`, replace:
```rust
            // A close tag carries no attribute list, and a skipped region is
            // not markup at all: neither can hold a reserved name.
```
  with:
```rust
            // A close tag carries no attribute list. A skipped region —
            // comment, CDATA, bogus comment, or a tag left unterminated at
            // EOF (ti 549b20) — never mints an element in a browser, so
            // reserved-name-shaped bytes inside one cannot become live
            // attributes; leaving them unstripped is fail-safe, not an
            // oversight.
```
  6. **The strip's fn doc** gains its malformed-markup posture, appended as a new paragraph after the `Only names are matched…` paragraph:
```rust
///
/// Malformed markup is tokenized the way a browser tokenizes it where the
/// two were measured to disagree (ti 549b20): a bare quote between
/// attributes is a NAME byte, never a value opener; a stray `=` in
/// attribute-name position STARTS an attribute named `=` rather than
/// opening a value, so the strip neither misses a plant hidden behind one
/// nor deletes text a browser paints after the tag's real end; and a tag
/// left unterminated at EOF is a passed-over [`TagToken::Skip`] region a
/// browser abandons. That exhausts the known divergences in the regions
/// this fix touched; two remain one state earlier — HTML's tag-name state
/// consumes `=` and quote bytes into the ELEMENT name until the first
/// whitespace, where this scanner ends the name earlier, and HTML's
/// end-tag-open state opens a bogus comment on `</` before a non-letter,
/// where this scanner sees plain text and keeps tokenizing — both
/// recorded in DCR-0032's 2026-08-21 amendment. Neither yields a
/// live-anchor construction through the pane path: a diverging element
/// name can never match a sanitizer's allowlist, and a browser mints no
/// element at all from a bogus comment's interior.
```
  7. **`ElementExtent`'s struct doc** gains the sentence wave 6 will read as binding, appended after the existing `**Not every tag mints one.**` paragraph:
```rust
///
/// **A truncated tag is not an element either.** A tag with no closing `>`
/// before EOF is a [`TagToken::Skip`], not an `Open` (ti 549b20): it mints
/// no extent, exactly as a browser abandons a tag cut off at EOF. An
/// element whose open tag is complete but whose end tag never arrives is
/// different — it still gets an extent with `close: None` and
/// `content_end == html.len()`.
```

- [ ] **Step 8: Green in-crate — and the pin goes red. The red is the deliverable, capture it before touching the goldens.**
```bash
echo 'cargo test -p transync-html --lib -- --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-green.txt
cargo test -p transync-html --lib -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-green.txt
echo 'cargo test -p transync-html --test token_stream_pin -- --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-pin-red.txt
cargo test -p transync-html --test token_stream_pin -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-pin-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-pin-red.txt
```
Read both separately. Expected: `t8-green.txt` — `CARGO_EXIT=0`, `test result: ok. 71 passed; 0 failed` (62 + the 9 from Step 4), **no pre-existing assertion edited**. `t8-pin-red.txt` — `CARGO_EXIT=101`, `0 passed; 2 failed; 1 ignored`: the token test's diff moving only in the three `stray-*` sections, and the balanced test failing for `stray-quote-bare` and **no other name** (`balance_fragment moved for \`stray-quote-bare\``). This red is the instrument reading the fix. It is evidence, not an obstacle — preserve the capture; it rides the final report.

- [ ] **Step 9: Re-bless through the hatch, then review the diff as the evidence it exists to be.** The second sanctioned `regenerate_goldens` run:
```bash
echo 'TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4' > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless2.txt
TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-html regenerate_goldens -- --ignored --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless2.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave0/gate/t8-bless2.txt
git diff --name-only -- crates/transync-html/tests/goldens > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-golden-files.txt
git diff -- crates/transync-html/tests/goldens > /Volumes/Temp/claude/ti490d97-wave0/gate/t8-golden-diff.txt
```
Then **review, against this exact prediction**:
  1. `t8-golden-files.txt` holds **exactly two lines**:
```text
crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt
crates/transync-html/tests/goldens/token-stream.txt
```
  2. `t8-golden-diff.txt`'s hunks, in full — **only the `-`/`+` lines below are the prediction**; the real diff also carries `@@` hunk headers, up to three context lines reaching into the `### doctype-lower` section above the first hunk, and two `\ No newline at end of file` markers on the `stray-quote-bare` hunk (neither version of that file ends in a newline) — that dressing is mechanical and not part of the check. `token-stream.txt` — the file's final nine lines (the three new sections; nothing above them):
```diff
 ### stray-quote-bare
-inventory: 
-stream: 
+inventory: div|p|/p
+stream: O:div:false|O:p:false|C:p@29..33
 ### stray-quote-doubled
-inventory: 
-stream: 
+inventory: div|/div
+stream: O:div:false|C:div@30..36
 ### stray-equals
-inventory: 
-stream: 
+inventory: div|/div
+stream: O:div:false|C:div@27..33
```
  (`C:p@29..33` is `</p>` in `<div "> <p data-sync-id="v">x</p>`; `C:div@30..36` is `</div>` in `<div a="x"" data-sync-id="v">y</div>`; `C:div@27..33` is `</div>` in `<div ="> data-sync-id="v">x</div>`, whose open tag is the eight bytes `<div =">` ending at the **first** `>`, exactly where Chromium ends it — half-open byte offsets, derivable by hand from the three corpus strings.) `balanced/stray-quote-bare.txt` — one line changes, gaining the same `</div>` the measured DOMPurify DOM has:
```diff
-<div "> <p data-sync-id="v">x</p>
+<div "> <p data-sync-id="v">x</p></div>
```
  3. **The safety property, asserted, with a STOP attached.** `balanced/stray-quote-doubled.txt` and `balanced/stray-equals.txt` do **not** appear — both inputs carry an explicit `</div>` that closes the div the fixed scanner now sees, so the balancer changes nothing for either. And none of the fifteen wave-0-era entries (three fixtures + twelve edge cases) appears anywhere in either file: none contains a bare quote reachable in `Outside` state, a stray `=` in attribute-name position, or an unterminated tag (re-proved in Step 0), so their goldens **must not move**. If any pre-existing golden appears in `t8-golden-files.txt`, or any hunk in `token-stream.txt` sits outside the final nine lines, **that is a finding about the fix's blast radius, not a bad bless**: STOP, write `REFUSED_AT=golden-blast-radius` to `/Volumes/Temp/claude/ti490d97-wave0/gate/t8-commitB.marker`, and report which entry moved and how. Do not re-bless it away and do not commit.
  4. Re-run the pin (capture as `t8-pin-green.txt`, same form): expected `CARGO_EXIT=0`, `2 passed; 0 failed; 1 ignored`.

- [ ] **Step 10: Commit B — the three fixes, their tests, their docs, and the reviewed re-bless, in one commit.** One detached background chain (workspace run + ~15-minute hook fit the ~60-minute background budget), one commit, marker last:
```bash
cd /Volumes/Common/QJoon/transync || exit 1
G=/Volumes/Temp/claude/ti490d97-wave0/gate
M="$G/t8-commitB.marker"
mkdir -p "$G"; rm -f "$M"
run() {
  local stage="$1"; shift
  echo "$*" > "$G/t8b-$stage.txt"
  "$@" >> "$G/t8b-$stage.txt" 2>&1
  echo "CARGO_EXIT=$?" >> "$G/t8b-$stage.txt"
  [ "$(tail -n 1 "$G/t8b-$stage.txt")" = "CARGO_EXIT=0" ] || { echo "REFUSED_AT=$stage" > "$M"; exit 1; }
}
ws() {
  echo 'cargo test --workspace -- --test-threads=4' > "$G/$1"
  cargo test --workspace -- --test-threads=4 >> "$G/$1" 2>&1
  echo "CARGO_EXIT=$?" >> "$G/$1"
  [ "$(tail -n 1 "$G/$1")" = "CARGO_EXIT=0" ]
}
run fmt cargo fmt --all
run clippy cargo clippy --all-targets --all-features -- -D warnings
run crate cargo test -p transync-html -- --test-threads=4
if ! ws t8b-workspace.txt; then
  if grep -q 'cancellation' "$G/t8b-workspace.txt"; then
    # Ticket d41782: preserve the red, re-run once, report BOTH files.
    # Never adjust the assertion.
    mv "$G/t8b-workspace.txt" "$G/t8b-workspace-LOADFLAKE-1.txt"
    ws t8b-workspace.txt || { echo "REFUSED_AT=workspace" > "$M"; exit 1; }
  else
    echo "REFUSED_AT=workspace" > "$M"; exit 1
  fi
fi
git status --porcelain -- 'crates/*/tests/fixtures' > "$G/t8b-fixtures.txt"
if [ -s "$G/t8b-fixtures.txt" ]; then echo "REFUSED_AT=fixtures" > "$M"; exit 1; fi
git diff --name-only -- crates/transync-html/tests/goldens > "$G/t8b-golden-files.txt"
if ! diff -q "$G/t8b-golden-files.txt" - <<'EOF' >/dev/null
crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt
crates/transync-html/tests/goldens/token-stream.txt
EOF
then echo "REFUSED_AT=golden-blast-radius" > "$M"; exit 1; fi
git add crates/transync-html/src/lib.rs \
  crates/transync-html/tests/goldens/token-stream.txt \
  crates/transync-html/tests/goldens/balanced/stray-quote-bare.txt
git commit -m "fix(transync-html): AttrState::Outside aligns with the browser on quotes, stray equals, and unterminated tags

Three scanner divergences from HTML lived in one state. A bare quote
opened a phantom quoted value where a browser grows an attribute name -
composed with the silent unterminated-tag abort, that was the strip
bypass ti 549b20 measured through the vendored DOMPurify. And a stray
equals sign entered the value state where a browser starts an attribute
NAMED equals - measured in headless Chromium, which ends that tag at the
first closing angle bracket and paints the rest as text. Fixing only the
ruled pair converted the bypass into a strip that DELETES painted text,
and the midpoint capture in the gate directory shows that red. As
landed: a bare quote is a name byte, equals opens a value only after a
consumed attribute name (has_attr_name is the bit that tells HTML's
before-attribute-name state apart from attribute-name), and a tag with
no closing angle bracket before EOF is pushed as TagToken::Skip spanning
to end of input before the scan stops.

The pin moved and was re-blessed with the diff reviewed: exactly the
three stray-markup entries added in the previous commit, and nothing
else - the fifteen pre-existing goldens are byte-identical. tag_inventory
filters Skip by construction, so the layer-3 ledger is untouched;
balance_fragment closes what the sanitized DOM closes; element_extents
mints what the old scanner never saw, and a truncated tag mints none;
the strip reaches both plants and no longer cuts what a browser paints.

TRACE: ti 490d97 wave 0
TRACE: ti 549b20
TRACE: DCR-0032

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" > "$G/t8b-commit.txt" 2>&1
echo "COMMIT_EXIT=$?" >> "$G/t8b-commit.txt"
[ "$(tail -n 1 "$G/t8b-commit.txt")" = "COMMIT_EXIT=0" ] || { echo "REFUSED_AT=pre-commit-hook" > "$M"; exit 1; }
echo "COMPLETED=t8-commitB COMMIT=$(git rev-parse HEAD)" > "$M"
```
Expected: `COMPLETED=t8-commitB COMMIT=<hash>`; `t8b-crate.txt` shows `71 passed` (lib) and `2 passed; 1 ignored` (pin). If a `t8b-workspace-LOADFLAKE-1.txt` exists, both workspace files go in the report. If the workspace stage fails in files this task never touched (other agents' in-flight edits are in the tree), that is `REFUSED_AT=workspace-foreign` territory: report it and stop; do not fix or revert their work.

- [ ] **Step 11: File the remainder ticket — the two divergences that outlive the three-arm alignment.** With `Outside` fixed, two constructible divergences remain — the owner's 2026-08-21 ruling comment on ti `549b20` names both as deliberately unfixed, with measurements — and both live earlier in the tokenizer than the attribute machine this task aligned. **First, HTML's *tag name* state consumes every non-whitespace, non-`/`, non-`>` byte — `=` and quotes included — into the ELEMENT name until the first whitespace**, while the scanner's name loop stops at the first byte outside `[A-Za-z0-9:-]` and attribute-walks the rest. So `<divq"x=" data-sync-id="v">z` is, to a browser, an element *named* `divq"x="` carrying a **real** `data-sync-id` attribute — while the scanner reads tag `divq` and files the plant inside a phantom quoted value its `collect_reserved_attr_spans` walk never examines as a name, leaving it unstripped. Why this is a remainder and not a re-opened bypass: the pane path mounts through DOMPurify fail-closed, and a tag name that diverges *necessarily* contains a byte (`"`, `'`, `=`) no allowlisted element name has — the sanitizer drops the unknown element and its attributes die with it, so no live-anchor construction through the pane path is known. **Verify rather than trust that claim when picking the fix.** Secondary consequences: `balance_fragment` can omit a close the browser would add (or append one the browser reads as orphan junk), and `element_extents` mis-names the element. **Second, HTML's *end-tag-open* state has no counterpart in `scan_tags`**: `</` followed by a non-letter is a parse error that opens a **bogus comment** consuming to the first `>`, while the scanner's `j == name_start` fall-through treats those bytes as plain text and keeps scanning. Measured in headless Chromium:
```text
</1 <div>x  →  html "<!--1 <div-->x"  ·  elements []  ·  comments ["1 <div"]
</ <div>x   →  html "<!-- <div-->x"   ·  elements []  ·  comments [" <div"]
<div>x      →  html "<div>x</div>"    ·  elements ["DIV"]      (control)
```
  The browser creates **zero elements**; the scanner emits `Open{div}` — so `balance_fragment` would append a `</div>` the browser reads as orphan junk after the comment, and `element_extents` would mint a phantom extent wave 6 walks. This is the phantom-structure class (R0003-0066), *not* a bypass, and the strip direction is fail-safe: a plant in that region is comment interior a browser mints no element from. Fix shape, one shared surface (HTML's tag-open / end-tag-open / tag-name states): extend the scanner's tag-name loop to HTML's tag-name state — after the leading ASCII letter, consume every byte until whitespace, `/`, or `>` — and route `</` before a non-letter into the existing bogus-comment skip (the branch `<!` and `<?` already take) instead of the plain-text fall-through, with the same two-bless golden discipline as this task if the corpus gains entries. Write the body to `/Volumes/Temp/claude/ti490d97-wave0/t8-remainder-ticket.md` — title on line 1, then both divergences above with their measurements, the trace, the DOMPurify-drops-unknown-elements argument for the tag-name case, and the comment-interior fail-safe argument for the end-tag-open case. Then:
```bash
ti new -F /Volumes/Temp/claude/ti490d97-wave0/t8-remainder-ticket.md \
  -g "490d97,finding,transync-html,security,scanner" --id-only
```
Record the printed id — Steps 12 and 14 cite it as `<remainder-id>`.

- [ ] **Step 12: Amend DCR-0032 — dated, appended, in ADR-0003's house form — and give the changelog its Fixed entry.**
  1. `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`: append at the **end of the file**, after the final section's last line (locate `## Migration / follow-up` by heading; modify nothing above the append point). The amendment's `###` heading level is deliberate: as in ADR-0003's house form, an amendment nests under the record's final `##` section — here `## Migration / follow-up` — rather than opening a new top-level section. Substitute `<remainder-id>` from Step 11:
```markdown

### Amendment 2026-08-21 (ti 549b20) — the scanner aligns with the browser it was measured against, and an unterminated tag becomes a named region

*Appended, not a rewrite. Everything above stands as written.*

Wave 0 task 6's adversarial review broke `strip_reserved_sync_attrs` on a
constructed input, the owner ruled on the fix, and task 8 landed it —
plus one sibling divergence in the same scanner state, measured into
scope during the task. Three tokenizer divergences, all in
`AttrState::Outside`:

- **A bare quote opened a phantom value.** HTML's tokenizer makes a quote
  not preceded by `=` part of the attribute **name**; the scanner opened
  a quoted value instead. One stray quote desynchronized its quote state
  from every browser's.
- **An unterminated tag aborted the whole scan.** The post-attribute-loop
  `break` left the *document* loop, so every byte after the desync point
  was invisible to the scanner — and to the strip. Measured through the
  vendored DOMPurify 3.2.6: `<div "> <p data-sync-id="v">x</p>` kept a
  live `P#v` anchor the strip never saw, while the well-formed control
  was stripped correctly. That contrast was the finding.
- **A stray `=` opened a value the browser never opens.** HTML's
  *before attribute name* state makes `=` a parse error that **starts an
  attribute named `=`**, and a following quote joins that name. Measured
  in headless Chromium: `<div ="> data-sync-id="v">x</div>` parses as one
  attribute named `="` with the tag ending at the **first** `>` and
  ` data-sync-id="v">x` painted as text (`live_sync_ids` empty — not an
  anchor bypass). A scanner that opens a value there swallows that `>`,
  and the strip then deletes text a browser paints.

The third fix is inside the ruling, not an expansion of it: the chosen
option is "align `AttrState::Outside` with HTML", the `=` transition is
that same arm, and the task's midpoint capture
(`t8-mid-red.txt`, preserved with the wave's gate evidence) shows what
stopping at the two previewed lines produces — the strip turning the
bypass into a *deletion* of painted text, the content-mutation class
invariant 6 forbids. Fixing one divergence in the arm while knowingly
leaving a sibling would also have falsified this amendment's own
rationale sentence, and the golden blast-radius window this task opens
(corpus commit, re-bless, reviewed diff) would have had to open twice.

The fix as landed: a bare quote in `Outside` is an ordinary name byte; a
new `has_attr_name` bit distinguishes HTML's *before attribute name*
state (stray `=` starts an attribute named `=`, no value state) from
*attribute name* / *after attribute name* (`=` after a consumed name
opens the value, the ordinary shape, unaffected); and a tag with no `>`
before EOF is pushed as `TagToken::Skip { span: (start, len) }` before
the scan stops — the passed-over region is named instead of silently
dropped. Blast radius, per consumer: `tag_inventory` filters `Skip` by
construction, so the layer-3 ledger is unaffected; `balance_fragment`
now closes what a browser closes on stray-quote input (`<div "> …` gains
the same `</div>` the sanitized DOM has) and is pinned unchanged on the
other two constructions, whose divs close explicitly; `element_extents`
mints the extents the old scanner never saw, and a truncated tag mints
none — a browser abandons it; the strip reaches both planted attributes
and no longer cuts bytes a browser paints.

**The token-stream pin moved, deliberately, twice.** Task 8 first taught
the corpus the blind spot — three `stray-*` entries blessed from the
broken scanner, which emits zero tokens for all three — then fixed the
scanner and re-blessed through the `TRANSYNC_REGEN_GOLDENS=1` hatch with
the diff reviewed: exactly the three new entries moved in
`token-stream.txt`, exactly one balanced golden moved
(`stray-quote-bare`), and the fifteen pre-existing goldens are
byte-identical. The corpus learning a gap and the fix are separate
commits, so the diff between the two blessings is itself the record of
how tokenization changed.

**Two remainder divergences outlive this amendment, filed together
rather than fixed** (ti `<remainder-id>`; the owner's ruling comment on
ti 549b20 names both as deliberately unfixed). First, HTML's *tag name*
state consumes `=` and quote bytes into the element name until the first
whitespace, where this scanner ends the name at the first byte outside
`[A-Za-z0-9:-]` — so an element a browser names `divq"x="` can carry a
real attribute the scanner files inside a phantom value. Second, HTML's
*end-tag-open* state makes `</` before a non-letter a parse error that
opens a bogus comment consuming to the first `>`, where this scanner's
name fall-through treats the bytes as plain text and keeps tokenizing
markup a browser never mints.

  **Both measured while landing this task, so the record states them
  rather than defers them.** Parsed raw, `<divq"x=" data-sync-id="v">z`
  yields an element `DIVQ"X="` carrying a **real** `data-sync-id` — a
  live anchor in an unsanitized DOM. Through the vendored DOMPurify the
  whole element is gone (`after_sanitize: "z"`, zero live ids), because
  its name can never match an allowlisted tag. So no live-anchor
  construction through the pane path is known — **and the safety is the
  sanitizer dropping an unknown element**, the same mechanism behind the
  custom-element collision measured in wave 6's pane plan (an anchor
  self-injected into an unknown element dies in the DOMPurify mount),
  here working in our favour. That remainder is therefore safe *only
  while every mount sanitizes*, which panes do, fail-closed by design.
  And in headless Chromium `</1 <div>x` parses to a comment `1 <div`
  plus text `x` — zero elements — while the scanner emits `Open{div}`:
  phantom structure (the R0003-0066 class), not a bypass, fail-safe in
  the strip direction because a plant there is comment interior a
  browser mints nothing from. The ticket states both dependencies
  rather than implying either divergence is harmless in itself.

The section *Two things added, one of them with no caller yet* above
describes `strip_reserved_sync_attrs`' guarantee as it stood at wave-0
landing; this amendment is the record that the guarantee was false for
stray-markup constructions until 2026-08-21, and of what made it true.
```
  2. `CHANGELOG.md`: under `## [Unreleased]`, after the final entry of the `### Added` block Task 7 wrote (immediately before the `## [0.4.0]` heading), insert — or, if a `### Fixed` subsection already exists there by the time this runs, append the bullet to it:
```markdown

### Fixed

- `scan_tags` tokenizes malformed attribute regions the way a browser does (ti `549b20`; DCR-0032 amendment 2026-08-21): a bare quote in attribute-name position is a name byte instead of opening a phantom quoted value; a stray `=` starts an attribute named `=` instead of opening a value, so the strip neither misses a plant behind one nor deletes text a browser paints; and a tag left unterminated at EOF becomes a `TagToken::Skip` spanning to end of input instead of a silent whole-suffix abort. One stray byte could previously desynchronize the scanner and hide everything after it from `tag_inventory`, `balance_fragment`, `element_extents` and `strip_reserved_sync_attrs` — including a planted `data-sync-id` that DOMPurify keeps as a live anchor.
```

- [ ] **Step 13: Commit C — the records.** One detached background chain (docs-only, but the hook still runs its ~15 minutes), one commit, marker last. Substitute `<remainder-id>`:
```bash
cd /Volumes/Common/QJoon/transync || exit 1
G=/Volumes/Temp/claude/ti490d97-wave0/gate
M="$G/t8-commitC.marker"
mkdir -p "$G"; rm -f "$M"
git add docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md CHANGELOG.md
git commit -m "docs: DCR-0032 records what the stray-markup divergences cost and what closed them

The dated amendment carries ti 549b20's finding, the owner's ruling, the
measured stray-equals sibling and why fixing it sits inside the ruling
rather than beside it, the midpoint capture that shows the ruled pair
alone turning the bypass into a deletion of painted text, the two-bless
golden protocol, the blast radius on the four scan_tags consumers, and
the two remainder divergences in the tag-name and end-tag-open states -
filed together as <remainder-id> rather than fixed, with the sanitizer
and comment-interior arguments the ticket tells its taker to verify
rather than trust. The changelog gains the
Fixed entry under Unreleased.

TRACE: ti 490d97 wave 0
TRACE: ti 549b20
TRACE: DCR-0032

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>" > "$G/t8c-commit.txt" 2>&1
echo "COMMIT_EXIT=$?" >> "$G/t8c-commit.txt"
[ "$(tail -n 1 "$G/t8c-commit.txt")" = "COMMIT_EXIT=0" ] || { echo "REFUSED_AT=pre-commit-hook" > "$M"; exit 1; }
echo "COMPLETED=t8-commitC COMMIT=$(git rev-parse HEAD)" > "$M"
```

- [ ] **Step 14: Close ti `549b20` with the evidence that closes it, and assemble the report.**
  1. Confirm the ticket is still open and unclaimed by anyone else (`ti show 549b20` — the ti-finish discipline: verify the work, confirm ownership, then close).
  2. Write the closure evidence to `/Volumes/Temp/claude/ti490d97-wave0/t8-close-evidence.md`, substituting the three commit hashes from the markers and `<remainder-id>`:
```markdown
Closed by ti 490d97 wave 0 task 8. The ruled pair landed exactly as
written, plus the sibling `=` divergence in the same `AttrState::Outside`
arm, measured into scope (headless Chromium) and fixed in the same task.

Fix (commit <B>): a bare quote in `Outside` is an ordinary name byte; a
stray `=` starts an attribute named `=` instead of opening a value
(`has_attr_name` distinguishes HTML's before-attribute-name state from
attribute-name, so ordinary `name=value` is untouched); a tag with no
`>` before EOF is pushed as `TagToken::Skip { span: (start, len) }`
before the scan stops.

Evidence:
- strip_tests::a_stray_quote_cannot_hide_a_planted_sync_attr and
  strip_tests::a_doubled_quote_cannot_hide_a_planted_sync_attr — red
  before the fix (both inputs came back byte-unchanged, plants intact),
  green after: `<div "> <p data-sync-id="v">x</p>` -> `<div "> <p>x</p>`
  and `<div a="x"" data-sync-id="v">y</div>` -> `<div a="x"">y</div>`.
- strip_tests::text_the_browser_paints_is_never_cut_by_the_strip —
  `<div ="> data-sync-id="v">x</div>` survives byte-identical, matching
  Chromium (one attribute named `="`, tag ends at the first `>`, the
  plant is painted text, live_sync_ids empty). The midpoint capture
  t8-mid-red.txt shows the ruled pair alone DELETING those painted
  bytes, leaving `<div =">>x</div>` — the swallowed first `>` left
  behind beside the tag's own — the measured reason the third arm was
  fixed here and not deferred.
- The token-stream pin moved and the movement was reviewed, not blessed
  away (commit <A> taught the corpus the three constructions and pinned
  the BROKEN behaviour — empty stream, balancer pass-through; commit <B>
  re-blessed through TRANSYNC_REGEN_GOLDENS=1): the diff touched exactly
  the three stray-* sections of token-stream.txt and
  balanced/stray-quote-bare.txt, which gained the same `</div>`
  DOMPurify 3.2.6 was measured to add. The other two balanced goldens
  and the fifteen pre-existing goldens are byte-identical, as predicted
  from the corpus's and fixtures' verified freedom from Outside-state
  quotes, stray `=`, and unterminated tags.
- Blast radius, per consumer: tag_inventory — Skip filtered by
  construction, ledger unchanged (pinned by
  inventory_tests::the_ledger_never_carries_a_skipped_suffix);
  balance_fragment — closes what the sanitized DOM closes on stray-quote
  input, inert on the other two constructions; element_extents — mints
  the extents the old scanner never saw, a truncated tag mints none;
  strip — reaches both plants and cuts nothing a browser paints.
- Remainders, both filed together as ti <remainder-id> (the owner's
  ruling comment names both as deliberately unfixed): HTML's tag-name
  state consumes `=`/quote bytes into the ELEMENT name where this
  scanner ends the name earlier (no live-anchor construction known
  because a diverging name cannot match a sanitizer allowlist), and
  HTML's end-tag-open state opens a bogus comment on `</` before a
  non-letter where this scanner sees plain text and keeps tokenizing
  (phantom structure, fail-safe in the strip direction: a browser mints
  no element from a bogus comment's interior) — verify, don't trust;
  DCR-0032 amendment 2026-08-21 (commit <C>) records finding, ruling,
  scope argument, midpoint evidence, protocol and remainders.
```
  3. Then:
```bash
ti comment -t 549b20 "$(cat /Volumes/Temp/claude/ti490d97-wave0/t8-close-evidence.md)"
ti close 549b20
ti show 549b20
```
  Expected: the final `ti show` prints `Status` resolved/closed with the comment attached.
  4. **The task's report** (to the wave controller) carries: the three `COMPLETED=` markers with hashes; `t8-mid-red.txt` (the ruled pair alone turning the bypass into a deletion), `t8-pin-red.txt` (the instrument reading the full fix) and `t8-golden-diff.txt` (the reviewed movement) verbatim; the Step 5 red capture; any `-LOADFLAKE-` pair from Step 10, both files; the remainder ticket id; and an explicit statement of the safety property's outcome — which golden entries moved (all three `stray-*` sections in `token-stream.txt`; `stray-quote-bare` alone in `balanced/`), which were pinned unmoved (`stray-quote-doubled` and `stray-equals` in `balanced/`), and that the fifteen wave-0-era entries did not move at all. The report also carries one cross-plan hazard for the controller: Step 12's CHANGELOG guard is one-directional — wave 1's plan (`2026-08-20-html-wave1-oi0035-anchor-trust.md`, Task 4 Step 5) inserts its own `### Fixed` immediately before `## [0.4.0]` with no already-exists guard, so with Task 8 landed first its literal execution would duplicate the heading. The controller files the one-line wave-1 correction; do not edit wave 1's plan from this task.
---

## Wave acceptance — check all five before declaring wave 0 done

**Read the fourth and fifth items together.** This section was written when the wave had seven tasks, all of which froze the token-stream goldens; Task 8 was added afterwards and *deliberately moves them*, twice. The pin criterion therefore splits: frozen across Tasks 1–7, moved-with-evidence at Task 8. Nothing about the pin's purpose changed — a golden that moves without a reviewed diff is still the failure it was always guarding against.

- [ ] Full workspace suite green with **zero fixture or expectation edits**: `git diff --stat <baseline-commit>..HEAD -- 'crates/*/tests/fixtures' 'crates/*/tests/scenarios' 'web/tests'` prints nothing. (`<baseline-commit>` is in `/Volumes/Temp/claude/ti490d97-wave0/gate/baseline-commit.txt`.)
- [ ] The two-package wasm gate exit 0, captured bare-to-file, string unchanged: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`.
- [ ] `workspace_publication.rs` green with the seven-member roster in dependency order.
- [ ] **Across Tasks 1–7:** the `tag_inventory`-unchanged pin green with its goldens byte-identical to the ones generated before the token change (`git diff --stat e52ea44..<task-7-head> -- crates/transync-html/tests/goldens` prints nothing), and a reviewer-checkable diff whose only non-mechanical hunks are the splice signature, the policy constructor, the two token changes, and the visibility changes.
- [ ] **At Task 8, the goldens move — and the movement is the deliverable, not a waiver.** Two blessings through the env-var-interlocked `regenerate_goldens` hatch, each committed on its own: the first teaches the corpus three constructions it was blind to (so the goldens capture the *broken* tokenization), the second records what the fix changed. Both diffs must be reported and reviewed, and **the fifteen wave-0-era entries must not move at either blessing** — any pre-existing golden moving is a blast-radius finding and a STOP (`REFUSED_AT=golden-blast-radius`), never a re-bless. The pin is doing its job in both directions here: it stayed silent for seven tasks because nothing changed, and it speaks at Task 8 because something did.


---

## Notes for the implementer

- **A crate extraction is not TDD-shaped, and this plan does not pretend otherwise.** Tasks 3–6 are red-first because they add behaviour. Task 1 is *characterized*: the existing suite is the test, the baseline capture in Step 1 is the "before", and Step 17's empty fixture diff is what makes "unchanged" checkable rather than hoped for. Task 2 exists because the one change in this wave that could silently move a shipped consumer — the token stream — has no natural red test, so it gets a golden generated from the pre-change code instead.
- **The one behavioural change in the wave is the bogus-comment state** (Task 4). It is browser-correct and it is inert over the corpus (verified: the three HTML-bearing fixtures contain only `<!--` comments), but it is not a no-op in general: `<! <div> >` no longer tokenizes the `<div>`. If the Task 2 pin ever goes red on it, that is the pin doing its job.
- **Do not run `regenerate_goldens` to make a red pin green.** It is `#[ignore]`d for that reason; the legitimate reasons to run it are a deliberate change to `FIXTURES` / `EDGE_CASES` — and Task 8's two sanctioned blessings, the named and bounded exception for a deliberate, reviewed tokenizer change: corpus commit first, fix commit second, both diffs reviewed and reported. Everywhere else the prohibition keeps its full force.
- **`transync-html` gets no `[features]` table and no workspace-member dependency**, ever. `transync-syntax` keeps both prohibitions plus the no-`transync-core`-dev-dependency rule.
