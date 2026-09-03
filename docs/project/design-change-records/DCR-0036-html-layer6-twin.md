---
type: DCR
title: The HTML layer-6 twin lands, and the document-level gate dispatches on source format
description: Wave 4 of the HTML→HTML feature adds transync-core::validate::full_rescan_html — ordered tag ledger, fresh segmentation through the intake, gap byte-identity including preamble and tail, and boundary sanity — plus the one format branch in pipeline::finalize. ReparseFailure is unchanged, so the DCR-0004 three-stage cascade is untouched by construction and by diff. No HTML translation run becomes reachable; this is the gate that must precede one.
tags: [change, project-control, DCR-0036]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-03T00:00:00Z
status: stable
---

# DCR-0036: The HTML layer-6 twin lands, and the document-level gate dispatches on source format

- **Date:** 2026-09-03 — this record, and the code, which landed the same day:
  `6503255` (the twin, the four checks, `layer6_gate`, and the dispatch tests).
  Read from `git log --date=short`, **not** from the plan's filename: the plan
  is dated 2026-08-20, which is its writing date — the rule DCR-0033 wrote down.
- **Source:** `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
  (ratified design spec), ticket `490d97`, **wave 4** of the eight in its §12.
  §7 is this wave in full. Plan:
  `docs/superpowers/plans/2026-08-20-html-wave4-layer6-twin.md`.
- **Affected ADRs:** `docs/decisions/0025-html-to-html-document-translation.md`
  (D9's per-`<li>` ruling as landed in a validator).
- **Affected DCRs:** `DCR-0004` (the three-stage fallback cascade, which this
  record leaves **untouched** — and proves untouched by diff, not by assertion);
  `DCR-0032` (`tag_inventory` / `scan_tags`, consumed unmodified);
  `DCR-0034` (`Document.format`, the axis the one branch dispatches on);
  `DCR-0035` (wave 3's intake and identity theorem, which check 2 calls and on
  which the cascade's terminal rung rests).
- **Numbering note.** This record takes **DCR-0036** because wave 4 reserved it,
  not because it was next free on disk. `DCR-0036`–`DCR-0039` remain one per
  unrun HTML→HTML wave (4–7); waves run partly in parallel, so "the next free
  number" is a race.

## What changed

`crates/transync-core/src/validate/full_rescan_html.rs` — THE HTML layer-6
twin, sibling of `full_reparse.rs`:

```rust
pub fn full_rescan_html(
    source_doc: &Document,
    regenerated: &str,
    offsets: &BlockOffsets,
) -> Result<(), ReparseFailure>
```

Four checks, in spec §7's order:

1. **Document-wide ordered tag ledger** — `tag_inventory(regenerated)` must
   equal `tag_inventory(source_text)`. Ordered `Vec`, so dropped, duplicated or
   reordered markup fires anywhere, inside blocks and in gap bytes alike.
2. **Fresh segmentation** — the regenerated document goes back through
   `transync_syntax::intake::html::parse`, and the block count and
   `wire_str()` label sequence must match `source_doc.blocks` in order. Written
   to D9's ruling: `<li>` is the block, so both sides carry per-item entries and
   attribution is per-`<li>`, never per-list. Attribution mirrors
   `full_reparse::attribute_offenders` — each fresh block's start offset mapped
   into the regen `BlockOffsets`; a source block owning ≠ 1 fresh block is the
   offender.
3. **Gap byte-identity** — every inter-block gap, the preamble and the tail,
   compared byte for byte. The check with no Markdown twin.
4. **Boundary sanity** — every block has a target range; ranges are in-bounds
   against the **actual output length**, on char boundaries, monotone and
   non-overlapping.

And `layer6_gate` in `crates/transync-core/src/pipeline/finalize.rs` — **the
one format branch**, exhaustive over `SourceFormat` with both arms named:
`reparse_full` for Markdown, `full_rescan_html` for HTML.

## Why

Wave 4 is **the gate**. Spec §7/§12's hard rule is that no HTML translation run
ships before the twin exists, and the reason is sharper than "untested": comrak
over an HTML document produces *some* node sequence and can pass while checking
nothing. An HTML run gated by `reparse_full` is **worse than ungated** — it
carries the authority of a gate and none of the coverage.

The wave's recorded red demonstrates it rather than arguing it. Before the
branch, an HTML document with one honestly translated `<p>` and one rule-T run
whose translation dissolved it routed into comrak's reparse, and the observed
failure reason was `source kind paragraph != regenerated kind html` — comrak's
vocabulary about a document it cannot read. Under `FallbackPerBlock` that
cascade over-downgraded past the honest translation on its way to
`FallbackAll`. After the branch, stage 1 downgrades exactly the dissolved run
and the honest translation survives.

## Check 3 is a gate, not decoration — and the red proves it

A dropped `<!DOCTYPE>` or a dropped comment leaves **no ledger entry** (each
scans as a `TagToken::Skip`, which `tag_inventory` filters) and mints **no
block** (wave 3's doctype-only pin). So checks 1 and 2 both accept it, and with
coherent offsets so does check 4. Only check 3 stands between that corruption
and a silently doctype-less output.

That is not an argument in this record; it is a recorded observation. Both
blind-spot tests were run against the two-check twin and **failed** there, then
passed once check 3 landed. The gate directory holds the round: seven named
reds, zero panics.

Check 3 defers a gap whose endpoints cannot be sliced to check 4 — the fault is
the **range**, not the bytes — and check 4's last step hard-errors on any
leftover deferral, so no gap can fall between the two checks. The deferral is
total by construction (every cause of an unreadable gap is one of check 4's
predicates), and the leftover arm is the belt-and-braces that makes the
totality claim checkable in code rather than asserted in prose.

## The cascade is untouched — by diff, not by assertion

Both gates return the same `ReparseFailure { reason, divergent_source_blocks }`.
That shared shape is the whole reason the DCR-0004 three-stage cascade does not
move: `downgrade_units` consumes only the id list, `widen_to_neighbors` consumes
the id list plus `regen::top_level_blocks` (whose body is `blocks.iter()` —
format-blind), `fall_back_all` consumes neither field, and `reason` reaches only
`tracing::warn!` and the Hard-arm `Err`.

Verified mechanically against the wave's baseline commit:

- `validate/full_reparse.rs`, `pipeline.rs` and `pipeline/report.rs` are
  **byte-untouched** — an empty diff. `full_reparse.rs` untouched is the
  strongest available form of "`ReparseFailure`'s shape is unchanged".
- `pipeline/finalize.rs` has exactly five hunks: three one-line call-site
  reroutes, `layer6_gate`, and the new test module. **No hunk touches**
  `downgrade_units`, `widen_to_neighbors`, `fall_back_all`, `regen_pass` or
  `collect_validated`.
- Nothing outside `crates/transync-core` and the named docs files changed —
  no `transync-syntax`, no `transync-html`, no CLI, no provider crate, no
  `web/`, no `Cargo.toml`, no `Cargo.lock`.

The cascade's terminal rung stays sound for HTML **because of wave 3's identity
theorem**: `regenerate(doc, &empty)` is byte-identical to the source, so a
full-fallback regen passes the twin trivially. `fall_back_all`'s "structurally
identical by construction" claim extends to HTML with no new code, and the
finalize-seam test pins it.

## No HTML translation run became reachable

`translate()` still has exactly one document constructor —
`let mut doc = parse(source)?`, the Markdown intake, which stamps
`format: SourceFormat::Markdown`. There is no `input_format` anywhere in
`transync-core`'s `lib.rs` (grep count 0), no CLI flag, and `intake::html`
appears in exactly two files: the twin's check 2 and `finalize.rs`'s dispatch
test module. The Html arm is **test-only** until wave 5's entry point and wave
6's flag. `html_dominance_warning`'s "HTML-to-HTML translation is not
implemented (ti 490d97)" tail is therefore still TRUE after this wave.

## Two spec amendments owed

Recorded as owed rather than silently diverged from, following wave 3's
deviation-5 pattern. **The spec file was deliberately not edited in this wave.**

1. **§12's parallelism claim.** §12 says wave 4 "runs parallel with 3" and is
   "developable against hand-built `Document`s before intake is complete."
   Check 2 *consumes* the intake, so that holds for checks 1, 3 and 4 and for
   the finalize branch, and is false for check 2. In the executed ordering wave
   4 followed wave 3 and calls `intake::html::parse` directly. The alternative
   — a segmenter seam so the twin is testable against a stub — was rejected
   twice over: it is the "untested code claiming to be a gate" shape wave 3's
   deviation 2 rejected for the intake's own `Result`, and a stub segmenter
   would be a **second HTML opinion**, which is the sin the architecture
   forbids. Check 2's entire value is that the *same* segmenter reads both
   sides. **Amend §12's wave-4 entry to say so.**
2. **§7's blind-spot wording.** §7's check-3 rationale still says `<!DOCTYPE>`
   and comments "produce no token in `scan_tags`". Since wave 0 landed the
   bogus-comment state they **do** produce a token — a `Skip` — and what they
   leave none of is a **ledger entry**, because `tag_inventory` filters `Skip`.
   The conclusion §7 draws is unchanged and still load-bearing (ledger-
   invisible, gap-visible, hence check 3); only the stated mechanism is stale.
   **Amend the sentence to the ledger-entry form.**

## The three residuals, recorded (spec §7; accepted limitation §13 item 7)

- **Two same-kind, identical-tag-skeleton blocks whose texts were swapped by an
  engine fault pass all four checks.** Exact parity with the shipped
  `reparse_full`'s blindness to two swapped paragraphs — parity, not
  regression. Per-unit layer 3 already ledger-checks each accepted splice
  individually, which bounds the fault surface to regen's assembly ordering,
  and checks 3 + 4 jointly cover any block whose neighbors differ. Pinned by
  `swapped_same_skeleton_texts_pass_by_documented_parity`, so the residual is a
  tested fact rather than a forgotten one: if a future check closes it, that
  pin flips and the closure record retires it deliberately.
- **Attribute values are not tokenized.** `scan_tags` skips attribute
  internals, and RCDATA/raw-text contents are never tokenized. Compensating
  controls: splice never edits inside tags by construction, translated-text
  escaping is pinned by test in `transync-html`, and SCN-16's acceptance
  criterion ("untouched markup byte-identical outside text nodes") tests it end
  to end.
- **`scan_tags` is not a general HTML parser** — which is fine, because both
  sides of every comparison use the *same* scanner. The check is
  self-consistency of one tokenizer opinion, the same rule the architecture
  enforces for comrak on the Markdown side.

## One deviation from the plan, and why

**The plan's three commits became one.** It splits this wave into three (twin
spine / checks 3+4 / dispatcher). That boundary is not reachable: `validate` is
`pub(crate)`, so `pub fn full_rescan_html` is crate-private, and until
`finalize` calls it `-D dead_code` fails the pre-commit hook on the twin and all
six of its helpers. Since `--no-verify` is never used, the twin and its caller
must land together.

Every staged red the plan specifies was still produced and recorded — the reds
are the evidence, and a commit boundary is not what makes them evidence:

| round | observed |
|---|---|
| wiring | `E0425: cannot find function full_rescan_html` at each call site |
| accept-everything stub | 3 passed / 5 failed — the three pins that *should* accept passing on a stub that checks nothing |
| checks 3+4 | 7 named reds, **zero panics** |
| dispatch | 2 reds, on assertions rather than compiles, with comrak's reason observed live |

The plan's own "record whichever state is observed" instruction covered two
tests that passed in their red rounds, and both are honest paths it anticipated:
`fallback_all_returns_the_source_bytes_for_an_html_document` (FallbackAll never
consults the gate) and `text_moved_between_a_block_and_a_gap_is_still_caught`
(its corruption dissolves a run, so check 2 rejects before check 3 reads a gap).

**Plan deviation 4 does not apply in this execution.** The plan assigns
`docs/index.md` to a separate controller and forbids the implementer from
touching it. There is no separate controller here — one agent executed the wave
— so leaving `docs_index_drift` red would be leaving the tree broken, not
handing work over. DCR-0036's link is added with the plan's own exact line, in
the records commit, exactly as waves 2 and 3 did.

## Evidence

- The staged red/green gate files under
  `/Volumes/Temp/claude/ti490d97-wave4/gate/`: `t2-red-compile.txt`,
  `t2-red-stub.txt`, `t3-red.txt` (the decoration-proof), `t3-green.txt`,
  `t4-red.txt` (comrak-over-HTML observed live), `t4-green.txt`,
  `t5-untouched.txt`, `t5-finalize-hunks.txt`, `t5-shape.txt`, `t5-scope.txt`.
- 16 module tests in the twin; 4 dispatch tests in `finalize.rs`.
- `cargo test -p transync-core -- --test-threads=4`: 480 passed, 0 failed,
  with **zero edits** to any pre-existing test — half of the "Markdown runs
  still take `reparse_full`" proof. The other half is positive:
  `a_markdown_document_still_takes_reparse_full_not_the_twin` asserts
  `reparse_full`'s reason vocabulary present and the twin's absent.
- `cargo test --workspace -- --test-threads=4`: 46 binaries, 1234 passed, 0
  failed, `CARGO_EXIT=0`. Two-package `wasm32` check: `CARGO_EXIT=0`, string
  unchanged. `fmt` and `clippy --all-targets --all-features -D warnings`: clean.

## Hand-forwards

- **Wave 5** makes `translate()` reach `format == Html`: the entry point, units,
  context, prompt, cache separation, and the `html_dominance_warning` re-text.
  It inherits **one wording residual recorded here**: the pipeline's Hard-arm
  mapping `"full reparse failed: {reason}"` — pinned out of crate by the
  facade's `boundary_v02::hard_failure_maps_error_and_evicts_implicated_keys` —
  will prefix the twin's reasons too, at which point "reparse" is loose for
  HTML. Re-word or re-pin **in the wave the entry point lands**, never
  silently.
- **Wave 6** adds the CLI flag, the wire's `source_format` / `input_format`
  fields, and `out.html`.
- **Wave 7** adds the browser gate and the SCN-16 scenario-matrix row.

The standing hard rule is now **discharged**: the twin exists, so wave 5 may
proceed.
