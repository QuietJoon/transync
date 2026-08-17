---
type: DCR
title: Inline-protection validation layer — link/image destinations and pledged code spans
description: A sixth validation layer compares source-vs-translated inline destination sequences (and, when pledged, code-span multisets); ADR-0012 amended — destinations are protected structure, inline text stays LLM-owned.
tags: [change, project-control, DCR-0010]
status: deprecated
---

# DCR-0010: Inline-protection validation layer

- **Date:** 2026-07-13
- **Source:** External review 2026-07 (EXT-2026-07 P1-5)
- **Affected ADRs:** docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md (amended in place — destinations and pledged code spans reclassified as protected structure)

## What Changed

New `ValidationLayer::Inline` (wire `"inline"`), implemented in
`crates/transync-core/src/validate/inline.rs`, running after fragment
reparse and before acceptance:

- **Destinations** (links, images, autolinks): compared as an ordered,
  kind-tagged `(Link|Image, url)` sequence extracted by a symmetric comrak
  walk of the source vs translated payload. Enforced unless the profile
  sets `preserve_urls = false` (the shipped localize-permitted policy);
  `None`/unset enforces — secure by default. Autolink ↔ explicit-link form
  changes with the same destination pass; any URL change, added link, or
  dropped link rejects.
- **Code spans**: compared as an unordered multiset (spans legitimately
  move with target-language word order), enforced only when the profile
  pledges `preserve_code_identifiers = true`.
- Mismatches are retryable validation rejections riding the standard
  verbatim-retry → fallback-source path; there is **no repair pass** — the
  pipeline never rewrites provider output.
- User prompts gain policy-gated instruction sentences mirroring the
  enforced gates (a model is never punished for an untold rule).

## Why

A swapped URL is invisible in the rendered pane, breaks navigation
silently, and DOMPurify does not defend against a phishing destination.
ADR-0012's "bounded by outer-structure checks" consequence was wrong for
exactly this surface. Link *text*, alt text, titles, and emphasis remain
LLM-owned — the amendment narrows, it does not revert.

## Affected Areas

- `crates/transync-core/src/validate/inline.rs` (new), `validate.rs`
- `crates/transync-openai/src/client.rs` (`build_user_prompt`)
- `docs/architecture/contracts.md` (§2 constraints effect, §5 note),
  `README.md` validation bullet, ADR-0012 amendment

## Migration / Follow-up

- Documented v1 gap: reference-style link labels are not protected
  (fragment parses resolve no refmap — symmetric no-op). Future option:
  broken-link-callback refmap seeding; verify the comrak API first.
- No wire/schema/profile-key changes; the two existing booleans gained
  teeth. Profiles that relied on silent destination rewriting under
  `preserve_urls` unset must now set it to `false` explicitly.
