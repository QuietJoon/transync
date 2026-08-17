//! Wire-form kind labels for block-level AST nodes.
//!
//! These strings *are* the structural fingerprint that `validate::per_kind`
//! compares across the source/translation boundary, so their exact spelling
//! is load-bearing on both sides — changing a label changes the fingerprint.
//!
//! TRACE: SCN-05
//! TRACE: SCN-06

/// Wire-form kind label for one block-level AST node. Unknown / unsupported
/// node kinds map to an explicit `"unknown"` label so they still COUNT in a
/// child-kind fingerprint (R0008-0014 / R0008-0015) instead of being
/// silently dropped, which would let a provider insert or remove them
/// undetected.
pub(crate) fn block_node_kind_label(value: &comrak::nodes::NodeValue) -> String {
    use comrak::nodes::NodeValue;
    match value {
        NodeValue::Heading(h) => format!("heading-{}", h.level),
        NodeValue::Paragraph => "paragraph".to_string(),
        NodeValue::Table(_) => "table".to_string(),
        NodeValue::CodeBlock(_) => "code-block".to_string(),
        NodeValue::List(_) => "list".to_string(),
        NodeValue::Item(_) | NodeValue::TaskItem(_) => "list-item".to_string(),
        NodeValue::BlockQuote => "blockquote".to_string(),
        NodeValue::ThematicBreak => "thematic-break".to_string(),
        NodeValue::HtmlBlock(_) => "html-block".to_string(),
        _ => "unknown".to_string(),
    }
}

/// Direct block-child kinds of a list item, in order (R0008-0014).
pub(crate) fn item_child_kinds<'a>(item: &'a comrak::nodes::AstNode<'a>) -> Vec<String> {
    item.children()
        .map(|n| block_node_kind_label(&n.data.borrow().value))
        .collect()
}

/// One-level structural label for a blockquote's direct child (EXT-2026-07
/// P2-9, OI-0022 / DCR-0007). Non-structure-bearing children keep their
/// plain wire-form kind label (`"paragraph"`, `"heading-2"`, …).
/// Structure-bearing children carry a compact suffix so a provider cannot
/// reshape structure *inside* the quote without changing the fingerprint:
///
/// - a direct child list records its marker facts and per-item markers,
///   e.g. `list(ordered,start=1,delim=.,tight=true;items=[item,item])`;
/// - a direct child blockquote records its own direct-child kind labels,
///   e.g. `blockquote(children=[paragraph])`.
///
/// Recursion is capped at **one** level: labels inside a nested list /
/// blockquote are plain kind labels, not themselves expanded.
pub(crate) fn blockquote_child_label<'a>(node: &'a comrak::nodes::AstNode<'a>) -> String {
    use comrak::nodes::{ListDelimType, ListType, NodeValue};
    let value = node.data.borrow().value.clone();
    match value {
        NodeValue::List(list) => {
            let ordered = matches!(list.list_type, ListType::Ordered);
            let mut facts: Vec<String> =
                vec![if ordered { "ordered" } else { "unordered" }.to_string()];
            if ordered {
                facts.push(format!("start={}", list.start));
                facts.push(format!(
                    "delim={}",
                    match list.delimiter {
                        ListDelimType::Period => ".",
                        ListDelimType::Paren => ")",
                    }
                ));
            }
            facts.push(format!("tight={}", list.tight));
            let items: Vec<String> = node
                .children()
                .map(|it| match &it.data.borrow().value {
                    NodeValue::TaskItem(marker) => match marker {
                        Some('x') | Some('X') => "task-done".to_string(),
                        _ => "task".to_string(),
                    },
                    _ => "item".to_string(),
                })
                .collect();
            format!("list({};items=[{}])", facts.join(","), items.join(","))
        }
        NodeValue::BlockQuote => {
            let kinds: Vec<String> = node
                .children()
                .map(|n| block_node_kind_label(&n.data.borrow().value))
                .collect();
            format!("blockquote(children=[{}])", kinds.join(","))
        }
        other => block_node_kind_label(&other),
    }
}
