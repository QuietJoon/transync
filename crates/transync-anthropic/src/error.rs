//! Map provider-specific errors back to the `transync::TranslatorError`
//! variants in `contracts.md` §1.
//!
//! The split mirrors `transync-openai`'s `error` module and exists for the
//! same reason: classification happens at the site that reads the signal — a
//! status code, a `stop_reason`, a `stop_details` category, the size cap —
//! and [`map_provider_error`] only renames. Picking a [`ProviderError`]
//! variant *is* the retry decision (core re-dispatches exactly `Network` and
//! `RateLimited`) and *is* the taxonomy decision a consumer reads off
//! `TranslatorError::stable_code()`.
//!
//! TRACE: DCR-0029
//! TRACE: contracts.md §1

use std::time::Duration;
use transync::llm::TranslatorError;

/// The 512-byte cap every provider-controlled string obeys before it can
/// reach stderr, the logs, or a run's report artifacts.
///
/// It lives here rather than beside the status table because two unrelated
/// readers need it — the envelope reader (a refusal explanation) and the
/// transport classifier (an error body) — and both are *constructing
/// diagnostics*, which is this module's subject. Unbounded, a hostile or
/// misbehaving gateway could expand one refusal to whatever the 32 MiB
/// response cap allows and push all of it downstream.
pub(crate) fn truncate_diagnostic(bytes: &[u8]) -> String {
    const MAX: usize = 512;
    let text = String::from_utf8_lossy(bytes);
    if text.len() <= MAX {
        return text.into_owned();
    }
    let mut end = MAX;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… ({} bytes total, truncated)", &text[..end], bytes.len())
}

/// Errors raised by this crate's Messages-API wrapper before they are
/// mapped to the `transync::TranslatorError` surface.
///
/// TRACE: DCR-0029
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("transport: {0}")]
    Transport(String),

    #[error("authentication: {0}")]
    Auth(String),

    /// Rate-limit response, carrying whatever the `retry-after` header
    /// parsed to (including its absence) so the pipeline can pace from the
    /// provider's own guidance.
    #[error("rate limited (retry after {0:?})")]
    RateLimitedAfter(Option<Duration>),

    #[error("malformed: {0}")]
    Malformed(String),

    /// `stop_reason: "refusal"` **with** a `stop_details.category`: the
    /// provider's safety layer named a policy category.
    #[error("content filtered: {0}")]
    ContentFiltered(String),

    /// `stop_reason: "max_tokens"` — the required output ceiling was
    /// exhausted before the answer was complete.
    #[error("output ceiling exhausted: {0}")]
    OutputCeilingExhausted(String),

    /// `stop_reason: "refusal"` with **no** category: the model declined
    /// without a policy label.
    #[error("model refused: {0}")]
    ModelRefused(String),

    /// The answer body blew past the transport's answer cap.
    #[error("response too large: {0}")]
    ResponseTooLarge(String),

    /// `stop_reason: "model_context_window_exceeded"` — input plus requested
    /// output did not fit the model's context window.
    #[error("context window exceeded: {0}")]
    ContextWindowExceeded(String),

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
/// One-for-one renaming; the classification already happened where the
/// signal was read.
///
/// TRACE: DCR-0029
/// TRACE: contracts.md §1
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
        // DCR-0029's open fork, resolved to Option A: the cause has a
        // remediation of its own (the batching configuration), so under
        // DCR-0023's own criterion — a distinct cause with a distinct
        // remediation gets a name — it gets one rather than the catch-all.
        ProviderError::ContextWindowExceeded(s) => TranslatorError::ContextWindowExceeded(s),
        ProviderError::Rejected { status, message } => {
            TranslatorError::ProviderRejected { status, message }
        }
        ProviderError::Other(s) => TranslatorError::Other(s),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A short body passes through untouched; an over-long one is cut at a
    /// char boundary and annotated with the true byte count.
    #[test]
    fn truncate_diagnostic_caps_and_stays_char_safe() {
        assert_eq!(truncate_diagnostic(b"short"), "short");

        let long = "a".repeat(600);
        let capped = truncate_diagnostic(long.as_bytes());
        assert!(capped.starts_with(&"a".repeat(512)), "got {capped:?}");
        assert!(
            capped.ends_with("… (600 bytes total, truncated)"),
            "got {capped:?}"
        );

        // The 512-byte cut lands inside a 3-byte char; it must walk back to a
        // boundary rather than panic on a non-char-boundary slice.
        let mixed = format!("{}한글", "a".repeat(511));
        let capped = truncate_diagnostic(mixed.as_bytes());
        assert_eq!(
            capped,
            format!(
                "{}… ({} bytes total, truncated)",
                "a".repeat(511),
                mixed.len()
            )
        );
    }

    /// The renaming is total and each cause keeps its own stable code, so a
    /// consumer branching on the code sees this provider's causes as the same
    /// vocabulary the shipped adapter produces.
    #[test]
    fn every_provider_error_renames_to_its_own_taxonomy_code() {
        let cases = [
            (ProviderError::Transport("t".into()), "provider_network"),
            (ProviderError::Auth("a".into()), "provider_auth"),
            (
                ProviderError::RateLimitedAfter(Some(Duration::from_secs(5))),
                "provider_rate_limited",
            ),
            (
                ProviderError::Malformed("m".into()),
                "provider_malformed_response",
            ),
            (
                ProviderError::ContentFiltered("c".into()),
                "provider_content_filtered",
            ),
            (
                ProviderError::OutputCeilingExhausted("o".into()),
                "provider_output_ceiling_exhausted",
            ),
            (
                ProviderError::ModelRefused("r".into()),
                "provider_model_refused",
            ),
            (
                ProviderError::ResponseTooLarge("l".into()),
                "provider_response_too_large",
            ),
            (
                ProviderError::ContextWindowExceeded("w".into()),
                "provider_context_window_exceeded",
            ),
            (
                ProviderError::Rejected {
                    status: Some(404),
                    message: "HTTP 404".into(),
                },
                "provider_rejected",
            ),
            (ProviderError::Other("x".into()), "provider_error"),
        ];
        for (err, expected) in cases {
            let printed = format!("{err}");
            assert_eq!(
                map_provider_error(err).stable_code(),
                expected,
                "for {printed}"
            );
        }
    }

    /// DCR-0029's fork, resolved to Option A: a context-window stop reaches a
    /// consumer as its **own** cause with its own stable code — never
    /// `OutputCeilingExhausted`, which would point the operator at a knob that
    /// cannot help, and no longer the unclassified catch-all, which would
    /// point at no knob at all when there is one.
    #[test]
    fn the_context_window_stop_is_its_own_named_cause() {
        let mapped = map_provider_error(ProviderError::ContextWindowExceeded(
            "stop_reason: model_context_window_exceeded".into(),
        ));
        assert_eq!(mapped.stable_code(), "provider_context_window_exceeded");
        assert!(
            matches!(&mapped, TranslatorError::ContextWindowExceeded(s)
                if s.contains("model_context_window_exceeded")),
            "the reason must survive into the message, got {mapped:?}"
        );
        assert_ne!(
            mapped.stable_code(),
            TranslatorError::OutputCeilingExhausted("x".into()).stable_code(),
            "the two token-shaped causes name opposite knobs and must not share a code"
        );
        assert_ne!(
            mapped.stable_code(),
            TranslatorError::Other("x".into()).stable_code(),
            "a cause with a remediation must not read downstream as unknown"
        );
    }
}
