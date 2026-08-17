//! Failure classification: HTTP status / `reqwest::Error` → [`ProviderError`].
//!
//! This module owns the **classification table** that the core pipeline's
//! retry policy consumes as input. `transync-core`'s
//! `pipeline::dispatch::translate_with_provider_retries` retries exactly
//! `TranslatorError::Network` and `TranslatorError::RateLimited`, and
//! [`crate::error::map_provider_error`] produces those from
//! [`ProviderError::Transport`] and [`ProviderError::RateLimitedAfter`] —
//! so picking a variant here *is* declaring a failure retryable or terminal.
//! The policy itself (attempt budget, backoff, `Retry-After` honoring) lives
//! in core and is deliberately not duplicated here.
//!
//! Split out of `client.rs` (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`) so that
//! [`super::transport::post_json`] is left with send + size cap +
//! accumulation only. The table's behavior is unchanged by that split and is
//! pinned by the tests below.
//!
//! TRACE: SCN-12
//! TRACE: contracts.md §1

use crate::error::ProviderError;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::transport::MAX_RESPONSE_BYTES;

/// Classify a non-success HTTP response into a [`ProviderError`].
///
/// - 401/403 → terminal [`ProviderError::Auth`], carrying the bare body
///   excerpt.
/// - 429 → [`ProviderError::RateLimitedAfter`] with whatever
///   [`parse_retry_after`] made of the `Retry-After` header.
/// - R0008-0032: 408 (Request Timeout), 409 (Conflict) and 425 (Too Early)
///   are commonly retryable, so they are classified as transient
///   ([`ProviderError::Transport`] → `Network`) like 5xx rather than as a
///   terminal rejection.
/// - Any other 4xx → terminal [`ProviderError::Rejected`], carrying the
///   status **as a number** as well as in the message (ti 1a85f3): a 404 is
///   a model name that does not exist and an operator has to fix it, which
///   a consumer could not tell from a 400 while the whole family arrived as
///   one opaque string.
/// - R0004-0021: **3xx** → terminal [`ProviderError::Rejected`]. The client
///   this crate builds follows **no** redirects (see
///   `crate::http_client_builder_with`), so a `3xx` is a response this
///   adapter is handed rather than an internal step. Nothing about it changes
///   on a verbatim resubmission: the same POST to the same endpoint earns the
///   same `Location`, so retrying spends the whole bounded budget to arrive
///   at the same answer later. It is the endpoint configuration that has to
///   change, which is what the message says.
/// - Everything else (5xx and beyond) → transient
///   [`ProviderError::Transport`].
///
/// `body` is the raw response bytes; R0008-0034 caps the diagnostic excerpt
/// here (see [`truncate_diagnostic`]) so a hostile or misbehaving proxy
/// cannot flood CLI stderr and logs with an echoed document.
pub(super) fn provider_error_for_status(
    status: reqwest::StatusCode,
    retry_after: Option<Duration>,
    body: &[u8],
) -> ProviderError {
    let text = truncate_diagnostic(body);
    match status.as_u16() {
        401 | 403 => ProviderError::Auth(text),
        429 => ProviderError::RateLimitedAfter(retry_after),
        408 | 409 | 425 => ProviderError::Transport(format!("HTTP {status}: {text}")),
        code @ 300..=399 => ProviderError::Rejected {
            status: Some(code),
            message: format!(
                "HTTP {status}: the endpoint answered the POST with a redirect, which this \
                 client does not follow — the bearer credential and the document body would \
                 travel to the redirect target. Point base_url at the endpoint that answers \
                 directly. {text}"
            ),
        },
        code @ 400..=499 => ProviderError::Rejected {
            status: Some(code),
            message: format!("HTTP {status}: {text}"),
        },
        _ => ProviderError::Transport(format!("HTTP {status}: {text}")),
    }
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
            "redirect policy refused the hop, which no resubmission changes — the bearer \
             credential and the document body would travel to the redirect target; point \
             base_url at the endpoint that answers directly: {err}"
        ))
    } else {
        ProviderError::Transport(err.to_string())
    }
}

/// Build the terminal error for a response that blows past
/// [`MAX_RESPONSE_BYTES`]. `declared` is the `Content-Length` when the
/// cap was tripped up front, or `None` when it was tripped mid-stream.
/// EXT-2026-07 P1-7.
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

/// What a refusal carrying no string of its own reports in place of one. The
/// refusal itself is the fact worth carrying; its wording is optional.
///
/// It lives here, beside [`truncate_diagnostic`], because **both** surfaces
/// report refusals and neither owns the other's vocabulary: a refusal with no
/// usable detail reaches the operator with the same words whether it arrived
/// as a Chat content part or as a Responses `refusal` segment. It was private
/// to `chat` while only that surface had a stand-in, and the asymmetry was the
/// defect (ti `594a7f`) — the Responses path printed "model refused to
/// translate: " and stopped.
pub(super) const NO_REFUSAL_DETAIL: &str = "no refusal detail";

/// Cap a diagnostic body to a bounded, char-safe excerpt so a large or
/// hostile error payload cannot flood stderr and logs (R0008-0034).
pub(super) fn truncate_diagnostic(bytes: &[u8]) -> String {
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

/// Parse a `Retry-After` header value into a `Duration`. Both RFC 7231
/// forms are honored: delta-seconds ("120") and the HTTP-date deadline
/// ("Wed, 21 Oct 2015 07:28:00 GMT"), the latter converted against the
/// system clock. Unparseable values — and a deadline that is already at or
/// behind the clock — produce `None`, which surfaces upstream as
/// `RateLimited { retry_after: None }` and leaves the pipeline on its own
/// exponential backoff.
///
/// R0001-0029: the date form used to be rejected outright, so a provider
/// or intermediary that sent it got the pipeline's short backoff instead of
/// the pause it asked for. The pipeline's 30 s cap (contracts.md §5) still
/// applies on top of whatever this returns.
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
    // A deadline in the past (or right now) carries no wait, and treating it
    // as a zero delay would turn a skewed clock into a hot retry loop — so it
    // degrades to "no hint" and the caller's backoff schedule takes over.
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
/// Hand-rolled on purpose: this is the only date in the whole workspace, and
/// a `chrono`/`httpdate` dependency for one header is not a trade worth
/// making (the ticket leaves the call here). All three forms are UTC by
/// definition, so no zone database is involved — only calendar arithmetic.
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
    use std::time::Duration;
    use transync::llm::TranslatorError;

    fn status(code: u16) -> StatusCode {
        StatusCode::from_u16(code).expect("test uses a valid status code")
    }

    /// R0008-0032 pin: 408 (Request Timeout), 409 (Conflict) and 425 (Too
    /// Early) are the *retryable* 4xx statuses — they must land on
    /// `Transport`, which `map_provider_error` turns into
    /// `TranslatorError::Network`, the variant `pipeline::dispatch`'s
    /// transient-retry loop consumes.
    #[test]
    fn retryable_4xx_statuses_classify_as_transport() {
        for code in [408u16, 409, 425] {
            let err = provider_error_for_status(status(code), None, b"upstream said no");
            match &err {
                ProviderError::Transport(s) => {
                    assert!(
                        s.starts_with(&format!("HTTP {code}")),
                        "{code}: message must lead with the status, got {s:?}"
                    );
                    assert!(
                        s.ends_with("upstream said no"),
                        "{code}: message must carry the body excerpt, got {s:?}"
                    );
                }
                other => panic!("{code} must be retryable Transport, got {other:?}"),
            }
        }
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

    /// 429 carries the parsed `Retry-After` through verbatim (including its
    /// absence) so the pipeline can pace from the provider's own guidance.
    #[test]
    fn rate_limited_carries_the_parsed_retry_after() {
        let with_hint =
            provider_error_for_status(status(429), Some(Duration::from_secs(120)), b"slow down");
        assert!(
            matches!(
                &with_hint,
                ProviderError::RateLimitedAfter(Some(d)) if *d == Duration::from_secs(120)
            ),
            "got {with_hint:?}"
        );

        let without_hint = provider_error_for_status(status(429), None, b"slow down");
        assert!(
            matches!(&without_hint, ProviderError::RateLimitedAfter(None)),
            "got {without_hint:?}"
        );
    }

    /// 401/403 are terminal auth failures and carry the *bare* body excerpt,
    /// with no `HTTP <status>:` prefix.
    #[test]
    fn auth_statuses_classify_as_auth_with_bare_body() {
        for code in [401u16, 403] {
            let err = provider_error_for_status(status(code), None, b"invalid api key");
            match &err {
                ProviderError::Auth(s) => assert_eq!(s, "invalid api key", "{code}"),
                other => panic!("{code} must classify as Auth, got {other:?}"),
            }
        }
    }

    /// Every other 4xx is a terminal rejection — retrying a 400/404/422 only
    /// burns quota — and it carries the status **as a number** (ti 1a85f3)
    /// so a consumer can act on a 404 (a model name that does not exist,
    /// which an operator has to fix) without string-matching the message.
    #[test]
    fn other_4xx_statuses_are_terminal_rejections_carrying_their_status() {
        for code in [400u16, 404, 418, 422, 499] {
            let err = provider_error_for_status(status(code), None, b"nope");
            match &err {
                ProviderError::Rejected { status, message } => {
                    assert_eq!(
                        *status,
                        Some(code),
                        "{code}: status must be typed, not prose"
                    );
                    assert!(
                        message.starts_with(&format!("HTTP {code}")) && message.ends_with("nope"),
                        "{code}: got {message:?}"
                    );
                }
                other => panic!("{code} must be a terminal Rejected, got {other:?}"),
            }
        }
    }

    /// 5xx (and anything else non-success outside 4xx) is transient.
    #[test]
    fn server_errors_classify_as_transport() {
        for code in [500u16, 502, 503, 504] {
            let err = provider_error_for_status(status(code), None, b"upstream down");
            assert!(
                matches!(&err, ProviderError::Transport(s) if s.starts_with(&format!("HTTP {code}"))),
                "{code}: got {err:?}"
            );
        }
    }

    /// The table is the pipeline's retry-policy *input*: what matters
    /// downstream is which `TranslatorError` each status becomes.
    /// `pipeline::dispatch` retries exactly `Network` and `RateLimited`.
    #[test]
    fn classification_maps_onto_the_retry_surface_the_pipeline_consumes() {
        let retryable = [408u16, 409, 425, 500, 503];
        for code in retryable {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::Network(_)),
                "{code} must reach the pipeline as a retryable Network error, got {mapped:?}"
            );
        }
        let mapped = map_provider_error(provider_error_for_status(
            status(429),
            Some(Duration::from_secs(5)),
            b"",
        ));
        assert!(
            matches!(
                mapped,
                TranslatorError::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(5)
            ),
            "429 must reach the pipeline as RateLimited with the hint, got {mapped:?}"
        );
        for code in [400u16, 404, 422] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::ProviderRejected { status: Some(s), .. } if s == code),
                "{code} must be terminal and name its status, got {mapped:?}"
            );
            assert_eq!(
                mapped.stable_code(),
                "provider_rejected",
                "{code} must reach a code consumer as its own cause"
            );
        }
        for code in [401u16, 403] {
            let mapped = map_provider_error(provider_error_for_status(status(code), None, b""));
            assert!(
                matches!(mapped, TranslatorError::Authentication(_)),
                "{code} must be terminal auth, got {mapped:?}"
            );
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
    }

    /// 1994-11-06 08:49:37 UTC — the RFC's own example instant, and the
    /// anchor for every date case below.
    const REFERENCE_INSTANT: i64 = 784_111_777;

    fn retry_after_at(raw: &str, now_unix: i64) -> Option<Duration> {
        let header = HeaderValue::from_str(raw).expect("valid header value");
        parse_retry_after_at(Some(&header), now_unix)
    }

    /// R0001-0029: all three RFC 7231 §7.1.1.1 date forms name the same
    /// instant and are honored, not discarded. (This test replaces the one
    /// that used to pin the date form as *rejected*.)
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

    /// A deadline already at or behind the clock is *not* a zero-second
    /// wait: it degrades to "no hint" so the pipeline's own backoff runs,
    /// rather than turning a skewed clock into an immediate re-dispatch.
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

    /// Garbage — date-shaped or not — still degrades to `None`, which
    /// surfaces as `RateLimited { retry_after: None }` so the caller uses
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

    #[test]
    fn parse_retry_after_absent_header_is_none() {
        assert_eq!(parse_retry_after(None), None);
    }

    /// EXT-2026-07 P1-7: the byte-cap helper reports the declared length up
    /// front and flags mid-stream overflow, both as a terminal
    /// `ResponseTooLarge` (ti 1a85f3 — it used to be an `Other` a consumer
    /// could only recognize by grepping for "32 MiB cap").
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

    /// R0008-0034: a short body passes through untouched; an over-long one is
    /// cut at a char boundary and annotated with the true byte count.
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
}
