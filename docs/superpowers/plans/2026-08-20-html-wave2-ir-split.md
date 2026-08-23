# HTML→HTML Wave 2 — The IR Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Separate what a block *is* from how it was *written*, so an HTML document's blocks can carry real semantic kinds — and do it in one sanctioned v0.5.0 breaking window, changing nothing a user can see.

**Architecture:** `BlockKind` keeps the semantic vocabulary and loses its one piece of spelling metadata; a new `Spelling` enum beside it says how the source wrote the block; `Block` gains `spelling`, `Document` gains `format`. `BlockKind::Html { block_type: u8 }` narrows to a unit variant meaning "HTML content with **no semantic equivalent**", and `BlockKind::Title` is added for D5's anchor-less page title. Everything downstream that keyed on kind-`Html` moves to the axis it actually meant — `Spelling::Html` for IR-side questions, `InputMode::HtmlSegments` for unit-side ones, `constraints.html` for the layer-3 splice — and because kind-`Html` ⇔ mode-`HtmlSegments` ⇔ spelling-`Html` all hold today on the Markdown path, every one of those moves is **behaviour-preserving**. The one genuinely new behaviour is the `Title` variant, whose two most important consumers sit behind catch-all arms that would have swallowed it silently; those get red-first tests that assert the *answer*, not the compile.

**Tech Stack:** Rust 2024 workspace (rustc ≥ 1.88), `transync-html` (landed in wave 0), comrak 0.27, `serde`/`serde_json`, siphasher, tiktoken-rs, `wasm32-unknown-unknown` check gate, tracked pre-commit hook (fmt + clippy + wasm gate + rustdoc gate).

**Spec:** docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md

**Depends on:** **wave 0, complete** (`docs/superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md`) — all **eight** of its tasks, not just the crate move (Task 8 was added 2026-08-21 in `d2dac83`; "seven" here was correct when written and corrected 2026-08-23). Task 1 Step 2 is a hard precondition gate on that, because this plan's regen and validate edits call `transync_html::BlankLinePolicy`, which wave 0 Task 3 introduces. The spec's dependency graph is `0 → 2 → {3, 4} → 5 → 6 → 7`, with wave 1 parallel to 2–5. **Nothing after this wave can start without it**, and it is the only part of the feature that cannot slip past a version window.

**Overlap with wave 1, which may or may not have landed:** wave 1 edits `web/js/sync.js` (both copies) and the `BlockKind::Html` success arm in `crates/transync-syntax/src/render.rs`. This plan touches **no JS at all**, and its single edit to that render arm is to the arm's *head* (`if let BlockKind::Html { .. } = kind` → `if matches!(kind, BlockKind::Html)`), never its body. Task 4 Step 9 says so explicitly. The two waves are otherwise disjoint; either order works.

## Global Constraints

- **Acceptance gate for the whole wave: the full workspace suite green with ZERO fixture edits.** Not one byte under `crates/*/tests/fixtures/`, and not one *expected value* in an existing assertion. This is the wave's whole claim — it is breaking by policy and behaviour-preserving in fact — so a fixture or expectation that has to move is a plan failure, not a fix. Stop and report. Task 8 Step 12 checks it with a real command over a recorded baseline commit; two named, bounded exceptions are declared in *Deviations from the spec* below and nowhere else.
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`. `Spelling` lives in `transync-syntax::id` and names `transync_html::BlankLinePolicy`; that edge already exists and already compiles for `wasm32`, so the gate string does not move and no package joins the command.
- `transync-syntax` may gain **no `[features]` table** and **no `transync-core` dependency, dev-dependencies included** — either breaks the gate that lives in `scripts/hooks/pre-commit` and `scripts/smoke.sh`. `transync-html` keeps the same two prohibitions and its `lol_html` + `htmlize` dependency set.
- Every test run: `cargo test -p <crate> -- --test-threads=4`; workspace runs: `cargo test --workspace -- --test-threads=4`. **Never raise the cap.**
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append the exit code, and inspect the file as a *separate* step. A pipeline reports the last stage's status, so a failing suite reads as a pass, and `tail -N` over filtered lines drops early failures.
- Lint gates (the pre-commit hook enforces them): `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- **`git commit --no-verify` is never used.** A commit step's expected result is that the hook runs fmt, clippy, the wasm gate and the rustdoc gate, and all four pass. If the hook blocks, fix the cause. (The hook's JS leg reports `SKIP: prettier not installed` / `SKIP: eslint not installed` — normal output here, not a failure.)
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave2/` — never `/tmp`, never `/private/tmp`, never `$TMPDIR`, never the OS default. If `/Volumes/Temp/claude` is unreachable, stop and ask.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`. If a cargo command fails because the target dir is unreachable, stop and ask.
- No pure-formatting edits. Let `cargo fmt` own wrapping — where this plan shows wrapped Rust, run `cargo fmt --all` afterwards and take the formatter's answer.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, cite, or create them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **A new document under `docs/` turns `docs_index_drift` red the moment it exists**, because that test walks the filesystem and requires every `.md` under `docs/` to be linked from `docs/index.md`. **The link must land in the same change that creates the file** — wave 0 paid for this lesson twice and its plan states it. This wave creates three documents (this plan, ADR-0025, DCR-0034), and each one's index link rides the commit that creates it, never a later task. **This plan is itself such a document:** the planning agent that wrote it was not permitted to touch `docs/index.md`, so Task 1 Step 4 adds the link and Step 6 commits both together.
- **`BlockKind` stays exhaustive: no `#[non_exhaustive]`, and no `_ =>` catch-all is ADDED to any match over it.** The two catch-alls this wave has to work around (`align::sync_role_for`, `walk::label_for`) are pre-existing and stay; the point of Task 6 is that a new variant must not disappear into one silently. Adding a third would be adding the next silent swallow.

---

- **The corpus gate's pathspec ends in `/*`, and it must stay that way (added 2026-08-23, ti `490d97` wave 1).** `git diff -- 'crates/*/tests/fixtures'` matches **nothing** — git runs the pattern against the whole path and `*` matches `/`, but the pattern ends at `fixtures`, so only a path *ending* there could match, and every corpus file is one level deeper. Waves 0 and 1 both ran this gate in its vacuous form; both verdicts happened to survive re-checking, which is luck, not evidence. Before trusting any run of it, prove the selector selects something:
```bash
git ls-files -- 'crates/*/tests/fixtures/*' | wc -l   # must be > 0
```
  A filter that matches nothing and a filter whose subject is clean both print silence. That is why the assertion above exists and why the `/*` is not a typo to tidy away.

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `crates/transync-syntax/src/id.rs` | modify | the two new vocabularies (`Spelling`, `SourceFormat`), `Spelling::blank_line_policy_for`, the `Html` narrowing, the `Title` variant, and its `id_code` / `wire_str` arms |
| `crates/transync-syntax/src/parser.rs` | modify | `Block.spelling`, `Document.format`, `parse`'s `SourceFormat::Markdown` stamp, the `HtmlBlock` arm's `emit_html_here` call, the spelling-stamp invariant test, the two html-block characterization tests moved onto the spelling axis |
| `crates/transync-syntax/src/parser/emit.rs` | modify | `emit` takes a `Spelling`; `emit_here` is the Markdown-spelled case; `emit_html_here` is the ONE home of the island's two-axis stamp |
| `crates/transync-syntax/src/regen.rs` | modify | the two format-aware arms re-keyed on spelling; the hand-built-`Block` test literal |
| `crates/transync-syntax/src/render.rs` | modify | the `BlockKind::Html` arm head, `wrapper_element_for`'s `Title` arm, the hand-built-`Block` test literal, one test filter |
| `crates/transync-syntax/src/align.rs` | modify | `sync_role_for`'s explicit `Title => NonSync` arm **and its test**; the extraction-failed arm re-keyed on spelling; the hand-built-`Block` test literal |
| `crates/transync-syntax/src/outcome.rs` | modify | `html_outcomes` and `is_translatable_block` iterate spelling; the kind-level exclusions become the Markdown arm; the `Title` translatability pin |
| `crates/transync-syntax/src/walk.rs` | modify | `label_for_pins_every_kind` gains the `Title` row (15 → 16) |
| `crates/transync-core/src/unit/payload.rs` | modify | `assemble` branches on **spelling first**; the `block_type = 0` sentinel; the `heading_level` pin gains `Title` |
| `crates/transync-core/src/unit/context.rs` | modify | `document_title` prefers a `Title` block **and its tests** |
| `crates/transync-core/src/unit/split.rs` | modify | the explicit `(Table × Html)` guard **and its test** |
| `crates/transync-core/src/unit/section.rs` | modify | one test's `BlockKind::Html` construction |
| `crates/transync-core/src/unit.rs` | modify | `html_dominance_warning`'s html-mass predicate re-keyed on spelling; the hand-built-`Block` test literal |
| `crates/transync-core/src/validate.rs` | modify | layer 3 keyed on `constraints.html` with the mode as debug-assert twin; the two layer-call sites' new arguments |
| `crates/transync-core/src/validate/per_kind.rs` | modify | the new first arm on `InputMode::HtmlSegments`, the module-doc rationale, the `Title` arm, nine test call sites |
| `crates/transync-core/src/validate/fragment_reparse.rs` | modify | the early return keyed on the mode; the `Title` label arm |
| `crates/transync-core/src/validate/schema.rs` | modify | `check_payload_bytes` takes the mode instead of the kind; four tests |
| `crates/transync-core/src/validate/inline.rs` | modify | the skip set keyed on the mode; the html-skip test strengthened so it is not vacuous |
| `crates/transync-core/src/validate/full_reparse.rs` | modify | two `BlockKind::Html { .. }` patterns lose their braces |
| `crates/transync-core/src/llm.rs` | modify | `HtmlSegmentConstraints::block_type`'s amended domain doc (1–7 island, 0 = HTML document) |
| `crates/transync-core/src/llm/prompt.rs` | modify | `has_html_unit` keyed on the mode; one test's `BlockKind::Html` construction |
| `crates/transync-core/src/batch.rs`, `crates/transync-core/src/pipeline.rs` | modify | one `BlockKind::Html` construction each |
| `Cargo.toml` (root) | modify | `[workspace.package] version` → `0.5.0-dev` **and the five internal requirements that do not inherit it** |
| `Cargo.lock` | modify | regenerated by cargo, staged by hand |
| `docs/decisions/0025-html-to-html-document-translation.md` | **create** | ADR-0025 — the twelve decisions, the rejected alternatives, the accepted limitations, D11's deferral, the `htmlseg` → `transync-html` name-continuity note |
| `docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md` | **create** | the wave's record |
| `docs/architecture/contracts.md` | modify | §0 prose paragraph for the four breaking changes; §1's `BlockKind` stability bullet |
| `docs/architecture/rough-schema.md` | modify | the `BlockKind` sketch, the id-prefix set, the `Document`/`Block` IR sketch, the two new vocabularies |
| `docs/implementation/module-map.md` | modify | `id.rs`'s one-line description |
| `CLAUDE.md` | modify | architectural invariant 1's replacement wording (spec §10) |
| `CHANGELOG.md` | modify | the `[Unreleased]` window paragraph + a `### Changed (BREAKING)` block |
| `docs/index.md` | modify (Tasks 1, 8) | this plan's link (Task 1); ADR-0025's and DCR-0034's (Task 8) |
| `docs/project/status.md`, `docs/project/phase-state.yaml` | modify | the workspace-version line, the open-window statement, the per-wave record |

**Not touched, deliberately:** `web/js/sync.js` and its embedded twin (wave 1 owns them; this wave changes no JS); `crates/transync-html/src/lib.rs` (wave 0 owns the mechanics; wave 2 only *calls* `BlankLinePolicy`); `crates/transync/tests/public_surface.rs` (no §0 row moves — see the deviation below); `crates/transync-syntax/src/parser/classify.rs` (it maps nodes to *kinds*, and the kind→spelling pairing deliberately lives at the emit site instead); `docs/architecture/source-of-truth-table.md` (grepped: it names no `BlockKind` fact); `scripts/lib/rustdoc-gate.sh` (no crate joins or leaves).

---

## Deviations from the spec

Four, each a scope call with a mechanical reason. Undeclared divergence is what makes a reviewer distrust a plan that was right on everything else.

1. **`SourceFormat` derives `Default` (`#[default] Markdown`), which spec §3's snippet does not list.** **Why:** `parser::Document` derives `Default`, and `#[derive(Default)]` on a struct requires `Default` on every field's type — without it, adding `format: SourceFormat` is an `E0277` at the derive, and `regen.rs`'s `..Document::default()` test literal stops compiling. The alternative, a hand-written `impl Default for Document`, replaces a derive with a body that must be maintained in lockstep with four other fields. `Markdown` is the only honest default: a `Document::default()` was an empty **Markdown** document before this field existed, and every producer that is not the Markdown intake sets the field explicitly.
2. **No §0 row for `SourceFormat` in this wave**, though spec §10 lists one. **Why:** `public_surface.rs`'s anti-vacuity check asserts that every §0 row naming a facade *root* item is a name the `lib.rs` scrape actually found (`unseen` must be empty). `SourceFormat` is not re-exported by `transync` in wave 2 — it reaches the facade with the `TranslateOptions` input-format option, and the row lands with the export, which is the same rule §0 states for every other row. (Corrected 2026-08-20: this deviation said the export arrives in **wave 6**, alongside `AlignmentMap.input_format`. It arrives in **wave 5**. The rule stated here is untouched and is what forces the correction — wave 5's demonstrable outcome is `translate()` returning translated HTML, so the option, its export and its §0 row all land there. `AlignmentMap.input_format` is still wave 6's; the two simply stopped being the same wave. Nothing in wave 2 changes: the row was not going to be written here either way.)
3. **`Document.format` lands as a field + invariant, not as a refusal.** Spec §3 says the field "lets format-committed consumers refuse the wrong document"; spec §12 assigns the actual branch to **wave 4** (`finalize_regen_with_reparse_policy`) and the pane/render half to **wave 6**. **Why here:** nothing constructs a `format == Html` document until wave 3's intake exists, so a refusal written now has no reachable trigger and no honest test — it would be untested code claiming to be a gate. Wave 2 ships the fact; waves 4 and 6 ship the branches that read it.
4. **Two test *inputs* are strengthened, and they are the only source-file test edits in this wave that are not mechanically forced.** (a) `validate::inline::html_units_skip_the_inline_layer` gets a source payload carrying a raw tag, because after the re-key it would otherwise pass whether or not the skip fired — the exact way a re-key silently de-fangs a test. (b) `validate::schema::a_nul_free_payload_passes_on_every_kind` is renamed to `…_in_every_input_mode` and iterates modes, because `check_payload_bytes` no longer takes a kind. **No expected value changes in either.** Both are named here so the zero-fixture-edit check in Task 8 Step 12 stays exactly what it claims to be.

---

### Task 1: Preconditions, baseline, and this plan's index link

**Files:**
- Modify: `docs/index.md`
- Commit (already on disk, untracked): `docs/superpowers/plans/2026-08-20-html-wave2-ir-split.md`
- Test: none. This task is **characterized** — its whole product is a recorded green "before".

**Interfaces:**
- Consumes: wave 0's complete public surface — `transync_html::{extract, splice, tag_inventory, balance_fragment, BlankLinePolicy, TagToken, ElementExtent, element_extents, strip_reserved_sync_attrs, scan_tags, is_void, is_raw_text, implicitly_closes}`.
- Produces: `/Volumes/Temp/claude/ti490d97-wave2/gate/baseline-commit.txt`, the commit every later "zero fixture edits" check diffs against.

- [ ] **Step 1: Create the wave's temp directory.**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave2/gate
```
Expected: no output, exit 0. If the path cannot be created because `/Volumes/Temp/claude` is unreachable, **stop and ask the user** — do not fall back to `/tmp`.

- [ ] **Step 2: Hard precondition gate — wave 0 must be COMPLETE, not merely started.** Wave 0's crate move (`1d6f19d`) is its Task 1; Tasks 2–7 add the four things this plan calls. Check for all four plus the record:
```bash
grep -c 'pub enum BlankLinePolicy' crates/transync-html/src/lib.rs
grep -c 'Skip *{' crates/transync-html/src/lib.rs
grep -c 'pub fn element_extents' crates/transync-html/src/lib.rs
grep -c 'pub fn strip_reserved_sync_attrs' crates/transync-html/src/lib.rs
ls docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md
```
Expected: each count **≥ 1**, and the path echoed. **Any `0`, or a missing DCR, means wave 0 is unfinished — STOP.** Two notes on reading this gate: `grep -c` *exits non-zero* on a zero count, so do not run these under `set -e` or the shell will abort before printing the later lines; and a count above 1 is fine — a doc comment naming the symbol alongside its definition is not a defect, absence is. This plan's Task 4 and Task 5 both call `transync_html::BlankLinePolicy::from_commonmark_html_block_type`, which does not exist until wave 0 Task 3; writing this wave against the old `splice(block, translated, block_type: u8)` signature would make every code block in it wrong and would have to be undone when wave 0 finishes.

- [ ] **Step 3: Capture the baseline, bare-to-file.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-cli.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-wasm.txt
```
Then, as a **separate** step, read the last line of each file. Expected: `CARGO_EXIT=0` in all three, and no line containing `FAILED`. If any is non-zero, STOP — the tree was not green before this wave, and nothing after this point can be attributed to the IR split. If the failure is `docs_index_drift`, read the failure's file list and link what it names before continuing (that test walks the filesystem, so an unlinked `.md` anywhere under `docs/` is red).

- [ ] **Step 4: Verify this plan's link in `docs/index.md` — it is already there.** `docs_index_drift` walks the filesystem, so an unlinked `.md` under `docs/` is red from the moment the file is written; the controller therefore linked this plan when it created it, rather than leaving Step 3's baseline to STOP against a document this very plan is. Confirm the line below is present immediately **after** the wave 1 line (the one beginning `- [HTML→HTML Wave 1 — OI-0035 anchor trust — implementation plan (2026-08-20)]`) and matches word for word. If it is missing, add it; if it differs, the drift is real and belongs in Task 8's DCR:
```markdown
- [HTML→HTML Wave 2 — the IR split — implementation plan (2026-08-20)](superpowers/plans/2026-08-20-html-wave2-ir-split.md) — the 8-task plan for the breaking window: `Spelling` and `SourceFormat` join `BlockKind` in `transync-syntax::id`, `Block` gains `spelling` and `Document` gains `format`, `BlockKind::Html` narrows to a unit variant and `BlockKind::Title` is added with explicit `sync_role_for` / `document_title` arms, every dispatch site is re-keyed onto the axis it actually meant, and the workspace goes to `0.5.0-dev`. Breaking by policy, behaviour-preserving in fact: the acceptance gate is the suite green with zero fixture edits.
```

- [ ] **Step 5: Record the baseline commit — BEFORE the commit below, so the diff in Task 8 covers this wave and nothing else.**
```bash
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-commit.txt
cat /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-commit.txt
```
Expected: one 40-character SHA.

- [ ] **Step 6: Verify the index weld, then commit the plan and its link together.**
```bash
cargo test -p transync --test docs_index_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t1-index.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t1-index.txt
```
Expected: `CARGO_EXIT=0`. A red here names an unlinked document; link it.
```bash
git add docs/superpowers/plans/2026-08-20-html-wave2-ir-split.md docs/index.md
git commit -m "docs(plan): wave 2 gets its plan, and the index gets its link in the same commit

docs_index_drift walks the filesystem under docs/ and requires every .md it
finds to be linked from docs/index.md, so writing the file is itself what
turns the weld red. File and link are one change; the commit is what makes
that true for anyone but the author.

TRACE: ti 490d97 wave 2

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: `Spelling` and `SourceFormat` — the two vocabularies, with no consumers yet

**Files:**
- Modify: `crates/transync-syntax/src/id.rs`
- Test: new in-crate module `spelling_tests` in the same file (red-first, and the red is real: the types do not exist)

**Interfaces:**
- Consumes: `transync_html::BlankLinePolicy` and `BlankLinePolicy::from_commonmark_html_block_type` (wave 0 Task 3).
- Produces, for Tasks 3–8 and every later wave:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
  pub enum Spelling {
      Markdown,
      Html { block_type: Option<u8> },
  }
  impl Spelling {
      pub fn blank_line_policy_for(block_type: Option<u8>) -> transync_html::BlankLinePolicy;
  }

  #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
  #[serde(rename_all = "kebab-case")]
  pub enum SourceFormat { #[default] Markdown, Html }
  ```
- Nothing else changes in this task. `BlockKind` is untouched; `Block` and `Document` do not know these types yet.

- [ ] **Step 1: Write the failing tests.** Append to `crates/transync-syntax/src/id.rs`, after the existing `mod rekey_tests` block:
```rust
// ti 490d97 wave 2 (spec §3, decision D2): semantic kind and source spelling
// are orthogonal axes. These pin the new vocabularies before anything
// consumes them — the projections a later variant or a later intake must
// extend in lockstep.
#[cfg(test)]
mod spelling_tests {
    use super::*;
    use transync_html::BlankLinePolicy;

    #[test]
    fn spelling_is_copy_and_carries_the_commonmark_type_only_for_an_island() {
        let island = Spelling::Html {
            block_type: Some(6),
        };
        // `Copy`, so a consumer that reads it does not move it out of a Block.
        let copied = island;
        assert_eq!(copied, island);
        assert_ne!(island, Spelling::Html { block_type: None });
        assert_ne!(island, Spelling::Markdown);
    }

    #[test]
    fn the_splice_policy_is_the_spellings_to_decide_and_it_is_total() {
        // An island inside a Markdown document takes the CommonMark rule,
        // which wave 0 moved into its one home.
        for t in 0u8..=7 {
            assert_eq!(
                Spelling::blank_line_policy_for(Some(t)),
                BlankLinePolicy::from_commonmark_html_block_type(t),
                "an island of type {t} must defer to the CommonMark mapping",
            );
        }
        // A block of an HTML document has no CommonMark context, and nothing
        // ever Markdown-reparses the output, so a blank line terminates
        // nothing: Keep, always.
        assert_eq!(
            Spelling::blank_line_policy_for(None),
            BlankLinePolicy::Keep,
            "collapse is a CommonMark rule; an HTML document has no CommonMark",
        );
        // The two agree on the sentinel `constraints.html.block_type` carries
        // for an HTML-document unit, which is what lets `validate`'s layer 3
        // reach the same answer from a `u8` it cannot distinguish from a
        // missing value.
        assert_eq!(
            BlankLinePolicy::from_commonmark_html_block_type(0),
            Spelling::blank_line_policy_for(None),
        );
    }

    #[test]
    fn source_format_wire_form_is_kebab_case_in_both_directions() {
        assert_eq!(
            serde_json::to_string(&SourceFormat::Markdown).expect("serializes"),
            "\"markdown\"",
        );
        assert_eq!(
            serde_json::to_string(&SourceFormat::Html).expect("serializes"),
            "\"html\"",
        );
        assert_eq!(
            serde_json::from_str::<SourceFormat>("\"html\"").expect("deserializes"),
            SourceFormat::Html,
        );
        // The default is what an empty `Document` means, and it is the format
        // every document had before this type existed.
        assert_eq!(SourceFormat::default(), SourceFormat::Markdown);
    }
}
```

- [ ] **Step 2: Run it and see it fail.**
```bash
cargo test -p transync-syntax spelling_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t2-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t2-red.txt
```
Read the file. Expected: `CARGO_EXIT=101`, and compile errors — `error[E0433]: failed to resolve: use of undeclared type `Spelling`` and `error[E0433]: failed to resolve: use of undeclared type `SourceFormat``. A compile error **is** the red here; there is nothing to run yet.

- [ ] **Step 3: Add `Spelling`, immediately after the `BlockKind` `impl` block** (so the three projections — `id_code`, `heading_level`, `wire_str` — stay together above it) in `crates/transync-syntax/src/id.rs`:
```rust
/// How the source **spelled** a block — the axis orthogonal to [`BlockKind`].
///
/// [`BlockKind`] says what a block *is* (a level-2 heading, a table, a list
/// item); this says how the source *wrote* it. The two were conflated until ti
/// `490d97` wave 2: `BlockKind::Html { block_type }` meant both "no semantic
/// classification" and "spelled as HTML", which is harmless while every
/// document is Markdown and wrong the moment an HTML document's `<h1>` has to
/// be a heading. Splitting the axes is what lets that `<h1>` be
/// [`BlockKind::Heading1`] and keep `h1-0001`, its section scope, and its
/// heading context, while still translating through the segment engine.
///
/// [`Copy`] on purpose: it is two words at most, every consumer wants it by
/// value, and a `Block` field that had to be borrowed would make the dispatch
/// sites noisier for nothing.
///
/// TRACE: ADR-0025
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spelling {
    /// GFM syntax. Every consumer that reparses the block under comrak —
    /// `validate::fragment_reparse`, `validate::full_reparse`, the Markdown
    /// renderer, `unit::context`'s heading projection — lives behind this arm.
    Markdown,
    /// Raw HTML markup, translated through text-segment extraction and
    /// positional splice-back (ADR-0018). The markup itself never reaches the
    /// model.
    Html {
        /// The CommonMark HTML block type (1–7) comrak reported, when the
        /// block is a raw-HTML **island inside a Markdown document**. It is
        /// the only input the blank-line splice policy takes.
        ///
        /// `None` for every block of an **HTML document**, where no CommonMark
        /// context exists and blank-line collapse must never run: collapsing
        /// exists because a blank line terminates a CommonMark HTML block of
        /// type 6/7 at reparse, and nothing ever Markdown-reparses an HTML
        /// document.
        block_type: Option<u8>,
    },
}

impl Spelling {
    /// THE spelling→splice-policy mapping, and the only place the
    /// HTML-document case is decided.
    ///
    /// An island defers to [`BlankLinePolicy::from_commonmark_html_block_type`]
    /// — wave 0's one home for the `6 | 7` rule. A block of an HTML document
    /// answers [`BlankLinePolicy::Keep`], because "a blank line terminates
    /// this block" is a statement about the *host format* and an HTML document
    /// has no such rule.
    ///
    /// Takes the `Option<u8>` rather than `&self` so the caller that has
    /// already destructured `Spelling::Html { block_type }` in a match arm can
    /// use it without a second match and without an `expect` on the
    /// [`Spelling::Markdown`] arm, which never splices.
    ///
    /// [`BlankLinePolicy::from_commonmark_html_block_type`]: transync_html::BlankLinePolicy::from_commonmark_html_block_type
    /// [`BlankLinePolicy::Keep`]: transync_html::BlankLinePolicy::Keep
    pub fn blank_line_policy_for(block_type: Option<u8>) -> transync_html::BlankLinePolicy {
        match block_type {
            Some(t) => transync_html::BlankLinePolicy::from_commonmark_html_block_type(t),
            None => transync_html::BlankLinePolicy::Keep,
        }
    }
}
```

- [ ] **Step 4: Add `SourceFormat`, immediately after `Spelling`:**
```rust
/// Which intake produced a document — the plain two-value format label.
///
/// Distinct from [`Spelling`] and doing a different job. Spelling is
/// **per block**, and it has to be: the Markdown parser's `HtmlBlock` arm
/// interleaves HTML-spelled blocks with Markdown-spelled ones inside one
/// document, so no document-level bit can carry that axis. This label is
/// per *document*, and it exists so a format-committed consumer can **refuse**
/// the wrong document instead of silently producing garbage — comrak over an
/// HTML document yields *some* node sequence and would pass a check that is
/// checking nothing.
///
/// Invariant, established at intake: `Html` ⟹ every block's spelling is
/// `Spelling::Html { block_type: None }`; `Markdown` ⟹ spellings are mixed,
/// and every HTML island carries `Some(t)`.
///
/// The wire form is kebab-case (`"markdown"` / `"html"`). It reaches the wire
/// in a later wave as the alignment map's `input_format` and each row's
/// `source_format`; the spelling is fixed here so those two cannot be spelled
/// differently when they arrive.
///
/// [`Default`] is [`SourceFormat::Markdown`] because [`crate::parser::Document`]
/// derives `Default` and an empty document was a Markdown document before this
/// field existed. Every producer that is not the Markdown intake sets the
/// field explicitly.
///
/// TRACE: ADR-0025
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFormat {
    #[default]
    Markdown,
    Html,
}
```

- [ ] **Step 5: Run the tests and see them pass.**
```bash
cargo test -p transync-syntax spelling_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t2-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t2-green.txt
```
Expected: `CARGO_EXIT=0`, `test result: ok. 3 passed`.

- [ ] **Step 6: Verify the whole tree, including the wasm gate, then commit.** The wasm gate matters here specifically: `id.rs` now names a `transync_html` type, and `transync-syntax` is one of the two packages the gate builds.
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave2/gate/t2-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t2-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave2/gate/t2-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t2-wasm.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t2-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t2-workspace.txt
```
Expected: `CARGO_EXIT=0` in all three.
```bash
git add crates/transync-syntax/src/id.rs
git commit -m "feat(id): semantic kind gets a sibling axis, and it is called Spelling

BlockKind says what a block IS; Spelling says how the source WROTE it. The two
were one thing — BlockKind::Html { block_type } meant both 'no semantic
classification' and 'spelled as HTML' — which is harmless while every document
is Markdown and wrong the moment an HTML document's <h1> has to be a heading.

SourceFormat is the per-document twin, and it does a different job: spelling is
per block because one Markdown document interleaves both, while the format
label is what lets a comrak-committed consumer refuse a document it cannot
read instead of producing a node sequence that checks nothing.

Nothing consumes either type yet. blank_line_policy_for is the one place the
HTML-document splice case is decided, and it defers to wave 0's CommonMark
home for every island.

TRACE: ti 490d97 wave 2
TRACE: ADR-0025

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: `Block.spelling` and `Document.format` — the Markdown intake stamps both

**Files:**
- Modify: `crates/transync-syntax/src/parser.rs`, `crates/transync-syntax/src/parser/emit.rs`
- Modify (test literals only): `crates/transync-syntax/src/align.rs`, `crates/transync-syntax/src/render.rs`, `crates/transync-syntax/src/regen.rs`, `crates/transync-core/src/unit.rs`
- Test: new in-crate module `spelling_stamp_tests` in `parser.rs` (red-first)

**Interfaces:**
- Consumes from Task 2: `crate::id::{Spelling, SourceFormat}`.
- Produces:
  ```rust
  pub struct Document {
      pub source_text: String,
      pub format: SourceFormat,   // NEW
      pub blocks: Vec<Block>,
      pub warnings: Vec<String>,
      pub ref_defs: String,
  }
  pub struct Block {
      pub block_id: BlockId,
      pub kind: BlockKind,
      pub spelling: Spelling,     // NEW
      pub source_range: ByteRange,
      pub source_hash: u64,
      pub section_path: Vec<BlockId>,
      pub ast_path: AstPath,
  }
  ```
  and, inside `parser::emit`:
  ```rust
  pub(super) fn emit(&mut self, node: &AstNode<'_>, kind: BlockKind, spelling: Spelling,
                     section_path: Vec<BlockId>, ast_path: Vec<usize>) -> BlockId;
  pub(super) fn emit_here(&mut self, node: &AstNode<'_>, kind: BlockKind,
                          section_path: Vec<BlockId>) -> BlockId;          // Markdown-spelled
  pub(super) fn emit_html_here(&mut self, node: &AstNode<'_>, block_type: u8,
                               section_path: Vec<BlockId>) -> BlockId;     // the island's ONE home
  ```
- **`BlockKind::Html { block_type: u8 }` still has its field at the end of this task** — Task 4 removes it. For one commit the CommonMark type is recorded on both axes, which is what lets each commit compile.

- [ ] **Step 1: Write the failing invariant test.** Append to `crates/transync-syntax/src/parser.rs`, after the existing `mod html_block_tests` block:
```rust
// ti 490d97 wave 2 (spec §3): the Markdown intake stamps BOTH axes, and this
// is the Markdown half of the format invariant — `format == Markdown`, every
// raw-HTML island carrying the CommonMark type comrak reported, and every
// other block spelled Markdown. The HTML half (`format == Html` ⟹ every
// spelling is `Html { None }`) has no producer until wave 3.
#[cfg(test)]
mod spelling_stamp_tests {
    use super::*;
    use crate::id::{SourceFormat, Spelling};

    #[test]
    fn the_markdown_intake_stamps_markdown_format_and_per_block_spellings() {
        let src = "# T\n\npara\n\n<div>island</div>\n\n<pre>\nart\n</pre>\n\n- a\n- b\n";
        let doc = parse(src).expect("parses");

        assert_eq!(
            doc.format,
            SourceFormat::Markdown,
            "the GFM intake is the only producer here, whatever the document holds",
        );

        let got: Vec<(&str, Spelling)> = doc
            .blocks
            .iter()
            .map(|b| (b.kind.wire_str(), b.spelling))
            .collect();
        assert_eq!(
            got,
            vec![
                ("heading-1", Spelling::Markdown),
                ("paragraph", Spelling::Markdown),
                (
                    "html",
                    Spelling::Html {
                        block_type: Some(6)
                    }
                ),
                (
                    "html",
                    Spelling::Html {
                        block_type: Some(1)
                    }
                ),
                ("list-item", Spelling::Markdown),
                ("list-item", Spelling::Markdown),
            ],
            "a Markdown document interleaves both spellings, which is why the \
             axis cannot be a document-level bit",
        );
    }

    #[test]
    fn a_document_with_no_island_is_markdown_spelled_throughout() {
        let doc = parse("# T\n\npara\n\n> quote\n\n```\ncode\n```\n\n---\n").expect("parses");
        assert!(
            doc.blocks
                .iter()
                .all(|b| b.spelling == Spelling::Markdown),
            "spellings: {:?}",
            doc.blocks.iter().map(|b| b.spelling).collect::<Vec<_>>(),
        );
    }
}
```

- [ ] **Step 2: Run it and see it fail.**
```bash
cargo test -p transync-syntax spelling_stamp_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red1.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red1.txt
```
Read the file. Expected: `CARGO_EXIT=101`, with `error[E0609]: no field `format` on type `Document`` and `error[E0609]: no field `spelling` on type `&Block``.

- [ ] **Step 3: Add the two fields, and nothing else yet.** In `crates/transync-syntax/src/parser.rs`:
  - the import line `use crate::id::{BlockId, BlockKind};` becomes:
```rust
use crate::id::{BlockId, BlockKind, SourceFormat, Spelling};
```
  - in `pub struct Document`, immediately after the `source_text` field and its doc comment:
```rust
    /// Which intake produced this document, and therefore which reader may
    /// touch it. `Markdown` is [`parse`]'s answer; a comrak-committed
    /// consumer (`validate::full_reparse`, the Markdown renderer, the heading
    /// projection in `transync-core`'s `unit::context`) can refuse anything
    /// else rather than hand an HTML document to a Markdown parser and
    /// believe the node sequence it gets back.
    ///
    /// Invariant: `Html` ⟹ every block's `spelling` is
    /// `Spelling::Html { block_type: None }`.
    pub format: SourceFormat,
```
  - in `pub struct Block`, immediately after `pub kind: BlockKind,`:
```rust
    /// How the SOURCE spelled this block — orthogonal to [`Block::kind`]
    /// (spec 2026-08-20 §3, decision D2). A Markdown document interleaves
    /// both spellings, which is why this is per block and
    /// [`Document::format`] is not a substitute for it.
    pub spelling: Spelling,
```

- [ ] **Step 4: Walk the compiler to every construction site — in two captures, because a single one cannot exist.**

A missing field is an `error[E0063]` at every initializer, and neither `Block` nor `Document` has a `Default` or a `#[non_exhaustive]`, so nothing can construct one without answering the question. That makes the compiler an exhaustive census. It is exhaustive **per crate, not per workspace**: `transync-core` depends on `transync-syntax`, so while the syntax crate's own targets are failing, core is never compiled and its site cannot appear in the same output. A step that promised one capture holding both crates' sites would be promising something cargo will not produce.

Capture the syntax crate:
```bash
cargo check -p transync-syntax --all-targets > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red2-syntax.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red2-syntax.txt
```
Read the file. Expected: `CARGO_EXIT=101`, and exactly **six** `E0063` **sites**, every one of them in `transync-syntax`. **Six sites is not six printed lines** (measured 2026-08-24): `emit.rs` is reported once for the `lib test` target and again for the `lib` target, so seven `error[E0063]` lines print — and the `lib` unit's summary says `due to 2 previous errors` while showing only one, because cargo cancelled it (`build failed, waiting for other jobs to finish...`) before flushing `parser.rs`'s. Count distinct **file:line** pairs, not diagnostic lines. Same distinction as the census below: lines are not sites.

| Site | Missing field | Answered by |
|---|---|---|
| `parser.rs`, the `Ok(Document { … })` literal in `parse` | `format` | Step 7 |
| `parser/emit.rs`, `emit`'s own `Block` literal | `spelling` | Step 5 |
| `parser.rs`, `skipped_block_id_round_trips_assign_block_ids` | `spelling` | Step 8 |
| `align.rs`, `skipped_row_is_preserved_anchor_and_uncounted` | `spelling` | Step 8 |
| `render.rs`, `render_with_synthetic_skipped` | `spelling` | Step 8 |
| `regen.rs`, `a_hand_built_range_inside_a_char_does_not_panic` | `spelling` | Step 8 |

**A seventh site exists and this capture cannot show it:** `crates/transync-core/src/unit.rs`, `skipped_node_yields_no_translation_unit`. Step 8 captures it in its own `-p transync-core` run, after the syntax crate compiles again, and answers it there. Seven total.

If the syntax count is not six, do **not** treat the difference as noise: a construction site was added or removed since this plan was written. Fix it the same way and record the deviation in Task 8's DCR.

**What is and is not evidence here.** Neither capture on its own proves the census is complete — each proves only that the compiler had nothing further to say about the crate it compiled. The completeness claim is Step 9's, and it rests on two facts together: the workspace builds clean, and this literal census accounts for every one of the seven sites above — see Step 9's 2026-08-23 correction for why "and no eighth" was the wrong shape for that check.
```bash
rg -n --no-heading -e '(^|[^\w])Block \{' -e '(^|[^\w])Document \{' \
  crates/transync-syntax/src crates/transync-core/src \
  | grep -v 'pub struct ' | grep -v -- '->'
```

- [ ] **Step 5: Give `emit` the spelling parameter, and give the island its one home.** In `crates/transync-syntax/src/parser/emit.rs`:
  - the import becomes — **the module-level one at the top of the file, not the identical line inside `emit.rs`'s own test module** (there are two; the test module builds no `Block` literal, so giving it `Spelling` would be an unused import and `-D warnings` would block the commit):
```rust
use crate::id::{BlockId, BlockKind, Spelling};
```
  - `emit`'s signature and push:
```rust
    pub(super) fn emit(
        &mut self,
        node: &AstNode<'_>,
        kind: BlockKind,
        spelling: Spelling,
        section_path: Vec<BlockId>,
        ast_path: Vec<usize>,
    ) -> BlockId {
```
```rust
        self.blocks.push(Block {
            block_id: id.clone(),
            kind,
            spelling,
            source_range: range,
            source_hash: hash,
            section_path,
            ast_path: AstPath(ast_path),
        });
```
  - `emit_here` keeps its three parameters and names its spelling; replace its body and extend its doc comment:
```rust
    /// [`WalkState::emit`] for the common case: this node, at the walker's
    /// current path, **spelled Markdown**. Only the `List` arm needs the
    /// general form, because its items own a path the walker never stands on,
    /// and only the `HtmlBlock` arm needs [`WalkState::emit_html_here`],
    /// because it is the one arm whose spelling is not Markdown.
    ///
    /// The spelling is written here rather than derived from the kind on
    /// purpose (decision D2): deriving it would re-create the conflation the
    /// split removes, and it would collapse the day an intake emits a
    /// semantic kind with a non-Markdown spelling — which is the whole point
    /// of the axis.
    pub(super) fn emit_here(
        &mut self,
        node: &AstNode<'_>,
        kind: BlockKind,
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let ast_path = self.ast_path.clone();
        self.emit(node, kind, Spelling::Markdown, section_path, ast_path)
    }

    /// The ONE place a raw-HTML island's two axes are stamped together.
    ///
    /// `NodeValue::HtmlBlock` is the only Markdown-intake arm that produces a
    /// non-Markdown spelling, and the CommonMark block type it carries has to
    /// land on the *spelling*, not on the kind — the kind's job is semantics,
    /// and an island has none the Markdown intake is willing to guess at
    /// (decision D11: reclassifying `html-0007` to `t-0007` would move the
    /// block id, and with it the alignment row, the DOM anchor and the cache
    /// axis). Keeping both stamps in one function is what stops a future arm
    /// from setting one and forgetting the other.
    pub(super) fn emit_html_here(
        &mut self,
        node: &AstNode<'_>,
        block_type: u8,
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let ast_path = self.ast_path.clone();
        self.emit(
            node,
            BlockKind::Html { block_type },
            Spelling::Html {
                block_type: Some(block_type),
            },
            section_path,
            ast_path,
        )
    }
```

- [ ] **Step 6: Point the two walker arms that do not go through `emit_here` at the new shapes.** In `crates/transync-syntax/src/parser.rs`:
  - the `NodeValue::List` arm's `self.emit(child, kind, sp, item_path);` becomes:
```rust
                    self.emit(child, kind, Spelling::Markdown, sp, item_path);
```
  - the whole `NodeValue::HtmlBlock(h)` arm body becomes:
```rust
            NodeValue::HtmlBlock(h) => {
                // Spec 2026-08-03 §3.1: block-level raw HTML is translatable.
                // Segment extraction happens at unit construction, not here.
                // ti 490d97 wave 2: the CommonMark block type is SPELLING, not
                // kind — `emit_html_here` is the one place both are stamped.
                let sp = self.sections.current_path();
                self.emit_html_here(node, h.block_type, sp);
            }
```

- [ ] **Step 7: Stamp the document's format.** In `parse`, the returned literal becomes:
```rust
    Ok(Document {
        source_text: normalized.into_owned(),
        // The GFM intake, whatever the document happens to contain: a
        // Markdown file that is 90 % raw HTML is still a Markdown file, and
        // `html_dominance_warning` is what says so out loud.
        format: SourceFormat::Markdown,
        blocks,
        warnings,
        ref_defs,
    })
```

- [ ] **Step 8: Answer the five hand-built `Block` literals.** Each one is a synthetic `Skipped` (or `Paragraph`) block in a test, and each takes `spelling: Spelling::Markdown` immediately after its `kind:` line. **Add no imports** (corrected 2026-08-24 by Task 3's implementer, who caught it before committing). This preamble used to say to add `use crate::id::Spelling;` to `align.rs`'s `skipped_row_tests` and to `render.rs`'s and `regen.rs`'s test modules — which **contradicts the bullets below**, since every one of them writes the fully-qualified `spelling: crate::id::Spelling::Markdown,`. Following the preamble leaves three unused imports, and the pre-commit hook's `clippy --all-targets --all-features -- -D warnings` turns `unused_imports` into an error, so the commit is **blocked**. Follow the bullets. Only `parser.rs` writes the bare `Spelling::Markdown`, and it resolves through Step 3's module-level import via the test module's `use super::*`:
  - `crates/transync-syntax/src/parser.rs`, `skipped_block_id_round_trips_assign_block_ids` → `spelling: Spelling::Markdown,`
  - `crates/transync-syntax/src/align.rs`, `skipped_row_is_preserved_anchor_and_uncounted` → `spelling: crate::id::Spelling::Markdown,`
  - `crates/transync-syntax/src/render.rs`, `render_with_synthetic_skipped` → `spelling: crate::id::Spelling::Markdown,`
  - `crates/transync-syntax/src/regen.rs`, `a_hand_built_range_inside_a_char_does_not_panic` → `spelling: crate::id::Spelling::Markdown,`
  Those four restore `transync-syntax` to compiling, which is the precondition for seeing the fifth at all. **Now capture the seventh site** — the one Step 4's syntax-only run structurally could not show:
```bash
cargo check -p transync-core --all-targets > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red2-core.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-red2-core.txt
```
  Read it. Expected: `CARGO_EXIT=101` and exactly **one** `E0063`, `missing field \`spelling\` in initializer of \`parser::Block\``, at `crates/transync-core/src/unit.rs`. More than one means a core construction site this plan did not enumerate — fix it the same way and record the deviation in Task 8's DCR. Then answer it:

  - `crates/transync-core/src/unit.rs`, `skipped_node_yields_no_translation_unit` → `spelling: crate::id::Spelling::Markdown,`

  **Change nothing else in these five tests.** They are characterization pins for `Skipped` blocks; the spelling of a synthetic Markdown-document block is Markdown and the assertions below each literal stay exactly as they are.

- [ ] **Step 9: Run the new tests and the whole suite, then close the census.** After the suite is green, run the literal census from Step 4 and confirm it covers **the seven sites** Step 4 and Step 8 between them enumerated — six in `transync-syntax`, one in `transync-core` — each now carrying a `spelling:` (or `format:`) line. **Corrected 2026-08-23 (drift sweep).** The pattern used to be `[^:\w]`, which excludes the `:` immediately before `Block`, so the two literals spelled `crate::parser::Block {` — `align.rs:370` and `unit.rs:870` — could **never** appear in it. Measured at `6f7b3c4`, the old command returned **eight** lines of which those two were absent and three were not sites at all (`parser.rs:63`/`:109`, the `pub struct` declarations). The instrument was broken in both directions while the E0063 table it was meant to check was correct.

  The corrected command drops `pub struct` declarations and `->` return types, and at `6f7b3c4` returns **eight lines for seven sites**: `parser.rs:245` (`Ok(Document {`), `parser.rs:432`, `emit.rs:52` (`self.blocks.push(Block {`), `regen.rs:557`+`:559` — **one** literal spanning two lines, `let doc = Document { blocks: vec![Block { … }] }` — `render.rs:1656`, `align.rs:370`, and `unit.rs:870`. Six in `transync-syntax`, one in `transync-core`, exactly as Step 4 enumerates. Do not treat the line count as the site count. What this must prove is that **every one of the seven carries a stamp** and that no hit is a construction Step 4 did not enumerate; a hit you cannot place is a site the compiler answered for you in a way you did not read; go look at it. This pairing is what makes "no construction site can be missed" a checked claim rather than an assertion.

```bash
cargo test -p transync-syntax spelling_stamp_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-green1.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-green1.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-workspace.txt
```
Expected: `CARGO_EXIT=0` in both; `test result: ok. 2 passed` for the filtered run.

- [ ] **Step 10: Verify the fixtures did not move, then commit.**
```bash
git status --porcelain -- 'crates/*/tests/fixtures/*' > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-fixtures.txt
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave2/gate/t3-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t3-clippy.txt
```
Expected: `t3-fixtures.txt` empty; `CARGO_EXIT=0`.
```bash
git add crates/transync-syntax/src/parser.rs crates/transync-syntax/src/parser/emit.rs \
  crates/transync-syntax/src/align.rs crates/transync-syntax/src/render.rs \
  crates/transync-syntax/src/regen.rs crates/transync-core/src/unit.rs
git commit -m "feat(parser): every block records how it was spelled, every document records its intake

Block.spelling is per block because it has to be: the HtmlBlock arm interleaves
HTML-spelled blocks with Markdown-spelled ones inside one document, so no
document-level bit can carry the axis. Document.format does the other job — it
names the intake, so a comrak-committed reader can refuse a document it cannot
read rather than trust whatever node sequence comes back.

emit takes the spelling as a parameter rather than deriving it from the kind.
Deriving would re-create exactly the conflation this split removes, and it
collapses the day an intake emits a semantic kind with an HTML spelling —
which is the point. emit_html_here is the one place an island's two axes are
stamped, so a later arm cannot set one and forget the other.

BlockKind::Html still carries block_type for one more commit; the narrowing is
next, and this ordering is what keeps every commit compiling.

TRACE: ti 490d97 wave 2
TRACE: ADR-0025

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `BlockKind::Html` narrows to a unit variant

**Files:**
- Modify (`transync-syntax`): `src/id.rs`, `src/parser.rs`, `src/parser/emit.rs`, `src/regen.rs`, `src/render.rs`, `src/align.rs`, `src/outcome.rs`, `src/walk.rs`
- Modify (`transync-core`): `src/unit/payload.rs`, `src/unit/section.rs`, `src/unit.rs`, `src/validate.rs`, `src/validate/schema.rs`, `src/validate/per_kind.rs`, `src/validate/inline.rs`, `src/validate/fragment_reparse.rs`, `src/validate/full_reparse.rs`, `src/llm.rs`, `src/llm/prompt.rs`, `src/batch.rs`, `src/pipeline.rs`
- Test: no new test file. This task is **characterized** — the existing suite is the test, and the "red" is the compiler's own list of sites that named the departing field.

**Interfaces:**
- Consumes from Task 3: `Block::spelling`.
- Produces:
  ```rust
  pub enum BlockKind {
      // …
      Html,                       // was: Html { block_type: u8 }
      Skipped { label: String },
  }
  ```
  and, in `transync-core`:
  ```rust
  pub(crate) fn assemble(doc: &Document, block: &Block) -> (String, InputMode, BlockConstraints);
  // now matches on `block.spelling` FIRST, not on `block.kind`
  ```
- `HtmlSegmentConstraints::block_type` keeps its `u8` type and gains the `0` sentinel in its documented domain. The field's type is frozen: it is a §0 tier-(a) row without `#[non_exhaustive]`, and the v0.4.0 window closed on release, so `Option<u8>` was never on the table.

- [ ] **Step 1: Narrow the variant.** In `crates/transync-syntax/src/id.rs`, replace the `Html { block_type: u8 }` variant and its doc comment with:
```rust
    /// HTML content with **no semantic equivalent** — a `div`, a custom
    /// element, an unknown or future tag. Translatable via segment extraction
    /// (ADR-0018).
    ///
    /// The variant carried `block_type: u8` until ti `490d97` wave 2. That
    /// field was 100 % spelling metadata — its only consumers were splice
    /// normalization and `HtmlSegmentConstraints` — so it moved to
    /// [`Spelling::Html`], leaving this variant with one meaning instead of
    /// two ("no semantic classification" *and* "spelled as HTML").
    ///
    /// On the Markdown path every raw-HTML island still lands here, and that
    /// is deliberate (decision D11): reclassifying `html-0007` to `t-0007`
    /// would move the block id, and with it the alignment row, the DOM anchor
    /// and the `block_kind` cache axis. The narrowed meaning binds the HTML
    /// intake, where a `<table>` really is a [`BlockKind::Table`].
    Html,
```

- [ ] **Step 2: Capture the site list — syntax first, then core, for the same reason Task 3 Step 4 gave.** `transync-core` cannot compile while `transync-syntax` is failing, so no single run can hold both crates' diagnostics. Eighteen of the twenty-four sites below are in core; a run that showed only six would look like a plan that mis-modelled the tree.

```bash
cargo check -p transync-syntax --all-targets > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-red-syntax.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-red-syntax.txt
```
Expected: `CARGO_EXIT=101`, and the **six syntax-crate sites** from the two classes below — `parser.rs` ×2 and `regen.rs` ×1 (`E0026`), `parser/emit.rs` ×2 and `walk.rs` ×1 (`E0559`). Steps 3–6 and 9 answer them.

Fix those six, then capture core:
```bash
cargo check -p transync-core --all-targets > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-red-core.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-red-core.txt
```
Expected: `CARGO_EXIT=101` and the **eighteen core sites** — `unit/payload.rs` ×1 and `validate.rs` ×1 (`E0026`), and the **sixteen** `E0559` constructions ("fifteen" until 2026-08-23; the per-file list below sums to sixteen for core, which is what makes the "eighteen core sites" and "twenty-four, 5 + 19" totals consistent — the nineteen minus `emit.rs` ×2 and `walk.rs` ×1, which are `transync-syntax`) in `validate/schema.rs`, `validate/per_kind.rs`, `validate/inline.rs`, `unit/section.rs`, `unit/payload.rs`, `llm/prompt.rs`, `batch.rs`, `pipeline.rs` and `validate.rs`.

Across the two captures, two error classes and nothing else — **twenty-four sites, 5 + 19**:
  - `error[E0026]: variant `BlockKind::Html` does not have a field named `block_type`` — **five pattern sites**, the ones that *bound* the field: `parser.rs` ×2 (`top_level_html_block_becomes_block_kind_html_with_type`, `pre_block_records_type_1`), `regen.rs` ×1 (the splice arm), `unit/payload.rs` ×1 (`assemble`'s `if let`), `validate.rs` ×1 (layer 3's `if let`).
  - `error[E0559]: variant `BlockKind::Html` has no field named `block_type`` — **nineteen construction sites**: `parser/emit.rs` ×2 (`emit_html_here`, and `empty_range_guard_tests`), `walk.rs` ×1 (the `label_for` pin), `validate/schema.rs` ×3, `validate/per_kind.rs` ×5, `validate/inline.rs` ×1, `unit/section.rs` ×1, `unit/payload.rs` ×1 (the heading-level pin), `llm/prompt.rs` ×1, `batch.rs` ×1, `pipeline.rs` ×1, `validate.rs` ×2.

  **A note on what does NOT appear.** `BlockKind::Html { .. }` patterns — the ones that bind nothing — are accepted on a unit variant (a braced pattern on a fieldless variant has been legal since RFC 1506), so the compiler will not flag them. **Rewrite them anyway, in this task**: braces on a fieldless variant misdescribe the variant, and leaving them is how the next reader concludes the field is still there. Step 9 names every one of them.

- [ ] **Step 3: Fix the intake.** In `crates/transync-syntax/src/parser/emit.rs`, `emit_html_here`'s `emit` call:
```rust
        self.emit(
            node,
            BlockKind::Html,
            Spelling::Html {
                block_type: Some(block_type),
            },
            section_path,
            ast_path,
        )
```
and in the same file's `empty_range_guard_tests`, `&BlockKind::Html { block_type: 2 }` becomes `&BlockKind::Html`.

- [ ] **Step 4: Move the two intake characterization tests onto the spelling axis.** In `crates/transync-syntax/src/parser.rs`, `mod html_block_tests`. These pinned the CommonMark type; the type did not go away, it moved, so the assertion follows it. Add `use crate::id::Spelling;` to the module and rewrite the two assertions:
  - `top_level_html_block_becomes_block_kind_html_with_type` (rename to `top_level_html_block_becomes_kind_html_spelled_html_with_its_type`):
```rust
        assert!(
            matches!(html.kind, BlockKind::Html),
            "expected kind Html, got {:?}",
            html.kind
        );
        assert_eq!(
            html.spelling,
            Spelling::Html {
                block_type: Some(6)
            },
            "the CommonMark type is SPELLING now, not kind — it moved, it did \
             not go away",
        );
```
  - `pre_block_records_type_1`:
```rust
        assert_eq!(
            doc.blocks[0].spelling,
            Spelling::Html {
                block_type: Some(1)
            },
            "got {:?}",
            doc.blocks[0].spelling
        );
```

- [ ] **Step 5: Re-key regen's two format-aware arms onto spelling.** In `crates/transync-syntax/src/regen.rs`, replace the whole `let to_write: Cow<'_, str> = match &block.kind { … };` expression (keeping the comment block above it) with:
```rust
        // Fallback payloads are the block's raw source bytes; splice them
        // verbatim so fallback output stays byte-identical to the source —
        // the "by construction" guarantee FallbackAll / stage 3 rely on to
        // skip the verifying reparse (DCR-0002, DCR-0004). Only translated
        // payloads need fence re-wrapping.
        //
        // ti 490d97 wave 2 (spec §5/§6): the two transforming arms are keyed
        // on SPELLING, not on kind. That is what makes "(CodeBlock × Html)
        // never reaches the fence synthesizer" a compile shape rather than a
        // convention — DCR-0031's `regenerate_code_block` is Markdown-only,
        // and an HTML document's `<pre>` is a `CodeBlock` that must never be
        // re-fenced. It is also what puts the splice policy where it belongs:
        // the spelling decides it, never the kind.
        let to_write: Cow<'_, str> = match block.spelling {
            Spelling::Markdown => match &block.kind {
                BlockKind::CodeBlock { info, .. } if translated => {
                    Cow::Owned(regenerate_code_block(payload, info.as_deref()))
                }
                _ => Cow::Borrowed(payload),
            },
            Spelling::Html { block_type } if translated => {
                let source_bytes = &doc.source_text[bstart..bend];
                // Validation layer 3 already proved this splice succeeds;
                // if it still fails, splice the source bytes — out.md stays
                // honest, never corrupt.
                match serde_json::from_str::<Vec<String>>(payload)
                    .ok()
                    .and_then(|segs| {
                        transync_html::splice(
                            source_bytes,
                            &segs,
                            Spelling::blank_line_policy_for(block_type),
                        )
                        .ok()
                    }) {
                    Some(spliced) => Cow::Owned(spliced),
                    None => Cow::Borrowed(source_bytes),
                }
            }
            Spelling::Html { .. } => Cow::Borrowed(payload),
        };
```
  and add `Spelling` to the file's `use crate::id::…` import line.

- [ ] **Step 6: Re-key `assemble` onto spelling — first, not as a fallthrough.** In `crates/transync-core/src/unit/payload.rs`, replace the body of `assemble` (keeping its `# Panics` doc block) with:
```rust
pub(crate) fn assemble(doc: &Document, block: &Block) -> (String, InputMode, BlockConstraints) {
    // Spec §6: the branch is on SPELLING, and it is first. Html units carry a
    // JSON segment array, not the block's raw bytes — the markup never reaches
    // the model, and the structural facts the validator needs ride in
    // `constraints.html`. Branching here rather than on the kind is what
    // structurally prevents `(CodeBlock × Html)` from reaching `code_payload`'s
    // indented-block re-fencing: DCR-0031's fence synthesizer is Markdown-only,
    // and an HTML document's `<pre>` is a `BlockKind::CodeBlock`.
    match block.spelling {
        Spelling::Html { block_type } => {
            let raw = block_payload(doc, block);
            let segs = transync_html::extract(&raw)
                .expect("outcome said Unit — extract cannot fail here (same input, same routine)");
            let payload = serde_json::to_string(&segs.texts)
                .expect("Vec<String> JSON serialization is infallible");
            let constraints = BlockConstraints {
                html: Some(crate::llm::HtmlSegmentConstraints {
                    segment_count: segs.texts.len() as u32,
                    segment_labels: segs.labels,
                    source_bytes: raw,
                    // Spec §6: `0` is the sentinel for "a block of an HTML
                    // document, where no CommonMark type applies". The field's
                    // type is frozen (§0 tier (a), no `#[non_exhaustive]`, and
                    // the v0.4.0 window is closed), so `Option<u8>` was never
                    // available; `0` and `1` behave identically under the
                    // `matches!(t, 6 | 7)` rule, and `0` is the one that does
                    // not claim to be a CommonMark type. The splice policy is
                    // still derived from the SPELLING — this field is a
                    // record, not a switch.
                    block_type: block_type.unwrap_or(0),
                }),
                ..BlockConstraints::default()
            };
            (payload, InputMode::HtmlSegments, constraints)
        }
        Spelling::Markdown => {
            let payload = code_payload(doc, block);
            let constraints = constraints_for(&block.kind, &payload);
            (payload, input_mode_for(&block.kind), constraints)
        }
    }
}
```
  and extend the file's import line to `use crate::id::{BlockKind, Spelling};`.

- [ ] **Step 7: Amend the constraint field's documented domain.** In `crates/transync-core/src/llm.rs`, `HtmlSegmentConstraints::block_type`'s doc comment becomes:
```rust
    /// The CommonMark HTML block type of a raw-HTML **island inside a
    /// Markdown document**: `1`–`7`, where types 6/7 get blank-line
    /// normalization at splice and type 1 is exempt (spec §3.3).
    ///
    /// `0` means **a block of an HTML document**, where no CommonMark type
    /// applies. It is a record of what the source was, not the switch the
    /// splice reads: the policy is derived from the block's spelling
    /// (`Spelling::blank_line_policy_for`), and `0` reaches the same answer
    /// through `BlankLinePolicy::from_commonmark_html_block_type` because
    /// anything outside `6 | 7` keeps its blank lines. A doc-comment domain
    /// widening, not a type change — this struct is a §0 tier-(a) row without
    /// `#[non_exhaustive]`, so its field types are frozen between windows.
    pub block_type: u8,
```

- [ ] **Step 8: Fix layer 3's `block_type` reader.** In `crates/transync-core/src/validate.rs`, the guard `if let BlockKind::Html { block_type } = &unit.block_kind && let Some(h) = &unit.constraints.html` becomes `if let Some(h) = &unit.constraints.html` — **the mode-keyed debug assertion and the rest of the re-key are Task 5 Step 5; this step changes only what the compiler forces.** For now:
```rust
    if let Some(h) = &unit.constraints.html {
```
  and, inside, the splice call's third argument:
```rust
            Ok(segs) => match transync_html::splice(
                &h.source_bytes,
                &segs,
                transync_html::BlankLinePolicy::from_commonmark_html_block_type(h.block_type),
            ) {
```

- [ ] **Step 9: Drop the braces everywhere they now lie.** Mechanical, one form: `BlockKind::Html { .. }` → `BlockKind::Html`, and `BlockKind::Html { block_type: N }` → `BlockKind::Html`. The sites, all of them:
  - `crates/transync-syntax/src/id.rs` — `id_code`, `wire_str`
  - `crates/transync-syntax/src/render.rs` — `wrapper_element_for`'s arm; the live-render arm's **head only**: `if let BlockKind::Html { .. } = kind {` becomes `if matches!(kind, BlockKind::Html) {`. **Leave the arm's body exactly as you find it** — wave 1 adds a `transync_html::strip_reserved_sync_attrs` call inside it, and if wave 1 has landed that call is already there and is not yours to touch. Also `render.rs`'s `render_with_status` test filter.
  - `crates/transync-syntax/src/outcome.rs` — both `matches!` sites (Task 5 re-keys these to spelling; drop the braces now so this commit compiles cleanly either way)
  - `crates/transync-syntax/src/align.rs` — the `(BlockKind::Html { .. }, Some(HtmlOutcome::ExtractionFailed(_)))` arm
  - `crates/transync-syntax/src/parser/emit.rs` — `empty_html_range_does_not_trip_the_list_item_guard`'s `find` predicate
  - `crates/transync-syntax/src/walk.rs` — `label_for_pins_every_kind`'s `(BlockKind::Html { block_type: 6 }, "html")` row
  - `crates/transync-core/src/validate/fragment_reparse.rs` — the early return's `matches!` and `expected_label`'s arm
  - `crates/transync-core/src/validate/per_kind.rs` — the dispatch arm and all five `&BlockKind::Html { block_type: 6 }` test arguments
  - `crates/transync-core/src/validate/schema.rs` — the `matches!` in `check_payload_bytes`, the `matches!` in `a_nul_free_payload_passes_on_every_kind`, and all three `BlockKind::Html { block_type: 6 }` test constructions
  - `crates/transync-core/src/validate/inline.rs` — the skip-set `matches!` and the test construction
  - `crates/transync-core/src/validate/full_reparse.rs` — both test `matches!` sites
  - `crates/transync-core/src/unit.rs` — `html_dominance_warning`'s `matches!`
  - `crates/transync-core/src/unit/section.rs` — `unit("html-0006", BlockKind::Html { block_type: 6 })`
  - `crates/transync-core/src/unit/payload.rs` — `(BlockKind::Html { block_type: 6 }, None)` in the heading-level pin
  - `crates/transync-core/src/llm/prompt.rs` — `has_html_unit`'s `matches!` and `html_unit`'s assignment
  - `crates/transync-core/src/batch.rs` — `html()`'s `block_kind`
  - `crates/transync-core/src/pipeline.rs` — `html_unit`'s assignment
  - `crates/transync-core/src/validate.rs` — both test units' `block_kind`

- [ ] **Step 10: Run the whole suite.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-cli.txt
```
Expected: `CARGO_EXIT=0` in both. **This is the wave's first real proof of behaviour preservation** — the whole corpus (SCN-01..15, `cli_smoke.rs` goldens, render and align expectations) regenerates, renders and aligns identically with the field on the other axis. A red here is a genuine behaviour change; do not adjust a fixture, find it.

- [ ] **Step 11: Verify the fixtures did not move, run the wasm gate, then commit.**
```bash
git status --porcelain -- 'crates/*/tests/fixtures/*' > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-fixtures.txt
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave2/gate/t4-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t4-wasm.txt
```
Expected: `t4-fixtures.txt` empty; `CARGO_EXIT=0` in both.
```bash
git add crates/transync-syntax/src crates/transync-core/src
git commit -m "feat(id)!: BlockKind::Html stops carrying the CommonMark type, because that was never a kind

The field was 100 % spelling metadata: its only two consumers were splice
normalization and HtmlSegmentConstraints, and both of them are asking how the
source was written, not what the block is. It moved to Spelling::Html, and the
variant is left with one meaning — HTML content with no semantic equivalent —
instead of two.

regen and unit::payload now branch on spelling FIRST. That ordering is the
point, not a style: it makes '(CodeBlock x Html) never reaches the fence
synthesizer' a compile shape rather than a convention, and DCR-0031's
synthesizer is Markdown-only. constraints.html.block_type keeps its u8 and
gains 0 as the honest sentinel for a block of an HTML document; the type is
frozen, so Option<u8> was never on the table.

Breaking by policy: BlockKind is exhaustive, its variants' field sets are part
of the contract, and this rides the sanctioned v0.5.0 window. Behaviour
preserving in fact: the whole corpus regenerates, renders and aligns
byte-identically, with zero fixture edits.

TRACE: ti 490d97 wave 2
TRACE: ADR-0025

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: The re-keying — every dispatch site moves to the axis it actually meant

**Files:**
- Modify: `crates/transync-syntax/src/outcome.rs`, `crates/transync-syntax/src/align.rs`, `crates/transync-core/src/unit.rs`, `crates/transync-core/src/unit/split.rs`, `crates/transync-core/src/validate.rs`, `crates/transync-core/src/validate/fragment_reparse.rs`, `crates/transync-core/src/validate/schema.rs`, `crates/transync-core/src/validate/inline.rs`, `crates/transync-core/src/validate/per_kind.rs`, `crates/transync-core/src/llm/prompt.rs`
- Test: one **red-first** test for the `(Table × Html)` guard in `unit/split.rs`; everything else in this task is **characterized** by the existing suite

**Interfaces:**
- Signature changes, all of them:
  ```rust
  // validate/schema.rs — the kind is not needed at all any more
  pub(crate) fn check_payload_bytes(input_mode: &InputMode, payload: &str) -> Result<(), String>;
  // validate/fragment_reparse.rs
  pub fn reparse_fragment(kind: &BlockKind, input_mode: &InputMode, result: &UnitResult) -> Result<(), String>;
  // validate/per_kind.rs
  pub fn check(constraints: &BlockConstraints, kind: &BlockKind, input_mode: &InputMode,
               result: &UnitResult) -> Result<(), String>;
  ```
- **Why this is behaviour-preserving, stated once so no step has to re-argue it:** on every document the tree can produce today, `kind == BlockKind::Html` ⇔ `spelling == Spelling::Html { .. }` ⇔ `input_mode == InputMode::HtmlSegments`. The Markdown intake sets kind and spelling in one function (`emit_html_here`), and `assemble` sets the mode from the spelling. So each move below re-keys a predicate onto a *different expression of the same set*, and the existing suite is what proves it. What changes is which set the predicate names **after wave 3**, when an HTML document's `<p>` is `BlockKind::Paragraph` and still ships a segment array.
- **Two things on spec §7's re-keying list that this task deliberately does NOT edit, so their absence is not read as an omission.** (a) *"the `build_batches` outcome-map skip"* — `unit::build_batches`'s `if !is_translatable_block(block, html_outcomes) { continue; }` calls the predicate Step 13 re-keys, so it moves with it and its own line does not change. Editing it would put a second copy of the spelling question one call above the one place that owns it. (b) *`id_code` / `wire_str` / `heading_level` take **no** HTML arm* — semantic kind wins, which is the whole point of the split: an HTML `<h1>` is `h1-0001`, reports `"heading-1"` on the wire, answers `Some(1)`, and `partition_by_section` (whose predicate is `heading_level().is_some()`) therefore works unchanged. Adding a spelling arm to any of those three would move block ids and undo the corpus-stability acceptance criterion. `walk::*` likewise takes no HTML arm: it is the *Markdown* layer-6/renderer pairing and must simply never be called on an HTML document, which is a job for wave 4's format branch, not for an added case here.

- [ ] **Step 1: Write the failing test for the `(Table × Html)` split guard.** Append to `crates/transync-core/src/unit/split.rs`'s `mod tests`:
```rust
    /// Spec §7: `(Table × Html)` is excluded from the GFM row-window splitter
    /// **explicitly**. `inspect_table` would return `None` on an HTML table's
    /// segment-array payload anyway, but "it happens to fail" is not a guard:
    /// this splitter is a Markdown-table machine that slices header, delimiter
    /// and body ROWS out of pipe syntax, and an HTML `<table>` is one leaf
    /// block whose payload is a JSON array. The HTML segment-window splitter
    /// is future work (spec §15 item 4).
    ///
    /// The control case is what makes the assertion mean something: the same
    /// payload under the same budget DOES split when the mode is Markdown.
    #[test]
    fn an_html_segments_unit_is_never_row_window_split_even_when_its_kind_is_table() {
        let bpe = resolve_encoder(None, "gpt-4o-mini");
        let factor = crate::batch::resolve_expansion_factor(None);
        let payload = table(40);

        let markdown_table = TranslationUnit::new(
            BlockId::new("t", 1),
            BlockKind::Table,
            InputMode::FullTableMarkdown,
            payload.clone(),
            0,
        );
        assert!(
            windows_of(&markdown_table, 400, factor, &bpe).is_some(),
            "the control case must split, or this test proves nothing",
        );

        let mut html_table = markdown_table.clone();
        html_table.input_mode = InputMode::HtmlSegments;
        assert!(
            windows_of(&html_table, 400, factor, &bpe).is_none(),
            "an html-segments unit must never reach the GFM row-window splitter",
        );
    }
```

- [ ] **Step 2: Run it and see it fail.**
```bash
cargo test -p transync-core an_html_segments_unit_is_never_row_window_split -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` and an assertion failure naming the second assertion — `an html-segments unit must never reach the GFM row-window splitter`. The control assertion above it passes, which is what proves the failure is the guard's absence and not a mis-sized budget.

- [ ] **Step 3: Add the guard.** In `crates/transync-core/src/unit/split.rs`, at the top of `windows_of`, **above** the existing table check:
```rust
    // Spec §7: the explicit `(Table × Html)` exclusion. Keyed on the input
    // mode because that is the unit-level carrier of spelling (§6): after the
    // HTML intake lands, an HTML `<table>` is `BlockKind::Table` with an
    // `HtmlSegments` payload, and everything below this line assumes pipe
    // syntax. `inspect_table` would refuse it anyway; an accident is not a
    // guard.
    if matches!(unit.input_mode, InputMode::HtmlSegments) {
        return None;
    }
    if !matches!(unit.block_kind, BlockKind::Table) {
        return None;
    }
```
(the second `if` is the existing one, unchanged — it moves down, it does not change).

- [ ] **Step 4: Run it and see it pass.**
```bash
cargo test -p transync-core an_html_segments_unit_is_never_row_window_split -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-green1.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-green1.txt
```
Expected: `CARGO_EXIT=0`, `test result: ok. 1 passed`.

- [ ] **Step 5: Layer 3 — the mode becomes a debug-assert twin.** In `crates/transync-core/src/validate.rs`, inside the `if let Some(h) = &unit.constraints.html {` block Task 4 Step 8 left, as its first statement:
```rust
        // Spec §7 (the re-keying) with §6's `assemble` as its origin: the
        // layer is keyed on the CONSTRAINTS' presence — the
        // splice check's own input — with the mode as its twin. The documented
        // invariant is `spelling Html ⇔ InputMode::HtmlSegments`, established
        // at `unit::payload::assemble` and never re-derived; a unit carrying
        // `constraints.html` under any other mode was built by hand and is a
        // caller bug, not a provider fault, so it belongs in a debug assertion
        // rather than in a rejection.
        debug_assert!(
            matches!(unit.input_mode, InputMode::HtmlSegments),
            "unit {} carries constraints.html under {:?}; the invariant is \
             spelling Html <=> InputMode::HtmlSegments",
            unit.unit_id,
            unit.input_mode,
        );
```
  Also update the block comment above the `if let` so it says what the key is now: replace the sentence beginning "The check's input is `constraints.html.source_bytes`" with the same sentence plus:
```rust
    // ti 490d97 wave 2: keyed on `constraints.html` rather than on
    // `BlockKind::Html`, because the kind stops being the html axis once an
    // HTML document's `<p>` is a Paragraph. The splice policy is derived from
    // the recorded CommonMark type, whose `0` sentinel resolves to `Keep` —
    // the same answer `Spelling::blank_line_policy_for(None)` gives.
```

- [ ] **Step 6: `fragment_reparse` — the early return keys on the mode.** In `crates/transync-core/src/validate/fragment_reparse.rs`:
```rust
pub fn reparse_fragment(
    kind: &BlockKind,
    input_mode: &InputMode,
    result: &UnitResult,
) -> Result<(), String> {
    if result.translated_payload.trim().is_empty() {
        return Err("empty translated payload".into());
    }

    if matches!(input_mode, InputMode::HtmlSegments) {
        // Spec §4.2: html payloads are JSON segment arrays, not Markdown —
        // structure is owned by the splice check (layer 3), not a comrak
        // reparse. ti 490d97 wave 2 re-keyed this from the kind onto the MODE:
        // an HTML document's `<p>` is `BlockKind::Paragraph` and still ships a
        // segment array, so a kind-keyed early return would hand its JSON to
        // comrak and reject every unit in the document.
        return Ok(());
    }
```
  and the file's import gains `InputMode`: `use crate::llm::{InputMode, UnitResult};`.
  In `crates/transync-core/src/validate.rs`, the call becomes:
```rust
    if let Err(reason) = fragment_reparse::reparse_fragment(&unit.block_kind, &unit.input_mode, result)
    {
```

- [ ] **Step 7: `check_payload_bytes` — the kind leaves entirely.** In `crates/transync-core/src/validate/schema.rs`:
```rust
pub(crate) fn check_payload_bytes(input_mode: &InputMode, payload: &str) -> Result<(), String> {
    // An html unit's payload is a JSON array of text segments, and regen
    // splices the *decoded* segments — so a NUL rides in escaped
    // (as the six characters `\u0000`), invisible to a scan of the
    // payload text. Decode first.
    // A payload that will not decode is malformed JSON, which
    // `per_kind::check_html` rejects a moment later with a better
    // diagnostic; fall through to the raw scan so no path is left uncovered
    // (a payload holding a *literal* NUL is not valid JSON either).
    //
    // ti 490d97 wave 2: keyed on the MODE, which is what makes the decode
    // question answerable at all — the payload shape is the mode's statement,
    // not the kind's, and after the HTML intake lands the two disagree.
    if matches!(input_mode, InputMode::HtmlSegments)
        && let Ok(segments) = serde_json::from_str::<Vec<String>>(payload)
    {
```
  The `BlockKind` import may become unused in this file; let the compiler say so and remove it if it does. The caller in `validate.rs`:
```rust
    if let Err(reason) = schema::check_payload_bytes(&unit.input_mode, &result.translated_payload) {
```

- [ ] **Step 8: Move `schema.rs`'s four tests onto the mode.** In `mod` at the foot of `crates/transync-core/src/validate/schema.rs`:
  - `a_markdown_payload_carrying_a_nul_is_rejected` — first argument becomes `&InputMode::TextFragment`.
  - `a_nul_free_payload_passes_on_every_kind` — **rename to `a_nul_free_payload_passes_in_every_input_mode`** and replace its body (declared deviation 4b; no expected value changes). The loop must list **all seven** `InputMode` variants: the old name's "every kind" was already a totality claim, and trading it for a mode-shaped one that covers six of seven would leave the same defect under a new name. Check the enum in `llm.rs` as you write it — if it has grown an eighth variant since this plan, add that too and note it in Task 8's DCR:
```rust
    #[test]
    fn a_nul_free_payload_passes_in_every_input_mode() {
        for mode in [
            InputMode::TextFragment,
            InputMode::FullTableMarkdown,
            InputMode::FullCodeBlock {
                language_info: None,
            },
            InputMode::ListItemContent,
            InputMode::BlockquoteContent,
            InputMode::HtmlSegments,
            // The seventh variant. A window's payload is a whole GFM table
            // and takes the same raw scan `FullTableMarkdown` does, so
            // omitting it would leave the test's name claiming a totality
            // its body does not have -- the exact name-vs-body drift this
            // repository's tests are policed for.
            InputMode::TableRowWindow {
                parent_block_id: crate::id::BlockId("t-0001".to_string()),
                window_index: 0,
                window_count: 2,
            },
        ] {
            // The html arm reads its payload as a segment array; the others
            // read raw markdown. Give each the shape it expects so the pass
            // is a real pass and not a decode failure.
            let payload = if matches!(mode, InputMode::HtmlSegments) {
                "[\"하나\", \"둘\"]"
            } else {
                "표준 문단, U+FFFD도 아니고 NUL도 아님."
            };
            assert!(
                check_payload_bytes(&mode, payload).is_ok(),
                "{mode:?} rejected a clean payload"
            );
        }
    }
```
  - `an_escaped_nul_inside_an_html_segment_is_rejected` and `a_literal_nul_in_an_undecodable_html_payload_is_still_rejected` — first argument becomes `&InputMode::HtmlSegments`. **Both expected error strings stay exactly as they are.**
  - Add `InputMode` to the test module's imports if it is not already there.

- [ ] **Step 9: `check_inline` — the skip set keys on the mode, and its test stops being vacuous.** In `crates/transync-core/src/validate/inline.rs`:
```rust
    // Fences carry no inline nodes; html payloads are JSON (the splice check
    // owns their structure). Skip both parses. ti 490d97 wave 2: the html half
    // is keyed on the MODE — an HTML document's `<p>` is a Paragraph whose
    // payload is a segment array, and running the inline inventory over JSON
    // would compare bracket counts and call them links.
    if matches!(unit.block_kind, BlockKind::CodeBlock { .. })
        || matches!(unit.input_mode, InputMode::HtmlSegments)
    {
        return Ok(());
    }
```
  and the module doc's line `` `BlockKind::CodeBlock` and `BlockKind::Html` units return early `` becomes `` `BlockKind::CodeBlock` units and `InputMode::HtmlSegments` units return early ``. Then replace `html_units_skip_the_inline_layer` (declared deviation 4a — the input gets teeth, the expectation does not move):
```rust
    #[test]
    fn html_units_skip_the_inline_layer() {
        // The payload carries a raw tag on the source side and none on the
        // translated side, so the always-on tag-identity guard WOULD reject it
        // if this layer ran. That is what makes the `is_ok()` below a
        // statement about the skip rather than about two payloads that happen
        // to contain nothing the layer inspects.
        let mut u = unit(BlockKind::Html, "[\"press <kbd>Ctrl</kbd>\"]");
        u.input_mode = InputMode::HtmlSegments;
        assert!(
            check_inline(&policy(None, None), &u, &unit_result("[\"누르세요\"]"), "").is_ok(),
            "an html-segments unit must not reach the inline layer at all",
        );

        // The contrast: the same two payloads under a Markdown mode DO reject,
        // which is the layer this skip is bypassing.
        let md = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd>");
        assert!(
            check_inline(&policy(None, None), &md, &unit_result("누르세요"), "").is_err(),
            "the guard the skip bypasses must be live, or the skip proves nothing",
        );
    }
```

- [ ] **Step 10: `per_kind::check` — the new first arm, and the rationale the spec asks for in the module doc.** In `crates/transync-core/src/validate/per_kind.rs`, extend the module doc with:
```rust
//! **Why an html-segments unit takes one arm for every kind (spec §6).** For
//! an HTML-spelled unit, structure never crosses the wire: `assemble` sends a
//! JSON array of decoded text segments and keeps the markup in
//! `constraints.html.source_bytes`, documented "never sent to the model". The
//! model physically cannot change a column count, a heading level or a list
//! topology it never saw. The enforcement point is layer 3's splice plus its
//! ordered tag ledger, which proves the output block's tag skeleton equals the
//! source's exactly — and tag-sequence identity SUBSUMES column-count
//! identity. So per-kind structural validation for (any kind × html-segments)
//! is satisfied by construction, and `check_table` deliberately gains no html
//! variant: an HTML `<table>` does have a column count, and checking it here
//! would be strictly weaker than the ledger. The honest shape is an early
//! dispatch, not an added case.
```
  and the dispatch:
```rust
pub fn check(
    constraints: &BlockConstraints,
    kind: &BlockKind,
    input_mode: &InputMode,
    result: &UnitResult,
) -> Result<(), String> {
    // The first arm, and it is first for the reason in the module doc: an
    // html-segments unit's shape question is answered by `check_html` whatever
    // its semantic kind is.
    if matches!(input_mode, InputMode::HtmlSegments) {
        return check_html(constraints, result);
    }
    match kind {
        BlockKind::Heading1
        | BlockKind::Heading2
        | BlockKind::Heading3
        | BlockKind::Heading4
        | BlockKind::Heading5
        | BlockKind::Heading6 => check_heading(constraints, result),
        BlockKind::Paragraph => Ok(()),
        BlockKind::Table => check_table(constraints, result),
        BlockKind::CodeBlock { .. } => check_code(constraints, result),
        BlockKind::ListItem { .. } => check_list(constraints, result),
        BlockKind::Blockquote => check_blockquote(constraints, result),
        // Defensive: a kind-Html unit is an html-segments unit by the §6
        // invariant, so the arm above already answered. Kept because the kind
        // still exists and the match is exhaustive by policy.
        BlockKind::Html => check_html(constraints, result),
        // Skipped blocks are never batched (A3), so this arm is defensive
        // and unreachable — kept to keep the match total.
        BlockKind::ThematicBreak | BlockKind::Image | BlockKind::Skipped { .. } => Ok(()),
    }
}
```
  and the file's import line becomes `use crate::llm::{BlockConstraints, InputMode, ListTopologyEntry, UnitResult};`.
  The caller in `validate.rs`:
```rust
    if let Err(reason) = per_kind::check(&unit.constraints, &unit.block_kind, &unit.input_mode, result)
    {
```

- [ ] **Step 11: Give `per_kind`'s nine test call sites their third argument.** The five html tests (`html_payload_that_is_not_json_is_rejected`, `html_segment_count_change_is_rejected`, `empty_html_segment_is_rejected`, `whitespace_only_html_segment_is_rejected`, `well_shaped_html_payload_passes`) take `&InputMode::HtmlSegments` between the kind and the result; the four code-block sites (`infoless_code_fence_may_not_gain_an_info_string`, `infoless_code_fence_round_trips`, and both in `present_code_fence_info_must_survive_verbatim`) take:
```rust
                &InputMode::FullCodeBlock { language_info: None },
```
  **Not the info string the constraint carries** — the mode's `language_info` field is not what `check_code` reads (the constraint is), and passing `None` keeps these tests about the constraint, exactly as they are today. Add `InputMode` to the test module's imports.

- [ ] **Step 12: `has_html_unit` keys on the mode.** In `crates/transync-core/src/llm/prompt.rs`:
```rust
/// Does this population hold a unit whose payload is extracted raw-HTML text
/// segments? The one predicate behind the html-segment instruction clause,
/// read through [`DocumentFacts`] so the prompt and the packer's reserve
/// cannot ask it differently (ti aa92d6).
///
/// ti 490d97 wave 2: keyed on `InputMode::HtmlSegments`, which is exactly the
/// question the clause answers — "is there a unit whose payload is a segment
/// array" — rather than on the block kind, which stops being that question the
/// day an HTML document's `<p>` ships one.
pub(crate) fn has_html_unit(units: &[TranslationUnit]) -> bool {
    units
        .iter()
        .any(|u| matches!(u.input_mode, crate::llm::InputMode::HtmlSegments))
}
```

- [ ] **Step 13: `outcome` iterates spelling, and the kind exclusions become the Markdown arm.** In `crates/transync-syntax/src/outcome.rs`:
  - the import becomes `use crate::id::{BlockId, BlockKind, Spelling};`
  - `html_outcomes`'s skip:
```rust
        if !matches!(block.spelling, Spelling::Html { .. }) {
            continue;
        }
```
  - `is_translatable_block`, with its branches **flipped** — the spelling question is asked first:
```rust
pub fn is_translatable_block(block: &Block, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> bool {
    // Spec §7: for an HTML-spelled block the extraction outcome decides,
    // whatever its semantic kind — an HTML document's `<h1>` is a `Heading1`
    // and still translates through the segment engine, and its `<hr>` is a
    // `ThematicBreak` that extracts zero segments and lands
    // `PreservedZeroSegment`. The kind-level exclusions below are therefore
    // the MARKDOWN arm; asking them first would have excluded a
    // ThematicBreak-kinded HTML block before the outcome map ever saw it,
    // which is the same answer by a route that stops being right.
    if matches!(block.spelling, Spelling::Html { .. }) {
        return matches!(html_outcomes.get(&block.block_id), Some(HtmlOutcome::Unit));
    }
    is_translatable(&block.kind)
}
```
  - `html_outcomes`'s doc line "Run `transync_html::extract` over every `BlockKind::Html` block once" becomes "…over every **HTML-spelled** block once", and `is_translatable`'s doc sentence "`Html` is kind-level translatable" becomes "An HTML-spelled block is kind-level translatable whatever its kind".

- [ ] **Step 14: `align`'s extraction-failed arm keys on spelling.** In `crates/transync-syntax/src/align.rs`:
```rust
            match (block.spelling, html_outcomes.get(&block.block_id)) {
                // Spec §3.2: the rewriter errored — placeholder presentation,
                // honest fallback status, uncounted. Keyed on SPELLING since
                // ti 490d97 wave 2: an HTML document's `<p>` whose extraction
                // failed is a `Paragraph`, and a kind-keyed arm would fall
                // through to `Preserved` and claim the block was carried over
                // intact.
                (crate::id::Spelling::Html { .. }, Some(HtmlOutcome::ExtractionFailed(_))) => {
                    FallbackStatus::FallbackSource
                }
                // OI-0002 / A5: never-batched blocks (thematic-break, image,
                // skipped, zero-segment html) are carried over verbatim —
                // "preserved" is the honest status; claiming "translated"
                // misreports blocks the LLM never saw.
                _ => FallbackStatus::Preserved,
            }
```

- [ ] **Step 15: `html_dominance_warning`'s mass predicate keys on spelling.** In `crates/transync-core/src/unit.rs`:
```rust
        // The sentence this feeds is about how much of a MARKDOWN document is
        // written as raw HTML, which is a spelling question, not a kind one.
        if matches!(block.spelling, crate::id::Spelling::Html { .. }) {
            html_bytes += len;
        }
```
  The warning's text and its two thresholds are unchanged; its suppression for declared-HTML runs and its re-texted tail are **wave 5's**, landing with the entry point that makes them true. (Corrected 2026-08-20: this line said *wave 6's*. Spec §12 assigns the re-text to wave 5 and DCR-0036's hand-forward repeats it, and the reasoning behind the line — "landing with the entry point that makes them true" — points the same way: the entry point is `TranslateOptions`' input-format option, which is **wave 5's**, not the CLI flag, which is wave 6's. The clause was right; only the wave number was wrong.)

- [ ] **Step 16: Run the whole suite, both halves.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-cli.txt
```
Expected: `CARGO_EXIT=0` in both.

- [ ] **Step 17: Verify the fixtures did not move, run the wasm gate, then commit.**
```bash
git status --porcelain -- 'crates/*/tests/fixtures/*' > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-fixtures.txt
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave2/gate/t5-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t5-wasm.txt
```
Expected: `t5-fixtures.txt` empty; `CARGO_EXIT=0` in both.
```bash
git add crates/transync-syntax/src crates/transync-core/src
git commit -m "refactor: every html dispatch site keys on the axis it actually meant

Nine predicates asked 'is this block kind Html?' when they meant one of three
different things: is it spelled as HTML (outcome, align, the dominance mass),
is its payload a segment array (fragment reparse, the NUL scan, the inline
skip, per-kind shape, has_html_unit), or does it carry splice constraints
(layer 3). The three sets are identical today — one function stamps kind and
spelling together, and assemble derives the mode from the spelling — so this
changes no behaviour and the corpus proves it with zero fixture edits. It stops
being identical the moment an HTML document's <p> is a Paragraph that ships a
segment array, and then every one of these would have been wrong in a
different direction.

per_kind gets one arm for every kind under html-segments, with the reason in
the module doc: structure never crosses the wire for such a unit, and layer 3's
ordered tag ledger subsumes the column-count check the table arm would run.
check_table therefore gains no html variant — an early dispatch is honest,
'strictly weaker' is not.

The (Table x Html) exclusion in the row-window splitter is explicit and tested.
inspect_table would refuse an HTML payload anyway; an accident is not a guard.

TRACE: ti 490d97 wave 2

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: `BlockKind::Title`, and the two catch-alls that would have swallowed it

**Files:**
- Modify: `crates/transync-syntax/src/id.rs`, `crates/transync-syntax/src/align.rs`, `crates/transync-syntax/src/render.rs`, `crates/transync-syntax/src/walk.rs`, `crates/transync-syntax/src/outcome.rs`, `crates/transync-core/src/validate/per_kind.rs`, `crates/transync-core/src/validate/fragment_reparse.rs`, `crates/transync-core/src/unit/context.rs`, `crates/transync-core/src/unit/payload.rs`
- Test: **two red-first tests that assert the answer, not the compile** (`align.rs`, `unit/context.rs`), plus two characterization extensions (`walk.rs`, `unit/payload.rs`) and one translatability pin (`outcome.rs`)

**Interfaces:**
- Produces:
  ```rust
  pub enum BlockKind {
      // …
      Title,      // NEW: id_code "title", wire_str "title", heading_level() None
  }
  fn sync_role_for(kind: &BlockKind) -> SyncRole;              // gains an explicit Title => NonSync
  pub(crate) fn document_title(doc: &Document) -> Option<String>; // prefers a Title block
  ```
- **`Title` has no producer in this wave.** The Markdown intake cannot emit one; wave 3's HTML intake is what mints it from `<title>` in head mode. That is exactly why the two trap tests below have to construct their inputs by hand, and exactly why Step 1 is safe: a wrong answer from a catch-all is unreachable for the length of this task.

- [ ] **Step 1: Add the variant with every arm the compiler forces, and NOT the two it does not.** This is the sequencing decision of the whole wave, so it is stated before the edit: adding a variant to a non-`#[non_exhaustive]` enum breaks **every exhaustive match at once**, so there is no compiling intermediate and the variant plus its five forced arms land together. The two arms the compiler *cannot* demand — `align::sync_role_for` behind `_ => SyncRole::Anchor`, `unit::context::document_title` behind a `matches!` on `Heading1` alone — are deliberately left wrong until Step 4, so the tests in Step 2 can fail **for the right reason**. Nothing reachable is wrong in between: no producer emits a `Title`.

  In `crates/transync-syntax/src/id.rs`, add the variant immediately after `Image` (before `Html`):
```rust
    /// An HTML document's `<title>` — real translatable content that is
    /// **not page content** (decision D5).
    ///
    /// It gets a real block, a real translation unit and a real alignment row,
    /// and it gets **no DOM anchor**: the browser chrome renders it, not the
    /// pane, so there is nothing in the pane to anchor. Its row therefore
    /// carries `sync_role: "non-sync"` — the shape a thematic break has had
    /// since schema 1.0 — which is why `align::sync_role_for` has an
    /// **explicit** arm for this variant rather than letting it reach the
    /// `_ => SyncRole::Anchor` default. This amends architectural invariant 1:
    /// the DOM-anchor leg of the chain is conditional on being page content,
    /// not on the kind.
    ///
    /// `heading_level()` is `None`: a page title is not a heading level, it
    /// opens no section scope, and `partition_by_section` puts it in the
    /// preamble section — correct, since no glossary section selector can
    /// mean a page title. `unit::context::document_title` prefers it over the
    /// first `Heading1` by an explicit arm, for the same reason.
    ///
    /// TRACE: ADR-0025
    Title,
```
  Then the five arms the compiler demands:
  - `id.rs`, `id_code` — after the `Image` arm:
```rust
            // An HTML document's page title. No collision with h1..h6, p, t,
            // c, li, q, hr, img, html, x.
            BlockKind::Title => "title",
```
  - `id.rs`, `wire_str` — after the `Image` arm:
```rust
            BlockKind::Title => "title",
```
  - `crates/transync-core/src/validate/per_kind.rs`, `check` — a `Title` unit is an html-segments unit and took the first arm; group it with the other defensive kinds by extending that arm:
```rust
        // Skipped blocks are never batched (A3), and a `Title` unit is an
        // html-segments unit that the first arm already answered, so both are
        // defensive and unreachable — kept to keep the match total.
        BlockKind::ThematicBreak
        | BlockKind::Image
        | BlockKind::Title
        | BlockKind::Skipped { .. } => Ok(()),
```
  - `crates/transync-core/src/validate/fragment_reparse.rs`, `expected_label` — after the `Html` arm:
```rust
        // Defensive and unreachable for the same reason as `Html`: a Title
        // unit's payload is a JSON segment array, so `reparse_fragment`
        // returns early on the mode and never asks for a label. `title` is a
        // CommonMark type-6 tag, so `html-block` is what one WOULD reparse as.
        BlockKind::Title => "html-block",
```
  - `crates/transync-syntax/src/render.rs`, `wrapper_element_for` — after the `Image` arm:
```rust
        // D5: a `<title>` is never in a pane — it has no anchor and no
        // presentation here, and the Markdown renderer never sees an HTML
        // document at all. A transparent `<div>` keeps the match total without
        // inventing a presentation for a block that has none.
        BlockKind::Title => "div",
```

- [ ] **Step 2: Write the two trap tests. They must fail on the ANSWER.** Both variants now exist, so neither test is blocked on a compile.
  First, in `crates/transync-syntax/src/align.rs`, appended after `mod skipped_row_tests`:
```rust
// ti 490d97 wave 2 (spec §3 / decision D5): `sync_role_for` ends in
// `_ => SyncRole::Anchor`, so a new kind is silently an ANCHORING row unless
// someone writes the arm — the precise opposite of the decision for `<title>`.
// The compiler cannot say so; this test can. It asserts the ROLE, not that the
// code compiles, which is the only assertion a catch-all cannot satisfy.
#[cfg(test)]
mod sync_role_tests {
    use super::*;

    #[test]
    fn a_title_row_is_non_sync_while_the_catch_all_still_anchors_everything_else() {
        assert_eq!(
            sync_role_for(&BlockKind::Title),
            SyncRole::NonSync,
            "D5: the <title> is rendered by browser chrome, not by the pane — \
             there is nothing there to anchor",
        );
        // The contrast that makes the assertion above non-vacuous: the
        // catch-all this arm escapes is what every other kind still takes.
        assert_eq!(sync_role_for(&BlockKind::Heading1), SyncRole::Anchor);
        assert_eq!(sync_role_for(&BlockKind::Paragraph), SyncRole::Anchor);
        // The kind that has answered `non-sync` since schema 1.0, so the row
        // shape a title ships is not a new one.
        assert_eq!(sync_role_for(&BlockKind::ThematicBreak), SyncRole::NonSync);
        assert_eq!(sync_role_for(&BlockKind::Blockquote), SyncRole::Container);
    }
}
```
  Second, in `crates/transync-core/src/unit/context.rs`, appended at the end of the file:
```rust
// ti 490d97 wave 2 (spec §3 / §6): `document_title` matched `BlockKind::Heading1`
// and nothing else — not "the first block with a heading level" — so a `Title`
// block does NOT enter the document title for free, and extending it is
// explicit work. These pin the selection rule; the TEXT projection for a real
// `<title>…</title>` slice is wave 5's, and the third test says so out loud
// rather than leaving the gap to be discovered.
#[cfg(test)]
mod document_title_tests {
    use super::*;
    use crate::id::{SourceFormat, Spelling};
    use crate::parser::AstPath;
    use crate::parser::ranges::ByteRange;

    /// A hand-built HTML document: a `Title` block over the first line, an H1
    /// over the last, gap in between. Hand-built because no intake emits a
    /// `Title` until wave 3, and the H1 is there so the test can tell the new
    /// rule from the old one.
    fn hand_built_titled_document(title_slice: &str) -> Document {
        let source_text = format!("{title_slice}\n\n# Heading one\n");
        let h1_start = title_slice.len() + 2;
        let end = source_text.len();
        Document {
            source_text,
            format: SourceFormat::Html,
            blocks: vec![
                Block {
                    block_id: BlockId::new("title", 1),
                    kind: BlockKind::Title,
                    spelling: Spelling::Html { block_type: None },
                    source_range: ByteRange {
                        start: 0,
                        end: title_slice.len(),
                    },
                    source_hash: 0,
                    section_path: Vec::new(),
                    ast_path: AstPath(vec![0]),
                },
                Block {
                    block_id: BlockId::new("h1", 2),
                    kind: BlockKind::Heading1,
                    spelling: Spelling::Html { block_type: None },
                    source_range: ByteRange {
                        start: h1_start,
                        end,
                    },
                    source_hash: 0,
                    section_path: Vec::new(),
                    ast_path: AstPath(vec![1]),
                },
            ],
            ..Document::default()
        }
    }

    #[test]
    fn a_title_block_wins_the_document_title_over_the_first_h1() {
        let doc = hand_built_titled_document("Doc name");
        assert_eq!(
            document_title(&doc).as_deref(),
            Some("Doc name"),
            "D5 / spec §6: an HTML run's document title is its <title>, not \
             the first heading the page happens to contain",
        );
    }

    #[test]
    fn without_a_title_block_the_first_h1_still_wins() {
        // The Markdown rule, unchanged — this is the regression guard on the
        // half that must not move.
        let doc = crate::parser::parse("intro\n\n# Heading one\n\n# Heading two\n")
            .expect("parses");
        assert_eq!(document_title(&doc).as_deref(), Some("Heading one"));
    }

    /// The projection gap, pinned rather than left to be found later: a REAL
    /// `<title>…</title>` slice reads as empty prose today, because
    /// `heading_plain_text` runs comrak over the slice and comrak sees one
    /// `HtmlBlock` node with no inline children to walk. Wave 5 replaces the
    /// projection for Html-spelled blocks with `transync_html::extract(…)`
    /// (spec §6); wave 2 owns only the SELECTION rule above. When wave 5
    /// lands, this expectation becomes `Some("Doc name")` and this test's
    /// name and comment go with it.
    #[test]
    fn a_real_title_element_projects_to_empty_prose_until_wave_5() {
        let doc = hand_built_titled_document("<title>Doc name</title>");
        assert_eq!(document_title(&doc).as_deref(), Some(""));
    }
}
```

- [ ] **Step 3: Run both and see them fail on the value.**
```bash
cargo test -p transync-syntax sync_role_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-red-role.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-red-role.txt
cargo test -p transync-core document_title_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-red-title.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-red-title.txt
```
Read both files. Expected, and the exact shape matters — **these are assertion failures, not compile errors**, which is the whole point of adding the variant first:
  - `t6-red-role.txt`: `CARGO_EXIT=101`, `assertion `left == right` failed: D5: the <title> is rendered by browser chrome…` with `left: Anchor` and `right: NonSync`. The naive version *compiles and passes silently* if the test only checks the code builds; it fails here because the test names the role.
  - `t6-red-title.txt`: `CARGO_EXIT=101`, two failures — `a_title_block_wins_the_document_title_over_the_first_h1` with `left: Some("Heading one")` / `right: Some("Doc name")`, and `a_real_title_element_projects_to_empty_prose_until_wave_5` with `left: Some("Heading one")` / `right: Some("")`. `without_a_title_block_the_first_h1_still_wins` passes already; it is the characterization guard, and its passing before and after is the evidence that the Markdown answer did not move.

- [ ] **Step 4: Write the two arms the compiler could not demand.**
  In `crates/transync-syntax/src/align.rs`, `sync_role_for`:
```rust
fn sync_role_for(kind: &crate::id::BlockKind) -> SyncRole {
    use crate::id::BlockKind::*;
    match kind {
        ThematicBreak => SyncRole::NonSync,
        // D5 (ti 490d97 wave 2): a `<title>` is translated and aligned but is
        // NOT page content — the browser chrome renders it, so the pane has
        // nothing to anchor. EXPLICIT, and pinned by `sync_role_tests`,
        // because the `_` arm below answers `Anchor` and would have made this
        // row anchoring — the precise opposite of the decision — without one
        // word of warning from the compiler.
        Title => SyncRole::NonSync,
        Blockquote => SyncRole::Container,
        // R0006-0042: items render as top-level `<li>` sync anchors (no
        // list-level row exists), so `anchor` is the honest role —
        // `child-only` contradicted the DOM.
        ListItem { .. } => SyncRole::Anchor,
        // A5: a Skipped placeholder is rendered as a `<pre>` in both panes
        // and must anchor scroll there (owner decision) — never non-sync.
        Skipped { .. } => SyncRole::Anchor,
        _ => SyncRole::Anchor,
    }
}
```
  In `crates/transync-core/src/unit/context.rs`, `document_title` — replace the function body and the first paragraph of its doc comment:
```rust
/// The document's title: the plain text of its **`Title` block** when it has
/// one, else of its **first level-1 heading**, else `None`.
///
/// The `Title` preference is explicit, and it has to be (ti 490d97 wave 2):
/// [`BlockKind::Title`]'s `heading_level()` is `None` — a page title is not a
/// heading level — so "the first block with a heading level" would never find
/// it, and this `.find` matched `Heading1` and nothing else before the arm
/// below existed. A Markdown document has no `Title` block, so its answer is
/// unchanged, which is what `without_a_title_block_the_first_h1_still_wins`
/// pins.
pub(crate) fn document_title(doc: &Document) -> Option<String> {
    doc.blocks
        .iter()
        .find(|b| matches!(b.kind, BlockKind::Title))
        .or_else(|| {
            doc.blocks
                .iter()
                .find(|b| matches!(b.kind, BlockKind::Heading1))
        })
        .map(|b| heading_plain_text(doc, b))
}
```
  (keep the rest of the existing doc comment — the paragraphs about the per-document cost, about the text being untrusted prose, and about it possibly being empty, all of which stay true).

- [ ] **Step 5: Run both and see them pass.**
```bash
cargo test -p transync-syntax sync_role_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-green-role.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-green-role.txt
cargo test -p transync-core document_title_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-green-title.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-green-title.txt
```
Expected: `CARGO_EXIT=0` in both; `test result: ok. 1 passed` and `test result: ok. 3 passed`.

- [ ] **Step 6: Extend the two projection pins the compiler also could not demand.**
  `crates/transync-syntax/src/walk.rs`, `label_for_pins_every_kind` — the array's declared length goes `15` → `16` and the row lands after `(BlockKind::Image, "paragraph")`:
```rust
        let cases: [(BlockKind, &str); 16] = [
```
```rust
            // `label_for` delegates to `wire_str` here, and `walk` is the
            // MARKDOWN layer-6/renderer pairing — a Title cannot occur in a
            // Markdown document, so this row pins the delegation rather than a
            // reachable case.
            (BlockKind::Title, "title"),
```
  `crates/transync-core/src/unit/payload.rs`, `only_heading_kinds_carry_a_heading_level_and_it_is_their_depth` — one row after `(BlockKind::Image, None)`:
```rust
            // D5: a page title is not a heading level. `heading_level` reaches
            // it through `_ => None`, so this row is what says the answer is
            // intended rather than incidental — and it is what
            // `partition_by_section` reads to leave a title in the preamble
            // section.
            (BlockKind::Title, None),
```

- [ ] **Step 7: Pin the kind-level translatability.** Append to `crates/transync-syntax/src/outcome.rs`:
```rust
// D5: a `<title>` is real translatable content — the single highest-value
// string on many pages — so it gets a real unit and a real row. The kind-level
// predicate says so by NOT excluding it; this pins that the omission is the
// decision and not an oversight, beside the two kinds that ARE excluded.
#[cfg(test)]
mod title_translatability_tests {
    use super::*;

    #[test]
    fn a_title_is_kind_level_translatable_and_the_excluded_kinds_still_are_not() {
        assert!(is_translatable(&BlockKind::Title));
        assert!(!is_translatable(&BlockKind::ThematicBreak));
        assert!(!is_translatable(&BlockKind::Image));
    }
}
```

- [ ] **Step 8: Run the whole suite, both halves.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-cli.txt
```
Expected: `CARGO_EXIT=0` in both.

- [ ] **Step 9: Verify the fixtures did not move, run the wasm gate, then commit.**
```bash
git status --porcelain -- 'crates/*/tests/fixtures/*' > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-fixtures.txt
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave2/gate/t6-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t6-wasm.txt
```
Expected: `t6-fixtures.txt` empty; `CARGO_EXIT=0` in both.
```bash
git add crates/transync-syntax/src crates/transync-core/src
git commit -m "feat(id)!: BlockKind gains Title, and the two arms the compiler could not ask for

Five matches over BlockKind are exhaustive, so adding a variant breaks them all
at once and there is no compiling intermediate: the variant and its five forced
arms land in one commit. Two consumers sit behind catch-alls and would have
taken the variant silently, in both cases to the wrong answer.

align::sync_role_for ends in `_ => SyncRole::Anchor`, so a title row would have
been an ANCHORING row — the exact opposite of D5, which says a <title> is
rendered by browser chrome and has nothing in the pane to anchor. The arm is
explicit and the test asserts the ROLE, because a test that only checked the
code compiles would have passed against the bug.

unit::context::document_title matched Heading1 and nothing else — not 'any
heading' — so a Title does not enter the document title for free. It is an
explicit preference now, and the Markdown answer is pinned unchanged beside it.

Title has no producer yet; wave 3's HTML intake mints it from <title> in head
mode. Breaking by policy (BlockKind is exhaustive), riding the sanctioned
v0.5.0 window, and behaviour-preserving in fact: zero fixture edits.

TRACE: ti 490d97 wave 2
TRACE: ADR-0025

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: The version — `0.5.0-dev`, and the five requirements that do not inherit it

**Files:**
- Modify: `Cargo.toml` (root), `Cargo.lock`
- Test: no new test. `crates/transync/tests/workspace_publication.rs::every_internal_requirement_equals_the_workspace_version` is the gate, and it already exists — this task's red is produced by editing one key and leaving the five it shadows.

**Interfaces:**
- Produces: `[workspace.package] version = "0.5.0-dev"`, inherited by all eight members through `version.workspace = true`, plus five `[workspace.dependencies]` requirements equal to it.
- **There are FIVE internal requirements, not four.** The release checklist's step 17 and the root manifest's own comment both say five, and both were updated by wave 0 when `transync-html` joined the table (`transync-html`, `transync-syntax`, `transync-core`, `transync`, `transync-openai`). Any plan or record that still says four predates wave 0. `transync-anthropic` publishes but has **no entry**, deliberately — nothing in the workspace depends on it, and `workspace_publication.rs` fails an unused entry as `declared but unused`.

- [ ] **Step 1: Bump ONLY the workspace version, and watch it self-report.** In the root `Cargo.toml`, under `[workspace.package]`:
```toml
version      = "0.5.0-dev"
```
```bash
cargo test -p transync --test workspace_publication -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t7-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t7-red.txt
```
Read the file. Expected: `CARGO_EXIT=101`, and the failure is **cargo's own resolution error, before the test binary is even built** — `error: failed to select a version for the requirement `transync-syntax = "^0.4.0"`` (and the same for the other four), because the path member now offers `0.5.0-dev` and `^0.4.0` does not accept it. **That is the point of doing it in this order, and the reason the test exists anyway:** a *minor* bump self-reports at the very next build, but a **patch** bump does not — `^0.2.0` still accepts `0.2.1` — so a patch-level miss stays silent and ships a published manifest understating what its sibling needs. `every_internal_requirement_equals_the_workspace_version` is the gate that does not depend on which kind of bump this is.

- [ ] **Step 2: Move the five requirements in the same edit.** In the root `Cargo.toml`, under `[workspace.dependencies]`:
```toml
transync-html   = { version = "0.5.0-dev", path = "crates/transync-html" }
transync-syntax = { version = "0.5.0-dev", path = "crates/transync-syntax" }
transync-core   = { version = "0.5.0-dev", path = "crates/transync-core" }
transync        = { version = "0.5.0-dev", path = "crates/transync" }
transync-openai = { version = "0.5.0-dev", path = "crates/transync-openai" }
```
  **Do not touch** the `transync-anthropic` paragraph above them, the "Bumping `[workspace.package] version` means bumping these five in the same edit" sentence (still true, still five), or the publication-set comment (still seven members).

- [ ] **Step 3: Regenerate and stage the lockfile.** `Cargo.lock` **is** committed (owner decision 2026-08-06, `6cf4164`).
```bash
cargo build --workspace > /Volumes/Temp/claude/ti490d97-wave2/gate/t7-build.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t7-build.txt
git status --porcelain -- Cargo.lock > /Volumes/Temp/claude/ti490d97-wave2/gate/t7-lock.txt
```
Expected: `CARGO_EXIT=0`; `t7-lock.txt` shows ` M Cargo.lock`. The diff rewrites the eight workspace entries' `version` fields and nothing else.

- [ ] **Step 4: Prove both welds green.** Two tests read the version, from opposite ends: the publication gate reads the manifests, and `public_surface.rs::generator_version_matches_the_facade_crate` reads `env!("CARGO_PKG_VERSION")` against the alignment map's `generator.version`, so a bump that compiles but does not reach the wire is red there.
```bash
cargo test -p transync --test workspace_publication --test public_surface -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t7-welds.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t7-welds.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t7-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t7-workspace.txt
```
Expected: `CARGO_EXIT=0` in both.

- [ ] **Step 5: Commit.**
```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: the workspace opens the v0.5.0 window, and the five requirements move with it

version under [workspace.package] does not reach [workspace.dependencies]:
those five internal requirements carry their own literal, because cargo
publish demands a version beside every path. A minor bump like this one fails
the very next build if they are forgotten — a patch bump does not, and would
ship a manifest understating what its sibling needs, which is why
workspace_publication.rs checks them rather than the build.

0.5.0-dev, not 0.5.0: the window is open, the release is not cut.

TRACE: ti 490d97 wave 2

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: The window's paperwork — ADR-0025, contracts, CHANGELOG, and the records

**Files:**
- Create: `docs/decisions/0025-html-to-html-document-translation.md`, `docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md`
- Modify: `docs/index.md`, `docs/architecture/contracts.md`, `docs/architecture/rough-schema.md`, `docs/implementation/module-map.md`, `CLAUDE.md`, `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`

**Interfaces:**
- Consumes: everything Tasks 2–7 landed. The `TRACE: ADR-0025` lines written in Tasks 2, 3, 4 and 6 point at nothing until Step 1 exists.
- **Record numbers are reserved per wave, not taken from the disk.** Wave 0 holds DCR-0032, **wave 1 holds DCR-0033 by name in its own plan**, and wave 2 takes **DCR-0034 even if 0033 is not on disk yet** — wave 1 runs parallel to 2–5, so "the next free number" is a race. ADR-0025 is the spec's own reservation (§10).
- Produces: no code. The paperwork is part of this wave and not a follow-up, because §0's path table structurally cannot see a field- or variant-level change ("No other type's fields are covered"), which makes the §0 prose paragraph the only place these four can be recorded at all.

- [ ] **Step 1: Write ADR-0025.** `docs/decisions/0025-html-to-html-document-translation.md`, opening with OKF frontmatter in the house shape:
```markdown
---
type: ADR
title: HTML documents are a second intake into one pipeline, and semantic kind is orthogonal to source spelling
description: One architectural commitment covering the twelve ratified decisions of the HTML→HTML design — the kind ⊥ spelling split, the transync-html mechanics crate, structural pass-through with content stop, the anchor-less <title>, panes as a sync surface not a fidelity preview, flag-only format routing, <li> as the block, and the deferral of Markdown-island reclassification to a schema-2.x window.
tags: [decision, architecture, ADR-0025]
generated:
  by: claude-code/claude-opus-5
  at: <execution date>T00:00:00Z    # the day this ADR is written, from `date -u`; NOT the plan's writing date
status: stable
---

# ADR-0025: HTML documents are a second intake into one pipeline
```
  The body carries these sections, each stating a fact this design settled:
  - **Status / Date / Source** — accepted **on the execution date** (DCR-0033's rule: the plan's 2026-08-20 was its writing date, and `git log --date=short` is the check), ticket `490d97`, spec `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`. ADR-0018 is the precedent for one ADR carrying several decisions.
  - **Context** — the CLI refuses HTML-shaped input with a pointer at this ticket; the shipped IR conflates semantic kind with source spelling; everything downstream of the IR is already format-agnostic or one dispatch away from it.
  - **Decision** — the twelve, in the spec's own order and wording (D1 staged waves; D2 kind ⊥ spelling; D3 crates on the layer axis with the format seam a module boundary; D4 structural pass-through / content stop / unknown stop; D5 `<title>` translated but anchor-less, attribute text untranslated; D6 panes are a sync surface; D7 entity drift accepted; D8 explicit `--input-format`, the sniff stays a refusal; D9 `<li>` is the block; D10 the crate is `transync-html`; D11 island reclassification deferred; D12 one v0.5.0 window).
  - **Rejected alternatives**, each with its ground: top-level `<body>` children as blocks (a wrapper-heavy page becomes one giant block); a format-axis crate split (intake classification names `BlockKind`, so a format crate either depends on the IR or duplicates it); panes as a fidelity preview (breaks contracts §4a's direct-child invariant, corrupts `offsetTop` geometry via source-controlled `style`, re-opens OI-0035's surface); sniff-as-router (silent format selection is the class of guess ADR-0017's refusals exist to prevent); an HTML→Markdown round-trip.
  - **Accepted limitations**, with the visible consequence spelled out: attribute text (`alt`, `title`, `placeholder`, `<meta description>`, `og:*`) stays source-language, so **a shared link previews in the source language**; entity-spelling drift in genuinely translated segments; panes carry no page layout; `<ul>`/`<ol>` markup is gap and the anchor is per item; a custom element mid-sentence splits the sentence; the `<title>` has no anchor.
  - **D11's deferral, with its pin.** The Markdown intake keeps emitting `BlockKind::Html` for islands; the narrowed meaning binds the HTML intake only. Reclassifying moves block ids — id prefixes come from `id_code()` — and with them alignment rows, DOM anchors and the `block_kind` cache axis, which violates the acceptance criterion that the Markdown path stay byte-identical. Revisit in a **schema-2.x-shaped window**, not before.
  - **The amendment to architectural invariant 1**, quoted in full as the replacement wording (spec §10), with the note that CLAUDE.md carries it as of this wave.
  - **The name-continuity note, in both directions** — D10 gave up the grep continuity `transync-htmlseg` would have kept, and this is where it is bought back: **the module formerly called `htmlseg` inside `transync-syntax` is the crate now called `transync-html`**. ADR-0018, ADR-0003, `stub-manifest.md`, `status.md`, `open-issues-archive.md` and `implementation-slice-checklists.md` still say `htmlseg`; those records are dated and are not rewritten. The forward half — structural intake lives in `transync-syntax`, not in `transync-html` — is in the crate's own doc comment.
  - **Consequences** — one pipeline with two intakes; four breaking-by-policy IR changes riding one v0.5.0 window; the layer-6 twin becomes a hard precondition for any HTML run.

- [ ] **Step 2: Write DCR-0034.** `docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md`, same frontmatter shape (`type: DCR`, `tags: [change, project-control, DCR-0034]`). Body sections:
  - **Date / Source** — **the execution date**, ticket `490d97`, wave 2 of the eight in the spec's §12. Date it from `git log --date=short` over this wave's own commits, not from this plan's filename: waves 0 and 1 both had their landings mis-dated to the plan's writing date, and DCR-0033 line 29 records the rule.
  - **Breaking by policy, behaviour-preserving in fact.** The four changes, each with what a consumer sees: `Block.spelling` and `Document.format` added (tier-(c) engine types — `parser` is a hidden module and the facade re-exports nothing from it, so only a direct `transync-syntax` dependant is affected, under the weaker promise §0 tier (c) grants); `BlockKind::Html { block_type: u8 }` narrowed to a unit variant (a consumer's `BlockKind::Html { .. }` pattern still compiles — braces on a fieldless variant are legal — but `Html { block_type }` does not, and neither does constructing it); `BlockKind::Title` added (every exhaustive match over `BlockKind` in a consumer's tree stops compiling). `BlockKind` is a §0 tier-(a) row, so the last two are facade-visible; §1 records them.
  - **What did NOT move**, which is the wave's actual claim: no `id_code`, no `wire_str`, no alignment `schema_version`, no `VALIDATION_SCHEMA_VERSION`, no prompt or instruction bytes, no cache axis. The corpus regenerates, renders and aligns byte-identically with **zero fixture edits**.
  - **The two catch-alls, named.** `align::sync_role_for`'s `_ => SyncRole::Anchor` and `unit::context::document_title`'s `matches!` on `Heading1` alone. Both got explicit arms and both got tests that assert the *answer*; a plan that trusted "the compiler walks us to every match" would have shipped an anchoring `<title>` row and a document title that ignored the page title.
  - **The re-keying, with its warrant.** Nine predicates moved from kind-`Html` to `Spelling::Html` / `InputMode::HtmlSegments` / `constraints.html`; the three sets are identical today and stop being identical the day an HTML document's `<p>` is a `Paragraph` that ships a segment array. `is_translatable_block`'s branch order flipped so the kind exclusions became the Markdown arm. The `(Table × Html)` guard is explicit and tested.
  - **The sentinel.** `constraints.html.block_type` keeps its `u8` and gains `0` for "a block of an HTML document". The type is frozen — §0 tier (a), no `#[non_exhaustive]`, window closed on the v0.4.0 release — so `Option<u8>` was never available; `0` behaves identically to `1` under `matches!(t, 6 | 7)` and is the value that does not claim to be a CommonMark type.
  - **Evidence.** Workspace suite green with zero fixture edits; CLI stub suite green; the two-package wasm gate exit 0 with its string unchanged; `workspace_publication.rs` green at `0.5.0-dev` across all five internal requirements.
  - **Handed forward.** The `Document.format` refusals (wave 4's `finalize` branch, wave 6's render/pane guard); the `document_title` and heading text projections through `extract` (wave 5); `SourceFormat`'s §0 row and the schema-1.3.0 wire fields (wave 6); the HTML segment-window splitter (spec §15 item 4, unscheduled).

- [ ] **Step 3: `docs/index.md` — both links, in this same change.** In the ADR list, after the ADR 0024 line:
```markdown
- [ADR 0025 — HTML documents are a second intake into one pipeline, and semantic kind is orthogonal to source spelling](decisions/0025-html-to-html-document-translation.md)
```
  In the DCR list, after the DCR-0031 line (leaving room for wave 0's DCR-0032 and wave 1's DCR-0033, whichever are present):
```markdown
- [DCR-0034 — Semantic kind and source spelling become separate axes in the block IR](project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md) — `Spelling` and `SourceFormat` join `BlockKind`, `BlockKind::Html` narrows to a unit variant, `BlockKind::Title` is added anchor-less, and every html dispatch site is re-keyed; the v0.5.0 window opens here.
```

- [ ] **Step 4: `contracts.md` §0 — the prose paragraph, because nothing else in §0 can hold this.** Insert it immediately **after** the `BlockKind::CodeBlock` / `fenced` paragraph (the last of the recorded surface decisions) and before the `Two tiers are listed:` line:
```markdown
**Recorded surface decision — the block IR splits semantic kind from source spelling, and `BlockKind` gains a variant while `Html` loses a field (ti `490d97` wave 2, the four sanctioned v0.5.0 surface-window changes, batched).** Four changes ride one window. (1) `parser::Block` gains `pub spelling: Spelling` and (2) `parser::Document` gains `pub format: SourceFormat`, two new `transync-syntax::id` types saying respectively how the source *wrote* a block and which intake produced the document. (3) `BlockKind::Html { block_type: u8 }` narrows to the unit variant `BlockKind::Html`, meaning "HTML content with no semantic equivalent"; the `block_type` moves to `Spelling::Html`, where it was always spelling metadata — its only consumers were splice normalization and `HtmlSegmentConstraints`. (4) `BlockKind::Title` is added for an HTML document's `<title>`: `id_code() = "title"`, `wire_str() = "title"`, `heading_level() = None`, and — explicitly, against the `_ => SyncRole::Anchor` default that would otherwise have made it an anchoring row — `sync_role_for(Title) = NonSync`, because the browser chrome renders a title and the pane has nothing to anchor.

The two field additions are **tier (c)**: `parser` is a hidden module, the facade re-exports nothing from it, and only a consumer depending on `transync-syntax` directly is affected — under the weaker promise tier (c) already grants. The two `BlockKind` changes are **tier (a)** and breaking for anyone who named them: `BlockKind::Html { block_type }` no longer compiles as a pattern and cannot be constructed (a braceless `BlockKind::Html` is the new spelling; a bare `BlockKind::Html { .. }` pattern still compiles, since a braced pattern on a fieldless variant is legal, and it is now misleading), and **every exhaustive `match` over `BlockKind` in a consumer's tree stops compiling** when `Title` lands. This is the same shape of break as `translated_document` and `fenced`: §0's path table cannot see it — `transync::BlockKind` is still exactly that path, so `public_surface.rs` does not move — and the four field-level facts the gate does weld do not include this one ("No other type's fields are covered"), so this prose paragraph is again the only place in §0 that can carry it. Nothing on the wire moves: `id_code`/`wire_str` are unchanged for every pre-existing kind, no `schema_version` bumps, and no cache axis is added. `Spelling` and `SourceFormat` carry **no §0 row in this window** — the facade does not export them yet; `SourceFormat` reaches it with the alignment map's `input_format` field, and the row lands in the same commit as the export, exactly as §0 requires of every row.
```

- [ ] **Step 5: `contracts.md` §1 — extend the `BlockKind` stability bullet.** Replace the existing bullet beginning `- `BlockKind` is an **exhaustive** enum` with:
```markdown
- `BlockKind` is an **exhaustive** enum, and so are its variants' field sets: adding a variant, removing one, or adding/removing a field on one is breaking-by-policy and rides a version bump. The 0.4.0 window carried one — `CodeBlock` gains `fenced: bool` (ti `457e51`; §0 records the decision, DCR-0031 the behavior it enables). A consumer's `BlockKind::CodeBlock { info }` pattern becomes `BlockKind::CodeBlock { info, .. }`, and the `wire_str()` vocabulary the alignment map and `CacheKey` are keyed on is unchanged — `code-block` still names both spellings, so no `schema_version` moves. **The v0.5.0 window carries two more** (ti `490d97` wave 2, DCR-0034): `Html { block_type: u8 }` narrows to the unit variant `Html`, and `Title` is added. The narrowing breaks any pattern or construction that named the field; the addition breaks **every exhaustive match** a consumer wrote, which is the cost the exhaustive-by-policy rule exists to make visible rather than silent. `wire_str()` is unchanged for every pre-existing kind and `Title` takes the fresh label `"title"`, so again no `schema_version` moves in this window. The two axis types the split introduces — `Spelling` (per block: how the source wrote it) and `SourceFormat` (per document: which intake produced it) — are **exhaustive by policy on `BlockKind`'s own ground**, since a consumer matching a spelling has to be told when a third one appears; they are **tier (c)** engine types today, reached only through a direct `transync-syntax` dependency, and each gets its §0 row in the window that exports it.
```

- [ ] **Step 6: `rough-schema.md` — the sketch stops describing the old IR.** Four edits:
  1. §1's prefix set: `{h1, h2, h3, h4, h5, h6, p, t, c, li, q, hr, img, html, x}` becomes `{h1, h2, h3, h4, h5, h6, p, t, c, li, q, hr, img, title, html, x}`, and the sentence after it gains: `` `title` is an HTML document's `<title>` (ADR-0025) — a real row with `sync_role: non-sync` and no DOM anchor. ``
  2. §2's enum sketch: `Html { block_type: u8 }` becomes `Html,` with the comment `` // wire: "html" — HTML content with NO semantic equivalent; the CommonMark block type moved to Spelling::Html (ADR-0025) ``, and a `Title,` line lands after `Image` with `` // wire: "title" — an HTML document's <title>: translated, aligned, never anchored (D5) ``.
  3. A new §2a, **`Spelling` and `SourceFormat`**, carrying the two enum sketches from spec §3 verbatim plus the one-line invariant (`format == Html` ⟹ every spelling is `Html { None }`).
  4. §3's IR sketch: `Document` gains `format: SourceFormat,   // which intake produced it` after `source_text`, and `Block` gains `spelling: Spelling,       // how the SOURCE wrote it — orthogonal to kind` after `kind`.

- [ ] **Step 7: `module-map.md` — one line.** `    ├── id.rs                           # BlockId assignment + source_hash` becomes:
```markdown
    ├── id.rs                           # BlockId assignment + source_hash + the two vocabularies (BlockKind, Spelling/SourceFormat)
```

- [ ] **Step 8: `CLAUDE.md` — architectural invariant 1 takes the ADR-0025 wording.** Replace invariant 1 under **Architectural Invariants** with the spec §10 text, verbatim:
```markdown
1. **Block ID is the only sync currency.** Never use heading text, slugs, line numbers, or scroll percentage. The same `block_id` flows: Rust IR → LLM request → LLM response → regenerated document → rendered DOM anchors → JS sync engine. The final leg is conditional on visibility, not on kind: every block rides the chain unbroken through the regenerated document, and every block displayed in the page gets a DOM anchor — a block that is not page content (today exactly one: an HTML document's `<title>`, translated but rendered by the browser chrome rather than the pane) carries a `sync_role: non-sync` alignment row instead of an anchor, the same shape a thematic break has had since schema 1.0. No block is ever anchored by anything other than its `block_id`, and the engine takes its anchor set from validated alignment rows, never from whatever the DOM happens to carry.
```
  Also, in the same file, sweep the remaining text that promises Markdown as the *output* format. **Invariant 6 does not say "regenerated Markdown" — do not grep for it there.** The invariant that did is invariant 1, which this step already replaces wholesale. What is left is two sites, and neither is matched by a literal search for "regenerated Markdown":
  - the project description's **"the regenerated translated Markdown"** → "the regenerated translated document";
  - invariant 4's **"in the translated Markdown"** → "in the translated document".

  The field has been `translated_document` since v0.4.0 and an HTML run returns HTML. Verify with `grep -n 'translated Markdown\|regenerated Markdown' CLAUDE.md` returning nothing; that grep, not a per-invariant one, is this step's check.

- [ ] **Step 9: `CHANGELOG.md` — the window opens, and the entries say what broke.** Under `## [Unreleased]`, **replace the window paragraph** (after wave 0 it reads "v0.4.0 closed the sanctioned breaking window opened after v0.3.0, and everything below it is additive … wave 2 … is the one asking for it, against a future v0.5.0"; if wave 0's Task 7 left different words, the rule is the same — the paragraph must end up saying the window is open) with:
```markdown
**The sanctioned v0.5.0 breaking window is OPEN.** It was opened by wave 2 of
the HTML→HTML feature (ti `490d97`, ADR-0025, DCR-0034), which splits semantic
kind from source spelling in the block IR. Four breaking-by-policy changes ride
it, batched; nothing user-visible changed, and the corpus regenerates, renders
and aligns byte-identically with zero fixture edits. Additive changes land here
as they come; further breaking changes may ride this window until it is closed
by the v0.5.0 release.
```
  Then, as the **first** subsection under that paragraph (above whatever `### Changed` / `### Added` blocks wave 0 and wave 1 left):
```markdown
### Changed (BREAKING)

- **`BlockKind::Html { block_type: u8 }` narrows to the unit variant `BlockKind::Html`** (ti `490d97` wave 2, ADR-0025 / DCR-0034). The variant now means one thing — "HTML content with no semantic equivalent" — instead of two. The CommonMark block type moved to the new `Spelling::Html`, where it was always spelling metadata. A pattern or construction naming `block_type` no longer compiles; a bare `BlockKind::Html { .. }` pattern still does, and is now misleading.
- **`BlockKind` gains a `Title` variant** for an HTML document's `<title>`: `id_code` `"title"`, `wire_str` `"title"`, `heading_level()` `None`, and an **explicit** `sync_role` of `non-sync` — a title is translated and aligned but has no DOM anchor, because the browser chrome renders it and the pane has nothing to anchor. Every exhaustive `match` over `BlockKind` in a consumer's tree needs the new arm.
- **`parser::Block` gains `spelling: Spelling`; `parser::Document` gains `format: SourceFormat`.** Engine-tier (§0 tier (c)) types, reached only by a consumer depending on `transync-syntax` directly. Per-block spelling is required rather than stylistic: one Markdown document interleaves HTML-spelled islands with Markdown-spelled blocks, so no document-level bit can carry the axis.

### Changed

- Every html dispatch site is re-keyed onto the axis it meant — `Spelling::Html` for IR questions, `InputMode::HtmlSegments` for unit questions, `constraints.html` for the layer-3 splice. Behaviour-preserving today, by three-way set identity; the point is what it will mean once an HTML document's `<p>` is a `Paragraph` that ships a segment array.
- The GFM row-window table splitter now excludes html-segments units **explicitly**, with a test. `inspect_table` would have refused one anyway; an accident is not a guard.
- Workspace version `0.5.0-dev`.
```

- [ ] **Step 10: `docs/project/status.md` — two edits, both in sections that already exist.**
  1. The `- Workspace version:` line at the top. Keep the whole v0.4.0 sentence (it is still the last release and its record stands) and **prepend**:
```markdown
- Workspace version: **`0.5.0-dev`** — the **sanctioned v0.5.0 breaking window is OPEN**, opened **on the execution date** by ti `490d97` wave 2 (ADR-0025 / DCR-0034: the block IR splits semantic kind from source spelling). Four breaking-by-policy changes ride it, batched: `Block.spelling`, `Document.format`, `BlockKind::Html` narrowed to a unit variant, and `BlockKind::Title` added. Nothing user-visible changed — the corpus regenerates, renders and aligns byte-identically with zero fixture edits. Last release:
```
  (so the existing `**v0.4.0** — **released 2026-08-20**…` text becomes the tail of the same bullet).
  2. `## Immediate Next Actions` — a new bullet immediately **after** wave 0's (the one beginning `**Action (HTML→HTML feature, ti `490d97`):** ~~wave 0`):
```markdown
- **Action (HTML→HTML feature, ti `490d97`):** ~~wave 2 — the IR split, the breaking window~~ **LANDED <execution date>** (ADR-0025 / DCR-0034): `Spelling` and `SourceFormat` joined `BlockKind` in `transync-syntax::id`, `Block` gained `spelling` and `Document` gained `format`, `BlockKind::Html` narrowed to a unit variant, and `BlockKind::Title` landed with the **two explicit arms the compiler could not ask for** — `align::sync_role_for` ends in `_ => SyncRole::Anchor` and would have made a title an anchoring row, and `unit::context::document_title` matched `Heading1` alone, so a title did not enter the document title for free. Both got tests that assert the answer, not the compile. Nine dispatch sites were re-keyed onto `Spelling::Html` / `InputMode::HtmlSegments` / `constraints.html`, and the `(Table × Html)` split exclusion is explicit and tested. The workspace is at **`0.5.0-dev`** and the **v0.5.0 window is open**. Acceptance held: workspace + CLI suites green with **zero fixture edits**, the two-package wasm gate exit 0 with its string unchanged. **Waves 3–7 are unblocked** (dependency order `0 → 2 → {3, 4} → 5 → 6 → 7`); wave 4's layer-6 twin is a **hard precondition for any HTML translation run**.
```

  3. **`## Immediate Next Actions`, wave 1's bullet — retire its closing sentence (added 2026-08-23, after this plan's last correction pass).** That bullet now ends `**Waves 2–7 remain unstarted.**`, written by `9cbb39a` when wave 1 landed. Edit 2 above inserts a "wave 2 LANDED" bullet directly after it, so executing Step 10 without this third edit leaves `status.md` asserting both at once. Change it to:
```markdown
**Waves 2–7 were unstarted at that point.**
```
  Same treatment wave 1's Task 4 applied to wave 0's identical clause, and for the same reason: the sentence was true about the moment it describes, so it is dated rather than deleted. Check `phase-state.yaml`'s wave-1 note for the same clause while you are there — if it carries one, it needs the same past tense.

- [ ] **Step 11: `docs/project/phase-state.yaml` — three edits, and one non-edit.** The file has **three** `status:` keys, not two: two section-level phase keys (`design.status: handoff_complete`, `implementation.status: mvp_complete`) and one pointer entry (`status: docs/project/status.md`), plus `baseline_status:` and `sync_status:` that a bare `grep -c 'status:'` also matches. **None of them moves**; wave 2 changes no phase. Do not use a `status:` count as a verification for this step — count the three edits below instead.
  1. `project.last_updated:` → `<execution date>-ti490d97-wave2` — the day the wave lands, not this plan's writing date.
  2. `design.closed_change_records:` — append, at the file's four-space list indent, after wave 0's and (if present) wave 1's entries:
```yaml
    - DCR-0034-ir-semantic-kind-and-spelling-split
```
  3. `project.notes:` is a `|` literal block of unformatted prose — no backticks, no markdown, `->` for arrows. Insert after wave 0's paragraph, at the block's existing four-space indent:
```yaml
    TI 490d97 WAVE 2 LANDED <execution date> (ADR-0025 / DCR-0034): the block IR
    split semantic kind from source spelling. Spelling and SourceFormat joined
    BlockKind in transync-syntax::id, Block gained spelling and Document
    gained format, BlockKind::Html narrowed to a unit variant (its CommonMark
    block type moved to Spelling::Html, where it always belonged), and
    BlockKind::Title landed with id_code "title", wire_str "title",
    heading_level None and an EXPLICIT non-sync role. Two catch-alls would
    have swallowed the new variant silently: align::sync_role_for ends in
    _ => Anchor, which would have made a title an anchoring row - the exact
    opposite of D5 - and unit::context::document_title matched Heading1 and
    nothing else, so a title did not enter the document title for free. Both
    got explicit arms and tests that assert the ANSWER rather than the
    compile. Nine dispatch sites were re-keyed onto Spelling::Html /
    InputMode::HtmlSegments / constraints.html; the three sets are identical
    today and stop being identical once an HTML document's <p> is a Paragraph
    shipping a segment array. The (Table x Html) row-window exclusion is
    explicit and tested. Workspace at 0.5.0-dev; the SANCTIONED v0.5.0
    BREAKING WINDOW IS OPEN, carrying four breaking-by-policy changes batched.
    Acceptance: workspace and CLI suites green with ZERO fixture edits,
    two-package wasm gate exit 0, string unchanged. Waves 3-7 unblocked; wave
    4's layer-6 twin is a hard precondition for any HTML translation run.
```

- [ ] **Step 12: The acceptance check that this whole wave rests on — zero fixture edits, over the recorded baseline.**
```bash
BASE=$(cat /Volumes/Temp/claude/ti490d97-wave2/gate/baseline-commit.txt)
git diff --stat "$BASE"..HEAD -- 'crates/*/tests/fixtures/*' 'crates/*/tests/scenarios/*' 'web/tests' > /Volumes/Temp/claude/ti490d97-wave2/gate/t8-zero-fixture.txt 2>&1
echo "GIT_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-zero-fixture.txt
git status --porcelain -- 'crates/*/tests/fixtures/*' 'crates/*/tests/scenarios/*' 'web/tests' >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-zero-fixture.txt
```
Read the file. Expected: **`GIT_EXIT=0` and no diff lines at all** — the file holds nothing but that one line. A single changed fixture, scenario or browser test means this wave changed behaviour somewhere; it is a plan failure, not something to accept. Stop and report what moved.

- [ ] **Step 13: Run the docs welds and the whole suite.**
```bash
cargo test -p transync --test docs_index_drift --test docs_ownership_drift --test docs_gate_claims_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t8-docs.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-docs.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t8-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave2/gate/t8-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-cli.txt
```
Expected: `CARGO_EXIT=0` in all three. A red `docs_index_drift` names an unlinked document — link it; never exclude it.

- [ ] **Step 14: Run the aggregate hard gate once, then commit.**
```bash
./scripts/smoke.sh > /Volumes/Temp/claude/ti490d97-wave2/gate/t8-smoke.txt 2>&1
echo "SMOKE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave2/gate/t8-smoke.txt
```
Read the file separately. Expected: `SMOKE_EXIT=0`. This is the run that exercises the rustdoc gate over the new doc comments — every intra-doc link written in Tasks 2–6 (`[`Spelling::Html`]`, `[`BlankLinePolicy::Keep`]`, `[`BlockKind::Title`]`, …) has to resolve under `RUSTDOCFLAGS="-D warnings"`, and the workspace test suite never checks that.
```bash
git add docs/decisions/0025-html-to-html-document-translation.md \
  docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md \
  docs/index.md docs/architecture/contracts.md docs/architecture/rough-schema.md \
  docs/implementation/module-map.md CLAUDE.md CHANGELOG.md \
  docs/project/status.md docs/project/phase-state.yaml
git commit -m "docs: the v0.5.0 window opens, and ADR-0025 says what it is for

Four breaking-by-policy changes ride one window, batched and recorded the way
CodeBlock.fenced was: a contracts §0 prose paragraph, a §1 stability bullet,
and a CHANGELOG BREAKING entry. The prose paragraph is not a stylistic choice —
§0's path table names items, not fields or variants, and the gate welds exactly
four field-level facts and says so ('No other type's fields are covered'), so
prose is the only place in §0 that can carry a field addition or a variant
narrowing at all.

ADR-0025 carries the twelve ratified decisions as one architectural
commitment, their rejected alternatives, the accepted limitations with their
visible consequences, D11's deferral with its schema-2.x pin, and the
name-continuity note D10 owes in both directions: the module that used to be
called htmlseg is the crate now called transync-html, and structural intake is
not in it.

Invariant 1 takes its amended wording here too: the DOM-anchor leg is
conditional on being page content, not on the kind, and today exactly one
block is not page content.

TRACE: ti 490d97 wave 2
TRACE: ADR-0025
TRACE: DCR-0034

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Wave acceptance — check all six before declaring wave 2 done

- [ ] **Corpus byte-identical, zero fixture edits.** `git diff --stat <baseline-commit>..HEAD -- 'crates/*/tests/fixtures/*' 'crates/*/tests/scenarios/*' 'web/tests'` prints nothing, and `cargo test --workspace -- --test-threads=4` plus `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4` are both exit 0, captured bare-to-file. (`<baseline-commit>` is in `/Volumes/Temp/claude/ti490d97-wave2/gate/baseline-commit.txt`.)
- [ ] **The two-package wasm gate exit 0, string unchanged:** `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`.
- [ ] **The `Title => NonSync` test and the `document_title` tests exist and are non-vacuous** — each was observed failing on a *value* (`Anchor` vs `NonSync`; `Some("Heading one")` vs `Some("Doc name")`) before its arm was written, with the capture files to show for it.
- [ ] **contracts §0 prose + §1 bullet + CHANGELOG BREAKING entries** all present, and all four changes named in each.
- [ ] **Version `0.5.0-dev` in `[workspace.package]` AND in all five `[workspace.dependencies]` internal requirements**, with `workspace_publication.rs` and `public_surface.rs` both green.
- [ ] **ADR-0025 and DCR-0034 exist and are linked from `docs/index.md`**, and `docs_index_drift` is green.

---

## Notes for the implementer

- **How the enum change is sequenced, because this is the question a reviewer will ask first.** Adding a variant to a non-`#[non_exhaustive]` enum breaks **every** exhaustive match simultaneously, so there is no compiling intermediate: Task 6 Step 1 lands `Title` together with all five arms the compiler demands (`id_code`, `wire_str`, `per_kind::check`, `fragment_reparse::expected_label`, `render::wrapper_element_for`) in one commit. The narrowing of `Html` is a *different* shape of change — it breaks only the sites that named the departing field — so it gets its own earlier commit (Task 4), and `Block.spelling` gets one before that (Task 3), which is what gives the narrowing somewhere to move the field to. Every commit in the wave builds and every commit's suite is green.
- **The one commit that is deliberately green-but-incomplete.** Between Task 6 Step 1 and Step 4, `sync_role_for(Title)` answers `Anchor` and `document_title` ignores a `Title` block. That interval is inside one task and one commit, and nothing reachable is wrong: no producer emits a `Title` in this wave. The interval exists so the two tests can fail on a *value*. If they were written before the variant, their red would be `E0599`/`E0433` — a compile error proves the variant is missing, not that the catch-all is wrong, and a catch-all's whole hazard is that it makes the wrong answer compile.
- **Characterized vs red-first, stated plainly rather than dressed up.** Red-first with a real, observed failure: Task 2 (the two vocabularies), Task 3 (the stamp invariant), Task 5 Step 1 (the `(Table × Html)` guard), Task 6 (both traps), Task 7 (the version, whose red is cargo's own resolution error). **Characterized** — the existing suite is the test, and the evidence is the baseline capture versus the verification capture: Task 1, Task 4, and the other sixteen steps of Task 5. Do not invent a red-first ceremony for the mechanical re-keying; a test written to fail against a predicate you are about to replace with a provably equal one proves nothing, and the honest gate for that work is the zero-fixture-edit diff.
- **The `matches!` de-fanging hazard, in one sentence:** re-keying a predicate from `block_kind` to `input_mode` silently disarms every test that set only the kind. Two were found (`validate::inline`'s html skip, `validate::schema`'s per-kind loop) and both are handled in Task 5; if the suite goes suspiciously green after a re-key, look for a third before believing it.
- **`align.rs`'s extraction-failed arm is not in the spec's §5 re-keying list, and it must move anyway** (Task 5 Step 14). It keys on the same axis for the same reason, and after wave 3 an HTML `<p>` whose extraction failed would fall through to `Preserved` — an alignment row claiming a block was carried over intact when it was not. The spec's list is the sites its verification pass enumerated, not a closed set; this one was found by grepping every `BlockKind::Html` occurrence in the tree.
- **`walk::label_for` and `payload::input_mode_for` are on the prompt's "compiler will walk you to" list but are not** — both end in a catch-all (`other => other.wire_str()`, `_ => InputMode::TextFragment`), so `Title` reaches the right answer through them silently. That is fine for both (`"title"`; and a `Title` unit never takes the Markdown arm of `assemble`), but it is why Task 6 Step 6 extends `label_for`'s pin by hand: the array's declared length is what turns a missing row into `E0308`.
- **Do not add a `_ =>` arm to any match over `BlockKind` to make a compile error go away.** The two this wave works around are pre-existing and are exactly the reason it needs care; a third would be the next silent swallow, and the next wave would pay for it.
- **`transync-syntax` keeps both prohibitions** — no `[features]` table, no `transync-core` dependency including dev-dependencies. `Spelling::blank_line_policy_for` names a `transync_html` type, which is an edge that already exists and already compiles for `wasm32`; nothing new joins the graph.
