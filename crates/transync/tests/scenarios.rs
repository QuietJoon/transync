//! Integration test aggregator. Each `scn_NN_*` submodule covers one
//! mandatory scenario per `docs/architecture/scenario-matrix.md`.
//!
//! After Phase-3 implementation, every scenario asserts its done-gate
//! criteria from `docs/project/implementation-slice-checklists.md`. The
//! `MockTranslator` modes in `common::mock_translator` cover passthrough,
//! validation rejection, persistent failure, recording, all-fallback, and
//! `detected_source_language` echo behaviors so each scenario can drive
//! the real pipeline through its expected path.
//!
//! TRACE: SCN-01..SCN-15

mod common;

#[path = "scenarios/scn_01_headings_and_paragraphs.rs"]
mod scn_01_headings_and_paragraphs;
#[path = "scenarios/scn_02_table_small.rs"]
mod scn_02_table_small;
#[path = "scenarios/scn_03_table_large.rs"]
mod scn_03_table_large;
#[path = "scenarios/scn_04_code_block.rs"]
mod scn_04_code_block;
#[path = "scenarios/scn_05_nested_list.rs"]
mod scn_05_nested_list;
#[path = "scenarios/scn_06_blockquote.rs"]
mod scn_06_blockquote;
#[path = "scenarios/scn_07_validation_retry.rs"]
mod scn_07_validation_retry;
#[path = "scenarios/scn_08_fallback.rs"]
mod scn_08_fallback;
#[path = "scenarios/scn_09_prompt_injection.rs"]
mod scn_09_prompt_injection;
#[path = "scenarios/scn_10_long_document.rs"]
mod scn_10_long_document;
#[path = "scenarios/scn_11_language_auto.rs"]
mod scn_11_language_auto;
#[path = "scenarios/scn_14_full.rs"]
mod scn_14_full;
#[path = "scenarios/scn_15_html_blocks.rs"]
mod scn_15_html_blocks;

// EXT-2026-07 P1-5: inline-protection layer (link/image destinations +
// policy-gated inline code spans; ADR-0012 amendment).
#[path = "scenarios/inline_protection.rs"]
mod inline_protection;
