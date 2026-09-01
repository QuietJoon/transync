---
type: DCR
title: Voidness is an HTML-content rule, and the breakout model already retired the end-tag-br hazard that kept it global
description: Foreign content has no void elements, so a void name used as a real foreign element now opens and its author's end tag is a real closer instead of an orphan the balancer deletes — safe because the five void names HTML also treats as breakout tags leave foreign content before they are processed.
tags: [change, project-control, DCR-0043]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# DCR-0043: Voidness is an HTML-content rule

- **Date:** 2026-09-01
- **Source:** ticket `48f3c6`, filed from ti `490d97` wave 1's design review
- **Affected DCRs:** `DCR-0032` (its "void names used as real foreign elements" divergence bullet is closed), `DCR-0041` and `DCR-0042` (their matching "still not modelled" residuals are closed)

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This is the next free number after DCR-0042.

## What was wrong

`is_void` decided whether a start tag opens an element, globally — foreign
content included. HTML's void list is an HTML-content rule. In foreign content
the "in foreign content" insertion mode's *any other start tag* inserts a
foreign element and pops it only if the self-closing flag is set; there is no
void list at all. So `<svg><link>a</link>` is a genuine, closable SVG element,
and the author's `</link>` was classified an orphan and **deleted** from the
pane — the balancer editing what the reader sees, in exactly the direction it
exists to prevent. Thirteen HTML void names are reachable this way (`area`,
`base`, `basefont`, `bgsound`, `col`, `frame`, `input`, `keygen`, `link`,
`param`, `source`, `track`, `wbr`).

## Why it stayed open, and why that reason expired

The trade was recorded deliberately, not overlooked. Pushing void names inside
foreign content risks the balancer appending `</br>`, and HTML's
end-tag-`br` rule turns that back into a **fresh `<br>`** — the balancer
minting structure rather than repairing it, which is worse than the bug.

That hazard is unreachable now, and it was DCR-0041 that made it so. `br` is in
`FOREIGN_BREAKOUT_TAGS`, so `<svg><br>` tears out of foreign content **before**
the tag is processed and lands in HTML content, where `is_void` still wins and
nothing is ever pushed. The same holds for the other four void names that are
also breakout tags: `embed`, `hr`, `img`, `meta`. Before DCR-0041 modelled
breakout there was no such guarantee and the trade was correct; after it, the
premise was simply stale. Nobody noticed because the ticket predates the model
that answered it.

The intersection is pinned rather than described:
`a_void_name_that_is_also_a_breakout_tag_never_opens_inside_foreign_content`
asserts it is exactly those five and balances each one, so removing a name from
`FOREIGN_BREAKOUT_TAGS` fails here with the `</br>` hazard named in the message.

## What changed

One expression in `scan_tags`, where DCR-0042 had just put the content-mode
stack: `is_void` is consulted only when the tag's parent mode is HTML content.
`walk_elements` reads the resulting `opens_element` verdict, so there is still
one place that decides.

`is_void`'s own contract is unchanged and stays a name-only predicate, like
`is_raw_text`: it answers "is this an HTML void name", the caller owns the
context, and `scan_tags` is the one that pairs the two.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 40/40 binaries,
  **1145 passed, 0 failed**, `CARGO_EXIT=0` (1142 before, plus three tests).
- Two-bless protocol again. The movement between the blessings is two lines:
  `foreignvoid-closer` stops losing the author's `</link>`, and
  `foreignvoid-unclosed` gains the closer it was owed. `foreignvoid-br-guard`
  did not move — the hazard guard held — and the **token stream did not move at
  all**, since this is a walk change and `scan_tags`' token projection cannot
  see it.
