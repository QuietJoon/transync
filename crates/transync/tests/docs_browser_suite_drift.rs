//! Guard against the browser suite's test count creeping back into living docs.
//!
//! `scripts/test-browser.sh` runs the Playwright specs under `web/tests/`. That
//! number changes every time a test is added, and it had already gone stale in
//! three documents at once before ticket `e9481b` settled the convention:
//!
//! > a living document names the **spec files** — `web/tests/scn13.spec.js` and
//! > `web/tests/wasm.spec.js` — and never states how many tests they hold.
//!
//! Naming the files tells a reader where to look instead of how many there are,
//! and it cannot rot. This test keeps the convention from being half-applied:
//! it fails when a guarded document states a count near a mention of the suite,
//! and when a document that describes the suite operationally stops naming its
//! spec files.
//!
//! A count does not have to be written in digits. `web/SMOKE.md` carried "the
//! six tests (`web/tests/scn13.spec.js`)" straight through the sweep that
//! introduced this file — the digit scan had nothing to see in it — until ticket
//! `8e350d4` noticed the suite had grown to three spec files. Spelled-out counts
//! are recognised too now, and `web/SMOKE.md` joined the documents that must
//! keep naming the spec files, since that is what replaced its count.
//!
//! The third spec joined the pin later than the other two. `web/tests/
//! engine.spec.js` — `sync.js`'s mount contract driven directly over a bare
//! two-pane rig — has been in the suite and run by the same `pnpm exec
//! playwright test` all along (`playwright.config.js` sets
//! `testDir: "./tests"`), but `docs/Developer_Guide.md` named only the other
//! two, and `SPEC_FILES` cannot pin a file that a `MUST_NAME_SPECS` document
//! does not name. So the file was unpinned: renaming or deleting it would have
//! left every guarded document green. Ticket `729ec8` closed that by naming it
//! in the guide and in the module map's `scripts/` tree, which is what let it
//! join `SPEC_FILES` here.
//!
//! Snapshot documents are deliberately **not** guarded. DCR-0019 (8/8),
//! DCR-0020 (12/12), the resolved `open-issues.md` entry (6 tests) and the wave
//! log in `docs/project/status.md` each record what was true when written, and
//! are correct as written.
//!
//! TRACE: ti e9481b — the count convention, applied to every living document.

use std::path::{Path, PathBuf};

/// Living documents that describe the browser suite as it is **now**. Snapshot
/// records (ADRs, DCRs, dated status entries, superseded plans) are excluded on
/// purpose — see the module docs.
const GUARDED: &[&str] = &[
    "docs/implementation/module-map.md",
    "docs/Developer_Guide.md",
    "docs/project/release-checklist.md",
    "docs/architecture/README.md",
    "docs/architecture/mvp-scope.md",
    "docs/architecture/scenario-matrix.md",
    "README.md",
    "web/SMOKE.md",
];

/// Documents that must keep naming the spec files, since dropping the count
/// only helps if the pointer that replaced it survives.
const MUST_NAME_SPECS: &[&str] = &[
    "docs/implementation/module-map.md",
    "docs/Developer_Guide.md",
    "web/SMOKE.md",
];

const SPEC_FILES: &[&str] = &[
    "scn13.spec.js",
    "engine.spec.js",
    "wasm.spec.js",
    "scn16.spec.js",
];

/// A line mentioning any of these is a line about the browser suite.
const SUITE_MENTIONS: &[&str] = &[
    "test-browser.sh",
    "web/tests",
    "scn13.spec.js",
    "engine.spec.js",
    "wasm.spec.js",
    "scn16.spec.js",
    "Playwright",
];

/// How far a count may sit from the mention and still be a count *of the
/// suite*. The module map's layout tree wraps one entry over continuation
/// lines, which is exactly how the offending `(16: 11 SCN-13 + 5 wasm demo)`
/// escaped a same-line check.
const WINDOW: usize = 3;

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

/// Bytes that make a neighbouring digit run part of something bigger — a date
/// (`2026-07-13`), a grouped size (`1,950,000`), a version (`0.3.0`), a path
/// segment. A number flanked by one of these is not a standalone count.
fn glues_to_number(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'.' | b',' | b'/' | b'-' | b'_')
}

/// End of the digit run starting at `i`, or `None` when `i` is not a digit.
fn digits_end(bytes: &[u8], i: usize) -> Option<usize> {
    if i >= bytes.len() || !bytes[i].is_ascii_digit() {
        return None;
    }
    let mut end = i;
    while end < bytes.len() && bytes[end].is_ascii_digit() {
        end += 1;
    }
    Some(end)
}

fn skip_spaces(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }
    i
}

/// Counts spelled as words rather than digits — the shape that carried "the
/// six tests" past every earlier sweep, because the digit scan has nothing to
/// see in it.
///
/// `one` is deliberately absent: "run one test with `-g`" is an instruction,
/// not a published count, and it is the only value common enough in prose for
/// the false positive to outweigh the catch.
const NUMBER_WORDS: &[&str] = &[
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
];

/// True when `b` cannot continue a word, so `six` matches in `six tests` and
/// not in `sixteen`, `web/six.js`, or `nineteen-eighty`.
fn word_boundary(b: Option<u8>) -> bool {
    match b {
        None => true,
        Some(c) => !(c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'/' | b'.')),
    }
}

/// The offending substring when `line` spells the count out: `six tests`.
///
/// Byte offsets from the lowercased copy index the original safely — ASCII
/// lowercasing is length-preserving, and every match starts and ends on an
/// ASCII byte.
fn spelled_count(line: &str) -> Option<String> {
    let lower = line.to_ascii_lowercase();
    let b = lower.as_bytes();
    for word in NUMBER_WORDS {
        let mut from = 0;
        while let Some(rel) = lower[from..].find(word) {
            let start = from + rel;
            let end = start + word.len();
            from = end;
            let before = start.checked_sub(1).map(|p| b[p]);
            if !word_boundary(before) || !word_boundary(b.get(end).copied()) {
                continue;
            }
            // At least one space: `sixtests` is not a count of anything.
            let after = skip_spaces(b, end);
            if after == end {
                continue;
            }
            let rest = &lower[after..];
            let tail = rest
                .strip_prefix("tests")
                .or_else(|| rest.strip_prefix("test"));
            if let Some(tail) = tail
                && word_ends(tail)
            {
                let stop = rest.len() - tail.len() + after;
                return Some(line[start..stop].to_string());
            }
        }
    }
    None
}

/// True when what follows a `test`/`tests` word ends it rather than continuing
/// it, so `13 tests` counts but `13 test-browser.sh` does not.
fn word_ends(tail: &str) -> bool {
    match tail.chars().next() {
        None => true,
        Some(c) => c.is_whitespace() || matches!(c, '.' | ',' | ';' | ':' | ')' | '!' | '*' | '`'),
    }
}

/// The offending substring when `line` states a count, in any of the four
/// shapes this convention has actually been broken in:
///
/// - `13 tests` / `16 Playwright tests` — a number qualifying the word;
/// - `(16: …` / `(16)` — a parenthetical opening straight onto a number;
/// - `12/12` — the pass-ratio idiom, recognised by its two equal halves;
/// - `six tests` — the count spelled out (ti `8e350d4`).
fn published_count(line: &str) -> Option<String> {
    if let Some(found) = spelled_count(line) {
        return Some(found);
    }
    let b = line.as_bytes();
    for i in 0..b.len() {
        // `(16:` — a parenthetical whose first token is a bare number.
        if b[i] == b'(' {
            let start = skip_spaces(b, i + 1);
            if let Some(end) = digits_end(b, start) {
                let close = skip_spaces(b, end);
                if close < b.len() && (b[close] == b':' || b[close] == b')') {
                    return Some(line[i..=close].to_string());
                }
            }
        }

        if !b[i].is_ascii_digit() || (i > 0 && glues_to_number(b[i - 1])) {
            continue;
        }
        let first_end = digits_end(b, i).expect("checked digit above");
        let first = &line[i..first_end];

        // Optional `/N` second half, for the pass-ratio idiom.
        let mut end = first_end;
        let mut second = None;
        if end < b.len()
            && b[end] == b'/'
            && let Some(e) = digits_end(b, end + 1)
        {
            second = Some(&line[end + 1..e]);
            end = e;
        }

        // `N tests` / `N/N tests`.
        let after = skip_spaces(b, end);
        let rest = &line[after..];
        let lower = rest.to_ascii_lowercase();
        let tail = lower
            .strip_prefix("tests")
            .or_else(|| lower.strip_prefix("test"));
        if let Some(tail) = tail
            && word_ends(tail)
        {
            let stop = rest.len() - tail.len() + after;
            return Some(line[i..stop].to_string());
        }

        // `12/12` standing alone.
        if second == Some(first) && (end >= b.len() || !glues_to_number(b[end])) {
            return Some(line[i..end].to_string());
        }
    }
    None
}

/// Every line of `text` that is within `WINDOW` lines of a suite mention,
/// as `(1-based line number, line)`.
fn lines_about_the_suite(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![false; lines.len()];
    for (i, line) in lines.iter().enumerate() {
        if !SUITE_MENTIONS.iter().any(|m| line.contains(m)) {
            continue;
        }
        let lo = i.saturating_sub(WINDOW);
        let hi = (i + WINDOW + 1).min(lines.len());
        keep[lo..hi].iter_mut().for_each(|k| *k = true);
    }
    lines
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(i, line)| (i + 1, line))
        .collect()
}

#[test]
fn no_living_doc_publishes_the_browser_suite_test_count() {
    let mut offenders = Vec::new();
    for rel in GUARDED {
        let text = read(rel);
        for (number, line) in lines_about_the_suite(&text) {
            if let Some(found) = published_count(line) {
                offenders.push(format!("{rel}:{number}: `{found}`  in  {}", line.trim()));
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "living document(s) state a count for the browser suite. The count rots \
         the next time a Playwright test is added, so name the spec files \
         (`web/tests/scn13.spec.js`, `web/tests/engine.spec.js`, \
         `web/tests/wasm.spec.js`, `web/tests/scn16.spec.js`) instead of counting them (ti e9481b). If the passage is a dated snapshot rather \
         than a description of the suite today, it does not belong in a guarded \
         living document:\n  {}",
        offenders.join("\n  "),
    );
}

#[test]
fn living_docs_name_the_browser_spec_files() {
    let mut missing = Vec::new();
    for rel in MUST_NAME_SPECS {
        let text = read(rel);
        for spec in SPEC_FILES {
            if !text.contains(spec) {
                missing.push(format!("{rel} does not name `{spec}`"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "the browser suite is identified by its spec files, not by a count \
         (ti e9481b) — dropping the count only helps while the pointer that \
         replaced it survives:\n  {}",
        missing.join("\n  "),
    );
}

#[test]
fn count_detector_recognises_the_shapes_the_convention_has_been_broken_in() {
    // The three historical forms.
    assert_eq!(
        published_count("plus the wasm demo (13 tests)").as_deref(),
        Some("13 tests")
    );
    assert_eq!(
        published_count("#   (16: 11 SCN-13 + 5 wasm demo)").as_deref(),
        Some("(16:")
    );
    assert_eq!(
        published_count("Playwright **12/12**.").as_deref(),
        Some("12/12")
    );
    assert_eq!(published_count("the suite (16)").as_deref(), Some("(16)"));

    // The spelled-out shape, which `web/SMOKE.md` carried (ti 8e350d4).
    assert_eq!(
        published_count("six tests (`web/tests/scn13.spec.js`) cover bidirectional sync,")
            .as_deref(),
        Some("six tests")
    );
    assert_eq!(
        published_count("Twelve tests now, in two spec files.").as_deref(),
        Some("Twelve tests")
    );

    // Number words that are not counts of tests.
    assert_eq!(published_count("the sixteen-column fixture table"), None);
    assert_eq!(published_count("nineteen-eighty-four test fixtures"), None);
    assert_eq!(published_count("six spec files under web/tests"), None);
    assert_eq!(published_count("run one test with -g"), None);

    // Numbers that are not counts.
    assert_eq!(
        published_count("Shipped 2026-07-13 (EXT-2026-07 P2-10)"),
        None
    );
    assert_eq!(
        published_count("raw <= 1,950,000 B, gzip <= 810,000 B"),
        None
    );
    assert_eq!(published_count("binaryen >= 121 is required"), None);
    assert_eq!(
        published_count("run `scripts/test-browser.sh` (Chromium)"),
        None
    );
    assert_eq!(published_count("v0.3.0 ships the wasm demo"), None);
    assert_eq!(published_count("see web/tests/scn13.spec.js"), None);
}
