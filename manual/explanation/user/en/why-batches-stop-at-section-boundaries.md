---
type: Explanation
title: Why batches stop at section boundaries
description: Why no request ever mixes units from two headings, what that buys in glossary exactness and context coherence, and what it costs in request count on heading-rich documents.
tags: [batching, profile, glossary, DCR-0027]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: unit-section-rs, resource: crates/transync-core/src/unit/section.rs }
  - { id: unit-rs, resource: crates/transync-core/src/unit.rs }
  - { id: batch-rs, resource: crates/transync-core/src/batch.rs }
  - { id: profile-rs, resource: crates/transync-core/src/profile.rs }
  - { id: dcr-0027, resource: docs/project/design-change-records/DCR-0027-section-coherent-batching-and-glossary-section-scope.md }
  - { id: backlog, resource: docs/backlog.md }
synced_hash: fb15c0c6a4a9ed82121802194c5514c42d5be21d6b9c5931f142cebcdc521f8c
---

# Why batches stop at section boundaries

A translation run does not send your document one block at a time. Blocks are
packed into batches, and each batch becomes one request. How that packing works
is mostly a budget question — keep adding units until the estimated input or the
estimated response would overflow, then start a new batch — but there is one
rule that overrides the budgets entirely:

**A batch's units always come from one section.** The unit list is partitioned
at every heading before anything is packed, and each section is then packed on
its own. No request ever contains blocks from two different headings.

A section here is exactly what it looks like in the source: a heading of any
level from `#` to `######` starts one, and the heading belongs to the section it
*opens*, not to the one it closes. Everything before the document's first
heading is its own section, the preamble. A document with no headings at all is
one section, which is why a flat document packs exactly as it always did.

## What the rule buys

### A glossary scope that can mean something

This is the reason the rule exists. `scope = "section"` was in the profile
schema for a long time before it did anything, and the obstacle was not the
matching logic — it was that a batch could contain the tail of one section and
the head of the next, so "which glossary entries apply to this request's system
prompt" had no single answer. Every available answer was wrong: render the
scoped entry and it steers blocks outside its section; drop it and it fails to
steer blocks inside.

Confining a batch to one section makes the question answerable, and the answer
is what each request's prompt is compiled from. See
[how glossary entries are resolved](./how-glossary-entries-are-resolved.md) for
the resolution rules themselves.

### Context that is about one thing

Every unit in a batch now sits under the same headings. The surrounding material
a model reads while translating a paragraph is material from the same part of
the document, rather than a boundary-straddling mixture where the first half of
the request is release notes and the second half is an API table. This is a
softer benefit than the glossary one — it is a claim about translation quality,
which nothing here measures — but it is the natural consequence, and it points
the same way.

### A per-request reserve priced from the right prompt

Some of every request is fixed overhead: the compiled system prompt, the
assembled instruction envelope, the two language labels. That overhead is
measured and subtracted from the input budget before any unit is packed, so
packing cannot overshoot. Because a batch belongs to one section, that
subtraction can use *the section's own* compiled prompt rather than a
document-wide worst case, so the reserve gets more exact rather than more
conservative. The same property carries into retries: a retry re-packs within a
batch, and a batch is single-section, so its budget is the same in the first
round as in the last.

## What the rule costs

It is worth being blunt about this, because the cost lands hardest on exactly
the documents this project is most often pointed at.

**A heading-rich document dispatches at least one batch per section, however you
set the budgets.** Reference pages, API documentation, FAQs, changelogs — any
document that is mostly short sections under many headings — will produce far
more requests than the raw token arithmetic suggests. Raising
`[batching].target_input_tokens_per_batch` or `max_units_per_batch` does not
change this: those budgets bind *within* a section, and nothing in the packer
merges two sections. A ten-line section under its own `###` is one request even
if the budget could have swallowed the entire document.

**Small trailing sections do not merge.** There is no "if it is tiny, tack it
onto the neighbour" relaxation. A section that is one heading and two sentences
gets its own request, next to a sibling that is also one heading and two
sentences.

**More requests means more latency and more per-request overhead.** The system
prompt was always paid once per batch; what grows is the number of batches, so
the fixed overhead is paid more times, and the run spends more wall-clock time
in round trips. Nothing about a *unit's* cost changes — the same blocks are
translated either way — but the envelope around them is paid per request.

There is also a cache consequence, and it cuts both ways. Because a request's
prompt is now compiled per section, two sections that resolve to the same
glossary compile to the same prompt bytes and their units can share cache
entries. Two units that differ only in which glossary their section resolved to
stop sharing — correctly, since the model genuinely read different prompts for
them. A profile with no section-scoped entry has exactly one such compilation
for the whole run, so its prompts and its cache keys are byte-identical to what
they were before section coherence existed; the machinery is invisible until a
profile uses the scope.

## The one exception runs the other way

A section that cannot fit in one request still splits across several. When the
budget runs out inside a section, packing does what it always did — fills a
batch, starts another — and all of the resulting batches stay inside that
section. This is the only thing that ever separates one section's units.

Note the asymmetry, because it is the whole shape of the rule: a section may be
*split across* batches, but two sections are never *merged into* one. Batch
numbering is unaffected either way — batches stay a single document-order
sequence from one to N across the whole document, regardless of how the sections
divided them.

## Why small sections are not coalesced — and what is not available

The obvious relaxation was considered and named at design time: let **adjacent
whole sections share a batch when their effective glossaries are identical**.
That version preserves glossary exactness (a co-batched section reads the same
prompt it would have read alone) and never separates a section's units, and for
a glossary without section scopes it would restore the old request counts almost
exactly, since every section resolves to the same glossary. It fails the rule as
written, though — such a batch does contain a section boundary — so it needed
its own decision rather than riding along.

That decision was made, and the answer was **deferred to 2027**. Not declined:
tracked, with a revisit date, in the project's backlog under
`section-batch-coalescing`. The reasoning for a date rather than a condition is
recorded there and is worth repeating, because it is honest about what is not
known: the natural condition — "revisit once real request-count pain is
observed" — needs someone running heading-rich documents at volume, and no such
measurement exists yet. A date guarantees the question gets asked again; the
condition, if it fires first, is a reason to ask early.

Strict confinement shipped first for a reason that is not just literalism. It is
what the commissioned acceptance criteria say, and the asymmetry above is why:
the sanctioned exception splits a section across batches and never merges two
into one. Beyond that, coalescing is cheap to add later and impossible to remove
quietly once defaults depend on it — so shipping the strict version first keeps
the option open in both directions.

**Nothing of this exists today, and it should not be written into a profile.**
The shape sketched for it, if it is ever built, is an opt-in
`[batching].coalesce_sections` knob defaulting to off. There is no such key, no
CLI flag, and no code behind either. Putting `coalesce_sections` in a
`[batching]` table earns an `unknown profile key` warning on stderr and changes
nothing about the run.

## Related

- [How glossary entries are resolved](./how-glossary-entries-are-resolved.md) —
  the feature this batching rule exists to make possible.
- [Profile TOML schema](../../../reference/user/en/profile-toml-schema.md) —
  the `[batching]` keys that do exist, with their types and defaults.
- [How to write a Profile TOML for a translation style](../../../how-to/user/en/write-a-translation-profile.md) —
  where those keys go in a working profile.
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md) —
  reading the warnings and the validation report a run produces.
