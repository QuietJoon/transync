//! One connection: read a request head, answer it, close.
//!
//! **No keep-alive, deliberately.** Every response carries
//! `Connection: close` and the socket is dropped afterwards, so exactly one
//! request is read per connection and there is never a second message whose
//! boundary has to be inferred. That erases request smuggling and pipelining
//! desync as a class rather than defending against them, and it costs a demo
//! server bounded by a six-file bundle nothing measurable.
//!
//! **`GET` and `HEAD` only.** Everything else is `405` with an `Allow` header.
//! There is no upload path, no directory listing and nothing is executed: a
//! request either names a regular file inside the served root or it is
//! refused.
//!
//! **Who the request is for is decided before what it asks for.** The `Host`
//! check ([`super::host`]) runs on the head, ahead of the method and ahead of
//! any filesystem work, because a request that does not name this server is
//! not a request this server answers — not even to say what it would have
//! refused.
//!
//! TRACE: SCN-13

use super::Site;
use super::host::Verdict;
use super::mime;
use super::route::{self, Refusal};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Longest request head this server will read, terminator included. A client
/// that has not finished its head by here is not sending one.
///
/// The number is exact rather than approximate: [`read_head`] never reads past
/// it, so a head of `MAX_HEAD_BYTES + 1` is refused instead of landing
/// wherever the last read happened to stop.
const MAX_HEAD_BYTES: usize = 8 * 1024;

/// How long the head may take to arrive. Bounds a connection that opens and
/// then says nothing, which would otherwise hold a task for the process's
/// lifetime.
const HEAD_TIMEOUT: Duration = Duration::from_secs(15);

/// How long the whole answer may take, measured from the moment the head is
/// in hand. Bounds the *other* half of the same problem [`HEAD_TIMEOUT`]
/// covers (R0009-0001): a peer that finishes its head correctly and then stops
/// reading leaves every response write parked on the socket's send buffer, and
/// `MAX_IN_FLIGHT` makes those parked tasks a finite shared resource — so a
/// handful of such peers is enough to make the server unavailable without
/// sending a single malformed byte.
///
/// A **total** deadline rather than a write-idle one, deliberately. An idle
/// deadline is defeated by a peer that reads one byte per interval, which is
/// the same attack at a lower rate; a total deadline cannot be. What it costs
/// is the honest tradeoff: a legitimate transfer slower than this is cut off
/// too. That is the right trade here and only here — this is a loopback demo
/// server for an `--html-out` bundle (a handful of files, the largest of them
/// a few megabytes), so a minute is orders of magnitude more than any real
/// client on the path this server is built for needs.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

/// At or below this size the file is read into memory, so the
/// `Content-Length` announced is the length actually written. Above it the
/// body streams and the length comes from the open handle's metadata — a
/// promise made before the bytes are read, so [`send_file`] checks the copy
/// against it and gives up the connection when the file no longer has them.
const IN_MEMORY_LIMIT: u64 = 8 * 1024 * 1024;

/// The methods a `405` advertises.
const ALLOWED_METHODS: &str = "GET, HEAD";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Get,
    Head,
}

enum HeadRead {
    Complete(String),
    TooLarge,
    /// The peer went away before finishing its head; there is nobody to answer.
    Incomplete,
}

/// Answer one request on `stream`, then return so the caller can drop it.
///
/// An `Err` here is a response that could not be delivered, never a rejected
/// request — every rejection is a status code written back on this same
/// connection. There are three of them, and all three end the same way, with
/// the socket dropped and its slot returned: a dead socket, a peer that did
/// not read its response inside [`RESPONSE_TIMEOUT`], and — the one case where
/// this server is the party that broke the message — a streamed body that came
/// up short of the `Content-Length` already announced for it ([`send_file`]).
///
/// Generic over the stream so the deadline can be exercised against a peer
/// that accepts no bytes at all; `accept_loop` passes the socket.
pub async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    site: Arc<Site>,
) -> io::Result<()> {
    serve_within(stream, site, RESPONSE_TIMEOUT).await
}

/// [`serve`] with the response deadline supplied, so a test can drive a
/// never-reading peer without waiting out the shipped minute.
async fn serve_within<S: AsyncRead + AsyncWrite + Unpin>(
    mut stream: S,
    site: Arc<Site>,
    response_timeout: Duration,
) -> io::Result<()> {
    // Both halves of the head read that still owe the peer an answer become
    // one, so that answer goes out under the same deadline as every other.
    let head = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut stream)).await {
        Err(_elapsed) => Err((408_u16, "Request Timeout")),
        Ok(Err(err)) => return Err(err),
        Ok(Ok(HeadRead::Incomplete)) => return Ok(()),
        Ok(Ok(HeadRead::TooLarge)) => Err((431, "Request Header Fields Too Large")),
        Ok(Ok(HeadRead::Complete(head))) => Ok(head),
    };
    match tokio::time::timeout(response_timeout, respond(&mut stream, &site, head)).await {
        Ok(result) => result,
        Err(_elapsed) => Err(io::Error::new(
            io::ErrorKind::TimedOut,
            format!(
                "the peer did not read its response within {}s",
                response_timeout.as_secs()
            ),
        )),
    }
}

/// Everything after the head, under one deadline: route it, then write it.
///
/// `head` is `Err((code, reason))` when reading it already decided the answer
/// — those two refusals are responses like any other and are written here.
async fn respond<S: AsyncWrite + Unpin>(
    stream: &mut S,
    site: &Site,
    head: Result<String, (u16, &'static str)>,
) -> io::Result<()> {
    let head = match head {
        Err((code, reason)) => return status(stream, Method::Get, code, reason).await,
        Ok(head) => head,
    };

    let Some((method_token, target)) = parse_request_line(&head) else {
        return status(stream, Method::Get, 400, "Bad Request").await;
    };
    let method = match method_token {
        "GET" => Some(Method::Get),
        "HEAD" => Some(Method::Head),
        _ => None,
    };

    // Ahead of the `405`: an unservable method is still a request, and the
    // question of whose server it was sent to is settled first. `Method::Get`
    // stands in for a method this server does not have, which only decides
    // whether the refusal carries its body.
    match site.hosts.verdict(&head) {
        Verdict::Answered => {}
        Verdict::Malformed => {
            return status(stream, method.unwrap_or(Method::Get), 400, "Bad Request").await;
        }
        Verdict::Elsewhere => {
            return status_detail(
                stream,
                method.unwrap_or(Method::Get),
                421,
                "Misdirected Request",
                Some(&format!(
                    "This server answers for {}. Add another with --allow-host.",
                    site.hosts.answered_authorities(),
                )),
            )
            .await;
        }
    }

    let Some(method) = method else {
        return write_head(
            stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            &[("Allow", ALLOWED_METHODS)],
            0,
        )
        .await;
    };

    match resolve(site.root.as_path(), target).await {
        Err(refusal) => {
            let (code, reason) = match refusal {
                Refusal::BadRequest => (400, "Bad Request"),
                Refusal::Forbidden => (403, "Forbidden"),
                Refusal::NotFound => (404, "Not Found"),
            };
            status(stream, method, code, reason).await
        }
        Ok(found) => send_file(stream, method, found).await,
    }
}

/// A regular file inside the served root, already open.
struct Found {
    file: tokio::fs::File,
    len: u64,
    /// The **canonical** path, which is what names the content type. A symlink
    /// therefore cannot relabel a file: `page.html -> data.json` is served as
    /// JSON, the type of the bytes that actually come back.
    canonical: PathBuf,
}

/// Turn a request target into an open handle on a regular file inside `root`.
///
/// The order is the security property: **canonicalize first, check second,
/// open the canonical path third.** Opening the requested path and asking
/// afterwards would mean the handle and the answer describe different files
/// whenever a symlink sits in between, which is the whole attack.
///
/// What that order narrows rather than closes: another process on this machine
/// could still replace an entry between the `canonicalize` and the `open`. The
/// window is one syscall wide and the path opened is the one just verified to
/// have no links in it, so exploiting it needs local write access inside the
/// served directory — at which point the contents are already the attacker's.
/// Closing it entirely means walking the path with `openat`/`O_NOFOLLOW`,
/// which is out of scope for a loopback demo server (ticket `b791d6`) and is
/// recorded here rather than left to be rediscovered.
async fn resolve(root: &Path, target: &str) -> Result<Found, Refusal> {
    let candidate = route::confine(root, target)?;
    // Resolves every symlink and every `..` the *filesystem* contains — which
    // is what the lexical pass in `route` cannot see.
    let canonical = tokio::fs::canonicalize(&candidate)
        .await
        .map_err(|_| Refusal::NotFound)?;
    if !route::is_contained(root, &canonical) {
        return Err(Refusal::Forbidden);
    }
    let file = tokio::fs::File::open(&canonical)
        .await
        .map_err(|_| Refusal::NotFound)?;
    let meta = file.metadata().await.map_err(|_| Refusal::NotFound)?;
    if !meta.is_file() {
        // A directory, a device, a socket. There is no directory listing.
        return Err(Refusal::NotFound);
    }
    Ok(Found {
        file,
        len: meta.len(),
        canonical,
    })
}

/// Write the `200` for an open file: buffered below [`IN_MEMORY_LIMIT`],
/// streamed above it.
///
/// The two branches answer the same question — *is the announced
/// `Content-Length` the number of bytes this response actually carries?* —
/// and they can afford different answers because only one of them still has
/// the choice. Buffered, the read happens first, so the head states the length
/// that was read and a file that changed size under it is simply described
/// correctly. Streamed, the head is already on the wire when the body is read,
/// so a file that **shrank** since `resolve` measured it (an operator
/// regenerating the bundle is the innocent case) leaves no truthful body to
/// send: the copy ends short of the promise. That is a protocol violation
/// either way, and the only thing left to decide is whether the peer can tell
/// — so the short copy is an `Err` (R0009-0002), which makes `accept_loop`
/// drop the socket with the message incomplete rather than close it as if the
/// response had been delivered. A conforming client records an incomplete
/// message (RFC 9112 §8) instead of caching a truncated document.
async fn send_file<S: AsyncWrite + Unpin>(
    stream: &mut S,
    method: Method,
    found: Found,
) -> io::Result<()> {
    let Found {
        file,
        len,
        canonical,
    } = found;
    let content_type = mime::content_type_for(&canonical);
    if method == Method::Head {
        return write_head(stream, 200, "OK", content_type, &[], len).await;
    }
    if len <= IN_MEMORY_LIMIT {
        let body = read_capped(file, len).await?;
        // The length actually written, not the length metadata predicted.
        write_head(stream, 200, "OK", content_type, &[], body.len() as u64).await?;
        stream.write_all(&body).await
    } else {
        write_head(stream, 200, "OK", content_type, &[], len).await?;
        let mut limited = file.take(len);
        // `take` bounds the copy from above, so the only disagreement this
        // can report is a body shorter than its own `Content-Length`.
        let sent = tokio::io::copy(&mut limited, stream).await?;
        if sent != len {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "{} shrank while it was being served: {len} bytes announced, {sent} sent \
                     — the connection is dropped, since the response cannot be completed",
                    canonical.display(),
                ),
            ));
        }
        Ok(())
    }
}

/// Read at most `cap` bytes of `file` into memory.
///
/// The cap is on the **read**, not on the metadata that predicted it. `resolve`
/// takes `len` from the handle's metadata and `send_file` uses it only to
/// choose a branch; a writer appending to the file between those two points —
/// an operator regenerating the bundle while `serve` runs is the innocent case
/// — would otherwise be buffered at the file's *new* size, past
/// `IN_MEMORY_LIMIT`. `Read::take` is the same discipline the CLI's own
/// admission control states for `--max-input-bytes` (contracts.md §6: "file
/// metadata is never trusted alone"), and it is what the streaming branch
/// already applies, so both branches now answer a mid-request growth
/// identically: the bytes that were there when the handle was opened.
///
/// **The other direction — a file that shrinks — is also handled here, and
/// only here** (R0009-0002). `read_to_end` stops at the real end of the file,
/// so a `cap` the file no longer reaches comes back short and `send_file`
/// announces the length of *this* `Vec` rather than the metadata's. Buffering
/// before announcing is what makes that possible, so it is a property of this
/// branch and not of the server: past `IN_MEMORY_LIMIT` the head goes out
/// first and a shrink can only be detected and the connection dropped.
async fn read_capped(file: tokio::fs::File, cap: u64) -> io::Result<Vec<u8>> {
    let mut body = Vec::with_capacity(cap as usize);
    file.take(cap).read_to_end(&mut body).await?;
    Ok(body)
}

/// A status-only response with a one-line `text/plain` body (none for `HEAD`,
/// which must not carry one).
///
/// `Content-Length` is the body's length under both methods. On a `HEAD`
/// response the field describes the representation a `GET` would have returned
/// (RFC 9110 §8.6), so zeroing it alongside the body would describe a
/// zero-length error page that no `GET` of this URL ever produces — and
/// `send_file` already gets that right for `200`s.
async fn status<S: AsyncWrite + Unpin>(
    stream: &mut S,
    method: Method,
    code: u16,
    reason: &str,
) -> io::Result<()> {
    status_detail(stream, method, code, reason, None).await
}

/// The same, with a second body line saying what to do about it. Reserved for
/// the refusals whose cause is a configuration the operator can change — a
/// `404` explains itself, a `421` does not.
async fn status_detail<S: AsyncWrite + Unpin>(
    stream: &mut S,
    method: Method,
    code: u16,
    reason: &str,
    detail: Option<&str>,
) -> io::Result<()> {
    let mut body = format!("{code} {reason}\n");
    if let Some(detail) = detail {
        body.push_str(detail);
        body.push('\n');
    }
    let len = body.len() as u64;
    write_head(stream, code, reason, "text/plain; charset=utf-8", &[], len).await?;
    if method == Method::Head {
        return Ok(());
    }
    stream.write_all(body.as_bytes()).await
}

/// Write the response head. Three headers are unconditional: `Connection:
/// close` (there is no second request on this socket), `X-Content-Type-Options:
/// nosniff` (the type came from the table in `mime` and the browser must not
/// re-decide it), and `Cache-Control: no-store` (a regenerated bundle must
/// never lose to a cached `sync.js` — `Troubleshooting.md` has that failure).
async fn write_head<S: AsyncWrite + Unpin>(
    stream: &mut S,
    code: u16,
    reason: &str,
    content_type: &str,
    extra: &[(&str, &str)],
    len: u64,
) -> io::Result<()> {
    let mut head = String::with_capacity(256);
    head.push_str(&format!("HTTP/1.1 {code} {reason}\r\n"));
    head.push_str(&format!("Content-Type: {content_type}\r\n"));
    head.push_str(&format!("Content-Length: {len}\r\n"));
    head.push_str("Connection: close\r\n");
    head.push_str("X-Content-Type-Options: nosniff\r\n");
    head.push_str("Cache-Control: no-store\r\n");
    for (name, value) in extra {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes()).await
}

/// Read up to and including the blank line that ends the request head.
///
/// Generic over the reader so the cap can be exercised against a source that
/// hands out chosen chunk sizes; `serve` passes the socket.
async fn read_head<R: AsyncRead + Unpin + ?Sized>(stream: &mut R) -> io::Result<HeadRead> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        if let Some(end) = head_end(&buf) {
            return Ok(HeadRead::Complete(
                String::from_utf8_lossy(&buf[..end]).into_owned(),
            ));
        }
        // The cap is on the head, not on the head plus whatever the last read
        // happened to carry. Asking only for the remaining room is what makes
        // the two the same number: a fixed-size read on top of a
        // near-full buffer would accept up to a chunk more than the constant
        // says, so the limit would be soft by the chunk size and would move
        // with the client's write pattern.
        let room = (MAX_HEAD_BYTES - buf.len()).min(chunk.len());
        if room == 0 {
            return Ok(HeadRead::TooLarge);
        }
        let read = stream.read(&mut chunk[..room]).await?;
        if read == 0 {
            return Ok(HeadRead::Incomplete);
        }
        buf.extend_from_slice(&chunk[..read]);
    }
}

/// Index of the blank line that ends a request head, if it has arrived.
///
/// **The earliest terminator wins, whichever spelling it has (R0009-0006).**
/// Searching for CRLF-CRLF first and falling back to LF-LF only when the
/// buffer holds no CRLF-CRLF *anywhere* read a bare-LF head terminated early
/// and followed later by a CRLF blank line at the wrong place: everything
/// between the two — body bytes — was handed to the header parser as fields.
/// That is framing disagreement of exactly the kind this server's no-keep-alive
/// design exists to erase, so the two searches are run independently and the
/// smaller index is taken.
fn head_end(buf: &[u8]) -> Option<usize> {
    let crlf = buf.windows(4).position(|w| w == b"\r\n\r\n");
    let lf = buf.windows(2).position(|w| w == b"\n\n");
    match (crlf, lf) {
        (Some(crlf), Some(lf)) => Some(crlf.min(lf)),
        (crlf, lf) => crlf.or(lf),
    }
}

/// `(method, target)` from the request line, or `None` if the line is not one.
///
/// Exactly three space-separated tokens, the third being a version this server
/// implements. Anything else is a `400` rather than a guess.
///
/// **The version token is matched, not prefixed (R0009-0004).** Accepting
/// anything starting `HTTP/` let `HTTP/2`, `HTTP/9.9` and `HTTP/nonsense`
/// through to a responder that answers `HTTP/1.1` with textual framing and
/// `Connection: close` — an HTTP/2 upgrade preface in particular is a request
/// this server cannot speak, and answering it in HTTP/1 leaves the two ends
/// disagreeing about the protocol rather than about one message. The two
/// tokens below are the two this server actually implements.
fn parse_request_line(head: &str) -> Option<(&str, &str)> {
    let line = head.lines().next()?;
    let mut parts = line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some() || !matches!(version, "HTTP/1.0" | "HTTP/1.1") {
        return None;
    }
    if method.is_empty() || target.is_empty() {
        return None;
    }
    Some((method, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_request_line_yields_its_method_and_target() {
        assert_eq!(
            parse_request_line("GET /index.html HTTP/1.1\r\nHost: x\r\n"),
            Some(("GET", "/index.html")),
        );
        assert_eq!(
            parse_request_line("HEAD / HTTP/1.0\r\n"),
            Some(("HEAD", "/"))
        );
    }

    #[test]
    fn a_line_that_is_not_a_request_line_is_refused_rather_than_guessed_at() {
        assert_eq!(parse_request_line(""), None);
        assert_eq!(parse_request_line("GET /index.html\r\n"), None);
        assert_eq!(parse_request_line("GET /a b HTTP/1.1\r\n"), None);
        assert_eq!(parse_request_line("GET /a NOTHTTP/1.1\r\n"), None);
        assert_eq!(
            parse_request_line("\0\0\0\0\r\n"),
            None,
            "a TLS ClientHello arriving on the plaintext port is not a request",
        );
        // R0009-0004: an `HTTP/` prefix is not a version this server speaks.
        for version in [
            "HTTP/2",
            "HTTP/2.0",
            "HTTP/0.9",
            "HTTP/9.9",
            "HTTP/nonsense",
        ] {
            assert_eq!(
                parse_request_line(&format!("GET / {version}\r\n")),
                None,
                "{version} is not a version this server implements",
            );
        }
    }

    /// The index returned is where the head's *text* stops — the start of the
    /// terminator — so slicing there yields the head and never any of the body.
    /// Asserting on that slice rather than on the number is the point: an
    /// off-by-one that fed a stray `\r` or a byte of body into
    /// `parse_request_line` would be invisible against a hand-counted index.
    #[test]
    fn the_head_ends_at_the_blank_line_and_not_before() {
        assert_eq!(head_end(b"GET / HTTP/1.1\r\n"), None);
        assert_eq!(head_end(b"GET / HTTP/1.1\r\nHost: x\r\n"), None);

        let crlf = b"GET / HTTP/1.1\r\nHost: x\r\n\r\nBODY";
        let end = head_end(crlf).expect("a blank line ends the head");
        assert_eq!(&crlf[..end], b"GET / HTTP/1.1\r\nHost: x");

        let lf = b"GET / HTTP/1.1\nHost: x\n\nBODY";
        let end = head_end(lf).expect("a bare-LF client still terminates its head");
        assert_eq!(&lf[..end], b"GET / HTTP/1.1\nHost: x");
    }

    /// R0009-0006: when both terminators are present the earlier one ends the
    /// head, whichever spelling it has. The mixed case is the whole point —
    /// preferring CRLF-CRLF *wherever it appears* fed the header parser every
    /// byte between the two blank lines, so a body could state fields.
    #[test]
    fn the_earlier_blank_line_ends_the_head_whichever_spelling_it_has() {
        let lf_first = b"GET / HTTP/1.1\n\nBODY\r\n\r\nSMUGGLED";
        let end = head_end(lf_first).expect("the first blank line ends the head");
        assert_eq!(&lf_first[..end], b"GET / HTTP/1.1");

        let crlf_first = b"GET / HTTP/1.1\r\n\r\nBODY\n\nMORE";
        let end = head_end(crlf_first).expect("the first blank line ends the head");
        assert_eq!(&crlf_first[..end], b"GET / HTTP/1.1");
    }

    /// A request head of exactly `total` bytes, terminator included.
    fn head_of(total: usize) -> String {
        const OVERHEAD: usize = "GET / HTTP/1.1\r\nX-Pad: \r\n\r\n".len();
        assert!(total >= OVERHEAD);
        format!(
            "GET / HTTP/1.1\r\nX-Pad: {}\r\n\r\n",
            "p".repeat(total - OVERHEAD)
        )
    }

    /// A reader that hands out `first` bytes and then the rest, so the read
    /// loop's buffer sits at a boundary the chunk size does not divide —
    /// which is the only arrangement in which a fixed-size read can overshoot.
    fn in_two_deliveries(head: &str, first: usize) -> impl AsyncRead + Unpin + '_ {
        let (delivered, rest) = head.as_bytes().split_at(first);
        delivered.chain(rest)
    }

    /// A file that grows *after* its length was measured is still read at the
    /// measured length. The in-memory branch is chosen on metadata, so trusting
    /// metadata for the read too would let an appending writer decide how large
    /// a buffer this server allocates — the eight-megabyte ceiling would hold
    /// only for files nobody is writing to.
    #[tokio::test]
    async fn a_file_that_grows_mid_request_is_read_at_the_length_that_was_measured() {
        let dir = crate::output::testing::scratch_dir("serve-grow");
        let path = dir.join("bundle.html");
        std::fs::write(&path, b"the length the head will announce").unwrap();

        let file = tokio::fs::File::open(&path).await.unwrap();
        let len = file.metadata().await.unwrap().len();

        // The window `resolve` documents: between the metadata call and the
        // read, another process appends to the same file.
        let mut appending = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut appending, &vec![b'x'; 4096]).unwrap();
        drop(appending);

        let body = read_capped(file, len).await.unwrap();
        assert_eq!(
            body.len() as u64,
            len,
            "the read is capped at the measured length, not at what the file has since become",
        );
        assert_eq!(body, b"the length the head will announce");
    }

    /// R0009-0002, the direction the cap cannot rescue. Past
    /// `IN_MEMORY_LIMIT` the `Content-Length` is on the wire before the body
    /// is read, so a file that **shrank** since `resolve` measured it leaves
    /// no truthful body to send. The copy ends short of the promise and the
    /// response fails, which is what makes `accept_loop` drop the socket with
    /// the message incomplete; before the fix the count `copy` returned was
    /// thrown away (`.map(|_| ())`) and a short body was reported as a
    /// delivered response.
    ///
    /// The shrink is staged as the state `resolve` hands on — a `len` the file
    /// no longer reaches — rather than by racing a truncation against a live
    /// server, because the two are the same input to `send_file` and only one
    /// of them is deterministic.
    #[tokio::test]
    async fn a_file_that_shrank_fails_the_response_instead_of_under_delivering_it() {
        let dir = crate::output::testing::scratch_dir("serve-shrink");
        let path = dir.join("big.bin");
        std::fs::write(&path, b"all that is left of it").unwrap();
        let file = tokio::fs::File::open(&path).await.unwrap();

        // Over the limit, so this is the streaming branch — and larger than
        // the file, which is exactly what a truncation between `metadata()`
        // and the copy leaves behind.
        let announced = IN_MEMORY_LIMIT + 1;
        let found = Found {
            file,
            len: announced,
            canonical: path.clone(),
        };

        let mut sent: Vec<u8> = Vec::new();
        let err = send_file(&mut sent, Method::Get, found)
            .await
            .expect_err("a body shorter than its Content-Length is not a delivered response");
        assert_eq!(
            err.kind(),
            io::ErrorKind::UnexpectedEof,
            "the connection is given up on, so `accept_loop` drops it: {err}",
        );

        let written = String::from_utf8_lossy(&sent).into_owned();
        assert!(
            written.contains(&format!("Content-Length: {announced}\r\n")),
            "the head had already promised the measured length: {written}",
        );
        assert!(
            sent.ends_with(b"all that is left of it"),
            "and what the file still had was written before the promise broke",
        );
    }

    /// The cap never invents bytes: a file shorter than its cap comes back
    /// whole, which is what keeps `Content-Length` the length actually written.
    #[tokio::test]
    async fn a_capped_read_below_the_cap_returns_the_whole_file() {
        let dir = crate::output::testing::scratch_dir("serve-short");
        let path = dir.join("small.txt");
        std::fs::write(&path, b"short").unwrap();
        let file = tokio::fs::File::open(&path).await.unwrap();
        let body = read_capped(file, IN_MEMORY_LIMIT).await.unwrap();
        assert_eq!(body, b"short");
    }

    /// `MAX_HEAD_BYTES` is the head's ceiling, not a number the last read may
    /// step over. Before this was exact, a head arriving on an unaligned
    /// boundary was accepted up to a whole chunk past the constant, so the
    /// documented cap was soft by 12.5% and moved with the client's write
    /// pattern.
    #[tokio::test]
    async fn the_head_cap_is_the_number_it_states() {
        let exact = head_of(MAX_HEAD_BYTES);
        match read_head(&mut in_two_deliveries(&exact, 100))
            .await
            .unwrap()
        {
            HeadRead::Complete(head) => assert!(
                head.starts_with("GET / HTTP/1.1"),
                "a head that fits exactly is still a head",
            ),
            _ => panic!("a head of exactly MAX_HEAD_BYTES must be served"),
        }

        for over in [MAX_HEAD_BYTES + 1, MAX_HEAD_BYTES + 700, MAX_HEAD_BYTES * 2] {
            let head = head_of(over);
            assert!(
                matches!(
                    read_head(&mut in_two_deliveries(&head, 100)).await.unwrap(),
                    HeadRead::TooLarge,
                ),
                "a {over}-byte head is over the {MAX_HEAD_BYTES}-byte cap",
            );
        }
    }

    /// The slow reader, reduced to its essence: a peer that accepts no bytes
    /// at all. Every write parks forever, which is what `RESPONSE_TIMEOUT`
    /// exists to bound.
    struct NeverReads;

    impl AsyncWrite for NeverReads {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            _buf: &[u8],
        ) -> std::task::Poll<io::Result<usize>> {
            std::task::Poll::Pending
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Pending
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Pending
        }
    }

    /// R0009-0001. The head read was bounded and the response was not, so a
    /// peer that sent a perfectly well-formed head and then stopped reading
    /// held one of `MAX_IN_FLIGHT` connection slots for the process's
    /// lifetime — 128 of them and the server is unavailable, with no
    /// malformed byte anywhere in the exchange.
    ///
    /// The refusal path is the one measured because it is the cheapest to
    /// stage — no file has to exist — and because it proves the deadline is
    /// not a property of `send_file` alone: a `404`'s head is bytes the peer
    /// will not take, and it parks exactly like a bundle would.
    #[tokio::test]
    async fn a_peer_that_stops_reading_does_not_hold_its_connection_slot() {
        let dir = crate::output::testing::scratch_dir("serve-slow-reader");
        let site = Arc::new(Site {
            root: dir.to_path_buf(),
            hosts: crate::serve_cmd::host::HostPolicy::new(
                "127.0.0.1:7470".parse().expect("test bind should parse"),
                &[],
            ),
        });
        let head = b"GET /absent.html HTTP/1.1\r\nHost: 127.0.0.1:7470\r\n\r\n";
        let stream = tokio::io::join(&head[..], NeverReads);

        let err = serve_within(stream, site, Duration::from_millis(50))
            .await
            .expect_err("a peer that never reads must not be awaited forever");
        assert_eq!(
            err.kind(),
            io::ErrorKind::TimedOut,
            "the connection is given up on, so `accept_loop` can drop it: {err}",
        );
    }
}
