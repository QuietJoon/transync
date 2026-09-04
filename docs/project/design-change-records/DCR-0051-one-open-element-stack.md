---
type: DCR
title: One open-element stack — transync-html's tree construction becomes browser-faithful
description: scan_tags and walk_elements kept two open-element stacks that popped differently from each other and from a browser, which is the single root cause behind five tickets — two of them putting an attacker-chosen data-sync-id into a live DOM. The scanner now keeps THE stack and the walk replays it, and that stack carries HTML's special-element guard, its four scopes, foreign content's end-tag dispatch (including the breakout `</p>` and `</br>`), the start tags "in body" ignores, and the mglyph/malignmark integration-point exception. Measured against Chromium: agreement 7,697 -> 8,619 of 10,000 on an unchanged alphabet, sec4a:break-nested 1,358 -> 8, reserved:dom-only 1 -> 0. The adoption agency algorithm is deliberately not implemented and the residual is measured and ticketed.
tags: [change, project-control, DCR-0051]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-05T00:00:00Z
status: stable
---

# DCR-0051: One open-element stack

- **Date:** 2026-09-05
- **Source:** tickets `9b4d66` (P1), `307283` (P2, with R0010-0039 and
  R0010-0043 folded in), `895fb7` (P2), `da6bb5` (P2), `bb961a` (P1) and
  `a7e625` (P2, spec-traced and now browser-confirmed)
- **Affected contracts:** `docs/architecture/contracts.md` §4 (the balancer's
  guarantee, and the residual it now carries) and §4a (direct-child survival)
- **Affected DCRs:** DCR-0032, DCR-0041, DCR-0042, DCR-0043, DCR-0047 and
  DCR-0050 each gain a dated note. None is rewritten; each remains correct as
  written for its own date.
- **Follow-up filed:** ti `525bef` — the adoption agency algorithm and the list
  of active formatting elements, the one part of HTML's algorithm this record
  deliberately leaves out.

**Numbering note.** DCR numbering is independent of ADR numbering. DCR-0035–0039
remain reserved by the HTML→HTML waves; this is the next free number after
DCR-0050.

## What this record is for

The dominant defect class in this repository is **two places holding one
opinion**, and this ticket family IS that defect. `scan_tags_with_state` kept a
`mode_stack` so it could answer "what content mode is the next byte tokenized
in". `walk_elements` kept an `open_stack` so it could answer "which element does
this closer close". Both are the *same* stack — HTML's stack of open elements —
and they popped differently from each other and from a browser.

The in-code comment that licensed the divergence read:

> Implied closes (see `implicitly_closes`) are deliberately NOT applied here, so
> this stack can hold frames `walk_elements` has already popped. That cannot
> change the answer: every name in an `implicitly_closes` set is an ordinary
> HTML element, so its child mode equals its parent's, and an extra frame of
> that shape reports the mode the one below it would.

Every clause of that is true, and the conclusion does not follow. It reasons
about the extra frame's own content mode and never about what `truncate` does to
the frames sitting **above** it. `<p><ul><svg></p>` left a stale `p` at the
bottom of the scanner's stack; the close tag found it by name; and the `<svg>`
came off with it.

The test that comment named as its falsifier was green throughout, and for two
reasons rather than one. The first is the ordinary one: it pinned the *premise*
(the extra frame's own mode) rather than the conclusion (what `truncate` pops),
which is exactly why a green test is not the same as a checked claim. The second
is worth recording because it is the sharper failure — **the comment cited the
test under a name it has never had.** There is no
`implicitly_closed_names_never_change_the_content_mode` anywhere in this
repository's history; the real test is
`no_implicitly_closed_name_changes_the_content_mode`, and a reader following the
comment to its evidence would have found nothing. A citation nothing enforces is
the same shape as the reasoning it was defending.

The test is **kept and re-scoped**, not deleted, because DCR-0042 references it
by name and long-lived records should not be broken to tidy a docstring. Its two
surviving claims — that `KEYS` is closed under `implicitly_closes`, and that no
implicitly-closed name moves a content mode — are stated in its own docstring
along with what it stopped being.

So the fix is not "teach the scanner about implied closes too" — that is a
third opinion. It is one stack.

## The shape of the change

`scan_tags_with_state` keeps **THE** open-element stack, because it is the layer
that cannot do without one: HTML's tokenizer is not standalone, and whether
`<script>` opens a raw-text run, whether `<![CDATA[` ends at `]]>` or at the
first `>`, and whether a `/` is honoured all depend on it. Every mutation it
makes is reported on the token:

```rust
struct ScannedTag {
    token: TagToken,
    /// Frames popped BEFORE the token was processed: implied end tags, and the
    /// pops a breakout start tag — or a breakout `</p>` / `</br>` — performs to
    /// leave foreign content.
    pre_pops: usize,
    effect: StackEffect,
}

enum StackEffect {
    Push,
    /// Closed `n` frames; the deepest is the element this tag closed.
    Close(usize),
    /// Moved nothing, and the bytes stay.
    Inert,
    /// Moved nothing, and the bytes must go.
    Orphan,
}
```

`walk_elements` **replays** that and adds only what the scanner has no use for:
extents, close spans, orphan spans. Its parallel stack carries a name and an
extent index and does no searching at all. This is the same move ti `e20490`
made with `tag_name_end` serving both the scanner and the strip, and DCR-0050
made with `comment_end` serving both the scanner and the balancer — one rule,
one home, read by everyone who needs it.

Each frame carries **two** content modes, because HTML asks two different
questions of an open element and one field cannot answer both:

* `child` — the mode its children are read in. This is what the tokenizer needs
  and what `current_mode` reports.
* `own` — the element's own namespace. This decides whether an end tag is
  dispatched to the foreign-content rules, whether the frame is in HTML's
  **special** category, and whether it terminates a scope search.

`<svg><desc>` is the pair that separates them: `own == Svg`, `child == Html`.
Reading the wrong one is ti `bb961a`.

## What was implemented

### 1. The special-element guard (ti `307283`, ti `bb961a`)

In-body's "any other end tag" walks down from the current node and **stops at
the first special element**, ignoring the token. Without it, `</b>` in
`<div><b><div></b></div>` closed the inner `div`, the balancer called the
fragment balanced, and the pane's own `</div>` was consumed. And in
`<svg><desc><div></desc>`, `</desc>` popped back into foreign content, where
`<![CDATA[` is a real CDATA section running to EOF — so everything after it was
text to `strip_reserved_sync_attrs` and markup to the browser, and a planted
`data-sync-id` reached a live DOM.

The category is namespace-qualified: MathML's `mi`/`mo`/`mn`/`ms`/`mtext`/
`annotation-xml` and SVG's `foreignObject`/`desc`/`title` are special too, and
they are the *same* names that terminate a foreign scope search. One list, two
questions.

### 2. Four scopes (ti `307283` / R0010-0039, ti `da6bb5`)

Normal, list-item, button and table scope, with HTML's own terminator lists.
This is what distinguishes the block-level end tags — which walk straight
*through* a `<section>`, because a scope search stops only at a scope
TERMINATOR — from "any other end tag", which stops at any special element.

Table scope is the half R0010-0039 named. `<div><table>a</div>` has no `div` in
scope, because `table` terminates the search: Chromium ignores the closer and
leaves the table open, and this crate used to pop the table at the nearest name
match. In a pane the wrapper's own `</div>` was ignored the same way and the
next block's `<p data-sync-id>` was foster-parented **inside** the previous
block's wrapper — measured `parent-div`.

Button scope is ti `da6bb5`'s second half: `<ul>` closes an open `<p>` *in
button scope*, which reaches through the `<em>` sitting on top of it. The crate
closed only the innermost frame.

### 3. Foreign content's end-tag dispatch (ti `9b4d66`, ti `895fb7`)

`</p>` and `</br>` are **breakout end tags**: HTML lists them alongside the
breakout START tags, so they pop out of `<svg>`/`<math>` and are then
reprocessed by the HTML rules. This is what `<p><ul><svg></p><title>…` really
does in a browser, and the crate reached the same reading by accident and lost
it the moment the balancer deleted the `</p>` as an orphan.

Every other end tag walks down the foreign run looking for its own name and
falls through to the HTML rules at the first HTML-namespace frame — **in that
step order**, which is load-bearing. The name test applies to the current node
and to each *foreign* node below it, and the namespace test comes first on every
step after the first. Testing the name at an HTML-namespace frame as well let
`<svg><foreignObject><p></foreignObject></svg>…</div>` walk past the
`foreignObject` — a scope terminator — and match the sync wrapper's own `<div>`.
That is §4a's break arriving *through* the repair meant to prevent it, and it
was found by the 400,000-input deep property run while this record was being
written, not by review.

### 4. The start tags "in body" ignores (ti `da6bb5`)

`caption`, `col`, `colgroup`, `frame`, `tbody`, `td`, `tfoot`, `th`, `thead` and
`tr` outside a table are a parse error the parser IGNORES — no element, no
frame. Pushing one made the balancer append a `</td>` or `</tr>` to a fragment
HTML needs none for, which is inventing structure in a pane: the direction
`implicitly_closes`' own docstring says the walk must not take.

### 5. The MathML text-integration-point exception (ti `a7e625`)

**Confirmed by measurement, not accepted from the spec trace.** The ticket was
explicit that it was spec-traced and unmeasured, and asked for the oracle to be
run before anything was fixed. Chromium, on `<math><mi><mglyph><script><div>`:

```text
<math><mi><mglyph><script></script></mglyph><div></div></mi></math>
```

with `["math","mi","div"]` still open. `mglyph` and `malignmark` are the two
start tags HTML's tree-construction dispatcher does *not* hand to the HTML rules
inside a MathML text integration point, so they stay MathML and a `<script>`
beneath one is a foreign element rather than a raw-text run. `malignmark`
measures identically. The crate read `<script>` as raw text, made `<div>` its
text content, and appended four closers a browser then ignores.

The dispatcher's sibling exception rides along: an `<svg>` start tag inside a
MathML `annotation-xml` IS handed to the HTML rules, whatever the element's
`encoding`, so it opens a real SVG element.

### 6. A nested `<svg>` inside `<math>` is a MathML element

Not in any ticket; it fell out of separating `own` from `child` and is measured.
Foreign content's "any other start tag" inserts in the namespace of the adjusted
current node, so `<math><svg>` is a **MathML** element that happens to be named
`svg`. Chromium parses `<math><svg><desc><div>` with the `div` at top level —
`desc` under a MathML `svg` is not an SVG HTML integration point, so the
breakout tag leaves foreign content entirely. Reading the name without the
parent claimed an HTML island that is not there.

## Two decisions worth stating rather than inferring

### An end tag that closes nothing is not automatically an orphan

The balancer deletes orphan close tags, and that deletion is what keeps the sync
wrapper's own `</div>` safe. It now deletes strictly fewer of them, and the line
it draws is the pane's:

* **`Orphan`, deleted** — the search ran off the bottom of *this fragment's*
  stack with nothing to stop it. A fragment is mounted INSIDE the wrapper's
  `<div>`, so a browser's search continues past that bottom and into the
  wrapper. `<svg><g></div>` is the shape: measured, the `</div>` closes the
  wrapper.
* **`Inert`, kept** — a special element or a scope terminator *the fragment
  itself contributes* stopped the search, above anything the wrapper adds. A
  browser mounting the pane stops in the same place, so the bytes are inert
  there too, and deleting them would edit a reader's pane for nothing.

Deleting on an approximation is not the same as deleting on a browser's verdict,
and this crate's verdict for a *formatting* end tag is an approximation (see the
residual below). `</b>` in `<div><b><div></b></div>` is a token this crate
declines to act on while a browser reparents through it; deleting it produced a
different DOM.

`</p>` and `</br>` are never orphans at all. A browser turns each into content —
`</p>` out of button scope inserts an empty paragraph and closes it, `</br>`
mints a `<br>` — so deleting one silently removes something the author's own
bytes render. Measured: `<p>a<div>b</div>c</p>` renders
`<p>a</p><div>b</div>c<p></p>`. This reverses one line of R0002-0061's fix,
which was about the balancer **appending** a second `</p>`; deleting one the
author wrote is the different act of editing the pane, and
`optional_end_tags_do_not_accumulate_phantom_closes` now pins the fragment as a
fixed point instead.

### P2's orphan clause moved into P1

`crates/transync-html/tests/generative_properties.rs` asserted that a balanced
output carries no orphan closers, and reconstructed the orphan set from the
public surface — "every `Close` token no extent claims". That reconstruction is
now over-inclusive by design, because HTML has end tags that close nothing and
are not orphans. Re-deriving the real set there would be a second opinion about
exactly the question this record removed a second opinion from, so the clause is
gone and P1 carries it: `balance_fragment` deletes exactly the closers whose
search runs off the bottom, so an output that still held one would not be a
fixed point. The helper survives for the two weld counterfactuals, which want
the pre-OI-0046 behaviour and are comparisons rather than predicates.

## What was NOT implemented, and why the omission is safe

**The adoption agency algorithm and the list of active formatting elements.** A
formatting end tag is routed through "any other end tag" instead. Where the two
differ, AAA removes the formatting element from the stack and inserts a clone
deeper, while this keeps the frame — so the walk holds a formatting frame a
browser has moved, and the balancer appends one redundant `</b>`-shaped closer.
That closer is a no-op in a browser (AAA drops an end tag whose formatting
element is not on the stack), a formatting element is never special and never
terminates a scope, and the direction is the safe one: an extra frame appends a
closer rather than losing one.

That argument covers the closer the balancer appends. It does **not** cover
RECONSTRUCTION — when a browser inserts the next element it reconstructs the
active formatting elements, so a `<b>` the crate has closed reappears as a real
frame, and that frame is what the appended closers then have to get past. The
residual is measured (8 §4a cases in 10,000, down from 1,358) and owned by ti
`525bef`, which carries the exemplar and the mechanism.

**HTML's insertion modes.** This crate has none, and three consequences are
deliberate:

* `head`, `body`, `html` and `frameset` are on the same in-body ignore list as
  the table-section start tags and are **not** ignored here, because
  `intake::html` walks whole HTML *documents* through `element_extents`, where
  those four are processed in "before head" / "in head" / "after head" and
  really do open elements. A fragment carrying a second `<body>` therefore still
  opens one — the over-open direction.
* `</body>` and `</html>` are treated as ordinary scope-based block end tags
  rather than as insertion-mode switches, which is what an intake reading a
  whole document needs from them.
* "Clear the stack back to a table context" is not modelled; the table-section
  start tags keep `implicitly_closes`' top-of-stack pops. Over-open again, and
  the balancer's appended `</td></tr></table>` is measured balanced in Chromium.

**Foster parenting.** Not modelled, and it is why the oracle's `open@input`
column disagrees on table fragments even where §4a is fine: Chromium's insertion
point for `<div><table>a` is *before* the table, so its sentinel reports `["div"]`
where the crate reports `div` and `table` open. The balanced output is measured
balanced; the disagreement is about where the next byte lands, not about whether
the wrapper survives.

## How this was verified, and why that matters here

Every expected value in `tree_construction_tests` was **measured in headless
Chromium before it was written**, through the same `innerHTML`-on-a-detached-
`div` fragment parse the shipped shells mount a pane with, and the measurement
is quoted beside each one. This is not ceremony: the previous harness certified
this code as correct for months because every oracle it had was computed from
`scan_tags` / `walk_elements` / `balance_fragment` themselves, so a divergence
between this crate's stacks and a browser's was invisible by construction. That
is the finding ti `ec235f` was filed for.

Eleven new tests were written against HEAD's code first. **Ten of the eleven
were red** (the eleventh is discussed below), each with the exact pre-fix output
its ticket names — `<div><b><div></b></div>` came back unchanged, `<div><table>a</div>`
came back unchanged, `<math><mi><mglyph><script><div>` came back
`…<div></script></mglyph></mi></math>`, and the ti `9b4d66` chain came back with
its `</p>` deleted and the impostor `data-sync-id` live.

The eleventh — ti `bb961a`'s generated exemplar — passed at HEAD and **could
not** have failed there, and that is the ticket's own point restated as
evidence. The strip was right that those bytes were text *under the crate's own
reading*, and the reading was a browser's only if the `</desc>` had been
honoured. An in-crate property cannot see a defect in which the crate is
self-consistently wrong; only the browser column can. It is pinned here as a
regression whose falsifier lives in the oracle.

## Consequences

**The census, on an unchanged alphabet.** `ATOMS` and `PHRASES` are untouched, so
DCR-0050's lesson applies in the other direction: this is the same population and
the numbers compare directly.

| | before | after |
|---|---|---|
| cases in full agreement (of 10,000) | 7,697 | **8,619** |
| browser-declared abstentions | 3,874 | 3,863 |
| distinct divergence mechanisms | 13 | **9** |
| `sec4a:break-nested` | 1,358 | **8** |
| `reserved:dom-only` | 1 | **0** |

The abstention count barely moving is the load-bearing half of that table:
agreement did not rise by making Chromium stop answering.

**The `KNOWN_DIVERGENT` ledger retired three of its four entries**, from both
halves — the Rust emitter's list and the spec's — in this change. ti `9b4d66`,
`<div><b><div></b></div></div>` and ti `895fb7`'s `<p/><ul><math></p><div ">`
each measure HEALTHY, which the spec treats as a failure on purpose. The
survivor, `<div><b><div></b></div>`, survives for a different reason than it was
filed for: its §4a break is closed and only the adoption-agency disagreement
about the input remains, so it is re-blessed under ti `525bef`.

**`reserved:dom-only` reaching zero is the security half.** Two of these tickets
put an attacker-chosen `data-sync-id` into a live DOM, which defeats the layer
OI-0035's closure depends on: "ours are the only sync attributes in this DOM" is
a construction, not a scan (DCR-0033), and `web/js/sync.js`'s own documented
residual is that a *listed* impostor preceding the genuine anchor wins. That
route is gone by measurement, not by argument.

**No golden moved, in the whole workspace.** 1,300+ tests across nine members,
including `token_stream_pin.rs`'s byte-exact token, inventory and balancer
goldens and every `transync-syntax` render and intake fixture. That is checkable
rather than lucky: the paths this record changed are misnesting, stray
table-section tags, foreign-content end tags and formatting closers, and no
golden corpus in this repository contains one — which is the same coverage gap
`generative_properties.rs` was written for and the reason the browser oracle
exists at all.

## Verification

- `cargo test -p transync-html -- --test-threads=4` — 124 + 4 + 3 pass, 0
  fail. `tree_construction_tests` contributes 12 of the 124: the eleven above
  plus the one the deep run found.
- `cargo test --workspace -- --test-threads=4` — 1,343 pass, 0 fail, no golden
  moved.
- `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`
  — 222 pass, 0 fail.
- `TRANSYNC_HTML_DEEP_PROPERTIES=1 cargo test -p transync-html --test
  generative_properties -- --ignored the_properties_hold_over_a_deep_generated_run`
  — 400,000 generated inputs, 8,124 of them weld sites, 0 excluded, all six
  properties hold. This run is what found the foreign-content step-order defect
  in §3 above.
- `cargo clippy --all-targets --all-features -- -D warnings` — clean.
- `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  — exit 0. Nothing here adds a dependency, a feature or a non-`wasm32` API, and
  the standing gate says so rather than the reasoning.
- `./scripts/test-browser.sh` — the oracle's three tests pass on the re-blessed
  census. `web/tests/engine.spec.js` test `l` is red before and after and is not
  this change's (ti `8cd7ba`).
