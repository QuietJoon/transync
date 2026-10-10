---
type: DCR
title: Adjacent sections coalesce into one batch
description: DCR-0027 confined every batch to one section and left the relaxation open as OQ-A, recommending an opt-in knob once request-count pain was observed. This record settles OQ-A as option (b), cohort-restricted whole-section coalescing, and ships it on by default — with a second admission term DCR-0027 did not name, because the assembled instruction is batch-scoped too.
tags: [batching, performance, DCR-0057, DCR-0027, DCR-0026, SCN-10]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-16T14:02:00Z
status: stable
---

# DCR-0057: Adjacent sections coalesce into one batch

- **Date:** 2026-09-16.
- **Source:** owner request — "if you split by section strictly, I want to pack
  multiple sections to a single batch if they are in the budget."
- **Settles:** DCR-0027's open question OQ-A, as option **(b)**. DCR-0027 is a
  Record and is **not** edited; this record is its successor.
- **Supersedes:** `docs/backlog.md`'s `section-batch-coalescing` entry, which
  carried an owner deferral to 2027. The owner has moved it forward.
- **Behaviour changed:** yes, by default. A run packs fewer, larger batches.
  No output byte changes; no cache entry is orphaned. See *Identity* below.

**Numbering note.** Next free number after DCR-0056.

## What DCR-0027 left open

DCR-0027 P2 confined every batch to exactly one section, and its OQ-A named the
relaxation without taking it:

> **(b) Cohort-restricted coalescing.** Adjacent **whole** sections may share a
> batch when their effective glossaries (G5) are identical […]
> **Recommendation: ship (a), ask the owner about (b) as an opt-in knob** (e.g.
> `[batching].coalesce_sections`) once real request-count pain is observed.

The pain is observable and was measured before this record was written. The
cost of (a) is one provider request per heading, floored — no token budget can
buy it back, because the partition runs before the packer. On a 30-section
fixture that is 30 requests for ~120 units the budget would have packed into a
fraction of that; on this repository's own `docs/architecture/contracts.md` the
confined count is roughly 62 batches where the budgets alone would ask for 37.
A heading is a cheap thing to write and an expensive thing to pay a round-trip
for.

## The decision

Adjacent **whole** sections share a batch when they fit and when they are
*admissible* with one another. Shipped **on**; `[batching].coalesce_sections =
false` restores DCR-0027 P2 exactly.

On by default rather than opt-in — the deviation from OQ-A's recommendation —
because the relaxation is invisible to every consumer of the output. It changes
how many requests a document costs and nothing else: not a rendered byte, not
an alignment row, not a cache key (see *Identity*). A knob defaulting to the
more expensive shape would mean most runs keep paying for a property that no
longer buys them anything.

## Admission: two terms, not one

OQ-A named one term. There are two, and the second is why this record does not
simply implement the sentence OQ-A wrote.

A batch is **one provider request**. It carries one compiled system prompt and
one assembled instruction, so two sections may share it only when they agree
about both.

1. **The effective glossary cohort** — OQ-A's term. The compiled system prompt
   follows the section's effective glossary (DCR-0027 G5), and two sections
   resolving to the same cohort key are handed literally the same memoized
   `ProfileMetadata`, so their `prompt_body` is byte-identical.

2. **`DocumentFacts`** — the term OQ-A did not name. The instruction's
   membership-driven clauses (the raw-HTML segment contract, the DCR-0026
   row-window contract, the HTML-document clause) are assembled per batch:
   `llm::prompt::build_user_prompt` asks `InstructionVariant::for_batch`, which
   reads `DocumentFacts::of(&batch.units)` — the batch's **own** membership.
   Merge a prose section with one holding a raw-HTML unit and the prose units'
   request grows a clause it never carried.

Contracts §2's rule that those clauses are reserved on a *document-level upper
bound* is about the packer's **budget**, deliberately conservative so a retry
round prices what the first round did. It is not a claim about the bytes sent.
Reading it as one is the mistake this section exists to prevent.

Both terms are equality tests over consecutive sections, so packing walks
maximal runs of sections agreeing on both and packs each run.

## Whole sections, never part of one

The unit of admission is a whole section. A batch is one of exactly two shapes:

- *N consecutive whole sections* whose combined estimates fit all three caps, or
- *a fragment of one oversize section*, split by the same greedy packer
  DCR-0027 P4 already used.

This is strictly weaker than concatenating a run and packing it flat, and the
difference is deliberate. Under the flat reading, a small section sitting
immediately before an oversize one has its batch topped up by the big section's
leading units — a section boundary inside a fragment, which is the one shape
both DCR-0027 and this record refuse. An oversize section therefore coalesces
in neither direction: it cannot join the batch before it, and its trailing
fragment cannot host the section after it.

P4 is otherwise untouched. A section too big to be one batch splits inside
itself exactly as before.

## Identity: nothing re-keys

A `CacheKey` has exactly two batch-scoped axes, and the admission rule pins
both:

| Axis | Why coalescing cannot move it |
|---|---|
| `CohortDigest` | Hashes `prompt_body` + `glossary`; term 1 makes them byte-identical across the run |
| `InstructionDigest` | Derived from `DocumentFacts`; term 2 makes them equal across the run |

Every other field of the key is per-unit. So a coalesced batch's request is
byte-identical to the one its sections would have received alone, and no cache
entry written by a pre-DCR-0057 run is orphaned. `pipeline::run_level_tests::
coalescing_moves_no_cache_key` pins this directly: it digests every unit under
both settings, over a fixture that deliberately carries a raw-HTML block so
that dropping term 2 would fail it.

No schema version moves. `ALIGNMENT_SCHEMA_VERSION`,
`VALIDATION_SCHEMA_VERSION`, `VALIDATION_REPORT_SCHEMA_VERSION` and
`CACHE_DISK_FORMAT_VERSION` are all untouched — batch ids reach neither the
alignment map nor the validation report.

## What does change

- **Batch count**, downward, on any document with more than one admissible
  section. This is the point.
- **`ValidationReport.provider_retries` and `batch_schema_faults` ceilings**,
  which contracts §5 documents as multiples of the input batch count. Fewer
  batches means a tighter run-wide retry allowance. The per-batch budgets
  (`max_per_batch_schema_retries`, `max_per_batch_provider_retries`) are
  unchanged; what shrinks is the number of batches each is charged against.
- **Retry blast radius per envelope fault.** A provider that drops or
  duplicates rows makes every implicated unit an offender, and a larger batch
  has more of them. Per-unit *content* faults are unaffected: since OI-0031 an
  innocent batch-mate's payload is validated and accepted in the round it
  arrived, never re-sent.
- **Concurrency headroom.** `max_concurrent_batches` defaults to 6; a document
  that coalesces below six batches no longer fills the dispatch slots. On a
  document small enough for that, the round-trip count it saves is worth more
  than the parallelism it gives up — but a caller tuning for wall clock on a
  short, heading-dense document is the one case where `false` is the better
  setting.

## Rejected alternatives

- **Opt-in, default off** (OQ-A's own recommendation). Rejected: see *The
  decision*. The relaxation is output-invisible, so a default that keeps paying
  for confinement buys nothing.
- **Cohort equality alone**, as OQ-A's sentence specifies. Rejected: it moves
  `instruction_hash` for any document mixing raw-HTML or row-window sections
  with prose, silently invalidating those units' cache entries and changing the
  bytes the model reads.
- **Concatenate a run and pack it flat.** Rejected: puts a section boundary
  inside a fragment of an oversize section. See *Whole sections*.
- **Whole-section coalescing across cohorts, re-compiling a merged prompt.**
  Rejected outright: a merged glossary is a different prompt than either
  section would have read alone, which is the exactness DCR-0027 G5 exists to
  provide.

## Gates

`crates/transync-core/src/unit.rs`'s `section_coherent_packing_tests` carries
both shapes: DCR-0027's P1–P4 tests now take a `coalesce_sections = false`
profile and assert what they always asserted, and four new tests cover the
default — sections share a batch when they fit, a coalesced batch never ends
mid-section, an oversize section coalesces in neither direction, and the unit
set is unchanged. `crates/transync/tests/scenarios/scn_10_long_document.rs`
carries the scenario pair. `pipeline::run_level_tests::coalescing_moves_no_cache_key`
is the identity gate.
