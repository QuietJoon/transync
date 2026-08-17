//! One default profile file exists, and it is the one the runtime embeds.
//!
//! `transync-core` `include_str!`s `crates/transync-core/profiles/default.toml`
//! into `DEFAULT_PROFILE_TOML`; that file is the only default profile the
//! pipeline — CLI included, since it resolves its default through
//! `profile::default_profile()` — ever sees.
//!
//! A second copy lived at `crates/transync-cli/profiles/default.toml` until
//! 2026-08-06. No `include_str!`, no source file, no test and no script
//! referenced it, and its comments had already drifted from the core copy, so
//! a contributor editing the CLI-labelled file changed nothing at all. It was
//! deleted rather than welded, because nothing needed a second copy; this test
//! keeps it deleted. If a CLI-side copy ever becomes genuinely necessary,
//! replace this test with a byte-equality weld in the style of
//! `sync_js_drift.rs`, which guards the copies that really must exist twice.
//!
//! TRACE: R0001-0041

#[test]
fn the_cli_carries_no_second_default_profile() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    let canonical = manifest.join("../transync-core/profiles/default.toml");
    assert!(
        canonical.exists(),
        "the canonical default profile is missing at {} — transync-core \
         include_str!s it, so this test is reading the wrong path",
        canonical.display(),
    );

    let duplicate = manifest.join("profiles/default.toml");
    assert!(
        !duplicate.exists(),
        "{} is back. Nothing embeds it: the default the runtime sees is {}. \
         Edit that file instead, or — if a CLI-side copy is truly needed — \
         weld the two byte-identically the way sync_js_drift.rs does.",
        duplicate.display(),
        canonical.display(),
    );
}
