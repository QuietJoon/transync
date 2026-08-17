---
type: DCR
title: transync-syntax crate split + whole-document AST-direct renderer
description: A fifth workspace member, crates/transync-syntax, takes the whole syntax layer (parser, id, regen, render, align, htmlseg, outcome, walk, ParseError) and compiles for wasm32-unknown-unknown behind a standing gate; transync-core keeps the pipeline on top. Dissolving the boundary inversions made regenerate/build_alignment_map take neutral parameters, and the renderer was rewritten from per-block fragment reparse to one AST-direct parse per pane, retiring strip_outer_wrapper and the ref-defs append. Resolves OI-0028, narrows OI-0008.
tags: [change, project-control, DCR-0017]
status: deprecated
---

# DCR-0017: `transync-syntax` crate split + AST-direct renderer

- **Date:** 2026-08-04
- **Source:** OI-0028 crate-split wave, implementing the owner-approved design spec `docs/superpowers/specs/2026-08-04-transync-syntax-split-design.md` (v2, review-hardened, approved 2026-08-04) against the 9-task plan `docs/superpowers/plans/2026-08-04-transync-syntax-split.md`. No new ADR: the decision this executes is OI-0028's 2026-08-03 owner decision (Option B — crate split), and the records it changes are ADR-0003 / ADR-0006 / ADR-0007 / DCR-0005 / DCR-0007 amendments below.
- **Resolves:** **OI-0028** (WASM render path has no compile path). **Narrows:** **OI-0008** — the two renderer items (R0001-0069 per-block reparse, R0001-0096 brittle `strip_outer_wrapper`) are cleared; the other four bullet groups stay open.
- **Affected ADRs:** `docs/decisions/0003-cargo-workspace-with-provider-crates.md` (**dated amendment 2026-08-04** — the dependency DAG gains `transync-core → transync-syntax`, the workspace is five members, the tree and the "three `Cargo.toml` files" tradeoff line updated); `docs/decisions/0006-renderer-output-shape.md` (**dated mechanism amendment 2026-08-04** — the decision stands; the renderer's internal mechanism and its crate home changed); `docs/decisions/0007-renderer-wrapper-div-for-tables-and-code.md` (**dated mechanism amendment 2026-08-04** — the wrapper-div decision stands; the AST-surgery-free premise is now satisfied by whole-node formatting rather than by string-stripping comrak's fragment output); `docs/decisions/0004-comrak-as-gfm-parser.md` (upheld — one parser, now with strictly fewer invocations); `docs/decisions/0018-html-content-translation-via-segment-extraction.md` (upheld; its `htmlseg` path moved — dated path note added); `docs/decisions/0015-cache-poison-and-invalid-hit-policy.md` (upheld — the reason the Guard-1 validator tightening needs **no** `VALIDATION_SCHEMA_VERSION` bump). **Not amended:** DCR-0005's dialect-trait deferral text — see *Dialect trait NOT instantiated* below.

## What Changed

### Part A — Crate topology

- **`crates/transync-syntax`** is the workspace's **fifth member** (explicit
  `members` list, never a glob — ADR-0003). It has **no `[features]` at all**
  and **no core-directed dev-dependency**: either one would reintroduce the
  feature-unification trap that Option B exists to avoid, or force the wasm
  gate down to `--lib` and lose test-target coverage.
- **What it owns:** `parser` (+ `parser/ranges`, `parser/refdefs`), `id`,
  `regen`, `render` (+ `render/attrs`), `align`, `htmlseg`, plus two modules
  created by the split — `outcome` (the relocated `HtmlOutcome` closure) and
  `walk` (the single shared top-level normalization) — and
  `error::ParseError`. `Section` / `Document::hierarchy` moved with the parser
  (no pipeline consumer).
- **What `transync-core` keeps:** `pipeline` (+ `pipeline/retry`), `batch`,
  `llm` (+ `llm/prompt`), `validate` (+ its five layer modules), `cache`,
  `profile`, `unit` (+ `unit/context`), `error::TransyncError`.
- **Dependency partition.** `transync-syntax`: comrak, serde, serde_json,
  siphasher, thiserror, lol_html, htmlize. `transync-core` (after):
  transync-syntax + serde, serde_json, thiserror, tracing, async-trait,
  comrak, toml, futures, tiktoken-rs, tokio. comrak stays in **both**
  manifests deliberately — `parser::comrak_options()` and
  `ranges::byte_range_for` put comrak types in syntax's public signatures and
  core's `validate`/`unit` parse with them, so the shared
  `{ workspace = true }` entry makes a version mismatch a compile error.
- **Facade.** `crates/transync` gains a direct `transync-syntax` dependency
  and `pub use transync_syntax;` alongside `pub use transync_core::*;`, so
  consumers can name the base crate directly
  (`transync::transync_syntax::…`) without adding a dependency.
- **Dependency canary (2026-08-04, first gate of the split).** The full
  `transync-syntax` dependency set — comrak **0.27.0**, serde **1.0.228**,
  serde_json **1.0.149**, siphasher **1.0.2**, thiserror **1.0.69**, lol_html
  **2.9.0**, htmlize **1.1.0** — **compiles** for
  `wasm32-unknown-unknown`. Compile-only, as with OI-0028's 2026-08-03
  canary: nothing was executed on a WASM host and runtime behavior is
  unverified. Per OI-0028's canary-validity rule the manifests float and
  there is no committed lockfile (OI-0020), so the standing gate below — not
  this one-time result — is what keeps the property true.

### Part B — Boundary inversions dissolved, and the migration list

The proposed cut was not a DAG cut. Three moves dissolved every inversion —
relocating the `HtmlOutcome` closure to syntax (which alone killed four of
`align`'s five inversions and all seven test-helper inversions), and
neutralizing `regen::regenerate` and `align::build_alignment_map`. Each
intentional break is listed here (per owner decision 5: path preservation is
the **default**, not an obligation — a better boundary may break a path, and
the break lands in this list).

- **M1 — `regen::regenerate` takes a neutral map.**
  `pub fn regenerate(doc: &Document, accepted: &HashMap<BlockId, String>) -> (String, BlockOffsets)`.
  **Fallback is expressed by absence:** a block missing from `accepted` falls
  back to its own source bytes, which is the whole contract. `regen`'s
  `collect_translations` helper was **deleted** — it read only `unit_id` +
  `accepted_payload` (the `final_status` it also collected was never
  consumed); `pipeline::regen_pass` builds the map instead.
- **M2 — `align::build_alignment_map` takes neutral statuses.**
  `&[ValidatedBatch]` → `&HashMap<BlockId, FallbackStatus>` (it read only
  `unit_id` + `final_status`). The pipeline now owns the completeness
  obligation this moved outward, pinned by a **debug-only assertion** in
  `regen_pass`: every block `outcome::is_translatable_block` accepts must
  carry a finalized status into the map, so no translatable row can silently
  synthesize `preserved`.
- **M3 — `build_alignment_map`'s languages arrive as plain strings.**
  `&TranslateOptions` → `source_language: &str, target_language: &str` (the
  only two fields it read), which takes the function from **arity 6 to 7**.
  Full signature:
  `build_alignment_map(doc, statuses, offsets, source_language, target_language, detected_source_language, html_outcomes) -> AlignmentMap`
  — `offsets` / `detected_source_language` / `html_outcomes` carry over
  unchanged. M2 + M3 together are what make `align` carry **no** pipeline
  types, which is the reason it can live in the wasm-gated crate at all.
- **M4 — `regen::top_level_blocks` widened to `pub`** (the pipeline consumes
  it). Non-breaking — a new public item, no existing path changed.
- **M5 — the moved module paths are now re-exports.** `transync-core` carries
  `pub use transync_syntax::{parser, id, regen, render, align};`, so every
  `use crate::parser::…` / `crate::id::…` inside core resolves unchanged, the
  facade's `pub use transync_core::*;` is untouched, and **every verified
  downstream path is preserved** — `transync::align::ALIGNMENT_SCHEMA_VERSION`,
  `transync::id::BlockId`, `transync::parser::parse`, the root
  `BlockId`/`FallbackStatus` aliases, and `transync::unit::html_outcomes`
  (`unit` re-exports `HtmlOutcome` + `html_outcomes` from
  `transync_syntax::outcome`, which is what keeps `transync-openai`'s
  `live_smoke.rs` compiling untouched). `transync::error::ParseError` keeps
  resolving because core's `error.rs` holds
  `Parse(#[from] transync_syntax::error::ParseError)` and re-exports the type.
- **M6 — `GeneratorMeta.version` provenance moved crates.** The
  `env!("CARGO_PKG_VERSION")` that stamps every alignment map's
  `generator.version` now compiles inside `transync-syntax`, so it reports
  **that** crate's version, not `transync-core`'s. **Inert today** and
  intended to stay so: both crates carry `version.workspace = true`, so the
  two versions are the same string by construction. **Constraint to
  preserve:** if either crate ever pins its own `version`, the alignment
  map's `generator.version` silently starts naming the syntax crate — either
  keep them in lockstep or move the stamp behind an explicit constant.
- **Core-internal path change (not a public break):** core's `unit.rs` and
  `validate.rs` call `transync_syntax::htmlseg::…` where they used to call
  `crate::htmlseg::…`. `htmlseg` is `pub` in syntax but `#[doc(hidden)]` and
  is still **not** re-exported on the facade, so no consumer path changed.
- **`parser/refdefs`' rustdoc links de-linked.** Its intra-doc links into
  `crate::validate::*` are unresolvable cross-crate and became plain text.
  Four sibling references that *would* still resolve were left plain **for
  symmetry** — the whole paragraph reads as one prose list, and a half-linked
  list is worse than an unlinked one.
- **Plan gap fixed in flight:** the plan's Task 1 omitted adding the
  `transync-syntax` dependency to `crates/transync-core/Cargo.toml` (it only
  added it to the facade). Task 2 added it as its first step; the tree was
  never left non-compiling.

#### Widened surface handed to OI-0027

These items are `pub` **only because the crate boundary forced it**. Each is
`#[doc(hidden)]` where it is not real API. OI-0027's curation owns the final
disposition; the narrowing notes are the split's own findings, recorded so
they are not rediscovered:

For the `htmlseg` rows below, the `#[doc(hidden)]` sits on the **module
declaration** in `transync-syntax/src/lib.rs` (`#[doc(hidden)] pub mod
htmlseg;`) and is inherited by every item — there are **no** per-item
attributes to find in `htmlseg.rs`. The `outcome` rows are the opposite: those
attributes are per-item, on the four helpers themselves.

| item(s) | status | note for OI-0027 |
|---|---|---|
| `htmlseg::{extract, splice, tag_inventory, HtmlSegments}` | `pub`, hidden via the module | genuinely cross-crate (`unit`/`validate` consume them) |
| `htmlseg::balance_fragment` | `pub`, hidden via the module | **may narrow to `pub(crate)`** — its only consumer, `render`, is now in the same crate |
| `outcome::{HtmlOutcome, html_outcomes}` | `pub`, documented | real API; core's `unit` re-exports both, preserving `transync::unit::html_outcomes` |
| `outcome::{block_payload, is_translatable_block, has_translatable_blocks}` | `pub` + `#[doc(hidden)]` | cross-crate consumers exist (`unit`, `validate`, `pipeline`) |
| `outcome::is_translatable` | `pub` + `#[doc(hidden)]` | **no cross-crate consumer** — could narrow |
| `walk::{normalize_top_level, NormalizedEntry, direct_item_count, node_label}` | `pub` | cross-crate by design — `validate::full_reparse` is the whole point of the shared home |
| `walk::label_for` | `pub` | **could be `pub(crate)`** — no cross-crate consumer today |
| `regen::top_level_blocks` | `pub` | M4; pipeline consumer |

Also inherited by OI-0027: **six rustdoc warnings** in `transync-syntax`
(`cargo doc -p transync-syntax --no-deps`), all of the same shape —
public-item docs linking a private sibling. Five are in `htmlseg`
(`htmlseg` → `rewriter_settings`; `extract` → `scan`; `splice` → `scan`,
`rewriter_settings`, `wanted_text_type`), where a `#[doc(hidden)] pub` module
now surfaces links into its own private internals; the sixth is the renderer's
module doc pointing at the private `node_inner_html`, introduced by the
per-kind emission table in Part C. They are warnings only — `cargo doc`
succeeds. Fixing them means either widening the linked items or de-linking the
prose (as `parser/refdefs` did), which is a curation call, not a split call.

### Part C — Renderer rework: whole-document AST-direct

`render_source` / `render_target` keep their signatures. Everything behind
them changed.

- **One comrak parse per pane** (source pane parses `doc.source_text`, target
  pane parses the regenerated `translated_md`; the arena is local to the
  call), replacing per-block fragment reparse. The top-level node sequence is
  zipped against the alignment rows through the **single shared**
  `walk::normalize_top_level` helper that `validate::full_reparse` also uses
  — two copies in two crates would drift.
- **Per-kind emission table.** Headings 1–6, Paragraph: format the node's
  **children** (the strip-kinds' inner HTML). Image: format the Paragraph
  node's children into the `<figure>` wrapper. ListItem: format the
  `Item`/`TaskItem` node's children into the `<li>`, and for `TaskItem` the
  **renderer itself** emits the checkbox `<input>` prefix — comrak writes it
  in the item's open tag, not in any child, so children-format alone would
  drop it (checked-ness derives from the node's symbol, cross-checked against
  the row's `task` flag). Table, CodeBlock, Blockquote: format the **whole
  node**, so comrak's own `<table>` / `<pre><code>` / `<blockquote>` lives
  inside the transparent `<div>` exactly as before — CodeBlock is a childless
  leaf and children-format would emit nothing, and Table's `</tbody>` is
  written only by the Table exit arm. Html, Skipped, ThematicBreak: bypass
  arms, unchanged presentation, node still advances the walk. List group tags
  (`<ul>` / `<ol start=N>`) are written by the renderer from the `List`
  node's `list_type` / `start`.
- **`cr()`-emulation assembly.** Child outputs concatenate into one buffer,
  inserting `\n` before a block-level child when the buffer does not already
  end with one — each `format_html` call resets comrak's `WriteWithLast`, and
  a tight paragraph ends without a trailing newline. The final trailing
  newline is trimmed before the wrapper closes, so the one-line-per-block
  `writeln!` shape survives (scn_08's same-line pin stays valid).
- **Deleted outright:** `strip_outer_wrapper` (all arms **and** its dead
  fallback path — the R0001-0096 debt), `block_inner_html` (the per-block
  reparse closure — the R0001-0069 debt), `ordered_list_start`, and the
  **per-fragment append of the document's whole ref-defs pool**. A
  whole-document parse resolves reference links natively; `doc.ref_defs`
  survives for `validate::inline` only.
- **Cost.** Per run: **2 parses** instead of `2 × (N_blocks + N_ol_runs)`,
  and the O(N_blocks × |ref_defs|) append tax is gone entirely (a 200-block
  document with a 5 KB definition pool used to re-parse ~2 MB of definitions
  per pane). This is the cost profile that would otherwise have shipped to
  the browser under Track C.

#### Accepted byte-shape changes (spec §4.4)

Both panes became more faithful in three known ways. The first two are
**source-driven**, not translation-driven — `validate::per_kind` already
rejects translation-introduced looseness via the `tight` topology field:

1. **Loose source lists render loose in BOTH panes.** A source list whose
   items are blank-line separated parsed *tight per item* under fragment
   reparse and rendered without `<p>` wrappers. The whole-document parse sees
   the blank lines and renders `<li><p>…</p></li>`, in the source pane and
   the target pane alike.
2. **comrak's loose-task-item shape** — `<input …/>` followed by a newline
   and then `<p>` — is now emitted verbatim rather than approximated.
3. **BUG FIX: 4-space-indented code blocks rendered as `<p>`.** The old path
   called `md.trim()` on each block's slice before reparsing, which destroyed
   the leading indent that *made* it a code block, so an indented code block
   came out as a paragraph. The AST-direct path never trims, and the block
   renders as `<pre><code>`. Regression-pinned by
   `indented_code_block_renders_as_code_not_paragraph`.

Attribute order and the one-line-per-block shape are unchanged
(`write_attrs` and the `writeln!` discipline did not change), so scn_08's
same-line pin and the scn13 DOM assertions hold.

**Byte-diff evidence:** rendering the SCN-14 fixture through the old and the
new renderer produces **byte-identical** output in both panes — the fixture
contains no loose list, no loose task item, and no indented code block, so
the three changes above are exactly the delta and nothing else moved.

### Part D — The two guards

`pipeline::finalize_regen_with_reparse_policy` already guarantees that when
render runs, either `reparse_full` returned `Ok` on this exact `md` (top-level
label sequence == normalized source sequence) or the cascade escalated to
`FallbackAll`, whose output is structurally identical to the source. The one
hole: `reparse_full` never counted a list's items.

- **Guard 1 (validation-side).** `validate::full_reparse` gains a third scan:
  per top-level list, `walk::direct_item_count` (direct `Item` **or**
  `TaskItem` children only — nested lists' items excluded) must equal the
  normalized entry's collapsed-`ListItem`-row count. **Defense in depth, not
  a new front line:** `per_kind::check_list` already rejects naive per-unit
  splits and merges via list topology, so the residual this check owns is
  **cross-unit splice adjacency** — individually-valid payloads that merge or
  split at a regen boundary without moving the top-level label sequence (a
  translation that indents a list marker collapsing a list is the pinned
  case) — plus being the invariant the render zip's row↔item pairing actually
  needs. **Validator tightening only: NO `VALIDATION_SCHEMA_VERSION` bump**,
  per the const's own bump discipline (cache hits are re-validated —
  ADR-0015).
  - **Spec residual (a) DISPROVEN.** The spec listed "units whose
    `expected_list_topology` came back `None`" as a residual. It cannot
    happen for well-formed items: comrak anchors an `Item`'s sourcepos at the
    **marker**, so `inspect_list_topology` always gets an item subtree back
    and never returns `None` for a payload sliced from a real list item. The
    premise is void, and the only route that could revive it is the lone-CR
    sourcepos defect filed as **OI-0033** (below).

    **Note (2026-08-05):** OI-0033 is fixed under the *support* posture —
    `LineOffsets` now counts a lone `\r` as a line boundary, matching
    comrak's own `is_line_end_char` rule — so the revival route is gone and
    residual (a) is **void unqualified**. Asserted directly, not inferred:
    `unit::payload::lone_cr_topology_tests::lone_cr_list_items_carry_list_topology_constraints`
    parses a lone-CR list document and requires every list-item unit to carry
    a non-empty payload *and* `Some(expected_list_topology)`. A parser-side
    guard (`parser::emit::warn_on_empty_list_item_range`) backs this up for the
    open class of future sourcepos quirks: a list item with a non-degenerate
    `Sourcepos` but an empty computed range now emits a document warning
    naming the block instead of silently disarming `check_list`. The guard is
    scoped strictly to list items because SCN-15's comment block legitimately
    slices to an empty range.
- **Guard 2 (render-side, defensive).** If the zip still cannot pair a row
  with a node — unreachable by construction — that row alone degrades to the
  old byte-range path: escaped bytes emitted inside its normal wrapper, the
  **anchor survives**, and the node cursor is **not** advanced (so a
  mispaired row cannot cascade into the rows after it). Never a panic, never
  a dropped anchor. Pinned by
  `zip_mismatch_degrades_to_escaped_bytes_never_panics`.

### Part E — Gates

- **`cargo check -p transync-syntax --target wasm32-unknown-unknown` is a
  standing gate line** in **both** pre-commit hook copies
  (`scripts/hooks/pre-commit`, tracked; `.git/hooks/pre-commit`, the active
  untracked copy) and in `scripts/smoke.sh`.
- **Hook guard posture: fail loudly, never skip.** The hook first checks
  `rustup target list --installed` for `wasm32-unknown-unknown`; if the
  target is absent it **fails the commit** with the one-line
  `rustup target add wasm32-unknown-unknown` hint, rather than silently
  passing. `smoke.sh` runs the check unconditionally.
- The pre-existing full gate set (fmt, clippy, workspace tests at
  `--test-threads=4`, CLI stub suite, Playwright, drift tests) stayed green
  at every task boundary.
- `.claude/settings.local.json` gained a `Bash(cargo check:*)` allowlist
  entry so agent-run gates do not prompt.

### Dialect trait NOT instantiated

This split created the **crate boundary only**. DCR-0005's `Dialect`-trait
deferral stands **unchanged and unamended**: its release condition is a real
second dialect supplying real constraints, and that condition is **still
unmet** (GFM remains the only dialect). A trait with one implementation would
encode guesses. `transync-syntax` is the natural landing zone for that seam
when the condition is met — being the landing zone is not the same as being
occupied.

## Why

Track C (in-browser WASM rendering) is a confirmed goal, and `transync-core`
simply cannot compile for `wasm32-unknown-unknown`: it unconditionally
inherits tokio with `rt-multi-thread` (a `compile_error!` on wasm32) and
pulls tiktoken-rs, neither of which rendering needs. The owner chose a crate
split over feature gates because the boundary is **compiler-enforced** — a
`cargo check --target wasm32-unknown-unknown` line is a gate a person cannot
forget, with no feature-unification trap and no `#[cfg]` sprawl through mixed
files. Only a split makes "the syntax layer is WASM-clean" a fact the build
checks rather than a claim the docs make.

The renderer rework had to ride the same effort rather than follow it,
because shipping today's renderer to WASM would have **exported its costs to
the browser**: every non-bypass block individually re-parsed, with the
document's entire link-reference-definition pool appended to each fragment.
The rework is not only cheaper, it is strictly more faithful — a
whole-document parse keeps parent links alive, so list tightness and
`<tbody>` decisions are context-faithful instead of reconstructed from a
one-block fragment. That fidelity is what surfaced the indented-code-block
bug: `strip_outer_wrapper`'s string surgery was brittle *and* the `trim()`
that fed it was silently lossy. Deleting the mechanism deleted the bug class.

Guard 1 exists because the render zip now *depends* on an invariant nobody
was checking. Guard 2 exists because "unreachable by construction" is a claim
about today's pipeline, and a dropped sync anchor is an invariant-1 violation
— degrading one block is always better than losing the anchor that keeps the
rest of the document aligned.

## Affected Areas

- `Cargo.toml` — `crates/transync-syntax` added to the explicit `members` list
- `crates/transync-syntax/Cargo.toml` (new), `crates/transync-syntax/src/lib.rs` (new)
- `crates/transync-syntax/src/{parser.rs, parser/ranges.rs, parser/refdefs.rs, id.rs, regen.rs, align.rs, render.rs, render/attrs.rs, htmlseg.rs}` — moved from `crates/transync-core/src/` via `git mv` (history preserved)
- `crates/transync-syntax/src/error.rs` (new) — `ParseError`
- `crates/transync-syntax/src/outcome.rs` (new) — `HtmlOutcome`, `html_outcomes`, `block_payload`, `is_translatable`, `is_translatable_block`, `has_translatable_blocks`
- `crates/transync-syntax/src/walk.rs` (new) — `normalize_top_level`, `NormalizedEntry`, `label_for`, `node_label`, `direct_item_count`
- `crates/transync-syntax/src/render.rs` — the AST-direct rewrite (per-kind emission, `cr()` emulation, Guard 2 degrade); `strip_outer_wrapper` / `block_inner_html` / `ordered_list_start` / ref-defs append deleted
- `crates/transync-syntax/src/regen.rs` — M1 neutral `accepted` map; `collect_translations` deleted; `top_level_blocks` widened
- `crates/transync-syntax/src/align.rs` — M2/M3 neutral statuses + language parameters
- `crates/transync-core/Cargo.toml` — `transync-syntax` dependency added; `lol_html` / `htmlize` removed; header comment corrected (it no longer contains the comrak front-end)
- `crates/transync-core/src/lib.rs` — module re-exports (`pub use transync_syntax::{align, id, parser, regen, render};`) + re-exported type aliases
- `crates/transync-core/src/error.rs` — `Parse(#[from] transync_syntax::error::ParseError)` + re-export
- `crates/transync-core/src/unit.rs` — `HtmlOutcome` closure removed; re-exports `HtmlOutcome`/`html_outcomes`; `transync_syntax::htmlseg::` call sites
- `crates/transync-core/src/unit/context.rs`, `src/validate.rs` — `transync_syntax::` call sites
- `crates/transync-core/src/validate/full_reparse.rs` — Guard 1 per-list item count; walks via `transync_syntax::walk`
- `crates/transync-core/src/pipeline.rs` — builds the `accepted` map and the `statuses` map; passes languages positionally; statuses-completeness `debug_assert!`
- `crates/transync/Cargo.toml`, `crates/transync/src/lib.rs` — `transync-syntax` dependency + `pub use transync_syntax;`
- `scripts/hooks/pre-commit`, `.git/hooks/pre-commit` (untracked), `scripts/smoke.sh` — the standing wasm32 gate line + the rustup fail-loudly guard
- `.claude/settings.local.json` — `Bash(cargo check:*)` allowlist entry
- Records: this DCR (new), `docs/decisions/0003-…` (dated amendment), `docs/decisions/0006-…` + `0007-…` (dated mechanism amendments), `docs/decisions/0018-…` (dated path note), `DCR-0005-…` + `DCR-0007-…` (dated notes), `docs/project/open-issues.md` (OI-0028 RESOLVED, OI-0008 narrowed, **OI-0033** filed), `CLAUDE.md` module-split list (on-disk; the file is globally gitignored)
- Contracts / architecture: `docs/architecture/contracts.md` §4 / §4a, `docs/architecture/README.md` (component table), `docs/architecture/source-of-truth-table.md`, `docs/implementation/module-map.md` (layout tree + internal interfaces)
- Guides: `README.md` (workspace layout), `docs/Developer_Guide.md` (workspace tree + gate list), `docs/Troubleshooting.md` (one `git log` path)
- Project state: `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/index.md`, `CHANGELOG.md`

**Deliberately not rewritten:** the historical records that name the old
module paths — `docs/project/stub-manifest.md`,
`docs/project/design-baseline-2026-07.md`, `DCR-0007`, `DCR-0013`, `DCR-0016`
— keep their as-of-that-date paths under a dated pointer note, following
DCR-0005's own precedent ("historical records intentionally keep the old
paths"). `docs/project/implementation-slice-checklists.md` stays `SL-00`..`SL-14`
MVP-scoped by design.

### Discriminating tests

Shared walk (`transync-syntax`): `normalize_collapses_one_list_and_splits_on_marker_change`,
`node_label_matches_the_source_side_labels`,
`node_label_of_an_unmodeled_node_is_skipped`,
`direct_item_count_counts_plain_items`,
`direct_item_count_counts_task_items`,
`direct_item_count_excludes_nested_list_items`,
`direct_item_count_of_non_list_is_zero`.

Guard 1 (`validate::full_reparse`, called directly with crafted regenerated
MD — a pipeline-driven split/merge dies at `per_kind` first and never reaches
this check): `reparse_full_accepts_unchanged_list_item_count`,
`reparse_full_accepts_unchanged_task_list_item_count`,
`reparse_full_accepts_nested_sublist_round_trip`,
`reparse_full_rejects_list_item_merge`,
`reparse_full_rejects_list_item_split`,
`reparse_full_rejects_task_list_item_split`. Pipeline-level residual:
`splice_adjacency_item_collapse_falls_back_and_completes` (a marker-indenting
translation collapses a list; the cascade falls back and the run completes).

Renderer shapes: `loose_source_list_renders_loose_in_the_pane` and
`loose_source_list_renders_loose_in_the_target_pane_too` (the change is
source-driven, so spec §4.4's looseness pin is claimed in BOTH panes; the
target half goes through the real `regen` → `render_target` path, because
looseness lives in the regenerated Markdown's blank lines and in no alignment
field), `loose_task_item_keeps_the_checkbox_then_newline_shape`,
`tight_item_with_nested_list_gets_the_cr_newline` (the `cr()`-emulation
case), `task_item_checkbox_survives`,
`table_keeps_its_element_and_tbody`,
`code_block_content_survives_whole_node_formatting`,
`blockquote_keeps_its_element`, `wrapper_close_stays_on_the_block_line`,
`reference_links_resolve_without_the_append`,
`indented_code_block_renders_as_code_not_paragraph` (the bug fix).

Guard 2: `zip_mismatch_degrades_to_escaped_bytes_never_panics`.

Suite totals after the split: `transync-syntax` **96** unit tests (the moved
modules' inline tests, now syntax-local with zero core types — the invariant
that keeps the wasm gate covering test targets), `transync-core` **165** (+1
ignored golden-regen helper), 21 scenario + 6 `boundary_v02` + 2
`error_fixtures` + 2 `reader_honesty` + 2 `docs_index_drift` in the facade
crate, `transync-openai` 25 (+2 ignored live), CLI 11 + 3 drift.

Unchanged-by-design suites that prove the paths survived: `boundary_v02`
(names nothing from the moving set), `docs_index_drift` (fixed depth-2 root
join, unaffected by a same-depth sibling crate), `cli_smoke`'s cross-crate
fixture join, the SCN-01..15 scenario suites in the facade crate,
`transync-openai`'s `live_smoke.rs` (`transync::unit::html_outcomes` still
resolves), and the Playwright SCN-13 suite.

## Migration / Follow-up

Breaking changes ride the **still-open 0.2.0 window** (owner decision 6:
improvement-driven breaks are all allowed in it). Every intentional break is
in the **Part B migration list** above; the consumer-facing summary:

- **`regen::regenerate`** — signature changed to
  `(doc, &HashMap<BlockId, String>)`. A caller that passed
  `&[ValidatedBatch]` must build the accepted-payload map itself; omit a
  block to fall back. `regen::collect_translations` is **gone**.
- **`align::build_alignment_map`** — arity 6 → 7; `&[ValidatedBatch]` →
  `&HashMap<BlockId, FallbackStatus>` and `&TranslateOptions` →
  `source_language: &str, target_language: &str`.
- **No module path changed** for `parser` / `id` / `regen` / `render` /
  `align` / `error::ParseError` / `unit::html_outcomes` — they are
  re-exported at their old locations. Consumers depending on `transync` are
  unaffected by the type moves.
- **New public items:** `regen::top_level_blocks`, the `transync-syntax`
  crate itself (named as `transync::transync_syntax` or as a direct
  dependency), `walk::*`, `outcome::*`.
- **Rendered-HTML byte shapes changed** in the three ways in Part C above
  (loose lists in both panes, loose task items, and the indented-code-block
  fix). A consumer diffing rendered output against a stored golden must
  regenerate it. The alignment map, `out.md`, `ALIGNMENT_SCHEMA_VERSION`
  (`1.2.0`), `VALIDATION_SCHEMA_VERSION` (`2`), `CacheKey`, and both
  `sync.js` copies are **unchanged**.

Follow-ups deliberately left open:

- **OI-0027 facade curation is next** and inherits this split's widened
  surface: the `#[doc(hidden)]` inventory above, the **three** narrowing
  candidates (`htmlseg::balance_fragment`, `outcome::is_translatable`,
  `walk::label_for`), and the **six** rustdoc warnings (five in `htmlseg`, one
  in `render`). Then **0.2.0 release**.
- **OI-0008's remaining four bullet groups** stay open: retry/fallback policy
  split across no-op helpers (R0001-0057) and the concern-mixing in
  `run_pipeline`, the parser walker, unit construction, the OpenAI client,
  and the CLI translate command (R0001-0076..0080). The **parser walker was
  moved, not refactored** — its concern-mixing travelled to
  `transync-syntax` intact.
- **OI-0033** (new): `LineOffsets` counts only `\n`, so a lone-CR source
  desyncs the sourcepos→byte mapping. Pre-existing, orthogonal to this
  split, discovered by the Guard-1 residual probe.
- **Track C proper** — wasm-bindgen entry points, JS integration, and the
  browser demo — remains separate, post-0.2.0 work. This split delivers the
  compile path and the gate that keeps it, nothing more.
- **`GeneratorMeta.version` must not diverge** (M6): keep both crates on
  `version.workspace = true`, or move the stamp behind an explicit constant
  before pinning either crate's version independently.
