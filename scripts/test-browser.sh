#!/usr/bin/env bash
# transync — SCN-13 headless browser suite.
#
# Automates SCN-13 (block-level dual-pane scroll sync), the product's
# defining browser scenario, with a small Playwright suite. Builds the
# stub CLI, regenerates a real HTML bundle from the SCN-14 fixture, and
# runs the suite against it (Chromium, headless), served by `transync
# serve` on loopback — the shipped server, since ti b791d6.
#
# The fixture is regenerated on every run so the suite never depends on a
# stale bundle. Override the output dir with TRANSYNC_FIXTURE_WORKDIR.
#
# It then assembles the Track C wasm demo (page + glue + module + the two
# Markdown payloads) on top of that bundle, inside the fixture dir only —
# see the "WASM demo leg" block below for the import-path layout.
#
# TRACE: SCN-13
# TRACE: OI-0023
# TRACE: ADR-0019

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="$REPO_ROOT/web"

# Deletion guard for the env-overridable fixture workdir — see the file
# for the rule (temp-root containment, a not-yet-existing path, or our own
# marker file; never a mere 'transync-' basename).
# shellcheck source-path=SCRIPTDIR source=lib/workdir-guard.sh
source "$REPO_ROOT/scripts/lib/workdir-guard.sh"
# Where a scratch workdir goes — one rule for all five scripts (ti `13a73b`).
# shellcheck source-path=SCRIPTDIR source=lib/workdir.sh
source "$REPO_ROOT/scripts/lib/workdir.sh"

# The fixture bundle lives under html/. This script always passes the
# resulting path to Playwright as TRANSYNC_FIXTURE_DIR, so it does not rely
# on playwright.config.js's default (which is repository-relative and exists
# for bare `pnpm exec playwright test` runs, not for this wrapper).
WORKDIR="$(transync_pick_workdir TRANSYNC_FIXTURE_WORKDIR /Volumes/Temp/claude/transync-browser-fixture)"
transync_guard_workdir test-browser TRANSYNC_FIXTURE_WORKDIR "$WORKDIR"

HTML_OUT="$WORKDIR/html"
INPUT="$REPO_ROOT/crates/transync/tests/fixtures/scn-14-full.md"

if [[ ! -f "$INPUT" ]]; then
  echo "[test-browser] input fixture not found: $INPUT" >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "[test-browser] pnpm is required (workspace convention). Install it and re-run." >&2
  exit 1
fi

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
transync_mark_workdir "$WORKDIR"

cd "$REPO_ROOT"

# One build, two uses. The fixture below is generated with this binary, and
# (ti b791d6) the Playwright `webServer` serves it with the same one — so the
# SCN-13 browser evidence covers the server transync actually ships instead of
# a stand-in. Resolving the path out of cargo's own JSON rather than guessing
# `target/debug/…` keeps working under a redirected CARGO_TARGET_DIR, and it is
# what keeps the repoint from being a source of flake: nothing compiles inside
# Playwright's webServer startup timeout, because the binary already exists by
# the time Playwright runs.
echo "[test-browser] building the stub CLI"
BUILD_LOG="$WORKDIR/cargo-build.jsonl"
cargo build -p transync-cli --features test-stub-provider \
  --message-format=json-render-diagnostics > "$BUILD_LOG"
TRANSYNC_BIN="$(sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' "$BUILD_LOG" | tail -n 1)"
if [[ -z "$TRANSYNC_BIN" || ! -x "$TRANSYNC_BIN" ]]; then
  echo "[test-browser] FAIL: could not resolve the transync binary from $BUILD_LOG" >&2
  exit 1
fi

echo "[test-browser] regenerating fixture -> $HTML_OUT"
"$TRANSYNC_BIN" translate \
  --input "$INPUT" \
  --output "$WORKDIR/out.md" \
  --map "$WORKDIR/out.json" \
  --html-out "$HTML_OUT" \
  --target-language ko

for path in \
  "$HTML_OUT/index.html" \
  "$HTML_OUT/source.html" \
  "$HTML_OUT/target.html" \
  "$HTML_OUT/alignment.json" \
  "$HTML_OUT/sync.js" \
  "$HTML_OUT/purify.min.js"
do
  if [[ ! -s "$path" ]]; then
    echo "[test-browser] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

# --- WASM demo leg (Track C, ADR-0019) ------------------------------------
#
# The wasm demo is NOT part of the CLI's six-file `--html-out` bundle
# contract; it is assembled on top of the generated bundle, inside this
# scratch fixture dir only. Nothing here touches HTML_BUNDLE_ENTRIES.
#
# Import-path layout (decided in Track C task 4). `web/js/wasm-demo.js`
# imports the glue as `../wasm/transync_wasm.js`, and the wasm-bindgen glue
# resolves its own module as a sibling of itself
# (`new URL('transync_wasm_bg.wasm', import.meta.url)`). So the fixture
# mirrors the repo's `web/` shape exactly:
#
#   $HTML_OUT/demo-wasm.html        <- web/demo-wasm.html
#   $HTML_OUT/js/wasm-demo.js       <- web/js/wasm-demo.js
#   $HTML_OUT/js/sync.js            <- web/js/sync.js       (page uses ./js/sync.js)
#   $HTML_OUT/wasm/transync_wasm*   <- web/wasm/            (glue + module, same dir)
#   $HTML_OUT/vendor/purify.min.js  <- web/vendor/          (page uses vendor/…)
#
# The bundle's own flat `sync.js` / `purify.min.js` stay where the CLI put
# them for `index.html`; these are additional workspace-path copies for
# `demo-wasm.html`, which uses the `web/` paths rather than the bundle-flat
# ones.
echo "[test-browser] building the wasm demo module"
"$REPO_ROOT/scripts/build-wasm.sh"

mkdir -p "$HTML_OUT/js" "$HTML_OUT/wasm" "$HTML_OUT/vendor"
cp "$WEB_DIR/demo-wasm.html" "$HTML_OUT/demo-wasm.html"
cp "$WEB_DIR/js/wasm-demo.js" "$HTML_OUT/js/wasm-demo.js"
cp "$WEB_DIR/js/sync.js" "$HTML_OUT/js/sync.js"
cp "$WEB_DIR/vendor/purify.min.js" "$HTML_OUT/vendor/purify.min.js"
cp "$REPO_ROOT/web/wasm/transync_wasm.js" "$HTML_OUT/wasm/transync_wasm.js"
cp "$REPO_ROOT/web/wasm/transync_wasm_bg.wasm" "$HTML_OUT/wasm/transync_wasm_bg.wasm"
# The two Markdown payloads the demo renders from — absent from the bundle
# by design (it ships rendered HTML, not Markdown).
cp "$INPUT" "$HTML_OUT/source.md"
cp "$WORKDIR/out.md" "$HTML_OUT/out.md"

for path in \
  "$HTML_OUT/demo-wasm.html" \
  "$HTML_OUT/js/wasm-demo.js" \
  "$HTML_OUT/js/sync.js" \
  "$HTML_OUT/vendor/purify.min.js" \
  "$HTML_OUT/wasm/transync_wasm.js" \
  "$HTML_OUT/wasm/transync_wasm_bg.wasm" \
  "$HTML_OUT/source.md" \
  "$HTML_OUT/out.md"
do
  if [[ ! -s "$path" ]]; then
    echo "[test-browser] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

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

cd "$WEB_DIR"

if [[ -f "pnpm-lock.yaml" ]]; then
  echo "[test-browser] pnpm install --frozen-lockfile"
  pnpm install --frozen-lockfile
else
  echo "[test-browser] pnpm install"
  pnpm install
fi

# Idempotent — a no-op once the browser is cached; downloads it otherwise.
echo "[test-browser] ensuring Chromium is installed"
pnpm exec playwright install chromium

echo "[test-browser] running Playwright suite against $HTML_OUT (served by transync serve)"
TRANSYNC_SERVE_BIN="$TRANSYNC_BIN" TRANSYNC_FIXTURE_DIR="$HTML_OUT" \
  pnpm exec playwright test "$@"

echo "[test-browser] OK — SCN-13 headless suite passed"
