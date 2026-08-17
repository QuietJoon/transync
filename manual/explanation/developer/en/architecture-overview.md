---
type: Explanation
title: Why block-ID sync, not scroll-percentage
description: The reasoning behind transync's core design choice — one stable block ID as the only sync currency, end to end — how that choice shapes every other layer, and why a table split for the model is still one block for the reader.
tags: [architecture, ADR-0001, ADR-0002, DCR-0026, DCR-0029]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: arch-readme, resource: docs/architecture/README.md }
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: adr-0001, resource: docs/decisions/0001-block-level-alignment-as-sync-currency.md }
  - { id: adr-0002, resource: docs/decisions/0002-http-free-core-with-translator-trait.md }
  - { id: adr-0019, resource: docs/decisions/0019-wasm-demo-layer.md }
  - { id: dcr-0026, resource: docs/project/design-change-records/DCR-0026-oversize-table-row-window-split.md }
  - { id: dcr-0029, resource: docs/project/design-change-records/DCR-0029-transync-anthropic-second-provider.md }
  - { id: llm, resource: crates/transync-core/src/llm.rs }
  - { id: unit-split, resource: crates/transync-core/src/unit/split.rs }
  - { id: pipeline-merge, resource: crates/transync-core/src/pipeline/merge.rs }
  - { id: regen, resource: crates/transync-syntax/src/regen.rs }
  - { id: render, resource: crates/transync-syntax/src/render.rs }
  - { id: anthropic, resource: crates/transync-anthropic/src/lib.rs }
  - { id: cli-provider, resource: crates/transync-cli/src/translate_cmd/provider.rs }
synced_hash: 1348d82f2cb0cd9348c92130bfea22d761b1a619a9ec3856613f75acf5275479
---

# Why block-ID sync, not scroll-percentage

transync renders a source document and its translation side by side and
keeps the two panes scrolling together. The obvious-looking implementation
of that — match scroll position by percentage of document height — is the
one this project deliberately does not build, and the reason is worth
understanding before touching any of the layers that follow from it.

## The problem with the obvious approach

Scroll-percentage sync assumes the two documents are the same shape: that
40% down the source is "the same place" as 40% down the translation. It
rarely is. A translated paragraph is longer or shorter than its source
almost every time (Korean and Japanese routinely run 1.3–1.8× the source
token count); a table stays the same row count but its cell widths differ;
a heading might wrap to two lines in one language and one in another.
Percentage sync degrades quietly — the panes drift further apart the
longer the document runs, with no error, just a reader increasingly
looking at unrelated content in the two panes.

An alternative that fixes the drift but trades it for a different failure
is sentence-level pairing by ordinal index (the approach one of this
project's own reference implementations, `LLM-API/LLM-Trans`, takes):
segment both documents into sentences, pair sentence *N* of the source
with sentence *N* of the translation. This works until the model merges
two source sentences into one, splits one into two, or reorders a clause
across a sentence boundary — all ordinary translation behavior, not edge
cases — at which point every pairing after that point is silently wrong.

## The decision: one ID, assigned once, never re-derived

transync's answer (ADR-0001) is to give every sync-relevant block — a
heading, paragraph, table, code block, list item, blockquote — one stable
ID, assigned once by the Rust parser from the *source* document, and to
carry that same ID through every layer that follows: the LLM request, the
LLM response, the regenerated translated Markdown, the alignment map, and
the `data-sync-id` attributes in the rendered HTML both panes mount. The
browser's sync engine never computes position from content — it looks up
one pane's active block ID and asks the DOM for the element carrying the
same ID in the other pane. There is no percentage anywhere in the sync
path, and no independent judgment about what "corresponds" to what: the
correspondence was decided once, upstream, by the same process that will
go on to constrain the translation itself.

This is also why block-level, not sentence-level: a block is a structural
unit the parser already produces and the LLM contract already treats as
one translation request, so choosing it as the sync unit adds no new
segmentation logic that Rust and JavaScript would otherwise have to agree
on independently. Sentence-level click-to-pair — a real feature the
LLM-Trans reference has — is consequently out of scope; the project
accepted that trade explicitly rather than reaching for it by default.

## What this buys, and what it costs

The ID-first design pays off in more than the browser: it also sets the
shape of the validation contract described in
[why layered validation and bounded retry/fallback](./validation-retry-fallback-model.md).
Because the LLM is handed one block per request and told which ID it's
translating, a validator can check "does the ID set that came back match
the ID set that went out" as a first, cheap, purely structural gate before
looking at content at all — a check that has no equivalent in a
sentence-ordinal scheme, where nothing but convention says that sentence 7
in the response is sentence 7 of the request. That same gate does double
duty as an *attribution* rule: a missing or duplicated row is a statement
about the provider's envelope rather than about any unit's content, so it
is charged to its own budget and the innocent units in the same batch flow
on untouched.

The explicit cost, beyond the deferred click-to-pair feature: intra-block
sync is coarser than ideal for very long blocks (a 200-row table, a
long code fence) — the whole block is one sync anchor, so scrolling inside
it doesn't move the partner pane until you cross the block boundary. The
project's own docs note this as a known, accepted rough edge rather than
a solved problem.

## A table may be split for the model; it is never split for the reader

That 200-row table is also where the design's most misreadable rule
lives, and the rule changed shape in 2026-08 without changing its point.

Tables used to be translated strictly as one whole block. They are not
any more: under the shipped default strategy, a table whose estimated
response would blow the run's output ceiling is broken at packing time
into *row windows*, each dispatched, validated, retried and cached as its
own unit (DCR-0026). The interesting question is not whether a table can
be split — it can — but why a table is still never translated
cell-by-cell, and the two are easy to conflate.

A window is a **complete GFM table**: the source header row, the delimiter
row that carries the per-column alignment, and one contiguous run of body
rows in source order. Never an isolated cell, never a headerless
fragment. That is what makes a window translatable at all — the model
sees a table, with its column headers as context, and the per-kind
validator can check column count and shape against a real table rather
than against a fragment that only means something in a context the model
was not shown. Cell-by-cell translation would delete exactly that context:
a bare cell has no header telling the model whether "state" is a US state
or a program state, and no shape for a validator to check.

The second half of the rule is what keeps the sync currency intact.
Windows exist only between packing and merge. Once every window has a
verdict, they are reassembled into one table — window one whole, later
windows minus the header and delimiter rows they carried only as
context — and the merged table is re-inspected against the source block's
column count, alignment, and body-row count before it is accepted. From
regeneration onward there is one block again, with the parent's ID.

So the ID chain described above holds where it matters, with one honest
qualification: on the *wire*, and in the validation report, an id can now
be a window id — the parent block id with a window ordinal appended —
which names nothing in the alignment map and nothing in the DOM. A tool
that assumed every unit id in a report resolves to a rendered anchor was
relying on something that is no longer true. The alignment map and the
`data-sync-id` attributes are unchanged: one row, one anchor, per source
block, exactly as before the splitter existed.

## The corollary: JavaScript never parses Markdown

Once block IDs are the only sync currency, a second design question
follows immediately: who is allowed to *decide* what the blocks are? If
the browser parsed Markdown independently to build its own DOM, it could
disagree with the Rust parser about where one block ends and the next
begins — and the moment it does, the shared ID space the whole sync
mechanism depends on stops meaning the same thing on both sides. So the
rule is unconditional: JavaScript mounts and reacts to already-rendered,
already-ID-annotated content; it never runs a Markdown parser of its own.
Two paths satisfy that rule today — the CLI emits annotated HTML directly,
or (since ADR-0019) the same Rust renderer runs *in* the browser, compiled
to WebAssembly — and both produce byte-identical fragments, because
they're the same renderer either way, not two implementations that have to
be kept in sync by discipline.

Sameness cuts both ways, which is a feature: when the renderer learned to
refuse an alignment row whose byte range it cannot slice, both paths
inherited the refusal in the same edit. Neither one can quietly present a
truncated block under a correct-looking anchor while the other reports the
problem.

## Where the provider boundary fits

A related, separately-decided question is who owns the network call to
the LLM. ADR-0002 keeps the core pipeline HTTP-free: it depends on a
`Translator` trait, not on any concrete provider. This isn't about sync
currency at all — it's a portability and testability decision, made
independently, that happens to compose cleanly with the ID-first design:
because a `TranslationUnit` is already "one block, one ID, one payload,"
any provider that implements the trait automatically inherits the
ID-preservation contract without having to know why it matters.

Since DCR-0029 that argument is demonstrated rather than asserted. There
are two provider crates in the tree — the OpenAI adapter and an Anthropic
one — implementing the same trait against genuinely different HTTP
surfaces: a different auth header, a required protocol-version header, and
an output ceiling the API demands on every request rather than accepting
by omission. None of that reached the core. What a second implementation
did change is worth more than the portability claim itself: it added one
term to the shared error vocabulary. An over-long *request* and a
truncated *answer* are both about tokens, both terminal, and point at
opposite knobs, so collapsing them would have named a remedy that cannot
help — the adapter that met the distinction first is the reason the trait
now has a name for each. That is what a boundary drawn at roughly the
right place looks like in practice: the second implementation costs a
vocabulary addition, not a redesign.

One honest limit: the reference CLI cannot select the second provider. It
depends on the OpenAI crate alone and has no provider flag, because a
`--provider` axis is real design work — an environment surface, key
resolution, an argument-contract change — that DCR-0029 deliberately left
out of its own scope. Reaching the Anthropic adapter today means calling
the library and handing the pipeline your own `Translator`, exactly as
[how to implement a custom Translator provider](../../../how-to/developer/en/implement-a-custom-translator.md)
describes for a provider you write yourself.

## Related

- [Why layered validation and bounded retry/fallback](./validation-retry-fallback-model.md)
- [The `Translator` trait](../../../reference/developer/en/translator-trait.md) —
  the provider contract this section argues about, stated normatively.
- [Alignment-map JSON schema](../../../reference/developer/en/alignment-map-schema.md) —
  the durable shape of the ID chain's last structural link.
- `docs/architecture/README.md` — the full pipeline diagram and component
  table this page's prose summarizes.
- `docs/decisions/0001-block-level-alignment-as-sync-currency.md`,
  `docs/decisions/0019-wasm-demo-layer.md` — the source ADRs.
