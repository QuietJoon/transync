#!/usr/bin/env bash
# transync — gated live-endpoint smoke (OI-0030).
#
# Runs the machine-asserted live round-trip tests against the real APIs:
#   crates/transync-openai/tests/live_smoke.rs     (chat, responses)
#   crates/transync-anthropic/tests/live_smoke.rs  (anthropic — DCR-0029)
# Each test does one full translate() round-trip, so a pass is evidence that
# the whole validation stack (schema, ID-set, per-kind, fragment reparse,
# inline protection, full reparse) accepted a genuine provider response.
#
# This script is the human opt-in: it supplies both gates the tests
# require (#[ignore] via --ignored, TRANSYNC_LIVE_SMOKE=1 via the
# environment of its own child process only). Nothing else in the repo
# turns them on.
#
# Usage: scripts/smoke-live-gate.sh [chat|responses|anthropic|all]   (default: all)
#
# Required — per leg, checked up front so a missing or unusable key is an
# immediate refusal rather than a skipped test that reads like a pass:
#   OPENAI_API_KEY                          — for chat / responses / all
#   ANTHROPIC_API_KEY                       — for anthropic / all
#
# Optional:
#   TRANSYNC_LIVE_SMOKE_CHAT_MODEL          — overrides default (gpt-4o-mini)
#   TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL     — overrides default (gpt-5-mini)
#   TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL     — overrides default (claude-haiku-4-5)
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
# its own key: the two providers are separate accounts and separate spend, so
# "all" without one of them is a misconfiguration to name now rather than a
# leg that quietly reports nothing later.
NEEDS_OPENAI=0
NEEDS_ANTHROPIC=0
case "$SURFACE" in
  chat)      NEEDS_OPENAI=1 ;;
  responses) NEEDS_OPENAI=1 ;;
  anthropic) NEEDS_ANTHROPIC=1 ;;
  all)       NEEDS_OPENAI=1; NEEDS_ANTHROPIC=1 ;;
  *)
    echo "[smoke-live-gate] unknown surface '${SURFACE}' (expected chat|responses|anthropic|all)" >&2
    exit 1
    ;;
esac

# R0011-0011 — `-z` alone was not the check this gate advertises. A key that is
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
    echo "[smoke-live-gate] $var_name is whitespace only — it cannot authenticate." >&2
    exit 1
  fi
  if [[ "$value" == *[[:cntrl:]]* ]]; then
    echo "[smoke-live-gate] $var_name contains a control character (a trailing newline from" >&2
    echo "                  \`\$(cat …)\` is the usual one) — it cannot go in an HTTP header." >&2
    exit 1
  fi
}

# R0011-0057 — an override the endpoint cannot resolve costs a compile and a
# real request to discover. The model-name rule itself stays with the provider
# (the tests pass the value through as-is); this refuses only the padding a
# shell copy-paste adds, which is invisible in the failure message that follows.
# An empty value is left alone: both live_smoke suites read it as "unset" and
# use their own cheap default.
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

# Checked for every surface, not per leg: these are the knobs an operator edits
# once and forgets, and naming a bad one now is free whichever leg reads it.
require_unpadded TRANSYNC_LIVE_SMOKE_CHAT_MODEL
require_unpadded TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL
require_unpadded TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL
require_http_url TRANSYNC_OPENAI_BASE_URL
require_http_url TRANSYNC_ANTHROPIC_BASE_URL

echo "[smoke-live-gate] running gated live smoke (${SURFACE}) — this spends real API tokens"

# --test-threads=1 keeps the live calls sequential (politeness towards the
# endpoint + deterministic --nocapture logs). The repo-wide --test-threads=4
# rule is a cap, so 1 is compliant.
run_leg() {
  local package="$1" filter="$2"
  TRANSYNC_LIVE_SMOKE=1 cargo test -p "$package" --test live_smoke ${filter:+"$filter"} \
    -- --ignored --test-threads=1 --nocapture
}

case "$SURFACE" in
  chat)      run_leg transync-openai    live_chat_surface_round_trip ;;
  responses) run_leg transync-openai    live_responses_surface_round_trip ;;
  anthropic) run_leg transync-anthropic "" ;;
  all)
    run_leg transync-openai    ""
    run_leg transync-anthropic ""
    ;;
esac

echo "[smoke-live-gate] PASS — live round-trip validated."
echo "[smoke-live-gate] For an interactive browser check, run scripts/smoke-live.sh"
