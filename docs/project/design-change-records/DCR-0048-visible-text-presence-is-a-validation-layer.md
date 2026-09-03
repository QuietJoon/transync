---
type: DCR
title: Visible-text presence becomes a per-unit validation layer, on both intakes
description: Every per-unit layer was structural and text-blind, so a provider that kept each kind's structural fact and erased its words shipped as `translated` on all six Markdown kinds and both HTML paths. One scoped comparison — the source had visible text and the translation has none — now owns that question for both intakes, and `per_kind`'s whitespace guard (wrong predicate, false premise) is subsumed.
tags: [change, project-control, DCR-0048]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-04T00:00:00Z
status: stable
---

# DCR-0048: Visible-text presence becomes a validation layer

- **Date:** 2026-09-04
- **Source:** ticket `c887bc`, filed from ti `490d97` wave 5's execution and rewritten on verification
- **Affected contracts:** `docs/architecture/contracts.md` §5 (the per-unit fault list and the provisional tier)
- **Affected DCRs:** DCR-0037 amended (its characterization of the `c887bc` finding was wrong on scope); DCR-0016's "no blank element" rule superseded by the scoped comparison

**Numbering note.** DCR-0039 remains **reserved** by the unrun HTML→HTML wave 7.
This is the next free number after DCR-0047, taken for the same reason
DCR-0040–0047 were: "the next free number" would collide with an unrun wave's
paperwork.

## What was wrong

Every per-unit validation layer compares **structure**. `check_heading`
compares the reparsed level; `check_table` compares `(cols, rows,
alignments)`; `check_code` compares the fence info string; `check_list` and
`check_blockquote` compare node-kind-label fingerprints; `check_html` compares
the JSON shape and the segment count; `fragment_reparse` compares kind labels;
`inline` compares destinations and code-span multisets; layer 6 compares block
counts, tag ledgers and gap bytes.

None of them asks whether the words survived. A provider that returns a
payload keeping each kind's structural fact and none of its text was therefore
**accepted** — `fallback_status: translated`, no warning, a blank for the
reader. That is architectural invariant 6 violated silently, and it was
reachable on **all six Markdown block kinds and both HTML paths**.

One arm anywhere in the engine looked at text — the html-segment guard in
`per_kind::check_html` — and it was wrong twice over:

1. **Wrong predicate.** `char::is_whitespace` is exactly the 25 Unicode
   `White_Space` codepoints. U+200B, U+200C, U+200D, U+2060, U+00AD and U+FEFF
   are not among them; neither is U+2800 BRAILLE PATTERN BLANK, which is `So`.
2. **False premise.** Its comment justified being unconditional with "source
   segments carry at least one non-whitespace char by construction — the
   extraction drop step". `transync_html::extract`'s drop test is the *same*
   `char::is_whitespace`, so a zero-width-only source segment is kept and
   sent, and its faithful echo was rejected — a false positive that spent the
   unit's whole retry budget reaching a fallback that changed nothing.

## The finding as first filed was wrong on scope, and the correction changed the fix

Ticket `c887bc` originally claimed an HTML **document** run was protected by
the layer-6 twin (`full_rescan_html`, DCR-0036) and only a raw-HTML island in
a Markdown run was exposed. Both halves were wrong:

- **The twin is no net at all** for U+200B / U+200C / U+200D / U+2060 /
  U+00AD. Rule T's invisible set is `White_Space ∪ {U+FEFF}` and the old
  guard's was `White_Space`; the disagreement is a **singleton, U+FEFF**, so
  every other zero-width character leaves the twin an unchanged document.
- **Even for U+FEFF the twin catches only a rule-T anonymous run.** An
  *element* block erased to U+FEFF keeps its ledger entry, gap bytes and
  boundary bookkeeping. The test that appeared to prove otherwise aimed at
  `doc.blocks[1]` — the run — not at `blocks[0]`, the `<p>`.
- **The Markdown path had no check at all**, on any kind. The only emptiness
  gate is `fragment_reparse`'s `translated_payload.trim().is_empty()`, and
  `str::trim` uses the same `White_Space` property.

The rule the measured evidence shows: **the more markup a kind carries, the
more the attacker must keep — and keeping it is always sufficient.** `##` is a
level-2 heading; blank cells preserve a table's geometry; a bare `rust` fence
preserves the info string; `- ` preserves list topology.

## The decision

**One scoped comparison, in one module, for both intakes.**

Reject when — and only when — **the source payload carried visible text and
the translation carries none.**

Three properties make this the right shape:

1. **A comparison, not a predicate.** The predicate-membership question the
   ticket flagged as an owner decision stops being one under the scope. A
   `<td>&#8203;</td>` layout shim, a ZWJ-only emoji fragment, an empty-bodied
   fence and a thematic break each have no visible source text either, so the
   check never fires on them. Over-inclusion in the invisibility test costs
   nothing; under-inclusion is a miss — which is the error direction a
   hand-written table should have.
2. **One home.** `validate::text_presence` holds THE definition of "renders
   nothing" and applies it on both intakes, so they cannot drift the way the
   old guard drifted from rule T. `per_kind::check_html` is now shape-only.
3. **Two intakes read two different things.** `HtmlSegments` compares the
   segment **strings**: `extract` decodes source segments and `splice` writes
   a translated one as `ContentType::Text`, which escapes `&`, so a
   provider's `"&nbsp;"` reaches the page as six visible characters —
   decoding here would invent an erasure the splice prevents. Every Markdown
   mode compares **comrak's decoded text**: nothing re-escapes on that path,
   and `&nbsp;` / `&#8203;` / `&#xFEFF;` are pure ASCII in the payload bytes,
   so a byte-level test would be bypassed.

### The invisibility test

`char::is_whitespace` (from `std`, so it tracks the compiler's Unicode
version) plus an explicit list of codepoints that render nothing and are not
`White_Space`: U+00AD, U+034F, U+061C, U+115F/U+1160, U+17B4/U+17B5,
U+180B–180F, U+200B–200F, U+202A–202E, U+2060–206F, U+2800, U+3164,
U+FE00–FE0F, U+FEFF, U+FFA0, U+FFF9–FFFB, U+1D173–1D17A, U+E0000–E0FFF.

Hand-written rather than a `unicode_categories`-style `Other_Format` test for
two reasons. It is not **sufficient** — U+2800 is `So` and the Hangul fillers
are `Lo`, and all render blank. And a vendored category table pins an **older
Unicode version than `std`'s**, so the two halves of the test would disagree
about anything assigned in between: the same drift, one layer down.

### Layer attribution

Reported under `ValidationLayer::PerKindShape`, not a new variant. The check
subsumes the old guard, which rejected under that layer, so report rows for
the case already covered stay stable — and a seventh layer name would be a
second opinion about which layer owns content preservation, for a rejection
the operator reads by its reason string.

## Rejected alternatives

- **Widen `check_html`'s predicate only.** Right about the predicate, wrong
  about the site: it leaves all six Markdown kinds undefended, because this
  is not one guard with a gap — it is one guard, and five kinds with nothing.
- **Give layer 6 a text-presence check.** Wrong layer. Layer 6 runs once per
  document after regeneration; a per-unit fault detected there cannot be
  retried, and per-block attribution would have to be reconstructed. An
  erasure is a unit's own fault and belongs where the retry budget applies.
- **Accept and record as a limitation.** Refuted by its own premises: the
  removed guard rejected `" "` on exactly this reasoning, and the Markdown
  path — never considered when the ticket was filed — is the larger half.
- **Widen `transync_html`'s `kept` predicate** so zero-width-only segments are
  never sent. This would make the old guard's premise true, but it flips any
  block whose only text is zero-width from `translated` to
  `HtmlOutcome::PreservedZeroSegment` — a wire-visible outcome change — and
  the scope makes it unnecessary. `kept` answers a different question ("is
  this segment worth sending to a translator?") and is deliberately left
  alone.

## Deliberately out of scope

**Visible garbage.** A payload of `**` renders two visible asterisks, so it
passes; so do `"..."` and `"TODO"`. Deciding which visible characters are
"really" text is a content decision and belongs to the model (architectural
invariant 2). Pinned by a test so the boundary stays a decision.

## Consequences

- **Two behaviour changes at the seam.** A whitespace-only translation of a
  source segment that had no visible text now **passes** where it used to be
  rejected (nothing was erased, so passing is correct). And an erasure that
  used to reach layer 6 now stops at layer 2.
- **Wave 5's layer-6 routing proof needed a new lever, and the swap sharpens
  the layering.** `pipeline::html_run_tests`' `DissolvesRun` reached layer 6
  by erasing a run's text to U+FEFF. Its fixture is now
  `HTML_SRC_INVISIBLE_RUN`, whose anonymous run is a single ZWSP: the source
  has no visible text to lose, so the erasure layer's scope correctly leaves
  it alone, and what changes at the rescan is purely structural — U+FEFF is
  stripped before rule T's test where U+200B is not, so the run block
  dissolves. **Layer 2 now owns "the words are gone"; layer 6 owns "the block
  is gone."** The two failures are disjoint, which is a better statement of
  the layering than the overlap the old lever depended on.
- **No wire change.** No new `ValidationLayer` variant, no
  `VALIDATION_SCHEMA_VERSION` bump, no alignment-schema move. `transync-syntax`
  and the JS are untouched, so the wasm32 gate and the browser suite are
  unaffected.

## Verification

- `validate::text_presence::tests` — 17 tests: the invisibility test's
  membership one codepoint at a time (including the assertion that each
  zero-width member is **not** `char::is_whitespace`, which is why the old
  guard missed it), the measured erasure table per kind, the entity bypass
  closed on the Markdown path, the entity **non**-erasure on the HTML path,
  the scope stated as the false positives it prevents, empty markup not
  counting as text, and totality at the malformed-input seams.
- `pipeline::text_erasure_run_tests` — 4 tests end to end through
  `run_pipeline`, the residual gap the analysis could not close.
  `an_erasing_translation_falls_back_on_every_kind` drives a nine-unit
  document deliberately free of links, images, code spans and inline tags —
  because with the shipped default profile any of them would mask the erasure
  through `check_inline`'s count, which is why the gap survived this long.
- **Staged red observed** with the wiring disabled: 3 of the 4 end-to-end
  tests failed and printed a fully blanked document, every row `translated`;
  the fourth (no-false-positive) passed both ways, as it must.
- Gates: workspace 46 binaries / **1291 passed / 0 failed** / exit 0;
  `cargo clippy --all-targets -- -D warnings` clean.
