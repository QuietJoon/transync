//! The format seam: one intake per source format, one block IR out of both
//! (spec 2026-08-20 §4, decision D3).
//!
//! [`html`] is the HTML document intake, [`markdown`] the Markdown one. The
//! seam is symmetric: both intakes produce the same [`Document`], normalize
//! NUL through the same `markdown::normalize_source` (crate-private), stamp
//! ids with the same global-ordinal scheme, and share THE heading-scope stack
//! (`markdown::sections`).
//!
//! Not to be confused with [`crate::intake::markdown::intake`], the
//! NUL/nesting guard *function* on the Markdown side (OI-0034), which
//! predates this module and shares the word.
//!
//! [`Document`]: crate::intake::markdown::Document
//!
//! TRACE: ADR-0025

pub mod html;
pub mod markdown;
