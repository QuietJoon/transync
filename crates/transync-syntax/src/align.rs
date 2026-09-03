//! Alignment-map construction. The on-disk JSON shape is the durable wire
//! contract between the renderer and the JS sync engine.
//!
//! TRACE: contracts.md §3
//! TRACE: ADR-0001

use crate::id::{BlockId, SourceFormat, Spelling};
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
///
/// Bumped 1.2.0 → 1.3.0 for three additive changes (ti 490d97 / DCR-0038):
/// the per-row `source_format`, the map-level `input_format`, and the
/// `"title"` `block_kind` value riding the `non-sync` role shipped since
/// 1.0.
pub const ALIGNMENT_SCHEMA_VERSION: &str = "1.3.0";

/// On-disk JSON shape — `schema_version 1.3.0`.
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
    /// Which intake produced this run — the run-level fact a pane consumer
    /// branches on (schema 1.3.0, ti 490d97). `#[serde(default)]` reads a
    /// pre-1.3.0 map as `Markdown`, which is the format every pre-1.3.0 run
    /// actually had, so the default is a fact rather than a guess.
    #[serde(default)]
    pub input_format: SourceFormat,
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
            // The enum's own `#[default]`, spelled explicitly because this
            // literal lists every field.
            input_format: SourceFormat::Markdown,
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
    /// (schema 1.x pins it (1.3.0 at this writing), `contracts.md` §3) for a future nested-anchor
    /// scheme, paired with the reserved `child-only` [`SyncRole`].
    pub parent_id: Option<BlockId>,
    /// How the source SPELLED this block — `"markdown"` or `"html"` (schema
    /// 1.3.0, ti 490d97). Mandatory in every map this engine emits; the
    /// `Option` exists solely so `Deserialize` accepts a pre-1.3.0 map.
    /// Reader rule for an absent value: `block_kind == "html" ? html :
    /// markdown` — never a bare default to markdown, which would mislabel
    /// every 1.2.0 html-island row. Distinct from
    /// [`AlignmentMap::input_format`]: a Markdown run's map legitimately
    /// carries html-spelled island rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_format: Option<SourceFormat>,
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
            // The row is the block's SPELLING (spec §3): an island inside a
            // Markdown document says html here while the map says markdown.
            // Exhaustive by charter; `Html { .. }` names the variant, the
            // rest pattern covers its field.
            source_format: Some(match block.spelling {
                Spelling::Markdown => SourceFormat::Markdown,
                Spelling::Html { .. } => SourceFormat::Html,
            }),
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
        input_format: doc.format,
        generator: GeneratorMeta::default(),
        blocks,
        validation_summary: summary,
    }
}

/// The alignment row's `sync_role` for a block kind.
///
/// **Exhaustive on purpose — there is no `_` arm** (ti `9ffb97`).
/// [`SyncRole`] is wire-visible: it decides whether a row gets a DOM anchor
/// and whether the engine counts the row as synchronizable. Behind a default,
/// adding a `BlockKind` variant compiles clean and the new kind silently
/// starts anchoring, with nothing to say that a role was never chosen for it.
///
/// That is not a hypothetical risk. Wave 2's plan documented this exact
/// failure prospectively, in the comment it dictated for the `Title` arm:
/// the default "would have made this row anchoring — the precise opposite of
/// the decision — without one word of warning from the compiler". `Title` was
/// caught because a human wrote the arm and a test pinned it. The variant
/// after it would have had neither. Now the compiler asks.
///
/// The rule is narrow: a **dispatch that assigns behaviour per variant** must
/// not have a default. Membership tests are a different shape and stay as
/// they are — `unit::context::document_title` asks "is this block a title"
/// with `matches!`, and an exhaustive match returning `bool` there would be
/// noise, not safety.
///
/// [`SyncRole::ChildOnly`] is never returned, and that is contractual rather
/// than an omission: `contracts.md` §4 records it as RESERVED for a future
/// nested-anchor scheme, since the current parser is leaf-block (DCR-0007).
fn sync_role_for(kind: &crate::id::BlockKind) -> SyncRole {
    use crate::id::BlockKind::*;
    match kind {
        ThematicBreak => SyncRole::NonSync,
        // D5 (ti 490d97 wave 2): a `<title>` is translated and aligned but is
        // NOT page content — the browser chrome renders it, so the pane has
        // nothing to anchor. Pinned by `sync_role_tests`; it was the arm that
        // proved the old `_ => Anchor` default was answering for kinds nobody
        // had decided about.
        Title => SyncRole::NonSync,
        Blockquote => SyncRole::Container,
        // R0006-0042: items render as top-level `<li>` sync anchors (no
        // list-level row exists), so `anchor` is the honest role —
        // `child-only` contradicted the DOM.
        ListItem { .. } => SyncRole::Anchor,
        // A5: a Skipped placeholder is rendered as a `<pre>` in both panes
        // and must anchor scroll there (owner decision) — never non-sync.
        Skipped { .. } => SyncRole::Anchor,
        // Page content that renders as one addressable element in each pane.
        // Spelled out rather than defaulted: every name here is a decision
        // someone made, and the next variant added to `BlockKind` has to be
        // one too before this compiles.
        Heading1
        | Heading2
        | Heading3
        | Heading4
        | Heading5
        | Heading6
        | Paragraph
        | Table
        | CodeBlock { .. }
        | Image
        | Html => SyncRole::Anchor,
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

// ti 490d97 wave 2 (spec §3 / decision D5), and ti `9ffb97` after it.
// `sync_role_for` used to end in `_ => SyncRole::Anchor`, so a new kind was
// silently an ANCHORING row unless someone wrote the arm — the precise
// opposite of the decision for `<title>`. It is exhaustive now, so the
// compiler DOES say so. What the compiler still cannot say is whether an arm
// that exists is RIGHT, which is what these tests assert.
#[cfg(test)]
mod sync_role_tests {
    use super::*;
    // `align.rs` imports `crate::id::BlockId` at module scope, not `BlockKind`,
    // so `use super::*` does not bring it in — same explicit import
    // `skipped_row_tests` above already carries.
    use crate::id::BlockKind;

    #[test]
    fn a_title_row_is_non_sync_and_ordinary_page_content_anchors() {
        assert_eq!(
            sync_role_for(&BlockKind::Title),
            SyncRole::NonSync,
            "D5: the <title> is rendered by browser chrome, not by the pane — \
             there is nothing there to anchor",
        );
        // The contrast that makes the assertion above non-vacuous. These used
        // to reach a `_ => Anchor` catch-all and now reach a named arm; the
        // answers are unchanged, which is the point of pinning them across
        // ti `9ffb97`.
        assert_eq!(sync_role_for(&BlockKind::Heading1), SyncRole::Anchor);
        assert_eq!(sync_role_for(&BlockKind::Paragraph), SyncRole::Anchor);
        // The kind that has answered `non-sync` since schema 1.0, so the row
        // shape a title ships is not a new one.
        assert_eq!(sync_role_for(&BlockKind::ThematicBreak), SyncRole::NonSync);
        assert_eq!(sync_role_for(&BlockKind::Blockquote), SyncRole::Container);
    }

    /// Every `BlockKind` variant, each named once, with the role it answers
    /// today. The compiler already forbids a missing arm in `sync_role_for`
    /// (ti `9ffb97`); this pins what the arms SAY, so a future variant cannot
    /// be waved through by copying a neighbour's role without anyone noticing
    /// the wire behaviour it inherits.
    #[test]
    fn every_block_kind_answers_the_role_it_was_given() {
        let cases: &[(BlockKind, SyncRole)] = &[
            (BlockKind::Heading1, SyncRole::Anchor),
            (BlockKind::Heading2, SyncRole::Anchor),
            (BlockKind::Heading3, SyncRole::Anchor),
            (BlockKind::Heading4, SyncRole::Anchor),
            (BlockKind::Heading5, SyncRole::Anchor),
            (BlockKind::Heading6, SyncRole::Anchor),
            (BlockKind::Paragraph, SyncRole::Anchor),
            (BlockKind::Table, SyncRole::Anchor),
            (
                BlockKind::CodeBlock {
                    info: None,
                    fenced: true,
                },
                SyncRole::Anchor,
            ),
            (
                BlockKind::ListItem {
                    ordered: false,
                    task: None,
                },
                SyncRole::Anchor,
            ),
            (BlockKind::Blockquote, SyncRole::Container),
            (BlockKind::ThematicBreak, SyncRole::NonSync),
            (BlockKind::Image, SyncRole::Anchor),
            (BlockKind::Title, SyncRole::NonSync),
            (BlockKind::Html, SyncRole::Anchor),
            (
                BlockKind::Skipped {
                    label: "front-matter".to_string(),
                },
                SyncRole::Anchor,
            ),
        ];
        for (kind, want) in cases {
            assert_eq!(sync_role_for(kind), *want, "role for {kind:?}");
        }
        // `child-only` is RESERVED by contracts.md §4 for a future
        // nested-anchor scheme, so nothing may answer it while the parser is
        // leaf-block (DCR-0007).
        assert!(
            cases.iter().all(|(_, r)| *r != SyncRole::ChildOnly),
            "sync_role_for must not emit the reserved `child-only` role"
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

// ti 490d97 wave 6 (spec §3): schema 1.3.0's two additive fields. The row
// field is the block's SPELLING; the map field is the run's INTAKE — two
// facts, two names, never one doing double duty (the synthesis found two
// areas colliding on one name here, and the ruling is both fields).
#[cfg(test)]
mod wire_format_tests {
    use super::*;
    use crate::id::{SourceFormat, assign_block_ids};

    fn built_map(mut doc: crate::parser::Document) -> AlignmentMap {
        assign_block_ids(&mut doc);
        let outcomes = crate::outcome::html_outcomes(&doc);
        build_alignment_map(
            &doc,
            &HashMap::new(),
            &crate::regen::BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &outcomes,
        )
    }

    /// A Markdown run: the map says markdown, a prose row says markdown, and
    /// an html-ISLAND row says html — the row is the spelling, not the run.
    /// This is what makes the reader rule for absent values non-arbitrary:
    /// a bare "default markdown" would mislabel exactly this row.
    #[test]
    fn a_markdown_runs_rows_carry_their_spelling_and_the_map_carries_the_intake() {
        let map = built_map(crate::parser::parse("prose\n\n<div>island</div>\n").expect("parses"));
        assert_eq!(map.input_format, SourceFormat::Markdown);
        let prose = map
            .blocks
            .iter()
            .find(|r| r.block_kind == "paragraph")
            .expect("p row");
        assert_eq!(prose.source_format, Some(SourceFormat::Markdown));
        let island = map
            .blocks
            .iter()
            .find(|r| r.block_kind == "html")
            .expect("island row");
        assert_eq!(
            island.source_format,
            Some(SourceFormat::Html),
            "an island inside a Markdown document is html-SPELLED; the run is still markdown",
        );
    }

    /// An HTML-intake document: every row html, the map html, and the title
    /// row carries the D5 shape — kind "title", role non-sync, format html.
    #[test]
    fn an_html_intake_map_says_html_everywhere_and_the_title_row_is_non_sync() {
        let map = built_map(crate::intake::html::parse(
            "<html><head><title>T</title></head><body><p>x</p></body></html>\n",
        ));
        assert_eq!(map.input_format, SourceFormat::Html);
        for row in &map.blocks {
            assert_eq!(
                row.source_format,
                Some(SourceFormat::Html),
                "{}: format == Html implies every spelling is Html (spec §3's invariant)",
                row.source_block_id.0,
            );
        }
        let title = map
            .blocks
            .iter()
            .find(|r| r.block_kind == "title")
            .expect("title row");
        assert_eq!(title.sync_role, SyncRole::NonSync);
    }

    /// Backward: a pre-1.3.0 map (no source_format, no input_format) still
    /// deserializes — input_format reads as the format every pre-1.3.0 run
    /// had, and the row's absence stays None (the documented reader rule is
    /// the CONSUMER's, applied at read time, never invented by serde).
    #[test]
    fn a_pre_1_3_0_map_still_deserializes() {
        // The §3 example row, verbatim shape, minus the two new fields.
        let json = r#"{
          "schema_version": "1.2.0",
          "document_id": "a91f2c0d2e1bbb40",
          "source_language": "en",
          "target_language": "ko",
          "detected_source_language": null,
          "generator": { "name": "transync", "version": "0.4.0" },
          "blocks": [{
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
          }],
          "validation_summary": {
            "total_units": 1, "translated": 1, "preserved": 0,
            "partially_translated": 0, "fallback_source": 0, "retried_units": 0
          }
        }"#;
        let map: AlignmentMap = serde_json::from_str(json).expect("a 1.2.0 map deserializes");
        assert_eq!(map.input_format, SourceFormat::Markdown);
        assert_eq!(map.blocks[0].source_format, None);
    }

    /// Forward: the wire keys and kebab-case values, and the None-row key
    /// omission — the exact bytes a JS consumer sees.
    #[test]
    fn the_wire_keys_are_kebab_case_and_an_absent_row_value_omits_the_key() {
        let map = built_map(crate::parser::parse("prose\n\n<div>island</div>\n").expect("parses"));
        let v = serde_json::to_value(&map).expect("serializes");
        assert_eq!(v["schema_version"], "1.3.0");
        assert_eq!(v["input_format"], "markdown");
        let rows = v["blocks"].as_array().expect("array");
        assert!(rows.iter().any(|r| r["source_format"] == "markdown"));
        assert!(rows.iter().any(|r| r["source_format"] == "html"));

        // A hand-built None row (same crate; #[non_exhaustive] allows it here)
        // omits the key rather than writing null — round-trip honesty for the
        // one shape only a pre-1.3.0 file can hold.
        let mut none_row = map.blocks[0].clone();
        none_row.source_format = None;
        let rv = serde_json::to_value(&none_row).expect("serializes");
        assert!(
            rv.get("source_format").is_none(),
            "a None row must omit the key, not write null: {rv}",
        );
    }
}
