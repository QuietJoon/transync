# transync — is the tree the pre-commit gates read the tree being committed?
#
# Sourced (never executed) by scripts/hooks/pre-commit, which calls
# `transync_commit_snapshot_guard` before any gate runs.
#
# Every gate the hook runs reads the WORKING TREE: cargo fmt, clippy, the
# wasm32 check and both rustdoc legs compile the files on disk, and biome
# reads the on-disk bytes of the files git lists. A commit records the
# INDEX. Where the two disagree, a green hook describes a tree nobody is
# committing (R0012-0016): `cargo fmt --all` without re-staging passes the
# format check and commits the unformatted blob, and a staged `mod foo;`
# whose foo.rs was never added compiles here and nowhere else.
#
# THE POLICY (owner route A of R0012-0016, DCR-0063).
#
#   Refused — the hook exits 1 before any gate runs, naming each path:
#     1. a gate input this commit changes whose working copy differs from
#        its staged copy (partially staged, or edited after staging);
#     2. an untracked gate input git does not ignore.
#   Warned — listed at the end of the hook's run; the commit proceeds:
#     3. a gate input this commit does not change whose working copy
#        differs from the index. The gates read that edit, so the verdict
#        covers a superset of the commit: a staged change that builds only
#        against such an edit passes here and does not build in the commit.
#
# Nothing else is consulted. No gate reads docs/, so a docs file staged and
# edited again changes no verdict and draws no word from the hook.
#
# Gate inputs are chosen by PATH, not by file extension, because the gates
# compile whatever the build reaches: include_str! pulls templates, the
# shipped profile, golden JSON and test fixtures from under crates/, and the
# hook that runs is the working copy of scripts/hooks/pre-commit. The list
# is TRANSYNC_GATE_INPUTS below. The JavaScript corpus is the one input
# chosen by extension, because the hook chooses it that way (`git ls-files
# '*.js' '*.mjs' '*.cjs'`); for the same reason an untracked .js outside
# crates/ is not an input — git does not list it, so biome never reads it.
#
# Every query is plain git porcelain, so GIT_INDEX_FILE is honoured:
# `git commit -a` and `git commit <paths>` hand the hook a temporary index,
# and the guard judges the commit that index describes.
#
# THE COST. The check is a few git queries — milliseconds, against a gate
# that takes minutes. What it costs in practice is turn-taking: two sessions
# editing the SAME gate-input file cannot commit it piecemeal. When one
# stages only its own hunks (`git add -p`, `git apply --cached`), the other
# session's hunks are still in the working copy, so rule 1 refuses — the
# hook cannot build the staged file the commit would carry. One session's
# change to that file has to be committed before the other's goes in.
#
# shellcheck shell=bash

# Pathspecs for the gate inputs, with explicit magic so a caller's
# GIT_*_PATHSPECS settings cannot change what they match.
TRANSYNC_GATE_INPUTS=(
  ':(glob)crates/**'
  ':(glob)Cargo.*'
  ':(glob)rust-toolchain*'
  ':(glob).cargo/**'
  ':(literal)rustfmt.toml'
  ':(literal).rustfmt.toml'
  ':(literal)clippy.toml'
  ':(literal).clippy.toml'
  ':(literal)biome.json'
  ':(literal)biome.jsonc'
  ':(literal)package.json'
  ':(literal)web/package.json'
  ':(literal)tsconfig.json'
  ':(literal)web/tsconfig.json'
  ':(glob)scripts/hooks/**'
  ':(glob)scripts/lib/**'
)
# The tracked JavaScript corpus — gate inputs while tracked, never untracked.
TRANSYNC_GATE_CORPUS=(
  ':(glob)**/*.js'
  ':(glob)**/*.mjs'
  ':(glob)**/*.cjs'
)

# The warned paths of the last guard run (rule 3), for the hook's summary.
TRANSYNC_SNAPSHOT_OUTSIDE=()

# git with every pathspec-interpreting override cleared, so the magic in the
# lists above means what it says.
_transync_snapshot_git() {
  (
    unset GIT_LITERAL_PATHSPECS GIT_GLOB_PATHSPECS GIT_NOGLOB_PATHSPECS GIT_ICASE_PATHSPECS
    git "$@"
  )
}

# transync_commit_snapshot_guard
#
# Returns 0 when every gate input the commit carries is the tree the gates
# will read, filling TRANSYNC_SNAPSHOT_OUTSIDE with rule-3 paths. Returns 1
# having printed a NOT CHECKABLE refusal (rules 1 and 2), or when git cannot
# answer — a guard that cannot run is not a pass.
transync_commit_snapshot_guard() {
  local staged unstaged untracked path
  local -a diverged=() not_tracked=()
  TRANSYNC_SNAPSHOT_OUTSIDE=()

  if ! staged="$(_transync_snapshot_git diff --cached --no-renames --name-only -- \
    "${TRANSYNC_GATE_INPUTS[@]}" "${TRANSYNC_GATE_CORPUS[@]}")" \
    || ! unstaged="$(_transync_snapshot_git diff --no-renames --name-only -- \
      "${TRANSYNC_GATE_INPUTS[@]}" "${TRANSYNC_GATE_CORPUS[@]}")" \
    || ! untracked="$(_transync_snapshot_git ls-files --others --exclude-standard -- \
      "${TRANSYNC_GATE_INPUTS[@]}")"; then
    echo "[pre-commit] FAIL: git could not compare the index with the working tree, so nothing shows the gates would check this commit" >&2
    return 1
  fi

  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    case $'\n'"$staged"$'\n' in
      *$'\n'"$path"$'\n'*) diverged+=("$path") ;;
      *) TRANSYNC_SNAPSHOT_OUTSIDE+=("$path") ;;
    esac
  done <<<"$unstaged"
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    not_tracked+=("$path")
  done <<<"$untracked"

  if ((${#diverged[@]} == 0 && ${#not_tracked[@]} == 0)); then
    return 0
  fi

  {
    echo "[pre-commit] NOT CHECKABLE: the index and the working tree disagree on gate inputs."
    echo "[pre-commit]   Every gate reads the working tree and the commit records the index,"
    echo "[pre-commit]   so no verdict here would describe this commit (R0012-0016)."
    if ((${#diverged[@]})); then
      echo "[pre-commit]   Staged, then changed again (\`git diff -- <path>\` shows the unstaged part):"
      for path in "${diverged[@]}"; do
        echo "[pre-commit]     $path"
      done
    fi
    if ((${#not_tracked[@]})); then
      echo "[pre-commit]   Untracked — the build reaches it and the commit would not carry it:"
      for path in "${not_tracked[@]}"; do
        echo "[pre-commit]     $path"
      done
    fi
    echo "[pre-commit]   Stage what belongs in this commit (\`git add <path>\`) and take the"
    echo "[pre-commit]   rest out of the working tree. A hunk that is another session's work"
    echo "[pre-commit]   cannot be committed around — see docs/Troubleshooting.md."
  } >&2
  return 1
}
