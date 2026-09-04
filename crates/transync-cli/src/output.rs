//! Output writers. `write_fileset_atomic` performs a **staged fileset
//! commit** for the individual CLI outputs (out.md, alignment map, and the
//! optional six-file `--html-out` bundle per ADR-0006): every payload is
//! staged and fsynced before the first rename, so a failure during the
//! content-writing phase leaves no target touched (R0006-0012).
//! `publish_out_dir` extends the same staged-commit discipline to
//! `--out-dir`, publishing a whole directory tree by rename (EXT-2026-07
//! P1-6) — one rename onto a fresh target, two when an existing target has
//! to be moved aside first.
//!
//! Both entry points take a [`lock::PublishLock`] over every directory they
//! publish into, so two concurrent runs aimed at the same outputs serialize
//! rather than interleaving their rename passes (R0001-0034) — plus, on each
//! side, the one directory the *other* mode would be holding instead: a
//! fileset commit claims the deepest existing level on its way to a
//! destination it has to create ([`fileset::claim_anchor`]), and a directory publish
//! claims the directories inside its target (`publish::published_dirs_inside`). That
//! pair is what closes DCR-0021's nested boundary — `--out-dir X` racing a
//! files-mode publish into `X` (ti `40e2a5`).
//!
//! No pair of claims closes it at *every* depth: a replace claims a fixed,
//! shallow set while a publication claims a point that can be arbitrarily deep
//! (ti `cbbc4e`). What bounds the gap is not a third claim but the two orderings
//! around it. A publication reads its anchor **before** it creates anything, so
//! the level it claims is one that existed when it started — for a destination
//! under an `--out-dir` target, that is the target or a level above it, both of
//! which the replace holds. And [`preflight::ensure_out_dir_replaceable`] runs again with
//! the staged tree in hand, so a directory that appeared deeper inside the
//! target meanwhile makes the replace refuse instead of renaming it away.
//! Together they leave the replace destroying only what it was shown, and the
//! remainder — `--force`, which waives the guard by request, and the syscalls
//! between the second guard and the rename — is stated in contracts.md §6.
//!
//! One rule governs every deletion in this module, because a publisher that
//! cleans up after itself is one bad predicate away from deleting somebody
//! else's work: **remove only what this call created, and prove it.** The
//! staged tree proves it by `create_dir` under a per-run random token
//! (R0002-0001); the directory levels a failed staging rolls back prove it by
//! being empty when the rollback reaches them (R0002-0002) — anything a peer
//! added while this run was staging makes the removal a no-op instead of a
//! loss — *and* by nobody holding their publish lock at that moment
//! (R0003-0001), because an empty directory a peer has already locked is a
//! directory that peer is about to publish into.
//!
//! The mechanics live in one child module per concern, and this module
//! publishes the surface the commands call:
//!
//! * [`fileset`] — the staged fileset commit ([`write_fileset_atomic`]);
//! * [`publish`] — the `--out-dir` directory publication
//!   ([`publish_out_dir`]);
//! * [`preflight`] — the guards that decide whether either one may start;
//! * [`destination`] — destination vetting and lexical path normalization;
//! * [`bundle`] — the `--html-out` shell's embedded assets and template;
//! * [`lock`] — the publish lock both commits take.
//!
//! What stays here is what more than one of them needs: the [`Notify`]
//! channel, the residue note both cleanup paths compose, and the
//! best-effort directory fsync.
//!
//! TRACE: persistence-and-files.md §staged-fileset-commit
//! TRACE: ADR-0006
//! TRACE: DCR-0021

mod bundle;
mod destination;
mod fileset;
mod lock;
mod preflight;
mod publish;

pub use bundle::html_bundle_files;
pub use fileset::write_fileset_atomic;
pub use preflight::{
    html_bundle_paths, preflight_destination_set, preflight_html_out, preflight_out_dir,
};
pub use publish::publish_out_dir;

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

/// Where this layer's operator notices go: one already-composed line per
/// call, which the caller renders through its reporter (`transync: <line>`)
/// and `--quiet` suppresses. Publication knows facts nobody above it can
/// observe, and none of them is worth a bespoke channel:
///
/// * it is waiting behind another run;
/// * it is leaving another run's staging temps alone;
/// * a directory's flush to disk failed — or the directory could not even be
///   opened to try — so the entries naming the published files may not
///   survive a crash (R0002-0025, R0003-0021);
/// * a best-effort cleanup after a failed publication left residue behind
///   (staged temps, or directory levels this run created) — the run still
///   reports its original error, the note only says what is still on disk
///   (R0003-0022, R0003-0024);
/// * an existing destination's permissions could not be carried over to the
///   file replacing it, so it now carries this run's default mode
///   (R0003-0023).
///
/// Everything on that list is *advisory*: none of it changes what the run
/// returns, because none of it changes whether the published bytes are
/// correct.
///
/// TRACE: contracts.md §6
pub type Notify<'a> = &'a dyn Fn(&str);

/// Say — once, whatever the count — that a best-effort cleanup left something
/// on disk, naming the count and one concrete example so the sentence stays
/// bounded however many entries failed (R0003-0022, R0003-0024). The residue
/// is inert; the note exists so "this ran and could not finish" is not
/// indistinguishable from "there was nothing to clean up".
///
/// `occasion` is when the sweep ran, because this module has two kinds: the
/// obligations of a publication that failed, and the opportunistic passes that
/// clear a crashed predecessor's own residue before a publication starts
/// (R0004-0056). Both leave the same kind of leftover behind when they fail
/// and both owe the operator the same sentence; only the clause naming *when*
/// differs.
fn note_cleanup_residue(
    what: &str,
    occasion: &str,
    failures: &[(PathBuf, io::Error)],
    notify: Notify<'_>,
) {
    let Some((path, e)) = failures.first() else {
        return;
    };
    notify(&format!(
        "note: could not clean up {} {what} {occasion} (e.g. {}: {e}); the residue is inert — \
         delete it by hand once no transync run is active",
        failures.len(),
        path.display(),
    ));
}

/// The `occasion` of a cleanup a failed publication owed ([`note_cleanup_residue`]).
const AFTER_A_FAILED_PUBLICATION: &str = "after a failed publication";

/// The `occasion` of the opportunistic sweeps that clear this process's own
/// crashed-predecessor residue on the way in ([`note_cleanup_residue`]).
const BEFORE_THIS_PUBLICATION: &str = "found before this publication";

/// The zero-byte-ish marker a `--out-dir` publication writes into the tree it
/// publishes, as proof that transync produced this directory (ti `66339b`,
/// OI-0036).
///
/// It is staged with the rest of the tree and renamed into place with it, so a
/// published target carries it from the instant it exists, and a target that
/// does not carry it was not published by this tool.
pub const OUT_DIR_MARKER_NAME: &str = ".transync-out-dir";

/// Best-effort fsync of a directory so a preceding rename/create is durable.
///
/// Best-effort, and **audible** (R0002-0025). The publication is not as durable
/// as this writer claims, nobody but this writer can know it, and swallowing it
/// left the run reporting an unqualified success. It is not fatal — the payload
/// bytes are already `sync_all`ed one by one (`fileset::stage_one_file`,
/// `publish::stage_out_dir_files`, which propagate their errors) so the worst a crash
/// can now expose is the previous consistent state, never a torn one.
///
/// Silence is reserved for the platform saying it has no such operation — see
/// [`no_directory_handle`]. R0002-0025 made the *sync* audible but left every
/// open failure quiet on the strength of that Windows case, which also
/// swallowed the Unix `EACCES`/`EIO` that mean the same thing as a failed sync:
/// this directory was not flushed (R0003-0021).
fn fsync_dir(dir: &Path, notify: Notify<'_>) {
    let f = match File::open(dir) {
        Ok(f) => f,
        Err(e) => {
            if !no_directory_handle(&e) {
                notify(&fsync_failure_note(dir, &e));
            }
            return;
        }
    };
    if let Err(e) = f.sync_all() {
        notify(&fsync_failure_note(dir, &e));
    }
}

/// Whether a failure to open a directory means "this platform has no directory
/// handle to flush" rather than "this directory could not be flushed".
///
/// On Windows every such open fails and there is no equivalent operation to
/// report on, so the whole call is a silent no-op there — that is the rationale
/// R0002-0025 recorded, and it is kept, narrowed to the platforms it is true
/// of. On Unix a directory opens for read like any other file, so an error is
/// about *this* directory and is worth saying out loud; only an explicit
/// `Unsupported` (a filesystem that refuses the open outright) is the platform
/// speaking.
#[cfg(unix)]
fn no_directory_handle(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::Unsupported
}

#[cfg(not(unix))]
fn no_directory_handle(_e: &io::Error) -> bool {
    true
}

/// The operator-facing sentence for a failed directory fsync: which directory,
/// what is at risk, and what is not.
fn fsync_failure_note(dir: &Path, e: &io::Error) -> String {
    format!(
        "note: could not flush the directory {} to disk ({e}); the published file CONTENTS are \
         durable, but the directory entries naming them may not survive a crash — re-run the \
         publication if the machine goes down before the filesystem catches up",
        dir.display()
    )
}

/// Scratch-directory helper shared by this module's tests and [`lock`]'s.
#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    /// A fresh, empty directory for one test, removed when the returned guard
    /// drops. Prefers the workspace scratch volume (the same rule
    /// `tests/common/mod.rs` uses) and falls back to the platform temp dir
    /// when it is not mounted.
    ///
    /// `Drop` is the cleanup point rather than a call at the end of each test
    /// because a test that panics never reaches its last statement — so
    /// end-of-test removal cleans up exactly the runs that did not need it.
    /// Nothing cleaned these up at all before ticket `2fef6a`, and the
    /// twenty-odd directories this module left behind per run accumulated
    /// until the scratch volume filled and an unrelated test failed with
    /// `StorageFull`, which reads like a regression in the code under test and
    /// is not one. Removal is best-effort: it must never turn a passing test
    /// red or abort an unwind already in progress.
    pub(crate) fn scratch_dir(prefix: &str) -> ScratchDir {
        let preferred = Path::new("/Volumes/Temp/claude");
        let root = if preferred.exists() {
            preferred.to_path_buf()
        } else {
            std::env::temp_dir()
        };
        let path = root.join(format!(
            "transync-output-{prefix}-{pid}-{nanos}-{n}",
            pid = std::process::id(),
            nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            n = NEXT_SCRATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        ));
        std::fs::create_dir_all(&path).expect("scratch dir should be creatable");
        ScratchDir { path }
    }

    /// Distinguishes the scratch directories one test binary creates; the pid
    /// distinguishes the binaries. The clock alone does not settle it — two
    /// threads under `--test-threads=N` can read the same nanosecond, and a
    /// self-removing guard must never own a directory another guard owns too.
    static NEXT_SCRATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    /// Owns one scratch directory for the life of the binding; derefs to its
    /// path.
    pub(crate) struct ScratchDir {
        path: PathBuf,
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    impl std::ops::Deref for ScratchDir {
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
}

#[cfg(test)]
mod tests {
    use super::testing::scratch_dir;
    use super::*;
    use crate::output::preflight::HTML_BUNDLE_ENTRIES;
    use crate::output::publish::HTML_SUBDIR;
    use std::cell::RefCell;
    use std::fs::File;

    pub(super) fn silent(_: &str) {}

    /// The complete set a `--out-dir` publish writes, as destinations relative
    /// to the target — what [`publish_out_dir`]'s caller hands it.
    pub(super) fn out_dir_payloads(marker: u8) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = vec![
            (PathBuf::from("out.md"), vec![marker; 8]),
            (PathBuf::from("alignment.json"), vec![marker; 8]),
            (PathBuf::from("validation-report.json"), vec![marker; 8]),
        ];
        for name in HTML_BUNDLE_ENTRIES {
            files.push((Path::new(HTML_SUBDIR).join(name), vec![marker; 8]));
        }
        files
    }

    /// R0002-0025: a directory whose fsync fails no longer passes in silence.
    /// The sentence has to carry all three facts an operator needs — which
    /// directory, that the entry may not survive a crash, and that the file
    /// contents are not what is at risk.
    #[test]
    fn a_failed_directory_flush_is_reported_with_its_scope() {
        let note = fsync_failure_note(
            Path::new("/out/published"),
            &io::Error::from(io::ErrorKind::Other),
        );
        assert!(
            note.contains("/out/published"),
            "name the directory: {note}"
        );
        assert!(
            note.contains("CONTENTS are durable"),
            "say what is NOT at risk: {note}"
        );
        assert!(
            note.contains("may not survive a crash"),
            "say what IS at risk: {note}"
        );
    }

    /// The sentence is half of R0002-0025; the other half is that [`fsync_dir`]
    /// actually *speaks* when the flush fails, instead of swallowing the error
    /// as it used to. No directory a test creates can be made to fail
    /// `sync_all` portably, but macOS's `devfs` supplies a genuine one: `/dev`
    /// opens for read like any directory and answers `fsync` with `ENOTSUP`.
    /// That is the vehicle, so the test asserts the vehicle still works before
    /// it asserts the behaviour — a silent pass would hide the loss of
    /// coverage rather than report it.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_directory_that_cannot_be_flushed_reaches_the_operator() {
        let devfs = Path::new("/dev");
        let err = File::open(devfs)
            .expect("/dev opens for read")
            .sync_all()
            .expect_err("devfs must still refuse fsync — this test has no other vehicle");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        fsync_dir(devfs, &sink);
        let notices = notices.into_inner();

        assert_eq!(
            notices.len(),
            1,
            "a failed flush must produce exactly one note: {notices:?}"
        );
        assert!(
            notices[0].contains("/dev") && notices[0].contains(&err.to_string()),
            "the note must name the directory and the real error: {}",
            notices[0]
        );
        assert!(
            notices[0].contains("CONTENTS are durable"),
            "and still bound the damage: {}",
            notices[0]
        );
    }

    /// R0003-0021: the *other* way a flush does not happen is that the
    /// directory never opened, and R0002-0025 left every one of those quiet on
    /// the strength of a Windows-only rationale. On Unix a directory opens like
    /// any file, so a failure is about this directory and reaches the operator
    /// with the same sentence a failed `sync_all` gets.
    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_even_be_opened_reaches_the_operator() {
        let root = scratch_dir("fsync-open");
        let gone = root.join("never-created");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        fsync_dir(&gone, &sink);
        let notices = notices.into_inner();

        assert_eq!(
            notices.len(),
            1,
            "a directory that cannot be opened must produce exactly one note: {notices:?}"
        );
        assert!(
            notices[0].contains(&gone.display().to_string()),
            "the note must name the directory: {}",
            notices[0]
        );
        assert!(
            notices[0].contains("CONTENTS are durable"),
            "and still bound the damage: {}",
            notices[0]
        );
    }

    /// What one publication reports back to the test that is racing it: that
    /// it has parked on a lock, or that it has finished.
    pub(super) enum Step {
        Parked,
        Done(Result<(), String>),
    }
}
