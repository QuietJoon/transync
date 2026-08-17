//! v0.2 provider/cache boundary + RetryContext integration tests.
//!
//! Covers the black-box-observable behavior of the EXT-2026-07 P1-4 /
//! P0-2 changes: provider-fingerprint cache namespacing, targeted
//! eviction after the full-reparse cascade and on `Hard` failure, and
//! the non-content `RetryContext` channel threaded through re-dispatch.
//!
//! TRACE: EXT-2026-07 P1-4
//! TRACE: EXT-2026-07 P0-2

use std::sync::Mutex;
use transync::llm::{
    OutputKind, ProviderFingerprint, RetryContext, TranslationBatch, TranslationBatchResult,
    Translator, TranslatorError, UnitResult,
};
use transync::{
    FullReparseFailure, InMemoryCache, TranslateOptions, TransyncError, ValidationLayer,
    translate_with_cache,
};

fn opts(policy: FullReparseFailure) -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();
    opts.max_per_unit_validation_retries = 0;
    opts.full_reparse_failure = policy;
    opts
}

// ---------------------------------------------------------------------------
// Test 2 — fingerprint default distinguishes types
// ---------------------------------------------------------------------------

struct TypeA;
struct TypeB;

async fn echo_units(batch: TranslationBatch) -> Result<TranslationBatchResult, TranslatorError> {
    let units = batch
        .units
        .iter()
        .map(|u| UnitResult {
            unit_id: u.unit_id.clone(),
            output_kind: OutputKind::Translated,
            translated_payload: u.source_payload.clone(),
            warnings: Vec::new(),
        })
        .collect();
    Ok(TranslationBatchResult {
        batch_id: batch.batch_id,
        detected_source_language: None,
        units,
    })
}

#[async_trait::async_trait]
impl Translator for TypeA {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        echo_units(batch).await
    }
}
#[async_trait::async_trait]
impl Translator for TypeB {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        echo_units(batch).await
    }
}

#[test]
fn fingerprint_default_distinguishes_types() {
    // Two distinct implementor types get unequal default fingerprints.
    assert_ne!(TypeA.fingerprint(), TypeB.fingerprint());
    // Two instances of the same type share one.
    assert_eq!(TypeA.fingerprint(), TypeA.fingerprint());
    assert_eq!(TypeB.fingerprint(), TypeB.fingerprint());
}

// ---------------------------------------------------------------------------
// Counting stub with a configurable fingerprint (tests 3 + eviction tests)
// ---------------------------------------------------------------------------

struct Stub {
    fingerprint: Option<ProviderFingerprint>,
    dispatched: Mutex<Vec<String>>,
    // when Some, the second list item is nested to force full-reparse
    // divergence (list boundary contamination).
    nest_second_item: bool,
}

impl Stub {
    fn echo(tag: &str) -> Self {
        Self {
            fingerprint: Some(ProviderFingerprint::new(tag, &[])),
            dispatched: Mutex::new(Vec::new()),
            nest_second_item: false,
        }
    }
    fn nesting() -> Self {
        Self {
            fingerprint: None,
            dispatched: Mutex::new(Vec::new()),
            nest_second_item: true,
        }
    }
    fn dispatched(&self) -> Vec<String> {
        self.dispatched.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Translator for Stub {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let units = batch
            .units
            .iter()
            .map(|u| {
                self.dispatched.lock().unwrap().push(u.unit_id.0.clone());
                // Nest the second list item: "- second" -> "  - second".
                // Valid as a standalone list fragment (passes per-unit
                // validation) but changes the document's top-level
                // structure, tripping the full-document reparse.
                let payload = if self.nest_second_item && u.source_payload.contains("second") {
                    "  - second".to_string()
                } else {
                    u.source_payload.clone()
                };
                UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: payload,
                    warnings: Vec::new(),
                }
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        })
    }

    fn fingerprint(&self) -> ProviderFingerprint {
        self.fingerprint
            .clone()
            .unwrap_or_else(|| ProviderFingerprint::from_type_name("boundary::Stub"))
    }
}

// ---------------------------------------------------------------------------
// Test 3 — provider-namespace isolation (R0008-0002)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn provider_namespace_isolation() {
    const SRC: &str = "# Heading\n\npara one\n\npara two\n";
    let cache = InMemoryCache::new();
    let o = opts(FullReparseFailure::FallbackPerBlock);

    // Provider A, first run: dispatches every unit, caches under A.
    let a1 = Stub::echo("provider-a");
    translate_with_cache(SRC, &o, &a1, &cache).await.unwrap();
    let a1_units = a1.dispatched().len();
    assert!(a1_units >= 3, "A's first run must dispatch every unit");

    // Provider A, second run on the same cache: all hits, zero dispatch.
    let a2 = Stub::echo("provider-a");
    translate_with_cache(SRC, &o, &a2, &cache).await.unwrap();
    assert_eq!(
        a2.dispatched().len(),
        0,
        "same-fingerprint re-run must hit the cache for every unit"
    );

    // Provider B (different fingerprint): no cross-provider hits.
    let b = Stub::echo("provider-b");
    translate_with_cache(SRC, &o, &b, &cache).await.unwrap();
    assert_eq!(
        b.dispatched().len(),
        a1_units,
        "a different provider fingerprint must dispatch every unit (no cross-provider replay)"
    );
}

// ---------------------------------------------------------------------------
// Test 5 — cascade eviction after the full-reparse downgrade (R0008-0003)
// ---------------------------------------------------------------------------

const NEST_SRC: &str = "- outer\n- second\n\nafter\n";

#[tokio::test(flavor = "current_thread")]
async fn cascade_downgrade_evicts_only_downgraded_units() {
    let cache = InMemoryCache::new();
    let o = opts(FullReparseFailure::FallbackPerBlock);

    // Run 1: every unit passes per-unit validation and is cached, then
    // the full-document reparse downgrades the divergent list boundary.
    let run1 = Stub::nesting();
    let out1 = translate_with_cache(NEST_SRC, &o, &run1, &cache)
        .await
        .unwrap();
    let mut downgraded: Vec<String> = out1
        .validation_report
        .full_reparse_fallbacks
        .iter()
        .map(|b| b.0.clone())
        .collect();
    downgraded.sort();
    assert!(
        !downgraded.is_empty(),
        "the nesting mock must trip a full-reparse downgrade"
    );
    // The healthy first item is not among the downgraded set.
    assert!(
        !downgraded.contains(&"li-0001".to_string()),
        "the healthy first list item must not be downgraded"
    );

    // Run 2 on the shared cache: only the downgraded (evicted) units
    // re-dispatch; the healthy unit(s) stay cached.
    let run2 = Stub::nesting();
    let _ = translate_with_cache(NEST_SRC, &o, &run2, &cache)
        .await
        .unwrap();
    let mut redispatched = run2.dispatched();
    redispatched.sort();
    assert_eq!(
        redispatched, downgraded,
        "run 2 must re-dispatch exactly the downgraded/evicted units"
    );
    assert!(
        !redispatched.contains(&"li-0001".to_string()),
        "a healthy unit's cache entry must survive the cascade (OI-0011 keep-progress)"
    );
}

// ---------------------------------------------------------------------------
// Test 6 — Hard failure maps the error and evicts implicated keys (R0008-0004)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "current_thread")]
async fn hard_failure_maps_error_and_evicts_implicated_keys() {
    let cache = InMemoryCache::new();
    let o = opts(FullReparseFailure::Hard);

    // Run 1 under Hard: per-unit-valid results are cached, then the
    // full-document reparse fails and the run aborts with the stable
    // error string; the implicated key(s) are evicted.
    let run1 = Stub::nesting();
    let err = translate_with_cache(NEST_SRC, &o, &run1, &cache)
        .await
        .expect_err("Hard policy must surface the full-reparse failure");
    match err {
        TransyncError::Validation(msg) => assert!(
            msg.starts_with("full reparse failed: "),
            "unexpected error message: {msg}"
        ),
        other => panic!("expected TransyncError::Validation, got {other:?}"),
    }

    // Run 2 on the shared cache: the evicted (implicated) unit re-dispatches
    // while healthy sibling entries stay cached (OI-0011 keep-progress).
    let run2 = Stub::nesting();
    let _ = translate_with_cache(NEST_SRC, &o, &run2, &cache).await;
    let redispatched = run2.dispatched();
    assert!(
        redispatched.contains(&"p-0003".to_string()),
        "the implicated block must have been evicted and re-dispatched; got {redispatched:?}"
    );
    assert!(
        !redispatched.contains(&"li-0001".to_string()),
        "a healthy sibling's cache entry must survive the Hard failure (OI-0011)"
    );
}

// ---------------------------------------------------------------------------
// Test 7 — an eviction naming a SPLIT table reaches its windows' keys
// (DCR-0026 §4)
// ---------------------------------------------------------------------------

/// A ceiling the generated table cannot fit under, so the DCR-0026 split is
/// forced rather than hoped for. Well above the 64-token response-envelope
/// reserve, so it is a legal ceiling.
const SPLITTING_CEILING: u32 = 1_200;

/// A document whose table is oversize under [`SPLITTING_CEILING`] — so packing
/// replaces it with row windows — and whose two-item list is the
/// [`Stub::nesting`] full-reparse divergence trigger.
fn split_src() -> String {
    let mut s = String::from("- outer\n- second\n\n");
    s.push_str(
        "| Code   | Name              | Description                                | Notes   |\n",
    );
    s.push_str(
        "|--------|-------------------|--------------------------------------------|---------|\n",
    );
    for i in 1..=200 {
        s.push_str(&format!(
            "| R-{i:04} | row-name-{i:04}    | Description for row {i:04}, short text      | row {i} |\n"
        ));
    }
    s.push_str("\nafter\n");
    s
}

/// [`opts`] plus the profile knobs the split needs: the shipped default
/// profile is already `row-window-first`, so only the ceiling is set here.
fn splitting_opts(policy: FullReparseFailure) -> TranslateOptions {
    let mut o = opts(policy);
    let mut profile = transync::profile::default_profile();
    profile.batching.target_output_tokens = Some(SPLITTING_CEILING);
    o.profile = Some(profile);
    o
}

/// The dispatched ids that are window ids, deduplicated and ordered.
fn window_ids(dispatched: &[String]) -> Vec<String> {
    let mut v: Vec<String> = dispatched
        .iter()
        .filter(|id| id.contains(".w"))
        .cloned()
        .collect();
    v.sort();
    v.dedup();
    v
}

/// A split parent is never dispatched and has no cache key of its own, so an
/// eviction naming it must reach its **windows'** keys (DCR-0026 §4). Without
/// that mapping the windows' entries survive the eviction and a shared-cache
/// re-run deterministically replays the state the reparse just disqualified —
/// the exact replay bug R0008-0003 closed for whole blocks.
#[tokio::test(flavor = "current_thread")]
async fn split_table_eviction_reaches_the_windows_keys() {
    let src = split_src();

    // Half 1 (non-vacuity): with no downgrade, a window's entry is written and
    // reused. An entry that was never cached would also be "not there" after a
    // broken eviction, so the second half proves nothing without this one.
    let clean_cache = InMemoryCache::new();
    let clean_opts = splitting_opts(FullReparseFailure::FallbackPerBlock);
    let clean1 = Stub::echo("split-window");
    translate_with_cache(&src, &clean_opts, &clean1, &clean_cache)
        .await
        .unwrap();
    let windows = window_ids(&clean1.dispatched());
    assert!(
        windows.len() >= 2,
        "the oversize table must be dispatched as two or more row windows, got {windows:?}"
    );
    let clean2 = Stub::echo("split-window");
    translate_with_cache(&src, &clean_opts, &clean2, &clean_cache)
        .await
        .unwrap();
    assert!(
        clean2.dispatched().is_empty(),
        "a re-run must hit the cache for every unit, windows included; re-dispatched {:?}",
        clean2.dispatched()
    );

    // Half 2: the nesting stub trips the full-document reparse and the
    // FallbackAll policy downgrades every block, so `full_reparse_fallbacks`
    // names the split table's parent id — a block with no key of its own.
    let cache = InMemoryCache::new();
    let o = splitting_opts(FullReparseFailure::FallbackAll);
    let run1 = Stub::nesting();
    translate_with_cache(&src, &o, &run1, &cache).await.unwrap();
    let dispatched1 = window_ids(&run1.dispatched());
    assert_eq!(
        dispatched1, windows,
        "run 1 dispatches the same windows the control run did"
    );

    // Run 2 on the shared cache: the windows re-dispatch, which is only true
    // if the eviction resolved the parent id through the split plan.
    let run2 = Stub::nesting();
    translate_with_cache(&src, &o, &run2, &cache).await.unwrap();
    assert_eq!(
        window_ids(&run2.dispatched()),
        windows,
        "every window entry of the downgraded table must have been evicted"
    );
}

// ---------------------------------------------------------------------------
// RetryContext threading (tests 8 + 9)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Received {
    source_payload: String,
    retry: Option<RetryContext>,
}

struct RetryMock {
    /// Fail attempts `1..=fail_until` by returning a level-changed heading
    /// (a deterministic `PerKindShape` rejection); accept afterwards.
    fail_until: u32,
    calls: Mutex<u32>,
    log: Mutex<Vec<Received>>,
}

impl RetryMock {
    fn new(fail_until: u32) -> Self {
        Self {
            fail_until,
            calls: Mutex::new(0),
            log: Mutex::new(Vec::new()),
        }
    }
    fn calls(&self) -> u32 {
        *self.calls.lock().unwrap()
    }
    fn log(&self) -> Vec<Received> {
        self.log.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Translator for RetryMock {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &transync::CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        let call = {
            let mut c = self.calls.lock().unwrap();
            *c += 1;
            *c
        };
        let units = batch
            .units
            .iter()
            .map(|u| {
                self.log.lock().unwrap().push(Received {
                    source_payload: u.source_payload.clone(),
                    retry: u.retry.clone(),
                });
                let payload = if call <= self.fail_until {
                    u.source_payload.replacen("# ", "## ", 1)
                } else {
                    u.source_payload.clone()
                };
                UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: payload,
                    warnings: Vec::new(),
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

const HEADING_SRC: &str = "# Heading\n";

fn retry_opts() -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();
    opts.max_per_unit_validation_retries = 3;
    opts
}

#[tokio::test(flavor = "current_thread")]
async fn retry_context_threads_latest_failure_and_caches_pristine() {
    let cache = InMemoryCache::new();
    let mock = RetryMock::new(1);
    let out = translate_with_cache(HEADING_SRC, &retry_opts(), &mock, &cache).await;
    assert!(out.is_ok(), "the unit must be accepted on the retry");

    let log = mock.log();
    assert_eq!(log.len(), 2, "expected one failed attempt then one accept");
    // First dispatch is a fresh attempt — no retry channel.
    assert!(log[0].retry.is_none(), "first dispatch must carry no retry");
    // Second dispatch carries the latest failure via the side channel.
    let ctx = log[1]
        .retry
        .as_ref()
        .expect("retry attached on re-dispatch");
    assert_eq!(ctx.attempt, 2);
    assert_eq!(ctx.rejected_by, Some(ValidationLayer::PerKindShape));
    assert!(
        ctx.reason.as_deref().is_some_and(|r| !r.is_empty()),
        "retry reason must be present"
    );
    // ADR-0009: the payload is resubmitted byte-for-byte.
    assert_eq!(log[0].source_payload, log[1].source_payload);

    // Third run against the shared cache: the accepted result was cached
    // under the pristine key, so nothing dispatches.
    let cached = RetryMock::new(0);
    let _ = translate_with_cache(HEADING_SRC, &retry_opts(), &cached, &cache).await;
    assert_eq!(cached.calls(), 0, "accepted result must be cached");
}

#[tokio::test(flavor = "current_thread")]
async fn retry_context_never_stacks() {
    let cache = InMemoryCache::new();
    let mock = RetryMock::new(2);
    let _ = translate_with_cache(HEADING_SRC, &retry_opts(), &mock, &cache).await;

    let log = mock.log();
    assert_eq!(log.len(), 3, "two failures then one accept");
    // The third dispatch carries ONLY the latest failure (attempt 3),
    // built from the pristine unit each round — never an accumulation.
    let ctx = log[2]
        .retry
        .as_ref()
        .expect("retry attached on third dispatch");
    assert_eq!(ctx.attempt, 3);
    assert_eq!(ctx.rejected_by, Some(ValidationLayer::PerKindShape));
    assert!(ctx.reason.as_deref().is_some_and(|r| !r.is_empty()));
}
