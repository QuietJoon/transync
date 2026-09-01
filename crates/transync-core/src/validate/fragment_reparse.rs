//! Fragment reparse: parse the translated payload as the same block kind
//! it claimed to be, in isolation.
//!
//! TRACE: SCN-02
//! TRACE: SCN-04

use crate::id::BlockKind;
use crate::llm::{InputMode, UnitResult};
use comrak::nodes::NodeValue;

/// Reparse one translated payload under GFM and assert kind equality.
///
/// TRACE: SCN-02
pub fn reparse_fragment(
    kind: &BlockKind,
    input_mode: &InputMode,
    result: &UnitResult,
) -> Result<(), String> {
    if result.translated_payload.trim().is_empty() {
        return Err("empty translated payload".into());
    }

    if matches!(input_mode, InputMode::HtmlSegments) {
        // Spec §4.2: html payloads are JSON segment arrays, not Markdown —
        // structure is owned by the splice check (layer 3), not a comrak
        // reparse. ti 490d97 wave 2 re-keyed this from the kind onto the MODE:
        // an HTML document's `<p>` is `BlockKind::Paragraph` and still ships a
        // segment array, so a kind-keyed early return would hand its JSON to
        // comrak and reject every unit in the document.
        return Ok(());
    }

    let arena = comrak::Arena::new();
    let opts = crate::parser::comrak_options();
    // Guarded because this is provider bytes; `validate_unit` refuses past
    // the ceiling before we get here, so this is the belt to that braces —
    // it keeps a future caller of this function from reopening the abort.
    let root = crate::parser::guarded_parse(&arena, &result.translated_payload, &opts)
        .map_err(|too_deep| too_deep.to_string())?;

    let mut found_kinds: Vec<&'static str> = Vec::new();
    for child in root.children() {
        let label: &'static str = match &child.data.borrow().value {
            NodeValue::Heading(_) => "heading",
            NodeValue::Paragraph => "paragraph",
            NodeValue::Table(_) => "table",
            NodeValue::CodeBlock(_) => "code-block",
            NodeValue::List(_) => "list",
            NodeValue::Item(_) | NodeValue::TaskItem(_) => "list-item",
            NodeValue::BlockQuote => "blockquote",
            NodeValue::ThematicBreak => "thematic-break",
            // Raw HTML must never appear in the output of a NON-html unit.
            // Label it so it can never satisfy such an expected kind:
            // leading HTML fails the primary-kind check, trailing HTML fails
            // the trailing-block check. Html units themselves never reach
            // this loop — they return early above, because their payload is
            // a JSON segment array rather than Markdown. Do NOT mirror this
            // in full_reparse — source documents legitimately carry HTML
            // comments in inter-block gaps that the IR skips but regen
            // preserves.
            NodeValue::HtmlBlock(_) => "html-block",
            _ => continue,
        };
        found_kinds.push(label);
    }

    let expected = expected_label(kind);
    let primary = match found_kinds.first() {
        Some(k) => *k,
        None => return Err("translated payload contained no recognized block".into()),
    };

    if primary != expected {
        return Err(format!(
            "fragment reparse kind mismatch: expected {expected}, got {primary}"
        ));
    }

    // Reject extra blocks past the expected one. A unit-shaped reparse
    // must contain exactly one top-level recognized block — anything
    // beyond that is a structural drift the model introduced (most
    // commonly an extra paragraph appended after a code fence or a
    // trailing instruction echo). The "list" expectation is the one
    // exception: a list-item unit reparses as a `list` containing
    // sub-items, and Comrak coalesces consecutive list nodes, so we
    // accept the singular `list` shape here too.
    if found_kinds.len() > 1 {
        return Err(format!(
            "fragment reparse trailing-block mismatch: expected one {expected}, got {} blocks ({})",
            found_kinds.len(),
            found_kinds.join(", ")
        ));
    }
    Ok(())
}

fn expected_label(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Heading1
        | BlockKind::Heading2
        | BlockKind::Heading3
        | BlockKind::Heading4
        | BlockKind::Heading5
        | BlockKind::Heading6 => "heading",
        BlockKind::Paragraph => "paragraph",
        BlockKind::Table => "table",
        BlockKind::CodeBlock { .. } => "code-block",
        BlockKind::ListItem { .. } => "list",
        BlockKind::Blockquote => "blockquote",
        BlockKind::ThematicBreak => "thematic-break",
        BlockKind::Image => "paragraph",
        // Defensive and unreachable: an html unit's payload is a JSON segment
        // array (spec §4.1), so `reparse_fragment` returns early on the MODE
        // (ti 490d97 wave 2) and never asks for its label — a kind-Html unit
        // is an html-segments unit by the §6 invariant. Kept to keep the match
        // total; the label is the one a raw HTML block WOULD reparse as.
        BlockKind::Html => "html-block",
        // Defensive and unreachable for the same reason as `Html`: a Title
        // unit's payload is a JSON segment array, so `reparse_fragment`
        // returns early on the mode and never asks for a label. `title` is a
        // CommonMark type-6 tag, so `html-block` is what one WOULD reparse as.
        BlockKind::Title => "html-block",
        // Skipped blocks are never batched (A3), so this arm is defensive
        // and unreachable — kept to keep the match total.
        BlockKind::Skipped { .. } => "skipped",
    }
}
