//! `Translator` trait + wire types crossing the LLM boundary.
//!
//! Stability: `Translator`, `TranslationBatch`, `TranslationBatchResult`,
//! and `UnitResult` are versioned by the contract pack
//! (`docs/architecture/contracts.md` §1).
//!
//! [`prompt`] holds the provider-neutral half of the boundary — the user
//! prompt + hint assembly, the Structured Output schema object, and the
//! response-envelope parser — so every provider speaks the wording the
//! validators expect (OI-0029).
//!
//! TRACE: ADR-0002

pub mod prompt;

use crate::id::{BlockId, BlockKind};
use crate::profile::ProfileMetadata;
use crate::validate::ValidationLayer; // module-level cycle with validate.rs is fine (same crate)
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// Identifier handed to the provider so it can group/log work.
///
/// TRACE: SCN-10
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BatchId(pub String);

impl BatchId {
    pub fn new(ordinal: u32) -> Self {
        Self(format!("b-{ordinal:04}"))
    }
}

impl std::fmt::Display for BatchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One unit of translation work: a source-block payload with structural
/// constraints and surrounding context.
///
/// Non-exhaustive: construct via [`TranslationUnit::new`] + `with_*`;
/// struct literals are in-crate only.
///
/// TRACE: SCN-01..SCN-06
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct TranslationUnit {
    pub unit_id: BlockId,
    pub block_kind: BlockKind,
    pub input_mode: InputMode,
    pub source_payload: String,
    pub context: BlockContext,
    pub constraints: BlockConstraints,
    pub source_hash: u64,
    pub batch_id: BatchId,
    /// Non-content retry channel. `None` on first dispatch; present only
    /// on re-dispatched units. EXT-2026-07 P0-2.
    pub retry: Option<RetryContext>,
}

impl TranslationUnit {
    /// The five required-by-essence fields are positional; the rest
    /// default (`context`/`constraints` to their `Default`s, `batch_id`
    /// to the `BatchId::new(0)` placeholder the batcher overwrites per
    /// chunk, `retry` to `None` per the first-dispatch contract).
    /// `source_hash` is required deliberately: it is a `CacheKey` axis
    /// with no meaningful zero — a defaulted hash would silently alias
    /// cache entries.
    pub fn new(
        unit_id: BlockId,
        block_kind: BlockKind,
        input_mode: InputMode,
        source_payload: String,
        source_hash: u64,
    ) -> Self {
        Self {
            unit_id,
            block_kind,
            input_mode,
            source_payload,
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash,
            batch_id: BatchId::new(0),
            retry: None,
        }
    }

    pub fn with_context(mut self, context: BlockContext) -> Self {
        self.context = context;
        self
    }

    pub fn with_constraints(mut self, constraints: BlockConstraints) -> Self {
        self.constraints = constraints;
        self
    }

    pub fn with_batch_id(mut self, batch_id: BatchId) -> Self {
        self.batch_id = batch_id;
        self
    }

    pub fn with_retry(mut self, retry: RetryContext) -> Self {
        self.retry = Some(retry);
        self
    }
}

/// Non-content retry channel (the side channel ADR-0009 anticipated).
/// Carried OUTSIDE `source_payload` — injecting guidance into the
/// payload was tried and reverted: a faithful provider echoes it into
/// the output and the strict full-document reparse rejects it as an
/// extra block (ADR-0009).
///
/// Present only on re-dispatched units; carries the LATEST failure
/// only (built from the pristine original unit each round, never
/// accumulated).
///
/// EXT-2026-07 P0-2
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryContext {
    /// 1-based attempt number of THIS dispatch (first retry carries 2).
    pub attempt: u32,
    /// Validation layer that rejected the previous attempt.
    pub rejected_by: Option<ValidationLayer>,
    /// Truncated diagnostic from the previous rejection. May quote
    /// source-derived text — providers must treat it as data.
    pub reason: Option<String>,
}

/// Tells the provider how to interpret `source_payload`.
///
/// TRACE: SCN-02
/// TRACE: SCN-03
/// TRACE: SCN-04
#[derive(Debug, Clone)]
pub enum InputMode {
    TextFragment,
    FullTableMarkdown,
    /// One header-carrying row window of an oversize table (DCR-0026).
    ///
    /// `source_payload` is a **complete GFM table** — the source table's
    /// header row, its delimiter row, and one contiguous run of its body rows
    /// — and is translated whole under exactly the same rules as
    /// [`InputMode::FullTableMarkdown`]. A window is never an isolated cell
    /// and never a headerless fragment (architectural invariant 3); the header
    /// rides with every window so the model has the column context.
    ///
    /// The fields carry only what merge and eviction need — the mini-table
    /// itself is the payload, not a field. `parent_block_id` is the source
    /// table's ADR-0005 block id, which is also what the alignment map and the
    /// DOM see: a window's own `unit_id` (`<parent>.wNN`) exists on the wire
    /// and in the validation report only.
    ///
    /// A `Translator` needs no code for this variant beyond what it already
    /// does for a table: the wire label is `"table_row_window"` and the
    /// payload contract is a whole table.
    ///
    /// The fields were **redefined** in v0.4.0 (DCR-0026): the reserved
    /// `header_markdown` / `rows_markdown` / `window_index` shape was never
    /// produced and described a payload split the design does not use.
    ///
    /// TRACE: SCN-03
    /// TRACE: DCR-0026
    TableRowWindow {
        /// The source table this window is a slice of.
        parent_block_id: BlockId,
        /// 0-based position of this window among its siblings, in source row
        /// order.
        window_index: u32,
        /// How many windows the parent table was split into.
        window_count: u32,
    },
    /// Full fenced code block — `source_payload` carries the entire
    /// ` ```lang … ``` ` Markdown including the open/close fences.
    /// `language_info` mirrors the info string for downstream re-fencing
    /// (the regenerator may swap fence length to escape backticks in the
    /// translated content, but the info string itself is preserved).
    ///
    /// Earlier docs claimed this variant carried *content only* with
    /// fences owned by the regenerator; in practice the entire pipeline
    /// — payload extraction, the user-prompt body, the per-kind
    /// validator, and the regenerator's content-or-source fallback —
    /// all assume a full fenced block, and the live model is asked to
    /// preserve fences explicitly. The variant name and docs now match
    /// that behavior. Switching to genuine content-only is a separate
    /// design change; see R0001-0026 in reviews/reviewed/0001.md.
    ///
    /// A source block spelled with a four-column INDENT rather than fences
    /// rides this same variant: `source_payload` is that block's content
    /// re-fenced by the engine (`regen::regenerate_code_block`, bare fence, no
    /// info string, length chosen safe against backtick runs in the body), so
    /// the wire label, the payload contract, and `language_info` are all
    /// unchanged — the indented spelling is made to conform to this variant,
    /// not to redefine it. What the provider returns is validated and
    /// regenerated as a fenced block; the indented spelling survives only on
    /// the fallback path.
    FullCodeBlock {
        language_info: Option<String>,
    },
    ListItemContent,
    BlockquoteContent,
    /// Block-level raw HTML: `source_payload` is a compact JSON array of
    /// decoded text segments (spec §4.1); markup never reaches the model.
    HtmlSegments,
}

/// Document-local context passed alongside each unit.
///
/// TRACE: SCN-09
#[derive(Debug, Clone, Default)]
pub struct BlockContext {
    pub section_path: Vec<HeadingSnippet>,
    pub preceding_block: Option<NeighborSnippet>,
    pub following_block: Option<NeighborSnippet>,
    pub document_title: Option<String>,
}

/// Heading snippet inside a section path.
///
/// **Both** fields cross the provider boundary (R0001-0024):
/// [`prompt::build_user_prompt`] serializes each entry as
/// `context.section_path[] = {level, text}`, and both feed the
/// `context_hash` component of the cache key, so a heading whose level or
/// text changed does not reuse a cached translation.
///
/// TRACE: SCN-09
#[derive(Debug, Clone)]
pub struct HeadingSnippet {
    /// Heading level 1–6, straight from the parsed block kind. Sent as-is;
    /// the path is not always contiguous (an `h1` may be followed directly
    /// by an `h3`), which is why position cannot stand in for it.
    pub level: u8,
    /// The heading's plain text — inline markup flattened by the parser,
    /// content bytes untouched (`unit::context`). Source-derived, therefore
    /// untrusted data (invariant 7); the user prompt frames it as such.
    pub text: String,
}

/// Brief snippet of a neighboring block, surfaced to the provider for context.
///
/// **Both** fields cross the provider boundary (R0001-0024): the kind rides
/// as `context.preceding_kind` / `context.following_kind` in its wire
/// spelling ([`BlockKind::wire_str`]) and the summary as
/// `context.preceding_summary` / `context.following_summary`, and both feed
/// `context_hash`.
///
/// TRACE: SCN-09
#[derive(Debug, Clone)]
pub struct NeighborSnippet {
    /// What the neighbor *is* — a preceding code block and a preceding
    /// paragraph are read very differently by a translator.
    pub kind: BlockKind,
    /// Leading source excerpt of the neighbor, truncated by character
    /// count. Source-derived, therefore untrusted data (invariant 7).
    pub summary: String,
}

/// Structural constraints that the provider must honor.
///
/// TRACE: SCN-02
/// TRACE: SCN-04
/// TRACE: SCN-05
#[derive(Debug, Clone, Default)]
pub struct BlockConstraints {
    pub must_preserve_table_columns: Option<u32>,
    pub must_preserve_table_alignment: Option<Vec<TableAlign>>,
    pub must_preserve_table_row_count: Option<u32>,
    pub must_preserve_code_fence_info: Option<String>,
    pub must_preserve_list_topology: bool,
    pub must_preserve_heading_level: Option<u8>,
    /// Hint the provider that the translated table cells must not
    /// introduce block-level structure (extra paragraphs, sublists,
    /// fenced code) inside an inline cell. Currently advisory only —
    /// the validator does not enforce it; intended as a hook for a
    /// future inline-content checker.
    ///
    /// TRACE: R0001-0046 in `reviews/reviewed/0001.md` (removed; kept for
    /// forward-compatibility; not enforced)
    pub forbid_block_breaks_in_inline: bool,
    /// Expected list-item topology: `(depth, ordered, task)` tuples in
    /// source order. `None` for non-list units.
    ///
    /// TRACE: SCN-05
    pub expected_list_topology: Option<Vec<ListTopologyEntry>>,
    /// Expected blockquote child-kind sequence (wire-form strings).
    ///
    /// TRACE: SCN-06
    pub expected_blockquote_children: Option<Vec<String>>,
    /// Html-kind constraints (spec §3.2/§4.2): segment count + parent labels
    /// feed the prompt hints and the per-kind count check; `source_bytes`
    /// and `block_type` are validator-side inputs for the splice check and
    /// are NEVER serialized into the prompt.
    pub html: Option<HtmlSegmentConstraints>,
}

/// Structural facts of one `BlockKind::Html` unit (spec §3.2/§4.2).
///
/// `segment_count` / `segment_labels` are prompt-visible hints; the other
/// two fields are validator-side inputs only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlSegmentConstraints {
    pub segment_count: u32,
    pub segment_labels: Vec<String>,
    /// Raw source bytes of the block — the splice-check input. Never sent
    /// to the model.
    pub source_bytes: String,
    /// The CommonMark HTML block type of a raw-HTML **island inside a
    /// Markdown document**: `1`–`7`, where types 6/7 get blank-line
    /// normalization at splice and type 1 is exempt (spec §3.3).
    ///
    /// `0` means **a block of an HTML document**, where no CommonMark type
    /// applies. It is a record of what the source was, not the switch the
    /// splice reads: the policy is derived from the block's spelling
    /// (`Spelling::blank_line_policy_for`), and `0` reaches the same answer
    /// through `BlankLinePolicy::from_commonmark_html_block_type` because
    /// anything outside `6 | 7` keeps its blank lines. A doc-comment domain
    /// widening, not a type change — this struct is a §0 tier-(a) row without
    /// `#[non_exhaustive]`, so its field types are frozen between windows.
    pub block_type: u8,
}

/// One entry in a list-topology fingerprint — depth, ordered flag,
/// task marker state, and (EXT-2026-07 P2-9) the owning ordered list's
/// marker facts.
///
/// `depth` is a comparison fingerprint, not a human-readable level.
/// The walker that produces it (`structure::inspect_list_topology`) starts
/// at 0 on the document node and increments on each `List` node it
/// enters, so items of the outermost list record depth **1**, items of
/// a first-level nested list record depth 2, and so on. Both sides of
/// validation use the same walker, so only relative consistency
/// matters; tests + downstream consumers should match this convention.
/// R0006-0032.
///
/// **There is no depth ceiling.** The walker counts nesting in `u32` and
/// records it here unchanged, so two items at different levels always
/// fingerprint differently, however pathological the nesting. Through v0.2.0
/// this field was a `u8` and the walker *saturated* into it at 255
/// (R0001-0025), which left the depth field unable to tell deep items apart
/// and leaned the whole comparison on entry count, entry order, `child_kinds`
/// and the marker facts; widening the field in the 0.3.0 curated-surface
/// window (owner decision 2026-08-06) removed the saturation, the constant
/// and the ceiling warning together.
///
/// The exactness is not academic. `markdown::MAX_BLOCK_NESTING_DEPTH` (128)
/// keeps *source* payloads far short of any interesting depth, but a provider
/// result is fingerprinted by this same walker **without** passing through
/// `parse` — so the actual side of the comparison is the untrusted one, and it
/// is exactly the side a ceiling would have blinded.
///
/// TRACE: SCN-05
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListTopologyEntry {
    pub depth: u32,
    pub ordered: bool,
    pub task: Option<bool>,
    /// Wire-form kinds of this item's direct block children, in order
    /// (e.g. `["paragraph", "list"]`). R0008-0014: the `(depth, ordered,
    /// task)` tuple alone let a provider add or drop a paragraph / code
    /// block / blockquote inside an item without failing validation.
    pub child_kinds: Vec<String>,
    /// EXT-2026-07 P2-9 (OI-0022): the owning ordered list's start ordinal
    /// (`3` for a list opening at `3.`). Replicated on every item of the
    /// list so a positional entry-by-entry compare catches a renumbered
    /// list. `None` for items of an unordered list.
    pub start: Option<u32>,
    /// EXT-2026-07 P2-9: the owning ordered list's item delimiter
    /// (`.` vs `)`). `None` for items of an unordered list.
    pub delimiter: Option<ListDelimiter>,
    /// EXT-2026-07 P2-9: the owning list's tightness — whether items'
    /// paragraphs are rendered loose (wrapped in `<p>`) or tight. Set for
    /// every list item (`Some`).
    pub tight: Option<bool>,
}

/// Ordered-list item delimiter — the character following each number
/// (`1.` vs `1)`). Mirrors Comrak's `ListDelimType`; Rust-internal (the
/// wire form is carried by the provider client's `ListTopologyHint`).
///
/// TRACE: SCN-05
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListDelimiter {
    /// A period delimiter: `1.`
    Period,
    /// A paren delimiter: `1)`
    Paren,
}

/// Column alignment marker for GFM tables.
///
/// TRACE: SCN-02
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableAlign {
    None,
    Left,
    Center,
    Right,
}

/// One translatable batch handed to the provider.
///
/// TRACE: SCN-10
#[derive(Debug, Clone)]
pub struct TranslationBatch {
    pub batch_id: BatchId,
    pub units: Vec<TranslationUnit>,
    pub source_language: String,
    pub target_language: String,
    pub glossary: Vec<GlossaryEntry>,
    pub profile: ProfileMetadata,
}

/// Provider's response for one batch.
///
/// TRACE: SCN-11
#[derive(Debug, Clone)]
pub struct TranslationBatchResult {
    pub batch_id: BatchId,
    pub detected_source_language: Option<String>,
    pub units: Vec<UnitResult>,
}

/// Per-unit result inside a [`TranslationBatchResult`].
///
/// The serde derives are `cache::DiskCache`'s on-disk transport (DCR-0028 §2),
/// not a provider wire contract — a provider's response is parsed by
/// [`prompt::parse_batch_output`], which owns that shape.
///
/// TRACE: SCN-08
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UnitResult {
    pub unit_id: BlockId,
    pub output_kind: OutputKind,
    pub translated_payload: String,
    pub warnings: Vec<String>,
}

/// What the provider did with a unit.
///
/// The serde shape is `snake_case` — `"translated"`, `"preserved"`,
/// `"partially_translated"`, `"failed_needs_fallback"` — matching the wire
/// vocabulary the Structured Output schema already uses, so a stored entry and
/// a provider response spell the same value the same way (DCR-0028 §2).
///
/// TRACE: SCN-08
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    Translated,
    Preserved,
    PartiallyTranslated,
    FailedNeedsFallback,
}

/// Entry in the active profile's glossary. Wire form (TOML/JSON) uses
/// `source` / `target` field names; the Rust field names keep the
/// `_term` suffix for clarity and are mapped via `#[serde(rename)]`.
///
/// TRACE: SCN-09
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlossaryEntry {
    #[serde(rename = "source")]
    pub source_term: String,
    #[serde(rename = "target")]
    pub target_term: String,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub scope: GlossaryScope,
    /// Which sections a [`GlossaryScope::ConditionalOnSection`] entry applies
    /// to: heading-text selectors, compared trimmed and case-folded against
    /// every heading in a section's heading stack (DCR-0027 G1–G3). Matching
    /// the *stack* rather than the innermost heading is what gives subsection
    /// inheritance: a term selected by `"Installation"` also applies inside
    /// `### Windows` beneath `## Installation`.
    ///
    /// Required non-empty for the section scope — an entry whose selector list
    /// is empty after normalization cannot mean what it says and is dropped
    /// with a warning. Meaningless for [`GlossaryScope::GlobalAcrossDocument`],
    /// where a non-empty list is cleared with a warning instead of silently
    /// narrowing an entry the author asked to apply everywhere.
    ///
    /// Never rendered: the compiled prompt carries a section-scoped entry's
    /// bullet in its sections and nothing at all elsewhere, so scope and
    /// selectors are *filtering* inputs, never prompt text (ADR-0014 as
    /// amended, DCR-0027 G8). They are likewise not cache identity — they
    /// change no byte the model reads (contracts.md §5a).
    ///
    /// TRACE: DCR-0027
    #[serde(default)]
    pub sections: Vec<String>,
}

/// Scope of a glossary entry. Wire values are `"global"` or
/// `"section"` — long internal names are preserved as deserialization
/// aliases so existing JSON dumps still round-trip.
///
/// TRACE: SCN-09
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum GlossaryScope {
    #[default]
    #[serde(rename = "global", alias = "global_across_document")]
    GlobalAcrossDocument,
    #[serde(rename = "section", alias = "conditional_on_section")]
    ConditionalOnSection,
}

/// Preflight request for candidate-glossary extraction (OI-0026).
///
/// Handed to [`Translator::extract_glossary`] once per run, before any
/// batch is built, so the harvested terms can be merged into the run's
/// glossary and therefore reach every batch's system prompt and both
/// glossary-sensitive `CacheKey` components identically.
///
/// TRACE: SCN-09
/// TRACE: OI-0026
#[derive(Debug, Clone)]
pub struct GlossaryExtractionRequest {
    /// Source document text, pre-truncated by the pipeline to
    /// [`MAX_EXTRACTION_SOURCE_BYTES`] (char-boundary safe). Untrusted
    /// data; providers must frame it as data, never instructions
    /// (invariant 7).
    pub source_text: String,
    /// Truncation marker so providers/report can surface partial harvests.
    pub source_truncated: bool,
    pub source_language: String,
    pub target_language: String,
    /// Source terms already pinned by the static profile glossary, so the
    /// model does not waste term slots on them.
    pub existing_terms: Vec<String>,
    /// Hard cap on returned terms; providers should also stamp it into
    /// their Structured Output schema (`maxItems`).
    pub max_terms: u32,
}

/// Default cap on auto-extracted glossary terms (OI-0026).
///
/// 24 terms ≈ ≤ ~600 tokens of glossary section appended to **every**
/// batch's system prompt (each rendered line is ~15–25 tokens); that
/// recurring per-batch cost, not the single extraction call, is the real
/// cost lever.
///
/// TRACE: OI-0026
pub const DEFAULT_MAX_AUTO_GLOSSARY_TERMS: u32 = 24;

/// Byte ceiling on the source excerpt handed to
/// [`Translator::extract_glossary`] (OI-0026).
///
/// 128 KiB (≈ 30k–45k tokens) covers the overwhelming majority of real
/// documents whole; larger inputs yield a partial harvest that degrades
/// gracefully (missed late-document terms = the pre-OI-0026 status quo)
/// and is flagged via [`GlossaryExtractionRequest::source_truncated`].
///
/// TRACE: OI-0026
pub const MAX_EXTRACTION_SOURCE_BYTES: usize = 128 * 1024;

/// Cache-relevant identity of a `Translator` instance. Two translator
/// instances that can return different output for the same
/// `TranslationBatch` MUST have unequal fingerprints; instances that
/// are interchangeable SHOULD share one. Over-distinguishing is safe
/// (worst case: a redundant re-translation); under-distinguishing lets
/// a shared cache replay another provider's output (R0008-0002).
///
/// The serde shape is `#[serde(transparent)]` — the composed string itself,
/// with no wrapper object (DCR-0028 §2). That string is already injectively
/// framed by [`ProviderFingerprint::new`], so a disk-backed cache transports
/// it verbatim rather than re-encoding an axis: nothing about the value's
/// identity is created or lost by writing it down. Deserialization is
/// deliberately unvalidated for the same reason — the only producer is
/// `new` / `from_type_name`, and a round trip must reproduce exactly what a
/// prior run wrote, framing included.
///
/// EXT-2026-07 P1-4
/// TRACE: DCR-0028
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ProviderFingerprint(String);

impl ProviderFingerprint {
    /// Compose from a provider family label plus its config parts.
    ///
    /// The composition is **injective**: every part — the family label
    /// included — is written as its byte length, then a `\u{1F}`, then the
    /// part itself, so no arrangement of part *contents* can reproduce the
    /// string another arrangement of *parts* produces. That is the
    /// string-space form of the discipline `pipeline::context_hash` and
    /// the glossary digest use on their byte buffers (ti 53d495); the
    /// separator is kept only so the value stays readable in a log line.
    ///
    /// R0002-0007: a bare `\u{1F}` join was ambiguous, whatever the old
    /// doc comment claimed — `new("p", &["a\u{1F}b", "c"])` and
    /// `new("p", &["a", "b\u{1F}c"])` were the same fingerprint. Two
    /// configurations sharing a namespace is the under-distinguishing
    /// direction, the one that lets a shared cache replay foreign output.
    pub fn new(provider: &str, config_parts: &[&str]) -> Self {
        let mut s = String::new();
        push_framed(&mut s, provider);
        for p in config_parts {
            push_framed(&mut s, p);
        }
        Self(s)
    }

    /// Default identity for single-configuration translators: the
    /// reserved `type` family with the type name as its only part.
    ///
    /// Literally `new("type", &[name])` — a provider family that really
    /// does call itself `type` shares this namespace by construction
    /// rather than by a framing accident.
    pub fn from_type_name(name: &str) -> Self {
        Self::new("type", &[name])
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Append one framed part to a [`ProviderFingerprint`] string: its byte
/// length, the `\u{1F}` separator, then the part. The length prefix is
/// what makes the composition injective.
fn push_framed(s: &mut String, part: &str) {
    s.push_str(&part.len().to_string());
    s.push('\u{1F}');
    s.push_str(part);
}

/// Which token encoder approximates this provider's tokenizer for
/// batch-budget estimation. Budget packing is soft (estimates, not
/// enforcement), so an approximation is acceptable — but it must be a
/// *deliberate* one rather than a model-name guess made by the engine
/// on a provider it knows nothing about (OI-0029).
///
/// Deliberately NOT part of [`ProviderFingerprint`] / `CacheKey`:
/// batching shape does not participate in content identity (same
/// precedent as `ProfileBatching`). For this hint the justification is
/// the soft-cap one above — budgets are estimates, so an approximation
/// cannot change what a response says. That argument does **not** cover
/// the one batching field which leaves the engine as a hard,
/// provider-enforced request parameter, `[batching].target_output_tokens`;
/// contracts.md §1 *Output ceiling and cache identity* records why that
/// field is exempt on a narrower argument (R0001-0004).
///
/// TRACE: SCN-10
/// TRACE: OI-0029
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenizerHint {
    /// tiktoken `o200k_base` (gpt-4o / gpt-5 / o-series generations).
    O200kBase,
    /// tiktoken `cl100k_base` (GPT-4 / 3.5 era; also the documented
    /// approximation default for non-OpenAI providers).
    Cl100kBase,
}

/// The provider boundary. Stable from `0.1.0`.
///
/// TRACE: ADR-0002
#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    /// Translate one batch.
    ///
    /// `cancel` is the run's cancellation token (DCR-0024). It is **live**
    /// for the duration of the call and shared with every other call of the
    /// same run, so an implementation must not cancel it — only observe it.
    ///
    /// Honoring it is a SHOULD, not a MUST, and the honest way to honor it is
    /// to race the provider round-trip:
    ///
    /// ```ignore
    /// tokio::select! {
    ///     biased;
    ///     _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
    ///     r = self.round_trip(&batch) => r,
    /// }
    /// ```
    ///
    /// `biased;` is not decoration: it polls the token first, so an
    /// already-cancelled run never issues the request at all.
    /// `CancellationToken::run_until_cancelled` is biased the other way — it
    /// polls the inner future first — and would send it.
    ///
    /// An implementation that ignores the argument is still cancelled — the
    /// pipeline races this future against the same token and **drops** it,
    /// which aborts an in-flight `reqwest`-style request by construction. What
    /// honoring the token buys is (a) a typed answer instead of a silently
    /// dropped future, (b) correctness for an implementation whose work is
    /// *not* drop-cancellable (an inner `tokio::spawn`, a `spawn_blocking`
    /// client, an internal queue), and (c) correctness when the implementation
    /// is called directly rather than through [`crate::translate`].
    ///
    /// Returning [`TranslatorError::Cancelled`] when the token was **not**
    /// cancelled is legal (an implementation may hold a cancellation source of
    /// its own) and is terminal: the pipeline never re-dispatches it.
    ///
    /// TRACE: DCR-0024
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError>;

    /// Cache-namespace identity of this instance. The default derives
    /// it from the implementing type's name, which is correct only
    /// when every instance of the type is interchangeable. Implementors
    /// whose instances can differ (configurable model, endpoint, API
    /// surface, prompt template) MUST override this to cover every
    /// output-affecting configuration axis.
    ///
    /// EXT-2026-07 P1-4
    fn fingerprint(&self) -> ProviderFingerprint {
        ProviderFingerprint::from_type_name(std::any::type_name::<Self>())
    }

    /// Encoder hint for batch-budget token estimation. `None` (the
    /// default) falls back to the OpenAI-model-name heuristic over
    /// [`crate::TranslateOptions::model_id`]
    /// (`encoder_for`), which resolves unknown names to
    /// `cl100k_base` — a deliberate, documented approximation, adequate
    /// because batch budgets are soft caps. Providers whose models are
    /// not OpenAI-shaped SHOULD override this instead of relying on a
    /// heuristic that knows nothing about their tokenizer.
    ///
    /// Not an output-affecting axis: it changes only how units are packed
    /// into batches, never a unit's content, so it stays out of
    /// [`Translator::fingerprint`].
    ///
    /// TRACE: OI-0029
    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        None
    }

    /// Optional preflight: harvest recurring source terminology and pin
    /// target-language renderings, to be merged into the run's glossary
    /// (static profile entries win on conflict — see
    /// [`crate::profile::merge_auto_glossary`]).
    ///
    /// `Ok(None)` means "this translator does not support extraction" (the
    /// default). `Ok(Some(vec![]))` means "supported, nothing salient
    /// found" — the two are kept distinct so the report and CLI can tell
    /// "provider can't" from "provider found nothing".
    ///
    /// Errors NEVER abort the run: the pipeline records the failure on
    /// `ValidationReport.auto_glossary` and proceeds with the static
    /// glossary only (degrade-like-cache-errors). This is deliberately
    /// *not* an ADR-0017 batch-terminal failure — no batch was dispatched,
    /// output coverage is unaffected, and the degraded state equals the
    /// opted-out baseline. Note the report is the *only* channel for it:
    /// unlike `Ok(None)`, an `Err(_)` gets no `tracing` record, because the
    /// reference CLI prints it unconditionally and two copies of one
    /// sentence is what that costs (ti 33e178, contracts.md §6).
    ///
    /// Implementors: the request's `source_text` is untrusted document
    /// data (invariant 7) and must be framed as data, never instructions.
    /// The returned entries' `scope` is ignored — the merge forces
    /// [`GlossaryScope::GlobalAcrossDocument`] (ADR-0014).
    ///
    /// `cancel` is the run's token, on exactly [`Translator::translate_batch`]'s
    /// terms. One asymmetry is worth stating: an extraction *error* degrades
    /// the run, but an extraction *cancellation* does not — the pipeline
    /// re-checks the token after this call returns and aborts with
    /// [`crate::TransyncError::Cancelled`] whatever this method answered, so a
    /// cancelled preflight never resolves into "proceeded on the static
    /// glossary" (DCR-0024).
    ///
    /// TRACE: SCN-09
    /// TRACE: OI-0026
    /// TRACE: DCR-0024
    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let _ = (req, cancel);
        Ok(None)
    }
}

/// Errors raised by a [`Translator`] implementation.
///
/// Every variant names **why the provider stopped**, never what the caller
/// should do about it. Retry policy is deliberately not encoded here: the
/// pipeline's own rule (`pipeline::dispatch` re-dispatches [`Self::Network`]
/// and [`Self::RateLimited`], nothing else) is one consumer of this taxonomy,
/// and an application's user-facing policy is another — see
/// [`TranslatorError::stable_code`] and contracts.md §1.
///
/// Non-exhaustive: new variants may be added in minor releases; match with
/// a wildcard arm. [`Self::Other`] is the standing catch-all for a provider
/// failure this taxonomy does not name, so a new provider quirk never has to
/// wait for a variant.
///
/// TRACE: ADR-0002
/// TRACE: ti 1a85f3
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum TranslatorError {
    #[error("network: {0}")]
    Network(String),

    #[error("authentication: {0}")]
    Authentication(String),

    #[error("rate limited (retry_after = {retry_after:?})")]
    RateLimited { retry_after: Option<Duration> },

    #[error("malformed response: {0}")]
    MalformedResponse(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    /// The provider's own content policy ended generation before an answer
    /// existed — not the model declining (that is [`Self::ModelRefused`]),
    /// and not a transport fault. Nothing in the request moves it, so there
    /// is no operator remediation to offer.
    #[error("content filtered: {0}")]
    ContentFiltered(String),

    /// The output-token ceiling was exhausted before the answer was
    /// complete. The ceiling rides in the request, so the operator knob
    /// (`[batching].target_output_tokens`) is the remediation.
    #[error("output ceiling exhausted: {0}")]
    OutputCeilingExhausted(String),

    /// The request did not fit the model's **context window** — input plus
    /// requested output, not output alone.
    ///
    /// Deliberately adjacent to [`Self::OutputCeilingExhausted`], because
    /// the pair is the one an operator must not confuse and the difference
    /// is the whole reason this variant exists: both are terminal, both are
    /// about tokens, and they name **opposite knobs**. An exhausted output
    /// ceiling says the answer was cut off and points at
    /// `[batching].target_output_tokens`; an exceeded context window says the
    /// request was too large to answer at all and points at the batching
    /// configuration that decides how much goes into one — the `[batching]`
    /// token budget and `max_units_per_batch`. Reporting the second as the
    /// first would name a knob that cannot help, which is exactly the
    /// actively-harmful classification DCR-0023 exists to remove; reporting
    /// it as [`Self::Other`] would name no knob at all, when there is one.
    ///
    /// Terminal: a verbatim resubmission (ADR-0009) is the same request and
    /// does not fit either.
    ///
    /// Not every provider surfaces this as its own signal — one that answers
    /// an over-long request with an HTTP 400 keeps the status-based
    /// [`Self::ProviderRejected`] classification, which is already honest
    /// there. An implementor SHOULD reach for this variant only when the
    /// provider says *this specifically*.
    ///
    /// TRACE: DCR-0029
    /// TRACE: contracts.md §1
    #[error("context window exceeded: {0}")]
    ContextWindowExceeded(String),

    /// The model declined to produce the translation and said so. Carries
    /// the provider's refusal text, already length-capped by the adapter.
    #[error("model refused: {0}")]
    ModelRefused(String),

    /// The response body exceeded the adapter's size cap and was not read.
    #[error("response too large: {0}")]
    ResponseTooLarge(String),

    /// The provider rejected the *request* rather than failing to answer it
    /// — a non-transient client error. `status` is the HTTP status when the
    /// provider speaks HTTP (`Some(404)` is a model name that does not
    /// exist: an operator fault, not a document fault) and `None` for a
    /// provider that has no such code to give.
    #[error("provider rejected the request: {message}")]
    ProviderRejected {
        status: Option<u16>,
        message: String,
    },

    /// This `Translator` has no provider to call, and the run needed one.
    ///
    /// Not a fault of any provider — there was none. It is the answer an
    /// implementation gives when it was constructed **on purpose** without the
    /// means to translate, and the run then reached work only a provider could
    /// do. The CLI's `--offline` is the shipped case: it builds a translator
    /// that refuses, so a run whose every unit hits the cache completes with no
    /// credentials at all, and a run that misses stops **at the miss** with
    /// this error rather than at startup with a credential complaint about a
    /// provider it was never going to call (ti `30a744`).
    ///
    /// Terminal, and unlike [`Self::Unsupported`] it is **not** ambiguous: the
    /// cause is the caller's own configuration and the remediation is theirs
    /// too — supply a provider, or warm the cache the run was pointed at. That
    /// difference is why this is its own variant rather than a message inside
    /// `Unsupported`, whose exit code deliberately admits it cannot tell
    /// whether the configuration or the document is at fault.
    ///
    /// TRACE: ti 30a744
    /// TRACE: contracts.md §1
    #[error("no provider available for this run: {0}")]
    NoProviderAvailable(String),

    /// The call stopped because it was cancelled — the run's
    /// [`CancellationToken`] fired, or the implementation holds a cancellation
    /// source of its own. Terminal by construction: the pipeline never
    /// re-dispatches it, and a verbatim resubmission of a cancelled call is
    /// exactly the work the caller asked to stop.
    ///
    /// It carries no message on purpose. Every other variant exists to explain
    /// a failure the caller did not ask for; this one names an outcome the
    /// caller requested, and there is nothing about it a diagnostic string
    /// could add. Stuffing it into [`Self::Other`] instead was rejected for
    /// the reason DCR-0023 gave for the five causes it pulled out of that
    /// variant: `Other` reads downstream as *unknown*, and the honest handling
    /// of unknown is retry.
    ///
    /// TRACE: DCR-0024
    #[error("cancelled")]
    Cancelled,

    /// A provider failure this taxonomy does not name. Deliberately kept:
    /// without it every future provider quirk would be a new variant, and
    /// the catch-all is what lets one land as a message instead.
    #[error("other: {0}")]
    Other(String),
}

impl TranslatorError {
    /// Stable, machine-readable code for this failure cause.
    ///
    /// Mirrors [`crate::TransyncError::stable_code`] and shares its
    /// vocabulary namespace — `TransyncError::Translator` returns exactly
    /// what this method returns, so these strings cross the process boundary
    /// and are governed by contracts.md §1's append-only rule.
    ///
    /// The match below is exhaustive with **no wildcard arm** on purpose,
    /// exactly as `TransyncError::stable_code`'s is: a new variant cannot
    /// land without being assigned a code.
    ///
    /// TRACE: contracts.md §1
    /// TRACE: ti 1a85f3
    pub fn stable_code(&self) -> &'static str {
        match self {
            TranslatorError::Network(_) => "provider_network",
            TranslatorError::Authentication(_) => "provider_auth",
            TranslatorError::RateLimited { .. } => "provider_rate_limited",
            TranslatorError::MalformedResponse(_) => "provider_malformed_response",
            TranslatorError::Unsupported(_) => "provider_unsupported",
            TranslatorError::ContentFiltered(_) => "provider_content_filtered",
            TranslatorError::OutputCeilingExhausted(_) => "provider_output_ceiling_exhausted",
            TranslatorError::ContextWindowExceeded(_) => "provider_context_window_exceeded",
            TranslatorError::ModelRefused(_) => "provider_model_refused",
            TranslatorError::ResponseTooLarge(_) => "provider_response_too_large",
            TranslatorError::ProviderRejected { .. } => "provider_rejected",
            // Appended 2026-09-02 (ti `30a744`). The vocabulary is
            // append-only, so this is a new code rather than a
            // reinterpretation of `provider_unsupported` — a script
            // branching on that code must not start seeing this case.
            TranslatorError::NoProviderAvailable(_) => "provider_unavailable",
            // Deliberately NOT the engine-side `cancelled` code, though the two
            // variants mean the same thing to a reader. Every provider-side
            // code is `provider_`-prefixed, and the distinction is real: the
            // engine's `cancelled` says the *run* stopped on the caller's own
            // token, while this one says a `Translator` call stopped. A run
            // cancelled on the caller's token never shows this code —
            // `run_pipeline` answers it with `TransyncError::Cancelled` first —
            // but an implementation may answer `Cancelled` off a cancellation
            // source of its own while the run's token never fired, and that is
            // a terminal provider error, so a `translate` caller does see this
            // code by that route.
            TranslatorError::Cancelled => "provider_cancelled",
            TranslatorError::Other(_) => "provider_error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_unit_constructor_defaults_match_first_dispatch_shape() {
        let u = TranslationUnit::new(
            BlockId("p-0001".to_string()),
            BlockKind::Paragraph,
            InputMode::TextFragment,
            "hello".to_string(),
            42,
        );
        assert_eq!(u.unit_id.0, "p-0001");
        assert_eq!(u.source_hash, 42);
        assert_eq!(u.batch_id, BatchId::new(0)); // batcher placeholder semantics
        assert!(u.retry.is_none()); // first-dispatch contract
        let with = u.with_batch_id(BatchId::new(7)).with_retry(RetryContext {
            attempt: 1,
            rejected_by: None,
            reason: None,
        });
        assert_eq!(with.batch_id, BatchId::new(7));
        assert_eq!(with.retry.as_ref().map(|r| r.attempt), Some(1));
    }

    /// R0002-0007: composing a fingerprint is injective, so two
    /// configurations can never land in one cache namespace by moving a
    /// separator-looking byte across a part boundary. The first pair is
    /// the exact one the review constructed — under the old bare-`\u{1F}`
    /// join both spelled `p\u{1F}a\u{1F}b\u{1F}c`.
    ///
    /// Under-distinguishing is the direction that replays foreign output
    /// (contracts.md §1), which is why this is pinned rather than left to
    /// the shipped adapter's field set — `TransyncOpenAI::fingerprint`
    /// happens to be safe (a parsed URL percent-encodes C0 controls, and
    /// the api/effort labels are closed 0x1F-free sets), but the
    /// constructor is public and a third-party `Translator` composes from
    /// whatever its configuration holds.
    #[test]
    fn fingerprint_parts_cannot_be_forged_by_moving_bytes_across_a_boundary() {
        let cases = [
            ProviderFingerprint::new("p", &["a\u{1F}b", "c"]),
            ProviderFingerprint::new("p", &["a", "b\u{1F}c"]),
            ProviderFingerprint::new("p", &["a", "b", "c"]),
            ProviderFingerprint::new("p\u{1F}a", &["b", "c"]),
            ProviderFingerprint::new("p", &["abc"]),
            ProviderFingerprint::new("p", &["ab", "c"]),
            ProviderFingerprint::new("p", &["a", "bc"]),
            // An empty part is a part: three axes where one is unset is
            // not the same instance as two axes.
            ProviderFingerprint::new("p", &["a", "", "c"]),
            ProviderFingerprint::new("p", &["a", "c"]),
            // A part that spells out a length prefix of its own.
            ProviderFingerprint::new("p", &["1\u{1F}a"]),
        ];

        for (i, a) in cases.iter().enumerate() {
            for (j, b) in cases.iter().enumerate().skip(i + 1) {
                assert_ne!(
                    a,
                    b,
                    "case {i} and case {j} are different provider \
                     configurations and must not share a cache namespace:\n  \
                     {:?}\n  {:?}",
                    a.as_str(),
                    b.as_str()
                );
            }
        }
    }

    /// The type-name default is the reserved `type` family, stated as
    /// such: it is `new("type", &[name])` by construction, so the
    /// equivalence is a documented namespace rather than a framing
    /// accident.
    #[test]
    fn the_type_name_default_is_the_reserved_type_family() {
        assert_eq!(
            ProviderFingerprint::from_type_name("some::Translator"),
            ProviderFingerprint::new("type", &["some::Translator"])
        );
        assert_ne!(
            ProviderFingerprint::from_type_name("a"),
            ProviderFingerprint::from_type_name("b")
        );
        assert!(
            ProviderFingerprint::from_type_name("some::Translator")
                .as_str()
                .contains("some::Translator"),
            "the value stays readable in a log line"
        );
    }
}
