---
type: DCR
title: The element walk models HTML integration points and foreign-content breakout, closing a live contracts.md §4a break
description: walk_elements carries a three-valued content mode per open element instead of one inherited in-foreign bool, so foreignObject/desc and the MathML islands return their children to HTML and the breakout tag set pops foreign elements — which stops an svg-wrapped div from swallowing the anchor of the block that follows.
tags: [change, project-control, DCR-0041]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# DCR-0041: The element walk models integration points and breakout

- **Date:** 2026-09-01
- **Source:** ticket `e77173`, filed from ti `490d97` wave 1's design review
- **Affected DCRs:** `DCR-0032-transync-html-crate-extraction.md` (amended — its "what it deliberately does not model" bullet is now closed rather than accepted)
- **Affected contracts:** `docs/architecture/contracts.md` §4a (the break this closes)

**Numbering note.** This record takes **DCR-0041**, not the next free number.
DCR-0035–0039 are reserved by the unrun HTML→HTML waves 3–7 and DCR-0040 is
taken, so the next genuinely free number is this one. Read a gap in the roster
as a reservation, not as a missing record.

## What was wrong

`walk_elements` carried a single `in_foreign` bool per stack entry, inherited
downward: an entry was foreign iff it was an `<svg>`/`<math>` root or its
parent was. That models foreign content as a region you enter once and leave
once, and HTML's is not one. It has **islands** — places where the parser is
back in HTML content while still inside an `<svg>` — and **exits** that no end
tag spells.

Two rules were therefore unmodelled, both measured in real Chromium while
designing the wave-1 fix:

1. **HTML integration points.** `<svg><foreignObject><div/>x` leaves that div
   **open** in a browser: `foreignObject` re-enters HTML content, where the
   self-closing flag is the parse error it always is. The walk treated it as
   ordinary foreign content and closed the div.
2. **Foreign-content breakout.** A start tag on HTML's breakout list pops open
   foreign elements until the parser is back in HTML. `<svg><div>x</svg>` puts
   that div at top level, not inside the svg.

## Why it was not a benign extent divergence

The ticket was filed reasoning that `element_extents` has no in-tree callers,
so only extents diverged and bytes stayed intact. **DCR-0032's own 2026-08-23
correction had already retired that framing**, and it is the reason this was
scheduled before wave 3 rather than with it: `balance_fragment` reads the
**same** walk, and its caller is the shipped pane path.

Measured through the shipped DOMPurify mount:

```html
<div class="wrap"><svg><div/>x</svg></div>
```

A browser pops the `<svg>` at the `<div>`, leaving that div open in HTML
content, where it swallows the anchor of the block that follows — which mounts
with parent `DIV.wrap` rather than `<main>`, breaking `contracts.md` §4a's
direct-child rule. The old walk closed the div at `</svg>`, saw a balanced
fragment, and passed it through with nothing to repair. Reachable from
**untrusted source Markdown** through a type-6 html block.

**The self-closing flag was never the mechanism.** The unflagged
`<svg><div>x</svg>` does exactly the same thing, because `div` is on the
breakout list. A fix that handled only the flagged spelling would have closed
half of it; the regression test asserts both.

## The change

The stack entry's third field stops being `bool` and becomes `ContentMode`
(`Html` / `Svg` / `MathMl`), meaning **the mode this element's children are
parsed in**. A bool cannot express an island; three values can.

- `foreignObject` and `desc` return their children to `Html`.
- MathML's text integration points (`mi`, `mo`, `mn`, `ms`, `mtext`) do too,
  as does `annotation-xml` at `encoding="text/html"` or
  `"application/xhtml+xml"` — and at no other value.
- A breakout start tag seen while the parent mode is not `Html` pops entries
  until the top parses its children as `Html`, closing each implicitly at the
  breakout tag's own start. An integration point already qualifies, which is
  why `<svg><foreignObject><div>` nests and `<svg><g><div>` does not.
- `font` breaks out only when it carries `color`, `face` or `size`.

`font` and `annotation-xml` need attribute values, and `TagToken::Open` carries
none. Rather than mint a second opinion about where an attribute ends, the
attribute loop already inside `collect_reserved_attr_spans` was extracted into
one `walk_attrs` that both it and the new predicates use. That function backs
`strip_reserved_sync_attrs`, the OI-0035 control, so its existing tests are
also the regression test for the extraction.

## What is still not modelled

- **SVG `<title>` as an integration point.** The spec lists it; `scan_tags`
  enters raw-text state for that name unconditionally, so its contents never
  reach this walk as markup at all. Listing it here would claim a fidelity the
  tokenizer one layer down does not provide. This is DCR-0032's separate
  "foreign raw-text/RCDATA" bullet and it remains open.

  *Closed 2026-09-01 by ticket `2e2453`, recorded in **DCR-0042**.* The
  tokenizer enters that state only in HTML content now, so the fidelity is
  there and `title` is listed. The content-mode stack this record introduced
  moved down into `scan_tags` at the same time — the tokenizer needs the same
  answer, and one model answering both layers is the point.
- **Void names used as real foreign elements.** Unchanged; `is_void` still
  wins globally.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 40/40 binaries,
  **1128 passed, 0 failed**, `CARGO_EXIT=0` (1123 before, plus five new tests).
- The token-stream golden pin is **byte-identical** old-versus-new, so
  `scan_tags` and `balance_fragment` are unmoved on the existing corpus.
- Five tests, each discriminating: integration points against an ordinary
  foreign element, breakout against a non-breakout tag, `font` with and
  without its attributes, `annotation-xml` at four encodings, and the §4a
  scenario in both the flagged and unflagged spelling.
