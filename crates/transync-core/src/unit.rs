//! Translation-unit construction.
//!
//! Walks `markdown::Document.blocks` and yields `TranslationBatch`es with
//! per-unit `BlockContext` and `BlockConstraints` populated.
//!
//! TRACE: SCN-01..SCN-06

pub(crate) mod budget;
pub mod context;
pub(crate) mod payload;
pub(crate) mod section;
pub(crate) mod split;

use crate::TranslateOptions;
use crate::id::BlockId;
use crate::llm::{BatchId, TokenizerHint, TranslationBatch, TranslationUnit};
use crate::markdown::Document;
use crate::profile::{default_profile, render_prompt_body};
use std::collections::HashMap;
use transync_syntax::outcome::is_translatable_block;

/// The html extraction outcome and its per-run map now live in
/// `transync_syntax::outcome` (their whole dependency closure is
/// syntax-side). Re-exported here so `transync_core::unit::{HtmlOutcome,
/// html_outcomes}` keeps resolving for the provider live-smoke test, which
/// reaches this module cross-crate via a dev-dependency. Not curated API:
/// the facade stopped re-exporting `unit` in the OI-0027 curation.
pub use transync_syntax::outcome::{HtmlOutcome, html_outcomes};

/// Build batched translation units from a parsed document.
///
/// Each sync-relevant block produces one `TranslationUnit`. Units are
/// grouped into batches honoring the configured `max_units_per_batch`.
///
/// `tokenizer_hint` is the dispatching provider's
/// `Translator::tokenizer_hint()` (the pipeline passes it through);
/// `None` selects the legacy model-name heuristic over `opts.model_id`
/// (OI-0029). Callers batching outside the pipeline — i.e. reaching this
/// module directly rather than going through `translate_with_cache` —
/// should pass the same value their translator reports.
///
/// `html_outcomes` is [`html_outcomes`]'s result **for this same `doc`** —
/// computed once per run so batching and (from Task 6) alignment and the
/// report agree on which html blocks became units (spec §3.2).
///
/// Infallible since v0.4.0 (DCR-0027). It was fallible for exactly one reason
/// — the reserved `conditional-on-section` glossary scope, refused here as
/// well as at the loader and the translate boundary because this is the only
/// door onto `render_prompt_body` a caller reaches without crossing that
/// boundary (R0001-0006, ti 0ed6eb). That scope now loads and takes effect per
/// section, so the refusal is gone and nothing else in this function can fail:
/// every remaining profile defect is dropped and named, not raised.
///
/// # Panics
///
/// Panics if `html_outcomes` was built from a **different** document than
/// `doc`. Block IDs are deterministic path-based ordinals, so two documents
/// can legitimately share one: a foreign map can claim
/// [`HtmlOutcome::Unit`] for an id whose block in *this* document fails
/// segment extraction, and unit construction then has no segments to build a
/// payload from. Panicking loudly is deliberate — the alternative (silently
/// skipping the block) would put `build_batches` out of agreement with
/// `has_translatable_blocks`, which is the worse failure. Callers must
/// honor the same-document contract; `run_pipeline` does so by construction.
///
/// TRACE: SCN-01
/// TRACE: SCN-10
/// TRACE: ADR-0001
/// TRACE: DCR-0027
pub fn build_batches(
    doc: &Document,
    opts: &TranslateOptions,
    tokenizer_hint: Option<TokenizerHint>,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Vec<TranslationBatch> {
    let mut units: Vec<TranslationUnit> = Vec::new();
    // R0006-0029/0030: id→block index + document title are per-document
    // invariants; build them once instead of per unit.
    let ctx_index = context::build_index(doc);
    for (i, block) in doc.blocks.iter().enumerate() {
        if !is_translatable_block(block, html_outcomes) {
            continue;
        }
        let (payload, input_mode, constraints) = payload::assemble(doc, block);
        let context = context::build_context(doc, i, &ctx_index);
        units.push(TranslationUnit {
            unit_id: block.block_id.clone(),
            block_kind: block.kind.clone(),
            input_mode,
            source_payload: payload,
            context,
            constraints,
            source_hash: block.source_hash,
            // Placeholder; final batch_id is overwritten below per chunk.
            batch_id: BatchId::new(0),
            retry: None,
        });
    }
    // ORDERING IS DELIBERATE: the empty-document early return stays ahead of
    // profile *rendering* and budget resolution. A malformed profile must not
    // surface on an empty document — `has_translatable_blocks` is the
    // preflight's predicate for "this run produces batches" (OI-0026), and it
    // inspects only blocks. Resolving the profile first would still return no
    // batches, but it would move a profile warning to documents that never
    // needed a profile at all. (Until DCR-0027 a hard gate stood *above* this
    // return, because the translate boundary raised it before it had even seen
    // the document; nothing here refuses a profile any more, so every profile
    // diagnostic is now below this line, where it names entries this document
    // would have translated under.)
    if units.is_empty() {
        return Vec::new();
    }

    let mut raw_profile = opts.profile.clone().unwrap_or_else(default_profile);
    // ti 490d97 wave 5 (spec §6): [system].prompt_html selection — before
    // the first compile, so the initial body, every cohort re-compile, and
    // the template rewind below inherit the selected body, and
    // CohortDigest::profile_prompt_hash moves with the format. This door
    // rather than the pipeline because it is the one BOTH callers pass
    // through (the ti 28110f rule). The fallback is the operator's prompt
    // verbatim — never a core-synthesized merge (spec §6).
    if let Some(w) = crate::profile::select_prompt_for_format(&mut raw_profile, doc.format) {
        tracing::warn!(target: "transync::profile", "{w}");
    }
    // R0001-0016 / R0001-0017: the second gate on unusable `[batching]`
    // values, for the same reason the glossary gate below has one (R0001-0006)
    // — `ProfileMetadata` is `Deserialize` with public fields, so
    // a caller-built or post-load-mutated profile never crossed
    // `load_profile`. This is the funnel that matters: the profile cloned onto
    // every `TranslationBatch` below is the one `transync-openai` reads
    // `target_output_tokens` from to fill `max_completion_tokens` /
    // `max_output_tokens`, so a zero surviving here is a provider request with
    // no room to answer. A loaded profile is already normalized and warns
    // nothing.
    for w in crate::profile::normalize_batching(&mut raw_profile.batching) {
        tracing::warn!(target: "transync::profile", "{w}");
    }
    // ti 5f6664 (backstop): the same third gate, for the same reason, on the
    // static glossary. `format_glossary` skips an entry with an empty term and
    // `escape_for_quoted` neutralizes a control character on every render path,
    // but neither knows anything about the claimed-once rule — so a profile
    // that reached this door without crossing the boundary used to render
    // *both* of two conflicting entries for one source term into every batch's
    // system prompt, and carry both on `TranslationBatch::glossary`. Warn-only,
    // like the `[batching]` knobs above — since DCR-0027 removed the reserved
    // scope's refusal, no profile check at this door refuses anything — and
    // therefore below the empty-document early return: it names entries this
    // document would have translated under. A profile that came through the
    // translate boundary (or the loader) is already normalized and warns
    // nothing, auto-glossary merge included — the merge dedupes on the same key.
    if let Some((normalized, warnings)) = crate::profile::normalized_glossary_profile(&raw_profile)
    {
        for w in &warnings {
            tracing::warn!(target: "transync::profile", "{w}");
        }
        raw_profile = normalized;
    }
    let profile = render_prompt_body(&raw_profile, &opts.source_language, &opts.target_language);
    // ti 28110f (backstop): the fourth gate on the same profile, and the last
    // moment before the compiled prompt becomes what every batch ships. A
    // profile compiled and then EDITED carries the earlier compile's sections
    // and the call above appends this run's on top; nothing here can rewind
    // that — the fields that rendered the old sections are gone — so the
    // doubled prompt is named rather than sent in silence. It lives here rather
    // than at the translate boundary because this is the door BOTH callers pass
    // through, so the direct batcher is covered and the pipeline still says it
    // exactly once; and because the body counted is the one the batches below
    // actually carry — the glossary normalization above included, and, on the
    // pipeline path, the auto-glossary merge that ran before this call.
    for w in crate::profile::stacked_prompt_section_warnings(&profile.prompt_body) {
        tracing::warn!(target: "transync::profile", "{w}");
    }
    // R0001-0015, ti 5f6664 (backstop): the control-character advisory on the
    // glossary, at the same door and for the same two reasons. It is not a
    // gate — the entry is kept and `escape_for_quoted` has already neutralized
    // the character in the bullets just rendered above — so it drops nothing,
    // which is exactly why it cannot ride along with the gate: a check that
    // changes nothing repeats itself at every gate that runs it, and this one
    // used to print the identical line three times in one run (loader,
    // boundary, here). Said once, here, over `profile.glossary` — the list
    // every batch below carries, auto-glossary merge included, so a control
    // character in a provider-extracted term is named too.
    for w in crate::profile::glossary_control_char_warnings(&profile.glossary) {
        tracing::warn!(target: "transync::profile", "{w}");
    }
    // DCR-0026: the packing-time row-window split. It runs HERE — after the
    // profile is normalized, before the instruction envelope is priced —
    // because the envelope's row-window clause is reserved on "does this
    // document hold a window unit", which only becomes true once this has run.
    // A no-op unless the run has an output ceiling and the effective table
    // strategy is `row-window-first`; see `split::split_oversize_tables` for
    // every other way it declines. It happens exactly once per run: nothing
    // downstream re-scopes a unit (ADR-0009).
    split::split_oversize_tables(&mut units, &opts.model_id, &raw_profile, tokenizer_hint);
    // ti aa92d6: the constant user-message envelope is reserved off the input
    // target from the REAL assembled strings, not from a fixed allowance. Two
    // of the instruction's conditional clauses follow the profile; the other
    // two follow batch membership, which is what packing is about to decide —
    // so they are reserved on the document-level upper bound (owner decision
    // 2026-08-06, extended to row windows by DCR-0026): does this document
    // hold an html unit, or a row-window unit, at all?
    let facts = crate::llm::prompt::DocumentFacts::of(&units);
    // ti 490d97 wave 5 (spec §6): the html_document clause is set run-level
    // from the input format. The derivation reads the §6 sentinel off the
    // units (the one carrier a bare batch holds — build_user_prompt's
    // signature is frozen); this assert is the weld that the derived
    // run-level value IS the input format, so the two can never drift apart
    // silently. Debug builds only — debug_assert_eq! compiles out in
    // release, where the §6 invariant chain plus the pins beside these
    // modules are the whole guarantee. Exhaustive match, not `==`: a third
    // SourceFormat variant must stop the compiler here.
    debug_assert_eq!(
        facts.html_document,
        match doc.format {
            crate::id::SourceFormat::Html => true,
            crate::id::SourceFormat::Markdown => false,
        },
        "the sentinel derivation and Document.format disagree — the §6 \
         invariant chain (format ⟹ spelling ⟹ assemble's sentinel) broke",
    );
    let instruction_envelope = crate::llm::prompt::instruction_envelope_json(
        crate::llm::prompt::InstructionVariant::for_run(&profile.constraints, facts),
    );
    // OI-0040: the reference-definition append tripwire. HERE because this is
    // the last point at which the run's real unit list is still owned — after
    // the row-window split above (a window is validated as its own unit, so it
    // pays the append too) and before `partition_by_section` below consumes it
    // — and because `build_batches` runs exactly once per run, which is how
    // often this note is allowed to speak.
    if let Some(w) = ref_defs_append_warning(doc, &units) {
        tracing::warn!(target: "transync::pipeline", "{w}");
    }
    // DCR-0027 P1: partition, then pack — instead of handing the whole unit
    // list to one packing call, which let a batch straddle a `##` boundary and
    // left "which glossary entries apply to this batch's prompt" without a
    // single answer. It runs after the row-window split above (a window
    // inherits its parent table's context, so it lands in the parent's section
    // by construction) and after the document-level instruction verdict, which
    // section awareness does not move: the html and row-window clauses are
    // still document facts (ti aa92d6, DCR-0026).
    //
    // The encoder is resolved once for the whole run and handed to each
    // section's packing call: building a `CoreBPE` decodes a bundled rank
    // table, and a heading-rich document has many sections.
    let bpe = crate::batch::resolve_encoder(tokenizer_hint, &opts.model_id);
    let sections = section::partition_by_section(units, &ctx_index);

    // DCR-0027 §5: a COHORT is a distinct effective glossary among the run's
    // sections, and its prompt is compiled once and shared. Memoizing is a
    // saving and nothing more — the exactly-once rule for G5's shadowing
    // warnings (R0001-0032) is NOT a property of this map, because two sections
    // can reach one cohort by different routes and only one of them shadowed
    // anything. That dedup is `said_shadowings` below (DCR-0027 amended
    // 2026-08-09, which retired §5's "emitted at the memoized cohort render").
    //
    // The full-glossary cohort is seeded with the body compiled above, so a run
    // with no section-scoped entry — every section resolving to the whole list
    // — carries byte-identically the prompt it carried before DCR-0027, and
    // therefore identical cache keys.
    let mut cohorts: HashMap<Vec<usize>, crate::profile::ProfileMetadata> = HashMap::new();
    cohorts.insert((0..raw_profile.glossary.len()).collect(), profile.clone());
    // The body a cohort re-compiles from. Rewinding first is the ti 28110f
    // rule: a cohort has a *different* glossary, so appending its sections to
    // an already-compiled body would leave the full glossary's section standing
    // beside the cohort's. `None` is the ordinary case (a template needs no
    // rewind) and also the un-rewindable third state, which
    // `stacked_prompt_section_warnings` above has already named.
    let cohort_template = crate::profile::template_prompt_body(&raw_profile).map(str::to_owned);
    // G7: which section-scoped entries found a section of THIS document.
    let mut ever_applied: std::collections::HashSet<usize> = std::collections::HashSet::new();
    // G5: shadowings already reported, by (loser, winner). A term shadowed
    // under one heading is shadowed under every heading nested below it, and
    // that is one finding, not one per section (R0001-0032). The cohort memo
    // cannot carry this: two sections resolving to the SAME entries can differ
    // in whether a shadowing happened on the way there.
    let mut said_shadowings: std::collections::HashSet<(usize, usize)> =
        std::collections::HashSet::new();
    // R0004-0076: input-budget findings already said, by their message. One
    // per distinct cohort reserve, not one per section — a document with fifty
    // headings under one cohort has one finding to report, not fifty.
    let mut said_input_budget: std::collections::HashSet<String> = std::collections::HashSet::new();

    let mut chunked: Vec<(Vec<TranslationUnit>, crate::profile::ProfileMetadata)> = Vec::new();
    for sec in sections {
        for (i, e) in raw_profile.glossary.iter().enumerate() {
            if matches!(e.scope, crate::llm::GlossaryScope::ConditionalOnSection)
                && crate::profile::entry_applies_to_section(e, &sec.heading_stack)
            {
                ever_applied.insert(i);
            }
        }
        let (cohort_key, shadowed) =
            crate::profile::effective_glossary(&raw_profile.glossary, &sec.heading_stack);
        for s in shadowed {
            if said_shadowings.insert((s.index, s.winner)) {
                tracing::warn!(target: "transync::profile", "{}", s.message);
            }
        }
        if !cohorts.contains_key(&cohort_key) {
            let mut cohort_raw = raw_profile.clone();
            if let Some(template) = &cohort_template {
                cohort_raw.prompt_body = template.clone();
            }
            cohort_raw.glossary = cohort_key
                .iter()
                .map(|&i| raw_profile.glossary[i].clone())
                .collect();
            let compiled =
                render_prompt_body(&cohort_raw, &opts.source_language, &opts.target_language);
            cohorts.insert(cohort_key.clone(), compiled);
        }
        let cohort = &cohorts[&cohort_key];
        // DCR-0027 §3: one budget per section, pricing the per-batch envelope
        // reserve from THIS section's own compiled prompt. A section's cohort
        // is knowable before packing — unlike the instruction's membership
        // clauses, which is why those stay document-level — so the reserve gets
        // more exact rather than more conservative.
        let budget = budget::resolve(
            opts,
            &raw_profile,
            tokenizer_hint,
            &cohort.prompt_body,
            &instruction_envelope,
        );
        // R0004-0076: the input axis gets the preflight the output axis has
        // had since D1. A reserve that has eaten the whole input target packs
        // one unit per batch and sends every one of them over the target, and
        // the packer's `saturating_sub(...).max(1)` made that silent. Said
        // here rather than inside the packer because the packer also runs on
        // the retry path, where the finding is the same one and repeating it
        // per round is noise; and deduped, because a heading-rich document
        // resolves this budget once per section and the answer only varies
        // with the cohort's compiled prompt.
        if let Some(w) = crate::batch::input_budget_warning(&budget, &bpe)
            && said_input_budget.insert(w.to_string())
        {
            tracing::warn!(target: "transync::profile", "{w}");
        }
        for chunk in crate::batch::group_by_token_budget_with(sec.units, &budget, &bpe) {
            chunked.push((chunk, cohort.clone()));
        }
    }

    // G7: a section-scoped entry that matched no section of this document is
    // advisory, not a defect — the profile is document-independent and the
    // entry may be meant for a sibling document — so it is named once, here,
    // the door that computed the cohorts.
    for (i, e) in raw_profile.glossary.iter().enumerate() {
        if matches!(e.scope, crate::llm::GlossaryScope::ConditionalOnSection)
            && !ever_applied.contains(&i)
        {
            tracing::warn!(
                target: "transync::profile",
                "glossary[{i}] is scoped to sections {:?}, none of which this document has; \
                 the entry reaches no prompt in this run — headings are matched trimmed and \
                 case-insensitively, at any depth",
                e.sections
            );
        }
    }

    // P6: batch ids stay a single document-order sequence 1..N across
    // sections; nothing about batch identity changes shape.
    chunked
        .into_iter()
        .enumerate()
        .map(|(i, (chunk, cohort))| {
            let batch_id = BatchId::new(i as u32 + 1);
            let units_with_batch: Vec<TranslationUnit> = chunk
                .into_iter()
                .map(|mut u| {
                    u.batch_id = batch_id.clone();
                    u
                })
                .collect();
            TranslationBatch {
                batch_id,
                units: units_with_batch,
                source_language: opts.source_language.clone(),
                target_language: opts.target_language.clone(),
                glossary: cohort.glossary.clone(),
                profile: cohort,
            }
        })
        .collect()
}

/// Share of the unit-backed translatable source bytes that must be raw HTML
/// before [`html_dominance_warning`] speaks: 80 percent.
///
/// High on purpose. A README whose hero banner, badge wall and feature table
/// are hand-written HTML is ordinary, supported input, and a note it earns on
/// every run is a note its author learns to skip. Four fifths of the
/// translatable bytes is past that shape and into the one the note is about —
/// a document whose Markdown is the accident rather than the substance.
const HTML_MASS_WARN_PERCENT: u64 = 80;

/// Floor on the translatable mass the ratio is computed over: 512 bytes.
///
/// Under it the ratio is decided by one or two blocks and says nothing about
/// the document — a fixture that is a single `<div>` is 100 percent raw HTML
/// and is not the mistake this warns about. Every HTML document a browser is
/// ever served clears it several times over, so the floor costs the warning
/// nothing on the input it exists for.
const HTML_MASS_WARN_MIN_BYTES: u64 = 512;

/// Say so when a document's translatable mass is overwhelmingly raw HTML
/// (ticket `13e145`).
///
/// **A note, never a refusal.** `translate` accepts GFM Markdown, and raw
/// HTML *inside* it is a first-class translatable kind (ADR-0018), so a
/// document that is mostly HTML islands is legitimate input that this
/// function must not break. What it cannot be is *silent*: feeding a whole
/// HTML document to a Markdown pipeline produces output that is close enough
/// to correct to be believed — the tag-opening runs translate as html blocks
/// and splice back byte-exact, while every blank-line-separated run that does
/// not open with a tag re-enters as *Markdown*. Measured against the echo stub
/// rather than reasoned about (ticket `d990b6`), that costs three things:
/// prose is re-read under Markdown inline rules (`*`, `_` and `[` consumed as
/// markup, entities decoded); `document_title` and every `section_path` come
/// from Markdown headings, so an `<h1>` leaves them empty and the prompt
/// context degrades with nothing said; and a four-space-indented run becomes
/// an indented code block, which the engine translates and re-emits **fenced**
/// (ti `457e51`), so the document that comes back has a shape the one handed
/// in did not.
///
/// All three are *silent*. The third used to be the exception — the payload
/// was the block's dedented body, every attempt reparsed as a paragraph, and
/// the block settled as a **loud** `fallback_source` (invariant 6) after three
/// provider calls — and ti `457e51`, by making indented code translate,
/// removed the only thing this shape ever reported about itself. That is why
/// the note exists rather than why it could be dropped. HTML→HTML translation
/// exists (ticket `490d97`): a run that declares
/// `TranslateOptions.input_format = SourceFormat::Html` takes the HTML intake
/// and none of the three costs above. What no library can detect is a
/// *Markdown-declared* run that is really HTML — so the honest answer to that
/// shape is still to name it, and now to point at the declaration.
///
/// The CLI refuses the clear case up front by sniffing the preamble for
/// `<!doctype` / `<html`, which is the cheap and certain half. This is the
/// other half: the backstop for a library caller that never crosses the CLI
/// boundary, and for a document that declares nothing and still is one.
///
/// **The note stays inside this library's vocabulary.** It used to close by
/// telling the reader to pass the CLI's `--allow-html-input`, which was wrong
/// twice over (ticket `d990b6`). `transync-core` is a library: a caller that
/// never crosses the CLI cannot act on that sentence, and no weld tied the
/// string to the clap flag's name, so a rename would have drifted silently.
/// And on the CLI it was unusable in *both* reachable cases — a run whose
/// preamble declared HTML only reached this code because the flag was already
/// passed, and a run whose preamble declared nothing is not what the flag is
/// about. The note now states the condition and names the absent feature; what
/// to do about it belongs to whoever owns the boundary. Naming
/// `TranslateOptions.input_format` honors the same rule from the other side:
/// it is this library's own §0 surface, the one thing every caller of this
/// code can act on, and the pins beside this function are the weld the flag
/// never had.
///
/// **Mass, not count.** The subject is the source bytes of the blocks that
/// actually become translation units — [`is_translatable_block`] against this
/// run's `html_outcomes`, the same predicate `build_batches` packs by. Counting
/// blocks instead would let a badge wall of one-line `<img>` rows outvote the
/// prose it decorates; weighing bytes lets the prose answer for itself. Blocks
/// that are not units contribute to neither side: a zero-segment html block
/// carries no translatable text, so it is not translatable mass in either
/// direction.
///
/// `html_outcomes` must be [`html_outcomes`]'s result for **this same `doc`**,
/// the same contract [`build_batches`] states.
///
/// TRACE: ti 13e145
pub(crate) fn html_dominance_warning(
    doc: &Document,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> Option<String> {
    let mut html_bytes: u64 = 0;
    let mut total_bytes: u64 = 0;
    for block in &doc.blocks {
        if !is_translatable_block(block, html_outcomes) {
            continue;
        }
        let len = block
            .source_range
            .end
            .saturating_sub(block.source_range.start) as u64;
        total_bytes += len;
        // The sentence this feeds is about how much of a MARKDOWN document is
        // written as raw HTML, which is a spelling question, not a kind one.
        if matches!(block.spelling, crate::id::Spelling::Html { .. }) {
            html_bytes += len;
        }
    }

    if total_bytes < HTML_MASS_WARN_MIN_BYTES
        || html_bytes * 100 < HTML_MASS_WARN_PERCENT * total_bytes
    {
        return None;
    }

    // Integer percent, truncated, so the sentence is a deterministic function
    // of the document and two runs over the same bytes read identically.
    let percent = html_bytes * 100 / total_bytes;
    Some(format!(
        "{percent}% of this document's translatable bytes are raw HTML blocks \
         ({html_bytes} of {total_bytes}): everything outside them is still \
         translated as GFM Markdown. If this is an HTML document rather than \
         Markdown carrying HTML, then its prose is re-read under Markdown inline \
         rules (`*`, `_` and `[` become markup, entities are decoded); the \
         document title and every section path come from Markdown headings, so \
         an <h1> leaves them empty and the provider loses that context; and any \
         four-space-indented run becomes a code block that is translated and \
         written back FENCED, so the document changes shape and nothing \
         reports it. If this is an HTML document, declare it: set \
         TranslateOptions.input_format to SourceFormat::Html and it is \
         translated as HTML (ti 490d97)"
    ))
}

// ti 13e145: the html-dominance note. Both directions matter — it must fire on
// the shape it exists for, and it must stay quiet on the shapes ADR-0018 made
// supported input, because a note nobody can act on is a note everybody
// learns to skip.
#[cfg(test)]
mod html_dominance_tests {
    use super::*;

    /// Parse + id-assign + outcome map, the three steps `run_pipeline` takes
    /// before it asks the question.
    fn warning_for(src: &str) -> Option<String> {
        let mut doc = crate::markdown::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        html_dominance_warning(&doc, &outcomes)
    }

    /// A `<div>` block of `bytes` translatable characters, as its own
    /// blank-line-delimited block.
    fn html_block(bytes: usize) -> String {
        format!("<div>\n{}\n</div>\n\n", "x".repeat(bytes))
    }

    #[test]
    fn an_html_heavy_document_is_named() {
        let src = format!("{}{}Prose.\n", html_block(400), html_block(400));
        let w = warning_for(&src).expect("html mass dominates");
        assert!(
            w.contains("raw HTML blocks"),
            "the note must say what it is about: {w}"
        );
        assert!(w.contains("490d97"), "the note must cite the ticket: {w}");
        assert!(
            w.contains("input_format"),
            "the note must name the library entry point (spec §6; ti d990b6's \
             vocabulary rule): {w}"
        );
        assert!(
            !w.contains("not implemented"),
            "the feature exists; the note must not lie in that direction: {w}"
        );
        assert!(
            !w.contains("--"),
            "never CLI vocabulary — no flag has a name here (ti d990b6): {w}"
        );
    }

    #[test]
    fn a_readme_with_html_islands_stays_quiet() {
        // A hero banner and a badge row above a page of prose: the islands
        // ADR-0018 exists for, and legitimate input.
        let src = format!(
            "# Project\n\n{}\n{}",
            html_block(120),
            "Ordinary prose paragraph that carries the document.\n\n".repeat(12),
        );
        assert_eq!(
            warning_for(&src),
            None,
            "HTML islands in a Markdown document must not raise the note",
        );
    }

    /// The floor. A fixture that *is* one `<div>` is 100% raw HTML and is not
    /// the mistake this warns about.
    #[test]
    fn a_small_document_is_below_the_floor() {
        assert_eq!(warning_for("<div>hello</div>\n"), None);
        assert_eq!(warning_for(&html_block(100)), None);
    }

    /// Zero-segment html blocks are not translatable mass in either
    /// direction, so a badge wall cannot vote the ratio up.
    #[test]
    fn zero_segment_html_is_not_translatable_mass() {
        let badges = format!("<p>{}</p>\n\n", "<img src=\"b.svg\">".repeat(40));
        let src = format!(
            "{badges}{}",
            "Ordinary prose paragraph that carries the document.\n\n".repeat(12),
        );
        assert_eq!(
            warning_for(&src),
            None,
            "an image-only html block carries no translatable text, so it is \
             neither html mass nor total mass",
        );
    }

    /// A document with no translatable mass at all divides by nothing.
    #[test]
    fn an_empty_document_is_silent() {
        assert_eq!(warning_for(""), None);
        assert_eq!(warning_for("---\n"), None);
    }
}

/// Where [`ref_defs_append_warning`] starts speaking: 6,000,000 byte-units,
/// i.e. inline-eligible units × `|Document.ref_defs|`.
///
/// Measured, not chosen. The append costs **8.7 ns per appended byte**
/// (validated out of sample to 1.2 % on a held-back 1,200-unit × 50 KB point),
/// and it is paid twice per unit because the pool goes onto both payloads —
/// so a product of 6,000,000 is ~104 ms of extra comrak work. That is the
/// smallest point in the sweep whose tax was clearly above run-to-run noise
/// (1,200 units × a 5 KB pool: +105.9 ms on a 450 ms baseline, 23.5 %). Below
/// it the append is not worth a sentence; above it, it is the run's shape.
///
/// For scale in the other direction: the largest pool anywhere in this
/// repository's own 262-document corpus is 511 bytes, and `CHANGELOG.md` — the
/// worst real case, 1,239 units against a 350 B pool — is a product of 433,650,
/// about 7 % of this threshold. Nothing here fires it, which is exactly why the
/// tripwire is code rather than a note in a register.
const REF_DEFS_APPEND_WARN_BYTE_UNITS: u64 = 6_000_000;

/// Nanoseconds of extra comrak work per byte of appended pool, per side: 87/10.
///
/// Kept as a rational rather than a `f64` so the reported figure is a
/// deterministic function of the document and two runs over the same bytes
/// read identically — the same rule `html_dominance_warning`'s truncated
/// integer percent follows.
const REF_DEFS_APPEND_NS_PER_BYTE_NUMERATOR: u64 = 87;
const REF_DEFS_APPEND_NS_PER_BYTE_DENOMINATOR: u64 = 10;

/// Say so when a run is large enough for the whole-document reference-
/// definition append to cost real time (OI-0040).
///
/// **The deferral's own re-trigger, in code.** `validate::inline` appends
/// `Document.ref_defs` — the WHOLE document's link-reference-definition pool —
/// to BOTH payloads of every inline-eligible unit before its two comrak
/// parses, so the reparsed bytes grow as O(units × |ref_defs|). The
/// `[`-prefilter in `crate::validate::inline` (OI-0040 Required Action 1)
/// removes that for any payload with no bracket to resolve, which is 90.7 % of
/// `CHANGELOG.md`'s units and every unit of the 259 documents in this corpus
/// that define no references at all. What it cannot reach is the
/// reference-heavy document — link-reference house style, where nearly every
/// block cites something — and there the append is back to full price. The fix
/// for THAT is the label-filtered pool (append only the definitions a payload
/// could reference), and it is deliberately deferred rather than built.
///
/// A deferral is only honest if someone would notice when it starts to matter,
/// and measurement says nobody here would: 3 of 262 documents carry any pool,
/// the largest is 511 bytes, `samples/` has none, and no gate can see it. So
/// the condition reports itself. A run past
/// [`REF_DEFS_APPEND_WARN_BYTE_UNITS`] is a run that would have benefited from
/// the filtered pool, and it says so once, on `transync::pipeline` — the
/// channel the sibling run-shape notes use (`unit::split`'s row-window split,
/// `batch::output_budget_warnings`), because this is a fact about the run's
/// size and not about the operator's profile.
///
/// **A note, never a refusal**, like every other advisory at this door: the
/// pool is what makes reference-style link validation work at all (design D2
/// §B4, ADR-0012 amendment §4), so nothing is dropped, skipped or capped. If
/// this ever needs to become a refusal, that is ADR-0016's recorded revisit
/// trigger — transync running as a service or over third-party documents,
/// where `units × |ref_defs|` stops being operator-self-inflicted and an
/// attacker defeats the prefilter with one `[` per block.
///
/// **What is counted.** Inline-eligible units only: `check_inline` returns
/// early for a `CodeBlock` (a fence body has no inline nodes) and for an
/// `InputMode::HtmlSegments` unit (its payload is a JSON segment array the
/// splice check owns), and a unit that never reaches the append must not vote
/// on whether the append is expensive. Two sides per unit, since the pool goes
/// onto the source payload and the translated one. Retry rounds and the
/// pre-network per-heading twin in `unit::context` are NOT counted: both would
/// only raise the figure, and this note is a floor on the cost, not a budget.
///
/// TRACE: OI-0040
pub(crate) fn ref_defs_append_warning(doc: &Document, units: &[TranslationUnit]) -> Option<String> {
    let pool_bytes = doc.ref_defs.len() as u64;
    if pool_bytes == 0 {
        return None;
    }
    let eligible = units
        .iter()
        .filter(|u| {
            !matches!(u.block_kind, crate::id::BlockKind::CodeBlock { .. })
                && !matches!(u.input_mode, crate::llm::InputMode::HtmlSegments)
        })
        .count() as u64;
    let byte_units = eligible.saturating_mul(pool_bytes);
    if byte_units < REF_DEFS_APPEND_WARN_BYTE_UNITS {
        return None;
    }

    // Integer arithmetic throughout: 2 sides × 8.7 ns per appended byte,
    // truncated to whole milliseconds.
    let est_ms = byte_units.saturating_mul(2 * REF_DEFS_APPEND_NS_PER_BYTE_NUMERATOR)
        / (REF_DEFS_APPEND_NS_PER_BYTE_DENOMINATOR * 1_000_000);
    Some(format!(
        "this run appends the document's whole {pool_bytes}-byte \
         link-reference-definition pool (`Document.ref_defs`) to both payloads of each \
         of its {eligible} inline-eligible units before the inline layer's parses: \
         {byte_units} byte-units, past the {REF_DEFS_APPEND_WARN_BYTE_UNITS} this note \
         fires at, and about {est_ms} ms of extra parsing per validation round \
         (measured at 8.7 ns per appended byte). \
         Nothing is refused and nothing is skipped — the pool is what makes reference-style \
         links validate. The append is already skipped for any payload carrying no `[`, so a \
         document that cites few of its definitions pays little of this; this one does not. \
         What remains is the label-filtered pool — append only the definitions a payload \
         could reference — deferred with this note as its re-trigger (OI-0040)"
    ))
}

// OI-0040: the deferral's tripwire. Both directions matter for the same reason
// they do for the html-dominance note above — it has to fire on the shape it
// exists for, and it has to stay silent on this repository's own corpus, which
// is what makes it evidence rather than noise.
#[cfg(test)]
mod ref_defs_append_tripwire_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode};

    /// A pool of `bytes` bytes, spelled as real definitions so the number
    /// stands for something a document could actually contain.
    fn pool(bytes: usize) -> String {
        let mut s = String::new();
        let mut i = 0;
        while s.len() < bytes {
            s.push_str(&format!(
                "[ref{i:05}]: https://example.com/docs/page-{i:05}\n"
            ));
            i += 1;
        }
        s.truncate(bytes);
        s
    }

    fn units(n: usize, kind: BlockKind, mode: InputMode) -> Vec<TranslationUnit> {
        (0..n)
            .map(|i| TranslationUnit {
                unit_id: BlockId(format!("p-{i:04}")),
                block_kind: kind.clone(),
                input_mode: mode.clone(),
                source_payload: "See [the docs][ref00000].".to_string(),
                context: BlockContext::default(),
                constraints: BlockConstraints::default(),
                source_hash: 0,
                batch_id: BatchId::new(1),
                retry: None,
            })
            .collect()
    }

    /// A parsed document carrying `pool` as its reference-definition pool.
    /// Parsed rather than hand-built so the field this reads is the one
    /// `markdown::refdefs` fills.
    fn doc_with_pool(pool: &str) -> Document {
        let mut doc = crate::markdown::parse(&format!("Prose.\n\n{pool}")).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        doc.ref_defs = pool.to_string();
        doc
    }

    #[test]
    fn a_large_pool_against_many_units_reports_itself() {
        // The smallest point in the measured sweep that is above noise:
        // 1,200 units × a 5 KB pool.
        let p = pool(5_000);
        let doc = doc_with_pool(&p);
        let us = units(1_200, BlockKind::Paragraph, InputMode::TextFragment);
        let w = ref_defs_append_warning(&doc, &us).expect("6,000,000 byte-units must fire");
        assert!(
            w.contains("link-reference-definition pool"),
            "the note must say what it is about: {w}"
        );
        assert!(w.contains("6000000"), "it must show the product: {w}");
        assert!(w.contains("1200"), "and the unit count: {w}");
        assert!(
            w.contains("104 ms"),
            "and the measured cost, deterministically: {w}"
        );
        assert!(
            w.contains("OI-0040"),
            "and the record whose deferral it re-triggers: {w}"
        );
    }

    #[test]
    fn the_repositorys_own_worst_document_stays_silent() {
        // CHANGELOG.md's measured shape: 1,239 units, a 350 B pool — 7 % of
        // the threshold. If this ever fires, the note has become noise.
        let doc = doc_with_pool(&pool(350));
        let us = units(1_239, BlockKind::Paragraph, InputMode::TextFragment);
        assert_eq!(ref_defs_append_warning(&doc, &us), None);
    }

    #[test]
    fn a_document_with_no_definitions_is_never_asked() {
        // 259 of this corpus's 262 documents, and every `samples/` fixture:
        // an empty pool costs nothing at any unit count.
        let doc = doc_with_pool("");
        let us = units(100_000, BlockKind::Paragraph, InputMode::TextFragment);
        assert_eq!(ref_defs_append_warning(&doc, &us), None);
    }

    #[test]
    fn units_that_never_reach_the_append_do_not_vote() {
        // Both of `check_inline`'s early returns, at a raw product ten times
        // the threshold. A fence body has no inline nodes and an html unit's
        // payload is a segment array, so neither pays the append — and a note
        // priced off them would be a note about nothing.
        let doc = doc_with_pool(&pool(20_000));
        let fences = units(
            3_000,
            BlockKind::CodeBlock {
                info: Some("sh".to_string()),
                fenced: true,
            },
            InputMode::TextFragment,
        );
        assert_eq!(ref_defs_append_warning(&doc, &fences), None);
        let segments = units(3_000, BlockKind::Paragraph, InputMode::HtmlSegments);
        assert_eq!(ref_defs_append_warning(&doc, &segments), None);

        // The contrast, or the two assertions above prove nothing: the same
        // pool against the same number of ordinary paragraph units fires.
        let paras = units(3_000, BlockKind::Paragraph, InputMode::TextFragment);
        assert!(ref_defs_append_warning(&doc, &paras).is_some());
    }

    /// The wiring, not the predicate: a real `build_batches` run over a real
    /// parsed document says it on `transync::pipeline`, and says it ONCE —
    /// this door runs once per run and the note is priced off the whole unit
    /// list, so a per-section or per-batch repeat would be the defect.
    #[test]
    fn the_note_reaches_the_pipeline_channel_once_per_run() {
        use crate::test_fixtures::{EventLog, record_events};
        use std::sync::Arc;

        // 60 units against a ~110 KB pool clears 6,000,000 byte-units with a
        // document small enough to parse and pack in a test: the product is
        // what the tripwire reads, not either factor on its own.
        let mut src = String::new();
        for i in 0..60 {
            src.push_str(&format!("Paragraph {i} of prose, citing nothing.\n\n"));
        }
        src.push_str(&pool(110_000));
        let mut doc = crate::markdown::parse(&src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        assert!(
            doc.ref_defs.len() >= 100_000,
            "the definitions must have reached the pool, not the blocks: {} B",
            doc.ref_defs.len()
        );

        let opts = TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let log = Arc::new(EventLog::default());
        {
            let _guard = record_events(Arc::clone(&log));
            build_batches(&doc, &opts, None, &html_outcomes(&doc));
        }
        let said: Vec<String> = log
            .messages_on("transync::pipeline")
            .into_iter()
            .filter(|m| m.contains("link-reference-definition pool"))
            .collect();
        assert_eq!(said.len(), 1, "once per run, on the run channel: {said:?}");
        assert!(
            said[0].contains("Document.ref_defs"),
            "the message names the quantity that moves it: {}",
            said[0]
        );

        // And the same 60 paragraphs with no definitions at all are silent —
        // the ordinary document, and 259 of this corpus's 262.
        let mut bare =
            crate::markdown::parse(&src[..src.find("[ref").expect("pool starts")]).expect("parses");
        crate::id::assign_block_ids(&mut bare);
        assert!(bare.ref_defs.is_empty());
        let quiet = Arc::new(EventLog::default());
        {
            let _guard = record_events(Arc::clone(&quiet));
            build_batches(&bare, &opts, None, &html_outcomes(&bare));
        }
        assert!(
            quiet
                .messages_on("transync::pipeline")
                .iter()
                .all(|m| !m.contains("link-reference-definition pool")),
            "a document with no pool must not hear about one: {:?}",
            quiet.messages_on("transync::pipeline")
        );
    }
}

// DCR-0027 SL-107: the packer runs per section, so a batch's units all come
// from one section. `unit::section` pins the partition itself; these pin what
// `build_batches` does with it — the boundary rule end to end, the two packing
// consequences (P3/P4), and the two things section coherence must NOT change
// (document order, and a flat document's packing).
#[cfg(test)]
mod section_coherent_packing_tests {
    use super::*;
    use crate::llm::TranslationBatch;
    use crate::llm::prompt::{DocumentFacts, InstructionVariant, instruction_envelope_json};
    use crate::profile::{ProfileMetadata, default_profile};

    fn opts_with(profile: ProfileMetadata) -> TranslateOptions {
        TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        }
    }

    fn batches_of(src: &str, profile: ProfileMetadata) -> Vec<TranslationBatch> {
        let mut doc = crate::markdown::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = opts_with(profile);
        build_batches(&doc, &opts, None, &html_outcomes(&doc))
    }

    /// A profile with `max_units_per_batch` set and nothing else touched.
    fn capped(max_units: u32) -> ProfileMetadata {
        let mut p = default_profile();
        p.batching.max_units_per_batch = Some(max_units);
        p
    }

    /// Every unit in document order, with the section index it belongs to —
    /// **re-derived from the emitted units**, not read back off the partition,
    /// so the assertion is independent of the code it checks.
    fn units_with_section(batches: &[TranslationBatch]) -> Vec<(&str, usize)> {
        let mut section = 0usize;
        let mut first = true;
        batches
            .iter()
            .flat_map(|b| &b.units)
            .map(|u| {
                if u.block_kind.heading_level().is_some() && !first {
                    section += 1;
                }
                first = false;
                (u.unit_id.0.as_str(), section)
            })
            .collect()
    }

    /// The section index of every unit, keyed by unit id.
    fn section_of(batches: &[TranslationBatch]) -> std::collections::HashMap<&str, usize> {
        units_with_section(batches).into_iter().collect()
    }

    fn ids(batches: &[TranslationBatch]) -> Vec<Vec<&str>> {
        batches
            .iter()
            .map(|b| b.units.iter().map(|u| u.unit_id.0.as_str()).collect())
            .collect()
    }

    /// A document of `sections` sections, `paras` paragraphs apiece, with a
    /// preamble paragraph in front.
    fn sectioned_document(sections: usize, paras: usize) -> String {
        let mut src = String::from("a preamble paragraph\n");
        for s in 0..sections {
            src.push_str(&format!("\n## Section {s}\n"));
            for p in 0..paras {
                src.push_str(&format!("\nparagraph {p} of section {s}\n"));
            }
        }
        src
    }

    /// THE acceptance criterion: no batch holds units from two sections. The
    /// cap here is *larger* than a section, so before DCR-0027 the packer
    /// would have filled batches straight across the `##` boundaries.
    #[test]
    fn no_batch_straddles_a_section_boundary() {
        let batches = batches_of(&sectioned_document(4, 2), capped(5));
        let section = section_of(&batches);

        assert!(batches.len() > 1, "the fixture produces several batches");
        for b in &batches {
            let sections: std::collections::HashSet<usize> = b
                .units
                .iter()
                .map(|u| section[u.unit_id.0.as_str()])
                .collect();
            assert_eq!(
                sections.len(),
                1,
                "batch {:?} spans {sections:?}: {:?}",
                b.batch_id,
                ids(&batches)
            );
        }
    }

    /// P1's rule stated where it bites: a heading belongs to the section it
    /// **opens**, never to the one it closes. The parser stamps a heading's own
    /// `section_path` excluding itself, so a partition keyed on raw path
    /// equality would pack `## Beta` with Alpha's prose — and then steer the
    /// Beta heading with Alpha's glossary (SL-108).
    #[test]
    fn a_heading_packs_with_the_section_it_opens() {
        let batches = batches_of(
            "## Alpha\n\nalpha prose\n\n## Beta\n\nbeta prose\n",
            // A cap far above the unit count: only the section rule can split.
            capped(64),
        );
        assert_eq!(
            ids(&batches),
            vec![vec!["h2-0001", "p-0002"], vec!["h2-0003", "p-0004"],],
            "each heading rides with its own body, and nothing merges"
        );
    }

    /// P3: a section whose units fit the budget and the unit cap is exactly
    /// one batch — one request per section, not one per unit.
    #[test]
    fn a_section_that_fits_is_exactly_one_batch() {
        let batches = batches_of(&sectioned_document(3, 3), capped(64));
        // Preamble + three sections, each whole.
        assert_eq!(batches.len(), 4, "{:?}", ids(&batches));
        assert_eq!(batches[0].units.len(), 1, "the preamble is its own section");
        for b in &batches[1..] {
            assert_eq!(b.units.len(), 4, "heading plus its three paragraphs");
        }
    }

    /// P4: the sanctioned exception. A section that cannot fit one batch is
    /// split — and every piece stays inside that section, which is the only
    /// way one section's units are ever separated.
    #[test]
    fn an_oversize_section_splits_inside_itself() {
        let batches = batches_of(&sectioned_document(2, 5), capped(2));
        let section = section_of(&batches);

        for b in &batches {
            assert!(
                b.units.len() <= 2,
                "the cap still binds: {:?}",
                ids(&batches)
            );
            let sections: std::collections::HashSet<usize> = b
                .units
                .iter()
                .map(|u| section[u.unit_id.0.as_str()])
                .collect();
            assert_eq!(sections.len(), 1, "and the split stays inside one section");
        }
        // 1 preamble + two sections of 6 units each at a cap of 2 = 1 + 3 + 3.
        assert_eq!(batches.len(), 7, "{:?}", ids(&batches));
    }

    /// Nothing is reordered: batching regroups requests, it never moves a unit
    /// past another (contracts.md §5).
    #[test]
    fn document_order_survives_the_partition() {
        let batches = batches_of(&sectioned_document(3, 2), capped(2));
        let flat: Vec<&str> = batches
            .iter()
            .flat_map(|b| b.units.iter().map(|u| u.unit_id.0.as_str()))
            .collect();
        let mut sorted = flat.clone();
        // Block ids carry a source-order ordinal (ADR-0005), so lexical order
        // on the ordinal is document order for this fixture's ids.
        sorted.sort_by_key(|id| id.rsplit('-').next().unwrap().to_string());
        assert_eq!(flat, sorted, "units stayed in document order");
        // And batch ids are one 1..N sequence across sections (P6).
        for (i, b) in batches.iter().enumerate() {
            assert_eq!(b.batch_id, BatchId::new(i as u32 + 1));
            for u in &b.units {
                assert_eq!(u.batch_id, b.batch_id);
            }
        }
    }

    /// The no-op case, pinned: a document with no heading is one section, so
    /// its packing is bit-identical to the pre-DCR-0027 behavior.
    #[test]
    fn a_flat_document_packs_exactly_as_it_did() {
        let src = "one\n\ntwo\n\nthree\n\nfour\n\nfive\n";
        let batches = batches_of(src, capped(2));
        assert_eq!(
            ids(&batches),
            vec![
                vec!["p-0001", "p-0002"],
                vec!["p-0003", "p-0004"],
                vec!["p-0005"],
            ]
        );
    }

    /// DCR-0026 composes by ordering: the splitter runs first, and a window
    /// inherits the parent table's context — so windows land in the parent's
    /// section without the partition knowing what a window is.
    #[test]
    fn row_windows_land_in_their_parent_tables_section() {
        let mut table = String::from("| id | name | kind | note |\n|---|---|---|---|");
        for i in 1..=40 {
            table.push_str(&format!(
                "\n| {i} | item number {i} | widget | a note about item {i} |"
            ));
        }
        let src = format!("## Alpha\n\nalpha prose\n\n## Beta\n\n{table}\n");
        let mut profile = default_profile();
        profile.constraints.default_table_strategy = Some("row-window-first".to_string());
        profile.batching.target_output_tokens = Some(400);
        let batches = batches_of(&src, profile);

        let windows: Vec<&str> = batches
            .iter()
            .flat_map(|b| &b.units)
            .filter(|u| matches!(u.input_mode, crate::llm::InputMode::TableRowWindow { .. }))
            .map(|u| u.unit_id.0.as_str())
            .collect();
        assert!(windows.len() > 1, "the fixture must really split");

        // The general rule holds for a split document too: the windows'
        // batches are Beta's, and Alpha's heading never shares one with them.
        let section = section_of(&batches);
        for b in &batches {
            let sections: std::collections::HashSet<usize> = b
                .units
                .iter()
                .map(|u| section[u.unit_id.0.as_str()])
                .collect();
            assert_eq!(
                sections.len(),
                1,
                "batch {:?} spans {sections:?}",
                b.batch_id
            );
        }
        for b in &batches {
            let has_window = b
                .units
                .iter()
                .any(|u| matches!(u.input_mode, crate::llm::InputMode::TableRowWindow { .. }));
            if !has_window {
                continue;
            }
            for u in &b.units {
                assert!(
                    u.unit_id.0.starts_with("t-0004") || u.unit_id.0 == "h2-0003",
                    "a window's batch may only hold Beta's own units, got {}",
                    u.unit_id
                );
            }
        }
    }

    /// §3, the reserve half: each batch's envelope reserve plus its per-unit
    /// estimates still bound the request that batch becomes — now asserted per
    /// batch of a sectioned document, which is where per-section budgets have
    /// to hold once each section prices its own prompt (SL-108).
    #[test]
    fn every_sections_reserve_bounds_its_own_request() {
        let batches = batches_of(&sectioned_document(3, 2), capped(3));
        let bpe = crate::batch::resolve_encoder(None, &TranslateOptions::default().model_id);
        let facts = DocumentFacts::of_batches(&batches);

        for b in &batches {
            let envelope = instruction_envelope_json(InstructionVariant::for_run(
                &b.profile.constraints,
                facts,
            ));
            let opts = opts_with(b.profile.clone());
            let budget =
                budget::resolve(&opts, &b.profile, None, &b.profile.prompt_body, &envelope);
            let estimated = crate::batch::envelope_input_tokens(&budget, &bpe)
                + b.units
                    .iter()
                    .map(|u| crate::batch::estimate_unit_tokens(u, &bpe))
                    .sum::<usize>();
            let actual = bpe
                .encode_with_special_tokens(
                    &crate::llm::prompt::build_user_prompt(b).expect("prompt builds"),
                )
                .len();
            assert!(
                estimated >= actual,
                "batch {:?}: estimated {estimated} < actual {actual}",
                b.batch_id
            );
        }
    }
}

// A3: Skipped nodes are never batched — they carry no translatable text
// and are preserved verbatim (invariant 7).
#[cfg(test)]
mod skipped_unit_tests {
    use super::*;
    use crate::id::BlockKind;

    #[test]
    fn skipped_node_yields_no_translation_unit() {
        // No node maps to Skipped under the current comrak options (html
        // became a real kind); pin the never-batched path synthetically.
        let mut doc = crate::markdown::parse("real paragraph\n").expect("parses");
        doc.blocks.insert(
            0,
            crate::markdown::Block {
                block_id: crate::id::BlockId::new("x", 99),
                kind: BlockKind::Skipped {
                    label: "unsupported".to_string(),
                },
                spelling: crate::id::Spelling::Markdown,
                source_range: crate::markdown::ranges::ByteRange { start: 0, end: 0 },
                source_hash: 0,
                section_path: Vec::new(),
                ast_path: crate::markdown::AstPath(Vec::new()),
            },
        );
        crate::id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &html_outcomes(&doc));

        let mut units = batches.iter().flat_map(|b| b.units.iter());
        assert!(
            !units.clone().any(|u| u.unit_id.0.starts_with("x-")),
            "a Skipped (x-) block must never become a translation unit",
        );
        assert!(
            units.any(|u| u.unit_id.0.starts_with("p-")),
            "the ordinary paragraph should still be batched as a unit",
        );
    }
}

// OI-0026: the extraction preflight skips documents that would produce no
// batches, so its predicate must agree with `build_batches` exactly.
#[cfg(test)]
mod oi_0026_tests {
    use super::*;
    use transync_syntax::outcome::has_translatable_blocks;

    #[test]
    fn has_translatable_blocks_matches_batch_emptiness() {
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        // `---` is a thematic break: parsed as a block yet never batched, so
        // a document made only of them must report false. A text-bearing
        // `<div>` IS batched (spec §3.2) and must report true.
        for (src, expected) in [
            ("", false),
            ("---\n", false),
            ("<div class=\"note\">n</div>\n", true),
            ("real paragraph\n", true),
            ("---\n\nreal paragraph\n", true),
        ] {
            let mut doc = crate::markdown::parse(src).expect("parses");
            crate::id::assign_block_ids(&mut doc);
            let outcomes = html_outcomes(&doc);
            assert_eq!(
                has_translatable_blocks(&doc, &outcomes),
                expected,
                "has_translatable_blocks disagreed for {src:?}"
            );
            assert_eq!(
                !build_batches(&doc, &opts, None, &outcomes).is_empty(),
                expected,
                "build_batches disagreed for {src:?}"
            );
        }
    }
}

// R0001-0016 / R0001-0017: unusable `[batching]` values are neutralized on
// the profile that rides out with every batch, not only on the one the loader
// produced.
#[cfg(test)]
mod zero_batching_knob_tests {
    use super::*;

    fn opts_with_profile(profile: crate::profile::ProfileMetadata) -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        }
    }

    /// The batch profile is what `transync-openai` serializes: it reads
    /// `profile.batching.target_output_tokens` straight into
    /// `max_completion_tokens` / `max_output_tokens`, and omits the field for
    /// `None` (pinned there by `chat_request_body_omits_max_completion_tokens_when_none`
    /// and its Responses twin). So "no batch carries a zero ceiling" is the
    /// same statement as "no request carries a zero ceiling" — including for a
    /// hand-built profile, which never crossed `load_profile`.
    #[test]
    fn no_batch_carries_a_zero_output_ceiling() {
        let mut profile = crate::profile::default_profile();
        profile.batching.target_output_tokens = Some(0);
        profile.batching.max_units_per_batch = Some(0);
        profile.batching.target_input_tokens_per_batch = Some(0);

        let mut doc =
            crate::markdown::parse("first paragraph\n\nsecond paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let batches = build_batches(&doc, &opts_with_profile(profile), None, &outcomes);

        assert!(!batches.is_empty(), "the fixture produces batches");
        for b in &batches {
            assert_eq!(
                b.profile.batching.target_output_tokens, None,
                "a zero ceiling must not reach the provider request"
            );
            assert_eq!(b.profile.batching.max_units_per_batch, None);
            assert_eq!(b.profile.batching.target_input_tokens_per_batch, None);
        }
        // The zero unit cap resolved as unset, not as a cap of one: both
        // paragraphs fit the built-in default cap and pack into one batch.
        assert_eq!(batches.len(), 1, "a zero cap is ignored, not floored to 1");
    }

    /// A profile the loader produced is already normalized, so the gate is a
    /// no-op on the common path — a legal ceiling survives untouched.
    #[test]
    fn a_legal_ceiling_rides_out_unchanged() {
        let mut profile = crate::profile::default_profile();
        profile.batching.target_output_tokens = Some(8000);

        let mut doc = crate::markdown::parse("a paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let batches = build_batches(&doc, &opts_with_profile(profile), None, &outcomes);

        assert_eq!(batches[0].profile.batching.target_output_tokens, Some(8000));
    }
}

// R0001-0018 / R0001-0019, ti 5f6664 backstop: a glossary entry that cannot
// mean what it says is dropped on the profile riding out with every batch, not
// only on the one the loader produced and the one the translate boundary saw.
#[cfg(test)]
mod unusable_glossary_entry_tests {
    use super::*;
    use crate::llm::{GlossaryEntry, GlossaryScope};
    use crate::profile::ProfileMetadata;

    fn entry(source: &str, target: &str) -> GlossaryEntry {
        GlossaryEntry {
            source_term: source.to_string(),
            target_term: target.to_string(),
            note: None,
            scope: GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }
    }

    fn opts_with_profile(profile: ProfileMetadata) -> TranslateOptions {
        TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        }
    }

    fn two_paragraph_doc() -> crate::markdown::Document {
        let mut doc =
            crate::markdown::parse("first paragraph\n\nsecond paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        doc
    }

    fn occurrences(haystack: &str, needle: &str) -> usize {
        haystack.matches(needle).count()
    }

    /// `GlossaryEntry` is not `PartialEq` (curated surface, frozen), so
    /// compare the two fields the gate decides about.
    fn pairs(entries: &[GlossaryEntry]) -> Vec<(&str, &str)> {
        entries
            .iter()
            .map(|e| (e.source_term.as_str(), e.target_term.as_str()))
            .collect()
    }

    /// The batch profile *is* the provider's system prompt. Two entries
    /// claiming one source term are two answers to one question, and the
    /// renderer has no opinion about that — it escapes and skips, it does not
    /// dedupe — so without this gate a caller batching directly shipped both
    /// contradictory bullets in every batch.
    #[test]
    fn no_batch_carries_two_renderings_of_one_source_term() {
        let mut profile = default_profile();
        profile.glossary = vec![
            entry("agent", "에이전트"),
            // Same term under the trimmed, case-folded identity.
            entry(" Agent ", "대리인"),
            entry("   ", "dropped: empty source"),
            entry("tool use", "  "),
        ];

        let doc = two_paragraph_doc();
        let outcomes = html_outcomes(&doc);
        let batches = build_batches(&doc, &opts_with_profile(profile), None, &outcomes);

        assert!(!batches.is_empty(), "the fixture produces batches");
        for b in &batches {
            assert_eq!(
                pairs(&b.profile.glossary),
                vec![("agent", "에이전트")],
                "only the first claim on the term survives, and neither empty-term entry does"
            );
            // `TranslationBatch::glossary` is the same list a reviewer or an
            // exporter reads programmatically.
            assert_eq!(pairs(&b.glossary), pairs(&b.profile.glossary));
            assert_eq!(
                occurrences(&b.profile.prompt_body, "에이전트"),
                1,
                "the winning rendering appears once: {:?}",
                b.profile.prompt_body
            );
            assert_eq!(
                occurrences(&b.profile.prompt_body, "대리인"),
                0,
                "the losing rendering never reaches the system prompt: {:?}",
                b.profile.prompt_body
            );
            assert_eq!(
                occurrences(&b.profile.prompt_body, "dropped: empty source"),
                0
            );
        }
    }

    /// The ti 28110f rewind travels with the gate: dropping an entry under an
    /// already-compiled profile must rewind the body to its template, or the
    /// stale section stays and the effective one is appended beside it.
    #[test]
    fn a_compiled_profile_losing_an_entry_carries_one_glossary_section() {
        let mut profile = default_profile();
        profile.glossary = vec![entry("agent", "에이전트"), entry("AGENT", "대리인")];
        let compiled = render_prompt_body(&profile, "en", "ko");
        assert_eq!(
            occurrences(&compiled.prompt_body, "대리인"),
            1,
            "the fixture's compiled body starts out carrying both bullets"
        );

        let doc = two_paragraph_doc();
        let outcomes = html_outcomes(&doc);
        let batches = build_batches(&doc, &opts_with_profile(compiled), None, &outcomes);

        for b in &batches {
            assert_eq!(
                occurrences(
                    &b.profile.prompt_body,
                    "Glossary (when the source term appears"
                ),
                1,
                "one glossary section, not the stale one plus the effective one: {:?}",
                b.profile.prompt_body
            );
            assert_eq!(occurrences(&b.profile.prompt_body, "대리인"), 0);
            assert_eq!(occurrences(&b.profile.prompt_body, "에이전트"), 1);
        }
    }

    /// The gate is a no-op on every profile that came through the loader or the
    /// translate boundary: a clean glossary rides out byte-identical, section
    /// and all.
    #[test]
    fn a_clean_glossary_rides_out_unchanged() {
        let mut profile = default_profile();
        profile.glossary = vec![entry("agent", "에이전트"), entry("tool use", "도구 사용")];
        let compiled = render_prompt_body(&profile, "en", "ko");

        let doc = two_paragraph_doc();
        let outcomes = html_outcomes(&doc);
        let batches = build_batches(&doc, &opts_with_profile(compiled.clone()), None, &outcomes);

        for b in &batches {
            assert_eq!(pairs(&b.profile.glossary), pairs(&compiled.glossary));
            assert_eq!(
                b.profile.prompt_body, compiled.prompt_body,
                "no rewind, no re-compile"
            );
        }
    }
}

// R0001-0015, ti 5f6664 (backstop): the control-character advisory is not a
// gate — nothing is dropped and `escape_for_quoted` has already neutralized
// the character — so a gate is the wrong place for it: every gate that ran it
// re-printed it. It is said ONCE, at the door the compiled prompt leaves by,
// over the entries that actually ship.
#[cfg(test)]
mod glossary_control_char_tests {
    use super::*;
    use crate::llm::{GlossaryEntry, GlossaryScope};
    use crate::profile::{ProfileMetadata, default_profile, load_profile};
    use crate::test_fixtures::{EventLog, record_events};
    use std::sync::Arc;

    fn flagged(source: &str, target: &str, note: &str) -> GlossaryEntry {
        GlossaryEntry {
            source_term: source.to_string(),
            target_term: target.to_string(),
            note: Some(note.to_string()),
            scope: GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }
    }

    fn two_paragraph_doc() -> crate::markdown::Document {
        let mut doc =
            crate::markdown::parse("first paragraph\n\nsecond paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        doc
    }

    /// Batch under `profile`, returning the batches and every message the run
    /// raised on the profile channel.
    fn batch_and_capture(profile: ProfileMetadata) -> (Vec<TranslationBatch>, Vec<String>) {
        let doc = two_paragraph_doc();
        let outcomes = html_outcomes(&doc);
        let opts = TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        };
        let log = Arc::new(EventLog::default());
        let batches = {
            let _guard = record_events(Arc::clone(&log));
            build_batches(&doc, &opts, None, &outcomes)
        };
        (batches, log.messages_on("transync::profile"))
    }

    /// A caller-built profile never met the loader, so this door is the only
    /// one that can name the character — and it does.
    #[test]
    fn a_control_character_is_named_at_the_batching_door() {
        let mut profile = default_profile();
        profile.glossary = vec![flagged("agent", "에이전트", "line\nbreak")];

        let (batches, said) = batch_and_capture(profile);
        let hits: Vec<&String> = said.iter().filter(|m| m.contains("U+000A")).collect();
        assert_eq!(hits.len(), 1, "named, once: {said:?}");
        assert!(hits[0].starts_with("glossary[0].note"), "{hits:?}");
        // The advisory is diagnostics-only: the entry ships, escaped.
        for b in &batches {
            assert_eq!(b.glossary.len(), 1);
            assert!(
                b.profile.prompt_body.contains("line\\nbreak"),
                "the note rides out escaped, not dropped and not raw: {}",
                b.profile.prompt_body
            );
            assert!(!b.profile.prompt_body.contains("line\nbreak"));
        }
    }

    /// Diagnostics-only, on the bytes: a *compiled* profile whose only oddity
    /// is a flagged entry rides out byte-identical. This is the path that
    /// changed — the advisory used to make `normalized_glossary_profile`
    /// return `Some`, which rewound the compiled body to its template before
    /// re-compiling it. That was a no-op on the bytes, and stays one now that
    /// no rewind happens at all.
    #[test]
    fn a_compiled_profile_with_a_flagged_entry_is_byte_identical() {
        let mut profile = default_profile();
        profile.prompt_body = "Translate from {{source_language}} to {{target_language}}.".into();
        profile.glossary = vec![flagged("agent", "에이전트", "line\nbreak")];
        let compiled = crate::profile::render_prompt_body(&profile, "en", "ko");

        let (batches, _) = batch_and_capture(compiled.clone());
        for b in &batches {
            assert_eq!(b.profile.prompt_body, compiled.prompt_body);
        }
    }

    /// The regression this backstop closes: a profile the loader already
    /// looked at is not re-diagnosed here. Before, `normalize_glossary`
    /// returned this warning without dropping anything, so the boundary and
    /// this door each printed the loader's line again — three identical WARN
    /// lines in one CLI run (R0001-0032).
    #[test]
    fn a_loaded_profile_is_named_once_here_and_not_by_the_loader() {
        let toml = "slug = \"p\"\nversion = \"1.0.0\"\n[system]\nprompt = \"Translate.\"\n\
             [[glossary]]\nsource = \"agent\"\ntarget = \"에이전트\"\nnote = \"line\\nbreak\"\n";
        let log = Arc::new(EventLog::default());
        let loaded = {
            let _guard = record_events(Arc::clone(&log));
            load_profile(toml).expect("loads")
        };
        assert_eq!(
            log.messages_on("transync::profile")
                .iter()
                .filter(|m| m.contains("U+000A"))
                .count(),
            0,
            "the loader records the finding; it does not emit it"
        );

        let (_, said) = batch_and_capture(loaded);
        assert_eq!(
            said.iter().filter(|m| m.contains("U+000A")).count(),
            1,
            "and the batching door says it exactly once: {said:?}"
        );
    }

    /// The advisory follows the entries that ship, not the ones the file held:
    /// a term merged in after the loader — the auto-glossary harvest arrives
    /// this way — is scanned too.
    #[test]
    fn a_term_that_never_met_the_loader_is_scanned_as_well() {
        let toml = "slug = \"p\"\nversion = \"1.0.0\"\n[system]\nprompt = \"Translate.\"\n\
             [[glossary]]\nsource = \"agent\"\ntarget = \"에이전트\"\n";
        let mut loaded = load_profile(toml).expect("loads");
        assert!(loaded.load_warnings.is_empty(), "the file was clean");
        loaded
            .glossary
            .push(flagged("tensor", "텐서", "harvested\u{2028}note"));

        let (_, said) = batch_and_capture(loaded);
        let hits: Vec<&String> = said.iter().filter(|m| m.contains("U+2028")).collect();
        assert_eq!(hits.len(), 1, "{said:?}");
        assert!(hits[0].starts_with("glossary[1].note"), "{hits:?}");
    }

    /// And a clean glossary stays silent — the advisory must not turn into
    /// "every run now warns".
    #[test]
    fn a_clean_glossary_says_nothing() {
        let mut profile = default_profile();
        profile.glossary = vec![GlossaryEntry {
            source_term: "agent".into(),
            target_term: "에이전트".into(),
            note: Some("a plain note".into()),
            scope: GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }];
        assert!(batch_and_capture(profile).1.is_empty());
    }
}

// ti 28110f (backstop): the state no rewind can reach — a profile compiled and
// then edited outside the library — is NAMED where the compiled prompt becomes
// what every batch ships. That door is this one, not the translate boundary:
// `build_batches` is `#[doc(hidden)] pub` and a caller batching directly used
// to ship the doubled system prompt with nothing said at all.
#[cfg(test)]
mod stacked_section_gate_tests {
    use super::*;
    use crate::llm::{GlossaryEntry, GlossaryScope};
    use crate::profile::{ProfileMetadata, default_profile, render_prompt_body};
    use crate::test_fixtures::{EventLog, record_events};
    use std::sync::Arc;

    const GLOSSARY_HEADER: &str =
        "Glossary (when the source term appears, prefer the target form below; omit otherwise):";
    const POLICY_HEADER: &str = "Structural policies:";

    fn entry(source: &str, target: &str) -> GlossaryEntry {
        GlossaryEntry {
            source_term: source.to_string(),
            target_term: target.to_string(),
            note: None,
            scope: GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }
    }

    fn opts_with_profile(profile: ProfileMetadata) -> TranslateOptions {
        TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        }
    }

    fn two_paragraph_doc() -> crate::markdown::Document {
        let mut doc =
            crate::markdown::parse("first paragraph\n\nsecond paragraph\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        doc
    }

    fn section_lines(body: &str, header: &str) -> usize {
        body.lines().filter(|l| *l == header).count()
    }

    /// Batch under `profile`, returning the batches and every message the run
    /// raised on the profile channel.
    fn batch_and_capture(profile: ProfileMetadata) -> (Vec<TranslationBatch>, Vec<String>) {
        let doc = two_paragraph_doc();
        let outcomes = html_outcomes(&doc);
        let opts = opts_with_profile(profile);
        let log = Arc::new(EventLog::default());
        let batches = {
            let _guard = record_events(Arc::clone(&log));
            build_batches(&doc, &opts, None, &outcomes)
        };
        let messages = log.messages_on("transync::profile");
        (batches, messages)
    }

    /// A compiled profile whose glossary was edited afterwards. Both sections
    /// land in every batch's system prompt twice, and — the point of this
    /// backstop — the door says so.
    #[test]
    fn a_compiled_then_edited_profile_is_named_at_the_batching_door() {
        let mut profile = default_profile();
        profile.prompt_body = "Translate from {{source_language}} to {{target_language}}.".into();
        profile.constraints.preserve_urls = Some(true);
        profile.glossary = vec![entry("agent", "에이전트")];
        let mut edited = render_prompt_body(&profile, "en", "ko");
        edited.glossary = vec![entry("agent", "에이전트"), entry("tensor", "텐서")];

        let (batches, messages) = batch_and_capture(edited);

        assert!(!batches.is_empty(), "the fixture produces batches");
        for b in &batches {
            assert_eq!(
                section_lines(&b.profile.prompt_body, GLOSSARY_HEADER),
                2,
                "the fixture really does ship a doubled prompt: {}",
                b.profile.prompt_body
            );
        }
        let named: Vec<&String> = messages
            .iter()
            .filter(|m| m.contains("copies of the"))
            .collect();
        assert_eq!(named.len(), 2, "one per doubled section: {messages:?}");
        for name in ["structural-policy", "glossary"] {
            // Anchored on the counting clause: both messages also *mention*
            // "glossary" in the advice sentence they share.
            let claim = format!("copies of the {name} section");
            assert_eq!(
                named.iter().filter(|m| m.contains(&claim)).count(),
                1,
                "{name}: {named:?}"
            );
        }
        for m in &named {
            assert!(m.contains("2 copies"), "the count is stated: {m}");
        }
    }

    /// The two states the type is meant to hold stay silent, so the gate
    /// cannot fire on an ordinary run: a template, and the compiled prompt it
    /// compiles to — which is exactly what every batch already carries.
    #[test]
    fn a_template_and_its_own_compiled_prompt_are_silent() {
        let mut profile = default_profile();
        profile.constraints.preserve_urls = Some(true);
        profile.glossary = vec![entry("agent", "에이전트")];

        for candidate in [profile.clone(), render_prompt_body(&profile, "en", "ko")] {
            let (batches, messages) = batch_and_capture(candidate);
            for b in &batches {
                assert_eq!(section_lines(&b.profile.prompt_body, GLOSSARY_HEADER), 1);
                assert_eq!(section_lines(&b.profile.prompt_body, POLICY_HEADER), 1);
            }
            assert!(
                messages.iter().all(|m| !m.contains("copies of the")),
                "{messages:?}"
            );
        }
    }

    /// The count is taken on the body the batches actually carry, so the
    /// glossary gate that runs just above it cannot be counted around: an entry
    /// dropped there rewinds the compiled body, and one section comes out.
    #[test]
    fn a_dropped_entry_rewinds_before_the_count_and_stays_silent() {
        let mut profile = default_profile();
        profile.glossary = vec![entry("agent", "에이전트"), entry("AGENT", "대리인")];
        let compiled = render_prompt_body(&profile, "en", "ko");

        let (batches, messages) = batch_and_capture(compiled);
        for b in &batches {
            assert_eq!(section_lines(&b.profile.prompt_body, GLOSSARY_HEADER), 1);
        }
        assert!(
            messages.iter().all(|m| !m.contains("copies of the")),
            "the rewind ran first, so there is nothing to name: {messages:?}"
        );
        assert!(
            messages.iter().any(|m| m.contains("대리인")),
            "the dropped entry is still named: {messages:?}"
        );
    }
}

// Spec §3.2–§3.3: per-block translatability and the zero-segment /
// extraction-failure outcomes. The JSON segment payload itself is pinned
// next to the arm that builds it, in `unit::payload`.
#[cfg(test)]
mod html_unit_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::markdown::parse;
    use transync_syntax::outcome::has_translatable_blocks;

    fn opts() -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        }
    }

    #[test]
    fn zero_segment_html_block_builds_no_unit() {
        let mut doc = parse("<!-- just a comment -->\n\nreal paragraph\n").expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(
            outcomes.get(&doc.blocks[0].block_id),
            Some(&HtmlOutcome::PreservedZeroSegment)
        );
        let batches = build_batches(&doc, &opts(), None, &outcomes);
        assert!(
            batches
                .iter()
                .flat_map(|b| &b.units)
                .all(|u| !u.unit_id.0.starts_with("html-")),
            "no unit for a zero-segment block"
        );
    }

    #[test]
    fn has_translatable_blocks_matches_batch_emptiness_for_html() {
        // The doc-comment contract: exactly the predicate deciding batch
        // emptiness — now per-block for html.
        for (src, expected) in [
            ("<!-- only a comment -->\n", false),
            ("<div class=\"note\">n</div>\n", true),
            ("<img src=\"a.png\">\n", false), // html img wall: zero segments
        ] {
            let mut doc = parse(src).expect("parses");
            assign_block_ids(&mut doc);
            let outcomes = html_outcomes(&doc);
            assert_eq!(
                has_translatable_blocks(&doc, &outcomes),
                expected,
                "predicate for {src:?}"
            );
            assert_eq!(
                !build_batches(&doc, &opts(), None, &outcomes).is_empty(),
                expected,
                "batches for {src:?}"
            );
        }
    }
}

// DCR-0027 SL-108: what replaced the three-gate refusal (ti 0ed6eb,
// R0001-0006's residual). `build_batches` is still the one door onto
// `render_prompt_body` a caller reaches without crossing the translate
// boundary — but it no longer refuses a section-scoped entry, it *filters* it:
// the term reaches the prompts of its own sections and no others.
#[cfg(test)]
mod section_scoped_glossary_tests {
    use super::*;
    use crate::llm::{GlossaryEntry, GlossaryScope, TranslationBatch};
    use crate::profile::ProfileMetadata;
    use crate::test_fixtures::{EventLog, record_events};
    use std::sync::Arc;

    /// Three sections plus a preamble, each carrying prose of its own.
    const SOURCE: &str = "preamble prose\n\n\
         # Guide\n\nguide prose\n\n\
         ## Tables\n\ntable prose\n\n\
         ## Prisons\n\nprison prose\n";

    fn global(source: &str, target: &str) -> GlossaryEntry {
        GlossaryEntry {
            source_term: source.to_string(),
            target_term: target.to_string(),
            note: None,
            scope: GlossaryScope::GlobalAcrossDocument,
            sections: Vec::new(),
        }
    }

    fn sectioned(source: &str, target: &str, sections: &[&str]) -> GlossaryEntry {
        GlossaryEntry {
            source_term: source.to_string(),
            target_term: target.to_string(),
            note: None,
            scope: GlossaryScope::ConditionalOnSection,
            sections: sections.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn profile_with(glossary: Vec<GlossaryEntry>) -> ProfileMetadata {
        let mut profile = default_profile();
        profile.glossary = glossary;
        profile
    }

    fn opts_with(profile: ProfileMetadata) -> TranslateOptions {
        TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        }
    }

    /// Batch `src` under `profile`, returning the batches and every message
    /// raised on the profile channel.
    fn batch_and_capture(
        src: &str,
        profile: ProfileMetadata,
    ) -> (Vec<TranslationBatch>, Vec<String>) {
        let mut doc = crate::markdown::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = opts_with(profile);
        let log = Arc::new(EventLog::default());
        let batches = {
            let _guard = record_events(Arc::clone(&log));
            build_batches(&doc, &opts, None, &html_outcomes(&doc))
        };
        (batches, log.messages_on("transync::profile"))
    }

    /// The batch holding the unit whose id starts with `prefix`.
    fn batch_of<'a>(batches: &'a [TranslationBatch], id: &str) -> &'a TranslationBatch {
        batches
            .iter()
            .find(|b| b.units.iter().any(|u| u.unit_id.0 == id))
            .unwrap_or_else(|| panic!("no batch holds {id}"))
    }

    fn targets(batch: &TranslationBatch) -> Vec<&str> {
        batch
            .glossary
            .iter()
            .map(|e| e.target_term.as_str())
            .collect()
    }

    /// THE acceptance criterion for the second half of the commission: the
    /// section-scoped term reaches its section's prompt and no other, and the
    /// global default stands everywhere it is not overridden.
    #[test]
    fn a_section_scoped_term_reaches_its_own_sections_prompt_and_no_other() {
        let (batches, said) = batch_and_capture(
            SOURCE,
            profile_with(vec![
                global("cell", "셀"),
                sectioned("cell", "감방", &["Prisons"]),
                sectioned("row", "행", &["Tables"]),
            ]),
        );

        // p-0001 preamble, h1-0002 Guide, h2-0004 Tables, h2-0006 Prisons.
        let preamble = batch_of(&batches, "p-0001");
        let guide = batch_of(&batches, "h1-0002");
        let tables = batch_of(&batches, "h2-0004");
        let prisons = batch_of(&batches, "h2-0006");

        assert_eq!(targets(preamble), vec!["셀"], "no heading encloses it");
        assert_eq!(targets(guide), vec!["셀"]);
        assert_eq!(targets(tables), vec!["셀", "행"]);
        assert_eq!(
            targets(prisons),
            vec!["감방"],
            "the section-scoped entry replaces the global rendering here"
        );

        // And the compiled prompts agree with the structured lists — the model
        // reads the bullets, not `TranslationBatch::glossary`.
        assert!(prisons.profile.prompt_body.contains("\"cell\" → \"감방\""));
        assert!(!prisons.profile.prompt_body.contains("\"cell\" → \"셀\""));
        assert!(!prisons.profile.prompt_body.contains("행"));
        assert!(tables.profile.prompt_body.contains("\"row\" → \"행\""));
        assert!(!tables.profile.prompt_body.contains("감방"));
        assert!(!guide.profile.prompt_body.contains("감방"));
        // Selectors are filtering inputs, never prompt text (G8).
        for b in &batches {
            assert!(!b.profile.prompt_body.contains("Prisons"));
            assert!(!b.profile.prompt_body.contains("section"));
        }
        assert!(said.is_empty(), "a clean profile says nothing: {said:?}");
    }

    /// The heading unit itself takes its own section's cohort. Its wire
    /// `section_path` excludes the heading that selects that cohort, so this is
    /// exactly the case the partition rule (P1) exists for.
    #[test]
    fn the_opening_heading_translates_under_the_section_it_opens() {
        let (batches, _) = batch_and_capture(
            SOURCE,
            profile_with(vec![sectioned("cell", "감방", &["Prisons"])]),
        );
        let prisons = batch_of(&batches, "h2-0006");
        assert!(
            prisons.units.iter().any(|u| u.unit_id.0 == "h2-0006"),
            "the heading is in the batch"
        );
        assert_eq!(targets(prisons), vec!["감방"]);
        assert!(
            prisons
                .units
                .iter()
                .find(|u| u.unit_id.0 == "h2-0006")
                .map(|u| u.context.section_path.iter().all(|h| h.text != "Prisons"))
                .unwrap(),
            "and its own wire path does NOT name Prisons — the cohort is the \
             partition's answer, not the path's"
        );
    }

    /// Subsection inheritance, end to end: a term scoped to the enclosing
    /// section reaches every section nested under it.
    #[test]
    fn a_term_scoped_to_an_enclosing_section_reaches_its_subsections() {
        let (batches, _) = batch_and_capture(
            SOURCE,
            profile_with(vec![sectioned("cell", "셀", &["Guide"])]),
        );
        for id in ["h1-0002", "h2-0004", "h2-0006"] {
            assert_eq!(targets(batch_of(&batches, id)), vec!["셀"], "{id}");
        }
        assert!(
            targets(batch_of(&batches, "p-0001")).is_empty(),
            "but not the preamble, which no heading encloses"
        );
    }

    /// §5: one compiled prompt per COHORT, shared by every section with that
    /// cohort — which is what keeps two sections reading one prompt (and, §6,
    /// sharing cache entries) instead of two equal-looking ones.
    #[test]
    fn sections_with_the_same_effective_glossary_share_one_compiled_prompt() {
        let (batches, _) = batch_and_capture(
            SOURCE,
            profile_with(vec![
                global("cell", "셀"),
                sectioned("row", "행", &["Tables"]),
            ]),
        );
        let bodies: std::collections::HashSet<&str> = batches
            .iter()
            .map(|b| b.profile.prompt_body.as_str())
            .collect();
        assert_eq!(
            bodies.len(),
            2,
            "two cohorts: with and without the row term"
        );
        assert_eq!(
            batch_of(&batches, "p-0001").profile.prompt_body,
            batch_of(&batches, "h2-0006").profile.prompt_body,
            "the preamble and Prisons resolve to the same glossary, so they \
             read the same bytes"
        );
    }

    /// The invisibility guarantee: a profile with no section-scoped entry
    /// compiles the byte-identical prompt it compiled before DCR-0027, for
    /// every batch — which is why no cache entry is orphaned (§6).
    #[test]
    fn a_glossary_without_section_scope_compiles_the_prompt_it_always_did() {
        let profile = profile_with(vec![global("cell", "셀"), global("row", "행")]);
        let expected = render_prompt_body(&profile, "en", "ko");
        let (batches, said) = batch_and_capture(SOURCE, profile);

        assert!(batches.len() > 1, "the fixture is sectioned");
        for b in &batches {
            assert_eq!(b.profile.prompt_body, expected.prompt_body);
            assert_eq!(targets(b), vec!["셀", "행"]);
        }
        assert!(said.is_empty(), "{said:?}");
    }

    /// G7: an entry naming a section this document does not have is advisory —
    /// the profile is document-independent — and is named once per run.
    #[test]
    fn an_entry_matching_no_section_of_this_document_is_named_once() {
        let (_, said) = batch_and_capture(
            SOURCE,
            profile_with(vec![
                sectioned("cell", "감방", &["Prisons"]),
                sectioned("row", "행", &["Nowhere", "Elsewhere"]),
            ]),
        );
        let hits: Vec<&String> = said
            .iter()
            .filter(|m| m.contains("none of which this document has"))
            .collect();
        assert_eq!(
            hits.len(),
            1,
            "once, and only for the unmatched one: {said:?}"
        );
        assert!(hits[0].starts_with("glossary[1]"), "{hits:?}");
    }

    /// G5's shadowing warning is said once per `(shadowed, winner)` pair — the
    /// caller's dedup, not the cohort memo's (DCR-0027 amended 2026-08-09).
    /// The fixture is the route difference that distinction exists for:
    /// `# Guide` resolves to the same cohort as `## Tables` while shadowing
    /// nothing, and it packs first, so a warning emitted at the cohort's first
    /// render would be lost rather than deduped (R0001-0032).
    #[test]
    fn a_shadowed_entry_is_named_once_however_many_sections_share_the_cohort() {
        // The `Guide` selector matches all three headed sections (nested
        // headings carry `Guide` on their stack); the `Tables` one matches
        // `## Tables` alone. So all three resolve to cohort `[0]`, and only
        // `## Tables` reaches it by shadowing `glossary[1]`.
        let (_, said) = batch_and_capture(
            SOURCE,
            profile_with(vec![
                sectioned("cell", "셀", &["Guide"]),
                sectioned("cell", "감방", &["Tables"]),
            ]),
        );
        let hits: Vec<&String> = said
            .iter()
            .filter(|m| m.contains("is shadowed in"))
            .collect();
        assert_eq!(hits.len(), 1, "{said:?}");
        assert!(hits[0].starts_with("glossary[1] is shadowed"), "{hits:?}");
    }

    /// The rewind rule (ti 28110f / R0001-0014) under cohorts: a caller handing
    /// in an ALREADY-COMPILED profile must not get the full glossary's section
    /// standing beside its cohort's.
    #[test]
    fn a_cohort_compiled_from_a_compiled_profile_carries_one_glossary_section() {
        let profile = profile_with(vec![
            global("cell", "셀"),
            sectioned("row", "행", &["Tables"]),
        ]);
        let compiled = render_prompt_body(&profile, "en", "ko");
        let (batches, said) = batch_and_capture(SOURCE, compiled);

        for b in &batches {
            assert_eq!(
                b.profile
                    .prompt_body
                    .lines()
                    .filter(|l| *l
                        == "Glossary (when the source term appears, prefer the target \
                                        form below; omit otherwise):")
                    .count(),
                1,
                "one glossary section per cohort: {}",
                b.profile.prompt_body
            );
        }
        assert!(
            !batch_of(&batches, "p-0001")
                .profile
                .prompt_body
                .contains("행"),
            "the preamble's cohort does not inherit the full list's bullet"
        );
        assert!(said.is_empty(), "{said:?}");
    }
}

// R0004-0076: the input-budget preflight, at the door that resolves the
// budget. The packer floors the payload target at 1, so a reserve that has
// eaten the whole input target does not fail — it packs one unit per batch and
// sends every request over the target. These pin that the run says so, once.
#[cfg(test)]
mod input_budget_preflight_tests {
    use super::*;
    use crate::profile::ProfileMetadata;
    use crate::test_fixtures::{EventLog, record_events};
    use std::sync::Arc;

    /// A preamble plus three headed sections, so the dedup below has more than
    /// one section to be tested against.
    const SOURCE: &str = "preamble prose\n\n\
         # Guide\n\nguide prose\n\n\
         ## Tables\n\ntable prose\n\n\
         ## Prisons\n\nprison prose\n";

    /// The default profile with an input target that cannot hold its own
    /// compiled prompt. `normalize_batching` refuses only a zero here, so a
    /// small positive number is a value a real profile can carry.
    fn starved(target: u32) -> ProfileMetadata {
        let mut profile = default_profile();
        profile.batching.target_input_tokens_per_batch = Some(target);
        profile
    }

    fn batch_and_capture(src: &str, profile: ProfileMetadata) -> Vec<String> {
        let mut doc = crate::markdown::parse(src).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(profile),
            ..TranslateOptions::default()
        };
        let log = Arc::new(EventLog::default());
        {
            let _guard = record_events(Arc::clone(&log));
            build_batches(&doc, &opts, None, &html_outcomes(&doc));
        }
        log.messages_on("transync::profile")
    }

    fn budget_findings(said: &[String]) -> Vec<&String> {
        said.iter()
            .filter(|m| m.contains("per-batch input reserve"))
            .collect()
    }

    /// The finding this ticket exists for: before it, this run was silent.
    #[test]
    fn a_starved_input_target_is_named_at_the_batching_door() {
        let said = batch_and_capture(SOURCE, starved(8));
        let hits = budget_findings(&said);
        assert_eq!(hits.len(), 1, "{said:?}");
        assert!(
            hits[0].contains("[batching].target_input_tokens_per_batch"),
            "the message names the knob that moves it: {hits:?}"
        );
    }

    /// Once per run, not once per section — the same exactly-once discipline
    /// every other finding at this door follows (R0001-0032). All four
    /// sections here share one cohort, so they share one reserve and one
    /// finding.
    #[test]
    fn the_finding_is_said_once_however_many_sections_the_document_has() {
        let sections = "# S\n\nprose\n\n".to_string()
            + &(1..=12)
                .map(|i| format!("## S{i}\n\nprose {i}\n\n"))
                .collect::<String>();
        let said = batch_and_capture(&sections, starved(8));
        assert_eq!(budget_findings(&said).len(), 1, "{said:?}");
    }

    /// And an ordinary profile is silent, so this cannot become a line every
    /// run prints.
    #[test]
    fn an_ordinary_profile_says_nothing() {
        let said = batch_and_capture(SOURCE, default_profile());
        assert!(budget_findings(&said).is_empty(), "{said:?}");
    }

    /// The document-shaped consequence the message describes, observed on the
    /// batches themselves: the packer's floor of 1 puts every unit in a batch
    /// of its own.
    #[test]
    fn the_starved_run_packs_one_unit_per_batch() {
        let mut doc = crate::markdown::parse(SOURCE).expect("parses");
        crate::id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            profile: Some(starved(8)),
            ..TranslateOptions::default()
        };
        let batches = build_batches(&doc, &opts, None, &html_outcomes(&doc));
        assert!(batches.len() > 1);
        assert!(
            batches.iter().all(|b| b.units.len() == 1),
            "the state the finding describes: {:?}",
            batches.iter().map(|b| b.units.len()).collect::<Vec<_>>()
        );
    }
}

// ti 490d97 wave 5 (spec §6): [system].prompt_html is selected at the
// cohort-compile door, for the run's format, before the first compile — so
// the initial body, every cohort re-compile, and the template rewind all
// inherit it, and CohortDigest's profile_prompt_hash moves with it.
#[cfg(test)]
mod prompt_html_selection_tests {
    use super::*;
    use crate::profile::load_profile;
    use crate::test_fixtures::{EventLog, record_events};
    use std::sync::Arc;

    fn html_doc() -> crate::markdown::Document {
        let mut doc =
            transync_syntax::intake::html::parse("<h1>T</h1>\n<p>alpha</p>\n<p>bravo</p>\n");
        crate::id::assign_block_ids(&mut doc);
        doc
    }

    fn md_doc() -> crate::markdown::Document {
        let mut doc = crate::markdown::parse("# T\n\nalpha\n\nbravo\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        doc
    }

    fn batches_for(
        doc: &crate::markdown::Document,
        profile: Option<crate::profile::ProfileMetadata>,
    ) -> (Vec<crate::llm::TranslationBatch>, Vec<String>) {
        let outcomes = html_outcomes(doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            profile,
            ..TranslateOptions::default()
        };
        let log = Arc::new(EventLog::default());
        let batches = {
            let _guard = record_events(Arc::clone(&log));
            build_batches(doc, &opts, None, &outcomes)
        };
        (batches, log.messages_on("transync::profile"))
    }

    /// The default profile ships a prompt_html body, and an HTML run's every
    /// batch carries ITS compiled form — which is exactly what moves
    /// CohortDigest::profile_prompt_hash between the two formats.
    #[test]
    fn an_html_run_compiles_the_html_prompt_and_a_markdown_run_does_not() {
        let (html_batches, said) = batches_for(&html_doc(), None);
        assert!(!html_batches.is_empty());
        for b in &html_batches {
            assert!(
                b.profile.prompt_body.contains("HTML documents"),
                "an HTML run's system prompt is the prompt_html body: {}",
                b.profile.prompt_body,
            );
            assert!(
                !b.profile.prompt_body.contains("GitHub Flavored Markdown"),
                "and not the Markdown one: {}",
                b.profile.prompt_body,
            );
        }
        assert!(
            said.iter().all(|m| !m.contains("prompt_html")),
            "the shipped default has the key; no advisory fires: {said:?}",
        );

        let (md_batches, _) = batches_for(&md_doc(), None);
        for b in &md_batches {
            assert!(
                b.profile.prompt_body.contains("GitHub Flavored Markdown"),
                "a Markdown run's prompt is byte-identical to before this wave: {}",
                b.profile.prompt_body,
            );
        }
    }

    /// §6: a custom profile without the key falls back to [system].prompt
    /// UNCHANGED — never a core-synthesized merge into operator-owned text —
    /// plus the advisory, emitted once, at this door, on the profile target.
    #[test]
    fn a_custom_profile_without_prompt_html_falls_back_verbatim_with_one_advisory() {
        let toml =
            "slug = \"p\"\nversion = \"1.0.0\"\n[system]\nprompt = \"Operator words only.\"\n";
        let loaded = load_profile(toml).expect("loads");
        let (batches, said) = batches_for(&html_doc(), Some(loaded));
        for b in &batches {
            assert!(
                b.profile.prompt_body.starts_with("Operator words only."),
                "the operator's prompt, verbatim at the front — no synthesized \
                 HTML addendum: {}",
                b.profile.prompt_body,
            );
        }
        let hits: Vec<&String> = said.iter().filter(|m| m.contains("prompt_html")).collect();
        assert_eq!(hits.len(), 1, "said once: {said:?}");
        assert!(
            hits[0].contains("written for Markdown"),
            "the spec's sentence: {hits:?}"
        );
    }

    /// The advisory is about a gap an HTML run actually hits: the same
    /// custom profile on a MARKDOWN run says nothing — a warning nobody can
    /// act on is a warning everybody learns to skip.
    #[test]
    fn the_advisory_never_fires_on_a_markdown_run() {
        let toml =
            "slug = \"p\"\nversion = \"1.0.0\"\n[system]\nprompt = \"Operator words only.\"\n";
        let loaded = load_profile(toml).expect("loads");
        let (_, said) = batches_for(&md_doc(), Some(loaded));
        assert!(said.iter().all(|m| !m.contains("prompt_html")), "{said:?}");
    }

    /// A custom profile WITH the key is selected like the default's, and its
    /// template variables substitute in the html body too.
    #[test]
    fn a_custom_prompt_html_is_selected_and_substituted() {
        let toml = "slug = \"p\"\nversion = \"1.0.0\"\n[system]\nprompt = \"MD body.\"\nprompt_html = \"HTML body into {{target_language}}.\"\n";
        let loaded = load_profile(toml).expect("loads");
        assert!(
            loaded
                .load_warnings
                .iter()
                .all(|w| !w.contains("prompt_html")),
            "prompt_html is a KNOWN [system] key — no unknown-key warning: {:?}",
            loaded.load_warnings,
        );
        let (batches, said) = batches_for(&html_doc(), Some(loaded));
        for b in &batches {
            assert!(
                b.profile.prompt_body.starts_with("HTML body into ko."),
                "selected and substituted: {}",
                b.profile.prompt_body,
            );
        }
        assert!(said.iter().all(|m| !m.contains("prompt_html")), "{said:?}");
    }
}
