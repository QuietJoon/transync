//! Model-name → HTTP-surface dispatch.
//!
//! One heuristic over the model string, plus the `TRANSYNC_OPENAI_API`
//! override that can force it. Deliberately independent of
//! [`crate::tokenizer`]'s model→encoder heuristic: the two read the same
//! string for unrelated reasons and must be free to disagree.
//!
//! Split out of `client.rs` (OI-0008 / `R0001-0079` in the removed
//! `reviews/reviewed/0001.md`). `super` re-exports [`Api`],
//! [`ParseApiError`], and [`api_for_model`], which are public crate surface;
//! `crate` re-exports the first two at the root as well.
//!
//! TRACE: SCN-12

use std::fmt;
use std::str::FromStr;

/// Which OpenAI HTTP surface to call for a given request.
///
/// Re-exported at the crate root as `transync_openai::Api`, because
/// [`crate::TransyncOpenAI::with_api`] takes one and a caller pinning a
/// surface should not have to name the `client` module to say which.
///
/// TRACE: SCN-12
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Api {
    ChatCompletions,
    Responses,
}

impl Api {
    /// The canonical name of this surface: `"chat"` or `"responses"`.
    ///
    /// Round-trips through [`FromStr`] (which additionally accepts the
    /// `chat_completions` / `chatcompletions` aliases), and is the token
    /// [`crate::TransyncOpenAI`]'s `fingerprint()` puts in the provider
    /// namespace — so these two strings are **cache-visible**: changing
    /// them re-namespaces every warm entry.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat",
            Self::Responses => "responses",
        }
    }
}

impl fmt::Display for Api {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "unsupported OpenAI API surface {0:?}; expected chat (aliases chat_completions, chatcompletions) or responses"
)]
pub struct ParseApiError(String);

/// Parse a caller-supplied surface name exactly the way
/// `TRANSYNC_OPENAI_API` is parsed — same tokens, same aliases, same
/// case- and whitespace-insensitivity — because both go through the one
/// private `parse_api` (unlinked: it is crate-private, which the rustdoc
/// gate refuses to link from public docs). A `--api` flag that accepted a
/// different vocabulary than the variable it overrides would be a new
/// instance of the very divergence
/// [`crate::TransyncOpenAI::with_api`] exists to remove.
impl FromStr for Api {
    type Err = ParseApiError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        parse_api(value).ok_or_else(|| ParseApiError(value.to_owned()))
    }
}

/// THE surface-name vocabulary. Both the environment override and
/// [`Api::from_str`] resolve through here, so the two cannot drift apart.
///
/// `chat_completions` / `chatcompletions` are long-standing aliases of
/// `chat`; the endpoint is spelled `/v1/chat/completions`, so an operator
/// naming it that way is not making a mistake worth refusing.
fn parse_api(value: &str) -> Option<Api> {
    match value.trim().to_ascii_lowercase().as_str() {
        "chat" | "chat_completions" | "chatcompletions" => Some(Api::ChatCompletions),
        "responses" => Some(Api::Responses),
        _ => None,
    }
}

/// The first GPT generation that speaks Responses rather than Chat
/// Completions. Read as a floor, not a list: every generation from here up
/// dispatches to Responses, so a model OpenAI has not shipped yet needs no
/// edit to this crate.
const FIRST_RESPONSES_GPT_GENERATION: u32 = 5;

/// The generation number a model name declares right after `prefix`, when
/// what follows the prefix is a digit run: `gpt-` in `gpt-5.6-terra` yields
/// `5`, `o` in `o4-mini` yields `4`, and `gpt-` in `gpt-oss` yields `None`.
///
/// Only the leading integer is read, so a point release rides its major
/// generation (`gpt-5.6` is a GPT-5). A digit run too long for `u32` saturates
/// to `u32::MAX` instead of failing to parse: an absurd generation is still a
/// *later* one, and the failure direction matters — see [`api_for_model`].
pub(crate) fn generation_after(model: &str, prefix: &str) -> Option<u32> {
    let rest = model.strip_prefix(prefix)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        return None;
    }
    Some(digits.parse::<u32>().unwrap_or(u32::MAX))
}

/// Heuristic dispatch based on the model name. The rules:
///
/// - Models whose name contains `chat` (e.g. `gpt-5-chat-latest`) ride
///   on Chat Completions even when the family otherwise uses Responses.
///   This is checked first, so it holds for every generation.
/// - O-series reasoning models (`o1*`, `o3*`, `o4*`, and any later `o<N>`)
///   are Responses-only.
/// - GPT generation `FIRST_RESPONSES_GPT_GENERATION` **and above** default
///   to Responses — `gpt-5`, `gpt-5.6-terra`, `gpt-6`, `gpt-7`, `gpt-10`.
/// - Everything else defaults to Chat Completions (most permissive,
///   widest 3rd-party-proxy support): `gpt-4o`, `gpt-4.1`, `gpt-3.5-turbo`.
///
/// **Why the comparison is `>=` rather than a list of known families.** The
/// rule used to name `gpt-5` by prefix, which silently routed every later
/// family — `gpt-6`, `gpt-7` — back to Chat Completions, the surface those
/// models are least likely to support. That failure is quiet: the request is
/// well-formed, so the operator sees a provider-side rejection rather than a
/// dispatch bug. Reading the generation number makes a newer family the
/// *default-forward* case, and the direction of the remaining guess is the
/// safer one. A model this gets wrong is still pinned by
/// `TRANSYNC_OPENAI_API` or [`crate::TransyncOpenAI::with_api`].
///
/// TRACE: SCN-12
/// TRACE: DCR-0059
pub fn api_for_model(model: &str) -> Api {
    let m = model.to_ascii_lowercase();
    if m.contains("chat") {
        return Api::ChatCompletions;
    }
    if generation_after(&m, "o").is_some() {
        return Api::Responses;
    }
    if generation_after(&m, "gpt-").is_some_and(|g| g >= FIRST_RESPONSES_GPT_GENERATION) {
        return Api::Responses;
    }
    Api::ChatCompletions
}

/// Pick the API surface, honoring `TRANSYNC_OPENAI_API` if set and
/// falling back to [`api_for_model`].
///
/// **This reads the environment, so call it exactly once per adapter.**
/// [`crate::TransyncOpenAI::new`] does, storing the answer; `fingerprint()`
/// and both request paths then read that field. The previous arrangement
/// called this from each of them independently and *asserted* that
/// `TRANSYNC_OPENAI_API` holds still for the life of the process — an
/// unenforceable claim about a library's host, and one a process that
/// mutates the variable between building a cache key and issuing the
/// request could break, naming one surface in the fingerprint while
/// calling the other (R0001-0031). Storing the value makes the claim true
/// by construction instead of by hope.
///
/// It is also not the only way to choose a surface: the variable is
/// process-global, so a host running two adapters on two surfaces at once
/// pins the second one with [`crate::TransyncOpenAI::with_api`] instead of
/// mutating the environment between constructions (ticket `42c8e6d3`).
///
/// The only remaining per-call callers are [`super::call_api`] and
/// [`super::call_glossary_extraction`], free functions with no instance to
/// remember anything in; their docs say so.
///
/// EXT-2026-07 P1-4
pub(crate) fn api_from_env_or_model(model: &str) -> Api {
    api_from_override_or_model(std::env::var("TRANSYNC_OPENAI_API").ok().as_deref(), model)
}

/// The environment-free half of [`api_from_env_or_model`]: resolve an
/// already-read override, or fall back to the model-name heuristic. Split
/// out so the override's vocabulary — including the warning an
/// unrecognized value earns — is testable without `set_var`, which is
/// `unsafe` and racy against every other test in the binary.
fn api_from_override_or_model(raw: Option<&str>, model: &str) -> Api {
    if let Some(raw) = raw {
        match parse_api(raw) {
            Some(api) => return api,
            // R0006-0017: a typo like `TRANSYNC_OPENAI_API=response`
            // silently fell through to the heuristic; surface it so the
            // operator learns the override never took effect.
            None => tracing::warn!(
                target: "transync::openai",
                "unrecognized TRANSYNC_OPENAI_API value {raw:?} (expected \"chat\" or \"responses\"); falling back to the model-name heuristic"
            ),
        }
    }
    api_for_model(model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_dispatch_heuristic_covers_gpt5_generations() {
        assert_eq!(api_for_model("gpt-5-chat-latest"), Api::ChatCompletions);
        for model in [
            "gpt-5",
            "gpt-5.0",
            "gpt-5.1",
            "gpt-5.2",
            "gpt-5.3",
            "gpt-5.4",
            "gpt-5.5",
            "gpt-5.6-terra",
            "gpt-5.7-future",
        ] {
            assert_eq!(
                api_for_model(model),
                Api::Responses,
                "{model} must use the Responses API"
            );
        }
        assert_eq!(api_for_model("o3-mini"), Api::Responses);
        assert_eq!(api_for_model("gpt-4o"), Api::ChatCompletions);
    }

    /// DCR-0059: the rule is a floor, not a roster. Every GPT generation from
    /// [`FIRST_RESPONSES_GPT_GENERATION`] up dispatches to Responses without
    /// an edit to this crate — which is the whole point, since the previous
    /// `starts_with("gpt-5")` sent `gpt-6` to Chat Completions, the surface a
    /// newer family is least likely to accept.
    #[test]
    fn a_generation_past_gpt5_rides_responses_without_being_listed() {
        for model in [
            "gpt-6",
            "gpt-6-mini",
            "gpt-6.1",
            "gpt-7",
            "gpt-7.4-preview",
            "gpt-10",
            "gpt-12.0-something",
        ] {
            assert_eq!(
                api_for_model(model),
                Api::Responses,
                "{model} is a generation past GPT-{FIRST_RESPONSES_GPT_GENERATION} and must use Responses"
            );
        }
        // O-series, likewise: the rule reads the number, not a list of three.
        for model in ["o1", "o1-preview", "o3-mini", "o4-mini", "o5", "o6-pro"] {
            assert_eq!(
                api_for_model(model),
                Api::Responses,
                "{model} is an O-series reasoning model and must use Responses"
            );
        }
    }

    /// The `chat` rule is checked before the generation rule, so it keeps
    /// holding for families that do not exist yet — a future `gpt-6-chat-*`
    /// snapshot is a chat-surface model exactly as `gpt-5-chat-latest` is.
    #[test]
    fn a_chat_snapshot_stays_on_chat_completions_in_every_generation() {
        for model in [
            "gpt-5-chat-latest",
            "gpt-6-chat-latest",
            "gpt-7-chat",
            "gpt-10-chat-preview",
        ] {
            assert_eq!(
                api_for_model(model),
                Api::ChatCompletions,
                "{model} names a chat snapshot and must stay on Chat Completions"
            );
        }
    }

    /// The other direction: older families, and names that only look like a
    /// generation, must NOT be dragged onto Responses. `gpt-4o` ends in a
    /// letter `o` and `gpt-oss` begins with one — neither declares a number
    /// after its prefix, so neither parses as a generation.
    #[test]
    fn older_and_numberless_families_stay_on_chat_completions() {
        for model in [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4.1",
            "gpt-4",
            "gpt-3.5-turbo",
            "gpt-oss-120b",
            "omni-moderation-latest",
            "text-embedding-3-large",
        ] {
            assert_eq!(
                api_for_model(model),
                Api::ChatCompletions,
                "{model} must stay on Chat Completions"
            );
        }
    }

    /// A digit run too long for `u32` saturates rather than failing to parse.
    /// Parsing failure would fall through to Chat Completions — the wrong
    /// direction for something that is, whatever else it is, a later
    /// generation than 5.
    #[test]
    fn an_absurd_generation_saturates_forward_rather_than_falling_back() {
        assert_eq!(
            generation_after("gpt-99999999999999999999", "gpt-"),
            Some(u32::MAX)
        );
        assert_eq!(
            api_for_model("gpt-99999999999999999999"),
            Api::Responses,
            "an unparseable-but-large generation is still a later one"
        );
        assert_eq!(generation_after("gpt-oss", "gpt-"), None);
        assert_eq!(generation_after("o4-mini", "o"), Some(4));
        assert_eq!(generation_after("gpt-5.6-terra", "gpt-"), Some(5));
    }

    /// Ticket `42c8e6d3`: a caller-supplied surface string parses exactly
    /// the way `TRANSYNC_OPENAI_API` does. Asserted against
    /// `api_from_override_or_model` — the *environment* path with the
    /// `var()` read lifted out — rather than against a second copy of the
    /// token list, so a token added to one and not the other fails here.
    /// `o3-mini` is the model, so the heuristic would say `Responses`: any
    /// `ChatCompletions` below is the override winning, not a coincidence.
    #[test]
    fn from_str_accepts_exactly_the_env_override_vocabulary() {
        for (raw, expected) in [
            ("chat", Api::ChatCompletions),
            ("chat_completions", Api::ChatCompletions),
            ("chatcompletions", Api::ChatCompletions),
            ("responses", Api::Responses),
            // Case- and whitespace-insensitive on both paths.
            ("  Chat_Completions ", Api::ChatCompletions),
            ("RESPONSES", Api::Responses),
        ] {
            assert_eq!(raw.parse::<Api>(), Ok(expected), "{raw:?} must parse");
            assert_eq!(
                api_from_override_or_model(Some(raw), "o3-mini"),
                expected,
                "{raw:?} must override the model-name heuristic identically"
            );
        }

        // …and reject identically, falling back to the heuristic when it is
        // the environment that carried the bad value.
        for raw in ["response", "", "chat completions", "v1/chat/completions"] {
            assert!(raw.parse::<Api>().is_err(), "{raw:?} must not parse");
            assert_eq!(
                api_from_override_or_model(Some(raw), "o3-mini"),
                Api::Responses,
                "{raw:?} must fall back to the model-name heuristic"
            );
        }

        // No override at all is the heuristic, unchanged.
        assert_eq!(
            api_from_override_or_model(None, "gpt-4o"),
            Api::ChatCompletions
        );
    }

    /// The canonical name round-trips, and is the token the fingerprint
    /// field carries (pinned end-to-end in `crate::tests`).
    #[test]
    fn canonical_name_round_trips() {
        for api in [Api::ChatCompletions, Api::Responses] {
            assert_eq!(api.as_str().parse::<Api>(), Ok(api));
            assert_eq!(api.to_string(), api.as_str());
        }
        assert_eq!(Api::ChatCompletions.as_str(), "chat");
        assert_eq!(Api::Responses.as_str(), "responses");
    }
}
