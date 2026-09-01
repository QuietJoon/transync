---
type: DCR
title: HTML-content translation — segment extraction, always-on inline tag guard, alignment schema 1.2.0
description: Block-level raw HTML becomes the translatable kind BlockKind::Html. A shared lol_html routine extracts decoded text segments, the LLM sees only a JSON array of strings, and a positional splice with identity-skip puts translations back with every non-text byte preserved. Successful blocks live-render (render-side fragment auto-balancing) through the shells' DOMPurify mount; the DCR-0013 escaped placeholder becomes the failure-only presentation. Adds an always-on inline raw-HTML tag guard, bumps VALIDATION_SCHEMA_VERSION to 2 and the alignment schema to 1.2.0, and mirrors <details> toggle state across panes.
tags: [change, project-control, DCR-0016]
status: active
---

# DCR-0016: HTML-content translation via segment extraction

- **Date:** 2026-08-04
- **Source:** HTML-content translation feature wave, implementing the owner-approved design spec `docs/superpowers/specs/2026-08-03-html-content-translation-design.md` (v2, review-hardened, approved 2026-08-03). Sixteen implementation tasks; the decision record is **ADR-0018**.
- **Supersedes (partially):** `archive/DCR-0013-reader-honesty-placeholders-and-refmap.md` — its "raw HTML is never translated, always an escaped placeholder" posture. The escaped `<pre data-skipped="html-block">` placeholder **stays**, but for html blocks it now presents *failure* only (extraction failure or fallback). Everything else DCR-0013 shipped (the other `Skipped` labels, the refmap resolution, the legend) is untouched.
- **Affected ADRs:** **ADR-0018** (new — records this decision and amends invariant 7); `docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md` (**dated amendment 2026-08-04** — raw inline-HTML tags move from LLM-owned inline markup to protected structure; unlike the earlier destination / code-span narrowings the guard has **no policy gate**, because a tag is not a content decision a profile could delegate); `docs/decisions/0009-reject-retry-policy-changes.md` (upheld — per-kind html rejections ride the existing verbatim-retry budget unchanged; splice-*engine* faults deliberately bypass it as non-model faults); `docs/decisions/0015-cache-poison-and-invalid-hit-policy.md` (upheld — the reason the `VALIDATION_SCHEMA_VERSION` bump is costless today); `docs/decisions/0004-comrak-as-gfm-parser.md` (upheld — `lol_html` extracts segments *inside* a block; block boundaries still come from comrak alone); `docs/decisions/0017-batch-terminal-failures-abort-the-run.md` (untouched — a giant HTML table is an ordinary oversize unit).

## What Changed

### Part A — Block modeling and the segment engine

- **New dependencies** in `transync-core`: `lol_html` **2.9.0** (Cloudflare's
  streaming rewriter — byte-exact preservation of untouched markup) and
  `htmlize` **1.1.0** with the `unescape` feature (the full named-entity decode
  table `lol_html` deliberately does not carry). Both were canaried for
  `wasm32-unknown-unknown` **compilation** before adoption (2026-08-03,
  compile-only — nothing was executed), because the OI-0028 base-crate split
  that follows depends on the base dependency set staying WASM-clean.
- **`BlockKind::Html { block_type }`** is emitted by a **new explicit**
  `NodeValue::HtmlBlock` match arm in `parser::visit` — previously those nodes
  fell through the catch-all to `BlockKind::Skipped { label: "html-block" }`.
  The comrak HTML block type (CommonMark types 1–7) is recorded because
  splice-time normalization is type-conditional. ID code `html`
  (`html-0004`); wire form `html`. `BlockKind::Skipped` keeps every other
  unmodeled kind and is otherwise unchanged.
- **`crates/transync-core/src/htmlseg.rs`** (new) is the whole engine, built
  around **one pinned `rewriter_settings`** (encoding, memory ceiling,
  strictness) shared by the extract pass and the splice pass — sharing the
  routine is what guarantees the two passes make identical coalesce/drop
  decisions:
  - `scan` walks the block and returns one record per text node in document
    order (including the whitespace-only nodes `extract` drops), coalescing the
    rewriter's arbitrary chunks per text node **before** decoding entities, so
    an entity cut in half by a write boundary still decodes. A `wanted_text_type`
    filter keeps `Data`/`RCData` and excludes `ScriptData`/`RawText`/`PlainText`;
    comments and CDATA never reach a text handler at all.
  - **`<template>` content is excluded** (spec §9 — same bucket as
    script/style). The text-type filter cannot see it: `lol_html`'s lexer
    delivers template contents as `TextType::Data`, so `scan` carries a
    `template_depth` counter on its open-element stack (incremented when a
    `<template>` start tag registers its end handler, decremented on the same
    pop-by-name check the label stack uses) and marks every text node collected
    inside one `kept: false`. Because both passes share `scan`, the suppressed
    nodes pass through the splice byte-verbatim automatically.
  - Whitespace-only segments are dropped **judged on the decoded form** (an
    `&nbsp;`-only node decodes to whitespace and is dropped by both passes).
    Surviving segments carry a positional index and a parent-element label
    (`summary`, `td`, `pre`, …) as translation context.
  - `splice` replaces translated segments positionally, with an **identity
    skip**: a segment whose accepted translation equals its decoded source is
    not replaced at all, so its bytes and entity forms pass through untouched
    (this is what makes `preserved` and echoed blocks byte-identical to source
    by construction). Genuinely translated segments are inserted in text mode —
    only `<`, `>`, `&` are re-escaped, so entity **form** drift in `out.md` is
    accepted and pinned by test. Interior blank lines are collapsed for block
    types **6/7 only** (CommonMark's definition — a whitespace-only line counts
    as blank); type-1 blocks (`<pre>`, `<textarea>`) are exempt because their
    blank lines are significant.
  - `balance_fragment` (render path only) closes tags opened but never closed
    in a fragment and **drops** orphan close tags; `tag_inventory` returns the
    ordered lowercase tag-name sequence used by the post-splice check. Both ride
    a minimal tag tokenizer that understands comments, script/style raw-text
    state, quoted attribute values, and void elements — explicitly *not* a
    general HTML parser.
- **Per-block outcome, computed once per run.** `unit::html_outcomes` returns
  `HtmlOutcome { Unit, PreservedZeroSegment, ExtractionFailed(String) }` per
  `html` block, and every html-aware stage reads that one map — `build_batches`
  and `has_translatable_blocks` for what becomes a unit, `align` for the row's
  `fallback_status` and whether it counts, the pipeline for the warning rows.
  A block's row, its counters, and its presentation therefore cannot disagree.
  This is the first **per-block** translatability decision inside a translatable
  kind (the Image precedent is kind-level), which is why `is_translatable`,
  `has_translatable_blocks`, and `align`'s missing-result accounting were
  widened together.
- **Counting rule:** unit-backed html blocks are counted in
  `validation_summary`; **zero-segment and extraction-failed blocks are not**
  (they are not units — the same rule that keeps Image rows uncounted). Both
  uncounted outcomes emit a **reader-honesty warning row** on
  `ValidationReport::skipped_source_nodes` — a new unit-construction-time
  producer alongside DCR-0013's parse-time producers — so a block that
  live-renders as nothing, or degrades to a placeholder, is visible in the
  report instead of silently absent.

### Part B — Wire shape, prompt, validation

- **`InputMode::HtmlSegments`** (serialized `html_segments`); the unit payload
  is a **string containing a JSON array of strings**, one element per source
  segment, in order. `HtmlSegmentConstraints` carries `segment_count` +
  `segment_labels` as **prompt-visible** hints and `source_bytes` + `block_type`
  as **validator-only** inputs that are *never* serialized into the prompt.
  `BlockConstraints.html` is how the source-side segment count travels, so
  validators stay comrak-only and never re-run `lol_html`.
- **Prompt.** `llm::prompt` gains a **conditional** html-segment instruction
  (emitted only when the batch contains html units — a batch without them keeps
  the legacy instruction byte-for-byte) plus an **unconditional** inline
  raw-HTML tag instruction. The unconditional line is a deliberate,
  consciously-regenerated change to DCR-0015's byte-identity goldens
  (`user_prompt_first_dispatch.json`, `user_prompt_retry.json`): the guard in
  Part B below is always on, so the instruction that mirrors it must be too.
- **Per-kind `check_html`:** the payload parses as a JSON array of strings, the
  element count equals the source segment count, and no element is empty
  (source segments are non-empty by construction). Violations are **retryable**
  rejections that ride the normal verbatim-retry → fallback path.
- **Layer-3 post-splice check:** the splice rewriter must complete cleanly and
  the spliced block's tag inventory must equal the source's (belt-and-braces —
  true by construction, cheap to verify). A rewriter **error** here is not a
  model fault, so it takes a **DIRECT fallback**: no retry burn and **no cache
  put**, with `rejected_by` left `None`.
- **Fragment reparse returns early for `Html`** — the payload is a JSON segment
  list, not a Markdown fragment, so structure is the layer-3 splice check's job.
  The layer's shared empty-payload guard still runs first; only the comrak
  reparse is skipped, and `expected_label`'s `Html` arm is defensive-unreachable.
  **Full-document reparse** gains an `html` label on
  **both** sides of its compare — the `BlockKind` side and the reparse side —
  without which every successfully spliced block would label-mismatch against
  the `"skipped"` catch-all and cascade to fallback.
- **Always-on inline raw-HTML tag guard** in `validate/inline.rs`: the ordered
  sequence of comrak `HtmlInline` literals in the source payload must equal the
  translated payload's, verbatim per token. It covers every unit the
  inline-protection layer covers (paragraph-family payloads) and is **hoisted
  above** `check_inline`'s policy early-return, so `preserve_urls = false` with
  no code pledge cannot disable it. Mismatch = retryable `Inline` rejection.
  Html units themselves skip the inline layer.
- **`VALIDATION_SCHEMA_VERSION` 1 → 2.** Justification: the new
  `html_segments` payload semantics, the prompt instruction changes, and the
  new always-on tag guard change what "validated" *means* for paragraph-family
  units — it is **not** a validator tightening of an unchanged contract. The
  bump is currently **costless**: the shipped cache is in-memory and hits are
  re-validated on every run (ADR-0015), so it only invalidates within-process
  reuse. It rides the existing `CacheKey` field; no key or `Cache`-trait change.

### Part C — Render, sync, and the schema-1.2.0 lockstep

- **Live render.** For `translated`/`preserved` html blocks the renderer emits
  the (spliced or source) HTML — auto-balanced per `balance_fragment` — inside
  the standard sync wrapper, which for this kind is a **`<div>`** carrying the
  standard attribute set (`data-sync-id`, `data-block-kind="html"`,
  `data-order`, `data-fallback`), matching the table/code-block/blockquote
  pattern. comrak is not involved for this kind. The mount path is the shells'
  existing DOMPurify fail-closed sanitize-before-mount — the same path all pane
  content already takes.
- **Failure render.** Extraction-failed and fallback blocks keep the DCR-0013
  escaped `<pre data-skipped="html-block">` placeholder plus the fallback tint.
  The legend already explains both presentations; no shell copy changed.
- **`<details>` toggle mirroring** (owner decision 9) in both byte-mirrored
  `sync.js` copies: a **capture-phase** `toggle` listener per pane (the event
  does not bubble), mirror **by index** within the matching `data-sync-id`
  anchor, with an equality guard so the mirrored assignment's own event is a
  no-op — no ping-pong. `destroy()` unwires both listeners.
- **Alignment schema 1.1.0 → 1.2.0** (additive `html` `block_kind`), with the
  full DCR-0013 lockstep drill actually performed:
  1. `ALIGNMENT_SCHEMA_VERSION` const in `align.rs`;
  2. `KNOWN_SCHEMA` in **both** byte-mirrored `sync.js` copies (`web/js/sync.js`
     and `crates/transync-cli/web/sync.js`) plus their doc comments;
  3. the Playwright forward-drift probe moved **1.2.0 → 1.3.0** (test g still
     proves a newer minor warns and syncs; test f still proves a newer *major*
     is refused);
  4. the **12** scenario schema pins (`scn_01`–`scn_11`, `scn_14`) and the
     `transync-cli` `cli_smoke` pin updated.

### Part D — Known limits, discovered and verified during implementation

Recorded here so none is mistaken for a bug or silently rediscovered:

- **Unbalanced-fragment reality.** An interleaved `<details>` region renders its
  translated `<summary>` live but its body **always visible** — the fold is not
  reproduced (spec decision 5, ADR-0018). Verified in the browser suite.
- **comrak 0.27 sourcepos quirks** (harmless here — gap-byte splicing and
  render-side balancing absorb them, and the gap bytes still round-trip
  byte-exactly):
  - **Type-2 (comment) blocks** report end-before-start, so `source_range` comes
    out empty and the block becomes a zero-segment "preserved" row *for the
    wrong reason*. The presentation and the report row are still correct.
  - **Type-1 (`<pre>`) ranges exclude the closing `</pre>`**, so a type-1 unit
    payload can itself be an unbalanced fragment. This touches invariant 4's
    "whole fenced block" framing for *HTML* blocks (not for Markdown code
    fences, which are unaffected).
- **Self-closing raw-text/RCDATA start tags count as OPEN in the balancer**
  (corrected 2026-08-04; the first draft of this record claimed `<script/>` was
  deliberately left unbalanced because "fixing it would contradict the
  void/self-closing invariant" — that reasoning was **wrong**). HTML ignores the
  self-closing flag on `script`, `style`, `textarea`, and `title`, and the
  module's own tokenizer already enters raw-text state for `<script/>` /
  `<style/>` regardless of the flag, so honouring the flag in the balancer was
  *inconsistent with the tokenizer*, not a defence of an invariant.
  `<textarea/>` was the sharp case: the browser enters RCDATA and swallows every
  later sibling — the sync wrapper's own `</div>` included — as text, and
  DOMPurify **keeps** `<textarea>`, so the pane tail collapsed into a form
  control. `balance_fragment` now pushes that element set onto the open stack
  even when self-closing and appends the close tag. The void/self-closing
  invariant is unchanged for every other element (pinned by
  `self_closing_non_raw_text_elements_stay_closed`).

  *Refined 2026-09-01 by ticket `2e2453`, recorded in **DCR-0042**: this is an
  HTML-CONTENT rule. Inside `<svg>`/`<math>` those four names are ordinary
  foreign elements — the scanner never enters raw text there — so
  `<svg><script/>x` honours its slash, leaves `x` a sibling, and earns no
  appended `</script>`. At top level the rule above is unchanged: `<textarea/>`
  still earns its `</textarea>`.*
- **Inline guard known false-reject** (accepted, pinned by
  `line_initial_inline_tag_reclassification_false_rejects`): a model that
  legally moves a **type-6** tag such as `<div>` to line-start reclassifies the
  paragraph remainder as an HTML block, so the `HtmlInline` tokens vanish on the
  translated side only and the guard rejects. Verbatim retry usually recovers.
  **Type-7 tags (`<b>`, `<kbd>`, …) cannot interrupt a paragraph** — comrak-
  verified and pinned by test — so the false-reject is confined to the type-6
  set. `<br>` normalization by models (`<br>` ⇄ `<br/>`) is a **live rejection
  source to watch**: the guard is byte-verbatim per token by design.
- **Report-contract caveat.** A splice-check **direct fallback** produces a unit
  whose `final_status` is `fallback_source` while `rejected_by` **and**
  `rejection_reason` are **both `None`** — there is no rejecting validation
  layer, because the fault was the engine's, not the model's. Consumers reading
  the validation report must therefore join `final_status` with the **warnings**
  channel; `rejected_by` alone does not account for every fallback.
- **Spec §7 vs fixture.** SCN-15's hero `<div>` is **closed** in
  `scn-15-html-blocks.md`; unclosed-fragment coverage lives in the `<details>`
  open fragment plus the `render`/`htmlseg` unit tests, not in the scenario
  fixture. The spec's §7 prose describes the intent, the tests describe the
  coverage.

## Why

The 2026-07 design review named untranslated HTML the largest remaining
reader-value gap, and DCR-0013's placeholder was the honest holding pattern —
correct, but it turned a `<details>` section into escaped source text.
Segment extraction is the only mechanism that closes the gap **without**
relaxing invariant 2: because tags never enter the model's output space, a
structural violation is not caught after the fact, it is unrepresentable.
Whole-block round-tripping and HTML→Markdown conversion were both weighed and
rejected in ADR-0018.

The two changes that look incidental are the ones that keep the system honest.
**Render-side balancing** exists because mounting an unbalanced fragment lets
the browser's tree correction eat the sync wrapper's own `</div>` — an unclosed
hero `<div>` would swallow every later anchor and silently degrade the document
tail to proportional mapping, a direct invariant-1 violation; a Playwright
assertion now pins that no sync wrapper is nested inside another after mount.
The **always-on tag guard** exists because inline HTML was the one place where
raw markup still reached the model in a payload it was invited to rewrite, and
a policy gate on a *structural* identity check is a category error — the
policy gates content decisions (ADR-0012), not skeleton integrity.

Finally, everything degrades onto rungs that already existed. Extraction
failure, zero-segment content, model shape faults, and splice-engine faults all
end at a placeholder or a live-but-empty anchor with a recorded status and a
warning row. No new terminal condition was added, so ADR-0017's abort semantics
are untouched.

## Affected Areas

*Paths as of 2026-08-04 (this record's own date), not rewritten. The `transync-syntax` crate split landed later the same day (**DCR-0017**) and moved `htmlseg.rs`, `parser.rs`, `id.rs`, `regen.rs`, `align.rs`, and `render.rs` (+`attrs.rs`) to `crates/transync-syntax/src/`, along with `HtmlOutcome` / `html_outcomes` (from `unit.rs` into `outcome.rs`, still reachable as `transync::unit::html_outcomes`); `lol_html` + `htmlize` moved from `transync-core`'s manifest to `transync-syntax`'s. `unit.rs`, `llm*`, `validate/*`, `batch.rs`, `profile.rs`, and `pipeline.rs` stayed in `transync-core`. Everything this record decided is unaffected; the `render.rs` live-html arm is unchanged in behavior (it was already a comrak-free bypass arm).*

- `Cargo.toml`, `crates/transync-core/Cargo.toml` — `lol_html` 2.9.0, `htmlize` 1.1.0 (`unescape`)
- `crates/transync-core/src/htmlseg.rs` (new) — scan / extract / splice / `balance_fragment` / `tag_inventory` / pinned settings
- `crates/transync-core/src/lib.rs` — `mod htmlseg`
- `crates/transync-core/src/parser.rs` — explicit `HtmlBlock` arm → `BlockKind::Html { block_type }`
- `crates/transync-core/src/id.rs` — `html` id code + re-keying coverage
- `crates/transync-core/src/unit.rs` — `HtmlOutcome`, `html_outcomes`, segment-unit construction, per-block translatability predicate
- `crates/transync-core/src/llm.rs` — `InputMode::HtmlSegments`, `HtmlSegmentConstraints`, `BlockConstraints.html`
- `crates/transync-core/src/llm/prompt.rs` (+ `golden/user_prompt_first_dispatch.json`, `golden/user_prompt_retry.json`) — conditional html instruction, unconditional inline-tag instruction, regenerated goldens
- `crates/transync-core/src/validate.rs`, `validate/per_kind.rs`, `validate/inline.rs`, `validate/fragment_reparse.rs`, `validate/full_reparse.rs` — `check_html`, splice check + direct fallback, always-on tag guard, `Html` reparse-skip, `html` labels on both compare sides, `VALIDATION_SCHEMA_VERSION = 2`
- `crates/transync-core/src/regen.rs` — splice the translated html payload into `out.md` (fallback blocks stay byte-verbatim)
- `crates/transync-core/src/render.rs` — live html render inside a `<div>` wrapper with balancing; placeholder path for failure
- `crates/transync-core/src/align.rs` — `html` `block_kind`, counting rule, `ALIGNMENT_SCHEMA_VERSION = "1.2.0"`
- `crates/transync-core/src/pipeline.rs` — html outcomes threaded, `skipped_source_nodes` warning rows for both uncounted outcomes
- `crates/transync-core/src/batch.rs`, `src/profile.rs` — html payload feeds the existing output-budget estimate; no new knobs
- `web/js/sync.js`, `crates/transync-cli/web/sync.js` — toggle mirroring + `KNOWN_SCHEMA` 1.2.0 (byte mirror preserved)
- `crates/transync-cli/src/translate_cmd.rs`, `crates/transync-cli/tests/cli_smoke.rs` — html outcomes plumbed; schema pin
- `crates/transync/tests/fixtures/scn-15-html-blocks.md` (new), `crates/transync/tests/fixtures/scn-14-full.md` (appended html tail, `html-0015`..`html-0018`)
- `crates/transync/tests/scenarios/scn_15_html_blocks.rs` (new), `scenarios.rs` aggregator, 12 scenario schema pins, `crates/transync/tests/reader_honesty.rs`
- `web/tests/scn13.spec.js` (test **h**), `web/tests/support/harness.js` — browser suite now **8/8**
- Records: `docs/decisions/0018-html-content-translation-via-segment-extraction.md` (new), this DCR (new), `CLAUDE.md` invariant 7 (amended), `docs/decisions/0012-…` (dated 2026-08-04 amendment), `DCR-0013-…` (superseded-in-part banner)
- Contracts / architecture: `docs/architecture/contracts.md` §1 / §3 / §4 / §5, `docs/architecture/README.md` (component table), `docs/architecture/mvp-scope.md` (post-MVP feature wave + accepted limits), `docs/architecture/scenario-matrix.md` (SCN-15 row, block-kind coverage, out-of-scope line), `docs/architecture/rough-schema.md` (`BlockKind` variants), `docs/architecture/source-of-truth-table.md`, `docs/implementation/module-map.md` (`htmlseg` + SCN-15 rows)
- Project state: `docs/project/status.md`, `docs/project/phase-state.yaml`, `docs/project/open-issues.md` (OI-0028 wasm-canary note + OI-0027/OI-0028 sequencing marked), `docs/index.md`, `CHANGELOG.md`

**Deliberately not updated:** `docs/project/implementation-slice-checklists.md` is
`SL-00`..`SL-14` **MVP-scoped by design** — it checklists the original skeleton
slices, not post-MVP feature waves. `SCN-15` coverage is recorded in
`scenario-matrix.md` and `implementation/module-map.md` instead.

### Discriminating tests

Engine (`htmlseg`): `segments_are_collected_in_document_order_with_parent_labels`,
`entity_split_across_two_writes_is_coalesced_before_decoding`,
`entities_are_decoded_after_coalescing`,
`nbsp_only_text_node_is_dropped_on_the_decoded_form`,
`dropped_nodes_agree_between_extract_and_splice_passes`,
`script_style_and_comment_content_is_never_collected`,
`template_content_is_never_collected`,
`template_nested_in_a_div_suppresses_only_its_own_text`,
`template_bearing_block_splices_byte_identical`,
`top_level_text_outside_any_element_is_captured_as_fragment`,
`pre_content_is_extracted_with_pre_label`,
`identity_segments_splice_byte_identical_including_entities`,
`entity_drift_for_translated_segments_is_pinned`,
`whitespace_only_line_counts_as_blank_for_collapse`,
`replacement_lands_on_the_right_node_past_dropped_and_filtered_nodes`,
`rewriter_error_surfaces_as_err`, `zero_segment_block_yields_empty_extraction`.

Balancer / tokenizer: `unclosed_div_is_closed_at_fragment_end`,
`nested_unclosed_tags_close_in_reverse_order`, `orphan_close_tag_is_dropped`,
`balanced_fragment_is_untouched`, `void_elements_do_not_accumulate_open_tags`,
`self_closing_raw_text_elements_still_get_a_close_tag`,
`balanced_raw_text_elements_are_untouched`,
`self_closing_non_raw_text_elements_stay_closed`,
`unquoted_attribute_value_ending_in_slash_is_not_self_closing`,
`a_slash_outside_any_attribute_value_still_self_closes`,
`raw_text_exits_only_on_a_delimited_close_tag`,
`comments_and_script_content_are_not_scanned_for_tags`,
`non_ascii_raw_text_does_not_split_a_char_boundary`,
`tag_inventory_lists_open_and_close_tags_in_order`,
`identity_splice_preserves_tag_inventory`.

Parser / id / unit: `top_level_html_block_becomes_block_kind_html_with_type`,
`interleaved_details_region_parses_as_multiple_fragments`,
`html_block_ids_round_trip_assign_block_ids`,
`skipped_block_id_round_trips_assign_block_ids`,
`html_block_with_text_yields_a_json_segment_unit`,
`zero_segment_html_block_builds_no_unit`,
`has_translatable_blocks_matches_batch_emptiness_for_html`.

Validation: `well_shaped_html_payload_passes`,
`html_payload_that_is_not_json_is_rejected`,
`html_segment_count_change_is_rejected`, `empty_html_segment_is_rejected`,
`clean_html_splice_is_accepted_with_wire_payload`,
`splice_engine_failure_falls_back_directly_without_retry_marker`,
`html_units_skip_the_inline_layer`, `reparse_full_rejects_dropped_html_block`,
`dropped_inline_tag_is_rejected`, `mangled_inline_tag_is_rejected`,
`reordered_inline_tags_are_rejected`, `text_only_change_around_tags_passes`,
`tag_guard_survives_permissive_policy`,
`line_initial_inline_tag_reclassification_false_rejects`.

Prompt: `html_unit_gains_segment_instruction_and_hints`,
`batch_without_html_units_keeps_the_legacy_instruction`, plus DCR-0015's
`golden_user_prompt_first_dispatch` / `golden_user_prompt_retry` re-asserting the
regenerated goldens (the `#[ignore]`d `regen_prompt_goldens` is the regeneration
helper, not an assertion).

Render / regen / align: `preserved_html_block_live_renders_unescaped_and_balanced`,
`unclosed_html_fragment_is_balanced_in_the_pane`,
`orphan_close_html_fragment_renders_empty_in_the_pane`,
`fallback_html_block_renders_the_escaped_placeholder`,
`skipped_block_renders_as_anchored_escaped_placeholder`,
`translated_html_block_is_spliced_into_out_md`,
`fallback_html_block_splices_source_bytes_verbatim`,
`preserved_html_block_round_trips_byte_identical`,
`zero_segment_html_row_is_preserved_anchor_and_uncounted`,
`extraction_failed_html_row_is_fallback_source_and_uncounted`,
`unit_backed_html_block_missing_from_results_is_fallback_and_counted`.

Reader honesty: `zero_segment_html_block_is_reported_preserved_and_uncounted`,
`ordinary_document_has_no_skip_warnings`.

Cache: `pre_bump_validation_schema_version_never_replays` — the
`VALIDATION_SCHEMA_VERSION` `1` → `2` bump is what makes pre-bump entries
unreachable, so the bump is asserted, not assumed.

Lockstep: `sync_js_known_schema_matches_the_rust_alignment_schema_version` —
`KNOWN_SCHEMA` in **both** `sync.js` copies must equal
`transync::align::ALIGNMENT_SCHEMA_VERSION`, so a one-sided schema bump fails
the build instead of turning real drift into a "compatible" read (OI-0024).

SCN-15 (`scn_15_html_blocks.rs`):
`raw_html_block_round_trips_live_rendered_and_anchored`,
`translated_segments_are_spliced_and_markup_survives`,
`failed_html_unit_falls_back_to_escaped_placeholder`,
`html_units_cache_and_replay_without_provider_calls`.

Browser (`web/tests/scn13.spec.js`, 8/8): test **h** —
"html blocks live-render, never nest wrappers, and mirror details toggles";
test **f/g** re-pinned for the 1.2.0 → 1.3.0 probe move.

## Migration / Follow-up

Breaking / wire changes ride the **still-open 0.2.0 window**:

- **`BlockKind::Html { block_type: u8 }`** is a new variant on a non-`#[non_exhaustive]`
  public enum — breaking for any external exhaustive match. A document whose
  raw-HTML blocks previously reported `block_kind: "skipped"` now reports
  `"html"`.
- **`InputMode::HtmlSegments`** is a new variant (wire `html_segments`); a
  `Translator` impl that matches `InputMode` exhaustively must add the arm, and
  one that ignores it simply never sees html payloads it cannot handle (it may
  return `FailedNeedsFallback`, which rides the existing path).
- **`BlockConstraints` gains `html: Option<HtmlSegmentConstraints>`**;
  `HtmlSegmentConstraints` is a new public struct. `source_bytes` and
  `block_type` are validator-only and are never serialized into a prompt.
- **`unit::build_batches` gained a 4th parameter** (the per-run
  `HtmlOutcome` map), and **`unit::html_outcomes` / `unit::HtmlOutcome` are new
  public items**. Breaking for a library caller that batches outside
  `run_pipeline`; `run_pipeline` callers are unaffected. Moot once OI-0027
  curates the facade's public surface.
- **Alignment schema `1.1.0` → `1.2.0`** — additive only (a new `block_kind`
  value). Under the forward-minor policy a 1.1.0-pinned consumer accepts a
  1.2.0 map with a `console.warn` and renders the new value inertly. Both
  `sync.js` copies, the Playwright probe (now 1.3.0), the 12 scenario pins, and
  the CLI smoke pin were updated in lockstep.
- **`VALIDATION_SCHEMA_VERSION` `1` → `2`** — invalidates in-process cache
  entries only (in-memory cache, hits re-validated). `CacheKey`'s shape, the
  `Cache` trait, and `ProviderFingerprint` are **unchanged**.
- **DCR-0015's prompt goldens were regenerated** for the unconditional
  inline-tag instruction. The pins remain in force: any further prompt change
  must again be a conscious regeneration.
- `out.md` is **no longer byte-verbatim for successfully translated html
  blocks** (fallback and extraction-failed blocks still are), and translated
  segments carry the accepted entity-form drift.

Follow-ups deliberately left open (also listed as out-of-scope in the spec):

- **Fold reproduction** for interleaved `<details>` regions (render-side region
  re-merging) — the body renders always-visible today.
- **Same-parent segment grouping** via opaque placeholder tokens — the recorded
  fast-follow if segment-count rejections dominate telemetry.
- **Native-array wire field** for html payloads — the telemetry-gated escape
  hatch for inner-escaping failures; recorded, not built.
- **Attribute-text translation** (`alt` / `title` / `aria-label`) and
  `<template>` / web-component content stay out of scope.
- **HTML blocks nested inside list items or blockquotes** are protected only by
  the coarse child-kind topology labels; tag text there remains model-mutable —
  an accepted v1 gap.
- **`<br>` normalization telemetry.** If models normalizing `<br>` ⇄ `<br/>`
  becomes a top inline-rejection cause, the guard's byte-verbatim comparison is
  where to revisit it.
- **Sub-block sync anchors** inside HTML blocks remain YAGNI; revisit only if
  reading long `<details>` bodies proves it necessary.
- **Next in sequence (unchanged):** OI-0028 Option B crate split + renderer
  rework → OI-0027 facade curation → 0.2.0 release.

## Amendment (2026-08-08) — the tokenizer's half of the raw-text rule, and two balancer edges

*Appended, not a rewrite. The design stands: one `lol_html` Settings source,
segment-array payloads, `balance_fragment` on the render path only,
`tag_inventory` as the post-splice check. Three statements about the minimal
tokenizer/balancer above are now stale, all found by review 0002.*

- **Raw-text/RCDATA state is entered for all four elements, not two**
  (R0002-0020). The 2026-08-04 correction argued from *tokenizer/balancer
  consistency* — and the same argument condemned what it left standing: the
  balancer treated `<textarea/>` as raw-text-open while `scan_tags` still
  entered raw-text state only for `script` and `style`, so `<textarea>` and
  `<title>` content was scanned as markup. The cost was real and twofold. On
  the validation path, `tag_inventory` reported a phantom `/p` for
  `<textarea>Use </p> to close</textarea>` on the source side only — lol_html
  reads RCDATA correctly and the splice escapes `<>&` — so the inventories
  diverged, layer 3 called it an engine fault, and any translated
  `<textarea>`/`<title>` whose content held tag-like bytes fell back to
  source. On the render path the same phantom was DELETED from the pane as an
  orphan close tag, editing what the reader sees. `scan_tags` now enters the
  state for every `RAW_TEXT_ELEMENTS` entry; the self-closing expectations
  recorded above are unchanged (`<textarea/>` still gets its appended close).
- **Colonized (namespaced) tag names are scanned whole** (R0002-0060). The
  name scan stopped at `:`, so `<div:x>` tokenized as a `div` — and the
  balancer appended a `</div>` that, at DOMPurify parse time, pops the sync
  wrapper's own `<div>` and spills the rest of the block outside its anchor.
  That is precisely the failure balancing exists to prevent. A `:` is now part
  of the name after at least one name byte (a leading colon is still not a tag
  name, as HTML's tag-open state requires a letter), so the appended close
  names the element that is actually open.
- **HTML's optional end tags are generated implicitly** (R0002-0061). Every
  non-void open tag used to sit on the stack until an explicit close, so
  `<p>a<p>b` earned two appended `</p>`s and a browser turned the surplus one
  into a phantom empty `<p></p>` — structure in the pane that is in no source
  document. A start tag now pops the optional-end-tag elements it closes
  (`p` by the block-level set, `li`, `dt`/`dd`, `tr`/`td`/`th`, the table
  section elements, `option`/`optgroup`, `rp`/`rt`) before it is pushed. The
  rule is deliberately bounded to elements at the TOP of the stack: HTML's
  real algorithm walks down through non-special elements, and guessing at
  that from a flat stack would close more than a browser does. Under-closing
  costs at most the phantom this removes; over-closing would invent
  structure. Fragments that write their optional end tags out are unaffected —
  the implicit close pops exactly what the explicit one would have.

## Amendment (2026-08-09) — a blank segment is an empty one, and two more tokenizer edges

*Appended, not a rewrite. The design stands. Three statements above are now
stale, all found by review 0003.*

- **`check_html` rejects a WHITESPACE-ONLY element, not only an empty one**
  (R0003-0042). The Part B bullet says "no element is empty (source segments
  are non-empty by construction)"; the parenthetical understates what
  extraction guarantees. `scan` drops a source text node whose *decoded* form
  is `chars().all(char::is_whitespace)` (`kept: false`), so a segment that
  reaches the model always carries visible text — which means a
  whitespace-only source segment cannot exist and a whitespace-only
  *translation* is content loss with nothing legitimate behind it. Testing
  `is_empty` alone left that one character class through: `" "` splices back
  as markup-preserving whitespace, so the layer-3 tag inventory, the fragment
  reparse and the full reparse all see an unchanged structure and the block
  ships as `translated` with its visible text gone. The predicate is now the
  same one extraction uses, and the rejection stays retryable, naming the
  correct answer (echo the source segment).
- **A tag name must START with an ASCII letter** (R0003-0066). The 2026-08-08
  amendment's colon bullet quoted the rule — "HTML's tag-open state requires
  a letter" — while `scan_tags`'s name loop still accepted
  `is_ascii_alphanumeric` at the first byte, so `<1>` and `<2026 rows>`
  tokenized as tags. The cost is R0002-0020's exactly: `balance_fragment`
  appended a `</1>` (and *deleted* an orphan `</1>` from the pane), and
  `tag_inventory` recorded a phantom open for the source that the escaped
  spliced translation never reproduces — so layer 3 called an engine fault
  and dropped a good translation to `fallback_source`. The first name byte is
  now `is_ascii_alphabetic`; the continuation set (alphanumeric, `-`, `:`) is
  unchanged, so `<h1>` and `<x-2>` are still tags and `<:x>` is still text.
- **CDATA sections are a tokenizer state** (R0003-0067). `scan_tags` knew
  comments and raw text but had no `<![CDATA[`, so tag-looking bytes inside an
  SVG/MathML section were inventoried and balanced as real markup — where a
  browser tokenizes none of them. The scanner now skips the section, with the
  terminator HTML actually uses in each context: `]]>` inside foreign content,
  and the first `>` outside it, where the same bytes open a *bogus comment*.
  Foreign content is tracked as a depth count over `<svg>` / `<math>` start
  and end tags. That is an approximation, and a deliberately bounded one for
  the same reason `implicitly_closes` is bounded: a flat scanner cannot know
  the adjusted current node, and counting the two integration-point roots is
  the honest limit. Extraction is unaffected either way — `lol_html` never
  delivers CDATA to a text handler, as Part A already records.

## Amendment (2026-08-23) — Part D's self-closing carve-out was one step short, and two of the tests it names have been renamed

*Appended, not a rewrite. Everything above stands as written, including the
sentence this amendment supersedes — it records what the project believed
between 2026-08-04 and 2026-08-23, which is the point of dating a record.*

Part D says, of the raw-text carve-out it added on 2026-08-04:

> `balance_fragment` now pushes that element set onto the open stack even when
> self-closing and appends the close tag. **The void/self-closing invariant is
> unchanged for every other element** (pinned by
> `self_closing_non_raw_text_elements_stay_closed`).

**The emphasised clause is superseded.** As of ti `490d97` wave 1
(`d146a53`, 2026-08-23) the balancer honours a start tag's self-closing `/` in
exactly two places — inside foreign content, and on the `<svg>`/`<math>` start
tags that enter it. Everywhere else the slash is a parse error the HTML parser
ignores, so a flagged non-void tag **opens** and the author's matching end tag
is its real closer rather than an orphan to delete. The reasoning, the
measurements and the consequences are in **DCR-0032's amendment of the same
date**; they are not repeated here.

### Why this record reached the smaller rule

Part D's argument was already the right one — it is quoted here because it is
worth seeing that the conclusion was contained in the premise:

> HTML ignores the self-closing flag on `script`, `style`, `textarea`, and
> `title`, and the module's own tokenizer already enters raw-text state for
> `<script/>` / `<style/>` regardless of the flag, so honouring the flag in the
> balancer was *inconsistent with the tokenizer*, not a defence of an invariant.

HTML ignores the flag on `div` and `span` too. The 2026-08-04 wave was fixing a
`<textarea/>` incident and generalised only as far as the incident reached —
four names — while the argument it had just made covered every non-void,
non-foreign element. What made the remainder invisible was that nothing was
calling the balancer on adversarial input yet: the defect became reachable when
wave 1 gave `strip_reserved_sync_attrs` its first call site, and a stripped
`<div/data-sync-id="x">` left exactly the `<div/>` spelling Part D had left
unmodelled.

Worth naming, since it is the second occurrence: the paragraph being amended is
*itself* a correction — "corrected 2026-08-04; the first draft of this record
claimed `<script/>` was deliberately left unbalanced because 'fixing it would
contradict the void/self-closing invariant' — that reasoning was **wrong**."
Twice now this record has been wrong about the same flag, in the same
direction, for the same reason: reading the slash the way XML means it. The
generalisable form is not about HTML at all — **a carve-out written to the size
of the incident that prompted it will be re-opened by the next incident**,
because the argument that justified it was always broader than the case.

### Two test names in this record no longer exist

Both were renamed by `d146a53`, and both are named above — in Part D and in the
`### Discriminating tests` list. The names are left in place; this table is the
forwarding address.

| named in this record | now called | why the name had to change |
|---|---|---|
| `self_closing_non_raw_text_elements_stay_closed` | `only_void_and_foreign_tags_are_closed_by_their_own_slash` | its premise **was** the defect: it asserted `<span/>after` gets no appended closer. Measured in headless Chromium, `<span/>after` parses to `<span>after</span>`. |
| `a_slash_outside_any_attribute_value_still_self_closes` | `a_slash_outside_any_attribute_value_sets_the_flag` | the scanner still *sets* the flag there; what changed is that setting it no longer means the element closes itself. |

The surviving tests carry doc comments naming their predecessors, so the code
was never silent about the rename. This record was — which is why the amendment
exists.

### Addendum (2026-08-29) — a third rename, from a later commit than this amendment

*Appended to the 2026-08-23 amendment rather than opened as a fourth one: it is
the same defect class — a test name this record publishes that no longer
resolves — and the table above is where a reader looks. The heading two levels
up says "two of the tests it names have been renamed"; read it as two **as of
2026-08-23**. This one came later, which is why it is not in it.*

`954711a` (2026-08-24, `feat(id)!: BlockKind::Html stops carrying the CommonMark
type, because that was never a kind`) renamed a third name published above, in
the `### Discriminating tests` list under *Parser / id / unit*:

| named in this record | now called | why the name had to change |
|---|---|---|
| `top_level_html_block_becomes_block_kind_html_with_type` | `top_level_html_block_becomes_kind_html_spelled_html_with_its_type` | the old name asserted the kind *carries* the CommonMark type. `954711a` removed the field from `BlockKind::Html`, so the kind no longer has a type to carry; the type is now the block's spelling. The name had to stop claiming a field that is gone. |

The name is left in place above, as the two before it were. `git log -S` over
`crates` is the check, and it returns exactly `954711a` plus the
2026-08-17 restart commit `59ce8df`.
