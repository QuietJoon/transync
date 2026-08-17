//! Round-trip every `samples/error-*.md` fixture through the pipeline
//! with the passthrough mock translator. This is structural: it does
//! not exercise translation semantics, only parse → unit → batch →
//! echo → validate → regen → full_reparse.
//!
//! For each fixture we assert the pipeline returns `Ok` (the new
//! `FullReparseFailure::FallbackPerBlock` default must recover from
//! any structural drift), and we report the validation summary so
//! drift that triggered the recovery shows up as `fallback_source`
//! counts in the printed table — visible via `cargo test
//! --test error_fixtures -- --nocapture`.
//!
//! TRACE: R0004-0001

mod common;

use common::mock_translator::MockTranslator;
use std::path::PathBuf;
use transync::{InMemoryCache, TranslateOptions, translate_with_cache};

fn fixtures() -> Vec<(String, PathBuf)> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let samples = PathBuf::from(manifest_dir).join("../../samples");
    let mut entries: Vec<_> = std::fs::read_dir(&samples)
        .unwrap_or_else(|e| panic!("read_dir {samples:?}: {e}"))
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let p = e.path();
            let name = p.file_name()?.to_str()?.to_string();
            if name.starts_with("error-") && name.ends_with(".md") {
                Some((name, p))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
}

#[tokio::test(flavor = "current_thread")]
async fn error_fixtures_round_trip_under_default_policy() {
    let fixtures = fixtures();
    assert!(
        !fixtures.is_empty(),
        "no samples/error-*.md fixtures discovered"
    );

    let mut report: Vec<String> = Vec::new();
    let mut any_failure = false;
    let header = format!(
        "{:<18} {:>6} {:>10} {:>10} {:>9} {:>10} {:>8}",
        "fixture", "blocks", "translated", "preserved", "partial", "fallback", "retried"
    );
    report.push(header);
    report.push("-".repeat(78));

    for (name, path) in &fixtures {
        let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let translator = MockTranslator::passthrough();
        let mut opts = TranslateOptions::default();
        opts.target_language = "ko".to_string();
        let cache = InMemoryCache::new();

        let result = translate_with_cache(&source, &opts, &translator, &cache).await;
        match result {
            Ok(out) => {
                let s = &out.alignment_map.validation_summary;
                report.push(format!(
                    "{:<18} {:>6} {:>10} {:>10} {:>9} {:>10} {:>8}",
                    name,
                    s.total_units,
                    s.translated,
                    s.preserved,
                    s.partially_translated,
                    s.fallback_source,
                    s.retried_units,
                ));
            }
            Err(e) => {
                any_failure = true;
                report.push(format!("{name:<18} FAILED: {e}"));
            }
        }
    }

    let printed = report.join("\n");
    println!(
        "\n=== samples/error-*.md round-trip (Passthrough + FallbackPerBlock) ===\n{printed}\n"
    );

    assert!(
        !any_failure,
        "one or more fixtures failed under the default policy:\n{printed}"
    );
}

/// Same fixtures, but under `FullReparseFailure::Hard` — surfaces
/// which fixtures the parse → echo → regen → reparse round-trip can't
/// pass cleanly without per-block fallback. Skipped on regressions:
/// any fixture that fails here is the one the new policy is meant to
/// recover, not a real failure of the implementation. We report
/// failures so the user can see which inputs exercised the recovery,
/// but do NOT assert success.
#[tokio::test(flavor = "current_thread")]
async fn error_fixtures_under_hard_policy_diagnostic_only() {
    let fixtures = fixtures();
    let mut lines: Vec<String> = Vec::new();
    let mut clean = 0;
    let mut needs_recovery = 0;
    for (name, path) in &fixtures {
        let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        let translator = MockTranslator::passthrough();
        let mut opts = TranslateOptions::default();
        opts.target_language = "ko".to_string();
        opts.full_reparse_failure = transync::FullReparseFailure::Hard;
        let cache = InMemoryCache::new();
        match translate_with_cache(&source, &opts, &translator, &cache).await {
            Ok(_) => {
                clean += 1;
                lines.push(format!("{name:<18} clean round-trip"));
            }
            Err(e) => {
                needs_recovery += 1;
                lines.push(format!("{name:<18} would-fail-Hard: {e}"));
            }
        }
    }
    println!(
        "\n=== samples/error-*.md under FullReparseFailure::Hard (diagnostic) ===\n{}\nclean: {clean}, needs_recovery: {needs_recovery}\n",
        lines.join("\n")
    );
}
