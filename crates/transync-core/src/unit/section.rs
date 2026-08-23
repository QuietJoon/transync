//! The section partition: P1 of section-coherent packing (DCR-0027).
//!
//! `unit::build_batches` no longer hands the whole unit list to one
//! [`crate::batch::group_by_token_budget`] call. It partitions first and packs
//! second, so a batch's units all come from one section of the document and no
//! batch straddles a `##` / `###` boundary — which is what gives "which glossary
//! entries apply to this batch's prompt" a single well-defined answer (SL-108).
//!
//! The partition is a pure function of the unit list, it runs exactly once per
//! run (after the DCR-0026 row-window split, before any packing), and nothing
//! downstream re-partitions: retry rounds re-pack *within* a batch, and a batch
//! is single-section by construction.
//!
//! TRACE: DCR-0027
//! TRACE: SCN-10

use crate::llm::TranslationUnit;
use crate::unit::context::ContextIndex;

/// One section of the document: the units that pack together, and the heading
/// stack that identifies the section to everything downstream.
///
/// The **heading stack** is the plain text of every heading enclosing the
/// section's body blocks, outermost first, *including the heading that opens
/// it* — which is why it cannot simply be read off a unit's
/// `context.section_path`, since a heading's own path excludes itself. It is
/// what glossary selectors are matched against (DCR-0027 G3), and the preamble
/// before a document's first heading has an empty one.
pub(crate) struct Section {
    pub(crate) heading_stack: Vec<String>,
    pub(crate) units: Vec<TranslationUnit>,
}

/// Split `units` — in document order, never reordered — into the sections the
/// packer then packs one at a time.
///
/// A new section starts at unit index 0 and at every heading unit, of any level
/// 1–6. **A heading unit belongs to the section it opens**, which is the rule
/// worth stating at the unit level: the parser stamps a heading's own
/// `section_path` *excluding* itself (`sections::close_through` is captured
/// before `open_scope`), so grouping by raw path equality would dangle every
/// heading off the section it closes — the `## B` line would be packed with
/// `## A`'s prose and could be steered by `A`'s glossary.
///
/// Level is deliberately not consulted beyond "is this a heading". A deeper
/// heading opens a subsection, and a subsection is a section: `### Windows`
/// under `## Installation` packs on its own. That is the strict reading of the
/// commissioned acceptance (DCR-0027 P2/OQ-A) — nothing here ever merges two
/// sections into one batch, and the only thing that ever separates one
/// section's units is the packer running out of budget inside it (P4).
///
/// The units before the document's first heading are their own section (the
/// preamble); a document with no heading at all is one section, which is why a
/// glossary-free flat document packs exactly as it did before DCR-0027.
///
/// Row windows need no special case: DCR-0026 gives a window the parent
/// table's `context` unchanged, and the split has already replaced the parent
/// by the time this runs, so every window lands in the parent's section by
/// construction.
///
/// `index` is the run's [`ContextIndex`], read only for the opening heading's
/// own plain text — the last element of the section's heading stack.
///
/// TRACE: DCR-0027
pub(crate) fn partition_by_section(
    units: Vec<TranslationUnit>,
    index: &ContextIndex<'_>,
) -> Vec<Section> {
    let mut sections: Vec<Section> = Vec::new();
    for unit in units {
        let opens = unit.block_kind.heading_level().is_some();
        if sections.is_empty() || opens {
            let heading_stack = if opens {
                heading_stack_of(&unit, index)
            } else {
                // The preamble: no heading encloses it, so no section-scoped
                // entry can select it (DCR-0027 G3).
                Vec::new()
            };
            sections.push(Section {
                heading_stack,
                units: Vec::new(),
            });
        }
        // Non-empty by the push above on the first iteration.
        sections
            .last_mut()
            .expect("a section is open once the first unit has been seen")
            .units
            .push(unit);
    }
    sections
}

/// The heading stack of the section `heading` opens: its own ancestors, then
/// itself.
///
/// The ancestors come from the unit's `context.section_path` — the very
/// snippets that cross the wire — and the opener's own text from the index, so
/// both halves of the stack are spelled exactly as the model and the alignment
/// map see them. A heading the index does not know (a synthetic unit, or one
/// whose id was rewritten) contributes no name of its own; its ancestors still
/// do, so a term scoped to an enclosing section keeps applying.
fn heading_stack_of(heading: &TranslationUnit, index: &ContextIndex<'_>) -> Vec<String> {
    let mut stack: Vec<String> = heading
        .context
        .section_path
        .iter()
        .map(|h| h.text.clone())
        .collect();
    if let Some(own) = index.heading_text(&heading.unit_id) {
        stack.push(own.to_string());
    }
    stack
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode};

    fn unit(id: &str, kind: BlockKind) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: kind,
            input_mode: InputMode::TextFragment,
            source_payload: format!("payload for {id}"),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(0),
            retry: None,
        }
    }

    fn ids(sections: &[Section]) -> Vec<Vec<&str>> {
        sections
            .iter()
            .map(|s| s.units.iter().map(|u| u.unit_id.0.as_str()).collect())
            .collect()
    }

    /// A document with no blocks, so the index knows no heading — enough for
    /// the grouping rules, which read the index only for a name.
    fn empty_doc() -> crate::parser::Document {
        crate::parser::parse("").expect("parses")
    }

    /// The units `build_batches` would build for `src`, and the index it would
    /// build them with — so the stack assertions below run over the real
    /// snippets, not over hand-written ones.
    fn parsed(src: &str) -> (crate::parser::Document, Vec<TranslationUnit>) {
        let mut doc = crate::parser::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = crate::unit::html_outcomes(&doc);
        let index = crate::unit::context::build_index(&doc);
        let units = doc
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| transync_syntax::outcome::is_translatable_block(b, &outcomes))
            .map(|(i, b)| {
                let (payload, input_mode, constraints) = crate::unit::payload::assemble(&doc, b);
                TranslationUnit {
                    unit_id: b.block_id.clone(),
                    block_kind: b.kind.clone(),
                    input_mode,
                    source_payload: payload,
                    context: crate::unit::context::build_context(&doc, i, &index),
                    constraints,
                    source_hash: b.source_hash,
                    batch_id: BatchId::new(0),
                    retry: None,
                }
            })
            .collect();
        (doc, units)
    }

    /// The heading stacks of `src`'s sections, in document order.
    fn stacks(src: &str) -> Vec<Vec<String>> {
        let (doc, units) = parsed(src);
        let index = crate::unit::context::build_index(&doc);
        partition_by_section(units, &index)
            .into_iter()
            .map(|s| s.heading_stack)
            .collect()
    }

    /// The two rules together: a heading opens a section and belongs to it, and
    /// whatever precedes the first heading is the preamble section.
    #[test]
    fn a_heading_opens_a_section_and_the_preamble_is_one_of_its_own() {
        let doc = empty_doc();
        let sections = partition_by_section(
            vec![
                unit("p-0001", BlockKind::Paragraph),
                unit("h-0002", BlockKind::Heading2),
                unit("p-0003", BlockKind::Paragraph),
                unit("p-0004", BlockKind::Paragraph),
                unit("h-0005", BlockKind::Heading2),
                unit("p-0006", BlockKind::Paragraph),
            ],
            &crate::unit::context::build_index(&doc),
        );
        assert_eq!(
            ids(&sections),
            vec![
                vec!["p-0001"],
                vec!["h-0002", "p-0003", "p-0004"],
                vec!["h-0005", "p-0006"],
            ]
        );
    }

    /// Depth is not the question — "is this a heading" is. An h1 → h3 jump, and
    /// a climb back out to h2, each open a section of their own.
    #[test]
    fn every_heading_level_opens_a_section_including_a_skipped_level() {
        let doc = empty_doc();
        let sections = partition_by_section(
            vec![
                unit("h-0001", BlockKind::Heading1),
                unit("h-0002", BlockKind::Heading3),
                unit("p-0003", BlockKind::Paragraph),
                unit("h-0004", BlockKind::Heading2),
                unit("p-0005", BlockKind::Paragraph),
            ],
            &crate::unit::context::build_index(&doc),
        );
        assert_eq!(
            ids(&sections),
            vec![
                vec!["h-0001"],
                vec!["h-0002", "p-0003"],
                vec!["h-0004", "p-0005"],
            ],
            "a section with nothing but its own heading is still a section"
        );
    }

    /// Only headings partition. Every other kind — list items, blockquotes,
    /// tables, code, html — stays where document order put it, so a section is
    /// never split by what it happens to contain.
    #[test]
    fn no_other_block_kind_opens_a_section() {
        let doc = empty_doc();
        let sections = partition_by_section(
            vec![
                unit("h-0001", BlockKind::Heading2),
                unit(
                    "li-0002",
                    BlockKind::ListItem {
                        ordered: false,
                        task: None,
                    },
                ),
                unit("bq-0003", BlockKind::Blockquote),
                unit("tbl-0004", BlockKind::Table),
                unit(
                    "code-0005",
                    BlockKind::CodeBlock {
                        info: None,
                        fenced: true,
                    },
                ),
                unit("html-0006", BlockKind::Html),
            ],
            &crate::unit::context::build_index(&doc),
        );
        assert_eq!(sections.len(), 1, "{:?}", ids(&sections));
        assert_eq!(sections[0].units.len(), 6);
    }

    /// A document with no heading is one section, which is what keeps a flat
    /// document packing exactly as it did before DCR-0027.
    #[test]
    fn a_document_without_headings_is_a_single_section() {
        let doc = empty_doc();
        let sections = partition_by_section(
            vec![
                unit("p-0001", BlockKind::Paragraph),
                unit("p-0002", BlockKind::Paragraph),
            ],
            &crate::unit::context::build_index(&doc),
        );
        assert_eq!(ids(&sections), vec![vec!["p-0001", "p-0002"]]);
        assert!(
            sections[0].heading_stack.is_empty(),
            "no heading encloses it, so nothing can select it"
        );
    }

    /// Empty in, empty out — `build_batches` returns before this on an empty
    /// document, but a partition that invented a section would be a batch of
    /// nothing.
    #[test]
    fn no_units_means_no_sections() {
        let doc = empty_doc();
        assert!(
            partition_by_section(Vec::new(), &crate::unit::context::build_index(&doc)).is_empty()
        );
    }

    /// P5: the partition is a pure function of the list, so two runs over the
    /// same units agree exactly — packing has to stay deterministic across
    /// retry rounds (ADR-0009).
    #[test]
    fn the_partition_is_deterministic() {
        let src = "preamble\n\n# Guide\n\n## Install\n\nprose\n";
        assert_eq!(stacks(src), stacks(src));
        let (doc, units) = parsed(src);
        let index = crate::unit::context::build_index(&doc);
        assert_eq!(
            ids(&partition_by_section(units, &index)),
            vec![vec!["p-0001"], vec!["h1-0002"], vec!["h2-0003", "p-0004"],]
        );
    }

    /// G3's input, built the way a run builds it: the stack is ancestors plus
    /// **the opening heading itself**, which no unit's own `section_path`
    /// carries. A term scoped to `"Guide"` has to reach `## Install` — that is
    /// subsection inheritance — and a term scoped to `"Install"` has to reach
    /// the `## Install` heading unit, whose wire path names only `Guide`.
    #[test]
    fn a_sections_heading_stack_is_its_ancestors_plus_its_own_heading() {
        assert_eq!(
            stacks("preamble\n\n# Guide\n\nintro\n\n## Install\n\nprose\n\n# Other\n\nx\n"),
            vec![
                Vec::<String>::new(),
                vec!["Guide".to_string()],
                vec!["Guide".to_string(), "Install".to_string()],
                vec!["Other".to_string()],
            ]
        );
    }

    /// The stack carries the heading as **prose**, the same rendering the
    /// wire's `section_path` snippets carry — markers and inline delimiters
    /// consumed, content kept — so a selector matches an ancestor and an opener
    /// under one spelling.
    #[test]
    fn a_headings_own_name_is_its_plain_text() {
        assert_eq!(
            stacks("## `code` and *emphasis*\n\nprose\n"),
            vec![vec!["code and emphasis".to_string()]]
        );
    }
}
