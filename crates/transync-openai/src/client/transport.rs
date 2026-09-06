//! Bearer-authenticated JSON POST with a hard ceiling on the response body.
//!
//! This module is deliberately thin: [`post_json`] sends the request, caps
//! the response size, and accumulates the bytes. Deciding what a non-success
//! status *means* — retryable or terminal — belongs to [`super::classify`],
//! which owns that table on its own so the policy is testable without a
//! socket.
//!
//! Split out of `client.rs` (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`).
//!
//! TRACE: SCN-12

use secrecy::{ExposeSecret, SecretString};
use serde::Serialize;
use transync::llm::TranslatorError;
use url::Url;

use crate::error::{ProviderError, map_provider_error};

use super::classify;

/// EXT-2026-07 P1-7 (OI-0019): hard ceiling on a single response body.
/// A well-formed batch result is well under a megabyte; a body larger
/// than this is a misbehaving or hostile proxy, not a legitimate answer,
/// so [`post_json`] refuses to buffer past it instead of letting memory
/// grow with whatever the far end streams.
pub(super) const MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024;

/// R0002-0038: the ceiling on a **non-success** body, which is read for one
/// purpose only — the 512-byte diagnostic excerpt
/// [`classify::truncate_diagnostic`] keeps. Buffering a 4xx/5xx to the same
/// 32 MiB an answer gets pays for bytes nothing will ever look at, so an
/// error body stops here instead: 64 KiB is orders of magnitude above any
/// real `{"error": …}` object and orders of magnitude below the answer cap.
/// Hitting it is not a failure — the read simply stops and the response is
/// dropped, which ends the transfer too — because the **status** is the
/// classification, and an oversized error body must not turn a retryable 503
/// into a terminal oversize error.
pub(super) const MAX_ERROR_RESPONSE_BYTES: u64 = 64 * 1024;

/// POST `body` to `endpoint` with bearer auth (the request timeout comes
/// from the shared `reqwest::Client`; see `crate::build_http_client`).
/// Returns the raw response bytes; a non-success status is handed to
/// [`classify::provider_error_for_status`] so every transport surface
/// produces the same `TranslatorError` variants.
///
/// EXT-2026-07 P1-7 (OI-0019): an **answer** body is bounded at
/// [`MAX_RESPONSE_BYTES`]. A declared `Content-Length` over the cap is
/// rejected before a single byte is read; otherwise the body is accumulated
/// chunk-by-chunk and the running total is checked so a chunked/streamed body
/// cannot grow past the cap either. Overshooting is a TERMINAL error
/// (`Other`) — an oversized body will not shrink on retry, so retrying only
/// burns quota.
///
/// R0002-0038: a **non-success** body is bounded far tighter, at
/// [`MAX_ERROR_RESPONSE_BYTES`], and reaching that ceiling is not an error at
/// all — the read stops, the rest is never transferred, and the status
/// classifies the failure as it always would. The excerpt
/// [`classify::provider_error_for_status`] carries therefore reports the
/// bytes that were *read*, which for a body over the ceiling is the ceiling
/// rather than whatever the far end intended to send.
pub(super) async fn post_json<T: Serialize + ?Sized>(
    client: &reqwest::Client,
    api_key: &SecretString,
    endpoint: &Url,
    body: &T,
) -> Result<Vec<u8>, TranslatorError> {
    let mut response = client
        .post(endpoint.clone())
        .bearer_auth(api_key.expose_secret())
        .json(body)
        .send()
        .await
        .map_err(|e| map_provider_error(classify::map_reqwest_error(e)))?;

    let status = response.status();
    let retry_after =
        classify::parse_retry_after(response.headers().get(reqwest::header::RETRY_AFTER));

    // R0002-0038: an answer is read to hold it, an error body only to quote
    // 512 bytes of it, so the two get different ceilings.
    let cap = if status.is_success() {
        MAX_RESPONSE_BYTES
    } else {
        MAX_ERROR_RESPONSE_BYTES
    };

    // Reject an over-cap *answer* up front when the length is declared, so a
    // huge honest response never even starts buffering. A non-success body is
    // truncated instead of refused: the status already says what the failure
    // is, and refusing here would replace a retryable 503 with a terminal
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
    loop {
        // R0011-0026: a body read that dies mid-stream under a non-success
        // status is still that status. Both the status and its `Retry-After`
        // were read before the body started, so handing back a bare
        // `Transport` string here would demote a paced 429 to an unpaced
        // `Network` — strictly less than the headers already permit, and the
        // pipeline paces from that hint. Under a *success* status there is
        // nothing to inherit: the transfer itself is the failure.
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(_) if !status.is_success() => {
                return Err(map_provider_error(classify::provider_error_for_status(
                    status,
                    retry_after,
                    &bytes,
                )));
            }
            Err(e) => {
                return Err(map_provider_error(ProviderError::Transport(e.to_string())));
            }
        };
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
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-shot HTTP server on loopback: read the request, answer with
    /// `status_line`, a **declared** `Content-Length` of `declared`, and
    /// `sent` bytes of body, then close. Declared and sent are independent on
    /// purpose — the point of these tests is what the client does with a
    /// declaration it should not trust and a body it should not finish.
    /// Returns the bound address.
    fn serve_once(status_line: &'static str, declared: u64, sent: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            // The request is a small JSON POST; one read drains enough of it
            // that the client is never blocked writing.
            let mut scratch = [0u8; 8192];
            let _ = stream.read(&mut scratch);
            let header = format!("{status_line}\r\nContent-Length: {declared}\r\n\r\n");
            if stream.write_all(header.as_bytes()).is_err() {
                return;
            }
            let block = vec![b'x'; 16 * 1024];
            let mut written = 0usize;
            while written < sent {
                let n = block.len().min(sent - written);
                if stream.write_all(&block[..n]).is_err() {
                    return;
                }
                written += n;
            }
            let _ = stream.flush();
        });
        addr
    }

    async fn post_to(addr: &str) -> Result<Vec<u8>, TranslatorError> {
        let client = crate::build_http_client();
        let key = SecretString::new("sk-test".to_string().into());
        let endpoint = Url::parse(&format!("http://{addr}/v1/test")).expect("endpoint parses");
        post_json(&client, &key, &endpoint, &serde_json::json!({ "ping": 1 })).await
    }

    /// R0002-0038: a non-success body is read only for its diagnostic
    /// excerpt, so it is bounded at [`MAX_ERROR_RESPONSE_BYTES`] rather than
    /// at the 32 MiB answer cap — and reaching that ceiling is **not** a
    /// failure of its own. Before this, a 503 whose `Content-Length` claimed
    /// more than the answer cap was refused up front as a terminal
    /// `Other("response exceeded 32 MiB cap …")`, replacing the retryable
    /// classification the status had already earned with a verdict about how
    /// much text the proxy attached.
    #[tokio::test]
    async fn an_oversized_error_body_is_truncated_not_reclassified() {
        let addr = serve_once(
            "HTTP/1.1 503 Service Unavailable",
            40 * 1024 * 1024,
            256 * 1024,
        );
        let err = post_to(&addr).await.expect_err("503 is an error");
        match &err {
            TranslatorError::Network(s) => {
                assert!(s.contains("503"), "must name the status, got {s:?}");
                assert!(
                    !s.contains("cap"),
                    "the status classifies it, not the body size, got {s:?}"
                );
            }
            other => panic!("a 503 must stay retryable, got {other:?}"),
        }
    }

    /// R0002-0038, the streaming half: an honestly-declared error body under
    /// the answer cap used to be buffered whole — 256 KiB read to quote 512
    /// bytes of it. The read now stops at [`MAX_ERROR_RESPONSE_BYTES`], which
    /// the excerpt's own byte count shows: it reports what was *read*.
    #[tokio::test]
    async fn an_error_body_is_read_only_to_the_diagnostic_ceiling() {
        let sent = 256 * 1024;
        let addr = serve_once("HTTP/1.1 500 Internal Server Error", sent as u64, sent);
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
    /// declares more than [`MAX_RESPONSE_BYTES`] is still refused before a
    /// byte is read, and still terminally (EXT-2026-07 P1-7 / OI-0019).
    #[tokio::test]
    async fn an_oversized_success_body_is_still_refused_up_front() {
        let addr = serve_once("HTTP/1.1 200 OK", 40 * 1024 * 1024, 0);
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

    /// R0004-0001, through two sockets: an endpoint that answers the POST
    /// with a cross-origin `302` does not get the request delivered to the
    /// target it named.
    ///
    /// The bearer credential this adapter sends *is* in the set `reqwest`
    /// strips on a cross-origin hop, so this crate was never the one that
    /// leaked — but the two provider clients now refuse redirects identically
    /// rather than one of them being safe by accident of which header its
    /// provider chose, and the document body would have travelled either way.
    ///
    /// The second listener answers `200 OK`, so a regression fails **fast and
    /// loudly** — the follow-up succeeds, `post_json` returns `Ok`, and the
    /// `expect_err` below fires — instead of hanging until the request budget
    /// expires.
    #[tokio::test]
    async fn a_cross_origin_redirect_is_refused_and_never_reaches_the_target() {
        use std::sync::mpsc;

        // The redirect target. Answers 200 and reports what it saw.
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

        // The configured endpoint. Redirects to the target.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        let location = format!("http://{target_addr}/v1/test");
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
            "the request must never reach the redirect target, but it saw: {reached_target:?}"
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
        let key = SecretString::new("sk-test".to_string().into());
        let endpoint = Url::parse("http://127.0.0.1:1/v1/test").expect("endpoint parses");
        let err = post_json(&client, &key, &endpoint, &unserializable)
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

    /// R0011-0026: the status and its `Retry-After` are read before the body
    /// starts, so a body that dies mid-stream must not throw them away. A 429
    /// whose connection goes down after the header block is still a 429 with
    /// a pacing hint; the bare `Transport` string this used to return arrives
    /// as an unpaced `Network`, which re-dispatches on the pipeline's own
    /// backoff instead of the one the provider asked for.
    #[tokio::test]
    async fn a_body_read_failure_keeps_the_status_and_its_hint() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let addr = listener.local_addr().expect("bound address").to_string();
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut scratch = [0u8; 8192];
            let _ = stream.read(&mut scratch);
            // 1 KiB declared, 16 bytes delivered, then the socket goes away:
            // the status and headers are already in hand when `chunk()` fails.
            let _ = stream.write_all(
                b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 42\r\n\
                  Content-Length: 1024\r\n\r\nxxxxxxxxxxxxxxxx",
            );
            let _ = stream.flush();
        });

        let err = post_to(&addr)
            .await
            .expect_err("a half-delivered body is not an answer");

        assert!(
            matches!(&err, TranslatorError::RateLimited { retry_after: Some(d) }
                if *d == std::time::Duration::from_secs(42)),
            "a truncated error body must keep the status-derived class and its hint, got {err:?}"
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
