---
type: ADR
title: The WASM publish lock fails closed, and the remedy is a human
description: build-wasm.sh's publish lock has no automatic stale-owner recovery on purpose — it is held for milliseconds, only SIGKILL can strand it, and breaking a live lock nests one build's staging inside another's web/wasm, which is the corruption the lock exists to prevent.
tags: [decision, ADR-0030]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-06T00:00:00Z
status: stable
---

# ADR: The WASM publish lock fails closed, and the remedy is a human

## Context and Problem Statement

Found in Review 0011 (Issue R0011-0016, Severity: MEDIUM).
Location: `scripts/build-wasm.sh` (the `until mkdir "$PUBLISH_LOCK"` acquisition
loop and the `cleanup` EXIT trap).

The demo module is published by moving a staging directory onto `web/wasm`. `mv`
moves a directory *into* an existing destination rather than replacing it, so two
interleaved swaps would nest one run's staging inside the other's published tree.
`mkdir` on `$WEB_DIR/.wasm.publish.lock` is the atomic test-and-set that serializes
the swap; the lock is held for two renames and released by the EXIT trap.

The lock has no stale-owner recovery. If a build is killed in a way that skips its
trap, the lock directory survives and every later build waits 30 seconds and then
refuses. The question is whether the script should break a lock it believes is
abandoned.

## Decision Drivers

* The hold time is two renames — milliseconds. A concurrent acquirer that waits
  even one second is already an anomaly, and the loop waits thirty.
* `INT` and `TERM` are routed through `exit` precisely so the EXIT trap runs, so a
  stranded lock requires `SIGKILL`, a power loss, or a filesystem that lost the
  `rmdir`. It is not a routine outcome.
* Automatic staleness needs an owner identity plus a liveness proof, and this
  repository's standing rule is that a PID alone is not one: a PID must be paired
  with start identity, executable, and process group before any cleanup claim. A
  lock file carrying only a timestamp lets a slow-but-live build be broken by a
  fast one — producing exactly the nested-staging corruption the lock exists to
  prevent, and producing it silently.
* The cost of failing closed is bounded and visible: `web/wasm` is a gitignored
  build artifact, and the refusal message already names the remedy in the failure
  itself ("…or one was killed mid-swap — in the second case remove that directory
  by hand once no build is running"). A developer loses one `rmdir`.
* The cost of failing open is a corrupted published module that no test in the
  suite is positioned to notice, because the browser suite mounts whatever
  `web/wasm` contains.

## Considered Options

1. Break the lock when its mtime exceeds a threshold.
2. Write a PID/host owner file and break the lock when the owner is not alive.
3. Keep the lock fail-closed, and treat the in-message manual remedy as the
   recovery path.

## Decision Outcome

REJECT: we decided for option 3, because every automatic recovery has a wrong
branch whose consequence is the corruption the lock was added to prevent, while
the failure it avoids is a thirty-second wait followed by an error message that
tells the developer the exact command to run. Option 1 cannot distinguish a slow
build from a dead one. Option 2 would need liveness evidence this script has no
way to gather portably, and a half-proof is worse than no proof here.

Status: Implemented (no code change; the script already fails closed and already
names the remedy in its refusal).

### Implementation

No change. The acquisition loop, the thirty-second wait, the EXIT-trap release,
and the remedy sentence in the failure message are the decision.

## Consequences

* Good, because no build can ever publish into another build's staging directory
  as a side effect of an incorrect liveness guess.
* Good, because the recovery instruction reaches the person at the moment they
  need it, in the error itself, rather than in a document they would have to find.
* Bad, because a `SIGKILL`ed build leaves a lock that the next developer must
  remove by hand, and CI running this script unattended would need that step
  scripted at the CI level rather than inside `build-wasm.sh`.
