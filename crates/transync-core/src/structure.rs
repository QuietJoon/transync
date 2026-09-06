//! Structural fingerprinting — the shared "what shape is this block" oracle.
//!
//! Both sides of structural validation read from here: `unit::payload`
//! records the *source* block's fingerprint into `BlockConstraints`, and
//! `validate::per_kind` re-derives the same fingerprint from the *translated*
//! payload and compares the two. One implementation is what makes that
//! comparison meaningful — a second copy would drift and silently disarm the
//! check.
//!
//! Each inspector re-parses its payload fragment under the shared GFM options
//! (`markdown::comrak_options`), so the fingerprint is always taken with the
//! same parser the pipeline uses (invariant: no second Markdown parser).
//!
//! TRACE: SCN-02
//! TRACE: SCN-05
//! TRACE: SCN-06

pub(crate) mod labels;

use crate::llm::{ListDelimiter, ListTopologyEntry, TableAlign};
use labels::{blockquote_child_label, item_child_kinds};

/// Marker facts of the list that owns a run of items, threaded down the
/// walk so both `Item` and `TaskItem` entries record them. EXT-2026-07
/// P2-9 (OI-0022): a `TaskItem` AST node carries no list type / start /
/// delimiter of its own — those live on the enclosing `List` node — so we
/// capture them once when entering the list and replicate them onto each
/// item entry.
#[derive(Clone, Copy)]
struct ListFacts {
    ordered: bool,
    start: Option<u32>,
    delimiter: Option<ListDelimiter>,
    tight: Option<bool>,
}

impl ListFacts {
    /// Facts derived from a Comrak `NodeList`. `start`/`delimiter` are
    /// meaningful only for ordered lists and are `None` otherwise.
    fn from_list(list: &comrak::nodes::NodeList) -> Self {
        use comrak::nodes::{ListDelimType, ListType};
        let ordered = matches!(list.list_type, ListType::Ordered);
        Self {
            ordered,
            start: ordered.then_some(list.start as u32),
            delimiter: ordered.then_some(match list.delimiter {
                ListDelimType::Period => ListDelimiter::Period,
                ListDelimType::Paren => ListDelimiter::Paren,
            }),
            tight: Some(list.tight),
        }
    }
}

/// Walk a fenced list-item payload via Comrak and produce the topology
/// fingerprint — `(depth, ordered, task, child_kinds)` plus the owning
/// ordered list's `(start, delimiter, tight)` marker facts (EXT-2026-07
/// P2-9). Returns `None` when the payload does not parse as a list-item
/// subtree.
///
/// Depth is recorded exactly, at any nesting: the counter is a `u32` and so
/// is [`ListTopologyEntry::depth`](crate::llm::ListTopologyEntry::depth), so
/// there is nothing here to wrap, saturate or clamp (R0001-0025, then
/// ti 0d8277 widened the field and deleted the saturation).
///
/// TRACE: SCN-05
pub(crate) fn inspect_list_topology(source_payload: &str) -> Option<Vec<ListTopologyEntry>> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let root = comrak::parse_document(&arena, source_payload, &opts);

    let mut entries: Vec<ListTopologyEntry> = Vec::new();
    // `facts` carries the enclosing List node's marker facts so both Item
    // and TaskItem entries record whether they live in an ordered list,
    // its start ordinal, its delimiter, and its tightness. R0006-0033: an
    // ordered task list previously fingerprinted as `ordered: false`,
    // letting a translation flip the list type without failing validation.
    //
    // R0001-0025: `depth` counts in `u32`, which is also the width of the
    // field it lands in (ti 0d8277), so no arithmetic here can wrap or panic
    // on a deeply nested document and nothing is lost on the way to the
    // entry. The increment saturates on top of that, which is four billion
    // levels out of reach — but the walker is fed untrusted input and should
    // not need that argument to be sound.
    //
    // The traversal is an explicit stack rather than recursion, and that is
    // a safety property, not a style choice: this walker was the one place
    // in the workspace where untrusted nesting reached the *call* stack,
    // and it died — measured on a 2 MiB stack — at 2,193 nested list levels,
    // at 1,100 on the 1 MiB a wasm module gets. A stack overflow aborts the
    // process; a host embedding this library cannot catch it. Heap frames
    // grow instead now, so the only ceiling left is memory. The parser's
    // ti `148fcf` routed the other provider-payload reparses through
    // `markdown::guarded_parse`. These three walkers are deliberately NOT
    // routed, and that is not an oversight: they are hardened rather than
    // bounded. `deep_nesting_walks_on_the_heap_not_the_call_stack` pins a
    // thousand levels walked on a 256 KiB stack, and applying the ceiling
    // here would refuse depths this module is built to handle while turning
    // "too deep" into "not a list" — a worse diagnostic that can mask a real
    // shape change. Provider payloads are refused at the door in
    // `validate::validate_unit`, before they ever reach here; the safety this
    // module provides is what remains for any caller that arrives another way
    // (`pipeline::merge` fingerprints already-merged windows).
    // `MAX_BLOCK_NESTING_DEPTH` pre-scan keeps *source* payloads far short
    // of either, but a provider result reaches this function without
    // passing through it, which is exactly why the depth must not live on
    // the call stack.
    //
    // Order is document order: each frame is a node whose value has not been
    // read yet, and children are pushed in reverse so the leftmost pops
    // first — the same sequence the recursive form produced.
    let root_facts = ListFacts {
        ordered: false,
        start: None,
        delimiter: None,
        tight: None,
    };
    type Frame<'a> = (&'a comrak::nodes::AstNode<'a>, u32, ListFacts);
    fn push_children<'a>(
        stack: &mut Vec<Frame<'a>>,
        node: &'a comrak::nodes::AstNode<'a>,
        depth: u32,
        facts: ListFacts,
    ) {
        let first = stack.len();
        stack.extend(node.children().map(|c| (c, depth, facts)));
        stack[first..].reverse();
    }

    let mut stack: Vec<Frame<'_>> = Vec::new();
    push_children(&mut stack, root, 0, root_facts);

    while let Some((node, depth, facts)) = stack.pop() {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::List(list) => push_children(
                &mut stack,
                node,
                depth.saturating_add(1),
                ListFacts::from_list(&list),
            ),
            NodeValue::Item(item) => {
                entries.push(ListTopologyEntry {
                    depth,
                    ordered: matches!(item.list_type, comrak::nodes::ListType::Ordered),
                    task: None,
                    child_kinds: item_child_kinds(node),
                    start: facts.start,
                    delimiter: facts.delimiter,
                    tight: facts.tight,
                });
                push_children(&mut stack, node, depth, facts);
            }
            NodeValue::TaskItem(marker) => {
                let task = match marker {
                    Some('x') | Some('X') => Some(true),
                    _ => Some(false),
                };
                entries.push(ListTopologyEntry {
                    depth,
                    ordered: facts.ordered,
                    task,
                    child_kinds: item_child_kinds(node),
                    start: facts.start,
                    delimiter: facts.delimiter,
                    tight: facts.tight,
                });
                push_children(&mut stack, node, depth, facts);
            }
            // Any other node kind: not a list container, and the recursive
            // form did not descend through one either.
            _ => {}
        }
    }
    if entries.is_empty() {
        return None;
    }
    Some(entries)
}

/// Walk a blockquote payload via Comrak and capture the structural labels
/// of its direct children, in order. Structure-bearing children (lists,
/// nested blockquotes) carry a one-level structural suffix — see
/// [`blockquote_child_label`].
///
/// TRACE: SCN-06
pub(crate) fn inspect_blockquote_children(source_payload: &str) -> Option<Vec<String>> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let root = comrak::parse_document(&arena, source_payload, &opts);

    for child in root.children() {
        if matches!(child.data.borrow().value, NodeValue::BlockQuote) {
            // R0008-0015: label every direct child (unknown kinds become
            // "unknown") rather than dropping unrecognized nodes, so a
            // result-only insertion of raw HTML or an unsupported node
            // changes the fingerprint and fails validation. EXT-2026-07
            // P2-9: structure-bearing children additionally carry a
            // one-level structural suffix so a reshape *inside* the quote
            // (an item added to a nested list, a child added to a nested
            // quote, a flipped list marker) also changes the fingerprint.
            let kinds: Vec<String> = child.children().map(blockquote_child_label).collect();
            return Some(kinds);
        }
    }
    None
}

/// Re-parse a per-block GFM table fragment via Comrak and extract its
/// column count, per-column alignment, and data-row count. Returns `None`
/// if the fragment does not parse as a single table.
///
/// TRACE: SCN-02
pub(crate) fn inspect_table(source_payload: &str) -> Option<(u32, Vec<TableAlign>, u32)> {
    use comrak::nodes::{NodeValue, TableAlignment};
    let arena = comrak::Arena::new();
    let opts = crate::markdown::comrak_options();
    let root = comrak::parse_document(&arena, source_payload, &opts);
    for child in root.children() {
        if let NodeValue::Table(t) = &child.data.borrow().value {
            let alignments: Vec<TableAlign> = t
                .alignments
                .iter()
                .map(|a| match a {
                    TableAlignment::None => TableAlign::None,
                    TableAlignment::Left => TableAlign::Left,
                    TableAlignment::Center => TableAlign::Center,
                    TableAlignment::Right => TableAlign::Right,
                })
                .collect();
            let cols = alignments.len() as u32;
            // Count data rows (every TableRow child whose `header` is false).
            let mut rows: u32 = 0;
            for row in child.children() {
                if let NodeValue::TableRow(is_header) = row.data.borrow().value
                    && !is_header
                {
                    rows += 1;
                }
            }
            return Some((cols, alignments, rows));
        }
    }
    None
}

/// R0001-0025, then ti 0d8277. The topology walker runs over untrusted
/// Markdown, so its depth counter has to survive nesting no human would
/// write — and, since ti 0d8277 widened `ListTopologyEntry::depth` to `u32`,
/// has to keep *discriminating* there rather than merely not crashing.
///
/// The module name is historical: there is no ceiling any more, and these
/// tests are what says so. It is kept because it is the name every ticket,
/// review finding and CHANGELOG entry in this thread points at.
#[cfg(test)]
mod depth_ceiling_tests {
    use super::inspect_list_topology;

    /// One item per level, each level nested inside the previous.
    ///
    /// Built rather than spelled out: every level adds two more leading
    /// spaces, so a 300-level fixture is ~90 KB of indentation. It is also
    /// deliberately not a checked-in `.md` file — the OI-0033 precedent, for
    /// the same reason there: nothing in the repo guarantees an adversarial
    /// whitespace fixture survives a round trip through whatever normalizes
    /// files on the way in.
    fn nested_list(levels: usize) -> String {
        let mut src = String::new();
        for level in 0..levels {
            src.push_str(&" ".repeat(level * 2));
            src.push_str("- item\n");
        }
        src
    }

    /// The shallow case is the control: an ordinary document's items record
    /// their exact level, and always did.
    #[test]
    fn ordinary_nesting_records_the_exact_level() {
        let entries = inspect_list_topology(&nested_list(12)).expect("a list-item subtree");
        let depths: Vec<u32> = entries.iter().map(|e| e.depth).collect();
        assert_eq!(depths, (1u32..=12).collect::<Vec<u32>>());
    }

    /// 300 levels of `- item` is ~90 KB of Markdown and passes comrak
    /// untouched (its ~99-level cap applies to blocks opened on a *single*
    /// line, not to indented nesting). This fixture has caught three
    /// different states of the depth counter: it panicked with "attempt to
    /// add with overflow" in a checked build and wrapped `255 -> 0` in a
    /// release one before R0001-0025; it then flattened everything from 255
    /// on to a single saturated value; and since ti 0d8277 widened the field
    /// to `u32` it records all three hundred levels exactly, which is what
    /// this asserts — not a bound, the whole sequence.
    #[test]
    fn nesting_no_human_would_write_still_records_every_exact_level() {
        let entries = inspect_list_topology(&nested_list(300)).expect("a list-item subtree");
        assert_eq!(entries.len(), 300, "one item per level");
        let depths: Vec<u32> = entries.iter().map(|e| e.depth).collect();
        assert_eq!(
            depths,
            (1u32..=300).collect::<Vec<u32>>(),
            "every level records its own depth; the old ceiling flattened \
             everything from index 254 on to 255, and the wrap before it \
             sent index 255 back to 0"
        );
        // Spelled out at the three indices that matter, so a regression names
        // itself rather than showing a 300-long vector diff. 254 is the old
        // saturation boundary and reads the same either way; the other two are
        // where each earlier state went wrong.
        assert_eq!(entries[254].depth, 255, "the old saturation boundary");
        assert_eq!(
            entries[255].depth, 256,
            "the first level past it: `0` under the pre-R0001-0025 wrap, `255` under the saturation"
        );
        assert_eq!(
            entries[299].depth, 300,
            "the innermost level; `255` under the saturation"
        );
    }

    /// The traversal is an explicit stack, and this is what that buys:
    /// nesting deep enough to exhaust the *call* stack no longer aborts the
    /// process. The recursive form spent two frames per level and died —
    /// measured on this workspace — at 2,193 levels on the 2 MiB a test
    /// thread gets, and at 1,100 on the 1 MiB a wasm module gets. Running
    /// the walk on a deliberately small stack makes the difference
    /// observable without a million-line fixture: those two measurements
    /// put it at ~950 bytes of frames per level, so 256 KiB held about 275
    /// levels and a thousand needs the better part of a megabyte. The heap
    /// does not notice.
    ///
    /// A stack overflow is an abort, not a panic, so there is nothing here
    /// to catch: if this regresses, the whole test binary dies with
    /// `fatal runtime error: stack overflow`. That is the point — the same
    /// thing would happen to a host embedding the library.
    ///
    /// The parser's `MAX_BLOCK_NESTING_DEPTH` keeps *source* payloads three
    /// orders of magnitude short of this, but a provider result is
    /// fingerprinted by the same walker without passing through `parse`.
    #[test]
    fn deep_nesting_walks_on_the_heap_not_the_call_stack() {
        let walk = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .name("topology-depth".to_string())
            .spawn(|| inspect_list_topology(&nested_list(1_000)).map(|e| e.len()))
            .expect("spawn the small-stack walker");
        assert_eq!(
            walk.join().expect("the walk returns instead of aborting"),
            Some(1_000),
            "one entry per level, all thousand of them",
        );
    }

    /// Pathological nesting reaches the *actual* side of validation too: the
    /// same walker fingerprints a provider's translated payload, so a hostile
    /// response is no more able to overflow it than a hostile source is —
    /// and, unlike the source side, it never passed the parser's
    /// `MAX_BLOCK_NESTING_DEPTH` refusal on the way in. Comparing the two
    /// fingerprints must still work at these depths, and must still reject a
    /// payload whose shape moved.
    #[test]
    fn a_deeply_nested_payload_still_compares_on_both_sides() {
        use crate::id::BlockId;
        use crate::llm::{BlockConstraints, OutputKind, UnitResult};
        use crate::validate::per_kind::check_list;

        let result = |payload: String| UnitResult {
            unit_id: BlockId("li-0001".to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: payload,
            warnings: Vec::new(),
        };

        let deep = nested_list(300);
        let constraints = BlockConstraints {
            must_preserve_list_topology: true,
            expected_list_topology: inspect_list_topology(&deep),
            ..BlockConstraints::default()
        };
        assert!(
            check_list(&constraints, &result(deep.clone())).is_ok(),
            "an echoed payload must still validate"
        );

        // Drop the innermost level: the entry count changes, and now so do
        // the deep depths — under the old saturation everything from index
        // 254 on read the same `255` on both sides.
        assert!(
            check_list(&constraints, &result(nested_list(299))).is_err(),
            "a reshaped payload must still be rejected"
        );
    }
}
