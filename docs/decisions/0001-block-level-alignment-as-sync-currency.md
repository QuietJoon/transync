---
type: ADR
title: Block-level alignment as the only sync currency
description: Block IDs are the sole sync currency end-to-end (Rust IR through LLM contract, alignment map, DOM anchors, JS engine); heading text, slugs, line numbers, and scroll percentage are never used.
tags: [decision, ADR-0001]
status: active
---

# ADR 0001: Block-level alignment as the only sync currency

## Context and Problem Statement

The library must keep two rendered Markdown panes synchronized as the user scrolls. The two reference projects in this workspace took different approaches:

- `LLM-API/LLM-Trans` segments documents at sentence level, pairs sentences by ordinal index, and parses both source and translated Markdown in the browser using `marked`. Because the source and translated panes are parsed independently in the browser, parser divergence between source and target is silent — and ordinal pairing breaks the moment the LLM merges, splits, or reorders sentences.
- `references/draft.md` mandates block-level synchronization (NG3, FR-7, FR-8) with stable IDs that flow through the Rust IR, the LLM contract, and the rendered DOM.

We must decide which approach `transync` adopts, because the choice shapes the data model, the LLM contract, the validation surface, and the JS sync engine.

## Decision Drivers

- The LLM contract must be enforceable: structural drift (column-count change, list-topology change, missing IDs) must be detected and either retried or fallen back, not rendered.
- The JS sync engine should not need its own Markdown parser. Anchor IDs must be the only currency.
- The MVP must ship with a tractable validation surface. Sentence-level sync would require sentence segmentation rules in Rust *and* JS that agree at the boundary.
- The user explicitly confirmed that the LLM-Trans samples are use-case references, not the algorithmic spec.

## Considered Options

1. **Pure block-level synchronization.** The Rust IR assigns one stable ID per sync-relevant block (heading, paragraph, table, code block, list item, blockquote). All sync currency — LLM contract, alignment map, DOM anchors — uses the same IDs.
2. **Block-level outer, sentence-level inner.** Block IDs are the only thing the LLM and Rust IR see; sentences exist only as render-time sub-anchors inside paragraphs for a click-to-pair UX. Translation requests still send whole paragraphs; sentences are derived after translation.
3. **Both as first-class.** Sentence-level segments flow all the way through the LLM contract (id + order, like LLM-Trans), with block-level metadata alongside. Doubles the surface area, doubles the validation cases, contradicts NG3 of the design draft.

## Decision Outcome

We chose **option 1**. The user selected it during brainstorming Q2 with the rationale that the LLM-Trans samples are reference UX, not algorithmic guidance. Block-level alignment honors the design draft's structural contract, eliminates the parser-divergence trap, and produces a tractable validation surface where each unit has exactly one ID and exactly one block-kind constraint.

Status: Decided 2026-05-01 (brainstorming Q2). Shipped — block IDs are the sole sync currency end-to-end (Rust IR → LLM contract → alignment map → `data-sync-id` DOM anchors → JS sync engine); the sync engine was later amended by DCR-0008 (see Implementation).

### Implementation

- The Rust IR (`Document`) assigns one `BlockId` per sync-relevant block. Inline sentences and runs do not get IDs.
- The `TranslationUnit` data type carries the same `BlockId` as `unit_id`.
- The annotated HTML renderer emits exactly one `data-sync-id` attribute per sync-relevant block.
- The JS sync engine works against `[data-sync-id]` anchors only. No sentence-level lookups. *(Amended by DCR-0008: the originally-specified `IntersectionObserver` + intersection-ratio + viewport-center + hysteresis design was replaced by the shipped scroll-listener engine — RAF-coalesced scroll events, a reference-line active-block pick with intra-block progress fraction, per-frame lerp toward the partner target, and a per-pane programmatic-scroll lock. The authoritative algorithm description is `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`. The block-ID-as-only-currency principle this ADR decides is unchanged.)*
- The alignment map records `source_block_id` and `target_block_id` (identical for static documents); per-unit `fallback_status` rides alongside.

## Consequences

- **Good:** One ID currency end-to-end. Validation has a single surface per block kind. The JS sync engine is parser-free.
- **Good:** Tables and code blocks are first-class units, not bags of sentences — matches the draft's whole-block translation policy.
- **Good:** No need to reconcile Rust and JS sentence-segmentation rules.
- **Bad:** Sentence-click-to-pair (a feature LLM-Trans has) is unavailable in MVP. Acceptable; the user explicitly de-prioritized this.
- **Bad:** Within very long blocks (large tables, long code blocks), intra-block sync is coarser than ideal. Mitigated post-MVP by optional intra-block progress (draft §18.2).

## Amendment (2026-08-09, ticket `d3acc3`) — ID identity is normative, not incidental

The Implementation note above records that `source_block_id` and
`target_block_id` are "identical for static documents". That was written as an
observation about the emitter. It is now a **stated invariant of alignment
schema 1.x**, and the consumer side is written against it:

- The reference JS engine pairs the two panes by **identical `data-sync-id`**.
  It always did (`handleScroll` looks the active block's own id up in the
  partner map); what changed is that this is the contract rather than an
  implementation detail one could read either way.
- The map's `source_block_id` / `target_block_id` indirection is therefore
  **RESERVED** — in the same sense as `parent_id` and the `child-only`
  `sync_role` — for a future revision in which the two documents' block sets
  may genuinely diverge. It stays in the wire format; nothing routes through
  it today, because under this ADR there is nothing for it to express.
- A schema-1.x map whose row contradicts the identity is refused by the engine
  rather than mounted. Review 0003's `R0003-0002` found the two consumer code
  paths disagreeing about it — the map/DOM drift warning resolved
  `target_block_id` while the scroll handler resolved `source_block_id`, so
  such a map warned correctly and then could not sync in either direction.
  Making the identity normative is what makes one of those two readings the
  right one; refusing the map is what keeps the other from being reached.

The block-ID-as-only-currency decision itself is unchanged; this names which
id, on which side, is that currency. The consumer contract lives in
`docs/architecture/contracts.md` §3.
