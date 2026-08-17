//! `transync serve` is a real loopback static server, and this is what it
//! promises (ticket `b791d6`, retiring `STUB-061`).
//!
//! The file it replaces, `serve_deferred.rs`, pinned the opposite: that the
//! command bound nothing and exited `5`. That contract is gone, so its
//! assertions are gone with it — except the one that outlived it, which is
//! kept below: a missing `--rendered` is still clap's argument error (`1`) and
//! not any of the server's own exit codes.
//!
//! Everything here talks HTTP over a raw `TcpStream` on purpose. A convenience
//! client normalizes the request target before sending it — `curl` collapses
//! `/../secret` into `/secret` and never puts the traversal on the wire — so a
//! traversal test written with one measures the *client*. The raw socket is
//! the only way to make the server answer the question.
//!
//! Ports come from the OS: every server starts with `--port 0` and the test
//! reads the port back from the `listening on` line, which the CLI prints from
//! `TcpListener::local_addr()`. That is the kernel's answer rather than an
//! echo of the argument, so the same line also proves *which address* was
//! bound — and it means these tests never collide with each other or with a
//! developer's own `7470`.
//!
//! Needs no provider and no feature gate: `serve` makes no provider call, so
//! this runs under a plain `cargo test --workspace`.
//!
//! TRACE: SCN-13
//! TRACE: contracts.md §6

mod common;

use common::ScratchDir;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, ChildStderr, Command, Stdio};
use std::time::Duration;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_transync")
}

/// A directory shaped like a `--html-out` bundle: the six filenames the CLI
/// emits, so the content-type assertions below are about the files that are
/// actually served rather than about invented ones. The real bundle — content
/// and all — is exercised end-to-end by `scripts/test-browser.sh`, which now
/// drives this same server.
fn write_bundle(dir: &Path) {
    std::fs::write(dir.join("index.html"), "<!doctype html><p>index</p>").unwrap();
    std::fs::write(dir.join("source.html"), "<p data-sync-id=\"p-0001\">s</p>").unwrap();
    std::fs::write(dir.join("target.html"), "<p data-sync-id=\"p-0001\">t</p>").unwrap();
    std::fs::write(dir.join("alignment.json"), "{\"schema_version\":\"1.0.0\"}").unwrap();
    std::fs::write(dir.join("sync.js"), "export function mountSync() {}\n").unwrap();
    std::fs::write(dir.join("purify.min.js"), "/* DOMPurify */\n").unwrap();
}

/// A running `transync serve`, killed when the test drops it.
struct Server {
    child: Child,
    stderr: BufReader<ChildStderr>,
    addr: String,
}

impl Server {
    /// Start a server over `root` and wait for its `listening on` line.
    fn start(root: &Path, extra: &[&str]) -> Server {
        let mut child = Command::new(bin())
            .arg("serve")
            .arg("--rendered")
            .arg(root)
            .arg("--port")
            .arg("0")
            .args(extra)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("transync binary must run");
        let mut stderr = BufReader::new(child.stderr.take().expect("stderr is piped"));

        let mut seen = String::new();
        let addr = loop {
            let mut line = String::new();
            let read = stderr
                .read_line(&mut line)
                .expect("stderr should be readable");
            assert!(
                read > 0,
                "serve exited before it announced a bound address; stderr =\n{seen}",
            );
            seen.push_str(&line);
            if let Some(rest) = line.split("listening on http://").nth(1) {
                let addr = rest
                    .split('/')
                    .next()
                    .expect("the announced URL should end at its path")
                    .trim()
                    .to_string();
                break addr;
            }
        };
        Server {
            child,
            stderr,
            addr,
        }
    }

    /// The `host:port` the kernel actually bound.
    fn addr(&self) -> &str {
        &self.addr
    }

    /// Send `raw` verbatim — no normalization, no header rewriting — and read
    /// the whole response back.
    fn raw(&self, raw: &str) -> Response {
        raw_to(self.addr(), raw)
    }

    fn get(&self, target: &str) -> Response {
        self.raw(&get_request(target, self.addr()))
    }

    /// Ask the process to stop the way an operator does, and report how it
    /// went: `(exit code, the stderr it printed after startup)`.
    #[cfg(unix)]
    fn interrupt_and_wait(mut self) -> (Option<i32>, String) {
        let pid = self.child.id().to_string();
        let killed = Command::new("kill")
            .args(["-INT", &pid])
            .status()
            .expect("kill(1) should run");
        assert!(killed.success(), "kill -INT {pid} failed");
        let status = self.child.wait().expect("serve should terminate");
        let mut tail = String::new();
        self.stderr.read_to_string(&mut tail).unwrap_or_default();
        // Drop's kill is a no-op on an already-reaped child.
        (status.code(), tail)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// One request, one response, one connection. The server closes after
/// answering, so "read to end" is an unambiguous message boundary.
fn raw_to(addr: &str, raw: &str) -> Response {
    let mut stream = TcpStream::connect(addr).expect("the server should accept");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    stream.write_all(raw.as_bytes()).unwrap();
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .expect("response should read");
    Response::parse(&bytes)
}

fn get_request(target: &str, host: &str) -> String {
    format!("GET {target} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n")
}

struct Response {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    fn parse(bytes: &[u8]) -> Response {
        let split = bytes
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .unwrap_or_else(|| {
                panic!(
                    "response should have a head: {:?}",
                    String::from_utf8_lossy(bytes)
                )
            });
        let head = String::from_utf8_lossy(&bytes[..split]).into_owned();
        let body = bytes[split + 4..].to_vec();
        let mut lines = head.lines();
        let status_line = lines.next().expect("a response has a status line");
        let status: u16 = status_line
            .split(' ')
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or_else(|| panic!("unparsable status line: {status_line}"));
        let headers = lines
            .filter_map(|line| {
                let (name, value) = line.split_once(':')?;
                Some((name.trim().to_ascii_lowercase(), value.trim().to_string()))
            })
            .collect();
        Response {
            status,
            headers,
            body,
        }
    }

    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

// --- the happy path -------------------------------------------------------

/// The bundle is servable: every file the CLI writes comes back, with the
/// content type its loader needs. `alignment.json` fetched as anything but
/// JSON and `sync.js` as anything but a script are both mount failures in the
/// shell, so these are the types SCN-13 depends on, not decoration.
#[test]
fn every_bundle_file_comes_back_with_its_content_type() {
    let root = ScratchDir::new("transync-serve-bundle");
    write_bundle(&root);
    let server = Server::start(&root, &[]);

    for (target, content_type) in [
        ("/index.html", "text/html; charset=utf-8"),
        ("/source.html", "text/html; charset=utf-8"),
        ("/target.html", "text/html; charset=utf-8"),
        ("/alignment.json", "application/json; charset=utf-8"),
        ("/sync.js", "text/javascript; charset=utf-8"),
        ("/purify.min.js", "text/javascript; charset=utf-8"),
    ] {
        let response = server.get(target);
        assert_eq!(response.status, 200, "GET {target}");
        assert_eq!(
            response.header("content-type"),
            Some(content_type),
            "GET {target}"
        );
        let expected = std::fs::read(root.join(target.trim_start_matches('/'))).unwrap();
        assert_eq!(response.body, expected, "GET {target} body");
        assert_eq!(
            response.header("content-length"),
            Some(expected.len().to_string().as_str()),
            "GET {target} content-length",
        );
    }

    // `/` is the shell, which is what a browser asks for first.
    let index = server.get("/");
    assert_eq!(index.status, 200);
    assert_eq!(
        index.header("content-type"),
        Some("text/html; charset=utf-8")
    );
    assert_eq!(index.body, std::fs::read(root.join("index.html")).unwrap());

    // Two headers every response carries: the type must not be re-sniffed,
    // and a regenerated bundle must not lose to a cached `sync.js`.
    assert_eq!(index.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(index.header("cache-control"), Some("no-store"));
}

/// An extension the table does not name is data, not something to guess at.
#[test]
fn an_unknown_extension_is_served_as_opaque_bytes() {
    let root = ScratchDir::new("transync-serve-unknown");
    write_bundle(&root);
    std::fs::write(root.join("payload.bin"), b"\x00\x01\x02").unwrap();
    let server = Server::start(&root, &[]);

    let response = server.get("/payload.bin");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("content-type"),
        Some("application/octet-stream"),
        "an unknown extension must not be guessed into something executable",
    );
    assert_eq!(response.body, b"\x00\x01\x02");
}

/// A body past the in-memory limit streams instead of being buffered, and the
/// two paths must agree on what `Content-Length` means. Nothing in a bundle is
/// this big — the wasm module is the largest thing anyone lays beside one, at
/// under 2 MB — but `--rendered` takes any directory, so the branch is
/// reachable and is pinned rather than assumed.
#[test]
fn a_body_past_the_in_memory_limit_streams_intact() {
    let root = ScratchDir::new("transync-serve-large");
    write_bundle(&root);
    // One byte over the 8 MiB threshold, with a recognizable tail so a
    // truncated stream cannot pass by matching a prefix.
    let mut big = vec![b'x'; 8 * 1024 * 1024 + 1];
    let tail = b"END-OF-STREAM";
    big.truncate(big.len() - tail.len());
    big.extend_from_slice(tail);
    std::fs::write(root.join("big.bin"), &big).unwrap();
    let server = Server::start(&root, &[]);

    let response = server.get("/big.bin");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.header("content-length"),
        Some(big.len().to_string().as_str()),
    );
    assert_eq!(response.body.len(), big.len(), "the whole body arrives");
    assert_eq!(response.body, big);
}

/// `HEAD` answers the same headers with no body — a `Content-Length` that
/// described a body it also sent would break any client that trusts it.
#[test]
fn head_answers_the_headers_and_no_body() {
    let root = ScratchDir::new("transync-serve-head");
    write_bundle(&root);
    let server = Server::start(&root, &[]);

    let get = server.get("/index.html");
    let head = server.raw(&format!(
        "HEAD /index.html HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        server.addr()
    ));
    assert_eq!(head.status, 200);
    assert_eq!(head.header("content-type"), get.header("content-type"));
    assert_eq!(head.header("content-length"), get.header("content-length"));
    assert!(head.body.is_empty(), "a HEAD response carries no body");
}

/// The same promise on the refusals. `Content-Length` on a `HEAD` response
/// describes the representation a `GET` would have returned, so a `HEAD` that
/// answers `404` announces the length of the `404` page — announcing `0`
/// describes a response no `GET` of that URL ever produces, and a prober that
/// reads it learns the wrong thing about every error this server can give.
#[test]
fn head_and_get_agree_on_the_length_of_an_error_representation() {
    let root = ScratchDir::new("transync-serve-head-error");
    write_bundle(&root);
    let server = Server::start(&root, &[]);

    // A 404 and a 403, so the agreement is about the status path and not
    // about one code that happens to be right.
    for target in ["/missing.html", "/../outside.txt"] {
        let get = server.get(target);
        let head = server.raw(&format!(
            "HEAD {target} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            server.addr()
        ));
        assert_eq!(head.status, get.status, "HEAD {target}");
        assert_ne!(get.body.len(), 0, "the GET body is the representation");
        assert_eq!(
            head.header("content-length"),
            get.header("content-length"),
            "HEAD {target} must announce the length a GET would have sent",
        );
        assert!(head.body.is_empty(), "a HEAD response carries no body");
    }
}

// --- the refusals ---------------------------------------------------------

/// A `..` on the wire — the request `curl` refuses to send — is refused, and
/// the file one level up never appears in any response.
#[test]
fn a_dot_dot_traversal_is_refused() {
    let outside = ScratchDir::new("transync-serve-traversal");
    let root = outside.join("bundle");
    std::fs::create_dir_all(&root).unwrap();
    write_bundle(&root);
    std::fs::write(outside.join("secret.txt"), "TOP-SECRET-CANARY").unwrap();
    let server = Server::start(&root, &[]);

    for target in [
        "/../secret.txt",
        "/./../secret.txt",
        "/sub/../../secret.txt",
        "/%2e%2e/secret.txt",
        "/%2E%2E/secret.txt",
    ] {
        let response = server.get(target);
        assert_eq!(
            response.status,
            403,
            "GET {target} must be refused, got {} / {}",
            response.status,
            response.text(),
        );
        assert!(
            !response.text().contains("TOP-SECRET-CANARY"),
            "GET {target} leaked the file above the root",
        );
    }

    // A separator the *decode* invents is a different refusal — the request
    // was never a single path component — but it is still a refusal.
    for target in ["/..%2fsecret.txt", "/%2f%2e%2e%2fsecret.txt"] {
        let response = server.get(target);
        assert_eq!(response.status, 400, "GET {target}");
        assert!(!response.text().contains("TOP-SECRET-CANARY"));
    }
}

/// The check no request-text inspection can make: a symlink *inside* the root
/// pointing outside it. Lexical confinement passes this request; canonicalizing
/// after resolution is what stops it.
#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_root_is_refused_and_one_inside_it_is_not() {
    let outside = ScratchDir::new("transync-serve-symlink");
    let root = outside.join("bundle");
    std::fs::create_dir_all(&root).unwrap();
    write_bundle(&root);
    std::fs::write(outside.join("secret.txt"), "TOP-SECRET-CANARY").unwrap();
    std::os::unix::fs::symlink(outside.join("secret.txt"), root.join("escape.html")).unwrap();
    std::os::unix::fs::symlink("index.html", root.join("alias.html")).unwrap();
    let server = Server::start(&root, &[]);

    let escaped = server.get("/escape.html");
    assert_eq!(
        escaped.status,
        403,
        "a symlink leaving the root must be refused, got {}",
        escaped.text(),
    );
    assert!(!escaped.text().contains("TOP-SECRET-CANARY"));

    // The containment check must not over-refuse: a link that stays inside
    // the root is an ordinary file.
    let alias = server.get("/alias.html");
    assert_eq!(alias.status, 200, "a link within the root is servable");
    assert_eq!(alias.body, std::fs::read(root.join("index.html")).unwrap());
}

/// `--rendered` may itself be a symlink, and the containment check must not
/// turn that into a server that refuses everything.
///
/// The root is canonicalized once at startup, so it is the *link target* that
/// every request is measured against. Without that, every canonical result
/// would sit outside the link path the operator typed and every request would
/// be a `403` — a plausible-looking way to ship a server that serves nothing.
#[cfg(unix)]
#[test]
fn a_symlinked_rendered_root_serves_what_it_points_at() {
    let base = ScratchDir::new("transync-serve-linkedroot");
    let real = base.join("bundle");
    std::fs::create_dir_all(&real).unwrap();
    write_bundle(&real);
    let link = base.join("link-to-bundle");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let server = Server::start(&link, &[]);
    let response = server.get("/index.html");
    assert_eq!(response.status, 200);
    assert_eq!(
        response.body,
        std::fs::read(real.join("index.html")).unwrap()
    );
}

/// No listing, in either spelling. A listing publishes filenames the operator
/// chose to serve but never chose to advertise.
#[test]
fn a_directory_is_never_listed() {
    let root = ScratchDir::new("transync-serve-listing");
    write_bundle(&root);
    std::fs::create_dir_all(root.join("private")).unwrap();
    std::fs::write(root.join("private/notes.txt"), "notes").unwrap();
    let server = Server::start(&root, &[]);

    for target in ["/private", "/private/"] {
        let response = server.get(target);
        assert_eq!(response.status, 404, "GET {target}");
        assert!(
            !response.text().contains("notes.txt"),
            "GET {target} listed the directory",
        );
    }
    // The root itself has an index.html, so `/` is a page rather than a
    // listing — and the file inside the subdirectory is still reachable by
    // name, which is what makes the refusal above about listing and not about
    // access.
    assert_eq!(server.get("/private/notes.txt").status, 200);
}

/// Serve only. Anything that could write, tunnel or probe is a `405` naming
/// what is allowed.
#[test]
fn only_get_and_head_are_answered() {
    let root = ScratchDir::new("transync-serve-methods");
    write_bundle(&root);
    let server = Server::start(&root, &[]);

    for method in ["POST", "PUT", "DELETE", "OPTIONS", "CONNECT", "TRACE"] {
        let response = server.raw(&format!(
            "{method} /index.html HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            server.addr()
        ));
        assert_eq!(response.status, 405, "{method} must not be served");
        assert_eq!(response.header("allow"), Some("GET, HEAD"));
    }

    // A request line that is not one at all is a refusal rather than a guess.
    let garbage = server.raw("not-a-request\r\n\r\n");
    assert_eq!(garbage.status, 400);
}

// --- who the request is for -----------------------------------------------

/// The DNS-rebinding request, on the wire: a browser that reached this socket
/// under a name the attacker owns. The packet is indistinguishable from a
/// legitimate one *except* for the authority it names, and after the rebind
/// the browser treats whatever comes back as same-origin with the attacker's
/// page — so serving it hands the translated document to script on
/// `attacker.example`. It is refused, and refused before the served root is
/// consulted, so not even the shape of the bundle comes back.
#[test]
fn a_request_naming_another_authority_is_refused() {
    let root = ScratchDir::new("transync-serve-rebind");
    write_bundle(&root);
    std::fs::write(
        root.join("index.html"),
        "<!doctype html><p>PRIVATE-CANARY</p>",
    )
    .unwrap();
    let server = Server::start(&root, &[]);
    let port = server
        .addr()
        .rsplit_once(':')
        .expect("the announced address is host:port")
        .1
        .to_string();

    for host in [
        format!("attacker.example:{port}"),
        "attacker.example".to_string(),
        // The bound address is not the same authority on another port, and a
        // sibling loopback address is not this server either.
        format!("127.0.0.2:{port}"),
        "127.0.0.1:1".to_string(),
    ] {
        for target in ["/index.html", "/", "/nothing-here.html"] {
            let response = server.raw(&get_request(target, &host));
            assert_eq!(
                response.status, 421,
                "GET {target} for {host} must not be answered",
            );
            assert!(
                !response.text().contains("PRIVATE-CANARY"),
                "GET {target} for {host} served the bundle",
            );
        }
    }

    // The refusal says what to do about it, because the operator who trips it
    // is the one who reached the server by a name it was not told about.
    let refused = server.raw(&get_request("/index.html", "demo.example"));
    assert!(
        refused.text().contains("--allow-host"),
        "the refusal should name its remedy; body = {}",
        refused.text(),
    );
}

/// A request that does not state exactly one authority states none. Both
/// shapes are `400`: with no `Host` there is nothing to check, and with two
/// there is no answer to "which one" that some other reader would not answer
/// differently.
#[test]
fn a_request_that_does_not_state_one_authority_is_refused() {
    let root = ScratchDir::new("transync-serve-host-count");
    write_bundle(&root);
    let server = Server::start(&root, &[]);
    let addr = server.addr().to_string();

    let missing = server.raw("GET /index.html HTTP/1.1\r\nConnection: close\r\n\r\n");
    assert_eq!(missing.status, 400, "a request with no Host is refused");

    let twice = server.raw(&format!(
        "GET /index.html HTTP/1.1\r\nHost: {addr}\r\nHost: attacker.example\r\n\
         Connection: close\r\n\r\n",
    ));
    assert_eq!(twice.status, 400, "two Host fields are refused");

    let twice_reversed = server.raw(&format!(
        "GET /index.html HTTP/1.1\r\nHost: attacker.example\r\nHost: {addr}\r\n\
         Connection: close\r\n\r\n",
    ));
    assert_eq!(
        twice_reversed.status, 400,
        "and the order does not rescue it",
    );

    // Still servable to a request that states one authority, so the refusals
    // above are about the header and not about the server having stopped.
    assert_eq!(server.get("/index.html").status, 200);
}

/// `localhost` is the name the smoke checklists and the browser address bar
/// use for a loopback bind, and RFC 6761 reserves it for exactly that address
/// — so it is answered for, while the check still holds against every name an
/// attacker can point somewhere.
#[test]
fn localhost_names_the_same_server_as_the_loopback_address() {
    let root = ScratchDir::new("transync-serve-localhost");
    write_bundle(&root);
    let server = Server::start(&root, &[]);
    let port = server
        .addr()
        .rsplit_once(':')
        .expect("the announced address is host:port")
        .1
        .to_string();

    for host in [format!("localhost:{port}"), format!("LOCALHOST:{port}")] {
        assert_eq!(
            server.raw(&get_request("/index.html", &host)).status,
            200,
            "{host}",
        );
    }
}

/// `--allow-host` is how a bind that cannot name its own reachable authority
/// gets told one. It widens the policy by exactly what it is given.
#[test]
fn allow_host_adds_an_authority_and_only_that_one() {
    let root = ScratchDir::new("transync-serve-allowhost");
    write_bundle(&root);
    let server = Server::start(&root, &["--allow-host", "demo.example"]);
    let port = server
        .addr()
        .rsplit_once(':')
        .expect("the announced address is host:port")
        .1
        .to_string();

    assert_eq!(
        server
            .raw(&get_request("/index.html", &format!("demo.example:{port}")))
            .status,
        200,
        "the allowed authority is answered for",
    );
    assert_eq!(
        server.get("/index.html").status,
        200,
        "and the bound address still is",
    );
    assert_eq!(
        server
            .raw(&get_request(
                "/index.html",
                &format!("other.example:{port}"),
            ))
            .status,
        421,
        "one --allow-host allows one authority, not any name",
    );
}

/// `--allow-host` takes an authority, and a value that is not one is clap's
/// error before anything opens a socket — the same treatment `--bind` gives a
/// non-address.
#[test]
fn an_allow_host_value_that_is_not_an_authority_is_an_argument_error() {
    let root = ScratchDir::new("transync-serve-badallowhost");
    write_bundle(&root);

    let out = Command::new(bin())
        .arg("serve")
        .arg("--rendered")
        .arg(&root)
        .arg("--allow-host")
        .arg("demo.example:not-a-port")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr = {}",
        String::from_utf8_lossy(&out.stderr),
    );
}

// --- binding --------------------------------------------------------------

/// Loopback by default: with no `--bind`, the address the kernel reports is a
/// loopback one. `local_addr()` is the source, so this is the socket's own
/// answer and not an echo of the flag.
#[test]
fn the_default_bind_is_loopback() {
    let root = ScratchDir::new("transync-serve-loopback");
    write_bundle(&root);
    let server = Server::start(&root, &[]);

    let (host, _port) = server
        .addr()
        .rsplit_once(':')
        .expect("the announced address is host:port");
    assert_eq!(
        host, "127.0.0.1",
        "serve must bind loopback unless --bind says otherwise",
    );
    assert_eq!(server.get("/index.html").status, 200);
}

/// A non-loopback bind happens **only** when it is asked for, and says so.
///
/// This test does bind an ephemeral port on every interface for the length of
/// one request; that exposure is the behavior under test, and the point is
/// that reaching it took an explicit flag.
#[test]
fn a_non_loopback_bind_takes_an_explicit_flag_and_warns() {
    let root = ScratchDir::new("transync-serve-anyaddr");
    write_bundle(&root);
    let mut server = Server::start(&root, &["--bind", "0.0.0.0"]);

    let (host, port) = server
        .addr()
        .rsplit_once(':')
        .expect("the announced address is host:port");
    assert_eq!(host, "0.0.0.0", "--bind must reach the socket");
    // `0.0.0.0` includes loopback, so the bundle is still reachable there —
    // which is how we know the socket is live and not merely announced.
    // A wildcard bind still answers for the loopback authority it is also
    // listening on, which is how we know the socket is live and not merely
    // announced.
    let via_loopback = raw_to(
        &format!("127.0.0.1:{port}"),
        &get_request("/index.html", &format!("127.0.0.1:{port}")),
    );
    assert_eq!(via_loopback.status, 200);

    let mut warning = String::new();
    let mut line = String::new();
    while server.stderr.read_line(&mut line).unwrap_or(0) > 0 {
        warning.push_str(&line);
        if line.contains("press Ctrl-C") {
            break;
        }
        line.clear();
    }
    assert!(
        warning.contains("WARNING") && warning.contains("not a loopback address"),
        "a non-loopback bind must be called out; stderr =\n{warning}",
    );
}

// --- startup refusals -----------------------------------------------------

/// The flags are parsed, not ignored: omitting the required `--rendered` is
/// clap's argument error (`1`), a different code from every one the server
/// itself can return. Inherited from `serve_deferred.rs`, which is the one
/// assertion of that file the real server did not invalidate.
#[test]
fn serve_still_rejects_missing_required_flag() {
    let out = Command::new(bin())
        .arg("serve")
        .output()
        .expect("transync binary must run");

    assert_eq!(
        out.status.code(),
        Some(1),
        "a missing --rendered is ArgumentError(1): stderr = {}",
        String::from_utf8_lossy(&out.stderr),
    );
}

/// `--rendered` has to name a readable directory, and that is an input
/// failure (`2`) rather than "other" — the same code `translate` gives an
/// `--input` that is not there.
#[test]
fn a_rendered_path_that_is_not_a_directory_is_exit_two() {
    let root = ScratchDir::new("transync-serve-notdir");
    let file = root.join("index.html");
    std::fs::write(&file, "<p>not a directory</p>").unwrap();

    for path in [file.clone(), root.join("does-not-exist")] {
        let out = Command::new(bin())
            .arg("serve")
            .arg("--rendered")
            .arg(&path)
            .arg("--port")
            .arg("0")
            .output()
            .expect("transync binary must run");
        assert_eq!(
            out.status.code(),
            Some(2),
            "--rendered {} should be exit 2: stderr = {}",
            path.display(),
            String::from_utf8_lossy(&out.stderr),
        );
        assert!(out.stdout.is_empty(), "serve writes nothing to stdout");
    }
}

/// An address that cannot be bound exits `5` rather than hanging or
/// pretending. `203.0.113.1` is TEST-NET-3 (RFC 5737): never assigned to a
/// local interface, so the bind fails without a packet leaving the machine.
#[test]
fn an_unbindable_address_is_exit_five() {
    let root = ScratchDir::new("transync-serve-badbind");
    write_bundle(&root);

    let out = Command::new(bin())
        .arg("serve")
        .arg("--rendered")
        .arg(&root)
        .arg("--port")
        .arg("0")
        .arg("--bind")
        .arg("203.0.113.1")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        out.status.code(),
        Some(5),
        "an unbindable address is Other(5): stderr = {}",
        String::from_utf8_lossy(&out.stderr),
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("203.0.113.1"),
        "the failure must name the address it could not bind",
    );
}

/// `--bind` takes an address, and a non-address is clap's error before
/// anything opens a socket.
#[test]
fn a_bind_value_that_is_not_an_address_is_an_argument_error() {
    let root = ScratchDir::new("transync-serve-badbindvalue");
    write_bundle(&root);

    let out = Command::new(bin())
        .arg("serve")
        .arg("--rendered")
        .arg(&root)
        .arg("--bind")
        .arg("not-an-address")
        .output()
        .expect("transync binary must run");
    assert_eq!(
        out.status.code(),
        Some(1),
        "stderr = {}",
        String::from_utf8_lossy(&out.stderr),
    );
}

// --- shutdown -------------------------------------------------------------

/// Ctrl-C stops the server the way an operator expects: it says it is
/// stopping and exits `0`. A server that had to be killed would exit on a
/// signal instead, and a script chaining off `transync serve` would read that
/// as a failure.
#[cfg(unix)]
#[test]
fn ctrl_c_shuts_down_cleanly() {
    let root = ScratchDir::new("transync-serve-shutdown");
    write_bundle(&root);
    let server = Server::start(&root, &[]);
    assert_eq!(server.get("/index.html").status, 200);

    let (code, tail) = server.interrupt_and_wait();
    assert_eq!(code, Some(0), "Ctrl-C is a clean stop, not a failure");
    assert!(
        tail.contains("shutting down"),
        "the shutdown should be announced; stderr tail =\n{tail}",
    );
}
