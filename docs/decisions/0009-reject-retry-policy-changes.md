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

Status: No change required.

### Implementation

None. The retry-hint side channel remains a candidate future feature.

## Consequences

* Good, because retry behavior stays deterministic-safe and full-reparse-safe.
* Bad, because deterministic providers burn bounded retry calls on
  provider-declared fallbacks.

## Amendment (2026-07)

The anticipated non-content retry channel shipped as `TranslationUnit.retry`
(`RetryContext { attempt, rejected_by, reason }`) — see DCR-0009.
Payload verbatim-resubmission and the bounded budget are unchanged;
only the side channel was added, exactly as Considered-Option 3 anticipated.
The `transync-openai` client serializes it as a data-framed `retry` object in
the user-prompt unit (never into `source_payload`) and extends the
injection-guard framing to the source-derived `reason` field.

## Amendment (2026-08-03)

A **second** bounded retry budget is added *beside* the per-unit one, for
batch-envelope faults only — see DCR-0014 (OI-0031). Nothing in this ADR's
decision is reversed:

- `max_per_unit_validation_retries` keeps its meaning, its default (2), and
  its arithmetic (total content attempts per unit = retries + 1). Resubmission
  is still **verbatim** — same payload, same scope — with guidance confined to
  the non-content `RetryContext` side channel.
- What changed is *attribution*, not policy. A batch-level schema fault (a
  requested unit's result row dropped, or returned more than once) is no longer
  charged to every batch-mate's per-unit budget. Batch-mates whose rows arrived
  intact are validated and accepted in that same round; the implicated units
  are re-dispatched verbatim on the new
  `TranslateOptions.max_per_batch_schema_retries` (default 2, charged once per
  round, library-configurable in the same spirit as the per-unit budget), and
  finalize as honest `fallback_source` once it is spent.
- Rationale for a separate budget rather than a shared one: a dropped unit's
  *content* was never judged, so spending its content budget on the provider's
  formatting failure would leave a twice-dropped unit with fewer real
  translation attempts than a merely-mistranslated one — the opposite of the
  bounded-fairness this ADR defends.
- Retries remain provably bounded: total rounds per input batch are
  `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`.

ADR-0017 is untouched: schema faults are validation-layer events with a
fallback path, not provider/transport-terminal errors.

## Amendment (2026-08-05) — the transient budget's scope is per batch

The third bounded budget, `max_per_batch_provider_retries` (transient
`Network` / `RateLimited` transport failures only), was **charged per dispatch
round** rather than per batch: `process_one_batch` calls the transport-retry
loop once per round of the ladder above, and the round boundary reset the
counter. A batch that walked its whole ladder could therefore spend up to
`rounds × max_per_batch_provider_retries` transient retries, while the knob's
name and its rustdoc both promised per batch (`contracts.md` §5's pseudocode
was the one document that matched the code, by restarting its `attempt`
counter at each validation-retry round). Ticket `294dda` fixed the code to
match the name: the counter now carries across dispatch rounds, so one batch
spends at most `max_per_batch_provider_retries` transient retries in total,
and the backoff ordinal keeps climbing with it. §5's table and pseudocode were
updated to state the per-batch scope explicitly.

Nothing else in this ADR moves. The transient budget is not a content budget:
it is charged only for errors that never produced a payload, so it neither
touches `max_per_unit_validation_retries` nor participates in the verbatim
resubmission rule, and it has no fallback rung — ADR-0017 still owns what
happens when it is spent (whole-run abort, not `fallback_source`). The round
bound `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
is unchanged; what changed is only how many *provider calls* one round may
contain across a batch's lifetime, which the bound never counted.

Consequence, stated plainly: a run against a provider that is both flaky and
structurally sloppy now aborts earlier than it did before 2026-08-05. That is
the documented contract taking effect, and the escape hatch is the same
caller-configurable knob (default 1).

## Amendment (2026-08-06) — a retry round is packed, and the payload still is not

Review-0001 `R0001-0012` observed that the retry side channel this ADR
authorized was never *budgeted*: `pipeline::dispatch` rebuilt the next round
straight from its `retry_units` vector, so a retry round shipped as one batch
however large — even though every re-dispatched unit carries a `RetryContext`
whose `reason` runs to 512 bytes that the original token packing never counted.
Ticket `24fd28` runs retry units through `batch::group_by_token_budget` under
the same budget the first round used, and teaches the estimator to encode the
`retry` hint.

**Nothing in this ADR's decision moves, and the verbatim rule is preserved
exactly.** The distinction is between the *payload* and the *request boundary*
around it:

- Resubmission is still verbatim — same `source_payload`, same scope, the
  pristine original with `retry: None` re-fetched from the request map, and
  guidance confined to the non-content `RetryContext`. `retry_validation_unit`
  is untouched. Re-packing only decides **which request** a unit rides in; it
  never rewrites, merges, splits, or re-scopes one, and the tests assert the
  re-dispatched payloads are byte-identical to the first round's.
- Every budget keeps its meaning and its arithmetic. `max_per_unit_validation_retries`
  and `max_per_batch_schema_retries` are charged **per round**, and a round is a
  pass over the outstanding population however many batches it is divided into:
  the batch-fault admission runs once over the round's union of offenders, so a
  split round costs exactly what an unsplit one would. The per-batch transient
  budget still spans the whole ladder (2026-08-05 amendment).
- The round bound `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
  is unchanged. As in the 2026-08-05 amendment, what changed is only how many
  *provider calls* one round may contain — a quantity the bound never counted.

Consequence, stated plainly: a retry round that used to be one over-budget
request is now several in-budget ones. That is more requests for the same work,
which is the same asymmetry `batch.rs` already documents for the output
expansion factor — overestimating costs linear request overhead, underestimating
aborts the run at the provider.

## Amendment (2026-08-06) — the budget that packs both rounds now prices the instruction

Ticket `aa92d64f`, a follow-up to the amendment above. The 2026-08-06 rule that
**the same budget packs every round** is unchanged; what changed is what that
budget reserves. The per-batch envelope used to open with a flat 256-token
allowance for the constant user-message instruction; the instruction is longer
than that, so both packers believed they had room they did not have. It is now
encoded from the string `llm::prompt` assembles.

One consequence lands on this ADR's territory, and is settled here rather than
left implicit:

- The instruction's raw-HTML segment clause rides only on a batch that holds a
  raw-HTML unit, so its cost depends on batch membership — which is what packing
  decides. The owner's resolution (2026-08-06) is a **document-level** upper
  bound: reserve the clause whenever the *document* holds at least one html unit.
- `pipeline::dispatch` can see one batch, not the document. Deriving the verdict
  there would give an html-free batch of an html-bearing document a *smaller*
  reserve than the round that packed it, and "the same budget packs every round"
  would quietly stop being true. So the verdict is computed once by
  `run_pipeline`, where the whole population is still in hand, and handed to
  every batch. Both packers price the same instruction, byte for byte.

Nothing about the payload moves, again: resubmission stays verbatim, guidance
stays confined to the non-content `RetryContext`, and every budget keeps its
meaning and arithmetic. The reserve is larger than it was, so a round packed to
its budget may divide into slightly more requests than before — the same
overestimate-is-cheap asymmetry the previous amendment states.

## Amendment (2026-08-09) — a packing-time split is not a retry, and windows are units

DCR-0026 commissions the oversize table row-window splitter (ticket `fc0304`,
STUB-017 cluster). Because "a split changes the scope" sounds adjacent to the
resubmission rule this ADR defends, the boundary is drawn here explicitly:

- **The split happens before round one, once, deterministically.** It is an act
  of the same kind as `group_by_token_budget` deciding batch boundaries —
  packing, not retrying. It consumes no retry budget and touches no in-flight
  unit.
- **A window is an ordinary unit from birth.** Its payload and scope are fixed
  for the whole run. A validation failure re-dispatches *that window* verbatim
  — same payload, same scope, guidance confined to the non-content
  `RetryContext` — charged to `max_per_unit_validation_retries` per window.
  The batch-fault and transient budgets apply to windows unchanged, and the
  round bound `1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
  holds with `U` counting windows.
- **No re-split exists.** Nothing — no failure, no provider signal, no retry
  round — changes any unit's scope after packing. The verbatim-resubmission
  rule is therefore untouched rather than amended; this note exists so the next
  reviewer does not have to re-derive that.

The relationship to ADR-0017 is unchanged as well: a window that cannot fit
the ceiling even alone is flagged by the preflight and its terminal provider
error aborts the run — the split adds prevention ahead of this ADR's machinery,
never a rung inside it.
