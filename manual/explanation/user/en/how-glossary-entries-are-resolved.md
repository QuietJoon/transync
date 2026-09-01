---
type: Explanation
title: How glossary entries are resolved
description: Why a glossary term can come from your profile or from the auto-glossary preflight, how a section-scoped entry is matched to a heading, and which entry wins when several could apply to the same term.
tags: [profile, glossary, ADR-0014, DCR-0027]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: profile-rs, resource: crates/transync-core/src/profile.rs }
  - { id: llm-rs, resource: crates/transync-core/src/llm.rs }
  - { id: prompt-rs, resource: crates/transync-core/src/llm/prompt.rs }
  - { id: unit-rs, resource: crates/transync-core/src/unit.rs }
  - { id: unit-section-rs, resource: crates/transync-core/src/unit/section.rs }
  - { id: adr-0014, resource: docs/decisions/archive/0014-section-scoped-glossary-renders-globally.md }
  - { id: dcr-0027, resource: docs/project/design-change-records/DCR-0027-section-coherent-batching-and-glossary-section-scope.md }
synced_hash: 2c09f59a330a21394e7b6e454dca86e4234070b07e717e88929332f694fef104
---

# How glossary entries are resolved

A glossary entry is advisory prompt text. It becomes one bullet —
`- "source" → "target"`, plus the note when you wrote one — inside the system
prompt of the requests that carry it, and nothing on the way back checks that
the model actually used the term. No validation layer looks at the glossary;
none of them could, because "did this paragraph honor the preferred rendering"
is a content judgement, and content is the model's half of the contract.

That one fact explains most of the resolution rules below. Because nothing
downstream can correct a term that reached the wrong place, the design spends
its effort on making sure a term only reaches the places you meant — and on
saying so out loud when it can tell that something you wrote cannot mean what
it says.

## Two places entries come from, and why the profile wins

Every entry in a run's glossary is either **static** — written by you in the
profile's `[[glossary]]` tables — or **extracted**, harvested from the source
document by the auto-glossary preflight when `auto_glossary` is on. The
preflight runs once, after the document parses and before any batch is built,
so its results are in force everywhere a static entry is.

The two are merged with a plain rule: the static entry wins. An extracted term
whose source term matches a static one (compared trimmed and case-folded) is
dropped, counted as a conflict in the run's report, and never rendered. The
asymmetry is deliberate rather than tidy — you chose your terms, and the
preflight guessed at them from the document, so where a guess and a choice
collide the choice is the one with intent behind it.

Since section scope shipped, that rule is sharper than "static wins" in one
specific way: an extracted term is dropped only when a **global** static entry
claims it. A static entry scoped to one section claims the term *in that
section*, and dropping the extracted global one on that basis would leave the
rest of the document with no rendering at all for a term the document
demonstrably uses. The two coexist instead, and per-section resolution then
hands the static entry its own sections and the extracted one everywhere else —
preserving both intents rather than letting one section's terminology erase the
document's.

## Why a scope a provider returns is ignored

Extracted entries are always document-wide, and they are made document-wide by
force, not by trust. The extraction request's response schema carries exactly
three fields per term — source, target, and an optional note — and is strict, so
scope and selectors have no wire representation to travel in. If a custom
provider implementation constructs entries directly and sets them anyway, the
merge overwrites the scope to global and empties the selector list rather than
carrying it as inert data.

The reason is the one that governs everything the model touches: the source
document is untrusted input, and a harvest derived from it is untrusted too. A
profile is authored, reviewed, and version-controlled; a harvest is produced in
one call from text that may itself be trying to steer the run. Letting a
harvest decide *where* a term applies would put section semantics on the far
side of that boundary, so the extractor is allowed to propose terminology and
never allowed to propose scope.

## Global scope versus section scope

A global entry — the default, and what you get when you leave `scope` out —
renders into every batch's system prompt. A section-scoped entry renders into
the prompts of the batches belonging to its own sections and into no other
prompt at all.

That second sentence took a long time to become true, and the history is worth
knowing because it explains the shape the feature finally took. The scope
existed in the schema long before any filtering did. The first attempt rendered
scoped entries into every prompt with an advisory suffix appended to the bullet,
which was reversed (ADR-0014, amended): a term meant for one section silently
steering every section is the *most* misleading failure mode available when
nothing downstream can correct it, and the suffix only papers over that on
models that read it. The scope was then rejected outright at load time, as a
deferral with a stated trigger — re-admit it once batches stop straddling
headings, because only then does "which entries apply to this batch's prompt"
have a single answer.

That trigger fired with section-coherent batching (DCR-0027), and the rejection
was retired by deletion rather than left in place as a disabled gate. The
consequence you can rely on: filtering, never annotation. Scope and selectors
are never rendered as prompt text and never become part of a cache key, because
they change no byte the model reads. They decide which bullets exist in a given
request, and that is the whole of their effect.

See [why batches stop at section boundaries](./why-batches-stop-at-section-boundaries.md)
for the batching half of that story, including what it costs.

## How a section is matched: by name, at any depth

A section's identity is its **heading stack** — the plain text of every heading
enclosing its content, outermost first, including the heading that opens it.
Under `### Windows` beneath `## Installation` beneath `# Guide`, the stack is
`Guide`, `Installation`, `Windows`. A section-scoped entry applies to that
section when any selector you listed matches any heading on the stack, compared
trimmed and case-folded — the same identity rule that decides when two source
terms are "the same term".

Two consequences fall out of matching the *stack* rather than the innermost
heading. Subsection inheritance is free: a term selected by `Installation`
applies inside `### Windows`, because `Installation` is still on the stack
there. And the heading that opens a section is steered by its own section — the
`## Installation` line itself is translated under the terminology you scoped to
`Installation`, which is not what would happen if applicability were computed
per block from each block's own ancestors.

Heading **levels** are never consulted beyond "is this a heading". A selector
names a section by what it is called, not by how deep it sits, and that was
chosen rather than fallen into. Documents get restructured: a `##` becomes a
`###` when a chapter grows a parent, and a selector that had encoded depth would
silently stop applying with no error and no output difference you could see.
Richer selectors — level-aware paths like `Install > Windows`, globs, regular
expressions — were left out for now as a compatible later extension, on the
grounds that text-at-any-depth is the behavior most authors would predict and
the one that survives editing.

The **preamble** — everything before a document's first heading — has an empty
heading stack, so no section-scoped entry can ever reach it. There is no
selector that names it, because there is no heading to name. Only global entries
steer a document's opening material, which is worth remembering for documents
that carry a substantial lead-in before the first `#`.

## Which entry wins inside a section

For each batch, the entries that render are resolved for that batch's section:
every global entry, plus every section-scoped entry that applies there, reduced
to one winner per source term.

**An applicable section-scoped entry beats the global one, silently.** No
warning is emitted, because that is precisely what the scope is for — you asked
for a different rendering here, and you got it. A diagnostic would be a warning
that the design is working. This is the single most useful thing to know about
the precedence rules: a term you defined globally can simply not appear in one
section's prompt, with nothing on stderr to tell you so, and that is by
construction rather than by accident.

**Two section-scoped entries on one term can both apply**, and then profile
order decides. The loader already refuses the case it can settle on its own —
two section-scoped entries on the same term whose selector lists overlap, which
is a fact about the profile and needs no document to detect — but nested
headings can put two entries with genuinely *disjoint* selectors over one
section, because a section's stack carries several heading names at once. When
that happens the first entry in profile order wins, the shadowed one is named,
and the message says which section it was observed in.

That warning is emitted once per (shadowed entry, winning entry) pair per run,
not once per section. A shadowing that recurs across a dozen nested subsections
is one finding about your profile, not twelve; and the pair, not the section, is
what the finding is actually about. (An earlier design deduplicated on the
compiled prompt instead, which turned out to *lose* the warning rather than
condense it — two sections can arrive at the same set of surviving entries by
different routes, one with a shadowing and one without, and if the quiet route
is packed first the noisy one never speaks. DCR-0027's appended amendment is the
record of that correction; its original section 5 describes the mechanism that
did not ship.)

The upshot for authoring is that "a source term can be claimed once" is now
"once per place the claims can meet". Two global entries on one term still
collide, and the first wins. Two section-scoped entries on one term are fine
while their selectors stay disjoint. A global entry and a section-scoped entry
on one term are not a collision at all — they are the override pattern, and both
are supposed to load.

## Why an empty selector list applies nowhere rather than everywhere

A `scope = "section"` entry with no usable selectors — you left `sections` out,
or every string in it was blank — cannot say where it applies. There are only
two readings available, and the loader takes the strict one: the entry applies
nowhere, and because an entry that applies nowhere is not a narrow entry but a
dead one, the whole entry is dropped and named.

Reading it as "everywhere" was rejected because it makes a typo change the
meaning of the scope key. An author who wrote `scope = "section"` said, in
writing, that this term should *not* steer the whole document; silently
promoting it back to global on a missing selector would deliver the exact
behavior ADR-0014's amendment was written to remove, and would do so at the
moment the author was least likely to be watching.

The same instinct — say it, do not fail — runs through all of the glossary
handling. A `sections` list on a global entry is cleared and named, since a
global entry applies everywhere by definition and honoring the list as a
narrowing would hand you the opposite of what you wrote. A repeated selector in
one entry is deduplicated and named. A section-scoped entry naming sections this
particular document does not have is named once per run and left alone entirely,
because a profile is document-independent — the entry may be perfectly correct
for the sibling document you will translate next, and refusing the run over it
would make profiles unshareable. None of these are errors. Loading a profile can
fail for exactly two reasons — a required field is missing, or the TOML is
malformed — and no glossary problem is either of them.

## Related

- [Why batches stop at section boundaries](./why-batches-stop-at-section-boundaries.md) —
  the batching rule that makes section scope possible, and the request-count
  cost it carries.
- [How to write a Profile TOML for a translation style](../../../how-to/user/en/write-a-translation-profile.md) —
  the authoring steps, including where the `[[glossary]]` tables go.
- [Profile TOML schema](../../../reference/user/en/profile-toml-schema.md) —
  the exact keys, their types, and their defaults.
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md) —
  reading the warnings a run prints, and the run's validation report.
