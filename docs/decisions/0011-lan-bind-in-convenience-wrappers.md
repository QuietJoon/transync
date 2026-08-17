---
type: ADR
title: Keep the LAN bind (0.0.0.0) in the personal convenience wrappers test.sh / test-long.sh
description: The demo-server LAN bind in the two convenience wrappers is a deliberate owner choice for cross-machine browsing, documented here so reviews stop re-flagging it.
tags: [decision, ADR-0011]
status: active
---

# ADR: Keep the LAN bind in the convenience wrappers

## Context and Problem Statement

Found in Review 0008 (Issue R0008-0010, Severity: High) (review archived and removed). Location:
`scripts/test.sh`, `scripts/test-long.sh`.

Both wrappers export `TRANSYNC_LIVE_BIND=0.0.0.0`, so the Python demo
server behind `scripts/smoke-live.sh` listens on every interface and
serves the translated document, alignment metadata, and directory
listings to the local network without authentication. The reviewer
recommended defaulting to `127.0.0.1` with an explicit opt-in.

## Decision Drivers

* The LAN bind exists on purpose: commit `03365d9` added
  `TRANSYNC_LIVE_BIND` specifically so the owner's second machine on the
  same network can browse the demo produced on the dev machine.
* The underlying script (`scripts/smoke-live.sh`) already defaults to
  `127.0.0.1`; only these two personal convenience wrappers force the
  LAN bind, and their in-file comments say so and how to change it.
* The served content is the owner's own translation output on a trusted
  home/dev network, not a shared or hostile environment.

## Considered Options

1. Flip the wrappers to `${TRANSYNC_LIVE_BIND:-127.0.0.1}` (rejected —
   defeats the wrappers' whole purpose; the safe default already lives
   one layer down in `smoke-live.sh`).
2. Delete the wrappers from the repo (rejected — they document the
   owner's standard invocation).
3. Keep the LAN bind and record the choice here.

## Decision Outcome

Chosen option 3 (user routing, Review 0008 gate, 2026-07-11). The
wrappers stay bound to `0.0.0.0`; `smoke-live.sh` keeps its `127.0.0.1`
default for everyone who calls it directly.

Status: No change required.

### Implementation

None. Reviewers: treat `TRANSYNC_LIVE_BIND=0.0.0.0` in
`scripts/test.sh` / `scripts/test-long.sh` as settled by this ADR.

## Consequences

* Good, because the owner's cross-machine demo workflow keeps working
  with zero flags.
* Bad, because anyone copying the wrappers verbatim onto an untrusted
  network exposes their translation output; the wrappers' comments and
  this ADR carry that warning.

## Amendment (2026-08-16) — the bind is no longer sufficient on its own

The decision stands; one consequence above has expired. Since the `Host`
check landed (R0004-0002, 2026-08-12), a bind decides which *network* can
open the socket and no longer decides which authorities the server
answers for. `serve_cmd::host::HostPolicy::new` derives nothing from a
wildcard bind but the loopback authorities it is also listening on —
`127.0.0.1`, `[::1]`, `localhost` — so a browser on the second machine
typing the dev box's LAN address sends a `Host` naming that address,
gets `Verdict::Elsewhere`, and is answered with a `421`.

So "keeps working with zero flags" now reads: keeps working with zero
flags **from this machine**, at `localhost`. Cross-machine browsing
additionally needs the authority the other browser types, named through
`TRANSYNC_LIVE_ALLOW_HOST=<host:port>`, which `scripts/smoke-live.sh`
forwards as `--allow-host`. The wrappers cannot set it themselves — the
address is the operator's to state — so their comments say so instead.

The "bad" consequence is correspondingly narrower: a wrapper copied onto
an untrusted network still publishes the socket, but a page reaching it
under a name the policy does not answer for is refused before the served
root is consulted.
