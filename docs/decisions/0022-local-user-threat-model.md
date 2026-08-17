---
type: ADR
title: A local user with write access to transync's own paths is outside the threat model
description: transync is a single-operator tool; a process already able to write into the served tree, the output directory or the cache path is not defended against, and this record is what findings resting on that premise resolve to.
tags: [decision, ADR-0022]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-13T00:00:00Z
status: stable
---

# ADR: A local user with write access to transync's own paths is outside the threat model

## Context and Problem Statement

Found in Review 0004 (Issue R0004-0034, reviewer severity Medium; verified
severity at HEAD: Low) (review archived and removed). Location: `crates/transync-core/src/cache/disk.rs`
(`DiskCache::open`).

`DiskCache` creates its log with the process umask, normally `0644`, so another
local user can read it — and what it holds is the user's translated documents.

The narrow question is whether to narrow those permissions. The larger one is
the one worth deciding, because **the same premise decided this review's
verdict on roughly a dozen findings**: R0004-0004, R0004-0010, R0004-0024,
R0004-0025, R0004-0049, R0004-0055, R0004-0057 and others each rest on "an
attacker who can already write into the served tree, the output directory, or
the cache path". Each was individually judged low or rejected on that ground,
by three consecutive verification passes, without the ground itself ever being
written down.

That is the same shape ADR-0020 addressed for hash collision: a premise the
project reasons from every round and has never stated, so every round pays to
re-derive it and the answer drifts with whoever is arguing.

## Decision Drivers

* Who transync is for. It is a CLI a person runs on their own machine against
  documents they chose, with an API key they supplied. It is not a daemon
  serving mutually distrusting users, and it holds no secret its operator does
  not already have.
* The comparison class. `cargo`'s registry cache, `pip`'s wheel cache and
  `npm`'s store all inherit the umask, and none is considered defective for it.
* The honest counter-argument, which is real: those caches hold **public
  packages**, and this one holds the user's **private documents**. That is a
  genuine difference, and it is why this is a decision rather than an
  observation.
* Cost of silence. An unstated premise in a security-shaped area is re-raised
  by every reviewer, and each re-raising costs a verification pass. Three
  rounds have now spent that cost.

## Considered Options

1. **State the premise and keep the current behavior** — record that a local
   user with write (or read) access to transync's own paths is out of scope,
   and let findings that rest on it resolve to this record.
2. **Narrow the permissions** — create the cache directory `0700` and its files
   `0600` on Unix. A few lines, no design conflict.
3. **Both** — state the premise *and* narrow the permissions, on the grounds
   that a cheap mitigation is worth taking even for an out-of-scope threat.

## Decision Outcome

**Option 1, by owner routing (2026-08-12: `R0004-0034: adr`).**

A local process that can already write into the tree transync serves, the
directory it publishes to, or the path its cache lives at is **outside this
program's threat model**. It is not defended against, and a finding whose
mechanism requires that access is a known and accepted property rather than a
defect.

Status: Implemented (the decision is the record; no code changed).

### Implementation

No code change. The cache inherits the umask as `cargo` and `pip` do.

Two limits on how far this record reaches, because a premise this broad is
easy to over-apply:

- **It is about the ATTACKER, not the failure.** A finding whose mechanism
  needs a hostile local writer resolves here. A finding where transync's own
  code loses or corrupts data on its own — a rollback deleting a peer's output,
  a guard reading a directory as a file — does not, however local the setting.
  Review 0004's R0004-0045 (an unbounded read where the project's own contract
  demanded `Read::take`) was fixed, not dismissed, and correctly so.
- **It does not reach the network.** Everything an attacker reaches without
  local access stays in scope: the `Host` validation that closed DNS rebinding
  (R0004-0002) and the redirect policy that stopped an API key crossing an
  origin (R0004-0001) both landed in this same round.

## Consequences

* Good, because the premise is now answerable by citation. The dozen findings
  above, and their successors, resolve without another verification pass.
* Good, because it states the deployment boundary plainly rather than leaving
  it implied by a pattern of dismissals.
* Bad, because it is a real limitation: transync's cache should not be placed
  on a shared multi-user machine where other users must not read the operator's
  documents. If that becomes a supported deployment, this ADR is what has to be
  superseded — and option 2 above is the ready-made first step, deliberately
  left costed and un-taken rather than unexamined.
* Bad, because "outside the threat model" can be over-read as "we do not care".
  The two limits in *Implementation* exist to stop that, and a future reviewer
  citing this record should be held to them.
