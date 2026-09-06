# transync — the standing rustdoc gate's crate lists (DCR-0018).
#
# Sourced (never executed) by the two scripts that run its `cargo doc` legs
# under `RUSTDOCFLAGS="-D warnings"`: scripts/smoke.sh and
# scripts/hooks/pre-commit. One home on purpose. The library list was written
# by hand at DCR-0018 and widened by hand at DCR-0020, and `transync-openai`
# was missed both times (ti 000e5a) — long enough for its public `client`
# module doc to carry eight intra-doc links to private items unnoticed. A
# second copy in the hook would be a third chance to miss one.
#
# THE LIBRARY LIST. Every workspace member that has a library target, spelled
# out rather than derived so it stays readable. `transync-cli` is not on it
# because it has no library target — it rides the bin-only list below, on its
# own leg.
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
# or RUSTDOC_GATE_UNGATED_BIN means and prints its own message.
RUSTDOC_GATE_CRATES=(transync-lang transync-html transync-syntax transync-core transync transync-openai transync-anthropic transync-wasm)

# The same list as `cargo doc` package arguments: -p a -p b … — built here so
# neither caller repeats the loop.
RUSTDOC_GATE_ARGS=()
for _transync_rustdoc_crate in "${RUSTDOC_GATE_CRATES[@]}"; do
  RUSTDOC_GATE_ARGS+=(-p "$_transync_rustdoc_crate")
done

# THE BIN-ONLY LIST. Every workspace member with no library target — today
# `transync-cli` alone.
#
# It used to be absent from this file entirely, and the comment said why:
# "bin-only, with no public API to document". That is true and beside the
# point. `--document-private-items` is how a developer reads this crate, and
# that reader is the one the intra-doc links are for. Nothing here checked
# them — clippy does not read intra-doc links at all — so they rotted
# unobserved and a refactor in one file broke links in a file it never
# opened: nine were measured on 2026-09-04 during OI-0043's `output.rs`
# split, FOUR of them in `crates/transync-cli/src/output/lock.rs`, whose
# `super::X` references had silently become sibling references when the
# concerns moved out of `output.rs`. A tenth was latent behind
# `#[cfg(not(unix))]` and would have surfaced only on a Windows build.
#
# A SECOND leg rather than more entries in the list above, because the flag
# does not compose with it: `--document-private-items` documents the private
# items, which is precisely what stops `rustdoc::private_intra_doc_links`
# from firing — the failure DCR-0018 built the library leg to catch, and the
# one that put eight dead links in `transync-openai`'s `client` doc. Folding
# the two into one invocation would widen this gate by disarming that one.
RUSTDOC_GATE_BIN_CRATES=(transync-cli)

# The bin-only leg's whole argument list, its flag included, so a caller runs
# `cargo doc --no-deps "${RUSTDOC_GATE_BIN_ARGS[@]}"` and this file stays the
# one place that decides what that leg is.
RUSTDOC_GATE_BIN_ARGS=(--document-private-items)
for _transync_rustdoc_crate in "${RUSTDOC_GATE_BIN_CRATES[@]}"; do
  RUSTDOC_GATE_BIN_ARGS+=(-p "$_transync_rustdoc_crate")
done

# THE COMPLETENESS CHECK. Every member directory holding a `src/lib.rs` has
# to be named in the library list, and every other member holding a
# `src/main.rs` in the bin-only list; anything else is a target sitting
# outside the gate. Package name == directory name for every member here; a
# rename that breaks that shows up as a `cargo doc` "package not found",
# which is the loud failure we want either way.
#
# `src/lib.rs` is asked FIRST and answers alone. A member carrying both
# targets is already covered by the library leg — `cargo doc -p X` documents
# every documentable target of X — and must not be moved onto the leg that
# disarms `private_intra_doc_links` merely because it also ships a binary.
#
# Membership is matched against each list joined into one space-delimited
# string. Crate names cannot contain spaces, and a `case` whose matching
# branch is empty cannot return non-zero — which an arithmetic test at the
# tail of a sourced file can, and that is the whole hazard above. The
# `if`/`elif` pair is safe for the same reason: with no `else`, a member that
# is neither a library nor a binary leaves it returning zero.
#
# The tree is located from THIS file rather than from the caller's cwd:
# smoke sources it by absolute path and the hook by a relative one, so a
# cwd-relative glob would make the two callers disagree.
RUSTDOC_GATE_UNGATED=()
RUSTDOC_GATE_UNGATED_BIN=()
_transync_rustdoc_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." 2>/dev/null && pwd)"
# Both callers cd to the repository root before sourcing, so the cwd is the
# honest fallback if this file is ever reached by a path that does not sit
# two levels below it.
[ -d "$_transync_rustdoc_root/crates" ] || _transync_rustdoc_root="."
_transync_rustdoc_listed=" ${RUSTDOC_GATE_CRATES[*]} "
_transync_rustdoc_listed_bin=" ${RUSTDOC_GATE_BIN_CRATES[*]} "
for _transync_rustdoc_manifest in "$_transync_rustdoc_root"/crates/*/Cargo.toml; do
  # Parameter expansion, not `dirname`/`basename`: this runs on every commit,
  # and two forks per member is most of what the check costs.
  _transync_rustdoc_dir="${_transync_rustdoc_manifest%/Cargo.toml}"
  _transync_rustdoc_member="${_transync_rustdoc_dir##*/}"
  if [ -f "$_transync_rustdoc_dir/src/lib.rs" ]; then
    case "$_transync_rustdoc_listed" in
    *" $_transync_rustdoc_member "*) ;;
    *) RUSTDOC_GATE_UNGATED+=("$_transync_rustdoc_member") ;;
    esac
  elif [ -f "$_transync_rustdoc_dir/src/main.rs" ]; then
    case "$_transync_rustdoc_listed_bin" in
    *" $_transync_rustdoc_member "*) ;;
    *) RUSTDOC_GATE_UNGATED_BIN+=("$_transync_rustdoc_member") ;;
    esac
  fi
done

unset _transync_rustdoc_crate _transync_rustdoc_root _transync_rustdoc_listed \
  _transync_rustdoc_listed_bin _transync_rustdoc_manifest _transync_rustdoc_dir \
  _transync_rustdoc_member
