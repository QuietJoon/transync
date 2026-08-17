---
type: ADR
title: Batch-terminal provider failures abort the whole run
description: Batch-terminal provider failures (output truncation / incomplete, model refusal, malformed JSON, exhausted transport retries) keep whole-run abort semantics — no per-batch fallback rung and no policy flag. Both were considered and rejected by the owner (2026-07-24, DR-2026-07); mitigation is prevention-side (DCR-0012).
tags: [decision, ADR-0017]
status: active
---

# ADR: Batch-terminal provider failures abort the whole run

> **Status: accepted.** Owner-settled 2026-07-24 during the 2026-07
> design/architecture fitness review (design-review wave **DR-2026-07**);
> the mitigation wave shipped 2026-07-27. This ADR records a decision to
> *keep* the shipped behavior against two proposed alternatives, both
> explicitly rejected.

## Context and Problem Statement

The DR-2026-07 review re-examined what happens when a single batch fails in
a way that no retry can recover: the provider truncates output
(`incomplete (reason: max_output_tokens)`, R0008-0036), refuses the request,
returns JSON the client cannot parse, or exhausts its bounded transport
retries. Today `run_pipeline` (`crates/transync-core/src/pipeline.rs`)
returns `Err` after all batches settle — the lowest-index error is surfaced —
and the whole run aborts with no output written (contracts.md §5:
"a `Translator` error is **not** a fallback path").

The review asked whether a batch-terminal failure should instead degrade
*that batch* to source content (a per-batch fallback rung, analogous to the
per-unit `fallback_source` path), optionally behind a policy flag so callers
could opt into "translate what you can, pass through the rest."

This is distinct from — and must not be confused with — the *validation*
fallback path (ADR-0009, invariant 6), where a unit that fails structural
validation after its bounded retries is emitted as `fallback_source`. That
path stays. This ADR is only about *provider/transport-terminal* batch
failures.

## Decision Drivers

* **All-or-nothing artifact staging (DCR-0006 / DCR-0011).** The CLI already
  commits its outputs as one staged fileset commit precisely so a consumer
  never has to reason about a half-written result set. A per-batch fallback
  rung would reintroduce exactly the ambiguity staging removed: a
  "successful" (exit 0) artifact silently carrying source-language blocks
  wherever a batch died.
* **Loud failure beats a quietly-degraded document.** A translated document
  with an untranslated island is worse than no document: the island is not
  visually distinguished in raw Markdown, so a downstream reader can ship it
  without noticing. An abort with a diagnostic is self-correcting; a silent
  partial is not.
* **The failure is preventable at packing time.** The dominant
  batch-terminal cause under the default configuration is output-token
  truncation, and that is an estimation problem the packer can address
  *before* dispatch (DCR-0012) rather than a runtime condition that must be
  caught and absorbed.
* **A policy flag multiplies the contract.** "Sometimes abort, sometimes
  degrade" doubles the states every consumer of `TranslationOutput` and the
  alignment map must handle, for a mode whose only honest use is "I accept a
  silently mixed-language document" — which the owner does not want to
  sanction as a supported shape.

## Considered Options

1. **Keep whole-run abort (shipped).** A batch-terminal error returns `Err`;
   no output is written.
2. **Add a per-batch fallback rung.** On a batch-terminal error, emit that
   batch's units as `fallback_source` and continue, marking `fallback_status`
   in the alignment map.
3. **Add a policy flag** gating option 1 vs option 2 per run.

## Decision Outcome

**ACCEPTED: option 1.** Batch-terminal provider failures keep whole-run
abort semantics. Options 2 and 3 are **both rejected by the owner**
(2026-07-24). There is deliberately no per-batch fallback rung and no policy
flag.

The decision is paired with a prevention-side mitigation wave (DCR-0012):
output-aware batch packing shrinks batches so a full batch cannot outgrow the
output ceiling in the first place; an at-risk preflight names any single
oversize unit before dispatch; and five CLI batching flags let an operator
raise the ceiling, lower the expansion factor, or split the input without
authoring a profile TOML. Abort stays the terminal behavior; prevention makes
the terminal case rare and self-diagnosing.

### Implementation

None for the abort path itself — it is the shipped behavior (contracts.md §5;
`run_pipeline`'s first-error selection branch). The wave that accompanies this
ADR is entirely prevention/diagnosis and is recorded in DCR-0012:

* Output-aware packing and the `OutputBudgetWarning` preflight
  (`crates/transync-core/src/batch.rs`).
* Terminal-error annotation: when the aborting batch was flagged by the
  preflight, `run_pipeline` appends `"; preflight: <diagnosis>"` to the inner
  message of the terminal `TranslatorError` (variant and `stable_code()`
  preserved — message enrichment, not error reshaping). This makes the abort
  self-diagnosing at the CLI, which is otherwise unreachable by the
  success-path report/stderr channel.

## Consequences

* Good, because a non-zero exit always means "your outputs are as before, and
  here is why" — the staged-commit contract and the exit-code semantics stay
  honest; there is never a silently mixed-language "success."
* Good, because the terminal-error annotation names the culprit block, its
  estimated output size vs the ceiling, and the remediation flags, so the
  most common abort (output truncation) diagnoses itself.
* Bad, because a **boundary document** — one whose largest translatable unit
  is a single block bigger than the output ceiling can hold (a giant table or
  a wall-of-text paragraph) — is *untranslatable* under a too-low ceiling:
  output-aware packing cannot split a single unit, so the run aborts until the
  operator raises `--target-output-tokens` (or the deferred oversize-split
  lands — STUB-017, the provider-side row-window / block splitter). The
  preflight names the block and the remediation flags rather than letting the
  provider truncate silently.
* Bad, because callers who genuinely want "best-effort, pass through what
  fails" have no supported mode; that appetite is deliberately unmet.

## Related

- DCR-0012 — output-aware batching + CLI knobs (the prevention-side mitigation)
- DCR-0006 / DCR-0011 — staged fileset commit (the all-or-nothing artifact discipline this preserves)
- ADR-0009 — bounded retry policy (the *validation* fallback path, which is unchanged and distinct)
- STUB-017 — deferred provider-side oversize / row-window splitter
- contracts.md §5 — retry / fallback policy ("a `Translator` error is not a fallback path")

## Note (2026-08-08) — cancellation adopts this shape, and is not this decision

*Appended, not a rewrite. Nothing above changes; this note records that a later
decision leaned on the reasoning here, and marks the boundary between them.*

DCR-0024 added run cancellation (`TranslateOptions.cancel`), and settled its
central question — what a cancelled run returns — by applying this ADR's
reasoning unchanged: a cancelled run answers `Err(TransyncError::Cancelled)`
and writes no output, rather than a `TranslationOutput` whose unreached blocks
are marked `fallback_source`. The driver that carried it is the second one
listed above: *loud failure beats a quietly-degraded document*, because an
untranslated island is not visually distinguished in raw Markdown and a
downstream reader can ship it without noticing.

Cancellation added a second objection of its own, which is why it is a separate
record rather than an extension of this one: `fallback_source` is a statement
that a block *was attempted* and its retries were spent (ADR-0009, invariant
6), and a cancelled run's unreached blocks were never attempted. Borrowing the
marker would make the two indistinguishable in the alignment map.

The scope boundary: **this ADR governs provider/transport-terminal batch
failures only.** A cancellation is not a failure at all — it is the caller's
own instruction — so it does not consume this ADR's rejected alternatives, and
a future proposal to add a per-batch fallback rung would still be arguing
against the decision above, not against DCR-0024.

## Note (2026-08-09) — the row-window splitter is commissioned; the boundary-document cost narrows to non-table kinds

*Appended, not a rewrite. Nothing in the decision above moves.*

DCR-0026 (ticket `fc0304`, owner decision 2026-08-06) commissions the deferred
oversize split this ADR's first "Bad" consequence pointed at (STUB-017), for
**tables only**, and it lands entirely on this ADR's prevention side: an
oversize table is split into header-carrying row-window units **at packing
time, before the first provider call**, deterministically — the same kind of
act as batch packing, not a runtime rung that catches a provider failure.

What this ADR settled stays settled, on every point:

- **Abort remains the terminal behavior.** No per-batch fallback rung and no
  policy flag were added. A provider/transport-terminal error still aborts the
  whole run, windows included; a provider-side oversize signal after packing is
  still terminal. DCR-0026 explicitly rejected a reactive re-split for exactly
  the reasons recorded here.
- **The "boundary document" consequence narrows rather than disappears.** A
  document whose over-ceiling block is a **table** stops being untranslatable
  under a too-low ceiling once DCR-0026's slices ship. Every other kind —
  paragraph, code block (invariant 4), list item, blockquote, html — keeps this
  ADR's semantics exactly, each excluded by a recorded per-kind decision in
  DCR-0026 §7, with the preflight naming the block and the ceiling flags (and
  now the table-strategy knob, where applicable) as the remedy.
- The preflight and its terminal-error annotation are unchanged; a window that
  cannot fit even alone (a single giant row) is flagged and aborts exactly as a
  whole oversize unit does today.
