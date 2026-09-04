//! The `--out-dir` directory publication: [`publish_out_dir`] stages a whole
//! output tree under a per-run random token, publishes it by rename, and
//! carries the ownership marker that tells the next run this directory is
//! transync's to replace.
//!
//! Lifted out of `output.rs` unchanged by OI-0043; the two orderings that
//! bound the nested-claim gap are stated in the parent module's
//! documentation.
//!
//! TRACE: EXT-2026-07 P1-6
//! TRACE: DCR-0021

use super::destination::destination_file_name;
use super::fileset::{claim_anchor, fsync_created_levels, missing_ancestors};
use super::lock::PublishLock;
use super::preflight::ensure_out_dir_replaceable;
use super::{
    AFTER_A_FAILED_PUBLICATION, BEFORE_THIS_PUBLICATION, Notify, OUT_DIR_MARKER_NAME, fsync_dir,
    note_cleanup_residue,
};
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::hash::{BuildHasher, Hasher, RandomState};
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::process;

/// The subdirectory of an `--out-dir` target that holds the demo bundle. The
/// one nested directory a published out-dir has, which is why it is also the
/// one the guard recurses into and the one a replace locks (ti `40e2a5`).
pub(super) const HTML_SUBDIR: &str = "html";

/// The top-level entries a `--out-dir` target is allowed to contain and
/// still be treated as a prior transync out-dir (safe to replace without
/// `--force`). Any other entry marks the directory as foreign.
///
/// Being *allowed* is not the same as being *ours*: the allow-list bounds what
/// may sit beside a published set, while [`OUT_DIR_MARKER_NAME`] is what says
/// transync published it. See [`ensure_out_dir_replaceable`].
///
/// EXT-2026-07 P1-6
/// `out.md` and `out.html` are **alternatives** — a run writes exactly one —
/// which is why the complete-set fallback in
/// [`ensure_out_dir_replaceable`] is a predicate, not a count (ti `490d97`
/// wave 6).
pub(super) const OUT_DIR_ENTRIES: &[&str] = &[
    "out.md",
    "out.html",
    "alignment.json",
    "validation-report.json",
    HTML_SUBDIR,
];

/// What the ownership marker says, for the operator who finds one. The file
/// is read by nobody — its *presence* is the whole signal — so the body is
/// there to answer "what is this and what happens if I delete it?" in the
/// place the question gets asked.
pub(super) const OUT_DIR_MARKER_BODY: &[u8] = b"transync --out-dir publication marker.\n\
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
/// `super::preflight::take_staging_temp` were the two places not to draw.
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
/// [`super::preflight_out_dir`] too, so a target this writer cannot name costs no
/// provider call.
pub(super) fn out_dir_name(target: &Path) -> io::Result<&str> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::testing::scratch_dir;
    use crate::output::tests::{Step, out_dir_payloads, silent};
    use std::cell::RefCell;

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
}
