#!/usr/bin/env bash
# transync — build the Track C wasm demo module (ADR-0019).
#
# Emits web/wasm/{transync_wasm.js,transync_wasm_bg.wasm} (gitignored build
# output) for the browser demo shell to import.
#
# Publication is staged, never in place (R0003-0007, R0003-0008, R0003-0081).
# Everything — the wasm-pack build, the wasm-opt pass, the size measurement and
# both budget checks — happens in a per-run staging directory; web/wasm is only
# then replaced, as a directory, by rename. Three properties follow, and each
# one was missing while the copy came first:
#   * a build that fails a gate leaves the previously good module in place
#     instead of installing the module it just rejected;
#   * glue and module land as one version-coupled pair, never a mixed one a
#     browser tab or a parallel Playwright run can observe halfway;
#   * web/wasm holds exactly the set THIS build emitted, so an artifact a past
#     build wrote under a name this one no longer produces does not linger.
# The staging directories carry a per-run token, so two concurrent runs (this
# repo does run concurrent agent sessions) cannot read or delete each other's
# half-built module (R0003-0083).
#
# Prereqs fail loudly, never skip — this is a shipped artifact, not an
# optional linter:
#   * wasm-pack
#   * a binaryen wasm-opt >= 121. wasm-pack 0.15 caches binaryen 117, which
#     rejects rustc 1.97 output ("Bulk memory operations require bulk memory
#     [--enable-bulk-memory]"), so we build with --no-opt and invoke the
#     PATH wasm-opt ourselves with the feature flags spelled out.
#
# The custom `wasm-release` profile is also why --no-opt is mandatory rather
# than merely preferable: wasm-pack only honours
# [package.metadata.wasm-pack.profile.{dev,release,profiling}] and silently
# ignores the key for a user-defined profile.
#
# Never sets CARGO_TARGET_DIR or --target-dir: wasm-pack shells out to cargo
# and already respects the machine-global target dir.
#
# TRACE: ADR-0019

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WEB_DIR="$REPO_ROOT/web"
OUT_DIR="$WEB_DIR/wasm"

# Deletion guard for the env-overridable staging dir — see the file for the
# rule (temp-root containment, a not-yet-existing path, or our own marker
# file; never a mere 'transync-' basename).
# shellcheck source-path=SCRIPTDIR source=lib/workdir-guard.sh
source "$REPO_ROOT/scripts/lib/workdir-guard.sh"

# Raw / gzip ceilings for the emitted module, owner-approved 2026-08-05.
# The module measures 1,639,520 B raw / 671,795 B gzip today; the ceilings sit
# ~12 % (raw) / ~13 % (gzip) above that — the relative headroom the previous
# pair carried, re-applied to the smaller module. A breach is a design signal,
# not a number to bump reflexively.
#
# These replace the spec's provisional 1,400,000 / 500,000, which came from
# an exploration probe that compiled only parse -> alignment map -> render.
# The shipped crate also compiles edit mode, and `regen::regenerate` +
# `outcome::html_outcomes` pull in Markdown *reserialization* — measured at
# +435,770 B on an otherwise identical probe, which the view-only baseline
# could not see. The JSON boundary itself is cheap (+42,127 B). The one
# recoverable slice in that attribution, the fat-LTO scope lost to
# transync-wasm's `rlib` crate-type sitting beside the `cdylib`, was SPENT on
# 2026-08-05 (ticket 92ac61b9): dropping `rlib` returned 101,228 B raw /
# 46,758 B gzip, which is why these ceilings dropped with it. The design spec
# and plan carry dated corrections recording the pre-drop numbers.
MAX_RAW=1840000
MAX_GZ=760000

# The staging ROOT — the directory this run creates its own private staging
# tree inside. It is still what TRANSYNC_WASM_STAGING names and still what the
# deletion guard is asked about, so pointing the override at a valuable
# directory is refused exactly as before; what changed is that the root is no
# longer wiped wholesale at start (that is what made two concurrent runs delete
# each other's build — R0003-0083).
STAGING_ROOT="${TRANSYNC_WASM_STAGING:-/Volumes/Temp/claude/transync-wasm-pkg}"
if [[ ! -d "$(dirname "$STAGING_ROOT")" ]]; then
  STAGING_ROOT="${TMPDIR:-/tmp}/transync-wasm-pkg"
fi

transync_guard_workdir build-wasm TRANSYNC_WASM_STAGING "$STAGING_ROOT"

if ! command -v wasm-pack >/dev/null 2>&1; then
  echo "[build-wasm] wasm-pack not found — install it (brew install wasm-pack) and re-run." >&2
  exit 1
fi

# Resolve wasm-opt once, by absolute path, so the invocation below can never
# drift onto wasm-pack's cached binaryen 117 in ~/Library/Caches/.wasm-pack/.
WASM_OPT="$(command -v wasm-opt || true)"
if [[ -z "$WASM_OPT" ]]; then
  echo "[build-wasm] wasm-opt not found — install binaryen >= 121 (brew install binaryen) and re-run." >&2
  echo "[build-wasm] wasm-pack's own cached binaryen is NOT a substitute: it is 117 and rejects current rustc output." >&2
  exit 1
fi

# Anchor on the "version <n>" token binaryen's banner prints ("wasm-opt version
# 131 (version_131)") rather than on the first digits anywhere in the output: a
# vendor banner leading with a year ("binaryen 2024 … version 117") would
# otherwise be read as the version and pass a gate it fails (R0003-0082). A
# misfire is caught downstream — wasm-opt rejects the module, or the size gate
# trips — so this is diagnostics, which is also why the raw output is echoed
# verbatim when nothing parses.
# `|| true`: a wasm-opt whose --version carries no such token must reach the
# diagnostic below, not die silently on grep's exit 1 under `set -e`. stderr is
# folded in because a banner printed there is evidence too.
OPT_VER_RAW="$("$WASM_OPT" --version 2>&1 || true)"
OPT_VER="$(printf '%s\n' "$OPT_VER_RAW" | grep -oE 'version[[:space:]]+[0-9]+' | head -1 | grep -oE '[0-9]+' || true)"
if [[ ! "$OPT_VER" =~ ^[0-9]+$ ]]; then
  echo "[build-wasm] could not parse a 'version <n>' from '$WASM_OPT --version' — need binaryen >= 121. It printed:" >&2
  printf '%s\n' "$OPT_VER_RAW" >&2
  exit 1
fi
if (( OPT_VER < 121 )); then
  echo "[build-wasm] wasm-opt $OPT_VER is too old (need >= 121; binaryen < 121 rejects current rustc bulk-memory output)." >&2
  echo "[build-wasm] upgrade it (brew upgrade binaryen) and re-run. Found: $WASM_OPT" >&2
  exit 1
fi

cd "$REPO_ROOT"

mkdir -p "$STAGING_ROOT"
transync_mark_workdir "$STAGING_ROOT"

# Reclaim the staging trees of a crashed predecessor whose pid the OS handed
# back to us — and only those. The pid is in the name for exactly this reason:
# a blind sweep of the root would delete a concurrently running build's tree,
# which is the failure this per-run layout exists to prevent. This is the rule
# `crates/transync-cli/src/output.rs` publishes by (R0002-0001), transcribed.
for stale in "$STAGING_ROOT/run.$$."*; do
  if [[ -e "$stale" ]]; then
    rm -rf "$stale"
  fi
done

# `mktemp -d` creates the directory whose name it prints. That creation is what
# authorizes removing it later; the name alone never is.
STAGING="$(mktemp -d "$STAGING_ROOT/run.$$.XXXXXXXXXX")"

# The publish staging is a SIBLING of web/wasm, so the swap below is a rename
# inside one directory — one filesystem, cheap metadata op, no half-copied
# directory ever visible under the published name.
PUBLISH_STAGING="$(mktemp -d "$WEB_DIR/.wasm.staging.XXXXXXXXXX")"
PUBLISH_BACKUP="$WEB_DIR/.wasm.backup.${PUBLISH_STAGING##*.}"
# This directory BECOMES web/wasm, so it has to carry the mode `mkdir -p` would
# have given it. `mktemp -d` makes 0700 — correct for a scratch dir, wrong for
# a directory a local static server reads the demo module out of.
chmod "$(printf '%04o' "$(( 0777 & ~0$(umask) ))")" "$PUBLISH_STAGING"

# The swap at the end is serialized across concurrent builds, because `mv`
# moves a directory INTO an existing destination instead of replacing it: two
# interleaved swaps would nest one run's staging inside the other's published
# web/wasm. `mkdir` is the atomic test-and-set every filesystem provides. The
# lock is held for two renames — milliseconds — and released by the EXIT trap.
PUBLISH_LOCK="$WEB_DIR/.wasm.publish.lock"
LOCK_HELD=0

# Every path this removes, this run created (both mktemp -d, the lock's mkdir)
# or moved aside itself (the backup). Nothing is deleted because of how it is
# named.
cleanup() {
  rm -rf "$STAGING" || true
  if [[ -e "$PUBLISH_BACKUP" ]]; then
    if [[ -e "$OUT_DIR" ]]; then
      rm -rf "$PUBLISH_BACKUP" || true
    else
      # Killed mid-swap: the moved-aside module is the only one left. Put it
      # back rather than leaving the demo with no module at all.
      mv "$PUBLISH_BACKUP" "$OUT_DIR" || true
    fi
  fi
  rm -rf "$PUBLISH_STAGING" || true
  if (( LOCK_HELD )); then
    rmdir "$PUBLISH_LOCK" || true
  fi
}
trap cleanup EXIT
# Route signals through `exit` so the EXIT trap above is what actually cleans up.
trap 'exit 130' INT
trap 'exit 143' TERM

echo "[build-wasm] wasm-pack build --target web --profile wasm-release --no-opt -> $STAGING"
wasm-pack build crates/transync-wasm \
  --target web \
  --profile wasm-release \
  --no-opt \
  --no-pack \
  --out-dir "$STAGING"

GLUE="$STAGING/transync_wasm.js"
MODULE="$STAGING/transync_wasm_bg.wasm"
for path in "$GLUE" "$MODULE"; do
  if [[ ! -s "$path" ]]; then
    echo "[build-wasm] FAIL: wasm-pack did not emit $(basename "$path"). Staging contains:" >&2
    ls -1 "$STAGING" >&2
    exit 1
  fi
done

# Explicit optimization pass. The --enable-* flags name the proposals rustc
# 1.97 and wasm-bindgen 0.2.126 emit by default; binaryen validates the input
# against them before it will touch the module.
echo "[build-wasm] wasm-opt -Oz (binaryen $OPT_VER at $WASM_OPT)"
"$WASM_OPT" -Oz \
  --enable-bulk-memory \
  --enable-reference-types \
  --enable-nontrapping-float-to-int \
  --enable-sign-ext \
  --enable-mutable-globals \
  "$MODULE" -o "$MODULE.opt"
mv "$MODULE.opt" "$MODULE"

# Measured in staging, BEFORE anything is published: a module that breaches the
# budget must not replace a good one on its way to exiting 1 (R0003-0008).
RAW=$(( $(wc -c < "$MODULE") ))
GZ=$(( $(gzip -9 -c "$MODULE" | wc -c) ))
echo "[build-wasm] size: raw=${RAW}B gzip=${GZ}B (budget: raw<=${MAX_RAW}B gzip<=${MAX_GZ}B)"

if (( RAW > MAX_RAW )); then
  echo "[build-wasm] FAIL: raw size $RAW exceeds the $MAX_RAW budget — $OUT_DIR left untouched." >&2
  exit 1
fi
if (( GZ > MAX_GZ )); then
  echo "[build-wasm] FAIL: gzip size $GZ exceeds the $MAX_GZ budget — $OUT_DIR left untouched." >&2
  exit 1
fi

# Assemble the published set — exactly the pair the demo imports, not whatever
# else wasm-pack left in staging — and re-check it where it will be published
# from, so the rename below moves a directory that was verified as a whole.
cp "$GLUE" "$MODULE" "$PUBLISH_STAGING/"
for path in "$PUBLISH_STAGING/transync_wasm.js" "$PUBLISH_STAGING/transync_wasm_bg.wasm"; do
  if [[ ! -s "$path" ]]; then
    echo "[build-wasm] FAIL: staged $(basename "$path") is missing or empty — $OUT_DIR left untouched." >&2
    exit 1
  fi
done

# Publish by rename. Crash-safe, not atomic, and for the same reason
# `publish_out_dir` documents for --out-dir: rename cannot replace a non-empty
# directory in place, so replacing an existing web/wasm is two renames with a
# window in which the directory is absent. A reader that looks into that window
# finds nothing — never a mixed or rejected pair, which is the property that
# was missing. The EXIT trap restores the moved-aside module if the second
# rename never happens.
waited=0
until mkdir "$PUBLISH_LOCK" 2>/dev/null; do
  if (( waited >= 30 )); then
    echo "[build-wasm] FAIL: $PUBLISH_LOCK has been held for ${waited}s. Either another build is publishing right now, or one was killed mid-swap — in the second case remove that directory by hand once no build is running." >&2
    exit 1
  fi
  sleep 1
  waited=$(( waited + 1 ))
done
LOCK_HELD=1

if [[ -e "$OUT_DIR" ]]; then
  mv "$OUT_DIR" "$PUBLISH_BACKUP"
fi
if ! mv "$PUBLISH_STAGING" "$OUT_DIR"; then
  echo "[build-wasm] FAIL: could not publish the built module into $OUT_DIR." >&2
  exit 1
fi
rm -rf "$PUBLISH_BACKUP"

echo "[build-wasm] OK — module + glue landed in $OUT_DIR"
