//! Run-level batch-budget resolution.
//!
//! Turns the caller's [`TranslateOptions`] and the run's profile into the
//! single [`BatchBudget`] the packer runs with. Three knobs are resolved
//! here — the input-token target, the hard unit cap, and the output ceiling
//! (plus its expansion factor) — and the first two share one resolution rule,
//! expressed once in [`caller_wins`].
//!
//! TRACE: OI-0003
//! TRACE: contracts.md §2

use crate::TranslateOptions;
use crate::batch::{BatchBudget, resolve_expansion_factor};
use crate::llm::TokenizerHint;
use crate::profile::ProfileMetadata;

/// The shared "caller wins, else profile, else built-in default" rule
/// (D1 / OI-0003).
///
/// `caller_value` is the [`TranslateOptions`] field, `default_value` the same
/// field on [`TranslateOptions::default()`], `profile_value` the matching
/// `[batching]` entry. A caller value differing from the built-in default is
/// taken as deliberate and wins outright; otherwise the profile supplies the
/// value, and an unset profile entry leaves the built-in default standing.
///
/// The known limit: a caller *explicitly* passing the built-in default is
/// indistinguishable from "unset", so the profile still wins in that case.
/// That is documented behavior (contracts.md §2), not an oversight —
/// `TranslateOptions` has no `Option` for these fields.
fn caller_wins(caller_value: u32, default_value: u32, profile_value: Option<u32>) -> u32 {
    if caller_value != default_value {
        caller_value
    } else {
        profile_value.unwrap_or(default_value)
    }
}

/// Report a zero caller-side knob and resolve it as if it were unset
/// (R0001-0017).
///
/// Neither shared knob can pack a batch at `0`, and [`TranslateOptions`]
/// carries no `Option` in which to say "unset" — so a zero is reported on the
/// `tracing` channel (the same one `profile::load_profile` warns on) and then
/// handed to [`caller_wins`] as the built-in default, which makes the profile
/// value, else that default, apply. The previous behavior was a bare
/// `.max(1)`: the run silently proceeded at one unit (or one token) per
/// batch, a shape nobody asked for and nobody was told about.
fn caller_zero_is_unset(field: &str, caller_value: u32, default_value: u32) -> u32 {
    if caller_value == 0 {
        tracing::warn!(
            target: "transync::profile",
            "TranslateOptions::{field} = 0 cannot pack a batch; ignored — the profile value, \
             else the built-in default ({default_value}), applies"
        );
        return default_value;
    }
    caller_value
}

/// THE run's output ceiling, resolved: profile-only (there is no
/// `TranslateOptions` counterpart), with a zero reading as "unset" for the
/// same reason the two shared knobs ignore theirs (R0001-0016).
///
/// [`resolve`] fills [`BatchBudget::target_output_tokens`] from this, and the
/// row-window splitter (`unit::split`, DCR-0026) asks the same question before
/// packing — so "is there a ceiling, and what is it" has one answer per run.
///
/// TRACE: DCR-0026
pub(crate) fn output_ceiling(profile: &ProfileMetadata) -> Option<usize> {
    profile
        .batching
        .target_output_tokens
        .filter(|&t| t != 0)
        .map(|t| t as usize)
}

/// Resolve the run's packing budget.
///
/// `profile` supplies the `[batching]` defaults; `system_prompt` is the
/// *rendered* profile's prompt body (its encoded length is reserved off the
/// input target once per batch, R0008-0029), `instruction_envelope` the
/// constant user-message envelope the run will send
/// (`llm::prompt::instruction_envelope_json`, reserved the same way — ti
/// aa92d6), and `tokenizer_hint` the dispatching provider's declared encoder.
///
/// Both borrowed strings are the caller's because they are *assembled*, not
/// stored: budget resolution never loads or renders a profile itself, and it
/// does not know the document-level fact that picks the instruction variant.
pub(crate) fn resolve<'a>(
    opts: &'a TranslateOptions,
    profile: &ProfileMetadata,
    tokenizer_hint: Option<TokenizerHint>,
    system_prompt: &'a str,
    instruction_envelope: &'a str,
) -> BatchBudget<'a> {
    let defaults = TranslateOptions::default();
    // D1: the profile's [batching].target_input_tokens_per_batch supplies the
    // default input budget under the exact same resolution rule as
    // max_units_per_batch below.
    //
    // R0001-0017: no `.max(1)` floor survives here. A zero is unusable, not a
    // request for the smallest legal batch, so each side turns it back into
    // "unset" — the caller side loudly (`caller_zero_is_unset`), the profile
    // side quietly, because `profile::normalize_batching` already warned about
    // it at both profile gates and this filter only keeps `resolve` total for
    // a profile that reached it some other way. Every remaining branch is
    // non-zero: `TranslateOptions::default()` sets both knobs above zero.
    let target_input_tokens = caller_wins(
        caller_zero_is_unset(
            "target_input_tokens_per_batch",
            opts.target_input_tokens_per_batch,
            defaults.target_input_tokens_per_batch,
        ),
        defaults.target_input_tokens_per_batch,
        profile
            .batching
            .target_input_tokens_per_batch
            .filter(|&v| v != 0),
    ) as usize;
    // OI-0003: the profile's [batching].max_units_per_batch supplies the
    // default unit cap under the same rule.
    let max_units = caller_wins(
        caller_zero_is_unset(
            "max_units_per_batch",
            opts.max_units_per_batch,
            defaults.max_units_per_batch,
        ),
        defaults.max_units_per_batch,
        profile.batching.max_units_per_batch.filter(|&v| v != 0),
    ) as usize;
    // D1: the output cap comes from the profile only — there is no
    // TranslateOptions counterpart, so no sentinel applies.
    // `target_output_tokens: None` disables output-aware packing (and the
    // preflight); the factor is resolved to a sane value (invalid values were
    // normalized to None at load, but resolve_expansion_factor guards direct
    // library callers too). A zero ceiling reads as `None` for the same reason
    // the two knobs above ignore theirs (R0001-0016).
    BatchBudget {
        target_input_tokens,
        max_units,
        target_output_tokens: output_ceiling(profile),
        output_expansion_factor: resolve_expansion_factor(profile.batching.output_expansion_factor),
        model: &opts.model_id,
        tokenizer_hint,
        system_prompt,
        instruction_envelope,
        // R0001-0013: the labels every batch carries. `build_batches` copies
        // these two straight onto each `TranslationBatch`, so measuring the
        // caller's values here measures what goes on the wire. A caller
        // assembling a batch by hand may diverge; the retry packer in
        // `pipeline::dispatch` re-points these at the batch's own labels for
        // exactly that reason.
        source_language: &opts.source_language,
        target_language: &opts.target_language,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::default_profile;

    /// A profile whose `[batching]` section is mutated by `f`.
    fn profile_with(f: impl FnOnce(&mut crate::profile::ProfileBatching)) -> ProfileMetadata {
        let mut p = default_profile();
        f(&mut p.batching);
        p
    }

    fn opts_with(f: impl FnOnce(&mut TranslateOptions)) -> TranslateOptions {
        let mut o = TranslateOptions::default();
        f(&mut o);
        o
    }

    #[test]
    fn input_token_target_resolves_caller_then_profile_then_default() {
        let defaults = TranslateOptions::default();

        // caller-wins: a value differing from the built-in default beats the
        // profile entry.
        let caller = opts_with(|o| o.target_input_tokens_per_batch = 1234);
        let b = resolve(
            &caller,
            &profile_with(|c| c.target_input_tokens_per_batch = Some(4000)),
            None,
            "",
            "",
        );
        assert_eq!(b.target_input_tokens, 1234);

        // profile-wins: caller left at the built-in default.
        let b = resolve(
            &defaults,
            &profile_with(|c| c.target_input_tokens_per_batch = Some(4000)),
            None,
            "",
            "",
        );
        assert_eq!(b.target_input_tokens, 4000);

        // default: neither side set.
        let b = resolve(
            &defaults,
            &profile_with(|c| c.target_input_tokens_per_batch = None),
            None,
            "",
            "",
        );
        assert_eq!(
            b.target_input_tokens, defaults.target_input_tokens_per_batch as usize,
            "unset on both sides leaves the built-in default standing"
        );
    }

    #[test]
    fn unit_cap_resolves_caller_then_profile_then_default() {
        let defaults = TranslateOptions::default();

        let caller = opts_with(|o| o.max_units_per_batch = 7);
        let b = resolve(
            &caller,
            &profile_with(|c| c.max_units_per_batch = Some(1)),
            None,
            "",
            "",
        );
        assert_eq!(b.max_units, 7, "caller override beats the profile cap");

        let b = resolve(
            &defaults,
            &profile_with(|c| c.max_units_per_batch = Some(1)),
            None,
            "",
            "",
        );
        assert_eq!(
            b.max_units, 1,
            "profile cap applies when the caller is at default"
        );

        let b = resolve(
            &defaults,
            &profile_with(|c| c.max_units_per_batch = None),
            None,
            "",
            "",
        );
        assert_eq!(b.max_units, defaults.max_units_per_batch as usize);
    }

    /// R0001-0017: a zero on either side is an unusable value, not a request
    /// for the smallest legal batch. It resolves as if unset — profile, else
    /// built-in default — and never as the old silent floor of `1`.
    #[test]
    fn a_zero_knob_resolves_as_unset_never_as_a_floor_of_one() {
        let defaults = TranslateOptions::default();
        let zero_caller = opts_with(|o| {
            o.max_units_per_batch = 0;
            o.target_input_tokens_per_batch = 0;
        });

        // Caller zero, profile silent → the built-in defaults, not 1.
        let b = resolve(
            &zero_caller,
            &profile_with(|c| {
                c.max_units_per_batch = None;
                c.target_input_tokens_per_batch = None;
            }),
            None,
            "",
            "",
        );
        assert_eq!(b.max_units, defaults.max_units_per_batch as usize);
        assert_eq!(
            b.target_input_tokens,
            defaults.target_input_tokens_per_batch as usize
        );

        // Caller zero, profile set → the profile wins, exactly as for an
        // untouched caller: an ignored value cannot beat a real one.
        let b = resolve(
            &zero_caller,
            &profile_with(|c| {
                c.max_units_per_batch = Some(4);
                c.target_input_tokens_per_batch = Some(400);
            }),
            None,
            "",
            "",
        );
        assert_eq!(b.max_units, 4);
        assert_eq!(b.target_input_tokens, 400);

        // A profile zero that reached `resolve` without crossing
        // `profile::normalize_batching` is inert too — including the output
        // ceiling, which must never reach a provider as a zero budget
        // (R0001-0016).
        let b = resolve(
            &defaults,
            &profile_with(|c| {
                c.max_units_per_batch = Some(0);
                c.target_input_tokens_per_batch = Some(0);
                c.target_output_tokens = Some(0);
            }),
            None,
            "",
            "",
        );
        assert_eq!(b.max_units, defaults.max_units_per_batch as usize);
        assert_eq!(
            b.target_input_tokens,
            defaults.target_input_tokens_per_batch as usize
        );
        assert_eq!(
            b.target_output_tokens, None,
            "a zero ceiling is no ceiling, not a ceiling of zero"
        );
    }

    #[test]
    fn output_ceiling_comes_from_the_profile_only() {
        let defaults = TranslateOptions::default();

        // profile-wins: the ceiling and its factor both come from [batching].
        let b = resolve(
            &defaults,
            &profile_with(|c| {
                c.target_output_tokens = Some(8000);
                c.output_expansion_factor = Some(2.5);
            }),
            None,
            "",
            "",
        );
        assert_eq!(b.target_output_tokens, Some(8000));
        assert_eq!(b.output_expansion_factor, 2.5);

        // default: an unset ceiling disables output-aware packing entirely,
        // and an unset factor falls back to the built-in one.
        let b = resolve(
            &defaults,
            &profile_with(|c| {
                c.target_output_tokens = None;
                c.output_expansion_factor = None;
            }),
            None,
            "",
            "",
        );
        assert_eq!(b.target_output_tokens, None);
        assert_eq!(
            b.output_expansion_factor,
            crate::batch::DEFAULT_OUTPUT_EXPANSION_FACTOR
        );

        // no caller-wins arm exists: TranslateOptions carries no output-cap
        // field, so caller settings cannot influence the ceiling.
        let caller = opts_with(|o| {
            o.target_input_tokens_per_batch = 99;
            o.max_units_per_batch = 99;
        });
        let b = resolve(
            &caller,
            &profile_with(|c| c.target_output_tokens = Some(8000)),
            None,
            "",
            "",
        );
        assert_eq!(b.target_output_tokens, Some(8000));
    }

    #[test]
    fn borrowed_fields_come_from_the_caller_and_the_rendered_prompt() {
        let opts = opts_with(|o| o.model_id = "gpt-test".to_string());
        let b = resolve(&opts, &default_profile(), None, "SYSTEM", "ENVELOPE");
        assert_eq!(b.model, "gpt-test");
        assert_eq!(b.system_prompt, "SYSTEM");
        // ti aa92d6: the assembled instruction envelope is the caller's too,
        // and reaches the packer unaltered.
        assert_eq!(b.instruction_envelope, "ENVELOPE");
    }
}
