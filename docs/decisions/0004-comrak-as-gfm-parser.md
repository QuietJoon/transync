---
type: ADR
title: Comrak as the GFM parser
description: Comrak's AST-oriented GFM parser, with CommonMark/GFM and HTML serializers, is the single parser shared across parse, per-kind validation, fragment/full reparse, and render.
tags: [decision, ADR-0004]
status: active
---

# ADR 0004: Comrak as the GFM parser

## Context and Problem Statement

The library needs a Rust GFM parser that:
- Produces an AST or equivalent IR with stable structural access (block path, parent, children).
- Supports GFM tables, fenced code blocks with info strings, task list markers, strikethrough, autolinks.
- Lets us record source ranges for each block (byte or line/column).
- Reserializes back to Markdown for the regeneration step.

`references/draft.md` §4.4 names two candidates: Comrak and pulldown-cmark.

## Decision Drivers

- We need block-level identity that survives parse → mutate → reserialize. An AST-shaped parser supports this naturally; an event stream forces us to bookkeep the AST ourselves.
- We need to reserialize translated Markdown back. Comrak ships an HTML and a CommonMark/GFM serializer; pulldown-cmark only emits HTML.
- We need to render annotated HTML with our own attributes. Both parsers can do this — Comrak via AST visitors, pulldown-cmark via event interception.
- Performance is not a primary driver at MVP scale (single-document translation, not bulk indexing).

## Considered Options

1. **Comrak.** AST-oriented, GFM-compatible, HTML + CommonMark/GFM serializers, active maintenance.
2. **pulldown-cmark.** Event-based, GFM-compatible, fast, but requires us to re-build an AST for ID assignment and to write a custom serializer for Markdown regeneration.
3. **A hand-written GFM parser.** Rejected on scope grounds.

## Decision Outcome

We chose **option 1**. The reasoning is in `references/draft.md` §4.4: an AST-oriented parser is preferable when block IDs, source ranges, validation, and regeneration need structural access. All four of those concerns are core to `transync`.

Status: Decided 2026-05-01 (brainstorming defaults). Shipped — Comrak is the sole GFM parser across parse, per-kind validation, fragment/full reparse, and render (one shared `comrak_options()`).

### Implementation

- `crates/transync` depends on `comrak` (latest stable) with default features plus `arena_tree` for AST manipulation.
- The parser module wraps Comrak's `parse_document` to produce a `Document` IR that adds:
  - `block_id` per sync-relevant block (assigned in source order; format finalized Phase 2).
  - `source_range` per block (byte offsets recovered from Comrak's `start_line` / `start_column` / source slicing).
  - `source_hash` per block (for cache keys + integrity checks).
- The regenerator uses Comrak's GFM serializer for non-mutated subtrees and a custom path for mutated table cells / code-block bodies (where translated content replaces source content).
- The renderer extends Comrak's HTML renderer to emit `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback` attributes on the `<section>` / `<table>` / `<pre>` / `<blockquote>` / `<li>` wrapper for each sync-relevant block.

## Consequences

- **Good:** One parser handles parse → mutate → reserialize → render → reparse. SCN-14 (full-document reparse) is straightforward.
- **Good:** AST visitors cleanly localize the ID assignment, the renderer attribute injection, and the validator's per-kind checks.
- **Good:** Comrak handles GFM tables, task lists, strikethrough, autolinks out of the box. No custom GFM extensions to maintain.
- **Bad:** Comrak's AST is more allocation-heavy than pulldown-cmark's event stream. Not a concern at MVP scale.
- **Bad:** Comrak's serializer emits *its* canonical Markdown formatting, not the source's exact whitespace. Acceptable per draft NG1 (semantic preservation, not byte-for-byte).

## Amendment (2026-08-06) — regeneration splices source bytes; no Markdown serializer is used

*Appended, not a rewrite. The decision stands: Comrak is the sole GFM parser,
and one shared `comrak_options()` serves parse, per-kind validation, fragment
and full reparse, and render. What is corrected is the "Implementation"
bullet claiming the regenerator reserializes through Comrak's GFM serializer —
Review-0001 `R0001-0050`. It never has.*

- **How regeneration actually works.** `regen::regenerate` walks the
  **top-level** blocks in source order, copies the text *between* blocks
  verbatim, and at each block's recorded source range splices either the
  accepted translated payload or — when the block is absent from the accepted
  map — that block's own source bytes. Comrak's CommonMark/GFM serializer
  (`format_commonmark`) is called **nowhere** in the workspace; the only
  Comrak serializer in use is `format_html`, in `render`. Comrak still does
  the *reading* on this path: the code-block extractor reparses a fragment to
  recover its body and info string, and validation reparses both the fragment
  and the full regenerated document.
- **The one content-aware path** is the fenced code block (invariant 4): the
  payload's body is extracted, then re-wrapped by `regenerate_code_fence` with
  a fence at least one character longer than the longest run of that fence
  character inside the body, switching backticks to tildes when the info
  string itself contains a backtick. `regenerate_table` exists but is the
  identity function — the row-window splitter it reserves room for is
  deferred, and whole-block tables need no reassembly.
- **What that buys, and what it costs.** The "Bad" consequence above about
  canonical formatting does not bite: everything the translation did not touch
  is copied byte-for-byte, so a fully-fallback document is byte-identical to
  its source — the property DCR-0002 / DCR-0004 rely on to skip the verifying
  reparse. The cost is the other side of the same coin: splicing is top-level
  only, so a nested block (a paragraph inside a blockquote, an item inside a
  list) is not replaced individually — its whole top-level container is the
  unit. `regen`'s module docs record the AST-splicing follow-up.
- **Crate home and dependency.** Since DCR-0017 `parser`, `regen` and `render`
  live in **`crates/transync-syntax`**, not `crates/transync` (which declares
  nothing and only re-exports); `validate`, which drives the fragment and
  full-document reparses, stays in `transync-core` and calls into the parser
  from there. The dependency is declared once in the workspace root as
  `comrak = { version = "0.27", default-features = false }` — no extra feature
  is enabled; the AST arena is Comrak's own, reached through
  `comrak::Arena` + `parse_document`, not a separate `arena_tree` feature.
