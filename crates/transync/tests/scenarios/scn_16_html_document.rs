//! SCN-16 — HTML→HTML document translation end-to-end (ti 490d97 wave 5,
//! spec 2026-08-20 §12's wave-5 acceptance).
//!
//! The fixture is wave 3's designed 14-block document (doctype, head+title,
//! nested sections, script/style, entities, unclosed fragment): block ids
//! `title-0001 … p-0014`, of which the badge-pair image (`img-0007`) and the
//! `<hr>` (`hr-0012`) are zero-segment preserved rows and the other twelve
//! are units. If the id set here disagrees with the landed intake, STOP and
//! re-derive against the wave-3 plan — do not adjust either side to pass.
//!
//! Three legs: (1) identity — the echo stub returns the document BYTE
//! IDENTICAL, which is "untouched markup byte-identical outside text nodes"
//! in its strongest form (identity-skipped echoes keep source bytes exactly;
//! D7); (2) invariant 6 — a really-rejected unit (per-kind segment-count
//! mismatch, retried with a RetryContext, exhausted) falls back to its
//! SOURCE BYTES verbatim, per block, while its neighbors stay translated and
//! the untouched markup stays byte-identical outside the text nodes the
//! stub really translated; (3) cache — an HTML run and a Markdown run of
//! the same bytes share NOTHING: with a shared cache, the second run's
//! provider still sees every one of its units.
//!
//! TRACE: SCN-16

use crate::common::mock_translator::MockTranslator;
use transync::{
    FallbackStatus, InMemoryCache, SourceFormat, SyncRole, TranslateOptions, translate,
    translate_with_cache,
};

const SRC: &str = include_str!("../fixtures/scn-16-html-document.html");

fn html_opts() -> TranslateOptions {
    let mut opts = crate::common::opts_for("ko");
    opts.input_format = SourceFormat::Html;
    opts
}

#[tokio::test]
async fn smoke_scn_16_identity_and_the_title_row() {
    let translator = MockTranslator::passthrough();
    let out = translate(SRC, &html_opts(), &translator).await.expect("Ok");

    // Leg 1: identity. Every segment echoed ⇒ every splice is the identity
    // ⇒ the whole document, head, scripts, entities and unclosed fragment
    // included, comes back byte for byte. This subsumes every "outside text
    // nodes" assertion there is.
    assert_eq!(out.translated_document, SRC);

    // Wave 5 ships no wire change: the map's schema is still 1.2.0 (row
    // source_format / map input_format are wave 6's, schema 1.3.0).
    assert_eq!(out.alignment_map.schema_version, "1.3.0");

    // D5: the <title> is a real translated row with a non-sync role — the
    // shape a thematic break has had since schema 1.0 — and no pane anchor
    // exists for it: §8's per-row rule skips every non-sync row.
    let title = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "title")
        .expect("the <title> has a real alignment row");
    assert_eq!(title.source_block_id.0, "title-0001");
    assert_eq!(title.sync_role, SyncRole::NonSync);
    assert!(
        out.annotated_source_html.starts_with("<main>")
            && out.annotated_target_html.starts_with("<main>"),
        "wave 6: the panes are the §8 synthesized fragments now",
    );
    assert!(
        !out.annotated_source_html.contains("<script")
            && !out.annotated_target_html.contains("<script"),
        "the fixture's <script> is gap and never reaches a pane (D6)",
    );

    // §6: document_title is the <title>'s extracted, entity-decoded text —
    // what the provider was told the document is called.
    assert_eq!(
        out.document_title.as_deref(),
        Some("Transync & the two-pane page")
    );

    // Suppressed by construction: a declared-HTML run carries no dominance
    // note however HTML-heavy it is.
    assert!(
        out.validation_report
            .skipped_source_nodes
            .iter()
            .all(|n| !n.contains("raw HTML blocks")),
        "{:?}",
        out.validation_report.skipped_source_nodes,
    );
}

#[tokio::test]
async fn smoke_scn_16_per_block_fallback_is_verbatim_source() {
    // Leg 2 (invariant 6, the retry-then-fallback proof): p-0006 — the
    // paragraph whose markup is `<p>A translated page keeps its
    // <em>structure</em> &mdash; only the text moves.</p>` — gets one EXTRA
    // segment appended on every attempt. per_kind::check_html rejects the
    // count mismatch on attempt 1, the ADR-0009 retry re-sends the same
    // payload with a RetryContext, the stub fails it identically, the
    // budget exhausts, and the block falls back to SOURCE BYTES.
    let translator = MockTranslator::appends_extra_segment(transync::BlockId("p-0006".to_string()));
    let out = translate(SRC, &html_opts(), &translator).await.expect("Ok");

    // The fallen block: its exact source bytes, verbatim — entity spelling
    // (&mdash;), inline markup and all. Never the mangled payload.
    assert!(
        out.translated_document.contains(
            "<p>A translated page keeps its <em>structure</em> &mdash; only the text moves.</p>"
        ),
        "invariant 6 — the fallen block is its source bytes:\n{}",
        out.translated_document,
    );
    assert!(
        !out.translated_document.contains("EXTRA"),
        "the rejected payload must never reach the output",
    );

    // Per block, never per document: everything else echoed clean, so the
    // rest of the document is still byte-identical — markup outside text
    // nodes included. (The fallen block's bytes equal its source bytes too,
    // so the WHOLE document is byte-identical on this run; the row status
    // below is what distinguishes fallback from translation.)
    assert_eq!(out.translated_document, SRC);

    let row = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "p-0006")
        .expect("the fallen block keeps its row");
    assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
    // An honestly-translated neighbor is NOT downgraded.
    let neighbor = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "h2-0005")
        .expect("the section heading keeps its row");
    assert_eq!(neighbor.fallback_status, FallbackStatus::Translated);
    // The report shows the real retries this cost (attempts > 1 for one
    // unit) — the "real rejection" half of the acceptance criterion.
    assert!(
        out.validation_report.total_retries >= 1,
        "a rejected unit must actually retry before falling back: {:?}",
        out.validation_report.total_retries,
    );
}

#[tokio::test]
async fn smoke_scn_16_html_and_markdown_runs_share_no_cache_entries() {
    // Leg 3 (spec §12's cache acceptance): the same bytes, both formats,
    // one shared cache. Run 1 (Markdown declaration) populates; run 2
    // (HTML declaration) must find NOTHING — its provider sees every unit.
    //
    // Honesty about what this leg proves: END-TO-END CORROBORATION, not the
    // axis proof. In this fixture every md/html candidate pair already
    // differs on a non-prompt axis too — block_kind (an island is "html";
    // the html run's blocks carry their element kinds) or context_hash
    // (only the html run has a title and section paths) — so this test
    // cannot isolate the prompt axes and would stay green even if both
    // prompt separations vanished. The falsifiable evidence is
    // run_level_tests::an_html_document_unit_and_a_markdown_island_unit_are_not_one_entry,
    // which holds every OTHER axis equal by construction; this leg adds the
    // one thing that test cannot: the whole translate()+shared-cache round
    // trip through the public surface.
    let cache = InMemoryCache::new();

    let md_translator = MockTranslator::recording();
    let md_opts = crate::common::opts_for("ko"); // input_format: Markdown default
    let md_out = translate_with_cache(SRC, &md_opts, &md_translator, &cache)
        .await
        .expect("the Markdown-declared run completes (with the dominance note)");
    let entries_after_md = cache.len();
    assert!(entries_after_md > 0, "run 1 populated the cache");
    // Vacuity guard (shape, not axis-collision): the Markdown parse of this
    // HTML file really produces html-segment island units, so run 2's
    // zero-hit count below is measured against a cache holding same-shaped
    // work — not against an empty overlap. It does NOT establish that any
    // pair agrees on the non-prompt axes; see the leg comment above.
    assert!(
        md_out
            .alignment_map
            .blocks
            .iter()
            .any(|b| b.block_kind == "html"),
        "the fixture parses to raw-HTML islands under Markdown",
    );

    let html_translator = MockTranslator::recording();
    let html_out = translate_with_cache(SRC, &html_opts(), &html_translator, &cache)
        .await
        .expect("Ok");
    let live_units: usize = html_translator
        .recorded()
        .iter()
        .map(|b| b.units.len())
        .sum();
    let total_units = html_out.alignment_map.validation_summary.total_units as usize;
    assert_eq!(
        live_units, total_units,
        "every unit of the HTML run was dispatched live — zero cross-format \
         cache hits end to end (corroboration; the axis-isolated proof is \
         run_level_tests', where every other axis is held equal)",
    );
    assert!(
        cache.len() > entries_after_md,
        "the HTML run filed its own entries beside — never over — the \
         Markdown run's: {} then {}",
        entries_after_md,
        cache.len(),
    );
}
