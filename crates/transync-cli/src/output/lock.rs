//! Cross-process exclusion for output publication (R0001-0034).
//!
//! Staging to `<name>.tmp.<pid>` keeps two runs from colliding *while they
//! write*, but nothing covered the publication itself: two `transync`
//! processes aimed at the same `--output` / `--map` / `--html-out` paths could
//! interleave their rename passes and leave a bundle mixing two translations.
//! [`PublishLock`] closes that window by locking every directory a run
//! publishes into, held from the foreign-file guard through the last rename,
//! so two runs that share any output directory serialize instead of
//! interleaving.
//!
//! The lock is an OS file lock (`flock`-equivalent, via `std::fs::File`) taken
//! on `<dir>/.transync-publish.lock`. Two properties make that the right
//! primitive here, and both are why this is not a hand-rolled `create_new`
//! lease:
//!
//! * The kernel releases it when the holding file descriptor closes —
//!   including on `SIGKILL`, on a panic, and across a reboot. A crashed run
//!   therefore cannot leave behind a lock that blocks every later run, which
//!   is the failure mode an advisory lock *file* has to paper over with
//!   timeouts and steal protocols.
//! * It is per open file description, so two *threads* of one process exclude
//!   each other exactly as two processes do.
//!
//! The lock file is a zero-byte marker and is deliberately **not** removed on
//! release: unlinking it would let a later run create a fresh inode at the
//! same path and lock that instead, while a waiting run still held the old
//! one — two "exclusive" holders at once. Both directory guards
//! (`super::preflight::scan_bundle_dir`, [`super::preflight::ensure_out_dir_replaceable`]) list it
//! as a transync file, so it is never mistaken for foreign content.
//!
//! One caller does unlink a marker: the rollback of a directory level a failed
//! publication created and left empty, which would otherwise leave a directory
//! containing nothing but the marker. It goes through
//! [`hold_marker_for_removal`] rather than deleting the file outright, so no
//! marker is unlinked while a peer *holds* its lock. A peer merely *queued* in
//! `lock()` is invisible to that probe — POSIX advisory locks report no
//! waiters — so the waiter checks for itself: [`lock_marker`] verifies that the
//! marker it was granted is still the marker at the path, and re-locks the
//! current one when it is not (ti `8792b7`). That is what makes every unlink
//! safe rather than only the ones a probe can see.
//!
//! Scope, stated honestly: the lock covers runs publishing *into* the same
//! directory, and — since ti `40e2a5` — a `--out-dir` publish that *replaces*
//! a directory another run is publishing into, which was DCR-0021's stated
//! boundary. Closing it is the callers' work rather than this module's: each
//! mode also locks the one directory the other would be holding instead
//! ([`super::fileset::claim_anchor`], `super::publish::published_dirs_inside`). What is left to
//! *this* primitive is a peer publishing **deeper** inside an `--out-dir`
//! target than the `html/` level transync itself writes, and a peer built
//! before this lock existed. Neither is a lock problem: no finite claim set
//! covers every depth, and a run that takes no lock is not excludable by one.
//! Both are bounded outside this module — by reading a claim before creating
//! the level it stands for, and by restating the `--out-dir` guard with the
//! staged tree in hand, so a replace refuses a target that grew a directory
//! while it was staging (ti `cbbc4e`).
//!
//! TRACE: DCR-0021
//! TRACE: contracts.md §6

use super::Notify;
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

/// The zero-byte marker file transync locks in every directory it publishes
/// into. Recognized by the `--html-out` and `--out-dir` guards as a transync
/// file, never as foreign content.
pub const PUBLISH_LOCK_NAME: &str = ".transync-publish.lock";

/// An exclusive hold on every directory of one publication, released when the
/// value is dropped (or when the process dies).
///
/// The held descriptors are the whole value: closing them is what releases the
/// locks, so the field exists to be kept alive, not to be read.
pub struct PublishLock {
    _held: Vec<File>,
}

impl PublishLock {
    /// Lock every directory in `dirs` (deduplicated) and keep the locks until
    /// the returned value is dropped. Each directory must already exist.
    ///
    /// Contention is reported through `notify` once per directory and then
    /// *waited* on rather than refused: the peer holding the lock is a
    /// transync run in its (short) publication phase, and serializing behind
    /// it is exactly the wanted behavior.
    pub fn acquire(dirs: &[PathBuf], notify: Notify<'_>) -> io::Result<Self> {
        let mut held = Vec::new();
        for dir in lock_order(dirs) {
            held.push(lock_marker(&dir, notify)?);
        }
        Ok(Self { _held: held })
    }
}

/// How many times [`lock_marker`] will re-lock after finding that the marker it
/// was granted is no longer the marker at that path.
///
/// A bound rather than an unbounded loop because the retry is driven by other
/// processes: each one costs an open and a lock, and nothing in this process
/// can promise the churn ends. Sixteen is far past any real sequence — the only
/// caller that unlinks a marker does so once, while removing the directory it
/// sits in — and exhausting it is reported rather than papered over.
const MARKER_REACQUISITIONS: usize = 16;

/// Lock `dir`'s publish marker, and **verify that the marker locked is the one
/// at that path** before treating the lock as held.
///
/// A lock file that can be unlinked needs this: the descriptor keeps the inode
/// alive after the entry is gone, so a run granted the lock on an unlinked
/// marker holds an exclusion nobody else can see. The sequence is three deep
/// and every step is legal on its own (R0003-0001 residual, ti `8792b7`):
///
/// 1. A holds the lock and B blocks in `lock()` on marker inode `I1`. POSIX
///    advisory locks report no waiters, so B is invisible to A.
/// 2. A's rollback finds the marker unheld — it is: B is queued, not holding —
///    unlinks `I1` and removes the directory level.
/// 3. C recreates the directory, creates marker `I2`, locks it, and publishes.
///    B is granted `I1` and publishes too: two "exclusive" publishers of one
///    directory, interleaving their renames.
///
/// Revalidating on the **waiter** side closes it wherever it arises, rather
/// than closing the one window a remover-side probe can see: B compares the
/// `dev`+`ino` it holds against the path's, finds them different (or the path
/// gone), drops the descriptor and locks the current marker instead. The
/// contention notice is emitted at most once however many times that repeats —
/// the operator is being told they are queued, not how the queue is
/// implemented.
///
/// On platforms without a `dev`+`ino` identity the check is a documented no-op
/// (see [`same_file`]) and the loop runs exactly once, which is the behavior
/// this function had before.
fn lock_marker(dir: &Path, notify: Notify<'_>) -> io::Result<File> {
    let path = dir.join(PUBLISH_LOCK_NAME);
    let mut announced = false;
    for _ in 0..MARKER_REACQUISITIONS {
        let file = open_lock_file(dir)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                if !announced {
                    notify(&format!(
                        "note: waiting for another transync run to finish publishing into {}",
                        dir.display()
                    ));
                    announced = true;
                }
                file.lock().map_err(|e| lock_error(dir, &e))?;
            }
            Err(TryLockError::Error(e)) => return Err(lock_error(dir, &e)),
        }
        if same_file(&file, &path) {
            return Ok(file);
        }
        // The marker moved out from under this lock. Dropping the descriptor
        // releases an exclusion that guards nothing, and the next pass opens
        // whatever is at the path now — creating it when the path is empty.
        drop(file);
    }
    Err(io::Error::other(format!(
        "gave up locking {} for publication: its {PUBLISH_LOCK_NAME} marker was replaced \
         {MARKER_REACQUISITIONS} times while this run waited for it",
        dir.display()
    )))
}

/// Whether the open `file` is the file currently named by `path`, compared by
/// filesystem identity (`dev` + `ino`) rather than by path string.
///
/// `false` also covers "the path names nothing" and "the metadata could not be
/// read": an identity that cannot be established is not an identity that
/// matched, and the one caller that acts on `true` is deciding whether an
/// exclusion is real.
#[cfg(unix)]
fn same_file(file: &File, path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let (Ok(held), Ok(named)) = (file.metadata(), fs::metadata(path)) else {
        return false;
    };
    held.dev() == named.dev() && held.ino() == named.ino()
}

/// Windows has no portable file identity in `std` (`file_index` is documented
/// as not guaranteed), so the revalidation is skipped there and the lock
/// behaves exactly as it did before ti `8792b7` — the same platform shape
/// `super::fileset::preserve_target_permissions` uses. The exposure it leaves is the
/// three-run sequence in [`lock_marker`], whose window is a few syscalls wide.
#[cfg(not(unix))]
fn same_file(_file: &File, _path: &Path) -> bool {
    true
}

/// The distinct directories of one publication, in a stable global order.
///
/// Sorting by the *resolved* path is what keeps two runs whose directory sets
/// only partially overlap from deadlocking: every run takes the shared
/// directories in the same sequence. Resolution also collapses two spellings
/// of one directory (`out/html` vs an absolute path, a symlinked parent) into
/// a single lock instead of two.
fn lock_order(dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut ordered: Vec<PathBuf> = dirs
        .iter()
        .map(|dir| fs::canonicalize(dir).unwrap_or_else(|_| dir.clone()))
        .collect();
    ordered.sort();
    ordered.dedup();
    ordered
}

/// Open (creating if needed) the lock marker in `dir`. Opened for writing
/// because a lock needs a descriptor, not because anything is ever written:
/// the file stays empty.
fn open_lock_file(dir: &Path) -> io::Result<File> {
    let path = dir.join(PUBLISH_LOCK_NAME);
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("could not open the publish lock {}: {e}", path.display()),
            )
        })
}

/// A hold taken on a directory's publish-lock marker by a caller that is about
/// to **delete** it, released as soon as that caller is done.
///
/// Like [`PublishLock`], the descriptor is the whole value: it exists to be
/// kept alive across the deletion, not to be read. While it lives no peer can
/// take the lock on the inode it names, so no peer can slip between "nobody
/// holds this" and "the marker is gone".
pub struct MarkerHold {
    _held: File,
}

/// Take `dir`'s publish lock without waiting, on behalf of a rollback that
/// wants to unlink the marker.
///
/// `Some` means no peer holds the lock, and none can take it until the returned
/// value drops. `None` means **do not delete**, and deliberately collapses two
/// answers into one instruction:
///
/// * a peer holds the lock — the case this exists for. A rolled-back run that
///   unlinked the marker underneath it would leave that peer locking an
///   unlinked inode while the next run created a fresh marker at the same path
///   and locked *that*: two runs, both "the exclusive publisher" of one
///   directory, interleaving their renames. That is the module invariant above,
///   not merely a lost run.
/// * the probe could not decide — the marker vanished, the open failed, the
///   lock call errored. Leaving a directory behind is inert; deleting one out
///   from under a peer is not, so an undecidable probe answers the same way a
///   held one does.
///
/// Opened read-only and **without** `create`: the question is about a marker
/// that already exists, and a probe that created one would only ever be
/// answering about its own file. The hold is granted only if the locked inode
/// is still the one at the path ([`same_file`]) — otherwise the probe would
/// answer about a marker some other run already replaced, and the caller would
/// unlink the replacement.
///
/// TRACE: DCR-0021
pub fn hold_marker_for_removal(dir: &Path) -> Option<MarkerHold> {
    let path = dir.join(PUBLISH_LOCK_NAME);
    let file = File::open(&path).ok()?;
    match file.try_lock() {
        Ok(()) if same_file(&file, &path) => Some(MarkerHold { _held: file }),
        _ => None,
    }
}

/// Name the directory whose lock could not be taken; the bare OS error says
/// nothing about which publication step failed.
fn lock_error(dir: &Path, e: &io::Error) -> io::Error {
    io::Error::new(
        e.kind(),
        format!("could not lock {} for publication: {e}", dir.display()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::testing::scratch_dir;

    fn silent(_: &str) {}

    /// The lock excludes a second holder for as long as it lives, and only for
    /// as long as it lives. Probed with the same OS primitive a second process
    /// would use, so the assertion is about the real lock and not about
    /// bookkeeping this module keeps.
    #[test]
    fn a_held_lock_excludes_a_second_holder_until_it_is_dropped() {
        let dir = scratch_dir("lock-exclusion");
        let dirs = [dir.to_path_buf()];
        let lock = PublishLock::acquire(&dirs, &silent).expect("first hold");

        let probe = File::open(dir.join(PUBLISH_LOCK_NAME)).expect("lock file exists");
        assert!(
            matches!(probe.try_lock(), Err(TryLockError::WouldBlock)),
            "a second holder must be refused while the publish lock is held"
        );
        drop(probe);

        drop(lock);
        let probe = File::open(dir.join(PUBLISH_LOCK_NAME)).expect("lock file survives release");
        assert!(
            probe.try_lock().is_ok(),
            "dropping the publish lock must release it"
        );
    }

    /// The rollback probe (R0003-0001) answers "do not remove this marker" for
    /// both reasons it can have — a peer holding the lock, and a probe that
    /// cannot decide — and, when it does say yes, keeps saying it: the returned
    /// hold is what closes the gap between the answer and the `remove_file`.
    #[test]
    fn the_removal_probe_refuses_a_held_marker_and_an_undecidable_one() {
        let dir = scratch_dir("marker-probe");
        assert!(
            hold_marker_for_removal(&dir).is_none(),
            "no marker to probe is not evidence that removing one is safe"
        );

        let peer = PublishLock::acquire(&[dir.to_path_buf()], &silent).expect("peer holds");
        assert!(
            hold_marker_for_removal(&dir).is_none(),
            "a marker a peer holds the lock on is not the rollback's to unlink"
        );
        drop(peer);

        let hold = hold_marker_for_removal(&dir).expect("an unheld marker is removable");
        let probe = File::open(dir.join(PUBLISH_LOCK_NAME)).expect("the marker survives the probe");
        assert!(
            matches!(probe.try_lock(), Err(TryLockError::WouldBlock)),
            "the hold must keep a peer out for as long as the remover keeps it"
        );
        drop(probe);
        drop(hold);
    }

    /// ti `8792b7` / the R0003-0001 residual: a run granted the lock on a
    /// marker that has since been **unlinked** holds an exclusion nobody else
    /// can see — the next run creates a fresh marker at the same path and locks
    /// that, and both are "the exclusive publisher" of one directory. The
    /// waiter has to notice, because the remover's probe cannot: POSIX advisory
    /// locks report no waiters, so a peer queued inside `lock()` is invisible to
    /// it.
    ///
    /// Deterministic, with the real lock and no sleeps. B's own contention
    /// notice is the signal that it has opened the marker and is about to block
    /// on it, which is exactly the state the sequence needs: the marker is then
    /// unlinked (what a rollback does) and the holder releases, so B is granted
    /// a lock on an inode that is no longer at the path. Before the
    /// revalidation B returns holding that orphan and the path has no marker at
    /// all; after it, B is holding whatever the path names now.
    ///
    /// B takes its own evidence and reports it rather than parking until the
    /// asserting thread says it may exit: a scoped thread waiting on a channel
    /// whose sender the main thread still holds turns a failed assertion into a
    /// hang, because the scope joins before it propagates the panic. B probes
    /// the path with a second descriptor — the lock is per open file
    /// description, so its own lock excludes it exactly as another process's
    /// would.
    #[cfg(unix)]
    #[test]
    fn a_waiter_granted_an_unlinked_marker_locks_the_marker_at_the_path_instead() {
        let dir = scratch_dir("marker-revalidation");
        let path = dir.join(PUBLISH_LOCK_NAME);
        let first = PublishLock::acquire(&[dir.to_path_buf()], &silent).expect("A holds the lock");

        let (parked, is_parked) = std::sync::mpsc::channel();
        let (report, reported) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let waiter = dir.to_path_buf();
            let probed = path.clone();
            scope.spawn(move || {
                let announce = move |line: &str| {
                    assert!(line.contains("waiting"), "unexpected notice: {line}");
                    let _ = parked.send(());
                };
                let held = PublishLock::acquire(&[waiter], &announce).expect("B is granted a lock");
                let evidence = File::open(&probed)
                    .map(|probe| matches!(probe.try_lock(), Err(TryLockError::WouldBlock)));
                drop(held);
                let _ = report.send(evidence);
            });

            is_parked.recv().expect("B parks on the held marker");
            // What a rolled-back run does to a level it created and left empty.
            fs::remove_file(&path).expect("unlink the marker B is queued on");
            drop(first);

            let evidence = reported.recv().expect("B stops waiting");
            assert!(
                evidence.is_ok(),
                "B holds an unlinked marker: nothing is at {} for it to be excluding",
                path.display()
            );
            assert_eq!(
                evidence.ok(),
                Some(true),
                "the marker at the path must be the one B holds"
            );
        });
    }

    /// The primitive both call sites turn on: identity is the inode, not the
    /// path. A path that names nothing and a path that names a *fresh* file
    /// are both "not the file I am holding" — the second is the one a path
    /// comparison would get wrong, and it is the case the three-run sequence
    /// above is built out of.
    ///
    /// The open descriptor is what makes the recreated marker provably a
    /// different inode: a filesystem that reuses inode numbers cannot reuse one
    /// while a reference to it is still alive.
    #[cfg(unix)]
    #[test]
    fn marker_identity_is_the_inode_not_the_path() {
        let dir = scratch_dir("marker-identity");
        let path = dir.join(PUBLISH_LOCK_NAME);
        fs::write(&path, b"").expect("the marker");
        let held = File::open(&path).expect("hold it open across the replacement");
        assert!(same_file(&held, &path), "an untouched marker is itself");

        fs::remove_file(&path).expect("unlink it");
        assert!(
            !same_file(&held, &path),
            "an unlinked marker is not the file at a path that names nothing"
        );

        fs::write(&path, b"").expect("a fresh marker at the same path");
        assert!(
            !same_file(&held, &path),
            "a fresh inode at the same path is a different file"
        );
    }

    /// Two spellings of one directory take one lock, not two — the second
    /// would otherwise be taken against a different path string and the
    /// self-exclusion below would deadlock.
    #[test]
    fn one_directory_spelled_two_ways_takes_one_lock() {
        let dir = scratch_dir("lock-dedupe");
        let ordered = lock_order(&[dir.to_path_buf(), dir.join("."), dir.to_path_buf()]);
        assert_eq!(ordered.len(), 1, "expected one lock, got {ordered:?}");
    }

    /// Every run orders the shared directories identically, which is what
    /// makes overlapping (not identical) directory sets deadlock-free.
    #[test]
    fn lock_order_is_stable_across_runs() {
        let root = scratch_dir("lock-order");
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&a).expect("a");
        fs::create_dir_all(&b).expect("b");
        assert_eq!(
            lock_order(&[a.clone(), b.clone()]),
            lock_order(&[b, a]),
            "the acquisition order must not depend on the caller's argument order"
        );
    }
}
