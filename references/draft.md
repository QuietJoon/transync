# Design & Requirements Document

## Markdown Translation Alignment and Synchronized Dual-Pane Rendering

**Version:** Draft 1.0
**Target stack:** Rust for Markdown decomposition, translation orchestration, validation, and Markdown regeneration; vanilla JavaScript for rendering coordination and scroll synchronization; WASM optional.
**Primary output:** Regenerated translated Markdown plus an alignment map that allows source and translation panes to synchronize by semantic block ID.

---

## 1. Executive Summary

The system shall translate a Markdown document while preserving a reliable block-level correspondence between the source Markdown and the translated Markdown. The primary synchronization unit shall be a **semantic Markdown block**, not an absolute scroll percentage.

The design should not rely on whole-document translation, because whole-document translation makes it difficult to retain stable alignment metadata. It should also not rely on naive row/cell-level translation for tables, because cell-level translation can lose contextual meaning and increase token overhead.

The proposed architecture is:

> **Parse Markdown in Rust → create block-level translation units → preserve stable source IDs → pass context-rich units to the LLM → validate translated Markdown fragments → regenerate full Markdown → render both documents with shared sync anchors → synchronize panes in JavaScript by active block ID.**

The LLM may decide whether a given piece of text should be translated, preserved, partially translated, or left unchanged. This applies especially to code block comments, identifiers, literals, table values, acronyms, proper nouns, and technical terms. However, the application must still enforce structural constraints: IDs, Markdown block envelopes, table shape, list topology, and code block boundaries must remain valid.

GitHub Flavored Markdown should be treated as the supported dialect, with GFM tables as a mandatory feature. GFM models a document as a sequence of block-level structures such as paragraphs, block quotes, lists, headings, rules, and code blocks, with inline content inside certain blocks; this supports an AST/block-based processing model rather than a raw line-splitting model. ([GitHub][1])

---

## 2. Goals

### 2.1 Product Goals

The system shall:

| ID | Goal                                                                                                                           |
| -- | ------------------------------------------------------------------------------------------------------------------------------ |
| G1 | Translate Markdown documents while preserving enough structural correspondence for synchronized source/translation viewing.    |
| G2 | Regenerate a valid translated Markdown document, not only HTML.                                                                |
| G3 | Support GFM tables.                                                                                                            |
| G4 | Allow the LLM to decide which content should be translated or preserved based on context.                                      |
| G5 | Treat code blocks as whole contextual units and allow the LLM to decide whether comments or docstrings should be translated.   |
| G6 | Synchronize source and translation panes by semantic block ID rather than scroll ratio.                                        |
| G7 | Support partial retry and fallback when translation or validation fails.                                                       |
| G8 | Keep implementation practical in Rust and vanilla JavaScript, with WASM available if parser/rendering consistency requires it. |

### 2.2 Non-Goals

| ID  | Non-goal                                                                                                                                                               |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| NG1 | Byte-for-byte preservation of the original Markdown formatting. Semantic preservation is required; exact whitespace/style preservation is optional.                    |
| NG2 | MDX, raw HTML, YAML frontmatter, and math syntax support in the first version.                                                                                         |
| NG3 | Sentence-level scroll synchronization. Paragraph/block-level synchronization is sufficient.                                                                            |
| NG4 | Real-time editing of the source document while preserving previous translation anchors. The document is assumed to be static.                                          |
| NG5 | Perfect semantic alignment when the LLM rewrites content heavily. The system should constrain and validate, but not claim perfect alignment under arbitrary rewriting. |

---

## 3. Core Design Principles

### 3.1 Block-Level Alignment Is the Source of Truth

Each meaningful Markdown block shall receive a stable internal ID. The same ID shall be used in:

| Layer                                | Use of ID                        |
| ------------------------------------ | -------------------------------- |
| Rust AST/intermediate representation | Source block tracking            |
| Translation request                  | LLM input unit identity          |
| Translation result                   | Output unit identity             |
| Markdown regeneration                | Placement of translated fragment |
| Rendered source pane                 | Source sync anchor               |
| Rendered target pane                 | Target sync anchor               |
| JavaScript sync engine               | Active block mapping             |

The scroll engine shall never depend on heading text, generated slugs, rendered line numbers, or whole-document scroll percentage.

### 3.2 The LLM May Decide Translation vs Preservation

The LLM should be allowed to decide whether text should be translated or left unchanged. This is especially important for:

* Code comments and docstrings
* Code identifiers and string literals
* Table values
* Technical terms
* Product names
* Acronyms
* Command names
* File paths
* Error messages
* Proper nouns

However, this freedom applies to **content**, not to the document’s structural contract. The model must not be allowed to change unit IDs, table column count, outer block kind, code block language metadata unless explicitly allowed, or list hierarchy.

### 3.3 Context-Rich Units Are Preferred Over Cell-Level Units

Tables and code blocks shall be translated as **context-preserving units**.

For tables, the default translation unit shall be the entire GFM table block, not individual cells. This preserves column semantics, row relationships, and surrounding context.

For very large tables, the fallback strategy shall be row-window batching, not isolated cell translation. A row-window batch may include the table header, selected contiguous rows, nearby row context, and the surrounding section context.

### 3.4 Rust Owns Markdown Structure

Rust shall be the authoritative layer for:

* GFM parsing
* Block identification
* Source range tracking
* Translation unit creation
* LLM response validation
* Markdown regeneration
* Alignment map generation

JavaScript shall not independently infer Markdown structure from raw Markdown unless it uses the same Rust parser compiled to WASM. Parser divergence between Rust and browser JavaScript would undermine anchor stability.

### 3.5 JavaScript Owns User Interaction

Vanilla JavaScript shall own:

* Dual-pane mounting
* Scroll observation
* Active block selection
* Counterpart block lookup
* Programmatic scroll control
* Feedback-loop prevention
* Resize/reflow handling

IntersectionObserver is suitable for observing which rendered anchors intersect a scroll container or viewport; it reports visibility changes asynchronously relative to a root element or viewport. ([MDN 웹 문서][2])

---

## 4. Standards and Implementation Basis

### 4.1 Markdown Dialect

The supported Markdown dialect shall be **GFM-compatible Markdown**, with GFM tables required.

GFM distinguishes block-level structure from inline content, and its parsing model supports first determining block structure and then parsing inline content inside blocks. ([GitHub][1])

GFM tables impose specific structural constraints: table rows contain cells separated by pipes, cell content is inline content, and block-level elements cannot be inserted inside a table cell. ([GitHub][1])

GFM also has table-shape behavior that must be accounted for: escaped pipes are valid inside cells, a table can be broken by an empty line or another block-level structure, header and delimiter cell counts must match for table recognition, and body rows with too few or too many cells are normalized by inserting empty cells or ignoring excess cells in HTML output. ([GitHub][1])

### 4.2 Code Blocks

Fenced code blocks shall be handled as literal content. GFM treats fenced code block content as literal text, not parsed as inline Markdown. Fences are made from at least three backticks or tildes, and the closing fence must use the same fence character with at least the same length. ([GitHub][1])

The system shall pass code block contents to the LLM as one unit, along with metadata indicating that the unit is a code block and specifying the language info string when available.

### 4.3 LLM Output Format

The translation API should use structured output where supported, because JSON mode only guarantees valid JSON while Structured Outputs are intended to match a specified JSON Schema. Application-level validation is still required after schema validation. ([OpenAI Developers][3])

### 4.4 Rust Parser Candidates

For Rust, **Comrak** is a strong candidate when AST manipulation is needed, because it is a CommonMark/GFM-compatible parser and renderer and exposes document parsing to an AST. ([GitHub][4])

**pulldown-cmark** is also relevant, especially when an event-based parser is preferred; it supports optional GFM features such as tables, task lists, and strikethrough. ([GitHub][5])

For this design, an AST-oriented parser is preferable because block IDs, source ranges, validation, and regeneration need structural access.

---

## 5. High-Level Architecture

### 5.1 Components

| Component                |                                    Runtime | Responsibility                                                                                        |
| ------------------------ | -----------------------------------------: | ----------------------------------------------------------------------------------------------------- |
| Markdown Parser          |                                       Rust | Parse source Markdown into GFM-aware AST or equivalent IR.                                            |
| Block Indexer            |                                       Rust | Assign stable IDs to blocks and record source ranges, AST paths, and hierarchy.                       |
| Translation Unit Builder |                                       Rust | Convert blocks into context-rich LLM translation units.                                               |
| Batch Manager            |                                       Rust | Group units by section and token budget.                                                              |
| LLM Client               |                                       Rust | Submit translation batches and receive structured results.                                            |
| Validator                |                                       Rust | Validate schema, IDs, Markdown parseability, table shape, code block constraints, and block envelope. |
| Regenerator              |                                       Rust | Build translated Markdown from accepted translated fragments.                                         |
| Alignment Map Generator  |                                       Rust | Produce source-target block mapping and render metadata.                                              |
| Renderer                 | Preferably Rust/WASM or Rust-produced HTML | Render source and target with identical sync anchors.                                                 |
| Sync Engine              |                                 Vanilla JS | Detect active source/target block and scroll counterpart pane.                                        |

### 5.2 Recommended Data Flow

| Step | Description                                                                  |
| ---: | ---------------------------------------------------------------------------- |
|    1 | Receive source Markdown.                                                     |
|    2 | Parse as GFM-compatible Markdown.                                            |
|    3 | Build a canonical document IR with block IDs and source ranges.              |
|    4 | Generate translation units at block granularity.                             |
|    5 | Add section path and local context to each unit.                             |
|    6 | Batch units by section and token budget.                                     |
|    7 | Send batches to the LLM using structured output.                             |
|    8 | Validate every returned unit.                                                |
|    9 | Retry failed units with stricter prompts or smaller scope.                   |
|   10 | Fallback to source fragment if retry fails and fallback is allowed.          |
|   11 | Regenerate full translated Markdown.                                         |
|   12 | Reparse translated Markdown and verify anchor count and block compatibility. |
|   13 | Generate alignment map.                                                      |
|   14 | Render source and target panes with shared sync IDs.                         |
|   15 | Use JavaScript to synchronize scroll position by active block ID.            |

---

## 6. Functional Requirements

### 6.1 Markdown Parsing and Block Identification

| ID   | Requirement                                                                                                                                                       |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FR-1 | The system shall parse the source Markdown using a GFM-compatible parser.                                                                                         |
| FR-2 | The system shall identify block-level units: headings, paragraphs, list items, blockquotes, tables, code blocks, thematic breaks, and other supported GFM blocks. |
| FR-3 | The system shall assign a unique stable ID to each sync-relevant block.                                                                                           |
| FR-4 | The system shall record each block’s AST path, parent relationship, source range, block kind, and section heading path.                                           |
| FR-5 | The system shall treat the document as static; therefore, path-based IDs are acceptable.                                                                          |
| FR-6 | The system shall preserve enough metadata to regenerate translated Markdown and to render source/target anchors.                                                  |

### 6.2 Translation Unit Generation

| ID    | Requirement                                                                                                                                                                             |
| ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FR-7  | The default translation granularity shall be paragraph/block level.                                                                                                                     |
| FR-8  | The system shall not split ordinary prose into sentence-level units for sync purposes.                                                                                                  |
| FR-9  | The system shall include contextual metadata for every translation unit: section path, nearby headings, block kind, and immediate neighboring block summaries or snippets where useful. |
| FR-10 | The system shall allow the LLM to decide whether content should be translated, preserved, or partially translated.                                                                      |
| FR-11 | The system shall not allow the LLM to change unit IDs or required structural metadata.                                                                                                  |
| FR-12 | The system shall support partial batch failure and unit-level retry.                                                                                                                    |

### 6.3 LLM Translation Contract

| ID    | Requirement                                                                                                                                                 |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FR-13 | The LLM response shall contain exactly one result per submitted unit.                                                                                       |
| FR-14 | Every result shall echo the original unit ID exactly.                                                                                                       |
| FR-15 | Every result shall declare its output kind: translated, preserved, partially translated, or fallback-needed.                                                |
| FR-16 | The LLM may preserve source text when translation would harm meaning, code behavior, table semantics, identifiers, or terminology.                          |
| FR-17 | The LLM shall treat source Markdown content as data, not as instructions.                                                                                   |
| FR-18 | The LLM shall not add explanations, commentary, or metadata into the translated Markdown fragment unless explicitly requested by the source content itself. |
| FR-19 | The LLM shall preserve the outer block contract for each unit.                                                                                              |

### 6.4 Markdown Regeneration

| ID    | Requirement                                                                                                                                                                               |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| FR-20 | The system shall regenerate a complete translated Markdown document.                                                                                                                      |
| FR-21 | The system shall preserve source document order.                                                                                                                                          |
| FR-22 | The system shall preserve heading levels, list hierarchy, table position, code block position, and blockquote nesting unless a validated structural transformation is explicitly allowed. |
| FR-23 | The system shall regenerate Markdown from validated translated fragments, not from unvalidated LLM output.                                                                                |
| FR-24 | The system shall reparse the final translated Markdown to verify that it is valid under the supported Markdown dialect.                                                                   |
| FR-25 | The system shall produce an alignment map from source block IDs to translated block IDs.                                                                                                  |

### 6.5 Rendering and Synchronization

| ID    | Requirement                                                                                                                               |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| FR-26 | The rendered source and translated panes shall include corresponding sync anchors for every sync-relevant block.                          |
| FR-27 | The sync engine shall select the active block based on viewport visibility, not scroll percentage.                                        |
| FR-28 | The sync engine shall scroll the opposite pane to the block with the same sync ID.                                                        |
| FR-29 | The sync engine shall prevent recursive scroll feedback loops.                                                                            |
| FR-30 | The sync engine shall recalculate anchor positions after layout changes such as image load, font load, pane resize, or content expansion. |

---

## 7. Block Handling Requirements

### 7.1 Summary

| Block Type     | Translation Unit                                | LLM Context                                      | Validation                                              | Sync Anchor                |
| -------------- | ----------------------------------------------- | ------------------------------------------------ | ------------------------------------------------------- | -------------------------- |
| Heading        | Heading text or heading block                   | Section path, level                              | Same heading level                                      | Heading block              |
| Paragraph      | Paragraph block                                 | Section path, neighboring blocks                 | Must remain paragraph-compatible                        | Paragraph                  |
| List item      | List item content                               | Parent list context, sibling snippets            | List topology preserved                                 | List item                  |
| Nested list    | Recursive list item units                       | Parent-child path                                | Nesting preserved                                       | List item                  |
| Table          | Whole table block by default                    | Full table, section context, preceding paragraph | Valid GFM table, same shape                             | Table block; row optional  |
| Code block     | Whole code content                              | Language, section context, surrounding prose     | Fence/info preserved; content accepted with constraints | Code block                 |
| Blockquote     | Container with child block units or quote block | Quote context                                    | Quote container preserved                               | Inner block or quote block |
| Thematic break | Non-translatable                                | None                                             | Preserved                                               | Usually no anchor          |
| Image          | Image block/inline                              | Alt text context                                 | URL preserved                                           | Image block if block-level |

---

## 8. Table Translation Design

### 8.1 Table Policy

Tables shall not be translated cell-by-cell by default. The default unit shall be:

> **One GFM table block → one context-rich table translation unit → one translated GFM table block.**

This preserves context and minimizes repeated token overhead.

### 8.2 LLM Input for Tables

A table translation unit shall include:

| Field                              | Purpose                                                                  |
| ---------------------------------- | ------------------------------------------------------------------------ |
| Unit ID                            | Stable mapping key                                                       |
| Block kind                         | Table                                                                    |
| Source table Markdown              | Full original table                                                      |
| Column count                       | Structural constraint                                                    |
| Header row                         | Semantic context                                                         |
| Alignment metadata                 | Regeneration constraint                                                  |
| Section path                       | Document context                                                         |
| Preceding/following block snippets | Local prose context                                                      |
| Translation instruction            | Translate or preserve content based on context, but keep table structure |

### 8.3 LLM Output for Tables

The LLM should return a translated GFM table Markdown fragment for the same table unit.

The system shall require:

| Constraint                 | Requirement                                                            |
| -------------------------- | ---------------------------------------------------------------------- |
| Column count               | Must match the source header column count.                             |
| Header/delimiter shape     | Must parse as a valid GFM table.                                       |
| Row count                  | Should match the source row count unless explicitly allowed otherwise. |
| Alignment                  | Should preserve source alignment.                                      |
| Escaped pipes              | Must remain valid.                                                     |
| Block content inside cells | Not allowed under GFM table rules.                                     |
| Table boundaries           | Must not absorb adjacent paragraphs or blockquotes.                    |

Because GFM table cells contain inline content and cannot contain block-level elements, table translation must reject outputs that introduce paragraph breaks, lists, fenced code blocks, or blockquotes inside cells. ([GitHub][1])

### 8.4 Large Table Strategy

For large tables, use tiered handling:

| Tier | Condition                  | Strategy                                                                                                                                 |
| ---- | -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| T1   | Small/medium table         | Translate whole table in one unit.                                                                                                       |
| T2   | Large but manageable table | Translate by row windows; include header and neighboring context.                                                                        |
| T3   | Very large table           | Translate header and selected row windows; keep non-translated rows as source or process asynchronously within current request pipeline. |
| T4   | Validation failure         | Retry with stricter table-only prompt.                                                                                                   |
| T5   | Repeated failure           | Fallback to source table and mark unit as fallback.                                                                                      |

A row-window must not be a cell-only translation. It should include enough table context to preserve meaning.

---

## 9. Code Block Translation Design

### 9.1 Code Policy

Code blocks shall be passed to the LLM as whole code-content units. The LLM may decide whether comments, docstrings, string literals, or embedded human-language text should be translated.

The application shall not pre-decide whether comments are translatable.

### 9.2 LLM Input for Code Blocks

A code block unit shall include:

| Field                    | Purpose                                                |
| ------------------------ | ------------------------------------------------------ |
| Unit ID                  | Stable mapping key                                     |
| Block kind               | Code block                                             |
| Language info string     | Syntax and context hint                                |
| Full code content        | Required for contextual judgment                       |
| Surrounding section path | Document context                                       |
| Adjacent prose snippets  | Helps determine whether comments are explanatory prose |
| Constraint summary       | Preserve executable semantics where applicable         |

### 9.3 LLM Output for Code Blocks

The preferred output is **code content only**, not a full fenced block. Rust should preserve or regenerate the opening and closing fence.

This avoids fence corruption and allows the serializer to choose a safe fence length if the translated code content contains backticks or tildes.

### 9.4 Code Validation

The system shall validate:

| Check                 | Description                                                                               |
| --------------------- | ----------------------------------------------------------------------------------------- |
| Unit ID               | Must match source unit.                                                                   |
| Content presence      | Output must not be empty unless source was empty or model explicitly preserves emptiness. |
| Fence safety          | Serializer must choose a fence that cannot be prematurely closed by content.              |
| Language metadata     | Source info string should be preserved unless explicitly allowed.                         |
| Indentation sanity    | Significant indentation should not be destroyed.                                          |
| Optional syntax check | If language tooling exists, run a parser/linter in non-blocking validation mode.          |

The system shall not require code content equality, because comments may be translated.

---

## 10. List and Blockquote Design

### 10.1 Lists

Lists are structurally sensitive because list membership depends on markers, indentation, and continuation rules. GFM list items are defined by marker type, marker width, spacing, and indentation; task list markers also have explicit checkbox semantics. ([GitHub][1])

The system shall:

| Requirement                      | Description                                                           |
| -------------------------------- | --------------------------------------------------------------------- |
| Preserve list topology           | Number of top-level items and nested item paths should remain stable. |
| Preserve marker type             | Ordered/unordered list type should remain stable.                     |
| Preserve task state              | Checked/unchecked state must remain stable.                           |
| Translate item content           | LLM may translate item prose, preserve terms, or partially translate. |
| Avoid marker regeneration by LLM | Rust should regenerate list markers where possible.                   |

For list items containing only inline text, the LLM may return translated item content. For list items containing nested blocks, the system should either translate child blocks separately or treat the list item as a constrained block fragment.

### 10.2 Blockquotes

Blockquotes are containers. The quote marker itself should be preserved by Rust, while the LLM translates or preserves the contained content.

The default sync anchor should be the inner block if blockquote contents are complex, or the quote block itself if the quote is simple.

---

## 11. Data Model

### 11.1 Source Block

Each parsed source block shall have the following conceptual fields:

| Field              | Description                                                            |
| ------------------ | ---------------------------------------------------------------------- |
| `block_id`         | Stable sync ID                                                         |
| `block_kind`       | Heading, paragraph, table, code block, list item, etc.                 |
| `ast_path`         | Structural path in parsed document                                     |
| `parent_id`        | Parent block ID if nested                                              |
| `section_path`     | Heading hierarchy at this block                                        |
| `source_range`     | Byte or line/column range in source Markdown                           |
| `source_markdown`  | Raw Markdown slice for the block                                       |
| `source_hash`      | Hash for integrity and caching                                         |
| `sync_role`        | Anchor, container, child-only, or non-sync                             |
| `translation_mode` | Heading text, paragraph fragment, table block, code content, preserved |

### 11.2 Translation Unit

Each LLM unit shall have:

| Field            | Description                                                          |
| ---------------- | -------------------------------------------------------------------- |
| `unit_id`        | Same as source block ID                                              |
| `block_kind`     | Source block kind                                                    |
| `input_mode`     | Markdown fragment, full table, code content, list item content, etc. |
| `source_payload` | Text or Markdown sent to the LLM                                     |
| `context`        | Section path and local context                                       |
| `constraints`    | Block-specific output rules                                          |
| `source_hash`    | Integrity check                                                      |
| `batch_id`       | Batch grouping identifier                                            |

### 11.3 Translation Result

Each LLM result shall have:

| Field                | Description                                                    |
| -------------------- | -------------------------------------------------------------- |
| `unit_id`            | Must exactly match request                                     |
| `output_kind`        | Translated, preserved, partially translated, failed            |
| `translated_payload` | Markdown fragment, table Markdown, or code content             |
| `model_notes`        | Optional internal diagnostic field; not inserted into Markdown |
| `warnings`           | Optional validation hints; not inserted into Markdown          |

### 11.4 Alignment Map

The alignment map shall be emitted separately from Markdown.

| Field             | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `document_id`     | Document identity                                    |
| `source_block_id` | Source anchor ID                                     |
| `target_block_id` | Target anchor ID, usually identical                  |
| `block_kind`      | Block kind                                           |
| `source_order`    | Source rendering order                               |
| `target_order`    | Target rendering order                               |
| `source_range`    | Source Markdown range                                |
| `target_range`    | Translated Markdown range if available               |
| `sync_role`       | Anchor behavior                                      |
| `fallback_status` | Whether target is translated, preserved, or fallback |

For a static document, `source_block_id` and `target_block_id` can be identical. The map still matters because it records validation state, rendering order, and fallback status.

---

## 12. ID Strategy

Because the document is static, the ID scheme can be simple and deterministic.

Recommended ID composition:

| Component         | Example Purpose                   |
| ----------------- | --------------------------------- |
| Document prefix   | Avoid collisions across documents |
| Block order       | Stable within one parsed version  |
| AST path          | Helps trace nested blocks         |
| Block kind        | Easier debugging                  |
| Short source hash | Detects accidental mismatch       |

Example conceptual forms:

| Style      | Example             |
| ---------- | ------------------- |
| Sequential | `b-000042`          |
| Path-based | `h2-3/p-2`          |
| Hybrid     | `b-000042-p-a91f2c` |

For the first implementation, sequential IDs plus source hashes are sufficient. Since the source document is static, ID survival across editing is not required.

---

## 13. Translation Batching

### 13.1 Batch Scope

Batches should be section-aware. A batch should usually contain blocks from the same heading section.

Each batch should include:

| Context                             | Purpose                 |
| ----------------------------------- | ----------------------- |
| Document title                      | Global terminology      |
| Heading path                        | Section semantics       |
| Previous/next block snippets        | Local continuity        |
| Glossary if available               | Terminology consistency |
| Block kind metadata                 | Structural constraints  |
| Source language and target language | Translation direction   |

### 13.2 Batch Size

Batch size should be limited by:

* Model token limit
* Expected output expansion
* Number of table/code-heavy units
* Retry cost
* Validation complexity

Large tables and code blocks should consume more of the batch budget because their output is more fragile.

### 13.3 Caching

The system should cache translation results by:

| Key Part                   | Reason                                          |
| -------------------------- | ----------------------------------------------- |
| Source hash                | Detect unchanged block                          |
| Target language            | Different translations per language             |
| Translation policy version | Invalidate when prompt changes                  |
| Model identifier           | Invalidate if model behavior changes materially |
| Block kind                 | Separate table/code/prose policies              |

---

## 14. Validation Requirements

Validation must be layered. Structured output alone is not sufficient.

### 14.1 General Validation

| Check                                      | Required |
| ------------------------------------------ | -------: |
| JSON/schema validity                       |      Yes |
| All requested IDs returned                 |      Yes |
| No extra IDs returned                      |      Yes |
| No duplicate IDs                           |      Yes |
| Unit kind compatible                       |      Yes |
| Payload non-empty where required           |      Yes |
| No model commentary inserted into Markdown |      Yes |

Structured Outputs should be used when available because it provides schema adherence beyond ordinary JSON mode, but domain-specific checks such as ID equality, table shape, and block compatibility still belong in the application. ([OpenAI Developers][3])

### 14.2 Markdown Fragment Validation

| Block Kind | Validation                                                                  |
| ---------- | --------------------------------------------------------------------------- |
| Heading    | Same heading level; no unexpected block split.                              |
| Paragraph  | Parses as one paragraph or an allowed inline-compatible fragment.           |
| List item  | Does not alter list hierarchy unless explicitly allowed.                    |
| Table      | Parses as valid GFM table; same column count; row count normally unchanged. |
| Code block | Content accepted; fence regenerated safely by Rust.                         |
| Blockquote | Quote container preserved; children valid.                                  |

### 14.3 Full Document Validation

After regeneration, the full translated Markdown shall be reparsed.

The system shall verify:

| Check                 | Description                                              |
| --------------------- | -------------------------------------------------------- |
| Parse success         | Translated Markdown parses under supported dialect.      |
| Anchor count          | Expected sync anchors exist.                             |
| Anchor order          | Target anchor order matches source order.                |
| Table validity        | Translated tables are still recognized as tables.        |
| List topology         | List hierarchy remains compatible.                       |
| Code block boundaries | Code blocks do not absorb following content.             |
| Raw HTML policy       | Unsupported raw HTML is rejected, escaped, or sanitized. |

---

## 15. Retry and Fallback Policy

The user permits partial retry and fallback to source.

### 15.1 Retry Levels

| Level | Trigger              | Action                                                |
| ----- | -------------------- | ----------------------------------------------------- |
| R0    | Valid output         | Accept.                                               |
| R1    | Schema or ID failure | Retry same batch with stricter instruction.           |
| R2    | One unit fails       | Retry failed unit alone with full context.            |
| R3    | Table failure        | Retry table only with explicit shape constraints.     |
| R4    | Large table failure  | Retry by row windows with header context.             |
| R5    | Code block failure   | Retry code content only; preserve fence externally.   |
| R6    | Repeated failure     | Fallback to source fragment and mark fallback status. |

### 15.2 Fallback Output

Fallback must be explicit in the alignment map.

For fallback units:

| Field                   | Value                            |
| ----------------------- | -------------------------------- |
| Translation status      | `fallback_source`                |
| Rendered target content | Original source content          |
| Sync anchor             | Still present                    |
| User indication         | Optional UI marker               |
| Retry eligibility       | Yes, if manually triggered later |

---

## 16. Markdown Regeneration Strategy

### 16.1 Preferred Strategy

The system should use a **hybrid regeneration strategy**:

| Case                       | Strategy                                                                               |
| -------------------------- | -------------------------------------------------------------------------------------- |
| Unchanged/preserved blocks | Reuse original source slice where possible.                                            |
| Heading                    | Preserve heading marker; replace heading text.                                         |
| Paragraph                  | Replace paragraph content with validated translated paragraph fragment.                |
| List item                  | Preserve list topology; replace validated item content.                                |
| Table                      | Replace entire table block with validated translated table Markdown.                   |
| Code block                 | Preserve or regenerate fence/info string; replace content with validated model output. |
| Blockquote                 | Preserve quote structure; replace translated child content.                            |

This is preferable to fully serializing the entire AST when exact authoring style matters. However, if source range handling becomes complex, full AST serialization is acceptable as long as the output is valid GFM and structurally compatible.

### 16.2 Fence Regeneration

For code blocks, Rust should choose a fence delimiter that cannot conflict with the code content. If the code contains triple backticks, the serializer can use a longer backtick fence or tilde fence, subject to dialect policy.

### 16.3 Table Regeneration

For tables, Rust should not trust visual pipe alignment. The translated table should be parsed structurally, then reserialized into valid GFM.

The renderer must not depend on manually aligned table text. Long translated cell content may make Markdown table source visually wide, but the rendered table remains structurally valid.

---

## 17. Rendering Design

### 17.1 Preferred Rendering Architecture

The safest approach is:

> **Rust produces regenerated Markdown, alignment map, and annotated render-ready HTML. JavaScript mounts the HTML and performs sync.**

This avoids parser divergence between Rust and browser-side Markdown rendering.

Alternative acceptable approach:

> **Rust compiles parser/renderer logic to WASM. Browser uses WASM to render source and target with the same anchor logic. Vanilla JavaScript still handles sync.**

Least preferred approach:

> Browser JavaScript independently parses Markdown with a separate parser.

This should be avoided because different Markdown parsers may produce different block boundaries, especially around tables, lists, and code blocks.

### 17.2 Rendered Anchor Contract

Every sync-relevant rendered block shall include:

| Attribute          | Purpose                        |
| ------------------ | ------------------------------ |
| Sync ID            | Links source and target blocks |
| Block kind         | Debugging and sync heuristics  |
| Source order       | Stable ordering                |
| Fallback status    | UI indication                  |
| Optional parent ID | Nested block handling          |

The actual attribute names are implementation details, but the DOM must allow the sync engine to query all anchors in order.

---

## 18. Scroll Synchronization Design

### 18.1 Active Block Detection

The sync engine shall observe anchors inside the active pane.

Active block selection should use:

| Signal                      | Purpose                                       |
| --------------------------- | --------------------------------------------- |
| Intersection ratio          | Prefer substantially visible blocks.          |
| Distance to viewport center | Prefer the block the user is likely reading.  |
| Source order                | Tie-breaker.                                  |
| Hysteresis                  | Avoid rapid flipping between adjacent blocks. |
| Scroll direction            | Improve transition behavior.                  |

IntersectionObserver is appropriate for this because it can watch multiple target elements relative to a scroll container root. ([MDN 웹 문서][2])

### 18.2 Counterpart Scrolling

When active block `X` is selected in pane A:

1. Find block `X` in pane B.
2. Compute desired scroll target.
3. Align counterpart block near the same viewport region.
4. Apply programmatic scroll.
5. Suppress feedback until the programmatic scroll settles.

For block-level sync, top or center alignment is usually sufficient. For very large table/code blocks, intra-block progress may be used:

> Source block internal vertical progress → target block internal vertical progress.

This is optional but useful for large tables and code blocks.

### 18.3 Feedback Loop Prevention

The sync engine shall maintain:

| Mechanism                      | Purpose                                   |
| ------------------------------ | ----------------------------------------- |
| Active pane state              | Know which pane the user is controlling.  |
| Programmatic scroll flag       | Ignore scroll events caused by sync.      |
| Short lock window              | Prevent immediate reverse sync.           |
| RequestAnimationFrame batching | Avoid layout thrashing.                   |
| Resize/reflow observer         | Recompute positions after layout changes. |

### 18.4 Layout Change Handling

The sync engine shall recalculate or refresh anchor measurements after:

* Font load
* Image load
* Pane resize
* Browser zoom change
* Collapsible section expansion
* Table layout change
* Code block line wrapping change

---

## 19. Edge Cases

### 19.1 Tables

| Edge Case                     | Required Handling                                    |
| ----------------------------- | ---------------------------------------------------- |
| Escaped pipe inside a cell    | Preserve valid escaping or reserialize structurally. |
| Translated cell contains pipe | Escape or reserialize safely.                        |
| Header/delimiter mismatch     | Reject and retry.                                    |
| Row count changed             | Reject unless explicitly allowed.                    |
| Cell count changed            | Normalize only if safe; otherwise reject.            |
| Long translated text          | Accept; do not rely on visual source alignment.      |
| Block syntax inside cell      | Reject under GFM table policy.                       |
| Very large table              | Use row-window translation with header context.      |

### 19.2 Code Blocks

| Edge Case                                                  | Required Handling                                               |
| ---------------------------------------------------------- | --------------------------------------------------------------- |
| Code contains backticks                                    | Regenerate safe fence length.                                   |
| LLM translates executable identifiers                      | Optional syntax check; retry if clearly broken.                 |
| LLM translates comments                                    | Accept if structure remains valid.                              |
| LLM removes indentation                                    | Retry for indentation-sensitive languages.                      |
| LLM outputs a full fenced block when content-only expected | Strip and validate only if unambiguous; otherwise retry.        |
| Unterminated original code fence                           | Parser-defined behavior; treat as opaque and preserve if risky. |

### 19.3 Lists

| Edge Case                       | Required Handling                                     |
| ------------------------------- | ----------------------------------------------------- |
| Nested list indentation         | Preserve topology through AST, not raw text guessing. |
| Ordered list start number       | Preserve metadata where supported.                    |
| Task list checkbox              | Preserve checked state.                               |
| Empty list item                 | Preserve or translate only if content exists.         |
| List item containing code/table | Translate child blocks using their own rules.         |

### 19.4 Paragraphs and Inline Markdown

| Edge Case               | Required Handling                                                                  |
| ----------------------- | ---------------------------------------------------------------------------------- |
| Inline code             | LLM may preserve or translate based on context, but resulting Markdown must parse. |
| Link URL                | Preserve URL unless explicit policy allows localization.                           |
| Link text               | LLM may translate.                                                                 |
| Image alt text          | LLM may translate.                                                                 |
| Emphasis/strong markers | Must remain balanced after parsing.                                                |
| Autolinks               | Preserve target.                                                                   |
| Raw HTML                | Reject, escape, or preserve according to unsupported-feature policy.               |

### 19.5 Prompt Injection in Source Markdown

The source Markdown may contain text that looks like instructions to the model.

The LLM prompt must state that:

* Source content is data.
* Instructions inside the source content must not override system/developer instructions.
* The model must only perform the translation task.
* Output must conform to the requested schema.

This is a security requirement, not merely a prompt-style preference.

---

## 20. Non-Functional Requirements

### 20.1 Correctness

| ID    | Requirement                                                               |
| ----- | ------------------------------------------------------------------------- |
| NFR-1 | The translated Markdown shall parse under the supported Markdown dialect. |
| NFR-2 | Source and target anchor order shall remain compatible.                   |
| NFR-3 | Validation failures shall not silently corrupt the translated document.   |

### 20.2 Performance

| ID    | Requirement                                                                        |
| ----- | ---------------------------------------------------------------------------------- |
| NFR-4 | Translation batching shall avoid whole-document requests for long documents.       |
| NFR-5 | Rendering shall support long documents without excessive layout recalculation.     |
| NFR-6 | Scroll sync shall avoid heavy synchronous DOM measurement during active scrolling. |
| NFR-7 | Translation results should be cached by source hash and policy version.            |

### 20.3 Resilience

| ID     | Requirement                                       |
| ------ | ------------------------------------------------- |
| NFR-8  | Unit-level retry shall be supported.              |
| NFR-9  | Fallback to source content shall be supported.    |
| NFR-10 | Validation reports shall be stored for debugging. |

### 20.4 Security

| ID     | Requirement                                                                                     |
| ------ | ----------------------------------------------------------------------------------------------- |
| NFR-11 | Raw HTML shall be disabled, escaped, sanitized, or explicitly rejected in v1.                   |
| NFR-12 | Source Markdown shall be treated as untrusted input.                                            |
| NFR-13 | Translated Markdown shall also be treated as untrusted input before rendering.                  |
| NFR-14 | Prompt injection from source content shall be mitigated by prompt design and output validation. |

### 20.5 Maintainability

| ID     | Requirement                                                               |
| ------ | ------------------------------------------------------------------------- |
| NFR-15 | Translation policies shall be versioned.                                  |
| NFR-16 | Parser options shall be explicit and test-covered.                        |
| NFR-17 | Table, code, list, and paragraph validation shall be separately testable. |
| NFR-18 | The alignment map format shall be stable across renderer changes.         |

---

## 21. Testing and Acceptance Criteria

### 21.1 Parser and ID Tests

| Test                           | Acceptance Criteria                               |
| ------------------------------ | ------------------------------------------------- |
| Simple headings and paragraphs | Blocks receive stable IDs in source order.        |
| Nested lists                   | Parent-child block paths are correct.             |
| GFM table                      | Table recognized as one table block.              |
| Code block                     | Code content recognized as literal block content. |
| Blockquote with nested list    | Container and child IDs are generated correctly.  |

### 21.2 Translation Validation Tests

| Test                                 | Acceptance Criteria                          |
| ------------------------------------ | -------------------------------------------- |
| Missing unit ID                      | Batch rejected or retried.                   |
| Extra unit ID                        | Batch rejected or retried.                   |
| Table column mismatch                | Table unit rejected and retried.             |
| Code block with translated comments  | Accepted if content is valid and fence-safe. |
| Paragraph split into multiple blocks | Rejected unless policy allows.               |
| List item count changed              | Rejected unless policy allows.               |

### 21.3 Markdown Regeneration Tests

| Test                       | Acceptance Criteria                                           |
| -------------------------- | ------------------------------------------------------------- |
| Final Markdown reparse     | Must parse successfully.                                      |
| Source/target anchor count | Must match expected sync-relevant block count.                |
| Table remains table        | Reparsed target table is recognized as GFM table.             |
| Code block boundary        | Following Markdown is not swallowed into code block.          |
| Fallback block             | Source content appears in target with fallback status in map. |

### 21.4 Scroll Sync Tests

| Test                    | Acceptance Criteria                                                 |
| ----------------------- | ------------------------------------------------------------------- |
| Scroll source paragraph | Target pane scrolls to corresponding paragraph.                     |
| Scroll target paragraph | Source pane scrolls to corresponding paragraph.                     |
| Long table              | Target scrolls to table block; optional intra-block progress works. |
| Long code block         | Target scrolls to code block; no loop occurs.                       |
| Rapid scrolling         | Sync remains stable and does not oscillate.                         |
| Resize pane             | Sync recalculates positions correctly.                              |

---

## 22. Recommended Implementation Plan

### Phase 1 — Core Markdown Pipeline

* Select Rust parser with GFM table support.
* Parse source Markdown.
* Build block IR.
* Assign block IDs.
* Generate alignment map for source-only rendering.
* Regenerate source-equivalent Markdown from the IR.
* Verify parse → regenerate → parse stability.

### Phase 2 — Translation Units and LLM Contract

* Implement block-level translation units.
* Add section context.
* Implement structured output schema.
* Add validation for IDs and block kinds.
* Implement paragraph, heading, list item, table, and code block policies.

### Phase 3 — Validation and Fallback

* Add table shape validation.
* Add code block fence-safe regeneration.
* Add retry policy.
* Add fallback-to-source policy.
* Store validation report.

### Phase 4 — Translated Markdown Regeneration

* Splice or serialize translated fragments into full Markdown.
* Reparse translated Markdown.
* Verify source-target anchor compatibility.
* Emit translated Markdown and alignment map.

### Phase 5 — Rendering and Sync

* Generate annotated HTML in Rust, or expose Rust renderer through WASM.
* Mount source and target panes.
* Add vanilla JS sync engine.
* Use IntersectionObserver for active block detection.
* Add feedback-loop prevention.
* Add resize/reflow handling.

### Phase 6 — Hardening

* Add long-document batching.
* Add large-table row-window strategy.
* Add code syntax checks where practical.
* Add performance tests.
* Add security tests for prompt injection and unsafe HTML.

---

## 23. Key Design Decisions

| Decision                  | Final Recommendation                                                        |
| ------------------------- | --------------------------------------------------------------------------- |
| Translation granularity   | Block-level by default.                                                     |
| Scroll sync granularity   | Paragraph/block-level.                                                      |
| Table handling            | Whole-table unit by default; row-window fallback for large tables.          |
| Code handling             | Whole code-content unit; LLM decides comment translation.                   |
| LLM authority             | May decide translate vs preserve for content, but not structural contracts. |
| Markdown output           | Regenerate full translated Markdown in Rust.                                |
| Rendering source of truth | Prefer Rust-generated annotated HTML or Rust WASM renderer.                 |
| Browser sync              | Vanilla JS with IntersectionObserver and feedback lock.                     |
| Failure behavior          | Retry failed unit; fallback to source allowed.                              |
| Parser choice             | Prefer AST-oriented GFM parser; Comrak is a strong candidate.               |

---

## 24. Main Risk Areas

| Risk                                        | Mitigation                                                              |
| ------------------------------------------- | ----------------------------------------------------------------------- |
| LLM corrupts GFM table syntax               | Parse and validate translated table; retry or fallback.                 |
| LLM changes table shape                     | Enforce column and row constraints.                                     |
| LLM breaks code semantics                   | Preserve full code context; optional syntax checks; fallback if severe. |
| Browser renderer disagrees with Rust parser | Use Rust-generated annotated HTML or Rust WASM renderer.                |
| Scroll sync oscillates                      | Use active-pane state and programmatic-scroll lock.                     |
| Large tables exceed token budget            | Use row-window batching with header and local context.                  |
| Source Markdown contains prompt injection   | Treat source as data and validate output strictly.                      |
| Raw HTML creates security issues            | Disable, escape, sanitize, or reject raw HTML in v1.                    |

---

## 25. Final Recommended Architecture

The strongest version of the design is:

1. **Rust parses GFM Markdown into a block-aware IR.**
2. **Rust assigns stable block IDs.**
3. **Rust builds context-rich block-level translation units.**
4. **Tables are translated as whole table blocks by default.**
5. **Code blocks are passed as whole code-content units.**
6. **The LLM decides content translation vs preservation.**
7. **Rust validates all LLM outputs against structural constraints.**
8. **Rust retries failed units or falls back to source content.**
9. **Rust regenerates complete translated Markdown.**
10. **Rust emits an alignment map.**
11. **Rendering uses Rust-generated annotated HTML or WASM-backed rendering to avoid parser divergence.**
12. **Vanilla JavaScript synchronizes panes by active block ID using IntersectionObserver and feedback-loop protection.**

This design preserves the main advantage of your original ID-based approach while avoiding its main failure mode: giving the LLM excessive responsibility for Markdown structure. The LLM remains responsible for translation judgment, including whether to translate or preserve code comments and table values, but the application remains responsible for structural validity, anchor stability, and synchronized rendering.

[1]: https://github.github.com/gfm/ "GitHub Flavored Markdown Spec"
[2]: https://developer.mozilla.org/en-US/docs/Web/API/Intersection_Observer_API "Intersection Observer API - Web APIs | MDN"
[3]: https://developers.openai.com/api/docs/guides/structured-outputs "Structured model outputs | OpenAI API"
[4]: https://github.com/kivikakk/comrak?utm_source=chatgpt.com "kivikakk/comrak: CommonMark + GFM compatible ..."
[5]: https://github.com/pulldown-cmark/pulldown-cmark?utm_source=chatgpt.com "Pulldown Cmark"
