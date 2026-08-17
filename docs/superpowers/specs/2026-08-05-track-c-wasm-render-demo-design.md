# Track C — In-Browser WASM Rendering + Edit Demo — Design Spec

- **Date:** 2026-08-05
- **Status:** owner-approved design (approach A1; sections S1–S8 approved 2026-08-05)
- **Context:** Track C proper — the work OI-0028 (DCR-0017) deliberately deferred after
  delivering the compile path and its standing gate, and the path DCR-0019 shrank
  precisely so this ships lean. Post-0.2.0, additive: the facade §0 surface, the
  alignment schema (1.2.0), the CLI bundle contract (ADR-0006 six files), both shipped
  shells, and `sync.js` are all UNTOUCHED.
- **Owner decisions (2026-08-05):**
  1. **Scope = web-only demo + target-Markdown editing.** The CLI `--html-out` bundle
     does NOT gain wasm (measured module: 1.18 MiB raw / 431 KiB gzip ≈ 30× the entire
     current 42 KB JS payload; bundling would break the six-file contract and inflate
     the CLI binary ~1.2 MB for a redundant capability — bundles already carry rendered
     HTML). The wasm renderer lives in the `web/` workspace demo plus its Playwright
     smoke, and additionally supports in-browser editing of the translated Markdown.
     *(2026-08-05: the measured figures quoted here are the view-only probe's; the
     shipped module including edit mode is 1.66 MiB raw / 701 KiB gzip ≈ 41×. See the
     size-budget correction in §3 — the decision is unchanged and better supported.)*
  2. **Toolchain = brew binaryen (≥131) + custom profile.** The wasm-pack-cached
     binaryen 117 rejects rustc 1.97 output (bulk-memory — reproduced, blocking);
     `scripts/build-wasm.sh` drives explicit `wasm-opt` from brew binaryen with
     `wasm-pack --no-opt`, and a root `[profile.wasm-release]` keeps the CLI's release
     profile untouched. binaryen is a new documented host prerequisite; its absence
     fails LOUDLY (never a silent skip — the wasm-gate precedent).
     *(2026-08-05: "brew binaryen" names the intended version, not the channel that
     was used. On the implementing host `brew install binaryen` was impossible — the
     brew prefix is owned by another uid, verified as a real filesystem permission —
     and binaryen **131**, the same version brew ships, was installed from the
     SHA-256-verified upstream release into a user prefix ahead of brew on PATH.
     What the script actually enforces is "a `wasm-opt` ≥ 121 on PATH", which is
     host-portable by construction and leaves plain brew correct elsewhere. The
     install-channel record and its rationale are in **ADR-0019**, decision 3.)*
  3. **OI-0020 stays open; version pinned instead.** `Cargo.lock` remains uncommitted;
     `wasm-bindgen = "=0.2.126"` is pinned exactly (crate↔CLI exact-match requirement),
     after `cargo update -p wasm-bindgen` unifies the tree (slug→comrak already pulls
     wasm-bindgen on wasm32, so this is a shared resolution, not a new dependency
     class).
     *(2026-08-05: the parenthetical is wrong on the mechanism, and the correction
     sharpens OI-0020 rather than softening it. `slug→comrak` declares only a caret
     `"0.2"` and constrained nothing; the binding edge is host-side —
     `reqwest → js-sys` in `transync-openai`, where `js-sys 0.3.97` **exact-pins**
     `wasm-bindgen = "=0.2.120"`. `cargo update -p wasm-bindgen` is consequently a
     **no-op** here, and unification required a seven-package update that exists only
     in the gitignored lock. See **ADR-0019** decision 2 and the OI-0020 note in
     `open-issues.md` for the clean-checkout failure mode this creates.)*
- **Exploration evidence (2026-08-05 three-reader workflow, probe-verified):** a wasm
  build of parse→render round-trips **byte-identical** pane fragments against the
  host render; parse of a 42 KB document costs ~1–2 ms on wasm; wasm-pack respects the
  machine-global `CARGO_TARGET_DIR`; `--target web` emits native ESM that drops into
  the existing `<script type="module">` convention with no bundler.

## 0. Goals

A browser page that (a) renders both panes locally from `source.md` + `out.md` +
`alignment.json` via the same Rust renderer the CLI uses — no second Markdown parser,
per the standing invariant — and (b) lets the user edit a translated block's Markdown
payload and see the pane re-render, re-align, and re-sync live. Everything existing
stays byte-stable: this wave only ADDS files and gate lines.

## 1. New crate `crates/transync-wasm` (S1)

- `crate-type = ["cdylib", "rlib"]`, `version.workspace = true`, `publish = false`.
- Dependencies: **`transync-syntax` ONLY** among workspace crates (double rationale:
  the crate charter, and the getrandom trap — core's wasm32 tree reaches getrandom via
  tokio/tiktoken; syntax's does not), `wasm-bindgen = "=0.2.126"` (owner decision 3),
  `serde_json` (workspace). `transync-syntax`'s own manifest is untouched (no
  `[features]`, no core dep — the standing charter).
- Layout: logic functions in plain Rust modules (host-testable), `#[wasm_bindgen]`
  wrappers in a thin binding module. File-as-module, never `mod.rs`.
- Host builds are clean (verified: cdylib+bindgen crates build/test on the host;
  bindgen exports are inert there), so `cargo build/test/clippy --workspace` sweep the
  new member with no special-casing.

## 2. Boundary contract (S2, approach A1)

Two stateless entry points, JSON-string boundary in and out (no serde-wasm-bindgen —
no new dependency; JS does one `JSON.parse` per call). Errors become thrown JS
exceptions carrying stringified messages.

1. **View mode:** `render_pair(source_md, translated_md, alignment_json) →
   JSON{source_html, target_html}`. Internally: `parser::parse` →
   `id::assign_block_ids` → deserialize `AlignmentMap` → `render_source` /
   `render_target`. The probe proved this reproduces the CLI's pane fragments
   byte-identically.
2. **Edit mode (the full local loop):** `rebuild(source_md, payloads_json,
   statuses_json, source_lang, target_lang, detected_opt) →
   JSON{source_html, target_html, alignment_json, translated_md}`. Internally: parse →
   assign ids → `outcome::html_outcomes` → `regen::regenerate` →
   `align::build_alignment_map` → render both panes. `payloads_json` is
   `{block_id: markdown_payload}` (html blocks' payloads are their segment-array JSON,
   as regen expects); `statuses_json` is `{block_id: fallback_status}` (snake_case wire
   values).

**Why edit mode is the full loop:** re-using a fetched `alignment.json` against an
edited `out.md` silently mis-slices html/skipped blocks in the target pane (their
render arms slice `target_range` byte offsets that the edit shifted — probe-confirmed
hazard). Rebuilding regen+map+render per edit eliminates the stale-range trap, and
per-BLOCK payload editing structurally preserves the block set — so the ID-stability
wall (document-is-static invariant; NG4) is never approached.

3. **Lockstep leg:** `schema_version() → "1.2.0"` (re-exporting
   `ALIGNMENT_SCHEMA_VERSION`). The demo JS asserts it equals its own `KNOWN_SCHEMA`
   at init and refuses to run on mismatch (fail-closed); a host-side test in
   `transync-wasm` pins it against the constant so the wasm blob can never disagree
   with the crate it was built from.

## 3. Build pipeline (S3, owner decision 2)

- **Root `Cargo.toml`:** add `[profile.wasm-release]` (`inherits = "release"`,
  `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `panic = "abort"`,
  `strip = true`). The CLI's stock `release` profile is unchanged. `panic = "abort"`
  is safe: the syntax API returns `Result`s rather than panicking as control flow.
- **Workspace deps:** `comrak = { version = "0.27", default-features = false }` — the
  default `cli`/`syntect` features are used nowhere (grep-verified), and dropping them
  saves ~7.8 % gzip. Verify the whole workspace (CLI, core, tests) still builds and
  behaves identically — the change is manifest-only.
- **`scripts/build-wasm.sh`** (matching the existing scripts' conventions: REPO_ROOT
  derivation, temp-dir allowlist):
  1. Preflight: `command -v wasm-pack` and a brew-binaryen `wasm-opt` with
     `--version` ≥ 121 — missing/old fails LOUDLY with install hints
     (`brew install binaryen`).
  2. `wasm-pack build crates/transync-wasm --target web --profile wasm-release
     --no-opt --out-dir <staging>`.
  3. Explicit `wasm-opt -Oz --enable-bulk-memory --enable-reference-types
     --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals`
     over the emitted `.wasm` (the flag set verified to clear the rustc-1.97
     bulk-memory rejection).
  4. Place artifacts (`transync_wasm.js` glue + `transync_wasm_bg.wasm`) in
     `web/wasm/` — **gitignored** (root `.gitignore`'s reserved build-artifact slot;
     wasm-pack's own `pkg/.gitignore` self-ignores its staging).
  5. **Size budget assertion:** raw ≤ 1.4 MB, gzip ≤ 500 KB (measured baseline
     1.18 MiB / 431 KiB — headroom without slack for creep). Over-budget fails the
     script.

     **Correction (2026-08-05, owner-approved):** those ceilings were calibrated on
     an exploration probe that compiled **view mode only** — parse → alignment map →
     render. The approved crate also compiles **edit mode**, and `regen::regenerate`
     + `outcome::html_outcomes` pull in Markdown reserialization that the probe never
     reached: **+435,770 B**, measured on an otherwise identical probe. The corrected
     budget is **raw ≤ 1,950,000 B, gzip ≤ 810,000 B** against a measured
     **1,739,791 B / 718,136 B**. The JSON boundary itself costs only +42,127 B, so
     the `serde-wasm-bindgen`-free choice holds on size grounds. Full attribution
     chain (including the rebuilt view-only control) is in the Task 3 report; the
     remaining +113,844 B of fat-LTO scope lost to the `rlib` crate-type is ticketed
     as `92ac61b9`. The larger module *strengthens* the web-only decision above —
     the CLI bundle would have carried ~1.74 MB, not ~1.2 MB, for a redundant
     capability.
- wasm-pack verified to respect the machine-global cargo target dir; the script must
  never set `--target-dir` or `CARGO_TARGET_DIR`, and never add a repo-local
  `.cargo/config.toml` with `[build]` keys.

## 4. Demo page (S4)

New `web/demo-wasm.html` + `web/js/wasm-demo.js` (ESM, framework-free). The existing
`web/index.html`, `index.html.tpl`, and `sync.js` are untouched — the demo imports
`sync.js`'s public exports (`mountSync`, `safeStorage`, `fetchOk`) only.

Flow:
1. `import init, { render_pair, rebuild, schema_version } from "./wasm/transync_wasm.js"`;
   `await init()`; assert `schema_version()` vs the demo's `KNOWN_SCHEMA` mirror —
   mismatch renders an error panel and stops (fail-closed).
2. `fetchOk` `source.md`, `out.md`, `alignment.json` (fixture siblings).
3. Initial render via `render_pair`; **DOMPurify fail-closed mount exactly as the
   shipped shells** (wasm-rendered HTML is still untrusted-source content — invariant
   7; never bypass to `insertAdjacentHTML`); `mountSync`.
4. Build the initial edit model: for each alignment row that is translatable
   (non-html, non-skipped, sync-relevant), slice `out.md` by its `target_range` —
   valid at load time, before any edit — into `{id → payload}`; statuses from each
   row's `fallback_status` (kept as-is across edits: the status records LLM
   provenance; a human demo edit does not rewrite it). html blocks are read-only in
   the demo (their payloads are segment JSON, out of demo scope).

   **Correction (2026-08-05, post-implementation review):** "kept as-is across
   edits" holds for the rows this step actually collects, and the parenthetical
   above understated what "read-only" costs. The first implementation collected a
   payload *and* a status for **every** row, html included — and an html block's
   segment array is not recoverable from a rendered bundle, so its Markdown slice
   fails `regen`'s parse and the block silently reverts to **source** bytes on any
   edit anywhere in the document, while the preserved `translated` status kept the
   row (and the tint legend) claiming otherwise. The collection rule is therefore
   the one this step's own first sentence already states — **translatable rows
   only**: html, skipped, and non-sync rows are omitted from `payloads` *and*
   `statuses`. `align::build_alignment_map` then synthesizes the honest status for
   them (`fallback_source` for unit-backed html, `preserved` for the rest), and the
   renderer presents html rows as the escaped `data-skipped="html-block"`
   placeholder under the fallback tint in both panes. For skipped / non-sync rows
   the omission is a no-op (their target bytes already are their source bytes).
   Recorded in ADR-0019's read-only consequence and DCR-0020's edit-model
   carve-out; pinned by `transync-wasm`'s
   `omitted_unit_backed_html_row_is_fallback_source_not_translated` and browser
   test `c`.
5. Edit loop: click a target-pane block → textarea with its payload → debounced
   `rebuild()` → sanitize → replace both panes' HTML → restore each pane's
   `scrollTop` → carry `<details>` open state over → `mountSync` again (re-mount is
   already idempotent by design).

## 5. Fixture, serving, Playwright wiring (S5)

- `web/tests/support/static-server.mjs` gains `".wasm": "application/wasm"` (the glue
  falls back to `arrayBuffer()` on wrong MIME but logs a console warning, which would
  trip console-cleanliness assertions).
- `scripts/test-browser.sh` gains a wasm leg: run `scripts/build-wasm.sh`, then copy
  into the (temporary) fixture dir: the `web/wasm/` artifacts, `demo-wasm.html`,
  `js/wasm-demo.js`, the fixture's source `.md`, and `out.md` — the fixture dir is
  scratch, so the CLI bundle contract (`HTML_BUNDLE_ENTRIES`, foreign-file
  allowlists, the six-file assertion loops) is untouched; the existing six-file loop
  simply grows a wasm-leg sibling list for the demo files.
- New `web/tests/wasm.spec.js` runs in the same Playwright project (same server, same
  config; Chromium-only, workers 1).

## 6. Gates (S6)

- Both pre-commit hook copies' wasm line and `scripts/smoke.sh`'s wasm line become
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  (the hook stays rustup-only — no wasm-pack/binaryen in the hook).
- `scripts/smoke.sh` additionally runs `scripts/build-wasm.sh` (loud prereq failure)
  — which carries the size budget — and its rustdoc gate package list gains
  `-p transync-wasm`.
- The Playwright wasm spec belongs to `scripts/test-browser.sh` (as SCN-13 does).

## 7. Tests (S7)

- **Host-side (`transync-wasm`):** the logic layer round-trips `render_pair` and
  `rebuild` against a fixture (assert non-empty `<main>` fragments, alignment JSON
  re-parses, translated_md equals regen output); `schema_version` pinned to
  `ALIGNMENT_SCHEMA_VERSION`; error paths (bad JSON, unknown block id) return `Err`
  and never panic.
- **Playwright (`wasm.spec.js`):**
  1. module loads; console clean (zero warnings — including no MIME fallback warning);
  2. **parity pin:** wasm-rendered `source_html`/`target_html` fragments equal the
     CLI-generated `source.html`/`target.html` pane content for the same fixture (the
     probe's byte-identity as a standing gate);
  3. edit loop: change one paragraph payload → target pane updates, all
     `data-sync-id` anchors survive, sync still drives the counterpart pane;
  4. schema-mismatch injection → the demo refuses to mount (fail-closed panel).

## 8. Records (S8)

- **ADR-0019** — the wasm demo layer: crate shape (syntax-only dependency), the
  `=0.2.126` pin + OI-0020 deferral rationale, binaryen host prerequisite, the size
  budget, and the web-only decision (why the CLI bundle deliberately does NOT carry
  wasm).
- **DCR-0020** — the implementing record (files, gates, measured sizes, parity
  evidence).
- OI-0028 gains a dated "Track C proper landed" note; `mvp-scope.md`'s Track C line
  updated; `status.md` / `phase-state.yaml` / `CHANGELOG.md [Unreleased]` at landing;
  the binaryen prerequisite documented in the Developer Guide's smoke-script table.

## 9. Out of scope

- CLI bundle integration (owner decision 1) and any `HTML_BUNDLE_ENTRIES` change.
- Source-pane editing / block-structure-changing edits (NG4 ID-stability wall) —
  the demo edits per-block payloads only.
- In-browser re-translation (transync-core cannot compile to wasm32 — tokio/tiktoken;
  a network boundary or second split would be its own project).
- `wasm-bindgen-test` browser leg (a second browser-automation stack; Playwright
  covers the load path).
- Committing `Cargo.lock` (OI-0020 stays open per owner decision 3) and any registry
  publishing.
