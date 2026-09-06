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
# Where a scratch workdir goes — one rule for all five scripts (ti `13a73b`).
# shellcheck source-path=SCRIPTDIR source=lib/workdir.sh
source "$REPO_ROOT/scripts/lib/workdir.sh"

if [[ -z "${OPENAI_API_KEY:-}" ]]; then
  echo "[smoke-live] OPENAI_API_KEY is not set." >&2
  echo "             export it and re-run." >&2
  exit 1
fi

INPUT="${TRANSYNC_LIVE_INPUT:-crates/transync/tests/fixtures/scn-01-headings-and-paragraphs.md}"
TARGET_LANG="${TRANSYNC_LIVE_TARGET_LANG:-ko}"
# R0011-0049 — `--target-language ""` is a deterministic CLI refusal; reaching it
# after `cargo build` is the expensive way to read the same message.
if [[ -z "${TARGET_LANG//[[:space:]]/}" ]]; then
  echo "[smoke-live] TRANSYNC_LIVE_TARGET_LANG must not be blank." >&2
  exit 1
fi
# Default workdir is /Volumes/Temp/claude/transync-live. When that root is absent
# the run REFUSES and names TRANSYNC_LIVE_WORKDIR (ti `13a73b`) — there is no
# silent relocation to $TMPDIR; set TRANSYNC_WORKDIR_POLICY=warn to restore that.
# (R0011-0052: this comment promised the fallback the helper deliberately
# stopped doing.)
DEFAULT_WORKDIR="$(transync_pick_workdir TRANSYNC_LIVE_WORKDIR /Volumes/Temp/claude/transync-live)"
WORKDIR="${TRANSYNC_LIVE_WORKDIR:-$DEFAULT_WORKDIR}"
PORT="${TRANSYNC_LIVE_PORT:-7470}"
BIND="${TRANSYNC_LIVE_BIND:-127.0.0.1}"

# `serve --bind` is an `IpAddr` field, so a hostname dies in clap. Refuse it here:
# the server starts only after a *paid* translation run, and failing at the end of
# that run is the expensive way to learn the value was never parseable.
#
# This pair of shapes is the cheap hostname refusal only — it costs nothing and
# names the mistake operators actually make (`localhost`). It is NOT the
# grammar: `999.999.999.999` and `::::` both match and neither is an address
# (R0011-0007, R0011-0008). The argv probe after the build is the authority,
# because clap's own `IpAddr` parse is the only grammar worth trusting here.
if ! [[ "$BIND" =~ ^([0-9]{1,3}\.){3}[0-9]{1,3}$ ]] \
   && ! { [[ "$BIND" == *:* ]] && [[ "$BIND" =~ ^[0-9A-Fa-f:]+$ ]]; }; then
  echo "[smoke-live] TRANSYNC_LIVE_BIND must be an IP literal, not a hostname: $BIND" >&2
  echo "[smoke-live] use 127.0.0.1 or ::1 for loopback, 0.0.0.0 or a LAN IP otherwise." >&2
  exit 1
fi

# R0011-0009 — `serve --port` is a `u16`, so clap refuses a non-number and
# anything past 65535; the probe below would catch both, but this costs nothing.
# 0 is refused *here* although clap accepts it: 0 asks the kernel for a free
# port, and this script prints a fixed `http://host:$PORT/` before exec'ing the
# server, so the URL an operator is told to open would be the one thing that
# never listens. The five-digit bound keeps the arithmetic below from wrapping,
# and `10#` keeps a zero-padded `08080` out of bash's octal reading of it.
if ! [[ "$PORT" =~ ^[0-9]{1,5}$ ]] || (( 10#$PORT < 1 || 10#$PORT > 65535 )); then
  echo "[smoke-live] TRANSYNC_LIVE_PORT must be 1-65535 (0 would let the kernel pick, and the URL printed below names \$PORT): $PORT" >&2
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

# R0011-0050 / R0011-0051 — both of these are forwarded as paths the CLI opens.
# A path that is not a readable file fails the same way every time, so reach the
# failure here rather than after a build. Contents stay the CLI's call: this
# checks reachability, not that the TOML parses or the prompt is sensible.
if [[ -n "${TRANSYNC_LIVE_PROFILE:-}" \
      && ( ! -f "$TRANSYNC_LIVE_PROFILE" || ! -r "$TRANSYNC_LIVE_PROFILE" ) ]]; then
  echo "[smoke-live] profile is not a readable file: $TRANSYNC_LIVE_PROFILE" >&2
  exit 1
fi
if [[ -n "${TRANSYNC_LIVE_SYSTEM_PROMPT_FILE:-}" \
      && ( ! -f "$TRANSYNC_LIVE_SYSTEM_PROMPT_FILE" || ! -r "$TRANSYNC_LIVE_SYSTEM_PROMPT_FILE" ) ]]; then
  echo "[smoke-live] system-prompt file is not a readable file: $TRANSYNC_LIVE_SYSTEM_PROMPT_FILE" >&2
  exit 1
fi

# The demo server is `transync serve` from the very same binary as
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

# One build, one binary, three uses (R0011-0055). `cargo run` re-resolved the
# dependency graph and re-checked freshness for each phase, and — because those
# phases straddle a paid translation that can take minutes — a source edit
# between them handed the browser a *different* binary than the one that
# produced the bundle. Resolving the path out of cargo's own JSON rather than
# guessing `target/debug/…` keeps working under a redirected CARGO_TARGET_DIR;
# same pattern as test-browser.sh, and deliberately still `sed` rather than that
# script's answer to R0011-0012 — this one has no interpreter prerequisite at
# all (ti b791d6 retired the last), and a target dir whose path needs JSON
# unescaping is not a trade worth a Node dependency here. The `-x` guard below
# is what turns a mangled path into a refusal instead of a confusing later
# failure.
#
# R0011-0013 IS taken, though, because it costs nothing and is not about
# escaping: build scripts and any second binary added to this package later
# carry an `executable` too, so "the last line with a path" is not "the
# transync CLI". The filter names the bin target (`[[bin]] name = "transync"`),
# and anything other than exactly one match refuses rather than guesses.
echo "[smoke-live] cargo build -p transync-cli"
BUILD_LOG="$WORKDIR/cargo-build.jsonl"
cargo build -p transync-cli --message-format=json-render-diagnostics > "$BUILD_LOG"
# `|| true`: under `set -euo pipefail` a `grep` that matches nothing exits 1
# and would abort the script here, turning "no artifact" into a silent death
# instead of the refusal below.
TRANSYNC_BINS="$(
  {
    grep -F '"kind":["bin"]' "$BUILD_LOG" \
      | grep -F '"name":"transync"' \
      | sed -n 's/.*"executable":"\([^"]*\)".*/\1/p' \
      | sort -u
  } || true
)"
if [[ "$(printf '%s\n' "$TRANSYNC_BINS" | grep -c .)" -ne 1 ]]; then
  echo "[smoke-live] FAIL: expected exactly one transync bin artifact in $BUILD_LOG" >&2
  printf '%s\n' "$TRANSYNC_BINS" >&2
  exit 1
fi
TRANSYNC_BIN="$TRANSYNC_BINS"
if [[ -z "$TRANSYNC_BIN" || ! -x "$TRANSYNC_BIN" ]]; then
  echo "[smoke-live] FAIL: could not resolve the transync binary from $BUILD_LOG" >&2
  exit 1
fi

# R0011-0007 / R0011-0008 / R0011-0010 — the serve argv, checked before the
# *paid* translate instead of after it. This asks the one component whose answer
# counts rather than re-implementing IPv6 grammar and authority syntax in bash:
# clap parses --bind as an `IpAddr`, --port as a `u16` and every --allow-host as
# an `Authority`, so a bad value exits 1 (ArgumentError, contracts.md §6) with
# clap's own message. A well-formed argv reaches `serve`'s
# canonicalize(--rendered), which runs BEFORE `TcpListener::bind` and fails on
# this deliberately absent path with exit 2 (InputReadFailure) — so the probe
# opens no socket and cannot collide with anything already on $PORT. Exit 2 is
# the pass; anything else is a refusal.
PROBE_RENDERED="$WORKDIR/.serve-argv-probe-absent"
PROBE_ERR="$WORKDIR/serve-argv-probe.err"
probe_status=0
"$TRANSYNC_BIN" serve \
  --rendered "$PROBE_RENDERED" \
  --bind "$BIND" \
  --port "$PORT" \
  ${ALLOW_HOST:+--allow-host "$ALLOW_HOST"} >/dev/null 2>"$PROBE_ERR" || probe_status=$?
if [[ $probe_status -ne 2 ]]; then
  echo "[smoke-live] the demo server refuses these arguments (transync serve exited $probe_status)." >&2
  echo "[smoke-live] fixing them now costs nothing; after the translation it costs the translation." >&2
  sed 's/^/[smoke-live]   /' "$PROBE_ERR" >&2
  exit 1
fi
rm -f "$PROBE_ERR"

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
"$TRANSYNC_BIN" translate \
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

# The exact binary that produced this bundle, and the exact argv the probe
# above already accepted.
exec "$TRANSYNC_BIN" serve \
  --rendered "$HTML_OUT" \
  --bind "$BIND" \
  --port "$PORT" \
  ${ALLOW_HOST:+--allow-host "$ALLOW_HOST"}
