#!/usr/bin/env bash
# Install transync's tracked git hooks by pointing core.hooksPath at the
# repo's scripts/hooks directory (R0008-0058). Unlike a copy into
# .git/hooks, this keeps the hook under version control so a clean clone
# receives it after running this script once.
#
# The installer never silently discards a hooks configuration it did not
# write (R0001-0038). A core.hooksPath naming some other directory —
# including one inherited from global or system config — makes it refuse
# and print the ways forward; only an explicit --force takes over, and then
# in the scope that WINS — it says whether it replaced a local or worktree
# value or shadowed an inherited one, and it refuses on a command-scope value,
# which belongs to the invocation and outranks anything it could write
# (R0011-0014).
# The same rule covers hook *files*: a .git/hooks entry is removed only
# when it is byte-identical to the tracked hook it shadows, never on the
# strength of its name.
#
# TRACE: SCN-12

set -euo pipefail

want="scripts/hooks"
force=0

usage() {
  cat <<'USAGE'
Usage: scripts/install-hooks.sh [--force]

Points git's core.hooksPath at scripts/hooks so the tracked hooks run.

Without --force the script refuses, and changes nothing, when
core.hooksPath already names a different directory.

  --force     Take over anyway, in the scope that actually wins. A local or
              worktree value is overwritten in its OWN scope (the old one is
              saved to transync.replacedHooksPath there); an inherited
              global/system value is left where it is and shadowed by a
              local one. The restore command is printed in each case.
              A command-scope value (git -c core.hooksPath=..., or the
              GIT_CONFIG_* environment) belongs to the invocation rather
              than to any file, so no write this script can make would win:
              --force refuses instead of claiming an override.
  -h, --help  Show this text.
USAGE
}

for arg in "$@"; do
  case "$arg" in
    --force) force=1 ;;
    -h | --help)
      usage
      exit 0
      ;;
    *)
      echo "[install-hooks] unknown argument: $arg" >&2
      usage >&2
      exit 2
      ;;
  esac
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

# core.hooksPath is resolved relative to the top of the working tree, so
# "scripts/hooks", "./scripts/hooks" and the absolute path all name the
# same directory. Compare resolved paths, never the strings.
resolve_hooks_path() {
  local raw="$1" abs
  case "$raw" in
    /*) abs="$raw" ;;
    *) abs="$repo_root/$raw" ;;
  esac
  if [[ -d "$abs" ]]; then
    (cd "$abs" && pwd -P)
  else
    printf '%s' "$abs"
  fi
}

# --type=path expands a leading "~" the way git itself does when it reads
# core.hooksPath, so the comparison sees the same directory git would.
current_raw="$(git config --type=path --get core.hooksPath || true)"
current_scope=""
scope_flag=""
if [[ -n "$current_raw" ]]; then
  current_scope="$(git config --show-scope --get core.hooksPath | cut -f1 || true)"
  case "$current_scope" in
    local | global | system | worktree) scope_flag=" --$current_scope" ;;
    *) scope_flag="" ;;
  esac
fi

want_abs="$(resolve_hooks_path "$want")"
changed=0

if [[ -z "$current_raw" ]]; then
  git config --local core.hooksPath "$want"
  changed=1
  echo "[install-hooks] core.hooksPath -> $want (was unset)"
elif [[ "$(resolve_hooks_path "$current_raw")" == "$want_abs" ]]; then
  : # already ours — the run is a no-op apart from the housekeeping below
elif ((force == 0)); then
  # Every path printed as part of a copy-paste COMMAND goes through printf
  # '%q'; a bare '…' wrapper produces a line that does not parse the moment a
  # checkout path contains an apostrophe (R0011-0060). Paths printed as prose
  # (the current/resolved/wanted fields) stay unquoted — they are read, not run.
  {
    echo "[install-hooks] REFUSING: core.hooksPath already points somewhere else."
    echo "[install-hooks]   current : $current_raw${current_scope:+  (${current_scope} config)}"
    echo "[install-hooks]   resolved: $(resolve_hooks_path "$current_raw")"
    echo "[install-hooks]   wanted  : $want -> $want_abs"
    echo "[install-hooks] Nothing was changed; your hooks still run. Pick one:"
    echo "[install-hooks]   1. Chain both — have $(resolve_hooks_path "$current_raw")/pre-commit exec"
    echo "[install-hooks]      $(printf '%q' "$repo_root/scripts/hooks/pre-commit") (likewise for any other hook)."
    echo "[install-hooks]   2. Take over — scripts/install-hooks.sh --force"
    echo "[install-hooks]      (it prints the command that puts your value back)."
    echo "[install-hooks]   3. Clear it — git config${scope_flag} --unset core.hooksPath"
    echo "[install-hooks]      then re-run this script."
  } >&2
  exit 1
elif [[ "$current_scope" == "local" || "$current_scope" == "worktree" ]]; then
  # Write in the scope that WINS, not always --local. With
  # extensions.worktreeConfig on, .git/config.worktree outranks .git/config,
  # so a local write under a worktree-scoped value announces an override git
  # never honours — the promised hook stays inactive (R0011-0014).
  git config "--$current_scope" transync.replacedHooksPath "$current_raw"
  git config "--$current_scope" core.hooksPath "$want"
  changed=1
  echo "[install-hooks] --force: replaced the $current_scope core.hooksPath"
  echo "[install-hooks]   was    : $current_raw (saved as transync.replacedHooksPath in the $current_scope config)"
  echo "[install-hooks]   now    : $want"
  echo "[install-hooks]   restore: git config --$current_scope core.hooksPath $(printf '%q' "$current_raw")"
elif [[ "$current_scope" == "command" ]]; then
  # Command scope is `git -c core.hooksPath=…` or the GIT_CONFIG_COUNT/
  # GIT_CONFIG_KEY_n environment: the value belongs to this invocation, not to
  # a file, and it outranks every scope this script could write. Forcing here
  # would write config git then ignores and report an override that did not
  # happen (R0011-0014), so refuse and name where the value comes from.
  {
    echo "[install-hooks] REFUSING: core.hooksPath is set for this command, not in any config file."
    echo "[install-hooks]   current : $current_raw  (command scope)"
    echo "[install-hooks]   resolved: $(resolve_hooks_path "$current_raw")"
    echo "[install-hooks]   wanted  : $want -> $want_abs"
    echo "[install-hooks] --force cannot win: command scope outranks every config file this script may write,"
    echo "[install-hooks] so nothing was changed. Drop the 'git -c core.hooksPath=…' (or unset the GIT_CONFIG_*"
    echo "[install-hooks] variables) and re-run:  scripts/install-hooks.sh"
  } >&2
  exit 1
else
  git config --local core.hooksPath "$want"
  changed=1
  echo "[install-hooks] --force: shadowed the ${current_scope:-inherited} core.hooksPath with a local one"
  echo "[install-hooks]   was    : $current_raw (left in the ${current_scope:-inherited} config, untouched)"
  echo "[install-hooks]   now    : $want"
  echo "[install-hooks]   restore: git config --local --unset core.hooksPath"
fi

chmod +x scripts/hooks/* 2>/dev/null || true

# The chmod's own failure is swallowed on purpose (a read-only checkout, a
# foreign owner, a filesystem with no exec bit), so the bit is VERIFIED rather
# than assumed: git silently skips a hook that is not executable, and printing
# the success line over one is an installer that reports a gate it did not
# install (R0011-0015).
for hook in scripts/hooks/*; do
  [[ -f "$hook" ]] || continue
  [[ -x "$hook" ]] && continue
  {
    echo "[install-hooks] FAIL: $hook is not executable — git will skip it, so the hook is NOT installed."
    echo "[install-hooks]   fix: chmod +x $(printf '%q' "$repo_root/$hook")"
  } >&2
  exit 1
done

# A legacy copy in .git/hooks is DEAD once core.hooksPath is set, but it
# silently goes stale and misleads anyone inspecting .git/hooks. Remove
# such a copy (discovered 2026-08-04: the copy, not the tracked hook, had
# been the live one while hooksPath was unset) — but only when it is
# provably a copy, i.e. byte-identical to the tracked hook it shadows.
#
# The name alone is not evidence. `.git/hooks/pre-commit` is where a
# developer's own hook lives too, it is untracked and unrecoverable, and
# matching a tracked hook's basename says nothing about who wrote it. A
# differing file is therefore preserved and named, never deleted: it is
# inert either way (core.hooksPath governs), so the only thing deleting it
# would buy is a tidier directory, at the price of someone else's work.
for hook in scripts/hooks/*; do
  [[ -f "$hook" ]] || continue
  legacy=".git/hooks/$(basename "$hook")"
  [[ -f "$legacy" ]] || continue
  if cmp -s "$hook" "$legacy"; then
    rm "$legacy"
    echo "[install-hooks] removed inert legacy copy $legacy (byte-identical to $hook; core.hooksPath governs)"
  else
    {
      echo "[install-hooks] KEPT: $legacy is not a copy of $hook."
      echo "[install-hooks]   It is inert — core.hooksPath -> $want governs which hooks run — but it"
      echo "[install-hooks]   differs from the tracked hook, so it may be yours. Nothing was deleted."
      echo "[install-hooks]   If it is stale and you want it gone:  rm $(printf '%q' "$repo_root/$legacy")"
    } >&2
  fi
done

if ((changed)); then
  echo "[install-hooks] active hooks: $(ls scripts/hooks 2>/dev/null | tr '\n' ' ')"
else
  echo "[install-hooks] core.hooksPath already -> $want (unchanged)"
fi
