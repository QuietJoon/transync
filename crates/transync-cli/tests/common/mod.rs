//! Shared helpers for the `transync-cli` integration-test suite.
//!
//! Both `cli_smoke.rs` and `serve_static.rs` pull this in with `mod common;`,
//! and `cli_smoke.rs` is compiled with and without `test-stub-provider`, so a
//! given target does not necessarily reach every item here. The crate-level
//! allow below is what keeps `-D warnings` honest for the target that does
//! not, and matches the shape `crates/transync/tests/common/mod.rs` already
//! uses.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// A uniquely-named scratch directory that removes itself when it drops.
///
/// The scratch root is the workspace temp volume when it is mounted and the
/// OS temp dir otherwise — the rule both test files carried privately before
/// this module existed.
///
/// `Drop` is the cleanup point on purpose rather than a `remove_dir_all` at
/// the end of each test: a test that panics unwinds past its last statement,
/// so end-of-test removal cleans up exactly the runs that did not need it and
/// leaks exactly the ones that did. Removal is best-effort — a scratch
/// directory that will not go away must not turn a passing test red, and must
/// never abort an unwind that is already in progress.
///
/// The name carries a process-wide counter as well as the clock: two threads
/// can read the same nanosecond, and with a self-removing guard a shared name
/// would mean one test deleting another's files rather than the harmless
/// sharing the clock-only name risked before (ticket `2fef6a`).
pub struct ScratchDir {
    path: PathBuf,
}

/// Distinguishes scratch directories created within one test binary; the pid
/// distinguishes the binaries from each other.
static NEXT: AtomicU64 = AtomicU64::new(0);

impl ScratchDir {
    /// Create `<scratch root>/<prefix>-<pid>-<nanos>-<n>` and own it.
    pub fn new(prefix: &str) -> ScratchDir {
        let preferred = Path::new("/Volumes/Temp/claude");
        let root = if preferred.exists() {
            preferred.to_path_buf()
        } else {
            std::env::temp_dir()
        };
        let unique = format!(
            "{prefix}-{pid}-{nanos}-{n}",
            pid = std::process::id(),
            nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            n = NEXT.fetch_add(1, Ordering::Relaxed),
        );
        let path = root.join(unique);
        std::fs::create_dir_all(&path).expect("scratch dir should be creatable");
        ScratchDir { path }
    }

    /// The directory this guard owns.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

impl Deref for ScratchDir {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<Path> for ScratchDir {
    fn as_ref(&self) -> &Path {
        &self.path
    }
}

impl AsRef<OsStr> for ScratchDir {
    fn as_ref(&self) -> &OsStr {
        self.path.as_os_str()
    }
}
