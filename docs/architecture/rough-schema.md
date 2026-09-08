# Rough Schemas

Schema outlines for every durable / wire-crossing data type. These are the **rough** shapes — exact field names and Cargo type definitions land in code during Phase 4. Deviations from these shapes require a Design Change Record (DCR).

## 1. `BlockId`

A stable string identifier per sync-relevant block. Shape decided in ADR-0005:

```
<kind>-<NNNN>
```

- `<kind>` ∈ `{h1, h2, h3, h4, h5, h6, p, t, c, li, q, hr, img, title, html, x}` — `html` is a block-level raw HTML block (ADR-0018) and `x` a skipped node (DCR-0013); both are real prefixes, appearing in alignment rows since schema 1.2.0 (1.3.0 today) and in `data-sync-id` anchors. `BlockKind::id_code` is the authority. `title` is an HTML document's `<title>` (ADR-0025) — a real row with `sync_role: non-sync` and no DOM anchor.
- `<NNNN>` is a zero-padded sequential number assigned in source-order traversal of the AST, starting at `0001`. Numbers are not reused within a document.

Examples: `h2-0001`, `p-0042`, `t-0007`, `c-0003`, `li-0019`.

A separate `source_hash: u64` integrity field travels alongside the ID in the IR — it is **not** part of the ID string.

## 2. `BlockKind`

Rust enum mirrored across the wire as a kebab-case string:

```rust
enum BlockKind {
    Heading1, Heading2,        // six unit variants, not one level-carrying variant:
    Heading3, Heading4,        //   the level lives in the variant, and
    Heading5, Heading6,        //   BlockKind::heading_level projects it back out.
                               // wire: "heading-1" .. "heading-6"
    Paragraph,                 // wire: "paragraph"
    Table,                     // wire: "table"
    CodeBlock { info: Option<String>, fenced: bool }, // wire: "code-block"
                               //   `fenced` records how the SOURCE spelled it;
                               //   the wire label is the same either way.
    ListItem { ordered: bool, task: Option<bool> }, // wire: "list-item"
    Blockquote,                // wire: "blockquote"
    ThematicBreak,             // wire: "thematic-break"
    Image,                     // wire: "image" (block-level only)
    Title,                     // wire: "title" — an HTML document's <title>:
                               //   translated, aligned, never anchored (D5)
    Html,                      // wire: "html" — HTML content with NO semantic
                               //   equivalent; the CommonMark block type moved
                               //   to Spelling::Html (ADR-0025)
    Skipped { label: String }, // wire: "skipped" — a top-level node not modeled as
                               //   translatable (DCR-0013). label ∈ {front-matter,
                               //   footnote-definition, unsupported}; "html-block" is
                               //   no longer among them since ADR-0018
}
```

`contracts.md` §3/§4 is authoritative for the alignment-map wire form and the current `schema_version`; this section is the Phase-2 outline of the Rust shape.

Inline kinds (text, emphasis, link, …) do **not** get a `BlockKind` — they are content within blocks, not sync anchors.

## 2a. `Spelling` and `SourceFormat`

The second and third vocabularies, beside `BlockKind` in `transync-syntax::id`
(ADR-0025 D2, ti `490d97` wave 2). `BlockKind` says what a block **is**;
`Spelling` says how the source **wrote** it; `SourceFormat` says which intake
produced the document.

```rust
/// How the source spelled a block. The semantic vocabulary stays on
/// `BlockKind`; this axis is orthogonal (D2, ti 490d97).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Spelling {
    /// GFM syntax. Every consumer that reparses under comrak lives here.
    Markdown,
    /// Raw HTML markup, translated via segment extraction/splice.
    Html {
        /// CommonMark HTML block type (1–7) when the block is an island
        /// inside a *Markdown* document. `None` for every block of an
        /// *HTML* document, where no CommonMark context exists and
        /// blank-line collapse must never run.
        block_type: Option<u8>,
    },
}

/// The plain two-value format label: `Document.format`, per-row
/// `source_format` on the alignment wire, `AlignmentMap.input_format`,
/// and the `TranslateOptions` input-format option.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceFormat { Markdown, Html }
```

Invariant, established at intake: `format == Html` ⇒ every block's spelling is
`Html { block_type: None }`. `format == Markdown` ⇒ spellings are mixed, and
every raw-HTML island carries `Some(t)`.

Per-block spelling is required rather than stylistic: one Markdown document
interleaves HTML-spelled islands with Markdown-spelled blocks, so no
document-level bit can carry that axis. `SourceFormat` **is** exported by the
`transync` facade — tier (a), `transync::SourceFormat`, since ti `490d97`
wave 5 / DCR-0037, alongside `TranslateOptions.input_format`. `Spelling` is
still a tier-(c) engine type, reachable only through a direct
`transync-syntax` dependency (`contracts.md` §0/§1).

## 3. `Document` IR (in-memory only — not serialized)

```
Document {
  source_text: String,         // original input
  format: SourceFormat,        // which intake produced it
  blocks: Vec<Block>,          // flat sequence in source order
  warnings: Vec<String>,       // reader-honesty notes for unmodeled top-level nodes
  ref_defs: String,            // link-reference-definition pool (DCR-0013)
}

Block {
  block_id: BlockId,
  kind: BlockKind,
  spelling: Spelling,          // how the SOURCE wrote it — orthogonal to kind
  source_range: ByteRange,     // (start, end) byte offsets into source_text
  source_hash: u64,            // SipHash13 over the canonical block bytes
  section_path: Vec<BlockId>,  // chain of enclosing headings
  ast_path: AstPath,           // index path into the AST for regeneration
}
```

> **Note (2026-08-05, DCR-0019).** Two fields left this sketch when the parser
> walker was reworked: `Document.hierarchy` (a `Vec<Section>` re-derived on
> every parse and read by nothing) and `Block.parent_id` (provably always
> `None` — the walker is leaf-block and never recurses). The `Section` type
> survives, shape-frozen and with no producer, as the projection target for a
> future hierarchical-alignment revision. **The wire is unaffected:**
> `AlignmentBlock.parent_id` below stays in schema `1.3.0` as a reserved,
> always-`null` field. The same edit corrected two older drifts in this
> sketch: the `Document` IR holds no owning `ast` handle (the arena is local
> to each parse), and it does carry the DCR-0013 `ref_defs` pool.

The IR is not stable across releases — it is internal. Wire-crossing types are the ones below.

## 4. `TranslationUnit` (wire — into the `Translator` trait)

```
TranslationUnit {
  unit_id: BlockId,             // identical to source Block.block_id
  block_kind: BlockKind,
  input_mode: InputMode,        // see §5
  source_payload: String,       // text/markdown sent to the LLM
  context: BlockContext,        // see §6
  constraints: BlockConstraints,// see §7
  source_hash: u64,             // for cache-key + integrity check on the way back
  batch_id: BatchId,            // grouping handle
  retry: Option<RetryContext>,  // v0.2: None on first dispatch; present on re-dispatch (contracts §1)
}

BatchId = String                 // shape "b-NNNN" — assigned by transync-core::batch

RetryContext {                   // v0.2 non-content retry side channel (ADR-0009; contracts §1)
  attempt: u32,                  // 1-based attempt of THIS dispatch (first retry carries 2)
  rejected_by: Option<ValidationLayer>,  // layer that rejected the previous attempt (§12)
  reason: Option<String>,        // truncated diagnostic; carried OUTSIDE source_payload
}
```

## 5. `InputMode`

Tells the provider impl how to interpret `source_payload`:

```
enum InputMode {
  TextFragment,         // paragraph or heading text
  FullTableMarkdown,    // entire GFM table block
  TableRowWindow {      // one header-carrying window of an oversize table (DCR-0026)
    parent_block_id: BlockId,   // the source table this window slices
    window_index: u32,          // 0-based, in source row order
    window_count: u32,          // how many windows the table was split into
  },                            // source_payload is a COMPLETE GFM table:
                                // header + delimiter + a contiguous run of body rows
  FullCodeBlock {          // full fenced block — payload carries the whole ```lang … ``` incl. fences
    language_info: Option<String>,
  },
  ListItemContent,
  BlockquoteContent,
  HtmlSegments,         // block-level raw HTML (ADR-0018 / DCR-0016): source_payload is a
                        // compact JSON ARRAY of decoded text segments, NOT markdown —
                        // markup never reaches the model, and splice-back restores it.
                        // A provider that treats this payload as markdown fails every
                        // html unit into fallback.
}
```

## 6. `BlockContext`

```
BlockContext {
  section_path: Vec<HeadingSnippet>,  // up to 3 nearest enclosing headings, with text
  preceding_block: Option<NeighborSnippet>,
  following_block: Option<NeighborSnippet>,
  document_title: Option<String>,
}

HeadingSnippet  { level: u8, text: String }
NeighborSnippet { kind: BlockKind, summary: String }   // first ~120 chars
```

## 7. `BlockConstraints`

```
BlockConstraints {
  must_preserve_table_columns: Option<u32>,   // for tables
  must_preserve_table_alignment: Option<Vec<TableAlign>>,
  must_preserve_table_row_count: Option<u32>, // a row-window unit carries its OWN body-row count (DCR-0026)
  must_preserve_code_fence_info: Option<String>, // for code blocks
  must_preserve_list_topology: bool,
  must_preserve_heading_level: Option<u8>,
  forbid_block_breaks_in_inline: bool,        // for table cells (advisory; not enforced)
  expected_list_topology: Option<Vec<ListTopologyEntry>>,  // list units; None otherwise (SCN-05)
  expected_blockquote_children: Option<Vec<String>>,       // blockquote child kinds, wire form (SCN-06)
  html: Option<HtmlSegmentConstraints>,                    // html units; None otherwise (SCN-15)
}

HtmlSegmentConstraints {        // structural facts of one Html unit (ADR-0018)
  segment_count: u32,           // prompt-visible hint; also the per-kind count check
  segment_labels: Vec<String>,  // prompt-visible hint: parent tag per segment
  source_bytes: String,         // splice-check input — NEVER serialized into the prompt
  block_type: u8,               // 0..7. 1..7 = the CommonMark HTML block type of a raw-HTML
                                // island inside a Markdown document; 0 = a block of an HTML
                                // document, where no CommonMark type applies.
                                // A RECORD of what the source was, NOT the switch the splice
                                // reads: blank-line policy is derived from the block's spelling
                                // (Spelling::blank_line_policy_for). 0 reaches the same answer
                                // via BlankLinePolicy::from_commonmark_html_block_type, since
                                // anything outside 6|7 keeps its blank lines.
                                // NEVER serialized into the prompt.
}

ListTopologyEntry {           // one list item's structural fingerprint (SCN-05)
  depth: u32,                 // comparison fingerprint (outermost list = 1), not a display level
                              // exact at any nesting — no ceiling (widened from u8 in the 0.3.0
                              // surface window; see contracts.md §0)
  ordered: bool,
  task: Option<bool>,
  child_kinds: Vec<String>,   // wire-form kinds of the item's direct block children, in order
  start: Option<u32>,         // owning ordered list's start ordinal; None for unordered
  delimiter: Option<ListDelimiter>, // owning ordered list's item delimiter (. vs )); None for unordered
  tight: Option<bool>,        // owning list's tightness; Some for every list item
}
```

## 8. `TranslationBatch` and `TranslationBatchResult` (wire — `Translator` trait input/output)

```
TranslationBatch {
  batch_id: BatchId,
  units: Vec<TranslationUnit>,
  source_language: String,       // "auto" allowed
  target_language: String,
  glossary: Vec<GlossaryEntry>,
  profile: ProfileMetadata,
}

TranslationBatchResult {
  batch_id: BatchId,
  detected_source_language: Option<String>,  // an envelope observation; the run keeps the earliest qualifying one. A fully-cached run makes no provider call, so it replays the detection from the Cache's document-level record instead (DCR-0028)
  units: Vec<UnitResult>,
}

UnitResult {
  unit_id: BlockId,
  output_kind: OutputKind,
  translated_payload: String,    // empty if output_kind = FailedNeedsFallback
  warnings: Vec<String>,         // optional model diagnostic; never inserted into Markdown
}

enum OutputKind {
  Translated,
  Preserved,             // model deliberately kept source content
  PartiallyTranslated,
  FailedNeedsFallback,
}
```

## 9. `GlossaryEntry`

```
GlossaryEntry {
  source_term: String,
  target_term: String,
  note: Option<String>,
  scope: GlossaryScope,   // GlobalAcrossDocument | ConditionalOnSection
  sections: Vec<String>,  // heading-text selectors a ConditionalOnSection entry applies to
                          // (DCR-0027), matched trimmed + case-folded against the whole
                          // heading stack. REQUIRED non-empty for that scope — an entry
                          // whose list is empty after normalization is dropped with a
                          // warning; a non-empty list on a global entry is cleared with one.
}
```

## 10. `ProfileMetadata`

```
ProfileMetadata {
  slug: String,           // "default", "technical-docs-ko", ...
  version: String,        // opaque non-empty identifier; any change invalidates cache
  prompt_body: String,    // the compiled system prompt (after profile + glossary substitution)
  prompt_html: Option<String>,      // the raw [system].prompt_html template an --input-format html run compiles instead of prompt (ti 490d97); None = key absent
  glossary: Vec<GlossaryEntry>,     // structured entries, also rendered into prompt_body
  constraints: ProfileConstraints,  // typed [constraints] section (OI-0003)
  batching: ProfileBatching,        // typed [batching] section (OI-0003)
  render: ProfileRender,            // typed [render] section (OI-0032)
  auto_glossary: Option<bool>,      // top-level TOML key (OI-0026); None = unset, so the
                                    // precedence is flag > profile > built-in default
  load_warnings: Vec<String>,       // unknown sections/keys, unsupported values (contracts §2)
}

ProfileConstraints {
  preserve_code_identifiers: Option<bool>,
  preserve_urls: Option<bool>,
  default_table_strategy: Option<String>,  // "whole-block" | "row-window-first" (both effective, DCR-0026;
                                           // "row-window-first" is the shipped default profile's value, and
                                           // unset/unrecognized resolves to "whole-block")
}

ProfileBatching {
  target_output_tokens: Option<u32>,  // shipped as the provider output ceiling (contracts §7)
  target_input_tokens_per_batch: Option<u32>, // soft per-batch INPUT cap
  max_units_per_batch: Option<u32>,
  output_expansion_factor: Option<f64>,       // output-to-source token expansion used to
                                              // size the response budget when packing
}

ProfileRender {
  target_direction: Option<String>,   // "auto" (default) | "ltr" | "rtl" — a presentation
                                      // hint for the bundle emitters only: never rendered
                                      // into prompt_body, never part of cache identity
}
```

## 11. `AlignmentMap` (durable JSON — schema_version 1.3.0)

The on-disk JSON shape. The full schema is in `contracts.md` §3.

```
AlignmentMap {
  schema_version: "1.3.0",
  document_id: String,             // hex SipHash13 of full source
  source_language: String,
  target_language: String,
  detected_source_language: Option<String>,
  input_format: SourceFormat,      // 1.3.0: which intake produced the document; serde default, so a pre-1.3.0 map reads as markdown
  generator: GeneratorMeta,        // { name, version }
  blocks: Vec<AlignmentBlock>,
  validation_summary: ValidationSummary,
}

AlignmentBlock {
  source_block_id: BlockId,
  target_block_id: BlockId,        // identical to source for static documents
  block_kind: String,              // wire form of BlockKind
  source_order: u32,
  target_order: u32,
  source_range: ByteRange,         // bytes into source MD
  target_range: ByteRange,         // bytes into translated MD
  sync_role: SyncRole,             // Anchor | Container | ChildOnly | NonSync
  fallback_status: FallbackStatus, // Translated | Preserved | PartiallyTranslated | FallbackSource
  parent_id: Option<BlockId>,
  source_format: Option<SourceFormat>, // 1.3.0: how the source spelled this block; skip-serialized when None
}

ValidationSummary {
  total_units: u32,
  translated: u32,
  preserved: u32,
  partially_translated: u32,
  fallback_source: u32,
  retried_units: u32,
}
```

## 12. `ValidationReport` (in-memory — returned to caller, and a durable artifact)

Also published as the root object of `validation-report.json`; the durable
shape and its compatibility rules are `contracts.md` §3a.

```
ValidationReport {
  schema_version: String,                // v0.2: the report artifact's own semver (contracts.md §3a) — NOT the CacheKey axis in §14
  per_unit: Vec<UnitValidationRecord>,   // document (source) order
  total_retries: u32,                    // validation-rejection retries of a unit's own content
  total_fallbacks: u32,
  full_reparse_fallbacks: Vec<BlockId>,  // units the post-regen full-doc reparse downgraded (DCR-0004 cascade); document order
  provider_retries: u32,                 // transient provider-transport retries (network / rate-limit)
  batch_schema_faults: u32,              // v0.2: rounds charged to the per-batch schema budget (OI-0031)
  skipped_source_nodes: Vec<String>,     // top-level nodes parsed but not modeled as sync anchors
  output_budget_warnings: Vec<OutputBudgetWarning>, // v0.2: units estimated over the output ceiling (DCR-0012)
  auto_glossary: Option<AutoGlossaryReport>,        // v0.2: extraction preflight outcome; absent when off (OI-0026)
}

UnitValidationRecord {
  unit_id: BlockId,
  attempts: Vec<AttemptOutcome>,
  final_status: FallbackStatus,
  warnings: Vec<String>,                 // provider warnings on the accepted attempt
}

AttemptOutcome {
  attempt_number: u32,                   // 0 = the cache-hit re-validation row (ADR-0015), charges no budget
  rejected_by: Option<ValidationLayer>,  // None = accepted
  rejection_reason: Option<String>,
  batch_fault: bool,                     // v0.2: charged to the per-batch schema budget, not the unit's (OI-0031); skip-serialized when false
}

enum ValidationLayer {
  // IdSet + FullReparse are reserved: ID-set equality is enforced inside Schema, and the full-document
  // reparse reports through ValidationReport.full_reparse_fallbacks, not through per-attempt outcomes.
  Schema, IdSet, PerKindShape, FragmentReparse, Inline, FullReparse, Provider,
}
```

## 13. `Profile` TOML on-disk

See `contracts.md` §2 for the file format. The schema is consumer-editable; bumps to the `version` field invalidate cache entries.

## 14. Cache key

```
CacheKey {
  provider_fingerprint: ProviderFingerprint, // v0.2: producing translator's identity (R0008-0002)
  validation_schema_version: u32,            // v0.2: core prompt/validation contract generation
  source_hash: u64,
  source_lang: String,
  target_lang: String,
  profile_version: String,
  profile_prompt_hash: u64,   // hash of the rendered system prompt
  glossary_hash: u64,         // hash of the raw glossary fields
  model_id: String,
  block_kind: String,         // wire form
  input_mode: String,         // v0.4: wire form; `block_kind` implies it for every kind but table (ti 5f7942)
  context_hash: u64,          // provider-visible context hints (R0008-0001)
  instruction_hash: u64,      // v0.3: the instruction the unit's batch assembled (R0001-0005)
}
```

Serialized since v0.4.0 — `DiskCache` transports the whole key as a plain JSON object over these field names (DCR-0028 §2), which is why the field set is exhaustive by policy.

Identity rule (owner decision 2026-08-06, ti c02f69): the three namespace axes aside (`provider_fingerprint`, `model_id`, `validation_schema_version`), a field is an axis iff it changed the prompt bytes the model saw for that unit. `context_hash` covers the unit's own hints; `block_kind` + `input_mode` cover the two labels the prompt spells beside the payload; `instruction_hash` covers the one thing its co-batched peers can move — the assembled instruction. `contracts.md` §5a carries the full statement, the two deliberate exclusions (`BlockId`, the retry hint), and the batch-not-round resolution.

`Cache` trait v2 (v0.2, EXT-2026-07 P1-4): `get` / `put` / `evict`, all returning `Result<_, CacheError>` (the pipeline degrades — miss / not-persisted / stale-entry-may-replay — never aborts on a cache error). `evict` removes exactly one key (absent key = `Ok(())`) and is the mechanism behind the §5a document-level disqualification eviction. `validation_schema_version` tracks `validate::VALIDATION_SCHEMA_VERSION`; a bump orphans (never corrupts) old entries because it changes the key.

Since v0.4.0 the trait also carries two **document-scoped** record families, four defaulted methods in all, so an implementation that ignores them keeps its pre-0.4.0 behavior: `get_document_meta` / `put_document_meta` (`DocumentMetaKey` → `DocumentMeta`, the detected source language a fully-cache-hit run would otherwise lose — DCR-0028) and `get_glossary_extraction` / `put_glossary_extraction` (`GlossaryExtractionKey` → `GlossaryExtraction`, the auto-glossary harvest, so a warm run stops re-paying the one preflight call no unit entry can elide — ti `dca5bf`). Neither key is a unit key: the metadata key is deliberately narrower than `CacheKey` (namespace axes, language labels, whole-document hash), and the extraction key is the namespace axes plus one digest of the assembled extraction prompt's bytes. `contracts.md` §5a carries both identity arguments.
