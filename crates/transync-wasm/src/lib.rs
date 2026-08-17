//! transync-wasm: the browser (wasm-bindgen) surface over `transync-syntax`.
//!
//! The demo page renders both panes with the *same* Rust renderer the CLI
//! uses — the standing invariant is that a second Markdown parser never
//! enters the system, so the renderer crosses to the browser instead
//! (ADR-0019; the compile path and its gate came from DCR-0017).
//!
//! This module is deliberately thin: three `#[wasm_bindgen]` entry points
//! that stringify, delegate to [`engine`], and re-serialize. All logic —
//! and all of the tests — live in [`engine`], which knows nothing about
//! wasm-bindgen and therefore runs on the host.
//!
//! # Boundary contract
//!
//! JSON strings in, one JSON string out (spec 2026-08-05 §2, approach A1):
//! no `serde-wasm-bindgen`, no new dependency, one `JSON.parse` per call on
//! the JS side. Errors become thrown JS exceptions carrying the stringified
//! [`engine::EngineError`] message.
//!
//! The rendered fragments are still untrusted-source content (invariant 7):
//! the mounting page must keep the shells' fail-closed DOMPurify step and
//! must never bypass it with a raw `insertAdjacentHTML`.
//!
//! TRACE: ADR-0019

pub mod engine;

use wasm_bindgen::prelude::*;

/// View mode — re-render both panes from an already-translated triple.
///
/// Returns `{"source_html": …, "target_html": …}`. `alignment_json` must be
/// the map built against *this* `translated_md`; see
/// [`engine::render_pair_impl`].
#[wasm_bindgen]
pub fn render_pair(
    source_md: &str,
    translated_md: &str,
    alignment_json: &str,
) -> Result<String, JsError> {
    let out = engine::render_pair_impl(source_md, translated_md, alignment_json).map_err(to_js)?;
    encode(&out)
}

/// Edit mode — regen, rebuild the alignment map, render both panes.
///
/// Returns `{"source_html": …, "target_html": …, "alignment_json": …,
/// "translated_md": …}`, where `alignment_json` is itself a JSON string
/// (nested — the caller parses it a second time or hands it straight back
/// to [`render_pair`]). See [`engine::rebuild_impl`] for the input shapes.
#[wasm_bindgen]
pub fn rebuild(
    source_md: &str,
    payloads_json: &str,
    statuses_json: &str,
    source_lang: &str,
    target_lang: &str,
    detected: Option<String>,
) -> Result<String, JsError> {
    let out = engine::rebuild_impl(
        source_md,
        payloads_json,
        statuses_json,
        source_lang,
        target_lang,
        detected,
    )
    .map_err(to_js)?;
    encode(&out)
}

/// The alignment-map schema version this module speaks.
///
/// The demo asserts it against its own mirrored constant at init and
/// refuses to mount on mismatch (fail-closed).
#[wasm_bindgen]
pub fn schema_version() -> String {
    engine::schema_version_impl().to_string()
}

/// Serialize an engine output struct for the boundary.
fn encode<T: serde::Serialize>(value: &T) -> Result<String, JsError> {
    serde_json::to_string(value)
        .map_err(|e| JsError::new(&format!("result did not serialize: {e}")))
}

/// Stringify an engine error for the JS `throw`.
fn to_js(err: engine::EngineError) -> JsError {
    JsError::new(&err.to_string())
}
