---
type: ADR
title: BlockId format `<kind>-<NNNN>`
description: Block IDs are kind-prefixed, globally-sequential, zero-padded decimals; the source hash travels alongside on the IR rather than being baked into the ID string.
tags: [decision, ADR-0005]
status: active
---

# ADR 0005: BlockId format `<kind>-<NNNN>`

## Context and Problem Statement

`BlockId` is the single sync currency through the entire pipeline (per ADR-0001). Its format must be:
- **Stable** — identical across the pipeline for a given source block, byte-for-byte.
- **Deterministic** — derived from source-order traversal of the AST so that two parses of the same source produce identical IDs.
- **Debuggable** — a human reading a log or a JSON alignment map should be able to identify the block kind without cross-referencing.
- **Compact** — IDs land in HTML attributes and JSON; longer is worse.
- **Collision-free within a document** — sequential numbering is sufficient because the document is static (NG4 of `references/draft.md`).

The Phase 1 brainstorming spec listed this as deferred to Phase 2 (intake.md). `references/draft.md` §12 outlines three styles: sequential (`b-000042`), path-based (`h2-3/p-2`), or hybrid (`b-000042-p-a91f2c`).

## Decision Drivers

- IDs are read by humans in JSON alignment maps and in HTML attribute inspectors.
- IDs are written into `data-sync-id` attributes — every extra character lands in HTML output.
- The document is static, so cross-edit ID survival is a non-goal.
- Source hash already exists as a separate integrity field on the IR; baking the hash into the ID string would duplicate it.
- Path-based IDs (`h2-3/p-2`) carry hierarchy information, but that information is already available through `parent_id` and `section_path` in the alignment map.

## Considered Options

1. **Pure sequential.** `b-0042`. Compact; no kind hint; least debuggable.
2. **Kind-prefixed sequential.** `p-0042`, `h2-0001`, `t-0007`. Compact; kind visible at a glance; deterministic.
3. **Path-based.** `h2-3/p-2`. Encodes hierarchy in the ID; longer; complicates uniqueness when nested lists deepen; requires `/` escaping in HTML attributes.
4. **Hybrid with hash.** `b-0042-p-a91f2c`. Adds integrity into the ID at the cost of length; duplicates information already on the IR.

## Decision Outcome

We chose **option 2**, kind-prefixed sequential.

```
<kind>-<NNNN>
```

- `<kind>` ∈ `{h1, h2, h3, h4, h5, h6, p, t, c, li, q, hr, img}`. Inline runs are not assigned IDs.
- `<NNNN>` is a zero-padded decimal sequence assigned by source-order AST traversal, starting at `0001`. Zero padding is to four digits; documents with more than 9999 sync-relevant blocks bump to five digits and beyond on demand (no separate format flag needed — readers parse the trailing integer).
- Numbers are not reused within a document; each `BlockKind` increments its own counter (so `p-0001` and `h1-0001` can coexist) — wait, no: see below.

After review of the validator surface, we will use a **single global counter across all kinds**, not per-kind counters. This guarantees that ID-set equality checks in the validator can detect a missing unit even if the wrong kind was returned. So `h1-0001` followed by `p-0002` followed by `t-0003` is the canonical form.

The source hash travels alongside as a separate `source_hash: u64` field on the IR and on each `TranslationUnit`. It is **not** part of the ID string.

Status: Decided 2026-05-01 (Phase 2). Shipped — `<kind>-<NNNN>` IDs flow end-to-end (IR → LLM contract → alignment map → `data-sync-id` DOM anchors).

### Implementation

- `transync::id::assign_block_ids` walks the Comrak AST in source order; sync-relevant nodes receive an ID with the appropriate kind prefix and the next sequence number.
- The AST walker uses the same traversal order as the renderer so DOM order matches `data-order`.
- `transync::id::source_hash_block` computes SipHash-1-3 over the canonical block bytes (the source slice between the block's `source_range`); this is the integrity hash, not part of the ID.

## Consequences

- **Good:** A human reading `data-sync-id="t-0007"` immediately knows it's a table.
- **Good:** Validator can do simple set-equality on `unit_id`s.
- **Good:** Length is bounded: prefix ≤ 3 chars, dash, four digits — typically 7–8 characters.
- **Good:** No HTML attribute escaping needed (no `/` or `:`).
- **Bad:** ID does not encode hierarchy. Recovered via `parent_id` and `section_path` in `AlignmentMap`. Acceptable.
- **Bad:** ID does not survive source edits. Documented as a non-goal (NG4).
