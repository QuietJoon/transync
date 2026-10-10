---
type: ADR
title: Reject changes to the bounded retry policy (provider-declared fallback, verbatim resubmission)
description: Retrying provider-declared fallbacks within the per-unit budget and resubmitting rejected units verbatim are deliberate tradeoffs, documented here so they are not relitigated.
tags: [decision, ADR-0009]
status: active
---

# ADR: Reject changes to the bounded retry policy

## Context and Problem Statement

Found in Review 0006 (Issues R0006-0022 and R0006-0023, Severity: Medium) (review archived and removed).
Location: `crates/transync-core/src/validate.rs` (`validate_unit`, Provider
layer) and `crates/transync-core/src/pipeline/retry.rs`
(`retry_validation_unit`)

The reviewer flagged (a) that a provider-declared
`FailedNeedsFallback` is retried through the same budget as structural
validation failures, and (b) that validation retries resubmit the exact
same unit without corrective guidance.

## Decision Drivers

* A non-deterministic provider can recover on retry; a deterministic one
  stays at fallback and the per-unit budget bounds the waste. This is
  documented at the rejection site.
* Injecting guidance into `source_payload` was tried and reverted: a
  faithful provider echoes the injected note into the output, which the
  strict full-document reparse then rejects as an extra block. A safe
  retry signal needs a non-content channel (system prompt or a
  retry-hint side field on `TranslationUnit` threaded through the
  provider prompt) — a cross-crate feature, not a fix.

## Considered Options

1. Skip retries for provider-declared fallbacks (rejected — loses the
   non-deterministic-provider recovery for marginal savings).
2. Inject failure hints into the payload (rejected — breaks full reparse;
   previously attempted).
3. Keep the bounded uniform retry; build the retry-hint side channel as a
   future feature if retry efficacy becomes a measured problem.

## Decision Outcome

REJECT (both issues): We keep option 3. Third-party review keeps raising
this; the policy is intentional and its escape hatch
(`max_per_unit_validation_retries`) is caller-configurable.

Status: The rejection required no change. The side channel option 3
anticipated is implemented (DCR-0009), and the rules below are the
current policy built around it.

### Implementation

`validate_unit` reports a provider-declared `FailedNeedsFallback` as a
Provider-layer rejection, so the per-unit budget applies to it exactly as
to a structural failure. `retry_validation_unit` attaches the side channel
to a re-dispatched unit and touches nothing else.

**Resubmission is verbatim.** A unit that fails validation is re-dispatched
with the same `source_payload` and the same scope — the pristine original
with `retry: None` re-fetched from the request map — and guidance is
confined to the non-content `RetryContext` side channel. The rule holds in
every round: the budgets and packing rules below decide what a retry is
charged to and which request it rides in, never its payload or its scope.

**The side channel.** `TranslationUnit.retry`
(`RetryContext { attempt, rejected_by, reason }`) is present only on a
re-dispatched unit. The provider-neutral request assembly, `llm::prompt` in
`transync-core` (OI-0029 / DCR-0015), serializes it as a data-framed `retry`
object in the user-prompt unit (never into `source_payload`) and extends the
injection-guard framing to the source-derived `reason` field. `contracts.md`
§1 owns that wire contract.

### Three bounded budgets

Each budget is charged for its own class of fault, and none is spent on
another's.

- `max_per_unit_validation_retries` (default 2) — faults in the unit's own
  output, a provider-declared `FailedNeedsFallback` included. Total content
  attempts per unit = retries + 1.
- `max_per_batch_schema_retries` (default 2; DCR-0014, OI-0031) —
  batch-envelope faults only: a requested unit's result row dropped, or
  returned more than once. Such a fault is not charged to any batch-mate's
  per-unit budget. Batch-mates whose rows arrived intact are validated and
  accepted in that same round; the implicated units are re-dispatched
  verbatim on this budget (charged once per round, library-configurable in
  the same spirit as the per-unit budget), and finalize as honest
  `fallback_source` once it is spent. The budget is separate rather than
  shared because a dropped unit's *content* was never judged, so spending
  its content budget on the provider's formatting failure would leave a
  twice-dropped unit with fewer real translation attempts than a
  merely-mistranslated one — the opposite of the bounded-fairness this ADR
  defends. Schema faults are validation-layer events with a fallback path,
  not provider/transport-terminal errors, so ADR-0017 does not reach them.
- `max_per_batch_provider_retries` (default 1) — transient `Network` /
  `RateLimited` transport failures only, charged **per batch**: the counter
  carries across every dispatch round of the batch's retry ladder, so one
  batch spends at most this many transient retries in total, and the backoff
  ordinal keeps climbing with it. It is not a content budget: it is charged
  only for errors that never produced a payload, so it neither touches
  `max_per_unit_validation_retries` nor participates in the verbatim
  resubmission rule, and it has no fallback rung — ADR-0017 owns what happens
  when it is spent (whole-run abort, not `fallback_source`).

Retries are provably bounded: total rounds per input batch are
`≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`.
The bound counts *rounds*; how many *provider calls* one round may contain is
a quantity the bound never counted.

### A round is a pass, not a request

Retry units are re-packed through `batch::group_by_token_budget` under the
same budget the first round used, and the estimator encodes each unit's
`retry` hint, so a round whose hints push it past the token target divides
into several in-budget requests. The distinction is between the *payload*
and the *request boundary* around it: re-packing only decides **which
request** a unit rides in; it never rewrites, merges, splits, or re-scopes
one, and the tests assert the re-dispatched payloads are byte-identical to
the first round's.

Splitting a round changes no budget's meaning or arithmetic.
`max_per_unit_validation_retries` and `max_per_batch_schema_retries` are
charged **per round**, and a round is a pass over the outstanding population
however many batches it is divided into: the batch-fault admission runs once
over the round's union of offenders, so a split round costs exactly what an
unsplit one would. The per-batch transient budget spans the whole ladder.

**The same budget packs every round**, and it reserves the per-batch
user-message instruction encoded from the string `llm::prompt` assembles.
The instruction's raw-HTML segment clause rides only on a batch that holds a
raw-HTML unit, so its cost depends on batch membership — which is what
packing decides. The owner's resolution is a **document-level** upper bound:
reserve the clause whenever the *document* holds at least one html unit
(DCR-0026 reserves its row-window clause on the same document-level bound).
`pipeline::dispatch` can see one batch, not the document. Deriving the
verdict there would give an html-free batch of an html-bearing document a
*smaller* reserve than the round that packed it, and "the same budget packs
every round" would quietly stop being true. So the verdict is computed once
by `run_pipeline`, where the whole population is still in hand, and handed to
every batch. Both packers price the same instruction, byte for byte.

### A packing-time split is not a retry

DCR-0026's oversize table row-window split sits beside the resubmission rule,
not inside it:

- **The split happens before round one, once, deterministically.** It is an
  act of the same kind as `group_by_token_budget` deciding batch boundaries —
  packing, not retrying. It consumes no retry budget and touches no in-flight
  unit.
- **A window is an ordinary unit from birth.** Its payload and scope are
  fixed for the whole run. A validation failure re-dispatches *that window*
  verbatim — same payload, same scope, guidance confined to the non-content
  `RetryContext` — charged to `max_per_unit_validation_retries` per window.
  The batch-fault and transient budgets apply to windows unchanged, and the
  round bound holds with `U` counting windows.
- **No re-split exists.** Nothing — no failure, no provider signal, no retry
  round — changes any unit's scope after packing. The verbatim-resubmission
  rule is therefore untouched rather than amended.

Nor does the split reach into ADR-0017: a window that cannot fit the
ceiling even alone is flagged by the preflight and its terminal provider
error aborts the run — the split adds prevention ahead of this ADR's
machinery, never a rung inside it.

## Consequences

* Good, because retry behavior stays deterministic-safe and full-reparse-safe.
* Good, because a batch-envelope fault spends no unit's content budget, so a
  dropped unit keeps as many real translation attempts as a mistranslated one.
* Bad, because deterministic providers burn bounded retry calls on
  provider-declared fallbacks.
* Bad, because a run against a provider that is both flaky and structurally
  sloppy aborts as soon as one batch meets more transient failures across its
  whole ladder than its transient budget allows — sooner than a per-round
  budget would. That is the documented contract taking effect, and the escape
  hatch is the same caller-configurable knob.
* Bad, because packing every round to the same budget, with a document-level
  instruction reserve, can divide a round into more requests for the same
  work. That is the asymmetry `batch.rs` documents for the output expansion
  factor — overestimating costs linear request overhead, underestimating
  aborts the run at the provider.

## History

Full text of each event: [`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0009.

- 2026-07 (DCR-0009): the side channel shipped as `TranslationUnit.retry`; the `transync-openai` client serialized it.
- 2026-08-03 (DCR-0014, OI-0031): a second budget, `max_per_batch_schema_retries`, for batch-envelope faults only.
- 2026-08-03 (OI-0029, DCR-0015): `retry` serialization moved with request assembly into `transync-core`'s `llm::prompt`.
- 2026-08-05 (ti `294dda`): `max_per_batch_provider_retries` charged per batch across the whole ladder, not per round.
- 2026-08-06 (R0001-0012, ti `24fd28`): retry rounds packed by the first round's budget; the estimator encodes `retry`.
- 2026-08-06 (ti `aa92d64f`): the instruction reserve encoded from `llm::prompt`; the raw-HTML clause on a document-level bound.
- 2026-08-09 (DCR-0026, ti `fc0304`): a packing-time row-window split is not a retry; windows are units; no re-split.
