//! Regenerate translated Markdown from the source IR + accepted payloads.
//!
//! Strategy: walk top-level blocks in source order, copy any inter-block
//! text verbatim, and splice the translated payload (or the source text on
//! fallback) at each block's source range. Per-block byte offsets into the
//! regenerated output are returned for the alignment-map builder.
//!
//! This module is deliberately validation-agnostic (spec 2026-08-04 §3.2):
//! it takes a plain `BlockId -> payload` map and trusts it. Deciding which
//! payloads are acceptable, and leaving out the ones that are not, is the
//! caller's job — a block absent from the map falls back to its own source
//! bytes, so "fell back" and "not in the map" are the same statement.
//!
//! Code blocks get special handling: regen extracts the body (sans fence)
//! from the accepted payload, then re-wraps with a fence whose length is
//! at least one greater than the longest backtick run inside the body.
//!
//! Nested content is spliced whole, and that is the permanent design rather
//! than a gap. The parser is leaf-block: a list item inside a list, or a
//! paragraph inside a blockquote, is not a block of its own — the enclosing
//! list or blockquote is ONE unit whose `source_range` already covers its
//! nested content (see [`top_level_blocks`] and `parser::Block`), so this
//! splice replaces it in one piece. Nothing here is waiting to be narrowed
//! to per-nested-item splicing; doing that would break the whole-block
//! invariant and the reserved child-only design that keeps every anchor
//! top-level ([`crate::align::AlignmentBlock::parent_id`] is on the wire and
//! always `null`, paired with the reserved `child-only` sync role). A
//! nested-anchor scheme is a contract change, not a fill-in here.
//!
//! TRACE: SCN-01
//! TRACE: SCN-04
//! TRACE: SCN-14

use crate::align::ByteRange;
use crate::id::{BlockId, BlockKind, Spelling};
use crate::parser::{Block, Document};
use std::borrow::Cow;
use std::collections::HashMap;

/// Per-block byte offsets into the regenerated MD, used to populate the
/// alignment map's `target_range`.
///
/// TRACE: SCN-12
#[derive(Debug, Clone, Default)]
pub struct BlockOffsets(pub HashMap<BlockId, ByteRange>);

/// Regenerate the full translated MD from the source IR + the accepted
/// per-block payloads.
///
/// `accepted` maps a block ID to the payload that was validated for it. A
/// block absent from the map falls back to its own source bytes — that is
/// the whole contract, so a unit that fell back is simply not in the map.
///
/// TRACE: SCN-01
/// TRACE: SCN-14
pub fn regenerate(doc: &Document, accepted: &HashMap<BlockId, String>) -> (String, BlockOffsets) {
    let mut output = String::with_capacity(doc.source_text.len());
    let mut offsets = BlockOffsets::default();
    let mut last_end = 0usize;

    for block in top_level_blocks(&doc.blocks) {
        // R0003-0059: clamped AND snapped to char boundaries, through the
        // same helper the renderer's `block_text` uses. Numeric clamping
        // alone left `&doc.source_text[bstart..bend]` able to panic on a
        // hand-built `Document` whose range splits a multi-byte char —
        // `Document` and `Block` are `pub` with `pub` fields, so that
        // document is constructible outside this crate. Parsed documents are
        // unaffected: comrak's positions are boundary-valid, so the snap is
        // the identity for every range `parse` produces.
        let (bstart, bend) =
            crate::parser::ranges::clamped_char_bounds(&doc.source_text, block.source_range);

        // Copy inter-block text verbatim (blank lines, frontmatter-like
        // whitespace between blocks).
        if bstart > last_end {
            output.push_str(&doc.source_text[last_end..bstart]);
        }

        let target_start = output.len();

        // Borrowed, never copied: the accepted payload lives in `accepted`
        // and the fallback slice lives in `doc.source_text`, both of which
        // outlive this splice. Only the two transforming arms below own a
        // string, so a document whose blocks all take the common arm is
        // regenerated without a second copy of itself (R0002-0069).
        let (payload, translated): (&str, bool) = match accepted.get(&block.block_id) {
            Some(p) => (p.as_str(), true),
            None => (&doc.source_text[bstart..bend], false),
        };

        // Fallback payloads are the block's raw source bytes; splice them
        // verbatim so fallback output stays byte-identical to the source —
        // the "by construction" guarantee FallbackAll / stage 3 rely on to
        // skip the verifying reparse (DCR-0002, DCR-0004). Only translated
        // payloads need fence re-wrapping.
        //
        // ti 490d97 wave 2 (spec §5/§6): the two transforming arms are keyed
        // on SPELLING, not on kind. That is what makes "(CodeBlock × Html)
        // never reaches the fence synthesizer" a compile shape rather than a
        // convention — DCR-0031's `regenerate_code_block` is Markdown-only,
        // and an HTML document's `<pre>` is a `CodeBlock` that must never be
        // re-fenced. It is also what puts the splice policy where it belongs:
        // the spelling decides it, never the kind.
        let to_write: Cow<'_, str> = match block.spelling {
            Spelling::Markdown => match &block.kind {
                BlockKind::CodeBlock { info, .. } if translated => {
                    Cow::Owned(regenerate_code_block(payload, info.as_deref()))
                }
                _ => Cow::Borrowed(payload),
            },
            Spelling::Html { block_type } if translated => {
                let source_bytes = &doc.source_text[bstart..bend];
                // Validation layer 3 already proved this splice succeeds;
                // if it still fails, splice the source bytes — out.md stays
                // honest, never corrupt.
                match serde_json::from_str::<Vec<String>>(payload)
                    .ok()
                    .and_then(|segs| {
                        transync_html::splice(
                            source_bytes,
                            &segs,
                            Spelling::blank_line_policy_for(block_type),
                        )
                        .ok()
                    }) {
                    Some(spliced) => Cow::Owned(spliced),
                    None => Cow::Borrowed(source_bytes),
                }
            }
            Spelling::Html { .. } => Cow::Borrowed(payload),
        };

        output.push_str(&to_write);
        let target_end = output.len();

        offsets.0.insert(
            block.block_id.clone(),
            ByteRange {
                start: target_start,
                end: target_end,
            },
        );

        last_end = bend;
    }

    if last_end < doc.source_text.len() {
        output.push_str(&doc.source_text[last_end..]);
    }

    (output, offsets)
}

/// [`regenerate`], plus the `accepted` keys that named no block in `doc`.
///
/// R0003-0057: `regenerate` reads the map from the *document's* side — it
/// walks blocks and asks `accepted` for each one — so a key that matches no
/// block is never read and never mentioned. For the pipeline that is
/// unreachable (it builds `accepted` from the document's own units), but a
/// programmatic caller who mistypes a `BlockId`, or holds a map built
/// against a different parse of the same file, gets a regenerated document
/// with the edit silently missing. The `transync-wasm` engine already had to
/// write its own unknown-id check to avoid exactly that, which is the
/// evidence this seam owed callers a signal.
///
/// The signal is a companion rather than a new return type on `regenerate`:
/// this module is deliberately validation-agnostic (see the module docs), so
/// unknown ids are *reported*, never rejected — the returned Markdown is
/// byte-identical to what [`regenerate`] produces for the same inputs. The
/// returned ids are sorted, so a caller can print them deterministically.
///
/// TRACE: SCN-01
/// TRACE: SCN-14
pub fn regenerate_checked(
    doc: &Document,
    accepted: &HashMap<BlockId, String>,
) -> (String, BlockOffsets, Vec<BlockId>) {
    let (md, offsets) = regenerate(doc, accepted);
    // The same walk `regenerate` splices against, so "known" cannot drift
    // from "consulted" if that walk ever narrows.
    let known: std::collections::HashSet<&BlockId> =
        top_level_blocks(&doc.blocks).map(|b| &b.block_id).collect();
    let mut unknown: Vec<BlockId> = accepted
        .keys()
        .filter(|id| !known.contains(id))
        .cloned()
        .collect();
    unknown.sort_by(|a, b| a.0.cmp(&b.0));
    (md, offsets, unknown)
}

/// Top-level blocks, non-overlapping and in source order.
///
/// The parser is leaf-block — it never recurses, so EVERY block is
/// top-level and this is now the identity walk. It survives as a named
/// concept because `transync-core`'s reparse-fallback policy and the
/// regenerator must agree on which ordering they splice against, and a
/// future nested-anchor scheme would narrow it here rather than at four
/// separate call sites.
///
/// Public because `transync-core`'s reparse-fallback policy needs the same
/// top-level ordering the regenerator splices against.
pub fn top_level_blocks(blocks: &[Block]) -> impl Iterator<Item = &Block> {
    blocks.iter()
}

/// The line-level pieces of one GFM table: its header row, its delimiter
/// row, and its body rows, in source order.
///
/// Lines carry no trailing `\n` — [`TableRows::window`] and
/// [`regenerate_table`] put the separators back — but they DO carry a
/// trailing `\r` when the source is CRLF, so a window assembled from them
/// keeps the source's line endings byte-for-byte.
///
/// TRACE: SCN-03
/// TRACE: DCR-0026
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableRows {
    /// The header row, verbatim.
    pub header: String,
    /// The delimiter row (`|---|:--:|`), verbatim — it carries the
    /// per-column alignment, so every window must repeat it.
    pub delimiter: String,
    /// The body rows, in source order. May be empty: a header-plus-delimiter
    /// table is a valid GFM table with nothing to split.
    pub body: Vec<String>,
}

impl TableRows {
    /// Assemble one row window: this table's header and delimiter followed by
    /// `body_rows`, which the caller takes from [`TableRows::body`].
    ///
    /// The result is a complete, valid GFM table — never a headerless
    /// fragment and never an isolated cell (architectural invariant 3). It is
    /// shaped like the payload `outcome::block_payload` produces for a whole
    /// table: no trailing newline, because a block's source range stops at the
    /// last row.
    pub fn window(&self, body_rows: &[String]) -> String {
        let mut out = String::with_capacity(
            self.header.len()
                + self.delimiter.len()
                + body_rows.iter().map(|r| r.len() + 1).sum::<usize>()
                + 2,
        );
        out.push_str(&self.header);
        out.push('\n');
        out.push_str(&self.delimiter);
        for row in body_rows {
            out.push('\n');
            out.push_str(row);
        }
        out
    }
}

/// Slice a GFM table payload into its header row, delimiter row, and body
/// rows — the source side of the DCR-0026 row-window split.
///
/// Returns `None` unless `source` parses, under the pipeline's own comrak
/// options, as exactly one table and nothing else. The verdict is comrak's,
/// not a line-shape guess: the payload must produce a single top-level
/// `Table` node, and the number of lines the slicing finds must equal the
/// number of rows comrak counted plus the header and delimiter. That second
/// check is what makes the function safe on payloads whose line endings this
/// slicing does not model (a lone-`\r` document splits into fewer lines than
/// comrak saw, and is refused rather than mis-sliced).
///
/// Pure over strings + comrak, like every other function in this module: it
/// knows nothing about units, budgets, or windows, and the caller decides how
/// many rows a window holds.
///
/// TRACE: SCN-03
/// TRACE: DCR-0026
pub fn split_table_rows(source: &str) -> Option<TableRows> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::parser::comrak_options();
    let root = comrak::parse_document(&arena, source, &opts);

    // Exactly one top-level node, and it is a table. A payload that also
    // holds a paragraph (or a second table) is not one table's worth of
    // rows, so there is nothing here to window.
    let mut children = root.children();
    let table = children.next()?;
    if children.next().is_some() {
        return None;
    }
    let body_row_count = {
        let value = &table.data.borrow().value;
        if !matches!(value, NodeValue::Table(_)) {
            return None;
        }
        table
            .children()
            .filter(|row| {
                matches!(row.data.borrow().value, NodeValue::TableRow(is_header) if !is_header)
            })
            .count()
    };

    // With a single table as the only block, every non-blank line of the
    // payload belongs to it — a blank line would have ended the table and
    // produced a second top-level node. So the table's lines are exactly the
    // span between the first and last non-blank line.
    let lines: Vec<&str> = source.split('\n').collect();
    let first = lines.iter().position(|l| !l.trim().is_empty())?;
    let last = lines.iter().rposition(|l| !l.trim().is_empty())?;
    let span = &lines[first..=last];
    // Header + delimiter + every body row comrak counted. Any other count
    // means this slicing does not describe what comrak parsed.
    if span.len() != body_row_count + 2 {
        return None;
    }

    Some(TableRows {
        header: span[0].to_string(),
        delimiter: span[1].to_string(),
        body: span[2..].iter().map(|l| (*l).to_string()).collect(),
    })
}

/// Reassemble one GFM table from the translated row windows
/// [`split_table_rows`] made possible (DCR-0026 §4).
///
/// Window 0 is taken verbatim — its translated header and delimiter are the
/// table's — and every later window contributes only the lines *after* its
/// delimiter row. Later windows' headers were context for the model
/// (architectural invariant 3's header-carrying shape), so a divergent header
/// translation across windows cannot reach the output.
///
/// A one-element call returns that element unchanged, which is exactly what
/// the whole-block path wants and what this function did as STUB-017's
/// identity placeholder.
///
/// Purely textual, and safe to be so: every input has already passed fragment
/// reparse as a single table, and the caller re-inspects the merged result
/// against the source table's column count, alignment, and row total before
/// it is accepted (DCR-0026 §3).
///
/// TRACE: SCN-02
/// TRACE: SCN-03
/// TRACE: DCR-0026
pub fn regenerate_table(windows: &[&str]) -> String {
    let mut out = String::new();
    for (i, window) in windows.iter().enumerate() {
        let contribution = if i == 0 {
            *window
        } else {
            body_rows_of(window)
        };
        if contribution.is_empty() {
            continue;
        }
        if !out.is_empty() && !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(contribution);
    }
    out
}

/// Everything after a window's delimiter row — i.e. past its second newline.
/// A window with fewer than two line breaks carries no body rows at all and
/// contributes nothing.
fn body_rows_of(window: &str) -> &str {
    let mut breaks = window.match_indices('\n');
    match (breaks.next(), breaks.next()) {
        (Some(_), Some((delimiter_end, _))) => &window[delimiter_end + 1..],
        _ => "",
    }
}

/// Re-wrap a fenced code block: extract the body from `payload` (which may
/// include the original fence and info string), then call
/// [`regenerate_code_fence`] to wrap with a safe fence.
///
/// An INDENTED source block that is accepted comes back through here too and
/// is therefore re-emitted **fenced** — a bare fence of a safe length, with no
/// info string, spliced at column 0 (the block's `source_range` covers its
/// indent, so no indent byte survives as inter-block text ahead of the
/// fence). Only a block that FELL BACK keeps its indented spelling, because
/// the fallback arm splices the source bytes verbatim and never reaches this
/// function. `transync-core`'s `unit::payload` calls it a second time, on the
/// way OUT, to synthesize the wire payload for an indented block.
///
/// TRACE: SCN-04
pub fn regenerate_code_block(payload: &str, info_hint: Option<&str>) -> String {
    let (body, info) = extract_code_body(payload).unwrap_or_else(|| {
        // Could not parse; treat the whole payload as body and reuse the
        // info hint from the source IR.
        (payload.to_string(), info_hint.map(str::to_string))
    });
    regenerate_code_fence(&body, info.as_deref())
}

/// Pick a safe fence character and length, then reassemble a fenced
/// code block. CommonMark forbids backticks in a backtick fence's
/// info string (tilde fences allow them), so an info string containing
/// a backtick switches the fence to tildes. Fence length is
/// `max(3, longest_fence_char_run_in_body + 1)`. When `info` is `None`
/// the fence info string is omitted.
///
/// TRACE: SCN-04
pub fn regenerate_code_fence(body: &str, info: Option<&str>) -> String {
    let info_str = info.unwrap_or("");
    let fence_char = if info_str.contains('`') { '~' } else { '`' };
    let mut max_run: usize = 0;
    let mut current: usize = 0;
    for c in body.chars() {
        if c == fence_char {
            current += 1;
            if current > max_run {
                max_run = current;
            }
        } else {
            current = 0;
        }
    }
    let fence_len = (max_run + 1).max(3);
    let fence: String = String::from(fence_char).repeat(fence_len);
    let mut body_owned = body.to_string();
    if !body_owned.ends_with('\n') {
        body_owned.push('\n');
    }
    format!("{fence}{info_str}\n{body_owned}{fence}\n")
}

/// Try to parse `fragment` as a single code block and return
/// `(body, info)`. Returns `None` when the fragment does not parse as one.
///
/// TRACE: SCN-04
fn extract_code_body(fragment: &str) -> Option<(String, Option<String>)> {
    use comrak::nodes::NodeValue;
    let arena = comrak::Arena::new();
    let opts = crate::parser::comrak_options();
    let root = comrak::parse_document(&arena, fragment, &opts);
    for child in root.children() {
        if let NodeValue::CodeBlock(c) = &child.data.borrow().value {
            let info = if c.info.is_empty() {
                None
            } else {
                Some(c.info.clone())
            };
            // Comrak stores the body without the trailing newline that
            // our regenerator wants. Normalize separately.
            let mut body = c.literal.clone();
            if body.ends_with('\n') {
                body.pop();
            }
            return Some((body, info));
        }
    }
    None
}

// Spec §3.3: regen splices the translated segments into the original
// markup; fallback html blocks stay byte-verbatim.
#[cfg(test)]
mod html_regen_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::parser::parse;

    /// The accepted-payload map `regenerate` consumes. `None` = the unit fell
    /// back, which the real pipeline expresses as absence from the map.
    fn accepted_map(unit_id: &str, payload: Option<&str>) -> HashMap<BlockId, String> {
        payload
            .map(|p| HashMap::from([(BlockId(unit_id.to_string()), p.to_string())]))
            .unwrap_or_default()
    }

    #[test]
    fn translated_html_block_is_spliced_into_out_md() {
        let src = "<details><summary>Click</summary></details>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let accepted = accepted_map("html-0001", Some("[\"펼치기\"]"));
        let (md, offsets) = regenerate(&doc, &accepted);
        assert_eq!(md, "<details><summary>펼치기</summary></details>\n");
        assert!(offsets.0.contains_key(&BlockId("html-0001".to_string())));
    }

    #[test]
    fn preserved_html_block_round_trips_byte_identical() {
        let src = "<p>Caf&eacute;&nbsp;&copy;</p>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        // Preserved: the wire payload echoes the source segments — the
        // identity skip keeps entity forms byte-exact (spec §3.3).
        let segs = transync_html::extract("<p>Caf&eacute;&nbsp;&copy;</p>").expect("extracts");
        let payload = serde_json::to_string(&segs.texts).unwrap();
        let accepted = accepted_map("html-0001", Some(&payload));
        let (md, _) = regenerate(&doc, &accepted);
        assert_eq!(md, src, "byte-identical incl. entities");
    }

    #[test]
    fn fallback_html_block_splices_source_bytes_verbatim() {
        let src = "<div>original</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        // A fallback unit is absent from the accepted map.
        let accepted = accepted_map("html-0001", None);
        let (md, _) = regenerate(&doc, &accepted);
        assert_eq!(md, src);
    }
}

// R0003-0057 / R0003-0059: what `regenerate` does with input it did not
// build itself — a map keyed by ids the document never had, and a `Document`
// a caller assembled by hand.
#[cfg(test)]
mod caller_input_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::parser::{AstPath, parse};

    #[test]
    fn unknown_accepted_ids_are_reported_without_changing_the_output() {
        let src = "# Title\n\npara\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let real = doc.blocks[0].block_id.clone();
        let accepted = HashMap::from([
            (real.clone(), "# 제목".to_string()),
            // Two ids this document does not have, deliberately inserted
            // out of sorted order.
            (BlockId("p-9999".to_string()), "dropped".to_string()),
            (BlockId("h1-9998".to_string()), "also dropped".to_string()),
        ]);
        assert!(
            !accepted.contains_key(&doc.blocks[1].block_id),
            "the fixture ids must be absent from the document"
        );

        let (plain, _) = regenerate(&doc, &accepted);
        let (checked, _, unknown) = regenerate_checked(&doc, &accepted);
        assert_eq!(
            checked, plain,
            "the companion reports; it never rejects or rewrites"
        );
        assert_eq!(
            unknown,
            vec![
                BlockId("h1-9998".to_string()),
                BlockId("p-9999".to_string())
            ],
            "every unknown id, sorted"
        );
        assert!(
            checked.contains("# 제목"),
            "the id that DID match is still applied: {checked}"
        );

        // A map the pipeline could have built reports nothing.
        let honest = HashMap::from([(real, "# 제목".to_string())]);
        assert!(regenerate_checked(&doc, &honest).2.is_empty());
    }

    #[test]
    fn a_hand_built_range_inside_a_char_does_not_panic() {
        // R0003-0059: `Document` and `Block` are `pub` with `pub` fields, so
        // a range that splits a multi-byte char is constructible outside
        // this crate — and `&source_text[start..end]` panics on one. Both
        // ends here land INSIDE the first Hangul syllable ("한" is 3 bytes).
        let doc = Document {
            source_text: "한글 문서\n".to_string(),
            blocks: vec![Block {
                block_id: BlockId("p-0001".to_string()),
                kind: BlockKind::Paragraph,
                spelling: crate::id::Spelling::Markdown,
                source_range: ByteRange { start: 1, end: 2 },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: AstPath::default(),
            }],
            ..Document::default()
        };
        // Fallback (no payload) slices the source; the translated arm slices
        // it too, for the html splice's source bytes.
        let (md, offsets) = regenerate(&doc, &HashMap::new());
        assert_eq!(
            md, "한글 문서\n",
            "the whole source still reaches the output"
        );
        assert!(offsets.0.contains_key(&BlockId("p-0001".to_string())));

        // The end-before-start direction, and a range past the end.
        for range in [
            ByteRange { start: 4, end: 1 },
            ByteRange { start: 2, end: 99 },
            ByteRange {
                start: usize::MAX,
                end: usize::MAX,
            },
        ] {
            let mut d = doc.clone();
            d.blocks[0].source_range = range;
            let (out, _) = regenerate(&d, &HashMap::new());
            assert!(
                out.is_char_boundary(0) && out.chars().count() > 0,
                "regenerating {range:?} produced {out:?}"
            );
        }
    }
}

// DCR-0026 SL-100: the source-side slice and the target-side reassembly, as
// a closed pair. The property that matters is that they are inverses under
// the identity translation — split a table any way the budget could, hand the
// pieces back unchanged, and the original table comes out byte-for-byte.
// Everything the splitter above this layer decides (how many rows a window
// holds) is a parameter of that property, so every partition is exercised
// rather than one chosen one.
#[cfg(test)]
mod table_row_window_tests {
    use super::*;

    /// Every way to cut `n` body rows into contiguous windows of a fixed
    /// size, for every window size from 1 (one row each) to `n` (one window).
    fn windowings(rows: &TableRows) -> Vec<Vec<String>> {
        let n = rows.body.len().max(1);
        (1..=n)
            .map(|size| {
                rows.body
                    .chunks(size)
                    .map(|chunk| rows.window(chunk))
                    .collect()
            })
            .collect()
    }

    fn merge(windows: &[String]) -> String {
        let refs: Vec<&str> = windows.iter().map(String::as_str).collect();
        regenerate_table(&refs)
    }

    /// The fixtures name the three payload shapes the DCR calls out — escaped
    /// pipes, multibyte cells, mixed alignment — plus the two line-ending and
    /// arity edges.
    fn fixtures() -> Vec<(&'static str, String)> {
        let mut out = vec![
            (
                "escaped pipes",
                "| expr | meaning |\n|---|---|\n| `a \\| b` | union |\n| `a \\|\\| b` | or |\n| plain | none |"
                    .to_string(),
            ),
            (
                "multibyte cells",
                "| 용어 | 뜻 |\n|---|---|\n| 에이전트 | agent |\n| 도구 사용 | tool use |\n| 정렬 | alignment |"
                    .to_string(),
            ),
            (
                "mixed alignment",
                "| l | c | r | n |\n|:---|:---:|---:|---|\n| 1 | 2 | 3 | 4 |\n| 5 | 6 | 7 | 8 |\n| 9 | 10 | 11 | 12 |"
                    .to_string(),
            ),
            (
                "single body row",
                "| a | b |\n|---|---|\n| 1 | 2 |".to_string(),
            ),
        ];
        // A long table is the case the splitter exists for: 40 rows is enough
        // for the chunked partitions to be genuinely different from each other.
        let mut long = String::from("| n | square |\n|---:|---:|");
        for i in 1..=40 {
            long.push_str(&format!("\n| {i} | {} |", i * i));
        }
        out.push(("forty rows", long));
        out
    }

    #[test]
    fn split_then_merge_reproduces_the_source_table_for_every_partition() {
        for (name, src) in fixtures() {
            let rows = split_table_rows(&src).unwrap_or_else(|| panic!("{name} must slice"));
            assert_eq!(
                rows.body.len(),
                src.lines().count() - 2,
                "{name}: every line past the delimiter is a body row"
            );
            for windows in windowings(&rows) {
                assert_eq!(
                    merge(&windows),
                    src,
                    "{name}: {} window(s) must reassemble byte-for-byte",
                    windows.len()
                );
            }
        }
    }

    /// The one-window call is the identity — which is what this function did
    /// as STUB-017's placeholder, so the whole-block path is unmoved by the
    /// real body landing.
    #[test]
    fn a_single_window_is_returned_verbatim() {
        for (_, src) in fixtures() {
            assert_eq!(regenerate_table(&[src.as_str()]), src);
        }
        // Including payloads that are not tables at all: the function does not
        // parse its input, it splices it.
        assert_eq!(regenerate_table(&["not a table"]), "not a table");
        assert_eq!(regenerate_table(&[]), "");
    }

    /// Window 0's header is canonical; later windows' headers were context and
    /// must not reach the output, however they were translated.
    #[test]
    fn later_windows_contribute_only_the_rows_below_their_delimiter() {
        let merged = regenerate_table(&[
            "| head | canonical |\n|---|---|\n| r1 | a |",
            "| DIVERGENT | HEADER |\n|:---:|---|\n| r2 | b |\n| r3 | c |",
            "| ANOTHER | ONE |\n|---|---|\n| r4 | d |",
        ]);
        assert_eq!(
            merged,
            "| head | canonical |\n|---|---|\n| r1 | a |\n| r2 | b |\n| r3 | c |\n| r4 | d |",
            "one header, one delimiter, every body row in window order"
        );
        assert!(!merged.contains("DIVERGENT") && !merged.contains("ANOTHER"));
    }

    /// A window the model returned without a trailing newline still joins onto
    /// the next one — the seam is repaired, never run together.
    #[test]
    fn windows_join_on_a_line_break_whatever_the_model_returned() {
        for w0 in [
            "| a | b |\n|---|---|\n| 1 | 2 |",
            "| a | b |\n|---|---|\n| 1 | 2 |\n",
        ] {
            assert_eq!(
                regenerate_table(&[w0, "| a | b |\n|---|---|\n| 3 | 4 |"]),
                "| a | b |\n|---|---|\n| 1 | 2 |\n| 3 | 4 |",
                "trailing newline on window 0: {}",
                w0.ends_with('\n')
            );
        }
        // A window with no body rows of its own contributes nothing rather
        // than a stray blank line.
        assert_eq!(
            regenerate_table(&["| a |\n|---|\n| 1 |", "| a |\n|---|"]),
            "| a |\n|---|\n| 1 |"
        );
    }

    /// CRLF survives because the rows keep their `\r` and only the `\n` is
    /// re-supplied.
    #[test]
    fn crlf_tables_round_trip_byte_for_byte() {
        let src = "| a | b |\r\n|---|---|\r\n| 1 | 2 |\r\n| 3 | 4 |\r\n| 5 | 6 |";
        let rows = split_table_rows(src).expect("slices");
        assert!(rows.header.ends_with('\r'), "the CR rides with the line");
        assert_eq!(rows.body.len(), 3);
        for windows in windowings(&rows) {
            assert_eq!(merge(&windows), src);
        }
    }

    /// Blank lines around the payload are not part of the table, so the
    /// reassembly drops them — and says so here rather than in a surprise.
    #[test]
    fn surrounding_blank_lines_are_not_rows() {
        let rows = split_table_rows("\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n").expect("slices");
        assert_eq!(rows.header, "| a | b |");
        assert_eq!(rows.body, vec!["| 1 | 2 |".to_string()]);
        assert_eq!(rows.window(&rows.body), "| a | b |\n|---|---|\n| 1 | 2 |");
    }

    /// A table with no body rows is still a table; there is simply nothing to
    /// window.
    #[test]
    fn a_header_only_table_slices_to_no_body_rows() {
        let rows = split_table_rows("| a | b |\n|---|---|").expect("slices");
        assert!(rows.body.is_empty());
        assert_eq!(rows.window(&[]), "| a | b |\n|---|---|");
    }

    /// The refusals. Each of these would mis-slice under a line-shape guess,
    /// which is why the verdict is comrak's.
    #[test]
    fn a_payload_that_is_not_exactly_one_table_is_refused() {
        for (name, src) in [
            ("a paragraph", "just a sentence with | pipes | in it"),
            ("empty", ""),
            ("blank lines only", "\n  \n"),
            (
                "a fenced code block that looks like a table",
                "```\n| a | b |\n|---|---|\n| 1 | 2 |\n```",
            ),
            ("two tables", "| a |\n|---|\n| 1 |\n\n| b |\n|---|\n| 2 |"),
            (
                "a table plus a trailing paragraph",
                "| a |\n|---|\n| 1 |\n\ntail paragraph",
            ),
            ("a heading", "# not a table"),
            (
                // Lone-CR line endings: comrak counts three rows, this
                // slicing sees one line. Refused, not mis-sliced.
                "lone-CR line endings",
                "| a | b |\r|---|---|\r| 1 | 2 |",
            ),
        ] {
            assert_eq!(split_table_rows(src), None, "{name} must not slice");
        }
    }
}

// ti `457e51`: an indented code block's splice.
//
// `regenerate` needs no new logic for this — the accepted arm already
// re-emits a code block fenced, with a safe fence length. What it needs is a
// `source_range` that starts at column 0, so the four columns of block
// structure are inside the block's span instead of being copied out as
// inter-block gap bytes ahead of the fence.
#[cfg(test)]
mod indented_code_regen_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::parser::parse;

    const SRC: &str = "Intro.\n\n    let x = 1;\n\nOutro.\n";

    fn top_level_labels(md: &str) -> Vec<&'static str> {
        use comrak::nodes::NodeValue;
        let arena = comrak::Arena::new();
        let opts = crate::parser::comrak_options();
        let root = comrak::parse_document(&arena, md, &opts);
        root.children()
            .map(|child| match &child.data.borrow().value {
                NodeValue::Paragraph => "paragraph",
                NodeValue::CodeBlock(_) => "code-block",
                NodeValue::Heading(_) => "heading",
                NodeValue::List(_) => "list",
                NodeValue::BlockQuote => "blockquote",
                NodeValue::HtmlBlock(_) => "html",
                NodeValue::ThematicBreak => "thematic-break",
                _ => "other",
            })
            .collect()
    }

    #[test]
    fn an_accepted_indented_block_is_re_emitted_fenced_at_column_zero() {
        let mut doc = parse(SRC).expect("parses");
        assign_block_ids(&mut doc);
        let accepted = HashMap::from([(
            BlockId("c-0002".to_string()),
            "```\nlet x = 1;\n```".to_string(),
        )]);

        let (md, offsets) = regenerate(&doc, &accepted);

        assert_eq!(
            md, "Intro.\n\n```\nlet x = 1;\n```\n\nOutro.\n",
            "no residual indent may survive ahead of the opening fence",
        );
        assert!(
            !md.contains("    ```"),
            "an indented fence marker re-opens an indented code block and \
             leaves the real fence unclosed: {md:?}",
        );
        assert_eq!(
            top_level_labels(&md),
            top_level_labels(SRC),
            "the regenerated document keeps the source's top-level shape",
        );
        assert!(offsets.0.contains_key(&BlockId("c-0002".to_string())));
    }

    /// The consequence the snap's whitespace walk exists to prevent: a
    /// document-leading BOM must survive an ACCEPTED indented block.
    ///
    /// Regeneration replaces exactly the block's `source_range` with the
    /// accepted payload, so any byte the range swallowed is gone from the
    /// output. Comrak counts a leading BOM's three bytes in the column it
    /// reports for the block, so a snap straight to column 1 would put the
    /// BOM inside that range — and this run would silently drop it, while the
    /// fallback path (which splices the same range's source bytes back) would
    /// keep it. The parser-side pin is
    /// `parser::indented_code_tests::the_snap_stops_at_a_document_leading_bom`;
    /// this is the end of the same wire.
    #[test]
    fn a_leading_bom_survives_an_accepted_indented_block() {
        const BOM: &str = "\u{feff}";
        let src = format!("{BOM}    let x = 1;\n\nOutro.\n");

        let mut doc = parse(&src).expect("parses");
        assign_block_ids(&mut doc);
        let code_id = doc
            .blocks
            .iter()
            .find(|b| matches!(b.kind, crate::id::BlockKind::CodeBlock { .. }))
            .expect("the fixture has a code block")
            .block_id
            .clone();
        let accepted = HashMap::from([(code_id, "```\nlet x = 1;\n```".to_string())]);

        let (md, _) = regenerate(&doc, &accepted);
        assert_eq!(
            md,
            format!("{BOM}```\nlet x = 1;\n```\n\nOutro.\n"),
            "the BOM is not the block's, so translating the block must not \
             consume it",
        );

        // And the fallback direction still round-trips byte for byte.
        let (md, _) = regenerate(&doc, &HashMap::new());
        assert_eq!(md, src);
    }

    #[test]
    fn an_empty_accepted_map_round_trips_the_indented_source_byte_for_byte() {
        // The fallback pin for the widened range (invariant 6): a block
        // absent from the map splices its own source bytes, so the indented
        // spelling must come back exactly — no dedent, no re-fencing, and no
        // indent bytes copied twice (once as gap, once as block).
        let mut doc = parse(SRC).expect("parses");
        assign_block_ids(&mut doc);
        let (md, _) = regenerate(&doc, &HashMap::new());
        assert_eq!(md, SRC);

        // Same, for the spellings whose indent is not four literal spaces.
        for src in [
            "Intro.\n\n\tlet x = 1;\n\nOutro.\n",
            "Intro.\n\n        let x = 1;\n\nOutro.\n",
            "Intro.\n\n    a\n\n    b\n\nOutro.\n",
        ] {
            let mut doc = parse(src).expect("parses");
            assign_block_ids(&mut doc);
            let (md, _) = regenerate(&doc, &HashMap::new());
            assert_eq!(md, src, "fallback round-trip for {src:?}");
        }
    }
}
