//! The served content types.
//!
//! A **table**, not a sniffer. The type is decided by the filename extension
//! and by nothing else: reading the first bytes of a file to guess what it is
//! is how a static server ends up serving an attacker-chosen document as
//! `text/html`, and the bundle's file set is fixed and known, so guessing buys
//! nothing here. Every response carries `X-Content-Type-Options: nosniff` as
//! well, so the browser does not re-open the question we just closed.
//!
//! The table covers exactly two file sets and stops:
//!
//! - the `--html-out` bundle — `index.html`, `source.html`, `target.html`
//!   (`.html`), `alignment.json` (`.json`), `sync.js` and `purify.min.js`
//!   (`.js`);
//! - what `scripts/test-browser.sh` assembles on top of it for the wasm demo
//!   — `.mjs`, `.wasm` (`WebAssembly.instantiateStreaming` rejects every other
//!   type) and the two `.md` payloads it renders from.
//!
//! Plus four types a hand-written page beside a bundle plausibly reaches for
//! (`.css`, `.png`, `.svg`, `.txt`) and `.map`, which is JSON. Anything else
//! is [`DEFAULT`] — `application/octet-stream`, which a browser downloads
//! rather than executes. Adding a type is a deliberate act; a new extension
//! arriving on its own gets the conservative answer.
//!
//! TRACE: SCN-13

use std::path::Path;

/// What an unknown extension gets: the type that asks the browser to render
/// nothing and interpret nothing.
pub const DEFAULT: &str = "application/octet-stream";

/// The `Content-Type` for `path`, by extension alone.
///
/// The extension is compared case-insensitively (`INDEX.HTML` is HTML), and a
/// path with no extension at all — a `README`, a dotfile — takes [`DEFAULT`].
pub fn content_type_for(path: &Path) -> &'static str {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return DEFAULT;
    };
    match ext.to_ascii_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "wasm" => "application/wasm",
        _ => DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ct(name: &str) -> &'static str {
        content_type_for(&PathBuf::from(name))
    }

    /// The six files the CLI actually emits. If this list ever disagrees with
    /// `output::HTML_BUNDLE_ENTRIES`, the bundle is being served with a type
    /// the shell did not expect.
    #[test]
    fn every_bundle_file_gets_a_real_type() {
        assert_eq!(ct("index.html"), "text/html; charset=utf-8");
        assert_eq!(ct("source.html"), "text/html; charset=utf-8");
        assert_eq!(ct("target.html"), "text/html; charset=utf-8");
        assert_eq!(ct("alignment.json"), "application/json; charset=utf-8");
        assert_eq!(ct("sync.js"), "text/javascript; charset=utf-8");
        assert_eq!(ct("purify.min.js"), "text/javascript; charset=utf-8");
    }

    /// The wasm demo leg `scripts/test-browser.sh` lays on top of a bundle.
    /// `application/wasm` is not a nicety: `instantiateStreaming` refuses to
    /// compile a module served as anything else.
    #[test]
    fn the_wasm_demo_leg_gets_its_types() {
        assert_eq!(ct("transync_wasm_bg.wasm"), "application/wasm");
        assert_eq!(ct("transync_wasm.js"), "text/javascript; charset=utf-8");
        assert_eq!(ct("wasm-demo.mjs"), "text/javascript; charset=utf-8");
        assert_eq!(ct("source.md"), "text/markdown; charset=utf-8");
    }

    #[test]
    fn an_unknown_or_absent_extension_takes_the_conservative_default() {
        assert_eq!(ct("payload.bin"), DEFAULT);
        assert_eq!(ct("archive.tar.zst"), DEFAULT);
        assert_eq!(ct("README"), DEFAULT);
        assert_eq!(ct(".transync-publish.lock"), DEFAULT);
        assert_eq!(
            ct("index.php"),
            DEFAULT,
            "an extension this server does not know is data, never something \
             a browser or a downstream proxy should interpret",
        );
    }

    #[test]
    fn the_extension_match_is_case_insensitive() {
        assert_eq!(ct("INDEX.HTML"), "text/html; charset=utf-8");
        assert_eq!(ct("Sync.JS"), "text/javascript; charset=utf-8");
    }
}
