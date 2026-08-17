//! Top-level error type for the `transync` library.
//!
//! TRACE: ADR-0002

use crate::llm::TranslatorError;
use crate::profile::ProfileError;

/// The parse-side error now lives in `transync-syntax`; re-exported here so
/// this crate's root alias — and through it the curated facade path
/// `transync::ParseError` — resolves without naming the syntax crate.
pub use transync_syntax::error::ParseError;

/// Every fallible path in `transync` terminates here.
///
/// Non-exhaustive: new variants may be added in minor releases; match with
/// a wildcard arm.
///
/// Every variant carries a stable machine code — see
/// [`TransyncError::stable_code`] and contracts.md §1.
///
/// TRACE: SCN-08
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TransyncError {
    #[error("parse error: {0}")]
    Parse(#[from] transync_syntax::error::ParseError),

    #[error("translator error: {0}")]
    Translator(#[from] TranslatorError),

    #[error("validation rejected: {0}")]
    Validation(String),

    #[error("regenerate error: {0}")]
    Regen(String),

    #[error("profile error: {0}")]
    Profile(#[from] ProfileError),

    #[error("alignment-map serialization error: {0}")]
    Alignment(#[from] serde_json::Error),

    /// The run stopped because the caller's
    /// [`crate::TranslateOptions::cancel`] token fired (DCR-0024).
    ///
    /// **A cancelled run is not a degraded one.** It returns no
    /// [`crate::TranslationOutput`] at all — not a partial document with the
    /// unreached blocks marked `fallback_source`. ADR-0017 settled that shape
    /// for batch-terminal provider failures and the reasoning transfers
    /// unchanged: an untranslated island in a "successful" result is not
    /// visually distinguished in raw Markdown, so a consumer can ship it
    /// without noticing. `fallback_source` means "we tried this block and it
    /// could not be translated"; a cancelled run's unreached blocks were never
    /// tried, and borrowing the marker would make the two indistinguishable in
    /// the alignment map.
    ///
    /// **Progress is preserved in the cache, not in the return value.** Every
    /// unit accepted before the cancellation was already written to the
    /// [`crate::Cache`] as it was accepted, so a caller that supplied its own
    /// cache to [`crate::translate_with_cache`] re-dispatches only the
    /// remainder on the next attempt. That is the OI-0011 keep-progress
    /// mechanism, and it is the reason a cancelled run can afford to answer
    /// with an error rather than a half-document.
    #[error("cancelled")]
    Cancelled,

    #[error("internal error: {0}")]
    Internal(String),
}

impl TransyncError {
    /// Stable, machine-readable error code for downstream consumers
    /// that map `TransyncError` to a wire format. The returned strings
    /// will not change without a major-version bump.
    ///
    /// The `Translator` arm **delegates** to
    /// [`TranslatorError::stable_code`] rather than flattening the whole
    /// provider family to one code (ti 1a85f3): a consumer that only ever
    /// sees the code string — the inter-process case this method exists for
    /// — otherwise could not tell a content-policy stop from an exhausted
    /// output ceiling from a mistyped model name, and so could not tell a
    /// retryable failure from a guaranteed-identical one. `provider_error`
    /// survives as the code for `TranslatorError::Other`, the unclassified
    /// case. Both matches stay exhaustive with no wildcard arm, so a new
    /// variant on either enum cannot land without a code.
    ///
    /// TRACE: R0004-0001
    /// TRACE: ti 1a85f3
    pub fn stable_code(&self) -> &'static str {
        match self {
            TransyncError::Parse(_) => "parse_failed",
            TransyncError::Translator(e) => e.stable_code(),
            TransyncError::Validation(_) => "validation_failed",
            TransyncError::Regen(_) => "regen_failed",
            TransyncError::Profile(_) => "profile_failed",
            TransyncError::Alignment(_) => "alignment_failed",
            TransyncError::Cancelled => "cancelled",
            TransyncError::Internal(_) => "internal",
        }
    }
}
