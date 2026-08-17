# HTML-Content Translation — Design Spec

- **Date:** 2026-08-03 (v2 — review-hardened)
- **Status:** **approved** (owner, 2026-08-03 — v2 final sign-off after the two-agent ground + adversarial review and the owner's resolution of every review decision; implementation planning may proceed)
- **Owner decisions recorded in session:** live rendering; blocks + inline guard scope; segment-extraction mechanism (approach A, `lol_html`); fallback keeps the escaped placeholder. **v2 additions (post-review):** render-side fragment auto-balancing; per-segment quality ceiling accepted with prompt mitigation; JSON-in-string wire kept with a recorded native-array escape hatch; zero-segment blocks may render as nothing (with a warning row); `<details>` toggle mirroring across panes.
- **Supersedes (partially):** DCR-0013's "HTML is never translated, always an escaped placeholder" posture. Placeholders remain the *failure* presentation.
- **Amends:** Invariant 7 ("raw HTML in v1 is disabled / escaped / rejected") — raw HTML becomes *translatable, structurally-owned* content whose live rendering rides the shells' existing sanitized mount path. A new ADR records this amendment when the feature lands.
- **Review record:** the 2026-08-03 review verified this spec's code claims against current HEAD, the vendored comrak 0.27 source, and the lol_html documentation. It found one blocker (multi-fragment HTML regions), four spec-text defects, four owner-decision points, and seven grounding corrections — all resolved below. Where v2 contradicts v1, v2 wins.

## 1. Motivation

Real-world documents — GitHub READMEs above all — carry meaningful content in raw HTML: `<details>` sections, `<summary>` labels, HTML tables, centered hero blocks. Today (post DR-2026-07 / DCR-0013) that content is preserved byte-verbatim in `out.md` and rendered in both panes as an escaped `<pre data-skipped>` placeholder: honest, but untranslated and unreadable as content. The 2026-07 design-review fitness verdict named this the largest remaining reader-value gap.

**Fragment reality (v2).** By CommonMark's own rules these flagship patterns are *multi-fragment*: a `<details>` region whose body is processed as Markdown must contain blank lines, and a type-6/7 HTML block ends at the first blank line — so the region parses as an opening HTML fragment (`<details><summary>…</summary>`), ordinary Markdown blocks, and a closing fragment (`</details>`). The same holds for `<div align="center">` heroes. This design therefore treats **unbalanced fragments as the normal case** (§3.4); it never assumes an HTML block is a complete element tree.

Goal: text inside block-level raw HTML is translated; the surrounding markup is preserved **by construction**; successful blocks render live in both panes; the block-ID sync chain extends over them unchanged.

## 2. Owner decisions (settled — do not relitigate)

1. **Live rendering.** Translated (and cleanly preserved) HTML blocks render as real content in the panes via the existing DOMPurify fail-closed mount path. Failed/fallback blocks keep the DCR-0013 escaped placeholder + fallback tint — live rendering is a translated-content privilege.
2. **Scope: blocks + inline guard.** Block-level HTML becomes translatable. Inline raw HTML inside paragraphs gains a tag-identity validation guard (tags must survive verbatim; text may change). **Attribute text (`title`/`alt`/`aria-label`) is NOT translated** — preserved verbatim; a future policy may revisit.
3. **Mechanism: segment extraction (approach A).** The application parses the HTML and sends the LLM only text segments; tags never reach the model. Whole-block round-tripping (approach B) was rejected: reserialization false-rejects, retry burn, output-ceiling pressure. HTML→Markdown round-trip (approach C) rejected as lossy.
4. **Parser dependency: `lol_html`** (Cloudflare streaming rewriter) added to `transync-core`. Chosen for byte-exact preservation of untouched markup, text-handler rewriting as its core use case, leniency on real-world HTML, and expected WASM compatibility (to be verified — §8, §10).
5. **Fragment auto-balancing at render (v2).** Unbalanced fragments are translated normally and live-rendered through render-side auto-balancing (§3.4). `out.md` keeps the true translated fragment bytes. Consequence accepted: an interleaved `<details>` region renders with its body always visible — GitHub's fold is not reproduced in v1.
6. **Per-segment quality ceiling accepted (v2).** Text-node granularity caps translation quality across inline-tag boundaries (word-order reflow for CJK cannot cross segments). Accepted for v1 with the prompt mitigation in §4.1; same-parent segment grouping via opaque placeholder tokens is the recorded fast-follow (§9) if segment-count rejections dominate telemetry.
7. **JSON-in-string wire kept (v2).** The provider schema cannot constrain the inner array; the failure mode (inner-escaping slips → per-kind rejection → verbatim retry → fallback) is accepted, mitigated by an exact output example in the prompt. The designated escape hatch — an additive native-array field in the provider schema — is recorded (§4.1, §9), not built.
8. **Zero-segment blocks may vanish (v2).** Comment-only and `script`/`style`-only blocks live-render as visually empty anchors (DOMPurify strips them at mount — matching GitHub's own rendering). An info-level warning row keeps the disappearance visible in the report.
9. **`<details>` toggle mirroring (v2).** The shells mirror `toggle` state across panes so anchor geometry stays congruent (§5).

## 3. Architecture

### 3.1 Block modeling (parser / id)

- New translatable kind: **`BlockKind::Html`**, emitted by `parser::visit` for `NodeValue::HtmlBlock` nodes. Today those nodes reach `BlockKind::Skipped { label: "html-block" }` via the catch-all arm — implementing this is a **new explicit match arm**, not an edit to an existing one.
- `BlockKind::Html` records the comrak **`block_type`** (CommonMark HTML block types 1–7, from `NodeHtmlBlock`): splice-time normalization is type-conditional (§3.3).
- `BlockKind::Skipped` remains for the other unmodeled kinds (front-matter, footnote definitions, unsupported). Nothing else about Skipped changes.
- ID code: `html` (IDs like `html-0004`), assigned in the same single source-order walk; `assign_block_ids` re-keying covers the new code like any other.
- The block records its raw source bytes (`source_range`) as today; segment extraction happens at unit construction, not parse time.
- HTML blocks nested inside list items or blockquotes are not root-level blocks; they remain part of the parent leaf payload and are covered only by the child-kind topology labels (§9).

### 3.2 Segment extraction (unit construction)

One shared extraction routine, with one pinned `lol_html` `Settings` (strict flag and memory limits fixed), is used by **both** the extraction pass and the splice pass — chunking is deterministic for identical input + settings, and sharing the routine is what guarantees the two passes make identical coalesce/drop decisions. The routine, as an **ordered algorithm**:

1. **Document-level text handler with `TextType` filtering** (not an element-selector handler): captures top-level text outside any element (orphan-fragment prose included); excludes script/stylesheet text by `TextType`; comments, CDATA, and processing instructions never reach text handlers.
2. **Coalesce** the rewriter's arbitrary text chunks per text node using the end-of-text-node signal. (A text node interrupted by a comment legitimately yields multiple segments; entities may be split across chunk boundaries, which is why decoding happens *after* coalescing.)
3. **Decode entities** on the coalesced text via a dedicated decoder with the full named-entity table (`lol_html` provides no decoder; e.g. the `htmlize` family). Decoded text is what the model sees.
4. **Drop whitespace-only segments, judged on the decoded form** (an `&nbsp;`-only text node decodes to whitespace and is dropped — by both passes, same routine).
5. Each surviving segment records its **positional index** and a short **parent-element label** (`summary`, `td`, `p`, `pre`, …) as translation context.

- **Zero-segment blocks** (pure-tag content: badge rows, `<img>` walls, comment-only, `script`/`style`-only, orphan close tags): no translation unit is built. The block is `preserved`, anchored, and live-rendered. This is a **per-block** decision inside a translatable kind — a first (the Image precedent is kind-level) — so `unit::is_translatable`, `has_translatable_blocks`, and `align`'s missing-result accounting are extended **together** to a per-block predicate; otherwise `align` would mark the unit-less block `fallback_source` as an internal error. Zero-segment blocks are **not counted** in `validation_summary` (they are not units — same as Image rows); their alignment rows still carry `block_kind: "html"`. Live rendering may be **visually empty** (decision 8); an info-level warning row is emitted on the `skipped_source_nodes` channel — a new unit-construction-time producer alongside today's parse-time producers.
- **Extraction failure** (the rewriter errors): the block **keeps its `html-` id and `BlockKind::Html`** — parse-time ids cannot be re-keyed to `x-` without breaking `assign_block_ids` idempotency. A degraded flag routes it to the escaped-placeholder presentation (same rendering as Skipped), its alignment row is `block_kind: "html"` with `fallback_status: fallback_source`, it is **not counted** in `validation_summary`, and a warning row records the cause. Degrade, never abort.

### 3.3 Translation unit & splice

- One unit per HTML block (`unit_id` = block id). The unit payload is the ordered decoded segment list; the block's raw bytes remain the cache-identity source (`source_hash` as today).
- On acceptance, the splice pass (same routine/settings as extraction) replaces translated segments back into the original markup **per text node, positionally**. Every byte outside the collected text nodes is preserved verbatim.
- **Identity skip:** a segment whose accepted translation is identical to its decoded source segment is **not replaced** — its source bytes, entity forms included, pass through untouched. `preserved` blocks and echoed segments are therefore byte-identical to the source by construction (and §7's identity-splice test is exact as stated).
- For genuinely translated segments, insertion is text-mode: `<`, `>`, `&` are escaped; named/numeric entity forms are **not** reconstructed (`&nbsp;` → literal U+00A0, `&copy;` → ©, numeric refs → literal chars). This **entity-form drift in `out.md` is accepted** and pinned by test.
- **Blank-line normalization is type-conditional:** applied only to block types 6/7 (where a blank line splits the block under CommonMark), with "blank line" defined per CommonMark — a line containing only spaces/tabs is blank. Type-1 blocks (`<pre>`, `<textarea>`; `script`/`style` never have segments) are **exempt**: their interior blank lines are legal and whitespace-significant.
- `<pre>`/`<textarea>` text **is** extracted, carrying its parent label so the model knows the content is preformatted; the LLM owns the translate/preserve decision (invariant 2), and the identity skip makes preservation byte-exact.
- **Splice-pass failure after acceptance is not a model fault:** it goes directly to fallback (placeholder + `fallback_status`), never into the verbatim-retry loop.
- Regen splices the *translated* block bytes into `out.md` (HTML blocks are no longer byte-verbatim in the output when translation succeeds; fallback blocks still are).
- Batching, retry (verbatim + RetryContext), fallback, output-budget estimation (the serialized segment payload feeds the token estimate), and cache flow are untouched — an Html unit is a normal unit. The one cache-adjacent change is the `VALIDATION_SCHEMA_VERSION` bump (§6, Cache row), riding the existing `CacheKey` field.

### 3.4 Fragment balancing (render path only — v2)

- The live renderer **auto-balances** each fragment: tags opened but not closed within the fragment are closed at the fragment's end; a fragment consisting only of orphan close tags renders as an empty wrapper. **`out.md` is untouched** — balancing exists only on the render path.
- Rationale (review blocker): mounting an unbalanced fragment as-is lets the browser's tree correction consume the sync wrapper's own `</div>` — an unclosed `<div align="center">` would swallow **every subsequent sync anchor** into its wrapper, degrading sync for the document tail to proportional mapping (an invariant-1 violation). Auto-balancing makes that impossible; a Playwright assertion pins that **no sync wrapper is nested inside another after mount**.
- Consequence (decision 5): an interleaved `<details>` region renders as a translated `<summary>` plus ordinary always-visible Markdown blocks — the fold is not reproduced in v1 (§9).

## 4. LLM contract & validation

### 4.1 Wire shape

- `translated_payload` for `html` units is a **string containing a JSON array of strings**, one element per source segment, same order. `output_kind` semantics: `translated` or `preserved` are the acceptable successes; per-segment preservation is expressed by echoing a segment unchanged; `partially_translated` is not offered for html units. **`failed_needs_fallback` remains a legal wire value** and rides the existing Provider-rejection → fallback path. A `preserved` claim is checked byte-for-byte against the unit's **segment-list payload encoding** (the existing preserved == source check).
- The user prompt frames the segments as an indexed list with parent-element labels, under the existing data-framing (source content is data, not instructions), and includes:
  - an **exact output example** (indexed segments in → JSON string array out), and
  - the instruction that **segments sharing a parent element are pieces of one sentence** — translate each so the concatenation reads naturally (decision 6).
- **Trade-off recorded (decision 7):** the provider's structured-output schema validates `translated_payload` only as a string — inner count/string-ness/JSON validity are deferred to per-kind validation, and a systematic inner-escaping failure repeats under verbatim retry until fallback. The rejection-layer breakdown already visible in `ValidationReport` makes the mix measurable; if double-encoding proves a top rejection cause, the designated escape hatch is an **additive native-array field** in the provider schema (a wire + `UnitResult` widening) — recorded, not built.

### 4.2 Validation layers (existing order, new checks)

1. **Schema / ID-set** — unchanged (one layer in code: ID-set equality is enforced inside the schema layer's classifier).
2. **Per-kind (html):** payload parses as a JSON array of strings; element count equals the source segment count; no element may be empty (source segments are non-empty by construction — §3.2 step 4 drops whitespace-only segments). The source-side segment count travels in **`BlockConstraints`** — validators stay comrak-only and never re-run `lol_html`. Violations are retryable rejections.
3. **Post-splice structural check:** the splice rewriter completes cleanly and the spliced block's tag inventory equals the source's (belt-and-braces; true by construction, cheap to verify). A rewriter *error* here is not a model fault → **direct fallback**, no retry (§3.3).
4. **Full-document reparse** — same position and mechanism, but `validate/full_reparse.rs` gains an `html` label arm on **both sides** of its compare: the `BlockKind` side (`label_for`, an exhaustive match) and the reparse side (`NodeValue` match, which today buckets `HtmlBlock` under the `"skipped"` catch-all — left unchanged, every successfully spliced html block would label-mismatch and cascade to fallback). The spliced block must still parse as a single `HtmlBlock` at its position (guaranteed for types 6/7 by splice-time blank-line normalization; type-1 blocks terminate at their close tag).

### 4.3 Inline raw-HTML guard (paragraphs)

- New check in the **inline-protection layer** (`validate/inline.rs`): the ordered sequence of raw inline-HTML tokens (comrak `HtmlInline` literals) in the source payload must equal the translated payload's, **verbatim string equality per token, ordered**. It applies to every unit the inline-protection layer covers (paragraph-family payloads — paragraphs, headings, list items, blockquote payloads), not literal paragraphs only.
- Always on — no profile gate (a model has no legitimate reason to alter a tag). The guard is **hoisted above** `check_inline`'s policy early-return: `preserve_urls == false` with no code pledge must not disable it. Mismatch = retryable `Inline` rejection → verbatim retry → fallback, exactly like link destinations.
- **Known false-reject (accepted, pinned by test):** a model legally moving an inline tag to start a line can make the fragment reparse classify the paragraph remainder as an HTML block, vanishing the `HtmlInline` tokens on the translated side only; verbatim retry usually recovers.

## 5. Rendering, sync, schema

- **Live render:** for `translated`/`preserved` Html blocks, the renderer emits the (spliced or source) HTML — auto-balanced per §3.4 — inside the standard sync wrapper. For this kind the wrapper element is a `<div>`, matching the existing table/code-block/blockquote pattern; it carries the standard attribute set (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`, and `data-parent-id` where applicable). comrak is not involved for this kind. The shells' existing DOMPurify fail-closed sanitize-before-mount is the mount path — same as all pane content (OI-0001 machinery).
- **Fallback / extraction-failure render:** these blocks keep the DCR-0013 escaped `<pre data-skipped="html-block">` placeholder + fallback tint. The legend already explains both presentations.
- **Sync:** one anchor per HTML block (fragment). Geometry comes from the live-rendered container; visually-empty anchors are benign in the shipped engine (a zero-height block cannot win active-block selection, and the DOM-drift check stays quiet because the anchor exists). No sub-anchors inside HTML blocks (YAGNI; revisit only if reading long `<details>` bodies proves it necessary).
- **Toggle mirroring (decision 9):** the shells listen for `toggle` events inside each pane and mirror the `open` attribute onto the counterpart pane's same-`data-sync-id` `<details>` — keeping pane geometry congruent so in-block progress mapping stays meaningful. This is JS interaction ownership, not structure.
- **Alignment map:** new `block_kind: "html"` wire value; `sync_role: anchor`. **Unit-backed blocks are counted in `validation_summary`; zero-segment and extraction-failed blocks are not** (they are not units — the same rule that keeps Image rows uncounted). Schema **1.1.0 → 1.2.0** (additive — verified: the shipped `loadAlignment` accepts a newer minor with a console warning and never consumes `block_kind`), with the full DCR-0013 lockstep drill: `ALIGNMENT_SCHEMA_VERSION` const, `KNOWN_SCHEMA` in both byte-mirrored `sync.js` copies, Playwright forward-drift probe moves to 1.3.0, scenario/CLI pins updated.
- Pane `lang`/`dir` attributes are inherited from the pane (the OI-0032 direction work is orthogonal).

## 6. Errors and edge cases

| Case | Behavior |
|---|---|
| Extraction fails (rewriter error) | Block keeps its `html-` id, renders the escaped placeholder, alignment row `block_kind: "html"` + `fallback_status: fallback_source`, uncounted, warning row; run continues |
| Zero translatable segments | No unit; `preserved`; live-rendered (possibly visually empty — comments/scripts stripped at mount, matching GitHub); info warning row; uncounted |
| Unbalanced fragment (open tag without close / orphan close tag) | Translated normally; render-side auto-balancing (§3.4); `out.md` keeps true bytes; can never swallow sibling sync wrappers |
| Interleaved `<details>` region | Summary translated and rendered; body renders as ordinary always-visible blocks; fold not reproduced (accepted, decision 5) |
| Segment-count / JSON-shape mismatch from model | Per-kind retryable rejection → verbatim retry → fallback placeholder |
| Splice rewriter error after acceptance | Direct fallback — never burns verbatim retries (§3.3) |
| Translated segment contains blank lines | Collapsed at splice for block types 6/7 only, using CommonMark's blank-line definition (whitespace-only lines included); type-1 exempt |
| `<pre>`/`<textarea>` with interior blank lines | Exempt from collapse; identity skip preserves bytes when the model echoes |
| Entities (`&amp;`, `&nbsp;`, `&#8203;`, …) | Decoded for the model (full named-entity table); identity-skipped segments keep source entity forms byte-exactly; genuinely translated segments re-escape only `<`/`>`/`&` — entity-form drift in `out.md` accepted, round-trip pinned by test |
| Giant HTML table | Existing machinery: own batch, output-budget preflight warning, ADR-0017 abort-all applies as everywhere |
| `script`/`style`/comment content | Never extracted (`TextType` filter / comment handlers), never translated, preserved verbatim in `out.md`; may render as nothing (decision 8) |
| Attributes (`alt`, `title`, …) | Preserved verbatim (out of scope by owner decision) |
| Inline tag altered by model | Inline-guard rejection → retry → fallback |
| `<details>` toggled by the user | `toggle` mirrored to the counterpart pane (decision 9) |
| Cache | `source_hash` over raw block bytes as today; `VALIDATION_SCHEMA_VERSION` bumped — justified by the **new segment-list payload semantics** and the **new always-on inline tag guard** on paragraph-family units (html entries cannot pre-exist; the bump is currently costless: the shipped cache is in-memory and hits are re-validated) |

## 7. Testing

- **Parser/id tests:** the new explicit `HtmlBlock` arm emits `BlockKind::Html` with `block_type` recorded; `html-NNNN` id assignment in the source-order walk; `assign_block_ids` re-keying/idempotency over the new code.
- **Extraction unit tests (ordered algorithm):** coalescing across arbitrary chunk splits; an entity split across chunk boundaries; decode-then-drop (an `&nbsp;`-only text node dropped identically by both passes); top-level text outside any element captured; `script`/`style`/comment exclusion; parent labels including `pre`; positional keys; **pass-1 segment count == pass-2 replaceable-slot count** (including the `&nbsp;`-only and split-entity cases); zero-segment path (no unit built, `preserved`, uncounted, info warning row emitted); extraction-failure degrade path (id retained, placeholder, warning, uncounted).
- **Splice tests:** identity skip → spliced output byte-identical to source (entities included — exact as stated); translated-segment entity drift pinned; blank-line collapse for types 6/7 including a whitespace-only line (`"\n \n"`); type-1 `<pre>`-with-blank-lines round-trip untouched.
- **Render tests:** auto-balancing (unclosed `<div>` closed at fragment end; orphan-close fragment renders empty; `out.md` bytes untouched); escaped-placeholder rendering for extraction-failed and fallback blocks.
- **Per-kind validation tests:** bad JSON, wrong count, empty-where-non-empty, all retryable; constraints carried in `BlockConstraints`; post-splice inventory check; splice-error → direct fallback.
- **Inline-guard tests:** dropped/mangled/reordered inline tag rejects through `validate_batch`; text-only change passes; guard still active under `preserve_urls = false` (hoisted above the early-return); **line-initial inline-tag false-reject regression**; paragraphs without inline HTML unaffected.
- **Full-reparse tests:** spliced html blocks label `"html"` on both sides of the compare (not `"skipped"`).
- **New end-to-end scenario** (README-style fixture with the *real* shapes): an interleaved `<details>` region (blank lines + Markdown body + close fragment), an unclosed `<div align="center">` hero, an orphan close-tag block, an HTML table, inline `<kbd>` in prose — through the stub pipeline: `out.md` spliced correctly, alignment rows `html` with the counting rule applied, both panes live-render, fallback path renders placeholder.
- **Playwright:** an HTML block live-renders in both panes (real `<details>` element present, no `<pre data-skipped>` for it) and participates in scroll sync; **no sync wrapper is nested inside another after mount**; toggling a `<details>` in one pane mirrors to the other; schema-drift tests updated for 1.2.0 (probe → 1.3.0).
- **Cache:** the `VALIDATION_SCHEMA_VERSION` bump is pinned (a pre-bump `CacheKey` no longer matches — no pre-feature replay).
- **Lockstep/drift:** existing `sync_js_drift` + docs-index tests; scenario schema pins to 1.2.0.

## 8. Records impact (on landing)

- New **ADR**: HTML content is translatable via app-owned segment extraction; amends invariant 7; records the live-render decision, the fallback-placeholder rule, and render-side fragment auto-balancing.
- New **DCR**: supersedes DCR-0013's html-block posture (placeholder becomes failure-only); schema 1.2.0; records the `VALIDATION_SCHEMA_VERSION` bump and its justification.
- `CLAUDE.md` invariant 7 wording updated; contracts.md §3/§4 rows for the `html` kind; mvp-scope updated.
- Open-issues: none closed by this feature directly. The OI-0028 note ("lol_html is WASM-compatible") is written **only after** a `wasm32-unknown-unknown` check-build of `lol_html` passes — the crate's docs do not state WASM support; verification is the implementation plan's first task (§10).

## 9. Out of scope (explicit)

- Attribute-text translation (`alt`/`title`/`aria-label`).
- Sub-block sync anchors inside HTML blocks.
- Inline raw-HTML *translation* semantics beyond the tag guard (inline tags pass through as today, now verified).
- `<template>`/web-component content: text inside `<template>` is **not extracted** — same bucket as `script`/`style`. Revisit only on demonstrated need.
- **Fold reproduction for interleaved `<details>` regions** (render-side region re-merging) — the body renders always-visible in v1 (decision 5).
- **Same-parent segment grouping** via opaque placeholder tokens — the recorded fast-follow if segment-count rejections dominate telemetry (decision 6).
- **Native-array wire field** for html payloads — the recorded, telemetry-gated escape hatch (decision 7).
- **HTML blocks nested inside list items/blockquotes:** protected only by the coarse child-kind topology labels; tag text there remains model-mutable — an accepted v1 gap.
- Any change to Markdown-kind handling.

## 10. Sequencing

- **Precondition satisfied:** the OI-2026-08 wave (terminology mechanism, provider-neutral prompt-assembly lift, retry fairness, RTL, live smoke) landed on this tree (prompt lift in `763d091` et al.); this v2 is written against that state.
- **First implementation-plan task:** verify `lol_html` builds for `wasm32-unknown-unknown` (before the OI-0028 records note is written), pin the shared `Settings` (strict flag, memory limits) used by both extraction passes, and choose + verify the entity-decoder crate (full named-entity table, WASM-compatible).
- **Next step:** owner sign-off on this v2 → `writing-plans` for the implementation plan.
