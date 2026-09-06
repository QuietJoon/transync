//! Run cancellation — the caller-facing contract (ti `43331a`, DCR-0024).
//!
//! DCR-0009 deferred deadline/cancellation on `Translator` with a YAGNI
//! rationale and a stated revisit condition: *a long-running service consumer
//! appearing*. `dynwebserver` is that consumer, so the deferral was
//! re-decided. This file pins what replaced it.
//!
//! Four claims, in the order a consumer meets them:
//!
//! 1. **Cancellation stops work.** An already-cancelled run issues no provider
//!    request at all; a run cancelled mid-flight stops issuing new ones, drops
//!    the in-flight one, and does not sit out a transport backoff.
//! 2. **A cancelled run returns an error, not a document.** ADR-0017 settled
//!    that a run which cannot complete answers `Err` rather than a quietly
//!    degraded artifact, and a cancelled run is the same shape: unreached
//!    blocks were never *tried*, so marking them `fallback_source` would make
//!    them indistinguishable from blocks that were tried and failed.
//! 3. **The paid-for progress survives in the caller's cache**, which is what
//!    makes claim 2 affordable — the answer to "you threw away my work" is a
//!    cache the caller owns, not a half-document.
//! 4. **A `Translator` that ignores its token is still cancelled**, because
//!    the pipeline races every provider call and drops the loser.
//!
//! Two smaller sections follow them: the auto-glossary preflight's asymmetry
//! (an extraction *error* degrades, a *cancellation* aborts) and the two
//! cancellation codes' distinctness.
//!
//! **Nothing here is timed, as of 2026-09-06.** This is ti `d41782`'s **option
//! 2**, which that ticket named as the strongest outcome, declined to take at
//! the time, and left explicitly available: "assert the property with an
//! instrumented drop counter … it remains the strongest outcome and is still
//! available".
//!
//! The two cancellation races below used to end in a wall-clock bound. Those
//! bounds tripped three times on trees containing no async, timing or
//! cancellation code at all — 8.46 s, 5.04 s and 5.55 s against a 5 s bound —
//! and `d41782` answered on 2026-09-01 with its option 1: derive each bound
//! from the signature it names, giving `CAPPED_BACKOFF / 2` (15 s) and
//! `IGNORED_SLEEP / 10` (60 s). That removed the recurring cost at a fraction
//! of the change and was the right call then. It did not change the
//! instrument, which is a stopwatch measuring a host: this suite runs at
//! `--test-threads=4` on a machine that parks a fresh test binary at zero CPU
//! for minutes, so seconds of scheduler noise land inside a millisecond of
//! work — and one of the two stalls being ruled out is only 30 s, so widening
//! has a ceiling.
//!
//! The property both bounds were reaching for is **abandonment**, not speed.
//! It is now pinned by two observations that hold whatever the host is doing:
//! a scheduler-turn budget (yielding does not move the clock, so a run sitting
//! out a `tokio::time::sleep` is still pending on every turn of it) and a
//! counter the ten-minute provider increments only on the far side of its
//! sleep. Both regression signatures are unchanged — still a 120 s
//! `retry_after` capped to 30 s, still a 600 s sleep — and both tests still
//! fail decisively against them.

use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::task::Poll;
use std::time::Duration;

use transync::llm::{
    GlossaryEntry, GlossaryExtractionRequest, OutputKind, TranslationBatch, TranslationBatchResult,
    Translator, TranslatorError, UnitResult,
};
use transync::{Cache, CancellationToken, InMemoryCache, TranslateOptions, TransyncError};

/// Four short paragraphs, one unit each. `max_units_per_batch = 1` below turns
/// them into four batches, which is what gives "some batches ran, others never
/// started" a shape to observe.
const SOURCE: &str =
    "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n\nFourth paragraph.\n";

fn opts() -> TranslateOptions {
    let mut o = TranslateOptions::default();
    o.source_language = "en".to_string();
    o.target_language = "ko".to_string();
    // One unit per batch, dispatched one at a time: the ordering that makes
    // "stopped part-way" deterministic rather than a race between six
    // concurrent futures.
    o.max_units_per_batch = 1;
    o.max_concurrent_batches = 1;
    o
}

fn echo(batch: &TranslationBatch) -> TranslationBatchResult {
    TranslationBatchResult {
        batch_id: batch.batch_id.clone(),
        detected_source_language: None,
        units: batch
            .units
            .iter()
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: u.source_payload.clone(),
                warnings: Vec::new(),
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// The instrument that replaced the stopwatch
// ---------------------------------------------------------------------------

/// How many scheduler turns a cancelled run may spend before this file calls it
/// stuck. Generous by three orders of magnitude — the runs below finish in a
/// couple of turns, since nothing on a cancelled path awaits a timer, and no
/// number of turns finishes a run that is waiting one out.
const MAX_TURNS: u32 = 10_000;

/// Drives `run` on the current task, one poll per scheduler turn, calling
/// `between` after every poll that leaves it pending — the hook is where a test
/// fires its token, so a cancellation that "arrives mid-flight" is *sequenced*
/// against the run's own progress instead of raced with a timer.
///
/// This is a budget of **turns**, not of time, and that is the whole point: a
/// `tokio::task::yield_now` does not move the clock, so a run that is sitting
/// out a `tokio::time::sleep` of tens of seconds is pending on every turn here
/// however fast or slow the host is, while a run that raced that sleep against
/// its token needs no timer at all and finishes immediately. A contended
/// machine makes this loop take longer in seconds; it cannot make it take more
/// turns. (ti `d41782` — see the module doc for the wall-clock bounds this
/// replaced and the false failures they produced.)
async fn drive_to_completion<F: Future>(
    run: F,
    mut between: impl FnMut(),
    stuck: &str,
) -> F::Output {
    let mut run = pin!(run);
    for _ in 0..MAX_TURNS {
        if let Poll::Ready(out) = poll_fn(|cx| Poll::Ready(run.as_mut().poll(cx))).await {
            return out;
        }
        between();
        tokio::task::yield_now().await;
    }
    panic!("the run was still pending after {MAX_TURNS} scheduler turns: {stuck}");
}

// ---------------------------------------------------------------------------
// Translators
// ---------------------------------------------------------------------------

/// Echoes, counting calls and recording every unit id it was asked to
/// translate. `cancel_after` fires the run's own token once that many calls
/// have been made — the mid-flight case, driven from inside the provider so it
/// needs no timing.
#[derive(Default)]
struct CountingEcho {
    calls: AtomicU32,
    seen_units: Mutex<Vec<String>>,
    cancel_after: Option<u32>,
    /// Whether the token this provider was handed was live and shared: it
    /// records what it observed at the *end* of the call that cancelled.
    saw_live_token: AtomicU32,
}

impl CountingEcho {
    fn cancelling_after(n: u32) -> Self {
        Self {
            cancel_after: Some(n),
            ..Self::default()
        }
    }
}

#[async_trait::async_trait]
impl Translator for CountingEcho {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut seen = self.seen_units.lock().expect("unit log");
            seen.extend(batch.units.iter().map(|u| u.unit_id.0.clone()));
        }
        let result = echo(&batch);
        if self.cancel_after == Some(n) {
            cancel.cancel();
            // The argument really is the run's token, not a private one: the
            // provider cancelled it and can see the effect immediately.
            if cancel.is_cancelled() {
                self.saw_live_token.store(1, Ordering::SeqCst);
            }
        }
        Ok(result)
    }
}

/// Ignores its `cancel` argument entirely and sleeps for ten minutes. The only
/// way a run using it can stop is the pipeline dropping the future.
#[derive(Default)]
struct IndifferentAndSlow {
    started: AtomicU32,
    /// Incremented **only on the far side of the sleep**, which is what makes
    /// "the future was abandoned" observable rather than inferred from a clock:
    /// a dropped future never reaches this line, and a run that waits the sleep
    /// out reaches it on any machine at any speed.
    reached_far_side: AtomicU32,
}

#[async_trait::async_trait]
impl Translator for IndifferentAndSlow {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        self.started.fetch_add(1, Ordering::SeqCst);
        // Far longer than the test's patience, and deliberately not raced
        // against the token: dropping this future is the whole assertion.
        tokio::time::sleep(Duration::from_secs(600)).await;
        // Answering normally rather than `unreachable!()`: a run that sat the
        // sleep out is a failure the counter names, not a panic raised from
        // inside a provider call the test is no longer even watching.
        self.reached_far_side.fetch_add(1, Ordering::SeqCst);
        Ok(echo(&batch))
    }
}

/// Rate-limits with a long `Retry-After`, then cancels the run. Exercises the
/// backoff-sleep race: without it the run would wait out the cap.
#[derive(Default)]
struct RateLimitsThenCancels {
    calls: AtomicU32,
}

#[async_trait::async_trait]
impl Translator for RateLimitsThenCancels {
    async fn translate_batch(
        &self,
        _batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        // Cancel *before* answering, so the pipeline is already holding a
        // cancelled token when it decides to back off. The retry_after is
        // above the 30 s cap, so the sleep the pipeline schedules is 30 s.
        cancel.cancel();
        Err(TranslatorError::RateLimited {
            retry_after: Some(Duration::from_secs(120)),
        })
    }
}

/// Cancels the run from inside the auto-glossary preflight. Counts translation
/// calls so the test can show the run never reached dispatch.
#[derive(Default)]
struct CancelsInPreflight {
    extractions: AtomicU32,
    translations: AtomicU32,
}

#[async_trait::async_trait]
impl Translator for CancelsInPreflight {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        self.translations.fetch_add(1, Ordering::SeqCst);
        Ok(echo(&batch))
    }

    async fn extract_glossary(
        &self,
        _req: &GlossaryExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        self.extractions.fetch_add(1, Ordering::SeqCst);
        cancel.cancel();
        Err(TranslatorError::Cancelled)
    }
}

// ---------------------------------------------------------------------------
// 1. Cancellation stops work
// ---------------------------------------------------------------------------

/// A run handed an already-cancelled token spends nothing: no parse-dependent
/// provider call, no preflight, no batch. This is the shutdown case — a daemon
/// draining its queue can hand every remaining job a cancelled token and know
/// none of them will bill.
#[tokio::test]
async fn an_already_cancelled_run_issues_no_provider_request() {
    let translator = CountingEcho::default();
    let mut o = opts();
    let token = CancellationToken::new();
    token.cancel();
    o.cancel = Some(token);

    let err = transync::translate(SOURCE, &o, &translator)
        .await
        .expect_err("an already-cancelled run cannot succeed");

    assert!(
        matches!(err, TransyncError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    assert_eq!(err.stable_code(), "cancelled");
    assert_eq!(
        translator.calls.load(Ordering::SeqCst),
        0,
        "a cancelled run must not call the provider at all"
    );
}

/// The same fixtures with no token succeed. Without this, every assertion
/// above could be passing because the fixture never worked.
#[tokio::test]
async fn the_same_run_without_a_token_completes() {
    let translator = CountingEcho::default();
    let out = transync::translate(SOURCE, &opts(), &translator)
        .await
        .expect("an uncancelled run of the same fixture must succeed");
    assert_eq!(
        translator.calls.load(Ordering::SeqCst),
        4,
        "four one-unit batches"
    );
    assert!(out.translated_document.contains("Fourth paragraph."));
}

/// A cancelled run does not sit out the transport backoff. The provider
/// answers `RateLimited { retry_after: 120s }` — capped by policy to a 30 s
/// sleep — after cancelling; the run must abandon that sleep rather than serve
/// it out.
///
/// This is the term the filing consumer measured as worst case: a stall
/// multiplied by batches and by retries, for a job nobody wants any more.
#[tokio::test]
async fn cancellation_interrupts_the_transport_backoff() {
    let translator = RateLimitsThenCancels::default();
    let mut o = opts();
    o.cancel = Some(CancellationToken::new());

    // The abandonment is observed as *turns*, not seconds. The provider cancels
    // before answering `RateLimited`, so the pipeline reaches its backoff select
    // holding an already-cancelled token: racing it there returns with no timer
    // in play, which is a couple of turns. Sitting the 30 s sleep out instead
    // leaves the run pending through every turn of the budget — a `yield_now`
    // does not move the clock — so this fails the regression on a machine of
    // any speed, and cannot fail because the machine was slow.
    let err = drive_to_completion(
        transync::translate(SOURCE, &o, &translator),
        || {},
        "the provider cancelled the run before answering `RateLimited`, so the \
         only thing that keeps the run pending here is the policy's 30 s capped \
         backoff being slept rather than raced against the token",
    )
    .await
    .expect_err("a cancelled run cannot succeed");

    assert!(
        matches!(err, TransyncError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    // Non-vacuity: the run really did dispatch and really did get the transient
    // error that schedules the backoff. Without this the test would also pass
    // for a run that answered `Cancelled` before ever calling the provider.
    assert_eq!(
        translator.calls.load(Ordering::SeqCst),
        1,
        "the backoff path must have been reached: exactly one dispatch, \
         answered with the rate limit that schedules the sleep"
    );
}

// ---------------------------------------------------------------------------
// 2. A cancelled run returns an error, not a document
// ---------------------------------------------------------------------------

/// Cancelled mid-run, after one batch of four has been translated. The caller
/// gets `Cancelled` — **not** a `TranslationOutput` whose last three
/// paragraphs are source text marked `fallback_source`.
///
/// The distinction is the whole design decision. `fallback_source` means "this
/// block was attempted and could not be translated"; these blocks were never
/// attempted. ADR-0017 refused the same conflation for batch-terminal provider
/// failures — a translated document with an untranslated island is not
/// visually distinguished in raw Markdown, so a consumer can ship it without
/// noticing — and nothing about cancellation weakens that.
#[tokio::test]
async fn a_cancelled_run_returns_an_error_rather_than_a_partial_document() {
    let translator = CountingEcho::cancelling_after(1);
    let mut o = opts();
    o.cancel = Some(CancellationToken::new());

    let err = transync::translate(SOURCE, &o, &translator)
        .await
        .expect_err("a cancelled run must not produce a document");

    assert!(
        matches!(err, TransyncError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    assert_eq!(err.stable_code(), "cancelled");
    // Exactly one batch was dispatched: the one that cancelled. The three
    // queued behind it started no work.
    assert_eq!(
        translator.calls.load(Ordering::SeqCst),
        1,
        "batches queued behind the cancellation must not dispatch"
    );
    assert_eq!(
        translator.saw_live_token.load(Ordering::SeqCst),
        1,
        "the token handed to the provider must be the run's own, not a copy \
         the pipeline ignores"
    );
}

// ---------------------------------------------------------------------------
// 3. Progress survives in the caller's cache
// ---------------------------------------------------------------------------

/// The answer to "cancelling threw away everything I paid for": it did not,
/// as long as the caller owns the cache.
///
/// Pass 1 cancels after two of four batches. Pass 2 runs the same document
/// against the same cache with a fresh token and must dispatch only the
/// remaining two units — the first two come back as cache hits. This is the
/// OI-0011 keep-progress mechanism, now load-bearing for cancellation, and it
/// is why `translate_with_cache` is the entry point a cancelling consumer
/// should use.
#[tokio::test]
async fn cancellation_leaves_its_paid_progress_in_the_callers_cache() {
    let cache = InMemoryCache::new();
    let cache_dyn: &dyn Cache = &cache;

    let first = CountingEcho::cancelling_after(2);
    let mut o = opts();
    o.cancel = Some(CancellationToken::new());
    let err = transync::translate_with_cache(SOURCE, &o, &first, cache_dyn)
        .await
        .expect_err("pass 1 is cancelled");
    assert!(matches!(err, TransyncError::Cancelled));
    assert_eq!(first.calls.load(Ordering::SeqCst), 2);
    let done: Vec<String> = first.seen_units.lock().expect("unit log").clone();
    assert_eq!(done.len(), 2, "two units were paid for before the cancel");

    // Pass 2: same document, same cache, no cancellation.
    let second = CountingEcho::default();
    let out = transync::translate_with_cache(SOURCE, &opts(), &second, cache_dyn)
        .await
        .expect("the resumed run completes");

    let redone: Vec<String> = second.seen_units.lock().expect("unit log").clone();
    assert_eq!(
        second.calls.load(Ordering::SeqCst),
        2,
        "the resumed run must dispatch only the two units the cancelled run \
         never reached, not all four"
    );
    for id in &done {
        assert!(
            !redone.contains(id),
            "unit {id} was paid for in pass 1 and must come from the cache in \
             pass 2, not from a second provider call"
        );
    }
    assert!(out.translated_document.contains("Fourth paragraph."));
}

/// The mirror image, and the reason `translate` carries a warning in its
/// rustdoc: with the engine's own throwaway cache there is nothing to resume
/// from, so a cancelled run really does discard its progress. Pinning it keeps
/// the two entry points' guidance honest rather than aspirational.
#[tokio::test]
async fn without_a_caller_owned_cache_a_cancelled_run_keeps_nothing() {
    let first = CountingEcho::cancelling_after(2);
    let mut o = opts();
    o.cancel = Some(CancellationToken::new());
    let err = transync::translate(SOURCE, &o, &first)
        .await
        .expect_err("pass 1 is cancelled");
    assert!(matches!(err, TransyncError::Cancelled));

    let second = CountingEcho::default();
    transync::translate(SOURCE, &opts(), &second)
        .await
        .expect("the retry completes");
    assert_eq!(
        second.calls.load(Ordering::SeqCst),
        4,
        "translate() builds a fresh cache per call, so a cancelled run's work \
         is not recoverable through it"
    );
}

// ---------------------------------------------------------------------------
// 4. An indifferent Translator is still cancelled
// ---------------------------------------------------------------------------

/// Honoring the `cancel` argument is a SHOULD. This provider ignores it and
/// sleeps for ten minutes; the run must still stop, because the pipeline races
/// every provider call against the token and **drops** the loser. Dropping is
/// what aborts an in-flight `reqwest` request, so this is the guarantee that
/// covers implementations transync does not own.
#[tokio::test]
async fn a_translator_that_ignores_the_token_is_cancelled_by_being_dropped() {
    let translator = IndifferentAndSlow::default();
    let token = CancellationToken::new();
    let mut o = opts();
    o.cancel = Some(token.clone());

    // Cancellation still arrives from *outside* the provider, mid-flight — the
    // consumer's shape — but it arrives on the first turn where the provider is
    // known to be in flight rather than after a 50 ms timer. The token stays the
    // caller's: contracts.md §5b forbids an implementor cancelling the token it
    // was handed, so the fixture must not fire it from inside `translate_batch`.
    let err = drive_to_completion(
        transync::translate(SOURCE, &o, &translator),
        || {
            if translator.started.load(Ordering::SeqCst) == 1 {
                token.cancel();
            }
        },
        "the provider ignores its token and sleeps ten minutes, so a run still \
         pending here is one that is awaiting that sleep instead of dropping \
         the losing future",
    )
    .await
    .expect_err("a cancelled run cannot succeed");

    assert!(
        matches!(err, TransyncError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    // The real assertion, and the one that reads the mechanism rather than the
    // machine: the provider's sleep was ABANDONED, never completed. A dropped
    // future cannot reach its far side; a run that waited the 600 s out reaches
    // it on any host. The turn budget above is the same statement made early
    // enough to fail in milliseconds instead of ten minutes.
    assert_eq!(
        translator.reached_far_side.load(Ordering::SeqCst),
        0,
        "the pipeline must have dropped the losing future — reaching the far \
         side of the provider's sleep means it waited the call out instead"
    );
    assert_eq!(
        translator.started.load(Ordering::SeqCst),
        1,
        "exactly the one in-flight call — the queued batches must not start"
    );
}

// ---------------------------------------------------------------------------
// The preflight asymmetry
// ---------------------------------------------------------------------------

/// An auto-glossary *error* degrades the run (static glossary only, a `Failed`
/// report row, no abort). A *cancellation* during the same call does not: the
/// pipeline re-reads the token after the preflight returns and aborts.
///
/// Without that re-read the two would be indistinguishable, and cancelling
/// during the one call a run makes before batching would mean "proceed to
/// translate the whole document" — the exact opposite of the instruction.
#[tokio::test]
async fn a_cancelled_preflight_aborts_rather_than_degrading() {
    let translator = CancelsInPreflight::default();
    let mut o = opts();
    o.auto_glossary = Some(true);
    o.cancel = Some(CancellationToken::new());

    let err = transync::translate(SOURCE, &o, &translator)
        .await
        .expect_err("a run cancelled during the preflight cannot succeed");

    assert!(
        matches!(err, TransyncError::Cancelled),
        "expected Cancelled, got {err:?}"
    );
    assert_eq!(translator.extractions.load(Ordering::SeqCst), 1);
    assert_eq!(
        translator.translations.load(Ordering::SeqCst),
        0,
        "a cancelled preflight must not be followed by the whole document's \
         worth of dispatch"
    );
}

/// Non-vacuity for the test above: with the token left alone, an extraction
/// that fails is still only a degradation — the run completes.
#[tokio::test]
async fn a_failed_preflight_still_only_degrades() {
    struct FailingExtractor;

    #[async_trait::async_trait]
    impl Translator for FailingExtractor {
        async fn translate_batch(
            &self,
            batch: TranslationBatch,
            _cancel: &CancellationToken,
        ) -> Result<TranslationBatchResult, TranslatorError> {
            Ok(echo(&batch))
        }

        async fn extract_glossary(
            &self,
            _req: &GlossaryExtractionRequest,
            _cancel: &CancellationToken,
        ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
            Err(TranslatorError::Other("extraction is down".into()))
        }
    }

    let mut o = opts();
    o.auto_glossary = Some(true);
    let out = transync::translate(SOURCE, &o, &FailingExtractor)
        .await
        .expect("a failed extraction degrades, it does not abort");
    assert!(out.translated_document.contains("Fourth paragraph."));
}

// ---------------------------------------------------------------------------
// Provider-level naming
// ---------------------------------------------------------------------------

/// `TranslatorError::Cancelled` is its own cause with its own code, rather
/// than a message inside `Other`. DCR-0023 pulled five terminal causes out of
/// `Other` precisely because `Other` reads downstream as *unknown* and the
/// honest handling of unknown is retry; a cancelled call is the one outcome
/// where retrying is most obviously wrong.
///
/// The code is `provider_cancelled`, not the engine's `cancelled`: a run
/// cancelled on the caller's token answers `TransyncError::Cancelled` before a
/// provider code can surface, so this one names the other case — a provider
/// that cancelled off a source of its own. The test below it drives that case
/// through `translate` to show it is a route a pipeline caller really has, not
/// only one a direct trait call has.
#[test]
fn a_cancelled_provider_call_has_its_own_name_and_code() {
    assert_eq!(
        TranslatorError::Cancelled.stable_code(),
        "provider_cancelled"
    );
    assert_eq!(TransyncError::Cancelled.stable_code(), "cancelled");
    assert_ne!(
        TranslatorError::Cancelled.stable_code(),
        TranslatorError::Other("cancelled".into()).stable_code(),
    );
    // A provider's own cancellation, when the run's token never fired, is a
    // terminal provider error and keeps its provider-side identity.
    let wrapped = TransyncError::from(TranslatorError::Cancelled);
    assert_eq!(wrapped.stable_code(), "provider_cancelled");
}

/// Answers `Cancelled` off a cancellation source of its own — the trait
/// permits exactly this — without ever touching the run's token.
#[derive(Default)]
struct CancelsOnItsOwnSource {
    calls: AtomicU32,
}

#[async_trait::async_trait]
impl Translator for CancelsOnItsOwnSource {
    async fn translate_batch(
        &self,
        _batch: TranslationBatch,
        _cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(TranslatorError::Cancelled)
    }
}

/// `provider_cancelled` is reachable through `translate`, not only by driving
/// the trait directly. The provider cancels on its own source while the run's
/// token stays unfired, so the verdict checkpoint does not trigger and the
/// batch's terminal error is returned as-is.
///
/// The run is genuinely *not* cancelled here — `Cancelled` (engine) would be
/// the wrong answer, and the code the caller gets says whose decision stopped
/// the work. Pinning it keeps contracts.md §1's account of the two cancellation
/// codes honest.
#[tokio::test]
async fn a_providers_own_cancellation_surfaces_as_a_provider_error() {
    let translator = CancelsOnItsOwnSource::default();
    let mut o = opts();
    // One batch, unlike the rest of this file: the call count is then a
    // statement about the retry ladder alone, with no per-batch term in it.
    o.max_units_per_batch = 16;
    let token = CancellationToken::new();
    o.cancel = Some(token.clone());

    let err = transync::translate(SOURCE, &o, &translator)
        .await
        .expect_err("a terminal provider error aborts the run");

    assert!(
        !token.is_cancelled(),
        "the run's own token must stay unfired — otherwise this test is the engine-side case"
    );
    assert!(
        matches!(err, TransyncError::Translator(TranslatorError::Cancelled)),
        "expected Translator(Cancelled), got {err:?}"
    );
    assert_eq!(err.stable_code(), "provider_cancelled");
    assert_eq!(
        translator.calls.load(Ordering::SeqCst),
        1,
        "terminal by construction: the ladder must not re-dispatch a cancelled call"
    );
}
