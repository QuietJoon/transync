---
type: DCR
title: Grouped list rendering with <li> sync anchors
description: Consecutive items of one source list now share a single unattributed <ul>/<ol>; sync attributes moved from per-item <div> wrappers onto real <li> elements, and list items' sync_role became anchor.
tags: [change, project-control, DCR-0007]
status: active
---

# DCR-0007: Grouped list rendering with `<li>` sync anchors

- **Date:** 2026-07-10
- **Source:** OI-0010 [archived] (R0003-0027) + folded R0006-0042; user-approved resolution session (review archived and removed)
- **Affected ADRs:** none superseded. `docs/architecture/contracts.md` §4 and §4a updated
  (this change also applied DCR-0001's pending §4 prose migration for the
  table/code/blockquote wrapper-div rows).

## What Changed

Before: each top-level list item rendered as its own
`<div data-sync-id="li-NNNN">` whose Comrak inner HTML reparsed as a
single-item `<ul><li>…</li></ul>` — so `- a\n- b` produced two separate
one-item lists (broken list styling, screen readers announcing N lists),
and the alignment row said `sync_role: child-only` while the DOM treated
each item as a top-level anchor.

Now:

1. `render::render_fragment` groups consecutive top-level list items that
   belong to the same source list — identified by their `ast_path` prefix
   (items carry the owning List node's path plus their own index) — inside
   one shared `<ul>`/`<ol>`. The group element carries no attributes.
2. Sync attributes moved onto real `<li>` elements (wrapper for
   `BlockKind::ListItem` changed `div` → `li`); the per-item inner HTML
   strips Comrak's redundant `<ul><li>` wrapper.
3. A marker-type change starts a new group, matching CommonMark list
   boundaries and the full-reparse normalizer's grouping rule.
4. `sync_role` for list items changed `child-only` → `anchor`
   (R0006-0042): items are the primary anchors; no list-level row exists.

## Why

`- a\n- b` must render as one list. Anchoring on `<li>` keeps sync
identity per item without a second wrapper layer. Contracts §4 had always
promised "emit on the `<li>`" — the renderer now honors it.

## Affected Areas

- `crates/transync-core/src/render.rs` (`render_fragment`,
  `wrapper_element_for`, `strip_outer_wrapper`)
- `crates/transync-core/src/align.rs` (`sync_role_for`)
- `docs/architecture/contracts.md` §4, §4a
- Both `sync.js` copies are unaffected (they query `[data-sync-id]` only);
  `offsetTop` math is unaffected because the group element is never
  positioned.

## Migration / Follow-up

- Consumers that assumed every `[data-sync-id]` element is a DIRECT child
  of `<main>` must tolerate `<li>` anchors one level deep (§4a wording
  updated). `querySelectorAll`-based consumers are unaffected.
- Known limitation: ordered lists render with sequential numbering from 1;
  a source list starting at another number (`3. x`) loses its `start`
  offset because the IR does not carry it. Rendering was strictly worse
  before (every item numbered "1."); carry `start` in the IR if this
  matters later.
- SCN-05 visual smoke re-run recommended at the next `web/SMOKE.md` pass.

### Update 2026-07-13 — start-offset limitation resolved (EXT-2026-07 P2-9)

The ordered-list start-offset limitation noted above is resolved (OI-0022).
`render::render_fragment` now derives the start ordinal from the first
item's fragment when it opens an ordered group and emits `<ol start="N">`
whenever `N != 1`; a list opening at `3.` renders `<ol start="3">` instead
of renumbering from 1. The group element still carries no *sync* attributes
(sync identity remains on the per-item `<li>` anchors) — `start` is a
presentational attribute only, so the DCR-0007 invariant holds in the
common (`start == 1`) case, which keeps the bare `<ol>`.

The same change extended the list fingerprint (`llm::ListTopologyEntry`
gained `start` / `delimiter` / `tight`) so `validate::per_kind::check_list`
rejects a provider that renumbers, flips `.`↔`)`, or changes list tightness,
and the provider client's `ListTopologyHint` now tells the model to preserve
those marker facts. Blockquote validation gained one level of structural
recursion in the same pass (a reshape of a list or nested quote *inside* a
quote is now rejected).

### Update 2026-08-04 — grouping is now walk-driven (DCR-0017)

*Paths and mechanism above are as of 2026-07-10/13; the decisions stand.
`render.rs` and `align.rs` now live in `crates/transync-syntax/src/`.*

Every rendered shape this record decided is unchanged — one shared
unattributed `<ul>`/`<ol>` per source list, sync attributes on real `<li>`
anchors, marker-type change starts a new group, `sync_role: anchor` for items,
`<ol start="N">` when `N != 1`. What changed is the mechanism:

- **Grouping no longer reads `ast_path` prefixes in the renderer.** The
  consecutive-`ListItem`-rows ↔ one source `List` collapse now comes from the
  single shared `transync_syntax::walk::normalize_top_level` helper, which
  `validate::full_reparse` consumes too — one implementation, so the
  renderer's grouping and the validator's grouping cannot drift apart.
- **The group tags are written from the paired `List` AST node** (its
  `list_type` and `start`), which retired the `ordered_list_start` helper that
  derived the start ordinal by inspecting the first item's fragment text.
- **The per-item "strip Comrak's redundant `<ul><li>` wrapper" step is
  gone.** `strip_outer_wrapper` was deleted; the renderer formats the paired
  `Item`/`TaskItem` node's children directly into the `<li>`, and emits the
  `TaskItem` checkbox `<input>` itself (Comrak writes it in the item's open
  tag, which children-format does not reach).
- **New in DCR-0017:** because the pane is parsed as a whole document, list
  **tightness is context-faithful** — a loose source list now renders
  `<li><p>…</p></li>` in both panes, where per-item fragment reparse used to
  flatten it to tight. And `validate::full_reparse` gained a per-list
  item-count check (`walk::direct_item_count`) so the row↔item pairing the
  renderer depends on is validated, not assumed.

### Update 2026-08-05 — re-checked end to end; the limitation bullet is historical

Backlog item `ordered-list-start-offset` re-opened this record's "Known
limitation" bullet (ordered lists renumber from 1, "carry `start` in the IR if
this matters later"). **That bullet has been stale since 2026-07-13** — it is
the original 2026-07-10 text, kept verbatim because this is a snapshot record,
and the two update sections above supersede it. No IR change was ever needed:
`start` is read off the paired Comrak `List` node at wrapper-emission time, so
the fix is entirely render-side and `pub struct Block` never grew a field.

What this pass verified rather than changed:

- **Both panes**, not just the source one. The panes share the renderer, but
  the target pane parses the *regenerated* Markdown, so the ordinal reaches it
  only if the accepted payload keeps its own marker. `regen::regenerate`
  splices list-item payloads verbatim (there is no list reserialization step),
  and `validate::per_kind::check_list` rejects a provider that renumbers, so a
  faithful translation and a fallback both keep `3.`. Newly pinned by
  `render::list_grouping_tests::ordered_start_reaches_the_target_pane_too`,
  which runs the real `regen` → `render_target` path and asserts
  `<ol start="3">` in **both** panes.
- **The sanitizer does not strip it.** Both demo shells and the CLI
  `--html-out` bundle mount through `DOMPurify.sanitize(html)` with no config,
  and the vendored build (3.2.6, `web/vendor/purify.min.js`) carries `start` in
  its default attribute allowlist — confirmed empirically by sanitizing
  `<ol start="3">` in headless Chromium against that exact file: attribute and
  `data-sync-id` both survive.
- **The CLI bundle path needs nothing extra.** `pipeline` fills
  `annotated_source_html` from `render_source`, and the bundle embeds that
  string verbatim, so it inherits the attribute with no bundle-side work.

### Update 2026-08-06 — the group is no longer strictly unattributed: task lists carry the GFM classes

Review-0001 finding `R0001-0026` (ticket `98f9ecfd`). This record's shorthand
for the group tag — "one shared **unattributed** `<ul>`/`<ol>` per source list"
— was already loosened once, by the 2026-07-13 `start` update. It is loosened a
second time, on the same terms: **presentational attributes only, never sync
identity.** A group holding at least one task row now opens as
`<ul class="contains-task-list">`, and each task row's anchor as
`<li class="task-list-item" data-sync-id="…" …>`, the class ahead of a sync
attribute set that is byte-identical to what it was before.

What the finding was actually about: those two classes are the **GFM
task-list convention** that rendered GitHub Markdown carries and that task-list
styling and integrations key on — not, despite the finding's wording, bytes
comrak ever wrote. The pinned comrak (0.27.0) emits neither class anywhere, and
exposes no option to turn them on: its `List` arm writes a bare `<ul>`/`<ol>`
whose only attribute is `start`, and its `TaskItem` arm a bare `<li>` plus the
checkbox `<input>`. Reconstructing the group here (this record) is therefore
what makes the convention ours to keep, and the checkbox surviving alone was
not the same DOM.

Decisions worth keeping:

- **Both classes ride the paired `TaskItem` node**, the same signal that
  produces the checkbox `<input>` — not `BlockKind::ListItem`'s `task` field.
  So class and checkbox cannot disagree, including on the Guard-2 degrade path
  (no paired node → no checkbox, no class) and in the **target pane**, where
  the source block still says "task" for a row whose translated payload dropped
  its `[ ]` marker. Such a row renders no checkbox, and must not claim a class
  for one.
- **Order is fixed**: the class leads the group tag (`<ol
  class="contains-task-list" start="3">`) and leads the `<li>`'s sync
  attributes.
- **Non-task lists are unchanged byte-for-byte** — no class is emitted anywhere
  on that path.
- **`task-list-item-checkbox`, GFM's third class, is deliberately not emitted.**
  The checkbox is reproduced byte-for-byte as comrak writes it; a class there
  would make this checkbox differ from every other checkbox comrak emits.

Verified rather than assumed:

- **The sanitizer does not strip it.** Same check the `start` update ran:
  sanitizing the exact emitted markup with the vendored DOMPurify
  (3.2.6, `web/vendor/purify.min.js`) in headless Chromium keeps both classes,
  both `start` and every `data-sync-id` — `class` is in its default attribute
  allowlist.
- **Known limitation (tracked separately, ticket `cfeb5df5`):** only the
  reconstructed top-level group is marked. A task list *nested* inside an item
  is not reconstructed — it is a child node comrak formats whole — so it
  renders `<ul><li><input …></li></ul>` with neither class. Confirmed by
  rendering `- [x] outer\n  - [ ] nested\n`.

### Update 2026-08-07 — the nested-task-list gap is accepted, and the contract states it

Ticket `cfeb5df5`, the gap the 2026-08-06 update filed against itself. Decision:
**accept it and document it.** No renderer change; `contracts.md` §4/§4a now
describe the rendered shape a consumer actually gets, so downstream CSS is
written knowing it.

Why not the two alternatives:

- **Upgrading comrak** to a release that exposes a task-list-class render
  option would cover top level and nested alike with one setting — but comrak
  is the anchor-stability-critical dependency (ADR-0004). Every block ID,
  `ast_path`, top-level normalization decision and reparse guard is derived
  from its tree, so the bump is re-validated against the whole alignment
  surface, not against the two attributes that motivated it. That is
  disproportionate risk for a pair of CSS classes.
- **Rendering nested lists ourselves** duplicates comrak's item and
  tightness rendering below the top level, and either that or rewriting
  comrak's output bytes breaks the renderer's standing rule — no string
  surgery on comrak's HTML — which is exactly what makes list tightness
  source-faithful (DCR-0017).

What downstream sees, measured through `render::render_source` rather than
assumed:

- `- [ ] outer` with `- [x] inner` / `- [ ] inner` indented under it: the
  reconstructed group opens `<ul class="contains-task-list">` and the outer row
  anchors as `<li class="task-list-item" data-sync-id="…" …>`, while the nested
  list inside that `<li>` is comrak's own `<ul>` holding bare
  `<li><input type="checkbox" …> …</li>` rows — no class on either element, and
  no sync attributes, since a nested item is not a block and never becomes an
  anchor.
- A plain outer row with a task row nested under it (`- outer` / `  - [x] a`)
  marks *nothing*: the group has no top-level task row, so it renders exactly
  as a plain list does, checkbox and all.
- Nesting depth changes nothing further — three levels deep, only the outermost
  group and its own rows are marked — and the classes are the *only* thing
  missing down there: comrak still writes its own `start="N"` on a nested
  ordered list, so ordinals survive nesting even though the classes do not.

Consequences worth stating plainly:

- **The gap is presentational only.** Scroll sync is untouched — nested rows
  were never anchors, so `querySelectorAll("[data-sync-id]")` returns the same
  elements it always did.
- **No shipped CSS keys on the classes.** The demo shells and the CLI
  `--html-out` bundle style lists without them, so nothing in-tree renders
  wrong today; this is a statement for consumers who add their own rules.
- **The guidance that follows from it** is in `contracts.md` §4: a rule that
  must reach every task row keys on the checkbox `<input>` the row always
  carries, not on `.contains-task-list` / `.task-list-item`, which mark the
  top level and stop there.
