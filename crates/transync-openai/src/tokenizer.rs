//! Model-name → tiktoken-encoder heuristic for batch-budget estimation.
//!
//! This is batch-budgeting knowledge, not HTTP knowledge, which is why it
//! lives outside `client` entirely (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`). It is deliberately
//! independent of `client::api_for_model`'s surface-dispatch heuristic: a
//! `*-chat-*` alias rides on Chat Completions but is still an o200k-era
//! model.
//!
//! TRACE: SCN-10
//! TRACE: OI-0029

use crate::client::generation_after;
use transync::llm::TokenizerHint;

/// The first GPT generation encoded by `o200k_base`. A floor, not a roster:
/// every later generation is o200k too, so a model OpenAI has not shipped yet
/// needs no edit here. `gpt-4o` is the one earlier family on the o200k side
/// and is named separately, because its `o` is a product suffix rather than a
/// generation number.
const FIRST_O200K_GPT_GENERATION: u32 = 5;

/// Which tiktoken encoder approximates this model's tokenizer, for
/// batch-budget estimation (OI-0029). The `o200k_base` side is `gpt-4o`,
/// every GPT generation from [`FIRST_O200K_GPT_GENERATION`] up, and the whole
/// O-series — owned here, where the knowledge of OpenAI model names belongs,
/// rather than inferred by the engine from an advisory
/// `TranslateOptions::model_id`.
///
/// **This reads the generation number rather than listing families**, and
/// getting that wrong is more expensive here than in surface dispatch. The
/// rule used to name `gpt-5`, `o1`, `o3` and `o4` by prefix, so `gpt-6` and
/// any later family fell through to `cl100k_base` — a *silently wrong token
/// count*, not an error. Batch packing sizes every request from this estimate
/// (`batch::estimate_unit`), so an under-count builds batches that overrun the
/// real context window and are rejected at the provider, and an over-count
/// wastes requests. Sharing [`generation_after`] with surface dispatch keeps
/// the two readings of a model name from drifting apart.
///
/// It stays independent of `client::api_for_model` in one respect, deliberately:
/// a `*-chat-*` alias rides on Chat Completions but is still an o200k-era
/// model, so the `chat` rule is not repeated here.
///
/// `transync_core::batch::encoder_for` carries a deliberate twin of this rule
/// for a provider that declares no hint, because core depends on no provider
/// crate; change both together (DCR-0059).
///
/// TRACE: SCN-10
/// TRACE: OI-0029
/// TRACE: DCR-0059
pub(crate) fn tokenizer_hint_for_model(model: &str) -> TokenizerHint {
    let m = model.to_ascii_lowercase();
    let is_o200k = m.starts_with("gpt-4o")
        || generation_after(&m, "o").is_some()
        || generation_after(&m, "gpt-").is_some_and(|g| g >= FIRST_O200K_GPT_GENERATION);
    if is_o200k {
        TokenizerHint::O200kBase
    } else {
        TokenizerHint::Cl100kBase
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// DCR-0059: the same floor the surface dispatch uses. A generation past
    /// GPT-5 must not fall back to `cl100k_base` — that is a wrong token count
    /// rather than an error, and batch packing believes it.
    #[test]
    fn a_generation_past_gpt5_is_o200k_without_being_listed() {
        for model in [
            "gpt-5",
            "gpt-5.6-terra",
            "gpt-5-chat-latest",
            "gpt-6",
            "gpt-6-mini",
            "gpt-7.4-preview",
            "gpt-10",
            "o1",
            "o3-mini",
            "o4-mini",
            "o5",
            "o6-pro",
            "gpt-4o",
            "gpt-4o-mini",
        ] {
            assert_eq!(
                tokenizer_hint_for_model(model),
                TokenizerHint::O200kBase,
                "{model} is an o200k-era model"
            );
        }
    }

    /// The other direction stays put: pre-4o families, and names that only
    /// look like a generation, remain `cl100k_base`.
    #[test]
    fn older_and_numberless_families_stay_cl100k() {
        for model in [
            "gpt-4",
            "gpt-4.1",
            "gpt-4-turbo",
            "gpt-3.5-turbo",
            "gpt-oss-120b",
            "text-davinci-003",
        ] {
            assert_eq!(
                tokenizer_hint_for_model(model),
                TokenizerHint::Cl100kBase,
                "{model} is not an o200k-era model"
            );
        }
    }
}
