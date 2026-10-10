---
type: archive
title: Troubleshooting archive — text moved out of docs/Troubleshooting.md
description: Verbatim passages that left the living troubleshooting guide, each under a heading naming its source section and the date it left. It is an archive — verbatim, append-only, never read for current truth. The live guide is docs/Troubleshooting.md; the object-loss events themselves are recorded in docs/project/git-history-loss-2026-08-10.md and docs/project/git-history-loss-2026-08-17.md.
tags: [archive, troubleshooting]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-06T00:00:00Z
status: deprecated
---

# Troubleshooting archive

This is an **archive** (DCR-0054). `docs/Troubleshooting.md` is a living document:
it says what to do now. Everything below was in it once and is kept verbatim.

Read this to answer *what did the troubleshooting guide say about this, before it
was compacted*. For current diagnosis and repair, read `docs/Troubleshooting.md`;
for the events themselves, the incident records named in each entry are the
authority.

## `fatal: unable to read tree` / `git fsck` reports a missing object — the object-loss narrative (moved 2026-10-06)

- **Source:** `docs/Troubleshooting.md`, section "`fatal: unable to read tree` /
  `git fsck` reports a missing object", as it stood at `origin/master` `755d7ba`.
- **Why it moved:** documentation compaction wave 3 (ti `d4b142`;
  `docs/superpowers/specs/2026-09-13-docs-compaction-design.md` §5). The section
  keeps its diagnosis, its tree-rebuild repair, its mitigations and its cautions;
  the incident history it carried is held by
  `docs/project/git-history-loss-2026-08-10.md` and
  `docs/project/git-history-loss-2026-08-17.md`.
- **Why it is kept here as well:** `docs/project/git-history-loss-2026-08-17.md`
  cites `docs/Troubleshooting.md` for the 2026-08-07 repair of commit `494bc9c`'s
  tree and for the posture recorded under ticket `7a7feb`; this excerpt is the
  text that citation meant. The commit hashes in it do not resolve in this object
  store.

The passages below are verbatim and in source order. Passages that sat between
them and are still in the living section are not repeated.

*The section's opening paragraph and its example error:*

An object vanished from `.git/objects`. It has happened three times here —
2026-08-07 (one tree, repaired), 2026-08-10 (30 objects, history restarted:
`docs/project/git-history-loss-2026-08-10.md`) and 2026-08-17 (33 objects,
history restarted again: `docs/project/git-history-loss-2026-08-17.md`, which
carries the full recovery runbook). The worked example below is the first one,
because it is the only one that ended in a repair rather than a restart: the
`docs/` subtree of commit `494bc9c` was gone minutes after the commit was
written, so every `git show`, `git diff` and `git log -p` crossing that commit
died with

```
fatal: unable to read tree (c8daa7a8aa53cbaed7e805c6db86023a8e052e70)
```

*The cause paragraph:*

**Cause — confirmed 2026-08-17.** A file-sync client (Insync/Google Drive,
here) re-materializing files under `.git/` while git is writing them — this
worktree lives on a synced external volume. For the first two events this was
an inference; on 2026-08-17 the client's conflict copies were found *inside*
`.git`: 23 duplicated object **fanout directories** named `0a (2)`, `6d (2)`
and so on, three duplicated object files, and a duplicated ref file
`refs/heads/master (2)` holding a null sha1 — 27 collision paths inside `.git`
against **0** in the working tree.

*The two further tells:*

Two older tells still apply: a pack whose `.pack` mtime is newer than its own
`.idx` (git never rewrites a finished pack), and the fact that nothing in this
repository prunes objects and no `gc` had run.

*The paragraph after the `git mktree` example (its first two sentences are still
in the living section; the third is the one that moved):*

Rebuild the deepest missing tree first, then each parent tree with the
child's new hash substituted in. If the hash `git mktree` prints equals
the hash git said was missing, the object is the original byte for byte —
nothing else can produce that hash. The 2026-08-07 repair rebuilt three
nested trees this way, after which `git fsck` was clean and `git show
--stat 494bc9c` worked again.

*The third item of "Mitigations in place", up to the paragraph that opened with
"There is now a mirror to push to."; the living section replaced that paragraph
with its push-order bullet in the commit that added this file (ti `6b4300`):*

- **The cause is excluded from the sync client, since 2026-08-17.** `.git` is
  now in the client's ignore list (`/Volumes/Common/GDrive/InsyncIgnore.txt`),
  which had previously excluded only rebuildable directories like `target` and
  `node_modules`. This **reverses** the earlier posture — excluding the
  repository had been weighed and declined under ticket `7a7feb`, leaving
  detection as the standing answer, and two restarts followed.

  **The exclusion did not work.** On 2026-08-19 the loss recurred with the line
  still in the file and the client still running: 13 objects missing,
  `git ls-tree -r HEAD` aborting at 151 of 342 paths. It also left **no
  collision copies** — the census read 0 while objects were vanishing — so a
  clean census is not evidence of anything. `git fsck` is the check.

  What repaired it was **redundancy**: every one of the 13 objects was found in
  the salvaged store at `/Volumes/Common/git-backup/transync-broken-git-20260817`
  and re-imported hash-verified.

*The section's closing paragraph:*

The commit hashes in this section — `494bc9c` above among them — name commits
that are **not in this object store**: the 2026-08-10 and 2026-08-17 restarts
replaced the graph twice. They are kept because the story is what makes the
repair legible, not because they can be checked out. See the two records named
at the top of this section for the archived stores that may still hold them.
