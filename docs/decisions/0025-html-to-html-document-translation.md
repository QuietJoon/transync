---
type: ADR
title: HTML documents are a second intake into one pipeline, and semantic kind is orthogonal to source spelling
description: One architectural commitment covering the twelve ratified decisions of the HTML→HTML design — the kind ⊥ spelling split, the transync-html mechanics crate, structural pass-through with content stop, the anchor-less <title>, panes as a sync surface not a fidelity preview, flag-only format routing, <li> as the block, and the deferral of Markdown-island reclassification to a schema-2.x window.
tags: [decision, ADR-0025]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# ADR: HTML documents are a second intake into one pipeline

## Status / Date / Source

- **Status:** accepted.
- **Date:** 2026-09-01 — this record. The decisions it carries were ratified in
  the design session of 2026-08-20; wave 2, the first implementation wave that
  changes the IR, landed its code on **2026-08-24** (`20b78b7`…`ff788f7`). All
  three dates are read from `git log --date=short` over the commits themselves,
  never from a plan's filename: DCR-0033 records the rule, and waves 0 and 1
  both had their landings mis-dated to a plan's writing date before it existed.
- **Ticket:** `490d97`.
- **Source:** `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
  (owner-ratified design spec, §2 for the twelve decisions, §13 for the accepted
  limitations, §10 for the records this ADR owes).
- **Precedent for the shape:** ADR-0018 already carries several decisions as one
  architectural commitment. This does the same, for the same reason — the twelve
  are not independent, and splitting them into twelve records would let a later
  reader adopt one without the constraint that pays for it.

## Context

`transync` translates GFM Markdown documents and renders source and target
side-by-side with block-ID scroll sync. Since ADR-0018 / DCR-0016 it also
translates the *content* of raw-HTML blocks **inside** Markdown documents. What
it cannot do is take an **HTML document** as input: the CLI's preamble sniff
refuses `<!doctype`/`<html`-shaped input with a pointer at this ticket, and
`html_dominance_warning` names "HTML-to-HTML translation is not implemented
(ti 490d97)" as the library-side backstop.

Three facts shape the design:

- **The shipped IR conflates semantic kind with source spelling.**
  `BlockKind::Html { block_type: u8 }` meant both "no semantic classification"
  *and* "spelled as HTML". An HTML document's `<h1>` is semantically a heading;
  calling it `Html` would forfeit section partitioning, heading context, and
  honest wire labels.
- **The HTML mechanics were a `#[doc(hidden)]` module inside `transync-syntax`.**
  The intake, the layer-6 twin, and the pane injector all need its scanner and
  its pairing discipline. Keeping it hidden inside one crate would force either
  re-implementation — a second HTML opinion, forbidden by the same rule that
  forbids a second Markdown parser — or an awkward visibility widening.
- **Everything downstream of the IR is already format-agnostic, or one dispatch
  away from it.** `regen::regenerate` copies gaps and tails verbatim and is
  format-blind; the retry cascade consumes only
  `ReparseFailure { reason, divergent_source_blocks }`; `CacheKey` already
  carries `input_mode`. The feature is large, but the architecture does not
  move.

The commitment is therefore **one pipeline, two intakes** — never a parallel
HTML pipeline.

## Decision

The twelve, in the spec's own order and wording.

**D1 — Scope: one spec, staged waves.** The whole feature is designed in one
document; implementation is staged into eight waves, each with a demonstrable
outcome. *Rejected:* per-area partial specs — the synthesis showed the areas'
seams (the splice policy, the pane list-grouping, the twin's segmentation
projection) are exactly where independent designs diverged, and one document is
the anti-drift instrument.

**D2 — Semantic kind ⊥ spelling.** `BlockKind` stays the semantic vocabulary; a
new per-block `Spelling` says how the source spelled the block; `BlockKind::Html`
narrows to "HTML content with no semantic equivalent" (a `div`, a custom
element). An HTML `<h1>` is `Heading1` + `Spelling::Html`; a Markdown island
stays kind `Html` + `Spelling::Html { Some(block_type) }`. *Rejected:* deriving
spelling from `document format × kind` (true today, but it hard-codes the very
conflation the decision removes, and collapses when D11's deferral ends);
keeping `block_type` on the kind (it is 100 % spelling metadata — its only
consumers were splice normalization and `HtmlSegmentConstraints`).

**D3 — Crates: layer axis, not format axis.** The HTML mechanics move to a new
crate (named by D10); the workspace keeps its layer-based crate axis; the
*format* seam is a module boundary inside `transync-syntax` —
`intake::markdown` (today's `parser`) beside `intake::html`. *Rejected:* a
format-axis crate split (`transync-markdown` / `transync-html-format`): intake
classification names `BlockKind`/`Block`, so a format crate would either depend
on the IR (inverting the layer axis) or duplicate it.

**D4 — Block definition: structural pass-through, content stop, unknown stop.**
Structural and grouping elements (`div`, `section`, `body`, …) pass through —
their markup becomes inter-block gap bytes that `regen` copies verbatim.
Segmentation stops at content elements (`h1`–`h6`, `p`, `table`, `pre`,
`blockquote`, `li` per D9, …), which become leaf blocks of the matching semantic
kind. Unknown and custom elements **stop** as `BlockKind::Html` — the safe
direction: over-stopping yields a coarser block, while under-stopping would leak
text into untranslated gap. *Rejected:* top-level `<body>` children as blocks;
budget-driven block sets and intake descent into oversize elements (block
identity would depend on token budgets). Two recorded amendments to the
decision's literal element list: the PHRASING class (a necessary widening —
without it `Hello <b>world</b>` shatters into three blocks; its consequence is a
named post-implementation review item) and D9's list ruling.

**D5 — Head policy: `<title>` translated, anchor-less; attribute text
untranslated.** The `<title>` is real translatable content and gets a real
block, a real unit, and a real alignment row — but **no DOM anchor**: it is
rendered by browser chrome, not by the pane. Its row ships
`sync_role: "non-sync"`, the shape a thematic break has had since schema 1.0.
**This amends architectural invariant 1** (below). Attribute text (`alt`,
`title`, `placeholder`, `<meta description>`, `og:*`) stays untranslated — an
explicitly accepted limitation with its visible consequence named. *Rejected:*
skipping the head entirely (the title is the single highest-value string on many
pages); anchoring the title (there is nothing in the pane to anchor).

**D6 — Panes are a sync surface, not a fidelity preview.** The bundle's panes
exist to sync blocks, exactly as they do for Markdown. Viewing the real
translated page — head, scripts, styles, layout — is `transync serve` over the
published `out.html`. *Rejected:* panes-as-fidelity-preview / whole-document
annotated panes.

**D7 — Entity drift accepted.** Translated segments lose named-entity spelling
(`&nbsp;` → U+00A0, `&copy;` → ©); untouched segments and identity-skipped
echoes keep their source bytes exactly. Already test-pinned since DCR-0016; this
feature adds population, not policy.

**D8 — CLI: explicit `--input-format markdown|html`; the sniff stays a refusal,
never a router.** Routing is flag-only; the operator asserts the format. The
preamble sniff continues to refuse HTML-shaped input on Markdown runs (with an
updated pointer message) and is not consulted on HTML runs. *Rejected:*
sniff-as-router; a reverse sniff on HTML runs (a body fragment is legitimately
accepted HTML and no preamble check can recognize one). Final ruling on the
decision's open half: `--allow-html-input` is **kept**, narrowed in
documentation only to its real case — "this is Markdown that opens with an HTML
island" — plus a new **exit-1** argument-conflict rule against
`--input-format html`.

**D9 — `<li>` is the block, not `<ul>`.** The owner ratified the synthesis's
recommendation; this **deviates from D4's literal element list** (`ul`/`ol`
appeared among the stop elements) and the deviation is recorded here. Rationale:
(a) one block-granularity convention across both intakes — the shipped Markdown
model is leaf `ListItem` blocks, and `align::sync_role_for`'s comment,
`render::render_list_group`, `wrapper_element_for(ListItem) => "li"` and
`walk::normalize_top_level`'s prefix collapse all already assume item-level
anchors; (b) per-item fallback instead of whole-list fallback; (c) it dissolves
the oversize-list problem without the intake descent D4 rejected — a 400-item
list is 400 units, not one 200 KB unit needing a splitter that does not exist.
Accepted cost: `<ul>`'s own tags become gap bytes (which `regen` preserves
verbatim), and there is one anchor per item rather than per list. *Rejected:*
`<ul>` as one block.

**D10 — Crate name: `transync-html`.** The owner chose this over the synthesis's
recommendation `transync-htmlseg`, on the reasoning that "segment" now
under-describes a crate that also owns tokenization (`scan_tags`), the pairing
discipline and element extents — and that over-description ages better than
under-description. *Also rejected:* `transync-html-core` (`-core` already means
"the pipeline" in this workspace, so the name would claim pipeline status for a
mechanics crate); `transync-lolhtml` / `transync-rewriter` (they name the
dependency or the implementation, not the capability). **Required consequence:**
the crate's doc comment states that structural intake — classification,
`BlockKind` assignment, ids — lives in `transync-syntax`, *not* there;
`transync-html` is the mechanics layer under it.

**D11 — Markdown-island reclassification is deferred.** See its own section
below.

**D12 — Version: v0.5.0.** The four breaking-by-policy IR changes
(`Block.spelling`, `Document.format`, the `BlockKind::Html` narrowing,
`BlockKind::Title`) ride one sanctioned v0.5.0 window, batched. The owner
considered v0.4.1 with the measured facts in hand — neither known consumer
breaks (`dynwebserver` has `_ =>` wildcard arms and never constructs `Block`;
`resp-translator` never names `BlockKind`; nothing is published to crates.io
yet) — and **chose v0.5.0 anyway**: a policy exception is more expensive later
than a version number is now, and `CodeBlock.fenced` rode a sanctioned window
four days earlier for a strictly smaller change. *Rejected:* v0.4.1 — it would
establish that breaking-by-policy changes can ride patch releases whenever no
consumer is *measured* to break, which erodes the policy exactly when it
matters.

Three further confirmations were recorded with the decisions: `--allow-html-input`
kept and narrowed with the exit-1 conflict (folded into D8); **neighbor snippets
stay verbatim source excerpts for both formats** — only heading context and
`document_title` project through `extract`, which keeps the pinned test
`neighbor_summaries_stay_verbatim_source` green; and the PHRASING widening's
consequence is **accepted for now** and is a named post-implementation review
item, not a settled matter.

## Rejected alternatives

Each with the ground it was rejected on, because a rejected option with no
recorded reason gets re-proposed.

- **Top-level `<body>` children as blocks.** A wrapper-heavy page — the common
  case for anything built by a framework — becomes one giant block. Block
  identity would then track the page's div nesting rather than its content.
- **A format-axis crate split** (`transync-markdown` / `transync-html-format`).
  Intake classification names `BlockKind` and `Block`, so a format crate either
  depends on the IR — inverting the workspace's layer axis — or duplicates the
  IR, which is the same "second opinion" failure the single-parser rule exists
  to prevent.
- **Panes as a fidelity preview.** Four separate grounds: it breaks
  `contracts.md` §4a's direct-child invariant; it corrupts `offsetTop` geometry
  through source-controlled `style` attributes; it re-opens OI-0035's attack
  surface; and it delivers exactly the capability D6 declines to deliver.
  `transync serve` over `out.html` is the real page.
- **Sniff-as-router.** Content sniffing was put out of scope by ti `13e145`, and
  silent format selection is precisely the class of guess ADR-0017's refusals
  exist to prevent. The sniff stays a refusal.
- **An HTML→Markdown round-trip.** Converting the input to Markdown, translating
  it, and converting back discards every structural fact the source carried and
  cannot promise byte-identity anywhere. It also re-introduces a second opinion
  about HTML, in the conversion direction.

## Accepted limitations

Each is a knowing trade, with the visible consequence spelled out so a later
reader does not mistake it for an oversight.

1. **Attribute text is untranslated** (D5): `alt`, `title`, `placeholder`,
   `<meta description>` and `og:*` stay source-language. **Visible consequence:
   a shared link previews in the source language.** A future policy may revisit
   it; nothing in this design forecloses that.
2. **Entity-spelling drift in genuinely translated segments** (D7): translated
   text loses named and numeric entity forms. Untouched and identity-skipped
   segments keep their source bytes exactly.
3. **Panes carry no page layout** (D6): no styles, no scripts, no page chrome in
   the sync surface.
4. **`<ul>`/`<ol>` markup is gap, and the anchor is per item** (D9): the list's
   own tags never translate — they contain no text — and carry no anchor of
   their own.
5. **A custom element mid-sentence splits the sentence** — the PHRASING
   widening's consequence. Accepted *for now*, and carried as a named
   post-implementation review item rather than as a settled matter.
6. **The `<title>` has no anchor** (D5): translated, aligned, honest `non-sync`
   row, no pane presence.
7. **The layer-6 twin's blind spots:** swapped same-skeleton texts pass (parity
   with `reparse_full`); attribute values are untokenized; the scanner is one
   self-consistent opinion, not a general parser.
8. **OI-0035's in-pane-impostor residual:** a *listed* id preceding the genuine
   anchor defeats the engine layer alone, which is why the render-side strip is
   the other half rather than a nicety (DCR-0033).
9. **Markdown islands stay `BlockKind::Html`** (D11) — the honest cost of the
   corpus-stability acceptance criterion.

## D11's deferral, and its pin

**The Markdown intake keeps emitting `BlockKind::Html` for raw-HTML islands.**
D2's narrowed meaning — "HTML content with no semantic equivalent" — binds the
**HTML intake** only.

The pin is mechanical, not aesthetic. Reclassifying an island (`html-0007` →
`t-0007`) **moves block ids**, because id prefixes come from
`BlockKind::id_code()`. Moving a block id moves with it the alignment row, the
DOM anchor, and the `block_kind` cache axis — which violates the acceptance
criterion this whole feature is gated on: *the Markdown path stays byte-identical
over the existing corpus.*

It is defensible on its own terms too: a `<table>` island has a semantic
equivalent only if someone parses it, and the Markdown intake deliberately does
not.

**Revisit in a schema-2.x-shaped window, not before** — that is, in a window
where moving ids on the wire is itself the sanctioned change, rather than
collateral damage from one.

## The amendment to architectural invariant 1

D5 gives one block that is translated, aligned, and deliberately *not* anchored.
Invariant 1 as written promised a DOM anchor for every block, so the invariant
is amended rather than quietly contradicted. The replacement wording, adopted
verbatim from the design spec, and carried by `CLAUDE.md` as of this wave:

> **1. Block ID is the only sync currency.** Never use heading text, slugs, line
> numbers, or scroll percentage. The same `block_id` flows: Rust IR → LLM
> request → LLM response → regenerated document → rendered DOM anchors → JS sync
> engine. The final leg is conditional on visibility, not on kind: every block
> rides the chain unbroken through the regenerated document, and every block
> displayed in the page gets a DOM anchor — a block that is not page content
> (today exactly one: an HTML document's `<title>`, translated but rendered by
> the browser chrome rather than the pane) carries a `sync_role: non-sync`
> alignment row instead of an anchor, the same shape a thematic break has had
> since schema 1.0. No block is ever anchored by anything other than its
> `block_id`, and the engine takes its anchor set from validated alignment rows,
> never from whatever the DOM happens to carry.

Two things the amendment does **not** loosen: an anchor is still keyed on
`block_id` and nothing else, and the engine's anchor set still comes from
validated alignment rows rather than from the DOM (the OI-0035 closure, DCR-0033).
The same edit sweeps `CLAUDE.md`'s remaining promises of Markdown as the
*output* format — the field has been `translated_document` since v0.4.0, and an
HTML run returns HTML.

## The name-continuity note, in both directions

D10 chose `transync-html` over `transync-htmlseg`, and the rejected name was
argued partly on **grep continuity**. That cost has to be bought back here,
because after wave 0 the identifier `htmlseg` exists **nowhere in the tree**
while several shipped records still name it.

**Backward — what `htmlseg` means now.** The module formerly called `htmlseg`
inside `transync-syntax` is the crate now called **`transync-html`**. ADR-0018,
ADR-0003, `docs/project/stub-manifest.md`, `docs/project/status.md`,
`docs/project/open-issues-archive.md` and
`docs/implementation/implementation-slice-checklists.md` still say `htmlseg`.
Those records are **dated and are not rewritten** — this repository's records are
append-only — so this sentence is the mapping, and it lives here because ADR-0025
is where a reader looking up "htmlseg" will land.

**Forward — what `transync-html` is not.** The name reads as though it owns HTML
end to end. It does not: structural intake — classification, `BlockKind`
assignment, block ids — lives in `transync-syntax`, and `transync-html` is the
mechanics layer *under* it (tag scanning, the pairing discipline, element
extents, extract/splice, fragment balancing). That half of the note lives in the
crate's own doc comment, where a contributor reading the crate will hit it.

Two notes, because the confusion runs in both directions.

## Consequences

- **One pipeline, two intakes.** Batching, prompting, caching, retry/fallback,
  alignment and the sync engine are shared; only the intake and the layer-6
  reparse gate dispatch on format. There is no parallel HTML pipeline to keep in
  step, which is the property this ADR is protecting.
- **Four breaking-by-policy IR changes ride one sanctioned v0.5.0 window**
  (D12), batched and recorded the way `CodeBlock.fenced` was: a `contracts.md`
  §0 prose paragraph, a §1 stability bullet, and a CHANGELOG BREAKING entry.
  Wave 2 (DCR-0034) landed them. The window's *total* is larger than wave 2's
  four — `transync-openai`'s unreachable `ProviderError::RateLimited` was also
  removed under it (`8d4efda`, DCR-0040 §3) — and the two counts must not be
  conflated.
- **The layer-6 twin becomes a hard precondition for any HTML run.** An HTML
  document cannot be validated by Markdown reparse, so until the format-appropriate
  full-document gate exists, an HTML translation would ship with its last
  validation layer missing. That is why the twin is scheduled ahead of the waves
  that make an HTML run demonstrable, and why `Document.format` lands as a fact
  before any consumer branches on it.
- **`Spelling` and `SourceFormat` are engine-tier types today.** They live in
  `transync-syntax::id`, the facade re-exports neither, and each gets its
  `contracts.md` §0 row in the window that exports it — not before.

## Related

- `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` — the
  ratified design spec this ADR records.
- `docs/project/design-change-records/DCR-0032-transync-html-crate-extraction.md`
  — D3/D10 as landed (wave 0).
- `docs/project/design-change-records/DCR-0033-oi0035-anchor-trust-both-layers.md`
  — OI-0035 closed at both layers (wave 1), the precondition D6's pane ruling
  leans on.
- `docs/project/design-change-records/DCR-0034-ir-semantic-kind-and-spelling-split.md`
  — D2/D5/D12 as landed (wave 2).
- `docs/decisions/0018-html-content-translation-via-segment-extraction.md` — the
  multi-decision precedent, and the origin of the segment-extraction contract.
- `docs/decisions/0017-batch-terminal-failures-abort-the-run.md` — the refusal
  posture D8 keeps rather than replaces with a sniff.
- `docs/architecture/contracts.md` §0/§1 — where the four surface changes are
  recorded.
