//! `x-api-key`-authenticated JSON POST with a hard ceiling on the response
//! body.
//!
//! Deliberately thin: [`post_json`] sends the request, caps the response
//! size, and accumulates the bytes. Deciding what a non-success status
//! *means* — retryable or terminal — belongs to [`super::classify`], which
//! owns that table on its own so the policy is testable without a socket.
//!
//! Two headers are this provider's, and both are non-negotiable: the
//! credential goes in **`x-api-key`** (not `Authorization: Bearer`), and
//! every request declares the protocol version in **`anthropic-version`**.
//! The version is a crate constant rather than a per-instance axis, which is
//! why [`crate::TransyncAnthropic::fingerprint`] leaves it out.
//!
//! TRACE: DCR-0029
//! TRACE: SCN-12

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use transync::llm::TranslatorError;
use url::Url;

use crate::ANTHROPIC_VERSION;
use crate::error::{ProviderError, map_provider_error};

use super::classify;

/// Hard ceiling on a single **answer** body. A well-formed batch result is
/// well under a megabyte; a body larger than this is a misbehaving or hostile
/// proxy, not a legitimate answer, so [`post_json`] refuses to buffer past it
/// instead of letting memory grow with whatever the far end streams.
pub(super) const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

/// The ceiling on a **non-success** body, which is read for one purpose only
/// — the 512-byte diagnostic excerpt. Buffering a 4xx/5xx to the same 32 MiB
/// an answer gets pays for bytes nothing will ever look at, so an error body
/// stops here instead: 64 KiB is orders of magnitude above any real
/// `{"error": …}` envelope and orders of magnitude below the answer cap.
///
/// Hitting it is **not** a failure — the read simply stops and the response
/// is dropped, which ends the transfer — because the **status** is the
/// classification, and an oversized error body must not turn a retryable 529
/// into a terminal oversize error.
pub(super) const MAX_ERROR_RESPONSE_BYTES: u64 = 64 * 1024;

/// POST `body` to `endpoint` with this provider's two headers.
///
/// The request timeout comes from the shared `reqwest::Client` the adapter
/// holds; see `crate::build_http_client`.
///
/// An **answer** body is bounded at [`MAX_RESPONSE_BYTES`]: a declared
/// `Content-Length` over the cap is rejected before a single byte is read,
/// and otherwise the body is accumulated chunk-by-chunk with the running
/// total checked, so a chunked/streamed body cannot grow past the cap either.
/// Overshooting is TERMINAL — an oversized body will not shrink on retry, so
/// retrying only burns quota.
///
/// A **non-success** body is bounded far tighter, at
/// [`MAX_ERROR_RESPONSE_BYTES`], and reaching that ceiling is not an error at
/// all: the read stops, the rest is never transferred, and the status
/// classifies the failure as it always would. The excerpt therefore reports
/// the bytes that were *read*, which for a body over the ceiling is the
/// ceiling rather than whatever the far end intended to send.
pub(super) async fn post_json<T: Serialize + ?Sized>(
    client: &reqwest::Client,
    api_key: &SecretString,
    endpoint: &Url,
    body: &T,
) -> Result<Vec<u8>, TranslatorError> {
    let mut response = client
        .post(endpoint.clone())
        .header("x-api-key", api_key.expose_secret())
        .header("anthropic-version", ANTHROPIC_VERSION)
        .json(body)
        .send()
        .await
        .map_err(|e| map_provider_error(classify::map_reqwest_error(e)))?;

    let status = response.status();
    let retry_after =
        classify::parse_retry_after(response.headers().get(reqwest::header::RETRY_AFTER));

    // An answer is read to hold it, an error body only to quote 512 bytes of
    // it, so the two get different ceilings.
    let cap = if status.is_success() {
        MAX_RESPONSE_BYTES
    } else {
        MAX_ERROR_RESPONSE_BYTES
    };

    // Reject an over-cap *answer* up front when the length is declared, so a
    // huge honest response never even starts buffering. A non-success body is
    // truncated instead of refused: the status already says what the failure
    // is, and refusing here would replace a retryable 529 with a terminal
    // oversize error on the strength of how much text the proxy attached.
    if status.is_success()
        && let Some(len) = response.content_length()
        && len > MAX_RESPONSE_BYTES
    {
        return Err(map_provider_error(classify::oversize_response_error(Some(
            len,
        ))));
    }

    // Pre-size for the common case; a missing/over-cap length falls back to
    // an empty buffer that the streaming cap below still guards.
    let mut bytes: Vec<u8> = match response.content_length() {
        Some(len) if len <= cap => Vec::with_capacity(len as usize),
        _ => Vec::new(),
    };
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|e| map_provider_error(ProviderError::Transport(e.to_string())))?
    {
        if bytes.len() as u64 + chunk.len() as u64 > cap {
            if !status.is_success() {
                // Fill the diagnostic ceiling and stop reading; dropping the
                // response ends the transfer.
                let room = usize::try_from(cap)
                    .unwrap_or(usize::MAX)
                    .saturating_sub(bytes.len());
                bytes.extend_from_slice(&chunk[..room.min(chunk.len())]);
                break;
            }
            return Err(map_provider_error(classify::oversize_response_error(None)));
        }
        bytes.extend_from_slice(&chunk);
    }

    if !status.is_success() {
        return Err(map_provider_error(classify::provider_error_for_status(
            status,
            retry_after,
            &bytes,
        )));
    }

    Ok(bytes)
}

#[cfg(test)]
pub(super) mod loopback {
    //! A one-shot HTTP server on loopback, shared by this module's transport
    //! tests and the flow's end-to-end test. One copy so both exercise the
    //! same server behavior.

    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// Serve one request: read it, answer with `status_line`, a **declared**
    /// `Content-Length` of `declared`, and `sent` bytes of `filler`, then
    /// close. Declared and sent are independent on purpose — the point of the
    /// cap tests is what the client does with a declaration it should not
    /// trust and a body it should not finish. Returns the bound address.
    pub(in crate::client) fn serve_filler(
        status_line: &'static str,
        declared: u64,
        sent: usize,
    ) -> String {
        serve(status_line, declared, Body::Filler(sent), &[])
    }

    /// Serve one request with an exact body and honest `Content-Length`, plus
    /// any extra headers. Returns the bound address.
    pub(in crate::client) fn serve_body(
        status_line: &'static str,
        body: &'static str,
        extra_headers: &'static [&'static str],
    ) -> String {
        serve(
            status_line,
            body.len() as u64,
            Body::Exact(body),
            extra_headers,
        )
    }

    enum Body {
        Filler(usize),
        Exact(&'static str),
    }

    fn serve(
        status_line: &'static str,
        declared: u64,
        body: Body,
        extra_headers: &'static [&'static str],
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            // The request is a JSON POST; drain enough of it that the client
            // is never blocked writing.
            let mut scratch = [0u8; 65536];
            let _ = stream.read(&mut scratch);
            let mut header = format!("{status_line}\r\nContent-Length: {declared}\r\n");
            for extra in extra_headers {
                header.push_str(extra);
                header.push_str("\r\n");
            }
            header.push_str("\r\n");
            if stream.write_all(header.as_bytes()).is_err() {
                return;
            }
            match body {
                Body::Exact(text) => {
                    let _ = stream.write_all(text.as_bytes());
                }
                Body::Filler(sent) => {
                    let block = vec![b'x'; 16 * 1024];
                    let mut written = 0usize;
                    while written < sent {
                        let n = block.len().min(sent - written);
                        if stream.write_all(&block[..n]).is_err() {
                            return;
                        }
                        written += n;
                    }
                }
            }
            let _ = stream.flush();
        });
        addr
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use loopback::{serve_body, serve_filler};

    fn key() -> SecretString {
        SecretString::new("sk-ant-test".to_string().into())
    }

    async fn post_to(addr: &str) -> Result<Vec<u8>, TranslatorError> {
        let client = crate::build_http_client();
        let endpoint = Url::parse(&format!("http://{addr}/v1/messages")).expect("endpoint parses");
        post_json(
            &client,
            &key(),
            &endpoint,
            &serde_json::json!({ "ping": 1 }),
        )
        .await
    }

    /// The two headers this provider requires, observed on the wire rather
    /// than asserted about the code that sets them. `Authorization: Bearer`
    /// — the shipped adapter's scheme — must be absent: sending it here
    /// authenticates nothing and would put the credential somewhere this
    /// provider does not read.
    #[tokio::test]
    async fn the_request_carries_x_api_key_and_the_protocol_version() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        let seen = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut scratch = [0u8; 65536];
            let n = stream.read(&mut scratch).expect("read request");
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            let _ = stream.flush();
            String::from_utf8_lossy(&scratch[..n]).to_ascii_lowercase()
        });

        post_to(&addr).await.expect("200 is a success");
        let request = seen.join().expect("server thread");

        assert!(
            request.contains("x-api-key: sk-ant-test"),
            "the credential must ride in x-api-key: {request}"
        );
        assert!(
            request.contains(&format!("anthropic-version: {ANTHROPIC_VERSION}")),
            "every request must declare the protocol version: {request}"
        );
        assert!(
            !request.contains("authorization:"),
            "the bearer scheme authenticates nothing here and must not be sent: {request}"
        );
        assert!(
            request.contains("content-type: application/json"),
            "the body is JSON: {request}"
        );
    }

    /// A non-success body is read only for its diagnostic excerpt, so it is
    /// bounded at [`MAX_ERROR_RESPONSE_BYTES`] rather than at the 32 MiB
    /// answer cap — and reaching that ceiling is **not** a failure of its
    /// own. Refusing here would replace the retryable classification the
    /// status has already earned with a verdict about how much text the proxy
    /// attached, which for this provider's overload signal is exactly the
    /// wrong trade.
    #[tokio::test]
    async fn an_oversized_error_body_is_truncated_not_reclassified() {
        let addr = serve_filler("HTTP/1.1 529 Overloaded", 40 * 1024 * 1024, 256 * 1024);
        let err = post_to(&addr).await.expect_err("529 is an error");
        match &err {
            TranslatorError::Network(s) => {
                assert!(s.contains("529"), "must name the status, got {s:?}");
                assert!(
                    !s.contains("cap"),
                    "the status classifies it, not the body size, got {s:?}"
                );
            }
            other => panic!("a 529 must stay retryable, got {other:?}"),
        }
    }

    /// The streaming half: an honestly-declared error body under the answer
    /// cap must not be buffered whole — the read stops at
    /// [`MAX_ERROR_RESPONSE_BYTES`], which the excerpt's own byte count
    /// shows, because it reports what was *read*.
    #[tokio::test]
    async fn an_error_body_is_read_only_to_the_diagnostic_ceiling() {
        let sent = 256 * 1024;
        let addr = serve_filler("HTTP/1.1 500 Internal Server Error", sent as u64, sent);
        let err = post_to(&addr).await.expect_err("500 is an error");
        let TranslatorError::Network(s) = &err else {
            panic!("a 500 must stay retryable, got {err:?}");
        };
        assert!(
            s.contains(&format!(
                "({MAX_ERROR_RESPONSE_BYTES} bytes total, truncated)"
            )),
            "the read must stop at the diagnostic ceiling, got {s:?}"
        );
    }

    /// The answer path is untouched by the above: a **success** body that
    /// declares more than [`MAX_RESPONSE_BYTES`] is refused before a byte is
    /// read, and terminally.
    #[tokio::test]
    async fn an_oversized_success_body_is_refused_up_front() {
        let addr = serve_filler("HTTP/1.1 200 OK", 40 * 1024 * 1024, 0);
        let err = post_to(&addr)
            .await
            .expect_err("an over-cap answer is refused");
        assert!(
            matches!(&err, TranslatorError::ResponseTooLarge(s)
                if s.contains("32 MiB cap") && s.contains("Content-Length")),
            "got {err:?}"
        );
        assert_eq!(err.stable_code(), "provider_response_too_large");
    }

    /// The classify table reached through a real socket, not just called
    /// directly: a 429 with a `retry-after` header arrives as `RateLimited`
    /// carrying the parsed hint, which is what lets the pipeline pace from
    /// the provider's own guidance.
    #[tokio::test]
    async fn a_rate_limit_arrives_with_its_parsed_hint() {
        let addr = serve_body(
            "HTTP/1.1 429 Too Many Requests",
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"},"request_id":"req_rl"}"#,
            &["retry-after: 120"],
        );
        let err = post_to(&addr).await.expect_err("429 is an error");
        assert!(
            matches!(&err, TranslatorError::RateLimited { retry_after: Some(d) }
                if *d == std::time::Duration::from_secs(120)),
            "got {err:?}"
        );
    }

    /// …and a 401 arrives as terminal authentication, with the provider's own
    /// envelope quoted and the key nowhere in sight.
    #[tokio::test]
    async fn an_auth_failure_is_terminal_and_never_echoes_the_key() {
        let addr = serve_body(
            "HTTP/1.1 401 Unauthorized",
            r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_auth"}"#,
            &[],
        );
        let err = post_to(&addr).await.expect_err("401 is an error");
        let TranslatorError::Authentication(s) = &err else {
            panic!("401 must be terminal auth, got {err:?}");
        };
        assert!(s.contains("authentication_error"), "got {s:?}");
        assert!(s.contains("req_auth"), "got {s:?}");
        assert!(
            !s.contains("sk-ant-test"),
            "the credential must never reach a diagnostic: {s:?}"
        );
    }

    /// A 200 returns its bytes verbatim — the transport reads, it does not
    /// interpret.
    #[tokio::test]
    async fn a_success_returns_the_body_bytes_unchanged() {
        let addr = serve_body("HTTP/1.1 200 OK", r#"{"content":[]}"#, &[]);
        let bytes = post_to(&addr).await.expect("200 is a success");
        assert_eq!(bytes, br#"{"content":[]}"#);
    }

    /// R0004-0001, the security claim itself, observed on two sockets rather
    /// than asserted about a builder: an endpoint that answers the Messages
    /// POST with a cross-origin `302` must not get the api key delivered to
    /// the target it named.
    ///
    /// The credential rides in the custom `x-api-key` header, and `reqwest`
    /// strips only its own fixed sensitive set — `Authorization`, `Cookie`,
    /// `Proxy-Authorization`, `Www-Authenticate` — on a cross-origin hop, so
    /// under the default follow-up-to-ten policy this key would have travelled
    /// to `origin_b` together with the document body.
    ///
    /// The second listener answers `200 OK`, so a regression fails **fast and
    /// loudly** — the follow-up succeeds, `post_json` returns `Ok`, and the
    /// `expect_err` below fires — instead of hanging until the request budget
    /// expires.
    #[tokio::test]
    async fn a_cross_origin_redirect_is_refused_and_the_key_never_reaches_the_target() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc;

        // origin_b: the redirect target. Answers 200 and reports what it saw.
        let target = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let target_addr = target.local_addr().expect("bound address").to_string();
        let (tx, rx) = mpsc::channel();
        // Deliberately not joined: on the passing path nothing ever connects,
        // so this thread stays parked on `accept` until the test binary
        // exits. Joining it would be the hang the test exists to prevent.
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = target.accept() else {
                return;
            };
            let mut scratch = [0u8; 65536];
            let n = stream.read(&mut scratch).unwrap_or(0);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
            let _ = stream.flush();
            let _ = tx.send(String::from_utf8_lossy(&scratch[..n]).to_ascii_lowercase());
        });

        // origin_a: the configured endpoint. Redirects to origin_b.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        let location = format!("http://{target_addr}/v1/messages");
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut scratch = [0u8; 65536];
            let _ = stream.read(&mut scratch);
            let _ = stream.write_all(
                format!("HTTP/1.1 302 Found\r\nLocation: {location}\r\nContent-Length: 0\r\n\r\n")
                    .as_bytes(),
            );
            let _ = stream.flush();
        });

        let err = post_to(&addr)
            .await
            .expect_err("a redirect is not an answer");

        assert!(
            matches!(
                &err,
                TranslatorError::ProviderRejected {
                    status: Some(302),
                    ..
                }
            ),
            "an unfollowed redirect must be terminal and carry its status, got {err:?}"
        );
        let reached_target = rx.try_recv();
        assert!(
            reached_target.is_err(),
            "the api key must never reach the redirect target, but it saw: {reached_target:?}"
        );
    }

    /// R0004-0022: a request that cannot even be built is decided by the
    /// request itself, so ADR-0009's verbatim resubmission reproduces it
    /// exactly. It must arrive terminal rather than as the retryable
    /// `Network` the catch-all used to hand back.
    #[tokio::test]
    async fn a_request_that_cannot_be_built_is_terminal() {
        // A map with tuple keys is not representable as JSON, so `.json()`
        // fails and `send()` surfaces a builder-kind `reqwest::Error`.
        let mut unserializable = std::collections::HashMap::new();
        unserializable.insert((1u8, 2u8), 3u8);

        let client = crate::build_http_client();
        let endpoint = Url::parse("http://127.0.0.1:1/v1/messages").expect("endpoint parses");
        let err = post_json(&client, &key(), &endpoint, &unserializable)
            .await
            .expect_err("an unserializable body cannot be sent");

        assert!(
            matches!(&err, TranslatorError::Other(_)),
            "a deterministic build failure must not be retried, got {err:?}"
        );
        assert!(
            !matches!(&err, TranslatorError::Network(_)),
            "the retryable class is for transient causes only, got {err:?}"
        );
    }

    /// R0004-0022, the other half: the terminal classes must stay narrow.
    ///
    /// `reqwest`'s async client wraps **every** in-flight failure the
    /// underlying hyper service reports into `Kind::Request`, so
    /// `is_request()` is not a class of deterministic causes — a server that
    /// closes the socket before answering lands there with neither a
    /// connect-phase source (`is_connect` false) nor a `TimedOut` one
    /// (`is_timeout` false). That is the textbook transient fault, and the
    /// pipeline re-dispatches only `Network`/`RateLimited`, so classifying it
    /// terminal would drop the unit to source content on the first blip
    /// instead of spending ADR-0009's retry budget on it.
    #[tokio::test]
    async fn a_connection_dropped_in_flight_stays_retryable() {
        use std::io::Read;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            // Drain the request so the client completes its write, then drop
            // the socket without a status line: the answer never starts.
            let mut scratch = [0u8; 8192];
            let _ = stream.read(&mut scratch);
            drop(stream);
        });

        let err = post_to(&addr)
            .await
            .expect_err("a dropped connection is not an answer");

        assert!(
            matches!(&err, TranslatorError::Network(_)),
            "a socket dropped mid-request is transient and must stay retryable, got {err:?}"
        );
    }
}
