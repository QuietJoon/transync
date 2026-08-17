---
type: ADR
title: Renderer wraps `<table>` and `<pre>` in a transparent `<div>`
description: Sync-id attributes live on a transparent wrapper `<div>` instead of Comrak's own `<table>`/`<pre>`, avoiding AST surgery while giving a single addressable `[data-sync-id]` element per block.
tags: [decision, ADR-0007]
status: active
---

# ADR: Renderer wraps `<table>` and `<pre>` in a transparent `<div>`

## Context and Problem Statement

Found in Review 0001 (Issue R0001-0024, Severity: High) (review archived and removed).
Location: `crates/transync-core/src/render.rs::wrapper_element_for`
(now `crates/transync-syntax/src/render.rs` — see the 2026-08-04 amendment
at the end of this record)

`docs/architecture/contracts.md` §4 (HTML attributes) specifies that the
sync-id attributes (`data-sync-id`, `data-block-kind`, `data-order`,
`data-fallback`, `data-parent-id`) live on the *semantic outer element*
of each block. For tables that means `<table data-sync-id="t-0001">…`;
for code blocks that means `<pre data-sync-id="c-0001"><code>…`.

The shipping renderer instead wraps tables and code blocks in a
transparent `<div data-sync-id="t-0001"><table>…</table></div>` /
`<div data-sync-id="c-0001"><pre><code>…</code></pre></div>`. The
inline comment on `wrapper_element_for` documents the choice:

> For block kinds whose Comrak HTML already includes the semantic
> element (`<table>`, `<pre>`, `<ul><li>...`, `<blockquote>`), we use
> a transparent `<div>` so the sync-id wrapper does not nest a second
> copy of the same element.

This is a real conflict between the contract and the implementation.

## Decision Drivers

* Comrak emits its own `<table>` / `<pre>` outer element when rendering
  the block to HTML. We do not own that emission — Comrak does.
* Injecting attributes into Comrak's emitted outer element requires
  either AST-level node manipulation (Comrak supports it but the API
  is verbose) or post-process string surgery (brittle).
* Wrapping in a transparent `<div>` is one line of code per block kind
  and produces predictable output independent of Comrak version.
* The JS sync engine reads attributes from the outermost
  `[data-sync-id]` element regardless of tag name; nothing
  user-visible breaks under either path.
* Downstream consumers of the alignment-map JSON read attribute *names*,
  not tag names, so the contract's *attribute* shape is unchanged.

## Considered Options

1. **Update contract to document the wrapper-div convention.**
   Codify the existing renderer behavior. The semantic `<table>` /
   `<pre>` stays clean (no transync-specific attributes), and the
   sync-id wrapper is the addressable element for both CSS and JS.
2. **Rewrite renderer to inject attributes into Comrak's outer
   element.** Brings the implementation back to the contract's
   original wording. Costs: more code, Comrak-version risk, and the
   semantic outer element gets transync-specific attributes that may
   surprise downstream consumers.
3. **Drop the contract requirement entirely; allow either form.**
   Lowest commitment but least useful — consumers cannot rely on
   a stable shape.

## Decision Outcome

ACCEPT Option 1: We decided for the wrapper-div convention because the
implementation has been live since v0.1.0, all SCN-13 demos pass, the
JS sync engine works against either form, and the cost of (2) is real
(Comrak AST surgery, version brittleness) without buying anything that
either (a) the user observes or (b) downstream consumers depend on.

Status: Implemented. Contract update in
`docs/architecture/contracts.md` §4 + cross-reference DCR-0001.

### Implementation

No code change required — the renderer behavior is being formalized,
not modified. The action items are documentation:
- Update `contracts.md` §4 to describe the wrapper-div pattern as the
  intended shape.
- Cross-reference this ADR + DCR-0001.
- Keep the inline comment on `wrapper_element_for` as the
  authoritative description of the choice.

## Consequences

* Good, because the contract now matches reality. New contributors
  reading either the ADR or the inline comment reach the same mental
  model.
* Good, because the implementation can use Comrak's HTML output
  unmodified — no AST surgery required for tables and code blocks.
* Good, because downstream CSS/JS consumers always have a single,
  predictable element to target via `[data-sync-id]` regardless of
  block kind.
* Bad, because the semantic outer element (`<table>`, `<pre>`) no
  longer carries the transync attributes, so a consumer that walks
  *up* from a table cell to find the sync-id has to walk past the
  table to the wrapping div. (The current JS sync engine walks down
  from the wrapper, not up from the cell, so this is moot for the
  shipping demo.)

## Amendment 2026-08-04 (DCR-0017) — mechanism, not decision

*Appended, not a rewrite. The decision stands unchanged: sync-id attributes
live on a transparent `<div>` wrapping Comrak's own `<table>` / `<pre>` /
`<blockquote>`, and `contracts.md` §4 continues to document that as the
intended shape.*

Two mechanism statements above are now stale:

- **Location.** `wrapper_element_for` lives in
  `crates/transync-syntax/src/render.rs` (the renderer moved to the new
  wasm32-compilable base crate). Its inline comment remains the authoritative
  description of the wrapper choice, per the *Implementation* section's third
  action item.
- **"No AST surgery required" is now satisfied differently — and better.**
  The 2026-07 renderer avoided AST surgery by reparsing each block as an
  isolated fragment and string-stripping Comrak's outer element
  (`strip_outer_wrapper`). That mechanism is **deleted**. The renderer now
  parses each pane once and hands the paired AST node straight to
  `comrak::format_html`: for table / code block / blockquote it formats the
  **whole node**, so Comrak still emits its own `<table>` / `<pre><code>` /
  `<blockquote>` inside the transparent `<div>` — identical output, reached by
  reading the AST instead of by rewriting Comrak's HTML.

The *Decision Drivers* bullet "Injecting attributes into Comrak's emitted
outer element requires either AST-level node manipulation … or post-process
string surgery (brittle)" is unchanged as a rejection of option 2 — the
renderer still injects nothing into Comrak's element. What changed is that the
alternative it chose is no longer string surgery either, which retires the
R0001-0096 brittleness that OI-0008 tracked.
