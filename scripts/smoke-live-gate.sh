#!/usr/bin/env bash
# transync — gated live-endpoint smoke (OI-0030).
#
# Runs the machine-asserted live round-trip tests against the real APIs:
#   crates/transync-openai/tests/live_smoke.rs     (chat, responses)
#   crates/transync-anthropic/tests/live_smoke.rs  (anthropic — DCR-0029)
#   crates/transync-google/tests/live_smoke.rs     (google-vertex, google-vertex-express, google-developer — DCR-0058, DCR-0060)
# Each test does one full translate() round-trip, so a pass is evidence that
# the whole validation stack (schema, ID-set, per-kind, fragment reparse,
# inline protection, full reparse) accepted a genuine provider response.
#
# This script is the human opt-in: it supplies both gates the tests
# require (#[ignore] via --ignored, TRANSYNC_LIVE_SMOKE=1 via the
# environment of its own child process only). Nothing else in the repo
# turns them on.
#
# Usage: scripts/smoke-live-gate.sh [chat|responses|anthropic|google-vertex|google-vertex-express|google-developer|all]
#        (default: all)
#
# Required — per leg, checked up front so a missing key or project is an
# immediate refusal rather than a skipped test that reads like a pass.
# Vertex ADC is loaded by the test's adapter constructor:
#   OPENAI_API_KEY                          — for chat / responses / all
#   ANTHROPIC_API_KEY                       — for anthropic / all
#   GOOGLE_CLOUD_PROJECT                    — for google-vertex / all (ADC also required)
#   GEMINI_API_KEY                          — for google-developer / all
#   GOOGLE_API_KEY                          — for google-vertex-express / all (Vertex express mode, no ADC)
#
# Each key may instead live in a gitignored file at the repository root, which
# this script loads when the variable is unset (every whitespace character
# stripped):
#   OPENAI_API_KEY    <- .openai.api_key
#   ANTHROPIC_API_KEY <- .anthropic.api_key
#   GEMINI_API_KEY    <- .gemini.api_key
#   GOOGLE_API_KEY    <- .google_vertex.api_key
#
# VERIFIED — google-vertex-express. The adapter sends the express-mode key as an
# `x-goog-api-key` HTTP header rather than the `?key=` query parameter Google's
# own published examples use, because the header is the only form that keeps the
# key out of URLs, logs and proxies (DCR-0030). A live round-trip against
# `gemini-3.8-flash` accepted the header, so the documented query form is one
# spelling of this key and not the only one; no `?key=` fallback is needed.
#
# Optional:
#   TRANSYNC_LIVE_SMOKE_CHAT_MODEL          — overrides default (gpt-4o-mini)
#   TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL     — overrides default (gpt-5-mini)
#   TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL     — overrides default (claude-haiku-4-5)
#   TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL        — overrides default (gemini-3.8-flash)
#   TRANSYNC_OPENAI_BASE_URL                — overrides https://api.openai.com
#   TRANSYNC_ANTHROPIC_BASE_URL             — overrides https://api.anthropic.com
#
# TRACE: OI-0030
# TRACE: DCR-0029
# TRACE: SCN-12

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

SURFACE="${1:-all}"

# Which provider crates this invocation will actually call. Each leg demands
# its own credentials or project: the providers are separate accounts and spend, so
# "all" without one of them is a misconfiguration to name now rather than a
# leg that quietly reports nothing later.
NEEDS_OPENAI=0
NEEDS_ANTHROPIC=0
NEEDS_GOOGLE_VERTEX=0
NEEDS_GOOGLE_DEVELOPER=0
NEEDS_GOOGLE_VERTEX_EXPRESS=0
case "$SURFACE" in
  chat)      NEEDS_OPENAI=1 ;;
  responses) NEEDS_OPENAI=1 ;;
  anthropic) NEEDS_ANTHROPIC=1 ;;
  google-vertex) NEEDS_GOOGLE_VERTEX=1 ;;
  google-developer) NEEDS_GOOGLE_DEVELOPER=1 ;;
  google-vertex-express) NEEDS_GOOGLE_VERTEX_EXPRESS=1 ;;
  all)       NEEDS_OPENAI=1; NEEDS_ANTHROPIC=1; NEEDS_GOOGLE_VERTEX=1; NEEDS_GOOGLE_VERTEX_EXPRESS=1; NEEDS_GOOGLE_DEVELOPER=1 ;;
  *)
    echo "[smoke-live-gate] unknown surface '${SURFACE}' (expected chat|responses|anthropic|google-vertex|google-vertex-express|google-developer|all)" >&2
    exit 1
    ;;
esac

# A credential may live in a gitignored key file at the repository root
# instead of the environment — `.openai.api_key` has been managed that way by
# hand since the first live run. This makes the convention real rather than a
# habit: when the variable is unset and the file exists, its contents become
# the variable with every whitespace character stripped. That strip is the
# point — the trailing newline a `$(cat …)` picks up is exactly what
# `require_key` below refuses, and refusing an operator's own file for a
# newline they never typed is a bad trade.
#
# The file is never echoed. `.gitignore` already covers `*.api_key`.
load_key_file() {
  local var_name="$1" file="$2"
  [[ -n "${!var_name:-}" ]] && return 0
  [[ -r "$REPO_ROOT/$file" ]] || return 0
  local value
  value="$(tr -d "[:space:]" < "$REPO_ROOT/$file")"
  [[ -n "$value" ]] || return 0
  printf -v "$var_name" '%s' "$value"
  export "${var_name?}"
  echo "[smoke-live-gate] $var_name loaded from $file" >&2
}

load_key_file OPENAI_API_KEY    .openai.api_key
load_key_file ANTHROPIC_API_KEY .anthropic.api_key
load_key_file GEMINI_API_KEY    .gemini.api_key
load_key_file GOOGLE_API_KEY    .google_vertex.api_key

# R0011-0011 — `-z` alone was not the check this gate advertises. A value that is
# present but whitespace-only, or that carries the trailing newline a
# `$(cat ~/.key)` picks up, satisfies `-z` and then fails at the endpoint as an
# authentication error — after a full test-target compile, and reading like the
# provider's fault rather than the shell's. A control character cannot go into
# an HTTP header at all, so it is refused for a different reason and said so.
require_key() {
  local var_name="$1"
  local value="${!var_name:-}"
  if [[ -z "$value" ]]; then
    echo "[smoke-live-gate] $var_name is not set (needed for surface '${SURFACE}')." >&2
    echo "                  export it and re-run, or pick a surface that does not need it." >&2
    exit 1
  fi
  if [[ -z "${value//[[:space:]]/}" ]]; then
    echo "[smoke-live-gate] $var_name is whitespace only — it cannot be used." >&2
    exit 1
  fi
  if [[ "$value" == *[[:cntrl:]]* ]]; then
    echo "[smoke-live-gate] $var_name contains a control character (a trailing newline from" >&2
    echo "                  \`\$(cat …)\` is the usual one) — it cannot be used in an API request." >&2
    exit 1
  fi
}

# R0011-0057 — an override the endpoint cannot resolve costs a compile and a
# real request to discover. The model-name rule itself stays with the provider
# (the tests pass the value through as-is); this refuses only the padding a
# shell copy-paste adds, which is invisible in the failure message that follows.
# An empty value is left alone: each live_smoke suite reads it as "unset" and
# uses its own default.
require_unpadded() {
  local var_name="$1"
  local value="${!var_name:-}"
  [[ -n "$value" ]] || return 0
  local trimmed="${value#"${value%%[![:space:]]*}"}"
  trimmed="${trimmed%"${trimmed##*[![:space:]]}"}"
  if [[ "$value" != "$trimmed" ]]; then
    echo "[smoke-live-gate] $var_name has leading or trailing whitespace: '$value'" >&2
    exit 1
  fi
}

# R0011-0058 — same trade for the endpoint overrides. `Url::parse` in the tests
# stays authoritative; this is the shape that makes the compile worth starting.
require_http_url() {
  local var_name="$1"
  local value="${!var_name:-}"
  [[ -n "$value" ]] || return 0
  if ! [[ "$value" =~ ^https?://[^[:space:]]+$ ]]; then
    echo "[smoke-live-gate] $var_name must be an absolute http(s) URL: '$value'" >&2
    exit 1
  fi
}

if [[ $NEEDS_OPENAI -eq 1 ]]; then
  require_key OPENAI_API_KEY
fi

if [[ $NEEDS_ANTHROPIC -eq 1 ]]; then
  require_key ANTHROPIC_API_KEY
fi

if [[ $NEEDS_GOOGLE_VERTEX -eq 1 ]]; then
  require_key GOOGLE_CLOUD_PROJECT
fi

if [[ $NEEDS_GOOGLE_DEVELOPER -eq 1 ]]; then
  require_key GEMINI_API_KEY
fi

if [[ $NEEDS_GOOGLE_VERTEX_EXPRESS -eq 1 ]]; then
  require_key GOOGLE_API_KEY
fi

# Checked for every surface, not per leg: these are the knobs an operator edits
# once and forgets, and naming a bad one now is free whichever leg reads it.
require_unpadded TRANSYNC_LIVE_SMOKE_CHAT_MODEL
require_unpadded TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL
require_unpadded TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL
require_unpadded TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL
require_http_url TRANSYNC_OPENAI_BASE_URL
require_http_url TRANSYNC_ANTHROPIC_BASE_URL

echo "[smoke-live-gate] running gated live smoke (${SURFACE}) — this spends real API tokens"

# --test-threads=1 keeps the live calls sequential (politeness towards the
# endpoint + deterministic --nocapture logs). The repo-wide --test-threads=4
# rule is a cap, so 1 is compliant.
run_leg() {
  local package="$1" filter="$2"
  # libtest exits 0 when a name filter selects zero tests. Check the ignored
  # test inventory before the billable run so a renamed/removed leg cannot
  # make the release gate print PASS without exercising an API.
  local selected
  selected=$(cargo test -p "$package" --test live_smoke ${filter:+"$filter"} -- --ignored --list)
  if ! printf '%s\n' "$selected" | awk '/: test$/ { found = 1 } END { exit !found }'; then
    echo "[smoke-live-gate] no ignored live test matched '${filter:-<all>}' in $package." >&2
    exit 1
  fi
  TRANSYNC_LIVE_SMOKE=1 cargo test -p "$package" --test live_smoke ${filter:+"$filter"} \
    -- --ignored --test-threads=1 --nocapture
}

case "$SURFACE" in
  chat)      run_leg transync-openai    live_chat_surface_round_trip ;;
  responses) run_leg transync-openai    live_responses_surface_round_trip ;;
  anthropic) run_leg transync-anthropic "" ;;
  google-vertex) run_leg transync-google vertex_live_smoke ;;
  google-developer) run_leg transync-google developer_live_smoke ;;
  google-vertex-express) run_leg transync-google vertex_express_live_smoke ;;
  all)
    run_leg transync-openai    ""
    run_leg transync-anthropic ""
    run_leg transync-google vertex_live_smoke
    run_leg transync-google vertex_express_live_smoke
    run_leg transync-google developer_live_smoke
    ;;
esac

echo "[smoke-live-gate] PASS — live round-trip validated."
echo "[smoke-live-gate] For an interactive browser check, run scripts/smoke-live.sh"
