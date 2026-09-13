---
type: reference
title: The nineteen divergences of transync-html from the retired htmlseg
description: transync-html was lifted out of transync-syntax::htmlseg verbatim and has deliberately diverged from it nineteen times since. This is the enumeration — each divergence with its date, ticket and record — and the rule it exists to serve: a difference from the old htmlseg in any of the nineteen is the fix, not a regression to restore.
tags: [reference, architecture, transync-html, DCR-0032]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-13T00:00:00Z
status: active
---

# The nineteen divergences of `transync-html` from `htmlseg`

DCR-0032 lifted the HTML mechanics out of `transync-syntax::htmlseg` into the
`transync-html` crate. At the extraction the new crate was verbatim. It is
deliberately not verbatim any more.

**The rule this list exists to serve:** a difference between `transync-html` and
the old `htmlseg` in any of the nineteen below is the **fix**, not a regression
to restore. Anyone comparing the two crates — or reading an old test, an old
fixture, or `git show` of a pre-extraction file — needs to be able to tell a
deliberate correction from a bug, and that is what this table is for.

Why the table is here and not in DCR-0032: a DCR is a record, and records are
write-once (DCR-0054). DCR-0032's amendment trail numbers its last five
divergences "the fifteenth through nineteenth" but carries no running list that
reaches fourteen, so the tally could not be walked from the record that owned it.
The enumeration therefore lives in a living document, where it can be maintained;
the records stay as written.

Every divergence is measured against a browser, not against this crate's own
expectations — `web/tests/html-oracle.spec.js` drives headless Chromium over
generated fragments, with the corpus emitted by the double-gated
`emit_browser_oracle_corpus` in `crates/transync-html/tests/generative_properties.rs`
so `cargo test --workspace` never needs a browser.

## The enumeration

| # | Date | Ticket | Record | What changed |
|---|---|---|---|---|
| 1 | 2026-08-21 | `549b20` | — | `scan_tags`'s `AttrState::Outside` arm tokenizes a stray quote, a stray `=` and an unterminated tag the way a browser does. |
| 2 | 2026-08-23 | `490d97` wave 1 | — | `walk_elements` honours a start tag's self-closing `/` in only the two places HTML does — inside foreign content, and on the `<svg>`/`<math>` start tags that enter it. A flagged non-void tag now opens, and its author's end tag is a real closer rather than an orphan. |
| 3 | 2026-08-23 | `490d97` wave 1 | — | `strip_reserved_sync_attrs` coalesces adjacent cuts and replaces the run with one space wherever deleting it would weld bytes together. Before this, `<div data-sync-id="a b="c">x` stripped to `<divc">x` and the tag inventory moved from `["div"]` to `["divc"]`. |
| 4 | 2026-09-01 | `e77173` | DCR-0041 | `walk_elements`' single inherited `in_foreign` bool becomes a three-valued content mode per open element: SVG's `foreignObject`/`desc` and MathML's text integration points return their children to HTML content, and a breakout start tag pops foreign elements. Closes a live `contracts.md` §4a break reachable from untrusted source Markdown, where an `<svg>`-wrapped `<div>` stayed open and swallowed the following block's anchor. |
| 5 | 2026-09-01 | `2e2453` | DCR-0042 | Raw text and RCDATA become the HTML-CONTENT states they are, so inside `<svg>`/`<math>` the elements `script`, `style`, `textarea` and `title` are ordinary foreign elements whose contents are markup and whose self-closing `/` is honoured. Moves the content-mode stack down out of `walk_elements` into `scan_tags`, where the tokenizer needs the same answer, and closes a second strip bypass: a `data-sync-id` planted inside `<svg><title>` is live in a browser and used to survive the strip. |
| 6 | 2026-09-01 | `48f3c6` | DCR-0043 | Voidness becomes an HTML-content rule, so a void name used as a real foreign element — `<svg><link>a</link>` — opens, and its author's end tag is a real closer instead of an orphan the balancer deletes. Safe because the five void names that are also breakout tags leave foreign content before they are processed. |
| 7 | 2026-09-01 | `e20490` | — | `scan_tags`' TAG-NAME and END-TAG-OPEN states follow HTML, with one shared `tag_name_end` serving both the scanner and the strip. Closes a live strip bypass: `<divq"x=" data-sync-id="v">` is one element carrying a **real** `data-sync-id` that the old name boundary buried in a phantom quoted value. |
| 8 | 2026-09-01 | `95f55b` | — | `balance_fragment` withholds an appended closer that an unterminated trailing comment, CDATA, bogus comment, raw-text run or tag would swallow. A buried closer closes nothing, and each re-balance added another — 15,726 violations across 200,000 fuzz iterations before the fix. |
| 9 | 2026-09-01 | `415cdb` | — | A stray `=` begins an attribute name, the way HTML's before-attribute-name state reads it, so the strip stops cutting a non-reserved attribute across one. The direction was always safe — a 51-case browser equivalence probe found 50/51 equal, one over-deletion and **zero** under-deletions — so the impostor-anchor property was never at risk. |
| 10 | 2026-09-03 | `c1f9a8` | — | `balance_fragment` **repairs** a fragment that ends inside an unterminated region rather than passing it through: the region is terminated where a browser terminates it and deleted where a browser abandons it, with `TagToken::Skip` carrying the kind and terminated-ness so the balancer never classifies bytes itself. Closes a §4a direct-child break — three of the four region kinds end at the first `>`, which in a mounted pane is the sync wrapper's own `</div>`. |
| 11 | 2026-09-05 | `bebebe` | DCR-0050 | `RAW_TEXT_ELEMENTS` gains `xmp`, `iframe`, `noembed` and `noframes`, which HTML's in-body and in-head rules make generic raw text. `<div><iframe></div></iframe>` is one type-6 html block and stops losing its author's `</iframe>` to the orphan pass. |
| 12 | 2026-09-05 | `bebebe` | DCR-0050 | `comment_end` becomes the one home of "where does a comment end", and knows HTML's second closing form `--!>`. `<!-- a --!><div>y<!-- b -->` stops hiding a live `<div>` inside one comment. |
| 13 | 2026-09-05 | `bebebe` | DCR-0050 | `tag_attr_value` decodes character references through `htmlize::unescape_attribute` before `child_content_mode` compares them, so `<annotation-xml encoding="text&#47;html">` is the HTML integration point a browser makes it. |
| 14 | 2026-09-05 | `bebebe` | DCR-0050 | `<plaintext>` in HTML content becomes a `SkipKind::PlainText` region covering the start tag and every byte after it, and the region is **deleted**. This is the one arm the terminate-or-delete rule of #10 never had, because HTML's PLAINTEXT state has no exit at all; DCR-0050 records why keeping the tag, or keeping the bytes after it, is each a break rather than a repair. |
| 15 | 2026-09-05 | `9b4d66` et al. | DCR-0051 | In-body's "any other end tag" stops at the first **special** element rather than at the nearest name match. `</b>` in `<div><b><div></b></div>` closes nothing, and `</desc>` in `<svg><desc><div></desc><![CDATA[` leaves the fragment in HTML content, where a browser reads a bogus comment and the strip's reading is the browser's. |
| 16 | 2026-09-05 | `307283` et al. | DCR-0051 | HTML's **four scopes** decide the block-level, `li`, `h1`–`h6` and table end tags, so `<div><table>a</div>` leaves the table open the way a browser does, instead of popping it and letting the pane's own `</div>` be ignored the same way (R0010-0039). |
| 17 | 2026-09-05 | `895fb7` et al. | DCR-0051 | Foreign content's end-tag dispatch is followed **in the spec's step order**, with `</p>` and `</br>` as breakout end tags — the rule that makes `<p><ul><svg></p><title>` read `<title>` as HTML RCDATA on purpose rather than by accident. |
| 18 | 2026-09-05 | `da6bb5` et al. | DCR-0051 | The start tags that "in body" **ignores** — `<td>`, `<tr>`, `<tbody>` and friends outside a table — open no element and earn no invented closer. |
| 19 | 2026-09-05 | `a7e625` | DCR-0051 | `mglyph` and `malignmark` keep their children in MathML. Filed as spec-traced, and the browser oracle **confirmed** it. |

## What "orphan close tags are dropped" now means

The guarantee narrowed with divergences 15–19. A closer stopped **inside** the
fragment by a special element or a scope terminator is **kept**, because a
browser stops in the same place, and `</p>` and `</br>` are never orphans at all
since a browser turns each into content.

## What is deliberately still absent

- **`noscript` is not raw text.** HTML makes it raw text only under the
  scripting flag, which is context rather than a name, so the crate does not
  treat the name as raw text. Ticket `16721e90` carries the open question of
  what the panes should do about it.
- **The adoption agency algorithm is not implemented.** The residual is **8 of
  10,000** generated fragments, recorded in `contracts.md` §4b and owned by
  ticket `525bef`.

## What DCR-0051 measured

Against the oracle's unchanged atom alphabet, over 10,000 generated fragments:

| Measure | Before | After |
|---|---|---|
| Agreement with Chromium | 7,697 | 8,619 |
| `sec4a:break-nested` | 1,358 | 8 |
| `reserved:dom-only` | 1 | 0 |

Three of the four `KNOWN_DIVERGENT` routes retired from both halves of the
ledger, and no golden moved anywhere in the workspace.
