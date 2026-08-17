---
type: ADR
title: The output publication protocol — a kernel lock, a surviving marker, and own-pid-only reclamation
description: Publication serializes on an OS file lock rather than a lease file, the lock marker deliberately outlives the run, contention waits instead of failing or stealing, and only staging temps carrying the running pid are reclaimed; recorded here because twelve findings across two review rounds have argued each of these back.
tags: [decision, ADR-0024]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-13T00:00:00Z
status: stable
---

# ADR: The output publication protocol

## Context and Problem Statement

Twelve findings across two independent review rounds have argued against one
piece or another of the way `transync` publishes its output files:

| Round | Findings | Where the rejection was recorded |
|---|---|---|
| Review 0002 (2026-08-07; review archived and removed) | R0002-0024, R0002-0027, R0002-0030 | DCR-0021, `contracts.md` §6 |
| Review 0004 (2026-08-11; review archived and removed) | R0004-0004, R0004-0053, R0004-0055, R0004-0058, R0004-0060, R0004-0061, R0004-0062, R0004-0064, R0004-0065 | in-code rustdoc, `contracts.md` §6, ticket `8792b7` |

Location: `crates/transync-cli/src/output.rs`,
`crates/transync-cli/src/output/lock.rs`,
`crates/transync-cli/src/translate_cmd/publish.rs`.

The protocol's *observable* rules are in `contracts.md` §6 and its *change*
history is in DCR-0006, DCR-0011 and DCR-0021. What has had no home is the set of
**decisions** behind them — why a kernel lock rather than a lease file, why the
marker is not unlinked, why a foreign pid's staging temp is left alone. A
reviewer who wants that reasoning currently has to read a change record, and a
change record is by definition a transient category: once its change is
absorbed, the pruning rules move it to an archive. This record puts the
decisions in the register that does not move.

## Decision Drivers

* Two runs aimed at the same outputs were collision-safe while they *wrote*
  (pid-qualified temp names) and unprotected while they *published* — phase 2 is
  a sequence of independent renames, and two interleaved sequences leave a
  bundle whose `out.md`, alignment map and `index.html` come from different
  translations, each individually well-formed and undetectable afterwards.
* Every mechanism that answers "what if the lock holder died" in user space is
  machinery: a TTL, an mtime staleness rule, an atomic steal protocol, and a
  wait bound longer than a legitimate publish but shorter than an operator's
  patience.
* Deleting another live run's staging directory is a worse failure than leaving
  an inert file on disk.
* `transync` is a local tool under ADR-0022's threat model: the peer processes
  contending for these paths are the user's own runs, not adversaries.

## Considered Options

1. A `create_new` lease file holding a pid and a timestamp, with a staleness
   rule and a steal protocol.
2. An OS file lock (`std::fs::File::{lock, try_lock}`) on a zero-byte marker,
   with the marker left in place.
3. No publication lock — document that concurrent runs into one output set are
   unsupported.

## Decision Outcome

**Option 2.** The decisions it commits to, each of which a review round has
since argued against:

**The lock is the kernel's, not ours.** `File::lock` releases when the
descriptor closes — including on `SIGKILL` and across a reboot — so the
"holder died" question needs no machinery at all. Being per open file
description, it excludes two *threads* of one process exactly as it excludes
two processes. It costs one dependency-free API stabilized in Rust 1.89, which
`transync-cli` declares as its `rust-version`.

**Contention waits. It does not fail, and it does not steal.** Serializing
behind a peer publication is the wanted behavior, not a degraded mode — the
run announces the wait on stderr, once per contended directory, and then
blocks. A timeout would
reintroduce exactly the lease-file machinery the kernel lock was chosen to
avoid (R0002-0027, R0004-0058).

**The marker file survives the run.** Unlinking a lock file on release is the
classic way to reintroduce the race it was meant to close: a later run creates
a fresh inode at the same path and locks *that* while a waiter still holds the
old one, and both are then "exclusive". The file is zero bytes and both
directory guards recognize it, so leaving it is cheap and correct
(R0004-0061, R0004-0064).

**A granted waiter revalidates the inode.** A marker can still leave its
path — a failed run's rollback unlinks the marker of an empty directory level
it created, and an `--out-dir` replace deliberately renames a locked target
away (ti `40e2a5`), so this is an ordinary path rather than an edge case — and
therefore `PublishLock::acquire` verifies after the grant that the marker path
still names the inode it locked, and retries onto the current marker if not
(ticket `8792b7`). On Windows this revalidation is
skipped: `std` exposes no portable file identity — `file_index` is documented
as not guaranteed — and the residual exposure is named in `lock.rs` rather than
papered over (R0004-0060).

**Only own-pid staging temps are reclaimed.** A `*.tmp.<pid>` file or a
`.<name>.staging.<pid>[.<token>]` directory is removed only when it carries the
running process's own pid, where it can only be a crashed predecessor whose pid
the OS reused. A foreign pid is not evidence of death. For the `*.tmp.<pid>`
files, the scan that sees them runs *before* the lock is taken, so "no live
peer" is not something it can establish — and reclaiming under the lock would
only be sound for the directories this run happens to lock, which is not the
set that accumulates leftovers (R0002-0030, R0004-0004, R0004-0055). The
`--out-dir` staging trees *are* reclaimed under the lock, and the rule does not
widen there: what authorizes a deletion is "this can only be a crashed
predecessor's", never an inference from the directory's state, and a backup
sibling — which can be the only copy of the operator's previous output — is
never reclaimed at all, whatever pid it carries.

**Replacing an existing target is crash-safe, not atomic.** `rename` cannot
replace a non-empty directory in place, so replacement is two renames with a
reader-visible missing-target window, and a crash between them can leave the
backup directory behind. The window is named in the `--out-dir` help text, and
both facts in `contracts.md` §6 and `persistence-and-files.md` — the contract
was corrected rather than the code (R0002-0024).

**A failed directory flush is advisory, not fatal.** The payload bytes are
already `sync_all`'ed; the directory entry's durability is what is at risk, so
the run prints a `transync: note:` line and succeeds (R0004-0062).

Status: Implemented.

### Implementation

Shipped across DCR-0011 (staged fileset commit), DCR-0021 (the lock, the
honest replacement wording, non-reclaimed temps), and the `8792b7` waiter
revalidation. `contracts.md` §6 carries the observable contract; this record
carries the reasoning.

## Consequences

* Good, because the crash-recovery question is answered by the kernel, so the
  protocol has no TTL, no steal, and no clock dependence.
* Good, because every one of the seven decisions above has a written reason, so
  a future round that re-raises one is answered by citation rather than by
  re-derivation — which is what the previous twelve findings cost.
* Bad, because two of them are visible residue a user can trip over: a marker
  file that stays, and a foreign-pid staging temp that is never cleaned. Both
  are documented, and the run names the second one when it leaves it behind.
* Bad, because the Windows waiter has a narrower guarantee than the Unix one,
  and the gap is bounded by documentation rather than by code.
