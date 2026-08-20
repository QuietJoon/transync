//! Weld `docs/architecture/source-of-truth-table.md` to the code it claims to
//! describe — its cache rows to the cache that ships, and its module
//! enumeration to the modules that exist.
//!
//! The table's job is to name one owner per piece of state. Its "Translation
//! cache" row named the right owner and the wrong *facts*: through 2026-08-09
//! it read "In-memory only per `mvp-scope.md`; disk-backed cache is DEFERRED"
//! while `DiskCache` had shipped in the same v0.4.0 window (DCR-0028 /
//! ADR-0021, `transync translate --cache-dir`). The living-doc closures that
//! rode that work reached `mvp-scope.md` and `open-issues.md` and stopped
//! short of this table and of `persistence-and-files.md` (ticket `d00367`).
//! Nothing was watching, because a table row is prose and prose does not
//! compile.
//!
//! This makes it compile-adjacent. The backend names come from the types
//! themselves via [`std::any::type_name`], so renaming `DiskCache` fails this
//! test until the documents are renamed with it, and deleting a backend fails
//! the *build* here rather than leaving a document describing a type nobody
//! can name. A short list of retired claims is asserted absent as well, each
//! one an absolute statement no qualifier can rescue, so a revert cannot pass
//! as a rewording.
//!
//! Deliberately narrow: this guards the two documents a reader consults to
//! answer "what does transync keep, and where". It is not a general prose
//! linter, and it says nothing about documents that are dated records —
//! ADRs, DCRs and CHANGELOG entries record what was true when written.
//!
//! The second weld (ti `2d3b16`) guards a different failure in the same file:
//! not a row that went stale, but a row that was never written. The table's
//! intro enumerates which module lives in which crate, and that enumeration
//! read as complete while omitting `structure`, `walk`, `outcome` and `error`
//! — so a reader asking "who owns a block's structural fingerprint?" found
//! `parser` and `validate` and no hint that the fingerprint they compare is a
//! third, separately-owned thing. An omission is invisible in a way a wrong
//! sentence is not: nothing about the paragraph advertises what is missing
//! from it. So the module list is read out of both crates' `lib.rs` rather
//! than restated here, and a module the table never names fails this test.
//!
//! TRACE: ti d00367
//! TRACE: ti 2d3b16
//! TRACE: DCR-0028
//! TRACE: ADR-0021

use std::path::{Path, PathBuf};

use transync::{DiskCache, InMemoryCache};

/// Repo root, derived robustly from this crate's manifest dir
/// (`crates/transync` -> `../..`). Same idiom as `docs_index_drift.rs`.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn read_doc(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{rel} should be readable ({}): {e}", path.display()))
}

/// The living documents that answer "what does transync keep, and where".
const GUARDED: &[&str] = &[
    "docs/architecture/source-of-truth-table.md",
    "docs/architecture/persistence-and-files.md",
];

/// Bare type name — `transync_core::cache::DiskCache` -> `DiskCache`.
fn short_type_name<T: ?Sized>() -> &'static str {
    std::any::type_name::<T>()
        .rsplit("::")
        .next()
        .expect("a type name always has a last segment")
}

/// Every `Cache` backend a consumer can construct. Naming the types here is
/// the weld: the list cannot silently fall behind the crate, because a removed
/// backend stops this file compiling.
fn shipped_cache_backends() -> Vec<&'static str> {
    vec![
        short_type_name::<InMemoryCache>(),
        short_type_name::<DiskCache>(),
    ]
}

#[test]
fn every_guarded_document_names_every_shipped_cache_backend() {
    let backends = shipped_cache_backends();
    let mut missing = Vec::new();
    for rel in GUARDED {
        let doc = read_doc(rel);
        for backend in &backends {
            if !doc.contains(backend) {
                missing.push(format!("{rel}: never names `{backend}`"));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "a document that tells a reader what transync keeps has to name every \
         cache backend a caller can construct — the default one and the \
         durable one — and say which is which:\n  {}",
        missing.join("\n  "),
    );
}

/// Claims these documents carried before `DiskCache` shipped, each false the
/// moment it did. Paired with what a reader should be told instead, so a
/// failure explains itself rather than just naming a forbidden string.
///
/// Every entry is an **absolute** claim — one no qualifier can rescue, so
/// banning the phrase cannot fire on a sentence somebody meant. "In-memory
/// only" is deliberately absent for the opposite reason: "in-memory only when
/// no `--cache-dir` is given" is a true sentence, and a guard that forbids it
/// is a guard the next author deletes. The positive check above is what
/// catches a row reverting to it, because a reverted row stops naming
/// `DiskCache`.
const RETIRED_CLAIMS: &[(&str, &str)] = &[
    (
        "disk-backed cache is DEFERRED",
        "the disk-backed cache is the second shipped backend (DCR-0028 / ADR-0021), not a deferral",
    ),
    (
        "filesystem-blind",
        "the library opens the cache directory a caller hands `DiskCache::open`",
    ),
    (
        "process-lifetime in-memory only",
        "a `DiskCache` entry outlives the process that wrote it",
    ),
];

/// The crate roots whose module lists the table's intro enumerates, each with
/// the floor its own `lib.rs` must clear.
///
/// The floor is **per root** because `transync-html` is a single file: ti
/// 490d97 wave 0 moved `htmlseg.rs` in verbatim, and the file-as-module split
/// the spec permits is a later, separate decision. One shared floor of 5
/// would assert a shape nobody has chosen for that crate; the two roots above
/// it are what keep the anti-vacuity check meaningful.
const CRATE_ROOTS: &[(&str, &str, usize)] = &[
    ("transync-syntax", "crates/transync-syntax/src/lib.rs", 5),
    ("transync-core", "crates/transync-core/src/lib.rs", 5),
    ("transync-html", "crates/transync-html/src/lib.rs", 0),
];

/// Every module a crate root declares, minus the `cfg`-gated ones.
///
/// `test_stub` (behind a feature) and `test_fixtures` (behind `cfg(test)`) are
/// scaffolding, not owned state, and a table that named them would be padding.
/// The gate is the `#[cfg(` attribute rather than a hand-written deny-list, so
/// a third test-only module is excluded the day it is written.
///
/// Deliberately a line scan and not a real parser: the input is two files this
/// repo controls, both a flat list of `mod` declarations, and a syn dependency
/// on a doc test buys nothing a `split` does not already do correctly here.
fn declared_modules(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cfg_gated = false;
    for line in src.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[") {
            // Only `cfg` hides a module; `#[doc(hidden)]` (`unit`)
            // means "not curated API", which is still an owner worth naming.
            cfg_gated |= line.starts_with("#[cfg(");
            continue;
        }
        // An attribute binds to the next *item*, so any other meaningful line
        // ends its reach — otherwise a `#[cfg]`-gated `use` would silently
        // swallow the module declared after it.
        let rest = line
            .strip_prefix("pub(crate) ")
            .or_else(|| line.strip_prefix("pub "))
            .unwrap_or(line);
        if let Some(name) = rest
            .strip_prefix("mod ")
            .and_then(|r| r.strip_suffix(';'))
            .map(str::trim)
            .filter(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
            && !cfg_gated
        {
            out.push(name.to_string());
        }
        cfg_gated = false;
    }
    out
}

/// A module is "named" if the document mentions it as a backticked token,
/// bare (`` `walk` ``) or crate-qualified (`` `transync-syntax::walk` ``).
/// Matching the raw name would pass on `rendered`, `unit tests` and `id`
/// inside a dozen unrelated words.
fn document_names_module(doc: &str, module: &str) -> bool {
    doc.contains(&format!("`{module}`")) || doc.contains(&format!("::{module}`"))
}

#[test]
fn the_module_enumeration_names_every_module_both_crates_declare() {
    let doc = read_doc("docs/architecture/source-of-truth-table.md");
    let mut unnamed = Vec::new();
    for (krate, rel, floor) in CRATE_ROOTS {
        let modules = declared_modules(&read_doc(rel));
        assert!(
            modules.len() >= *floor,
            "{rel} parsed to {} modules, below its floor of {floor} — the scan \
             broke, it did not get simpler",
            modules.len(),
        );
        for module in modules {
            if !document_names_module(&doc, &module) {
                unnamed.push(format!("{krate}::{module} (declared in {rel})"));
            }
        }
    }
    assert!(
        unnamed.is_empty(),
        "source-of-truth-table.md enumerates which module lives in which crate, \
         and that enumeration reads as complete — so a module it never names is \
         a module whose owner nobody wrote down. Add a row, or add the name to \
         the intro if the module owns nothing a reader would look up:\n  {}",
        unnamed.join("\n  "),
    );
}

#[test]
fn no_guarded_document_still_says_the_cache_cannot_reach_disk() {
    let mut violations = Vec::new();
    for rel in GUARDED {
        let doc = read_doc(rel);
        for (claim, why) in RETIRED_CLAIMS {
            if doc.contains(claim) {
                violations.push(format!("{rel}: still says \"{claim}\" — {why}"));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "living documents describing the cache have drifted back behind the \
         code:\n  {}",
        violations.join("\n  "),
    );
}
