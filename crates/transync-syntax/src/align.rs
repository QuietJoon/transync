//! Alignment-map construction. The on-disk JSON shape is the durable wire
//! contract between the renderer and the JS sync engine.
//!
//! TRACE: contracts.md §3
//! TRACE: ADR-0001

use crate::id::BlockId;
use crate::outcome::{HtmlOutcome, is_translatable_block};
use crate::parser::Document;
use crate::regen::BlockOffsets;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub use crate::parser::ranges::ByteRange;

/// Current alignment-map wire schema version. Single source of truth so
/// the `Default` impl and [`build_alignment_map`] cannot drift. Bumped
/// 1.0.0 → 1.1.0 for the additive `"skipped"` `block_kind` value + the
/// `data-skipped` DOM attribute (A8); old engines accept it as
/// forward-drift with a warning (OI-0024).
///
/// Bumped 1.1.0 → 1.2.0 for the additive `"html"` `block_kind` value
/// (HTML-content translation, spec 2026-08-03 §5).
pub const ALIGNMENT_SCHEMA_VERSION: &str = "1.2.0";

/// On-disk JSON shape — `schema_version 1.2.0`.
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct — new fields may be added in minor releases (additive
/// on the wire per the schema-version policy).
///
/// TRACE: SCN-12
/// TRACE: SCN-13
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlignmentMap {
    pub schema_version: String,
    pub document_id: String,
    pub source_language: String,
    pub target_language: String,
    pub detected_source_language: Option<String>,
    pub generator: GeneratorMeta,
    pub blocks: Vec<AlignmentBlock>,
    pub validation_summary: ValidationSummary,
}

impl Default for AlignmentMap {
    fn default() -> Self {
        Self {
            schema_version: ALIGNMENT_SCHEMA_VERSION.to_string(),
            document_id: String::new(),
            source_language: String::new(),
            target_language: String::new(),
            detected_source_language: None,
            generator: GeneratorMeta::default(),
            blocks: Vec::new(),
            validation_summary: ValidationSummary::default(),
        }
    }
}

/// Generator identifier embedded in the alignment map.
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct.
///
/// TRACE: SCN-12
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct GeneratorMeta {
    pub name: String,
    pub version: String,
}

impl Default for GeneratorMeta {
    fn default() -> Self {
        Self {
            name: "transync".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Per-block alignment row.
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct — new fields may be added in minor releases.
///
/// TRACE: SCN-12
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct AlignmentBlock {
    pub source_block_id: BlockId,
    /// **Equal to `source_block_id`, and that is normative for schema 1.x**
    /// (ticket `d3acc3`, 2026-08-09; ADR-0001's amendment, `contracts.md`
    /// §3). Block ids survive translation, so the two documents' block sets
    /// are the same set under the same names, and consumers pair the panes
    /// by looking the *same* `data-sync-id` up on the other side rather than
    /// routing through this field.
    ///
    /// The indirection is RESERVED — like [`parent_id`] and the
    /// [`SyncRole::ChildOnly`] role — for a future revision in which the two
    /// documents may genuinely diverge. Until then there is nothing for it
    /// to express, so a second, disagreeable way of saying "identity" is
    /// exactly what it must not become. The reference JS engine refuses a
    /// schema-1.x map whose row contradicts the identity.
    ///
    /// [`parent_id`]: AlignmentBlock::parent_id
    pub target_block_id: BlockId,
    pub block_kind: String,
    pub source_order: u32,
    pub target_order: u32,
    pub source_range: ByteRange,
    pub target_range: ByteRange,
    pub sync_role: SyncRole,
    pub fallback_status: FallbackStatus,
    /// RESERVED, always `null`. The parser is leaf-block — every source
    /// block is top-level — so nothing populates this. It stays on the wire
    /// (schema 1.2.0 pins it, `contracts.md` §3) for a future nested-anchor
    /// scheme, paired with the reserved `child-only` [`SyncRole`].
    pub parent_id: Option<BlockId>,
}

/// Whether the block is a primary scroll anchor, a container, etc.
///
/// TRACE: SCN-13
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SyncRole {
    Anchor,
    Container,
    ChildOnly,
    NonSync,
}

/// What happened when this block was translated.
///
/// TRACE: SCN-08
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackStatus {
    Translated,
    Preserved,
    PartiallyTranslated,
    FallbackSource,
}

/// Tally of validation outcomes across the document.
///
/// Non-exhaustive: produced by the engine; consumers read fields rather
/// than construct — new tallies may be added in minor releases.
///
/// TRACE: SCN-07
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ValidationSummary {
    pub total_units: u32,
    pub translated: u32,
    pub preserved: u32,
    pub partially_translated: u32,
    pub fallback_source: u32,
    pub retried_units: u32,
}

/// Construct an alignment map from the source IR + the per-block final
/// statuses + regenerated-MD offsets.
///
/// `statuses` maps a block ID to the status the pipeline finalized for it;
/// every unit it produced belongs in the map, fallbacks included. Blocks
/// absent from the map get a status derived from `html_outcomes` and the
/// translatability predicate below.
///
/// `html_outcomes` must be [`crate::outcome::html_outcomes`]'s result for this
/// same `doc` — the map that decided which html blocks became units. Passing
/// a stale or foreign map desynchronizes the rows from what was batched
/// (spec §3.2).
///
/// `offsets` must be [`crate::regen::regenerate`]'s result for this same
/// `doc`. That function splices every block, so the lookup below is total for
/// any in-pipeline call; a block it does not cover gets the `0..0` range this
/// function has always used and a `tracing::warn` naming the ids (R0003-0055).
/// The warning, not a `Result`, is the signal on purpose: incomplete offsets
/// are programmatic misuse of a two-argument agreement, the empty range is
/// already the *safe* answer (the alternative, falling back to
/// `block.source_range`, indexes the wrong string and panicked the renderer
/// on a char boundary), and making the function fallible would force every
/// caller to handle an error that no shipped path can produce.
///
/// TRACE: SCN-01
/// TRACE: SCN-12
pub fn build_alignment_map(
    doc: &Document,
    statuses: &HashMap<BlockId, FallbackStatus>,
    offsets: &BlockOffsets,
    source_language: &str,
    target_language: &str,
    detected_source_language: Option<String>,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> AlignmentMap {
    let mut blocks = Vec::with_capacity(doc.blocks.len());
    let mut summary = ValidationSummary::default();
    let mut missing_offsets: Vec<&str> = Vec::new();

    for (i, block) in doc.blocks.iter().enumerate() {
        // Translatable blocks should always have a row in `statuses`. If
        // a translatable block is missing, that's an internal pipeline
        // bug (a unit was dropped between batching and validation), and
        // we record it explicitly as FallbackSource rather than silently
        // claiming Translated. Blocks this predicate calls non-translatable
        // (thematic-break, image, skipped, and html blocks with no unit
        // behind them) never get a *synthesized* FallbackSource — a real
        // `statuses` row, when one exists, still wins.
        //
        // Spec §3.2: for html the decision is PER BLOCK, keyed off the same
        // `HtmlOutcome` map `build_batches` used, so a row's status and the
        // counters cannot disagree with what was actually batched.
        let translatable = is_translatable_block(block, html_outcomes);
        let fallback_status = if let Some(s) = statuses.get(&block.block_id).copied() {
            s
        } else if translatable {
            FallbackStatus::FallbackSource
        } else {
            match (block.spelling, html_outcomes.get(&block.block_id)) {
                // Spec §3.2: the rewriter errored — placeholder presentation,
                // honest fallback status, uncounted. Keyed on SPELLING since
                // ti 490d97 wave 2: an HTML document's `<p>` whose extraction
                // failed is a `Paragraph`, and a kind-keyed arm would fall
                // through to `Preserved` and claim the block was carried over
                // intact.
                (crate::id::Spelling::Html { .. }, Some(HtmlOutcome::ExtractionFailed(_))) => {
                    FallbackStatus::FallbackSource
                }
                // OI-0002 / A5: never-batched blocks (thematic-break, image,
                // skipped, zero-segment html) are carried over verbatim —
                // "preserved" is the honest status; claiming "translated"
                // misreports blocks the LLM never saw.
                _ => FallbackStatus::Preserved,
            }
        };

        // ONE row constructor for both branches (R0002-0086). Translatability
        // decides what the row is *counted* as, never what it looks like —
        // every field below is identical for a counted and an uncounted
        // block, so writing them twice was a pure drift surface.
        //
        // `regen::regenerate` splices every block, so `offsets` covers the
        // whole block list and the lookup only misses when the caller passed
        // a foreign or empty `BlockOffsets` (the in-crate tests do). The
        // default `0..0` is the safe answer there: falling back to
        // `block.source_range` would be wrong, because those byte offsets
        // index the SOURCE string, whose boundaries differ from the
        // translated string once the model emits multi-byte characters
        // (e.g. Korean) — which panicked the renderer on a char boundary.
        //
        // R0003-0055: safe is not the same as silent. Collect the misses and
        // name them once below, so a caller whose map came from somewhere
        // else learns it from a log line instead of from empty panes.
        let target_range = match offsets.0.get(&block.block_id) {
            Some(r) => *r,
            None => {
                missing_offsets.push(block.block_id.0.as_str());
                ByteRange::default()
            }
        };

        blocks.push(AlignmentBlock {
            source_block_id: block.block_id.clone(),
            target_block_id: block.block_id.clone(),
            block_kind: block.kind.wire_str().to_string(),
            source_order: i as u32,
            target_order: i as u32,
            source_range: block.source_range,
            target_range,
            sync_role: sync_role_for(&block.kind),
            fallback_status,
            parent_id: None,
        });

        // `R0001-0022` in the removed `reviews/reviewed/0001.md` (not the
        // live round's 0022, which is about heading context): summary
        // counts only the units that the pipeline
        // actually batched and translated. Non-translatable blocks
        // (thematic-break, image, skipped) appear in the row stream so the
        // JS engine can render them, but they should not inflate the
        // per-status counters that the CLI uses to decide exit code 3.
        // Spec §5: unit-backed html blocks count; zero-segment and
        // extraction-failed html blocks do not.
        if !translatable {
            continue;
        }

        summary.total_units += 1;
        match fallback_status {
            FallbackStatus::Translated => summary.translated += 1,
            FallbackStatus::Preserved => summary.preserved += 1,
            FallbackStatus::PartiallyTranslated => summary.partially_translated += 1,
            FallbackStatus::FallbackSource => summary.fallback_source += 1,
        }
    }

    if !missing_offsets.is_empty() {
        // One line for the whole document, with a bounded sample: a wholly
        // foreign `BlockOffsets` misses every block, and a warning that
        // prints one id per block is a second failure mode.
        //
        // Larger than `render::MAX_NAMED_IDS` (5), the crate's other bounded
        // id sample, on purpose: the `missing` and `total` fields below carry
        // the magnitude independently, so this sample only has to identify
        // blocks — while a `RenderError` message has to fit its ids *and* its
        // own overflow count into one sentence. Two knobs on two message
        // shapes, neither a contract; they are not meant to be one shared
        // constant.
        const SAMPLE: usize = 8;
        tracing::warn!(
            missing = missing_offsets.len(),
            total = doc.blocks.len(),
            ids = %missing_offsets
                .iter()
                .take(SAMPLE)
                .copied()
                .collect::<Vec<_>>()
                .join(", "),
            "alignment map built with incomplete BlockOffsets: these blocks got an empty \
             target_range. Pass the offsets `regen::regenerate` returned for THIS document."
        );
    }

    let document_id = format!(
        "{:016x}",
        crate::id::source_hash_bytes(doc.source_text.as_bytes())
    );

    AlignmentMap {
        schema_version: ALIGNMENT_SCHEMA_VERSION.to_string(),
        document_id,
        source_language: source_language.to_string(),
        target_language: target_language.to_string(),
        detected_source_language,
        generator: GeneratorMeta::default(),
        blocks,
        validation_summary: summary,
    }
}

fn sync_role_for(kind: &crate::id::BlockKind) -> SyncRole {
    use crate::id::BlockKind::*;
    match kind {
        ThematicBreak => SyncRole::NonSync,
        Blockquote => SyncRole::Container,
        // R0006-0042: items render as top-level `<li>` sync anchors (no
        // list-level row exists), so `anchor` is the honest role —
        // `child-only` contradicted the DOM.
        ListItem { .. } => SyncRole::Anchor,
        // A5: a Skipped placeholder is rendered as a `<pre>` in both panes
        // and must anchor scroll there (owner decision) — never non-sync.
        Skipped { .. } => SyncRole::Anchor,
        _ => SyncRole::Anchor,
    }
}

// A5: a Skipped block gets an honest, anchoring alignment row that is
// never counted as a translated/fallback unit.
#[cfg(test)]
mod skipped_row_tests {
    use super::*;
    use crate::id::BlockKind;

    #[test]
    fn skipped_row_is_preserved_anchor_and_uncounted() {
        // No node maps to Skipped under the current comrak options (html
        // became a real kind); pin the never-batched row synthetically.
        let mut doc = crate::parser::parse("real paragraph\n").expect("parses");
        doc.blocks.insert(
            0,
            crate::parser::Block {
                block_id: BlockId::new("x", 99),
                kind: BlockKind::Skipped {
                    label: "unsupported".to_string(),
                },
                spelling: crate::id::Spelling::Markdown,
                source_range: ByteRange { start: 0, end: 0 },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: crate::parser::AstPath(Vec::new()),
            },
        );
        crate::id::assign_block_ids(&mut doc);
        let map = build_alignment_map(
            &doc,
            &HashMap::new(),
            &BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &crate::outcome::html_outcomes(&doc),
        );

        assert_eq!(map.schema_version, ALIGNMENT_SCHEMA_VERSION);

        let skipped = map
            .blocks
            .iter()
            .find(|b| b.block_kind == "skipped")
            .expect("a skipped alignment row exists");
        assert_eq!(skipped.sync_role, SyncRole::Anchor);
        assert_eq!(skipped.fallback_status, FallbackStatus::Preserved);
        assert!(skipped.parent_id.is_none(), "skipped row has no parent");
        assert!(
            skipped.source_block_id.0.starts_with("x-"),
            "skipped id uses x- prefix, got {}",
            skipped.source_block_id.0,
        );

        // The paragraph is the only counted unit; the skipped row is excluded.
        assert_eq!(
            map.validation_summary.total_units, 1,
            "validation_summary must exclude the skipped block",
        );
    }
}

// Spec §3.2/§5: per-block html accounting — Unit counted; ZeroSegment
// preserved+uncounted; ExtractionFailed fallback_source+uncounted.
#[cfg(test)]
mod html_row_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::outcome::{HtmlOutcome, html_outcomes};
    use crate::parser::parse;
    use std::collections::HashMap;

    fn map_for(src: &str, outcomes: &HashMap<crate::id::BlockId, HtmlOutcome>) -> AlignmentMap {
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            outcomes,
        )
    }

    #[test]
    fn zero_segment_html_row_is_preserved_anchor_and_uncounted() {
        let src = "<!-- note -->\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let map = map_for(src, &outcomes);
        let row = &map.blocks[0];
        assert_eq!(row.block_kind, "html");
        assert_eq!(row.fallback_status, FallbackStatus::Preserved);
        assert_eq!(row.sync_role, SyncRole::Anchor);
        assert_eq!(
            map.validation_summary.total_units, 0,
            "not a unit — uncounted"
        );
    }

    #[test]
    fn extraction_failed_html_row_is_fallback_source_and_uncounted() {
        let src = "<div>x</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let mut outcomes = HashMap::new();
        outcomes.insert(
            doc.blocks[0].block_id.clone(),
            HtmlOutcome::ExtractionFailed("boom".to_string()),
        );
        let map = map_for(src, &outcomes);
        let row = &map.blocks[0];
        assert_eq!(row.block_kind, "html");
        assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
        assert_eq!(map.validation_summary.total_units, 0, "uncounted");
    }

    #[test]
    fn unit_backed_html_block_missing_from_results_is_fallback_and_counted() {
        // The existing internal-bug rule extends to html Unit blocks.
        let src = "<div>real text</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(outcomes.values().next(), Some(&HtmlOutcome::Unit));
        let map = map_for(src, &outcomes);
        assert_eq!(
            map.blocks[0].fallback_status,
            FallbackStatus::FallbackSource
        );
        assert_eq!(map.validation_summary.total_units, 1, "Unit blocks count");
    }
}
