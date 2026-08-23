//! Per-block payload strategy: what a translatable block sends to the model.
//!
//! One block yields one `(source_payload, InputMode, BlockConstraints)`
//! triple, and the recipe is per block kind — html blocks send a JSON array
//! of extracted text segments, an indented code block sends the fenced
//! re-spelling of its source slice, and every other kind sends that slice
//! raw. [`assemble`] is the single entry point; `unit::build_batches` calls
//! it once per unit and never branches on kind itself.
//!
//! TRACE: SCN-01..SCN-06

use crate::id::{BlockKind, Spelling};
use crate::llm::{BlockConstraints, InputMode};
use crate::parser::{Block, Document};
use crate::structure::{inspect_blockquote_children, inspect_list_topology, inspect_table};
use transync_syntax::outcome::block_payload;

/// Build the payload triple for one translatable `block` of `doc`.
///
/// # Panics
///
/// Panics on an html block whose segment extraction fails. Reaching this
/// function at all means the caller's `HtmlOutcome` map said
/// [`HtmlOutcome::Unit`](transync_syntax::outcome::HtmlOutcome::Unit) for
/// this block, and that verdict came from the same extraction routine over
/// the same bytes — so a failure here means the map was built from a
/// *different* document, which is the same-document contract violation
/// `build_batches` documents.
pub(crate) fn assemble(doc: &Document, block: &Block) -> (String, InputMode, BlockConstraints) {
    // Spec §6: the branch is on SPELLING, and it is first. Html units carry a
    // JSON segment array, not the block's raw bytes — the markup never reaches
    // the model, and the structural facts the validator needs ride in
    // `constraints.html`. Branching here rather than on the kind is what
    // structurally prevents `(CodeBlock × Html)` from reaching `code_payload`'s
    // indented-block re-fencing: DCR-0031's fence synthesizer is Markdown-only,
    // and an HTML document's `<pre>` is a `BlockKind::CodeBlock`.
    match block.spelling {
        Spelling::Html { block_type } => {
            let raw = block_payload(doc, block);
            let segs = transync_html::extract(&raw)
                .expect("outcome said Unit — extract cannot fail here (same input, same routine)");
            let payload = serde_json::to_string(&segs.texts)
                .expect("Vec<String> JSON serialization is infallible");
            let constraints = BlockConstraints {
                html: Some(crate::llm::HtmlSegmentConstraints {
                    segment_count: segs.texts.len() as u32,
                    segment_labels: segs.labels,
                    source_bytes: raw,
                    // Spec §6: `0` is the sentinel for "a block of an HTML
                    // document, where no CommonMark type applies". The field's
                    // type is frozen (§0 tier (a), no `#[non_exhaustive]`, and
                    // the v0.4.0 window is closed), so `Option<u8>` was never
                    // available; `0` and `1` behave identically under the
                    // `matches!(t, 6 | 7)` rule, and `0` is the one that does
                    // not claim to be a CommonMark type. The splice policy is
                    // still derived from the SPELLING — this field is a
                    // record, not a switch.
                    block_type: block_type.unwrap_or(0),
                }),
                ..BlockConstraints::default()
            };
            (payload, InputMode::HtmlSegments, constraints)
        }
        Spelling::Markdown => {
            let payload = code_payload(doc, block);
            let constraints = constraints_for(&block.kind, &payload);
            (payload, input_mode_for(&block.kind), constraints)
        }
    }
}

/// The block's raw source slice — except for an INDENTED code block, which is
/// re-spelled as a fenced block before it goes on the wire.
///
/// [`InputMode::FullCodeBlock`] promises "the entire ` ```lang … ``` `
/// Markdown including the open/close fences", and an indented block's raw
/// slice is not that: it reparses as a paragraph (or, when the source indent
/// is deeper than four columns, as a *nested* indented block), so
/// `validate::fragment_reparse` rejected it on every attempt. Re-fencing here
/// makes the indented spelling conform to the axis rather than redefining the
/// axis, which is why `VALIDATION_SCHEMA_VERSION` does not move.
///
/// The synthesizer is [`transync_syntax::regen::regenerate_code_block`] — THE
/// one this engine already uses to re-emit an accepted code block — so fence
/// length safety against a backtick run in the body is inherited rather than
/// reimplemented, and the dedent is comrak's CommonMark dedent (tabs, partial
/// tabs, and indentation past four columns all handled as content). Its
/// trailing newline is trimmed so the payload has the same envelope as a
/// fenced unit's raw slice, which never carries one.
///
/// TRACE: SCN-04
fn code_payload(doc: &Document, block: &Block) -> String {
    let raw = block_payload(doc, block);
    if !matches!(block.kind, BlockKind::CodeBlock { fenced: false, .. }) {
        return raw;
    }
    let mut fenced = transync_syntax::regen::regenerate_code_block(&raw, None);
    if fenced.ends_with('\n') {
        fenced.pop();
    }
    fenced
}

fn input_mode_for(kind: &BlockKind) -> InputMode {
    match kind {
        BlockKind::Table => InputMode::FullTableMarkdown,
        BlockKind::CodeBlock { info, .. } => InputMode::FullCodeBlock {
            language_info: info.clone(),
        },
        BlockKind::ListItem { .. } => InputMode::ListItemContent,
        BlockKind::Blockquote => InputMode::BlockquoteContent,
        _ => InputMode::TextFragment,
    }
}

pub(crate) fn constraints_for(kind: &BlockKind, source_payload: &str) -> BlockConstraints {
    // Heading depth is read from THE kind→level mapping,
    // [`BlockKind::heading_level`], rather than re-listing the six heading
    // variants here — one home for the fact (spec §7). It yields `None` for
    // every non-heading kind, which is exactly the default this field had
    // before, so the `match` below need not mention headings at all.
    let mut c = BlockConstraints {
        must_preserve_heading_level: kind.heading_level(),
        ..BlockConstraints::default()
    };
    match kind {
        BlockKind::Table => {
            if let Some((cols, alignments, rows)) = inspect_table(source_payload) {
                c.must_preserve_table_columns = Some(cols);
                c.must_preserve_table_alignment = Some(alignments);
                c.must_preserve_table_row_count = Some(rows);
                c.forbid_block_breaks_in_inline = true;
            }
        }
        BlockKind::CodeBlock { info, .. } => c.must_preserve_code_fence_info = info.clone(),
        BlockKind::ListItem { .. } => {
            c.must_preserve_list_topology = true;
            c.expected_list_topology = inspect_list_topology(source_payload);
        }
        BlockKind::Blockquote => {
            c.expected_blockquote_children = inspect_blockquote_children(source_payload);
        }
        _ => {}
    }
    c
}

// Spec §7 ("ONE `heading_level` mapping"): `constraints_for` derives the
// heading depth from [`BlockKind::heading_level`] instead of carrying its own
// kind→level ladder. Nothing else asserted the projection it must produce, so
// this pins it for every kind — the six headings get `Some(1..=6)`, every
// other kind gets `None`.
#[cfg(test)]
mod heading_level_constraint_tests {
    use super::constraints_for;
    use crate::id::BlockKind;

    #[test]
    fn only_heading_kinds_carry_a_heading_level_and_it_is_their_depth() {
        let expected = [
            (BlockKind::Heading1, Some(1)),
            (BlockKind::Heading2, Some(2)),
            (BlockKind::Heading3, Some(3)),
            (BlockKind::Heading4, Some(4)),
            (BlockKind::Heading5, Some(5)),
            (BlockKind::Heading6, Some(6)),
            (BlockKind::Paragraph, None),
            (BlockKind::Table, None),
            (
                BlockKind::CodeBlock {
                    info: None,
                    fenced: true,
                },
                None,
            ),
            (
                BlockKind::CodeBlock {
                    info: Some("rust".to_string()),
                    fenced: true,
                },
                None,
            ),
            (
                BlockKind::CodeBlock {
                    info: None,
                    fenced: false,
                },
                None,
            ),
            (
                BlockKind::ListItem {
                    ordered: false,
                    task: None,
                },
                None,
            ),
            (BlockKind::Blockquote, None),
            (BlockKind::ThematicBreak, None),
            (BlockKind::Image, None),
            (BlockKind::Html, None),
            (
                BlockKind::Skipped {
                    label: "front-matter".to_string(),
                },
                None,
            ),
        ];

        for (kind, level) in &expected {
            assert_eq!(
                constraints_for(kind, "").must_preserve_heading_level,
                *level,
                "{kind:?} must project to {level:?}",
            );
        }
    }
}

// Spec §3.2–§3.3: the html arm's JSON segment payload and the constraints
// it packages for the validator.
#[cfg(test)]
mod html_payload_tests {
    use crate::TranslateOptions;
    use crate::id::assign_block_ids;
    use crate::llm::InputMode;
    use crate::parser::parse;
    use crate::unit::{HtmlOutcome, build_batches, html_outcomes};

    #[test]
    fn html_block_with_text_yields_a_json_segment_unit() {
        let mut doc =
            parse("<details><summary>Click &amp; go</summary></details>\n").expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(
            outcomes.get(&doc.blocks[0].block_id),
            Some(&HtmlOutcome::Unit)
        );

        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &outcomes);
        let unit = &batches[0].units[0];
        assert_eq!(unit.unit_id.0, "html-0001");
        assert!(matches!(unit.input_mode, InputMode::HtmlSegments));
        assert_eq!(
            unit.source_payload, "[\"Click & go\"]",
            "decoded, compact JSON"
        );
        let h = unit.constraints.html.as_ref().expect("html constraints");
        assert_eq!(h.segment_count, 1);
        assert_eq!(h.segment_labels, vec!["summary".to_string()]);
        assert_eq!(h.block_type, 6);
        assert!(h.source_bytes.contains("&amp;"), "raw bytes, not decoded");
    }
}

// OI-0033 / DCR-0017 Guard-1 residual (a). The ONLY known route to
// `expected_list_topology == None` was a mis-sliced (empty) payload from the
// lone-CR sourcepos desync: `inspect_list_topology("")` walks to zero item
// entries and returns `None`, which makes `validate::per_kind::check_list`
// return `Ok(())` unconditionally. With CR-aware line offsets the route must
// be closed — this asserts the residual predicate directly rather than
// inferring it. It travels with `constraints_for`, which is its subject.
#[cfg(test)]
mod lone_cr_topology_tests {
    use crate::TranslateOptions;
    use crate::id::BlockKind;
    use crate::llm::TranslationUnit;
    use crate::unit::{build_batches, html_outcomes};

    #[test]
    fn lone_cr_list_items_carry_list_topology_constraints() {
        let src = "intro para\r\r- alpha\r- bravo\r- charlie\r\rtail para\r";
        let mut doc = crate::parser::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &outcomes);

        let li_units: Vec<&TranslationUnit> = batches
            .iter()
            .flat_map(|b| &b.units)
            .filter(|u| matches!(u.block_kind, BlockKind::ListItem { .. }))
            .collect();
        assert_eq!(
            li_units.len(),
            3,
            "fixture must yield three list-item units, got {:?}",
            li_units.iter().map(|u| &u.unit_id).collect::<Vec<_>>(),
        );

        for u in &li_units {
            assert!(
                !u.source_payload.is_empty(),
                "{} payload must be non-empty",
                u.unit_id,
            );
            assert!(
                u.constraints.expected_list_topology.is_some(),
                "{} must carry Some(list topology) — residual (a) is void",
                u.unit_id,
            );
            assert!(
                u.constraints.must_preserve_list_topology,
                "{} must still demand topology preservation",
                u.unit_id,
            );
        }
    }
}

// ti `457e51`: the wire payload for an INDENTED code block.
//
// `outcome::block_payload` hands back the block's source slice, which for an
// indented block is its indented spelling — a shape the model has never been
// asked to handle, and one NO per-unit layer would catch: the indented
// spelling reparses as a code block, so `fragment_reparse` matches the kind
// and `per_kind::check_code` sees `None`/`None` and passes. An un-fenced wire
// payload would therefore violate `InputMode::FullCodeBlock`'s contract
// SILENTLY, which is exactly how the pre-fix eight-space class reached
// regeneration and corrupted the output. The unit must instead carry the
// block re-fenced by the engine's OWN fence
// synthesizer (`regen::regenerate_code_block`), so fence-length safety
// against a backtick run in the body is inherited rather than reimplemented
// and the CommonMark dedent is comrak's, not ours.
//
// The wire contract does not move: the label is still `full_code_block`, the
// payload is still a whole fenced block, and the info string is still the
// source's (here: absent).
#[cfg(test)]
mod indented_code_payload_tests {
    use crate::TranslateOptions;
    use crate::id::BlockKind;
    use crate::llm::{InputMode, TranslationUnit};
    use crate::parser::parse;
    use crate::unit::{build_batches, html_outcomes};

    /// Intro / four-space block whose body holds a ``` run / separator /
    /// eight-space block / outro. Same shape as
    /// `crates/transync/tests/fixtures/scn-04-indented-code.md`.
    const SRC: &str = "Intro.\n\n    let s = \"```\";\n    println!(\"{}\", s);\n\nSeparator.\n\n        indented code line\n\nOutro.\n";

    fn code_units(src: &str) -> Vec<TranslationUnit> {
        let mut doc = parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        build_batches(&doc, &opts, None, &outcomes)
            .iter()
            .flat_map(|b| b.units.clone())
            .filter(|u| matches!(u.block_kind, BlockKind::CodeBlock { .. }))
            .collect()
    }

    /// Every top-level code block in `payload`, as `(fenced, info, literal)`.
    fn code_blocks(payload: &str) -> Vec<(bool, String, String)> {
        use comrak::nodes::NodeValue;
        let arena = comrak::Arena::new();
        let opts = crate::parser::comrak_options();
        let root = comrak::parse_document(&arena, payload, &opts);
        root.children()
            .filter_map(|c| match &c.data.borrow().value {
                NodeValue::CodeBlock(cb) => Some((cb.fenced, cb.info.clone(), cb.literal.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn an_indented_block_is_sent_as_a_synthesized_fenced_block() {
        let units = code_units(SRC);
        assert_eq!(
            units.len(),
            2,
            "the fixture has two code blocks: {:?}",
            units
                .iter()
                .map(|u| u.unit_id.0.as_str())
                .collect::<Vec<_>>(),
        );

        for u in &units {
            assert!(
                matches!(
                    u.input_mode,
                    InputMode::FullCodeBlock {
                        language_info: None
                    }
                ),
                "{} must ride the code-block axis with no info string, got {:?}",
                u.unit_id,
                u.input_mode,
            );
            assert_eq!(
                u.constraints.must_preserve_code_fence_info, None,
                "{} has no source info string to preserve",
                u.unit_id,
            );

            let blocks = code_blocks(&u.source_payload);
            assert_eq!(
                blocks.len(),
                1,
                "{} payload must parse as exactly one code block, got {:?} for {:?}",
                u.unit_id,
                blocks,
                u.source_payload,
            );
            assert!(
                blocks[0].0,
                "{} payload must be FENCED, not indented: {:?}",
                u.unit_id, u.source_payload,
            );
            assert_eq!(blocks[0].1, "", "{} carries no info string", u.unit_id);
        }

        // Body 1: the CommonMark dedent, with the ``` run intact — which is
        // what forces the synthesized fence to four backticks.
        assert_eq!(
            code_blocks(&units[0].source_payload)[0].2,
            "let s = \"```\";\nprintln!(\"{}\", s);\n",
        );
        assert!(
            units[0].source_payload.starts_with("````"),
            "fence-length safety is inherited from the engine's synthesizer: {:?}",
            units[0].source_payload,
        );

        // Body 2: eight spaces = four of block structure + four of CONTENT.
        assert_eq!(
            code_blocks(&units[1].source_payload)[0].2,
            "    indented code line\n",
        );
    }

    /// Blast-radius pin (the half that must NOT move): widening an indented
    /// block's source range must leave its NEIGHBORS' `CacheKey.context_hash`
    /// alone. The mechanism is `unit::context`'s `.trim()` on the neighbor
    /// summary plus `context_hash`'s use of `BlockKind::wire_str()` rather
    /// than the serde form — so the summary of an indented block is its
    /// indent-free content either way, and the kind label is `"code-block"`
    /// either way. Green before the fix and after; it guards the swap.
    #[test]
    fn a_neighbors_view_of_an_indented_block_is_indent_free() {
        use crate::llm::TranslationUnit;

        let mut doc = parse(SRC).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let units: Vec<TranslationUnit> = build_batches(&doc, &opts, None, &outcomes)
            .iter()
            .flat_map(|b| b.units.clone())
            .collect();

        let separator = units
            .iter()
            .find(|u| u.unit_id.0 == "p-0003")
            .expect("the separator paragraph is a unit");
        let preceding = separator
            .context
            .preceding_block
            .as_ref()
            .expect("it has a preceding block");
        assert_eq!(preceding.kind.wire_str(), "code-block");
        assert_eq!(
            preceding.summary, "let s = \"```\";\n    println!(\"{}\", s);",
            "the summary is trimmed, so the block-structure indent never \
             reached it and widening the range cannot move it",
        );

        let following = separator
            .context
            .following_block
            .as_ref()
            .expect("it has a following block");
        assert_eq!(following.kind.wire_str(), "code-block");
        assert_eq!(following.summary, "indented code line");
    }

    /// The payload has the shape a fenced unit's does — no trailing newline
    /// past the closing fence — so nothing downstream can tell the two
    /// spellings apart by their envelope.
    #[test]
    fn the_synthesized_payload_has_the_same_envelope_as_a_fenced_units() {
        let indented = code_units(SRC);
        let fenced = code_units("Intro.\n\n```\nlet x = 1;\n```\n\nOutro.\n");
        assert_eq!(fenced.len(), 1);
        assert_eq!(
            fenced[0].source_payload, "```\nlet x = 1;\n```",
            "a fenced block still sends its own source bytes, unchanged",
        );
        for u in &indented {
            assert!(
                !u.source_payload.ends_with('\n'),
                "{} payload must not carry a trailing newline: {:?}",
                u.unit_id,
                u.source_payload,
            );
        }
    }
}
