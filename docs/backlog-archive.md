---
type: "Backlog"
title: "Backlog — Archive"
description: "Retired backlog entries kept verbatim for audit, each moved here by the commit that closed it, with the record that settled it named on the entry"
tags: ["project", "reference"]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-16T14:20:00Z
---
# Backlog — Archive

Entries moved from `docs/backlog.md` by the commit that closed them, once the
resolution was verified against code or a shipped artifact. Kept verbatim for
audit. Do not add new OPEN entries here, and do not edit an entry after it
lands — the line naming the record that settled it is the only thing this file
adds to what `backlog.md` carried. Superseded censuses and section prose from
`backlog.md` move here too, each under a heading that names its section and
date, with one lead line saying what replaced it.

## section-batch-coalescing _(owner-deferred to 2027, 2026-08-09)_

**Settled by DCR-0057 (2026-09-16), which supersedes the deferral.** The owner
moved the question forward and took option (b) — on by default rather than the
opt-in the entry anticipated, and with a second admission term (`DocumentFacts`)
the entry did not know it needed. Entry verbatim below.

- **Type:** 3
- **Description:** Section-coherent batching (DCR-0027, ticket `43cfb4`) confines every
  batch to one section, so a document of many small sections dispatches **one batch per
  section** where the old packer coalesced them — more requests, more latency, more
  per-request envelope cost, on exactly the heading-rich documents (reference pages, API
  docs, FAQs) where it hurts most. The relaxation is **cohort-restricted coalescing**:
  adjacent *whole* sections may share a batch when their effective glossaries are
  identical, so a co-batched section reads the same prompt it would have read alone —
  glossary exactness is preserved and no section's units are separated. For a
  glossary-free document every section shares one cohort, which restores the old batch
  count almost exactly. Shape if built: an opt-in `[batching].coalesce_sections` knob,
  default off.
- **Background:** DCR-0027's open question OQ-A. Strict confinement is the literal reading
  of the commissioned acceptance criteria, and its sanctioned exception splits a section
  *across* batches — it never merges two sections *into* one — so coalescing needs its own
  decision rather than riding the slice. The asymmetry is why (a) shipped first: (b) is
  cheap to add later and impossible to remove quietly once defaults depend on it.
- **Blocked by:** owner decision 2026-08-09 — **deferred until 2027**. A date rather than
  a condition because the natural condition ("real request-count pain observed") needs a
  consumer running heading-rich documents at volume and no such measurement exists yet;
  the date guarantees the question is re-asked, and the condition, if it fires first, is
  the reason to re-ask early. **No TicGit ticket, deliberately** — TicGit is the queue for
  work someone means to start, and a 2027 deferral is the opposite; this entry is the
  tracker.
- **Sources:** ticgit:bfe808e6-ab0b-4f32-8449-a70fdba217e7
- **Last seen:** 2026-09-12

## docs/backlog.md header — superseded censuses and sweep dating (2026-08-08 to 2026-09-08)

**Superseded by the 2026-09-12 census**, which stays in the `backlog.md` header (release-checklist step 20b: each census replaces the previous one). Moved from the `docs/backlog.md` header on 2026-10-06. Paragraphs verbatim below, in their original order; the first is the superseded tail of the header's opening paragraph: its first sentence and its Sources list stay in the header, and so does its source-derivation rule, with its opening word changed from "That" to "The" because the sentence it pointed back to is the one that moved.

Original note follows: last full update:
**2026-08-26** (the review-0009 track register). Three sweeps have run since the
2026-08-07 refresh this line used to date itself from, and each one's markers are dated
in place on the entries it touched: **2026-08-24** (`e89b537` — the index was missing 18
of 24 open tickets), **2026-08-25** (`7a1eaa0` — twelve deferrals audited, and the census
below became answerable) and **2026-08-26** (`e663422` — ten entries for thirty-six
review-0009 findings: the thirty-five the owner routed `track`, plus R0009-0014).
That 2026-08-07 refresh was derived from
`ti list --all` and `git log`, not from this file's own previous text — the review-0001
arc produced several tickets whose root cause was one document restating another, and the
index must not become one. Every sweep since has held to that rule.

**Census (2026-08-26).** Of the **64** entries below: **34 count against the gate**,
**23 are explicitly deferred with a recorded condition**, **7 are resolved or historical**.
The 2026-08-25 deferral audit (`7a1eaa0`) established **24 / 23 / 7** over the 54 entries
that existed then; the ten review-0009 entries `e663422` added the next day
(OI-0039..OI-0048) are all open work, and are the whole of the difference. Re-derive these
figures on each sweep rather than trusting them — an entry counts unless it carries a
resolution or a dated deferral line. The release-side half of this rule is
`docs/project/release-checklist.md` step 2a, which is where a release actually consults
the census.

**Census (2026-09-08) — post-v0.5.0; superseded by the 2026-09-12 census above.** Re-derived against
`ti list --all` and `docs/project/open-issues.md`, not read off the 2026-09-05 line below.
The **TicGit queue still holds exactly two open tickets** — ti `525bef96` and
ti `16721e90`, both still tagged `deferred-v050` with their re-triggers unchanged and
unfired — and both now have entries of their own under Type 3, which they did not on
2026-09-05. The **open-issue register is no longer empty**: OI-0051, OI-0052, OI-0053
were filed 2026-09-06 by the review-0010/0011 gate and OI-0054 on 2026-09-08, and all
four are blocked rather than ready — OI-0051/0052/0054 on a **sanctioned breaking window
that v0.5.0 closed** (the owner has recorded that OI-0051 and OI-0052 may be deferred
again when v0.6.0's opens), OI-0053 on a scope decision. OI-0049's deferral stands.
**Type 1 is empty**, since `parser-intake-markdown-rename` resolved 2026-09-07 (DCR-0053).
What that means for the *next* gate is the owner's to state, because v0.5.0's
window-closing condition was written for v0.5.0; nothing here counts against a gate that
has not been declared. Five Type 2/Type 3 entries that read as open on 2026-09-05 were
**already resolved** and are now marked so — tickets `6b43008a`, `473dd1e0`, `650bbba6`,
`7e2fd068` and `dd21ad59`, three of them since 2026-09-02/03; the 2026-09-06 pass that
reconciled nineteen entries was scoped to Type 1 and decision-made Type 2, so these were
never in it.

**Census (2026-09-05) — the v0.5.0 cut.** Re-derived against `ti list --all` and
`docs/project/open-issues.md` rather than read off the line above, as step 2a requires:
**zero** entries count against the gate. The **open-issue register is empty of counting
entries** — OI-0044 was resolved on all three of its members on 2026-09-05, and OI-0049
registers Review 0010's sixteen non-blocking survivors as *explicitly deferred* with a
three-way re-trigger. The **TicGit queue holds two open tickets and both are explicitly
deferred** with recorded reasons and objective, self-firing re-triggers, both tagged
`deferred-v050`: ti `525bef` (no active-formatting-element list and no adoption agency —
residual measured at `sec4a:break-nested` 8 per 10,000 generated inputs and
`reserved:dom-only` **0**; re-triggers on the census rising above 8 on an unchanged
`ATOMS` alphabet, on any `reserved:dom-only` case at all, or on a route in this family
being found by hand rather than by the oracle) and ti `16721e` (`noscript` read as markup;
re-triggers on the oracle measuring a §4a break or anchor loss on a `noscript` route).
That is the answer step 2a asks for, and it is what let v0.5.0 be cut. The 2026-08-26
figures above are left as written — they are that sweep's measurement, and this line is
the next one, not a correction of it.

**TicGit state (2026-08-08, after the Review-0002 gate):** 91 tickets, 78 closed,
**thirteen open**. Those thirteen are the six commissioned roadmap tickets in Type 1
(`fc0304`, `43cfb4`, `d3acc3`, `b791d6`, `bda471`, `f12b8b`), the three ticketed design
questions in Type 2 (`40e2a5`, `66339b`, `1347b4`), and four filed against the API
surface and its consumers: `1a85f3` (`TranslatorError::Other` conflates five terminal
causes), `43331a` (deadline/cancellation on `Translator` — DCR-0009's stated revisit
condition has fired now that a long-running service consumer exists), `2d8a9a`
(release-checklist step 8 knows only one path consumer), and `92abf5` (a rolled-back run
can still remove an empty directory a peer just locked). Two Type 2 entries carry an
`OI-` id rather than a ticket: the gate registers documents only, so they wait for
`reopen`'s selection gate.
Of the 34 tickets tagged `review-0001`, 33 are closed; the one survivor is Type 2's
`40e2a5`.

**TicGit state superseded (2026-08-24, `e89b537`).** The snapshot above is kept as the
2026-08-08 record and is not rewritten; it is no longer the current count. The 2026-08-24
sweep found **24** open tickets, all 24 now indexed here — 13 Type 1, 8 Type 2, 3 Type 3 —
and filed three more entries (OI-0037 `148fcf83`, OI-0038 `30a744f1`, OI-0016 `dd21ad59`).
Correlation between the two registers depends on the `ticgit:<8-hex>` spelling, not on
`` ti `xxxxxx` ``: an entry written the short way is re-filed as untracked by the next
sweep. Read the census above for the current answer to the release gate; read `ti list
--all` for the current queue.

## docs/backlog.md header — superseded census (2026-10-10)

**Superseded by the 2026-10-13 census**, which stays in the `backlog.md` header (release-checklist step 20b: each census replaces the previous one). Moved from the `docs/backlog.md` header on 2026-10-13. Two paragraphs verbatim below, in their original order: the census, and the paragraph that followed it as it read beside that census.

**Census (2026-10-10) — a count, not a sweep, and this is the current one.** Computed from
`ti list --open --json` and `docs/project/open-issues.md`. No other source was re-scanned, so the
last full sweep is still the 2026-09-12 one. **44 entries; 41 are live** — each carrying at least
one open TicGit ticket. By section: Type 1 holds 1 entry and it is live, Type 2 holds 19 of which
**17 are live**, Type 3 holds 24 of which **23 are live**. Three entries carry no open ticket:
`anthropic-leg-outside-the-release-gate-trigger` and `oversize-tombstones-point-at-a-closed-oi-0008`
cite none, and `phrasing-extension-list-for-custom-elements` cites only a closed one. **The two
registers do not agree in both directions.** TicGit holds **74 open tickets** (17 new, 57 blocked,
none in progress). Forty-one are cited by an entry and **33 have no entry here**. All 33 were filed
after the 2026-09-12 sweep, 32 of them by the documentation reconciliation and compaction runs
that followed it, and the next sweep adopts them. Until it runs, the queue is `ti list --open`, not
this file. The open-issue register holds nine issues: **4 OPEN** (OI-0051 through OI-0054),
**1 DEFERRED** (OI-0049) and four marked RESOLVED (OI-0040, OI-0041, OI-0046, OI-0048) that stay in
the register because each still has an open ticket against it. **What counts against a gate is
still the owner's to state**: no gate has been declared since v0.5.0, and whether a v0.6.0 breaking
window is open is undecided (ticket `2d616b6a`).

Re-derive these figures on each sweep rather than trusting them — an entry counts unless it
carries a resolution or a dated deferral line. The release-side half of this rule is
`docs/project/release-checklist.md` step 2a, which is where a release actually consults
the census. Each census replaces the previous one (release-checklist step 20b): the superseded
censuses of 2026-08-26, 2026-09-05 (the v0.5.0 cut), 2026-09-08 and 2026-09-12, and the TicGit
snapshots of 2026-08-08 and 2026-08-24, are kept verbatim in `docs/backlog-archive.md`. An entry
moves there too, under its own heading, once its resolution is verified against code or a shipped
artifact.

## docs/backlog.md header — superseded census (2026-09-12)

**Superseded by the 2026-10-10 census**, which stays in the `backlog.md` header (release-checklist step 20b: each census replaces the previous one). Moved from the `docs/backlog.md` header on 2026-10-10. Two paragraphs verbatim below, in their original order: the census, and the paragraph that followed it as it read beside that census.

**Census (2026-09-12) — the full sweep, and this is the current one.** Re-derived against
`ti list --open`, `ti list --all`, `docs/project/open-issues.md` and the source-C registers, not
read off the 2026-09-08 line below. **87 entries; 40 are live** — each carrying exactly one open
TicGit ticket, and every one of the **40 open tickets** has an entry, so the two registers agree
in both directions. By section: Type 1 holds 26 entries of which **2 are live**, Type 2 holds 32
of which **15 are live**, Type 3 holds 29 of which **23 are live**; the remaining 47 are resolved
or historical records retained deliberately. Six entries are **new this run** and all six were
filed as tickets at the selection gate: the v0.5.0 live-endpoint gate still unrun (`342e028d`),
the Anthropic leg never executed (`40b29a9d`), the unopened breaking window (`2d616b6a`), the disk
cache's exempt document-scoped records (`9c26d90c`), the `transync-local-llama` deferral with no
trigger (`3887f02e`), and three documents still calling the oversize deferral live (`2183fbbb`).
**Type 1 is no longer empty**, and what is in it is two items rather than one: the new
`mvp-scope-lists-retired-oversize-deferral`, and `adr-0014-anchors-without-their-successor`, whose
entry has read Type 1 since 2026-09-11 while its ticket stays `blocked` — a **deliberate**
divergence, documented in that entry, because 4B narrowed the ticket's broader claim to a settled
citation fix and this command may not reclassify a ticket it did not file. Sources scanned this
run: TicGit, `docs/project/open-issues.md`, `reviews/*.md` (top level — every round is gated and
archived, so it yielded nothing), `docs/decisions/*.md` (**zero** `Pending`),
`docs/project/stub-manifest.md` (every STUB and all 13 DEFERRED rows closed),
`docs/project/implementation-slice-checklists.md` (**zero** unticked boxes),
`docs/architecture/mvp-scope.md`, `docs/project/intake.md`, `docs/project/status.md`,
`docs/project/phase-state.yaml`, `docs/project/implementation-impact-report.md`, and all 53 DCRs.
Nothing was absent or unreadable. **What counts against a gate is still the owner's to state**, for
the reason the 2026-09-08 line gives: v0.5.0's window-closing condition was written for v0.5.0, and
no later gate has been declared.

Re-derive these figures on each sweep rather than trusting them — an entry counts unless it
carries a resolution or a dated deferral line. The release-side half of this rule is
`docs/project/release-checklist.md` step 2a, which is where a release actually consults
the census. Each census replaces the previous one (release-checklist step 20b): the superseded
censuses of 2026-08-26, 2026-09-05 (the v0.5.0 cut) and 2026-09-08, and the TicGit snapshots
of 2026-08-08 and 2026-08-24, are kept verbatim in `docs/backlog-archive.md`; the "2026-09-08 line" the current census
cites is the one kept there. An entry moves
there too, under its own heading, once its resolution is verified against code or a shipped
artifact — so the 2026-09-12 section counts above still include entries that have moved since,
until the next sweep re-derives them.

## adr-0014-anchors-without-their-successor

**Settled by ti `772a36` / commit `a67e704` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against code: all fourteen `ADR-0014` anchors in `crates/` now name `DCR-0027` within their own block, and both ti `772a36bb` and ti `6f52db2d` are closed/resolved. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `772a36`, commit `a67e704`). All four anchors now name `DCR-0027`
> beside `ADR-0014`, each in its own idiom — two prose doc comments, one `assert!` failure message,
> one inline comment — with `ADR-0014` retained at every site, because it is the record that defines
> what the scope meant. Re-censused after the edit: fourteen anchors, fourteen still citing
> `ADR-0014`, **zero** now lacking `DCR-0027` in their own block, against four before. Nothing
> asserted changed and `scn_09` passes. **Ticket `6f52db2d` stays open and blocked on purpose** —
> it is `reconcile-docs`'s finding, carrying its own `Needs decision:` marker and the broader claim
> the 2026-09-11 verdict refuted; only its filer may narrow or close it, and a comment there points
> at this work. Two corrections to this entry's own wording, neither affecting the fix: the fourth
> site is an inline comment rather than a `TRACE:` anchor, and the `llm.rs` site sits in
> `Translator::extract_glossary`'s doc, not in the `GlossaryScope` enum doc — whose own text cites
> neither record.

- **Type:** 1
- **Verified:** yes — reopen verified 2026-09-11 first-hand: `grep -rn 'ADR-0014' crates/` gives
  fourteen anchors, and reading twelve lines above each shows ten name DCR-0027 and four do not
- **Sources:** ticgit:772a36bb (the narrowed fix, resolved), ticgit:6f52db2d, docs/decisions/archive/0014-section-scoped-glossary-renders-globally.md
- **First seen:** 2026-09-11
- **Last seen:** 2026-09-12

### Description

Four places in the source cite `ADR-0014` with nothing pointing onward to the record that
superseded it. `ADR-0014` carries `status: deprecated` and its own description says "this is the
record of the rejection era, not current behavior", so a maintainer who follows one of those four
lands on a document that tells them it does not describe how the system works today and offers no
next step. Ten of the fourteen anchors already name `DCR-0027` nearby and are fine.

The four are `crates/transync-core/src/llm.rs` (the `GlossaryScope::GlobalAcrossDocument` doc),
`crates/transync-core/src/llm/prompt.rs` (the module doc and the force-global-scope assertion
message), and `crates/transync/tests/scenarios/scn_09_prompt_injection.rs`.

### Background

`ADR-0014` originally rejected the reserved glossary scope `conditional-on-section`: a glossary
entry meant for one section of a document would have steered every section, because nothing could
filter it per section. Its 2026-07-13 amendment turned the rejection into a **time-boxed
deferral** — the profile loader refuses the scope with `ProfileError::Unsupported`, and the scope
is "re-admitted the moment section-aware batching makes per-section filtering cheap". `DCR-0027`
(2026-08-09) is the change that fired that trigger: it made batches section-coherent and, in its
own title, made "the glossary section scope become real". So the successor reversed the
predecessor's operative holding rather than refining it, and both records say so — `ADR-0014`
carries `superseded_by` and `DCR-0027` carries the matching `supersedes`.

This item is the narrow residue of a wider claim that did not survive verification. Ticket
`6f52db2d`, filed by the `reconcile-docs` command on 2026-09-10, asserted that code was resting on
the reversed holding and named a sentence in `prompt.rs` as the exemplar. Reading the paragraph
that sentence sits in refutes that: the comment opens by stating the reversal outright, then gives
three current and independent reasons the *extraction* exclusion survives it — the extraction wire
schema carries no scope or sections field, the model is shown flat text with no section identity,
and no layer validates a returned glossary — and attributes the survival to `DCR-0027`, the live
record. The same ticket's claim that only one anchor cites the successor was also wrong: ten of
fourteen do. What is left is citation hygiene on four sites, which is why this is type 1 and not
the design question the ticket carries.

Nothing is blocked and nothing is owed a decision. The repository already contains the shape to
copy, since `prompt.rs` writes the paired form in one place. While it stays open the cost is small
but recurring and demonstrated: it is what led a documentation-reconciliation run to file an
over-stated ticket, and it will read the same way to the next reader or tool that follows one of
the four. Done means each of the four names `DCR-0027` beside `ADR-0014`, with no assertion text
and no behaviour changed.

### Options

The approach is settled, so what follows is the method rather than a choice. Add the successor
beside the predecessor at each of the four sites, in the paired form the file already uses, taking
each site in its own idiom: two are prose rationale in doc comments, one is the text of an
assertion message, and one is a `TRACE:` anchor, so a blanket string substitution would read
wrongly in at least two of them.

Two alternatives are ruled out by the item's own evidence rather than by preference. Retargeting
the citations to `DCR-0027` **alone** would lose the origin — `ADR-0014` as amended is still the
record that defines what the scope *means*, and `DCR-0027` describes the change, not the semantics.
Deleting the anchors would drop traceability that the `TRACE:` weld depends on.

One thing a reader should know: ticket `6f52db2d` is still open, still carries the broader
unnarrowed claim, and is still `blocked` with a "Needs decision" marker from its filer. This
command does not reclassify a ticket it did not file, so that divergence is reported rather than
reconciled; `reconcile-docs` owns closing it on its next run.

## open-issue-verification-boxes-unticked-on-eleven-resolved-entries

**Settled by ti `13fcede1` (2026-09-09).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `docs/project/open-issues.md` and `open-issues-archive.md`: the template sits under `## Entry template — not an issue`, OI-0016 has its verification block, the ten carry three ticked boxes each, and OI-0051..0054 keep theirs unticked. Entry verbatim below.


> **RESOLVED 2026-09-09** (ticket `13fcede1`). **And the count in this entry's own topic name is
> wrong: it is ten, not eleven.** Resolving it found why. The register carried a literal
> `## OI-NNNN: <Title>` template stub between OI-0016 and OI-0037 — placeholder fields, three
> unticked boxes — and because `NNNN` is not digits, a parser splitting on `## OI-\d{4}` does not
> see it as an entry boundary and folds it into the preceding entry. This sweep used exactly that
> pattern, so the stub's three boxes were counted against OI-0016 and the total came out one high.
> OI-0016 in fact had **no `### Verification` block at all** — the only resolved entry in the file
> without one, and the only one resolved by a measurement rather than a code change.
>
> Fixed, all three parts: the stub moved to the end of the file under
> `## Entry template — not an issue`, inside a fence and indented two spaces so neither a
> Markdown-aware nor a line-based parser can read it as a heading; OI-0016 gained a verification
> block whose first box says **"not applicable, and deliberately so"** rather than claiming a code
> change that was deliberately not made; and each of the ten — OI-0038, 0039, 0040, 0041, 0042,
> 0043, 0045, 0046, 0047, 0048 — now records evidence per box drawn from its own resolution, with
> the three partial resolutions (OI-0039's unadopted linter half, OI-0041's declined remainder,
> OI-0048's deferred R0009-0052) saying which box does not apply and why. No `Status:`,
> `Resolution:` or body prose was rewritten. Acceptance re-checked mechanically: zero non-issue
> `## OI-` headings, zero resolved entries without a verification block, zero resolved entries with
> an unticked box, and the four OPEN entries still unticked — correctly, since their work is undone.


- **Type:** 1
- **Verified:** yes — reopen verified 2026-09-09 by counting `- [x]` against `- [ ]` per entry in
  `docs/project/open-issues.md` and comparing with `docs/project/open-issues-archive.md`
- **Sources:** ticgit:13fcede1, docs/project/open-issues.md — the `OI-NNNN` template stub; OI-0016
  (no verification block at all); and the ten with unticked boxes: OI-0038, OI-0039, OI-0040,
  OI-0041, OI-0042, OI-0043, OI-0045, OI-0046, OI-0047, OI-0048
- **First seen:** 2026-09-09
- **Last seen:** 2026-09-09

### Description

Eleven entries in the live Open Issues register say `RESOLVED` on their `Status:` line and carry
three **unticked** `- [ ]` boxes under their own `### Verification` heading. The boxes are the
register's standard three — "Code change applied", "Tests pass (if applicable)", "No regressions
observed" — so each of the eleven simultaneously asserts that the work is done and records that
none of the three verification steps was confirmed.

This is not a claim that the work did not ship. It did: every one of the eleven names a date and,
in most cases, a commit. What is missing is the verification record, and its absence is
indistinguishable from the case the boxes exist to catch — an entry marked resolved whose fix was
never checked.

### Background

The Open Issues register is written by `indy-review-gate` and read by anyone auditing what shipped
and on what evidence. Its header states the retention rule: entries are removed once fully
resolved, and resolved entries **with audit value** move to `open-issues-archive.md`. So a resolved
entry that stays in the live file stays precisely because someone may need to audit it later, and
the `### Verification` block is the part of it that an auditor reads.

The checkbox convention is demonstrably live rather than vestigial. `open-issues-archive.md` holds
58 ticked boxes against 12 unticked. In the live register three resolved entries have all three
ticked — OI-0037 (resolved 2026-09-01), OI-0044 (2026-09-05) and OI-0050 (2026-09-06) — and those
include the two most recent resolutions in the file. The convention was therefore being honoured
both before and after the gap.

The eleven that are unticked cluster perfectly by batch: every one was resolved on 2026-09-02 or
2026-09-04, the review-0009 fix wave recorded in DCR-0040. A genuine per-entry verification gap
would not sort itself that neatly by date; a maintenance lapse during one large batch would, and
that is the reading the dates support. The four genuinely OPEN entries (OI-0051 through OI-0054)
are unticked correctly — their work has not been done — and OI-0049 carries no boxes at all because
it is a recorded deferral rather than a resolution.

It is type 1 because nothing has to land first and no one has to choose anything: the register's own
convention settles the approach, the evidence each entry needs is already cited inside it, and the
three most recent resolutions show the intended end state. Three of the eleven resolved only
partially and say so in their own `Status:` line — OI-0039 (the linter half deliberately not
adopted), OI-0041 (partial, the rest declined or deferred) and OI-0048 (R0009-0052 deferred) — so
those want a short note beside the boxes rather than a bare tick. That is work, not a decision.

Done looks like this: each of the eleven either has its three boxes ticked against the evidence its
own entry cites, or carries one line saying which box does not apply and why. A reader who then
finds an unticked box in the live register can trust that it means what it says.

## commissioned-roadmap-post-0.3.0 — **ALL SIX LANDED (2026-08-09/10)**

**Settled by DCR-0026, DCR-0027, DCR-0028 and DCR-0029 and the six roadmap tickets `fc0304`, `43cfb4`, `d3acc3`, `b791d6`, `bda471`, `f12b8b` (all closed/resolved; landed 2026-08-09/10).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06, together with the italic 2026-08-05-sweep note that sat under it, after spot-checking each landing in code. The Anthropic live leg it names as never executed is tracked by the Type 3 entry `anthropic-live-leg-never-run` (ti `40b29a9d`). Entry verbatim below.


- **Type:** 1
- **Description:** Six feature-scale work items the owner commissioned on 2026-08-06
  (decisions delegated to the controller and recorded in each ticket, tagged
  `owner-decision,roadmap`, priority 5). Each was actionable as-is. Four opened with a DCR
  before code per repo convention — `fc0304` and `bda471` with SL slice numbers too —
  while `d3acc3` and `b791d6` were already decided and started at implementation.
  **All six have since landed**, the last on 2026-08-10, so nothing in this entry is open
  work; it stays here as the roadmap's record. Recorded order, each with what it shipped:
  1. `fc0304` — the oversize row-window splitter (STUB-017 cluster): no single unit may
     abort a run. Scheduled FIRST. **Landed 2026-08-09** (DCR-0026, slices
     SL-100..SL-105): every obligation its DCR owed is discharged. Row-window batching
     carries header context — `unit::split` replaces an oversize table, in place and in
     document order, with windows that are each a complete GFM table holding the real
     header and delimiter rows, so invariant 3's ban on isolated cells holds by
     construction. The split is **packing, not retrying**, and happens exactly once
     before round one, which leaves ADR-0009's verbatim-resubmission budget untouched
     rather than amended. `regen::regenerate_table` is the plug-in point the windows
     reassemble through, and `pipeline::merge` runs between result aggregation and
     regeneration so a window id never reaches `regen::regenerate`, the alignment map or
     the DOM. Oversize *non-table* units are excluded, each with its ground stated in the
     record. Of the two advisory knobs, `default_table_strategy` became **effective**
     (the shipped default profile carries `row-window-first`) and `max_split_retries` was
     **removed** — a deterministic packing-time split has no split retry to bound, and a
     profile still carrying it now gets the ordinary unknown-key load warning. SCN-03
     stopped being stub-only: `scn_03_table_large.rs` drives the live path and asserts
     that two or more windows were dispatched, that they reassemble into one 200×4 table,
     and that the alignment map holds exactly one `Translated` row for it and no window
     id. STUB-017 closed with it.
  2. `43cfb4` — section-aware batching. Scheduled SECOND. **Landed 2026-08-09**
     (DCR-0027, slices SL-106..SL-109): batches are section-coherent,
     `[[glossary]].scope = "section"` is accepted end-to-end with a `sections`
     selector, and its `ProfileError::Unsupported` rejection retired at all three
     gates — which retired the Type-3 `glossary-section-scope` item in the same
     motion, as commissioned.
  3. `d3acc3` — the sync-engine pass. Scheduled THIRD. **Landed 2026-08-09:**
     single-file packaging re-affirmed (recorded in the module doc-comment and
     DCR-0008's dated note), ID-identity pairing documented as a normative
     schema-1.x invariant and *enforced* — a map whose row contradicts it is
     refused, which is also the `R0003-0002` resolution — and all three reflow
     hooks (ResizeObserver on both panes, `document.fonts.ready`, image
     `load`/`error`) built, re-collecting the anchor sets and re-driving the
     follower. OI-0015 and OI-0024 both close, and the Type-3
     `oi-0024-reflow-hooks` item is retired below.
  4. `b791d6` — `transync serve` becomes a real loopback static server (STUB-061
     un-deferred). Scheduled FOURTH. **Landed 2026-08-09:** the server binds
     `127.0.0.1:7470`, serves `GET`/`HEAD` only, confines every request to the served
     root in two layers (per-segment percent-decode, then canonicalize-and-contain,
     which is the one that sees a symlink escape), types responses from a fixed
     extension table, and shuts down cleanly on Ctrl-C/SIGTERM. Both statements it was
     scheduled to retire are retired — the exit-5 contract in `contracts.md` §6 and the
     "deferred stub — nothing binds" paragraph the Developer Guide's CLI reference
     gained in `a5ffe50` — along with STUB-061 itself and every `python3 -m
     http.server` the smoke paths reached for. `scripts/test-browser.sh` now serves
     SCN-13 with it, so the browser suite is evidence for the shipped server.
  5. `bda471` — `transync-anthropic` commissioned as the second provider crate
     (anthropic first, local-llama later); its landing supplies half of the Type-3
     `provider-capability-set` revisit condition. **Landed 2026-08-10** as DCR-0029,
     slices SL-115..SL-119: a rostered, publishable member implementing `Translator`
     over the Messages API, with construction-time configuration identity, a
     required output ceiling, a schema-profile pass for the provider's narrower
     dialect, an honestly-mapped stop-reason table, and a keyless end-to-end test
     that drives `transync::translate` over a loopback socket. One taxonomy variant
     rode along — `TranslatorError::ContextWindowExceeded` — and the core trait did
     not move, which is ADR-0002's promise exercised rather than merely restated.
     The `provider-capability-set` entry below is marked awake accordingly. The live
     leg is written and gated but **never executed**: there is no `ANTHROPIC_API_KEY`
     in this environment.
  6. `f12b8b` — the disk-backed cache design pass; its DCR must resolve OI-0017's one
     remaining item (detected-language persistence) explicitly rather than re-defer it,
     unblocking that Type-3 item. Its scope also absorbs three Review-0003 findings the
     gate routed here on 2026-08-09 rather than fixing standalone, because this pass owns
     eviction semantics and would re-decide any interim answer: `InMemoryCache` has **no
     capacity bound** (R0003-0048 — the routed one), its lookup cost (R0003-0049) and its
     key representation/interning (R0003-0051) are the same conversation, and R0003-0050
     was already a direct duplicate of this commission.
- **Background:** All six originate from the 2026-08-05 sweep's Type-2 section; the
  owner's 2026-08-06 ruling converted them from "needs decision" to "commissioned,
  scheduled post-v0.3.0". **All six have landed and all six are `closed` / `resolved` in
  TicGit** (re-checked 2026-08-16 with `ti show` on each id) — the entry survives here as
  the roadmap's record, not as open work. The "none has been started — all six are
  `state: new` and unassigned" note this line used to carry described the 2026-08-07
  refresh and was already stale by 2026-08-10.
- **Unfiled:** resolved and retained for the record; no ticket is owed

_The 2026-08-05 sweep's Type-1 work is fully resolved: its nine items landed 2026-08-05/06
(`0f137ed`..`3c9bc15`). The review-0001 arc that followed ran waves C–H over 2026-08-06/07
and closed out on 2026-08-07 with a documentation-accuracy tail — nine tickets fixing docs
that asserted what the code or a neighbouring document did not say, one cache/CLI fix
(`aa21e8e`, the `--source-language AUTO` sentinel), and the git-object-loss incident
`7a7feb`, repaired and then parked by the owner. v0.3.0 was cut at `30d467c` and tagged
locally only — no remote, nothing published — so post-release CHANGELOG material goes under
`[Unreleased]`. `reviews/reviewed/0001-dispositions.md` carries the per-finding table for the 52
findings of `reviews/reviewed/0001.md`; `ti list --status closed --tag review-0001` enumerates the
tickets; `CHANGELOG.md` carries the per-ticket entries under `[0.3.0]` and `[Unreleased]`._

## html-walk-foreign-content-breakout — **`e77173`, a live §4a break, and wave 3's precondition**

**Settled by ti `e77173` / DCR-0041 (2026-09-01), refined by DCR-0042, DCR-0043 and DCR-0051.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync-html` (breakout pop, integration points, and the pinned `<div class="wrap"><svg><div>x</svg></div>` case in `tests/divergence_tree_construction.rs`). Entry verbatim below.


> **RESOLVED 2026-09-01** (ti `e77173`, DCR-0041), and superseded twice since. `walk_elements`'
> single inherited `in_foreign` bool became a three-valued content mode per open element, so
> SVG's `foreignObject`/`desc`/`title` and MathML's text integration points return their children
> to HTML content and a breakout start tag pops foreign elements — which is exactly the
> `<div class="wrap"><svg><div>x</svg></div>` case this entry names. DCR-0042 then moved that
> stack down into `scan_tags` where the tokenizer needs the same answer, DCR-0043 made voidness
> an HTML-content rule, and DCR-0051 collapsed the crate's two open-element stacks into one.
> `contracts.md` §4 records the carve-out as CLOSED. Verified 2026-09-06.

- **Type:** 1
- **Verified:** yes — measured 2026-08-23 through the shipped DOMPurify mount by the
  wave-1 fix review, and again by the 2026-08-24 design review.
- **Sources:** ticgit:e77173bb, `docs/architecture/contracts.md` §4a (the carve-out),
  `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`
  (2026-08-23 amendment)
- **First seen:** 2026-08-22 · **Last seen:** 2026-08-24

### Description

`walk_elements` models neither HTML's foreign-content **breakout** tags nor its
integration points. `<div class="wrap"><svg><div>x</svg></div>` followed by a block
mounts that block's anchor **inside** the wrapper rather than as a direct child of
`<main>` — the §4a rule the engine's `offsetTop` math rests on. Reachable from
untrusted source Markdown through a type-6 html block, which invariant 7 says to
assume hostile.

**The self-closing slash is not the mechanism.** The *unflagged* `<svg><div>x</svg>`
does the same thing: `div` is on HTML's breakout list, so a browser pops the `<svg>`
at the `<div>` and continues in HTML content, while the walk closes it at `</svg>`
and passes the fragment through. So this is not wave 1's fix leaking — it is an older
gap that fix neither introduced nor worsened (`balance_fragment` output is
byte-identical old-versus-new for every one of these inputs).

### Background — why it is scheduled rather than parked

The ticket's own words put the fix-or-accept decision **before wave 3 lands**, and the
2026-08-24 design review found it had no owner: `e77173`, "breakout" and
`foreignObject` appear **zero** times in the wave 3–7 plans, in `status.md`, in
`open-issues.md`, and in this file. It lived in three places, none on an execution
path — which is what this entry exists to end.

Two waves are exposed. **Wave 3** consumes `element_extents` — the same walk — and its
deviation 6 makes the extents binding on phrasing-run boundaries, so on breakout inputs
the intake draws block boundaries a browser disagrees with, blesses an identity corpus
over them, and a later fix then re-derives block sets, **ids and section paths**. Ids
are this project's only sync currency; re-deriving them is not a code change. **Wave 6**
mounts every HTML-document block through the same balance pipe, and wave 7's browser
gate has no case that would catch it.

**Disposition: fix it, between wave 2 closing and wave 3 starting.** It is a live
correctness break today, its cost grows discontinuously at wave 3's blessing, and
ti `48f3c6` records that wave 1's stack-scoped foreign bit makes the modelling cheap
now that the bit exists — the expensive prerequisite is already paid. Not folded into
wave 2: that is the breaking window, and an orthogonal walk defect does not belong in
its diff. Wave 1's `transync-html` fix is the precedent for a scoped,
separately-reviewed fix between waves.

The ticket suggests the corpus entry that would have caught it:
`("selfclose-breakout", "<div class=\"w\"><svg><div/>x</svg></div>")` — the balanced
golden corpus has no foreign-content breakout entry at all, which is why five
`selfclose-*` entries could not see this.

## transync-html-wave-0-1-findings — five tickets from the wave 0/1 reviews

**Settled by tickets `95f55b`, `415cdb`, `4882ac`, `e20490`, `2e2453`, `48f3c6` (DCR-0042, DCR-0043) and `c1f9a8` / DCR-0047 (2026-09-03), all closed/resolved.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync-html`. Entry verbatim below.


> **RESOLVED 2026-09-03.** All five tickets landed: `95f55b` (a closer the balancer would bury in
> an unterminated region is withheld), `415cdb` (a stray `=` begins an attribute name, so the strip
> stops cutting across one), `e20490` (TAG-NAME and END-TAG-OPEN follow HTML through one shared
> `tag_name_end`), `2e2453` (raw text and RCDATA are HTML-content states — DCR-0042), and `48f3c6`
> (voidness is an HTML-content rule — DCR-0043). `c1f9a8`/DCR-0047 then replaced `95f55b`'s
> withhold with terminate-or-delete. Verified 2026-09-06.

- **Type:** 1
- **Verified:** yes — each reproduced by the review that filed it.
- **Sources:** ticgit:95f55b3a, ticgit:415cdb7f, ticgit:4882ac7d, ticgit:2e2453dd, ticgit:48f3c6d8
- **First seen:** 2026-08-22 · **Last seen:** 2026-08-24

### Description

| ticket | finding | class |
|---|---|---|
| `95f55b` | `balance_fragment` appends a closer that lands **inside** an unterminated trailing comment/CDATA/tag, so a re-balance adds another. 15,726 violations / 200k fuzz iterations | pre-existing, widened by wave 1's fix; **latent** — balance runs once on the render path, and the mount outcome is identical either way |
| `415cdb` | `collect_reserved_attr_spans` over-deletes across a stray `=`: `<div =data-sync-id="x">y` → `<div =>y`, moving a **non-reserved** attribute | pre-existing; over-deletion, so the impostor-anchor property holds (51-case probe, zero under-deletions) |
| `4882ac` | `image` is a documented `VOID_ELEMENTS` exclusion with no test or golden anywhere | test gap; the decision is held only by prose |
| `2e2453` | `scan_tags` enters raw-text state for `script`/`style`/`textarea`/`title` even inside `svg`/`math`, where a browser does not | scanner-level, pre-existing |
| `48f3c6` | a void name used as a real foreign element keeps `is_void` globally | deliberate trade, recorded |

### Background

These were filed rather than fixed because each is pre-existing and orthogonal to the
wave that found it. `415cdb` additionally **contradicts a shipped contract sentence** —
`contracts.md` §4's "no element name and no other attribute moves" — so it now carries a
carve-out beside that sentence, the way §4a's breakout carve-out was added. `48f3c6` and
`2e2453` are carve-outs the same foreign-content fix as the entry above would naturally
sweep in, which is a reason to schedule that fix rather than five separate ones.

## transync-html-one-open-element-stack — six tickets, one root cause — **RESOLVED 2026-09-05**

**Settled by DCR-0051 (2026-09-05) and its six tickets, all closed/resolved.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `walk_elements` replays the scanner's one stack. The residual it names, ti `525bef`, stays open under the Type 3 entry `adoption-agency-and-active-formatting-elements` and `contracts.md` §4b. Entry verbatim below.


- **Type:** 1
- **Verified:** yes — five reproduced by the reviews and the browser oracle that filed
  them; the sixth (`a7e625`) filed SPEC-TRACED and confirmed by Chromium before it was
  fixed, as it asked to be.
- **Sources:** ticgit:9b4d663e, ticgit:307283df, ticgit:895fb712, ticgit:da6bb5e0,
  ticgit:bb961a70, ticgit:a7e62558
- **First seen:** 2026-09-04 · **Last seen:** 2026-09-05 · **Resolved:** 2026-09-05

### Description

| ticket | finding | class |
|---|---|---|
| `9b4d66` | `scan_tags`' stale-frame `truncate` pops frames `walk_elements` and a browser keep, so `<title>` reads as HTML RCDATA and a planted `data-sync-id` survives `strip_reserved_sync_attrs` into a live DOM | **P1**, reopened OI-0035 |
| `307283` | `balance_fragment`'s `Close` pop is "any other end tag" with the guards removed: no special-element guard, no table scope. One §4a escape the balancer **creates** | P2, contracts §4a |
| `895fb7` | `balance_fragment` not idempotent on stale-frame input — appends `</math>`, then deletes it | P2 |
| `da6bb5` | the OVER-open direction: a stray `<td>`/`<tr>`/`<tbody>` start tag opens a frame HTML ignores, and implied end tags were nearest-match where HTML uses scope | P2 |
| `bb961a` | an ignored `</desc>` puts the crate in foreign content reading a CDATA section where Chromium is in HTML content reading a bogus comment — a live `data-sync-id` plus a `parent-td` §4a break | **P1**, OI-0035 again |
| `a7e625` | `mglyph`/`malignmark` keep their children in MathML; `child_content_mode` returned `Html` for every child of `mi`/`mo`/`mn`/`ms`/`mtext` | P2, was `needs-measurement` |

### Background

All six were the same defect: the crate kept **two** open-element stacks —
`scan_tags_with_state`'s `mode_stack` and `walk_elements`' `open_stack` — and they popped
differently from each other and from a browser. Fixed by **DCR-0051**, which makes the
scanner keep THE stack and the walk replay it, and teaches that one stack HTML's
special-element guard, its four scopes, foreign content's end-tag dispatch (with `</p>`
and `</br>` as breakout end tags), the start tags "in body" ignores, implied end tags by
scope, and the `mglyph`/`malignmark` dispatcher exception.

Measured against Chromium through the ti `ec235f` browser oracle, on an **unchanged**
atom alphabet so the numbers compare with DCR-0050's pin: agreement 7,697 → 8,619 of
10,000, mechanisms 13 → 9, `sec4a:break-nested` **1,358 → 8**, `reserved:dom-only`
**1 → 0**. Three of the four `KNOWN_DIVERGENT` routes went HEALTHY and were deleted from
both halves of the ledger.

**What is left, and it counts against the gate:** ti `525bef`, filed by DCR-0051 before
pinning its census, for the adoption agency algorithm and the list of active formatting
elements — the one part of HTML's tree construction the record deliberately leaves out.
Residual measured at 8 of 10,000 generated fragments and recorded in `contracts.md` §4b.

## git-object-loss-residual-blobs

**Settled by the close of ti `494a754c` (2026-09-06).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06; `git fsck --connectivity-only` re-run that day reported dangling objects only. The standing record is the two `docs/project/git-history-loss-*.md` documents (see `git-history-lost-twice-standing-record` in this archive). Entry verbatim below.


> **RESOLVED 2026-09-06.** Re-verified today: `git diff --stat HEAD` succeeds, and `git fsck`
> reports only dangling objects — no missing or corrupt ones. The two blobs this entry names are
> unreferenced debris, not tree content, and cannot be reconstructed; there was never anything
> left to repair. Ticket `494a754c` is closed/resolved, so the decision-free close this entry was
> waiting for has happened. The standing record of the two losses stays where it belongs, in
> `git-history-lost-twice-standing-record` and `docs/project/git-history-loss-*.md`.

- **Type:** 1
- **Verified:** partly — reopen verified 2026-08-24: `git diff --stat HEAD` now succeeds; the two named blobs still answer `could not get object info`
- **Sources:** ticgit:494a754c
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

Two blobs the ticket names are still unreadable in `.git`, but they are no longer HEAD blobs and the symptom the ticket is titled after — `git diff` over the whole tree aborting — no longer reproduces.

### Background

Filed when a plain `git diff --stat` aborted with `fatal: unable to read 1b7a866f…`. The 2026-08-17 restart at `59ce8df` re-created those paths as fresh blobs, so the unreadable objects became unreferenced debris rather than tree content. Nothing at HEAD depends on them and `git fsck` is clean. It is type 1 because the remaining action is a decision-free re-scope or close, not a repair — there is nothing left to recover, and the objects cannot be reconstructed. Left open rather than closed here because this command never closes a ticket it did not file. The related standing record is `ticgit:6b43008a`.

## developer-guide-names-two-of-three-browser-specs

**Settled by ti `729ec8ec`.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `docs/Developer_Guide.md` names all five browser specs. Entry verbatim below.


> **RESOLVED.** Verified in `docs/Developer_Guide.md` on 2026-09-06: the browser-suite row now
> names all five specs — `scn13.spec.js`, `scn16.spec.js`, `engine.spec.js`, `wasm.spec.js` and
> `html-oracle.spec.js` — and states that the runner is the authority on coverage, so
> `docs_browser_suite_drift.rs` welds every one of them.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:729ec8ec
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`docs/Developer_Guide.md` describes the browser suite as SCN-13 plus the wasm demo, but `web/tests/` holds a third spec — `engine.spec.js`, `sync.js`'s mount contract driven over a bare two-pane rig — which the same runner executes.

### Background

`playwright.config.js` sets `testDir: "./tests"` and `scripts/test-browser.sh` ends in a bare `pnpm exec playwright test`, so all three specs run. The weld `docs_browser_suite_drift.rs` requires living documents to name the spec files rather than publish a count, and it can only pin what the document names. A spec the guide omits is therefore a spec no weld protects: it could be deleted and both the document and the test would stay green. The fix is a one-line documentation edit plus confirming the weld then covers all three. Type 1 — nothing to decide, no prerequisite.

## playwright-outside-the-runner-validates-a-stale-bundle

**Settled by ti `ed2e73`.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `web/playwright.config.js` (stale-bundle refusal, `TRANSYNC_ALLOW_STALE_FIXTURE`). Entry verbatim below.


> **RESOLVED** (ti `ed2e73`). A bare `pnpm exec playwright test` now REFUSES a bundle older than
> its inputs: `playwright.config.js` compares the fixture's newest file against `crates/`,
> `web/js`, `web/vendor`, `web/wasm` and the two shell HTML files — deliberately not `web/tests`,
> so the documented spec-editing inner loop stays green — and `TRANSYNC_ALLOW_STALE_FIXTURE=1`
> downgrades it to a warning. Observed firing on 2026-09-06: a bare run outside the script
> refused with "No rendered bundle at …" rather than validating a stale one.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:ed2e73c9
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

A bare `pnpm exec playwright test` from `web/` skips the rebuild that `scripts/test-browser.sh` performs, so it validates whatever bundle the last full run left behind.

### Background

The script rebuilds the stub CLI, regenerates every bundle and rebuilds the wasm module on each invocation. The inner loop is documented in the wave plans as a legitimate speed-up with the caveat that any pass must be re-confirmed by a full run — but nothing enforces the caveat, and a green inner-loop run looks identical to a green full run. Pre-existing and suite-wide; surfaced by the adversarial review of wave 1 Task 2. Type 1 because the shape of the fix is settled: make the fast path announce what it did not rebuild, or make it refuse when the bundle is older than its inputs.

## cancellation-tests-assert-wall-clock-bounds

**Settled by ti `d41782` (2026-09-06).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync/tests/cancellation.rs` (no `Instant`; `drive_to_completion`, `reached_far_side`). Entry verbatim below.


> **RESOLVED 2026-09-06 — and it is ti `d41782`'s option 2, not a new idea.** One correction to this
> entry as filed: the bounds were no longer the 5 s ones it describes. `d41782` resolved on
> 2026-09-01 with its **option 1** — derive each bound from the signature it names, giving
> `CAPPED_BACKOFF / 2` (15 s) and `IGNORED_SLEEP / 10` (60 s) — and recorded in the same breath that
> **option 2, "assert the property with an instrumented drop counter", was NOT taken, "remains the
> strongest outcome and is still available"**. So this is the closing half of a decision already
> made, not a re-litigation of one: widening removed the recurring cost cheaply, but it left a
> stopwatch measuring a host, and one of the two stalls being ruled out is only 30 s, so widening
> has a ceiling. Both wall-clock bounds are now gone, along with the `Instant` import, and the
> property each existed to pin is measured by a clock-free instrument. A `drive_to_completion` helper drives the run one
> poll per scheduler turn against a 10,000-turn budget with a hook that fires the token between
> polls: a `yield_now` does not advance the clock, so a run sitting out a `tokio::time::sleep` stays
> pending on every turn regardless of how loaded the host is, while a raced sleep needs no timer at
> all. And `IndifferentAndSlow` gained a `reached_far_side` counter incremented only past its 600 s
> sleep, asserted zero after the cancelled run. **The 30 s and 600 s regression signatures are
> untouched, and both still fail against them** — the first because the run cannot finish without
> the timer, the second on either instrument. Test 1 also gained a non-vacuity assertion that the
> provider really was dispatched once; test 2's canceller became the sequenced hook, so cancellation
> still arrives from OUTSIDE the provider, which `contracts.md` §5b requires. tokio's paused clock
> was the other candidate and was rejected: `test-util` is enabled nowhere in the workspace.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:d4178267
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

Two tests in `crates/transync/tests/cancellation.rs` end in a 5-second wall-clock assertion against 30 s and 600 s regression signatures, and flake under load.

### Background

The bound exists to prove a sleep was raced against a cancellation token rather than waited out. It is a real property worth pinning, but wall-clock is the wrong instrument on a machine where this session measured `syspolicyd` parking fresh binaries at `_dyld_start` for 30–40 minutes and five foreign cargo processes contending for one target dir. A flake here reads as a cancellation regression, which is the most expensive possible false positive: it points an investigator at the retry policy. Type 1 — the fix is to assert on the observable (the token was observed, the provider was dropped) rather than on elapsed time.

## scratch-scripts-fall-back-to-tmpdir

**Settled by ti `13a73b` (2026-09-03).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `scripts/lib/workdir.sh` (`TRANSYNC_WORKDIR_POLICY` defaults to `refuse`). Entry verbatim below.


> **RESOLVED 2026-09-03** (ti `13a73b`). `scripts/lib/workdir.sh` is now the one place the rule
> lives, and the silent relocation is gone: an explicit override wins verbatim, the preferred
> `/Volumes/Temp/claude/<name>` is used when its parent exists, and otherwise the script REFUSES,
> naming the missing root and the override variable that fixes it. `TRANSYNC_WORKDIR_POLICY`
> defaults to `refuse`; `warn` restores the old behaviour in one edit, and an unrecognized
> spelling is itself a refusal (R0011-0061, 2026-09-06).

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:13a73be0
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

Five scripts prefer `/Volumes/Temp/claude/<name>` and silently fall back to `${TMPDIR:-/tmp}/<name>` when that parent is absent.

### Background

`test-browser.sh`, `smoke.sh`, `smoke-live.sh`, `smoke-live-long.sh` and one more all carry the same two-step rule. The workspace's standing rule is that every temporary artifact lives under `/Volumes/Temp/claude/` and that an unreachable volume is a stop-and-ask, not a fallback — precisely because a silent fallback puts build artifacts somewhere nobody is looking and nobody cleans. The fallback also makes the failure invisible: a run that should have halted instead succeeds against a different directory, and the operator learns nothing. Type 1 — replace the fallback with a refusal that names the missing path.

## transync-html-token-pin-reads-another-crates-fixtures

**Settled by ti `4fb858` (2026-09-04).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: the three fixtures under `crates/transync-html/tests/fixtures/` are byte-identical to their originals and the pin roots at its own crate. Entry verbatim below.


> **RESOLVED 2026-09-04 (`ticgit:4fb858`).** Option 1 — the corpus is vendored into the crate: `crates/transync-html/tests/fixtures/{scn-15-html-blocks,scn-14-full,reader-honesty}.md` are byte-verbatim copies, sha256-verified, so the pin no longer reaches through `CARGO_MANIFEST_DIR/../..` into a sibling crate. It was also the only admissible option: `publish = false` contradicts wave 0's roster decision, and the feature-gate variant this entry recorded is **barred outright** — `transync-html` may never carry a `[features]` table (CLAUDE.md, and the crate's own manifest header), because that would break the standing two-package `wasm32` gate. The ticket's own hold ("do not act during ti 490d97 waves 0-7 — the fixture freeze holds for the duration") expired when wave 7 landed.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:4fb85819
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`crates/transync-html`'s `token_stream_pin.rs` reads fixtures from `crates/transync/tests/fixtures/` via `CARGO_MANIFEST_DIR/../..`, which a published tarball does not contain.

### Background

The crate carries no `publish = false`, so it sits on the publication roster. A tarball ships the crate's own `tests/` but not another member's fixtures, so the pin cannot run from it. Nothing breaks today only because nothing has published from a tarball. The second consequence is worse in daily use: when the fixtures move, the pin fails with a file-not-found that reads like a corpus regression rather than a path problem, so the red misdiagnoses itself. Type 1 — either vendor the fixtures the pin needs into the crate, or gate the pin behind a feature that a tarball run does not enable.

## sync-role-for-catch-all-would-mis-classify-a-new-kind

**Settled by ti `9ffb97ee`.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `align::sync_role_for` is an exhaustive match with no catch-all arm. Entry verbatim below.


> **RESOLVED.** Verified in `crates/transync-syntax/src/align.rs` on 2026-09-06: `sync_role_for`
> has no `_ => SyncRole::Anchor` arm left. Every `BlockKind` is spelled out — including
> `Title => NonSync`, the arm this entry predicted the catch-all would swallow — behind the stated
> rule that "the next variant added to `BlockKind` has to be [a decision] before this compiles".
> The trap the entry asked for is the exhaustive match itself.

- **Type:** 1
- **Verified:** yes — gated finding register; **wave 2 Task 6 demonstrated it live**
- **Sources:** ticgit:9ffb97ee
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`align::sync_role_for` ends in `_ => SyncRole::Anchor`, so any `BlockKind` nobody named silently anchors — and `SyncRole` is wire-visible, deciding whether a row gets a DOM anchor and whether the engine counts it synchronizable.

### Background

This is no longer a hypothesis. Wave 2 Task 6 added `BlockKind::Title` and captured the catch-all swallowing it: the trap test failed with `left: Anchor, right: NonSync` before the explicit arm was written. That red is the ticket's claim, reproduced. Wave 2 worked around it deliberately rather than fixing it, because removing a catch-all is a change to every kind's dispatch and did not belong in a breaking-window wave. The related and sharper instance is `ticgit:18b9c34b`, where the renderer decides the same question from a literal match instead of asking this function at all. Type 1 — the approach is settled: enumerate the arms and delete the catch-all, letting the compiler demand a decision per variant.

## render-block-decides-anchoring-without-asking-sync-role-for

**Settled by ti `18b9c3` / DCR-0044.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `render::attrs` reads the alignment row's `sync_role`. Entry verbatim below.


> **RESOLVED** (ti `18b9c3`). Verified in `crates/transync-syntax/src/render.rs` on 2026-09-06:
> the sync-attribute omission is no longer decided in `render_block`. `attrs::write_attrs` reads
> the alignment ROW's `sync_role`, so every non-sync kind gets the same treatment with no second
> literal list to keep in step; the `ThematicBreak` arm survives only because an `<hr>` needs a
> different ELEMENT, not different attributes. This is the rule architectural invariant 1 now
> states (DCR-0044).

- **Type:** 1
- **Verified:** yes — reopen verified 2026-08-24: `grep -c sync_role_for crates/transync-syntax/src/render.rs` returns 0
- **Sources:** ticgit:18b9c34b
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`render_block` enforces the non-sync-implies-no-anchor rule with a literal `matches!(kind, BlockKind::ThematicBreak)` rather than by consulting `sync_role_for`, so a `Title` block would get a DOM anchor its own alignment row denies.

### Background

Verified: the renderer never calls the role function at all. Unreachable at wave 2's HEAD because nothing mints a `BlockKind::Title` yet — the Markdown intake cannot, and the HTML intake that will is wave 3's — so wave 2's suite is green on the merits rather than by luck. Wave 3 mints titles and wave 6 builds the HTML panes, which is where such a block first reaches a renderer on a live path. Wave 6 must extend the guard or state why a literal list is the right shape. Deriving it is the better fix and matches what wave 1 already did one layer up, when it made `synchronizableRowCount` derive from `synchronizableRowIds(...)` precisely so the mount predicate and the anchor predicate could not drift. Type 1 — the approach is settled and the deadline is wave 6.

## scan-tags-tag-name-and-end-tag-open-divergences

**Settled by ti `e20490` (2026-09-01).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: scanner and strip share one `tag_name_end`. Entry verbatim below.


> **RESOLVED 2026-09-01** (ti `e20490`). Both divergences are gone: `scan_tags`' TAG-NAME and
> END-TAG-OPEN states follow HTML, and the scanner and the strip share one `tag_name_end`, which
> closed the live bypass where `<divq"x=" data-sync-id="v">` was one element carrying a REAL
> `data-sync-id` that the old name boundary buried in a phantom quoted value. Verified 2026-09-06.

- **Type:** 1
- **Verified:** yes — gated finding register; owner ruled both in scope for a later task
- **Sources:** ticgit:e20490fe
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

Two deliberate divergences from HTML survive in `scan_tags`: one plants a real attribute the strip never examines, the other invents structure a browser never mints.

### Background

Wave 0 task 8 (`ti 549b20`) aligned `AttrState::Outside` with the browser and closed a measured strip bypass. The owner's 2026-08-21 ruling named these two as out of scope for that task and asked for them to be filed together, because they share a fix. Both are on the untrusted-source path that architectural invariant 7 says to assume hostile, and one is explicitly security-shaped: an attribute the strip never examines is an attribute the strip cannot remove. Type 1 — the owner already ruled, so nothing is undecided; what remains is the work. Related foreign-content gaps that the same stack-scoped bit would sweep in: `ticgit:2e2453dd`, `ticgit:48f3c6d8`, `ticgit:e77173bb`.

## manual-still-calls-serve-a-deferred-stub

**Settled by ti `0eee5c55`.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of the English manual pages (`manual/reference/user/en/cli.md`, `manual/how-to/operator/en/serve-the-demo-bundle.md`). Entry verbatim below.


> **RESOLVED.** Verified across `manual/` on 2026-09-06: `manual/reference/user/en/cli.md`
> documents `transync serve`'s full flag set, defaults, limits and exit codes with source
> references into `crates/transync-cli/src/serve_cmd*`, and
> `manual/how-to/operator/en/serve-the-demo-bundle.md` is a how-to for the real server. The only
> surviving "deferred stub" text is in `manual/log.md`, which is a dated log and correct as
> written.

- **Type:** 1
- **Verified:** yes — gated finding register
- **Sources:** ticgit:0eee5c55
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`manual/` still describes `transync serve` as a deferred stub that never binds, two weeks after the real loopback static file server shipped.

### Background

`transync serve` became real on 2026-08-09 (ticket `b791d6`); `contracts.md` §6 records that it supersedes the deferred-stub contract that stood from SL-13. The `docs/` bundle moved with it. `manual/` is derived one-way from `docs/` and did not. Found while working the exit-code tables, whose scope was table content only — this needs whole sections rewritten, which is a different job. The reason it went unnoticed is `ticgit:7e2fd068`: no weld machine-checks the manual's CLI reference, so a section can describe software that no longer exists and nothing fails. Type 1 — the correct text already exists in `docs/`; this is a derivation the manual pipeline owes.

## provider-payload-intake-guard (OI-0037)

**Settled by ti `148fcf` / OI-0037 (2026-09-01).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `validate::validate_unit` refuses an over-deep payload, and every validate layer reparses through `guarded_parse` (now `intake::markdown::guarded_parse`, DCR-0053). Entry verbatim below.


> **RESOLVED 2026-09-01** (ticket `148fcf`), per `docs/project/open-issues.md#OI-0037`. Provider
> payloads are refused at one door — `validate::validate_unit` — before any layer parses them, and
> every remaining provider-path reparse goes through `parser::guarded_parse`, which applies the
> same ceiling `parser::intake` applies. `structure.rs`'s three walkers are deliberately not
> routed; the OI records why.

- **Type:** 1
- **Verified:** yes — Review 0003 findings (R0003-0004, R0003-0005, R0003-0088), gate-accepted, user-routed track
- **Sources:** ticgit:148fcf83, `docs/project/open-issues.md#OI-0037`
- **First seen:** 2026-08-09
- **Last seen:** 2026-08-09

### Description

Source Markdown enters through `parser::intake` and its depth pre-scan
(ticket `07844d`); provider-returned payloads call `comrak::parse_document`
directly at five sites (`validate/fragment_reparse.rs`, `validate/inline.rs`,
`validate/per_kind.rs`, `structure.rs`, `validate/full_reparse.rs`), so the
ceiling that exists because stack exhaustion is an uncatchable abort governs
what the user wrote but not what the model returned. Route provider payloads
through one guarded entry point and convert a refusal into `ReparseFailure`
with attributed fallback ids, so the existing retry-then-fallback machinery
carries it.

### Background

Type 1, not 2: the approach is settled (one guarded entry point, refusal ->
`ReparseFailure`) and nothing has to land first — it is tracked rather than
fixed only because it spans five call sites and has **no reachable failure
today**. The reviewer's stack-exhaustion impact was refuted by the measured,
regression-pinned record in `parser/depth.rs`: comrak 0.27's block parse is
iterative, every untrusted-tree walker here was made iterative for exactly
this path, and memory is linear in the 32 MiB response cap. What survives is
that the safety rests on comrak internals rather than on anything this
repository asserts — a dependency upgrade could reintroduce a recursive block
parse with nothing going red. R0003-0088 is the test gap and cannot land
before the guard.

## degraded-regen-cascade-scans (OI-0042)

**Settled by OI-0042 (RESOLVED 2026-09-04).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: all three scans are indexed or `partition_point` sweeps. Entry verbatim below.


> **RESOLVED 2026-09-04.** All three sites linearized — including `validate/full_rescan_html.rs`'s twin, written 2026-09-03, eight days after the entry, and covered here rather than given its own row because splitting twins across two entries is how one of them rots. `widen_to_neighbors` also stopped comparing `String` where `usize` was available, which is why it was 8× its sibling: 26 ms / 245 ms / 1.9 s at 5,000 / 20,000 / 50,000 blocks, and N is the caller's document rather than this repository's — which is what moved it off the "cold path, therefore negligible" reading.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0033, R0009-0079),
  gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0042, reviews/reviewed/0009.md#R0009-0033,
  reviews/reviewed/0009.md#R0009-0079
- **Unfiled:** none owed — OI-0042 is RESOLVED 2026-09-04; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Two scans that are worse than linear, both on the path that runs only after
regeneration has already failed. `validate/full_reparse.rs`'s
`attribute_offenders` filters the whole `regen_top` collection once per entry,
with both collections sized by the document's top-level block count; its two
call sites are inside terminal error branches of `reparse_full` that return
`Err` immediately. `pipeline/finalize.rs`'s `widen_to_neighbors` does a linear
`BlockId` string comparison per seed (`top_level.iter().position(|id| *id == seed)`)
over the whole identity walk, and is reached only at stage 2 of the cascade,
after `reparse_full` failed **and** the stage-1 re-regeneration failed too.

### Background

Type 1: both fixes are settled and unblocked — one `HashMap<&BlockId, usize>`
for the seed lookup, and a sorted sweep or binary search over `regen_top` by
`start` for the offender scan. The review filed both as Medium hot-path
problems; verification placed both on a cold failure path that runs at most a
handful of times per run, up to three through the DCR-0004 cascade. Recording
that is the entry's main value — the next reader of the review should not
re-raise these as throughput issues. Verification also corrected the proposed
fix for R0009-0033: containment there is **range**-based (`top.start` inside the
entry's offset span), not id-based, so an ID lookup table is the wrong shape.

## module-size-versus-inline-tests (OI-0043)

**Settled by OI-0043 (RESOLVED 2026-09-04).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `crates/transync-cli/src/output.rs` is 411 lines over six submodules. The `transync-html` file-as-module question it mentions is held by DCR-0032's *Handed forward* list and the `CRATE_ROOTS` floor in `docs_ownership_drift.rs`. Entry verbatim below.


> **RESOLVED 2026-09-04.** `output.rs` 4,005 → 411 lines across five concern modules; the other three files close as **measured non-issues**, which is what this entry already suspected. The split is proven pure — an independent review found 119/119 item bodies and 49/49 test bodies **byte-exact** and the code string-literal multiset identical at 746/746, so the staging → fsync → rename → rollback ordering behind `--out-dir`'s all-or-nothing guarantee cannot have moved.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0068, R0009-0069,
  R0009-0070, R0009-0071), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0043, reviews/reviewed/0009.md#R0009-0068,
  reviews/reviewed/0009.md#R0009-0069, reviews/reviewed/0009.md#R0009-0070,
  reviews/reviewed/0009.md#R0009-0071
- **Unfiled:** none owed — OI-0043 is RESOLVED 2026-09-04; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Four files the review called oversized concern bundles from raw `wc -l`.
Re-split on `#[cfg(test)]` with brace tracking, three of the four are mostly
inline test code: `transync-html/src/lib.rs` is 2,550 lines but 1,242
production; `transync-core/src/profile.rs` is 4,035 but 1,738; and
`transync-core/src/pipeline.rs` is 4,981 but only 1,457 — 71% tests, with
`dispatch`, `finalize`, `merge`, `policy`, `report` and `retry` already living
in their own files. The exception is `transync-cli/src/output.rs`: 3,967 lines,
**2,041 of them production**, with only `lock.rs` extracted, so target
resolution, staging, bundle rendering and filesystem policy genuinely share one
module.

### Background

Type 1: the approach for each is settled — split `output.rs` under the repo's
file-as-module convention, and for the other two move the inline test modules
out (deciding per module between `tests/`, which loses access to private items,
and `src/<name>/tests.rs`, which keeps it). No behavioural defect anywhere; all
four are Low. The value of the verified table is that it stops the wrong work:
acting on the raw counts would mean re-cutting `pipeline.rs`'s orchestration,
which is already cut. R0009-0068 is a **re-raise of a known open item** —
DCR-0032's "Handed forward" section already names the file-as-module split, and
`crates/transync/tests/docs_ownership_drift.rs` encodes it as a `CRATE_ROOTS`
floor of 0, "a state nobody has decided against rather than a shape anyone
chose" — so it rides DCR-0032 and should be sequenced behind the HTML→HTML
waves still landing in that crate rather than racing them.

## disk-cache-log-edges (OI-0044)

**Settled by OI-0044 (RESOLVED 2026-09-05) and ti `0a3fca`.** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync-core/src/cache/disk.rs`. The stale `contracts.md` §6 sentence about the trim is tracked by the Type 2 entry `disk-cache-meta-records-evictability` (ti `9c26d90c`). Entry verbatim below.


> **RESOLVED 2026-09-05.** All three members fixed — the non-converging trim on
> 2026-09-03 (ti `0a3fca`), the poisonable write and the unbounded read on
> 2026-09-05 — each with a test observed red against pre-fix code. Two
> corrections to this entry. The write fix takes the *second* arm ("a poisoned
> writer") rather than a last-good offset, and closes the log instead of
> reopening it: an append-mode reopen positions at the same EOF the partial
> bytes end at, so it would weld exactly as before, while closing leaves those
> bytes as the file's last ones — a torn tail replay already repairs. And the
> read fix is deliberately smaller than "cap it": the cap is `Read::take` at the
> log's own default byte budget plus a resync, recorded as doc accuracy and
> insurance against accidental corruption, **not** as a threat-model change —
> ADR-0022 already disposes of R0004-0024, and a legitimately large record still
> has to be materialized because it lands in the index.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0080, R0009-0081,
  R0009-0082), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0044, reviews/reviewed/0009.md#R0009-0080,
  reviews/reviewed/0009.md#R0009-0081, reviews/reviewed/0009.md#R0009-0082
- **Unfiled:** none owed — OI-0044 is RESOLVED 2026-09-05; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

One finding on each of `crates/transync-core/src/cache/disk.rs`'s three paths.
Replay: `scan_log` reads a physical line with `.read_until(b'\n', &mut line)`
into an uncapped `Vec<u8>` and only decides the header afterwards, so a foreign
file with no newline is materialized whole before rejection — contradicting the
function's own claim that "peak memory is the index it is building plus one
record". Write: `buffer_record` returns `Err` with the `BufWriter<File>` still
installed in `DiskState`, and the next `put`/`evict` writes onto it; there is no
last-good-offset and `truncate_to` is never called from the write path. Trim:
`trim_to_budget` measures `header_bytes() + meta_bytes` plus entries but drops
only entries, so when `oldest_first` is exhausted the loop ends with the budget
still exceeded — while DCR-0028 §4 promises entries are dropped "until within
budget" and the public `max_bytes` doc names no exemption.

### Background

Type 1: each fix shape is settled — a floor plus a doc sentence for the trim, a
last-good offset or a poisoned writer for the write, `Read::take` (or a
corrected doc claim) for the read — and nothing must land first. Verification
narrowed two of the three substantially and sharpened the third. The write
poisoning is **one welded line, not open-ended corruption**: sub-capacity
records self-heal because `BufWriter::flush_buf` re-queues the remainder, so
damage needs a record at or above the 8 KiB buffer; the result is an unparseable
line `scan_log` skips with a warning, and a truncated JSON object concatenated
with a whole one cannot deserialize, so **no wrong value is ever served** — the
cost is two lost entries, inside DCR-0028's stated degrade-to-re-translation
envelope. The unbounded read's security framing was already answered: ADR-0022
puts a local writer to the cache path outside the threat model, naming
R0004-0024 — this same memory concern — by number. The trim is **worse than
filed**: if header plus `meta_bytes` alone exceeds `max_bytes`, every open drops
every unit entry and still forces a compaction, so the cache permanently retains
nothing across opens. No test covers a failing writer.

## serve-body-and-authority-reporting (OI-0045)

**Settled by OI-0045 (RESOLVED 2026-09-04).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync-cli/src/serve_cmd/{conn,host}.rs`. Entry verbatim below.


> **RESOLVED 2026-09-04.** All three fixed, each with a test proven red against pre-fix code. One correction to this entry: it says `contracts.md` carries the `[::1]` claim in prose and must move with the code — it does not. §6's authority sentence is family-agnostic; the enumeration lived in `docs/Developer_Guide.md` and `docs/Quick_Start.md`, both now corrected.

- **Type:** 1
- **Verified:** yes — Review 0009 findings (R0009-0002, R0009-0008,
  R0009-0009), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0045, reviews/reviewed/0009.md#R0009-0002,
  reviews/reviewed/0009.md#R0009-0008, reviews/reviewed/0009.md#R0009-0009
- **Unfiled:** none owed — OI-0045 is RESOLVED 2026-09-04; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Three residual `transync serve` findings after the Review 0009 fix pass. In
`serve_cmd/conn.rs`, the streaming branch announces `len` and then discards the
count `tokio::io::copy` returns (`.map(|_| ())`), so a file past the 8 MiB
in-memory limit that shrinks mid-response sends fewer bytes than its
`Content-Length`; the in-memory branch, by contrast, announces
`body.len() as u64`, and `read_capped`'s doc comment reasons only about a file
that *grows*. In `serve_cmd/host.rs`, one `is_unspecified()` branch pushes both
loopback families without consulting `local.ip()`, so a `0.0.0.0` bind — which
is IPv4-only — advertises `[::1]:port` in startup output and 421 bodies; and
derived entries plus every `--allow-host` go into one un-normalized `Vec`, so
`--bind 127.0.0.1 --allow-host localhost` prints `localhost:7470` twice.

### Background

Type 1: three small, settled fixes, nothing blocked. All three were filed above
their verified severity. The truncated-body case reaches only the >8 MiB branch
— the serve test's own comment says "Nothing in a bundle is this big" — and the
client sees a short read on a `Connection: close` socket rather than silent
corruption. The IPv6 advertisement costs guidance accuracy and one unreachable
allowlist entry; no verdict changes, because a client using `http://[::1]:port/`
never reaches the socket. The duplicate authority is cosmetic: lookup is
`self.answered.iter().any(...)`. Two constraints the work must respect: the
`::` case is **correct as written** (a dual-stack `::` listener does answer at
`127.0.0.1`), so only the IPv4 branch changes — and the fix is three edits, the
code plus the test that pins the wrong expectation plus the `contracts.md`
sentence that repeats it in prose; and the dedup must be order-preserving
retain-first, because `the_authorities_print_the_way_they_are_typed`
deliberately pins ordering.

## parser-intake-markdown-rename

**Settled by ti `b7e4cc70` / commit `18c1ab4` / DCR-0053 (2026-09-07).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: `crates/transync-syntax/src/intake/markdown/` exists and `src/parser.rs` does not. The naming collision it hands off is owned by OI-0054 / ti `cdadffea`, Type 3 `intake-markdown-intake-name-collision`. Entry verbatim below.


> **RESOLVED 2026-09-07** (ticket `b7e4cc70`, commit `18c1ab4`; DCR-0053). Done, and done under this
> entry's own acceptance condition: the diff is *only* the rename. `src/parser.rs` and its seven
> children moved to `src/intake/markdown/` by `git mv` — git reports all eight as renames at 94–100%
> similarity — and ~300 path references followed across four crates and eleven documents. Zero
> behaviour changed, and the workspace tally is identical to the pre-rename baseline: 47 targets,
> 1,359 passed, 0 failed, 8 ignored, with fmt, clippy, the two-package wasm32 gate, both rustdoc legs
> and biome all exit 0. Minimality was verified mechanically rather than by eye — the transform was
> replayed over each file's `HEAD` content and all 47 remaining `.rs` files are byte-identical to its
> output, so only three files are hand-edited: `lib.rs` (drops `pub mod parser;`), `intake.rs` (the
> deferral paragraph this change makes false, replaced by the symmetric statement), and
> `intake/markdown.rs` (one comment naming its own old filename). Four judgement calls are recorded in
> DCR-0053 rather than left to inference: `transync-core` keeps a **leaf** re-export so it still reads
> `crate::markdown::…`; `public_surface.rs` has **zero diff**, keeping `"parser"` as a re-introduction
> guard exactly as it still keeps `"htmlseg"`; living reference documents moved while every dated
> record did not; and the `intake::markdown::intake` collision the rename creates is left alone as a
> separate decision, owned by OI-0034.
>
> **Correction, 2026-09-08:** that attribution is wrong, and it was wrong when written above.
> OI-0034 is *comrak's NUL→U+FFFD substitution desyncs byte columns*, RESOLVED 2026-08-06 and
> archived — it owns the NUL normalization the guard performs, which is what the code's
> `TRACE: OI-0034` tags correctly cite, and it has never owned a naming decision. The collision's
> real owner is now `OI-0054` / ticket `cdadffea`, registered under
> `intake-markdown-intake-name-collision` in Type 3.

> **2026-09-06 — attempted, and deliberately not done in this tree; ticket `b7e4cc70`, state
> `blocked`.** Blocked on a commit boundary, not on a decision. This entry's own acceptance
> condition is that the paying wave's diff must be *only* the rename. The working tree currently
> carries ~50 files of verified but uncommitted work from the review-0010/0011 decision gate, and
> three of them are also on the rename's surface — `crates/transync-core/src/profile.rs` (a
> `crate::parser` reference), `crates/transync-syntax/src/parser/ranges.rs` (a file the rename
> moves), and `crates/transync-html/src/lib.rs`. Staging exact paths cannot separate two changes
> inside one file, so renaming now would produce exactly the unreviewable diff the deferral exists
> to prevent. Unblocks the moment the gate work is committed. Surface re-measured today: 106
> `parser` references in `transync-syntax`, 122 in `transync-core`, plus `transync-wasm`'s import
> and `public_surface.rs`'s hidden-module pin.

- **Type:** 1
- **Description:** The format seam is asymmetric on purpose — `intake::html` sits beside
  `parser`, not beside a sibling `intake::markdown` — because the rename is mechanical
  and large: ~43 references in `transync-syntax`, ~74 in `transync-core`, the wasm
  import, `public_surface.rs`'s hidden-module pin, and four documents, for **zero**
  behaviour change. Unblocked, pick-up-as-is; the paying wave's diff must be *only* the
  rename, or the review cannot tell the move from a change.
- **Background:** ti `490d97` wave 3 deviation 1; DCR-0035. Registered by wave 7's
  closure so the deferral survives outside the DCR.
- **Unfiled:** resolved and retained for the record; no ticket is owed

## serve-connection-task-panics-are-discarded (OI-0050)

**Settled by OI-0050 (RESOLVED 2026-09-06).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `crates/transync-cli/src/serve_cmd.rs` (`drain_connections`, `connection_exit_notice`). Entry verbatim below.


> **RESOLVED 2026-09-06**, the same day it was filed. All three `join_next()` sites hand the joined
> `Result<(), JoinError>` to one reporter, which stays silent for a task that finished on its own
> terms and otherwise names the death as a **panic** — carrying `JoinError`'s task id and payload,
> plus the sentence that it is a bug in transync rather than a peer failure — or a **cancellation**.
> Nothing became fatal; the accept loop keeps going. The shutdown drain was lifted verbatim into a
> private `drain_connections` so one real join site is reachable from a test, since the other two
> need a bound listener and a live peer and there is still no panic site in non-test `serve_cmd/`.
> Four tests. The per-connection I/O policy is untouched — R0010-0076 was dropped deliberately and
> its in-code comment remains that decision's record.

- **Type:** 1
- **Verified:** yes — Review 0010 finding (R0010-0077), gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0050, reviews/reviewed/0010.md#R0010-0077
- **Unfiled:** none owed — OI-0050 is RESOLVED 2026-09-06; this entry is retained for the record
- **First seen:** 2026-09-06 · **Last seen:** 2026-09-06

### Description

All three `join_next()` sites in `serve_cmd/` ignore the `JoinError` they may
receive, so a connection task that panics is indistinguishable from one that
returned normally. The server keeps accepting, and nothing — not stderr, not an
exit code, not a log line — records that a task died.

### Background

Filed LOW. There is nothing to swallow **today**: no panic site exists in
non-test code under `serve_cmd/`. That is a property of the current code, not a
guarantee it makes, which is why this is tracked rather than dropped — the
sibling finding R0010-0076 (per-connection I/O errors discarded) *was* dropped,
because its in-code comment "a dead socket is the peer's business" is already the
record for a deliberate policy. The re-trigger is self-firing: the first panic
site entering non-test code under `serve_cmd/`. Type 1 because the approach is
settled and nothing has to land first — match the `Err` arm and report a
panicked task distinctly from a peer disconnect.

## bin-only-crates-are-outside-the-rustdoc-gate

**Settled by the rustdoc gate's bin-crate leg (2026-09-06; no ticket owed).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification against `scripts/lib/rustdoc-gate.sh`, `scripts/smoke.sh` and `scripts/hooks/pre-commit`. Entry verbatim below.


> **RESOLVED 2026-09-06.** (Declared `**Type:** 1` while sitting under the Type 2
> heading until 2026-09-11, when it was moved into Type 1 to match its own field.) The gate gained a
> second leg — `RUSTDOC_GATE_BIN_CRATES=(transync-cli)` and
> `RUSTDOC_GATE_BIN_ARGS=(--document-private-items -p transync-cli)` in
> `scripts/lib/rustdoc-gate.sh`, run by both callers (`scripts/smoke.sh`, `scripts/hooks/pre-commit`)
> — plus a mirrored completeness check that fails on any member with no `src/lib.rs` but a
> `src/main.rs` that is not listed, since a bin-only crate can never appear in the existing
> `RUSTDOC_GATE_UNGATED`. It is a **second** `cargo doc` invocation rather than two more `-p`
> entries, because `--document-private-items` documents the private items and so stops
> `rustdoc::private_intra_doc_links` from firing — which is the failure DCR-0018 built the library
> leg to catch. The stale "no public API to document" comment now says why that is true and beside
> the point.
>
> **The link half needed no change, and that is worth recording rather than claiming a fix.** All
> ten links this entry describes were already repaired in commit `2ca092f` (2026-09-05) — the same
> commit that recorded the finding — including the four `super::X` references in
> `crates/transync-cli/src/output/lock.rs` and the tenth behind `#[cfg(not(unix))]`. Every remaining
> doc link in the crate was re-verified against its own module's item inventory. So what was missing
> was only the gate, which is exactly what this entry was about. Three documents that described the
> crate as deliberately outside the gate were corrected with it: `docs/Developer_Guide.md`,
> `docs/Troubleshooting.md` and `docs/project/release-checklist.md`.

- **Type:** 1
- **Verified:** yes — measured 2026-09-04 during OI-0043's `output.rs` split: nine broken intra-doc links,
  four of them in `crates/transync-cli/src/output/lock.rs`, **a file the split never opened**, because its
  `super::X` references silently became sibling references when the concerns moved out.
- **Sources:** OI-0043's resolution (2026-09-04)
- **First seen:** 2026-09-04 · **Last seen:** 2026-09-04
- **Unfiled:** resolved and retained for the record; no ticket is owed

### Description

Nothing in this repository would ever have reported those nine links. `scripts/lib/rustdoc-gate.sh` excludes
`transync-cli` by design — its own comment reads "`transync-cli` is absent on purpose — it is bin-only, with
no public API to document" — and `cargo clippy --all-targets -- -D warnings` does not check intra-doc links
at all. So a bin-only crate's rustdoc can rot without limit, and a refactor in one file can break doc links
in a file it never touched. A tenth break was latent behind `#[cfg(not(unix))]` and would have surfaced only
on a Windows build.

### Background

Distinct from OI-0046's rustdoc **completeness** sub-item, which asks whether every *library* member is
inside the gate; this asks whether bin-only members should be doc-checked at all. The cheap shape is a
`cargo doc -p transync-cli --no-deps --document-private-items` leg with `-D warnings`, which is what
diagnosed these nine — it costs one more `cargo doc` invocation and needs no new dependency. The reason it
was excluded was "no public API to document", which is true and beside the point: `--document-private-items`
is how a developer reads this crate, and that is the reader the links are for.

## mvp-scope-lists-retired-oversize-deferral

**Settled by ti `2183fb` / commit `2b5a650` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of `mvp-scope.md`, `source-of-truth-table.md` and `stub-manifest.md` and of the retired-phrase weld in `docs_ownership_drift.rs`. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `2183fb`, commit `2b5a650`). All three carriers now state the same
> resolution in the same sentence, and the phrase is guarded: `docs_ownership_drift.rs` carries
> `provider-side splitting stays deferred` as a retired claim paired with its replacement, traced to
> this ticket, DCR-0026 and ADR-0017. Verified both ways — the workspace gate is green (1,606 passed,
> 0 failed, `CARGO_EXIT=0`), and with the phrase re-inserted into a guarded document the test fails
> and names the file, the phrase and its replacement. Three corrections the work made to this entry's
> own reasoning. The splitter does not *halve*: `greedy_plan` sizes each window greedily against the
> output target, so all three rows say it **replaces** an oversize table with header-carrying row
> windows. The dated note reads as the deferral resolving rather than as a third `Closed 2026-08-09`,
> because the manifest's preamble twice names STUB-017 and STUB-061 as the last two closures on that
> date. And "deleting the rows would break the `TRACE:` anchors" does not hold — no `TRACE:` anchor
> anywhere names STUB-045; what is real is STUB-029's cross-reference to it, and the manifest's own
> rule that a closed row keeps its place with a dated note. `GUARDED` stays at its two documents by
> choice, so `mvp-scope.md` and `stub-manifest.md` remain unwatched by the weld — widening it is a
> decision to take deliberately, not a side effect of this repair.

- **Type:** 1
- **Verified:** yes — reopen verified 2026-09-12 first-hand against
  `crates/transync-core/src/pipeline/retry.rs`, which states the deferral is over, and against
  DCR-0026, which is the half that shipped; **re-verified 2026-09-13** by a second, adversarial
  pass (two independent lenses plus a completeness critic; verdict *partly confirmed*, type 1
  upheld by both). That pass corrected three facts carried here and in the ticket: the
  `source-of-truth-table.md` clause is not a leftover copy but was written fresh on 2026-09-08 by
  commit `0d403b9`, which took its wording from `mvp-scope.md`; the CHANGELOG entry that seeded the
  claim is the **v0.3.0** one (internal-quality wave, 2026-08-05), not v0.4.0; and the
  "adapter-internal batch pagination is genuinely undecided" sub-question is refuted — no document
  in the tree proposes it, and the only mention of that sense is a *rejected* alternative in the
  advisory `docs/investigation/` bundle
- **Sources:** ticgit:2183fbbb, docs/architecture/mvp-scope.md, docs/project/stub-manifest.md,
  docs/architecture/source-of-truth-table.md
- **First seen:** 2026-09-12
- **Last seen:** 2026-09-13

### Description

Three living documents still describe provider-side oversize handling as deferred work waiting
to be done. It is not waiting: one half shipped and the other was refused. `mvp-scope.md` says
"Provider-side batch pagination / split-on-oversize resubmit is **deferred** … The deferral
stands; only the dead placeholder is gone." The code's own record contradicts it —
`pipeline/retry.rs` says, of the `oversize_split` hook it removed, "The deferral this bullet
recorded is over, resolved in opposite directions for its two halves." Row-window splitting
**shipped** with DCR-0026, as *packing* rather than retrying: `unit::split::split_oversize_tables`
replaces an oversize table with header-carrying windows exactly once, before round one, and
`pipeline::merge` reassembles one table afterwards. The *reactive* form — re-scoping a unit after
a provider signals oversize — is **REJECTED, not deferred**, which is what leaves ADR-0009's
verbatim-resubmission rule untouched rather than amended.

### Background

The item reached this register the way the defect predicts: a `/reopen` sweep read the scope
document's DEFERRED table, found a deferral with no resolution beside it, and collected it as
open work — which is exactly what the next sweep, and the next reader, will also do. That is the
cost while it stands, and it is recurring rather than one-off. The research pass widened the item
from the one row this sweep first found to three carriers, because the stale sentence is repeated
rather than isolated — and the 2026-09-13 verification pass showed the repetition is still active:
`source-of-truth-table.md`'s copy did not exist on 2026-08-12, it was written on 2026-09-08 by the
architecture drift audit (`0d403b9`), which carried `mvp-scope.md`'s stale bullet into a tier-2
document with DCR-0026 cited in the same sentence; the entry's title reflects the wider finding rather than the narrower one it
started as. Type 1 because the answer already exists in two records and the work is transcribing
it: DCR-0026 says what shipped and in what form, and `pipeline/retry.rs` plus `unit/split.rs`'s
module doc say what was refused and why. Nothing is blocked, nothing is owed a decision, and no
behaviour changes. Done means each of the three rows states its own resolution and cites the
record for it.

### Options

The approach is settled, so this is the method rather than a choice. Rewrite each row in place to
say which half shipped (packing-time row-window splitting, DCR-0026) and which half was rejected
(reactive re-scoping after a provider signal, per `unit/split`'s module doc and contracts §5),
citing the record for each; take the three documents in one pass so they cannot disagree again.
Two alternatives are ruled out by the item's own evidence. Deleting the rows is ruled out by
`stub-manifest.md`'s own preamble — a closed row keeps its place with a dated note — and because
STUB-029's note cross-references STUB-045, so deleting that row would strand it. Retargeting them
at `contracts.md` alone does not work either, but not for the reason this entry first gave: §5 does
cover the refused route, in its no-re-split sentence ("no failure, provider signal, or retry round
ever changes a unit's scope after packing") and in its `Translator`-error paragraph, which cites
ADR-0017 and DR-2026-07 for rejecting a per-batch fallback rung. What §5 does not contain is any
deferral to cite — the word *defer* does not occur in the section at all — so a citation naming it
as the tracker of a deferral has nothing to land on. A second, genuinely
optional route named by the research pass and marked *(proposed by reopen)*: add a docs-versus-code
test pinning the retired phrase so it cannot reappear. It is separable and need not be chosen
before starting.

## status-step8-consumer-pins-expired

**Settled by ti `e2ac70` / commit `79ca0b6` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of `docs/project/status.md`. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `e2ac70`, commit `79ca0b6`). The 2026-09-05 sentences are **struck**,
> not rewritten — a dated observation stands as written under `reviews/README.md` rule 5, and this is
> the file's own `~~old~~ **dated correction**` idiom. The correction names both moving commits, the
> two later `transync-lang` edges, and the count as re-read on 2026-09-13: seven edges at `v0.5.0`,
> none at `v0.4.0`. One correction to this entry's own framing — the expired claim was not confined to
> the pin and the edge count; the present-tense `cargo check` consequence drawn from them had to fall
> inside the same scoping clause, which a reviewer caught before the edit was applied. **Still open
> elsewhere:** `docs/project/phase-state.yaml` carries the same expired claim and is now the only live
> document making it — registered below as `phase-state-repeats-the-expired-consumer-pin`.

- **Type:** 1
- **Verified:** yes — reopen verification 2026-09-13 first-hand: both sibling manifests were read,
  and `grep 'tag ='` returns seven edges all at `tag = "v0.5.0"` (resp-translator 3, dynwebserver 4)
  with zero at `v0.4.0`; `docs/project/release-checklist.md` step 8 already carries the dated
  2026-09-06 correction naming both moving commits
- **Sources:** ticgit:e2ac70ae, docs/project/status.md (the "Action (v0.5.0 tag)" bullet and its step-8 paragraph),
  docs/project/release-checklist.md (step 8's "Updated 2026-09-06" note),
  resp-translator/Cargo.toml, dynwebserver/Cargo.toml, reviews/README.md (rule 5)
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

`docs/project/status.md`'s "Action (v0.5.0 tag)" bullet still tells a reader that both sibling
consumers pin this workspace at `v0.4.0` across five edges, and that neither consumer is affected
until it bumps its tag. Both moved on 2026-09-05 (dynwebserver, `9ea9711`) and 2026-09-06
(resp-translator, `d104793`), and each has since gained a `transync-lang` edge — seven edges, all
reading `v0.5.0`, none reading `v0.4.0`. The pinning sentence itself carries "(verified
2026-09-05)", so under `reviews/README.md` rule 5 it is a dated observation that stands as written;
what has expired is the forward-looking claim beside it, which still reads as current.

### Background

The release checklist's step 8 already carries the dated correction, so `status.md` is the only
carrier still out of date, and the fix is a dated addition recording the move rather than a rewrite
of the dated paragraph. This drift is not hypothetical in its cost: it is what the 2026-09-12 sweep
read when it filed `342e028d`, whose "Why it matters" argued that the consumers would *later* move
onto a tree whose gate never ran. Both lenses of the 2026-09-13 verification refuted that
independently against the manifests, and the argument had to be inverted in the ticket — the
unattested tree is the one they already build against. A living document that states an expired
consequence beside a dated observation is read as current by anyone who does not open the two
manifests, which is exactly what happened.

## oi0030-verification-box-never-ticked

**Settled by ti `1e1e74` / commit `8c5722a` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of OI-0030's note in `open-issues-archive.md`. The live gate it names stays open as ti `342e028d` (Type 3 `v050-live-gate-unrun`) and ti `40b29a9d` (Type 3 `anthropic-live-leg-never-run`). Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `1e1e74`, commit `8c5722a`). The box keeps its unticked form by design:
> an archived issue is a dated record, amended by an appended dated note rather than re-ticked in
> place. The note names the v0.2.0 PASS of 2026-08-05 and both model names. One correction to the
> first draft, caught in review: naming only the `anthropic` leg as what remains would have read as an
> exhaustive residual and hidden the v0.5.0 gate, which `[0.5.0]` and `status.md` both record as
> TRIGGERED and NOT RUN — so the note says the per-release step is owed twice over and points at ti
> `342e028d`.

- **Type:** 1
- **Verified:** yes — reopen verification 2026-09-13 first-hand: the unticked box exists in
  `docs/project/open-issues-archive.md`, and the `[0.2.0]` CHANGELOG section dated 2026-08-05
  records `scripts/smoke-live-gate.sh` **PASS** with 2/2 live round-trips, which is the next release
  after OI-0030 resolved on 2026-08-03
- **Sources:** ticgit:1e1e743c, docs/project/open-issues-archive.md (OI-0030's verification block),
  CHANGELOG.md `[0.2.0]` and `[0.4.0]` (the two recorded gate PASSes), reviews/README.md (rule 5),
  ticket `342e028d` (which cited the stale box; provenance, not this entry's ticket)
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

OI-0030 ("Live-endpoint evidence is manual-only", RESOLVED 2026-08-03) carries one unticked
verification box: "First actual live run recorded (date + models) in the CHANGELOG at the next
release touching transync-openai / llm::prompt / the output schema / batching". That condition was
discharged two days later — the `[0.2.0]` section dated 2026-08-05 records the gate **PASS** with
2/2 machine-asserted live round-trips against `gpt-4o-mini` and `gpt-5-mini` — and v0.4.0's
2026-08-20 PASS satisfied it again. The box is a stale tick in a resolved record.

### Background

It is not inert. The 2026-09-12 sweep cited this box as evidence that a first live run is still
owed, and built the "record needed: none" line of one route on it; the 2026-09-13 verification had
to strike that support in ticket `342e028d`. Two of the three agents that read the box reached
opposite conclusions about it — one confirmed the box exists unticked, the other established its
condition was met — and both were right about their own half, which is what makes the stale tick
worth retiring rather than leaving for the next reader to re-derive. An archived issue is a dated
record, so the remedy is an appended dated note saying the box was discharged by the v0.2.0 run,
not an in-place flip of the checkbox: flipping it would silently re-date a record that
`reviews/README.md` rule 5 protects. Nothing external is required and no decision is owed — the
evidence that discharges the box is already in the CHANGELOG, twice.

## emitted-diagnostics-carry-flattened-space-runs

**Settled by ti `d8cb4f` / commit `a0ea7dc` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of both string literals. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `d8cb4f`, commit `a0ea7dc`). Both literals now end each continued line
> with a backslash. Whitespace only: the placeholders and their order are byte-identical, so argument
> binding cannot shift, and the two tests that assert these messages still pass. The grep for space
> runs inside string literals found no third production site — the remaining matches are test fixtures
> where the spacing *is* the fixture.

- **Type:** 1
- **Verified:** yes — reopen verification 2026-09-13 first-hand: the clamp warning in
  `crates/transync-core/src/cache/disk.rs` carries a run of fourteen spaces between "eviction" and
  "cannot reclaim" in both the working tree and at HEAD, and the staging-sweep note in
  `crates/transync-cli/src/output/publish.rs` carries runs of the same length; a repo-wide grep for
  space runs inside string literals found no third production site
- **Sources:** ticgit:d8cb4f1a, crates/transync-core/src/cache/disk.rs (the `trim_to_budget` clamp warning),
  crates/transync-cli/src/output/publish.rs (the staging-sweep `notify` note),
  reopen verification 2026-09-13 of ticket `9c26d90c` (provenance, not this entry's ticket)
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

Two operator-facing messages carry long runs of embedded spaces where a line-continuation backslash
was evidently dropped, so the emitted text is mis-spaced rather than merely wrapped oddly in the
source. The OI-0044 clamp warning in the disk cache contains a fourteen-space run between
"eviction" and "cannot reclaim", and two more before "ever satisfy" and "Raise"; the CLI's
staging-sweep note contains runs of the same length before "by a crashed earlier run" and "there
stay where they are".

### Background

Both are user-visible. The first goes out through `tracing::warn!` on `transync::cache` at every
open whose byte budget is at or below the unreclaimable floor; the second through `notify` during
publication housekeeping. No test catches either, because the clamp test asserts only that the
emitted line contains "cannot reclaim" and the sweep note is not asserted at all — so the defect is
invisible to the suite by construction, not by oversight. The fix is the one Rust spelling that
exists for a wrapped literal: end each continued line with a backslash so the following indentation
is stripped, which changes the emitted text only by removing the space runs. It crosses two crates,
one of which has nothing to do with the cache, which is why it is filed apart from the ticket that
surfaced it. It was found the way such things usually are — an evidence-fidelity verifier noticed
that a quotation of the warning inside ticket `9c26d90c` had been silently whitespace-normalized,
and went to the literal to find out which side was wrong. The remaining grep matches are test
fixtures, where the spacing is the fixture.

## translator-error-variant-window-record-contradiction

**Settled by ti `bcde64` / commit `ef7bf4e` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification of DCR-0029's dated note. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `bcde64`, commit `ef7bf4e`). DCR-0029 carries a dated appended note:
> variant **additions** are non-breaking under the `#[non_exhaustive]` rule (OI-0027, resolved
> 2026-08-04), so the open v0.4.0 window was never the operative reason — removals and renames are
> what ride a window. Appended, not rewritten, so the fork's original reasoning stays readable as what
> was believed at the time; `contracts.md` §1 is untouched because it is tier 1 and already right. All
> nine reviewer findings were accepted rather than argued, including the removal of an ordinal no
> artifact supported and of a motive the owner decision does not carry. The attribute's own landing
> date stays unestablishable: history restarts at `59ce8df` on 2026-08-17.

- **Type:** 1
- **Verified:** yes — reopen verification 2026-09-13 first-hand: DCR-0029's taxonomy fork presents
  the new variant as carried by the open v0.4.0 window, `docs/architecture/contracts.md` §1 states
  the `#[non_exhaustive]` rule that makes a variant addition non-breaking, the variant's own §1 row
  says exactly that, and `crates/transync-core/src/llm.rs` carries the attribute at HEAD
- **Sources:** ticgit:bcde64b3, docs/project/design-change-records/DCR-0029-transync-anthropic-second-provider.md
  (the taxonomy fork and its resolution), docs/architecture/contracts.md §1,
  docs/project/open-issues-archive.md (OI-0027, resolved 2026-08-04),
  crates/transync-core/src/llm.rs, ticket `3887f02e` (surfaced while verifying it; provenance, not
  this entry's ticket)
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

Two live records disagree about whether adding a `TranslatorError` variant is a breaking change.
DCR-0029 presents `ContextWindowExceeded(String)` as a new variant "carried by the open v0.4.0
window" and recommends it because the window "is open now and will not be later", repeating the
window as the reason in its resolution. `contracts.md` §1 states the landed rule the other way —
the three error enums are `#[non_exhaustive]` (OI-0027), so variant additions are non-breaking —
and the variant's own §1 row closes with exactly that.

### Background

The chronology makes this more than wording drift: OI-0027 is archived as resolved on 2026-08-04,
six days before DCR-0029, so the window DCR-0029 believed it was spending may never have been
needed. The consequence is forward-looking rather than historical. The sanctioned v0.5.0 window
closed on 2026-09-05 and a further breaking change needs a new one, so a reader planning a third
adapter, a new terminal cause, or any taxonomy change will be guided by whichever of the two
statements they read first — and one of them says the change must wait for a window that does not
exist. The fix is the house amendment pattern, a dated note on DCR-0029 reconciling it with §1,
with no code change and no external prerequisite; the approach is settled because the repository
has shipped this exact correction class as docs-only before. One thing cannot be established from
here: the attribute's own landing date, because this repository's git history restarts at `59ce8df`
on 2026-08-17 and OI-0027 predates that. The ticket that surfaced it (`3887f02e`) quotes both sides
deliberately rather than averaging them, which is right for a ticket whose subject is something
else, and leaves the contradiction standing in the records themselves.

## phase-state-repeats-the-expired-consumer-pin

**Settled by ti `925960` / commit `ef8a542` (2026-09-13).** Moved from `docs/backlog.md` § Type 1 — Ready on 2026-10-06 after re-verification: the past-tensing parenthetical now sits in `docs/project/phase-state-notes.md`, where wave 2 moved the phase-state log. Entry verbatim below.


> **RESOLVED 2026-09-13** (ti `925960`, commit `ef8a542`). Appended as a dated parenthetical in the
> file's own past-tensing idiom — the shape the same block already uses for the breaking window that
> was used and closed — so the dated observation stays as written. With this carrier corrected, no
> live document says the consumers pin `v0.4.0`. `yq` re-parses the file with identical top-level
> keys, and `crates/transync/tests/docs_index_drift.rs` (the one test that reads the path) is green.
> One judgement recorded rather than taken: `project.last_updated` is left unbumped, because the
> history shows record work bumping it and drift sweeps not bumping it, and this is a drift fix.

- **Type:** 1
- **Verified:** yes — reopen verification 2026-09-13 first-hand, by the review lens that read
  `status.md`'s correction: `docs/project/phase-state.yaml`'s dated log segment carries the same
  "neither consumer is affected until it bumps" reading that `status.md` and
  `docs/project/release-checklist.md` step 8 have both now corrected
- **Sources:** ticgit:9259603f, docs/project/phase-state.yaml (the dated log segment preceding the 2026-09-07
  segment), docs/project/status.md (corrected 2026-09-13, commit `79ca0b6`),
  docs/project/release-checklist.md (step 8, corrected 2026-09-06),
  ticket `e2ac70ae` (whose review surfaced it; provenance, not this entry's ticket)
- **First seen:** 2026-09-13
- **Last seen:** 2026-09-13

### Description

`docs/project/phase-state.yaml` still carries the claim that both sibling consumers pin this
workspace at `v0.4.0` and are unaffected until they bump. With `status.md` corrected on 2026-09-13
and `release-checklist.md` step 8 corrected on 2026-09-06, it is now the **only** live document
making that claim.

### Background

The claim expired on 2026-09-05 and 2026-09-06, when dynwebserver (`9ea9711`) and resp-translator
(`d104793`) moved their pins; seven edges now read `tag = "v0.5.0"` and none reads `v0.4.0`. The
file is a phase-state log, so the fix follows the same rule its siblings did: the dated segment is
a record and stays as written, and a dated addition records the move rather than rewriting the old
reading. The approach is settled by two precedents in this repository from this week, which is what
makes this type 1 rather than a question. It is registered rather than folded into ti `e2ac70ae`
because that ticket's Scope named `status.md` alone, and widening a claimed ticket's scope
mid-flight is how a verified fix turns into an unverified one. The cost of leaving it is the cost
the `status.md` copy already demonstrated: the 2026-09-12 sweep read that copy and filed an
argument into ticket `342e028d` that both verification lenses then refuted against the manifests.
One carrier is enough to do that again.

## Type 2 — Needs decision: section preamble _(docs/backlog.md, last dated 2026-09-08)_

**Section prose, not an entry.** It moved here with the resolved entries it described —
`publish-lock-nested-tree-boundary`, `out-dir-foreign-staging-temp`,
`docs-index-orphaned-trees`, `sync-anchor-injection-via-raw-html`,
`out-dir-sparse-bundle-subset` and `warm-cache-run-needs-no-credentials`, all below — and the
section's current preamble in `docs/backlog.md` replaces it. Text verbatim below.

The open design questions here carry an `OI-` id rather than a ticket: a review gate
registers documents only, so each waits for `reopen`'s selection gate to offer it and
file a ticket for whatever is picked. The ticketed questions this section once listed —
`40e2a5`, `66339b`, `1347b4` — were all resolved on 2026-08-10 and are gone from it.

`sync-anchor-injection-via-raw-html` (OI-0035) asked which layer should own anchor
trust in the browser — resolved 2026-08-23 (route (c), both layers; ti `490d97` wave 1,
DCR-0033); its entry below is retained for the record. `out-dir-sparse-bundle-subset` (OI-0036) was settled with `66339b`
by the marker-file rule and is retained only for its record. `provider-payload-intake-guard`
(OI-0037) sits in Type 1 below, since its approach is settled.
`warm-cache-run-needs-no-credentials` (OI-0038) was **RESOLVED 2026-09-02** (`cdc1e32`,
DCR-0046, ticket `30a744` closed) — `transync translate --offline` runs from a warm cache
with no credential — so it is neither the newest nor a pending product question; the
entry directly beneath it says so. OI-0039 through OI-0054 followed it.

**Read this section as a record, not a queue** (2026-09-08). Every entry below is either
resolved-and-retained or explicitly deferred with its blocker stated; the three ticketed
2026-08-10 resolutions named above are retained below rather than "gone from it". The
only entries here that are genuinely open work are the ones whose own note says so.

## dcr-0032-divergence-tally-not-reconstructible

**Settled by ticket `3f0879ef` (closed 2026-09-13, commit `9bbbf59`), which took none of
the three routes below:** records are write-once under DCR-0054, so the enumeration went
to a living document instead — `docs/architecture/transync-html-divergences.md` lists all
nineteen divergences with date, ticket and record, and `CLAUDE.md` points at it. Verified
2026-10-06: that table has nineteen numbered rows, and `CLAUDE.md` and `docs/index.md` link
it. Entry verbatim below.

- **Type:** 2
- **Verified:** yes — reopen verified 2026-09-09: DCR-0032's amendment ordinals were read directly
  and jump from unnumbered prose to "the fifteenth through nineteenth divergences"
- **Sources:** ticgit:3f0879ef, docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md, CLAUDE.md
- **First seen:** 2026-09-08
- **Last seen:** 2026-09-12

### Description

`CLAUDE.md` tells every agent that the record trail "records **nineteen** deliberate behavioural
divergences" of `transync-html` from the retired `htmlseg` module, and that a difference from
`htmlseg` in any of the nineteen is the **fix** rather than a regression to restore. That sentence
is what stops the next reader from "repairing" a deliberate behaviour change. But the nineteen
cannot be enumerated from the trail: DCR-0032's last amendment is titled "the fifteenth through
nineteenth divergences", and no document carries the running list that reaches fourteen before it.

### Background

`transync-html` was extracted from `transync-syntax::htmlseg` by DCR-0032 in August, verbatim at
the extraction. Since then the crate has deliberately diverged from what `htmlseg` did, one
tokenizer or tree-construction rule at a time, each change made because a browser disagreed with
the old behaviour and several of them closing routes that put an attacker-chosen `data-sync-id`
into a live DOM. The count matters operationally: `CLAUDE.md` uses it to tell a future reader that
a difference from `htmlseg` is intentional, so a maintainer who finds one and cannot place it in
the list has no way to decide whether it is a fix or a bug.

The divergences were recorded as they landed, in DCR-0032's own amendments plus DCR-0041 through
DCR-0043, DCR-0050 and DCR-0051. The earlier amendments describe theirs in prose and by ticket —
"three tokenizer divergences" for ti `549b20`, two walk and strip changes on 2026-08-23 — without
assigning ordinals. The 2026-09-05 amendment then assigns ordinals for the first time, numbering
DCR-0051's five as the fifteenth through nineteenth. Anyone trying to verify "nineteen" therefore
has one bracket and no list.

Found by the 2026-09-08 documentation drift audit, which took the minimal repair on the living side
only: `CLAUDE.md` now attributes the number to DCR-0032's running count instead of asserting it
independently, and stops citing `contracts.md` §4 as a place that records the tally — §4 records
browser-verified behaviour and §4b a browser-oracle census, neither of which is a divergence count.
The record-side gap was left alone deliberately, because DCR-0032 is a dated record and closing the
gap means choosing how.

It is type 2 because the three ways forward are mutually exclusive and none is obviously right.
Appending a dated numbered index to DCR-0032 preserves the body and makes the count checkable, but
it means reconstructing fourteen ordinals from prose written without them, and a reconstruction
could be wrong in a record whose whole value is precision. Dropping the number from `CLAUDE.md` and
describing the class instead ("every divergence the DCR-0032 amendment trail records") is honest
and costs nothing, but gives a reader no way to know whether they have found all of them. Recording
that the number is DCR-0032's internal bookkeeping rather than a verifiable count is the cheapest,
and leaves `CLAUDE.md` making a claim no one can check. Nothing is blocked — a person has to pick.

## publish-lock-nested-tree-boundary (ticket `40e2a5`)

**Settled by ticket `40e2a5` (closed 2026-08-10), with ti `cbbc4e` for both residuals;
`contracts.md` §6 states the outcome in the publication lock's *Scope* and *Depth*
bullets.** Verified 2026-10-06 in `crates/transync-cli/src/output/`: `claim_anchor` in
`fileset.rs`, and in `publish.rs` `published_dirs_inside`, the bounded re-read in
`lock_publication_tree`, and the second `ensure_out_dir_replaceable` pass made with the
staged tree in hand. Entry verbatim below.

> **RESOLVED.** Ticket `40e2a5` is closed/resolved, and `contracts.md` §6 records the outcome: the
> lock now covers a `--out-dir` publish that replaces a directory another run is publishing into,
> the two modes reach one directory further to meet each other, and ti `cbbc4e` settled how deep
> that goes and what bounds the gap it deliberately leaves under `--force`. Verified 2026-09-06.

> **reopen 2026-08-24: ticgit:40e2a5 is RESOLVED.** Verified with `ti show 40e2a5 --json`. Kept for audit; it is not open work and does not count against the v0.5.0 gate.

- **Type:** 2
- **Description:** DCR-0021's publication lock serializes two runs publishing *into* the
  same directory, but `--out-dir X` locks the directory that **holds** `X` (where the
  staging and backup siblings live) while a files-mode publish into `X`
  (`--output X/out.md --map X/alignment.json`) locks `X` itself — different inodes, so the
  nested pair never serializes, and the operator can end up with a directory mixing two
  runs. An inode-based lock cannot close it: after the `--out-dir` run swaps its staged
  tree in, a waiter holds a lock on a directory that is no longer at that path. The
  choice: (a) name-keyed leases in the parent directory so the two modes serialize, with
  a test covering the two-mode race, or (b) accept the boundary permanently and say so in
  `contracts.md` §6 beside the existing "Scope" bullet, naming the operator-side
  mitigation (do not aim two runs at one tree through different modes).
- **Background:** Filed 2026-08-06 by the agent verifying ticket `f41f16`
  (R0001-0034/0035/0036); DCR-0021 already records it under "Known boundary", so the
  decision is whether that stays the end state. It is the last `review-0001`-tagged
  ticket still open.
- **Landed 2026-08-10 (option a, by a different key):** the boundary is closed, not
  accepted. Neither mode's lock moved; each reaches one directory further, to where the
  other already is — a fileset commit also locks the deepest level that *exists* on the
  way to a destination it has to create (`claim_anchor`; for a destination that already
  exists that is the destination itself, so ordinary runs are unchanged), and an
  `--out-dir` publish also locks the target and its `html/` (`published_dirs_inside`).
  Name-keyed leases in the parent were the routed sketch and were declined for a reason
  stated in DCR-0021's appended note: they make every files-mode publication write a lock
  marker into the parent of its destination — `--output out.md` in `~/project` marking
  `~` — and fail where a parent is unwritable but the destination is not. Locking a
  target the swap renames away is sound because of ticket `8792b7`'s waiter-side
  revalidation, which lands with it. Residual, stated in `contracts.md` §6: a run
  publishing *deeper* inside a target than `html/`.
- **Review fix round, 2026-08-10:** the inner set (`published_dirs_inside`) was read
  before the lock was taken and never revalidated, so a replace that started before its
  target existed held nothing inside it even after the target appeared. It is now re-read
  under the lock and the whole set retaken until it stops changing
  (`lock_publication_tree`, bounded). A second, narrower residual is stated with the
  first: a directory that appears inside the target after that last re-read, put there by
  something the replace is not serialized with. Both are ticket `cbbc4e`.
- **Both residuals settled 2026-08-10 (ticket `cbbc4e`), by ordering rather than by a
  third claim:** no finite pair of claims meets at every depth — a replace claims a fixed,
  shallow set while a publication claims a point that can be arbitrarily deep — so none
  was added. Claiming upward has no principled root short of `/`; claiming downward is
  unbounded under `--force`, litters a tree the guard may then refuse, and is a TOCTOU;
  and "no lock file beneath the target" is not "no publisher beneath it" (`8792b7`). What
  bounds it instead: a publication reads its anchor *before* it creates anything, so a
  peer aiming under an `--out-dir` target claims the target or a level above it and waits;
  and `ensure_out_dir_replaceable` now runs a **second** time with the staged tree in
  hand, so a directory that appeared inside the target while the bundle was being written
  makes the replace refuse instead of renaming it away. Making that argument true also
  required tightening the guard's shape recognition — the bundle allow-list and both
  staging-temp recognizers matched on name alone, so a *directory* named `index.html` or
  `out.md.tmp.<pid>` was walked past the check and into a backup `remove_dir_all` that
  needed no `--force`. Accepted rather than open, and stated in `contracts.md` §6: the
  `--force` path, which waives the guard by request, and the few syscalls between the
  second guard pass and the rename.

## out-dir-foreign-staging-temp (ticket `66339b`)

**Settled by ticket `66339b` (closed 2026-08-10), decided in one change with OI-0036;
`contracts.md` §6 states the rule.** Verified 2026-10-06: `ensure_out_dir_replaceable` in
`crates/transync-cli/src/output/preflight.rs` passes over a top-level `<name>.tmp.<pid>`
staging temp only when it is a regular file. Entry verbatim below.

> **RESOLVED.** Ticket `66339b` is closed/resolved. `contracts.md` §6 records the answer: a
> `<name>.tmp.<pid>` staging leftover is transync's own residue and is recognized as such — as a
> regular file only, since a directory at that name is somebody else's subtree — so it no longer
> makes the whole target read as foreign. ADR-0029 (2026-09-06) settles the neighbouring question
> this entry raised about the ownership marker. Verified 2026-09-06.

> **reopen 2026-08-24: ticgit:66339b is RESOLVED.** Verified with `ti show 66339b --json`. Kept for audit; not open work.

- **Type:** 2
- **Description:** `ensure_out_dir_replaceable` admits only `out.md`, `alignment.json`,
  `validation-report.json`, `html` and the `.transync-publish.lock` marker at an
  `--out-dir` target's top level; any other name makes the target foreign and the publish
  exits 4 until `--force` is passed. A crashed files-mode run into that same directory
  leaves exactly such a name behind — `write_fileset_atomic` stages each payload as
  `<path>.tmp.<pid>` inside the destination — so the entry being refused can be
  transync's own residue. `scan_bundle_dir` and the nested `html/` scan already recognize
  that suffix (`tmp_pid_suffix`); the top-level loop does not. The choice: (a) leave it,
  which is what `contracts.md` §6 says today and what `docs/Troubleshooting.md` now tells
  the operator (`a5ffe50`), or (b) give the top-level loop the same tmp-pid recognition —
  one rule instead of two, and one fewer `--force` recommendation — at the cost of
  widening what an `--out-dir` publish will `remove_dir_all` without asking, which owes
  the same "is nested user data reachable?" argument the EXT-2026-07 review-fix made for
  `html/`.
- **Background:** Found 2026-08-07 by ticket `3b90c1` while deriving the `--force` rule
  from `output.rs`. The behavior matches `contracts.md` §6 as written, so this is a
  proposal, not a defect.
- **Landed 2026-08-10 (option b, with OI-0036's marker paying for it):** the top-level
  loop got the tmp-pid recognition, and the widening it owed is paid by the second
  question the guard now asks — the `.transync-out-dir` ownership marker (or the complete
  published set) — so tolerating transync's own residue never widens what gets replaced
  in a directory transync did not publish. Decided in one change with OI-0036 directly
  below, as both entries required.

## out-dir-sparse-bundle-subset (OI-0036)

**Settled by OI-0036 (RESOLVED, `docs/project/open-issues-archive.md`), landed with ticket
`66339b`; ADR-0029 records why the marker's body is not part of the test.** Verified
2026-10-06: `publish_out_dir` stages the `.transync-out-dir` marker into every published
tree, and `ensure_out_dir_replaceable` passes a marker-less target only on the complete
published set. Entry verbatim below.

> **RESOLVED.** OI-0036 lives in `docs/project/open-issues-archive.md`. A target carrying no
> ownership marker is judged by the COMPLETE published set being present, so a user directory whose
> only entry happened to be called `out.md` no longer qualifies for replacement without `--force`.
> ADR-0029 (2026-09-06) records why the marker's *body* is deliberately not part of that test.
> Verified 2026-09-06.

> **reopen 2026-08-24: OI-0036 lives in `open-issues-archive.md`,** i.e. resolved and archived by `indy-review-prune`. Kept for audit; not open work.

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0028), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues-archive.md#OI-0036`
- **First seen:** 2026-08-08
- **Last seen:** 2026-08-08
- **Unfiled:** resolved and retained for the record; no ticket is owed

### Description

`ensure_out_dir_replaceable` treats a directory as a transync bundle — and so
replaceable without `--force` — when its entries are a **subset** of the allow-listed
bundle names. A user directory that happens to contain only `out.md` therefore
qualifies: it is moved aside and its backup recursively removed, with no `--force` and
no prompt. The operator did aim `--out-dir` at that directory, which is what keeps this
below the destructive findings the same review produced, but "holds one allow-listed
name" is a weaker test than "is a bundle this tool produced", and the guard reads as
though it were the stronger one.

### Background

Low severity at HEAD (the reviewer filed it Medium). It is deliberately parked beside
`out-dir-foreign-staging-temp` (`66339b`) directly above: both are the same question —
how strictly "is this my bundle?" should be answered — approached from opposite
directions, in the same function. Deciding them apart risks two inconsistent answers.
Candidate shapes: require a marker file written by publication; require the full bundle
set rather than a subset; or require at least one *distinctive* member
(`alignment.json` + `html/`).

**Landed 2026-08-10** (first candidate shape, with the second as its fallback), in one
change with `66339b` as both entries required. `publish_out_dir` stages a
`.transync-out-dir` ownership marker into every tree it publishes, and the guard asks
"is anything in here not transync's?" and "did transync publish this?" separately; a
target with no marker still passes on the **complete** published set, so bundles written
before the marker existed keep republishing. `docs/project/open-issues-archive.md#OI-0036` is
RESOLVED.

## sync-anchor-injection-via-raw-html (OI-0035)

**Settled by OI-0035 (RESOLVED 2026-08-23, `docs/project/open-issues-archive.md`) through
DCR-0033, route (c); the accepted impostor residual lives in DCR-0033's *The residual,
stated plainly*.** Verified 2026-10-06: `transync_html::strip_reserved_sync_attrs` strips
the reserved namespace, and `web/js/sync.js` collects each pane's anchors gated by the
validated rows' ids. Entry verbatim below.

> **RESOLVED.** OI-0035 lives in `docs/project/open-issues-archive.md`. `strip_reserved_sync_attrs`
> removes the whole reserved namespace from the block's own bytes before the wrapper is written, so
> the sync attributes a pane carries are a construction rather than a scan; DCR-0041's shared
> `walk_attrs` and ti `415cdb`/`e20490`/`2e2453` closed the four measured bypasses since. The
> browser oracle now measures the property directly: `reserved:dom-only` fell from 1 to 0 per
> 10,000 generated inputs at DCR-0051. Verified 2026-09-06.

> **reopen 2026-08-24: OI-0035 was RESOLVED 2026-08-23 and archived** (route (c), DCR-0033). Verified: 0 hits in `open-issues.md`, 1 in `open-issues-archive.md`. Kept for audit; not open work.

- **Type:** 2
- **Verified:** yes — Review 0002 finding (R0002-0018), gate-accepted, user-routed track
- **Sources:** `docs/project/open-issues-archive.md#OI-0035` (RESOLVED 2026-08-23, archived — was `open-issues.md` until then)
- **Accepted residual (ti `490d97` wave 7, DCR-0039):** recorded in DCR-0033 and not
  restated here beyond the one fact the note above omits — an impostor carrying a
  *listed* id ahead of the genuine anchor still defeats the engine layer alone; the
  render strip is what closes it for panes transync produces. Wave 7 corrected the
  Type-2 preamble above, which had gone on framing this as an open design question.
- **First seen:** 2026-08-08
- **Last seen:** 2026-08-08
- **Unfiled:** resolved and retained for the record; no ticket is owed

### Description

The browser sync engine builds its anchor sets by collecting **every** `data-sync-id` in
each pane's DOM; alignment rows validate and warn but are not the source of the anchor
set, so map membership does not gate what can become a scroll driver or target. Source
Markdown is untrusted (architectural invariant 7) and raw HTML blocks are translatable,
structurally-owned content that reaches the rendered pane — so a `data-sync-id` written
into a source document survives the default DOMPurify configuration and can **pre-claim
a real block's id**.

### Background

Low severity at HEAD; DOMPurify still bounds what markup renders, so the failure is
steering, not execution. Half of the reviewer's finding is the recorded `d3acc3` /
OI-0015 ID-identity decision and is not in question — this entry is only the unrecorded
half. It needs a decision before code because the fix could live at the sanitizer
(strip or namespace `data-*`), at the engine (build anchors from validated rows), or at
both; and whichever is chosen must move `web/js/sync.js` and its byte-identical embedded
CLI twin in one commit. *(2026-08-09: `d3acc3` landed and left all three routes open —
the anchor sets still come from `querySelectorAll("[data-sync-id]")`. It did make the
engine route cheaper: reflow recompute re-collects anchors through the same
`collectAnchors`, which is now the single place every anchor set passes through, at mount
and on every reflow, and it already takes a per-call policy argument.)*

## warm-cache-run-needs-no-credentials (OI-0038)

**Settled by DCR-0046 and ticket `30a744` (closed 2026-09-02); OI-0038 is RESOLVED in
`docs/project/open-issues-archive.md`.** Verified 2026-10-06: `transync translate
--offline` exists in `crates/transync-cli/src/translate_cmd.rs` and is refused without
`--cache-dir`. Entry verbatim below.

> **RESOLVED 2026-09-02** (`cdc1e32`, DCR-0046, ticket `30a744`), per
> `docs/project/open-issues.md#OI-0038`. The product question this entry existed to ask is
> answered — a fully-warm run offline is supported — behind an explicit `--offline` flag rather
> than by deferring translator construction unconditionally. Verified 2026-09-06.

- **Type:** 2
- **Verified:** yes — Review 0004 finding (R0004-0069), gate-accepted, user-routed track
- **Sources:** ticgit:30a744f1, `docs/project/open-issues.md#OI-0038`
- **First seen:** 2026-08-13
- **Last seen:** 2026-08-13

### Description

The pipeline constructs its `Translator` — and so demands an API key — before
the cache is consulted, so a run in which every unit is a cache hit and zero
provider calls would be made still cannot start without credentials. DCR-0028's
own tests prove such a run exists (they assert zero dispatches on a second
identical run); what no record says is whether that run is meant to be possible
**offline**.

### Background

Type 2, not 1: the product question comes first and decides the fix's shape —
is an offline run against a fully warm cache a supported scenario? Only then
does the mechanism follow (lazy `Translator` construction, or resolution at
first dispatch), and with it a semantics that does not exist today: what a run
that started without credentials does when it takes a cache **miss**. That
belongs with ADR-0009's retry/fallback contract rather than being invented at
the call site. Low severity — the failure is a clean refusal to start, not
wrong output — but it withholds the disk cache's most valuable property
(a document is paid for once) exactly where it is most wanted.

## docs-index-orphaned-trees (ticket `1347b4`) — **RESOLVED 2026-08-10**

**Settled by ticket `1347b4` (closed 2026-08-10, option (a)).** Verified 2026-10-06:
`crates/transync/tests/docs_index_drift.rs` requires every Markdown document under `docs/`,
reads its git-ignored exclusions out of `.gitignore`, and carries the two companion tests
the entry names; `docs/index.md` and `docs/architecture/README.md`'s *Where to look next*
both link `backlog.md`. Entry verbatim below.

- **Type:** 2
- **Description:** `docs/index.md` calls itself the "single entry point for everything
  under `docs/`" and tells its maintainer to link every new doc "so nothing becomes
  orphaned", but one tracked non-Korean doc was absent from it: this file. (The
  fourteen-file `docs/investigation/` bundle was the rest of the gap until 2026-08-08,
  when the owner untracked it — an untracked bundle is not the index's to carry, so only
  this file remained.) It was not linked from `docs/architecture/README.md` either, so it
  was unreachable from the entry point.
  `crates/transync/tests/docs_index_drift.rs` passed because it enforced only
  `docs/decisions/*.md` and the DCRs, while its module doc restated the header's broader
  claim. The choice: (a) index them and widen the drift test to the header's rule, or
  (b) narrow the header to the trees the index actually covers and align the test's own
  doc — not both.
- **Resolution (2026-08-10):** the owner routed it to **(a)**. This file and
  `docs/project/git-history-loss-2026-08-10.md` are linked from `docs/index.md`, this
  file also from `docs/architecture/README.md`'s "Where to look next", and the drift test
  now requires **every** Markdown document under `docs/` at any depth. Its exclusions are
  the ones the index states to a reader — `*.ko.md` siblings, git-ignored bundles under
  `docs/`, `index.md` itself, non-Markdown files — and the git-ignored set is read out of
  `.gitignore` instead of hardcoded, so re-tracking `docs/investigation/` requires its
  files again in the same edit. Two tests were added beside the widened one: one asserting
  the required set reaches past the two record trees, one asserting the stated exclusions
  are the only exclusions.
- **Background:** Found 2026-08-07 while refreshing this file (`9c709cc`), checking that
  the refreshed index of open items is reachable from the documentation entry point. It
  is a judgment call about what the index is for, which is why it is a decision rather
  than a one-line edit.
- **Unfiled:** resolved and retained for the record; no ticket is owed

## phrasing-custom-element-mid-sentence-review

**Settled by ticket `84bf37` (closed): the owner accepted the current behaviour on
2026-09-04.** Verified 2026-10-06: `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
carries the ruling as inline errata dated 2026-09-04 under ti `84bf37` at §4, §13 limitation
5 and §14 item 1, and the untaken lever is the entry
`phrasing-extension-list-for-custom-elements` in `docs/backlog.md`. Entry verbatim below.

> **RESOLVED 2026-09-04 (`ticgit:84bf37`) — the owner ACCEPTED the current behaviour.** The re-confirmation ran on the shipped intake over 42 real pages (10.1 MB, 8,879 blocks, 346 custom-element start tags) across two disjoint corpora, the second deliberately covering the generator classes the first missed (Sphinx, MkDocs, Docusaurus, Jekyll, Hugo, GitBook, WordPress, Ghost, Notion, Confluence, gov.uk, MediaWiki, two production email templates): **zero mid-sentence splits.** The measurement also narrowed the hazard — the split fires only in prose **not wrapped in an element**; inside `<p>`, a heading or an `<li>` the wrapping element is the block and the custom element is interior. Twelve errata landed in the spec recording that, including in §4's PHRASING bullet and §13 limitation 5, which both stated the split unconditionally. The untaken lever now lives at `phrasing-extension-list-for-custom-elements`, deferred at filing.

- **Type:** 2
- **Description:** A custom element mid-sentence **stops** segmentation (D4's DEFAULT-STOP
  default), so one sentence can cross three translation units. Accepted *for now* with
  the PHRASING widening — and the owner **explicitly required re-confirmation against
  real output**. Candidate lever if the review rejects it: a per-run or per-profile
  phrasing-extension list, designed then, not now.
- **Measured 2026-09-04 (ti `490d97` wave 7), and it is NARROWER than §14.1 states.**
  Three shapes through the shipped `intake::html`, captured in
  `/Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/`:
  - **Wrapped in `<p>` — no split.** `<p>The Pro plan costs <price-tag>29</price-tag>
    per month.</p>` is **one** `paragraph` block: the `<p>` is the element block and the
    custom element lives inside its extent. Three such sentences yielded
    `paragraph paragraph paragraph`, not nine blocks.
  - **Naked prose — the split fires.** The same sentence directly inside a `<div>`
    yields `p-0002 html-0003 p-0004`: run, stopped element, run. This is the §14.1
    hazard, reproduced.
  - **Known phrasing is absorbed either way.** `<em>` keeps its run whole.
  So the hazard is confined to prose that is **not wrapped in an element** — which real
  pages rarely are. §14.1's own example (`Price: <my-price/> today`) is written in the
  naked form without saying that the form is what makes it split.
- **Background:** spec `2026-08-20-html-to-html-translation-design.md` §14.1 and §13.5;
  ADR-0025 (D4). Wave 7 staged the evidence and filed the ticket; the ruling is the
  owner's and is not wave 7's to take.
- **Sources:** ticgit:84bf37 (filed 2026-09-04 with the measurement above and three
  options: amend §14.1 to state the wrapping condition, add a phrasing-extension list,
  or leave it).

## Type 2 — Needs decision: removal note _(docs/backlog.md, entries resolved since the 2026-08-05 sweep)_

**Section prose, not an entry.** A list of Type 2 entries already removed from
`docs/backlog.md`, each pointing at the record that carries it; every ticket it names is
closed and every OI it names is RESOLVED in `docs/project/open-issues-archive.md`
(re-checked 2026-10-06). Text verbatim below.

_Resolved since the 2026-08-05 sweep, and removed from this section (each carries its
record elsewhere; a bare six-hex id below is a TicGit ticket, a seven-hex one a commit):
`oi-0015` decided and commissioned as ticket `d3acc3` (single-file re-affirmed,
ID-identity to become normative, reflow hooks in the same pass) — that pass **ran
2026-08-09**, so `open-issues-archive.md` now carries OI-0015 (and OI-0024) as RESOLVED;
`oi-0018` **RESOLVED 2026-08-07**
as the opt-in `--strict-csp` flag (ticket `14307f`), default posture unchanged, and
`261e2c5` then corrected the three places that described the flag as confining a bundle
to its folder — CSP `'self'` scopes to the serving **origin**, so the guarantee is a
remote-load block, not a sandbox around the directory; `oi-0020` **RESOLVED 2026-08-06**
— `Cargo.lock` is committed (`6cf4164`, a repo-local `!Cargo.lock` un-ignore) and the
`wasm-bindgen = "=0.2.126"` pin is kept alongside it, while the `CLAUDE.md` half of the
same machine-global-gitignore concern was explicitly **declined**, so `CLAUDE.md` — not
the lockfile — stays untracked by owner choice; its entry still read OPEN until ticket
`d0cd98` corrected the record (`f3f52ac`); `oi-0034` and its payload-side residual
resolved (normalize at intake, ticket `743d27`; fail-the-unit for payload NUL, ticket
`d06c43` — `out.md` never contains a NUL, contracts.md §3);
`default-profile-korean-glossary` resolved — examples commented out (ticket `e7ae21`);
`cache-identity-batch-cohort` resolved — prompt-relevant identity, `CacheKey` gains
`instruction_hash` (ticket `c02f69`, 0.3.0 window); `next-release-cut` decided — v0.3.0
after the waves and smoke (owner, 2026-08-06), cut at `30d467c`;
`untracked-analysis-bundles` settled 2026-08-08 (owner) the other way: the
`docs/investigation/` bundle is untracked again, reversing the 2026-08-06 decision that
committed it in `727d3bc`, because it is regenerated from the code rather than authored,
so each regeneration produced a diff carrying no decision — the files stay on disk.
`reviews/` was not reversed with it: round 0001, its dispositions, its patch and
`README.md` stay committed as the dated record they were made, and a later round joins
them when the gate archives it;
`wasm-demo-schema-triple-mirror` decided 2026-08-06 under owner delegation — the
test-pinned mirrors ARE the end state (each of the three JS copies is pinned by a cargo
test; a build-time injection was declined because it would put a build step in front of
the deliberately no-build `web/` demos); the doc-hidden `build_batches` gate shipped
(ticket `0ed6eb`, commits `296ce67` + `721ba93`)._

## validation-report-schema-bump-for-document-level-entry

**Settled by ticket `52109b22` (closed 2026-09-03, commit `d136b0d`): no bump, `1.1.0`
stands.** Verified 2026-10-06: `VALIDATION_REPORT_SCHEMA_VERSION` in
`crates/transync-core/src/validate.rs` is `1.1.0`, and its rustdoc and `contracts.md` §3a
both state that the version covers keys, structured row shapes and the ordering of a
structured list, never the wording inside a note. Entry verbatim below.

> **RESOLVED.** Ticket `52109b22` is closed/resolved — the decision this entry was waiting on has
> been taken. Verified 2026-09-06.

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:52109b22
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`ValidationReport::skipped_source_nodes` now carries a document-level entry alongside the per-block notes it carried before, which changes the ordering contract `contracts.md` §3a states — and `VALIDATION_REPORT_SCHEMA_VERSION` stayed at `1.1.0`.

### Background

The precedent for the 1.0.0 → 1.1.0 bump (DCR-0026) was exactly 'a new row shape a 1.0.0 consumer never met'. This is a new entry shape by the same description, so either the precedent applies and the constant owes a bump, or the precedent is narrower than it reads and the difference should be written down. A consumer that iterates the array positionally is the one that would break. Type 2 because it is a versioning decision with a real cost either way: bumping obliges consumers, and not bumping weakens what the version number promises. Nobody can start until that is chosen.

## review-round-registry-number-collision

**Settled by ticket `bdf8d981` (closed 2026-09-03, commit `ba06db6`): a bare id means the
2026-08 gated round, and the 2026-05 rounds are the marked side.** Verified 2026-10-06:
rule 4 of the tracked `reviews/README.md` states it, with the per-number table. Entry
verbatim below.

> **RESOLVED.** Ticket `bdf8d981` is closed/resolved — the owner decision this entry was waiting on
> has been taken, and `reviews/README.md` is the resolver of record. Verified 2026-09-06.

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:bdf8d981
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`reviews/README.md` is the resolver for bare finding ids and states that rounds 0002 and up never collide. Three gated 2026-08 rounds reused 0002, 0003 and 0004, and 508 live citations depend on the rule that is now false.

### Background

The registry's rule 4 says verbatim that each number was used once, so `R0002-…` through `R0008-…` are unambiguous bare and need no marker. That is what makes a bare citation resolvable. With three numbers reused, a bare `R0002-0047` has two possible referents and a reader has no way to tell which. The collision was observed live on 2026-08-24, when `reviews/0002.md` existed again as a fresh `STATUS: CLAIMED` stub; that transient file is gone today, but the reused numbers and the citations that depend on them are not. Type 2 and tagged owner-decision because the repair is a choice between renumbering the newer rounds, qualifying every affected citation, or amending the rule and accepting ambiguity — each with a different cost across 508 citations.

## wrapper-ruling-leaves-named-default-stop-elements-exposed

**Settled by ticket `d4bce239` (closed 2026-09-03) and implemented by ti `490d97` wave 6
(DCR-0038): the self-injection boundary is the block's kind.** Verified 2026-10-06:
`contracts.md` §4 states the kind key, and `crates/transync-syntax/src/render/html_pane.rs`
self-injects only when the row's `block_kind` is not `html`. Entry verbatim below.

> **RESOLVED.** Ticket `d4bce239` is closed/resolved. `contracts.md` §4 records the answer: the
> transparent wrapper key is the block's KIND, never the sanitizer's allowlist, because duplicating
> DOMPurify's allowlist in Rust would be a second opinion about what the sanitizer accepts — and it
> was the earlier "is the element named in §4's tables" key that left `iframe` and `noscript`
> self-injecting. Kind-keying needs no list at all. Verified 2026-09-06.

- **Type:** 2
- **Verified:** yes — gated finding register
- **Sources:** ticgit:d4bce239
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

The owner's 2026-08-21 wrapper ruling closed the custom-element anchor hole, but it is keyed on elements unknown to spec §4's tables — and DOMPurify also removes some elements those tables do name.

### Background

The ruling fixed a measured defect: a block whose outermost element is unknown to the tables now takes a transparent `<div{attrs}>` wrapper instead of having the anchor injected into its own open tag, because the sanitizer was removing the element and the anchor with it. Verified against the vendored purify 3.2.6. The residue is that 'unknown to our tables' and 'removed by the sanitizer' are different sets, and the ruling covers only the first. A named DEFAULT-STOP element the sanitizer strips still loses its anchor. Type 2 and owner-decision because widening the rule means either deriving it from the sanitizer's behaviour — which the project has elsewhere refused, on the ground that a second opinion about HTML is how anchors drift — or enumerating a second list that must be maintained against a vendored dependency.

## html-to-html-document-translation-epic

**Settled by ticket `490d9712` (closed 2026-09-04 on owner acceptance of the eight-wave
delivery); records DCR-0032 through DCR-0039 and ADR-0025.** Verified 2026-10-06:
`--input-format html`, the layer-6 twin `validate::full_rescan_html` and
`ALIGNMENT_SCHEMA_VERSION` `1.3.0` are in the tree, and the edges the feature left open each got a backlog
entry — `phrasing-extension-list-for-custom-elements` (the lever
`phrasing-custom-element-mid-sentence-review` left), `markdown-island-reclassification` and
`html-oversize-leaf-block-split`, still in `docs/backlog.md`, and `parser-intake-markdown-rename`,
resolved 2026-09-07 (DCR-0053) and kept in this archive. Entry verbatim below.

- **Type:** 2
- **Verified:** yes — owner-commissioned roadmap ticket
- **Sources:** ticgit:490d9712
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

The HTML→HTML feature: a second intake producing the same block IR, delivered as eight waves. **COMPLETE 2026-09-04** — all eight waves (0–7) landed, recorded in DCR-0032 through DCR-0039.

### Background

This was the umbrella the HTML work ran under, recorded here because a backlog that omits the largest open item in the tree is not a census. **All eight waves landed** — DCR-0032 (wave 0) through DCR-0039 (wave 7, 2026-09-04) — and the executed order was 0 → 2 → 3 → 4 → 5 → 6 → 7, corrected from the spec's original graph by wave 4's deviation 1. Retained for the record; it is no longer open work and does not count against the v0.5.0 gate. What the feature deliberately left open has its own entries: `phrasing-custom-element-mid-sentence-review` (the owner's §14.1 re-confirmation, `ticgit:84bf37`), `markdown-island-reclassification`, `html-oversize-leaf-block-split` and `parser-intake-markdown-rename`.

## git-history-lost-twice-standing-record

**Settled by ticket `6b43008a` (closed 2026-09-02) on the owner's 2026-09-01 decision that
the authoritative `.git` is GitHub, recorded in `docs/project/git-history-loss-2026-08-17.md`
(commit `1173a02`).** The standing record of both losses is the two
`git-history-loss-*.md` documents. Verified 2026-10-06 against that record's answered
question. Entry verbatim below.

> **RESOLVED 2026-09-02** (ticket `6b43008a`), and the 2026-09-06 note below was already wrong when
> it was written. The owner decision it calls untaken was received **2026-09-01** — "the
> authoritative `.git` is GitHub" — and is recorded in
> `docs/project/git-history-loss-2026-08-17.md` (commit `1173a02`); the ticket closed once
> `origin/main` held every local commit. The standing record of the two losses lives in the two
> `git-history-loss-*.md` documents, which is where a reader should go. Retained here for audit;
> **not open work**, and not the "one open Type 2 item that needs a person".


> **2026-09-06 — facts refreshed; the decision is still the owner's and is deliberately not taken
> here.** `git rev-list --count HEAD` is now **135** (was 48), `git tag` prints **`v0.4.0` and
> `v0.5.0`** (was `v0.4.0` alone), `git fsck` reports only dangling objects, and the root commit is
> still `59ce8df7`, the 2026-08-17 restart. So the "exactly one commit, no tags" symptom is further
> from reproducing than ever, and the loss is no less permanent. What this entry asks — whether the
> ticket is a defect that should close or the standing record that this object store has lost
> history twice, and that `/Volumes/Common` is therefore not reliable storage, in which case it
> should stay open and be re-titled — is unchanged, and is the one open Type 2 item that needs a
> person rather than a maintainer. Left for the owner rather than decided in passing.

- **Type:** 2
- **Verified:** partly — reopen verified 2026-08-24: 48 commits and a v0.4.0 tag now exist, so the stated symptom is stale; the loss itself is permanent
- **Sources:** ticgit:6b43008a
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

The pre-restart history is unreachable and always will be. The ticket's stated symptom — exactly one commit, no tags — has been overtaken by subsequent work.

### Background

Verified: `git rev-list --count HEAD` is 48, `git tag` prints `v0.4.0`, `git fsck` is clean, and the root commit is `59ce8df723b4`, the 2026-08-17 restart rather than the original root. So half the claim stands permanently and half no longer reproduces. `docs/project/git-history-loss-2026-08-17.md` and its 2026-08-10 sibling record both events. Type 2 rather than type 1 because there is nothing to implement: what remains is a decision about what this ticket is *for*. Read as a defect it is unreproducible and should close; read as the standing record that this object store has now lost history twice — and that `/Volumes/Common` is therefore not reliable storage — it should stay open and be re-titled. That is an owner call, not a maintenance one.

## js-lint-gate-exit-status-is-dishonest (OI-0039)

**Settled by OI-0039 (RESOLVED, `docs/project/open-issues-archive.md`) — format half
2026-09-04 under DCR-0049, lint half 2026-09-15 under DCR-0056.** Verified 2026-10-06:
`biome.jsonc` turns the formatter and the linter on, and `scripts/hooks/pre-commit` runs
`biome format` and `biome lint --error-on-warnings`. Entry verbatim below.

> **RESOLVED 2026-09-04 — format half adopted, lint half deferred with a re-trigger.** `biome.jsonc` at the repo root (formatter on, linter and assist **off**, `@biomejs/biome` pinned exactly at 2.5.12 because formatter output is not semver-covered); 457 lines reformatted across 10 files; the hook's JS leg rewritten to a `git ls-files` corpus invoked **from the repo root**, so it reaches the CLI `sync.js` twin and `profile.mjs` — the 22% the old `cd web` scope structurally could not see — and an empty file list is now an explicit FAIL. Two things landed first as fixes rather than decisions: the printed remediation no longer instructs an action that makes the next commit fail with 180 errors, and a tool **declared** in `web/package.json` but not installed now fails instead of skipping. Taken now because the deferral's own re-trigger — "the first commit that already moves both `sync.js` copies together" — is OI-0047's fix, so deferring would have re-triggered immediately. **Lint half ADOPTED 2026-09-15 (DCR-0056)**, on the owner's decision rather than on the recorded re-trigger ("a JS defect reaching a shipped bundle that a lint rule would have caught"), which never fired: the linter is on, the hook runs `biome lint --error-on-warnings` because biome exits 0 on warnings, and all 36 current diagnostics are fixed — including the 30 guard-shape rewrites in `sync.js`/`wasm-demo.js`, verified by the 47-test headless browser suite. **Does not count against the v0.5.0 gate.**

- **Type:** 2
- **Verified:** yes — Review 0009 finding (R0009-0014), gate-accepted,
  user-routed **fix**; the visibility half landed in `88964df` and the
  exit-status half was deliberately refused there as a decision above a fix
  route
- **Sources:** docs/project/open-issues.md#OI-0039, reviews/reviewed/0009.md#R0009-0014
- **Unfiled:** none owed — OI-0039 is RESOLVED 2026-09-04; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

`scripts/hooks/pre-commit` — the only hook copy, the one `core.hooksPath`
points at — runs a JavaScript/TypeScript leg over `web/` whose `run_js_tool`
helper prints a SKIP line and `return 0` when the tool binary is absent.
`web/node_modules/.bin/` holds exactly one entry, `playwright`;
`web/package.json`'s only devDependency is `@playwright/test`; and there is no
`web/tsconfig.json`. **Every probe therefore misses**, and the leg's
contribution to the exit status is always zero. Measured by running the hook
against a scratch tree mirroring `web/` with no `Cargo.toml`:
`SKIP: prettier not installed in web`, `SKIP: eslint not installed in web`,
`HOOK_EXIT=0`. The consequence is that `web/js/sync.js` — 52,996 bytes, the
sync engine — plus `web/js/wasm-demo.js`, `web/playwright.config.js` and the
three spec files ship with zero format or lint coverage, and the hook is the
only automatic gate this repository has.

### Background

Type 2, and this is the whole point of the entry: the fixing agent did the
visibility half and **refused the substantive half on the record**. The hook now
ends with a framed block reading `THE JAVASCRIPT/TYPESCRIPT GATE DID NOT RUN.`,
naming the missing tools and stating that "a successful exit below covers the
Rust gates ONLY". The status is still 0. Making it honest requires choosing
between two routes, each with a named cost. **(a) Adopt and pin a linter** — the
hook already has a biome-first branch, so `pnpm add -D @biomejs/biome` alone
makes it fire, but the price is conforming roughly 190 KB of existing JavaScript
including `sync.js`, whose byte-identical twin at `crates/transync-cli/web/sync.js`
is pinned by `crates/transync-cli/tests/sync_js_drift.rs` — any reformat is a
two-file commit or that test goes red. **(b) Fail only on staged JavaScript** —
narrower, but it blocks work in progress, stopping the first developer to touch
a JS file mid-feature. Verification also narrowed the finding itself: the
reviewer's "as if the language gate ran" is wrong (two SKIP lines with the exact
remediation have always printed, and skip-with-notice is DCR-0018's recorded
design, which both 2026-08-20 wave plans call "normal output here, not a
failure"), and there is no correctness exposure because Playwright covers
browser behaviour. Do **not** close this by improving the message again — the
message is already as loud as a message can be; what is unresolved is the
status.

## sync-engine-dead-zone-and-quiet-reflow (OI-0047)

**Settled by OI-0047 (RESOLVED 2026-09-04, `docs/project/open-issues-archive.md`).**
Verified 2026-10-06: `web/js/sync.js` clamps both ends of the active-block search and
re-runs `warnMapDomDrift` and `warnOffsetParentDrift`, latched, on every reflow recompute;
its CLI twin is byte-identical; and `contracts.md` §4a states the per-recompute-frame
budget. Entry verbatim below.

> **RESOLVED 2026-09-04.** The bottom clamp landed and the two drift diagnostics now re-run on every coalesced reflow recompute, **latched** so a warning fires only when the verdict changes; the refresh-only alternative was rejected because a diagnostic that fires only on an explicit call is one nobody sees. Both `sync.js` copies moved in one commit. `contracts.md` §4a is amended for the consequence: its "one property read per pane at mount, never per frame" budget is now one read per pane **per recompute frame**, and that widening is the contract.

- **Type:** 2
- **Verified:** yes — Review 0009 findings (R0009-0021, R0009-0025),
  gate-accepted, user-routed track
- **Sources:** docs/project/open-issues.md#OI-0047, reviews/reviewed/0009.md#R0009-0021,
  reviews/reviewed/0009.md#R0009-0025
- **Unfiled:** none owed — OI-0047 is RESOLVED 2026-09-04; this entry is retained for the record
- **First seen:** 2026-08-26 · **Last seen:** 2026-08-26

### Description

Two findings in `web/js/sync.js`. `activeBlockWithProgress` returns `null` once
the reference line (`scrollTop + REFERENCE_OFFSET_PX`, with the constant at 4)
passes the last anchor's bottom, and `handleScroll` bails on `null` — so the
follower pane freezes. Measured against a stub DOM with three anchors ending at
y=900 and 1200px of unanchored trailing content: `scrollTop=896` gives
`{"id":"p-3","progress":1}`, `scrollTop=897` gives `null`, and so does
`scrollTop=1500`. Separately, `recompute` calls
`collectAnchors(pane, label, rowIds, true)` — quiet — and calls neither
`warnMapDomDrift` nor `warnOffsetParentDrift`: after mutating a mounted pane to
hold a duplicate id, an unlisted id and a block whose `offsetParent` is a
`<figure>`, `controller.refresh()` emitted **0** warnings where a fresh mount of
the same DOM emitted **3**.

### Background

Type 2: the clamp is settled, but the diagnostics question is not.
`contracts.md` §4a states a mount-time budget — "One property read per pane at
mount, never per frame" — so re-running the probes on every coalesced reflow
frame changes a documented budget rather than merely adding a call; the
candidate shapes are re-running only on the explicit `controller.refresh()`, or
re-running and latching so a warning fires only when the verdict changes.
Verification narrowed both members and corrected the review's paths. Neither
shipped shell can scroll into the dead zone: it needs more than a viewport of
unanchored content **inside the scroll box**, and shipped panes carry only the
sanitized `<main>` plus 12px of padding while the sole non-anchoring kinds are
`ThematicBreak` and `Title` — a third-party pane with a tall in-pane footer can.
Half the review's recommendation is already implemented (a viewport above the
first anchor already returns it at progress 0), so what is missing is the
symmetric bottom clamp, about three lines. On the quiet reflow, what is lost is
**reporting, not behaviour** — the `rowIds` gate is not quiet, so an unlisted
anchor stays inert and a duplicate keeps its first occurrence — and part of the
silence is deliberate and recorded: §4a says the skip warnings are "silent on
reflow" by design. What no record covers is that `warnMapDomDrift` and
`warnOffsetParentDrift` never re-run at all. The review's third path,
`crates/transync-wasm/demo/sync.js`, does not exist; the byte-identical twin is
`crates/transync-cli/web/sync.js`, pinned by
`crates/transync-cli/tests/sync_js_drift.rs`, so either fix is a two-file commit.

## docs/backlog.md Type 3 section note — the 2026-08-07 refresh _(corrected 2026-08-16)_

**Section note, not an entry; moved from `docs/backlog.md` § Type 3 — Blocked, which it opened.**
Both of its live claims were overtaken: `oi-0017-detected-language-caching` is archived below, and
`provider-capability-set` was moved into the Type 2 section on 2026-09-11 and filed as ti
`6fb6dab2`, which is where its open decision lives. Note verbatim below.

Every blocking reason below was re-checked against the code and the arc's commits at the
2026-08-07 refresh: **none was removed then**, so nothing moved out of Type 3 at that
sweep — three have since (the notes below), and
`oi-0017-detected-language-caching` is a fourth: its named unlock, ticket `f12b8b`, landed
2026-08-09 as DCR-0028, so that entry is marked RESOLVED in place below rather than left
carrying a gate that no longer holds. `provider-capability-set` is a fifth, and the
one to read carefully: its stated condition has **fully fired**, on both limbs —
`fc0304` / DCR-0026 shipped the oversize split on 2026-08-09, and `bda471` / DCR-0029
landed a second in-tree provider on 2026-08-10 — so the entry below is marked
**awake** rather than blocked, and it has no blocking reason left to state. Awake is not
resolved: what the two landings produced is the first real evidence, and the entry
records that the evidence points both ways. With no block left it is a Type-2 item
(needs a decision), marked as such in place rather than moved, per this file's
convention for an entry whose block lifts. _(Corrected 2026-08-16: this paragraph
previously read "half its stated condition fired … the remaining half (`fc0304`) is
still outstanding", which was wrong when it was written — DCR-0026 had been on disk
since 2026-08-09, a day before the `bda471` landing it was reacting to.)_

## translator-deadline-cancellation _(removal note, 2026-08-08)_

**Settled by DCR-0024 (ticket `43331a`), which supersedes DCR-0009's YAGNI entry; moved from
`docs/backlog.md` § Type 3 — Blocked, where this note had already replaced the entry.** Verified in
code: both `Translator` methods take `cancel: &CancellationToken`, and `contracts.md` §5b records
run cancellation. Note verbatim below.

_Removed 2026-08-08: **`translator-deadline-cancellation`** — the one entry whose block did
lift. Its stated condition ("a long-running service consumer appearing") fired when
`dynwebserver` shipped as a tokio daemon holding transync behind an HTTP endpoint, so the
deferral was re-decided rather than left to expire: ticket `43331a`, **DCR-0024**, which
supersedes DCR-0009's YAGNI entry. This is what a named revisit condition is for, and it is
the shape the remaining Type-3 entries should be read against — their conditions are still
unmet._

## glossary-section-scope _(removal note, 2026-08-09)_

**Settled by DCR-0027 (tickets `43cfb4` and `0ed6eb`); moved from `docs/backlog.md` § Type 3 —
Blocked, where this note had already replaced the entry.** Verified in code: `ProfileError` has no
`Unsupported` variant, `unit::build_batches` returns `Vec<TranslationBatch>` rather than a `Result`,
and a `scope = "section"` entry is resolved through its `sections` selector. Note verbatim below.

_Removed 2026-08-09: **`glossary-section-scope`** — `[[glossary]].scope = "section"` was
rejected with `ProfileError::Unsupported` at three gates (the loader, the translate
boundary, and the `#[doc(hidden)]` `unit::build_batches` entry point, which was fallible
for that reason alone — ticket `0ed6eb`). Its stated block was section-aware batching, and
ticket `43cfb4` / **DCR-0027** built it: batches are section-coherent, an entry names its
sections with a `sections` selector matched against the heading stack, and all three gates
retired **together** with the error variant and with `build_batches`'s fallibility, exactly
as the entry required. ADR-0014's own revisit trigger fired to get here; the entry's
condition is met, not waived._

## oi-0024-reflow-hooks _(removal note, 2026-08-09)_

**Settled by ticket `d3acc3` (OI-0024, archived RESOLVED in `docs/project/open-issues-archive.md`);
moved from `docs/backlog.md` § Type 3 — Blocked, where this note had already replaced the entry.**
Verified in `sync.js`: a `ResizeObserver`, `document.fonts.ready`, and capture-phase `load`/`error`
listeners on each pane. Note verbatim below.

_Removed 2026-08-09: **`oi-0024-reflow-hooks`** — `sync.js` wired scroll/wheel/touch/
pointer/key listeners and nothing else, so a resized window, a late webfont or a late
image left the two panes describing different places until the reader scrolled again, and
the docstring pushed destroy-and-remount onto the caller. Its stated block was the
commissioned sync-engine pass, and ticket `d3acc3` landed it: a `ResizeObserver` on both
panes, `document.fonts.ready`, and capture-phase `load`/`error` on `<img>` inside either
pane, all coalescing into one animation frame that re-collects both anchor sets and
re-runs the last driving pane's scroll handler. The block was coverage, and the coverage
paid out._

## oi-0016-active-block-scan-perf

**Settled by ti `dd21ad59` and `benchmark/scroll-frame/RESULTS.md` (commit `9fd8390`, 2026-09-02);
moved from `docs/backlog.md` § Type 3 — Blocked.** The measurement shows no scroll-frame overrun, so
the offset cache stays unbuilt and `activeBlockWithProgress` still walks every block. OI-0016 is
archived RESOLVED in `docs/project/open-issues-archive.md`, which records its reopen criterion (a
slow-device jank report); `intersection-observer-active-block` stays live on the same measurement.
Entry verbatim below.

> **RESOLVED 2026-09-02** (ticket `dd21ad59`). The profiling gate this entry says produced no
> evidence **fired**: `benchmark/scroll-frame/RESULTS.md` measured the linear scan's main-thread
> busy time from a V8 profile at up to 11.68% of a 16.67 ms frame with 5,000 blocks — no overrun —
> so the sorted-offset cache stays unbuilt by the ticket's own second acceptance criterion, and
> `docs/project/open-issues.md#OI-0016` reads RESOLVED. Not open work.

- **Type:** 3

- **Sources:** ticgit:dd21ad59, `docs/project/open-issues.md#OI-0016`

- **Description:** `activeBlockWithProgress` linearly scans all blocks every RAF frame; a
  sorted-offset cache + binary search is the named alternative, needing reflow
  invalidation and a non-monotonic-layout policy.
- **Background:** OI-0016 (Review 0006, R0006-0090).
- **Blocked by:** explicit profiling gate — "optimize only when profiling shows
  scroll-frame overruns"; no such evidence exists.

## oi-0017-detected-language-caching — **RESOLVED 2026-08-09**

**Settled by DCR-0028 (ticket `f12b8b`, slices SL-111..SL-114, 2026-08-09); moved from
`docs/backlog.md` § Type 3 — Blocked.** Verified in code: `Cache` carries `get_document_meta` /
`put_document_meta` as defaulted trait methods, `DiskCache` implements both, and
`cli_cache_dir_persists_progress_and_detected_language` drives the end-to-end case. Entry verbatim
below.

- **Type:** 3
- **Description:** A fully-cache-hit `--source-language auto` run reported
  `detected_source_language: None` because the per-unit cache stored no document-level
  metadata.
- **Background:** OI-0017's one remaining item (R0002-0026), narrowed 2026-07-13 when its
  three siblings shipped (`Translator::fingerprint()` + `CacheKey` provider/validation
  identity, `Cache::evict`, Hard-failure eviction). The 0.3.0 window's `instruction_hash`
  addition and the `AUTO`/`auto` sentinel canonicalization (`aa21e8e`) both touched cache
  identity without touching this.
- **Unblocked and resolved:** the named unlock landed. Ticket `f12b8b` / **DCR-0028** put
  the record on the `Cache` seam itself — `DocumentMetaKey` / `DocumentMeta` and two
  defaulted trait methods, written from a live qualifying envelope and replayed only on a
  run that dispatched zero provider batches (SL-111), made durable across processes by
  `DiskCache` (SL-112/SL-113) and exercised end-to-end by `transync translate --cache-dir`
  (SL-114). `open-issues-archive.md` carries OI-0017 as RESOLVED.
- **Unfiled:** resolved and retained for the record; no ticket is owed

## no-cli-surface-selects-the-second-provider

**Settled by ti `473dd1e0` (commit `d136b0d`, 2026-09-03), option (b): `transync-anthropic` is
library-only; moved from `docs/backlog.md` § Type 3 — Blocked.** Verified:
`crates/transync-cli/Cargo.toml` names no `transync-anthropic`, and the crate's module doc and
`contracts.md` §8 state the decision. DCR-0058 later gave the CLI a `--provider` axis for Google,
and `contracts.md` §8 records that it leaves Anthropic library-only. Entry verbatim below.

> **RESOLVED 2026-09-03** (ticket `473dd1e0`, commit `d136b0d`) — option (b): the binary stays
> OpenAI-backed by DCR-0029's scope, and the Anthropic adapter is **declared library-only**, in the
> crate's own module doc and in a paragraph of `contracts.md` §8. The decision this entry waits on
> was taken. Not open work.


- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -c transync-anthropic crates/transync-cli/Cargo.toml` returns 0
- **Blocked by:** an owner decision on whether the CLI links a second provider at all (ADR-0002 currently says compile-time choice)
- **Sources:** ticgit:473dd1e0
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`transync-anthropic` implements `Translator` and reads its three environment variables, but `transync-cli` does not depend on the crate, so no flag, environment variable or profile key can select it.

### Background

This is a dependency-graph fact rather than a missing-argument fact: the binary does not link the crate, so a flag could not reach it even if one existed. The user-visible trap is that someone who already has `ANTHROPIC_API_KEY` exported can reasonably conclude the CLI will use it, and it silently will not — the run goes to OpenAI. ADR-0002 records that the CLI's provider is a compile-time choice, so the current state is intentional, but the manual and the crate's presence together imply otherwise. Blocked on a decision: adding the dependency widens the CLI's build surface and its credential story, and the alternative is documenting the asymmetry loudly enough that nobody is caught by it.

## disk-cache-capacity-policy-unreachable-from-the-cli

**Settled by ti `650bbba6` (commit `d136b0d`, 2026-09-03), option (b): the capacity default is fixed
for CLI runs; moved from `docs/backlog.md` § Type 3 — Blocked.** Verified: `DiskCache::open` opens
with `DiskCacheOptions::default()`, the CLI's `--cache-dir` path calls it and names no
`DiskCacheOptions`, and the decision with its revisit trigger is written on
`DiskCacheOptions::max_bytes` and in `contracts.md` §6's `--cache-dir` row, which is where that
trigger now lives. Entry verbatim below.

> **RESOLVED 2026-09-03** (ticket `650bbba6`, commit `d136b0d`) — option (b): the 1 GiB /
> no-entry-cap default is **declared fixed for CLI runs**, written on `DiskCacheOptions::max_bytes`
> and in `contracts.md` §6's `--cache-dir` row, each carrying a revisit trigger (an operator
> reporting a real 1 GiB cache, or an `--offline` run failing at a miss an open-time trim evicted).
> The trim precondition became ticket `0a3fca`, closed under OI-0044. Not open work.


- **Type:** 3
- **Verified:** yes — reopen verified 2026-08-24: `grep -rc DiskCacheOptions crates/transync-cli/src/` sums to 0
- **Blocked by:** an owner decision between adding a CLI surface and documenting the policy as library-only
- **Sources:** ticgit:650bbba6
- **First seen:** 2026-08-24 · **Last seen:** 2026-08-24

### Description

`DiskCacheOptions` carries a byte budget and an entry cap; no CLI surface reaches either, so every `--cache-dir` run takes the defaults with no way to change them.

### Background

Verified at HEAD: the type is not mentioned anywhere in the CLI's sources. The ticket itself frames this as a decision rather than a defect, and names the two acceptable outcomes — a way to set the policy from the CLI, or an explicit statement in the CLI reference and the DCR that the defaults are fixed for CLI runs and the policy is a library-only knob. Either closes it honestly; leaving it silent does not, because a user who finds `DiskCacheOptions` in the API docs has no way to learn it is unreachable from the binary they are running. Blocked on that choice.

## anthropic-live-leg-never-run

**Settled 2026-10-13 by running the leg: `./scripts/smoke-live-gate.sh anthropic` PASSED** — one
live round-trip against `claude-haiku-4-5`. Moved from `docs/backlog.md` § Type 3 — Blocked on
2026-10-13, in the commit that closed it, verified against the shipped artifact rather than a
label: the gate's own structural assertions ran green against a real Messages API response. The
slug is historical — the leg has run. What closed it was a credential, not code, and it took two:
an organization-scoped key was refused `400` because the Messages API wants an
`anthropic-workspace-id` this adapter does not send, and a workspace-scoped key then passed
unchanged. That limit survived its own fix and is carried forward as
`anthropic-adapter-cannot-name-a-workspace`. Date and model are in the CHANGELOG's `[Unreleased]`
live-gate section, the README's provider-evidence table, and `docs/project/status-archive.md`'s
completed standing-gap action. Entry verbatim below.

> **RESOLVED 2026-10-13.** The adapter has spoken to its provider. The standing gap that had been
> open since the crate landed on 2026-08-10 — 64 days — is discharged, and `transync-anthropic`
> stops being a rostered, publishable crate whose only evidence was its own loopback server.


- **Type:** 3
- **Verified:** yes — reopen verified 2026-09-12 first-hand: the repository root holds
  `.openai.api_key` (gitignored, untracked, mode 0600) and no Anthropic counterpart, and
  `crates/transync-anthropic` sits on the `PUBLISHED_MEMBERS` roster in
  `crates/transync/tests/workspace_publication.rs`. **Re-verified 2026-09-13** by a second
  adversarial pass (two independent lenses plus a completeness critic): every code, record,
  roster and ticket quote in ti `40b29a9d` reproduces, and the type-3 prerequisite is confirmed
  unmet (no `ANTHROPIC_*` variable in the environment, no Anthropic key file, no Anthropic OAuth
  profile, no `ant` CLI). Four facts were refuted and corrected in the ticket body: the first
  recorded live run is 2026-08-05 (the `[0.2.0]` gate paragraph), not 2026-08-20; the crate has
  been packaged by exactly **one** recorded publish dry run (2026-09-05, `--allow-dirty`), not by
  every release's; v0.3.0 was cut 2026-08-07, three days before the crate existed, so the leg was
  unrun at two cuts rather than three; and R0009-0086's `dependentSchemas` fix is latent
  (`dependentSchemas` appears nowhere under `crates/transync-core/src/`), so no live call could
  confirm or refute it and it is withdrawn as evidence of drift risk. A sixth route was added.
  Still unverified: that the leg has never run *anywhere* (a universal negative), and that the
  script and the offline suite are green at HEAD — no cargo command and no network call was made
- **RESOLVED 2026-10-13 — the leg ran and PASSED.** The slug is historical and is kept because
  `docs/backlog-archive.md` cites it twice and an Archive is never edited; read the name as the
  entry's origin, not as its claim.
- **Blocked by:** nothing. Both blockers fell on the same day: a working `ANTHROPIC_API_KEY`
  reached this environment, and when the first one turned out to be organization-scoped — which
  the Messages API refuses for this endpoint — a **workspace-scoped** key replaced it and passed.
  No code changed.
- **Sources:** ticgit:40b29a9d, ticket `342e028d` (coupled route; provenance, not this entry's
  ticket), docs/project/status.md (standing
  gap, open 2026-08-10), docs/project/release-checklist.md (step 10's trigger paths, steps 11-12),
  docs/project/design-change-records/DCR-0029-transync-anthropic-second-provider.md,
  docs/architecture/mvp-scope.md
- **First seen:** 2026-09-12
- **Last seen:** 2026-10-13

### Description

`crates/transync-anthropic` is a complete, rostered workspace member implementing `Translator` over
the Anthropic Messages API. The machinery is written and double-gated — `tests/live_smoke.rs` holds
one full round-trip behind `#[ignore]` plus a predicate demanding `TRANSYNC_LIVE_SMOKE=1` and a
non-empty key, and `scripts/smoke-live-gate.sh anthropic` is the opt-in.

**RESOLVED 2026-10-13.** A credential arrived, the leg ran, and it passed — one round-trip against
`claude-haiku-4-5`. The entry's founding claim, that no process here had ever sent this adapter's
request to the real service, is retired.

It took two keys, and the first attempt is the part worth keeping. An **organization-scoped** key
came back **`400 invalid_request_error`**: *“This API key is not scoped to a workspace, so this
request must include the `anthropic-workspace-id` header with the ID of the workspace to use.”*
A 400 and not a 401 located the fault exactly — `x-api-key`, `anthropic-version`, the model name
and the body were parsed and accepted, and only workspace routing was missing. A workspace-scoped
key then passed with no code change. **The capability gap it exposed is real and outlived the
fix**: the adapter cannot name a workspace, so it requires a workspace-scoped key. That is a
documented constraint in the README's provider-evidence table, and the untaken lever — an optional
`anthropic-workspace-id` — is carried forward as its own entry rather than buried here.

### Background

The offline suite is strong and should not be re-litigated: `tests/offline_end_to_end.rs` drives
`transync::translate` through the adapter against a loopback server that parses the request the
adapter actually sent and answers per unit, so request assembly, the schema-profile pass, the
credential and protocol-version headers, transport, envelope reading and every validation layer are
exercised with no key and no network. What that server cannot do is **refuse** — the only status
line it ever writes is `200 OK`. So every assumption the adapter makes about the provider is
attested only by this repository's own code checked against this repository's own server, and the
one claim only a live call makes is that the provider accepts this request as written. DCR-0029
anticipated exactly this, saying the offline suite "must carry the whole correctness burden"
precisely because there is no key here — so the gap is designed around, not overlooked. The blast
radius is bounded on one side and not the other: no `transync` CLI run can reach the adapter, since
the CLI depends on `transync`, `transync-openai` and `transync-google`, none of which reaches it, and the workspace table deliberately
carries no entry for it — but the crate is on the publication roster at row 7, is packaged and
verify-built by the one publish dry run recorded since it landed (2026-09-05, 14 files / 267.7 KiB,
run with `--allow-dirty`; the `[0.4.0]` section records no publish dry run at all). **The
documentation half of this was fixed 2026-10-13**: the README now carries a provider-evidence table
naming every surface and marking this one **UNVERIFIED** with its reason, so the README no longer
presents the adapter symmetrically with the OpenAI one. The crate's own rustdoc still says nothing,
which is the remaining half of that route. Nothing in the workspace has
been pushed to a registry yet, which is exactly the window in which this is still cheap to decide.
Type 3: the prerequisite is external, concrete, and not an agent's to supply — both 2026-09-13
lenses agreed on the type, noting a real type-2 component (the owner must still pick a route, and
the documentation route is executable today) that the 3 > 2 > 1 tie-break absorbs. Two structural
facts the first pass missed: `crates/transync-anthropic` is **not** in release-checklist step 10's
trigger-path diff, so by the ritual's own rule this leg can never become *due* — it is outside the
gate rather than skipped by it, which is why it lives in `status.md` as a standing gap; and ti
`342e028d` (the v0.5.0 live gate, also blocked, same script) carries a route making a usable key a
precondition of the tag, which against the script's default surface `all` would make an Anthropic
key a cut-time requirement and decide this item with it.

### Options

**Run the leg by name once a key exists and record date and model in the CHANGELOG** *(source:
`docs/project/status.md`'s standing-gap action)* — closes the gap outright. Must be run **by name**,
because the script's default surface `all` demands both keys and refuses up front in this
environment. One caveat priced honestly: the effort estimate holds only if it passes; a failure is
adapter repair of unknown size in a rostered crate.

**Mark the adapter unverified against its live provider in its own rustdoc and the README**
*(proposed by reopen; — **README half DONE 2026-10-13**)* — needs no key at all, which is its
distinguishing point. Effort is two doc edits; blast radius is prose plus the Korean mirrors that
track it. It makes the record honest without making the adapter proven. The README half shipped as
a provider-evidence table covering all six surfaces rather than a sentence about this one; the
rustdoc half is still open.

**Give the adapter an optional `anthropic-workspace-id`** *(the 2026-10-13 run's own finding)* —
the only route that makes an organization-scoped key usable. **Not taken**: the cheap alternative,
a workspace-scoped key, needed no code and closed this entry the same day. It survives as the
`anthropic-adapter-cannot-name-a-workspace` entry, because the limit is still there.

**Replace the live leg with a contract test against captured real provider responses** *(proposed by
reopen)* — gives a refusable server without a standing key, but the capture itself needs one live
call, so it does not remove the prerequisite, it amortizes it. Needs a DCR, since it changes what
the gate means.

**Drop the adapter from the publication roster until it has spoken to its provider** *(proposed by
reopen)* — the only route that removes the exposure rather than documenting it. Blast radius is the
roster test and the release ritual; reversible, but it reverses DCR-0029's scope decision and would
need that record amended.

**Run the existing live test against an Anthropic-compatible endpoint via the base-URL override**
*(proposed by reopen, research pass 2026-09-12)* — the plumbing is already there and validated. The
distinguishing point is the same as its limit: a compatible gateway proves the request is
well-formed, not that Anthropic accepts it, so it is evidence of a different and weaker claim.

**Make the leg release-gated — add `crates/transync-anthropic` to release-checklist step 10's
trigger paths** *(proposed by reopen verification 2026-09-13)* — the only route aimed at
recurrence: the crate is absent from step 10's trigger diff today, so the leg can never become due
and a seventh release can be cut in this position. One checklist edit, no code, no credential, and
no provider evidence of its own; composes with any other route. Whether widening the trigger set
DCR-0015 names is a record-free "when to run" edit or needs a dated note on that archived record is
an owner reading, and should be settled before the edit.

## unopened-breaking-window

**Settled 2026-10-13 by the owner's decision to make the next version v0.6 (DCR-0062).** Moved
from `docs/backlog.md` § Type 2 — Needs decision on 2026-10-13, in the commit that closed it. The
window this entry existed for is **open**, so the two breaking changes parked on `master` since
before v0.5.0 closed — `LineOffsets::offsets` becoming private (R0010-0023) and the Markdown
intake's move from `parser` to `intake::markdown` (DCR-0053) — may now ship. Closes ti
`2d616b6a`.

What the decision deliberately did **not** do, because the entry's own second pass had already
corrected the filed text on this point: it did not make OI-0051, OI-0052 and OI-0054 a
precondition. They are eligible for the window, not required by it, and the owner's recorded
right to defer them again (stated at OI-0052 and repeated in `status.md`) stands. DCR-0062 also
carries forward this entry's verified sibling-consumer fact, re-checked 2026-10-13 rather than
taken on the entry's word: `dynwebserver`'s `dynweb-core` still builds `TranslationBatch` with
literals naming the field OI-0051 removes, at two sites, while `resp-translator`'s one production
impl still binds the batch `_batch`. Entry verbatim below.

> **RESOLVED 2026-10-13** (DCR-0062, ti `2d616b6a`). The window is open. What rides it is a
> separate decision per item, and the `[0.6.0]` CHANGELOG section will be its enumeration, the way
> `[0.5.0]`'s was for its seven.


- **Type:** 2
- **Verified:** yes — reopen verified 2026-09-12 first-hand: `LineOffsets::offsets` is private
  with `line_starts()` in its place, and CHANGELOG `[Unreleased]` carries both changes under
  Changed (BREAKING). **Second pass 2026-09-13** — two adversarial lenses plus a completeness
  critic, both *partly-confirmed*: every quoted record, both commits, the closed window, the
  manifest at `0.5.0` and all six ticket ids reproduced first-hand, and **type 2 upheld,
  uncontested**. Three facts in this entry were wrong and are corrected below — both sibling
  checkouts pin tag `v0.5.0`, not `v0.4.0`; nothing is published to a registry, so there is no
  release to yank; and carrying OI-0051/0052/0054 is the first route's chosen content rather
  than a precondition of opening a window. A sixth route was added, here and in the ticket.
- **Sources:** ticgit:2d616b6a, CHANGELOG.md `[Unreleased]`, docs/project/status.md,
  docs/project/phase-state.yaml, docs/project/open-issues.md#OI-0051,
  docs/project/open-issues.md#OI-0052, docs/project/open-issues.md#OI-0054
- **First seen:** 2026-09-12
- **Last seen:** 2026-09-13

### Description

Two API changes are already committed to the tree and cannot ship, because v0.5.0 closed the
sanctioned breaking window on 2026-09-05 and nobody has opened another.
`transync_syntax::intake::markdown::ranges::LineOffsets::offsets` went private with a read-only
`line_starts()` accessor in its place (review finding R0010-0023), and the Markdown intake moved
from `parser` to `intake::markdown` (DCR-0053). Both are invisible through the `transync` facade —
`public_surface.rs` pins the module hidden — and breaking only for a program depending on
`transync-syntax` directly. Three registered items are queued on the same unopened window:
OI-0051, OI-0052 and OI-0054. So **five records are gated by one decision nobody has taken**, and
that decision is the owner's; no amount of work clears it.

### Background

A "breaking window" here is an owner decision that a given release may carry breaking API changes.
The rule is stated in `CLAUDE.md`, in `phase-state.yaml` and in the release checklist, and it is
unchanged: a further breaking change needs a **new** window and the owner decision that opens one.
The two parked changes are not equivalent and the asymmetry decides the question. The private-field
change **has a revert fallback** — the field can go back to `pub` and only the accessor ships — and
the CHANGELOG says so explicitly. The rename has none: reverting it would re-open the asymmetry
DCR-0035 exists to explain, and it was itself the discharge of a deferral that record wrote down.
The three queued register entries each name the same blocker in their own words, and the owner has
already recorded that OI-0051 and OI-0052 *may be deferred again* when v0.6.0's window opens —
so opening a window does not automatically consume them. Type 2 rather than type 3: the
prerequisite is not external and unmet, it is a choice nobody has made, and the tie-break does not
apply because nothing outside the decision has to land first.

### Options

**Open a v0.6.0 window carrying both parked changes plus the three queued items** *(proposed by
reopen)* — the only route that clears all five records at once. Effort is three implementations
plus the release ritual: both parked changes are already committed, but OI-0051, OI-0052 and
OI-0054 are code, and carrying them is this route's own content rather than a precondition of
opening a window — the owner has recorded that two of the three may be deferred again when
v0.6.0's window opens, which is what option 6 below takes up. Blast radius reaches every direct
`transync-syntax` consumer, of which the roster names one (`dynwebserver`), plus anything naming
`transync::TranslationBatch` for OI-0051's tier (a) change; both sibling checkouts pin tag
`v0.5.0` (re-read 2026-09-13 — this entry had said `v0.4.0`) and so are unaffected until they
bump. Irreversible in one respect only: an annotated tag is outward-facing and is not withdrawn,
and nothing is published to a registry, so there is nothing to yank. Record needed: the owner decision that opens the window, plus the checklist's step 2a.

**Revert the private field to `pub` and ship only the accessor** *(source: `CHANGELOG.md`
`[Unreleased]`, which names this route)* — smallest blast radius and the cheapest to undo, but it
leaves the rename still needing a window, so it narrows the problem rather than closing it, and it
re-admits a field whose own doc comment argues the field and its source are one fact.

**Cut a non-breaking 0.5.x — only possible from a release branch off the `v0.5.0` tag — and defer
the decision again** *(proposed by reopen; restated 2026-09-13, since both breaks are already on
master and no 0.5.x cut from this tree is non-breaking)* — zero blast radius now and it forecloses
nothing, but it would be this repository's first release branch (`git branch` lists `master`
alone, and the release checklist has no rule about branches or cherry-picks), `43cc19c` carries
the `LineOffsets` break in the same two-file commit as an unrelated `transync-html` fix and so
would have to be split, and every deferral adds another record to the five while `[Unreleased]`
grows changes that cannot ship.

**Declare deep `transync-syntax` module paths unsupported, so neither change counts as breaking by
policy** *(proposed by reopen)* — needs no window at all, which is its distinguishing point. It is
the only route requiring a new or amended ADR, because it changes what the workspace promises
about a published crate's surface, and it is the hardest to reverse: a support promise withdrawn
is not re-made cheaply.

**Declare the window open on master now and decide what rides it later** *(proposed by reopen,
research pass 2026-09-12)* — separates the two questions the other routes fuse. Effort is one
recorded decision; it unblocks work immediately, at the cost of an open window, which the project's
own history shows is a state it has twice had to close deliberately.

**Open v0.6.0 and cut it promptly with the committed set, re-deferring the three queued items**
*(proposed by reopen verification 2026-09-13)* — the only route that unfreezes both pinned sibling
checkouts quickly, which is the first consequence the ticket names. Effort is the release ritual
alone: the two parked changes and the 31 commits since `v0.5.0` need no further code, though the
DCR-0015 Part C live-endpoint gate is already recorded as triggered and unrun at the last cut and
needs a real key and network, and the tag is the owner's act. Blast radius is the one direct
`transync-syntax` consumer, whose `parser::parse` call sites break when it bumps — the same break
every window route hands it, taken sooner. Record needed: the window opening as `ff788f7` recorded
it, step 2a's condition for the new number, and a dated, falsifiably-triggered deferral for each of
OI-0051, OI-0052 and OI-0054.
