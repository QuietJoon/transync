//! Destination vetting and lexical path normalization: the refusals
//! [`vet_destinations`] makes up front — before anything is created, staged
//! or locked — and the normalization that lets two spellings of one path be
//! recognized as the same destination.
//!
//! Lifted out of `output.rs` unchanged by OI-0043.
//!
//! TRACE: contracts.md §6

use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};

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
pub(super) fn vet_destinations<'a>(paths: impl IntoIterator<Item = &'a Path>) -> io::Result<()> {
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
/// `super::fileset::stage_one_file`'s, i.e. after the translation had been paid for, and it
/// said "path has no file name" about a path that has one. Asked in the
/// destination preflight and again where the name is actually built, with the
/// same sentence either way, so an operator cannot tell the two passes apart.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::fileset::write_fileset_atomic;
    use crate::output::preflight::{
        html_bundle_paths, preflight_destination_set, preflight_out_dir,
    };
    use crate::output::testing::scratch_dir;
    use crate::output::tests::silent;
    use std::ffi::OsStr;

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
