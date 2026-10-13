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
raise the ceiling, raise the expansion factor, or split the input without
authoring a profile TOML. Abort stays the terminal behavior; prevention makes
the terminal case rare and self-diagnosing.

The scope boundary: **this ADR governs provider/transport-terminal batch
failures only.** Run cancellation (DCR-0024, `TranslateOptions.cancel`)
applies this ADR's reasoning unchanged: a cancelled run answers
`Err(TransyncError::Cancelled)` and writes no output, rather than a
`TranslationOutput` whose unreached blocks are marked `fallback_source`,
because *loud failure beats a quietly-degraded document*. Cancellation adds an
objection of its own, which is why it is a separate record rather than an
extension of this one: `fallback_source` is a statement that a block *was
attempted* and its retries were spent (ADR-0009, invariant 6), and a cancelled
run's unreached blocks were never attempted, so borrowing the marker would make
the two indistinguishable in the alignment map. A cancellation is not a failure
at all — it is the caller's own instruction — so it does not consume this ADR's
rejected alternatives, and a proposal to add a per-batch fallback rung argues
against the decision above, not against DCR-0024.

**An oversize table is split at packing time (DCR-0026, ticket `fc0304`).**
This is the oversize split STUB-017 tracked, built for **tables only**, and it
lies entirely on this ADR's prevention side: an oversize table is split into
header-carrying row-window units **at packing time, before the first provider
call**, deterministically — the same kind of act as batch packing, not a
runtime rung that catches a provider failure. DCR-0026 §1 sets when the split
applies. It moves nothing this ADR settled:

- **Abort remains the terminal behavior.** A provider/transport-terminal error
  aborts the whole run, windows included; a provider-side oversize signal after
  packing is terminal. DCR-0026 explicitly rejected a reactive re-split for
  exactly the reasons recorded here.
- The preflight and its terminal-error annotation are unchanged by the split; a
  window that cannot fit even alone (a single giant row) is flagged and aborts
  exactly as a whole oversize unit does.

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
  is a single block bigger than the output ceiling can hold (a wall-of-text
  paragraph, say) — is *untranslatable* under a too-low ceiling: output-aware
  packing cannot split a single unit, so the run aborts until the operator
  raises `--target-output-tokens`. The cost narrows rather than disappears: where the
  row-window split applies (DCR-0026 §1) an over-ceiling table is split into
  row windows (Decision Outcome), and every
  other kind — paragraph, code block (invariant 4), list item, blockquote,
  html — keeps it exactly, each excluded by a recorded per-kind decision in
  DCR-0026 §7. The preflight names the block and the ceiling flags (and the
  table-strategy knob, where applicable) as the remedy rather than letting the
  provider truncate silently.
* Bad, because callers who genuinely want "best-effort, pass through what
  fails" have no supported mode; that appetite is deliberately unmet.

## Related

- DCR-0012 — output-aware batching + CLI knobs (the prevention-side mitigation)
- DCR-0006 / DCR-0011 — staged fileset commit (the all-or-nothing artifact discipline this preserves)
- ADR-0009 — bounded retry policy (the *validation* fallback path, which is unchanged and distinct)
- DCR-0024 — run cancellation (a separate decision that applies this ADR's reasoning; see Decision Outcome)
- DCR-0026 — packing-time row-window split for oversize tables (the STUB-017 oversize splitter, tables only)
- contracts.md §5 — retry / fallback policy ("a `Translator` error is not a fallback path")

## History

The full text of each moved passage is in
[`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0017.

- 2026-07-24 (DR-2026-07): the owner kept whole-run abort; the per-batch fallback rung and the policy flag were both rejected.
- 2026-07-27 (DCR-0012): the prevention-side wave shipped — output-aware packing, the at-risk preflight and the CLI batching flags.
- 2026-08-08 (DCR-0024): cancellation adopted this ADR's reasoning as its own decision; a note marked this ADR's scope as terminal failures only.
- 2026-08-09 (DCR-0026, ti `fc0304`): the STUB-017 splitter was commissioned for tables; the boundary-document cost narrowed to non-table kinds.
