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

Workspace shape (as amended by DCR-0005, DCR-0017, and DCR-0020):

```
transync/
├── Cargo.toml             # [workspace] members (explicit list)
├── crates/
│   ├── transync-syntax/   # wasm32-compilable base: parser, IR + IDs, regen, render, align, htmlseg
│   ├── transync-core/     # pipeline on top: unit, batch, llm, validate, cache, profile, pipeline
│   ├── transync/          # public facade: re-exports transync-core + transync-syntax
│   ├── transync-cli/
│   ├── transync-openai/
│   └── transync-wasm/     # browser (wasm-bindgen) surface over transync-syntax; publish = false
```

Future provider crates land at `crates/transync-anthropic/`, `crates/transync-local-llama/`, etc.

Status: Decided 2026-05-01 (brainstorming Q3). Amended 2026-07-10 by
DCR-0005: the structural core was extracted to `crates/transync-core/`;
`crates/transync/` remains the public dependency target as a re-export
facade (and will select a markdown/html dialect front-end once the
dialect split lands). The integration-test suite stays in the facade
crate. The dependency DAG gains one edge: `transync → transync-core`;
the drivers above (one-command build, compile-time wire compatibility,
localized provider addition) are unchanged. Shipped: the four-member
workspace (`transync-core`, `transync`, `transync-openai`,
`transync-cli`) builds from a clean checkout with one
`cargo build --workspace`.

### Amendment 2026-08-04 (DCR-0017) — fifth member, and Core does have an internal dependency

*Appended, not a rewrite. The original text above stands as the record of
what was decided in 2026-05-01 and amended in 2026-07-10.*

The workspace is now **five members**. `crates/transync-syntax/` was
extracted from `transync-core` as the wasm32-compilable base crate (OI-0028
Option B): it owns `parser`, `id`, `regen`, `render`, `align`, `htmlseg`,
`outcome`, `walk`, and `error::ParseError`; `transync-core` keeps `pipeline`,
`unit`, `batch`, `llm`, `validate`, `cache`, `profile`, and
`error::TransyncError`. See the tree above.

Two statements in the *Implementation* section below are superseded by this
amendment:

- **"Core has no internal dependencies" is no longer true.** The DAG gains
  one edge: `transync-core → transync-syntax`. The full DAG is now
  `transync-cli → {transync, transync-openai}`, `transync-openai → transync`,
  `transync → {transync-core, transync-syntax}`,
  `transync-core → transync-syntax`, and `transync-syntax` is the sole
  dependency-free member. It is still a strict DAG.
- **"Three `Cargo.toml` files instead of one"** (the *Bad* tradeoff line) is
  now **five**. Still acceptable, and for a new reason: the extra manifest is
  what makes
  `cargo check -p transync-syntax --target wasm32-unknown-unknown` a
  compiler-enforced gate instead of a documented intention. `transync-syntax`
  deliberately declares **no `[features]`** — a feature there would
  reintroduce the feature-unification trap the crate split exists to avoid.

The explicit-`members`-list rule is unchanged and now load-bearing for the
gate: `transync-syntax` is listed by name, never via a `crates/*` glob. The
original decision drivers (one-command build, compile-time wire
compatibility, localized provider addition, no `CARGO_TARGET_DIR` override)
are all unchanged.

### Amendment 2026-08-05 (ADR-0019 / DCR-0020) — sixth member, and the first one that is not published

*Appended, not a rewrite. Everything above stands as written.*

The workspace is now **six members**. `crates/transync-wasm/` is the browser
(wasm-bindgen) surface over `transync-syntax` — Track C proper. It differs
from every existing member in three ways worth naming:

- **`publish = false`, and `crate-type = ["cdylib", "rlib"]`.** It is a build
  target for a demo, not a library anyone depends on. Nothing in the workspace
  consumes it. *(2026-08-05, later the same day — ticket `92ac61b9`: the `rlib`
  is gone, `crate-type = ["cdylib"]`. The reasoning above is why it could go —
  nothing consumed the artifact — and it was worth 101,228 B of fat-LTO scope.
  `cargo test`/`clippy --workspace` still sweep the member; the lib target is
  compiled into its own test harness regardless of crate-type.)*
- **It depends on `transync-syntax` alone** among workspace crates, and the
  restriction is a decision rather than a coincidence: `transync-core`'s
  `wasm32` dependency tree reaches `getrandom` through tokio/tiktoken and
  `transync-syntax`'s does not, so a widening edge would break the link rather
  than merely the charter. The DAG gains `transync-wasm → transync-syntax` and
  stays strict; `transync-syntax` remains the sole dependency-free member.
- **It extends the gate rather than adding one.** The standing check is now
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  in both pre-commit hook copies and `scripts/smoke.sh`. `transync-syntax`'s
  own manifest is untouched — still no `[features]`, still no core dependency.

The *Bad* tradeoff line below ("Three `Cargo.toml` files", amended to five in
2026-08-04) is now **six**, and the newest one pays for itself the same way the
fifth did: it is what makes the browser surface a compiler-checked target
instead of a script that happens to work.

### Implementation

- Workspace root `Cargo.toml` declares an explicit `members` list (`crates/transync-core`, `crates/transync`, `crates/transync-openai`, `crates/transync-cli`; `crates/transync-syntax` added 2026-08-04; `crates/transync-wasm` added 2026-08-05) — not a `crates/*` glob — and a `[workspace.package]` block for shared metadata (license, edition, repository).
- Each crate has its own `Cargo.toml`; dependencies declared in `[workspace.dependencies]` for shared versions (Comrak, serde, tokio).
- The dependency graph is a strict DAG: `transync-cli → {transync, transync-openai}`, `transync-openai → transync`. Core has no internal dependencies. *(Superseded 2026-08-04 by DCR-0017 — see the amendment above: `transync-core → transync-syntax`.)*
- The `web/` directory and `docs/` live at the workspace root (outside `crates/`) so non-crate assets are discoverable without crate-relative path gymnastics.

## Consequences

- **Good:** One `cargo build --workspace` builds everything. One `cargo test --workspace` is the canonical test command.
- **Good:** Compile-time wire compatibility between trait and impl: adding a method to `Translator` immediately fails the OpenAI build until updated.
- **Good:** Adding a provider is a localized change.
- **Good:** Per-crate testing (`cargo test -p transync`) is fast — core has no I/O.
- **Bad:** Three `Cargo.toml` files instead of one — **five** since 2026-08-04 (DCR-0017), **six** since 2026-08-05 (DCR-0020). Acceptable; see the amendments above for why the fifth and sixth pay for themselves.
- **Bad:** Provider crates must be versioned in lockstep with core releases. Acceptable; we will sort out semver discipline at the first 0.x → 0.y bump.
