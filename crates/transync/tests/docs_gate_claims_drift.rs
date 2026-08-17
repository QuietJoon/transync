//! Guard the two claims living documents make about how this repository is
//! gated: that the pre-commit hook is never bypassed, and that there is no CI
//! behind it.
//!
//! Both were broken at once in `docs/Troubleshooting.md`, which offered
//! `git commit --no-verify` "only when you're certain" and justified it with
//! "the hook catches things that would otherwise blow up in CI" — while
//! `docs/project/release-checklist.md` said the hook "is never bypassed with
//! `--no-verify`", and while no CI has ever existed here (no workflow file in
//! the tree, none anywhere in the history). A bypassed hook is not caught
//! later by anything automatic; it is simply an unchecked commit until a human
//! runs `scripts/smoke.sh`.
//!
//! Three checks:
//!
//! - every `--no-verify` in a guarded living document sits in a paragraph that
//!   also says `never`, so the mention can be the prohibition but not an
//!   escape hatch;
//! - no CI configuration exists in the tree, which is what makes "there is no
//!   CI" true. If CI ever lands, this test fails and names the documents that
//!   have to stop saying otherwise — the claim is load-bearing prose, not a
//!   passing remark;
//! - no guarded living file *says* a CI runs anything. The first two checks
//!   missed exactly that: `docs/implementation/module-map.md` kept a table cell
//!   reading "stub for CI" one table above its own "there is no CI" answer
//!   (2026-08-07 review of this ticket), and
//!   `crates/transync-cli/Cargo.toml` documented the `test-stub-provider`
//!   feature as "Used by smoke + CI". A `CI` token in a guarded file must sit
//!   in a segment that also says `no CI`, so the mention can be the denial but
//!   not the claim.
//!
//! Snapshot documents (ADRs, DCRs, dated plans and status entries) are not
//! guarded: they record what was true when written. The wave plans under
//! `docs/superpowers/plans/` say "never `--no-verify`" and are correct as
//! written, but they are history, so they are not this test's business.
//!
//! TRACE: ti 8c5156

use std::path::{Path, PathBuf};

/// Living documents that tell a reader how to work in this repository today.
const GUARDED: &[&str] = &[
    "docs/Troubleshooting.md",
    "docs/Developer_Guide.md",
    "docs/Quick_Start.md",
    "docs/project/release-checklist.md",
    "docs/implementation/module-map.md",
    "README.md",
];

/// The bypass this project forbids.
const BYPASS: &str = "--no-verify";

/// The one manifest that documents the stub-provider feature, and therefore
/// the one non-document that has a reason to name the runs that use it. It is
/// guarded alongside the documents because it got the same claim wrong.
const STUB_FEATURE_MANIFEST: &str = "crates/transync-cli/Cargo.toml";

/// The only way a guarded file may name CI: as a denial.
const CI_DENIAL: &str = "no CI";

/// Paths that would mean a CI system exists. A directory or a file at any of
/// them is a CI configuration; nothing else in this tree is.
const CI_CONFIG: &[&str] = &[
    ".github/workflows",
    ".gitlab-ci.yml",
    ".circleci",
    ".travis.yml",
    "azure-pipelines.yml",
    "Jenkinsfile",
];

/// Documents whose prose states that this repository has no CI. Named here so
/// the failure message can send a future CI-adding change straight at them.
const DOCS_CLAIMING_NO_CI: &[&str] = &[
    "docs/Troubleshooting.md",
    "docs/implementation/module-map.md",
];

/// Repo root, derived robustly from this crate's manifest dir
/// (`crates/transync` -> `../..`). Same idiom as `docs_index_drift.rs`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} should be readable ({}): {e}", path.display()))
}

/// Paragraphs of `text` — blank-line-delimited runs of lines — as
/// `(1-based line number of the first line, paragraph text)`.
///
/// A paragraph is the unit a reader takes a rule from: a prohibition three
/// paragraphs up does not disarm a bypass offered here, and a numbered
/// checklist step is one paragraph however many lines it wraps over.
fn paragraphs(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut buf: Vec<&str> = Vec::new();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            if !buf.is_empty() {
                out.push((start + 1, buf.join("\n")));
                buf.clear();
            }
            continue;
        }
        if buf.is_empty() {
            start = i;
        }
        buf.push(line);
    }
    if !buf.is_empty() {
        out.push((start + 1, buf.join("\n")));
    }
    out
}

/// Segments of `text` — the unit a *claim* is read in, which is finer than the
/// unit a *rule* is read in. Prose still splits on blank lines, but a Markdown
/// table carries no blank line, so its rows would otherwise vouch for each
/// other: that is precisely how "stub for CI" survived in a table whose next
/// row said "there is no CI". Rows of a table are therefore split apart.
fn claim_segments(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (start, para) in paragraphs(text) {
        if para.lines().any(|line| line.trim_start().starts_with('|')) {
            for (offset, line) in para.lines().enumerate() {
                out.push((start + offset, line.to_string()));
            }
        } else {
            out.push((start, para));
        }
    }
    out
}

/// Whether `text` uses `CI` as a standalone word, so that `ASCII`, `EXPLICIT`
/// and `specific` do not count as mentions.
fn mentions_ci(text: &str) -> bool {
    let bytes = text.as_bytes();
    let boundary = |b: u8| !b.is_ascii_alphanumeric() && b != b'_';
    text.match_indices("CI").any(|(at, _)| {
        let before = at == 0 || boundary(bytes[at - 1]);
        let after = at + 2 >= bytes.len() || boundary(bytes[at + 2]);
        before && after
    })
}

#[test]
fn no_living_doc_sanctions_bypassing_the_pre_commit_hook() {
    let mut offenders = Vec::new();
    for rel in GUARDED {
        let text = read(rel);
        for (number, para) in paragraphs(&text) {
            if !para.contains(BYPASS) {
                continue;
            }
            if !para.to_ascii_lowercase().contains("never") {
                offenders.push(format!("{rel}:{number}: {}", para.replace('\n', " ")));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a living document mentions `{BYPASS}` without forbidding it in the \
         same paragraph. The pre-commit hook (`scripts/hooks/pre-commit`) runs \
         fmt, clippy, the wasm gate and the rustdoc gate, and nothing else runs \
         them by itself — there is no CI here — so a bypass is not a shortcut \
         past a slow gate, it is an unchecked commit. Say what to run instead \
         (ti 8c5156), the way `docs/project/release-checklist.md` steps 7 and \
         24 do:\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn the_repository_still_has_no_ci() {
    let root = repo_root();
    let found: Vec<&str> = CI_CONFIG
        .iter()
        .copied()
        .filter(|rel| root.join(rel).exists())
        .collect();
    assert!(
        found.is_empty(),
        "CI configuration landed at {} — which is good news, and it falsifies \
         prose that several documents carry. Update the living ones ({}) so \
         they describe the gate that now exists, then delete or rewrite this \
         test; snapshot records (DCR-0015, the resolved `open-issues.md` \
         entries) keep what was true when they were written (ti 8c5156).",
        found.join(", "),
        DOCS_CLAIMING_NO_CI.join(", "),
    );
}

#[test]
fn no_living_doc_claims_a_ci_runs_anything() {
    let mut offenders = Vec::new();
    for rel in GUARDED
        .iter()
        .copied()
        .chain(std::iter::once(STUB_FEATURE_MANIFEST))
    {
        let text = read(rel);
        for (number, segment) in claim_segments(&text) {
            if mentions_ci(&segment) && !segment.contains(CI_DENIAL) {
                offenders.push(format!("{rel}:{number}: {}", segment.replace('\n', " ")));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "a living file names CI without denying it. This repository has none \
         (see `the_repository_still_has_no_ci`): the automated runs are the \
         `test-stub-provider` echo `Translator` under `cargo test`, \
         `scripts/smoke.sh` and `scripts/test-browser.sh`, all human-invoked. \
         Name the run that really happens instead of a pipeline that does not, \
         or say `{CI_DENIAL}` in the same segment (ti 8c5156):\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn paragraph_splitter_keeps_a_rule_with_the_text_it_governs() {
    let doc = "intro line\n\nfirst para line one\nfirst para line two\n\n\
               7. a numbered step\n   that wraps, and says never here\n";
    let paras = paragraphs(doc);
    assert_eq!(paras.len(), 3);
    assert_eq!(paras[0], (1, "intro line".to_string()));
    // Line numbers point at the paragraph's first line, blank lines skipped.
    assert_eq!(paras[1].0, 3);
    assert_eq!(paras[2].0, 6);
    // A wrapped checklist step is one paragraph, so a `never` on its second
    // line still governs a mention on its first.
    assert!(paras[2].1.contains("never"));
}

#[test]
fn segment_splitter_does_not_let_one_table_row_vouch_for_another() {
    let doc = "prose that wraps\nover two lines\n\n\
               | SCN | Integration |\n|-----|-------------|\n\
               | 12  | stub for CI |\n| 13  | no CI here  |\n";
    let segments = claim_segments(doc);
    // The prose stays whole; each of the four table lines stands alone.
    assert_eq!(segments.len(), 5);
    assert_eq!(segments[0].1, "prose that wraps\nover two lines");
    assert_eq!(segments[1].0, 4);
    assert_eq!(segments[4].0, 7);
    // The offending row is judged without help from the row below it, which is
    // the row that carries the denial.
    assert!(mentions_ci(&segments[3].1));
    assert!(!segments[3].1.contains(CI_DENIAL));
    assert!(segments[4].1.contains(CI_DENIAL));
}

#[test]
fn ci_detector_reads_words_not_substrings() {
    assert!(mentions_ci("stub for CI)"));
    assert!(mentions_ci("CI logs"));
    assert!(mentions_ci("a CI-adding change"));
    // The three words that made a naive substring search useless here.
    assert!(!mentions_ci("EXPLICIT curated re-export list"));
    assert!(!mentions_ci("ASCII case"));
    assert!(!mentions_ci("a specific model"));
}
