---
type: DCR
title: Semantic kind and source spelling become separate axes in the block IR
description: Wave 2 of the HTML→HTML feature opens the sanctioned v0.5.0 window and lands four breaking-by-policy IR changes batched — Spelling and SourceFormat join BlockKind, BlockKind::Html narrows to a unit variant, BlockKind::Title is added anchor-less, and every html dispatch site is re-keyed onto the axis it actually meant. Breaking by policy, behaviour-preserving in fact: zero fixture edits.
tags: [change, project-control, DCR-0034]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# DCR-0034: Semantic kind and source spelling become separate axes in the block IR

- **Date:** 2026-09-01 — this record. The **code landed 2026-08-24**, in one
  day: `20b78b7` (the two vocabularies), `258ba01` (the IR stamps), `954711a`
  (`BlockKind::Html` narrowed), `1ed72d3` (the re-keying), `8b556e7`
  (`BlockKind::Title` and its two explicit arms) and `ff788f7` (the version).
  Both dates are read from `git log --date=short` over those commits, **not**
  from the plan's filename: the plan is dated 2026-08-20, which is its writing
  date, and waves 0 and 1 were both mis-dated that way before DCR-0033 wrote the
  rule down.
- **Source:** `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
  (ratified design spec), ticket `490d97`, **wave 2** of the eight in its §12.
  Plan: `docs/superpowers/plans/2026-08-20-html-wave2-ir-split.md`.
- **Affected ADRs:** `docs/decisions/0025-html-to-html-document-translation.md`
  (this is D2, D5 and D12 as landed).
- **Affected DCRs:** `DCR-0032` (the mechanics crate this sits on top of);
  `DCR-0031` (the `CodeBlock.fenced` window, whose recording shape this copies);
  `DCR-0040` (which removed a second, unrelated breaking change under the same
  window).
- **Numbering note.** This record takes **DCR-0034** because wave 2 reserved it,
  not because it was next free on disk. `DCR-0032`–`DCR-0039` are one per
  HTML→HTML wave, and waves run partly in parallel, so "the next free number" is
  a race. `DCR-0040` is out of sequence for the same reason, from the other
  direction.

## Breaking by policy, behaviour-preserving in fact

Four changes ride one sanctioned v0.5.0 window, batched. Each with what a
consumer actually sees:

1. **`parser::Block` gains `pub spelling: Spelling`.** Tier **(c)** — `parser`
   is a hidden module and the facade re-exports nothing from it, so only a
   consumer depending on `transync-syntax` *directly* is affected, under the
   weaker promise `contracts.md` §0 tier (c) already grants. A consumer
   constructing a `Block` by struct literal must supply the field.
2. **`parser::Document` gains `pub format: SourceFormat`.** Same tier, same
   shape. `Document` derives `Default`, so `..Document::default()` literals keep
   compiling; `SourceFormat`'s `Default` is `Markdown`, which is what an empty
   document always was.
3. **`BlockKind::Html { block_type: u8 }` narrows to the unit variant
   `BlockKind::Html`.** A consumer's `BlockKind::Html { .. }` pattern still
   compiles — a braced pattern on a fieldless variant is legal, and it is now
   *misleading* — but `Html { block_type }` does not, and neither does
   constructing the variant with a field.
4. **`BlockKind::Title` is added.** Every **exhaustive** `match` over `BlockKind`
   in a consumer's tree stops compiling. That is the cost the exhaustive-by-policy
   rule exists to make visible rather than silent.

`BlockKind` is a `contracts.md` §0 **tier-(a)** row, so changes 3 and 4 are
facade-visible; §1's stability bullet records them, and §0 carries the prose
paragraph, because §0's path table structurally cannot see a field- or
variant-level change ("No other type's fields are covered").

**Why per-block spelling rather than a document-level bit:** the Markdown
parser's `NodeValue::HtmlBlock` arm interleaves HTML-spelled blocks with
Markdown-spelled ones inside one document. No document-level flag can carry that
axis. `Document.format` does a different job — it lets a format-committed
consumer *refuse* the wrong document instead of silently believing whatever node
sequence comrak returns for an HTML page.

## One thing that DID move: `BlockKind`'s serde encoding

An html block now encodes as `{"kind":"html"}` where it encoded
`{"kind":"html","block_type":6}`. This is a derive-level consequence of change 3
— `BlockKind` carries `#[serde(rename_all = "kebab-case", tag = "kind")]`, so
dropping the field drops the key.

**It is inert**, and the sentence that makes it inert is already in the tree:
`id.rs`'s `CodeBlock::fenced` doc comment states that *no shipped artifact
serializes this enum* — "alignment rows and cache keys carry
[`BlockKind::wire_str`] strings, and `TranslationUnit` is not `Serialize`". So
no wire, no alignment map, no cache entry and no disk-cache record carries the
enum form. The encoding that moved is one nothing reads.

It is stated here anyway, deliberately. *"No wire shape moved"* is this wave's
headline claim, and a derive-level encoding change that nobody records is
exactly the kind of thing a future reader finds on their own and then mistrusts
the whole record for.

## What did NOT move — which is the wave's actual claim

- **No `id_code` changed** for any pre-existing kind, so no block id moved.
- **No `wire_str` changed** for any pre-existing kind. `Title` takes the fresh
  label `"title"`, which collides with nothing in the existing prefix set.
- **No alignment `schema_version`** — still 1.2.0.
- **No `VALIDATION_SCHEMA_VERSION`.**
- **No prompt or instruction bytes**, so no `instruction_hash` moved.
- **No cache axis** was added or re-keyed.

The corpus regenerates, renders and aligns byte-identically, with **zero fixture
edits** — not one byte under `crates/*/tests/fixtures/`, and not one *expected
value* in an existing assertion. That was the wave's acceptance gate, checked
with a real diff over the recorded pre-wave baseline (`6f7b3c4`) rather than
asserted.

Two test *inputs* were strengthened, and they are named here so the zero-edit
claim stays exactly what it says. `validate::inline`'s
`html_units_skip_the_inline_layer` got a source payload carrying a raw tag,
because after the re-key it would otherwise have passed whether or not the skip
fired — measured, not argued: with the html half of the skip disabled the new
test fails and a probe carrying the old payload pair verbatim passes.
`validate::schema`'s `a_nul_free_payload_passes_on_every_kind` became
`…_in_every_input_mode` and iterates modes, because `check_payload_bytes` no
longer takes a kind. **No expected value changed in either.**

## The two catch-alls, named

The variant addition is the part that could have shipped a wrong *answer* rather
than a compile error, because two shipped matches end in a catch-all and would
have swallowed `Title` silently:

- **`align::sync_role_for` ends `_ => SyncRole::Anchor`.** Without an explicit
  arm, an HTML document's `<title>` would have become an **anchoring** alignment
  row — the precise opposite of D5, and a row pointing at a DOM element that
  does not exist.
- **`unit::context::document_title` matched `BlockKind::Heading1` alone** — not
  "any heading", verified. Without an explicit arm, a page title would not have
  entered the document title at all, and the run's most valuable context string
  would have silently come from the first body heading instead.

Both got explicit arms, and both got tests that assert the **answer**, not the
compile: each was observed failing on a *value* (`Anchor` vs `NonSync`;
`Some("Heading one")` vs `Some("Doc name")`) before its arm existed. A plan that
trusted "the compiler walks us to every match" would have shipped both defects
green.

`walk::label_for` and `payload::input_mode_for` are the same shape and are
*fine* — `other => other.wire_str()` gives `"title"`, and a `Title` unit never
takes the Markdown arm of `assemble` — but `label_for`'s pin was extended by
hand (15 rows → 16), because the array's declared length is what turns a missing
row into a compile error.

**No `_ =>` arm was added to any match over `BlockKind`.** The two above are
pre-existing and stay; a third would have been the next silent swallow.

*Amended 2026-09-01, the same day this record landed: `align::sync_role_for`'s
catch-all was removed by **DCR-0044** (commit `cec5bc0`, tickets `18b9c3` /
`9ffb97`) — it names every `BlockKind` variant explicitly now, and the
alignment row it emits is what decides whether a block anchors.
`unit::context::document_title`'s shape is the one that stays: it is a
membership test, not a dispatch that assigns behaviour per variant.*

## The re-keying, with its warrant

Nine predicates moved off kind-`Html` and onto the axis each one actually meant:

- **`Spelling::Html`** for IR-side questions — `regen`'s two format-aware arms,
  `outcome::html_outcomes` / `is_translatable_block`, `align`'s
  extraction-failed arm, `unit::html_dominance_warning`'s html-mass predicate.
- **`InputMode::HtmlSegments`** for unit-side questions —
  `validate::per_kind`'s new first arm, `fragment_reparse`'s early return,
  `schema::check_payload_bytes`, `inline`'s skip set, `llm::prompt::has_html_unit`.
- **`constraints.html`** for the layer-3 splice in `validate`, with the mode kept
  as a `debug_assert` twin.

**Behaviour-preserving today by three-way set identity:** kind-`Html` ⇔
mode-`HtmlSegments` ⇔ spelling-`Html` all hold on the Markdown path, which is
the only path that exists. The point is what happens the day they stop being
identical — when an HTML document's `<p>` is a `Paragraph` (semantic kind) that
ships a segment array (input mode) and was spelled in HTML. On that day every
one of these predicates has to answer on its own axis, and re-keying them
afterwards would mean re-deciding nine questions under pressure.

`is_translatable_block`'s branch order flipped so that the kind-level exclusions
became the **Markdown** arm rather than the universal rule.
`align::sync_role_for`'s extraction-failed arm is not in the spec's own re-keying
list and moved anyway: after wave 3 an HTML `<p>` whose extraction failed would
otherwise fall through to `Preserved` — an alignment row claiming a block was
carried over intact when it was not.

The GFM row-window table splitter now excludes html-segments units **explicitly**,
with a test. `unit::split::inspect_table` would have refused one anyway; an
accident is not a guard.

## The sentinel

`HtmlSegmentConstraints::block_type` keeps its `u8` and gains the value **`0`**,
meaning "a block of an HTML document", where no CommonMark type applies.

`Option<u8>` was never available: `HtmlSegmentConstraints` is a §0 tier-(a) row
without `#[non_exhaustive]`, so its field *types* are frozen between windows, and
this window was already spending its budget on `BlockKind`. `0` is the value that
does not claim to be a CommonMark type (the domain is 1–7), and it behaves
identically to `1` under the splice's `matches!(t, 6 | 7)` rule — anything
outside `6 | 7` keeps its blank lines. It is a **record of what the source was**,
not the switch the splice reads: the policy is derived from the block's spelling
through `Spelling::blank_line_policy_for`, which is the one place the
HTML-document case is decided. A doc-comment domain widening, not a type change.

## Evidence

- **Workspace suite green with zero fixture edits.** At the wave's close
  (`ff788f7`): **1116 passed / 0 failed** across 40 binaries, `CARGO_EXIT=0`,
  from a pre-wave baseline of 1105 — the delta is the wave's own new tests.
  Re-verified at this record's tree: **40/40 binaries, 1123 passed / 0 failed,
  6 ignored**, `CARGO_EXIT=0` (the further delta is the review-0009 fix pass,
  not wave 2).
- **CLI stub suite green:** `cargo test -p transync-cli --features
  test-stub-provider` — **208 passed / 0 failed**, `CARGO_EXIT=0`.
- **The two-package wasm gate exit 0, with its command string unchanged:**
  `cargo check -p transync-syntax -p transync-wasm --target
  wasm32-unknown-unknown`. `Spelling::blank_line_policy_for` names a
  `transync_html` type, and that edge already existed and already compiled for
  `wasm32`, so no package joined the command. `transync-syntax` gained no
  `[features]` table and no `transync-core` dependency.
- **`workspace_publication.rs` green at `0.5.0-dev`** across the
  `[workspace.package]` version **and all five** `[workspace.dependencies]`
  internal requirements (`transync-html`, `transync-syntax`, `transync-core`,
  `transync`, `transync-openai`), which do not inherit it and were moved in the
  same edit.
- **`public_surface.rs` green.** No §0 row moved: `transync::BlockKind` is still
  exactly that path, which is precisely why the two `BlockKind` changes needed a
  prose paragraph in §0 rather than a table row.
- **Corpus unmoved**, checked with a selector proved non-vacuous first:
  `git ls-files` selects **16** fixture files, **14** scenario files and **5**
  browser tests, and `git diff --stat 6f7b3c4..HEAD` over the fixture and
  scenario pathspecs is empty (`6f7b3c4` is the pre-wave baseline). The `/*`
  suffix is load-bearing — a pathspec ending at `fixtures` matches **nothing**,
  and its silence reads exactly like a clean diff. Over `web/tests` the same
  range is *not* empty, and the reason is not this wave: `88964df` and `69701cd`
  (the review-0009 fix pass, 2026-08-26) added engine-refusal cases and extended
  the harness. Diffed over **wave 2's own commit range** (`20b78b7~1..ff788f7`),
  all three pathspecs are empty — the wave touched no fixture, no scenario and
  no browser test at all.

## Handed forward

- **The `Document.format` refusals.** The field lands here as a fact and an
  invariant, not as a gate: nothing constructs a `format == Html` document until
  wave 3's intake exists, so a refusal written now would be untested code
  claiming to be a guard. Wave 4 branches on it in
  `finalize_regen_with_reparse_policy`; wave 6 does the render/pane half.
- **`document_title` and the heading-text projections through `extract`** —
  wave 5, when an HTML document's title actually has HTML markup in it.
- **`SourceFormat`'s §0 row and the schema-1.3.0 wire fields** — the row lands
  in the same commit as the export (wave 5's `TranslateOptions` input-format
  option), and `AlignmentMap.input_format` plus the row-level `source_format`
  land in wave 6. `contracts.md` §0's rule for every row is the same rule.
- **The HTML segment-window splitter** for an oversize leaf block — the design
  commits to the *shape* only (a packing-time split in the DCR-0026 mold, never
  intake descent), and the splitter itself is unscheduled. D9 removed its most
  common trigger by making `<li>` the block.
- **Waves 3–7 are unblocked** (dependency order `0 → 2 → {3, 4} → 5 → 6 → 7`).
  Wave 4's layer-6 twin is a **hard precondition for any HTML translation run**:
  until it exists, an HTML document would ship with its last validation layer
  missing.
