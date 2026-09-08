---
type: DCR
title: The format seam becomes symmetric — the Markdown intake is intake::markdown
description: DCR-0035 deferred the parser→intake::markdown rename with a measured blast radius and one condition — a future wave pays the mechanical cost in a diff that is only that. This is that diff. Eight files move under intake/markdown/ by git mv, ~300 path references follow across four crates and eleven documents, and no behaviour changes. transync-core keeps a leaf re-export so it still reads crate::markdown::…; public_surface.rs keeps "parser" as a re-introduction guard beside the already-retired "htmlseg"; and the intake::markdown::intake collision is left alone as a separate decision.
tags: [change, project-control, DCR-0053]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-07T00:00:00Z
status: stable
---

# DCR-0053: The format seam becomes symmetric

- **Date:** 2026-09-07 — this record, and the code, in one commit: `18c1ab4`.
- **Source:** ticket `b7e4cc70`; `docs/backlog.md` Type 1
  `parser-intake-markdown-rename`; ti `490d97` wave 3 deviation 1.
- **Affected DCRs:** `DCR-0035`, whose "The deferred rename, on record" section
  this discharges. That section is **not rewritten** — it is correct as written
  for 2026-09-03, and its measured blast radius is the estimate this change
  tested.
- **Affected ADRs:** `docs/decisions/0025-html-to-html-document-translation.md`
  (D3 — "the *format* seam is a module boundary inside `transync-syntax`,
  `intake::markdown` beside `intake::html`"). D3 is now implemented as written;
  the ADR needs no amendment, because the shape it specified is the shape that
  landed.
- **Behaviour changed:** none. Not one branch, constant, string or wire byte.

**Numbering note.** DCR numbering is independent of ADR numbering. DCR-0035–0039
remain reserved by the HTML→HTML waves; this is the next free number after
DCR-0052.

## What this record is for

A rename is not normally worth a DCR. This one is, for two reasons.

It **closes a design boundary that a DCR opened**. DCR-0035 did not merely
postpone work — it recorded the asymmetric shape (`intake::html` beside
`parser`) as a *decision*, so that a reader would find a reason rather than an
accident. Removing that asymmetry removes the reason with it, and a boundary
that was worth writing down when it appeared is worth writing down when it goes.

And it **carries the four judgement calls a mechanical rename does not make for
you** — the re-export granularity, the guard list, the living/snapshot line
across the document tree, and the one collision the rename creates and does not
resolve. Each is recorded below, because each is a place the next reader could
otherwise conclude something was overlooked.

## What changed

`crates/transync-syntax/src/parser.rs` and its seven children move to
`crates/transync-syntax/src/intake/markdown.rs` and
`crates/transync-syntax/src/intake/markdown/{classify,depth,emit,options,ranges,refdefs,sections}.rs`,
by `git mv`, keeping the file-as-module layout. Roughly 300 path references
follow across four crates and eleven documents.

The seam now reads the way spec 2026-08-20 §4 sketches it and the way ADR-0025's
D3 specifies it: `intake::html` is the HTML document intake, `intake::markdown`
the Markdown one, siblings under the module that names the seam.

**Three files are hand-edited rather than mechanically rewritten**, and they are
the whole of the non-mechanical diff:

- `src/lib.rs` drops `pub mod parser;`. The crate root now declares eight
  modules — `align`, `error`, `id`, `intake`, `outcome`, `regen`, `render`,
  `walk` — and `docs/architecture/source-of-truth-table.md`'s enumeration, which
  `crates/transync/tests/docs_ownership_drift.rs` welds to that list, already
  names exactly those eight.
- `src/intake.rs` loses its deferral paragraph, which this change makes false,
  and gains the symmetric statement in its place.
- `src/intake/markdown.rs` drops one comment that named its own old filename.

Everything else is the transform. That was **verified mechanically rather than
by reading**: the rename transform was replayed over each file's `HEAD` content
and diffed against the working tree, and all 47 remaining `.rs` files are
byte-identical to the transform's output. Git's own rename detection agrees —
all eight moves are reported at 94–100% similarity.

## The four judgement calls

### 1. `transync-core` keeps a LEAF re-export

`transync-core`'s crate-private re-export becomes
`pub(crate) use transync_syntax::intake::markdown;`, so core still reads
`crate::markdown::…`. The alternative — re-exporting the parent, so core would
read `crate::intake::markdown::…` — was rejected on three grounds:

- Core's re-export block binds every syntax module by its **leaf** name
  (`align`, `id`, `regen`, `render`). The leaf name changed from `parser` to
  `markdown`; the binding shape did not. That is the pure rename.
- Re-exporting the parent is a re-export-*granularity* change sitting on top of
  the rename, which is the kind of thing the "diff must be only the rename"
  condition exists to keep out.
- Core already reaches the HTML intake fully qualified, as
  `transync_syntax::intake::html::parse`, at thirteen call sites. Binding
  `crate::intake` would give those a second spelling.

The block is alphabetically ordered and `markdown` sorts into the slot `parser`
vacated (`llm` < `markdown` < `pipeline`), so no line moved.

### 2. `public_surface.rs` is deliberately unchanged

`crates/transync/tests/public_surface.rs` keeps `"parser"` in the
hidden-module list, so that file has **zero diff**.

The decisive precedent is inside that same list: `"htmlseg"` — a module that has
not existed since DCR-0032 extracted it into `transync-html` — is still there.
The assertion these entries make is that no `contracts.md` §0 documented path
contains the segment. That assertion is still true, still guards, and nothing in
the file claims either module exists. Each entry is a **re-introduction guard**,
not an inventory. `"intake"` already covers the new path.

### 3. Living documents moved; dated records did not

Reference and explanation documents that state *current* facts were updated:
`module-map.md`, `source-of-truth-table.md`, `architecture/README.md`,
`scenario-matrix.md`, `Troubleshooting.md`, `Developer_Guide.md`,
`contracts.md`'s living tier-(c) list and §6 nesting contract, three manual
`resource:` paths, three `Cargo.toml` comments, and one comment in
`web/tests/wasm.spec.js`.

Dated records were left exactly as written, per the standing rule that they are
correct for their own date and are amended by appended notes rather than
rewritten: every ADR and DCR (including DCR-0035 itself), the dated plans and
specs, past CHANGELOG entries, `open-issues.md`'s two hits (both inside the
RESOLVED-and-dated OI-0037 and OI-0040 records), `stub-manifest.md` and
`implementation-slice-checklists.md` (each of which self-declares that its
source paths are as of its own date), `skeleton-plan.md`, and
`contracts.md`'s four `**Recorded surface decision — …**` blocks, each stamped
with its own owner-decision date and ticket.

The local precedent for that line is `mvp-scope.md`, whose dated DCR-0017
section still says `htmlseg` and was not rewritten when DCR-0032 retired the
name.

`docs/investigation/` is out of scope by a stronger rule than judgement: it is
gitignored, regenerated from the code, and excluded from `docs_index_drift.rs`
for exactly that reason.

### 4. The `intake::markdown::intake` collision is left open

The NUL/nesting guard **function** is now reachable as
`crate::intake::markdown::intake`. The path says "intake" twice, and it reads
badly.

It is deliberately not renamed here. The collision predates the seam module —
`intake.rs`'s doc has carried the wrinkle note since DCR-0035, and OI-0034 is
its home — and renaming a public function is a surface change, not a move. It
needs its own decision and its own record; folding it in would have made this
diff something other than the rename.

## Verification

Every gate captured bare-to-file with an exit marker appended after the native
status, under `/Volumes/Temp/claude/gate/rename/`:

| Gate | Result |
| --- | --- |
| `cargo fmt --all -- --check` | `FMT_EXIT=0` |
| `cargo check --workspace --all-targets` | `CHECK_EXIT=0`, zero warnings |
| `cargo clippy --all-targets --all-features -- -D warnings` | `CLIPPY_EXIT=0`, zero warnings |
| `cargo test --workspace -- --test-threads=4` | `CARGO_EXIT=0` — 47 targets, **1,359 passed, 0 failed, 8 ignored** |
| `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown` | `WASM32_EXIT=0` |
| rustdoc library leg, `-D warnings` | `RUSTDOC_LIB_EXIT=0`, zero warnings |
| rustdoc bin leg, `--document-private-items -p transync-cli` | `RUSTDOC_BIN_EXIT=0`, zero warnings |
| `biome format` | `BIOME_EXIT=0` |

The test tally is **identical to the pre-rename baseline** — same 47 targets,
same 1,359 passing, same 8 double-gated ignores. For a change that claims zero
behaviour, an unchanged tally is the claim's evidence.

`cargo fmt --all` was applied, not merely checked: the new path is nine
characters longer than the old one, which moved import sort order and pushed
several call sites past the width limit. That reflow is the formatter's, and it
is why the diff shows a few lines that are not literal substitutions.

Two welds were reasoned about before they ran, and both passed:
`docs_ownership_drift.rs` (the crate root's eight modules against the
source-of-truth table's enumeration) and `docs_index_drift.rs` (this record
against `docs/index.md`).

## Consequences

- **Good:** the seam is symmetric, so a reader who finds `intake::html` finds
  `intake::markdown` next to it rather than a cross-reference to a differently
  named module. DCR-0035's asymmetry note stops being load-bearing.
- **Good:** the deferral is discharged the way it specified — one diff, only the
  rename — so the move is reviewable as a move.
- **Neutral:** `transync-syntax` is a published crate and `parser` was a `pub
  mod`, so this is a path change for anyone depending on it directly. It rides
  the same unopened window as `LineOffsets::offsets` (R0010-0023), and is
  recorded in `CHANGELOG [Unreleased]` alongside it. The `transync` facade is
  unaffected: `public_surface.rs` pinned the module hidden, so
  `transync::parser::…` never resolved.
- **Bad:** `intake::markdown::intake` now says "intake" twice. Recorded above,
  owned by OI-0034, and deliberately not fixed here.

## Appended note (2026-09-08) — the collision's owner, corrected

The line above, and the two register sentences written with it, said the
collision was **owned by OI-0034**. That is wrong, and the 2026-09-08
documentation drift audit found it.

OI-0034 is *comrak's NUL→U+FFFD substitution desyncs byte columns* — **RESOLVED
2026-08-06** and archived to `open-issues-archive.md`. It owns the NUL
normalization the guard *performs*, which is exactly what every `TRACE: OI-0034`
in the code correctly cites. It has never owned a naming decision, and being
resolved and archived it cannot acquire one. So between 2026-09-07 and
2026-09-08 the collision this record created had **no live owner** — the record
said it was tracked, and it was not.

It has one now: **OI-0054** in `docs/project/open-issues.md`, ticket
`cdadffea`, with a paired `docs/backlog.md` entry
(`intake-markdown-intake-name-collision`). The entry records the option this
record did not consider: check whether any external caller needs the function at
all, because making it crate-private would remove the surface question and may
make the rename non-breaking. If it stays public, the rename rides the same
unopened window as `LineOffsets::offsets` and this record's own change.

The body above is otherwise correct as written and is not revised.
