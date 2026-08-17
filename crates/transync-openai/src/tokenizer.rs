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

use transync::llm::TokenizerHint;

/// Which tiktoken encoder approximates this model's tokenizer, for
/// batch-budget estimation (OI-0029). Same `o200k_base` families as
/// core's legacy `batch::encoder_for` heuristic — `chat` / `gpt-4o` /
/// `gpt-5` / `o1` / `o3` / `o4` — but owned here, where the knowledge of
/// OpenAI model names belongs, rather than inferred by the engine from an
/// advisory `TranslateOptions::model_id`.
///
/// TRACE: SCN-10
/// TRACE: OI-0029
pub(crate) fn tokenizer_hint_for_model(model: &str) -> TokenizerHint {
    let m = model.to_ascii_lowercase();
    let is_o200k = m.starts_with("gpt-4o")
        || m.starts_with("gpt-5")
        || m.starts_with("o1")
        || m.starts_with("o3")
        || m.starts_with("o4");
    if is_o200k {
        TokenizerHint::O200kBase
    } else {
        TokenizerHint::Cl100kBase
    }
}
