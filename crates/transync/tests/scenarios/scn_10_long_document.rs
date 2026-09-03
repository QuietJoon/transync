//! SCN-10 — Long document → many batches; partial-resume.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-10:
//! - The fixture splits into ≥ 4 batches at the default `max_units_per_batch = 8`.
//! - Partial-resume (same in-memory cache reused across runs) produces
//!   byte-identical final output.
//!
//! TRACE: SCN-10
//! TRACE: SL-10

use crate::common::fixture_gen::generate_scn_10;
use crate::common::mock_translator::MockTranslator;
use std::collections::HashSet;
use std::sync::Mutex;
use transync::BlockId;
use transync::llm::{
    OutputKind, TranslationBatch, TranslationBatchResult, Translator, TranslatorError, UnitResult,
};
use transync::{InMemoryCache, TransyncError, translate, translate_with_cache};

#[tokio::test]
async fn smoke_scn_10() {
    // Generate the 30-section variant on the fly (~120 sync-relevant blocks).
    let source = generate_scn_10(30);
    let translator = MockTranslator::recording();
    let opts = crate::common::opts_for("ko");

    let output = translate(&source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-10 30-section doc");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert!(!output.translated_document.is_empty());

    // Recording mock captured every batch; the count must be >= 4.
    let recorded = translator.recorded();
    assert!(
        recorded.len() >= 4,
        "expected >= 4 batches at the default token + unit caps, got {}",
        recorded.len(),
    );
    for (i, b) in recorded.iter().enumerate() {
        assert!(!b.units.is_empty(), "batch {i} should not be empty");
    }

    // Skeleton fixture must still round-trip.
    let skeleton = include_str!("../fixtures/scn-10-long-document.md");
    let _ = translate(skeleton, &opts, &MockTranslator::passthrough())
        .await
        .expect("skeleton SCN-10 fixture should still round-trip");
}

#[tokio::test]
async fn partial_resume_byte_identical() {
    let source = generate_scn_10(30);
    let opts = crate::common::opts_for("ko");

    // Run 1: shared cache populated by passthrough mock.
    let cache_a = InMemoryCache::new();
    let translator_a = MockTranslator::passthrough();
    let out_a = translate_with_cache(&source, &opts, &translator_a, &cache_a)
        .await
        .expect("first run completes");
    assert!(
        !cache_a.is_empty(),
        "cache should be populated after a successful run",
    );

    // Run 2: same cache, same options. Every unit should hit the cache and
    // the mock should NOT be called (call_count stays 0).
    let translator_b = MockTranslator::passthrough();
    let out_b = translate_with_cache(&source, &opts, &translator_b, &cache_a)
        .await
        .expect("second run completes via cache");
    assert_eq!(
        translator_b.call_count(),
        0,
        "second run with populated cache should not invoke the translator",
    );

    // Run 3: empty cache, fresh mock. Compare against run 2.
    let cache_c = InMemoryCache::new();
    let translator_c = MockTranslator::passthrough();
    let out_c = translate_with_cache(&source, &opts, &translator_c, &cache_c)
        .await
        .expect("third run completes");

    assert_eq!(
        out_b.translated_document, out_c.translated_document,
        "partial-resume run must produce byte-identical translated MD",
    );
    assert_eq!(
        serde_json::to_string(&out_b.alignment_map).unwrap(),
        serde_json::to_string(&out_c.alignment_map).unwrap(),
        "partial-resume run must produce identical alignment map JSON",
    );
    // Sanity: run 1 and run 3 (both fresh-cache) match too.
    assert_eq!(out_a.translated_document, out_c.translated_document);
}

/// DCR-0028 §3 / OI-0017 item 4 — the partial-resume guarantee stops having an
/// exception.
///
/// R0007-0005 narrowed the SL-10 contract because the per-unit cache stored no
/// document-level record: a fully-cached resume run made no provider call, so
/// no envelope existed, so `detected_source_language` came back `None` and the
/// byte-identical guarantee had to carve that one field out. The `Cache` seam
/// now carries a document record, written from the live run's qualifying
/// envelope and replayed by a run that dispatched **zero** provider batches, so
/// the carve-out is gone: the resume run's alignment map is byte-identical
/// including that field.
///
/// The zero-call assertion is what makes this a replay rather than a second
/// detection — the resume translator would answer "en" if it were asked, so the
/// value alone would not distinguish the two.
#[tokio::test]
async fn partial_resume_replays_the_detected_language() {
    let source = generate_scn_10(30);
    let mut opts = crate::common::opts_for("ko");
    opts.source_language = "auto".to_string();

    let cache = InMemoryCache::new();
    let out_first = translate_with_cache(
        &source,
        &opts,
        &MockTranslator::echoes_detected("en"),
        &cache,
    )
    .await
    .expect("first run completes");
    assert_eq!(
        out_first.detected_source_language.as_deref(),
        Some("en"),
        "a dispatching run surfaces the provider's detection",
    );

    let translator_resume = MockTranslator::echoes_detected("en");
    let out_resume = translate_with_cache(&source, &opts, &translator_resume, &cache)
        .await
        .expect("resume run completes via cache");
    assert_eq!(
        translator_resume.call_count(),
        0,
        "resume run with populated cache should not invoke the translator",
    );
    assert_eq!(
        out_resume.detected_source_language.as_deref(),
        Some("en"),
        "the stored document record answers for the envelope that never happened",
    );

    // No field is carved out any more: the whole map compares as it stands.
    assert_eq!(
        serde_json::to_string(&out_first.alignment_map).unwrap(),
        serde_json::to_string(&out_resume.alignment_map).unwrap(),
        "a fully-cached resume must produce an identical alignment map",
    );
    assert_eq!(
        out_first.translated_document,
        out_resume.translated_document
    );
}

/// Test-only translator that injects a mid-run provider failure.
///
/// The first `succeed_batches` calls pass through (echo `source_payload`
/// verbatim, exactly like `MockTranslator::passthrough`); every later call
/// returns a non-transient [`TranslatorError::Other`], which the pipeline
/// surfaces immediately without provider retries (only `Network` /
/// `RateLimited` are retried). It records the units it *successfully*
/// translated — the exact set that lands in the cache — and every unit it
/// was *asked* to translate, so a shared-cache resume can prove which units
/// were served from the cache instead of re-dispatched.
///
/// A dedicated type (rather than reusing `MockTranslator`) is required for
/// two reasons: `MockTranslator` can only emit per-unit `FailedNeedsFallback`
/// (the fallback path), never an aborting `TranslatorError`; and both runs
/// must share one translator *type* so the default `fingerprint()` — and
/// therefore every cache key — matches across the abort and the resume.
///
/// TRACE: OI-0011
/// TRACE: OI-0023
struct FailAfterNBatches {
    succeed_batches: u32,
    calls: Mutex<u32>,
    succeeded_units: Mutex<HashSet<BlockId>>,
    dispatched_units: Mutex<HashSet<BlockId>>,
}

impl FailAfterNBatches {
    fn new(succeed_batches: u32) -> Self {
        Self {
            succeed_batches,
            calls: Mutex::new(0),
            succeeded_units: Mutex::new(HashSet::new()),
            dispatched_units: Mutex::new(HashSet::new()),
        }
    }

    /// Units the provider returned `Translated` for — i.e. what the pipeline
    /// cached before (or during) the abort.
    fn succeeded_units(&self) -> HashSet<BlockId> {
        self.succeeded_units.lock().unwrap().clone()
    }

    /// Every unit the provider was asked to translate. On a shared-cache
    /// resume this is exactly the set of cache misses.
    fn dispatched_units(&self) -> HashSet<BlockId> {
        self.dispatched_units.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl Translator for FailAfterNBatches {
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
        {
            let mut d = self.dispatched_units.lock().unwrap();
            for u in &batch.units {
                d.insert(u.unit_id.clone());
            }
        }

        if call > self.succeed_batches {
            return Err(TranslatorError::Other(format!(
                "injected mid-run provider failure on batch call {call}"
            )));
        }

        {
            let mut s = self.succeeded_units.lock().unwrap();
            for u in &batch.units {
                s.insert(u.unit_id.clone());
            }
        }
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
}

/// OI-0023 item 3 — a mid-run provider failure must abort the run yet KEEP
/// the progress the successful batches already cached (the OI-0011
/// "abort but keep cached progress, never blanket-clear" contract).
///
/// This is deliberately adversarial: it FAILS if the abort path cleared the
/// cache and PASSES only under the current keep-progress behavior. The proof
/// is that on a shared-cache resume the units cached before the abort are
/// never re-sent to the provider (they resolve as cache hits), while the
/// un-cached remainder is re-dispatched and the run completes.
///
/// TRACE: OI-0011
/// TRACE: OI-0023
#[tokio::test]
async fn abort_keeps_cached_progress() {
    let source = generate_scn_10(30);
    let mut opts = crate::common::opts_for("ko");
    // Sequential dispatch makes "succeed for the first N batches, then
    // fail" deterministic: batch call k is input batch index k-1, so the
    // failure lands after exactly `succeed_batches` cached batches.
    opts.max_concurrent_batches = 1;

    // One cache shared across both runs so retention is observable.
    let shared_cache = InMemoryCache::new();

    // Run 1: succeed for 2 batches, then abort on every later batch.
    let failing = FailAfterNBatches::new(2);
    let err = translate_with_cache(&source, &opts, &failing, &shared_cache)
        .await
        .expect_err("a mid-run provider error must abort the run");
    assert!(
        matches!(err, TransyncError::Translator(TranslatorError::Other(_))),
        "the run must surface the provider abort error, got {err:?}",
    );

    // What the successful batches cached, and the full unit universe (every
    // batch was dispatched under sequential collect; the later ones errored).
    let cached_before = failing.succeeded_units();
    let total_units = failing.dispatched_units();
    assert!(
        !cached_before.is_empty(),
        "the successful batches must have cached at least one unit",
    );
    assert!(
        !shared_cache.is_empty(),
        "OI-0011: aborting must not blanket-clear the cache",
    );
    assert!(
        cached_before.len() < total_units.len(),
        "the abort must leave some units un-translated for the resume to pick up \
         (cached {}, total {})",
        cached_before.len(),
        total_units.len(),
    );

    // Run 2: same shared cache, same translator TYPE (so the provider
    // fingerprint and every cache key match run 1). This translator never
    // fails, so it completes — dispatching ONLY the units not already cached.
    let recovering = FailAfterNBatches::new(u32::MAX);
    let out = translate_with_cache(&source, &opts, &recovering, &shared_cache)
        .await
        .expect("resume run completes once the provider stops failing");
    assert!(!out.translated_document.is_empty());

    let redispatched = recovering.dispatched_units();

    // Core OI-0011 assertion: NONE of the units cached before the abort are
    // re-sent to the provider on the resume — they were retained as cache
    // hits. A blanket cache-clear on abort would re-dispatch every unit, and
    // this intersection would be non-empty.
    let leaked: Vec<&BlockId> = redispatched.intersection(&cached_before).collect();
    assert!(
        leaked.is_empty(),
        "OI-0011 violated: {} unit(s) cached before the abort were re-dispatched \
         on resume (cache was not retained): {:?}",
        leaked.len(),
        leaked,
    );

    // The resume did real work on exactly the un-cached remainder: retained +
    // re-dispatched partition the whole document with no overlap and no loss.
    assert!(
        !redispatched.is_empty(),
        "the resume must re-dispatch the batches that failed in run 1",
    );
    assert_eq!(
        cached_before.len() + redispatched.len(),
        total_units.len(),
        "retained ({}) + re-dispatched ({}) must cover every unit exactly once ({})",
        cached_before.len(),
        redispatched.len(),
        total_units.len(),
    );
}

/// DCR-0027, at scenario level: on a real 30-section document dispatched
/// through `translate`, no batch the provider receives holds units from two
/// sections.
///
/// The section a unit belongs to is re-derived here from the batches
/// themselves — flatten them in dispatch order, start a new section at every
/// heading unit — so the assertion does not read back the partition it is
/// checking. Each of this fixture's sections is a heading plus three
/// paragraphs, well under the default caps, so the sanctioned oversize-section
/// fallback never fires and the count is exactly one batch per section.
///
/// TRACE: DCR-0027
#[tokio::test]
async fn no_batch_straddles_a_section_boundary() {
    let sections = 30usize;
    let source = generate_scn_10(sections);
    let translator = MockTranslator::recording();
    let opts = crate::common::opts_for("ko");

    translate(&source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-10 30-section doc");

    let recorded = translator.recorded();
    // Batch ids are the document-order sequence the packer assigned; the mock
    // records in completion order, so sort before walking.
    let mut ordered = recorded.clone();
    // `BatchId` is a zero-padded `b-NNNN` string, so lexical order is
    // document order; it carries no `Ord` of its own (curated surface).
    ordered.sort_by(|a, b| a.batch_id.0.cmp(&b.batch_id.0));

    let mut section_of: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut section = 0usize;
    let mut first = true;
    for b in &ordered {
        for u in &b.units {
            if u.block_kind.heading_level().is_some() && !first {
                section += 1;
            }
            first = false;
            section_of.insert(u.unit_id.0.clone(), section);
        }
    }
    assert_eq!(
        section + 1,
        sections,
        "the fixture's section count must be what the walk finds"
    );

    for b in &ordered {
        let spanned: HashSet<usize> = b.units.iter().map(|u| section_of[&u.unit_id.0]).collect();
        assert_eq!(
            spanned.len(),
            1,
            "batch {:?} spans sections {spanned:?}",
            b.batch_id
        );
    }
    assert_eq!(
        ordered.len(),
        sections,
        "every section of this fixture fits one batch, so there is exactly \
         one batch per section"
    );
}
