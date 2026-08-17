//! Shared helpers for the integration-test suite.
//!
//! TRACE: SCN-01..SCN-14

#![allow(dead_code)]

pub mod fixture_gen;
pub mod mock_translator;

use transync::TranslateOptions;

/// Canonical scenario options: everything default except the required
/// target language. Default-then-assign, not FRU, so this keeps
/// compiling when `TranslateOptions` becomes `#[non_exhaustive]`
/// (OI-0027).
pub fn opts_for(target_language: &str) -> TranslateOptions {
    let mut opts = TranslateOptions::default();
    opts.target_language = target_language.to_string();
    opts
}
