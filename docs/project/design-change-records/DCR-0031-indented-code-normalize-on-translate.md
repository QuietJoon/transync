---
type: DCR
title: An indented code block is normalized on translate — re-fenced for the wire, re-emitted fenced in the output
description: Every indented (four-space or tab) code block was mishandled, in one of two ways, because the unit payload was the block's dedented body — a four-space block cost three provider calls and was never translated, while a deeper indent was accepted on the first call and corrupted the output until the full-document reparse caught it. BlockKind::CodeBlock gains a fenced flag, the parser stops discarding it, the block's range moves back over the block-structure indent, and unit assembly sends the block re-fenced by the engine's own synthesizer — so an accepted indented block is re-emitted FENCED and only a fallback keeps the indented spelling. Post-implementation record for ticket 457e51.
tags: [change, project-control, DCR-0031]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-16T00:00:00Z
status: stable
---

# DCR-0031: An indented code block is normalized on translate

- **Date:** 2026-08-16
- **Source:** ticket `457e51` — *Every indented code block burns three provider
  attempts and is never translated*, filed 2026-08-13 out of the correction
  work on ti `d990b6` and measured there against the in-repo echo stub.
- **Post-implementation.** Unlike DCR-0026..DCR-0029 this record follows the
  code: the defect and its two failure mechanisms were established by tests
  before the fix, and the one thing here that needed *deciding* rather than
  fixing — what an accepted indented block should look like in `out.md` — is
  what this record exists to carry. The living documents (`contracts.md`,
  `scenario-matrix.md`, `CLAUDE.md` invariant 4, module docs) were edited by
  the same change and describe the shipped behavior.
- **Affected ADRs:** none amended. No principle moves. ADR-0004 (Comrak as GFM
  parser) is load-bearing — the dedent this record relies on is comrak's, not
  ours — and ADR-0009's bounded retry policy is untouched: the fix removes
  three attempts that could never succeed, it does not change what an attempt
  is. `DCR-0004`'s full-reparse cascade is what contained the second failure
  mechanism below, and it is unchanged.
- **Affected contracts:** `docs/architecture/contracts.md` §0 (a prose
  *Recorded surface decision* for the field-level break; the **table** is
  untouched, so `crates/transync/tests/public_surface.rs` does not move), §1
  (a stability entry for `BlockKind`), and §6 (the `--allow-html-input` usage
  passage and the exit-code-`2` bullet, both of which cited this defect as
  live justification). Plus `scenario-matrix.md` SCN-04.
- **Breaking**, and it rides the open **v0.4.0** window: `BlockKind::CodeBlock`
  gains a field, and `out.md` changes shape for a document containing indented
  code. No `schema_version` moves.

## The problem being solved

Comrak reports an indented code block's `Sourcepos` **after** the four columns
of block structure it consumed, and only on the block's first line — every
later line sits inside the range with its indent intact.
`outcome::block_payload` slices that range, so the unit's `source_payload` was
the block's *dedented* body. `parser::classify::code_block_kind` then discarded
comrak's `fenced` flag, so `BlockKind::CodeBlock { info }` carried no record of
which spelling the source used and nothing downstream could tell the two apart.

Two distinct failures followed, and the ticket's headline is only the first.

1. **Four spaces — rejected three times, never translated.** The dedented
   payload can only reparse as a paragraph, so `validate::fragment_reparse`
   answered `fragment reparse kind mismatch: expected code-block, got
   paragraph` on every attempt. The unit settled as `fallback_source` with
   `retried_units: 1`, `out.md` came back byte-identical to the input, and the
   run had spent three provider calls to achieve that. This was
   provider-independent: the echo stub and a real model fail identically,
   because what is wrong is the *shape of the payload*.

2. **Eight spaces — accepted, then corrupting, then cascaded.** Here the slice
   is `"    indented code line"`, which is *itself* a valid indented code
   block, so the echoed payload passed every per-unit layer and was accepted on
   attempt one. Regeneration re-emitted it fenced — but the first line's four
   consumed columns sat **outside** the block's range and were copied back
   verbatim as inter-block text, producing an indented code block holding a
   fence marker followed by an **unclosed** fence that swallowed the following
   paragraph. `validate::full_reparse` caught the divergence and the DCR-0004
   cascade downgraded both blocks. That is the ticket's "drags the following
   paragraph into fallback".

Neither failure was visible to the **default workspace gate**: SCN-04 covered
*fenced* code only, and no fixture reachable from `cargo test --workspace`
carried an indented block. The qualification is load-bearing, because one test
did carry one — `cli_html_input_leaves_an_indented_run_untranslated_and_unfenced`,
filed under ti `d990b6` — and it welded mechanism 1's behavior *as a record of
the defect*, not as a demand that it stay. It is `#[cfg(feature =
"test-stub-provider")]`, so it runs only under the CLI's stub feature and never
under the default gate. It is the test the Migration section below records
being renamed and inverted.

## What changes

**`BlockKind::CodeBlock` gains `fenced: bool`.** The variant becomes
`CodeBlock { info: Option<String>, fenced: bool }`, and
`parser::classify::code_block_kind` fills it from `code.fenced` instead of
throwing it away. The flag is the fix rather than an ornament on it: every
decision below has to know which spelling the source used, and before this
there was no way to ask.

**An indented block's byte range moves back over the block-structure indent.**
`parser::emit` wraps `ranges::byte_range_for` in a private
`snap_indented_code_start`, which for `CodeBlock { fenced: false, .. }` walks
`start` backwards over the run of spaces and tabs immediately preceding it,
bounded by `pos_to_byte(start_line, 1)`, and leaves `end` alone; every other
kind passes through unchanged. It is never "start minus four bytes" — comrak
counts consumed *columns*, which is four bytes for four spaces, one byte for a
tab, and four of eight for a doubly-indented block whose other four columns are
content, so only a walk over the actual indent is right for all three.
`source_hash` is computed after the snap, so it covers the block's full
spelling.

It is also not an unconditional snap to column 1, and the case that forces the
distinction is a document-leading UTF-8 BOM. Comrak's column is a byte offset
within the line, and it skips the BOM for block structure while still counting
its three bytes, so `"\u{feff}    code\n"` reports its block at `1:8`. Snapping
to column 1 there pulls the BOM *inside* the block's range — and because an
accepted translation replaces exactly that range with a regenerated fence, it
would **delete** the BOM from `out.md`, while a BOM ahead of a paragraph, a
heading or a fenced block survives in the inter-block gap. The walk stops at
the first byte that is not indentation, so the BOM stays where every other kind
leaves it. Pinned at both ends of the wire:
`parser::indented_code_tests::the_snap_stops_at_a_document_leading_bom` and
`regen::indented_code_regen_tests::a_leading_bom_survives_an_accepted_indented_block`.

**Unit assembly sends the block re-fenced.** `unit::payload::code_payload`
takes the (now complete) slice and, for an indented block only, runs it through
`transync_syntax::regen::regenerate_code_block(&raw, None)` — THE existing
fence synthesizer — trimming the trailing newline so the payload's envelope
matches a fenced unit's. The LLM therefore still only ever sees a fenced block.

**An accepted indented block is re-emitted fenced.** `regen` needed no logic
change: the translated arm already emits a fenced block with a safe fence
length, and with the snapped range the splice lands at column 0 with no
residual indent.

Nothing in `validate/`, `walk`, `transync-wasm`, the provider crates, the
prompt builder, batching, the pipeline or the cache code changed.

## The decision — normalize on translate

The fix could have preserved the indented spelling on the way out: regenerate
an accepted indented block by re-indenting its translated content four columns,
so `out.md` matched the source's spelling. That was rejected.

- **The one fence synthesizer already exists, and a second emitter would not
  be one.** `regen::regenerate_code_block` chooses a safe fence length against
  backtick and tilde runs in the body. An indent-preserving emitter needs no
  fence — but it needs its own correctness argument about tabs, about content
  that is *itself* indented, and about a body line that would re-open a nested
  construct. That is a second implementation of "emit a code block", and the
  bug being fixed here is exactly what happens when two parts of the system
  disagree about what a code block's bytes are.
- **The indented spelling has no semantic content.** CommonMark gives an
  indented block no info string and no way to carry one; it is the *same* block
  as a bare fenced one, and `wire_str()` has always called both `code-block`.
  Preserving the spelling would preserve a presentation choice, at the price of
  a second code path on the one seam that has now failed twice.
- **The output is already not byte-preserving.** Every translated block in
  `out.md` is regenerated, not copied. A reader who needs the source's bytes
  reads the source; `out.md`'s promise is the same *structure*, and a fenced
  block is the same structure.
- **Fallback still keeps the source bytes.** The indented spelling survives
  exactly where invariant 6 requires it to: a block that falls back is spliced
  back byte-verbatim, indent and all. So the one case where the output must be
  the input is the one case that is unaffected.

The accepted cost is stated plainly rather than hidden: **for a document
containing indented code, `out.md`'s shape differs from the source's on
purpose.** That is now written into `CLAUDE.md` invariant 4, `contracts.md`,
and the SCN-04 row, so it is a documented normalization rather than a surprise.

## Blast radius

**What moves.**

- The public API: `BlockKind::CodeBlock` gains a field. A consumer's
  `BlockKind::CodeBlock { info }` pattern becomes `{ info, .. }`, and a
  construction site must supply `fenced`.
- `out.md` shape: a **translated or preserved** indented block is re-emitted
  fenced — bare fence, no info string, safe length. A **fallback** block keeps
  its indented source bytes verbatim.
- Alignment-map **values** for indented-code rows: `source_range` widens by the
  indent, and `fallback_status` flips from `fallback_source` to `translated`.
  The schema is unchanged.
- `source_hash`, and therefore `CacheKey`, for indented blocks only.

**What does not, and was asserted rather than assumed.**

`ALIGNMENT_SCHEMA_VERSION` (`1.2.0`), `VALIDATION_SCHEMA_VERSION` (`2`),
`VALIDATION_REPORT_SCHEMA_VERSION` (`1.1.0`), `CacheKey`'s field set, the
`input_mode` axis label, `context_hash`, `instruction_hash`, the wasm JSON
boundary and its gate, the prompt wording, both provider crates, and the CLI
surface. `crates/transync/tests/public_surface.rs` is untouched: it welds
*paths*, and `transync::BlockKind` is the same path it was.

`VALIDATION_SCHEMA_VERSION` in particular was given a second look and left
alone. Its bump trigger is "an `InputMode`'s payload meaning changes", and
`FullCodeBlock`'s documented contract already read *"`source_payload` carries
the entire fenced Markdown including the open/close fences"*. Indented blocks
**violated** that contract before this change and conform to it after; the
axis's meaning does not move, the implementation stops breaking it. A bump
would also have been strictly worse — it orphans every cached entry of every
kind, while the only entries whose correctness is in question already have
moving keys.

`context_hash` is stable for the same kind of reason: `unit::context::neighbor`
builds each neighbor summary through `text_from_range`, which ends in
`.trim()`, so a slice that widened only by leading whitespace yields a
byte-identical summary — and `pipeline::context_hash` pushes
`n.kind.wire_str()`, a `&'static str`, so adding `fenced` to the enum cannot
reach it either.

**No serde default on `fenced`, on purpose.** No shipped artifact serializes
the enum form: alignment rows and `CacheKey` both carry `wire_str()` strings,
`llm::prompt`'s wire structs likewise, the disk cache stores `UnitResult`
(which has no kind field at all), and neither `TranslationUnit` nor
`parser::Block` derives `Serialize`. There is no compatibility to buy, so a
loud deserialize failure beats a silent wrong guess if one ever appears.

## Cache invalidation — none, and the reason is not the obvious one

The plain argument ("only validated output is written to a cache, and a
pre-fix indented unit could never validate") is **wrong**: `pipeline/dispatch`
calls `cache_put` at the accept point, *before* the post-regen full reparse, so
the eight-space class above did get cached. Two properties keep the tree clean
instead — both checked in source, and only the second one unconditional:

1. A run that reaches the end of `run_pipeline` evicts `keys_to_evict(id)` for
   every id in `validation_report.full_reparse_fallbacks` — precisely the set
   the DCR-0004 cascade downgrades, which is precisely the eight-space class.
   *A run that does not reach the end evicts nothing.* `cache_put` runs per
   accepted unit at the Accept disposition in `pipeline/dispatch`, round by
   round, inside the batch fan-out, while all three eviction points
   (`merge_outcome.faulted_parents`, the FailStop `divergent_source_blocks`
   path, and `full_reparse_fallbacks`) sit in `run_pipeline` after the fan-out
   settles — behind the DCR-0024 cancellation checkpoint and behind the
   lowest-index terminal-error return. Cancel a run, let a sibling batch fail
   terminally, or kill the process, and no *targeted* eviction can ever name
   that entry again: semantic eviction derives its keys from post-fix
   identities, so the stranded entry is in no set they compute. Only
   `DiskCache`'s open-time capacity trim can drop it, incidentally, once the
   log exceeds its byte budget (1 GiB by default, oldest-written first).
   That is why property 2 is the one carrying the argument.
2. Post-fix, every indented block's range widens by at least one byte, so
   `source_hash` — a `CacheKey` axis — moves, and a surviving entry is
   unreachable **by the block that wrote it**. It is not unreachable outright,
   and saying so would be imprecise: a pre-fix eight-space entry is keyed on
   `H("    body")`, and post-fix a **four-space** block whose complete spelling
   is exactly those bytes hashes identically — `context_hash` does not separate
   them either, because `unit::context::neighbor` trims. That hit is harmless,
   and it was traced rather than assumed: the cached payload is re-validated
   against the requesting unit, `check_code` compares info strings (`None` on
   both sides), `fragment_reparse` accepts the indented payload as a code
   block, and `regen::regenerate_code_block` dedents it through comrak and
   re-fences, yielding exactly the content the four-space block carries. The
   accurate claim is therefore: **unreachable by the block that wrote it, and
   reachable only by a block for which it is a correct answer.**

## Affected areas

- `crates/transync-syntax/src/id.rs`, `parser/classify.rs`, `parser/emit.rs`,
  `regen.rs`
- `crates/transync-core/src/unit/payload.rs`, `llm.rs`, plus **two** passages in
  `unit.rs` — the html-dominance note (a runtime string) and
  `html_dominance_warning`'s rustdoc above it, both of which described the
  defect
- `crates/transync-cli/src/translate_cmd.rs` and `translate_cmd/input.rs` — the
  `--allow-html-input` refusal message and its rustdoc, same reason
- `crates/transync/tests/fixtures/scn-04-indented-code.md` and
  `tests/scenarios/scn_04_code_block.rs`; unit tests in `parser.rs`, `regen.rs`,
  `unit/payload.rs`, `validate.rs`
- `docs/architecture/contracts.md` §0, §1, §6; `docs/architecture/scenario-matrix.md`
  SCN-04; `docs/Developer_Guide.md`; `CLAUDE.md` invariant 4; `CHANGELOG.md`

## Migration / follow-up

- **Consumers matching the variant** add a rest pattern:
  `BlockKind::CodeBlock { info, .. }`. Consumers constructing it supply
  `fenced`. Nothing else about `BlockKind` moves, and the `wire_str()`
  vocabulary is unchanged.
- **Consumers diffing `out.md` across versions** will see indented code blocks
  become fenced. This is the normalization decided above, not a regression.
- **One test was renamed and inverted** rather than deleted:
  `cli_html_input_leaves_an_indented_run_untranslated_and_unfenced` becomes
  `cli_html_input_re_emits_an_indented_run_as_a_fenced_block`. Its own doc
  comment had predicted this — *"a future change that really did fence an
  indented run would fail this test rather than quietly make the old claim true
  again"* — and it fired as designed.
- **A justification the code no longer exhibits was corrected in every copy a
  whole-tree grep found**, which is the ti `d990b6` lesson applied to itself.
  The HTML-document refusal (ti `13e145`) rested partly on "a
  four-space-indented run is not translated at all after three provider calls".
  That leg is now false. The refusal **stands**, and its rationale is
  *stronger*: the indented run used to be the one loud, self-reporting harm on
  that path, and it is now a silent reshaping like the other two, so nothing a
  run emits distinguishes that input from a correct one. **Eight** passages
  across five files, two of them **runtime strings** an operator reads:

  | where | what it is |
  |---|---|
  | `crates/transync-cli/src/translate_cmd.rs` — the `--allow-html-input` refusal message | runtime string |
  | `crates/transync-core/src/unit.rs` — the html-dominance note | runtime string |
  | `docs/architecture/contracts.md` §6 — the `--allow-html-input` usage passage | prose |
  | `docs/architecture/contracts.md` §6 — the exit-code-`2` bullet | prose |
  | `docs/Developer_Guide.md` — the `translate` flag block | prose |
  | `crates/transync-cli/src/translate_cmd.rs` — `allow_html_input`'s rustdoc | rustdoc |
  | `crates/transync-cli/src/translate_cmd/input.rs` — the preamble-sniff rustdoc | rustdoc |
  | `crates/transync-core/src/unit.rs` — `html_dominance_warning`'s rustdoc | rustdoc |

  The last row is the one an earlier count of this table missed: that function's
  rustdoc states the claim in its own words, directly above the runtime string
  it builds, so correcting the string alone would have left one file arguing
  with itself. Two passages in `unit.rs`, two in `translate_cmd.rs`, two in
  `contracts.md`, one each in `input.rs` and `Developer_Guide.md` — eight, five
  files.

  No test welds any of that wording; `cli_html_document_input_is_refused`
  asserts only that `--allow-html-input` and `490d97` appear, and both do.
