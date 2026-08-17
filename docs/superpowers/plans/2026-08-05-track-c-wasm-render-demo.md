# Track C — WASM Render + Edit Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A `web/` demo page that renders both panes locally through a new `transync-wasm` crate (same Rust renderer, byte-parity with the CLI) and supports per-block editing of the translated Markdown with live re-render/re-sync — CLI bundle and shipped shells untouched.

**Architecture:** Approved spec: `docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md` (each task cites its sections). Exploration digests with measured facts and file:line maps: `/Volumes/Temp/claude/track-c-context/{wasmSurface,webSeams,toolchain}.md` — implementers SHOULD read the digest their task names. Order: workspace manifest prep (T1) → crate + logic + gates (T2) → build script (T3) → demo page + fixture wiring (T4) → Playwright suite (T5) → records + final gates (T6).

**Tech Stack:** Rust (wasm32-unknown-unknown, wasm-bindgen =0.2.126, wasm-pack 0.15 --target web), brew binaryen ≥121, framework-free ESM, Playwright (existing web/ project).

## Global Constraints

- FROZEN: the CLI bundle contract (`HTML_BUNDLE_ENTRIES` six files, foreign-file allowlists), `web/index.html`, `crates/transync-cli/web/index.html.tpl`, `web/js/sync.js` (both copies), alignment schema `1.2.0`, the facade §0 surface (`public_surface.rs` green unmodified), and `crates/transync-syntax/Cargo.toml` (no `[features]`, no core dep — its SOURCE is also untouched this wave).
- NEVER set/override `CARGO_TARGET_DIR`, pass `--target-dir`, or add a repo-local `.cargo/config.toml` with `[build]` keys (wasm-pack respects the machine-global target dir — verified).
- `cargo test --workspace -- --test-threads=4`; never raise the cap. Temp files under `/Volumes/Temp/claude/` only.
- `wasm-bindgen = "=0.2.126"` exact pin (owner decision: OI-0020 stays open; the pin is the defense). `Cargo.lock` stays uncommitted.
- New host prerequisite: brew `binaryen` (wasm-opt ≥121) — scripts that need it FAIL LOUDLY when absent (install hint), never skip silently. The pre-commit hook stays rustup-only.
- DOMPurify fail-closed mount is mandatory for every `innerHTML` write of rendered content (invariant 7); never `insertAdjacentHTML`/`createContextualFragment`.
- ESM, framework-free JS; file-as-module Rust, never `mod.rs`; no pure-formatting edits; `cargo fmt` before each commit.
- Each task ends: fmt → clippy `-D warnings` → `cargo test --workspace -- --test-threads=4` → commit (hook runs gates; never `--no-verify`).
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: Workspace manifest prep

Spec §3 (root profile + comrak features) + owner decision 3 (bindgen unification). Digest: `toolchain.md` §R2/R3/R6.

**Files:**
- Modify: `Cargo.toml` (root: `[workspace.dependencies]` comrak line; new `[profile.wasm-release]`)

**Interfaces:**
- Produces: `[profile.wasm-release]` (Task 3 builds with `--profile wasm-release`); comrak with `default-features = false`; the dependency tree resolved with wasm-bindgen 0.2.126 (Task 2 pins it in the new crate).

- [ ] **Step 1: comrak default features off**

In root `Cargo.toml` `[workspace.dependencies]`: `comrak = "0.27"` → `comrak = { version = "0.27", default-features = false }`. (The `cli`/`syntect` default features are used nowhere — digest R6 grep; this is manifest-only, −7.8 % gzip on the wasm module.)

- [ ] **Step 2: Add the wasm profile**

Append to root `Cargo.toml`:

```toml
# Track C (ADR-0019): profile for the wasm demo module only. Profiles must
# live at the workspace root (cargo ignores non-root profiles); a custom
# profile keeps transync-cli's stock `release` build untouched.
[profile.wasm-release]
inherits = "release"
opt-level = "z"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

- [ ] **Step 3: Unify wasm-bindgen resolution**

Run: `cargo update -p wasm-bindgen` — the lock (uncommitted) moves the slug→comrak transitive wasm-bindgen from 0.2.120 to 0.2.126. Verify: `cargo tree -p transync-syntax --target wasm32-unknown-unknown | grep wasm-bindgen` shows `v0.2.126`.

- [ ] **Step 4: Prove behavior preservation**

Run: `cargo test --workspace -- --test-threads=4` (expect the current baseline, zero failures — comrak's dropped features are unused) and `cargo check -p transync-syntax --target wasm32-unknown-unknown`. If ANY test changes behavior, STOP and report — the feature change would not be manifest-only.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml
git commit -m "build: comrak default-features off + wasm-release profile + bindgen 0.2.126 resolution (Track C prep)"
```

---

### Task 2: `transync-wasm` crate — logic layer, bindgen surface, gate lines

Spec §1–§2 + §6 (the crate's own gate lines land with it). Digest: `wasmSurface.md` §1 (the exact call sequences for both modes), `toolchain.md` §2/R5/R9. TDD for the logic layer.

**Files:**
- Create: `crates/transync-wasm/Cargo.toml`, `crates/transync-wasm/src/lib.rs`, `crates/transync-wasm/src/engine.rs`
- Modify: root `Cargo.toml` (workspace members), `scripts/hooks/pre-commit` (wasm check line), `scripts/smoke.sh` (wasm check line + rustdoc package list)

**Interfaces:**
- Produces (Task 3 builds this crate; Tasks 4–5 consume the JS surface):
  - `#[wasm_bindgen] pub fn render_pair(source_md: &str, translated_md: &str, alignment_json: &str) -> Result<String, JsError>` — returns `{"source_html":…,"target_html":…}` JSON
  - `#[wasm_bindgen] pub fn rebuild(source_md: &str, payloads_json: &str, statuses_json: &str, source_lang: &str, target_lang: &str, detected: Option<String>) -> Result<String, JsError>` — returns `{"source_html":…,"target_html":…,"alignment_json":…,"translated_md":…}` JSON (alignment_json is a nested JSON string)
  - `#[wasm_bindgen] pub fn schema_version() -> String` — `ALIGNMENT_SCHEMA_VERSION`
  - Logic twins in `engine.rs` (host-testable, no bindgen types): `render_pair_impl(...) -> Result<PairOutput, EngineError>`, `rebuild_impl(...) -> Result<RebuildOutput, EngineError>` where the outputs are plain structs with `Serialize`.

- [ ] **Step 1: Crate scaffold**

`crates/transync-wasm/Cargo.toml`:

```toml
[package]
name = "transync-wasm"
description = "Browser (wasm-bindgen) surface over transync-syntax: local pane rendering and the demo edit loop (Track C, ADR-0019)."
publish = false
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
transync-syntax = { path = "../transync-syntax" }
serde = { workspace = true }
serde_json = { workspace = true }
# Exact pin: wasm-bindgen requires crate<->CLI version identity, and the
# workspace lockfile is uncommitted (OI-0020, deferred by owner decision
# 2026-08-05) — the pin is the defense against clean-checkout skew.
wasm-bindgen = "=0.2.126"
```

Add `"crates/transync-wasm"` to the root workspace members list. Match the other members' style for dep lines (`{ workspace = true }` where the root declares them; add `wasm-bindgen` to `[workspace.dependencies]` as `wasm-bindgen = "=0.2.126"` and consume `{ workspace = true }` if that matches convention better — follow the root file's existing pattern).

- [ ] **Step 2: Write the failing host tests** (in `engine.rs`'s `#[cfg(test)]`):

```rust
const FIXTURE_MD: &str = "# Title\n\nfirst para\n\n- a\n- b\n";

#[test]
fn render_pair_round_trips_a_translated_fixture() {
    // Build the ground truth the way the pipeline does: parse + ids, then a
    // rebuild-mode run gives us a real alignment map + translated md to feed
    // the view-mode call.
    let payloads = serde_json::json!({}); // no translations: pure fallback pass
    let statuses = serde_json::json!({});
    let built = rebuild_impl(FIXTURE_MD, &payloads.to_string(), &statuses.to_string(), "en", "ko", None)
        .expect("rebuild succeeds");
    assert!(built.source_html.starts_with("<main>"), "source pane is a <main> fragment");
    assert!(built.target_html.starts_with("<main>"), "target pane is a <main> fragment");
    assert_eq!(built.translated_md, FIXTURE_MD, "no translations => regen is byte-identical source");
    let map: serde_json::Value = serde_json::from_str(&built.alignment_json).expect("map parses");
    assert_eq!(map["schema_version"], "1.2.0");

    let pair = render_pair_impl(FIXTURE_MD, &built.translated_md, &built.alignment_json)
        .expect("render_pair succeeds");
    assert_eq!(pair.source_html, built.source_html, "view mode reproduces rebuild's source pane");
    assert_eq!(pair.target_html, built.target_html, "view mode reproduces rebuild's target pane");
}

#[test]
fn schema_version_is_the_wire_constant() {
    assert_eq!(schema_version_impl(), transync_syntax::align::ALIGNMENT_SCHEMA_VERSION);
}

#[test]
fn bad_alignment_json_is_an_error_not_a_panic() {
    let err = render_pair_impl(FIXTURE_MD, FIXTURE_MD, "{not json").unwrap_err();
    assert!(err.to_string().contains("alignment"), "error names the failing input");
}

#[test]
fn unknown_block_id_in_payloads_is_an_error() {
    let payloads = serde_json::json!({"zz-9999": "ghost"});
    let err = rebuild_impl(FIXTURE_MD, &payloads.to_string(), "{}", "en", "ko", None).unwrap_err();
    assert!(err.to_string().contains("zz-9999"));
}
```

(Adjust assertion details to the real APIs — e.g. if `regenerate` requires payload ids to exist, the unknown-id check may live in `rebuild_impl`'s own validation, which the test then pins. Intent binding: errors are `Err`, never panics; the two modes agree byte-for-byte on the same inputs.)

- [ ] **Step 3: Run to verify failure** — `cargo test -p transync-wasm -- --test-threads=4` fails (module absent).

- [ ] **Step 4: Implement `engine.rs`** per the digest's exact call sequences:
- `render_pair_impl`: `parser::parse` → `id::assign_block_ids` → `serde_json::from_str::<AlignmentMap>` → `render::render_source` + `render::render_target`.
- `rebuild_impl`: parse → assign ids → validate every payload/status id exists in the doc (unknown id ⇒ `Err`) → `outcome::html_outcomes` → `regen::regenerate(&doc, &payloads_map)` → `align::build_alignment_map(&doc, &statuses_map, &offsets, source_lang, target_lang, detected, &outcomes)` → render both panes; serialize the map back to JSON for the output struct.
- `EngineError`: a small enum (thiserror not required — a plain `Display` impl or thiserror via workspace dep, follow the syntax crate's precedent) naming which input failed.
- NOTE: `transync-syntax`'s modules are reachable as `transync_syntax::{parser, id, outcome, regen, align, render}` — all pub in that crate. `statuses_json` values deserialize via the wire `FallbackStatus` serde (snake_case).

- [ ] **Step 5: Run to green**, then add the thin bindgen layer in `lib.rs`: the three `#[wasm_bindgen]` fns delegating to `engine::*_impl`, mapping `EngineError` → `JsError::new(&msg)`, plus module doc naming ADR-0019. Host suite green again (bindgen exports inert on host).

- [ ] **Step 6: Gate lines** — in BOTH `scripts/hooks/pre-commit` and `scripts/smoke.sh`, the wasm check becomes `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown` (edit each copy; they are independent). `scripts/smoke.sh`'s rustdoc line gains `-p transync-wasm`. Run the new check line once directly to prove the crate compiles for wasm32.

- [ ] **Step 7: Full gates + commit**

```bash
git add Cargo.toml crates/transync-wasm/ scripts/hooks/pre-commit scripts/smoke.sh
git commit -m "feat: transync-wasm crate — render_pair/rebuild/schema_version over transync-syntax (Track C)"
```

---

### Task 3: `scripts/build-wasm.sh` + size budget

Spec §3. Digest: `toolchain.md` §R1 (the exact wasm-opt flag list, verified), §4/§5.

**Files:**
- Create: `scripts/build-wasm.sh` (executable)
- Modify: `.gitignore` (add `web/wasm/`), `scripts/smoke.sh` (run build-wasm.sh)

**Interfaces:**
- Produces: `web/wasm/transync_wasm.js` + `web/wasm/transync_wasm_bg.wasm` (gitignored build output) — Tasks 4–5 load these paths.

- [ ] **Step 1: Install the host prerequisite** — `brew install binaryen` (formula ≥131; owner-approved new prerequisite). Verify `wasm-opt --version` prints ≥ 121.

- [ ] **Step 2: Write the script** (match `smoke.sh`'s REPO_ROOT + guarded-temp conventions):

```bash
#!/usr/bin/env bash
# transync — build the Track C wasm demo module (ADR-0019).
#
# Prereqs (fail loudly, never skip): wasm-pack, brew binaryen's wasm-opt
# (>=121 — the wasm-pack-cached binaryen 117 rejects rustc 1.97 output).
# Output: web/wasm/ (gitignored). Never sets CARGO_TARGET_DIR; wasm-pack
# respects the machine-global target dir.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

command -v wasm-pack >/dev/null || { echo "[build-wasm] wasm-pack missing (brew install wasm-pack)" >&2; exit 1; }
command -v wasm-opt  >/dev/null || { echo "[build-wasm] wasm-opt missing (brew install binaryen)" >&2; exit 1; }
OPT_VER="$(wasm-opt --version | grep -oE '[0-9]+' | head -1)"
[ "$OPT_VER" -ge 121 ] || { echo "[build-wasm] wasm-opt $OPT_VER too old (need >=121; brew upgrade binaryen)" >&2; exit 1; }

STAGING="${TRANSYNC_WASM_STAGING:-/Volumes/Temp/claude/transync-wasm-pkg}"
rm -rf "$STAGING"
echo "[build-wasm] wasm-pack build --target web --profile wasm-release --no-opt"
wasm-pack build crates/transync-wasm --target web --profile wasm-release --no-opt --no-pack --out-dir "$STAGING"

echo "[build-wasm] wasm-opt -Oz (explicit, binaryen $OPT_VER)"
wasm-opt -Oz \
  --enable-bulk-memory --enable-reference-types \
  --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  "$STAGING/transync_wasm_bg.wasm" -o "$STAGING/transync_wasm_bg.opt.wasm"
mv "$STAGING/transync_wasm_bg.opt.wasm" "$STAGING/transync_wasm_bg.wasm"

mkdir -p web/wasm
cp "$STAGING/transync_wasm.js" "$STAGING/transync_wasm_bg.wasm" web/wasm/

RAW=$(wc -c < web/wasm/transync_wasm_bg.wasm)
GZ=$(gzip -9 -c web/wasm/transync_wasm_bg.wasm | wc -c)
echo "[build-wasm] size: raw=${RAW}B gzip=${GZ}B (budget: 1400000 / 500000)"
[ "$RAW" -le 1400000 ] || { echo "[build-wasm] RAW SIZE OVER BUDGET" >&2; exit 1; }
[ "$GZ"  -le  500000 ] || { echo "[build-wasm] GZIP SIZE OVER BUDGET" >&2; exit 1; }
echo "[build-wasm] OK -> web/wasm/"
```

(`--no-pack` skips package.json emission; adjust the staging filenames to what wasm-pack actually emits for this crate name — verify, don't assume.)

[**Correction (2026-08-05, owner-approved):** the `1400000 / 500000` in the skeleton above is superseded. Those ceilings were calibrated on an exploration probe that compiled **view mode only**; the shipped crate also compiles edit mode, where `regen::regenerate` + `outcome::html_outcomes` add a measured **+435,770 B**. As-built budget: **raw ≤ 1,950,000 B, gzip ≤ 810,000 B** against a measured **1,739,791 B / 718,136 B**. Attribution chain in the Task 3 report; the +113,844 B of fat-LTO scope lost to the `rlib` crate-type is ticketed `92ac61b9`. Emitted filenames were verified as `transync_wasm.js` / `transync_wasm_bg.wasm`, as the skeleton assumed.]

- [ ] **Step 3: `.gitignore`** — add `web/wasm/` under the reserved build-artifacts comment block.

- [ ] **Step 4: Run it end-to-end** — `scripts/build-wasm.sh` succeeds; sizes within budget (digest baseline: ~1.18 MiB / ~431 KiB). Record the printed sizes in your report. [**Corrected 2026-08-05:** the digest baseline was view-only; as-built is 1,739,791 B / 718,136 B against the corrected 1,950,000 / 810,000 budget — see the correction under Step 2.]

- [ ] **Step 5: Wire into smoke** — `scripts/smoke.sh` gains an announced `scripts/build-wasm.sh` step (after the cargo gates, before the CLI e2e), inheriting its loud-fail prereq behavior.

- [ ] **Step 6: Full gates + commit**

```bash
git add scripts/build-wasm.sh scripts/smoke.sh .gitignore
git commit -m "build: build-wasm.sh — wasm-pack + explicit binaryen wasm-opt + size budget (Track C)"
```

---

### Task 4: Demo page + fixture wiring

Spec §4 + §5 (fixture half). Digest: `webSeams.md` §3/§4/§5 (mount sequence, sync.js exports, re-mount contract).

**Files:**
- Create: `web/demo-wasm.html`, `web/js/wasm-demo.js`
- Modify: `web/tests/support/static-server.mjs` (`.wasm` MIME), `scripts/test-browser.sh` (wasm leg: build + fixture copies)

**Interfaces:**
- Consumes: Task 2's JS surface (`init` default export, `render_pair`, `rebuild`, `schema_version`), Task 3's `web/wasm/` artifacts, `sync.js`'s `{ mountSync, safeStorage, fetchOk }`.
- Produces: the served demo at `/demo-wasm.html` inside the Playwright fixture dir; fixture files `source.md` + `out.md` alongside the existing bundle files. Task 5 tests against exactly this.

- [ ] **Step 1: static-server MIME** — add `".wasm": "application/wasm"` to the `TYPES` map in `web/tests/support/static-server.mjs` (prevents the glue's console-warning fallback).

- [ ] **Step 2: `web/js/wasm-demo.js`** — ESM, framework-free, structured as:

```js
import init, { render_pair, rebuild, schema_version } from "./wasm/transync_wasm.js";
import { mountSync, fetchOk } from "./sync.js";

const KNOWN_SCHEMA = "1.2.0"; // demo's mirror; init() refuses on mismatch (fail-closed)

async function boot() {
  await init();
  if (schema_version() !== KNOWN_SCHEMA) { showFatal(`schema mismatch: wasm ${schema_version()} vs demo ${KNOWN_SCHEMA}`); return; }
  if (!window.DOMPurify) { showFatal("DOMPurify missing — refusing to mount (fail-closed)"); return; }
  const [sourceMd, outMd, alignmentJson] = await Promise.all([
    fetchOk("source.md", r => r.text()),
    fetchOk("out.md", r => r.text()),
    fetchOk("alignment.json", r => r.text()),
  ]);
  const model = buildEditModel(outMd, JSON.parse(alignmentJson)); // {payloads: {id: md}, statuses: {id: status}, editable: Set<id>}
  const pair = JSON.parse(render_pair(sourceMd, outMd, alignmentJson));
  mountPanes(pair.source_html, pair.target_html, JSON.parse(alignmentJson));
  wireEditing(sourceMd, model);
}
```

Required behaviors (each is a checkable requirement, not a suggestion):
- `boot` ALSO validates the fetched map's own `schema_version` against `KNOWN_SCHEMA` (mirroring `sync.js`'s `loadAlignment` gate) — fatal panel on major mismatch. This is the JS-testable half of the fail-closed pair (Task 5 test 4 mutates it); the wasm-side `schema_version()` check above is the build-time half.
- `buildEditModel`: for each alignment row, slice `outMd` by `target_range` (valid pre-edit) into `payloads[source_block_id]`; `statuses[source_block_id] = fallback_status`; `editable` = rows whose `block_kind` is not `"html"` and not `"skipped"` and whose `sync_role` isn't `"non-sync"`.
- `mountPanes(srcHtml, tgtHtml, map)`: `pane.innerHTML = DOMPurify.sanitize(html)` for both, then `mountSync(sourceEl, targetEl, map)` (idempotent re-mount is the engine's documented contract).
- `wireEditing`: click on a target-pane `[data-sync-id]` element whose id is in `editable` opens a textarea (below the panes) holding `payloads[id]`; input events debounce 300 ms → `rebuild(sourceMd, JSON.stringify(payloads), JSON.stringify(statuses), srcLang, tgtLang, detected)` (langs/detected read from the fetched alignment map) → parse result → record both panes' `scrollTop` → `mountPanes(...)` with the FRESH alignment JSON → restore `scrollTop`s → re-apply `<details open>` state captured before the swap (match by `data-sync-id`).
- `rebuild` exceptions render into a non-fatal error strip (the previous panes stay); `showFatal` replaces the source pane's content with the message (mirroring the shells' fail-closed style).

- [ ] **Step 3: `web/demo-wasm.html`** — clone `web/index.html`'s structural skeleton (two `.pane` divs with `position: relative`, the classic DOMPurify script tag `vendor/purify.min.js` BEFORE the module script, the same base CSS) but titled "transync — WASM render demo", loading `./js/wasm-demo.js`, plus an edit panel (`<textarea id="editor">` + a status strip). Keep it self-contained; do not modify `web/index.html`.

- [ ] **Step 4: `scripts/test-browser.sh` wasm leg** — after the existing six-file assertion loop: run `scripts/build-wasm.sh`; copy into `$HTML_OUT`: `web/wasm/transync_wasm.js`, `web/wasm/transync_wasm_bg.wasm`, `web/demo-wasm.html` (as `demo-wasm.html`), `web/js/wasm-demo.js` (as `js/wasm-demo.js` — mind the import paths: the demo page imports `./js/wasm-demo.js`, which imports `./wasm/transync_wasm.js` relative to ITSELF, i.e. `js/wasm/...` — RESOLVE THIS: either place the wasm artifacts at `$HTML_OUT/js/wasm/` or make wasm-demo.js import `../wasm/transync_wasm.js`; pick one, make the repo layout (`web/js/`, `web/wasm/`) and the fixture layout agree, and document the choice in the script comment), the fixture source md (`crates/transync/tests/fixtures/scn-14-full.md` → `source.md`), and `$WORKDIR`'s `out.md`. Extend the file-assertion loop with the new names. `sync.js` and `vendor/purify.min.js` need fixture-relative copies too (`js/sync.js`, `vendor/purify.min.js`) since the demo page uses the WORKSPACE paths, not the bundle-flat paths — copy from `web/`.

- [ ] **Step 5: Verify by serving** — run the fixture assembly, then `node web/tests/support/static-server.mjs "$HTML_OUT" 4321 &` and curl: `demo-wasm.html` 200, `js/wasm-demo.js` 200, the wasm 200 with `content-type: application/wasm`. Kill the server. Existing SCN-13 8/8 still green via `scripts/test-browser.sh`.

- [ ] **Step 6: Full gates + commit**

```bash
git add web/demo-wasm.html web/js/wasm-demo.js web/tests/support/static-server.mjs scripts/test-browser.sh
git commit -m "feat: wasm render+edit demo page and fixture wiring (Track C)"
```

---

### Task 5: Playwright `wasm.spec.js`

Spec §7 (browser half). Digest: `webSeams.md` §6 (config, harness idioms, console collection).

**Files:**
- Create: `web/tests/wasm.spec.js`

**Interfaces:**
- Consumes: Task 4's served demo + fixture layout; `harness.js` helpers where applicable.

- [ ] **Step 1: Write the four tests** (same project/config; follow `scn13.spec.js`'s idioms — console collection, `page.goto("/demo-wasm.html")`):

1. **loads clean**: goto demo page; wait for both panes to contain `[data-sync-id]` elements; assert ZERO console warnings/errors (this catches the MIME fallback and any bindgen warning).
2. **parity**: fetch `/source.html` and `/target.html` (the CLI-rendered bundle files, same fixture) via `page.request`; extract each `<main>…</main>` fragment; compare against the demo's rendered pane `innerHTML`... innerHTML normalization differs from source text — instead compare via DOM: for each `[data-sync-id]` in the CLI-parsed reference (use `page.evaluate` with `DOMParser` on the fetched text), assert the demo pane has the same id sequence, same `data-block-kind`s, and same `textContent` per block. (Full byte-parity of `innerHTML` is unreliable post-sanitize — DOMPurify may normalize; the parity claim is anchored on structure + text. Note in the test comment that byte-parity pre-sanitize is host-proven by transync-wasm's tests.)
3. **edit loop**: click the first editable paragraph in the target pane; type a replacement into the editor; wait past the debounce; assert the target pane shows the new text, the `data-sync-id` set is unchanged, and scrolling the source pane still drives the target (reuse scn13's scroll assertion helper pattern).
4. **fail-closed**: `page.route` the wasm glue request... simpler: `page.addInitScript` to predefine a broken `DOMPurify = undefined`? DOMPurify loads via script tag — instead route `**/vendor/purify.min.js` to an empty body → the demo must show the fatal panel and mount NOTHING (no `[data-sync-id]` in panes). Second variant: route `**/alignment.json` to a `schema_version: "9.9.9"` body → fatal schema panel. (The wasm-side schema constant can't be forged from JS, so the schema-mismatch test mutates the DEMO's comparison input — the fetched map — which flows into KNOWN_SCHEMA comparison? NOTE: the spec's fail-closed is wasm `schema_version()` vs the demo's KNOWN_SCHEMA constant; that pair can't be diverged in a test without rebuilding the wasm. The TESTABLE fail-closed paths are the DOMPurify-missing one and, if the demo also validates the fetched map's schema_version against KNOWN_SCHEMA (add that check in Task 4 if missed — it mirrors sync.js's own loadAlignment gate), the mutated-map one. Pin those two.)

- [ ] **Step 2: Run** — `scripts/test-browser.sh` green: 8 SCN-13 + 4 wasm = 12/12.

- [ ] **Step 3: Commit**

```bash
git add web/tests/wasm.spec.js
git commit -m "test: Playwright wasm demo suite — load-clean, parity, edit loop, fail-closed (Track C)"
```

---

### Task 6: Records + final gates

Spec §8.

**Files:**
- Create: `docs/decisions/0019-wasm-demo-layer.md` (ADR-0019), `docs/project/design-change-records/DCR-0020-track-c-wasm-demo.md`
- Modify: `docs/project/open-issues.md` (OI-0028 dated note), `docs/architecture/mvp-scope.md` (Track C line), `docs/project/status.md`, `docs/project/phase-state.yaml`, `CHANGELOG.md` (`[Unreleased]`), the Developer Guide's smoke-script table (binaryen prerequisite), `docs/implementation/module-map.md` (new crate + scripts)

**Interfaces:** consumes everything; cite actual commit hashes and the measured sizes from Task 3's report.

- [ ] **Step 1: ADR-0019** — the decision record: crate shape (syntax-only dependency, double rationale incl. getrandom), `=0.2.126` pin + OI-0020 deferral, binaryen host prerequisite + why explicit wasm-opt (the binaryen-117 trap, reproduced), the size budget and measured numbers, and the web-only decision (why the CLI bundle deliberately carries no wasm — the 30× payload argument and the redundancy argument). House ADR format (Context/Decision/Consequences), no line-number references.
- [ ] **Step 2: DCR-0020** — implementing record: files, gate-line changes, the boundary contract, the edit-model design (per-block payloads, statuses preserved, html read-only), parity evidence (host byte-parity tests + Playwright structural parity), fixture layout choice from Task 4 Step 4.
- [ ] **Step 3: Living docs** — OI-0028 dated "Track C proper landed" note; mvp-scope Track C line updated; status.md/phase-state.yaml latest-wave entries; CHANGELOG `[Unreleased]` (Added: wasm demo; Internal: profile/comrak-features/gate lines); Developer Guide smoke table gains binaryen; module-map gains `crates/transync-wasm` + `scripts/build-wasm.sh` + the demo files.
- [ ] **Step 4: Final full gates** — fmt/clippy/workspace suite (report summed count)/wasm check (both crates)/`scripts/build-wasm.sh` (sizes in budget)/`scripts/smoke.sh`/`scripts/test-browser.sh` (12/12)/rustdoc incl. transync-wasm/`public_surface.rs` green unmodified/resp-translator `cargo check` green.
- [ ] **Step 5: Commit**

```bash
git add docs/ CHANGELOG.md
git commit -m "docs: ADR-0019 + DCR-0020 — Track C wasm render+edit demo landed"
```

---

## Baseline numbers (for reviewers)

- Suite baseline at plan time: 385 passed / 0 failed / 3 ignored; Playwright 8/8. This wave adds host tests (T2 ≥4) and Playwright tests (T5 +4 → 12/12); zero removals.
- Size budget: raw ≤ 1,400,000 B, gzip ≤ 500,000 B (measured probe baseline 1,237,349 / 441,745). [**Corrected 2026-08-05, owner-approved:** raw ≤ **1,950,000** B, gzip ≤ **810,000** B, measured **1,739,791 / 718,136**. The 1,237,349 probe compiled view mode only; edit mode's `regen::regenerate` + `outcome::html_outcomes` add +435,770 B, the JSON boundary +42,127 B, and the `rlib` crate-type's lost fat-LTO scope +113,844 B (ticket `92ac61b9`). The same probe rebuilt after comrak `default-features = false` measures 1,148,050 B, confirming Task 1 landed.]
- Frozen-file proof at the end: `git diff <wave-base>..HEAD -- web/index.html web/js/sync.js crates/transync-cli/web/ crates/transync-syntax/src crates/transync/tests/public_surface.rs` is EMPTY (transync-syntax manifest included via the crates/transync-syntax path).
