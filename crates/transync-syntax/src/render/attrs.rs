//! Annotated HTML attribute writer.
//!
//! TRACE: contracts.md §4

use crate::align::{AlignmentBlock, SyncRole};

/// Format the canonical sync-attribute set for one block as a single string,
/// suitable for splicing into the open-tag of the block's outermost wrapper.
///
/// The four attributes below are the whole set. `data-parent-id` is
/// **RESERVED and never emitted** (`contracts.md` §4a): the parser is
/// leaf-block, so `AlignmentBlock::parent_id` — which stays on the wire at
/// schema 1.2.0 — is always `null`, and the conditional that used to read it
/// could not fire. A future nested-anchor scheme reinstates it together with
/// the reserved `child-only` sync role.
///
/// All values are HTML-attribute-escaped at the boundary even though
/// today's BlockId / kind / order strings are constrained by ADR-0005
/// (kind-prefixed alphanumeric IDs). A future addition that allows
/// arbitrary IDs or kind labels would otherwise leak `"` / `&` / `<`
/// into the output.
///
/// **A `non-sync` row gets none of them** — only `data-block-kind`, the
/// styling hook. A block that is not page content has no sync identity, so
/// emitting `data-sync-id` for it would put a DOM anchor in the pane that the
/// block's own alignment row denies (ADR-0006; `contracts.md` §4a rests on
/// the row and the DOM agreeing).
///
/// The decision is taken **here, from the row**, and that is the point of ti
/// `18b9c3`. It used to live in `render_block` as a literal
/// `matches!(kind, BlockKind::ThematicBreak)` test, which was a second
/// opinion about a question `align::sync_role_for` already answers — right
/// for the only kind that could reach it, and silently wrong for the next
/// one. `BlockKind::Title` is `SyncRole::NonSync` (decision D5) and nothing
/// mints one yet; wave 3 does, and it would have rendered with a real
/// `data-sync-id` while its row said `non-sync`. Reading `row.sync_role`
/// means the two cannot disagree: there is one value, written by
/// `sync_role_for` and read here.
///
/// TRACE: contracts.md §4
pub fn write_attrs(row: &AlignmentBlock) -> String {
    if row.sync_role == SyncRole::NonSync {
        return format!(
            " data-block-kind=\"{kind}\"",
            kind = escape_attr(&row.block_kind)
        );
    }
    let fallback = super::fallback_status_str(row.fallback_status);
    format!(
        " data-sync-id=\"{id}\" data-block-kind=\"{kind}\" data-order=\"{order}\" data-fallback=\"{fb}\"",
        id = escape_attr(&row.source_block_id.0),
        kind = escape_attr(&row.block_kind),
        order = row.source_order,
        fb = escape_attr(fallback),
    )
}

/// Minimal HTML attribute escaping for double-quoted contexts. Replaces
/// `&`, `"`, `<`, and `>` with their entity equivalents. `'` is safe
/// inside `"…"`. Unicode is passed through untouched.
///
/// `pub(crate)` so the `Skipped` placeholder renderer can escape its
/// `data-skipped` label (A6).
pub(crate) fn escape_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
    out
}
