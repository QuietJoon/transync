//! Standing gate on the public Rust surface (OI-0027).
//!
//! `crates/transync/src/lib.rs` is an explicitly curated re-export list, and
//! `docs/architecture/contracts.md` §0 tabulates it. Three artifacts name the
//! same row set — the §0 table, the `DOCUMENTED` constant, and the
//! `first_class` module — and this test welds all three to each other:
//!
//! 1. **Code side** — the `first_class` module below re-exports every §0 row.
//!    A row naming something the facade does not export is a compile error,
//!    so the table can never document surface that isn't there.
//! 2. **Table ↔ `DOCUMENTED`** — `surface_table_matches_the_documented_list`
//!    scrapes §0 out of contracts.md and diffs it against `DOCUMENTED` in
//!    both directions, naming whatever drifted.
//! 3. **`DOCUMENTED` ↔ `first_class`** —
//!    `first_class_module_matches_the_documented_list` scrapes the
//!    `first_class` module out of *this file's own source* and diffs it
//!    against `DOCUMENTED` the same way, so the compile assertion cannot
//!    quietly cover a different set than the two lists do.
//! 4. **`lib.rs` ↔ `DOCUMENTED`** —
//!    `lib_rs_exports_nothing_the_documented_list_omits` scrapes the facade's
//!    own `crates/transync/src/lib.rs` and asserts every name it publishes
//!    carries a §0 row, so a new `pub use` added *there alone* can no longer
//!    widen the surface silently. The feature-gated `transync::test_stub` is
//!    the one allowance, listed in `FEATURE_GATED`.
//! 5. **Negative side** — `hidden_modules_are_not_documented_as_surface`
//!    asserts the tier-(c) engine internals never appear in §0, so the
//!    semver firewall cannot be widened by documenting through it.
//! 6. **Version side** — `generator_version_matches_the_facade_crate` pins
//!    DCR-0017 M6: the alignment map stamps `transync-syntax`'s version, and
//!    the workspace-inherited version keeps it equal to the facade's.
//!    `validation_report_version_constant_matches_the_stamped_field` pins the
//!    second durable artifact the same way (ti 8a4a32): the exported
//!    `VALIDATION_REPORT_SCHEMA_VERSION` must equal the `schema_version` the
//!    engine stamps, or the constant a consumer branches on describes a report
//!    shape nothing writes.
//!
//! **What this does and does not guarantee.** Edges 2–4 close the square
//! table ↔ `DOCUMENTED` ↔ `first_class` ↔ `lib.rs`: any one of the four
//! drifting from another is a red test, and edge 1 makes a row the facade does
//! not export a red compile. Edge 4 reads source text rather than rustdoc, so
//! it leans on `lib.rs`'s stable shape — every export is a top-level `pub use`
//! (or a `pub mod NAME;`), and any other top-level `pub` form fails the scrape
//! outright instead of being skipped. It also asserts the reverse inclusion, so
//! a scrape that quietly stopped finding anything cannot pass vacuously.
//!
//! What remains unchecked is surface that reaches consumers *without being
//! named in `lib.rs` at all* — a core type newly exposed through an exported
//! type's signature, say. Only a rustdoc-JSON / `cargo-public-api` diff sees
//! that, and it stays deliberately out of scope for this gate.
//!
//! TRACE: OI-0027
//! TRACE: DCR-0017
//! TRACE: contracts.md §0

/// Compile-as-assertion: one `pub use` per contracts.md §0 row. Nothing
/// reads these — the point is that they resolve.
#[allow(unused_imports)]
mod first_class {
    // The four curated modules.
    pub use transync::cache;
    pub use transync::llm;
    pub use transync::llm::prompt;
    pub use transync::profile;

    // Root re-exports (tier a).
    pub use transync::{
        ALIGNMENT_SCHEMA_VERSION, AlignmentBlock, AlignmentMap, AttemptOutcome, AutoGlossaryReport,
        AutoGlossaryStatus, BatchFault, BatchId, BlockConstraints, BlockContext, BlockId,
        BlockKind, ByteRange, Cache, CacheError, CacheKey, CancellationToken,
        DEFAULT_MAX_AUTO_GLOSSARY_TERMS, DiskCache, DiskCacheOptions, DocumentMeta,
        DocumentMetaKey, FallbackStatus, FullReparseFailure, GeneratorMeta, GlossaryEntry,
        GlossaryExtraction, GlossaryExtractionKey, GlossaryExtractionRequest, GlossaryScope,
        InMemoryCache, InputMode, ListTopologyEntry, MAX_EXTRACTION_SOURCE_BYTES, MergedGlossary,
        OutputBudgetWarning, OutputKind, ParseError, ProfileError, ProfileMetadata,
        ProviderFingerprint, RetryContext, SyncRole, TableAlign, TokenizerHint, TranslateOptions,
        TranslationBatch, TranslationBatchResult, TranslationOutput, TranslationUnit, Translator,
        TranslatorError, TransyncError, UnitResult, UnitValidationRecord,
        VALIDATION_REPORT_SCHEMA_VERSION, VALIDATION_SCHEMA_VERSION, ValidationLayer,
        ValidationReport, ValidationSummary, merge_auto_glossary, translate, translate_with_cache,
    };

    // `transync::llm` items (tier a).
    pub use transync::llm::{
        HeadingSnippet, HtmlSegmentConstraints, ListDelimiter, NeighborSnippet,
    };

    // `transync::llm::prompt` items (tier b — provider SDK).
    pub use transync::llm::prompt::{
        EXTRACTION_SCHEMA_NAME, EXTRACTION_SYSTEM_PROMPT, SCHEMA_NAME,
        build_extraction_user_prompt, build_user_prompt, extraction_schema_object,
        parse_batch_output, parse_extraction_output, schema_object_for,
    };

    // `transync::profile` items (tier a, plus tier-b `render_prompt_body`).
    pub use transync::profile::{
        ProfileBatching, ProfileConstraints, ProfileRender, default_profile, load_profile,
        render_prompt_body,
    };
}

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

/// Every §0 row path, verbatim. Mirrors both the `first_class` module
/// above and contracts.md §0 — the test below fails on any drift.
const DOCUMENTED: &[&str] = &[
    "transync::translate",
    "transync::translate_with_cache",
    "transync::TranslateOptions",
    "transync::TranslationOutput",
    "transync::FullReparseFailure",
    "transync::CancellationToken",
    "transync::TransyncError",
    "transync::ParseError",
    "transync::Cache",
    "transync::CacheError",
    "transync::CacheKey",
    "transync::DocumentMetaKey",
    "transync::DocumentMeta",
    "transync::GlossaryExtractionKey",
    "transync::GlossaryExtraction",
    "transync::InMemoryCache",
    "transync::DiskCache",
    "transync::DiskCacheOptions",
    "transync::cache",
    "transync::Translator",
    "transync::TranslatorError",
    "transync::TranslationBatch",
    "transync::TranslationBatchResult",
    "transync::TranslationUnit",
    "transync::UnitResult",
    "transync::OutputKind",
    "transync::RetryContext",
    "transync::BatchId",
    "transync::BlockConstraints",
    "transync::BlockContext",
    "transync::InputMode",
    "transync::ListTopologyEntry",
    "transync::TableAlign",
    "transync::TokenizerHint",
    "transync::ProviderFingerprint",
    "transync::GlossaryEntry",
    "transync::GlossaryScope",
    "transync::GlossaryExtractionRequest",
    "transync::DEFAULT_MAX_AUTO_GLOSSARY_TERMS",
    "transync::MAX_EXTRACTION_SOURCE_BYTES",
    "transync::llm",
    "transync::llm::HeadingSnippet",
    "transync::llm::NeighborSnippet",
    "transync::llm::HtmlSegmentConstraints",
    "transync::llm::ListDelimiter",
    "transync::ProfileMetadata",
    "transync::ProfileError",
    "transync::MergedGlossary",
    "transync::merge_auto_glossary",
    "transync::profile",
    "transync::profile::ProfileConstraints",
    "transync::profile::ProfileBatching",
    "transync::profile::ProfileRender",
    "transync::profile::load_profile",
    "transync::profile::default_profile",
    "transync::AlignmentMap",
    "transync::AlignmentBlock",
    "transync::ALIGNMENT_SCHEMA_VERSION",
    "transync::ByteRange",
    "transync::FallbackStatus",
    "transync::GeneratorMeta",
    "transync::SyncRole",
    "transync::ValidationSummary",
    "transync::BlockId",
    "transync::BlockKind",
    "transync::ValidationReport",
    "transync::ValidationLayer",
    "transync::AttemptOutcome",
    "transync::UnitValidationRecord",
    "transync::BatchFault",
    "transync::OutputBudgetWarning",
    "transync::AutoGlossaryReport",
    "transync::AutoGlossaryStatus",
    "transync::VALIDATION_SCHEMA_VERSION",
    "transync::VALIDATION_REPORT_SCHEMA_VERSION",
    "transync::llm::prompt",
    "transync::llm::prompt::SCHEMA_NAME",
    "transync::llm::prompt::build_user_prompt",
    "transync::llm::prompt::schema_object_for",
    "transync::llm::prompt::parse_batch_output",
    "transync::llm::prompt::EXTRACTION_SCHEMA_NAME",
    "transync::llm::prompt::EXTRACTION_SYSTEM_PROMPT",
    "transync::llm::prompt::build_extraction_user_prompt",
    "transync::llm::prompt::extraction_schema_object",
    "transync::llm::prompt::parse_extraction_output",
    "transync::profile::render_prompt_body",
];

/// The root paths `lib.rs` publishes only behind a `#[cfg(feature = …)]`, and
/// which §0 therefore omits on purpose. contracts.md §0 names
/// `transync::test_stub` (the `test-stub` feature, forwarded to
/// `transync-core`) and excludes it explicitly: a feature-gated item is not
/// part of the default surface. These — and only these — may appear in `lib.rs`
/// without a §0 row.
const FEATURE_GATED: &[&str] = &["transync::test_stub"];

/// Repo root, derived from this crate's manifest dir (`crates/transync`).
/// Same idiom as `docs_index_drift.rs`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The `## 0.` section of contracts.md, from its heading up to the next
/// `## ` heading (or end of file).
fn section_zero() -> String {
    let text = fs::read_to_string(repo_root().join("docs/architecture/contracts.md"))
        .expect("contracts.md must exist");
    let start = text.find("## 0.").expect("contracts.md must carry §0");
    let rest = &text[start..];
    let end = rest[3..].find("\n## ").map(|i| i + 3).unwrap_or(rest.len());
    rest[..end].to_string()
}

/// The `path` cell of every §0 table row, in document order and *with*
/// duplicates — the callers below compare this length against the deduplicated
/// set so a copy-pasted row cannot silently inflate the row count.
fn table_rows() -> Vec<String> {
    section_zero()
        .lines()
        .filter(|l| l.starts_with("| `transync::"))
        .map(|l| {
            let cell = l.trim_start_matches("| `");
            cell[..cell.find('`').expect("row path must close its backtick")].to_string()
        })
        .collect()
}

/// The `path` cell of every §0 table row, deduplicated.
fn table_paths() -> BTreeSet<String> {
    table_rows().into_iter().collect()
}

/// The leaf path of every `pub use` statement in `src`, with duplicates.
///
/// Two statement forms occur:
///
/// - plain — `pub use transync::cache;` yields one leaf, `transync::cache`;
/// - group — `pub use transync::llm::{A, B};` yields one leaf per brace item,
///   `transync::llm::A` and `transync::llm::B`.
///
/// Statements are split on `;` *after* the source is joined into a single
/// string, so a group spread over many lines parses exactly like a one-liner
/// and a preceding `#[cfg(…)]` attribute rides along harmlessly. `//` comment
/// lines are dropped first, so a commented-out name cannot masquerade as an
/// export.
fn use_leaves(src: &str) -> Vec<String> {
    let block: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join(" ");

    let mut leaves = Vec::new();
    for stmt in block.split(';') {
        let Some(at) = stmt.find("pub use ") else {
            continue;
        };
        let path = stmt[at + "pub use ".len()..].trim();
        match path.split_once('{') {
            Some((prefix, items)) => {
                let prefix = prefix.trim();
                for item in items.trim_end().trim_end_matches('}').split(',') {
                    let item = item.trim();
                    if !item.is_empty() {
                        leaves.push(format!("{prefix}{item}"));
                    }
                }
            }
            None => leaves.push(path.to_string()),
        }
    }
    leaves
}

/// The leaf paths re-exported by this file's own `first_class` module.
///
/// The module is a *compile* assertion, so nothing can enumerate it at
/// runtime — we scrape this test's own source instead (same `repo_root()` +
/// `read_to_string` idiom as `table_rows`). Returned with duplicates, like
/// `table_rows`.
fn first_class_uses() -> Vec<String> {
    let text = fs::read_to_string(repo_root().join("crates/transync/tests/public_surface.rs"))
        .expect("this test's own source must be readable");
    // The first occurrence is the real module — the scraper sits below it.
    let start = text
        .find("mod first_class {")
        .expect("the first_class module must exist");
    let rest = &text[start..];
    // The module closes with a `}` at column 0; the `};` that ends each `use`
    // group is indented, so it cannot match.
    let end = rest
        .find("\n}\n")
        .expect("the first_class module must close at column 0");
    use_leaves(&rest[..end])
}

/// Every root path `crates/transync/src/lib.rs` actually publishes, with
/// duplicates.
///
/// A re-export's public name is its `as` alias, or else its last path segment:
/// `pub use transync_core::llm;` publishes `transync::llm`, the same string §0
/// tabulates. `pub mod NAME;` publishes `transync::NAME` the same way.
///
/// Any *other* top-level `pub ` item — `pub fn`, `pub struct`, an inline
/// `pub mod NAME { … }` — is a shape this scraper cannot read, so it panics
/// rather than skip the line: a gate that silently ignores what it does not
/// understand is not a gate. (`pub(crate)` does not start with `pub `, and a
/// `use` group's continuation lines are indented, so neither is mistaken for a
/// top-level item.)
fn facade_exports() -> Vec<String> {
    let text = fs::read_to_string(repo_root().join("crates/transync/src/lib.rs"))
        .expect("the facade's lib.rs must be readable");

    let mut roots: Vec<String> = Vec::new();
    for line in text.lines() {
        let Some(item) = line.strip_prefix("pub ") else {
            continue;
        };
        if item.starts_with("use ") {
            continue; // Collected below — a `use` group can span lines.
        }
        let name = item
            .strip_prefix("mod ")
            .and_then(|rest| rest.strip_suffix(';'))
            .unwrap_or_else(|| {
                panic!(
                    "crates/transync/src/lib.rs carries a top-level `pub` item this \
                     gate cannot read: `{line}` — teach `facade_exports` the form \
                     before shipping it, or the surface widens unchecked"
                )
            });
        roots.push(format!("transync::{}", name.trim()));
    }

    roots.extend(use_leaves(&text).iter().map(|leaf| {
        let name = match leaf.split_once(" as ") {
            Some((_, alias)) => alias.trim(),
            None => leaf.rsplit("::").next().unwrap_or(leaf).trim(),
        };
        format!("transync::{name}")
    }));
    roots
}

/// The distinct entries that appear more than once in `items`.
fn duplicates(items: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut dups = BTreeSet::new();
    for item in items {
        if !seen.insert(item) {
            dups.insert(item.clone());
        }
    }
    dups.into_iter().collect()
}

/// `DOCUMENTED` as a set, asserting on the way that it holds no duplicates —
/// without this, two identical entries would shrink the set and let a real
/// row go missing without the diffs below noticing.
fn documented_set() -> BTreeSet<String> {
    let listed: Vec<String> = DOCUMENTED.iter().map(|s| s.to_string()).collect();
    let set: BTreeSet<String> = listed.iter().cloned().collect();
    assert_eq!(
        DOCUMENTED.len(),
        set.len(),
        "DOCUMENTED lists a path twice: {:?}",
        duplicates(&listed)
    );
    set
}

#[test]
fn surface_table_matches_the_documented_list() {
    let rows = table_rows();
    let table: BTreeSet<String> = rows.iter().cloned().collect();
    assert_eq!(
        rows.len(),
        table.len(),
        "contracts.md §0 lists a path twice: {:?}",
        duplicates(&rows)
    );

    let documented = documented_set();
    let undocumented: Vec<_> = documented.difference(&table).collect();
    let orphaned: Vec<_> = table.difference(&documented).collect();
    assert!(
        undocumented.is_empty() && orphaned.is_empty(),
        "surface drift — missing from contracts.md §0: {undocumented:?}; \
         documented but not exported: {orphaned:?}"
    );
}

#[test]
fn first_class_module_matches_the_documented_list() {
    let uses = first_class_uses();
    let first_class: BTreeSet<String> = uses.iter().cloned().collect();
    assert_eq!(
        uses.len(),
        first_class.len(),
        "the first_class module re-exports a path twice: {:?}",
        duplicates(&uses)
    );

    let documented = documented_set();
    let unasserted: Vec<_> = documented.difference(&first_class).collect();
    let unlisted: Vec<_> = first_class.difference(&documented).collect();
    assert!(
        unasserted.is_empty() && unlisted.is_empty(),
        "surface drift — documented but not re-exported by the first_class \
         compile assertion: {unasserted:?}; re-exported by first_class but \
         missing from DOCUMENTED (and therefore from contracts.md §0): \
         {unlisted:?}"
    );
}

#[test]
fn lib_rs_exports_nothing_the_documented_list_omits() {
    let exports = facade_exports();
    let facade: BTreeSet<String> = exports.iter().cloned().collect();
    assert_eq!(
        exports.len(),
        facade.len(),
        "crates/transync/src/lib.rs publishes a name twice: {:?}",
        duplicates(&exports)
    );

    let documented = documented_set();
    let gated: BTreeSet<String> = FEATURE_GATED.iter().map(|s| s.to_string()).collect();
    let double_booked: Vec<_> = gated.intersection(&documented).collect();
    assert!(
        double_booked.is_empty(),
        "FEATURE_GATED excuses a path contracts.md §0 also documents: \
         {double_booked:?} — drop it from one side or the other"
    );

    // The widening direction: a `pub use` added to lib.rs alone is a red test.
    let allowed: BTreeSet<String> = documented.union(&gated).cloned().collect();
    let widened: Vec<_> = facade.difference(&allowed).collect();
    assert!(
        widened.is_empty(),
        "surface widening — published by crates/transync/src/lib.rs but absent \
         from contracts.md §0 (and therefore from DOCUMENTED): {widened:?}"
    );

    // Anti-vacuity: a scraper that saw nothing would satisfy the subset check
    // above trivially, so pin the other direction too. Every §0 row that names
    // a facade *root* item must be a name the scrape actually found.
    let documented_roots: BTreeSet<String> = documented
        .iter()
        .filter(|p| p.matches("::").count() == 1)
        .cloned()
        .collect();
    let unseen: Vec<_> = documented_roots.difference(&facade).collect();
    assert!(
        unseen.is_empty(),
        "the lib.rs scrape missed documented root exports {unseen:?} — either \
         the facade dropped them, or lib.rs changed shape and the scrape stopped \
         reading it"
    );
}

#[test]
fn hidden_modules_are_not_documented_as_surface() {
    let forbidden = [
        "pipeline",
        "parser",
        "intake",
        "unit",
        "batch",
        "validate",
        "regen",
        "render",
        "transync_syntax",
        "transync_html",
        // ti `e4f4b0`: the gate is a SIBLING of the facade, not below it.
        // The owner ruled a language-detection dependency must not reach
        // `transync`, so a `transync::lang` path appearing here would mean
        // that ruling had been reversed in code without being reversed in
        // the record. Consumers depend on `transync-lang` directly.
        "transync_lang",
        "htmlseg",
        "outcome",
        "walk",
        // The three `pub(crate)` syntax aliases whose OLD facade paths were
        // themselves documented — the likeliest way a stale doc edit
        // reintroduces a hidden module into §0.
        "id",
        "align",
        "error",
    ];
    for path in table_paths() {
        for seg in path.split("::") {
            assert!(
                !forbidden.contains(&seg),
                "contracts.md §0 documents hidden module path: {path}"
            );
        }
    }
}

/// `CacheKey` is **exhaustive by policy** (contracts.md §1): a disk-backed
/// `Cache` implementation has to serialize every axis, so the field set is
/// part of the supported surface and a field addition is a breaking change
/// that rides a version bump. §0 rows cannot see that — they name items, not
/// fields — so the field set is welded here instead, from *outside* the
/// defining crate, where the exhaustive destructure below stops compiling
/// the moment an axis is added, removed, or renamed.
///
/// The `..` rest pattern is deliberately absent. Adding one would make this
/// test pass through exactly the change it exists to catch.
///
/// ti c02f69 added `instruction_hash` in the sanctioned 0.3.0 surface window;
/// ti 5f7942 added `input_mode` in the 0.4.0 one.
#[test]
fn cache_key_field_set_is_the_documented_one() {
    let key = transync::CacheKey {
        provider_fingerprint: transync::ProviderFingerprint::from_type_name("surface-gate"),
        validation_schema_version: transync::VALIDATION_SCHEMA_VERSION,
        source_hash: 1,
        source_lang: "en".to_string(),
        target_lang: "ko".to_string(),
        profile_version: "v1".to_string(),
        profile_prompt_hash: 2,
        glossary_hash: 3,
        model_id: "model".to_string(),
        block_kind: "paragraph".to_string(),
        input_mode: "text_fragment".to_string(),
        context_hash: 4,
        instruction_hash: 5,
    };
    let transync::CacheKey {
        provider_fingerprint: _,
        validation_schema_version: _,
        source_hash: _,
        source_lang: _,
        target_lang: _,
        profile_version: _,
        profile_prompt_hash: _,
        glossary_hash: _,
        model_id: _,
        block_kind: _,
        input_mode: _,
        context_hash: _,
        instruction_hash: _,
    } = key;
}

/// `DocumentMetaKey` is **exhaustive by policy** on `CacheKey`'s own ground
/// (contracts.md §0, DCR-0028): a disk-backed `Cache` has to serialize and
/// reconstruct every field of a document-level record, so the field set is part
/// of the supported surface and an addition is breaking-by-policy. Welded the
/// same way and for the same reason as `CacheKey`'s, from outside the defining
/// crate, with no `..` rest pattern.
///
/// The absent fields are as load-bearing as the present ones: the key
/// deliberately carries no prompt-identity axis (§5a records why a
/// document-scoped record cannot honestly name a batch-scoped prompt), so a
/// later "just add `profile_prompt_hash`" is a change that has to be argued
/// here rather than made quietly.
#[test]
fn document_meta_key_field_set_is_the_documented_one() {
    let key = transync::DocumentMetaKey {
        provider_fingerprint: transync::ProviderFingerprint::from_type_name("surface-gate"),
        validation_schema_version: transync::VALIDATION_SCHEMA_VERSION,
        model_id: "model".to_string(),
        source_lang: "auto".to_string(),
        target_lang: "ko".to_string(),
        doc_source_hash: 7,
    };
    let transync::DocumentMetaKey {
        provider_fingerprint: _,
        validation_schema_version: _,
        model_id: _,
        source_lang: _,
        target_lang: _,
        doc_source_hash: _,
    } = key;

    let meta = transync::DocumentMeta {
        detected_source_language: Some("en".to_string()),
    };
    let transync::DocumentMeta {
        detected_source_language: _,
    } = meta;
}

/// `GlossaryExtractionKey` is the document-scoped family's second record (ti
/// `dca5bf`) and is **exhaustive by policy** for the same reason as the first:
/// a disk backend serializes and reconstructs every field.
///
/// Its absences are again load-bearing. There is no `doc_source_hash` and no
/// `source_truncated` — a truncated run's harvest is about the excerpt the
/// model was shown, not about the bytes past the ceiling — and no
/// `profile_version`, because the extraction prompt carries no profile prompt
/// body. Everything that *did* reach the extractor is inside `request_hash`, a
/// digest of the assembled prompt bytes themselves, so "add one more axis" is a
/// change that has to be argued here rather than made quietly.
#[test]
fn glossary_extraction_key_field_set_is_the_documented_one() {
    let key = transync::GlossaryExtractionKey {
        provider_fingerprint: transync::ProviderFingerprint::from_type_name("surface-gate"),
        validation_schema_version: transync::VALIDATION_SCHEMA_VERSION,
        model_id: "model".to_string(),
        request_hash: 11,
    };
    let transync::GlossaryExtractionKey {
        provider_fingerprint: _,
        validation_schema_version: _,
        model_id: _,
        request_hash: _,
    } = key;

    let extraction = transync::GlossaryExtraction {
        terms: vec![transync::GlossaryEntry {
            source_term: "tensor".to_string(),
            target_term: "텐서".to_string(),
            note: None,
            scope: transync::GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }],
    };
    let transync::GlossaryExtraction { terms: _ } = extraction;
}

/// `ListTopologyEntry::depth` widened from `u8` to `u32` in the sanctioned
/// 0.3.0 surface window (ti 0d8277, owner decision 2026-08-06), and the whole
/// point of the change is that the field can now hold a depth a `u8` could
/// not. §0 rows name items, not field *types*, so — as with `CacheKey`'s axes
/// — nothing above sees a narrowing; this welds the width here, from outside
/// the defining crate.
///
/// `300` is the load-bearing literal: it does not fit a `u8`, so re-narrowing
/// the field is a red compile rather than a silently re-imposed ceiling. The
/// value is also the depth `structure::depth_ceiling_tests` walks to, where
/// the old saturation flattened every level from 255 on.
#[test]
fn list_topology_entry_depth_holds_more_than_a_byte() {
    let entry = transync::ListTopologyEntry {
        depth: 300,
        ordered: false,
        task: None,
        child_kinds: vec!["paragraph".to_string()],
        start: None,
        delimiter: None,
        tight: Some(true),
    };
    assert_eq!(
        entry.depth, 300,
        "contracts.md §0: the depth fingerprint is exact at any nesting"
    );
}

/// `VALIDATION_REPORT_SCHEMA_VERSION` joined §0 in the sanctioned 0.3.0 window
/// (ti 8a4a32) so a Rust consumer can name the report version *this build*
/// emits without running a translation. That is only worth having if the
/// constant equals the value the engine actually stamps into the artifact —
/// pinned here, from outside the defining crate, where the facade's re-export
/// is the only path to either side.
///
/// Deliberately not a pin on the literal `"1.0.0"`: a version bump must not
/// have to be re-typed *here*, in a gate about the surface rather than about
/// the value. One literal pin does exist, in `transync-cli`'s
/// `--validation-report` smoke, and it is on the wire — the bytes the binary
/// wrote — which is the one place a bump should be visible on purpose.
#[test]
fn validation_report_version_constant_matches_the_stamped_field() {
    let report = transync::ValidationReport::default();
    assert_eq!(
        report.schema_version,
        transync::VALIDATION_REPORT_SCHEMA_VERSION,
        "contracts.md §3a: the constant declares what this build emits, so it \
         must equal the `schema_version` the engine stamps into every report"
    );
}

#[test]
fn generator_version_matches_the_facade_crate() {
    let map = transync::AlignmentMap::default();
    assert_eq!(
        map.generator.version,
        env!("CARGO_PKG_VERSION"),
        "GeneratorMeta stamps transync-syntax's version (DCR-0017 M6); \
         it must stay in lockstep with the facade via version.workspace"
    );
    assert_eq!(map.generator.name, "transync");
}
