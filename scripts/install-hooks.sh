#!/usr/bin/env bash
# Install transync's tracked git hooks by pointing core.hooksPath at the
# repo's scripts/hooks directory (R0008-0058). Unlike a copy into
# .git/hooks, this keeps the hook under version control so a clean clone
# receives it after running this script once.
#
# The installer never silently discards a hooks configuration it did not
# write (R0001-0038). A core.hooksPath naming some other directory —
# including one inherited from global or system config — makes it refuse
# and print the ways forward; only an explicit --force takes over, and it
# says whether it replaced a local value or shadowed an inherited one.
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

  --force     Take over anyway. A local value is overwritten (the old one
              is saved to the local config key transync.replacedHooksPath);
              an inherited global/system value is left where it is and
              shadowed by a local one. The restore command is printed
              either way.
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
  {
    echo "[install-hooks] REFUSING: core.hooksPath already points somewhere else."
    echo "[install-hooks]   current : $current_raw${current_scope:+  (${current_scope} config)}"
    echo "[install-hooks]   resolved: $(resolve_hooks_path "$current_raw")"
    echo "[install-hooks]   wanted  : $want -> $want_abs"
    echo "[install-hooks] Nothing was changed; your hooks still run. Pick one:"
    echo "[install-hooks]   1. Chain both — have $(resolve_hooks_path "$current_raw")/pre-commit exec"
    echo "[install-hooks]      '$repo_root/scripts/hooks/pre-commit' (likewise for any other hook)."
    echo "[install-hooks]   2. Take over — scripts/install-hooks.sh --force"
    echo "[install-hooks]      (it prints the command that puts your value back)."
    echo "[install-hooks]   3. Clear it — git config${scope_flag} --unset core.hooksPath"
    echo "[install-hooks]      then re-run this script."
  } >&2
  exit 1
elif [[ "$current_scope" == "local" ]]; then
  git config --local transync.replacedHooksPath "$current_raw"
  git config --local core.hooksPath "$want"
  changed=1
  echo "[install-hooks] --force: replaced the local core.hooksPath"
  echo "[install-hooks]   was    : $current_raw (saved as transync.replacedHooksPath)"
  echo "[install-hooks]   now    : $want"
  echo "[install-hooks]   restore: git config --local core.hooksPath '$current_raw'"
else
  git config --local core.hooksPath "$want"
  changed=1
  echo "[install-hooks] --force: shadowed the ${current_scope:-inherited} core.hooksPath with a local one"
  echo "[install-hooks]   was    : $current_raw (left in the ${current_scope:-inherited} config, untouched)"
  echo "[install-hooks]   now    : $want"
  echo "[install-hooks]   restore: git config --local --unset core.hooksPath"
fi

chmod +x scripts/hooks/* 2>/dev/null || true

# A legacy copy in .git/hooks is DEAD once core.hooksPath is set, but it
# silently goes stale and misleads anyone inspecting .git/hooks. Remove
# any copy that shadows a tracked hook (discovered 2026-08-04: the copy,
# not the tracked hook, had been the live one while hooksPath was unset).
for hook in scripts/hooks/*; do
  legacy=".git/hooks/$(basename "$hook")"
  if [[ -f "$legacy" ]]; then
    rm "$legacy"
    echo "[install-hooks] removed inert legacy copy $legacy (core.hooksPath governs)"
  fi
done

if ((changed)); then
  echo "[install-hooks] active hooks: $(ls scripts/hooks 2>/dev/null | tr '\n' ' ')"
else
  echo "[install-hooks] core.hooksPath already -> $want (unchanged)"
fi
