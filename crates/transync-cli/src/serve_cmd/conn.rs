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
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

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

/// At or below this size the file is read into memory, so the
/// `Content-Length` announced is the length actually written. Above it the
/// body streams and the length comes from the open handle's metadata.
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
/// An `Err` here is a dead socket, not a rejected request — every rejection is
/// a status code written back on this same connection.
pub async fn serve(mut stream: TcpStream, site: Arc<Site>) -> io::Result<()> {
    let head = match tokio::time::timeout(HEAD_TIMEOUT, read_head(&mut stream)).await {
        Err(_elapsed) => return status(&mut stream, Method::Get, 408, "Request Timeout").await,
        Ok(Err(err)) => return Err(err),
        Ok(Ok(HeadRead::Incomplete)) => return Ok(()),
        Ok(Ok(HeadRead::TooLarge)) => {
            return status(
                &mut stream,
                Method::Get,
                431,
                "Request Header Fields Too Large",
            )
            .await;
        }
        Ok(Ok(HeadRead::Complete(head))) => head,
    };

    let Some((method_token, target)) = parse_request_line(&head) else {
        return status(&mut stream, Method::Get, 400, "Bad Request").await;
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
            return status(
                &mut stream,
                method.unwrap_or(Method::Get),
                400,
                "Bad Request",
            )
            .await;
        }
        Verdict::Elsewhere => {
            return status_detail(
                &mut stream,
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
            &mut stream,
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
            status(&mut stream, method, code, reason).await
        }
        Ok(found) => send_file(&mut stream, method, found).await,
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

async fn send_file(stream: &mut TcpStream, method: Method, found: Found) -> io::Result<()> {
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
        tokio::io::copy(&mut limited, stream).await.map(|_| ())
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
async fn status(stream: &mut TcpStream, method: Method, code: u16, reason: &str) -> io::Result<()> {
    status_detail(stream, method, code, reason, None).await
}

/// The same, with a second body line saying what to do about it. Reserved for
/// the refusals whose cause is a configuration the operator can change — a
/// `404` explains itself, a `421` does not.
async fn status_detail(
    stream: &mut TcpStream,
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
async fn write_head(
    stream: &mut TcpStream,
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
fn head_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4)
        .position(|w| w == b"\r\n\r\n")
        .or_else(|| buf.windows(2).position(|w| w == b"\n\n"))
}

/// `(method, target)` from the request line, or `None` if the line is not one.
///
/// Exactly three space-separated tokens, the third being an `HTTP/` version.
/// Anything else is a `400` rather than a guess.
fn parse_request_line(head: &str) -> Option<(&str, &str)> {
    let line = head.lines().next()?;
    let mut parts = line.split(' ');
    let method = parts.next()?;
    let target = parts.next()?;
    let version = parts.next()?;
    if parts.next().is_some() || !version.starts_with("HTTP/") {
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
}
