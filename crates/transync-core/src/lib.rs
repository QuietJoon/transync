//! `transync-core` — GFM Markdown translation with block-level alignment maps.
//!
//! Downstream consumers normally depend on the `transync` facade crate,
//! which re-exports this crate's public surface.
//!
//! Public surface:
//! - [`translate`] — top-level pipeline entry.
//! - [`Translator`] — trait that consumers implement (or use `transync_openai` for the default).
//!   Carries the defaulted [`llm::ProviderFingerprint`]-valued `fingerprint` cache-namespace method.
//! - [`TranslationOutput`] — the pipeline result: the regenerated target
//!   document, the alignment map and the two annotated HTML panes that render
//!   against it, the validation report, and the metadata the run detected.
//!   It is `#[non_exhaustive]`, so read the type for the current field set —
//!   this list names the groups, not a count.
//! - [`cache::Cache`] v2 (`get`/`put`/`evict`, all fallible via [`cache::CacheError`]),
//!   [`cache::CacheKey`] (provider-fingerprinted + validation-schema-versioned),
//!   plus four defaulted document-level methods, in two record families:
//!   [`cache::Cache::get_document_meta`] / [`cache::Cache::put_document_meta`]
//!   (keyed by [`cache::DocumentMetaKey`]) let a fully-cache-hit run still
//!   report the detection a live run made (DCR-0028, OI-0017), and
//!   [`cache::Cache::get_glossary_extraction`] /
//!   [`cache::Cache::put_glossary_extraction`] (keyed by
//!   [`cache::GlossaryExtractionKey`]) let it elide the auto-glossary preflight
//!   — the one provider call a warm run still paid (ti `dca5bf`). Two backends
//!   ship: [`InMemoryCache`] (run-scoped) and [`DiskCache`] (a versioned
//!   JSON-lines log that outlives the process).
//! - [`llm::RetryContext`] — the non-content retry channel (ADR-0009).
//! - [`CancellationToken`] + [`TranslateOptions::cancel`] — run cancellation
//!   (DCR-0024). A cancelled run answers [`TransyncError::Cancelled`] and
//!   leaves its paid-for progress in the caller's [`Cache`].
//! - [`BatchFault`] + [`TranslateOptions::max_per_batch_schema_retries`]
//!   — batch-envelope faults (dropped/duplicated result rows) are attributed
//!   to the units they implicate and charged to their own bounded budget, so
//!   one mangled row never spends an innocent batch-mate's content-retry
//!   budget (OI-0031).
//! - [`llm::Translator::extract_glossary`] + [`GlossaryExtractionRequest`] —
//!   the optional, defaulted candidate-glossary preflight, opted into via
//!   [`TranslateOptions::auto_glossary`] and merged static-wins by
//!   [`merge_auto_glossary`]; its outcome rides on
//!   [`ValidationReport::auto_glossary`] (OI-0026).
//! - [`llm::prompt`] — provider-neutral user-prompt assembly, Structured
//!   Output schema object, and response-envelope parsing, shared by every
//!   provider so its wording stays in step with the validators (OI-0029);
//!   [`llm::TokenizerHint`] lets a provider name its own encoder for
//!   batch-budget estimation.
//! - All wire types under [`llm`], [`profile`], [`cache`] and the
//!   alignment-map types re-exported at the crate root.
//!
//! TRACE: ADR-0001
//! TRACE: ADR-0002

pub(crate) use transync_syntax::align;
pub(crate) mod batch;
pub mod cache;
pub(crate) mod error;
pub(crate) use transync_syntax::id;
pub mod llm;
pub(crate) use transync_syntax::parser;
pub(crate) mod pipeline;
pub mod profile;
pub(crate) use transync_syntax::regen;
pub(crate) use transync_syntax::render;
pub(crate) mod structure;
// Internal unit-construction engine. `pub` only because transync-openai's
// live-smoke test reaches it cross-crate via a dev-dependency (OI-0027
// spec §2.3); not part of the curated API.
#[doc(hidden)]
pub mod unit;
pub(crate) mod validate;

#[cfg(feature = "test-stub")]
pub mod test_stub;

#[cfg(test)]
pub(crate) mod test_fixtures;

pub use cache::{
    Cache, CacheError, CacheKey, DiskCache, DiskCacheOptions, DocumentMeta, DocumentMetaKey,
    GlossaryExtraction, GlossaryExtractionKey, InMemoryCache,
};
pub use error::{ParseError, TransyncError};
pub use llm::{
    BatchId, BlockConstraints, BlockContext, DEFAULT_MAX_AUTO_GLOSSARY_TERMS, GlossaryEntry,
    GlossaryExtractionRequest, GlossaryScope, InputMode, ListTopologyEntry,
    MAX_EXTRACTION_SOURCE_BYTES, OutputKind, ProviderFingerprint, RetryContext, TableAlign,
    TokenizerHint, TranslationBatch, TranslationBatchResult, TranslationUnit, Translator,
    TranslatorError, UnitResult,
};
pub use profile::{MergedGlossary, ProfileError, ProfileMetadata, merge_auto_glossary};
/// The run-cancellation handle (DCR-0024), re-exported rather than
/// re-invented: `tokio_util`'s token is what the async ecosystem already
/// composes with (`child_token`, `DropGuard`, graceful-shutdown crates), and a
/// transync-local look-alike would only make a consumer convert between two
/// types that mean the same thing.
///
/// Re-exporting it also puts the version requirement somewhere a consumer can
/// see: `transync::CancellationToken` and a token the consumer builds from its
/// own `tokio-util` are the same type exactly when the two requirements unify,
/// and a mismatch is a compile error naming both crates rather than a silent
/// misbehavior.
pub use tokio_util::sync::CancellationToken;
pub use transync_syntax::align::ALIGNMENT_SCHEMA_VERSION;
pub use transync_syntax::align::{
    AlignmentBlock, AlignmentMap, ByteRange, FallbackStatus, GeneratorMeta, SyncRole,
    ValidationSummary,
};
pub use transync_syntax::id::{BlockId, BlockKind, SourceFormat};
pub use validate::{
    AttemptOutcome, AutoGlossaryReport, AutoGlossaryStatus, BatchFault, OutputBudgetWarning,
    UnitValidationRecord, VALIDATION_REPORT_SCHEMA_VERSION, VALIDATION_SCHEMA_VERSION,
    ValidationLayer, ValidationReport,
};

/// Caller-tunable knobs for one [`translate`] call.
///
/// Defaults reflect `docs/architecture/contracts.md` §5.
///
/// Non-exhaustive: construct via default-then-assign —
/// `let mut opts = TranslateOptions::default(); opts.target_language = "ko".into();`
/// Struct-literal and functional-update construction are not available
/// outside this crate.
///
/// TRACE: SCN-11
/// TRACE: ADR-0002
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TranslateOptions {
    /// Source language as an opaque label, or the literal `"auto"` to let
    /// the model detect it (the detected value comes back on the alignment
    /// map's `detected_source_language`). BCP-47 codes (`ko`, `ja-JP`) are
    /// the recommended form, not a requirement: nothing parses, validates or
    /// case-folds the label — it reaches the prompt, the cache key and the
    /// alignment map verbatim (ADR-0013). Only the `"auto"` sentinel test
    /// tolerates surrounding whitespace and case; the label itself is left
    /// exactly as given, so ` ko ` and `ko` are different cache identities.
    pub source_language: String,
    /// Target language as an opaque label, on the same terms as
    /// [`Self::source_language`] — unparsed, unvalidated, forwarded verbatim.
    /// Required: [`translate_with_cache`] rejects an empty or whitespace-only
    /// value before the pipeline runs.
    pub target_language: String,
    /// **Advisory** model label. It is a labeling-and-estimation input,
    /// never a routing one: nothing here decides which model runs.
    ///
    /// What it governs, exhaustively:
    ///
    /// 1. **Encoder fallback** — the tiktoken encoding the batch packer
    ///    measures with, and *only* when the translator declares no
    ///    [`TokenizerHint`]. A provider that implements
    ///    [`Translator::tokenizer_hint`] (the bundled OpenAI adapter does,
    ///    from its own configured model) overrides this field outright, so
    ///    estimation follows the model actually being called.
    /// 2. **The output-budget preflight** — same encoder, same override
    ///    rule; it is resolved once and shared with the packer so the two
    ///    cannot disagree.
    /// 3. **One [`CacheKey`] axis** (`CacheKey::model_id`) — *alongside*
    ///    [`CacheKey::provider_fingerprint`], never instead of it.
    ///
    /// What it does **not** govern: which model is called, which endpoint
    /// or API surface the request goes to, or any field of the request
    /// body. Those belong entirely to the [`Translator`] instance, whose
    /// [`Translator::fingerprint`] is the authoritative record of the
    /// model, base URL, and surface a run used.
    ///
    /// So a value that disagrees with the provider's real model is **safe
    /// but wasteful** (R0001-0010). It cannot serve one model's text as
    /// another's, because `provider_fingerprint` already partitions the
    /// cache by the provider's own model; what it costs is a redundant
    /// cache-namespace split, a run labeled with a model it did not call,
    /// and — only for a provider that declares no hint — batches packed
    /// against the wrong encoder (a soft budget, so the effect is
    /// over- or under-full batches, not a failure).
    ///
    /// The field stays caller-owned rather than being read off the
    /// translator because [`Translator`] deliberately carries no model
    /// identity: an implementation may be multi-model, model-less, or
    /// route per request. The two seams it *does* expose carry the load
    /// instead — `fingerprint()` for identity, `tokenizer_hint()` for
    /// estimation — and the CLI resolves its model once and hands the same
    /// string to both.
    ///
    /// TRACE: OI-0029
    pub model_id: String,
    /// Per-unit validation-retry budget for faults in the unit's **own**
    /// output (per-kind shape, fragment reparse, inline protection, or a
    /// provider `FailedNeedsFallback`); total content attempts per unit =
    /// `retries + 1`.
    ///
    /// Batch-envelope faults are charged separately — see
    /// [`Self::max_per_batch_schema_retries`].
    pub max_per_unit_validation_retries: u32,
    /// OI-0031: per-batch budget for **batch-level schema faults** — rounds
    /// in which the provider dropped a requested unit's result row or
    /// returned it more than once. The implicated units are re-dispatched
    /// verbatim (ADR-0009) at most this many times per input batch; once the
    /// budget is spent they finalize as `fallback_source`. Innocent
    /// batch-mates are unaffected either way: their returned payloads are
    /// validated and accepted in the round they arrived.
    ///
    /// Charged **once per round**, not once per offender, so an entirely
    /// empty response costs one budget point rather than one per unit. This
    /// budget is deliberately separate from
    /// [`Self::max_per_unit_validation_retries`]: a dropped unit's content
    /// was never judged, so spending its content budget on the provider's
    /// formatting failure would leave a twice-dropped unit with fewer real
    /// translation attempts than a merely-mistranslated one.
    ///
    /// Default 2, mirroring the per-unit budget. Library-only, like the
    /// per-unit budget — the CLI exposes batching and output knobs, not
    /// retry-policy internals.
    ///
    /// TRACE: OI-0031
    pub max_per_batch_schema_retries: u32,
    /// Per-batch provider-retry budget for transient transport failures
    /// (`TranslatorError::Network` / `RateLimited`) — and only those; every
    /// other provider error is terminal on the first occurrence.
    ///
    /// The budget belongs to the **input batch as a whole**: it is charged
    /// across every dispatch round of that batch's retry ladder, so one batch
    /// consumes at most this many transient retries in total no matter how
    /// many validation- or schema-retry rounds it takes. Once it is spent,
    /// the next transient error is terminal and aborts the run (ADR-0017) —
    /// a transport failure is never a fallback path. Between retries the
    /// pipeline sleeps: the provider's `Retry-After` when given (capped at
    /// 30 s), otherwise 200 ms doubling per attempt (capped at 5 s), with the
    /// attempt ordinal counted per batch like the budget itself.
    ///
    /// `ValidationReport.provider_retries` reports the observed total, which
    /// is therefore bounded by `input batch count × this value`.
    ///
    /// Default 1. Library-only, like the other two retry budgets.
    pub max_per_batch_provider_retries: u32,
    /// Maximum number of in-flight provider requests at once. The
    /// pipeline dispatches batches concurrently up to this cap so
    /// total wall-clock time scales with the slowest concurrent batch
    /// rather than the sum of all batches. Set to 1 to force
    /// sequential dispatch.
    ///
    /// Supported range `>= 1`. A `0` cannot dispatch anything and this
    /// field has no `Option` in which to say "unset" (nor a profile
    /// counterpart — concurrency is a runtime property with no profile
    /// home), so a zero is reported on `tracing::warn` and then resolves
    /// as the built-in default (6). It is **not** floored to 1: a stray
    /// zero can no longer turn a run sequential without saying so.
    pub max_concurrent_batches: u32,
    /// Soft cap on per-batch input tokens. Units are packed into a
    /// batch until adding the next unit would push the running token
    /// count over this value, after which a new batch starts. Counted
    /// with the model's tiktoken encoding (`o200k_base` for gpt-4o /
    /// gpt-5 / o-series, `cl100k_base` otherwise) plus a small per-unit
    /// JSON overhead. Default 6000 leaves headroom for the model's own
    /// output within a typical 8K input context.
    pub target_input_tokens_per_batch: u32,
    /// Hard cap on units per batch regardless of token count, so retry
    /// granularity stays manageable on documents full of tiny blocks.
    pub max_units_per_batch: u32,
    /// Compiled profile (system prompt, glossary, batching hints).
    /// `None` = `profile::default_profile()` will be used.
    pub profile: Option<ProfileMetadata>,
    /// OI-0026: run the auto-glossary extraction preflight — an extraction
    /// pass before batching that harvests recurring source terminology and
    /// merges it into the run's glossary (static profile entries win on
    /// conflict), so every batch's system prompt pins the same target
    /// renderings.
    ///
    /// `None` (the default) defers to the profile's
    /// [`ProfileMetadata::auto_glossary`], which in turn defaults to
    /// `false`. `Option<bool>` rather than `bool` so an explicit opt-out
    /// is expressible over a profile that enables it — unlike
    /// `max_units_per_batch`, where a caller value equal to the built-in
    /// default is indistinguishable from unset.
    ///
    /// **Off by default.** The harvest is looked up in the [`Cache`]
    /// before the provider is called, under
    /// [`cache::GlossaryExtractionKey`] — whose `request_hash` is the
    /// assembled extraction request's own bytes, which are known long
    /// before the merge runs — so the extra provider call is paid once per
    /// distinct extraction question and a warm run replays the hit and
    /// calls nothing (ti `dca5bf`). What every enabled run does pay is a
    /// glossary section on every batch's system prompt, and a
    /// nondeterministic extractor lowers warm-cache hit rates. The drift
    /// it prevents starts mattering at ≥3 batches, so long-document
    /// profiles are the place to make it sticky.
    ///
    /// TRACE: OI-0026
    pub auto_glossary: Option<bool>,
    /// What to do when the post-regeneration full-document reparse
    /// rejects the output. See [`FullReparseFailure`].
    ///
    /// Default: [`FullReparseFailure::FallbackPerBlock`] — silently
    /// degrade by marking the divergent source block(s) as fallback,
    /// re-regenerating, and re-checking. The previous "hard error"
    /// behavior is preserved as opt-in via [`FullReparseFailure::Hard`].
    ///
    /// TRACE: R0004-0001
    pub full_reparse_failure: FullReparseFailure,
    /// Cancellation handle for this run (DCR-0024). `None` — the default —
    /// means the run cannot be cancelled and behaves exactly as it did before
    /// this field existed.
    ///
    /// When the token fires, [`translate`] / [`translate_with_cache`] stop
    /// issuing provider requests and return
    /// [`TransyncError::Cancelled`] — never a partial
    /// [`TranslationOutput`]. See that variant for why, and for where the
    /// paid-for progress goes.
    ///
    /// **Observation points.** Cancellation is observed at run entry (before
    /// the parse), around the auto-glossary preflight, at every batch's start,
    /// at every dispatch round, around every provider call, around every
    /// transport-backoff sleep, and once more after the batch fan-out settles.
    /// The last of those is authoritative: a cancelled run reports
    /// `Cancelled` even when a sibling batch also failed for its own reason.
    /// Past that point only regeneration and rendering remain — pure CPU, no
    /// I/O — and they are not interruptible, so a token that fires during them
    /// is not observed and the run returns `Ok`.
    ///
    /// **Every provider call is raced against this token**, so a `Translator`
    /// that ignores its `cancel` argument is still cancelled: its future is
    /// dropped, which aborts an in-flight `reqwest`-style request. An
    /// implementation whose work is not drop-cancellable must honor the
    /// argument itself.
    ///
    /// **One token per run.** The field is `Option` rather than a bare token
    /// so that "no cancellation requested" is expressible and is the default,
    /// and because [`TranslateOptions`] is `Clone`: a long-lived options
    /// template that carried a token would hand every run the *same* one, and
    /// cancelling one job would cancel all of them. Derive a per-run handle
    /// from a daemon-wide one instead —
    /// `opts.cancel = Some(shutdown.child_token())` — which cancels on either
    /// the job's own signal or the daemon's.
    ///
    /// The token is **not** a cache-identity axis: it cannot change what a
    /// completed provider call says, only whether one happens.
    ///
    /// TRACE: DCR-0024
    pub cancel: Option<CancellationToken>,
    /// Which intake reads `source` (ti 490d97, decision D8's library half).
    ///
    /// **Routing is explicit — the library never sniffs.** `Markdown` (the
    /// default) is today's path, unchanged in every byte. `Html` parses the
    /// source with the HTML intake (`transync-syntax`'s `intake::html`),
    /// translates the same blocks through the same pipeline, validates the
    /// regenerated document with the scanner-side layer-6 gate
    /// (`full_rescan_html` — never a comrak reparse), and returns HTML in
    /// [`TranslationOutput::translated_document`]. A Markdown string
    /// declared `Html` produces one text-heavy block set and translates —
    /// wrong shape, but *explicitly requested*, which is the boundary
    /// ADR-0017's silent-path refusals protect; there is no reverse sniff,
    /// because a body fragment is legitimately accepted HTML.
    ///
    /// The CLI flag is `--input-format` (contracts §6), and
    /// [`TranslationOutput`]'s two annotated pane fields carry the §8
    /// synthesized fragments since DCR-0038.
    ///
    /// TRACE: ADR-0025
    pub input_format: SourceFormat,
}

/// Caller-controlled severity for post-regeneration full-document
/// reparse failures.
///
/// The pipeline parses the regenerated Markdown and compares the
/// top-level block sequence to the source IR. When the two diverge,
/// this enum chooses the response.
///
/// TRACE: SCN-14
/// TRACE: R0004-0001
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FullReparseFailure {
    /// Surface a `TransyncError::Validation` with the divergence
    /// diagnostic and stop.
    Hard,
    /// Mark the divergent source block(s) as `FallbackSource`,
    /// re-regenerate, and re-check. If the reparse still fails, widen
    /// the fallback to the flagged blocks' immediate top-level
    /// neighbors and re-check; if that also fails, auto-escalate to
    /// [`FullReparseFailure::FallbackAll`]. Never returns `Err` —
    /// use [`FullReparseFailure::Hard`] to surface failures instead.
    /// See DCR-0004.
    #[default]
    FallbackPerBlock,
    /// Drop every translated payload and regenerate from source bytes.
    /// Always succeeds — the output is structurally the source.
    FallbackAll,
}

impl Default for TranslateOptions {
    fn default() -> Self {
        Self {
            source_language: "auto".to_string(),
            target_language: String::new(),
            model_id: "gpt-5-chat-latest".to_string(),
            max_per_unit_validation_retries: 2,
            max_per_batch_schema_retries: 2,
            max_per_batch_provider_retries: 1,
            max_concurrent_batches: 6,
            target_input_tokens_per_batch: 6000,
            max_units_per_batch: 32,
            profile: None,
            auto_glossary: None,
            full_reparse_failure: FullReparseFailure::default(),
            cancel: None,
            // The enum's own `#[default]`, spelled explicitly because this
            // literal lists every field.
            input_format: SourceFormat::Markdown,
        }
    }
}

/// Pipeline result returned to the caller.
///
/// Four groups, not a fixed count: the regenerated target document; the
/// alignment map plus the two annotated HTML panes that render against it;
/// the validation report; and the metadata the run detected from the source
/// (its language, its title).
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct — new fields may be added in minor releases, so do not
/// write prose (here or downstream) that counts them.
///
/// TRACE: SCN-12
/// TRACE: ADR-0001
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct TranslationOutput {
    /// The full regenerated target document, in the source document's own
    /// format.
    ///
    /// An `input_format = Html` run returns HTML here; a Markdown run returns
    /// Markdown — read the format from the input you handed the pipeline,
    /// never from this field's name. The field
    /// was called `translated_markdown` until v0.4.0. It was renamed because
    /// the format is a property of the *input*, not of this field: the
    /// HTML→HTML path (ticket `490d97`) translates an HTML document and returns
    /// HTML here, and a name that promised Markdown would then be a lie on
    /// exactly the runs a consumer most needs to trust it. Read the format from
    /// the input you handed the pipeline, never from this field's name.
    pub translated_document: String,
    pub alignment_map: AlignmentMap,
    /// The source pane: annotated HTML rendered from the source IR against
    /// [`Self::alignment_map`].
    ///
    /// On an `input_format = Html` run these are the §8 synthesized
    /// `<main>` fragments (DCR-0038): strip-then-inject over the blocks'
    /// own elements, with every `BlockKind::Html` block — and every
    /// element-less one, an img-run `Image` included — riding a transparent
    /// `<div>` instead, because the shells' DOMPurify mount can remove an
    /// element and every attribute on it. `li` blocks share one group;
    /// head, gap bytes, doctype, comments, `<script>`/`<style>` and
    /// non-sync rows are never present (D6 — a pane is a sync surface, not
    /// a fidelity preview; `transync serve` over the published document is
    /// the fidelity view). On a Markdown run they are the comrak-paired
    /// fragments, unchanged.
    pub annotated_source_html: String,
    /// The target pane, on the same terms as [`Self::annotated_source_html`].
    pub annotated_target_html: String,
    pub validation_report: ValidationReport,
    pub detected_source_language: Option<String>,
    /// The source document's title — the plain text of its first level-1
    /// heading — or `None` when it has none. Same value the provider read as
    /// `BlockContext::document_title` on every unit of this run, produced by
    /// the same extraction (ti 0f26b5), so a front end that titles a rendered
    /// document and the model that translated it cannot disagree about what
    /// the document is called.
    ///
    /// On an `input_format = Html` run it is the `<title>` block's extracted
    /// text when the document has one, else the first heading's — the same
    /// `unit::context` selection rule and the same projection the provider
    /// saw (ti 490d97 wave 5).
    ///
    /// It is Markdown-derived prose, not source: ATX/setext markers and inline
    /// delimiters are gone because the parser consumed them. It is also
    /// **untrusted** (invariant 7) and may be empty (a `#` with no text), so a
    /// consumer escapes it for its output format and decides for itself
    /// whether an empty title is a title.
    pub document_title: Option<String>,
}

/// Top-level translation entry. Runs the internal pipeline with a fresh in-memory cache.
///
/// Cancellable via [`TranslateOptions::cancel`] — but note that the fresh
/// cache dies with the call, so a cancelled `translate` discards the progress
/// it paid for. A caller that may cancel should use [`translate_with_cache`]
/// with a cache it owns; that is what turns a cancelled run into a resumable
/// one (DCR-0024).
///
/// TRACE: SCN-01..SCN-14
/// TRACE: ADR-0002
pub async fn translate<T>(
    source: &str,
    opts: &TranslateOptions,
    translator: &T,
) -> Result<TranslationOutput, TransyncError>
where
    T: Translator + ?Sized,
{
    let cache = InMemoryCache::new();
    let cache_dyn: &dyn Cache = &cache;
    translate_with_cache(source, opts, translator, cache_dyn).await
}

/// Same as [`translate`] but accepts an externally-managed cache so
/// callers can run multiple translation passes against shared state
/// (partial-resume across documents, or a cache that outlives the process).
///
/// Persistence is shipped, not future work: [`DiskCache`] (DCR-0028) is a
/// versioned JSON-lines log under a cache directory, and passing one here is
/// what makes a re-run of the same document cost only its misses. It carries
/// one constraint the caller owns — **one writer per cache directory**;
/// concurrent processes sharing one are unsupported, and the violation costs
/// *entries* (a re-translation), never corrupt output and never a failed run.
/// Its construction is fallible so the degrade decision stays with the caller;
/// see [`DiskCache`] for the recovery and capacity rules.
///
/// This is the entry point cancellation was designed against (DCR-0024):
/// units accepted before the token fired are already in `cache`, so a re-run
/// with the same cache re-dispatches only the remainder. With a
/// caller-owned cache, "cancel then resume" costs one round of in-flight
/// batches; without one, it costs the whole document.
///
/// TRACE: SCN-10
/// TRACE: ADR-0002
pub async fn translate_with_cache<T>(
    source: &str,
    opts: &TranslateOptions,
    translator: &T,
    cache: &dyn Cache,
) -> Result<TranslationOutput, TransyncError>
where
    T: Translator + ?Sized,
{
    if opts.target_language.trim().is_empty() {
        return Err(TransyncError::Internal(
            "TranslateOptions.target_language must be non-empty".to_string(),
        ));
    }
    pipeline::run_pipeline(source, opts, translator, cache).await
}
