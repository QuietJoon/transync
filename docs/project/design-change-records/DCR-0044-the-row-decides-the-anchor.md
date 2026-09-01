---
type: DCR
title: The alignment row decides whether a block gets a DOM anchor, and sync_role_for names every BlockKind variant
description: The omission of the sync-attribute set moves out of render_block's literal ThematicBreak test and into write_attrs, keyed on the row's own sync_role, while sync_role_for drops its catch-all so a new BlockKind cannot inherit a wire-visible role nobody chose.
tags: [change, project-control, DCR-0044]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# DCR-0044: The row decides the anchor

- **Date:** 2026-09-01
- **Source:** tickets `18b9c3` (wave 2 Task 6's implementer) and `9ffb97` (ti `490d97` wave 1 review)
- **Affected contracts:** `docs/architecture/contracts.md` §4a (amended — the non-sync attribute rule is stated for every kind rather than for thematic breaks alone)

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This is the next free number after DCR-0043.

## What was wrong

Two places decided whether a block anchors, and they did not consult each
other:

- `align::sync_role_for` answered `Title => SyncRole::NonSync` (decision D5).
- `render::render_block` enforced "no anchor" with a literal
  `matches!(kind, BlockKind::ThematicBreak)` test. It never called
  `sync_role_for` at all.

So a `Title` block reaching the renderer would get a real `data-sync-id` while
its own alignment row said `sync_role: non-sync` — the row and the DOM
disagreeing about one block, which is what every anchor rule in §4a rests on
not happening. Unreachable today (nothing mints a `Title`; the Markdown intake
cannot and wave 3's HTML intake is unrun), and reachable the moment wave 3
lands.

`sync_role_for` had the matching hazard from the other side: a `_ =>
SyncRole::Anchor` catch-all. `SyncRole` is wire-visible — it decides whether a
row gets a DOM anchor and whether the engine counts it synchronizable — so a
new `BlockKind` variant compiled clean and silently started anchoring. Wave 2's
own plan documented this prospectively in the comment it dictated for the
`Title` arm: the default "would have made this row anchoring — the precise
opposite of the decision — without one word of warning from the compiler".
`Title` was caught because a human wrote the arm and a test pinned it; the
variant after it would have had neither.

## Decision

**The row decides, not a list of kinds — and not `sync_role_for` either.**

`18b9c3` proposed that `render_block` call `sync_role_for`, or else that the
literal list be defended and `Title` added to it. Neither was taken. The
omission moved into `render::attrs::write_attrs`, keyed on **`row.sync_role`**:
a `non-sync` row gets `data-block-kind` and nothing else.

The difference matters. Two callers of one function is still two places that
can be given different arguments; one *value*, written by `sync_role_for` into
the row and read back at the point of emission, cannot disagree with itself.
It is also what invariant 1 already says the consumer does — "the engine takes
its anchor set from validated alignment rows, never from whatever the DOM
happens to carry" — so the emitter now reads the same authority the engine
does.

`render_block`'s `ThematicBreak` arm survives, because an `<hr>` needs a
different *element*; it no longer decides anything about *attributes*. Its
output is byte-identical, which the pre-existing render tests established
before a new one was written for it.

**`sync_role_for` is exhaustive.** All seventeen `BlockKind` variants are
named and the `_` arm is gone, so the next variant is a compile error at the
one place that decides its wire role. The rule is narrow and stated at the
function: a *dispatch that assigns behaviour per variant* must not have a
default. Membership tests are a different shape and are untouched —
`unit::context::document_title` asks "is this block a title" with `matches!`,
where an exhaustive match returning `bool` would be noise rather than safety.

`SyncRole::ChildOnly` is still never returned, and that is contractual (§4
records it RESERVED for a future nested-anchor scheme) rather than an
omission; a test asserts no arm answers it.

## Consequences

- Good: the divergence is discharged **before** wave 3 mints the block that
  would have exposed it, rather than at wave 6 where the ticket allowed.
- Good: no behaviour moves for any reachable input. Thematic breaks render
  byte-identically and every other kind still anchors.
- Neutral: adding a `BlockKind` variant now costs one deliberate line in
  `sync_role_for`. That is the intended cost.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 40/40 binaries,
  **1150 passed, 0 failed**, `CARGO_EXIT=0` (1147 before, plus three tests).
- `a_title_block_renders_without_the_dom_anchor_its_row_denies` asserts **both
  halves in one test**, as `18b9c3` asked: a synthetic `Title` block through
  the real renderer with its real `build_alignment_map` row — the row says
  `non-sync`, the pane carries no `data-sync-id` for it, and the paragraph
  beside it still anchors so the assertion is not passing on an empty render.
  Synthesizing the block follows `skipped_render_tests`' existing precedent;
  it takes the Guard-2 degrade path, since no Comrak node carries the label
  `title`.
- `the_thematic_break_arm_still_emits_exactly_its_styling_hook` pins the
  byte-identical `<hr data-block-kind="thematic-break">`.
- `every_block_kind_answers_the_role_it_was_given` pins what the seventeen
  arms *say*, since the compiler only enforces that they exist.
