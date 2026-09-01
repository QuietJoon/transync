---
type: DCR
title: Track C wasm render+edit demo — transync-wasm, the build pipeline, the browser demo, and the parity gates
description: Six commits landing Track C proper. A sixth workspace member (`transync-wasm`) wraps `transync-syntax` behind a JSON-string boundary with view mode (`render_pair`), edit mode (`rebuild`), and a `schema_version` lockstep leg; `scripts/build-wasm.sh` drives wasm-pack plus an explicit binaryen `wasm-opt` under an enforced size budget; `web/demo-wasm.html` + `web/js/wasm-demo.js` render both panes locally and edit translated blocks live. Playwright grows 8/8 to 12/12 with a CLI-vs-browser parity pin. Nothing existing moved — the shells, `sync.js`, the bundle contract, and the curated surface are byte-identical. Implements ADR-0019.
tags: [change, project-control, DCR-0020]
status: active
---

# DCR-0020: Track C wasm render+edit demo

- **Date:** 2026-08-05
- **Source:** the Track C wasm render+edit demo wave, implementing the
  owner-approved design spec
  `docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md`
  (approach **A1**; sections S1–S8 approved 2026-08-05) against the 6-task plan
  `docs/superpowers/plans/2026-08-05-track-c-wasm-render-demo.md`.
  **Six commits on `master`, `94d7e39`..`1d04135`** (one of them a fix commit
  aligning the spec and plan with the corrected size budget).
- **Implements:** **ADR-0019** — the wasm demo layer: crate shape, the
  `=0.2.126` pin over a committed lockfile, the binaryen prerequisite and its
  install-channel record, the size budget, and the web-only decision.
- **Affected ADRs:** `docs/decisions/0003-cargo-workspace-with-provider-crates.md`
  (**dated amendment 2026-08-05** — six members; the DAG gains
  `transync-wasm → transync-syntax`; the "five `Cargo.toml` files" tradeoff line
  becomes six); `docs/decisions/0006-renderer-output-shape.md` (**upheld** —
  the two-pane-fragment shape is what crosses the wasm boundary unchanged, and
  the six-file bundle contract is deliberately *not* extended); ADR-0001
  (**upheld** — the block ID remains the only sync currency across a new
  boundary and a live re-render); ADR-0004 (**upheld** — one parser; comrak's
  `default-features = false` drops `cli`/`syntect` only); ADR-0018
  (**upheld** — html blocks stay read-only in the demo *because* their payload
  is a segment array). **OI-0028** gains a dated "Track C proper landed" note;
  it stays **RESOLVED** — its scope was the compile path, and this wave is the
  integration it deferred.
- **Files:** TicGit ticket **`0c6a5c8b`** — the demo's uncovered non-fatal
  error-strip path (see *Migration / Follow-up*) — joining the pre-existing
  **`92ac61b9`**.

## What Changed

Everything is **additive**. The wave's frozen set —
`web/index.html`, `web/js/sync.js`, `crates/transync-cli/web/` (both
`index.html.tpl` and its `sync.js` copy), all of `crates/transync-syntax`, and
`crates/transync/tests/public_surface.rs` — has an **empty**
`git diff 94d7e39..HEAD`, which is the wave's proof it added a layer rather
than moving one.

### Part A — Manifest prep (commit `94d7e39`)

Root `Cargo.toml` only, +12 / −1.

- **`comrak = { version = "0.27", default-features = false }`** workspace-wide.
  The defaults are `cli` + `syntect`; a grep across `crates/`, `scripts/`, and
  `web/` for `syntect|ComrakPlugins|comrak::plugins|SyntectAdapter` returns
  **zero** hits, every comrak reference is core parse/render API, and no
  workspace member forwards a feature to comrak (all five `[features]` tables
  checked), so `clippy --all-features` cannot re-enable them. Post-change the
  whole dropped family — `clap`, `syntect`, `flate2`, `plist`, `yaml-rust`,
  `fancy-regex`, `bincode`, `walkdir` — greps clean out of the `wasm32`
  dependency tree.
- **`[profile.wasm-release]`** (`inherits = "release"`, `opt-level = "z"`,
  `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`).
  Profiles must live at the workspace root — cargo ignores non-root ones — and
  a *custom* profile is what leaves `transync-cli`'s stock `release` build
  untouched. Verified to actually resolve on the build path
  (`cargo check --profile wasm-release --target wasm32-unknown-unknown`).
- **`wasm-bindgen = "=0.2.126"`** in `[workspace.dependencies]`, with the
  rationale inline at the pin.
- **Behavior preservation is an exact per-target match, not a spot check.** A
  pre-change baseline was captured first and compared binary by binary:
  **385 passed / 0 failed / 3 ignored** before and after, identical in every
  one of the fifteen targets. The CLI stub suite (34) was run additionally,
  because `cli_smoke` has **zero** tests without `--features
  test-stub-provider` and is exactly where a comrak render regression would
  surface.

**The plan's one-liner did not work, and the correction is the ADR's
escalation.** `cargo update -p wasm-bindgen` is a **no-op** on this workspace
(`Locking 0 packages`). The binding constraint is `js-sys 0.3.97`'s
`wasm-bindgen = "=0.2.120"`, reached host-side through
`reqwest → transync-openai` — *not* the `slug → comrak` edge the analysis
predicted, which declares only a caret `"0.2"`. Unification needed a
seven-package update (`wasm-bindgen` + macro/macro-support/shared, `js-sys`
0.3.97→0.3.103, `web-sys` 0.3.97→0.3.103, `wasm-bindgen-futures`
0.4.70→0.4.76); `reqwest` itself did not move, and `transync-openai`'s 37 unit
tests plus 2 offline live-smoke tests stayed green, so the bump is behaviorally
inert here. MSRV was ruled out (wasm-bindgen 0.2.126 declares `rust-version`
1.77, below the workspace's 1.85). See ADR-0019 decision 2 for why this stays a
pin rather than a committed lock.

### Part B — `crates/transync-wasm` (commit `35aa2bf`)

Three files, 487 lines. `crate-type = ["cdylib", "rlib"]`, `publish = false`,
workspace-inherited metadata. Dependencies: `transync-syntax` (the only
workspace crate), `wasm-bindgen` (workspace pin), `serde`, `serde_json`,
`thiserror`.

**Layout is the point.** `engine.rs` holds every line of logic and **zero**
wasm-bindgen types; `lib.rs` holds three `#[wasm_bindgen]` wrappers that
stringify, delegate, and re-serialize. So the whole surface is exercised by
`cargo test -p transync-wasm` on the host, and the wrappers are short enough to
verify by inspection.

#### The boundary contract

JSON strings in, one JSON string out (spec §2, approach A1) — no
`serde-wasm-bindgen`, no new dependency, one `JSON.parse` per call on the JS
side. Errors become thrown JS exceptions carrying the stringified engine
message.

| entry point | inputs | returns |
|---|---|---|
| `render_pair` (view) | `source_md`, `translated_md`, `alignment_json` | `{source_html, target_html}` |
| `rebuild` (edit) | `source_md`, `payloads_json`, `statuses_json`, `source_lang`, `target_lang`, `detected?` | `{source_html, target_html, alignment_json, translated_md}` |
| `schema_version` (lockstep) | — | `"1.2.0"` |

- **View mode** is `parse` → `assign_block_ids` → deserialize `AlignmentMap` →
  `render_source` / `render_target`. It reproduces the CLI's pane fragments
  byte-identically; that is a standing gate, not a claim (Part E).
- **Edit mode** is the *whole* loop — `parse` → `assign_block_ids` →
  `outcome::html_outcomes` → `regen::regenerate` → `align::build_alignment_map`
  → render both panes — and deliberately never reuses the fetched map. The
  html and skipped render arms slice `target_range` **bytes**, so a stale map
  against an edited `out.md` mis-slices them (probe-confirmed). Rebuilding
  per edit removes the trap by construction, and `alignment_json` comes back
  **nested** (a JSON string inside the result object) so the caller can hand it
  straight back to `render_pair`.
- **`assign_block_ids` is called even though `parse` leaves it idempotent
  today**, because the ids on the wire must be the pipeline's, not the walker's
  provisional ones.
- **Unknown ids are rejected, not ignored.** An id in `payloads_json` or
  `statuses_json` that names no block in the parsed source produces an error
  naming the field and the sorted id list. Silently dropping it would drop a
  user's edit on the floor.
- **`EngineError` is `#[non_exhaustive]`** and every variant names the input
  that failed, because the JS side sees only the stringified message.

#### The edit model

`rebuild`'s two maps are the whole design:

- **`payloads_json` is `{block_id: markdown_payload}`.** Absence *is* the
  fallback contract — `regen::regenerate` falls a missing block back to its own
  source bytes — so an unedited document round-trips byte-identically
  (pinned: no translations ⇒ `translated_md == source_md`).
- **`statuses_json` is `{block_id: fallback_status}` in snake_case wire form.**
  Statuses are **preserved across edits, not rewritten**: the status records
  *LLM* provenance, and a human demo edit is not an LLM decision. A value
  outside the wire enum is a deserialize failure, not a silent default (pinned).

  **Carve-out, 2026-08-05:** preservation applies to the rows the demo can
  actually carry — the editable ones. For **html** rows, preserving the status
  is exactly what would make the tint legend lie: their payload cannot survive
  a rebuild (next bullet), so the block reverts to source content while a
  preserved `translated` kept asserting otherwise, untinted. `buildEditModel`
  therefore omits html rows (and skipped / non-sync rows, where omission is a
  no-op) from **both** maps, and `align::build_alignment_map` synthesizes the
  honest status: `fallback_source` for unit-backed html, `preserved` for the
  rest. Presentation follows — the escaped `data-skipped="html-block"`
  placeholder under the fallback tint, in both panes. This restores spec §4.4's
  own first sentence ("for each alignment row that is translatable — non-html,
  non-skipped, sync-relevant"), which the first implementation over-collected.
- **html blocks are read-only.** Their payload is an extracted text-segment
  array (ADR-0018), not Markdown, so a Markdown slice could not be spliced; the
  demo excludes them from its editable set rather than offering an edit it
  cannot round-trip. *(2026-08-05: and unrecoverable from the rendered bundle,
  which is why they cannot be carried through a rebuild either — see the
  carve-out above.)*
- **Per-block payloads structurally preserve the block set**, because identity
  comes from parsing the *source* document, which the demo never edits. The
  ID-stability wall (invariant 8) is never approached.

**Eight host tests** in `engine.rs`, of which four carry the argument: the
two-mode agreement pins (view mode reproduces rebuild's panes byte-for-byte,
both with and without an applied payload, the second also asserting the map's
`detected_source_language` and `validation_summary.translated`),
`schema_version_is_the_wire_constant` (the blob can never disagree with the
crate it was built from), and the four error-path tests that prove bad JSON,
an out-of-enum status, unknown ids, and an empty document all return `Err`
rather than panicking — which is what makes `panic = "abort"` safe.

*(2026-08-05: **nine**, with the status-honesty carve-out above —
`omitted_unit_backed_html_row_is_fallback_source_not_translated` pins the
synthesis the carve-out depends on: an omitted unit-backed html row comes back
`fallback_source` and renders as the escaped placeholder in both panes.)*

### Part C — `scripts/build-wasm.sh` (commits `fbb27c5`, `d40f372`)

New, `755`, shellcheck-clean, following its siblings' conventions (`REPO_ROOT`
derivation, `[build-wasm]` step announcements, a guarded staging dir).

- **Preflight fails loudly**, never skips: missing wasm-pack, missing
  `wasm-opt`, `wasm-opt` < 121, an unparseable `--version`, and a non-zero
  `wasm-opt` all exit 1 with a diagnostic — each **tested, not assumed**, along
  with a `TRANSYNC_WASM_STAGING` pointed at a valuable directory (refused).
- **`wasm-opt` is resolved once by absolute path** via `command -v` and invoked
  through that variable, so a run can never drift onto wasm-pack's cached
  binaryen 117 in `~/Library/Caches/.wasm-pack/`. Build is
  `--target web --profile wasm-release --no-opt --no-pack`; the explicit pass
  is `-Oz` with the five `--enable-*` proposal flags.
- **`--no-opt` is mandatory, not preferred**, under a custom profile: wasm-pack
  only honours `[package.metadata.wasm-pack.profile.{dev,release,profiling}]`
  and silently ignores the key for `wasm-release`.
- **Emitted filenames were verified, not assumed** — staging holds exactly
  `transync_wasm.js`, `transync_wasm_bg.wasm`, and their two `.d.ts` siblings.
  The script asserts both artifacts exist and dumps the staging listing if
  wasm-pack ever renames them.

  *(2026-08-05: "exactly" describes the four artifacts, not the whole
  listing — wasm-pack also drops its own `.gitignore` there, and staging now
  carries `.transync-workdir` as well: the marker this repo's scripts stamp
  right after `mkdir`. The backlog item
  `smoke-script-rm-rf-basename-allowlist` replaced the "basename starts with
  `transync-`" deletion hatch this script shared with its siblings, and that
  marker is what authorises a later run to wipe a staging dir sitting outside
  every approved temp root. The guard itself moved to the shared
  `scripts/lib/workdir-guard.sh`. The two emitted artifacts, and the sizes
  measured from them, are unchanged.)*
- **Never sets `CARGO_TARGET_DIR` or `--target-dir`**, and adds no repo-local
  `.cargo/config.toml`: wasm-pack shells out to cargo and already respects the
  machine-global target dir (confirmed — no repo-local `target/` appeared).
- **Output lands in `web/wasm/`**, added to `.gitignore` under the
  build-artifacts block (whose "none yet, but reserve" comment finally came
  true). `git check-ignore` resolves both emitted files to that rule and
  `git status` stays clean after a build.
- **A self-review defect was found and fixed here:** the version parse
  (`grep -oE … | head -1`) died *silently* under `set -euo pipefail` when
  `--version` printed no digits. Fixed with `|| true` plus an explicit regex
  check, then retested.

*(2026-08-09, Review 0003 batch B7 — `R0003-0007`, `R0003-0008`,
`R0003-0081`, `R0003-0082`, `R0003-0083`. The script's publication step is
rewritten; everything above about **what** it builds still holds. Then:
artifacts were copied into `web/wasm` and measured afterwards, so the size
budget was enforced on a module that was already installed, the glue and the
module arrived as two independent copies, `web/wasm` was updated by name
rather than replaced as a set, and the staging path was a fixed directory
wiped at start — which made two concurrent runs delete each other's build.
Now: the build, the `wasm-opt` pass, the measurement and both budget checks
happen in a per-run `mktemp -d` staging directory, and `web/wasm` is replaced
whole by renaming a directory under a `mkdir` publish lock. `TRANSYNC_WASM_STAGING`
consequently names the staging **root** — the directory each run creates its
own tree inside — rather than the staging directory itself; the deletion guard
is still asked about that root, so the refusal recorded above is unchanged, and
the root is no longer wiped wholesale. The version parse above gained a second
correction: it anchors on the `version <n>` token instead of the first digits
anywhere in the output, because a vendor banner leading with a year read as the
year, and it echoes unparseable output verbatim. Sizes, artifacts and the
budget derivation are untouched.)*

#### The size budget, and the durable attribution table

Enforced ceilings: **raw ≤ 1,950,000 B, gzip ≤ 810,000 B**, owner-approved
2026-08-05. Measured shipped module: **1,739,791 B raw / 718,136 B gzip**
(1.66 MiB / 701 KiB), byte-reproducible across two consecutive clean rebuilds.
Glue: 11,785 B.

The spec's provisional 1,400,000 / 500,000 came from an exploration probe that
compiled **view mode only**. The first real build came in 40 % over that
baseline, and the budget was **not** touched until the overage was fully
attributed by measurement. **This table is the durable home for that
attribution** — the spec's §3 correction cites a scratch task report, which is
why it is reproduced here:

| build | raw wasm (B) | delta (B) |
|---|---|---|
| control — the original exploration probe, rebuilt today on this toolchain | 1,148,050 | (was 1,237,349 before Task A's comrak change: **−89,299**, matching the predicted −87,334) |
| P1 — parse → alignment map → render *(what the probe actually compiled)* | 1,148,050 | baseline |
| P2 — P1 + the JSON boundary (`serde_json`, `AlignmentMap` Deserialize, output Serialize) | 1,190,177 | **+42,127** |
| P3 — P2 + `regen::regenerate` + `outcome::html_outcomes` + `id::assign_block_ids` | 1,625,947 | **+435,770** |
| shipped crate, `crate-type = ["cdylib", "rlib"]` | **1,739,791** | **+113,844** |

Three readings, all decision-relevant:

- **The dominant term is edit mode.** `rebuild` pulls Markdown
  *reserialization* — the regen splice/fence/table machinery — that a
  render-only path never touches. The old ceiling was calibrated against a
  strict **subset** of the shipped API. This is corrected input, not size creep
  and not a build defect.
- **The control run is the proof the environment is consistent**: rebuilding
  the *unchanged* probe today lands 89,299 B below its original figure, which
  is Part A's comrak change showing up exactly where predicted.
- **The last row is recoverable and deliberately not spent.** Rebuilding with
  `["cdylib"]` alone, nothing else changed, yields 1,625,947 B — the `rlib`
  target costs fat-LTO scope, nothing in the workspace consumes
  `transync-wasm` as a library, and it is ticketed as **`92ac61b9`** rather
  than taken (it touches the `cargo doc` / `cargo test` gates, and it would not
  have brought the module under the old ceiling anyway).

  *(2026-08-05, ticket `92ac61b9` resolved: **that row has now been spent.**
  `crate-type` is `["cdylib"]`; the gates it was expected to touch do not
  break, because `cargo test` compiles the lib target into its own test
  harness regardless of crate-type — `engine`'s host tests, `cargo doc
  -p transync-wasm` under `RUSTDOCFLAGS=-D warnings`, and `clippy
  --all-targets` were each re-run and stayed green. Re-measured A/B on one
  commit at the crate's then-current state, which is slightly larger than the
  table above (ticket `2a276c` added engine code): **1,740,748 → 1,639,520 B
  raw (−101,228)** and **718,553 → 671,795 B gzip (−46,758)**. The table's
  1,739,791 / 1,625,947 pair stands as the measurement of the day it was
  taken. `MAX_RAW` / `MAX_GZ` in `scripts/build-wasm.sh` came down with the
  module — 1,840,000 / 760,000, the same ~12 % headroom the earlier pair
  carried — so the saving is held rather than left as slack.)*

Commit `d40f372` is the fix commit: it aligned the spec §3 bullet, three
inline corrections in the plan, and the script's header comment with the
owner-approved numbers, plus a one-line dated pointer at the spec's own
decision-1 rationale, which quoted the stale "≈ 30×" figure. **Annotate, don't
rewrite** — the original sentences stand.

### Part D — The demo page (commit `bfe5a44`)

`web/demo-wasm.html` (180 lines) + `web/js/wasm-demo.js` (468 lines), ESM and
framework-free, importing only `sync.js`'s public exports (`mountSync`,
`fetchOk`). The shell is a clone of `web/index.html`'s skeleton — same `:root`
tints, the same `.pane { position: relative; overflow: auto }` layout
precondition, the same `[data-fallback]` tints and `pre[data-skipped]`
placeholder rule, the same classic `<script src="vendor/purify.min.js">`
**before** the module script — plus an `.editor-panel` spanning both grid
columns, `#editor-label`, `#status-strip`, and a hidden `#error-strip`.
`web/index.html` was never opened for writing.

#### The layout decision

**The wasm artifacts stay in `web/wasm/`, and `web/js/wasm-demo.js` imports the
glue as `../wasm/transync_wasm.js`; the Playwright fixture mirrors `web/`'s
directory shape verbatim**, so the same relative specifiers resolve in both
trees. Three reasons, in order of force:

1. The wasm-bindgen glue resolves its own module as a **sibling of itself**
   (`new URL('transync_wasm_bg.wasm', import.meta.url)`), so glue and `.wasm`
   must share one directory whatever is chosen — only the *parent* was ever in
   question.
2. `scripts/build-wasm.sh` already emits into `web/wasm/`; the alternative
   would have meant editing a committed, owner-approved build script from a
   task not scoped to touch it.
3. Keeping `js/` (hand-written source) and `wasm/` (gitignored build output) as
   siblings puts the gitignore boundary on a directory line.

The decision is documented in three places that a reader can actually hit: the
`scripts/test-browser.sh` "WASM demo leg" comment block (with the full path
table), the module header of `web/js/wasm-demo.js`, and the commit message.

#### Fail-closed posture, and four implementation details worth recording

Rendered HTML is still untrusted-source content (invariant 7), so **every**
`innerHTML` write goes through `window.DOMPurify` — checked at boot *and*
re-checked inside `mountPanes`, so no write can bypass it — exactly as the
shipped shells do. Three independent fail-closed gates: DOMPurify absent, the
**module's** `schema_version()` vs the demo's `KNOWN_SCHEMA`, and the **fetched
map's** `schema_version` (full `x.y.z` regex mirroring `sync.js`'s own
strictness; major must match; a newer minor/patch warns and proceeds, per
OI-0024 forward drift).

1. **Payload slicing is by UTF-8 bytes**, via `TextEncoder`/`TextDecoder`.
   `target_range` holds byte offsets, and `String.slice` would mis-cut — the
   SCN-14 fixture's em dashes already make byte ≠ UTF-16 index.
2. **Scroll and `<details>` restoration happens *between* the `innerHTML`
   writes and `mountSync`, not after mounting.** Measured: restoring after the
   engine's listeners are live reads as a *user* scroll and the two panes drive
   each other away from the restored position (observed 120/140 → 131/131);
   restoring inside the pre-mount window is exact (347 → 347). `<details>` is
   restored before `scrollTop` within that window, because re-opening a
   collapsed block changes `scrollHeight` and a `scrollTop` assigned against
   the shorter layout is clamped.
3. **`<details>` state is keyed `syncId#index`, not by `data-sync-id` alone**,
   because the renderer attributes the *wrapper*, so the `<details>` element
   carries no id of its own — the same pairing `sync.js`'s toggle mirror uses.
4. **Re-mount is `mountSync`'s documented idempotent path**; no `destroy()`
   exists or is needed.

A `rebuild` exception lands in the **non-fatal** `#error-strip`: nothing was
mounted, so the last good render stays up and `data-demo-state` stays `ready`.
`data-demo-state` on `<body>` (`booting` / `ready` / `fatal`) exists as a
readiness signal so tests do not race the async wasm init.

#### Fixture and serving

`web/tests/support/static-server.mjs` gained `".wasm": "application/wasm"` —
`WebAssembly.instantiateStreaming` rejects any other MIME and the glue's
fallback logs a console warning — plus `".md"`, since the fixture now serves
two Markdown payloads.

`scripts/test-browser.sh` gained a **wasm leg** after the existing six-file
bundle assertion (left intact): it runs `scripts/build-wasm.sh`, creates
`js/`, `wasm/`, and `vendor/` under the scratch fixture dir, copies the eight
demo files, and runs a **second** assertion loop over the eight new paths. Two
loops rather than one because the copies must happen *after* the CLI publish.
**Nothing is added to `HTML_BUNDLE_ENTRIES`**: the copies land in the scratch
fixture dir after the CLI has already published, so `ensure_html_out_safe`
never sees them, and the bundle's own flat `sync.js` / `purify.min.js` stay
exactly where the CLI wrote them for `index.html`.

### Part E — The Playwright suite (commit `1d04135`)

`web/tests/wasm.spec.js` (310 lines), **one file, nothing else touched** —
`scn13.spec.js`, `harness.js`, `playwright.config.js` are byte-identical to
their pre-task state. Same project, same server, same port, `workers: 1`,
`retries: 0`. **8 SCN-13 + 4 wasm = 12/12.**

- **a — boots clean.** Waits on `body[data-demo-state="ready"]`, asserts both
  panes' `[data-sync-id]` sequences equal the harness's 17-id `SYNC_IDS`, and
  asserts **zero** console warnings or errors at any severity plus an empty
  `pageerror` list. Exactly one entry is forgiven, matched on **resource URL**
  rather than message text: the fixture's `img-0014` points at a `logo.png` the
  bundle deliberately never ships, and Chromium's 404 text carries no URL.
  - **Verified non-vacuous by negative control:** re-serving the `.wasm` as
    `application/octet-stream` made test `a` fail with exactly the MIME
    fallback warning it exists to catch. The patch was reverted from a
    byte-level backup and `diff -q` confirmed the committed file is identical
    to the verified-green version.
- **b — parity, the probe's byte-identity promoted to a standing gate.** For
  each pane it fetches the CLI-rendered `source.html` / `target.html` from the
  same fixture, extracts the `<main>` fragment, and digests **both** sides with
  `DOMParser` into `{id, data-block-kind, textContent}` arrays, compared with
  one `toEqual` — plus a `SYNC_IDS` anti-vacuity guard so two empty lists can
  never pass. Zero diffs across all 17 blocks in both panes, `<details>`,
  `<figure>`, and table wrappers included.
  - The comparison is **structural rather than an `innerHTML` byte diff**
    because the demo mounts through DOMPurify (which may normalize) and the
    browser re-serializes what it parsed. **Pre-sanitize byte parity is proven
    on the host instead**, by `transync-wasm`'s two two-mode agreement tests —
    the in-test comment names them, so the two halves of the parity claim are
    linked in both directions.
- **c — the edit loop.** Ordered so the refusal is proven first: an html block
  reports "not editable" and leaves the editor disabled; a paragraph enables it
  preloaded with the sliced payload; `fill` arms the 300 ms debounce and the
  test waits on a **deterministic** status-strip signal rather than sleeping.
  Then: the target block carries the new text, **the source block's text is
  unchanged**, `#error-strip` stays hidden, both panes' id sequences still
  equal `SYNC_IDS` (invariant 1 survives a rebuild), and sync still drives the
  counterpart pane after the re-mount.
  - The landing assertion is **block-level, not exact `scrollTop`**: an edited
    target no longer shares the source's line-for-line layout, so equal scroll
    offsets would be the wrong claim. Both panes ending on the same block is
    the actual contract.
  - *(2026-08-05)* It also pins the html carve-out end to end: the block is
    `data-fallback="translated"` inside a live `<div>` before the edit, and a
    `fallback_source` `<pre data-skipped="html-block">` in both panes after —
    the honest degrade, not a silent revert under a `translated` row.
- **d — fail-closed, two variants in one test.** DOMPurify routed to an empty
  body, and the fetched map mutated to `schema_version: "9.9.9"`. Both assert
  `data-demo-state="fatal"`, a matching `#error-strip`, and **zero**
  `[data-sync-id]` anchors in either pane — refused, not crashed. The
  DOMPurify gate sits *after* wasm init and the module-vs-demo schema check, so
  reaching it also witnesses a healthy wasm boot. The wasm-side half of the
  schema pair is deliberately untested and the reason is in the test: a
  compiled-in `schema_version()` cannot be forged from JS.
- **Flake check:** four consecutive green 12/12 runs, zero retries.

### Part F — Gate lines

- Both pre-commit hook copies and `scripts/smoke.sh` now run
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`.
  The hook stays rustup-only — no wasm-pack or binaryen in the hook.
- `scripts/smoke.sh` additionally runs `scripts/build-wasm.sh` (which carries
  the size budget) after the rustdoc gate and before the CLI end-to-end, and
  its rustdoc package list gained `-p transync-wasm`.
- `scripts/test-browser.sh` owns the Playwright wasm spec, as it owns SCN-13.

### Wave-closing gate run

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo test --workspace -- --test-threads=4` — **393 passed / 0 failed /
  3 ignored** (summed `test result:` lines), against the pre-wave **385 / 0 /
  3**. The delta is exactly `transync-wasm`'s eight new host tests; **zero
  removals, zero adaptations** anywhere else.
- `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`
  — 34 passed (12 + 19 `cli_smoke` + 3 drift).
- `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  — clean.
- `scripts/build-wasm.sh` — **1,739,791 B raw / 718,136 B gzip**, inside the
  1,950,000 / 810,000 budget.
- `scripts/smoke.sh` — OK, including the standing
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` gate, which now covers
  `transync-wasm`'s modules warning-free.
- `scripts/test-browser.sh` — **12/12**.
- `crates/transync/tests/public_surface.rs` — green and **unmodified**;
  `git diff 94d7e39..HEAD` over the whole frozen set is **empty**.
- `/Volumes/Common/QJoon/resp-translator` — `cargo check --workspace` green
  against this HEAD. Nothing the sibling names moved.

### Invariants held

**Nothing on the wire moved.** The alignment-map schema stays **`1.2.0`** (the
new crate re-exports the constant rather than defining a second one),
`VALIDATION_SCHEMA_VERSION` stays **`2`**, `CacheKey` is untouched, both
`sync.js` copies are byte-identical, and the ADR-0006 six-file bundle contract
is unchanged. The curated facade surface is unchanged and `transync-syntax`
still declares no `[features]` and no dependency on `transync-core`.

**Invariant 1 holds across a new boundary.** The same `block_id` now flows Rust
IR → wasm boundary → regenerated Markdown → alignment map → DOM anchors → the
JS sync engine, and Playwright test `c` pins that the anchor set survives a
live rebuild.

**Invariant 8 is not approached.** Per-block payload editing of the translated
side cannot change the block set, because identity comes from the source parse
the demo never edits.

**DCR-0019's IR freeze rule is honored, and its open question is answered.**
`Document`, `Block`, and `Section` are unchanged: the JS boundary is
**neither** serde-on-the-IR nor a DTO layer — JSON strings cross it and the IR
never leaves Rust.

## Why

**A crate, not a feature**, because `transync-syntax` is forbidden a
`[features]` table by the charter that made the wasm gate real. Putting
bindings there would have reintroduced the exact unification trap OI-0028
Option B was chosen to avoid.

**Syntax-only dependency**, because the boundary is a cliff rather than a
slope: `transync-core`'s `wasm32` tree reaches `getrandom` via tokio/tiktoken,
and a single widening edge turns a working module into one that does not link —
discovered late, at link time, by whoever widened it. Two independent reasons
(charter, `getrandom`) mean the rule survives if either is forgotten.

**JSON strings over `serde-wasm-bindgen`**, because the boundary is stateless
and called at most once per keystroke-debounce, and the alternative buys a
dependency. The measurement settles it: the whole boundary costs **+42,127 B**,
about 2 % of the module.

**Edit mode runs the whole loop** because the cheap version is wrong. Re-using
the fetched `alignment.json` against an edited `out.md` leaves html and skipped
blocks slicing byte ranges the edit has shifted — a *silent* mis-render, which
is the worst failure class this project has. Rebuilding regen + map + render
per edit is more work per keystroke and removes an entire hazard class.

**Statuses are preserved, not rewritten, across edits** because
`fallback_status` records what the *LLM* did. A human editing a block in a demo
has not re-translated it, and overwriting the status would make the tint legend
lie.

**Web-only** because the two arguments point the same way and the measurement
sharpened both: the module is ~41× the entire 42 KB JS payload, and a
`--html-out` bundle already ships rendered HTML — the very fragments view mode
reproduces. Paying 1.74 MB to recompute what the artifact already contains is
the definition of a bad trade. The capability that is *not* redundant, local
editing, belongs in a demo.

**The size budget moved because the input was wrong, not because the module
grew.** The ceiling was calibrated on a probe compiling a strict subset of the
shipped API. The rule enforced here is the one that keeps a budget meaningful:
attribute first, adjust second, and never bump a ceiling that has not been
explained.

**binaryen is a prerequisite rather than a bundled tool** because wasm-pack's
cached 117 is not merely old, it *rejects* current rustc output — the failure
is blocking and the workaround (metadata flag lists) is silently ignored under
a custom profile. An explicit, absolutely-resolved `wasm-opt` invocation is the
only shape that cannot regress into the cached one.

## Affected Areas

- `Cargo.toml` — sixth member; comrak `default-features = false`; the
  `wasm-bindgen = "=0.2.126"` pin with its rationale; `[profile.wasm-release]`
- `crates/transync-wasm/Cargo.toml` (new) — `cdylib` + `rlib`, `publish = false`,
  syntax-only workspace dependency with the charter + `getrandom` rationale inline
- `crates/transync-wasm/src/lib.rs` (new) — the three `#[wasm_bindgen]` wrappers
- `crates/transync-wasm/src/engine.rs` (new) — `render_pair_impl`,
  `rebuild_impl`, `schema_version_impl`, `EngineError`, and the eight host tests
- `scripts/build-wasm.sh` (new) — preflight, wasm-pack `--no-opt`, explicit
  `wasm-opt`, `web/wasm/` placement, enforced size budget
- `scripts/hooks/pre-commit`, `scripts/smoke.sh` — wasm check line gains
  `-p transync-wasm`; smoke gains the build step and the rustdoc package
- `scripts/test-browser.sh` — the wasm demo leg (build, eight fixture copies,
  second assertion loop) plus the import-path table
- `web/demo-wasm.html`, `web/js/wasm-demo.js` (new) — the demo shell and
  controller
- `web/tests/support/static-server.mjs` — `.wasm` and `.md` MIME entries
- `web/tests/wasm.spec.js` (new) — the four-test suite
- `.gitignore` — `web/wasm/` under the build-artifacts block
- Records: this DCR (new), **ADR-0019** (new), `docs/decisions/0003-…`
  (dated six-member amendment), `docs/project/open-issues.md` (OI-0028 dated
  Track-C-landed note; OI-0020 dated pin-over-lock note),
  `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/index.md`,
  `CHANGELOG.md`
- Living docs: `docs/implementation/module-map.md` (the new crate, the demo
  files, and the scripts), `docs/architecture/mvp-scope.md` (the Track C line
  and the WASM-renderer row), `docs/Developer_Guide.md` (the widened wasm gate,
  the rustdoc package list, and binaryen/wasm-pack as prerequisites in the
  smoke-script table), `docs/architecture/README.md` (invariant 8's browser
  clause), `CLAUDE.md` (six members, the widened gate command, and the
  `transync-wasm` layout rule — **not in the commit**: `CLAUDE.md` is excluded
  by the owner's machine-global gitignore, so it is maintained on disk only)
- Tickets: **`92ac61b9`** (the `rlib` fat-LTO cost, filed before this record)
  and **`0c6a5c8b`** (the demo's uncovered non-fatal error-strip path)

**Deliberately not rewritten:** dated snapshots describing pre-wave state — the
bodies of earlier DCRs, the resolved-issue blocks in `open-issues.md`, and the
spec's and plan's original sentences under `docs/superpowers/`. Corrections
there are **appended dated notes**, following the precedent DCR-0017, DCR-0018,
and DCR-0019 set.

### Discriminating tests

`transync-wasm::engine` — `render_pair_round_trips_a_translated_fixture` and
`rebuild_applies_a_payload_and_view_mode_agrees` are the pair that matters:
they pin that the two modes agree **byte-for-byte** on the same inputs, which
is the host half of the parity claim Playwright test `b` makes structurally.
`schema_version_is_the_wire_constant` is the lockstep leg. The four error-path
tests (`bad_alignment_json_…`, `bad_payloads_and_statuses_json_…`,
`unknown_block_id_in_payloads_…`, `unknown_block_id_in_statuses_…`) plus
`empty_source_is_an_empty_pair_not_a_panic` are what make `panic = "abort"`
defensible rather than optimistic.

Playwright — `b` (CLI-vs-browser parity as a standing gate), `a` (zero console
complaints, proven non-vacuous by the MIME negative control), `c` (the anchor
set survives a live rebuild, and the source pane does not move when a target
payload changes), `d` (both fail-closed gates mount **zero** anchors).

Unchanged-by-design suites that prove this wave added rather than moved: all 21
scenario suites, `public_surface`, `sync_js_drift`, `docs_index_drift`, the 19
`cli_smoke` subprocess tests, and the eight SCN-13 browser tests.

## Migration / Follow-up

Nothing to migrate. The wave is additive: no public API moved, no wire field
changed, no output byte differs, and a consumer that never runs
`scripts/build-wasm.sh` is unaffected. Two consumer-visible notes:

- **`comrak` now builds with `default-features = false` workspace-wide.** A
  downstream crate that was relying on transync's dependency to enable comrak's
  `cli`/`syntect` features for it must declare them itself. Behavior across
  every test binary was verified identical.
- **Building the demo needs three host prerequisites** — the
  `wasm32-unknown-unknown` rustup target, wasm-pack, and binaryen ≥ 121 — and a
  dependency resolution satisfying the `=0.2.126` pin. `scripts/smoke.sh` now
  fails on their absence rather than skipping.

Follow-ups deliberately left open:

- **The demo's non-fatal error-strip path has no committed browser coverage.**
  The path is implemented and smoke-proven (inject a ghost `source_block_id`
  into `alignment.json`, `rebuild` throws, the strip appears, the panes stay
  mounted, `data-demo-state` stays `ready`), but adding it would have been a
  fifth test against a brief that pinned the suite at 12. **Filed as ticket
  `0c6a5c8b`** with the reproduction recipe and a negative-control step; test
  `c` covers only the strip's *hidden* state.
  *(2026-08-05, ticket resolved: `wasm.spec.js` gained test `e` and the suite
  is now **13/13** — 8 SCN-13 + 5 wasm. It runs the filed recipe: one
  `alignment.json` row's `source_block_id` rewritten to `zz-9999` over
  `page.route`, so the edit model carries a ghost and the debounced `rebuild`
  throws. Asserted: the strip is VISIBLE and names the ghost id, the panes are
  the last good render (same id sequences, same anchor count, the edited
  block's pre-edit text), `data-demo-state` is still `ready`, the status strip
  still reads its pre-edit line, and `pageerror` is empty. Because the
  `structure_warning` above shares this strip, the test uses a same-topology
  payload and asserts the strip is the rejection rather than the warning —
  which is announced only after a rebuild that already re-mounted. One boot
  cost is pinned rather than hidden: the renamed row no longer pairs with its
  block, so `render_fragment` skips it and the ghosted block is absent from
  both panes; every later block keeps its anchor. Negative control: making
  `applyEdit`'s catch swallow the error failed exactly test `e`, on the
  strip-visible assertion; the file was restored from a byte-level backup and
  `cmp` confirmed byte-identity.)*
- **Ticket `92ac61b9`** — dropping `rlib` from `crate-type` recovers 113,844 B
  of fat-LTO scope. It touches the `cargo doc` / `cargo test` gates, so it is a
  deliberate follow-up rather than a build-script change.
  *(2026-08-05: taken. `crate-type = ["cdylib"]`, −101,228 B raw / −46,758 B
  gzip re-measured at the crate's current size, no gate lost; the budget was
  re-tightened to match. See the dated note under the attribution table.)*
- **OI-0020 is sharper, not merely still open.** The `=0.2.126` pin now sits on
  a critical build path, and the resolution that satisfies it lives only in an
  uncommitted lockfile. See ADR-0019 decision 2 for the failure mode and the
  recovery.
  *(2026-08-06, superseded: the lock was committed after all — OI-0020 is
  RESOLVED while the `=0.2.126` pin stays; see ADR-0019's 2026-08-07
  amendment.)*
- **The demo carries a third `KNOWN_SCHEMA` mirror** alongside both `sync.js`
  copies. It is double-checked and fails closed, but the mirror count is a
  standing lockstep obligation on any future alignment-schema bump.
  *(2026-08-05: the mirror is now also pinned at cargo-test time —
  `sync_js_drift.rs::wasm_demo_js_known_schema_matches_the_rust_alignment_schema_version`
  — so a bump can no longer land green on `cargo test` and fail only in the
  browser.)*
- ***(2026-08-05)* Topology-changing payload edits degrade unannounced.**
  `rebuild_impl` carries none of the pipeline's validation layers, so a payload
  that changes a block's topology overruns `render_fragment`'s zip: the block
  degrades to escaped byte-range text and later same-label entries can pair
  with the wrong node. Safe and recoverable, out of scope per spec §9 — but
  silent, untested, and it makes `render.rs`'s Guard-2 "unreachable by
  construction" comment stale. **Filed as ticket `2a276cb4`** (the fix touches
  the frozen `transync-syntax` comment, so it could not ride this wave).
  *(2026-08-05, ticket resolved: `rebuild` now also returns
  `structure_warning` — a nullable fifth field on the edit-mode result,
  additive to the boundary table above — set when the regenerated document's
  normalized top-level count differs from the source document's. It stays a
  warning: the panes still render and the block stays editable, and
  `wasm-demo.js` announces it on the non-fatal `#error-strip`. The Guard-2
  comment in `render.rs` now names this rebuild path as its real caller and
  describes the degrade.)*
  *(2026-08-08, R0002-0014 — the count comparison above was necessary and not
  sufficient. A payload that swaps a block for a different KIND of block, one
  for one, leaves the count identical and still overruns the zip: `"EDITED"`
  typed over the blockquote of `> quoted text` + `second paragraph` renders
  the edited text under the FOLLOWING paragraph's anchor and drops that
  paragraph's own content from the pane — with no warning, which is the hole
  in the promise ticket `2a276cb4` shipped. `check_top_level_structure` now
  compares the whole normalized fingerprint: top-level count, label sequence,
  and each collapsed list's item count — the same three facts
  `validate::full_reparse` compares on the pipeline side, projected off
  `walk::normalize_top_level` so the normalization rule keeps its single home.
  The count message is unchanged, the two new cases name the offending
  1-based block position, and the posture is still warn-never-reject.)*
- **Console cleanliness is now load-bearing.** Any future `console.warn` on the
  demo's happy path fails Playwright test `a`. That is intended — it is what
  makes the MIME guard real — but it constrains adding diagnostics to
  `wasm-demo.js`.
- **`bootDemo`'s 20 s readiness ceiling** in the spec is sized for a cold
  fetch + compile + instantiate of a 1.7 MB module (observed ~90 ms warm). If
  the module grows materially, that is the number to revisit alongside the
  budget.
- **Playwright test `b` depends on the bundle emitting `source.html` /
  `target.html`** — deliberately, since those files are its oracle. If the
  bundle ever stops shipping them, the reference must be regenerated some
  other way.
