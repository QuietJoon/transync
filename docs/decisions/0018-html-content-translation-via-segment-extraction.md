---
type: ADR
title: HTML content is translatable via app-owned segment extraction
description: Block-level raw HTML becomes a translatable kind (BlockKind::Html). The application extracts text segments with lol_html and sends the LLM only text — tags never reach the model — then splices translations back per text node, so markup is preserved by construction. Successful blocks live-render through the shells' DOMPurify mount; the escaped placeholder becomes the failure presentation. Amends invariant 7. Owner-settled 2026-08-03.
tags: [decision, ADR-0018]
status: active
---

# ADR: HTML content is translatable via app-owned segment extraction

> **Status: accepted.** Owner-settled 2026-08-03 (design spec
> `docs/superpowers/specs/2026-08-03-html-content-translation-design.md`, v2,
> approved after a two-agent ground + adversarial review); implemented and
> landed the same window. This ADR records a decision that **changes** shipped
> behavior — the DCR-0013 "raw HTML is always an escaped placeholder" posture
> — and **amends architectural invariant 7**.

## Context and Problem Statement

Real-world documents — GitHub READMEs above all — carry meaningful reader
content inside block-level raw HTML: `<details>` sections with `<summary>`
labels, HTML tables, centered `<div align="center">` hero blocks. Since
DCR-0013 that content is preserved byte-verbatim in `out.md` and rendered in
both panes as an escaped `<pre data-skipped="html-block">` placeholder: honest,
but untranslated and unreadable as content. The 2026-07 design-review fitness
verdict (DR-2026-07) named this the largest remaining reader-value gap.

Making it translatable is not a matter of "send the block to the model." Two
facts constrain any design:

* **Structure must stay application-owned** (invariant 2). If the model
  receives markup it can reword an attribute, drop a close tag, or reorder
  elements — and every such change is either a false-reject (retry burn) or a
  structural corruption the validator has to catch after the fact.
* **These blocks are normally *unbalanced fragments*.** By CommonMark's own
  rules a `<details>` region whose body is processed as Markdown must contain
  blank lines, and a type-6/7 HTML block ends at the first blank line. So the
  flagship pattern parses as an *opening* fragment
  (`<details><summary>…</summary>`), ordinary Markdown blocks, and a *closing*
  fragment (`</details>`). The same holds for `<div align="center">` heroes.
  No design may assume an HTML block is a complete element tree.

Invariant 7's closing sentence ("Raw HTML in v1 is disabled / escaped /
rejected") encoded the safe-by-abstention answer. The question this ADR settles
is whether that abstention can be replaced by a mechanism that is safe by
*construction* instead.

## Decision Drivers

* **Tags must never reach the model.** The only way to make "the LLM owns
  content, the application owns structure" true for HTML is to not show the LLM
  any structure. Then a structural violation is not merely detected — it is
  unrepresentable in the model's output space.
* **Byte-exact preservation of untouched markup.** Anything that reserializes
  the block (parse → tree → print) drifts attribute quoting, whitespace, and
  entity forms, and every drift is a diff the validator must either accept
  (loosening the contract) or reject (burning retries on the model's behalf).
* **Fragments are the normal case, not an error case.** A design that treats an
  unclosed `<div>` as a fault would reject the most common real-world shape.
* **Live rendering is the point.** A translated `<summary>` that still renders
  as escaped source text delivers none of the reader value that motivated the
  work. But live-mounting author-supplied HTML must ride the sanitization path
  the panes already have, not a new one.
* **Sync integrity outranks fidelity.** Mounting an unbalanced fragment as-is
  lets the browser's tree correction consume the sync wrapper's own `</div>`,
  so an unclosed hero `<div>` would swallow *every subsequent sync anchor* into
  its wrapper and degrade the document tail to proportional mapping — a direct
  invariant-1 violation. Whatever renders must be structurally inert toward its
  siblings.
* **Degrade, never abort.** HTML is the messiest input in the corpus; every new
  failure mode must land on an existing honest rung (placeholder + recorded
  status), not on a new terminal one.

## Considered Options

1. **Segment extraction (approach A).** The application parses the HTML and
   sends the LLM only the ordered list of text segments; tags never appear in
   the translatable payload (neighbor-context summaries may still carry raw
   markup as untrusted context data). Translations are spliced back per text
   node, positionally.
2. **Whole-block round-tripping (approach B).** Send the whole HTML block, take
   the whole HTML block back, validate it structurally.
3. **HTML → Markdown → HTML round-trip (approach C).** Convert the block to
   Markdown, translate it as an ordinary Markdown unit, convert back.

## Decision Outcome

**ACCEPTED: option 1 (segment extraction), with `lol_html` as the extraction
engine.** Options 2 and 3 are rejected — B for reserialization false-rejects,
retry burn, and output-ceiling pressure (the model must echo all the markup it
was shown); C as lossy by construction (attributes, `<summary>`, arbitrary
nesting have no Markdown form).

The owner settled nine points, all binding:

1. **Live rendering.** Translated and cleanly-preserved HTML blocks render as
   real content in both panes through the shells' existing DOMPurify
   fail-closed sanitize-before-mount path. Live rendering is a
   translated-content *privilege*.
2. **Scope: blocks + an inline guard.** Block-level HTML becomes translatable.
   Inline raw HTML inside paragraph-family payloads gains a tag-identity
   guard (tags must survive verbatim; text may change). **Attribute text
   (`title`/`alt`/`aria-label`) is not translated** — preserved verbatim.
3. **Mechanism: segment extraction** (this decision's option 1).
4. **Parser dependency: `lol_html`** (Cloudflare's streaming rewriter) in
   `transync-core`, for byte-exact preservation of untouched markup,
   text-handler rewriting as its core use case, and leniency on real-world
   HTML. `htmlize` supplies the named-entity decode table `lol_html` lacks.
5. **Render-side fragment auto-balancing.** Unbalanced fragments translate
   normally; the *render path* closes tags opened but never closed within the
   fragment and drops orphan close tags. `out.md` keeps the true translated
   fragment bytes. Accepted consequence: an interleaved `<details>` region
   renders with its body always visible — GitHub's fold is not reproduced.
6. **Per-segment quality ceiling accepted.** Text-node granularity caps
   translation quality across inline-tag boundaries (CJK word-order reflow
   cannot cross segments). Mitigated by a prompt instruction that segments
   sharing a parent are pieces of one sentence.
7. **JSON-in-string wire kept.** The provider schema can only constrain the
   payload as a string; inner count/JSON validity are per-kind validation's
   job. A native-array wire field is the recorded, telemetry-gated escape
   hatch — not built.
8. **Zero-segment blocks may vanish.** Comment-only and `script`/`style`-only
   blocks live-render as visually empty anchors (DOMPurify strips them at
   mount, matching GitHub's own rendering). An info-level warning row keeps the
   disappearance visible in the report.
9. **`<details>` toggle mirroring.** The shells mirror `open` across panes so
   anchor geometry stays congruent — JS interaction ownership, not structure.

### Invariant 7 amendment

The sentence "Raw HTML in v1 is disabled / escaped / rejected" is **replaced**
(CLAUDE.md, this wave):

> Raw HTML blocks are translatable, structurally-owned content: the application
> extracts text segments (`lol_html`), the LLM sees only text, splice-back
> preserves markup by construction, and live rendering rides the shells'
> DOMPurify fail-closed mount. The escaped placeholder is the *failure*
> presentation (extraction failure / fallback). Inline raw-HTML tags are
> guarded verbatim.

The rest of invariant 7 — source Markdown is untrusted data, the system prompt
declares it as data, output is schema-validated — is untouched and is *more*
load-bearing now, not less: the extracted segments are document data that
travels into a prompt, and the same data-framing discipline covers them.

### Implementation

Recorded in full in **DCR-0016**; the decision-relevant shape:

*Path note (2026-08-04, DCR-0017): the module paths below are as of this
record's date. `htmlseg`, the parser, and `HtmlOutcome` moved to
`crates/transync-syntax/src/` in the crate split — `htmlseg.rs` and
`parser.rs` at the same filenames, `HtmlOutcome`/`html_outcomes` into
`outcome.rs` (still reachable as `transync::unit::html_outcomes`). The
decision is unaffected: one parser owns block boundaries, `lol_html` works
inside a block, and both now live in the same crate.*

*Path note 2 (2026-08-04, DCR-0018): the `transync::unit::html_outcomes`
path cited just above is no longer reachable. The OI-0027 curation made
`transync-core`'s `unit` module `#[doc(hidden)] pub` and dropped it from the
facade's curated re-export list, so `html_outcomes` is named as
`transync_core::unit::html_outcomes` (or `transync_syntax::outcome::html_outcomes`)
by code that depends on an engine crate directly — which is what
`transync-openai`'s live-smoke test now does, via dev-dependencies. `htmlseg`
was never on the facade and is additionally narrowed: `balance_fragment` is
`pub(crate)`. The decision is again unaffected — this changes who may name
the machinery, not what it does.*

* **Engine** — `crates/transync-core/src/htmlseg.rs`: one pinned `lol_html`
  `Settings` shared by the extract pass and the splice pass (so the two passes
  can never disagree on tokenization), ordered scan → coalesce → decode →
  drop-whitespace-only, positional splice-back with an **identity skip** (a
  segment whose translation equals its decoded source is not replaced, so its
  bytes and entity forms pass through untouched), type-6/7-only blank-line
  collapse, plus the render-path `balance_fragment` and the `tag_inventory`
  used by the post-splice check.
* **Structure** — `BlockKind::Html { block_type }` (id code `html`, wire
  `html`), emitted by a new explicit `HtmlBlock` match arm; per-block
  `HtmlOutcome { Unit, PreservedZeroSegment, ExtractionFailed }` computed once
  per run so a block's alignment row, its counters, and its presentation cannot
  disagree.
* **Wire** — `InputMode::HtmlSegments`, payload = a JSON array of strings;
  `HtmlSegmentConstraints` carries the prompt-visible segment count and parent
  labels *and* the validator-only source bytes and block type (never
  serialized into the prompt).
* **Validation** — per-kind `check_html` (JSON shape, count, non-empty;
  retryable); a layer-3 post-splice check whose *engine faults* go to **direct
  fallback** (no retry burn, no cache put) because a splice failure after
  acceptance is not a model fault; `html` labels on both sides of the
  full-document reparse compare; and an **always-on** inline raw-HTML tag
  guard hoisted above the inline layer's policy gates.
* **Versions** — `VALIDATION_SCHEMA_VERSION` 1 → 2 (new payload semantics +
  prompt instructions + the always-on guard); alignment schema 1.1.0 → 1.2.0
  (additive `html` `block_kind`).

### Sequencing

The feature lands **inside the still-open 0.2.0 breaking window**, as the first
step of the owner-approved road to 0.2.0: **HTML-content translation (this
ADR) → OI-0028 Option B crate split + renderer rework → OI-0027 facade
curation → 0.2.0 release**. A `wasm32-unknown-unknown` canary run before
implementation confirmed that `lol_html` 2.9.0 and `htmlize` 1.1.0 **compile
for `wasm32-unknown-unknown`** (compile-only check — nothing was executed), so
adding them does not compromise the base-crate split that follows.

## Consequences

* Good, because the structural invariant is enforced by *construction* rather
  than by inspection: the model's output space contains no tags, so it cannot
  emit a malformed one. Every byte outside the collected text nodes is
  preserved verbatim, and the identity skip makes an echoed segment
  byte-identical to its source.
* Good, because the reader-value gap DR-2026-07 named is closed for the
  flagship shapes: a translated `<summary>`, HTML table cell, or hero caption
  renders as live content in both panes and participates in block-ID sync
  unchanged.
* Good, because no new terminal failure rung exists. Extraction failure,
  zero-segment content, model shape faults, and splice faults all land on
  existing honest rungs (placeholder presentation, `fallback_status`, warning
  rows), and the run continues.
* Good, because sync integrity is provable: render-side balancing makes it
  impossible for a fragment to consume a sibling wrapper, pinned by a
  Playwright assertion that no sync wrapper is nested inside another after
  mount.
* Bad, because **the `<details>` fold is not reproduced** (decision 5). An
  interleaved region renders a live translated `<summary>` above an
  always-visible body. This is a rendering-fidelity regression against GitHub,
  accepted deliberately over the alternative (render-side region re-merging,
  which would have to reason across block boundaries the IR keeps separate).
* Bad, because translation quality is capped at text-node granularity
  (decision 6): a sentence split by `<b>`…`</b>` is translated in pieces, and
  languages that reflow word order cannot move text across the boundary.
* Bad, because the inline tag guard has a **known false-reject** (accepted,
  pinned by test): a model that legally moves a TYPE-6 tag such as `<div>` to
  line-start reclassifies the paragraph remainder as an HTML block, so the
  inline tokens vanish on the translated side and the guard rejects. Verbatim
  retry usually recovers. (Type-7 tags — `<b>`, `<kbd>`, … — cannot interrupt a
  paragraph, comrak-verified and pinned.)
* Bad, because `out.md` gains an accepted **entity-form drift**: a genuinely
  translated segment re-escapes only `<`, `>`, `&`, so `&nbsp;` becomes a
  literal U+00A0 and `&copy;` becomes ©. Identity-skipped segments are exempt.
* Bad, because two new dependencies enter `transync-core`. Both were canaried
  for `wasm32-unknown-unknown` compilation before adoption, precisely because
  the crate split that follows depends on the base crate's dependency set
  staying WASM-clean.
* Neutral, because the cache cost of `VALIDATION_SCHEMA_VERSION` 2 is currently
  zero: the shipped cache is in-memory and hits are re-validated on every run
  (ADR-0015), so the bump only invalidates within-process reuse.

## Amendment (2026-08-09) — "non-empty" is "non-blank" (R0003-0042)

*Appended, not a rewrite.* The Decision Outcome's **Validation** bullet above
says `check_html` enforces "JSON shape, count, non-empty". The last word is now
**non-blank**: a translated segment consisting only of whitespace is rejected
the same way an empty one is, and for the same reason. Extraction drops every
source text node whose decoded form is all whitespace, so a segment the model
sees always carries visible text — a whitespace-only answer is content erasure
with no legitimate case behind it, and it used to pass layer 2 and every layer
after it (structure is unchanged, so the splice, the fragment reparse and the
full reparse all agree) and ship as `translated`. The rejection is retryable,
like the rest of the bullet. See DCR-0016's 2026-08-09 amendment.

## Related

- DCR-0016 — the implementation record for this decision (what changed, the schema lockstep drill, the known limits, migration)
- DCR-0013 — the reader-honesty placeholder posture this **partially supersedes**: the escaped `<pre data-skipped="html-block">` placeholder stays, but for html blocks it now presents *failure* only
- ADR-0012 — inline content is LLM-owned; inline constraints advisory. **Amended 2026-08-04 by this ADR:** raw inline-HTML tags become protected structure, and unlike the earlier destination / code-span narrowings the guard has no policy gate — it is a structural identity check, not a content constraint
- ADR-0009 — bounded verbatim retry + `RetryContext`: per-kind html rejections ride it unchanged; splice-engine faults deliberately bypass it
- ADR-0015 — cache poison / invalid-hit policy: why the `VALIDATION_SCHEMA_VERSION` bump is costless today
- ADR-0017 — batch-terminal abort semantics: unchanged; a giant HTML table is an ordinary oversize unit
- ADR-0004 — comrak as GFM parser: `lol_html` is a *segment extractor inside a block*, not a second document parser; block boundaries still come from comrak alone
- OI-0028 — the crate split this feature's dependencies were canaried against
- CLAUDE.md invariant 7 (amended by this ADR), invariant 2 (structure is application-owned)
- `docs/superpowers/specs/2026-08-03-html-content-translation-design.md` — the approved v2 design spec
