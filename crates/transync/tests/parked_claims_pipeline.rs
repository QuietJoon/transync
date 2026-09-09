//! Executed proof of the ENGINE-side claims carried by the ten backlog
//! entries the owner deferred as a group on 2026-08-06 (`docs/backlog.md`,
//! Type 2 — "이번에는 보류").
//!
//! Each of those entries is a *scope question*, and each rests on a
//! *factual* claim about the tree ("X is not extracted", "no Y path
//! exists", "anchors are block-level only"). The questions are the owner's;
//! the facts are testable, and they had not been re-checked since they were
//! written — a month and roughly fifty DCRs earlier. This file re-checks
//! them by execution rather than by reading, so the next sweep inherits a
//! measurement instead of a month-old sentence.
//!
//! # What "proof" means here, and what it cannot mean
//!
//! Nine of these claims are claims about **today's tree only**. Unlike the
//! `transync-html` divergence work, there is no earlier implementation to
//! diff against: nothing was extracted or replaced, so the honest
//! instrument is a pin on current observable behaviour, read through the
//! channels a consumer actually has:
//!
//! - the `TranslationUnit`s a `Translator` really receives (what the model
//!   is offered for translation — the only channel in which "not extracted
//!   for translation" has meaning),
//! - `TranslationOutput::translated_document` (what the pipeline emits),
//! - `TranslationOutput::alignment_map` and its serialized wire form (what
//!   a front end syncs on),
//! - the two annotated panes (what a browser mounts).
//!
//! Two claims are not reducible to a unit test at all and are handled
//! honestly rather than stretched: the wasm32 build claim is proven by a
//! recorded `cargo check --target wasm32-unknown-unknown` probe (see
//! `transync_core_cannot_reach_wasm32_and_the_structural_reason_is_pinned`
//! for the recorded edge) with a cheap structural pin beside it, and
//! "no glossary editor tooling exists" is an assertion about the absence of
//! a *product*, which no in-tree test can establish.
//!
//! # A finding, stated up front
//!
//! `mdx-frontmatter-math-support` says the three syntaxes are
//! "unparsed/unsupported". Execution says **unsupported, but not
//! unparsed**: YAML frontmatter is parsed as a thematic break plus a
//! *setext heading* and its keys are dispatched to the model as heading
//! text; a `$$` display-math block is parsed as a paragraph and dispatched
//! as prose; MDX `import`/`export` lines are parsed as paragraphs and
//! dispatched as prose. "Parsed as something else and translated" is a
//! materially different — and riskier — situation than "unparsed", so the
//! three tests below pin what actually happens.
//!
//! TRACE: docs/backlog.md — the ten entries owner-deferred 2026-08-06

mod common;

use crate::common::mock_translator::MockTranslator;
use std::collections::BTreeSet;
use std::sync::Mutex;
use transync::llm::{
    OutputKind, TranslationBatch, TranslationBatchResult, Translator, TranslatorError, UnitResult,
};
use transync::{
    CancellationToken, FallbackStatus, SourceFormat, SyncRole, TranslateOptions, TranslationOutput,
    translate,
};

// ---------------------------------------------------------------------------
// Shared harness
// ---------------------------------------------------------------------------

fn opts() -> TranslateOptions {
    crate::common::opts_for("ko")
}

fn html_opts() -> TranslateOptions {
    let mut o = opts();
    o.input_format = SourceFormat::Html;
    o
}

/// Run the real pipeline with the recording passthrough mock and return both
/// the output and every `source_payload` the provider was offered, keyed by
/// unit id.
///
/// The recorded payloads are the point: "not extracted for translation" is a
/// statement about what crosses the `Translator` seam, and this is that seam.
async fn run_recording(
    src: &str,
    o: &TranslateOptions,
) -> (TranslationOutput, Vec<(String, String)>) {
    let t = MockTranslator::recording();
    let out = translate(src, o, &t).await.expect("the pipeline completes");
    let mut payloads = Vec::new();
    for batch in t.recorded() {
        for u in &batch.units {
            payloads.push((u.unit_id.0.clone(), u.source_payload.clone()));
        }
    }
    (out, payloads)
}

/// Every byte the provider was offered across every unit, concatenated.
/// A string absent from this is a string the model never saw.
fn all_payload_text(payloads: &[(String, String)]) -> String {
    payloads
        .iter()
        .map(|(id, p)| format!("{id}\u{1}{p}\u{1}"))
        .collect()
}

/// The set of `data-*` attribute NAMES present in a rendered pane.
///
/// Hand-scanned rather than regex-matched: this crate's dev-dependencies do
/// not include `regex`, and the shape (` data-name="`) is fixed by
/// `render::attrs::write_attrs`.
fn data_attr_names(pane: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let bytes = pane.as_bytes();
    let mut i = 0usize;
    while let Some(hit) = pane[i..].find(" data-") {
        let start = i + hit + 1; // skip the space
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'-') {
            end += 1;
        }
        names.insert(pane[start..end].to_string());
        i = end;
    }
    names
}

fn count_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

// ---------------------------------------------------------------------------
// 1. html-attribute-text-translation
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `html-attribute-text-translation`,
/// owner-deferred 2026-08-06; DCR-0016 / ADR-0018 §9 follow-up):** "HTML
/// attribute text (`alt`/`title`/`aria-label`) is not extracted for
/// translation."
///
/// The input is the smallest block carrying all three at once, with exactly
/// one text node (`Push`) so the expected payload is a one-element segment
/// array and any leaked attribute text would be a visible second element.
///
/// What the assertion establishes: the three attribute values never cross
/// the `Translator` seam — so no model is ever offered them — while the same
/// bytes survive verbatim in the emitted document. That is the difference
/// between "not translated" and "lost", and only the first is the claim.
#[tokio::test]
async fn html_attribute_text_is_never_offered_to_the_provider_markdown_island() {
    let src = "# T\n\n<div title=\"DIVTITLE\">\n<img src=\"a.png\" alt=\"ALTTEXT\">\n<button aria-label=\"ARIALABEL\">Push</button>\n</div>\n\nEnd.\n";
    let (out, payloads) = run_recording(src, &opts()).await;

    let offered = all_payload_text(&payloads);
    for value in ["DIVTITLE", "ALTTEXT", "ARIALABEL"] {
        assert!(
            !offered.contains(value),
            "attribute value {value:?} reached the provider. If this fails the \
             backlog entry `html-attribute-text-translation` has been silently \
             CLOSED by some other change and the entry must be re-derived, not \
             re-worded. Offered payloads: {payloads:?}"
        );
    }

    // The one html unit is offered exactly its one text node, labelled by its
    // owning element — the segment contract, unwidened.
    let html_payload = payloads
        .iter()
        .find(|(id, _)| id == "html-0002")
        .map(|(_, p)| p.as_str())
        .expect("the html island is a unit");
    assert_eq!(
        html_payload, "[\"Push\"]",
        "the segment array carries text nodes only; an attribute-widened \
         contract would carry four segments here"
    );

    // Not-extracted is not lost: the attributes are still in the output.
    for value in ["DIVTITLE", "ALTTEXT", "ARIALABEL"] {
        assert!(
            out.translated_document.contains(value),
            "attribute bytes must survive the splice untouched: {}",
            out.translated_document
        );
    }
}

/// **Claim (same entry, cross-reference "ti `490d97` wave 7"):** the
/// limitation grew "from raw-HTML islands to whole documents" — an HTML
/// *document* run does not extract attribute text either.
///
/// This test also settles the adjacency the entry's wording invites: an HTML
/// document's `<title>` **element** IS translated now (`BlockKind::Title`,
/// DCR-0038 / decision D5), which is a different thing from the `title`
/// **attribute**. Both facts are asserted in one place so a reader cannot
/// mistake one for the other.
#[tokio::test]
async fn an_html_document_translates_the_title_element_but_no_title_attribute() {
    let src = "<!doctype html>\n<html><head><title>PageTitle</title></head><body>\n<div title=\"DIVTITLE\"><img src=\"a.png\" alt=\"ALTTEXT\"><button aria-label=\"ARIALABEL\">Push</button></div>\n<p>Body prose.</p>\n</body></html>\n";
    let (out, payloads) = run_recording(src, &html_opts()).await;

    let offered = all_payload_text(&payloads);
    for value in ["DIVTITLE", "ALTTEXT", "ARIALABEL"] {
        assert!(
            !offered.contains(value),
            "attribute value {value:?} reached the provider on the HTML-document \
             path; the entry's wave-7 cross-reference would be stale. \
             Offered: {payloads:?}"
        );
    }

    // The adjacency, executed: the <title> ELEMENT is a real translated unit.
    let title = payloads
        .iter()
        .find(|(id, _)| id == "title-0001")
        .expect("the <title> element is a unit (DCR-0038)");
    assert_eq!(
        title.1, "[\"PageTitle\"]",
        "the <title> element's text IS offered for translation — this is the \
         `title` element, not the `title` attribute, and the entry's wording \
         survives the distinction"
    );
    let title_row = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.block_kind == "title")
        .expect("the <title> carries an alignment row");
    assert_eq!(
        title_row.sync_role,
        SyncRole::NonSync,
        "translated but not a pane anchor (invariant 1, D5)"
    );
}

// ---------------------------------------------------------------------------
// 2. streaming-translation
// ---------------------------------------------------------------------------

/// The `Translator` trait's method set, read out of the source of truth.
///
/// Textual, deliberately: Rust has no runtime reflection over a trait's
/// items, so the only way to *execute* "the surface offers no streaming
/// entry point" is to read the trait's own definition and enumerate it. The
/// slice is bounded by the `pub trait Translator` line and the first
/// column-0 `}`; doc-comment code fences inside it are all `///`-prefixed,
/// so they cannot contribute a false `fn`.
fn translator_trait_source() -> &'static str {
    const LLM: &str = include_str!("../../transync-core/src/llm.rs");
    let start = LLM
        .find("pub trait Translator")
        .expect("the Translator trait is in transync-core/src/llm.rs");
    let rest = &LLM[start..];
    let end = rest
        .find("\n}\n")
        .expect("the trait body ends at a column-0 brace");
    &rest[..end]
}

/// **Claim (`docs/backlog.md`, `streaming-translation`, owner-deferred
/// 2026-08-06; mvp-scope DEFERRED table):** "The pipeline is whole-batch
/// request/response only; no incremental/streaming delivery path exists."
///
/// Tested against the `Translator` trait's actual method set, which is the
/// only seam a provider can deliver through. The assertion establishes that
/// the set is exactly four methods — one whole-batch round trip, plus two
/// pure-metadata accessors and one preflight — and that no member of it
/// takes or returns a sink, channel, stream or callback.
#[test]
fn the_translator_trait_offers_exactly_one_whole_batch_round_trip() {
    let trait_src = translator_trait_source();

    let mut methods: Vec<&str> = Vec::new();
    for line in trait_src.lines() {
        let t = line.trim();
        let after_fn = t
            .strip_prefix("async fn ")
            .or_else(|| t.strip_prefix("fn "));
        if let Some(sig) = after_fn {
            let name = sig
                .split(|c: char| !(c.is_alphanumeric() || c == '_'))
                .next()
                .unwrap_or("");
            methods.push(name);
        }
    }
    methods.sort_unstable();

    assert_eq!(
        methods,
        vec![
            "extract_glossary",
            "fingerprint",
            "tokenizer_hint",
            "translate_batch"
        ],
        "the provider seam gained or lost a method. `streaming-translation` \
         asserts this set contains no incremental delivery entry point; if a \
         new method appeared, re-derive the entry rather than re-word it. \
         Found: {methods:?}"
    );

    // The one delivery method is whole-batch by signature: it takes an owned
    // `TranslationBatch` and returns a whole `TranslationBatchResult`. No
    // sink, channel, stream or callback appears anywhere in the trait.
    assert!(
        trait_src.contains("batch: TranslationBatch,")
            && trait_src.contains("Result<TranslationBatchResult, TranslatorError>"),
        "translate_batch is no longer a whole-batch round trip: {trait_src}"
    );
    let lowered = trait_src.to_ascii_lowercase();
    for word in [
        "stream",
        "chunk",
        "incremental",
        "sink",
        "callback",
        "yield",
    ] {
        assert!(
            !lowered.contains(word),
            "the Translator trait mentions {word:?}; an incremental delivery \
             path may have appeared and `streaming-translation`'s premise \
             would be stale"
        );
    }
}

/// A translator that answers for only the FIRST unit of every batch — the
/// shape "the provider streams the rest later" would take if the pipeline
/// tolerated partial delivery.
struct AnswersOnlyTheFirstUnit {
    calls: Mutex<u32>,
    widest_batch: Mutex<usize>,
}

#[async_trait::async_trait]
impl Translator for AnswersOnlyTheFirstUnit {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        *self.calls.lock().unwrap() += 1;
        let mut widest = self.widest_batch.lock().unwrap();
        *widest = (*widest).max(batch.units.len());
        drop(widest);

        let units = batch
            .units
            .iter()
            .take(1)
            .map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: u.source_payload.clone(),
                warnings: Vec::new(),
            })
            .collect();
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units,
        })
    }
}

/// **Claim (`streaming-translation`), behavioural half:** delivery is
/// whole-batch *request AND response*.
///
/// The trait-shape test above shows there is no streaming method. This one
/// shows the absence is enforced rather than merely unimplemented: a
/// response that answers some units and not others is treated as a **fault**
/// (the implicated units are re-dispatched, then finalize as fallback), not
/// as "the rest arrives later". A pipeline with any incremental delivery
/// notion would have to accept a partial response as legal.
///
/// Three paragraphs in one batch is the smallest input with both a delivered
/// and an undelivered unit.
#[tokio::test]
async fn a_partial_batch_response_is_a_fault_and_never_a_delivery_mode() {
    let src = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n";
    let t = AnswersOnlyTheFirstUnit {
        calls: Mutex::new(0),
        widest_batch: Mutex::new(0),
    };
    let out = translate(src, &opts(), &t).await.expect("never errors");

    // Request side: the whole batch is handed over in one call.
    assert_eq!(
        *t.widest_batch.lock().unwrap(),
        3,
        "all three units are dispatched in one whole-batch request"
    );

    // Response side, and this is the substance of the claim: the missing rows
    // are read as a FAULT and the unanswered units are re-dispatched. One call
    // would mean the partial answer had been accepted as a partial delivery.
    let calls = *t.calls.lock().unwrap();
    assert_eq!(
        calls, 3,
        "each partial response is a fault that re-dispatches only the units it \
         failed to answer, so this stub — which answers the first unit of \
         whatever batch it is handed — is asked three times: {{3 units}}, then \
         {{2}}, then {{1}}. One call would mean a partial answer had been \
         accepted as a delivery."
    );
    let statuses: Vec<(String, FallbackStatus)> = out
        .alignment_map
        .blocks
        .iter()
        .map(|b| (b.source_block_id.0.clone(), b.fallback_status))
        .collect();
    assert_eq!(
        statuses,
        vec![
            ("p-0001".to_string(), FallbackStatus::Translated),
            ("p-0002".to_string(), FallbackStatus::Translated),
            ("p-0003".to_string(), FallbackStatus::Translated),
        ],
        "all three finalize `translated` — NOT because the partial answers were \
         accepted, but because the re-dispatch outlasted the stub's stinginess: \
         round 2 answers p-0002 and round 3 answers p-0003. The re-dispatch \
         count above is what proves the fault path; this assertion only pins \
         that a re-dispatched unit is a normally translated unit and carries no \
         residue of having been missing once."
    );
}

/// A translator that answers for NO unit at all — the same fault as the stub
/// above, driven past the retry budget so the other end of the path is visible.
struct AnswersNothing {
    calls: Mutex<u32>,
}

#[async_trait::async_trait]
impl Translator for AnswersNothing {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        _cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        *self.calls.lock().unwrap() += 1;
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units: Vec::new(),
        })
    }
}

/// **Claim (`streaming-translation`), the other end of the fault path.**
///
/// The test above shows a partial response is re-dispatched rather than
/// accepted. It cannot show what happens when re-dispatch does not rescue the
/// run, because its stub answers one more unit each round and the run
/// completes. This one removes that escape: the provider answers nothing, ever.
///
/// If any incremental-delivery notion existed, an empty response would be a
/// legal "nothing yet". Instead the retry budget exhausts and every unit takes
/// invariant 6's fallback — source content, marked in the alignment map, never
/// silently corrupt.
#[tokio::test]
async fn a_response_that_never_answers_exhausts_into_fallback_not_into_waiting() {
    let src = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.\n";
    let t = AnswersNothing {
        calls: Mutex::new(0),
    };
    let out = translate(src, &opts(), &t).await.expect("never errors");

    assert!(
        *t.calls.lock().unwrap() > 1,
        "an unanswered batch is re-dispatched before it is given up on; \
         calls = {}",
        t.calls.lock().unwrap()
    );
    let statuses: Vec<FallbackStatus> = out
        .alignment_map
        .blocks
        .iter()
        .map(|b| b.fallback_status)
        .collect();
    assert_eq!(
        statuses,
        vec![
            FallbackStatus::FallbackSource,
            FallbackStatus::FallbackSource,
            FallbackStatus::FallbackSource,
        ],
        "every unit the provider never answered falls back to source and says \
         so in the alignment map. A pipeline with incremental delivery would \
         have had somewhere else to put these — a pending state, a partial \
         document — and there is no such state."
    );
}

// ---------------------------------------------------------------------------
// 3. sentence-level-sub-anchors
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `sentence-level-sub-anchors`, owner-deferred
/// 2026-08-06; draft NG3, ADR-0001):** "Sync anchors are block-level only",
/// and architectural invariant 1 names block ID as the only sync currency.
///
/// The input is two three-sentence paragraphs: the smallest document in
/// which a sentence-level scheme would be *visible* as extra anchors (six
/// sentences, three blocks).
///
/// The assertion establishes the one-to-one correspondence in both
/// directions — every anchor row has exactly one DOM anchor and every DOM
/// anchor has a row — and that nothing subdivides a block: the three
/// sentences of one paragraph produce ONE row, and the reserved
/// sub-anchoring machinery (`parent_id`, `SyncRole::ChildOnly`) is unused.
#[tokio::test]
async fn every_anchor_is_one_whole_block_and_nothing_subdivides_one() {
    let src = "# H\n\nOne. Two. Three sentences here.\n\nAnother. Paragraph. With three.\n";
    let (out, _) = run_recording(src, &opts()).await;

    let rows = &out.alignment_map.blocks;
    assert_eq!(
        rows.len(),
        3,
        "three source blocks ⇒ three rows; six sentences would be six under a \
         sentence-level scheme: {rows:?}"
    );

    let anchor_ids: Vec<&str> = rows
        .iter()
        .filter(|b| b.sync_role != SyncRole::NonSync)
        .map(|b| b.source_block_id.0.as_str())
        .collect();
    assert_eq!(anchor_ids, vec!["h1-0001", "p-0002", "p-0003"]);

    for pane in [&out.annotated_source_html, &out.annotated_target_html] {
        // Direction 1: each anchor row has exactly one DOM anchor.
        for id in &anchor_ids {
            assert_eq!(
                count_occurrences(pane, &format!("data-sync-id=\"{id}\"")),
                1,
                "block {id} must own exactly one DOM anchor: {pane}"
            );
        }
        // Direction 2: there are no OTHER anchors — no sub-anchor exists.
        assert_eq!(
            count_occurrences(pane, "data-sync-id=\""),
            anchor_ids.len(),
            "the pane carries one anchor per anchor row and nothing else; a \
             sentence-level sub-anchor would show up here: {pane}"
        );
    }

    // The reserved nesting machinery is still unused: nothing claims a parent
    // and nothing is child-only, so no anchor is *inside* another block.
    assert!(
        rows.iter()
            .all(|b| b.parent_id.is_none() && b.sync_role != SyncRole::ChildOnly),
        "`parent_id` and `child-only` are RESERVED for a future nested-anchor \
         scheme and must stay unpopulated while this entry is open: {rows:?}"
    );

    // Invariant 1's other half, executed: the paragraph's own three sentences
    // travel as ONE payload, so there is no sub-block unit either.
    let (_, payloads) = run_recording(src, &opts()).await;
    let p = payloads
        .iter()
        .find(|(id, _)| id == "p-0002")
        .expect("the paragraph is one unit");
    assert_eq!(p.1, "One. Two. Three sentences here.");
}

// ---------------------------------------------------------------------------
// 4. mdx-frontmatter-math-support  (three separate sub-claims)
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `mdx-frontmatter-math-support`,
/// owner-deferred 2026-08-06; draft NG2):** "YAML frontmatter … [is]
/// unparsed/unsupported".
///
/// **This test REFUTES the "unparsed" reading and pins what really
/// happens.** The canonical comrak configuration
/// (`intake::markdown::options`) sets no `front_matter_delimiter`, so a
/// frontmatter block is not skipped — it is parsed as ordinary GFM: the
/// opening `---` becomes a thematic break, and `title: …` followed by the
/// closing `---` becomes a **setext heading**, which is a translation unit.
/// The frontmatter's keys are therefore dispatched to the model as heading
/// text and come back as its answer.
///
/// The input is the smallest realistic frontmatter (two keys) followed by a
/// real heading, so the block sequence distinguishes "skipped" from
/// "reinterpreted".
#[tokio::test]
async fn yaml_frontmatter_is_not_unparsed_it_becomes_a_translated_setext_heading() {
    let src = "---\ntitle: My Doc\ntags: [a, b]\n---\n\n# Heading\n\nBody.\n";
    let (out, payloads) = run_recording(src, &opts()).await;

    let kinds: Vec<(&str, &str)> = out
        .alignment_map
        .blocks
        .iter()
        .map(|b| (b.source_block_id.0.as_str(), b.block_kind.as_str()))
        .collect();
    assert_eq!(
        kinds,
        vec![
            ("hr-0001", "thematic-break"),
            ("h2-0002", "heading-2"),
            ("h1-0003", "heading-1"),
            ("p-0004", "paragraph"),
        ],
        "frontmatter is REINTERPRETED, not skipped: `---` is a thematic break \
         and the YAML body is a setext heading-2. If this ever becomes a \
         frontmatter block (or disappears), the entry's premise changed."
    );

    // The load-bearing half: the YAML keys are offered to the model.
    let frontmatter_unit = payloads
        .iter()
        .find(|(id, _)| id == "h2-0002")
        .expect("the reinterpreted frontmatter is a real translation unit");
    assert_eq!(
        frontmatter_unit.1, "title: My Doc\ntags: [a, b]\n---",
        "the YAML keys AND the closing fence are the heading's payload — a \
         translator is free to rewrite `title:` and `tags:` into the target \
         language, which would break the frontmatter of any consumer that \
         reads it. This is the substance of the finding: `unsupported` is \
         true, `unparsed` is false."
    );
    let frontmatter_row = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "h2-0002")
        .expect("row exists");
    assert_eq!(
        frontmatter_row.fallback_status,
        FallbackStatus::Translated,
        "and it finalizes as TRANSLATED, not preserved"
    );
}

/// **Claim (same entry):** "math syntax [is] unparsed/unsupported".
///
/// **REFUTES "unparsed".** No math extension is enabled, so `$…$` and a
/// `$$` display block are not recognized as math — but they are not skipped
/// either: the display block is parsed as a **paragraph** and dispatched to
/// the model as prose, LaTeX and all. The input carries one inline and one
/// display form because they land in different places (inside a prose
/// payload, and as a payload of its own).
#[tokio::test]
async fn math_is_not_unparsed_it_becomes_a_translated_paragraph() {
    let src = "# M\n\nInline $E = mc^2$ here.\n\n$$\n\\int_0^1 x^2 dx\n$$\n\nEnd.\n";
    let (out, payloads) = run_recording(src, &opts()).await;

    let kinds: Vec<&str> = out
        .alignment_map
        .blocks
        .iter()
        .map(|b| b.block_kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        vec!["heading-1", "paragraph", "paragraph", "paragraph"],
        "no math block kind exists — `$$…$$` is a paragraph: {kinds:?}"
    );

    let display = payloads
        .iter()
        .find(|(id, _)| id == "p-0003")
        .expect("the display-math block is its own unit");
    assert_eq!(
        display.1, "$$\n\\int_0^1 x^2 dx\n$$",
        "the whole LaTeX body is offered to the model as paragraph prose; \
         nothing protects the `\\int` or the delimiters"
    );
    let inline = payloads
        .iter()
        .find(|(id, _)| id == "p-0002")
        .expect("the inline-math paragraph is a unit");
    assert_eq!(
        inline.1, "Inline $E = mc^2$ here.",
        "inline math is plain text inside a prose payload — no inline guard"
    );

    // Rendered, it is literal text in a <p>, not a math node.
    assert!(
        out.annotated_source_html
            .contains("$$\n\\int_0^1 x^2 dx\n$$"),
        "the pane renders the LaTeX verbatim as paragraph text: {}",
        out.annotated_source_html
    );
}

/// **Claim (same entry):** "MDX … [is] unparsed/unsupported".
///
/// **REFUTES "unparsed", and splits the answer in two.** MDX has three
/// distinguishable shapes and the intake treats them differently:
///
/// - `import` / `export` statements are parsed as **paragraphs** and
///   dispatched to the model as prose (so a translator may rewrite JS).
/// - a capitalized JSX element on its own line is a CommonMark type-7 html
///   block; it has no text nodes, so it is a **zero-segment preserved** row
///   — never dispatched, byte-preserved. This is the one MDX shape that is
///   safe by construction.
/// - a `{expression}` in prose is plain paragraph text, dispatched as prose.
///
/// The input carries exactly one of each, which is why it is three blocks
/// plus one.
#[tokio::test]
async fn mdx_is_not_unparsed_imports_translate_and_jsx_elements_preserve() {
    let src = "import Chart from './Chart'\n\nexport const meta = { a: 1 }\n\n<Chart data={items} title=\"Sales\" />\n\nCount: {count} items.\n";
    let (out, payloads) = run_recording(src, &opts()).await;

    let rows: Vec<(&str, &str, FallbackStatus)> = out
        .alignment_map
        .blocks
        .iter()
        .map(|b| {
            (
                b.source_block_id.0.as_str(),
                b.block_kind.as_str(),
                b.fallback_status,
            )
        })
        .collect();
    assert_eq!(
        rows,
        vec![
            ("p-0001", "paragraph", FallbackStatus::Translated),
            ("p-0002", "paragraph", FallbackStatus::Translated),
            ("html-0003", "html", FallbackStatus::Preserved),
            ("p-0004", "paragraph", FallbackStatus::Translated),
        ],
        "MDX statements are translated paragraphs; the JSX element is a \
         zero-segment preserved html block: {rows:?}"
    );

    let offered = all_payload_text(&payloads);
    assert!(
        offered.contains("import Chart from './Chart'")
            && offered.contains("export const meta = { a: 1 }")
            && offered.contains("Count: {count} items."),
        "the import, the export and the JSX expression in prose all cross the \
         provider seam as prose: {payloads:?}"
    );
    assert!(
        !payloads.iter().any(|(id, _)| id == "html-0003"),
        "the JSX element is never dispatched — no text nodes, no unit: \
         {payloads:?}"
    );
    assert!(
        out.translated_document
            .contains("<Chart data={items} title=\"Sales\" />"),
        "and it is byte-preserved in the output: {}",
        out.translated_document
    );
}

// ---------------------------------------------------------------------------
// 5. html-details-fold-reproduction
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `html-details-fold-reproduction`,
/// owner-deferred 2026-08-06; DCR-0016 follow-up, decision 5):**
/// "Interleaved `<details>` regions in raw-HTML blocks always render
/// visible; collapsed/expanded state is not reproduced."
///
/// "Interleaved" is the load-bearing word and the reason for this exact
/// input: `<details open>` + `<summary>` is one type-6 html block, the
/// Markdown body between them is a *separate* paragraph block, and
/// `</details>` is a third block. That is the smallest shape in which the
/// fold's contents are not inside the fold's own block.
///
/// The assertion separates two things the claim's wording runs together:
///
/// - **The document keeps the state.** `open` is markup, so extract/splice
///   never touches it; the emitted document is byte-identical here.
/// - **The pane does not reproduce it.** Per-anchor tag balancing closes
///   `<details>` at the block boundary, so the interleaved paragraph is
///   rendered as a *sibling* of the `<details>` element rather than its
///   child, and the orphan `</details>` block renders as an empty `<div>`.
///   A sibling is outside the fold, so it is visible whether the fold is
///   open or closed — which is exactly "always render visible".
///
/// The `closed` half is asserted in the same test to prove the outcome does
/// not depend on `open`: that is what "state is not reproduced" means.
#[tokio::test]
async fn an_interleaved_details_body_renders_outside_the_fold_regardless_of_open() {
    for (label, spelling) in [("open", "<details open>"), ("closed", "<details>")] {
        let src = format!(
            "# D\n\n{spelling}\n<summary>Click</summary>\n\nHidden **body**.\n\n</details>\n\nEnd.\n"
        );
        let (out, _) = run_recording(&src, &opts()).await;

        // The document half: markup, including `open`, is untouched.
        assert_eq!(
            out.translated_document, src,
            "[{label}] an identity translation must return the document byte \
             for byte, `open` attribute included"
        );

        let pane = &out.annotated_source_html;
        let details_at = pane
            .find(spelling)
            .unwrap_or_else(|| panic!("[{label}] the fold's start tag is in the pane: {pane}"));
        let close_at = pane[details_at..]
            .find("</details>")
            .map(|i| details_at + i)
            .unwrap_or_else(|| panic!("[{label}] the fold is closed in the pane: {pane}"));
        let body_at = pane
            .find("data-sync-id=\"p-0003\"")
            .unwrap_or_else(|| panic!("[{label}] the interleaved body has its own anchor: {pane}"));

        assert!(
            close_at < body_at,
            "[{label}] the `<details>` element closes BEFORE the interleaved \
             body's anchor, so the body is a sibling of the fold and renders \
             visible whether or not the fold is open. If this ever reverses, \
             the fold's contents became foldable and \
             `html-details-fold-reproduction` is resolved rather than \
             deferred. Pane: {pane}"
        );

        // The author's own `</details>` block contributes nothing: it is a
        // zero-segment preserved row whose pane anchor is an empty <div>.
        let orphan = out
            .alignment_map
            .blocks
            .iter()
            .find(|b| b.source_block_id.0 == "html-0004")
            .unwrap_or_else(|| panic!("[{label}] the closing tag is its own block"));
        assert_eq!(orphan.fallback_status, FallbackStatus::Preserved);
        assert!(
            pane.contains(
                "<div data-sync-id=\"html-0004\" data-block-kind=\"html\" data-order=\"3\" data-fallback=\"preserved\"></div>"
            ),
            "[{label}] the orphan close renders as an EMPTY div — the fold's \
             end is not reproduced in the pane at all: {pane}"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. html-nested-in-list-blockquote-protection
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`,
/// `html-nested-in-list-blockquote-protection`, owner-deferred 2026-08-06;
/// DCR-0016 follow-up, "an accepted v1 gap"), FIRST half:** HTML nested
/// inside list items / blockquotes "is protected only by coarse child-kind
/// topology labels".
///
/// The input is the smallest one covering both containers and both
/// granularities: an inline `<span>` with an attribute inside a list item, a
/// block-level `<div>` inside another list item, and both an inline `<b>`
/// and a `<div>` inside a blockquote.
///
/// The assertion establishes *what* protection exists, by naming the
/// mechanism rather than trusting a description: the units are
/// `ListItemContent` / `BlockquoteContent` (not `HtmlSegments`), they carry
/// **no** `constraints.html` — so no segment count, no segment labels, no
/// splice check — the raw tags are inside the payload the model is offered,
/// and the only structural fingerprints are the coarse ones: a list-topology
/// tuple list and a child-KIND string list (`["paragraph", "html-block"]`).
#[tokio::test]
async fn nested_html_is_offered_raw_and_guarded_only_by_coarse_topology_labels() {
    let src = "# N\n\n- item one <span data-x=\"1\">inline</span>\n- <div>block-ish</div>\n\n> quoted <b>bold</b> text\n>\n> <div>qdiv</div>\n";
    let t = MockTranslator::recording();
    let _ = translate(src, &opts(), &t).await.expect("Ok");

    let batches = t.recorded();
    let units: Vec<_> = batches.iter().flat_map(|b| b.units.iter()).collect();

    let li = units
        .iter()
        .find(|u| u.unit_id.0 == "li-0002")
        .expect("the list item is a unit");
    assert!(
        li.source_payload.contains("<span data-x=\"1\">"),
        "the raw tag AND its attribute are inside the payload the model sees: \
         {:?}",
        li.source_payload
    );
    assert!(
        li.constraints.html.is_none(),
        "a nested html island gets NO HtmlSegmentConstraints — no segment \
         count, no labels, no splice check. That absence IS the gap."
    );
    assert!(
        li.constraints.expected_list_topology.is_some(),
        "the only structural guard is the list-topology fingerprint"
    );
    assert!(
        matches!(li.input_mode, transync::InputMode::ListItemContent),
        "mode is ListItemContent, not HtmlSegments: {:?}",
        li.input_mode
    );

    let q = units
        .iter()
        .find(|u| u.unit_id.0 == "q-0004")
        .expect("the blockquote is a unit");
    assert!(
        q.source_payload.contains("<b>bold</b>") && q.source_payload.contains("<div>qdiv</div>"),
        "inline and block-level tags alike are raw in the payload: {:?}",
        q.source_payload
    );
    assert!(q.constraints.html.is_none());
    assert_eq!(
        q.constraints.expected_blockquote_children.as_deref(),
        Some(&["paragraph".to_string(), "html-block".to_string()][..]),
        "the guard is a child-KIND sequence — `html-block` says a raw-HTML \
         child is here, and nothing about what markup it contains. \"Coarse\" \
         is measured, not asserted."
    );
}

/// **Claim (same entry), SECOND half:** "the LLM can still mutate tag text
/// there."
///
/// **This half is only PARTLY true, and this test is the confirming leg.**
/// A *block-level* raw-HTML child of a container is guarded by nothing
/// finer than its child-KIND label, so a translator may rewrite its element
/// name and add attributes and be accepted: `<div>qdiv</div>` and
/// `<section onclick="steal()">qdiv</section>` are both `html-block`
/// children, so the coarse fingerprint cannot tell them apart.
///
/// The mutation is deliberately security-shaped, because the entry is a
/// protection entry: what the gap costs is markup the author did not write.
///
/// The blockquote is the input rather than the list item because the
/// blockquote is the container that *has* the child-kind label the entry
/// names — so this asserts the label's insufficiency directly, not by
/// analogy.
#[tokio::test]
async fn a_block_level_html_child_of_a_container_can_be_rewritten_and_is_accepted() {
    let src = "# N\n\n> quoted text\n>\n> <div>qdiv</div>\n";
    let t = MockTranslator::tamper_payload(
        "<div>qdiv</div>",
        "<section onclick=\"steal()\">qdiv</section>",
    );
    let out = translate(src, &opts(), &t).await.expect("Ok");

    assert!(
        out.translated_document
            .contains("<section onclick=\"steal()\">qdiv</section>"),
        "a renamed element with an injected event handler, inside a \
         blockquote, reached the output document: {}",
        out.translated_document
    );
    let q = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "q-0002")
        .expect("the blockquote has a row");
    assert_eq!(
        q.fallback_status,
        FallbackStatus::Translated,
        "and it was ACCEPTED — the child-kind label is still `html-block` \
         either way, so no layer objected. If this ever becomes \
         FallbackSource, an extraction-inside-containers design (or another \
         guard) has landed and the entry is resolved rather than deferred."
    );
}

/// **Claim (same entry), SECOND half — the refuting leg.**
///
/// The entry says nested HTML is protected "only by coarse child-kind
/// topology labels". For **inline** raw HTML that is measurably false
/// today: `validate::inline` opens with an always-on, policy-ungated,
/// ORDERED identity check over every `HtmlInline` token ("spec §4.3 …
/// hoisted ABOVE the policy gates so no profile can disable it"), and it
/// applies to `ListItemContent` and `BlockquoteContent` units like any
/// other.
///
/// So the same mutation that succeeds on a block-level child is rejected on
/// an inline one, retried, exhausted, and the unit falls back to its source
/// bytes (invariant 6). The input is the smallest pair — one inline tag in a
/// list item, one in a blockquote — and the mutation is the minimal one that
/// changes tag bytes without changing any coarse label.
///
/// This is why the entry's second half is reported PARTLY true: the gap is
/// real for block-level children and closed for inline tags.
#[tokio::test]
async fn an_inline_raw_html_tag_inside_a_container_is_guarded_verbatim() {
    let src = "# N\n\n- item one <span data-x=\"1\">inline</span>\n\n> quoted <b>bold</b> text\n";

    // (a) attribute injection into an inline tag inside a LIST ITEM.
    let t = MockTranslator::tamper_payload("data-x=\"1\"", "data-x=\"1\" onclick=\"steal()\"");
    let out = translate(src, &opts(), &t).await.expect("never errors");
    assert!(
        !out.translated_document.contains("onclick"),
        "the injected handler must NOT reach the output: the always-on \
         inline-tag identity check rejects it. If it appears, that check has \
         been weakened or stopped covering container units: {}",
        out.translated_document
    );
    assert!(
        out.translated_document
            .contains("<span data-x=\"1\">inline</span>"),
        "and the unit falls back to its SOURCE bytes verbatim (invariant 6): \
         {}",
        out.translated_document
    );
    let li = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "li-0002")
        .expect("row");
    assert_eq!(
        li.fallback_status,
        FallbackStatus::FallbackSource,
        "rejected, retried, exhausted — not accepted"
    );

    // (b) tag rename of an inline tag inside a BLOCKQUOTE: same answer.
    let t = MockTranslator::tamper_payload("<b>bold</b>", "<i>italic</i>");
    let out = translate(src, &opts(), &t).await.expect("never errors");
    assert!(
        !out.translated_document.contains("<i>italic</i>")
            && out.translated_document.contains("<b>bold</b>"),
        "an inline tag rename inside a blockquote is rejected too: {}",
        out.translated_document
    );
    let q = out
        .alignment_map
        .blocks
        .iter()
        .find(|b| b.source_block_id.0 == "q-0003")
        .expect("row");
    assert_eq!(q.fallback_status, FallbackStatus::FallbackSource);
}

// ---------------------------------------------------------------------------
// 7. in-browser-retranslation
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `in-browser-retranslation`, owner-deferred
/// 2026-08-06; Track C design spec §9):** "`transync-core` cannot compile to
/// wasm32 (tokio/tiktoken)."
///
/// This is a **build** claim, so a unit test is the wrong instrument for the
/// claim itself. It was proven by a recorded one-shot probe rather than
/// re-run here (a `--target wasm32-unknown-unknown` check on every test run
/// would cost the workspace a second target tree):
///
/// ```text
/// $ cargo check -p transync-core --target wasm32-unknown-unknown
/// error: Only features sync,macros,io-util,rt,time are supported on wasm.
///   --> …/tokio-1.52.1/src/lib.rs:478:1
/// error: could not compile `tokio` (lib) due to 1 previous error
/// CARGO_EXIT=101
/// ```
///
/// Recorded at `/Volumes/Temp/claude/parked-claims/wasm32-core-probe.txt`
/// (2026-09-09). Two corrections to the entry's parenthetical fall out of
/// it: the failing edge is **tokio alone** — specifically the workspace's
/// `rt-multi-thread` feature, which tokio's own `compile_error!` rejects on
/// wasm — and **`tiktoken-rs` checked clean** for wasm32 in the same run, as
/// did `transync-syntax`.
///
/// What this test pins is the *structural* fact behind that failure, cheaply
/// and on every run: `transync-core` carries the unsupported tokio feature,
/// while `transync-syntax` (the crate the standing wasm32 gate names) has
/// neither a `[features]` table nor any dependency on `transync-core` — the
/// two rules `CLAUDE.md` calls load-bearing for that gate. If the structural
/// facts move, the recorded probe is stale and must be re-run.
#[test]
fn transync_core_cannot_reach_wasm32_and_the_structural_reason_is_pinned() {
    const CORE: &str = include_str!("../../transync-core/Cargo.toml");
    const SYNTAX: &str = include_str!("../../transync-syntax/Cargo.toml");
    const ROOT: &str = include_str!("../../../Cargo.toml");

    // Manifests are parsed, not scraped: dotted keys and inline tables mean a
    // grep would fail open (the reason `workspace_publication.rs` parses too).
    let core: toml::Value = toml::from_str(CORE).expect("core manifest parses");
    let syntax: toml::Value = toml::from_str(SYNTAX).expect("syntax manifest parses");
    let root: toml::Value = toml::from_str(ROOT).expect("root manifest parses");

    // The failing edge: core depends on tokio, and the workspace pins tokio
    // with `rt-multi-thread`, which is not in tokio's wasm-supported set.
    assert!(
        core["dependencies"].get("tokio").is_some(),
        "transync-core still depends on tokio — the recorded wasm32 failure's \
         edge"
    );
    let tokio_features = root["workspace"]["dependencies"]["tokio"]["features"]
        .as_array()
        .expect("the workspace pins tokio's feature list")
        .iter()
        .map(|v| v.as_str().unwrap_or_default().to_string())
        .collect::<BTreeSet<_>>();
    let wasm_supported: BTreeSet<String> = ["sync", "macros", "io-util", "rt", "time"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let unsupported: Vec<&String> = tokio_features.difference(&wasm_supported).collect();
    assert!(
        !unsupported.is_empty(),
        "the workspace's tokio features are all wasm-supported now \
         ({tokio_features:?}); the recorded probe is STALE and \
         `in-browser-retranslation`'s premise must be re-measured"
    );
    assert_eq!(
        unsupported,
        vec!["rt-multi-thread"],
        "exactly one unsupported feature, and it is the one tokio's \
         compile_error! named"
    );

    // What keeps the standing gate green: the base crate has no [features]
    // table and no edge to core, dev-dependencies included.
    assert!(
        syntax.get("features").is_none(),
        "transync-syntax grew a [features] table — the standing \
         `cargo check -p transync-syntax -p transync-wasm --target \
         wasm32-unknown-unknown` gate depends on its absence"
    );
    for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(t) = syntax.get(table) {
            assert!(
                t.get("transync-core").is_none(),
                "transync-syntax gained a transync-core edge in [{table}] — \
                 that pulls tokio into the wasm32 tree and breaks the gate"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 8. intra-block-progress-indicator
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `intra-block-progress-indicator`,
/// owner-deferred 2026-08-06; ADR-0001's accepted "Bad" consequence):**
/// within very long blocks sync feels coarse, and "an optional intra-block
/// progress indicator is the drafted mitigation" (`references/draft.md`
/// §18.2).
///
/// The felt-coarseness half is a UX judgement and is not testable. The
/// falsifiable half is: **no intra-block progress signal exists** in either
/// channel a front end could read it from.
///
/// The input is a deliberately long block — a 12-row table and a 30-line
/// code block, the two shapes the entry names — so that if any per-row or
/// per-line signal were emitted, it would be emitted here.
///
/// The assertion is *set equality*, not absence of a guessed name: the
/// alignment row's wire keys are exactly the eleven schema-1.3.0 keys, and
/// the pane's `data-*` attribute names are exactly the four
/// `render::attrs::write_attrs` writes. Anything intra-block would have to
/// be a twelfth key or a fifth attribute.
#[tokio::test]
async fn no_intra_block_progress_signal_exists_in_the_wire_or_the_pane() {
    let mut src = String::from("# Long\n\n| A | B |\n| --- | --- |\n");
    for i in 0..12 {
        src.push_str(&format!("| a{i} | b{i} |\n"));
    }
    src.push_str("\n```rust\n");
    for i in 0..30 {
        src.push_str(&format!("let x{i} = {i};\n"));
    }
    src.push_str("```\n");

    let (out, _) = run_recording(&src, &opts()).await;

    let json: serde_json::Value =
        serde_json::to_value(&out.alignment_map).expect("the map serializes");
    let rows = json["blocks"].as_array().expect("blocks array");
    assert!(!rows.is_empty());
    let expected_keys: BTreeSet<String> = [
        "source_block_id",
        "target_block_id",
        "block_kind",
        "source_order",
        "target_order",
        "source_range",
        "target_range",
        "sync_role",
        "fallback_status",
        "parent_id",
        "source_format",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    for row in rows {
        let keys: BTreeSet<String> = row
            .as_object()
            .expect("a row is an object")
            .keys()
            .cloned()
            .collect();
        assert_eq!(
            keys, expected_keys,
            "an alignment row carries exactly the schema-1.3.0 keys. An \
             intra-block progress signal would have to appear here as a new \
             key — and `source_range`/`target_range` are the block's own \
             extent, not a position inside it."
        );
    }

    // The whole serialized map, not just the rows: no progress vocabulary
    // anywhere, including in `validation_summary`.
    let wire = serde_json::to_string(&out.alignment_map).expect("serializes");
    for word in [
        "progress",
        "offset",
        "sentence",
        "fraction",
        "percent",
        "line_number",
        "sub_anchor",
    ] {
        assert!(
            !wire.contains(word),
            "the alignment wire mentions {word:?}; an intra-block signal may \
             have landed and this entry's premise is stale: {wire}"
        );
    }

    for pane in [&out.annotated_source_html, &out.annotated_target_html] {
        let names = data_attr_names(pane);
        let expected: BTreeSet<String> = [
            "data-sync-id",
            "data-block-kind",
            "data-order",
            "data-fallback",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        assert_eq!(
            names, expected,
            "the rendered attribute set is exactly the four `write_attrs` \
             writes; an intra-block progress indicator would need a fifth. \
             Pane: {pane}"
        );
    }
}

// ---------------------------------------------------------------------------
// 9. glossary-editor-ux  (testable half only)
// ---------------------------------------------------------------------------

/// **Claim (`docs/backlog.md`, `glossary-editor-ux`, owner-deferred
/// 2026-08-06; mvp-scope DEFERRED table):** "Glossaries are hand-edited
/// profile TOML; no editor tooling exists."
///
/// Only the FIRST half is testable, and this test executes exactly it: a
/// glossary is authored as `[[glossary]]` in profile TOML, and hand-authored
/// TOML is the whole authoring path — the entry loads, travels on the batch,
/// and is compiled into the system prompt the provider receives.
///
/// The second half ("no editor tooling exists") is an assertion about the
/// absence of a *product*. No in-tree test can establish it — a test can
/// only show that some particular thing is missing, never that nothing
/// exists — so it is reported UNFALSIFIABLE rather than approximated here.
/// Manufacturing a proxy (grepping the CLI for a subcommand, say) would
/// dress a different, weaker claim in this one's clothes.
#[tokio::test]
async fn a_glossary_is_authored_in_profile_toml_and_reaches_the_prompt() {
    let toml_text = "slug = \"g\"\nversion = \"1\"\n\n[system]\nprompt = \"Translate from {{source_language}} to {{target_language}}.\"\n\n[[glossary]]\nsource = \"widget\"\ntarget = \"위젯\"\n";
    let profile = transync::profile::load_profile(toml_text).expect("hand-authored TOML loads");
    assert_eq!(
        profile.glossary.len(),
        1,
        "TOML is the authoring surface: `[[glossary]]` is the only input"
    );
    assert_eq!(profile.glossary[0].source_term, "widget");

    let mut o = opts();
    o.profile = Some(profile);
    let t = MockTranslator::recording();
    let _ = translate("# G\n\nA widget here.\n", &o, &t)
        .await
        .expect("Ok");

    let batches = t.recorded();
    assert!(!batches.is_empty(), "at least one batch was dispatched");
    for batch in &batches {
        assert_eq!(
            batch.glossary.len(),
            1,
            "the authored entry travels on the batch"
        );
        assert!(
            batch
                .profile
                .prompt_body
                .contains("- \"widget\" → \"위젯\""),
            "and is compiled into the system prompt the provider receives — \
             the full hand-edited-TOML-to-prompt path, with no tooling \
             anywhere in it: {}",
            batch.profile.prompt_body
        );
    }
}
