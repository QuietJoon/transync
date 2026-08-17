---
type: DCR
title: Publication lock over output directories, honest --out-dir replacement wording, and non-reclaimed staging temps
description: Output publication takes an exclusive OS file lock on every directory it writes into, so two concurrent runs serialize instead of interleaving their rename passes; the --out-dir help text stops calling an existing-target replacement atomic; and the contract stops promising automatic reclamation of foreign-pid staging temps.
tags: [change, project-control, DCR-0021]
status: active
---

# DCR-0021: Publication lock, honest replacement wording, non-reclaimed temps

- **Date:** 2026-08-06
- **Source:** Review-0001 findings R0001-0034, R0001-0035, R0001-0036 (ticket `f41f1652`)
- **Affected records:** DCR-0011 (staged fileset commit / `--out-dir`) — its
  design stands; this record adds the concurrency guarantee it lacked and
  corrects two claims made around it.

## What Changed

- **A publication holds an exclusive lock on every directory it writes
  into.** `write_fileset_atomic` and `publish_out_dir` take an OS file lock
  (`std::fs::File::{lock, try_lock}`) on a zero-byte
  `.transync-publish.lock` marker in each destination directory — for
  `--out-dir`, in the directory holding the target, where the staging and
  backup siblings already live. The lock spans the foreign-file guard, the
  staging pass and the whole rename pass. Directories are locked in
  resolved-path order, so runs with partially overlapping output sets cannot
  deadlock. Contention waits (and says so on stderr) rather than failing.
- **The `--out-dir` help text no longer says "atomically".** Publishing onto
  a fresh target is one atomic rename; replacing an existing target is
  crash-safe but *not* atomic, because `rename` cannot replace a non-empty
  directory in place. The help text, `contracts.md` §6 and
  `persistence-and-files.md` all now name the reader-visible
  missing-target window between the two renames.
- **Staging temps from other pids are documented as preserved, not
  reclaimed.** `contracts.md` §6 previously promised that "stale
  `*.tmp.<pid>` leftovers from an interrupted prior run are removed
  automatically"; the scanner only ever removed temps carrying its *own*
  pid. The contract now states the real rule, gives the R0008-0007 reason,
  and names the manual remedy — and the run prints one line whenever it
  leaves such a file behind.

## Why

Two runs aimed at the same outputs were collision-safe while they *wrote*
(pid-qualified temp names) and unprotected while they *published*: phase 2 is
a sequence of independent renames, and two interleaved sequences leave a
bundle whose `out.md`, alignment map and `index.html` come from different
translations. Nothing in the pipeline detects that afterwards — the artifacts
are individually well-formed.

**Why an OS file lock and not a lease file.** The obvious pure-`std`
alternative is a `create_new` lock file holding a pid and a timestamp. It has
to answer "what if the holder died", and every answer is machinery: a TTL, an
mtime-based staleness rule, an atomic steal protocol, and a wait bound that
has to be longer than a legitimate publish but shorter than an operator's
patience. `File::lock` answers it in the kernel — the lock is released when
the descriptor closes, including on `SIGKILL` and across a reboot — and, being
per open file description, it excludes two *threads* of one process exactly as
it excludes two processes. It costs one dependency-free API stabilized in Rust
1.89, which `transync-cli` now declares as its `rust-version` (library members
keep the workspace floor).

**Why the marker file survives the run.** Unlinking a lock file on release is
the classic way to reintroduce the race it was meant to close: a later run
creates a fresh inode at the same path and locks *that*, while a waiting run
still holds the old one. Both are then "exclusive". The file is zero bytes and
both directory guards recognize it, so leaving it is cheap and correct.

**Why the temps are still not reclaimed.** The scan that sees them runs before
the lock is taken, and a foreign pid is not evidence of death — deleting a
live run's staging is worse than leaving an inert file. Reclaiming them under
the lock would only be sound for directories this run happens to lock, which
is not the same set as the directories that accumulate leftovers. Between "fix
the code" and "fix the contract", the contract was the thing that was wrong.

## Affected Areas

- `crates/transync-cli/src/output.rs`, `crates/transync-cli/src/output/lock.rs` (new)
- `crates/transync-cli/src/translate_cmd.rs` (`--out-dir` help), `.../translate_cmd/publish.rs` (notice channel)
- `crates/transync-cli/Cargo.toml` (`rust-version = "1.89"`)
- `docs/architecture/contracts.md` §6, `docs/architecture/persistence-and-files.md`
- `crates/transync-cli/tests/cli_smoke.rs` (two concurrent CLI processes)

## Migration / Follow-up

- **Operator-visible:** output directories gain a `.transync-publish.lock`
  file that is never deleted. Tooling that enumerates a bundle directory (a
  packaging step, an rsync filter, a "is this dir clean" check) should treat
  it the way it treats `.DS_Store`. Publishing the bundle to a web server
  does not need it.
- **Behavioral:** two runs sharing an output directory now take turns.
  A run that would previously have raced now waits; nothing new fails.
- **Known boundary:** the lock covers runs publishing *into* the same
  directory. A `--out-dir` publish that replaces a directory another run is
  publishing *into* (nested transync output trees) is still unprotected, as
  is a peer built before this lock existed.

## Appended note — 2026-08-10: the one caller that unlinks a marker, and its probe

Ticket `92abf5`, carrying Review-0003 finding R0003-0001. The design above is
unchanged; this note records the one place it was not being honored, and what
now honors it.

"Why the marker file survives the run" is the guarantee. One caller has been an
exception to it since R0008-0041: `write_fileset_atomic` records the directory
levels it creates so a failed staging leaves no residue, and a level it created
and left empty contains nothing but the lock marker — so the rollback unlinks
that marker in order to `remove_dir` the level. R0002-0002 bounded that to
**sole occupancy**: with anything else in the directory the removal degrades to
a no-op, which is what stopped a rollback from deleting a peer's committed
output.

Sole occupancy answers *has a peer published here*. It cannot answer *is a peer
about to*. The rollback deliberately runs after `drop(lock)` — the ordering that
keeps the unlink portable — and that is precisely the moment a peer blocked in
`file.lock()` acquires the lock, having written nothing yet. Unlinking there
reintroduced this record's own race: the peer goes on locking an unlinked inode
while the next run creates a fresh marker at the same path and locks that, so
two runs are simultaneously "the exclusive publisher" of one directory and
interleave their renames. The lost run the ticket originally described (a clean
`ENOENT`) was the mild half of it.

**What changed.** The rollback now asks the lock instead of inferring from the
directory listing: `lock::hold_marker_for_removal` opens the marker read-only
and `try_lock`s it, never waiting, and the successful hold is kept across the
`remove_file` so nothing can slip in between. A marker a peer holds — or one the
probe cannot decide about at all (vanished, unopenable, lock call errored) —
leaves the whole level alone, marker and directory both. The choice is
deliberately asymmetric: an empty directory left behind is inert, a directory
removed out from under the run that just claimed it is not. Drop-then-roll-back
is untouched, so nothing here depends on unlinking an open file.

**Not closed by this note.** A peer *queued* inside a blocking `lock()` call is
invisible to any probe — no advisory-lock API reports waiters — so a rollback
that wins the race back to the lock can still unlink the marker that peer is
queued on. Closing that needs the waiter side rather than the remover side:
after the lock is granted, re-check that the marker path still names the inode
that was locked, and retry onto the current one if it does not. That is a
change to `PublishLock::acquire`'s mechanism, not to the rollback, and is
tracked separately.

## Appended note — 2026-08-10: the waiter revalidates the marker it was granted

Ticket `8792b7`, the residual the note above left open on purpose. The design is
still "the marker file survives the run"; what changes is that a run granted the
lock no longer assumes the marker it locked is still the marker at that path.

The remover-side probe closed the case where a peer *holds* the lock. It could
not close the case where a peer is *queued*: POSIX advisory locks report no
waiters, so a rollback that wins the race back to the lock legitimately reads
"free" while a peer sits inside `file.lock()` on that inode, and unlinks it.
Three runs then produce the violation this record exists to prevent — A unlinks
the marker and removes the level, B is granted the orphan, C recreates the
directory and locks a fresh marker at the same path, and B and C interleave
their renames as two "exclusive" publishers of one directory.

**What changed.** `PublishLock::acquire` goes through a new `lock_marker`, which
compares the `dev`+`ino` of the descriptor it holds against the `dev`+`ino` of
`<dir>/.transync-publish.lock` after the lock is granted. A mismatch — including
a path that names nothing — means the exclusion guards an inode nobody can
reach, so the descriptor is dropped and the current marker is locked instead,
bounded at 16 reacquisitions and reported as an error if that is ever exhausted.
The contention notice is emitted at most once however many passes it takes: the
operator is being told they are queued, not how the queue is implemented.
`hold_marker_for_removal` gained the same check, so the rollback cannot unlink a
replacement in place of the file it probed.

This is what makes **every** unlink safe rather than only the ones a probe can
see, which is also what let ticket `40e2a5` lock a directory an `--out-dir`
publish is about to rename away: a waiter left holding that target's old marker
retries onto the marker in the tree that replaced it.

**Platform.** `dev`+`ino` is `std::os::unix::fs::MetadataExt`. Windows has no
guaranteed file identity in `std`, so the check compiles to a no-op there and
the lock behaves exactly as it did before this note — the shape
`preserve_target_permissions` already uses. The exposure that leaves is the
three-run sequence above, whose window is a few syscalls wide.

## Appended note — 2026-08-10: the nested boundary is closed, not accepted

Ticket `40e2a5`. "Known boundary" above named one case this record left
unprotected: run A publishes `--out-dir X` — locking the directory that *holds*
X, where its staging and backup siblings live — while run B publishes
`--output X/out.md --map X/alignment.json`, locking X itself. Different inodes,
so nothing serialized them, and A's swap either takes B's committed files away
with the backup or leaves B writing into A's fresh tree. That is exactly the
failure this record set out to close for the flat case, reached through one more
level of nesting.

**What changed.** Neither mode's lock moved; each one reaches one directory
further, to where the other mode already is.

- A **fileset commit** locks, for every destination directory, the deepest
  ancestor of it that exists at the moment the run starts (`claim_anchor`), as
  well as the directory itself. For a destination that already exists those are
  the same path and the run locks exactly what it locked before. For one it has
  to create — which is the only case an inode-keyed lock cannot reach, because a
  directory that does not exist has no marker to lock — it is the level that
  holds the destination, which is the level the `--out-dir` publish is holding.
- A **directory publish** locks, besides the level holding its target, the
  directories inside the target (`published_dirs_inside`): the target and its
  `html/` subdirectory, which is the whole shape a published out-dir has and
  therefore the whole set a files-mode peer can be holding.

**Why not an inode-keyed lock, and why not a name-keyed lease.** The ticket
ruled out the first: after A's swap a waiter holds a lock on a directory no
longer at that path. The 2026-08-10 note above removes that objection —
a waiter revalidates the marker it is granted and re-locks the one that replaced
it — which is what makes locking a target that is about to be renamed away
sound. A name-keyed lease in the parent (`<parent>/.transync-publish.<name>.lock`)
was the other candidate and was not taken: it would have every files-mode
publication write a lock file into the parent of its destination, so
`transync translate --output out.md` in `~/project` creates a marker in `~`,
and it fails where a parent is not writable but the destination is. Claiming the
deepest *existing* level costs neither — it never writes outside a directory the
run is already creating into, and it covers the `html/` nesting a
parent-and-name key does not.

**What is still open.** A run publishing deeper inside an `--out-dir` target
than the `html/` level transync itself writes is not serialized against the
replace. Nothing transync produces has that shape, and `--force` waives the
guard for trees that do; `contracts.md` §6 states it under "Scope".

## Appended note — 2026-08-10: the inner lock set is re-read under the lock

The review of the note above (fix round of ticket `40e2a5`) found that
`published_dirs_inside` was read **before** `PublishLock::acquire` and never
looked at again, so the set the replace locked described a moment that had
passed. The traced sequence, all three runs legal on their own:

1. A starts `--out-dir X` while `X` does not exist. The inner set is empty, so A
   locks only the level holding `X`.
2. B publishes `--output X/out.md` — it creates `X` before taking any lock
   (a destination that does not exist has no marker to lock), claims the level
   holding `X` and the new `X`, publishes and releases.
3. A is granted the level, finds an `X` its guard accepts, and swaps it away —
   while C, *started after `X` appeared*, claims `X` itself (`claim_anchor`
   answers with `X` now) and publishes into the tree A is renaming.

**What changed.** `publish_out_dir` goes through `lock_publication_tree`: it
takes the whole set, re-reads what is inside the target with the locks held, and
— when the answer changed — drops the set and takes it again, bounded at four
passes and an error if that is exhausted. The set is dropped and retaken whole
rather than extended, because `PublishLock` acquires in a global order and that
order is what keeps two runs with overlapping sets from deadlocking. The re-read
runs before the replaceability guard, so the guard, the lock set and the swap all
act on one tree.

**What is still open, more narrowly than before.** A directory that appears
inside the target *after* the last re-read, put there by something this run is
not serialized with — a bare `mkdir`, or a peer that creates its destination
levels before parking on this run's lock — is not held, so a run that starts
inside that window and claims it can publish into a tree the replace takes away.
Closing it means either creating (and locking) the target under the parent lock,
which costs the fresh target's single-rename atomicity, or making a fileset
commit create its destination levels under the anchor lock it already takes.
Both are recorded in ticket `cbbc4e` beside the deeper-nesting residual they
share a shape with; `contracts.md` §6 states the residual under "Scope".

## Appended note — 2026-08-10: depth is bounded by ordering, not by a third claim

Ticket `cbbc4e`, carrying both residuals the two notes above left open: a run
publishing *deeper* inside an `--out-dir` target than `html/`, and a directory
that appears inside the target after the last re-read. They were filed together
because they share a shape, and the answer is the same for both.

**The claim scheme cannot be extended, and is not.** A replace claims a fixed,
shallow set — the target and its `html/` — while a publication claims a *point*
that can be arbitrarily deep. No finite pair of claims meets at every depth, so
each of the three ways to add one was weighed and rejected on its own terms:

- *Claim upward from the destination.* A fileset commit would also lock every
  existing ancestor. There is no principled root: stopping at the filesystem
  root serializes runs that merely share a home directory and writes lock
  markers into `~` and `/` (often unwritable, which would make the exclusion
  best-effort and so no exclusion at all), and every shallower root is a
  guess. Recognizing an ancestor as a transync out-dir would bound it, but
  that turns a lock into a predicate — and "carries no ownership marker" is not
  "is not a replace target".
- *Claim downward from the replace.* `publish_out_dir` would walk the target and
  lock every subdirectory. The walk itself is affordable — it is strictly
  cheaper than the `remove_dir_all` the replace is already committed to — but it
  writes markers throughout a tree the guard may then *refuse* to touch, it is
  unbounded under `--force`, and it is a TOCTOU: a directory created after the
  walk is unclaimed, which is precisely the residual it was meant to close.
- *Containment check instead of a claim.* The replace, holding its own locks,
  verifies that no live publish lock exists beneath the target. "No lock file"
  and "no publisher" are not the same answer — ticket `8792b7` is the record of
  how subtle that gap is — and a peer *queued* in a blocking lock call is
  invisible to any probe, because POSIX advisory locks report no waiters.

**What bounds the gap is two orderings, both already in the design.** A
publication reads its anchor *before* it creates anything (`claim_anchor`, ti
`40e2a5`), so the level it locks is one that existed when it started; for any
destination under an `--out-dir` target that is the target or a level above it,
and the replace holds both. The peer therefore waits — it cannot create a level
inside the target and then publish into it, because it parks before it writes.
The remaining way a deep, unheld level exists at all is that something created
it while the replace was staging, and the second ordering answers that.

**What changed.** `ensure_out_dir_replaceable` now runs **twice**: once before
staging, as before, and once more with the staged tree in hand, immediately
before the swap. The first verdict was separated from the rename acting on it by
the whole staging phase — the entire bundle written and fsynced — and the
publish lock does not hold it still, because the two writers that can put a
directory inside the target during that window take no lock (a bare `mkdir`) or
take one only *after* creating their levels (a files-mode peer). A target that
grew an entry meanwhile is now refused with `… (it changed while this run was
staging its output)`, its previous contents left in place and the staged tree
removed. This is the same guard, asked at the moment its answer is used.

**And the guard's shape recognition was tightened to make that true.** The
bundle allow-list and both staging-temp recognizers matched on *name* alone, so
a **directory** named `index.html`, or one named `out.md.tmp.<pid>`, was read as
a bundle file or as transync's own residue and walked past the check — carrying
an arbitrary subtree into the backup `remove_dir_all` of a replace that needed
no `--force`. That is the hazard the top level has refused for a directory named
`out.md` since ti `66339b` and for the two marker names since its review round;
it now holds everywhere. Without it, "a target the guard accepts contains no
directory but `html/`" — the claim the whole argument above rests on — was false.

**What is accepted, not open.** With `--force` the operator has waived the guard
by request, and with it this bound: two runs aimed at one tree through different
modes, at a depth transync does not itself write, can still have the replace take
the deeper output with it. That output is *lost*, not *corrupted*: every write
and rename is by path, so a peer whose tree has been renamed away fails on the
next one rather than landing half a publication somewhere invisible, and a peer
that had already finished simply lost to a later replace, which is ordinary.
Beyond that: the few syscalls between the second guard pass
and the rename, which nothing closes, because a guard is a check on a snapshot
and a writer that takes no lock is not excludable by one. `contracts.md` §6
states both under "Scope", as settled rather than residual.
