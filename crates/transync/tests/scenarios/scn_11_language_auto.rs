//! SCN-11 — `source-language=auto` echoed.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-11:
//! - `TranslateOptions { source_language: "auto", ... }` propagates.
//! - `AlignmentMap.detected_source_language == Some("fr")` after the run.
//! - `TranslationOutput.detected_source_language == Some("fr")` is also surfaced.
//!
//! The second test is not an SL-11 criterion: it pins which layer normalizes
//! the sentinel's spelling (ti fd5aa8).
//!
//! TRACE: SCN-11
//! TRACE: SL-11

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_11() {
    let source = include_str!("../fixtures/scn-11-language-auto.md");
    let translator = MockTranslator::echoes_detected("fr");
    let mut opts = crate::common::opts_for("en");
    opts.source_language = "auto".to_string();

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-11 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.3.0");
    assert_eq!(
        output.alignment_map.source_language, "auto",
        "alignment map should echo opts.source_language verbatim",
    );
    assert_eq!(
        output.alignment_map.target_language, "en",
        "alignment map should echo opts.target_language verbatim",
    );
    assert_eq!(
        output.alignment_map.detected_source_language.as_deref(),
        Some("fr"),
        "detected_source_language should be threaded into the alignment map",
    );
    assert_eq!(
        output.detected_source_language.as_deref(),
        Some("fr"),
        "detected_source_language should be surfaced on TranslationOutput",
    );
}

/// ti fd5aa8 / ADR-0013 (amended 2026-08-07): the **library** layer does not
/// fold the sentinel's spelling. `profile::render_prompt_body` recognizes
/// `AUTO` and compiles the auto-detection phrase — pinned there by
/// `the_auto_sentinel_survives_padding_and_case` — but
/// `TranslateOptions::source_language` still reaches the cache key and the
/// alignment map byte-for-byte, because normalizing belongs to the argument
/// boundary: the layer that knows a value was typed on a command line.
/// `transync-cli` canonicalizes before it builds these options, which is why
/// no CLI run can produce the map this test asserts.
#[tokio::test]
async fn scn_11_the_library_keeps_the_sentinels_spelling() {
    let source = include_str!("../fixtures/scn-11-language-auto.md");
    let translator = MockTranslator::echoes_detected("fr");
    let mut opts = crate::common::opts_for("en");
    opts.source_language = "AUTO".to_string();

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-11 fixture");

    assert_eq!(
        output.alignment_map.source_language, "AUTO",
        "the library echoes the label it was handed; only the CLI canonicalizes it",
    );
    assert_eq!(
        output.alignment_map.detected_source_language.as_deref(),
        Some("fr"),
        "and the provider's detection still reaches the map",
    );
}
