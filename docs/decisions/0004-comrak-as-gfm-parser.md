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

- **One configuration.** `comrak_options()` (`intake::markdown::options`, in `transync-syntax`) is the one Comrak configuration. Every Comrak parse in the workspace goes through it, so each sees the AST shape block IDs were assigned against.
- **Parse.** `intake::markdown::parse` wraps Comrak's `parse_document` to produce a `Document` IR that adds:
  - `block_id` per sync-relevant block (assigned in source order; ADR-0005 owns the format).
  - `source_range` per block (byte offsets recovered from Comrak's `start_line` / `start_column` / source slicing).
  - `source_hash` per block (for cache keys + integrity checks).
- **Regeneration splices source bytes; no Markdown serializer is used.** `regen::regenerate` walks the **top-level** blocks in source order, copies the text *between* blocks verbatim, and at each block's recorded source range splices either the accepted translated payload or — when the block is absent from the accepted map — that block's own source bytes. Comrak's CommonMark/GFM serializer (`format_commonmark`) is called **nowhere** in the workspace; the only Comrak serializer in use is `format_html`, in `render`. Comrak still does the *reading* on this path: the code-block extractor reparses a fragment to recover its body and info string, and validation reparses both the fragment and the full regenerated document.
- **The one content-aware path for a Markdown block** is the code block (invariant 4): `regenerate_code_block` extracts the payload's body and `regenerate_code_fence` re-wraps it with a fence at least one character longer than the longest run of that fence character inside the body, switching backticks to tildes when the info string itself contains a backtick; an accepted indented code block therefore comes back fenced (DCR-0031). A table takes the plain splice — an oversize table's row windows are reassembled into one table by `regenerate_table` before regeneration (DCR-0026). An HTML-spelled block splices through `transync_html::splice`, not Comrak (ADR-0018).
- **Render.** The renderer parses each pane once with Comrak and formats each block's paired AST node with `format_html`. `contracts.md` §4 owns the sync attribute set and the wrapper element for each block kind (ADR-0006, ADR-0007).
- **Crate home and dependency.** The Markdown intake (`intake::markdown`), `regen` and `render` live in **`crates/transync-syntax`** (DCR-0017), not `crates/transync` (which declares none of them and only re-exports); `validate`, which drives the fragment and full-document reparses, stays in `transync-core` and calls into the intake from there. The dependency is declared once in the workspace root as `comrak = { version = "0.27", default-features = false }` — no extra feature is enabled; the AST arena is Comrak's own, reached through `comrak::Arena` + `parse_document`, not a separate `arena_tree` feature.
- **Scope.** This decision covers Markdown source. Declared-HTML input is read by the `intake::html` scanner walk, not by Comrak (ADR-0025).

## Consequences

- **Good:** One parser and one `comrak_options()` serve parse → render → fragment and full-document reparse, with regeneration splicing source bytes in between. SCN-14 (full-document reparse) is straightforward.
- **Good:** AST visitors cleanly localize the ID assignment, the renderer attribute injection, and the validator's per-kind checks.
- **Good:** Comrak handles GFM tables, task lists, strikethrough, autolinks out of the box. No custom GFM extensions to maintain.
- **Good:** Comrak's canonical Markdown formatting never reaches the output: everything the translation did not touch is copied byte-for-byte, so a fully-fallback document is byte-identical to its source — the property DCR-0002 / DCR-0004 rely on to skip the verifying reparse.
- **Bad:** Comrak's AST is more allocation-heavy than pulldown-cmark's event stream. Not a concern at MVP scale.
- **Bad:** The splice is top-level only: a nested block (a paragraph inside a blockquote, an item inside a list) is not replaced individually — its whole top-level container is the unit. The AST-splicing follow-up is backlog `nested-block-ast-splicing` (blocked); whether it is still owed is undecided (ti `9b18ba45`).

## History

The replaced 2026-05-01 sections and the 2026-08-06 amendment are kept verbatim in [`archive/adr-amendments.md`](archive/adr-amendments.md), under *ADR-0004 — Comrak as the GFM parser*.

- 2026-05-01 (ADR-0004): decided — Comrak over pulldown-cmark and a hand-written parser, from the brainstorming defaults.
- 2026-05-04 (DCR-0001 / ADR-0007): the wrapper `<div>` around tables and code blocks became the documented renderer contract.
- 2026-08-04 (DCR-0017): the parser, `regen` and `render` moved to `crates/transync-syntax`; the renderer became one Comrak parse per pane.
- 2026-08-06 (Review-0001 `R0001-0050`): amendment — regeneration splices source bytes and never used a Markdown serializer; comrak 0.27, no extra feature.
- 2026-08-09 (DCR-0026): `regenerate_table` became the row-window reassembler; the amendment had recorded it as an identity placeholder.
- 2026-08-16 (DCR-0031): an accepted indented code block is re-emitted fenced through `regenerate_code_block`.
- 2026-09-07 (DCR-0053): the `parser` module became `intake::markdown`.
