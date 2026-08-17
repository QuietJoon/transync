//! Inline-protection layer, end-to-end (EXT-2026-07 P1-5; ADR-0012
//! amendment). Two paths through the real pipeline:
//!
//! - A tampered link destination under the default profile
//!   (`preserve_urls = true`) is rejected by `ValidationLayer::Inline`,
//!   retried verbatim, and — because the mock is deterministic — falls
//!   back to the SOURCE bytes, so the swapped URL never reaches output.
//! - The identical rewrite is accepted when the profile opts into URL
//!   localization (`preserve_urls = false`): no inline rejection, and the
//!   rewritten destination survives into the translated Markdown.
//!
//! TRACE: EXT-2026-07 P1-5

use crate::common::mock_translator::MockTranslator;
use transync::{FallbackStatus, ValidationLayer, profile, translate};

#[tokio::test]
async fn inline_destination_tamper_retries_then_falls_back() {
    // Default profile pledges `preserve_urls = true`, so a swapped
    // destination is enforced.
    let source = "See [the docs](https://example.com/a) for details.\n";
    let translator =
        MockTranslator::tamper_payload("https://example.com/a", "https://evil.example/x");
    let mut opts = crate::common::opts_for("ko");
    opts.max_per_unit_validation_retries = 2;

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok");

    // The paragraph unit must end up flagged FallbackSource.
    let para = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "paragraph")
        .expect("alignment map contains the paragraph");
    assert_eq!(
        para.fallback_status,
        FallbackStatus::FallbackSource,
        "a deterministic destination tamper must exhaust retries and fall back",
    );

    // The attempt log names the inline layer.
    let record = output
        .validation_report
        .per_unit
        .iter()
        .find(|r| r.unit_id == para.source_block_id)
        .expect("validation report contains the paragraph unit");
    assert!(
        record
            .attempts
            .iter()
            .any(|a| a.rejected_by == Some(ValidationLayer::Inline)),
        "at least one attempt must be rejected by the inline layer: {:?}",
        record.attempts,
    );

    // Output carries the SOURCE URL and never the tampered one.
    assert!(
        output.translated_document.contains("https://example.com/a"),
        "fallback must splice the source destination:\n{}",
        output.translated_document,
    );
    assert!(
        !output
            .translated_document
            .contains("https://evil.example/x"),
        "the tampered destination must never reach the output:\n{}",
        output.translated_document,
    );

    // One unit, retried the full budget: total_retries == the budget.
    assert_eq!(
        output.validation_report.total_retries, opts.max_per_unit_validation_retries,
        "the single tampered unit consumes exactly its retry budget",
    );
}

#[tokio::test]
async fn preserve_urls_false_accepts_localized_destination() {
    // A profile that opts into URL localization disables the destination
    // check, so the same rewrite is a legitimate translation decision.
    let profile_toml = r#"
        slug = "localize-urls"
        version = "1.0.0"
        [system]
        prompt = "Translate from {{source_language}} to {{target_language}}."
        [constraints]
        preserve_urls = false
    "#;
    let custom = profile::load_profile(profile_toml).expect("custom profile loads");

    let source = "See [the guide](https://example.com/en/guide) here.\n";
    let translator = MockTranslator::tamper_payload(
        "https://example.com/en/guide",
        "https://example.com/ko/guide",
    );
    let mut opts = crate::common::opts_for("ko");
    opts.profile = Some(custom);

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok");

    // The paragraph unit is accepted, not downgraded.
    let para = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "paragraph")
        .expect("alignment map contains the paragraph");
    assert_eq!(
        para.fallback_status,
        FallbackStatus::Translated,
        "localizable URLs must accept the rewritten destination",
    );

    // The rewritten destination survives into the output.
    assert!(
        output
            .translated_document
            .contains("https://example.com/ko/guide"),
        "the localized destination must reach the output:\n{}",
        output.translated_document,
    );

    // No attempt was rejected by the inline layer.
    let inline_rejections = output
        .validation_report
        .per_unit
        .iter()
        .flat_map(|r| r.attempts.iter())
        .filter(|a| a.rejected_by == Some(ValidationLayer::Inline))
        .count();
    assert_eq!(
        inline_rejections, 0,
        "preserve_urls = false must produce zero inline rejections",
    );
}
