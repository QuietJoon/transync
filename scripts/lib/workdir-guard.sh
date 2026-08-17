# transync — shared scratch-workdir deletion guard.
#
# Sourced (never executed) by every script that `rm -rf`s an
# env-overridable working directory: smoke.sh, smoke-live.sh,
# test-browser.sh, build-wasm.sh. One home for the rule on purpose — the
# guard used to be copy-pasted into each script, which is the shape that
# let the escape hatch this file replaces (R0001-0002) outlive the sibling
# fix that hardened the same guard's root-equality handling (R0001-0001).
#
# THE RULE. A path may be deleted only when it satisfies one of:
#
#   1. Containment — after resolving symlinks on its *parent*, the path is
#      a strict descendant of an approved temp root (/tmp, /private/tmp,
#      /var/folders, /Volumes/Temp/claude, $TMPDIR). The parent, not the
#      leaf: `rm -rf` on a symlinked leaf removes the link, never the
#      directory it points at, so resolving the leaf would reject safe
#      paths without making any unsafe one safe. Strict descendant, so a
#      temp root itself is never the target.
#   2. Absence — the path does not exist, so there is nothing to delete.
#      This is the fresh-directory bootstrap case: the caller's `mkdir -p`
#      creates it and `transync_mark_workdir` stamps it, which promotes it
#      to case 3 for every later run.
#   3. Ownership — it is a directory carrying the marker file that
#      `transync_mark_workdir` wrote on an earlier run of one of these
#      scripts.
#
# A path is NEVER deletable because of how it is *named*. The rule this
# replaces accepted any path whose basename began with "transync-",
# anywhere on disk, so pointing an override at /Users/x/transync-notes
# deleted it. Naming is not ownership; the marker file is.
#
# '..' is rejected outright before any of the above (R0008-0008):
# /tmp/../valuable-dir would otherwise satisfy case 1 by string prefix.
#
# shellcheck shell=bash

# The marker filename is part of the handshake — do not rename it without
# accounting for workdirs stamped by an older revision.
TRANSYNC_WORKDIR_MARKER=".transync-workdir"

# Print the symlink-resolved absolute path of an existing directory; print
# nothing when it does not exist, is not a directory, or is unreachable.
_transync_canonical_dir() {
  ( cd "$1" 2>/dev/null && pwd -P ) || true
}

# transync_guard_workdir <log-tag> <env-var-name> <path>
#
# Exits the calling script with status 1 and a diagnostic unless <path>
# satisfies THE RULE above. Call it immediately before `rm -rf <path>`.
transync_guard_workdir() {
  local tag="$1" var="$2" dir="$3"
  local parent canon root canon_root

  case "$dir" in
    /*) ;;
    *)
      echo "[$tag] refusing to rm -rf $dir — path must be absolute; set $var to an absolute path." >&2
      exit 1
      ;;
  esac

  case "$dir" in
    *..*)
      echo "[$tag] refusing to rm -rf $dir — path must not contain '..'" >&2
      exit 1
      ;;
  esac

  # Case 1 — containment under an approved temp root.
  parent="$(_transync_canonical_dir "$(dirname "$dir")")"
  if [[ -n "$parent" ]]; then
    canon="${parent%/}/$(basename "$dir")"
    for root in /tmp /private/tmp /var/folders /Volumes/Temp/claude "${TMPDIR:-}"; do
      [[ -n "$root" ]] || continue
      canon_root="$(_transync_canonical_dir "$root")"
      [[ -n "$canon_root" ]] || continue
      # A root of "/" would make the descendant test below match everything;
      # only $TMPDIR can be set that pathologically, and it forfeits the rule.
      [[ "$canon_root" != "/" ]] || continue
      if [[ "$canon" == "${canon_root%/}"/* ]]; then
        return 0
      fi
    done
  fi

  # Case 2 — nothing there to delete.
  if [[ ! -e "$dir" ]]; then
    return 0
  fi

  # Case 3 — a workdir one of these scripts created and stamped.
  if [[ -d "$dir" && -f "$dir/$TRANSYNC_WORKDIR_MARKER" ]]; then
    return 0
  fi

  echo "[$tag] refusing to rm -rf $dir — $var must name one of: a path under a temp root (/tmp, /private/tmp, /var/folders, /Volumes/Temp/claude, or \$TMPDIR), a path that does not exist yet (it will be created and marked), or a directory an earlier run marked as its own (one containing $TRANSYNC_WORKDIR_MARKER). A 'transync-' prefix in the name grants nothing." >&2
  exit 1
}

# transync_mark_workdir <path>
#
# Stamps the marker that lets a later run recognise <path> as its own
# disposable scratch directory. Call it immediately after `mkdir -p`.
transync_mark_workdir() {
  printf '%s\n' \
    "transync scratch workdir — created by this repo's scripts/, safe to delete." \
    "This marker is what authorises the next run's 'rm -rf' of this directory." \
    > "$1/$TRANSYNC_WORKDIR_MARKER"
}
