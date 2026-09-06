//! `transync serve` — a loopback static file server for an `--html-out`
//! bundle.
//!
//! Serves, and does nothing else. There is no upload, no directory listing, no
//! execution and no proxying: a request either names a regular file inside the
//! directory given to `--rendered` or it is refused. Four properties carry
//! that claim, and each lives in one place:
//!
//! - **Path confinement** — [`route`]: the request target is decoded
//!   segment-by-segment and refused if it names anything but a plain descent,
//!   then the resolved path is canonicalized and re-checked against the
//!   canonical root, so a symlink pointing out of the bundle is a `403` rather
//!   than a file.
//! - **Loopback by default** — `--bind` defaults to `127.0.0.1`. A
//!   non-loopback bind is possible and warned about on stderr; it is never the
//!   default, because the default would otherwise publish a local document
//!   tree to every machine on the network.
//! - **An answered-for authority** — [`host`]: every request must name, in its
//!   `Host` field, an authority this server was reached as. A loopback bind
//!   decides which network can open the socket and nothing about which page
//!   gets to read the answer; `Host` is the only thing that separates a
//!   browser asking for `127.0.0.1` from one that was rebound onto it under a
//!   name the attacker owns.
//! - **A fixed content-type table** — [`mime`]: extensions only, never content
//!   sniffing, with `application/octet-stream` for everything the table does
//!   not name.
//!
//! Ctrl-C (and `SIGTERM`) stops the accept loop, gives in-flight connections a
//! short grace period, and exits `0`.
//!
//! History: this command shipped as a deferred stub (`STUB-061`) from SL-13
//! until ticket `b791d6` — it parsed its flags, printed the address it would
//! have bound, and exited `5`. That contract is retired; `contracts.md` §6
//! carries the live one.
//!
//! TRACE: SCN-13
//! TRACE: contracts.md §6

mod conn;
mod host;
mod mime;
mod route;

use crate::error::ExitCode;
use clap::Args;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::task::{JoinError, JoinSet};

/// How many connections may be in flight before the accept loop waits for one
/// to finish. A demo server has no business holding thousands of tasks, and an
/// unbounded `JoinSet` is how a local process runs itself out of descriptors.
const MAX_IN_FLIGHT: usize = 128;

/// How long a shutdown waits for in-flight connections before closing anyway.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(2);

/// How long the accept loop pauses after an accept error, so a persistent one
/// (descriptor exhaustion) does not spin the CPU.
const ACCEPT_BACKOFF: Duration = Duration::from_millis(50);

/// Argument struct for `transync serve`.
///
/// TRACE: contracts.md §6
#[derive(Debug, Args)]
pub struct ServeArgs {
    /// Directory to serve. Normally an `--html-out` bundle.
    #[arg(long)]
    pub rendered: PathBuf,
    /// TCP port. `0` asks the OS for a free one, which is then printed.
    #[arg(long, default_value_t = 7470)]
    pub port: u16,
    /// Address to bind. Anything but a loopback address is reachable from
    /// other machines and is warned about.
    #[arg(long, default_value = "127.0.0.1")]
    pub bind: IpAddr,
    /// Another authority to answer for, as `host` or `host:port` (a bare host
    /// means the bound port). Repeatable. The bound address — and `localhost`
    /// when that address is a loopback one — is always answered for; this is
    /// for the names a bind cannot name, such as the address a `--bind
    /// 0.0.0.0` server is reached at from another machine.
    #[arg(long = "allow-host", value_name = "AUTHORITY")]
    pub allow_host: Vec<host::Authority>,
}

/// Everything a connection needs that outlives it: the canonical served root,
/// and the authorities this server answers for.
struct Site {
    root: PathBuf,
    hosts: host::HostPolicy,
}

/// Execute the subcommand.
///
/// Exit codes, per `contracts.md` §6: `0` when a signal stopped the server,
/// `1` for an argument error (clap's, before this runs), `2` when `--rendered`
/// is not a readable directory, `5` when the address could not be bound.
///
/// TRACE: SCN-13
pub async fn run(args: ServeArgs) -> i32 {
    // The canonical root, resolved once. Every request is checked against
    // *this* path, so a symlinked `--rendered` is served as the directory it
    // points at rather than being refused on every request.
    let root = match tokio::fs::canonicalize(&args.rendered).await {
        Ok(root) => root,
        Err(err) => {
            eprintln!(
                "transync serve: cannot serve {}: {err}",
                args.rendered.display()
            );
            return ExitCode::InputReadFailure as i32;
        }
    };
    match tokio::fs::metadata(&root).await {
        Ok(meta) if meta.is_dir() => {}
        Ok(_) => {
            eprintln!(
                "transync serve: {} is not a directory; --rendered takes the bundle directory.",
                args.rendered.display()
            );
            return ExitCode::InputReadFailure as i32;
        }
        Err(err) => {
            eprintln!(
                "transync serve: cannot serve {}: {err}",
                args.rendered.display()
            );
            return ExitCode::InputReadFailure as i32;
        }
    }

    let listener = match TcpListener::bind(SocketAddr::new(args.bind, args.port)).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!(
                "transync serve: cannot bind {}: {err}",
                SocketAddr::new(args.bind, args.port)
            );
            return ExitCode::Other as i32;
        }
    };
    // The kernel's answer, not the argument's: with `--port 0` this is the
    // port that was actually assigned, and it is the only address any caller
    // should trust.
    let local = match listener.local_addr() {
        Ok(local) => local,
        Err(err) => {
            eprintln!("transync serve: cannot read the bound address: {err}");
            return ExitCode::Other as i32;
        }
    };

    let site = Arc::new(Site {
        root,
        hosts: host::HostPolicy::new(local, &args.allow_host),
    });

    eprintln!(
        "transync serve: listening on http://{local}/ — serving {}",
        site.root.display()
    );
    // Printed rather than left to be discovered from a refusal: an operator
    // who reaches this server by a name it does not answer for gets a `421`
    // whose remedy is this list, and seeing the list at startup is how that
    // stops being a surprise.
    eprintln!(
        "transync serve: answering for {} — a request naming another authority is refused \
         (--allow-host adds one).",
        site.hosts.answered_authorities(),
    );
    if !local.ip().is_loopback() {
        eprintln!(
            "transync serve: WARNING: {} is not a loopback address — everything under {} is \
             reachable from other machines on this network.",
            local.ip(),
            site.root.display()
        );
    }
    eprintln!("transync serve: press Ctrl-C to stop.");

    accept_loop(listener, site).await;
    ExitCode::Success as i32
}

/// Accept until a shutdown signal arrives, then drain.
async fn accept_loop(listener: TcpListener, site: Arc<Site>) {
    let mut connections = JoinSet::new();
    let mut shutdown = std::pin::pin!(shutdown_signal());
    let mut stopping = false;

    while !stopping {
        // Hold the in-flight count under the cap, reaping finished tasks.
        while connections.len() >= MAX_IN_FLIGHT {
            tokio::select! {
                joined = connections.join_next() => {
                    if let Some(joined) = joined {
                        report_connection_exit(joined);
                    }
                }
                _ = &mut shutdown => { stopping = true; break; }
            }
        }
        if stopping {
            break;
        }
        tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok((stream, _peer)) => {
                    let site = Arc::clone(&site);
                    connections.spawn(async move {
                        // A dead socket is the peer's business, not an error
                        // worth printing on every reload.
                        let _ = conn::serve(stream, site).await;
                    });
                }
                Err(err) => {
                    eprintln!("transync serve: accept failed: {err}");
                    tokio::time::sleep(ACCEPT_BACKOFF).await;
                }
            },
            Some(joined) = connections.join_next(), if !connections.is_empty() => {
                report_connection_exit(joined);
            }
            _ = &mut shutdown => { stopping = true; }
        }
    }

    eprintln!("transync serve: shutting down.");
    // Stop accepting first, so nothing new arrives during the grace period.
    drop(listener);
    drain_connections(&mut connections).await;
}

/// Give the in-flight connections [`SHUTDOWN_GRACE`] to finish, then close
/// anyway.
///
/// Lifted out of [`accept_loop`] because it is the one of the three join sites
/// a test can drive: it needs the `JoinSet` and nothing else, so a task that
/// panics can go through the real drain rather than through a re-implementation
/// of it. The other two need a bound listener and a live peer, and there is no
/// panic site in non-test `serve_cmd/` to reach them with — which is what made
/// OI-0050 a tracked gap rather than a live defect.
async fn drain_connections(connections: &mut JoinSet<()>) {
    let drained = tokio::time::timeout(SHUTDOWN_GRACE, async {
        while let Some(joined) = connections.join_next().await {
            report_connection_exit(joined);
        }
    })
    .await;
    if drained.is_err() {
        eprintln!(
            "transync serve: {} connection(s) still open after {}s; closing anyway.",
            connections.len(),
            SHUTDOWN_GRACE.as_secs(),
        );
        connections.abort_all();
    }
}

/// What a joined connection task has to say for itself, or `None` when it
/// finished on its own terms.
///
/// A `JoinError` is never the peer's failure: an ordinary per-connection I/O
/// error is discarded one level down, deliberately — "a dead socket is the
/// peer's business", the record R0010-0076 was dropped on — so a peer reset, a
/// response timeout and OI-0045's `UnexpectedEof` never reach a join at all.
/// Only two things do, and the notice says which: a panic somewhere in the
/// connection path, or a task this server cancelled. Neither is fatal — the
/// accept loop keeps going, and OI-0050 asks only that the death stop being
/// indistinguishable from a normal return.
///
/// Split from the printing so the wording can be asserted without capturing
/// stderr.
///
/// TRACE: OI-0050 / R0010-0077
fn connection_exit_notice(joined: Result<(), JoinError>) -> Option<String> {
    let err = joined.err()?;
    // `JoinError`'s own `Display` carries the task id and, for a panic, the
    // payload string — the only part that names the bug. What it does not
    // carry is that this one is transync's and not the peer's, which is the
    // whole distinction the notice exists to draw.
    Some(if err.is_panic() {
        format!(
            "a connection task panicked; the server keeps accepting, but this is a bug in \
             transync rather than a peer failure: {err}"
        )
    } else {
        format!("a connection task was cancelled before it finished: {err}")
    })
}

/// Print [`connection_exit_notice`]. The one report path the join sites share,
/// so the three of them cannot drift apart.
fn report_connection_exit(joined: Result<(), JoinError>) {
    if let Some(notice) = connection_exit_notice(joined) {
        eprintln!("transync serve: {notice}");
    }
}

/// Resolves on Ctrl-C, and on `SIGTERM` where there is one — a supervisor that
/// stops the server (Playwright's `webServer`, a shell's `kill`) sends the
/// latter, and a server that only listens for Ctrl-C gets killed instead of
/// shutting down.
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// The report OI-0050 asks for. There is no panic site in non-test
    /// `serve_cmd/`, so the panic is injected here and joined the way the
    /// server joins — a real task, a real `JoinSet`, a real `JoinError`.
    #[tokio::test]
    async fn a_connection_task_that_panicked_is_reported_as_a_panic_and_carries_its_message() {
        let mut connections = JoinSet::new();
        connections.spawn(async {
            panic!("a slicing bug in header handling");
        });
        let joined = connections
            .join_next()
            .await
            .expect("the one spawned task joins");

        let notice = connection_exit_notice(joined).expect("a panic is not a normal return");
        assert!(notice.contains("panicked"), "{notice}");
        assert!(
            notice.contains("a slicing bug in header handling"),
            "the payload is the only part that names the bug: {notice}",
        );
        assert!(
            !notice.contains("cancelled"),
            "a panic and a cancellation are the two things a JoinError can be, and the \
             operator is told which: {notice}",
        );
    }

    /// The other half of "say which".
    #[tokio::test]
    async fn a_cancelled_connection_task_is_reported_as_a_cancellation_rather_than_a_panic() {
        let mut connections = JoinSet::new();
        connections.spawn(std::future::pending::<()>());
        connections.abort_all();
        let joined = connections
            .join_next()
            .await
            .expect("the aborted task still joins");

        let notice =
            connection_exit_notice(joined).expect("a cancellation is not a normal return either");
        assert!(notice.contains("cancelled"), "{notice}");
        assert!(!notice.contains("panicked"), "{notice}");
    }

    /// The policy this fix must not touch: an ordinary connection stays silent.
    /// `accept_loop` discards `conn::serve`'s own errors before the join can
    /// see them — "a dead socket is the peer's business" — so a task that
    /// returned, however its I/O went, has nothing to report.
    #[tokio::test]
    async fn a_connection_that_finished_on_its_own_terms_reports_nothing() {
        let mut connections = JoinSet::new();
        connections.spawn(async {
            // The shape `accept_loop` spawns: the connection's own error is
            // discarded inside the task, so the join sees a normal return.
            let _ = Err::<(), std::io::Error>(std::io::ErrorKind::ConnectionReset.into());
        });
        let joined = connections
            .join_next()
            .await
            .expect("the one spawned task joins");

        assert!(connection_exit_notice(joined).is_none());
    }

    /// The panic goes through the real join site, not a re-implementation of
    /// one: `drain_connections` reports it and keeps draining, because a
    /// panicking connection must not become fatal to the server. The wording is
    /// asserted above; what is asserted here is that the drain reached the
    /// report and then finished its work.
    #[tokio::test]
    async fn the_drain_survives_a_panicking_task_and_still_joins_the_rest() {
        let finished = Arc::new(AtomicBool::new(false));
        let mut connections = JoinSet::new();
        connections.spawn(async {
            panic!("an expect added under time pressure");
        });
        let flag = Arc::clone(&finished);
        connections.spawn(async move {
            tokio::task::yield_now().await;
            flag.store(true, Ordering::SeqCst);
        });

        drain_connections(&mut connections).await;

        assert!(
            finished.load(Ordering::SeqCst),
            "a sibling's panic does not end the drain",
        );
        assert!(connections.is_empty(), "every task was joined");
    }
}
