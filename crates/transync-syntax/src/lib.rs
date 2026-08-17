//! transync-syntax: the wasm32-compilable base crate for transync.
//!
//! Owns everything syntax-side: GFM parsing into the block IR, block IDs,
//! Markdown regeneration/splicing, alignment-map construction, annotated
//! HTML rendering, and the lol_html segment engine. `transync-core` builds
//! the LLM pipeline on top. Gate: this crate must always pass
//! `cargo check -p transync-syntax --target wasm32-unknown-unknown`.
//!
//! TRACE: DCR-0017

pub mod align;
pub mod error;
// Internal engine, not curated API: the lol_html extract/splice/inventory
// routines are consumed by `regen`/`render` inside this crate and by
// `unit`/`validate` in `transync-core`, which is the only reason they are
// `pub` at all. OI-0027 curates the real surface and gets this list.
#[doc(hidden)]
pub mod htmlseg;
pub mod id;
pub mod outcome;
pub mod parser;
pub mod regen;
pub mod render;
pub mod walk;
