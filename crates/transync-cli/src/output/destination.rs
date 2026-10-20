//! Destination vetting and entry identity: the refusals [`vet_destinations`]
//! makes up front — before anything is created, staged or locked — and the
//! identity ([`EntryIdentity`]) that lets two spellings of one directory entry
//! be recognized as the same destination.
//!
//! "Two spellings of one entry" is the filesystem's question, not a textual
//! one. `.` and `..`, a symlinked parent, and — on a volume that folds them —
//! letter case and Unicode normalization all name one entry under several
//! byte strings (R0002-0032, R0012-0002). The identity answers it by asking
//! the filesystem: the existing part of the parent is resolved, the parent is
//! also held by `dev`+`ino`, and the final component is the name its directory
//! **stores**. Nothing is lowercased: on a case-sensitive volume `a.md` and
//! `A.md` are two files.
//!
//! TRACE: contracts.md §6

use super::fileset::claim_anchor;
use super::lock::{PUBLISH_LOCK_NAME, is_publish_lock_marker, is_publish_lock_name};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::process;

/// Every question about a destination **set** that the arguments and the
/// filesystem answer on their own, asked before anything is written or locked
/// (R0008-0005) and — for a CLI run — before the provider is called
/// (R0002-0029). Asking them reads the filesystem (it resolves parents, lists
/// the directories holding existing destinations, and probes the staging
/// names); it never writes to it. The publication asks them all again once it
/// has created the missing directory levels, before it stages or locks
/// anything (`super::fileset::write_fileset_atomic`): a level that exists
/// resolves under every spelling the volume folds onto it, which a name that
/// does not exist yet cannot.
///
/// Six refusals, all of them cheap and all of them true before the run
/// started:
///
/// * **A name this writer cannot spell** ([`destination_file_name`]). The
///   staging names are built from the final path component, so a component
///   that is not UTF-8 fails at staging time; asking here is what keeps that
///   from being a discovery made after the provider has been paid
///   (R0004-0066).
/// * **A staging name the filesystem refuses** (R0013-0006,
///   [`refuse_unspellable_staging_name`]). A final component the volume
///   accepts can still be too long once `.tmp.<pid>` is appended; the volume
///   is asked, read-only, about the very name [`staging_temp_path`] builds.
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
///   wrong fileset. Sameness is [`EntryIdentity`]'s comparison rather than a
///   textual one: `out.md`, `./out.md`, `a/../out.md` and a path through a
///   symlinked parent all name one file (R0002-0032), and so do `out.md` and
///   `OUT.md`, or the two Unicode normalization forms of one name, on a volume
///   that folds them (R0012-0002) — and the operator gets that sentence
///   instead of the late, confusing "refusing existing temp" the colliding
///   `<dest>.tmp.<pid>` names would produce halfway through staging. Where
///   neither spelling exists yet there is no stored name to compare, and the
///   identity asks the volume, read-only, whether it folds ASCII letter case;
///   a pair that differs any other way is left to the publication, which
///   refuses it with this sentence once the first temp exists.
/// * **One destination inside another** (R0004-0067). `--output x --map x/y`
///   names no file twice, so identity comparison passed it — and then the
///   publication needs `x` to be a regular file and a directory at once. It
///   fails in `create_dir_all` before staging when `x` already exists, and in
///   the phase-2 renames when it does not, which is the one failure this
///   commit cannot roll back. It is a question about the argv and the
///   filesystem alone, and R0002-0029 is the rule that such questions are
///   asked before the provider call, not after it. Containment compares the
///   same identities as sameness, so where neither the outer destination nor
///   the inner one's parent exists, two spellings of that level compare under
///   ASCII case only as above; one the volume folds another way (`café/y.md`
///   beside `café` in the other normalization form) is refused by the
///   publication's second pass, once it has created the level and before it
///   stages anything.
/// * **A destination in another destination's staging namespace**
///   (R0013-0001). Destination `out.json.tmp.<pid>` beside destination
///   `out.json` is the second's staging name: both temps are absent when
///   `create_new` opens them, the first rename replaces the second's temp, and
///   the second rename publishes the first's bytes — exit 0, one output gone
///   and the other wrong. Refused in either order (the reverse one happened
///   to publish correctly; the namespace is reserved all the same), and so is
///   a destination *inside* a staging name, which used to fail late in
///   `create_new`. Exact to this run's pid: `out.json.tmp.0` is an ordinary
///   name. The publication asks the filesystem again once its levels exist and
///   once its temps exist (`super::fileset::write_fileset_atomic`), for the
///   spellings no comparison made here can see.
pub(super) fn vet_destinations<'a>(paths: impl IntoIterator<Item = &'a Path>) -> io::Result<()> {
    let cwd = std::env::current_dir()?;
    // A list rather than a set: containment needs every pair, and a fileset is
    // at most nine destinations (two outputs, a report, the six-file bundle).
    let mut seen: Vec<Vetted<'a>> = Vec::new();
    for path in paths {
        // Asks `destination_file_name` first, so a name this writer cannot
        // spell is refused with that sentence before anything else is asked.
        let staging = staging_temp_path(path)?;
        refuse_lock_marker_alias(path)?;
        refuse_unspellable_staging_name(path, &staging)?;
        let identity = EntryIdentity::of(&cwd, path);
        for other in &seen {
            if identity.same_entry(&other.identity) {
                return Err(duplicate_destination(path, other.path));
            }
            let (inner, outer) = if identity.is_inside(&other.identity) {
                (path, other.path)
            } else if other.identity.is_inside(&identity) {
                (other.path, path)
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
        seen.push(Vetted {
            path,
            identity,
            staging: EntryIdentity::of(&cwd, &staging),
        });
    }
    // The staging namespace, once every name in the set is known to be one
    // this writer can spell.
    for owner in &seen {
        for other in &seen {
            if std::ptr::eq(owner, other) {
                continue;
            }
            if other.identity.same_entry(&owner.staging) {
                return Err(staging_name_collision(other.path, owner.path));
            }
            if other.identity.is_inside(&owner.staging) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "output destination {} is inside {}, the staging name transync uses \
                         while publishing output destination {} ({}): choose another name",
                        other.path.display(),
                        owner.staging.path.display(),
                        owner.path.display(),
                        staging_scheme(),
                    ),
                ));
            }
        }
    }
    Ok(())
}

/// One destination [`vet_destinations`] has accepted so far: the path as the
/// caller spelled it (what every refusal names), the entry it identifies, and
/// the entry its staging temp will be.
struct Vetted<'a> {
    path: &'a Path,
    identity: EntryIdentity,
    staging: EntryIdentity,
}

/// The duplicate-destination refusal: `path` and `other` name one file.
/// Shared by [`vet_destinations`] and the publication's own late check, so the
/// operator reads one sentence whichever pass found it.
pub(super) fn duplicate_destination(path: &Path, other: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "duplicate output destination: {} names the same file as {}",
            path.display(),
            other.display()
        ),
    )
}

/// The staging-namespace refusal (R0013-0001): `destination` is the staging
/// name of `owner`. Shared by [`vet_destinations`] and the publication's check
/// after staging, which finds the spellings no name comparison can.
pub(super) fn staging_name_collision(destination: &Path, owner: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "output destination {} is the staging name transync uses while publishing output \
             destination {} ({}): publishing both would overwrite one output with the other's \
             bytes",
            destination.display(),
            owner.display(),
            staging_scheme(),
        ),
    )
}

/// How a files-mode staging name is spelled, for the refusals that cite it.
fn staging_scheme() -> String {
    format!("<name>{STAGING_TEMP_INFIX}<pid>, pid {}", process::id())
}

/// What a files-mode staging name puts between the destination's final
/// component and the pid: `out.md` stages as `out.md.tmp.<pid>`. Shared with
/// the recognizer the directory guards use for leftovers
/// (`super::fileset::tmp_pid_suffix`).
pub(super) const STAGING_TEMP_INFIX: &str = ".tmp.";

/// The one derivation of a files-mode staging name (R0013-0001): the temp
/// `super::fileset::write_fileset_atomic` stages `path` into, and the name
/// [`vet_destinations`] reserves and [`refuse_unspellable_staging_name`] asks
/// the filesystem about. Appended to the FULL file name rather than replacing
/// the extension — `with_extension` would collide same-stem siblings (`out.md`
/// and `out.json` both staging as `out.tmp.<pid>`) in one fileset.
pub(super) fn staging_temp_path(path: &Path) -> io::Result<PathBuf> {
    let name = destination_file_name(path)?;
    Ok(path.with_file_name(format!("{name}{STAGING_TEMP_INFIX}{}", process::id())))
}

/// Refuse `destination` when the filesystem refuses `derived`, the staging
/// name the publication would create for it (R0013-0006).
///
/// Every staging name is the final component plus a suffix — `.tmp.<pid>` in
/// files mode, `.<name>.staging.<pid>.<token>` beside an `--out-dir` target —
/// so a component the volume accepts can still produce one it refuses. That is
/// a fact about the arguments and the volume, and it surfaced only when the
/// staging create failed after the provider had been paid. The question is
/// put to the filesystem rather than to a byte count, because the limit is the
/// volume's: HFS+ and APFS count UTF-16 units, so a 300-byte CJK name is legal
/// there and a fixed 255-byte rule would refuse it.
///
/// Read-only: one `lstat` of the derived name in the deepest level of its
/// directory that exists ([`claim_anchor`]), because a lookup through a missing
/// level answers "not found" before it looks at the name. Only the
/// filesystem's own refusal of the name (`ErrorKind::InvalidFilename` —
/// `ENAMETOOLONG` on Unix) refuses the destination; every other answer passes
/// and stays with the publication's own checks. That also leaves one case
/// late: a destination whose missing levels push the whole path past the
/// platform's path limit is probed on the shorter path that exists, and is
/// found by the publication, which asks again once it has created those
/// levels.
pub(super) fn refuse_unspellable_staging_name(
    destination: &Path,
    derived: &Path,
) -> io::Result<()> {
    let Some(name) = derived.file_name() else {
        return Ok(());
    };
    let dir = match derived.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    match fs::symlink_metadata(claim_anchor(dir).join(name)) {
        Err(e) if e.kind() == io::ErrorKind::InvalidFilename => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "output destination {} cannot be published: the filesystem refuses the staging \
                 name transync derives from it, {} ({e}); transync adds a suffix to the final \
                 path component while publishing, so choose a shorter name",
                destination.display(),
                name.display(),
            ),
        )),
        _ => Ok(()),
    }
}

/// The final path component of a destination, as UTF-8 — the name every
/// staging name is built from (`<dest>.tmp.<pid>` by [`staging_temp_path`],
/// the `.<name>.staging.<pid>.<token>` siblings under `--out-dir`) — once the
/// path has been cleared of the publication-lock marker's reserved name
/// ([`refuse_reserved_names`]). Whether the filesystem accepts the staging
/// name built from it is [`refuse_unspellable_staging_name`]'s question.
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

/// A file's identity on the filesystem: `dev` and `ino`.
pub(super) type FileId = (u64, u64);

/// [`FileId`] of what `meta` describes; `None` off Unix, where `std` has no
/// portable file identity (the same limit `super::lock::same_file` documents),
/// so the comparisons that need one fall back to the path alone.
#[cfg(unix)]
pub(super) fn file_id(meta: &fs::Metadata) -> Option<FileId> {
    use std::os::unix::fs::MetadataExt;
    Some((meta.dev(), meta.ino()))
}

#[cfg(not(unix))]
pub(super) fn file_id(_meta: &fs::Metadata) -> Option<FileId> {
    None
}

/// What a path names, in the terms the destination guards compare: the
/// directory **entry** a publication would rename onto, which is not the same
/// thing as the byte string the caller typed (R0002-0032, R0012-0002).
///
/// Two rules shape it, and each one keeps a false alarm out:
///
/// * **The final component is never followed.** A destination that is itself
///   a symlink (`alias.md` → `out.md`) is *not* its target: publication
///   renames over the link entry rather than through it, so the two commit as
///   two independent files. A hard link is likewise its own entry. What must
///   collide is what shares a directory *and* a name, because that is what
///   shares a `<dest>.tmp.<pid>` staging name.
/// * **Nothing is lowercased.** The final component is spelled the way its
///   directory stores it ([`stored_leaf`]): on a volume that folds letter case
///   or Unicode normalization (HFS+, APFS, NTFS, a casefolded ext4 directory)
///   `OUT.md` resolves to the stored `out.md` and is that entry, while on a
///   case-sensitive volume `OUT.md` is a different name the directory does
///   not hold. The filesystem is asked; no folding table is assumed.
///
/// The parent is resolved as far as it exists (symlinks and `..` alike) and
/// the rest folded lexically. On macOS the resolution also spells every
/// existing component as stored, but on Linux `realpath` keeps the typed
/// spelling of a casefolded directory, so the parent is held by `dev`+`ino`
/// as well — the deepest level that exists, with the components below it — and
/// two identities through one directory compare equal however that directory
/// was spelled. Directories cannot be hard-linked, which is what makes that
/// exact.
#[derive(Debug)]
pub(crate) struct EntryIdentity {
    /// The path made absolute, its existing parent resolved, the remainder
    /// folded lexically, and the final component as its directory stores it.
    path: PathBuf,
    /// The deepest level of the parent that exists, and its identity.
    anchor: Option<(PathBuf, FileId)>,
    /// The components below the anchor: the parent's missing levels, as
    /// typed, then the final component, as stored.
    rest: Vec<OsString>,
    /// The identity of the anchor and of every level above it, the anchor
    /// first — what "this path runs through that directory" is decided by.
    levels: Vec<FileId>,
    /// The identity of the entry itself, when it exists and is not a symlink
    /// (a symlink to a directory is renamed over, never published through).
    own: Option<FileId>,
}

impl EntryIdentity {
    /// The identity of `path`, made absolute against `cwd`. Reads the
    /// filesystem and never writes to it.
    pub(crate) fn of(cwd: &Path, path: &Path) -> Self {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            cwd.join(path)
        };
        let (Some(parent), Some(name)) = (absolute.parent(), absolute.file_name()) else {
            let path = lexically_normalized(&absolute);
            let own = entry_id(&path);
            return Self {
                path,
                anchor: None,
                rest: Vec::new(),
                levels: Vec::new(),
                own,
            };
        };
        let parent = lexically_normalized(&resolve_existing_prefix(parent));
        let leaf = stored_leaf(&parent, name);
        let path = parent.join(&leaf);
        let anchor = parent.ancestors().find_map(|level| {
            let id = fs::metadata(level).ok().as_ref().and_then(file_id)?;
            Some((level.to_path_buf(), id))
        });
        let (rest, levels) = match &anchor {
            Some((at, _)) => (
                parent
                    .strip_prefix(at)
                    .map(|below| {
                        below
                            .components()
                            .map(|part| part.as_os_str().to_os_string())
                            .collect()
                    })
                    .unwrap_or_default(),
                at.ancestors()
                    .filter_map(|level| fs::metadata(level).ok().as_ref().and_then(file_id))
                    .collect(),
            ),
            None => (Vec::new(), Vec::new()),
        };
        let mut rest: Vec<OsString> = rest;
        rest.push(leaf);
        let own = entry_id(&path);
        Self {
            path,
            anchor,
            rest,
            levels,
            own,
        }
    }

    /// Whether `self` and `other` name one directory entry.
    pub(crate) fn same_entry(&self, other: &Self) -> bool {
        if self.path == other.path {
            return true;
        }
        let Some(anchor) = self.shared_anchor(other) else {
            return false;
        };
        match compare_components(&self.rest, &other.rest) {
            Spelling::Same => true,
            Spelling::LetterCase => self.unborn() && other.unborn() && folds_ascii_case(anchor),
            Spelling::Different => false,
        }
    }

    /// Whether `self` lies strictly **inside** `outer` — `outer` would have to
    /// be a directory for `self` to be published.
    pub(crate) fn is_inside(&self, outer: &Self) -> bool {
        // Component-wise, so `a/bc` is not inside `a/b`.
        if self.path != outer.path && self.path.starts_with(&outer.path) {
            return true;
        }
        if outer.own.is_some_and(|id| self.levels.contains(&id)) {
            return true;
        }
        let Some(anchor) = self.shared_anchor(outer) else {
            return false;
        };
        if self.rest.len() <= outer.rest.len() {
            return false;
        }
        match compare_components(&self.rest[..outer.rest.len()], &outer.rest) {
            Spelling::Same => true,
            Spelling::LetterCase => self.unborn() && outer.unborn() && folds_ascii_case(anchor),
            Spelling::Different => false,
        }
    }

    /// The anchor both identities hang from, when it is one directory.
    fn shared_anchor<'s>(&'s self, other: &Self) -> Option<&'s Path> {
        match (&self.anchor, &other.anchor) {
            (Some((at, mine)), Some((_, theirs))) if mine == theirs => Some(at),
            _ => None,
        }
    }

    /// Whether nothing exists at the final component yet — so no directory
    /// entry can say which spelling it stores, and only the volume's folding
    /// rule can decide whether two spellings are one name.
    fn unborn(&self) -> bool {
        fs::symlink_metadata(&self.path).is_err()
    }
}

/// How two component lists compare as spellings.
enum Spelling {
    /// Byte for byte.
    Same,
    /// Equal once ASCII letter case is ignored, and not otherwise.
    LetterCase,
    Different,
}

fn compare_components(left: &[OsString], right: &[OsString]) -> Spelling {
    if left.len() != right.len() {
        return Spelling::Different;
    }
    if left == right {
        return Spelling::Same;
    }
    let folded = left.iter().zip(right).all(|(a, b)| {
        a.as_encoded_bytes()
            .eq_ignore_ascii_case(b.as_encoded_bytes())
    });
    if folded {
        Spelling::LetterCase
    } else {
        Spelling::Different
    }
}

/// [`FileId`] of the entry at `path` itself, when it exists and is not a
/// symlink.
fn entry_id(path: &Path) -> Option<FileId> {
    let meta = fs::symlink_metadata(path).ok()?;
    if meta.file_type().is_symlink() {
        return None;
    }
    file_id(&meta)
}

/// `name` spelled the way `dir` **stores** it (R0012-0002).
///
/// A volume that folds letter case or Unicode normalization resolves several
/// spellings to one entry, and only one of them is in the directory. The
/// stored one is found by listing `dir`: the typed spelling when it is listed
/// verbatim (every case-sensitive volume, and the common case everywhere —
/// which also keeps two hard links of one file two entries), otherwise the
/// listed entry with the same `dev`+`ino` as the typed one (`lstat`, so the
/// final component is never followed and a symlink is identified by its own
/// stored name).
///
/// The typed spelling is returned unchanged when there is nothing to resolve
/// — nothing exists at the name — and whenever the filesystem cannot answer:
/// `dir` cannot be listed (searchable but not readable), or more than one
/// hard link of the file in `dir` could be the one meant and ASCII letter case
/// does not single one out. Falling back to the typed spelling can miss an
/// alias but never invent one, so the guards never refuse on their own
/// probe's failure.
fn stored_leaf(dir: &Path, name: &OsStr) -> OsString {
    let typed = || name.to_os_string();
    let Ok(entry) = fs::symlink_metadata(dir.join(name)) else {
        return typed();
    };
    let Ok(listing) = fs::read_dir(dir) else {
        return typed();
    };
    stored_spelling(dir, listing, name, &entry).unwrap_or_else(typed)
}

#[cfg(unix)]
fn stored_spelling(
    _dir: &Path,
    listing: fs::ReadDir,
    name: &OsStr,
    entry: &fs::Metadata,
) -> Option<OsString> {
    use std::os::unix::fs::{DirEntryExt, MetadataExt};
    let items: Vec<fs::DirEntry> = listing.flatten().collect();
    if items.iter().any(|item| item.file_name() == name) {
        return Some(name.to_os_string());
    }
    // `lstat` of the listed entry, `dev` included: the listing's inode number
    // is only unique within a volume.
    let same = |item: &&fs::DirEntry| {
        item.metadata()
            .is_ok_and(|meta| meta.dev() == entry.dev() && meta.ino() == entry.ino())
    };
    // `d_ino` first, so a large directory costs one `lstat` per entry that
    // could be the one rather than one per entry — and every entry only where
    // the listing's inode numbers do not match `lstat`'s (a mount point, some
    // overlay filesystems).
    let mut candidates: Vec<OsString> = items
        .iter()
        .filter(|item| item.ino() == entry.ino())
        .filter(same)
        .map(fs::DirEntry::file_name)
        .collect();
    if candidates.is_empty() {
        candidates = items
            .iter()
            .filter(same)
            .map(fs::DirEntry::file_name)
            .collect();
    }
    if let [only] = candidates.as_slice() {
        return Some(only.clone());
    }
    let mut folded = candidates.into_iter().filter(|stored| {
        stored
            .as_encoded_bytes()
            .eq_ignore_ascii_case(name.as_encoded_bytes())
    });
    match (folded.next(), folded.next()) {
        (Some(only), None) => Some(only),
        _ => None,
    }
}

/// Off Unix there is no `dev`+`ino` in `std` to match a listing against; the
/// platform's own path resolution is asked instead, which on Windows returns
/// the stored spelling of every component. A symlink keeps its typed name:
/// resolving it would name its target.
#[cfg(not(unix))]
fn stored_spelling(
    dir: &Path,
    mut listing: fs::ReadDir,
    name: &OsStr,
    entry: &fs::Metadata,
) -> Option<OsString> {
    if listing.any(|item| item.is_ok_and(|item| item.file_name() == name)) {
        return Some(name.to_os_string());
    }
    if entry.file_type().is_symlink() {
        return None;
    }
    Some(
        fs::canonicalize(dir.join(name))
            .ok()?
            .file_name()?
            .to_os_string(),
    )
}

/// Whether the volume holding `dir` resolves a spelling that differs from a
/// stored name only in ASCII letter case to that name — asked of the
/// filesystem, read-only, by `lstat`-ing a case-swapped spelling of a name it
/// holds and comparing `dev`+`ino` (R0012-0002).
///
/// The one place letter case is compared without a stored name to anchor it:
/// two destinations that do not exist yet, which no listing can resolve. Every
/// volume that folds case folds ASCII the same way, so the question has one
/// answer per directory. The probe is an entry of `dir` whose swapped spelling
/// is not listed beside it (so two distinct entries on a case-sensitive volume
/// cannot pass for one), or, in a directory with no such entry, `dir`'s own
/// name in the level holding it — when that level is on the same volume, since
/// a mount point's name lives on the volume above it. `false` whenever there is
/// nothing to ask with: the guards never refuse on a probe that could not run.
#[cfg(unix)]
fn folds_ascii_case(dir: &Path) -> bool {
    let swapped = |name: &OsStr| -> Option<OsString> {
        let bytes: Vec<u8> = name
            .as_encoded_bytes()
            .iter()
            .map(|b| match b {
                b'a'..=b'z' => b.to_ascii_uppercase(),
                b'A'..=b'Z' => b.to_ascii_lowercase(),
                _ => *b,
            })
            .collect();
        (bytes != name.as_encoded_bytes()).then(|| {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(bytes)
        })
    };
    let same = |a: &Path, b: &Path| match (fs::symlink_metadata(a), fs::symlink_metadata(b)) {
        (Ok(a), Ok(b)) => file_id(&a) == file_id(&b),
        _ => false,
    };
    let names: Vec<OsString> = fs::read_dir(dir)
        .map(|listing| listing.flatten().map(|item| item.file_name()).collect())
        .unwrap_or_default();
    if let Some((name, other)) = names.iter().find_map(|name| {
        let other = swapped(name)?;
        (!names.contains(&other)).then_some((name, other))
    }) {
        return same(&dir.join(name), &dir.join(other));
    }
    let (Some(holder), Some(own)) = (dir.parent(), dir.file_name()) else {
        return false;
    };
    let Some(other) = swapped(own) else {
        return false;
    };
    let on_one_volume = match (fs::metadata(holder), fs::metadata(dir)) {
        (Ok(holder), Ok(dir)) => {
            use std::os::unix::fs::MetadataExt;
            holder.dev() == dir.dev()
        }
        _ => false,
    };
    on_one_volume && same(dir, &holder.join(other))
}

/// No `dev`+`ino` to compare off Unix: the early pass answers "no" and the
/// publication's `create_new` is what refuses a pair of new names the volume
/// folds onto one.
#[cfg(not(unix))]
fn folds_ascii_case(_dir: &Path) -> bool {
    false
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
    use crate::output::testing::{
        folds_letter_case, name_is_refused, plant_with_normalization_alias, scratch_dir,
    };
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

    /// The refusal a pair naming one file twice has to produce: exit 4's
    /// `InvalidInput`, and the duplicate sentence rather than the late
    /// "refusing existing temp" the shared staging name used to produce.
    fn assert_duplicate_refusal(pass: &str, outcome: io::Result<()>) {
        let err = outcome.expect_err(pass);
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{pass}: {err}");
        assert!(
            err.to_string().contains("duplicate output destination"),
            "{pass}: the refusal must say the two destinations are one: {err}"
        );
    }

    /// R0012-0002: on a volume that folds letter case, `out.md` and `OUT.md`
    /// are one directory entry, and the identity kept the final component
    /// exactly as typed — so the pair passed every check and failed after the
    /// provider was paid, on the staging name the two shared. The leaf is now
    /// identified by the name its directory **stores** (`read_dir` plus the
    /// entry's `dev`+`ino`), never by lowercasing it: on a case-sensitive
    /// volume the two are two files, and refusing them is the false alarm the
    /// review warned about. A final symlink is still its own entry under any
    /// spelling (R0002-0032).
    #[test]
    fn case_aliases_of_one_entry_are_one_destination() {
        let root = scratch_dir("case-alias-destinations");
        if !folds_letter_case(&root) {
            vet_destinations([
                root.join("result.json").as_path(),
                root.join("RESULT.JSON").as_path(),
            ])
            .expect("a case-sensitive volume stores two names as two files");
            eprintln!(
                "note: {} is case-sensitive; the case-alias half of R0012-0002 did not run on \
                 this filesystem",
                root.display()
            );
            return;
        }

        fs::write(root.join("out.md"), b"previous").expect("an existing destination");
        for pair in [
            ["out.md", "OUT.md"],
            ["OUT.MD", "out.md"],
            ["OUT.MD", "OUT.MD"],
        ] {
            assert_duplicate_refusal(
                &format!("{pair:?} over an existing out.md"),
                vet_destinations(
                    pair.map(|name| root.join(name))
                        .iter()
                        .map(PathBuf::as_path),
                ),
            );
        }

        fs::create_dir(root.join("x")).expect("an existing directory");
        for pair in [
            [root.join("X"), root.join("x").join("y.json")],
            [root.join("x").join("y.json"), root.join("X")],
        ] {
            let err = vet_destinations(pair.iter().map(PathBuf::as_path))
                .expect_err("a case alias of a directory still holds the other destination");
            assert!(
                err.to_string().contains("is inside output destination"),
                "the containment refusal, not a late create_dir_all failure: {err}"
            );
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(root.join("out.md"), root.join("alias.md"))
                .expect("symlink the file");
            vet_destinations([
                root.join("out.md").as_path(),
                root.join("ALIAS.MD").as_path(),
            ])
            .expect("a destination that IS a symlink is its own file under any spelling");
        }

        // Neither name exists yet, so no directory entry can say which one it
        // stores: the volume is asked whether it folds letter case, read-only.
        let fresh = [root.join("result.json"), root.join("RESULT.JSON")];
        assert_duplicate_refusal(
            "two new names that differ only in letter case",
            vet_destinations(fresh.iter().map(PathBuf::as_path)),
        );
        assert_duplicate_refusal(
            "the publication of that pair",
            write_fileset_atomic(
                &[
                    (fresh[0].clone(), b"first".as_slice()),
                    (fresh[1].clone(), b"second".as_slice()),
                ],
                &silent,
            ),
        );
        assert!(
            !fresh[0].exists(),
            "a refused pair publishes neither member"
        );
        let err = vet_destinations([
            root.join("NEW").as_path(),
            root.join("new").join("y.json").as_path(),
        ])
        .expect_err("a new directory spelled two ways is one directory");
        assert!(
            err.to_string().contains("is inside output destination"),
            "{err}"
        );
    }

    /// R0012-0002, the normalization half: HFS+ and APFS resolve `é` written
    /// precomposed and written as `e` + COMBINING ACUTE to one entry. The alias
    /// is derived from the spelling `read_dir` reports — HFS+ stores NFD
    /// whatever it was given, APFS stores what it was given — so the pair is a
    /// real alias on both, and the test cannot compare a name with itself.
    #[test]
    fn normalization_aliases_of_one_entry_are_one_destination() {
        let root = scratch_dir("normalization-alias-destinations");
        let Some((stored, alias)) = plant_with_normalization_alias(&root, ".md") else {
            eprintln!(
                "note: {} does not fold Unicode normalization; the normalization half of \
                 R0012-0002 did not run on this filesystem",
                root.display()
            );
            return;
        };
        for pair in [[&stored, &alias], [&alias, &stored]] {
            assert_duplicate_refusal(
                "two normalization forms of one stored name",
                vet_destinations(pair.map(PathBuf::as_path)),
            );
        }
    }

    /// R0013-0001: one destination may not be another's staging name. With
    /// `--output out.json.tmp.<pid> --map out.json`, the first commit renamed
    /// its temp over the second's, and the second then published the first's
    /// bytes as `out.json` — exit 0, one output gone, the other wrong. The
    /// staging namespace is reserved in the pass before the provider call,
    /// in either order, through a symlinked parent, and for a destination
    /// *inside* a staging name (which used to fail late in `create_new`). It
    /// is exact to this run's pid, not a pattern ban on the suffix.
    #[test]
    fn the_staging_namespace_is_reserved_before_the_provider_call() {
        let root = scratch_dir("staging-namespace");
        let pid = std::process::id();
        let staged = root.join(format!("out.json.tmp.{pid}"));
        let final_ = root.join("out.json");

        for pair in [
            [staged.clone(), final_.clone()],
            [final_.clone(), staged.clone()],
            [staged.join("inner.json"), final_.clone()],
        ] {
            let err = preflight_destination_set(&pair)
                .expect_err("a destination in another's staging namespace is refused up front");
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{err}");
            let msg = err.to_string();
            assert!(
                msg.contains("staging name")
                    && msg.contains(&pair[0].display().to_string())
                    && msg.contains(&pair[1].display().to_string()),
                "the refusal must name both destinations and the reason: {msg}"
            );
        }

        #[cfg(unix)]
        {
            let real = root.join("real");
            fs::create_dir(&real).expect("the real directory");
            let link = root.join("link");
            std::os::unix::fs::symlink(&real, &link).expect("symlink the parent");
            preflight_destination_set(&[
                link.join("out.json"),
                real.join(format!("out.json.tmp.{pid}")),
            ])
            .expect_err("a symlinked parent still names the staging name");
        }

        preflight_destination_set(&[final_, root.join("out.json.tmp.0")])
            .expect("another pid's staging name is an ordinary destination");
        assert!(
            fs::read_dir(&root)
                .expect("list the root")
                .flatten()
                .all(|entry| entry.file_name() == "real" || entry.file_name() == "link"),
            "the preflight creates nothing"
        );
    }

    /// The same reservation under a spelling the volume folds: the reserved
    /// name in other letter case is the same entry on HFS+ and default APFS,
    /// and the identity comparison above is what has to see it.
    #[test]
    fn a_letter_case_variant_of_a_staging_name_is_refused_up_front() {
        let root = scratch_dir("staging-namespace-case");
        if !folds_letter_case(&root) {
            eprintln!(
                "note: {} is case-sensitive; the case-variant half of R0013-0001 did not run on \
                 this filesystem",
                root.display()
            );
            return;
        }
        let upper = root.join(format!("OUT.JSON.TMP.{}", std::process::id()));
        let final_ = root.join("out.json");
        for pair in [[&upper, &final_], [&final_, &upper]] {
            let err = vet_destinations(pair.map(PathBuf::as_path))
                .expect_err("a case variant of the staging name is the staging name");
            assert!(err.to_string().contains("staging name"), "{err}");
        }
    }

    /// R0013-0006: a final component the filesystem accepts can still produce
    /// a staging name it refuses — `<name>.tmp.<pid>` is the name plus up to a
    /// dozen bytes — and that used to surface from `stage_one_file`, after the
    /// provider had been paid, as "refusing existing temp" about a temp that
    /// never existed. The filesystem is asked, read-only, before anything is
    /// created, with the same sentence in both passes. No fixed byte count: a
    /// 300-byte CJK name is legal on HFS+ and APFS, which count UTF-16 units.
    #[test]
    fn a_name_whose_staging_name_the_filesystem_refuses_is_refused_before_the_provider_call() {
        let root = scratch_dir("staging-name-length");
        let pid = std::process::id().to_string();
        let fits = 255 - ".tmp.".len() - pid.len();
        let long_name = "a".repeat(fits + 1);
        let long = root.join(&long_name);
        if name_is_refused(&long) || !name_is_refused(&root.join(format!("{long_name}.tmp.{pid}")))
        {
            eprintln!(
                "note: {} does not cap a component at 255 bytes; R0013-0006's files-mode half \
                 did not run on this filesystem",
                root.display()
            );
            return;
        }

        for (pass, outcome) in [
            (
                "the files-mode preflight",
                preflight_destination_set(std::slice::from_ref(&long)),
            ),
            (
                "the files-mode publication",
                write_fileset_atomic(&[(long.clone(), b"x".as_slice())], &silent),
            ),
        ] {
            let err = outcome.expect_err(pass);
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{pass}: {err}");
            let msg = err.to_string();
            assert!(
                msg.contains("staging name") && msg.contains(&long_name),
                "{pass}: the refusal must name the destination and the derived name: {msg}"
            );
        }
        assert_eq!(
            fs::read_dir(&root).expect("list the root").count(),
            0,
            "a refused name creates nothing, not even a lock marker"
        );

        let fitting = root.join("a".repeat(fits));
        preflight_destination_set(std::slice::from_ref(&fitting))
            .expect("a name whose staging name fits is publishable");
        write_fileset_atomic(&[(fitting.clone(), b"x".as_slice())], &silent)
            .expect("and it publishes");

        let wide = root.join("\u{7ffb}".repeat(100));
        if !name_is_refused(&root.join(format!("{}.tmp.{pid}", "\u{7ffb}".repeat(100)))) {
            preflight_destination_set(std::slice::from_ref(&wide))
                .expect("a multi-byte name the filesystem can stage is not refused by byte count");
        }
    }

    /// R0013-0006 under `--out-dir`: the staging sibling is
    /// `.<name>.staging.<pid>.<16 hex>`, 27 bytes plus the pid longer than the
    /// target's own name, and `publish_into_parent` met the limit in a bare
    /// `create_dir` that named no path. Both passes now refuse it before
    /// anything is created.
    #[test]
    fn an_out_dir_whose_staging_sibling_the_filesystem_refuses_is_refused_up_front() {
        let root = scratch_dir("out-dir-sibling-length");
        let pid = std::process::id().to_string();
        let name = "b".repeat(256 - 27 - pid.len());
        let target = root.join(&name);
        let sibling = root.join(format!(".{name}.staging.{pid}.{}", "0".repeat(16)));
        if name_is_refused(&target) || !name_is_refused(&sibling) {
            eprintln!(
                "note: {} does not cap a component at 255 bytes; R0013-0006's --out-dir half \
                 did not run on this filesystem",
                root.display()
            );
            return;
        }

        for (pass, outcome) in [
            (
                "the --out-dir preflight",
                preflight_out_dir(&target, false, &silent),
            ),
            (
                "the --out-dir publication",
                publish_out_dir(&target, &one_file(), false, &silent),
            ),
        ] {
            let err = outcome.expect_err(pass);
            assert_eq!(err.kind(), io::ErrorKind::InvalidInput, "{pass}: {err}");
            let msg = err.to_string();
            assert!(
                msg.contains("staging name") && msg.contains(&name),
                "{pass}: the refusal must name the target and say why: {msg}"
            );
        }
        assert_eq!(
            fs::read_dir(&root).expect("list the root").count(),
            0,
            "a refused target creates nothing: no target, no sibling, no lock marker"
        );
    }
}
