//! transync-syntax: the wasm32-compilable base crate for transync.
//!
//! Owns everything syntax-side: GFM parsing into the block IR, block IDs,
//! Markdown regeneration/splicing, alignment-map construction, annotated
//! HTML rendering. The lol_html segment engine moved to `transync-html`
//! (DCR-0032), which this crate depends on. `transync-core` builds
//! the LLM pipeline on top. Gate: this crate must always pass
//! `cargo check -p transync-syntax --target wasm32-unknown-unknown`.
//!
//! TRACE: DCR-0017

pub mod align;
pub mod error;
pub mod id;
pub mod outcome;
pub mod parser;
pub mod regen;
pub mod render;
pub mod walk;
