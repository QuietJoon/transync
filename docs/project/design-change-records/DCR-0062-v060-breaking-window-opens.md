---
type: DCR
title: The v0.6.0 breaking window is open
description: "The owner decided on 2026-10-13 that the next version is v0.6. Under this project's own rule a breaking change needs a new window and the owner decision that opens one, so that decision opens the v0.6.0 window. Two breaking changes already committed to master and parked in the CHANGELOG's [Unreleased] section ride it: LineOffsets::offsets becoming private, and the Markdown intake's move from parser to intake::markdown. OI-0051, OI-0052 and OI-0054 are eligible for it but not required by it; the owner's recorded right to defer them again stands. Closes ticket 2d616b6a."
tags: [release, semver, breaking-window, DCR-0062, DCR-0053, R0010-0023, OI-0051, OI-0052, OI-0054]
generated:
  by: claude-code/claude-opus-5
  at: 2026-10-13T00:00:00Z
status: stable
---

# DCR-0062: the v0.6.0 breaking window is open

- **Date:** 2026-10-13.
- **Source:** the owner, asked which version should carry the work in flight,
  answered that it should go to v0.6. This project's rule is that "a further
  breaking change now needs a **new** window and the owner decision that opens
  one" (`status.md`, `release-checklist.md` step 2), so naming v0.6 as the next
  version *is* the decision that opens its window. This record states what that
  decision admits, and — equally — what it does not oblige.
- **Closes:** ticket `2d616b6a`, open since 2026-09-12: *"Two committed API
  changes and three registered items are all waiting on a breaking window
  nobody has opened."*
- **Behaviour changed:** none. A window is permission, not code. Nothing in
  this record ships anything.

**Numbering note.** Next free number after DCR-0061.

## What was actually stuck

Two breaking changes have been sitting on `master`, committed and merged, since
before the v0.5.0 window closed on 2026-09-05. Both are filed in `CHANGELOG.md`
under `### Changed (BREAKING)` inside `[Unreleased]`, and the preamble above
them has been stating the problem rather than resolving it: *"Two breaking
changes are sitting in this section, and they need a window."*

- **`LineOffsets::offsets` is private**, replaced by a read-only
  `line_starts() -> &[usize]` accessor (commit `43cc19c`, answering
  R0010-0023). The field and its `source` are one fact — every method reads the
  table as `new`'s output over that source — so an entry a caller could
  reassign describes bytes that are not there.
- **The Markdown intake moved from `transync_syntax::parser` to
  `transync_syntax::intake::markdown`** (commit `18c1ab4`, DCR-0053), making
  the format seam symmetric: `intake::html` and `intake::markdown` became
  siblings under the module that names the seam, which is the shape ADR-0025's
  D3 specified and DCR-0035 deferred.

Both are invisible through the `transync` facade — `public_surface.rs` pins the
modules hidden — and breaking only for a consumer depending on
`transync-syntax` directly. That is a narrow blast radius, and it is exactly
why the pair could sit parked without hurting anyone. What it could not do is
*ship*: a release carrying them with no window open would break the rule the
window ritual exists to enforce, and the only alternative on the table was
reverting work that answers a review finding and discharges a recorded
deferral.

The window ends the stalemate without either of those.

## What the window admits, and what it does not require

**It admits** the two changes above. They ride v0.6.0.

**It does not require** the three registered items queued against a v0.6.0
window — OI-0051 (every batch clones its profile and carries a second copy of
its glossary), OI-0052 (the checked provider constructors accept header values
that cannot become headers), and OI-0054 (the `intake::markdown` name
collision). They are **eligible** for it, which is what being queued meant, and
they are not a precondition of opening it or of cutting the release.

That distinction is the owner's, already on the record before this decision:
`docs/project/open-issues.md` under OI-0052 states that OI-0051 and OI-0052
*"may be deferred again when the v0.6.0 window opens"*, and `status.md` carries
the same ruling. Release-checklist step 2a lets a version state that it has no
window-closing condition. A record that quietly converted "eligible" into
"required" would manufacture an obligation the owner declined to take, and
would make this window's cut hostage to three implementations the owner has
twice said may wait.

So the window is open and its contents are not fixed. Each of the three is a
separate owner call, taken when it is taken; whatever is in the tree at cut
time is what the `[0.6.0]` CHANGELOG section enumerates, and that enumeration
is the window's count, exactly as it was for v0.5.0's seven.

**OI-0053 is not in this set.** It is a scope question about small-screen
layout in the shells, needs no window, and is listed here only because it is
easy to read it into the queue from the `[Unreleased]` *Tracked, not fixed*
section where it sits beside the other two.

## One consequence worth naming before the cut

OI-0051 is the only queued item whose break reaches **past** direct
`transync-syntax` consumers, and for one sibling it is demonstrable rather than
theoretical. It changes `transync::TranslationBatch`, which `contracts.md` §0
lists as tier (a) first-class API, so it reaches anything implementing the
`Translator` contract. `dynwebserver`'s `dynweb-core` builds that struct with
literals naming the field being removed, at two sites, and those stop compiling
when it bumps its pin. `resp-translator` receives the batch only as a
`Translator` parameter and binds it `_batch` in its one production impl, so
there the removal reaches tests at worst.

This is not an argument against OI-0051. It is the fact that decides whether it
rides *this* window or a later one, and it belongs in the record now rather
than in the cut.

## Version state

The root `Cargo.toml`'s `[workspace.package].version` already reads
`0.6.0-dev`, and has since the manifest moved with the work in flight; the
manifest is the one canonical record of the workspace version. A `-dev`
manifest is not the same as an open window — the manifest says what is being
built, the window says what may ship — which is precisely the gap this record
closes. Nothing about the version string changes here.

The last release is **v0.5.0**, tagged 2026-09-05, whose window is used and
closed with seven breaking-by-policy changes recorded in its CHANGELOG section.

## Gates

None. This record ships no code, so there is nothing to gate beyond the docs
drift suite that every record passes. The release that *closes* this window
runs the full ritual in `docs/project/release-checklist.md`, including step
2a's window-closing-condition check and the DCR-0015 Part C live-endpoint gate
— which, as of 2026-10-13, has four passing legs and is no longer the standing
obstacle it was at the three previous cuts.

## What this does not do

It does not cut a release, schedule one, or commit to a date. It does not
decide which of OI-0051, OI-0052 and OI-0054 ride it. It does not re-open the
v0.5.0 window, whose closure stands. And it does not change the rule: the next
breaking change *after* v0.6.0 needs a new window and a new owner decision,
exactly as this one did.
