//! Endpoint-URL construction for the Messages API.
//!
//! One path, so this module is the smallest of the four — but not absent:
//! the base URL is caller-supplied (an Anthropic-compatible gateway) and
//! therefore untrusted *shape-wise*, so it is normalized structurally rather
//! than by string concatenation.
//!
//! TRACE: DCR-0029
//! TRACE: SCN-12

use std::sync::LazyLock;

use transync::llm::TranslatorError;
use url::Url;

use crate::DEFAULT_BASE_URL;

/// The only endpoint path this crate posts to.
pub(super) const MESSAGES_PATH: &str = "/v1/messages";

/// R0011-0073: [`DEFAULT_BASE_URL`] is a compile-time constant, so its parse
/// is process-lifetime work, not per-request work — every batch used to
/// re-derive the same answer. Parsing it once here also retires the
/// unreachable `malformed default base_url` arm: a constant that failed to
/// parse is a defect this crate's own tests hit before a caller ever could,
/// not a runtime error to hand back to someone who configured nothing.
static DEFAULT_URL: LazyLock<Url> =
    LazyLock::new(|| Url::parse(DEFAULT_BASE_URL).expect("the default base URL is a constant"));

/// Join [`MESSAGES_PATH`] onto the configured base URL (or the provider
/// default).
pub(super) fn build_endpoint(base_url: Option<&Url>) -> Result<Url, TranslatorError> {
    // Manipulate the URL structurally instead of gluing strings — a base URL
    // carrying a query or fragment (`https://gateway?key=x`) would otherwise
    // swallow the endpoint path into the query string.
    let mut url = match base_url {
        Some(u) => u.clone(),
        None => DEFAULT_URL.clone(),
    };
    url.set_query(None);
    url.set_fragment(None);
    let mut base_path = url.path().trim_end_matches('/').to_string();
    // A gateway base URL that already ends in `/v1` — the natural form for a
    // compatible proxy — would otherwise produce `…/v1/v1/messages`. Strip a
    // trailing `/v1` from the base before joining so both `https://gw/v1` and
    // `https://gw` work the same.
    if base_path.ends_with("/v1") {
        base_path.truncate(base_path.len() - 3);
    }
    url.set_path(&format!("{base_path}{MESSAGES_PATH}"));
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_endpoint_default_base() {
        let url = build_endpoint(None).expect("builds");
        assert_eq!(url.as_str(), "https://api.anthropic.com/v1/messages");
    }

    /// A gateway base already ending in `/v1` must not produce `/v1/v1/…`.
    #[test]
    fn build_endpoint_dedupes_gateway_v1() {
        let base = Url::parse("https://gateway.example/v1").unwrap();
        let url = build_endpoint(Some(&base)).expect("builds");
        assert_eq!(url.as_str(), "https://gateway.example/v1/messages");
        // …and a trailing slash on that same base is the same base.
        let slashed = Url::parse("https://gateway.example/v1/").unwrap();
        assert_eq!(build_endpoint(Some(&slashed)).expect("builds"), url);
    }

    /// A base URL carrying a query or fragment must not swallow the path.
    #[test]
    fn build_endpoint_strips_query_and_fragment() {
        let base = Url::parse("https://gateway.example/sub?key=x#frag").unwrap();
        let url = build_endpoint(Some(&base)).expect("builds");
        assert_eq!(url.as_str(), "https://gateway.example/sub/v1/messages");
    }
}
