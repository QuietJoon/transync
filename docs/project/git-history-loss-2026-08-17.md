---
type: Reference
title: Git history loss, 2026-08-17 — the cause named with physical evidence, the recovery runbook, and where the past now lives
description: A cloud file-sync client writing conflict copies inside `.git` cost the object store 33 objects; 17 were recovered by hash from sibling stores and 16 were not, so the repository restarted from the verified working tree. This record holds the runbook that recovered the 17, and is where the pre-2026-08-17 history now lives.
tags: [project-control, incident]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-17T00:00:00Z
---

# Git history loss, 2026-08-17

**This repository's git history begins on 2026-08-17.** Everything before that
date happened, was tested, and is present in the code — but it is no longer in
this object store. This document is the record of why, and of what survives
where.

It is the **third** loss event and the **second** restart, all within ten days:

| Date | What was lost | Outcome |
|---|---|---|
| 2026-08-07 | one tree of commit `494bc9c` | repaired by hash-verified `git mktree` (ticket `7a7feb`; `docs/Troubleshooting.md`) |
| 2026-08-10 | 30 objects, four commits among them | first restart — `docs/project/git-history-loss-2026-08-10.md` |
| 2026-08-17 | 33 objects, 17 recovered | second restart — this record |

The 2026-08-10 record is **not** superseded. It is still the only record of
what went with *that* restart, and its reasoning about detection, redundancy
and `git gc` still stands. This record adds the two things it could not: the
cause, with physical evidence, and a recovery procedure that has now been run.

## What happened

`git fsck --no-progress --connectivity-only` reported **33 missing objects**
and **33 broken links** — 21 blobs, 11 trees and one commit — and above them,
six ref and reflog error lines that named the cause out loud (the two
reflog complaints are each emitted twice, once per ref being walked):

```
error: refs/heads/master (2): badRefName: invalid refname format
error: refs/heads/master (2): invalid sha1 pointer 0000000000000000000000000000000000000000
error: HEAD: invalid reflog entry 36d95930d6b3e61205297418a6a0f232aca6141f
error: HEAD: invalid reflog entry 36d95930d6b3e61205297418a6a0f232aca6141f
error: refs/heads/master: invalid reflog entry 36d95930d6b3e61205297418a6a0f232aca6141f
error: refs/heads/master: invalid reflog entry 36d95930d6b3e61205297418a6a0f232aca6141f
```

A ref file called `master (2)` holding a null sha1 is not something git writes.

Two of the missing objects decided the outcome:

- commit **`36d95930d6b3e61205297418a6a0f232aca6141f`** was the parent of
  `02c12f51`, so the chain was severed there — `git log` stopped with
  `fatal: Failed to traverse parents of commit 02c12f51…`;
- tree **`b18740a642b39da0135b4c91a6fb454072ebc308`** was `02c12f51`'s own root
  tree, so even that commit could not be checked out. Its *commit object*
  survived and still reads in the archived store, which is worth knowing: a
  readable commit object is not the same thing as a readable commit.

The full output is `/Volumes/Temp/claude/provenance-20260817/fsck-before.txt`.

## The cause, and its physical evidence

The 2026-08-10 record could only say the cause was "identified … and parked".
**It is no longer parked**, and this time the evidence was physical rather than
inferential: the sync client's conflict-copy artifacts were sitting inside
`.git`. A census of the store at one moment:

| Collision artifact inside `.git` | Count |
|---|---|
| Duplicated object **fanout directories** — `0a (2)`, `10 (2)`, `6d (2)`, … | 23 |
| Object **files** whose own name carried the ` (2)` suffix | 3 |
| Duplicated **ref file** — `refs/heads/master (2)` | 1 |
| **Collision paths, total** | **27** |
| The same census over the working tree | **0** |

Twenty-seven inside `.git` and zero in the working tree is the shape of the
finding. Whatever the client is doing, it is doing it to the one directory
nobody opens.

**The duplicated *directory* is the dangerous form**, and it is why the earlier
events looked causeless:

- The object files inside it carry ordinary 38-hex names. The collision marker
  is on the **parent**, so the obvious search — `find .git -name '* (*'` —
  walks straight past them. Of the 35 duplicated object copies in this store,
  only 3 had a name that search would match; the other 32 were inside a
  `XX (2)` directory with perfectly innocent names.
- **git never looks there either.** A loose object lives at
  `objects/<2 hex>/<38 hex>`, and `0a (2)` is not two hex characters, so the
  directory is not part of the layout git reads. It is invisible to the
  operator's grep and to git at the same time.

The client's ignore list — `/Volumes/Common/GDrive/InsyncIgnore.txt` — excluded
the rebuildable directories (`target`, `node_modules`, `build`, `dist`,
`venv` and similar) and did **not** exclude `.git`. **It does now**: `.git` was
appended on 2026-08-17. The file is UTF-16 LE with a BOM and CRLF line
endings; the edit preserved both, and the pre-edit copy is at
`/Volumes/Temp/claude/InsyncIgnore.txt.backup-20260817`.

That reverses the posture `docs/Troubleshooting.md` recorded under ticket
`7a7feb` — that excluding the repository from the sync client had been weighed
and declined, leaving the release-checklist preflight as the standing answer.
The preflight stays; it is now detection **behind** a removed cause rather than
instead of one.

## What was NOT lost

- **Every tracked file.** All **342** files in the index existed on disk and
  were byte-identical to what the working tree had been running — verified file
  by file against `/Volumes/Temp/claude/provenance-20260817/tracked.txt`, and
  again after the restart against the archived copy: 342 identical, 0 differing.
- **The test suite, green.** `cargo test --workspace -- --test-threads=4`
  reported **1066 passed / 0 failed / 5 ignored** with `CARGO_EXIT=0`, and the
  CLI stub suite **208 passed / 0 failed** — measured immediately before and
  immediately after the restart with identical results. The pre-restart run is
  `/Volumes/Temp/claude/provenance-20260817/tests-before.txt`.
- **The TicGit board.** Tickets live in `.git/git-meta.sqlite`, which was
  copied out before the old `.git` was moved aside and restored into the new
  one — **182 tickets, 10 of them open**.
- **The commit log as text.** The 23 commits still reachable from the old HEAD
  are listed with dates and subjects in
  `/Volumes/Temp/claude/provenance-20260817/log-reachable-23.txt`, and the full
  reflog in `reflog.txt` beside it.

## What recovery yielded

Recovery was attempted before restarting, and it worked in part: **17 of the
33** objects came back, hash-verified. `git fsck` went **33 → 16 missing**.

Three tracked files whose history had been unreadable became readable again in
the archived store at the commit below the old HEAD:

- `crates/transync-core/src/pipeline.rs`
- `crates/transync-core/profiles/default.toml`
- `docs/project/design-change-records/DCR-0025-review-0003-content-loss-and-refusal-precedence.md`

Where the 17 came from — four sibling object stores left over from earlier
work, each searched with `git cat-file -e` so that **packfiles counted**, not
only loose objects:

| Store | Hits on the 33 |
|---|---|
| `/Volumes/Temp/claude/transync-rescue-20260810/.git` | 17 |
| `/Volumes/Temp/claude/transync-broken-git-20260810` | 17 |
| `/Volumes/Temp/claude/transync-r1/verify/clone/.git` | 13 |
| `/Volumes/Temp/claude/r0002-b4-review/transync/.git` | 1 |

The columns overlap heavily; **17 distinct** objects is the union, and it is
also what the best single store held. Every one of them came from a snapshot
taken on **2026-08-10** — which is exactly why nothing older helped with the
remaining 16.

### The 16 that were not recoverable

**Sixteen objects were not found anywhere on this machine** — seven blobs,
eight trees and one commit, by type as `fsck` names them in
`/Volumes/Temp/claude/provenance-20260817/fsck-after-recovery.txt`. Their shas
are listed in
`/Volumes/Temp/claude/provenance-20260817/permanently-missing-16.txt`.

> The restart commit's own message says "six blobs, nine trees, and one
> commit". That is a miscount of the same sixteen objects, and a commit message
> cannot be amended after the fact; the `fsck` output named above is the
> authority for the breakdown.

Nothing else was available to try. There are **no APFS local snapshots** on
that volume, and **no clone of this repository newer than 2026-08-11** exists
on the machine. Blobs cannot be reconstructed from anything. The trees might in
principle have been rebuilt with `git mktree` — the repair that worked on
2026-08-07, and the reason that incident cost one afternoon instead of a
history — but the blobs those trees reference are themselves gone, so there was
nothing to feed it.

## The runbook

This is the procedure that produced the 17, in the order to run it. It is
written to be followed by someone who has just watched `fsck` name a missing
object and has not yet touched anything.

**Before anything else: recovery is purely additive.** Never `reset`, `gc`,
`prune`, or rewrite history to make the error go away — that converts a
recoverable hole into a lost commit, and `git prune` in particular deletes the
unreachable pool that `git fsck --lost-found` recovers from.

1. **Establish what is missing, and keep the output.**

   ```bash
   git fsck --no-progress --connectivity-only > fsck-before.txt 2>&1
   echo "FSCK_EXIT=$?" >> fsck-before.txt
   grep '^missing' fsck-before.txt | awk '{print $3}' | sort -u > missing.txt
   ```

   `dangling` lines are noise. `missing <type> <sha>` and `broken link from
   <sha> to <sha>` are the fault lines; the second names the parent still
   pointing at the hole, which is how you learn *what* was lost.

2. **Sweep the collision copies inside `.git` — directories as well as files.**
   The directory form is the one that hides objects (above), so a name search
   alone is not enough:

   ```bash
   find .git -name '* (*'                       # files AND directories
   find .git/objects -type d -name '* (*' -exec find {} -type f \;
   ```

   **Verify every copy by hash before trusting it.** A loose object is a
   zlib stream whose sha1 is taken over the decompressed bytes including the
   `<type> <len>\0` header; if the hash does not come back equal to the name
   you expected, the file is not that object and must not be imported. Any copy
   that decompresses to a valid object and hashes to a sha in `missing.txt` is
   a recovery.

3. **Sweep the working tree with the same search.** A conflict copy of a
   tracked file is not a recovery of a git object, but it tells you what the
   client is touching. Here it found **nothing**: 27 collision paths inside
   `.git`, 0 outside it.

4. **Search sibling stores with `git cat-file -e`, never by walking
   `objects/`.** This is the step that decides how much you get back. Walking
   the `objects/` directory only sees loose objects, and any store that has
   been packed — which is most of them — will look empty:

   ```bash
   for store in <paths to sibling .git dirs>; do
     n=0
     while read sha; do
       git --git-dir="$store" cat-file -e "$sha" 2>/dev/null && n=$((n+1))
     done < missing.txt
     echo "$store -> $n"
   done
   ```

   Look for rescue copies, old broken stores kept from previous incidents,
   verification clones, and review worktrees. All four of this repository's
   hits were of those kinds; none was a backup anyone had made on purpose.

5. **Re-import, asserting the sha comes back unchanged.** Pipe the object out
   of the donor and back in, and compare — the hash is the whole proof that you
   restored the original bytes rather than something that merely resembles them:

   ```bash
   got=$(git --git-dir="$store" cat-file -p "$sha" | git hash-object -w -t "$(git --git-dir="$store" cat-file -t "$sha")" --stdin)
   [ "$got" = "$sha" ] || echo "REFUSE: $sha came back as $got"
   ```

   Nothing else can produce that hash, which is the same argument the
   `git mktree` repair rests on.

6. **Re-measure.** Run step 1 again into a second file and diff the two
   `missing` sets. That difference, not the count of files you copied, is what
   you recovered.

**Honest note on step 2.** The inside-`.git` sweep recovered **nothing** here.
All 35 duplicated object copies decompressed cleanly and were valid git
objects; between them they held 33 distinct shas, and **all 33 were objects git
already had**. (That the two counts are both 33 is a coincidence — the overlap
with the 33 missing objects was **zero**.) The step is in the runbook on
**cost**, not on demonstrated yield: it takes a minute, it is the only step
that can tell you the loss is still in progress, and the copies are the
physical evidence of the cause. Do not budget hope for it.

## Why history was not preserved

This was measured, not preferred. Of the **23 commits still reachable** from
the old HEAD, exactly **one** had a tree that `git ls-tree -r` could walk to
completion — and that one was the HEAD commit itself, `6db2c5a`, which had been
created earlier the same day by restoring every missing index blob and
rebuilding the index without its cache-tree. The other **22 could not be
checked out**.

A history whose commits cannot be checked out is not history: preserving those
22 would have left `git clone` broken, which is precisely the property the
restart was for.

## The new history

Root commit **`59ce8df`**, message beginning "chore: the repository restarts
from a verified working tree". After it:

- `git fsck --no-progress --connectivity-only` — **0 missing, 0 broken links,
  0 dangling, 0 errors**, exit 0;
- `git ls-tree -r HEAD` walks **342/342**;
- `git status` is clean;
- **`git clone` of this repository succeeds**, producing 342 files whose own
  `fsck` is clean.

Cloneability is the property the previous state had lost, and it is the one
worth re-checking after any future event.

**No tags were recreated, because there were none to recreate.** `git tag -l`
has been empty since the 2026-08-10 restart, and v0.4.0 ships untagged by the
owner decision recorded in `docs/project/release-checklist.md`, under *When it
applies*.

## Where the past now lives

Nothing was deleted. The damaged object store was **moved**, so if a recovery
technique appears later the objects that do survive — including the 17
re-imported ones and all 35 collision copies — are still there.

**These paths moved on 2026-08-19, and most of the originals are gone.** The
archives listed here were written to `/Volumes/Temp`, which is a RAM disk; it
was cleared on 2026-08-19. What survived did so because it had been copied to
`/Volumes/Common/git-backup/` first. Read that directory's `README.md` before
using either store.

| What | Where | Survived the 2026-08-19 clear? |
|---|---|---|
| The damaged object store, untouched (23 commits, HEAD `6db2c5a`) | `/Volumes/Common/git-backup/transync-broken-git-20260817` | **yes** |
| The 2026-08-10 store (39 commits, HEAD `fecbfed`, and the only surviving copies of tags `v0.1.0`, `v0.2.0`, `v0.3.0`) | `/Volumes/Common/git-backup/transync-broken-git-20260810` | **yes** |
| A full working-tree copy, 342/342 verified against the index | `/Volumes/Temp/claude/transync-rescue-20260817-prerestart` | no — gone |
| Reflog, logs, `fsck` before and after recovery, the 16 lost shas, the TicGit db | `/Volumes/Temp/claude/provenance-20260817/` | partially |

The lesson that cost the least to learn and matters the most: an archive on a
RAM disk is not an archive. Both surviving stores are the *only* record of this
repository's pre-restart history, and they earned their keep two days later —
see the recurrence below.

## The cause was not removed: it recurred on 2026-08-19

The ignore-list edit did **not** take effect. Two days after the restart, on a
repository whose `fsck` had been silent, a routine re-check found **13 objects
missing** — 12 blobs and one tree — and `git ls-tree -r HEAD` aborting after 151
of 342 paths. The sync client was running at the time (`Insync`, plus its
daemon and plugin processes).

Two things about this recurrence are worth more than the loss itself:

1. **It left no collision copies.** The census that this record calls the check
   read **0** inside `.git` and 0 in the working tree, while objects were
   vanishing. A clean census is therefore *not* evidence that the cause is
   contained — this record's earlier framing of it as "the check" was too
   strong. `git fsck` is the check; the census only ever explained a mechanism.
2. **Recovery was complete and instant**, because a redundant copy existed:
   all 13 objects were found in `/Volumes/Common/git-backup/transync-broken-git-20260817`
   and re-imported with `git cat-file | git hash-object -w`, every one verified
   to hash to exactly the sha requested. `fsck --full` returned to silence and
   HEAD walked 342/342 again. The 2026-08-17 restart had produced a repository
   with **no remote and no redundancy**; that gap is what made the 2026-08-17
   loss unrecoverable and what the backup closed here.

The conclusion this record should have drawn on 2026-08-17, and draws now:
excluding a directory from the sync client is a mitigation whose effect nobody
verified, and it failed. Redundancy is the measure that worked.

## When you hit a wall walking history

The documents in this repository quote commit hashes freely — resolution
comments on tickets, ADR and DCR amendments, `CHANGELOG` entries, the wave
summaries in `docs/project/status.md`. **Almost none of them resolve in this
object store**, and `6db2c5a` and `aee79df` — the two most recently quoted, from
2026-08-16 and 2026-08-13 — are among the ones that do not.

That is expected, not corruption. What to do:

1. **Do not run `fsck` looking for a fault.** A hash that this store never had
   is not a missing object; `git cat-file -e <sha>` simply returns non-zero and
   `fsck` stays clean.
2. **Read the surrounding prose instead.** Every hash in this repository's
   documents is quoted *beside* a description of what the commit did. The
   description is the durable half; the hash was only ever the receipt.
3. **If you genuinely need the diff**, the archived stores hold it — but pick
   the right one, because each restart's archive begins where the previous
   restart put it:

   ```bash
   # commits from 2026-08-10 to 2026-08-17
   git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260817 show <sha>
   # anything older — that store begins at the 2026-08-10 root commit and
   # never held an earlier object
   git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260810 show <sha>
   ```

   *Paths corrected 2026-08-29 under this section's own rule 4.* Both stores
   were originally written under `/Volumes/Temp/claude/`, which is scratch and
   has since been wiped; they were copied to `/Volumes/Common/git-backup/`,
   where the table in **Where the evidence lives** and `status.md` already name
   them. The two invocations above were re-run at the new paths on 2026-08-29
   and both resolve. The dated evidence table earlier in this record keeps its
   `/Volumes/Temp/…` paths: it records where a search looked on the day it ran,
   which is still true and is not an instruction to follow.

   Both are damaged stores, so a hash in the right window may still fail to
   read; that is the loss, not a mistake in the command. A hash that fails in
   the newer archive is far more often in the *older* one — `bb93b68`, the
   commit `reviews/README.md` needs to recover the retired review rounds, is
   the worked example, and that file carries the exact invocation.
4. **Do not repair a document by inventing a new hash for it.** A living
   instruction that cannot be followed gets rewritten to name something that
   exists today (a path, a range, a tag, `HEAD`); a dated record that quotes a
   dead hash keeps it and gains a note.

## What the code history says instead

The repository's own documents carry the design history that the commit graph
no longer does, and they are the right place to look:

- `CHANGELOG.md` — per-release and `[Unreleased]` entries, written per ticket.
- `docs/decisions/` — 24 ADRs (23 live, one archived).
- `docs/project/design-change-records/` — 31 DCRs, each naming the ADRs it
  changed.
- `docs/project/open-issues.md` and `docs/project/open-issues-archive.md` —
  OI-0001 through OI-0038.
- `docs/backlog.md` — the cross-source index.
- The TicGit board — `ti list --all` — 182 tickets with resolution comments
  naming the commits that resolved them. Those hashes will not resolve in this
  object store; they resolve, where they resolve at all, in the archived ones.

## What changes going forward

The 2026-08-10 record's analysis of **write volume** still holds, and so does
its verdict on `git gc` / `git gcx` — packing reduces loose objects, but
`prune` deletes the unreachable pool that `fsck --lost-found` recovers from, so
mirror first and pack on a schedule rather than per commit. What this event
changes is the balance between its two named practices:

1. **Detection** was already in place and it worked: `git fsck
   --no-progress --connectivity-only` is release-checklist step 0, it is
   read-only, it takes seconds, and it named this loss on the day it happened.
   Detection is not what failed.
2. **Cause removal** is new, and it is the thing this event actually bought:
   `.git` is now in the sync client's ignore list. That is the first mitigation
   in three events that aims to stop the loss rather than report it.

   **It was written, and it did not work.** What was verified on 2026-08-17 was
   that the line is in the file with the encoding intact — nobody confirmed the
   client had re-read its ignore list, and this record flagged that gap as
   "written, not yet proven". On 2026-08-19 the gap closed the wrong way: the
   loss recurred with the line still in place and the client still running (see
   *The cause was not removed* above). Treat editing a sync client's ignore
   list as **unverified until a period of normal work passes an `fsck`**, and
   never as the measure a repository's survival rests on.

   The census this record proposed as the re-check is also weaker than it
   claimed: during the 2026-08-19 loss it read **0**, because that recurrence
   left no collision copies at all. `git fsck` is the check. The census
   explains a mechanism; it does not detect the failure.
3. **Redundancy is the measure that actually worked.** On 2026-08-17 it was
   missing, and that is why 16 objects were lost: every one of the 17 recovered
   objects came from a snapshot someone took for an unrelated reason, and every
   one of the 16 lost objects was younger than the newest such snapshot. On
   2026-08-19, with the salvaged stores deliberately kept under
   `/Volumes/Common/git-backup/`, the same class of event was a **complete
   repair in seconds** — 13 of 13 objects restored and hash-verified. The
   difference between a restart and a repair was one redundant copy.

   **Acted on, the same day.** The repository now has a mirror: a bare clone at
   `/Volumes/Common/git-backup/transync.git`, registered as the remote
   **`backup`** — named that rather than `origin` because it is a mirror of
   record, not an upstream anyone develops against — with `master` tracking
   `backup/master`. Push after every commit. Verified end to end on creation: a
   fresh `git clone` of it produced 343 files, two commits, and a silent
   `fsck --full`.

   Its limit, stated so nobody mistakes it for a solution: it sits on the same
   synced volume as the repository, so the same cause can reach it. It is
   redundancy, not immunity. A mirror on a volume the sync client does not
   touch — or a real off-machine remote — is the version of this that would
   also survive the volume.

   **Answered 2026-09-01 by the owner (ti `6b4300`): the authoritative `.git`
   is GitHub.** `origin` — `github:QuietJoon/transync.git` — is the store of
   record. That settles the question this record left open and it re-ranks the
   two local stores:

   | store | role after this decision |
   |---|---|
   | `origin` (GitHub) | **Authoritative.** Off-machine, off-volume, and outside the sync client's reach — the only copy the 2026-08-17 and 2026-08-19 causes cannot touch. |
   | `/Volumes/Common/QJoon/transync/.git` | A **working copy**. It has destroyed history twice. Nothing may exist only here. |
   | `backup` (`/Volumes/Common/git-backup/transync.git`) | Still a mirror of record for the salvaged stores, and still on the same synced volume, so still redundancy rather than immunity. It is no longer the last line — it is the fast local one. |

   The operational consequence is the part that bites, and it is a *standing*
   obligation rather than a one-off: **the authoritative store is only
   authoritative to the commit it actually holds.** Local `master` was found 24
   commits ahead of `origin/master` on the day the decision was taken, so
   twenty-four commits of work — every commit of 2026-09-01 among them —
   existed solely on the volume with the incident record. Push after every
   commit, to `origin` first; a local commit that has not reached GitHub is
   exactly the exposure this whole document is about.
