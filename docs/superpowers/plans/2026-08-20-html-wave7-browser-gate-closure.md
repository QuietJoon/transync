# HTML→HTML Wave 7 — browser gate and closure — implementation plan

**Date:** 2026-08-20
**Ticket:** ti `490d97` (HTML→HTML document translation), wave 7 of 8 — the **last** wave — spec `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12 wave 7, with §8 (what the panes are), §11 (the Playwright row), §13 (the accepted limitations the closure must re-state, never re-litigate) and §14/§15 (the items that come home here) binding.

**Goal:** the browser gate and the closure paperwork. Two deliverables, and the second is the larger one:

1. **`web/tests/scn16.spec.js`** — the shell-driven, DOMPurify-mounted Playwright spec over the SCN-16 HTML-run bundle that wave 6's `scripts/test-browser.sh` leg publishes at `<fixture>/scn16/`. Acceptance, verbatim from §12: *bidirectional sync by block id, the title absent from panes, zero console warnings.* Wave 6's `engine.spec.js` test `n` already drives the same pane files **without** the shell; this spec is the half wave 6's plan deliberately left here ("the shell-driven SCN-16 spec is wave 7's" — its Task 6 interface note and its DCR-0038 hand-forward both say so).
2. **Closure:** the scenario-matrix SCN-16 rows; the serve-door documentation; the backlog and open-issue cross-references; CHANGELOG / status / phase-state; the remaining contracts sections; the docs-drift weld growing to four spec files without lying anywhere; and — the characteristic work of a closure wave — **the deferred-item ledger**: every hand-forward, owed amendment, and §14/§15 item across waves 0–6 and the spec, each with an explicit disposition. The ledger is this plan's "Inherited obligations" section and lands verbatim (with execution-time verdicts) in DCR-0039.

**Architecture of the browser test.** No new engine code, no new Rust code except three constant edits in one weld test. `scripts/test-browser.sh` needs **no logic edit**: `playwright.config.js` sets `testDir: "./tests"` and the script ends in a bare `pnpm exec playwright test`, so a spec dropped into `web/tests/` is wired the moment it exists — the runner-is-authority doctrine `docs/Developer_Guide.md`'s command table and `web/SMOKE.md` both already state (Deviation 1). The spec drives `/scn16/index.html` — the CLI-emitted shell of the wave-6 bundle leg, fetch + DOMPurify fail-closed mount + `sync.js`, the way an operator meets an HTML run's `--html-out`. Every negative assertion is paired with a positive one in the same test (Deviation 5's pane-anchor equality), and every assertion is proven falsifiable against a **mutated copy of the real bundle** before commit (Task 2's falsification protocol — the config's `TRANSYNC_FIXTURE_DIR` + Node stand-in server make that possible with zero repo edits).

**DCR number: DCR-0039.** Waves 0–6 are DCR-0032/0033/0034/0035/0036/0037/0038 in order. Task 1 Step 2 verifies `docs/project/design-change-records/DCR-0039-*` does not exist; **on a collision, STOP and hand back to the controller — never renumber silently.**

---

## Inherited obligations — the deferred-item ledger (read before Task 1)

This is the wave's real work surface. Every row was collected from the seven wave plans (0–6), the spec's §12/§13/§14/§15, and the tree — plus rows 27–31 from the 2026-08-21 adversarial review: the measured §8 step 6 erratum, and the implicit class no wave assigned (living documents wave 6's own deliverables made stale); the **Disposition** column is what this plan commits to. Task 8 copies this table into DCR-0039 with execution-time verdicts (each "verify" resolved to what the landed tree actually showed). A row marked *unverified* was quoted from a plan, not a landed artifact — the STOP gate and each owning task re-read the landed text before acting.

| # | Item | Recorded where | Wave 7 disposition |
|---|---|---|---|
| 1 | `web/tests/scn16.spec.js`, shell-driven, wired into `scripts/test-browser.sh` | spec §12 wave 7; wave 6 plan Task 6 interface note + DCR-0038 hand-forward | **Discharged** — Tasks 2–3. Wiring is testDir discovery + wave 6's bundle leg (Deviation 1); `test-browser.sh` gets comment-truth edits only |
| 2 | Scenario-matrix SCN-16 row + block-kind coverage rows (`title`, html-document blocks) + out-of-scope clause update | spec §10, §12 wave 7; wave 3/5/6 "Not touched" rows all point here | **Discharged** — Task 4 |
| 3 | Serve-door documentation (§9: "D6's fidelity door is open the day `out.html` exists"; wave 6's coverage map maps it to "nothing touched") | spec §9, §12 wave 7 | **Discharged** — Task 5 (Developer_Guide serve section + contracts §6 one sentence) |
| 4 | Backlog / open-issue cross-references | spec §12 wave 7 | **Discharged** — Task 7. Includes fixing a wave-1 record gap: `docs/backlog.md`'s Type-2 `sync-anchor-injection-via-raw-html (OI-0035)` entry was never marked resolved (wave 1's plan touched `open-issues.md` + archive, **not** the backlog — verified against its File Structure table) |
| 5 | CHANGELOG / status.md / phase-state.yaml | spec §12 wave 7 | **Discharged** — Task 8 |
| 6 | "The remaining contracts sections" — anything §10 lists that no wave 0–6 commit made true | spec §12 wave 7; DCR-0038's hand-forward ("any contracts sections §10 lists that no wave-6 commit made true") | **Discharged** — Task 6, as a checklist sweep of §10's contracts list against the landed `contracts.md`, writing only what is missing. Expected result: nothing is missing (every §10 contracts item is assigned to a wave 0–6 plan); a gap found is written here with §10's text as the source |
| 7 | The docs-index link for this spec | spec §12 wave 7 | **Already satisfied — no step.** Verified at plan time: `docs/index.md` links `superpowers/specs/2026-08-20-html-to-html-translation-design.md`. Do **not** add a duplicate. One residue: the controller has already corrected that link's annotation once — the landed line reads "**Implementation is authorized and under way** — all eight waves have written, adversarially reviewed plans, linked above; wave 0 (the pure refactor) is landing on `master` now" (verified in the landed file, 2026-08-21) — and this wave makes it stale in a *new* direction: under-way-with-wave-0-landing must become implemented (DCR-0032..0039), release cut pending. `docs/index.md` is controller-owned — Task 8 hands the controller the replacement annotation, re-verified stale at execution first |
| 8 | §14 item 1 — the PHRASING widening's consequence (`Price: <my-price/> today` splits into three blocks), **owner-mandated re-confirmation against real output** | spec §14.1; §13.5; D4 | **Cannot be discharged by this wave** — the re-confirmation is the owner's, against real output. Wave 7 **stages the evidence** (Task 7 Step 4: real-shaped pages with custom elements through the intake, block sets captured), files a ti ticket, adds the backlog entry, and names the open action in status.md |
| 9 | Owed spec amendment A — §7 check-3 mechanism: "produce no token in `scan_tags`" is stale since wave 0's `Skip`; correct form is "no **ledger entry** — `tag_inventory` filters `Skip`" | wave 0 plan Task 7 (DCR-0032); repeated as owed by wave 4 (DCR-0036) | **Stays open, handed to the controller as a batch** — Task 8 Step 3 carries the exact replacement wording. The spec file is not edited by this wave (Deviation 3) |
| 10 | Owed spec amendment B — §4 rule T's "**MEASURED: `<!DOCTYPE html>` produces no token *and no skip* in today's scanner**" needs the "pre-wave-0 scanner" clarification | wave 0 plan Task 7 (DCR-0032) | Same batch — Task 8 Step 3 |
| 11 | Owed spec amendment C — §4's img-exception sentence amended to the implemented reading (a linked image is one `Image` block; "textless" excludes absorbed phrasing markup) | wave 3 deviation 5; DCR-0035 post-implementation item | Same batch — Task 8 Step 3 |
| 12 | Owed spec amendment D — §12's wave-4 "parallel with 3" holds only for checks 1/3/4 + the finalize branch; check 2 consumes the intake; executed ordering was 3 → 4 | wave 4 deviation 1; DCR-0036 | Same batch — Task 8 Step 3 |
| 13 | Owed spec amendment E — §6's context-projection hazard is the opposite failure (Html-spelled heading projects to **empty prose** under comrak, not raw-tag leakage); wave 5 hands "the spec amendment to the controller alongside the index line" | wave 5 Task 10 (DCR-0037 erratum note) | Same batch — Task 8 Step 3 |
| 14 | Owed spec amendment F — §7's "walk … must simply never be called on an HTML document" is over-broad; should name the comrak-typed consumers (`reparse_full`, `render_fragment`) | wave 6 deviation 2; DCR-0038 | Same batch — Task 8 Step 3 |
| 15 | Owed spec amendment G — §8 step 3/4 boundary sharpening, both halves: a lone void `<img>` presents no extent and takes the wrapper arm; and — the 2026-08-21 ruling — a block whose outermost element is unknown to §4's tables takes the wrapper too, so its anchor survives the shell's DOMPurify mount (keyed on §4's tables, never the sanitizer's allowlist) | wave 6 deviation 4, as amended by the 2026-08-21 owner ruling; DCR-0038 | Same batch — Task 8 Step 3 |
| 16 | Owed spec amendment H — §11's "these two additive fields as the *only* sanctioned alignment-output diff" is three parts (the version string moves too), and its cheap pin is written to §3's spelling definition | wave 6 deviation 5; DCR-0038 | Same batch — Task 8 Step 3 |
| 17 | D11 — Markdown-island reclassification deferred to a schema-2.x-shaped window | spec D11, §13.9; ADR-0025 | **Stays open** — Task 7 adds the backlog entry (Type 3, blocked by the window), so the deferral survives outside the ADR too |
| 18 | §15.4 — oversize HTML leaf blocks: the segment-window splitter (DCR-0026 mold, never intake descent) is future work, unscheduled; §15.5's merged-multi-range splice watch item rides with it | spec §15.4, §15.5 | **Stays open** — Task 7 adds the backlog entry (Type 3, need/telemetry gate), §15.5 folded in as its watch note |
| 19 | The `parser` → `intake::markdown` rename, deferred with its measured blast radius | wave 3 deviation 1; DCR-0035 | **Stays open** — Task 7 adds the backlog entry (Type 1, mechanical) |
| 20 | §15.3 — DOMPurify `ALLOW_DATA_ATTR` default verification | spec §15.3 | **Already discharged by wave 1** (measured in `scn13.spec.js` test `m`; DCR-0033). Verify at the STOP gate; no step |
| 21 | §15.1 pane module home / §15.2 `ElementExtent` shape / §15.6 `prompt_html` body | spec §15 | **Already discharged** by waves 6 / 3 / 5 respectively (their deviations record the decisions). Verify via the DCR files at the STOP gate; no step |
| 22 | DCR-0036's Hard-arm prefix hand-forward | wave 4 plan Task 5 | **Already discharged by wave 5** (re-pinned; `an_html_hard_failure_speaks_the_twins_vocabulary`; DCR-0037). No step |
| 23 | Wave 5's empty panes; wave 0's second `strip_reserved_sync_attrs` call site; wave 2's `Document.format` render/pane guard; wave 5 Task 8's sniff-pin boundary note | wave 6 Inherited obligations 1–4 | **Already discharged by wave 6** (DCR-0038 records all four). No step |
| 24 | Living-doc honesty the arc never assigned: `CLAUDE.md`'s Project opening sentence still describes a Markdown-only library; its Commands browser-suite sentence names three specs; `README.md` feature bullet 1 and the block-ID flow line say Markdown only; `docs/Quick_Start.md` shows no HTML run | nowhere — found by this plan's sweep of the wave 0–6 File Structure tables (wave 0 fixed members/wasm counts, wave 2 invariant 1 + the "regenerated Markdown" sweep, wave 5 the Tech-Stack parser clause, wave 6 the `render` row — none touches these) | **Discharged** — Task 5, declared as Deviation 2 (a scope addition grounded in wave 0's own CLAUDE.md rationale) |
| 25 | `docs_index_drift` standing allowance | wave 6 Global Constraints | **Inherited** — while DCR-0039 (and this plan file's own move-and-link) are pending the controller, `docs_index_drift` is expected red naming exactly those files; any other name is real drift and a STOP |
| 26 | ti `490d97` itself | TicGit | **Not closed by this wave's implementer by default** — the feature-complete verdict and the ticket close ride the controller/owner after acceptance; Task 8 Step 5 captures the ticket state and says so in the hand-off. (If the executing agent verifiably holds the claim, `ti-finish` applies — but never assume the claim) |
| 27 | Owed spec amendment I — §8 step 6's stated mechanism is measured-false: DOMPurify 3.2.6 passes a bare `<li>` as a direct `<main>` child through **unchanged, in place** — relocation is a table-family parser behaviour (`<tr>` without `<table>`), not `<li>`'s. The grouping mandate stands on D9's model; an ungrouped pane is structural drift the suite's count/equality assertions catch | nowhere — this wave's 2026-08-21 adversarial review, empirical probe against the vendored `web/vendor/purify.min.js`; the amendment-E erratum class | Same batch — Task 8 Step 3, item I |
| 28 | `docs/Troubleshooting.md`'s `.transync-out-dir` paragraph states the marker-less complete-set rule as the four-name set (`out.md`, `alignment.json`, `validation-report.json`, `html/`) that wave 6's Deviation 7 replaced with a predicate, and does not know `out.html` | nowhere — no wave plan names `Troubleshooting.md`, and Task 5's sweep greps only "not implemented", which cannot see this; the review's implicit-class sweep | **Discharged** — Task 5 Step 6 |
| 29 | `docs/Developer_Guide.md`'s bundle-title section — "titles itself with … the source document's **first level-1 heading**" — but after wave 6 an HTML run's middle rung is the source `<title>` (ti `0f26b5`'s chain, middle rung re-seated; DCR-0038) | nowhere — unassigned by any wave, unswept, in a file this plan already edits twice; the review's implicit-class sweep | **Discharged** — Task 5 Step 1, second bullet |
| 30 | `docs/backlog.md`'s Type-2 section preamble still frames OI-0035 as an open design question ("is about which layer should own anchor trust in the browser") two paragraphs above the entry Task 7 annotates | nowhere — Deviation 6 found the entry's record gap but, as first written, fixed only half of it; the review's implicit-class sweep | **Discharged** — Task 7 Step 2 item 1 (Deviation 6's other half) |
| 31 | `README.md`'s pipeline diagram — `regenerated MD ◀── regen`, a Markdown-only intake edge, and a `(schema 1.2.0)` label — two bullets below the flow line Task 5 rewords, in a sweep whose stated purpose is that first-contact documents stop describing a Markdown-only library | nowhere — the review's implicit-class sweep (the stale schema label is this plan's own find while re-reading the art) | **Discharged** — Task 5 Step 3, third bullet |

---

## Global Constraints

- **Temp files ONLY under `/Volumes/Temp/claude/ti490d97-wave7/`** — never `/tmp`, never `/private/tmp`, never `$TMPDIR`, never the OS default, never the harness scratchpad. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user**. (The browser fixture workdir `scripts/test-browser.sh` manages for itself is its own, pre-existing arrangement and is not this plan's to move.)
- **NEVER change or override `CARGO_TARGET_DIR`; never pass `--target-dir`.** If a cargo command fails because the target dir is unreachable, stop and ask.
- **Every `cargo test` invocation is capped:** `cargo test -p <crate> -- --test-threads=4`; workspace runs `cargo test --workspace -- --test-threads=4`. Never raise the cap.
- **Capture test runs bare-to-file, never `| grep | tail`:** run the command with no pipeline, redirect to a file under `/Volumes/Temp/claude/ti490d97-wave7/gate/`, append `echo "CARGO_EXIT=$?"` (or `SUITE_EXIT`/`PW_EXIT` as named) to the same file, and inspect the file as a **separate** step. A pipeline reports the last stage's status, so a failing suite reads as a pass.
- **If a number is offered as evidence, capture it to a file.** Grep counts, exit codes, Playwright pass lines, diff stats — every number an expected-result line names must exist in a file under `gate/`, not only in the transcript.
- **`git commit --no-verify` is never used.** The tracked pre-commit hook runs fmt, clippy, the two-package wasm gate and the rustdoc gate; if the hook blocks, fix the cause.
- **Stage exact paths only** — never `git add -A`, never `git add <directory>`. Every commit step names its files.
- **The enum charter: no `_ =>` catch-all arm and no `matches!(x, Variant)` in dispatch/production code this wave writes.** This wave's only Rust edit is three constant edits in one weld test — two `const` arrays and one panic-message string — no dispatch code exists to violate it — but the charter binds anyway, and any incidental Rust this wave finds itself writing names every arm.
- **NEVER run `regen_prompt_goldens` and NEVER set `TRANSYNC_REGEN_GOLDENS`.** This wave touches no prompt bytes; a red prompt golden is a finding — STOP and diagnose.
- **Both `sync.js` copies move in one commit** if the engine file is touched at all. This wave's declared engine diff is **empty** — `web/js/sync.js`, `crates/transync-cli/web/sync.js` and `web/js/wasm-demo.js` are on the not-touched list, and the acceptance diff proves it. If a finding ever forces an engine edit, STOP: that is a wave-1/wave-6 defect to hand back, not a wave-7 edit.
- **`SPEC_FILES` and all three `MUST_NAME_SPECS` documents move in one commit.** The weld's rule, verified in `crates/transync/tests/docs_browser_suite_drift.rs`: `SPEC_FILES` cannot contain a file that any of `docs/implementation/module-map.md`, `docs/Developer_Guide.md`, `web/SMOKE.md` fails to name. Ticket `729ec8` exists because this rule was half-applied once already. Task 3 is one commit for all four files plus the weld.
- **Never state a browser-suite count in a guarded living document.** The same weld's other test scans eight documents (`module-map.md`, `Developer_Guide.md`, `release-checklist.md`, `architecture/README.md`, `mvp-scope.md`, `scenario-matrix.md`, `README.md`, `web/SMOKE.md`) for digit, ratio, parenthetical **and spelled-out** counts within 3 lines of a suite mention. Name the spec files; never count them — not "four tests", not "(4)", not "4/4". Every doc edit in Tasks 3–5 is written under this rule.
- **`grep -c` exits non-zero on a zero count** — the precondition and gate greps read the *printed count*, not the exit code; do not run them under `set -e`. A count **≥ 1** is the expectation unless a step says otherwise.
- Standing wasm gate, string **unchanged**: `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`. No `Cargo.toml` or `Cargo.lock` is touched anywhere in this wave.
- **`docs/index.md` is NOT touched by this plan — the controller owns it.** DCR-0039's exact link line, and the stale spec-annotation replacement, are handed to the controller in Task 8; never added here. While the move-and-link is pending, `docs_index_drift` is expected red naming DCR-0039's file (plus, possibly, this plan's own file); any other name is real drift and a STOP. Consequence for every workspace capture below: `CARGO_EXIT=0`, **or** `CARGO_EXIT=101` whose only failure is `docs_index_drift` naming exactly the allowed file(s) — each later "Expected: `CARGO_EXIT=0`" carries this one carve-out without restating it.
- **The spec file (`docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`) is NOT edited** — the owed amendments go to the controller as Task 8's batch (Deviation 3).
- No pure-formatting edits; `cargo fmt --all` after any Rust edit and take the formatter's answer. `web/tests/` has no formatter hook — match the neighbouring specs' style by hand.
- Korean `*.ko.md` siblings and anything under `manual/` are out of scope: never read, edit, cite, or create them.
- Commit messages end with `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

---

- **The corpus gate's pathspec ends in `/*`, and it must stay that way (added 2026-08-23, ti `490d97` wave 1).** `git diff -- 'crates/*/tests/fixtures'` matches **nothing** — git runs the pattern against the whole path and `*` matches `/`, but the pattern ends at `fixtures`, so only a path *ending* there could match, and every corpus file is one level deeper. Waves 0 and 1 both ran this gate in its vacuous form; both verdicts happened to survive re-checking, which is luck, not evidence. Before trusting any run of it, prove the selector selects something:
```bash
git ls-files -- 'crates/*/tests/fixtures/*' | wc -l   # must be > 0
```
  A filter that matches nothing and a filter whose subject is clean both print silence. That is why the assertion above exists and why the `/*` is not a typo to tidy away.

## File Structure

| Path | Change | Responsibility |
|---|---|---|
| `web/tests/scn16.spec.js` | **create** | THE browser gate: three tests over the `/scn16/` bundle through the shipped shell — the 1.3.0 HTML wire shape, the mount with the title as chrome-not-content, bidirectional block-id sync; typed console capture inline (Deviation 4) |
| `scripts/test-browser.sh` | modify (comments only) | the header comment and the scn16-leg comment name the new spec; **zero logic lines move** (Deviation 1) |
| `crates/transync/tests/docs_browser_suite_drift.rs` | modify | `SPEC_FILES` += `"scn16.spec.js"`; `SUITE_MENTIONS` += `"scn16.spec.js"`; the count test's panic message names the fourth file |
| `docs/implementation/module-map.md` | modify | the `web/tests/` tree comment and the `scripts/test-browser.sh` tree comment name `scn16.spec.js`; the scenario tables gain SCN-16 rows |
| `docs/Developer_Guide.md` | modify | the `./scripts/test-browser.sh` command-table row names `scn16.spec.js`; the scripts-tree line gains SCN-16; the serve section gains the fidelity-door paragraph; the bundle-title section's middle rung gains the HTML-run `<title>` clause (ledger row 29) |
| `docs/Troubleshooting.md` | modify | the `.transync-out-dir` recovery sentence learns wave 6's complete-set predicate and `out.html` (ledger row 28) |
| `web/SMOKE.md` | modify | the spec-file list gains the `scn16.spec.js` bullet |
| `docs/architecture/scenario-matrix.md` | modify | the SCN-16 row, the intro sentence, the block-kind coverage rows, the out-of-scope clause |
| `docs/architecture/contracts.md` | modify | §6's serve note gains the fidelity-door sentence (Task 5); plus whatever Task 6's §10 sweep finds missing — expected nothing |
| `README.md` | modify | feature bullet 1 names HTML documents; the block-ID flow line says "regenerated document" (Deviation 2); the pipeline diagram's intake edge, regen box and schema label stop reading Markdown-only / 1.2.0 (ledger row 31) |
| `CLAUDE.md` | modify | the Project opening sentence names HTML documents; the Commands browser-suite sentence names `scn16.spec.js` (Deviation 2) |
| `docs/Quick_Start.md` | modify | one short HTML-run variant beside the existing translate example (Deviation 2; verify absent first) |
| `docs/backlog.md` | modify | the OI-0035 entry's resolved-in-place note + the Type-2 preamble's resolved tense (ledger row 30); annotations on five existing entries; four new entries (ledger rows 8, 17, 18, 19) |
| `docs/project/status.md` | modify (Task 8) | wave 7 LANDED + feature COMPLETE bullet; the §14.1 open action; the DCR list line |
| `docs/project/phase-state.yaml` | modify (Task 8) | `last_updated`, the DCR-0039 slug, the notes paragraph |
| `CHANGELOG.md` | modify (Task 8) | the `[Unreleased]` browser-gate + closure entries |
| `docs/project/design-change-records/DCR-0039-html-browser-gate-and-closure.md` | **create** (Task 8) | the wave's record: the gate, the falsification evidence, the deferred-item ledger with verdicts, the amendment batch, the controller hand-off |

**Not touched, deliberately:** `web/js/sync.js`, `crates/transync-cli/web/sync.js`, `web/js/wasm-demo.js` (the engine is wave 1's and wave 6's; this wave adds **zero** engine bytes — the acceptance diff proves it, and the twins rule above is therefore vacuous by construction); `web/tests/scn13.spec.js`, `web/tests/engine.spec.js`, `web/tests/wasm.spec.js` (their waves own them; wave 6's test `n` and wave 1's `k`/`l`/`m` are consumed as precedent, never edited); `web/tests/support/harness.js` and `web/tests/support/static-server.mjs` (the typed console collector is inline in the new spec — Deviation 4); `web/playwright.config.js` (testDir discovery is the wiring; nothing to add); every file under `crates/*/src/` (waves 0–6 own all of it; a code defect found here is a STOP-and-hand-back, not a wave-7 fix); every fixture (the SCN-16 fixture is wave 3's, consumed through the wave-6 bundle leg); every `Cargo.toml` and `Cargo.lock`; `crates/transync-cli/tests/*` and every other weld test but `docs_browser_suite_drift.rs` (welds are consumed as gates; this wave's one weld edit is the growth mechanism that test was built for — ticket `729ec8` is the precedent); `docs/index.md` (controller-owned); the spec file (controller-owned amendments — Deviation 3); `docs/project/open-issues.md` and `open-issues-archive.md` (verify-only: OI-0035 moved to the archive in wave 1; the three remaining OPEN entries — OI-0016, OI-0037, OI-0038 — are engine-perf and pipeline items this feature does not move; if the landed tree contradicts this, that is a Task 7 finding to record, not silently edit); `docs/architecture/mvp-scope.md`, `docs/architecture/README.md`, `docs/architecture/source-of-truth-table.md`, `docs/project/release-checklist.md` (verify-only in Task 6's sweep — §10 assigns their edits to earlier waves); `scripts/hooks/pre-commit`, `scripts/smoke.sh`, `scripts/build-wasm.sh`.

---

## Deviations (from the spec's letter, and from the sibling plans)

Seven, declared here and nowhere else.

1. **"Wired into `scripts/test-browser.sh`" is satisfied by testDir discovery plus wave 6's bundle leg; the script gets comment-truth edits only.** §12 wave 7 says "`web/tests/scn16.spec.js` wired into `scripts/test-browser.sh`". The script's own doctrine — stated in `docs/Developer_Guide.md`'s command table and `web/SMOKE.md`, both verified — is that the runner is the authority: `playwright.config.js` sets `testDir: "./tests"` and the script ends in a bare `pnpm exec playwright test`, so every spec under `web/tests/` runs. The SCN-16 *bundle* the spec needs is wave 6's leg (`$HTML_OUT/scn16/`, six files checked, the anchor-free `out.html` grep). Adding a redundant per-spec invocation would contradict the doctrine three documents state. What does move: the script's header comment (which currently narrates SCN-13 + the wasm demo) and the scn16-leg comment gain the spec's name, so the file reads true. Task 3's weld run and Task 2's full-suite capture are the proof of wiring.
2. **The living-document honesty sweep (Task 5) extends §12's literal list to `README.md`, `CLAUDE.md`'s Project/Commands sentences, and `docs/Quick_Start.md`.** §12 wave 7 does not name them; no wave 0–6 File Structure table claims them (verified — wave 0 fixed the member/wasm counts, wave 2 replaced invariant 1 and swept "regenerated/translated Markdown", wave 5 fixed the Tech-Stack parser clause, wave 6 the `render` row); and after wave 6 lands, "a library for translating GFM-compatible Markdown documents" is a false description in the two files every reader and every agent meets first. Wave 0's own CLAUDE.md rationale is the ground: *"a stale `CLAUDE.md` misinforms every agent that never went looking at all."* The sweep is narrow, named per-edit in Task 5, and every touched sentence is re-read from the landed file before editing. The same head covers the review-found stale documents the ledger's rows 28–31 name (`docs/Troubleshooting.md`'s out-dir rule, the Developer Guide's bundle-title chain, the backlog Type-2 preamble — Deviation 6's other half — and README's pipeline diagram): unassigned by any wave, made stale by wave 6's own deliverables, discharged where each file is already in hand.
3. **The owed spec amendments are handed to the controller as a consolidated batch, not applied.** Every wave that owed one (0, 3, 4, 5, 6) recorded it "§14-style, the spec file is not edited in this wave", and wave 5 explicitly hands its amendment "to the controller alongside the index line". Wave 7 keeps the convention: Task 8 Step 3 writes the batch — items A–I: the ledger's A–H plus the review-found §8 step 6 erratum (ledger row 27), each with exact replacement wording — into DCR-0039 and the hand-off, and the spec file stays byte-untouched by the implementer. The alternative (this wave editing the spec) would make the last wave the one that breaks the arc's own record discipline.
4. **The typed console collector is inline in `scn16.spec.js`, not a harness helper — and "zero console warnings" is scoped to `msg.type() === "warning"` plus page errors plus engine-prefixed errors.** `harness.collectConsole` records message *text* only; the acceptance needs the *type*, because browser resource noise (a missing `/favicon.ico` logs a network `error`-typed console message in Chromium) is not a warning and must not fail the gate, while everything the engine says — map/DOM drift, the wave-1 unlisted-anchor gate, forward drift, duplicate anchors, every `loadAlignment` refusal — arrives through `console.warn` (verified: the engine's diagnostic sites are `console.warn`; the success line is a `console.debug`, per `web/SMOKE.md`). The assertion set: zero `warning`-typed messages, zero uncaught page errors (`collectPageErrors`), and zero `error`-typed messages containing the engine's `transync:` prefix. Kept inline so the shared harness — three other specs' dependency — is untouched by the last wave.
5. **Every negative assertion is backed by a positive equality in the same test.** "The title absent from panes" passes vacuously when the selector is wrong or the pane never mounted. The backbone is: `waitForMounted` (mount or fail), then **the pane's anchor list `toEqual` the map's anchor-role row ids, in order, per pane** — which simultaneously proves the pane mounted, carries every promised anchor, carries no extra (a title or `<hr>` anchor would appear as a surplus element), and is order-true. The title negatives (id absent, text absent) then sit on that base, and the title's text is asserted **present** in the one place D5 puts it: `page.title()`, the browser chrome, via the §9 bundle-title chain. Falsification probes P1–P4 (Task 2 Step 5) prove each layer can fail.
6. **The backlog gains a resolved-in-place note for OI-0035's Type-2 entry — a wave-1 record gap fixed by a later wave.** Wave 1's File Structure moved `open-issues.md` and the archive but not `docs/backlog.md` (verified), so the backlog's `sync-anchor-injection-via-raw-html (OI-0035)` entry still reads as an open design question after the issue it indexes was resolved and archived — and so does the **Type-2 section preamble** two paragraphs above it, whose sentence "`sync-anchor-injection-via-raw-html` (OI-0035) is about which layer should own anchor trust in the browser" frames as open what the entry's new note records as landed. Two spots, one fix (Task 7 Step 2 item 1): the entry gains the landed-note AND the preamble sentence moves to the resolved tense, or the section contradicts its own entry. `backlog.md` is a living cross-source index (its own header says so), not a dated record, so correcting it here is doc truth, not history rewriting; the note names DCR-0033 and the archive as the authorities. Recorded in DCR-0039 as a found-and-fixed record gap.
7. **DCR-0039 carries the deferred-item ledger as a first-class section.** No DCR-0032…0038 has one — each carried only its own hand-forwards. A closure record's job is the sweep: every hand-forward from every prior DCR appears with a disposition, and the acceptance includes a mechanical check (grep the seven DCRs' hand-forward sections; every named item must appear in DCR-0039's ledger). This is a shape addition to the house DCR form, declared here.

---

### Task 1: Preconditions STOP gate, DCR collision check, and the recorded green "before"

**Files:**
- Test: none. This task's product is a verified precondition set and a captured baseline.

**Interfaces:**
- Consumes (and verifies landed): wave 0's crate + DCR-0032; wave 1's engine gate + DCR-0033; wave 2's IR split + DCR-0034; wave 3's intake + the SCN-16 fixture + DCR-0035; wave 4's twin + DCR-0036; wave 5's entry point + DCR-0037; wave 6's schema 1.3.0, CLI flag, pane derivation, the `test-browser.sh` scn16 leg, engine test `n`, + DCR-0038.
- Produces: `/Volumes/Temp/claude/ti490d97-wave7/gate/baseline-*.txt`.

- [ ] **Step 1: Create the wave's temp directory.**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave7/gate
```
Expected: no output, exit 0. If `/Volumes/Temp/claude` is unreachable, **stop and ask the user** — do not fall back to `/tmp`.

- [ ] **Step 2: Hard precondition gate — waves 0 through 6 must be COMPLETE, and DCR-0039 must be free.** Run each line separately (not under `set -e`; read the printed counts; capture the whole transcript to `gate/preconditions.txt`):
```bash
# wave 0 — the mechanics crate
grep -c 'pub fn strip_reserved_sync_attrs' crates/transync-html/src/lib.rs
ls docs/project/design-change-records/DCR-0032-*.md
# wave 1 — the engine gate, both twins, and the OI-0035 browser leg
grep -c 'synchronizableRowIds' web/js/sync.js
grep -c 'synchronizableRowIds' crates/transync-cli/web/sync.js
grep -c 'oi0035' scripts/test-browser.sh
ls docs/project/design-change-records/DCR-0033-*.md
# wave 2 — the IR split
grep -c 'pub enum SourceFormat' crates/transync-syntax/src/id.rs
grep -c 'Title => SyncRole::NonSync' crates/transync-syntax/src/align.rs
ls docs/project/design-change-records/DCR-0034-*.md
# wave 3 — the intake and THE fixture the browser bundle is built from
grep -c 'pub fn parse' crates/transync-syntax/src/intake/html.rs
ls crates/transync/tests/fixtures/scn-16-html-document.html
grep -c '<title>Transync &amp; the two-pane page</title>' crates/transync/tests/fixtures/scn-16-html-document.html
ls docs/project/design-change-records/DCR-0035-*.md
# wave 4 — the twin
grep -c 'pub fn full_rescan_html' crates/transync-core/src/validate/full_rescan_html.rs
ls docs/project/design-change-records/DCR-0036-*.md
# wave 5 — the entry point
grep -c 'pub input_format: SourceFormat' crates/transync-core/src/lib.rs
ls docs/project/design-change-records/DCR-0037-*.md
# wave 6 — schema 1.3.0, the CLI flag, the pane derivation, the scn16 leg, engine test n
grep -c '"1\.3\.0"' crates/transync-syntax/src/align.rs
grep -c 'input_format' crates/transync-cli/src/translate_cmd.rs
grep -c 'render_source_html' crates/transync-syntax/src/render.rs
grep -c 'scn16' scripts/test-browser.sh
grep -c 'scn16/source.html' web/tests/engine.spec.js
ls docs/project/design-change-records/DCR-0038-*.md
# this wave's number must be free — expect NO output (and a non-zero ls exit)
ls docs/project/design-change-records/DCR-0039-* 2>/dev/null
# and the weld must not already carry the spec — expect 0
grep -c 'scn16' crates/transync/tests/docs_browser_suite_drift.rs
```
Expected: every `grep -c` prints **≥ 1** except the last, which prints **0**; every `ls` except `DCR-0039-*` echoes path(s); the `DCR-0039-*` ls prints **nothing**. **Any zero where ≥ 1 is expected, any missing DCR-0032…0038, any existing DCR-0039, or a non-zero `scn16` count in the weld is a STOP** — hand the collision or the unfinished wave back to the controller. The fixture-title grep matters specifically: Task 2's `TITLE_TEXT` constant is derived from it; if the landed fixture's `<title>` differs from the wave-3 plan's text, take the landed bytes and adjust the constant — the landed tree wins over every quote in this plan (the wave-6 plan's Notes state the same rule for the same reason: the upstream shapes here are quoted from plans that were mid-landing when this was written).

- [ ] **Step 3: Verify the two already-satisfied ledger rows, so nobody re-does them.**
```bash
grep -n 'html-to-html-translation-design' docs/index.md
grep -c 'ALLOW_DATA_ATTR\|DOMPurify' web/tests/scn13.spec.js
```
Expected: the first prints exactly one line (the spec's existing link — ledger row 7; do **not** add another); the second prints ≥ 1 (wave 1's DOMPurify measurement — ledger row 20). Capture both into `gate/preconditions.txt`. Also read the printed `docs/index.md` line: the landed annotation reads "Implementation is authorized and under way … wave 0 (the pure refactor) is landing on `master` now" (verified 2026-08-21 — the controller already corrected this line once), and it goes stale again in a new direction the moment this wave closes the feature. Whatever the landed sentence turns out to say, capture it verbatim; Task 8's hand-off (the DCR and the commit message) carries the replacement annotation.

- [ ] **Step 4: Capture the baseline, bare-to-file.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-cli.txt
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown > /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-wasm.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-wasm.txt
scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-browser.txt 2>&1
echo "SUITE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-browser.txt
git rev-parse HEAD > /Volumes/Temp/claude/ti490d97-wave7/gate/baseline-commit.txt
```
Read each file separately. Expected: `CARGO_EXIT=0` in the cli and wasm captures; `SUITE_EXIT=0` and the closing `[test-browser] OK` line in the browser capture (this run also proves wave 6's scn16 bundle leg regenerates cleanly — the leg the new spec consumes); `baseline-workspace.txt` `CARGO_EXIT=0` under the standing-allowance carve-out. Anything else red is a STOP: this wave starts from green or it does not start.

---

### Task 2: `web/tests/scn16.spec.js` — the shell-driven browser gate, with its falsification protocol

**Files:**
- Create: `web/tests/scn16.spec.js`
- Modify: `scripts/test-browser.sh` (two comment lines)

**Interfaces:**
- Consumes: wave 6's bundle leg (`$HTML_OUT/scn16/` — `index.html`, `source.html`, `target.html`, `alignment.json`, `sync.js`, `purify.min.js`); the shipped shell's pane ids `#source`/`#target`; `harness.js`'s existing helpers (`readFixture` resolves against `TRANSYNC_FIXTURE_DIR`, so `readFixture("scn16/alignment.json")` reads the leg's map); the §9 bundle-title chain (flag > source `<title>` > `transync` — the leg passes no `--title`, so the shell's `<title>` is the fixture's own, echoed by the stub).
- Produces: the browser evidence for §12's acceptance line, and the falsification captures under `gate/`.
- **This is a characterization gate over a landed feature (waves 0–6), not red-first TDD** — a first honest run is expected green. What replaces the red is Step 5's falsification protocol: each key assertion is driven red against a **mutated copy of the real bundle**, with the exact expected failure named, and the mutations live entirely in the temp fixture dir — zero repo edits, nothing to revert in git.

- [ ] **Step 1: Write the spec.** Create `web/tests/scn16.spec.js` with exactly this content:
```js
// SCN-16 — the HTML-run bundle through the shipped shell, headless.
//
// Wave 7 of ti 490d97 (spec §12): the browser gate. scn13.spec.js drives
// the SCN-14 Markdown bundle; this file drives the SCN-16 HTML-run bundle
// that scripts/test-browser.sh publishes at <fixture>/scn16/ (the wave-6
// leg) through the SAME shipped shell — fetch, DOMPurify fail-closed
// mount, sync.js — the way an operator meets an HTML run's --html-out.
// engine.spec.js test n drives these pane files WITHOUT the shell; this
// spec is the shell-driven half that plan deliberately left to wave 7.
//
// The acceptance line, verbatim: bidirectional sync by block id, the
// title absent from panes, zero console warnings.
//
// One presentation this spec uniquely proves: the fixture's custom
// element (<x-note>) is unknown to spec §4's tables, so wave 6's
// 2026-08-21 ruling puts its anchor on a transparent <div> wrapper —
// self-injection would die in DOMPurify, which removes an unknown
// element and every attribute riding it. Every earlier browser proof
// deliberately bypassed the sanitizer (engine.spec.js test n: "no
// shell, no fetch, no DOMPurify"), so THIS suite is the first to show
// that anchor surviving the real mount; probe P4 drives the pre-ruling
// presentation red against the same vendored sanitizer.
//
// TRACE: SCN-16
// TRACE: ti 490d97 wave 7 (spec 2026-08-20 §8, §11, §12)

import { test, expect } from "@playwright/test";
import {
  activeSyncId,
  advanceFrames,
  collectPageErrors,
  constrainPanes,
  maxScrollOf,
  offsetTopOf,
  readFixture,
  setScrollTop,
  waitForMounted,
  waitForScrollNear,
} from "./support/harness.js";

// The shell page of the SCN-16 bundle leg (scripts/test-browser.sh).
const SHELL = "/scn16/index.html";

// The source document's <title> text, decoded — from the wave-3 fixture
// crates/transync/tests/fixtures/scn-16-html-document.html
// (`<title>Transync &amp; the two-pane page</title>`). Unique on
// purpose: no body block carries this string, so "absent from the panes"
// below is a claim about THE TITLE, not about text the panes never had.
// The stub provider echoes, so source and translated title agree.
const TITLE_TEXT = "Transync & the two-pane page";

// Typed console capture. harness.collectConsole records text only; the
// zero-warnings acceptance needs the TYPE: browser resource noise (a
// favicon 404) arrives as an error-typed network message, while
// everything the engine says — map/DOM drift, the unlisted-anchor gate,
// forward drift, duplicate anchors — arrives through console.warn.
// MUST be called before page.goto so mount-time warnings are captured.
function collectConsoleTyped(page) {
  const entries = [];
  page.on("console", (msg) =>
    entries.push({ type: msg.type(), text: msg.text() })
  );
  return entries;
}

// The acceptance's channel, plus the two ways an engine failure could
// dodge it: uncaught exceptions are collected separately, and an
// error-typed message carrying the engine's own prefix counts too.
function engineNoise(entries) {
  return entries.filter(
    (m) =>
      m.type === "warning" ||
      (m.type === "error" && m.text.includes("transync:"))
  );
}

function anchorRows(map) {
  return map.blocks.filter((r) => r.sync_role !== "non-sync");
}

test.describe("SCN-16 HTML-run bundle through the shipped shell", () => {
  test("a — the served map is the 1.3.0 HTML shape and the title row is honest", async () => {
    // Wire assertions on the served artifact — the positive base tests b
    // and c lean on, asserted here once so their failures read as wire
    // regressions rather than as mysterious pane emptiness.
    const map = JSON.parse(readFixture("scn16/alignment.json"));
    expect(map.schema_version).toBe("1.3.0");
    expect(map.input_format).toBe("html");

    // D5: exactly one title row, translated-but-non-sync, html-spelled.
    const titles = map.blocks.filter((r) => r.block_kind === "title");
    expect(titles.length).toBe(1);
    expect(titles[0].sync_role).toBe("non-sync");
    expect(titles[0].source_format).toBe("html");
    // The echo stub accepts every unit; either accepted status is honest
    // here — what must NOT appear is a fallback.
    expect(["translated", "preserved"]).toContain(titles[0].fallback_status);

    // Every row of an HTML-intake map declares the html spelling (§3's
    // format == Html invariant), and enough anchors exist to sync at all.
    for (const row of map.blocks) {
      expect(row.source_format, row.source_block_id).toBe("html");
    }
    expect(anchorRows(map).length).toBeGreaterThanOrEqual(5);
  });

  test("b — the shell mounts the panes; the title is chrome, never content", async ({
    page,
  }) => {
    const consoleLog = collectConsoleTyped(page);
    const errors = collectPageErrors(page);
    await page.goto(SHELL);
    await waitForMounted(page);

    const map = JSON.parse(readFixture("scn16/alignment.json"));
    const expected = anchorRows(map).map((r) => r.source_block_id);
    const titleId = map.blocks.find(
      (r) => r.block_kind === "title"
    ).source_block_id;

    // POSITIVE BACKBONE: each pane carries exactly the anchors the map
    // promises — same ids, same order. This is what keeps the negatives
    // below falsifiable: an empty pane, a wrong selector, a renamed id,
    // a leaked non-sync block (title OR the fixture's <hr>) all fail
    // HERE, as a surplus or a hole in this list, instead of passing
    // vacuously there.
    for (const sel of ["#source", "#target"]) {
      const ids = await page.evaluate(
        (s) =>
          Array.from(
            document.querySelectorAll(`${s} [data-sync-id]`),
            (el) => el.dataset.syncId
          ),
        sel
      );
      expect(ids, sel).toEqual(expected);
    }

    // The wave-6 2026-08-21 ruling is what makes the equality above
    // satisfiable at all: the fixture's custom element is unknown to spec
    // §4's tables, so its anchor rides a transparent <div> wrapper —
    // DOMPurify removes an unknown element and every attribute on it, so
    // a self-injected <x-note> anchor would die in this very mount (probe
    // P4 drives that red). Positive: the html-kind row's anchor exists in
    // both panes and IS the wrapper.
    const htmlRow = map.blocks.find(
      (r) => r.block_kind === "html" && r.sync_role !== "non-sync"
    );
    expect(htmlRow, "the fixture's <x-note> mints an anchor-role html row").toBeTruthy();
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate(
          ([s, id]) =>
            document.querySelector(`${s} [data-sync-id="${id}"]`).tagName,
          [sel, htmlRow.source_block_id]
        ),
        sel
      ).toBe("DIV");
    }

    // NEGATIVE (D5), on that base: nothing in either pane claims the
    // title's id, and the title's text is not pane content...
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate(
          ([s, id]) =>
            document.querySelectorAll(`${s} [data-sync-id="${id}"]`).length,
          [sel, titleId]
        ),
        sel
      ).toBe(0);
      expect(
        (await page.locator(sel).innerText()).includes(TITLE_TEXT),
        sel
      ).toBe(false);
    }
    // ...while the SAME text IS the page's chrome: the §9 bundle-title
    // chain (flag > source <title> > "transync") put it on the browser
    // tab. Translated, aligned, rendered by chrome — never by the pane.
    expect(await page.title()).toBe(TITLE_TEXT);

    // D9: item anchors live under a reconstructed list group — never a
    // bare <li> as a direct <main> child. Measured (the vendored DOMPurify
    // 3.2.6): the sanitizer passes a bare <li> through unchanged, in
    // place — no relocation — so the mount will neither repair nor betray
    // an ungrouped pane; these structural assertions are what catch it,
    // and the grouping is mandated by D9's model, not by any sanitizer
    // behaviour (amendment I corrects §8 step 6's claim). Positive twin:
    // the group really exists and carries anchored items.
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate(
          (s) => document.querySelectorAll(`${s} main > li`).length,
          sel
        ),
        sel
      ).toBe(0);
      expect(
        await page.evaluate(
          (s) =>
            document.querySelectorAll(`${s} main > ul > li[data-sync-id]`)
              .length,
          sel
        ),
        sel
      ).toBeGreaterThanOrEqual(2);
    }

    // Zero console warnings — the acceptance's third clause. Everything
    // the engine can complain about (a map row with no DOM anchor, an
    // anchor no row claims, schema drift, duplicates) lands in this set.
    expect(errors).toEqual([]);
    expect(engineNoise(consoleLog)).toEqual([]);
  });

  test("c — bidirectional sync by block id over HTML-derived anchors", async ({
    page,
  }) => {
    const consoleLog = collectConsoleTyped(page);
    const errors = collectPageErrors(page);
    await page.goto(SHELL);
    await waitForMounted(page);
    await constrainPanes(page, 160);
    // Cold-start warm-up before the first timed wait (scn13 test a's
    // rationale — harness plumbing, no engine effect).
    await advanceFrames(page, 12);

    const map = JSON.parse(readFixture("scn16/alignment.json"));
    const anchors = anchorRows(map).map((r) => r.source_block_id);
    // Forward driver: the LAST h2-prefixed anchor — mid-document, real
    // travel, reachable (scn13's DRIVER_BLOCK rationale). Reverse target:
    // the document's first anchor row (the <h1>). Derived from the map,
    // not hard-coded, so an id-assignment change upstream moves the test
    // with it instead of silently hollowing it.
    const driver = anchors.filter((id) => id.startsWith("h2-")).pop();
    const top = anchors[0];
    expect(driver).toBeTruthy();

    // The panes must genuinely overflow, or sync-by-scroll is untestable
    // — fail loudly rather than pass vacuously (scn13's guard; the
    // SCN-16 document is smaller than SCN-14, hence 160 px panes and a
    // 60 px floor).
    expect(await maxScrollOf(page, "#source")).toBeGreaterThan(60);
    expect(await maxScrollOf(page, "#target")).toBeGreaterThan(60);

    // The follow target must be reachable, or the settle wait would time
    // out against the pane's scroll ceiling rather than the engine. If
    // THIS guard trips, shrink the pane height above — do not widen the
    // tolerance below.
    const tgtOffset = await offsetTopOf(page, "#target", driver);
    expect(tgtOffset).toBeLessThanOrEqual(await maxScrollOf(page, "#target"));

    // Forward: drive the source; the target follows to the same block.
    await setScrollTop(
      page,
      "#source",
      await offsetTopOf(page, "#source", driver)
    );
    await waitForScrollNear(page, "#target", tgtOffset, 8);
    expect(await activeSyncId(page, "#target")).toBe(driver);

    await page.waitForTimeout(160); // programmatic-scroll lock decay

    // Reverse: drive the target back to the top block; the source follows.
    const srcOffset = await offsetTopOf(page, "#source", top);
    await setScrollTop(
      page,
      "#target",
      await offsetTopOf(page, "#target", top)
    );
    await waitForScrollNear(page, "#source", srcOffset, 8);
    expect(await activeSyncId(page, "#source")).toBe(top);

    expect(errors).toEqual([]);
    expect(engineNoise(consoleLog)).toEqual([]);
  });
});
```
  *What implementation bug makes each assertion red — named, or it is decoration:*
  - **test a, `sync_role` = `"non-sync"`:** wave 2's explicit `Title => SyncRole::NonSync` arm regressing to the `_ => SyncRole::Anchor` catch-all — D5's exact failure, the one §3 warned "the compiler will not ask for".
  - **test a, `input_format`/`source_format`/version:** wave 6's wire fields dropped or mispopulated; a schema constant regression.
  - **test b, anchor-list equality:** a pane derivation that drops a block (hole), renders a non-sync row (surplus — the title or the fixture's `<hr>`), loses `write_attrs`' `data-sync-id` spelling (hole), or emits blocks out of source order (order mismatch); a pane that failed to mount at all fails one step earlier at `waitForMounted`.
  - **test b, the html-row anchor is a `DIV`:** the derivation regressed to the pre-ruling self-injection — the anchor spliced into the unknown element's own open tag, which DOMPurify removes with the element; the equality above then also grows a hole and `warnMapDomDrift` fires, but this targeted assertion names the regression instead of leaving it to be inferred from list drift.
  - **test b, title id/text absent:** the derivation rendering non-sync rows; `sync_role_for` mis-classifying `Title`.
  - **test b, `page.title()`:** the §9 bundle-title chain broken — `document_title` no longer preferring the `Title` block, or the shell titling itself `transync` while a source title exists.
  - **test b, `main > li` = 0 / `ul > li` ≥ 2:** the `li` grouping (§8 step 6, `normalize_top_level`) missing or broken. An ungrouped pane is structural drift these count/equality assertions catch directly — measured: DOMPurify 3.2.6 passes a bare `<li>` through unchanged, in place, so no sanitizer behaviour would surface the defect for us; the grouping is D9's model, not a relocation dodge (§8 step 6's relocation claim is amendment I's erratum).
  - **test b/c, `engineNoise` empty:** any of — a map row promising an anchor the pane lacks (`warnMapDomDrift`), a pane anchor no row claims (wave 1's `collectAnchors` gate warning — e.g. a strip regression letting an author's `data-sync-id` through), `KNOWN_SCHEMA` drifting from the emitted map (forward-drift warning), a duplicate anchor.
  - **test c, the two `waitForScrollNear` + `activeSyncId` pairs:** the engine not wiring HTML-derived anchors; target-map byte ranges wrong (the pane geometry desynchronizes from the map); the reverse direction broken by a driver-lock regression. `waitForScrollNear` rejects with its own diagnostic (`#target at N, expected ~M±8`) on timeout.

- [ ] **Step 2: Comment-truth edits in `scripts/test-browser.sh`.** Two edits, comments only, zero logic lines:
  - The header comment block (it opens `# transync — SCN-13 headless browser suite.` and narrates the SCN-13 bundle + wasm demo): after the sentence block describing the wasm demo leg, add one line to the narration: `# The SCN-16 HTML-run bundle leg below feeds web/tests/scn16.spec.js — the shell-driven browser gate for ti 490d97 (spec §12 wave 7).` and add `# TRACE: SCN-16` beside the existing `# TRACE:` lines.
  - The scn16 leg's own comment (wave 6's `# --- SCN-16 leg (HTML→HTML, ti 490d97 wave 6) ---` block, which says the engine suite reads its pane files off disk): append one sentence to that comment: `# Wave 7's web/tests/scn16.spec.js additionally drives this bundle's shell at /scn16/index.html.`
  Read the landed comment text first; anchor on the quoted phrases, not on line numbers. Expected: `git diff scripts/test-browser.sh` shows only `#`-prefixed lines.

- [ ] **Step 3: Run the new spec inside the real runner, capture.**
```bash
scripts/test-browser.sh > /Volumes/Temp/claude/ti490d97-wave7/gate/t2-suite.txt 2>&1
echo "SUITE_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t2-suite.txt
```
Read the file. Expected: `SUITE_EXIT=0`; the Playwright list output names all three new tests (`a — the served map is the 1.3.0 HTML shape…`, `b — the shell mounts the panes…`, `c — bidirectional sync by block id…`) as passed alongside every pre-existing spec; the run ends `[test-browser] OK`. A red here is a **finding about waves 0–6**, not about this spec — diagnose before touching the spec; the spec is only wrong if a probe in Step 5 disagrees with its stated expectation. Two adjustable harness constants, named in advance with their symptom: the 160 px pane height / 60 px overflow floor (a too-tall pane fails the overflow guard — shrink the height), and the reachability guard (if `tgtOffset` exceeds `maxScroll`, shrink the height — never the tolerance).

- [ ] **Step 4: Keep the generated fixture for the probes.** The suite leaves its bundle at the workdir `scripts/test-browser.sh` printed (default `/Volumes/Temp/claude/transync-browser-fixture`). Copy the scn16 bundle aside so mutations never race a regeneration:
```bash
cp -R /Volumes/Temp/claude/transync-browser-fixture/html /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
```
Expected: exit 0; `probe-fixture/scn16/alignment.json` exists.

- [ ] **Step 5: The falsification protocol — four probes, each driving a named assertion red against a mutated real bundle.** All runs use the Node stand-in server (no `TRANSYNC_SERVE_BIN`), pointed at the mutated copy — zero repo edits. From `web/`:
```bash
# P1 — a title anchor leaks into the panes (mutate BOTH pane files).
TITLE_ID=$(python3 -c "import json;m=json.load(open('/Volumes/Temp/claude/ti490d97-wave7/probe-fixture/scn16/alignment.json'));print(next(r['source_block_id'] for r in m['blocks'] if r['block_kind']=='title'))")
for f in source.html target.html; do
  python3 - "$f" "$TITLE_ID" <<'EOF'
import sys, pathlib
p = pathlib.Path('/Volumes/Temp/claude/ti490d97-wave7/probe-fixture/scn16')/sys.argv[1]
t = p.read_text()
p.write_text(t.replace('</main>', f'<p data-sync-id="{sys.argv[2]}">chrome leak</p></main>', 1))
EOF
done
TRANSYNC_FIXTURE_DIR=/Volumes/Temp/claude/ti490d97-wave7/probe-fixture pnpm exec playwright test tests/scn16.spec.js > /Volumes/Temp/claude/ti490d97-wave7/gate/probe1.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/probe1.txt
```
Expected in `probe1.txt`: `PW_EXIT` non-zero; test **b** fails at the **anchor-list equality** — `expect(received).toEqual(expected)` with the received array carrying one extra element, the title's id, at its tail (both panes). Had the leak been engineered past the equality, the D5 count (`Received: 1`, `Expected: 0`) and `engineNoise` (the wave-1 unlisted-anchor mount warning — the title's id is in the map but non-sync, so it is outside `rowIdSet`) stand behind it. Tests a and c: a passes (the map was not mutated); c red or green is irrelevant to the probe.
```bash
# P2 — restore, then dissolve the target pane's list group.
rm -rf /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
cp -R /Volumes/Temp/claude/transync-browser-fixture/html /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
python3 - <<'EOF'
import pathlib, re
p = pathlib.Path('/Volumes/Temp/claude/ti490d97-wave7/probe-fixture/scn16/target.html')
t = p.read_text()
# Drop the <ul>/</ul> wrapper pair around the li group, leaving bare li
# elements as direct <main> children.
t = re.sub(r'<ul[^>]*>', '', t, count=1)
t = t.replace('</ul>', '', 1)
p.write_text(t)
EOF
TRANSYNC_FIXTURE_DIR=/Volumes/Temp/claude/ti490d97-wave7/probe-fixture pnpm exec playwright test tests/scn16.spec.js > /Volumes/Temp/claude/ti490d97-wave7/gate/probe2.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/probe2.txt
```
Expected in `probe2.txt`: `PW_EXIT` non-zero; test **b** fails in the D9 block for `#target` at the bare-`li` count — `Expected: 0`, received ≥ 1 — deterministically: DOMPurify 3.2.6 passes a bare `<li>` through unchanged and in place (measured — the vendored sanitizer neither relocates nor repairs it), so the ungrouped pane reaches the structural assertion intact and the count is what catches it, exactly the D9-model ground these assertions stand on (amendment I corrects the spec's relocation claim). Behind it, the `ul > li[data-sync-id]` floor would fail too — the group is gone — but the bare-`li` count fires first. That red is the probe passing: the assertion set cannot be satisfied by an ungrouped pane.
```bash
# P3 — restore, then corrupt the wire: the title row claims to anchor.
rm -rf /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
cp -R /Volumes/Temp/claude/transync-browser-fixture/html /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
python3 - <<'EOF'
import json, pathlib
p = pathlib.Path('/Volumes/Temp/claude/ti490d97-wave7/probe-fixture/scn16/alignment.json')
m = json.loads(p.read_text())
next(r for r in m['blocks'] if r['block_kind'] == 'title')['sync_role'] = 'anchor'
p.write_text(json.dumps(m))
EOF
TRANSYNC_FIXTURE_DIR=/Volumes/Temp/claude/ti490d97-wave7/probe-fixture pnpm exec playwright test tests/scn16.spec.js > /Volumes/Temp/claude/ti490d97-wave7/gate/probe3.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/probe3.txt
```
Expected in `probe3.txt`: `PW_EXIT` non-zero; test **a** fails first and deterministically — `expect(received).toBe(expected)` with `Expected: "non-sync"`, `Received: "anchor"`. Behind it, test b would fail at the equality (the anchor-role list now includes the title id the DOM lacks) and at `engineNoise` (`warnMapDomDrift`'s no-DOM-anchor warning) — the layered net the D5 regression cannot slip.
```bash
# P4 — restore, then regress the panes to the PRE-RULING presentation:
# the custom-element anchor self-injected into <x-note>'s own open tag
# (mutate BOTH pane files) — the exact defect the 2026-08-21 ruling
# closed, driven through the exact vendored sanitizer that kills it.
rm -rf /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
cp -R /Volumes/Temp/claude/transync-browser-fixture/html /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
for f in source.html target.html; do
  python3 - "$f" <<'EOF'
import pathlib, re, sys
p = pathlib.Path('/Volumes/Temp/claude/ti490d97-wave7/probe-fixture/scn16') / sys.argv[1]
t = p.read_text()
# <div {attrs}><x-note ...>text</x-note></div> -> <x-note ... {attrs}>text</x-note>
t2, n = re.subn(
    r'<div( [^>]*data-sync-id="html-[^"]*"[^>]*)>\s*<x-note([^>]*)>(.*?)</x-note>\s*</div>',
    r'<x-note\2\1>\3</x-note>',
    t, count=1, flags=re.S)
assert n == 1, f"wrapper block not found in {p}"
p.write_text(t2)
EOF
done
TRANSYNC_FIXTURE_DIR=/Volumes/Temp/claude/ti490d97-wave7/probe-fixture pnpm exec playwright test tests/scn16.spec.js > /Volumes/Temp/claude/ti490d97-wave7/gate/probe4.txt 2>&1
echo "PW_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/probe4.txt
```
Expected in `probe4.txt`: `PW_EXIT` non-zero; test **b** fails at the **anchor-list equality** — `expect(received).toEqual(expected)` with the received array carrying a **hole** where the map's `html-…` id should be, both panes: DOMPurify removed the self-injected `<x-note>` with its anchor and kept only the block's text — the wave-6 ruling's measured hazard, now demonstrated through the shipped shell. Behind the equality stand the targeted `DIV` assertion (`querySelector` returns null — the anchor is simply gone) and `engineNoise` (`warnMapDomDrift`'s `transync: alignment map block "html-…" has no DOM anchor`). This is the one probe **no earlier wave could run**: every prior browser proof deliberately bypassed the sanitizer (wave 6's test `n` is documented "no shell, no fetch, no DOMPurify"), so the pre-ruling defect passed every gate before this one. Tests a and c: a passes (the map was not mutated); c red or green is irrelevant to the probe.
```bash
rm -rf /Volumes/Temp/claude/ti490d97-wave7/probe-fixture
```
All four probe captures are acceptance evidence (wave acceptance 2). **The probes prove the negatives and the warning channel can fail; if any probe comes back green, the spec is decoration on that axis — STOP and fix the spec before commit.**

- [ ] **Step 6: Commit.**
```bash
git add web/tests/scn16.spec.js scripts/test-browser.sh
git commit -m "test(web): SCN-16 gets its shell-driven browser gate

scn16.spec.js drives the wave-6 bundle leg's /scn16/ shell — fetch,
DOMPurify fail-closed mount, sync.js — and holds the wave-7 acceptance
line: bidirectional sync by block id, the title absent from panes (and
present exactly once, on the browser tab, per the bundle-title chain),
zero console warnings on the engine's channel. Every negative rides a
positive: each pane's anchor list must equal the map's anchor-role rows
in order, so a leaked title, a rendered hr, a dropped block or a bare li
all surface as list drift rather than vacuous passes. Falsified before
commit against mutated copies of the real bundle: a leaked title anchor,
a dissolved li group, an anchor-role title row, and the pre-ruling
self-injected custom-element anchor (the vendored sanitizer eats it, and
the anchor equality grows the hole) each drive their named assertion
red. test-browser.sh moves by comments only — the
runner is the authority, and a spec under web/tests/ is wired the
moment it exists.

TRACE: SCN-16
TRACE: ti 490d97 wave 7 (spec 2026-08-20 §8, §11, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 3: The weld grows to four spec files — `SPEC_FILES`, the three naming documents, and the module map's SCN-16 rows, one commit

**Files:**
- Modify: `crates/transync/tests/docs_browser_suite_drift.rs`, `docs/implementation/module-map.md`, `docs/Developer_Guide.md`, `web/SMOKE.md`

**Interfaces:**
- The weld's rule (read from the landed test, not recalled): `living_docs_name_the_browser_spec_files` fails when any `MUST_NAME_SPECS` document does not contain any `SPEC_FILES` entry; `no_living_doc_publishes_the_browser_suite_test_count` fails when a `GUARDED` document states a count within `WINDOW = 3` lines of a `SUITE_MENTIONS` string. Growing `SPEC_FILES` without naming the file in all three documents **in the same commit** leaves the weld red in between — the half-application ticket `729ec8` closed.
- The red in this task is genuine and behavioural: the constant moves first, the test names exactly the three documents, the documents then catch up.

- [ ] **Step 1: Grow the weld.** In `crates/transync/tests/docs_browser_suite_drift.rs`, three edits:
  - `const SPEC_FILES: &[&str] = &["scn13.spec.js", "engine.spec.js", "wasm.spec.js"];` → `&["scn13.spec.js", "engine.spec.js", "wasm.spec.js", "scn16.spec.js"];`
  - In `SUITE_MENTIONS`, after the `"wasm.spec.js"` entry, add `"scn16.spec.js",` — so a line naming only the new spec is a line about the suite, and a count parked beside it is caught.
  - In `no_living_doc_publishes_the_browser_suite_test_count`'s assertion message, the parenthetical file list `` (`web/tests/scn13.spec.js`, `web/tests/engine.spec.js`, `web/tests/wasm.spec.js`) `` gains `` , `web/tests/scn16.spec.js` `` before the closing paren — the advice must name the file it is advising about.
  Nothing else in the file moves: the module doc's history paragraphs are dated narration and stay; `count_detector_recognises_the_shapes…` tests the detector, not the suite.

- [ ] **Step 2: Watch the behavioural red, capture it.**
```bash
cargo test -p transync --test docs_browser_suite_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t3-red.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t3-red.txt
```
Read the file. Expected: `CARGO_EXIT=101`; `living_docs_name_the_browser_spec_files` FAILED with its panic listing **exactly three** lines —
```
docs/implementation/module-map.md does not name `scn16.spec.js`
docs/Developer_Guide.md does not name `scn16.spec.js`
web/SMOKE.md does not name `scn16.spec.js`
```
— and both other tests in the file passed. A fourth missing-line, or a `no_living_doc_publishes…` failure, is a STOP: something upstream already wrote about the suite in a shape this plan did not predict; read what it names.

- [ ] **Step 3: Name the spec in the three documents.** Anchor every edit on the quoted landed text (waves 0–6 may have reflowed these files — re-read each hunk first; the naming obligation, not the exact prose, is the contract):
  - **`docs/Developer_Guide.md`**, the `./scripts/test-browser.sh` row of the commands table (it opens `Headless Playwright suite: SCN-13 dual-pane sync (`web/tests/scn13.spec.js`)…`): after the scn13 clause insert `, the SCN-16 HTML-run bundle driven through the shipped shell (`web/tests/scn16.spec.js`)`; keep the rest of the row — the runner-is-authority sentence is Deviation 1's ground and stays verbatim. Also the scripts tree line reading `test-browser.sh       headless Playwright: SCN-13 + the wasm demo` → `test-browser.sh       headless Playwright: SCN-13 + SCN-16 + the wasm demo`.
  - **`web/SMOKE.md`**, the spec-file bullet list (after the `web/tests/engine.spec.js` bullet):
```markdown
- `web/tests/scn16.spec.js` — the SCN-16 HTML-run bundle (an
  `--input-format html` run over the SCN-16 fixture, published by the
  runner at `scn16/` inside the served dir) driven through the shipped
  shell: bidirectional block-id sync over HTML-derived anchors, the
  `<title>` translated but rendered by browser chrome rather than either
  pane, and a console clean of warnings.
```
  - **`docs/implementation/module-map.md`**, two tree comments and the scenario tables:
    - the `web/tests/` tree comment (`# headless Playwright (scn13.spec.js + wasm.spec.js` / `#   + engine.spec.js, …`) gains `+ scn16.spec.js` in its enumeration;
    - the `scripts/test-browser.sh` tree comment (`#   (web/tests/scn13.spec.js + engine.spec.js` / `#    + wasm.spec.js — every spec under web/tests/)`) gains `+ scn16.spec.js` the same way;
    - the scenario tracing table (the one whose SCN-15 row reads `HTML-content translation end-to-end (post-MVP)`) gains, after SCN-15, a row in the same column shape — verify the landed header first and mirror it:
```markdown
| SCN-16  | HTML→HTML document translation end-to-end (post-MVP)        | `transync-html`, `{intake::html, id (Spelling/SourceFormat), regen, validate (full_rescan_html), render (html_pane), align}`, `web/js/sync.js` | `scn-16-html-document.html`; `web/tests/scn16.spec.js` | Integration + CLI; headless Chromium via Playwright (`scripts/test-browser.sh`) |
```
    - the automation table row that lists `SCN-01..11, SCN-14, SCN-15` extends its range text to include SCN-16 with the same `MockTranslator`/stub clause, and its browser-leg sentence gains `; SCN-16's rides `web/tests/scn16.spec.js``;
    - the **third** parallel per-scenario table, "Persistence / file touches per scenario" (its range row reads `SCN-01..11, SCN-14, SCN-15 | Fixture .md (test-only) | none (in-memory TranslationOutput returned)` — doubly wrong as a home for SCN-16: the fixture is `.html`, and the CLI leg writes), gains its own row after SCN-13's — verify the landed header and mirror it:
```markdown
| SCN-16  | Fixture `.html` (`scn-16-html-document.html`, test-only)         | none in the integration form (in-memory `TranslationOutput` returned); the CLI form writes `out.html`, `alignment.json`, `validation-report.json` and the `html/` bundle into its `--out-dir`, and `scripts/test-browser.sh`'s scn16 leg republishes that bundle into its served fixture workdir each run |
```
  **Count discipline:** every sentence above names files and scenarios; none states how many tests anything holds. Do not add one.

- [ ] **Step 4: Green, then one commit for all four files.**
```bash
cargo test -p transync --test docs_browser_suite_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t3-green.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t3-green.txt
```
Expected: `CARGO_EXIT=0`, all three tests passing — the count scan now also polices lines near `scn16.spec.js` mentions, which is the reason `SUITE_MENTIONS` moved with `SPEC_FILES`.
```bash
git add crates/transync/tests/docs_browser_suite_drift.rs \
  docs/implementation/module-map.md docs/Developer_Guide.md web/SMOKE.md
git commit -m "test+docs: the browser-suite weld grows to scn16.spec.js, and the three naming docs move with it

SPEC_FILES cannot pin a file a MUST_NAME_SPECS document does not name
(ti 729ec8's rule), so the constant, the mention list, the panic
message's advice, and all three documents move in one commit. The docs
name the new spec and state no count, which is the whole convention
(ti e9481b).

TRACE: ti 490d97 wave 7

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 4: The scenario matrix — SCN-16's row, the block-kind rows, the out-of-scope clause

**Files:**
- Modify: `docs/architecture/scenario-matrix.md`

**Interfaces:**
- Spec §10: "SCN-16 row (Integration + CLI + Playwright, the SCN-15 combined-type precedent); block-kind coverage rows for `title` and html-document blocks; the out-of-scope clause updated (attribute text and wasm-demo HTML documents stay out)." The file is GUARDED — the count rule binds every sentence.

- [ ] **Step 1: The intro sentence.** The paragraph `` `SCN-01`..`SCN-14` are the MVP gate. `SCN-15` is **post-MVP** coverage… `` gains, after the SCN-15 sentence:
```markdown
`SCN-16` is **post-MVP** coverage added by the HTML→HTML document-translation wave (ADR-0025, ti `490d97`, DCR-0032..0039) and is held to the same contract discipline.
```

- [ ] **Step 2: The SCN-16 row**, appended after SCN-15 in the main table (same five columns):
```markdown
| SCN-16 | HTML       | `transync translate --input-format html` over `crates/transync/tests/fixtures/scn-16-html-document.html` — doctype, head+title, nested sections, script/style, entities, an unclosed fragment | Translated HTML byte-identical outside text nodes; per-block fallback splices source bytes verbatim; the map is schema 1.3.0 with `input_format: "html"` and every row's `source_format` declared; the `<title>` row is `block_kind: "title"` with `sync_role: "non-sync"`; the published `out.html` is anchor-free; the bundle's panes mount and sync bidirectionally by block id with the title absent from both panes and no console warnings | Integration + CLI + Smoke (Playwright `web/tests/scn16.spec.js`) |
```

- [ ] **Step 3: Block-kind coverage.** The section's lead sentence (`…at least once across SCN-01 through SCN-06 (plus SCN-15 for the HTML kind)`) is extended: `(plus SCN-15 for the HTML kind, and SCN-16 for the HTML-document population — every kind the HTML intake emits, `title` included)`. Two rows appended to its table:
```markdown
| Title (HTML document) | SCN-16 |
| HTML-document blocks (every kind under `Spelling::Html`, `BlockKind::Html` for custom elements included) | SCN-16 |
```

- [ ] **Step 4: The out-of-scope list.** The `MDX, YAML frontmatter, math (NG2)` bullet's parenthetical already ends `…Attribute text, `<template>` content, and HTML nested inside list items/blockquotes remain out of scope.)`. One honesty constraint before writing: **no "HTML documents as input" entry ever stood on this list** (verified — the list is NG3, NG4, the NG2 bullet, the WASM rendering path, and cross-machine deployment), so there is nothing to strike through; what kept HTML documents out was the CLI's preamble-sniff refusal (exit 2), never an out-of-scope entry. The "Raw HTML left this list on 2026-08-04" precedent annotated an item that HAD been listed — do not imitate its strike-through for one that had not. Append one bullet after the NG2 bullet:
```markdown
- HTML-document **attribute text** (`alt`, `title`, `placeholder`, `og:*`) and HTML documents in the **wasm demo** (its render path remains Markdown-only). *(Recorded 2026-08-20 with ADR-0025, ti `490d97`, when whole HTML documents became first-class input via `--input-format html`. HTML documents were never on this list — the CLI's preamble-sniff refusal, not an out-of-scope entry, is what had kept them out — so this bullet records what the feature deliberately leaves out rather than striking anything. The attribute-text limitation is the one ADR-0025 names, visible as a shared link previewing in the source language.)*
```

- [ ] **Step 5: Verify the weld still holds, then commit.**
```bash
cargo test -p transync --test docs_browser_suite_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t4-weld.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t4-weld.txt
```
Expected: `CARGO_EXIT=0`. The SCN-16 row mentions `scn16.spec.js`, so the count scan now polices its 3-line window — the row above states no count, which is why.
```bash
git add docs/architecture/scenario-matrix.md
git commit -m "docs(scenario-matrix): SCN-16 joins the matrix, and the out-of-scope list says what stayed out

The row is the SCN-15 combined-type precedent (Integration + CLI +
Smoke), the block-kind table gains the title and html-document rows, and
the out-of-scope list records the trade ADR-0025 accepted: attribute
text and wasm-demo HTML documents stay out.

TRACE: ti 490d97 wave 7 (spec 2026-08-20 §10, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 5: The serve door, and the living-document honesty sweep

**Files:**
- Modify: `docs/Developer_Guide.md` (two sections), `docs/architecture/contracts.md` (§6, one sentence), `README.md`, `CLAUDE.md`, `docs/Quick_Start.md`, `docs/Troubleshooting.md` (ledger row 28)

**Interfaces:**
- §9's sentence this task makes findable: *"`transync serve` needs no change: `--out-dir` output contains `out.html` beside `html/`, and serve is a static server — D6's fidelity door is open the day `out.html` exists."* Wave 6's coverage map maps it to "nothing touched" — the *documentation* of the door is §12 wave 7's.
- Every edit below re-reads the landed sentence first (waves 0–6 touched all five files); the quoted "current" text is the pre-arc tree's, and the landed tree wins on wording while the obligation stands.

- [ ] **Step 1: The serve door, Developer_Guide.** In the serve section (the paragraph opening `` `transync serve --rendered <dir>` serves that directory over HTTP… ``), append a new short paragraph after the authority/`Host` discussion and before the STUB-061 retirement paragraph:
```markdown
Since the HTML→HTML wave (ADR-0025, ti `490d97`), serve is also the
**fidelity door**: an `--input-format html` run's `--out-dir` holds the
translated page itself — `out.html`, full head, scripts, styles,
anchor-free — beside the sync bundle in `html/`. The bundle's panes
deliberately carry no page fidelity (they are a sync surface — no
layout, no styles, no scripts), so to see the real translated page,
serve the out-dir and open `out.html`:
`transync serve --rendered <out-dir>`.
```
  And a **second edit in the same file** (ledger row 29): the bundle-title section (`### What the bundle calls the document (`--title`)`), whose chain sentence reads `…the first of these that exists: `--title <text>`, the source document's **first level-1 heading**, or the literal `transync``. After "first level-1 heading", insert the format split: ` (a Markdown run) — or, on an `--input-format html` run, the source document's `<title>` text, the same extraction the provider saw as `document_title` (ti `0f26b5`'s chain, middle rung re-seated; DCR-0038)`. Re-read the landed sentence first and keep the rest of the paragraph (the "used as prose" clause describes both extractions truthfully).

- [ ] **Step 2: The serve door, contracts §6.** In §6's `transync serve` note (beside the usage grammar), one sentence, anchored after the sentence describing what serve does:
```markdown
For an HTML run, `--out-dir` publishes `out.html` beside `html/`, and serving that directory is the **fidelity door** (ADR-0025 D6): the real translated page with head, scripts and styles — the bundle's panes are a sync surface, never a fidelity preview.
```

- [ ] **Step 3: README honesty.** Two edits (re-read the landed lines first):
  - Feature bullet 1 (`**Translates [GitHub Flavored Markdown](…) documents** with a consumer-supplied LLM, preserving…`): after `documents`, insert ` — and, since ADR-0025 (ti `490d97`), whole **HTML documents** through the same pipeline (`--input-format html`) —`.
  - The block-ID flow bullet (`**Stable `BlockId` flows through the entire pipeline** — Rust IR → LLM contract → regenerated Markdown → …`): `regenerated Markdown` → `regenerated document`.
  - The pipeline diagram, two bullets below (ledger row 31 — re-read the landed art first): the intake edge `.md  ──▶  parse` becomes `.md | .html  ──▶  parse/intake`, the regen box `regenerated MD  ◀── regen` becomes `regenerated document  ◀── regen`, and the AlignmentMap box's `(schema 1.2.0)` label becomes `(schema 1.3.0)` — the wire §3 emits since wave 6. Realign the box art by hand; add no other text.

- [ ] **Step 4: CLAUDE.md honesty.** Two edits (re-read the landed sentences first — wave 2 swept "translated/regenerated Markdown" and wave 5 re-worded the Tech-Stack parser clause, so the surviving stale text may differ from the pre-arc tree):
  - The `## Project` opening sentence (pre-arc: `` `transync` is a library for translating GFM-compatible Markdown documents with an LLM and rendering source/target side-by-side… ``): after `Markdown documents`, insert ` — and, since ADR-0025 (ti `490d97`), HTML documents through the same pipeline and the same block IR —`.
  - The Commands section's JS-tooling sentence (pre-arc: `…covers SCN-13 (`web/tests/scn13.spec.js`), the wasm demo (`web/tests/wasm.spec.js`), and `sync.js`'s mount-refusal contract driven directly rather than through a shell (`web/tests/engine.spec.js`)`): insert `, SCN-16's HTML-run bundle through the shipped shell (`web/tests/scn16.spec.js`)` after the SCN-13 clause.
  Nothing else in `CLAUDE.md` moves — in particular the invariants (wave 2 owns invariant 1's wording) and every count wave 0 fixed.

- [ ] **Step 5: Quick_Start.** Verify first: `grep -c 'input-format' docs/Quick_Start.md` — if ≥ 1, a prior wave already added the HTML run; skip this step and note it in DCR-0039. If 0, append beside the existing translate example (anchor on the sentence around `out.md (the regenerated Markdown)` — re-read it; if it still says "regenerated Markdown" it becomes "regenerated document" in the same edit):
```markdown
An HTML document goes through the same pipeline — declare the format
(routing is flag-only; the sniff never guesses):

```bash
transync translate --input page.html --input-format html \
  --out-dir out/ --target-language ko
```

That writes `out.html` (the translated page, anchor-free) beside the
same `html/` sync bundle; `transync serve --rendered out/` shows the
real page.
```

- [ ] **Step 6: `docs/Troubleshooting.md` — the out-dir recovery paragraph learns the predicate (ledger row 28).** The `.transync-out-dir` bullet's recovery sentence reads `…unless the complete published set — `out.md`, `alignment.json`, `validation-report.json` and `html/` — is still there, in which case the set itself is evidence enough`. That is the marker-less four-name rule wave 6's Deviation 7 replaced with a predicate, and it does not know `out.html`. Re-word the set to the landed predicate: `…unless the complete published set — `alignment.json`, `validation-report.json`, `html/`, and the run's translated document (`out.md` for a Markdown run, `out.html` for an `--input-format html` run; one of the two, never both) — is still there…`. Re-read the landed paragraph first; no other sentence in the file moves. No wave plan names `Troubleshooting.md`, which is exactly why the ledger had to — the sweep below greps only "not implemented" and structurally cannot see this staleness.

- [ ] **Step 7: The stale-claim sweep, captured.** The closure must prove no living document still calls the feature future or absent:
```bash
grep -rn "not implemented" README.md CLAUDE.md docs/ web/SMOKE.md --include='*.md' | grep -v '\.ko\.md' | grep -v 'superpowers/' | grep -v 'design-change-records/' | grep -v 'decisions/' | grep -v 'open-issues-archive' > /Volumes/Temp/claude/ti490d97-wave7/gate/t5-stale-claims.txt
grep -rn "490d97" crates/ --include='*.rs' -l >> /Volumes/Temp/claude/ti490d97-wave7/gate/t5-stale-claims.txt
```
Read the file. Expected: every surviving "not implemented" hit is either a dated record (DCRs/ADRs/archive — excluded above), a superseded plan/spec (excluded), or about a *different* feature; and every `490d97` source pointer is one wave 6 rewrote into a live meaning (the sniff-refusal message's new tail) rather than an "unimplemented" claim. Any hit that still says HTML→HTML translation is unimplemented is a missed re-text from an earlier wave: **fix it in that file** (it is doc-truth, in scope for a closure sweep), record the find in DCR-0039, and name the wave that owed it.

  Then the weld, captured — this task touches two GUARDED documents (`README.md`, `docs/Developer_Guide.md`) and the pre-commit hook runs fmt/clippy/wasm/rustdoc, **not** the suite, so wave acceptance 4 needs its own evidence for this commit:
```bash
cargo test -p transync --test docs_browser_suite_drift -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t5-weld.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t5-weld.txt
```
Read the file. Expected: `CARGO_EXIT=0` — none of this task's dictated sentences states a suite count (checked against the detector's shapes at plan time), so a red here means a written sentence drifted from the plan: fix the sentence, never the weld.

- [ ] **Step 8: Commit.**
```bash
git add docs/Developer_Guide.md docs/architecture/contracts.md README.md CLAUDE.md \
  docs/Quick_Start.md docs/Troubleshooting.md
git commit -m "docs: the serve door is documented, and the first-contact documents stop describing a Markdown-only library

The fidelity door (spec §9): an HTML run's out-dir holds out.html beside
the sync bundle, and transync serve over it is how you see the real
page — the panes are a sync surface, never a fidelity preview (D6).
README, CLAUDE.md and Quick_Start now say what the tree does: HTML
documents are first-class input through the same pipeline. Three more
staleness fixes ride along, review-found (ledger rows 28, 29, 31):
Troubleshooting's marker-less recovery rule learns the
out.md-or-out.html predicate, the bundle-title chain gains its HTML-run
title rung, and README's pipeline diagram stops reading Markdown-only
with a 1.2.0 label.

TRACE: ti 490d97 wave 7 (spec 2026-08-20 §9, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```
(If Step 5 skipped Quick_Start, drop `docs/Quick_Start.md` from the `git add` list.)

---

### Task 6: "The remaining contracts sections" — the §10 sweep

**Files:**
- Modify: `docs/architecture/contracts.md` — only if the sweep finds a gap. Expected: none.

**Interfaces:**
- Wave 6's hand-forward, verbatim: "any contracts sections §10 lists that no wave-6 commit made true". Every §10 contracts item was assigned to a wave 0–6 plan (verified across their File Structure tables); this task is the closure's proof, not a writing task — unless the landed file disagrees, in which case §10's own text is the source for what gets written.

- [ ] **Step 1: The checklist, run against the landed `contracts.md` and captured.** One grep per §10 contracts obligation (not under `set -e`; read counts; capture the transcript to `gate/t6-contracts-sweep.txt`):
```bash
grep -c 'SourceFormat' docs/architecture/contracts.md                    # §0 additive rows (wave 5)
grep -c 'transync-html' docs/architecture/contracts.md                   # §0 tier-(c) crate + no-facade-re-export note (wave 0)
grep -c 'Spelling' docs/architecture/contracts.md                        # §1 v0.5.0 entries + exhaustive-by-policy (wave 2)
grep -c '1\.3\.0' docs/architecture/contracts.md                         # §3 schema (wave 6)
grep -c 'input_format' docs/architecture/contracts.md                    # §3 map field (wave 6)
grep -c '"title"' docs/architecture/contracts.md                         # §3 title kind (wave 6)
grep -c 'reserved' docs/architecture/contracts.md                        # §4 reserved-namespace rule present (wave 1/6)
grep -c 'strip_reserved_sync_attrs\|reserved attribute' docs/architecture/contracts.md
grep -c 'full_rescan_html' docs/architecture/contracts.md                # §5a layer-6 dispatch (wave 4)
grep -c 'cache' docs/architecture/contracts.md                           # §5a no-axis paragraph (wave 5) — then READ §5a
grep -c 'input-format' docs/architecture/contracts.md                    # §6 flag grammar (wave 6)
grep -c 'out\.html' docs/architecture/contracts.md                       # §6 layout diagram + (after Task 5) the fidelity sentence
```
Expected: every count ≥ 1. **The counts are presence probes, not truth probes** — after them, read §0/§1/§3/§4/§4a/§5a/§6 once each against §10's enumerated list (§10 is quoted in full in the spec; take each clause in order) and tick every clause. Expected: all present.

- [ ] **Step 2: Disposition.** If everything is present: record "contracts §10 sweep — complete, nothing owed" with the capture path in DCR-0039 (Task 8). If a clause is missing: write it into the named section using §10's/the owning wave plan's text as the source, name the wave that owed it in the commit message and DCR-0039, and commit as its own `docs(contracts): …` commit staging only `docs/architecture/contracts.md`.

---

### Task 7: Backlog and open-issue cross-references, the still-open ledger rows, and the §14.1 evidence

**Files:**
- Modify: `docs/backlog.md`
- Verify only: `docs/project/open-issues.md`, `docs/project/open-issues-archive.md`
- Produces: `gate/phrasing-evidence/`, the ti ticket for ledger row 8

- [ ] **Step 1: Verify the open-issue side.** `docs/project/open-issues.md` should hold OI-0016, OI-0037, OI-0038 (all untouched by this feature) and **no** OI-0035 body (wave 1 archived it; the summary-table row survives by wave 1's design). `grep -c 'OI-0035' docs/project/open-issues.md` — expected: the summary-row mention only (read the hits). `grep -c 'DCR-0033' docs/project/open-issues-archive.md` — expected ≥ 1. Capture both to `gate/t7-issues.txt`. A discrepancy is a wave-1 landing defect: STOP and hand back rather than patch another wave's record file.

- [ ] **Step 2: The backlog cross-references.** Six in-place annotations, each in the file's own house shape (a bold dated note appended to the entry — the `provider-capability-set`/`out-dir-*` precedents). Re-read each landed entry first; wave 0 already re-worded `template-webcomponent-extraction`'s description:
  1. **`sync-anchor-injection-via-raw-html (OI-0035)`** (Type 2) — append (Deviation 6):
```markdown
**Landed 2026-08-20 (route (c) — both layers; ti `490d97` wave 1, DCR-0033):** the
renderer strips the reserved sync-attribute namespace out of raw-HTML blocks on the
way into a pane (`out.md` keeps the author's bytes), and the engine builds its anchor
set from validated alignment rows, so an unlisted anchor is inert forever — including
one inserted after mount. Accepted residual, recorded in DCR-0033: an impostor
carrying a *listed* id ahead of the genuine anchor defeats the engine layer alone;
the render strip is the layer that closes it for panes transync produces.
`docs/project/open-issues-archive.md` carries the resolved entry. This index entry
was not updated when the wave landed; corrected by ti `490d97` wave 7 (DCR-0039).
```
  And in the same edit, the **Type-2 section preamble** two paragraphs above (ledger row 30 — Deviation 6's other half): its sentence `` `sync-anchor-injection-via-raw-html` (OI-0035) is about which layer should own anchor trust in the browser. `` becomes `` `sync-anchor-injection-via-raw-html` (OI-0035) asked which layer should own anchor trust in the browser — resolved 2026-08-20 (route (c), both layers; ti `490d97` wave 1, DCR-0033); its entry below is retained for the record. `` The preamble must not keep framing as an open design question what the entry beneath it now records as landed.
  2. **`html-attribute-text-translation`** — append: `**Cross-reference (2026-08-20, ti `490d97`):** now also the HTML-document intake's recorded limitation — ADR-0025 names the visible consequence (a shared link previews in the source language). The owner gate is unchanged; the population grew from raw-HTML islands to whole documents.`
  3. **`template-webcomponent-extraction`** — append: `**Cross-reference (2026-08-20, ti `490d97`):** an HTML document's `<template>` is DEFAULT-STOP in the new intake — a `BlockKind::Html` block extracting zero segments, honest `PreservedZeroSegment` row — so the exclusion now has an anchor-bearing spelling too. Gate unchanged.`
  4. **`html-same-parent-segment-grouping`**, **`html-native-array-wire-field`**, **`html-br-normalization-guard`** — each gains the same one-liner: `**Population note (2026-08-20, ti `490d97`):** every block of an HTML document now rides the segment engine, not just raw-HTML islands — the telemetry gate is unchanged but has far more traffic to fire on.`
  5. **`dialect-trait`** (Type 3) — append, decision-neutral: `**Evidence note (2026-08-20, ti `490d97`):** a second front-end now exists in-tree — `intake::html` beside the comrak parser, a *module* seam, not a trait; ADR-0025 (D3) explicitly rejected a format-axis crate split. The YAGNI condition ("a real second dialect must supply constraints") should be re-read against ADR-0025 at the next sweep rather than assumed still unmet; nothing in the HTML wave required core to branch on a dialect capability.`

- [ ] **Step 3: The four new entries (ledger rows 8, 17, 18, 19).** Placed in the file's Type sections, in its entry shape (`### name` + Description/Background/Blocked-by):
  - **Type 2** — `phrasing-custom-element-mid-sentence-review` (row 8): Description: a custom element mid-sentence stops segmentation (D4's default), so `Price: <my-price/> today` becomes three blocks and the sentence crosses translation units; accepted *for now* with the PHRASING widening, **and the owner explicitly required re-confirmation against real output** — real pages with web components and icon elements, block sets reviewed. Candidate lever if it fails review: a per-run or per-profile phrasing-extension list, designed then. Background: spec §14.1, §13.5; ADR-0025. Note the staged evidence (`Step 4's capture, cited in DCR-0039`) and the ti ticket (Step 5).
  - **Type 3** — `markdown-island-reclassification` (row 17): Description: Markdown-intake raw-HTML islands stay `BlockKind::Html` (`html-…` ids) although some have semantic equivalents; reclassifying moves block ids and with them alignment rows, DOM anchors and the `block_kind` cache axis. Background: ADR-0025 D11; spec §13.9. Blocked by: a schema-2.x-shaped window — the corpus-stability acceptance criterion forbids it in any 1.x window.
  - **Type 3** — `html-oversize-leaf-block-split` (row 18, with §15.5 folded in): Description: a giant `<table>` or custom element exceeding the token budget has no splitter on the HTML path — the design commits to the *shape* only (a packing-time segment-window split in the DCR-0026 mold, never intake descent; D4 rejected budget-driven block sets) and to the shipped `(Table × Html)` exclusion from the GFM row-window splitter. Watch item riding with it (spec §15.5): if regen ever splices merged multi-element ranges, `BlankLinePolicy::Keep` and slice-agnostic extract/splice hold (measured), but the layer-3 inventory check must then run over the same merged slice. Background: spec §15.4/§15.5. Blocked by: demonstrated need — D9 removed the most common trigger (long lists), and no oversize-leaf abort has been observed.
  - **Type 1** — `parser-intake-markdown-rename` (row 19): Description: the format seam is asymmetric on purpose — `intake::html` beside `parser` — because renaming `parser` to `intake::markdown` moves ~43 references in `transync-syntax`, ~74 in `transync-core`, the wasm import, `public_surface.rs`'s pin and four documents for zero behaviour. Mechanical, unblocked, pick-up-as-is; the paying wave's diff must be *only* the rename. Background: wave 3 deviation 1; DCR-0035.

- [ ] **Step 4: Stage the §14.1 evidence (never a discharge — the owner reviews).**
```bash
mkdir -p /Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence
```
Write `/Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/custom-elements-page.html` — a real-shaped page exercising the §14.1 idioms (a web component mid-sentence, an icon element mid-sentence, a framework-style custom element wrapping a phrase):
```html
<!DOCTYPE html>
<html><head><title>Pricing</title></head><body>
<main>
<h1>Plans</h1>
<p>The Pro plan costs <price-tag currency="USD">29</price-tag> per month.</p>
<p>Click the <fa-icon name="gear"></fa-icon> icon to open settings.</p>
<p>Our <router-link to="/docs">documentation</router-link> covers the rest.</p>
<my-callout kind="tip">Custom elements at block level stop cleanly.</my-callout>
</main>
</body></html>
```
Then run it, plus the in-tree fixtures, through the real intake via the stub CLI, and capture the block sets:
```bash
cargo build -p transync-cli --features test-stub-provider > /Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-build.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-build.txt
```
(read it; `CARGO_EXIT=0`), then for each of `gate/phrasing-evidence/custom-elements-page.html`, `crates/transync/tests/fixtures/scn-16-html-document.html`, `crates/transync-syntax/tests/fixtures/html-corpus/marketing-page.html`:
```bash
<the built binary> translate --input <file> --input-format html \
  --output /Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/<name>-out.html \
  --map /Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/<name>-map.json \
  --target-language ko
```
Expected: exit 0 each; each map's `blocks` array is the evidence — for the custom-elements page, the mid-sentence stops are **visible as split paragraph rows around `html`-kind rows** (the `price-tag` and `fa-icon` sentences each yield three blocks; the `my-callout` line yields one `html` block). Summarize the observed block-kind sequences (three lines per file) in DCR-0039's §14.1 paragraph and in the ticket body below. This is the "against real output" material the owner asked for, staged; the ruling is not this wave's.

- [ ] **Step 5: File the ticket.** Write the body to `/Volumes/Temp/claude/ti490d97-wave7/ti-phrasing-review.md` (the §14.1 text, the candidate lever, the evidence paths and the three observed block-kind sequences), then:
```bash
ti new --help > /Volumes/Temp/claude/ti490d97-wave7/gate/ti-help.txt 2>&1
```
Read the help; then file with the verified syntax (expected shape, per the house convention: title positional, `-F` body file, `-g` tags):
```bash
ti new "Re-confirm the PHRASING widening's custom-element sentence splitting against real output (spec §14.1)" \
  -F /Volumes/Temp/claude/ti490d97-wave7/ti-phrasing-review.md \
  -g html,490d97,owner-decision > /Volumes/Temp/claude/ti490d97-wave7/gate/ti-new.txt 2>&1
```
Expected: a new ticket id printed; capture it — DCR-0039, the backlog entry and status.md all cite it. If `ti` is unavailable in the execution environment, record that in DCR-0039 and leave the backlog entry as the tracker (it already carries the full content).

- [ ] **Step 6: Commit.**
```bash
git add docs/backlog.md
git commit -m "docs(backlog): the HTML wave's cross-references land, and the still-open remainders get durable homes

OI-0035's index entry — and the Type-2 preamble that still framed it as
an open question — finally say what DCR-0033 landed (a wave-1 record
gap, fixed and named, both spots). Five entries gain cross-references the feature
changed the ground truth of. Four new entries carry what wave 7 cannot
close: the owner's §14.1 custom-element re-confirmation (evidence
staged, ticket filed), D11's island reclassification behind its
schema-2.x window, the oversize-leaf splitter with its §15.5 watch note,
and the deferred parser -> intake::markdown rename.

TRACE: ti 490d97 wave 7 (spec 2026-08-20 §12, §13, §14, §15)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

### Task 8: Records — DCR-0039, the routine files, the amendment batch, the controller hand-off

**Files:**
- Create: `docs/project/design-change-records/DCR-0039-html-browser-gate-and-closure.md`
- Modify: `CHANGELOG.md`, `docs/project/status.md`, `docs/project/phase-state.yaml`

**Interfaces:**
- **`docs/index.md` is NOT touched.** This task produces DCR-0039's exact link line and the spec-annotation replacement, and hands both to the controller in the commit message and status.md. The DCR link line, exactly:
```markdown
- [DCR-0039 — the browser gate closes the HTML→HTML feature, and the ledger says what stayed open (2026-08-21)](project/design-change-records/DCR-0039-html-browser-gate-and-closure.md) — ti `490d97` wave 7: `web/tests/scn16.spec.js` (bidirectional sync, title-as-chrome, zero warnings), the four-file suite weld, the closure sweep, and the deferred-item ledger with the owed spec amendments handed to the controller.
```
(date the line to the execution day). Until the controller links it, `docs_index_drift` is expected red naming exactly this file (plus this plan's own file while its move-and-link is pending); any other name is real drift and a STOP.

- [ ] **Step 1: Write DCR-0039.** OKF frontmatter in the house shape (`type: DCR`, `tags: [change, project-control, DCR-0039]`, dated at execution), title: *The browser gate closes the HTML→HTML feature, and the ledger says what stayed open.* Required sections, each with its ground:
  - **Date / Source** — execution date, ti `490d97`, wave 7 of 8 (the last), spec §12.
  - **What changed:** `web/tests/scn16.spec.js` (the three tests, the positive-backbone design of Deviation 5, the inline typed console capture and the warning-channel scoping of Deviation 4); the weld grown to four spec files with `SUITE_MENTIONS` and the panic message moving too, all in one commit with the three naming documents (the `729ec8` rule applied whole); the scenario-matrix rows; the serve-door documentation; the honesty sweep (Deviation 2's list, each landed sentence quoted before/after); the backlog cross-references including the found-and-fixed OI-0035 index gap (Deviation 6).
  - **Falsification evidence** — the four probes, each named with its mutated artifact and the exact assertion it drove red, with capture paths (`gate/probe1..4.txt`); P4 doubles as the empirical demonstration, through the shipped shell, of the hazard wave 6's 2026-08-21 ruling closed. This is the record that the closure wave's test can fail.
  - **The deferred-item ledger** (Deviation 7) — this plan's Inherited-obligations table, copied whole, every "verify" resolved to what the landed tree showed, every discharged row naming its task/commit, every open row naming its durable home (backlog entry name, ticket id).
  - **The owed spec amendments, handed to the controller as a batch** (Deviation 3) — items A–I, each as *location → current sentence (quoted from the spec) → replacement wording*:
    - **A (§7, check-3 rationale):** replace "`<!DOCTYPE>` and comments produce no token in `scan_tags`, so a dropped doctype or comment is ledger-invisible" with "`<!DOCTYPE>` and comments leave no **ledger entry** — since DCR-0032 they produce `TagToken::Skip`, and `tag_inventory` filters `Skip` — so a dropped doctype or comment is ledger-invisible".
    - **B (§4, rule T):** amend the MEASURED clause to "**MEASURED (pre-wave-0 scanner):** `<!DOCTYPE html>` produced no token and no skip before DCR-0032's bogus-comment state; the sentence records the rationale for adding `Skip`, not the shipped scanner."
    - **C (§4, the img exception):** replace the exception sentence with the implemented reading: "**Exception:** a run that is textless after excluding absorbed phrasing markup and `Skip` spans — no naked byte outside those spans — and that contains one or more `img` elements becomes **one** `Image` block (`<a><img></a>`, `<picture><img></picture>`, and two adjacent `<img>` tags are each ONE block, never two and never gap), so images keep a sync anchor instead of dissolving into gap (DCR-0035; pinned by `a_linked_image_keeps_its_anchor_as_one_image_block`)."
    - **D (§12, wave 4):** append "*(Amended: 'parallel with 3' held for checks 1, 3, 4 and the finalize branch; check 2 consumes `intake::html::parse`, and in the executed ordering wave 4 followed wave 3 — DCR-0036.)*"
    - **E (§6, context projection):** replace "an HTML `<h1>` would put raw tags into the heading stacks and `context_hash`" with "comrak sees an Html-spelled heading as one leaf `HtmlBlock` with no inline children, so it projected to **empty prose** — collapsed heading stacks and degraded `context_hash` identity, not markup leakage (measured by wave 2's pin; DCR-0037). The extract-projection remedy is unchanged."
    - **F (§7, the walk sentence):** replace "`walk::*` takes no HTML arm either: it is the *Markdown* layer-6/renderer pairing and must simply never be called on an HTML document — enforced by the format branch in `finalize`, not by adding arms" with "the comrak-typed consumers of `walk` (`reparse_full`, `render_fragment` — the Markdown layer-6/renderer pairing) must never run on an HTML document, enforced by the format branch in `finalize`, not by adding arms; `walk::normalize_top_level` itself is comrak-free IR projection and serves §8 step 6's `li` grouping on HTML documents (DCR-0038)."
    - **G (§8, steps 3/4), both halves:** append "A void element records no extent in the §5 walk, so an img-run block — the lone `<img>` included — presents no outermost element and takes step 4's transparent wrapper: the instrument's verdict, not a special case (DCR-0038). And a block whose outermost element is **unknown to §4's tables** — a custom element, a future element — takes the transparent wrapper even when it presents a complete extent: the shipped shell mounts panes through DOMPurify's untouched fail-closed config, which removes an unknown element and every attribute riding it, so a self-injected anchor would die in the mount (measured against the vendored 3.2.6). The rule is keyed on §4's tables, never on the sanitizer's allowlist — duplicating that allowlist in Rust would be a second opinion about what the sanitizer accepts, the same class of sin as a second Markdown parser; any element §4 does not know fails safe into the wrapper (the 2026-08-21 ruling; DCR-0038)."
    - **H (§11):** the review-gate row's "these two additive fields as the *only* sanctioned alignment-output diff" becomes "the `schema_version` string plus these two additive fields — the three-part sanctioned alignment-output diff"; and the cheap pin's sentence is re-worded to §3's definition (`input_format == "markdown"`; every row's `source_format` equals `block_kind == "html" ? "html" : "markdown"`) (DCR-0038).
    - **I (§8, step 6 — mechanism erratum, the amendment-E class):** the step's ground sentence — "a bare `<li>` as a direct `<main>` child gets relocated by DOMPurify and the anchor moves with it" — is measured-false: DOMPurify 3.2.6 (the vendored `web/vendor/purify.min.js`) passes `<li data-sync-id="li-1">bare</li>` through **unchanged, in place**; relocation is a table-family parser behaviour (`<tr>` without `<table>`), not `<li>`'s. Replace the mechanism with the true ground: an ungrouped pane is structural drift the suite's per-pane count/equality assertions catch, and the grouping is mandated by D9's model — item anchors inside one reconstructed group — not by any sanitizer behaviour. The step's instruction (group consecutive `li` blocks under one shared `<ul>`/`<ol>`) does not change. (2026-08-21 review measurement; ledger row 27; this plan's probe P2 drives the ungrouped pane red on exactly the structural assertion.)
  - **§14.1, staged not settled** — the evidence paths, the three observed block-kind sequences, the ticket id, and the sentence that the ruling is the owner's.
  - **What did NOT move:** the engine (`sync.js` byte-untouched in both copies — `sync_js_drift` green at zero edits); every crate's `src/`; every fixture; every `Cargo.toml`; the spec file (the batch above is the controller's); `docs/index.md` (ditto); no exit code, no schema motion, no cache axis.
  - **Feature status and what closure does NOT claim:** all eight waves landed; the v0.5.0 window is still open and **cutting v0.5.0 is a separate owner decision through `docs/project/release-checklist.md`** — this record closes the feature, not the release. The ti `490d97` close is the controller's after acceptance (ledger row 26).

- [ ] **Step 2: CHANGELOG.** Under `[Unreleased]`, an `### Added` entry: the SCN-16 browser gate (`web/tests/scn16.spec.js` — name the file, state no count) covering bidirectional block-id sync over an HTML-run bundle, the title-as-chrome D5 behaviour, and a warnings-clean console; and a `### Changed`/docs entry: the browser-suite weld pins four spec files; scenario-matrix SCN-16; the serve fidelity door documented; README/CLAUDE.md/Quick_Start now name HTML documents as first-class input. Anchor placement on the landed `[Unreleased]` content (waves 0–6 all wrote here).

- [ ] **Step 3: `status.md`.** In the shape waves 0–6 used (re-read the landed bullets first): the `Closed change records:` line extends to `DCR-0039` with its parenthetical item (`, 0039 the 2026-08-21 browser gate + closure (ti `490d97` wave 7 — the feature's last wave)`); the ti `490d97` action bullet: wave 7 **LANDED**, DCR-0039, demonstrable outcome "the SCN-16 bundle syncs in a real browser through the shipped shell, and every record says what the feature cost" — **the HTML→HTML feature is COMPLETE: all eight waves (0–7) landed.** Name the acceptance evidence (suite green, probes red-then-restored, welds green); name what stays open (the §14.1 owner re-confirmation with its ticket id; the four backlog entries; the owed spec amendments and the two index lines, all in the controller's hand-off); and state plainly that the v0.5.0 cut is a separate owner decision via the release checklist.

- [ ] **Step 4: `phase-state.yaml`.** Three edits in the wave 0 pattern (no per-wave keys exist — verify against the landed file): `project.last_updated:` → `<execution-date>-ti490d97-wave7`; `design.closed_change_records:` appends `- DCR-0039-html-browser-gate-and-closure` after the DCR-0038 slug; `project.notes:` gains a paragraph at the block's indent, plain prose, no backticks: `TI 490d97 WAVE 7 LANDED <date> (DCR-0039): the browser gate — scn16.spec.js drives the HTML-run bundle through the shipped shell (bidirectional block-id sync, title absent from panes, zero console warnings), the docs-drift weld pins four spec files, and the closure ledger dispositions every deferred item. THE HTML->HTML FEATURE (ti 490d97) IS COMPLETE - ALL EIGHT WAVES LANDED. Still open, with homes: the owner's 14.1 custom-element re-confirmation (ticket + backlog), island reclassification (schema-2.x window), the oversize-leaf splitter, the parser rename, and the spec-amendment batch handed to the controller. The v0.5.0 cut is a separate owner decision via the release checklist.`

- [ ] **Step 5: Capture the ticket state for the hand-off.**
```bash
ti show 490d97 > /Volumes/Temp/claude/ti490d97-wave7/gate/ti-490d97-state.txt 2>&1
```
Read it. Do **not** close the ticket unless this agent verifiably holds the claim (`ti-finish`'s own check); the default is the hand-off note in the commit message below.

- [ ] **Step 6: Verify every docs-drift weld, then commit.**
```bash
cargo test --workspace -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t8-workspace.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t8-workspace.txt
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4 > /Volumes/Temp/claude/ti490d97-wave7/gate/t8-cli.txt 2>&1
echo "CARGO_EXIT=$?" >> /Volumes/Temp/claude/ti490d97-wave7/gate/t8-cli.txt
```
Expected: `t8-cli.txt` `CARGO_EXIT=0`; `t8-workspace.txt` `CARGO_EXIT=101` with `docs_index_drift` the **only** failure, naming only DCR-0039's file (and this plan's file if its link is still pending) — the standing allowance; any other red is a STOP. After the controller confirms the links, re-run into `t8-index-after.txt` and expect `CARGO_EXIT=0`.
```bash
git add docs/project/design-change-records/DCR-0039-html-browser-gate-and-closure.md \
  CHANGELOG.md docs/project/status.md docs/project/phase-state.yaml
git commit -m "docs: DCR-0039 — the feature closes, and the ledger says what stayed open

The last wave's record: the shell-driven browser gate with its
falsification evidence, the four-file suite weld, the closure sweeps,
and the deferred-item ledger — every hand-forward from DCR-0032..0038
dispositioned, the owed spec amendments batched for the controller with
exact replacement wording, and the still-open items in durable homes.
docs/index.md is the controller's; two lines ride this message:

- [DCR-0039 — the browser gate closes the HTML→HTML feature, and the ledger says what stayed open (<date>)](project/design-change-records/DCR-0039-html-browser-gate-and-closure.md) — ti 490d97 wave 7: web/tests/scn16.spec.js (bidirectional sync, title-as-chrome, zero warnings), the four-file suite weld, the closure sweep, and the deferred-item ledger.

And the spec's own index line — already corrected once by the
controller to 'Implementation is authorized and under way … wave 0 …
is landing on master now' — is stale again now that all eight waves
are landed. Replacement annotation for the existing link: 'owner-
ratified design spec for the second intake …; ti 490d97. Implemented
2026-08-20/21 (DCR-0032..0039, ADR-0025); the v0.5.0 window it names is
open, the release cut pending.'

The ti 490d97 close is the controller's, after acceptance.

TRACE: ti 490d97 wave 7 (spec 2026-08-20 §10, §12)

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>"
```

---

## Wave acceptance — check all seven before declaring wave 7, and the feature, done

1. **Playwright SCN-16 green inside the real runner:** `gate/t2-suite.txt` shows `SUITE_EXIT=0` with the three scn16 tests listed as passed and `[test-browser] OK` closing the run — bidirectional sync by block id, title absent from panes, zero console warnings, exactly §12's line.
2. **The gate can fail:** `gate/probe1.txt`, `probe2.txt`, `probe3.txt`, `probe4.txt` each show `PW_EXIT` non-zero with the probe's named assertion in the failure output; the probe fixture was deleted after.
3. **Every docs-drift weld green:** `gate/t8-workspace.txt` + `gate/t8-cli.txt` show the full suites green under the standing `docs_index_drift` carve-out — which covers `docs_browser_suite_drift` (now four files), `docs_gate_claims_drift`, `docs_ownership_drift`, `reader_honesty`, `public_surface`, `workspace_publication`, `docs_cli_flags_drift`, `exit_code_docs_drift`, `sync_js_drift` — the last three at **zero edits** to their test files. What `sync_js_drift` proves is that the two shipped `sync.js` copies still match each other and mirror `ALIGNMENT_SCHEMA_VERSION`; that the engine moved zero bytes this wave is acceptance 5's `git diff`, not this weld's claim.
4. **The suite grew without a count appearing anywhere:** `gate/t3-green.txt`, `gate/t4-weld.txt` and `gate/t5-weld.txt` show `docs_browser_suite_drift` green around each commit that touches a GUARDED document (Tasks 3, 4 and 5; Tasks 6–7 touch no GUARDED file, and Task 8's workspace run re-covers the suite).
5. **The code surface is exactly one weld test:** `git diff <baseline-commit>..HEAD --stat -- crates/ web/js/` names only `crates/transync/tests/docs_browser_suite_drift.rs`; `git diff <baseline-commit>..HEAD --stat -- 'crates/*/tests/fixtures/*' web/js web/tests/support` is empty. Capture to `gate/acceptance-surface.txt`.
6. **The ledger is total:** for each of DCR-0032…0038, grep its hand-forward/owed section (`grep -n -A6 -i 'handed forward\|owed' docs/project/design-change-records/DCR-003[2-8]-*.md > /Volumes/Temp/claude/ti490d97-wave7/gate/acceptance-ledger.txt`) and check every named item appears in DCR-0039's ledger with a disposition. A hand-forward absent from the ledger is the closure failing at its one job — fix before declaring done.
7. **The controller hand-off is complete and explicit:** DCR-0039's index line, the spec-annotation replacement, and the A–I amendment batch all present in the DCR and the Task 8 commit message; status.md names them as pending controller action.

---

## Spec §12-wave-7 coverage map

| Spec item | Where in this plan |
|---|---|
| `web/tests/scn16.spec.js` wired into `scripts/test-browser.sh` | Task 2 (spec + comment-truth; Deviation 1's wiring argument); Task 3 (the weld) |
| Acceptance: bidirectional sync by block id | Task 2 test `c` (forward + reverse, map-derived ids, reachability guards) |
| Acceptance: title absent from panes | Task 2 test `b` (negative on the positive backbone; the chrome-side positive `page.title()`); probes P1/P3 |
| Acceptance: zero console warnings | Task 2 `engineNoise` in tests `b`/`c` (Deviation 4's scoping); probes P1/P3 |
| Acceptance: every docs-drift weld green | Wave acceptance 3–4 |
| Scenario-matrix rows | Task 4 |
| Serve-door documentation | Task 5 Steps 1–2 |
| Backlog / open-issue cross-references | Task 7 |
| CHANGELOG / status / phase-state | Task 8 Steps 2–4 |
| The remaining contracts sections | Task 6 |
| The docs-index link for this spec | Ledger row 7 — already satisfied, verified in Task 1 Step 3; stale annotation handed to the controller (Task 8) |
| §13 accepted limitations restated, not re-litigated | Task 4 Step 4 (out-of-scope), Task 7 backlog notes, DCR-0039 |
| §14 / §15 items dispositioned | The ledger; Task 7 Steps 3–5; Task 8 |

## Notes for the implementer

- **Quoted upstream shapes are from the wave 0–6 *plans*, not from a landed tree** — those waves were mid-landing when this plan was written. Task 1's STOP gate is the contract; wherever a landed sentence, table shape, or comment differs from a quote here, the landed tree wins: keep the obligation, adjust the mechanical anchor, and record the delta in DCR-0039's verification note.
- **A red anywhere in Task 2's Step 3 first run is a finding about waves 0–6, not about the spec.** The spec is only wrong if a falsification probe disagrees with its stated expectation. Diagnose, STOP, and hand back a code defect — this wave owns no `src/` file.
- **Do not edit weld tests to make them pass** — the one sanctioned weld edit is Task 3's growth of `docs_browser_suite_drift.rs`, which is that weld's designed mechanism (ticket `729ec8` is the precedent), moving in one commit with the three documents.
- **The two vacuity traps this wave was explicitly warned about:** a negative title assertion with a wrong selector (closed by the anchor-list equality + `page.title()` positive), and a zero-warnings assertion over a capture that never listened (closed by registering the typed collector **before** `page.goto`, and by probes P1/P3 proving the channel fires).
- **Resist the closing-wave itch to "tidy"**: no formatting sweeps, no rewording of true sentences, no spec edits, no `docs/index.md` edits, no engine edits. The closure's product is records that match the tree — nothing else moves.
