//! SCN-07 — Validation retry → success.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-07:
//! - Mock invoked exactly twice (initial batch + one-unit retry batch).
//! - Final output uses the corrected payload for the retried unit.
//! - ValidationReport.per_unit[<retried-unit>].attempts.len() == 2.
//! - First attempt has rejected_by == Some(_).
//!
//! TRACE: SCN-07
//! TRACE: SL-07

use crate::common::mock_translator::MockTranslator;
use transync::{FallbackStatus, translate};

#[tokio::test]
async fn smoke_scn_07() {
    let source = include_str!("../fixtures/scn-07-validation-retry.md");
    let translator = MockTranslator::rejects_then_accepts();
    let mut opts = crate::common::opts_for("ko");
    opts.max_per_unit_validation_retries = 2;

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-07 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.2.0");
    assert!(!output.translated_document.is_empty());

    // The fixture's table unit is the only one whose truncated attempt-1
    // payload fails per-kind validation; after retry it must end up
    // Translated. Other units may stay PartiallyTranslated (the mock's
    // truncation does not break them structurally).
    let table_row = output
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "table")
        .expect("alignment map should contain the SCN-07 table");
    assert_eq!(
        table_row.fallback_status,
        FallbackStatus::Translated,
        "table block should be Translated after the retry succeeds",
    );

    // The mock should have been invoked at least twice (initial batch +
    // one-unit retry batch for the table).
    let calls = translator.call_count();
    assert!(
        calls >= 2,
        "rejects-then-accepts mock should be invoked at least twice, got {calls}",
    );

    // ValidationReport: the table unit should have 2 attempts; first one
    // rejected by per-kind shape; second one accepted.
    let table_record = output
        .validation_report
        .per_unit
        .iter()
        .find(|r| r.unit_id == table_row.source_block_id)
        .expect("ValidationReport should contain the table unit");
    assert_eq!(
        table_record.attempts.len(),
        2,
        "table unit should have exactly 2 attempts",
    );
    assert!(
        table_record.attempts[0].rejected_by.is_some(),
        "first attempt for the table should record a rejection layer",
    );
    assert!(
        table_record.attempts[1].rejected_by.is_none(),
        "second attempt for the table should be accepted",
    );

    // ValidationReport.total_retries should reflect the retried unit.
    assert!(
        output.validation_report.total_retries >= 1,
        "ValidationReport.total_retries should be >= 1",
    );
    // `R0001-0025` in the removed `reviews/reviewed/0001.md` (the live
    // round's 0025 is the list-depth `u8` overflow): the alignment map
    // carries its own retry counter, which the
    // report phase derives from the per-unit attempt log above
    // (`pipeline::report::assemble_alignment_map`). No test pinned it at a
    // non-zero value, and a run that never retries cannot distinguish a
    // working derivation from a hardcoded zero — SCN-07 is the run that can.
    // Of this fixture's three units the mock's truncated attempt-1 payload is
    // rejected for two (the paragraph and the table); the heading is accepted
    // first try.
    assert_eq!(
        output.alignment_map.validation_summary.retried_units,
        2,
        "two of the three units took a second attempt, per-unit log: {:?}",
        output
            .validation_report
            .per_unit
            .iter()
            .map(|r| (r.unit_id.0.as_str(), r.attempts.len()))
            .collect::<Vec<_>>(),
    );
}
