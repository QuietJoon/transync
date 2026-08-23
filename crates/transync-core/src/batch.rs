//! Sequential token-budget batching.
//!
//! Units are packed in document order until adding the next one would
//! exceed the configured token budget, at which point a fresh batch
//! starts. A hard `max_units_per_batch` cap keeps retry granularity
//! reasonable when a document is full of tiny blocks. Each unit's token
//! count is estimated with the encoder the provider declares via
//! `Translator::tokenizer_hint`, falling back to the OpenAI-model-name
//! heuristic over `model_id` (`o200k_base` for gpt-4o / gpt-5 / o-series,
//! `cl100k_base` otherwise) when it declares none (OI-0029).
//!
//! The section logic is **one level up**, and it is real since DCR-0027:
//! `unit::section::partition_by_section` splits the unit list at every heading
//! and `unit::build_batches` calls this packer once per section, so a batch's
//! units always come from one section and no batch straddles a `##` / `###`
//! boundary. What lives here is unchanged by that — this function packs the
//! run of units it is handed, and it is the *caller* that decides those units
//! are one section's. The one thing a section can still do is exceed the
//! budget, and then it packs into several batches, all of them inside that
//! section (DCR-0027 P4). Until DCR-0027 there was no section awareness
//! anywhere, though this module was titled as if there were (EXT-2026-07 P2
//! renamed it honestly; the grouping the name promised has now landed).
//!
//! Everything that ships once per batch rather than once per unit — the
//! compiled system prompt, the assembled user-message instruction envelope,
//! and the two language labels — is *encoded* off the input target before any
//! unit is packed ([`envelope_input_tokens`]). None of it is a fixed
//! allowance: an allowance is a size assumption, and every one of those
//! strings is assembled at run time (ti aa92d6, R0001-0013, R0008-0029).
//!
//! Packing enforces TWO caps: the existing input-token budget and an
//! output-expansion budget (D1). Each unit's response size is estimated
//! as `source_tokens × output_expansion_factor` plus the echoed id and a
//! per-unit result-envelope allowance, and a batch breaks as soon as
//! either cap would be exceeded. The output cap is active only when the
//! profile sets `[batching].target_output_tokens`; when it is unset,
//! packing is bit-identical to the input-only behavior.
//!
//! TRACE: SCN-10
//! TRACE: contracts.md §5

use crate::llm::{TokenizerHint, TranslationBatch, TranslationUnit};
use crate::validate::OutputBudgetWarning;
use tiktoken_rs::CoreBPE;

/// Per-unit JSON-wrapping overhead — `{"unit_id": "...",
/// "block_kind": "...", "source_payload": "..."}` plus separators.
/// Tuned empirically; small enough that pinning it as a constant is
/// fine.
///
/// This crate *does* serialize this framing itself, in
/// `llm::prompt::build_user_prompt` — unlike the response-side constants, it is
/// not text only the model writes. It stays a tuned per-unit estimate rather
/// than an encoded string because its keys and punctuation are the same for
/// every unit: nothing about it follows the run's profile, labels, or content
/// (ti aa92d6).
const PER_UNIT_OVERHEAD_TOKENS: usize = 50;

/// Default output-to-source token-expansion factor used when the profile
/// leaves `[batching].output_expansion_factor` unset (D1). Covers the worst
/// *legitimate* case among the motivating targets (KO/JA under o200k ≈ 2.0×
/// source): under settled abort-all semantics the failure mode is asymmetric
/// — underestimating splits too little and aborts the whole run at the
/// provider, while overestimating only produces more, smaller batches (linear
/// request overhead, zero quality loss). Values `< 1.0` are legal for targets
/// that compress.
pub const DEFAULT_OUTPUT_EXPANSION_FACTOR: f64 = 2.0;

/// Per-unit result-envelope overhead on the OUTPUT side: the echoed result
/// object keys/punctuation (`{"unit_id": …, "output_kind": …,
/// "translated_payload": …, "warnings": []}`) plus JSON string-escaping slack
/// (`\n`, `\"` in structure-heavy payloads). The output-side analogue of
/// [`PER_UNIT_OVERHEAD_TOKENS`]; tuned constant, pinned (D1).
const PER_UNIT_OUTPUT_OVERHEAD_TOKENS: usize = 40;

/// Fixed per-batch reserve on the OUTPUT side: the top-level result framing
/// (`{"detected_source_language": …, "units": [ … ]}`). The output-side
/// counterpart of the input envelope [`envelope_input_tokens`] measures,
/// subtracted from the raw ceiling to derive the effective output target (D1).
/// A constant rather than an encoded string because nothing on the response
/// side is assembled by this crate — the model writes it.
///
/// It is also the floor a usable ceiling must clear: a
/// `[batching].target_output_tokens` at or below it leaves no room for a
/// single unit's answer, which is why `profile::normalize_batching` refuses
/// one (R0003-0034).
pub(crate) const OUTPUT_ENVELOPE_RESERVE_TOKENS: usize = 64;

/// THE effective per-batch output target: the raw ceiling minus the fixed
/// response-envelope reserve, floored at 1.
///
/// Three sites need this number and must not disagree about it — the packer
/// ([`group_by_token_budget`]), the preflight ([`output_budget_warnings`]),
/// and the row-window splitter (`unit::split`, DCR-0026). "Oversize" is
/// defined by this function and by [`estimate_payload_output_tokens`]
/// together; a private fourth copy of either is the bug class DCR-0026 rule 4
/// forbids.
///
/// TRACE: DCR-0026
pub(crate) fn effective_output_target(raw_ceiling: usize) -> usize {
    raw_ceiling
        .saturating_sub(OUTPUT_ENVELOPE_RESERVE_TOKENS)
        .max(1)
}

/// Dual token budget handed to [`group_by_token_budget`] (D1). Bundles the
/// existing input cap with the new output-expansion cap so a single caller
/// (`unit::build_batches`) resolves both from the profile.
pub struct BatchBudget<'a> {
    /// Soft cap on per-batch INPUT tokens (the pre-D1 `target_tokens_per_batch`).
    pub target_input_tokens: usize,
    /// Hard cap on units per batch regardless of token count.
    pub max_units: usize,
    /// Per-batch OUTPUT ceiling (raw profile `target_output_tokens`).
    /// `None` disables output-aware packing entirely — packing is then
    /// bit-identical to the input-only behavior and no [`OutputBudgetWarning`]
    /// is ever produced.
    pub target_output_tokens: Option<usize>,
    /// Output-to-source expansion factor applied to each unit's payload when
    /// estimating its response size. Already resolved (see
    /// [`resolve_expansion_factor`]); ignored when `target_output_tokens` is
    /// `None`.
    pub output_expansion_factor: f64,
    /// Model id, used to select the tiktoken encoder when
    /// [`BatchBudget::tokenizer_hint`] is `None` (see [`encoder_for`]).
    pub model: &'a str,
    /// Provider-declared encoder hint (`Translator::tokenizer_hint`).
    /// `Some` wins over the [`BatchBudget::model`] name heuristic;
    /// `None` keeps the legacy OpenAI-model-shaped fallback (OI-0029).
    pub tokenizer_hint: Option<TokenizerHint>,
    /// Compiled system prompt (incl. rendered glossary) whose encoded length
    /// is reserved off the input target once per batch (R0008-0029).
    pub system_prompt: &'a str,
    /// The constant part of the user-message envelope this run will send —
    /// the assembled instruction plus the JSON framing around it — as
    /// `llm::prompt::instruction_envelope_json` produces it for the run's
    /// instruction variant. Encoded off the input target once per batch, like
    /// [`BatchBudget::system_prompt`].
    ///
    /// ti aa92d6: this used to be a fixed 256-token allowance. The real
    /// instruction outgrew it — the unconditional part alone is longer, and up
    /// to three conditional clauses stack on top — and the error ran the unsafe
    /// way, leaving the packer believing it had room it did not have. The
    /// clause that depends on which units land in a batch (the html-segment
    /// contract) is reserved on a document-level upper bound; see
    /// `llm::prompt::InstructionVariant`.
    pub instruction_envelope: &'a str,
    /// Source-language label as it ships in the user-prompt envelope
    /// (`TranslationBatch::source_language`). ADR-0013 makes these labels
    /// opaque caller-supplied strings of *any* length, so the envelope
    /// estimate encodes them instead of folding them into a fixed allowance
    /// (R0001-0013) — the same rule the instruction now follows.
    pub source_language: &'a str,
    /// Target-language label, under the same rule as
    /// [`BatchBudget::source_language`].
    pub target_language: &'a str,
}

/// Tokens reserved off the raw input target once per batch: the constant
/// user-message envelope ([`BatchBudget::instruction_envelope`] — the
/// assembled instruction plus its JSON framing, ti aa92d6), the compiled
/// system prompt (incl. rendered glossary, R0008-0029) and the two language
/// labels (R0001-0013).
///
/// Every one of them is *encoded*, not allowed for: nothing in this
/// once-per-batch reserve is a constant standing in for a string the crate can
/// produce. What is left to a constant is fixed-shape framing whose size does
/// not follow the run: the per-unit input JSON framing
/// ([`PER_UNIT_OVERHEAD_TOKENS`]) — which this crate does serialize, but
/// identically for every unit — and the response-side envelope
/// ([`OUTPUT_ENVELOPE_RESERVE_TOKENS`]), which only the model writes. The
/// containment test
/// `instruction_reserve_tests::the_reserve_plus_the_per_unit_estimates_bound_the_whole_request`
/// pins that this reserve plus the per-unit estimates still bound the whole
/// assembled request.
///
/// Split out of [`group_by_token_budget`] so the retry packer in
/// `pipeline::dispatch` and the tests can ask for the same number the packer
/// runs with, rather than re-deriving it.
///
/// TRACE: SCN-10
pub(crate) fn envelope_input_tokens(budget: &BatchBudget<'_>, bpe: &CoreBPE) -> usize {
    // R0003-0032: a label lands *inside* the user-prompt JSON, so what it
    // costs is its escaped body — the envelope already holds the two keys and
    // their quotes. Identity for an ordinary label; longer for one carrying a
    // quote, a backslash or a control character, which ADR-0013 permits.
    let source_label = crate::llm::prompt::label_wire_text(budget.source_language);
    let target_label = crate::llm::prompt::label_wire_text(budget.target_language);
    let mut tokens = 0;
    for text in [
        budget.instruction_envelope,
        budget.system_prompt,
        source_label.as_str(),
        target_label.as_str(),
    ] {
        if !text.is_empty() {
            tokens += bpe.encode_with_special_tokens(text).len();
        }
    }
    tokens
}

/// Group units into batches honoring a dual token budget + a hard unit cap.
///
/// Both an INPUT-token cap and (when the profile sets a ceiling) an
/// OUTPUT-expansion cap apply: a batch breaks before the next unit as soon
/// as adding it would push EITHER running total past its target, or the
/// unit cap is hit — whichever binds first (D1). Empty input yields no
/// batches. A unit whose own estimated cost exceeds either target still
/// gets its own batch (rather than being dropped); [`output_budget_warnings`]
/// names the over-ceiling ones so the run can be diagnosed instead of just
/// aborting at the provider.
///
/// When [`BatchBudget::target_output_tokens`] is `None` the output cap is
/// inert and packing is bit-identical to the pre-D1 input-only behavior.
///
/// TRACE: SCN-10
pub fn group_by_token_budget(
    units: Vec<TranslationUnit>,
    budget: &BatchBudget<'_>,
) -> Vec<Vec<TranslationUnit>> {
    let bpe = resolve_encoder(budget.tokenizer_hint, budget.model);
    group_by_token_budget_with(units, budget, &bpe)
}

/// [`group_by_token_budget`] over an encoder the caller already resolved.
///
/// DCR-0027 made the packer run **once per section** rather than once per run,
/// and building a `CoreBPE` decodes a bundled rank table — a cost that belongs
/// to the run, not to each of a heading-rich document's sections. The two
/// entry points resolve the encoder through the same [`resolve_encoder`], so a
/// section-packed run and a retry round still measure with the same tokenizer.
///
/// TRACE: DCR-0027
pub(crate) fn group_by_token_budget_with(
    units: Vec<TranslationUnit>,
    budget: &BatchBudget<'_>,
    bpe: &CoreBPE,
) -> Vec<Vec<TranslationUnit>> {
    if units.is_empty() {
        return Vec::new();
    }
    let max_units = budget.max_units.max(1);
    // R0008-0029 / R0001-0013 / ti aa92d6: the system prompt (including any
    // rendered glossary), the assembled instruction envelope, and the two
    // language labels ship with EVERY batch. Reserve room for them so a large
    // profile/glossary — or a long opaque language label, or an instruction
    // carrying every conditional clause — can't push a batch past the provider
    // context window even when the per-unit estimates fit the raw budget.
    let envelope_tokens = envelope_input_tokens(budget, bpe);
    let input_target = budget
        .target_input_tokens
        .saturating_sub(envelope_tokens)
        .max(1);
    // D1: reserve the top-level result framing off the raw output ceiling to
    // get the effective per-batch output target. `None` leaves the output
    // cap inert (input-only packing).
    let output_target = budget.target_output_tokens.map(effective_output_target);

    let mut batches: Vec<Vec<TranslationUnit>> = Vec::new();
    let mut current: Vec<TranslationUnit> = Vec::new();
    let mut in_sum: usize = 0;
    let mut out_sum: usize = 0;

    for unit in units {
        let est = estimate_unit(&unit, bpe, budget.output_expansion_factor);

        let would_exceed_input = !current.is_empty() && in_sum + est.input > input_target;
        // R0003-0029: the output axis is the one carrying a caller-supplied
        // float multiplier, so unlike `in_sum` — bounded by the bytes of units
        // that are all alive in memory at once — this sum can be pushed to the
        // top of `usize` by an absurd but finite `output_expansion_factor`.
        // Saturating there says "does not fit", which is both true and the
        // behavior the packer already has for a single over-ceiling unit.
        let would_exceed_output = !current.is_empty()
            && output_target.is_some_and(|target| out_sum.saturating_add(est.output) > target);
        let would_overfill = current.len() >= max_units;
        if would_exceed_input || would_exceed_output || would_overfill {
            batches.push(std::mem::take(&mut current));
            in_sum = 0;
            out_sum = 0;
        }

        current.push(unit);
        in_sum += est.input;
        out_sum = out_sum.saturating_add(est.output);
    }

    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

/// Input + estimated-output token cost of one unit, computed in a single
/// encoder pass. Encoding is the hot cost, so `source_payload` and `unit_id`
/// are each encoded exactly once and reused for both sides (D1).
struct UnitEstimate {
    input: usize,
    output: usize,
}

/// Estimate both the input and the response-side token cost of a unit in one
/// pass.
///
/// Input side (unchanged from pre-D1): the system prompt + glossary the
/// provider also sees, and the assembled instruction envelope, are fixed per
/// batch, not per unit; [`group_by_token_budget`] reserves room for them up
/// front (R0008-0029, ti aa92d6) so they are not double counted here.
/// R0006-0027: the per-unit `context` hints
/// (document title, section path, two neighbor snippets — each a kind label
/// plus a summary since R0001-0024) ship with every request and can add
/// hundreds of tokens on heading-rich documents, so they are encoded too —
/// and, since R0003-0031, from the wire JSON
/// ([`crate::llm::prompt::context_hint_json`]) rather than from a
/// newline-joined concatenation that priced neither the field names nor the
/// escaping a quote-bearing heading picks up.
/// R0001-0012 / R0001-0013: the two remaining advisory fields — the
/// `constraints` hints and the ADR-0009 `retry` side channel — follow the same
/// rule, encoded from the very JSON `llm::prompt` puts on the wire
/// ([`crate::llm::prompt::advisory_hint_json`]) rather than approximated by
/// per-entry constants. That matters most on a **retry** round, where every
/// re-dispatched unit carries a rejection reason of up to 512 bytes plus a
/// truncation marker that the original packing never counted.
///
/// Output side (D1): the provider echoes `unit_id` byte-for-byte and returns
/// the translated payload (source × `output_expansion_factor`), plus the
/// per-unit result-object framing ([`PER_UNIT_OUTPUT_OVERHEAD_TOKENS`]).
/// Context hints, constraints, and the retry channel are input-only (never
/// echoed) and are excluded. A saturating float→int cast pins a non-finite /
/// negative factor to 0 expansion rather than panicking — callers
/// pre-validate, this is defense-in-depth. R0003-0030: the sum built on top of
/// that cast saturates too, so a *huge but finite* factor (which
/// `resolve_expansion_factor` accepts — it only rejects non-finite and
/// non-positive) reports "larger than any budget" instead of overflowing.
///
/// TRACE: SCN-10
fn estimate_unit(
    unit: &TranslationUnit,
    bpe: &CoreBPE,
    output_expansion_factor: f64,
) -> UnitEstimate {
    let payload_tokens = bpe.encode_with_special_tokens(&unit.source_payload).len();
    let id_tokens = bpe.encode_with_special_tokens(&unit.unit_id.0).len();
    let kind_tokens = bpe
        .encode_with_special_tokens(unit.block_kind.wire_str())
        .len();

    // R0003-0031: the `context` object, encoded from the exact JSON
    // `llm::prompt` puts on the wire — keys, heading levels, neighbor kinds
    // and escaping included — rather than from a newline-joined concatenation
    // of the same strings. A title or neighbor summary heavy in `"`, `\` or
    // newlines is longer on the wire than in memory, and the per-unit framing
    // constant below was never scoped to cover that difference.
    let context_json = crate::llm::prompt::context_hint_json(unit);
    let context_tokens = if context_json.is_empty() {
        0
    } else {
        bpe.encode_with_special_tokens(&context_json).len()
    };

    // The `constraints` + `retry` hints, encoded from the exact JSON the user
    // prompt carries (R0001-0012 / R0001-0013). Empty for a first-dispatch
    // unit with no structural constraints, which is the common case.
    let advisory_json = crate::llm::prompt::advisory_hint_json(unit);
    let advisory_tokens = if advisory_json.is_empty() {
        0
    } else {
        bpe.encode_with_special_tokens(&advisory_json).len()
    };

    let input = payload_tokens
        + id_tokens
        + kind_tokens
        + context_tokens
        + advisory_tokens
        + PER_UNIT_OVERHEAD_TOKENS;

    UnitEstimate {
        input,
        output: output_estimate(payload_tokens, id_tokens, output_expansion_factor),
    }
}

/// THE output-side formula, from an already-encoded payload and unit id:
/// `payload x factor`, plus the echoed id, plus the per-unit result-object
/// framing.
///
/// R0003-0030: the float->int cast saturates, and the additions after it must
/// too — a factor large enough to saturate the cast would otherwise overflow
/// on the very next line, which is the panic the cast's own comment claims to
/// have avoided.
fn output_estimate(payload_tokens: usize, id_tokens: usize, factor: f64) -> usize {
    let expanded_payload = (payload_tokens as f64 * factor.max(0.0)).ceil() as usize;
    expanded_payload
        .saturating_add(id_tokens)
        .saturating_add(PER_UNIT_OUTPUT_OVERHEAD_TOKENS)
}

/// The response-side cost of a payload that will ship under `unit_id`, for a
/// unit that does not exist yet.
///
/// The row-window splitter (`unit::split`, DCR-0026) sizes candidate windows
/// with this: it is measuring a payload it is still assembling, so it has no
/// [`TranslationUnit`] to hand [`estimate_unit_output_tokens`]. Both routes go
/// through [`output_estimate`], so a window the splitter calls "fits" is one
/// the packer and the preflight also call fitting — DCR-0026 rule 4.
///
/// TRACE: DCR-0026
pub(crate) fn estimate_payload_output_tokens(
    payload: &str,
    unit_id: &str,
    bpe: &CoreBPE,
    output_expansion_factor: f64,
) -> usize {
    output_estimate(
        bpe.encode_with_special_tokens(payload).len(),
        bpe.encode_with_special_tokens(unit_id).len(),
        output_expansion_factor,
    )
}

/// Estimate the input-token cost of a single unit when wrapped in the
/// per-batch JSON envelope. Delegates to [`estimate_unit`]; the input
/// estimate is independent of the output-expansion factor.
///
/// Test-only since OI-0027: the packer itself calls [`estimate_unit`]
/// directly for both axes, and with `batch` now `pub(crate)` the only
/// remaining callers are this file's own tests.
///
/// TRACE: SCN-10
#[cfg(test)]
pub fn estimate_unit_tokens(unit: &TranslationUnit, bpe: &CoreBPE) -> usize {
    estimate_unit(unit, bpe, DEFAULT_OUTPUT_EXPANSION_FACTOR).input
}

/// Estimate the response-side token cost of a single unit under the given
/// expansion factor (D1). Used by the packer's output cap and by
/// [`output_budget_warnings`].
///
/// TRACE: SCN-10
pub fn estimate_unit_output_tokens(
    unit: &TranslationUnit,
    bpe: &CoreBPE,
    output_expansion_factor: f64,
) -> usize {
    estimate_unit(unit, bpe, output_expansion_factor).output
}

/// Resolve a profile's optional expansion factor to a usable value: a finite,
/// strictly-positive number is honored; anything else — unset, or an invalid
/// value that slipped past [`crate::profile::load_profile`]'s sanity check —
/// falls back to [`DEFAULT_OUTPUT_EXPANSION_FACTOR`] (D1).
pub(crate) fn resolve_expansion_factor(raw: Option<f64>) -> f64 {
    raw.filter(|f| f.is_finite() && *f > 0.0)
        .unwrap_or(DEFAULT_OUTPUT_EXPANSION_FACTOR)
}

/// Flag every unit whose estimated response size exceeds the per-batch output
/// ceiling (D1 §2.3). Reads the ceiling + expansion factor from each batch's
/// `profile.batching` (uniform per run), re-derives the effective output
/// target the same way [`group_by_token_budget`] does, and returns one
/// [`OutputBudgetWarning`] per over-ceiling unit. Returns empty when the
/// ceiling is unset (`target_output_tokens: None`).
///
/// `tokenizer_hint` must be the same value the packer ran with (the
/// pipeline forwards `Translator::tokenizer_hint()` to both) so the
/// preflight estimates and the packing decisions agree (OI-0029).
///
/// TRACE: SCN-10
pub fn output_budget_warnings(
    batches: &[TranslationBatch],
    model: &str,
    tokenizer_hint: Option<TokenizerHint>,
) -> Vec<OutputBudgetWarning> {
    let mut warnings = Vec::new();
    if batches.is_empty() {
        return warnings;
    }
    let bpe = resolve_encoder(tokenizer_hint, model);
    for batch in batches {
        let Some(ceiling) = batch.profile.batching.target_output_tokens else {
            continue;
        };
        let factor = resolve_expansion_factor(batch.profile.batching.output_expansion_factor);
        let output_target = effective_output_target(ceiling as usize);
        for unit in &batch.units {
            let est_out = estimate_unit_output_tokens(unit, &bpe, factor);
            if est_out > output_target {
                warnings.push(OutputBudgetWarning {
                    unit_id: unit.unit_id.clone(),
                    estimated_output_tokens: est_out.min(u32::MAX as usize) as u32,
                    ceiling_tokens: ceiling,
                });
            }
        }
    }
    warnings
}

/// A run whose fixed per-batch input reserve leaves no room for any unit
/// (R0004-0076) — the input-side counterpart of
/// [`OutputBudgetWarning`](crate::validate::OutputBudgetWarning).
///
/// Not a [`ValidationReport`](crate::validate::ValidationReport) field, unlike
/// its output-side twin, and the asymmetry is the one between the two axes:
/// the output warning names *units*, one per over-ceiling block, and a caller
/// reading the report needs the list. This one names the run's own
/// configuration — the same class as the `[batching]` and glossary diagnostics
/// `unit::build_batches` already raises on the `transync::profile` channel, and
/// it is raised at the same door, so a direct batching caller hears it too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputBudgetWarning {
    /// What [`envelope_input_tokens`] measured for this budget.
    envelope_tokens: usize,
    /// The raw `target_input_tokens` the reserve is taken out of.
    target_tokens: usize,
}

impl std::fmt::Display for InputBudgetWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "the per-batch input reserve is ~{} tokens (system prompt, instruction envelope and \
             language labels) but the per-batch input target is {}; nothing is left for the units \
             themselves, so every unit is packed alone and every request exceeds the target — \
             raise [batching].target_input_tokens_per_batch or shorten the profile's prompt and \
             glossary",
            self.envelope_tokens, self.target_tokens
        )
    }
}

/// Flag a budget whose once-per-batch input reserve has eaten the whole input
/// target (R0004-0076): `envelope_input_tokens >= target_input_tokens`.
///
/// The preflight the input axis was missing. [`group_by_token_budget_with`]
/// derives its payload target as `target - envelope`, floored at 1, so this
/// condition does not fail the run — it silently collapses packing to one unit
/// per batch, with every request over the target the reserve exists to defend.
/// A warning rather than a refusal because the input target is a documented
/// SOFT cap ([`BatchBudget::target_input_tokens`]) and a single over-budget
/// unit deliberately still ships; what was missing is the diagnosis, not an
/// abort.
///
/// `>=`, not `>`: a reserve that exactly equals the target leaves zero payload
/// capacity, which is the same degenerate state as exceeding it.
///
/// TRACE: SCN-10
/// TRACE: R0004-0076
pub(crate) fn input_budget_warning(
    budget: &BatchBudget<'_>,
    bpe: &CoreBPE,
) -> Option<InputBudgetWarning> {
    let envelope_tokens = envelope_input_tokens(budget, bpe);
    (envelope_tokens >= budget.target_input_tokens).then_some(InputBudgetWarning {
        envelope_tokens,
        target_tokens: budget.target_input_tokens,
    })
}

/// Load the encoder a [`TokenizerHint`] names. Panics only if the bundled
/// tiktoken assets are broken (same failure mode as [`encoder_for`]).
///
/// TRACE: SCN-10
/// TRACE: OI-0029
pub fn encoder_for_hint(hint: TokenizerHint) -> CoreBPE {
    let primary = match hint {
        TokenizerHint::O200kBase => {
            tiktoken_rs::o200k_base().or_else(|_| tiktoken_rs::cl100k_base())
        }
        TokenizerHint::Cl100kBase => tiktoken_rs::cl100k_base(),
    };
    primary.expect(
        "tiktoken encoders (cl100k_base and o200k_base) both failed to load — \
         bundled assets are corrupt or the tiktoken-rs crate is misbuilt",
    )
}

/// Resolve the encoder for a run: a provider-declared [`TokenizerHint`]
/// wins; otherwise fall back to the OpenAI-model-name heuristic over
/// `model_id` ([`encoder_for`]).
///
/// This is the single place the "hint beats heuristic" rule lives, so the
/// packer and the output-budget preflight can never disagree.
///
/// TRACE: SCN-10
/// TRACE: OI-0029
pub fn resolve_encoder(hint: Option<TokenizerHint>, model_id: &str) -> CoreBPE {
    hint.map(encoder_for_hint)
        .unwrap_or_else(|| encoder_for(model_id))
}

/// Pick the right tiktoken encoder for the configured model. Falls
/// back to `cl100k_base` (GPT-4 / GPT-3.5 era) for unknown identifiers.
/// If both bundled encoders fail to load, panics with a clear
/// "build asset is broken" message — there's no useful recovery
/// because we can't construct a `CoreBPE` without a vocab.
///
/// OpenAI-model-shaped **legacy heuristic**: used only when no
/// [`TokenizerHint`] is supplied (OI-0029). A provider whose model names
/// are not OpenAI-shaped should override `Translator::tokenizer_hint`
/// rather than rely on this mapping, which silently resolves every
/// unrecognized name to `cl100k_base`.
///
/// TRACE: SCN-10
pub fn encoder_for(model: &str) -> CoreBPE {
    let m = model.to_ascii_lowercase();
    let is_o200k = m.starts_with("gpt-4o")
        || m.starts_with("gpt-5")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4");
    let primary = if is_o200k {
        tiktoken_rs::o200k_base().or_else(|_| tiktoken_rs::cl100k_base())
    } else {
        tiktoken_rs::cl100k_base()
    };
    primary.expect(
        "tiktoken encoders (cl100k_base and o200k_base) both failed to load — \
         bundled assets are corrupt or the tiktoken-rs crate is misbuilt",
    )
}

// ti aa92d6: the per-batch instruction is ENCODED, not allowed for. It was a
// flat 256-token constant that the real instruction had outgrown — and the
// error ran the unsafe way, leaving the packer believing it had room it did
// not have.
#[cfg(test)]
mod instruction_reserve_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::prompt::{
        DocumentFacts, InstructionVariant, build_user_prompt, instruction_envelope_json,
    };
    use crate::llm::{BatchId, BlockConstraints, BlockContext, HtmlSegmentConstraints, InputMode};
    use crate::profile::{default_profile, render_prompt_body};

    const MODEL: &str = "gpt-4o";
    /// The allowance this reserve used to be: one constant standing for the
    /// whole assembled instruction, whatever it said.
    const OLD_FIXED_ALLOWANCE: usize = 256;

    fn paragraph(id: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "the quick brown fox jumps over the lazy dog".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    /// A raw-HTML unit as `unit::payload::assemble` builds one: the payload is
    /// the extracted text segments, never the markup.
    fn html(id: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Html,
            input_mode: InputMode::HtmlSegments,
            source_payload: "[\"Click\",\"here\"]".to_string(),
            constraints: BlockConstraints {
                html: Some(HtmlSegmentConstraints {
                    segment_count: 2,
                    segment_labels: vec!["summary".to_string(), "a".to_string()],
                    source_bytes: "<summary>Click<a>here</a></summary>".to_string(),
                    block_type: 6,
                }),
                ..BlockConstraints::default()
            },
            ..paragraph(id)
        }
    }

    /// One batch under the **default** profile, compiled exactly as
    /// `unit::build_batches` compiles it.
    fn batch_of(units: Vec<TranslationUnit>) -> TranslationBatch {
        let profile = render_prompt_body(&default_profile(), "en", "ko");
        TranslationBatch {
            batch_id: BatchId::new(1),
            units,
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: profile.glossary.clone(),
            profile,
        }
    }

    /// The envelope `unit::build_batches` reserves for a document whose units
    /// are `units` — profile clauses from the profile, html clause from the
    /// document-level verdict (owner decision 2026-08-06).
    fn envelope_for(batch: &TranslationBatch) -> String {
        instruction_envelope_json(InstructionVariant::for_run(
            &batch.profile.constraints,
            DocumentFacts::of(&batch.units),
        ))
    }

    fn budget_for<'a>(batch: &'a TranslationBatch, envelope: &'a str) -> BatchBudget<'a> {
        BatchBudget {
            target_input_tokens: 1_000_000,
            max_units: 100_000,
            target_output_tokens: None,
            output_expansion_factor: DEFAULT_OUTPUT_EXPANSION_FACTOR,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt: &batch.profile.prompt_body,
            instruction_envelope: envelope,
            source_language: &batch.source_language,
            target_language: &batch.target_language,
        }
    }

    /// Tokens the request actually spends on the instruction — read back off
    /// the built prompt, not off any constant this module holds.
    fn instruction_tokens(batch: &TranslationBatch, bpe: &CoreBPE) -> usize {
        let prompt = build_user_prompt(batch).expect("prompt builds");
        let v: serde_json::Value = serde_json::from_str(&prompt).expect("prompt is JSON");
        let instruction = v["instruction"].as_str().expect("instruction string");
        bpe.encode_with_special_tokens(instruction).len()
    }

    /// THE acceptance test. For a representative default-profile batch — with
    /// and without a raw-HTML unit — the reserve covers what
    /// `build_user_prompt` spends on the instruction, and the old constant did
    /// not.
    #[test]
    fn the_reserve_covers_the_instruction_the_request_spends() {
        let bpe = encoder_for(MODEL);
        for units in [
            vec![paragraph("p-0001")],
            vec![paragraph("p-0001"), html("html-0009")],
        ] {
            let batch = batch_of(units);
            let envelope = envelope_for(&batch);
            let reserve = envelope_input_tokens(&budget_for(&batch, &envelope), &bpe);
            let spent = instruction_tokens(&batch, &bpe);

            assert!(
                spent > OLD_FIXED_ALLOWANCE,
                "the instruction the default profile ships is longer than the allowance it \
                 used to get: {spent} vs {OLD_FIXED_ALLOWANCE}"
            );
            // The reserve covers the instruction AND the two other strings that
            // ride with every batch, which is what the packer subtracts.
            let others = bpe
                .encode_with_special_tokens(&batch.profile.prompt_body)
                .len()
                + bpe.encode_with_special_tokens(&batch.source_language).len()
                + bpe.encode_with_special_tokens(&batch.target_language).len();
            assert!(
                reserve >= spent + others,
                "reserve {reserve} must cover the instruction ({spent}) plus the system prompt \
                 and labels ({others})"
            );
        }
    }

    /// The reserve covers the WHOLE request, not only its instruction: the
    /// packer's per-unit estimates plus the envelope bound what
    /// `build_user_prompt` actually emits, html unit included.
    #[test]
    fn the_reserve_plus_the_per_unit_estimates_bound_the_whole_request() {
        let bpe = encoder_for(MODEL);
        let batch = batch_of(vec![
            paragraph("p-0001"),
            paragraph("p-0002"),
            html("html-0009"),
        ]);
        let envelope = envelope_for(&batch);
        let budget = budget_for(&batch, &envelope);
        let estimated = envelope_input_tokens(&budget, &bpe)
            + batch
                .units
                .iter()
                .map(|u| estimate_unit(u, &bpe, DEFAULT_OUTPUT_EXPANSION_FACTOR).input)
                .sum::<usize>();
        let actual = bpe
            .encode_with_special_tokens(&build_user_prompt(&batch).expect("prompt builds"))
            .len();
        assert!(
            estimated >= actual,
            "the packer must never believe a batch is smaller than the request it becomes: \
             estimated {estimated} < actual {actual}"
        );
    }

    /// The document-level html verdict is a *cost*, and the packer pays it:
    /// the same units under the same raw target fit one batch when the
    /// document is html-free and no longer do when it is not.
    #[test]
    fn the_document_level_html_verdict_shrinks_the_batches() {
        let bpe = encoder_for(MODEL);
        let units: Vec<TranslationUnit> =
            (1..=6).map(|i| paragraph(&format!("p-{i:04}"))).collect();
        let batch = batch_of(units.clone());

        let markdown_only = envelope_for(&batch);
        let with_html = instruction_envelope_json(InstructionVariant::for_run(
            &batch.profile.constraints,
            DocumentFacts {
                has_html_unit: true,
                ..DocumentFacts::default()
            },
        ));
        assert!(
            with_html.len() > markdown_only.len(),
            "the html clause is real text with a real cost"
        );

        // A target that exactly holds the six units under the html-free
        // reserve — zero slack, the honest worst case.
        let lean = budget_for(&batch, &markdown_only);
        let target = envelope_input_tokens(&lean, &bpe)
            + units
                .iter()
                .map(|u| estimate_unit_tokens(u, &bpe))
                .sum::<usize>();

        let mut lean = budget_for(&batch, &markdown_only);
        lean.target_input_tokens = target;
        let mut fat = budget_for(&batch, &with_html);
        fat.target_input_tokens = target;

        assert_eq!(
            group_by_token_budget(units.clone(), &lean).len(),
            1,
            "an html-free document pays nothing extra and packs as before"
        );
        assert!(
            group_by_token_budget(units, &fat).len() > 1,
            "a document holding an html unit reserves its clause, and packing feels it"
        );
    }
}

// R0003-0031 / R0003-0032: the packer counts the WIRE form of the last two
// approximated inputs — the per-unit `context` object and the two opaque
// language labels. Both are source-derived or caller-supplied text that JSON
// escaping can inflate well past its in-memory length, and the per-unit
// framing constant was never scoped to absorb that difference.
#[cfg(test)]
mod wire_fidelity_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::prompt::{
        DocumentFacts, InstructionVariant, build_user_prompt, instruction_envelope_json,
    };
    use crate::llm::{
        BatchId, BlockConstraints, BlockContext, HeadingSnippet, InputMode, NeighborSnippet,
    };
    use crate::profile::{default_profile, render_prompt_body};

    const MODEL: &str = "gpt-4o";

    /// Text a Markdown document may legitimately carry in a heading, a title
    /// or a neighbor summary, and whose JSON form is far longer than its
    /// bytes: every character here escapes to two, and the newline to two as
    /// well.
    fn escape_heavy(reps: usize) -> String {
        "\"\\\n".repeat(reps)
    }

    fn unit_with_context(id: &str, ctx: BlockContext) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "the quick brown fox jumps over the lazy dog".to_string(),
            context: ctx,
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    /// A quote/backslash/newline-heavy `BlockContext`, filled on every field
    /// the wire carries.
    fn heavy_context() -> BlockContext {
        BlockContext {
            document_title: Some(escape_heavy(40)),
            section_path: vec![
                HeadingSnippet {
                    level: 1,
                    text: escape_heavy(40),
                },
                HeadingSnippet {
                    level: 3,
                    text: escape_heavy(40),
                },
            ],
            preceding_block: Some(NeighborSnippet {
                kind: BlockKind::Paragraph,
                summary: escape_heavy(40),
            }),
            following_block: Some(NeighborSnippet {
                kind: BlockKind::CodeBlock {
                    info: None,
                    fenced: true,
                },
                summary: escape_heavy(40),
            }),
        }
    }

    fn batch_of(units: Vec<TranslationUnit>, src: &str, tgt: &str) -> TranslationBatch {
        let profile = render_prompt_body(&default_profile(), src, tgt);
        TranslationBatch {
            batch_id: BatchId::new(1),
            units,
            source_language: src.to_string(),
            target_language: tgt.to_string(),
            glossary: profile.glossary.clone(),
            profile,
        }
    }

    fn budget_for<'a>(batch: &'a TranslationBatch, envelope: &'a str) -> BatchBudget<'a> {
        BatchBudget {
            target_input_tokens: 1_000_000,
            max_units: 100_000,
            target_output_tokens: None,
            output_expansion_factor: DEFAULT_OUTPUT_EXPANSION_FACTOR,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt: &batch.profile.prompt_body,
            instruction_envelope: envelope,
            source_language: &batch.source_language,
            target_language: &batch.target_language,
        }
    }

    /// THE discriminating test for R0003-0031 + R0003-0032. The containment
    /// property the module already claims — reserve + per-unit estimates bound
    /// the assembled request — held only for a quote-free fixture. Give the
    /// units escaping-heavy context and the run escaping-heavy opaque labels
    /// and it breaks: the estimator encoded the raw strings while
    /// `build_user_prompt` ships their JSON form, so the packer believed it had
    /// room it did not have.
    #[test]
    fn escaping_heavy_context_and_labels_stay_inside_the_estimate() {
        let bpe = encoder_for(MODEL);
        let batch = batch_of(
            vec![
                unit_with_context("p-0001", heavy_context()),
                unit_with_context("p-0002", heavy_context()),
            ],
            &escape_heavy(20),
            &escape_heavy(20),
        );
        let envelope = instruction_envelope_json(InstructionVariant::for_run(
            &batch.profile.constraints,
            DocumentFacts::of(&batch.units),
        ));
        let budget = budget_for(&batch, &envelope);
        let estimated = envelope_input_tokens(&budget, &bpe)
            + batch
                .units
                .iter()
                .map(|u| estimate_unit(u, &bpe, DEFAULT_OUTPUT_EXPANSION_FACTOR).input)
                .sum::<usize>();
        let actual = bpe
            .encode_with_special_tokens(&build_user_prompt(&batch).expect("prompt builds"))
            .len();
        assert!(
            estimated >= actual,
            "the packer must never believe a batch is smaller than the request it becomes, \
             escaping included: estimated {estimated} < actual {actual}"
        );
    }

    /// What a unit's context adds to its estimate is at least what that
    /// context costs on the wire. Before R0003-0031 the estimator charged the
    /// newline-joined raw strings, which for an escaping-heavy heading is
    /// strictly cheaper than the `context` object the request carries — the
    /// per-unit framing constant was the only thing standing between that gap
    /// and an over-budget request.
    #[test]
    fn a_contexts_cost_is_at_least_its_cost_on_the_wire() {
        let bpe = encoder_for(MODEL);
        let bare = estimate_unit(
            &unit_with_context("p-0001", BlockContext::default()),
            &bpe,
            DEFAULT_OUTPUT_EXPANSION_FACTOR,
        )
        .input;
        let heavy_unit = unit_with_context("p-0001", heavy_context());
        let heavy = estimate_unit(&heavy_unit, &bpe, DEFAULT_OUTPUT_EXPANSION_FACTOR).input;

        let wire = crate::llm::prompt::context_hint_json(&heavy_unit);
        let wire_tokens = bpe.encode_with_special_tokens(&wire).len();
        assert!(
            heavy - bare >= wire_tokens,
            "the context must cost at least its wire form: charged {} for a {wire_tokens}-token \
             object",
            heavy - bare
        );
    }

    /// R0003-0032: a label's cost is its escaped body. An ordinary label is
    /// unaffected (escaping is identity), so this changes nothing for the
    /// common case — which is why the envelope keeps measuring labels
    /// separately from the framing the envelope already holds.
    #[test]
    fn a_labels_cost_is_its_escaped_body() {
        let bpe = encoder_for(MODEL);
        let quoted = escape_heavy(24);
        let batch = batch_of(vec![], "en", "ko");
        let mut budget = budget_for(&batch, "");
        budget.system_prompt = "";

        budget.source_language = "en";
        budget.target_language = "ko";
        let plain = envelope_input_tokens(&budget, &bpe);
        assert_eq!(
            plain,
            bpe.encode_with_special_tokens("en").len() + bpe.encode_with_special_tokens("ko").len(),
            "an escape-free label costs exactly its own tokens, as it always did"
        );

        budget.source_language = &quoted;
        budget.target_language = "ko";
        let escaped = envelope_input_tokens(&budget, &bpe);
        let wire = serde_json::to_string(&quoted).expect("label serializes");
        assert_eq!(
            escaped,
            bpe.encode_with_special_tokens(&wire[1..wire.len() - 1])
                .len()
                + bpe.encode_with_special_tokens("ko").len(),
            "an escaping-heavy label costs its wire form, not its bytes in memory"
        );
        assert!(
            escaped
                > plain + bpe.encode_with_special_tokens(&quoted).len()
                    - bpe.encode_with_special_tokens("en").len(),
            "the escaped form is strictly dearer than the raw one"
        );
    }
}

// R0001-0012 / R0001-0013: what the packer is willing to *count*. A
// re-dispatched unit's ADR-0009 retry hint is provider-visible input the
// original packing never saw, and the two language labels are opaque
// caller-supplied strings ADR-0013 lets run to any length. Both move the
// estimate now: the hint per unit, the labels once per batch.
#[cfg(test)]
mod retry_and_label_budget_tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode, RetryContext};
    use crate::validate::ValidationLayer;

    const MODEL: &str = "gpt-4o";

    fn unit(id: &str) -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "the quick brown fox jumps over the lazy dog".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    /// The pristine unit plus the ADR-0009 side channel exactly as
    /// `pipeline::retry::retry_validation_unit` attaches it — same payload,
    /// same scope, with a `reason` at the 512-byte cap that helper truncates
    /// to. The payload is deliberately untouched: this test is about the
    /// *budget* around a verbatim resubmission, not about the resubmission.
    fn with_max_retry_hint(mut u: TranslationUnit) -> TranslationUnit {
        u.retry = Some(RetryContext {
            attempt: 2,
            rejected_by: Some(ValidationLayer::PerKindShape),
            reason: Some("table column count changed: ".repeat(18)),
        });
        u
    }

    fn budget<'a>(target_input_tokens: usize, src: &'a str, tgt: &'a str) -> BatchBudget<'a> {
        BatchBudget {
            target_input_tokens,
            max_units: 100_000,
            target_output_tokens: None,
            output_expansion_factor: DEFAULT_OUTPUT_EXPANSION_FACTOR,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt: "",
            // Empty on purpose: these two tests isolate the label and the
            // retry hint, so every token they measure has one source. The
            // instruction envelope has its own module below.
            instruction_envelope: "",
            source_language: src,
            target_language: tgt,
        }
    }

    fn ids(batches: &[Vec<TranslationUnit>]) -> Vec<String> {
        batches
            .iter()
            .flat_map(|b| b.iter().map(|u| u.unit_id.0.clone()))
            .collect()
    }

    /// R0001-0012 — THE discriminating test. Six units are packed under a
    /// target that holds them exactly. Attach the retry hint every
    /// re-dispatched unit carries and the same population no longer fits, so
    /// the packer splits it. Before the fix `estimate_unit` ignored `retry`
    /// entirely and this returned one batch, at a 512-byte reason prefix plus
    /// its truncation marker per unit over the cap.
    #[test]
    fn a_retry_hint_is_counted_and_splits_a_round_that_used_to_fit() {
        let bpe = encoder_for(MODEL);
        let pristine: Vec<TranslationUnit> = (1..=6).map(|i| unit(&format!("p-{i:04}"))).collect();
        let retried: Vec<TranslationUnit> =
            pristine.iter().cloned().map(with_max_retry_hint).collect();

        for (a, b) in pristine.iter().zip(&retried) {
            assert!(
                estimate_unit_tokens(b, &bpe) > estimate_unit_tokens(a, &bpe),
                "the retry hint must raise {}'s estimate",
                a.unit_id.0
            );
        }

        // A target that exactly holds the pristine round — zero slack, which
        // is the honest worst case for a first round packed to its budget.
        let target = envelope_input_tokens(&budget(0, "en", "ko"), &bpe)
            + pristine
                .iter()
                .map(|u| estimate_unit_tokens(u, &bpe))
                .sum::<usize>();
        let b = budget(target, "en", "ko");

        let first_round = group_by_token_budget(pristine.clone(), &b);
        assert_eq!(first_round.len(), 1, "the pristine round fits exactly");

        let retry_round = group_by_token_budget(retried, &b);
        assert!(
            retry_round.len() > 1,
            "the retry hints must push the round past the budget, got {} batch(es)",
            retry_round.len()
        );
        // Splitting must not lose or reorder a unit: the same ids, in the
        // same order, spread over more batches.
        assert_eq!(ids(&retry_round), ids(&first_round));
        assert!(
            retry_round.iter().all(|c| !c.is_empty()),
            "no empty retry batch"
        );
    }

    /// R0001-0013 — ADR-0013 lets a caller pass an opaque label of any
    /// length, so the envelope reserve encodes the labels instead of assuming
    /// they fit inside the fixed per-batch allowance the reserve used to open
    /// with. The estimate grows with the label, and packing feels it.
    #[test]
    fn a_long_opaque_language_label_grows_the_envelope_estimate() {
        let bpe = encoder_for(MODEL);
        // A label no fixed allowance could have covered — legal under
        // ADR-0013, which treats the label as an opaque pass-through string.
        let long_label =
            "Middle High German as written in the Rhine Franconian scribal tradition, ".repeat(24);

        let short = envelope_input_tokens(&budget(0, "en", "ko"), &bpe);
        let long = envelope_input_tokens(&budget(0, "en", &long_label), &bpe);
        assert!(
            long > short + 200,
            "the long label must be measured, not allowed for: {short} -> {long}"
        );
        assert_eq!(
            long - short,
            bpe.encode_with_special_tokens(&long_label).len()
                - bpe.encode_with_special_tokens("ko").len(),
            "the whole difference is the label itself"
        );
        // Both labels are counted, not just one.
        assert_eq!(
            envelope_input_tokens(&budget(0, &long_label, "ko"), &bpe),
            long,
            "source and target labels are costed the same way"
        );
        // An empty label costs nothing — and since ti aa92d6 neither does
        // anything else: with an empty envelope, an empty system prompt and
        // empty labels the reserve is exactly zero, because every one of its
        // components is now an encoded string rather than an allowance.
        assert_eq!(envelope_input_tokens(&budget(0, "", ""), &bpe), 0);

        // Packing feels it: the same units under the same raw target fit one
        // batch with a short label and no longer do with the long one.
        let units: Vec<TranslationUnit> = (1..=6).map(|i| unit(&format!("p-{i:04}"))).collect();
        let target = short
            + units
                .iter()
                .map(|u| estimate_unit_tokens(u, &bpe))
                .sum::<usize>();
        assert_eq!(
            group_by_token_budget(units.clone(), &budget(target, "en", "ko")).len(),
            1
        );
        assert!(
            group_by_token_budget(units, &budget(target, "en", &long_label)).len() > 1,
            "a label that eats the envelope must shrink the batches"
        );
    }
}

#[cfg(test)]
mod d1_output_budget_tests {
    use super::*;
    use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode};

    const MODEL: &str = "gpt-4o";
    /// The language labels every budget below runs with. Ordinary short
    /// labels, so the envelope reserve stays close to the pre-R0001-0013
    /// numbers; the pathological case has its own test.
    const SRC_LANG: &str = "en";
    const TGT_LANG: &str = "ko";

    /// The real assembled instruction envelope for the default profile's
    /// variant (ti aa92d6), so these tests pack against the reserve a real run
    /// carries rather than an empty one.
    fn envelope() -> String {
        crate::llm::prompt::instruction_envelope_json(
            crate::llm::prompt::InstructionVariant::for_run(
                &crate::profile::default_profile().constraints,
                crate::llm::prompt::DocumentFacts::default(),
            ),
        )
    }

    fn make_unit(id: &str, payload: &str) -> TranslationUnit {
        use crate::id::{BlockId, BlockKind};
        TranslationUnit {
            unit_id: BlockId(id.to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: payload.to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    fn make_batch(
        units: Vec<TranslationUnit>,
        ceiling: Option<u32>,
        factor: Option<f64>,
    ) -> TranslationBatch {
        let mut profile = crate::profile::default_profile();
        profile.batching.target_output_tokens = ceiling;
        profile.batching.output_expansion_factor = factor;
        TranslationBatch {
            batch_id: BatchId::new(1),
            units,
            source_language: "en".to_string(),
            target_language: "ko".to_string(),
            glossary: profile.glossary.clone(),
            profile,
        }
    }

    fn ids(batches: &[Vec<TranslationUnit>]) -> Vec<Vec<String>> {
        batches
            .iter()
            .map(|b| b.iter().map(|u| u.unit_id.0.clone()).collect())
            .collect()
    }

    /// The exact pre-D1 input-only packer, kept here as a regression oracle.
    /// The language labels are part of the envelope reserve since R0001-0013
    /// and the instruction envelope since ti aa92d6, so the oracle encodes all
    /// of them the same way — by hand, independently of
    /// [`envelope_input_tokens`].
    fn input_only_reference(
        units: &[TranslationUnit],
        input_target: usize,
        max_units: usize,
        model: &str,
        system_prompt: &str,
        instruction_envelope: &str,
    ) -> Vec<Vec<String>> {
        let bpe = encoder_for(model);
        let mut envelope = 0usize;
        for text in [instruction_envelope, system_prompt, SRC_LANG, TGT_LANG] {
            if !text.is_empty() {
                envelope += bpe.encode_with_special_tokens(text).len();
            }
        }
        let target = input_target.saturating_sub(envelope).max(1);
        let max_units = max_units.max(1);
        let mut batches: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        let mut sum = 0usize;
        for u in units {
            let t = estimate_unit_tokens(u, &bpe);
            if (!current.is_empty() && sum + t > target) || current.len() >= max_units {
                batches.push(std::mem::take(&mut current));
                sum = 0;
            }
            current.push(u.unit_id.0.clone());
            sum += t;
        }
        if !current.is_empty() {
            batches.push(current);
        }
        batches
    }

    // Test #1: input target huge, ceiling small, factor 1.0 → split points
    // driven purely by the output estimate.
    #[test]
    fn output_cap_splits_batches() {
        let bpe = encoder_for(MODEL);
        let units: Vec<TranslationUnit> = (1..=5)
            .map(|i| {
                make_unit(
                    &format!("p-{i:04}"),
                    "lorem ipsum dolor sit amet consectetur ",
                )
            })
            .collect();
        // Identical payloads + same-length ids → identical output estimate.
        let est = estimate_unit_output_tokens(&units[0], &bpe, 1.0);
        let env = envelope();
        // Ceiling s.t. output_target == est: one unit fits, two exceed.
        let budget = BatchBudget {
            target_input_tokens: 100_000_000,
            max_units: 100_000,
            target_output_tokens: Some(est + OUTPUT_ENVELOPE_RESERVE_TOKENS),
            output_expansion_factor: 1.0,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt: "",
            instruction_envelope: &env,
            source_language: SRC_LANG,
            target_language: TGT_LANG,
        };
        let batches = group_by_token_budget(units, &budget);
        assert_eq!(batches.len(), 5, "output cap forces one unit per batch");
        assert!(batches.iter().all(|b| b.len() == 1));
    }

    // Test #2: identical units; factor 1.0 → 1 batch, factor 4.0 → several.
    #[test]
    fn expansion_factor_changes_batch_shape() {
        let bpe = encoder_for(MODEL);
        let units: Vec<TranslationUnit> = (1..=4)
            .map(|i| {
                make_unit(
                    &format!("p-{i:04}"),
                    &"lorem ipsum dolor sit amet consectetur adipiscing elit ".repeat(20),
                )
            })
            .collect();
        let e1 = estimate_unit_output_tokens(&units[0], &bpe, 1.0);
        // Ceiling that exactly holds all four at factor 1.0.
        let ceiling = 4 * e1 + OUTPUT_ENVELOPE_RESERVE_TOKENS;
        let env = envelope();
        let mk = |factor: f64| BatchBudget {
            target_input_tokens: 100_000_000,
            max_units: 100_000,
            target_output_tokens: Some(ceiling),
            output_expansion_factor: factor,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt: "",
            instruction_envelope: &env,
            source_language: SRC_LANG,
            target_language: TGT_LANG,
        };
        let low = group_by_token_budget(units.clone(), &mk(1.0));
        let high = group_by_token_budget(units, &mk(4.0));
        assert_eq!(low.len(), 1, "all four fit in one batch at factor 1.0");
        assert!(
            high.len() > 1,
            "factor 4.0 must split into several batches, got {}",
            high.len()
        );
    }

    // Test #3: target_output_tokens None reproduces input-only packing exactly
    // (and the expansion factor is ignored). Regression pin.
    #[test]
    fn unset_ceiling_matches_input_only_packing() {
        let system_prompt = "You are a translator. Preserve structure.";
        let units: Vec<TranslationUnit> = vec![
            make_unit("p-0001", &"alpha ".repeat(10)),
            make_unit("p-0002", &"beta ".repeat(30)),
            make_unit("p-0003", &"gamma ".repeat(5)),
            make_unit("p-0004", &"delta ".repeat(25)),
            make_unit("p-0005", &"epsilon ".repeat(15)),
            make_unit("p-0006", &"zeta ".repeat(8)),
        ];
        let bpe = encoder_for(MODEL);
        let env = envelope();
        let reserve = bpe.encode_with_special_tokens(&env).len()
            + bpe.encode_with_special_tokens(system_prompt).len()
            + bpe.encode_with_special_tokens(SRC_LANG).len()
            + bpe.encode_with_special_tokens(TGT_LANG).len();
        let total: usize = units.iter().map(|u| estimate_unit_tokens(u, &bpe)).sum();
        let input_target = reserve + total / 3; // forces several batches
        let max_units = 100_000;

        let budget = BatchBudget {
            target_input_tokens: input_target,
            max_units,
            target_output_tokens: None,
            output_expansion_factor: 100.0, // must be ignored when the ceiling is unset
            model: MODEL,
            tokenizer_hint: None,
            system_prompt,
            instruction_envelope: &env,
            source_language: SRC_LANG,
            target_language: TGT_LANG,
        };
        let got = ids(&group_by_token_budget(units.clone(), &budget));
        let expected =
            input_only_reference(&units, input_target, max_units, MODEL, system_prompt, &env);
        assert_eq!(
            got, expected,
            "None ceiling must reproduce input-only packing exactly"
        );
        assert!(
            got.len() > 1,
            "fixture must actually split for the pin to bite"
        );
    }

    // Test #4: both caps apply; whichever binds first breaks the batch.
    #[test]
    fn both_caps_whichever_binds_first() {
        let bpe = encoder_for(MODEL);
        let units: Vec<TranslationUnit> = (1..=5)
            .map(|i| make_unit(&format!("p-{i:04}"), &"word ".repeat(40)))
            .collect();

        // (a) Input binds first: small input target, generous output ceiling.
        let e_in = estimate_unit_tokens(&units[0], &bpe);
        let env = envelope();
        // ~two units per batch, on top of the run's real envelope reserve.
        let input_target = bpe.encode_with_special_tokens(&env).len() + 2 * e_in;
        let a = group_by_token_budget(
            units.clone(),
            &BatchBudget {
                target_input_tokens: input_target,
                max_units: 100_000,
                target_output_tokens: Some(100_000_000),
                output_expansion_factor: 2.0,
                model: MODEL,
                tokenizer_hint: None,
                system_prompt: "",
                instruction_envelope: &env,
                source_language: SRC_LANG,
                target_language: TGT_LANG,
            },
        );
        let a_ref = input_only_reference(&units, input_target, 100_000, MODEL, "", &env);
        assert_eq!(
            ids(&a),
            a_ref,
            "input cap drives the split; the generous output ceiling never binds"
        );
        assert!(a.len() > 1, "input cap must actually split");

        // (b) Output binds first: huge input target, tight output ceiling.
        let e_out = estimate_unit_output_tokens(&units[0], &bpe, 2.0);
        let ceiling = e_out + OUTPUT_ENVELOPE_RESERVE_TOKENS; // one unit per batch by output
        let b = group_by_token_budget(
            units.clone(),
            &BatchBudget {
                target_input_tokens: 100_000_000,
                max_units: 100_000,
                target_output_tokens: Some(ceiling),
                output_expansion_factor: 2.0,
                model: MODEL,
                tokenizer_hint: None,
                system_prompt: "",
                instruction_envelope: &env,
                source_language: SRC_LANG,
                target_language: TGT_LANG,
            },
        );
        assert_eq!(
            b.len(),
            units.len(),
            "output cap drives the split: one unit per batch"
        );
        // Contrast: drop the output cap and the same huge input target yields a
        // single batch — proving the split in (b) was output-driven.
        let b_noout = group_by_token_budget(
            units,
            &BatchBudget {
                target_input_tokens: 100_000_000,
                max_units: 100_000,
                target_output_tokens: None,
                output_expansion_factor: 2.0,
                model: MODEL,
                tokenizer_hint: None,
                system_prompt: "",
                instruction_envelope: &env,
                source_language: SRC_LANG,
                target_language: TGT_LANG,
            },
        );
        assert_eq!(
            b_noout.len(),
            1,
            "no output cap + huge input target → one batch"
        );
    }

    // Test #5: a single unit whose own est_out exceeds the target gets its own
    // batch (never dropped) and output_budget_warnings names it with the
    // correct estimate + ceiling numbers.
    #[test]
    fn oversized_unit_gets_own_batch_and_is_flagged() {
        let bpe = encoder_for(MODEL);
        let tiny = make_unit("p-0001", "x");
        let giant = make_unit("p-0002", &"word ".repeat(60));
        let factor = 2.0;
        let est_tiny = estimate_unit_output_tokens(&tiny, &bpe, factor);
        let est_giant = estimate_unit_output_tokens(&giant, &bpe, factor);
        // output_target between tiny and giant so only the giant is oversized.
        let output_target = est_giant - 1;
        assert!(
            est_tiny <= output_target,
            "the tiny unit must fit under the ceiling"
        );
        let ceiling = output_target + OUTPUT_ENVELOPE_RESERVE_TOKENS;
        let env = envelope();

        let batches = group_by_token_budget(
            vec![tiny.clone(), giant.clone()],
            &BatchBudget {
                target_input_tokens: 100_000_000,
                max_units: 100_000,
                target_output_tokens: Some(ceiling),
                output_expansion_factor: factor,
                model: MODEL,
                tokenizer_hint: None,
                system_prompt: "",
                instruction_envelope: &env,
                source_language: SRC_LANG,
                target_language: TGT_LANG,
            },
        );
        assert_eq!(batches.len(), 2, "the oversized unit breaks off");
        assert_eq!(
            batches[1].len(),
            1,
            "the oversized unit is alone in its own batch"
        );
        assert_eq!(batches[1][0].unit_id.0, "p-0002");

        // The preflight flags exactly the oversized unit.
        let batch = make_batch(vec![tiny, giant], Some(ceiling as u32), Some(factor));
        let warns = output_budget_warnings(std::slice::from_ref(&batch), MODEL, None);
        assert_eq!(
            warns.len(),
            1,
            "only the oversized unit is flagged: {warns:?}"
        );
        assert_eq!(warns[0].unit_id.0, "p-0002");
        assert_eq!(
            warns[0].ceiling_tokens, ceiling as u32,
            "reports the raw ceiling"
        );
        assert_eq!(warns[0].estimated_output_tokens, est_giant as u32);
        let msg = warns[0].to_string();
        assert!(
            msg.contains("p-0002")
                && msg.contains(&est_giant.to_string())
                && msg.contains(&(ceiling as u32).to_string()),
            "Display names the id, estimate, and ceiling: {msg}"
        );
    }

    // Unset ceiling → no warnings, even for an obviously huge unit.
    #[test]
    fn unset_ceiling_produces_no_warnings() {
        let giant = make_unit("p-0002", &"word ".repeat(200));
        let batch = make_batch(vec![giant], None, Some(2.0));
        assert!(output_budget_warnings(std::slice::from_ref(&batch), MODEL, None).is_empty());
    }

    /// R0003-0030 / R0003-0029 — an absurd but FINITE expansion factor is a
    /// legal configuration: `resolve_expansion_factor` and
    /// `profile::normalize_batching` reject only non-finite and non-positive
    /// values, and a library caller sets `BatchBudget::output_expansion_factor`
    /// directly. Such a factor saturates the float→int cast, and before this
    /// the additions built on top of it — the per-unit estimate and the
    /// running batch sum — overflowed on the very next line: a panic in any
    /// checked build, a wrapped total in release. The estimate now saturates
    /// through, which reads as "larger than any budget" and degrades into the
    /// packer's existing one-unit-per-batch behavior.
    #[test]
    fn an_absurd_finite_expansion_factor_saturates_instead_of_overflowing() {
        let bpe = encoder_for(MODEL);
        let factor = 1e308_f64;
        let units: Vec<TranslationUnit> = (1..=3)
            .map(|i| make_unit(&format!("p-{i:04}"), "lorem ipsum dolor sit amet "))
            .collect();

        // The per-unit estimate saturates rather than panicking (R0003-0030).
        let est = estimate_unit_output_tokens(&units[0], &bpe, factor);
        assert_eq!(
            est,
            usize::MAX,
            "a saturated expansion plus the id and framing must stay saturated"
        );

        // And the packer's running sum survives it (R0003-0029): three units
        // whose estimates are each usize::MAX pack one to a batch, in order,
        // losing nobody.
        let env = envelope();
        let batches = group_by_token_budget(
            units.clone(),
            &BatchBudget {
                target_input_tokens: 100_000_000,
                max_units: 100_000,
                target_output_tokens: Some(8_000),
                output_expansion_factor: factor,
                model: MODEL,
                tokenizer_hint: None,
                system_prompt: "",
                instruction_envelope: &env,
                source_language: SRC_LANG,
                target_language: TGT_LANG,
            },
        );
        assert_eq!(
            ids(&batches),
            vec![
                vec!["p-0001".to_string()],
                vec!["p-0002".to_string()],
                vec!["p-0003".to_string()],
            ],
            "every unit is over the ceiling, so each gets its own batch and none is dropped"
        );

        // The preflight names all three rather than tripping over the same
        // arithmetic, and reports a saturated estimate as the u32 it can.
        let batch = make_batch(units, Some(8_000), Some(factor));
        let warns = output_budget_warnings(std::slice::from_ref(&batch), MODEL, None);
        assert_eq!(warns.len(), 3, "{warns:?}");
        assert_eq!(warns[0].estimated_output_tokens, u32::MAX);
    }
}

// OI-0029: encoder selection is a provider-declared hint with the legacy
// model-name heuristic as the documented fallback.
#[cfg(test)]
mod tokenizer_hint_tests {
    use super::*;

    /// A probe long enough that the two vocabularies disagree on its token
    /// count, so "same encoder" is a real assertion and not a coincidence.
    const PROBE: &str = "안녕하세요 — transync가 블록 ID를 유지합니다. `code_span` and a URL: \
                         https://example.com/path?x=1";

    fn count(bpe: &CoreBPE) -> usize {
        bpe.encode_with_special_tokens(PROBE).len()
    }

    #[test]
    fn encoder_for_hint_maps_both_variants() {
        assert_eq!(
            count(&encoder_for_hint(TokenizerHint::O200kBase)),
            count(&encoder_for("gpt-4o")),
            "O200kBase must load the same encoder the heuristic picks for gpt-4o"
        );
        assert_eq!(
            count(&encoder_for_hint(TokenizerHint::Cl100kBase)),
            count(&encoder_for("gpt-4-turbo")),
            "Cl100kBase must load the same encoder the heuristic picks for gpt-4-turbo"
        );
        assert_ne!(
            count(&encoder_for_hint(TokenizerHint::O200kBase)),
            count(&encoder_for_hint(TokenizerHint::Cl100kBase)),
            "the probe must actually distinguish the two vocabularies"
        );
    }

    /// An explicit hint beats the model name, so a provider whose model
    /// ids mean nothing to `encoder_for` still gets its real tokenizer.
    #[test]
    fn resolve_encoder_prefers_the_hint_over_the_model_name() {
        let hinted = resolve_encoder(Some(TokenizerHint::O200kBase), "some-vendor/model-x");
        assert_eq!(count(&hinted), count(&encoder_for("gpt-4o")));
        let hinted_down = resolve_encoder(Some(TokenizerHint::Cl100kBase), "gpt-5-chat-latest");
        assert_eq!(
            count(&hinted_down),
            count(&encoder_for("gpt-4-turbo")),
            "the hint must win even when the model name looks o200k-shaped"
        );
    }

    /// `None` keeps the pre-OI-0029 behavior byte-for-byte.
    #[test]
    fn resolve_encoder_falls_back_to_the_model_heuristic() {
        for model in ["gpt-5-chat-latest", "gpt-4-turbo", "some-vendor/model-x"] {
            assert_eq!(
                count(&resolve_encoder(None, model)),
                count(&encoder_for(model)),
                "no hint must reproduce encoder_for({model:?})"
            );
        }
    }
}

// R0004-0076: the input axis's preflight. The packer's
// `target - envelope, floored at 1` turns "the reserve ate the budget" into a
// silent one-unit-per-batch run in which every request is over target; these
// pin the boundary the diagnosis fires on and the fact that an ordinary budget
// stays quiet.
#[cfg(test)]
mod input_budget_preflight_tests {
    use super::*;

    const MODEL: &str = "gpt-4o";

    /// A budget whose reserve is a real compiled system prompt, at whatever
    /// input target the caller wants to test.
    fn budget<'a>(target_input_tokens: usize, system_prompt: &'a str) -> BatchBudget<'a> {
        BatchBudget {
            target_input_tokens,
            max_units: 100,
            target_output_tokens: None,
            output_expansion_factor: DEFAULT_OUTPUT_EXPANSION_FACTOR,
            model: MODEL,
            tokenizer_hint: None,
            system_prompt,
            instruction_envelope: "",
            source_language: "en",
            target_language: "ko",
        }
    }

    const PROMPT: &str = "Translate the following Markdown blocks faithfully, preserving \
                          structure, and never inventing content that is not in the source.";

    /// The boundary, from both sides. `>=` is the condition, so the target
    /// that exactly equals the reserve is already the degenerate one — it
    /// leaves zero payload capacity, which is what the floor of 1 hides.
    #[test]
    fn the_finding_fires_at_the_exact_point_the_reserve_meets_the_target() {
        let bpe = encoder_for(MODEL);
        let reserve = envelope_input_tokens(&budget(0, PROMPT), &bpe);
        assert!(reserve > 1, "the fixture prompt must cost something");

        assert!(
            input_budget_warning(&budget(reserve + 1, PROMPT), &bpe).is_none(),
            "one token of payload capacity is a budget, however thin"
        );
        let flagged =
            input_budget_warning(&budget(reserve, PROMPT), &bpe).expect("equal is degenerate");
        assert_eq!(flagged.envelope_tokens, reserve);
        assert_eq!(flagged.target_tokens, reserve);
        assert!(
            input_budget_warning(&budget(reserve - 1, PROMPT), &bpe).is_some(),
            "and below it, plainly"
        );
    }

    /// The message names the knob that moves the condition, because the
    /// remedy is a profile edit and the reader is holding a profile.
    #[test]
    fn the_message_names_the_profile_knob() {
        let bpe = encoder_for(MODEL);
        let said = input_budget_warning(&budget(1, PROMPT), &bpe)
            .expect("a one-token target cannot hold this prompt")
            .to_string();
        assert!(
            said.contains("[batching].target_input_tokens_per_batch"),
            "{said}"
        );
    }

    /// The other direction, and the reason this is a preflight rather than a
    /// new default warning: an ordinary run says nothing at all.
    #[test]
    fn an_ordinary_budget_is_silent() {
        let bpe = encoder_for(MODEL);
        let default_target = crate::TranslateOptions::default().target_input_tokens_per_batch;
        assert!(
            input_budget_warning(&budget(default_target as usize, PROMPT), &bpe).is_none(),
            "the built-in input target dwarfs an ordinary compiled prompt"
        );
    }

    /// The reserve is the packer's own number, not a second reading of it —
    /// so the preflight cannot disagree with the packing it warns about.
    #[test]
    fn the_flagged_run_really_does_pack_one_unit_per_batch() {
        use crate::id::{BlockId, BlockKind};
        use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode};

        let bpe = encoder_for(MODEL);
        let reserve = envelope_input_tokens(&budget(0, PROMPT), &bpe);
        let b = budget(reserve, PROMPT);
        assert!(input_budget_warning(&b, &bpe).is_some());

        let units: Vec<TranslationUnit> = (1..=4)
            .map(|i| TranslationUnit {
                unit_id: BlockId(format!("p-{i:04}")),
                block_kind: BlockKind::Paragraph,
                input_mode: InputMode::TextFragment,
                source_payload: "a short paragraph".to_string(),
                context: BlockContext::default(),
                constraints: BlockConstraints::default(),
                source_hash: 0,
                batch_id: BatchId::new(1),
                retry: None,
            })
            .collect();
        let packed = group_by_token_budget_with(units, &b, &bpe);
        assert_eq!(
            packed.len(),
            4,
            "the degenerate state the warning describes: one unit per batch"
        );
    }
}
