//! Guard against drift between the workspace `web/js/sync.js` and the
//! CLI-embedded copy at `crates/transync-cli/web/sync.js`.
//!
//! The CLI emits its embedded copy into `--html-out` for offline /
//! self-contained demos; the workspace copy is what `python3 -m
//! http.server` serves from the repo root. The two files must stay in
//! lockstep — anything else surfaces as different sync behavior
//! between the workspace demo and the CLI bundle, which has bitten us
//! before (commit comments on the embedded copy drifted from the
//! workspace copy in real history).
//!
//! It also carries the alignment-schema lockstep for every JS mirror of
//! `ALIGNMENT_SCHEMA_VERSION` — both `sync.js` copies and, since ADR-0019,
//! `web/js/wasm-demo.js` — and, since R0001-0043, the JS mirror of the
//! `SyncRole` enumeration that the engine's row gate refuses unknown values
//! against.
//!
//! TRACE: R0001-0067 in `reviews/reviewed/0001.md` (removed) — the
//! bare `R0001-0043` above is the live round, `reviews/0001.md`.

/// OI-0001: same lockstep guarantee for the vendored DOMPurify build —
/// the CLI embeds `crates/transync-cli/web/purify.min.js` into the
/// bundle while the workspace demo serves `web/vendor/purify.min.js`.
#[test]
fn purify_js_workspace_and_embedded_are_byte_identical() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .join("../../web/vendor/purify.min.js")
        .canonicalize()
        .expect("workspace web/vendor/purify.min.js should exist");
    let embedded = manifest
        .join("web/purify.min.js")
        .canonicalize()
        .expect("CLI-embedded crates/transync-cli/web/purify.min.js should exist");
    assert_eq!(
        std::fs::read(&workspace).expect("workspace purify readable"),
        std::fs::read(&embedded).expect("embedded purify readable"),
        "purify.min.js drifted between {} and {}",
        workspace.display(),
        embedded.display(),
    );
}

#[test]
fn sync_js_workspace_and_embedded_are_byte_identical() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest
        .join("../../web/js/sync.js")
        .canonicalize()
        .expect("workspace web/js/sync.js should exist");
    let embedded = manifest
        .join("web/sync.js")
        .canonicalize()
        .expect("CLI-embedded crates/transync-cli/web/sync.js should exist");

    let workspace_bytes = std::fs::read(&workspace).expect("workspace sync.js should be readable");
    let embedded_bytes = std::fs::read(&embedded).expect("embedded sync.js should be readable");

    assert_eq!(
        workspace_bytes,
        embedded_bytes,
        "sync.js drifted between workspace ({}) and CLI-embedded ({}). \
         Re-copy one to match the other so the demo and CLI bundle stay \
         in lockstep.",
        workspace.display(),
        embedded.display(),
    );
}

/// Pull `major`/`minor`/`patch` out of sync.js's
/// `const KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 };` line.
fn known_schema_triple(js: &str) -> (u32, u32, u32) {
    let line = js
        .lines()
        .find(|l| l.contains("KNOWN_SCHEMA = {"))
        .expect("sync.js should declare KNOWN_SCHEMA");
    let field = |name: &str| -> u32 {
        let after = line
            .split_once(&format!("{name}:"))
            .unwrap_or_else(|| panic!("KNOWN_SCHEMA line should carry `{name}:` — got {line:?}"))
            .1;
        after
            .trim_start()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect::<String>()
            .parse()
            .unwrap_or_else(|e| panic!("`{name}` in {line:?} should be a number: {e}"))
    };
    (field("major"), field("minor"), field("patch"))
}

/// The last unpinned leg of the alignment-schema lockstep: the Rust emitter
/// bumps [`transync::ALIGNMENT_SCHEMA_VERSION`], and the JS consumer's
/// forward-drift guard (OI-0024) compares against its own `KNOWN_SCHEMA`
/// literal. A bump on one side without the other silently turns real drift
/// into a "compatible" read (or a spurious warning), so pin them here.
#[test]
fn sync_js_known_schema_matches_the_rust_alignment_schema_version() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rust = transync::ALIGNMENT_SCHEMA_VERSION;
    let mut parts = rust.split('.');
    let mut next = |what: &str| -> u32 {
        parts
            .next()
            .unwrap_or_else(|| panic!("ALIGNMENT_SCHEMA_VERSION {rust:?} should have a {what}"))
            .parse()
            .unwrap_or_else(|e| panic!("{what} of {rust:?} should be a number: {e}"))
    };
    let expected = (next("major"), next("minor"), next("patch"));
    assert_eq!(
        parts.next(),
        None,
        "{rust:?} should be exactly major.minor.patch"
    );

    for rel in ["../../web/js/sync.js", "web/sync.js"] {
        let path = manifest
            .join(rel)
            .canonicalize()
            .unwrap_or_else(|e| panic!("{rel} should exist: {e}"));
        let js = std::fs::read_to_string(&path).expect("sync.js should be readable");
        assert_eq!(
            known_schema_triple(&js),
            expected,
            "KNOWN_SCHEMA in {} drifted from ALIGNMENT_SCHEMA_VERSION ({rust}). \
             Bump both, or the JS forward-drift guard (OI-0024) reads real \
             drift as compatible.",
            path.display(),
        );
    }
}

/// The wire form of every [`transync::SyncRole`], by exhaustive match.
///
/// Exhaustive on purpose: adding a variant to `SyncRole` stops compiling
/// here until its wire value is spelled out, which is the same moment the
/// JS mirror below has to grow it.
fn sync_role_wire(role: transync::SyncRole) -> &'static str {
    match role {
        transync::SyncRole::Anchor => "anchor",
        transync::SyncRole::Container => "container",
        transync::SyncRole::ChildOnly => "child-only",
        transync::SyncRole::NonSync => "non-sync",
    }
}

/// Pull the members out of sync.js's
/// `const KNOWN_SYNC_ROLES = new Set(["anchor", …]);` line.
fn known_sync_roles(js: &str) -> Vec<String> {
    let line = js
        .lines()
        .find(|l| l.contains("KNOWN_SYNC_ROLES = new Set("))
        .expect("sync.js should declare KNOWN_SYNC_ROLES");
    let inner = line
        .split_once('[')
        .and_then(|(_, rest)| rest.split_once(']'))
        .unwrap_or_else(|| {
            panic!("KNOWN_SYNC_ROLES line should carry a [..] literal — got {line:?}")
        })
        .0;
    inner
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_matches('"').to_string())
        .collect()
}

/// R0001-0043: `loadAlignment` now REFUSES an in-band map whose row carries
/// a `sync_role` outside the JS mirror, so that mirror has to match the Rust
/// enumeration exactly. Too narrow and the engine rejects maps the emitter
/// legitimately produces; too wide and a corrupt role is waved through.
/// (A newer-minor map is the deliberate exception — contracts.md §3 lets a
/// minor bump add enumerated values, and the engine warns instead.)
#[test]
fn sync_js_known_sync_roles_match_the_rust_sync_role_enum() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rust: Vec<String> = [
        transync::SyncRole::Anchor,
        transync::SyncRole::Container,
        transync::SyncRole::ChildOnly,
        transync::SyncRole::NonSync,
    ]
    .into_iter()
    .map(|role| {
        // Pin the hand-written arm against serde's kebab-case rename, so the
        // arm cannot drift from what actually goes on the wire.
        let serialized = serde_json::to_value(role).expect("SyncRole serializes");
        let serialized = serialized.as_str().expect("SyncRole is a JSON string");
        assert_eq!(
            serialized,
            sync_role_wire(role),
            "sync_role_wire disagrees with serde for {role:?}"
        );
        serialized.to_string()
    })
    .collect();

    for rel in ["../../web/js/sync.js", "web/sync.js"] {
        let path = manifest
            .join(rel)
            .canonicalize()
            .unwrap_or_else(|e| panic!("{rel} should exist: {e}"));
        let js = std::fs::read_to_string(&path).expect("sync.js should be readable");
        assert_eq!(
            known_sync_roles(&js),
            rust,
            "KNOWN_SYNC_ROLES in {} drifted from the Rust `SyncRole` enumeration. \
             Mirror the wire values into BOTH sync.js copies, or the engine \
             refuses maps carrying the new role.",
            path.display(),
        );
    }
}

/// Pull the version out of wasm-demo.js's `const KNOWN_SCHEMA = "1.2.0";`
/// line. The demo mirrors the schema as one whole string rather than
/// sync.js's destructured triple, because it compares against the wasm
/// module's `schema_version()` as well as the fetched map.
fn known_schema_literal(js: &str) -> String {
    let line = js
        .lines()
        .find(|l| l.contains("const KNOWN_SCHEMA = \""))
        .expect("wasm-demo.js should declare KNOWN_SCHEMA as a string literal");
    line.split('"')
        .nth(1)
        .unwrap_or_else(|| panic!("KNOWN_SCHEMA line should carry a quoted version — got {line:?}"))
        .to_string()
}

/// The demo leg of the same lockstep (ADR-0019). `web/js/wasm-demo.js` is a
/// third schema mirror, and it was pinned only by the browser suite — so a
/// bump to [`transync::ALIGNMENT_SCHEMA_VERSION`] could land green on
/// `cargo test` and surface only later, as the demo's fail-closed boot
/// refusing to mount against its own renderer. Pin it here too.
#[test]
fn wasm_demo_js_known_schema_matches_the_rust_alignment_schema_version() {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = manifest
        .join("../../web/js/wasm-demo.js")
        .canonicalize()
        .expect("workspace web/js/wasm-demo.js should exist");
    let js = std::fs::read_to_string(&path).expect("wasm-demo.js should be readable");
    assert_eq!(
        known_schema_literal(&js),
        transync::ALIGNMENT_SCHEMA_VERSION,
        "KNOWN_SCHEMA in {} drifted from ALIGNMENT_SCHEMA_VERSION. Bump both, \
         or the wasm demo refuses to mount against the renderer it was built \
         from.",
        path.display(),
    );
}
