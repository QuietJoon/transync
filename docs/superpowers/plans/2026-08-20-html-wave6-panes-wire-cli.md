# HTML→HTML Wave 6 — panes, alignment wire, CLI — implementation plan

**Date:** 2026-08-20
**Ticket:** ti `490d97` (HTML→HTML document translation), wave 6 of 8 — spec `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §8 (render, anchors, OI-0035), §9 (CLI surface), §12 wave 6, with §13's D5/D6/D9 limitations binding.

**Goal:** the operator-visible feature. `transync translate --input-format html --input page.html --out-dir out/` writes a translated page (`out.html` — the full page, head, scripts, doctype, **anchor-free**) and a bundle whose panes mount and sync by block id. Three deliverables: (1) the HTML pane derivation — synthesized `<main>` fragments, strip → inject → wrap → balance → group, never annotated whole documents; (2) alignment schema **1.3.0** — per-row `source_format`, map-level `input_format`, the `"title"` kind with its `non-sync` role, under the existing forward-minor policy so a 1.2.0-era engine still drives a 1.3.0 map; (3) the CLI surface — `--input-format markdown|html` (flag-only routing), the sniff-refusal rewrite, the narrowed `--allow-html-input` with the exit-1 conflict, `out.html`, `Destinations.document`, the `--output` help degeneralization, and the new `about`.

**Architecture.** One new module, `crates/transync-syntax/src/render/html_pane.rs`, a child of `render` (spec §15 item 1 delegates the module home and entry-point names to this plan; the decision and its grounds are Deviation 1). One private worker serves both panes — `render_html_fragment(doc, alignment, pane, outcomes)` over `super::Pane::Source` (slicing `doc.source_text`) or `super::Pane::Target(&translated_html)` (slicing the regenerated HTML) — behind two `pub` entry points, `render_source_html` / `render_target_html`, re-exported from `render` beside `render_source` / `render_target`. It **reuses `PaneCtx::new` literally**, so the three refusals (`DuplicateRow` / `UncoveredBlock` / `UnusableRange`) carry over verbatim rather than by imitation, and the target string passes the same `parser::intake` guard `render_target` applies to `translated_md` (spec §8 step 1). Per row with `sync_role != "non-sync"`, in source order: slice → **strip** the reserved namespace (`transync_html::strip_reserved_sync_attrs`) → **inject** the four `render::attrs::write_attrs` attributes into the block's **own outermost element's** open tag when §4's tables know that element (no wrapper `<div>` for tables/code — a deliberate, documented divergence from the Markdown path, Deviation 4) → wrap an element-less block — or one whose outermost element is **unknown to §4's element tables** (the 2026-08-21 ruling; Deviation 4: the shell's DOMPurify mount removes an unknown element and every attribute riding it, so a self-injected anchor would die there) — in a transparent `<div{attrs}>` (the DCR-0016 precedent) → **balance** (`transync_html::balance_fragment`) → group consecutive `li` blocks under one shared `<ul>`/`<ol>` via `walk::normalize_top_level`'s collapse (Deviation 2). Gap bytes, `<head>`, doctype, comments, `<script>`/`<style>` are never emitted; a `fallback_source` block renders **live** with `data-fallback="fallback_source"`; only an extraction-failure block (per the run's `HtmlOutcome` map — Deviation 3) takes the escaped `<pre data-skipped="html-block">` placeholder. `out.html` is `regen::regenerate` output and **nothing in this wave touches `regen`**: injection is a bundle-only derivation over an immutable `&str`, and the acceptance pins `out.html` anchor-free end to end (spec §8: "anchors never touch the regen path").

**Two invariants this wave must not silently reopen, named up front:**

- **Strip-then-inject is a construction, not a scan** (spec §8 step 2/3). The strip runs over the block's raw bytes; injection then splices into the *stripped* string, using an open-tag span scanned from the *stripped* string. Reversing the order would strip the very attributes we injected (they are in the reserved namespace) and silently reintroduce OI-0035. Task 3 Step 1 test `strip_then_inject_makes_ours_the_only_sync_attributes` fails in **both** wrong orders: inject-then-strip yields zero of our anchors; no-strip yields two claimants for one id.
- **No route from anchors to `out.html`.** `out.html` = `TranslationOutput::translated_document` = `regen::regenerate`'s string; the derivation takes `&str` and returns a new `String`. `regen.rs` is on the not-touched list, the derivation's signatures admit no mutation, and two independent pins read the published document and assert `data-sync-id` absent (Task 5's `cli_html_format_run_writes_out_html_and_a_syncing_bundle` and Task 6's `test-browser.sh` grep). A "centralization" that injects into the regenerated string before splitting panes off it would fail both.

**DCR number: DCR-0038.** Waves 0–5 are DCR-0032/0033/0034/0035/0036/0037 in order. Task 1 Step 2 verifies `docs/project/design-change-records/DCR-0038-*` does not exist; **on a collision, STOP and hand back to the controller — never renumber silently.**

---

## Inherited obligations (read before Task 1)

Four items land on this wave from the earlier plans and records; each gets a step, not a hope.

1. **Wave 5 deviation 5's hand-forward: the empty panes.** Wave 5's plan (`2026-08-20-html-wave5-units-context-prompt.md`, deviation 5 and Task 6 Step 4) leaves an HTML run's `annotated_source_html` / `annotated_target_html` as empty strings, documents that on `TranslationOutput`, pins it twice — `pipeline.rs::html_run_tests::an_echo_html_run_is_byte_identical_with_empty_panes` (asserting `out.annotated_source_html.is_empty() && out.annotated_target_html.is_empty()`, message "panes are wave 6's (D6)…") and the SCN-16 scenario's empty-pane assertion (`scn_16_html_document.rs`, comment "panes themselves are wave 6's; both empty here") — and records the pair in DCR-0037 as **wave 6's hand-forward**. Task 4 executes both flips. They are this wave's two pre-authorized expectation edits on wave-5 tests, authorized by the plan that wrote them.
2. **Wave 5 Task 8's boundary note: the sniff-refusal pins are this wave's.** Wave 5's Task 8 interface says, verbatim: "One near-miss that is **not** a pin on this string and must not be touched: `crates/transync-cli/tests/cli_smoke.rs` asserts `stderr.contains(\"490d97\")` on the **CLI preamble-sniff refusal** — a different string, owned by wave 6." Task 5 rewrites that message and its pins together, per spec §9: "The `(ti 490d97)` clause goes **in the same commit**, or the string lies."
3. **Wave 0's hand-forward: `strip_reserved_sync_attrs`' second call site.** Wave 0's DCR-0032 text records "`strip_reserved_sync_attrs`' call sites, which are wave 1's and wave 6's." Wave 1 landed the Markdown `BlockKind::Html` arm (`balance_fragment(&strip_reserved_sync_attrs(md))` in `render.rs`); the HTML pane path here is the second and last call site. After Task 3, the function has no unclaimed caller left.
4. **Wave 2's ADR-0025 hand-forward: the `Document.format` render/pane guard.** Wave 2's records name "the `Document.format` refusals (wave 4's `finalize` branch, wave 6's render/pane guard)" as handed forward. Discharged here as a **pair** of `debug_assert_eq!`s, one per direction, both in Task 3 Step 3: `doc.format == SourceFormat::Html` at the top of the new derivation, and the mirror `doc.format == SourceFormat::Markdown` at the top of `render_fragment` — the consumer spec §3 actually names ("the Markdown renderer" is on its format-committed list, the consumers that must *refuse* the wrong document). The mirror is the half with a real hazard behind it: `render_source` is `pub` in a published crate and the wasm `rebuild` path is shipped precedent for out-of-pipeline render calls, and without it comrak parses an HTML-intake document as Markdown, Guard-2 degrades every row to escaped byte ranges, and a plausible-looking pane comes out with no refusal and no assert — the exact "passes while checking nothing" hazard wave 5 cited when it chose to ship empty panes. Both are assertions, not dispatches, mirroring wave 5's `build_batches` `debug_assert_eq!` posture: the *live* routing guarantee is `run_pipeline`'s exhaustive format match (wave 5), and a second fallible refusal here would be an unreachable arm claiming to be a gate (the rule wave 2's deviation 3 and wave 3's deviation 2 both applied) — that rule forbids the unreachable *fallible* arm; the debug assert is the same rule's tripwire for the day "unreachable" stops being true. DCR-0038 records **both halves**; the item does not close on one.

---

## Global Constraints

- **Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave6/`** — never `/tmp`, never `/private/tmp`, never `$TMPDIR`, never the OS default. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user**.
- **NEVER change or override `CARGO_TARGET_DIR`; never pass `--target-dir`.** If a cargo command fails because the target dir is unreachable, stop and ask.
- **Every `cargo test` invocation is capped:** `cargo test -p <crate> -- --test-threads=4`; workspace runs `cargo test --workspace -- --test-threads=4`. Never raise the cap.
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under the wave's temp dir, append `echo "CARGO_EXIT=$?"` to the same file, and inspect the file as a **separate** step. A pipeline reports the last stage's status, so a failing suite reads as a pass, and `tail -N` over filtered lines drops early failures.
- **If a number is offered as evidence, capture it to a file.** Test counts, grep counts, exit codes, diff stats — every number an expected-result line names must exist in a file under `/Volumes/Temp/claude/ti490d97-wave6/gate/`, not only in the transcript.
- **`git commit --no-verify` is never used.** The tracked pre-commit hook runs fmt, clippy, the two-package wasm gate and the rustdoc gate; a commit step's expected result is that all four pass. If the hook blocks, fix the cause.
- **Stage exact paths only** — never `git add -A`, never `git add <directory>`. Every commit step names its files.
- **The enum charter: no `_ =>` catch-all arm and no `matches!(x, Variant)` in dispatch/production code this wave writes**, over `SourceFormat`, `Spelling`, `BlockKind`, `SyncRole`, `HtmlOutcome`, `InputFormatArg` or any other workspace enum — `matches!` expands to a match with an implicit `_ => false`, so a variant added later silently takes the `false` branch instead of stopping the compiler. **Scope, stated so nobody manufactures a false deviation:** the charter binds code that *dispatches* on a variant. Three uses are sanctioned and named: (a) **test assertions** — `assert!(matches!(…))` fails loudly on a missed variant, the tree's shipped idiom; (b) **membership predicates** — a `matches!` inside `.find()` / `.is_some_and()` that asks "is this row/block the one I'm looking for?" is a filter, not a dispatch; this wave has exactly one, Task 3 Step 3's ordered-flag read `is_some_and(|b| matches!(b.kind, BlockKind::ListItem { ordered: true, .. }))`, which is byte-for-byte the shipped Guard-2 line in `render_list_group` and whose non-`ListItem` answer is unreachable by `normalize_top_level`'s label rule; (c) **`debug_assert`s** — the two `debug_assert_eq!(doc.format, …)` guards (the Html half in the derivation, the Markdown mirror in `render_fragment`) are assertions, nothing branches on them. Everything else — the CLI's two `InputFormatArg` matches, the sniff-arm match, the `Destinations` filename match, the row-role match, the `HtmlOutcome` match, the `Spelling → SourceFormat` projection — names every arm.
- **NEVER run `regen_prompt_goldens` and NEVER set `TRANSYNC_REGEN_GOLDENS`, under any circumstances.** This wave touches no prompt bytes; a red prompt golden is a finding — a bug in this wave's code — never a regeneration errand. STOP and diagnose.
- **Both `sync.js` copies move in one commit** whenever the engine file is touched at all — `web/js/sync.js` is edited, `crates/transync-cli/web/sync.js` is refreshed **by `cp`, never by hand**, and `crates/transync-cli/tests/sync_js_drift.rs` is the weld. This wave touches sync.js exactly once (Task 2's `KNOWN_SCHEMA` bump + its doc line); the engine's *logic* is deliberately untouched (Deviation 6).
- **Zero fixture edits, and zero expectation edits outside the declared set.** The declared set: Task 2's sanctioned schema-literal sweep (Deviation 5 enumerates it — the two `engine.spec.js` forward-minor specimens included), the two wave-5 pane flips (Inherited obligation 1), and the sniff-refusal pin rewrite in `cli_html_document_input_is_refused`, doc comment included (Inherited obligation 2, Task 5). §11's review gate — the Markdown corpus byte-identical — is checked in the acceptance section with the sanctioned diffs named.
- **`grep -c` exits non-zero on a zero count** — the precondition and gate greps below read the *printed count*, not the exit code; do not run them under `set -e`. A count **≥ 1** is the expectation unless a step says otherwise (a doc comment naming a symbol beside its definition is not a defect — absence is). Inside `scripts/test-browser.sh` (which runs `set -euo pipefail`), zero-expected greps use `if grep -q …; then FAIL` so the non-zero exit stays inside the `if`.
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`. `transync-syntax` gains **no `[features]` table and no `transync-core` dependency** (no `Cargo.toml` is touched anywhere in this wave; the acceptance diff proves it). `transync-wasm`'s dependency set stays `transync-syntax` alone (this wave touches only a test literal in `engine.rs`).
- **`docs/index.md` is NOT touched by this plan — the controller owns it this wave.** This plan file's own link is the controller's atomic move-and-link step; DCR-0038's link line is handed to the controller in Task 7 with its exact text, never added here. Between DCR-0038's creation and the controller's link, `docs_index_drift` is expected red naming DCR-0038's file — plus, possibly, this plan's own file while the move-and-link is pending; any name outside those two is real drift and a STOP. The exit-code consequence, stated once for every workspace gate below: while the allowance is live, a `cargo test --workspace` capture reads `CARGO_EXIT=101` with `docs_index_drift` its only failure — each later "Expected: `CARGO_EXIT=0`" on a workspace run carries this one carve-out without restating it.
- No pure-formatting edits. Where this plan shows wrapped Rust, run `cargo fmt --all` afterwards and take the formatter's answer.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, cite, or create them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `crates/transync-syntax/src/align.rs` | modify | `ALIGNMENT_SCHEMA_VERSION` → `"1.3.0"`; `AlignmentBlock.source_format: Option<SourceFormat>`; `AlignmentMap.input_format: SourceFormat`; both populated in `build_alignment_map`; the new `wire_format_tests` module |
| `crates/transync-syntax/src/render.rs` | modify | `pub mod html_pane;` + `pub use html_pane::{render_source_html, render_target_html};` plus the mirror `Document.format` debug assert at the top of `render_fragment` (Inherited obligation 4); no other line moves |
| `crates/transync-syntax/src/render/html_pane.rs` | **create** | THE §8 pane derivation: `PaneCtx` reuse, strip → inject → wrap → balance → group, the `HtmlOutcome`-keyed placeholder arm, the `parser::intake` target guard, and the `html_pane_tests` module |
| `crates/transync-syntax/src/render/attrs.rs` | modify | doc-truth only: the `parent_id` doc's "schema 1.2.0" → "schema 1.x (1.3.0 at this writing)" |
| `crates/transync-syntax/src/intake/html.rs` | modify | **additive only** (the 2026-08-21 ruling): the `pub(crate)` predicate `is_named_in_the_tables` beside the §4 classification consts, plus the named-DEFAULT-STOP const it reads; no table entry moves (Task 3 Step 3) |
| `crates/transync-core/src/pipeline.rs` | modify | the render dispatch's `Html` arm fills the panes (`render_source_html` / `render_target_html`, threading the run's `html_outcomes`); the `html_run_tests` empty-pane pin flips (pre-authorized) |
| `crates/transync-core/src/lib.rs` | modify | doc-truth only: `TranslationOutput`'s "empty until wave 6" pane note and `TranslateOptions.input_format`'s "the CLI surface is wave 6's" sentence both stop pointing forward |
| `web/js/sync.js` | modify | `KNOWN_SCHEMA` → `{ major: 1, minor: 3, patch: 0 }`; the `@param` doc line's `"1.2.0"` → `"1.3.0"`. **Logic untouched** (Deviation 6) |
| `crates/transync-cli/web/sync.js` | modify (by `cp`) | byte-identical twin of the above, same commit |
| `web/js/wasm-demo.js` | modify | `const KNOWN_SCHEMA = "1.3.0";` |
| `crates/transync-wasm/src/engine.rs` | modify | the one test literal `"1.2.0"` → `"1.3.0"` (sanctioned sweep) |
| `crates/transync/tests/scenarios/scn_01…scn_15` (13 files) + `scn_16_html_document.rs` | modify | the sanctioned sweep: `schema_version` assertion literals → `"1.3.0"`; scn_16 additionally flips its empty-pane assertion (Inherited obligation 1) and, if it pins the version, joins the sweep |
| `crates/transync-cli/src/main.rs` | modify | `about` → `"GFM Markdown and HTML document translation with block-level sync."`; the `Translate` subcommand doc line names both formats |
| `crates/transync-cli/src/translate_cmd.rs` | modify | `TranslateArgs.input_format`; the exit-1 conflict check; the sniff gate moves inside the Markdown arm of an exhaustive format match; `opts.input_format` wiring; the refusal message's rewritten tail; the narrowed `--allow-html-input` doc |
| `crates/transync-cli/src/translate_cmd/args.rs` | modify | `InputFormatArg` (clap `ValueEnum`) + `to_source_format()` (exhaustive) + its unit tests |
| `crates/transync-cli/src/translate_cmd/publish.rs` | modify | `Destinations.markdown` → `Destinations.document` (verified private — free rename); the `--out-dir` arm's per-format filename (`out.md` / `out.html`) |
| `crates/transync-cli/src/output.rs` | modify | `OUT_DIR_ENTRIES` gains `"out.html"`; the marker-less complete-set fallback becomes a predicate (Deviation 7 — the `published.len() == OUT_DIR_ENTRIES.len()` equality would be unsatisfiable) |
| `crates/transync-cli/tests/cli_smoke.rs` | modify | schema literal + the §11 cheap wire pin in `cli_translate_smoke` (Task 2); the refusal-message pins rewritten; five new tests (conflict, explicit-markdown sniff, HTML run end-to-end, no-reverse-sniff, marker-less republish) |
| `web/tests/engine.spec.js` | modify | tests `c`/`g` — their hard-coded `"1.3.0"` forward-minor specimens become derived, and the shared `forwardMinorVersion()` helper lands (sanctioned sweep, Task 2); test `m` — forward-minor: a one-minor-newer map carrying 1.3.0's additions still drives (Task 2); test `n` — the AC's direct-drive pane-sync assertion over the SCN-16 bundle (Task 6) |
| `scripts/test-browser.sh` | modify | the SCN-16 HTML-run bundle leg + the `out.html`-anchor-free grep (Task 6) |
| `docs/architecture/contracts.md` | modify | §3 rewritten to schema 1.3.0 (fields, the `"title"` kind, the forward-minor normativity note); §4 gains the HTML-source-panes subsection + the reserved-namespace rule's pane half; §4a gains one sentence; §6 gains the `--input-format` entry, the rewritten `--allow-html-input` entry, the rewritten exit-2 refusal prose, the `out.md | out.html` layout line, the allow-list prose, the bundle-title chain sentence, the `--output` entry |
| `docs/Developer_Guide.md` | modify | the "## CLI reference" block gains `--input-format` (with its stated default `markdown`) and the `--allow-html-input` one-liner re-centered — same commit as the flag (the drift weld) |
| `docs/implementation/module-map.md` | modify | the `render/` subtree gains the `html_pane.rs` row |
| `CLAUDE.md` | modify | the `transync-syntax` module-split `render` row: "(+ `render/attrs`)" → "(+ `render/attrs`, `render/html_pane`)" |
| `docs/project/design-change-records/DCR-0038-html-panes-wire-cli.md` | **create** (Task 7) | the wave's record: the derivation, the wire, the CLI, the sanctioned diffs, the discharged hand-forwards, the declared deviations |
| `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml` | modify (Task 7) | routine per-wave records |

**Not touched, deliberately:** `crates/transync-syntax/src/regen.rs` (**anchors never touch the regen path** — spec §8; `out.html` is `regenerate`'s string and stays anchor-free by construction, and this row is the standing proof that no injection was "centralized" into it); `crates/transync-syntax/src/{parser.rs,parser/,intake.rs,walk.rs,outcome.rs,id.rs,error.rs}` and — with exactly one sanctioned exception — `intake/` (waves 2/3 own them; `walk::normalize_top_level` and `outcome::HtmlOutcome` are **consumed, never modified**; the exception is the 2026-08-21 ruling's additive `is_named_in_the_tables` predicate in `intake/html.rs`, Task 3 Step 3, which adds beside §4's tables and moves no entry of them); everything under `crates/transync-core/src/validate/`, `crates/transync-core/src/unit*`, `crates/transync-core/src/llm*`, `crates/transync-core/src/profile.rs`, `crates/transync-core/src/pipeline/finalize.rs` (waves 4/5 own them; the layer-6 twin and the finalize dispatch are consumed); `crates/transync-html/` (wave 0's crate — both of its functions this wave calls already exist); `crates/transync-wasm/src/lib.rs` and the wasm demo surface (its `schema_version()` follows the constant automatically; scenario-matrix keeps wasm-demo HTML documents out of scope); `web/js/sync.js` **logic** (Deviation 6 — the only edits are the two schema-mirror lines); `web/tests/scn13.spec.js` and `web/tests/wasm.spec.js` (wave 1 and Track C own them; wave 7 owns `scn16.spec.js`, which this wave deliberately does **not** create); `cli_smoke.rs`'s `cli_allow_html_input_forces_the_previous_behavior` and `cli_html_input_re_emits_an_indented_run_as_a_fenced_block` (the override's behavior is bit-identical — §9 — and these two prove it by not moving); `crates/transync-cli/tests/{docs_cli_flags_drift.rs,exit_code_docs_drift.rs,sync_js_drift.rs}` (weld tests are consumed as gates, never edited — no new exit codes exist, so `exit_code_docs_drift` must stay green with zero edits); every `Cargo.toml` and `Cargo.lock` (no dependency moves, no version moves — the workspace is `0.5.0-dev` since wave 2); every existing fixture (the SCN-16 fixture is wave 3's file, consumed by path); `docs/architecture/scenario-matrix.md` (SCN-16's row is wave 7's); `docs/index.md` (controller-owned); the spec file itself; `scripts/hooks/pre-commit`, `scripts/smoke.sh`, `scripts/build-wasm.sh`.

---

## Deviations from the spec (and from the sibling plans)

Nine, declared here and nowhere else.

1. **The pane derivation's module home and entry-point names are decided here** (spec §15 item 1 delegates them, "under the constraint that it stays inside `transync-syntax`/`transync-html`"): `crates/transync-syntax/src/render/html_pane.rs`, entry points `render_source_html(doc, alignment, outcomes)` and `render_target_html(doc, translated_html, alignment, outcomes)`, re-exported from `render`. Grounds: a child of `render` can reuse the parent's private `PaneCtx`, `Pane`, `block_text`, `html_escape` and `FRAGMENT_OPEN`/`FRAGMENT_CLOSE` **without widening any visibility**, which is what makes "the three refusals carry over verbatim" literal reuse instead of a second copy; the names mirror `render_source`/`render_target` so `run_pipeline`'s two arms read as one family; and the wasm gate's dependency rules hold trivially (`transync-syntax` still depends only on `transync-html`, and `transync-wasm` is untouched). `public_surface.rs` already pins `render` as facade-hidden, so no §0 motion.
2. **`walk::normalize_top_level` is the `li`-grouping's one home — an interpretation of §7's "walk … must simply never be called on an HTML document", resolved toward D9 and OI-0010, with wave 3's pin as the sanction.** §7's sentence sits in the *validation* section and names walk as "the Markdown layer-6/renderer **pairing**" — the comrak-zip consumers (`reparse_full`, `render_fragment`), which this wave keeps off HTML documents exactly as wave 5's render dispatch left them. `normalize_top_level` itself is comrak-free (verified: pure IR projection — `label_for` + the `ast_path`-prefix collapse), D9's rationale (a) names its prefix collapse among the things that "already assume item-level anchors", spec §4 requires the intake's `ast_path`s to keep the collapse total precisely so "adjacent lists can never merge", and wave 3's plan ships `the_prefix_collapse_is_total_over_html_list_items` — `walk::normalize_top_level` called on an `intake::html::parse` document, asserted `[("list", 2), ("list", 1)]`. Re-implementing the collapse privately in `html_pane.rs` would be the second-opinion sin OI-0010 exists to forbid. DCR-0038 records the interpretation, and the owed §14-style spec amendment it implies: §7's blanket sentence should name the comrak-typed consumers (`reparse_full`, `render_fragment`) rather than the `walk` module — over-broad as written, contradicted by the spec's own §4 sentence and by §8 step 6 (Task 7 carries the note; the spec file stays untouched). If the landed wave-3 tree lacks that pin (see Task 1's gate), STOP and re-read the landed `walk.rs` before proceeding.
3. **The derivation takes the run's `HtmlOutcome` map as a parameter.** §8 step 7 distinguishes a `fallback_source` block (renders **live**) from an extraction-failure block (the escaped placeholder), but the alignment row alone cannot: `align::build_alignment_map` maps `ExtractionFailed` to `fallback_status: FallbackSource` too (verified in `align.rs`). The one artifact that knows the difference is the per-run `html_outcomes` map, already threaded to batching, alignment and the report "so all three agree" (outcome.rs's own doctrine); the derivation becomes its fourth consumer, with the same must-be-this-doc's-map contract `build_alignment_map` documents. Recomputing it inside the renderer was rejected — a second computation is a second chance to disagree with what was batched.
4. **No wrapper `<div>` for tables, code blocks, blockquotes, or any block that IS one element *§4's tables know* — a deliberate, documented divergence from the Markdown path, stated so nobody "fixes" it back — amended 2026-08-21 by owner ruling into a three-way boundary.** The Markdown path's transparent wrapper (ADR-0007 / DCR-0001) exists because comrak's HTML output could not carry attributes; an HTML block's own `<table>`/`<pre>`/`<blockquote>`/`<h2>` open tag can, and §8 step 3 mandates injecting there. The transparent `<div{attrs}>` survives for exactly three populations — **element-less blocks** (a rule-T bare-text run, a block missing its open tag), **img-run `Image` blocks** (the void corollary below), and — **the 2026-08-21 ruling** — **any block whose outermost element is unknown to §4's element tables** (a custom element, a future element) — §8 step 4's DCR-0016 precedent. The ruling's ground, empirically confirmed against the vendored DOMPurify 3.2.6 (the exact `web/vendor/purify.min.js` the bundle ships): the shipped shell mounts panes through `DOMPurify.sanitize(...)`, whose fail-closed config §8 pins **untouched**, and the sanitizer removes an unknown element **with every attribute riding it** — `sanitize('<x-note tone="info" data-sync-id="html-0012" …>A custom element stops, loudly.</x-note>')` keeps only the text — so self-injection into `<x-note>` ships an anchor that dies in the mount: a hole in the pane's anchor list and a `warnMapDomDrift` "no DOM anchor" warning at every load. Wrapped, the same block mounts as `<div data-sync-id="html-0012" …>…</div>` and the anchor survives (`div` and `data-*` are inside what the untouched config accepts — wave 1's `ALLOW_DATA_ATTR` probe measured the attribute half). **The rule is keyed on §4's element tables, never on DOMPurify's allowlist** — this is the part a later reader will be tempted to "simplify" wrongly: duplicating DOMPurify's allowlist in Rust would be a **second opinion about what the sanitizer accepts**, the same class of sin the architecture forbids for a second Markdown parser. §4's tables already classify a custom element as unknown — that is precisely *why* D4 makes it a boundary — so the wrapper rule uses knowledge the crate already owns, and it is deliberately **broader** than "custom elements": any element §4 does not know takes the wrapper, which fails safe as HTML evolves. One home for the answer: `intake::html::is_named_in_the_tables`, `pub(crate)`, beside the tables themselves (Task 3 Step 3's third file — this wave's one sanctioned, additive touch on a wave-3 file). Pinned by `an_unknown_element_block_takes_the_wrapper_not_its_own_open_tag`; Task 3's test 1 pins the divergence (`<table data-sync-id=` present, `<div data-sync-id="t-` absent); Task 4 writes all of it into contracts §4 with this rationale, and both step-3/4 boundary refinements ride the owed §8 amendment (Task 7). Corollary, also deliberate: an HTML-document `Image` block renders under the transparent `<div>` wrapper, not the Markdown pane's `<figure>` — the engine reads `[data-sync-id]` only (§4's own closing rule), and `data-block-kind="image"` still names the kind. The mechanism, and a **declared sharpening of §8 step 3/4's boundary**: `element_extents` never records an extent for a void element (the landed walk's push is guarded `!is_void(name) && …`), so an img-run block — the lone `<img>` included — presents zero top-level extents and takes the wrapper arm. By §8's letter a lone `<img>` is neither "element-less" nor "missing its open tag" — it IS one element with an open tag — but step 3's "own outermost element" is read through the §5 instrument, and the instrument has nothing for injection to land in; special-casing voids here would be a second opinion about HTML structure, the exact thing the one-walk rule exists to forbid. Pinned by `a_lone_void_img_takes_the_wrapper_not_its_own_open_tag`; DCR-0038 records the sharpening.
5. **The sanctioned expectation sweep of the 1.3.0 bump, enumerated.** §11 calls the Markdown corpus a review gate ("zero fixture or expectation edits in every refactor commit") and names "these two additive fields as the *only* sanctioned alignment-output diff." The bump commit is not a refactor commit, and its sanctioned alignment-output diff is **three** things, not two — the `schema_version` string itself moves with the fields. The expectation edits that follow, all mechanical and all in one commit (Task 2): the 13 scenario files asserting `schema_version == "1.2.0"` (`scn_01_headings_and_paragraphs`, `scn_02_table_small`, `scn_03_table_large`, `scn_04_code_block`, `scn_05_nested_list`, `scn_06_blockquote`, `scn_07_validation_retry`, `scn_08_fallback`, `scn_09_prompt_injection`, `scn_10_long_document`, `scn_11_language_auto`, `scn_14_full`, `scn_15_html_blocks` — inventory from a planning-time grep, re-verified fresh in Task 2 Step 4 because wave 5's `scn_16` may add a fourteenth), `cli_smoke.rs::cli_translate_smoke`'s literal, `transync-wasm/src/engine.rs`'s one test literal, **and the two `web/tests/engine.spec.js` forward-minor specimens** — tests `c` and `g` hard-code `"1.3.0"`, written when the engine knew 1.2.0 as a one-minor-newer specimen; this bump makes that the engine's own version, `forwardDrift` goes false, `c` loses its "is newer than this engine" warning and `g`'s divergent row meets `validateRows`' strict identity check, so both go red at the first browser run. Their sanctioned edit is not a literal bump but a derivation: the specimen becomes `${known.major}.${known.minor + 1}.0` read from the served sync.js — exactly as the new test `m` derives it — via one shared helper, landing in the same commit as the bump (Step 5). And a warning the inventory's own history earned: the planning-time inventory came from a `"1\.2\.0"` grep, which is blind to these two **by construction** — their literal is the NEW version. The complete sweep is two greps: `"1\.2\.0"` for stale backward literals, and `"1\.3\.0"` over `web/tests/` for hard-coded forward specimens the bump just made current. No fixture moves; no non-version expectation moves. DCR-0038 states the three-part sanctioned diff, sharpening §11's two-field sentence. **A second sharpening of the same §11 sentence:** its cheap pin reads "a Markdown run's map declares `source_format: "markdown"` on every row" — but §3 (the normative wire section) defines the row field as the block's **spelling**, and its reader rule exists precisely because a 1.2.0 html-*island* row is `"html"`. The pin is written to §3: `input_format == "markdown"`, and every row's `source_format` equals `block_kind == "html" ? "html" : "markdown"` over the SCN-14 corpus (which contains islands, making the html arm non-vacuous). Resolved toward §3 under the document hierarchy; recorded in DCR-0038.
6. **The engine's logic is untouched; the schema bump is the whole sync.js edit.** §12 wave 6 lists no engine work (wave 1 landed OI-0035's engine half), and 1.3.0's additions are outside `validateRows`' five policed facts (verified: it checks row-is-object, `source_block_id`, uniqueness, `target_block_id` identity, `sync_role` — extra fields are never enumerated), while `"title"` rows arrive with `sync_role: "non-sync"`, a value `KNOWN_SYNC_ROLES` has carried since 1.0, skipped by `warnMapDomDrift`, excluded by `synchronizableRowCount`, and excluded from wave 1's `rowIdSet` — all four verified against the shipped engine. What *must* move is the three schema mirrors (`KNOWN_SCHEMA` in both sync.js copies and in `wasm-demo.js`), because `sync_js_drift.rs` welds each to `ALIGNMENT_SCHEMA_VERSION` and all three go red the moment the constant bumps.
7. **`OUT_DIR_ENTRIES` gains `"out.html"`, and the marker-less complete-set fallback becomes a predicate — a consequence §9 implies but does not spell out.** Adding the name alone would break two ways: without it, an HTML run's own `--out-dir` target reads as **foreign** on republish (`out.html` off the allow-list → exit 4); with it added naively, `ensure_out_dir_replaceable`'s marker-less fallback `published.len() == OUT_DIR_ENTRIES.len()` becomes **unsatisfiable** — a run writes `out.md` *or* `out.html`, never both, so no target ever holds all five names and the ti-`66339b` cp-drops-dotfiles recovery dies silently. The predicate: the three fixed members (`alignment.json`, `validation-report.json`, `html/`) plus **at least one** document file. Task 5 pins both directions end to end.
8. **The `parser::intake` guard on the target string is applied for its refusal; the pane still slices the caller's own bytes.** §8 step 1 mandates the guard ("otherwise the pane path indexes a different string than the one `regen` produced offsets into"), and `render_fragment`'s R0002-0059 comment states the resolution this plan copies: only a *parse* may consume the normalized `Cow` — "`ctx.md` stays the caller's own bytes, because the byte ranges … index THOSE." The HTML path has no parse, so the guard's live effects are exactly two: the `TooDeeplyNested` refusal (surfacing as the existing `RenderError::Intake`) and the NUL rule's parity — and since a NUL cannot legitimately reach regen output (source NUL-normalized at intake, payloads NUL-refused at the schema layer), a string the guard would *normalize* is one regen did not produce, which is precisely what sharing the one guarded door makes visible. Named honestly: the nesting bound is a Markdown-container heuristic that scores **per line** — each line's own container-marker prefix, with the document's answer the largest any single line reaches — so the refusal needs a single line carrying 129+ `>` markers (Task 3's test uses one line of 300), or a list run that out-indents a marker-per-line 129 times; 129 lines each opening with one `>` score depth 1 apiece and pass. A translated segment tripping either shape is pathological, accepted, and the same refusal `render_target` would give the equivalent Markdown pane. The guard is target-only, as §8 spells it: the source string already passed the HTML intake's own normalization.
9. **Non-sync rows are absent from HTML panes — including `<hr>`.** §8's derivation is "per row with `sync_role != \"non-sync\"`", so an HTML document's `ThematicBreak` block (like its `Title`) never reaches the pane, where the Markdown pane renders a bare `<hr data-block-kind>` for continuity. D6 (a sync surface, not a fidelity preview) is the ground and §8's own letter is the rule; §13 item 6 records the title's half, and there is **no** `<hr>` item to cite — its absence follows from the rule, not from a recorded limitation (item 3's no-layout/no-styles/no-scripts sentence is the nearest neighbor and does not name it); the engine is indifferent (non-sync rows are skipped by every consumer — Deviation 6's list). Also under this head: a source `<ol start="7">`'s container tags are **gap** (D9 / §13 item 4), so the pane's reconstructed `<ol>` carries no `start` and renumbers from 1 — reading the gap to recover it would be a new intake opinion, and the pane is not a fidelity preview. Both recorded in DCR-0038 and in contracts §4's new subsection.

---

### Task 1: Preconditions STOP gate, DCR collision check, and the recorded green "before"

**Files:**
- Test: none. This task's product is a verified precondition set and a captured baseline.

**Interfaces:**
- Consumes (and verifies landed): wave 0's `transync_html::{strip_reserved_sync_attrs, balance_fragment, element_extents, scan_tags, TagToken}`; wave 1's engine gate + Markdown-arm strip; wave 2's `SourceFormat`/`Spelling`/`Title`; wave 3's `intake::html::parse` + the SCN-16 fixture; wave 4's `full_rescan_html` + the finalize branch; wave 5's `TranslateOptions.input_format` + the `transync::SourceFormat` export + the empty-pane pins.
- Produces: `/Volumes/Temp/claude/ti490d97-wave6/gate/baseline-*.txt`, the files the acceptance section diffs against.

- [ ] **Step 1: Create the wave's temp directory.**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave6/gate
```
Expected: no output, exit 0. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user** — do not fall back to `/tmp`.

- [ ] **Step 2: Hard precondition gate — waves 0 through 5 must be COMPLETE, and DCR-0038 must be free.** Every code block below is written against the landed shapes of six prior waves; on a partial wave, the whole plan is wrong. Run each line separately (not under `set -e`; read the printed counts):
```bash
# wave 0 — the mechanics crate and both functions this wave calls
grep -c 'pub fn strip_reserved_sync_attrs' crates/transync-html/src/lib.rs
grep -c 'pub fn balance_fragment' crates/transync-html/src/lib.rs
grep -c 'pub fn element_extents' crates/transync-html/src/lib.rs
ls docs/project/design-change-records/DCR-0032-*.md
# wave 1 — the engine gate and the Markdown-arm strip (both twins)
grep -c 'strip_reserved_sync_attrs' crates/transync-syntax/src/render.rs
grep -c 'synchronizableRowIds' web/js/sync.js
grep -c 'synchronizableRowIds' crates/transync-cli/web/sync.js
ls docs/project/design-change-records/DCR-0033-*.md
# wave 2 — the IR split
grep -c 'pub enum SourceFormat' crates/transync-syntax/src/id.rs
grep -c 'pub enum Spelling' crates/transync-syntax/src/id.rs
grep -c 'Title => SyncRole::NonSync' crates/transync-syntax/src/align.rs
grep -c '0.5.0-dev' Cargo.toml
ls docs/project/design-change-records/DCR-0034-*.md
# wave 3 — the intake, the fixture, and the grouping sanction (Deviation 2)
grep -c 'pub fn parse' crates/transync-syntax/src/intake/html.rs
ls crates/transync/tests/fixtures/scn-16-html-document.html
grep -c 'the_prefix_collapse_is_total_over_html_list_items' crates/transync-syntax/tests/html_intake_alignment.rs
ls docs/project/design-change-records/DCR-0035-*.md
# wave 4 — the twin and the finalize dispatch
grep -c 'pub fn full_rescan_html' crates/transync-core/src/validate/full_rescan_html.rs
grep -c 'full_rescan_html' crates/transync-core/src/pipeline/finalize.rs
ls docs/project/design-change-records/DCR-0036-*.md
# wave 5 — the entry point, the export, the empty-pane pins this wave flips
grep -c 'pub input_format: SourceFormat' crates/transync-core/src/lib.rs
grep -c 'SourceFormat' crates/transync/src/lib.rs
grep -c 'an_echo_html_run_is_byte_identical_with_empty_panes' crates/transync-core/src/pipeline.rs
grep -c 'intake::html::parse' crates/transync-core/src/pipeline.rs
ls docs/project/design-change-records/DCR-0037-*.md
# DCR-0038 must NOT exist — this one expects NO output (and a non-zero ls exit)
ls docs/project/design-change-records/DCR-0038-* 2>/dev/null
```
Expected: every `grep -c` prints **≥ 1**; every `ls` except the last echoes exactly one path; the last `ls` prints **nothing**. **Any zero count, any missing DCR-0032…0037, or any existing DCR-0038 file is a STOP** — hand the collision or the unfinished wave back to the controller; never renumber, never write against a shape you have not verified. Capture the whole transcript:
```bash
# re-run the block above with `> /Volumes/Temp/claude/ti490d97-wave6/gate/preconditions.txt 2>&1` on a wrapper script if you prefer; the counts must land in the file either way
```
One softer probe, informational rather than gating: `grep -c 'source_format' crates/transync-syntax/src/align.rs` — expected **0** (the field does not pre-exist; if a prior wave landed it, Task 2's compile red will not appear and Task 2's steps must be re-read against the landed file before editing).

- [ ] **Step 3: Capture the baseline, bare-to-file.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-cli.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-wasm.txt
scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-browser.txt 2>&1
echo "SUITE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-browser.txt
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave6/gate/baseline-commit.txt
```
Read each file separately. Expected: `CARGO_EXIT=0` in the cli and wasm captures and `SUITE_EXIT=0` in the browser capture, unconditionally. `baseline-workspace.txt` is the one with a carve-out: `CARGO_EXIT=0`, **or** `CARGO_EXIT=101` whose only failing test is `docs_index_drift` naming the pending plan file — the standing allowance lives inside `cargo test --workspace`, so when it is live it changes the exit code, not just a log line, and demanding 0 in the same breath as granting it would be a contradiction. Anything else red — any other test, or `docs_index_drift` naming any other file — is a STOP: this wave starts from green or it does not start.

---

### Task 2: Schema 1.3.0 — the wire fields, the constant, the three JS mirrors, the sanctioned sweep, and the forward-minor proof

**Files:**
- Modify: `crates/transync-syntax/src/align.rs`, `crates/transync-syntax/src/render/attrs.rs` (doc line), `web/js/sync.js`, `crates/transync-cli/web/sync.js` (by `cp`), `web/js/wasm-demo.js`, `crates/transync-wasm/src/engine.rs` (test literal), the 13 (+ possibly `scn_16`) scenario files, `crates/transync-cli/tests/cli_smoke.rs`, `docs/architecture/contracts.md` (§3), `web/tests/engine.spec.js` (the `c`/`g` specimen rewrites + the `forwardMinorVersion` helper in Step 5; test `m` in Step 7)

**Interfaces:**
- Produces, for Tasks 3–6 and every consumer of the wire:
  ```rust
  // crates/transync-syntax/src/align.rs
  pub const ALIGNMENT_SCHEMA_VERSION: &str = "1.3.0";
  // AlignmentBlock gains (it is #[non_exhaustive]: additive on the wire)
  pub source_format: Option<SourceFormat>,   // always Some in emitted maps
  // AlignmentMap gains (also #[non_exhaustive])
  pub input_format: SourceFormat,
  ```
- **Two facts, two names, never one doing double duty** (spec §3): the row field is the block's **spelling** (a Markdown run's html-island row says `"html"`); the map field is the run's **intake** (what a pane mounter branches on). The row's `Option` exists solely so `Deserialize` accepts a pre-1.3.0 map; the documented reader rule for an absent value is `block_kind == "html" ? html : markdown` — a bare "default markdown" is rejected because it would mislabel every 1.2.0 html-island row.
- **The forward-minor criterion is designed in, not asserted in:** no new `sync_role` value, no new required row fact, both additions outside `validateRows`' five policed facts, `"title"` riding the `non-sync` role shipped since 1.0. Test `m` (Step 7) is the mechanism made durable.
- **One commit for the whole Rust+JS mirror set** (Steps 1–6). The pre-commit hook runs fmt/clippy/wasm/rustdoc but not the suites, so nothing *forces* one commit — house discipline does: no commit of this wave may leave `cargo test --workspace`, the cli suite, or `sync_js_drift` red, and the mirrors go red the instant the constant moves.

- [ ] **Step 1: Write the failing wire tests.** In `crates/transync-syntax/src/align.rs`, append after `mod html_row_tests`:
```rust
// ti 490d97 wave 6 (spec §3): schema 1.3.0's two additive fields. The row
// field is the block's SPELLING; the map field is the run's INTAKE — two
// facts, two names, never one doing double duty (the synthesis found two
// areas colliding on one name here, and the ruling is both fields).
#[cfg(test)]
mod wire_format_tests {
    use super::*;
    use crate::id::{SourceFormat, assign_block_ids};

    fn built_map(mut doc: crate::parser::Document) -> AlignmentMap {
        assign_block_ids(&mut doc);
        let outcomes = crate::outcome::html_outcomes(&doc);
        build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &outcomes,
        )
    }

    /// A Markdown run: the map says markdown, a prose row says markdown, and
    /// an html-ISLAND row says html — the row is the spelling, not the run.
    /// This is what makes the reader rule for absent values non-arbitrary:
    /// a bare "default markdown" would mislabel exactly this row.
    #[test]
    fn a_markdown_runs_rows_carry_their_spelling_and_the_map_carries_the_intake() {
        let map = built_map(
            crate::parser::parse("prose\n\n<div>island</div>\n").expect("parses"),
        );
        assert_eq!(map.input_format, SourceFormat::Markdown);
        let prose = map.blocks.iter().find(|r| r.block_kind == "paragraph").expect("p row");
        assert_eq!(prose.source_format, Some(SourceFormat::Markdown));
        let island = map.blocks.iter().find(|r| r.block_kind == "html").expect("island row");
        assert_eq!(
            island.source_format,
            Some(SourceFormat::Html),
            "an island inside a Markdown document is html-SPELLED; the run is still markdown",
        );
    }

    /// An HTML-intake document: every row html, the map html, and the title
    /// row carries the D5 shape — kind "title", role non-sync, format html.
    #[test]
    fn an_html_intake_map_says_html_everywhere_and_the_title_row_is_non_sync() {
        let map = built_map(crate::intake::html::parse(
            "<html><head><title>T</title></head><body><p>x</p></body></html>\n",
        ));
        assert_eq!(map.input_format, SourceFormat::Html);
        for row in &map.blocks {
            assert_eq!(
                row.source_format,
                Some(SourceFormat::Html),
                "{}: format == Html implies every spelling is Html (spec §3's invariant)",
                row.source_block_id.0,
            );
        }
        let title = map.blocks.iter().find(|r| r.block_kind == "title").expect("title row");
        assert_eq!(title.sync_role, SyncRole::NonSync);
    }

    /// Backward: a pre-1.3.0 map (no source_format, no input_format) still
    /// deserializes — input_format reads as the format every pre-1.3.0 run
    /// had, and the row's absence stays None (the documented reader rule is
    /// the CONSUMER's, applied at read time, never invented by serde).
    #[test]
    fn a_pre_1_3_0_map_still_deserializes() {
        // The §3 example row, verbatim shape, minus the two new fields.
        let json = r#"{
          "schema_version": "1.2.0",
          "document_id": "a91f2c0d2e1bbb40",
          "source_language": "en",
          "target_language": "ko",
          "detected_source_language": null,
          "generator": { "name": "transync", "version": "0.4.0" },
          "blocks": [{
            "source_block_id": "h1-0001",
            "target_block_id": "h1-0001",
            "block_kind": "heading-1",
            "source_order": 0,
            "target_order": 0,
            "source_range": { "start": 0, "end": 18 },
            "target_range": { "start": 0, "end": 22 },
            "sync_role": "anchor",
            "fallback_status": "translated",
            "parent_id": null
          }],
          "validation_summary": {
            "total_units": 1, "translated": 1, "preserved": 0,
            "partially_translated": 0, "fallback_source": 0, "retried_units": 0
          }
        }"#;
        let map: AlignmentMap = serde_json::from_str(json).expect("a 1.2.0 map deserializes");
        assert_eq!(map.input_format, SourceFormat::Markdown);
        assert_eq!(map.blocks[0].source_format, None);
    }

    /// Forward: the wire keys and kebab-case values, and the None-row key
    /// omission — the exact bytes a JS consumer sees.
    #[test]
    fn the_wire_keys_are_kebab_case_and_an_absent_row_value_omits_the_key() {
        let map = built_map(
            crate::parser::parse("prose\n\n<div>island</div>\n").expect("parses"),
        );
        let v = serde_json::to_value(&map).expect("serializes");
        assert_eq!(v["schema_version"], "1.3.0");
        assert_eq!(v["input_format"], "markdown");
        let rows = v["blocks"].as_array().expect("array");
        assert!(rows.iter().any(|r| r["source_format"] == "markdown"));
        assert!(rows.iter().any(|r| r["source_format"] == "html"));

        // A hand-built None row (same crate; #[non_exhaustive] allows it here)
        // omits the key rather than writing null — round-trip honesty for the
        // one shape only a pre-1.3.0 file can hold.
        let mut none_row = map.blocks[0].clone();
        none_row.source_format = None;
        let rv = serde_json::to_value(&none_row).expect("serializes");
        assert!(
            rv.get("source_format").is_none(),
            "a None row must omit the key, not write null: {rv}",
        );
    }
}
```
- [ ] **Step 2: Watch the compile red.**
```bash
cargo test -p transync-syntax wire_format_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` with `error[E0609]: no field `input_format` on type` and `error[E0609]: no field `source_format`` at the test sites (and nothing else structural — `intake::html::parse` and `html_outcomes` resolve). A compile error **is** the red; the fields do not exist.

- [ ] **Step 3: Land the constant, the fields, and the population.** In `crates/transync-syntax/src/align.rs`:
  - the constant becomes `pub const ALIGNMENT_SCHEMA_VERSION: &str = "1.3.0";` and its doc comment gains, after the 1.2.0 sentence: `/// Bumped 1.2.0 → 1.3.0 for three additive changes (ti 490d97 / DCR-0038): the per-row `source_format`, the map-level `input_format`, and the `"title"` `block_kind` value riding the `non-sync` role shipped since 1.0.` Update the struct-level `/// On-disk JSON shape — `schema_version 1.2.0`.` line and the §3-style mentions in this file to 1.3.0 (the `parent_id` doc's "schema 1.2.0 pins it" becomes "schema 1.x pins it (1.3.0 at this writing)").
  - the import line `use crate::id::{BlockId, BlockKind};` becomes `use crate::id::{BlockId, BlockKind, SourceFormat, Spelling};`
  - `AlignmentBlock` gains, after `parent_id`:
```rust
    /// How the source SPELLED this block — `"markdown"` or `"html"` (schema
    /// 1.3.0, ti 490d97). Mandatory in every map this engine emits; the
    /// `Option` exists solely so `Deserialize` accepts a pre-1.3.0 map.
    /// Reader rule for an absent value: `block_kind == "html" ? html :
    /// markdown` — never a bare default to markdown, which would mislabel
    /// every 1.2.0 html-island row. Distinct from [`AlignmentMap::input_format`]:
    /// a Markdown run's map legitimately carries html-spelled island rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_format: Option<SourceFormat>,
```
  - `AlignmentMap` gains, after `detected_source_language`:
```rust
    /// Which intake produced this run — the run-level fact a pane consumer
    /// branches on (schema 1.3.0, ti 490d97). `#[serde(default)]` reads a
    /// pre-1.3.0 map as `Markdown`, which is the format every pre-1.3.0 run
    /// actually had, so the default is a fact rather than a guess.
    #[serde(default)]
    pub input_format: SourceFormat,
```
  - `Default for AlignmentMap` gains `input_format: SourceFormat::Markdown,` — the enum's own `#[default]` value spelled explicitly, because this literal lists every field.
  - `build_alignment_map`'s row constructor gains, after `parent_id: None,`:
```rust
            // The row is the block's SPELLING (spec §3): an island inside a
            // Markdown document says html here while the map says markdown.
            // Exhaustive by charter; `Html { .. }` names the variant, the
            // rest pattern covers its field.
            source_format: Some(match block.spelling {
                Spelling::Markdown => SourceFormat::Markdown,
                Spelling::Html { .. } => SourceFormat::Html,
            }),
```
  - the closing `AlignmentMap { … }` literal gains `input_format: doc.format,` beside `detected_source_language`.

- [ ] **Step 4: Run the crate, then enumerate the downstream reds before sweeping.**
```bash
cargo test -p transync-syntax -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-syntax.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-syntax.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-downstream-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-downstream-red.txt
cargo test -p transync-cli --features test-stub-provider cli_translate_smoke -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-cli-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-cli-red.txt
```
Expected, and this is the wave's key **behavioural red inventory** — read the files and check each name off: `t2-syntax.txt` `CARGO_EXIT=0` (the four `wire_format_tests` pass; if `a_pre_1_3_0_map_still_deserializes` is red with `missing field `input_format``, the `#[serde(default)]` attribute was dropped — that red is the attribute's own falsification, fix and re-run). `t2-downstream-red.txt` `CARGO_EXIT=101` with **exactly** these failures: the 13 scenario tests' `assertion `left == right` failed … left: "1.3.0" / right: "1.2.0"` (plus `scn_16`'s, if wave 5 pinned the version — record the observed count), `transync-wasm`'s `engine` test on the same literal, and the two `sync_js_drift` weld tests covering the three JS mirrors (`sync_js_known_schema_matches_the_rust_alignment_schema_version` — both sync.js copies — and `wasm_demo_js_known_schema_matches_the_rust_alignment_schema_version`), each naming the drifted file. `t2-cli-red.txt` `CARGO_EXIT=101` on `cli_translate_smoke`'s `alignment_map.schema_version must be 1.2.0`. **Any failure outside this inventory is a STOP** — it means 1.3.0 moved something the design says it must not. Two more casualties exist that **no capture in this step can show**: `web/tests/engine.spec.js` tests `c` and `g` are Playwright tests, invisible to cargo, and their hard-coded `"1.3.0"` forward-minor specimens just became same-version maps. They are Deviation 5's two JS members; Step 5 rewrites them before Step 7 runs the browser suite, which is the first place their reds could ever surface.

- [ ] **Step 5: The sanctioned sweep + the three JS mirrors, one commit's worth of edits.**
  - Re-verify the literal inventory fresh, capturing it: `grep -rn '"1\.2\.0"' crates/ web/js/ web/tests/ > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-literal-inventory.txt` — expected hits: the 13 scenario files, `cli_smoke.rs`, `transync-wasm/src/engine.rs`, `web/js/wasm-demo.js`, `engine.spec.js`'s `mapOf` (see below), doc-comment mentions in the `sync.js` copies, **and two expected-and-unswept files**: `crates/transync-syntax/src/align.rs` — Step 1's backward-compat fixture embeds `"schema_version": "1.2.0"` deliberately; it IS the pre-1.3.0 specimen and stays — and `crates/transync-cli/tests/sync_js_drift.rs` — one hit, the doc comment quoting wasm-demo.js's old `const KNOWN_SCHEMA = "1.2.0";` line. That one goes stale and **stays stale, accepted**: the weld's zero-edit pin (acceptance 2) outranks a cosmetic version inside an illustrative comment, and the comment quotes the line *shape* the parser scrapes, not a value anything asserts — its sibling comment quoting the object shape `{ major: 1, minor: 2, patch: 0 }` is equally stale and invisible to this grep, and both stay for the same reason. Anything else: read it before touching it; a hit in `validate`/`llm` would be a different "1.2.0" (a version of something else) and is **not** swept.
  - In each of the 13 scenario files and in `cli_smoke.rs::cli_translate_smoke`: the assertion literal `"1.2.0"` → `"1.3.0"` (and the assertion message's `1.2.0` → `1.3.0` where it names the number).
  - In `crates/transync-wasm/src/engine.rs`: `assert_eq!(map["schema_version"], "1.2.0");` → `"1.3.0"`.
  - In `web/js/sync.js`: `const KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 };` → `{ major: 1, minor: 3, patch: 0 };` and the `@param {Object} alignmentMap  // schema_version "1.2.0"` doc line → `"1.3.0"`. **Nothing else in the file moves.** Then refresh the twin **by copy**: `cp web/js/sync.js crates/transync-cli/web/sync.js`.
  - In `web/js/wasm-demo.js`: `const KNOWN_SCHEMA = "1.2.0";` → `"1.3.0";`.
  - `web/tests/engine.spec.js`'s `mapOf` keeps its `schema_version: "1.2.0"` default **deliberately**: after this bump that is an *older-minor* map, accepted silently (`forwardDrift` false, `console.debug`), so every rig test riding the default keeps passing unchanged — and a 1.2.0-shaped map staying drivable is itself the compatibility property. Do not "modernize" it. Two rig tests do **not** ride the default: `c` and `g` override it with a hard-coded `"1.3.0"` as their forward-minor specimen, and the next bullet is their sanctioned rewrite.
  - **The sweep's two JS members — tests `c` and `g` become derived (Deviation 5).** The `"1\.2\.0"` inventory grep above is blind to them by construction — their literal is the *new* version — so run the companion grep and capture it: `grep -n '"1\.3\.0"' web/tests/engine.spec.js > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-forward-specimens.txt` — expected: exactly the three hits inside tests `c` and `g`, and zero when re-run after this edit. Why they are red after the bump: `"1.3.0"` was written as a one-minor-newer specimen when the engine knew 1.2.0; it is now the engine's own version, `forwardDrift` is false, so test `c`'s coda never gets its "is newer than this engine" warning and test `g`'s divergent-row map meets `validateRows`' strict identity check and the mount refuses. First add the shared helper beside `mapOf` (one home; test `m` in Step 7 rides it too):
```js
/**
 * The version one minor NEWER than the served engine's own KNOWN_SCHEMA —
 * THE forward-minor specimen. Derived, never hard-coded: a literal here
 * becomes a same-version map at the next bump and the forward coverage
 * dies silently (the 1.3.0 bump caught tests c and g holding "1.3.0"
 * literals written when the engine knew 1.2.0).
 */
function forwardMinorVersion() {
  const line = readFixture("sync.js")
    .split("\n")
    .find((l) => l.includes("KNOWN_SCHEMA = {"));
  return `${/major:\s*(\d+)/.exec(line)[1]}.${Number(/minor:\s*(\d+)/.exec(line)[1]) + 1}.0`;
}
```
    (`readFixture` needs importing from `./support/harness.js` if this file does not already import it — check the import list; wave 1's tests did not need it.) Then in test `c`, the forward-minor coda's two expectations become:
```js
    const forwardMinor = forwardMinorVersion();
    expect(await mount(page, mapOf(BLOCK_IDS, { schema_version: forwardMinor }))).toBe(
      "controller"
    );
    expect(
      logs.some((m) => m.includes("is newer than this engine") && m.includes(forwardMinor))
    ).toBe(true);
```
    and in test `g`, `const future = mapOf(BLOCK_IDS, { schema_version: "1.3.0" });` becomes `const future = mapOf(BLOCK_IDS, { schema_version: forwardMinorVersion() });`. Neither test's *meaning* moves — `c`'s coda still proves the digit-count bound is not a newer-version refusal, `g`'s still proves the forward-minor exemption relaxes the identity check to a warning — the specimen is simply derived, so both stay true across every future bump instead of true once.
  - In `cli_smoke.rs::cli_translate_smoke`, add the §11 cheap wire pin (as sharpened by Deviation 5) after the existing `generator.name` assertion:
```rust
    // ti 490d97 wave 6 (spec §11's cheap pin, written to §3's normative
    // definition): the map declares the run's intake, and every row declares
    // its block's SPELLING — which over the SCN-14 corpus (it contains raw
    // html islands) makes the html arm non-vacuous.
    assert_eq!(
        alignment["input_format"].as_str(),
        Some("markdown"),
        "a Markdown run's map declares input_format markdown"
    );
    for row in alignment["blocks"].as_array().expect("blocks array") {
        let expected = if row["block_kind"].as_str() == Some("html") {
            "html"
        } else {
            "markdown"
        };
        assert_eq!(
            row["source_format"].as_str(),
            Some(expected),
            "row {}: source_format is the block's spelling",
            row["source_block_id"]
        );
    }
```
- [ ] **Step 6: Green, docs, commit.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-green.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-cli-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-cli-green.txt
```
Expected: `CARGO_EXIT=0` in both — the whole inventory of Step 4 green with **no non-inventory edit having been needed** (if one was, it is a finding for DCR-0038, not a silent fix). The two JS specimens are Playwright-side; Step 7's browser run is their gate.
  Then rewrite `docs/architecture/contracts.md` §3 in the same commit:
  - the heading: `## 3. AlignmentMap JSON (durable wire — `schema_version 1.3.0`)`; the example JSON gains `"input_format": "markdown"` after `"detected_source_language"` and `"source_format": "markdown"` on the first example row (and `"source_format"` on the second);
  - the Stability bullet's current-version sentence: current version is **`1.3.0`** — bumped from `1.2.0` by three additive changes (ti `490d97` / DCR-0038): the per-row `source_format` (the block's **spelling** — a Markdown run's html-island row says `"html"`; reader rule for a pre-1.3.0 row with the field absent: `block_kind == "html" ? html : markdown`, never a bare markdown default), the map-level `input_format` (the run's **intake**, what a pane consumer branches on — two facts, two names, never one field doing double duty), and the `"title"` `block_kind` value;
  - a `title` bullet beside the thematic-break clause in the `block_kind` list (which itself gains `title`): **`title`** (schema 1.3.0, D5): an HTML document's `<title>` — real translatable content with a real unit and a real row, `sync_role: "non-sync"` because browser chrome renders it and there is nothing in a pane to anchor; the same row shape a thematic break has carried since schema 1.0, which is why a 1.2.0-era engine classifies it with no change;
  - the **normativity note**, verbatim intent from spec §10: every "schema 1.x" statement in this section applies unchanged — a 1.2.0-era engine reads a 1.3.0 map under the existing forward-minor policy: no new `sync_role` values were added, and both new fields are outside the five facts the minimum-usable-row gate polices.
  Commit (one commit — the weld set):
```bash
cargo fmt --all
git add crates/transync-syntax/src/align.rs crates/transync-syntax/src/render/attrs.rs \
  web/js/sync.js crates/transync-cli/web/sync.js web/js/wasm-demo.js \
  crates/transync-wasm/src/engine.rs crates/transync-cli/tests/cli_smoke.rs \
  web/tests/engine.spec.js \
  crates/transync/tests/scenarios/scn_01_headings_and_paragraphs.rs \
  crates/transync/tests/scenarios/scn_02_table_small.rs \
  crates/transync/tests/scenarios/scn_03_table_large.rs \
  crates/transync/tests/scenarios/scn_04_code_block.rs \
  crates/transync/tests/scenarios/scn_05_nested_list.rs \
  crates/transync/tests/scenarios/scn_06_blockquote.rs \
  crates/transync/tests/scenarios/scn_07_validation_retry.rs \
  crates/transync/tests/scenarios/scn_08_fallback.rs \
  crates/transync/tests/scenarios/scn_09_prompt_injection.rs \
  crates/transync/tests/scenarios/scn_10_long_document.rs \
  crates/transync/tests/scenarios/scn_11_language_auto.rs \
  crates/transync/tests/scenarios/scn_14_full.rs \
  crates/transync/tests/scenarios/scn_15_html_blocks.rs \
  docs/architecture/contracts.md
# add crates/transync/tests/scenarios/scn_16_html_document.rs to the list IF the
# fresh inventory showed it pinning the version
git commit -m "feat(align): schema 1.3.0 — the row says the spelling, the map says the intake

Three additive changes ride one bump: per-row source_format (Option only so
a pre-1.3.0 map deserializes; the documented reader rule for an absent value
is block_kind == html ? html : markdown, because a 1.2.0 island row IS html),
map-level input_format (#[serde(default)] reads an old map as the Markdown
run it actually was), and the title kind's non-sync row. Forward-minor by
construction: no new sync_role value, both fields outside the engine's five
policed row facts — a 1.2.0-era engine reading a 1.3.0 map warns and drives.
The three JS schema mirrors move in the same commit (sync_js_drift is the
weld; the CLI twin is refreshed by cp), and the sanctioned expectation sweep
is exactly the version literals: 13 scenario asserts, cli_translate_smoke,
the wasm engine test, and engine.spec.js's two forward-minor specimens
(tests c and g), rewritten from a hard-coded "1.3.0" to a version derived
from the served engine — the bump made their literals same-version, and a
derived specimen cannot die that death again. cli_translate_smoke
additionally pins the wire:
input_format markdown, and every row's source_format equal to its kind's
spelling over a corpus that contains islands.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §3, §10, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
Expected: the hook's four gates pass; no `--no-verify`.

- [ ] **Step 7: The forward-minor proof — engine.spec.js test `m`.** Append to `web/tests/engine.spec.js`, after wave 1's test `l` and before the closing `});`:
```js
  test("m — a one-minor-newer map carrying 1.3.0's additions still drives, with the forward-drift warning", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // ti 490d97 wave 6: the acceptance criterion "a 1.2.0-era engine reading
    // a 1.3.0 map still drives", stated durably — the engine under test reads
    // a map one minor NEWER than its own KNOWN_SCHEMA that carries exactly
    // the 1.3.0 additions: per-row source_format, map-level input_format,
    // and a title row with the non-sync role. forwardMinorVersion() (Step
    // 5's helper — the one home) derives the specimen from the served
    // sync.js, keeping this the same test after every future bump.
    const map = mapOf(BLOCK_IDS, {
      schema_version: forwardMinorVersion(),
      input_format: "html",
    });
    for (const row of map.blocks) row.source_format = "html";
    map.blocks.push({
      source_block_id: "title-0001",
      target_block_id: "title-0001",
      block_kind: "title",
      sync_role: "non-sync",
      // Wire spelling (§3): this row is the 1.3.0 specimen, not a rig row —
      // the rig's `order` shorthand would undercut "carrying exactly the
      // 1.3.0 additions".
      source_order: map.blocks.length,
      target_order: map.blocks.length,
      fallback_status: "translated",
      source_format: "html",
    });

    expect(await mount(page, map)).toBe("controller");
    expect(
      logs.some((m) => m.includes("is newer than this engine")),
      "the forward-drift warning is the policy's visible half"
    ).toBe(true);
    expect(logs.some((m) => m.includes("rejecting alignment map"))).toBe(false);
    // The title row is inert in every direction: no drift warning about its
    // missing anchor (non-sync rows are skipped), and no anchor claims it.
    expect(logs.some((m) => m.includes("title-0001"))).toBe(false);

    // And it DRIVES: the reader lands on the third block, the follower comes.
    const third = await offsetTopOf(page, TGT, "e-0003");
    await setScrollTop(page, SRC, await offsetTopOf(page, SRC, "e-0003"));
    await waitForScrollNear(page, TGT, third, 6);

    expect(errors).toEqual([]);
  });
```
  (`forwardMinorVersion` and its `readFixture` import landed in Step 5; by this step both already exist — nothing new to import here.)
  *What implementation bug makes this red:* a `validateRows` tightened to reject unknown fields or unknown `block_kind`; a broken `forwardDrift` comparison refusing newer-minor maps; a non-sync row entering the drift warning or the row-id gate; a future KNOWN_SCHEMA bump that forgets the constant's shape (the regex fails loudly). If it is red at first run, the engine — not the test — moved.
```bash
scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave6/gate/t2-browser.txt 2>&1
echo "SUITE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t2-browser.txt
```
Expected: `SUITE_EXIT=0` — the new test green; every wave-1 engine/scn13 case green, tests `c` and `g` now running on their Step 5 derived specimens (this run is the first that could ever have surfaced their old hard-coded reds, and the sweep already closed them); and the wasm demo leg green (`test-browser.sh` rebuilds the wasm module itself, so `schema_version()` already answers 1.3.0 against the bumped `wasm-demo.js` mirror). Commit:
```bash
git add web/tests/engine.spec.js
git commit -m "test(web): the forward-minor policy is a driven proof, not a sentence

A map one minor newer than the engine's own KNOWN_SCHEMA — carrying exactly
schema 1.3.0's additions: per-row source_format, map-level input_format, a
title row on the non-sync role — mounts with the forward-drift warning,
stays silent about the anchor-less title row, and drives the follower.
Deriving the version from the served sync.js keeps the pin true across
every future bump instead of true once.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §10; OI-0024)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The pane derivation — `render/html_pane.rs`

**Files:**
- Create: `crates/transync-syntax/src/render/html_pane.rs`
- Modify: `crates/transync-syntax/src/render.rs` (the two wiring lines + the mirror `Document.format` guard in `render_fragment`), `crates/transync-syntax/src/intake/html.rs` (additive: the 2026-08-21 ruling's `is_named_in_the_tables` predicate + its named-DEFAULT-STOP const), `docs/implementation/module-map.md`, `CLAUDE.md` (the `render` row)

**Interfaces:**
- Produces, for Task 4:
  ```rust
  // re-exported from crate::render
  pub fn render_source_html(
      doc: &Document,
      alignment: &AlignmentMap,
      outcomes: &HashMap<BlockId, HtmlOutcome>,
  ) -> Result<String, RenderError>;
  pub fn render_target_html(
      doc: &Document,
      translated_html: &str,
      alignment: &AlignmentMap,
      outcomes: &HashMap<BlockId, HtmlOutcome>,
  ) -> Result<String, RenderError>;
  ```
- Consumes: `super::{PaneCtx, Pane, RenderError, block_text, html_escape, attrs, FRAGMENT_OPEN, FRAGMENT_CLOSE}` (all private-to-`render`, reachable from the child module — that reachability is Deviation 1's ground); `crate::walk::normalize_top_level` (Deviation 2); `crate::outcome::HtmlOutcome` (Deviation 3); `crate::parser::intake` (Deviation 8); `crate::intake::html::is_named_in_the_tables` (the 2026-08-21 ruling — Deviation 4); `transync_html::{strip_reserved_sync_attrs, balance_fragment, element_extents}`.
- **The seven §8 steps, mapped to code:** slice (`block_text` after `PaneCtx::new`'s refusals) → strip → inject-or-wrap → balance → group; gaps/head/doctype/comments/script/style unreachable by construction (only row ranges are ever read); fallback-live vs extraction-placeholder keyed on `outcomes`.
- **One shape to verify at execution, marked unverified here:** `ElementExtent`'s exact field semantics for void/closeless elements were refined in wave 3 (spec §15 item 2 delegates them). Step 3's "one-element block" rule states the *decision*; read the landed `ElementExtent` doc in `crates/transync-html/src/lib.rs` first and adjust the coverage-end expression (`close` end vs `content_end` vs tag end) to the landed contract — Step 1's tests are the arbiter: `anchors_land_…` (closed elements) and `an_unclosed_block_…` (force-closed) arbitrate the expression itself, and `a_lone_void_img_takes_the_wrapper_not_its_own_open_tag` pins the boundary's far side — a void records **no** extent, so it must take the wrapper arm and never reaches the expression — while `an_unknown_element_block_takes_the_wrapper_not_its_own_open_tag` pins the ruling's near side: a complete extent whose element §4's tables do not know still takes the wrapper, whatever the coverage expression says.

- [ ] **Step 1: Declare the module and write the file with its doc and tests — no implementation yet.** In `crates/transync-syntax/src/render.rs`, after `pub mod attrs;`:
```rust
pub mod html_pane;
pub use html_pane::{render_source_html, render_target_html};
```
Create `crates/transync-syntax/src/render/html_pane.rs` opening with this module doc (doc-link rule: never intra-doc-link a private item — `PaneCtx`, `block_text` and the fragment consts are named in plain backticks; `parser::intake`, `walk::normalize_top_level`, the `transync_html` functions and `RenderError` are `pub` and may be linked — the rustdoc gate runs `-D warnings` on this crate):
```rust
//! HTML-source pane derivation (ti 490d97, spec 2026-08-20 §8).
//!
//! Panes are **synthesized `<main>` fragments, not annotated whole
//! documents** — four reasons, all tracing to locked decisions: D6 (panes
//! are a sync surface; fidelity is `transync serve` over `out.html`),
//! contracts §4a (single `<main>`, every block a direct child, in source
//! order — so anchor survival, one-element-per-anchored-row, `offsetTop`
//! math and the offsetParent probe carry over with zero amendment), the
//! by-construction death of script/style/head in the mount, and OI-0035's
//! shrunken surface (gap markup is exactly where impostor anchors would
//! ride).
//!
//! One worker serves both panes — the source pane over
//! `Document::source_text`, the target pane over the regenerated HTML —
//! per alignment row with `sync_role != "non-sync"`, in source order:
//!
//! 1. slice the block's bytes by the row's range (`PaneCtx::new`'s three
//!    refusals — duplicate row, uncovered block, unusable range — carry
//!    over VERBATIM: this module calls the same constructor);
//! 2. **strip** the reserved attribute namespace
//!    ([`transync_html::strip_reserved_sync_attrs`]) — strip-then-inject
//!    is what makes "ours are the only sync attributes" a construction
//!    rather than a scan; reversing the order strips the anchors this
//!    module just injected and silently reopens OI-0035;
//! 3. **inject** the four canonical attributes (`attrs::write_attrs`, so
//!    the two formats' spelling and order cannot fork) into the block's
//!    OWN outermost element's open tag — when §4's tables KNOW that
//!    element (`intake::html::is_named_in_the_tables`). No wrapper
//!    `<div>` for tables or code here: the Markdown path's wrapper exists
//!    because comrak output could not carry attributes (ADR-0007 /
//!    DCR-0001); an HTML block's own element can. Deliberate, documented
//!    divergence — do not "fix" it back;
//! 4. wrap the rest in a transparent `<div{attrs}>…</div>` — the shipped
//!    DCR-0016 precedent. Three populations: an element-less block (a
//!    rule-T text run, a block missing its open tag); an img-run `Image`
//!    block (a void carries no extent, so even a lone `<img>` is
//!    element-less to the instrument); and — the 2026-08-21 ruling — a
//!    block whose outermost element is UNKNOWN to §4's tables (a custom
//!    element, a future element). The ruling's ground: the shipped shell
//!    mounts panes through DOMPurify's untouched fail-closed config,
//!    which removes an unknown element and every attribute riding it, so
//!    an anchor injected into `<x-note>` dies in the mount and the pane
//!    grows a hole. The key is §4's tables, NEVER the sanitizer's
//!    allowlist: duplicating DOMPurify's allowlist in Rust would be a
//!    second opinion about what the sanitizer accepts — the same class of
//!    sin as a second Markdown parser — while §4 already classifies a
//!    custom element as unknown, which is exactly why D4 makes it a
//!    boundary. Deliberately broader than "custom elements": any element
//!    §4 does not know fails safe into the wrapper as HTML evolves;
//! 5. **balance** ([`transync_html::balance_fragment`]) so a malformed
//!    block cannot swallow the next block's anchor after the DOMPurify
//!    innerHTML mount;
//! 6. group consecutive `li` blocks under ONE shared `<ul>`/`<ol>` via
//!    `walk::normalize_top_level`'s collapse — under D9 this is mandatory,
//!    not cosmetic. D9 makes the list container gap: `<ul>`/`<ol>` carry no
//!    text and no anchor of their own, only the items do. So without the
//!    group the items land as direct `<main>` children — invalid HTML with
//!    no list semantics, rendered unlike the Markdown pane, which has always
//!    grouped them via `render_list_group`. Two panes that disagree about
//!    list structure cannot be compared by eye, which is what panes are for.
//!    (NOT a sanitizer behaviour: spec §8 step 6 says DOMPurify relocates a
//!    bare `<li>`, and that is measured-false — 3.2.6 passes it through
//!    unchanged, in place. Relocation is a table-family parser behaviour,
//!    `<tr>` outside `<table>`. Amendment I corrects the spec; the
//!    requirement stands on D9's model, not on the sanitizer.)
//!
//! Gap bytes, `<head>`, doctype, comments, `<script>`/`<style>` are never
//! emitted — only row ranges are read, so they are unreachable rather than
//! filtered. A `fallback_source` block renders LIVE (its bytes are source
//! bytes, already valid HTML) with `data-fallback="fallback_source"`;
//! ONLY an extraction-failure block (per the run's `HtmlOutcome` map)
//! takes the escaped `<pre data-skipped="html-block">` placeholder,
//! mirroring DCR-0016's failure presentation. Non-sync rows — the
//! `<title>` and `<hr>` — reach no pane at all: §8's own per-row rule
//! (`sync_role != "non-sync"`), with D5/D6 as its grounds and §13 item 6
//! recording the title's half.
//!
//! The published `out.html` is `regen::regenerate` output — full page,
//! head, scripts, doctype, ANCHOR-FREE. Anchors never touch the regen
//! path; this module reads `&str`s and returns a new `String`, and the
//! same `block_id` flows IR → LLM → regenerated HTML → alignment row →
//! this injection (which reads the ROW, never the document's own claims)
//! → DOM anchor → engine. The document never votes on its own anchors.
//!
//! TRACE: ADR-0025
//! TRACE: contracts.md §4, §4a
```
Then the imports and the test module (the functions the tests call do not exist yet — that is Step 2's red):
```rust
use super::{FRAGMENT_CLOSE, FRAGMENT_OPEN, Pane, PaneCtx, RenderError, attrs, block_text, html_escape};
use crate::align::{AlignmentMap, SyncRole};
use crate::id::{BlockId, BlockKind, SourceFormat};
use crate::intake;
use crate::outcome::HtmlOutcome;
use crate::parser::{self, Block, Document};
use crate::walk;
use std::collections::HashMap;
use std::fmt::Write;
```
and, at the foot of the file:
```rust
// ti 490d97 wave 6 (spec §8): every trap in the derivation has a test that
// names the bug that would spring it. Statuses are pinned to Preserved by
// the helper so `data-fallback` is deterministic; tests that care about a
// status build their own map.
#[cfg(test)]
mod html_pane_tests {
    use super::*;
    use crate::align::{FallbackStatus, build_alignment_map};
    use crate::id::assign_block_ids;
    use crate::intake;
    use crate::outcome::html_outcomes;
    use crate::regen;

    struct Rig {
        doc: Document,
        map: AlignmentMap,
        translated: String,
        outcomes: HashMap<BlockId, HtmlOutcome>,
    }

    /// Parse with the HTML intake, regenerate the identity document, and
    /// build the map with every block Preserved — so `data-fallback` is
    /// deterministic in the structural tests. The fallback/extraction test
    /// below builds its own statuses instead.
    fn rig(src: &str) -> Rig {
        let mut doc = intake::html::parse(src);
        assign_block_ids(&mut doc);
        let (translated, offsets) = regen::regenerate(&doc, &HashMap::new());
        let outcomes = html_outcomes(&doc);
        let statuses: HashMap<BlockId, FallbackStatus> = doc
            .blocks
            .iter()
            .map(|b| (b.block_id.clone(), FallbackStatus::Preserved))
            .collect();
        let map = build_alignment_map(&doc, &statuses, &offsets, "auto", "ko", None, &outcomes);
        Rig { doc, map, translated, outcomes }
    }

    /// §8 step 3 + Deviation 4: the anchor lands on the block's OWN element
    /// — h2, p, table, pre, blockquote — never on a wrapper div. The absent
    /// `<div data-sync-id="t-` is the pin against "fixing" the divergence
    /// back to the Markdown path's ADR-0007 wrapper.
    #[test]
    fn anchors_land_on_the_blocks_own_elements_with_no_wrapper_div() {
        let r = rig(
            "<h2>Head</h2>\n<p>Body</p>\n<table><tr><td>x</td></tr></table>\n\
             <pre><code>let a;</code></pre>\n<blockquote><p>q</p></blockquote>\n",
        );
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(pane.starts_with(FRAGMENT_OPEN), "{pane}");
        assert!(pane.ends_with(FRAGMENT_CLOSE), "{pane}");
        for open in [
            "<h2 data-sync-id=\"h2-",
            "<p data-sync-id=\"p-",
            "<table data-sync-id=\"t-",
            "<pre data-sync-id=\"c-",
            "<blockquote data-sync-id=\"q-",
        ] {
            assert!(pane.contains(open), "missing own-element anchor {open}:\n{pane}");
        }
        assert!(
            !pane.contains("<div data-sync-id=\"t-") && !pane.contains("<div data-sync-id=\"c-"),
            "the Markdown path's wrapper div must NOT come back (Deviation 4):\n{pane}"
        );
    }

    /// §8 steps 2+3, the ORDER: strip runs first, injection second, and the
    /// injection scans the STRIPPED bytes. Inject-then-strip deletes our own
    /// anchors (count 0); no strip leaves two claimants for one id (count 2).
    /// Either wrong order is a red here — this is the OI-0035 interlock.
    #[test]
    fn strip_then_inject_makes_ours_the_only_sync_attributes() {
        let r = rig("<p data-sync-id=\"p-9999\" DATA-Order=\"7\" class=\"k\">text</p>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert_eq!(
            pane.matches("data-sync-id=").count(),
            1,
            "exactly one claimant — ours:\n{pane}"
        );
        assert!(!pane.contains("p-9999"), "the impostor id is gone:\n{pane}");
        assert!(!pane.contains("DATA-Order"), "the strip is case-insensitive:\n{pane}");
        assert!(pane.contains("class=\"k\""), "non-namespace attributes survive:\n{pane}");
        assert!(pane.contains(">text</p>"), "content survives:\n{pane}");
    }

    /// §8 step 6 / D9: consecutive items share ONE group; adjacent lists —
    /// different ast_path prefixes — never merge; the ordered flag picks the
    /// tag. No-grouping is a mount-breaking bug, not a style choice: D9
    /// makes the container gap, so an ungrouped pane emits items as direct
    /// <main> children — invalid HTML, no list semantics, and structurally
    /// unlike the Markdown pane's render_list_group output. (The spec blames
    /// DOMPurify relocation here; that is measured-false — 3.2.6 leaves a
    /// bare <li> in place. See amendment I.)
    #[test]
    fn consecutive_items_share_one_group_and_adjacent_lists_never_merge() {
        let r = rig("<ul><li>a</li><li>b</li></ul>\n<ol><li>c</li></ol>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert_eq!(pane.matches("<ul>").count(), 1, "one shared ul:\n{pane}");
        assert_eq!(pane.matches("<ol>").count(), 1, "the ol stays its own group:\n{pane}");
        assert_eq!(
            pane.matches("<li data-sync-id=\"li-").count(),
            3,
            "every item is its own anchor inside a group:\n{pane}"
        );
        let ul_end = pane.find("</ul>").expect("ul closes");
        let ol_start = pane.find("<ol>").expect("ol opens");
        assert!(ul_end < ol_start, "groups do not interleave:\n{pane}");
    }

    /// §8 step 4: a rule-T anonymous run has no outermost element, so it
    /// takes the transparent DCR-0016 wrapper; its element-bearing neighbor
    /// does not.
    #[test]
    fn an_anonymous_run_takes_the_transparent_div_wrapper() {
        let r = rig("<div>\nnaked run text\n<p>kept</p>\n</div>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"p-") && pane.contains(">naked run text</div>"),
            "the run rides a transparent wrapper:\n{pane}"
        );
        assert!(pane.contains("<p data-sync-id=\"p-"), "the real <p> keeps its own tag:\n{pane}");
    }

    /// §8 step 3/4's boundary, as Deviation 4 sharpens it: a void element is
    /// never pushed by the one stack walk, so `element_extents("<img …>")`
    /// is EMPTY — a lone `<img>` is one element to the eye and zero extents
    /// to the §5 instrument, and the wrapper arm takes every img-run Image
    /// block, single or multi. The bug this pins out: a "helpful" special
    /// case that injects into the void's own open tag, forking the
    /// derivation from the instrument it must defer to.
    #[test]
    fn a_lone_void_img_takes_the_wrapper_not_its_own_open_tag() {
        let r = rig("<p>before</p>\n<img src=\"badge.png\" alt=\"a\">\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"img-"),
            "the single-img Image block rides the transparent wrapper:\n{pane}"
        );
        assert!(
            !pane.contains("<img data-sync-id="),
            "injection must not land inside the void's own tag — the \
             instrument records no extent to land in (Deviation 4):\n{pane}"
        );
        assert!(
            pane.contains("data-block-kind=\"image\""),
            "the kind still names it:\n{pane}"
        );
        assert!(pane.contains("<img src=\"badge.png\""), "the img itself survives:\n{pane}");
    }

    /// §8 step 3/4's boundary, the 2026-08-21 ruling's half: a block whose
    /// outermost element is UNKNOWN to §4's tables — the SCN-16 fixture's
    /// `<x-note>`, any custom or future element — takes the transparent
    /// wrapper, never self-injection. Measured ground (wave 7's review,
    /// the vendored DOMPurify 3.2.6): sanitize over a self-injected
    /// `<x-note … data-sync-id="html-…">` removes the element AND the
    /// anchor, keeping only the text — the pane mounts with a hole in its
    /// anchor list and a warnMapDomDrift warning at every load. The
    /// wrapper `<div>` survives the same mount with its attributes intact
    /// (`div` + data-* are inside the untouched config; wave 1 measured
    /// the ALLOW_DATA_ATTR half). The bug this pins out: the pre-ruling
    /// injected_fragment — treating "IS one element" as sufficient and
    /// splicing the anchor into the unknown element's own open tag. Keyed
    /// on §4's tables, never the sanitizer's allowlist (see
    /// intake::html::is_named_in_the_tables' doc for why).
    #[test]
    fn an_unknown_element_block_takes_the_wrapper_not_its_own_open_tag() {
        let r = rig(
            "<p>before</p>\n<x-note tone=\"info\">A custom element stops, loudly.</x-note>\n",
        );
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"html-"),
            "the unknown-element block rides the transparent wrapper:\n{pane}"
        );
        assert!(
            !pane.contains("<x-note data-sync-id="),
            "injection must not land in a tag the sanitizer will remove \
             (the 2026-08-21 ruling):\n{pane}"
        );
        assert!(
            pane.contains("<x-note tone=\"info\">A custom element stops, loudly.</x-note>"),
            "the element renders LIVE inside the wrapper — transparent, \
             not an escape:\n{pane}"
        );
        assert!(
            pane.contains("data-block-kind=\"html\""),
            "the kind still names it:\n{pane}"
        );
    }

    /// §8 step 7 + D5/D6: gaps, head, doctype, comments, script/style and
    /// non-sync rows (title, hr) never reach a pane. The pass-through
    /// wrappers (header/section/body) are gap markup and are absent too —
    /// this is the whole-document-pane refusal made checkable.
    #[test]
    fn gaps_head_scripts_and_non_sync_rows_never_reach_the_pane() {
        let r = rig(
            "<!DOCTYPE html>\n<html><head><title>T</title>\n<style>p{}</style>\n\
             <script>1 < 2</script></head>\n<body><header>\n<h1>H</h1>\n</header>\n\
             <!-- note -->\n<p>body</p>\n<hr>\n</body></html>\n",
        );
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        for absent in [
            "<script", "<style", "DOCTYPE", "<title", "<head", "<header", "<body",
            "<!-- note -->", "<hr", "data-sync-id=\"title-",
        ] {
            assert!(!pane.contains(absent), "{absent} must not reach a pane:\n{pane}");
        }
        assert!(pane.contains("<h1 data-sync-id=\"h1-"), "{pane}");
        assert!(pane.contains("<p data-sync-id=\"p-"), "{pane}");
    }

    /// §8 step 7's split, keyed on the run's HtmlOutcome map (Deviation 3):
    /// a fallback_source block renders LIVE — its bytes are source bytes,
    /// already valid HTML — and only an extraction failure takes the escaped
    /// placeholder. Copying the Markdown arm's escape-on-fallback here is
    /// the bug this test exists to catch.
    #[test]
    fn fallback_renders_live_and_only_extraction_failure_takes_the_placeholder() {
        let src = "<p>alpha</p>\n<x-note>beta</x-note>\n";
        let mut doc = intake::html::parse(src);
        assign_block_ids(&mut doc);
        let (_, offsets) = regen::regenerate(&doc, &HashMap::new());
        let p_id = doc
            .blocks
            .iter()
            .find(|b| b.block_id.0.starts_with("p-"))
            .expect("p block")
            .block_id
            .clone();
        let x_id = doc
            .blocks
            .iter()
            .find(|b| b.block_id.0.starts_with("html-"))
            .expect("x-note block")
            .block_id
            .clone();
        // The paragraph FELL BACK: unit attempted, exhausted — a real
        // statuses row says so.
        let mut statuses = HashMap::new();
        statuses.insert(p_id, FallbackStatus::FallbackSource);
        // The custom element's EXTRACTION failed (forced — the real rewriter
        // handles this element fine, which is exactly why the outcome map is
        // the derivation's input). It gets NO statuses row: align derives
        // fallback_source from the ExtractionFailed outcome itself, so BOTH
        // rows read fallback_source and only the outcome map tells them
        // apart — the premise, asserted before the render.
        let mut outcomes = html_outcomes(&doc);
        outcomes.insert(x_id.clone(), HtmlOutcome::ExtractionFailed("boom".to_string()));
        let map = build_alignment_map(&doc, &statuses, &offsets, "auto", "ko", None, &outcomes);
        let x_row = map
            .blocks
            .iter()
            .find(|r| r.source_block_id == x_id)
            .expect("x-note row");
        assert_eq!(
            x_row.fallback_status,
            FallbackStatus::FallbackSource,
            "premise: the ROW cannot distinguish unit-fallback from extraction \
             failure — that is why the derivation takes the outcome map",
        );
        let pane = render_source_html(&doc, &map, &outcomes).expect("renders");
        assert!(
            pane.contains("data-fallback=\"fallback_source\">alpha</p>"),
            "a fallen block renders LIVE with the honest attribute:\n{pane}"
        );
        assert!(
            pane.contains("data-skipped=\"html-block\">&lt;x-note&gt;beta"),
            "the extraction failure takes the escaped placeholder, its own \
             bytes escaped inside it:\n{pane}"
        );
        assert!(
            !pane.contains("data-skipped=\"html-block\">alpha")
                && !pane.contains("&lt;p&gt;alpha"),
            "the fallen paragraph must NOT be escaped — that is the Markdown \
             arm's presentation, deliberately diverged from here:\n{pane}"
        );
    }

    /// §8 step 5: an unclosed leaf, balanced, cannot swallow the following
    /// block's anchor. Without balance_fragment the open <b> runs on and the
    /// appended closes never appear.
    #[test]
    fn an_unclosed_block_cannot_swallow_the_next_anchor() {
        let r = rig("<section>\n<x-note>an <b>unclosed bold\n</section>\n<p>after</p>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        let x = pane.find("data-block-kind=\"html\"").expect("x-note block renders");
        let p = pane.find("<p data-sync-id=\"p-").expect("the following anchor survives");
        assert!(x < p, "source order:\n{pane}");
        let between = &pane[x..p];
        assert!(
            between.contains("</b>"),
            "the balancer closed the dangling <b> before the next anchor:\n{pane}"
        );
    }

    /// §8 step 1: the three PaneCtx refusals carry over VERBATIM — same
    /// constructor, same errors. A derivation that bypassed PaneCtx would
    /// accept this map and render a duplicate-claimed pane.
    #[test]
    fn the_three_map_refusals_carry_over() {
        let r = rig("<p>x</p>\n");
        let mut dup = r.map.clone();
        dup.blocks.push(dup.blocks[0].clone());
        let err = render_source_html(&r.doc, &dup, &r.outcomes).expect_err("refused");
        assert!(matches!(err, RenderError::DuplicateRow { .. }), "{err}");

        let mut uncovered = r.map.clone();
        uncovered.blocks.clear();
        let err = render_source_html(&r.doc, &uncovered, &r.outcomes).expect_err("refused");
        assert!(matches!(err, RenderError::UncoveredBlock { .. }), "{err}");
    }

    /// §8 step 1 / Deviation 8: the target string passes the same
    /// parser::intake guard render_target applies to translated_md. Skipping
    /// the guard returns Ok here — the red this test exists for.
    #[test]
    fn the_target_pane_passes_the_intake_guard() {
        let r = rig("<p>x</p>\n");
        // In-bounds, ASCII, and refused by the guard's nesting bound: the
        // pathological prefix scores past MAX_BLOCK_NESTING_DEPTH.
        let bogus = format!("{}\n{}", ">".repeat(300), r.translated);
        let err =
            render_target_html(&r.doc, &bogus, &r.map, &r.outcomes).expect_err("guard refuses");
        assert!(matches!(err, RenderError::Intake(_)), "{err}");
        // And the honest control: the real regenerated string renders.
        render_target_html(&r.doc, &r.translated, &r.map, &r.outcomes)
            .expect("the real target renders");
    }

    /// The identity round-trip's pane corollary: over the identity target,
    /// source and target panes carry the same anchor ids in the same order.
    #[test]
    fn the_two_panes_carry_the_same_anchor_set_in_order() {
        let r = rig("<h2>a</h2>\n<ul><li>b</li><li>c</li></ul>\n<p>d</p>\n");
        let s = render_source_html(&r.doc, &r.map, &r.outcomes).expect("source");
        let t = render_target_html(&r.doc, &r.translated, &r.map, &r.outcomes).expect("target");
        let ids = |pane: &str| -> Vec<String> {
            pane.match_indices("data-sync-id=\"")
                .map(|(i, m)| {
                    let rest = &pane[i + m.len()..];
                    rest[..rest.find('"').expect("closed attr")].to_string()
                })
                .collect()
        };
        assert_eq!(ids(&s), ids(&t), "same ids, same order, both panes");
        assert_eq!(ids(&s).len(), 4, "h2 + two li + p");
    }
}
```
*(The `matches!` uses above are test assertions — the charter's sanctioned form.)*

- [ ] **Step 2: Watch the compile red.**
```bash
cargo test -p transync-syntax html_pane_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t3-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t3-red.txt
```
Read the file. Expected: `CARGO_EXIT=101` with `error[E0425]: cannot find function `render_source_html` in this scope` (and its sibling) at the test call sites, plus the same two names unresolved at `render.rs`'s new `pub use` — the module, doc and imports resolve; only the functions are missing. This is the wiring red.

- [ ] **Step 3: Implement the derivation.** Below the imports, before the test module:
```rust
/// Render the source pane for an HTML-intake document. See the module doc;
/// the map refusals and range discipline are `render_source`'s exactly.
pub fn render_source_html(
    doc: &Document,
    alignment: &AlignmentMap,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    render_html_fragment(doc, alignment, Pane::Source, outcomes)
}

/// Render the target pane from the regenerated translated HTML. Same block
/// set, same refusals, plus the `parser::intake` guard on `translated_html`
/// itself — the same guard `render_target` applies to `translated_md`
/// (R0002-0059; spec §8 step 1).
pub fn render_target_html(
    doc: &Document,
    translated_html: &str,
    alignment: &AlignmentMap,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    render_html_fragment(doc, alignment, Pane::Target(translated_html), outcomes)
}

fn render_html_fragment(
    doc: &Document,
    alignment: &AlignmentMap,
    pane: Pane<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    // Wave 2's handed-forward render/pane guard, discharged as a PAIR of
    // assertions — this Html half, plus the Markdown mirror at the top of
    // render_fragment (Inherited obligation 4): the LIVE routing guarantee
    // is run_pipeline's exhaustive format match; a fallible refusal here
    // would be an unreachable arm claiming to be a gate. An assertion is
    // not a dispatch (charter scope (c)).
    debug_assert_eq!(
        doc.format,
        SourceFormat::Html,
        "html_pane is the HTML-intake pane derivation; a Markdown document \
         takes render_source/render_target"
    );
    let ctx = PaneCtx::new(doc, alignment, pane)?;
    // Deviation 8: the guard is consumed for its REFUSAL; the ranges index
    // the caller's own bytes (ctx.md), exactly render_fragment's documented
    // posture — there is no parse here to consume the normalized Cow.
    if let Pane::Target(t) = pane {
        let _guarded = parser::intake(t)?;
    }

    let mut out = String::from(FRAGMENT_OPEN);
    for entry in walk::normalize_top_level(doc) {
        if entry.label == "list" {
            emit_list_group(&mut out, &entry.sources, &ctx, outcomes);
        } else if let Some((block, row)) = entry.sources.first().and_then(|id| ctx.pair(id)) {
            emit_block(&mut out, block, row, &ctx, outcomes);
        }
    }
    out.push_str(FRAGMENT_CLOSE);
    Ok(out)
}

/// One shared `<ul>`/`<ol>` for the collapsed entry's items (§8 step 6, D9).
/// The tag comes from the first row's own kind — there is no comrak node
/// here and never will be. The group carries no attributes at all: the
/// source list's own tags are GAP under D9 (its `start`/classes included —
/// §13 item 4; the pane is a sync surface, not a fidelity preview).
fn emit_list_group(
    out: &mut String,
    sources: &[BlockId],
    ctx: &PaneCtx<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) {
    // Membership predicate, sanctioned by the charter's stated carve-out and
    // byte-identical to render_list_group's shipped Guard-2 line: a "list"
    // entry's sources are ListItem by normalize_top_level's label rule, so
    // the false answer is unreachable, and false degrades to <ul> — safe.
    let ordered = sources
        .first()
        .and_then(|id| ctx.by_id.get(id))
        .is_some_and(|b| matches!(b.kind, BlockKind::ListItem { ordered: true, .. }));
    let tag = if ordered { "ol" } else { "ul" };
    let _ = writeln!(out, "<{tag}>");
    for id in sources {
        if let Some((block, row)) = ctx.pair(id) {
            emit_block(out, block, row, ctx, outcomes);
        }
    }
    let _ = writeln!(out, "</{tag}>");
}

/// One row: skip non-sync, key the placeholder on the OUTCOME (Deviation 3),
/// otherwise strip → inject-or-wrap → balance (§8 steps 2–5).
fn emit_block(
    out: &mut String,
    block: &Block,
    row: &crate::align::AlignmentBlock,
    ctx: &PaneCtx<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) {
    match row.sync_role {
        // D5/D6: nothing in the pane to anchor — the row is the record.
        SyncRole::NonSync => return,
        SyncRole::Anchor | SyncRole::Container | SyncRole::ChildOnly => {}
    }
    let bytes = block_text(ctx.md, ctx.pane.range(row, block));
    let attrs_str = attrs::write_attrs(row);
    match outcomes.get(&block.block_id) {
        Some(HtmlOutcome::ExtractionFailed(_)) => {
            // The FAILURE presentation (DCR-0016's placeholder): extraction
            // failed, so live-mounting markup the rewriter could not read is
            // not on offer. This is the ONLY escaped arm — a fallback_source
            // block whose extraction succeeded renders live below.
            let _ = writeln!(
                out,
                "<pre{attrs_str} data-skipped=\"html-block\">{}</pre>",
                html_escape(bytes),
            );
            return;
        }
        Some(HtmlOutcome::Unit) | Some(HtmlOutcome::PreservedZeroSegment) | None => {}
    }
    // §8 step 2 THEN step 3: strip first, and scan the STRIPPED bytes for
    // the injection point, so our injected attributes cannot be stripped and
    // an author's reserved-namespace attributes cannot survive.
    let stripped = transync_html::strip_reserved_sync_attrs(bytes);
    let fragment = injected_fragment(stripped.trim(), &attrs_str);
    let _ = writeln!(out, "{fragment}");
}

/// §8 steps 3–5, under the 2026-08-21 ruling's three-way boundary. The
/// block self-injects iff exactly one depth-0 extent opens at the trimmed
/// start, reaches the trimmed end (its close span's end, or — for a
/// closeless extent: void, self-closed, or force-closed at EOF — the
/// extent's own end per the landed `ElementExtent` contract), AND names an
/// element §4's tables know (`intake::html::is_named_in_the_tables`).
/// Then the four attributes splice into that open tag just before its `>`
/// or `/>`; the open span here IS `TagToken::Open.span` — `element_extents`
/// is the same walk (§5), so the spec's instrument is what is read.
/// Anything else — bare text, a multi-element run, an orphan close, an
/// element UNKNOWN to the tables (the shell's DOMPurify mount would remove
/// it, anchor and all — the ruling) — takes the transparent wrapper (§8
/// step 4). Both arms balance (§8 step 5).
fn injected_fragment(trimmed: &str, attrs_str: &str) -> String {
    let extents = transync_html::element_extents(trimmed);
    let top: Vec<&transync_html::ElementExtent> =
        extents.iter().filter(|e| e.depth == 0).collect();
    if let [only] = top.as_slice() {
        let coverage_end = only.close.map_or(only.content_end.max(only.open.1), |c| c.1);
        // The 2026-08-21 ruling: self-injection only into an element §4's
        // tables know. An unknown element — custom, future — takes the
        // wrapper arm below: the shipped shell's DOMPurify mount removes an
        // unknown element and every attribute on it, so an anchor injected
        // here would die in the sanitizer and the pane would mount with a
        // hole. Keyed on §4's tables, never on the sanitizer's allowlist —
        // is_named_in_the_tables' doc says why re-keying it is the
        // second-opinion sin.
        if only.open.0 == 0
            && coverage_end == trimmed.len()
            && intake::html::is_named_in_the_tables(&only.name)
        {
            let (_, open_end) = only.open;
            let insert_at = if trimmed[..open_end].ends_with("/>") {
                open_end - 2
            } else {
                open_end - 1
            };
            let injected = format!(
                "{}{}{}",
                &trimmed[..insert_at],
                attrs_str,
                &trimmed[insert_at..]
            );
            return transync_html::balance_fragment(&injected);
        }
    }
    format!(
        "<div{attrs_str}>{}</div>",
        transync_html::balance_fragment(trimmed)
    )
}
```
  **Execution note on `injected_fragment`:** the `coverage_end` expression is written against the planning-time reading of `ElementExtent` (`close: Option<(usize, usize)>`, `content_end: usize`); wave 3 refined the field semantics (spec §15 item 2). Read the landed doc comments in `crates/transync-html/src/lib.rs` before keeping or adjusting the expression — Step 1's tests (`anchors_land_…` covers closed elements, `an_unclosed_block_…` covers force-closed, `a_lone_void_img_…` covers the void directly — no extent recorded, the wrapper arm, never the expression — and Task 5's end-to-end adds the SCN-16 badge pair's multi-img case) are the arbiter, and an adjustment here is expected, not a deviation.

  **The ruling's red, named:** implement `injected_fragment` without the `is_named_in_the_tables` conjunct — the pre-ruling shape, "IS one element" treated as sufficient — and `an_unknown_element_block_takes_the_wrapper_not_its_own_open_tag` goes red at its **first** assertion, `the unknown-element block rides the transparent wrapper`, with the printed pane showing `<x-note tone="info" data-sync-id="html-…" data-block-kind="html" …>`: the anchor spliced into the very tag DOMPurify removes. That is the disqualifying defect wave 7's review measured in headless Chromium; the conjunct is what this test holds in place, and reverting it re-opens a mount-time anchor hole, not a cosmetic choice.

  **Step 3's second file — the mirror guard (Inherited obligation 4's other half).** In `crates/transync-syntax/src/render.rs` (already touched by Step 1's two wiring lines), at the top of `render_fragment`, immediately before the `PaneCtx::new` call:
```rust
    // Wave 2's handed-forward render/pane guard, the Markdown mirror
    // (ADR-0025; spec §3 names the Markdown renderer among the format-
    // committed consumers that must refuse the wrong document). Without
    // this, comrak happily parses an HTML-intake document as Markdown,
    // Guard-2 degrades every row to escaped byte ranges, and a plausible-
    // looking pane comes out with no refusal anywhere — render_source is
    // pub in a published crate and the wasm rebuild path is shipped
    // precedent for out-of-pipeline calls, so the assert is not dead code.
    // Same posture as the Html half: an assertion, not a dispatch.
    debug_assert_eq!(
        doc.format,
        SourceFormat::Markdown,
        "render_fragment is the Markdown pane path; an HTML-intake document \
         takes render_source_html/render_target_html"
    );
```
(add `SourceFormat` to `render.rs`'s existing `crate::id` import if absent.) No existing test trips it — every shipped render test's document is comrak-parsed, so `format == Markdown` throughout — and no new fallible arm appears: the pair discharges wave 2's hand-forward in both directions, which is what DCR-0038 records.

  **Step 3's third file — the §4-tables predicate (the 2026-08-21 ruling's one cross-module addition).** In `crates/transync-syntax/src/intake/html.rs`, beside the classification consts (waves 2/3 own this file; this addition is sanctioned by the ruling, additive only — no table entry moves):
```rust
/// The DEFAULT-STOP class's NAMED members (spec §4). Custom and future
/// elements share the class by fallthrough, not by name — the distinction
/// `is_named_in_the_tables` exists to read.
const DEFAULT_STOP_NAMED: &[&str] = &[
    "summary", "dt", "dd", "address", "dialog", "iframe", "noscript",
    "template", "legend",
];

/// Does spec §4 KNOW this element — is it named in any classification
/// table, the walker-owned names (`li`, `hr`, `title`, `head`) included?
/// `false` is D4's "unknown stop": a custom or future element, DEFAULT-STOP
/// by fallthrough rather than by name. The pane derivation keys its
/// wrapper arm on this answer (the 2026-08-21 ruling): the shipped shell
/// mounts panes through DOMPurify, which removes an unknown element and
/// every attribute riding it, so a sync anchor must never be injected into
/// one. The key is DELIBERATELY this module's tables and not the
/// sanitizer's allowlist — duplicating DOMPurify's allowlist in Rust would
/// be a second opinion about what the sanitizer accepts, the same class of
/// sin as a second Markdown parser; §4 already classifies a custom element
/// as unknown, which is exactly why D4 makes it a boundary, and any element
/// HTML grows later fails safe into the wrapper.
pub(crate) fn is_named_in_the_tables(name: &str) -> bool {
    is_pass_through(name)
        || is_verbatim(name)
        || is_phrasing(name)
        || semantic_stop_kind(name).is_some()
        || ["li", "hr", "title", "head"].contains(&name)
        || DEFAULT_STOP_NAMED.contains(&name)
}
```
  (The helper names above are quoted from wave 3's plan; if the landed `intake/html.rs` spells its §4 tables differently, implement the predicate over the landed spellings — the obligation is one answer sourced from the tables' one home, not these exact identifiers. `contains` over a str slice is a membership test, not an enum dispatch — the charter is not in play.)

- [ ] **Step 4: Green, then the crate suite.**
```bash
cargo test -p transync-syntax -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t3-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t3-green.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave6/gate/t3-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t3-wasm.txt
```
Expected: `CARGO_EXIT=0` in both — the twelve `html_pane_tests` pass, every existing render/align/walk test passes untouched (the mirror guard fires on none of them: their documents are comrak-parsed, `format == Markdown` throughout), and the wasm gate holds (the new module rides `transync-syntax`, whose dependency set did not move).

- [ ] **Step 5: The two doc rows, then commit.** In `docs/implementation/module-map.md`, insert a `│   ├── html_pane.rs` row in the `render/` subtree (beside `attrs.rs`, alphabetical), described as "the §8 HTML-source pane derivation (strip → inject → balance → group)". In `CLAUDE.md`'s `transync-syntax` module-split list, the `render` row's "(+ `render/attrs`)" becomes "(+ `render/attrs`, `render/html_pane` — the HTML-source pane derivation, DCR-0038)".
```bash
cargo fmt --all
git add crates/transync-syntax/src/render.rs crates/transync-syntax/src/render/html_pane.rs \
  crates/transync-syntax/src/intake/html.rs \
  docs/implementation/module-map.md CLAUDE.md
git commit -m "feat(render): the HTML pane derivation — strip, inject into the block's own tag, balance, group

Panes are synthesized <main> fragments, never annotated whole documents
(D6; contracts §4a carries over with zero amendment). One worker serves
both panes behind render_source_html/render_target_html, reusing PaneCtx
literally so the three map refusals carry over verbatim, and the target
string passes the same parser::intake guard render_target applies. Strip
runs FIRST and injection scans the stripped bytes — the order is the
OI-0035 construction, and the interlock test fails in both wrong orders.
No wrapper div for a block that IS one element the §4 tables know: the
Markdown wrapper existed because comrak output could not carry attributes;
an HTML block's own tag can — deliberate divergence, pinned so nobody
fixes it back. An element the tables do NOT know — a custom element, a
future one — takes the transparent wrapper instead (the 2026-08-21
ruling): the shipped shell's DOMPurify mount removes an unknown element
and every attribute on it, so a self-injected anchor would die in the
sanitizer; keyed on §4's tables via intake::html's own predicate, never
on the sanitizer's allowlist.
Consecutive li blocks share one ul/ol via walk::normalize_top_level's
collapse (D9 makes the container gap, so ungrouped items would land as
direct main children - invalid HTML, no list semantics, and unlike the
Markdown pane's render_list_group output; NOT the sanitizer relocation
the spec claims, which is measured-false); the collapse has one home,
and wave 3 pinned its totality over HTML-intake documents. A fallback_source block renders LIVE; only an
extraction failure — known only to the run's HtmlOutcome map, which is
why the derivation takes it — takes the escaped placeholder. Wave 2's
handed-forward Document.format render/pane guard discharges as a pair of
debug asserts: the Html half atop the derivation, the Markdown mirror
atop render_fragment — spec §3's named consumer — same posture, no new
fallible arm.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §8; ADR-0025; OI-0035)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: `run_pipeline`'s Html arm fills the panes, the two pre-authorized flips, and contracts §4/§4a

**Files:**
- Modify: `crates/transync-core/src/pipeline.rs`, `crates/transync-core/src/lib.rs` (doc-truth), `crates/transync/tests/scenarios/scn_16_html_document.rs`, `docs/architecture/contracts.md` (§4, §4a)

**Interfaces:**
- Consumes from Task 3: `crate::render::{render_source_html, render_target_html}` (via core's existing `pub(crate) use transync_syntax::render;` family — extend the `use crate::render::{render_source, render_target};` import line with the two new names).
- Consumes from wave 5 (quote, from its Task 6 Step 4 — verify against the landed file): the render dispatch's Html arm is `crate::id::SourceFormat::Html => (String::new(), String::new()),` under the comment "ti 490d97 wave 5, deviation 5: the pane derivation for HTML runs is wave 6's".
- The run's `html_outcomes` map is already in scope at the render site (it is computed once near the top of `run_pipeline` and consumed by `build_alignment_map` a few lines above the render calls — verified at planning time).

- [ ] **Step 1: Flip the two pre-authorized pins — RED FIRST.** In `crates/transync-core/src/pipeline.rs`, `html_run_tests::an_echo_html_run_is_byte_identical_with_empty_panes`: rename to `an_echo_html_run_is_byte_identical_and_the_panes_are_real`, and replace the empty-pane assertion (wave 5's `out.annotated_source_html.is_empty() && out.annotated_target_html.is_empty()` block, message "panes are wave 6's (D6)…") with:
```rust
        assert!(
            out.annotated_source_html.starts_with("<main>")
                && out.annotated_target_html.starts_with("<main>"),
            "wave 6: the panes are the §8 synthesized fragments now",
        );
        assert!(
            out.annotated_source_html.contains("data-sync-id=\"p-")
                && out.annotated_target_html.contains("data-sync-id=\"p-"),
            "anchored rows reach both panes: {}",
            out.annotated_source_html,
        );
```
In `crates/transync/tests/scenarios/scn_16_html_document.rs`, find the empty-pane assertion (wave 5's comment: "panes themselves are wave 6's; both empty here") and replace it the same way, adding one SCN-16-specific line:
```rust
    assert!(
        !output.annotated_source_html.contains("<script")
            && !output.annotated_target_html.contains("<script"),
        "the fixture's <script> is gap and never reaches a pane (D6)",
    );
```
Run and capture the reds:
```bash
cargo test -p transync-core html_run_tests -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t4-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t4-red.txt
cargo test -p transync scn_16 -- --test-threads=4 >> /Volumes/Temp/claude/ti490d97-wave6/gate/t4-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t4-red.txt
```
Expected: `CARGO_EXIT=101` twice, both failing the new `starts_with("<main>")` assertion with the observed value the empty string — the honest behavioural red: the pipeline still returns wave 5's empty pair.

- [ ] **Step 2: Fill the arm.** In `run_pipeline`, the render dispatch's Html arm becomes:
```rust
        // ti 490d97 wave 6 (spec §8): the pane derivation. Same refusal
        // mapping as the Markdown arm; the run's html_outcomes map rides
        // along so the extraction-failure placeholder and the batching /
        // alignment decisions cannot disagree (outcome.rs's agreement rule,
        // fourth consumer).
        crate::id::SourceFormat::Html => (
            render_source_html(&doc, &alignment_map, &html_outcomes)
                .map_err(render_fault("source"))?,
            render_target_html(&doc, &translated_document, &alignment_map, &html_outcomes)
                .map_err(render_fault("target"))?,
        ),
```
and the import line gains the two names: `use crate::render::{render_source, render_source_html, render_target, render_target_html};`. Update the arm's wave-5 comment (it said "empty panes until wave 6") — the replacement comment above is the whole of it.

- [ ] **Step 3: Doc-truth in `crates/transync-core/src/lib.rs`.** Two edits, no behavior: `TranslationOutput.annotated_source_html`/`annotated_target_html`'s shared "empty on an `input_format = Html` run until wave 6" note becomes: "On an `input_format = Html` run these are the §8 synthesized `<main>` fragments — strip-then-inject over the blocks' own known elements (a block whose outermost element §4's tables do not know rides the transparent wrapper, as do element-less blocks — the 2026-08-21 ruling), `li` groups shared, head/gap/script bytes never present (D6). On a Markdown run they are the comrak-paired fragments, unchanged." And `TranslateOptions.input_format`'s "The CLI's `--input-format` flag … are the NEXT wave's (wave 6); on an `Html` run this release, [`TranslationOutput`]'s two annotated pane fields come back empty" paragraph becomes: "The CLI flag is `--input-format` (contracts §6); the pane fields carry the §8 fragments since DCR-0038."

- [ ] **Step 4: Green.**
```bash
cargo test -p transync-core -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t4-core.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t4-core.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t4-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t4-green.txt
```
Expected: `CARGO_EXIT=0` in both — the two flipped tests pass, and **every Markdown-side pipeline test passes byte-untouched** (the Markdown arm did not move; the diff shows exactly one arm and one import line in `pipeline.rs`).

- [ ] **Step 5: Contracts §4 + §4a, same commit.** In `docs/architecture/contracts.md`:
  - §4 gains a subsection after the "HTML block (schema 1.2.0 …)" bullet, titled **HTML-source panes (schema 1.3.0, ti 490d97 / DCR-0038)**, carrying: panes for an `input_format: "html"` run are synthesized `<main>` fragments (never annotated whole documents — D6; the four-reason §8 derivation summarized); the four attributes land on the block's **own outermost element's open tag** — a deliberate divergence from the Markdown wrapper rules above, because ADR-0007's wrapper existed for comrak output that could not carry attributes and an HTML block's own element can; the transparent `<div>` survives for element-less blocks (text runs, img-run `Image` blocks — a void element records no extent, so even a lone `<img>` reads as element-less to the §5 walk — and blocks missing their open tag) **and for any block whose outermost element is unknown to §4's element tables** (a custom element, a future element — the 2026-08-21 ruling): the shells mount panes through DOMPurify's untouched fail-closed config, which removes an unknown element and every attribute riding it, so a sync anchor never rides a tag the sanitizer will not keep — and the rule is keyed on §4's tables, never on the sanitizer's allowlist, because duplicating DOMPurify's allowlist in Rust would be a second opinion about what the sanitizer accepts, the same sin as a second Markdown parser (any element §4 does not know fails safe into the wrapper); every emitted block is **stripped of the reserved namespace first and balanced after**; consecutive `li` blocks share one attribute-less `<ul>`/`<ol>` group; head, gap bytes, doctype, comments, `<script>`/`<style>` and non-sync rows (`title`, `thematic-break`) are not present; a `fallback_source` block renders live with its honest `data-fallback`, and only an extraction failure takes the `<pre data-skipped="html-block">` placeholder. **The reserved-namespace rule's pane half:** the six reserved names (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`, `data-skipped`) are stripped from every HTML-source pane block before injection, exactly as the Markdown html-block paragraph above states for its arm (wave 1) — the strip is **pane-only**: `out.md` and `out.html` keep the author's bytes, whose reserved-looking attributes are their content.
  - §4a gains one sentence at the end of its first paragraph: "The same outer-wrapper contract holds for an HTML-source run's panes (ti `490d97` / DCR-0038): a single `<main>`, every block a direct child in source order, `li` anchors exactly one level deep inside an attribute-less group — so every consumer statement in this section reads on both formats unchanged."
```bash
cargo fmt --all
git add crates/transync-core/src/pipeline.rs crates/transync-core/src/lib.rs \
  crates/transync/tests/scenarios/scn_16_html_document.rs docs/architecture/contracts.md
git commit -m "feat(core): an HTML run's panes are real — the §8 fragments flow out of translate()

The render dispatch's Html arm calls the wave-6 derivation with the run's
own html_outcomes map (the agreement rule's fourth consumer), under the
same render_fault mapping as the Markdown arm. The two wave-5 empty-pane
pins flip exactly as their own comments pre-authorized: byte-identity
stays, the panes now start with <main> and carry the anchors, and the
SCN-16 scenario additionally pins the fixture's <script> absent from both
panes. Contracts §4 gains the HTML-source-panes subsection with the
own-element divergence stated as deliberate, and §4a extends the outer-
wrapper contract to both formats in one sentence.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §8, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: The CLI — `--input-format`, the conflict, the refusal rewrite, `out.html`, `Destinations.document`, the out-dir guard, and both usage blocks

**Files:**
- Modify: `crates/transync-cli/src/main.rs`, `crates/transync-cli/src/translate_cmd.rs`, `crates/transync-cli/src/translate_cmd/args.rs`, `crates/transync-cli/src/translate_cmd/publish.rs`, `crates/transync-cli/src/output.rs`, `crates/transync-cli/tests/cli_smoke.rs`, `docs/architecture/contracts.md` (§6), `docs/Developer_Guide.md` (CLI reference)

**Interfaces:**
- Produces:
  ```rust
  // translate_cmd/args.rs
  #[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
  pub(crate) enum InputFormatArg { Markdown, Html }
  impl InputFormatArg {
      pub(crate) fn to_source_format(self) -> transync::SourceFormat; // exhaustive
  }
  // translate_cmd.rs — TranslateArgs gains
  #[arg(long = "input-format", value_enum, default_value = "markdown")]
  pub input_format: InputFormatArg,
  ```
  (`default_value = "markdown"` rather than `default_value_t`: clap parses the string through `ValueEnum` and renders the same `[default: markdown]`, without needing a `Display` impl on the enum.)
- **The exit-code map does not move.** Conflict → exit **1** (`ExitCode::ArgumentError` — the *arguments* are malformed, §6's code-1/code-2 boundary). Intake-level input failure on an HTML run → exit **2**: `intake::html::parse` is infallible (wave 3 deviation 2), so the Parse arm's clause is already occupied by exactly the failures that exist — unreadable file, size cap, non-UTF-8 — and **no new exit code and no new `ExitCode` variant appears anywhere in this wave** (`exit_code_docs_drift.rs` green with zero edits is the proof).
- **Routing is flag-only** (D8): the sniff runs on the Markdown arm only; there is no reverse sniff on the Html arm; `--input-format markdown` alone still trips the sniff (the flag names the arm, not a waiver).
- `docs_cli_flags_drift.rs` binds this task's code and docs into **one commit**: the moment clap accepts `--input-format`, both usage blocks must name it and state its default `markdown`, or the suite is red.

- [ ] **Step 1: Write the failing tests.** In `crates/transync-cli/tests/cli_smoke.rs`:
  - Rewrite `cli_html_document_input_is_refused`'s two message pins: the `stderr.contains("490d97")` assertion (message "stderr must name the absent HTML-to-HTML feature") becomes:
```rust
    assert!(
        stderr.contains("--input-format html"),
        "stderr must point at the HTML-document path that now exists: {stderr}"
    );
    assert!(
        !stderr.contains("unimplemented") && !stderr.contains("not implemented"),
        "the feature exists; the refusal must not lie in that direction: {stderr}"
    );
```
  (keep the `--allow-html-input` and no-output assertions unchanged.) Rewrite its **doc comment** in the same edit — it currently ends "…the message names the override flag and the absent HTML→HTML feature", which would lie above the rewritten pins. It becomes: "ti `13e145`: an `--input` whose preamble declares an HTML document is refused at the boundary. Exit 2 — the same code the other admission refusals use — the message names both escapes (`--input-format html` and `--allow-html-input`), and nothing is written." A doc comment narrating a dead assertion is the same lie the §9 same-commit rule exists to kill — the pins and their description move together.
  - Append five tests after `cli_markdown_opening_with_an_html_island_is_not_refused`:
```rust
/// §9: --allow-html-input asserts "Markdown despite the preamble";
/// --input-format html asserts "an HTML document". Together they are an
/// argument error — exit 1, not 2, because the ARGUMENTS are malformed.
/// The bug this catches: clap cannot express a value-dependent conflict,
/// so forgetting the manual check makes the pair silently run.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_input_format_conflict_is_an_argument_error() {
    let workdir = ScratchDir::new("transync-cli-fmt-conflict");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();
    let out_md = workdir.join("out.md");
    let out_json = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input").arg(&input)
        .arg("--output").arg(&out_md)
        .arg("--map").arg(&out_json)
        .arg("--target-language").arg("ko")
        .arg("--input-format").arg("html")
        .arg("--allow-html-input")
        .output()
        .expect("transync binary must run");

    assert_eq!(output.status.code(), Some(1), "an argument conflict is exit 1");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--allow-html-input") && stderr.contains("--input-format"),
        "the refusal names both flags: {stderr}"
    );
    assert!(!out_md.exists() && !out_json.exists(), "no output on a refused run");
}

/// §9: `--input-format markdown` alone RE-TRIPS the sniff — the flag names
/// the arm, not a waiver. The bug this catches: keying the sniff on the
/// flag's PRESENCE instead of on the Markdown arm.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_explicit_markdown_still_trips_the_sniff() {
    let workdir = ScratchDir::new("transync-cli-explicit-md");
    let input = workdir.join("page.html");
    std::fs::write(&input, HTML_DOCUMENT).unwrap();

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input").arg(&input)
        .arg("--output").arg(workdir.join("out.md"))
        .arg("--map").arg(workdir.join("out.json"))
        .arg("--target-language").arg("ko")
        .arg("--input-format").arg("markdown")
        .output()
        .expect("transync binary must run");
    assert_eq!(output.status.code(), Some(2), "the sniff still refuses, exit 2");
}

/// D8: no reverse sniff. A genuinely-Markdown file declared html produces
/// one text-heavy block set and translates — wrong shape, explicitly
/// requested. The bug this catches: someone "helpfully" adding a
/// Markdown-shape sniff to the html arm.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_markdown_declared_html_translates_without_a_reverse_sniff() {
    let workdir = ScratchDir::new("transync-cli-no-reverse-sniff");
    let input = workdir.join("notes.md");
    std::fs::write(&input, "# Title\n\nA paragraph.\n").unwrap();
    let out = workdir.join("out.html");
    let map = workdir.join("out.json");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input").arg(&input)
        .arg("--input-format").arg("html")
        .arg("--output").arg(&out)
        .arg("--map").arg(&map)
        .arg("--target-language").arg("ko")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "explicitly requested is the ADR-0017 boundary: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(out.exists() && map.exists());
}

/// THE operator-visible feature (spec §12 wave 6), end to end over the
/// SCN-16 fixture: out.html (never out.md), the wire's input_format html,
/// the title row's D5 shape, panes that carry anchors and no script, the
/// bundle titled by the source <title> — and out.html ANCHOR-FREE, which
/// is the "anchors never touch the regen path" invariant read off disk.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_format_run_writes_out_html_and_a_syncing_bundle() {
    let workdir = ScratchDir::new("transync-cli-html-run");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../transync/tests/fixtures/scn-16-html-document.html")
        .canonicalize()
        .expect("the SCN-16 fixture exists (wave 3)");
    let out_dir = workdir.join("published");

    let output = Command::new(bin())
        .arg("translate")
        .arg("--input").arg(&input)
        .arg("--input-format").arg("html")
        .arg("--out-dir").arg(&out_dir)
        .arg("--target-language").arg("ko")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(out_dir.join("out.html").exists(), "an HTML run publishes out.html");
    assert!(
        !out_dir.join("out.md").exists(),
        "one of the two, never both (§9/§6)"
    );

    let published = std::fs::read_to_string(out_dir.join("out.html")).expect("readable");
    assert!(published.contains("<!DOCTYPE html>"), "gaps are verbatim: doctype");
    assert!(published.contains("<script>"), "gaps are verbatim: script");
    assert!(
        !published.contains("data-sync-id"),
        "out.html is ANCHOR-FREE — injection is a bundle-only derivation \
         and must never reach the regen path (spec §8)"
    );

    let alignment: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(out_dir.join("alignment.json")).expect("map readable"),
    )
    .expect("alignment JSON parses");
    assert_eq!(alignment["schema_version"].as_str(), Some("1.3.0"));
    assert_eq!(alignment["input_format"].as_str(), Some("html"));
    let title = alignment["blocks"]
        .as_array()
        .expect("blocks")
        .iter()
        .find(|b| b["block_kind"].as_str() == Some("title"))
        .expect("the fixture's <title> has a row");
    assert_eq!(title["sync_role"].as_str(), Some("non-sync"), "D5");
    assert_eq!(title["source_format"].as_str(), Some("html"));

    let source_html =
        std::fs::read_to_string(out_dir.join("html/source.html")).expect("readable");
    let target_html =
        std::fs::read_to_string(out_dir.join("html/target.html")).expect("readable");
    for pane in [&source_html, &target_html] {
        assert!(pane.contains("data-sync-id=\"h1-"), "anchors reach the pane");
        assert!(pane.contains("<ul>"), "the nav items share a group (D9)");
        assert!(!pane.contains("<script"), "script is gap, never pane (D6)");
        assert!(!pane.contains("data-sync-id=\"title-"), "the title has no anchor (D5)");
    }

    // ti 0f26b5's chain, middle rung re-seated for HTML runs (§9): the
    // bundle title is the source document's <title> text — the same
    // extraction the provider saw — entity-decoded then re-escaped by the
    // shell assembler.
    let index = std::fs::read_to_string(out_dir.join("html/index.html")).expect("readable");
    assert!(
        index.contains("<title>Transync &amp; the two-pane page</title>"),
        "flag > source <title> > transync: {index}"
    );
}

/// Deviation 7, both directions: an HTML run's own out-dir republishes —
/// and still republishes after the ownership marker is lost (the ti 66339b
/// cp-drops-dotfiles recovery), which the naive published.len() == 5
/// equality would have silently killed.
#[cfg(feature = "test-stub-provider")]
#[test]
fn cli_html_out_dir_republishes_with_and_without_its_marker() {
    let workdir = ScratchDir::new("transync-cli-html-republish");
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../transync/tests/fixtures/scn-16-html-document.html")
        .canonicalize()
        .expect("fixture exists");
    let out_dir = workdir.join("published");
    let run = || {
        Command::new(bin())
            .arg("translate")
            .arg("--input").arg(&input)
            .arg("--input-format").arg("html")
            .arg("--out-dir").arg(&out_dir)
            .arg("--target-language").arg("ko")
            .output()
            .expect("transync binary must run")
    };
    assert_eq!(run().status.code(), Some(0), "first publish");
    assert_eq!(run().status.code(), Some(0), "republish over the marker");
    std::fs::remove_file(out_dir.join(".transync-out-dir")).expect("drop the marker");
    let third = run();
    assert_eq!(
        third.status.code(),
        Some(0),
        "the marker-less COMPLETE html set republishes without --force: {}",
        String::from_utf8_lossy(&third.stderr)
    );
}
```
- [ ] **Step 2: Watch the red.**
```bash
cargo test -p transync-cli --features test-stub-provider --test cli_smoke -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t5-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t5-red.txt
```
Expected: `CARGO_EXIT=101` with **exactly six failures** — the five new tests and the rewritten `cli_html_document_input_is_refused` — and every other `cli_smoke` case green. (Running the whole `cli_smoke` binary rather than a name filter is deliberate: the six reds live under five different name stems, and a filter that matches only one — `cli_input_format`, say — captures a red that cannot contain the observations below.) The new tests fail at the binary boundary: clap rejects `--input-format` as an unknown argument, so `cli_input_format_conflict_is_an_argument_error` observes exit 1 **for the wrong reason** — read the captured stderr and confirm it says `unexpected argument '--input-format'`; the test's *stderr* assertions (`contains("--allow-html-input") && contains("--input-format")`) are what hold it red rather than the exit code alone (clap's message names only the unknown flag). `cli_explicit_markdown_still_trips_the_sniff` observes exit 1 (unknown argument), expected 2; `cli_markdown_declared_html_translates_without_a_reverse_sniff`, `cli_html_format_run_writes_out_html_and_a_syncing_bundle` and `cli_html_out_dir_republishes_with_and_without_its_marker` observe exit 1, expected 0. The rewritten refusal pin fails on `contains("--input-format html")` — the shipped message still carries the ti-490d97 "unimplemented" tail. Record which assertions fired.

- [ ] **Step 3: Land the flag and the routing.**
  - `translate_cmd/args.rs` gains `InputFormatArg` + `to_source_format()` exactly as the interface block above (both matches exhaustive; doc comments: the enum's = "Which intake parses `--input` (D8: routing is flag-only; the sniff stays a refusal, never a router)"), plus two unit tests in the existing `mod tests`: the default is `Markdown` (`parse_args(&[]).input_format`), and `to_source_format` maps both variants (`assert_eq!(InputFormatArg::Html.to_source_format(), transync::SourceFormat::Html)` and the Markdown twin).
  - `translate_cmd.rs`: `TranslateArgs` gains the field (after `allow_html_input`, doc comment per the interface — the full flag doc names the default, the routing rule, the no-reverse-sniff consequence, and the conflict); `use args::InputFormatArg;` joins the imports.
  - The conflict check lands in `execute()`, immediately after the `resolve_title_flag` call (argument-level mistakes fail before any file is read):
```rust
    // ti 490d97 wave 6 (§9): the pair asserts contradictory things about one
    // input — "Markdown despite its preamble" / "an HTML document". Exit 1,
    // not 2: the ARGUMENTS are malformed (§6's code-1/code-2 boundary). Clap
    // cannot express a value-dependent conflict, so it is enforced here.
    // Exhaustive by charter.
    match args.input_format {
        InputFormatArg::Html => {
            if args.allow_html_input {
                return Err(CliFailure::new(
                    ExitCode::ArgumentError,
                    "--allow-html-input says the input is Markdown despite its preamble; \
                     --input-format html says it is an HTML document. Pass one or the other",
                ));
            }
        }
        InputFormatArg::Markdown => {}
    }
```
  - The sniff block is wrapped in the format match (Markdown arm keeps the existing `!args.allow_html_input && let Some(marker) = html_document_marker(&source)` gate **byte-identical**; the Html arm is empty with the D8 comment: "the sniff is not consulted, and there is no reverse sniff — a body fragment is legitimately accepted HTML"), and the refusal message's closing two sentences — currently "…HTML-to-HTML translation is a separate, unimplemented feature (ti 490d97). Pass --allow-html-input to translate it as Markdown anyway" — become §9's exact tail:
```text
… so the document changes shape and nothing reports it. Pass --input-format html to \
translate it as an HTML document, or --allow-html-input to translate it as Markdown anyway
```
    (the "(ti 490d97)" clause goes in this same commit, or the string lies — §9.)
  - After `opts.auto_glossary = …`: `opts.input_format = args.input_format.to_source_format();`
  - `TranslateArgs.allow_html_input`'s doc comment is narrowed to its real case (§9, documentation only — behavior bit-identical): the flag exists for **Markdown that genuinely opens with an `<html>`/`<!doctype`-shaped island** — `--input-format html` routes to a different intake, and `--input-format markdown` alone re-trips the sniff, so this is the only way to say "Markdown intake despite the preamble". Keep the measured three-consequences text; delete its "HTML→HTML translation is a separate, unimplemented feature (ti `490d97`)" sentence; add the conflict sentence.
  - `main.rs`: `about = "GFM Markdown and HTML document translation with block-level sync."`; the `Translate` variant's doc line becomes "Translate a Markdown or HTML document end-to-end."
- [ ] **Step 4: Land the publication half.**
  - `translate_cmd/publish.rs`: rename `Destinations.markdown` → `document` (every use — the struct field, both `Destinations { … }` literals, the `files` vec's `(dest.document, output.translated_document.as_bytes())`), and the `--out-dir` arm's filename becomes:
```rust
            // §9/§6: out.html for an HTML run, out.md for a Markdown run —
            // one of the two, never both. Routing is flag-only, so the flag
            // is the authority here too. Exhaustive by charter.
            document: PathBuf::from(match args.input_format {
                InputFormatArg::Markdown => "out.md",
                InputFormatArg::Html => "out.html",
            }),
```
    (import `InputFormatArg` from `super::args`.) Update the module doc's "translated Markdown" phrasing to "the translated document".
  - `output.rs`: `OUT_DIR_ENTRIES` becomes `&["out.md", "out.html", "alignment.json", "validation-report.json", HTML_SUBDIR]` (doc comment gains: "`out.md` and `out.html` are alternatives — a run writes exactly one — which is why the complete-set fallback below is a predicate, not a count"). In `ensure_out_dir_replaceable`, the acceptance line `if owned || published.is_empty() || published.len() == OUT_DIR_ENTRIES.len()` becomes:
```rust
    // ti 490d97 wave 6: a publication writes ONE translated document —
    // out.md or out.html, never both — so "the complete published set" is
    // the three fixed members plus at least one document file. The old
    // len() == OUT_DIR_ENTRIES.len() equality would be unsatisfiable with
    // five allow-listed names and silently kill the ti-66339b marker-less
    // recovery.
    let complete_set = published.contains("alignment.json")
        && published.contains("validation-report.json")
        && published.contains(HTML_SUBDIR)
        && (published.contains("out.md") || published.contains("out.html"));
    if owned || published.is_empty() || complete_set {
        return Ok(());
    }
```
    Sweep the function's doc comment and §6's mirrored prose for the "complete published set" definition (Step 5). The doc-comment sweep includes the two allow-list prose mentions at the top of `output.rs` that enumerate `out.md` by name.
  - `--output`'s doc comment (in `translate_cmd.rs`): "Translated-document output path — Markdown in, Markdown out; HTML in (`--input-format html`), HTML out. Required together with `--map` unless `--out-dir` is given."
- [ ] **Step 5: Both usage blocks, same commit.**
  - `docs/architecture/contracts.md` §6, in the fenced usage block, after the `[--allow-html-input]` entry:
```text
  [--input-format <markdown|html>] (default: markdown. Which intake parses
                            --input; routing is FLAG-ONLY — the sniff above
                            stays a refusal, never a router. markdown is
                            today's path, sniff intact. html is the HTML→HTML
                            path (ti 490d97): the HTML intake, the same
                            pipeline and block ids, HTML back out — the sniff
                            is not consulted and there is no reverse sniff,
                            so a genuinely-Markdown file declared html
                            translates as one text-heavy block set: wrong
                            shape, explicitly requested, which is the
                            boundary ADR-0017's silent-path refusals protect.
                            An input the run cannot read is exit 2, the same
                            clause every input failure already occupies — no
                            new exit codes. Together with --allow-html-input:
                            argument error, exit 1 — the pair asserts
                            contradictory things about one input. DCR-0038.)
```
    and the `[--allow-html-input]` entry is rewritten around the island case: keep the measured-consequences core, replace "HTML→HTML translation is a separate, unimplemented feature." with "For an actual HTML document, pass --input-format html instead; this flag is for Markdown that genuinely OPENS with an <html>/<!doctype island, which needs the Markdown intake despite its preamble — --input-format markdown alone re-trips the sniff. Conflicts with --input-format html (exit 1)."; the `--output` entry becomes "(required unless --out-dir is given; must accompany --map; the translated-document path — Markdown in, Markdown out; HTML in, HTML out)".
  - §6's layout diagram line `├── out.md                   # translated Markdown` becomes:
```text
├── out.md | out.html        # the translated document — out.md for a Markdown
│                            #   run, out.html for --input-format html
│                            #   (one of the two, never both)
```
    and the two prose passages that enumerate the allow-list / complete set (`{out.md, alignment.json, validation-report.json, html}` and "all three files and an `html` directory") gain `out.html` with the one-of-the-two clause.
  - §6's exit-`2` bullet: the HTML-document-refusal sentence's tail is updated to name both escapes ("…is declined unless `--allow-html-input` (translate it as Markdown anyway) or `--input-format html` (translate it as the HTML document it is, ti `490d97`/DCR-0038) is given…"), and the bundle-title paragraph gains one sentence: "On an `--input-format html` run the middle rung is the source document's `<title>` text — the same extraction and value the provider read as `document_title` — with the flag above it and the `transync` literal below it unchanged (ti `0f26b5`'s chain, third rung untouched)."
  - `docs/Developer_Guide.md`'s "## CLI reference" fenced block gains, beside `--allow-html-input`:
```text
  [--input-format <markdown|html>]    (default: markdown) which intake parses
                            --input; html is the HTML→HTML path — flag-only
                            routing, the sniff is never a router
```
    and its `--allow-html-input` one-liner is re-centered on the island case ("Markdown that opens with an HTML island; conflicts with --input-format html").
- [ ] **Step 6: Green — the whole CLI surface.**
```bash
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t5-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t5-green.txt
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave6/gate/t5-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t5-workspace.txt
```
Expected: `CARGO_EXIT=0` in both. Specifically green, by name: the five new tests; the rewritten `cli_html_document_input_is_refused`; the untouched `cli_allow_html_input_forces_the_previous_behavior` and `cli_html_input_re_emits_an_indented_run_as_a_fenced_block` (the override's behavior is bit-identical — their staying green with zero edits IS §9's "behavior is bit-identical to today" evidence); `docs_cli_flags_drift.rs`'s four checks (both blocks name the flag, neither invents one, the sets agree, the `markdown` default is stated in both entries); `exit_code_docs_drift.rs` with **zero edits** (no new codes). If `docs_cli_flags_drift` is red, read which of its four failure sentences fired before touching anything — the fix is a block edit, never a test edit.

- [ ] **Step 7: Format, lint, commit.**
```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings > /Volumes/Temp/claude/ti490d97-wave6/gate/t5-clippy.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t5-clippy.txt
git add crates/transync-cli/src/main.rs crates/transync-cli/src/translate_cmd.rs \
  crates/transync-cli/src/translate_cmd/args.rs crates/transync-cli/src/translate_cmd/publish.rs \
  crates/transync-cli/src/output.rs crates/transync-cli/tests/cli_smoke.rs \
  docs/architecture/contracts.md docs/Developer_Guide.md
git commit -m "feat(cli): --input-format lands — flag-only routing, the sniff stays a refusal

markdown (the default) is today's path, sniff intact and re-tripped even
when the flag is explicit; html takes the HTML intake with no reverse
sniff — wrong shape on a Markdown file is explicitly requested, ADR-0017's
boundary. --allow-html-input narrows in documentation to its real case
(the island) and conflicts with --input-format html at exit 1, enforced in
execute() because clap cannot express a value-dependent conflict. The
refusal tail now points at both escapes and the ti-490d97 'unimplemented'
clause dies in the same commit, or the string lies. Publication:
Destinations.markdown becomes Destinations.document (private, free), an
HTML run writes out.html — one of the two, never both — and the out-dir
allow-list gains out.html with the complete-set fallback rewritten as a
predicate, because the old len()-equality would be unsatisfiable with five
names and would silently kill the marker-less republish recovery. Both
usage blocks move in this commit; docs_cli_flags_drift is the weld, and
exit_code_docs_drift stays green with zero edits because no exit code
moved. out.html is pinned ANCHOR-FREE end to end.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §9, §12; D8)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 6: The browser leg — the SCN-16 bundle in `test-browser.sh` and the direct-drive pane-sync proof

**Files:**
- Modify: `scripts/test-browser.sh`, `web/tests/engine.spec.js`

**Interfaces:**
- §12 wave 6's acceptance names "the AC's pane-sync assertion under the **direct-drive engine suite**" — engine.spec.js's rig, driving `sync.js` directly over the real bundle's pane files. The shell-driven, DOMPurify-mounted SCN-16 Playwright spec (`web/tests/scn16.spec.js`) is **wave 7's** and is deliberately not created here (Deviation 9's second half).
- `test-browser.sh` runs under `set -euo pipefail`; zero-expected greps use `if grep -q`.

- [ ] **Step 1: The bundle leg.** In `scripts/test-browser.sh`, immediately after wave 1's OI-0035 leg (its closing `fi`) and before the `cd "$WEB_DIR"` line:
```bash
# --- SCN-16 leg (HTML→HTML, ti 490d97 wave 6) ------------------------------
#
# An --input-format html run over the SCN-16 fixture, published as a THIRD
# bundle inside the served fixture dir so the engine suite can read its pane
# files and map off disk. Scratch only; the six-file bundle contract and the
# SCN-14 corpus are untouched.
echo "[test-browser] regenerating the SCN-16 HTML-run bundle -> $HTML_OUT/scn16"
SCN16_INPUT="$REPO_ROOT/crates/transync/tests/fixtures/scn-16-html-document.html"
if [[ ! -f "$SCN16_INPUT" ]]; then
  echo "[test-browser] FAIL: SCN-16 fixture not found: $SCN16_INPUT" >&2
  exit 1
fi
"$TRANSYNC_BIN" translate \
  --input "$SCN16_INPUT" \
  --input-format html \
  --output "$WORKDIR/scn16-out.html" \
  --map "$WORKDIR/scn16.json" \
  --html-out "$HTML_OUT/scn16" \
  --target-language ko

for path in \
  "$HTML_OUT/scn16/index.html" \
  "$HTML_OUT/scn16/source.html" \
  "$HTML_OUT/scn16/target.html" \
  "$HTML_OUT/scn16/alignment.json" \
  "$HTML_OUT/scn16/sync.js" \
  "$HTML_OUT/scn16/purify.min.js"
do
  if [[ ! -s "$path" ]]; then
    echo "[test-browser] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

# Anchors are a bundle-only derivation (spec §8): the published translated
# document is anchor-free, and this is the one place a script reads it to
# hold the sentence true — every other assertion looks at pane HTML, so a
# derivation "centralized" into regen would pass everything above and
# silently ship anchors in out.html.
if grep -q 'data-sync-id' "$WORKDIR/scn16-out.html"; then
  echo "[test-browser] FAIL: the published HTML document carries sync anchors — injection must never reach the regen path (ti 490d97 §8)" >&2
  exit 1
fi
```
- [ ] **Step 2: The direct-drive test.** In `web/tests/engine.spec.js`, after Task 2's test `m`:
```js
  test("n — the SCN-16 HTML-run bundle's panes mount and sync by block id, direct-drive", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // ti 490d97 wave 6 (spec §12's acceptance): the real bundle's pane
    // fragments and map, driven through mountSync directly — no shell, no
    // fetch, no DOMPurify (wave 7's scn16.spec.js drives the shipped shell).
    const sourcePane = readFixture("scn16/source.html");
    const targetPane = readFixture("scn16/target.html");
    const map = JSON.parse(readFixture("scn16/alignment.json"));
    expect(map.schema_version).toBe("1.3.0");
    expect(map.input_format).toBe("html");

    await page.goto("/");
    await waitForMounted(page);
    await page.evaluate(
      ([src, tgt]) => {
        document.body.innerHTML = "";
        document.body.style.cssText = "display:block;margin:0;padding:0";
        for (const [id, html] of [["eng-source", src], ["eng-target", tgt]]) {
          const pane = document.createElement("div");
          pane.id = id;
          pane.style.cssText =
            "position:relative;height:200px;overflow:auto;margin:0;padding:0";
          pane.innerHTML = html;
          document.body.appendChild(pane);
        }
      },
      [sourcePane, targetPane]
    );
    expect(await mount(page, map)).toBe("controller");

    // D5: the title row is in the map, and NOTHING in a pane claims it.
    expect(
      await page.evaluate(
        () => document.querySelectorAll('[data-sync-id^="title-"]').length
      )
    ).toBe(0);
    // D9: no bare <li> as a direct pane child. The group is required by D9's
    // model (the container is gap; only items anchor), not by any sanitizer
    // behaviour — DOMPurify leaves a bare <li> in place, measured.
    expect(
      await page.evaluate(
        () => document.querySelectorAll("#eng-source > main > li").length
      )
    ).toBe(0);

    // Bidirectional block-id sync over real HTML-derived anchors.
    const h2 = await page.evaluate(() =>
      document.querySelector('#eng-source [data-sync-id^="h2-"]').dataset.syncId
    );
    const srcTop = await offsetTopOf(page, SRC, h2);
    const tgtTop = await offsetTopOf(page, TGT, h2);
    await setScrollTop(page, SRC, srcTop);
    await waitForScrollNear(page, TGT, tgtTop, 8);
    await page.waitForTimeout(160);
    await setScrollTop(page, TGT, 0);
    await waitForScrollNear(page, SRC, 0, 8);

    expect(errors).toEqual([]);
  });
```
  *What implementation bug makes this red:* panes that are whole documents (the `<main>` selector and the pane geometry break); a missed `li` group (the direct-child `li` count trips, and DOMPurify aside, the map/DOM drift warnings fire); a title anchor leaking (D5's count trips); an injection that lost the `data-sync-id` spelling `write_attrs` owns (mount refuses or drift-warns); a map whose `input_format`/version regressed.
- [ ] **Step 3: Run, capture, commit.**
```bash
scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave6/gate/t6-browser.txt 2>&1
echo "SUITE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave6/gate/t6-browser.txt
```
Expected: `SUITE_EXIT=0` — the scn16 leg generates, the anchor-free grep passes, tests `a`–`n` green, scn13 + wasm legs untouched and green. (`crates/transync/tests/docs_browser_suite_drift.rs` walks spec *files* and no file was added — it stays green with zero edits; if it is red, STOP and read what it names.)
```bash
git add scripts/test-browser.sh web/tests/engine.spec.js
git commit -m "test(web): the SCN-16 bundle mounts and syncs, driven directly

A third bundle leg publishes the --input-format html run of the SCN-16
fixture into the served fixture dir, checks all six files, and holds the
one sentence no pane assertion can: the published translated document is
anchor-free, because injection is a bundle-only derivation. Test n mounts
the real pane fragments and the real 1.3.0 map through mountSync directly
and drives both directions by block id, with the title row anchor-less and
no bare li as a direct child; the shell-driven SCN-16 spec is wave 7's.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §8, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 7: Records — DCR-0038 and the routine files (no `docs/index.md` step)

**Files:**
- Create: `docs/project/design-change-records/DCR-0038-html-panes-wire-cli.md`
- Modify: `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`

**Interfaces:**
- **`docs/index.md` is controller-owned this wave.** This task produces DCR-0038's exact link line and hands it to the controller in the commit message and in `status.md`'s action bullet; it is **never added to `docs/index.md` here**. The line, exactly:
```markdown
- [DCR-0038 — HTML panes, alignment wire 1.3.0, and the CLI surface (2026-08-20)](project/design-change-records/DCR-0038-html-panes-wire-cli.md) — ti `490d97` wave 6: the §8 pane derivation (strip → inject into the block's own tag → balance → group), schema 1.3.0's `source_format`/`input_format`/`title`, and `--input-format` with the sniff kept a refusal.
```
  Until the controller links it, `docs_index_drift` is expected red naming exactly this file (the standing allowance); any other name is real drift and a STOP.

- [ ] **Step 1: Write DCR-0038.** Frontmatter/shape per DCR-0032…0037's house form. Required content, each with its ground:
  - **What changed:** the §8 derivation (module home `render/html_pane.rs`, entry points, `PaneCtx` reuse — spec §15 item 1's delegated decision, recorded); schema 1.3.0 (the three-part sanctioned diff: the version string, row `source_format`, map `input_format` — the sharpening of §11's two-field sentence, plus the §11 cheap-pin sharpening toward §3's spelling definition, both from this plan's Deviation 5); the CLI surface (flag, conflict, refusal rewrite, `out.html`, `Destinations.document`, the out-dir predicate — Deviation 7 in full).
  - **The judgement calls, each with its ground:** this plan's Deviations 1–9 in full — notably the `normalize_top_level` grouping interpretation (Deviation 2, with wave 3's pin cited), the `HtmlOutcome` parameter (Deviation 3), the own-element divergence stated so it survives review, with its img-run sharpening and the 2026-08-21 unknown-element ruling on §8 step 3/4's boundary (Deviation 4 — the DOMPurify measurement, the §4-tables key, and the warning against re-keying it on the sanitizer's allowlist all recorded), the target-only intake guard with its nesting-bound side effect named (Deviation 8), and the non-sync-rows-absent panes with the `<ol start>` gap cost (Deviation 9 — grounded on §8's per-row rule and D6, §13 item 6 for the title, §13 item 4 for the `<ol start>` gap; §13 records no `<hr>` item, and the record must not invent one).
  - **Owed spec amendments, §14-style (the spec file is NOT edited) — two.** First, §7's "walk … must simply never be called on an HTML document" is over-broad. The spec's own §4 keeps `walk::normalize_top_level`'s prefix collapse total "so adjacent lists can never merge" — a sentence with no referent unless the collapse runs on HTML documents — and §8 step 6 mandates a grouping whose only shipped home is that function; wave 3 ships `the_prefix_collapse_is_total_over_html_list_items` calling it on `intake::html::parse` output. The sentence should name the comrak-typed consumers it means (`reparse_full`, `render_fragment` — the Markdown layer-6/renderer pairing), not the module. Second, §8 steps 3/4: the boundary's literal text under-describes the wrapper's population twice over — the void sharpening (Deviation 4's img-run corollary) and the 2026-08-21 ruling (a block whose outermost element is unknown to §4's tables takes step 4's transparent wrapper, so its anchor survives the shell's DOMPurify mount; keyed on §4's tables, never the sanitizer's allowlist, with the measured removes-unknown-elements ground). Record both amendments as owed, the way DCR-0035 recorded the img-exception amendment; wave 7 carries them to the controller in its batch.
  - **Discharged hand-forwards:** wave 5's empty panes (both pre-authorized flips executed, named by test); wave 0's second `strip_reserved_sync_attrs` call site (no unclaimed caller remains); wave 2's render/pane `Document.format` guard (**both halves** — the Html assert atop the derivation and the Markdown mirror atop `render_fragment`, spec §3's named consumer; the `debug_assert_eq!` posture and why a fallible refusal was rejected; the item closes on the pair, never on one); wave 5 Task 8's note that the sniff pins were this wave's (executed).
  - **The OI-0035 posture restated for the new producer:** the HTML pane path strips before injecting, so "ours are the only sync attributes" stays a construction; the honest residual (an in-pane impostor carrying a **listed** id ahead of the genuine anchor defeats the engine layer alone) is unchanged from DCR-0033 and re-cited, not re-litigated.
  - **What did NOT move:** `regen` (out.html anchor-free, pinned twice); the engine's logic (`KNOWN_SCHEMA` and one doc line are the whole sync.js diff — Deviation 6's verified list of why 1.3.0 needs no engine change); no exit code, no `ExitCode` variant, no cache axis, no `VALIDATION_SCHEMA_VERSION` motion; `--allow-html-input` behavior bit-identical (the two untouched smoke tests named as the evidence).
  - **Handed forward to wave 7:** `web/tests/scn16.spec.js` (shell-driven, DOMPurify-mounted, zero-console-warnings — the sanitizer-facing gate no earlier wave ran; wave 7's 2026-08-21 review has since measured that DOMPurify relocates no bare `li`, so its D9 assertions stand on structural drift and the mechanism erratum rides wave 7's amendment batch), the scenario-matrix SCN-16 row and block-kind coverage rows, the serve-door documentation, any contracts sections §10 lists that no wave-6 commit made true, and the docs-index link for the spec itself.
- [ ] **Step 2: CHANGELOG.** Under `[Unreleased]` (the 0.5.0-dev window), an `### Added` entry for the operator surface (`--input-format`, `out.html`, the bundle that syncs), an entry for schema 1.3.0 naming the three additive changes and the forward-minor guarantee, and a `### Changed` entry for the refusal-message rewrite and the narrowed `--allow-html-input` documentation. No `BREAKING` entry: everything in this wave is additive on the wire and on the surface (the breaking window's four changes were wave 2's).
- [ ] **Step 3: `status.md` + `phase-state.yaml`.** A ti `490d97` action bullet in the shape waves 0–5 used: wave 6 **LANDED**, DCR-0038, demonstrable outcome "`transync translate --input-format html … --out-dir out/` writes a translated page and a bundle that mounts and syncs"; acceptance held (name the five wave-acceptance checks below); **wave 7 (browser gate and closure) is the next action and the last**. Update `phase-state.yaml`'s wave marker the same way waves 0–5 did. Include the controller hand-off note: "DCR-0038's index link is the controller's (line handed in the DCR commit)."
- [ ] **Step 4: Commit.**
```bash
git add docs/project/design-change-records/DCR-0038-html-panes-wire-cli.md \
  CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml
git commit -m "docs: DCR-0038 — the operator surface exists, and the records say what it cost

The wave's record: the §8 derivation with its module home decided (spec
§15 item 1), schema 1.3.0's three-part sanctioned diff, the CLI surface
with the sniff kept a refusal, the nine declared deviations, the four
discharged hand-forwards, and the wave-7 hand-off. docs/index.md is the
controller's this wave; DCR-0038's exact link line rides this message:

- [DCR-0038 — HTML panes, alignment wire 1.3.0, and the CLI surface (2026-08-20)](project/design-change-records/DCR-0038-html-panes-wire-cli.md) — ti 490d97 wave 6: the §8 pane derivation (strip → inject into the block's own tag → balance → group), schema 1.3.0's source_format/input_format/title, and --input-format with the sniff kept a refusal.

TRACE: ti 490d97 wave 6 (spec 2026-08-20 §10, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Wave acceptance — check all six before declaring wave 6 done

1. **The demonstrable outcome, run by hand and captured:** `transync translate --input-format html --input crates/transync/tests/fixtures/scn-16-html-document.html --out-dir /Volumes/Temp/claude/ti490d97-wave6/demo --target-language ko` (stub provider build) exits 0; `demo/out.html` opens as the full page and contains no `data-sync-id`; `demo/html/` mounts and syncs (Task 6's test `n` is the mechanized form). Capture the command's exit code to `gate/acceptance-demo.txt`.
2. **`cli_smoke.rs` + `docs_cli_flags_drift.rs` green** (`gate/t5-green.txt`), with `exit_code_docs_drift.rs` and `sync_js_drift.rs` green at **zero edits** to either test file.
3. **Forward-minor is a driven proof:** engine.spec.js test `m` green (`gate/t2-browser.txt`) — a map one minor newer, carrying exactly 1.3.0's additions, mounts with the warning and drives.
4. **The AC's pane-sync assertion under the direct-drive engine suite:** test `n` green (`gate/t6-browser.txt`), including the anchor-free `out.html` grep in the same run.
5. **The Markdown corpus is byte-identical outside the sanctioned diff:** `gate/t4-green.txt` / `gate/t5-workspace.txt` show the full workspace green, and `git diff <baseline-commit> --stat -- crates/transync/tests/fixtures/` is **empty** (zero fixture edits); the only expectation edits in `git log <baseline-commit>..` touching existing tests are Deviation 5's version literals (the Rust asserts and the two `engine.spec.js` forward-minor specimens, tests `c`/`g`, rewritten to derive), the two pre-authorized wave-5 pane flips, and the rewritten sniff-refusal pins (`cli_html_document_input_is_refused`, its doc comment included) — diff and check by eye, capture the file list to `gate/acceptance-expectation-edits.txt`.
6. **The two silent-reintroduction routes are closed and pinned:** grep the tree — `grep -c 'data-sync-id' crates/transync-syntax/src/regen.rs` prints `0` (no injection reached regen), and `grep -n 'strip_reserved_sync_attrs' crates/transync-syntax/src/render/html_pane.rs` shows the strip on the raw bytes with injection scanning the stripped string (the interlock test `strip_then_inject_makes_ours_the_only_sync_attributes` is the mechanized form). Capture both to `gate/acceptance-invariants.txt`.

---

## Spec §8 / §9 coverage map

| Spec item | Where in this plan |
|---|---|
| §8 panes are synthesized fragments, four reasons | Task 3 module doc; contracts §4 subsection (Task 4 Step 5) |
| §8 step 1 — slice by row range, three refusals verbatim, `parser::intake` guard on the target | Task 3 Step 3 (`PaneCtx::new` reuse; the guard); tests `the_three_map_refusals_carry_over`, `the_target_pane_passes_the_intake_guard`; Deviation 8 |
| §8 step 2/3 — strip then inject, own outermost known element, `write_attrs` | Task 3 Step 3 (`emit_block`, `injected_fragment` with the §4-tables conjunct); test `strip_then_inject_…`; Deviation 4 |
| §8 step 4 — transparent `<div>` for element-less blocks, every img-run block (Deviation 4's sharpening), and every block whose outermost element is unknown to §4's tables (the 2026-08-21 ruling) | `injected_fragment`'s wrapper arm; tests `an_anonymous_run_…`, `a_lone_void_img_…`, `an_unknown_element_block_…` |
| §8 step 5 — balance | both arms of `injected_fragment`; test `an_unclosed_block_cannot_swallow_the_next_anchor` |
| §8 step 6 — `li` grouping, mandatory under D9 | `emit_list_group` via `normalize_top_level` (Deviation 2); test `consecutive_items_…`; browser test `n`'s direct-child-`li` count |
| §8 step 7 — gaps/head/script not emitted; fallback live; extraction-failure placeholder | `emit_block`'s outcome match (Deviation 3); tests `gaps_head_…`, `fallback_renders_live_…` |
| §8 — `out.html` anchor-free, injection bundle-only | not-touched `regen.rs`; `cli_html_format_run_…`'s grep; Task 6's script grep; acceptance 6 |
| §8 OI-0035 render half's second call site | `emit_block` (Inherited obligation 3); the interlock test |
| §9 `--input-format`, flag-only, no reverse sniff, exit 2 clause unchanged | Task 5 Steps 3/5; tests `cli_explicit_markdown_still_trips_the_sniff`, `cli_markdown_declared_html_translates_…` |
| §9 refusal tail rewrite, `(ti 490d97)` clause same commit | Task 5 Step 3; rewritten `cli_html_document_input_is_refused` |
| §9 `--allow-html-input` kept/narrowed + exit-1 conflict | Task 5 Steps 3/5; test `cli_input_format_conflict_…`; the two untouched override tests |
| §9 `out.html` / `Destinations.document` / `--output` help / `about` | Task 5 Steps 3/4; test `cli_html_format_run_…` |
| §9 bundle title: flag > source `<title>` > `transync` | falls out of wave 5's `document_title`; pinned in `cli_html_format_run_…`'s `<title>` assertion; §6 sentence (Task 5 Step 5) |
| §9 `transync serve` needs no change | nothing touched; `out.html` beside `html/` exists the moment Task 5 lands |
| §10/§3 schema 1.3.0 + forward-minor normativity | Task 2 throughout; engine test `m` |
| §12 wave 6 acceptance line | Wave acceptance 2–4 |

## Notes for the implementer

- **The two ways this wave silently reintroduces OI-0035, one more time:** injection before strip (the interlock test red in both wrong orders — trust it), and any refactor that moves injection into `regen` or writes `annotated_target_html` where `translated_document` goes (acceptance 6's greps and two end-to-end anchor-free pins). If either pin is ever "in the way", the change it blocks is the bug.
- **Do not edit weld tests to make them pass.** `docs_cli_flags_drift`, `exit_code_docs_drift`, `sync_js_drift`, `docs_browser_suite_drift`, `public_surface` are gates; every red they can show in this wave has a named doc- or copy-side fix in the task that causes it.
- **Quoted upstream shapes are from the wave 0–5 *plans*, not from a landed tree** (those waves were mid-landing when this plan was written). Task 1's STOP gate is the contract: if a grep fails, or a landed shape differs from a quote here (wave 5's dispatch comment text, the scn_16 assertion wording, `ElementExtent`'s field semantics), the landed tree wins — adjust the mechanical detail, keep the invariant, and record the delta in DCR-0038's verification note.
- **The `mapOf` literal in engine.spec.js stays 1.2.0 on purpose** (Task 2 Step 5). If a future reader "fixes" it, test `m` still holds the forward-minor mechanism, but the accidental older-minor coverage dies — say so in review if you see it happen. The inverse rule got its own scar this wave: a forward-minor specimen must never be a hard-coded literal — tests `c`, `g` and `m` all derive theirs through `forwardMinorVersion()`, because a literal specimen is exactly what the 1.3.0 bump had to sweep.
- **`grep -c` under `set -e`:** the acceptance greps expecting `0` will exit non-zero; run them bare in the shell, not in a script with `-e`, and read the printed count.
