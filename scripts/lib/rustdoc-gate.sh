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
# What keeps it total is the completeness check in scripts/smoke.sh: a
# member that grows a `src/lib.rs` and is not named here fails the smoke run
# instead of quietly sitting outside the gate. That check reads this array,
# so it covers the hook's invocation as well as smoke's.
RUSTDOC_GATE_CRATES=(transync-html transync-syntax transync-core transync transync-openai transync-anthropic transync-wasm)

# The same list as `cargo doc` package arguments: -p a -p b … — built here so
# neither caller repeats the loop.
RUSTDOC_GATE_ARGS=()
for _transync_rustdoc_crate in "${RUSTDOC_GATE_CRATES[@]}"; do
  RUSTDOC_GATE_ARGS+=(-p "$_transync_rustdoc_crate")
done
unset _transync_rustdoc_crate
