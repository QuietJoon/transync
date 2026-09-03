---
type: DCR
title: The HTML document intake lands, and identity becomes a theorem rather than a test hope
description: Wave 3 of the HTML→HTML feature adds transync-syntax::intake::html — spec §4's five-class classification with default STOP, rule T anonymous text runs, whole-element ranges, emission-order ids, shared heading scopes and force-close warnings — so an HTML document goes in and comes out byte-identical with a real block set, real ids and a real alignment map. No LLM, no pipeline change, no core change. The parser→intake::markdown rename is deferred on record.
tags: [change, project-control, DCR-0035]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-03T00:00:00Z
status: stable
---

# DCR-0035: The HTML document intake lands, and identity becomes a theorem rather than a test hope

- **Date:** 2026-09-03 — this record, and the code, which landed the same day:
  `949ec94` (the module, the fixtures, the spine, the three welds), `e257937`
  (rule T's fine grain and the five-class table pinned per element), `1a977e2`
  (warnings, hostile input, the invariant tripwire), `390051e` (ids, sections,
  paths and the alignment map). Read from `git log --date=short` over those
  commits, **not** from the plan's filename: the plan is dated 2026-08-20,
  which is its writing date — the rule DCR-0033 wrote down after waves 0 and 1
  were both mis-dated that way.
- **Source:** `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
  (ratified design spec), ticket `490d97`, **wave 3** of the eight in its §12.
  Plan: `docs/superpowers/plans/2026-08-20-html-wave3-intake-round-trip.md`.
- **Affected ADRs:** `docs/decisions/0025-html-to-html-document-translation.md`
  (this is D3, D4, D5, D8 and D9 as landed).
- **Affected DCRs:** `DCR-0032` (the mechanics crate this consumes and does not
  modify); `DCR-0034` (wave 2's IR split, which this is the first *producer*
  of — `Spelling::Html`, `Document.format`, `BlockKind::Title`);
  `DCR-0041`/`DCR-0042`/`DCR-0043` (the three HTML-content scoping fixes that
  landed **between** the plan's writing and its execution, and moved three of
  its premises — see "Four premises moved under the plan" below).
- **Numbering note.** This record takes **DCR-0035** because wave 3 reserved it,
  not because it was next free on disk. `DCR-0032`–`DCR-0039` are one per
  HTML→HTML wave, and waves run partly in parallel, so "the next free number"
  is a race.

## What changed

`crates/transync-syntax/src/intake/html.rs` — THE HTML document intake, and
`intake.rs`, the format seam that names it. One linear pass over `scan_tags`'
token stream, steered positionally by `element_extents`' precomputed pairing
verdicts:

- **Spec §4's five classes, default STOP:** PASS-THROUGH (17 wrappers, plus the
  D9 list containers passing through *to their items*), STOP → semantic kind
  (`h1`–`h6`, `p`, `figcaption`, `table`, `pre`, `blockquote`, `hr`, and `li`
  under a list), VERBATIM (`script`, `style`, `link`, `meta`, `base`), PHRASING
  (48 names, absorbed into text runs and never a boundary), and DEFAULT-STOP →
  `BlockKind::Html` for everything else, including custom and future elements.
- **Rule T:** maximal anonymous runs between hard boundaries, tested on BYTES
  outside tag and `Skip` spans with U+FEFF stripped, trimmed of leading and
  trailing whitespace, BOM and whole `Skip` spans. Text → one `Paragraph`;
  textless with one or more `img` → one `Image`; textless without → gap. Never
  in head mode, end of input included.
- **Whole-element ranges,** tags included: the close tag's end when closed, the
  boundary minus trailing ASCII whitespace when force-closed, the tag span alone
  when the element has no extent.
- **Ids, hashes, sections, paths:** one global ordinal in emission order,
  `source_hash_bytes` over the exact range, THE shared `SectionStack` (not a
  copy), and comrak's child-index `ast_path` convention so
  `walk::normalize_top_level`'s prefix collapse stays total.
- **`<title>` in head mode** → `BlockKind::Title` with an EMPTY `section_path`;
  a duplicate title yields two honest blocks; an `<svg><title>` is run content
  and never `Title`.
- **Warnings, not panics:** implicit closes, force-closes on unclosed leaves and
  EOF force-closes each append a `Document::warnings` note naming the block.
- **The debug-asserted invariants** — source order, non-overlap, in-bounds,
  UTF-8 boundaries — checked on every `parse`.

Two crate-internal visibility widenings so both intakes share one rule each:
`parser::normalize_source` (the NUL → U+FFFD contract) and `parser::sections`
(the heading-scope stack). Neither is visible outside `transync-syntax`.

New fixtures: `crates/transync/tests/fixtures/scn-16-html-document.html` (spec
§11 row 1, shared forward to wave 5's scenario test) and the two-page identity
corpus under `crates/transync-syntax/tests/fixtures/html-corpus/`.

## Why

Wave 3 is the feature's first demonstrable milestone: an HTML document goes in
and comes out **byte-identical**, with a real block set, real ids and a real
alignment map — no LLM, no pipeline change, no core change.

## Identity is a theorem, not a test hope

`regen::regenerate` copies inter-block gaps verbatim and splices
`source_text[range]` for every block absent from the accepted map. So
`regenerate(parse(s), &empty) == parse(s).source_text` holds **exactly when**
the intake's ranges are in source order, non-overlapping and in bounds — which
is what `debug_assert_block_invariants` asserts on every parse. The identity
test does not hope; it reads off a property the producer guarantees.

The corollary is the trap, and it shaped every test in the wave: a `parse` that
finds **zero blocks** makes `regenerate` reproduce the whole source as one gap,
so a byte-identity assertion **alone is unfalsifiable**. Every spine test
therefore pairs the round trip with a kind-sequence or block-set assertion in
the same test, and the module landed with `flush_run` deliberately classifying
nothing — a staged red where byte-identity was already green and three
kind-sequence assertions failed with real diffs. That red is the demonstration;
an identity-only test must never be added.

## Four premises moved under the plan, and the plan said to re-derive

The plan was written 2026-08-20 and executed 2026-09-03. Its own Global
Constraints flag three passages as `SUPERSEDED` by ti `490d97` wave 1 and
instruct the implementer to **re-derive rather than transcribe**. Doing so
changed what the shipped comments say, not what the code does:

1. `open()` still leaves `self_closing` unread — but now because
   `element_extents` already models HTML's two honouring sites (foreign content,
   and the `<svg>`/`<math>` start tags that enter it), **not** because `<div/>`
   mints no extent. It does mint one, and it opens.
2. The PASS-THROUGH branch takes the container path for `<div/>` too. The
   `if let` there is the foreign-content carve-out, since no pass-through name
   is void.
3. `emit_extent_leaf`'s no-extent arm is a void name in HTML content or a
   self-closing tag in foreign content — not "a self-closing spelling".

A **fourth** premise moved after the plan was written, and the plan does not
mention it: ti `c1f9a8` gave `TagToken::Skip` two more fields (`kind`,
`terminated`), so the plan's `Skip { span }` patterns do not compile. They are
`Skip { span, .. }` here, and `token_span`'s doc records why neither new fact is
read — an unterminated trailing region is gap either way, because rule T trims
whole `Skip` spans, and repairing one is `balance_fragment`'s job at a wrapper
seam. The variant list is still matched exhaustively, so a new *token* would
break the build.

One premise moved in the intake's favour: `is_phrasing("img")` now covers a
source `<image>` for free, because ti `e923ef` renames it in HTML content the
way HTML's tree builder does, with the span still over the original bytes.

**One pin had to be re-derived, and it is the plan that was stale, not the
code.** The plan predicted `<div>Price: <my-price/> today</div>` → three blocks
(`Price:` / `<my-price/>` / `today`). It gives two: `Price:` and one
DEFAULT-STOP block over `<my-price/> today`. Pre-wave-1, `<my-price/>` minted no
extent, so the stop block was the tag span alone and ` today` became a third
block — a paragraph sitting OUTSIDE the element a browser says contains it.
Since wave 1, `<my-price/>` opens, `</div>` closes it implicitly, and its whole
extent is one block. The spec sentence the pin exists for still holds — the
sentence is still severed at the custom element, which is what §14 carries as a
review item — but the severed tail is no longer misattributed.

## The plan's blocking precondition, discharged

The plan carried a hard precondition added 2026-08-24 by design review: the
foreign-content breakout gap (ti `e77173`) "must be discharged before this
wave's corpus is blessed", because this intake consumes the same walk through
`element_extents` and would otherwise bless a corpus over block boundaries a
browser disagrees with — re-deriving ids and section paths for that page class
later, and ids are the project's only sync currency. It is discharged:
`DCR-0041` landed the three-valued content mode 2026-09-01. The corpus here is
blessed over a walk that agrees with a browser about breakout tags and
integration points.

## The deferred rename, on record

Spec §4 sketches today's `parser` as `intake::markdown` and delegates the path
decision to the implementation plan. **The rename is deferred.** Its measured
blast radius: ~43 `crate::parser` references across 11 files in
`transync-syntax`, ~74 `crate::parser` / `transync_syntax::parser` references
across 18 files in `transync-core` (including the crate-private re-export
`pub(crate) use transync_syntax::parser;` that keeps `crate::parser::…`
resolving inside core), `transync-wasm`'s import and doc paths,
`public_surface.rs`'s hidden-path pin naming `"parser"`, the module map, the
source-of-truth table and `CLAUDE.md` — 30+ files for zero behaviour, in the
same season wave 2 already rewrote many of them. A wave whose deliverable is
*new* behaviour must keep its diff readable as new behaviour.

The seam is real without it: both intakes produce the same `Document`, normalize
NUL through the same `parser::normalize_source`, stamp ids with the same
global-ordinal scheme, and share THE heading-scope stack. The asymmetric shape
(`intake::html` beside `parser`) is recorded here and in `intake.rs`'s module
doc so it reads as a decision rather than an accident. A future wave that
renames pays the mechanical cost in a diff that is *only* that.

One wrinkle recorded in the same module doc: `parser::intake` — the NUL/nesting
guard **function** (OI-0034) — predates this module and shares the word.

## The amended img exception, on record, with a spec amendment owed

Spec §4's literal wording — "a textless run whose only non-whitespace content is
one or more `img` elements" — would classify `<a href="…"><img …></a>` as gap:
the `<a>`/`</a>` tag bytes are non-whitespace outside the `img` spans. That
reading silently strips the sync anchor **and** the alignment row from every
linked badge, logo and thumbnail — the dominant image idiom on exactly the
corpus-class pages this wave covers.

The wave implements the reading consistent with the spec's own stated rationale
for the exception ("so images keep a sync anchor instead of dissolving into
gap") and with PHRASING's definition ("absorbed into text runs, **never a
boundary**"): a run that is textless **after excluding absorbed phrasing markup
and `Skip` spans** and contains one or more `img` elements becomes ONE `Image`
block, wrappers included. A run with actual text stays a `Paragraph`, linked or
not. A textless run with no `img` at all stays gap.

Pinned by `a_linked_image_keeps_its_anchor_as_one_image_block` and
`an_img_with_real_text_is_still_a_paragraph`.

**Post-implementation review item (§14-style):** amend spec §4's exception
sentence to match the implemented reading. The spec file was deliberately not
edited in this wave; the amendment is recorded here as **owed**.

## Evidence

- `html_intake_identity.rs` — byte-identity paired with block-set assertions
  over the SCN-16 fixture, a messy marketing page, an unclosed docs fragment,
  and the CRLF / lone-CR / BOM / NUL / doctype-only / plain-text edge documents.
  5 passed.
- `html_intake_alignment.rs` — `build_alignment_map` over a parsed page: a row
  for every block, real ranges on both sides, `document_id` unchanged, the D5
  title row `non-sync`, the prefix collapse total. 3 passed.
- `intake/html.rs`'s in-module pins — rule T (10), the five-class table (5),
  head mode and `<title>` (4), warnings and hostile input (3), the invariant
  tripwire (4, three of them `#[should_panic]` on the assertion's own message).
- `cargo test --workspace -- --test-threads=4`: 46 binaries, 1214 passed, 0
  failed, `CARGO_EXIT=0`. Two-package `wasm32` check: `CARGO_EXIT=0`. `fmt` and
  `clippy --all-targets --all-features -D warnings`: clean.
- Zero edits to pre-existing fixtures or expectations, over a corpus selector
  proved to select 16 files (`git diff -- 'crates/*/tests/fixtures/*'` is not
  vacuous — the trailing `/*` matters, ti `490d97` wave 1).

## What this wave deliberately did NOT do

It made **no HTML translation run possible**. Spec §12's hard rule: no HTML
translation run ships before the layer-6 twin exists (wave 4). `translate()`,
`TranslateOptions`, every refusal, every prompt and `html_dominance_warning` are
untouched — and that warning's "HTML-to-HTML translation is not implemented
(ti 490d97)" tail is still **TRUE** after this wave: intake exists, translation
does not. It is re-texted in the wave the *entry point* lands (spec §6).

Verified mechanically: the diff from this wave's baseline commit touches no file
under `crates/transync-core`, `crates/transync-cli`, `crates/transync-openai`,
`crates/transync-anthropic`, `crates/transync-wasm`, `crates/transync-html` or
`web/`.

## Hand-forwards

- **Wave 4** — `validate::full_rescan_html` (the layer-6 twin) and the one
  format branch in `pipeline::finalize`. The twin is the gate before any
  translation run.
- **Wave 5** — units, context, prompt and cache over HTML documents, including
  what `html_outcomes`' per-block outcomes mean for batching. Reuses the SCN-16
  fixture from its canonical home.
- **Wave 6** — the alignment wire's `source_format` / `input_format` fields
  (schema 1.2.0 → 1.3.0, additive), `--input-format`, `out.html`, and the
  refusal re-text.
- **Wave 7** — the SCN-16 scenario-matrix row and Playwright pane-sync coverage.
- **Not this feature's:** attribute-text translation (the standing
  `html-attribute-text-translation` deferral), and wasm demo support for HTML
  documents.

Do **not** add `intake` re-exports to `transync-core` or the facade: wave 5
decides how core reaches the intake, and `public_surface.rs`'s `forbidden` array
now pins `"intake"` as never-documented surface.
