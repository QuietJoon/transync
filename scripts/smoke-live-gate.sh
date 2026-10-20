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
# Optional — each is shape-checked (unpadded model, absolute http(s) URL)
# before any compile, and only when a selected leg reads it:
#   TRANSYNC_LIVE_SMOKE_CHAT_MODEL          — overrides default (gpt-4o-mini); chat
#   TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL     — overrides default (gpt-5-mini); responses
#   TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL     — overrides default (claude-haiku-4-5); anthropic
#   TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL        — overrides default (gemini-3.8-flash); the google legs
#   TRANSYNC_OPENAI_BASE_URL                — overrides https://api.openai.com; chat, responses
#   TRANSYNC_ANTHROPIC_BASE_URL             — overrides https://api.anthropic.com; anthropic
#
# Every leg runs ONE live test, named exactly, and every selected leg's test
# must be listed among its crate's ignored tests before the first paid call
# of the run — a renamed, removed or un-ignored test refuses the run instead
# of letting it print PASS over a leg that never ran.
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
#
# LEGS is the live tests this invocation runs, as <crate>:<test>, each named
# exactly (R0012-0017). An empty filter would select whatever ignored tests
# the binary happens to hold, so under `all` a deleted or un-ignored
# responses test left chat standing in for both OpenAI surfaces. `all` is the
# six single-surface legs, by name.
NEEDS_OPENAI=0
NEEDS_ANTHROPIC=0
NEEDS_GOOGLE_VERTEX=0
NEEDS_GOOGLE_DEVELOPER=0
NEEDS_GOOGLE_VERTEX_EXPRESS=0
LEG_CHAT=transync-openai:live_chat_surface_round_trip
LEG_RESPONSES=transync-openai:live_responses_surface_round_trip
LEG_ANTHROPIC=transync-anthropic:live_messages_api_round_trip
LEG_GOOGLE_VERTEX=transync-google:vertex_live_smoke
LEG_GOOGLE_VERTEX_EXPRESS=transync-google:vertex_express_live_smoke
LEG_GOOGLE_DEVELOPER=transync-google:developer_live_smoke
case "$SURFACE" in
  chat)      NEEDS_OPENAI=1; LEGS=("$LEG_CHAT") ;;
  responses) NEEDS_OPENAI=1; LEGS=("$LEG_RESPONSES") ;;
  anthropic) NEEDS_ANTHROPIC=1; LEGS=("$LEG_ANTHROPIC") ;;
  google-vertex) NEEDS_GOOGLE_VERTEX=1; LEGS=("$LEG_GOOGLE_VERTEX") ;;
  google-developer) NEEDS_GOOGLE_DEVELOPER=1; LEGS=("$LEG_GOOGLE_DEVELOPER") ;;
  google-vertex-express) NEEDS_GOOGLE_VERTEX_EXPRESS=1; LEGS=("$LEG_GOOGLE_VERTEX_EXPRESS") ;;
  all)
    NEEDS_OPENAI=1; NEEDS_ANTHROPIC=1; NEEDS_GOOGLE_VERTEX=1; NEEDS_GOOGLE_VERTEX_EXPRESS=1; NEEDS_GOOGLE_DEVELOPER=1
    LEGS=("$LEG_CHAT" "$LEG_RESPONSES" "$LEG_ANTHROPIC" "$LEG_GOOGLE_VERTEX" "$LEG_GOOGLE_VERTEX_EXPRESS" "$LEG_GOOGLE_DEVELOPER")
    ;;
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

# R0012-0027 — each override is shape-checked for the legs that read it, and
# only for those, still before any compile (R0011-0057/0058). A leg this run
# does not call cannot spend a request on a bad value, so a value only an
# unselected leg reads is no reason to refuse the legs that do run; the run
# that selects its leg checks it.
check_leg_overrides() {
  case "$1" in
    live_chat_surface_round_trip)
      require_unpadded TRANSYNC_LIVE_SMOKE_CHAT_MODEL
      require_http_url TRANSYNC_OPENAI_BASE_URL
      ;;
    live_responses_surface_round_trip)
      require_unpadded TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL
      require_http_url TRANSYNC_OPENAI_BASE_URL
      ;;
    live_messages_api_round_trip)
      require_unpadded TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL
      require_http_url TRANSYNC_ANTHROPIC_BASE_URL
      ;;
    vertex_live_smoke | vertex_express_live_smoke | developer_live_smoke)
      require_unpadded TRANSYNC_LIVE_SMOKE_GOOGLE_MODEL
      ;;
  esac
}

for leg in "${LEGS[@]}"; do
  check_leg_overrides "${leg#*:}"
done

# R0012-0017 — libtest exits 0 when a name filter selects zero tests, so a
# leg whose test was renamed, removed or lost its #[ignore] would run nothing
# and still print PASS. Each leg's test is listed by its exact name among the
# crate's ignored tests, and EVERY selected leg is listed before the first
# billable run: under `all`, a missing anthropic or google test refuses the
# run before the OpenAI legs spend. `listed` is assigned apart from its
# `local` so a failed compile still stops the script under `set -e`.
check_leg() {
  local package="$1" name="$2"
  local listed
  listed="$(cargo test -p "$package" --test live_smoke "$name" -- --ignored --exact --list)"
  if ! grep -Fxq -- "$name: test" <<<"$listed"; then
    echo "[smoke-live-gate] expected ignored live test '$name' is missing from $package (renamed, removed, or no longer #[ignore]) — refusing before any billable call." >&2
    exit 1
  fi
}

# --test-threads=1 keeps the live calls sequential (politeness towards the
# endpoint + deterministic --nocapture logs). The repo-wide --test-threads=4
# rule is a cap, so 1 is compliant.
run_leg() {
  local package="$1" name="$2"
  TRANSYNC_LIVE_SMOKE=1 cargo test -p "$package" --test live_smoke "$name" \
    -- --ignored --exact --test-threads=1 --nocapture
}

for leg in "${LEGS[@]}"; do
  check_leg "${leg%%:*}" "${leg#*:}"
done

echo "[smoke-live-gate] running gated live smoke (${SURFACE}) — this spends real API tokens"

for leg in "${LEGS[@]}"; do
  run_leg "${leg%%:*}" "${leg#*:}"
done

echo "[smoke-live-gate] PASS — live round-trip validated."
echo "[smoke-live-gate] legs run, each by exact name: ${LEGS[*]}"
echo "[smoke-live-gate] For an interactive browser check, run scripts/smoke-live.sh"
