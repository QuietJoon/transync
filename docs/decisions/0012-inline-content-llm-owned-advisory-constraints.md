---
type: ADR
title: Inline Markdown content is LLM-owned; inline-structure constraints are advisory
description: Validation enforces outer block structure plus, since the 2026-07 amendment, link/image destinations and profile-pledged inline code spans, and since the 2026-08-04 amendment, raw inline-HTML tag identity (always on, no policy gate); other inline markup belongs to the model.
tags: [decision, ADR-0012]
status: active
---

# ADR: Inline content is LLM-owned; inline-structure constraints are advisory

## Context and Problem Statement

Found in Review 0001 (Issue R0001-0046, Severity: Medium) (review archived and removed), re-raised in
Review 0008 (Issues R0008-0011, Severity: High; R0008-0016, Severity:
Medium). Location: `crates/transync-core/src/llm.rs`
(`BlockConstraints.forbid_block_breaks_in_inline`),
`crates/transync-core/src/validate/per_kind.rs` (`check`, paragraph arm),
`crates/transync-openai/src/client.rs` (`hint_constraints`).

Three findings across two reviews flagged the same gap from different
angles: paragraphs/headings carry no inline-structure fingerprint (a
provider may alter link destinations, inline code, emphasis boundaries),
and the `forbid_block_breaks_in_inline` table-cell constraint is sent to
the model as a hint but never checked on the way back.

## Decision Drivers

* Architectural invariant 2 (CLAUDE.md): **the LLM owns content
  decisions; the application owns structure.** Translation legitimately
  reshapes inline markup — emphasis boundaries move across word-order
  changes, link text is translated, inline code may be preserved or
  localized per profile policy. A strict inline fingerprint would reject
  correct translations wholesale.
* The enforced surface is deliberately the *outer* structural contract:
  block kind, heading level, table shape, list topology, fence info —
  the things scroll-sync and regeneration depend on.
* The advisory hints cost a few prompt tokens and measurably reduce
  retries; enforcing them is a separate, large design problem
  (language-aware inline diffing) with a poor cost/benefit at MVP scope.

## Considered Options

1. Enforce an inline-structure fingerprint (link counts/destinations,
   inline-code spans, emphasis counts) per block kind.
2. Parse table cells and reject block-level content inside them
   (enforce `forbid_block_breaks_in_inline`).
3. Keep inline content LLM-owned; send constraints as advisory hints
   only; document the hook for a future opt-in validator.

## Decision Outcome

REJECT (all three findings): option 3. Inline validation contradicts
invariant 2 for translatable text, and the fragment/full reparse layers
already catch inline damage severe enough to break block structure.
`forbid_block_breaks_in_inline` remains a documented advisory hook
(`R0001-0046` trace comments at the constraint and hint sites).

Status: No change required. Raised three times across two independent
reviews. This decision rests on the assumption that scroll-sync and
regeneration depend only on the *outer* block structure, never on inline
markup. Revisit trigger: if the sync/regen contract ever starts depending on
inline structure (e.g. an anchor scheme keyed to inline spans), the
inline-fingerprint option is back on the table.

### Implementation

None. The hint plumbing (R0006-0036) ships; the validator hook stays
open for a future opt-in profile policy.

## Consequences

* Good, because correct translations are never rejected for legitimate
  inline reshaping, and the model still receives the structural hints.
* Bad, because a misbehaving provider can alter link destinations or
  inject inline content within a block without tripping validation —
  bounded by the outer-structure checks and DOMPurify at render time.

## Amendment (2026-07, external review P1-5): destinations and pledged code spans are protected structure

The rejection above treated *all* inline markup as LLM-owned. This
amendment narrows it: link and image **destinations**, and — when the
profile pledges `preserve_code_identifiers = true` — inline **code
spans**, are reclassified as protected structure, enforced by the
`Inline` validation layer (`validate/inline.rs`) after fragment
reparse. Destinations are operational metadata, not prose: a swapped
URL is invisible in the rendered pane, navigation breaks silently, and
DOMPurify does not defend against a phishing destination. The original
"bounded by outer-structure checks" consequence was wrong for exactly
this surface, and the check is cheap — both sides reuse the comrak
parse the validators already run.

Policy gates (profile `[constraints]`):

* `preserve_urls` unset or `true` → destination identity enforced
  (ordered, kind-tagged link/image sequence; autolinks included,
  autolink ↔ explicit-link form changes with the same destination
  pass). `false` → the shipped "URLs may be localized" policy; no
  destination check.
* `preserve_code_identifiers = true` → code-span identity enforced as
  an unordered multiset (spans legitimately move with target-language
  word order). Unset or `false` → advisory only, as before.

Mismatches — including added/dropped links — are retryable validation
rejections (`ValidationLayer::Inline`) following the standard
verbatim-retry → fallback-source path (ADR-0009, invariant 6). There
is no repair pass: the pipeline never rewrites provider output.

What did **not** change: inline TEXT remains LLM-owned. Link text,
image alt text, link titles, emphasis boundaries, and table-cell prose
are never fingerprinted; `forbid_block_breaks_in_inline` remains an
advisory hint; reference-link labels are not protected (fragment
parses resolve no refmap — a symmetric no-op, documented gap). *(This
reference-label gap was closed 2026-07-27 — see the DR-2026-07 amendment
below.)* The original rejection rationale above stands as history for those
surfaces.

## Amendment (2026-07-27, DR-2026-07): reference destinations protected; two residuals recorded

The 2026-07 design review (design-review wave DR-2026-07) revisited the inline
surface and produced one narrowing plus two explicitly-accepted residuals.

**Reference-style destinations are now protected (DCR-0013).** The previous
amendment's documented gap — "reference-link labels are not protected
(fragment parses resolve no refmap)" — is **closed**. A document-level
link-reference-definition pool is now recovered from the inter-block gaps at
parse time (`parser/refdefs.rs` → `Document.ref_defs`) and appended to each
fragment before both the render reparse and the inline-inventory parse. A
`[text][ref]` link therefore resolves to a real destination on both sides, so
a provider that mistranslates a reference **label** changes the resolved
destination inventory and trips the same `ValidationLayer::Inline` rejection as
an explicit-link destination change. Reference-link **text** remains LLM-owned,
exactly like explicit-link text. See DCR-0013 for the mechanism, including the
narrow render-only duplicate-label ordering limitation (validation stays
symmetric, so no false accept/reject).

**Accepted residuals (within the LLM-owns-content doctrine).** Two inline-level
exposures are acknowledged and accepted, not fixed — both follow directly from
invariant 2:

1. **Same-kind payload transposition is structurally undetectable.** If a
   provider swaps the translated payloads of two units of the *same* block kind
   (paragraph A's translation returned under paragraph B's id and vice-versa),
   every structural and inline check still passes on each unit in isolation —
   the ID↔content binding ultimately rests on **provider honesty**, not on any
   validator. The MUST-preserve-`unit_id` contract (§1) and per-unit validation
   catch id corruption and shape drift, but not an honest-looking same-kind
   content swap.
2. **Math / LaTeX content is unprotected.** Inline and block math is opaque
   paragraph text to the pipeline (math extensions are off); it is neither
   fingerprinted nor destination-checked, so a provider may alter a formula as
   freely as prose. This is the same posture as inline text generally.

Both residuals sit inside the "LLM owns content decisions" doctrine and are
recorded for honesty, not slated for a fix at this scope.

## Amendment (2026-08-04, ADR-0018 / DCR-0016): raw inline-HTML tags are protected structure, unconditionally

Raw inline HTML (`<kbd>`, `<br>`, `<sup>`, `<b>`, …) was covered by "other
inline markup belongs to the model". It no longer is. The ordered sequence of
comrak `HtmlInline` literals in a paragraph-family payload must survive
**byte-verbatim**, in order; only the text around the tags is translatable. A
mismatch is a retryable `ValidationLayer::Inline` rejection on the standard
verbatim-retry → fallback path, exactly like a destination change.

Two things distinguish this from the amendments above:

* **No policy gate.** Unlike `preserve_urls` / `preserve_code_identifiers`, the
  guard is **always on** and is deliberately *hoisted above* `check_inline`'s
  policy early-return, so a profile with `preserve_urls = false` and no code
  pledge cannot switch it off. The rationale is that a tag is not a content
  decision the profile could reasonably delegate: a model has no legitimate
  reason to alter markup it was told to reproduce, and the HTML-block feature
  (ADR-0018) made raw markup a first-class structural concern rather than an
  incidental one. The user prompt carries the matching instruction on **every**
  dispatch, and both halves live in `transync::llm::prompt` so they cannot
  drift apart.
* **An accepted known false-reject.** A model that legally moves a **type-6**
  tag such as `<div>` to line-start makes the fragment reparse reclassify the
  paragraph remainder as an HTML block, so the `HtmlInline` tokens vanish on the
  translated side only and the guard rejects. Verbatim retry usually recovers;
  the behavior is pinned by test. Type-7 tags (`<b>`, `<kbd>`, …) cannot
  interrupt a paragraph (comrak-verified, also pinned), so the exposure is
  confined to the type-6 set. Model normalization of `<br>` ⇄ `<br/>` is a live
  rejection source to watch — the comparison is byte-verbatim by design.

What still did **not** change: inline **text**, including the text between and
around tags, remains LLM-owned; attribute text inside inline tags is preserved
only as a side effect of the tag being reproduced verbatim, and is never
translated (ADR-0018). Both residuals recorded in the DR-2026-07 amendment
above stand.

