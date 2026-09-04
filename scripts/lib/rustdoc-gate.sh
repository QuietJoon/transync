# transync — the standing rustdoc gate's crate list (DCR-0018).
#
# Sourced (never executed) by the two scripts that run
# `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over it:
# scripts/smoke.sh and scripts/hooks/pre-commit. One home on purpose. The
# list was written by hand at DCR-0018 and widened by hand at DCR-0020, and
# `transync-openai` was missed both times (ti 000e5a) — long enough for its
# public `client` module doc to carry eight intra-doc links to private items
# unnoticed. A second copy in the hook would be a third chance to miss one.
#
# THE LIST. Every workspace member that has a library target, spelled out
# rather than derived so it stays readable. `transync-cli` is absent on
# purpose — it is bin-only, with no public API to document.
#
# What keeps it total is the completeness check at the bottom of this file.
# That check used to live in scripts/smoke.sh alone (OI-0046, finding
# R0009-0015), which meant the gate the hook ran on EVERY commit was total
# only for as long as nobody added a library member without also running
# smoke. Both callers already source this file, so both now get the check
# from one definition, and neither has to repeat the discovery loop.
#
# NOTHING IN THIS FILE MAY `exit` OR END ON A NON-ZERO STATUS. Under
# `source` an `exit` terminates the CALLER — the hook would lose its
# JavaScript leg and its skip summary — and a sourced file whose last
# command returns non-zero aborts scripts/smoke.sh at its `source` line,
# because smoke runs `set -euo pipefail`. The check therefore only records
# what it found; each caller decides what a non-empty RUSTDOC_GATE_UNGATED
# means and prints its own message.
RUSTDOC_GATE_CRATES=(transync-lang transync-html transync-syntax transync-core transync transync-openai transync-anthropic transync-wasm)

# The same list as `cargo doc` package arguments: -p a -p b … — built here so
# neither caller repeats the loop.
RUSTDOC_GATE_ARGS=()
for _transync_rustdoc_crate in "${RUSTDOC_GATE_CRATES[@]}"; do
  RUSTDOC_GATE_ARGS+=(-p "$_transync_rustdoc_crate")
done

# THE COMPLETENESS CHECK. Every member directory holding a `src/lib.rs` has
# to be named above; anything else is a library sitting outside the gate.
# Package name == directory name for every member here; a rename that breaks
# that shows up as a `cargo doc` "package not found", which is the loud
# failure we want either way.
#
# Membership is matched against the list joined into one space-delimited
# string. Crate names cannot contain spaces, and a `case` whose matching
# branch is empty cannot return non-zero — which an arithmetic test at the
# tail of a sourced file can, and that is the whole hazard above.
#
# The tree is located from THIS file rather than from the caller's cwd:
# smoke sources it by absolute path and the hook by a relative one, so a
# cwd-relative glob would make the two callers disagree.
RUSTDOC_GATE_UNGATED=()
_transync_rustdoc_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd)"
# Both callers cd to the repository root before sourcing, so the cwd is the
# honest fallback if this file is ever reached by a path that does not sit
# two levels below it.
[ -d "$_transync_rustdoc_root/crates" ] || _transync_rustdoc_root="."
_transync_rustdoc_listed=" ${RUSTDOC_GATE_CRATES[*]} "
for _transync_rustdoc_manifest in "$_transync_rustdoc_root"/crates/*/Cargo.toml; do
  # Parameter expansion, not `dirname`/`basename`: this runs on every commit,
  # and two forks per member is most of what the check costs.
  _transync_rustdoc_dir="${_transync_rustdoc_manifest%/Cargo.toml}"
  _transync_rustdoc_member="${_transync_rustdoc_dir##*/}"
  [ -f "$_transync_rustdoc_dir/src/lib.rs" ] || continue
  case "$_transync_rustdoc_listed" in
  *" $_transync_rustdoc_member "*) ;;
  *) RUSTDOC_GATE_UNGATED+=("$_transync_rustdoc_member") ;;
  esac
done

unset _transync_rustdoc_crate _transync_rustdoc_root _transync_rustdoc_listed \
  _transync_rustdoc_manifest _transync_rustdoc_dir _transync_rustdoc_member
