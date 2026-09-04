---
type: DCR
title: Raw text and RCDATA are HTML-content states, and the scanner owns the content-mode model both layers need
description: scan_tags enters raw-text/RCDATA only in HTML content, so inside svg/math those four names are ordinary foreign elements whose contents are markup and whose self-closing slash is honoured — which lets SVG title be listed as the HTML integration point the spec makes it, and stops a planted sync attribute there from surviving the strip.
tags: [change, project-control, DCR-0042]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-01T00:00:00Z
status: stable
---

# DCR-0042: Raw text is an HTML-content state, and the scanner owns the mode

- **Date:** 2026-09-01
- **Source:** ticket `2e2453`, measured in real Chromium while designing ti `490d97` wave 1
- **Affected DCRs:** `DCR-0041` (its "SVG `<title>` as an integration point" residual is closed), `DCR-0032` (its "foreign raw-text/RCDATA" divergence bullet is closed), `DCR-0016` Part D (refined, not reverted)
- **Affected records:** R0002-0020 (refined — see below)

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This record takes the next genuinely free number after DCR-0041.

## What was wrong

`scan_tags` entered raw-text / RCDATA state for `script`, `style`, `textarea`
and `title` **unconditionally** — inside `<svg>` and `<math>` too. A browser
does not. Those states are entered by the tree construction stage from the "in
body" insertion mode; the "in foreign content" insertion mode has no such rule,
so inside foreign content all four are ordinary foreign elements. Measured:

- `<svg><title>a<b>c</b></title>` mints a real `<b>` element. The scanner read
  the interior as RCDATA, so neither the `<b>` nor its closer existed for any
  consumer.
- `<svg><script/>x` honours the self-closing flag — foreign content is one of
  the two places HTML does — leaving the script empty and `x` a sibling. The
  scanner entered raw text and swallowed `x` to EOF, so `balance_fragment`
  appended a `</script>` closing an element a browser never opened.

Two consequences followed from the first one, and the second is the one that
matters:

1. **`is_svg_html_integration_point` could not list `title`.** The spec makes
   it one. `walk_elements` deliberately omitted it and said why: claiming it
   would assert a fidelity the tokenizer one layer down did not provide.
2. **`strip_reserved_sync_attrs` could not reach a planted anchor there.** In
   `<svg><title><div data-sync-id="p-0001">`, a browser sees a live
   `data-sync-id` on a real `div`; the scanner saw RCDATA text and walked past
   it. The strip is the construction behind "ours are the only sync attributes
   in this DOM" (OI-0035 route (c)), so a surviving anchor there is the same
   defect shape as ti `e20490`, one region further out — a disagreement about
   where a tokenizer state begins, cashed out as a live anchor.

## What changed

**The qualifier, not the rule.** `is_raw_text` still names the same four
elements; `scan_tags` now enters the state only when the tag's parent mode is
HTML content. R0002-0020's motivation is untouched by that — it was about the
tokenizer disagreeing with the balancer over user-visible RCDATA text, all of
it in HTML content — so this refines the rule rather than reverting it, which
is what that ticket asked any change here to check first.

**The self-closing carve-out goes with it.** `honours_flag` carried a
`!is_raw_text(name)` term, because the scanner *did* enter raw text inside
foreign content and an unhonoured flag was what kept the appended closer
visible to `balance_fragment`'s re-scan (DCR-0016 Part D). In HTML content that
term could never fire — no raw-text name is a foreign root — and inside foreign
content it was the bug. Part D's behaviour at top level is unchanged:
`<textarea/>` still earns its `</textarea>`.

**The content-mode model moved down a layer, and there is now one of it.**
DCR-0041 gave `walk_elements` a three-valued `ContentMode` per open element.
The tokenizer needs the same answer — whether `<script>` opens a raw-text run
depends on it — and HTML says so directly: the tree construction stage feeds
back into the tokenizer. So the stack moved into `scan_tags`, replacing
R0003-0067's `foreign_depth` counter, and `walk_elements` now reads the
scanner's verdict (`child_mode`, `opens_element`, `broke_out`) instead of
deriving a second one. Two places deciding where foreign content begins is
precisely the defect shape ti `415cdb` and ti `e20490` both were; `tag_name_end`
was that remedy for the tag-name boundary, and this is it for the region.

A counter would not have done. Inside an HTML integration point the children
are HTML content again, so `<svg><foreignObject><script>` *does* hold raw text
— a `foreign_depth > 0` suppression gets that backwards. The corpus entry
`foreign-integration-rawtext` exists to fail if anyone tries it.

**The scanner does not model optional end tags.** Its stack can therefore hold
frames `walk_elements` has already popped. That is sound only while no
implicitly-closed name changes the content mode, which is true today and is now
pinned by `no_implicitly_closed_name_changes_the_content_mode` rather than left
to a comment — the live direction being exactly the one this change walked:
adding a name to `is_svg_html_integration_point`.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 40/40 binaries,
  **1142 passed, 0 failed**, `CARGO_EXIT=0` (1136 before, plus six new tests).
- Landed through the token pin's **two-bless protocol**. The first commit adds
  three corpus entries and blesses the BROKEN scanner; the fix re-blesses. The
  movement between the blessings is the whole record, and it is three lines:
  `foreign-title-markup` gains `O:b|C:b` in the token stream, and
  `foreign-script-selfclose` loses the invented `</script>` from its balanced
  output. `foreign-integration-rawtext` did not move — the guard held — and
  neither did the fifteen wave-0-era entries nor any `stray-*`, `selfclose-*`,
  `tagname-*` or `endtag-*` entry.
- `foreign-script-selfclose`'s **token stream did not move either**, only its
  balanced golden. The swallowed `x` holds no tag-shaped bytes, so the
  projection could not see it. That is the pin's own documented limit
  ("a green token-stream pin is not evidence that nothing moved") observed in
  the wild, and the second time this session that a real defect was visible
  only in a consequence.

## What is still not modelled

- **Void names used as real foreign elements.** `is_void` still wins globally,
  so a genuine `<svg><link>…</link>`'s closer is dropped as an orphan. The
  stack-scoped mode this record moves into the scanner is exactly what closing
  it needs; it is ticket `48f3c6` and it stays open here.

  *Closed the same day by that ticket, recorded in **DCR-0043**.* It used the
  mode this record had just moved down, and cost one expression.
## Amendment (2026-09-05) — the premise this record's test was defending is gone

*An appended note, not a rewrite. Everything above stands as written.*

This record states that "the scanner does not model optional end tags", so its
stack can hold frames `walk_elements` has already popped, and that this is safe
while no implicitly-closed name changes the content mode — pinned by
`no_implicitly_closed_name_changes_the_content_mode`.

**DCR-0051 removed the premise.** There is one open-element stack now, it
applies HTML's implied end tags, and the scanner cannot hold a frame the walk
has popped because the walk keeps no stack of its own to pop from. The safety
argument was also wrong on its own terms: it reasoned about the extra frame's
own mode and never about `truncate` popping the frames above it, which is ti
`9b4d66` — a live impostor `data-sync-id` in a reader's DOM.

The test **keeps its name** so this reference resolves, and its docstring says
what it now pins: that `KEYS` is closed under `implicitly_closes`, and that no
implicitly-closed name moves a content mode. Everything else in this record —
raw text and RCDATA as HTML-CONTENT states, `title` as an SVG integration point,
the content-mode stack living in the scanner — is unchanged and is what DCR-0051
built on.
