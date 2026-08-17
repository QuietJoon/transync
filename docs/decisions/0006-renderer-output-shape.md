---
type: ADR
title: Renderer output shape — two pane files plus shell
description: The renderer emits fragment-shaped source.html and target.html; the CLI adds the demo shell, alignment JSON, sync.js, and vendored DOMPurify, committed as one staged six-file fileset — atomic per file, and see the 2026-08-12 amendment for what "one atomic set" does and does not guarantee.
tags: [decision, ADR-0006]
status: active
---

# ADR 0006: Renderer output shape — two pane files plus shell

## Context and Problem Statement

`transync-cli translate --html-out <dir>` produces the static demo bundle that the JS sync engine mounts. The intake (`docs/project/intake.md`) flagged the renderer's output shape as a Phase-3 open item, with two candidates surfaced during brainstorming:

- **Option A — two pane files plus shell.** Render the source block tree into `source.html`, the translated block tree into `target.html`, and emit a small `index.html` shell that loads both into mounting points and pulls in `sync.js` and `alignment.json`.
- **Option B — one document with two pane sections.** Render a single `index.html` that already contains both block trees inline (e.g. as two `<section>` elements), with `sync.js` reading from in-document selectors only.

The choice has to satisfy the eight architectural invariants in `CLAUDE.md` — particularly that the Rust renderer owns structural HTML emission, JS owns only interaction, and a future WASM port (track C, post-MVP) must be a drop-in replacement for the renderer alone.

## Decision Drivers

- **Renderer/JS separation.** `transync::render` should emit just the annotated block-tree HTML; layout, mounting, and demo chrome are CLI concerns. Coupling layout into the renderer makes the WASM-track-C swap harder.
- **Independent reuse.** `source.html` and `target.html` should each be loadable in isolation — both for hand inspection and for downstream consumers (docs sites, diff tools) that want to embed only one pane.
- **Atomic-write contract.** Each output path must be written atomically (`.tmp.<pid>` → fsync → rename). Splitting into separate files makes per-file atomicity natural; collapsing into one file means a single large render+template buffer that has to be assembled before the first write.
- **Smoke testing.** A headless smoke test that asserts "every block-id from the alignment map is present in the rendered HTML" is simpler when each pane is its own file with a single block-tree DOM.
- **Fallback styling.** `data-fallback="fallback_source"` blocks may want a target-pane-specific class. With separate files the CSS is scoped naturally; with one file the JS engine has to disambiguate by ancestor selectors.

## Considered Options

1. **Two pane files plus shell** (`source.html`, `target.html`, `index.html`, `alignment.json`, `sync.js`).
2. **One document with two inline pane sections** (`index.html`, `alignment.json`, `sync.js`).
3. **Three-file no-shell** (`source.html`, `target.html`, `alignment.json` — consumer brings its own demo shell). Rejected: the CLI's job per `mvp-scope.md` includes shipping a runnable demo; pushing the shell to the consumer breaks `cargo run -p transync-cli -- translate ... && transync serve`.

## Decision Outcome

We chose **option 1 — two pane files plus shell**.

`<--html-out>/` will contain exactly six files (sixth added by OI-0001 [archived], 2026-07-10):

```
<--html-out>/
├── index.html        # demo shell — fetches source.html, target.html, alignment.json; loads sync.js
├── source.html       # annotated source block tree (no shell, no head, just the block-tree fragment in a single <main>)
├── target.html       # annotated target block tree (same shape as source.html)
├── alignment.json    # AlignmentMap JSON, schema_version 1.0.0
├── sync.js           # vanilla ESM sync engine (copied from web/js/sync.js at compile time)
└── purify.min.js     # vendored DOMPurify (OI-0001) — shells fail closed if absent
```

`source.html` and `target.html` are **fragment-shaped** — each contains a single `<main>` element wrapping the block tree, with no `<html>`, `<head>`, or `<body>`. The demo shell `fetch()`es each fragment, sanitizes it with `DOMPurify.sanitize()`, and assigns the result to the slot's `innerHTML` (OI-0001) — failing closed and refusing to mount when `DOMPurify` is unavailable. This matches `persistence-and-files.md` §output-directory-layout.

`transync::render` returns these fragment strings — it never templates the shell. The shell is templated inside `transync-cli::output` against an embedded `index.html.tpl`.

Status: Decided 2026-05-01 (Phase 3). Shipped — the six-file bundle is emitted atomically by `transync-cli::output` and mounted by the vendored shell; amended 2026-07-10 by OI-0001 to add `purify.min.js` and sanitize fetched fragments before mount.

### Implementation seams

- `transync::render::render_source(doc, alignment) -> String` — fragment only.
- `transync::render::render_target(doc, translated_md, alignment) -> String` — fragment only.
- `transync-cli::output::write_html_bundle(dir, source_fragment, target_fragment, alignment_json) -> io::Result<()>` — commits all six files as a single staged fileset — `index.html`, `source.html`, `target.html`, `sync.js`, `purify.min.js`, and `alignment.json` (the last from the `--map` argument when `--html-out` is also set).
- `transync-cli/web/index.html.tpl` is `include_str!`'d at compile time. The template has named placeholders (e.g. `{{TRANSYNC_TITLE}}`, `{{ALIGNMENT_MAP_REL}}`) but no per-document content beyond title.

## Consequences

- **Good:** Renderer stays a pure function from IR + alignment → block-tree HTML. WASM track-C reuses it byte-for-byte.
- **Good:** Each pane file is independently inspectable, diffable, and embeddable.
- **Good:** Atomic-write contract maps to one file = one rename; the shell never partially overlays a previous run.
- **Good:** The SCN-13 smoke driver can `curl` `source.html` or `target.html` and grep for `data-sync-id` without parsing a multi-pane composite document.
- **Bad:** Six files instead of one. Mitigated by the fact that the CLI ships them together as one atomic set; they are served by an external static server (e.g. `python3 -m http.server`), since `transync serve` is a deferred placeholder (STUB-061).

  > **Correction (2026-08-16, ti `1c79f8`).** The bullet's *cost* stands — six
  > files are still six files — but both halves of its mitigation have expired,
  > and both are corrected in appended amendments below rather than here. A
  > reader skimming this list for current operational guidance should not act on
  > the sentence as written:
  >
  > - **`transync serve` is not a placeholder and STUB-061 is closed.** It binds
  >   `127.0.0.1:7470` and serves the `--rendered` directory, path-confined,
  >   with a content-type table covering exactly this bundle — so the external
  >   static server is an option, not the step it reads as. See *Amendment
  >   (2026-08-09)*; `contracts.md` §6 carries the contract and
  >   `docs/project/stub-manifest.md` records the closure.
  > - **"One atomic set" is a staging guarantee, not a rollback promise.** The
  >   files mode can leave a mixed set if the rename pass fails partway; the
  >   set-level property is `--out-dir`'s. See *Amendment (2026-08-12)*, which
  >   names this bullet by title.

- **Bad:** Shell template lives in the CLI crate and is therefore CLI-only. WASM consumers will need to bring their own shell. This is acceptable: the shell is demo chrome, not part of the rendering contract.

## Amendment 2026-08-04 (DCR-0017) — mechanism, not decision

*Appended, not a rewrite. Every decision above stands: two fragment-shaped
pane files plus a templated shell, `render_source` / `render_target`
signatures unchanged, renderer never templates the shell.*

Two statements about *how* the renderer works are now stale:

- **Crate home.** `render` (and `render/attrs`) live in
  **`crates/transync-syntax/src/`**, not `crates/transync-core/src/`. The
  paths `transync::render::render_source` / `render_target` are unchanged —
  core re-exports the module and the facade re-exports core.
- **Rendering mechanism.** The renderer used to reparse each block's Markdown
  slice as an isolated fragment and then string-strip Comrak's outer wrapper.
  It now does **one Comrak parse per pane** and zips the top-level AST node
  sequence against the alignment rows, formatting the paired node (its
  children for the strip-kinds, the whole node for table / code block /
  blockquote). The emitted attribute set, wrapper elements, and
  one-line-per-block shape are unchanged; three rendered byte shapes changed
  (loose lists, loose task items, and a fixed bug where 4-space-indented code
  blocks rendered as `<p>`) — see DCR-0017.

The consequence line *"Renderer stays a pure function from IR + alignment →
block-tree HTML. WASM track-C reuses it byte-for-byte"* is the one this
amendment makes literally true: the renderer's crate now compiles for
`wasm32-unknown-unknown` under a standing gate, which is what the 2026-05-01
decision was reserving room for.

## Amendment (2026-08-06) — the seams have been renamed, split, and narrowed

*Appended, not a rewrite. Every decision above stands: six files, two
fragment-shaped pane files, a templated shell the renderer never writes.
Corrected here is the "Implementation seams" list, which still names an API
and a template placeholder that no longer exist — Review-0001 `R0001-0051`.*

- **`write_html_bundle` is gone.** It was renamed and split on 2026-07-13
  (`12545e3`) into two seams with different jobs. `output::html_bundle_files`
  is a **pure** function: it fills the template and returns the six
  `(path, bytes)` pairs, deciding names and contents but writing nothing. It
  takes more than the four arguments listed above — the language labels and
  the two direction attributes are template inputs too (next bullet).
  `output::write_fileset_atomic` then performs the **staged fileset commit**:
  every payload is staged as `<path>.tmp.<pid>` and fsynced, and only when all
  of them are staged does a second pass rename each into place, under a
  publish lock over the destination directories. It is generic over any
  fileset, so `out.md`, the alignment map, and the validation report are
  committed by the same call — the six-file bundle is one caller of it, not
  its definition.
- **The template's placeholders are all `TRANSYNC_*`.** `{{ALIGNMENT_MAP_REL}}`
  no longer exists; the shell's inline module fetches `source.html`,
  `target.html` and `alignment.json` by fixed relative name, so the bundle's
  own file names are not parameterized. What *is* substituted:
  `{{TRANSYNC_TITLE}}`, `{{TRANSYNC_DOC_LANG}}`, and the four per-pane
  attribute slots `{{TRANSYNC_SOURCE_LANG_ATTR}}` /
  `{{TRANSYNC_TARGET_LANG_ATTR}}` / `{{TRANSYNC_SOURCE_DIR_ATTR}}` /
  `{{TRANSYNC_TARGET_DIR_ATTR}}`. So "no per-document content beyond title" is
  no longer true either: the resolved language labels reach the shell
  (attribute-escaped, since they are caller text), and RTL targets get a
  `dir` attribute on the panes — never on `<html>`, which would flip the demo
  chrome.
- **The render seam is not public.** `render_source` / `render_target` keep
  the signatures and the semantics recorded above, but they are
  `transync-syntax::render`'s and are called by `transync-core`'s pipeline,
  which hands the two fragments to the CLI on `TranslationOutput`. Since
  DCR-0018 the facade re-exports no renderer, so the 2026-08-04 amendment's
  `transync::render::…` paths no longer resolve; a consumer that wants to
  render for itself depends on `transync-syntax` directly (`contracts.md` §0
  tier (c)) — which is exactly what `transync-wasm` does.
- **The map in the bundle is `schema_version` 1.2.0**, not the 1.0.0 the file
  tree above records; `contracts.md` §3 owns that number, and the bundle
  simply writes the bytes the run produced.

## Amendment (2026-08-07) — a seventh placeholder: the opt-in bundle CSP

*Appended. The decision above is unchanged: six files, a templated shell, no
per-document content the run did not resolve. What changes is the placeholder
inventory the 2026-08-06 amendment lists.*

- **`{{TRANSYNC_CSP_META}}`** is the seventh slot, and unlike the other six it
  is not caller text at all: `--strict-csp` (OI-0018, posture amended
  2026-08-07) selects between a fixed `Content-Security-Policy` `<meta>`
  literal and the empty string, so the shell cannot be injected into through
  it. `html_bundle_files` therefore gained a `bool`, not a ninth `&str`.
- **The slot sits at the end of the charset line**, so the default (empty)
  substitution leaves `index.html` byte-identical to the pre-flag output —
  which is the whole posture: the CSP is opt-in, and the accepted default
  tradeoff (remote images render) stands. `contracts.md` §6 "Bundle
  Content-Security-Policy" owns the policy text and its derivation.

## Amendment (2026-08-07) — the title slot carries document content, so the fill changed shape

*Appended. The decision above is unchanged: six files, two fragment-shaped pane
files, a templated shell the renderer never writes. What changes is one
sentence of the original "Implementation seams" list — "no per-document content
beyond title" — which the 2026-08-06 amendment already corrected for the
language labels and which is now wrong in the other direction too: the title
itself is per-document content (ti 0f26b5, closing STUB-062/063).*

- **`{{TRANSYNC_TITLE}}` is resolved, not a literal.** `--title` when given,
  else the source document's **first level-1 heading** as parser-plain text,
  else the `transync` literal the slot always carried. The middle level is not
  a second extraction: it is `TranslationOutput::document_title`, the same
  value the provider already read as `BlockContext.document_title`, so the
  document is named identically to the model and to the reader.
  `contracts.md` §6 "Bundle title and language" owns the precedence.
- **The alignment map was deliberately not extended.** A document title is
  presentation, and the map is a durable inter-process wire shape (§3); making
  the bundle's title a schema field would have moved `schema_version` for
  something no sync consumer reads. Bundle assembly reads pipeline state
  instead — which is also why `--title` never reaches `out.md`, the map, or the
  provider, exactly like `--target-direction`.
- **The fill became a single pass** (`output::fill_template`) instead of a
  chain of `String::replace`. With a slot fed by untrusted source content
  (invariant 7), a replace chain lets a heading that reads `{{TRANSYNC_…}}` be
  re-substituted by a later slot's value; scanning once and never rescanning a
  substituted value makes that structural rather than an ordering the next edit
  can revoke. The title is HTML-escaped by the same escaper the language labels
  use, which is a superset of what a `<title>` text node needs.

## Amendment (2026-08-08) — the render seam is fallible

*Appended, not a rewrite. Every decision above stands: two fragment-shaped
pane files plus a templated shell, the renderer never templates the shell,
the seam stays out of the curated facade (`contracts.md` §0 tier (c)). One
statement in the "Implementation seams" list — and its restatement in the
2026-08-04 amendment, "`render_source` / `render_target` keep the signatures"
— is now stale: both return a `Result`.*

Review 0002 (R0002-0010, R0002-0011) found the renderer treating an
alignment map as trustworthy in two ways that failed silently:

- **A repeated `source_block_id`** decided an id's attributes and byte ranges
  by whichever row the `HashMap` happened to index last.
- **A block with no row** was skipped entirely — no anchor, no placeholder,
  no warning — so an incomplete pane was indistinguishable from a complete
  one, against §4a's per-row anchor-survival guarantee.

Both are producer defects (`align::build_alignment_map` emits exactly one
row per block, in document order), and both were reachable only through a
caller-supplied map — `transync-wasm`'s view mode. Since transync has never
shipped to a consumer, the honest fix was taken over the compatible one:

- `render_source(doc, alignment) -> Result<String, RenderError>`
- `render_target(doc, translated_md, alignment) -> Result<String, RenderError>`

`RenderError` is `transync_syntax::render`'s, `#[non_exhaustive]`, and names
the offending ids (bounded). Rows naming blocks the document does not have
are NOT refused — they are inert. Byte-range *sanity* is still the caller's
to keep: `block_text` continues to clamp and snap to char boundaries, the
panic-free posture recorded in that function.

The seam is not in the facade, so `contracts.md` §0 and the
`public_surface.rs` weld are untouched — `render` is on that test's
`forbidden` list and stays there. Callers thread the error: the pipeline maps
it to `TransyncError::Internal` (its own map builder is the only producer, so
a refusal there is an internal fault), and `transync-wasm` grew
`EngineError::Render`.

A third change rides the same signature (R0002-0059): both panes' Markdown
now reaches Comrak through `parser::intake` — the nesting-depth guard plus
NUL normalization `parser::parse` applies — instead of the target pane
calling `comrak::parse_document` directly. A document the library refuses as
a source is no longer accepted as a target. Only the parse consumes the
normalized string; the bypass and degrade arms still slice the caller's own
bytes, which is what the byte ranges index.

## Amendment (2026-08-08) — the refusal reaches range *values*, not just map shape

*Appended, not a rewrite. Everything above stands, including the whole
2026-08-08 amendment; one sentence of it is superseded and is marked here
rather than edited there: "Byte-range **sanity** is still the caller's to
keep: `block_text` continues to clamp and snap to char boundaries." It no
longer does either.*

Review 0003 (R0003-0060, and R0003-0078 as its browser-side shadow) found
the residual the 0002 fix deliberately stopped short of. That fix drew the
refuse-vs-degrade line at map **shape** — a repeated row, a missing row —
and left range **values** to `block_text`, which clamped them into the
pane's length, reordered a reversed pair, and snapped both ends to UTF-8
boundaries. That kept the renderer panic-free, which was the goal, but it
also meant a row pointing at the wrong bytes still rendered: an empty block
for a reversed range, a truncated one for an out-of-bounds range, a block
missing the character it split for a mid-character range — each under the
row's own correct anchor, with its `fallback_status` unchanged and no signal
anywhere. A coercion that keeps rendering is not a guard; it is a silent
wrong answer with a defensive shape.

The values are now checked once per pane, in `PaneCtx::new`, beside the two
0002 refusals, and the renderer refuses with `RenderError::UnusableRange {
pane, ids }` — naming which pane's Markdown the ranges were measured against
and, per offending row, the offsets and the fault, bounded by the same
`MAX_NAMED_IDS` the other refusals use. Each pane checks only what it reads:
`Block::source_range` against `doc.source_text` for the source pane,
`AlignmentBlock::target_range` against the handed `translated_md` for the
target pane. Every block is checked, not only the kinds that read raw bytes
today, because Guard 2 can send any row down the byte-reading path.

Three properties of the check are decisions rather than details:

- **An empty in-bounds range is not a fault.** `build_alignment_map` emits
  `0..0` for a block its `BlockOffsets` did not cover, and warns about it
  (R0003-0055). That map still renders.
- **Inert rows stay inert.** Rows naming blocks the document does not have
  are still accepted and still never looked up, so their ranges are never
  measured either.
- **`block_text` slices instead of coercing.** It borrows `source.get(..)`
  behind a `debug_assert`, so the only residue of the old posture is a
  wrong-but-not-panicking answer for a future caller that builds a `PaneCtx`
  some other way. `regen::regenerate` keeps `clamped_char_bounds`
  (R0003-0059): it is infallible by design and has no error channel to
  refuse through.

R0003-0078 closes with it. `web/js/wasm-demo.js` checks `target_range`
values only for the rows it lets you edit, because its own blast radius is
the edit model (R0002-0054) — but html and skipped rows' ranges are read by
the renderer's bypass arms, and nothing was checking those. The renderer now
does, for every row it reads, so the demo needs no second gate; its gate
stays where it is and keeps owning the earlier, edit-model-specific message.

The seam is still not in the facade: `render` is `contracts.md` §0 tier (c)
and on `public_surface.rs`'s `forbidden` list, so the §0 table and the weld
are untouched. `RenderError` is `#[non_exhaustive]`, so the new variant is
additive for every caller that matches on it; the pipeline's blanket mapping
to `TransyncError::Internal` and `transync-wasm`'s `EngineError::Render`
carry it unchanged.

## Amendment (2026-08-09) — the "external static server" mitigation is now transync's own

*Appended, not a rewrite. The decision above stands whole: six files, two
fragment-shaped pane files plus a templated shell, committed as one atomic
set.*

The **Bad: six files instead of one** consequence names its mitigation as
"they are served by an external static server (e.g. `python3 -m
http.server`), since `transync serve` is a deferred placeholder
(STUB-061)". Half of that sentence has expired. `transync serve` stopped
being a placeholder on 2026-08-09 (ticket `b791d6`): it binds
`127.0.0.1:7470` and serves the `--rendered` directory, path-confined to
it, with a content-type table that covers exactly this bundle's file set.

The consequence itself is unchanged and, if anything, cheaper: six files
are still six files, but the server that turns them back into one page is
now the same binary that wrote them, so "bring your own static server" is
no longer a step a reader has to supply. An external server still works —
the bundle is self-contained, which is the property that made the
mitigation valid in the first place and is what the whole rejected
**Three-file no-shell** option was measured against. `contracts.md` §6
carries the server's contract; `docs/project/stub-manifest.md` records the
STUB-061 closure.

## Amendment (2026-08-12) — "one atomic set" is a staging guarantee, not a rollback promise

*Appended, not a rewrite. The decision above stands whole: six files, two
fragment-shaped pane files plus a templated shell the renderer never writes.
What is corrected is one word this ADR leans on — "atomic" — as it appears in
the Status line ("emitted atomically by `transync-cli::output`") and in the
**Bad: six files instead of one** consequence ("the CLI ships them together as
one atomic set"). Both read as a promise of transactional replacement, which
the files mode does not make and never did (Review 0004 `R0004-0100`).*

`output::write_fileset_atomic` is a **staged fileset commit**, and it is
atomic in one direction only:

- **Phase 1** stages every payload as `<path>.tmp.<pid>` and fsyncs it. A
  failure here — the phase that does the real I/O — removes every staged temp
  and leaves every target untouched. That much this ADR's consequence line
  claims correctly, and it is what "one file = one rename; the shell never
  partially overlays a previous run" was reaching for.
- **Phase 2** renames each staged file over its target. A crash or I/O error
  partway through this pass **can leave a mixed set** — some outputs from this
  run beside some from the previous one. Nothing rolls that back: the run says
  so on stderr and exits `4`. What the staging buys is a smaller exposure, not
  its absence — "seconds of content writing" shrink to a few metadata renames
  — and the publish lock over every destination directory (DCR-0021) confines
  the window to a crash/IO-error one rather than a concurrency one, because a
  *second run* cannot interleave its renames with this one.

`contracts.md` §6 states both of those precisely (exit code `4`, and
"Publication locking"), and it is the authority; this ADR's prose is a summary
of it and should be read that way wherever the two are not word-for-word.

Set-level atomicity — the property where no reader ever sees a mixed set — is
a **different publication mode that already ships**, not missing work.
`--out-dir` stages the whole tree in a sibling directory and renames it into
place: onto a fresh target that is one atomic rename, and replacing an
existing target is crash-safe but *not* atomic (the old tree is moved aside
first, so a reader between the two renames finds no target at all).
`contracts.md` §6 owns both descriptions. Nothing here asks for the files mode
to grow a transaction; the choice between the two exposures is the one the two
flags already offer, and this amendment exists so the ADR stops implying the
files mode already made it.

(The **Bad** line's other half — "since `transync serve` is a deferred
placeholder (STUB-061)" — expired separately and is corrected by the
2026-08-09 amendment directly above.)
