#!/usr/bin/env bash
# transync — live OpenAI smoke.
#
# Drives the CLI against the real OpenAI API (no test-stub-provider
# feature) — the endpoint (Chat Completions vs Responses) is chosen by
# transync-openai's model-driven dispatch; set TRANSYNC_OPENAI_API to
# override. The resulting HTML bundle is then served by `transync serve`
# (ti b791d6; this used to shell out to `python3 -m http.server`, which
# is why the script no longer needs a Python at all) so you can verify
# SCN-13 partner-pane sync in a browser.
#
# Required:
#   OPENAI_API_KEY   — set before running
#
# Optional:
#   TRANSYNC_OPENAI_MODEL            — overrides default (gpt-5-chat-latest)
#   TRANSYNC_OPENAI_BASE_URL         — overrides https://api.openai.com
#   TRANSYNC_LIVE_INPUT              — fixture path (default: SCN-01 fixture)
#   TRANSYNC_LIVE_TARGET_LANG        — target language label, opaque (default: ko)
#   TRANSYNC_LIVE_WORKDIR            — output directory (default: /Volumes/Temp/claude/transync-live)
#   TRANSYNC_LIVE_PORT               — server port (default: 7470)
#   TRANSYNC_LIVE_BIND               — address the demo server listens on (default: 127.0.0.1).
#                                      Must be an IP literal: `transync serve --bind` parses an
#                                      `IpAddr`, so a hostname such as `localhost` is rejected —
#                                      spell loopback 127.0.0.1 or ::1. Set 0.0.0.0 (or your dev
#                                      server's LAN IP) when browsing from a different machine
#                                      on the same network.
#   TRANSYNC_LIVE_ALLOW_HOST         — an extra authority the server answers for, as host or
#                                      host:port (forwarded as --allow-host). Needed with a
#                                      non-loopback bind: the server answers only for
#                                      authorities the bind names, so a browser typing the
#                                      machine's LAN address or name otherwise gets a 421.
#   TRANSYNC_LIVE_PROFILE            — path to a Profile TOML (forwarded as --profile)
#   TRANSYNC_LIVE_SYSTEM_PROMPT      — inline system-prompt body (forwarded as --system-prompt)
#   TRANSYNC_LIVE_SYSTEM_PROMPT_FILE — file containing the system-prompt body (forwarded as --system-prompt-file)
#                                      (TRANSYNC_LIVE_SYSTEM_PROMPT and ..._FILE are mutually exclusive)
#
# TRACE: SCN-12
# TRACE: SCN-13

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# Deletion guard for the env-overridable workdir — see the file for the rule.
# shellcheck source-path=SCRIPTDIR source=lib/workdir-guard.sh
source "$REPO_ROOT/scripts/lib/workdir-guard.sh"

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "[smoke-live] OPENAI_API_KEY is not set." >&2
  echo "             export it and re-run." >&2
  exit 1
fi

INPUT="${TRANSYNC_LIVE_INPUT:-crates/transync/tests/fixtures/scn-01-headings-and-paragraphs.md}"
TARGET_LANG="${TRANSYNC_LIVE_TARGET_LANG:-ko}"
# Default workdir prefers /Volumes/Temp/claude (per project guidance) but
# falls back to a system tmpdir when that path doesn't exist, so the
# script is portable for contributors without that volume mounted.
DEFAULT_WORKDIR="/Volumes/Temp/claude/transync-live"
if [[ -z "${TRANSYNC_LIVE_WORKDIR:-}" ]] && [[ ! -d "$(dirname "$DEFAULT_WORKDIR")" ]]; then
  DEFAULT_WORKDIR="${TMPDIR:-/tmp}/transync-live"
fi
WORKDIR="${TRANSYNC_LIVE_WORKDIR:-$DEFAULT_WORKDIR}"
PORT="${TRANSYNC_LIVE_PORT:-7470}"
BIND="${TRANSYNC_LIVE_BIND:-127.0.0.1}"

# `serve --bind` is an `IpAddr` field, so a hostname dies in clap. Refuse it here:
# the server starts only after a *paid* translation run, and failing at the end of
# that run is the expensive way to learn the value was never parseable.
if ! [[ "$BIND" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]] \
   && ! { [[ "$BIND" == *:* ]] && [[ "$BIND" =~ ^[0-9A-Fa-f:]+$ ]]; }; then
  echo "[smoke-live] TRANSYNC_LIVE_BIND must be an IP literal, not a hostname: $BIND" >&2
  echo "[smoke-live] use 127.0.0.1 or ::1 for loopback, 0.0.0.0 or a LAN IP otherwise." >&2
  exit 1
fi
ALLOW_HOST="${TRANSYNC_LIVE_ALLOW_HOST:-}"

if [[ ! -f "$INPUT" ]]; then
  echo "[smoke-live] input fixture not found: $INPUT" >&2
  exit 1
fi

if [[ -n "${TRANSYNC_LIVE_SYSTEM_PROMPT:-}" && -n "${TRANSYNC_LIVE_SYSTEM_PROMPT_FILE:-}" ]]; then
  echo "[smoke-live] TRANSYNC_LIVE_SYSTEM_PROMPT and TRANSYNC_LIVE_SYSTEM_PROMPT_FILE are mutually exclusive." >&2
  exit 1
fi

# The demo server is `transync serve` from the same workspace build as
# the translate run below, so there is no external interpreter to locate
# and no version of it to verify. (R0008-0053's Python-3 probe lived here
# for exactly that reason and retired with the dependency, ti b791d6.)

# R0003-0066 — refuse to delete an env-controlled path that this script
# does not own. Anything we wipe must satisfy the shared rule sourced
# above: temp-root containment, a not-yet-existing path, or our own
# marker file. A typo'd TRANSYNC_LIVE_WORKDIR can't take down a real
# directory, and (R0001-0002) a 'transync-' basename no longer excuses
# one that lives outside every temp root.
transync_guard_workdir smoke-live TRANSYNC_LIVE_WORKDIR "$WORKDIR"
rm -rf "$WORKDIR"
mkdir -p "$WORKDIR"
transync_mark_workdir "$WORKDIR"

OUT_MD="$WORKDIR/out.md"
OUT_JSON="$WORKDIR/out.json"
HTML_OUT="$WORKDIR/html"

echo "[smoke-live] cargo build --workspace"
cargo build --workspace --quiet

# Assemble optional CLI arguments based on env.
extra_args=()
if [[ -n "${TRANSYNC_LIVE_PROFILE:-}" ]]; then
  extra_args+=( --profile "$TRANSYNC_LIVE_PROFILE" )
fi
if [[ -n "${TRANSYNC_LIVE_SYSTEM_PROMPT:-}" ]]; then
  extra_args+=( --system-prompt "$TRANSYNC_LIVE_SYSTEM_PROMPT" )
elif [[ -n "${TRANSYNC_LIVE_SYSTEM_PROMPT_FILE:-}" ]]; then
  extra_args+=( --system-prompt-file "$TRANSYNC_LIVE_SYSTEM_PROMPT_FILE" )
fi

echo "[smoke-live] translating ${INPUT} -> ${TARGET_LANG} via the OpenAI API (endpoint chosen by transync-openai's model-name dispatch — Chat Completions or Responses; override with TRANSYNC_OPENAI_API)"
echo "[smoke-live] model: ${TRANSYNC_OPENAI_MODEL:-gpt-5-chat-latest} (override via TRANSYNC_OPENAI_MODEL)"
if [[ ${#extra_args[@]} -gt 0 ]]; then
  # R0008-0055: redact the inline --system-prompt body so proprietary prompt
  # text doesn't leak into terminals/CI logs. Option names and file paths
  # are safe to show.
  display_args=()
  redact_next=0
  for a in "${extra_args[@]}"; do
    if [[ $redact_next -eq 1 ]]; then
      display_args+=( "<redacted>" )
      redact_next=0
      continue
    fi
    display_args+=( "$a" )
    if [[ "$a" == "--system-prompt" ]]; then
      redact_next=1
    fi
  done
  echo "[smoke-live] extra args: ${display_args[*]}"
fi
cargo run -p transync-cli --quiet -- translate \
  --input "$INPUT" \
  --output "$OUT_MD" \
  --map "$OUT_JSON" \
  --html-out "$HTML_OUT" \
  --target-language "$TARGET_LANG" \
  "${extra_args[@]}"

echo "[smoke-live] verifying output files"
for path in \
  "$OUT_MD" \
  "$OUT_JSON" \
  "$HTML_OUT/index.html" \
  "$HTML_OUT/source.html" \
  "$HTML_OUT/target.html" \
  "$HTML_OUT/alignment.json" \
  "$HTML_OUT/sync.js" \
  "$HTML_OUT/purify.min.js"
do
  if [[ ! -s "$path" ]]; then
    echo "[smoke-live] FAIL: $path missing or empty" >&2
    exit 1
  fi
done

echo "[smoke-live] OK — outputs landed in $WORKDIR"
echo
echo "============================================================"
echo "  Translated MD:    $OUT_MD"
echo "  Alignment JSON:   $OUT_JSON"
echo "  Demo bundle:      $HTML_OUT/"
echo "============================================================"
echo
echo "[smoke-live] starting demo server: transync serve --bind $BIND --port $PORT"
# An IPv6 literal needs brackets in a URL; an IPv4 one must not have them.
if [[ "$BIND" == *:* ]]; then BIND_URL_HOST="[$BIND]"; else BIND_URL_HOST="$BIND"; fi
if [[ "$BIND" == "127.0.0.1" || "$BIND" == "::1" ]]; then
  echo "[smoke-live] open http://${BIND_URL_HOST}:${PORT}/  (Ctrl-C to stop)"
else
  echo "[smoke-live] open http://<this-host-on-${BIND}>:${PORT}/  (Ctrl-C to stop)"
  echo "[smoke-live] (binding to ${BIND}; reachable from other machines on the network)"
  if [[ -z "$ALLOW_HOST" ]]; then
    echo "[smoke-live] the server answers only for the authorities this bind names — set" >&2
    echo "[smoke-live] TRANSYNC_LIVE_ALLOW_HOST=<the host:port the browser types> if it 421s." >&2
  fi
fi
echo

# Same package and same (default) feature set as the translate run above,
# so this resolves to the binary already built rather than triggering a
# rebuild between the two.
exec cargo run -p transync-cli --quiet -- serve \
  --rendered "$HTML_OUT" \
  --bind "$BIND" \
  --port "$PORT" \
  ${ALLOW_HOST:+--allow-host "$ALLOW_HOST"}
