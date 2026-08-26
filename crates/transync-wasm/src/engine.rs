//! Logic layer for the browser surface — plain Rust, zero wasm-bindgen.
//!
//! Everything here is host-testable: no bindgen types cross this module's
//! boundary, so `cargo test -p transync-wasm` exercises the real call
//! sequences on the host and the `#[wasm_bindgen]` wrappers in [`crate`]
//! stay thin enough to be obviously correct by inspection.
//!
//! Two stateless modes, both taking and returning owned strings (spec
//! 2026-08-05 §2):
//!
//! - **View mode** ([`render_pair_impl`]) re-renders both panes from an
//!   already-translated triple (`source_md`, `translated_md`,
//!   `alignment_json`). It is the browser-side twin of what the CLI wrote
//!   into its bundle, and reproduces those fragments byte-identically.
//! - **Edit mode** ([`rebuild_impl`]) owns the whole local loop: regen the
//!   translated Markdown from per-block payloads, build a fresh alignment
//!   map against *those* offsets, then render. Re-using a stale
//!   `alignment.json` after an edit would mis-slice the html/skipped render
//!   arms, which read raw `target_range` bytes — so edit mode never does.
//!
//! # The edit-mode result shape
//!
//! [`RebuildOutput`] carries four always-present strings — the two pane
//! fragments, the regenerated `translated_md`, and the `alignment_json`
//! built against *it* (nested JSON: the caller parses that string a second
//! time, or hands it straight back to [`render_pair_impl`]) — plus one
//! nullable `structure_warning`.
//!
//! That warning is the *only* structural check edit mode makes. None of
//! `transync-core`'s validation layers are available here — the crate
//! depends on `transync-syntax` alone by charter (ADR-0019) — so a payload
//! that changes the document's top-level block topology is regenerated and
//! rendered like any other. It is a warning and never a rejection: the
//! panes in the same result are a real render, and the caller must stay
//! editable so the user can undo the change. `check_top_level_structure`
//! (private, below) documents what the degraded render looks like.
//!
//! TRACE: ADR-0019

use serde::Serialize;
use std::collections::{BTreeSet, HashMap, HashSet};
use transync_syntax::align::{self, AlignmentMap, FallbackStatus};
use transync_syntax::id::{self, BlockId};
use transync_syntax::{outcome, parser, regen, render, walk};

/// What went wrong at the JS boundary. Every variant names the input that
/// failed, because the JS side sees only the stringified message.
///
/// Non-exhaustive: new inputs may grow new rejections.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    /// `source_md` did not parse as GFM.
    #[error("source Markdown did not parse: {0}")]
    Parse(#[from] transync_syntax::error::ParseError),
    /// `alignment_json` was not a serialized [`AlignmentMap`].
    #[error("alignment JSON did not deserialize: {0}")]
    AlignmentJson(#[source] serde_json::Error),
    /// `payloads_json` was not a `{block_id: payload}` object.
    #[error("payloads JSON did not deserialize: {0}")]
    PayloadsJson(#[source] serde_json::Error),
    /// `statuses_json` was not a `{block_id: fallback_status}` object.
    #[error("statuses JSON did not deserialize: {0}")]
    StatusesJson(#[source] serde_json::Error),
    /// An id in `payloads_json` / `statuses_json` names no block in the
    /// parsed source document. Silently ignoring it would drop an edit on
    /// the floor, so it is rejected instead.
    #[error("{field} names block id(s) absent from the source document: {ids}")]
    UnknownBlockId {
        /// Which input carried the offending ids.
        field: &'static str,
        /// A sorted, comma-joined **sample** of the unknown ids, followed by
        /// `(and N more)` when there were more than the sample holds.
        ids: String,
    },
    /// The freshly built alignment map failed to serialize.
    #[error("alignment map did not serialize: {0}")]
    AlignmentSerialize(#[source] serde_json::Error),
    /// The renderer refused the pane: the alignment map does not describe
    /// the parsed source document (a repeated row id, a block with no row,
    /// or a byte range that is not a slice of the pane it indexes), or the
    /// pane's Markdown failed the parser's intake guard.
    ///
    /// View mode is where this earns its keep — `alignment_json` is the
    /// caller's, and serde accepts all three malformations: two `usize`s
    /// say nothing about their order, their magnitude, or where a UTF-8
    /// character starts (R0002-0010, R0002-0011, R0003-0060). In edit mode
    /// the map is built here, from the document just parsed and against the
    /// Markdown just regenerated, so only the intake guard can fire.
    #[error("render refused: {0}")]
    Render(#[from] transync_syntax::render::RenderError),
}

/// View-mode result — the two annotated pane fragments.
#[derive(Debug, Clone, Serialize)]
pub struct PairOutput {
    /// Annotated `<main>…</main>` fragment for the source pane.
    pub source_html: String,
    /// Annotated `<main>…</main>` fragment for the target pane.
    pub target_html: String,
}

/// Edit-mode result — the panes plus everything the caller needs to keep
/// editing: the regenerated Markdown and the map built against it.
#[derive(Debug, Clone, Serialize)]
pub struct RebuildOutput {
    /// Annotated `<main>…</main>` fragment for the source pane.
    pub source_html: String,
    /// Annotated `<main>…</main>` fragment for the target pane.
    pub target_html: String,
    /// The freshly built alignment map, serialized. Nested JSON: the JS
    /// side parses it a second time (or hands it straight back to
    /// [`render_pair_impl`]).
    pub alignment_json: String,
    /// The regenerated translated Markdown the map's `target_range`s index.
    pub translated_md: String,
    /// Set when the edited payloads changed the document's top-level block
    /// topology, `None` (JSON `null`) on the normal path. The rebuild
    /// succeeded either way — this announces a *degraded* render, it does
    /// not reject one; see this module's `check_top_level_structure`.
    pub structure_warning: Option<String>,
}

/// Re-render both panes from an already-translated triple.
///
/// `alignment_json` must be the map that was built against *this*
/// `translated_md`: the html and skipped render arms slice its
/// `target_range` bytes directly.
///
/// That precondition is now checked rather than assumed
/// ([`EngineError::Render`]). serde accepts every one of these — they are
/// shape-valid — and the renderer used to accept them too: a repeated
/// `source_block_id` meant "last row wins", a block with no row was dropped
/// from both panes (R0002-0010, R0002-0011), and a `target_range` that was
/// reversed, past the end of `translated_md`, or landing mid-character was
/// clamped and snapped until it sliced *something* (R0003-0060). The range
/// half covers the rows the browser demo's own gate cannot: it checks only
/// the rows it lets you edit, while html and skipped rows' ranges are read
/// by the renderer's bypass arms (R0003-0078).
pub fn render_pair_impl(
    source_md: &str,
    translated_md: &str,
    alignment_json: &str,
) -> Result<PairOutput, EngineError> {
    let doc = parse_with_ids(source_md)?;
    let map: AlignmentMap =
        serde_json::from_str(alignment_json).map_err(EngineError::AlignmentJson)?;

    Ok(PairOutput {
        source_html: render::render_source(&doc, &map)?,
        target_html: render::render_target(&doc, translated_md, &map)?,
    })
}

/// Run the full local loop: regen → alignment map → render both panes.
///
/// `payloads_json` is `{block_id: markdown_payload}` — an html block's
/// payload is its segment-array JSON, exactly as `regen::regenerate`
/// expects. `statuses_json` is `{block_id: fallback_status}` in the
/// snake_case wire form. A block absent from `payloads_json` falls back to
/// its own source bytes, which is regen's whole contract; a block absent
/// from `statuses_json` gets the same synthesized status the pipeline
/// would give it.
pub fn rebuild_impl(
    source_md: &str,
    payloads_json: &str,
    statuses_json: &str,
    source_lang: &str,
    target_lang: &str,
    detected: Option<String>,
) -> Result<RebuildOutput, EngineError> {
    let doc = parse_with_ids(source_md)?;

    let payloads: HashMap<BlockId, String> =
        serde_json::from_str(payloads_json).map_err(EngineError::PayloadsJson)?;
    let statuses: HashMap<BlockId, FallbackStatus> =
        serde_json::from_str(statuses_json).map_err(EngineError::StatusesJson)?;

    let known: HashSet<&str> = doc.blocks.iter().map(|b| b.block_id.0.as_str()).collect();
    reject_unknown_ids("payloads", payloads.keys(), &known)?;
    reject_unknown_ids("statuses", statuses.keys(), &known)?;

    let html_outcomes = outcome::html_outcomes(&doc);
    let (translated_md, offsets) = regen::regenerate(&doc, &payloads);
    let map = align::build_alignment_map(
        &doc,
        &statuses,
        &offsets,
        source_lang,
        target_lang,
        detected,
        &html_outcomes,
    );
    let alignment_json = serde_json::to_string(&map).map_err(EngineError::AlignmentSerialize)?;

    let structure_warning = check_top_level_structure(&doc, &translated_md);

    let source_html = render::render_source(&doc, &map)?;
    let target_html = render::render_target(&doc, &translated_md, &map)?;

    Ok(RebuildOutput {
        source_html,
        target_html,
        alignment_json,
        translated_md,
        structure_warning,
    })
}

/// Does the regenerated document still have the source document's top-level
/// shape? `Some(message)` when it does not.
///
/// `render::render_fragment` zips the source document's normalized
/// top-level entries against the *pane's* reparsed top-level nodes, and
/// advances its node cursor only on a label match (its Guard 2). A payload
/// that changes a block's topology — `# X\n\npara` typed over a paragraph —
/// makes the target pane's node sequence longer than the entry sequence, so
/// the entry at the seam pairs with the wrong label: it degrades to its own
/// byte range, escaped, and because the cursor stays put, later entries with
/// the same label can pair with the wrong node — shifted content under
/// correct anchors.
///
/// That degrade is the designed safe fallback (nothing panics, byte ranges
/// stay honest, re-editing the payload fixes it) and structure-changing
/// edits are not a supported feature. Announcing it is the point: this is
/// the signal that the render the caller is about to mount is the degraded
/// one.
///
/// Both sides go through [`parser::parse`] + [`walk::normalize_top_level`],
/// the same normalization the renderer's zip consumes — so a collapsed list
/// run counts as one entry on both sides, exactly as it pairs with one
/// Comrak `List` node. What is compared is [`top_level_shape`]'s whole
/// fingerprint, not just its length: R0002-0014 showed that a payload which
/// swaps one block for a different KIND of block keeps the count identical
/// and still mis-renders — `"EDITED"` typed over a blockquote leaves two
/// top-level entries either way, while the following paragraph's anchor
/// picks up the edited text and the paragraph's own content disappears.
fn check_top_level_structure(doc: &parser::Document, translated_md: &str) -> Option<String> {
    let expected = top_level_shape(doc);
    let reparsed = match parser::parse(translated_md) {
        Ok(reparsed) => reparsed,
        // Reachable: `parse` refuses a document past
        // `parser::MAX_BLOCK_NESTING_DEPTH`, and edited payloads can deepen
        // a regeneration past a source that was inside it. A silent `None`
        // here would claim a check that never ran.
        Err(err) => {
            return Some(format!(
                "the regenerated translation did not reparse ({err}), so its \
                 structure could not be checked against the source document"
            ));
        }
    };
    let found = top_level_shape(&reparsed);
    let divergence = describe_divergence(&expected, &found)?;
    Some(format!(
        "edited payloads changed the document structure: {divergence}. Both \
         panes still rendered, but the block that changed shape falls back \
         to its raw Markdown and blocks after it may show shifted content — \
         undo the structural change (one payload stays one block of the same \
         kind) to clear this."
    ))
}

/// One normalized top-level entry reduced to what the renderer's zip
/// actually pairs on: the label it matches a node by, and — for a collapsed
/// list run — how many `<li>` anchors that group pairs positionally against
/// the `List` node's direct items (`render_list_group`).
struct TopLevelShape {
    label: &'static str,
    items: usize,
}

/// The document's top-level fingerprint, source-IR side and reparsed side
/// alike.
///
/// [`walk::normalize_top_level`] is THE home of the normalization rule and
/// of the per-list item count (CLAUDE.md; `transync-core`'s
/// `validate::full_reparse` compares the same view) — this is a projection
/// of its entries, never a second derivation of the rule. A non-list entry
/// always has exactly one source, so `items` is only ever interesting for
/// the `"list"` label.
fn top_level_shape(doc: &parser::Document) -> Vec<TopLevelShape> {
    walk::normalize_top_level(doc)
        .into_iter()
        .map(|entry| TopLevelShape {
            label: entry.label,
            items: entry.sources.len(),
        })
        .collect()
}

/// Name the first way `found` departs from `expected`, or `None` when the
/// two fingerprints agree.
///
/// Three scans, coarsest first, mirroring the three `validate::full_reparse`
/// runs on the pipeline side: a count change is the most legible summary
/// when there is one, a kind substitution is the seam Guard 2 cannot pair,
/// and the per-list item count is meaningful only once the labels and the
/// count already line up (both sides are then indexed by the same `i`).
///
/// Block positions in the message are 1-based, because it is read by a
/// person editing the document rather than by the pipeline's log.
fn describe_divergence(expected: &[TopLevelShape], found: &[TopLevelShape]) -> Option<String> {
    if expected.len() != found.len() {
        return Some(format!(
            "the regenerated translation has {} top-level blocks where the \
             source document has {}",
            found.len(),
            expected.len()
        ));
    }
    for (i, (want, got)) in expected.iter().zip(found).enumerate() {
        if want.label != got.label {
            return Some(format!(
                "top-level block {} of the regenerated translation is a {} \
                 where the source document has a {}",
                i + 1,
                got.label,
                want.label
            ));
        }
    }
    for (i, (want, got)) in expected.iter().zip(found).enumerate() {
        if want.label == "list" && want.items != got.items {
            return Some(format!(
                "the list at top-level block {} of the regenerated \
                 translation has {} items where the source document's has {}",
                i + 1,
                got.items,
                want.items
            ));
        }
    }
    None
}

/// The alignment-map schema version this module speaks.
///
/// The demo JS asserts this against its own mirrored constant at init and
/// refuses to mount on mismatch, so a wasm blob can never silently disagree
/// with the crate it was built from.
pub fn schema_version_impl() -> &'static str {
    align::ALIGNMENT_SCHEMA_VERSION
}

/// Parse + re-key IDs, the two calls every mode starts with.
///
/// `assign_block_ids` is idempotent right after `parse` today, but the
/// pipeline always calls it and the IDs on the wire must be the pipeline's,
/// not the walker's provisional ones.
fn parse_with_ids(source_md: &str) -> Result<parser::Document, EngineError> {
    let mut doc = parser::parse(source_md)?;
    id::assign_block_ids(&mut doc);
    Ok(doc)
}

/// How many unknown ids the message names before it stops naming and starts
/// counting.
///
/// The rest of this module already answers "what is wrong with this input"
/// with a bounded diagnostic — `describe_divergence` names the *first*
/// divergence and stops — so listing every alien key was the odd one out
/// (R0009-0032): a rebuild request whose payload map is entirely foreign
/// built a string proportional to that map before returning the error that
/// rejects it, and the reader of that string could not use the part past the
/// first few entries anyway.
const UNKNOWN_ID_SAMPLE: usize = 8;

/// Reject ids that name no block in the document.
///
/// The sample is the first [`UNKNOWN_ID_SAMPLE`] in sort order, not the first
/// encountered, so the message is deterministic regardless of hash order —
/// and the total is counted separately, so the count is honest even though
/// only a sample is retained.
fn reject_unknown_ids<'a>(
    field: &'static str,
    ids: impl Iterator<Item = &'a BlockId>,
    known: &HashSet<&str>,
) -> Result<(), EngineError> {
    let mut sample: BTreeSet<&str> = BTreeSet::new();
    let mut total = 0_usize;
    for id in ids
        .map(|id| id.0.as_str())
        .filter(|id| !known.contains(*id))
    {
        total += 1;
        sample.insert(id);
        if sample.len() > UNKNOWN_ID_SAMPLE {
            // Drop the largest, so what survives is the sorted prefix.
            sample.pop_last();
        }
    }
    if total == 0 {
        return Ok(());
    }
    let shown = sample.len();
    let mut ids = sample.into_iter().collect::<Vec<_>>().join(", ");
    if total > shown {
        ids.push_str(&format!(" (and {} more)", total - shown));
    }
    Err(EngineError::UnknownBlockId { field, ids })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE_MD: &str = "# Title\n\nfirst para\n\n- a\n- b\n";

    /// The id of the first block of `wire_kind`, the way the demo learns
    /// it: off the parsed + id-assigned document, never guessed.
    fn first_block_id_of(md: &str, wire_kind: &str) -> String {
        let mut doc = parser::parse(md).expect("fixture parses");
        id::assign_block_ids(&mut doc);
        doc.blocks
            .iter()
            .find(|b| b.kind.wire_str() == wire_kind)
            .unwrap_or_else(|| panic!("fixture has a {wire_kind}"))
            .block_id
            .0
            .clone()
    }

    fn first_paragraph_id(md: &str) -> String {
        first_block_id_of(md, "paragraph")
    }

    #[test]
    fn render_pair_round_trips_a_translated_fixture() {
        // Ground truth the way the pipeline builds it: a rebuild-mode run
        // with no translations is a pure fallback pass, and it hands back a
        // real alignment map + translated md to feed the view-mode call.
        let payloads = serde_json::json!({});
        let statuses = serde_json::json!({});
        let built = rebuild_impl(
            FIXTURE_MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            None,
        )
        .expect("rebuild succeeds");
        assert!(
            built.source_html.starts_with("<main>"),
            "source pane is a <main> fragment"
        );
        assert!(
            built.target_html.starts_with("<main>"),
            "target pane is a <main> fragment"
        );
        assert_eq!(
            built.translated_md, FIXTURE_MD,
            "no translations => regen is byte-identical source"
        );
        let map: serde_json::Value =
            serde_json::from_str(&built.alignment_json).expect("map parses");
        assert_eq!(map["schema_version"], "1.2.0");

        let pair = render_pair_impl(FIXTURE_MD, &built.translated_md, &built.alignment_json)
            .expect("render_pair succeeds");
        assert_eq!(
            pair.source_html, built.source_html,
            "view mode reproduces rebuild's source pane"
        );
        assert_eq!(
            pair.target_html, built.target_html,
            "view mode reproduces rebuild's target pane"
        );
    }

    #[test]
    fn rebuild_applies_a_payload_and_view_mode_agrees() {
        // The edit loop proper: one paragraph payload replaced. Both modes
        // must still agree byte-for-byte on the same inputs.
        let para_id = first_paragraph_id(FIXTURE_MD);
        let payloads = serde_json::json!({ para_id.clone(): "첫 문단" });
        let statuses = serde_json::json!({ para_id: "translated" });
        let built = rebuild_impl(
            FIXTURE_MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            Some("en".to_string()),
        )
        .expect("rebuild succeeds");

        assert!(
            built.translated_md.contains("첫 문단"),
            "regen spliced the payload: {}",
            built.translated_md
        );
        assert!(
            !built.translated_md.contains("first para"),
            "the source paragraph is gone"
        );
        let map: AlignmentMap =
            serde_json::from_str(&built.alignment_json).expect("map deserializes");
        assert_eq!(map.detected_source_language.as_deref(), Some("en"));
        assert_eq!(map.validation_summary.translated, 1);

        let pair = render_pair_impl(FIXTURE_MD, &built.translated_md, &built.alignment_json)
            .expect("render_pair succeeds");
        assert_eq!(pair.source_html, built.source_html);
        assert_eq!(pair.target_html, built.target_html);
    }

    #[test]
    fn topology_changing_payload_warns_and_still_rebuilds() {
        // One paragraph payload split into a heading + a paragraph: the
        // regenerated document reparses to FOUR top-level entries
        // (heading, heading, paragraph, list) where the source document
        // normalizes to three, which is exactly the overrun
        // `render_fragment`'s zip cannot absorb.
        let para_id = first_paragraph_id(FIXTURE_MD);
        let payloads = serde_json::json!({ para_id.clone(): "# interloper\n\nfirst para" });
        let statuses = serde_json::json!({ para_id: "translated" });
        let built = rebuild_impl(
            FIXTURE_MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            None,
        )
        // A warning, not a rejection: the caller must stay editable so the
        // user can undo the change that caused it.
        .expect("rebuild still succeeds");

        let warning = built
            .structure_warning
            .as_deref()
            .expect("a topology-changing payload warns");
        assert!(
            warning.contains("has 4 top-level blocks"),
            "the warning names the regenerated count: {warning}"
        );
        assert!(
            warning.contains("source document has 3"),
            "the warning names the expected count: {warning}"
        );
        // The degraded render is still a render — both panes came back and
        // the edited text is in the document, so the demo has something to
        // mount and something to undo.
        assert!(built.source_html.starts_with("<main>"), "source pane");
        assert!(built.target_html.starts_with("<main>"), "target pane");
        assert!(
            built.translated_md.contains("# interloper"),
            "regen spliced the payload verbatim: {}",
            built.translated_md
        );
    }

    /// R0002-0014: a payload that swaps one block for a DIFFERENT KIND of
    /// block, one for one, leaves the top-level count untouched — so the
    /// count-only check this replaced stayed silent while the render
    /// degraded exactly the way `topology_changing_payload_warns_and_still_rebuilds`
    /// describes.
    ///
    /// The mis-render is asserted alongside the warning, because it is the
    /// warning's whole justification: `render_fragment`'s cursor does not
    /// advance past the paragraph node the edited blockquote produced, so
    /// the NEXT entry — a paragraph — pairs with that node and renders the
    /// edited text under `p-0002`'s anchor, while "second paragraph" is
    /// nowhere in the pane. If a future renderer change makes the pairing
    /// robust, this half is what tells the reader the warning's premise
    /// moved; the warning itself is still owed until then.
    #[test]
    fn a_same_count_kind_substitution_warns() {
        const MD: &str = "> quoted text\n\nsecond paragraph\n";
        let quote_id = first_block_id_of(MD, "blockquote");
        let payloads = serde_json::json!({ quote_id.clone(): "EDITED PLAIN TEXT" });
        let statuses = serde_json::json!({ quote_id: "translated" });
        let built = rebuild_impl(
            MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            None,
        )
        // Still a warning, not a rejection.
        .expect("rebuild still succeeds");

        let warning = built
            .structure_warning
            .as_deref()
            .expect("a same-count kind substitution warns");
        assert!(
            warning.contains("top-level block 1") && warning.contains("blockquote"),
            "the warning names the block that changed kind: {warning}"
        );

        assert_eq!(
            built.target_html.matches("EDITED PLAIN TEXT").count(),
            2,
            "the edited text renders under BOTH anchors — the degrade the \
             warning announces:\n{}",
            built.target_html
        );
        assert!(
            !built.target_html.contains("second paragraph"),
            "the following paragraph's own content is gone from the pane:\n{}",
            built.target_html
        );
    }

    /// The other same-count shape change: a list payload that splits into
    /// two items. Label sequence and top-level count both hold (one `list`
    /// entry either way), and `render_list_group` pairs rows against the
    /// `List` node's direct items positionally — so the extra item is
    /// dropped from the pane entirely. Only the per-list item count sees it.
    #[test]
    fn a_same_count_list_item_split_warns() {
        const MD: &str = "- alpha\n- beta\n";
        let item_id = first_block_id_of(MD, "list-item");
        let payloads = serde_json::json!({ item_id.clone(): "- alpha\n- inserted" });
        let statuses = serde_json::json!({ item_id: "translated" });
        let built = rebuild_impl(
            MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            None,
        )
        .expect("rebuild still succeeds");

        let warning = built
            .structure_warning
            .as_deref()
            .expect("a list that gained an item warns");
        assert!(
            warning.contains("has 3 items") && warning.contains("has 2"),
            "the warning names both item counts: {warning}"
        );
        assert!(
            built.translated_md.contains("- inserted"),
            "regen spliced the payload verbatim: {}",
            built.translated_md
        );
    }

    #[test]
    fn same_topology_payload_does_not_warn() {
        // The normal path must stay silent — a false positive here would
        // put a scary strip in front of every ordinary demo edit.
        let para_id = first_paragraph_id(FIXTURE_MD);
        let payloads = serde_json::json!({ para_id.clone(): "still exactly one paragraph" });
        let statuses = serde_json::json!({ para_id: "translated" });
        let built = rebuild_impl(
            FIXTURE_MD,
            &payloads.to_string(),
            &statuses.to_string(),
            "en",
            "ko",
            None,
        )
        .expect("rebuild succeeds");
        assert_eq!(
            built.structure_warning, None,
            "a same-topology edit is not a structure change"
        );

        // Neither does the no-edit pass, whose regen is byte-identical
        // source — the floor case for "no false positives".
        let untouched = rebuild_impl(FIXTURE_MD, "{}", "{}", "en", "ko", None).expect("rebuild");
        assert_eq!(untouched.structure_warning, None);
    }

    /// The cost of comparing more than a count is more room for a FALSE
    /// positive, and a false positive here puts a scary strip in front of an
    /// ordinary edit. So sweep the shapes whose source-IR label or item
    /// count could disagree with a reparse of their own regenerated bytes:
    /// a no-edit pass is byte-identical Markdown, hence the floor case for
    /// every one of them.
    #[test]
    fn the_shape_comparison_has_no_false_positives() {
        for (name, md) in [
            (
                "html block",
                "para\n\n<div align=\"center\">\n<b>Hi</b>\n</div>\n",
            ),
            ("nested list", "- a\n  - a1\n  - a2\n- b\n"),
            ("marker change", "- a\n- b\n\n* c\n\n1. one\n2. two\n"),
            ("task list", "- [ ] a\n- [x] b\n"),
            ("table", "| h |\n| - |\n| c |\n"),
            ("code fence", "```rust\nlet x = 1;\n```\n"),
            ("blockquote", "> quoted\n>\n> still quoted\n"),
            ("thematic break", "para\n\n---\n\npara\n"),
            ("footnote def", "text[^1]\n\n[^1]: the note\n"),
            ("link refdef", "[a](b)\n\n[ref]: https://example.com\n"),
        ] {
            let built = rebuild_impl(md, "{}", "{}", "en", "ko", None)
                .unwrap_or_else(|err| panic!("{name} rebuilds: {err}"));
            assert_eq!(
                built.structure_warning, None,
                "{name}: an unedited document is not a structure change"
            );
        }
    }

    /// The demo's html carve-out rests on this synthesis, so pin it here
    /// rather than in a comment: a unit-backed html block whose id is absent
    /// from BOTH maps regenerates from its own source bytes and — because
    /// `align::build_alignment_map` treats a missing status for a
    /// translatable block as an internal drop — is labelled
    /// `fallback_source`, which the renderer presents as the escaped
    /// `data-skipped="html-block"` placeholder in both panes.
    ///
    /// That is what makes omitting html rows from `web/js/wasm-demo.js`'s
    /// edit model *honest* rather than merely harmless: the row cannot keep
    /// claiming `translated` over content that reverted to source.
    #[test]
    fn omitted_unit_backed_html_row_is_fallback_source_not_translated() {
        const HTML_MD: &str = "para\n\n<div align=\"center\">\n<b>Hero</b>\n</div>\n";
        let built = rebuild_impl(HTML_MD, "{}", "{}", "en", "ko", None).expect("rebuild succeeds");
        let map: AlignmentMap =
            serde_json::from_str(&built.alignment_json).expect("map deserializes");
        let html_row = map
            .blocks
            .iter()
            .find(|b| b.block_kind == "html")
            .expect("fixture has an html row");
        assert_eq!(
            html_row.fallback_status,
            FallbackStatus::FallbackSource,
            "an omitted unit-backed html row must not claim translated"
        );
        for (pane, html) in [
            ("source", &built.source_html),
            ("target", &built.target_html),
        ] {
            assert!(
                html.contains("data-skipped=\"html-block\""),
                "{pane} pane presents the escaped placeholder:\n{html}"
            );
            assert!(
                html.contains("data-fallback=\"fallback_source\""),
                "{pane} pane carries the honest tint attribute:\n{html}"
            );
        }
    }

    #[test]
    fn schema_version_is_the_wire_constant() {
        assert_eq!(
            schema_version_impl(),
            transync_syntax::align::ALIGNMENT_SCHEMA_VERSION
        );
    }

    #[test]
    fn bad_alignment_json_is_an_error_not_a_panic() {
        let err = render_pair_impl(FIXTURE_MD, FIXTURE_MD, "{not json").unwrap_err();
        assert!(
            err.to_string().contains("alignment"),
            "error names the failing input: {err}"
        );
    }

    #[test]
    fn bad_payloads_and_statuses_json_are_errors_not_panics() {
        let payload_err =
            rebuild_impl(FIXTURE_MD, "{not json", "{}", "en", "ko", None).unwrap_err();
        assert!(
            payload_err.to_string().contains("payloads"),
            "error names the failing input: {payload_err}"
        );
        // A status value outside the snake_case wire enum is a deserialize
        // failure, not a silent default.
        let status_err = rebuild_impl(
            FIXTURE_MD,
            "{}",
            &serde_json::json!({ "p-0002": "Translated" }).to_string(),
            "en",
            "ko",
            None,
        )
        .unwrap_err();
        assert!(
            status_err.to_string().contains("statuses"),
            "error names the failing input: {status_err}"
        );
    }

    #[test]
    fn unknown_block_id_in_payloads_is_an_error() {
        let payloads = serde_json::json!({"zz-9999": "ghost"});
        let err =
            rebuild_impl(FIXTURE_MD, &payloads.to_string(), "{}", "en", "ko", None).unwrap_err();
        assert!(err.to_string().contains("zz-9999"), "got: {err}");
        assert!(err.to_string().contains("payloads"), "got: {err}");
    }

    #[test]
    fn unknown_block_id_in_statuses_is_an_error() {
        let statuses = serde_json::json!({"zz-9999": "translated"});
        let err =
            rebuild_impl(FIXTURE_MD, "{}", &statuses.to_string(), "en", "ko", None).unwrap_err();
        assert!(err.to_string().contains("zz-9999"), "got: {err}");
        assert!(err.to_string().contains("statuses"), "got: {err}");
    }

    /// R0009-0032: the message names a bounded, sorted sample and counts the
    /// rest. What it must NOT do is grow with the malformed request — the
    /// diagnostic for "these keys are all wrong" cannot itself be a copy of
    /// every wrong key.
    #[test]
    fn many_unknown_block_ids_are_sampled_rather_than_listed() {
        let mut payloads = serde_json::Map::new();
        for i in 0..(UNKNOWN_ID_SAMPLE * 5) {
            payloads.insert(format!("zz-{i:04}"), serde_json::json!("ghost"));
        }
        let total = payloads.len();
        let json = serde_json::Value::Object(payloads).to_string();
        let message = rebuild_impl(FIXTURE_MD, &json, "{}", "en", "ko", None)
            .unwrap_err()
            .to_string();

        // The sorted prefix, and nothing after it — the sample is the first
        // N in sort order, so it is the same N whatever order the map hashed.
        for i in 0..UNKNOWN_ID_SAMPLE {
            assert!(
                message.contains(&format!("zz-{i:04}")),
                "the sorted prefix is named: {message}"
            );
        }
        assert!(
            !message.contains(&format!("zz-{:04}", UNKNOWN_ID_SAMPLE)),
            "nothing past the sample is named: {message}"
        );
        assert!(
            message.contains(&format!("(and {} more", total - UNKNOWN_ID_SAMPLE)),
            "the total is still honest: {message}"
        );
    }

    /// A shape-valid map that does not describe `source_md`. serde accepts
    /// both malformations — they are structurally perfect JSON — so view
    /// mode is where the renderer's own checks (R0002-0010, R0002-0011) are
    /// the only thing standing between a foreign map and a pane that reads
    /// as a clean render.
    fn map_of(source_md: &str) -> serde_json::Value {
        let built =
            rebuild_impl(source_md, "{}", "{}", "en", "ko", None).expect("rebuild succeeds");
        serde_json::from_str(&built.alignment_json).expect("map parses")
    }

    #[test]
    fn a_map_repeating_a_row_id_is_refused_by_view_mode() {
        let mut map = map_of(FIXTURE_MD);
        let dup = map["blocks"][0].clone();
        map["blocks"].as_array_mut().expect("blocks").push(dup);

        let err = render_pair_impl(FIXTURE_MD, FIXTURE_MD, &map.to_string()).unwrap_err();
        assert!(matches!(err, EngineError::Render(_)), "got: {err}");
        assert!(
            err.to_string().contains("repeats source_block_id"),
            "error says what is wrong with the map: {err}"
        );
    }

    #[test]
    fn a_map_missing_a_row_is_refused_by_view_mode() {
        let mut map = map_of(FIXTURE_MD);
        let dropped = map["blocks"]
            .as_array_mut()
            .expect("blocks")
            .remove(1)
            .get("source_block_id")
            .expect("row has an id")
            .as_str()
            .expect("id is a string")
            .to_string();

        let err = render_pair_impl(FIXTURE_MD, FIXTURE_MD, &map.to_string()).unwrap_err();
        assert!(matches!(err, EngineError::Render(_)), "got: {err}");
        assert!(
            err.to_string().contains(&dropped),
            "error names the uncovered block: {err}"
        );
    }

    /// R0003-0078, closing through R0003-0060. The browser demo checks
    /// `target_range` *values* only for the rows it lets you edit, and html
    /// rows are not editable (`isEditableRow` in `web/js/wasm-demo.js`) —
    /// but the renderer's html bypass arm slices their range anyway. Before
    /// the renderer policed ranges, a corrupt one on such a row was clamped
    /// into truncated, empty or unrelated bytes and mounted as content, with
    /// no refusal anywhere in the stack. The demo needs no second gate: this
    /// one covers every row the render path reads.
    #[test]
    fn a_corrupt_range_on_a_row_the_demo_does_not_gate_is_refused_by_view_mode() {
        const HTML_FIXTURE_MD: &str = "<div>keep me</div>\n\npara\n";
        let built =
            rebuild_impl(HTML_FIXTURE_MD, "{}", "{}", "en", "ko", None).expect("rebuild succeeds");

        // Control: the honest triple presents the html block's own bytes.
        let ok = render_pair_impl(HTML_FIXTURE_MD, &built.translated_md, &built.alignment_json)
            .expect("the honest triple renders");
        assert!(ok.target_html.contains("keep me"), "{}", ok.target_html);

        let mut map: serde_json::Value =
            serde_json::from_str(&built.alignment_json).expect("map parses");
        {
            let row = map["blocks"]
                .as_array_mut()
                .expect("blocks")
                .iter_mut()
                .find(|b| b["block_kind"] == "html")
                .expect("fixture has an html row");
            let start = row["target_range"]["start"].as_u64().expect("start");
            let end = row["target_range"]["end"].as_u64().expect("end");
            assert!(end > start, "the html row covers real bytes");
            row["target_range"] = serde_json::json!({ "start": end, "end": start });
        }

        let err = render_pair_impl(HTML_FIXTURE_MD, &built.translated_md, &map.to_string())
            .expect_err("a reversed range on an html row is refused");
        assert!(matches!(err, EngineError::Render(_)), "got: {err}");
        assert!(
            err.to_string().contains("unusable target byte range"),
            "error says which pane and what is wrong: {err}"
        );
    }

    #[test]
    fn empty_source_is_an_empty_pair_not_a_panic() {
        let built = rebuild_impl("", "{}", "{}", "en", "ko", None).expect("rebuild succeeds");
        assert_eq!(built.translated_md, "");
        let pair = render_pair_impl("", &built.translated_md, &built.alignment_json)
            .expect("render_pair succeeds");
        assert_eq!(pair.source_html, built.source_html);
        assert_eq!(pair.target_html, built.target_html);
    }
}
