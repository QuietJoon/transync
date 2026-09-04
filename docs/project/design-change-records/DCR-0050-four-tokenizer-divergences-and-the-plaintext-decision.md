---
type: DCR
title: Four transync-html tokenizer divergences close, and PLAINTEXT gets the arm ti c1f9a8's rule never had
description: The raw-text element set gains xmp/iframe/noembed/noframes, comments learn HTML's `--!>` closing form, attribute values are decoded before the annotation-xml integration-point test, and `<plaintext>` becomes a passed-over region. All four ended in the same harm — the sync wrapper's own `</div>` swallowed — and all four are verified against Chromium rather than against this crate's own answers. PLAINTEXT is a region a browser never leaves, which ti c1f9a8's terminate-or-delete rule has no arm for; this record decides it is deleted, start tag included, and says why the two alternatives were rejected.
tags: [change, project-control, DCR-0050]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-05T00:00:00Z
status: stable
---

# DCR-0050: Four tokenizer divergences, and the PLAINTEXT decision

- **Date:** 2026-09-05
- **Source:** ticket `bebebe`, from Review 0010 findings R0010-0031, R0010-0032,
  R0010-0034 and R0010-0049 — triaged, then adversarially verified by a pass
  told to default to refuted; all four survived it
- **Affected contracts:** `docs/architecture/contracts.md` §4 (the comment
  terminator enumeration, which `--!>` makes incomplete)
- **Affected DCRs:** none amended. This is the fifth scanner/balancer change in
  the `DCR-0041`–`DCR-0043` / `DCR-0047` sequence and shares their shape: one
  rule, one home, HTML-content-qualified.

**Numbering note.** DCR numbering is independent of ADR numbering. DCR-0035–0039
remain reserved by the HTML→HTML waves; this is the next free number after
DCR-0049.

## What this record is for

`crates/transync-html` models HTML by hand, and every divergence it has ever had
ended in the same place: the sync wrapper's own `</div>` is swallowed, and every
following block's anchor mounts inside the previous block's wrapper. That is
`contracts.md` §4a and architectural invariants 1 and 7 at once, and it is
reachable from ordinary untrusted source Markdown through a CommonMark type-6
html block.

Four more of them are closed here. Three are ordinary corrections with an
obvious right answer. The fourth — `<plaintext>` — is a **decision**, because
HTML's PLAINTEXT state is the one tokenizer state with no exit, and ti
`c1f9a8`'s rule ("the region is terminated where a browser terminates it and
deleted where a browser abandons it") has no arm for a region a browser neither
terminates nor abandons. Inventing a third arm silently is what this record
exists to prevent.

## 1. The raw-text element set was short by four names (R0010-0031)

`RAW_TEXT_ELEMENTS` held `script`, `style`, `textarea`, `title`. HTML reaches
the **generic raw text element parsing algorithm** from "in body" for `xmp`,
`iframe` and `noembed`, and from "in head" for `noframes`, exactly as it does
for `style`. Their contents are text in every browser; the scanner read them as
markup.

`<div><iframe></div></iframe>` is **one** CommonMark type-6 html block — `div`
and `iframe` are both type-6 start-condition names — so a source Markdown file
carries it with no HTML-document run involved. `walk_elements` popped `iframe`
at `</div>` and `balance_fragment` deleted the author's `</iframe>` as an
orphan, emitting `<div><iframe></div>`: a fragment whose iframe is still open.
Chromium, asked directly, mounts the pane for that output with the following
block's anchor **absent** — `iframe` is in DOMPurify's default
`FORBID_CONTENTS`, so the swallowed remainder is dropped rather than merely
misparented.

`xmp` is also in `implicitly_closes`' paragraph-closing set, and both facts hold
at once: it closes an open `<p>` *and* its contents are raw text.

**`noscript` is deliberately not added.** HTML makes it generic raw text only
when the *scripting flag* is enabled, which is a property of the parsing context
rather than of the name, and `is_raw_text` answers the name question and leaves
context to the caller (the discipline ti `48f3c6` and ti `2e2453` established
for voidness and raw text). The case is real — every pane this crate feeds is
mounted by JavaScript, so the flag is always set there — and it is filed as its
own ticket rather than decided as a drive-by, because the same change would
move `intake::html`'s reading of every real-world `<noscript>` analytics
fallback.

## 2. `<plaintext>` — the decision

HTML's "in body" rule for a `plaintext` start tag inserts the element and
switches the tokenizer to the PLAINTEXT state. **Nothing switches it back.** No
end tag, no character sequence, no insertion mode: only EOF. So every byte after
`<plaintext>` is text in a browser, and the scanner had no such state at all —
`<div><plaintext></div>` walked as div → plaintext → both closed at `</div>` and
passed through byte-identical, while a browser turned the wrapper's `</div>`,
the next block's anchor and the rest of the pane into text.

### What was decided

`scan_tags` emits **one `TagToken::Skip` of the new kind `SkipKind::PlainText`,
spanning the `<plaintext>` start tag and every byte after it**, and stops. Its
`terminator()` is `None`, which routes it to the arm ti `c1f9a8` already built:
the balancer deletes it.

So the rule now reads: *terminated where a browser terminates it; deleted where
a browser abandons it **or will not leave it***. That is a widening of the
existing "delete" arm rather than a third rule, and the widening is what this
record authorises.

### Why the start tag is inside the deleted span

Two alternatives were considered and rejected:

* **Keep the element, delete only its contents.** The tag stays in the pane, so
  the tokenizer still switches, and the wrapper's `</div>` is still swallowed.
  It repairs nothing.
* **Delete only the tag, keep the bytes after it.** Those bytes then return to
  HTML's tokenizer as *markup*, after `strip_reserved_sync_attrs` has already
  passed over them as the *text* a browser makes of them. The render path is
  `balance_fragment(&strip_reserved_sync_attrs(md))`, so a
  `<div data-sync-id="…">` written after a `<plaintext>` would arrive in the
  pane as a live element carrying a live anchor. That is OI-0046's
  strip-then-balance bypass exactly, one region larger, and it is an invariant-7
  break rather than a cosmetic loss.

Emitting an `Open` token beside the `Skip` was rejected for the same reason in
reverse: two tokens over the same bytes leave `walk_elements` owing a
`</plaintext>` that the truncation would then orphan, and the strip and the
balancer would be reading different structures for one region.

### Why not escape the tail instead

Deleting the region loses the block's remaining text from the **pane** (never
from `out.md` / `out.html`, which the balancer does not touch). Re-encoding the
tail — replacing `<` and `&` so the bytes render as the characters a browser
shows — would preserve it. It was rejected on three grounds:

1. It makes the balancer an **editor of surviving content**, which is the trade
   `orphan_cut_would_weld` already refused in this crate, for this reason, in
   writing.
2. The content is not translatable content in this pipeline anyway: `lol_html`
   classifies it `TextType::PlainText`, which `wanted_text_type` excludes, so the
   pane would be showing untranslated source text inside a translated pane.
3. `plaintext` is an obsolete element HTML tells authors not to use. Input
   containing one is malformed input under invariant 7 — expected, but not
   owed fidelity at the cost of a new class of edit.

The loss is recorded here rather than left to be discovered, and it is bounded:
one block's pane rendering, in a document that contains an element no HTML
parser has been able to represent since 1994.

Like voidness (ti `48f3c6`), raw text (ti `2e2453`) and the `image` rename (ti
`e923ef`), this is an **HTML-content rule**. The switch is a tree-construction
rule of the "in body" insertion mode and foreign content has none, so
`<svg><plaintext>` is an ordinary foreign element; `plaintext` is not a breakout
tag, so nothing pulls it out of `<svg>` first.

## 3. Attribute values were compared undecoded (R0010-0034)

`tag_attr_value` returned the raw source slice, and `child_content_mode`
compared it to `text/html` / `application/xhtml+xml`. A browser decodes
character references in an attribute value before anything reads it, so
`encoding="text&#47;html"` **is** `text/html` and
`<math><annotation-xml encoding="text&#47;html"><div/>x` is an HTML integration
point in Chromium and was MathML content here.

The direction is under-close. The walk saw MathML content, the breakout rule
fired on `div`, and the balancer owed only `</div>`; a browser had opened the
`div` as HTML content and then **ignores** both `</annotation-xml>` and
`</math>`, because in-body's "any other end tag" stops at the special `div`. The
fragment therefore reaches the pane with `math`, `annotation-xml` and `div` all
open — and `annotation-xml` is a **scope terminator**, which makes the wrapper's
own `</div>` a token a browser ignores outright. Chromium reports
`parent-annotation-xml` for the following block.

The decode lives in `tag_attr_value` and not in `walk_attrs`, and the split is
the crate's usual one: `walk_attrs` reports byte **spans**, because
`strip_reserved_sync_attrs` cuts bytes out of the source with them and a decoded
string has no offsets to cut. `tag_attr_value` is the one accessor that hands a
caller a value to *read*, so it is the one place the decode belongs.
`htmlize::unescape_attribute` rather than `unescape`, because HTML's character
reference state has an attribute-value branch (the ambiguous-ampersand rule) and
that branch is the one a browser applies to these bytes. The crate already
depends on the same table for text nodes, so this is one decoder used in two
contexts rather than a second opinion about entities.

## 4. Comments have two closing forms (R0010-0049)

HTML's comment-end state closes on `>`; on `!` it moves to **comment-end-bang**,
where `>` also closes the comment (an incorrectly-closed-comment parse error —
the token is still emitted). So `--!>` closes a comment in every browser, and
the scanner searched only for `-->`.

`<!-- a --!>` + `<div>y` + `<!-- b -->` is **one** type-2 html block. The
scanner read the whole block as a single terminated comment, never saw the
`<div>`, and the balancer owed nothing; the still-open `div` then consumed the
sync wrapper's own `</div>` in the pane.

The answer lives in one new function, `comment_end`, for the same reason
`tag_name_end` is one function: "where does a comment end" is a question
`scan_tags` answers and `balance_fragment` acts on. The two searches begin at
**different offsets**, and the asymmetry is HTML's:

* `-->` is searched from the `<` itself, which is what models the
  abrupt-closing rules without a state machine — comment-start and
  comment-start-dash both close on `>`, so `<!-->` and `<!--->` are complete
  empty comments and a search from the `<` lands on exactly their last three
  bytes. This was the pre-existing behaviour and is unchanged.
* `--!>` is searched from **after** the opener, because comment-end-bang is
  reachable only *through* comment-end, which the opener's own `--` never
  enters. `<!--!>` is therefore not a closed comment, and a search from the `<`
  would have claimed it was.

`SkipKind::Comment::terminator()` still answers `-->`, and that stays correct:
it names the bytes that would *close* an unterminated comment, and appending
them to a fragment ending in `--!` still closes it (comment-end-bang →
comment-end-dash → comment-end → `>`).

## How this was verified, and why that matters here

Not against this crate's own answers. The previous harness certified this code
for months while every oracle it had was computed from `scan_tags` /
`walk_elements` / `balance_fragment` — the functions under test — so a
crate-versus-browser divergence was invisible by construction (ti `ec235f`).

Each of the four routes was put to **headless Chromium** directly, in the shape
`web/tests/html-oracle.spec.js` uses: mount the balanced fragment inside the
pane wrapper and ask the DOM where the following block's anchor landed. Before
the fix, all four produce a §4a break in a real browser — `break-absent` for the
`iframe` and `plaintext` routes, `break-nested` for the `annotation-xml` and
`--!>` routes. After the fix, all four have an empty divergence signature: the
two columns agree about everything they both answered, and the following block's
anchor is a direct child of the pane's `<main>`.

The generated browser-oracle corpus could not have found any of them, and that
is a fact worth recording rather than a coincidence: `ATOMS` contained no
`plaintext`, no `xmp`/`iframe`/`noembed`/`noframes`, no `--!>` and no
entity-spelled attribute value, so the four states were unreachable from the
generator. The generator gains an atom per state in the same commit, so the
oracle covers them from now on, and the census is re-blessed against the larger
population it now draws from.

## Consequences

* Good, because four `contracts.md` §4a breaks reachable from untrusted source
  Markdown are closed, and each is pinned by a regression whose assertion is the
  browser-observable consequence rather than a new return value.
* Good, because the PLAINTEXT arm is a **recorded** widening of an existing rule
  instead of a third rule discovered later in the code.
* Good, because the generator now reaches all four states, so the browser oracle
  is an ongoing check on them rather than a one-off measurement.
* Bad, because a `<plaintext>` block loses its remaining text from the pane.
  `out.md` / `out.html` are untouched, the element is obsolete, and the
  alternatives are argued above — but it is a loss and it is not silent only
  because this paragraph exists.
* Bad, because `noscript` is left diverging. It is filed, not forgotten, and the
  reason it is not fixed here is that its rule is context-conditional in a way
  the other four are not.
* Neutral, because the count that measures the crate's remaining divergence —
  `sec4a:break-nested`, 1,490 of 10,000 generated inputs at filing — is owned by
  the tree-construction tickets (`9b4d66`, `307283`, `895fb7`, `da6bb5`), not by
  this one. On the *identical* corpus it did not move at all, and the proof is
  byte-level rather than argumentative: a corpus emitted by the fixed crate over
  the old alphabet is `cmp`-identical to the one emitted at `14d6eda`. On the
  enlarged alphabet it reads 1,358 of 10,000, and that is a different
  population, not progress. This record does not claim the number fell.
* Neutral, because the enlarged alphabet immediately produced a **new** finding
  that is not this ticket's: ti `bb961a`, an ignored `</desc>` that puts the
  crate in foreign content (CDATA section) where a browser is in HTML content
  (bogus comment), which lands a live impostor `data-sync-id` in a real DOM.
  Same family as `9b4d66` / `895fb7`. Filed before it was pinned, per the
  oracle's own instruction.

## Verification

Every gate captured bare-to-file with its native exit status appended after it.

- `cargo test -p transync-html -- --test-threads=4` — **119 passed, 0 failed**
  (112 unit + 4 generative + 3 token-stream pin), `CARGO_EXIT=0`.
- `cargo test --workspace -- --test-threads=4` — **1331 passed, 0 failed**, 47
  suites, `CARGO_EXIT=0`.
- `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`
  — **222 passed, 0 failed**, `CARGO_EXIT=0`.
- `cargo clippy --all-targets --all-features -- -D warnings` — `CARGO_EXIT=0`,
  zero warnings.
- `cargo fmt --all` — `CARGO_EXIT=0`.
- `./scripts/test-browser.sh` — **44 passed, 1 failed**; the failure is
  `web/tests/engine.spec.js` test `l` (ti `8cd7ba`), red at `14d6eda` before any
  of this and unrelated to it.
- The six new regressions were **staged red** against `HEAD`'s implementation
  first: the same test bodies over the unfixed scanner fail 6/6, each reporting
  exactly the pre-fix output its finding describes.
- **Chromium, per route, both directions.** Each of the four inputs was mounted
  in the pane shape `web/tests/html-oracle.spec.js` uses. Before the fix all
  four carry a §4a break in a real browser (`sec4a:break-absent` for the
  `iframe` and `plaintext` routes — the following block's anchor is *gone*, not
  merely misparented — and `sec4a:break-nested` for the `annotation-xml` and
  `--!>` routes); after the fix all four have an empty divergence signature and
  `open@balanced` is `[]` on both sides.
- `web/tests/html-oracle.spec.js` — 3 passed, census re-blessed to
  7,697/10,000 agreeing and 3,874 abstentions with the reason for the movement
  written into the pin. None of the four `KNOWN_DIVERGENT` routes healed, so
  none was retired.
