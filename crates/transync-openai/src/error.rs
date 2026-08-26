//! Map provider-specific errors back to the `transync::TranslatorError`
//! variants in `contracts.md` §1.
//!
//! TRACE: SCN-12

use std::time::Duration;
use transync::llm::TranslatorError;

/// Errors raised by the OpenAI client wrapper before they are mapped to
/// the `transync::TranslatorError` surface.
///
/// TRACE: SCN-12
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("transport: {0}")]
    Transport(String),

    #[error("authentication: {0}")]
    Auth(String),

    /// Rate-limit response, carrying whatever the `Retry-After` header
    /// parsed to — **including its absence**, as `None`. The one rate-limit
    /// variant on purpose (R0009-0085): a bare `RateLimited` alongside it
    /// described the same fault twice, only ever appeared in mapping code,
    /// and invited later code and tests to disagree about which one a 429
    /// actually produces. `provider_error_for_status` has always answered
    /// this one.
    #[error("rate limited (retry after {0:?})")]
    RateLimitedAfter(Option<Duration>),

    #[error("malformed: {0}")]
    Malformed(String),

    /// The provider's content policy ended generation (Chat
    /// `finish_reason: "content_filter"`; a Responses `incomplete` whose
    /// `reason` names the filter).
    #[error("content filtered: {0}")]
    ContentFiltered(String),

    /// The output ceiling was exhausted before the answer was complete
    /// (Chat `finish_reason: "length"`; Responses
    /// `incomplete (reason: max_output_tokens)`).
    #[error("output ceiling exhausted: {0}")]
    OutputCeilingExhausted(String),

    /// The model declined, on either surface and in any of the shapes a
    /// refusal arrives in.
    #[error("model refused: {0}")]
    ModelRefused(String),

    /// The answer body blew past the transport's `MAX_RESPONSE_BYTES` cap.
    #[error("response too large: {0}")]
    ResponseTooLarge(String),

    /// A non-transient client error: the provider rejected the request
    /// itself. `status` is the HTTP status that decided it.
    #[error("rejected: {message}")]
    Rejected {
        status: Option<u16>,
        message: String,
    },

    #[error("other: {0}")]
    Other(String),
}

/// Map a provider error to the public `TranslatorError`.
///
/// One-for-one: this adapter classifies at the site that reads the signal
/// (a status code, a `finish_reason`, a `status`, a refusal field, the size
/// cap) and this function only renames. Picking a variant there is still the
/// retry decision — `Transport` and `RateLimitedAfter` are the retryable
/// ones (see the `client::classify` module doc) — and it is now
/// also the **taxonomy** decision a consumer reads off
/// `TranslatorError::stable_code()` (ti 1a85f3).
///
/// TRACE: SCN-12
pub fn map_provider_error(err: ProviderError) -> TranslatorError {
    match err {
        ProviderError::Transport(s) => TranslatorError::Network(s),
        ProviderError::Auth(s) => TranslatorError::Authentication(s),
        ProviderError::RateLimitedAfter(retry_after) => {
            TranslatorError::RateLimited { retry_after }
        }
        ProviderError::Malformed(s) => TranslatorError::MalformedResponse(s),
        ProviderError::ContentFiltered(s) => TranslatorError::ContentFiltered(s),
        ProviderError::OutputCeilingExhausted(s) => TranslatorError::OutputCeilingExhausted(s),
        ProviderError::ModelRefused(s) => TranslatorError::ModelRefused(s),
        ProviderError::ResponseTooLarge(s) => TranslatorError::ResponseTooLarge(s),
        ProviderError::Rejected { status, message } => {
            TranslatorError::ProviderRejected { status, message }
        }
        ProviderError::Other(s) => TranslatorError::Other(s),
    }
}
