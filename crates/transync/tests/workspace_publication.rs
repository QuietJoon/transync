//! The publication roster and the internal-version bump obligation live in the
//! manifests, so they are checked against the manifests.
//!
//! `c447700f` (R0001-0040) made the workspace publishable and wrote the roster
//! down. Until this file, both facts were held by prose alone — a comment in
//! the root `Cargo.toml` and steps 17/19 of
//! `docs/project/release-checklist.md` — and prose does not fail a build:
//!
//! 1. **The internal requirements shadow the workspace version.** The root
//!    `[workspace.dependencies]` table declares `transync-syntax`,
//!    `transync-core`, `transync` and `transync-openai` as
//!    `{ version = "X.Y.Z", path = "crates/…" }`. Both keys are load-bearing —
//!    `path` is what the workspace builds against, `version` is what
//!    `cargo publish` demands — and the `version` half does **not** inherit
//!    from `[workspace.package]`. Cargo only notices a stale one when the bump
//!    is minor or major; after a *patch* bump `^0.2.0` still accepts `0.2.1`,
//!    so `cargo build --workspace` stays green while the published manifest
//!    would understate what its sibling actually needs.
//! 2. **The private set is exactly `{transync-wasm}`** (`publish = false`,
//!    ADR-0019). Publishing is cargo's default, so a member added without a
//!    decision joins the roster by saying nothing at all.
//!
//! The only mechanical check for either was `cargo publish --dry-run
//! --workspace`, which verify-builds the shared graph once per package and
//! costs roughly 41 minutes. That is a release step, not a per-commit gate;
//! these tests are the per-commit half, and they read the same files the dry
//! run would.
//!
//! **They fail closed.** A `[workspace] members` glob, a member with no
//! `[package] name`, a dependency table holding something other than a table,
//! or a `publish` value that is neither absent nor `false` is a panic with an
//! explanation, never a skipped entry — the
//! `lib_rs_exports_nothing_the_documented_list_omits` precedent in
//! `public_surface.rs`. Both roster lists below are asserted to cover the
//! workspace exactly, so a new member is red until someone records which side
//! it is on; `every_crate_directory_is_a_declared_member` closes the step
//! before that, where a crate directory never reaches the members list at all;
//! and the edge test asserts the reverse inclusion, so a scrape that stopped
//! finding anything cannot pass vacuously.
//!
//! **Why `toml` and not a hand parser.** DCR-0018 kept the facade surface
//! scrape dependency-free, but the thing it was refusing there was an
//! *install-dependent* external tool that skips when absent; a dev-dependency
//! is neither absent nor skippable. What a hand parser would have to get right
//! here is dotted keys (`version.workspace = true`) and inline tables
//! (`{ workspace = true }`, `{ version = "…", path = "…" }`) in the same file —
//! exactly the shapes a naive line scan misreads, and a misread here fails
//! open. `toml` is already a `transync-core` dependency, so the cost is one
//! dev-dependency edge in `Cargo.lock`, not a new package in the tree.
//!
//! TRACE: R0001-0040
//! TRACE: ADR-0019
//! TRACE: release-checklist steps 17, 19

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use toml::Value;
use toml::value::Table;

/// Members that publish to crates.io, in the dependency order step 19 of
/// `docs/project/release-checklist.md` names for a first publication. Nothing
/// here depends on the order — it is written this way so the two documents
/// read alike.
const PUBLISHED_MEMBERS: &[&str] = &[
    // No internal edge at all (ti `e4f4b0`), so it can be published before
    // anything else — like `transync-html`, and unlike every member after it.
    "transync-lang",
    "transync-html",
    "transync-syntax",
    "transync-core",
    "transync",
    "transync-openai",
    "transync-anthropic",
    "transync-cli",
];

/// Members that must not publish. ADR-0019: `transync-wasm` is a build target
/// for the browser demo, not a library anyone depends on. A member belongs on
/// this list only by decision, which is the entire point — a crate that
/// publishes because someone chose it and a crate that publishes because
/// nobody said otherwise are the same manifest, and this list is where the
/// difference is written down.
const PRIVATE_MEMBERS: &[&str] = &["transync-wasm"];

/// The dependency tables a member manifest may declare an internal edge in.
/// `[target.…]` variants nest these one level deeper and are walked too.
const DEPENDENCY_TABLES: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];

/// Keys that must never sit beside `workspace = true` on an internal edge:
/// each one re-introduces a source the root table no longer controls.
const FORBIDDEN_EDGE_KEYS: &[&str] = &["path", "version", "git", "registry"];

/// Repo root, derived from this crate's manifest dir (`crates/transync`
/// -> `../..`), the way `docs_index_drift.rs` derives it.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR/../..")
}

fn read_manifest(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} should be readable: {e}", path.display()));
    toml::from_str(&text).unwrap_or_else(|e| panic!("{} should be valid TOML: {e}", path.display()))
}

fn root_manifest() -> Value {
    read_manifest(&repo_root().join("Cargo.toml"))
}

/// A `Value` that must be a table, or a panic naming where it was found.
fn expect_table<'a>(value: &'a Value, what: &str) -> &'a Table {
    value
        .as_table()
        .unwrap_or_else(|| panic!("`{what}` should be a table, found `{value}`"))
}

/// One workspace member, as `[workspace] members` lists it and as its own
/// manifest names it.
struct Member {
    /// Path relative to the repo root, verbatim from `[workspace] members` —
    /// the string the root `[workspace.dependencies]` `path` key must match.
    dir: String,
    /// `[package] name`: what cargo and the registry know it by.
    name: String,
    manifest: PathBuf,
    doc: Value,
}

/// Every workspace member, read from disk. Panics rather than skipping on
/// anything it cannot enumerate.
fn members() -> Vec<Member> {
    let root = repo_root();
    let doc = root_manifest();
    let listed = doc
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(Value::as_array)
        .expect("root Cargo.toml should declare `[workspace] members` as an array");
    assert!(
        !listed.is_empty(),
        "root Cargo.toml declares an empty `[workspace] members` — every \
         assertion in this file would pass vacuously"
    );

    listed
        .iter()
        .map(|entry| {
            let dir = entry
                .as_str()
                .unwrap_or_else(|| panic!("`[workspace] members` entry is not a string: {entry}"));
            assert!(
                !dir.contains('*') && !dir.contains('?'),
                "`[workspace] members` entry `{dir}` is a glob. This file \
                 enumerates members one at a time so that one it cannot \
                 classify is a failure rather than a silent skip; teach it to \
                 expand globs before introducing one."
            );
            let manifest = root.join(dir).join("Cargo.toml");
            let doc = read_manifest(&manifest);
            let name = doc
                .get("package")
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("{} declares no `[package] name`", manifest.display()))
                .to_string();
            Member {
                dir: dir.to_string(),
                name,
                manifest,
                doc,
            }
        })
        .collect()
}

/// The root `[workspace.dependencies]` table.
fn workspace_dependencies(root: &Value) -> &Table {
    let value = root
        .get("workspace")
        .and_then(|w| w.get("dependencies"))
        .expect("root Cargo.toml should declare a `[workspace.dependencies]` table");
    expect_table(value, "workspace.dependencies")
}

/// Every dependency table in a member manifest, paired with the label to name
/// it by in a failure message. Panics on a `[target.…]` shape it cannot read.
fn dependency_tables(member: &Member) -> Vec<(String, &Table)> {
    let mut out = Vec::new();
    for name in DEPENDENCY_TABLES {
        if let Some(value) = member.doc.get(*name) {
            out.push(((*name).to_string(), expect_table(value, name)));
        }
    }
    if let Some(target) = member.doc.get("target") {
        for (cfg, spec) in expect_table(target, "target") {
            let spec = expect_table(spec, &format!("target.{cfg}"));
            for name in DEPENDENCY_TABLES {
                if let Some(value) = spec.get(*name) {
                    let label = format!("target.{cfg}.{name}");
                    let table = expect_table(value, &label);
                    out.push((label, table));
                }
            }
        }
    }
    out
}

/// The package a dependency entry resolves to: its `package` key when the edge
/// is renamed, otherwise the key itself.
fn dependency_target<'a>(key: &'a str, spec: &'a Value) -> &'a str {
    spec.get("package").and_then(Value::as_str).unwrap_or(key)
}

/// Half one of the bump obligation: the requirement the registry will see names
/// the version the member actually is. Half two: the member's version really is
/// the workspace's, so there is one number to bump and the requirement tracks
/// it.
#[test]
fn every_internal_requirement_equals_the_workspace_version() {
    let root = root_manifest();
    let workspace_version = root
        .get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(Value::as_str)
        .expect("root Cargo.toml should declare `[workspace.package] version`");

    let members = members();
    let by_name: BTreeMap<&str, &Member> = members.iter().map(|m| (m.name.as_str(), m)).collect();

    let mut checked = BTreeSet::new();
    for (name, spec) in workspace_dependencies(&root) {
        let Some(member) = by_name.get(name.as_str()) else {
            continue; // third-party; its requirement is nobody's to shadow
        };
        let spec = expect_table(spec, &format!("workspace.dependencies.{name}"));

        let version = spec
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or_else(|| {
                panic!(
                    "`[workspace.dependencies] {name}` carries no `version`. A \
                     path dependency with no version requirement cannot be \
                     published at all (R0001-0040): packaging strips `path` and \
                     resolves the requirement from the registry."
                )
            });
        assert_eq!(
            version, workspace_version,
            "`[workspace.dependencies] {name}` requires `{version}` but \
             `[workspace.package] version` is `{workspace_version}`. That key \
             does not inherit — release-checklist step 17 says both move in one \
             edit. A patch bump leaves `cargo build --workspace` green here \
             (`^{version}` still accepts `{workspace_version}`) and publishes a \
             manifest understating what the sibling needs."
        );

        let path = spec.get("path").and_then(Value::as_str).unwrap_or_else(|| {
            panic!(
                "`[workspace.dependencies] {name}` carries no `path`. Without \
                 it the workspace resolves the sibling from the registry \
                 instead of from this checkout."
            )
        });
        // Resolved, not compared as text: `./crates/x` and `crates/x` are the
        // same directory, and only a genuinely different one should be red.
        let root = repo_root();
        let pointed_at = root.join(path).canonicalize().unwrap_or_else(|e| {
            panic!(
                "`[workspace.dependencies] {name}` points at `{path}`, which \
                 does not resolve under the repo root: {e}"
            )
        });
        assert_eq!(
            pointed_at,
            root.join(&member.dir)
                .canonicalize()
                .expect("a member directory read earlier should still resolve"),
            "`[workspace.dependencies] {name}` points at `{path}`, but the \
             member is listed at `{}`",
            member.dir
        );

        checked.insert(name.clone());
    }

    assert!(
        !checked.is_empty(),
        "no `[workspace.dependencies]` entry names a workspace member — either \
         the internal requirements were removed (they are what makes this \
         workspace publishable) or this test is reading the wrong table"
    );

    // Half two. Without it the requirement above tracks a number no member
    // publishes: a member pinning its own `version = "0.2.1"` against a
    // `^0.2.0` requirement resolves fine and ships the same understatement.
    let pinned: Vec<String> = members
        .iter()
        .filter(|m| {
            m.doc
                .get("package")
                .and_then(|p| p.get("version"))
                .and_then(|v| v.get("workspace"))
                .and_then(Value::as_bool)
                != Some(true)
        })
        .map(|m| m.manifest.display().to_string())
        .collect();
    assert!(
        pinned.is_empty(),
        "these members do not declare `version.workspace = true`, so their \
         version is no longer the one `[workspace.package]` bumps and the \
         requirements checked above track a number nothing publishes:\n  {}",
        pinned.join("\n  ")
    );
}

/// Every edge between two members goes through the root table, so the version
/// requirement checked above is the only place the edge is written.
#[test]
fn every_internal_edge_is_declared_through_the_workspace() {
    let root = root_manifest();
    let members = members();
    let names: BTreeSet<&str> = members.iter().map(|m| m.name.as_str()).collect();

    let mut reached: BTreeSet<String> = BTreeSet::new();
    for member in &members {
        for (label, table) in dependency_tables(member) {
            for (key, spec) in table {
                let target = dependency_target(key, spec);
                if !names.contains(target) {
                    continue;
                }
                let spec = expect_table(spec, &format!("{label}.{key}"));
                assert_eq!(
                    spec.get("workspace").and_then(Value::as_bool),
                    Some(true),
                    "{} declares internal dependency `{key}` under `[{label}]` \
                     without `workspace = true`. Internal edges are declared \
                     once, in the root `[workspace.dependencies]` table, so \
                     that the registry version requirement lives beside the \
                     `[workspace.package]` version it has to track.",
                    member.manifest.display()
                );
                for forbidden in FORBIDDEN_EDGE_KEYS {
                    assert!(
                        !spec.contains_key(*forbidden),
                        "{} declares internal dependency `{key}` under \
                         `[{label}]` with both `workspace = true` and \
                         `{forbidden}`. The second key overrides what the root \
                         table controls; drop it and edit the root entry.",
                        member.manifest.display()
                    );
                }
                reached.insert(target.to_string());
            }
        }
    }

    // The reverse inclusion, which is also the anti-vacuity guard: a root entry
    // naming a member nothing depends on is a requirement that will never be
    // exercised, and a scrape that found no edges at all fails here loudly
    // instead of passing on an empty set.
    let declared: BTreeSet<String> = workspace_dependencies(&root)
        .keys()
        .filter(|k| names.contains(k.as_str()))
        .cloned()
        .collect();
    assert_eq!(
        reached,
        declared,
        "the internal edges the members actually declare and the internal \
         entries in `[workspace.dependencies]` disagree.\n  \
         declared but unused: {:?}\n  used but undeclared: {:?}",
        declared.difference(&reached).collect::<Vec<_>>(),
        reached.difference(&declared).collect::<Vec<_>>(),
    );
}

/// The roster tests below classify `[workspace] members`, so a crate that
/// never made that list is invisible to them. Cargo will not say so either: a
/// directory under `crates/` with its own manifest and no `members` entry is
/// simply not built by `cargo build --workspace`, and only errors if something
/// reaches into it directly. This is the check that makes the roster total
/// against the filesystem rather than against the list.
#[test]
fn every_crate_directory_is_a_declared_member() {
    let crates_dir = repo_root().join("crates");
    let declared: BTreeSet<String> = members().into_iter().map(|m| m.dir).collect();

    let mut orphans = Vec::new();
    for entry in std::fs::read_dir(&crates_dir).expect("crates/ should be a directory") {
        let entry = entry.expect("readable crates/ entry");
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        if !entry.path().join("Cargo.toml").is_file() {
            continue; // not a crate, so not a member question
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let dir = format!("crates/{name}");
        if !declared.contains(&dir) {
            orphans.push(dir);
        }
    }
    orphans.sort();

    assert!(
        orphans.is_empty(),
        "these directories carry a Cargo.toml but are not in `[workspace] \
         members`, so nothing builds, lints, or classifies them — the \
         publication roster below cannot see them at all:\n  {}",
        orphans.join("\n  ")
    );
}

/// Which members publish is a decision, and the decision is recorded twice —
/// here and in release-checklist step 19. A member on neither list is red.
#[test]
fn the_publication_roster_is_exactly_the_recorded_one() {
    let published_roster: BTreeSet<String> =
        PUBLISHED_MEMBERS.iter().map(|s| (*s).to_string()).collect();
    let private_roster: BTreeSet<String> =
        PRIVATE_MEMBERS.iter().map(|s| (*s).to_string()).collect();
    let double_booked: Vec<_> = published_roster.intersection(&private_roster).collect();
    assert!(
        double_booked.is_empty(),
        "a member is on both roster lists: {double_booked:?}"
    );

    let members = members();
    let workspace: BTreeSet<String> = members.iter().map(|m| m.name.clone()).collect();
    let roster: BTreeSet<String> = published_roster.union(&private_roster).cloned().collect();
    assert_eq!(
        roster,
        workspace,
        "the roster in this file and the workspace members disagree.\n  \
         in the workspace, on no list: {:?}\n  on a list, not a member: {:?}\n\
         A new member publishes to crates.io unless its manifest says \
         `publish = false`, so joining the roster is a decision: record it in \
         `docs/project/release-checklist.md` step 19 first, and in \
         PUBLISHED_MEMBERS / PRIVATE_MEMBERS here second.",
        workspace.difference(&roster).collect::<Vec<_>>(),
        roster.difference(&workspace).collect::<Vec<_>>(),
    );

    let mut published = BTreeSet::new();
    let mut private = BTreeSet::new();
    for member in &members {
        match member.doc.get("package").and_then(|p| p.get("publish")) {
            None => {
                published.insert(member.name.clone());
            }
            Some(value) if value.as_bool() == Some(false) => {
                private.insert(member.name.clone());
            }
            Some(other) => panic!(
                "{} declares `publish = {other}`, which this test will not \
                 classify. The roster is two-valued on purpose: the key is \
                 absent (publishes to crates.io) or it is `false` (private, \
                 ADR-0019). A registry allow-list, or an explicit \
                 `publish = true`, is a change to what a release does and \
                 belongs in `docs/project/release-checklist.md` step 19 before \
                 it belongs here.",
                member.manifest.display()
            ),
        }
    }

    assert_eq!(
        private, private_roster,
        "the members carrying `publish = false` are not the recorded private \
         set. `cargo publish --dry-run --workspace` skips exactly these; \
         anything else either ships a crate nobody decided to ship or holds \
         one back that step 19 promises."
    );
    assert_eq!(
        published, published_roster,
        "the members that publish are not the recorded published set"
    );
}
