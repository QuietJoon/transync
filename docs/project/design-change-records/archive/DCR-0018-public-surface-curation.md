---
type: DCR
title: Public-surface curation — explicit facade, compiler-enforced tier boundary, non_exhaustive
description: crates/transync stops re-exporting transync-core wholesale. The facade becomes an explicit 78-item list mirrored by contracts.md §0 and welded to it by a bidirectional drift gate; transync-core's engine modules go pub(crate) so the boundary is compiler-enforced rather than documented. Three error enums, five option/profile structs, and TranslationUnit become #[non_exhaustive] (TranslationUnit gains new + with_* instead of Default), the three DCR-0017 narrowing candidates go pub(crate), and cargo doc is warning-free under a standing RUSTDOCFLAGS="-D warnings" gate. Resolves OI-0027.
tags: [change, project-control, DCR-0018]
status: deprecated
---

# DCR-0018: Public-surface curation (the 0.2.0 window-closing ritual)

- **Date:** 2026-08-04
- **Source:** OI-0027 public-surface hardening wave, implementing the owner-approved design spec `docs/superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md` against the 13-task plan `docs/superpowers/plans/2026-08-04-oi0027-public-surface-hardening.md` (amended in flight — see *Plan amendments* below). No new ADR: the decision this executes is OI-0027's 2026-08-03 owner decision (the three-tier boundary), with two owner corrections recorded 2026-08-04.
- **Resolves:** **OI-0027** (public-surface hardening for the next breaking window). This is the last item on the road to **0.2.0**.
- **Affected ADRs:** `docs/decisions/0002-http-free-core-with-translator-trait.md` and `docs/decisions/0003-cargo-workspace-with-provider-crates.md` (both **upheld, not amended** — the facade-as-firewall decision is what this record *implements*; nothing about the decision changed, only the firewall's aperture). `docs/decisions/0018-html-content-translation-via-segment-extraction.md` gained a second dated path note (the `transync::unit::html_outcomes` path it cited is now engine-internal). **DCR-0005** gained a dated note: the firewall it created is now *curated*.

## What Changed

### Part A — The facade becomes an explicit list, and the boundary becomes compiler-enforced

Before this wave `crates/transync/src/lib.rs` was two glob-shaped lines —
`pub use transync_core::*;` plus `pub use transync_syntax;` — over an
all-`pub` core. Item-level breakage in an engine crate leaked straight
through the semver firewall the facade exists to be.

- **The facade is now an explicit re-export list** (commit `ccdbd76`),
  grouped by cluster: the three curated modules (`cache`, `llm` including
  `llm::prompt`, `profile`), the entry points and their option/output
  types, the errors, the cache cluster, the Translator-contract cluster,
  the profile types, the alignment-map wire types, block identity, and the
  validation-report family. `pub use transync_syntax;` is **gone**, and so
  is the facade's `transync-syntax` dependency itself (commit `b67f395`) —
  the last reference disappeared when the syntax module aliases went
  `pub(crate)`, and both the workspace build and the new compile-assertion
  test prove it unused.
- **`transync-core`'s tier-(c) modules went `pub(crate)`** in the same
  commit: `batch`, `error`, `pipeline`, `validate` as `pub(crate) mod`, and
  the five re-exported syntax module aliases (`align`, `id`, `parser`,
  `regen`, `render`) as `pub(crate) use`. `cache`, `llm`, and `profile`
  stay `pub` — they are tier (a)/(b). The feature-gated `test_stub` stays
  `pub` behind its feature.
- **One deliberate exception: `unit`.** It is `#[doc(hidden)] pub mod`, not
  `pub(crate)`, because `transync-openai`'s `live_smoke.rs` reaches
  `unit::{build_batches, html_outcomes}` cross-crate to pin the live
  fixture's block/unit shape offline. That reach is declared honestly
  rather than hidden: `transync-openai` gained `transync-core` and
  `transync-syntax` as **dev-dependencies** (commit `37308ad`), and the
  test now names `transync_syntax::parser::parse` and
  `transync_core::unit::…` directly instead of borrowing the facade's
  re-export. `#[doc(hidden)]` plus the module comment say what the
  `pub` is for; it is not curated API.
- **This is the point of the whole part:** the tier boundary is now a
  *compiler* fact. A future `pub use` that widens the surface has to be
  written deliberately in `crates/transync/src/lib.rs`, and a core module
  that wants to become API has to stop being `pub(crate)` first.

#### Closure fallout — dead code the public re-exports had been keeping alive

Demoting the modules made the compiler see code that nothing reached.
Resolved in the same commit with **zero `#[allow]` additions and zero test
removals**:

- **`pipeline::retry`'s three no-op hooks — `retry_validation`,
  `oversize_split`, `fallback_to_source` — are deleted**, following the
  R0006-0053 precedent already set in that file for `provider_retry`. The
  deferral rationale survives as a comment so the names stay greppable, and
  **OI-0008 still tracks the retry/fallback redesign** (R0001-0057): the
  policy debt is unchanged, only the empty hooks are gone.
- **`batch::group_by_unit_count` is deleted together with its pre-existing
  `#[allow(dead_code)]`.** It had no caller anywhere in the workspace, and
  its doc offered it "for callers that still want a pure unit-count
  chunker" — an offer `pub(crate)` makes impossible. The demotion turned a
  decorative allow into load-bearing suppression of dead code with a false
  doc, so the allow went with the function. (It was removed during fix
  round 1, after the first review pass.)
- **`ValidatedBatch::batch_id_str` is dropped** — nothing ever read it.
- **`batch::estimate_unit_tokens` and `validate::schema::check_schema`
  became `#[cfg(test)]`.** Their only callers are this crate's own tests,
  and `check_schema`'s byte-compat oracle test is precisely why the
  function is kept rather than deleted.

### Part B — Three narrowings and zero rustdoc warnings

Commit `f4f069d` closed DCR-0017's handover list.

- **The three narrowing candidates went `pub(crate)`**, each confirmed to
  have no cross-crate consumer once the syntax split had landed:
  `htmlseg::balance_fragment` (sole consumer `render`, same crate),
  `outcome::is_translatable` (sole consumer `outcome` itself — its
  `#[doc(hidden)]` and its "pub only because the crate boundary forces it"
  comment went with it; `is_translatable_block`, which core's `unit` and
  `pipeline` do call, stays `pub`), and `walk::label_for` (sole consumer
  `normalize_top_level`, same crate).
- **`cargo doc --no-deps` is warning-free for all three library crates.**
  **Fourteen** intra-doc links to now-private items became plain backtick
  code text, per the `parser/refdefs` precedent DCR-0017 set: six
  pre-existing in `transync-syntax` (the five `htmlseg` links and the
  renderer's `node_inner_html` link that DCR-0017 handed forward), one
  created by the `label_for` narrowing above, and seven in `transync-core`
  — six of those newly exposed by Part A's closure. Prose is untouched;
  the cross-references still read the same, they just stop promising a
  hyperlink rustdoc cannot make.

### Part C — `OutputBudgetWarning` moves, and four root aliases appear

- **`OutputBudgetWarning` moved from `batch` to `validate`** (commit
  `8ecf6e3`), where `ValidationReport` — the only thing that carries it —
  already lives. It is a report type, not a batching type, and leaving it
  in a module that was about to go `pub(crate)` would have made a curated
  item's home private. Type identity, serde shape, and every field are
  unchanged.
- **Root aliases so every curated item has a flat path** (commit
  `cd04507`, plus `OutputBudgetWarning` from `8ecf6e3`):
  `ParseError`, `GlossaryScope`, `ALIGNMENT_SCHEMA_VERSION`, and
  `OutputBudgetWarning` are re-exported at `transync_core`'s root. Their
  old module paths (`error::ParseError`, `llm::GlossaryScope`,
  `align::ALIGNMENT_SCHEMA_VERSION`, `batch::OutputBudgetWarning`) were the
  only way to name them, and three of those four modules were about to
  become unreachable. The in-tree users of the module paths migrated in the
  same commit: `transync-openai`'s client, the CLI's `sync_js_drift` test,
  the facade's `mock_translator` helper, and `scn_10`.

### Part D — `#[non_exhaustive]`, and how construction works now

Commit `7f62cb9` (see *Plan amendments*: the attributes and the
construction-site sweep had to land atomically) added the attribute to
**nine** items, taking the workspace from 2 to **11**:

| item | kind | construction / matching rule |
|---|---|---|
| `TranslatorError` | enum | match with a wildcard arm; variant additions are non-breaking |
| `TransyncError` | enum | same; every variant still carries a stable machine code |
| `ParseError` | enum | same |
| `TranslateOptions` | struct | `let mut o = TranslateOptions::default(); o.field = …;` |
| `ProfileMetadata` | struct | default-then-assign, or deserialize |
| `ProfileConstraints` | struct | default-then-assign, or deserialize |
| `ProfileBatching` | struct | default-then-assign, or deserialize |
| `ProfileRender` | struct | default-then-assign, or deserialize |
| `TranslationUnit` | struct | `TranslationUnit::new(…)` + `with_*` (commit `5bdd2ff`) |

The two pre-existing `#[non_exhaustive]` items — `TokenizerHint` and
`CacheError` — are unchanged.

- **`TranslationUnit::new(unit_id, block_kind, input_mode, source_payload,
  source_hash)`** takes the five fields that are required by essence and
  defaults the rest: `context`/`constraints` to their `Default`s,
  `batch_id` to the `BatchId::new(0)` placeholder the batcher overwrites
  per chunk, and `retry` to `None` per the first-dispatch contract.
  `with_context` / `with_constraints` / `with_batch_id` / `with_retry`
  cover the rest. **`source_hash` is positional on purpose:** it is a
  `CacheKey` axis with no meaningful zero, and a defaulted hash would
  silently alias cache entries. `Default` is deliberately *not*
  implemented — the required fields are the type's essence.
- **Exhaustive by policy, stated in contracts.md §1 and unchanged here:**
  `UnitResult` and `TranslationBatchResult`, because downstream translators
  *produce* them (marking them `#[non_exhaustive]` would break every
  provider and mock impl); `TranslationBatch`, produced by core only — a
  policy statement, not an enforced one; and `GlossaryEntry`, whose serde
  shape is third-party wire format (embedded in consumers' public types and
  in operator-edited profile TOML), so any new field must be
  `#[serde(default)]`.

### Part E — The migration list

Every intentional break, and what absorbed it.

- **`transync::pipeline::run_pipeline` is no longer reachable.**
  `translate_with_cache(source, opts, translator, cache)` is its curated
  replacement and was promoted to the facade per the owner decision.
  `scn_10` — the partial-resume / abort-keeps-progress suite, the only
  in-tree caller of the module path — moved onto it (commit `2a54a84`),
  preserving DCR-0005's "verify the public surface through the facade"
  value. **`translate_with_cache` is not a bare alias:** it preflights
  `opts.target_language` and returns `TransyncError::Internal` on an empty
  or whitespace-only value before dispatching, where `run_pipeline` would
  have proceeded.
- **Functional-record-update construction is gone.** `..Default::default()`
  is illegal cross-crate on a `#[non_exhaustive]` struct, so every external
  construction site became default-then-assign. In-tree that is the CLI's
  `translate_cmd`, `transync-openai`'s `live_smoke`, and the facade's
  scenario/boundary/error/reader-honesty test suites; the scenario suites
  share the new `tests/common::opts_for(target_language)` helper (everything
  default except the required target language), so a future field addition
  touches one function rather than fifteen files.
- **Module-path users migrated to root aliases** — see Part C.
- **`transync-openai`'s `live_smoke` reaches the engine crates directly**
  through dev-dependencies — see Part A.
- **The sibling repo migrated the same day.**
  `/Volumes/Common/QJoon/resp-translator` commit **`8b6dbd5`** (branch
  `main`, "migrate: transync curated surface — translate_with_cache +
  default-then-assign construction"): `copy-transfer-mcp`'s translation
  path moved from `transync::pipeline::run_pipeline` to
  `transync::translate_with_cache`, and `profile_bridge`'s two FRU
  constructors became default-then-assign. **Behavioral delta, recorded
  there and here:** an empty profile `target_language` now fails fast with
  `stable_code` `"internal"` instead of proceeding (every shipped profile
  sets it, so no live path changes). A second, smaller win came with it:
  the default-then-assign rewrite of `to_transync_profile` **deleted** the
  hardcoded `auto_glossary: None` / `render: ProfileRender::default()`
  lines, so those fields now flow from transync's own default profile
  instead of being restated downstream. That migration was not deferred
  behind the transync wave because resp-translator's own pre-commit hook
  runs `clippy -D warnings`, and the lint suppression this wave needed is
  cross-crate-`non_exhaustive`-only (see *Plan amendments*).

### Part F — contracts.md §0 and the bidirectional drift gate

Commit `b67f395` welded the documentation and the code together so neither
can rot alone.

- **`contracts.md` gained `## 0. Public Rust surface`** — a **78-row**
  table (`path` / `kind` / `tier`) that **is** the supported surface, plus
  the tier (a) / tier (b) split, an explicit statement that tier (c)
  engine internals are *deliberately absent* (that absence is the firewall),
  and the note that the feature-gated `transync::test_stub` sits outside the
  gate because a feature-gated item is not part of the default surface.
- **`crates/transync/tests/public_surface.rs` has six parts:**
  1. a `first_class` module that re-exports **every** §0 row as a real
     `pub use` — a row naming something the facade does not export is a
     **compile error**;
  2. a `DOCUMENTED` constant mirroring the same row set as strings;
  3. `surface_table_matches_the_documented_list`, which scrapes §0 out of
     `contracts.md` and diffs the two sets **in both directions** — a name
     added to `DOCUMENTED` but not to the table fails, and a row deleted or
     renamed in the table fails *naming the row*;
  4. `first_class_module_matches_the_documented_list`, which scrapes the
     `first_class` module out of the test's **own source file** (the module
     is a compile assertion, so nothing can enumerate it at runtime) and
     diffs it against `DOCUMENTED` the same way — closing the third edge, so
     the compile assertion cannot quietly cover a different set than the two
     lists do. Both scrape-diffs also assert row/entry counts against their
     deduplicated sets, so a copy-pasted duplicate cannot mask a missing one;
  5. `hidden_modules_are_not_documented_as_surface`, a negative test that
     keeps `pipeline` / `parser` / `unit` / `batch` / `validate` / `regen` /
     `render` / `transync_syntax` / `htmlseg` / `outcome` / `walk` — plus
     `id` / `align` / `error`, the three `pub(crate)` syntax aliases whose
     *old* facade paths were themselves documented — out of §0 **by path
     segment**; segment matching, not substring, so
     `profile::render_prompt_body` passes while a `render::…` path cannot;
  6. `generator_version_matches_the_facade_crate`, which pins DCR-0017's
     **M6** constraint: `AlignmentMap::default().generator.version` must
     equal the facade crate's own version, which `version.workspace = true`
     guarantees. M6 warned that pinning either crate's version
     independently would silently make the alignment map name
     `transync-syntax`; this test makes that a red build.

  Parts 1, 3, and 4 weld **three** artifacts to each other — the table,
  `DOCUMENTED`, and `first_class` — so any one of them drifting from another
  is a red test or a red compile. The direction that stays *unchecked* is
  `lib.rs` itself: a `pub use` added there alone widens the real surface
  without failing anything, because nothing enumerates the facade's exports
  mechanically. That was a deliberate scope call — the mechanical answer is a
  rustdoc-JSON diff (`cargo public-api` or equivalent), not a scrape — and
  until it lands the direction is human-enforced by the three-place editing
  obligation (`lib.rs`, `first_class`, the table) plus a manual pre-release
  read-through of `lib.rs`, both stated in `contracts.md` §0.
- **`contracts.md` §1 additions:** the `TransyncError::stable_code()`
  seven-code vocabulary is now an explicit inter-process contract
  (append-only; a code never changes meaning), all **nine** `llm::prompt`
  items are named instead of four, and the *pattern* half of
  `#[non_exhaustive]` is stated (external destructuring needs a trailing
  `..`).
- **`llm/prompt.rs` says outright** — on the module and on
  `EXTRACTION_SYSTEM_PROMPT` — that the prompt/instruction **text is not a
  contract**; only the schema object shape and the parse behavior are. The
  golden byte-identity pins exist to make a wording change *deliberate*,
  not to promise callers a stable string.

### Part G — Gates

- **Standing rustdoc gate (owner-approved, added in flight).**
  `scripts/smoke.sh` runs
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps -p transync-syntax -p transync-core -p transync`
  (commit `bda8c1a`). Part B's zero-warning state was a one-time result;
  this line is what keeps it true.
- **Hook JS-section back-port (DCR-0017 handover).** The tracked
  `scripts/hooks/pre-commit` carried a JS section that assumed a root
  `package.json` and a `pnpm exec` / `npx exec` indirection (`npx` has no
  `exec` subcommand). It now discovers the package root (`.` then `web/`),
  invokes locally-installed binaries directly, prefers `biome check` when
  present, and **skips with a message** rather than failing when no JS
  linter is installed — no JS linter is a declared devDependency yet, so
  failing there would block every commit.
- **DISCOVERY during the back-port: the tracked hook was not live.**
  `core.hooksPath` was previously **unset**, so git ran only
  `.git/hooks/pre-commit`; `scripts/hooks/pre-commit` was documentation
  wearing an executable bit. `scripts/install-hooks.sh` set
  `core.hooksPath=scripts/hooks`, which makes the tracked copy the one that
  runs. The `.git/hooks/pre-commit` copy is now **inert but deliberately
  kept byte-identical**, so nothing regresses if `core.hooksPath` is ever
  cleared. Every DCR-0017 claim about "both hook copies" was true of the
  files and false of the execution — this is the correction.
- The pre-existing gate set (fmt, clippy `--all-targets -D warnings`,
  `cargo test --workspace -- --test-threads=4`, the CLI stub suite, the
  wasm32 check, the Playwright SCN-13 suite, the drift tests) stayed green
  at every task boundary.

### Plan amendments (recorded because the execution order is not the plan's)

- **Before dispatch:** a probe confirmed that clippy's
  `field_reassign_with_default` fires on default-then-assign, and that its
  suppression is only expressible where the FRU alternative is illegal
  (cross-crate `non_exhaustive`). That forced the `#[non_exhaustive]`
  attributes and the whole construction-site sweep into **one atomic
  commit** (`7f62cb9`) — splitting them would have left the tree failing
  `clippy -D warnings` in between. Executed order became tasks
  1, 6, 2, 3, 4, 5, 8, 9, 10, 11, 12, 13; resp-translator migrated
  immediately after task 1 for the same reason (its own hook runs
  `clippy -D warnings`).
- **In flight (owner-approved, commit `f1c714f`):** the facade's unused
  `transync-syntax` dependency drop folded into Task 11, and the standing
  rustdoc gate folded into Task 12.

### Owner-decision corrections (2026-08-04)

Two statements in OI-0027's 2026-08-03 owner decision were wrong on the
facts and were re-decided during execution:

1. **"`TranslateOptions` … (construction via `..Default::default()`)" was
   unsound.** Functional-record update is *illegal* cross-crate on a
   `#[non_exhaustive]` struct — the two halves of that sentence cannot both
   hold. The owner re-decided **default-then-assign** on 2026-08-04, which
   is what shipped and what the type's own rustdoc now instructs.
2. **Tier (a)'s "`TranslateOutput`" is actually `TranslationOutput`.** A
   naming slip in the decision text, not a rename: no type was renamed in
   this wave, and `TranslationOutput` is the name in §0 and on the facade.

### Invariants held

**Nothing on the wire moved.** The alignment-map schema stays **`1.2.0`**,
`VALIDATION_SCHEMA_VERSION` stays **`2`**, `CacheKey`'s identity and shape
are untouched, and **both `sync.js` copies** (`web/js/sync.js` and the CLI's
embedded copy) were not modified. This wave changed *who may name what* in
Rust, and — apart from the `translate_with_cache` preflight in Part E —
changed no behavior.

## Why

DCR-0005 built the facade as a semver firewall, but `pub use
transync_core::*;` over an all-`pub` core meant the firewall had no
aperture: every item core made public became public API by accident, and
item-level breakage passed straight through. Two consecutive structural
waves (ADR-0018 / DCR-0016's HTML feature, DCR-0017's crate split) had just
widened that surface further — DCR-0017 explicitly handed forward a list of
items that were `pub` "only because the crate boundary forced it". A
firewall whose aperture is "everything" only costs discipline; it never
pays.

The curation had to happen **inside the 0.2.0 window** because the fixes are
breaking by construction: hiding `run_pipeline`, adding `#[non_exhaustive]`,
and removing glob re-exports all break code that compiles today. Doing it as
the window-closing step means a 0.3 breaking window becomes unnecessary —
`#[non_exhaustive]` makes future variant and field additions additive, and
the explicit list makes future widening a deliberate act.

Making the boundary **compiler-enforced** rather than documented is the same
argument OI-0028 used for a crate split over feature gates: a `pub(crate)`
is a gate a person cannot forget. The drift test is the other half — a
curated list and a documented table are two things that can disagree, so
they are welded from both directions and the weld is a build failure, not a
review comment.

`TranslationUnit` got a constructor rather than `Default` because `Default`
is meaningless for it: a unit with no id, no payload, and a zero source hash
is not a degenerate unit, it is a cache-poisoning bug. The five positional
fields are the ones with no honest zero.

## Affected Areas

- `crates/transync/src/lib.rs` — the explicit curated re-export list; `pub use transync_core::*;` and `pub use transync_syntax;` removed
- `crates/transync/Cargo.toml` — `transync-syntax` dependency dropped; header comment corrected
- `crates/transync/tests/public_surface.rs` (new) — the five-part drift gate
- `crates/transync-core/src/lib.rs` — module visibilities (`pub(crate)` for `batch`/`error`/`pipeline`/`validate` and the five syntax aliases; `#[doc(hidden)] pub` for `unit`), root aliases, `TranslateOptions` attribute + construction doc
- `crates/transync-core/src/llm.rs` — `TranslationUnit` attribute + `new`/`with_*`; `TranslatorError` attribute
- `crates/transync-core/src/llm/prompt.rs` — prompt-text-is-not-a-contract notes; rustdoc de-links
- `crates/transync-core/src/error.rs` — `TransyncError` attribute + stable-code pointer
- `crates/transync-core/src/profile.rs` — attributes on the four profile structs
- `crates/transync-core/src/batch.rs` — `OutputBudgetWarning` moved out; `group_by_unit_count` + its `#[allow(dead_code)]` deleted; `estimate_unit_tokens` → `#[cfg(test)]`
- `crates/transync-core/src/validate.rs` — `OutputBudgetWarning` moved in; `ValidatedBatch::batch_id_str` dropped; rustdoc de-links
- `crates/transync-core/src/validate/schema.rs` — `check_schema` → `#[cfg(test)]`
- `crates/transync-core/src/pipeline.rs`, `src/pipeline/retry.rs` — the three no-op hooks deleted with a greppable tombstone comment
- `crates/transync-syntax/src/htmlseg.rs`, `src/outcome.rs`, `src/walk.rs`, `src/render.rs` — the three narrowings + rustdoc de-links
- `crates/transync-syntax/src/error.rs` — `ParseError` attribute
- `crates/transync-openai/Cargo.toml`, `tests/live_smoke.rs` — engine dev-dependencies + direct engine paths
- `crates/transync-openai/src/client.rs` — root aliases; `TranslationUnit::new` at the construction site
- `crates/transync-cli/src/translate_cmd.rs`, `crates/transync-cli/tests/sync_js_drift.rs` — default-then-assign; root alias
- `crates/transync/tests/common/mod.rs` (`opts_for`), `tests/common/mock_translator.rs`, `tests/boundary_v02.rs`, `tests/error_fixtures.rs`, `tests/reader_honesty.rs`, and the SCN-01..15 scenario suites — construction sweep; `scn_10` onto `translate_with_cache`
- `scripts/smoke.sh` — standing `RUSTDOCFLAGS="-D warnings" cargo doc` gate
- `scripts/hooks/pre-commit` — JS-section back-port (and the `core.hooksPath` discovery above)
- Contracts / architecture: `docs/architecture/contracts.md` (§0 new, §1 extended), `docs/architecture/source-of-truth-table.md`, `docs/implementation/module-map.md`
- Records: this DCR (new), `DCR-0005` (dated note), `docs/decisions/0018-…` (second dated path note), `docs/project/open-issues.md` (OI-0027 RESOLVED), `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/index.md`, `CHANGELOG.md`
- Sibling repo: `/Volumes/Common/QJoon/resp-translator` commit `8b6dbd5`

**Deliberately not rewritten:** the dated snapshots that name the pre-curation
paths — DCR-0016's and DCR-0017's bodies, the resolved-issue blocks in
`open-issues.md`, and the specs/plans under `docs/superpowers/` — keep their
as-of-that-date wording, following DCR-0017's own precedent.
`docs/project/stub-manifest.md` keeps its historical STUB rows for the same
reason (its own convention says so); only its live DEFERRED rows and its
path-migration note were touched.

### Discriminating tests

- `translation_unit_constructor_defaults_match_first_dispatch_shape`
  (`transync-core::llm`) — `new` sets the `BatchId::new(0)` batcher
  placeholder and `retry: None`, and the `with_*` chain overrides both.
- `surface_table_matches_the_documented_list` — bidirectional `DOCUMENTED` ↔
  contracts.md §0 equality, plus a duplicate-row guard.
- `first_class_module_matches_the_documented_list` — bidirectional
  `DOCUMENTED` ↔ `first_class` equality by scraping the test's own source,
  plus a duplicate-`pub use` guard.
- `hidden_modules_are_not_documented_as_surface` — segment-based negative
  test; `profile::render_prompt_body` must pass while any `render::…` path
  fails.
- `generator_version_matches_the_facade_crate` — DCR-0017 M6 guard.
- The `first_class` module itself is a compile-time assertion: it has no
  `#[test]`, and its failure mode is a build error naming the missing item.

Suite totals after the wave: **340 passed / 0 failed / 3 ignored**
workspace-wide at `--test-threads=4` — the pre-wave 335, plus the
`TranslationUnit` constructor test and the four `public_surface.rs` tests.
**No test was removed anywhere in this wave**, including by the dead-code
closure fallout.

## Migration / Follow-up

Breaking changes ride the **still-open 0.2.0 window**. The consumer-facing
summary:

- **`transync::pipeline::run_pipeline` → `transync::translate_with_cache`.**
  Same four arguments, same return type, plus a fail-fast preflight on an
  empty `target_language`.
- **`transync::{parser, id, unit, batch, validate, regen, render, align,
  pipeline}` and `transync::transync_syntax` are gone from the facade.** A
  consumer that genuinely needs an engine internal — a WASM renderer host,
  an alternative front-end — depends on `transync-core` / `transync-syntax`
  **directly** and accepts their weaker stability promise. In-tree,
  `transync-openai`'s live smoke is the worked example.
- **Module paths that became root aliases:** name `transync::ParseError`,
  `transync::GlossaryScope`, `transync::ALIGNMENT_SCHEMA_VERSION`, and
  `transync::OutputBudgetWarning` at the crate root.
- **`#[non_exhaustive]`:** matches over `TranslatorError` / `TransyncError`
  / `ParseError` need a wildcard arm; destructuring any of the five
  `#[non_exhaustive]` structs needs a trailing `..`; construction is
  default-then-assign (or `TranslationUnit::new` + `with_*`). Struct-literal
  and `..Default::default()` construction are in-crate only.
- **Deleted items:** `pipeline::retry::{retry_validation, oversize_split,
  fallback_to_source}`, `batch::group_by_unit_count`,
  `ValidatedBatch::batch_id_str`. All were `pub` and all were no-ops or
  unreferenced; none had an out-of-tree caller that the workspace or the
  sibling repo could name.
- **Unchanged:** `out.md`, the alignment map and its `1.2.0` schema,
  `VALIDATION_SCHEMA_VERSION` (`2`), `CacheKey`, and both `sync.js` copies.

Follow-ups deliberately left open:

- **0.2.0 release** is next — this was the last item on that road.
- **OI-0008's remaining bullet groups** stay open: the retry/fallback policy
  redesign (R0001-0057) is *not* closed by deleting the three no-op hooks,
  and the concern-mixing in `run_pipeline`, the parser walker, unit
  construction, the OpenAI client, and the CLI translate command is
  untouched.
- **`unit` is `pub` on sufferance.** The one reason it is not `pub(crate)`
  is `transync-openai`'s offline fixture-shape test. If that test is ever
  restructured to assert through the facade, `unit` should follow the other
  engine modules.
- **Track C** (wasm-bindgen entry points, JS integration, browser demo)
  remains post-0.2.0 work, unaffected by this wave.

### Addendum (2026-08-04, pre-tag)

At release prep the owner extended the `#[non_exhaustive]` set to the six
read-only **output types** — `TranslationOutput`, `ValidationReport`,
`AlignmentMap`, `AlignmentBlock`, `ValidationSummary`, `GeneratorMeta` —
adopting the final whole-branch review's spec-level observation (workspace
attribute count 11 → 17). A construction census found **zero cross-crate
literals** for all six, so no migration accompanied the attributes; the
JSON wire stays governed by `schema_version` independently of the Rust
attribute. `ByteRange` stays exhaustive by policy — a byte range is
complete by definition, constructed as plain data on both sides of the
boundary (contracts.md §1). Landed in the v0.2.0 release-prep commit, the
window's last change.

### Addendum (2026-08-06, backlog `public-surface-widening-gate`)

**The direction this record left unchecked is now checked.** Part F above
called `lib.rs` itself the one unguarded direction — a `pub use` added there
alone widened the real surface without failing anything — and scoped its
answer to a rustdoc-JSON diff (`cargo public-api`) that was never wired up.
`public_surface.rs` gained a **fourth** scrape instead,
`lib_rs_exports_nothing_the_documented_list_omits`: it reads
`crates/transync/src/lib.rs`, resolves each re-export to the name it
publishes (the `as` alias, else the last path segment), and asserts every
published name carries a §0 row. The three-place editing obligation
(`lib.rs`, `first_class`, the table) survives unchanged — what changed is
that forgetting any one place is now a red test rather than a missed manual
read-through, and the pre-release read-through stops being the only
unmechanized part of §0.

The scrape stays **dependency-free** on purpose. The OI-0027 spec (§0.2
approach 3) had already rejected a `cargo-public-api` snapshot gate as
install-dependent — the "silently skips" failure mode DCR-0017 called out —
and left the tool available only as a manual ritual step, never a test; a
scrape that ships with the test suite is what that rejection leaves room for.
It also refuses to fail open in its own way: any top-level `pub` form it
cannot read (`pub fn`, `pub struct`, an inline `pub mod NAME { … }`) is a hard
panic, not a skipped line, and the reverse inclusion is asserted too so a
scrape that stopped finding anything cannot pass vacuously. Feature-gated
exports are the one
allowance — `transync::test_stub`, which §0 already excluded in prose, is now
excluded by a named `FEATURE_GATED` constant that is asserted not to overlap
the table. What remains out of scope, and is stated as such in §0, is surface
reaching consumers without being named in `lib.rs` at all; that is still
rustdoc-JSON territory. No row of the 78-row table, no `DOCUMENTED` entry,
and no line of `lib.rs` changed.

### Addendum (2026-08-07, ti `000e5a`)

**Part G's gate had a hand-written crate list, and the list was short one
member.** The line landed in `bda8c1a` naming `transync-syntax`,
`transync-core` and `transync`; DCR-0020 widened it by hand to include
`transync-wasm`. `transync-openai` — a published library member with a
`pub mod client` and a `pub mod error` — was never in it, so its rendered
documentation was the one library surface in the workspace nobody was
checking. Run against it, the gate failed with **eight**
`private_intra_doc_links` errors, all of them in `client`'s public module
doc: the "# Layout" list linked the six private submodules it names, and
the paragraph beneath linked the private `SurfaceRequest` and
`round_trip`. Fixed the way Part B's precedent and `d8b1cac` fix this
shape — the prose **names them in plain code spans** rather than widening
eight internals to make links resolve.

**The gate now checks its own list rather than trusting it.** The crate
names still live in `scripts/smoke.sh`, in a `RUSTDOC_GATE_CRATES` array,
because an explicit list is what makes a gate auditable. What is new is
that the script walks `crates/*/` and **fails the run** if any member with
a `src/lib.rs` is missing from that array, naming it and the three places
to add it. So a future member joins the gate by being caught, not by being
remembered. `transync-cli` is the one member outside the gate and stays
outside it deliberately — bin-only, no public API to document — and the
check agrees with that by construction, since it has no `src/lib.rs`.

Two things this addendum does **not** change. The gate still lives in
`scripts/smoke.sh` alone: the pre-commit hook carries the wasm gate and
not this one, which is what the ticket had assumed otherwise, and a
comment beside the wasm gate in `scripts/hooks/pre-commit` now records
that so the next reader does not commit believing rustdoc was checked.
And the facade is untouched — no §0 row, no `DOCUMENTED` entry, no
visibility, and no line of `crates/transync/src/lib.rs` moved; the eight
items stayed private, which is the whole point of de-linking rather than
publishing them.
