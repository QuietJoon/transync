# transync-syntax Crate Split + AST-Direct Renderer — Design Spec

- **Date:** 2026-08-04 (v2 — review-hardened)
- **Status:** **approved** (owner approved all four design sections and pre-authorized implementation after review; v2 folds in the 2026-08-04 two-agent ground + adversarial review — grounding 12/14 holds with 2 immaterial nuances, one blocker and five should-fixes resolved below)
- **Review record:** verified against the tree at `cd8fa16` and the vendored comrak 0.27.0 source. The blocker: §4.1's original universal "format each child" rule was wrong for Table/CodeBlock/Blockquote (CodeBlock is a childless leaf — children-format emits nothing). v2's per-kind emission table replaces it.
- **Resolves:** OI-0028 (WASM render compile path — owner decision 2026-08-03, Option B). **Narrows:** OI-0008 (clears the two renderer items R0001-0069 + R0001-0096; the other four bullet groups stay open).
- **Owner decisions recorded in session (2026-08-04):**
  1. Scope = split + renderer rework + standing wasm gate ONLY. Track C's actual entry point (wasm-bindgen API, JS integration, browser demo) is separate, post-0.2.0 work.
  2. Crate name: **`transync-syntax`** (the owner decision's working name, confirmed).
  3. wasm gate placement: **pre-commit hook (both copies) + `scripts/smoke.sh`**.
  4. Renderer rework approach: **R1 — whole-document AST-direct rendering**, with the `full_reparse` per-list item-count strengthening.
  5. **Path preservation is the default, not an obligation** — module re-exports keep existing paths where that is the cheapest shape, but a better boundary may break a path (each break recorded in the DCR migration list).
  6. **Improvement-driven breaking changes are all allowed** (0.2.0 window; OI-0027 curation follows this work).
- **Out of scope (explicit):** the `Dialect` trait (DCR-0005's deferral condition — a real second dialect supplying constraints — is still unmet; this split creates the crate boundary only); wasm-bindgen entry points and any browser-side WASM integration; OI-0027 facade curation; OI-0008's remaining four bullet groups (retry/fallback helper split, `run_pipeline`/parser-walker/unit-construction/OpenAI-client/CLI concern-mixing).

## 1. Motivation

Track C (in-browser WASM rendering) is a confirmed goal, but `transync-core` cannot compile for `wasm32-unknown-unknown`: it unconditionally inherits tokio with `rt-multi-thread` (a `compile_error!` on wasm32) and pulls tiktoken-rs, neither of which rendering needs. The owner chose a crate split over feature gates because the boundary is compiler-enforced — `cargo check -p transync-syntax --target wasm32-unknown-unknown` becomes a standing gate with no feature-unification trap.

The renderer rework rides the same effort because shipping today's renderer to WASM would export its costs to the browser: every non-bypass block is individually re-parsed by comrak **with the document's entire link-reference-definition pool appended** (O(blocks × |ref_defs|) — a 200-block document with a 5 KB pool re-parses ~2 MB of definitions per pane), and the resulting HTML is post-processed by `strip_outer_wrapper`, a brittle string surgery with a latent multi-block-payload hazard and a dead fallback path.

## 2. Crate topology (S1)

### 2.1 New workspace member

`crates/transync-syntax` — fifth member, added to the explicit `members` list (never a glob, ADR-0003). Manifest follows workspace conventions: header `# TRACE:` comment block, the five inherited `workspace = true` keys, its own `description`, `default = []`-free (no `[features]` at all — a feature there would reintroduce the unification trap Option B exists to avoid).

Dependency partition:

| crate | deps |
|---|---|
| `transync-syntax` | comrak, serde, serde_json, siphasher, lol_html, htmlize, thiserror |
| `transync-core` (after) | transync-syntax, serde, serde_json, thiserror, tracing, async-trait, comrak, toml, futures, tiktoken-rs, tokio |
| `transync` (facade, after) | transync-core, **transync-syntax** (for the `pub use transync_syntax;` direct-naming re-export, §2.3) |

comrak stays in BOTH manifests deliberately: `parser::comrak_options()` and `ranges::byte_range_for` put comrak types in syntax's public signatures, and core's validate/unit modules parse with them — the shared `{ workspace = true }` entry makes a version mismatch a compile error.

### 2.2 What moves

`parser` (+ `parser/ranges`, `parser/refdefs`), `id`, `regen`, `render` (+ `render/attrs`), `align`, **`htmlseg`**, and **`error::ParseError`**:

- `htmlseg` must move: it is consumed on both sides of the incision (`render::balance_fragment`, `regen::splice` on the syntax side; `unit::extract`, `validate::{splice, tag_inventory}` on the core side). It becomes `pub` in syntax but `#[doc(hidden)]` (internal engine, not curated API); core consumes it cross-crate and continues NOT re-exporting it on the facade.
- `ParseError` moves (its four workspace-wide use sites are all parser-adjacent); core's `error.rs` keeps `Parse(#[from] transync_syntax::error::ParseError)` and re-exports the type so `transync::error::ParseError` keeps resolving. `TransyncError` stays in core.
- The `HtmlOutcome` closure moves from `unit.rs` (see §3).
- `Section`/`Document::hierarchy` move with parser (no pipeline consumer; free).

### 2.3 Path strategy

Default: core re-exports the moved **modules** (`pub use transync_syntax::parser;` etc. — module re-exports, not globs), which makes every existing `use crate::parser::…`/`crate::id::…` path in core's ~20 consuming files resolve unchanged, keeps the facade's `pub use transync_core::*;` working untouched, and preserves all verified downstream paths (`transync::align::ALIGNMENT_SCHEMA_VERSION`, `transync::id::BlockId`, `transync::parser::parse`, root `BlockId`/`FallbackStatus` aliases). Name-conflict audit: clean.

Per owner decision 5, this is the default, not a straitjacket: where a preserved path would require contortion, the better shape wins and the break lands in DCR-0017's migration list. The facade additionally gains `pub use transync_syntax;` so consumers can name the base crate directly.

### 2.4 Records this amends

ADR-0003's "Core has no internal dependencies" sentence, ASCII tree, and member list get a dated amendment (`transync-core → transync-syntax` edge; five members). DCR-0005's core-contents enumeration gets a pointer note. The stale "Three Cargo.toml files… Acceptable" bad-tradeoff line updates to five.

## 3. Boundary inversions (S2)

The proposed cut is not currently a DAG cut; three moves dissolve every inversion:

1. **The `HtmlOutcome` closure relocates to syntax** — `HtmlOutcome`, `html_outcomes`, `is_translatable`, `is_translatable_block`, `has_translatable_blocks`, `block_payload` (unit.rs's six items whose entire dependency closure — `Document`, `Block`, `BlockKind::Html`, `htmlseg::extract` — is syntax-side) move into a new `transync-syntax` module (working shape: `src/outcome.rs`). Core's `unit::build_batches`, `pipeline`, and `validate` consume them cross-crate in the correct direction. This one move dissolves four of align's five inversions and all seven test-helper inversions. **Visibility/curation posture (per review):** `HtmlOutcome` + `html_outcomes` are real API — core's `unit` module re-exports them (`pub use transync_syntax::outcome::{HtmlOutcome, html_outcomes};`), preserving the `transync::unit::html_outcomes` path that `transync-openai/tests/live_smoke.rs` and core-internal callers use. The other four (`block_payload`, `is_translatable`, `is_translatable_block`, `has_translatable_blocks` — private/pub(crate) today) become `pub` only because the crate boundary forces it: they get `#[doc(hidden)]` (the htmlseg treatment), and the widened-item list is handed to OI-0027 via DCR-0017.
2. **`regen::regenerate` takes a neutral parameter**: `pub fn regenerate(doc: &Document, accepted: &HashMap<BlockId, String>) -> (String, BlockOffsets)`. Evidence: `collect_translations` reads only `unit_id` + `accepted_payload` (the collected `final_status` was never consumed). Core builds the map in `pipeline::regen_pass`.
3. **`align::build_alignment_map` takes neutral parameters**: statuses as `&HashMap<BlockId, FallbackStatus>` (it read only `unit_id` + `final_status` from `ValidatedBatch`) and languages as `source_language: &str, target_language: &str` (the only two `TranslateOptions` fields it read). Full target signature: `build_alignment_map(doc: &Document, statuses: &HashMap<BlockId, FallbackStatus>, offsets: &BlockOffsets, source_language: &str, target_language: &str, detected_source_language: Option<String>, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> AlignmentMap` — `offsets`/`detected`/`outcomes` carry over unchanged (the outcome type now lives syntax-side).

Also: `regen::top_level_blocks` widens to `pub` (pipeline consumes it); `parser/refdefs`' rustdoc links into `crate::validate::*` become plain text (unresolvable cross-crate).

**Invariant: zero core-directed dev-dependencies in `transync-syntax`.** The seven test helpers inside moving modules currently construct `TranslateOptions`/`ValidatedBatch`/`ValidatedUnit`; under (1)–(3) they all become syntax-local. This is load-bearing: a syntax→core dev-dep cycle would force the wasm gate down to `--lib`, losing test-target coverage.

## 4. Renderer rework — whole-document AST-direct (S3)

### 4.1 Architecture

`render_source`/`render_target` keep their signatures. Internally, `render_fragment` replaces the per-block string closure with **one comrak parse per pane** (source pane parses `doc.source_text`; target pane parses `translated_md`; arena local to the call). The top-level node sequence is zipped against the alignment rows using **one shared normalization helper** (see below). Per row: open the sync wrapper (`attrs::write_attrs` unchanged, `wrapper_element_for` unchanged), emit the node's content per the emission table, close the wrapper.

**Per-kind emission table (v2 — replaces the wrong universal children-format rule):**

| kinds | emission |
|---|---|
| Heading1–6, Paragraph | format the node's **children** (`comrak::format_html(child, …)` per child — a subtree walk seeded at the argument node, so children yield exactly the inner HTML for these strip-kinds) |
| Image | format the **Paragraph node's children** into the `<figure>` wrapper |
| ListItem | format the **`Item`/`TaskItem` node's children** into the `<li>` wrapper; for `TaskItem`, the renderer itself emits the checkbox `<input>` prefix first (comrak writes it in the item's OPEN tag, not in any child — children-format alone would drop it; derive checked-ness from the node's symbol, cross-checked by the row's `task` flag) |
| Table, CodeBlock, Blockquote | format the **whole node** (comrak's own `<table>`/`<pre><code>`/`<blockquote>` element lives INSIDE the transparent `<div>` wrapper, as today). CodeBlock is a childless leaf — children-format would emit nothing; Table's `</tbody>` is written only by the Table exit arm |
| Html, Skipped, ThematicBreak | bypass arms (unchanged presentation, §4.2); node still advances the walk |
| List group tags | written by the renderer from the `List` node's `list_type`/`list.start` (retiring `ordered_list_start`) |

**Assembly discipline (v2):** concatenate child outputs into one buffer, inserting `\n` before a block-level child when the buffer does not already end with one (emulating comrak's `cr()` — a fresh `format_html` call resets `WriteWithLast`, and tight paragraphs end without a trailing newline); trim the final trailing newline before closing the sync wrapper so the one-line-per-block `writeln!` shape survives (scn_08's same-line pin stays valid).

Deleted outright: `strip_outer_wrapper` (all arms + dead fallback), `block_inner_html`, `ordered_list_start`, and the `ref_defs` append (a whole-document parse resolves reference links natively; `doc.ref_defs` remains for `validate::inline` only).

**Single home for the shared walk (v2):** the top-level normalization (consecutive `ListItem` rows sharing an `ast_path` prefix ↔ one `List` node) and the direct-item-count function (**counting `Item` OR `TaskItem` direct children of a top-level `List` only** — nested lists' items excluded) live ONCE in `transync-syntax`; both the renderer's zip and core's `validate::full_reparse` consume that helper cross-crate. Two copies in two crates would drift.

### 4.2 Bypass arms (unchanged presentation, node still counted)

`Html` (own bytes through `balance_fragment` / escaped placeholder on fallback — comrak with `unsafe_ = false` would emit `<!-- raw HTML omitted -->`), `Skipped` (escaped `<pre>`), `ThematicBreak` (bare `<hr>`, non-sync). Their nodes advance the walk so ordering never drifts. `Image` formats the Paragraph node's children into the `<figure>` wrapper (what the old `Image → strip "p"` arm approximated). Lists: one `List` node serves the row-run; items pair `Item` children with `ListItem` rows; parent links stay alive, so tightness (`<p>` suppression) and `<tbody>` decisions are context-faithful — a strict fidelity improvement over fragment reparse.

### 4.3 Why the zip cannot drift, and the two guards

`pipeline::finalize_regen_with_reparse_policy` guarantees that when render runs, either `reparse_full` returned Ok on this exact `md` (top-level label sequence == normalized source sequence) or the cascade escalated to `FallbackAll`, whose output is structurally identical to the source. The one hole: `reparse_full` never counted a `List`'s items.

- **Guard 1 (validation-side, the fix):** `full_reparse` gains a per-list **item-count** check via the shared helper — counts `Item`/`TaskItem` direct children of each reparsed top-level `List` against the normalized entry's collapsed-`ListItem`-row count. **Defense-in-depth framing (v2):** `per_kind::check_list` already rejects naive per-unit splits/merges (topology length), so this check's real residuals are (a) units whose `expected_list_topology` came back `None` (`inspect_list_topology` fails on payloads that don't reparse as an item subtree — `check_list` then no-ops entirely), (b) cross-unit splice-adjacency effects where individually-valid payloads merge/split at regen boundaries without moving the top-level label sequence, and (c) being the invariant the render zip's row↔item pairing actually needs. Validator tightening only: per the const's own bump discipline, NO `VALIDATION_SCHEMA_VERSION` bump (cache hits are re-validated).
- **Guard 2 (render-side, defensive):** if the zip still cannot pair a row with a node (unreachable by construction), that block alone degrades to the old byte-range rendering path (`block_text` + escaped/plain emission per kind) — never a panic, never a dropped anchor.

### 4.4 Accepted byte-shape changes

Both panes become MORE faithful in known ways (v2 — the change is **source-driven**, not translation-driven: `per_kind` already rejects translation-introduced looseness via the `tight` topology field): a source list that is loose (blank-line-separated items) parsed tight per-item under the old fragment reparse and rendered without `<p>` wrappers — the whole-document parse renders it loose in the **source pane AND the target pane**. Reference links resolve identically but without the append hack. Render/scenario/Playwright pins that encode the old shapes get updated in both panes (attribute order and the one-line-per-block shape are preserved — `write_attrs` and the `writeln!` discipline don't change; scn_08's same-line pin and scn13's DOM assertions stay valid).

### 4.5 Performance

Per run: 2 parses (one per pane) instead of `2 × (N_blocks + N_ol_runs)` parses plus the O(N × |ref_defs|) append tax. This is the cost profile that would otherwise ship to the browser under Track C.

## 5. Gates (S4)

- `cargo check -p transync-syntax --target wasm32-unknown-unknown` lands in **both** pre-commit hook copies (`scripts/hooks/pre-commit` — tracked; `.git/hooks/pre-commit` — the active untracked copy on this machine) and in `scripts/smoke.sh`. **Hook guard (v2):** the hook first checks `rustup target list --installed` for `wasm32-unknown-unknown`; if absent it FAILS LOUDLY with the one-line `rustup target add wasm32-unknown-unknown` hint (the gate stays real rather than silently skipping); `smoke.sh` runs the check unconditionally. Verified non-issues: resolver = "2" + syntax having no `[features]` kills the unification trap; wasm artifacts land in their own `target/wasm32-unknown-unknown` subtree of the shared target dir. First implementation task re-runs the dependency canary (per OI-0028's canary-validity rule: manifests float, re-canary on resolver drift and as the split's first gate).
- The existing full gate set (fmt, clippy ×2, workspace tests at `--test-threads=4`, CLI stub suite, Playwright, drift tests) must stay green at every task boundary.
- `.claude/settings.local.json` gains a `Bash(cargo check:*)` allowlist entry (agent-run gates would otherwise prompt).

## 6. Testing

- All existing suites keep running unchanged (paths unaffected: `docs_index_drift`'s fixed depth-2 root join is untouched by a same-depth sibling crate, `cli_smoke`'s cross-crate fixture join survives, scenario fixtures stay in the facade crate per DCR-0005; `transync::unit::html_outcomes` keeps resolving via the §3.1 re-export — live_smoke untouched).
- New: per-list item-count tests **calling `reparse_full` directly with crafted regenerated_md** (a pipeline-driven split/merge payload dies at `per_kind` first and never reaches the new check), plus one pipeline-level test for the None-constraint residual (a source item whose payload defeats `inspect_list_topology`); TaskItem coverage (checkbox emission equivalence; TaskItem lists in the item-count check); renderer equivalence tests pinning the new output shapes (source-pane AND target-pane loose-list `<p>` pins; tight item with nested list — the `cr()` emulation case; table/codeblock/blockquote same-line wrapper close); the defensive-degrade arm covered by a synthetic mismatch test; wasm gate exercised by the hook/smoke line itself.
- Moved modules' inline tests move with them and become syntax-local (no core types).
- `boundary_v02` untouched (it names nothing from the moving set — verified).

## 7. Records impact (on landing)

- **DCR-0017:** the split + renderer rework + neutral signatures + every intentionally broken path (migration list), the wasm gate lines, and the OI-0008 narrowing rationale.
- **ADR-0003 dated amendment:** dependency DAG (`transync-core → transync-syntax`), five members, updated tree.
- **DCR-0005 note:** core-contents enumeration superseded pointer; dialect-trait deferral text UNCHANGED (condition still unmet — restated in DCR-0017).
- **OI-0028 → RESOLVED**; **OI-0008 narrowed** (renderer items cleared; four bullet groups remain, parser-walker explicitly noted as moved-but-not-refactored).
- Docs sweep: module-map, architecture README, repo README, source-of-truth-table (`transync-core::{parser, htmlseg, regen, render}` → `transync-syntax::…`), CLAUDE.md module-split list, Developer_Guide, docs/index links (drift test enforces). **v2 additions:** contracts.md §4/§4a (the renderer's DOM/layout contract — the loose-list shape change touches it); dated mechanism amendments to ADR-0006/ADR-0007/DCR-0007 (their wrapper/strip mechanism descriptions go stale; the decisions survive).

## 8. Sequencing

Precondition satisfied: the HTML-content translation feature landed 2026-08-04 (`3c73b39`). This work → OI-0027 facade curation → 0.2.0 release. Implementation plan (next step: `writing-plans`) is written against the post-HTML tree, ordered so the tree stays green at every task boundary: canary + crate scaffold → module moves with re-exports (compile-stable) → inversion dissolution → renderer rework → full_reparse strengthening → gates → records.
