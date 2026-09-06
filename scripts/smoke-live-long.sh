#!/usr/bin/env bash
# transync — live OpenAI smoke against the long-form demo input.
#
# Thin wrapper over scripts/smoke-live.sh that pins TRANSYNC_LIVE_INPUT to
# samples/demo-long.md. This sample (the indy-review report on transync
# itself) exercises the messy real-world cases — many tables, many code
# fences, deeply nested lists, mixed inline code and prose — and is the
# one this project's user has hit the most rough edges with. Use it to
# reproduce / verify any fix that touches batching, validation, regen,
# or the JS sync engine on a non-trivial document.
#
# Required:
#   OPENAI_API_KEY   — set before running
#
# All other knobs (TRANSYNC_OPENAI_MODEL, TRANSYNC_LIVE_TARGET_LANG,
# TRANSYNC_LIVE_PORT, TRANSYNC_LIVE_BIND, TRANSYNC_LIVE_PROFILE,
# TRANSYNC_LIVE_SYSTEM_PROMPT[_FILE], TRANSYNC_LIVE_WORKDIR) pass through
# untouched — see scripts/smoke-live.sh for documentation.
#
# TRACE: SCN-12
# TRACE: SCN-13

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# Where a scratch workdir goes — one rule for all five scripts (ti `13a73b`).
# shellcheck source-path=SCRIPTDIR source=lib/workdir.sh
source "$REPO_ROOT/scripts/lib/workdir.sh"
export TRANSYNC_LIVE_INPUT="${TRANSYNC_LIVE_INPUT:-${REPO_ROOT}/samples/demo-long.md}"

# Use a separate workdir + port so concurrent runs of smoke-live.sh and
# smoke-live-long.sh don't clobber each other's output bundles or race
# for the same demo-server port. The default is
# /Volumes/Temp/claude/transync-live-long; when that root is absent the run
# REFUSES and names TRANSYNC_LIVE_WORKDIR (ti `13a73b`) rather than relocating
# to $TMPDIR — R0008-0054's portable answer is still there, but you ask for it
# with TRANSYNC_WORKDIR_POLICY=warn or by naming the path. (R0011-0053: this
# comment claimed the silent fallback the helper deliberately stopped doing.)
DEFAULT_LONG_WORKDIR="$(transync_pick_workdir TRANSYNC_LIVE_WORKDIR /Volumes/Temp/claude/transync-live-long)"
export TRANSYNC_LIVE_WORKDIR="${TRANSYNC_LIVE_WORKDIR:-$DEFAULT_LONG_WORKDIR}"
export TRANSYNC_LIVE_PORT="${TRANSYNC_LIVE_PORT:-7471}"

exec "$(dirname "$0")/smoke-live.sh"
