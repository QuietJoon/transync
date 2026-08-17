---
type: DCR
title: Reader-honesty rendering — skipped-node placeholders and reference-link (refmap) resolution
description: Two coupled fixes so both rendered panes are honest about the source. (A) Unmodeled top-level nodes (raw HTML, front matter, …) become an anchored, HTML-escaped placeholder in both panes instead of vanishing; the alignment schema bumps 1.0.0 → 1.1.0. (B) Document-level link-reference definitions are recovered from inter-block gaps so `[text][ref]` renders as a real link and translated reference labels become retryable rejections.
tags: [change, project-control, DCR-0013]
status: deprecated
---

# DCR-0013: Reader-honesty placeholders + refmap resolution

- **Date:** 2026-07-27
- **Source:** DR-2026-07 design review (design item D2); owner decisions 2026-07-24
- **Affected ADRs:** docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md (amendment — reference-style destinations are now validatable, closing the "fragment parses resolve no refmap" documented gap); alignment-map schema 1.0.0 → 1.1.0 (contracts.md §3/§4)
- **SUPERSEDED IN PART (2026-08-04) by ADR-0018 / DCR-0016:** raw HTML blocks are no longer `BlockKind::Skipped` and no longer *always* an escaped placeholder. They became the translatable kind `BlockKind::Html` (wire `html`, alignment schema 1.2.0) and, when translation succeeds, **live-render** in both panes — so Part A's "invariant 7: never live HTML" reasoning below no longer holds for HTML blocks, and `parser::skipped_label` can no longer return `"html-block"`. Everything else in this record stands: the escaped `<pre data-skipped>` placeholder itself, its attribute set and anchoring, the other `Skipped` labels (front matter, footnote definitions, unsupported), the exclusion from `validation_summary`, and all of Part B (refmap resolution). Read this record together with DCR-0016.
- **MECHANISM NOTE (2026-08-04, DCR-0017)** — *decisions unchanged; two named mechanisms below no longer exist.* The crate split moved `parser.rs` (+`refdefs.rs`), `id.rs`, `align.rs`, `regen.rs`, and `render.rs` (+`attrs.rs`) to `crates/transync-syntax/src/`; paths in this record are as of 2026-07-27 and are not rewritten. And the AST-direct renderer **deleted `render::block_inner_html`** together with the per-fragment **ref-defs append** that Part B threaded into it — a whole-document parse resolves reference links natively, so the append is unnecessary rather than replaced. Both of Part B's *outcomes* stand: `[text][ref]` renders as a real link, and a translated reference label is still a retryable rejection. `Document.ref_defs` itself survives, now consumed by `validate::inline` only.

## What Changed

Two independent honesty gaps, one per pane, plus two riders.

### Part A — anchored placeholders for skipped top-level nodes

Previously an unmodeled top-level source node (in practice a raw
`NodeValue::HtmlBlock`; also front matter / footnote definitions if their
extensions were on) was carried into the regenerated Markdown verbatim but had
**no** alignment row and **no** rendered element — it silently vanished from
both panes, and its block ID ordinal shifted every following block.

- **`BlockKind::Skipped { label }`** (`id.rs`), id-code prefix `"x"`, wire
  value `"skipped"`. The per-node `label` (`"html-block"`,
  `"footnote-definition"`, `"front-matter"`, `"unsupported"`) rides the DOM
  marker, not the alignment row's `block_kind`.
- **Never translated:** `is_translatable` excludes it, so it is never batched
  or sent to the LLM (invariant 2). Regen already splices its source bytes
  verbatim (no code change).
- **Honest alignment row:** `fallback_status = Preserved`, `sync_role = Anchor`
  (owner decision — it must anchor scroll in both panes), `parent_id = None`,
  and it is **excluded from `validation_summary`** counts so it cannot inflate
  the exit-3 fallback tally.
- **Rendered in both panes** as an inert, HTML-escaped
  `<pre data-skipped="<label>">…</pre>` carrying the full sync-attribute set,
  so the JS engine anchors it with zero JS changes. The payload is
  HTML-escaped and never passed to `block_inner_html` (comrak with
  `render.unsafe_ = false` would replace raw HTML with a comment, losing the
  content again) — **invariant 7: never live HTML**.
- **Schema bump 1.0.0 → 1.1.0** (additive `"skipped"` `block_kind` value +
  `data-skipped` attribute). A single `pub const ALIGNMENT_SCHEMA_VERSION`
  now feeds both `AlignmentMap::default()` and `build_alignment_map` so the two
  sites cannot drift. `KNOWN_SCHEMA` bumped to `{1,1,0}` in **both**
  byte-mirrored `sync.js` copies (`web/js/sync.js` and the CLI-embedded mirror,
  drift test green); the Playwright forward-drift test moved its
  newer-than-known assertion to `"1.2.0"` so it still exercises the
  accepted-with-warning path.

### Part B — reference-link (refmap) resolution

CommonMark link-reference definitions (`[label]: url "title"`) are
document-level state: comrak consumes them into a parser-private `RefMap`
during block finalization and leaves **no AST node**. A single-block *fragment*
parse therefore resolves nothing, so `[text][ref]` rendered as literal bracket
text and contributed no destination to inline protection.

- **`parser/refdefs.rs`** (new, file-as-module, registered `pub mod refdefs;`)
  walks the inter-block gaps at parse time and concatenates the
  pure-definition ones into a pool. Each gap is verify-parsed: **zero AST
  children ⇒ pure definitions** (a definition leaves no node), so only genuine
  definition text is pooled; a gap that parses to noded content raises a
  backstop warning. The pool is stored on **`Document.ref_defs: String`** (new
  pub field).
- **Appended before reparse** to fragments in `render::block_inner_html`
  **and** in the inline-inventory parse of `validate/inline.rs`, giving
  comrak-native label matching (normalization, first-wins duplicates, multiline
  titles) with zero grammar reimplementation and zero HTML output.
  `CodeBlock` (an unclosed fence would swallow the appended definitions) and
  `Skipped` are excluded.
- **Effect:** `[text][ref]` now renders as a real `<a href>` link in **both**
  panes, and a provider that mistranslates a reference **label** now changes
  the resolved destination inventory, so it becomes a retryable
  `ValidationLayer::Inline` rejection (per ADR-0012's amended doctrine). A
  genuinely undefined reference stays literal text on both sides — a symmetric
  no-op, never a false rejection.

**Known limitation (recorded).** Duplicate labels split across a container
block's *interior* and an *inter-block gap* can invert CommonMark's
first-definition-wins in the **per-block render only**: the gap pool is
appended *after* the fragment, so an in-fragment definition wins locally even
where the document-level first definition sits in the pool. Validation stays
symmetric (source and translated fragments both see the same appended pool), so
there is **no false accept or reject** — the divergence is render-only, narrow,
and accepted. The pool itself preserves document order, so pool-internal
duplicates are unaffected.

### Riders (same wave)

- **Web shells** (`web/index.html`, `crates/transync-cli/web/index.html.tpl`)
  gained an always-visible tint **legend** (`pointer-events: none`, so it never
  intercepts scroll/click), `partially_translated` block styling (a cool tint
  distinct from the warm fallback tint, sharing CSS variables with the legend
  swatches so the two cannot drift), and `pre[data-skipped]` placeholder
  styling.
- **Cache twin-block fix** (`pipeline.rs`): when two byte-identical blocks
  collide on one `CacheKey`, the cached `UnitResult.unit_id` is rewritten to
  the *requesting* unit's id at hit time, so every downstream consumer (schema
  validation, per-unit report, cache re-put, eviction key) sees the correct id
  instead of stranding the twin into a silent `FallbackSource`. Cross-block
  dedup is preserved — the payload is re-validated per unit against the
  requesting unit's byte-identical source.

## Why

Both panes must be honest about the source. A vanishing raw-HTML block and a
reference link that degrades to literal brackets are both silent
misrepresentations a reader cannot detect. Part A makes the unmodeled node
*visible and inert* (invariant 7) rather than absent; Part B makes reference
links *resolve and validate* rather than silently degrade. The refmap change
also narrows ADR-0012's documented gap: reference-style destinations join
explicit link/image destinations as protected structure.

## Affected Areas

*Paths as of 2026-07-27, not rewritten — see the 2026-08-04 mechanism note at the top of this record. `unit.rs`, `validate/*`, and `pipeline.rs` stayed in `transync-core`.*

- `crates/transync-core/src/id.rs` — `BlockKind::Skipped { label }` + id-code / wire arms
- `crates/transync-core/src/parser.rs` + `parser/refdefs.rs` (new) — skipped-node emission, `Document.ref_defs`
- `crates/transync-core/src/{unit,align,render,regen}.rs` — non-translatable + anchor + placeholder handling
- `crates/transync-core/src/render.rs` + `render/attrs.rs` — `<pre data-skipped>` placeholder, `ref_defs` threading
- `crates/transync-core/src/validate/{inline,full_reparse,fragment_reparse,per_kind}.rs` — `ref_defs` param, `"skipped"` kind
- `crates/transync-core/src/pipeline.rs` — cache twin-block fix; `validate_batch` call threads `ref_defs`
- `crates/transync-core/src/align.rs` — `pub const ALIGNMENT_SCHEMA_VERSION = "1.1.0"`
- `web/js/sync.js` + `crates/transync-cli/web/sync.js` — `KNOWN_SCHEMA` → `{1,1,0}` (byte-mirrored)
- `web/index.html` + `crates/transync-cli/web/index.html.tpl` — legend, `partially_translated`, `pre[data-skipped]` styling
- `web/tests/scn13.spec.js` — forward-drift assertion moved to `"1.2.0"`; scenario tests bumped to `"1.1.0"`
- `docs/architecture/contracts.md` §3 / §4 — schema 1.1.0, `skipped` kind row, `data-skipped` wrapper
- `docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md` — refmap amendment note

## Migration / Follow-up

Breaking / wire changes ride the **still-open 0.2.0 window**:

- `validate_batch` gained a trailing `ref_defs: &str` parameter (public API
  re-exported by the facade; external mock-driven callers must update the call).
- `Document` gains a `pub ref_defs` field (literal-construction breakage for
  any external code building `Document` by hand — a documented internal type).
- Alignment-map schema `1.1.0`: consumers reject unknown *major* versions and
  accept newer minor/patch with a `console.warn` (forward-compat), so a
  1.0.0-pinned engine now logs the OI-0024 forward-drift warning on every 1.1.0
  map. Intended and visible.
