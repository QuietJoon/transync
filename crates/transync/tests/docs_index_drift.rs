//! Guard against drift between `docs/index.md` and the docs tree.
//!
//! `docs/index.md` is the single entry point for everything under `docs/`
//! (it says so in its own header). Two ways it silently rots:
//!
//! (a) a doc lands but nobody links it — an orphan;
//! (b) the index links a doc that was renamed or moved — a dead link.
//!
//! This test fails on both:
//! - every Markdown document under `docs/`, at any depth, must be linked from
//!   `docs/index.md`;
//! - every relative markdown link target in `docs/index.md` must resolve to a
//!   real path on disk (http(s) / mailto links are skipped; targets resolve
//!   relative to `docs/`).
//!
//! Until 2026-08-10 the first check covered only `docs/decisions/*.md` and
//! `docs/project/design-change-records/DCR-*.md` while this module doc claimed
//! the header's broader rule, so the header, the test and the tree were three
//! different things — and `docs/backlog.md` sat unlinked from both the index
//! and `docs/architecture/README.md` with nothing going red (ticket `1347b4`).
//! The check now *is* the header's rule, narrowed by exactly the exclusions
//! `docs/index.md` states to a reader:
//!
//! - `*.ko.md` Korean siblings — generated translations, git-ignored, out of
//!   scope for this assistant by repo convention;
//! - anything under a `docs/` path this repository's `.gitignore` ignores —
//!   today `docs/investigation/`, a bundle regenerated from the code and
//!   untracked since 2026-08-08. An untracked bundle is not the index's to
//!   carry. The exclusion is **read out of `.gitignore`**, not hardcoded, so
//!   it cannot outlive the decision that created it: re-track the bundle and
//!   its files are required again in the same edit;
//! - `docs/index.md` itself, which needs no self-link;
//! - non-Markdown files. `docs/project/phase-state.yaml` is linked because a
//!   reader wants it, not because this test demands it — the index is an index
//!   of documents, and requiring every data file under `docs/` would fire on
//!   files nobody intends to index.
//!
//! An exclusion that fires on files nobody intends to index is how a drift
//! test gets weakened by the next person who trips it, so the set above is
//! deliberately small, stated in the index itself, and asserted by
//! [`the_stated_exclusions_are_the_only_exclusions`].
//!
//! TRACE: EXT-2026-07 docs-index drift test
//! TRACE: ti 1347b4

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Repo root, derived robustly from this crate's manifest dir
/// (`crates/transync` -> `../..`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn docs_dir() -> PathBuf {
    repo_root().join("docs")
}

fn read_index() -> String {
    let index = docs_dir().join("index.md");
    std::fs::read_to_string(&index).unwrap_or_else(|e| {
        panic!(
            "docs/index.md should be readable ({}): {e}",
            index.display()
        )
    })
}

/// Extract the raw target of every inline markdown link `[text](target)` in
/// `text`. A `]` only opens a target when immediately followed by `(`, so
/// stray `]` inside link text is tolerated. Reference-style links (`[a][b]`)
/// carry no `(` and are ignored on purpose.
fn link_targets(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b']' && i + 1 < bytes.len() && bytes[i + 1] == b'(' {
            let start = i + 2;
            if let Some(rel) = bytes[start..].iter().position(|&b| b == b')') {
                out.push(text[start..start + rel].to_string());
                i = start + rel + 1;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Normalize a raw link target to a bare relative path: drop an optional
/// link title (`(path "title")`), a `#fragment` or `?query`, and a leading
/// `./`.
fn normalize_target(raw: &str) -> String {
    let t = raw.trim();
    let t = t.split_whitespace().next().unwrap_or("");
    let t = t.split(['#', '?']).next().unwrap_or("");
    t.strip_prefix("./").unwrap_or(t).to_string()
}

fn is_ko(name: &str) -> bool {
    name.ends_with(".ko.md")
}

fn is_external(target: &str) -> bool {
    let lower = target.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
}

/// The index's own path, relative to `docs/`. It needs no self-link.
const INDEX_SELF: &str = "index.md";

/// The exclusions this repository's `.gitignore` implies for the docs tree,
/// read from the file rather than restated here.
///
/// Two shapes are recognised, and they are the two the tree actually uses:
/// a directory line rooted at `docs/` (`docs/investigation/` -> the prefix
/// `investigation`), and a global suffix pattern (`*.ko.md`, `*.tmp.md`).
/// Everything else — negations, `target/`, `web/.wasm.*` — cannot match a
/// document under `docs/` and is skipped.
#[derive(Debug, Default)]
struct GitignoredDocs {
    /// Directory paths relative to `docs/`, e.g. `investigation`.
    dirs: Vec<String>,
    /// File-name suffixes, e.g. `.ko.md`.
    suffixes: Vec<String>,
}

impl GitignoredDocs {
    fn read() -> Self {
        let path = repo_root().join(".gitignore");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
            panic!(
                "repo .gitignore should be readable ({}): {e}",
                path.display()
            )
        });
        let mut out = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("docs/") {
                if let Some(dir) = rest.strip_suffix('/') {
                    out.dirs.push(dir.to_string());
                }
            } else if let Some(suffix) = line.strip_prefix('*')
                && !suffix.contains('/')
                && suffix.starts_with('.')
            {
                out.suffixes.push(suffix.to_string());
            }
        }
        out
    }

    /// Is this directory (path relative to `docs/`) an ignored bundle?
    fn ignores_dir(&self, rel: &str) -> bool {
        self.dirs.iter().any(|d| d == rel)
    }

    /// Is this file name ignored by a global suffix pattern?
    fn ignores_file_name(&self, name: &str) -> bool {
        self.suffixes.iter().any(|s| name.ends_with(s.as_str()))
    }
}

/// Docs (relative to `docs/`) that MUST be linked from the index: every
/// Markdown document under `docs/` at any depth, minus the exclusions this
/// module's doc comment states.
///
/// Note what is *not* excluded: `docs/project/design-change-records/archive/`
/// and `docs/project/open-issues-archive.md` are archives, and the index links
/// them (marked `*(archived)*`). Archiving a record does not orphan it.
fn required_linked_docs() -> Vec<String> {
    let ignored = GitignoredDocs::read();
    let mut required = Vec::new();
    let mut stack = vec![(docs_dir(), String::new())];

    while let Some((dir, rel)) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("docs tree should be readable ({}): {e}", dir.display()));
        for entry in entries {
            let entry = entry.expect("readable docs tree entry");
            let name = entry.file_name().to_string_lossy().into_owned();
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let file_type = entry.file_type().expect("readable docs tree entry type");
            if file_type.is_dir() {
                if !ignored.ignores_dir(&child) {
                    stack.push((entry.path(), child));
                }
            } else if file_type.is_file()
                && name.ends_with(".md")
                && !is_ko(&name)
                && !ignored.ignores_file_name(&name)
                && child != INDEX_SELF
            {
                required.push(child);
            }
        }
    }

    required.sort();
    required
}

/// The set of normalized relative link targets in the index.
fn indexed_targets() -> BTreeSet<String> {
    link_targets(&read_index())
        .iter()
        .map(|t| normalize_target(t))
        .filter(|t| !t.is_empty())
        .collect()
}

#[test]
fn every_doc_under_docs_is_linked_from_index() {
    let indexed = indexed_targets();
    let missing: Vec<String> = required_linked_docs()
        .into_iter()
        .filter(|req| !indexed.contains(req))
        .collect();
    assert!(
        missing.is_empty(),
        "docs/index.md does not link these docs (add them under the \
         relevant index section — its header promises a single entry point \
         for everything under docs/):\n  {}",
        missing.join("\n  "),
    );
}

/// The widening itself, guarded. Stated structurally rather than by naming
/// files, so a rename cannot make this pass for the wrong reason: before
/// ticket `1347b4` the required set was exactly the two record trees, and
/// every reader-facing document under `docs/` could be dropped from the index
/// with nothing going red.
#[test]
fn the_required_set_is_the_whole_docs_tree_not_just_the_record_trees() {
    let required = required_linked_docs();
    let records =
        |p: &String| p.starts_with("decisions/") || p.starts_with("project/design-change-records/");
    assert!(
        required.iter().any(|p| !records(p)),
        "the required set collapsed back to decisions/ + DCRs only; \
         docs/index.md's header promises more than that",
    );
    for tree in [
        "architecture/",
        "implementation/",
        "project/",
        "superpowers/",
    ] {
        assert!(
            required.iter().any(|p| p.starts_with(tree)),
            "no docs/{tree} document is required — the walk stopped reaching it",
        );
    }
    assert!(
        required.iter().any(|p| !p.contains('/')),
        "no top-level docs/*.md document is required — `docs/backlog.md` sat \
         unlinked for exactly this reason",
    );
}

/// The exclusions are the ones `docs/index.md` states to a reader, and no
/// others. A drift test that fires on files nobody intends to index gets
/// weakened by whoever trips it next, so the excluded set is small, stated,
/// and checked here. Nothing asserts these files *exist* — a fresh clone has
/// no Korean siblings and no `docs/investigation/`, both being git-ignored.
#[test]
fn the_stated_exclusions_are_the_only_exclusions() {
    let required = required_linked_docs();
    let ignored = GitignoredDocs::read();

    for path in &required {
        assert!(
            !is_ko(path),
            "{path}: Korean siblings are out of scope and must not be required",
        );
        assert_ne!(
            path, INDEX_SELF,
            "the index must not be required to link itself",
        );
        assert!(
            path.ends_with(".md"),
            "{path}: only Markdown documents are required",
        );
        let top = path.split('/').next().unwrap_or_default();
        assert!(
            !ignored.ignores_dir(top),
            "{path}: sits under a git-ignored bundle and must not be required",
        );
    }

    // The `.gitignore` reader must actually see this repository's rules —
    // an empty parse would silently turn every exclusion off.
    assert!(
        ignored.suffixes.iter().any(|s| s == ".ko.md"),
        "the .gitignore reader stopped recognising the `*.ko.md` rule; the \
         exclusions it derives can no longer be trusted",
    );
}

#[test]
fn every_relative_link_in_index_resolves() {
    let docs = docs_dir();
    let mut broken = Vec::new();
    for raw in link_targets(&read_index()) {
        let target = normalize_target(&raw);
        if target.is_empty() || is_external(&target) {
            continue;
        }
        let resolved = docs.join(&target);
        if !resolved.exists() {
            broken.push(format!("{target}  ->  {}", resolved.display()));
        }
    }
    assert!(
        broken.is_empty(),
        "docs/index.md links target(s) that do not exist on disk \
         (relative to docs/):\n  {}",
        broken.join("\n  "),
    );
}
