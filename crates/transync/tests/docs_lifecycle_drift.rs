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
const CONSTRUCTS_ALLOWED_TODAY: &[&str] = &[];

/// A budget more than this far above the real size is stale: the document has
/// been compacted and its ceiling was never brought down with it.
const BUDGET_SLACK: u64 = 8 * 1024;

/// High-water marks for the documents this campaign targets, in bytes.
///
/// A budget goes **down** as a wave lands and never up. Raising one is a
/// decision that a document is allowed to be bigger, which is exactly the
/// conversation this gate exists to force.
///
/// Every ceiling is a whole KiB. A wave that lands lowers its document's
/// ceiling to the smallest whole KiB that holds the compacted file, and the
/// trailing comment names the wave: `docs/index.md` (wave 1),
/// `docs/backlog.md` (wave 2) and `docs/architecture/contracts.md` (wave 4)
/// came down that way. `docs/project/status.md` came down with the Phase 1–5
/// evidence tables that moved to `status-archive.md`, and keeps about 2 KiB of
/// working room.
///
/// A raise is the smallest whole-KiB increment that fits text a recorded
/// change added, and the trailing comment names the change. What the raises
/// below paid for is interface rules or state, not history: DCR-0058's Google
/// surfaces; DCR-0055's frontmatter header, DCR-0056's lint leg and DCR-0057's
/// packing policy and CLI pair; DCR-0059's model-generation floor; DCR-0060's
/// Vertex express mode; the shipped 32-unit default (ti `f17fc9`); the record
/// roster `phase-state.yaml` carries; and the two lines `CLAUDE.md` gained,
/// the `--google-auth` selector and the manifest's version. None is a licence
/// for the next entry, and the compaction targets in the trailing comments
/// are unchanged by them.
///
/// DCR-0061 paid for two more — one index line, and §9's corrected exit code,
/// loopback rule and `MAX_TOKENS` remediation, each of which is an interface
/// rule a caller has to be able to read.
///
/// The 2026-10-13 live-endpoint run paid for one more, and it is the kind of
/// growth this gate is meant to allow: evidence about the shipped artifact.
/// `CHANGELOG.md` records which provider surfaces answered a real request and
/// which did not.
///
/// `docs/backlog.md` moved twice on 2026-10-13 and the pair is worth reading
/// together. It came **down** first, which is the direction this table is
/// supposed to move: three entries resolved at once — both live-gate entries
/// and the unopened breaking window — and each left for `backlog-archive.md`
/// under the Register rule. (The archive is unbudgeted on purpose: it is
/// append-only and never read for current truth, so growth there is not the
/// growth this gate exists to stop.) It then went back up by 3 KiB for the
/// owner's stated v0.6.0 release condition and for the recorded deferral on
/// `v050-live-gate-unrun` — interface rules again, not history: the header
/// now has to say that its counts are **not** a gate verdict, and an entry
/// the owner set aside has to carry the trigger that would wake it.
const BUDGETS: &[(&str, u64)] = &[
    ("CLAUDE.md", 23_552), // wave 1 done; the rest is rules, not history; +1 KiB DCR-0060's `--google-auth` selector and the manifest version line
    ("docs/index.md", 27_648), // wave 1 landed, → 22_528 not reached; +1 KiB DCR-0061
    ("docs/architecture/contracts.md", 318_464), // wave 4 landed, → 163_840 not reached; +2 KiB DCR-0061's §9 corrections, +3 KiB DCR-0063 (R0013-0002), +3 KiB DCR-0063 (R0012-0015), +3 KiB DCR-0063 (R0013-0003), +1 KiB DCR-0063 (R0012-0010, R0012-0018, R0012-0024)
    ("docs/project/status.md", 24_576),          // wave 2 done; ~2 KiB of working room
    ("docs/project/release-checklist.md", 48_128), // wave 2; +1 KiB DCR-0058
    ("docs/project/phase-state.yaml", 7_168),    // wave 2 done; +1 KiB DCR-0054..DCR-0060 roster
    ("docs/project/open-issues.md", 49_152), // wave 2 done; lowered on 2026-10-14 when OI-0041 moved verbatim to the archive under the Register rule
    ("docs/backlog.md", 113_664), // wave 2 done; lowered to 102 KiB on 2026-10-13 when three entries archived in one day, then +3 KiB the same day for v0.6.0's stated release condition and the recorded deferral its gate verdict depends on, +4 KiB on 2026-10-14 for the two 4B-verdicted claims the agreement rule adopts, +2 KiB when the reviews entry was corrected: a Type 1 filed on a false premise became a Type 2 carrying why, which is the growth a register is for
    ("CHANGELOG.md", 118_784), // wave 3 done; +1 KiB DCR-0058, +1 KiB DCR-0055..0057, +1 KiB DCR-0059, DCR-0060, ti f17fc9, +5 KiB the 2026-10-13 live-gate roster, +4 KiB DCR-0061's four fixes, +1 KiB DCR-0063 (R0012-0004, R0012-0007, R0012-0008, R0012-0009), +1 KiB DCR-0063 (R0013-0002), +2 KiB DCR-0063 (R0012-0015), +2 KiB DCR-0063 (R0013-0003), +2 KiB DCR-0063 (R0012-0006, R0012-0012, R0012-0013, R0012-0025), +2 KiB DCR-0063 (R0012-0010, R0012-0018, R0012-0024)
    ("docs/Developer_Guide.md", 95_232), // wave 3; +6 KiB DCR-0058, +3 KiB DCR-0055, DCR-0057, DCR-0059, DCR-0060, ti f17fc9, +1 KiB SMOOTHING_FACTOR is a per-elapsed-ms lerp (R0011-0094, ti 336bf0c6)
    ("docs/Troubleshooting.md", 40_960), // wave 3
    ("docs/implementation/module-map.md", 41_984), // wave 3; +1 KiB DCR-0058, +1 KiB DCR-0063 (R0013-0003)
];

/// `docs/project/open-issues.md` is a live register: a resolved entry belongs
/// in `open-issues-archive.md`. Wave 2 moved the ten whose resolution verified
/// against code or a shipped artifact, one clause at a time. Four stayed: a
/// `RESOLVED` label is not itself evidence, and those four still owed
/// something. OI-0041's residual then landed, so three do.
const RESOLVED_INLINE_HIGH_WATER: usize = 3;

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

    // Top-level reader docs, minus the one register that lives among them
    // and the `*-archive.md` siblings, which DCR-0054 classes as archives.
    for path in markdown_in(&root.join("docs")) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        if name == "backlog.md" || name.ends_with("-archive.md") {
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
