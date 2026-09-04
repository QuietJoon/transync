//! The pre-publication guards: may this run write its bundle into that
//! directory ([`preflight_html_out`]), may it replace that `--out-dir` target
//! ([`preflight_out_dir`]), is its destination set free of duplicates and
//! nesting ([`preflight_destination_set`]) — plus the directory reading the
//! first two are decided by, which distinguishes this tool's own output and
//! staging residue from content it did not write.
//!
//! Lifted out of `output.rs` unchanged by OI-0043.
//!
//! TRACE: contracts.md §6
//! TRACE: DCR-0011

use super::destination::vet_destinations;
use super::fileset::tmp_pid_suffix;
use super::lock::PUBLISH_LOCK_NAME;
use super::publish::{HTML_SUBDIR, OUT_DIR_ENTRIES, out_dir_name};
use super::{BEFORE_THIS_PUBLICATION, Notify, OUT_DIR_MARKER_NAME, note_cleanup_residue};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

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
/// which is why [`super::publish_out_dir`] asks again under the publication lock and
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
/// [`super::html_bundle_files`] produces, without the payloads. The early
/// destination preflight (R0002-0029) needs the paths before there is any
/// content to put in them.
pub fn html_bundle_paths(dir: &Path) -> Vec<PathBuf> {
    HTML_BUNDLE_ENTRIES.iter().map(|e| dir.join(e)).collect()
}

/// Reject a destination set that names one file twice, nests one destination
/// inside another, or names a file this writer cannot build a staging name
/// for, before anything is written (R0008-0005, R0004-0066, R0004-0067) — and,
/// for a CLI run, before the provider is called (R0002-0029).
/// [`super::write_fileset_atomic`] repeats the checks on the set it is actually
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
pub(super) const HTML_BUNDLE_ENTRIES: &[&str] = &[
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
/// writer (see [`super::write_fileset_atomic`]) for one of the `expected` destination
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
pub(super) fn ensure_out_dir_replaceable(
    target: &Path,
    force: bool,
    notify: Notify<'_>,
) -> io::Result<()> {
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
    // ti 490d97 wave 6: a publication writes ONE translated document —
    // out.md or out.html, never both — so "the complete published set" is
    // the three fixed members plus at least one document file. The old
    // len() == OUT_DIR_ENTRIES.len() equality would be unsatisfiable with
    // five allow-listed names and silently kill the ti-66339b marker-less
    // recovery.
    let complete_set = published.contains("alignment.json")
        && published.contains("validation-report.json")
        && published.contains(HTML_SUBDIR)
        && (published.contains("out.md") || published.contains("out.html"));
    if owned || published.is_empty() || complete_set {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::bundle::html_bundle_files;
    use crate::output::publish::{OUT_DIR_MARKER_BODY, publish_out_dir};
    use crate::output::testing::scratch_dir;
    use crate::output::tests::{out_dir_payloads, silent};
    use std::cell::RefCell;
    use std::collections::HashSet;

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
}
