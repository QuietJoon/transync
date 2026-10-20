---
type: Explanation
title: Where batches stop — whole sections, budgets and coalescing
description: What decides where one request ends and the next begins — whole sections as the unit of packing, the two admission tests that keep every batch's prompt exact, and the `coalesce_sections` knob that trades request count for boundaries that line up with headings.
tags: [batching, profile, glossary, DCR-0027, DCR-0057]
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
  - { id: dcr-0057, resource: docs/project/design-change-records/DCR-0057-sections-coalesce-into-batches.md }
  - { id: backlog, resource: docs/backlog-archive.md }
synced_hash: fb15c0c6a4a9ed82121802194c5514c42d5be21d6b9c5931f142cebcdc521f8c
---

# Where batches stop — whole sections, budgets and coalescing

A batch stops at a section boundary when the next section is not
*admissible* — it would need a different request prompt — or when it does not
fit the budgets. It does not stop there merely because a heading is there.
Sections are the unit batches are built out of, and a batch takes as many
consecutive whole sections as it can.

A translation run does not send your document one block at a time. Blocks are
packed into batches, and each batch becomes one request. The unit list is
partitioned at every heading before anything is packed, and the packer then
walks those sections in document order. What it produces is always one of
exactly two shapes:

- **N consecutive whole sections** whose combined estimates fit all three
  budgets — the input-token target, the output-token ceiling scaled by the
  expansion factor, and the hard unit cap.
- **A fragment of one oversize section**, when a single section is too big to
  be one request.

What never happens is the third shape: a batch holding the tail of one section
and the head of the next. That shape is what the whole design is arranged to
exclude, and the rest of this page is why.

A section here is exactly what it looks like in the source: a heading of any
level from `#` to `######` starts one, and the heading belongs to the section it
*opens*, not to the one it closes. Everything before the document's first
heading is its own section, the preamble. A document with no headings at all is
one section.

## Why the boundary matters at all

### A glossary scope that can mean something

This is the reason sections exist as the packing unit. `scope = "section"` was
in the profile schema for a long time before it did anything, and the obstacle
was not the matching logic — it was that a batch could contain the tail of one
section and the head of the next, so "which glossary entries apply to this
request's system prompt" had no single answer. Every available answer was wrong:
render the scoped entry and it steers blocks outside its section; drop it and it
fails to steer blocks inside.

Packing whole sections makes the question answerable, and the answer is what
each request's prompt is compiled from. See
[how glossary entries are resolved](./how-glossary-entries-are-resolved.md) for
the resolution rules themselves.

### A per-request reserve priced from the right prompt

Some of every request is fixed overhead: the compiled system prompt, the
assembled instruction envelope, the two language labels. That overhead is
measured and subtracted from the input budget before any unit is packed, so
packing cannot overshoot. Because a batch's sections all compile to the same
prompt, that subtraction can use *that* prompt rather than a document-wide worst
case, so the reserve gets more exact rather than more conservative. The same
property carries into retries: a retry re-packs within a batch, and a batch's
prompt is fixed, so its budget is the same in the first round as in the last.

## The two admission tests

Exactness, not the boundary itself, is what the design protects. Two consecutive
sections may therefore share a batch, and they do so when they agree about both
things a request carries:

1. **Their effective glossary.** Two sections resolving to the same entry list
   compile to byte-identical prompt bytes, so a co-batched section reads exactly
   the prompt it would have read alone. Sections resolving to different
   glossaries are never merged — a merged glossary would be a prompt neither
   section asked for, which is the exactness section scope exists to provide.
2. **The instruction clauses their units call for.** The raw-HTML segment
   contract, the row-window table contract and the HTML-document clause are
   assembled from the batch's *own* membership. Merge a prose section with one
   holding a raw-HTML block and the prose units' request grows a clause it never
   carried, so that pair is not admissible either.

Both tests are equality tests, so packing walks maximal runs of consecutive
sections that agree on both, and packs each run against the budgets.

A profile with no section-scoped glossary entry has one compiled prompt for the
whole run, and a document that is entirely prose has one instruction shape, so
on the common document every section is admissible with its neighbors and only
the budgets decide where a batch ends.

## The oversize section, and the one direction it does not go

A section that cannot fit in one request still splits across several. When the
budget runs out inside a section, packing does what it always did — fills a
batch, starts another — and all of the resulting batches stay inside that
section. This is the only thing that ever separates one section's units.

Such a section coalesces in **neither** direction: it cannot join the batch
before it, and its trailing fragment cannot host the section after it. Both
would put a section boundary inside a fragment, which is the shape excluded
above. A small section sitting immediately before a large one therefore gets its
own request even though the large one's leading units would have fit alongside
it.

Batch ids are unaffected by any of this: they stay a single document-order
sequence from one to N across the whole document, however the sections divided
them.

## What coalescing costs

Fewer, larger batches are the point — on a 30-section fixture, the strictly
confined packing dispatches 30 requests where coalescing dispatches materially
fewer for the same units. Three consequences come with that, none of them
visible in the output bytes.

**The run-wide retry allowance gets tighter.** The per-batch retry budgets are
unchanged, but the validation report's `provider_retries` and
`batch_schema_faults` ceilings are multiples of the input batch count. Fewer
batches means fewer charges of the same per-batch budget across the run.

**An envelope fault implicates more units.** A provider that drops or duplicates
rows makes every implicated unit of that batch an offender, and a larger batch
has more of them. Per-unit *content* faults are unaffected: an innocent
batch-mate's payload is validated and accepted in the round it arrived, never
re-sent.

**A short document may stop filling the dispatch slots.** Concurrency defaults
to six in-flight requests; a document that coalesces into fewer than six batches
no longer has six to dispatch. On most documents the round trips saved are worth
more than the parallelism given up, but on a short, heading-dense document tuned
for wall clock it is the other way around.

## Turning it off

`[batching].coalesce_sections = false` — or `--no-coalesce-sections` for one
run — packs every section into batches of its own, so every batch's units come
from exactly one section and batch boundaries line up with headings. That is the
setting to reach for when you want boundaries to line up, or when the document
is short and heading-dense enough that the concurrency headroom matters more
than the request count. Its cost is one provider request per heading, floored:
no budget can buy it back, because the partition runs before the packer. A
ten-line section under its own `###` is one request even if the budget could
have swallowed the entire document.

`--coalesce-sections` is the other half of the pair, and exists to beat a
profile that set `false`; the argument parser rejects both flags at once, as
exit `1`. Coalescing is on when neither is passed and the profile says nothing,
so the key is worth writing only to turn it off.

## What does not change either way

Every unit is sent under the same system prompt and the same instruction
under either setting — both admission tests pin the two batch-scoped axes of
the cache key — so no cache entry written under one setting is orphaned under
the other. Batch ids reach neither the alignment map nor the validation report,
and no schema version moves.

What changes is how many requests the document costs, and what each request
carries. A coalesced request holds its neighbours' units too, and the response
schema follows its unit count, so the model reads each section in different
company under the two settings. A live model's translation can therefore
differ between them; nothing promises the same output from both.

## Related

- [How glossary entries are resolved](./how-glossary-entries-are-resolved.md) —
  the feature this packing rule exists to make possible.
- [Profile TOML schema](../../../reference/user/en/profile-toml-schema.md) —
  the `[batching]` keys, with their types and defaults.
- [CLI reference](../../../reference/user/en/cli.md) —
  the flags that overlay those keys for one run.
- [How to write a Profile TOML for a translation style](../../../how-to/user/en/write-a-translation-profile.md) —
  where those keys go in a working profile.
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md) —
  reading the warnings and the validation report a run produces.
