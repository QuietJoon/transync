---
type: DCR
title: balance_fragment repairs a fragment that ends inside an unterminated region, with the scanner deciding what the region is
description: A fragment ending in an unterminated comment, CDATA section, bogus comment or tag passed through unrepaired; three of those four kinds end at the first `>`, which in a mounted pane is the sync wrapper's own closer, so every following block nested inside. TagToken::Skip now carries the region's kind and terminated-ness.
tags: [change, project-control, DCR-0047]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-03T00:00:00Z
status: stable
---

# DCR-0047: The balancer repairs an unterminated trailing region

- **Date:** 2026-09-03
- **Source:** ticket `c1f9a8`, filed from ti `95f55b`'s own carve-out
- **Affected contracts:** `docs/architecture/contracts.md` §4 (the residual it recorded as open is closed)
- **Affected DCRs:** none amended; this is the fourth balancer/scanner change in the `DCR-0041`–`DCR-0043` sequence and shares their shape

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This is the next free number after DCR-0046.

## What was wrong

A fragment can end inside an unterminated comment, CDATA section, bogus comment
or tag — an author closes an HTML block mid-`<!--`, and invariant 7 says that
input is expected rather than exceptional.

ti `95f55b` stopped the balancer appending a closer such a region would
swallow, because a buried closer closes nothing and each re-balance added
another (15,726 violations across 200,000 fuzz iterations). That bought
idempotence and **left the fragment unrepaired**, which `95f55b` recorded as a
deliberate carve-out pointing here.

## The harm was larger than the ticket said, and that changed the decision

The ticket described the region as swallowing what follows. That is true of an
unterminated **comment** only. The other three kinds end at the **first `>`** —
and in a mounted pane the next `>` is the sync wrapper's own `</div>`.

So the wrapper closed inside the region, the wrapper stayed open, and every
following block nested inside this block's wrapper. That is `contracts.md`
§4a's direct-child break: the same failure wave 1 (`<div/>`) and DCR-0041
(`<svg><div>`) were each fixed for as **live** breaks, and the harm spec §3.4
gives as the balancer's entire reason to exist.

Reachable from ordinary source Markdown, by construction: a type-6 html block
ends at a blank line, so `<div>x<!--` followed by a blank line and a paragraph
is one html block followed by a real anchor. Both panes are affected — the
balancer runs on each, and the splice preserves the trailing `<!--`.

## The decision

**The scanner decides what the region is; the balancer acts on that answer.**

`TagToken::Skip` gains `kind: SkipKind` and `terminated: bool`. Both facts are
known at each of the five EOF sites at the moment the region is produced, and
neither is recoverable downstream — in particular `<![CDATA[` is a CDATA
section in foreign content and a *bogus comment* in HTML content, which the
balancer cannot tell without the scanner's content-mode stack.

Having the balancer re-derive it from the bytes was the alternative, and it is
the two-places-one-opinion anti-pattern this crate has now removed four times
(`415cdb`, `e20490`, `2e2453`, and the `tag_name_end` extraction). It also
could not have got CDATA right.

The repair runs **before** the unclosed-elements pass, because a comment-only
block has no unclosed element and still swallows the wrapper's closer.

## Terminate, or delete — and why they are the same act

`SkipKind::terminator()` answers `Some` for a comment (`-->`), a CDATA section
(`]]>`) and a bogus comment (`>`), and **`None` for an unterminated tag**.

A browser at EOF-inside-a-tag *abandons* it — no element, no attributes — so
appending `>` would invent structure rather than recover it. And no single
terminator works: a tag cut inside a quoted value (`<div class="x`) needs the
quote closed first, and guessing that is parsing. The faithful repair is to
delete the span, which is the act the balancer **already performs** on an
orphan close tag.

That answers the ticket's central objection — "synthesising a terminator is a
different kind of act". Every operation this function already performs is that
act: appending `</div>` invents bytes, dropping an orphan closer deletes them.
Terminating a comment and deleting an EOF-cut tag are the same mandate applied
to a fourth region kind: **make the fragment parse at the wrapper seam the way
a browser parses it in isolation.**

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 44/44 binaries,
  **1176 passed, 0 failed**, `CARGO_EXIT=0`.
- Landed through the token pin's **two-bless protocol**. Movement between the
  blessings is exactly the four repairs:

  | corpus entry | before | after |
  |---|---|---|
  | `unterminated-comment` | `<div>x<!--` | `<div>x<!----></div>` |
  | `unterminated-bogus` | `<div>x<?pi` | `<div>x<?pi></div>` |
  | `unterminated-cdata-foreign` | `<svg><text><![CDATA[y` | `<svg><text><![CDATA[y]]></text></svg>` |
  | `unterminated-tag` | `<div>x<p` | `<div>x</div>` |

  **The token stream did not move at all** — this is a balancer change, and the
  `Open`/`Close` projection cannot see it. Nor did any earlier entry: no corpus
  entry and none of the three repository fixtures ended in an unterminated
  region, which is exactly why the pin could not see this class.
- `95f55b`'s re-scan guard is **kept** and becomes dead in practice, which is
  the point: it is now an invariant check rather than the mechanism.
- Two tests flipped, both asserting the old non-repair, and both were rewritten
  to the new contract. `skip_tokens_are_invisible_to_both_shipped_consumers`
  was renamed — its claim is now false for the balancer — to
  `terminated_skip_regions_stay_inert_for_both_shipped_consumers`, whose
  narrower claim is what has to keep holding.

## Semver

`TagToken` is a `pub`, exhaustive-by-policy enum in a published crate, so
adding fields to `Skip` breaks a downstream constructor or pattern. Legal
**only** while the v0.5.0 window is open, which is why this landed now rather
than after the tag — after it, the same change waits for v0.6.0.
