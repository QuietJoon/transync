---
type: ADR
title: Renderer output shape — two pane files plus shell
description: The renderer emits fragment-shaped source.html and target.html; the CLI adds the demo shell, alignment JSON, sync.js, and vendored DOMPurify, committed as one staged six-file fileset — atomic per file, and the decision body states what "one atomic set" does and does not guarantee.
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

- **Renderer/JS separation.** `transync-syntax::render` should emit just the annotated block-tree HTML; layout, mounting, and demo chrome are CLI concerns. Coupling layout into the renderer makes the WASM-track-C swap harder.
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

The `--html-out` bundle is exactly six files. The directory may also hold the zero-byte `.transync-publish.lock` marker (DCR-0021), which is not part of the bundle; `persistence-and-files.md` lists both:

```
<--html-out>/
├── index.html        # demo shell — fetches source.html, target.html, alignment.json; loads sync.js
├── source.html       # annotated source block tree (no shell, no head, just the block-tree fragment in a single <main>)
├── target.html       # annotated target block tree (same shape as source.html)
├── alignment.json    # AlignmentMap JSON, the run's own map bytes; contracts.md §3 owns its schema_version
├── sync.js           # vanilla ESM sync engine (crates/transync-cli/web/sync.js, web/js/sync.js's byte-pinned twin, embedded at compile time)
└── purify.min.js     # vendored DOMPurify (OI-0001) — shells fail closed if absent
```

`source.html` and `target.html` are **fragment-shaped** — each contains a single `<main>` element wrapping the block tree, with no `<html>`, `<head>`, or `<body>`. The demo shell `fetch()`es each fragment, sanitizes it with `DOMPurify.sanitize()`, and assigns the result to the slot's `innerHTML` (OI-0001) — failing closed and refusing to mount when `DOMPurify` is unavailable. This matches `persistence-and-files.md` §output-directory-layout.

The renderer (`transync-syntax::render`) returns these fragment strings — it never templates the shell. The shell is templated inside `transync-cli::output` against an embedded `index.html.tpl`.

Status: Decided 2026-05-01 (Phase 3). Shipped — `transync-cli::output` commits the six-file bundle as one staged fileset (atomic per file, not as a set; see *What the bundle commit guarantees* below) and the vendored shell mounts it.

### What the bundle commit guarantees

"Atomic" in this ADR means atomic per file. `output::write_fileset_atomic` is
a **staged fileset commit**, and it is atomic in one direction only:

- **Phase 1** stages every payload as `<path>.tmp.<pid>` and fsyncs it. A
  failure here — the phase that does the real I/O — removes every staged temp
  and leaves every target untouched.
- **Phase 2** renames each staged file over its target. A crash or I/O error
  partway through this pass **can leave a mixed set** — some outputs from this
  run beside some from the previous one. Nothing rolls that back: the run says
  so on stderr and exits `4`. Staging shrinks the exposure from seconds of
  content writing to a few metadata renames, and the publish lock over every
  destination directory (DCR-0021) confines the window to a crash/IO-error
  one rather than a concurrency one, because a *second run* cannot interleave
  its renames with this one.

Set-level atomicity — the property where no reader ever sees a mixed set — is
a **different publication mode that already ships**, not missing work.
`--out-dir` stages the whole tree in a sibling directory and renames it into
place: onto a fresh target that is one atomic rename, and replacing an
existing target is crash-safe but *not* atomic (the old tree is moved aside
first, so a reader between the two renames finds no target at all). The files
mode does not grow a transaction; the choice between the two exposures is the
one the two flags offer.

`contracts.md` §6 states all of this precisely (exit code `4`, "Publication
locking", and both `--out-dir` descriptions), and it is the authority; this
section is a summary of it and should be read that way wherever the two are
not word-for-word.

### Implementation seams

**Rendering** lives in `transync-syntax`, the crate under the standing
`wasm32-unknown-unknown` gate, and stays outside the curated facade: the facade
re-exports no renderer (DCR-0018), and `render` is `contracts.md` §0 tier (c)
and on `public_surface.rs`'s `forbidden` list. `transync-core`'s pipeline
calls it and hands the two fragments to the CLI on `TranslationOutput`; a
consumer that wants to render for itself depends on `transync-syntax`
directly, which is exactly what `transync-wasm` does.

- `transync_syntax::render::render_source(doc, alignment) -> Result<String, RenderError>` — fragment only.
- `transync_syntax::render::render_target(doc, translated_md, alignment) -> Result<String, RenderError>` — fragment only.
- `transync_syntax::render::render_source_html(doc, alignment, outcomes)` / `render_target_html(doc, translated_html, alignment, outcomes)` — the panes of an `--input-format html` run (DCR-0038): the same `<main>` fragment shape and the same map refusals, built from the rows' byte ranges. Each block's reserved sync attributes are stripped and the four canonical ones injected into its own outermost element; a block with no covering element of its own (a text run, an image run) and every `html`-kind block get a transparent `<div>` instead. No Comrak parse runs, and the target pane still passes the `markdown::intake` guard `render_target` applies.

**Each pane names the author's ids in a namespace of its own** (R0012-0015,
DCR-0063). The shells mount both fragments into one page, where an id both
carried would send every same-document lookup from the target pane — a
fragment link, a label, a table header — into the source pane. The renderer
prefixes the source pane's author ids with `transync-src--` and the target
pane's with `transync-tgt--`, and moves the same-pane references with them;
`contracts.md` §4 owns the attribute list and the residuals. The *Independent reuse* driver
therefore holds with one qualification. A pane file loaded or embedded on its
own keeps its in-pane links, labels and header cells working, but an external
deep link into it, and consumer CSS or script keyed on an author id, address
the prefixed id: `source.html#transync-src--install`, never
`source.html#install`. The author's own ids are in the published document,
which the renderer never touches.

For a Markdown run, each pane is **one Comrak parse** (DCR-0017): its top-level AST node sequence
is zipped against the alignment rows, and each paired node is formatted — its
children for the strip-kinds, the whole node for table / code block /
blockquote. Both panes reach Comrak through `markdown::intake`, the
nesting-depth guard and NUL normalization a source document meets, so a
document the library refuses as a source is not accepted as a target
(R0002-0059). Only the parse consumes the normalized string; the bypass and
degrade arms slice the caller's own bytes, which is what the byte ranges
index.

**The renderer does not trust the map it is handed**, and `contracts.md` §4a
owns that contract. `PaneCtx::new` checks the map once per pane and refuses
with a `RenderError` — `#[non_exhaustive]`, naming the offending ids, bounded
by `MAX_NAMED_IDS` — on a repeated `source_block_id` (R0002-0010), on a block
with no row (R0002-0011), and on a byte range that is reversed, past the end,
or mid-character in the pane it indexes (`RenderError::UnusableRange { pane,
ids }`, R0003-0060). Each pane checks only what it reads —
`Block::source_range` against `doc.source_text`,
`AlignmentBlock::target_range` against the handed `translated_md` — but checks
every block, not only the kinds that read raw bytes, because Guard 2 can send
any row down the byte-reading path. An empty in-bounds range is not a fault:
`build_alignment_map` emits `0..0`, and warns, for a block its `BlockOffsets`
did not cover (R0003-0055). Rows naming blocks the document does not have stay
inert and are never measured.

A coercion that keeps rendering is not a guard; it is a silent wrong answer
with a defensive shape. So `block_text` slices (`source.get(..)` behind a
`debug_assert`) instead of clamping, leaving only a wrong-but-not-panicking
answer for a future caller that builds a `PaneCtx` some other way, while
`regen::regenerate` keeps `clamped_char_bounds` (R0003-0059) because it is
infallible by design and has no error channel to refuse through. The pipeline
maps a refusal to `TransyncError::Internal`: its own map builder
(`align::build_alignment_map`, exactly one row per block, in document order)
is the only producer on that path, so a refusal there is an internal fault.
The refusals exist for caller-supplied maps such as `transync-wasm`'s view
mode, which carries the error as `EngineError::Render`; `web/js/wasm-demo.js`
needs no second gate, and its own `target_range` check covers only the rows it
lets you edit, because its blast radius is the edit model (R0002-0054), and
keeps owning that edit-model-specific message (R0003-0078).

**Bundle assembly** is two seams in `transync-cli::output` with different
jobs:

- `output::html_bundle_files` is a **pure** function: it fills the template
  and returns the six `(path, bytes)` pairs, deciding names and contents but
  writing nothing. Besides the directory, the two fragments and the map
  bytes, it takes every template input — the resolved title, the two language
  labels, the two pane direction attributes, and a `strict_csp` `bool`.
  `alignment.json` is the map the run produced, byte for byte — the same
  bytes the run writes as its alignment map (`--map`); `contracts.md` §3 owns
  its `schema_version`.
- `output::write_fileset_atomic` performs the **staged fileset commit**
  described above. It is generic over any fileset, so `out.md`, the alignment
  map, and the validation report are committed by the same call — the
  six-file bundle is one caller of it, not its definition.

**The shell template**, `crates/transync-cli/web/index.html.tpl`, is
`include_str!`'d at compile time. Its seven placeholders are all
`TRANSYNC_*`, and the shell's inline module fetches `source.html`,
`target.html` and `alignment.json` by fixed relative name, so the bundle's own
file names are not parameterized.

- `{{TRANSYNC_TITLE}}` is resolved, not a literal: `--title` when given, else
  the source document's first level-1 heading as parser-plain text, else the
  literal `transync`. The middle level is `TranslationOutput::document_title`,
  the value the provider reads as `BlockContext.document_title`, so the
  document is named identically to the model and to the reader. `contracts.md`
  §6 "Bundle title and language" owns the precedence.
- `{{TRANSYNC_DOC_LANG}}` and the four per-pane slots
  (`{{TRANSYNC_SOURCE_LANG_ATTR}}` / `{{TRANSYNC_TARGET_LANG_ATTR}}` /
  `{{TRANSYNC_SOURCE_DIR_ATTR}}` / `{{TRANSYNC_TARGET_DIR_ATTR}}`) carry the
  resolved language labels, attribute-escaped because they are caller text, and
  an RTL target's `dir` attribute — on the panes, never on `<html>`, which
  would flip the demo chrome.
- `{{TRANSYNC_CSP_META}}` is not caller text at all: `--strict-csp` (OI-0018)
  selects between a fixed `Content-Security-Policy` `<meta>` literal and the
  empty string, so the shell cannot be injected into through it. The slot sits
  at the end of the charset line, so a run without the flag writes the same
  `index.html` it would with no slot there: the CSP is opt-in, and the
  accepted default tradeoff (remote images render) stands. `contracts.md` §6
  "Bundle Content-Security-Policy" owns the policy text and its derivation.

The fill is a **single pass** (`fill_template`, in `output`'s `bundle` child)
rather than a chain of `String::replace`. The title is fed by untrusted source
content (invariant 7), and a replace chain lets a heading that reads
`{{TRANSYNC_…}}` be re-substituted by a later slot's value; scanning once and
never rescanning a substituted value makes that structural rather than an
ordering the next edit can revoke. The title is HTML-escaped by the escaper
the language labels use, a superset of what a `<title>` text node needs.
**The alignment map was deliberately not extended** to carry the title: a
document title is presentation, and the map is a durable inter-process wire
shape (§3) whose `schema_version` would have moved for something no sync
consumer reads. Bundle assembly reads pipeline state instead, which is why
`--title` never reaches `out.md`, the map, or the provider, exactly like
`--target-direction`.

## Consequences

- **Good:** Renderer stays a pure function from IR + alignment → block-tree HTML. WASM track-C reuses it byte-for-byte. That is literal: the renderer's crate compiles for `wasm32-unknown-unknown` under a standing gate.
- **Good:** Each pane file is independently inspectable, diffable, and embeddable, and the two can share one page because each names its author ids in its own namespace.
- **Bad:** A consumer that deep-links into one pane file, or styles or scripts it by an author id, addresses the pane's prefixed id rather than the author's (`contracts.md` §4); the author's ids live in the published document.
- **Good:** Atomic-write contract maps to one file = one rename; the shell never partially overlays a previous run. That holds for every file and for any failure while staging; the rename pass is the one window, described under *What the bundle commit guarantees*.
- **Good:** The SCN-13 smoke driver can `curl` `source.html` or `target.html` and grep for `data-sync-id` without parsing a multi-pane composite document.
- **Bad:** Six files instead of one. Mitigated by the fact that the CLI ships them together, staged as one fileset, and that `transync serve` binds `127.0.0.1:7470` by default and serves the `--rendered` directory, path-confined to it, with a content-type table that covers exactly this bundle's file set — so the server that turns the six files back into one page is the same binary that wrote them. An external static server (e.g. `python3 -m http.server`) still works: the bundle is self-contained, which is the property that made the mitigation valid in the first place and is what the rejected **Three-file no-shell** option was measured against. `contracts.md` §6 carries the server's contract; `docs/project/stub-manifest.md` records the STUB-061 closure.
- **Bad:** Shell template lives in the CLI crate and is therefore CLI-only. WASM consumers will need to bring their own shell. This is acceptable: the shell is demo chrome, not part of the rendering contract.

## History

Every superseded passage and amendment is kept verbatim in
[`archive/adr-amendments.md`](archive/adr-amendments.md), under its *ADR-0006* heading.

- 2026-05-01 (Phase 3): decided — option 1, two fragment-shaped pane files plus a templated shell; five files.
- 2026-07-10 (OI-0001): sixth file `purify.min.js`; the shell sanitizes each fetched fragment and fails closed without it.
- 2026-07-13 (`12545e3`): `write_html_bundle` renamed and split into `html_bundle_files` and `write_fileset_atomic`.
- 2026-08-04 (DCR-0017): `render` moved to `transync-syntax`; one Comrak parse per pane; wasm32 standing gate.
- 2026-08-06 (R0001-0051): seams list corrected; placeholders all `TRANSYNC_*`; render seam outside the facade (DCR-0018).
- 2026-08-07 (OI-0018): seventh placeholder `{{TRANSYNC_CSP_META}}` for the opt-in `--strict-csp` bundle CSP.
- 2026-08-07 (ti 0f26b5, STUB-062/063): `{{TRANSYNC_TITLE}}` resolved from `--title` or the first H1; single-pass fill.
- 2026-08-08 (R0002-0010, R0002-0011, R0002-0059; DCR-0022): render seam returns `Result`; refuses repeated or missing rows; both panes pass intake.
- 2026-08-08 (R0003-0060, R0003-0078; DCR-0025): unusable byte ranges refused (`UnusableRange`); `block_text` stops clamping.
- 2026-08-09 (ticket b791d6, STUB-061): `transync serve` became a real loopback server; an external server is optional.
- 2026-08-12 (R0004-0100): "atomic" scoped to a staging guarantee; set-level atomicity is `--out-dir`'s.
- 2026-08-16 (ti 1c79f8): a correction under the **Bad: six files** bullet routed readers to the 08-09 and 08-12 changes.
- 2026-09-03 (DCR-0038): HTML-run panes render from row ranges by attribute injection, with no Comrak parse.
