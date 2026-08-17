//! Annotated HTML attribute writer.
//!
//! TRACE: contracts.md §4

use crate::align::AlignmentBlock;

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
/// TRACE: contracts.md §4
pub fn write_attrs(row: &AlignmentBlock) -> String {
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
