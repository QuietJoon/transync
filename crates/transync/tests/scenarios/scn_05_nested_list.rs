//! SCN-05 — Nested list with task items.
//!
//! Done-gate criteria from `docs/project/implementation-slice-checklists.md` SL-05:
//! - Source depth-3 structure survives in the translated MD.
//! - Checked / unchecked task markers preserved per item.
//! - Ordered / unordered marker types preserved.
//! - Asserts the topology tuple sequence.
//!
//! TRACE: SCN-05
//! TRACE: SL-05

use crate::common::mock_translator::MockTranslator;
use transync::translate;

#[tokio::test]
async fn smoke_scn_05() {
    let source = include_str!("../fixtures/scn-05-nested-list.md");
    let translator = MockTranslator::passthrough();
    let opts = crate::common::opts_for("ko");

    let output = translate(source, &opts, &translator)
        .await
        .expect("pipeline returns Ok against the SCN-05 fixture");

    assert_eq!(output.alignment_map.schema_version, "1.2.0");
    assert!(!output.translated_document.is_empty());

    // `R0001-0085` in the removed `reviews/reviewed/0001.md`: pin the
    // expected topology fingerprint of the
    // regenerated MD to a frozen ground-truth tuple sequence. The prior
    // version of this test compared the source walker to the regen
    // walker — both running the same logic — so a shared bug in
    // `topology_tuples` could mask itself. Pinning the expected
    // sequence catches regressions in either the production walker or
    // the test walker.
    //
    // Ground truth captured from the working SCN-05 round-trip on
    // 2026-05-04. Three top-level lists; the fixture exercises all
    // four task-marker states plus a depth-3 nesting. If the fixture
    // changes, regenerate this list and update the table.
    let expected: Vec<(u8, bool, Option<bool>)> = vec![
        // List 1 — task list with deep nesting
        (1, false, Some(true)),
        (2, false, Some(true)),
        (2, false, Some(true)),
        (3, false, Some(true)),
        (3, false, Some(false)),
        (1, false, Some(false)),
        (2, false, Some(true)),
        (2, false, Some(false)),
        (2, false, Some(false)),
        (1, false, Some(false)),
        (2, false, Some(false)),
        (2, false, Some(false)),
        (2, false, Some(false)),
        // List 2 — ordered top-level
        (1, true, None),
        (1, true, None),
        (1, true, None),
    ];
    let regen_tuples = topology_tuples(&output.translated_document);
    assert_eq!(
        regen_tuples, expected,
        "regenerated list topology must match the frozen ground-truth fingerprint",
    );

    // Round-trip: source → regen must preserve the same fingerprint
    // (this still uses the same walker on both sides, but the
    // expected fingerprint above is the ground-truth anchor; agreement
    // here just proves the round-trip didn't introduce drift).
    let src_tuples = topology_tuples(source);
    assert_eq!(src_tuples, regen_tuples);
}

fn topology_tuples(md: &str) -> Vec<(u8, bool, Option<bool>)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let mut opts: comrak::ComrakOptions<'static> = comrak::ComrakOptions::default();
    opts.extension.tasklist = true;
    opts.extension.table = true;
    let root = comrak::parse_document(&arena, md, &opts);

    let mut out: Vec<(u8, bool, Option<bool>)> = Vec::new();
    fn walk<'a>(
        node: &'a comrak::nodes::AstNode<'a>,
        depth: u8,
        out: &mut Vec<(u8, bool, Option<bool>)>,
    ) {
        for child in node.children() {
            match child.data.borrow().value {
                NodeValue::List(_) => walk(child, depth + 1, out),
                NodeValue::Item(ref item) => {
                    out.push((
                        depth,
                        matches!(item.list_type, comrak::nodes::ListType::Ordered),
                        None,
                    ));
                    walk(child, depth, out);
                }
                NodeValue::TaskItem(marker) => {
                    let task = match marker {
                        Some('x') | Some('X') => Some(true),
                        _ => Some(false),
                    };
                    out.push((depth, false, task));
                    walk(child, depth, out);
                }
                _ => {}
            }
        }
    }
    walk(root, 0, &mut out);
    out
}
