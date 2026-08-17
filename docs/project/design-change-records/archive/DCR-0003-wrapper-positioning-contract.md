# DCR-0003: Annotated-HTML wrapper positioning contract

> Archived 2026-07-11. Reason: change fully absorbed — affected ADRs/contracts in sync, migration/follow-up landed; kept for audit.

- **Date:** 2026-05-04
- **Source:** Review 0005, Issue R0005-0001 (external consumer report from `resp-translator`) (review archived and removed)
- **Affected ADRs:** none directly. Documents an explicit contract on `crates/transync/src/render.rs`'s `<main>` wrapper that was previously implicit. `docs/architecture/contracts.md` §4a is the new authoritative location.

## What Changed

The annotated-HTML renderer emits a single `<main>` wrapper around every block as direct children. Until this DCR, the wrapper's positioning behavior was undocumented — consumers using `block.offsetTop`-based scroll math hit a silent layout regression when neither the wrapper nor their pane carried `position: relative`, since `offsetTop` walked up to `<body>`.

This change documents the contract explicitly:

1. The wrapper structure is now described in `contracts.md` §4a (single outer `<main>`; blocks as direct children in source order; no `id`/`class`/inline style on the wrapper).
2. Consumers using `offsetTop`-based scroll math MUST set `position: relative` (or any non-static value) on their scrollable pane element. The contract names the precondition explicitly so consumers can satisfy it without trial-and-error debugging.
3. The renderer intentionally does NOT stamp `position: relative` on `<main>` itself. Doing so would make `<main>` the offsetParent (since it's the closer positioned ancestor), and any padding the consumer applies to the pane would silently shift `offsetTop` by that padding — replacing the original "off by hundreds" bug with a less obvious "off by 12px" bug.
4. The reference `web/js/sync.js` engine documents the precondition in its `mountSync` doc-block and inline scroll-math comments.

## Why

The reviewer (resp-translator integration team) hit this directly: 32-block document, no `position:relative` anywhere on the path from block to scroll container, `offsetTop` walked to `<body>`, partner pane jumped ~800 px on first scroll. The recommended fix (#2 in the report — stamp `position:relative` on the wrapper) papers over the symptom but introduces a new failure mode for consumers who already set `position:relative` on their pane (they would lose the offsetParent control they'd already engineered, since the wrapper would now win as the closer positioned ancestor).

Documenting the precondition is the honest API contract: `offsetTop`-based math depends on offsetParent semantics; transync names the offsetParent the consumer needs and stays out of the way.

## Affected Areas

- `docs/architecture/contracts.md` §4a — new subsection.
- `crates/transync/src/render.rs` — module-level doc explains the precondition; `FRAGMENT_OPEN` retains plain `<main>\n`.
- `web/js/sync.js` — `mountSync` doc-block names the precondition; inline comments at the two `offsetTop` read sites point to §4a instead of "see HTML demos".
- `crates/transync-cli/web/sync.js` — kept byte-identical with workspace `sync.js` (verified by `tests/sync_js_drift.rs`).

## Migration / Follow-up

- Existing consumers (e.g. `resp-translator`) need a one-line CSS change on their pane element. There is no API or output-format change.
- The reviewer's optional recommendation #3 (drop the `<main>` wrapper) remains an option for a future minor version if the wrapper element proves redundant. Not pursued here.
