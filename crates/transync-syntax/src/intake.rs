//! The format seam: one intake per source format, one block IR out of both
//! (spec 2026-08-20 §4, decision D3).
//!
//! [`html`] is the HTML document intake. The Markdown intake is
//! [`crate::parser`] — the spec sketches it as `intake::markdown`, and that
//! rename is **deferred, on record** (DCR-0035): `parser` is named by over a
//! hundred `crate::parser` / `transync_syntax::parser` paths across all four
//! consuming crates, by `transync-core`'s crate-private re-export, and by
//! `public_surface.rs`'s hidden-path pins, and moving it buys no behaviour.
//! The seam is real without it: both intakes produce the same [`Document`],
//! normalize NUL through the same `parser::normalize_source` (crate-private),
//! stamp ids with the same global-ordinal scheme, and share THE
//! heading-scope stack (`parser::sections`).
//!
//! Not to be confused with [`crate::parser::intake`], the NUL/nesting guard
//! *function* on the Markdown side (OI-0034), which predates this module and
//! shares the word.
//!
//! [`Document`]: crate::parser::Document
//!
//! TRACE: ADR-0025

pub mod html;
