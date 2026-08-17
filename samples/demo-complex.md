# Block-level synchronization for translated documents

Most browser-based translation viewers synchronize their two panes by
**scroll percentage**. This works as long as the source and the
translation render at exactly the same height — which they almost never
do. Headings collapse, paragraphs expand, code blocks stay the same
size while their neighboring prose grows or shrinks, and tables can
wrap unpredictably. The result is that, halfway through a long
document, the two panes drift apart and the reader has to scroll one
side manually to find the matching block on the other.

This document explains the alternative — *block-level synchronization*
— from first principles. It also walks through the design decisions a
production implementation has to make, with concrete examples in code
and JSON.

## The core idea

Each *block* in a Markdown document — a heading, a paragraph, a table,
a code fence — is given a stable identifier. The same identifier flows
through the entire pipeline, from the parser's intermediate
representation, through the LLM's batched translation request, into
the regenerated translated Markdown, and finally onto the rendered DOM
elements as `data-sync-id` attributes. The browser never reasons about
scroll percentage; it watches which block is currently visible in one
pane and scrolls the other pane to the element with the matching
`data-sync-id`.

A few invariants fall out of this:

- The **application** owns block IDs, table column counts, list
  topology, code-fence regeneration. The **LLM** is constrained to
  translation judgment alone.
- Tables translate as a whole block by default; a *row-window* fallback
  kicks in only for tables that exceed the per-batch token budget.
- Code blocks translate as content-only units. The application picks a
  fence length safe against the translated body's longest backtick run.
- Validation is layered: schema, ID-set equality, per-kind shape,
  fragment reparse, full document reparse.
- Persistent failures fall back to source content with a
  `fallback_status` flag — never silent corruption.

These rules also describe what a translator *must not* do. The system
prompt declares the source content as data, never as instructions, so
a paragraph saying "ignore previous instructions" stays a paragraph.

## Why scroll-percentage breaks

Consider a Markdown document with three sections, each containing a
short paragraph and one of: a fenced code block, a small table, and a
nested list. After translation into a more verbose target language,
the prose in each section grows by roughly 20% in rendered height
while the code, table, and list stay almost exactly the same height.
A scroll-percentage sync engine will report "you're at 60% of the
source pane" and scroll the target pane to its 60% mark — which is
already past the matching block, because the proportional growth was
unevenly distributed.

Block-level sync sidesteps the problem entirely. The engine asks
"which block is visible in this pane?" and answers with an ID. The
partner pane then asks "which DOM element carries that ID?" and
scrolls there. No proportional math; no drift.

> Anchoring on stable block IDs is the only approach that survives
> translation-induced length changes. A 50-row source table whose
> translated cells wrap to multiple lines will desynchronize a
> percentage-based scroller almost immediately, but it stays in
> lock-step with a block-anchor scroller because both panes still
> emit the same `data-sync-id` for the same logical row.

## What a translation unit looks like

Each translatable block becomes one *translation unit*. A unit carries
the structural ID, the source payload, the surrounding context, and
the per-block constraints the translator must respect.

```json
{
  "unit_id": "p-0042",
  "block_kind": "paragraph",
  "input_mode": "TextFragment",
  "source_payload": "Block-level sync sidesteps the problem entirely.",
  "context": {
    "section_path": [
      { "level": 2, "text": "Why scroll-percentage breaks" }
    ],
    "preceding_block": {
      "kind": "paragraph",
      "summary": "Consider a Markdown document with three sections..."
    },
    "following_block": null,
    "document_title": "Block-level synchronization for translated documents"
  },
  "constraints": {
    "must_preserve_table_columns": null,
    "must_preserve_table_alignment": null,
    "must_preserve_code_fence_info": null,
    "must_preserve_list_topology": false,
    "must_preserve_heading_level": null,
    "forbid_block_breaks_in_inline": false
  },
  "source_hash": 12345678901234567,
  "batch_id": "b-0001"
}
```

The `context` field gives the model just enough surrounding
information to translate idioms accurately without exceeding the
token budget. Two preceding/following snippets are typically enough;
beyond that, the context becomes noise.

## Per-kind constraints

The pipeline enforces a different shape per `block_kind`. The table
below summarizes which constraints apply to which kind. Validation
runs after the model returns; a failed unit is retried once with a
stricter prompt, and on persistent failure it falls back to the
source bytes.

| Block kind     | Required structural invariants                                           | Retry policy on rejection |
|:---------------|:-------------------------------------------------------------------------|:-------------------------:|
| Heading        | Level (`#` count) preserved exactly                                       |             yes           |
| Paragraph      | None at the structural level — accept any inline shape                    |             yes           |
| Table          | Column count + alignment row + data-row count preserved                   |             yes           |
| Code block     | Info string preserved byte-for-byte; fence ≥ longest body backtick run    |             yes           |
| List item      | Depth + ordered/unordered marker + task-checkbox state preserved          |             yes           |
| Blockquote     | Child-kind sequence (paragraphs, lists, code) preserved                   |             yes           |
| Thematic break | Not translated; rendered as-is                                            |              n/a          |
| Image          | Block-level images skip translation; alt text is currently not extracted  |              n/a          |

A few of these checks are stricter than they look. For tables, "column
count" is read from the delimiter row of the *translated* payload, not
from the model's claim about how many columns it returned. For code
blocks, the regenerator computes the longest run of backticks inside
the translated body, then picks a fence one backtick longer; this
holds even when the model returns body content that has more backticks
than the source.

## Retries and fallbacks

The retry / fallback state machine is intentionally bounded:

1. **Per-unit validation retry.** Default `max_per_unit_validation_retries = 2`.
   On rejection, the unit is re-batched alone with the rejection reason
   appended as a stricter prompt.
2. **Oversize split.** Default `max_oversize_split_retries_per_batch = 3`.
   When the provider signals that a batch exceeds its budget, the
   batch is halved and the halves are dispatched as new batches.
3. **Bounded transport retry.** Default `max_per_batch_provider_retries = 1`.
   Network and rate-limit errors retry once before propagating.
4. **Fallback to source.** If a unit's retry budget exhausts, the
   alignment map records `fallback_status: fallback_source` for that
   block, the regenerator splices the source bytes verbatim, and the
   renderer attaches `data-fallback="fallback_source"` so the demo can
   highlight the block visually.

The pipeline never accepts a structurally-invalid translation, and it
never silently corrupts the output. Either the block round-trips, or
the alignment map advertises that it didn't.

***

## Walking through a small example

The simplest end-to-end demonstration is a one-section document with
a heading, three paragraphs, and a small code block. After parsing,
the pipeline emits:

- `h2-0001` — "Walking through a small example"
- `p-0002` — "The simplest end-to-end demonstration is..."
- `p-0003` — "After parsing, the pipeline emits:"
- `c-0004` — the code block below

```rust
/// Translate one block-level Markdown document end-to-end.
///
/// Returns a [`TranslationOutput`] containing the translated Markdown,
/// the alignment map (schema_version "1.0.0"), the annotated source
/// and target HTML fragments, and a per-unit validation report.
pub async fn translate<T>(
    source: &str,
    opts: &TranslateOptions,
    translator: &T,
) -> Result<TranslationOutput, TransyncError>
where
    T: Translator + ?Sized,
{
    let cache = InMemoryCache::new();
    pipeline::run_pipeline(source, opts, translator, &cache).await
}
```

These four units travel as a single batch into the LLM, return as a
batch result, pass per-kind validation, and are spliced back into the
source string at their byte ranges. The alignment map's `blocks` array
gets four rows, each with a `source_block_id`, a `target_block_id`,
the wire-form `block_kind`, and the source/target byte ranges that
the JS sync engine reads.

## Things to verify when you test

- [x] Every block in the source has a matching `data-sync-id` in both
      `source.html` and `target.html`.
- [x] The alignment map's `schema_version` is exactly `"1.0.0"`.
- [x] Reparsing the regenerated translated Markdown via Comrak yields
      the same top-level kind sequence as the source IR.
- [ ] Scrolling either pane drives the other to the matching block
      within one animation frame.
- [ ] A continuous user scroll over a few seconds in one pane does not
      cause the other pane to oscillate. The programmatic-scroll lock
      is set to 120 ms by default; tune it down or up depending on
      your animation budget.
- [ ] Resizing the browser window does not strand the sync state —
      the IntersectionObserver re-fires on resize and the partner
      pane catches up within one frame.

If any of those fail in your local run, the most useful next step is
to load the alignment map JSON and grep for blocks whose
`fallback_status` is anything other than `translated`. A non-empty
fallback list narrows the failure to a specific unit and a specific
validation layer.

## A worked numerical example

Suppose your source document is 1024 lines long, contains 230
sync-relevant blocks (counting headings, paragraphs, table rows,
fenced code blocks, list items, blockquotes), and translates from
English into Korean. With the documented defaults — `target_output_tokens = 8000`,
`max_units_per_batch = 8` — the pipeline emits roughly 30 batches.

| Stage            | Wall-clock (typical) | Notes                                                  |
|:-----------------|---------------------:|:-------------------------------------------------------|
| Parse + IR       |        20 ms         | Comrak, single allocation                              |
| Build batches    |        35 ms         | Includes context-snippet construction                  |
| Translate (live) |   45 s – 2 min       | 30 batches × ~1.5 s round-trip — provider-dominated    |
| Validate         |       100 ms         | Per-batch, per-unit; reparse uses Comrak again         |
| Regenerate       |        45 ms         | Top-level splice, code-fence regeneration              |
| Render           |        80 ms         | Per-block Comrak inline render + HTML escaping         |
| **Total**        | **~ 2 min**          | Translation latency dominates by an order of magnitude |

Two observations from real runs:

1. The provider-roundtrip latency overwhelms everything else. Caching
   matters — a partial-resume run against the same in-memory cache
   skips every cached unit and produces byte-identical output.
2. Validation and regeneration together are typically under 200 ms
   even on documents with hundreds of blocks. Reparsing is cheaper
   than people expect, and the per-block reparse paths reuse the same
   GFM options as the initial parse.

## Caveats and known limitations

The MVP scope deliberately excludes a few things:

- **Sentence-level sub-anchors.** Two paragraphs split into a different
  number of sentences after translation would still align, but only
  at the paragraph boundary; sentences within a paragraph are not
  individually addressable.
- **Live-edit re-anchoring.** The document is assumed static. Editing
  the source after translation invalidates IDs assigned in source
  order; cross-edit ID survival is a non-goal.
- **MDX, raw HTML, YAML frontmatter, math syntax.** Out of scope for
  the current GFM-only pipeline.
- **WASM rendering.** The Rust renderer is structured to compile to
  `wasm32-unknown-unknown` but no `wasm-bindgen` entrypoint ships in
  the MVP.
- **Disk-backed cache.** The MVP cache is process-lifetime in-memory
  only; a future disk-backed implementation would slot in behind the
  same `Cache` trait without changing the pipeline.

If your use case needs any of those, the architecture leaves explicit
extension points — the `Translator` trait crosses the LLM boundary,
the `Cache` trait crosses the persistence boundary, and the
`AlignmentMap` JSON schema is versioned so wire-level evolution is
explicit.

## Final pointer

If you reached the bottom of this page in a browser served by
`transync serve`, scroll either pane back up. Both should follow each
other smoothly, with no oscillation, all the way to the title at the
top. If they don't, the alignment map and the per-block validation
report are the two artifacts to inspect first — together they tell
you exactly which block desynchronized and which validation layer
caught it.
