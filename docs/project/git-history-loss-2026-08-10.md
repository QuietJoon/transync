---
type: Reference
title: Git history loss, 2026-08-10 — what was lost, what was kept, and how to read this repository's past
description: The object store lost 30 objects including four commits and part of HEAD's own tree, so the repository was re-initialized from the verified working tree; this record is where the pre-2026-08-10 history now lives.
tags: [project-control, incident]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-10T00:00:00Z
---

# Git history loss, 2026-08-10

**This repository's git history begins on 2026-08-10.** Everything before that
date happened, was tested, and is present in the code — but it is no longer in
this object store. This document is the record of why, and of what survives
where.

## What happened

`git fsck --connectivity-only` reported **30 missing objects**: four commits
(`ef80cfe`, `e55f4909`, `d0e10870`, `e5aa8967`), and a mix of trees and blobs
they and others referenced. The decisive symptom was not the history gap but
this:

```
$ git ls-tree -r HEAD --name-only
error: Could not read d0b4efca5d8412a01c28e70796fee9167e0cbb09
exit 1
```

**HEAD could not describe its own tree.** A repository that cannot reconstruct
its checked-out state has no useful history to preserve, whatever else is
intact.

Recovery was attempted before re-initializing. A sibling clone left over from a
review wave (`/Volumes/Temp/claude/r0002-b4-review/transync`, at `6655e59`)
still held **3 of the 30** objects and one of the four lost commits; it predated
most of the loss. No other clone, worktree, or backup on this machine held the
rest. 27 objects were unrecoverable.

This was the **second** such event. The first, on 2026-08-07, lost a single tree
of commit `494bc9c` and was repaired by hash-verified `git mktree`
substitution — that incident is ticket `7a7feb`. The owner identified Insync
(Google Drive) syncing `/Volumes/Common` as the likely cause, could not stop the
sync, and parked the issue. Roughly 150 commits landed in the three days
between the two events.

## What was NOT lost

- **Every tracked file.** All 313 files in the index existed on disk and were
  byte-identical to what the working tree had been running.
- **The test suite, green.** `cargo test --workspace -- --test-threads=4`
  reported **943 passed / 0 failed** with `CARGO_EXIT=0`, measured immediately
  before and immediately after the re-initialization.
- **The TicGit board.** Tickets live in `.git/git-meta.sqlite`, which was copied
  out before the old `.git` was moved aside and restored into the new one —
  110 tickets, 10 of them open at the time.
- **The commit log as text.** 371 reflog entries covering 368 distinct commits,
  from `b67aa15` (2026-07-13, "Rename crates/transync to crates/transync-core")
  to `fecbfed` (2026-08-10).

## Where the past now lives

| What | Where |
|---|---|
| The broken object store, untouched | `/Volumes/Temp/claude/transync-broken-git-20260810` (22 MB) |
| A full working-tree copy, verified 943/0 | `/Volumes/Temp/claude/transync-rescue-20260810` |
| Reflog, logs, tags, fsck output, tracked-file list | `/Volumes/Temp/claude/provenance/` |

Nothing was deleted. The old `.git` was **moved**, not removed, so if a
recovery technique appears later the objects that do survive are still there.

The three release tags (`v0.1.0`, `v0.2.0`, `v0.3.0`) were **not** recreated:
they would have had to point at the initial commit, which is not where those
releases were cut. Their commit hashes are recorded in
`/Volumes/Temp/claude/provenance/tags.txt`. The CHANGELOG remains the readable
record of what each release contained.

## What the code history says instead

The repository's own documents carry the design history that the commit graph
no longer does, and they are the right place to look:

- `CHANGELOG.md` — per-release and `[Unreleased]` entries, written per ticket.
- `docs/decisions/` — 21 ADRs.
- `docs/project/design-change-records/` — 29 DCRs, each naming the ADRs it
  changed.
- `docs/project/open-issues.md` — OI-0001 through OI-0037.
- `docs/backlog.md` — the cross-source index.
- The TicGit board — `ti list --all` — 110 tickets with resolution comments
  naming the commits that resolved them. Those hashes will not resolve in this
  object store; they resolve in the archived one.

## What changes going forward

The exposure this repository carries and others on the same volume do not is
**write volume**: ~150 commits in three days from concurrent agent sessions,
leaving 251 loose-object directories. Loose objects are thousands of small
files; a packed store is three large ones. Whatever removes files here has
thousands of chances per day in this repository and a handful in a quiet one.

Two practices follow, and they are different in kind:

1. **Detection.** `git fsck --no-progress --connectivity-only` takes seconds
   and is read-only. Run it after a batch of commits; it names the loss on the
   day it happens rather than at the next clone. This is already release
   checklist step 0.
2. **Redundancy.** A mirror on a different volume is the only thing that makes
   an object recoverable. Detection without redundancy just tells you sooner
   that something is gone.

A note on `git gc` / `git gcx`: packing does reduce the loose-object count, so
the instinct to run it often is sound. But `git gcx` includes `git prune`,
which **deletes unreachable objects** — precisely the pool that
`git fsck --lost-found` recovers from. Running it after every commit would
remove the safety net at the moment it is most needed, and `gc --aggressive`
rewrites the entire object store each time, which maximizes writing in an
environment where writing is the suspected risk. Mirror first; pack on a
schedule, not per commit.

## Consequences

**2026-08-13 — the first release after the restart ships untagged.** The three
release tags did not survive and were not recreated (above); `git tag -l`
returns empty. The owner decided that v0.4.0 is recorded in `CHANGELOG.md`
only and carries no git tag, because a tag cut here would point at a commit
that does not carry the release's history. Tagging resumes at the next
release. The decision and the release steps it changes are in
`docs/project/release-checklist.md`, under *When it applies*.
