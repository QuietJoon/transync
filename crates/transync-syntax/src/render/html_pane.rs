//! HTML-source pane derivation (ti 490d97, spec 2026-08-20 §8).
//!
//! Panes are **synthesized `<main>` fragments, not annotated whole
//! documents** — four reasons, all tracing to locked decisions: D6 (panes
//! are a sync surface; fidelity is `transync serve` over `out.html`),
//! contracts §4a (single `<main>`, every block a direct child, in source
//! order — so anchor survival, one-element-per-anchored-row, `offsetTop`
//! math and the R0003-0076 pane-position probe carry over with zero
//! amendment), the by-construction death of script/style/head in the
//! mount, and OI-0035's shrunken surface (gap markup is exactly where
//! impostor anchors would ride).
//!
//! One worker serves both panes — the source pane over
//! `Document::source_text`, the target pane over the regenerated HTML —
//! per alignment row with `sync_role != "non-sync"`, in source order:
//!
//! 1. slice the block's bytes by the row's range (`PaneCtx::new`'s three
//!    refusals — duplicate row, uncovered block, unusable range — carry
//!    over VERBATIM: this module calls the same constructor);
//! 2. **strip** the reserved attribute namespace
//!    ([`transync_html::strip_reserved_sync_attrs`]) — strip-then-inject
//!    is what makes "ours are the only sync attributes" a construction
//!    rather than a scan; reversing the order strips the anchors this
//!    module just injected and silently reopens OI-0035;
//! 3. **inject** the four canonical attributes (`attrs::write_attrs`, so
//!    the two formats' spelling and order cannot fork) into the block's
//!    OWN outermost element's open tag — for the semantic kinds an HTML
//!    intake mints (`h1`–`h6`, paragraph, table, list-item, code-block,
//!    blockquote, image, figcaption), when one covering depth-0 extent
//!    exists. No wrapper `<div>` for tables or code here: the Markdown
//!    path's wrapper exists because comrak output could not carry
//!    attributes (ADR-0007 / DCR-0001); an HTML block's own element can.
//!    Deliberate, documented divergence — do not "fix" it back;
//! 4. wrap the rest in a transparent `<div{attrs}>…</div>` — the shipped
//!    DCR-0016 precedent. Three populations: an element-less block (a
//!    rule-T text run, a block missing its open tag); an img-run `Image`
//!    block (a void carries no extent, so even a lone `<img>` is
//!    element-less to the instrument); and **every `BlockKind::Html`
//!    block** — the ti `d4bce2` ruling.
//!
//!    That last key replaced an earlier "is the element named in §4's
//!    tables" test, and the reason is worth keeping: the shipped shell
//!    mounts panes through DOMPurify's untouched fail-closed config, which
//!    removes an unknown element **and every attribute riding it**, so an
//!    anchor injected into `<x-note>` dies in the mount and the pane grows
//!    a hole. Asking "does §4 know this element" closed that for custom
//!    elements but left `iframe` and `noscript` — named DEFAULT-STOP
//!    entries, so they self-injected, and DOMPurify removes both element
//!    and contents. Patching that would have meant naming which
//!    DEFAULT-STOP elements the sanitizer keeps, which is a **second
//!    opinion about what DOMPurify accepts** — the same class of sin as a
//!    second Markdown parser. Keying on the KIND refuses it too and needs
//!    no list at all. It also makes the two pane derivations agree:
//!    `render.rs`'s Markdown arm already wraps every `BlockKind::Html`
//!    block unconditionally. The cost, stated rather than hidden: one
//!    extra `<div>` around a `summary`/`dt`/`dd`/`address`/`dialog`/
//!    `legend` block that could have carried its own attributes, and an
//!    `iframe`/`noscript` block mounting as an anchored but empty `<div>`
//!    instead of leaving a hole in the anchor list.
//! 5. **balance** ([`transync_html::balance_fragment`]) so a malformed
//!    block cannot swallow the next block's anchor after the DOMPurify
//!    innerHTML mount;
//! 6. group consecutive `li` blocks under ONE shared `<ul>`/`<ol>` via
//!    [`crate::walk::normalize_top_level`]'s collapse — under D9 this is
//!    mandatory, not cosmetic. D9 makes the list container gap: `<ul>`/
//!    `<ol>` carry no text and no anchor of their own, only the items do.
//!    So without the group the items land as direct `<main>` children —
//!    invalid HTML with no list semantics, rendered unlike the Markdown
//!    pane, which has always grouped them via `render_list_group`. Two
//!    panes that disagree about list structure cannot be compared by eye,
//!    which is what panes are for. (NOT a sanitizer behaviour: spec §8
//!    step 6 says DOMPurify relocates a bare `<li>`, and that is
//!    measured-false — 3.2.6 passes it through unchanged, in place.
//!    Relocation is a table-family parser behaviour, `<tr>` outside
//!    `<table>`.)
//!
//! Gap bytes, `<head>`, doctype, comments, `<script>`/`<style>` are never
//! emitted — only row ranges are read, so they are unreachable rather than
//! filtered. A `fallback_source` block renders LIVE (its bytes are source
//! bytes, already valid HTML) with `data-fallback="fallback_source"`;
//! ONLY an extraction-failure block (per the run's `HtmlOutcome` map)
//! takes the escaped `<pre data-skipped="html-block">` placeholder,
//! mirroring DCR-0016's failure presentation. Non-sync rows — the
//! `<title>` and `<hr>` — reach no pane at all: §8's own per-row rule
//! (`sync_role != "non-sync"`), with D5/D6 as its grounds and §13 item 6
//! recording the title's half.
//!
//! The published `out.html` is `regen::regenerate` output — full page,
//! head, scripts, doctype, ANCHOR-FREE. Anchors never touch the regen
//! path; this module reads `&str`s and returns a new `String`, and the
//! same `block_id` flows IR → LLM → regenerated HTML → alignment row →
//! this injection (which reads the ROW, never the document's own claims)
//! → DOM anchor → engine. The document never votes on its own anchors.
//!
//! TRACE: ADR-0025
//! TRACE: contracts.md §4, §4a

use super::{
    FRAGMENT_CLOSE, FRAGMENT_OPEN, Pane, PaneCtx, RenderError, attrs, block_text, html_escape,
};
use crate::align::{AlignmentMap, SyncRole};
use crate::id::{BlockId, BlockKind, SourceFormat};
use crate::intake::markdown::{self, Block, Document};
use crate::outcome::HtmlOutcome;
use crate::walk;
use std::collections::HashMap;
use std::fmt::Write;

/// Render the source pane for an HTML-intake document. See the module doc;
/// the map refusals and range discipline are `render_source`'s exactly.
pub fn render_source_html(
    doc: &Document,
    alignment: &AlignmentMap,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    render_html_fragment(doc, alignment, Pane::Source, outcomes)
}

/// Render the target pane from the regenerated translated HTML. Same block
/// set, same refusals, plus the `markdown::intake` guard on `translated_html`
/// itself — the same guard `render_target` applies to `translated_md`
/// (R0002-0059; spec §8 step 1).
pub fn render_target_html(
    doc: &Document,
    translated_html: &str,
    alignment: &AlignmentMap,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    render_html_fragment(doc, alignment, Pane::Target(translated_html), outcomes)
}

fn render_html_fragment(
    doc: &Document,
    alignment: &AlignmentMap,
    pane: Pane<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Result<String, RenderError> {
    // Wave 2's handed-forward render/pane guard, discharged as a PAIR of
    // assertions — this Html half, plus the Markdown mirror at the top of
    // render_fragment: the LIVE routing guarantee is run_pipeline's
    // exhaustive format match; a fallible refusal here would be an
    // unreachable arm claiming to be a gate. An assertion is not a dispatch.
    debug_assert_eq!(
        doc.format,
        SourceFormat::Html,
        "html_pane is the HTML-intake pane derivation; a Markdown document \
         takes render_source/render_target"
    );
    let ctx = PaneCtx::new(doc, alignment, pane)?;
    // Deviation 8: the guard is consumed for its REFUSAL; the ranges index
    // the caller's own bytes (ctx.md), exactly render_fragment's documented
    // posture — there is no parse here to consume the normalized Cow.
    if let Pane::Target(t) = pane {
        let _guarded = markdown::intake(t)?;
    }

    let mut out = String::from(FRAGMENT_OPEN);
    for entry in walk::normalize_top_level(doc) {
        if entry.label == "list" {
            emit_list_group(&mut out, &entry.sources, &ctx, outcomes);
        } else if let Some((block, row)) = entry.sources.first().and_then(|id| ctx.pair(id)) {
            emit_block(&mut out, block, row, &ctx, outcomes);
        }
    }
    out.push_str(FRAGMENT_CLOSE);
    Ok(out)
}

/// One shared `<ul>`/`<ol>` for the collapsed entry's items (§8 step 6, D9).
/// The tag comes from the first row's own kind — there is no comrak node
/// here and never will be. The group carries no attributes at all: the
/// source list's own tags are GAP under D9 (its `start`/classes included —
/// §13 item 4; the pane is a sync surface, not a fidelity preview).
fn emit_list_group(
    out: &mut String,
    sources: &[BlockId],
    ctx: &PaneCtx<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) {
    // Membership predicate, sanctioned by the charter's stated carve-out and
    // byte-identical to render_list_group's shipped Guard-2 line: a "list"
    // entry's sources are ListItem by normalize_top_level's label rule, so
    // the false answer is unreachable, and false degrades to <ul> — safe.
    let ordered = sources
        .first()
        .and_then(|id| ctx.by_id.get(id))
        .is_some_and(|b| matches!(b.kind, BlockKind::ListItem { ordered: true, .. }));
    let tag = if ordered { "ol" } else { "ul" };
    let _ = writeln!(out, "<{tag}>");
    for id in sources {
        if let Some((block, row)) = ctx.pair(id) {
            emit_block(out, block, row, ctx, outcomes);
        }
    }
    let _ = writeln!(out, "</{tag}>");
}

/// One row: skip non-sync, key the placeholder on the OUTCOME (Deviation 3),
/// otherwise strip → inject-or-wrap → balance (§8 steps 2–5).
fn emit_block(
    out: &mut String,
    block: &Block,
    row: &crate::align::AlignmentBlock,
    ctx: &PaneCtx<'_>,
    outcomes: &HashMap<BlockId, HtmlOutcome>,
) {
    match row.sync_role {
        // D5/D6: nothing in the pane to anchor — the row is the record.
        SyncRole::NonSync => return,
        SyncRole::Anchor | SyncRole::Container | SyncRole::ChildOnly => {}
    }
    let bytes = block_text(ctx.md, ctx.pane.range(row, block));
    let attrs_str = attrs::write_attrs(row);
    match outcomes.get(&block.block_id) {
        Some(HtmlOutcome::ExtractionFailed(_)) => {
            // The FAILURE presentation (DCR-0016's placeholder): extraction
            // failed, so live-mounting markup the rewriter could not read is
            // not on offer. This is the ONLY escaped arm — a fallback_source
            // block whose extraction succeeded renders live below.
            let _ = writeln!(
                out,
                "<pre{attrs_str} data-skipped=\"html-block\">{}</pre>",
                html_escape(bytes),
            );
            return;
        }
        Some(HtmlOutcome::Unit) | Some(HtmlOutcome::PreservedZeroSegment) | None => {}
    }
    // §8 step 2 THEN step 3: strip first, and scan the STRIPPED bytes for
    // the injection point, so our injected attributes cannot be stripped and
    // an author's reserved-namespace attributes cannot survive.
    let stripped = transync_html::strip_reserved_sync_attrs(bytes);
    // ti d4bce2: the self-injection boundary is the block's KIND, read off
    // the ROW — the artifact DCR-0044 made the authority, and the same value
    // render.rs's Markdown arm keys its own unconditional wrapper on. A
    // string compare, deliberately: `wire_str()` maps exactly one kind to
    // "html", so this is `BlockKind::Html` by another name, without a second
    // 16-arm match to keep in step with the enum.
    let self_injectable = row.block_kind != "html";
    let fragment = injected_fragment(stripped.trim(), &attrs_str, self_injectable);
    let _ = writeln!(out, "{fragment}");
}

/// §8 steps 3–5, under ti `d4bce2`'s kind-keyed boundary. The block
/// self-injects iff its kind is not `Html` AND exactly one depth-0 extent
/// opens at the trimmed start and reaches the trimmed end (its close span's
/// end, or — for a closeless extent: self-closed in foreign content, or
/// force-closed at EOF — the extent's own end per the landed
/// `ElementExtent` contract). Then the four attributes splice into that open
/// tag just before its `>` or `/>`; the open span here IS
/// `TagToken::Open.span` — `element_extents` is the same walk (§5), so the
/// spec's instrument is what is read.
///
/// Anything else takes the transparent wrapper (§8 step 4): bare text, a
/// multi-element run, an orphan close, a void element (which records no
/// extent at all, so a lone `<img>` lands here), and every `Html`-kind
/// block — the population the shell's DOMPurify mount can remove along with
/// any anchor riding it. Both arms balance (§8 step 5).
fn injected_fragment(trimmed: &str, attrs_str: &str, self_injectable: bool) -> String {
    if self_injectable {
        let extents = transync_html::element_extents(trimmed);
        let top: Vec<&transync_html::ElementExtent> =
            extents.iter().filter(|e| e.depth == 0).collect();
        if let [only] = top.as_slice() {
            let coverage_end = only
                .close
                .map_or(only.content_end.max(only.open.1), |c| c.1);
            if only.open.0 == 0 && coverage_end == trimmed.len() {
                let (_, open_end) = only.open;
                let insert_at = if trimmed[..open_end].ends_with("/>") {
                    open_end - 2
                } else {
                    open_end - 1
                };
                let injected = format!(
                    "{}{}{}",
                    &trimmed[..insert_at],
                    attrs_str,
                    &trimmed[insert_at..]
                );
                return transync_html::balance_fragment(&injected);
            }
        }
    }
    format!(
        "<div{attrs_str}>{}</div>",
        transync_html::balance_fragment(trimmed)
    )
}

// ti 490d97 wave 6 (spec §8): every trap in the derivation has a test that
// names the bug that would spring it. Statuses are pinned to Preserved by
// the helper so `data-fallback` is deterministic; tests that care about a
// status build their own map.
#[cfg(test)]
mod html_pane_tests {
    use super::*;
    use crate::align::{FallbackStatus, build_alignment_map};
    use crate::id::assign_block_ids;
    use crate::intake;
    use crate::outcome::html_outcomes;
    use crate::regen;

    struct Rig {
        doc: Document,
        map: AlignmentMap,
        translated: String,
        outcomes: HashMap<BlockId, HtmlOutcome>,
    }

    /// Parse with the HTML intake, regenerate the identity document, and
    /// build the map with every block Preserved — so `data-fallback` is
    /// deterministic in the structural tests. The fallback/extraction test
    /// below builds its own statuses instead.
    fn rig(src: &str) -> Rig {
        let mut doc = intake::html::parse(src);
        assign_block_ids(&mut doc);
        let (translated, offsets) = regen::regenerate(&doc, &HashMap::new());
        let outcomes = html_outcomes(&doc);
        let statuses: HashMap<BlockId, FallbackStatus> = doc
            .blocks
            .iter()
            .map(|b| (b.block_id.clone(), FallbackStatus::Preserved))
            .collect();
        let map = build_alignment_map(&doc, &statuses, &offsets, "auto", "ko", None, &outcomes);
        Rig {
            doc,
            map,
            translated,
            outcomes,
        }
    }

    /// §8 step 3 + Deviation 4: the anchor lands on the block's OWN element
    /// — h2, p, table, pre, blockquote — never on a wrapper div. The absent
    /// `<div data-sync-id="t-` is the pin against "fixing" the divergence
    /// back to the Markdown path's ADR-0007 wrapper.
    #[test]
    fn anchors_land_on_the_blocks_own_elements_with_no_wrapper_div() {
        let r = rig(
            "<h2>Head</h2>\n<p>Body</p>\n<table><tr><td>x</td></tr></table>\n\
             <pre><code>let a;</code></pre>\n<blockquote><p>q</p></blockquote>\n",
        );
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(pane.starts_with(FRAGMENT_OPEN), "{pane}");
        assert!(pane.ends_with(FRAGMENT_CLOSE), "{pane}");
        for open in [
            "<h2 data-sync-id=\"h2-",
            "<p data-sync-id=\"p-",
            "<table data-sync-id=\"t-",
            "<pre data-sync-id=\"c-",
            "<blockquote data-sync-id=\"q-",
        ] {
            assert!(
                pane.contains(open),
                "missing own-element anchor {open}:\n{pane}"
            );
        }
        assert!(
            !pane.contains("<div data-sync-id=\"t-") && !pane.contains("<div data-sync-id=\"c-"),
            "the Markdown path's wrapper div must NOT come back (Deviation 4):\n{pane}"
        );
    }

    /// §8 steps 2+3, the ORDER: strip runs first, injection second, and the
    /// injection scans the STRIPPED bytes. Inject-then-strip deletes our own
    /// anchors (count 0); no strip leaves two claimants for one id (count 2).
    /// Either wrong order is a red here — this is the OI-0035 interlock.
    #[test]
    fn strip_then_inject_makes_ours_the_only_sync_attributes() {
        let r = rig("<p data-sync-id=\"p-9999\" DATA-Order=\"7\" class=\"k\">text</p>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert_eq!(
            pane.matches("data-sync-id=").count(),
            1,
            "exactly one claimant — ours:\n{pane}"
        );
        assert!(!pane.contains("p-9999"), "the impostor id is gone:\n{pane}");
        assert!(
            !pane.contains("DATA-Order"),
            "the strip is case-insensitive:\n{pane}"
        );
        assert!(
            pane.contains("class=\"k\""),
            "non-namespace attributes survive:\n{pane}"
        );
        assert!(pane.contains(">text</p>"), "content survives:\n{pane}");
    }

    /// §8 step 6 / D9: consecutive items share ONE group; adjacent lists —
    /// different ast_path prefixes — never merge; the ordered flag picks the
    /// tag. No-grouping is a mount-breaking bug, not a style choice.
    #[test]
    fn consecutive_items_share_one_group_and_adjacent_lists_never_merge() {
        let r = rig("<ul><li>a</li><li>b</li></ul>\n<ol><li>c</li></ol>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert_eq!(pane.matches("<ul>").count(), 1, "one shared ul:\n{pane}");
        assert_eq!(
            pane.matches("<ol>").count(),
            1,
            "the ol stays its own group:\n{pane}"
        );
        assert_eq!(
            pane.matches("<li data-sync-id=\"li-").count(),
            3,
            "every item is its own anchor inside a group:\n{pane}"
        );
        let ul_end = pane.find("</ul>").expect("ul closes");
        let ol_start = pane.find("<ol>").expect("ol opens");
        assert!(ul_end < ol_start, "groups do not interleave:\n{pane}");
    }

    /// §8 step 4: a rule-T anonymous run has no outermost element, so it
    /// takes the transparent DCR-0016 wrapper; its element-bearing neighbor
    /// does not.
    #[test]
    fn an_anonymous_run_takes_the_transparent_div_wrapper() {
        let r = rig("<div>\nnaked run text\n<p>kept</p>\n</div>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"p-") && pane.contains(">naked run text</div>"),
            "the run rides a transparent wrapper:\n{pane}"
        );
        assert!(
            pane.contains("<p data-sync-id=\"p-"),
            "the real <p> keeps its own tag:\n{pane}"
        );
    }

    /// §8 step 3/4's boundary, as Deviation 4 sharpens it: a void element is
    /// never pushed by the one stack walk, so `element_extents("<img …>")`
    /// is EMPTY — a lone `<img>` is one element to the eye and zero extents
    /// to the §5 instrument, and the wrapper arm takes every img-run Image
    /// block, single or multi. The bug this pins out: a "helpful" special
    /// case that injects into the void's own open tag, forking the
    /// derivation from the instrument it must defer to.
    #[test]
    fn a_lone_void_img_takes_the_wrapper_not_its_own_open_tag() {
        let r = rig("<p>before</p>\n<img src=\"badge.png\" alt=\"a\">\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"img-"),
            "the single-img Image block rides the transparent wrapper:\n{pane}"
        );
        assert!(
            !pane.contains("<img data-sync-id="),
            "injection must not land inside the void's own tag — the \
             instrument records no extent to land in (Deviation 4):\n{pane}"
        );
        assert!(
            pane.contains("data-block-kind=\"image\""),
            "the kind still names it:\n{pane}"
        );
        assert!(
            pane.contains("<img src=\"badge.png\""),
            "the img itself survives:\n{pane}"
        );
    }

    /// §8 step 3/4's boundary, under the ti `d4bce2` re-keying: an
    /// **`Html`-kind** block takes the transparent wrapper, never
    /// self-injection — the SCN-16 fixture's `<x-note>`, any custom or
    /// future element, and equally `iframe`/`noscript`, which the earlier
    /// "named in §4's tables" key would have let self-inject.
    ///
    /// Measured ground (the vendored DOMPurify 3.2.6): sanitize over a
    /// self-injected `<x-note … data-sync-id="html-…">` removes the element
    /// AND the anchor, keeping only the text — the pane mounts with a hole
    /// in its anchor list and a warnMapDomDrift warning at every load. The
    /// wrapper `<div>` survives the same mount with its attributes intact.
    /// The bug this pins out: treating "IS one element" as sufficient and
    /// splicing the anchor into a tag the sanitizer will remove.
    #[test]
    fn an_html_kind_block_takes_the_wrapper_not_its_own_open_tag() {
        let r =
            rig("<p>before</p>\n<x-note tone=\"info\">A custom element stops, loudly.</x-note>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        assert!(
            pane.contains("<div data-sync-id=\"html-"),
            "the Html-kind block rides the transparent wrapper:\n{pane}"
        );
        assert!(
            !pane.contains("<x-note data-sync-id="),
            "injection must not land in a tag the sanitizer will remove:\n{pane}"
        );
        assert!(
            pane.contains("<x-note tone=\"info\">A custom element stops, loudly.</x-note>"),
            "the element renders LIVE inside the wrapper — transparent, \
             not an escape:\n{pane}"
        );
        assert!(
            pane.contains("data-block-kind=\"html\""),
            "the kind still names it:\n{pane}"
        );
    }

    /// §8 step 7 + D5/D6: gaps, head, doctype, comments, script/style and
    /// non-sync rows (title, hr) never reach a pane. The pass-through
    /// wrappers (header/section/body) are gap markup and are absent too —
    /// this is the whole-document-pane refusal made checkable.
    #[test]
    fn gaps_head_scripts_and_non_sync_rows_never_reach_the_pane() {
        let r = rig(
            "<!DOCTYPE html>\n<html><head><title>T</title>\n<style>p{}</style>\n\
             <script>1 < 2</script></head>\n<body><header>\n<h1>H</h1>\n</header>\n\
             <!-- note -->\n<p>body</p>\n<hr>\n</body></html>\n",
        );
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        for absent in [
            "<script",
            "<style",
            "DOCTYPE",
            "<title",
            "<head",
            "<header",
            "<body",
            "<!-- note -->",
            "<hr",
            "data-sync-id=\"title-",
        ] {
            assert!(
                !pane.contains(absent),
                "{absent} must not reach a pane:\n{pane}"
            );
        }
        assert!(pane.contains("<h1 data-sync-id=\"h1-"), "{pane}");
        assert!(pane.contains("<p data-sync-id=\"p-"), "{pane}");
    }

    /// §8 step 7's split, keyed on the run's HtmlOutcome map (Deviation 3):
    /// a fallback_source block renders LIVE — its bytes are source bytes,
    /// already valid HTML — and only an extraction failure takes the escaped
    /// placeholder. Copying the Markdown arm's escape-on-fallback here is
    /// the bug this test exists to catch.
    #[test]
    fn fallback_renders_live_and_only_extraction_failure_takes_the_placeholder() {
        let src = "<p>alpha</p>\n<x-note>beta</x-note>\n";
        let mut doc = intake::html::parse(src);
        assign_block_ids(&mut doc);
        let (_, offsets) = regen::regenerate(&doc, &HashMap::new());
        let p_id = doc
            .blocks
            .iter()
            .find(|b| b.block_id.0.starts_with("p-"))
            .expect("p block")
            .block_id
            .clone();
        let x_id = doc
            .blocks
            .iter()
            .find(|b| b.block_id.0.starts_with("html-"))
            .expect("x-note block")
            .block_id
            .clone();
        // The paragraph FELL BACK: unit attempted, exhausted — a real
        // statuses row says so.
        let mut statuses = HashMap::new();
        statuses.insert(p_id, FallbackStatus::FallbackSource);
        // The custom element's EXTRACTION failed (forced). It gets NO
        // statuses row: align derives fallback_source from the
        // ExtractionFailed outcome itself, so BOTH rows read fallback_source
        // and only the outcome map tells them apart — the premise, asserted
        // before the render.
        let mut outcomes = html_outcomes(&doc);
        outcomes.insert(
            x_id.clone(),
            HtmlOutcome::ExtractionFailed("boom".to_string()),
        );
        let map = build_alignment_map(&doc, &statuses, &offsets, "auto", "ko", None, &outcomes);
        let x_row = map
            .blocks
            .iter()
            .find(|r| r.source_block_id == x_id)
            .expect("x-note row");
        assert_eq!(
            x_row.fallback_status,
            FallbackStatus::FallbackSource,
            "premise: the ROW cannot distinguish unit-fallback from extraction \
             failure — that is why the derivation takes the outcome map",
        );
        let pane = render_source_html(&doc, &map, &outcomes).expect("renders");
        assert!(
            pane.contains("data-fallback=\"fallback_source\">alpha</p>"),
            "a fallen block renders LIVE with the honest attribute:\n{pane}"
        );
        assert!(
            pane.contains("data-skipped=\"html-block\">&lt;x-note&gt;beta"),
            "the extraction failure takes the escaped placeholder, its own \
             bytes escaped inside it:\n{pane}"
        );
        assert!(
            !pane.contains("data-skipped=\"html-block\">alpha") && !pane.contains("&lt;p&gt;alpha"),
            "the fallen paragraph must NOT be escaped — that is the Markdown \
             arm's presentation, deliberately diverged from here:\n{pane}"
        );
    }

    /// §8 step 5: an unclosed leaf, balanced, cannot swallow the following
    /// block's anchor. Without balance_fragment the open <b> runs on and the
    /// appended closes never appear.
    #[test]
    fn an_unclosed_block_cannot_swallow_the_next_anchor() {
        let r = rig("<section>\n<x-note>an <b>unclosed bold\n</section>\n<p>after</p>\n");
        let pane = render_source_html(&r.doc, &r.map, &r.outcomes).expect("renders");
        let x = pane
            .find("data-block-kind=\"html\"")
            .expect("x-note block renders");
        let p = pane
            .find("<p data-sync-id=\"p-")
            .expect("the following anchor survives");
        assert!(x < p, "source order:\n{pane}");
        let between = &pane[x..p];
        assert!(
            between.contains("</b>"),
            "the balancer closed the dangling <b> before the next anchor:\n{pane}"
        );
    }

    /// §8 step 1: the three PaneCtx refusals carry over VERBATIM — same
    /// constructor, same errors. A derivation that bypassed PaneCtx would
    /// accept this map and render a duplicate-claimed pane.
    #[test]
    fn the_three_map_refusals_carry_over() {
        let r = rig("<p>x</p>\n");
        let mut dup = r.map.clone();
        dup.blocks.push(dup.blocks[0].clone());
        let err = render_source_html(&r.doc, &dup, &r.outcomes).expect_err("refused");
        assert!(matches!(err, RenderError::DuplicateRow { .. }), "{err}");

        let mut uncovered = r.map.clone();
        uncovered.blocks.clear();
        let err = render_source_html(&r.doc, &uncovered, &r.outcomes).expect_err("refused");
        assert!(matches!(err, RenderError::UncoveredBlock { .. }), "{err}");
    }

    /// §8 step 1 / Deviation 8: the target string passes the same
    /// markdown::intake guard render_target applies to translated_md. Skipping
    /// the guard returns Ok here — the red this test exists for.
    #[test]
    fn the_target_pane_passes_the_intake_guard() {
        let r = rig("<p>x</p>\n");
        // In-bounds, ASCII, and refused by the guard's nesting bound: the
        // pathological prefix scores past MAX_BLOCK_NESTING_DEPTH.
        let bogus = format!("{}\n{}", ">".repeat(300), r.translated);
        let err =
            render_target_html(&r.doc, &bogus, &r.map, &r.outcomes).expect_err("guard refuses");
        assert!(matches!(err, RenderError::Intake(_)), "{err}");
        // And the honest control: the real regenerated string renders.
        render_target_html(&r.doc, &r.translated, &r.map, &r.outcomes)
            .expect("the real target renders");
    }

    /// The identity round-trip's pane corollary: over the identity target,
    /// source and target panes carry the same anchor ids in the same order.
    #[test]
    fn the_two_panes_carry_the_same_anchor_set_in_order() {
        let r = rig("<h2>a</h2>\n<ul><li>b</li><li>c</li></ul>\n<p>d</p>\n");
        let s = render_source_html(&r.doc, &r.map, &r.outcomes).expect("source");
        let t = render_target_html(&r.doc, &r.translated, &r.map, &r.outcomes).expect("target");
        let ids = |pane: &str| -> Vec<String> {
            pane.match_indices("data-sync-id=\"")
                .map(|(i, m)| {
                    let rest = &pane[i + m.len()..];
                    rest[..rest.find('"').expect("closed attr")].to_string()
                })
                .collect()
        };
        assert_eq!(ids(&s), ids(&t), "same ids, same order, both panes");
        assert_eq!(ids(&s).len(), 4, "h2 + two li + p");
    }
}
