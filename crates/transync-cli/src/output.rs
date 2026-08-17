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
//! destination it has to create ([`claim_anchor`]), and a directory publish
//! claims the directories inside its target ([`published_dirs_inside`]). That
//! pair is what closes DCR-0021's nested boundary — `--out-dir X` racing a
//! files-mode publish into `X` (ti `40e2a5`).
//!
//! No pair of claims closes it at *every* depth: a replace claims a fixed,
//! shallow set while a publication claims a point that can be arbitrarily deep
//! (ti `cbbc4e`). What bounds the gap is not a third claim but the two orderings
//! around it. A publication reads its anchor **before** it creates anything, so
//! the level it claims is one that existed when it started — for a destination
//! under an `--out-dir` target, that is the target or a level above it, both of
//! which the replace holds. And [`ensure_out_dir_replaceable`] runs again with
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
//! TRACE: persistence-and-files.md §staged-fileset-commit
//! TRACE: ADR-0006
//! TRACE: DCR-0021

mod lock;

use lock::{PUBLISH_LOCK_NAME, PublishLock};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process;

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

/// The embedded JS sync engine source. Built into the CLI binary at compile
/// time so `transync translate --html-out <dir>` can write a self-contained
/// demo bundle without consulting the workspace.
///
/// TRACE: SCN-13
const SYNC_JS: &str = include_str!("../web/sync.js");

/// The embedded `index.html` template body.
///
/// TRACE: ADR-0006
const INDEX_TPL: &str = include_str!("../web/index.html.tpl");

/// The embedded DOMPurify build (vendored; mirror of
/// `web/vendor/purify.min.js` — byte-equality enforced by
/// `tests/sync_js_drift.rs`). The demo shell sanitizes fetched
/// fragments before mounting and fails closed without it. OI-0001.
const PURIFY_JS: &str = include_str!("../web/purify.min.js");

/// What `--strict-csp` puts in the bundle shell's `<head>`: a whole
/// `Content-Security-Policy` `<meta>` element, preceded by the newline and
/// indent that place it directly under the charset declaration. The flag is
/// off by default, the alternative substitution is the empty string, and the
/// slot sits at the END of the charset line — so a run without the flag
/// produces a byte-identical `index.html` to a pre-flag one (OI-0018, posture
/// amended 2026-08-07).
///
/// Every directive is derived from what this bundle actually loads:
///
/// * `default-src 'self'` — the fallback for everything not named below
///   (fonts, media, frames, workers): same origin or nothing. `'self'` is
///   origin-scoped, not directory-scoped — it stops remote hosts, not sibling
///   paths under whatever document root the bundle is served from.
/// * `img-src 'self' data:` — the point of the flag. Rendered content carries
///   whatever image URLs the untrusted source Markdown named, and comrak's
///   link sanitizer passes `data:image/*` through, so inline images keep
///   rendering while a remote host never learns that a viewer opened the
///   document (the tracking-pixel exposure).
/// * `script-src 'self' 'unsafe-inline'` — `purify.min.js` and `sync.js` are
///   bundle files, and `index.html`'s module script is inline. No nonce or
///   hash is emitted, so `'unsafe-inline'` is honored rather than ignored.
/// * `style-src 'self' 'unsafe-inline'` — the entire theme sheet is the inline
///   `<style>` block, and sanitized content may carry `style` attributes.
/// * `connect-src 'self'` — the shell fetches `source.html`, `target.html` and
///   `alignment.json` from beside itself, and nothing else.
/// * `object-src 'none'` / `base-uri 'none'` — the bundle has no plugin
///   content and no `<base>`; both would otherwise be reachable through
///   rendered content.
///
/// `frame-ancestors`, `sandbox` and `report-uri` are deliberately absent: a
/// `<meta>`-delivered policy ignores them, so naming them would only add a
/// console warning. The bundle is meant to be *served* (its fetches already
/// fail under `file://`), which is also where `'self'` has a meaning.
///
/// TRACE: OI-0018
const CSP_META: &str = concat!(
    "\n    <meta http-equiv=\"Content-Security-Policy\" content=\"",
    "default-src 'self'; ",
    "img-src 'self' data:; ",
    "script-src 'self' 'unsafe-inline'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "connect-src 'self'; ",
    "object-src 'none'; ",
    "base-uri 'none'",
    "\">",
);

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
/// because the lock marker lives in them.
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
    if let Err(e) = create_destination_parents(&dirs, &mut created_dirs) {
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

    let mut staged: Vec<(std::path::PathBuf, &std::path::PathBuf)> =
        Vec::with_capacity(files.len());
    for (path, bytes) in files {
        match stage_one_file(path, bytes) {
            Ok(tmp) => staged.push((tmp, path)),
            Err(e) => {
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

    commit_staged_renames(&staged, &created_dirs, notify)
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
fn rollback_created_dirs(created_dirs: &[PathBuf], notify: Notify<'_>) {
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

/// Every question about a destination **set** that the arguments and the
/// filesystem answer on their own, asked before any I/O (R0008-0005) and — for
/// a CLI run — before the provider is called (R0002-0029).
///
/// Three refusals, all of them cheap and all of them true before the run
/// started:
///
/// * **A name this writer cannot spell** ([`destination_file_name`]). The
///   staging names are built from the final path component, so a component
///   that is not UTF-8 fails at staging time; asking here is what keeps that
///   from being a discovery made after the provider has been paid
///   (R0004-0066).
/// * **Two destinations that are one destination** (R0008-0005). They would
///   stage to the same temp name and the rename pass would leave a partial or
///   wrong fileset. Sameness is [`destination_identity`]'s normalized
///   comparison rather than a textual one (R0002-0032): `out.md`, `./out.md`,
///   `a/../out.md` and a path through a symlinked parent all name one file,
///   and the operator gets that sentence instead of the late, confusing
///   "refusing existing temp" the colliding `<dest>.tmp.<pid>` names would
///   produce halfway through staging.
/// * **One destination inside another** (R0004-0067). `--output x --map x/y`
///   names no file twice, so identity comparison passed it — and then the
///   publication needs `x` to be a regular file and a directory at once. It
///   fails in `create_dir_all` before staging when `x` already exists, and in
///   the phase-2 renames when it does not, which is the one failure this
///   commit cannot roll back. It is a question about the argv and the
///   filesystem alone, and R0002-0029 is the rule that such questions are
///   asked before the provider call, not after it.
fn vet_destinations<'a>(paths: impl IntoIterator<Item = &'a Path>) -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    // A list rather than a set: containment needs every pair, and a fileset is
    // at most nine destinations (two outputs, a report, the six-file bundle).
    let mut seen: Vec<(PathBuf, &Path)> = Vec::new();
    for path in paths {
        destination_file_name(path)?;
        let identity = destination_identity(&cwd, path);
        for (other, other_path) in &seen {
            if *other == identity {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("duplicate output destination: {}", path.display()),
                ));
            }
            // Component-wise, so `a/bc` is not inside `a/b`.
            let (inner, outer) = if identity.starts_with(other) {
                (path, *other_path)
            } else if other.starts_with(&identity) {
                (*other_path, path)
            } else {
                continue;
            };
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "output destination {} is inside output destination {}: publishing both \
                     would need {} to be a file and a directory at once",
                    inner.display(),
                    outer.display(),
                    outer.display(),
                ),
            ));
        }
        seen.push((identity, path));
    }
    Ok(())
}

/// The final path component of a destination, as UTF-8 — the name every
/// staging name is built from (`<dest>.tmp.<pid>` here, the
/// `.<name>.staging.<pid>.<token>` siblings under `--out-dir`).
///
/// A `PathBuf` off the command line is arbitrary bytes on Unix, and nothing
/// upstream requires UTF-8 of it. Before R0004-0066 the first refusal was
/// [`stage_one_file`]'s, i.e. after the translation had been paid for, and it
/// said "path has no file name" about a path that has one. Asked in the
/// destination preflight and again where the name is actually built, with the
/// same sentence either way, so an operator cannot tell the two passes apart.
fn destination_file_name(path: &Path) -> io::Result<&str> {
    let Some(name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "output destination {} has no final path component",
                path.display()
            ),
        ));
    };
    name.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "output destination {} has a final path component that is not valid UTF-8, and \
                 transync builds its staging names from that component",
                path.display()
            ),
        )
    })
}

/// What makes two destinations "the same file" for [`vet_destinations`]:
/// the path made absolute against `cwd`, with its **deepest existing parent
/// resolved** (symlinks and `..` alike) and the not-yet-existing remainder
/// folded lexically.
///
/// The final component is deliberately left unresolved. A destination that is
/// itself a symlink (`alias.md` → `out.md`) is *not* the same destination as
/// its target: publication renames over the link entry rather than through it,
/// so the two commit as two independent files and refusing the pair would be a
/// false alarm. What must collide is what shares a directory *and* a name,
/// because that is what shares a `<dest>.tmp.<pid>` staging name.
fn destination_identity(cwd: &Path, path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    };
    let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
        return lexically_normalized(&absolute);
    };
    lexically_normalized(&resolve_existing_prefix(parent)).join(name)
}

/// `dir` with its deepest **existing** ancestor replaced by that ancestor's
/// canonical path; the components below it are kept verbatim because they name
/// nothing yet. `dir` unchanged when no ancestor resolves.
fn resolve_existing_prefix(dir: &Path) -> PathBuf {
    let mut tail: Vec<&OsStr> = Vec::new();
    let mut probe = dir;
    loop {
        if let Ok(real) = fs::canonicalize(probe) {
            let mut resolved = real;
            resolved.extend(tail.iter().rev());
            return resolved;
        }
        // No name to peel (an empty path, a bare root, or a trailing `..`):
        // the lexical fold in the caller is the whole normalization here.
        let (Some(name), Some(up)) = (probe.file_name(), probe.parent()) else {
            return dir.to_path_buf();
        };
        tail.push(name);
        probe = up;
    }
}

/// Fold `.` and `..` textually. Correct for the part of a path that does not
/// exist yet (nothing there can be a symlink); the existing part is resolved
/// by [`resolve_existing_prefix`] before this runs.
fn lexically_normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(Component::ParentDir);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// Stage one payload as `<path>.tmp.<pid>`, fsynced. The destination
/// directory already exists — [`create_destination_parents`] made it before
/// the publish lock was taken.
fn stage_one_file(path: &Path, bytes: &[u8]) -> io::Result<std::path::PathBuf> {
    // Append to the FULL file name (`out.md.tmp.<pid>`) rather than replacing
    // the extension — `with_extension` would collide same-stem siblings
    // (`out.md` and `out.json` both staging as `out.tmp.<pid>`) in one fileset.
    let file_name = destination_file_name(path)?;
    let tmp = path.with_file_name(format!("{file_name}.tmp.{}", process::id()));
    // `create_new` refuses a pre-existing temp, so a hostile or stale symlink
    // planted at that name cannot redirect the write and truncate an
    // unrelated file (R0008-0006).
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("refusing existing temp {}: {e}", tmp.display()),
            )
        })?;
    if let Err(e) = f.write_all(bytes).and_then(|()| f.sync_all()) {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(tmp)
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
fn fsync_created_levels(created_dirs: &[PathBuf], notify: Notify<'_>) {
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
fn claim_anchor(dir: &Path) -> PathBuf {
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
fn missing_ancestors(dir: &Path) -> Vec<PathBuf> {
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
fn tmp_pid_suffix(rest: Option<&str>) -> Option<&str> {
    rest?
        .strip_prefix(".tmp.")
        .filter(|pid| !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit()))
}

/// Assemble the six demo-bundle payloads (ADR-0006 + OI-0001) without
/// writing anything. The caller commits them through
/// [`write_fileset_atomic`] alongside the other CLI outputs.
///
/// `index.html.tpl`'s placeholders are substituted: `{{TRANSYNC_DOC_LANG}}`
/// becomes the target language (the document's primary content language),
/// the pane `{{TRANSYNC_*_LANG_ATTR}}` slots stamp per-pane `lang`
/// attributes for screen readers and hyphenation (R0008-0050), the pane
/// `{{TRANSYNC_*_DIR_ATTR}}` slots stamp per-pane text direction (OI-0032),
/// and `{{TRANSYNC_TITLE}}` becomes `title` — the caller's already-resolved
/// document title (ti 0f26b5; the resolution order lives in
/// [`crate::translate_cmd`]'s `resolve_bundle_title`, not here).
/// `source_language` is the caller's resolved source label (the detected
/// language when the run used `auto`), or empty when unknown.
///
/// `title` is the one slot whose value can come from the **source document**
/// rather than from an argument, which is why two things about the
/// substitution are deliberate: the value is escaped like every other caller
/// string, and the fill is [`fill_template`]'s single pass rather than a chain
/// of `String::replace` (untrusted text that looks like a placeholder must not
/// become one).
///
/// The two `*_dir_attr` arguments come from
/// [`crate::direction::dir_attr`] — either the fixed literal ` dir="rtl"`
/// or the empty string, never caller text — so unlike the language labels
/// they need no attribute escaping. `<html>` deliberately carries only
/// `lang`: stamping `dir` there would flip the themer/legend chrome, and
/// the panes are the content.
///
/// `strict_csp` (`--strict-csp`, OI-0018) selects between the two fixed
/// [`CSP_META`] substitutions rather than passing text in, so the flag cannot
/// become a `<head>` injection point. It is a `bool` and not a ninth `&str`
/// slot for the same reason the others are strings: a caller cannot silently
/// transpose it with one of them.
///
/// TRACE: ADR-0006
/// TRACE: SCN-12
// One flat parameter per template slot: the placeholders are independent
// strings resolved by different rules, and a wrapper struct would only move
// the same list one indirection away.
#[allow(clippy::too_many_arguments)]
pub fn html_bundle_files(
    dir: &Path,
    source_fragment: &str,
    target_fragment: &str,
    alignment_json: &[u8],
    title: &str,
    source_language: &str,
    target_language: &str,
    source_dir_attr: &str,
    target_dir_attr: &str,
    strict_csp: bool,
) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let index = fill_template(
        INDEX_TPL,
        &[
            (
                "{{TRANSYNC_DOC_LANG}}",
                escape_attr(target_language.trim()).as_str(),
            ),
            ("{{TRANSYNC_TITLE}}", escape_attr(title.trim()).as_str()),
            (
                "{{TRANSYNC_CSP_META}}",
                if strict_csp { CSP_META } else { "" },
            ),
            (
                "{{TRANSYNC_SOURCE_LANG_ATTR}}",
                lang_attr(source_language).as_str(),
            ),
            (
                "{{TRANSYNC_TARGET_LANG_ATTR}}",
                lang_attr(target_language).as_str(),
            ),
            ("{{TRANSYNC_SOURCE_DIR_ATTR}}", source_dir_attr),
            ("{{TRANSYNC_TARGET_DIR_ATTR}}", target_dir_attr),
        ],
    );
    vec![
        (dir.join("index.html"), index.into_bytes()),
        (dir.join("source.html"), source_fragment.as_bytes().to_vec()),
        (dir.join("target.html"), target_fragment.as_bytes().to_vec()),
        (dir.join("alignment.json"), alignment_json.to_vec()),
        (dir.join("sync.js"), SYNC_JS.as_bytes().to_vec()),
        (dir.join("purify.min.js"), PURIFY_JS.as_bytes().to_vec()),
    ]
}

/// Fill the shell template's `{{TRANSYNC_*}}` slots in **one pass**: scan the
/// template once, and push each matched slot's value straight to the output
/// where nothing looks at it again. An unknown `{{…}}` run is copied verbatim.
///
/// The single pass is the point. Chained `String::replace` calls rescan text
/// they have already substituted, so one slot's value can be re-substituted by
/// a later slot — and since ti 0f26b5 one of these values is untrusted
/// document content: the title defaults to the source document's first H1
/// (invariant 7). Under a replace chain, a document whose heading reads
/// `{{TRANSYNC_CSP_META}}` would pull a `<meta>` element into the shell's
/// `<title>`, and a heading reading `{{TRANSYNC_TARGET_LANG_ATTR}}` would pull
/// in an attribute fragment. Both are inert where they land (`<title>` is
/// RCDATA, and the values are fixed literals or escaped labels), so this is
/// hardening rather than a fix for a live escape — but "a substituted value is
/// never rescanned" is a structural property, while "the replaces happen to be
/// ordered so it does not matter" is one a later edit silently revokes.
fn fill_template(tpl: &str, slots: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        rest = &rest[open..];
        let Some(close) = rest.find("}}") else {
            // An unterminated `{{` is template text, not a slot. `rest` now
            // starts at it, and the push below emits it once.
            break;
        };
        let (name, tail) = rest.split_at(close + "}}".len());
        match slots.iter().find(|(slot, _)| *slot == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(name),
        }
        rest = tail;
    }
    out.push_str(rest);
    out
}

/// Escape a string for use inside a double-quoted HTML attribute value.
/// The language labels are caller-supplied, so escaping keeps a stray `"`
/// (or markup) from breaking out of the `lang="…"` attribute in the
/// generated `index.html`.
///
/// The `<title>` text node uses the same escaper (ti 0f26b5). Escaping `&`
/// and `<` is all a text node strictly needs, and this escapes those plus
/// three characters a text node would render identically either way (`>`,
/// `"`, `'` come back as themselves when the browser decodes the entity) — a
/// superset, not a mismatch. One escaper for both is deliberate: the title is
/// the one slot fed by untrusted source content, and a second escaper is a
/// second thing that can be wrong.
fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Render a leading-space ` lang="…"` attribute for a pane, or an empty
/// string when the language is unknown (e.g. `--source-language auto` with
/// no detection). R0008-0050.
fn lang_attr(language: &str) -> String {
    let lang = language.trim();
    if lang.is_empty() {
        String::new()
    } else {
        format!(" lang=\"{}\"", escape_attr(lang))
    }
}

/// Preflight the `--html-out` directory before any output is written,
/// so a foreign-file refusal cannot strand a partial output set.
/// R0006-0012.
///
/// TRACE: contracts.md §6
pub fn preflight_html_out(dir: &Path, force: bool, notify: Notify<'_>) -> io::Result<()> {
    ensure_html_out_safe(dir, force, notify)
}

/// Preflight an `--out-dir` target: the same replaceability guard the publish
/// runs under its lock, exposed so a caller can ask the question **before**
/// paying for a translation (R0002-0029).
///
/// Advisory by construction. The answer can go stale the moment it is given,
/// which is why [`publish_out_dir`] asks again under the publication lock and
/// that second answer is the one the swap acts on.
///
/// The name check ahead of it is not advisory in the same way — a target whose
/// final component is not UTF-8 is one this writer can never build its staging
/// and backup siblings from, whatever the filesystem does next (R0004-0066).
///
/// TRACE: contracts.md §6
pub fn preflight_out_dir(target: &Path, force: bool, notify: Notify<'_>) -> io::Result<()> {
    out_dir_name(target)?;
    ensure_out_dir_replaceable(target, force, notify)
}

/// Where the six demo-bundle files land under `dir` — the destinations
/// [`html_bundle_files`] produces, without the payloads. The early
/// destination preflight (R0002-0029) needs the paths before there is any
/// content to put in them.
pub fn html_bundle_paths(dir: &Path) -> Vec<PathBuf> {
    HTML_BUNDLE_ENTRIES.iter().map(|e| dir.join(e)).collect()
}

/// Reject a destination set that names one file twice, nests one destination
/// inside another, or names a file this writer cannot build a staging name
/// for, before anything is written (R0008-0005, R0004-0066, R0004-0067) — and,
/// for a CLI run, before the provider is called (R0002-0029).
/// [`write_fileset_atomic`] repeats the checks on the set it is actually
/// given; this is the same rule asked early. See [`vet_destinations`] for what
/// each refusal is about.
pub fn preflight_destination_set(paths: &[PathBuf]) -> io::Result<()> {
    vet_destinations(paths.iter().map(PathBuf::as_path))
}

/// The six demo-bundle filenames written into an `--html-out` dir and into
/// the `html/` subdir of an `--out-dir`. Any other regular file makes the
/// directory foreign (not safe to overwrite/replace without `--force`).
///
/// TRACE: ADR-0006
const HTML_BUNDLE_ENTRIES: &[&str] = &[
    "index.html",
    "source.html",
    "target.html",
    "alignment.json",
    "sync.js",
    "purify.min.js",
];

/// Outcome of scanning a directory that should hold only the six-file demo
/// bundle. See [`scan_bundle_dir`].
enum BundleScan {
    /// Only bundle files (plus recognized staging temps) present.
    Clean,
    /// A non-UTF-8 directory entry name.
    NonUtf8,
    /// An entry that is not part of the bundle (holds the offending name).
    Foreign(String),
}

/// Scan `dir` for anything that is not part of the six-file demo bundle
/// ([`HTML_BUNDLE_ENTRIES`]).
///
/// Recognized `<name>.tmp.<pid>` (current scheme) / `<stem>.tmp.<pid>`
/// (legacy) leftovers of our own atomic writer (see [`take_staging_temp`])
/// are tolerated, and so are transync's own marker files
/// ([`is_transync_marker`]) — none of them is foreign content. Every other
/// non-bundle entry is reported as [`BundleScan::Foreign`].
///
/// **The allow-list is about regular files with those names**, which is the
/// same rule [`is_transync_marker`] states and the `--out-dir` top level has
/// applied to `out.md` and its siblings since ti `66339b`: a *directory* named
/// `index.html` would otherwise carry arbitrary nested user data past a guard
/// whose whole job is to keep a replace from `remove_dir_all`-ing content
/// transync did not write — and past the lock too, since a peer publishing
/// inside it claims a level no replace holds (ti `cbbc4e`). `file_type` is
/// `symlink_metadata`'s, so a symlink is judged as the link rather than as
/// what it points at.
fn scan_bundle_dir(dir: &Path, sweep: &mut StagingSweep) -> io::Result<BundleScan> {
    let own_pid = process::id().to_string();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Ok(BundleScan::NonUtf8);
        };
        let file_type = entry.file_type()?;
        if is_transync_marker(name, file_type) {
            continue;
        }
        if take_staging_temp(
            &entry,
            name,
            file_type,
            HTML_BUNDLE_ENTRIES,
            &own_pid,
            sweep,
        ) {
            continue;
        }
        if !file_type.is_file() || !HTML_BUNDLE_ENTRIES.contains(&name) {
            return Ok(BundleScan::Foreign(name.to_string()));
        }
    }
    Ok(BundleScan::Clean)
}

/// The file names transync writes into a directory as **bookkeeping** rather
/// than as output. Neither directory guard may call one of them foreign, or a
/// second run into a directory the first one marked would start demanding
/// `--force`.
///
/// Two names, and they answer different questions — which is exactly why the
/// ownership marker is not the lock marker:
///
/// * [`PUBLISH_LOCK_NAME`] means "a publication locked this directory". It is
///   created in directories transync does **not** own, including one whose
///   guard is about to refuse it, so its presence proves nothing about who the
///   directory belongs to.
/// * [`OUT_DIR_MARKER_NAME`] means "a `--out-dir` publication produced this
///   directory". It is written as part of the staged tree, so it appears only
///   on a target this tool published, and it survives release — a lock is
///   dropped when the run ends, ownership is not.
///
/// **The shape is half the recognition.** These are the two names both guards
/// skip *without looking inside*, so a directory wearing one would carry
/// arbitrary nested user data past a check that refuses a directory named
/// `out.md` for exactly that reason — and past it into the backup
/// `remove_dir_all` of a replace that needed no `--force`. Transync writes both
/// markers as regular files, so anything else at those names is somebody else's
/// and is judged as what it is: an entry outside the allow-list. `file_type` is
/// `symlink_metadata`'s, so a symlink is the link rather than what it points at
/// (ti `66339b` / OI-0036, review fix round).
fn is_transync_marker(name: &str, file_type: fs::FileType) -> bool {
    file_type.is_file() && (name == PUBLISH_LOCK_NAME || name == OUT_DIR_MARKER_NAME)
}

/// The `<pid>` of a `<expected>.tmp.<pid>` (current scheme) or
/// `<stem>.tmp.<pid>` (legacy) staging leftover of transync's own atomic
/// writer (see [`write_fileset_atomic`]) for one of the `expected` destination
/// names — `None` when `name` is not one.
///
/// The recognizer both directory guards share (ti `66339b`). The `--out-dir`
/// top level used to be the one place that did not have it, so a crashed
/// `--output <dir>/out.md` run left a leftover there that made the whole
/// target read as foreign: transync refusing to replace a directory over its
/// own residue.
fn staging_temp_pid<'a>(name: &'a str, expected: &[&str]) -> Option<&'a str> {
    expected.iter().find_map(|e| {
        let stem = e.rsplit_once('.').map_or(*e, |(s, _)| s);
        tmp_pid_suffix(name.strip_prefix(e)).or_else(|| tmp_pid_suffix(name.strip_prefix(stem)))
    })
}

/// Recognize — and dispose of — a staging leftover in a directory this run is
/// about to write *into*. Returns whether `name` was one, in which case the
/// caller must not judge it: it is transync's residue, not foreign content.
///
/// Only temps stamped with OUR pid are cleaned in passing (a pid-reuse
/// leftover from a crashed predecessor). A temp stamped with a DIFFERENT pid
/// is **preserved**, and its name is recorded on `sweep` for the caller to
/// report (R0001-0036): it may belong to a live run whose staging deleting
/// it would corrupt (R0008-0007), and these scans run before the publish lock
/// is taken, so "no live peer" is not something they can establish. Nothing
/// reclaims those files — contracts.md §6 says so, and names the manual
/// remedy.
///
/// A removal that *fails* is recorded too (R0004-0056). The sweep is
/// best-effort and its failure changes nothing about the publication, but
/// dropping the error made "swept" and "could not sweep" the same observable
/// silence — which is the discipline [`note_cleanup_residue`] exists to keep
/// on the failure paths, and this was one of the two passes that bypassed it.
///
/// Only a **regular file** is one of ours (ti `cbbc4e`). This writer stages
/// files, so a directory at a staging-temp name is somebody else's, and
/// recognizing it as residue would walk a whole subtree of user data past the
/// guard that calls this — the same shape rule [`is_transync_marker`] applies
/// to the two marker names, and for the same reason. It also makes the
/// own-pid branch honest: `remove_file` was never going to remove a directory.
fn take_staging_temp(
    entry: &fs::DirEntry,
    name: &str,
    file_type: fs::FileType,
    expected: &[&str],
    own_pid: &str,
    sweep: &mut StagingSweep,
) -> bool {
    if !file_type.is_file() {
        return false;
    }
    let Some(pid) = staging_temp_pid(name, expected) else {
        return false;
    };
    if pid == own_pid {
        if let Err(e) = fs::remove_file(entry.path()) {
            sweep.unremoved.push((entry.path(), e));
        }
    } else {
        sweep.foreign.push(name.to_string());
    }
    true
}

/// What one directory scan learned about transync's own staging leftovers —
/// the two facts only the scan can see, carried out to the caller that has the
/// `notify` channel.
#[derive(Default)]
struct StagingSweep {
    /// Names carrying another run's pid: preserved, never reclaimed
    /// (R0001-0036).
    foreign: Vec<String>,
    /// Own-pid leftovers this scan meant to remove and could not
    /// (R0004-0056).
    unremoved: Vec<(PathBuf, io::Error)>,
}

impl StagingSweep {
    /// Say both facts, each at most once, in the order an operator reads them:
    /// what was left alone on purpose, then what was left behind by accident.
    fn note(&self, dir: &Path, notify: Notify<'_>) {
        note_foreign_temps(dir, &self.foreign, notify);
        note_cleanup_residue(
            "staging temp file(s) carrying this run's own pid",
            BEFORE_THIS_PUBLICATION,
            &self.unremoved,
            notify,
        );
    }
}

/// Say — once, whatever the count — that this run found staging temps it will
/// neither use nor delete, and what to do about them (R0001-0036). The
/// contract promises no automatic reclamation, so the operator is the one who
/// needs the fact.
fn note_foreign_temps(dir: &Path, foreign_temps: &[String], notify: Notify<'_>) {
    let Some(first) = foreign_temps.first() else {
        return;
    };
    notify(&format!(
        "note: {} holds {} staging temp(s) from other transync runs (e.g. {first}) — kept in \
         case a run is still staging into them; delete them by hand once no transync run is \
         active",
        dir.display(),
        foreign_temps.len(),
    ));
}

/// Reject writing into a non-empty output directory unless `--force` is
/// set. We only consider the bundle's own six filenames as "safe" to
/// overwrite; any other file aborts the write.
///
/// The **shape** question ([`ensure_bundle_dir_shape`]) is asked first and is
/// not one `--force` waives.
///
/// TRACE: contracts.md §6
fn ensure_html_out_safe(dir: &Path, force: bool, notify: Notify<'_>) -> io::Result<()> {
    ensure_bundle_dir_shape(dir)?;
    if force || !dir.exists() {
        return Ok(());
    }
    let mut sweep = StagingSweep::default();
    let scan = scan_bundle_dir(dir, &mut sweep)?;
    sweep.note(dir, notify);
    match scan {
        BundleScan::Clean => Ok(()),
        BundleScan::NonUtf8 => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "html-out contains a non-UTF-8 path; pass --force to overwrite",
        )),
        BundleScan::Foreign(name) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("html-out contains non-transync file {name:?}; pass --force to overwrite"),
        )),
    }
}

/// The one `--html-out` question `--force` does not answer: the bundle writes
/// its six files **into a directory**, and no waiver turns a regular file into
/// one (R0004-0063).
///
/// `--force` is consent to destroy foreign *content* — that is what DCR-0021
/// and contracts.md §6 record it as — not permission to skip a check that
/// costs one `stat` and that the filesystem has already answered. Skipping it
/// (the guard used to open with `if force || !dir.exists()`) meant
/// `--force --html-out <regular file>` passed the early preflight AND the
/// publication-time one, and died in `create_dir_all` on the way to staging:
/// a clean failure with the targets untouched, but only after the provider had
/// been paid for the whole run — the exact expense R0002-0029 exists to
/// prevent.
///
/// The other half is the `exists()` in that condition, which **follows
/// links**: a dangling symlink at the bundle path answered "nothing there" and
/// failed the same late way even without `--force`. So the entry is read with
/// `symlink_metadata` (does something occupy this name?) and then resolved
/// with `is_dir` (does it lead to a directory?) — a symlink to a real
/// directory still passes, a link to nothing or to a file does not.
fn ensure_bundle_dir_shape(dir: &Path) -> io::Result<()> {
    match fs::symlink_metadata(dir) {
        // Nothing occupies the name: the publication creates the directory.
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(_) if dir.is_dir() => Ok(()),
        Ok(_) => Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "--html-out {} is not a directory (and --force does not make it one); the bundle \
                 writes its six files into a directory",
                dir.display()
            ),
        )),
    }
}

/// The subdirectory of an `--out-dir` target that holds the demo bundle. The
/// one nested directory a published out-dir has, which is why it is also the
/// one the guard recurses into and the one a replace locks (ti `40e2a5`).
const HTML_SUBDIR: &str = "html";

/// The top-level entries a `--out-dir` target is allowed to contain and
/// still be treated as a prior transync out-dir (safe to replace without
/// `--force`). Any other entry marks the directory as foreign.
///
/// Being *allowed* is not the same as being *ours*: the allow-list bounds what
/// may sit beside a published set, while [`OUT_DIR_MARKER_NAME`] is what says
/// transync published it. See [`ensure_out_dir_replaceable`].
///
/// EXT-2026-07 P1-6
const OUT_DIR_ENTRIES: &[&str] = &[
    "out.md",
    "alignment.json",
    "validation-report.json",
    HTML_SUBDIR,
];

/// The zero-byte-ish marker a `--out-dir` publication writes into the tree it
/// publishes, as proof that transync produced this directory (ti `66339b`,
/// OI-0036).
///
/// It is staged with the rest of the tree and renamed into place with it, so a
/// published target carries it from the instant it exists, and a target that
/// does not carry it was not published by this tool.
pub const OUT_DIR_MARKER_NAME: &str = ".transync-out-dir";

/// What the ownership marker says, for the operator who finds one. The file
/// is read by nobody — its *presence* is the whole signal — so the body is
/// there to answer "what is this and what happens if I delete it?" in the
/// place the question gets asked.
const OUT_DIR_MARKER_BODY: &[u8] = b"transync --out-dir publication marker.\n\
\n\
This directory was published by `transync translate --out-dir` and will be\n\
replaced WHOLESALE by the next such publication into it. Deleting this file\n\
makes transync refuse to replace the directory without --force.\n";

/// Publish the complete `--out-dir` output set with a staged fileset commit
/// at directory granularity (EXT-2026-07 P1-6).
///
/// `files` are destinations relative to the directory root (e.g. `out.md`,
/// `html/index.html`). Every payload is written and fsynced into a staged
/// sibling directory `.<name>.staging.<pid>.<token>`; the staged tree is then
/// published by rename:
///
/// * **Fresh target** — one `rename(staging → target)`. The target appears
///   whole or not at all.
/// * **Existing target** — `rename(target → .<name>.backup.<pid>.<token>)`,
///   then `rename(staging → target)`; on a mid-swap failure the original is
///   restored (`rename(backup → target)`). The backup is removed
///   best-effort after a successful swap. This path is **crash-safe, not
///   atomic**: `rename` cannot replace a non-empty directory in place, so a
///   reader that looks between the two renames finds no target at all. Other
///   transync runs cannot land in that window (they wait on the publish
///   lock); unrelated readers can.
///
/// The whole sequence — including both passes of the replaceability guard —
/// runs under a [`PublishLock`] on the directory holding `target`, which is
/// also where the staging and backup siblings live (R0001-0034), **and** on the
/// directories inside the target that a run could be publishing into
/// ([`published_dirs_inside`], ti `40e2a5`). The second half is what makes this
/// mode and a files-mode publish into the same tree serialize: they lock
/// different things otherwise, and DCR-0021 recorded that as a known boundary.
/// That inner set is re-read once the locks are held and retaken until it stops
/// changing ([`lock_publication_tree`]) — a target that came into existence
/// after the run started is inside the exclusion, not beside it. The target's
/// own marker is renamed away by the swap that follows, which is sound because a
/// waiter revalidates the marker it is granted (DCR-0021's 2026-08-10 note) and
/// re-locks the one in the tree that replaced it.
///
/// The staged tree carries one entry the caller did not ask for: the
/// [`OUT_DIR_MARKER_NAME`] ownership marker, which is what lets the next run's
/// guard know transync published this directory instead of inferring it from
/// the file names (ti `66339b`, OI-0036).
///
/// Foreign-file preflight does not apply *inside* the staged dir — we own it
/// wholly. It applies to the *target*: an existing target that transync did
/// not publish, or that holds entries outside [`OUT_DIR_ENTRIES`], requires
/// `force`. See [`ensure_out_dir_replaceable`]. It runs **twice** (ti
/// `cbbc4e`): once before staging, so a refusal costs no I/O, and once with the
/// staged tree in hand, so the tree the swap renames away is the tree the guard
/// judged rather than the one it saw a whole bundle-write earlier.
///
/// **Every sibling this function deletes, it created** (R0002-0001). The
/// `<token>` in the two sibling names is random per run and the staged
/// directory is made with `create_dir` (which fails rather than adopting an
/// existing name), so "we made it" is what authorizes the removal — not the
/// name alone, which a pid can be reused into. The one deletion of a sibling
/// this run did not make is [`reclaim_own_staging`], and it is confined to
/// *staging* trees carrying this process's pid: staged content is transync's
/// own, freshly generated, and worthless to anyone.
///
/// TRACE: persistence-and-files.md §staged-fileset-commit
/// TRACE: contracts.md §6
pub fn publish_out_dir(
    target: &Path,
    files: &[(PathBuf, &[u8])],
    force: bool,
    notify: Notify<'_>,
) -> io::Result<()> {
    // staging + backup are siblings of `target`, so every rename below stays
    // within one directory (one filesystem) and is a cheap atomic metadata op.
    // That directory is also the one this publish locks, so it has to exist
    // before the guard runs.
    let parent = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    // Recorded before the create, for the same reason the fileset commit
    // records them: the chain may be several levels deep and each level's
    // entry lives in the level above it (R0003-0020).
    let anchor = claim_anchor(&parent);
    let created_dirs = missing_ancestors(&parent);
    fs::create_dir_all(&parent)?;
    // The parent (and, when it had to be created, the level holding it) is the
    // flat-case lock. The directories INSIDE the target are the nested case
    // (ti `40e2a5`): a files-mode run publishing into this target locks them,
    // and this publish is about to rename the whole tree away, so it has to be
    // the same exclusion rather than a neighbouring one. What is inside the
    // target is re-read once the locks are held, because until then it is a
    // snapshot (see [`lock_publication_tree`]).
    let _lock = lock_publication_tree(anchor, &parent, target, notify)?;

    // Under the lock, and before anything is staged: a target this run may not
    // replace costs no translation output being written. It is asked once more
    // just before the swap, because the lock alone does not make this verdict
    // keep — see the second call for which writers it does not reach.
    ensure_out_dir_replaceable(target, force, notify)?;

    let name = out_dir_name(target)?;
    // Under the lock, before this run picks its own name: reclaim the staging
    // trees of a crashed predecessor whose pid the OS handed back to us.
    reclaim_own_staging(&parent, name, notify);

    let token = publish_token();
    let staging = out_dir_sibling(target, "staging", &token);
    let backup = out_dir_sibling(target, "backup", &token);

    // The ownership marker rides in the staged tree rather than being written
    // into the target afterwards: it has to be there the instant the target
    // exists, or a crash between the rename and a follow-up write would leave
    // a published directory this tool no longer recognizes as its own. Added
    // here and not by the caller for the same reason the guard is not the
    // caller's: forgetting it is not an option a caller should have.
    let mut staged: Vec<(PathBuf, &[u8])> = Vec::with_capacity(files.len() + 1);
    staged.extend_from_slice(files);
    staged.push((PathBuf::from(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY));

    // `create_dir`, and nothing blind before it: the random token makes the
    // name unpredictable and this create is what proves the tree is ours to
    // delete on every failure path below (R0002-0001).
    fs::create_dir(&staging)?;
    if let Err(e) = stage_out_dir_files(&staging, &staged, notify) {
        discard_staged_tree(&staging, notify);
        return Err(e);
    }
    fsync_dir(&staging, notify);

    // Ask the guard again, now that the staged tree is on disk (ti `cbbc4e`).
    // The first pass described the target as it was *before* staging, and
    // staging is the long part of a publication — the whole bundle written and
    // fsynced. The publish lock does not make that verdict keep: it excludes
    // transync peers that take it, and the two writers that can put a directory
    // inside the target during that window take nothing. A bare `mkdir` takes
    // no lock at all, and a files-mode peer creates its destination levels
    // *before* it locks, so the levels it makes are real and unheld while it
    // parks on the level this run holds. A third run starting then claims one
    // of them ([`claim_anchor`] answers with it, since it now exists) and
    // publishes into a tree this replace is about to rename away and
    // `remove_dir_all`.
    //
    // Restating the guard here is what turns that from "the whole staging
    // phase" into the few syscalls between this line and the rename below. It
    // cannot close that remainder — the guard is a check on a snapshot, and no
    // lock reaches a writer that does not take one — but it is the difference
    // between a window the operator can hit and one they cannot. It also covers
    // the case the first pass declined to judge: a target that did not exist
    // then and does now, which the `else` branch below would otherwise rename
    // aside unexamined.
    //
    // Silent: whatever the first pass had to say about this target (foreign
    // staging temps) it already said, and repeating it would be noise.
    if let Err(e) = ensure_out_dir_replaceable(target, force, &|_| {}) {
        discard_staged_tree(&staging, notify);
        return Err(io::Error::new(
            e.kind(),
            format!("{e} (it changed while this run was staging its output)"),
        ));
    }

    if !target.exists() {
        if let Err(e) = fs::rename(&staging, target) {
            discard_staged_tree(&staging, notify);
            return Err(e);
        }
    } else {
        // Move the existing target aside (rename can't replace a non-empty
        // directory in place), swap the new tree in, then drop the backup.
        if let Err(e) = fs::rename(target, &backup) {
            discard_staged_tree(&staging, notify);
            return Err(e);
        }
        if let Err(e) = fs::rename(&staging, target) {
            // Roll back: restore the original target from the backup we moved
            // aside, and discard the staged tree either way. Name what state
            // the caller is left in so the diagnostic is actionable
            // (EXT-2026-07 review-fix).
            let restored = fs::rename(&backup, target).is_ok();
            discard_staged_tree(&staging, notify);
            let msg = if restored {
                format!(
                    "publishing to {} failed ({e}); rolled back to the previous output",
                    target.display()
                )
            } else {
                format!(
                    "publishing to {} failed ({e}); previous output preserved at {}",
                    target.display(),
                    backup.display()
                )
            };
            return Err(io::Error::new(e.kind(), msg));
        }
        // The backup holds whatever the target was — a prior out-dir, or a
        // regular file when `--force` waived the guard. Delete it by what it
        // IS: `remove_dir_all` alone fails with NotADirectory on a file and
        // leaves a hidden dot-name behind forever (R0002-0023).
        //
        // Best-effort, and the publication has already succeeded — the swap is
        // done, the target is the new tree, and nothing here can change that.
        // But a backup that survives is the operator's ENTIRE previous output
        // sitting under a hidden name they did not choose, and this call is
        // the only thing that can see it happened (R0004-0054).
        if let Err(e) = remove_publish_residue(&backup) {
            notify(&format!(
                "note: published to {}, but the backup of the previous output could not be \
                 removed ({}: {e}); it holds the output this run replaced — delete it by hand \
                 once you no longer need it",
                target.display(),
                backup.display(),
            ));
        }
    }

    // Durability: fsync the parent so the publish rename itself survives a
    // crash, and every level this call had to create to reach it so the
    // entries naming *those* survive too (R0003-0020).
    if let Some(parent) = target.parent() {
        fsync_dir(
            if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            },
            notify,
        );
    }
    fsync_created_levels(&created_dirs, notify);
    Ok(())
}

/// Remove a publication sibling by what it *is*: a directory tree, a regular
/// file, or a symlink (the link entry, never what it points at).
///
/// `remove_dir_all` on its own is wrong twice here — it fails with
/// `NotADirectory` on a regular file (R0002-0023) and it would be the wrong
/// verb for a symlink even where it silently removes only the link.
fn remove_publish_residue(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(path),
        Ok(_) => fs::remove_file(path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Drop the staged tree of a publication that is not going to happen, and say
/// so when it will not go (R0004-0056, same discipline as R0003-0022/0024).
///
/// The bytes in there are transync's own, freshly generated and worthless to
/// anyone — which is why the removal is best-effort and never changes the
/// error the caller is already returning. It is still a directory under a
/// hidden name that this run put on the operator's disk and could not take
/// back, and the run is the only thing that knows.
fn discard_staged_tree(staging: &Path, notify: Notify<'_>) {
    if let Err(e) = remove_publish_residue(staging) {
        note_cleanup_residue(
            "staged output tree(s)",
            AFTER_A_FAILED_PUBLICATION,
            &[(staging.to_path_buf(), e)],
            notify,
        );
    }
}

/// A random token, one per publication, stamped into the staging and backup
/// sibling names (R0002-0001).
///
/// It buys two things a bare pid does not. The name is **unpredictable**, so
/// nothing — a neighbor process, a crashed predecessor, an operator's script —
/// can be sitting at the name this run is about to create; and because the
/// staged directory is then made with `create_dir`, a name that *is* occupied
/// fails the run instead of being adopted and later deleted. `RandomState` is
/// the standard library's per-instance random seed; this is a collision
/// avoider, not a secret.
fn publish_token() -> String {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    hasher.write_u32(process::id());
    format!("{:016x}", hasher.finish())
}

/// Delete the `--out-dir` **staging** trees stamped with this process's own
/// pid, under the publication lock and before this run picks its token.
///
/// This is contracts.md §6's own-pid rule (a leftover carrying the running
/// process's pid can only come from a crashed predecessor whose pid the OS
/// reused) applied to directory publication, with the evidence the rule always
/// implied: the whole reserved name must match — `.<target>.staging.<our-pid>`
/// with an optional `.<token>` — and only a real directory is removed.
///
/// **Backups are deliberately left alone.** A `.<target>.backup.<pid>` is the
/// operator's *previous output*, moved aside by a run that then died before
/// putting it back; it can be the only copy, and no accumulation argument
/// justifies deleting it sight unseen. contracts.md §6 already says a crash
/// between the two renames can leave one behind.
///
/// **Opportunistic, and audible when it does not work** (R0004-0056). Nothing
/// about the publication depends on this sweep: the run's own tree carries a
/// fresh token, so a leftover it fails to remove is retried by the next run and
/// never mistaken for this one's. What a dropped error cost was the operator's
/// ability to tell "there was nothing here" from "there is something here I
/// could not remove" — the same distinction [`note_cleanup_residue`] draws on
/// every failure path in this module, which this sweep and
/// [`take_staging_temp`] were the two places not to draw.
fn reclaim_own_staging(parent: &Path, name: &str, notify: Notify<'_>) {
    let prefix = format!(".{name}.staging.{}", process::id());
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(e) => {
            notify(&format!(
                "note: could not scan {} for staging trees left by a crashed earlier run with \
                 this process's pid ({e}); any that are there stay where they are, inert, and \
                 the next run will try again",
                parent.display(),
            ));
            return;
        }
    };
    let mut left_behind: Vec<(PathBuf, io::Error)> = Vec::new();
    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let Some(rest) = file_name.to_str().and_then(|n| n.strip_prefix(&prefix)) else {
            continue;
        };
        // `<prefix>` exactly, or `<prefix>.<hex token>`. Anything else is a
        // different pid (`…staging.1234` under pid 123) or a different scheme.
        let ours = rest.is_empty()
            || rest.strip_prefix('.').is_some_and(|token| {
                !token.is_empty() && token.bytes().all(|b| b.is_ascii_hexdigit())
            });
        if !ours {
            continue;
        }
        if matches!(entry.file_type(), Ok(kind) if kind.is_dir())
            && let Err(e) = fs::remove_dir_all(entry.path())
        {
            left_behind.push((entry.path(), e));
        }
    }
    note_cleanup_residue(
        "staging tree(s) carrying this run's own pid",
        BEFORE_THIS_PUBLICATION,
        &left_behind,
        notify,
    );
}

/// Refuse to replace a `--out-dir` target that is not a prior transync
/// out-dir unless `force` is set. A missing target is replaceable; an existing
/// one has to pass **two** independent questions, because a successful publish
/// moves the whole prior target aside and `remove_dir_all`s it:
///
/// 1. **Is anything in here not transync's?** Every top-level entry must be a
///    transync marker ([`is_transync_marker`]), a recognized staging temp
///    *file* ([`staging_temp_pid`]; a directory wearing that name is somebody
///    else's subtree, ti `cbbc4e`), or a name in [`OUT_DIR_ENTRIES`] with the
///    expected shape — `out.md`, `alignment.json` and `validation-report.json`
///    as regular files, an `html` directory holding only the six-file demo
///    bundle ([`HTML_BUNDLE_ENTRIES`]). The nested check is load-bearing:
///    without it a user file inside `html/` — or hidden under a directory
///    masquerading as one of the file names, the marker names included — would
///    be silently destroyed (EXT-2026-07 review-fix; the marker names since the
///    review fix round of ti `66339b`).
/// 2. **Did transync make this directory?** Answered by the ownership marker
///    [`OUT_DIR_MARKER_NAME`] a publication writes into the tree it publishes
///    (ti `66339b`, OI-0036) — a fact rather than an inference. The fallback
///    for a target that has no marker is the **complete** published set
///    (`out.md`, `alignment.json`, `validation-report.json` and an `html`
///    directory, all present), which is the shape only an `--out-dir` publish
///    produces: it keeps a bundle written before this marker existed — or one
///    reassembled by a `cp <dir>/* …` that dropped the dot-files —
///    republishing without a flag it never needed. A target that holds **no
///    output at all** — a directory the operator made with `mkdir`, or one this
///    tool left holding nothing but its own markers — is not asked question 2:
///    a guard that exists to stop content being destroyed has nothing to say
///    about a directory with no content in it.
///
/// Question 2 is what OI-0036 was about. Question 1 alone accepted any
/// **subset** of the allow-list, so a user directory whose only entry happened
/// to be called `out.md` was replaced — and its backup recursively removed —
/// with no `--force` and no prompt. It is also what makes question 1 able to
/// afford tolerating staging temps (ti `66339b`): transync's own residue in a
/// directory transync owns is not evidence of a foreign directory, while the
/// same residue in a directory nobody claimed no longer decides anything.
///
/// TRACE: contracts.md §6
fn ensure_out_dir_replaceable(target: &Path, force: bool, notify: Notify<'_>) -> io::Result<()> {
    if force || !target.exists() {
        return Ok(());
    }
    if !target.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "--out-dir {} exists and is not a directory; pass --force to replace",
                target.display()
            ),
        ));
    }
    let mut published: HashSet<&'static str> = HashSet::new();
    let mut owned = false;
    for entry in fs::read_dir(target)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "--out-dir contains a non-UTF-8 entry; pass --force to replace",
            ));
        };
        let file_type = entry.file_type()?;
        if is_transync_marker(name, file_type) {
            owned |= name == OUT_DIR_MARKER_NAME;
            continue;
        }
        if file_type.is_file() && staging_temp_pid(name, OUT_DIR_ENTRIES).is_some() {
            // transync's own staging residue, and nothing more is done with it
            // here — unlike the scans of a directory this run writes *into*,
            // which sweep our pid's leftovers and report other runs'. This
            // level is either replaced whole (leftover included, with the
            // backup) or left entirely untouched by a refusal, so there is
            // nothing to sweep and nothing worth promising to keep. ti 66339b.
            continue;
        }
        let Some(expected) = OUT_DIR_ENTRIES.iter().copied().find(|e| *e == name) else {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "--out-dir {} contains unexpected entry {name:?} (not a transync out-dir); pass --force to replace",
                    target.display()
                ),
            ));
        };
        if expected == HTML_SUBDIR {
            // Normally the six-file bundle directory — recurse one level with
            // the bundle allow-list. A non-directory `html` (regular file /
            // symlink) carries a transync name and holds no nested user data,
            // so it needs no recursion — and does not count toward the
            // complete published set either.
            if file_type.is_dir() {
                let mut nested = StagingSweep::default();
                let scan = scan_bundle_dir(&entry.path(), &mut nested)?;
                nested.note(&entry.path(), notify);
                match scan {
                    BundleScan::Clean => {}
                    BundleScan::NonUtf8 => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "--out-dir {} contains a non-UTF-8 entry under html/; pass --force to replace",
                                target.display()
                            ),
                        ));
                    }
                    BundleScan::Foreign(nested) => {
                        return Err(io::Error::new(
                            io::ErrorKind::AlreadyExists,
                            format!(
                                "--out-dir {} contains non-transync file html/{nested} (not a transync out-dir); pass --force to replace",
                                target.display()
                            ),
                        ));
                    }
                }
                published.insert(expected);
            }
        } else if file_type.is_file() {
            published.insert(expected);
        } else {
            // out.md / alignment.json / validation-report.json must be regular
            // files; a directory here could hide arbitrary nested user data.
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "--out-dir {} contains {name:?} which is not a regular file (not a transync out-dir); pass --force to replace",
                    target.display()
                ),
            ));
        }
    }
    if owned || published.is_empty() || published.len() == OUT_DIR_ENTRIES.len() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "--out-dir {} holds no {OUT_DIR_MARKER_NAME} marker and is not a complete transync output set, so transync did not publish it; pass --force to replace it anyway",
            target.display()
        ),
    ))
}

/// The directories **inside** an existing `--out-dir` target that a transync
/// run could be publishing into, and that this publish is therefore about to
/// destroy: the target itself and its `html/` subdirectory (ti `40e2a5`).
///
/// That is the whole shape of a published out-dir, so it is the whole set a
/// peer's `write_fileset_atomic` can be holding locks on — `--output
/// <target>/out.md`, `--html-out <target>/html`, or both. Locking them here is
/// what makes the two modes serialize instead of racing: DCR-0021's lock on the
/// level holding the target covered runs publishing into *that* level, and left
/// the nested case as a stated boundary.
///
/// A target that is a symlink or a regular file is not a directory anybody
/// publishes into — the swap renames the entry, not what it points at — so it
/// contributes nothing. `symlink_metadata`, not `is_dir`, is what draws that
/// line.
///
/// Missing levels are deliberately absent rather than created: a peer that has
/// to create one first claims the deepest level that exists ([`claim_anchor`]),
/// which is the target or the level holding it — and this publish holds both.
fn published_dirs_inside(target: &Path) -> Vec<PathBuf> {
    let is_dir = |path: &Path| matches!(fs::symlink_metadata(path), Ok(meta) if meta.is_dir());
    if !is_dir(target) {
        return Vec::new();
    }
    let mut dirs = vec![target.to_path_buf()];
    let bundle = target.join(HTML_SUBDIR);
    if is_dir(&bundle) {
        dirs.push(bundle);
    }
    dirs
}

/// How many times [`lock_publication_tree`] re-reads the target and takes the
/// whole lock set again after finding that what is inside it changed.
///
/// Three passes is the most the set can need on its own — it gains at most the
/// target and the target's `html/` — so the fourth is slack against a peer that
/// makes and unmakes those levels while this run is queued. Bounded for the
/// same reason [`PublishLock`]'s marker revalidation is: the churn is driven by
/// other processes, each pass costs opens and locks, and nothing in this process
/// can promise it ends.
const PUBLISH_TREE_RESCANS: usize = 4;

/// Take the whole publication lock for a `--out-dir` replace: `anchor` and
/// `parent` (the level the staging and backup siblings live in, and the level
/// holding *it* when this run had to create it), plus every directory inside
/// `target` a peer could be publishing into — **re-read once the locks are
/// held**, and retaken until it stops changing.
///
/// [`published_dirs_inside`] is a filesystem read, and before this run holds a
/// lock its answer is a snapshot of a moment that has passed. The sequence that
/// gets wrong (ti `40e2a5`, review fix round): a replace starts while its target
/// does not exist, so the snapshot is empty and only the level holding the
/// target is locked; a files-mode peer creates the target and publishes into it;
/// this run then takes the parent lock, finds a target that passes the guard,
/// and swaps it away — while a third run, *started after the target appeared*,
/// locks the target itself ([`claim_anchor`] answers with the target, since it
/// now exists) and publishes into the tree being renamed. Re-reading under the
/// lock is what makes the guard, the lock set and the swap act on one tree
/// rather than on three snapshots of it.
///
/// The set is dropped and retaken whole rather than extended: [`PublishLock`]
/// acquires in a global order, which is what keeps runs with overlapping sets
/// from deadlocking, and adding a directory to a set already held would take it
/// out of that order.
///
/// **What this does not close**, stated because the rest of it is closed: a
/// directory that appears inside the target *after* the last re-read, put there
/// by something this run is not serialized with — a bare `mkdir`, or a peer
/// that creates its destination levels before parking on this run's lock — is
/// still not held, so a run that starts in that window and claims it publishes
/// into a tree this replace can take away. No lock closes that, because neither
/// writer takes one. What answers it instead is the guard, restated with the
/// staged tree in hand ([`publish_out_dir`], ti `cbbc4e`): a directory that
/// appeared inside the target is an entry outside [`OUT_DIR_ENTRIES`], and the
/// replace refuses rather than renaming it away.
fn lock_publication_tree(
    anchor: PathBuf,
    parent: &Path,
    target: &Path,
    notify: Notify<'_>,
) -> io::Result<PublishLock> {
    for _ in 0..PUBLISH_TREE_RESCANS {
        let inside = published_dirs_inside(target);
        let mut dirs = vec![anchor.clone(), parent.to_path_buf()];
        dirs.extend(inside.iter().cloned());
        let lock = PublishLock::acquire(&dirs, notify)?;
        if published_dirs_inside(target) == inside {
            return Ok(lock);
        }
        drop(lock);
    }
    Err(io::Error::other(format!(
        "gave up locking {} for publication: the directories inside it changed \
         {PUBLISH_TREE_RESCANS} times while this run waited for them",
        target.display()
    )))
}

/// The final component of a `--out-dir` target, which every sibling name is
/// derived from — the same question [`destination_file_name`] asks of a
/// files-mode destination, and answered by the same code so the two modes
/// refuse the same input with the same sentence (R0004-0066). Asked in
/// [`preflight_out_dir`] too, so a target this writer cannot name costs no
/// provider call.
fn out_dir_name(target: &Path) -> io::Result<&str> {
    destination_file_name(target)
}

/// Build the `.<name>.<kind>.<pid>.<token>` sibling path for a `--out-dir`
/// target. The pid keeps [`reclaim_own_staging`]'s own-pid rule expressible;
/// the token (see [`publish_token`]) makes the name this run's alone.
fn out_dir_sibling(target: &Path, kind: &str, token: &str) -> PathBuf {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!(".{name}.{kind}.{}.{token}", process::id()))
}

/// Write every payload into the (freshly created, empty) staged directory,
/// creating relative subdirectories as needed and fsyncing each file plus
/// **every** subdirectory level it made. `staging` itself is fsynced by the
/// caller, which is what makes the shallowest level's entry durable.
fn stage_out_dir_files(
    staging: &Path,
    files: &[(PathBuf, &[u8])],
    notify: Notify<'_>,
) -> io::Result<()> {
    let mut created_subdirs: Vec<PathBuf> = Vec::new();
    let mut recorded: HashSet<PathBuf> = HashSet::new();
    for (rel, bytes) in files {
        // Every entry must stay INSIDE the staged tree (R0002-0004). An
        // absolute `rel` would make `join` discard the staging root outright
        // and a `..` component would climb out of it, so the whole publication
        // — including the rename that replaces the target — would act on a
        // path the caller never named. Today's callers pass fixed names; this
        // is the guard that keeps that a fact rather than a habit.
        if rel.as_os_str().is_empty()
            || rel
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "out-dir entry must be a contained relative path: {}",
                    rel.display()
                ),
            ));
        }
        let dest = staging.join(rel);
        if let Some(parent) = dest.parent()
            && parent != staging
        {
            fs::create_dir_all(parent)?;
            // EVERY level, not just the deepest (R0003-0020): each one holds
            // the entry naming the level below it, so flushing only the file's
            // own parent leaves an unflushed link in the chain.
            for level in staged_levels(staging, &dest) {
                if recorded.insert(level.clone()) {
                    created_subdirs.push(level);
                }
            }
        }
        // `create_new` — the staged dir is ours and starts empty, so a
        // pre-existing name means a duplicate destination in `files`.
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&dest)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    for dir in &created_subdirs {
        fsync_dir(dir, notify);
    }
    Ok(())
}

/// Every directory level strictly between `staging` and `dest`, shallowest
/// first — the levels [`stage_out_dir_files`] had to create for one payload.
/// Empty when `dest` sits directly in `staging`.
///
/// Split out from its caller so the recorded set is testable: the defect it
/// fixes (R0003-0020) was recording only `dest.parent()`, which is this list's
/// last element.
fn staged_levels(staging: &Path, dest: &Path) -> Vec<PathBuf> {
    let mut levels: Vec<PathBuf> = Vec::new();
    let Some(parent) = dest.parent() else {
        return levels;
    };
    for level in parent.ancestors() {
        if level == staging || level.as_os_str().is_empty() {
            break;
        }
        levels.push(level.to_path_buf());
    }
    levels.reverse();
    levels
}

/// Best-effort fsync of a directory so a preceding rename/create is durable.
///
/// Best-effort, and **audible** (R0002-0025). The publication is not as durable
/// as this writer claims, nobody but this writer can know it, and swallowing it
/// left the run reporting an unqualified success. It is not fatal — the payload
/// bytes are already `sync_all`ed one by one ([`stage_one_file`],
/// [`stage_out_dir_files`], which propagate their errors) so the worst a crash
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
    use std::cell::RefCell;
    use std::sync::{Arc, Barrier};

    fn silent(_: &str) {}

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

    /// The bundle's `index.html` for one `--strict-csp` setting, with every
    /// other slot held fixed so the two shells differ by the flag alone.
    fn bundle_index(strict_csp: bool) -> String {
        titled_bundle_index("transync", strict_csp)
    }

    /// The bundle's `index.html` for one title, with every other slot fixed.
    fn titled_bundle_index(title: &str, strict_csp: bool) -> String {
        let files = html_bundle_files(
            Path::new("html"),
            "<main></main>",
            "<main></main>",
            b"{}",
            title,
            "en",
            "ko",
            "",
            "",
            strict_csp,
        );
        let (_, index) = files
            .into_iter()
            .find(|(path, _)| path.ends_with("index.html"))
            .expect("the bundle carries an index.html");
        String::from_utf8(index).expect("the shell is UTF-8")
    }

    /// The text between `<title>` and `</title>` in a shell.
    fn title_of(index: &str) -> &str {
        let open = index
            .find("<title>")
            .expect("the shell has a title element")
            + "<title>".len();
        let close = index[open..]
            .find("</title>")
            .expect("the title element closes");
        &index[open..open + close]
    }

    /// OI-0018 (posture amended 2026-08-07): the CSP is opt-in, and opting out
    /// leaves the shell exactly as it was before the flag existed. Asserting
    /// "the strict shell minus the element IS the default shell" pins both
    /// halves at once — the element is added, and nothing else moved.
    #[test]
    fn the_csp_element_is_the_only_difference_the_flag_makes() {
        let plain = bundle_index(false);
        let strict = bundle_index(true);

        assert!(
            !plain.contains("Content-Security-Policy"),
            "the default posture emits no policy: {plain}"
        );
        assert!(
            strict.contains(CSP_META),
            "--strict-csp must emit the policy element verbatim: {strict}"
        );
        assert_eq!(
            strict.replace(CSP_META, ""),
            plain,
            "--strict-csp must add the policy element and change nothing else"
        );
    }

    /// The policy has to sit in `<head>` (a `<meta>` policy governs only what
    /// is parsed after it), and it has to carry every directive the bundle
    /// needs in order to still work: its own inline style block and module
    /// script, its two sibling scripts, its three same-origin fetches, and
    /// `data:` images. The template also has to leave no slot unsubstituted in
    /// either mode.
    #[test]
    fn the_strict_shell_is_still_a_working_bundle_shell() {
        let strict = bundle_index(true);
        let head_end = strict.find("</head>").expect("the shell has a head");
        let policy_at = strict
            .find("http-equiv=\"Content-Security-Policy\"")
            .expect("the strict shell declares a policy");
        assert!(
            policy_at < head_end,
            "a meta policy must be declared in <head>, before what it governs"
        );

        for directive in [
            "default-src 'self'",
            "img-src 'self' data:",
            "script-src 'self' 'unsafe-inline'",
            "style-src 'self' 'unsafe-inline'",
            "connect-src 'self'",
            "object-src 'none'",
            "base-uri 'none'",
        ] {
            assert!(
                strict.contains(directive),
                "the policy must carry `{directive}` or the bundle stops working: {strict}"
            );
        }

        for shell in [strict, bundle_index(false)] {
            assert!(
                !shell.contains("{{TRANSYNC_"),
                "every template slot must be substituted: {shell}"
            );
        }
    }

    /// ti 0f26b5: the caller's title reaches `<title>` verbatim, and the two
    /// language slots are still the two the shell needs — `<html lang>` is the
    /// target (the document's own language) while the panes carry one each.
    #[test]
    fn the_shell_carries_the_callers_title_and_the_document_language() {
        let index = titled_bundle_index("Design Notes", false);
        assert_eq!(title_of(&index), "Design Notes");
        assert!(
            index.contains("<html lang=\"ko\">"),
            "the document language is the TARGET language: {index}"
        );
    }

    /// ti 0f26b5 / invariant 7: the title can come from the source document's
    /// first H1, so it is untrusted text. It must land in `<title>` as text —
    /// escaped, never as markup — and a title that *looks* like a template
    /// slot must stay text too: the single-pass fill never rescans a value it
    /// has already substituted.
    #[test]
    fn an_untrusted_title_is_escaped_and_never_re_substituted() {
        let hostile = titled_bundle_index("</title><script>alert(1)</script> & \"co\"", false);
        assert_eq!(
            title_of(&hostile),
            "&lt;/title&gt;&lt;script&gt;alert(1)&lt;/script&gt; &amp; &quot;co&quot;",
            "every markup character must be escaped inside the title text"
        );
        assert!(
            !hostile.contains("<script>"),
            "a heading must not be able to open an element: {hostile}"
        );

        let slotlike = titled_bundle_index("{{TRANSYNC_CSP_META}}", true);
        assert_eq!(
            title_of(&slotlike),
            "{{TRANSYNC_CSP_META}}",
            "a substituted value must never be substituted into again"
        );
        assert_eq!(
            slotlike.matches("Content-Security-Policy").count(),
            1,
            "the policy element belongs in <head> once, not also in the title: {slotlike}"
        );
    }

    /// The filler substitutes what it knows and leaves everything else alone —
    /// including a `{{…}}` run that names no slot and an unterminated `{{`,
    /// both of which are template text rather than a slot with a missing
    /// value.
    #[test]
    fn the_filler_replaces_known_slots_and_copies_the_rest() {
        let slots = [("{{A}}", "1"), ("{{B}}", "")];
        assert_eq!(fill_template("x{{A}}y{{B}}z", &slots), "x1yz");
        assert_eq!(fill_template("{{A}}{{A}}", &slots), "11");
        assert_eq!(
            fill_template("{{UNKNOWN}} {{A}}", &slots),
            "{{UNKNOWN}} 1",
            "an unknown slot is template text, not an empty substitution"
        );
        assert_eq!(fill_template("tail {{A", &slots), "tail {{A");
        assert_eq!(fill_template("no slots", &slots), "no slots");
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

    /// R0001-0036: a staging temp stamped with another process's pid is left
    /// alone — it may belong to a live run — and the run says so instead of
    /// pretending it reclaimed it. The pid is fabricated, so nothing here
    /// depends on a real second process existing.
    #[test]
    fn a_foreign_pid_temp_is_preserved_and_reported_not_reclaimed() {
        let dir = scratch_dir("foreign-temp");
        let stale = dir.join("index.html.tmp.2147483646");
        fs::write(&stale, b"someone else's staging").expect("plant the temp");
        fs::write(dir.join("index.html"), b"<html>").expect("plant a bundle file");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        ensure_html_out_safe(&dir, false, &sink)
            .expect("a foreign-pid temp is not a foreign-file refusal");
        let notices = notices.into_inner();

        assert!(
            stale.exists(),
            "a temp this run does not own must survive the scan"
        );
        assert_eq!(notices.len(), 1, "expected one operator note: {notices:?}");
        assert!(
            notices[0].contains("index.html.tmp.2147483646")
                && notices[0].contains("delete them by hand"),
            "the note must name the temp and the remedy: {}",
            notices[0]
        );
    }

    /// The same scan still reclaims a leftover carrying OUR pid (a crashed
    /// predecessor that the OS reused the pid of), and stays silent about it.
    #[test]
    fn an_own_pid_temp_is_reclaimed_silently() {
        let dir = scratch_dir("own-temp");
        let own = dir.join(format!("index.html.tmp.{}", process::id()));
        fs::write(&own, b"our own leftover").expect("plant the temp");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        ensure_html_out_safe(&dir, false, &sink).expect("own leftovers are not foreign");
        let notices = notices.into_inner();

        assert!(!own.exists(), "our own leftover must be reclaimed");
        assert!(notices.is_empty(), "nothing to report: {notices:?}");
    }

    /// The lock marker a previous publication left behind is transync's own
    /// file: neither directory guard may call it foreign, or the second run
    /// into a directory would refuse without `--force`.
    #[test]
    fn the_publish_lock_marker_is_not_foreign_content() {
        let bundle = scratch_dir("lock-marker-bundle");
        fs::write(bundle.join(PUBLISH_LOCK_NAME), b"").expect("plant the marker");
        ensure_html_out_safe(&bundle, false, &silent)
            .expect("the lock marker must not block an --html-out republish");

        let out_dir = scratch_dir("lock-marker-out-dir");
        fs::write(out_dir.join(PUBLISH_LOCK_NAME), b"").expect("plant the marker");
        fs::write(out_dir.join(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY)
            .expect("plant the ownership marker");
        fs::write(out_dir.join("out.md"), b"# prior").expect("plant a prior output");
        ensure_out_dir_replaceable(&out_dir, false, &silent)
            .expect("the lock marker must not block an --out-dir replace");
    }

    /// The complete set a `--out-dir` publish writes, as destinations relative
    /// to the target — what [`publish_out_dir`]'s caller hands it.
    fn out_dir_payloads(marker: u8) -> Vec<(PathBuf, Vec<u8>)> {
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

    /// The same set written straight onto disk, as a build that predates the
    /// ownership marker would have left it.
    fn plant_complete_out_dir(dir: &Path) {
        fs::create_dir_all(dir.join(HTML_SUBDIR)).expect("the bundle directory");
        for (rel, bytes) in out_dir_payloads(b'p') {
            fs::write(dir.join(rel), bytes).expect("plant a published file");
        }
    }

    /// OI-0036: "holds one allow-listed name" is not "is a bundle this tool
    /// produced". A user directory whose only entry happens to be called
    /// `out.md` used to qualify as a prior out-dir — moved aside and its
    /// backup recursively removed, with no `--force` and no prompt. The
    /// ownership marker is what turns the inference into a fact, so the same
    /// directory carrying one is replaceable again.
    #[test]
    fn a_sparse_allow_listed_subset_is_not_a_transync_out_dir() {
        let dir = scratch_dir("sparse-subset");
        fs::write(dir.join("out.md"), b"somebody's notes").expect("plant the user's file");

        let err = ensure_out_dir_replaceable(&dir, false, &silent)
            .expect_err("one allow-listed name is not evidence transync published this");
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{err}");
        assert!(
            err.to_string().contains(OUT_DIR_MARKER_NAME),
            "the refusal must name what is missing: {err}"
        );
        ensure_out_dir_replaceable(&dir, true, &silent).expect("--force still replaces it");

        fs::write(dir.join(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY).expect("plant the marker");
        ensure_out_dir_replaceable(&dir, false, &silent)
            .expect("a directory transync marked as its own is replaceable");
    }

    /// ti `66339b`: a `<name>.tmp.<pid>` at the TOP LEVEL of an `--out-dir`
    /// target is transync's own staging residue — left by a crashed
    /// `--output <dir>/out.md` run — and used to be the one place no guard
    /// recognized it, so the target read as foreign and the publish demanded
    /// `--force` over transync's own leftovers. Whose pid it carries does not
    /// change the answer here: this level is replaced whole or not at all.
    #[test]
    fn a_top_level_staging_temp_is_transync_residue_not_a_foreign_file() {
        let dir = scratch_dir("out-dir-top-temp");
        fs::write(dir.join(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY).expect("plant the marker");
        fs::write(dir.join("out.md"), b"# prior").expect("plant a prior output");
        let ours = dir.join(format!("out.md.tmp.{}", process::id()));
        let theirs = dir.join("alignment.json.tmp.2147483646");
        let legacy = dir.join("out.tmp.2147483645");
        for temp in [&ours, &theirs, &legacy] {
            fs::write(temp, b"staging").expect("plant the temp");
        }

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        ensure_out_dir_replaceable(&dir, false, &sink)
            .expect("transync's own staging residue must not make the target foreign");
        assert!(
            notices.into_inner().is_empty(),
            "a level about to be replaced whole has nothing to promise about its temps"
        );
    }

    /// The publication writes its own proof of authorship into the tree it
    /// publishes, so the very next run recognizes the target without guessing
    /// from file names — and republishing needs no `--force`, which is the
    /// property the marker must not cost.
    #[test]
    fn a_published_out_dir_carries_its_ownership_marker_and_republishes() {
        let root = scratch_dir("out-dir-marker");
        let target = root.join("published");
        let owned = out_dir_payloads(b'A');
        let files: Vec<(PathBuf, &[u8])> = owned
            .iter()
            .map(|(p, b)| (p.clone(), b.as_slice()))
            .collect();

        publish_out_dir(&target, &files, false, &silent).expect("first publish");
        let marker = target.join(OUT_DIR_MARKER_NAME);
        assert!(
            marker.is_file(),
            "a published out-dir must carry its ownership marker"
        );
        publish_out_dir(&target, &files, false, &silent)
            .expect("republishing over our own output needs no --force");
        assert!(
            marker.is_file(),
            "the marker must survive the swap that replaces the tree"
        );
    }

    /// `mkdir out && transync translate --out-dir out` is an ordinary way to
    /// start, and the ownership question must not turn it into a `--force`
    /// recommendation: an empty target holds nothing the replace could destroy,
    /// and neither does one holding only transync's own markers — the lock
    /// marker this very publish creates in the target it is about to replace,
    /// among them.
    #[test]
    fn an_empty_target_needs_no_ownership_marker() {
        let dir = scratch_dir("empty-target");
        ensure_out_dir_replaceable(&dir, false, &silent)
            .expect("an empty directory holds nothing to protect");

        fs::write(dir.join(PUBLISH_LOCK_NAME), b"").expect("the lock marker a publish creates");
        ensure_out_dir_replaceable(&dir, false, &silent)
            .expect("a target holding only transync's bookkeeping is still empty of output");

        fs::write(dir.join("out.md"), b"somebody's notes").expect("now it holds output");
        assert!(
            ensure_out_dir_replaceable(&dir, false, &silent).is_err(),
            "one file is content, and content is what the ownership question is about"
        );
    }

    /// The fallback the marker leaves in place: a directory holding the
    /// COMPLETE published set is a transync out-dir even without a marker —
    /// a bundle written before the marker existed, or one reassembled by a
    /// `cp <dir>/* …` that dropped the dot-files, still republishes. Removing
    /// any member of the set drops it back to the sparse-subset case.
    #[test]
    fn a_complete_prior_output_set_republishes_without_a_marker() {
        let dir = scratch_dir("complete-legacy");
        plant_complete_out_dir(&dir);
        ensure_out_dir_replaceable(&dir, false, &silent)
            .expect("a complete prior output set is a transync out-dir");

        fs::remove_file(dir.join("validation-report.json")).expect("break the set");
        assert!(
            ensure_out_dir_replaceable(&dir, false, &silent).is_err(),
            "an incomplete set with no marker is not evidence transync published this"
        );
    }

    /// The marker answers "did transync make this?", not "is everything in
    /// here transync's?" — the second question is what stops the backup
    /// `remove_dir_all` from taking a file the operator dropped into a
    /// published bundle.
    #[test]
    fn an_owned_out_dir_still_refuses_content_transync_did_not_write() {
        let dir = scratch_dir("owned-but-foreign");
        plant_complete_out_dir(&dir);
        fs::write(dir.join(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY).expect("plant the marker");
        fs::write(dir.join("NOTES.txt"), b"the operator's").expect("plant the user's file");

        let err = ensure_out_dir_replaceable(&dir, false, &silent)
            .expect_err("a marked directory is still not a licence to delete user data");
        assert!(
            err.to_string().contains("NOTES.txt"),
            "the refusal must name the entry: {err}"
        );
    }

    /// A marker NAME is not a marker. The two bookkeeping names are the only
    /// entries both guards skip *without looking inside*, so a directory
    /// wearing one carries whatever it likes past a check that refuses a
    /// directory called `out.md` for precisely that reason — and into the
    /// backup `remove_dir_all` of a replace that needed no `--force`. The lock
    /// name has had that exposure since DCR-0021; the ownership marker added a
    /// second one in the change that tightened the guard against nested data.
    #[test]
    fn a_directory_wearing_a_marker_name_is_not_a_marker() {
        let dir = scratch_dir("marker-shaped-directory");
        plant_complete_out_dir(&dir);
        let hidden = dir.join(OUT_DIR_MARKER_NAME);
        fs::create_dir(&hidden).expect("a directory wearing the ownership marker's name");
        fs::write(hidden.join("NOTES.txt"), b"the operator's").expect("user data under it");

        let err = ensure_out_dir_replaceable(&dir, false, &silent)
            .expect_err("a directory is not a marker, whatever it is called");
        assert!(
            err.to_string().contains(OUT_DIR_MARKER_NAME),
            "the refusal must name the entry: {err}"
        );
        ensure_out_dir_replaceable(&dir, true, &silent).expect("--force still replaces it");
        fs::remove_dir_all(&hidden).expect("clear it for the second name");

        fs::create_dir(dir.join(PUBLISH_LOCK_NAME)).expect("the same trick with the lock name");
        assert!(
            ensure_out_dir_replaceable(&dir, false, &silent).is_err(),
            "the lock name hides nested data exactly as well as the ownership marker's"
        );

        // The two guards must not disagree about what a bundle is, so the
        // `--html-out` scan judges the same entry the same way.
        let bundle = scratch_dir("marker-shaped-bundle-entry");
        fs::create_dir(bundle.join(PUBLISH_LOCK_NAME)).expect("plant it in a bundle directory");
        assert!(
            ensure_html_out_safe(&bundle, false, &silent).is_err(),
            "the bundle guard skips the marker names without looking inside them too"
        );
    }

    /// R0004-0063: `--force` is consent to destroy foreign *content*
    /// (DCR-0021, contracts.md §6). It is not permission to skip the one
    /// question that costs a single `stat` and that no waiver can change the
    /// answer to — the bundle writes six files INTO a directory, and a regular
    /// file is not one. Skipping it meant the run reached `create_dir_all` at
    /// publication time, i.e. after the provider had been paid for the whole
    /// document, to be told something that was true before it started.
    #[test]
    fn a_forced_html_out_still_has_to_be_a_directory() {
        let root = scratch_dir("html-out-shape");
        let regular = root.join("bundle");
        fs::write(&regular, b"not a directory").expect("plant the file at the bundle path");

        for force in [false, true] {
            let err = ensure_html_out_safe(&regular, force, &silent)
                .expect_err("a regular file cannot hold the bundle, forced or not");
            assert_eq!(err.kind(), io::ErrorKind::AlreadyExists, "{err}");
            assert!(
                err.to_string().contains("is not a directory"),
                "the message must name the shape, not a foreign file: {err}"
            );
        }

        // What the check must NOT refuse: a path nothing occupies (the
        // publication creates it) and an ordinary bundle directory.
        let fresh = root.join("fresh");
        for force in [false, true] {
            ensure_html_out_safe(&fresh, force, &silent)
                .expect("a destination that does not exist yet is made by the publication");
        }
        fs::create_dir(&fresh).expect("the ordinary case");
        ensure_html_out_safe(&fresh, false, &silent).expect("an empty directory is a bundle dir");

        #[cfg(unix)]
        {
            // The other half of the same defect: `exists()` FOLLOWS links, so
            // a dangling one answered "nothing here" and failed late even
            // without `--force`.
            let dangling = root.join("dangling");
            std::os::unix::fs::symlink(root.join("nowhere"), &dangling).expect("dangle it");
            for force in [false, true] {
                let err = ensure_html_out_safe(&dangling, force, &silent)
                    .expect_err("a link to nothing is not a directory either");
                assert!(err.to_string().contains("is not a directory"), "{err}");
            }

            let linked = root.join("linked");
            std::os::unix::fs::symlink(&fresh, &linked).expect("link to a real directory");
            ensure_html_out_safe(&linked, false, &silent)
                .expect("a symlink that resolves to a directory IS a directory");
        }
    }

    /// R0002-0004: `publish_out_dir` writes only INSIDE its staged tree. An
    /// absolute entry (which `join` would let replace the staging root) and a
    /// `..` entry (which would climb out of it) are refused before any file is
    /// opened, and the target is never created.
    #[test]
    fn out_dir_entries_that_escape_the_staged_tree_are_refused() {
        let root = scratch_dir("out-dir-escape");
        let outside = root.join("OUTSIDE.md");
        let target = root.join("published");

        for rel in [
            PathBuf::from("../OUTSIDE.md"),
            outside.clone(),
            PathBuf::from(""),
        ] {
            let files: Vec<(PathBuf, &[u8])> = vec![
                (PathBuf::from("out.md"), b"ok".as_slice()),
                (rel.clone(), b"escaped".as_slice()),
            ];
            let err = publish_out_dir(&target, &files, false, &silent)
                .expect_err("an entry outside the staged tree must be refused");
            assert_eq!(
                err.kind(),
                io::ErrorKind::InvalidInput,
                "expected a rejected path, got {err} for {}",
                rel.display()
            );
        }

        assert!(
            !outside.exists(),
            "nothing may be written outside the target"
        );
        assert!(!target.exists(), "a refused publish leaves no target");
    }

    /// R0002-0023: `--force` aimed at a target that is a regular file publishes
    /// the directory and leaves NO hidden backup beside it. `remove_dir_all`
    /// alone fails with NotADirectory on the moved-aside file and, being
    /// best-effort, said nothing — so the dot-name stayed forever.
    #[test]
    fn replacing_a_regular_file_target_leaves_no_backup_residue() {
        let root = scratch_dir("file-target");
        let target = root.join("published");
        fs::write(&target, b"a regular file, not an out-dir").expect("plant the file target");

        let files: Vec<(PathBuf, &[u8])> = vec![(PathBuf::from("out.md"), b"fresh".as_slice())];
        publish_out_dir(&target, &files, true, &silent).expect("--force replaces a file target");

        assert!(
            target.is_dir(),
            "the published target is the staged directory"
        );
        for entry in fs::read_dir(&root).expect("the parent is readable") {
            let name = entry.expect("entry").file_name();
            let name = name.to_string_lossy().into_owned();
            assert!(
                !name.contains(".backup.") && !name.contains(".staging."),
                "publication residue survived: {name}"
            );
        }
    }

    /// R0002-0001: the sibling names carry a random per-run token, so a
    /// publication cannot delete a sibling it did not create. The case that
    /// matters is a *backup* left by a crashed predecessor whose pid the OS
    /// reused — it holds the operator's previous output and can be the only
    /// copy. Before the token, republishing blew it away with an unconditional
    /// `remove_dir_all` on the pid-derived name.
    #[test]
    fn a_republish_leaves_a_predecessors_backup_alone() {
        let root = scratch_dir("predecessor-backup");
        let target = root.join("published");
        let stale_backup = root.join(format!(".published.backup.{}", process::id()));
        fs::create_dir(&stale_backup).expect("plant the predecessor's backup");
        fs::write(stale_backup.join("out.md"), b"the only copy").expect("plant its content");

        let files: Vec<(PathBuf, &[u8])> = vec![(PathBuf::from("out.md"), b"first".as_slice())];
        publish_out_dir(&target, &files, false, &silent).expect("first publish");
        publish_out_dir(&target, &files, false, &silent).expect("republish over the target");

        assert_eq!(
            fs::read(stale_backup.join("out.md")).ok(),
            Some(b"the only copy".to_vec()),
            "a predecessor's backup is the previous output, not scratch space"
        );
    }

    /// The other side of the same rule (contracts.md §6's own-pid reclamation,
    /// applied to `--out-dir`): a STAGING tree carrying our own pid is
    /// transync's own half-written scratch and is reclaimed, while another
    /// pid's staging — which may belong to a live run — is not.
    #[test]
    fn own_pid_staging_is_reclaimed_and_a_foreign_one_is_not() {
        let root = scratch_dir("staging-reclaim");
        let ours = root.join(format!(".published.staging.{}.abc123", process::id()));
        let ours_untokened = root.join(format!(".published.staging.{}", process::id()));
        let theirs = root.join(".published.staging.2147483646.abc123");
        let neighbor = root.join(format!(".other.staging.{}.abc123", process::id()));
        for dir in [&ours, &ours_untokened, &theirs, &neighbor] {
            fs::create_dir(dir).expect("plant the staging dir");
        }

        reclaim_own_staging(&root, "published", &silent);

        assert!(
            !ours.exists() && !ours_untokened.exists(),
            "our own leftovers are ours to clear"
        );
        assert!(
            theirs.exists(),
            "another pid's staging may belong to a live run"
        );
        assert!(
            neighbor.exists(),
            "another target's staging is not this publish's business"
        );
    }

    /// R0004-0056: the sweep is opportunistic and its failure changes nothing
    /// about the publication — but a dropped error made "there was nothing
    /// here" and "there is something here I could not remove" the same
    /// observable silence, which is the one distinction the whole
    /// [`note_cleanup_residue`] discipline exists to keep (R0003-0022/0024).
    /// Both of the sweep's failure modes speak now: the scan that cannot read
    /// the directory at all, and the leftover it cannot delete.
    #[cfg(unix)]
    #[test]
    fn a_staging_sweep_that_cannot_finish_says_so() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_dir("staging-sweep-notes");

        // A parent that cannot be read at all: no leftover is reclaimed and
        // the operator is told the sweep did not run, not that it found
        // nothing.
        let not_a_dir = root.join("regular-file");
        fs::write(&not_a_dir, b"x").expect("a parent that is not a directory");
        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        reclaim_own_staging(&not_a_dir, "published", &sink);
        let notices = notices.into_inner();
        assert_eq!(notices.len(), 1, "one note: {notices:?}");
        assert!(
            notices[0].contains(&not_a_dir.display().to_string()),
            "the note must name what could not be scanned: {}",
            notices[0]
        );

        // A leftover that will not go: read-only on the staging tree makes
        // `remove_dir_all` fail on the entry inside it, which is the shape of
        // any real failure here.
        let stuck = root.join(format!(".published.staging.{}.abc123", process::id()));
        fs::create_dir(&stuck).expect("plant the leftover");
        fs::write(stuck.join("out.md"), b"scratch").expect("plant its content");
        fs::set_permissions(&stuck, fs::Permissions::from_mode(0o500)).expect("seal it");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        reclaim_own_staging(&root, "published", &sink);
        let notices = notices.into_inner();
        // Restore before any assertion can unwind past the cleanup.
        fs::set_permissions(&stuck, fs::Permissions::from_mode(0o700)).expect("unseal it");

        assert_eq!(notices.len(), 1, "one bounded note: {notices:?}");
        assert!(
            notices[0].contains(&stuck.display().to_string())
                && notices[0].contains("staging tree(s)"),
            "the note must name the residue: {}",
            notices[0]
        );

        // And the ordinary case stays quiet: a sweep with nothing to do says
        // nothing, or the note would mean nothing.
        let quiet = scratch_dir("staging-sweep-quiet");
        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        reclaim_own_staging(&quiet, "published", &sink);
        assert!(
            notices.into_inner().is_empty(),
            "nothing to reclaim is not an event"
        );
    }

    /// R0004-0056, the other half: the directory guards sweep this run's own
    /// pid's staging temps out of a directory they are about to write into,
    /// and that removal dropped its error too. The scan's verdict is unchanged
    /// — transync's own residue is not foreign content — but the operator now
    /// hears that a file the run meant to clear is still there.
    #[cfg(unix)]
    #[test]
    fn a_staging_temp_that_cannot_be_swept_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_dir("sweep-temp-notes");
        let dir = root.join("bundle");
        fs::create_dir(&dir).expect("the bundle directory");
        let stuck = dir.join(format!("index.html.tmp.{}", process::id()));
        fs::write(&stuck, b"our own crashed predecessor's scratch").expect("plant the temp");
        // Read+execute only: the scan can still list the directory, but it
        // cannot unlink inside it.
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o500)).expect("seal the directory");

        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        let verdict = ensure_html_out_safe(&dir, false, &sink);
        let notices = notices.into_inner();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).expect("unseal it");

        verdict.expect("our own staging residue is not foreign content");
        assert_eq!(notices.len(), 1, "one bounded note: {notices:?}");
        assert!(
            notices[0].contains(&stuck.display().to_string()) && notices[0].contains("own pid"),
            "the note must name the temp and whose it is: {}",
            notices[0]
        );
    }

    /// R0004-0054: the publication has already succeeded when the backup is
    /// dropped, and nothing about the swap can be undone by this removal
    /// failing. What survives it is the operator's ENTIRE previous output,
    /// under a hidden dot-name they never chose — and this call is the only
    /// thing in the process that can see it happened. contracts.md §6 calls
    /// the removal best-effort; best-effort is not the same as silent.
    #[cfg(unix)]
    #[test]
    fn a_backup_that_survives_a_successful_publish_is_reported() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch_dir("backup-residue-note");
        let target = root.join("published");
        // The previous output holds a subdirectory the backup's own
        // `remove_dir_all` cannot descend into, so the swap succeeds and only
        // the cleanup fails.
        let sealed = target.join("sealed");
        fs::create_dir_all(&sealed).expect("the previous output");
        fs::write(target.join("out.md"), b"previous").expect("its markdown");
        fs::write(sealed.join("kept.md"), b"kept").expect("something to keep it non-empty");
        fs::set_permissions(&sealed, fs::Permissions::from_mode(0o500)).expect("seal it");

        let files: Vec<(PathBuf, &[u8])> = vec![(PathBuf::from("out.md"), b"fresh".as_slice())];
        let notices = RefCell::new(Vec::new());
        let sink = |line: &str| notices.borrow_mut().push(line.to_string());
        let outcome = publish_out_dir(&target, &files, true, &sink);
        let notices = notices.into_inner();

        let backup = fs::read_dir(&root)
            .expect("the parent is readable")
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().contains(".backup."))
            });
        if let Some(backup) = &backup {
            fs::set_permissions(backup.join("sealed"), fs::Permissions::from_mode(0o700))
                .expect("unseal the surviving backup");
        }

        outcome.expect("the publication itself succeeds; only the cleanup fails");
        assert_eq!(
            fs::read(target.join("out.md")).ok(),
            Some(b"fresh".to_vec()),
            "the swap happened"
        );
        let backup = backup.expect("the backup this run could not remove is still there");
        assert_eq!(
            notices
                .iter()
                .filter(|line| line.contains("backup of the previous output"))
                .count(),
            1,
            "exactly one note about the surviving backup: {notices:?}"
        );
        assert!(
            notices
                .iter()
                .any(|line| line.contains(&backup.display().to_string())),
            "the note must name where the previous output now lives: {notices:?}"
        );
    }

    /// R0002-0032: two spellings of one destination collide up front with the
    /// clear sentence, instead of surviving the preflight and failing halfway
    /// through staging on the temp name they share. A final-component symlink
    /// is NOT such a pair: publication renames over the link entry, so those
    /// two commit as two independent files.
    #[test]
    fn duplicate_destinations_are_recognized_through_normalization() {
        let root = scratch_dir("duplicate-destinations");
        let deep = root.join("a");
        fs::create_dir(&deep).expect("an existing directory to climb out of");

        let pairs = [
            (
                root.join("out.md"),
                root.join("a").join("..").join("out.md"),
            ),
            (root.join("out.md"), root.join(".").join("out.md")),
        ];
        for (first, second) in pairs {
            let err = vet_destinations([first.as_path(), second.as_path()])
                .expect_err("two spellings of one file are one destination");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
        }

        #[cfg(unix)]
        {
            let linked = root.join("link");
            std::os::unix::fs::symlink(&deep, &linked).expect("symlink the parent");
            let err = vet_destinations([
                deep.join("out.md").as_path(),
                linked.join("out.md").as_path(),
            ])
            .expect_err("a symlinked parent still names one file");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");

            let alias = root.join("alias.md");
            std::os::unix::fs::symlink(root.join("out.md"), &alias).expect("symlink the file");
            vet_destinations([root.join("out.md").as_path(), alias.as_path()])
                .expect("a destination that IS a symlink publishes as its own file");
        }
    }

    /// R0004-0067: two destinations can name no file twice and still be one
    /// publication. `--output x --map x/y` needs `x` to be a regular file and
    /// a directory at once — it dies in `create_dir_all` when `x` exists, and
    /// in the phase-2 renames (the pass that cannot be rolled back) when it
    /// does not. Identity comparison had nothing to say about it, so the pair
    /// reached the commit through a paid translation run; R0002-0029 is the
    /// standing rule that a question the argv and the filesystem answer is
    /// answered before the provider call.
    #[test]
    fn a_destination_inside_another_destination_is_refused() {
        let root = scratch_dir("nested-destinations");
        let outer = root.join("x");
        let inner = outer.join("y.json");

        // Either order: the refusal is about the pair, not about which came
        // first.
        for pair in [[&outer, &inner], [&inner, &outer]] {
            let err = vet_destinations(pair.map(PathBuf::as_path))
                .expect_err("one destination is inside the other");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            let msg = err.to_string();
            assert!(
                msg.contains(&inner.display().to_string())
                    && msg.contains(&outer.display().to_string()),
                "the refusal has to name both destinations: {msg}"
            );
        }

        // Nesting through a deeper level, and through spellings that normalize
        // onto the same tree, is the same collision.
        vet_destinations([outer.as_path(), outer.join("a").join("b.json").as_path()])
            .expect_err("depth does not make it a different destination");
        vet_destinations([
            outer.as_path(),
            root.join(".").join("x").join("y.json").as_path(),
        ])
        .expect_err("the comparison is on normalized identities, not on spellings");

        // The ordinary set is not nested: the bundle's six files sit beside
        // the two individual outputs, and a name that merely shares a prefix
        // (`xy` under `x`) is a different directory entry.
        let mut ordinary = vec![root.join("out.md"), root.join("alignment.json")];
        ordinary.extend(html_bundle_paths(&root.join("html")));
        preflight_destination_set(&ordinary).expect("a bundle beside its outputs is not nested");
        vet_destinations([outer.as_path(), root.join("xy").as_path()])
            .expect("containment is component-wise, not textual");
    }

    /// R0004-0066: a `PathBuf` off the command line is arbitrary bytes on
    /// Unix, and every staging name transync builds is `<final component>`
    /// plus a suffix. The refusal used to arrive from `stage_one_file` — after
    /// the provider had been paid — and said "path has no file name" about a
    /// path that has one. Both modes ask it in the preflight now, and the late
    /// pass says the same sentence, because contracts.md §6 requires that an
    /// operator cannot tell the two passes apart.
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_final_component_is_refused_before_the_provider_call() {
        use std::os::unix::ffi::OsStrExt;
        let root = scratch_dir("non-utf8-destination");
        let bad = root.join(OsStr::from_bytes(b"out\xffmd"));

        for err in [
            preflight_destination_set(std::slice::from_ref(&bad))
                .expect_err("files mode asks before the run"),
            preflight_out_dir(&bad, false, &silent).expect_err("--out-dir asks before the run"),
            write_fileset_atomic(&[(bad.clone(), b"x".as_slice())], &silent)
                .expect_err("and the publication asks again"),
        ] {
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            assert!(
                err.to_string().contains("not valid UTF-8"),
                "the message must name the real cause: {err}"
            );
        }

        // The refusal is about the NAME, not about the bytes further up the
        // path: only the final component becomes a staging name.
        let dir = root.join(OsStr::from_bytes(b"d\xffir"));
        fs::create_dir(&dir).expect("a directory whose own name is not UTF-8");
        preflight_destination_set(&[dir.join("out.md")])
            .expect("a UTF-8 file name under a non-UTF-8 parent is publishable");
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

    /// R0003-0020, the out-dir half: staging `a/b/file` creates two levels and
    /// the writer used to record only the deeper one, leaving the entry naming
    /// `a/b` inside `a` unflushed. `staged_levels` is that recorded set.
    #[test]
    fn a_nested_staged_entry_records_every_level_it_created() {
        let staging = Path::new("/s/.out.staging.1.abc");
        assert_eq!(
            staged_levels(staging, &staging.join("a").join("b").join("file")),
            vec![staging.join("a"), staging.join("a").join("b")],
            "shallowest first, and the deepest is only the last of them"
        );
        assert!(
            staged_levels(staging, &staging.join("out.md")).is_empty(),
            "a payload directly in the staged root creates no level"
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

    /// The early destination preflight (R0002-0029) has to name the same six
    /// bundle files the bundle assembler writes, or it would check a set the
    /// publication does not commit.
    #[test]
    fn the_bundle_path_list_matches_the_files_the_bundle_writes() {
        let dir = Path::new("html");
        let written: Vec<PathBuf> =
            html_bundle_files(dir, "", "", b"{}", "t", "en", "ko", "", "", false)
                .into_iter()
                .map(|(path, _)| path)
                .collect();
        assert_eq!(
            html_bundle_paths(dir).iter().collect::<HashSet<_>>(),
            written.iter().collect::<HashSet<_>>(),
            "the preflight's paths and the bundle's files must be one list"
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

    /// What one publication reports back to the test that is racing it: that
    /// it has parked on a lock, or that it has finished.
    enum Step {
        Parked,
        Done(Result<(), String>),
    }

    /// ti `40e2a5`, the direction DCR-0021 recorded as a known boundary: an
    /// `--out-dir` replace and a files-mode publish *into* that same directory
    /// locked different inodes, so nothing serialized them — and the replace
    /// moves the target aside, so the files-mode run's committed output either
    /// vanished with the backup or landed in the replace's fresh tree.
    ///
    /// Deterministic, with the real lock and no sleeps: the test holds exactly
    /// the lock a files-mode publisher into the target holds, and the replace
    /// has to park on it. A replace that reports `Done` without ever parking is
    /// the defect, and reporting both outcomes on one channel is what makes
    /// that a failed assertion rather than a hang.
    #[test]
    fn an_out_dir_replace_waits_for_a_files_mode_publish_into_its_target() {
        let root = scratch_dir("nested-existing-target");
        let target = root.join("published");
        let owned = out_dir_payloads(b'A');
        let files: Vec<(PathBuf, &[u8])> = owned
            .iter()
            .map(|(p, b)| (p.clone(), b.as_slice()))
            .collect();
        publish_out_dir(&target, &files, false, &silent).expect("the target to be replaced");

        // What `write_fileset_atomic` holds while publishing into `target`.
        let peer = PublishLock::acquire(std::slice::from_ref(&target), &silent)
            .expect("the files-mode publisher's lock");

        let (step, steps) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let done = step.clone();
            let replaced = target.clone();
            let payload = &files;
            scope.spawn(move || {
                let announce = move |line: &str| {
                    if line.contains("waiting") {
                        let _ = step.send(Step::Parked);
                    }
                };
                let outcome = publish_out_dir(&replaced, payload, false, &announce)
                    .map_err(|e| e.to_string());
                let _ = done.send(Step::Done(outcome));
            });

            match steps.recv().expect("the replace reports something") {
                Step::Parked => {}
                Step::Done(_) => panic!(
                    "the --out-dir replace ran while a peer held its target: the two modes did \
                     not serialize"
                ),
            }
            drop(peer);
            match steps
                .recv()
                .expect("the replace finishes once the peer releases")
            {
                Step::Done(Ok(())) => {}
                Step::Done(Err(e)) => panic!("the queued replace must still succeed: {e}"),
                Step::Parked => panic!("the replace parked twice on one lock"),
            }
        });
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

    /// ti `40e2a5`, review fix round: the inner half of the lock set is a
    /// filesystem read, and until the lock is held it describes a moment that
    /// has passed. A replace that starts before its target exists snapshots an
    /// empty set; by the time it is granted the level holding the target, a peer
    /// has created the target and a run that started after that claims the
    /// target itself — the deepest level that exists on the way to its own
    /// destination. Without the re-read the replace holds none of it and renames
    /// the tree out from under that run.
    ///
    /// Deterministic, with the real lock and no sleeps: the test holds the level
    /// first (so the replace has to park on the snapshot it took), creates the
    /// target while the replace is queued, and takes the lock a run publishing
    /// into the target would take. A replace that reaches `Done` from there
    /// never re-read anything, and reporting both outcomes on one channel makes
    /// that a failed assertion rather than a hang.
    #[test]
    fn a_replace_re_reads_what_is_inside_its_target_once_it_holds_the_lock() {
        let root = scratch_dir("nested-late-target");
        let target = root.join("published");
        assert!(
            !target.exists(),
            "the snapshot has to be taken on a target that is not there yet"
        );
        let owned = out_dir_payloads(b'A');
        let files: Vec<(PathBuf, &[u8])> = owned
            .iter()
            .map(|(p, b)| (p.clone(), b.as_slice()))
            .collect();

        // All the replace can lock on the strength of an empty snapshot.
        let holder = PublishLock::acquire(&[root.to_path_buf()], &silent)
            .expect("the level holding the target");

        let (step, steps) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            let done = step.clone();
            let replaced = target.clone();
            let payload = &files;
            scope.spawn(move || {
                let announce = move |line: &str| {
                    if line.contains("waiting") {
                        let _ = step.send(Step::Parked);
                    }
                };
                let outcome = publish_out_dir(&replaced, payload, false, &announce)
                    .map_err(|e| e.to_string());
                let _ = done.send(Step::Done(outcome));
            });

            match steps.recv().expect("the replace reports something") {
                Step::Parked => {}
                Step::Done(_) => {
                    panic!("the replace never waited for the level holding its target")
                }
            }
            // The target appears while the replace is queued on the level above
            // it — a peer that created its destination before parking, or a bare
            // `mkdir` — and a run starting now claims the target itself.
            fs::create_dir(&target).expect("the target comes into existence");
            let inside = PublishLock::acquire(std::slice::from_ref(&target), &silent)
                .expect("what a run publishing into the target holds");
            drop(holder);

            match steps.recv().expect("the replace reports again") {
                Step::Parked => {}
                Step::Done(_) => panic!(
                    "the replace swapped a target a peer was publishing into: it never re-read \
                     what was inside it"
                ),
            }
            drop(inside);
            match steps
                .recv()
                .expect("the replace finishes once the peer releases")
            {
                Step::Done(Ok(())) => {}
                Step::Done(Err(e)) => panic!("the queued replace must still succeed: {e}"),
                Step::Parked => panic!("the replace parked once more than there were holders"),
            }
        });
        assert!(
            target.join(OUT_DIR_MARKER_NAME).is_file(),
            "the queued replace must still publish its own tree"
        );
    }

    /// The other half of the same lock set: what a replace claims *inside* its
    /// target is the shape a published out-dir has, and nothing else. A target
    /// that is not a directory is not something anybody publishes into — the
    /// swap renames the entry — and a symlink is judged as the link it is
    /// rather than as what it points at.
    #[test]
    fn a_replace_claims_the_directories_a_bundle_publishes_into() {
        let root = scratch_dir("published-dirs-inside");
        let target = root.join("published");
        assert!(
            published_dirs_inside(&target).is_empty(),
            "a target that does not exist has nothing inside it"
        );

        fs::create_dir(&target).expect("the target");
        assert_eq!(
            published_dirs_inside(&target),
            vec![target.clone()],
            "a bundle-less target is one directory"
        );

        let bundle = target.join(HTML_SUBDIR);
        fs::create_dir(&bundle).expect("the bundle directory");
        assert_eq!(
            published_dirs_inside(&target),
            vec![target.clone(), bundle],
            "html/ is the one nested directory a publish writes into"
        );

        let plain = root.join("regular-file");
        fs::write(&plain, b"not a directory").expect("a file target");
        assert!(
            published_dirs_inside(&plain).is_empty(),
            "a --force'd file target holds nothing"
        );

        #[cfg(unix)]
        {
            let linked = root.join("linked");
            std::os::unix::fs::symlink(&target, &linked).expect("symlink the target");
            assert!(
                published_dirs_inside(&linked).is_empty(),
                "the swap renames the link entry, so the linked-to tree is not claimed"
            );
        }
    }

    /// ti `cbbc4e`: the bundle allow-list is a list of **file** names. A
    /// directory wearing one carried an arbitrary subtree past the guard and
    /// into the backup `remove_dir_all` of a replace that needed no `--force` —
    /// the exact hazard the top level has refused since ti `66339b` for a
    /// directory named `out.md`, and the exact shape that reopens the nesting
    /// question, since a peer publishing inside it claims a level no replace
    /// holds.
    ///
    /// Both recognizers are checked, because both said yes: the bundle name
    /// itself, and the staging-temp name — with **our own** pid, which is the
    /// branch that would otherwise have tried to delete it in passing.
    #[test]
    fn a_directory_wearing_a_bundle_name_is_not_a_bundle_file() {
        let dir = scratch_dir("bundle-name-directory");
        for name in HTML_BUNDLE_ENTRIES {
            fs::write(dir.join(name), b"a real bundle file").expect("plant a bundle file");
        }
        ensure_html_out_safe(&dir, false, &silent).expect("the six files are the bundle");

        fs::remove_file(dir.join("index.html")).expect("make room for the directory");
        fs::create_dir(dir.join("index.html")).expect("a directory named index.html");
        fs::write(dir.join("index.html").join("notes.md"), b"somebody's work")
            .expect("nested user data");
        let err = ensure_html_out_safe(&dir, false, &silent)
            .expect_err("a directory named index.html hides nested data the guard cannot see");
        assert!(
            err.to_string().contains("index.html"),
            "the refusal must name the entry: {err}"
        );

        fs::remove_dir_all(dir.join("index.html")).expect("clear it");
        fs::write(dir.join("index.html"), b"a real bundle file").expect("restore the bundle file");
        let own_temp = dir.join(format!("index.html.tmp.{}", process::id()));
        fs::create_dir(&own_temp).expect("a directory at one of our own staging-temp names");
        let err = ensure_html_out_safe(&dir, false, &silent)
            .expect_err("this writer stages files, so a directory is not its residue");
        assert!(
            err.to_string().contains("index.html.tmp."),
            "the refusal must name the entry: {err}"
        );
        assert!(
            own_temp.is_dir(),
            "a directory at a staging-temp name must survive the scan that judged it"
        );
    }

    /// The same rule at the `--out-dir` top level, which recognizes staging
    /// temps through its own branch rather than through
    /// [`take_staging_temp`] (ti `cbbc4e`). Without the shape check the
    /// directory is read as this run's own residue, the target passes as an
    /// empty one, and the replace renames the subtree away.
    #[test]
    fn a_directory_wearing_a_staging_temp_name_is_not_a_staging_temp() {
        let target = scratch_dir("top-level-temp-directory");
        let masquerading = target.join(format!("out.md.tmp.{}", process::id()));
        fs::create_dir(&masquerading).expect("a directory at our own staging-temp name");
        fs::write(masquerading.join("notes.md"), b"somebody's work").expect("nested user data");

        let err = ensure_out_dir_replaceable(&target, false, &silent)
            .expect_err("a directory at a staging-temp name is not transync residue");
        assert!(
            err.to_string().contains("out.md.tmp."),
            "the refusal must name the entry: {err}"
        );
    }

    /// ti `cbbc4e`: the replaceability guard is a check on a snapshot, and
    /// between that snapshot and the rename acting on it sits the whole staging
    /// phase — the entire bundle written and fsynced. The publish lock does not
    /// hold that verdict still: the two writers that can put a directory inside
    /// the target during the window take no lock at all (a bare `mkdir`) or take
    /// one only *after* creating their levels (a files-mode peer, which then
    /// parks on the level this replace holds while the levels it made sit
    /// unheld). Restating the guard with the staged tree in hand is what stops
    /// the replace renaming that away.
    ///
    /// Deterministic, single-threaded, no sleeps. The injection point is the
    /// foreign-staging-temp notice, which `ensure_out_dir_replaceable` emits
    /// **after** it has finished reading `html/` and before it judges the top
    /// level — so a directory created from that callback is provably invisible
    /// to the first pass and provably visible to the second. Without the second
    /// pass this run succeeds and the directory is gone.
    #[test]
    fn a_replace_asks_the_guard_again_with_the_staged_tree_in_hand() {
        let root = scratch_dir("guard-restated");
        let target = root.join("published");
        let bundle = target.join(HTML_SUBDIR);
        fs::create_dir_all(&bundle).expect("the bundle directory");
        fs::write(target.join(OUT_DIR_MARKER_NAME), OUT_DIR_MARKER_BODY).expect("the marker");
        for name in HTML_BUNDLE_ENTRIES {
            fs::write(bundle.join(name), b"the previous publication").expect("a bundle file");
        }
        // A peer's staging leftover: preserved, reported, and — for this test —
        // the callback that fires between the two halves of the first pass.
        fs::write(
            bundle.join("index.html.tmp.2147483646"),
            b"a peer's staging",
        )
        .expect("the foreign temp");

        let appeared = bundle.join("appeared");
        let announce = |line: &str| {
            if line.contains("staging temp(s) from other transync runs") {
                let _ = fs::create_dir(&appeared);
            }
        };

        let owned = out_dir_payloads(b'N');
        let files: Vec<(PathBuf, &[u8])> = owned
            .iter()
            .map(|(p, b)| (p.clone(), b.as_slice()))
            .collect();
        let err = publish_out_dir(&target, &files, false, &announce)
            .expect_err("a target that grew a directory while staging is not the judged target");
        assert!(
            err.to_string().contains("while this run was staging"),
            "the refusal must say when the target changed: {err}"
        );
        assert!(
            appeared.is_dir(),
            "the directory that appeared must survive the replace that refused"
        );
        assert_eq!(
            fs::read(bundle.join("index.html")).ok(),
            Some(b"the previous publication".to_vec()),
            "a refused replace leaves the previous publication in place"
        );
        assert!(
            !out_dir_sibling_exists(&root),
            "the staged tree must be removed when the second pass refuses"
        );
    }

    /// Whether any `.<name>.staging.<pid>.<token>` / `.<name>.backup.…` sibling
    /// is left in `parent` — publication residue a failed run must not leave.
    fn out_dir_sibling_exists(parent: &Path) -> bool {
        fs::read_dir(parent)
            .expect("read the level holding the target")
            .flatten()
            .any(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.contains(".staging.") || name.contains(".backup."))
            })
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
}
