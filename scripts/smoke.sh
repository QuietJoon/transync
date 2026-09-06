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

# The completeness check that keeps that list honest moved into the library
# with it (OI-0046, finding R0009-0015): sourcing above walked crates/*/ and
# left every library member missing from RUSTDOC_GATE_CRATES in
# RUSTDOC_GATE_UNGATED. The check now covers the hook's invocation as well as
# this one, and the verdict is still smoke's to act on — a member that grows
# a src/lib.rs and is not named there fails this run instead of quietly
# sitting outside the gate.
if ((${#RUSTDOC_GATE_UNGATED[@]})); then
  echo "[smoke] FAIL: library member(s) outside the rustdoc gate: ${RUSTDOC_GATE_UNGATED[*]}" >&2
  echo "[smoke]   add them to RUSTDOC_GATE_CRATES in scripts/lib/rustdoc-gate.sh" >&2
  echo "[smoke]   (and to the gate command in docs/Developer_Guide.md and the" >&2
  echo "[smoke]   release checklist)." >&2
  exit 1
fi

# The same completeness question for the OTHER kind of member. A bin-only
# crate has no library target, so it can never appear in RUSTDOC_GATE_UNGATED
# above; the library fills RUSTDOC_GATE_UNGATED_BIN for it instead.
if ((${#RUSTDOC_GATE_UNGATED_BIN[@]})); then
  echo "[smoke] FAIL: bin-only member(s) outside the rustdoc gate: ${RUSTDOC_GATE_UNGATED_BIN[*]}" >&2
  echo "[smoke]   add them to RUSTDOC_GATE_BIN_CRATES in scripts/lib/rustdoc-gate.sh" >&2
  exit 1
fi

echo "[smoke] RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps ${RUSTDOC_GATE_ARGS[*]}"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps "${RUSTDOC_GATE_ARGS[@]}"

# A SECOND invocation, not more -p entries on the first: --document-private-items
# documents the private items, which stops `rustdoc::private_intra_doc_links`
# from firing — and that lint is the failure DCR-0018 built the library leg to
# catch. The two legs therefore need different flags and cannot share a run.
echo "[smoke] RUSTDOCFLAGS=\"-D warnings\" cargo doc --no-deps ${RUSTDOC_GATE_BIN_ARGS[*]}"
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps "${RUSTDOC_GATE_BIN_ARGS[@]}"

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
