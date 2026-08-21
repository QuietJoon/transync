# HTML→HTML Wave 3 — HTML Intake + Identity Round-Trip Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An HTML document goes in and comes out **byte-identical**, with a real block set, real ids and a real alignment map — no LLM, no pipeline change, no core change.

**Architecture:** A new module `transync-syntax::intake::html` — the structural half the `transync-html` crate-doc already promises ("structural intake … lives in `transync_syntax::intake::html`, not here"). It consumes exactly two things from `transync-html`, both landed in wave 0: `element_extents()` (the one pairing opinion, precomputed over the whole input) and `scan_tags()`'s token stream (`Open`/`Close`/`Skip`, all with spans). One linear pass over the tokens classifies every element into spec §4's five classes — default STOP — pushes leaf blocks whose ranges are **whole elements, tags included**, accumulates anonymous text runs under **rule T**, and stamps ids, spellings, section scopes and `ast_path`s in emission order. Identity is then a *theorem, not a test hope*: `regen::regenerate` copies inter-block gaps verbatim and splices `source_text[range]` for every block absent from the accepted map, so `regenerate(parse, &HashMap::new())` is byte-identical to the input **exactly when** the intake's ranges are in source order, non-overlapping and in bounds — the three debug-asserted invariants this wave also ships. The spine test is written first and goes red **twice** — once as a compile red (the missing module), then again as a behavioural red against a deliberately rule-T-less walk, where byte-identity is already green and the kind-sequence assertions fail with real diffs; everything else refines it. Every identity assertion is paired with a block-set assertion in the same test, because byte-identity alone is satisfied by a parse that finds nothing.

**Tech Stack:** Rust 2024 workspace (rustc ≥ 1.88), `transync-html` (wave 0 complete), the wave-2 IR (`Spelling`, `SourceFormat`, `Block.spelling`, `Document.format`, `BlockKind::{Html, Title}`), `wasm32-unknown-unknown` check gate, tracked pre-commit hook (fmt + clippy + wasm gate + rustdoc gate).

**Spec:** docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md

**Depends on:** **wave 0, complete** (`docs/superpowers/plans/2026-08-20-html-wave0-transync-html-crate.md` — this plan consumes `element_extents`, `TagToken::Open.span`, `TagToken::Skip`, `is_void`/`is_raw_text`/`implicitly_closes`) **and wave 2, complete** (`docs/superpowers/plans/2026-08-20-html-wave2-ir-split.md` — this plan constructs `Block { spelling: Spelling::Html { block_type: None } }`, `Document { format: SourceFormat::Html }`, unit-variant `BlockKind::Html`, and `BlockKind::Title`, and leans on the `Title => SyncRole::NonSync` arm). Task 1 Step 2 is a hard gate on **both**. The spec's dependency graph is `0 → 2 → {3, 4} → 5 → 6 → 7`.

**Overlap with wave 4, which runs in parallel:** wave 4 owns `transync-core::validate` (the layer-6 twin `full_rescan_html`) and the one format branch in `pipeline::finalize`. This plan touches **no file in `transync-core` at all** — the "no core change" half of the wave's one-sentence claim is also what makes the parallelism safe. The two waves' only shared artifact is a DCR *number*: whichever lands second takes the next free number (Task 6 Step 1 checks). Wave 4 may hand-build `Document`s with `format == SourceFormat::Html` before this wave lands; nothing here invalidates them.

**What this wave must NOT do:** make an HTML translation run possible. Spec §12's hard rule: *no HTML translation run ships before the twin exists* (wave 4), and the operator surface is wave 6. This plan builds `Document`s with `format == Html` and proves identity through `regen` — it does not touch `translate()`, `TranslateOptions`, any refusal, any prompt, or `html_dominance_warning` (its "HTML-to-HTML translation is not implemented (ti 490d97)" tail stays TRUE after this wave — intake exists, translation still does not; spec §6 re-texts it in the wave the *entry point* lands). Task 8 of nothing: the acceptance section re-checks this with a diff.

## Global Constraints

- **The wasm gate is a live risk here, not a formality.** The new module lands **inside `transync-syntax`**, one of the two packages the standing gate builds: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown` — string unchanged, run in every task's verify step. The module may use only `std`, `transync-html` and in-crate items (all already wasm32-clean); **no tokio, no tiktoken, no getrandom-reaching edge**.
- `transync-syntax` may gain **no `[features]` table** and **no `transync-core` dependency, dev-dependencies included** — either breaks the gate welded into `scripts/hooks/pre-commit` and `scripts/smoke.sh`. This wave adds **no dependency edge anywhere**: no `Cargo.toml` is touched.
- Every test run: `cargo test -p <crate> -- --test-threads=4`; workspace runs: `cargo test --workspace -- --test-threads=4`. **Never raise the cap.**
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append the exit code, and inspect the file as a *separate* step. A pipeline reports the last stage's status, so a failing suite reads as a pass, and `tail -N` over filtered lines drops early failures.
- Lint gates (the pre-commit hook enforces them): `cargo fmt --all -- --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
- **`git commit --no-verify` is never used.** A commit step's expected result is that the hook runs fmt, clippy, the wasm gate and the rustdoc gate, and all four pass. If the hook blocks, fix the cause.
- Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave3/` — never `/tmp`, never `/private/tmp`, never `$TMPDIR`, never the OS default. If `/Volumes/Temp/claude` is unreachable, stop and ask.
- **NEVER change or override `CARGO_TARGET_DIR`**; never pass `--target-dir`. If a cargo command fails because the target dir is unreachable, stop and ask.
- **Zero edits to existing fixtures and existing expectations.** This wave *adds* fixtures (three new files); it must not change one byte of any existing fixture or any expected value in an existing assertion. The Markdown corpus staying byte-identical is §11's standing review gate, and this wave's only edits to shipped files are two visibility widenings (`parser::normalize_source`, `parser::sections`) and one `pub mod` line, none of which any existing test can observe.
- **A new document under `docs/` turns `docs_index_drift` red the moment it exists** — the test walks the filesystem and requires every `.md` under `docs/` to be linked from `docs/index.md`. **The link lands in the same change that creates the file.** This wave creates two documents (this plan, DCR-0035): the plan's link is Task 1's, the DCR's is Task 6's, each in the commit that creates the file. New *fixture* directories are safe — checked in Task 2's interface notes: `docs_index_drift` walks only `docs/`, `error_fixtures.rs` walks only its own `error-*.md` samples, `workspace_publication.rs` walks only `crates/*/Cargo.toml` — no weld walks `tests/fixtures/`.
- **`docs_ownership_drift` welds `transync-syntax/src/lib.rs`'s module list to `docs/architecture/source-of-truth-table.md`.** Adding `pub mod intake;` turns it red until the table names `` `intake` `` — so the module declaration and the table edit are ONE commit (Task 2).
- No pure-formatting edits. Where this plan shows wrapped Rust, run `cargo fmt --all` afterwards and take the formatter's answer.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, cite, or create them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `crates/transync-syntax/src/intake.rs` | **create** | the format seam's parent module: names `html` as the HTML intake, `parser` as the Markdown intake, and records the deferred `intake::markdown` rename |
| `crates/transync-syntax/src/intake/html.rs` | **create** | THE HTML document intake: the five-class classification table (spec §4, transcribed exactly), the one token walk over `element_extents` + `scan_tags`, rule T, ids/hashes/sections/`ast_path`, `<title>`, warnings, the debug-asserted invariants |
| `crates/transync-syntax/src/lib.rs` | modify | `pub mod intake;` + one crate-doc clause |
| `crates/transync-syntax/src/parser.rs` | modify | `normalize_source` → `pub(crate)`; `mod sections;` → `pub(crate) mod sections;` — both intakes must mean the same string by "source bytes" and share THE heading-scope stack |
| `crates/transync-syntax/src/parser/sections.rs` | modify | `pub(super)` → `pub(crate)` on `SectionStack` and its three methods (the one-home rule: the HTML intake reuses it, never re-implements it) |
| `crates/transync-syntax/tests/html_intake_identity.rs` | **create** | the spine: byte-identity + block-set pins over the SCN-16 fixture, the corpus, and the inline edge documents |
| `crates/transync-syntax/tests/html_intake_alignment.rs` | **create** | the "real ids and a real alignment map" leg: `build_alignment_map` over a parsed page, the `title` row's `non-sync` role, `document_id`, `assign_block_ids` idempotence, `normalize_top_level` prefix collapse |
| `crates/transync/tests/fixtures/scn-16-html-document.html` | **create** | the spec-mandated fixture (§11 row 1: doctype, head+title, nested sections, script/style, entities, unclosed fragment); wave 5's scenario test reuses this same file |
| `crates/transync-syntax/tests/fixtures/html-corpus/marketing-page.html` | **create** | the messy real-world page: uppercase tags, unquoted attributes, implicit `<p>` closes, imgs inside an unclosed paragraph, JSON-LD script, noscript/iframe/dl/legend default-stops, `&nbsp;`-entity run |
| `crates/transync-syntax/tests/fixtures/html-corpus/docs-fragment.html` | **create** | a body fragment: no doctype/head, implicit `<li>` closes, `dt`/`dd`, unclosed `<div><p>` at EOF — force-close with warnings, never a panic |
| `crates/transync/tests/public_surface.rs` | modify | `forbidden` array += `"intake"` — contracts §0 must never document an `intake::` path |
| `docs/architecture/source-of-truth-table.md` | modify | the module enumeration gains `intake` (the `docs_ownership_drift` weld) + one new ownership row for element classification |
| `docs/implementation/module-map.md` | modify | two tree rows for `intake.rs` / `intake/html.rs` |
| `docs/index.md` | modify (Tasks 1, 6) | this plan's link (Task 1); DCR-0035's link (Task 6) |
| `docs/project/design-change-records/DCR-0035-html-intake-identity-round-trip.md` | **create** | the wave's record, including the deferred-`parser`-rename note |
| `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml` | modify | routine per-wave records |
| `CLAUDE.md` | modify | the `transync-syntax` module-split list gains the `intake` rows |

**Not touched, deliberately:** anything under `crates/transync-core/` (wave 4 owns `validate`/`finalize`; wave 5 owns `unit`/`llm`; the wave's claim is "no core change"); `crates/transync-html/` (wave 0 owns the mechanics — the intake only *calls* them); `crates/transync-syntax/src/{regen,align,walk,outcome,render,id}.rs` (consumed, never modified — identity must come from the intake's ranges meeting `regenerate`'s existing contract, not from bending `regenerate`); `web/` (waves 1, 6, 7); every `Cargo.toml` and `Cargo.lock` (no dependency moves, no version moves); `docs/architecture/contracts.md` (no §0 row: `intake` is engine-tier and the facade re-exports nothing from it — the *forbidden* pin is the guard); `docs/architecture/scenario-matrix.md` (the SCN-16 row is wave 7's).

---

## Deviations from the spec

Six, declared here and nowhere else. Five are scope calls with a mechanical reason; deviation 5 is different in kind — it amends the spec's literal wording toward the spec's own stated intent, and the spec-side amendment is recorded as **owed** (DCR-0035 carries it as a §14-style post-implementation review item; the spec file itself is not edited in this wave).

1. **`parser` is NOT renamed to `intake::markdown` in this wave — the new module lands beside it, not instead of it.** Spec §4 sketches `intake::html`, sibling to `intake::markdown` "(today's `parser`, renamed as part of the seam — path details are the implementation plan's, not this spec's)" — the parenthetical delegates exactly this decision. **Why deferred:** the rename's measured blast radius is ~43 `crate::parser` references across 11 files in `transync-syntax`, ~74 `crate::parser`/`transync_syntax::parser` references across 18 files in `transync-core` (including the crate-private re-export `pub(crate) use transync_syntax::parser;` that keeps `crate::parser::…` resolving inside core), `transync-wasm`'s `use transync_syntax::{outcome, parser, regen, render, walk};` plus its doc paths, `public_surface.rs`'s hidden-path pin naming `"parser"`, the module map, the source-of-truth table's enumeration and rows, and CLAUDE.md's layout list — 30+ files for zero behaviour, in the same season wave 2 already rewrote many of those files. A wave whose deliverable is *new* behaviour must keep its diff readable as new behaviour. **What keeps the seam honest without it:** the `transync-html` crate-doc's promise names `transync_syntax::intake::html` — the module this wave creates at exactly that path — and `intake.rs`'s module doc + DCR-0035 record the Markdown half's rename as deferred, with the blast radius, so the asymmetry is a written decision rather than an accident. One wrinkle recorded in the same doc: `parser::intake` (the NUL/nesting guard *function*, OI-0034) predates this module and shares the word; the module doc disambiguates.
2. **`intake::html::parse(source: &str) -> Document` is infallible — no `Result`.** `scan_tags` and `element_extents` are total functions; malformed input force-closes with warnings by design (spec §4), and NUL is normalized, not refused. An unreachable `Err` variant would be untested code claiming to be a gate — the same rule wave 2's deviation 3 applied to the `Document.format` refusal. Spec §9's exit-2 `InputReadFailure` clause binds at the CLI's fallible boundary (file IO, wave 6), not here. If a genuine intake refusal ever materializes, the seam grows a `Result` then, with a reachable test.
3. **Head mode ends at the earlier of the `<head>` element's end and the first `<body>` open tag.** The pairing discipline (`implicitly_closes`) has no head-closes-at-body rule, so on a real page that omits `</head>` the head extent runs to EOF — and a mode keyed on the extent alone would dissolve the **entire body** into head-mode gap. The escape is classification-level (a *mode* boundary), touches no `transync-html` code, and creates no second pairing opinion: the extents still say what they said. Pinned by `a_missing_head_close_does_not_swallow_the_body`.
4. **A pass-through container force-closed mid-document does not warn; leaves and EOF do.** Spec §4 names three warning cases — implicit closes, force-closes on unclosed *leaves*, EOF force-close — and this plan implements exactly those: every emitted leaf whose extent has no close span warns (two message shapes: mid-document vs end-of-input), and every container still open at EOF warns — **delivered by `pop_closed`'s EOF-retention rule** (Task 2 Step 7: a never-explicitly-closed container whose extent runs to `source.len()` is left on the stack for the EOF loop, even when the document's last bytes are a close tag). A `<div>` force-closed by its ancestor's `</section>` mid-document stays silent: its own bytes are all gap, nothing a reader could misread — and by the extent contract its `content_end` sits *before* `source.len()`, which is exactly what lets `pop_closed` distinguish it from a container that is genuinely still open at EOF.
5. **Rule T's img exception reads "textless" after excluding absorbed phrasing markup — a declared deviation from the spec's literal wording, resolved toward the spec's own stated intent.** Spec §4's letter ("a textless run whose only non-whitespace content is one or more `img` elements") would classify `<a href="…"><img …></a>` as gap: the `<a>`/`</a>` tag bytes are non-whitespace outside the `img` spans. That reading silently strips the sync anchor *and* the alignment row from every linked badge, logo and thumbnail — the dominant image idiom on exactly the corpus-class pages this wave covers. But §4 states the exception's rationale itself — *"so images keep a sync anchor instead of dissolving into gap"* — and defines PHRASING as "absorbed into text runs, **never a boundary**"; a linked image is the case that rationale most obviously covers. The consistent reading, implemented here: a textless run (no naked byte outside the run's absorbed-tag and `Skip` spans) that contains **one or more `img` elements** becomes ONE `Image` block — `<a><img></a>`, `<picture><img></picture>`, `<span><img></span>`, and §11's two-adjacent-img case (still ONE block) all keep their anchor; a run with actual text stays a `Paragraph`; a genuinely empty run (no `img` at all) stays gap. Pinned by `a_linked_image_keeps_its_anchor_as_one_image_block` and `an_img_with_real_text_is_still_a_paragraph` (Task 3 Step 1). The spec sentence should be amended to match: recorded as owed in DCR-0035's post-implementation review item — the spec file is NOT edited in this wave.
6. **Phrasing elements hold their whole extent inside the run.** `Hello <svg><title>chart</title></svg> there` must be ONE paragraph and that `<title>` must never be `Title` (spec §4). A name-by-name walk would instead stop at the inner `title`; so when a phrasing element with an extent opens, every token before that extent's end is absorbed into the run — the extents' own verdict, positionally applied. Consequence, accepted and pinned: markup mis-nested *inside* a phrasing element (a `<div>` inside a `<span>`) is absorbed too, deterministically, rather than shattering the run.

---

### Task 1: Preconditions, baseline, and this plan's index link

**Files:**
- Modify: `docs/index.md`
- Commit (already on disk, untracked): `docs/superpowers/plans/2026-08-20-html-wave3-intake-round-trip.md`
- Test: none. This task's product is a recorded green "before" and two verified preconditions.

**Interfaces:**
- Consumes from wave 0: `transync_html::{element_extents, ElementExtent, scan_tags, TagToken::{Open, Close, Skip}, is_void, is_raw_text, implicitly_closes}`.
- Consumes from wave 2: `transync_syntax::id::{Spelling, SourceFormat, BlockKind::Title}`, `Block.spelling`, `Document.format`, `align`'s `Title => SyncRole::NonSync` arm.
- Produces: `/Volumes/Temp/claude/ti490d97-wave3/gate/baseline-commit.txt`, the commit the acceptance section diffs against.

- [ ] **Step 1: Create the wave's temp directory.**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave3/gate
```
Expected: no output, exit 0. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user** — do not fall back to `/tmp`.

- [ ] **Step 2: Hard precondition gate — waves 0 AND 2 must be COMPLETE.** Wave 0's scanner surface and wave 2's IR are both load-bearing for every code block below; writing this wave against the old shapes would make the whole plan wrong.
```bash
grep -c 'pub fn element_extents' crates/transync-html/src/lib.rs
grep -c 'Skip { span' crates/transync-html/src/lib.rs
grep -c 'pub struct ElementExtent' crates/transync-html/src/lib.rs
ls docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md
grep -c 'pub enum Spelling' crates/transync-syntax/src/id.rs
grep -c 'pub enum SourceFormat' crates/transync-syntax/src/id.rs
grep -c 'Title' crates/transync-syntax/src/id.rs
grep -c 'pub spelling: Spelling' crates/transync-syntax/src/parser.rs
grep -c 'pub format: SourceFormat' crates/transync-syntax/src/parser.rs
grep -c 'Title => SyncRole::NonSync' crates/transync-syntax/src/align.rs
grep -c '0.5.0-dev' Cargo.toml
```
Expected: every count **≥ 1** and the DCR path echoed. **Any `0`, or a missing DCR-0032, means a prerequisite wave is unfinished — STOP.** Reading notes: `grep -c` exits non-zero on a zero count, so do not run these under `set -e`; counts above 1 are fine (doc comments naming a symbol are not defects — absence is). Do not proceed on a partial wave 2: this plan constructs `Block` literals carrying `spelling:` and `Document` literals carrying `format:`, which do not compile before wave 2 Task 3.

- [ ] **Step 3: Capture the baseline, bare-to-file.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-cli.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-wasm.txt
```
Then, as a **separate** step, read the last line of each file. Expected: `CARGO_EXIT=0` in all three and no line containing `FAILED`. If any is non-zero, STOP — the tree was not green before this wave. If the failure is `docs_index_drift`, link what its file list names before continuing.

- [ ] **Step 4: Verify this plan's link in `docs/index.md`.** The planning agent that wrote this file was not permitted to touch `docs/index.md`, so unless a controller added it, the link is missing and the baseline above already went red on `docs_index_drift` naming this very file. Add the line below immediately **after** the wave 2 line (the one beginning `- [HTML→HTML Wave 2 — the IR split — implementation plan (2026-08-20)]`); if it is already present, confirm it matches word for word:
```markdown
- [HTML→HTML Wave 3 — HTML intake + identity round-trip — implementation plan (2026-08-20)](superpowers/plans/2026-08-20-html-wave3-intake-round-trip.md) — the 6-task plan for the first demonstrable milestone: `transync-syntax::intake::html` lands beside `parser` (the seam kept, the rename deferred and recorded), spec §4's five-class classification and rule T build a real block set with real ids, sections and ast_paths over `element_extents` + the token stream, and `regenerate(parse, &empty)` is byte-identical over the SCN-16 fixture and a real-page corpus. No LLM, no pipeline change, no core change — wave 4's layer-6 twin stays the gate before any translation run.
```

- [ ] **Step 5: Record the baseline commit — BEFORE the commit below, so the acceptance diff covers this wave and nothing else.**
```bash
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-commit.txt
cat /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-commit.txt
```
Expected: one 40-character SHA.

- [ ] **Step 6: Verify the index weld, then commit the plan and its link together.**
```bash
cargo test -p transync --test docs_index_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t1-index.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t1-index.txt
```
Expected: `CARGO_EXIT=0`. A red here names an unlinked document; link it.
```bash
git add docs/superpowers/plans/2026-08-20-html-wave3-intake-round-trip.md docs/index.md
git commit -m "docs(plan): wave 3 gets its plan, and the index gets its link in the same commit

docs_index_drift walks the filesystem under docs/ and requires every .md it
finds to be linked from docs/index.md, so writing the file is itself what
turns the weld red. File and link are one change.

TRACE: ti 490d97 wave 3

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 2: The spine — the fixtures, the red identity test, the intake module, and its welds

**Files:**
- Create: `crates/transync/tests/fixtures/scn-16-html-document.html`, `crates/transync-syntax/tests/fixtures/html-corpus/marketing-page.html`, `crates/transync-syntax/tests/fixtures/html-corpus/docs-fragment.html`, `crates/transync-syntax/tests/html_intake_identity.rs`, `crates/transync-syntax/src/intake.rs`, `crates/transync-syntax/src/intake/html.rs`
- Modify: `crates/transync-syntax/src/lib.rs`, `crates/transync-syntax/src/parser.rs`, `crates/transync-syntax/src/parser/sections.rs`, `crates/transync/tests/public_surface.rs`, `docs/architecture/source-of-truth-table.md`, `docs/implementation/module-map.md`

**Interfaces:**
- Consumes: `transync_html::{element_extents, ElementExtent, scan_tags, TagToken}`; `crate::parser::{Block, Document, AstPath, normalize_source}` (widened), `crate::parser::sections::SectionStack` (widened), `crate::parser::ranges::ByteRange`; `crate::id::{BlockId, BlockKind, Spelling, SourceFormat, source_hash_bytes}`.
- Produces, for waves 4/5/6 and for this wave's own Tasks 3–5:
  ```rust
  // crates/transync-syntax/src/intake/html.rs
  pub fn parse(source: &str) -> Document;                      // infallible (deviation 2)
  pub(crate) fn debug_assert_block_invariants(doc: &Document); // the spec §4 tripwire
  ```
- **The identity and the block set are asserted as a pair, by design (never identity alone):** `regen::regenerate` over a zero-block document reproduces the whole source as one verbatim gap, so a byte-identity assertion alone is satisfied by a stub `parse` that finds nothing — it is unfalsifiable against the obvious skeleton. Every test in the spine therefore pairs `round_trip` (identity) with a kind-sequence or block-set assertion in the same test; the pair, not the identity line, is the falsifiable unit, and a future test that asserted only identity would be vacuous and must not be added.
- **No weld walks the new fixture directory** (checked while planning): `docs_index_drift` walks `docs/` only; `error_fixtures.rs` walks its own `error-*.md` samples; `workspace_publication.rs` walks `crates/*/Cargo.toml`; nothing enumerates `tests/fixtures/`. The SCN-16 fixture's canonical home is `crates/transync/tests/fixtures/` (spec §11 row 1 — wave 5's scenario test consumes it there); this crate's test reaches it with a relative `include_str!`, so the two waves share one file and cannot drift.
- **Two welds ride this task's commit because they go red the moment the module exists:** `docs_ownership_drift` (lib.rs module list ↔ source-of-truth table) and nothing else; the `public_surface.rs` `forbidden` addition is preventive, not red, but belongs with the module it guards.

- [ ] **Step 1: Write the SCN-16 fixture.** Create `crates/transync/tests/fixtures/scn-16-html-document.html` with exactly this content (LF line endings, trailing newline — §11 row 1's required features are all present: doctype, head+title, nested sections, script/style, entities, unclosed fragment):
```html
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="description" content="Anchor &amp; sync, side by side">
<title>Transync &amp; the two-pane page</title>
<link rel="stylesheet" href="site.css">
<style>
main > section { margin: 0 auto; }
</style>
<script>
if (2 < 1) { document.write("</div>"); }
</script>
</head>
<body>
<header>
<h1 id="top">Anchors &amp; panes</h1>
<nav>
<ul>
<li><a href="#what">What it is</a></li>
<li><a href="#how">How it works</a></li>
</ul>
</nav>
</header>
<main>
<section id="what">
<h2>What it is</h2>
<p>A translated page keeps its <em>structure</em> &mdash; only the text moves.</p>
<img src="badge-build.svg" alt="build"> <img src="badge-docs.svg" alt="docs">
<blockquote>
<p>Blocks, not percentages.</p>
</blockquote>
</section>
<section id="how">
<h2>How it works</h2>
<table>
<tr><th>Side</th><th>Owner</th></tr>
<tr><td>Structure</td><td>Rust</td></tr>
</table>
<pre><code>let doc = parse(&amp;source);</code></pre>
<hr>
<x-note tone="info">A custom element stops, loudly.</x-note>
<!-- the trailing div below never closes: the intake must warn, not panic -->
<div class="trailing">
<p>This fragment never closes.
</section>
</main>
</body>
</html>
```
What each region is for: the doctype and the comment are `Skip`-token gaps (rule T's MEASURED case); the head exercises head mode (title stops, meta/link/style/script are gap; the script body contains `<` and a `"</div>"` string the raw-text state must not tokenize); the nav list is D9's per-`<li>` model; the badge pair is §11's pinned two-adjacent-`img` → one `Image` case; the two `<section>`s + `<h1>`/`<h2>`s exercise the `SectionStack`; `<x-note>` is DEFAULT-STOP; the trailing `<div><p>` is the force-close-with-warning acceptance case.

- [ ] **Step 2: Write the two corpus files.** Create `crates/transync-syntax/tests/fixtures/html-corpus/marketing-page.html`:
```html
<!DOCTYPE HTML>
<HTML>
<HEAD>
<META CHARSET=utf-8>
<TITLE>Blocks &copy; 2026</TITLE>
</HEAD>
<BODY class=home>
<DIV ID=page>
<div class=hero>
<h1>Ship <span class=accent>both</span> panes</h1>
<p>No rebuild<p>No reparse
<img src=one.png alt=one> <img src=two.png alt=two>
<div class=spacer>&nbsp;</div>
</div>
<noscript><p>Enable scripts for the live demo.</p></noscript>
<iframe src="demo.html" title="demo"></iframe>
<menu>
<li>copy
<li>paste
</menu>
<dl>
<dt>anchor</dt><dd>a block-id pair</dd>
</dl>
<form action="/subscribe">
<fieldset>
<legend>Stay current</legend>
<p><label>Email <input type=email name=q></label></p>
</fieldset>
</form>
<script type="application/ld+json">
{"@type":"WebPage","name":"Blocks"}
</script>
<footer>
<p>&copy; 2026 &mdash; <a href="/legal">legal</a></p>
</footer>
</DIV>
</BODY>
</HTML>
```
What it stresses: UPPERCASE tag spellings (the scanner lowercases *names* while spans stay exact — identity must survive); unquoted attribute values; `<p>No rebuild<p>No reparse` implicit closes; **the img pair that must NOT become an `Image` block** — it follows an unclosed `<p>`, so the pairing discipline puts both imgs *inside* that paragraph's extent (`implicitly_closes("div")` is what finally pops it at `<div class=spacer>`), the exact "one pairing opinion" property rule T must defer to; an `&nbsp;`-entity-only run (bytes are non-whitespace → a paragraph, by rule T's literal byte test); a JSON-LD `<script>` **in body, deliberately** — it sits between `</form>` and `<footer>`, NOT in `<HEAD>`, because a head-positioned script proves nothing about the VERBATIM class (head mode alone already makes every non-`title` element gap there); the body position is what exercises "VERBATIM anywhere, not just in head" (the review's F5 call: the fixture was moved rather than the annotation weakened, so the fixture proves what it claims); `noscript`/`iframe`/`dt`/`dd`/`legend` DEFAULT-STOPs; a `menu` list; phrasing `label`/`input` inside an explicit `<p>`.

Create `crates/transync-syntax/tests/fixtures/html-corpus/docs-fragment.html`:
```html
<section class="ref">
<h1>parse_html</h1>
<p>Builds the block IR from an <code>&lt;html&gt;</code> document.</p>
<ol>
<li>tokenize
<li>pair
<li>classify
</ol>
<pre>
let doc = intake::html::parse(source);
</pre>
<dl>
<dt>identity</dt>
<dd>byte-for-byte, outside translated text</dd>
</dl>
<div class="note">
<p>The tail of this fragment is deliberately unclosed
```
What it stresses: a legitimate **body fragment** (D8: no preamble shape at all — no doctype, no head); an *ordered* list with all-implicit `<li>` closes; `<pre>` → `CodeBlock { info: None, fenced: true }`; `dt`/`dd` under a pass-through `dl`; and an EOF force-close two levels deep (`<section>` and `<div>` still open, `<p>` unclosed) — warnings, never a panic, and still byte-identical.

- [ ] **Step 3: Write the spine test — FIRST, before any module exists.** Create `crates/transync-syntax/tests/html_intake_identity.rs`:
```rust
//! Wave 3's spine (spec 2026-08-20 §12): an HTML document goes in and comes
//! out byte-identical, with a real block set and real ids — no LLM, no
//! pipeline, no core. `regen::regenerate` copies inter-block gaps verbatim
//! and splices `source_text[range]` for every block absent from the accepted
//! map, so byte-identity over `&HashMap::new()` is precisely the statement
//! that the intake's ranges are in source order, non-overlapping and in
//! bounds — the three invariants the intake debug-asserts.
//!
//! A byte-identity assertion ALONE is unfalsifiable against the obvious
//! stub: a `parse` that finds zero blocks makes `regenerate` reproduce the
//! whole source as one gap, trivially. Every test here therefore pairs the
//! round trip with a block-set (kind-sequence) assertion in the same test;
//! the pair is the falsifiable unit. Do not add identity-only tests.

use std::collections::HashMap;
use transync_syntax::id::{SourceFormat, Spelling};
use transync_syntax::parser::Document;
use transync_syntax::{intake, regen};

/// The spec-mandated fixture (§11 row 1). Its canonical home is the SCN
/// suite — wave 5's scenario test consumes the SAME file — so this crate
/// reaches it relatively rather than keeping a copy that could drift.
const SCN_16: &str = include_str!("../../transync/tests/fixtures/scn-16-html-document.html");
const MARKETING: &str = include_str!("fixtures/html-corpus/marketing-page.html");
const DOCS_FRAGMENT: &str = include_str!("fixtures/html-corpus/docs-fragment.html");

/// Inline, never checked-in files: a lone CR, a BOM and a NUL are exactly
/// the bytes an editor, a filter, or an EOL-normalizing tool is liable to
/// mangle on the way to disk (the OI-0033 fixture precedent), and the test
/// would then pass vacuously.
const CRLF_PAGE: &str = "<div>\r\n<p>CRLF line one</p>\r\n<p>CRLF line two</p>\r\n</div>\r\n";
const LONE_CR_PAGE: &str = "<p>alpha</p>\r<p>bravo</p>\r";
const BOM_PAGE: &str = "\u{feff}<p>after the BOM</p>\n";
const DOCTYPE_ONLY: &str = "<!DOCTYPE html>\n";
const PLAIN_TEXT: &str = "Just words, no tags at all.\n";

/// Parse, regenerate against the empty map, assert byte-identity, and hand
/// the document back for block-set assertions — which every caller MUST
/// make: identity alone passes on a zero-block stub (see the module doc).
fn round_trip(name: &str, source: &str) -> Document {
    let doc = intake::html::parse(source);
    let (out, offsets) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(
        out, doc.source_text,
        "{name}: regenerate(parse_html(doc), &empty) must be byte-identical"
    );
    assert_eq!(offsets.0.len(), doc.blocks.len(), "{name}: one offset per block");
    doc
}

fn kinds(doc: &Document) -> Vec<&'static str> {
    doc.blocks.iter().map(|b| b.kind.wire_str()).collect()
}

#[test]
fn the_scn_16_fixture_round_trips_byte_identical_with_the_designed_block_set() {
    let doc = round_trip("scn-16", SCN_16);
    assert_eq!(doc.source_text, SCN_16, "no NUL, so parse must not copy-modify");
    assert_eq!(
        kinds(&doc),
        vec![
            "title", "heading-1", "list-item", "list-item", "heading-2", "paragraph",
            "image", "blockquote", "heading-2", "table", "code-block",
            "thematic-break", "html", "paragraph",
        ],
    );
    let ids: Vec<&str> = doc.blocks.iter().map(|b| b.block_id.0.as_str()).collect();
    assert_eq!(
        ids,
        vec![
            "title-0001", "h1-0002", "li-0003", "li-0004", "h2-0005", "p-0006",
            "img-0007", "q-0008", "h2-0009", "t-0010", "c-0011", "hr-0012",
            "html-0013", "p-0014",
        ],
        "emission-order stamping with the one global ordinal (spec §4)",
    );
    assert_eq!(doc.format, SourceFormat::Html);
    assert!(
        doc.blocks
            .iter()
            .all(|b| matches!(b.spelling, Spelling::Html { block_type: None })),
        "format == Html implies every spelling is Html {{ block_type: None }} (spec §3)",
    );
    assert_eq!(doc.warnings.len(), 1, "exactly the unclosed <p>: {:?}", doc.warnings);
    assert!(doc.warnings[0].contains("p-0014"), "{}", doc.warnings[0]);
    // §11's pinned case: two adjacent <img> tags are ONE Image block.
    let img = &doc.blocks[6];
    let payload = &doc.source_text[img.source_range.start..img.source_range.end];
    assert_eq!(
        payload,
        "<img src=\"badge-build.svg\" alt=\"build\"> <img src=\"badge-docs.svg\" alt=\"docs\">",
    );
}

#[test]
fn a_messy_marketing_page_round_trips_and_the_pairing_discipline_owns_its_imgs() {
    let doc = round_trip("marketing-page", MARKETING);
    assert_eq!(
        kinds(&doc),
        vec![
            "title", "heading-1", "paragraph", "paragraph", "paragraph", "html",
            "html", "list-item", "list-item", "html", "html", "html",
            "paragraph", "paragraph",
        ],
    );
    // The img pair follows an UNCLOSED <p>, so the extents put both imgs
    // INSIDE that paragraph — rule T's exception never sees them, and no
    // Image block may exist in this document.
    assert!(!kinds(&doc).contains(&"image"));
    let p2 = &doc.blocks[3];
    let payload = &doc.source_text[p2.source_range.start..p2.source_range.end];
    assert!(payload.starts_with("<p>No reparse"), "{payload}");
    assert!(payload.contains("src=two.png"), "{payload}");
    assert_eq!(
        doc.warnings.len(),
        4,
        "two implicit <p> closes + two implicit <li> closes: {:?}",
        doc.warnings,
    );
}

#[test]
fn an_unclosed_fragment_force_closes_with_warnings_and_still_round_trips() {
    let doc = round_trip("docs-fragment", DOCS_FRAGMENT);
    assert_eq!(
        kinds(&doc),
        vec![
            "heading-1", "paragraph", "list-item", "list-item", "list-item",
            "code-block", "html", "html", "paragraph",
        ],
    );
    use transync_syntax::id::BlockKind;
    assert!(
        doc.blocks[2..5]
            .iter()
            .all(|b| matches!(b.kind, BlockKind::ListItem { ordered: true, task: None })),
        "an <ol>'s items are ordered (D9)",
    );
    // Three implicit <li> closes, the EOF-force-closed <p>, and the two
    // containers (<section>, <div>) still open at end of input.
    assert_eq!(doc.warnings.len(), 6, "{:?}", doc.warnings);
    assert!(doc.warnings.iter().any(|w| w.contains("end of input")));
}

#[test]
fn edge_documents_round_trip() {
    let doc = round_trip("crlf", CRLF_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"]);

    // Spans are direct byte offsets — no line table exists on this path, so
    // the lone-CR sourcepos hazard (OI-0033) structurally cannot recur.
    let doc = round_trip("lone-cr", LONE_CR_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph", "paragraph"]);

    let doc = round_trip("bom", BOM_PAGE);
    assert_eq!(kinds(&doc), vec!["paragraph"]);
    assert_eq!(
        doc.blocks[0].source_range.start,
        "\u{feff}".len(),
        "the BOM stays in the inter-block gap, exactly as on the Markdown side",
    );

    let doc = round_trip("doctype-only", DOCTYPE_ONLY);
    assert!(
        doc.blocks.is_empty(),
        "a doctype-only gap mints NO block (spec §12 wave 3): {:?}",
        doc.blocks,
    );

    let doc = round_trip("empty", "");
    assert!(doc.blocks.is_empty());

    let doc = round_trip("plain-text", PLAIN_TEXT);
    assert_eq!(
        kinds(&doc),
        vec!["paragraph"],
        "a tagless file is one text-heavy block set — D8's explicitly-requested shape",
    );
}

#[test]
fn a_nul_bearing_document_round_trips_to_its_normalized_self() {
    let src = "<p>alpha\0bravo</p>\n";
    let doc = intake::html::parse(src);
    assert_eq!(
        doc.source_text, "<p>alpha\u{fffd}bravo</p>\n",
        "the SAME normalize_source contract as the Markdown intake (spec §4)",
    );
    let (out, _) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(out, doc.source_text, "identity is over Document::source_text");
}
```

- [ ] **Step 4: Run it and see it fail — the compile red.** Said plainly: this red is an unresolved import, not a behavioural failure — it proves the test harness and fixture wiring, nothing about the algorithm. The wave's *behavioural* red is Step 9, where a deliberately incomplete walk makes the kind-sequence assertions fail with real diffs while byte-identity already passes.
```bash
cargo test -p transync-syntax --test html_intake_identity -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` and
```
error[E0432]: unresolved import `transync_syntax::intake`
```
(with rustc noting there is `no `intake` in the root`). The fixtures must NOT be part of the failure — if an `include_str!` error appears, a fixture path from Steps 1–2 is wrong; fix that first so the one red is the missing module.

- [ ] **Step 5: Widen the two Markdown-intake internals both intakes must share.** In `crates/transync-syntax/src/parser.rs`, two one-line changes:
  - `mod sections;` becomes `pub(crate) mod sections;`
  - `fn normalize_source(source: &str) -> Cow<'_, str>` becomes `pub(crate) fn normalize_source(source: &str) -> Cow<'_, str>` (doc comment untouched — every word still holds; add one sentence at its end: `The HTML intake (`intake::html`) calls this too, so both intakes mean the same string by "source bytes" (spec §4).`)

  In `crates/transync-syntax/src/parser/sections.rs`, change all four `pub(super)` to `pub(crate)` (the `SectionStack` struct and its `current_path`/`close_through`/`open_scope` methods). This is the one-home rule in action: `sections.rs`'s own doc says "One rule, one implementation" — the HTML intake must reuse it, and a private copy would be the drift this module exists to prevent. Neither widening is visible outside the crate.

- [ ] **Step 6: Create the seam's parent module.** Create `crates/transync-syntax/src/intake.rs`:
```rust
//! The format seam: one intake per source format, one block IR out of both
//! (spec 2026-08-20 §4, decision D3).
//!
//! [`html`] is the HTML document intake. The Markdown intake is
//! [`crate::parser`] — the spec sketches it as `intake::markdown`, and that
//! rename is **deferred, on record** (DCR-0035): `parser` is named by over a
//! hundred `crate::parser` / `transync_syntax::parser` paths across all four
//! consuming crates, by `transync-core`'s crate-private re-export, and by
//! `public_surface.rs`'s hidden-path pins, and moving it buys no behaviour.
//! The seam is real without it: both intakes produce the same [`Document`],
//! normalize NUL through the same `parser::normalize_source` (crate-private),
//! stamp ids with the same global-ordinal scheme, and share THE
//! heading-scope stack (`parser::sections`).
//!
//! Not to be confused with [`crate::parser::intake`], the NUL/nesting guard
//! *function* on the Markdown side (OI-0034), which predates this module and
//! shares the word.
//!
//! [`Document`]: crate::parser::Document
//!
//! TRACE: ADR-0025

pub mod html;
```
  **Doc-link rule for this module and `intake::html` (both are `pub`):** a doc comment on a `pub` item must never intra-doc-link (``[`…`]``) a `pub(crate)`, `pub(super)` or private item — `parser::normalize_source` and `debug_assert_block_invariants` above/below are named in plain backticks for exactly this reason. Rustdoc's `private_intra_doc_links` lint is warn-by-default, and the pre-commit hook's rustdoc gate runs `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` (no `--document-private-items`) with `transync-syntax` in `RUSTDOC_GATE_CRATES`, so one such link blocks the commit outright. Links to `pub` items are fine and stay: [`crate::parser`], [`crate::parser::intake`] (a `pub fn`), `Document`, `scan_tags`, `element_extents`, `BlockKind::Html` — all verified `pub` while planning.

- [ ] **Step 7: Create the intake — with rule T deliberately staged.** Create `crates/transync-syntax/src/intake/html.rs` with exactly this content (the classification tables transcribe spec §4's element sets verbatim — do not reorder or "complete" them). **One piece is intentionally incomplete:** `flush_run` lands as a state-advancing stub that classifies nothing, and `trim_run`/`any_naked_char`/the `BOM` const do not exist yet — Step 10 adds them. This staging is what makes Step 9's red a *behavioural* red with a real failing diff, instead of the whole module landing in one pass-on-arrival step:
```rust
//! THE HTML document intake: `source_text` → block IR (spec 2026-08-20 §4).
//!
//! One linear pass over [`scan_tags`]' token stream, steered by
//! [`element_extents`]' precomputed pairing verdicts — the crate-external
//! half of `transync-html`'s single pairing discipline. This module decides
//! what a block *is* (classification, kinds, ids, sections, `ast_path`);
//! the mechanics crate has no opinion about that, and this module has no
//! opinion about pairing: it never re-pairs a tag, it reads the extents.
//!
//! **Classification — five classes, default STOP (spec §4):**
//! - PASS-THROUGH: markup becomes inter-block gap bytes; segmentation
//!   descends.
//! - STOP → semantic kind: the element becomes a leaf block; nothing is
//!   emitted inside one.
//! - VERBATIM: never a block; the bytes stay gap.
//! - PHRASING: absorbed into anonymous text runs; never a boundary.
//! - DEFAULT-STOP → [`BlockKind::Html`]: `summary`, `dt`, `dd`, `address`,
//!   `dialog`, `iframe`, `noscript`, `template`, `legend`, custom elements,
//!   future elements — the safe direction: over-stopping yields a coarser
//!   block, under-stopping would leak text into untranslated gap (D4).
//!
//! **Ranges are the whole element, tags included** (spec §4): void elements
//! are the tag span, anonymous runs are the trimmed run, forced closes run
//! to the boundary with trailing ASCII whitespace excluded. Whole-element
//! ranges are what `outcome::block_payload`, layer 3's
//! `HtmlSegmentConstraints.source_bytes`, `regen`'s fallback splice and
//! `source_hash` all want.
//!
//! **Identity by construction:** every emitted range starts at a `<`, ends
//! after a `>` or at an ASCII-whitespace trim, advances a monotone cursor,
//! and never nests — so `regen::regenerate(doc, &empty)` reproduces the
//! input byte-for-byte, which is wave 3's acceptance and this module's
//! `debug_assert_block_invariants` tripwire (named, not linked: the fn is
//! `pub(crate)` and this module doc is public).
//!
//! TRACE: ADR-0025

use crate::id::{BlockId, BlockKind, SourceFormat, Spelling};
use crate::parser::ranges::ByteRange;
use crate::parser::sections::SectionStack;
use crate::parser::{AstPath, Block, Document};
use std::collections::HashMap;
use transync_html::{ElementExtent, TagToken, element_extents, scan_tags};

/// PASS-THROUGH (spec §4): markup → gap bytes. `head` is handled by the
/// walker (it is the element that OPENS head mode) and — per D9 — `ul`,
/// `ol`, `menu`, `dl` pass through **to their items**.
const PASS_THROUGH: &[&str] = &[
    "html", "body", "div", "section", "article", "main", "nav", "aside",
    "header", "footer", "hgroup", "figure", "details", "form", "fieldset",
    "search", "center", "ul", "ol", "menu", "dl",
];

/// VERBATIM (spec §4): never a block; bytes stay gap. Extraction already
/// excludes script/style text by `TextType`; these have no reader-visible
/// anchor surface. ("Everything in head mode except `title`" is the mode
/// rule in the walker, not a name in this list.)
const VERBATIM: &[&str] = &["script", "style", "link", "meta", "base"];

/// PHRASING (spec §4, the recorded widening of D4): absorbed into text
/// runs, never a boundary. Without it `Hello <b>world</b> out there`
/// shatters into three blocks and the sentence crosses translation units.
const PHRASING: &[&str] = &[
    "a", "abbr", "audio", "b", "bdi", "bdo", "br", "button", "canvas",
    "cite", "code", "data", "del", "dfn", "em", "embed", "i", "img",
    "input", "ins", "kbd", "label", "mark", "math", "object", "optgroup",
    "option", "output", "picture", "q", "rp", "rt", "ruby", "s", "samp",
    "select", "small", "span", "strong", "sub", "sup", "svg", "textarea",
    "time", "u", "var", "video", "wbr",
];

fn is_pass_through(name: &str) -> bool {
    PASS_THROUGH.contains(&name)
}

fn is_verbatim(name: &str) -> bool {
    VERBATIM.contains(&name)
}

fn is_phrasing(name: &str) -> bool {
    PHRASING.contains(&name)
}

/// STOP → semantic kind (spec §4), for the kinds that need no context.
/// `li` (needs its parent), `hr` (void), `title` (needs head mode) and the
/// img-only run (rule T's exception) are the walker's; every name in no
/// class is DEFAULT-STOP → [`BlockKind::Html`].
fn semantic_stop_kind(name: &str) -> Option<BlockKind> {
    Some(match name {
        "h1" => BlockKind::Heading1,
        "h2" => BlockKind::Heading2,
        "h3" => BlockKind::Heading3,
        "h4" => BlockKind::Heading4,
        "h5" => BlockKind::Heading5,
        "h6" => BlockKind::Heading6,
        // `figcaption` is prose with a paragraph's shape (spec §4).
        "p" | "figcaption" => BlockKind::Paragraph,
        // Whole element, one leaf block — invariant 3's posture.
        "table" => BlockKind::Table,
        // `fenced` is vacuous under Html spelling: both consumers of
        // `fenced == false` sit behind `Spelling::Markdown` dispatch
        // (spec §4).
        "pre" => BlockKind::CodeBlock { info: None, fenced: true },
        "blockquote" => BlockKind::Blockquote,
        _ => return None,
    })
}

/// Parse an HTML document into the block IR. Infallible: `scan_tags` and
/// `element_extents` are total, malformed input force-closes with
/// [`Document::warnings`] notes (spec §4), and NUL is normalized — the same
/// `normalize_source` the Markdown intake applies, so both intakes mean the
/// same string by "source bytes". The CLI's fallible boundary (exit-2
/// `InputReadFailure`, spec §9) is file IO, which is not this function.
///
/// Identity contract, pinned by `tests/html_intake_identity.rs`:
/// `regen::regenerate(&parse(s), &HashMap::new()).0 == parse(s).source_text`.
pub fn parse(source: &str) -> Document {
    let normalized = crate::parser::normalize_source(source);
    let mut walk = HtmlWalk::new(&normalized);
    walk.walk();
    let HtmlWalk { blocks, warnings, .. } = walk;
    let doc = Document {
        source_text: normalized.into_owned(),
        format: SourceFormat::Html,
        blocks,
        warnings,
        // No CommonMark context exists, so no link-reference definitions.
        ref_defs: String::new(),
    };
    debug_assert_block_invariants(&doc);
    doc
}

/// Regen's three requirements, checked where they are created (spec §4):
/// source order, non-overlap, in-bounds offsets on UTF-8 boundaries.
/// Unreachable through [`parse`] by construction — every range starts on an
/// ASCII `<`, ends after an ASCII `>` or an ASCII-whitespace trim, and
/// follows a monotone cursor — so this is a debug tripwire, not a
/// production failure mode. `Document` is pub-fields, so the firing
/// behaviour is pinned by calling this directly on a hand-built bad
/// document (the `warn_on_empty_list_item_range` precedent).
pub(crate) fn debug_assert_block_invariants(doc: &Document) {
    let mut prev_end = 0usize;
    for block in &doc.blocks {
        let ByteRange { start, end } = block.source_range;
        debug_assert!(start <= end, "{}: inverted range {start}..{end}", block.block_id);
        debug_assert!(
            start >= prev_end,
            "{}: starts at {start}, inside or before its predecessor (which ends at \
             {prev_end}) — blocks must be in source order and non-overlapping",
            block.block_id,
        );
        debug_assert!(
            end <= doc.source_text.len(),
            "{}: range end {end} is past the end of source_text ({})",
            block.block_id,
            doc.source_text.len(),
        );
        debug_assert!(
            doc.source_text.is_char_boundary(start) && doc.source_text.is_char_boundary(end),
            "{}: range {start}..{end} splits a UTF-8 char",
            block.block_id,
        );
        prev_end = end;
    }
}

/// One open pass-through container.
struct Container {
    name: String,
    /// Whole-element end: the close tag's end, or `content_end` when the
    /// extents walk closed it implicitly / left it open at EOF.
    end: usize,
    /// The container's own `ast_path` (ancestors' child ordinals + its own).
    path: Vec<usize>,
    /// Whether the source closed it with a real end tag.
    explicitly_closed: bool,
}

/// Rule T's per-run accumulators: spans excluded from the naked-byte test.
#[derive(Default)]
struct RunTags {
    /// `Skip` spans (comments, CDATA, doctype/bogus comments) in the run.
    skips: Vec<(usize, usize)>,
    /// Phrasing/absorbed tag spans in the run.
    tags: Vec<(usize, usize)>,
    /// `img` open-tag spans in the run — rule T's exception.
    imgs: Vec<(usize, usize)>,
}

/// Everything one walk of one document accumulates (the `WalkState` shape,
/// for the second intake).
struct HtmlWalk<'s> {
    source: &'s str,
    extents: Vec<ElementExtent>,
    /// Extent index by open-tag start offset — `scan_tags` and
    /// `element_extents` see the same open tags at the same offsets, so
    /// this lookup is total for every extent-backed `Open` token.
    extent_at: HashMap<usize, usize>,
    blocks: Vec<Block>,
    warnings: Vec<String>,
    /// The one global ordinal (spec §4): `BlockId::new(kind.id_code(), n)`.
    counter: u32,
    /// THE heading-scope stack, shared with the Markdown intake.
    sections: SectionStack,
    containers: Vec<Container>,
    /// One child-ordinal counter per open level; `[0]` is the root level.
    child_counts: Vec<usize>,
    /// Where the current anonymous run began (rule T).
    seg_start: usize,
    run: RunTags,
    /// Bytes before this offset are inside an already-consumed leaf.
    cursor: usize,
    /// Head mode is active for tokens starting before this offset.
    head_until: Option<usize>,
    /// Tokens starting before this offset are inside an open PHRASING
    /// element's extent and are absorbed whole (deviation 6): this is what
    /// keeps `<svg><title>` out of head-mode `Title` and keeps a phrasing
    /// element's descendants inside one run.
    phrasing_hold: usize,
}

fn token_span(tok: &TagToken) -> (usize, usize) {
    match tok {
        TagToken::Open { span, .. } | TagToken::Close { span, .. } | TagToken::Skip { span } => {
            *span
        }
    }
}

impl<'s> HtmlWalk<'s> {
    fn new(source: &'s str) -> Self {
        let extents = element_extents(source);
        let extent_at = extents.iter().enumerate().map(|(i, e)| (e.open.0, i)).collect();
        HtmlWalk {
            source,
            extents,
            extent_at,
            blocks: Vec::new(),
            warnings: Vec::new(),
            counter: 0,
            sections: SectionStack::default(),
            containers: Vec::new(),
            child_counts: vec![0],
            seg_start: 0,
            run: RunTags::default(),
            cursor: 0,
            head_until: None,
            phrasing_hold: 0,
        }
    }

    /// Whole-element end for extent `idx`.
    fn element_end(&self, idx: usize) -> usize {
        let e = &self.extents[idx];
        e.close.map_or(e.content_end, |c| c.1)
    }

    /// The one pass (spec §4's "one stack walk"): tokens in source order,
    /// extents consulted positionally.
    fn walk(&mut self) {
        for tok in scan_tags(self.source) {
            let (tstart, tend) = token_span(&tok);
            if tstart < self.cursor {
                continue; // inside a consumed leaf: nothing is emitted there
            }
            if let Some(limit) = self.head_until {
                let body_open = matches!(&tok, TagToken::Open { name, .. } if name == "body");
                if tstart < limit && !body_open {
                    self.head_token(&tok, tend);
                    continue;
                }
                // Past the head — or a <body> arriving inside a head whose
                // </head> is missing (plan deviation 3): the MODE ends here;
                // the still-open head container is the extents' business.
                self.head_until = None;
            }
            if tstart < self.phrasing_hold {
                self.absorb(&tok);
                continue;
            }
            match &tok {
                TagToken::Skip { span } => self.run.skips.push(*span),
                TagToken::Close { name, span } => {
                    if is_phrasing(name) {
                        self.run.tags.push(*span);
                    } else {
                        // Enclosing (or orphan) close: a hard boundary.
                        self.flush_run(span.0);
                        self.pop_closed(span.1);
                        self.seg_start = span.1;
                    }
                }
                TagToken::Open { name, self_closing, span } => {
                    self.open(name, *self_closing, *span);
                }
            }
        }
        // Rule T never runs in head mode (spec §4: everything in head mode
        // except `title` is VERBATIM gap) — INCLUDING at EOF. On a page
        // truncated inside an unclosed <head> (`head_until` still reaching
        // `source.len()`), the tail is head-mode gap, not a paragraph.
        if !self.head_until.is_some_and(|limit| limit >= self.source.len()) {
            self.flush_run(self.source.len());
        }
        for c in std::mem::take(&mut self.containers) {
            if !c.explicitly_closed && c.end >= self.source.len() {
                self.warnings.push(format!(
                    "html intake: <{}> was still open at end of input and was force-closed",
                    c.name,
                ));
            }
        }
    }

    /// Head mode (spec §4): only `title` stops inside `<head>`; everything
    /// else — meta, link, base, script, style, stray text, anything — is
    /// VERBATIM gap. Never rule T.
    fn head_token(&mut self, tok: &TagToken, tend: usize) {
        match tok {
            TagToken::Open { name, span, .. } => {
                if let Some(&idx) = self.extent_at.get(&span.0) {
                    if name == "title" {
                        // Emitted in source order, EMPTY section_path — a
                        // title opens no scope (spec §4).
                        self.emit_extent_leaf(BlockKind::Title, *span, Vec::new());
                    } else {
                        self.cursor = self.cursor.max(self.element_end(idx));
                    }
                }
                // Void head furniture (<meta>, <link>, <base>) has no
                // extent; its tag bytes are already gap.
            }
            TagToken::Close { name, span } => {
                if !is_phrasing(name) {
                    // `</head>` itself lands here and pops by the extents'
                    // verdict; the mode then ends at its recorded limit.
                    self.pop_closed(span.1);
                }
            }
            TagToken::Skip { .. } => {}
        }
        self.seg_start = self.seg_start.max(tend).max(self.cursor);
    }

    /// Absorb a token into the current run without letting its name steer
    /// classification — used inside an open phrasing element's extent.
    fn absorb(&mut self, tok: &TagToken) {
        match tok {
            TagToken::Skip { span } => self.run.skips.push(*span),
            TagToken::Open { name, span, .. } => {
                self.run.tags.push(*span);
                if name == "img" {
                    self.run.imgs.push(*span);
                }
                if let Some(&idx) = self.extent_at.get(&span.0) {
                    let end = self.element_end(idx);
                    self.phrasing_hold = self.phrasing_hold.max(end);
                }
            }
            TagToken::Close { span, .. } => self.run.tags.push(*span),
        }
    }

    /// An `Open` token at segmentation level: classify and act.
    fn open(&mut self, name: &str, _self_closing: bool, span: (usize, usize)) {
        // The self-closing flag is deliberately unread here: the pairing
        // discipline already folded it into the extents (a self-closing
        // non-raw-text element minted no extent; a self-closing raw-text
        // element minted one — DCR-0016 Part D), and reading it again would
        // be a second opinion.
        if is_phrasing(name) {
            self.run.tags.push(span);
            if name == "img" {
                self.run.imgs.push(span);
            }
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                self.phrasing_hold = self.phrasing_hold.max(end);
            }
            return;
        }

        // Every non-phrasing open is a hard boundary for rule T.
        self.flush_run(span.0);

        if name == "head" {
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                let closed = self.extents[idx].close.is_some();
                self.enter_container(name, end, closed);
                self.head_until = Some(end);
            }
            self.seg_start = span.1;
            return;
        }
        if is_pass_through(name) {
            if let Some(&idx) = self.extent_at.get(&span.0) {
                let end = self.element_end(idx);
                let closed = self.extents[idx].close.is_some();
                self.enter_container(name, end, closed);
            }
            // A self-closing `<div/>` counts as closed under the pairing
            // discipline and minted no extent: pure gap.
            self.seg_start = span.1;
            return;
        }
        if is_verbatim(name) {
            // script/style hold raw text and have extents; link/meta/base
            // are void. Either way: no block, bytes stay gap.
            if let Some(&idx) = self.extent_at.get(&span.0) {
                self.cursor = self.cursor.max(self.element_end(idx));
            }
            self.seg_start = self.seg_start.max(span.1).max(self.cursor);
            return;
        }
        if name == "hr" {
            // Void STOP: the tag span is the block (spec §4).
            let sp = self.sections.current_path();
            self.emit(
                BlockKind::ThematicBreak,
                sp,
                ByteRange { start: span.0, end: span.1 },
            );
            self.seg_start = span.1;
            return;
        }

        // STOP → semantic kind, or DEFAULT-STOP → BlockKind::Html.
        let kind = self.stop_kind(name);
        if let Some(level) = kind.heading_level() {
            // Opening/closing SectionStack scopes exactly as Markdown
            // headings do — which is what keeps `partition_by_section`
            // working unchanged (spec §4).
            let sp = self.sections.close_through(level);
            let id = self.emit_extent_leaf(kind, span, sp);
            self.sections.open_scope(id, level);
        } else {
            let sp = self.sections.current_path();
            self.emit_extent_leaf(kind, span, sp);
        }
    }

    /// The kind a stopping element takes. `li` is `ListItem` only as the
    /// direct child of a list container (D9); `title` reaches here only
    /// OUTSIDE head mode, where it is nobody's semantic kind; everything
    /// unnamed is DEFAULT-STOP.
    fn stop_kind(&self, name: &str) -> BlockKind {
        if name == "li" {
            if let Some(c) = self.containers.last()
                && matches!(c.name.as_str(), "ul" | "ol" | "menu")
            {
                return BlockKind::ListItem { ordered: c.name == "ol", task: None };
            }
            return BlockKind::Html;
        }
        semantic_stop_kind(name).unwrap_or(BlockKind::Html)
    }

    /// Emit a leaf whose range is the WHOLE element, tags included
    /// (spec §4): close-tag end when closed; boundary minus trailing ASCII
    /// whitespace when force-closed; the open-tag span alone when the
    /// pairing discipline says a self-closing element is already closed.
    fn emit_extent_leaf(
        &mut self,
        kind: BlockKind,
        open_span: (usize, usize),
        section_path: Vec<BlockId>,
    ) -> BlockId {
        let extent = self.extent_at.get(&open_span.0).map(|&i| self.extents[i].clone());
        let (range, consumed_end, forced) = match &extent {
            Some(ex) => match ex.close {
                Some((_, close_end)) => {
                    (ByteRange { start: open_span.0, end: close_end }, close_end, false)
                }
                None => {
                    let end = trim_ascii_ws_back(self.source, ex.content_end, open_span.1);
                    (ByteRange { start: open_span.0, end }, ex.content_end, true)
                }
            },
            None => (ByteRange { start: open_span.0, end: open_span.1 }, open_span.1, false),
        };
        let id = self.emit(kind, section_path, range);
        if forced {
            let name = &extent.as_ref().expect("forced implies an extent").name;
            self.warnings.push(if consumed_end >= self.source.len() {
                format!(
                    "html intake: <{name}> ({id}) was still open at end of input and was \
                     force-closed"
                )
            } else {
                format!(
                    "html intake: <{name}> ({id}) has no end tag and was closed implicitly \
                     at byte {consumed_end}"
                )
            });
        }
        self.cursor = self.cursor.max(consumed_end);
        self.seg_start = self.cursor;
        id
    }

    /// Push ONE block: bump the one global ordinal, mint the kind-prefixed
    /// id, hash the exact range bytes, stamp `Spelling::Html { block_type:
    /// None }` (spec §3's format invariant) and the emission-order
    /// `ast_path`.
    fn emit(&mut self, kind: BlockKind, section_path: Vec<BlockId>, range: ByteRange) -> BlockId {
        self.counter += 1;
        let id = BlockId::new(kind.id_code(), self.counter);
        let hash =
            crate::id::source_hash_bytes(&self.source.as_bytes()[range.start..range.end]);
        let ast_path = self.next_child_path();
        self.blocks.push(Block {
            block_id: id.clone(),
            kind,
            spelling: Spelling::Html { block_type: None },
            source_range: range,
            source_hash: hash,
            section_path,
            ast_path: AstPath(ast_path),
        });
        id
    }

    /// The child-index convention that mirrors comrak's (spec §4): every
    /// emitted block and every entered container takes the next ordinal at
    /// its level, so an `<li>`'s path is its list container's path plus the
    /// item ordinal — `walk::normalize_top_level`'s prefix collapse stays
    /// total and adjacent lists can never merge.
    fn next_child_path(&mut self) -> Vec<usize> {
        let counter = self.child_counts.last_mut().expect("the root level always exists");
        let ordinal = *counter;
        *counter += 1;
        let mut path = self.containers.last().map(|c| c.path.clone()).unwrap_or_default();
        path.push(ordinal);
        path
    }

    fn enter_container(&mut self, name: &str, end: usize, explicitly_closed: bool) {
        let path = self.next_child_path();
        self.containers.push(Container {
            name: name.to_string(),
            end,
            path,
            explicitly_closed,
        });
        self.child_counts.push(0);
    }

    /// Pop every container the extents say has ended by `close_end` —
    /// positional, so an enclosing `</section>` pops the containers it
    /// implicitly closed without this module ever re-pairing a name.
    ///
    /// One retention rule guards the EOF warnings: a container that was
    /// never explicitly closed and whose extent runs to `source.len()`
    /// (wave 0's contract — unclosed at EOF means `close: None` and
    /// `content_end == html.len()`) is NOT popped here, even by a close
    /// tag whose own end reaches `source.len()`. Without it, a document
    /// whose last byte is a close tag (`…</body>`, `…</html>` — most real
    /// pages) satisfies `c.end <= close_end` as `len <= len` for every
    /// still-open ancestor, pops them all, and the EOF warning loop
    /// iterates an empty vec — the unclosed `<head>` pin would be
    /// guaranteed red and real pages would lose the warning silently. A
    /// container implicitly closed mid-document is untouched by the rule:
    /// its `content_end` sits before `source.len()`, so it still pops
    /// (and, per deviation 4, still pops silently).
    fn pop_closed(&mut self, close_end: usize) {
        while self.containers.last().is_some_and(|c| {
            c.end <= close_end && (c.explicitly_closed || c.end < self.source.len())
        }) {
            self.containers.pop();
            self.child_counts.pop();
        }
    }

    /// Rule T's landing site — **deliberately staged** (see Steps 9–10):
    /// this first landing advances the run state but classifies nothing,
    /// so the spine's byte-identity halves already pass while every
    /// block-set assertion that needs an anonymous run fails with a real
    /// diff. Step 10 replaces this body with the classification.
    fn flush_run(&mut self, boundary: usize) {
        self.seg_start = boundary;
        self.run = RunTags::default();
    }
}

/// Walk `end` back over ASCII whitespace, never past `floor` (the open
/// tag's end): a forced-close range runs "up to the boundary token,
/// trailing whitespace excluded" (spec §4).
fn trim_ascii_ws_back(source: &str, mut end: usize, floor: usize) -> usize {
    let bytes = source.as_bytes();
    while end > floor && end <= bytes.len() && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    end
}
```

  **`pop_closed`'s EOF-retention rule, hand-verified against four shapes** (do not simplify the condition back to `c.end <= close_end` — each shape below breaks a different way without one of its halves):
  1. `<head><title>t</title><body><p>x</p></body>` — the final `</body>` calls `pop_closed(len)`: `body` (`explicitly_closed`, `end == len`) pops; `head` (`!explicitly_closed`, `end == len`) is retained. Expected warnings: **exactly one**, the EOF force-close naming `<head>` — the `a_missing_head_close_does_not_swallow_the_body` pin's assertion, which the unconditioned pop made guaranteed-red.
  2. A well-formed document ending `</html>` with everything closed — every container has `explicitly_closed == true`, so every pop still happens (including the final `len <= len` ones); the EOF loop iterates an empty vec. Expected warnings: **none**.
  3. `<div><p>text</div>` — `<p>` is a *leaf* with `content_end` before `</div>`, so it warns via `emit_extent_leaf`'s mid-document shape; `<div>` is explicitly closed and pops at `</div>`. Expected warnings: **exactly one** (`p-0001 … closed implicitly at byte N`) — behaviour unchanged from the unconditioned pop, per deviation 4.
  4. `<div><p>tail with no closers` (document ends in text) — no close token exists, so `pop_closed` never runs with a boundary at `len`; the leaf warns end-of-input, the retained `<div>` warns in the EOF loop. Expected warnings: **two** — unchanged; the retention rule is not even exercised.

  The general statement: retention triggers exactly on `!explicitly_closed && end == source.len()`, which by wave 0's extent contract (`close: None`, `content_end == html.len()`) characterizes "still open at end of input" — precisely the set deviation 4 promises an EOF warning for, no more (mid-document implicit closes keep `content_end < len` and pop silently) and no less.

- [ ] **Step 8: Declare the module, with its crate-doc clause.** In `crates/transync-syntax/src/lib.rs`, add `pub mod intake;` to the module list (alphabetical — between `id` and `outcome`), and extend the crate doc's first sentence list: `Owns everything syntax-side: GFM parsing into the block IR, the HTML document intake (`intake::html`), block IDs, Markdown regeneration/splicing, alignment-map construction, annotated HTML rendering.`

- [ ] **Step 9: Run the spine against the staged walk — the behavioural red, with the real diffs.** This is the step the compile red of Step 4 could not give: assertions failing on *behaviour*, with byte-identity already green — the live demonstration that identity alone is unfalsifiable and the paired block-set assertions are the falsifiable half.
```bash
cargo test -p transync-syntax --test html_intake_identity -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-red2.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-red2.txt
```
Read the file. Expected: `CARGO_EXIT=101`, `test result: FAILED. 2 passed; 3 failed` — `an_unclosed_fragment_force_closes_with_warnings_and_still_round_trips` and `a_nul_bearing_document_round_trips_to_its_normalized_self` pass (every block they expect is STOP-emitted), while the three tests that need an anonymous run fail on their kind-sequence assertions. The scn-16 failure reads (rule T's `image` block missing, everything else present — note the identity assertion above it already passed):
```
assertion `left == right` failed
  left: ["title", "heading-1", "list-item", "list-item", "heading-2", "paragraph", "blockquote", "heading-2", "table", "code-block", "thematic-break", "html", "paragraph"]
 right: ["title", "heading-1", "list-item", "list-item", "heading-2", "paragraph", "image", "blockquote", "heading-2", "table", "code-block", "thematic-break", "html", "paragraph"]
```
and the `edge_documents_round_trip` failure reads (the tagless file minting nothing):
```
assertion `left == right` failed: a tagless file is one text-heavy block set — D8's explicitly-requested shape
  left: []
 right: ["paragraph"]
```
(`a_messy_marketing_page…` fails the same way: its rule-T `&nbsp;` paragraph is missing from position 4.) Transient `dead_code`-family warnings are expected at this staged point (the `RunTags` fields are written but never read — nothing reads the run in the stub) and disappear in Step 10; no commit happens between, so no gate ever sees them. If anything OTHER than the three kind-sequence assertions fails — an identity mismatch, a panic, a warning-count failure — stop: that is a bug in the staged walk itself (classification, extents plumbing, or `pop_closed`), and it must be fixed before rule T lands on top of it.

- [ ] **Step 10: Complete rule T — replace the stub, add its two helpers and the BOM constant.** In `crates/transync-syntax/src/intake/html.rs`, replace `flush_run`'s staged body (doc comment included) with:
```rust
    /// Rule T (spec §4, as amended by plan deviation 5): the bytes since
    /// the last hard boundary, evaluated as one anonymous run. At least
    /// one non-whitespace char outside tag and `Skip` spans (U+FEFF
    /// stripped) → ONE `Paragraph`; textless with one or more `img`
    /// elements in the run → ONE `Image` block — phrasing wrappers
    /// (`<a>`, `<picture>`, `<span>`) are absorbed markup, so a linked
    /// image keeps its sync anchor instead of dissolving into gap, the
    /// exception's own stated rationale; textless with no `img` at all →
    /// gap: nothing translatable exists, nothing is lost, bytes preserved
    /// verbatim.
    fn flush_run(&mut self, boundary: usize) {
        let start = self.seg_start;
        self.seg_start = boundary;
        let RunTags { skips, tags, imgs } = std::mem::take(&mut self.run);
        if start >= boundary {
            return;
        }
        let (start, end) = trim_run(self.source, start, boundary, &skips);
        if start >= end {
            return;
        }
        if any_naked_char(self.source, start, end, &[tags.as_slice(), skips.as_slice()]) {
            let sp = self.sections.current_path();
            self.emit(BlockKind::Paragraph, sp, ByteRange { start, end });
        } else if !imgs.is_empty() {
            // Textless — every non-whitespace byte is inside an absorbed
            // tag span or a Skip span — and at least one img: ONE Image
            // block, wrappers included (deviation 5's amended reading;
            // §11's two-adjacent-img case verbatim). A textless run with
            // NO img falls through to gap.
            let sp = self.sections.current_path();
            self.emit(BlockKind::Image, sp, ByteRange { start, end });
        }
    }
```
Then add, after the `impl` block and before `trim_ascii_ws_back`:
```rust
/// U+FEFF as UTF-8 — stripped before rule T's byte test and trimmed from
/// run edges: a leading BOM must not mint a paragraph (spec §4).
const BOM: &[u8] = "\u{feff}".as_bytes();

/// Trim leading/trailing ASCII whitespace, U+FEFF, and whole `Skip` spans
/// from a run (spec §4's "range trimmed of leading/trailing whitespace +
/// `Skip` spans + BOM"). Every step crosses ASCII bytes, a whole BOM, or a
/// whole `Skip` span, so the result stays on UTF-8 boundaries.
fn trim_run(
    source: &str,
    mut start: usize,
    mut end: usize,
    skips: &[(usize, usize)],
) -> (usize, usize) {
    let bytes = source.as_bytes();
    loop {
        if start < end && bytes[start].is_ascii_whitespace() {
            start += 1;
        } else if start + BOM.len() <= end && &bytes[start..start + BOM.len()] == BOM {
            start += BOM.len();
        } else if let Some(&(_, e)) = skips.iter().find(|&&(s, e)| s == start && e <= end) {
            start = e;
        } else {
            break;
        }
    }
    loop {
        if end > start && bytes[end - 1].is_ascii_whitespace() {
            end -= 1;
        } else if end >= start + BOM.len() && &bytes[end - BOM.len()..end] == BOM {
            end -= BOM.len();
        } else if let Some(&(s, _)) = skips.iter().find(|&&(s, e)| e == end && s >= start) {
            end = s;
        } else {
            break;
        }
    }
    (start, end)
}

/// Is there a non-whitespace, non-U+FEFF char in `[start, end)` covered by
/// NONE of the span lists? This is rule T's byte test, and the reason
/// `TagToken::Skip` exists: without it a doctype-only gap would mint a
/// phantom paragraph (spec §4, MEASURED).
fn any_naked_char(
    source: &str,
    start: usize,
    end: usize,
    cover: &[&[(usize, usize)]],
) -> bool {
    source[start..end].char_indices().any(|(rel, c)| {
        let at = start + rel;
        let covered = cover
            .iter()
            .any(|spans| spans.iter().any(|&(s, e)| s <= at && at < e));
        !covered && !c.is_whitespace() && c != '\u{feff}'
    })
}
```
Note what the Image arm does NOT do: it never re-tests the naked bytes against the `imgs` spans alone. The run is already known textless (the `Paragraph` arm's test failed), so demanding that the only markup be the `img` tags themselves is exactly the spec-literal reading deviation 5 amends — it would strip the anchor from every `<a href><img></a>`.

- [ ] **Step 11: Land the two guards that go with the module in the same change.**
  - `crates/transync/tests/public_surface.rs`, the `forbidden` array in `hidden_modules_are_not_documented_as_surface`: add `"intake",` after `"parser",` — contracts §0 must never document an `intake::` path; the module is engine-tier and the facade re-exports nothing from it.
  - `docs/architecture/source-of-truth-table.md` — **the `docs_ownership_drift` weld**: in the intro paragraph, extend the enumeration `` `align`, `id`, `outcome`, `parser`, `regen`, `render`, and `walk` live in `crates/transync-syntax` `` to `` `align`, `id`, `intake`, `outcome`, `parser`, `regen`, `render`, and `walk` live in `crates/transync-syntax` (`intake` joined 2026-08-20, ti 490d97 wave 3, as the format seam: `intake::html` is the HTML document intake, and `parser` is the Markdown intake standing where `intake::markdown` will eventually live) ``. Then add one ownership row immediately after the "Block-kind classification" row:
```markdown
| HTML element classification (five classes, default STOP) | `transync-syntax::intake::html` | The one home of spec 2026-08-20 §4's PASS-THROUGH / STOP / VERBATIM / PHRASING / DEFAULT-STOP table and of rule T's anonymous text runs. `transync-html` owns the mechanics under it (tokenizing, pairing, extents) and has no opinion about what a block is. |
```
  - `docs/implementation/module-map.md`: in the `crates/transync-syntax/` tree, insert immediately **after the `├── id.rs` row** and before `├── outcome.rs` — the same alphabetical slot the `pub mod` list takes in Step 8. Do NOT insert against the `parser/` subtree's terminal `│   └── refdefs.rs` row: new sibling rows do not belong directly under another directory's `└──` closer, and the id/outcome slot keeps the drawing's guide columns valid as-is:
```text
    ├── intake.rs                       # the format seam: one intake per source format (D3)
    ├── intake/
    │   └── html.rs                     # HTML document intake: five-class classification,
    │                                   #   rule T, ids/sections/ast_path (ti 490d97 wave 3)
```

- [ ] **Step 12: Run the spine and see it green.**
```bash
cargo test -p transync-syntax --test html_intake_identity -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-green.txt
```
Read the file. Expected: `CARGO_EXIT=0`, `test result: ok. 5 passed`. Failure triage, in the order they bite: an identity mismatch means a range ordering/overlap bug (read the first diverging byte offset); a kind-sequence mismatch on scn-16 means a classification or rule-T bug; a warning-count mismatch means the force-close notes fire too often or not at all. Fix the module, not the test — the expected values above are derived from the spec, and the fixture bytes are frozen as of Step 1.

- [ ] **Step 13: Verify the whole tree — the wasm gate is the live risk — and commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-clippy.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-wasm.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t2-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t2-workspace.txt
```
Expected: `CARGO_EXIT=0` in all three files, and `docs_ownership_drift` + `public_surface` + `docs_index_drift` all green inside the workspace run.
```bash
git add crates/transync-syntax/src/intake.rs crates/transync-syntax/src/intake/html.rs \
        crates/transync-syntax/src/lib.rs crates/transync-syntax/src/parser.rs \
        crates/transync-syntax/src/parser/sections.rs \
        crates/transync-syntax/tests/html_intake_identity.rs \
        crates/transync-syntax/tests/fixtures/html-corpus/marketing-page.html \
        crates/transync-syntax/tests/fixtures/html-corpus/docs-fragment.html \
        crates/transync/tests/fixtures/scn-16-html-document.html \
        crates/transync/tests/public_surface.rs \
        docs/architecture/source-of-truth-table.md docs/implementation/module-map.md
git commit -m "feat(intake): the HTML document intake, and identity is a theorem

transync-syntax::intake::html — the module the transync-html crate-doc has
promised since wave 0. One pass over scan_tags' tokens steered by
element_extents' pairing verdicts: spec §4's five classes (default STOP),
rule T's anonymous runs, whole-element ranges, emission-order ids, shared
SectionStack scopes, comrak-convention ast_paths, force-close warnings.

regenerate(parse, &empty) is byte-identical over the SCN-16 fixture, a messy
marketing page, an unclosed docs fragment, and the CRLF/lone-CR/BOM/NUL/
doctype-only/plain-text edge documents — regenerate copies gaps verbatim and
splices source ranges, so identity holds exactly when the intake's ranges
are ordered, non-overlapping and in bounds, which the intake debug-asserts.

The parser module is NOT renamed to intake::markdown: the seam is kept, the
rename deferred on record (plan deviation 1; DCR-0035 carries the blast
radius). No core file moves; wave 4's layer-6 twin remains the gate before
any HTML translation run.

TRACE: ti 490d97 wave 3
TRACE: ADR-0025

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: Rule T's fine grain and the classification table, pinned element by element

**Files:**
- Modify: `crates/transync-syntax/src/intake/html.rs` (append `#[cfg(test)]` modules only)

**Interfaces:**
- Consumes: Task 2's `parse`. No production code changes in this task; if any pin below is red, the bug is Task 2's and is fixed **there**, with the fix noted in the commit message.
- These are characterization pins, not red-first TDD — said plainly, per the wave-0 Notes precedent: the wave's reds were Task 2's (the Step 4 compile red and the Step 9 behavioural red against the staged walk), and these freeze the fine grain the spine cannot see (the wave-0 Task 2 token-stream-pin precedent). Each names its spec sentence.

- [ ] **Step 1: Append the rule T pins.** In `crates/transync-syntax/src/intake/html.rs`, after the last helper function:
```rust
// Rule T (spec 2026-08-20 §4): anonymous text runs. Each test names the
// sentence it pins.
#[cfg(test)]
mod rule_t_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src).blocks.iter().map(|b| b.kind.wire_str()).collect()
    }

    fn payloads_of(src: &str) -> Vec<String> {
        let doc = parse(src);
        doc.blocks
            .iter()
            .map(|b| doc.source_text[b.source_range.start..b.source_range.end].to_string())
            .collect()
    }

    /// "a maximal run … that contains at least one non-whitespace byte
    /// outside tag and Skip spans … becomes one Paragraph. A textless run
    /// is gap."
    #[test]
    fn whitespace_only_and_bom_only_runs_are_gap() {
        assert!(parse("<div>\n\t \n</div>\n").blocks.is_empty());
        assert!(parse("<div>\u{feff}</div>").blocks.is_empty());
        assert!(parse("\u{feff}\n").blocks.is_empty());
    }

    /// "U+FEFF stripped before the test — a leading BOM must not mint a
    /// paragraph", and the trim excludes it from the range.
    #[test]
    fn a_bom_is_stripped_before_the_test_and_trimmed_from_the_range() {
        let doc = parse("<div>\u{feff}Hello</div>");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(
            &doc.source_text[doc.blocks[0].source_range.start..doc.blocks[0].source_range.end],
            "Hello",
        );
    }

    /// MEASURED in the spec: pre-wave-0, `<!DOCTYPE html>` produced no
    /// token AND no skip — ordinary text to the scanner — so without
    /// `TagToken::Skip` this gap would mint a phantom paragraph. The Skip
    /// span's exclusion from the byte test is the token's reason to exist.
    #[test]
    fn a_doctype_only_gap_mints_no_block_because_skip_spans_are_excluded() {
        assert!(parse("<!DOCTYPE html>\n").blocks.is_empty());
        assert!(parse("<!-- a note -->\n<!-- another -->\n").blocks.is_empty());
        assert_eq!(kinds_of("<!DOCTYPE html>\n<p>x</p>\n"), vec!["paragraph"]);
    }

    /// "a textless run whose only non-whitespace content is ONE OR MORE
    /// img elements becomes ONE Image block" — §11 pins the two-image case
    /// because an earlier draft stated the rule two ways.
    #[test]
    fn one_or_more_imgs_in_a_textless_run_are_one_image_block() {
        assert_eq!(kinds_of("<div><img src=\"a.png\" alt=\"a\"></div>"), vec!["image"]);
        let src = "<div>\n<img src=\"a.png\"> <img src=\"b.png\">\n</div>";
        assert_eq!(kinds_of(src), vec!["image"], "two adjacent imgs: ONE block, not two, not gap");
        assert_eq!(payloads_of(src), vec!["<img src=\"a.png\"> <img src=\"b.png\">"]);
    }

    /// An img beside real text is ordinary phrasing content of a paragraph
    /// — the counter-case that keeps deviation 5's amended exception from
    /// swallowing prose: one naked byte and the run is a Paragraph, linked
    /// or not.
    #[test]
    fn an_img_with_real_text_is_still_a_paragraph() {
        assert_eq!(kinds_of("<div><img src=\"a.png\"> the caption</div>"), vec!["paragraph"]);
        assert_eq!(
            kinds_of("<div><a href=\"/x\"><img src=\"a.png\"></a> the caption</div>"),
            vec!["paragraph"],
        );
    }

    /// Deviation 5's amended reading, justified by spec §4's own rationale
    /// ("so images keep a sync anchor instead of dissolving into gap") and
    /// PHRASING's definition ("never a boundary"): a linked image is ONE
    /// Image block, wrapper included; two adjacent linked images are still
    /// ONE block (the §11 two-image rule); phrasing noise beside an img no
    /// longer degrades the anchor; and a textless run with NO img at all
    /// remains gap.
    #[test]
    fn a_linked_image_keeps_its_anchor_as_one_image_block() {
        let src = "<div><a href=\"/home\"><img src=\"logo.png\" alt=\"logo\"></a></div>";
        assert_eq!(kinds_of(src), vec!["image"]);
        assert_eq!(
            payloads_of(src),
            vec!["<a href=\"/home\"><img src=\"logo.png\" alt=\"logo\"></a>"],
        );
        let two =
            "<div><a href=\"/a\"><img src=\"a.png\"></a> <a href=\"/b\"><img src=\"b.png\"></a></div>";
        assert_eq!(kinds_of(two), vec!["image"], "two adjacent linked images: ONE block");
        assert_eq!(
            kinds_of("<div><b> </b><img src=\"a.png\"></div>"),
            vec!["image"],
            "phrasing noise beside an img keeps the anchor (the pre-amendment reading made this gap)",
        );
        assert!(
            parse("<div><b> </b><span></span></div>").blocks.is_empty(),
            "a textless run with no img at all is still gap",
        );
    }

    /// "without it `Hello <b>world</b> out there` at div level shatters
    /// into three blocks" — the PHRASING class, absorbed into ONE run.
    #[test]
    fn phrasing_markup_is_absorbed_into_one_paragraph() {
        let src = "<div>Hello <b>world</b> out there</div>";
        assert_eq!(kinds_of(src), vec!["paragraph"]);
        assert_eq!(payloads_of(src), vec!["Hello <b>world</b> out there"]);
    }

    /// "RCData counts as text (a bare <textarea> has translatable
    /// content)" — its content is never tokenized, so it is naked bytes.
    #[test]
    fn rcdata_counts_as_text() {
        assert_eq!(kinds_of("<div><textarea>a < b</textarea></div>"), vec!["paragraph"]);
    }

    /// Rule T tests BYTES, not decoded text: `&nbsp;` is six non-whitespace
    /// bytes, so the run mints a paragraph. Whether it holds translatable
    /// segments is extraction's question, in wave 5 — not intake's.
    #[test]
    fn an_entity_spelled_run_is_text_by_the_byte_test() {
        assert_eq!(kinds_of("<div>&nbsp;</div>"), vec!["paragraph"]);
    }

    /// "a custom element mid-sentence still stops (D4 verbatim), so
    /// `Price: <my-price/> today` splits into three blocks — loud rather
    /// than silent" — accepted for now; §14's named review item.
    #[test]
    fn a_custom_element_mid_sentence_still_stops_and_splits_the_sentence() {
        let src = "<div>Price: <my-price/> today</div>";
        assert_eq!(kinds_of(src), vec!["paragraph", "html", "paragraph"]);
        assert_eq!(payloads_of(src), vec!["Price:", "<my-price/>", "today"]);
    }
}
```

- [ ] **Step 2: Append the classification pins — every named element, spelled as the spec spells it.**
```rust
// The five-class table (spec 2026-08-20 §4), pinned per named element.
#[cfg(test)]
mod classification_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src).blocks.iter().map(|b| b.kind.wire_str()).collect()
    }

    /// STOP → semantic kind: h1–h6, p, figcaption, table, pre, blockquote,
    /// hr.
    #[test]
    fn stop_elements_become_their_semantic_kinds() {
        assert_eq!(
            kinds_of("<h1>a</h1><h2>b</h2><h3>c</h3><h4>d</h4><h5>e</h5><h6>f</h6>"),
            vec!["heading-1", "heading-2", "heading-3", "heading-4", "heading-5", "heading-6"],
        );
        assert_eq!(kinds_of("<p>a</p><figcaption>b</figcaption>"), vec!["paragraph", "paragraph"]);
        assert_eq!(kinds_of("<table><tr><td>x</td></tr></table>"), vec!["table"]);
        assert_eq!(kinds_of("<blockquote><p>q</p></blockquote>"), vec!["blockquote"]);
        assert_eq!(kinds_of("<hr>"), vec!["thematic-break"]);
        let doc = parse("<pre>let x = 1;</pre>");
        assert!(
            matches!(&doc.blocks[0].kind, BlockKind::CodeBlock { info: None, fenced: true }),
            "spec §4: pre → CodeBlock {{ info: None, fenced: true }}, got {:?}",
            doc.blocks[0].kind,
        );
    }

    /// PASS-THROUGH, the full list: markup becomes gap, content stops
    /// inside. Each wrapper must contribute zero blocks of its own.
    #[test]
    fn every_pass_through_element_is_gap_around_its_content() {
        for wrapper in [
            "html", "body", "div", "section", "article", "main", "nav", "aside", "header",
            "footer", "hgroup", "figure", "details", "form", "fieldset", "search", "center",
        ] {
            let src = format!("<{wrapper}><p>inner</p></{wrapper}>");
            assert_eq!(kinds_of(&src), vec!["paragraph"], "for <{wrapper}>");
        }
    }

    /// D9: `ul`, `ol`, `menu`, `dl` pass through TO THEIR ITEMS. An
    /// `<ol>`'s items are ordered; `dt`/`dd` are DEFAULT-STOP; an `<li>`
    /// that is not a list child is nobody's semantic kind.
    #[test]
    fn list_containers_pass_through_to_their_items() {
        let doc = parse("<ul><li>a</li><li>b</li></ul>");
        assert!(
            doc.blocks
                .iter()
                .all(|b| matches!(b.kind, BlockKind::ListItem { ordered: false, task: None })),
            "{:?}",
            doc.blocks.iter().map(|b| &b.kind).collect::<Vec<_>>(),
        );
        let doc = parse("<ol><li>a</li></ol>");
        assert!(matches!(doc.blocks[0].kind, BlockKind::ListItem { ordered: true, task: None }));
        let doc = parse("<menu><li>a</li></menu>");
        assert!(matches!(doc.blocks[0].kind, BlockKind::ListItem { ordered: false, task: None }));
        assert_eq!(kinds_of("<dl><dt>t</dt><dd>d</dd></dl>"), vec!["html", "html"]);
        assert_eq!(kinds_of("<div><li>stray</li></div>"), vec!["html"]);
    }

    /// DEFAULT-STOP → BlockKind::Html: summary, dt, dd, address, dialog,
    /// iframe, noscript, template, legend, custom elements. `<template>`
    /// needs no special case — it stops (zero segments is wave 5's story).
    #[test]
    fn the_default_stop_set_stops_as_block_kind_html() {
        for src in [
            "<summary>s</summary>",
            "<address>a</address>",
            "<dialog>d</dialog>",
            "<iframe src=\"x\"></iframe>",
            "<noscript><p>n</p></noscript>",
            "<template><p>t</p></template>",
            "<legend>l</legend>",
            "<x-widget>custom</x-widget>",
        ] {
            assert_eq!(kinds_of(src), vec!["html"], "for {src}");
        }
    }

    /// VERBATIM: script, style, link, meta, base — never a block, bytes
    /// stay gap, and "script content cannot reach a run because script is
    /// VERBATIM and a hard boundary".
    #[test]
    fn verbatim_elements_are_never_blocks() {
        assert_eq!(
            kinds_of("<script>if (a < b) { emit(\"</div>\"); }</script>\n<p>x</p>"),
            vec!["paragraph"],
        );
        assert_eq!(kinds_of("<style>main > p { color: red }</style>\n<p>x</p>"), vec!["paragraph"]);
        assert_eq!(
            kinds_of("<link rel=\"a\" href=\"b\">\n<meta name=\"c\">\n<base href=\"/\">\n<p>x</p>"),
            vec!["paragraph"],
        );
    }
}
```

- [ ] **Step 3: Append the head-mode and `<title>` pins.**
```rust
// Head mode and the D5 title (spec §4's <title> paragraph).
#[cfg(test)]
mod head_tests {
    use super::*;

    fn kinds_of(src: &str) -> Vec<&'static str> {
        parse(src).blocks.iter().map(|b| b.kind.wire_str()).collect()
    }

    /// "title in head mode → Title", with an EMPTY section_path — a page
    /// title opens no scope, and no glossary section-selector can mean it.
    #[test]
    fn the_title_stops_only_in_head_mode_and_opens_no_scope() {
        let doc = parse("<head><title>Page &amp; title</title></head><body><p>x</p></body>");
        assert_eq!(doc.blocks[0].kind.wire_str(), "title");
        assert!(doc.blocks[0].section_path.is_empty());
        assert_eq!(doc.blocks[1].kind.wire_str(), "paragraph");
        // Outside head mode the same element is DEFAULT-STOP:
        assert_eq!(kinds_of("<title>loose</title>"), vec!["html"]);
        // <svg><title> is phrasing-held run content — never Title:
        assert_eq!(
            kinds_of("<div><svg><title>chart</title></svg> labelled</div>"),
            vec!["paragraph"],
        );
    }

    /// "everything in head mode except title" is VERBATIM gap, and "a
    /// duplicate title yields two honest rows".
    #[test]
    fn everything_else_in_head_is_gap_and_a_duplicate_title_is_two_honest_blocks() {
        let doc = parse(
            "<head><meta charset=\"utf-8\"><style>p{}</style><title>a</title><title>b</title></head>",
        );
        assert_eq!(
            doc.blocks.iter().map(|b| b.kind.wire_str()).collect::<Vec<_>>(),
            vec!["title", "title"],
        );
    }

    /// Plan deviation 3: the pairing table has no head-closes-at-body rule,
    /// so on a page that omits </head> the MODE ends at the first <body>
    /// open — the body must not dissolve into head-mode gap.
    #[test]
    fn a_missing_head_close_does_not_swallow_the_body() {
        let doc = parse("<head><title>t</title><body><p>real content</p></body>");
        assert_eq!(
            doc.blocks.iter().map(|b| b.kind.wire_str()).collect::<Vec<_>>(),
            vec!["title", "paragraph"],
        );
        assert!(
            doc.warnings.iter().any(|w| w.contains("<head>")),
            "the truly-unclosed head is still reported at EOF: {:?}",
            doc.warnings,
        );
    }

    /// Rule T must not leak into head mode at EOF: a page truncated inside
    /// an unclosed <head> — exactly the shape a cut-off download produces —
    /// ends with stray head text, and that tail is head-mode VERBATIM gap
    /// (spec §4: everything in head mode except title), never a Paragraph.
    /// Byte-identity survives either way; the BLOCK SET is what this pins.
    #[test]
    fn trailing_text_inside_an_unclosed_head_is_gap_not_a_paragraph() {
        let doc = parse("<head><title>t</title>junk");
        assert_eq!(
            doc.blocks.iter().map(|b| b.kind.wire_str()).collect::<Vec<_>>(),
            vec!["title"],
            "the trailing head text must not mint a paragraph: {:?}",
            doc.warnings,
        );
        let (out, _) = crate::regen::regenerate(&doc, &std::collections::HashMap::new());
        assert_eq!(out, doc.source_text, "the tail stays verbatim gap");
        assert!(
            doc.warnings.iter().any(|w| w.contains("<head>")),
            "the unclosed head still warns at EOF: {:?}",
            doc.warnings,
        );
    }
}
```

- [ ] **Step 4: Run the three new modules and the whole crate.**
```bash
cargo test -p transync-syntax rule_t_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t3-rulet.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t3-rulet.txt
cargo test -p transync-syntax -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t3-crate.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t3-crate.txt
```
Expected: `CARGO_EXIT=0` in both; the first run shows `10 passed` for `rule_t_tests` (nine spec-sentence pins plus the linked-image pin), and the crate run includes `head_tests`' four (the fourth is the trailing-head-text EOF pin). **Expected to pass on arrival, and stated plainly as characterization** — these pin behaviour Task 2 built. A red here is a Task 2 defect: fix it in `intake/html.rs`, re-run the Task 2 spine to confirm it still holds, and name the fix in this task's commit body.

- [ ] **Step 5: Commit.**
```bash
git add crates/transync-syntax/src/intake/html.rs
git commit -m "test(intake): rule T's fine grain and the five-class table, pinned per element

Every element the spec names in every class appears here spelled the same:
the 17 pass-through wrappers, the D9 list containers, the five VERBATIM
names, the eight named DEFAULT-STOPs plus a custom element, and the STOP
table. Rule T's corners each get their own pin: the whitespace-only test,
the BOM strip, the Skip-span exclusion (the doctype MEASURED case — the
reason the token exists), the img exception in its amended reading (plan
deviation 5: a linked image keeps its anchor, the §11 two-image case
verbatim, a textless img-less run stays gap), RCData-as-text, the
entity-bytes run, the head-EOF tail staying gap (rule T never runs in head
mode, end of input included), and the custom-element sentence split that
§14 carries as a review item.

TRACE: ti 490d97 wave 3

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: Warnings, genuinely broken input, and the debug-asserted invariants

**Files:**
- Modify: `crates/transync-syntax/src/intake/html.rs` (append `#[cfg(test)]` modules only)

**Interfaces:**
- Consumes: Task 2's `parse` and `debug_assert_block_invariants`.
- Like Task 3, these are characterization pins over behaviour Task 2 built — stated plainly, not dressed as red-first. The `#[should_panic]` trio is the exception in kind: it is falsifiable by construction (a wrong or vague assertion message fails the `expected` match).
- The invariant-trip tests are the spec's "debug assertion pins order/non-overlap at intake" made checkable: `Document` is pub-fields, so a bad document is constructible, and the assertion must NAME the block and the violation.

- [ ] **Step 1: Append the warning pins.**
```rust
// "Implicit closes, force-closes on unclosed leaves, and EOF force-close
// each append a Document::warnings note — that channel is documented as
// not parse-only." (spec §4)
#[cfg(test)]
mod warning_tests {
    use super::*;

    /// An implicit close (the second <p> pops the first) and a force-close
    /// by the enclosing </div> each leave a note naming the block.
    #[test]
    fn an_implicit_close_and_a_force_close_each_leave_a_note() {
        let doc = parse("<div><p>alpha<p>bravo</div>");
        assert_eq!(doc.blocks.len(), 2);
        assert_eq!(doc.warnings.len(), 2, "{:?}", doc.warnings);
        assert!(
            doc.warnings[0].contains("p-0001") && doc.warnings[0].contains("closed implicitly"),
            "{}",
            doc.warnings[0],
        );
        assert!(doc.warnings[1].contains("p-0002"), "{}", doc.warnings[1]);
        // The force-closed ranges exclude trailing whitespace and the
        // boundary tokens (spec §4 "ranges"):
        let p1 = &doc.source_text[doc.blocks[0].source_range.start..doc.blocks[0].source_range.end];
        assert_eq!(p1, "<p>alpha");
    }

    /// EOF force-close: the leaf and the still-open container are each
    /// reported, and nothing panics.
    #[test]
    fn eof_force_close_warns_for_the_leaf_and_the_container() {
        let doc = parse("<div><p>tail with no closers");
        assert_eq!(doc.blocks.len(), 1);
        assert_eq!(doc.blocks[0].kind.wire_str(), "paragraph");
        assert!(
            doc.warnings.iter().any(|w| w.contains("p-0001") && w.contains("end of input")),
            "{:?}",
            doc.warnings,
        );
        assert!(
            doc.warnings.iter().any(|w| w.contains("<div>") && w.contains("end of input")),
            "{:?}",
            doc.warnings,
        );
    }

    /// Spec §12 wave 3 acceptance: "an unclosed fragment force-closing with
    /// a warning rather than a panic" — generalized to genuinely hostile
    /// shapes. Identity must survive every one of them, because identity is
    /// a range-bookkeeping property, not a well-formedness property.
    #[test]
    fn genuinely_broken_input_parses_without_panicking_and_round_trips() {
        for src in [
            "</p></div><p>orphans first",
            "<p><div></span><p>text",
            "<ul><li><table><tr>x",
            "<<<>>><p>&</p>",
            "<title>rcdata swallows <div> everything",
            "<head><head><title>t</title>",
        ] {
            let doc = parse(src);
            let (out, _) = crate::regen::regenerate(&doc, &std::collections::HashMap::new());
            assert_eq!(out, doc.source_text, "for {src:?}");
        }
    }
}
```

- [ ] **Step 2: Append the invariant-trip pins — the hand-built bad `Document`s.**
```rust
// The spec's intake tripwire ("A debug assertion pins order/non-overlap at
// intake"), pinned by tripping it: Document is pub-fields, so the bad
// documents below are constructible outside parse, and the assertion must
// name the block and the violation.
#[cfg(test)]
mod invariant_tests {
    use super::*;

    fn block(ordinal: u32, start: usize, end: usize) -> Block {
        Block {
            block_id: BlockId::new("p", ordinal),
            kind: BlockKind::Paragraph,
            spelling: Spelling::Html { block_type: None },
            source_range: ByteRange { start, end },
            source_hash: 0,
            section_path: Vec::new(),
            ast_path: AstPath(vec![0]),
        }
    }

    fn doc_with(source: &str, blocks: Vec<Block>) -> Document {
        Document {
            source_text: source.to_string(),
            format: SourceFormat::Html,
            blocks,
            warnings: Vec::new(),
            ref_defs: String::new(),
        }
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "inside or before its predecessor")]
    fn the_assertion_trips_on_overlapping_ranges() {
        debug_assert_block_invariants(&doc_with(
            "abcdefgh",
            vec![block(1, 0, 5), block(2, 3, 8)],
        ));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "splits a UTF-8 char")]
    fn the_assertion_trips_on_a_non_boundary_offset() {
        // Byte 2 is inside the three-byte '한'.
        debug_assert_block_invariants(&doc_with("a한글b", vec![block(1, 0, 2)]));
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "past the end of source_text")]
    fn the_assertion_trips_on_an_out_of_bounds_range() {
        debug_assert_block_invariants(&doc_with("short", vec![block(1, 0, 99)]));
    }

    /// The assertion is quiet on what parse actually builds — otherwise
    /// every test above this line would already have tripped it, but say
    /// so explicitly once.
    #[test]
    fn the_assertion_is_quiet_on_parse_output() {
        let doc = parse("<h1>a</h1><p>b</p><ul><li>c</li></ul>");
        debug_assert_block_invariants(&doc);
        assert_eq!(doc.blocks.len(), 3);
    }
}
```

- [ ] **Step 3: Run the two new modules.**
```bash
cargo test -p transync-syntax warning_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t4-warnings.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t4-warnings.txt
cargo test -p transync-syntax invariant_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t4-invariants.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t4-invariants.txt
```
Expected: `CARGO_EXIT=0` in both — `3 passed` for `warning_tests`, `4 passed` for `invariant_tests` (the three `should_panic` tests *pass by panicking*; `cargo test` builds with debug assertions on, so the `#[cfg(debug_assertions)]` gates are active).

- [ ] **Step 4: Run the whole crate, then commit.**
```bash
cargo test -p transync-syntax -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t4-crate.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t4-crate.txt
```
Expected: `CARGO_EXIT=0`.
```bash
git add crates/transync-syntax/src/intake/html.rs
git commit -m "test(intake): warnings not panics, and the invariant assertion trips by name

The three warning cases the spec names — implicit close, force-close on an
unclosed leaf, EOF force-close — each leave a Document::warnings note naming
the block; hostile input (orphan closes, mis-nesting, raw-text swallowing
the document, a doubled <head>) parses without panicking and still
round-trips byte-identical, because identity is range bookkeeping, not
well-formedness. The debug assertion is tripped three ways on hand-built
bad Documents — overlap, split UTF-8 char, out-of-bounds — and each panic
message names the offending block, which is what makes the tripwire worth
shipping.

TRACE: ti 490d97 wave 3

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: Ids, sections, paths, and the alignment map over a parsed page

**Files:**
- Create: `crates/transync-syntax/tests/html_intake_alignment.rs`
- Modify: `crates/transync-syntax/src/intake/html.rs` (append one `#[cfg(test)]` module)

**Interfaces:**
- Consumes: `crate::id::assign_block_ids`, `crate::align::build_alignment_map`, `crate::outcome::html_outcomes` (post-wave-2 it iterates spelling-Html blocks — on an HTML document that is *every* block, and `extract` runs host-side without complaint), `crate::walk::normalize_top_level`, `crate::regen::regenerate`.
- This task is the wave's "**with real ids and a real alignment map**" leg: nothing new is implemented; the existing downstream machinery is driven by the new intake's output and pinned. Everything here is a pin (pass-on-arrival expected; a red is a Task 2 bug, fixed there).

- [ ] **Step 1: Append the section-scope and `ast_path` pins.** In `crates/transync-syntax/src/intake/html.rs`:
```rust
// Ids, sections and paths (spec §4 "Ids, hashes, paths").
#[cfg(test)]
mod id_and_path_tests {
    use super::*;

    /// "opening/closing SectionStack scopes exactly as Markdown headings
    /// do" — same close-through-then-open rule, same section_path shapes.
    #[test]
    fn headings_open_and_close_section_scopes_exactly_as_markdown_headings_do() {
        let doc = parse("<h1>A</h1><p>one</p><h2>B</h2><p>two</p><h1>C</h1><p>three</p>");
        let paths: Vec<Vec<&str>> = doc
            .blocks
            .iter()
            .map(|b| b.section_path.iter().map(|id| id.0.as_str()).collect())
            .collect();
        assert_eq!(
            paths,
            vec![
                vec![],
                vec!["h1-0001"],
                vec!["h1-0001"],
                vec!["h1-0001", "h2-0003"],
                vec![],
                vec!["h1-0005"],
            ],
        );
    }

    /// "an <li> takes its list container's path + item ordinal, exactly the
    /// Markdown List-arm convention" — the prefix is shared within one list
    /// and differs across adjacent lists.
    #[test]
    fn list_items_carry_the_container_path_plus_their_ordinal() {
        let doc = parse("<ul><li>a</li><li>b</li></ul><ul><li>c</li></ul>");
        let paths: Vec<&[usize]> = doc.blocks.iter().map(|b| &b.ast_path.0[..]).collect();
        assert_eq!(paths, vec![&[0, 0][..], &[0, 1][..], &[1, 0][..]]);
    }

    /// "the same global-ordinal scheme … the pipeline's existing
    /// assign_block_ids is idempotent over it".
    #[test]
    fn assign_block_ids_is_idempotent_over_the_html_intake() {
        let mut doc = parse("<h1>t</h1><p>a</p><ul><li>b</li></ul><hr>");
        let before: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert_eq!(before, vec!["h1-0001", "p-0002", "li-0003", "hr-0004"]);
        crate::id::assign_block_ids(&mut doc);
        let after: Vec<String> = doc.blocks.iter().map(|b| b.block_id.0.clone()).collect();
        assert_eq!(before, after);
    }

    /// The hash covers the WHOLE element, attributes included, "so
    /// <p class=\"a\">x</p> and <p>x</p> cannot alias in the cache"
    /// (spec §4 "ranges").
    #[test]
    fn the_source_hash_covers_the_whole_element_attributes_included() {
        let a = parse("<p class=\"a\">x</p>");
        let b = parse("<p>x</p>");
        assert_ne!(a.blocks[0].source_hash, b.blocks[0].source_hash);
        assert_eq!(
            a.blocks[0].source_hash,
            crate::id::source_hash_bytes("<p class=\"a\">x</p>".as_bytes()),
        );
    }
}
```

- [ ] **Step 2: Write the alignment integration test.** Create `crates/transync-syntax/tests/html_intake_alignment.rs`:
```rust
//! Wave 3's "real ids and a real alignment map" leg (spec §12): the
//! existing downstream machinery — outcome, regen offsets, align, walk —
//! driven by the HTML intake's output, with the D5 title row's `non-sync`
//! role pinned end to end. No pipeline, no LLM, no core.

use std::collections::HashMap;
use transync_syntax::align::{self, SyncRole};
use transync_syntax::{id, intake, outcome, regen, walk};

const SCN_16: &str = include_str!("../../transync/tests/fixtures/scn-16-html-document.html");

#[test]
fn a_parsed_html_document_gets_a_full_alignment_map_with_a_non_sync_title_row() {
    let doc = intake::html::parse(SCN_16);
    let (out, offsets) = regen::regenerate(&doc, &HashMap::new());
    assert_eq!(out, doc.source_text);

    let outcomes = outcome::html_outcomes(&doc);
    let map = align::build_alignment_map(&doc, &HashMap::new(), &offsets, "en", "ko", None, &outcomes);

    assert_eq!(map.blocks.len(), doc.blocks.len(), "a row for every block");
    assert_eq!(
        map.document_id,
        format!("{:016x}", id::source_hash_bytes(doc.source_text.as_bytes())),
        "document_id stays source_hash_bytes(source_text) (spec §4)",
    );

    // The D5 title row: block_kind "title", sync_role non-sync, REAL ranges.
    let title = &map.blocks[0];
    assert_eq!(title.block_kind, "title");
    assert_eq!(title.sync_role, SyncRole::NonSync, "translated, aligned, anchor-less (D5)");
    assert_eq!(title.source_range, doc.blocks[0].source_range);
    assert_eq!(
        title.target_range, doc.blocks[0].source_range,
        "identity regen: target ranges mirror source ranges byte for byte",
    );

    // Every row's target range is real — the empty-range fallback would
    // mean regenerate skipped a block.
    for row in &map.blocks {
        assert!(
            row.target_range.end > row.target_range.start,
            "{}: empty target_range",
            row.source_block_id.0,
        );
    }

    // The thematic break keeps its schema-1.0 non-sync shape; everything
    // anchored stays anchored.
    let hr = map.blocks.iter().find(|r| r.block_kind == "thematic-break").expect("hr row");
    assert_eq!(hr.sync_role, SyncRole::NonSync);
    let h1 = map.blocks.iter().find(|r| r.block_kind == "heading-1").expect("h1 row");
    assert_eq!(h1.sync_role, SyncRole::Anchor);
}

#[test]
fn the_prefix_collapse_is_total_over_html_list_items() {
    // walk::normalize_top_level's collapse rule reads ast_path prefixes;
    // the intake's comrak-convention paths must keep it total (spec §4):
    // same-list items collapse, adjacent lists never merge.
    let doc = intake::html::parse("<ul><li>a</li><li>b</li></ul><ul><li>c</li></ul>");
    let entries = walk::normalize_top_level(&doc);
    let shape: Vec<(&str, usize)> = entries.iter().map(|e| (e.label, e.sources.len())).collect();
    assert_eq!(shape, vec![("list", 2), ("list", 1)]);
}

#[test]
fn the_docs_fragment_normalizes_with_one_three_item_list_entry() {
    let doc = intake::html::parse(include_str!("fixtures/html-corpus/docs-fragment.html"));
    let entries = walk::normalize_top_level(&doc);
    let shape: Vec<(&str, usize)> = entries.iter().map(|e| (e.label, e.sources.len())).collect();
    assert_eq!(
        shape,
        vec![
            ("heading-1", 1),
            ("paragraph", 1),
            ("list", 3),
            ("code-block", 1),
            ("html", 1),
            ("html", 1),
            ("paragraph", 1),
        ],
    );
}
```

- [ ] **Step 3: Run both new test surfaces.**
```bash
cargo test -p transync-syntax --test html_intake_alignment -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t5-align.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t5-align.txt
cargo test -p transync-syntax id_and_path_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t5-paths.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t5-paths.txt
```
Expected: `CARGO_EXIT=0` in both — `3 passed` for the alignment file, `4 passed` for `id_and_path_tests`.

- [ ] **Step 4: Run the whole workspace and the wasm gate.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t5-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t5-workspace.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave3/gate/t5-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t5-wasm.txt
```
Expected: `CARGO_EXIT=0` in both.

- [ ] **Step 5: Commit.**
```bash
git add crates/transync-syntax/src/intake/html.rs crates/transync-syntax/tests/html_intake_alignment.rs
git commit -m "test(intake): real ids, real sections, real paths, and a real alignment map

The wave's third claim, driven end to end through the EXISTING machinery:
build_alignment_map over a parsed HTML page yields a row for every block
with real byte ranges on both sides (identity regen makes them mirrors),
document_id stays source_hash_bytes(source_text), and the D5 title row is
block_kind 'title' with sync_role non-sync — wave 2's explicit arm, now
exercised by a producer. SectionStack scopes match the Markdown intake's
shapes, li ast_paths carry the container prefix so normalize_top_level's
collapse stays total and adjacent lists never merge, assign_block_ids is
idempotent over emission-order stamping, and the whole-element hash keeps
attribute-differing blocks from aliasing.

TRACE: ti 490d97 wave 3

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: Records — DCR-0035, the index links, and the routine per-wave files

**Files:**
- Create: `docs/project/design-change-records/DCR-0035-html-intake-identity-round-trip.md`
- Modify: `docs/index.md`, `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`, `CLAUDE.md`

**Interfaces:**
- Consumes: the finished wave. Produces: the record set spec §10 assigns per landed wave ("DCR per landed wave, starting at DCR-0032"; routine: CHANGELOG, status, phase-state, module map — the module map landed with its weld in Task 2).

- [ ] **Step 1: Confirm the DCR number is still free — wave 4 runs in parallel and takes numbers from the same sequence.**
```bash
ls docs/project/design-change-records/ | grep -c 'DCR-0035'
```
Expected: `0` (note: `grep -c` exits 1 on a zero count — that is the *good* outcome here). If it prints `1` or more, wave 4 landed first: renumber this task's file, link text and cross-references to the next free number (`DCR-0036`, checked the same way) — the content below does not change.

- [ ] **Step 2: Write the DCR.** Create `docs/project/design-change-records/DCR-0035-html-intake-identity-round-trip.md`, opening with the house OKF frontmatter (`type: DCR`, `tags: [change, project-control, DCR-0035]`, matching DCR-0034's field set exactly — copy its frontmatter shape, not its text). Body sections, in the house order (What changed / Why / Evidence / Consequences):
  - **What changed:** `transync-syntax::intake::html` — the HTML document intake (spec §4): five-class classification with default STOP, rule T anonymous runs, whole-element ranges, emission-order ids, shared `SectionStack` scopes, comrak-convention `ast_path`s, `<title>` blocks in head mode, force-close warnings, and the debug-asserted source-order/non-overlap invariants. Two crate-internal visibility widenings (`parser::normalize_source`, `parser::sections`) so both intakes share one NUL rule and one heading-scope stack. New fixtures: `scn-16-html-document.html` (shared forward to wave 5) and the two-page identity corpus.
  - **Why:** wave 3 of ti `490d97` — the first demonstrable milestone: an HTML document goes in and comes out byte-identical, with a real block set, real ids and a real alignment map; no LLM, no pipeline change, no core change.
  - **The deferred rename, on record:** spec §4 sketches today's `parser` as `intake::markdown` and delegates the path decision to the implementation plan. The rename is deferred: ~43 `crate::parser` references in `transync-syntax`, ~74 in `transync-core` (including the crate-private re-export), `transync-wasm`'s import, `public_surface.rs`'s pin, and four documents would move for zero behaviour. The seam is real without it (same `Document`, same `normalize_source`, same id scheme, same `SectionStack`); the asymmetric shape (`intake::html` beside `parser`) is recorded here and in `intake.rs`'s module doc so it reads as a decision, not an accident. A future wave that renames pays the mechanical cost in a diff that is *only* that.
  - **The amended img exception, on record (plan deviation 5):** spec §4's literal wording — "a textless run whose only non-whitespace content is one or more `img` elements" — would classify `<a href="…"><img …></a>` as gap and silently strip the sync anchor and alignment row from every linked badge, logo and thumbnail. The wave implements the reading consistent with the spec's own stated rationale ("so images keep a sync anchor instead of dissolving into gap") and with PHRASING's "never a boundary": textless after excluding absorbed phrasing markup, one or more `img` elements → one `Image` block. Pinned by `a_linked_image_keeps_its_anchor_as_one_image_block` / `an_img_with_real_text_is_still_a_paragraph`. **Post-implementation review item (§14-style): amend spec §4's exception sentence to match the implemented reading.** The spec file was deliberately not edited in this wave; the amendment is recorded here as owed.
  - **Evidence:** the Task 2/5 gate files (identity over the fixture + corpus + edge documents; the alignment map with the `non-sync` title row; the wasm gate exit 0), the Task 4 no-panic pins.
  - **Consequences / hand-forwards:** the layer-6 twin gates any translation run (wave 4, spec §7 hard rule); `html_outcomes`/units/prompt over HTML documents (wave 5); the alignment wire's `source_format`/`input_format` fields, `--input-format`, `out.html`, and the refusal re-text (wave 6); Playwright SCN-16 (wave 7). `html_dominance_warning`'s "not implemented (ti 490d97)" tail is still TRUE after this wave and is re-texted in the wave the entry point lands (spec §6).

- [ ] **Step 3: Link the DCR in `docs/index.md`, in this same change.** After the DCR-0034 line (or after the highest-numbered DCR line present, keeping numeric order):
```markdown
- [DCR-0035 — HTML intake + identity round-trip (ti 490d97 wave 3): `transync-syntax::intake::html` lands beside `parser`, and `regenerate(parse, &empty)` is byte-identical](project/design-change-records/DCR-0035-html-intake-identity-round-trip.md)
```

- [ ] **Step 4: CHANGELOG.** Under `## [Unreleased]`'s `### Added` (create the subsection if the waves before this one left none), append:
```markdown
- HTML document intake (`transync-syntax::intake::html`, ti `490d97` wave 3, spec §4): five-class element classification with default STOP, rule T anonymous text runs, whole-element byte ranges, emission-order ids, heading section scopes, `<title>` blocks, force-close warnings, and debug-asserted source-order/non-overlap invariants. `regen::regenerate(parse, &empty)` is byte-identical over the SCN-16 fixture and a real-page corpus. No LLM run, no pipeline change, no core change — the layer-6 HTML twin (wave 4) remains the gate before any HTML translation run.
```
If waves 0–2 left the `[Unreleased]` section in a different shape than expected, the rule is the same: this entry lands under `### Added`, after theirs, and the window paragraph is not touched.

- [ ] **Step 5: `docs/project/status.md` and `docs/project/phase-state.yaml`.** Record wave 3 as landed in exactly the shape waves 0–2 recorded theirs (a per-wave line/entry under the ti `490d97` work record): wave 3 — HTML intake + identity round-trip, DCR-0035, demonstrable outcome "HTML in, byte-identical out, with blocks/ids/alignment map". Do not mark the feature further along than it is: waves 4–7 remain open, and any "next action" line must still name wave 4's twin as the gate.

- [ ] **Step 6: `CLAUDE.md` — the module-split list.** Under **Layout** → "Module split in `crates/transync-syntax`", add two rows after the `parser` row, matching the list's indentation and voice:
```markdown
  - `intake` — the format seam (D3): one intake per source format
  - `intake/html` — THE HTML document intake: five-class classification (default STOP), rule T text runs, whole-element ranges, ids/sections/ast_path; the Markdown intake remains `parser` pending its `intake::markdown` rename (DCR-0035)
```

- [ ] **Step 7: Verify every docs weld, bare-to-file.**
```bash
cargo test -p transync --test docs_index_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t6-index.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t6-index.txt
cargo test -p transync --test docs_ownership_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave3/gate/t6-ownership.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/t6-ownership.txt
```
Expected: `CARGO_EXIT=0` in both.

- [ ] **Step 8: Commit the records.**
```bash
git add docs/project/design-change-records/DCR-0035-html-intake-identity-round-trip.md \
        docs/index.md CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml CLAUDE.md
git commit -m "docs(records): DCR-0035 — wave 3 landed, and the rename it deliberately did not do

The record carries the wave's three claims (byte-identity, real block set,
real alignment map), its hand-forwards (twin, units, wire, CLI, browser),
and the deferred parser→intake::markdown rename with its measured blast
radius, so the asymmetric module shape reads as a decision. The DCR's index
link rides the commit that creates it.

TRACE: ti 490d97 wave 3
TRACE: DCR-0035

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Wave acceptance — check all five before declaring wave 3 done

Spec §12 wave 3's acceptance items, plus this plan's own gates, each with its evidence:

1. **`regenerate(parse_html(fixture), &empty)` byte-identical over the SCN-16 fixture and a real-page corpus** — `html_intake_identity.rs`, all five tests green in `t2-green.txt` / re-confirmed in `t5-workspace.txt`.
2. **The debug-asserted source-order/non-overlap invariants** — `debug_assert_block_invariants` runs on every `parse`, and `invariant_tests` trips it three ways by name (`t4-invariants.txt`).
3. **A `<!doctype>`-only gap minting NO block** — `edge_documents_round_trip` + `a_doctype_only_gap_mints_no_block_because_skip_spans_are_excluded`.
4. **An unclosed fragment force-closing with a warning rather than a panic** — the docs-fragment corpus test + `warning_tests` (`t4-warnings.txt`).
5. **This wave enabled no translation run and changed no core file.** Verify mechanically against the Task 1 baseline:
```bash
git diff --stat "$(cat /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-commit.txt)"..HEAD -- crates/transync-core crates/transync-cli crates/transync-openai crates/transync-anthropic crates/transync-wasm crates/transync-html web > /Volumes/Temp/claude/ti490d97-wave3/gate/accept-no-core.txt 2>&1
echo "GIT_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave3/gate/accept-no-core.txt
```
Expected: an **empty diff** (`GIT_EXIT=0`, no file lines). Any hunk here is a plan violation — wave 4 owns core's validate/finalize, wave 5 owns units/prompt, wave 6 owns the CLI. Also confirm zero edits to pre-existing fixtures:
```bash
git diff --name-only "$(cat /Volumes/Temp/claude/ti490d97-wave3/gate/baseline-commit.txt)"..HEAD -- 'crates/*/tests/fixtures' > /Volumes/Temp/claude/ti490d97-wave3/gate/accept-fixtures.txt 2>&1
```
Expected: exactly the three **new** files (`scn-16-html-document.html`, `marketing-page.html`, `docs-fragment.html`) and nothing else.

Plus the standing gates, all bare-to-file in the task steps above: the two-package wasm gate exit 0; `cargo test --workspace -- --test-threads=4` green; `docs_index_drift`, `docs_ownership_drift`, `public_surface` green.

## Spec §4 coverage map

| Spec §4 element | Where in this plan |
|---|---|
| Module `intake::html` beside the Markdown intake; path details the plan's | Task 2 Steps 6–10; deviation 1 (rename deferred, recorded) |
| Same `normalize_source` (NUL → U+FFFD) | Task 2 Step 5 widening; `a_nul_bearing_document_round_trips_to_its_normalized_self` |
| Classification table, five classes, default STOP, every element transcribed | Task 2 Step 7 consts + `semantic_stop_kind`; Task 3 Step 2 pins each named element |
| One stack walk over `element_extents` + tokens, single pairing discipline; STOP pushes a leaf root; nothing emitted inside one | Task 2 Step 7 `HtmlWalk::walk` (positional `cursor`/`pop_closed` — the extents' verdicts, never re-paired) |
| Implicit closes / force-closes / EOF each warn | Task 2 `emit_extent_leaf` + `pop_closed`'s EOF-retention rule (hand-verified against four shapes, Step 7) + walk EOF loop; Task 4 Step 1 pins; deviation 4 scopes containers |
| Rule T in full: hard boundaries, byte test, BOM strip, Skip exclusion, trim, img exception, RCData-as-text, script unreachable, never in head mode (EOF included) | Task 2 Step 10's `flush_run`/`trim_run`/`any_naked_char` + the Step 7 head-mode EOF guard; Task 3 Step 1's ten pins + Step 3's head-EOF pin; deviation 5 (the amended img exception) and deviation 6 |
| Ranges = whole element, tags included; void = tag span; runs trimmed; forced closes trailing-ws-excluded | Task 2 `emit_extent_leaf` + `hr` arm + `trim_ascii_ws_back`; payload assertions in Tasks 2–4 |
| Ids: emission-order, one global ordinal; `assign_block_ids` idempotent; `document_id = source_hash_bytes(source_text)` | Task 2 `emit`; Task 5 Steps 1–2 pins |
| `ast_path` mirrors comrak's child-index convention; prefix collapse total; adjacent lists never merge | Task 2 `next_child_path`; Task 5 collapse pins |
| Regen's three requirements provable at intake + debug assertion | Task 2 `debug_assert_block_invariants`; Task 4 Step 2 trips it |
| `<title>`: head mode only, source order, empty `section_path`, duplicate = two rows, `<svg><title>` never Title; the `non-sync` row | Task 3 Step 3; Task 5 Step 2 (row-level pin) |
| No line table anywhere → OI-0033/0034 hazards structurally excluded | `edge_documents_round_trip`'s lone-CR case + the NUL test |

**Handed forward, explicitly (not covered here, by design):** `full_rescan_html` + the `finalize` format branch (wave 4); `Title`'s row on the *wire schema* beyond the existing `SyncRole` (`source_format`/`input_format`, schema 1.3.0 — wave 6); units/context/prompt/cache over HTML documents, including what `html_outcomes`' per-block outcomes *mean* for batching (wave 5); `--input-format`, the refusal re-text, `out.html` (wave 6); `html_dominance_warning` re-text (wave the entry point lands, per spec §6); SCN-16 scenario-matrix row and Playwright (wave 7).

## Notes for the implementer

- **Fix bugs where they live.** Tasks 3–5 are pins over Task 2's implementation; a red pin is a Task 2 defect. Change `intake/html.rs`, re-run the spine, and say so in the commit body — never weaken a pin to match the code.
- **The fixture bytes are frozen the moment Step 1–2 of Task 2 write them.** If an expected kind sequence in this plan disagrees with what a correct implementation produces, stop and re-derive by hand against spec §4 before touching either side; the sequences above were hand-derived from the fixture bytes and the classification table, and a disagreement means one of the two was misread.
- **`grep -c` exits non-zero on zero count** — the precondition and DCR-number checks read the *printed count*, not the exit code; do not run them under `set -e`.
- **The `&nbsp;`-run paragraph and the amended img exception are deliberate** (rule T's byte test, and deviation 5's declared amendment: a linked image is one `Image` block, wrapper included, because the spec's own rationale for the exception is keeping the sync anchor); if a reviewer questions them, the answer is in the Deviations section and DCR-0035's owed spec amendment, not in code changes.
- **The wave's reds, named honestly.** Two reds are real: Task 2 Step 4's compile red (an unresolved import — harness wiring, not behaviour) and Task 2 Step 9's staged red, where byte-identity is already green and the kind-sequence assertions fail with real diffs — the live proof that identity alone is unfalsifiable against a zero-block stub, which is also why every spine test pairs identity with a block-set assertion. Tasks 3–5 are characterization pins over Task 2's behaviour and say so in their prefaces (the wave-0 Notes precedent). Do not skip Step 9 "to save a compile": landing rule T without its red erases the one demonstrated behavioural failure this plan stages.
- **If wave 4 lands mid-wave**, nothing here conflicts: the only shared artifact is the DCR number (Task 6 Step 1), and wave 4's hand-built `Document`s are unaffected by a new producer.
- **Do not add `intake` re-exports to `transync-core` or the facade** — wave 5 decides how core reaches the intake; adding a path now would widen the surface this wave's `forbidden` pin exists to guard.
