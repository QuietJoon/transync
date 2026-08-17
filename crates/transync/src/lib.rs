//! `transync` — the curated public facade (semver firewall) over the
//! engine crates.
//!
//! Everything a consumer may name is re-exported here explicitly —
//! the supported surface is this file, mirrored by
//! `docs/architecture/contracts.md` §0 and pinned by
//! `tests/public_surface.rs`. Engine internals (`transync-core`'s
//! pipeline/batching/validation modules, `transync-syntax`'s
//! parser/renderer) are not reachable through this crate; advanced
//! consumers depend on the engine crates directly.
//!
//! TRACE: ADR-0002
//! TRACE: ADR-0003
//! TRACE: OI-0027

// Curated modules: the Translator/provider contract (`llm`, incl. the
// provider-SDK `llm::prompt`), profile handling, and the cache trait.
pub use transync_core::{cache, llm, profile};

#[cfg(feature = "test-stub")]
pub use transync_core::test_stub;

// Entry points and their option/result types.
pub use transync_core::{
    FullReparseFailure, TranslateOptions, TranslationOutput, translate, translate_with_cache,
};

// Run cancellation (DCR-0024). `tokio_util`'s token, re-exported rather than
// wrapped: it is the type the async ecosystem already composes with, and a
// facade-local look-alike would only force consumers to convert. Naming it
// here is also what pins the version a consumer's own `tokio-util` must unify
// with — the one place the facade admits a foreign type into the curated
// surface, and it does so deliberately.
pub use transync_core::CancellationToken;

// Errors.
pub use transync_core::{ParseError, TransyncError};

// Cache cluster.
pub use transync_core::{
    Cache, CacheError, CacheKey, DiskCache, DiskCacheOptions, DocumentMeta, DocumentMetaKey,
    GlossaryExtraction, GlossaryExtractionKey, InMemoryCache,
};

// Translator-contract cluster.
pub use transync_core::{
    BatchId, BlockConstraints, BlockContext, DEFAULT_MAX_AUTO_GLOSSARY_TERMS, GlossaryEntry,
    GlossaryExtractionRequest, GlossaryScope, InputMode, ListTopologyEntry,
    MAX_EXTRACTION_SOURCE_BYTES, OutputKind, ProviderFingerprint, RetryContext, TableAlign,
    TokenizerHint, TranslationBatch, TranslationBatchResult, TranslationUnit, Translator,
    TranslatorError, UnitResult,
};

// Profile types.
pub use transync_core::{MergedGlossary, ProfileError, ProfileMetadata, merge_auto_glossary};

// Alignment-map wire types.
pub use transync_core::{
    ALIGNMENT_SCHEMA_VERSION, AlignmentBlock, AlignmentMap, ByteRange, FallbackStatus,
    GeneratorMeta, SyncRole, ValidationSummary,
};

// Block identity.
pub use transync_core::{BlockId, BlockKind};

// Validation-report family.
pub use transync_core::{
    AttemptOutcome, AutoGlossaryReport, AutoGlossaryStatus, BatchFault, OutputBudgetWarning,
    UnitValidationRecord, VALIDATION_REPORT_SCHEMA_VERSION, VALIDATION_SCHEMA_VERSION,
    ValidationLayer, ValidationReport,
};
