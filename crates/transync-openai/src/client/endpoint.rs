//! Endpoint-URL construction for the OpenAI HTTP surfaces.
//!
//! Split out of `client.rs` (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`). The base URL is
//! caller-supplied (an OpenAI-compatible proxy) and therefore untrusted
//! shape-wise: this module normalizes it structurally rather than by string
//! concatenation.
//!
//! TRACE: SCN-12

use transync::llm::TranslatorError;
use url::Url;

use crate::error::{ProviderError, map_provider_error};

/// Where the endpoints live when the caller configures no base URL.
pub(crate) const DEFAULT_BASE_URL: &str = "https://api.openai.com";
/// The two endpoint paths, next to the joining rule that has to keep their
/// `/v1` prefix from being doubled by a proxy base that already carries one.
pub(super) const RESPONSES_PATH: &str = "/v1/responses";
pub(super) const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// Join `path` onto the configured base URL (or the OpenAI default).
pub(super) fn build_endpoint(base_url: Option<&Url>, path: &str) -> Result<Url, TranslatorError> {
    // R0006-0044: manipulate the URL structurally instead of gluing
    // strings — a base URL carrying a query or fragment
    // (`https://proxy?key=x`) would otherwise swallow the endpoint path
    // into the query string.
    let mut url = match base_url {
        Some(u) => u.clone(),
        None => Url::parse(DEFAULT_BASE_URL).map_err(|e| {
            map_provider_error(ProviderError::Other(format!(
                "malformed default base_url: {e}"
            )))
        })?,
    };
    url.set_query(None);
    url.set_fragment(None);
    let mut base_path = url.path().trim_end_matches('/').to_string();
    // R0003-0070: a user-supplied --base-url that already ends in
    // `/v1` (the natural form for an OpenAI-compatible proxy) would
    // produce e.g. `https://proxy.example/v1/v1/chat/completions`
    // when concatenated with our `/v1/...` path. Strip a trailing
    // `/v1` from the base before joining so both `https://proxy/v1`
    // and `https://proxy` work the same.
    if base_path.ends_with("/v1") {
        base_path.truncate(base_path.len() - 3);
    }
    url.set_path(&format!("{base_path}{path}"));
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_endpoint_default_base() {
        let url = build_endpoint(None, RESPONSES_PATH).expect("builds");
        assert_eq!(url.as_str(), "https://api.openai.com/v1/responses");
    }

    /// R0003-0070 pin: a proxy base already ending in `/v1` must not
    /// produce `/v1/v1/...`.
    #[test]
    fn build_endpoint_dedupes_proxy_v1() {
        let base = Url::parse("https://proxy.example/v1").unwrap();
        let url = build_endpoint(Some(&base), CHAT_COMPLETIONS_PATH).expect("builds");
        assert_eq!(url.as_str(), "https://proxy.example/v1/chat/completions");
    }

    /// R0006-0044 pin: a base URL carrying a query or fragment must not
    /// swallow the endpoint path.
    #[test]
    fn build_endpoint_strips_query_and_fragment() {
        let base = Url::parse("https://proxy.example/sub?key=x#frag").unwrap();
        let url = build_endpoint(Some(&base), RESPONSES_PATH).expect("builds");
        assert_eq!(url.as_str(), "https://proxy.example/sub/v1/responses");
    }
}
