#!/usr/bin/env bash
# Local one-shot for transync's live OpenAI smoke + demo server.
# Wraps scripts/smoke-live.sh with the system prompt this project tests with.

set -euo pipefail

# Required: OPENAI_API_KEY. Export it from the parent shell before
# running, or source it from a gitignored .env file. Never write a key
# — not even a commented-out template — into this tracked file.

export TRANSYNC_LIVE_SYSTEM_PROMPT='You are a literary translator. Translate from {{source_language}} to {{target_language}}. Preserve markdown structure exactly.'

# Long + complex sample input (64 blocks, 63 translation units, 12 batches).
# Override from the parent shell with TRANSYNC_LIVE_INPUT=... if needed.
export TRANSYNC_LIVE_INPUT="${TRANSYNC_LIVE_INPUT:-samples/demo-complex.md}"

# Optional knobs (uncomment to override smoke-live.sh defaults):
# export TRANSYNC_OPENAI_MODEL=gpt-4o-2024-11-20
# export TRANSYNC_LIVE_TARGET_LANG=ko
# export TRANSYNC_LIVE_PORT=7470

# Bind to all interfaces so the demo server's socket is reachable from
# other machines on this LAN. From this machine, browse it at
# http://localhost:7470/ — the wildcard bind still answers only for the
# loopback authorities (127.0.0.1, [::1], localhost), because the address
# other machines reach it at cannot be derived from a bind that names no
# interface.
#
# So the bind alone is NOT enough for a browser on another machine: one
# typing this dev server's LAN address (e.g. 10.0.0.4 reaching 10.0.0.2)
# sends `Host: 10.0.0.2:7470`, which `transync serve` refuses with a 421.
# Name that authority yourself in the parent shell —
#   TRANSYNC_LIVE_ALLOW_HOST=10.0.0.2:7470 ./scripts/test.sh
# — and smoke-live.sh forwards it as --allow-host. This script cannot set
# it for you: only you know the address the other browser will type.
#
# `transync serve` also warns on stderr that this is not a loopback bind.
# That warning is about network reach, not about the Host check, and it is
# expected here — use 127.0.0.1 (the default) if you only browse locally.
export TRANSYNC_LIVE_BIND=0.0.0.0

exec "$(dirname "$0")/smoke-live.sh"
