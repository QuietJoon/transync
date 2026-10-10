---
type: ADR
title: Cargo workspace with provider crates as separate workspace members
description: All crates live in one workspace under `crates/` with an explicit members list, so a clean checkout builds with one `cargo build` and trait/impl wire compatibility is enforced at compile time.
tags: [decision, ADR-0003]
status: active
---

# ADR 0003: Cargo workspace with provider crates as separate workspace members

## Context and Problem Statement

Given ADR-0002 (HTTP-free core + provider crates), we must decide how the crates are organized: single workspace, multiple workspaces, or some hybrid? And whether provider crates ship in the same git repository or live in separate repos.

## Decision Drivers

- The core, the CLI, and the default provider must build with one `cargo build` from a fresh checkout — the user explicitly committed to "clean-checkout-local" as the local run target.
- Wire compatibility between the core trait and the default provider must be enforced at compile time: a breaking change in `transync` must immediately break `transync-openai` so it cannot drift silently.
- Adding a future provider (`transync-anthropic`, `transync-local-llama`) must not require a refactor of the workspace topology.
- `CARGO_TARGET_DIR` must not be overridden anywhere in this repository (global rule).

## Considered Options

1. **Single Cargo workspace, all crates in `crates/`.** Members: `transync`, `transync-cli`, `transync-openai`. Future providers join as new members.
2. **Single workspace + provider crates in separate repos.** Core + CLI here; `transync-openai` lives in its own git repo.
3. **One fat crate.** Everything in `transync`. CLI is a `[[bin]]` target; OpenAI integration is a Cargo feature.

## Decision Outcome

We chose **option 1**. The user confirmed "Cargo workspace and multi-crate structure" during brainstorming Q3.

Workspace shape — the layers, not the roster. The root `Cargo.toml` `members` list is the canonical record of which crates are members:

```
transync/
├── Cargo.toml             # [workspace] members (explicit list) — the canonical member set
├── crates/
│   ├── transync-html/     # HTML mechanics, lifted out of transync-syntax's htmlseg; no workspace dependency
│   ├── transync-syntax/   # wasm32-compilable base: parser, IR + IDs, regen, render, align
│   ├── transync-core/     # pipeline on top: unit, batch, llm, validate, cache, profile, pipeline
│   ├── transync/          # public facade: re-exports transync-core + transync-syntax
│   ├── transync-openai/   # a provider crate; each further provider is one more sibling member
│   ├── transync-cli/
│   ├── transync-lang/     # source-language gate; no workspace dependency
│   └── transync-wasm/     # browser (wasm-bindgen) surface over transync-syntax; publish = false
```

Which module lives in which crate is `docs/implementation/module-map.md`'s to say (*Crate / module layout*).

Future provider crates land at `crates/transync-anthropic/`, `crates/transync-local-llama/`, etc. `crates/transync-anthropic/` landed exactly there, as one more member and with no topology change (DCR-0029); whether `crates/transync-local-llama/` ever lands is an open decision (ti `3887f02e`).

The structural core lives in `crates/transync-core/` (DCR-0005); `crates/transync/` remains the public dependency target as a re-export facade (and will select a markdown/html dialect front-end once the dialect split lands — DCR-0005 defers the dialect abstraction until a concrete second dialect supplies real constraints, and whether one now has is the open decision ti `8e383504`). The integration-test suite stays in the facade crate. The decision drivers above (one-command build, compile-time wire compatibility, localized provider addition, no `CARGO_TARGET_DIR` override) hold unchanged for every member.

Status: Decided 2026-05-01 (brainstorming Q3), and in force. *History* lists the changes this record has recorded; the root `Cargo.toml` `members` list is the current member set.

### Implementation

- Workspace root `Cargo.toml` declares an explicit `members` list — not a `crates/*` glob — and a `[workspace.package]` block for shared metadata (license, edition, repository). That list is the canonical record of the member set, and the explicit-list rule is load-bearing twice over: for the `wasm32` gate, `transync-syntax` is listed by name, never via a `crates/*` glob; and for publication, `transync-wasm`'s `publish = false` makes the publication roster every other member, which a first publication has to walk in dependency order (`docs/project/release-checklist.md` step 19, *Which members publish*, owns the roster and its order).
- Each crate has its own `Cargo.toml`; dependencies declared in `[workspace.dependencies]` for shared versions (Comrak, serde, tokio).
- The dependency graph is a strict DAG. Each member's own `Cargo.toml` records its edges; the shape they keep is:
  - **The engine stack is layered.** `transync-html` is its base and depends on no workspace member; `transync-syntax → transync-html`; `transync-core → transync-syntax`; the facade `transync` sits on top. Each depends only on members beneath it.
  - **A provider crate depends on the facade.** `transync-openai → transync`, and `transync-anthropic → transync` is exactly the same edge.
  - **`transync-cli` sits on top**, depending on the facade and on the provider crates it can select, `transync-openai` among them. No workspace member depends on `transync-anthropic` (DCR-0029).
  - **`transync-wasm` depends on `transync-syntax` alone** among workspace crates, and the restriction is a decision rather than a coincidence: `transync-core`'s `wasm32` dependency tree reaches `getrandom` through tokio/tiktoken and `transync-syntax`'s does not, so a widening edge would break the link rather than merely the charter.
  - **`transync-lang` depends on no workspace member**, and that absence is load-bearing rather than incidental (ADR-0028): a second local answer to "what language is this" is precisely what the gate must not become. It is the first member that is neither an engine layer nor a provider, and it answers a pre-flight question for the caller rather than a question inside the pipeline.
- **The `wasm32` gate.** The standing check is `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`, run by the pre-commit hook and `scripts/smoke.sh`. The gate's command string must stay unchanged: it names `transync-syntax` and `transync-wasm`, and reaches `transync-html` through the first. Two manifests carry prohibitions for it:
  - `transync-syntax` deliberately declares **no `[features]`** — a feature there would reintroduce the feature-unification trap the crate split exists to avoid — and takes no `transync-core` dependency.
  - `transync-html` carries the same two prohibitions for the same reason — **no `[features]`**, and no dependency on a workspace member — because it sits under the `wasm32-unknown-unknown` gate transitively.
- **`transync-wasm` is `publish = false`, with `crate-type = ["cdylib"]`.** It is a build target for a demo, not a library anyone depends on, and nothing in the workspace consumes it. `cargo test`/`clippy --workspace` still sweep the member; the lib target is compiled into its own test harness regardless of crate-type.
- The `web/` directory and `docs/` live at the workspace root (outside `crates/`) so non-crate assets are discoverable without crate-relative path gymnastics.

## Consequences

- **Good:** One `cargo build --workspace` builds everything. One `cargo test --workspace` is the canonical test command.
- **Good:** Compile-time wire compatibility between trait and impl: adding a method to `Translator` immediately fails the OpenAI build until updated.
- **Good:** Adding a provider is a localized change.
- **Good:** Per-crate testing (`cargo test -p transync`) is fast — core has no I/O.
- **Bad:** One `Cargo.toml` per member instead of one — the root `members` list says how many. Acceptable: the `transync-syntax` and `transync-wasm` manifests are what make `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown` a compiler-enforced gate instead of a documented intention, and the browser surface a compiler-checked target instead of a script that happens to work.
- **Bad:** Provider crates must be versioned in lockstep with core releases. Acceptable; we will sort out semver discipline at the first 0.x → 0.y bump.

## History

One line per recorded change. The full text of each amendment is in [`archive/adr-amendments.md`](archive/adr-amendments.md), under *ADR-0003*.

- 2026-05-01 (brainstorming Q3): decided — one workspace under `crates/`, three members: `transync`, `transync-cli`, `transync-openai`.
- 2026-07-10 (DCR-0005): `transync-core` extracted; `crates/transync` became the re-export facade; members listed explicitly, never a glob.
- 2026-08-04 (DCR-0017 / OI-0028): `transync-syntax` joined as the wasm32-compilable base; new edge `transync-core → transync-syntax`.
- 2026-08-05 (ADR-0019 / DCR-0020): `transync-wasm` joined, the first member with `publish = false`; the wasm32 gate grew to name it.
- 2026-08-05 (ticket `92ac61b9`): `transync-wasm` dropped its `rlib` for `crate-type = ["cdylib"]`, worth 101,228 B of fat-LTO scope.
- 2026-08-10 (DCR-0029): `transync-anthropic` joined as the second provider, with exactly the edge `transync-openai` has.
- 2026-08-20 (DCR-0032): `transync-html` lifted out of `transync-syntax`'s `htmlseg` module; new edge `transync-syntax → transync-html`.
- 2026-09-02 (ADR-0028 / DCR-0045): `transync-lang`, the source-language gate, joined, depending on no workspace member.
