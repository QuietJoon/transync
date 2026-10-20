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
# THE READ-SIDE RULE (R0012-0003). THE RULE asks whether a path is
# disposable; it never asks what the run opens after deleting it, and these
# scripts delete their workdir before the run reads its input, profile,
# prompt file or checkout. So a workdir that passes THE RULE must also hold
# nothing the run reads: `transync_guard_read_side` below refuses when
# deleting it would remove a read path or anything that path resolves
# through. smoke.sh, smoke-live.sh and test-browser.sh wipe their whole
# workdir and run both guards, in that order, immediately before `rm -rf`.
# build-wasm.sh does not call it: it never wipes its staging root, only
# trees it created itself in this run or a stale one named by its own pid.
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

# Print the inode of the directory entry <path> names, without following a
# final symbolic link (`ls -d` does not follow its operand). Prints nothing
# when there is no such entry. `ls`, because the portable alternatives are
# not: `stat` spells this differently on BSD and GNU, and `find -printf` is
# GNU-only. Only the leading number is read, so the name cannot mislead it.
_transync_entry_inode() {
  # shellcheck disable=SC2012
  ls -di -- "$1" 2>/dev/null | awk '{ print $1 }'
}

# _transync_resolves_through <mode> <workdir> <link-parent> <link-inode> <path>
#
# Resolves <path> one component at a time, the way the kernel does — every
# symbolic link expanded where it is met, `..` taken from the physical
# directory reached so far — and returns 1 as soon as the walk depends on
# what deleting <workdir> removes, 0 when it never does, and 2 when it cannot
# finish (a link that cannot be read, or more than 40 of them).
#
# <mode> says what is at stake. `tree`: the whole tree <workdir> resolves to.
# A walk starting at / reaches anything inside that tree only by standing in
# its root, so each directory the walk stands in is compared with <workdir>
# by identity (`-ef`), never by spelling. `link`: one directory entry, a
# symbolic link; the walk depends on it when it looks up an entry in the
# link's own directory (<link-parent>) and finds that same entry (inode
# <link-inode>).
_transync_resolves_through() {
  local mode="$1" dir="$2" link_parent="$3" link_inode="$4" path="$5"
  local cur rest comp entry target hops=0

  case "$path" in
    /*) ;;
    *) path="$PWD/$path" ;;
  esac
  cur=/
  rest="$path"
  while :; do
    if [[ "$mode" == tree && "$cur" -ef "$dir" ]]; then
      return 1
    fi
    while [[ "$rest" == /* ]]; do
      rest="${rest#/}"
    done
    if [[ -z "$rest" ]]; then
      return 0
    fi
    comp="${rest%%/*}"
    if [[ "$comp" == "$rest" ]]; then
      rest=""
    else
      rest="${rest#*/}"
    fi
    case "$comp" in
      .) continue ;;
      ..)
        cur="$(dirname "$cur")"
        continue
        ;;
    esac
    entry="${cur%/}/$comp"
    if [[ "$mode" == link && "$cur" -ef "$link_parent" ]]; then
      if [[ "$(_transync_entry_inode "$entry")" == "$link_inode" ]]; then
        return 1
      fi
    fi
    if [[ -L "$entry" ]]; then
      hops=$((hops + 1))
      if ((hops > 40)); then
        return 2
      fi
      if ! target="$(readlink "$entry")"; then
        return 2
      fi
      case "$target" in
        /*) cur=/ ;;
      esac
      rest="$target/$rest"
      continue
    fi
    cur="$entry"
  done
}

# transync_guard_read_side <log-tag> <env-var-name> <workdir> <read-path>...
#
# Exits the calling script with status 1 and a diagnostic when deleting
# <workdir> would remove a <read-path>, or a directory or symbolic link that
# <read-path> resolves through — so a link whose target sits inside the
# workdir is caught, and so is a path reaching the workdir under another
# spelling (a symlinked ancestor, `..`, another letter case on a volume that
# folds it). Call it after transync_guard_workdir and before
# `rm -rf <workdir>`, with every path the run reads after the deletion,
# including the checkout.
#
# What is at stake follows how `rm -rf` treats <workdir>'s spelling. A
# <workdir> that is a symbolic link, named without a trailing slash, is
# removed as a link: only that entry is at stake, so a read path inside the
# link's target is safe and one spelled through the link is not. Named with
# a trailing slash, `rm -rf` follows the link and deletes the directory it
# names, so the target's whole tree is at stake — as it is for every other
# existing <workdir>. A <workdir> that does not exist puts nothing at stake.
#
# Empty <read-path> arguments are skipped, so optional knobs can be passed
# unconditionally. A relative one is resolved against the current
# directory, which is where the caller's later commands resolve it.
transync_guard_read_side() {
  local tag="$1" var="$2" dir="$3"
  shift 3
  local mode link_parent="" link_inode="" read_path status

  if [[ -L "$dir" ]]; then
    mode="link"
    link_parent="$(_transync_canonical_dir "$(dirname "$dir")")"
    link_inode="$(_transync_entry_inode "$dir")"
  elif [[ -e "$dir" ]]; then
    mode="tree"
  else
    return 0
  fi

  for read_path in "$@"; do
    [[ -n "$read_path" ]] || continue
    status=0
    _transync_resolves_through "$mode" "$dir" "$link_parent" "$link_inode" "$read_path" || status=$?
    if [[ $status -eq 1 ]]; then
      echo "[$tag] refusing to rm -rf $dir — this run reads $read_path, which is inside it or resolves through it; set $var to a path that holds nothing the run reads." >&2
      exit 1
    fi
    if [[ $status -ne 0 ]]; then
      echo "[$tag] refusing to rm -rf $dir — cannot resolve $read_path (an unreadable symbolic link, or more than 40 of them), so nothing shows it lies outside; fix that path or set $var elsewhere." >&2
      exit 1
    fi
  done
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
