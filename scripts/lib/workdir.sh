# transync — where a script's scratch workdir goes. One rule, one place.
#
# Sourced, never executed. Provides `transync_pick_workdir`.
#
# ## Why this exists (ti `13a73b`)
#
# Five scripts each carried their own copy of "prefer /Volumes/Temp/claude,
# fall back to $TMPDIR when it is absent", and the copies had already drifted:
#
#   test-browser.sh   discarded an explicit TRANSYNC_FIXTURE_WORKDIR
#   smoke.sh          discarded an explicit TRANSYNC_SMOKE_WORKDIR
#   build-wasm.sh     discarded an explicit TRANSYNC_WASM_STAGING
#   smoke-live.sh     honoured its override
#   smoke-live-long.sh honoured its override
#
# The first three tested the PREFERRED root's parent and relocated on that
# answer alone, so pointing the override at a path whose parent did not exist
# yet silently sent the run somewhere else entirely. That is a bug, not a
# policy difference, and it is the kind that only shows up as "why is my output
# not where I put it".
#
# ## The rule
#
# 1. An explicit override WINS, always, verbatim. A caller who names a path has
#    said where they want it; second-guessing that is what the drift above was.
# 2. Otherwise the preferred path is used when its parent directory exists.
# 3. Otherwise the script REFUSES, naming the missing root and the override
#    variable that fixes it.
#
# ## On refusing rather than relocating
#
# The fallback was deliberate and correct for portability — `/Volumes/Temp/claude`
# is one machine's path (R0008-0054 asked for a working default without it). What
# was wrong is that it was SILENT: nothing distinguished "a contributor is on
# another machine" from "the volume this machine mandates is unmounted right
# now", and the operating rule for agent sessions here is that an unreachable
# scratch root is a stop-and-ask, not a work-around. A relocation nobody is told
# about leaves artifacts outside the tree that rule governs and reports the run
# as normal.
#
# Refusing keeps the portability answer available and makes it explicit: the
# message names the variable to set, so `TRANSYNC_SMOKE_WORKDIR=/tmp/x
# ./scripts/smoke.sh` works anywhere. The cost is honest and is the reason this
# is one constant rather than five: a bare `git clone && ./scripts/smoke.sh` on
# a machine without the volume now fails with an instruction instead of
# succeeding somewhere unexpected. Flip TRANSYNC_WORKDIR_POLICY to `warn` below
# to restore the old behaviour in one edit if that trade turns out wrong.
#
# TRACE: ti 13a73b
# TRACE: R0008-0054

# `refuse` (default) or `warn`. See the note above before changing it.
TRANSYNC_WORKDIR_POLICY="${TRANSYNC_WORKDIR_POLICY:-refuse}"

# transync_pick_workdir <override-var-name> <preferred-path>
#
# Echoes the chosen path. Exits 1 (policy `refuse`) or warns and falls back to
# ${TMPDIR:-/tmp} (policy `warn`) when the preferred root is absent and no
# override is set.
transync_pick_workdir() {
  local var_name="$1"
  local preferred="$2"
  local override="${!var_name:-}"

  # (1) An explicit override wins verbatim — including when its parent does not
  # exist yet, because the caller may intend the script to create it.
  if [[ -n "$override" ]]; then
    printf '%s\n' "$override"
    return 0
  fi

  # (2) The preferred path, when its parent is there to hold it.
  local root
  root="$(dirname "$preferred")"
  if [[ -d "$root" ]]; then
    printf '%s\n' "$preferred"
    return 0
  fi

  # (3) Neither. Say which root is missing and which variable fixes it.
  # Strip trailing slashes: macOS exports TMPDIR with one, and `$TMPDIR/x`
  # would otherwise read as `//x` in every message and every fallback path.
  local tmp="${TMPDIR:-/tmp}"
  tmp="${tmp%/}"
  local leaf
  leaf="$(basename "$preferred")"
  local msg="scratch root $root is not present, so $preferred cannot be used.
Set $var_name to a writable path, e.g.:
  $var_name=$tmp/$leaf $0"
  if [[ "$TRANSYNC_WORKDIR_POLICY" == "warn" ]]; then
    printf 'warning: %s\n' "$msg" >&2
    printf '%s\n' "$tmp/$leaf"
    return 0
  fi
  printf 'error: %s\n' "$msg" >&2
  return 1
}
