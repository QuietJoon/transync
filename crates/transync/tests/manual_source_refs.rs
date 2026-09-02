//! Every `resource:` path in the manual bundle's frontmatter resolves.
//!
//! # Why this exists
//!
//! `manual/` is derived one-way from `docs/` and **nothing gated it**. Two
//! consequences were observed rather than predicted (ti `7e2fd0`, ti `0eee5c`):
//! the `transync serve` deferred-stub prose survived three weeks past the
//! commit that retired that contract, and two frontmatter `resource:` paths
//! pointed at files that had not existed since the moves that broke them —
//! `crates/transync-syntax/src/htmlseg.rs`, lifted into its own crate by
//! DCR-0032, and an ADR the archive prune relocated. Both were found by hand.
//!
//! # What it does and does not cover
//!
//! This is the **cheap half** of ti `7e2fd0`, and deliberately only that half.
//! A `resource:` path either resolves or it does not, so checking costs a
//! directory probe, reads no prose, and — the point — creates **no coupling
//! between the docs bundle and the manual bundle**. `docs_cli_flags_drift.rs`
//! stops at `docs/` on purpose, and widening it would be a decision about
//! whether a derived bundle is a gated artifact at all.
//!
//! So this does **not** catch prose drift: a manual page that describes a flag
//! wrongly, or omits one, still passes here. That remains ti `7e2fd0`'s open
//! question. What it does catch is every future instance of the class that
//! actually bit twice, at the commit that breaks it.
//!
//! Korean pages are **included**. The convention that `*.ko.md` files are out
//! of scope governs what a human or an assistant reads; this test extracts path
//! values and never looks at prose, and a weld that skipped half the bundle
//! would leave exactly half the references unguarded.
//!
//! TRACE: ti 7e2fd0

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

/// Every Markdown file under `manual/`, recursively.
fn manual_pages(root: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(e) => panic!("{} should be readable: {e}", dir.display()),
        };
        for entry in entries {
            let path = entry.expect("a readable dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "md") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(&root.join("manual"), &mut out);
    out.sort();
    out
}

/// The `resource:` values in one page's frontmatter `sources:` block.
///
/// Line-based rather than a YAML parse, so this test adds no dependency to a
/// crate whose dependency set is a contract. The shape it reads is the one the
/// generator writes and every page carries:
///
/// ```text
/// sources:
///   - { id: contracts, resource: docs/architecture/contracts.md }
/// ```
///
/// Anything that is not that shape is skipped rather than guessed at — and the
/// floor assertions below are what keep "skipped everything" from reading as
/// "found no problems".
fn resource_paths(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        let Some(rest) = line.split_once("resource:") else {
            continue;
        };
        let value = rest
            .1
            .trim()
            .trim_end_matches('}')
            .trim()
            .trim_matches('"')
            .trim_matches('\'');
        if !value.is_empty() {
            out.push(value.to_string());
        }
    }
    out
}

#[test]
fn every_manual_source_reference_points_at_a_file_that_exists() {
    let root = repo_root();
    let pages = manual_pages(&root);

    // Anti-vacuity, both directions. A parser that stopped matching, or a
    // bundle that moved, must fail here rather than report a clean sweep.
    assert!(
        pages.len() >= 30,
        "found only {} manual pages — the walk broke, the bundle did not shrink",
        pages.len()
    );

    let mut refs = 0usize;
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for page in &pages {
        let body = std::fs::read_to_string(page)
            .unwrap_or_else(|e| panic!("{} should be readable: {e}", page.display()));
        for value in resource_paths(&body) {
            refs += 1;
            if !root.join(&value).exists() {
                let rel = page
                    .strip_prefix(&root)
                    .unwrap_or(page.as_path())
                    .display()
                    .to_string();
                missing.insert(format!("{value}  (named by {rel})"));
            }
        }
    }

    assert!(
        refs >= 200,
        "found only {refs} `resource:` references across {} pages — the \
         extraction broke, the bundle did not empty",
        pages.len()
    );

    assert!(
        missing.is_empty(),
        "the manual names {} source file(s) that do not exist. A `resource:` \
         path is a provenance claim: it says \"this page was written from that \
         file\", so a dangling one means the page's warrant moved and nobody \
         followed it. Repoint each at where the file went — or drop the entry \
         if the source is genuinely gone:\n  {}",
        missing.len(),
        missing.into_iter().collect::<Vec<_>>().join("\n  "),
    );
}
