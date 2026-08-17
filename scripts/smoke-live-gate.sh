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
# Required — per leg, checked up front so a missing key is an immediate
# refusal rather than a skipped test that reads like a pass:
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

if [[ $NEEDS_OPENAI -eq 1 && -z "${OPENAI_API_KEY:-}" ]]; then
  echo "[smoke-live-gate] OPENAI_API_KEY is not set (needed for surface '${SURFACE}')." >&2
  echo "                  export it and re-run, or pick a surface that does not need it." >&2
  exit 1
fi

if [[ $NEEDS_ANTHROPIC -eq 1 && -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "[smoke-live-gate] ANTHROPIC_API_KEY is not set (needed for surface '${SURFACE}')." >&2
  echo "                  export it and re-run, or pick a surface that does not need it." >&2
  exit 1
fi

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
