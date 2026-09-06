//! Per-batch dispatch — the I/O half of the retry loop (OI-0008).
//!
//! [`process_one_batch`] runs exactly one input batch end-to-end: cache
//! partition + cache-hit re-validation, provider dispatch with transport
//! retries, the batch-id identity check, per-round validation, retry-batch
//! construction, and the *apply* half of every [`super::policy`] decision.
//! Every state change is batch-local until the function returns, which is
//! what lets [`super::run_pipeline`] dispatch batches concurrently.
//!
//! TRACE: SCN-07
//! TRACE: SCN-10
//! TRACE: OI-0031

use super::{
    CacheKeyContext, CohortDigest, InstructionDigest, cache_evict, cache_get, cache_put, policy,
    retry,
};
use crate::FallbackStatus;
use crate::TranslateOptions;
use crate::cache::Cache;
use crate::error::TransyncError;
use crate::id::BlockId;
use crate::llm::{
    BatchId, OutputKind, TranslationBatch, TranslationBatchResult, TranslationUnit, Translator,
    TranslatorError, UnitResult,
};
use crate::validate::{AttemptOutcome, ValidatedBatch, ValidatedUnit};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio_util::sync::CancellationToken;

/// Per-batch result returned by [`process_one_batch`]. Owned (no
/// borrows of pipeline state) so it can travel through
/// `buffer_unordered` cleanly.
pub(crate) struct BatchResult {
    /// Index of this batch in the input order; used to keep
    /// `detected_source_language` deterministic when batches finish
    /// out of order.
    pub(crate) input_index: usize,
    pub(crate) accepted: Vec<ValidatedUnit>,
    pub(crate) attempts: Vec<(BlockId, Vec<AttemptOutcome>)>,
    /// The language reported by the earliest dispatch round whose envelope
    /// passed batch schema validation, or `None` when no round qualified
    /// (every round faulted, or the provider reported nothing). Per-unit
    /// rejections do not disqualify a round — see the latch in
    /// [`process_one_batch`] for the rule and why. R0001-0007.
    pub(crate) detected_source_language: Option<String>,
    /// Transient provider-transport retries this batch consumed. R0006-0024.
    pub(crate) provider_retries: u32,
    /// Whether this batch reached the provider at all (DCR-0028 §3). `false`
    /// exactly when every one of its units was served from cache, so no
    /// envelope — and therefore no `detected_source_language` observation —
    /// could have existed for it. The run-level fold turns this into the one
    /// question the document-metadata replay asks: did the run dispatch zero
    /// provider batches?
    pub(crate) dispatched_provider_call: bool,
    /// OI-0031: rounds charged to this batch's schema-fault budget (a
    /// mangled response envelope, re-dispatched for the implicated units).
    pub(crate) batch_schema_fault_rounds: u32,
}

/// Process exactly one input batch end-to-end: cache lookup, dispatch,
/// validation, retry-on-rejection. Independent of all other batches —
/// every state change is local until the function returns.
///
/// This function owns the **I/O**: the provider call, cache get/put/evict,
/// the backoff sleep, attempt-row emission, and logging. Every retry /
/// fallback *decision* below belongs to [`policy::BatchPolicy`], which holds
/// the counters and budgets described here and is consulted at each point.
///
/// **Two independent retry budgets (OI-0031).** A round's rejections are
/// split by scope:
/// - *unit faults* — the provider returned this unit's row and it failed a
///   per-unit layer (or the provider opted the unit out). Charged to
///   `opts.max_per_unit_validation_retries`.
/// - *batch faults* — the provider dropped this unit's row or returned it
///   more than once, so its content was never judged. Charged once per
///   round to `opts.max_per_batch_schema_retries`, never to the unit's
///   content budget, and never to an innocent batch-mate (whose returned
///   payload is validated and accepted in the round it arrived).
///
/// **A round is a pass, not a request (R0001-0012).** The first round is
/// always the one batch handed in; a retry round is re-packed by
/// [`pack_retry_batches`] under the same budget `unit::build_batches` used, so
/// it may be several batches — one provider call each — when the units'
/// ADR-0009 retry hints push them past the token target. Both budgets are
/// still charged **per round**: the batch-fault admission runs once over the
/// round's union of offenders, and the transient-transport budget is the
/// policy's and spans the whole ladder. Nothing about the *payload* changes —
/// resubmission stays verbatim (ADR-0009); only how many requests one round is
/// divided into.
///
/// Termination: every iteration either stops (no retry units), strictly
/// increments some unit's fault counter, or strictly increments this
/// batch's `batch_fault_rounds` — all three bounded, and exhausted units
/// are finalized rather than re-queued. So the loop runs at most
/// `1 + max_per_batch_schema_retries + units × max_per_unit_validation_retries`
/// times, and a provider that never returns a usable envelope terminates
/// in `1 + max_per_batch_schema_retries` rounds with every unit at an honest
/// `fallback_source`. That bound is [`policy::BatchPolicy::max_rounds`], and
/// the loop debug-asserts against it. It bounds *rounds*; a round's provider
/// calls are bounded in turn by the round's batch count times the transient
/// budget.
///
/// `document_facts` are the run-level facts the retry packer's instruction
/// reserve needs (ti aa92d6, DCR-0026): whether ANY unit in the whole document
/// carries extracted raw-HTML segments, and whether any is a table row window
/// — the two membership facts `unit::build_batches` reserved the matching
/// instruction clauses on. They are passed rather than derived so both packers
/// price the same instruction.
///
/// Returns `Err(TransyncError::Validation)` when the *request* carries
/// duplicate unit ids: re-dispatching an identically malformed request is
/// futile, so the caller's contract violation is loud instead of silently
/// draining every budget. `run_pipeline` cannot trigger it — `build_batches`
/// derives ids from distinct blocks — so this is a direct-caller guard.
///
/// **Cancellation (DCR-0024).** `cancel` is the run's token. This function
/// observes it at three places, each answering a different way a cancelled run
/// could otherwise keep spending:
/// - **at entry**, so a batch still queued behind `buffer_unordered`'s
///   concurrency cap starts no work at all — this is what stops a cancelled
///   run from issuing new requests;
/// - **at the head of every dispatch round**, so a ladder that has already
///   paid for one round does not pay for the next;
/// - **around the provider call and its backoff sleep**
///   ([`translate_with_provider_retries`]), so an in-flight request is dropped
///   and a 30-second `Retry-After` is not sat out.
///
/// It answers [`TransyncError::Cancelled`], which discards this batch's
/// `accepted` vec — but not the work behind it: every unit accepted in an
/// earlier round was written to `cache` as it was accepted, so the progress
/// survives in the caller's cache rather than in the return value. That is the
/// same OI-0011 keep-progress mechanism a failing sibling batch relies on.
///
/// TRACE: SCN-07
/// TRACE: SCN-10
/// TRACE: OI-0031
/// TRACE: DCR-0024
#[allow(clippy::too_many_arguments)]
pub(crate) async fn process_one_batch<T: Translator + ?Sized>(
    input_index: usize,
    batch: TranslationBatch,
    opts: &TranslateOptions,
    translator: &T,
    cache: &(dyn Cache + '_),
    retry_seq: &AtomicU32,
    key_ctx: &CacheKeyContext,
    ref_defs: &str,
    document_facts: crate::llm::prompt::DocumentFacts,
    cancel: &CancellationToken,
) -> Result<BatchResult, TransyncError> {
    // DCR-0024: the entry checkpoint. `buffer_unordered` polls queued batches
    // as slots free up, so without this a cancelled run would still walk every
    // remaining batch's cache partition and dispatch its first round.
    if cancel.is_cancelled() {
        return Err(TransyncError::Cancelled);
    }

    let unit_lookup: HashMap<BlockId, TranslationUnit> = batch
        .units
        .iter()
        .map(|u| (u.unit_id.clone(), u.clone()))
        .collect();

    // OI-0031 preflight: a duplicate request id collapses in `unit_lookup`
    // and would make every downstream count lie. Name the ids and abort
    // before spending a provider call.
    if unit_lookup.len() != batch.units.len() {
        let mut seen: std::collections::HashSet<&BlockId> = std::collections::HashSet::new();
        let mut dups: Vec<&str> = batch
            .units
            .iter()
            .filter(|u| !seen.insert(&u.unit_id))
            .map(|u| u.unit_id.0.as_str())
            .collect();
        dups.sort_unstable();
        dups.dedup();
        return Err(TransyncError::Validation(format!(
            "duplicate unit_id in request batch {}: [{}]",
            batch.batch_id.0,
            dups.join(", ")
        )));
    }

    let mut accepted: Vec<ValidatedUnit> = Vec::new();
    let mut attempts: HashMap<BlockId, Vec<AttemptOutcome>> = HashMap::new();
    let mut detected_source_language: Option<String> = None;
    let mut provider_retries: u32 = 0;
    // OI-0008: every retry/fallback decision below is the policy's; this
    // function keeps the I/O (dispatch, cache, sleep, attempt rows, logs).
    let mut policy = policy::BatchPolicy::new(opts);

    // Cache-key components (`key_ctx`) are computed once per run and
    // passed in by reference (EXT-2026-07 P1-4).
    //
    // ti c02f69: the one component that is neither run- nor unit-scoped is
    // the instruction this batch assembles, so it is derived once here, from
    // the batch as the packer built it, and used for every key this function
    // takes — lookup, write, and eviction alike. `run_pipeline` derives it
    // from the same batch for its eviction map, so the two agree by
    // construction.
    let instruction = InstructionDigest::for_batch(&batch);
    // DCR-0027: and the other batch-scoped component — the compiled prompt and
    // effective glossary this batch's SECTION carries. Same treatment as the
    // instruction above, for the same reason: derived once from the batch as
    // the packer built it, and used for every key this function takes.
    let cohort = CohortDigest::for_batch(&batch);

    // Cache lookup: separate units into "already cached" (use cached
    // translation directly) and "needs dispatch". The cache contract
    // (TRACE: SCN-10) lets a partial-resume run skip units that
    // succeeded in a prior call against the same cache.
    let mut cached_units: Vec<UnitResult> = Vec::new();
    let mut pending_units: Vec<TranslationUnit> = Vec::new();
    for unit in &batch.units {
        let key = key_ctx.key_for(opts, unit, instruction, cohort);
        if let Some(mut cached) = cache_get(cache, &key) {
            // Twin-block collision (cache.rs): `CacheKey` carries no
            // `BlockId`, so two blocks with identical source bytes and an
            // identical (120-char-truncated) neighbor/section context share
            // one key — and the cached `UnitResult` keeps whichever block's
            // id last populated that key. Left unrewritten, the requesting
            // unit's id never appears in the synthetic result below, so it is
            // filtered out of the synthetic batch (schema mismatch) and
            // stranded: neither accepted nor re-queued, silently degrading to
            // `FallbackSource` and losing its per-unit report row. Rewrite the
            // cached id to the requesting unit's id so every downstream
            // consumer (schema validation, per-unit report, cache re-put,
            // eviction key) sees the correct id. Cross-block dedup is
            // preserved: the payload is re-validated per unit against the
            // requesting unit's source, which is byte-identical by
            // construction (the keys collided).
            cached.unit_id = unit.unit_id.clone();
            cached_units.push(cached);
        } else {
            pending_units.push(unit.clone());
        }
    }

    if !cached_units.is_empty() {
        let synth = TranslationBatchResult {
            batch_id: batch.batch_id.clone(),
            detected_source_language: None,
            units: cached_units,
        };
        // R0006-0051: index the cached IDs once instead of scanning the
        // synthetic result list per unit (O(N*M) on large cached batches).
        let cached_ids: std::collections::HashSet<&BlockId> =
            synth.units.iter().map(|r| &r.unit_id).collect();
        let synth_batch = TranslationBatch {
            batch_id: batch.batch_id.clone(),
            units: batch
                .units
                .iter()
                .filter(|u| cached_ids.contains(&u.unit_id))
                .cloned()
                .collect(),
            ..batch.clone()
        };
        let validated = crate::validate::validate_batch(&synth_batch, &synth, ref_defs);
        // OI-0031: the synthetic batch is built 1:1 from the cached rows
        // (`synth_batch.units` is filtered to exactly `cached_ids`), so a
        // batch-level schema fault is impossible by construction here.
        debug_assert!(
            validated.batch_fault.is_none(),
            "synthetic cache batch cannot carry a schema fault: {:?}",
            validated.batch_fault
        );
        for vu in validated.units {
            let rejected_by = vu.rejected_by;
            attempts
                .entry(vu.unit_id.clone())
                .or_default()
                .push(AttemptOutcome {
                    attempt_number: 0, // 0 = cache hit, not a network attempt
                    rejected_by,
                    // EXT-2026-07 review-fix: an accepted hit is not a
                    // rejection — carry `None` so consumers don't read the
                    // old "cache hit" sentinel as a failure. A rejected hit
                    // keeps its real reason from validation.
                    rejection_reason: vu.rejection_reason.clone(),
                    batch_fault: false,
                });
            if rejected_by.is_none() {
                accepted.push(vu);
            } else if let Some(orig) = unit_lookup.get(&vu.unit_id) {
                // EXT-2026-07 review-fix: the cached translation no longer
                // passes validation (schema/prompt drift, or a poisoned
                // shared-cache entry). Evict the stale key before re-queueing
                // the pristine original — otherwise, if the fresh dispatch
                // also fails to produce a cacheable result, the disqualified
                // entry stays cached and replays on every future shared-cache
                // run (R0008-0003).
                cache_evict(cache, &key_ctx.key_for(opts, orig, instruction, cohort));
                pending_units.push(orig.clone());
            }
        }
    }

    if pending_units.is_empty() {
        return Ok(BatchResult {
            input_index,
            accepted,
            attempts: attempts.into_iter().collect(),
            detected_source_language,
            provider_retries,
            batch_schema_fault_rounds: policy.batch_fault_rounds(),
            // DCR-0028 §3: this is the one return that made no provider call —
            // every unit came from cache, so nothing observed the document.
            dispatched_provider_call: false,
        });
    }

    // R0001-0012: the packing budget this batch's units were grouped under,
    // rebuilt here so a retry round can be packed by the *same* budget rather
    // than shipping as one unbounded batch. `unit::build_batches` resolved it
    // from `opts` + the run's profile; both are still available, and
    // `batch.profile` is the rendered profile whose `prompt_body` is the very
    // system prompt the provider sees, so the reconstruction is exact.
    //
    // ti aa92d6: exact includes the instruction envelope, whose html and
    // row-window clauses are reserved on document-level facts. This function
    // sees one batch, so those facts are handed in by `run_pipeline` —
    // deriving them from `batch.units` would give an html-free batch of an
    // html-bearing document a *smaller* reserve than the round that packed it,
    // and "the same budget packs every round" would stop being true.
    let instruction_envelope = crate::llm::prompt::instruction_envelope_json(
        crate::llm::prompt::InstructionVariant::for_run(&batch.profile.constraints, document_facts),
    );
    let mut retry_budget = crate::unit::budget::resolve(
        opts,
        &batch.profile,
        translator.tokenizer_hint(),
        &batch.profile.prompt_body,
        &instruction_envelope,
    );
    // The labels on the wire are this batch's own. `build_batches` copies them
    // from `opts`, but `process_one_batch` is reachable directly with a
    // hand-built batch, and the envelope estimate must measure what is sent.
    retry_budget.source_language = &batch.source_language;
    retry_budget.target_language = &batch.target_language;

    // A dispatch round is a pass over the whole outstanding population. The
    // first round is always exactly one batch — the one handed in; a retry
    // round is however many batches the packer produced for that round's
    // re-dispatched units (R0001-0012).
    let mut current_batches = vec![TranslationBatch {
        batch_id: batch.batch_id.clone(),
        units: pending_units,
        source_language: batch.source_language.clone(),
        target_language: batch.target_language.clone(),
        glossary: batch.glossary.clone(),
        profile: batch.profile.clone(),
    }];
    // The termination bound below is a function of the dispatched population,
    // fixed before the first round (retry rounds only ever shrink it).
    let dispatched_units = current_batches[0].units.len();

    loop {
        // DCR-0024: the per-round checkpoint. A ladder can run several rounds
        // and several provider calls per round; checking here bounds a
        // cancelled batch's remaining cost at the round already in flight.
        if cancel.is_cancelled() {
            return Err(TransyncError::Cancelled);
        }
        let round = policy.begin_dispatch_round();
        debug_assert!(
            round <= policy.max_rounds(dispatched_units),
            "batch {} exceeded its retry termination bound ({round} > {}): a budget stopped being charged",
            batch.batch_id.0,
            policy.max_rounds(dispatched_units)
        );

        // Dispatch every batch this round consists of, in packing order, and
        // keep each one's validation verdict for the shared per-unit walk
        // below. The transient-transport budget is the policy's and spans the
        // whole ladder (ti 294dda), so splitting a round across several
        // provider calls does not refund it.
        let mut round_verdicts: Vec<ValidatedBatch> = Vec::with_capacity(current_batches.len());
        // R0010-0006: a sub-batch that ends the run must not discard what this
        // round's EARLIER sub-batches already earned. ADR-0017 still aborts —
        // the error is returned unchanged once the round is settled below —
        // but the units those sub-batches accepted are written to `cache`
        // first, so a resumed run does not pay the provider for them twice.
        // That is the same OI-0011 keep-progress mechanism a failing sibling
        // batch relies on: progress survives in the caller's cache, never in
        // a return value the error throws away.
        let mut round_error: Option<TransyncError> = None;
        for current_batch in &current_batches {
            let dispatched = translate_with_provider_retries(
                translator,
                current_batch.clone(),
                &mut policy,
                cancel,
            )
            .await;
            let (result, transport_retries) = match dispatched {
                Ok(pair) => pair,
                Err(e) => {
                    round_error = Some(e);
                    break;
                }
            };
            // R0003-0045: a work-driven counter — every increment is one real
            // round trip — but it is reported telemetry, and telemetry that
            // wraps is worse than telemetry that stops. Saturating keeps the
            // number honest at the top instead of restarting at zero.
            provider_retries = provider_retries.saturating_add(transport_retries);
            // OI-0013: a misbehaving provider could swap responses between
            // concurrent batches. Unit-ID schema validation would reject the
            // foreign units anyway, but the identity check names the actual
            // failure instead of degrading it into per-unit fallbacks.
            if result.batch_id != current_batch.batch_id {
                round_error = Some(TransyncError::Validation(format!(
                    "provider returned batch_id {:?} for dispatched batch {:?}",
                    result.batch_id.0, current_batch.batch_id.0
                )));
                break;
            }
            let validated = crate::validate::validate_batch(current_batch, &result, ref_defs);

            // R0001-0007: only a round whose *envelope* survived schema
            // validation may commit `detected_source_language`, so the latch
            // is written here — after `validate_batch` — and never before it.
            //
            // The disqualification rule, stated rather than left to statement
            // order: a `BatchFault` disqualifies the envelope; a per-unit
            // rejection does not.
            // - `detected_source_language` is an envelope field, and a
            //   `BatchFault` is precisely a statement that the envelope did
            //   not address the batch we sent (rows dropped, duplicated,
            //   invented, or a malformed request). A reply we cannot trust to
            //   have answered *this* batch cannot be trusted to have detected
            //   *this* batch's language either.
            // - The per-unit layers (per-kind, fragment reparse, inline
            //   protection) judge one block's content shape — table columns, a
            //   heading level, a link destination. None of that bears on which
            //   language the provider read the source in, so letting one
            //   stubborn table veto the detection would discard a sound
            //   observation for an unrelated reason, and any batch ending with
            //   a unit in fallback would report no language at all.
            //
            // The `is_none()` guard keeps the *earliest qualifying* envelope,
            // which is also the best-sourced one: retry rounds only ever
            // shrink the dispatched population (`retry_units` is a subset), so
            // an earlier round always observed at least as much of the
            // document. Within one round the batches are dispatched in packing
            // (document) order, so the choice is deterministic.
            //
            // R0003-0038: the value itself is provider-authored free text, and
            // this is the only door it enters by. It is bounded here like every
            // other provider-authored string the run keeps (`warnings`,
            // `rejection_reason`, the retry `reason`, the auto-glossary
            // diagnostic — R0002-0068 / R0008-0034), because from here it
            // reaches `TranslationOutput`, the alignment map's
            // `detected_source_language`, and the published bundle's `lang`
            // attribute. A hostile or broken provider gets to name a language,
            // not to set the size of the artifacts.
            if detected_source_language.is_none() && validated.batch_fault.is_none() {
                detected_source_language = result
                    .detected_source_language
                    .as_deref()
                    .map(crate::validate::truncate_diagnostic);
            }
            round_verdicts.push(validated);
        }

        if let Some(err) = round_error {
            persist_round_progress(
                cache,
                &round_verdicts,
                &unit_lookup,
                key_ctx,
                opts,
                instruction,
                cohort,
            );
            return Err(err);
        }

        // OI-0031: decide the round's batch-fault routing once, before the
        // per-unit walk — once per *round*, not once per batch in it, so a
        // split retry round costs the same budget an unsplit one would.
        // `malformed_request` is unreachable — the request is preflighted
        // above — and a fault with no offender (foreign ids only) costs
        // nobody anything.
        let offender_set: std::collections::HashSet<BlockId> = round_verdicts
            .iter()
            .filter_map(|v| v.batch_fault.as_ref())
            .flat_map(|bf| bf.offenders.iter().cloned())
            .collect();
        // Charged once per round, not once per offender: an empty response
        // costs one budget point, not one per dropped unit.
        let redispatch_offenders = policy.admit_batch_fault_round(!offender_set.is_empty());
        for (current_batch, validated) in current_batches.iter().zip(&round_verdicts) {
            let Some(bf) = &validated.batch_fault else {
                continue;
            };
            let disposition = if bf.offenders.is_empty() {
                "no requested unit implicated"
            } else if redispatch_offenders {
                "re-dispatched on the per-batch schema budget"
            } else {
                "finalized as fallback_source (batch budget spent)"
            };
            tracing::warn!(
                target: "transync::pipeline",
                "batch {}: provider schema fault ({}); {} offender(s) {} ({}/{} batch-fault round(s) used), {} foreign id(s) discarded",
                current_batch.batch_id.0,
                bf.reason,
                bf.offenders.len(),
                disposition,
                policy.batch_fault_rounds(),
                policy.max_batch_fault_rounds(),
                bf.foreign_ids.len()
            );
        }

        let mut retry_units: Vec<TranslationUnit> = Vec::new();
        for vu in round_verdicts.into_iter().flat_map(|v| v.units) {
            let dispatch_n = policy.record_dispatch(&vu.unit_id);
            let is_offender = offender_set.contains(&vu.unit_id);
            attempts
                .entry(vu.unit_id.clone())
                .or_default()
                .push(AttemptOutcome {
                    attempt_number: dispatch_n,
                    rejected_by: vu.rejected_by,
                    rejection_reason: vu.rejection_reason.clone(),
                    batch_fault: is_offender,
                });

            let validation_failed = vu.rejected_by.is_some();
            let disposition = policy.unit_disposition(&vu.unit_id, is_offender, validation_failed);
            match disposition {
                // Both retry shapes re-dispatch the PRISTINE original
                // (`retry == None`, so contexts never stack) carrying the
                // policy's attempt number: an offender on the batch budget
                // (ADR-0009, `RetryContext` naming the Schema layer), a
                // unit fault on the unit's own content budget.
                policy::UnitDisposition::RedispatchOffender { attempt }
                | policy::UnitDisposition::RetryUnit { attempt } => {
                    if let Some(orig) = unit_lookup.get(&vu.unit_id) {
                        retry_units.push(retry::retry_validation_unit(
                            orig.clone(),
                            attempt,
                            vu.rejected_by,
                            vu.rejection_reason.as_deref().unwrap_or(""),
                        ));
                    }
                }
                // Budget spent: finalize as it stands (an honest
                // `fallback_source`) and never persist it.
                policy::UnitDisposition::FinalizeFallback => accepted.push(vu),
                policy::UnitDisposition::Accept => {
                    debug_assert!(
                        !validation_failed,
                        "Accept implies the unit passed its own layers"
                    );
                    if let (Some(orig), Some(cached_result)) =
                        (unit_lookup.get(&vu.unit_id), cacheable_result(&vu))
                    {
                        // ti c02f69: the instruction axis names this
                        // batch, not this round. A round dispatching a
                        // strict subset (cache hits removed peers, or a
                        // retry round did) can assemble a shorter
                        // instruction than the batch as packed — the two
                        // membership-driven clauses (the html-segment
                        // contract and the DCR-0026 row-window contract)
                        // are the only ones that can move that way.
                        // Keying on the round instead was rejected for two
                        // reasons: the lookup above runs before round
                        // membership exists (it is what decides it), and a
                        // key that moves mid-run desynchronizes
                        // `run_pipeline`'s eviction map, which would leave
                        // a reparse-disqualified entry cached under a key
                        // nothing looks up. See contracts.md §5a.
                        let key = key_ctx.key_for(opts, orig, instruction, cohort);
                        cache_put(cache, key, cached_result);
                    }
                    accepted.push(vu);
                }
            }
        }

        if retry_units.is_empty() {
            break;
        }

        current_batches = pack_retry_batches(retry_units, &retry_budget, &batch, retry_seq);
        debug_assert!(
            !current_batches.is_empty(),
            "a non-empty retry population must pack into at least one batch"
        );
    }

    Ok(BatchResult {
        input_index,
        accepted,
        attempts: attempts.into_iter().collect(),
        detected_source_language,
        provider_retries,
        batch_schema_fault_rounds: policy.batch_fault_rounds(),
        // Past the early return above the loop always runs at least one round,
        // and every round dispatches at least one batch (DCR-0028 §3).
        dispatched_provider_call: true,
    })
}

/// The cache record for a unit the round accepted, or `None` when the unit
/// must not be persisted.
///
/// contracts.md §5: a clean `rejected_by` is not enough — the html-splice
/// engine fault sets `FallbackSource` with `rejected_by: None`, and a fallback
/// must never be cached. A unit with no accepted payload has nothing to write.
fn cacheable_result(vu: &ValidatedUnit) -> Option<UnitResult> {
    let output_kind = match vu.final_status {
        FallbackStatus::Translated => OutputKind::Translated,
        FallbackStatus::Preserved => OutputKind::Preserved,
        FallbackStatus::PartiallyTranslated => OutputKind::PartiallyTranslated,
        FallbackStatus::FallbackSource => return None,
    };
    Some(UnitResult {
        unit_id: vu.unit_id.clone(),
        output_kind,
        translated_payload: vu.accepted_payload.clone()?,
        warnings: vu.warnings.clone(),
    })
}

/// Write the units a partially-dispatched round already accepted to `cache`,
/// on the way out of a round that ends in a terminal error (R0010-0006).
///
/// The error still aborts the run (ADR-0017) and this batch's `accepted` vec
/// is still discarded, so the ONLY thing this buys is that a resumed run does
/// not pay the provider a second time for work the provider already did — the
/// same OI-0011 keep-progress mechanism a failing sibling batch relies on.
///
/// The acceptance rule is `policy::UnitDisposition::Accept`'s, restated rather
/// than asked for: the policy's counters are round-scoped bookkeeping for a
/// round that is not going to finish, so recording dispatches and charging
/// budgets here would only corrupt them. `Accept` is exactly "not an offender
/// and not rejected", and the offender set is the union over the verdicts that
/// DID arrive — a fault in an earlier sub-batch of the same round still
/// disqualifies its offenders here.
fn persist_round_progress(
    cache: &(dyn Cache + '_),
    round_verdicts: &[ValidatedBatch],
    unit_lookup: &HashMap<BlockId, TranslationUnit>,
    key_ctx: &CacheKeyContext,
    opts: &TranslateOptions,
    instruction: InstructionDigest,
    cohort: CohortDigest,
) {
    let offenders: std::collections::HashSet<&BlockId> = round_verdicts
        .iter()
        .filter_map(|v| v.batch_fault.as_ref())
        .flat_map(|bf| bf.offenders.iter())
        .collect();
    for vu in round_verdicts.iter().flat_map(|v| v.units.iter()) {
        if vu.rejected_by.is_some() || offenders.contains(&vu.unit_id) {
            continue;
        }
        if let (Some(orig), Some(cached_result)) =
            (unit_lookup.get(&vu.unit_id), cacheable_result(vu))
        {
            // Same key the accept path takes: the instruction and cohort axes
            // name the BATCH as packed, not the round (ti c02f69).
            let key = key_ctx.key_for(opts, orig, instruction, cohort);
            cache_put(cache, key, cached_result);
        }
    }
}

/// Pack one round's re-dispatched units into retry batches under the SAME
/// budget the input batches were packed with (R0001-0012).
///
/// Before this existed, `retry_units` became one batch however large it was —
/// and every re-dispatched unit now carries an ADR-0009 `RetryContext` whose
/// `reason` runs to a 512-byte prefix plus a truncation marker
/// (`retry::MAX_RETRY_REASON_PREFIX_BYTES`), which the original packing never
/// counted. The
/// estimator counts it (`batch::estimate_unit`), so a round whose hints push
/// the population past the token target splits here instead of shipping one
/// over-budget request.
///
/// `template` supplies the batch-invariant fields (languages, glossary,
/// profile); `retry_seq` is the run-level retry-id allocator (OI-0025).
/// R0008-0020: every produced batch stamps its own id on every unit it
/// carries, so a translator inspecting `TranslationUnit.batch_id` always sees
/// the enclosing `TranslationBatch.batch_id` — for the second and third retry
/// batch of a round as much as for the first.
///
/// Never returns an empty `Vec` for a non-empty input: `group_by_token_budget`
/// gives an over-budget unit its own batch rather than dropping it.
///
/// TRACE: SCN-10
fn pack_retry_batches(
    units: Vec<TranslationUnit>,
    budget: &crate::batch::BatchBudget<'_>,
    template: &TranslationBatch,
    retry_seq: &AtomicU32,
) -> Vec<TranslationBatch> {
    crate::batch::group_by_token_budget(units, budget)
        .into_iter()
        .map(|chunk| {
            let batch_id = BatchId::new(retry_seq.fetch_add(1, Ordering::Relaxed));
            let units = chunk
                .into_iter()
                .map(|mut u| {
                    u.batch_id = batch_id.clone();
                    u
                })
                .collect();
            TranslationBatch {
                batch_id,
                units,
                source_language: template.source_language.clone(),
                target_language: template.target_language.clone(),
                glossary: template.glossary.clone(),
                profile: template.profile.clone(),
            }
        })
        .collect()
}

/// Re-dispatch a batch while the provider raises a transient transport-level
/// error (`Network` or `RateLimited`) and the batch's transient budget has
/// room. Other `TranslatorError` variants (auth, malformed, unsupported,
/// other) are surfaced immediately — they are not transient and retrying
/// just burns quota.
///
/// The budget (`opts.max_per_batch_provider_retries`) belongs to the whole
/// **batch**, not to one call of this function: `process_one_batch` calls it
/// once per dispatch round and the policy's counter carries over, so a batch
/// consumes at most that many transient retries across its entire retry
/// ladder (ti 294dda). The `u32` returned is this call's contribution to
/// `ValidationReport.provider_retries`, which stays the honest observed total.
///
/// R0006-0015: between attempts this honors the provider's `Retry-After`
/// when given (capped at 30 s) and otherwise backs off exponentially
/// (200 ms doubling, capped at 5 s) instead of re-dispatching in a tight
/// loop. Returns the number of transient retries consumed alongside the
/// result so the pipeline can report it (R0006-0024).
///
/// The budget and the backoff schedule are
/// [`policy::BatchPolicy::on_transient_error`]'s; this function owns only
/// the I/O — the call, the sleep, and the log. The transient *predicate*
/// stays here because it is a statement about [`TranslatorError`], which
/// deliberately never reaches the policy module.
///
/// **Cancellation (DCR-0024).** Both awaits here are raced against `cancel`:
///
/// - The **provider call**. Winning the race drops the `translate_batch`
///   future, which drops whatever it holds — for a `reqwest`-shaped client
///   that aborts the in-flight request. This is the guarantee that does not
///   depend on the implementation: a `Translator` that ignores its `cancel`
///   argument is cancelled anyway, by being dropped. Implementations whose
///   work is *not* drop-cancellable are why the argument exists as well.
/// - The **backoff sleep**. A `Retry-After` is capped at 30 s, and sleeping
///   out a rate limit for a run nobody wants is the single longest stall a
///   cancelled batch could contribute.
///
/// Both arms are `biased` so an already-cancelled token wins deterministically
/// instead of racing a provider that returns instantly — which is what makes
/// the "no new requests after cancellation" property testable rather than
/// probabilistic.
///
/// TRACE: contracts.md §1
/// TRACE: DCR-0024
async fn translate_with_provider_retries<T>(
    translator: &T,
    batch: TranslationBatch,
    policy: &mut policy::BatchPolicy,
    cancel: &CancellationToken,
) -> Result<(TranslationBatchResult, u32), TransyncError>
where
    T: Translator + ?Sized,
{
    let mut attempts: u32 = 0;
    loop {
        let outcome = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TransyncError::Cancelled),
            r = translator.translate_batch(batch.clone(), cancel) => r,
        };
        match outcome {
            Ok(result) => return Ok((result, attempts)),
            Err(e @ (TranslatorError::Network(_) | TranslatorError::RateLimited { .. })) => {
                let retry_after = match &e {
                    TranslatorError::RateLimited { retry_after } => *retry_after,
                    _ => None,
                };
                let Some(backoff) = policy.on_transient_error(retry_after) else {
                    // Budget spent: a transient error is now terminal.
                    return Err(e.into());
                };
                attempts = attempts.saturating_add(1);
                tracing::warn!(
                    target: "transync::pipeline",
                    "provider attempt {attempts} failed transiently: {e}; retrying in {backoff:?}"
                );
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => return Err(TransyncError::Cancelled),
                    _ = tokio::time::sleep(backoff) => {}
                }
            }
            Err(e) => return Err(e.into()),
        }
    }
}

// The dispatch-path pins that drive `run_pipeline` end-to-end: the cache-hit
// arm's eviction and twin-id rewrite, and the retry-batch id space (OI-0025).
#[cfg(test)]
mod cache_dispatch_tests {
    use super::*;
    // The moved tests keep the exact scope they had in `pipeline.rs`.
    use crate::pipeline::*;

    // EXT-2026-07 review-fix: a cached unit that no longer passes
    // re-validation must have its stale key evicted at the cache-hit arm.
    // Pre-seed a shared cache with a poisoned (FailedNeedsFallback) entry
    // under the exact key the run computes, drive a translator that also
    // refuses (so the fresh dispatch produces no cacheable result), and
    // assert the key is gone afterward. Without the eviction the poisoned
    // entry would replay on every future shared-cache run.
    #[tokio::test]
    async fn stale_cache_hit_key_is_evicted_when_revalidation_fails() {
        use crate::cache::InMemoryCache;

        struct AlwaysFallbackTranslator;
        #[async_trait::async_trait]
        impl Translator for AlwaysFallbackTranslator {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let units = batch
                    .units
                    .iter()
                    .map(|u| UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::FailedNeedsFallback,
                        translated_payload: String::new(),
                        warnings: Vec::new(),
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        let src = "hello world\n";
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };
        let translator = AlwaysFallbackTranslator;

        // Recompute the exact CacheKey the pipeline will derive for the sole
        // unit: id assignment + batching are deterministic from src + opts,
        // and the fingerprint is the concrete translator type's namespace.
        let mut doc = parse(src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let batches = build_batches(&doc, &opts, None, &crate::unit::html_outcomes(&doc));
        let batch = batches.first().expect("at least one batch");
        let unit = batch.units.first().expect("at least one unit").clone();
        let key_ctx = CacheKeyContext::for_run(&opts, translator.fingerprint());
        // ti c02f69: the instruction axis comes from the batch the packer
        // built, exactly as `process_one_batch` derives it.
        let key = key_ctx.key_for(
            &opts,
            &unit,
            InstructionDigest::for_batch(batch),
            CohortDigest::for_batch(batch),
        );

        // Poison the shared cache with an entry that fails re-validation.
        let cache = InMemoryCache::new();
        cache
            .put(
                key.clone(),
                UnitResult {
                    unit_id: unit.unit_id.clone(),
                    output_kind: OutputKind::FailedNeedsFallback,
                    translated_payload: String::new(),
                    warnings: Vec::new(),
                },
            )
            .unwrap();
        assert!(cache.get(&key).unwrap().is_some(), "seeded entry present");

        let cache_dyn: &dyn Cache = &cache;
        run_pipeline(src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes (all units fall back to source)");

        assert!(
            cache.get(&key).unwrap().is_none(),
            "stale cache-hit key must be evicted after re-validation failure"
        );
    }

    // OI-0025: input batches are numbered 1..=N; the retry ordinal must be
    // seeded at N+1 so retry ids can never collide with input ids regardless
    // of document size (the old fixed 10_000 seed collided once a document
    // produced >= 10_000 batches). Drive many single-unit batches through a
    // translator that fails the first attempt and recovers on retry, then
    // assert every retry ordinal is strictly greater than the largest input
    // ordinal and that the smallest retry ordinal is exactly N+1.
    #[tokio::test]
    async fn retry_batch_ids_never_collide_with_input_batch_ids() {
        use crate::cache::InMemoryCache;
        use std::sync::Mutex;

        fn ordinal(id: &BatchId) -> u32 {
            id.0.strip_prefix("b-")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or_else(|| panic!("unexpected BatchId format: {id}"))
        }

        // Fails every first attempt (Provider-layer rejection → retryable),
        // then recovers verbatim on retry (`unit.retry` is `Some`). Records
        // the ordinal of every dispatched batch, split by first-attempt vs
        // retry so the two id spaces can be compared.
        #[derive(Default)]
        struct RecordingRetryTranslator {
            input_ords: Mutex<Vec<u32>>,
            retry_ords: Mutex<Vec<u32>>,
        }
        #[async_trait::async_trait]
        impl Translator for RecordingRetryTranslator {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let ord = ordinal(&batch.batch_id);
                let is_retry = batch.units.iter().any(|u| u.retry.is_some());
                if is_retry {
                    self.retry_ords.lock().unwrap().push(ord);
                } else {
                    self.input_ords.lock().unwrap().push(ord);
                }
                let units = batch
                    .units
                    .iter()
                    .map(|u| {
                        if u.retry.is_some() {
                            // Recover: echo the source back unchanged.
                            UnitResult {
                                unit_id: u.unit_id.clone(),
                                output_kind: OutputKind::Preserved,
                                translated_payload: u.source_payload.clone(),
                                warnings: Vec::new(),
                            }
                        } else {
                            // Force a retryable rejection on the first attempt.
                            UnitResult {
                                unit_id: u.unit_id.clone(),
                                output_kind: OutputKind::FailedNeedsFallback,
                                translated_payload: String::new(),
                                warnings: Vec::new(),
                            }
                        }
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        // `max_units_per_batch = 1` puts every block in its own batch, so a
        // K-paragraph document yields exactly K input batches (b-0001..b-000K).
        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            max_units_per_batch: 1,
            ..Default::default()
        };

        // Derive the expected input ordinal set deterministically (batching is
        // pure over src + opts), independent of dispatch order.
        let input_ids_for = |src: &str| -> Vec<u32> {
            let mut doc = parse(src).expect("source parses");
            id::assign_block_ids(&mut doc);
            build_batches(&doc, &opts, None, &crate::unit::html_outcomes(&doc))
                .iter()
                .map(|b| ordinal(&b.batch_id))
                .collect()
        };

        // --- Multi-batch case: several single-unit batches. ---
        let multi_src = "para one\n\npara two\n\npara three\n\npara four\n\npara five\n";
        let input_ords = input_ids_for(multi_src);
        let n = input_ords.len() as u32;
        assert_eq!(n, 5, "expected five single-unit input batches");
        let max_input = *input_ords.iter().max().unwrap();
        assert_eq!(max_input, n, "input ordinals must be the contiguous 1..=N");

        let translator = RecordingRetryTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        run_pipeline(multi_src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes (units recover on retry)");

        let retry_ords = translator.retry_ords.into_inner().unwrap();
        assert!(
            !retry_ords.is_empty(),
            "the forced first-attempt failure must have triggered retries"
        );
        // Non-collision: every retry ordinal is strictly greater than the
        // largest input ordinal, so no retry id can equal an input id.
        for r in &retry_ords {
            assert!(
                *r > max_input,
                "retry ordinal {r} collides with input space 1..={max_input}"
            );
        }
        // Seeding rule pinned exactly: the smallest retry ordinal is N+1.
        // (Under the old fixed 10_000 seed this would be 10_000, not 6.)
        assert_eq!(
            *retry_ords.iter().min().unwrap(),
            n + 1,
            "retry sequence must be seeded at input_batch_count + 1"
        );

        // --- Single-batch edge: one input batch, first retry id is 2. ---
        let single_src = "only paragraph\n";
        assert_eq!(input_ids_for(single_src).len(), 1);
        let translator = RecordingRetryTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        run_pipeline(single_src, &opts, &translator, cache_dyn)
            .await
            .expect("single-batch pipeline completes");
        let retry_ords = translator.retry_ords.into_inner().unwrap();
        assert_eq!(
            retry_ords,
            vec![2],
            "single input batch (b-0001) must retry as b-0002"
        );

        // --- Empty edge: no batches, no retries, no panic on seeding. ---
        assert!(input_ids_for("").is_empty());
        let translator = RecordingRetryTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        run_pipeline("", &opts, &translator, cache_dyn)
            .await
            .expect("empty-source pipeline completes");
        assert!(
            translator.retry_ords.into_inner().unwrap().is_empty(),
            "empty document produces no retries"
        );
    }

    /// Twin-block cache collision (`cache.rs`: `CacheKey` carries no
    /// `BlockId`). Several byte-identical consecutive paragraphs share an
    /// identical key (source-hash + paragraph-kind + 120-char-truncated
    /// context), so a warm/shared cache holds ONE entry for the whole group,
    /// and its stored `UnitResult` keeps just one block's id. Before the fix
    /// the other twins' ids were absent from the synthetic cached result, so
    /// each was filtered out of the synthetic batch (schema mismatch) and then
    /// neither accepted nor re-queued — silently degrading to `FallbackSource`
    /// and losing its per-unit report row, nondeterministically. The fix
    /// rewrites the cached id to the requesting unit's id at hit time, so on a
    /// warm cache every twin is re-validated, accepted, reported, and the run
    /// is byte-deterministic across re-runs.
    ///
    /// Fails pre-fix: run 2 (warm) reports only three of the six units and
    /// splices source (no marker) for the three stranded twins.
    #[tokio::test]
    async fn twin_paragraphs_survive_warm_shared_cache() {
        use crate::cache::InMemoryCache;

        // Deterministic translator: translate every unit by prefixing a
        // marker. Identical source → identical output, so the twins' cached
        // payloads are interchangeable — which is exactly why rewriting the
        // cached id is sound: the payload re-validates against any twin's
        // (byte-identical) source.
        struct MarkTranslator;
        #[async_trait::async_trait]
        impl Translator for MarkTranslator {
            async fn translate_batch(
                &self,
                batch: TranslationBatch,
                _cancel: &crate::CancellationToken,
            ) -> Result<TranslationBatchResult, TranslatorError> {
                let units = batch
                    .units
                    .iter()
                    .map(|u| UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::Translated,
                        translated_payload: format!("translated {}", u.source_payload),
                        warnings: Vec::new(),
                    })
                    .collect();
                Ok(TranslationBatchResult {
                    batch_id: batch.batch_id,
                    detected_source_language: None,
                    units,
                })
            }
        }

        // One run's output must be complete: a per-unit row for EVERY block
        // (matching the document id set), nothing degraded to FallbackSource
        // in the report OR the alignment map, and the translated payload
        // (marker) present once per paragraph — never spliced source. Returns
        // (markdown, per-unit statuses) for the determinism comparison.
        fn check(
            out: &TranslationOutput,
            block_ids: &[BlockId],
        ) -> (String, Vec<(BlockId, FallbackStatus)>) {
            let mut reported: Vec<BlockId> = out
                .validation_report
                .per_unit
                .iter()
                .map(|r| r.unit_id.clone())
                .collect();
            reported.sort_by(|a, b| a.0.cmp(&b.0));
            let mut expected = block_ids.to_vec();
            expected.sort_by(|a, b| a.0.cmp(&b.0));
            assert_eq!(
                reported, expected,
                "every unit must keep its per_unit row (none stranded by a twin cache hit)"
            );

            assert!(
                out.validation_report
                    .per_unit
                    .iter()
                    .all(|r| !matches!(r.final_status, FallbackStatus::FallbackSource)),
                "no reported unit may fall back: {:?}",
                out.validation_report.per_unit
            );
            assert!(
                out.alignment_map
                    .blocks
                    .iter()
                    .all(|b| !matches!(b.fallback_status, FallbackStatus::FallbackSource)),
                "no alignment block may be FallbackSource"
            );
            assert_eq!(out.validation_report.total_fallbacks, 0, "zero fallbacks");

            assert_eq!(
                out.translated_document.matches("translated ").count(),
                block_ids.len(),
                "every paragraph must be translated in the output:\n{}",
                out.translated_document
            );

            let statuses = out
                .validation_report
                .per_unit
                .iter()
                .map(|r| (r.unit_id.clone(), r.final_status))
                .collect();
            (out.translated_document.clone(), statuses)
        }

        // Six byte-identical consecutive paragraphs. The four interior
        // paragraphs additionally share an identical context window (their
        // preceding + following neighbor summaries are the same identical
        // paragraph text), so all four collide on ONE key; the two ends differ
        // only by a `None` neighbor. => six units collapse to three keys.
        let para = "the quick brown fox jumps over the lazy dog";
        let src = format!("{para}\n\n{para}\n\n{para}\n\n{para}\n\n{para}\n\n{para}\n");

        let opts = crate::TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };

        // The document's six paragraph block ids, in source order — the exact
        // set every warm run must report and translate.
        let mut doc = parse(&src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let block_ids: Vec<BlockId> = doc.blocks.iter().map(|b| b.block_id.clone()).collect();
        assert_eq!(
            block_ids.len(),
            6,
            "fixture is six paragraphs: {block_ids:?}"
        );

        // One cache shared across every run (partial-resume / shared-cache
        // contract, SCN-10).
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;

        // Run 1 (cold) populates the cache and CREATES the collision: the six
        // units collapse to three keys, so a warm run gets, for five of six
        // lookups, a hit whose stored id belongs to a different block. The
        // `len() == 3` guard proves the fixture exercises the collision (not a
        // vacuous pass).
        let out1 = run_pipeline(&src, &opts, &MarkTranslator, cache_dyn)
            .await
            .expect("cold run completes");
        let r1 = check(&out1, &block_ids);
        assert_eq!(
            cache.len(),
            3,
            "twin collision: six paragraphs must collapse to three cache keys"
        );

        // Runs 2 and 3 (warm): every lookup is now a cross-id cache hit. Both
        // must still translate + report every block, and be byte- and
        // status-identical (deterministic across re-runs).
        let out2 = run_pipeline(&src, &opts, &MarkTranslator, cache_dyn)
            .await
            .expect("first warm run completes");
        let r2 = check(&out2, &block_ids);
        let out3 = run_pipeline(&src, &opts, &MarkTranslator, cache_dyn)
            .await
            .expect("second warm run completes");
        let r3 = check(&out3, &block_ids);

        assert_eq!(cache.len(), 3, "warm runs do not grow the key set");
        assert_eq!(r2, r3, "warm re-runs are deterministic");
        assert_eq!(
            r1, r2,
            "cold and warm runs agree byte-for-byte and status-for-status"
        );
    }
}

// R0001-0012: a retry round is packed by the SAME budget the first round was.
// Before this, `retry_units` became one batch however large — even though
// every re-dispatched unit now carries an ADR-0009 retry hint (a 512-byte
// reason prefix plus a truncation marker) that the original packing never
// counted. Both tests are
// stub-driven and offline.
#[cfg(test)]
mod retry_repacking_tests {
    use super::*;
    use crate::cache::InMemoryCache;
    use crate::pipeline::*;
    use std::sync::Mutex;

    /// Six short paragraphs: one input batch under a generous token target,
    /// and a population large enough that the retry hints split it.
    fn fixture_source() -> String {
        (1..=6)
            .map(|i| format!("paragraph number {i} of the retry fixture\n\n"))
            .collect()
    }

    /// What one dispatched batch looked like from the provider's side.
    #[derive(Debug)]
    struct SeenBatch {
        batch_id: String,
        unit_ids: Vec<String>,
        is_retry: bool,
        /// R0008-0020: every unit's own `batch_id` matched the enclosing
        /// batch's.
        units_stamped: bool,
        /// ADR-0009: the re-dispatched payload is the pristine source.
        payloads: Vec<String>,
    }

    /// Rejects every unit on its first dispatch (a provider-declared
    /// fallback — a retryable *unit* fault) and translates it on the
    /// re-dispatch, recording the shape of every batch it is handed.
    #[derive(Default)]
    struct RejectThenTranslate {
        seen: Mutex<Vec<SeenBatch>>,
    }

    #[async_trait::async_trait]
    impl Translator for RejectThenTranslate {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.seen.lock().unwrap().push(SeenBatch {
                batch_id: batch.batch_id.0.clone(),
                unit_ids: batch.units.iter().map(|u| u.unit_id.0.clone()).collect(),
                is_retry: batch.units.iter().any(|u| u.retry.is_some()),
                units_stamped: batch.units.iter().all(|u| u.batch_id == batch.batch_id),
                payloads: batch
                    .units
                    .iter()
                    .map(|u| u.source_payload.clone())
                    .collect(),
            });
            let units = batch
                .units
                .iter()
                .map(|u| {
                    if u.retry.is_some() {
                        UnitResult {
                            unit_id: u.unit_id.clone(),
                            output_kind: OutputKind::Translated,
                            translated_payload: format!("{} 번역", u.source_payload),
                            warnings: Vec::new(),
                        }
                    } else {
                        UnitResult {
                            unit_id: u.unit_id.clone(),
                            output_kind: OutputKind::FailedNeedsFallback,
                            translated_payload: String::new(),
                            warnings: Vec::new(),
                        }
                    }
                })
                .collect();
            Ok(TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
    }

    /// The token target that holds the fixture's FIRST round exactly and
    /// nothing more — the honest worst case for a batch packed to its budget.
    /// Derived from the packer's own estimator so the test states a fact
    /// about the budget rather than a hand-tuned magic number.
    fn target_that_exactly_holds_the_first_round(src: &str) -> u32 {
        let probe_opts = TranslateOptions {
            target_language: "ko".to_string(),
            target_input_tokens_per_batch: 1_000_000,
            ..Default::default()
        };
        let mut doc = parse(src).expect("source parses");
        id::assign_block_ids(&mut doc);
        let probe = build_batches(&doc, &probe_opts, None, &crate::unit::html_outcomes(&doc));
        assert_eq!(probe.len(), 1, "the fixture must be one input batch");
        assert_eq!(probe[0].units.len(), 6, "the fixture is six units");

        let bpe = crate::batch::resolve_encoder(None, &probe_opts.model_id);
        // ti aa92d6: the same envelope `build_batches` reserved — the fixture
        // is html-free, so the html clause is off on both sides.
        let envelope = crate::llm::prompt::instruction_envelope_json(
            crate::llm::prompt::InstructionVariant::for_run(
                &probe[0].profile.constraints,
                crate::llm::prompt::DocumentFacts::default(),
            ),
        );
        let mut budget = crate::unit::budget::resolve(
            &probe_opts,
            &probe[0].profile,
            None,
            &probe[0].profile.prompt_body,
            &envelope,
        );
        budget.source_language = &probe[0].source_language;
        budget.target_language = &probe[0].target_language;
        let total = crate::batch::envelope_input_tokens(&budget, &bpe)
            + probe[0]
                .units
                .iter()
                .map(|u| crate::batch::estimate_unit_tokens(u, &bpe))
                .sum::<usize>();
        u32::try_from(total).expect("the fixture's budget fits a u32")
    }

    fn opts_with_target(target: u32) -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            target_input_tokens_per_batch: target,
            ..Default::default()
        }
    }

    /// Every unit recovers on its re-dispatch, whichever retry batch carried
    /// it — the split must not cost a single unit its translation.
    fn assert_every_unit_recovered(out: &TranslationOutput) {
        assert_eq!(out.validation_report.per_unit.len(), 6);
        for r in &out.validation_report.per_unit {
            assert_eq!(r.final_status, FallbackStatus::Translated, "{r:?}");
            assert_eq!(r.attempts.len(), 2, "one rejection, one recovery: {r:?}");
        }
        assert_eq!(out.validation_report.total_fallbacks, 0);
        assert_eq!(out.validation_report.total_retries, 6);
        assert_eq!(out.translated_document.matches("번역").count(), 6);
    }

    /// THE discriminating test. Pre-fix the retry round shipped as one batch
    /// carrying six retry hints the budget never accounted for; now it is
    /// re-packed and splits.
    #[tokio::test]
    async fn a_retry_round_is_repacked_and_splits_when_the_hints_overflow() {
        let src = fixture_source();
        let opts = opts_with_target(target_that_exactly_holds_the_first_round(&src));
        let translator = RejectThenTranslate::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(&src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let seen = translator.seen.into_inner().unwrap();
        let (first, retries): (Vec<&SeenBatch>, Vec<&SeenBatch>) =
            seen.iter().partition(|s| !s.is_retry);

        assert_eq!(first.len(), 1, "one input batch, dispatched once: {seen:?}");
        assert_eq!(first[0].unit_ids.len(), 6);
        assert!(
            retries.len() > 1,
            "the retry round must be re-packed into several batches, got {}: {seen:?}",
            retries.len()
        );

        // Together the retry batches cover the whole retry population exactly
        // once, in document order — a split is a regrouping, not a filter.
        let retried_ids: Vec<&String> = retries.iter().flat_map(|s| s.unit_ids.iter()).collect();
        assert_eq!(retried_ids, first[0].unit_ids.iter().collect::<Vec<_>>());
        assert!(
            retries.iter().all(|s| !s.unit_ids.is_empty()),
            "no empty retry batch: {seen:?}"
        );

        // R0008-0020 for EVERY produced retry batch, not just the first: each
        // gets its own id and stamps it on its own units.
        let mut retry_ids: Vec<&String> = retries.iter().map(|s| &s.batch_id).collect();
        let produced = retry_ids.len();
        retry_ids.sort();
        retry_ids.dedup();
        assert_eq!(
            retry_ids.len(),
            produced,
            "each retry batch must get its own id: {seen:?}"
        );
        assert!(
            seen.iter().all(|s| s.units_stamped),
            "every batch's units must carry the enclosing batch id: {seen:?}"
        );

        // ADR-0009: re-packing regroups requests; it never touches a payload.
        let sent: Vec<&String> = retries.iter().flat_map(|s| s.payloads.iter()).collect();
        assert_eq!(
            sent,
            first[0].payloads.iter().collect::<Vec<_>>(),
            "resubmission stays verbatim: same payloads, same scope"
        );

        assert_every_unit_recovered(&out);
    }

    /// The contrast that proves the split above was the *budget* talking: with
    /// room to spare, the same run re-dispatches the same six units as one
    /// retry batch, exactly as before this ticket.
    #[tokio::test]
    async fn a_retry_round_that_still_fits_stays_one_batch() {
        let src = fixture_source();
        let opts = opts_with_target(1_000_000);
        let translator = RejectThenTranslate::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(&src, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let seen = translator.seen.into_inner().unwrap();
        assert_eq!(
            seen.len(),
            2,
            "one input dispatch and one retry dispatch: {seen:?}"
        );
        assert!(!seen[0].is_retry && seen[1].is_retry);
        assert_eq!(seen[1].unit_ids, seen[0].unit_ids);
        assert!(seen.iter().all(|s| s.units_stamped));
        assert_every_unit_recovered(&out);
    }

    /// Rejects every unit on its first dispatch (so the round splits, exactly
    /// as above), answers the FIRST retry sub-batch, and ends the run on the
    /// second.
    #[derive(Default)]
    struct AnswerThenFailTranslator {
        /// The unit ids of the retry sub-batch this translator answered.
        answered: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl Translator for AnswerThenFailTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            if batch.units.iter().any(|u| u.retry.is_some()) {
                let mut answered = self.answered.lock().unwrap();
                if !answered.is_empty() {
                    return Err(TranslatorError::Authentication(
                        "the second retry sub-batch never lands".to_string(),
                    ));
                }
                *answered = batch.units.iter().map(|u| u.unit_id.0.clone()).collect();
            }
            let units = batch
                .units
                .iter()
                .map(|u| {
                    if u.retry.is_some() {
                        UnitResult {
                            unit_id: u.unit_id.clone(),
                            output_kind: OutputKind::Translated,
                            translated_payload: format!("{} 번역", u.source_payload),
                            warnings: Vec::new(),
                        }
                    } else {
                        UnitResult {
                            unit_id: u.unit_id.clone(),
                            output_kind: OutputKind::FailedNeedsFallback,
                            translated_payload: String::new(),
                            warnings: Vec::new(),
                        }
                    }
                })
                .collect();
            Ok(TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
    }

    /// R0010-0006. A terminal error in a LATER sub-batch of a round must not
    /// throw away what the EARLIER ones already earned. ADR-0017 still ends
    /// the run — the error is the answer, not a half-document — but the units
    /// the answered sub-batch translated survive in the cache, so the resumed
    /// run buys them from the provider once rather than twice. Pre-fix the
    /// `?` on the failing sub-batch exited before the per-unit walk that
    /// writes them, and the cache came out empty.
    #[tokio::test]
    async fn a_failing_later_retry_batch_keeps_the_earlier_ones_progress() {
        let src = fixture_source();
        let opts = opts_with_target(target_that_exactly_holds_the_first_round(&src));
        let translator = AnswerThenFailTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let err = run_pipeline(&src, &opts, &translator, cache_dyn)
            .await
            .expect_err("the failing retry sub-batch ends the run");
        assert!(
            matches!(
                err,
                TransyncError::Translator(TranslatorError::Authentication(_))
            ),
            "the provider's terminal error is the answer: {err:?}"
        );

        let answered = translator.answered.into_inner().unwrap();
        assert!(
            !answered.is_empty(),
            "the retry round must have split, with its first sub-batch answered"
        );
        assert_eq!(
            cache.len(),
            answered.len(),
            "exactly the answered sub-batch's units are cached"
        );
    }
}

// OI-0031: batch-envelope faults vs per-unit content faults. Every test is
// stub-driven and offline. What they pin: a provider that mangles ONE row
// must not drag its batch-mates through the retry ladder into fallback
// (`batch_fault_spares_innocent_units` fails on the pre-OI-0031 code, where
// a schema fault blanket-rejected the whole batch), the two budgets drain
// independently, and a provider that never returns a usable envelope still
// terminates inside the per-batch bound.
#[cfg(test)]
mod batch_fault_tests {
    use super::*;
    // The moved tests keep the exact scope they had in `pipeline.rs`.
    use crate::cache::InMemoryCache;
    use crate::llm::RetryContext;
    use crate::pipeline::*;
    use crate::validate::{UnitValidationRecord, ValidationLayer, ValidationReport};
    use std::sync::Mutex;

    /// Three paragraphs → ids p-0001..p-0003, one batch under the default
    /// packing knobs. `p-0001` is the lexicographically first id, i.e. the
    /// one the sabotaging stubs single out.
    const SRC: &str = "alpha paragraph\n\nbravo paragraph\n\ncharlie paragraph\n";
    const OFFENDER: &str = "p-0001";

    fn opts() -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        }
    }

    /// A well-formed translation of one unit. No brackets or parens, so the
    /// inline layer sees the same (zero) link count on both sides.
    fn translated(u: &TranslationUnit) -> UnitResult {
        UnitResult {
            unit_id: u.unit_id.clone(),
            output_kind: OutputKind::Translated,
            translated_payload: format!("{} 번역", u.source_payload),
            warnings: Vec::new(),
        }
    }

    fn failed(u: &TranslationUnit) -> UnitResult {
        UnitResult {
            unit_id: u.unit_id.clone(),
            output_kind: OutputKind::FailedNeedsFallback,
            translated_payload: String::new(),
            warnings: Vec::new(),
        }
    }

    fn ok(batch: TranslationBatch, units: Vec<UnitResult>) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        }
    }

    /// As [`ok`], but carrying an envelope-level `detected_source_language`
    /// so a round's language can be traced to the round that reported it.
    fn ok_lang(
        batch: TranslationBatch,
        units: Vec<UnitResult>,
        lang: &str,
    ) -> TranslationBatchResult {
        TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: Some(lang.to_string()),
            units,
        }
    }

    /// The lexicographically first requested id — stable across retry
    /// rounds, where the offender may be the only unit left.
    fn first_id(batch: &TranslationBatch) -> String {
        batch
            .units
            .iter()
            .map(|u| u.unit_id.0.clone())
            .min()
            .expect("batch is never empty")
    }

    fn record<'a>(out: &'a TranslationOutput, id: &str) -> &'a UnitValidationRecord {
        out.validation_report
            .per_unit
            .iter()
            .find(|r| r.unit_id.0 == id)
            .unwrap_or_else(|| panic!("no report row for {id}"))
    }

    fn status_in_map(out: &TranslationOutput, id: &str) -> FallbackStatus {
        out.alignment_map
            .blocks
            .iter()
            .find(|b| b.source_block_id.0 == id)
            .unwrap_or_else(|| panic!("no alignment block for {id}"))
            .fallback_status
    }

    /// Drops the lexicographically first requested unit's result row on
    /// every round and returns the rest correctly — the OI's "provider
    /// deterministically drops one unit" failure mode.
    #[derive(Default)]
    struct DropOneTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for DropOneTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let dropped = first_id(&batch);
            let units = batch
                .units
                .iter()
                .filter(|u| u.unit_id.0 != dropped)
                .map(translated)
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 1 — THE discriminating test. Pre-OI-0031, one dropped row
    /// blanket-rejected the batch: every sibling was re-dispatched through
    /// the whole per-unit ladder and finalized as `fallback_source`. Now the
    /// innocents are accepted in round 1 and only the dropped unit pays.
    #[tokio::test]
    async fn batch_fault_spares_innocent_units() {
        let opts = opts();
        let translator = DropOneTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("a schema fault is a validation event, never an abort");

        for id in ["p-0002", "p-0003"] {
            let r = record(&out, id);
            assert_eq!(
                r.final_status,
                FallbackStatus::Translated,
                "innocent {id} must keep its translation: {r:?}"
            );
            assert_eq!(status_in_map(&out, id), FallbackStatus::Translated);
            assert_eq!(
                r.attempts.len(),
                1,
                "innocent {id} must not be re-dispatched: {:?}",
                r.attempts
            );
            assert_eq!(r.attempts[0].attempt_number, 1);
            assert_eq!(r.attempts[0].rejected_by, None);
            assert!(!r.attempts[0].batch_fault);
            assert!(
                out.translated_document.contains(&format!(
                    "{} 번역",
                    if id == "p-0002" {
                        "bravo paragraph"
                    } else {
                        "charlie paragraph"
                    }
                )),
                "innocent payload must reach the output:\n{}",
                out.translated_document
            );
        }

        // The offender: one row per dispatch, every one attributed to the
        // batch envelope, ending in an honest fallback.
        let r = record(&out, OFFENDER);
        assert_eq!(r.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            status_in_map(&out, OFFENDER),
            FallbackStatus::FallbackSource
        );
        assert_eq!(
            r.attempts.len() as u32,
            1 + opts.max_per_batch_schema_retries,
            "offender rounds = initial dispatch + the per-batch budget: {:?}",
            r.attempts
        );
        for (i, a) in r.attempts.iter().enumerate() {
            assert_eq!(a.attempt_number, i as u32 + 1, "dispatch ordinals");
            assert!(a.batch_fault, "row {i} must be attributed to the batch");
            assert_eq!(a.rejected_by, Some(ValidationLayer::Schema));
            assert!(
                a.rejection_reason
                    .as_deref()
                    .unwrap_or_default()
                    .contains("missing from provider result"),
                "reason: {:?}",
                a.rejection_reason
            );
        }
        assert!(
            out.translated_document.contains("alpha paragraph")
                && !out.translated_document.contains("alpha paragraph 번역"),
            "the dropped unit falls back to source bytes:\n{}",
            out.translated_document
        );

        assert_eq!(
            out.validation_report.batch_schema_faults, opts.max_per_batch_schema_retries,
            "one charge per re-dispatched round"
        );
        assert_eq!(
            out.validation_report.total_fallbacks, 1,
            "exactly one unit falls back — the clustered-fallback bug is gone"
        );
        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            1 + opts.max_per_batch_schema_retries,
            "no round beyond the batch budget"
        );
    }

    /// Returns `Ok` with an empty unit list every round: every requested
    /// unit is an offender, forever.
    #[derive(Default)]
    struct EmptyResultTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for EmptyResultTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(ok(batch, Vec::new()))
        }
    }

    /// §B.6 test 2 — the termination guarantee. A provider that never
    /// returns a usable envelope must stop at the per-batch bound (one
    /// charge per round, not per offender) with every unit honestly marked.
    #[tokio::test]
    async fn fully_hostile_empty_result_terminates() {
        let opts = opts();
        let translator = EmptyResultTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("a hostile envelope degrades, it does not abort");

        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            1 + opts.max_per_batch_schema_retries,
            "an empty response costs ONE budget point per round, not one per unit"
        );
        assert_eq!(out.validation_report.per_unit.len(), 3);
        for r in &out.validation_report.per_unit {
            assert_eq!(
                r.final_status,
                FallbackStatus::FallbackSource,
                "{:?} must fall back honestly",
                r.unit_id
            );
            assert_eq!(
                r.attempts.len() as u32,
                1 + opts.max_per_batch_schema_retries
            );
            assert!(r.attempts.iter().all(|a| a.batch_fault));
        }
        assert_eq!(out.validation_report.total_fallbacks, 3);
        assert_eq!(
            out.validation_report.batch_schema_faults, opts.max_per_batch_schema_retries,
            "charged per round, so 3 offenders × 2 rounds is still 2"
        );
        assert_eq!(
            out.translated_document, SRC,
            "every unit fell back, so the output is the source"
        );
    }

    /// Returns the lexicographically first unit twice (with different
    /// payloads) and the rest correctly.
    #[derive(Default)]
    struct DuplicateOneTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for DuplicateOneTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let dup = first_id(&batch);
            let mut units: Vec<UnitResult> = Vec::new();
            for u in &batch.units {
                units.push(translated(u));
                if u.unit_id.0 == dup {
                    let mut second = translated(u);
                    second.translated_payload = format!("{} 다른 번역", u.source_payload);
                    units.push(second);
                }
            }
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 3 — a duplicated row is charged to its own unit only, and
    /// both copies are discarded (no coin flip on content).
    #[tokio::test]
    async fn duplicate_result_id_charges_only_the_duplicated_unit() {
        let opts = opts();
        let translator = DuplicateOneTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        for id in ["p-0002", "p-0003"] {
            let r = record(&out, id);
            assert_eq!(r.final_status, FallbackStatus::Translated);
            assert_eq!(r.attempts.len(), 1, "{id}: {:?}", r.attempts);
        }
        let r = record(&out, OFFENDER);
        assert_eq!(r.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            r.attempts.len() as u32,
            1 + opts.max_per_batch_schema_retries
        );
        assert!(r.attempts.iter().all(|a| a.batch_fault));
        assert!(
            r.attempts[0]
                .rejection_reason
                .as_deref()
                .unwrap_or_default()
                .contains("duplicated in provider result"),
            "reason: {:?}",
            r.attempts[0].rejection_reason
        );
        assert!(
            !out.translated_document.contains("다른 번역"),
            "neither copy of an ambiguous row may be spliced:\n{}",
            out.translated_document
        );
        assert_eq!(
            out.validation_report.batch_schema_faults,
            opts.max_per_batch_schema_retries
        );
    }

    /// Answers every requested unit correctly and volunteers one id that
    /// was never asked for.
    #[derive(Default)]
    struct ForeignIdTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for ForeignIdTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let mut units: Vec<UnitResult> = batch.units.iter().map(translated).collect();
            units.push(UnitResult {
                unit_id: BlockId("zz-9999".to_string()),
                output_kind: OutputKind::Translated,
                translated_payload: "유령 블록".to_string(),
                warnings: Vec::new(),
            });
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 4 — an invented id implicates no requested unit: it is
    /// discarded, nobody is charged, and no round is spent.
    #[tokio::test]
    async fn foreign_ids_are_discarded_without_charges() {
        let opts = opts();
        let translator = ForeignIdTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            1,
            "a foreign row alone must not trigger a retry round"
        );
        assert_eq!(out.validation_report.per_unit.len(), 3);
        for r in &out.validation_report.per_unit {
            assert_eq!(r.final_status, FallbackStatus::Translated, "{r:?}");
            assert_eq!(r.attempts.len(), 1);
            assert!(!r.attempts[0].batch_fault);
        }
        assert_eq!(out.validation_report.batch_schema_faults, 0);
        assert_eq!(out.validation_report.total_fallbacks, 0);
        assert!(
            !out.translated_document.contains("유령"),
            "an unrequested payload must never be spliced:\n{}",
            out.translated_document
        );
    }

    /// Drops p-0001 every round, fails p-0002 at the provider layer every
    /// round, translates p-0003 correctly.
    #[derive(Default)]
    struct MixedFaultTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for MixedFaultTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let units = batch
                .units
                .iter()
                .filter(|u| u.unit_id.0 != "p-0001")
                .map(|u| {
                    if u.unit_id.0 == "p-0002" {
                        failed(u)
                    } else {
                        translated(u)
                    }
                })
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 5 — the budgets drain independently: neither population's
    /// round count is inflated by the other's.
    #[tokio::test]
    async fn mixed_batch_and_unit_faults_drain_separate_budgets() {
        let opts = opts();
        let translator = MixedFaultTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let c = record(&out, "p-0003");
        assert_eq!(c.final_status, FallbackStatus::Translated);
        assert_eq!(c.attempts.len(), 1, "the healthy unit settles in round 1");

        let a = record(&out, OFFENDER);
        assert_eq!(a.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            a.attempts.len() as u32,
            1 + opts.max_per_batch_schema_retries,
            "the dropped unit spends the BATCH budget: {:?}",
            a.attempts
        );
        assert!(a.attempts.iter().all(|r| r.batch_fault));

        let b = record(&out, "p-0002");
        assert_eq!(b.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            b.attempts.len() as u32,
            1 + opts.max_per_unit_validation_retries,
            "the mistranslated unit spends its OWN budget: {:?}",
            b.attempts
        );
        assert!(b.attempts.iter().all(|r| !r.batch_fault));
        assert!(
            b.attempts
                .iter()
                .all(|r| r.rejected_by == Some(ValidationLayer::Provider))
        );

        // Dispatch ordinals stay consecutive from 1 for each population.
        for r in [a, b] {
            let ordinals: Vec<u32> = r.attempts.iter().map(|x| x.attempt_number).collect();
            let expected: Vec<u32> = (1..=r.attempts.len() as u32).collect();
            assert_eq!(ordinals, expected, "{:?}", r.unit_id);
        }
        assert_eq!(
            out.validation_report.batch_schema_faults,
            opts.max_per_batch_schema_retries
        );
    }

    /// Drops p-0001 on the FIRST round only; from then on p-0001 comes back
    /// as a provider-layer failure and its siblings translate cleanly.
    #[derive(Default)]
    struct DropThenFailTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for DropThenFailTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let units = batch
                .units
                .iter()
                .filter(|u| !(round == 1 && u.unit_id.0 == OFFENDER))
                .map(|u| {
                    if u.unit_id.0 == OFFENDER {
                        failed(u)
                    } else {
                        translated(u)
                    }
                })
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 6 — the point of the separation: being dropped must not
    /// cost the unit any of its content-retry budget.
    #[tokio::test]
    async fn offender_keeps_full_unit_budget_after_batch_fault() {
        let opts = opts();
        let translator = DropThenFailTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        for id in ["p-0002", "p-0003"] {
            let r = record(&out, id);
            assert_eq!(r.final_status, FallbackStatus::Translated);
            assert_eq!(r.attempts.len(), 1, "{id}: {:?}", r.attempts);
        }

        let a = record(&out, OFFENDER);
        assert_eq!(
            a.attempts.len() as u32,
            1 + (1 + opts.max_per_unit_validation_retries),
            "one batch-fault round PLUS the untouched content budget: {:?}",
            a.attempts
        );
        assert!(a.attempts[0].batch_fault, "round 1 was an envelope fault");
        assert!(
            a.attempts[1..]
                .iter()
                .all(|r| !r.batch_fault && r.rejected_by == Some(ValidationLayer::Provider)),
            "later rounds are the unit's own faults: {:?}",
            a.attempts
        );
        assert_eq!(a.final_status, FallbackStatus::FallbackSource);
        assert_eq!(
            out.validation_report.batch_schema_faults, 1,
            "only the first round was charged to the batch budget"
        );
    }

    /// Drops p-0001 on the first round, then translates everything —
    /// recording the `RetryContext` each unit arrived with.
    #[derive(Default)]
    struct RecoveringOffenderTranslator {
        calls: AtomicU32,
        seen_retries: Mutex<Vec<(String, RetryContext)>>,
    }
    #[async_trait::async_trait]
    impl Translator for RecoveringOffenderTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            for u in &batch.units {
                if let Some(ctx) = &u.retry {
                    self.seen_retries
                        .lock()
                        .unwrap()
                        .push((u.unit_id.0.clone(), ctx.clone()));
                }
            }
            let units = batch
                .units
                .iter()
                .filter(|u| !(round == 1 && u.unit_id.0 == OFFENDER))
                .map(translated)
                .collect();
            Ok(ok(batch, units))
        }
    }

    /// §B.6 test 7 — a dropped unit that comes back ends `Translated`, and
    /// the re-dispatch carries the ADR-0009 side channel naming the Schema
    /// layer (pristine payload, non-content hint).
    #[tokio::test]
    async fn recovering_offender_ends_translated() {
        let opts = opts();
        let translator = RecoveringOffenderTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let a = record(&out, OFFENDER);
        assert_eq!(a.final_status, FallbackStatus::Translated);
        assert_eq!(a.attempts.len(), 2, "{:?}", a.attempts);
        assert!(a.attempts[0].batch_fault);
        assert_eq!(a.attempts[1].rejected_by, None);
        assert!(!a.attempts[1].batch_fault);
        assert!(
            out.translated_document.contains("alpha paragraph 번역"),
            "the recovered translation must be spliced:\n{}",
            out.translated_document
        );
        assert_eq!(out.validation_report.batch_schema_faults, 1);
        assert_eq!(out.validation_report.total_fallbacks, 0);

        let seen = translator.seen_retries.lock().unwrap();
        assert_eq!(
            seen.len(),
            1,
            "only the offender was re-dispatched: {seen:?}"
        );
        let (id, ctx) = &seen[0];
        assert_eq!(id, OFFENDER);
        assert_eq!(ctx.attempt, 2);
        assert_eq!(
            ctx.rejected_by,
            Some(ValidationLayer::Schema),
            "the side channel must name the layer that rejected it"
        );
        assert!(
            ctx.reason
                .as_deref()
                .unwrap_or_default()
                .contains("missing from provider result"),
            "reason: {:?}",
            ctx.reason
        );
    }

    /// Drops the first requested row on round 1 while reporting "de", then
    /// returns everything on round 2 reporting "en". Round 1's envelope
    /// therefore carries a batch schema fault and round 2's does not.
    #[derive(Default)]
    struct FaultThenRecoverLanguageTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for FaultThenRecoverLanguageTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let dropped = first_id(&batch);
            let units = batch
                .units
                .iter()
                .filter(|u| !(round == 1 && u.unit_id.0 == dropped))
                .map(translated)
                .collect();
            let lang = if round == 1 { "de" } else { "en" };
            Ok(ok_lang(batch, units, lang))
        }
    }

    /// R0001-0007 — a round whose envelope was rejected must not decide the
    /// published language. Round 1 says "de" and is thrown away; the
    /// accepted round 2 says "en", and "en" is what the run reports.
    #[tokio::test]
    async fn a_rejected_round_does_not_decide_the_reported_language() {
        let opts = opts();
        let translator = FaultThenRecoverLanguageTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        assert_eq!(
            out.detected_source_language.as_deref(),
            Some("en"),
            "the language must come from the accepted round, not the discarded one"
        );
        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            2,
            "exactly one faulted round and one accepted round"
        );
        assert_eq!(out.validation_report.batch_schema_faults, 1);
        assert_eq!(out.validation_report.total_fallbacks, 0);
    }

    /// Returns every requested row on every round, but has the provider opt
    /// the first unit out on round 1 (a per-unit `Provider` rejection, not
    /// an envelope fault) while reporting "en". Round 2 translates the
    /// retried unit and reports "de".
    #[derive(Default)]
    struct UnitFaultLanguageTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for UnitFaultLanguageTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let round = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let opted_out = first_id(&batch);
            let units = batch
                .units
                .iter()
                .map(|u| {
                    if round == 1 && u.unit_id.0 == opted_out {
                        failed(u)
                    } else {
                        translated(u)
                    }
                })
                .collect();
            let lang = if round == 1 { "en" } else { "de" };
            Ok(ok_lang(batch, units, lang))
        }
    }

    /// R0001-0007, the other half of the rule — a *per-unit* rejection does
    /// not disqualify the round's detection. Round 1's envelope was sound
    /// and observed the whole batch, so its "en" stands even though the one
    /// retried unit came back from a round that said "de".
    #[tokio::test]
    async fn a_per_unit_rejection_does_not_disqualify_the_rounds_language() {
        let opts = opts();
        let translator = UnitFaultLanguageTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        assert_eq!(
            out.detected_source_language.as_deref(),
            Some("en"),
            "a clean envelope commits its language; one opted-out unit is not an envelope fault"
        );
        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            2,
            "the opted-out unit was retried once"
        );
        assert_eq!(
            out.validation_report.batch_schema_faults, 0,
            "no envelope was ever malformed"
        );
        assert_eq!(out.validation_report.total_fallbacks, 0);
        assert_eq!(
            record(&out, OFFENDER).final_status,
            FallbackStatus::Translated
        );
    }

    /// Drops the first requested row on *every* round while always
    /// reporting "de" — no envelope in the run ever qualifies.
    #[derive(Default)]
    struct AlwaysFaultingLanguageTranslator {
        calls: AtomicU32,
    }
    #[async_trait::async_trait]
    impl Translator for AlwaysFaultingLanguageTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let dropped = first_id(&batch);
            let units = batch
                .units
                .iter()
                .filter(|u| u.unit_id.0 != dropped)
                .map(translated)
                .collect();
            Ok(ok_lang(batch, units, "de"))
        }
    }

    /// R0001-0007 — when no round's envelope survives, the run reports no
    /// language at all rather than one lifted from a discarded envelope.
    /// `detected_source_language` is optional by construction, so "unknown"
    /// is an honest answer and a wrong language is not.
    #[tokio::test]
    async fn no_qualifying_round_reports_no_language() {
        let opts = opts();
        let translator = AlwaysFaultingLanguageTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("a schema fault is a validation event, never an abort");

        assert_eq!(
            out.detected_source_language, None,
            "every round faulted, so nothing was entitled to set the language"
        );
        assert_eq!(
            out.validation_report.batch_schema_faults, opts.max_per_batch_schema_retries,
            "one charge per re-dispatched round"
        );
        assert_eq!(out.validation_report.total_fallbacks, 1);
    }

    /// Reports a megabyte-long "language" on a clean envelope — the one
    /// provider-authored envelope field that reaches the caller's output, the
    /// alignment map and the published bundle.
    #[derive(Default)]
    struct HugeLanguageTranslator;
    #[async_trait::async_trait]
    impl Translator for HugeLanguageTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let units = batch.units.iter().map(translated).collect();
            Ok(ok_lang(batch, units, &"en".repeat(600_000)))
        }
    }

    /// R0003-0038 — `detected_source_language` is provider-authored text and
    /// is bounded like every other provider-authored string the run keeps. A
    /// hostile or broken provider names a language; it does not get to decide
    /// how large `TranslationOutput`, the alignment map and the HTML bundle's
    /// `lang` attribute are.
    #[tokio::test]
    async fn a_giant_detected_language_is_bounded_before_it_is_kept() {
        let opts = opts();
        let translator = HugeLanguageTranslator;
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let out = run_pipeline(SRC, &opts, &translator, cache_dyn)
            .await
            .expect("pipeline completes");

        let lang = out
            .detected_source_language
            .as_deref()
            .expect("a clean envelope commits its language");
        assert!(
            lang.len() < 1_024,
            "the kept language must be bounded, got {} bytes",
            lang.len()
        );
        assert!(
            lang.starts_with("enen"),
            "the bound is a truncation, not a discard: {}",
            &lang[..lang.len().min(32)]
        );
        assert_eq!(
            out.alignment_map.detected_source_language.as_deref(),
            Some(lang),
            "the alignment map carries exactly what the run kept"
        );
    }

    /// §B.6 test 8 (pipeline half) — a caller-built batch with a duplicated
    /// request id aborts loudly instead of silently draining every budget
    /// and falling everyone back. Unreachable through `run_pipeline`, whose
    /// batcher derives ids from distinct blocks.
    #[tokio::test]
    async fn request_duplicate_ids_abort_loudly() {
        let opts = opts();
        let mut doc = parse("alpha paragraph\n").expect("source parses");
        id::assign_block_ids(&mut doc);
        let mut batch = build_batches(&doc, &opts, None, &crate::unit::html_outcomes(&doc))
            .into_iter()
            .next()
            .expect("one batch");
        let dup = batch.units[0].clone();
        batch.units.push(dup);

        let translator = DropOneTranslator::default();
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;
        let key_ctx = CacheKeyContext::for_run(&opts, translator.fingerprint());
        let outcome = process_one_batch(
            0,
            batch,
            &opts,
            &translator,
            cache_dyn,
            &AtomicU32::new(99),
            &key_ctx,
            "",
            crate::llm::prompt::DocumentFacts::default(),
            &CancellationToken::new(),
        )
        .await;
        let err = match outcome {
            Ok(_) => panic!("a malformed request must not be dispatched"),
            Err(e) => e,
        };

        match &err {
            TransyncError::Validation(m) => {
                assert!(m.contains("duplicate unit_id in request batch"), "{m}");
                assert!(m.contains(OFFENDER), "the offending id is named: {m}");
            }
            other => panic!("expected a Validation error, got {other:?}"),
        }
        assert_eq!(
            translator.calls.load(Ordering::SeqCst),
            0,
            "the preflight runs before any provider call"
        );
    }

    /// §B.6 test 10 — wire-compat pin: `batch_fault: false` must not appear
    /// in the serialized attempt row, so pre-OI-0031 report JSON is
    /// byte-identical until a batch fault actually happens.
    #[test]
    fn attempt_outcome_batch_fault_is_skipped_when_false() {
        let clean = AttemptOutcome {
            attempt_number: 1,
            rejected_by: None,
            rejection_reason: None,
            batch_fault: false,
        };
        let v = serde_json::to_value(&clean).expect("serializes");
        assert!(
            v.get("batch_fault").is_none(),
            "a clean row must not gain a key: {v}"
        );

        let faulted = AttemptOutcome {
            batch_fault: true,
            ..clean
        };
        let v = serde_json::to_value(&faulted).expect("serializes");
        assert_eq!(v["batch_fault"], serde_json::Value::Bool(true));
    }

    /// The report counter is additive and absent-by-default-shaped: a run
    /// with no batch fault reports 0 (and existing consumers keep reading
    /// the same numbers).
    #[test]
    fn clean_report_counts_no_batch_schema_faults() {
        let report = ValidationReport::default();
        let v = serde_json::to_value(&report).expect("serializes");
        assert_eq!(v["batch_schema_faults"], 0);
    }
}

// ti 294dda: `max_per_batch_provider_retries` is charged to the whole input
// batch, not refunded at each dispatch round. These are the only tests in the
// workspace that drive a transient `TranslatorError` out of `translate_batch`
// end-to-end, so they also cover the transport path itself: the transient
// predicate, the charge, the `Retry-After` sleep, and the terminal surfacing
// once the budget is spent.
#[cfg(test)]
mod provider_retry_budget_tests {
    use super::*;
    use crate::cache::InMemoryCache;
    use crate::pipeline::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use std::time::Duration;

    /// One paragraph → one unit in one batch, so every round below draws on
    /// the same batch's budget.
    const SRC: &str = "alpha paragraph\n";

    fn opts(provider_retries: u32) -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            max_per_batch_provider_retries: provider_retries,
            ..Default::default()
        }
    }

    /// What the scripted provider does on one `translate_batch` call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Step {
        /// A transient transport failure. `retry_after` is 1 ms — it wins
        /// over the exponential schedule (capped at 30 s), which keeps the
        /// pipeline's real sleep negligible without an injectable clock
        /// (explicitly out of scope for this ticket) and charges the budget
        /// on exactly the same path.
        Transient,
        /// A provider-declared per-unit fallback: a *validation*-layer
        /// rejection that is retryable, which is what opens the next
        /// dispatch round.
        RejectUnit,
        /// A clean translation of every requested unit.
        Translate,
    }

    /// Plays a fixed script back, one step per call. Running off the end
    /// panics on purpose: an overrun is precisely the pre-fix behavior (a
    /// fresh transient budget every round), so the panic names the
    /// regression instead of letting the run quietly succeed.
    struct ScriptedTranslator {
        script: Mutex<VecDeque<Step>>,
        calls: AtomicU32,
    }

    impl ScriptedTranslator {
        fn new(script: impl IntoIterator<Item = Step>) -> Self {
            Self {
                script: Mutex::new(script.into_iter().collect()),
                calls: AtomicU32::new(0),
            }
        }

        fn calls(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }

        fn remaining(&self) -> usize {
            self.script.lock().unwrap().len()
        }
    }

    #[async_trait::async_trait]
    impl Translator for ScriptedTranslator {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let step = self
                .script
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| panic!("provider call {n} is past the script: the batch got more transient retries than its budget"));
            let out = |kind: OutputKind, payload: fn(&TranslationUnit) -> String| {
                TranslationBatchResult {
                    batch_id: batch.batch_id.clone(),
                    detected_source_language: None,
                    units: batch
                        .units
                        .iter()
                        .map(|u| UnitResult {
                            unit_id: u.unit_id.clone(),
                            output_kind: kind,
                            translated_payload: payload(u),
                            warnings: Vec::new(),
                        })
                        .collect(),
                }
            };
            match step {
                Step::Transient => Err(TranslatorError::RateLimited {
                    retry_after: Some(Duration::from_millis(1)),
                }),
                Step::RejectUnit => Ok(out(OutputKind::FailedNeedsFallback, |_| String::new())),
                Step::Translate => Ok(out(OutputKind::Translated, |u| {
                    format!("{} 번역", u.source_payload)
                })),
            }
        }
    }

    // THE discriminating test. Budget 2, spent one round at a time: round 1
    // charges the first, so round 2 has exactly one left and its *second*
    // transient error is terminal. Under the pre-fix per-round charging,
    // round 2 would have started from a fresh 2 and the run would have gone
    // on — one call past the script.
    #[tokio::test]
    async fn transient_budget_is_not_refunded_at_each_dispatch_round() {
        let translator = ScriptedTranslator::new([
            Step::Transient,  // round 1: charge 1 of 2, sleep, re-dispatch
            Step::RejectUnit, // round 1: the unit's own fault → round 2
            Step::Transient,  // round 2: charge 2 of 2, sleep, re-dispatch
            Step::Transient,  // round 2: budget spent → terminal
        ]);
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;

        let err = run_pipeline(SRC, &opts(2), &translator, cache_dyn)
            .await
            .expect_err("a spent transient budget aborts the run (ADR-0017)");

        assert!(
            matches!(
                err,
                TransyncError::Translator(TranslatorError::RateLimited { .. })
            ),
            "the provider's own transient error is surfaced verbatim: {err:?}"
        );
        assert_eq!(
            translator.calls(),
            4,
            "2 transient retries for the whole batch, not 2 per round"
        );
        assert_eq!(translator.remaining(), 0, "the script ran to its end");
    }

    // The report stays the honest observed total, and that total is the
    // per-batch budget — reached here across two rounds rather than one.
    #[tokio::test]
    async fn provider_retries_report_the_batch_total_across_rounds() {
        let translator = ScriptedTranslator::new([
            Step::Transient,  // round 1: charge 1 of 2
            Step::RejectUnit, // round 1: the unit's own fault → round 2
            Step::Transient,  // round 2: charge 2 of 2 — the batch's last one
            Step::Translate,  // round 2: clean
        ]);
        let cache = InMemoryCache::new();
        let cache_dyn: &dyn Cache = &cache;

        let out = run_pipeline(SRC, &opts(2), &translator, cache_dyn)
            .await
            .expect("the budget covered both rounds, so the run completes");

        assert_eq!(translator.calls(), 4);
        assert_eq!(translator.remaining(), 0);
        assert_eq!(
            out.validation_report.provider_retries, 2,
            "the honest total: the whole per-batch budget, spent across two rounds"
        );
        assert_eq!(
            out.validation_report.total_retries, 1,
            "content retries are counted apart: one rejected round"
        );
        let unit = out
            .validation_report
            .per_unit
            .first()
            .expect("one unit reported");
        assert_eq!(
            unit.final_status,
            FallbackStatus::Translated,
            "the transient failures never degraded the unit: {unit:?}"
        );
    }
}
