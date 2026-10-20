//! Destination vetting and lexical path normalization: the refusals
//! [`vet_destinations`] makes up front — before anything is created, staged
//! or locked — and the normalization that lets two spellings of one path be
//! recognized as the same destination.
//!
//! Lifted out of `output.rs` unchanged by OI-0043.
//!
//! TRACE: contracts.md §6

use super::lock::{PUBLISH_LOCK_NAME, is_publish_lock_marker, is_publish_lock_name};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Every question about a destination **set** that the arguments and the
/// filesystem answer on their own, asked before any I/O (R0008-0005) and — for
/// a CLI run — before the provider is called (R0002-0029).
///
/// Four refusals, all of them cheap and all of them true before the run
/// started:
///
/// * **A name this writer cannot spell** ([`destination_file_name`]). The
///   staging names are built from the final path component, so a component
///   that is not UTF-8 fails at staging time; asking here is what keeps that
///   from being a discovery made after the provider has been paid
///   (R0004-0066).
/// * **The publication-lock marker** (R0012-0005). No component of a
///   destination may take [`PUBLISH_LOCK_NAME`] — by name in any ASCII case
///   ([`refuse_reserved_names`], asked inside [`destination_file_name`]), or by
///   being that file under a spelling the filesystem folds further
///   ([`refuse_lock_marker_alias`]). Publishing onto the marker replaces the
///   inode the run's own lock is held on. The identity half is asked again by
///   the publication once it has created its levels and once it holds its
///   locks, because a marker can come into existence after this pass.
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
pub(super) fn vet_destinations<'a>(paths: impl IntoIterator<Item = &'a Path>) -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    // A list rather than a set: containment needs every pair, and a fileset is
    // at most nine destinations (two outputs, a report, the six-file bundle).
    let mut seen: Vec<(PathBuf, &Path)> = Vec::new();
    for path in paths {
        destination_file_name(path)?;
        refuse_lock_marker_alias(path)?;
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
/// `.<name>.staging.<pid>.<token>` siblings under `--out-dir`) — once the
/// path has been cleared of the publication-lock marker's reserved name
/// ([`refuse_reserved_names`]).
///
/// A `PathBuf` off the command line is arbitrary bytes on Unix, and nothing
/// upstream requires UTF-8 of it. Before R0004-0066 the first refusal was
/// `super::fileset::stage_one_file`'s, i.e. after the translation had been paid for, and it
/// said "path has no file name" about a path that has one. Asked in the
/// destination preflight and again where the name is actually built, with the
/// same sentence either way, so an operator cannot tell the two passes apart.
/// That is also why the reserved name is refused here rather than beside it:
/// every pass of both modes, and the staging itself, asks this function.
pub(super) fn destination_file_name(path: &Path) -> io::Result<&str> {
    let Some(name) = path.file_name() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "output destination {} has no final path component",
                path.display()
            ),
        ));
    };
    refuse_reserved_names(path)?;
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

/// Refuse a destination any of whose path components is
/// [`PUBLISH_LOCK_NAME`] in any ASCII letter case (R0012-0005).
///
/// The **final** component is the defect the finding names. A run locks the
/// marker in the directory it publishes into and then renames its staged
/// output onto the same entry, replacing the very inode its lock is held on;
/// the next run opens and locks the replacement while this one still has
/// renames to do, and two "exclusive" publishers share one directory. Under
/// `--out-dir --force` the swap moves the locked marker aside and leaves a
/// directory at its path, after which no publication into the parent can open
/// its lock at all.
///
/// The **other** components matter because the run creates the missing ones.
/// A level it makes at the marker's name breaks the level holding it the same
/// way — and when that holding level is itself being created, nothing in the
/// run locks it, so `--output X/new/.transync-publish.lock/out.md` used to
/// succeed and leave `X/new` unable to be published into again. A component
/// that already exists under the name is refused too: a regular file there
/// fails the publication after the provider has been paid, and a directory
/// there is the same broken state an earlier run should never have made.
fn refuse_reserved_names(path: &Path) -> io::Result<()> {
    let Some(reserved) = path.components().find_map(|part| match part {
        Component::Normal(name) if is_publish_lock_name(name) => Some(name),
        _ => None,
    }) else {
        return Ok(());
    };
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "output destination {} uses {reserved:?}, the publication-lock marker's name (in \
             any letter case); transync locks that marker in every directory it publishes \
             into, so no output, and no directory on the way to one, may take it — choose \
             another name",
            path.display()
        ),
    ))
}

/// The backstop behind [`refuse_reserved_names`] (R0012-0005): refuse a
/// destination when it, or any directory level on the way to it, **is** the
/// publication-lock marker of the level holding it by filesystem identity
/// ([`is_publish_lock_marker`]) — the spellings a volume folds beyond ASCII,
/// which no comparison of names can enumerate.
///
/// Identity is a fact about the filesystem *now*, so the answer depends on when
/// it is asked, and the callers ask it at each point it can change: before
/// anything is created (a marker a prior publication left), once the run has
/// created its levels and before it locks (a level that landed on the marker
/// path of the level holding it — locking that level would open a directory),
/// and once the locks are held (a destination that names nothing until the
/// lock creates the marker it then resolves to). Each pass is a handful of
/// `lstat`s per level.
pub(super) fn refuse_lock_marker_alias(path: &Path) -> io::Result<()> {
    let Some(level) = path
        .ancestors()
        .find(|level| !level.as_os_str().is_empty() && is_publish_lock_marker(level))
    else {
        return Ok(());
    };
    let subject = if level == path {
        format!("output destination {}", path.display())
    } else {
        format!(
            "output destination {} runs through {}, which",
            path.display(),
            level.display()
        )
    };
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "{subject} is the publication-lock marker {PUBLISH_LOCK_NAME} under another name (the \
             filesystem resolves both to one file); transync locks that marker in every \
             directory it publishes into, so no output, and no directory on the way to one, may \
             take it — choose another name"
        ),
    ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::fileset::write_fileset_atomic;
    use crate::output::lock::PUBLISH_LOCK_NAME;
    use crate::output::preflight::{
        html_bundle_paths, preflight_destination_set, preflight_out_dir,
    };
    use crate::output::publish::publish_out_dir;
    use crate::output::testing::scratch_dir;
    use crate::output::tests::silent;
    use std::ffi::OsStr;

    /// The refusal every reserved-name case has to produce: an argument
    /// fault (`InvalidInput`, so exit 4 like every destination refusal) that
    /// names the lock marker as the reason.
    fn assert_lock_name_refusal(pass: &str, outcome: io::Result<()>) {
        let err = outcome.expect_err(pass);
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{pass}: {err}");
        assert!(
            err.to_string().contains("publication-lock marker"),
            "{pass}: the refusal must say the name is the lock marker's: {err}"
        );
    }

    /// The payload an `--out-dir` publication is handed in these tests.
    fn one_file() -> Vec<(PathBuf, &'static [u8])> {
        vec![(PathBuf::from("out.md"), b"translated".as_slice())]
    }

    /// R0012-0005: `.transync-publish.lock` is the marker every publication
    /// locks in each directory it writes into, and a destination of that name
    /// made the run rename its own output over the inode it held — after which
    /// the next run opened and locked the *new* inode while this one still had
    /// renames to do: two "exclusive" publishers in one directory. Under
    /// `--out-dir`, `--force` additionally left a directory at the marker path
    /// and every later publication into the parent failed. The name is refused
    /// in both modes, in the pass before the provider call and again in the
    /// publication itself, ASCII case-insensitively (a case-insensitive volume
    /// resolves `.TRANSYNC-PUBLISH.LOCK` to the marker's own entry), and
    /// `--force` does not waive it.
    #[test]
    fn a_destination_named_like_the_publish_lock_is_refused_in_every_pass() {
        let root = scratch_dir("reserved-lock-name");
        let marker = root.join(PUBLISH_LOCK_NAME);
        fs::write(&marker, b"").expect("the marker a prior publication left");
        #[cfg(unix)]
        let before = {
            use std::os::unix::fs::MetadataExt;
            fs::metadata(&marker).expect("the marker").ino()
        };
        let map = root.join("map.json");
        let fresh = scratch_dir("reserved-lock-name-target");

        for name in [PUBLISH_LOCK_NAME, ".TRANSYNC-PUBLISH.LOCK"] {
            let dest = root.join(name);
            assert_lock_name_refusal(
                "the files-mode preflight",
                preflight_destination_set(&[dest.clone(), map.clone()]),
            );
            assert_lock_name_refusal(
                "the files-mode publication",
                write_fileset_atomic(
                    &[
                        (dest.clone(), b"translated".as_slice()),
                        (map.clone(), b"{}".as_slice()),
                    ],
                    &silent,
                ),
            );
            let out_dir = fresh.join(name);
            assert_lock_name_refusal(
                "the --out-dir preflight",
                preflight_out_dir(&out_dir, true, &silent),
            );
            assert_lock_name_refusal(
                "the --out-dir publication, forced",
                publish_out_dir(&out_dir, &one_file(), true, &silent),
            );
        }

        let meta = fs::symlink_metadata(&marker).expect("the marker is still there");
        assert!(
            meta.is_file() && meta.len() == 0,
            "the marker must still be the zero-byte lock file"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            assert_eq!(
                meta.ino(),
                before,
                "and the same inode the locks were taken on"
            );
        }
        assert!(
            !map.exists(),
            "a refused fileset publishes none of its members"
        );
        assert!(
            !fresh.join(PUBLISH_LOCK_NAME).exists(),
            "the --out-dir refusal comes before anything is created or locked"
        );

        // The reservation is the whole name, not a substring: an output that
        // merely contains it is an ordinary destination, and publishes.
        let neighbours = [
            root.join(".transync-publish.lock.md"),
            root.join("notes.transync-publish.lock"),
        ];
        write_fileset_atomic(
            &[
                (neighbours[0].clone(), b"translated".as_slice()),
                (neighbours[1].clone(), b"{}".as_slice()),
            ],
            &silent,
        )
        .expect("a name that only contains the marker's name is not the marker's name");
        assert_eq!(
            fs::read(&marker).ok(),
            Some(Vec::new()),
            "publishing beside the marker leaves it the zero-byte lock file"
        );
    }

    /// The same name on a directory a run would have to **create** on the way
    /// to a destination (the skeptic's case on R0012-0005). With `X/new`
    /// missing, `--output X/new/.transync-publish.lock/out.md` made the
    /// directory, locked it and `X` — never `X/new` itself — and exited 0,
    /// leaving a directory at `X/new`'s marker path: every later publication
    /// into `X/new` failed. Where the parent existed it failed after the
    /// provider was paid instead, when the lock on the parent opened a
    /// directory. Both modes, both passes, and `--html-out` (whose six
    /// destinations sit under the directory it names).
    #[test]
    fn a_directory_on_the_way_to_a_destination_cannot_take_the_lock_name() {
        let root = scratch_dir("reserved-lock-ancestor");
        let map = root.join("map.json");
        let nested = root.join("new").join(PUBLISH_LOCK_NAME).join("out.md");
        let out_dir = root.join("deep").join(".Transync-Publish.Lock").join("sub");

        assert_lock_name_refusal(
            "the files-mode preflight",
            preflight_destination_set(&[nested.clone(), map.clone()]),
        );
        assert_lock_name_refusal(
            "an --html-out named like the marker",
            preflight_destination_set(&html_bundle_paths(&root.join(PUBLISH_LOCK_NAME))),
        );
        assert_lock_name_refusal(
            "the files-mode publication",
            write_fileset_atomic(
                &[
                    (nested, b"translated".as_slice()),
                    (map.clone(), b"{}".as_slice()),
                ],
                &silent,
            ),
        );
        assert_lock_name_refusal(
            "the --out-dir preflight",
            preflight_out_dir(&out_dir, false, &silent),
        );
        assert_lock_name_refusal(
            "the --out-dir publication",
            publish_out_dir(&out_dir, &one_file(), false, &silent),
        );

        assert!(
            !root.join("new").exists() && !root.join("deep").exists(),
            "a refused destination creates no directory level"
        );
        assert!(
            !map.exists(),
            "a refused fileset publishes none of its members"
        );
        assert!(
            !root.join(PUBLISH_LOCK_NAME).exists(),
            "nothing was locked, so nothing made a marker"
        );
    }

    /// R0012-0005's backstop: a name is not the only way to reach the marker.
    /// Whatever spelling the filesystem resolves to the marker's own file is
    /// that file, so the check is by identity (`dev`+`ino`), not by text. A
    /// hard link is the portable way to give the marker a second name; it is
    /// refused because it IS the lock file, which no output is.
    #[cfg(unix)]
    #[test]
    fn a_destination_that_is_the_lock_marker_by_identity_is_refused() {
        let root = scratch_dir("lock-marker-identity");
        let marker = root.join(PUBLISH_LOCK_NAME);
        fs::write(&marker, b"").expect("the marker a prior publication left");
        let alias = root.join("alias.md");
        fs::hard_link(&marker, &alias).expect("a second name for the marker");
        let map = root.join("map.json");

        assert_lock_name_refusal(
            "the files-mode preflight",
            preflight_destination_set(&[alias.clone(), map.clone()]),
        );
        assert_lock_name_refusal(
            "the files-mode publication",
            write_fileset_atomic(
                &[
                    (alias.clone(), b"translated".as_slice()),
                    (map.clone(), b"{}".as_slice()),
                ],
                &silent,
            ),
        );
        assert_lock_name_refusal(
            "the --out-dir preflight",
            preflight_out_dir(&alias, true, &silent),
        );
        assert_lock_name_refusal(
            "the --out-dir publication, forced",
            publish_out_dir(&alias, &one_file(), true, &silent),
        );

        let meta = fs::symlink_metadata(&marker).expect("the marker is still there");
        assert!(meta.is_file() && meta.len() == 0, "the marker is untouched");
        assert!(
            !map.exists(),
            "a refused fileset publishes none of its members"
        );
    }

    /// Spellings that some filesystem resolves to the marker's own directory
    /// entry although the ASCII fold does not see them: HFS+ drops a zero-width
    /// non-joiner from the comparison, case-insensitive APFS folds the KELVIN
    /// SIGN to `k`, and APFS and NTFS fold the LONG S to `s`.
    const FOLDED_LOCK_SPELLINGS: &[&str] = &[
        ".transync-publish.lock\u{200C}",
        ".transync-publish.loc\u{212A}",
        ".tran\u{17F}ync-publish.lock",
    ];

    /// The first of [`FOLDED_LOCK_SPELLINGS`] that the filesystem under `dir`
    /// resolves to a marker, asked of the filesystem itself — the property
    /// belongs to the volume, not to the target triple.
    fn folded_lock_spelling(dir: &Path) -> Option<&'static str> {
        let probe = dir.join("fold-probe");
        fs::create_dir(&probe).expect("a probe directory");
        fs::write(probe.join(PUBLISH_LOCK_NAME), b"").expect("a probe marker");
        let found = FOLDED_LOCK_SPELLINGS
            .iter()
            .copied()
            .find(|spelling| fs::symlink_metadata(probe.join(spelling)).is_ok());
        fs::remove_dir_all(&probe).expect("remove the probe");
        found
    }

    /// The identity backstop has to run **under the lock**, not only up front.
    /// Before a directory has ever been published into it holds no marker, so
    /// a folded spelling names nothing for the preflight to compare; it is the
    /// lock that creates the marker, and the rename that would then replace it.
    /// The same holds for a directory a run creates on the way: once created,
    /// it IS the marker path of the level holding it.
    ///
    /// Runs wherever the scratch volume folds one of the spellings (HFS+ and
    /// case-insensitive APFS both do); elsewhere it says so rather than
    /// reporting coverage it never had.
    #[cfg(unix)]
    #[test]
    fn a_spelling_the_filesystem_folds_onto_the_lock_marker_is_refused_under_the_lock() {
        let root = scratch_dir("folded-lock-spelling");
        let Some(spelling) = folded_lock_spelling(&root) else {
            eprintln!(
                "note: {} folds none of the probed spellings onto the marker; the folded-alias \
                 half of R0012-0005's backstop did not run on this filesystem",
                root.display()
            );
            return;
        };
        let marker_in = |dir: &Path| {
            let meta = fs::symlink_metadata(dir.join(PUBLISH_LOCK_NAME))
                .expect("the lock marker is there");
            meta.is_file() && meta.len() == 0
        };

        let files = root.join("files");
        fs::create_dir(&files).expect("a directory never published into");
        let dest = files.join(spelling);
        let map = files.join("map.json");
        preflight_destination_set(&[dest.clone(), map.clone()])
            .expect("before the lock there is no marker for the spelling to name");
        assert_lock_name_refusal(
            "the files-mode publication",
            write_fileset_atomic(
                &[
                    (dest, b"translated".as_slice()),
                    (map.clone(), b"{}".as_slice()),
                ],
                &silent,
            ),
        );
        assert!(marker_in(&files), "the marker must still be the lock file");
        assert!(
            !map.exists(),
            "a refused fileset publishes none of its members"
        );

        let holder = root.join("out-dir");
        fs::create_dir(&holder).expect("the level holding the target");
        assert_lock_name_refusal(
            "the --out-dir publication, forced",
            publish_out_dir(&holder.join(spelling), &one_file(), true, &silent),
        );
        assert!(marker_in(&holder), "the swap must not take the marker away");

        let level = root.join("levels");
        fs::create_dir(&level).expect("the deepest level that exists");
        assert_lock_name_refusal(
            "a created level, files mode",
            write_fileset_atomic(
                &[
                    (
                        level.join("new").join(spelling).join("out.md"),
                        b"translated".as_slice(),
                    ),
                    (level.join("map.json"), b"{}".as_slice()),
                ],
                &silent,
            ),
        );
        assert_lock_name_refusal(
            "a created level, --out-dir",
            publish_out_dir(
                &level.join("deeper").join(spelling).join("sub"),
                &one_file(),
                false,
                &silent,
            ),
        );
        assert!(
            !level.join("new").exists() && !level.join("deeper").exists(),
            "the levels a refused run created are rolled back"
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
        //
        // This half needs the filesystem to actually *hold* a directory whose
        // own name is not UTF-8, and that is a property of the filesystem
        // rather than of the platform: ext4 stores arbitrary bytes, while APFS
        // validates names and refuses this one with `EILSEQ`. `cfg(unix)`
        // cannot express that difference, so ask the filesystem instead of
        // asking the target triple.
        let dir = root.join(OsStr::from_bytes(b"d\xffir"));
        match fs::create_dir(&dir) {
            Ok(()) => {
                preflight_destination_set(&[dir.join("out.md")])
                    .expect("a UTF-8 file name under a non-UTF-8 parent is publishable");
            }
            Err(refusal) => {
                // Tell "this filesystem cannot spell that name" apart from a
                // scratch root that is missing or read-only: without this the
                // arm would swallow a real environment failure and report
                // coverage it never ran. A UTF-8 sibling must still succeed.
                fs::create_dir(root.join("utf8-sibling"))
                    .expect("the scratch root is writable, so the refusal above is about the NAME");
                eprintln!(
                    "note: {} refused a non-UTF-8 directory name ({refusal}); the \
                     non-UTF-8-PARENT half of this case did not run on this filesystem",
                    root.display()
                );
            }
        }
    }
}
