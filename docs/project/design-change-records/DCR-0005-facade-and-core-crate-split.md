---
type: DCR
title: Facade crate + transync-core split
description: The structural core moved to crates/transync-core; crates/transync is now a re-export facade that will own dialect selection.
tags: [change, project-control, DCR-0005]
status: active
---

# DCR-0005: Facade crate + `transync-core` split

- **Date:** 2026-07-10
- **Source:** Review 0007, Issue R0007-0021 (gate session; the restructure itself is maintainer work-in-progress predating the review) (review archived and removed)
- **Affected ADRs:** docs/decisions/0003-cargo-workspace-with-provider-crates.md (updated)

## What Changed

ADR-0003's three-member workspace (`transync` as the core library) became a
four-member workspace:

- `crates/transync-core/` — the structural core (parser, block IR, id, unit,
  batch, llm, validate, regen, align, render, pipeline, profile, cache). All
  former `crates/transync/src` modules moved here.
- `crates/transync/` — the public facade. Today it is a single re-export
  (`pub use transync_core::*;`); once the planned dialect split lands it will
  select a front-end (markdown / html) per `TranslateOptions`. External
  consumers (transync-cli, transync-openai, resp-translator) keep depending on
  `transync`.
- The scenario/error-fixture integration-test suite stays in the facade crate
  (`crates/transync/tests/`), exercising the public surface.
- Workspace members are now listed explicitly (no `crates/*` glob).

## Why

Preparation for multi-dialect input (markdown / html) without breaking the
`transync` dependency target. Detailed rationale lives in ADR-0003's amended
Decision Outcome.

## Affected Areas

- `Cargo.toml` (workspace members), `crates/transync-core/`, `crates/transync/`
- Live docs were rewritten from `crates/transync/src` → `crates/transync-core/src`
  in this session (README, architecture docs, guides, stub manifest, module map).
  Historical records (design-baseline, intake, skeleton-plan, DCR-0001..0004)
  intentionally keep the old paths.

## Migration / Follow-up

- The planned `Dialect` trait and markdown/html front-end crates have not
  landed; manifests describe them in future tense.
- `scripts/smoke.sh`, `scripts/smoke-live.sh`, and `web/SMOKE.md` reference
  `crates/transync/tests/fixtures/...` — still correct (tests stayed in the
  facade); do not rewrite those paths.

## Note (2026-07-13, external architecture review)

*Appended, not a rewrite of the record above.*

**Steady-state purpose of the facade.** `crates/transync/`'s durable reason
to exist is a **semver / API firewall** for consumers: it pins the public
surface (`transync::…`) so internal refactoring of `transync-core` (module
moves, type reshaping, the very split this DCR records) never ripples out to
`transync-cli`, `transync-openai`, or external consumers such as
`resp-translator`. That firewall role stands on its own — it is the reason to
keep the facade even if no second dialect ever lands.

**Dialect abstraction is deferred (YAGNI).** The `Dialect` trait and
markdown/html front-end crates named in *Migration / Follow-up* (and in
ADR-0003's amended Decision Outcome) must **not** be designed speculatively.
With a single dialect (GFM) a `Dialect` trait would have exactly one
implementation, which encodes guesses rather than requirements and risks
baking GFM assumptions into the abstraction. Defer the trait until a concrete
second dialect (e.g. raw-HTML or MDX input) supplies real constraints to
design against; until then the facade earns its keep purely as the API
firewall described above.

## Note (2026-08-04, DCR-0017 — `transync-syntax` split)

*Appended, not a rewrite of the record above.*

**The core-contents enumeration in *What Changed* is superseded.**
`crates/transync-core/` no longer holds "parser, block IR, id, unit, batch,
llm, validate, regen, align, render, pipeline, profile, cache". A fifth
member, **`crates/transync-syntax/`**, took the syntax half — `parser`, `id`,
`regen`, `render`, `align`, `htmlseg`, `outcome`, `walk`,
`error::ParseError` — so that it compiles for `wasm32-unknown-unknown` behind
a standing gate (OI-0028 Option B). `transync-core` keeps `unit`, `batch`,
`llm`, `validate`, `cache`, `profile`, `pipeline`, `error::TransyncError`. The
authoritative enumeration is **DCR-0017**; ADR-0003's amended tree is the
topology of record.

**Everything this record actually decided is unchanged.** The facade is still
the semver / API firewall, and this split is the exact scenario the firewall
was kept for: the syntax modules changed crates and *no consumer path moved*,
because `transync-core` re-exports the moved modules and `crates/transync`
re-exports core. The facade additionally gained `pub use transync_syntax;` so
the base crate can be named directly. The integration-test suite still lives
in `crates/transync/tests/`; `scripts/smoke.sh`, `scripts/smoke-live.sh`, and
`web/SMOKE.md` still point at `crates/transync/tests/fixtures/...` — still
correct, still do not rewrite them.

**The dialect deferral above is UNCHANGED and was deliberately not amended.**
Its release condition — a concrete second dialect supplying real constraints
— is **still unmet** (GFM remains the only dialect), so no `Dialect` trait
was instantiated. `transync-syntax` is the natural landing zone for that seam
when the condition is finally met; being the landing zone is not the same as
being occupied. Manifests still describe the front-end crates in future
tense, on purpose.

## Note (2026-08-04, DCR-0018 — the firewall is now curated)

*Appended, not a rewrite of the record above.*

**The firewall this record created is no longer a glob.** `crates/transync`
was `pub use transync_core::*;` (and, after DCR-0017, `pub use
transync_syntax;`) over an all-`pub` core, so every item an engine crate made
public became public API by accident and item-level breakage passed straight
through. The OI-0027 curation replaced both lines with an **explicit
re-export list**, dropped the facade's `transync-syntax` dependency, and
demoted `transync-core`'s engine modules (`batch`, `error`, `pipeline`,
`validate`, and the five re-exported syntax aliases) to `pub(crate)` — so the
tier boundary is a **compiler** fact, not a documented intention. The
supported surface is enumerated in `contracts.md` §0 and welded to the code
by `crates/transync/tests/public_surface.rs`, which fails whenever §0, the
test's `DOCUMENTED` mirror, or the test's `first_class` compile assertion
drifts from either of the other two. The one direction it does not catch is a
`pub use` added to the facade's `lib.rs` alone — nothing enumerates the
facade's exports mechanically yet (a rustdoc-JSON diff would); that stays
human-enforced by the three-place editing obligation and a pre-release
read-through.

**The decision this record made is unchanged and is exactly what made the
curation cheap.** A facade whose whole job is to be the semver / API firewall
is the right place to narrow an aperture: engine modules moved crates
(DCR-0017) and then went private (DCR-0018) without a second front-end crate
or a dialect trait being involved. Consumers that genuinely need an engine
internal now depend on `transync-core` / `transync-syntax` **directly** and
accept their weaker stability promise — the one in-tree example is
`transync-openai`'s offline live-smoke fixture test, which declares the reach
as dev-dependencies. The authoritative record is **DCR-0018**.

**The dialect deferral above is STILL unchanged and still unamended.** GFM
remains the only dialect; no `Dialect` trait was instantiated by this wave
either. The manifests still describe the front-end crates in future tense, on
purpose.
