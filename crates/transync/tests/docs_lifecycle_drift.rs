//! Guard the document-kind lifecycle (DCR-0054).
//!
//! Every tracked document is one of four kinds, and the kind decides how it
//! changes:
//!
//! - **Living** — says what is true now, and is edited in place. It carries no
//!   amendment construct: no `## Amendment` section, no `> **Correction`
//!   blockquote, no "Appended, not a rewrite" preamble, no `**Numbering
//!   note.**`, no "It read, until that date" recital.
//! - **Register** — holds open work. A resolved entry moves to its archive in
//!   the commit that closes it, so a live register accumulates no resolved
//!   entries.
//! - **Record** — DCRs, released `CHANGELOG` sections, plans, specs. Write-once:
//!   its words never change and nothing is appended to it. Records are
//!   therefore **not checked here** — this file guards the two kinds that are
//!   allowed to change.
//! - **Archive** — verbatim, append-only, never read for current truth. Also
//!   not checked.
//!
//! This gate is a **ratchet, not a cleanup**. It ships green over the tree as
//! it stands: every living document that carries a construct today is listed in
//! `CONSTRUCTS_ALLOWED_TODAY`, and every budgeted document's budget is its
//! current size. What the ratchet forbids is *regrowth* —
//!
//! - a living document that is **not** on the allowlist may not gain a
//!   construct;
//! - a document **on** the allowlist that has become clean must be removed from
//!   the list (so the list only ever shrinks);
//! - a budgeted document may not exceed its budget, and a budget more than
//!   `BUDGET_SLACK` above the real size is stale and must be lowered.
//!
//! Each entry therefore comes off the list or down in size as a compaction wave
//! lands, and nothing silently grows back between waves. The campaign is
//! `docs/superpowers/specs/2026-09-13-docs-compaction-design.md`.
//!
//! Why a test and not the pre-commit hook: the hook runs fmt, clippy, the
//! `wasm32` gate, the rustdoc gate and biome — it runs no docs gate at all. The
//! docs-drift family runs in `cargo test --workspace`, which is where this
//! belongs beside `docs_index_drift`, `docs_gate_claims_drift`,
//! `docs_ownership_drift` and `docs_browser_suite_drift`.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

/// The textual machinery of revision. A living document that carries one of
/// these is recording a change to itself instead of stating what is true.
///
/// Matched as plain substrings, except the entries whose pattern begins with a
/// line anchor, which are matched per line after trimming leading whitespace —
/// `## Amendment` must not fire on a sentence that merely mentions one.
const CONSTRUCTS: &[Construct] = &[
    Construct::heading("Amendment"),
    Construct::substring("> **Correction"),
    Construct::substring("Appended, not a rewrite"),
    Construct::substring("**Numbering note.**"),
    Construct::substring("It read, until that date"),
    Construct::substring("Original note follows"),
];

enum Construct {
    /// A markdown heading at any level whose text starts with this word.
    Heading(&'static str),
    /// A literal substring anywhere in the document.
    Substring(&'static str),
}

impl Construct {
    const fn heading(word: &'static str) -> Self {
        Construct::Heading(word)
    }
    const fn substring(text: &'static str) -> Self {
        Construct::Substring(text)
    }

    fn name(&self) -> &'static str {
        match self {
            Construct::Heading(w) => w,
            Construct::Substring(s) => s,
        }
    }

    fn found_in(&self, body: &str) -> bool {
        match self {
            Construct::Heading(word) => body.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with('#')
                    && line.trim_start_matches('#').trim_start().starts_with(*word)
            }),
            Construct::Substring(text) => body.contains(text),
        }
    }
}

/// Living documents that carry a construct **today**, with the wave that is
/// scheduled to take it off this list.
///
/// This list may only shrink. Adding a path to it is not a repair — it is a
/// decision to let a living document keep a piece of its own revision history,
/// and it belongs in a DCR, not here.
const CONSTRUCTS_ALLOWED_TODAY: &[&str] = &[
    // wave 4 — the recitals move to docs/architecture/contracts-history.md
    "docs/architecture/contracts.md",
    // wave 5 — the amendment sections move to docs/decisions/archive/adr-amendments.md
    "docs/decisions/0001-block-level-alignment-as-sync-currency.md",
    "docs/decisions/0002-http-free-core-with-translator-trait.md",
    "docs/decisions/0003-cargo-workspace-with-provider-crates.md",
    "docs/decisions/0004-comrak-as-gfm-parser.md",
    "docs/decisions/0006-renderer-output-shape.md",
    "docs/decisions/0007-renderer-wrapper-div-for-tables-and-code.md",
    "docs/decisions/0009-reject-retry-policy-changes.md",
    "docs/decisions/0011-lan-bind-in-convenience-wrappers.md",
    "docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md",
    "docs/decisions/0013-opaque-language-labels.md",
    "docs/decisions/0016-whole-document-in-memory.md",
    "docs/decisions/0017-batch-terminal-failures-abort-the-run.md",
    "docs/decisions/0018-html-content-translation-via-segment-extraction.md",
    "docs/decisions/0019-wasm-demo-layer.md",
];

/// A budget more than this far above the real size is stale: the document has
/// been compacted and its ceiling was never brought down with it.
const BUDGET_SLACK: u64 = 8 * 1024;

/// High-water marks for the documents this campaign targets, in bytes.
///
/// A budget goes **down** as a wave lands and never up. Raising one is a
/// decision that a document is allowed to be bigger, which is exactly the
/// conversation this gate exists to force.
const BUDGETS: &[(&str, u64)] = &[
    ("CLAUDE.md", 22_528),     // wave 1 done; the rest is rules, not history
    ("docs/index.md", 41_984), // wave 1 → 22_528
    ("docs/architecture/contracts.md", 307_200), // wave 4 → 163_840
    ("docs/project/status.md", 30_720), // wave 2 done
    ("docs/project/release-checklist.md", 47_104), // wave 2
    ("docs/project/phase-state.yaml", 6_144), // wave 2 done
    ("docs/project/open-issues.md", 121_856), // wave 2
    ("docs/backlog.md", 261_120), // wave 2
    ("CHANGELOG.md", 96_256),  // wave 3 done
    ("docs/Developer_Guide.md", 84_992), // wave 3
    ("docs/Troubleshooting.md", 40_960), // wave 3
    ("docs/implementation/module-map.md", 39_936), // wave 3
];

/// `docs/project/open-issues.md` is a live register: a resolved entry belongs
/// in `open-issues-archive.md`. Fourteen are still inline, and wave 2 moves the
/// ones whose resolution it can verify against code or a shipped artifact — a
/// `RESOLVED` label is not itself evidence, so an entry whose evidence is
/// absent stays here on purpose.
const RESOLVED_INLINE_HIGH_WATER: usize = 14;

const RESOLVED_MARKER: &str = "**Status:** RESOLVED";

fn is_korean_sibling(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".ko.md"))
}

fn markdown_in(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .filter(|p| !is_korean_sibling(p))
        .collect();
    out.sort();
    out
}

/// The living set: documents whose job is to say what is true now.
///
/// Deliberately excluded — `docs/project/design-change-records/` and every
/// `archive/` (records), `docs/superpowers/` (plans and specs are records), the
/// four self-declared dated snapshots and the incident records under
/// `docs/project/` (their staleness is their content), `docs/investigation/`
/// (gitignored, regenerated from code), and `docs/backlog.md` +
/// `open-issues.md`, which are registers and are checked by the register test
/// instead.
fn living_documents() -> Vec<PathBuf> {
    let root = repo_root();
    let mut files = vec![root.join("README.md"), root.join("CLAUDE.md")];

    // Top-level reader docs, minus the one register that lives among them.
    for path in markdown_in(&root.join("docs")) {
        if path.file_name().and_then(|n| n.to_str()) == Some("backlog.md") {
            continue;
        }
        files.push(path);
    }

    files.extend(markdown_in(&root.join("docs/architecture")));
    files.extend(markdown_in(&root.join("docs/implementation")));
    // ADR decision bodies are living: the authority hierarchy ranks their
    // rationale as current truth that "must be fixed" when it goes stale.
    // `docs/decisions/archive/` is not read — a retired ADR is a record.
    files.extend(markdown_in(&root.join("docs/decisions")));

    files.push(root.join("docs/project/status.md"));
    files.push(root.join("docs/project/release-checklist.md"));

    files.retain(|p| p.is_file());
    files
}

fn relative(path: &Path) -> String {
    path.strip_prefix(repo_root())
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read(path: &Path) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} should be readable: {e}", relative(path)))
}

#[test]
fn no_living_document_gains_an_amendment_construct() {
    let allowed: BTreeSet<&str> = CONSTRUCTS_ALLOWED_TODAY.iter().copied().collect();
    let mut offences = Vec::new();

    for path in living_documents() {
        let rel = relative(&path);
        if allowed.contains(rel.as_str()) {
            continue;
        }
        let body = read(&path);
        for construct in CONSTRUCTS {
            if construct.found_in(&body) {
                offences.push(format!("{rel}: {}", construct.name()));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "a living document says what is true now — it does not carry the machinery of its own \
         revision. Edit the document in place and cite the record as `(DCR-00NN)` instead of \
         appending a note. If the history genuinely has to stay in the document, that is a DCR \
         decision, not an allowlist entry. Offending:\n  {}",
        offences.join("\n  ")
    );
}

#[test]
fn the_construct_allowlist_only_shrinks() {
    let mut clean = Vec::new();
    let mut missing = Vec::new();

    for rel in CONSTRUCTS_ALLOWED_TODAY {
        let path = repo_root().join(rel);
        if !path.is_file() {
            missing.push((*rel).to_string());
            continue;
        }
        let body = read(&path);
        if !CONSTRUCTS.iter().any(|c| c.found_in(&body)) {
            clean.push((*rel).to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "CONSTRUCTS_ALLOWED_TODAY names files that no longer exist. A moved or deleted document \
         leaves the list with it:\n  {}",
        missing.join("\n  ")
    );
    assert!(
        clean.is_empty(),
        "these documents have been compacted and carry no amendment construct any more, so they \
         must come off CONSTRUCTS_ALLOWED_TODAY — leaving them on it would let the construct come \
         back unnoticed:\n  {}",
        clean.join("\n  ")
    );
}

#[test]
fn budgeted_documents_stay_within_their_budget() {
    let mut over = Vec::new();
    let mut stale = Vec::new();

    for (rel, budget) in BUDGETS {
        let path = repo_root().join(rel);
        // `CLAUDE.md` is budgeted but gitignored by owner decision, so it is
        // absent from a fresh clone and from CI. An absent file is skipped
        // rather than failed: this gate exists to stop documents growing, and a
        // document that is not there cannot grow.
        let Ok(meta) = fs::metadata(&path) else {
            continue;
        };
        let actual = meta.len();

        if actual > *budget {
            over.push(format!("{rel}: {actual} bytes, budget {budget}"));
        } else if actual + BUDGET_SLACK < *budget {
            stale.push(format!("{rel}: {actual} bytes, budget still {budget}"));
        }
    }

    assert!(
        over.is_empty(),
        "a budgeted document grew past its ceiling. Compact it, or raise the budget in a change \
         that says why the document is allowed to be bigger:\n  {}",
        over.join("\n  ")
    );
    assert!(
        stale.is_empty(),
        "these documents have been compacted well below their budget, so the budget no longer \
         ratchets anything — lower it to the new size:\n  {}",
        stale.join("\n  ")
    );
}

#[test]
fn a_live_register_does_not_accumulate_resolved_entries() {
    let path = repo_root().join("docs/project/open-issues.md");
    let body = read(&path);
    let inline = body.lines().filter(|l| l.contains(RESOLVED_MARKER)).count();

    assert!(
        inline <= RESOLVED_INLINE_HIGH_WATER,
        "docs/project/open-issues.md holds {inline} resolved entries, above the high-water mark \
         of {RESOLVED_INLINE_HIGH_WATER}. Its own rule is that a resolved entry moves verbatim to \
         docs/project/open-issues-archive.md in the commit that closes it."
    );
    assert!(
        inline >= RESOLVED_INLINE_HIGH_WATER,
        "docs/project/open-issues.md is down to {inline} resolved entries — lower \
         RESOLVED_INLINE_HIGH_WATER to {inline} so the register cannot quietly refill to \
         {RESOLVED_INLINE_HIGH_WATER}."
    );
}
