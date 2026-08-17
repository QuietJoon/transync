# DCR-0001: Renderer wrapper-div for tables and code blocks

> Archived 2026-07-11. Reason: change fully absorbed — affected ADRs/contracts in sync, migration/follow-up landed; kept for audit.

- **Date:** 2026-05-04
- **Source:** Review 0001, Issue R0001-0024 (review archived and removed)
- **Affected ADRs:** docs/decisions/0007-renderer-wrapper-div-for-tables-and-code.md (new in this session)

## What Changed

The HTML-attributes contract in `docs/architecture/contracts.md` §4 originally said the sync-id attributes (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`) live on the semantic outer element of each block — i.e. `<table data-sync-id=…>` and `<pre data-sync-id=…>`. The shipping renderer instead wraps tables and code blocks in a transparent `<div data-sync-id=…><table>…</table></div>` / `<div data-sync-id=…><pre><code>…</code></pre></div>`.

ADR-0007 formalizes the wrapper-div convention as the intended contract. The renderer behavior is unchanged; only the documented contract is being aligned with reality.

## Why

Comrak owns the emission of `<table>` and `<pre>`. Injecting transync attributes into those elements requires AST-level node manipulation that is verbose, version-brittle, and produces no user-visible benefit over the wrapper-div approach already shipping. The JS sync engine works against either form via `[data-sync-id]`.

## Affected Areas

- `docs/architecture/contracts.md` §4 — needs revised wording to describe the wrapper-div pattern.
- `crates/transync/src/render.rs::wrapper_element_for` — already implements the new contract; inline comment is the authoritative explanation.

## Migration / Follow-up

- Update `contracts.md` §4 prose so future contributors don't read the old wording and "fix" the renderer to match it. (Applied 2026-07-10, together with DCR-0007's list-grouping amendment.)
- Downstream consumers walking up from a table cell to find the sync-id must walk past the `<table>` to the wrapping `<div>`. The shipping JS sync engine walks down from the wrapper, not up from the cell, so this is currently moot.
