//! Per-block-kind structural validators.
//!
//! **Why an html-segments unit takes one arm for every kind (spec §6).** For
//! an HTML-spelled unit, structure never crosses the wire: `assemble` sends a
//! JSON array of decoded text segments and keeps the markup in
//! `constraints.html.source_bytes`, documented "never sent to the model". The
//! model physically cannot change a column count, a heading level or a list
//! topology it never saw. The enforcement point is layer 3's splice plus its
//! ordered tag ledger, which proves the output block's tag skeleton equals the
//! source's exactly — and tag-sequence identity SUBSUMES column-count
//! identity. So per-kind structural validation for (any kind × html-segments)
//! is satisfied by construction, and `check_table` deliberately gains no html
//! variant: an HTML `<table>` does have a column count, and checking it here
//! would be strictly weaker than the ledger. The honest shape is an early
//! dispatch, not an added case.
//!
//! TRACE: SCN-02
//! TRACE: SCN-04
//! TRACE: SCN-05

use crate::id::BlockKind;
use crate::llm::{BlockConstraints, InputMode, ListTopologyEntry, UnitResult};
use crate::structure::{inspect_blockquote_children, inspect_list_topology, inspect_table};

/// Dispatch to the per-kind validator for `kind`. Returns `Ok(())` if no
/// per-kind constraint applies (e.g. paragraphs in SL-01).
///
/// TRACE: SCN-02
/// TRACE: SCN-04
/// TRACE: SCN-05
pub fn check(
    constraints: &BlockConstraints,
    kind: &BlockKind,
    input_mode: &InputMode,
    result: &UnitResult,
) -> Result<(), String> {
    // The first arm, and it is first for the reason in the module doc: an
    // html-segments unit's shape question is answered by `check_html` whatever
    // its semantic kind is.
    if matches!(input_mode, InputMode::HtmlSegments) {
        return check_html(constraints, result);
    }
    match kind {
        BlockKind::Heading1
        | BlockKind::Heading2
        | BlockKind::Heading3
        | BlockKind::Heading4
        | BlockKind::Heading5
        | BlockKind::Heading6 => check_heading(constraints, result),
        BlockKind::Paragraph => Ok(()),
        BlockKind::Table => check_table(constraints, result),
        BlockKind::CodeBlock { .. } => check_code(constraints, result),
        BlockKind::ListItem { .. } => check_list(constraints, result),
        BlockKind::Blockquote => check_blockquote(constraints, result),
        // Defensive: a kind-Html unit is an html-segments unit by the §6
        // invariant, so the arm above already answered. Kept because the kind
        // still exists and the match is exhaustive by policy.
        BlockKind::Html => check_html(constraints, result),
        // Skipped blocks are never batched (A3), and a `Title` unit is an
        // html-segments unit that the first arm already answered, so both are
        // defensive and unreachable — kept to keep the match total.
        BlockKind::ThematicBreak
        | BlockKind::Image
        | BlockKind::Title
        | BlockKind::Skipped { .. } => Ok(()),
    }
}

/// Validate a translated heading unit's level by reparsing the payload
/// under the canonical GFM options. Accepts both ATX (`# Title`) and
/// setext (`Title` underlined with `===`/`---`) forms; only the parsed
/// heading level must match the source's.
///
/// TRACE: SCN-01
pub fn check_heading(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let expected = match constraints.must_preserve_heading_level {
        Some(l) => l,
        None => return Ok(()),
    };
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let root = crate::markdown::guarded_parse(&arena, &result.translated_payload, &opts)
        .map_err(|too_deep| too_deep.to_string())?;
    let level = root.children().find_map(|c| match &c.data.borrow().value {
        NodeValue::Heading(h) => Some(h.level),
        _ => None,
    });
    match level {
        None => Err("translated payload did not parse as a heading".into()),
        Some(l) if l != expected => Err(format!(
            "heading level changed: expected {expected}, got {l}"
        )),
        Some(_) => Ok(()),
    }
}

/// Validate a translated table unit against its column-count and row-count
/// constraints. Re-parses the translated payload as a GFM document and
/// asserts shape parity with the source.
///
/// TRACE: SCN-02
pub fn check_table(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let (cols, alignments, rows) = match inspect_table(&result.translated_payload) {
        Some(t) => t,
        None => return Err("translated payload did not parse as a single GFM table".into()),
    };
    if let Some(expected_cols) = constraints.must_preserve_table_columns
        && cols != expected_cols
    {
        return Err(format!(
            "table column count changed: expected {expected_cols}, got {cols}"
        ));
    }
    if let Some(expected_rows) = constraints.must_preserve_table_row_count
        && rows != expected_rows
    {
        return Err(format!(
            "table row count changed: expected {expected_rows}, got {rows}"
        ));
    }
    if let Some(expected_alignment) = &constraints.must_preserve_table_alignment
        && expected_alignment != &alignments
    {
        return Err(format!(
            "table alignment changed: expected {expected_alignment:?}, got {alignments:?}"
        ));
    }
    Ok(())
}

/// Validate a translated list unit's topology.
///
/// Compares the fingerprint of the translated payload against the one
/// captured from the source at unit-build time: per item its
/// `(depth, ordered, task, child_kinds)` plus the owning ordered list's
/// `(start, delimiter, tight)` marker facts (EXT-2026-07 P2-9). A
/// divergence names the specific field that changed.
///
/// TRACE: SCN-05
pub fn check_list(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let expected = match &constraints.expected_list_topology {
        Some(t) => t,
        None => return Ok(()),
    };
    let actual = inspect_list_topology(&result.translated_payload)
        .ok_or_else(|| "translated payload did not parse as a list-item subtree".to_string())?;
    if actual.len() != expected.len() {
        return Err(format!(
            "list topology length changed: expected {} entries, got {}",
            expected.len(),
            actual.len()
        ));
    }
    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        if let Some(diff) = list_entry_divergence(e, a) {
            return Err(format!("list topology entry {i} {diff}"));
        }
    }
    Ok(())
}

/// First diverging field between an expected and actual list-topology
/// entry, phrased for the rejection reason. `None` when the entries match.
/// EXT-2026-07 P2-9.
fn list_entry_divergence(e: &ListTopologyEntry, a: &ListTopologyEntry) -> Option<String> {
    if e.depth != a.depth {
        return Some(format!(
            "nesting depth changed: expected {}, got {}",
            e.depth, a.depth
        ));
    }
    if e.ordered != a.ordered {
        return Some(format!(
            "ordered flag changed: expected {}, got {}",
            e.ordered, a.ordered
        ));
    }
    if e.task != a.task {
        return Some(format!(
            "task marker changed: expected {:?}, got {:?}",
            e.task, a.task
        ));
    }
    if e.start != a.start {
        return Some(format!(
            "ordered-list start ordinal changed: expected {:?}, got {:?}",
            e.start, a.start
        ));
    }
    if e.delimiter != a.delimiter {
        return Some(format!(
            "ordered-list delimiter changed: expected {:?}, got {:?}",
            e.delimiter, a.delimiter
        ));
    }
    if e.tight != a.tight {
        return Some(format!(
            "list tightness changed: expected {:?}, got {:?}",
            e.tight, a.tight
        ));
    }
    if e.child_kinds != a.child_kinds {
        return Some(format!(
            "item child structure changed: expected {:?}, got {:?}",
            e.child_kinds, a.child_kinds
        ));
    }
    None
}

/// Validate a translated blockquote unit. Asserts the child-kind sequence
/// of the translated payload matches the source's.
///
/// EXT-2026-07 P2-9: each child label carries a **one-level** structural
/// suffix (a nested list's marker facts + item markers; a nested quote's
/// own direct-child kinds — see [`crate::structure::inspect_blockquote_children`]),
/// so a reshape one level deep inside the quote is rejected. Structure
/// nested more than one level below the quote is not fingerprinted.
///
/// TRACE: SCN-06
pub fn check_blockquote(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let expected = match &constraints.expected_blockquote_children {
        Some(k) => k,
        None => return Ok(()),
    };
    let actual = inspect_blockquote_children(&result.translated_payload)
        .ok_or_else(|| "translated payload did not parse as a single blockquote".to_string())?;
    if actual.len() != expected.len() {
        return Err(format!(
            "blockquote child count changed: expected {} children, got {}",
            expected.len(),
            actual.len()
        ));
    }
    for (i, (e, a)) in expected.iter().zip(actual.iter()).enumerate() {
        if e != a {
            return Err(format!(
                "blockquote child {i} kind diverged: expected {e}, got {a}"
            ));
        }
    }
    Ok(())
}

/// Spec §4.2 layer 2 (html), SHAPE only: a JSON array of strings whose
/// element count equals the source segment count. Retryable.
///
/// Blankness is NOT checked here. It used to be — one arm rejecting a
/// segment whose chars were `all char::is_whitespace` — and that arm carried
/// two mistakes worth remembering (ti `c887bc`):
///
/// 1. Its predicate missed every zero-width and format character. U+200B,
///    U+2060, U+00AD and U+FEFF are not `White_Space`, so a segment of only
///    those passed, spliced back as markup-preserving invisible text, and
///    shipped as `translated` with the text gone — the exact attack the arm
///    was written to stop, one character class over.
/// 2. It justified being unconditional with "source segments carry at least
///    one non-whitespace char by construction — the extraction drop step",
///    which is false: `transync_html::extract`'s drop test is the same
///    `char::is_whitespace`, so a zero-width-only source segment is KEPT and
///    sent, and its faithful echo was rejected.
///
/// Both are fixed by making the question a comparison rather than a
/// predicate, which needs the source payload this function does not take.
/// It lives in [`super::text_presence`] now — one definition of "renders
/// nothing" for this intake and the Markdown one both.
pub fn check_html(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let Some(h) = &constraints.html else {
        return Ok(());
    };
    let segs: Vec<String> = serde_json::from_str(&result.translated_payload)
        .map_err(|e| format!("html payload is not a JSON array of strings: {e}"))?;
    if segs.len() != h.segment_count as usize {
        return Err(format!(
            "html segment count changed: expected {}, got {}",
            h.segment_count,
            segs.len()
        ));
    }
    Ok(())
}

/// Validate a translated code-block unit: the payload's fence info string
/// must equal the source's, in BOTH directions.
///
/// R0003-0043: the absent direction counts too. `regen::regenerate_code_block`
/// takes the info string from the *translated payload*, so an infoless source
/// fence whose translation comes back as ```` ```rust ```` emitted an invented
/// language into `out.md` — the LLM deciding structure, which architectural
/// invariant 2 ("must not alter … code fence/language metadata") reserves for
/// the application, and which no decision permits in the None→Some direction.
/// The check compares the whole expectation instead of early-returning on an
/// absent one; Comrak reports an infoless fence as `Some("")`, so the empty
/// string and `None` are treated as the same statement on both sides.
/// Rejection is retryable like the rest of layer 2.
///
/// TRACE: SCN-04
pub fn check_code(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let expected_info = constraints
        .must_preserve_code_fence_info
        .as_deref()
        .unwrap_or_default();
    // `None` here means "the payload does not parse as a code block at all";
    // its info string is absent either way, which is what the source claims
    // when `expected_info` is empty. When the source HAS an info string that
    // same absence is the mismatch this check exists to catch.
    let actual_info = extract_code_info(&result.translated_payload).unwrap_or_default();
    if actual_info != expected_info {
        return Err(format!(
            "code fence info changed: expected {expected_info:?}, got {actual_info:?}"
        ));
    }
    Ok(())
}

/// Extract the info string from a fenced-code-block fragment by parsing it
/// with Comrak. Returns `None` when the fragment does not parse as a
/// single code block.
///
/// TRACE: SCN-04
pub fn extract_code_info(fragment: &str) -> Option<String> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let root = crate::markdown::guarded_parse(&arena, fragment, &opts).ok()?;
    for child in root.children() {
        if let NodeValue::CodeBlock(c) = &child.data.borrow().value {
            return Some(c.info.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::BlockId;
    use crate::llm::OutputKind;
    use crate::structure::{inspect_blockquote_children, inspect_list_topology};

    fn unit_result(payload: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId("x-0001".to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: payload.to_string(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn list_child_structure_change_is_rejected() {
        // R0008-0014: an item that gains a second paragraph must fail even
        // though the (depth, ordered, task) topology is unchanged.
        let source = "- item one\n- item two\n";
        let c = BlockConstraints {
            expected_list_topology: inspect_list_topology(source),
            ..BlockConstraints::default()
        };
        assert!(
            check_list(&c, &unit_result("- 항목 1\n- 항목 2\n")).is_ok(),
            "faithful round-trip passes"
        );
        let tampered = "- item one\n\n  injected second paragraph\n- item two\n";
        assert!(
            check_list(&c, &unit_result(tampered)).is_err(),
            "an added child block inside an item is caught"
        );
    }

    #[test]
    fn blockquote_extra_child_is_rejected() {
        // R0008-0015: an extra child inside a blockquote changes the count.
        let source = "> quoted paragraph\n";
        let c = BlockConstraints {
            expected_blockquote_children: inspect_blockquote_children(source),
            ..BlockConstraints::default()
        };
        assert!(
            check_blockquote(&c, &unit_result("> 인용문\n")).is_ok(),
            "faithful round-trip passes"
        );
        let tampered = "> 인용문\n>\n> injected second paragraph\n";
        assert!(
            check_blockquote(&c, &unit_result(tampered)).is_err(),
            "an added blockquote child is caught"
        );
    }

    #[test]
    fn ordered_list_start_change_is_rejected() {
        // EXT-2026-07 P2-9: a list opening at 3 must not be renumbered to 1.
        let source = "3. a\n4. b\n";
        let c = BlockConstraints {
            expected_list_topology: inspect_list_topology(source),
            ..BlockConstraints::default()
        };
        assert!(
            check_list(&c, &unit_result("3. 가\n4. 나\n")).is_ok(),
            "faithful round-trip preserves the start ordinal"
        );
        let err = check_list(&c, &unit_result("1. 가\n2. 나\n")).unwrap_err();
        assert!(err.contains("start ordinal"), "got: {err}");
    }

    #[test]
    fn ordered_list_delimiter_change_is_rejected() {
        // EXT-2026-07 P2-9: `.` must not become `)`.
        let source = "1. a\n2. b\n";
        let c = BlockConstraints {
            expected_list_topology: inspect_list_topology(source),
            ..BlockConstraints::default()
        };
        assert!(check_list(&c, &unit_result("1. 가\n2. 나\n")).is_ok());
        let err = check_list(&c, &unit_result("1) 가\n2) 나\n")).unwrap_err();
        assert!(err.contains("delimiter"), "got: {err}");
    }

    #[test]
    fn list_tightness_change_is_rejected() {
        // EXT-2026-07 P2-9: a tight list must not become loose.
        let source = "- a\n- b\n";
        let c = BlockConstraints {
            expected_list_topology: inspect_list_topology(source),
            ..BlockConstraints::default()
        };
        assert!(check_list(&c, &unit_result("- 가\n- 나\n")).is_ok());
        let err = check_list(&c, &unit_result("- 가\n\n- 나\n")).unwrap_err();
        assert!(err.contains("tightness"), "got: {err}");
    }

    #[test]
    fn blockquote_nested_quote_reshape_is_rejected() {
        // EXT-2026-07 P2-9: the outer quote keeps two direct children
        // (paragraph + nested quote), so only the one-level recursion into
        // the nested quote catches the added paragraph inside it.
        let source = "> intro\n>\n> > nested\n";
        let c = BlockConstraints {
            expected_blockquote_children: inspect_blockquote_children(source),
            ..BlockConstraints::default()
        };
        assert!(
            check_blockquote(&c, &unit_result(source)).is_ok(),
            "faithful round-trip passes"
        );
        let tampered = "> intro\n>\n> > nested\n> >\n> > added\n";
        let err = check_blockquote(&c, &unit_result(tampered)).unwrap_err();
        assert!(err.contains("kind diverged"), "got: {err}");
    }

    // Spec §4.2 layer 2: html payload shape — all retryable.
    fn html_constraints(count: u32) -> BlockConstraints {
        BlockConstraints {
            html: Some(crate::llm::HtmlSegmentConstraints {
                segment_count: count,
                segment_labels: vec!["p".to_string(); count as usize],
                source_bytes: String::new(),
                block_type: 6,
            }),
            ..BlockConstraints::default()
        }
    }

    #[test]
    fn html_payload_that_is_not_json_is_rejected() {
        let err = check(
            &html_constraints(1),
            &BlockKind::Html,
            &InputMode::HtmlSegments,
            &unit_result("not json"),
        )
        .unwrap_err();
        assert!(err.contains("JSON array"), "got: {err}");
    }

    #[test]
    fn html_segment_count_change_is_rejected() {
        let err = check(
            &html_constraints(2),
            &BlockKind::Html,
            &InputMode::HtmlSegments,
            &unit_result("[\"only one\"]"),
        )
        .unwrap_err();
        assert!(err.contains("segment count"), "got: {err}");
    }

    /// The shape layer is text-blind on purpose now: an erased segment has
    /// the right SHAPE, and saying so is what makes `text_presence` the one
    /// place that owns the erasure question (ti `c887bc`). Both erasure
    /// cases this arm used to reject — `""` and `" "` — are re-pinned as
    /// rejections in `super::text_presence`'s tests, where the source
    /// payload is available to scope them.
    #[test]
    fn an_erased_segment_still_has_the_right_shape() {
        for payload in ["[\"ok\",\"\"]", "[\"ok\",\" \"]", "[\"ok\",\"\u{feff}\"]"] {
            assert!(
                check(
                    &html_constraints(2),
                    &BlockKind::Html,
                    &InputMode::HtmlSegments,
                    &unit_result(payload),
                )
                .is_ok(),
                "{payload}: shape is intact, so this layer must pass it",
            );
        }
    }

    #[test]
    fn well_shaped_html_payload_passes() {
        assert!(
            check(
                &html_constraints(2),
                &BlockKind::Html,
                &InputMode::HtmlSegments,
                &unit_result("[\"하나\",\"둘\"]"),
            )
            .is_ok()
        );
    }

    fn code_constraints(info: Option<&str>) -> BlockConstraints {
        BlockConstraints {
            must_preserve_code_fence_info: info.map(str::to_string),
            ..BlockConstraints::default()
        }
    }

    fn code_kind(info: Option<&str>) -> BlockKind {
        BlockKind::CodeBlock {
            info: info.map(str::to_string),
            fenced: true,
        }
    }

    #[test]
    fn infoless_code_fence_may_not_gain_an_info_string() {
        // R0003-0043: the direction the early return used to wave through.
        // regen reads the info string off the payload, so accepting this
        // would emit ```rust into out.md for a source that said ```.
        let err = check(
            &code_constraints(None),
            &code_kind(None),
            &InputMode::FullCodeBlock {
                language_info: None,
            },
            &unit_result("```rust\nlet x = 1;\n```\n"),
        )
        .unwrap_err();
        assert!(err.contains("code fence info changed"), "got: {err}");
        assert!(err.contains("rust"), "the invented info is named: {err}");
    }

    #[test]
    fn infoless_code_fence_round_trips() {
        // The same absent expectation still accepts every honest answer:
        // an infoless fence back (Comrak reports its info as ""), and a
        // payload that does not parse as a code block at all (no info
        // string to invent — regen falls back to the source's None).
        for payload in ["```\nlet x = 1;\n```\n", "let x = 1;\n"] {
            assert!(
                check(
                    &code_constraints(None),
                    &code_kind(None),
                    &InputMode::FullCodeBlock {
                        language_info: None
                    },
                    &unit_result(payload)
                )
                .is_ok(),
                "payload {payload:?} must pass"
            );
        }
    }

    #[test]
    fn present_code_fence_info_must_survive_verbatim() {
        // The Some→Some rule the fix must not disturb, in both failure
        // directions: changed, and dropped.
        let c = code_constraints(Some("rust"));
        assert!(
            check(
                &c,
                &code_kind(Some("rust")),
                &InputMode::FullCodeBlock {
                    language_info: None
                },
                &unit_result("```rust\nlet x = 1;\n```\n")
            )
            .is_ok()
        );
        for payload in ["```python\nx = 1\n```\n", "```\nlet x = 1;\n```\n"] {
            let err = check(
                &c,
                &code_kind(Some("rust")),
                &InputMode::FullCodeBlock {
                    language_info: None,
                },
                &unit_result(payload),
            )
            .unwrap_err();
            assert!(err.contains("code fence info changed"), "got: {err}");
        }
    }

    #[test]
    fn blockquote_nested_list_marker_flip_is_rejected() {
        // EXT-2026-07 P2-9: an unordered nested list flipped to ordered
        // inside the quote changes the nested list's label.
        let source = "> intro\n>\n> - a\n> - b\n";
        let c = BlockConstraints {
            expected_blockquote_children: inspect_blockquote_children(source),
            ..BlockConstraints::default()
        };
        assert!(check_blockquote(&c, &unit_result(source)).is_ok());
        let tampered = "> intro\n>\n> 1. a\n> 2. b\n";
        let err = check_blockquote(&c, &unit_result(tampered)).unwrap_err();
        assert!(err.contains("kind diverged"), "got: {err}");
    }
}
