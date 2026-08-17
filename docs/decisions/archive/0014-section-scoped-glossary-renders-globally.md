---
type: ADR
title: Reserved glossary scope "conditional-on-section" is rejected until section-aware batching lands
description: Superseded 2026-08-09 by DCR-0027 — this is the record of the rejection era, not current behavior. It recorded that load_profile (and, from 2026-08-05, the translate boundary) rejected glossary entries with scope "conditional-on-section" (ProfileError::Unsupported) instead of rendering them globally with an advisory suffix, as a time-boxed deferral until section-aware batching made per-section filtering cheap. That trigger fired: SL-106..SL-109 shipped section-coherent batching and per-cohort prompts, and removed the rejection helper, ProfileMetadata::ensure_supported and ProfileError::Unsupported outright. Amended 2026-07-13 (external review P0-3); superseded 2026-08-09 (DCR-0027).
tags: [decision, ADR-0014]
status: deprecated
superseded_by: project/design-change-records/DCR-0027-section-coherent-batching-and-glossary-section-scope.md
---

# ADR: Reserved glossary scope `conditional-on-section` is rejected until filtering exists

> **Amended 2026-07-13 (external review P0-3).** The original decision —
> render `conditional-on-section` entries globally with a
> `(section-scoped — currently advisory only)` suffix — was reversed.
> Globally applying a *scoped* glossary is semantically misleading: a
> term meant for one section silently steers every section, and the
> advisory suffix only papers over that on models that ignore it. The
> scope is now handled as a **time-boxed deferral**: `load_profile`
> rejects it with a clear not-implemented error
> (`ProfileError::Unsupported`), and it is re-admitted the moment
> section-aware batching makes per-section filtering cheap (see the
> revisit trigger below). The `GlossaryScope::ConditionalOnSection` enum
> variant is retained for wire compatibility and future use. The
> original rationale is preserved below under *History*.

## Context and Problem Statement

Found in Review 0002 (Issue R0002-0019, Severity: High) (review archived and removed), re-raised in
Review 0008 (Issue R0008-0026, Severity: Low). Location:
`crates/transync-core/src/profile.rs` (`format_glossary`, `load_profile`),
`crates/transync-core/src/llm.rs` (`GlossaryScope`).

**Numbering caveat:** live code comments cite this decision as
"R0002-0017" (e.g. `profile.rs` at the former scope-suffix site) — the archived
review file's own numbering for the same finding was R0002-0019; an
ID skew between the review draft and the applied-fix comments. Both
refer to this decision. The 2026-07-13 amendment is tagged
`EXT-2026-07 P0-3` in code.

The profile schema models `scope = "conditional-on-section"`, but no
part of the pipeline filters glossary entries by section — the prompt
renderer appends every glossary entry to every batch's system prompt. A
term intended for one section can therefore influence translations in
every section.

## Decision Drivers

* Correct per-section filtering needs a reliable unit→section mapping at
  *batch assembly* time and a per-batch prompt variant — batches can
  span sections, so filtering also forces section-aligned batching or
  per-unit glossaries. That is real design work, not a bug fix. (The
  batcher today is sequential token-budget packing with **no** section
  logic — see `batch.rs`.)
* Glossaries are advisory prompt text end to end (they are never
  validated on the way back). The original decision leaned on this to
  justify shipping a global render with an advisory suffix; the
  amendment weighs it the other way — because nothing downstream can
  correct a mis-scoped term, silently widening its scope is the most
  misleading failure mode, not the most tolerable one.
* Glossary content participates in the cache key via the glossary hash,
  so any rendering is at least deterministic and cache-consistent — but
  determinism does not make a semantically wrong scope correct.

## Considered Options

1. Filter entries per unit/batch section (requires section-aligned
   batching or per-unit prompt assembly).
2. Reject the `conditional-on-section` scope value at load time until
   filtering is implemented.
3. Render globally with an explicit advisory-scope suffix; defer
   filtering until section-aware batching exists.

## Decision Outcome

**Amended (2026-07-13, external review P0-3): option 2 — reject until
implemented, as a time-boxed deferral.**

`load_profile` returns `ProfileError::Unsupported` for any glossary
entry whose scope resolves to `GlossaryScope::ConditionalOnSection`
(wire values `"section"` / `"conditional_on_section"` — the hyphenated
spelling is the variant's conceptual name, not a value serde accepts):

> glossary scope "section" (alias "conditional_on_section") is reserved
> and not implemented; scope filtering lands with section-aware
> batching — use scope = "global", or remove the scope key ("global" is
> the default)

(Message corrected 2026-08-04: the original hint said `use
"global-across-document"`, which is neither a serde name nor an alias —
an operator following it hit a second, different error. Found by
resp-translator's profile-diagnostics review; a regression test now
pins that the hint only names values that parse.)

Rejecting at the loader (rather than rendering something misleading)
keeps the contract honest: a profile that asks for behavior the system
does not have fails loudly with an actionable message, instead of
quietly translating as if the scope were global.

**Revisit trigger (the deferral is time-boxed):** when section-aware
batching lands — i.e. there is a reliable unit→section mapping at
batch-assembly time — per-section filtering becomes cheap. At that
point: implement filtering, drop the loader rejection, and re-admit the
`conditional-on-section` scope. Until then the scope stays rejected.
This ADR is the record to reopen when that batching work starts.

### History — original decision (superseded 2026-07-13)

REJECT (both findings): option 3. Removing/blocking the scope value was
judged, at the time, to break existing profile files for a cosmetic
gain; option 1 was deferred as a feature tracked with post-MVP batching.
The advisory suffix `(section-scoped — currently advisory only)` shipped
in commit `bc47dd3`, appended by `format_glossary` for
`ConditionalOnSection` entries, with a code comment tracing the
decision. The original status line read: "No change required. Raised
twice — settled until section-aware batching lands, at which point
filtering becomes cheap and this ADR should be revisited." The external
review (P0-3) took up exactly that revisit and concluded the global
render was misleading enough to reject in the interim.

### Implementation

`load_profile` rejects `ConditionalOnSection` entries with
`ProfileError::Unsupported`. The advisory-suffix branch in
`format_glossary` was removed — every entry that now reaches the
renderer is `global-across-document`. The `GlossaryScope` enum in
`llm.rs` is unchanged (variant retained for wire compatibility).

## Consequences

* Good, because a profile can no longer silently mistranslate: a
  section-scoped term never bleeds into other sections, because such a
  profile does not load at all.
* Good, because the error names the reserved scope and the supported
  alternative, so the fix (`scope = "global"`, or deleting the scope
  key) is obvious.
* Bad, because profiles that previously loaded with a
  `conditional-on-section` entry (relying on the advisory suffix) now
  fail to load and must be edited. This is an accepted, deliberate
  break: the prior behavior was misleading, and the scope was always
  documented as advisory-only / post-MVP.

## Note — 2026-08-05: the loader was never the only entry path

Backlog item `profile-section-scope-programmatic-bypass`, closing review
finding R0001-0006 (High). The *Implementation* section above names
`load_profile` as the enforcement point; that was accurate about the
loader and wrong about the invariant. `ProfileMetadata` derives
`Deserialize` and its fields are public, so a caller who builds the
profile in code — `serde_json::from_str` over a JSON config, or
`default_profile()` followed by `glossary.push(...)` — never crosses the
loader. Such a profile reached `format_glossary` and had its
section-scoped entry rendered into every batch's system prompt: the exact
global application this decision reversed, reachable through a door the
decision never looked at. (`#[non_exhaustive]`, added in the OI-0027
wave, blocks the external struct literal but not either of those paths.)

The rejection is now a shared helper,
`profile::reject_reserved_glossary_scope`, called from **two** places:
`load_profile` as before, and `pipeline::run_pipeline` — the single
funnel behind `translate` / `translate_with_cache` — on
`TranslateOptions::profile`, before the parse and before any provider
call (including the auto-glossary preflight). Both gates raise the same
`ProfileError::Unsupported` with the same message, surfacing at the
translate boundary as `TransyncError::Profile` (stable code
`profile_failed`). Nothing about the decision changes: the scope stays
rejected, the message stays as corrected on 2026-08-04, and the revisit
trigger above is unchanged — when filtering lands, dropping the rejection
means dropping one helper rather than hunting its call sites.

## Note — 2026-08-09: the revisit trigger fired; re-admission designed (DCR-0027)

Ticket `43cfb4` (owner-commissioned 2026-08-06) builds section-aware
batching, which is exactly the condition the revisit trigger names: with
section-coherent batches (every batch confined to one heading-delimited
section) there is a reliable unit→section mapping at batch-assembly time,
and per-section filtering becomes cheap. **DCR-0027** is the design record
that re-admits the scope; the rejection stays in force until its SL-108
slice lands, and is then retired as *removal*, not as a disabled gate:
`reject_reserved_glossary_scope`, `ProfileMetadata::ensure_supported`, and
the `ProfileError::Unsupported` variant (this rejection is its only
constructor) all go, `unit::build_batches` becomes infallible, and the
three gates — the loader, the translate boundary, and the batching door
(ticket `0ed6eb`) — retire together, per the backlog entry's own condition.

What re-admission means, decided there and summarized here:

- `GlossaryEntry` gains a `sections = ["…"]` selector list, required
  non-empty for `scope = "section"`. An entry applies to a section iff any
  heading in the section's heading stack (levels ignored, opening heading
  included) matches any selector, trimmed and case-folded — so a term
  scoped to a section inherits into its subsections by stack containment.
- Filtering, never annotation: each batch's system prompt renders only the
  entries applicable to that batch's section; scope and selectors are never
  rendered as prompt text. The advisory-suffix option this ADR's amendment
  rejected stays rejected.
- The claimed-once rule relaxes only where claims cannot meet: a global and
  a section-scoped entry may share a source term (the override pattern —
  the section entry wins inside its sections), while same-term section
  entries must keep disjoint selector sets. The auto-glossary merge's
  static-wins rule sharpens accordingly: an extracted global term is
  dropped only on a static *global* claim.
- Cache identity follows the prompt bytes, as ADR-0020/§5a require: the
  profile-prompt and glossary hashes become batch-scoped, derived from what
  each batch actually carries. A run with no section-scoped entries keeps
  byte-identical prompts and identical cache keys.

The *History* above, the 2026-08-04 message correction, and the 2026-08-05
programmatic-bypass note remain accurate records of the rejection era.

**Landed 2026-08-09.** SL-106..SL-109 shipped the above as designed: the
selector, the section-coherent packer, per-cohort prompts, the batch-scoped
cache axes, and the removal of `reject_reserved_glossary_scope`,
`ProfileMetadata::ensure_supported`, `ProfileError::Unsupported` and
`unit::build_batches`'s fallibility — all three gates in the one slice.
`contracts.md` §2 / §5 / §5a and the Developer Guide carry the shipped
semantics; the rejection era is over and this ADR is a record of it.

## Note — 2026-08-12: the selector identity gains NFC normalization

The summary above says a heading "matches any selector, trimmed and
case-folded". That is now one step longer: **trimmed, case-folded and
normalized to Unicode NFC** (Review 0004 `R0004-0080`, ticket `ad8b54e4`).
Case folding alone is per-scalar and leaves composition unsettled, so an
accented heading written as base + combining mark never matched the
precomposed selector naming it — the entry applied nowhere, silently.

Nothing this ADR decided moves; the identity the decision is expressed in
terms of got more exact. DCR-0027's amendment of the same date carries the
mechanism and the one-time cache consequence for profiles it actually moves.
