---
type: Reference
title: Alignment-map JSON schema
description: The durable wire shape of the alignment map — top-level keys, every block-level field, the schema-version policy, and the normative pairing rule.
tags: [alignment-map, wire-format, sync, reference, ADR-0001, DCR-0016]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: align, resource: crates/transync-syntax/src/align.rs }
  - { id: id, resource: crates/transync-syntax/src/id.rs }
  - { id: outcome, resource: crates/transync-syntax/src/outcome.rs }
  - { id: ranges, resource: crates/transync-syntax/src/parser/ranges.rs }
  - { id: render, resource: crates/transync-syntax/src/render.rs }
  - { id: render-attrs, resource: crates/transync-syntax/src/render/attrs.rs }
  - { id: report-phase, resource: crates/transync-core/src/pipeline/report.rs }
  - { id: merge, resource: crates/transync-core/src/pipeline/merge.rs }
  - { id: validate, resource: crates/transync-core/src/validate.rs }
  - { id: core-lib, resource: crates/transync-core/src/lib.rs }
  - { id: wasm-engine, resource: crates/transync-wasm/src/engine.rs }
  - { id: cli-publish, resource: crates/transync-cli/src/translate_cmd/publish.rs }
  - { id: cli-output, resource: crates/transync-cli/src/output.rs }
  - { id: cli-translate, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: drift-test, resource: crates/transync-cli/tests/sync_js_drift.rs }
  - { id: sync-js, resource: web/js/sync.js }
  - { id: wasm-demo-js, resource: web/js/wasm-demo.js }
  - { id: contracts, resource: docs/architecture/contracts.md }
synced_hash: a5c1d14d64e0100f2f41a3aaefd3b228883790019199c22e0dd100d81739a9a1
---

# Alignment-map JSON schema

The alignment map is the artifact the pipeline exists to produce: the
block-by-block correspondence that lets two rendered panes scroll together
without either side re-parsing Markdown. It is a durable wire contract —
breaking changes require a design change record and a `schema_version` bump.

The Rust type is `transync::AlignmentMap`, built by `build_alignment_map` in
`crates/transync-syntax/src/align.rs`. The JSON form appears at these
surfaces:

| Surface | Form |
|---|---|
| `transync::TranslationOutput.alignment_map` | in-memory Rust value |
| `transync translate --map <path>` | pretty-printed JSON file |
| `transync translate --out-dir <dir>` | `alignment.json` in that directory |
| `--html-out` bundle, and the `html/` subdirectory of an `--out-dir` | `alignment.json`, one of the six bundle files; `index.html` fetches it beside itself |
| `transync-wasm`'s `render_pair` / `rebuild` | JSON string across the wasm boundary |

Both CLI paths serialize with `serde_json::to_vec_pretty`, so the file bytes
are the same either way. Flags and exit codes are in
[the CLI reference](../../user/en/cli.md).

## Top-level keys

| Key | JSON type | Value |
|---|---|---|
| `schema_version` | string | `"1.2.0"` at HEAD. See below. |
| `document_id` | string | 16 lowercase hex digits — SipHash-1-3 (keys `0, 0`) over the parsed source text. |
| `source_language` | string | `TranslateOptions::source_language` verbatim, including the literal `"auto"`. Nothing parses, validates or case-folds the label. |
| `target_language` | string | `TranslateOptions::target_language` verbatim, on the same terms. |
| `detected_source_language` | string or `null` | The language a provider envelope reported for this run, or — for a run that dispatched **zero** provider batches, every unit served from cache — the value replayed from the document-metadata store. `null` when neither applies. |
| `generator` | object | `{ "name": "transync", "version": "<crate version>" }`. The version is `transync-syntax`'s own package version, which is the workspace version (`0.3.0` at HEAD). |
| `blocks` | array | One row per top-level source block, in source order. See [Block rows](#block-rows). |
| `validation_summary` | object | Run-level tallies. See [validation_summary](#validation_summary). |

No field carries a serde default, so a Rust consumer deserializing into
`AlignmentMap` fails by name on any absent or mistyped field. Unknown fields
are ignored rather than rejected — that is what lets a build pinned to an
older minor read a newer map.

## `schema_version`

The current wire version is **`1.2.0`**. Its single Rust source of truth is
`ALIGNMENT_SCHEMA_VERSION` in `crates/transync-syntax/src/align.rs`, which is
also on the public surface as `transync::ALIGNMENT_SCHEMA_VERSION` and is what
the wasm module's `schema_version()` export returns.

Version history, all additive: `1.1.0` added the `"skipped"` `block_kind`
value (DCR-0013), `1.2.0` added the `"html"` value (ADR-0018 / DCR-0016).

**Policy.** The field is semver. A major bump is breaking. A minor or patch
bump may add fields and may add enumerated values. Consumers must reject an
unknown major and must accept an unknown minor or patch of the same major,
surfacing it rather than swallowing it.

**Version-string form.** Both JS gates require exactly `major.minor.patch`
with each component 1–9 digits. `"1"`, `"1.foo"`, `"1garbage"` and a component
of 400 digits are all refused as malformed — the digit bound exists so an
overlong component cannot become `Infinity` and read as forward drift forever.

**What each consumer does:**

| Consumer | Its constant | Same version, or older same-major | Same major, newer | Unknown major, or malformed |
|---|---|---|---|---|
| `web/js/sync.js` — `loadAlignment` | `KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 }` | `console.debug`: `transync: alignment map loaded (schema_version=<v>)`; mounts | `console.warn`: `…is newer than this engine (1.2.0); proceeding, but sync may be incomplete`; mounts, and two row rules relax (below) | `console.warn`: `rejecting alignment map with unknown major schema_version=<v>`; `mountSync` returns `null` and the panes stay unwired |
| `web/js/wasm-demo.js` — `alignmentSchemaVerdict` | `KNOWN_SCHEMA = "1.2.0"` | mounts | `console.warn`: `…is newer than this demo (1.2.0); proceeding, but rendering may be incomplete` | fatal panel: `alignment map schema_version=<v> is not major 1 (demo speaks 1.2.0) — refusing to mount` |

The wasm demo runs a second, unrelated check earlier in its boot: it compares
the module's `schema_version()` against its own `KNOWN_SCHEMA` constant and
fails with `schema mismatch: wasm <x> vs demo <y>`. No fetched map takes part
in that comparison.

**Forward drift relaxes exactly two row rules in `sync.js`**, both with a
warning: an unrecognized `sync_role` is treated as a scroll anchor, and a
non-identity `target_block_id` is paired by the source id. Each warning prints
for the first five occurrences and then once more with a suppressed tally.
Either value in an in-band map (same major, not newer than `1.2.0`) is
corruption and the map is refused.

The three JS mirrors of the version are welded to the Rust constant by
`crates/transync-cli/tests/sync_js_drift.rs`
(`sync_js_known_schema_matches_the_rust_alignment_schema_version` covers
`web/js/sync.js` and the CLI-embedded copy;
`wasm_demo_js_known_schema_matches_the_rust_alignment_schema_version` covers
the demo). The same file welds `sync.js`'s `KNOWN_SYNC_ROLES` to the Rust
`SyncRole` enum, and asserts the workspace and CLI-embedded `sync.js` are
byte-identical.

## Block rows

`blocks` holds one row per top-level source block, in source order — including
blocks that never become translation units and blocks that carry no DOM
anchor. Rows for nested content do not exist: the parser is leaf-block, so
list items are themselves top-level blocks and nothing below a block is
addressed.

| Field | JSON type | Value |
|---|---|---|
| `source_block_id` | string | The block's stable id. |
| `target_block_id` | string | Equal to `source_block_id` in every row this project emits — normative for schema 1.x. |
| `block_kind` | string | Kebab-case wire form of the block kind. |
| `source_order` | number (u32) | The block's 0-based index in source order. |
| `target_order` | number (u32) | Equal to `source_order` in every row this project emits. |
| `source_range` | object | `{ "start": <byte>, "end": <byte> }` into the source document. |
| `target_range` | object | `{ "start": <byte>, "end": <byte> }` into the regenerated Markdown. |
| `sync_role` | string | `anchor`, `container`, `child-only` or `non-sync`. |
| `fallback_status` | string | `translated`, `preserved`, `partially_translated` or `fallback_source`. |
| `parent_id` | string or `null` | Always `null`. Reserved. |

Rows appear in ascending `source_order` with no gaps, so the array index and
`source_order` agree for any map the pipeline emitted. The renderer stamps
`source_order` as the `data-order` attribute in **both** panes; nothing on the
render path reads `target_order`.

### `source_block_id`, `target_block_id`, and the pairing rule

Ids have the form `<kind-code>-<NNNN>`: a kind prefix — `h1`…`h6`, `p`, `t`,
`c`, `li`, `q`, `hr`, `img`, `html`, `x` — then a document-wide 1-based
counter, zero-padded to four digits and wider past 9999. The counter runs
across all kinds, so `h1-0001` is followed by `p-0002`. The same string is the
`data-sync-id` attribute on the block's rendered wrapper in both panes.

**Panes pair by identical id, and that is normative for schema 1.x.** Block
ids survive translation (ADR-0001), so the two documents hold the same block
set under the same names; the partner of a block is the element carrying the
*same* `data-sync-id` in the other pane. `build_alignment_map` writes one
`BlockId` into both fields, and `transync-syntax::render` keys every attribute
and both panes' `data-sync-id` off `source_block_id` — nothing on the render
path reads `target_block_id`.

The source/target indirection is **reserved**, in the same sense as
`parent_id` and the `child-only` `sync_role`: it is held for a future revision
in which the two documents' block sets may genuinely diverge, and that
revision will arrive as a schema bump. Until then it expresses nothing, and a
second way of saying "identity" is precisely what it must not become.

`web/js/sync.js` enforces its half. A row whose `target_block_id` is a
non-empty string different from its `source_block_id` describes a pairing the
engine will not perform, so an in-band map carrying one is refused —
`mountSync` returns `null`, the panes stay unwired, and the console message
names both ids. Under forward drift the same row is warned about and paired by
the source id. An absent, `null` or empty `target_block_id` is not a
violation; omitting the field says nothing that contradicts the identity.

### `block_kind`

The kebab-case wire form of the block kind, identical to the `data-block-kind`
DOM attribute: `heading-1`, `heading-2`, `heading-3`, `heading-4`,
`heading-5`, `heading-6`, `paragraph`, `table`, `code-block`, `list-item`,
`blockquote`, `thematic-break`, `image`, `html`, `skipped`.

`skipped` (schema 1.1.0) is a top-level source node the pipeline does not
model as translatable — front matter, a footnote definition, an unsupported
node. It is never sent to a provider, its source bytes are spliced verbatim,
and it renders as an escaped, inert `<pre data-skipped="<label>">` placeholder
in both panes. The label naming the underlying node kind lives on that DOM
attribute, not in `block_kind`.

`html` (schema 1.2.0) is a block-level raw-HTML node, and it is a translatable
kind: text segments are extracted, translated, and spliced back. Whether a
given html block became a unit is decided per block, and shows up in that
row's `fallback_status` and in whether it counts in `validation_summary`.

The JS sync engine reads `[data-sync-id]` only and tolerates `block_kind`
values it does not recognize, which is what makes an added kind a minor bump.

### `sync_role`

| Value | Emitted for | Meaning |
|---|---|---|
| `anchor` | every kind except thematic break and block quote — headings, paragraphs, tables, code blocks, list items, images, html blocks, and `skipped` placeholders | the row anchors scroll in both panes |
| `container` | block quotes | anchors scroll; the wrapper holds other content |
| `child-only` | never | RESERVED for a future nested-anchor scheme (DCR-0007 left the parser leaf-block) |
| `non-sync` | thematic breaks | no DOM anchor exists; the block renders as a bare `<hr>` |

`sync.js` acts on `non-sync` alone — every other known role means "this row
anchors scroll". A role outside the four is refused in an in-band map and
treated as an anchor with a warning under forward drift. The map-versus-DOM
drift check skips `non-sync` rows, since those legitimately have no anchor to
find.

### `fallback_status`

What happened to this block's content. The same value is the `data-fallback`
DOM attribute.

| Value | Meaning |
|---|---|
| `translated` | the provider returned a translation for the unit and it passed validation |
| `preserved` | the content was deliberately kept as-is — either the provider said so for a unit, or the block was never batched at all |
| `partially_translated` | part of the block is translated and part is not — the provider's own verdict for a unit, or the merged verdict of an oversize table split into row windows whose statuses disagreed (DCR-0026) |
| `fallback_source` | the block's bytes are its source bytes: retries were exhausted, the provider opted out, or html text extraction failed |

Blocks that are never translation units — thematic breaks, images, `skipped`
nodes, and html blocks with no extractable text — carry `preserved`, which is
the honest reading: the LLM never saw them. An html block whose extraction
failed carries `fallback_source` and renders as the escaped placeholder. A
*translatable* block for which the pipeline finalized no status at all is
recorded as `fallback_source` rather than silently claimed as translated; that
combination signals an internal pipeline fault.

How a unit reaches each status — the layered validators, the bounded retries,
and the fallback that follows them — is explained in
[the validation, retry and fallback model](../../../explanation/developer/en/validation-retry-fallback-model.md).

### `source_range` and `target_range`

Half-open `[start, end)` byte ranges over UTF-8, serialized as
`{ "start": <n>, "end": <n> }`. They are byte offsets, not character or
UTF-16 indices.

`source_range` indexes the source document **as parsed**, and `document_id`
hashes that same string. That is byte-for-byte the file the caller passed with
one exception: a NUL byte (`U+0000`) is replaced with `U+FFFD` before parsing,
because CommonMark requires the substitution and the parser reports positions
only afterwards. A consumer slicing `source_range` out of the original file
must apply the same replacement first; `out.md` and the rendered panes already
carry the substituted form.

`target_range` indexes the regenerated Markdown — `out.md` — which never
contains a NUL on either side of the document: the source side by
substitution, the target side by refusal, since a `U+0000` in a translated
payload fails the schema validation layer.

An **empty in-bounds range (`0..0`) is legitimate**: `build_alignment_map`
emits it for a block whose regeneration offsets it was not given, and logs one
`tracing::warn` for the document naming up to eight such ids. It is a warning
rather than an error because the empty range is the safe answer — the
alternative indexes the wrong string.

Ranges are not merely advisory to the renderer. `transync-syntax::render`
refuses a range it cannot slice — reversed, past the end, or landing
mid-character — as `RenderError::UnusableRange`, alongside
`RenderError::DuplicateRow` for a repeated `source_block_id` and
`RenderError::UncoveredBlock` for a block of the document that no row names.
Each pane is measured against what it slices: the target pane's ranges are the
rows' `target_range`, measured against the translated Markdown, while the
source pane measures the parsed document's own block ranges against the source
text. In the browser demo the refusal surfaces as a fatal
`render failed: … unusable target byte range …`.

`web/js/wasm-demo.js` additionally checks `target_range` before it builds its
edit model, and only for the rows it will let a user edit (non-`html`,
non-`skipped`, non-`non-sync`): integer offsets, `0 <= start <= end`, `end`
within `out.md`, and both ends on UTF-8 character boundaries. Rows it does not
gate are covered by the renderer's own refusal.

### `parent_id`

Always `null`. Reserved, together with the `child-only` `sync_role`, for a
future nested-anchor scheme. The parser is leaf-block, so no block has a
parent to name and no rendered block carries a `data-parent-id` attribute.

## `validation_summary`

| Field | Type | Value |
|---|---|---|
| `total_units` | number (u32) | Rows whose block became a translation unit. |
| `translated` | number (u32) | Of those, the ones finalized `translated`. |
| `preserved` | number (u32) | …finalized `preserved`. |
| `partially_translated` | number (u32) | …finalized `partially_translated`. |
| `fallback_source` | number (u32) | …finalized `fallback_source`. |
| `retried_units` | number (u32) | Units whose attempt log holds an attempt numbered above 1. |

The tallies count **units, not rows**. Thematic breaks, images, `skipped`
nodes, and html blocks with no unit behind them appear in `blocks` — so a
consumer can render and anchor them — but are excluded from every counter
here. The number of `block_kind: "html"` rows may therefore exceed the html
units counted; the uncounted ones are named on the validation report's warning
channel. The four status counters partition `total_units`.

`retried_units` is the one field the syntax layer cannot compute. The
pipeline's report phase patches it in from the validation report's per-unit
attempt log: a unit counts as retried when it accumulated an attempt with
`attempt_number > 1`. Cache hits, recorded at attempt number 0, do not count.

The CLI reads these counters directly: a run exits `3` when `total_units > 0`
and `fallback_source == total_units`, with the outputs still written. Exit
codes and the `--verbose` tally line are in
[the CLI reference](../../user/en/cli.md); reading a run's counters against
its validation report is covered by
[diagnosing a translation run](../../../how-to/user/en/diagnose-a-translation-run.md).

## What a consumer must be handed

Two different bars apply, because the two reference consumers need different
things from the same file.

**Rust (`render_pair`, or any `serde_json` read into `AlignmentMap`)** —
every field documented above must be present with the correct type. There are
no serde defaults, so a missing `blocks`, a row without a `source_block_id`, a
mistyped offset or an unknown `sync_role` all fail at deserialization, by
name. Unknown extra fields are ignored.

**`web/js/sync.js`** — the row gate requires only what synchronization needs:

- `blocks` is an array;
- every row is an object with a non-empty string `source_block_id`;
- `source_block_id` is unique across the map;
- `target_block_id`, when present as a non-empty string, equals it;
- `sync_role` is a string among the four known values.

Ranges, orders, `block_kind` and `fallback_status` are not policed there. Two
further mount-time refusals sit beside the row gate: a map that describes no
synchronizable block at all — `blocks: []`, or every row `non-sync` — is
refused when the panes carry anchors (empty panes still mount an empty map),
and `mountSync` refuses one element passed as both panes. Every refusal
returns `null` and leaves the panes unwired, with the reason on the console.

The demo bundle that consumes a published map is described in
[serving the demo bundle](../../../how-to/operator/en/serve-the-demo-bundle.md),
and the in-browser renderer that produces one is in
[building the wasm demo](../../../how-to/operator/en/build-the-wasm-demo.md).

## Example

Illustration only — two rows of a real map:

```json
{
  "schema_version": "1.2.0",
  "document_id": "a91f2c0d2e1bbb40",
  "source_language": "en",
  "target_language": "ko",
  "detected_source_language": "en",
  "generator": { "name": "transync", "version": "0.3.0" },
  "blocks": [
    {
      "source_block_id": "h1-0001",
      "target_block_id": "h1-0001",
      "block_kind": "heading-1",
      "source_order": 0,
      "target_order": 0,
      "source_range": { "start": 0, "end": 18 },
      "target_range": { "start": 0, "end": 22 },
      "sync_role": "anchor",
      "fallback_status": "translated",
      "parent_id": null
    },
    {
      "source_block_id": "p-0002",
      "target_block_id": "p-0002",
      "block_kind": "paragraph",
      "source_order": 1,
      "target_order": 1,
      "source_range": { "start": 20, "end": 187 },
      "target_range": { "start": 24, "end": 211 },
      "sync_role": "anchor",
      "fallback_status": "fallback_source",
      "parent_id": null
    }
  ],
  "validation_summary": {
    "total_units": 2,
    "translated": 1,
    "preserved": 0,
    "partially_translated": 0,
    "fallback_source": 1,
    "retried_units": 0
  }
}
```

## Related

- [CLI reference](../../user/en/cli.md) — the flags that write the map and the exit codes that read it.
- [The `Translator` trait](./translator-trait.md) — the provider contract whose results become each row's `fallback_status`.
- [Validation, retry and fallback](../../../explanation/developer/en/validation-retry-fallback-model.md) — how a unit arrives at its status.
- [Architecture overview](../../../explanation/developer/en/architecture-overview.md) — where the map sits between the pipeline and the browser.
