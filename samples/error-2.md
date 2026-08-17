Here's the summary of how I applied reviews 0004 and 0005 in this session:

## Review 0004 — R0004-0001 (full-reparse mismatch unrecoverable)

**Reviewer's symptom:** real consumer (`resp-translator`) hit a deterministic failure where post-regen full-document reparse rejected the output → `TransyncError::Validation(_)` → whole document failed → retry produced byte-identical output → same dead end with no per-block diagnostic.

**Your routing:** "Fix-with-knob" → apply all three reviewer recommendations.

**What I applied:**

1. **`FullReparseFailure` enum** in `crates/transync/src/lib.rs` with three variants:
   - `Hard` — old behavior (return `Err`)
   - `FallbackPerBlock` — **new default**: identify divergent source block(s), mark as `FallbackSource`, re-regen, re-reparse once
   - `FallbackAll` — drop every translated payload, return source bytes
   Wired in via new `TranslateOptions.full_reparse_failure` field.

2. **Byte-offset attribution** in `validate/full_reparse.rs`: `reparse_full` now takes `&BlockOffsets`, maps each reparsed top-level block to the source block whose target byte range contains it, and names the source blocks owning ≠1 reparsed blocks (over-producers / lost) as `divergent_source_blocks` in a new `ReparseFailure` struct.

3. **Diagnostic `BlockId`s** in failure messages — e.g. `"between source blocks `p-0001` and `h2-0002`"`.

4. **`TransyncError::stable_code() -> &'static str`** in `error.rs` for downstream wire-format mapping.

5. **Helper extraction** in `pipeline.rs`: `finalize_regen_with_reparse_policy` orchestrates the three policies via `regen_pass` and `downgrade_units` helpers; returns `final_validated` so the success path no longer recomputes it.

6. **Tests:** 3 policy tests in `pipeline.rs::reparse_policy_tests`, 1 attribution test + 2 normalizer fixture tests in `validate/full_reparse.rs`. Shared `evil_units` / `evil_accepted` fixture in new `crates/transync/src/test_fixtures.rs`.

7. **DCR-0002** + CHANGELOG entry. Architecture docs (gitignored locally) updated.

## Review 0005 — R0005-0001 (annotated-HTML wrapper offsetParent undocumented)

**Reviewer's symptom:** consumer mounted `annotated_source_html` in a pane without `position: relative` → `block.offsetTop` walked past `<main>` and the pane to `<body>` → partner pane jumped ~800 px on first scroll.

**Your routing:** ACCEPT (no Phase 2 needed — gate determined the fix was contained).

**What I applied — but with one deliberate departure from the reviewer:**

The reviewer's recommendation #2 was to stamp `position: relative` on `<main>` itself. I analyzed this and **did NOT do it**, because consumers who already correctly set `position: relative` on their pane would lose that as the offsetParent (the closer `<main>` would win), and any pane padding would silently shift `offsetTop` — replacing one bug with a subtler one. The honest contract is to document the precondition.

Specifically:

1. **`docs/architecture/contracts.md` §4a** (new subsection) documents the `<main>` wrapper structure and names the consumer-side `position: relative` precondition for `offsetTop`-based scroll math.

2. **`crates/transync/src/render.rs`** module-doc explains the precondition and why `<main>` deliberately stays unstyled.

3. **`web/js/sync.js` + `crates/transync-cli/web/sync.js`**: `mountSync` doc-block names the precondition; inline scroll-math comments cite §4a instead of "see HTML demos". Mirrored byte-identical (drift test enforces).

4. **DCR-0003** + CHANGELOG entry.

## Commits landed

- `0c0289c` Full-reparse: recoverable per-block fallback with FullReparseFailure knob
- `8306a6c` Docs: DCR-0002 + CHANGELOG + archived review for full-reparse recovery
- `ac35889` Renderer + sync.js: name pane position:relative offsetParent precondition
- `327f3fa` Docs: contracts.md §4a + DCR-0003 + CHANGELOG for wrapper offsetParent

All tests green: 17 lib + 13 scenarios + 5 cli_smoke + 1 sync_js_drift.
