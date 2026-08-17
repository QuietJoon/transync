//! Guard against drift between the shipped `ExitCode` enum and every living
//! document that publishes its numbers.
//!
//! The exit codes are a contract with a calling script, and unlike the flag
//! surface next door there is no `--help` to derive them from: the enum is
//! the authority and four documents copy it by hand. Extending the table by
//! two (ti `e62b59`) meant editing all five artifacts at once, and this test
//! is what makes the *next* extension impossible to do halfway — a stale
//! exit-code table is a defect this repository has paid for repeatedly.
//!
//! Three checks, all keyed on the numbers rather than on prose:
//!
//! - **`contracts.md` §6 lists exactly the codes the enum defines.** §6 is
//!   the contract; a code missing there is undocumented, and a code listed
//!   there that no longer exists is a promise the binary cannot keep.
//! - **`docs/Developer_Guide.md`'s "Exit codes" table lists exactly the same
//!   set.** It is the reader-facing copy, and it is the one that drifted.
//! - **Every `exit codes 0..N` range claim in a living document names the
//!   real maximum.** That single-character claim is the cheapest thing in
//!   the repository to leave stale, and it appears in four places.
//!
//! Snapshot documents are deliberately outside the guarded set:
//! `design-baseline.md` and `stub-manifest.md` each carry an explicit "as of
//! its date, deliberately not rewritten" banner, and `0..5` was true when
//! they were written — rewriting them would destroy the record they exist to
//! keep. They are named in `HISTORICAL` below so the exclusion is a recorded
//! decision. (`implementation-slice-checklists.md` is a third such snapshot,
//! but its claim is about SL-12's test coverage — `exit codes 1..4` — rather
//! than the range, so no check reaches it in either direction.)
//!
//! What this cannot check is *meaning* — whether the sentence beside code 6
//! describes code 6. Prose is left to review; the numbers are what a script
//! branches on, so the numbers are what is welded.
//!
//! TRACE: contracts.md §6
//! TRACE: ti e62b59

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Repo root, derived from this crate's manifest dir (`crates/transync-cli`
/// -> `../..`). Same idiom as `docs_cli_flags_drift.rs`.
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

/// Living documents that state the range as `exit codes 0..N`.
///
/// An allow-list rather than a tree walk, because the same phrase appears in
/// the historical documents below and is correct there.
const RANGE_CLAIMS: &[&str] = &[
    "README.md",
    "docs/Developer_Guide.md",
    "docs/project/design-baseline-2026-07.md",
    "crates/transync-cli/src/translate_cmd.rs",
];

/// Snapshot documents that state an older range on purpose. Listed so the
/// exclusion is a recorded decision rather than an oversight, and asserted
/// to still say what they say — if one is ever rewritten into a live claim,
/// that is a decision someone should make deliberately.
const HISTORICAL: &[&str] = &[
    "docs/project/design-baseline.md",
    "docs/project/stub-manifest.md",
];

/// The codes `crates/transync-cli/src/error.rs` defines, scraped from the
/// `#[repr(i32)] enum ExitCode` body.
///
/// Scraped as text rather than read through the type, because `transync-cli`
/// is a binary-only crate: an integration test cannot import `ExitCode`, and
/// the source file is the authority either way.
fn enum_codes() -> BTreeSet<u32> {
    let src = read("crates/transync-cli/src/error.rs");
    let body = src
        .split_once("pub enum ExitCode {")
        .expect("error.rs should declare `pub enum ExitCode {`")
        .1
        .split_once('}')
        .expect("the ExitCode enum body should be brace-delimited")
        .0;

    let mut codes = BTreeSet::new();
    for line in body.lines() {
        let line = line.trim();
        // `Variant = N,` — doc comments and blank lines yield nothing.
        let Some((_, rhs)) = line.split_once('=') else {
            continue;
        };
        let value = rhs.trim().trim_end_matches(',').trim();
        let parsed: u32 = value
            .parse()
            .unwrap_or_else(|e| panic!("ExitCode discriminant {value:?} should be a number: {e}"));
        assert!(
            codes.insert(parsed),
            "two ExitCode variants share the discriminant {parsed}"
        );
    }
    assert!(
        codes.len() >= 6,
        "scraping ExitCode found only {} codes, which means the scrape broke \
         rather than that the enum shrank",
        codes.len()
    );
    codes
}

/// The codes a `- \`N\` — …` bullet list publishes, starting at `after`.
fn bullet_codes(doc: &str, after: &str) -> BTreeSet<u32> {
    let tail = doc
        .split_once(after)
        .unwrap_or_else(|| panic!("the document should contain {after:?}"))
        .1;
    let mut codes = BTreeSet::new();
    for line in tail.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("- `") else {
            break; // the list ended
        };
        let Some((num, _)) = rest.split_once('`') else {
            break;
        };
        let Ok(parsed) = num.parse::<u32>() else {
            break;
        };
        codes.insert(parsed);
    }
    codes
}

/// The codes the Developer Guide's "### Exit codes" markdown table publishes
/// in its first column.
fn table_codes(doc: &str) -> BTreeSet<u32> {
    let tail = doc
        .split_once("### Exit codes")
        .expect("the Developer Guide should have an `### Exit codes` heading")
        .1;
    let mut codes = BTreeSet::new();
    let mut seen_table = false;
    for line in tail.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            if seen_table {
                break; // past the end of the table
            }
            continue;
        }
        seen_table = true;
        let first = trimmed
            .trim_start_matches('|')
            .split('|')
            .next()
            .unwrap_or("");
        if let Ok(parsed) = first.trim().parse::<u32>() {
            codes.insert(parsed);
        }
    }
    codes
}

/// Every `N` in an `exit codes 0..N` claim inside `text`.
fn range_claims(text: &str) -> Vec<u32> {
    let mut found = Vec::new();
    let lowered = text.to_lowercase();
    let mut from = 0usize;
    while let Some(at) = lowered[from..].find("exit codes 0..") {
        let start = from + at + "exit codes 0..".len();
        let digits: String = lowered[start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let Ok(parsed) = digits.parse::<u32>() {
            found.push(parsed);
        }
        from = start.max(from + at + 1);
    }
    found
}

#[test]
fn contracts_section_6_lists_every_shipped_exit_code() {
    let documented = bullet_codes(&read("docs/architecture/contracts.md"), "\nExit codes:\n");
    assert_eq!(
        documented,
        enum_codes(),
        "contracts.md §6's exit-code list and `ExitCode` disagree; §6 is the \
         contract, so both move in the same commit"
    );
}

#[test]
fn developer_guide_table_lists_every_shipped_exit_code() {
    let documented = table_codes(&read("docs/Developer_Guide.md"));
    assert_eq!(
        documented,
        enum_codes(),
        "the Developer Guide's exit-code table and `ExitCode` disagree"
    );
}

#[test]
fn every_living_range_claim_names_the_real_maximum() {
    let max = *enum_codes()
        .iter()
        .next_back()
        .expect("ExitCode defines at least one code");
    for rel in RANGE_CLAIMS {
        let claims = range_claims(&read(rel));
        assert!(
            !claims.is_empty(),
            "{rel} is listed as stating `exit codes 0..N` but states none; if the \
             sentence was removed, drop the file from RANGE_CLAIMS"
        );
        for claimed in claims {
            assert_eq!(
                claimed, max,
                "{rel} claims exit codes 0..{claimed} but the enum's highest is {max}"
            );
        }
    }
}

/// The historical documents are excluded on purpose, not by accident. If one
/// of them stops carrying its as-of-date range the exclusion should be
/// revisited rather than silently kept.
#[test]
fn historical_documents_keep_their_as_of_date_ranges() {
    for rel in HISTORICAL {
        let text = read(rel);
        assert!(
            text.to_lowercase().contains("exit codes 0.."),
            "{rel} is excluded from the live check because it states a dated \
             range; it no longer states one, so the exclusion needs revisiting"
        );
        assert!(
            text.contains("historical") || text.contains("HISTORICAL"),
            "{rel} is excluded from the live check because it is a snapshot; it \
             no longer says so, so the exclusion needs revisiting"
        );
    }
}
