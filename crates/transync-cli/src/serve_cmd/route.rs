//! Request target → a path inside the served root, or a refusal.
//!
//! This is the security surface of `transync serve`, and it is deliberately
//! two checks rather than one, because either alone is a known hole.
//!
//! **Lexical confinement ([`confine`]).** The target is split into `/`
//! segments *before* anything is percent-decoded, each segment is decoded on
//! its own, and a decoded segment that is `.`, `..`, or that carries a
//! separator or a NUL is refused. Decoding first and splitting afterwards is
//! the classic bug: `%2e%2e%2f` becomes `../` only after the decode, so a
//! splitter that runs before it sees one innocent segment and a splitter that
//! runs after it sees a traversal that the *decoder* invented. Refusing a
//! decoded separator outright is what makes the order safe in both directions.
//!
//! **Containment after resolution (the caller's second step).** Lexical
//! confinement says nothing about symlinks: a link named `escape.html` inside
//! the root resolves to wherever it points, and no amount of inspecting the
//! request text can see that. So the caller canonicalizes the candidate — an
//! operation that resolves every link and every `.`/`..` the filesystem itself
//! contains — and only then asks whether the result is still under the
//! canonical root. [`is_contained`] is that question. Both the root and the
//! candidate are canonical when it is asked, which is the only form in which
//! the answer means anything.
//!
//! Refusals **reject, never clamp**. Clamping a traversal back to the root
//! turns a request for something outside into a successful response for
//! something inside, which is a different document than the one asked for and
//! hides the attempt from anyone reading the response.
//!
//! TRACE: SCN-13

use std::path::{Component, Path, PathBuf};

/// Why a request cannot be answered from the served root.
///
/// Each maps to one HTTP status in `conn`; they are separate here because the
/// reason is worth keeping distinct at the point it is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The request target is not a thing this server can interpret: not
    /// origin-form, a broken percent escape, or a segment that decoded into
    /// something that is not a single path component. `400`.
    BadRequest,
    /// The target is well-formed and names something outside the served root
    /// — a `..` segment, or a path that resolved out through a symlink.
    /// `403`.
    Forbidden,
    /// Nothing is there, or what is there is not a regular file. `404`.
    NotFound,
}

/// The path portion of a request target: everything before `?` or `#`.
fn path_of(target: &str) -> &str {
    let end = target.find(['?', '#']).unwrap_or(target.len());
    &target[..end]
}

/// Decode one path segment's percent escapes.
///
/// Refuses a truncated or non-hex escape, and refuses a decode that is not
/// UTF-8 — the bundle's filenames are ASCII, and a server that guesses at
/// byte sequences it cannot name is a server that can be talked into naming
/// something unintended.
fn percent_decode(segment: &str) -> Result<String, Refusal> {
    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        if i + 2 >= bytes.len() {
            return Err(Refusal::BadRequest);
        }
        let hi = hex(bytes[i + 1]).ok_or(Refusal::BadRequest)?;
        let lo = hex(bytes[i + 2]).ok_or(Refusal::BadRequest)?;
        out.push(hi * 16 + lo);
        i += 3;
    }
    String::from_utf8(out).map_err(|_| Refusal::BadRequest)
}

/// Join `target` onto `root` as a candidate path, refusing anything that is
/// not a plain descent through named components.
///
/// `root` must already be canonical — [`is_contained`] compares against it.
/// A target ending in `/`, and the bare `/`, resolve to `index.html` in the
/// directory named; there is no directory listing, by design (the ticket asks
/// for none, and a listing publishes filenames the operator never chose to
/// publish).
pub fn confine(root: &Path, target: &str) -> Result<PathBuf, Refusal> {
    let path = path_of(target);
    // Only origin-form. `*` (OPTIONS) and absolute-form (`http://host/x`,
    // which a proxy would send) are not requests this server answers.
    if !path.starts_with('/') {
        return Err(Refusal::BadRequest);
    }

    let mut candidate = root.to_path_buf();
    let mut named_something = false;
    for raw in path.split('/') {
        if raw.is_empty() {
            // `//a` and the leading `/`: an empty segment names nothing.
            continue;
        }
        let segment = percent_decode(raw)?;
        if segment == "." || segment == ".." {
            return Err(Refusal::Forbidden);
        }
        if segment.contains('/') || segment.contains('\\') || segment.contains('\0') {
            // A decoded separator (`%2f`, `%5c`) or a NUL: the escape was
            // trying to become structure after the split had already run.
            return Err(Refusal::BadRequest);
        }
        // Belt and braces: reject anything the OS would read as more than one
        // ordinary component, whatever the byte inspection above concluded.
        if Path::new(&segment)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(Refusal::BadRequest);
        }
        candidate.push(segment);
        named_something = true;
    }

    if !named_something || path.ends_with('/') {
        candidate.push("index.html");
    }
    Ok(candidate)
}

/// Whether `resolved` is the served root or something beneath it.
///
/// Both arguments must be canonical. `Path::starts_with` compares whole
/// components, so a sibling root named `<root>-public` is not "under" `<root>`
/// the way a string prefix test would have said it was.
pub fn is_contained(root: &Path, resolved: &Path) -> bool {
    resolved == root || resolved.starts_with(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = "/srv/bundle";

    fn ok(target: &str) -> PathBuf {
        confine(Path::new(ROOT), target).expect("target should resolve")
    }

    fn err(target: &str) -> Refusal {
        confine(Path::new(ROOT), target).expect_err("target should be refused")
    }

    #[test]
    fn ordinary_targets_land_under_the_root() {
        assert_eq!(ok("/index.html"), PathBuf::from("/srv/bundle/index.html"));
        assert_eq!(ok("/js/sync.js"), PathBuf::from("/srv/bundle/js/sync.js"));
        assert_eq!(
            ok("/alignment.json?v=2"),
            PathBuf::from("/srv/bundle/alignment.json"),
            "a query string is not part of the path",
        );
        assert_eq!(
            ok("/index.html#top"),
            PathBuf::from("/srv/bundle/index.html"),
            "a fragment never reaches a server, but if one does it is not a filename",
        );
    }

    #[test]
    fn a_directory_target_resolves_to_its_index() {
        assert_eq!(ok("/"), PathBuf::from("/srv/bundle/index.html"));
        assert_eq!(ok("/sub/"), PathBuf::from("/srv/bundle/sub/index.html"));
        assert_eq!(
            ok("//"),
            PathBuf::from("/srv/bundle/index.html"),
            "empty segments name nothing and must not become empty directories",
        );
    }

    /// The whole point. Every spelling of "go up" is refused, decoded or not.
    #[test]
    fn traversal_is_refused_in_every_spelling() {
        assert_eq!(err("/../secret"), Refusal::Forbidden);
        assert_eq!(err("/a/../../secret"), Refusal::Forbidden);
        assert_eq!(err("/%2e%2e/secret"), Refusal::Forbidden);
        assert_eq!(err("/%2E%2E/secret"), Refusal::Forbidden);
        assert_eq!(err("/."), Refusal::Forbidden);
        assert_eq!(
            err("/..%2fsecret"),
            Refusal::BadRequest,
            "the decode invented a separator the split never saw",
        );
        assert_eq!(err("/%2f%2e%2e%2fsecret"), Refusal::BadRequest);
        assert_eq!(
            err("/a%5c..%5csecret"),
            Refusal::BadRequest,
            "a backslash is a separator on the platform this could be replayed to",
        );
    }

    /// A double decode would turn `%252e%252e` into `..`; one decode turns it
    /// into the harmless literal `%2e%2e`, which is a filename and nothing
    /// more. Decoding exactly once is the contract.
    #[test]
    fn decoding_happens_exactly_once() {
        assert_eq!(
            ok("/%252e%252e/x"),
            PathBuf::from("/srv/bundle/%2e%2e/x"),
            "the decoder must not run over its own output",
        );
    }

    #[test]
    fn a_malformed_or_unnameable_target_is_a_bad_request() {
        assert_eq!(err("*"), Refusal::BadRequest);
        assert_eq!(err("http://elsewhere/x"), Refusal::BadRequest);
        assert_eq!(err("index.html"), Refusal::BadRequest);
        assert_eq!(err("/%"), Refusal::BadRequest);
        assert_eq!(err("/%zz"), Refusal::BadRequest);
        assert_eq!(err("/%2"), Refusal::BadRequest);
        assert_eq!(err("/a%00b"), Refusal::BadRequest);
        assert_eq!(
            err("/%ff%fe"),
            Refusal::BadRequest,
            "a filename this server cannot name is a filename it will not open",
        );
    }

    #[test]
    fn a_dotfile_is_an_ordinary_name_not_a_traversal() {
        assert_eq!(
            ok("/.transync-publish.lock"),
            PathBuf::from("/srv/bundle/.transync-publish.lock"),
            "only `.` and `..` exactly are structure; a leading dot is a name",
        );
    }

    #[test]
    fn containment_compares_components_not_string_prefixes() {
        let root = Path::new("/srv/bundle");
        assert!(is_contained(root, Path::new("/srv/bundle")));
        assert!(is_contained(root, Path::new("/srv/bundle/index.html")));
        assert!(is_contained(root, Path::new("/srv/bundle/js/sync.js")));
        assert!(!is_contained(root, Path::new("/srv/bundle-public/x")));
        assert!(!is_contained(root, Path::new("/srv")));
        assert!(!is_contained(root, Path::new("/etc/passwd")));
    }
}
