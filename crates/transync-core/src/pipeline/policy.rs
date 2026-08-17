//! Pure retry / fallback decision policy for one batch (OI-0008).
//!
//! This module owns the *decisions* [`super::dispatch::process_one_batch`] used to make
//! inline: which budget a rejection is charged to, whether a unit is
//! re-dispatched or finalized, how long to wait after a transient provider
//! error, and what the report counts as a content retry.
//!
//! It owns **no I/O** — no provider, no cache, no clock, no logging, and
//! nothing `async`. The orchestrator keeps every side effect (dispatch, cache
//! get/put/evict, sleeping the returned [`Duration`], attempt-row emission,
//! tracing) and consults this module at each decision point. That is what
//! makes the state machine unit-testable without a mock provider.
//!
//! **Not** owned here: the run-level retry-batch id allocator
//! (`retry_seq`, OI-0025) is orchestration state shared across batches, and
//! the transient-error *predicate* (`TranslatorError::Network |
//! RateLimited`) stays at the orchestrator's `match` so no provider type
//! reaches this module.
//!
//! TRACE: SCN-07
//! TRACE: SCN-08
//! TRACE: OI-0031
//! TRACE: contracts.md §5

use crate::TranslateOptions;
use crate::id::BlockId;
use crate::validate::AttemptOutcome;
use std::collections::HashMap;
use std::time::Duration;

/// What the orchestrator should do with one validated unit this round.
///
/// The policy decides the *routing*; the caller performs the effects. In
/// particular cache-put eligibility is the caller's call, made from the
/// unit's `final_status` — see [`UnitDisposition::Accept`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnitDisposition {
    /// The unit passed its own layers this round. Finalize it, and put it in
    /// the cache unless its `final_status` says otherwise (the html-splice
    /// engine fault sets `FallbackSource` with no `rejected_by`, and must
    /// never be cached — contracts.md §5).
    Accept,
    /// The unit's own output was rejected and its content budget still has
    /// room: re-dispatch it verbatim carrying `RetryContext { attempt }`.
    RetryUnit { attempt: u32 },
    /// The unit's row was dropped or duplicated by the provider, so its
    /// content was never judged: re-dispatch it verbatim on the per-batch
    /// schema budget, carrying `RetryContext { attempt }`.
    RedispatchOffender { attempt: u32 },
    /// No budget left. Finalize the unit as it stands (an honest
    /// `fallback_source`) and never cache it.
    FinalizeFallback,
}

/// The per-batch retry/fallback state machine.
///
/// One instance per call to [`super::dispatch::process_one_batch`]; every counter is
/// batch-local, which is what makes batches independent.
///
/// Round protocol — the orchestrator calls, in order:
/// 1. [`Self::begin_dispatch_round`] before dispatching,
/// 2. [`Self::on_transient_error`] for each transient provider failure,
/// 3. [`Self::admit_batch_fault_round`] once, after validation,
/// 4. [`Self::record_dispatch`] then [`Self::unit_disposition`] once per
///    validated unit, in the batch's request order.
///
/// Every budget here is charged **per batch**, including the transient
/// transport one: opening a round resets nothing.
pub(crate) struct BatchPolicy {
    /// `opts.max_per_unit_validation_retries` (D5i).
    max_unit_validation_retries: u32,
    /// `opts.max_per_batch_schema_retries` (D5e).
    max_batch_schema_retries: u32,
    /// `opts.max_per_batch_provider_retries` (D5a).
    max_provider_retries: u32,
    /// OI-0031: two per-unit counters, deliberately not one.
    /// `dispatch_counter` is the unit's attempt ordinal — every round it
    /// appears in, whatever the outcome — and drives `attempt_number` +
    /// `RetryContext.attempt`.
    dispatch_counter: HashMap<BlockId, u32>,
    /// …while `unit_fault_counter` counts only rejections of the unit's
    /// *own* output and drives the per-unit content budget. With no batch
    /// fault the two coincide, so attempt numbering and budget arithmetic
    /// are unchanged from the pre-OI-0031 single counter.
    unit_fault_counter: HashMap<BlockId, u32>,
    /// Rounds charged to this batch's schema-fault budget.
    batch_fault_rounds: u32,
    /// This round's routing decision, set by [`Self::admit_batch_fault_round`].
    redispatch_offenders: bool,
    /// The round ordinal `redispatch_offenders` was last decided for, so the
    /// once-per-round protocol is debug-asserted rather than assumed: a
    /// second admission in one round would double-charge the schema budget,
    /// and a missing one would route the round on the *previous* round's
    /// decision. 0 = no round admitted yet.
    admitted_round: u32,
    /// Transient provider attempts charged to this batch so far — across
    /// every dispatch round of its ladder, never reset (ti 294dda).
    provider_attempts: u32,
    /// Dispatch rounds opened so far, 1-based once the first one starts.
    rounds: u32,
}

impl BatchPolicy {
    /// Copy the caller's budgets; the counters start empty.
    pub(crate) fn new(opts: &TranslateOptions) -> Self {
        Self {
            max_unit_validation_retries: opts.max_per_unit_validation_retries,
            max_batch_schema_retries: opts.max_per_batch_schema_retries,
            max_provider_retries: opts.max_per_batch_provider_retries,
            dispatch_counter: HashMap::new(),
            unit_fault_counter: HashMap::new(),
            batch_fault_rounds: 0,
            redispatch_offenders: false,
            admitted_round: 0,
            provider_attempts: 0,
            rounds: 0,
        }
    }

    /// Open a dispatch round and return its 1-based ordinal (the caller
    /// checks it against [`Self::max_rounds`]).
    ///
    /// The transient-transport budget deliberately does **not** reset here:
    /// `max_per_batch_provider_retries` is charged per *batch*, so one batch
    /// consumes at most that many transient retries across its whole retry
    /// ladder, however many rounds the ladder takes. Until ti 294dda this
    /// method reset the counter, which made the budget per round and let a
    /// batch spend `rounds × max_per_batch_provider_retries` in total.
    pub(crate) fn begin_dispatch_round(&mut self) -> u32 {
        self.rounds += 1;
        self.rounds
    }

    /// D5a: the provider raised a transient transport error.
    ///
    /// Returns `Some(backoff)` — how long the caller should sleep before
    /// re-dispatching — while the batch's budget has room, and `None` when
    /// it is spent, meaning the caller surfaces the provider's error. The
    /// budget spans dispatch rounds (see [`Self::begin_dispatch_round`]), so
    /// the backoff ordinal keeps growing across a batch's ladder too.
    pub(crate) fn on_transient_error(&mut self, retry_after: Option<Duration>) -> Option<Duration> {
        if self.provider_attempts >= self.max_provider_retries {
            return None;
        }
        self.provider_attempts += 1;
        Some(backoff_delay(self.provider_attempts, retry_after))
    }

    /// D5e: decide this round's batch-fault routing, once, *before* the
    /// per-unit walk.
    ///
    /// `has_offenders` is whether the round's [`crate::validate::BatchFault`]
    /// implicates at least one requested unit (a fault naming only foreign
    /// ids costs nobody anything). Returns whether the offenders are
    /// re-dispatched; when they are, the round is charged **once**, not once
    /// per offender, so an entirely empty response costs one budget point.
    ///
    /// Must be called exactly once per round — including rounds with no
    /// fault at all (`has_offenders == false`), which is how the previous
    /// round's routing is cleared. Debug-asserted, the same way
    /// [`Self::unit_disposition`] asserts its own ordering against
    /// [`Self::record_dispatch`].
    pub(crate) fn admit_batch_fault_round(&mut self, has_offenders: bool) -> bool {
        debug_assert!(
            self.rounds > 0 && self.admitted_round < self.rounds,
            "admit_batch_fault_round must run exactly once per dispatch round (round {}, last admitted {})",
            self.rounds,
            self.admitted_round
        );
        self.admitted_round = self.rounds;
        self.redispatch_offenders =
            has_offenders && self.batch_fault_rounds < self.max_batch_schema_retries;
        if self.redispatch_offenders {
            self.batch_fault_rounds += 1;
        }
        self.redispatch_offenders
    }

    /// Rounds charged to the per-batch schema budget so far — the report's
    /// `batch_schema_faults` contribution for this batch.
    pub(crate) fn batch_fault_rounds(&self) -> u32 {
        self.batch_fault_rounds
    }

    /// The ceiling [`Self::batch_fault_rounds`] is charged against
    /// (`opts.max_per_batch_schema_retries`), so a caller reporting "n of m
    /// rounds used" reads both numbers off the policy that owns them rather
    /// than pairing this module's counter with its own copy of the options.
    pub(crate) fn max_batch_fault_rounds(&self) -> u32 {
        self.max_batch_schema_retries
    }

    /// D5f: bump and return the unit's 1-based dispatch ordinal for this
    /// round. The caller stamps it on the round's `AttemptOutcome`.
    ///
    /// Must be called exactly once per unit per round, before
    /// [`Self::unit_disposition`] for that unit.
    pub(crate) fn record_dispatch(&mut self, unit_id: &BlockId) -> u32 {
        let n = self.dispatch_counter.entry(unit_id.clone()).or_insert(0);
        *n += 1;
        *n
    }

    /// D5h / D5i / D5j: route one validated unit.
    ///
    /// `is_offender` — the round's batch fault implicated this unit (its row
    /// was dropped or duplicated, so its content was never judged).
    /// `validation_failed` — `rejected_by.is_some()`.
    ///
    /// Only a fault in the unit's OWN output draws on its content budget, so
    /// a batch-fault round leaves that budget untouched.
    pub(crate) fn unit_disposition(
        &mut self,
        unit_id: &BlockId,
        is_offender: bool,
        validation_failed: bool,
    ) -> UnitDisposition {
        let dispatch_n = self.dispatch_counter.get(unit_id).copied().unwrap_or(0);
        debug_assert!(
            dispatch_n > 0,
            "record_dispatch must run before unit_disposition for {unit_id:?}"
        );
        debug_assert!(
            self.rounds > 0 && self.admitted_round == self.rounds,
            "admit_batch_fault_round must run before unit_disposition for {unit_id:?} (round {}, last admitted {})",
            self.rounds,
            self.admitted_round
        );

        if is_offender {
            return if self.redispatch_offenders {
                UnitDisposition::RedispatchOffender {
                    attempt: dispatch_n + 1,
                }
            } else {
                UnitDisposition::FinalizeFallback
            };
        }

        if !validation_failed {
            return UnitDisposition::Accept;
        }

        let fault_n = {
            let n = self.unit_fault_counter.entry(unit_id.clone()).or_insert(0);
            *n += 1;
            *n
        };
        if fault_n <= self.max_unit_validation_retries {
            UnitDisposition::RetryUnit {
                attempt: dispatch_n + 1,
            }
        } else {
            UnitDisposition::FinalizeFallback
        }
    }

    /// The documented termination bound for the per-batch loop: every round
    /// after the first is preceded by at least one charge, and both charge
    /// pools are bounded, so the loop runs at most this many times.
    ///
    /// Kept as a debug-assertion hook (and as the anchor the doc comment on
    /// [`super::dispatch::process_one_batch`] points at) rather than as a live guard —
    /// tripping it means a budget stopped being charged, which is a bug, not
    /// a condition to recover from.
    pub(crate) fn max_rounds(&self, unit_count: usize) -> u32 {
        let units = u32::try_from(unit_count).unwrap_or(u32::MAX);
        1u32.saturating_add(self.max_batch_schema_retries)
            .saturating_add(units.saturating_mul(self.max_unit_validation_retries))
    }
}

/// The transient-retry backoff schedule, as a pure function of the attempt
/// ordinal and the provider's `Retry-After`.
///
/// A provider-supplied `Retry-After` wins, capped at 30 s; otherwise 200 ms
/// doubling per attempt, capped at 5 s. R0006-0015.
pub(crate) fn backoff_delay(attempts: u32, retry_after: Option<Duration>) -> Duration {
    match retry_after {
        Some(d) => d.min(Duration::from_secs(30)),
        None => Duration::from_millis(200u64 << attempts.saturating_sub(1).min(5))
            .min(Duration::from_secs(5)),
    }
}

/// How many of a unit's attempt rows are **content retries** — the quantity
/// `ValidationReport.total_retries` totals.
///
/// A content retry is a re-dispatch of the unit's own content, so three row
/// kinds are excluded:
/// - the cache-hit re-validation row (`attempt_number == 0`), which is not a
///   network attempt and charges no budget (ADR-0015);
/// - batch-fault rounds (`batch_fault == true`), in which the provider
///   mangled the envelope and the unit's content was never judged — those
///   are counted apart, in `ValidationReport.batch_schema_faults` (OI-0031);
/// - the unit's *first* content dispatch, which is the original attempt
///   rather than a retry.
pub(crate) fn content_retry_count(attempts: &[AttemptOutcome]) -> u32 {
    let content_dispatches = attempts
        .iter()
        .filter(|a| a.attempt_number > 0 && !a.batch_fault)
        .count();
    content_dispatches.saturating_sub(1) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::id::BlockId;
    use crate::validate::AttemptOutcome;
    use std::time::Duration;

    fn opts() -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        }
    }

    fn id(s: &str) -> BlockId {
        BlockId(s.to_string())
    }

    /// One round of a unit that is not implicated in a batch fault. Opens the
    /// round first: the whole protocol (open → admit → dispatch → dispose) is
    /// debug-asserted, so a test that skipped a step would trip it.
    fn content_round(p: &mut BatchPolicy, u: &BlockId, failed: bool) -> (u32, UnitDisposition) {
        p.begin_dispatch_round();
        p.admit_batch_fault_round(false);
        let n = p.record_dispatch(u);
        (n, p.unit_disposition(u, false, failed))
    }

    // D5i: the per-unit content budget. Default 2 retries → 3 content
    // dispatches; the third fault finalizes instead of re-queueing.
    #[test]
    fn per_unit_content_budget_exhausts_after_the_default_two_retries() {
        let o = opts();
        assert_eq!(o.max_per_unit_validation_retries, 2, "default budget");
        let mut p = BatchPolicy::new(&o);
        let u = id("p-0001");

        assert_eq!(
            content_round(&mut p, &u, true),
            (1, UnitDisposition::RetryUnit { attempt: 2 })
        );
        assert_eq!(
            content_round(&mut p, &u, true),
            (2, UnitDisposition::RetryUnit { attempt: 3 })
        );
        assert_eq!(
            content_round(&mut p, &u, true),
            (3, UnitDisposition::FinalizeFallback),
            "the third fault is past the budget"
        );
    }

    // A clean round accepts and leaves the content budget untouched, so a
    // unit that fails only after a clean sibling round still gets its full
    // ladder.
    #[test]
    fn a_clean_round_is_accepted_and_charges_nothing() {
        let o = opts();
        let mut p = BatchPolicy::new(&o);
        let u = id("p-0001");
        assert_eq!(
            content_round(&mut p, &u, false),
            (1, UnitDisposition::Accept)
        );
        assert_eq!(p.batch_fault_rounds(), 0);
    }

    // D5e: charged ONCE per round (not once per offender) and capped by
    // `max_per_batch_schema_retries`.
    #[test]
    fn batch_fault_rounds_are_charged_once_per_round_and_capped() {
        let o = opts();
        assert_eq!(o.max_per_batch_schema_retries, 2, "default budget");
        let mut p = BatchPolicy::new(&o);

        p.begin_dispatch_round();
        assert!(p.admit_batch_fault_round(true));
        assert_eq!(p.batch_fault_rounds(), 1);
        p.begin_dispatch_round();
        assert!(p.admit_batch_fault_round(true));
        assert_eq!(p.batch_fault_rounds(), 2);
        p.begin_dispatch_round();
        assert!(
            !p.admit_batch_fault_round(true),
            "the batch budget is spent"
        );
        assert_eq!(p.batch_fault_rounds(), 2, "a refused round charges nothing");
    }

    // A fault that implicates no requested unit (foreign ids only) costs
    // nobody anything.
    #[test]
    fn a_fault_with_no_offender_costs_nothing() {
        let o = opts();
        let mut p = BatchPolicy::new(&o);
        p.begin_dispatch_round();
        assert!(!p.admit_batch_fault_round(false));
        assert_eq!(p.batch_fault_rounds(), 0);
    }

    // D5h vs D5i — THE separation: being dropped by the provider is not a
    // content fault, so the offender still owns its whole content ladder.
    #[test]
    fn offender_redispatch_does_not_consume_the_units_content_budget() {
        let o = opts();
        let mut p = BatchPolicy::new(&o);
        let u = id("p-0001");

        p.begin_dispatch_round();
        assert!(p.admit_batch_fault_round(true));
        assert_eq!(p.record_dispatch(&u), 1);
        assert_eq!(
            p.unit_disposition(&u, true, true),
            UnitDisposition::RedispatchOffender { attempt: 2 },
            "the offender is re-dispatched on the batch budget"
        );

        assert_eq!(
            content_round(&mut p, &u, true),
            (2, UnitDisposition::RetryUnit { attempt: 3 })
        );
        assert_eq!(
            content_round(&mut p, &u, true),
            (3, UnitDisposition::RetryUnit { attempt: 4 })
        );
        assert_eq!(
            content_round(&mut p, &u, true),
            (4, UnitDisposition::FinalizeFallback),
            "1 batch-fault round + the full (1 + 2) content ladder"
        );
    }

    // Once the batch budget is spent the offender finalizes as an honest
    // fallback — never on its own content budget.
    #[test]
    fn offender_finalizes_once_the_batch_budget_is_spent() {
        let o = TranslateOptions {
            max_per_batch_schema_retries: 0,
            ..opts()
        };
        let mut p = BatchPolicy::new(&o);
        let u = id("p-0001");

        p.begin_dispatch_round();
        assert!(!p.admit_batch_fault_round(true));
        assert_eq!(p.record_dispatch(&u), 1);
        assert_eq!(
            p.unit_disposition(&u, true, true),
            UnitDisposition::FinalizeFallback
        );
        // The content budget was never touched.
        assert_eq!(
            content_round(&mut p, &u, true),
            (2, UnitDisposition::RetryUnit { attempt: 3 })
        );
    }

    // D5a backoff schedule: 200 ms doubling, capped at 5 s.
    #[test]
    fn backoff_is_exponential_from_200ms_and_capped_at_5s() {
        assert_eq!(backoff_delay(1, None), Duration::from_millis(200));
        assert_eq!(backoff_delay(2, None), Duration::from_millis(400));
        assert_eq!(backoff_delay(5, None), Duration::from_millis(3200));
        assert_eq!(backoff_delay(6, None), Duration::from_secs(5));
        assert_eq!(backoff_delay(9, None), Duration::from_secs(5));
    }

    // A provider-supplied `Retry-After` wins over the schedule, capped at 30 s.
    #[test]
    fn retry_after_wins_and_is_capped_at_30s() {
        assert_eq!(
            backoff_delay(1, Some(Duration::from_secs(60))),
            Duration::from_secs(30)
        );
        assert_eq!(
            backoff_delay(4, Some(Duration::from_secs(2))),
            Duration::from_secs(2)
        );
    }

    // D5a: `None` once the transient budget is spent — the caller surfaces
    // the provider's error instead of sleeping again.
    #[test]
    fn transient_retries_stop_when_the_provider_budget_is_spent() {
        let o = opts();
        assert_eq!(o.max_per_batch_provider_retries, 1, "default budget");
        let mut p = BatchPolicy::new(&o);
        assert_eq!(p.on_transient_error(None), Some(Duration::from_millis(200)));
        assert_eq!(p.on_transient_error(None), None);
    }

    // ti 294dda: the transient budget is charged per BATCH. Opening a new
    // dispatch round refunds nothing, so a batch whose budget was spent in
    // round 1 surfaces the next transient error as terminal in round 2.
    // (This test previously pinned the opposite — a per-round reset — which
    // was the bug the knob's name and doc always contradicted.)
    #[test]
    fn the_provider_budget_survives_a_dispatch_round_boundary() {
        let o = TranslateOptions {
            max_per_batch_provider_retries: 2,
            ..opts()
        };
        let mut p = BatchPolicy::new(&o);
        assert_eq!(p.begin_dispatch_round(), 1);
        assert_eq!(p.on_transient_error(None), Some(Duration::from_millis(200)));

        // Round 2 inherits the spent half of the budget: one retry left, and
        // the backoff ordinal keeps climbing rather than restarting at 200 ms.
        assert_eq!(p.begin_dispatch_round(), 2);
        assert_eq!(p.on_transient_error(None), Some(Duration::from_millis(400)));
        assert_eq!(p.on_transient_error(None), None, "budget spent, not reset");
        assert_eq!(p.begin_dispatch_round(), 3);
        assert_eq!(p.on_transient_error(None), None, "and still spent");
    }

    // The documented termination bound.
    #[test]
    fn max_rounds_is_the_documented_termination_bound() {
        let o = opts();
        let p = BatchPolicy::new(&o);
        assert_eq!(p.max_rounds(3), 1 + 2 + 3 * 2);
        assert_eq!(p.max_rounds(0), 1 + 2);
    }

    fn row(attempt_number: u32, batch_fault: bool) -> AttemptOutcome {
        AttemptOutcome {
            attempt_number,
            rejected_by: None,
            rejection_reason: None,
            batch_fault,
        }
    }

    // `total_retries` semantics: content retries only. A row is a content
    // retry when it is a dispatch of the unit's own content (attempt_number
    // > 0, not a batch-fault round) beyond the first such dispatch.
    #[test]
    fn content_retries_exclude_first_dispatch_batch_faults_and_cache_rows() {
        assert_eq!(content_retry_count(&[]), 0);
        assert_eq!(content_retry_count(&[row(1, false)]), 0, "first dispatch");
        assert_eq!(content_retry_count(&[row(1, false), row(2, false)]), 1);

        // The cache-hit re-validation row is not a network attempt.
        assert_eq!(content_retry_count(&[row(0, false)]), 0);
        assert_eq!(content_retry_count(&[row(0, false), row(1, false)]), 0);
        assert_eq!(
            content_retry_count(&[row(0, false), row(1, false), row(2, false)]),
            1
        );

        // A unit dropped every round never had its content judged at all.
        assert_eq!(
            content_retry_count(&[row(1, true), row(2, true), row(3, true)]),
            0
        );
        // One batch-fault round, then a clean dispatch: zero content retries.
        assert_eq!(content_retry_count(&[row(1, true), row(2, false)]), 0);
        // One batch-fault round, then the full content ladder: two retries.
        assert_eq!(
            content_retry_count(&[row(1, true), row(2, false), row(3, false), row(4, false)]),
            2
        );
    }
}
