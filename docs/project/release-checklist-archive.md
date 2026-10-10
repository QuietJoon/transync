---
type: archive
title: Release checklist archive — the spent v0.4.0 carve-outs
description: The historical half of docs/project/release-checklist.md, moved out on 2026-10-06. It holds the owner decisions that v0.4.0 ships untagged (2026-08-13) and is tagged after all (2026-08-20), with their reasoning, and every step-level note those decisions and the v0.5.0 release made spent, each verbatim. It is an archive — verbatim, append-only, never read for current truth. The live checklist is docs/project/release-checklist.md.
tags: [archive, process, project-control]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-06T00:00:00Z
status: deprecated
---

# Release checklist archive

This is an **archive** (DCR-0054). `docs/project/release-checklist.md` is a living
document: it says how to cut a release now. Everything below was true once and
is kept verbatim, with its dates, commit hashes and record ids intact.
Paragraphs that sat inside a numbered step are shown without the step's list
indentation; their words are unchanged.

Read this to answer *why v0.4.0 shipped the way it did, and what each step said
for it*. Read the checklist for what a release does today.

## `docs/project/release-checklist.md` § When it applies — “v0.4.0 shipped untagged, then was tagged” (owner decisions 2026-08-13 and 2026-08-20)

> Archived 2026-10-06. Reason: the decision is superseded — `v0.4.0` was tagged on
> 2026-08-20 and `v0.5.0` on 2026-09-05 — and its one live consequence, the
> carve-outs in steps 3 and 10, stays stated in the checklist under *When it
> applies*, which also points here. Three records cite this subsection as
> “`docs/project/release-checklist.md`, under *When it applies*”:
> `docs/project/git-history-loss-2026-08-10.md`,
> `docs/project/git-history-loss-2026-08-17.md` and `CHANGELOG-archive.md`
> `[0.4.0]`. The subsection follows verbatim, heading included.

### v0.4.0 shipped untagged, then was tagged — owner decisions, 2026-08-13 and 2026-08-20

**Superseded. `v0.4.0` is tagged.** The decision below stood from 2026-08-13
until the release had already shipped, and the owner reversed it the same day
the release landed: an **annotated `v0.4.0` tag** now points at the release-prep
commit from step 24, and it is pushed to the `backup` mirror.

What reversed it was not a change of mind about this repository's history —
that reasoning, kept below, still holds — but a use the original decision did
not weigh: **consumers need a ref to depend on.** A tag serves that whether or
not the commit's ancestry is the development that produced the release. The tag
makes no claim about ancestry; it names which tree is v0.4.0.

So **steps 1, 15, 20, 25, 26, 27 and 28 run as written** — their v0.4.0
carve-outs are spent, and each says so where it stands. Two carve-outs are
**not** spent, because they were never about the tag: **steps 3 and 10** rest on
the history restart, and both facts are still true — no `v0.3.0` tag exists to
name, and this history is one root commit below the release, so a
`git log v<prev>..HEAD` or `git diff v<prev>..HEAD` range still cannot be
formed. Do not read "the tag exists now" as retiring those.

The original decision and its reasoning, kept because the next reader needs to
know why the release shipped the way it did:

**Scope: v0.4.0 only.** That release is recorded in `CHANGELOG.md` and in the
section F records, and **no `v0.4.0` tag is created for it**. Tagging resumes
at the next release, which runs every step below exactly as written.

The reason is a fact about this object store, not a preference. This
repository's git history has been restarted **twice** — on 2026-08-10
(`docs/project/git-history-loss-2026-08-10.md`) and again on 2026-08-17
(`docs/project/git-history-loss-2026-08-17.md`) — each time after the object
store lost objects to a file-sync client writing inside `.git`, 30 the first
time and 33 the second. Each restart replaced the whole graph, so **this
history has exactly one commit below the work in progress**: the restart
commit, whose message begins "chore: the repository restarts from a verified
working tree". Name it by resolving it rather than by quoting a hash — an
earlier revision of this file quoted the 2026-08-10 restart's hash, and the
2026-08-17 restart made that instruction unfollowable:

```bash
git rev-list --max-parents=0 HEAD    # the root commit of whatever history this clone holds
```

`git tag -l` returns **empty**: the three previously shipped tags did not
survive, and the commits they pointed at are not in this clone. Tagging v0.4.0
here would put a release tag on a commit that does not carry the release's
history.

Nothing below is deleted for this — every step that needs a tag needs one
again at the next release. Where a step cannot be followed without a tag
*now*, it says what to do for v0.4.0 instead: steps 1, 3, 10, 15, 20, 25, 26,
27 and 28.

*(Since the 2026-08-20 reversal that list has two live members, not nine.
Steps 1, 15, 20, 25, 26, 27 and 28 needed a tag and now have one, so each
carries a spent-note where it stands and otherwise reads as written. **Steps 3
and 10 are still live**, because they never depended on the tag: no `v0.3.0`
exists to name, and this history is one root commit below the release, so a
`v<prev>..HEAD` range cannot be formed either way.)*

## `docs/project/release-checklist.md` § A. Preflight, step 1 — v0.4.0 carve-out spent (2026-08-20)

> Archived 2026-10-06. Reason: spent — the `v0.4.0` tag points at the commit the
> carve-out fixed, so the step reads as written. Step 1 is unchanged.

v0.4.0's carve-out here is spent: it fixed one commit — the release-prep
commit from step 24 — and since 2026-08-20 the `v0.4.0` tag points at that
same commit, so the step reads as written with nothing removed.

## `docs/project/release-checklist.md` § B. Standing gates, step 8 — “Verified 2026-09-05” consumer premise and “Updated 2026-09-06” pin note

> Archived 2026-10-06. Reason: the second note amends the first, and both are now
> stated once in step 8 as current fact: both consumers pin
> `tag = "v0.5.0"` — moved there by dynwebserver `9ea9711` and resp-translator
> `d104793`, which step 8 still names — with no `[patch]` override or path
> dependency (re-read in
> `/Volumes/Common/QJoon/resp-translator/Cargo.toml` and
> `/Volumes/Common/QJoon/dynwebserver/Cargo.toml` on 2026-10-06), and what step 8
> owes them is the next release's migration items. Both notes follow verbatim.

**Verified 2026-09-05, and the step's original premise is spent.** It read
"every checkout below depends on `crates/transync` and `crates/transync-openai`
by **path**, so it compiles against this working tree". That is no longer
true of either: both now depend on the **GitHub tag** — `{ git =
"https://github.com/QuietJoon/transync.git", tag = "v0.4.0" }` — and neither
carries a `[patch]` override or a path dependency. So `cargo check
--workspace` in those checkouts compiles the **published v0.4.0 tree** and
says nothing whatever about the commit being tagged; running it and
recording a green would be evidence about the wrong tree. Check first,
then choose:

**Updated 2026-09-06.** Both consumers have since moved their pin from
`v0.4.0` to `tag = "v0.5.0"` — dynwebserver on 2026-09-05 (`9ea9711`),
resp-translator on 2026-09-06 (`d104793`) — so the migration items step 8
owed them for v0.5.0 are spent, and what this step owes now is the set the
**next** release will hand them. The 2026-09-05 paragraph above is a dated
observation and stands as written; the branch it selects has not changed.

## `docs/project/release-checklist.md` § D. CHANGELOG, step 15 — v0.4.0 commit-URL form spent (2026-08-20)

> Archived 2026-10-06. Reason: spent — both link entries were rewritten when
> `v0.4.0` was tagged, and `[Unreleased]` has since moved on to compare
> `v0.5.0...HEAD`. Step 15 keeps the stanza rule (the `v0.1.0`–`v0.3.0` entries
> stay as written), the `[0.4.0]` normal form and the two-commit warning; step
> 28 keeps the explanation of the second commit.

**v0.4.0's commit-URL form is spent, and it cost a second commit — which
is the lesson worth keeping.** While the release was untagged, `[0.4.0]`
was defined as a commit URL for the release-prep commit and `[Unreleased]`
compared that sha to `HEAD`. A commit URL names the sha of the commit it
ships in, which cannot be known before that commit exists, so the release
committed with a `RELEASE_PREP_SHA` placeholder and substituted the real
sha immediately afterward. **Expect two commits whenever a release ships
untagged**; a tagged release does not have the problem, because the tag
name is known in advance. Since the 2026-08-20 tag both entries were
rewritten to their normal form — `[0.4.0]` is
`https://github.com/QuietJoon/transync/releases/tag/v0.4.0` and
`[Unreleased]` compares `v0.4.0...HEAD`. Step 16's invariant is unchanged:
the heading still needs exactly one definition, and step 27 still requires
it to render as a link. The stanza's existing `v0.1.0`–`v0.3.0` entries
stay as written; they name refs those releases really carried when they
shipped.

## `docs/project/release-checklist.md` § F. Records, step 20 — v0.4.0 version-line carve-out spent (2026-08-20)

> Archived 2026-10-06. Reason: spent — the ref resolves. Step 20 keeps the rule the
> carve-out rested on, which still governs the workspace-version line.

v0.4.0's carve-out here is spent. While the release was untagged the line
read `v0.4.0 (untagged, owner decision 2026-08-13)` and pointed at *When it
applies*, on the rule that a version line naming a ref `git rev-parse`
cannot resolve is worse than one saying why there is none. That rule still
governs; it simply no longer applies, because the ref resolves. The line
now names the tag and the commit it points at, and says the tag was created
after the release and makes no claim about ancestry — `git log v0.4.0`
reaches one synthetic root, not the development that produced the release.

## `docs/project/release-checklist.md` § G. Commit and tag, step 25 — v0.4.0 tag skip spent (2026-08-13, reversed 2026-08-20)

> Archived 2026-10-06. Reason: spent. Step 25 keeps that it has no exception and
> the late-tag rule, both as they were worded here.

**v0.4.0's skip is spent.** It was skipped on 2026-08-13's decision and
then tagged on 2026-08-20 when the owner reversed it (*When it applies*),
so this step has no exception left in it. A release that tags late tags the
release-prep commit from step 24 — the same commit the CHANGELOG names —
and says in the tag message why it is late, because a tag whose date
trails its release is a question someone will ask.

## `docs/project/release-checklist.md` § G. Commit and tag, step 26 — v0.4.0 inverted tag check spent (2026-08-20)

> Archived 2026-10-06. Reason: spent — the inverted form ran once, for v0.4.0, and
> passed. Step 26 keeps the normal form; step 25 names the tags that exist.

**v0.4.0's inverted form is spent.** While the release was untagged this
check ran backwards — the first command had to print *nothing*, confirming
both that step 25 was skipped on purpose and that no tag had been
resurrected by accident. It was run in that form and passed. Since the
2026-08-20 reversal the normal form applies: `v0.4.0 tag 2026-08-20` and
`v0.5.0 tag 2026-09-05` are what it prints, and `%(objecttype)` reading
`tag` on **both** lines is what it proves.

## `docs/project/release-checklist.md` § H. After the tag, step 27 — v0.4.0 stand-in sentence spent (2026-08-20)

> Archived 2026-10-06. Reason: spent. Only the step's last sentence (“v0.4.0's
> stand-in … name the tag now.”) left the checklist; the whole step is shown
> verbatim, as it stood, for context.

27. **Re-read the top of the rendered CHANGELOG.** The new version heading
    must render as a link (the step-15 failure mode is a heading that renders
    as literal text), and `[Unreleased]` must hold only its placeholder line
    and compare against the new tag. v0.4.0's stand-in — the release-prep
    commit URL from step 15 — is spent; both entries name the tag now.
