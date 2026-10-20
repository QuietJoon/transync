//! The staged fileset commit for the individual CLI outputs:
//! [`write_fileset_atomic`] and the phases it runs — destination vetting,
//! parent creation, per-file staging, the rename pass, and the rollback that
//! removes only what the call created.
//!
//! Lifted out of `output.rs` unchanged by OI-0043; the discipline every
//! function here serves is stated in the parent module's documentation.
//!
//! TRACE: persistence-and-files.md §staged-fileset-commit
//! TRACE: ADR-0006

use super::destination::{
    FileId, STAGING_TEMP_INFIX, duplicate_destination, file_id, refuse_lock_marker_alias,
    staging_name_collision, staging_temp_path, vet_destinations,
};
use super::lock::{self, PUBLISH_LOCK_NAME, PublishLock};
use super::{AFTER_A_FAILED_PUBLICATION, Notify, fsync_dir, note_cleanup_residue};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Commit a set of files as a **staged fileset commit** (R0006-0012).
///
/// Phase 1 stages every payload as `<path>.tmp.<pid>` and fsyncs it;
/// only after every stage succeeds does phase 2 rename each staged file
/// over its target (with a best-effort parent-directory fsync so the
/// rename itself is durable). A failure during phase 1 — the phase that
/// does the real I/O — removes all staged temps and leaves every target
/// untouched. A failure mid-phase-2 can still leave a mixed set, but
/// the exposure shrinks from "seconds of content writing" to a few
/// metadata renames.
///
/// Both phases run under a [`PublishLock`] over every destination directory
/// (R0001-0034): a *concurrent* run cannot interleave its renames with this
/// one, so the "mixed set" exposure above is a crash/IO-error window, not a
/// concurrency one. The lock is taken after the destination parents exist,
/// because the lock marker lives in them. That exclusion holds only while the
/// marker stays the inode locked, so no destination — and no level created on
/// the way to one — may be the marker (R0012-0005): its name is refused up
/// front, and its identity is asked again after the levels exist and once the
/// locks are held.
///
/// Two destinations must not share a staged temp either, by name or by entry,
/// and one must not lie inside another or inside another's staging name.
/// [`vet_destinations`] refuses the spellings it can compare up front, and the
/// filesystem is asked twice more, because a spelling the volume folds onto a
/// name that did not exist yet (a Unicode normalization variant, say) is
/// invisible until that name does. Once the levels exist, before anything is
/// staged or locked, the whole vetting runs again: a level this call created
/// now resolves under every spelling of it, so a destination the volume folds
/// onto it, or onto the staging name it took, is the containment or
/// staging-name refusal it is, and the rollback removes the level
/// (R0012-0002, R0013-0001). Once the temps exist, a temp whose `create_new`
/// meets a temp this call already staged is the duplicate destination it is
/// (R0012-0002), and a final destination that resolves to a temp this call
/// staged is refused before the first rename, through the phase-1 rollback —
/// otherwise that rename would replace the other destination's staged bytes
/// and the run would exit 0 with one output gone and the other wrong
/// (R0013-0001).
///
/// TRACE: persistence-and-files.md §staged-fileset-commit
/// TRACE: contracts.md §6
pub fn write_fileset_atomic(
    files: &[(std::path::PathBuf, &[u8])],
    notify: Notify<'_>,
) -> io::Result<()> {
    vet_destinations(files.iter().map(|(path, _)| path.as_path()))?;

    // Directories this call creates, so a staging (phase-1) failure leaves
    // no directory residue (R0008-0041). Phase-2 (rename) failures keep the
    // documented "mixed set possible" behavior and do not roll dirs back.
    let mut created_dirs: Vec<PathBuf> = Vec::new();
    let dirs = destination_dirs(files);
    // Read before anything is created, because that is when it is true: a
    // destination directory that does not exist yet has no marker to lock, and
    // the deepest level that DOES exist is what a run replacing that directory
    // by name is excluded on (ti `40e2a5`). Where the destination already
    // exists this resolves to the destination itself and nothing changes.
    let mut lock_dirs: Vec<PathBuf> = dirs.iter().map(|dir| claim_anchor(dir)).collect();
    // Every question `vet_destinations` asked is asked again once the levels
    // exist, before locking, because a level this call just created can be
    // what makes a spelling the volume folds resolve. It may have landed on
    // the lock-marker path of the level holding it (R0012-0005), and locking
    // would then open a directory. It may be another destination, or that
    // destination's staging name, under another normalization form — one
    // destination inside the other (R0012-0002, R0013-0001), which the rename
    // pass would otherwise meet as a bare "Is a directory" after publishing
    // part of the set. And the staging names can now be probed in the levels
    // they will actually be created in (R0013-0006).
    if let Err(e) = create_destination_parents(&dirs, &mut created_dirs)
        .and_then(|()| vet_destinations(files.iter().map(|(path, _)| path.as_path())))
    {
        rollback_created_dirs(&created_dirs, notify);
        return Err(e);
    }
    lock_dirs.extend(dirs.iter().cloned());
    let lock = match PublishLock::acquire(&lock_dirs, notify) {
        Ok(lock) => lock,
        Err(e) => {
            rollback_created_dirs(&created_dirs, notify);
            return Err(e);
        }
    };
    // And once more under the lock: taking it created the marker in every
    // destination directory that had none, so a destination that named
    // nothing a moment ago can name the marker now — and the rename pass would
    // replace the inode this lock is held on (R0012-0005).
    if let Err(e) = refuse_marker_aliases(files) {
        drop(lock);
        rollback_created_dirs(&created_dirs, notify);
        return Err(e);
    }

    let mut staged: Vec<(std::path::PathBuf, &std::path::PathBuf)> =
        Vec::with_capacity(files.len());
    // The identity of each staged temp, read from the handle that created it,
    // in `staged`'s order.
    let mut staged_ids: Vec<Option<FileId>> = Vec::with_capacity(files.len());
    for (path, bytes) in files {
        match stage_one_file(path, bytes, notify) {
            Ok((tmp, id)) => {
                staged.push((tmp, path));
                staged_ids.push(id);
            }
            Err(e) => {
                let e = name_a_duplicate(e, path, &staged, &staged_ids);
                cleanup_temps(&staged, notify);
                // Release before removing the tree the markers live in: a run
                // blocked on one of these locks resumes into a directory this
                // call is about to delete otherwise.
                drop(lock);
                rollback_created_dirs(&created_dirs, notify);
                return Err(e);
            }
        }
    }

    // Before the first rename, and still inside phase 1's all-or-nothing:
    // a final destination the filesystem resolves to a temp this call staged
    // would have that rename replace the other destination's bytes
    // (R0013-0001).
    if let Err(e) = refuse_destinations_on_staged_temps(&staged, &staged_ids) {
        cleanup_temps(&staged, notify);
        drop(lock);
        rollback_created_dirs(&created_dirs, notify);
        return Err(e);
    }

    commit_staged_renames(&staged, &created_dirs, notify)
}

/// Turn a staging failure into the duplicate-destination refusal it is, when it
/// is one (R0012-0002): `path`'s temp could not be created because its name
/// already resolves to a temp **this call** staged for another destination —
/// two spellings of one new name, which the volume folds onto one entry and no
/// comparison before the run could see (nothing existed to compare). Any other
/// failure is returned as it came.
fn name_a_duplicate(
    e: io::Error,
    path: &Path,
    staged: &[(PathBuf, &PathBuf)],
    staged_ids: &[Option<FileId>],
) -> io::Error {
    if e.kind() != io::ErrorKind::AlreadyExists {
        return e;
    }
    let occupant = staging_temp_path(path)
        .and_then(fs::symlink_metadata)
        .ok()
        .as_ref()
        .and_then(file_id);
    match staged_ids
        .iter()
        .position(|id| id.is_some() && *id == occupant)
    {
        Some(at) => duplicate_destination(path, staged[at].1),
        None => e,
    }
}

/// Refuse when a final destination names a temp this call staged for a
/// different destination, by filesystem identity (R0013-0001).
///
/// [`vet_destinations`] refuses the reserved staging namespace by the names it
/// can compare before anything exists. This asks the filesystem itself, once
/// the temps do exist, so a spelling the volume folds onto a staging name —
/// Unicode normalization, or a case variant where no probe could answer up
/// front — is refused too, before any rename can act on it. `lstat`, so a
/// destination that is a symlink is judged as the link, which is what the
/// rename would replace. A no-op off Unix, where `std` has no file identity.
fn refuse_destinations_on_staged_temps(
    staged: &[(PathBuf, &PathBuf)],
    staged_ids: &[Option<FileId>],
) -> io::Result<()> {
    for (_, destination) in staged {
        let Some(id) = fs::symlink_metadata(destination)
            .ok()
            .as_ref()
            .and_then(file_id)
        else {
            continue;
        };
        let owner = staged
            .iter()
            .zip(staged_ids)
            .find(|((_, owner), owner_id)| **owner_id == Some(id) && owner != destination);
        if let Some(((_, owner), _)) = owner {
            return Err(staging_name_collision(destination, owner));
        }
    }
    Ok(())
}

/// [`refuse_lock_marker_alias`] over every destination of the set.
fn refuse_marker_aliases(files: &[(PathBuf, &[u8])]) -> io::Result<()> {
    files
        .iter()
        .try_for_each(|(path, _)| refuse_lock_marker_alias(path))
}

/// The directory each destination lands in, one entry per file (duplicates
/// included — [`PublishLock`] deduplicates). A destination with no parent
/// component (`out.md`) lands in the working directory.
fn destination_dirs(files: &[(std::path::PathBuf, &[u8])]) -> Vec<PathBuf> {
    files
        .iter()
        .map(|(path, _)| match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        })
        .collect()
}

/// Create every destination directory before anything is staged or locked,
/// recording **every** level this call had to create so a later failure can
/// roll it back (R0008-0041). Directory creation moved ahead of staging
/// when the publish lock landed: the lock marker lives inside these
/// directories, so they have to exist before the lock can be taken.
///
/// Every level, not just the shallowest one, because the rollback removes
/// empty directories one at a time (R0002-0002) instead of deleting a tree.
fn create_destination_parents(dirs: &[PathBuf], created_dirs: &mut Vec<PathBuf>) -> io::Result<()> {
    for dir in dirs {
        // Record the not-yet-existing levels BEFORE creating them, so cleanup
        // removes exactly what we made.
        created_dirs.extend(missing_ancestors(dir));
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Best-effort removal of the directory levels this call created, deepest
/// record first — and **empty-only** (R0002-0002).
///
/// `fs::remove_dir`, never `remove_dir_all`: the levels were observed missing
/// before the publish lock was taken, so by the time a rollback runs another
/// process — including a transync peer that was blocked on this run's lock and
/// resumed when it was released — may have created files under them. A
/// recursive delete would take that content with it; an empty-only delete
/// degrades to a no-op instead, which is the whole point. The one entry this
/// call is entitled to remove is its own publish-lock marker, and only while
/// it is the directory's sole occupant *and* nobody is locking it — see
/// [`remove_own_lock_marker_if_sole`].
///
/// Two removal outcomes are the design working, not a problem, and stay silent:
/// `NotFound` (somebody got there first) and `DirectoryNotEmpty` (the empty-only
/// degradation above — a peer published here while this run was staging).
/// Anything else means residue this run meant to remove and could not, and the
/// operator hears about it once (R0003-0022) — as a note beside the run's real
/// error, never in place of it.
pub(super) fn rollback_created_dirs(created_dirs: &[PathBuf], notify: Notify<'_>) {
    let mut left_behind: Vec<(PathBuf, io::Error)> = Vec::new();
    for dir in created_dirs.iter().rev() {
        match remove_own_lock_marker_if_sole(dir) {
            LevelDecision::Proceed => {}
            // The level belongs to whoever holds its lock now, marker included.
            // Only this level is skipped: the shallower ones still hold it, so
            // their own `remove_dir` declines with `DirectoryNotEmpty` anyway.
            LevelDecision::PeerMayHoldTheLock => continue,
            LevelDecision::Residue(path, e) => left_behind.push((path, e)),
        }
        if let Err(e) = fs::remove_dir(dir)
            && !matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::DirectoryNotEmpty
            )
        {
            left_behind.push((dir.clone(), e));
        }
    }
    note_cleanup_residue(
        "directory level(s) this run created",
        AFTER_A_FAILED_PUBLICATION,
        &left_behind,
        notify,
    );
}

/// What [`remove_own_lock_marker_if_sole`] decided about one directory level,
/// and therefore what [`rollback_created_dirs`] may do with it next.
enum LevelDecision {
    /// This function raises no objection. It is still the caller's empty-only
    /// `fs::remove_dir` that decides whether the level actually goes.
    Proceed,
    /// A peer holds this directory's publish lock — or the probe could not
    /// prove otherwise. Leave the level alone: marker and directory both.
    PeerMayHoldTheLock,
    /// A removal this call decided to make and could not, for the caller to
    /// report as residue (R0003-0022).
    Residue(PathBuf, io::Error),
}

/// Remove the [`PUBLISH_LOCK_NAME`] marker from a directory this call created,
/// but **only** when it is the directory's single entry *and* no run holds the
/// lock on it.
///
/// The marker is transync's own file and this call is what put it there, so
/// removing it is not destructive — but it is also the file every peer locks,
/// and unlinking it lets a later run create a fresh inode at the same path and
/// "hold" a lock nobody else sees. Two independent facts have to be true before
/// that is safe, and each answers a different question:
///
/// * **Sole occupancy** — nobody has *published* here. With anything else
///   present the directory is not ours to remove anyway (R0002-0002).
/// * **Nobody is locking it** — nobody is *about to* publish here. This call
///   releases its own lock before rolling back (so the marker can be unlinked
///   portably), which is exactly the moment a peer that was blocked behind it
///   acquires the lock; that peer has not written anything yet, so sole
///   occupancy still holds and cannot see it. Removing the level there does not
///   lose bytes — it breaks the exclusion itself, leaving the peer on an
///   unlinked inode while the next run locks a fresh marker at the same path
///   (R0003-0001). [`lock::hold_marker_for_removal`] both answers the question
///   and keeps the answer true across the `remove_file` below.
///
/// Returns [`LevelDecision::Residue`] for a removal this call decided to make
/// and could not, for [`rollback_created_dirs`] to report (R0003-0022).
/// Deciding *not* to remove — an unreadable directory, a peer's file beside the
/// marker — is not a failure; neither is a marker another process already took.
/// Without that distinction the removal's failure would reappear one line later
/// as the `DirectoryNotEmpty` the rollback deliberately keeps quiet about.
fn remove_own_lock_marker_if_sole(dir: &Path) -> LevelDecision {
    let Ok(mut entries) = fs::read_dir(dir) else {
        return LevelDecision::Proceed;
    };
    let Some(Ok(only)) = entries.next() else {
        return LevelDecision::Proceed;
    };
    if entries.next().is_some() {
        return LevelDecision::Proceed;
    }
    if only.file_name().to_str() != Some(PUBLISH_LOCK_NAME) {
        return LevelDecision::Proceed;
    }
    let Some(_hold) = lock::hold_marker_for_removal(dir) else {
        return LevelDecision::PeerMayHoldTheLock;
    };
    match fs::remove_file(only.path()) {
        Ok(()) => LevelDecision::Proceed,
        Err(e) if e.kind() == io::ErrorKind::NotFound => LevelDecision::Proceed,
        Err(e) => LevelDecision::Residue(only.path(), e),
    }
}

/// Stage one payload as `<path>.tmp.<pid>` ([`staging_temp_path`]), fsynced,
/// and return the temp with its filesystem identity (read from the handle
/// that created it, so it is the file this call wrote). The destination
/// directory already exists — [`create_destination_parents`] made it before
/// the publish lock was taken.
fn stage_one_file(
    path: &Path,
    bytes: &[u8],
    notify: Notify<'_>,
) -> io::Result<(std::path::PathBuf, Option<FileId>)> {
    let tmp = staging_temp_path(path)?;
    // `create_new` refuses a pre-existing temp, so a hostile or stale symlink
    // planted at that name cannot redirect the write and truncate an
    // unrelated file (R0008-0006). "Refusing" is that case's word only: any
    // other failure to create the temp is said as what it is, rather than
    // blamed on a temp that never existed (R0013-0006).
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| {
            let doing = if e.kind() == io::ErrorKind::AlreadyExists {
                "refusing existing temp"
            } else {
                "could not create staging temp"
            };
            io::Error::new(e.kind(), format!("{doing} {}: {e}", tmp.display()))
        })?;
    if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
        // R0011-0017: the write's own error is what this returns — the bytes
        // are what failed, and nothing here changes that. But the temp this
        // call created a moment ago and now cannot take back is residue only
        // this call can see, and it is the same residue (and the same
        // sentence) [`cleanup_temps`] owes for the temps of a failed rename
        // pass: silence would make it indistinguishable from a temp that was
        // never there. `NotFound` means somebody got there first and stays
        // quiet, exactly as it does over there.
        if let Err(rm) = fs::remove_file(&tmp)
            && rm.kind() != io::ErrorKind::NotFound
        {
            note_cleanup_residue(
                "staged temp file(s)",
                AFTER_A_FAILED_PUBLICATION,
                &[(tmp, rm)],
                notify,
            );
        }
        return Err(e);
    }
    let id = f.metadata().ok().as_ref().and_then(file_id);
    Ok((tmp, id))
}

/// Phase 2: rename each staged temp over its target, preserving any existing
/// target's permissions (R0008-0042) and fsyncing the parent directory —
/// plus, once the set is committed, the directory levels this call created on
/// the way in (`created_dirs`, R0003-0020).
fn commit_staged_renames(
    staged: &[(std::path::PathBuf, &std::path::PathBuf)],
    created_dirs: &[PathBuf],
    notify: Notify<'_>,
) -> io::Result<()> {
    for (tmp, path) in staged {
        preserve_target_permissions(tmp, path, notify);
        if let Err(e) = fs::rename(tmp, path) {
            cleanup_temps(staged, notify);
            return Err(e);
        }
        // Best-effort fsync of the parent directory so the rename itself is
        // durable.
        if let Some(parent) = path.parent() {
            fsync_dir(
                if parent.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    parent
                },
                notify,
            );
        }
    }
    // Only now, with every destination in place: a failure above returns the
    // rename's error and the levels are about to be somebody else's problem
    // anyway.
    fsync_created_levels(created_dirs, notify);
    Ok(())
}

/// Give the staged temp the permissions of the destination it is about to
/// replace, so an update does not silently reset a file's mode to this
/// process's umask (R0008-0042).
///
/// Nothing here is fatal — the *bytes* are right either way, and the mode is a
/// property of the file this run is replacing rather than of the content it
/// publishes. But a failure is not nothing either: it ships a mode the operator
/// did not choose, and only this call can see that it happened (R0003-0023).
/// A destination that does not exist yet has no mode to carry over and is not a
/// failure.
fn preserve_target_permissions(tmp: &Path, path: &Path, notify: Notify<'_>) {
    #[cfg(unix)]
    if let Ok(meta) = fs::metadata(path)
        && let Err(e) = fs::set_permissions(tmp, meta.permissions())
    {
        notify(&format!(
            "note: could not carry the existing permissions of {} over to its replacement ({e}); \
             the published file keeps this run's default mode instead",
            path.display(),
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = (tmp, path, notify);
    }
}

/// Best-effort removal of staged temps; the original io::Error is what the
/// caller ultimately returns.
///
/// A temp that is already gone is the ordinary case on the phase-2 path (the
/// rename consumed it) and stays silent; anything else is residue the operator
/// hears about once, beside the run's real error (R0003-0024).
fn cleanup_temps(staged: &[(std::path::PathBuf, &std::path::PathBuf)], notify: Notify<'_>) {
    let mut left_behind: Vec<(PathBuf, io::Error)> = Vec::new();
    for (tmp, _) in staged {
        if let Err(e) = fs::remove_file(tmp)
            && e.kind() != io::ErrorKind::NotFound
        {
            left_behind.push((tmp.clone(), e));
        }
    }
    note_cleanup_residue(
        "staged temp file(s)",
        AFTER_A_FAILED_PUBLICATION,
        &left_behind,
        notify,
    );
}

/// Flush the directory levels this call created, so the entries *naming* them
/// are as durable as the files inside them (R0003-0020).
///
/// `create_dir_all` can make a whole chain — `a`, `a/b`, `a/b/c` for one
/// `a/b/c/out.md` — and [`commit_staged_renames`]'s per-destination flush
/// reaches only the deepest of them. The entry naming a level lives in the
/// level *above* it, so that is what gets flushed here: shallowest first, and
/// once per directory however many levels share it. Without this a crash can
/// leave a published file whose contents and whose own directory entry are both
/// durable, under an ancestor directory that never made it to disk.
pub(super) fn fsync_created_levels(created_dirs: &[PathBuf], notify: Notify<'_>) {
    let mut flushed: HashSet<PathBuf> = HashSet::new();
    for holder in levels_holding(created_dirs) {
        if flushed.insert(holder.clone()) {
            fsync_dir(&holder, notify);
        }
    }
}

/// The directory that holds each created level's own entry, in the order the
/// levels were recorded (shallowest first within each chain). Split out from
/// [`fsync_created_levels`] so the selection is testable without a filesystem
/// whose `fsync` can be made to fail.
fn levels_holding(created_dirs: &[PathBuf]) -> Vec<PathBuf> {
    created_dirs
        .iter()
        .map(|level| match level.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
            _ => PathBuf::from("."),
        })
        .collect()
}

/// The deepest ancestor of `dir` (inclusive) that **exists right now** — the
/// directory a publication claims on `dir`'s behalf when `dir` itself cannot
/// be claimed yet (ti `40e2a5`).
///
/// A publish lock lives on a marker file *inside* the directory it protects, so
/// a destination that does not exist yet cannot be locked at all: the run
/// creates it and locks it, and a peer replacing that same directory by name
/// (`--out-dir <dir>`, which locks the level *holding* its target) never meets
/// it. Locking the deepest level that does exist closes that: the peer holds
/// the level it is publishing its siblings into, which for a fresh target is
/// exactly this one.
///
/// Two properties make this safe to add unconditionally. When `dir` already
/// exists the answer is `dir`, so nothing about an ordinary publication
/// changes; and when it does not, the level returned is one this run is about
/// to create *into*, so it is writable whenever the run could have proceeded at
/// all — no publication starts needing permission it did not need before.
pub(super) fn claim_anchor(dir: &Path) -> PathBuf {
    for level in dir.ancestors() {
        if level.as_os_str().is_empty() {
            break;
        }
        if level.exists() {
            return level.to_path_buf();
        }
    }
    // A relative path whose every component is missing: the working directory
    // is what holds the shallowest of them.
    PathBuf::from(".")
}

/// Every ancestor of `dir` (inclusive) that does not yet exist, **shallowest
/// first** — exactly the directory levels this writer is about to create for
/// `dir` (R0008-0041). Empty when `dir` already exists.
pub(super) fn missing_ancestors(dir: &Path) -> Vec<PathBuf> {
    let mut missing = Vec::new();
    for anc in dir.ancestors() {
        if anc.as_os_str().is_empty() || anc.exists() {
            break;
        }
        missing.push(anc.to_path_buf());
    }
    missing.reverse();
    missing
}

/// If `rest` is `Some(".tmp.<digits>")`, return the `<digits>` pid slice;
/// otherwise `None`. Used to recognize this writer's own staging temps.
pub(super) fn tmp_pid_suffix(rest: Option<&str>) -> Option<&str> {
    rest?
        .strip_prefix(STAGING_TEMP_INFIX)
        .filter(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::preflight::HTML_BUNDLE_ENTRIES;
    use crate::output::testing::{NFC_E, NFD_E, folds_normalization, scratch_dir};
    use crate::output::tests::{Step, silent};
    use std::cell::RefCell;
    use std::fs::File;
    use std::process;
    use std::sync::Arc;
    use std::sync::Barrier;

    /// A destination in `dir` whose staging **fails**, and only once the
    /// publish locks are held: its `<dest>.tmp.<pid>` name is already occupied,
    /// which [`stage_one_file`]'s `create_new` refuses (R0008-0006). Returns
    /// the destination to hand to [`write_fileset_atomic`].
    ///
    /// The lock-ordering tests used a destination with no file name for this,
    /// which stopped reaching the lock phase when R0004-0066 moved that
    /// refusal into the up-front destination vetting — a preflight that fires
    /// before anything is created or locked, which is the whole point of it.
    fn occupied_staging_name(dir: &Path) -> PathBuf {
        fs::write(
            dir.join(format!("blocked.md.tmp.{}", process::id())),
            b"somebody is already staging this name",
        )
        .expect("occupy the staging name");
        dir.join("blocked.md")
    }

    /// The six bundle names plus the two individual outputs, all filled with
    /// one marker byte so a mixed publication is visible as a mixed set.
    fn fileset(dir: &Path, marker: u8) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = vec![
            (dir.join("out.md"), vec![marker; 64]),
            (dir.join("alignment.json"), vec![marker; 64]),
        ];
        for name in HTML_BUNDLE_ENTRIES {
            files.push((dir.join("html").join(name), vec![marker; 64]));
        }
        files
    }

    /// R0001-0034: two writers aimed at one fileset publish one run or the
    /// other — never a bundle mixing both. The barrier makes them contend on
    /// purpose (no sleeps, no wall-clock assumptions); with the publish lock
    /// removed this same test fails on the colliding `<name>.tmp.<pid>`
    /// staging names, which is the in-process face of the same defect.
    #[test]
    fn two_concurrent_writers_publish_one_run_not_a_mixture() {
        let dir = scratch_dir("concurrent-fileset");
        let gate = Arc::new(Barrier::new(2));

        std::thread::scope(|scope| {
            for marker in *b"AB" {
                let dir = dir.to_path_buf();
                let gate = Arc::clone(&gate);
                scope.spawn(move || {
                    let owned = fileset(&dir, marker);
                    let files: Vec<(PathBuf, &[u8])> = owned
                        .iter()
                        .map(|(p, b)| (p.clone(), b.as_slice()))
                        .collect();
                    gate.wait();
                    write_fileset_atomic(&files, &silent).expect("publication should succeed");
                });
            }
        });

        let published: Vec<u8> = fileset(&dir, b'?')
            .iter()
            .map(|(path, _)| {
                let bytes = fs::read(path).expect("every destination is published");
                assert_eq!(bytes.len(), 64, "{} is a whole payload", path.display());
                bytes[0]
            })
            .collect();
        assert!(
            published.iter().all(|m| *m == published[0]),
            "the published set mixes two runs: {:?}",
            published.iter().map(|m| *m as char).collect::<Vec<_>>()
        );
    }

    /// R0003-0020, the fileset half: `create_dir_all` can make a whole chain
    /// for one destination, and the commit used to flush only the destination's
    /// own parent. The entry naming each level lives in the level above it, so
    /// that is the set — including `.` for the shallowest, which no
    /// per-destination flush ever reaches.
    #[test]
    fn every_created_level_is_flushed_not_only_the_deepest() {
        let created = [
            PathBuf::from("a"),
            PathBuf::from("a/b"),
            PathBuf::from("a/b/c"),
        ];
        assert_eq!(
            levels_holding(&created),
            vec![PathBuf::from("."), PathBuf::from("a"), PathBuf::from("a/b")],
            "each created level's entry lives in the level above it"
        );
    }

    /// And the flush itself runs over that set, once per directory however many
    /// levels share one holder. The vehicle is R0003-0021's newly audible open
    /// failure: a holder that does not exist announces itself, so the notices
    /// are a readout of which directories were actually visited.
    #[cfg(unix)]
    #[test]
    fn the_created_levels_are_flushed_once_each() {
        let root = scratch_dir("created-levels");
        let missing = root.join("gone");
        let created = [missing.join("a"), missing.join("b"), root.join("here")];

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        fsync_created_levels(&created, &sink);
        let notices = notices.into_inner();

        assert_eq!(
            notices.len(),
            1,
            "two levels share one holder, and `root` flushes cleanly: {notices:?}"
        );
        assert!(
            notices[0].contains(&missing.display().to_string()),
            "the visited holder is the one that could not be flushed: {}",
            notices[0]
        );
    }

    /// R0003-0024: a staged temp the cleanup could not remove is residue only
    /// this call knows about. A temp the rename already consumed is the
    /// ordinary case and stays quiet, so the note means what it says.
    #[cfg(unix)]
    #[test]
    fn a_staged_temp_that_survives_cleanup_is_reported_once() {
        let root = scratch_dir("cleanup-temps");
        // `remove_file` on a directory fails on every Unix (EPERM/EISDIR),
        // which is the portable stand-in for a temp that will not go away.
        let stuck = root.join("out.md.tmp.1");
        fs::create_dir(&stuck).expect("the immovable temp");
        let gone = root.join("already-renamed.tmp.1");
        let dest = root.join("out.md");
        let staged: Vec<(PathBuf, &PathBuf)> = vec![(stuck.clone(), &dest), (gone, &dest)];

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        cleanup_temps(&staged, &sink);
        let notices = notices.into_inner();

        assert_eq!(
            notices.len(),
            1,
            "one bounded note, whatever the count: {notices:?}"
        );
        assert!(
            notices[0].contains(&stuck.display().to_string())
                && notices[0].contains("staged temp file(s)"),
            "the note must name what was left behind: {}",
            notices[0]
        );

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        cleanup_temps(&[(root.join("nothing.tmp.1"), &dest)], &sink);
        assert!(
            notices.into_inner().is_empty(),
            "an already-consumed temp is not residue"
        );
    }

    /// R0003-0022: the rollback's *expected* refusals stay silent — that
    /// empty-only degradation is the whole point of R0002-0002 — but a level
    /// this run meant to remove and could not is residue the operator hears
    /// about, beside the run's real error rather than instead of it.
    #[test]
    fn a_rollback_that_leaves_residue_says_so_but_not_for_the_expected_refusals() {
        let root = scratch_dir("rollback-notes");
        let peer_held = root.join("peer-held");
        fs::create_dir(&peer_held).expect("a level a peer published into");
        fs::write(peer_held.join("peer.md"), b"peer").expect("the peer's output");
        let not_a_dir = root.join("regular-file");
        fs::write(&not_a_dir, b"x").expect("a level that is not a directory");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        rollback_created_dirs(&[peer_held, root.join("never-existed")], &sink);
        assert!(
            notices.into_inner().is_empty(),
            "a non-empty level and a missing one are the design working"
        );

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        rollback_created_dirs(std::slice::from_ref(&not_a_dir), &sink);
        let notices = notices.into_inner();
        assert_eq!(notices.len(), 1, "one bounded note: {notices:?}");
        assert!(
            notices[0].contains(&not_a_dir.display().to_string()),
            "the note must name the residue: {}",
            notices[0]
        );
    }

    /// R0003-0023: preserving the replaced file's mode (R0008-0042) is
    /// best-effort, but a failure ships a mode the operator did not choose and
    /// nothing above this call can see it happen. A destination that does not
    /// exist yet has no mode to carry and is not a failure.
    #[cfg(unix)]
    #[test]
    fn a_mode_that_cannot_be_carried_over_is_reported() {
        let root = scratch_dir("preserve-mode");
        let dest = root.join("out.md");
        fs::write(&dest, b"previous").expect("the file being replaced");
        // The temp is gone, so `set_permissions` fails where the metadata read
        // succeeded — the shape of any real failure here.
        let missing_tmp = root.join("out.md.tmp.1");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        preserve_target_permissions(&missing_tmp, &dest, &sink);
        let notices = notices.into_inner();

        assert_eq!(notices.len(), 1, "exactly one note: {notices:?}");
        assert!(
            notices[0].contains(&dest.display().to_string()) && notices[0].contains("default mode"),
            "the note must name the destination and what it now carries: {}",
            notices[0]
        );

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        preserve_target_permissions(&missing_tmp, &root.join("fresh.md"), &sink);
        assert!(
            notices.into_inner().is_empty(),
            "a destination with no predecessor has no mode to carry"
        );
    }

    /// R0002-0002, the race with no attacker in it: run A creates a shared
    /// directory and records it, run B publishes into that directory while A
    /// is still working, A then fails and rolls back. A's rollback must not
    /// take B's committed output with it.
    ///
    /// Deterministic, with the real lock and no sleeps. A is aimed at two
    /// directories: `held` (which the test pre-created and holds the lock on,
    /// sorting first in `lock_order`) and `made` (which A creates). A therefore
    /// creates `made`, records it, and parks on `held`'s lock — announcing that
    /// through its own `notify`, which is the signal the test waits for. B then
    /// publishes into `made` unobstructed, the test releases `held`, and A
    /// proceeds to fail staging (its third destination's staging name is
    /// already taken) and roll back. Before the empty-only rollback this test
    /// loses `peer.md`.
    #[test]
    fn a_rollback_cannot_delete_a_peers_published_output() {
        let root = scratch_dir("rollback-peer");
        let held = root.join("held");
        let made = root.join("made");
        fs::create_dir(&held).expect("the pre-existing directory");
        let blocked = occupied_staging_name(&held);
        let held_lock =
            PublishLock::acquire(std::slice::from_ref(&held), &silent).expect("hold `held`");

        let (parked, is_parked) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                let announce = move |line: &str| {
                    // The one notice A emits is "waiting for another transync
                    // run"; by then it has created `made` and is about to block.
                    assert!(line.contains("waiting"), "unexpected notice: {line}");
                    let _ = parked.send(());
                };
                let files: Vec<(PathBuf, &[u8])> = vec![
                    (made.join("out.md"), b"A".as_slice()),
                    (held.join("out.md"), b"A".as_slice()),
                    (blocked.clone(), b"A".as_slice()),
                ];
                write_fileset_atomic(&files, &announce)
            });

            is_parked.recv().expect("A parks on the held lock");
            let peer = made.join("peer.md");
            let peer_files: Vec<(PathBuf, &[u8])> = vec![(peer.clone(), b"peer".as_slice())];
            write_fileset_atomic(&peer_files, &silent).expect("B publishes into the shared dir");

            drop(held_lock);
            assert!(a.join().expect("A joins").is_err(), "A must fail staging");

            assert_eq!(
                fs::read(&peer).ok(),
                Some(b"peer".to_vec()),
                "the rollback deleted a peer's published output"
            );
        });
    }

    /// R0003-0001, the variant sole occupancy cannot see: a level this run
    /// created is still EMPTY — its only entry is the publish-lock marker — but
    /// a peer already holds the lock on that marker, because it was blocked
    /// behind this run and resumed the moment this run released. Removing the
    /// level there loses no bytes; it breaks the exclusion. The peer keeps
    /// locking an unlinked inode while the next run creates a fresh marker at
    /// the same path and locks that, so two runs are simultaneously "the
    /// exclusive publisher" of one directory and interleave their renames.
    ///
    /// Deterministic, with the real lock and no sleeps, on the same harness as
    /// `a_rollback_cannot_delete_a_peers_published_output`. The lever is that
    /// [`create_destination_parents`] records **every** level it creates while
    /// [`PublishLock`] only locks the *destination* directories: A is aimed at
    /// `made/deep`, so it creates and records `made` as well and never locks
    /// it. A parks on `held` (pre-created, held by the test, sorting first in
    /// `lock_order`) with `made` already on disk and still empty, which is when
    /// the peer takes `made`'s lock. A then fails staging and rolls back
    /// `made/deep` (its own, empty, unlocked — that one goes) and reaches
    /// `made`, whose sole entry is the marker the peer is holding.
    #[test]
    fn a_rollback_leaves_a_level_whose_lock_a_peer_has_taken() {
        let root = scratch_dir("rollback-locked");
        let held = root.join("held");
        let made = root.join("made");
        let deep = made.join("deep");
        fs::create_dir(&held).expect("the pre-existing directory");
        let blocked = occupied_staging_name(&held);
        let held_lock =
            PublishLock::acquire(std::slice::from_ref(&held), &silent).expect("hold `held`");

        let (parked, is_parked) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                let announce = move |line: &str| {
                    assert!(line.contains("waiting"), "unexpected notice: {line}");
                    let _ = parked.send(());
                };
                let files: Vec<(PathBuf, &[u8])> = vec![
                    (deep.join("out.md"), b"A".as_slice()),
                    (held.join("out.md"), b"A".as_slice()),
                    (blocked.clone(), b"A".as_slice()),
                ];
                write_fileset_atomic(&files, &announce)
            });

            is_parked.recv().expect("A parks on the held lock");
            // The peer: it publishes nothing, it only takes the lock — which is
            // precisely the state an empty-only rollback cannot distinguish
            // from "nobody was ever here".
            let peer_lock = PublishLock::acquire(std::slice::from_ref(&made), &silent)
                .expect("peer locks made");

            drop(held_lock);
            assert!(a.join().expect("A joins").is_err(), "A must fail staging");

            assert!(
                !deep.exists(),
                "the level A created, locked and left empty is still cleaned up"
            );
            let marker = made.join(PUBLISH_LOCK_NAME);
            assert!(
                marker.exists(),
                "the rollback unlinked a marker a peer was holding the lock on"
            );
            let probe = File::open(&marker).expect("the marker the peer holds");
            assert!(
                matches!(probe.try_lock(), Err(std::fs::TryLockError::WouldBlock)),
                "the peer must still be the only run that can publish into this directory"
            );
            drop(probe);
            drop(peer_lock);
        });
    }

    /// The other half of the same rule: a directory the run created and left
    /// EMPTY (only its own lock marker in it) is still cleaned up, so
    /// R0008-0041's "no directory residue" survives the empty-only rollback.
    #[test]
    fn a_rollback_still_removes_the_levels_it_left_empty() {
        let root = scratch_dir("rollback-empty");
        let deep = root.join("made").join("here");
        fs::create_dir_all(&deep).expect("the created levels");
        fs::write(deep.join(PUBLISH_LOCK_NAME), b"").expect("the marker this run left");

        rollback_created_dirs(&[root.join("made"), deep], &silent);

        assert!(
            !root.join("made").exists(),
            "an empty created level (marker aside) must be removed"
        );
    }

    /// The same boundary from the other side, and the case an inode-keyed lock
    /// cannot reach: the target does not exist yet, so it has no marker to
    /// lock. The files-mode run has to claim the level that *holds* it — the
    /// one the `--out-dir` publish keeps its staging and backup siblings in —
    /// or it creates the target and publishes into it while the replace renames
    /// its own tree over the top.
    #[test]
    fn a_files_mode_publish_into_a_fresh_directory_waits_for_the_run_that_holds_its_parent() {
        let root = scratch_dir("nested-fresh-target");
        let target = root.join("published");
        assert!(!target.exists(), "the target must be the fresh case");

        // What `publish_out_dir` holds for a target inside `root`.
        let peer = PublishLock::acquire(&[root.to_path_buf()], &silent)
            .expect("the --out-dir publisher's lock");

        let (step, steps) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let done = step.clone();
            let destination = target.join("out.md");
            scope.spawn(move || {
                let announce = move |line: &str| {
                    if line.contains("waiting") {
                        let _ = step.send(Step::Parked);
                    }
                };
                let payload: Vec<(PathBuf, &[u8])> = vec![(destination, b"files-mode".as_slice())];
                let outcome = write_fileset_atomic(&payload, &announce).map_err(|e| e.to_string());
                let _ = done.send(Step::Done(outcome));
            });

            match steps.recv().expect("the fileset commit reports something") {
                Step::Parked => {}
                Step::Done(_) => panic!(
                    "a publish into a directory that does not exist yet locked nothing a peer \
                     replacing that directory holds"
                ),
            }
            drop(peer);
            match steps.recv().expect("it finishes once the peer releases") {
                Step::Done(Ok(())) => {}
                Step::Done(Err(e)) => panic!("the queued publication must still succeed: {e}"),
                Step::Parked => panic!("the publication parked twice on one lock"),
            }
        });
        assert_eq!(
            fs::read(target.join("out.md")).ok(),
            Some(b"files-mode".to_vec()),
            "the queued publication must still commit its output"
        );
    }

    /// The claim a publication makes on behalf of a destination it has to
    /// create is the deepest level that exists — and for a destination that
    /// already exists it is that destination, so an ordinary publication locks
    /// exactly what it locked before.
    #[test]
    fn the_claim_anchor_is_the_deepest_level_that_exists() {
        let root = scratch_dir("claim-anchor");
        let nested = root.join("a").join("b");
        assert_eq!(claim_anchor(&root), root.to_path_buf());
        assert_eq!(claim_anchor(&nested), root.to_path_buf());
        fs::create_dir_all(&nested).expect("create the chain");
        assert_eq!(claim_anchor(&nested), nested);
        assert_eq!(
            claim_anchor(Path::new("no/such/relative/path")),
            PathBuf::from("."),
            "a relative path with nothing on it is held by the working directory"
        );
    }

    /// A failed staging pass still leaves no directory residue (R0008-0041) —
    /// including the lock marker the publish now creates inside the
    /// directories it made. Directory creation moved ahead of staging when the
    /// lock landed, so this is the regression guard for that move.
    #[test]
    fn a_failed_stage_removes_the_directories_it_created() {
        let root = scratch_dir("stage-rollback");
        let fresh = root.join("made").join("here");
        let files: Vec<(PathBuf, &[u8])> = vec![
            (fresh.join("out.md"), b"payload".as_slice()),
            // A path ending in `..` has no file name, so staging it fails
            // after the first payload is already staged and the directories
            // (and their lock markers) exist.
            (fresh.join(".."), b"payload".as_slice()),
        ];
        assert!(
            write_fileset_atomic(&files, &silent).is_err(),
            "a destination with no file name must fail staging"
        );
        assert!(
            !root.join("made").exists(),
            "the created directory levels must be rolled back"
        );
    }

    /// Whether `dir` holds nothing a refused publication may leave: no output
    /// and no temp. The lock marker stays — it is created once in a directory
    /// that already existed and never removed, by design (DCR-0021).
    fn holds_nothing(dir: &Path) -> bool {
        fs::read_dir(dir)
            .expect("list the directory")
            .flatten()
            .all(|entry| entry.file_name() == PUBLISH_LOCK_NAME)
    }

    /// R0013-0001: a final destination that is another destination's
    /// `<name>.tmp.<pid>` staging name. `create_new` cannot see it — both temps
    /// are absent when opened — and in this order the first rename replaced the
    /// second's temp, which the second rename then published as `out.json`:
    /// exit 0, the first output gone and the second holding the wrong bytes.
    /// The reverse order happened to publish correctly; it is refused too,
    /// because the reserved namespace is a property of the set, not of the
    /// order its members were listed in.
    #[test]
    fn a_destination_that_is_another_destinations_staging_name_is_refused() {
        let root = scratch_dir("staging-name-collision");
        let pid = process::id();
        let staged = root.join(format!("out.json.tmp.{pid}"));
        let final_ = root.join("out.json");

        for (first, second) in [(&staged, &final_), (&final_, &staged)] {
            let err = write_fileset_atomic(
                &[
                    (first.clone(), b"first".as_slice()),
                    (second.clone(), b"second".as_slice()),
                ],
                &silent,
            )
            .expect_err("a destination in another's staging namespace must not publish");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            let msg = err.to_string();
            assert!(
                msg.contains(&staged.display().to_string())
                    && msg.contains(&final_.display().to_string()),
                "the refusal must name both destinations: {msg}"
            );
            assert!(
                holds_nothing(&root),
                "nothing is published and no temp is left: {:?}",
                fs::read_dir(&root)
                    .map(|it| it.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            );
        }
    }

    /// The same collision under a spelling no name comparison catches: the two
    /// Unicode normalization forms of one name are one entry on HFS+ and APFS,
    /// and neither exists before the run, so nothing up front can tell. What
    /// can is the filesystem once the temps exist: before the first rename,
    /// a final destination that resolves to a temp this call staged
    /// (`dev`+`ino`) is refused and the phase-1 rollback runs.
    #[test]
    fn a_normalization_variant_of_a_staging_name_is_refused_before_any_rename() {
        let root = scratch_dir("staging-name-normalization");
        if !folds_normalization(&root) {
            eprintln!(
                "note: {} does not fold Unicode normalization; the post-staging identity half of \
                 R0013-0001 did not run on this filesystem",
                root.display()
            );
            return;
        }
        let pid = process::id();
        let colliding = root.join(format!("caf{NFD_E}.json.tmp.{pid}"));
        let owner = root.join(format!("caf{NFC_E}.json"));

        for (first, second) in [(&colliding, &owner), (&owner, &colliding)] {
            let err = write_fileset_atomic(
                &[
                    (first.clone(), b"first".as_slice()),
                    (second.clone(), b"second".as_slice()),
                ],
                &silent,
            )
            .expect_err("a destination the filesystem resolves to a staged temp must not publish");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            assert!(err.to_string().contains("staging name"), "{err}");
            assert!(
                holds_nothing(&root),
                "the rollback leaves nothing behind: {:?}",
                fs::read_dir(&root)
                    .map(|it| it.flatten().map(|e| e.file_name()).collect::<Vec<_>>())
            );
        }
    }

    /// R0012-0002's late half: two new names in the two normalization forms are
    /// one file, and no read-only question asked before the run can say so —
    /// neither exists. The second temp's `create_new` is what finds out, and it
    /// used to report "refusing existing temp" about a temp this very call had
    /// staged. It is the duplicate it is.
    #[test]
    fn two_new_normalization_forms_of_one_name_are_refused_as_one_destination() {
        let root = scratch_dir("normalization-duplicate");
        if !folds_normalization(&root) {
            eprintln!(
                "note: {} does not fold Unicode normalization; the late duplicate half of \
                 R0012-0002 did not run on this filesystem",
                root.display()
            );
            return;
        }
        let nfc = root.join(format!("caf{NFC_E}.json"));
        let nfd = root.join(format!("caf{NFD_E}.json"));
        let err = write_fileset_atomic(
            &[
                (nfc.clone(), b"first".as_slice()),
                (nfd.clone(), b"second".as_slice()),
            ],
            &silent,
        )
        .expect_err("two spellings of one new name are one destination");
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
        let msg = err.to_string();
        assert!(
            msg.contains("duplicate output destination")
                && msg.contains(&nfc.display().to_string())
                && msg.contains(&nfd.display().to_string()),
            "the refusal must say the two are one and name both: {msg}"
        );
        assert!(holds_nothing(&root), "a refused pair publishes nothing");
    }

    /// The names a refused publication left in `dir`, for a failure message.
    fn listing(dir: &Path) -> Vec<std::ffi::OsString> {
        fs::read_dir(dir)
            .map(|it| it.flatten().map(|e| e.file_name()).collect())
            .unwrap_or_default()
    }

    /// R0012-0002's containment half, for new names the volume folds by Unicode
    /// normalization: `caf<NFD é>/y.md` beside `caf<NFC é>` is one destination
    /// inside another, and nothing before the run can say so — neither name
    /// exists, and the two spellings differ in more than ASCII letter case.
    /// Creating the inner destination's parent is what makes the outer one
    /// resolve to that directory. Unasked, the staging passed (the two temps
    /// have different names), the rename pass published the inner file and then
    /// failed renaming the outer one onto the new directory with a bare "Is a
    /// directory", leaving a mixed set and the directory behind. Asked again
    /// once the levels exist, it is the containment refusal it is, before
    /// anything is staged or locked, and the rollback removes the level.
    #[test]
    fn a_created_level_the_volume_folds_onto_another_destination_is_refused_before_staging() {
        let root = scratch_dir("normalization-containment");
        if !folds_normalization(&root) {
            eprintln!(
                "note: {} does not fold Unicode normalization; the created-level half of \
                 R0012-0002's containment refusal did not run on this filesystem",
                root.display()
            );
            return;
        }
        for (inner_e, outer_e) in [(NFD_E, NFC_E), (NFC_E, NFD_E)] {
            let inner = root.join(format!("caf{inner_e}")).join("y.md");
            let deeper = root.join(format!("caf{inner_e}")).join("a").join("y.md");
            let outer = root.join(format!("caf{outer_e}"));
            let outer_deeper = outer.join("a");
            for (first, second) in [
                (&inner, &outer),
                (&outer, &inner),
                (&deeper, &outer_deeper),
                (&outer_deeper, &deeper),
            ] {
                let err = write_fileset_atomic(
                    &[
                        (first.clone(), b"first".as_slice()),
                        (second.clone(), b"second".as_slice()),
                    ],
                    &silent,
                )
                .expect_err("one destination inside another is never published");
                assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
                assert!(
                    err.to_string().contains("is inside output destination"),
                    "the containment refusal, not a failed rename: {err}"
                );
                assert!(
                    listing(&root).is_empty(),
                    "nothing published, no temp, no created level, no lock marker: {:?}",
                    listing(&root)
                );
            }
        }
    }

    /// R0013-0001 through a level the run creates: `caf<NFD é>.json.tmp.<pid>/y.md`
    /// beside `caf<NFC é>.json` puts a destination inside the other's staging
    /// name, under a spelling no comparison before the run can see. Creating
    /// the inner destination's parent makes that staging name a directory, and
    /// the outer destination's `create_new` then met it as "refusing existing
    /// temp" — after the paid run, naming a temp that was never one. Asked
    /// again once the levels exist, it is refused with the staging-name
    /// sentence and the level is rolled back.
    #[test]
    fn a_created_level_the_volume_folds_onto_a_staging_name_is_refused_before_staging() {
        let root = scratch_dir("normalization-staging-level");
        if !folds_normalization(&root) {
            eprintln!(
                "note: {} does not fold Unicode normalization; the created-level half of \
                 R0013-0001 did not run on this filesystem",
                root.display()
            );
            return;
        }
        let pid = process::id();
        let inner = root.join(format!("caf{NFD_E}.json.tmp.{pid}")).join("y.md");
        let owner = root.join(format!("caf{NFC_E}.json"));
        for (first, second) in [(&inner, &owner), (&owner, &inner)] {
            let err = write_fileset_atomic(
                &[
                    (first.clone(), b"first".as_slice()),
                    (second.clone(), b"second".as_slice()),
                ],
                &silent,
            )
            .expect_err("a destination inside another's staging name is never published");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            assert!(err.to_string().contains("staging name"), "{err}");
            assert!(
                listing(&root).is_empty(),
                "nothing published, no temp, no created level, no lock marker: {:?}",
                listing(&root)
            );
        }
    }

    /// R0013-0006's late backstop: "refusing existing temp" is the sentence for
    /// a staging name that is already taken, and it was said about every
    /// failure to create one — a missing directory, a name the filesystem
    /// refuses — naming a pre-existing temp that never existed.
    #[test]
    fn a_staging_failure_that_is_not_an_occupied_name_says_what_it_was() {
        let root = scratch_dir("staging-open-failure");
        let err = stage_one_file(&root.join("missing").join("out.md"), b"x", &silent)
            .expect_err("there is no directory to stage into");
        assert_eq!(err.kind(), io::ErrorKind::NotFound, "{err}");
        let msg = err.to_string();
        assert!(
            !msg.contains("refusing existing temp") && msg.contains("out.md.tmp."),
            "the sentence must name the temp without claiming it already existed: {msg}"
        );
    }
}
