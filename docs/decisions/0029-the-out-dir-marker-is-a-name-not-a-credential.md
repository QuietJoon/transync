---
type: ADR
title: The out-dir ownership marker is a name, not a credential
description: --out-dir ownership is decided by the marker file's presence and file type; its body is documentation for a human, never authenticated, because the fallback for an unmarked tree is already the conservative complete-fileset check.
tags: [decision, ADR-0029]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-06T00:00:00Z
status: stable
---

# ADR: The out-dir ownership marker is a name, not a credential

## Context and Problem Statement

Found in Review 0011 (Issue R0011-0003, Severity: HIGH).
Location: `crates/transync-cli/src/output/preflight.rs` (the `owned |= name == OUT_DIR_MARKER_NAME` arm of the sparse-tree scan, and `is_transync_control_file`).

`--out-dir` may replace a directory without `--force` when transync recognizes the
directory as its own publication. Recognition is `file_type.is_file() && name ==
OUT_DIR_MARKER_NAME` — the filename and the file type, nothing else. The canonical
`OUT_DIR_MARKER_BODY` that `publish::publish_out_dir` writes into the marker is
never read back during preflight.

The reviewer reads that as an authorization hole: any regular file named
`.transync-out-dir`, whatever its contents, authorizes replacing a sparse
directory transync did not publish. The question this record settles is whether
the marker's **body** is part of its contract.

## Decision Drivers

* The marker exists to answer one question — "did a `--out-dir` publication produce
  this tree?" — for a directory path **the operator named on the command line**
  (ti `66339b`; OI-0036).
* The fallback is not permissive. A directory carrying no marker is not refused
  outright: it goes through the conservative complete-fileset check, and only a
  tree that *is* a complete transync output set is replaceable without `--force`.
  So the marker is a fast path over a check that already stands on its own.
* OI-0036 action 3 chose that conservatism deliberately, and a byte-exact body
  compare would work against it: a marker whose newlines were converted by a
  checkout, an editor, or an archive round-trip would stop being recognized, and
  a tree transync really did publish would start demanding `--force`. That is a
  false refusal introduced to close a case the fileset check already covers.
* The threat this would defend against requires an actor who can create a file
  inside the operator's chosen output directory. Such an actor can equally create
  the whole complete fileset, which defeats a body compare too. The marker cannot
  be made into a security boundary by reading more of it.
* `OUT_DIR_MARKER_BODY` is prose addressed to a human who opens the file to find
  out what it is. Turning it into a compared token would make a documentation
  string load-bearing, and future edits to that prose would silently change
  ownership semantics.

## Considered Options

1. Require an exact, bounded marker-body match before treating a directory as
   owned; fall through to the complete-fileset check otherwise (the reviewer's
   recommendation, and the shape carried in `reviews/0011.patch`).
2. Keep presence-and-file-type as the whole contract, and record that it is the
   contract rather than an oversight.
3. Authenticate the marker (HMAC or similar over the published set).

## Decision Outcome

REJECT: we decided for option 2, because the marker is a *naming* mechanism whose
failure mode is a collision, not an *authentication* mechanism whose failure mode
is forgery — and the collision case is already handled by the complete-fileset
check that runs when no marker is found. Option 1 buys nothing against an actor
who can write into the directory while introducing a real false-refusal path that
OI-0036 action 3 explicitly designed against. Option 3 has no key material to
stand on and no threat model asking for it.

Status: Implemented (no code change; `docs/architecture/contracts.md` §6 now states
the contract, and this record carries the rationale).

### Implementation

No behavioural change. `contracts.md` §6 gained a paragraph saying that the
`--out-dir` ownership marker is recognized by name and file type, that its body is
documentation and is never compared, and that an unmarked directory falls through
to the complete-fileset check rather than being refused.

## Consequences

* Good, because a marker that survives a checkout, an editor, or a line-ending
  conversion keeps working, and no run is refused for a cosmetic difference in a
  prose file.
* Good, because the body stays editable documentation instead of becoming a
  compared token whose wording is load-bearing.
* Bad, because a stray regular file that happens to be named `.transync-out-dir`
  inside an operator-named output directory does grant the fast path. The record
  now says so out loud, which is the difference between a known contract and a
  silent hole.
* If the ownership question is ever asked about a directory the operator did
  **not** name — a discovered or inherited path — this record does not cover that
  case and must be revisited before the marker is trusted there.
