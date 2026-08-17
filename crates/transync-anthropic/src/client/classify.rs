//! Failure classification: HTTP status / `reqwest::Error` → [`ProviderError`].
//!
//! This module owns the **classification table**, and that table is this
//! adapter's *entire* retry contribution. `transync-core`'s
//! `pipeline::dispatch::translate_with_provider_retries` re-dispatches exactly
//! `TranslatorError::Network` and `TranslatorError::RateLimited` as bounded
//! verbatim resubmissions (ADR-0009, contracts.md §5), so picking a variant
//! here **is** declaring a failure retryable or terminal. The adapter runs no
//! retry loop of its own — an inner loop would multiply core's budgets — and
//! the policy itself (attempt budget, backoff, `Retry-After` honoring) lives
//! in core and is deliberately not duplicated.
//!
//! Every row is this provider's own evidence, not an analogy to the shipped
//! adapter's table. The one that would be missed by analogy is **529
//! `overloaded_error`**: this provider's overload signal, outside the
//! familiar 5xx range, named here and in a test rather than left to fall
//! through a `>499` arm by luck.
//!
//! TRACE: DCR-0029
//! TRACE: contracts.md §1
//! TRACE: contracts.md §8

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::error::{ProviderError, truncate_diagnostic};

use super::transport::MAX_RESPONSE_BYTES;

/// Classify a non-success HTTP response into a [`ProviderError`].
///
/// - **401** (`authentication_error`) / **403** (`permission_error`) →
///   terminal [`ProviderError::Auth`]. Both are credential-or-permission
///   faults and neither is fixed by trying again; the key is never echoed.
/// - **429** (`rate_limit_error`) → [`ProviderError::RateLimitedAfter`] with
///   whatever [`parse_retry_after`] made of the header.
/// - **408**, **409**, **425** → transient [`ProviderError::Transport`].
///   These are the retryable 4xx statuses, classified with 5xx rather than
///   as terminal rejections.
/// - **5xx, and 529 in particular** → transient. 529 is this provider's
///   `overloaded_error`; it sits outside the 500-599 band nothing else here
///   reaches, so it is matched by name.
/// - **any other 4xx** — 400 `invalid_request_error`, 404 `not_found_error`,
///   413 `request_too_large` — → terminal [`ProviderError::Rejected`],
///   carrying the status **as a number** as well as in the message. The
///   number is the point: 404 is a model name that does not exist and an
///   operator has to fix it, which a consumer could not tell from a 400
///   while the whole family arrived as one opaque string.
///
///   **This arm also carries context-window overflows that surface at the
///   HTTP layer** — an over-long prompt can be rejected as an invalid
///   request rather than answered with a stop reason. Same fact as the
///   `model_context_window_exceeded` stop reason, different door; the
///   status-based classification is already honest for it, and guessing at
///   the provider's prose to promote a 400 into the named cause would be
///   exactly the string-matching the taxonomy exists to end.
///
/// - **3xx** → terminal [`ProviderError::Rejected`] (R0004-0021). The client
///   this crate builds follows **no** redirects — `crate::http_client_builder_with`
///   explains why: the api key rides in a custom `x-api-key` header that
///   `reqwest` would not strip on a cross-origin hop — so a `3xx` is a
///   response this adapter is handed rather than an internal step. Nothing
///   about it changes on a verbatim resubmission: the same POST to the same
///   endpoint earns the same `Location`, so retrying spends the whole
///   bounded budget to arrive at the same answer later. It is the endpoint
///   configuration that has to change, which is what the message says.
///
/// `body` is the raw response bytes; the diagnostic excerpt is capped by
/// [`truncate_diagnostic`] so a hostile or misbehaving proxy cannot flood
/// stderr and logs with an echoed document.
pub(super) fn provider_error_for_status(
    status: reqwest::StatusCode,
    retry_after: Option<Duration>,
    body: &[u8],
) -> ProviderError {
    let text = describe_error_body(body);
    match status.as_u16() {
        401 | 403 => ProviderError::Auth(text),
        429 => ProviderError::RateLimitedAfter(retry_after),
        408 | 409 | 425 => ProviderError::Transport(format!("HTTP {status}: {text}")),
        // This provider's overload signal. Named rather than inherited: 529
        // is not in `reqwest::StatusCode`'s well-known set and is outside the
        // 5xx band, so nothing else in this table would have caught it.
        529 => ProviderError::Transport(format!("HTTP {status} (overloaded_error): {text}")),
        code @ 300..=399 => ProviderError::Rejected {
            status: Some(code),
            message: format!(
                "HTTP {status}: the endpoint answered the Messages POST with a redirect, which \
                 this client does not follow — the api key rides in the x-api-key header and \
                 would travel to the redirect target. Point base_url at the endpoint that \
                 answers directly. {text}"
            ),
        },
        code @ 400..=499 => ProviderError::Rejected {
            status: Some(code),
            message: format!("HTTP {status}: {text}"),
        },
        _ => ProviderError::Transport(format!("HTTP {status}: {text}")),
    }
}

/// Render a non-success body into the capped diagnostic excerpt.
///
/// This provider's error envelope is
/// `{"type":"error","error":{"type":…,"message":…},"request_id":…}`. When the
/// body parses as that shape the excerpt is built from its fields — including
/// **`request_id`**, which is the provider's own correlation handle and costs
/// nothing to carry. When it does not parse (a gateway with its own shape, an
/// HTML error page, truncated bytes), the raw excerpt is used, because the
/// status is the classification either way and a body this reader cannot
/// understand is still worth quoting.
fn describe_error_body(body: &[u8]) -> String {
    let Ok(envelope) = serde_json::from_slice::<ErrorEnvelope>(body) else {
        return truncate_diagnostic(body);
    };
    let Some(error) = envelope.error else {
        return truncate_diagnostic(body);
    };
    let mut described = match (error.ty.as_deref(), error.message.as_deref()) {
        (Some(ty), Some(message)) => format!("{ty}: {message}"),
        (Some(ty), None) => ty.to_string(),
        (None, Some(message)) => message.to_string(),
        (None, None) => return truncate_diagnostic(body),
    };
    if let Some(request_id) = envelope.request_id.as_deref().filter(|id| !id.is_empty()) {
        described.push_str(&format!(" (request_id: {request_id})"));
    }
    truncate_diagnostic(described.as_bytes())
}

#[derive(serde::Deserialize)]
struct ErrorEnvelope {
    #[serde(default)]
    error: Option<ErrorBody>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct ErrorBody {
    #[serde(rename = "type", default)]
    ty: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

/// Classify a `reqwest` failure raised before any status was seen.
///
/// Timeout and connect are the two genuinely transient causes and keep the
/// retryable [`ProviderError::Transport`]; a decode failure is a body this
/// reader could not make sense of, which is terminal for the same reason a
/// malformed envelope is.
///
/// **The two deterministic classes are terminal** (R0004-0022). ADR-0009's
/// retry is a *verbatim* resubmission — same payload, same scope — so a
/// failure decided entirely by the request itself produces the identical
/// failure on every attempt and can only spend the bounded budget before
/// surfacing later:
///
/// - `is_builder` — the request could not be constructed at all (an
///   unusable URL, a header value that is not valid). Nothing about it is
///   time-dependent.
/// - `is_redirect` — the redirect policy refused a hop. This client follows
///   no redirects, so in practice an unfollowed `3xx` arrives as a *response*
///   and is classified by [`provider_error_for_status`]; the arm is here so
///   the table stays total and so a future policy that errors instead of
///   stopping cannot silently land in the retryable catch-all.
///
/// They take [`ProviderError::Other`] rather than
/// [`ProviderError::Rejected`]: nothing reached the provider, so there is no
/// status to carry and "the provider rejected this" would be a false claim
/// about where the fault is.
///
/// `is_request` is deliberately **not** a third: it reports `reqwest`'s
/// `Kind::Request`, which the async client stamps on *every* failure the
/// underlying hyper service reports — a connect refusal, a socket the server
/// closed before answering, a reset after send, an `h2` GOAWAY. Only the
/// first of those is caught above by `is_connect`, which walks the source
/// chain; the rest are the textbook transient faults and carry neither a
/// connect-phase source nor a `TimedOut` one. Classifying that class terminal
/// would bypass the retry budget on the first blip, since
/// `pipeline::dispatch` re-dispatches only `Network`/`RateLimited`.
///
/// So anything left over keeps the retryable catch-all: an unrecognized cause
/// is more usefully retried once than declared permanent on a guess.
pub(super) fn map_reqwest_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() {
        ProviderError::Transport(format!("timeout: {err}"))
    } else if err.is_connect() {
        ProviderError::Transport(format!("connect: {err}"))
    } else if err.is_decode() {
        ProviderError::Malformed(err.to_string())
    } else if err.is_builder() {
        ProviderError::Other(format!(
            "request could not be built, which no resubmission changes — check base_url and the \
             configured model: {err}"
        ))
    } else if err.is_redirect() {
        ProviderError::Other(format!(
            "redirect policy refused the hop, which no resubmission changes — the api key rides \
             in the x-api-key header and would travel to the redirect target; point base_url at \
             the endpoint that answers directly: {err}"
        ))
    } else {
        ProviderError::Transport(err.to_string())
    }
}

/// Build the terminal error for a response that blows past
/// [`MAX_RESPONSE_BYTES`]. `declared` is the `Content-Length` when the cap was
/// tripped up front, or `None` when it was tripped mid-stream.
pub(super) fn oversize_response_error(declared: Option<u64>) -> ProviderError {
    const CAP_MIB: u64 = MAX_RESPONSE_BYTES / (1024 * 1024);
    match declared {
        Some(len) => ProviderError::ResponseTooLarge(format!(
            "response exceeded {CAP_MIB} MiB cap (Content-Length: {len} bytes)"
        )),
        None => ProviderError::ResponseTooLarge(format!(
            "response exceeded {CAP_MIB} MiB cap while streaming"
        )),
    }
}

/// Parse a `Retry-After` header value into a `Duration`.
///
/// Both RFC 7231 forms are honored: delta-seconds (`120`) and the HTTP-date
/// deadline (`Wed, 21 Oct 2015 07:28:00 GMT`), the latter resolved against
/// the system clock. Unparseable values — and a deadline already at or behind
/// the clock — produce `None`, which surfaces as
/// `RateLimited { retry_after: None }` and leaves the pipeline on its own
/// exponential backoff rather than turning a skewed clock into a hot retry
/// loop. Whatever is parsed is still subject to the pipeline's 30 s cap
/// (contracts.md §5): honoring the header changes *when* a retry is
/// dispatched, never what it contains.
///
/// TRACE: contracts.md §1
pub(super) fn parse_retry_after(header: Option<&reqwest::header::HeaderValue>) -> Option<Duration> {
    parse_retry_after_at(header, now_unix_secs())
}

/// [`parse_retry_after`] with the clock passed in, so the date arithmetic is
/// testable without one.
fn parse_retry_after_at(
    header: Option<&reqwest::header::HeaderValue>,
    now_unix: i64,
) -> Option<Duration> {
    let raw = header?.to_str().ok()?.trim();
    if let Ok(secs) = raw.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let deadline = parse_http_date(raw, now_unix)?;
    let delta = deadline.checked_sub(now_unix)?;
    Some(Duration::from_secs(
        u64::try_from(delta).ok().filter(|d| *d > 0)?,
    ))
}

/// Now, as whole seconds since the Unix epoch.
fn now_unix_secs() -> i64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(since) => i64::try_from(since.as_secs()).unwrap_or(i64::MAX),
        Err(before) => -i64::try_from(before.duration().as_secs()).unwrap_or(i64::MAX),
    }
}

/// Parse the three date forms RFC 7231 §7.1.1.1 requires a recipient to
/// accept, into whole seconds since the Unix epoch:
///
/// ```text
///   Sun, 06 Nov 1994 08:49:37 GMT     ; IMF-fixdate (the only form a sender may emit)
///   Sunday, 06-Nov-94 08:49:37 GMT    ; obsolete RFC 850
///   Sun Nov  6 08:49:37 1994          ; obsolete asctime
/// ```
///
/// Hand-rolled for the same reason the sibling adapter's is: this and that
/// one are the only dates in the whole workspace, and a `chrono`/`httpdate`
/// dependency for one header is not a trade worth making. All three forms are
/// UTC by definition, so no zone database is involved — only calendar
/// arithmetic.
fn parse_http_date(raw: &str, now_unix: i64) -> Option<i64> {
    let (year, month, day, time) = match raw.split_once(", ") {
        Some((day_name, rest)) => {
            if !is_day_name(day_name) {
                return None;
            }
            // The obsolete RFC 850 form is the only comma form with hyphens.
            if rest.contains('-') {
                rfc850_date(rest, now_unix)?
            } else {
                imf_fixdate(rest)?
            }
        }
        None => asctime_date(raw)?,
    };
    let (hour, minute, second) = time_of_day(time)?;
    if day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let secs_of_day = i64::from(hour) * 3600 + i64::from(minute) * 60 + i64::from(second);
    Some(days_from_civil(year, month, day) * 86_400 + secs_of_day)
}

/// `06 Nov 1994 08:49:37 GMT` — the day-name and its comma already consumed.
fn imf_fixdate(rest: &str) -> Option<(i64, u32, u32, &str)> {
    let mut fields = rest.split(' ');
    let day = fixed_digits(fields.next()?, 2)?;
    let month = month_number(fields.next()?)?;
    let year = i64::from(fixed_digits(fields.next()?, 4)?);
    let time = fields.next()?;
    if !fields.next()?.eq_ignore_ascii_case("GMT") || fields.next().is_some() {
        return None;
    }
    Some((year, month, day, time))
}

/// `06-Nov-94 08:49:37 GMT` — the day-name and its comma already consumed.
fn rfc850_date(rest: &str, now_unix: i64) -> Option<(i64, u32, u32, &str)> {
    let mut fields = rest.split(' ');
    let date = fields.next()?;
    let time = fields.next()?;
    if !fields.next()?.eq_ignore_ascii_case("GMT") || fields.next().is_some() {
        return None;
    }
    let mut parts = date.split('-');
    let day = fixed_digits(parts.next()?, 2)?;
    let month = month_number(parts.next()?)?;
    let two_digit_year = fixed_digits(parts.next()?, 2)?;
    if parts.next().is_some() {
        return None;
    }
    Some((
        expand_two_digit_year(two_digit_year, now_unix),
        month,
        day,
        time,
    ))
}

/// `Sun Nov  6 08:49:37 1994` — no comma, a space-padded day, no zone.
fn asctime_date(raw: &str) -> Option<(i64, u32, u32, &str)> {
    let mut fields = raw.split_ascii_whitespace();
    if !is_day_name(fields.next()?) {
        return None;
    }
    let month = month_number(fields.next()?)?;
    let day_field = fields.next()?;
    if day_field.len() > 2 {
        return None;
    }
    let day = fixed_digits(day_field, day_field.len())?;
    let time = fields.next()?;
    let year = i64::from(fixed_digits(fields.next()?, 4)?);
    if fields.next().is_some() {
        return None;
    }
    Some((year, month, day, time))
}

/// RFC 7231: a two-digit year that would land more than 50 years in the
/// future means the most recent past year with the same last two digits.
fn expand_two_digit_year(two_digit_year: u32, now_unix: i64) -> i64 {
    // Mean Gregorian year; the estimate can be a year off at a boundary, and
    // that cannot change the outcome — the two candidates are a century apart.
    let approx_year = 1970 + now_unix.div_euclid(31_556_952);
    let candidate = approx_year - approx_year.rem_euclid(100) + i64::from(two_digit_year);
    if candidate > approx_year + 50 {
        candidate - 100
    } else {
        candidate
    }
}

/// `08:49:37`. A leap second (`:60`) is accepted and simply lands a second
/// later; anything else out of range is rejected.
fn time_of_day(field: &str) -> Option<(u32, u32, u32)> {
    let mut parts = field.split(':');
    let hour = fixed_digits(parts.next()?, 2)?;
    let minute = fixed_digits(parts.next()?, 2)?;
    let second = fixed_digits(parts.next()?, 2)?;
    if parts.next().is_some() || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    Some((hour, minute, second))
}

/// Exactly `len` ASCII digits, or nothing.
fn fixed_digits(field: &str, len: usize) -> Option<u32> {
    if field.len() != len || !field.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    field.parse().ok()
}

/// Either the abbreviated or the full day name, in any case. Which day it
/// names is not checked against the date — no recipient is required to.
fn is_day_name(field: &str) -> bool {
    const DAY_NAMES: [(&str, &str); 7] = [
        ("Mon", "Monday"),
        ("Tue", "Tuesday"),
        ("Wed", "Wednesday"),
        ("Thu", "Thursday"),
        ("Fri", "Friday"),
        ("Sat", "Saturday"),
        ("Sun", "Sunday"),
    ];
    DAY_NAMES
        .iter()
        .any(|(short, long)| field.eq_ignore_ascii_case(short) || field.eq_ignore_ascii_case(long))
}

/// `Jan` → 1 … `Dec` → 12.
fn month_number(field: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let index = MONTHS.iter().position(|m| field.eq_ignore_ascii_case(m))?;
    u32::try_from(index + 1).ok()
}

fn days_in_month(year: i64, month: u32) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days from 1970-01-01 to `year-month-day` in the proleptic Gregorian
/// calendar (Howard Hinnant's `days_from_civil`, exact for every year this
/// parser can produce: four digits, so no overflow is reachable).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let month = i64::from(month);
    let shifted_year = if month <= 2 { year - 1 } else { year };
    let era = if shifted_year >= 0 {
        shifted_year
    } else {
        shifted_year - 399
    } / 400;
    let year_of_era = shifted_year - era * 400;
    let day_of_year =
        (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::map_provider_error;
    use reqwest::StatusCode;
    use reqwest::header::HeaderValue;
    use transync::llm::TranslatorError;

    fn status(code: u16) -> StatusCode {
        StatusCode::from_u16(code).expect("test uses a valid status code")
    }

    /// The whole table at once, read where it actually matters: which
    /// `TranslatorError` each status becomes, since `pipeline::dispatch`
    /// re-dispatches exactly `Network` and `RateLimited` and surfaces
    /// everything else.
    #[test]
    fn the_table_maps_onto_the_retry_surface_the_pipeline_consumes() {
        // Retryable — including 529, this provider's own overload signal.
        for code in [408u16, 409, 425, 500, 502, 503, 504, 529] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::Network(_)),
                "{code} must reach the pipeline as a retryable Network error, got {mapped:?}"
            );
        }
        // Terminal rejections, each carrying its own status as a number.
        for code in [400u16, 404, 413, 418, 422, 499] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::ProviderRejected { status: Some(s), .. }
                    if s == code),
                "{code} must be terminal and name its status, got {mapped:?}"
            );
            assert_eq!(mapped.stable_code(), "provider_rejected");
        }
        // Terminal auth.
        for code in [401u16, 403] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::Authentication(_)),
                "{code} must be terminal auth, got {mapped:?}"
            );
        }
        // Rate limiting carries the parsed hint through verbatim.
        let mapped = map_provider_error(provider_error_for_status(
            status(429),
            Some(Duration::from_secs(5)),
            b"",
        ));
        assert!(
            matches!(mapped, TranslatorError::RateLimited { retry_after: Some(d) }
                if d == Duration::from_secs(5)),
            "429 must reach the pipeline as RateLimited with the hint, got {mapped:?}"
        );
        assert!(matches!(
            map_provider_error(provider_error_for_status(status(429), None, b"")),
            TranslatorError::RateLimited { retry_after: None }
        ));
    }

    /// R0004-0021: a `3xx` is terminal. The client follows no redirects
    /// (R0004-0001), so a redirect status arrives here as a *response*, and
    /// it is decided entirely by the endpoint's configuration — the same POST
    /// earns the same `Location` on every attempt, so the catch-all's
    /// retryable `Network` only spent the bounded budget before surfacing the
    /// same thing later.
    #[test]
    fn a_redirect_status_is_terminal_and_says_why() {
        for code in [300u16, 301, 302, 303, 307, 308, 399] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::ProviderRejected { status: Some(s), .. }
                    if s == code),
                "{code} must be terminal and name its status, got {mapped:?}"
            );
            let TranslatorError::ProviderRejected { message, .. } = &mapped else {
                unreachable!("checked above");
            };
            assert!(
                message.contains("redirect") && message.contains("base_url"),
                "{code} must tell the operator what to change: {message:?}"
            );
        }
    }

    /// 529 is named, not inherited. It sits outside the 5xx band, so a table
    /// that only knew `>499` would let it fall through to the catch-all arm
    /// by luck rather than by decision — and the message says which signal it
    /// is, because "HTTP 529" alone tells an operator nothing.
    #[test]
    fn the_overload_signal_is_named_rather_than_left_to_a_range() {
        let err = provider_error_for_status(status(529), None, b"");
        match &err {
            ProviderError::Transport(s) => assert!(
                s.contains("529") && s.contains("overloaded_error"),
                "the overload signal must name itself: {s:?}"
            ),
            other => panic!("529 must be retryable Transport, got {other:?}"),
        }
    }

    /// 401/403 carry the *bare* excerpt, with no `HTTP <status>:` prefix —
    /// and never the key, which this adapter has and never puts in a message.
    #[test]
    fn auth_statuses_carry_a_bare_excerpt() {
        for code in [401u16, 403] {
            let err = provider_error_for_status(status(code), None, b"invalid x-api-key");
            match &err {
                ProviderError::Auth(s) => assert_eq!(s, "invalid x-api-key", "{code}"),
                other => panic!("{code} must classify as Auth, got {other:?}"),
            }
        }
    }

    /// The provider's error envelope is read when it is there — including
    /// `request_id`, its own correlation handle — and the raw bytes are
    /// quoted when it is not, because the status classifies either way.
    #[test]
    fn the_error_envelope_is_read_when_present_and_quoted_when_not() {
        let envelope = br#"{"type":"error","error":{"type":"not_found_error","message":"model: nope"},"request_id":"req_123"}"#;
        let err = provider_error_for_status(status(404), None, envelope);
        let ProviderError::Rejected { message, .. } = &err else {
            panic!("404 must be a rejection, got {err:?}");
        };
        assert!(message.contains("not_found_error"), "got {message:?}");
        assert!(message.contains("model: nope"), "got {message:?}");
        assert!(
            message.contains("req_123"),
            "the provider's correlation handle costs nothing and must ride along: {message:?}"
        );

        // A gateway with its own shape, an HTML page, or truncated bytes: the
        // raw excerpt is still worth quoting.
        for foreign in [
            &b"<html>502 from the proxy</html>"[..],
            &b"{\"type\":\"error\"}"[..],
            &b"{\"error\":{}}"[..],
            &b""[..],
        ] {
            let err = provider_error_for_status(status(400), None, foreign);
            let ProviderError::Rejected { message, .. } = &err else {
                panic!("400 must be a rejection, got {err:?}");
            };
            let quoted = String::from_utf8_lossy(foreign);
            assert!(
                message.ends_with(quoted.as_ref()),
                "the unparsed body must be quoted: {message:?}"
            );
        }
    }

    /// A provider-controlled error body is capped at the 512-byte diagnostic
    /// excerpt whether it parses as the envelope or not — a hostile gateway
    /// must not be able to push a document into stderr through either door.
    #[test]
    fn an_error_body_is_capped_through_either_door() {
        let huge = "m".repeat(200_000);
        let envelope = serde_json::to_vec(&serde_json::json!({
            "type": "error",
            "error": { "type": "invalid_request_error", "message": huge },
        }))
        .expect("fixture serializes");
        for (shape, body) in [("envelope", envelope), ("raw", huge.into_bytes())] {
            let err = provider_error_for_status(status(400), None, &body);
            let ProviderError::Rejected { message, .. } = &err else {
                panic!("{shape}: expected a rejection");
            };
            assert!(
                message.len() < 1024,
                "{shape}: the excerpt must be capped, got {} bytes",
                message.len()
            );
            assert!(message.contains("truncated"), "{shape}: got {message:?}");
        }
    }

    #[test]
    fn parse_retry_after_reads_delta_seconds() {
        let header = HeaderValue::from_static("120");
        assert_eq!(
            parse_retry_after(Some(&header)),
            Some(Duration::from_secs(120))
        );
        // Surrounding whitespace is tolerated.
        let padded = HeaderValue::from_static("  7  ");
        assert_eq!(
            parse_retry_after(Some(&padded)),
            Some(Duration::from_secs(7))
        );
        let zero = HeaderValue::from_static("0");
        assert_eq!(parse_retry_after(Some(&zero)), Some(Duration::from_secs(0)));
        assert_eq!(parse_retry_after(None), None);
    }

    /// 1994-11-06 08:49:37 UTC — the RFC's own example instant, and the
    /// anchor for every date case below.
    const REFERENCE_INSTANT: i64 = 784_111_777;

    fn retry_after_at(raw: &str, now_unix: i64) -> Option<Duration> {
        let header = HeaderValue::from_str(raw).expect("valid header value");
        parse_retry_after_at(Some(&header), now_unix)
    }

    /// Both RFC 7231 forms — all three date spellings a recipient must accept
    /// — name the same instant and are honored, not discarded.
    #[test]
    fn parse_retry_after_reads_the_three_http_date_forms() {
        let now = REFERENCE_INSTANT - 90;
        for raw in [
            "Sun, 06 Nov 1994 08:49:37 GMT",  // IMF-fixdate
            "Sunday, 06-Nov-94 08:49:37 GMT", // obsolete RFC 850
            "Sun Nov  6 08:49:37 1994",       // obsolete asctime
        ] {
            assert_eq!(
                retry_after_at(raw, now),
                Some(Duration::from_secs(90)),
                "{raw:?} must resolve to the deadline's distance from now"
            );
        }
        // Surrounding whitespace is tolerated on the date form too, and the
        // zone token is matched case-insensitively.
        assert_eq!(
            retry_after_at("  Sun, 06 Nov 1994 08:49:37 gmt  ", now),
            Some(Duration::from_secs(90))
        );
        // The calendar is real: a leap day parses, and the year rolls over.
        assert_eq!(
            retry_after_at("Mon, 29 Feb 2016 00:00:01 GMT", 1_456_704_000),
            Some(Duration::from_secs(1))
        );
    }

    /// A deadline already at or behind the clock is *not* a zero-second wait:
    /// it degrades to "no hint" so the pipeline's own backoff runs, rather
    /// than turning a skewed clock into an immediate re-dispatch.
    #[test]
    fn parse_retry_after_ignores_a_deadline_that_has_passed() {
        for now in [REFERENCE_INSTANT, REFERENCE_INSTANT + 1] {
            assert_eq!(retry_after_at("Sun, 06 Nov 1994 08:49:37 GMT", now), None);
        }
    }

    /// The public entry point reads the same date against the real clock —
    /// the seam above is only about determinism.
    #[test]
    fn parse_retry_after_honors_a_date_against_the_system_clock() {
        let header = HeaderValue::from_static("Fri, 31 Dec 2100 23:59:59 GMT");
        let parsed = parse_retry_after(Some(&header)).expect("a far-future deadline is honored");
        assert!(
            parsed > Duration::from_secs(365 * 24 * 3600),
            "got {parsed:?}"
        );
    }

    /// Garbage — date-shaped or not — degrades to `None`, so the caller uses
    /// its own backoff instead of a bogus delay.
    #[test]
    fn parse_retry_after_rejects_garbage() {
        for raw in [
            "soon",
            "-5",
            "1.5",
            "",
            "Wed, 21 Oct 2015 07:28:00",       // no zone
            "Wed, 21 Oct 2015 07:28:00 PST",   // not GMT
            "Xyz, 21 Oct 2015 07:28:00 GMT",   // not a day name
            "Wed, 21 Foo 2015 07:28:00 GMT",   // not a month
            "Wed, 32 Oct 2015 07:28:00 GMT",   // no such day
            "Wed, 29 Feb 2015 00:00:00 GMT",   // 2015 is not a leap year
            "Wed, 21 Oct 2015 24:00:00 GMT",   // no such hour
            "Wed, 21 Oct 2015 07:61:00 GMT",   // no such minute
            "Wed, 21 Oct 15 07:28:00 GMT",     // two-digit year outside RFC 850
            "Wed, 21 Oct 2015 07:28:00 GMT x", // trailing junk
            "Sun Nov 6 08:49:37",              // asctime without a year
        ] {
            assert_eq!(
                retry_after_at(raw, REFERENCE_INSTANT - 10_000),
                None,
                "{raw:?}"
            );
        }
        // A header whose bytes are not visible ASCII cannot be read at all.
        let opaque = HeaderValue::from_bytes(&[0xff, 0xfe]).expect("opaque header value");
        assert_eq!(parse_retry_after(Some(&opaque)), None);
    }

    /// The obsolete two-digit year resolves into the 50-year window around
    /// the clock, per RFC 7231 — never a century off.
    #[test]
    fn rfc850_two_digit_years_land_in_the_window_around_now() {
        // 2026-07-25. The window is [1976, 2076]: `75` is still ahead of it
        // by less than 50 years, `77` would be more than 50 years out and so
        // means the most recent past year ending in 77.
        let now = 1_785_000_000;
        assert_eq!(expand_two_digit_year(26, now), 2026);
        assert_eq!(expand_two_digit_year(75, now), 2075);
        assert_eq!(expand_two_digit_year(77, now), 1977);
        assert_eq!(expand_two_digit_year(99, now), 1999);
    }

    /// The byte-cap helper reports the declared length up front and flags
    /// mid-stream overflow, both as a terminal `ResponseTooLarge`.
    #[test]
    fn oversize_response_error_messages() {
        let declared = oversize_response_error(Some(40 * 1024 * 1024));
        assert!(
            matches!(&declared, ProviderError::ResponseTooLarge(s)
                if s.contains("32 MiB cap") && s.contains("Content-Length")),
            "got {declared:?}"
        );
        let streamed = oversize_response_error(None);
        assert!(
            matches!(&streamed, ProviderError::ResponseTooLarge(s)
                if s.contains("32 MiB cap") && s.contains("streaming")),
            "got {streamed:?}"
        );
        for err in [
            oversize_response_error(Some(1)),
            oversize_response_error(None),
        ] {
            assert_eq!(
                map_provider_error(err).stable_code(),
                "provider_response_too_large"
            );
        }
    }
}
