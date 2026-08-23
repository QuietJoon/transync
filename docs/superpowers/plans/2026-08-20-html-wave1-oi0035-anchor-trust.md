# HTML→HTML Wave 1 — OI-0035, Both Layers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close OI-0035 — a `data-sync-id` written into source content can pre-claim a real block's anchor — at **both** layers, before the HTML pane producer that would make it worse exists. The renderer stops emitting foreign sync attributes into a pane; the engine stops taking its anchor set from whatever the DOM happens to carry.

**Architecture:** Two independent halves, one issue. **Render half:** the Markdown path's `BlockKind::Html` success arm — the only live route by which source-controlled markup reaches a Markdown pane, since panes render with `render.unsafe_ = false` — runs the block's own bytes through `transync_html::strip_reserved_sync_attrs` before the wrapper writes ours. Strip-then-inject is what makes "ours are the only sync attributes in this DOM" a *construction* rather than a scan. The strip is **pane-only**: `out.md` (and the future `out.html`) keep the author's bytes, because their `data-sync-id` is their content. **Engine half:** `mountSync` builds an id set from the validated alignment rows whose `sync_role !== "non-sync"` — derived from the same function `synchronizableRowCount` answers with, so the count that gates the mount and the set that gates the anchors cannot disagree — and `collectAnchors`, the single choke point both panes pass through at mount *and* on every reflow recompute, skips any element whose `data-sync-id` is not in it. An unlisted anchor is inert forever, including one inserted after mount. **One thing the gate must not swallow:** R0002-0047's refusal of a map that describes no synchronizable block is conditioned on what the panes *carry*, so that count keeps being read straight off the DOM, ungated, and the refusal runs **before** the gated collection — otherwise the two conditions annihilate each other and a title-only map over anchored panes would mount vacuously instead of being refused (spec §8 records that interaction as verified). **The honest residual, recorded rather than glossed:** the engine alone cannot defeat an in-pane impostor carrying a *listed* id that precedes the genuine anchor in document order — `collectAnchors` is first-occurrence-wins and no DOM-visible discriminator separates the two. That is precisely why route (c) is both layers, and why the render half is not optional.

**Tech Stack:** Rust 2024 workspace (rustc ≥ 1.88), `transync-html` (landed in wave 0), comrak 0.27, vanilla ESM (no build step) for `web/js/sync.js`, headless Playwright + Chromium driven by `pnpm` for the browser suite, `wasm32-unknown-unknown` check gate, tracked pre-commit hook (fmt + clippy + wasm gate + rustdoc gate).

**Spec:** docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md

**Depends on:** wave 0 (`docs/superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md`), which lands `transync_html::strip_reserved_sync_attrs` with its own tests and **no call site**. Wave 1 is its first consumer. **The dependency is split, and the split is load-bearing:** Tasks 1–3 need only wave 0 **Task 6** — the function and its six tests in their final home — but Task 4 additionally requires wave 0 **Task 7's records commit** (DCR-0032, the CHANGELOG entry, `status.md`, `phase-state.yaml`), because every one of Task 4's edit anchors quotes the post-Task-7 text: a DCR roster ending at 0032, a status line reading "through DCR-0032", the `**Action (HTML→HTML feature, ti \`490d97\`):**` bullet, the `WAVE 0 LANDED` paragraph, and an `[Unreleased]` block that no longer holds the "Nothing yet" placeholder. Task 4 Step 0 is the guard that proves this before anything is written; dispatched between wave 0's Task 6 and its Task 7, Task 4 stops there — it does not improvise. Wave 1 is independent of waves 2–7 and runs parallel to 2–5 (spec §12).

## Global Constraints

- **The two `sync.js` copies must stay byte-identical, and they move in ONE commit.** `web/js/sync.js` is the workspace copy; `crates/transync-cli/web/sync.js` is the CLI-embedded twin (`include_str!` at `crates/transync-cli/src/output.rs`). There is **no build step and no generator** — the twin is kept in sync by a literal file copy, and `crates/transync-cli/tests/sync_js_drift.rs::sync_js_workspace_and_embedded_are_byte_identical` compares `std::fs::read` of both, i.e. **full byte identity**, not a looser structural check. The copy direction used here is always workspace → embedded: `cp web/js/sync.js crates/transync-cli/web/sync.js`.
- **The browser suite exercises the EMBEDDED twin for `index.html` and `engine.spec.js`.** `scripts/test-browser.sh` regenerates the bundle with a freshly built `transync-cli`, and that binary `include_str!`s `crates/transync-cli/web/sync.js` into `$HTML_OUT/sync.js`; `engine.spec.js` imports `/sync.js` and the bundle shell imports `./sync.js`. Only `demo-wasm.html` (`wasm.spec.js`) loads the workspace copy, via the script's `cp "$WEB_DIR/js/sync.js" "$HTML_OUT/js/sync.js"`. **Editing only `web/js/sync.js` and running the browser suite tests the OLD engine.** Copy first, always.
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`.
- Every Rust test run: `cargo test -p <crate> -- --test-threads=4`; workspace runs: `cargo test --workspace -- --test-threads=4`. **Never raise the cap.**
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append the exit code, and inspect the file as a *separate* step. A pipeline reports the last stage's status, so a failing suite reads as a pass.
- Lint gates (the pre-commit hook enforces them): `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- **`git commit --no-verify` is never used.** A commit step's expected result is that the hook runs fmt, clippy, the wasm gate and the rustdoc gate, and all four pass. If the hook blocks, fix the cause. (The hook's JS leg reports `SKIP: prettier not installed` / `SKIP: eslint not installed` — that is its normal output here, not a failure.)
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave1/` — never `/tmp`, never `/private/tmp`, never `$TMPDIR`. The browser fixture keeps its own default location, `/Volumes/Temp/claude/transync-browser-fixture/`, which is already under the temp root. If `/Volumes/Temp/claude` is unreachable, stop and ask.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`. If a cargo command fails because the target dir is unreachable, stop and ask.
- No pure-formatting edits. Let `cargo fmt` own wrapping — where this plan shows wrapped Rust, run `cargo fmt --all` afterwards and take the formatter's answer. The JS has no formatter installed; match the file's existing two-space indent by hand.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, or cite them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.
- **A new document under `docs/` turns `docs_index_drift` red the moment it exists**, because that test walks the filesystem and requires every `.md` under `docs/` to be linked from `docs/index.md`. The link must land in the same change that creates the file. **This plan is itself such a document.** The planning agent that wrote it was not permitted to touch `docs/index.md`, so the link was added afterwards by the orchestration session — it is already in the working tree, and `docs_index_drift` is therefore **green on disk before Task 1 runs**. Task 1 Step 1 **verifies** that link rather than adding a second one; do not paste a line in. What Task 1 must still do early is *commit* — the same test resolves every index link against the filesystem, so a tracked index pointing at an untracked plan file is green here and a dead link on a fresh clone. The lesson is unchanged and only its action moves: file and link are one change, and the commit is what makes that true for anyone but you.
- **Do not edit `crates/transync-html/src/lib.rs`.** `strip_reserved_sync_attrs` ships from wave 0 with its own six tests. Wave 1 consumes it; it does not re-implement, re-name, or re-tune it.

---

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `docs/index.md` | verify (Task 1), modify (Task 4) | Task 1 checks that this plan's link is already there and commits the plan file the link points at; Task 4 links DCR-0033 |
| `crates/transync-syntax/src/render.rs` | modify | the `BlockKind::Html` success arm strips before it wraps, plus the unit test that pins it |
| `crates/transync/tests/fixtures/oi-0035-impostor-anchor.md` | **create** | the end-to-end specimen: a raw-HTML block whose open tag claims the id of the paragraph that follows it |
| `scripts/test-browser.sh` | modify | the OI-0035 leg — a second `--html-out` bundle at `$HTML_OUT/oi0035/`, generated from that fixture with the same stub binary |
| `web/tests/scn13.spec.js` | modify | test `m` — the end-to-end case: the bundle's `source.html` carries no impostor attribute, and the pane holds exactly one claimant |
| `web/tests/engine.spec.js` | modify | new tests `k` and `l` — the engine-direct cases: unlisted anchors inert + warned at mount; listed duplicates still first-occurrence-wins; a post-mount arrival stays inert through the reflow that used to activate it. Plus one assertion added to the **existing** test `b`, pinning the R0002-0047 refusal the gate must not swallow |
| `web/js/sync.js` | modify | `synchronizableRowIds`, the `collectAnchors` gate, the reflow thread-through, the R0002-0047 refusal moved above the collection and kept on an ungated DOM count, the docstrings |
| `crates/transync-cli/web/sync.js` | modify (`cp` of the above) | the byte-identical embedded twin |
| `docs/architecture/contracts.md` | modify (Task 2, Task 3) | Task 2: §4's reserved-namespace rule, beside the render change that makes it true. Task 3: §4's engine-reads-listed-ids sentence, §3's cross-reference to it, **and** §4a's anchor-set-from-rows paragraph with the residual — all in the commit that changes the engine, so no commit carries a docs claim the code has not made yet |
| `docs/project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md` | **create** | the wave's record |
| `docs/project/open-issues.md` | modify | OI-0035 → RESOLVED, entry moved to the archive, summary-table row updated |
| `docs/project/open-issues-archive.md` | modify | receives the resolved OI-0035 entry with its archive banner |
| `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml` | modify | routine per-wave records |

**Not touched, deliberately:** `crates/transync-html/src/lib.rs` (wave 0 owns the function), `crates/transync/tests/fixtures/scn-14-full.md` (the shared SCN-12/13/14 corpus stays byte-identical — the end-to-end case gets its own fixture and its own bundle rather than polluting the canonical one), `crates/transync-cli/tests/sync_js_drift.rs` (it is the weld, unchanged, per spec §11's "Twins" row).

---

## Deviations from the spec

Two, both narrow, both scope calls rather than disagreements. They are named here because undeclared divergence is what makes a later reviewer distrust a plan that was right on everything else.

1. **Spec §11's OI-0035 test row asks for "a Rust unit on `strip_reserved_sync_attrs`"; this plan writes a Rust unit at the *call site* instead** (Task 2, Step 1 — `html_block_impostor_sync_attributes_never_reach_the_pane`, in `crates/transync-syntax/src/render.rs`). **Why:** wave 0 Task 6 already ships six units on the function itself — case-insensitive removal of all six names, non-reserved attributes and content surviving, the borrow-when-nothing-matched path, RCDATA/comment interiors, self-closing and close tags, and tag-inventory invariance — and this plan's Global Constraints forbid touching `crates/transync-html/src/lib.rs`. A seventh unit there would re-test wave 0's contract; the thing wave 1 has to prove is that the arm *calls* it, on the live-render path, before the wrapper writes ours. That is what the call-site unit pins, and the end-to-end browser case (Task 2, Step 5) proves the same fact through the bundle. §11's row is satisfied in substance — a Rust unit covering the OI-0035 render half — at the layer this wave owns.
2. **Spec §10 pairs contracts.md §4's reserved-namespace rule with a "HTML-source-panes subsection"; this plan lands only the rule** (Task 2, Step 10). **Why:** that subsection documents the HTML pane derivation — own-element injection as a deliberate divergence, the transparent wrapper for element-less blocks, head/gap/script/style exclusion. That producer is **wave 6's** and does not exist after wave 1; documenting it here would put a contract in `contracts.md` describing code no reader can go and check. §10's bullet spans the whole feature, not one wave, and its parenthetical already says the reserved-namespace rule "also lands in the Markdown html-block paragraph" — which is exactly the half this wave can make true. The subsection stays wave 6's, with the §4a sentence extending the outer-wrapper contract to both formats.

**Not a deviation, but the interaction that constrains Task 3.** Spec §8 asserts two things at once: `collectAnchors` takes the row-id set and skips what is not in it, *and* "a title-only map still trips the empty-map gate when panes carry anchors". Both hold only if R0002-0047's anchor count is read off the DOM ungated and the refusal runs before the gated collection — see Task 3, Step 7. An implementation that counts collected anchors satisfies the first clause and silently destroys the second.

---

### Task 1: Verify this plan's link, commit the file it points at, and capture the baseline

**Files:**
- Verify (do **not** edit): `docs/index.md`
- Commit: `docs/superpowers/plans/2026-08-20-html-wave1-oi0035-anchor-trust.md` (this file, currently untracked), together with `docs/index.md` if it is still uncommitted

**Interfaces:**
- Consumes: nothing. This is the first task.
- Produces: a tracked plan file under a tracked index link — the pair `docs_index_drift` checks in both directions — and a recorded before-state that every later "it was green before" claim rests on.

- [ ] **Step 1: Verify this plan's link already exists in `docs/index.md`.** The orchestration session added it when this file was written; the planning agent could not. **Do not paste a line in** — a second entry for the same file is a duplicate, not a fix.
```bash
grep -c 'superpowers/plans/2026-08-20-html-wave1-oi0035-anchor-trust.md' /Volumes/Common/QJoon/transync/docs/index.md
```
Expected: `1`, exactly. If it prints `0`, the link is genuinely missing — add one line immediately **after** the wave-0 entry (`- [HTML→HTML Wave 0 — the \`transync-html\` crate — implementation plan (2026-08-20)](…)`) and before the blank line preceding `## Browser demo`, naming this file and summarizing it in the neighbours' voice. If it prints `2` or more, an entry was pasted in on top of the existing one: delete the duplicate, keep the one already in the file, and do not "merge" their wording.

- [ ] **Step 2: Verify the weld both files depend on is green.** `docs_index_drift` checks two directions — every `.md` under `docs/` is linked, and every index link resolves on disk — so this run is what proves Step 1's finding rather than assuming it.
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave1/gate
cargo test -p transync --test docs_index_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t1-index.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t1-index.txt
```
Read the file as a separate step. Expected: `CARGO_EXIT=0` and `test result: ok.` — the tree is green here *before* the wave starts, because the link is already in place. If it is red, read the failure: an "unlinked document" list names files nobody linked (link what it names — do not add an exclusion), while a "dead link" failure names an index entry pointing at a path that does not exist, which means Step 1's `grep` found a line for a file spelled differently from this one. Fix the mismatch before going further; everything after this point assumes the pair is intact.

- [ ] **Step 3: Baseline capture.** Wave 1 changes behaviour, so most of it is red-first — but "the browser suite was green before I touched it" is a claim that needs a before-state, and the browser suite is slow enough that discovering a pre-existing failure at Step 9 of Task 2 wastes an hour.
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-cli.txt
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-commit.txt
```

- [ ] **Step 4: Baseline the browser suite.** This is the run that also proves wave 0 left `transync-html` in a state the demo bundle builds from.
```bash
./scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-browser.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/baseline-browser.txt
```
Read all four capture files as a separate step. Expected: `CARGO_EXIT=0` in both cargo files, `PW_EXIT=0` in the browser file, the browser file ending `[test-browser] OK — SCN-13 headless suite passed`, and no line containing `FAILED` or `failed`. **If any is non-zero, STOP** — the tree was not green before this wave and nothing after this point can be attributed to it.

- [ ] **Step 5: Commit the plan, and the link if it is still uncommitted.** The link may already be tracked — wave 0's own first commit stages `docs/index.md`, and this file's entry sits in the same working-tree change as wave 0's. Check first, so the commit says what it does:
```bash
git status --porcelain -- docs/index.md docs/superpowers/plans/2026-08-20-html-wave1-oi0035-anchor-trust.md
git diff -- docs/index.md
```
Expected: `?? docs/superpowers/plans/2026-08-20-html-wave1-oi0035-anchor-trust.md`, plus either nothing for `docs/index.md` (wave 0's commit already carried the link — commit the plan file alone) or ` M docs/index.md` whose diff adds **only** this plan's entry. A third state is legitimate: if **both** paths print nothing, the controller already committed the plan file with its link (the plan went through a reviewed fix round after it was written, and committing the result was the controller's), so the pair this step exists to create is already tracked — Step 2 has proven it green — and there is nothing to commit; check the box and move on rather than manufacturing an empty commit. **If the diff also adds another wave's entry whose plan file is still untracked, do not run the commands below:** `git add docs/index.md` stages the whole file, so committing it would publish a link to a file nobody has committed — the dead link this task exists to prevent — and the other wave's plan file is not yours to commit. Where the problem goes depends on whether that file exists on disk. If it does **not** exist, the index points at nothing: that is a defect in the index edit itself — report it. If it **does** exist (the normal case: the controller writes every wave's plan before dispatching any of them), **escalate to the controller to commit each sibling plan file together with its index line, then re-run this step once the index diff shows only this wave's entry or none.** Waiting for "that wave to land first" does not resolve here: every wave's Task 1 carries this same guard, so the standoff is symmetric — wave 2's Task 1 would be waiting on this wave's entry exactly as this step waits on wave 2's — and only the controller, who owns all the plan files, can break the cycle. However it resolves, never commit `docs/index.md` wholesale.
```bash
git add docs/superpowers/plans/2026-08-20-html-wave1-oi0035-anchor-trust.md docs/index.md
git commit -m "docs(plan): wave 1's plan lands under the index entry that names it

docs_index_drift checks both directions: every document under docs/ is linked,
and every index link resolves on disk. The second check passes for an untracked
file, so an index entry can be tracked while the document it names is not —
green here, a dead link on a fresh clone. The file is committed with its link,
not after it.

TRACE: ti 490d97 wave 1
TRACE: OI-0035

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
(`git add` on an unmodified path is a no-op, so the one command is correct in both cases above.)

---

### Task 2: The render half — strip the reserved namespace at the Markdown `BlockKind::Html` success arm

**Files:**
- Modify: `crates/transync-syntax/src/render.rs`
- Create: `crates/transync/tests/fixtures/oi-0035-impostor-anchor.md`
- Modify: `scripts/test-browser.sh`
- Modify: `web/tests/scn13.spec.js`
- Modify: `docs/architecture/contracts.md` (§4's **HTML block** row only — its engine sentence belongs to Task 3's commit)

**Interfaces:**
- **Consumes, from wave 0 Task 6 — do not re-implement:**
  ```rust
  // in crates/transync-html/src/lib.rs, called as transync_html::strip_reserved_sync_attrs
  pub fn strip_reserved_sync_attrs(html: &str) -> Cow<'_, str>;
  ```
  It removes, case-insensitively, the six reserved attribute names — `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`, `data-skipped` — from element **open tags** only, taking each removed attribute's leading whitespace with it, and borrows when nothing matched. Attribute *values* and text content are never inspected; RCDATA and comment interiors are unreachable because `scan_tags` never tokenizes them; `tag_inventory` is unchanged by it, because attributes are not structure.
- Also consumes, from wave 0 Task 1: `transync_html::balance_fragment(html: &str) -> String`, already the arm's call.
- **Produces:** no new public item. The arm's emitted bytes change: for a live-rendered html block the pane's fragment is `balance_fragment(strip_reserved_sync_attrs(md))` instead of `balance_fragment(md)`.
- The **failure** arm (`FallbackStatus::FallbackSource` → `<pre data-skipped="html-block">` with `html_escape(md)`) is deliberately **not** stripped: its payload is escaped text, so an impostor attribute there is characters on the screen, not markup in the DOM.
- `Cow<'_, str>` derefs to `&str`, so `balance_fragment(&strip_reserved_sync_attrs(md))` compiles with no intermediate binding; the temporary lives to the end of the statement.

- [ ] **Step 1: Write the failing Rust unit test at the call site.** In `crates/transync-syntax/src/render.rs`, inside `mod html_render_tests`, append after `fallback_html_block_renders_the_escaped_placeholder`:
```rust
    /// OI-0035 route (c), render half (spec 2026-08-20 §8). A `data-sync-id`
    /// written into a source raw-HTML block used to reach the pane verbatim,
    /// where the engine could not tell it from an anchor the renderer emitted
    /// — so it could pre-claim a real block's id and become that block's
    /// scroll driver. Source Markdown is untrusted data (invariant 7) and this
    /// arm is the only live route by which source-controlled markup reaches a
    /// Markdown pane, because panes render with `unsafe_ = false`.
    ///
    /// The specimen puts the impostor BEFORE the block it impersonates, which
    /// is the shape the engine layer cannot defeat on its own: a listed id
    /// preceding the genuine anchor wins `collectAnchors`' first-occurrence
    /// policy. This is the layer that closes it.
    #[test]
    fn html_block_impostor_sync_attributes_never_reach_the_pane() {
        let html = render_with_status(
            "<div data-sync-id=\"p-0002\" DATA-Order=\"99\" class=\"note\">side note</div>\n\npara\n",
            FallbackStatus::Preserved,
        );
        assert_eq!(
            html.matches("data-sync-id=\"p-0002\"").count(),
            1,
            "only the real p-0002 paragraph may claim p-0002; the impostor in \
             the html block's own bytes must not survive into the pane:\n{html}"
        );
        assert!(
            !html.contains("DATA-Order"),
            "the strip is case-insensitive over the whole reserved namespace:\n{html}"
        );
        assert!(
            html.contains("<div class=\"note\">side note</div>"),
            "everything outside the namespace is content and survives \
             untouched, including the block's own text:\n{html}"
        );
        assert!(
            !html.contains("data-skipped=\"html-block\""),
            "still the live-render arm, not the escaped placeholder:\n{html}"
        );
    }
```

- [ ] **Step 2: Run it and see it fail.**
```bash
cargo test -p transync-syntax --lib html_block_impostor -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-red-rust.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-red-rust.txt
```
Read the file separately. Expected: `CARGO_EXIT=101`, `test result: FAILED. 0 passed; 1 failed`, and inside the failure:
```
assertion `left == right` failed: only the real p-0002 paragraph may claim p-0002; …
  left: 2
 right: 1
```
`left: 2` is the impostor plus the genuine anchor. If `left` is `1` already, the arm is not the one being edited — stop and re-read `render_block`.

- [ ] **Step 3: Write the end-to-end fixture.** `crates/transync/tests/fixtures/oi-0035-impostor-anchor.md`, exactly:
```markdown
# OI-0035 — an impostor anchor written into source HTML

<div data-sync-id="p-0003" data-block-kind="paragraph" data-order="0" data-fallback="translated" class="impostor-marker">
This raw HTML claims the id of the paragraph below it. It is content, and it
translates like any other html block — but its sync attributes are not ours.
</div>

This paragraph is the real `p-0003`: the block the raw HTML above tries to
pre-claim. In document order the impostor comes first, which is exactly the
shape the engine layer cannot defeat by itself.
```
Block ids fall out of the shared ordinal counter: `h1-0001`, `html-0002`, `p-0003`. The impostor therefore claims a real id **and precedes it**, which is the case the render half exists for. Keep the fixture's html block free of interior blank lines — a blank line would end the CommonMark type-6 html block and split it in two, moving every id after it.

- [ ] **Step 4: Add the OI-0035 leg to `scripts/test-browser.sh`.** Insert immediately **after** the wasm-demo leg's closing `done` (the loop that checks `$HTML_OUT/out.md`) and **before** the `cd "$WEB_DIR"` line:
```bash
# --- OI-0035 leg (route (c), render half) ---------------------------------
#
# A SECOND `--html-out` bundle, generated from a fixture whose raw-HTML block
# carries a `data-sync-id` colliding with a real block id. It lives inside the
# served fixture dir (like the wasm leg above) so `scn13.spec.js` can both read
# its files off disk and navigate to it; it is scratch only, and nothing here
# touches the six-file bundle contract or the SCN-14 corpus.
#
# The canonical SCN-12/13/14 fixture is deliberately NOT the specimen: an
# impostor attribute in it would move SCN-13's own expectations for a case
# that wants its own document.
echo "[test-browser] regenerating the OI-0035 bundle -> $HTML_OUT/oi0035"
OI0035_INPUT="$REPO_ROOT/crates/transync/tests/fixtures/oi-0035-impostor-anchor.md"
if [[ ! -f "$OI0035_INPUT" ]]; then
  echo "[test-browser] FAIL: OI-0035 fixture not found: $OI0035_INPUT" >&2
  exit 1
fi
"$TRANSYNC_BIN" translate \
  --input "$OI0035_INPUT" \
  --output "$WORKDIR/oi0035.md" \
  --map "$WORKDIR/oi0035.json" \
  --html-out "$HTML_OUT/oi0035" \
  --target-language ko

# All six bundle files, mirroring the main leg's loop — purify.min.js
# included: a sub-bundle missing the sanitizer would otherwise surface as an
# opaque waitForMounted timeout in test `m` instead of this leg's named FAIL.
for path in \
  "$HTML_OUT/oi0035/index.html" \
  "$HTML_OUT/oi0035/source.html" \
  "$HTML_OUT/oi0035/target.html" \
  "$HTML_OUT/oi0035/alignment.json" \
  "$HTML_OUT/oi0035/sync.js" \
  "$HTML_OUT/oi0035/purify.min.js"
do
  if [[ ! -s "$path" ]]; then
    echo "[test-browser] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

# The strip is PANE-ONLY: out.md keeps the author's bytes, because their
# data-sync-id is their content. That sentence is written into render.rs,
# contracts.md §4, DCR-0033 and the archived issue — and this check is the one
# place anything READS the published Markdown to hold them to it. Every other
# assertion in this wave looks at pane HTML, so a strip wired one layer too
# deep — into regen or the splice, now or by a later "centralization" —
# changes zero pane bytes, passes everything above, and silently deletes
# author content from out.md. A contract sentence no test reads is the shape
# that rots.
#
# `-lt 1`, not exactly 1: the attribute survives translation by splice
# construction (and byte-verbatim through fallback), so >=1 holds on any
# correct implementation — but its exact multiplicity in the translated
# document belongs to the stub's behaviour, not to this contract. The only
# thing pane-only forbids is LOSS, and loss is what -lt 1 catches; pinning
# the count would turn an unrelated stub change into a false red here.
if [[ "$(grep -c 'data-sync-id="p-0003"' "$WORKDIR/oi0035.md")" -lt 1 ]]; then
  echo "[test-browser] FAIL: out.md lost the author's bytes — the strip must be pane-only (OI-0035)" >&2
  exit 1
fi
```

- [ ] **Step 5: Write the failing end-to-end browser test.** In `web/tests/scn13.spec.js`, append after test `l` and before the closing `});` of the `test.describe` block:
```js
  test("m — an impostor data-sync-id written into source HTML never reaches a pane", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // OI-0035 route (c), render half, end to end. The fixture's raw-HTML block
    // claims `p-0003` — the id of the paragraph that FOLLOWS it — so before the
    // strip the pane held two elements claiming p-0003 and the impostor, being
    // first in document order, won the engine's first-occurrence lookup and
    // became that paragraph's scroll driver. The bundle is a second
    // `--html-out` run inside the fixture dir (scripts/test-browser.sh, the
    // OI-0035 leg).
    const sourceHtml = readFixture("oi0035/source.html");
    const targetHtml = readFixture("oi0035/target.html");

    // Guards first: the assertions below are only meaningful while the block
    // takes the LIVE-render arm. On the escaped-placeholder arm the impostor's
    // quotes become `&quot;` and every "does not contain" would pass vacuously.
    expect(sourceHtml).toContain('class="impostor-marker"');
    expect(sourceHtml).not.toContain('data-skipped="html-block"');
    // The target pane carries the guard too: if the stub's translation of the
    // html block ever fell back, the target block would be the escaped
    // placeholder — its count of 1 below would then hold without the strip
    // ever running on that pane. Same vacuous pass, seen from the other side.
    expect(targetHtml).not.toContain('data-skipped="html-block"');

    expect(sourceHtml.split('data-sync-id="p-0003"').length - 1).toBe(1);
    expect(targetHtml.split('data-sync-id="p-0003"').length - 1).toBe(1);
    expect(sourceHtml).not.toContain('data-fallback="translated" class=');

    // And in the DOM the bundle actually mounts.
    await page.goto("/oi0035/");
    await waitForMounted(page);

    expect(
      await page.evaluate(
        () => document.querySelectorAll('#source [data-sync-id="p-0003"]').length
      )
    ).toBe(1);
    expect(
      await page.evaluate(
        () => document.querySelector('#source [data-sync-id="p-0003"]').tagName
      )
    ).toBe("P");
    // A nested anchor is the same defect seen from the other side: the impostor
    // sat inside the html block's own wrapper (contracts.md §4a wants every
    // anchor a direct child of <main>, list items excepted).
    expect(
      await page.evaluate(
        () => document.querySelectorAll("[data-sync-id] [data-sync-id]").length
      )
    ).toBe(0);
    expect(logs.some((m) => m.includes("duplicate data-sync-id"))).toBe(false);

    // Spec §15 item 3: DOMPurify's `ALLOW_DATA_ATTR` default was INFERRED when
    // route (c) was chosen, never measured. Measure it here, against the
    // vendored build the bundle actually ships, because the render half's
    // necessity rests on it — a sanitizer that dropped `data-*` would already
    // have closed this route. The DCR's mechanism sentence quotes this.
    expect(
      await page.evaluate(() =>
        window.DOMPurify.sanitize('<div data-sync-id="p-0003">x</div>')
      )
    ).toContain('data-sync-id="p-0003"');

    expect(errors).toEqual([]);
  });
```
No import changes: `collectConsole`, `collectPageErrors`, `readFixture` and `waitForMounted` are already imported at the top of this file.

- [ ] **Step 6: Run the browser suite and see the end-to-end case fail.** The script rebuilds the stub CLI, regenerates both bundles, rebuilds the wasm demo, and runs Playwright; extra arguments are forwarded to `playwright test`.
```bash
./scripts/test-browser.sh tests/scn13.spec.js -g "impostor" > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-red-browser.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-red-browser.txt
```
Read the file separately. Expected: `PW_EXIT=1`, and — this is what a failing Playwright assertion looks like under the `list` reporter — a block of the shape:
```
  1) [chromium] › tests/scn13.spec.js:NNN:3 › SCN-13 dual-pane sync › m — an impostor data-sync-id written into source HTML never reaches a pane

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: 1
    Received: 2

      at /Volumes/Common/QJoon/transync/web/tests/scn13.spec.js:NNN:NN
```
followed by `1 failed`. `Received: 2` is the impostor plus the genuine anchor, the same count the Rust unit reported. If instead the run dies before Playwright starts with `[test-browser] FAIL: …/oi0035/source.html missing or empty`, the leg in Step 4 is misplaced or the fixture path is wrong — fix that first; it is not the red this step wants.

- [ ] **Step 7: Implement the strip.** In `crates/transync-syntax/src/render.rs`, in `render_block`'s `BlockKind::Html` arm, replace the `else` branch:
```rust
        } else {
            let _ = writeln!(
                out,
                "<div{attrs}>{}</div>",
                transync_html::balance_fragment(md),
            );
        }
```
with:
```rust
        } else {
            // OI-0035 route (c), render half (spec 2026-08-20 §8): strip the
            // reserved sync-attribute namespace out of the block's own bytes
            // before the wrapper writes ours. Strip-then-inject is what makes
            // "ours are the only sync attributes in this DOM" a construction
            // rather than a scan — the engine's row gate covers unlisted ids,
            // but a LISTED id planted ahead of the genuine anchor wins
            // first-occurrence-wins, and only this layer closes that.
            //
            // Pane-only. `out.md` keeps the author's bytes: their
            // `data-sync-id` is their content, and we own this namespace only
            // in DOM we mount. Stripping never changes rendered appearance,
            // because attributes do not paint.
            //
            // The failure arm above needs none of this: its payload is
            // HTML-escaped, so an impostor attribute there is text.
            let _ = writeln!(
                out,
                "<div{attrs}>{}</div>",
                transync_html::balance_fragment(&transync_html::strip_reserved_sync_attrs(md)),
            );
        }
```

- [ ] **Step 8: Run the Rust unit and see it pass.**
```bash
cargo test -p transync-syntax --lib html_render_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-green-rust.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-green-rust.txt
```
Expected: `CARGO_EXIT=0`, `test result: ok. 5 passed` (the four shipped presentation tests plus the new one).

- [ ] **Step 9: Run the whole browser suite and see it pass.** The whole suite, not just `-g impostor`: the strip changes bytes in every pane that carries an html block, so SCN-13's `h` (live render, no nested wrappers, details mirroring) and `wasm.spec.js`'s html-0018 assertions are the ones that would notice a strip that took too much.
```bash
./scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-green-browser.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-green-browser.txt
```
Expected: `PW_EXIT=0` and the file ending `[test-browser] OK — SCN-13 headless suite passed`.

- [ ] **Step 10: Land the contracts.md §4 rule the strip makes true.** One edit, in §4's **HTML block** row. (§4's *other* OI-0035 sentence — "the engine reads `[data-sync-id]` only" — is **not** touched here. It describes engine behaviour that does not change until Task 3, and it moves in Task 3's commit, beside the §4a paragraph it points at. Editing it now would put a docs claim one commit ahead of the code that makes it true.)

  In the **Success** bullet, append at the end of the bullet (after `…a visually empty wrapper is legitimate when the sanitizer strips the whole payload (comment-only, \`script\`-only).`):
```markdown
 **The reserved attribute namespace is stripped out of the block's own bytes before the wrapper is written (OI-0035, ti `490d97` wave 1).** `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id` and `data-skipped` are removed case-insensitively from every open tag inside the fragment, so the sync attributes a pane carries are exactly the ones the renderer put there — a construction, not a scan. Source Markdown is untrusted data (invariant 7) and DOMPurify's default keeps `data-*` attributes (**measured** 2026-08-20 against the vendored build, `web/tests/scn13.spec.js`), so without the strip an author's `data-sync-id` reached the DOM as an anchor the engine could not tell from the renderer's own. The strip is **pane-only**: `out.md` keeps the author's bytes, because their `data-sync-id` is their content and this namespace is owned only in DOM transync mounts. It never changes rendered appearance — attributes do not paint — and it never moves the tag inventory, because attributes are not structure. The **failure** presentation below needs no strip: its payload is HTML-escaped, so an impostor attribute there is text.
```

- [ ] **Step 11: Verify the workspace and the lints.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-clippy.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t2-workspace.txt
git status --porcelain -- crates/transync/tests/fixtures crates/transync-cli/tests > /Volumes/Temp/claude/ti490d97-wave1/gate/t2-fixtures.txt
```
Read the three files separately. Expected: `CARGO_EXIT=0` in both cargo files; `t2-fixtures.txt` containing exactly one line, `?? crates/transync/tests/fixtures/oi-0035-impostor-anchor.md` — a **new** fixture is fine, a modified existing one is not.

- [ ] **Step 12: Commit.**
```bash
git add crates/transync-syntax/src/render.rs \
  crates/transync/tests/fixtures/oi-0035-impostor-anchor.md \
  scripts/test-browser.sh \
  web/tests/scn13.spec.js \
  docs/architecture/contracts.md
git commit -m "fix(render): a pane carries our sync attributes and only ours

OI-0035's render half. The Markdown path's BlockKind::Html success arm is the
only live route by which source-controlled markup reaches a pane — panes
render with unsafe_ = false — and it used to copy the author's bytes through
verbatim. A data-sync-id written into a raw HTML block therefore landed in the
DOM as an anchor the engine could not tell from one the renderer emitted, and
could pre-claim a real block's id.

The arm now runs the block through transync_html::strip_reserved_sync_attrs
before the wrapper writes ours: strip-then-inject makes 'ours are the only
ones' a construction rather than a scan. Pane-only — out.md keeps the author's
bytes, because their data-sync-id is their content.

Proven twice: a unit at the call site, and an end-to-end case over a second
--html-out bundle whose fixture's html block claims the id of the paragraph
that follows it. The same run measures what route (c) had only inferred —
DOMPurify's default really does keep data-* attributes.

TRACE: ti 490d97 wave 1
TRACE: OI-0035
TRACE: contracts.md §4

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The engine half — `collectAnchors` gated on the validated rows, both twins in one commit

**Files:**
- Modify: `web/tests/engine.spec.js` (new tests `k` and `l`; one assertion added to the existing test `b`)
- Modify: `web/js/sync.js`
- Modify: `crates/transync-cli/web/sync.js` (by `cp`, never by hand)
- Modify: `docs/architecture/contracts.md` (§4's engine sentence, §3's cross-reference to it, and §4a's new paragraph)

**Interfaces:**
- **New module-private function in `sync.js`:**
  ```js
  function synchronizableRowIds(alignmentMap) -> Set<string>
  ```
  The ids of every row whose `sync_role !== "non-sync"`. `loadAlignment` has already refused any map whose rows are not objects with a unique non-empty string `source_block_id`, so this runs over rows it can trust.
- **Changed signature:**
  ```js
  function collectAnchors(pane, label, rowIds, quiet) -> { blocks: HTMLElement[], byId: Map<string, HTMLElement> }
  ```
  `rowIds` is inserted as the **third** parameter and `quiet` moves to fourth. **Four call expressions move with it, not three:** two in `mountSync` (Step 7) and two in `wireReflowRecompute`'s `recompute` (Step 8). A reader who stops after the mount pair leaves the reflow path calling a four-parameter function with three arguments — `rowIds` would arrive as `true` and `quiet` as `undefined`, so every reflow would re-collect against a boolean with no `.has`, and the first reflow would throw.
- **Changed body, same signature:**
  ```js
  function synchronizableRowCount(alignmentMap) -> number   // === synchronizableRowIds(alignmentMap).size
  ```
  Deriving the count from the set is what makes "the exact predicate `synchronizableRowCount` uses" literally one function instead of two that agree today. Safe because `validateRows` has already refused a duplicate `source_block_id`, so rows and distinct ids are the same number for any map that reaches here; `mountSync` is its only caller.
- **Changed shape:**
  ```js
  function wireReflowRecompute({ sourcePane, targetPane, sourceCtx, targetCtx, state, rowIds })
  ```
  The recompute closes over `rowIds` so a reflow re-collection is gated on the same set as the mount.
- **Unchanged on purpose, and the thing most easily broken here:** the R0002-0047 refusal in `mountSync` — "the map describes no synchronizable block, but the panes carry anchors" — keeps counting anchors with an **ungated** `pane.querySelectorAll("[data-sync-id]")`, and moves to *before* the two `collectAnchors` calls. Counting collected anchors instead makes its condition unsatisfiable and deletes the refusal in silence; Step 7 carries the mechanism and Step 3 the test that catches it.
- Consumes from Task 2: nothing in code. The two halves are independent; this one is sequenced second so no commit ever leaves the browser suite red.
- **Carries `contracts.md` §4's engine sentence, §3's cross-reference to it, and §4a's paragraph** (Step 13). All three describe the behaviour this commit introduces, so all three land in this commit: at no point does the tree hold a contract the code has not yet made true.

- [ ] **Step 1: Add the rig helper the new tests need.** In `web/tests/engine.spec.js`, immediately after the `appendBlock` helper and before `test.describe(`:
```js
/** Plant one anchor carrying `id` at the top or the bottom of a rig pane. */
function plantAnchor(page, paneId, id, px, where) {
  return page.evaluate(
    ([paneId, id, px, where]) => {
      const pane = document.getElementById(paneId);
      const el = document.createElement("p");
      el.dataset.syncId = id;
      el.style.cssText = `height:${px}px;margin:0;padding:0`;
      el.textContent = id;
      if (where === "top") pane.insertBefore(el, pane.firstChild);
      else pane.appendChild(el);
    },
    [paneId, id, px, where]
  );
}
```

- [ ] **Step 2: Write the failing engine-direct tests.** In `web/tests/engine.spec.js`, append after test `j` and before the closing `});` of the `test.describe` block:
```js
  test("k — an anchor no alignment row claims cannot drive the follower", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // OI-0035 route (c), engine half. Source content is untrusted data
    // (architectural invariant 7), raw HTML is translatable
    // structurally-owned content, and DOMPurify's default keeps `data-*` — so
    // a `data-sync-id` an author writes reaches BOTH panes. DOM membership
    // used to decide what could drive scroll while map membership decided
    // nothing at all; the validated rows are the authority now.
    //
    // The impostor sits at the TOP of the driver and the BOTTOM of the
    // follower, so if it were collected it would answer for the reader's very
    // first block and fling the follower to the other end of the document.
    await plantAnchor(page, "eng-source", "x-9999", BLOCK_PX, "top");
    await plantAnchor(page, "eng-target", "x-9999", BLOCK_PX, "bottom");

    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    for (const pane of ["source", "target"]) {
      expect(
        logs.some(
          (m) =>
            m.includes('ignoring anchor "x-9999"') &&
            m.includes(`${pane} pane`) &&
            m.includes("no alignment row claims it")
        ),
        pane
      ).toBe(true);
    }
    // Two policies, kept distinct: an unlisted anchor is not a duplicate, and
    // reporting it as one would send a reader hunting for a producer defect
    // that is not there.
    expect(logs.some((m) => m.includes("duplicate data-sync-id"))).toBe(false);

    // Park the reader mid-document, so "the follower came home" is a claim
    // about a pane that had somewhere else to be.
    const third = await offsetTopOf(page, TGT, "e-0003");
    await setScrollTop(page, SRC, await offsetTopOf(page, SRC, "e-0003"));
    await waitForScrollNear(page, TGT, third, 6);
    await page.waitForTimeout(160); // let the follower's 90 ms lock decay

    // Back to the top, where the impostor straddles the driver's reference
    // line. Collected, it pairs with the follower's copy 750 px down and the
    // follower runs to its maximum; ignored, the scan falls through to
    // `e-0001` and the follower comes home.
    await setScrollTop(page, SRC, 0);
    await waitForScrollNear(page, TGT, 0, 4);

    expect(errors).toEqual([]);
  });

  test("l — a duplicate the map does claim keeps its first occurrence, and an anchor that arrives after mount stays inert", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // First half. The row gate runs BEFORE the duplicate policy, so the policy
    // has to be shown intact (R0001-0044): a SECOND `e-0001` at the bottom of
    // the follower is a producer defect, warned about, and the FIRST
    // occurrence is what both the scan array and the partner lookup name.
    await plantAnchor(page, "eng-target", "e-0001", BLOCK_PX, "bottom");
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(
      logs.some(
        (m) =>
          m.includes("duplicate data-sync-id") &&
          m.includes("e-0001") &&
          m.includes("keeping the first occurrence")
      )
    ).toBe(true);

    const third = await offsetTopOf(page, TGT, "e-0003");
    await setScrollTop(page, SRC, await offsetTopOf(page, SRC, "e-0003"));
    await waitForScrollNear(page, TGT, third, 6);
    await page.waitForTimeout(160);
    await setScrollTop(page, SRC, 0);
    // The first e-0001 at offsetTop 0, never the copy 750 px down.
    await waitForScrollNear(page, TGT, 0, 4);

    // Second half — the 2026-08-09 correction's residual, closed. A reflow
    // recompute ACTIVATES an anchor that entered the DOM after mount, with the
    // duplicate audit deliberately suppressed, so before this gate there was a
    // way into the live anchor set with no audit at any point. Now the row list
    // decides: a late arrival is inert on the same terms as one that was there
    // all along, and the recompute stays quiet about it.
    // 400 px, not BLOCK_PX: the driver's reachable scroll range is
    // `content - 200`, so a 150 px tail would sit entirely below the deepest
    // reference line the pane admits and the case could never be exercised.
    // At 400 px the band the impostor occupies is genuinely reachable.
    await appendBlock(page, "x-8888", 400);
    await setPaneHeight(page, "eng-target", 260);
    await advanceFrames(page, 6);
    expect(logs.some((m) => m.includes("x-8888"))).toBe(false);
    await page.waitForTimeout(160);

    // Drive the reader onto the late anchor's band. Collected, it pairs with
    // the follower's copy at 900 px and drags the pane there; inert, nothing
    // straddles the line, `handleScroll` returns early and the follower stays
    // home at 0.
    await setScrollTop(page, SRC, (await offsetTopOf(page, SRC, "x-8888")) + 4);
    await advanceFrames(page, 45);
    expect(await scrollTopOf(page, TGT)).toBeLessThan(50);

    expect(errors).toEqual([]);
  });
```
No import changes: `advanceFrames`, `collectConsole`, `collectPageErrors`, `offsetTopOf`, `scrollTopOf`, `setScrollTop` and `waitForScrollNear` are already imported at the top of this file, and `installRig` / `mount` / `mapOf` / `appendBlock` / `setPaneHeight` are local.

- [ ] **Step 3: Pin the refusal this gate must not kill — sharpen test `b`.** `web/tests/engine.spec.js` test `b` ("a map with no synchronizable row cannot drive anchored panes") is R0002-0047's only proof: over anchored panes, both `blocks: []` and an all-`non-sync` map must return `null`, leave the panes untagged, and say so on the console. It passes today, and **it is the test the naive version of this task turns red** — so it gets one more assertion before anything changes, and it is run green now, so a later red is unambiguously this wave's doing. This is a pin, not a red-first test.

  Inside test `b`, replace:
```js
    expect(
      logs.some((m) => m.includes("describes no synchronizable block"))
    ).toBe(true);
```
  with:
```js
    // The refusal message names what the PANES carry, and that number is the
    // pin. R0002-0047 asks "do these panes carry anchors at all", which is a
    // question about the DOM — so `mountSync` must keep answering it with an
    // ungated read. An anchor count taken from `collectAnchors`' output would
    // be 0 here once the OI-0035 row gate lands, because a map with no
    // synchronizable row lists no ids and the rig's ten anchors are then all
    // unlisted; `anchorCount > 0 && rowCount === 0` would become
    // unsatisfiable and this refusal would quietly turn into a vacuous mount.
    expect(
      logs.some(
        (m) =>
          m.includes("describes no synchronizable block") &&
          m.includes(`the panes carry ${BLOCK_IDS.length * 2} anchors`)
      )
    ).toBe(true);
```
  `BLOCK_IDS.length * 2` is ten — `installRig` puts one anchor per id in **both** panes — and ten is what the shipped engine already prints, so the assertion holds before the change as well as after.

```bash
./scripts/test-browser.sh tests/engine.spec.js -g "b — " > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-pin-b.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-pin-b.txt
```
Read the file separately. Expected: `PW_EXIT=0` and `1 passed`. If it is red **now**, before a single line of `sync.js` has moved, the count in the message is not ten — read the actual warning in the failure output and fix the assertion to match the shipped engine, not the other way round.

- [ ] **Step 4: Run the two new tests and see them fail.**
```bash
./scripts/test-browser.sh tests/engine.spec.js -g "k — |l — " > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-red.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-red.txt
```
Read the file separately. Expected: `PW_EXIT=1` and `2 failed`, with these two failures:
```
  1) [chromium] › tests/engine.spec.js:NNN:3 › sync.js mount contract › k — an anchor no alignment row claims cannot drive the follower

    Error: expect(received).toBe(expected) // Object.is equality

    Expected: true
    Received: false

      at /Volumes/Common/QJoon/transync/web/tests/engine.spec.js:NNN:NN
```
— the mount warning does not exist yet (the `source` annotation is printed beside the matcher) — and
```
  2) [chromium] › tests/engine.spec.js:NNN:3 › sync.js mount contract › l — a duplicate the map does claim keeps its first occurrence, …

    Error: expect(received).toBeLessThan(expected)

    Expected: < 50
    Received: 904
```
`904` is the follower dragged to the target's copy of an anchor the map never mentioned. (If the 45 frames ran out mid-lerp the number is a few pixels short of 904 and prints as a float — either way it is nine-hundred-ish, not under 50.) If `k` instead fails at `waitForScrollNear: #eng-target at 700, expected ~0±4`, that is the same defect one assertion later and is equally valid red.

- [ ] **Step 5: Add the row-id set to `web/js/sync.js`.** Replace the whole `synchronizableRowCount` function and its comment block:
```js
// How many rows of the map claim a DOM anchor. Every `sync_role` other than
// "non-sync" anchors scroll (contracts.md §3), including the ones a
// newer-minor map may add — `validateRows` has already decided that an
// unclassifiable role is either refused or treated as an anchor, so counting
// everything-but-"non-sync" here agrees with what the engine will wire.
function synchronizableRowCount(alignmentMap) {
  const rows = Array.isArray(alignmentMap && alignmentMap.blocks)
    ? alignmentMap.blocks
    : [];
  let count = 0;
  for (const row of rows) {
    if (row && row.sync_role !== "non-sync") count += 1;
  }
  return count;
}
```
with:
```js
// The ids of every row that claims a DOM anchor. Every `sync_role` other than
// "non-sync" anchors scroll (contracts.md §3), including the ones a
// newer-minor map may add — `validateRows` has already decided that an
// unclassifiable role is either refused or treated as an anchor, so taking
// everything-but-"non-sync" here agrees with what the engine will wire.
//
// OI-0035: this set is the engine's anchor authority. `collectAnchors` skips
// any DOM element whose `data-sync-id` is not in it, so a `data-sync-id` that
// rode in on document content — source Markdown is untrusted data, raw HTML is
// translatable content, and DOMPurify's default keeps `data-*` — cannot become
// a scroll driver or a scroll target.
function synchronizableRowIds(alignmentMap) {
  const rows = Array.isArray(alignmentMap && alignmentMap.blocks)
    ? alignmentMap.blocks
    : [];
  const ids = new Set();
  for (const row of rows) {
    if (row && row.sync_role !== "non-sync") ids.add(row.source_block_id);
  }
  return ids;
}

// How many rows claim a DOM anchor — derived from the set above rather than
// counted again, so the predicate that gates the mount and the predicate that
// gates the anchors are one function and cannot drift apart (OI-0035). The two
// numbers agree because `validateRows` refuses a duplicate `source_block_id`
// before either is asked, so rows and distinct ids are the same count for
// every map that reaches here.
function synchronizableRowCount(alignmentMap) {
  return synchronizableRowIds(alignmentMap).size;
}
```

- [ ] **Step 6: Gate `collectAnchors` on the set.** In `web/js/sync.js`, replace the whole function and the tail of its docstring. The docstring's final paragraph:
```js
 * `quiet` suppresses those warnings for the reflow recompute, which re-runs
 * this on every observed reflow: mount is the audit point for a producer
 * defect, and a window-resize drag would otherwise turn one duplicate into
 * a console message per frame.
 */
```
becomes:
```js
 * `quiet` suppresses those warnings for the reflow recompute, which re-runs
 * this on every observed reflow: mount is the audit point for a producer
 * defect, and a window-resize drag would otherwise turn one duplicate into
 * a console message per frame.
 *
 * **`rowIds` is the anchor authority, not the DOM (OI-0035).** Source content
 * can carry a `data-sync-id` — source Markdown is untrusted data
 * (architectural invariant 7), raw HTML blocks are translatable content that
 * reaches the pane, and DOMPurify's default keeps `data-*` attributes — and an
 * anchor the alignment map never claimed used to be indistinguishable from one
 * the renderer emitted. Every element whose id is absent from the validated
 * rows is now skipped, at mount and at every reflow recompute alike, so it is
 * inert forever: an anchor inserted AFTER mount can no longer be folded into
 * the live set by the next quiet recompute. Skips are announced under the same
 * first-five-then-a-tally policy as duplicates, and for the same reason they
 * are silent on reflow.
 *
 * **What this cannot do, stated rather than implied.** An impostor carrying a
 * LISTED id that precedes the genuine anchor in document order still wins,
 * because first-occurrence-wins has no DOM-visible discriminator to prefer one
 * over the other. That residual is why OI-0035 was closed at two layers: the
 * renderer strips the reserved namespace out of raw-HTML blocks
 * (`contracts.md` §4), so panes transync produces never contain one, and this
 * gate makes every UNLISTED id inert in any pane, whoever produced it. The
 * one case neither layer reaches is a listed impostor in a pane transync did
 * not produce — a third-party producer can still hand the engine one, and the
 * engine will drive from it.
 *
 * What this deliberately does NOT govern is the anchor count R0002-0047 asks
 * `mountSync` for. "Do these panes carry anchors at all" is a question about
 * the DOM, answered there by an ungated `querySelectorAll` before this
 * function runs; routing it through this gate would make that refusal
 * unsatisfiable, because the maps it refuses are exactly the maps for which
 * `rowIds` is empty.
 */
```
and the body:
```js
function collectAnchors(pane, label, quiet) {
  const blocks = [];
  const byId = new Map();
  let duplicates = 0;
  for (const el of pane.querySelectorAll("[data-sync-id]")) {
    const id = el.dataset.syncId;
    if (byId.has(id)) {
      duplicates += 1;
      if (!quiet && duplicates <= 5) {
        console.warn(
          `transync: duplicate data-sync-id "${id}" in ${label} pane; keeping the first occurrence and ignoring this one`
        );
      }
      continue;
    }
    byId.set(id, el);
    blocks.push(el);
  }
  if (!quiet && duplicates > 5) {
    console.warn(
      `transync: ${duplicates - 5} more duplicate data-sync-id warnings suppressed (${label} pane)`
    );
  }
  return { blocks, byId };
}
```
becomes:
```js
function collectAnchors(pane, label, rowIds, quiet) {
  const blocks = [];
  const byId = new Map();
  let duplicates = 0;
  let unlisted = 0;
  for (const el of pane.querySelectorAll("[data-sync-id]")) {
    const id = el.dataset.syncId;
    // The map gate runs FIRST, so an unlisted anchor is never reported as a
    // duplicate: the two are different defects with different owners — one is
    // a producer emitting a repeated row, the other is content claiming an
    // anchor it was never given.
    if (!rowIds.has(id)) {
      unlisted += 1;
      if (!quiet && unlisted <= 5) {
        console.warn(
          `transync: ignoring anchor "${id}" in ${label} pane — no alignment row claims it`
        );
      }
      continue;
    }
    if (byId.has(id)) {
      duplicates += 1;
      if (!quiet && duplicates <= 5) {
        console.warn(
          `transync: duplicate data-sync-id "${id}" in ${label} pane; keeping the first occurrence and ignoring this one`
        );
      }
      continue;
    }
    byId.set(id, el);
    blocks.push(el);
  }
  if (!quiet && unlisted > 5) {
    console.warn(
      `transync: ${unlisted - 5} more unlisted-anchor warnings suppressed (${label} pane)`
    );
  }
  if (!quiet && duplicates > 5) {
    console.warn(
      `transync: ${duplicates - 5} more duplicate data-sync-id warnings suppressed (${label} pane)`
    );
  }
  return { blocks, byId };
}
```

- [ ] **Step 7: Thread the set through `mountSync`, and keep R0002-0047 reachable.** This step moves two things at once because they are one edit: the gated collection, and the empty-map refusal that sits directly under it. **Replace the whole run — the two `collectAnchors` calls *and* the R0002-0047 block that follows them:**
```js
  const sourceAnchors = collectAnchors(sourcePane, "source");
  const targetAnchors = collectAnchors(targetPane, "target");

  // R0002-0047: a map whose rows describe no synchronizable block — `blocks:
  // []`, or every row `non-sync` — has no malformed row for the shape gate to
  // name, and would then wire whatever `data-sync-id` anchors the panes
  // happen to carry. That is precisely the "pair whatever the DOM has"
  // degradation the gate above exists to refuse, so it is refused here
  // instead. The refusal is conditioned on the panes because an empty
  // document legitimately mounts an empty map over empty panes: nothing to
  // pair on either side is a vacuous mount, not a mismatched one.
  const anchorCount = sourceAnchors.blocks.length + targetAnchors.blocks.length;
  if (anchorCount > 0 && synchronizableRowCount(alignmentMap) === 0) {
    console.warn(
      `transync: rejecting alignment map — it describes no synchronizable ` +
        `block, but the panes carry ${anchorCount} anchors`
    );
    return null;
  }
```
with:
```js
  // OI-0035: the anchor set comes from the validated rows, never from whatever
  // the DOM happens to carry. Built after `loadAlignment` because it is only
  // meaningful over rows the shape gate has already accepted.
  const rowIds = synchronizableRowIds(alignmentMap);

  // R0002-0047: a map whose rows describe no synchronizable block — `blocks:
  // []`, or every row `non-sync` — has no malformed row for the shape gate to
  // name, and would then wire whatever `data-sync-id` anchors the panes
  // happen to carry. That is precisely the "pair whatever the DOM has"
  // degradation the gate above exists to refuse, so it is refused here
  // instead. The refusal is conditioned on the panes because an empty
  // document legitimately mounts an empty map over empty panes: nothing to
  // pair on either side is a vacuous mount, not a mismatched one.
  //
  // The count is read straight off the DOM, and this refusal runs BEFORE the
  // collection below — both deliberate, and both load-bearing since the
  // OI-0035 gate (ti `490d97` wave 1). Counting collected anchors instead
  // would make the condition unsatisfiable: the maps refused here are exactly
  // the maps whose `rowIds` is empty, a gated collection returns nothing for
  // them, so `anchorCount` would always be 0 and a title-only map over
  // anchored panes would mount vacuously instead of being refused. This read
  // is not a hole in the gate — nothing it sees is retained, paired, driven,
  // or handed to a context; it answers only "do these panes carry anchors at
  // all", which is the question R0002-0047 asks and the one question the
  // validated rows cannot answer.
  const anchorCount =
    sourcePane.querySelectorAll("[data-sync-id]").length +
    targetPane.querySelectorAll("[data-sync-id]").length;
  if (anchorCount > 0 && synchronizableRowCount(alignmentMap) === 0) {
    console.warn(
      `transync: rejecting alignment map — it describes no synchronizable ` +
        `block, but the panes carry ${anchorCount} anchors`
    );
    return null;
  }

  const sourceAnchors = collectAnchors(sourcePane, "source", rowIds);
  const targetAnchors = collectAnchors(targetPane, "target", rowIds);
```
  Three things about that replacement, in the order a reviewer will ask them.

  **Why the refusal moved above the collection rather than just changing its count.** Both work for the condition, but a refused mount should not first warn about every anchor in both panes: under an empty `rowIds` the gated `collectAnchors` would print `ignoring anchor …` five times per pane and then a tally, all for a map that is about to be rejected for a different and more informative reason. Refuse first, collect second.

  **Why the call is still `synchronizableRowCount(alignmentMap)` and not `rowIds.size`.** They are the same number by construction after Step 5, and the function is kept because it is the name spec §8 uses for this predicate and `mountSync` is its caller. Replacing the call would leave it with none.

  **What changed in the number.** The old count deduplicated (it read `blocks.length`); this one counts elements, so a pane with a repeated id now reports the higher figure. The branch is `> 0`, so no decision moves, and the message is now literally what the panes carry. Test `b` asserts ten, which both spellings produce for the rig.

Then, further down, replace:
```js
  const reflow = wireReflowRecompute({
    sourcePane,
    targetPane,
    sourceCtx,
    targetCtx,
    state,
  });
```
with:
```js
  const reflow = wireReflowRecompute({
    sourcePane,
    targetPane,
    sourceCtx,
    targetCtx,
    state,
    rowIds,
  });
```

- [ ] **Step 8: Gate the reflow re-collection on the same set.** In `web/js/sync.js`, replace:
```js
function wireReflowRecompute({ sourcePane, targetPane, sourceCtx, targetCtx, state }) {
```
with:
```js
function wireReflowRecompute({ sourcePane, targetPane, sourceCtx, targetCtx, state, rowIds }) {
```
and inside `recompute`, replace:
```js
    // Quiet re-collection: mount is the audit point for duplicate ids, and
    // a resize drag would otherwise replay the same warning every frame.
    const source = collectAnchors(sourcePane, "source", true);
    const target = collectAnchors(targetPane, "target", true);
```
with:
```js
    // Quiet re-collection: mount is the audit point for duplicate ids, and
    // a resize drag would otherwise replay the same warning every frame. The
    // row gate is NOT quiet in the same sense — it still applies here, and
    // that is what makes an anchor inserted after mount inert forever rather
    // than merely inert until the next reflow (OI-0035).
    const source = collectAnchors(sourcePane, "source", rowIds, true);
    const target = collectAnchors(targetPane, "target", rowIds, true);
```

- [ ] **Step 9: Record the gate in `mountSync`'s docstring — two edits, the opening sentence and a new paragraph.** The docstring's first sentence is the one a skimming reader takes away, and left alone it keeps describing ungated collection after every paragraph below it has been qualified. In `web/js/sync.js`, first replace:
```js
 * Caches each pane's `[data-sync-id]` element list and a partner-side
 * `id → element` map at mount time, so per-frame scroll handling
 * doesn't repeat a `querySelectorAll` + attribute-selector lookup.
```
with:
```js
 * Caches each pane's `[data-sync-id]` element list — the elements whose
 * ids the validated rows claim (OI-0035) — and a partner-side
 * `id → element` map at mount time, so per-frame scroll handling
 * doesn't repeat a `querySelectorAll` + attribute-selector lookup.
```
Then, immediately after the `**Pairing is by identical \`data-sync-id\`, and that is normative …**` paragraph and before `**Refusal is a real outcome, and it is signalled (R0002-0016).**`, insert:
```js
 * **The anchor set comes from the validated rows, not from the DOM
 * (OI-0035).** Every element whose `data-sync-id` is absent from the map's
 * synchronizable rows is skipped — at mount, where the skip is announced, and
 * at every reflow recompute, where it is silent — so a `data-sync-id` carried
 * in by document content is inert forever, including one inserted after
 * mount. The residual this cannot reach is an impostor with a LISTED id
 * placed ahead of the genuine anchor, which first-occurrence-wins cannot tell
 * apart; the renderer closes that one by stripping the reserved namespace out
 * of raw-HTML blocks (`contracts.md` §4).
 *
 * The "no synchronizable row for panes that carry anchors" refusal below is
 * NOT routed through that set. Whether the panes carry anchors is read
 * straight off the DOM, before the anchor sets are collected, because it asks
 * what the panes hold rather than what the engine may drive — and a map with
 * no synchronizable row lists no ids, so a gated count would be zero for
 * exactly the maps that refusal exists to refuse.
 *
```

- [ ] **Step 10: Copy the workspace file over the embedded twin, and prove the bytes.** This is the whole of "how the twin is kept in sync": a literal copy, in the same commit, verified two ways.
```bash
cp /Volumes/Common/QJoon/transync/web/js/sync.js /Volumes/Common/QJoon/transync/crates/transync-cli/web/sync.js
cmp /Volumes/Common/QJoon/transync/web/js/sync.js /Volumes/Common/QJoon/transync/crates/transync-cli/web/sync.js
echo "CMP_EXIT=$?"
cargo test -p transync-cli --test sync_js_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-drift.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-drift.txt
```
Expected: `CMP_EXIT=0` with no output from `cmp`, and in the capture file `CARGO_EXIT=0` with `test result: ok. 5 passed`. A red `sync_js_workspace_and_embedded_are_byte_identical` here means the copy did not happen; re-run the `cp`. **Never** hand-edit the embedded twin to make this pass — the two files then have two edit histories, which is the drift the test exists to catch.

- [ ] **Step 11: Run the two new tests and the pinned refusal, and see all three pass.** The script rebuilds `transync-cli`, so the bundle picks up the twin copied in Step 10. Test `b` rides along deliberately: it is the one that fails if Step 7 was implemented by counting collected anchors, and finding that out here costs one filtered run instead of a full suite.
```bash
./scripts/test-browser.sh tests/engine.spec.js -g "k — |l — |b — " > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-green.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-green.txt
```
Expected: `PW_EXIT=0` and `3 passed`. If `b` is the one that failed — `expect(received).toBe(expected) / Expected: true / Received: false` on the `describes no synchronizable block` assertion, with `ignoring anchor "e-0001"`-style warnings in the captured output instead — the refusal has been silently deleted: `anchorCount` is being computed from `collectAnchors`' output rather than from `querySelectorAll`, or the refusal is still sitting below the collection. Re-read Step 7; do not "fix" test `b`.

- [ ] **Step 12: Run the whole browser suite.** The gate touches every mount in every spec, so the regression surface is the whole suite — `engine.spec.js` `b` (R0002-0047's refusal, which the gate must leave standing: it is answered from an ungated DOM read, above the collection, per Step 7), `engine.spec.js` `h` (a map promising an anchor the DOM gains later — that id *is* listed, so the reflow still activates it), `scn13.spec.js` `k` (the duplicate policy, which now runs second and only over listed ids) and `wasm.spec.js` (which re-mounts against a freshly rebuilt map after every edit) are the four that would notice a gate that took too much.
```bash
./scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-browser.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-browser.txt
```
Expected: `PW_EXIT=0` and the file ending `[test-browser] OK — SCN-13 headless suite passed`.

- [ ] **Step 13: Land the contracts.md sentence and paragraph this commit makes true.** Three edits in `docs/architecture/contracts.md`, all describing behaviour that exists as of *this* commit and none earlier — §4's sentence about what the engine reads, and §3's cross-reference to that same claim, are edited here beside the §4a paragraph they point at, so no commit ever ships a docs claim ahead of its code.

  1. In **§4**, replace the sentence that follows the wrapper-element list:
```markdown
The JS sync engine reads `[data-sync-id]` only. It MUST tolerate unknown `data-block-kind` values for forward compatibility.
```
  with:
```markdown
The JS sync engine reads `[data-sync-id]` only, and only for the ids the alignment map claims (§4a). It MUST tolerate unknown `data-block-kind` values for forward compatibility.
```

  2. In **§4a**, insert a new paragraph immediately **after** the paragraph beginning `**Anchor survival is unconditional (DCR-0017).**` and before the one beginning `**The renderer checks the map instead of trusting it`:
```markdown
**The engine's anchor set comes from the validated rows, not from the DOM (OI-0035, ti `490d97` wave 1).** `mountSync` builds the id set from every row whose `sync_role` is not `non-sync` — derived from the same function that answers whether the map describes a synchronizable block at all, so the two cannot drift apart — and `collectAnchors`, the single choke point both panes pass through at mount **and** on every reflow recompute, skips any element whose `data-sync-id` is not in it. An unlisted anchor is therefore inert forever, including one inserted after mount, which the reflow recompute would otherwise have folded into the live set with the duplicate audit suppressed. Each skip is named at mount under the same first-five-then-a-tally policy the duplicate warning uses, and is silent on reflow for the same reason. Duplicate handling is unchanged: among listed ids, the first occurrence in document order still wins in both the scan array and the partner lookup. The refusal of a map that describes no synchronizable block over panes that carry anchors (§3) is also unchanged, and deliberately asks the DOM rather than the rows: it is a question about what the panes hold, so it is answered by an ungated `querySelectorAll` before the gated collection runs — the gate governs what may *drive scroll*, not what may be counted. **The honest residual:** this gate cannot defeat an in-pane impostor carrying a *listed* id that precedes the genuine anchor in document order — first-occurrence-wins has no DOM-visible discriminator to prefer one over the other. That is why the defense is two layers rather than one: §4's render-side strip guarantees the panes transync produces never contain such an impostor, and this gate makes every *unlisted* id inert in any pane, whoever produced it. Each layer covers the other's blind spot; neither is optional. What the pair still does not reach is that same residual seen from outside — a *listed* impostor in a pane transync did not produce, which a third-party producer can hand the engine and the engine will drive from.
```

  3. In **§3**, the **Forward-minor policy** bullet ends with the same read-claim §4 carried, as a cross-reference — and edit 1 corrected the original while this parenthetical still asserts the unqualified read. It is genuinely about the anchor read: "it reads `[data-sync-id]` only" is the stated *reason* an unknown `block_kind` renders inertly, and the gate strengthens that reason rather than changing it — an engine that reads only the listed `data-sync-id`s still reads no `block_kind`. Left alone it would contradict the §4a paragraph landing two edits up. Replace:
```markdown
(it reads `[data-sync-id]` only — §4)
```
  with:
```markdown
(it reads `[data-sync-id]` only, and only for the ids the alignment map claims — §4, §4a)
```
  `§4` stays in the pointer — it is where the attribute contract lives — and `§4a` joins it for the gate.

- [ ] **Step 14: Verify the workspace and the lints.**
```bash
cargo fmt --all -- --check > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-fmt.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-fmt.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t3-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t3-workspace.txt
```
Expected: `CARGO_EXIT=0` in both. (No Rust changed in this task, so `fmt --check` is the cheap form; the workspace run is what re-proves `sync_js_drift` alongside everything else.)

- [ ] **Step 15: Commit both copies together.**
```bash
git add web/js/sync.js crates/transync-cli/web/sync.js \
  web/tests/engine.spec.js \
  docs/architecture/contracts.md
git commit -m "fix(sync.js): the alignment map decides what can drive scroll

OI-0035's engine half. The engine built its anchor sets from every
[data-sync-id] in each pane, so map membership validated and warned but gated
nothing — a data-sync-id carried in by document content became a scroll driver
the engine could not tell from a real one. mountSync now takes the id set from
the validated rows whose sync_role is not non-sync, and collectAnchors — the
one choke point both panes pass through at mount and on every reflow — skips
what the map does not claim. That also closes the 2026-08-09 correction's
residual: an anchor inserted after mount is no longer activated by the next
quiet recompute.

synchronizableRowCount is now derived from the same set, so the predicate that
gates the mount and the predicate that gates the anchors are one function.
R0002-0047's refusal keeps its own question: whether the panes carry anchors is
read off the DOM, ungated and before the collection, because a map with no
synchronizable row lists no ids and a gated count would be zero for exactly
the maps that refusal exists to refuse.

Recorded rather than glossed: this cannot defeat an impostor carrying a LISTED
id ahead of the genuine anchor — first-occurrence-wins has nothing in the DOM
to tell them apart. The render-side strip is the layer that closes that one,
which is why route (c) is both layers.

Both sync.js copies move here, byte-identical; sync_js_drift.rs is the weld.

TRACE: ti 490d97 wave 1
TRACE: OI-0035
TRACE: contracts.md §3
TRACE: contracts.md §4
TRACE: contracts.md §4a

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Records — DCR-0033, OI-0035 RESOLVED and archived, CHANGELOG, status, phase state

**Files:**
- Create: `docs/project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md`
- Modify: `docs/project/open-issues.md`, `docs/project/open-issues-archive.md`, `docs/index.md`, `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`

**Interfaces:**
- Consumes: everything Tasks 2 and 3 landed, plus the DOMPurify measurement Task 2 Step 5 wrote into the browser suite.
- Produces: nothing code-facing. **No ADR in this wave** — ADR-0025 records the twelve HTML→HTML decisions and lands in wave 2 (spec §12).
- **Additionally consumes wave 0 Task 7's records commit** — DCR-0032, the CHANGELOG entry, `status.md`, `phase-state.yaml` — which Tasks 1–3 did not need. Step 0 is the check.

- [ ] **Step 0: Verify wave 0's records commit (Task 7) has landed.** Tasks 1–3 needed only wave 0's *code* (Task 6). This task edits five documents at anchors wave 0's **Task 7** wrote — the roster tail, the "through DCR-0032" line, the `**Action (HTML→HTML feature, ti \`490d97\`):**` bullet, the `WAVE 0 LANDED` paragraph, the emptied `[Unreleased]` placeholder — and Step 1's take-the-next-number rule is only safe over a roster whose tail is wave 0's. Dispatched between wave 0's Task 6 and its Task 7, Step 1's `tail -3` prints `0029/0030/0031`, "the next number" is then **DCR-0032 — wave 0's reserved number** — and taking it corrupts both waves' records at once; Step 5 executed literally over the same tree would also leave the "Nothing yet —" placeholder standing directly above a real `### Fixed` section. So check both directions of the landing before writing anything:
```bash
ls /Volumes/Common/QJoon/transync/docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md
grep -c 'through DCR-0032' /Volumes/Common/QJoon/transync/docs/project/status.md
```
Expected: the `ls` prints the path — the file exists — and the `grep -c` prints `1` — `status.md`'s Design Track line already reads `DCR-0001 through DCR-0032`. **If either check fails, wave 0's records have not landed: STOP and wait for wave 0's Task 7.** Never take 0032 for this wave's DCR, and never write wave 0's missing records yourself — they are wave 0's to write, with wave 0's content, in wave 0's commit, and a wave-1 agent filling them in leaves one record with two authors and an audit trail no reader can untangle.

- [ ] **Step 1: Confirm the DCR number is free.**
```bash
ls /Volumes/Common/QJoon/transync/docs/project/design-change-records/ | grep -v '\.ko\.md' | sort | tail -3
```
Expected: the last three lines are `DCR-0031-indented-code-normalize-on-translate.md`, `DCR-0032-transync-html-crate-extraction.md` (wave 0's) and nothing higher — so **DCR-0033** is next. If a `DCR-0033-*` already exists, another wave took it: stop, take the next free number, and change every `DCR-0033` in this task to it.

- [ ] **Step 2: Write the DCR.** `docs/project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md`, opening with OKF frontmatter in the house shape:
```markdown
---
type: DCR
title: An anchor is trusted because the alignment map claims it, and because the renderer put it there
description: OI-0035 closed at both layers (route (c)). The Markdown pane's html-block arm strips the reserved sync-attribute namespace out of the block's own bytes before writing ours, and mountSync takes its anchor set from the validated alignment rows so an unlisted data-sync-id is inert forever, including one inserted after mount. The residual neither layer closes alone is recorded rather than glossed.
tags: [change, project-control, DCR-0033]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-20T00:00:00Z
status: stable
---

# DCR-0033: An anchor is trusted because the map claims it, and because we put it there
```
Body sections, each stating a fact this wave actually established:
  - **Date / Source** — 2026-08-20, ticket `490d97`, wave 1 of the eight in `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12. Independent of every wave after it; landed before the HTML pane producer that would have widened the exposure.
  - **Not breaking, and not schema-visible.** No §0 surface item moves; the alignment wire is unchanged; `ALIGNMENT_SCHEMA_VERSION` stays `1.2.0`. What changes is the bytes a pane carries and which of them the engine will act on.
  - **The mechanism, now measured rather than inferred.** Spec §15 item 3 flagged DOMPurify's `ALLOW_DATA_ATTR` default as *inferred*. It was measured in this wave against the vendored build the bundle actually ships (`web/tests/scn13.spec.js`, test `m`): `DOMPurify.sanitize('<div data-sync-id="p-0003">x</div>')` returns the attribute intact. The sanitizer is not, and was never, the layer that closed this.
  - **Route (c), and why not (a) or (b) alone.** (a) sanitize-time stripping of `data-*` would take the renderer's own anchors with it or need a sanitizer config transync does not own in a third-party mount; (b) the engine gate alone leaves the listed-id-ahead-of-the-genuine-anchor case open. Two layers, each covering the other's blind spot.
  - **The render half.** The Markdown path's `BlockKind::Html` **success** arm, the only live route by which source-controlled markup reaches a Markdown pane (panes render with `unsafe_ = false`), now emits `balance_fragment(strip_reserved_sync_attrs(md))`. Six names, case-insensitive, open tags only. The **failure** arm is deliberately untouched: its payload is HTML-escaped, so an impostor attribute there is text. **Pane-only** — `out.md` keeps the author's bytes, because their `data-sync-id` is their content; the namespace is owned only in DOM transync mounts. `strip_reserved_sync_attrs` itself shipped in wave 0 (DCR-0032) with no call site, precisely so this wave would not write it in the old crate and move it a commit later.
  - **The engine half.** `mountSync` builds the id set from rows whose `sync_role !== "non-sync"`; `synchronizableRowCount` is now derived from that set, so the predicate gating the mount and the predicate gating the anchors are one function rather than two that happen to agree. `collectAnchors` — the single choke point for both panes at mount and at every reflow recompute — skips what the set does not contain, warns at mount under the existing first-five-then-a-tally policy, and stays quiet on reflow. **This closes the 2026-08-09 correction's residual**: an anchor entering the DOM after mount used to be folded into the live set by the next quiet recompute with no audit at any point; it is now inert forever. First-occurrence-wins among listed ids is unchanged. So is R0002-0047's refusal, and keeping it took a deliberate decision rather than no change at all: whether the panes carry anchors is a question about the DOM, so it is answered by an ungated `querySelectorAll` and asked *before* the gated collection. Counting collected anchors would have made `anchorCount > 0 && synchronizableRowCount === 0` unsatisfiable — the maps that refusal exists to refuse are exactly the maps whose id set is empty — and a title-only map over anchored panes would have mounted vacuously. The gate governs what may drive scroll, not what may be counted.
  - **The residual, stated plainly.** Neither the gate nor any DOM-visible discriminator can defeat an in-pane impostor carrying a **listed** id that precedes the genuine anchor in document order. The render strip is what makes that case unreachable in panes transync produces; the gate is what makes every **unlisted** id inert in any pane, whoever produced it. A future producer outside this repository can still hand the engine a pane with a listed impostor, and the engine will drive from it. That is the accepted boundary of route (c), also recorded as spec §13 item 8.
  - **Evidence.** A Rust unit at the call site (`html_block_impostor_sync_attributes_never_reach_the_pane`); an end-to-end browser case over a second `--html-out` bundle built from `crates/transync/tests/fixtures/oi-0035-impostor-anchor.md`, whose html block claims the id of the paragraph that follows it, asserting the bundle's `source.html` carries exactly one claimant and the mounted pane exactly one element; two engine-direct browser cases (an unlisted anchor cannot drive the follower and is named once per pane at mount; a listed duplicate still keeps its first occurrence and a post-mount arrival stays inert through the reflow that used to activate it); the R0002-0047 refusal proved intact by the sharpened `engine.spec.js` `b`, which now asserts that the refusal message names the anchors the panes actually carry — the assertion that goes red if the gate is ever wired into that count; the full workspace suite and the full browser suite green; `sync_js_drift.rs` green over two byte-identical copies.
  - **Welds.** `crates/transync-cli/tests/sync_js_drift.rs`, unchanged, is what makes "both copies moved in one commit" checkable — full byte identity by `std::fs::read`, not a structural check. The browser suite's OI-0035 leg lives in `scripts/test-browser.sh` beside the wasm leg, inside the scratch fixture dir only; the six-file bundle contract is untouched. `contracts.md` §4 (the reserved-namespace rule plus the corrected "reads `[data-sync-id]` only" sentence) and §4a (the anchor-set paragraph with the residual) carry the rules.
  - **Handed forward.** The second call site — the HTML pane derivation's strip-then-inject — is **wave 6's**, and spec §8 already fixes its position: strip before injection, so the same "ours are the only ones" construction holds for HTML panes. Nothing in this wave forecloses it, and nothing in wave 6 needs to revisit these two.

- [ ] **Step 3: Resolve OI-0035 in `docs/project/open-issues.md`.** Three edits.

  1. In the OI-0035 entry's header block, replace:
```markdown
- **Decision:** ACCEPT (track — the mechanism is confirmed, but which layer should close it is a design choice)
- **Status:** OPEN
- **Resolution:** —
```
  with:
```markdown
- **Decision:** ACCEPT (track — the mechanism is confirmed, but which layer should close it is a design choice)
- **Status:** RESOLVED (2026-08-20)
- **Resolution:** Route **(c) — both layers**, owner-ratified in the 2026-08-20 HTML→HTML design (spec §8) and landed as ti `490d97` wave 1 (DCR-0033). **Required action 1** is answered by the route itself. **Action 2:** the renderer's Markdown `BlockKind::Html` success arm — the only live route by which source-controlled markup reaches a Markdown pane, since panes render with `unsafe_ = false` — strips the six reserved attribute names (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`, `data-skipped`) case-insensitively from every open tag in the block's own bytes before the wrapper writes ours, and `mountSync` builds its anchor set from the validated rows whose `sync_role` is not `non-sync` while `collectAnchors` — the single choke point for both panes at mount and at every reflow recompute — skips what that set does not contain. An unlisted anchor is inert forever, including one inserted after mount, which closes the 2026-08-09 correction's residual as well. Both `sync.js` copies moved in one commit; `sync_js_drift.rs` is the weld. The strip is **pane-only**: `out.md` keeps the author's bytes. **Action 3:** the browser suite gained an end-to-end case over a second `--html-out` bundle whose fixture's raw HTML claims the id of the paragraph that follows it (`web/tests/scn13.spec.js`), plus two engine-direct cases (`web/tests/engine.spec.js`) and a Rust unit at the render call site. **Honest residual, accepted:** neither layer alone — and no DOM-visible discriminator — defeats an in-pane impostor carrying a *listed* id placed ahead of the genuine anchor, because `collectAnchors` is first-occurrence-wins. The render strip makes that case unreachable in panes transync produces; the engine gate makes every *unlisted* id inert in any pane, whoever produced it. A *listed* impostor in a pane transync did not produce is reached by neither, and that is the accepted boundary of route (c). **Measured while closing this, correcting an inference:** DOMPurify's default really does keep `data-*` attributes, verified against the vendored build the bundle ships.
```

  2. In the same entry's `### Verification` block, replace the three unchecked boxes with:
```markdown
- [x] Code change applied — `crates/transync-syntax/src/render.rs` (render half); `web/js/sync.js` + `crates/transync-cli/web/sync.js` (engine half, one commit, byte-identical)
- [x] Tests pass — workspace suite and browser suite green; `sync_js_drift.rs` green
- [x] No regressions observed — SCN-13, the wasm demo and the engine-direct suite all green with no expectation edits outside the two new cases
```

  3. In the **Open Issues Summary** table at the foot of the file, replace the OI-0035 row:
```markdown
| OI-0035  | Injected `data-sync-id` can pre-claim a real block's anchor | OPEN (2026-08-08) | Low |
```
  with:
```markdown
| OI-0035  | Injected `data-sync-id` can pre-claim a real block's anchor | RESOLVED (2026-08-20) — archived | Low |
```

- [ ] **Step 4: Move the resolved entry to the archive.** The file's own rule is at its top: *"Remove entries once fully resolved; resolved entries with audit value move to `open-issues-archive.md`."* Cut the whole `## OI-0035: …` section from `docs/project/open-issues.md` — from its heading through the `### Related` list's last bullet, plus the `***` separator that follows it — and paste it into `docs/project/open-issues-archive.md` in OI-number order (after OI-0034's entry if one is present, otherwise at the end of the file), preceded by the archive's banner convention and followed by a `***` separator:
```markdown
> Archived 2026-08-20. Reason: resolved at both layers (route (c), DCR-0033) — audit history, and the record of a residual that is accepted rather than fixed.
```
Append one bullet to the entry's `### Related` list while moving it:
```markdown
- DCR-0033 — the closure: the render-side strip, the engine-side row gate, and the residual neither closes alone.
```
Leave `docs/project/open-issues.md`'s Open Issues Summary row in place (Step 3 edit 3): the table is the index a reader scans, and an entry that vanished from both the body and the table reads as one that was never filed.

- [ ] **Step 5: CHANGELOG.** At the **end** of the `## [Unreleased]` block — immediately before the `## [0.4.0] - 2026-08-20` heading — **append this bullet to the existing `### Fixed` section, or create that section if it is absent**, with one blank line on each side. (Corrected 2026-08-22: this step used to say *insert a `### Fixed` section* unconditionally, which was true when it was written and stopped being true when wave 0's Task 8 created one at exactly this anchor. Executed literally now, it produces a **duplicate `### Fixed` heading**. Task 8's own CHANGELOG step carries this conditional; wave 1's did not, and the asymmetry is what made the hazard invisible from either side.):
```markdown
### Fixed

- **A `data-sync-id` written into source content can no longer pre-claim a real block's anchor** (OI-0035, ti `490d97` wave 1, DCR-0033). Closed at both layers. The renderer strips the six reserved sync-attribute names from raw-HTML blocks on the way into a **pane** — `out.md` keeps the author's bytes — and `mountSync` takes its anchor set from the validated alignment rows, so an element the map does not claim is inert forever, including one inserted after mount. Both `web/js/sync.js` and its byte-identical CLI-embedded twin moved in one commit. Recorded residual: an impostor carrying a *listed* id ahead of the genuine anchor still wins the engine's first-occurrence lookup, which is why the render-side strip is not optional.
```

- [ ] **Step 6: `docs/project/status.md` — three edits, all in sections that already exist.**

  1. `## Design Track`, the `- Closed change records:` line. Change `DCR-0001 through DCR-0032` to `DCR-0001 through DCR-0033`, and extend the parenthetical's final item, before its closing `)`, with:
```markdown
, 0033 the 2026-08-20 OI-0035 closure at both layers (ti `490d97` wave 1)
```

  2. `## Open Issues`, the OI-0016 bullet's neighbourhood: the section's cluster bullets are struck through as they close. Insert a new bullet immediately **after** the `- **OI-0016** — Active-block selection scans every block on every scroll frame (revisit behind a profiling gate).` line:
```markdown
- ~~**OI-0035** — an injected `data-sync-id` can pre-claim a real block's anchor.~~ **RESOLVED 2026-08-20** (ti `490d97` wave 1, DCR-0033) via route **(c) — both layers**: the renderer strips the reserved sync-attribute namespace out of raw-HTML blocks on the way into a pane (`out.md` keeps the author's bytes), and `mountSync` builds its anchor set from the validated alignment rows so `collectAnchors` — the one choke point at mount and at every reflow — ignores what the map does not claim. That also closes the 2026-08-09 correction's residual: an anchor inserted after mount is no longer activated by the next quiet recompute. **The residual that remains is accepted and recorded**: an impostor carrying a *listed* id ahead of the genuine anchor still wins first-occurrence-wins, which is exactly why the render-side strip is the other half rather than a nicety. Record archived to `open-issues-archive.md` the same day.
```

  3. `## Immediate Next Actions`, on the wave-0 bullet added by DCR-0032 (the one opening `**Action (HTML→HTML feature, ti \`490d97\`):**`): append at the end of that bullet:
```markdown
 **Wave 1 also LANDED 2026-08-20** (DCR-0033): OI-0035 closed at both layers before the HTML pane producer that would have widened it exists — the Markdown pane's html-block arm strips the reserved sync-attribute namespace, `mountSync` takes its anchor set from the validated alignment rows, both `sync.js` copies moved in one commit, and the residual neither layer closes alone is written into the DCR, `contracts.md` §4a and the archived issue rather than glossed. **Waves 2–7 remain unstarted.**
```

- [ ] **Step 7: `docs/project/phase-state.yaml` — three edits.** There is no per-wave entry list and no per-entry `status:` key; the two section-level `status:` keys (`design.status`, `implementation.status`) do **not** move — wave 1 changes no phase. `design.open_change_records: []` stays `[]`: the record lands closed, in the same commit as the work it describes.

  1. `project.last_updated:` — `2026-08-20-ti490d97-wave0` → `2026-08-20-ti490d97-wave1`.
  2. `design.closed_change_records:` — append one item, keeping the four-space list indentation, immediately after `    - DCR-0032-transync-html-crate-extraction`:
```yaml
    - DCR-0033-oi0035-anchor-trust-both-layers
```
  3. `project.notes:` is a `|` literal block of unformatted prose — no backticks, no markdown, `->` for arrows. Append this paragraph immediately after the `TI 490d97 WAVE 0 LANDED 2026-08-20` paragraph, at the block's existing four-space indent:
```yaml
    TI 490d97 WAVE 1 LANDED 2026-08-20 (DCR-0033): OI-0035 closed at BOTH
    layers, route (c). The Markdown pane's html-block success arm strips the
    six reserved sync-attribute names out of the block's own bytes before the
    wrapper writes ours - pane-only, out.md keeps the author's bytes - and
    mountSync builds its anchor set from the validated alignment rows so
    collectAnchors, the one choke point at mount and at every reflow, ignores
    an element the map does not claim. An anchor inserted after mount is inert
    forever, which also closes the 2026-08-09 correction's residual. Both
    sync.js copies moved in one commit; sync_js_drift.rs is the weld. Measured
    while closing it, correcting an inference the design carried: DOMPurify's
    default really does keep data-* attributes. THE RESIDUAL IS ACCEPTED AND
    RECORDED: an impostor carrying a LISTED id ahead of the genuine anchor
    still wins first-occurrence-wins, which is why the render strip is the
    other half rather than a nicety. Nothing schema-visible moved; alignment
    stays 1.2.0. Waves 2-7 are UNSTARTED.
```

- [ ] **Step 8: `docs/index.md` — link the DCR.** In the record list, immediately after the `DCR-0032` line wave 0 added:
```markdown
- [DCR-0033 — An anchor is trusted because the map claims it, and because we put it there: OI-0035 closed at both layers (ti 490d97 wave 1)](project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md)
```

- [ ] **Step 9: Verify — the docs-drift welds are the gate here.**
```bash
cargo test -p transync --test docs_index_drift --test docs_ownership_drift --test docs_gate_claims_drift --test docs_browser_suite_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t4-docs.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t4-docs.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave1/gate/t4-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t4-workspace.txt
```
Read both files separately. Expected: `CARGO_EXIT=0` in both. A red `docs_index_drift` means the DCR is unlinked — link it rather than excluding it. A red `docs_browser_suite_drift` means a record above published a test count near a mention of the suite; name the spec files instead of counting them.

- [ ] **Step 10: Run the aggregate hard gate once, then commit.**
```bash
./scripts/smoke.sh > /Volumes/Temp/claude/ti490d97-wave1/gate/t4-smoke.txt 2>&1
echo "SMOKE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave1/gate/t4-smoke.txt
```
Read the file separately. Expected: `SMOKE_EXIT=0`. (`smoke.sh` covers the build, the two-package wasm gate, the workspace suite, the CLI stub suite and the rustdoc gate; it does **not** run the browser suite — Task 3 Step 12 is that evidence.)
```bash
git add docs/project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md \
  docs/project/open-issues.md docs/project/open-issues-archive.md \
  docs/index.md CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml
git commit -m "docs: OI-0035 is closed, and the record says what closing it does not cover

DCR-0033 carries route (c) — the render-side strip, the engine-side row gate,
and the reason neither is optional. The issue moves to the archive with its
verification boxes checked and its residual stated in the same breath as its
resolution: an impostor carrying a LISTED id ahead of the genuine anchor still
wins first-occurrence-wins, and no DOM-visible discriminator exists.

It also corrects an inference the design carried: DOMPurify's ALLOW_DATA_ATTR
default was measured this wave, against the vendored build the bundle ships.

TRACE: ti 490d97 wave 1
TRACE: DCR-0033
TRACE: OI-0035

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Wave acceptance — check all five before declaring wave 1 done

- [ ] `crates/transync-cli/tests/sync_js_drift.rs` green, and `cmp web/js/sync.js crates/transync-cli/web/sync.js` silent — the two copies are byte-identical and moved in **one** commit (`git log --oneline -1 -- crates/transync-cli/web/sync.js` and `git log --oneline -1 -- web/js/sync.js` name the same commit).
- [ ] The two browser cases green: `web/tests/engine.spec.js` `k` + `l` (engine-direct) and `web/tests/scn13.spec.js` `m` (end-to-end). The whole browser suite green, captured bare-to-file — **including `engine.spec.js` `b`**, the R0002-0047 refusal this wave can silently delete: it must still return `null` over anchored panes and still name the anchors those panes carry (Task 3, Steps 3 and 7).
- [ ] The bundle's `source.html` provably free of an impostor attribute: `grep -c 'data-sync-id="p-0003"' /Volumes/Temp/claude/transync-browser-fixture/html/oi0035/source.html` prints `1`, over a bundle whose fixture wrote that attribute into its raw HTML.
- [ ] The residual is written down in all four places it belongs: DCR-0033, `contracts.md` §4a, the archived OI-0035 resolution, and `collectAnchors`' docstring in both `sync.js` copies.
- [ ] The corpus did not move: `git diff --stat <baseline-commit>..HEAD -- 'crates/*/tests/fixtures/*' 'crates/*/tests/scenarios/*'` names **only** the added `oi-0035-impostor-anchor.md`. (`<baseline-commit>` is in `/Volumes/Temp/claude/ti490d97-wave1/gate/baseline-commit.txt`.)

  > **Pathspec corrected 2026-08-23 (ti `490d97` wave 1).** Both globs gained a trailing `/*`. Without it the pathspec matches **nothing**: git runs the pattern against the whole path and `*` matches `/`, but the pattern ended at `fixtures`, so only a path *ending* there could match — and every corpus file is one level deeper. Measured: `git ls-files -- 'crates/*/tests/fixtures'` prints 0 files, `git ls-files -- 'crates/*/tests/fixtures/*'` prints 16. The wave's claim survives re-checking with the corrected glob: the range names exactly the one added file, which is what this item requires. The verdict was right; the instrument was not measuring. The lesson is not "test your gates" but the narrower one: **a filter that matches nothing is indistinguishable from a filter whose subject is clean** — both print silence — so a gate built on a pathspec, a grep pattern or a test-name filter needs a companion assertion that the selector selects something.


---

## Notes for the implementer

- **The single most expensive mistake available in this wave is editing `web/js/sync.js` and running the browser suite without copying to `crates/transync-cli/web/sync.js` first.** `engine.spec.js` imports `/sync.js` — the bundle-flat file, which the CLI binary `include_str!`s from the *embedded* twin — so the suite will keep exercising the old engine and the new tests will keep failing for a reason that has nothing to do with the code you wrote. Only `demo-wasm.html` (`wasm.spec.js`) loads the workspace copy. Copy, then run.
- **The JS red-first cycle is not Rust's.** There is no `cargo test` for `sync.js`; the evidence is Playwright, through `scripts/test-browser.sh`, which rebuilds the stub CLI, regenerates both bundles and rebuilds the wasm module on every invocation. That is slow but self-contained, and its `"$@"` passthrough means `./scripts/test-browser.sh tests/engine.spec.js -g "k — "` narrows the run without bypassing the regeneration. If you need a faster inner loop after a full run has already produced the fixture, `cd web && TRANSYNC_FIXTURE_DIR=/Volumes/Temp/claude/transync-browser-fixture/html pnpm exec playwright test tests/engine.spec.js -g "k — "` re-runs against the existing bundle with the Node stand-in server (documented in `playwright.config.js`) — but that bundle carries whatever `sync.js` the last full run embedded, so any inner-loop pass must be re-confirmed by a full `scripts/test-browser.sh` before the commit.
- **A failing Playwright assertion looks nothing like a Rust panic.** Under the `list` reporter you get a numbered block naming the project, the file, the line, the describe title and the test title, then `Error: expect(received).toBe(expected)` with `Expected:` / `Received:` lines, then a stack; the run ends with `N failed`. The harness's own `waitForScrollNear` rejects with a *message* instead — `waitForScrollNear: #eng-target at 700, expected ~0±4` — which is the more useful red here because it names the wrong place the follower went.
- **`strip_reserved_sync_attrs` is wave 0's.** It ships tested and unused; wave 1 is its first caller. If a test in this wave suggests the function should behave differently, that is a wave-0 conversation (and a DCR-0032 amendment), not an edit to make here.
- **The row gate has one neighbour it can delete without failing to compile: R0002-0047.** `mountSync` refuses a map that describes no synchronizable block *when the panes carry anchors*, and the maps it refuses are exactly the maps whose row-id set is empty — so an anchor count taken from the gated collection is 0 for every one of them and the condition can never fire again. The count therefore stays an ungated `querySelectorAll` and the refusal moves above the collection (Task 3, Step 7); `engine.spec.js` `b` is what proves it, and Task 3 Step 3 sharpens `b` so it says *why* it went red rather than just that it did.
- **Do not "fix" the residual.** The listed-id-ahead-of-the-genuine-anchor case is deliberately left open at the engine layer and deliberately closed at the render layer, and the whole point of route (c) is that neither layer pretends to do the other's job. A change that makes `collectAnchors` prefer, say, the *last* occurrence, or the one whose `data-order` matches the row, is a new design decision — file it, do not land it.
- **The end-to-end bundle is scratch.** `$HTML_OUT/oi0035/` exists only inside the Playwright fixture directory, exactly like the wasm demo leg above it. Nothing about the six-file `--html-out` bundle contract, `HTML_BUNDLE_ENTRIES`, or `transync serve`'s surface changes.
