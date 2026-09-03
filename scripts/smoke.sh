#!/usr/bin/env bash
# transync — Phase-4 hard-gate smoke.
#
# In order: builds the workspace, runs the standing wasm32 gate, runs all
# unit + integration tests and the CLI stub suite, runs the rustdoc gate,
# builds the wasm demo module, and only then drives the CLI end-to-end
# against the SCN-14 fixture using the in-process echo translator (no API
# key needed).
#
# Host prerequisites beyond a Rust toolchain — the wasm32-unknown-unknown
# rustup target, wasm-pack, and binaryen wasm-opt >= 121 — fail the run
# loudly rather than skipping. This is the repository's hard gate, not a
# getting-started demo; docs/Quick_Start.md step 3 has the light path.
#
# TRACE: SCN-12
# TRACE: SL-12

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Deletion guard for the env-overridable workdir: temp-root containment,
# a not-yet-existing path, or our own marker file — never a mere
# 'transync-' basename. The rule lives in one place; read it there.
# shellcheck source-path=SCRIPTDIR source=lib/workdir-guard.sh
source "$REPO_ROOT/scripts/lib/workdir-guard.sh"
# Where a scratch workdir goes — one rule for all five scripts (ti `13a73b`).
# shellcheck source-path=SCRIPTDIR source=lib/workdir.sh
source "$REPO_ROOT/scripts/lib/workdir.sh"

WORKDIR="$(transync_pick_workdir TRANSYNC_SMOKE_WORKDIR /Volumes/Temp/claude/transync-smoke)"
transync_guard_workdir smoke TRANSYNC_SMOKE_WORKDIR "$WORKDIR"

rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
transync_mark_workdir "$WORKDIR"

cd "$REPO_ROOT"

echo "[smoke] cargo build --workspace"
cargo build --workspace

echo "[smoke] cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown"
cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown

# --test-threads=4: CARGO_TARGET_DIR lives on an external volume here, and
# dyld stalls when many cold test binaries start at once. Harmless elsewhere.
echo "[smoke] cargo test --workspace -- --test-threads=4"
cargo test --workspace -- --test-threads=4

echo "[smoke] cargo test -p transync-cli --features test-stub-provider -- --test-threads=4"
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4

# The rustdoc gate (DCR-0018) covers every workspace member that has a
# library target. Its crate list lives in one place, shared with
# scripts/hooks/pre-commit, which runs the same gate on every commit.
# shellcheck source-path=SCRIPTDIR source=lib/rustdoc-gate.sh
source "$REPO_ROOT/scripts/lib/rustdoc-gate.sh"

# The completeness check is what keeps that list honest, and it stays here:
# a member that grows a src/lib.rs and is not named there fails the run
# instead of quietly sitting outside the gate. ti 000e5a: the list was
# written by hand at DCR-0018 and widened by hand at DCR-0020, and
# transync-openai was left out both times — long enough for its public
# `client` module doc to carry eight intra-doc links to private items
# unnoticed. transync-cli is absent on purpose — it is bin-only, with no
# public API to document.
ungated=()
for manifest in "$REPO_ROOT"/crates/*/Cargo.toml; do
  member_dir="$(dirname "$manifest")"
  member="$(basename "$member_dir")"
  # Package name == directory name for every member here; a rename that
  # breaks that shows up as a `cargo doc` "package not found", which is
  # the loud failure we want either way.
  [[ -f "$member_dir/src/lib.rs" ]] || continue
  gated=0
  for crate in "${RUSTDOC_GATE_CRATES[@]}"; do
    if [[ "$crate" == "$member" ]]; then
      gated=1
      break
    fi
  done
  if ((gated == 0)); then
    ungated+=("$member")
  fi
done

if ((${#ungated[@]})); then
  echo "[smoke] FAIL: library member(s) outside the rustdoc gate: ${ungated[*]}" >&2
  echo "[smoke]   add them to RUSTDOC_GATE_CRATES in scripts/lib/rustdoc-gate.sh" >&2
  echo "[smoke]   (and to the gate command in docs/Developer_Guide.md and the" >&2
  echo "[smoke]   release checklist)." >&2
  exit 1
fi

echo "[smoke] RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps ${RUSTDOC_GATE_ARGS[*]}"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps "${RUSTDOC_GATE_ARGS[@]}"

# Builds the Track C demo module into web/wasm/ and enforces its size
# budget. Its prereqs (wasm-pack, binaryen >= 121) fail loudly rather than
# skipping — the module is a shipped artifact, not an optional linter.
echo "[smoke] scripts/build-wasm.sh"
"$REPO_ROOT/scripts/build-wasm.sh"

echo "[smoke] CLI dry run against SCN-14 fixture"
cargo run -p transync-cli --features test-stub-provider --quiet -- translate \
  --input "$REPO_ROOT/crates/transync/tests/fixtures/scn-14-full.md" \
  --output "$WORKDIR/out.md" \
  --map "$WORKDIR/out.json" \
  --html-out "$WORKDIR/html" \
  --target-language ko

for path in \
  "$WORKDIR/out.md" \
  "$WORKDIR/out.json" \
  "$WORKDIR/html/index.html" \
  "$WORKDIR/html/source.html" \
  "$WORKDIR/html/target.html" \
  "$WORKDIR/html/alignment.json" \
  "$WORKDIR/html/sync.js" \
  "$WORKDIR/html/purify.min.js"
do
  if [[ ! -s "$path" ]]; then
    echo "[smoke] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

echo "[smoke] OK — outputs landed in $WORKDIR"
