//! SCN-08 — Persistent failure → fallback to source.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-08:
//! - Failed unit has `fallback_status == FallbackSource`.
//! - Rest of the document translates normally.
//! - Target HTML carries `data-fallback="fallback_source"` on that block.
//! - Sync anchor (`data-sync-id`) is still present on the fallback block.
//!
//! TRACE: SCN-08
//! TRACE: SL-08

use crate::common::mock_translator::MockTranslator;
use transync::{BlockId, FallbackStatus, translate};

#[tokio::test]
async fn smoke_scn_08() {
    let source = include_str!("../fixtures/scn-08-fallback.md");
    let target_unit = BlockId::new("p", 3);
    let translator = MockTranslator::always_fails_unit(target_unit.clone());
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-08 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.2.0");
    assert!(!output.translated_document.is_empty());

    // The failing unit must end up flagged FallbackSource.
    let failing_row = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id == target_unit)
        .expect("alignment map should still include the failing unit");
    assert_eq!(
        failing_row.fallback_status,
        FallbackStatus::FallbackSource,
        "failing unit should be FallbackSource after retries exhaust",
    );

    // Other units must remain Translated.
    for row in &output.alignment_map.blocks {
        if row.source_block_id != target_unit {
            assert_eq!(
                row.fallback_status,
                FallbackStatus::Translated,
                "block {} should remain Translated",
                row.source_block_id,
            );
        }
    }

    // Target HTML must carry the fallback attribute on the failing block,
    // alongside its sync-id anchor.
    let needle_id = format!("data-sync-id=\"{}\"", target_unit);
    let needle_fallback = "data-fallback=\"fallback_source\"";
    let fallback_block_html = output
        .annotated_target_html
        .lines()
        .find(|l| l.contains(&needle_id))
        .expect("target HTML must include the fallback unit's wrapper");
    assert!(
        fallback_block_html.contains(needle_fallback),
        "fallback block should carry data-fallback=\"fallback_source\"",
    );

    // Validation report should record this as a fallback.
    assert!(
        output.validation_report.total_fallbacks >= 1,
        "ValidationReport.total_fallbacks should be >= 1",
    );
}
