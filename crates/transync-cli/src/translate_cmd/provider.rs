//! Provider construction for `transync translate` — argument resolution is
//! shared by the in-process stub and live OpenAI/Google adapters.
//!
//! TRACE: SCN-12

use clap::ValueEnum;
#[cfg(not(feature = "test-stub-provider"))]
use secrecy::SecretString;
#[cfg(not(feature = "test-stub-provider"))]
use transync_google::GoogleCredentials;
use transync_google::{
    GoogleConfig, GoogleSurface, ModelId as GoogleModelId, ThinkingLevel, TransyncGoogle,
    VertexLocation,
};
use transync_openai::ModelId;
use url::Url;

use super::TranslateArgs;
use super::args::resolve_model;

const GOOGLE_DEFAULT_MODEL: &str = "gemini-3.8-flash";

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ProviderArg {
    Openai,
    Google,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GoogleSurfaceArg {
    Vertex,
    Developer,
}

/// How a Vertex AI run authenticates. Explicit on purpose — see
/// `--google-auth` in `translate_cmd`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GoogleAuthArg {
    /// Application Default Credentials (the default).
    Adc,
    /// A Vertex AI express-mode API key, read from `GOOGLE_API_KEY`.
    ApiKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GoogleLocationArg {
    Global,
    Us,
    Eu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GoogleThinkingArg {
    Low,
    Medium,
    High,
}

pub(crate) enum ProviderConfig {
    OpenAi {
        model: ModelId,
        base_url: Option<Url>,
    },
    Google(GoogleConfig),
}

impl ProviderConfig {
    pub(crate) fn model_id(&self) -> &str {
        match self {
            Self::OpenAi { model, .. } => &model.0,
            Self::Google(config) => &config.model.0,
        }
    }
}

/// OpenAI model and base URL validation retained from R0010-0029. The selected
/// provider is resolved by [`resolve_provider_config`] before choosing transport.
///
/// The `test-stub-provider` feature exists so the smoke suites can drive **this
/// CLI**; a stub arm that answers before the arguments are parsed tests a CLI
/// with a different preflight from the shipped one. A `--base-url` that is not
/// a URL, or a `--model` [`ModelId::parse`] refuses, used to reach the echo
/// translator and exit 0 under the feature while the live build refused the
/// same argv at exit 1. So everything that depends on the *arguments* rather
/// than on the transport is answered here, where both arms run it. The stub
/// also constructs the Google offline adapter to keep its validation aligned
/// with the live path.
///
/// ti `30a744`: the base URL is resolved BEFORE the credential is demanded,
/// because an offline run needs the former and must not be asked for the
/// latter. The order is the whole fix — it used to be reversed, so a run that
/// was going to make no provider call still could not start.
///
/// The live adapter later validates scheme, host and userinfo through its
/// constructor. The stub preserves the existing OpenAI argument checks here.
fn provider_config(model: &str, base_url: Option<&str>) -> Result<(ModelId, Option<Url>), String> {
    let base_url = match base_url {
        Some(raw) => Some(Url::parse(raw).map_err(|e| format!("invalid --base-url: {e}"))?),
        None => match std::env::var("TRANSYNC_OPENAI_BASE_URL")
            .ok()
            .filter(|s| !s.is_empty())
        {
            Some(raw) => Some(
                Url::parse(&raw).map_err(|e| format!("invalid TRANSYNC_OPENAI_BASE_URL: {e}"))?,
            ),
            None => None,
        },
    };
    // R0002-0037 / R0003-0009: one rule for "this identifier names a model",
    // stated on `ModelId` and reused rather than restated.
    let model = ModelId::parse(model).map_err(|e| e.to_string())?;
    Ok((model, base_url))
}

fn env_value(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

fn env_choice<T: ValueEnum>(name: &str) -> Result<Option<T>, String> {
    env_value(name)
        .map(|value| T::from_str(&value, false).map_err(|e| format!("invalid {name}: {e}")))
        .transpose()
}

fn resolved_base_url(flag: Option<&str>, env_name: &str) -> Result<Option<Url>, String> {
    let (raw, source) = match flag {
        Some(raw) => (Some(raw.to_string()), "--base-url"),
        None => (env_value(env_name), env_name),
    };
    raw.map(|value| Url::parse(&value).map_err(|e| format!("invalid {source}: {e}")))
        .transpose()
}

/// Resolve the selected provider before reading an input file or credential.
/// The same path runs under `test-stub-provider`, so a test build cannot hide
/// an invalid model, URL, project, location, or thinking value.
pub(crate) fn resolve_provider_config(args: &TranslateArgs) -> Result<ProviderConfig, String> {
    match args.provider {
        ProviderArg::Openai => {
            if args.google_surface.is_some()
                || args.google_auth.is_some()
                || args.google_project.is_some()
                || args.google_location.is_some()
                || args.google_thinking.is_some()
            {
                return Err("--google-* flags require --provider google".to_string());
            }
            let model = resolve_model(args.model.as_deref());
            let (model, base_url) = provider_config(&model, args.base_url.as_deref())?;
            Ok(ProviderConfig::OpenAi { model, base_url })
        }
        ProviderArg::Google => {
            let surface = match args.google_surface {
                Some(surface) => surface,
                None => env_choice("TRANSYNC_GOOGLE_SURFACE")?.unwrap_or(GoogleSurfaceArg::Vertex),
            };
            let model = args
                .model
                .clone()
                .or_else(|| env_value("TRANSYNC_GOOGLE_MODEL"))
                .unwrap_or_else(|| GOOGLE_DEFAULT_MODEL.to_string());
            let model = GoogleModelId::parse(model).map_err(|e| e.to_string())?;
            let thinking = match args.google_thinking {
                Some(thinking) => thinking,
                None => env_choice("TRANSYNC_GOOGLE_THINKING")?.unwrap_or(GoogleThinkingArg::Low),
            };
            let thinking_level = Some(match thinking {
                GoogleThinkingArg::Low => ThinkingLevel::Low,
                GoogleThinkingArg::Medium => ThinkingLevel::Medium,
                GoogleThinkingArg::High => ThinkingLevel::High,
            });
            let (surface, base_env) = match surface {
                GoogleSurfaceArg::Developer => {
                    if args.google_auth.is_some() {
                        return Err(
                            "--google-auth applies to --google-surface vertex only; the Developer \
                             API always authenticates with GEMINI_API_KEY"
                                .to_string(),
                        );
                    }
                    if args.google_project.is_some() || args.google_location.is_some() {
                        return Err(
                            "--google-project and --google-location require --google-surface vertex"
                                .to_string(),
                        );
                    }
                    (
                        GoogleSurface::Developer,
                        "TRANSYNC_GOOGLE_DEVELOPER_BASE_URL",
                    )
                }
                GoogleSurfaceArg::Vertex if args.google_auth == Some(GoogleAuthArg::ApiKey) => {
                    // Vertex in express mode: same service and models, reached
                    // with an API key. Its URL carries no project and no
                    // location, so an explicitly passed one would be silently
                    // dropped — refuse instead. An ambient GOOGLE_CLOUD_PROJECT
                    // is ignored rather than refused: it is environment, not a
                    // request.
                    if args.google_project.is_some() || args.google_location.is_some() {
                        return Err("--google-project and --google-location do not apply to \
                             --google-auth api-key: Vertex express mode addresses the publisher \
                             model directly, with no project or location in the request"
                            .to_string());
                    }
                    tracing::info!(
                        target: "transync::provider",
                        "Google: Vertex AI in express mode (--google-auth api-key); project and location do not apply"
                    );
                    (
                        GoogleSurface::VertexExpress,
                        "TRANSYNC_GOOGLE_VERTEX_BASE_URL",
                    )
                }
                GoogleSurfaceArg::Vertex => {
                    let project = args
                        .google_project
                        .clone()
                        .or_else(|| env_value("GOOGLE_CLOUD_PROJECT"))
                        .ok_or_else(|| {
                            "Vertex AI needs --google-project or GOOGLE_CLOUD_PROJECT".to_string()
                        })?;
                    if project.trim().is_empty() || project.trim() != project {
                        return Err("Google project must be non-empty and unpadded".to_string());
                    }
                    let location = match args.google_location {
                        Some(location) => location,
                        None => env_choice("GOOGLE_CLOUD_LOCATION")?
                            .unwrap_or(GoogleLocationArg::Global),
                    };
                    let location = match location {
                        GoogleLocationArg::Global => VertexLocation::Global,
                        GoogleLocationArg::Us => VertexLocation::Us,
                        GoogleLocationArg::Eu => VertexLocation::Eu,
                    };
                    (
                        GoogleSurface::Vertex { project, location },
                        "TRANSYNC_GOOGLE_VERTEX_BASE_URL",
                    )
                }
            };
            let base_url = resolved_base_url(args.base_url.as_deref(), base_env)?;
            Ok(ProviderConfig::Google(GoogleConfig {
                surface,
                model,
                base_url,
                thinking_level,
            }))
        }
    }
}

/// Pick the in-process echo translator after validating the selected
/// provider's configuration. No key or network is needed under this feature.
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
pub(crate) fn translator_for_run(
    config: ProviderConfig,
    // ti `30a744`: accepted and unused. The stub needs no credential, so it is
    // already what `--offline` asks the live build to become; honouring the
    // flag by refusing to translate would make the stub useless for the smoke
    // suites that exist to exercise the translating path. The flag's argument
    // guard still runs, so `--offline` without `--cache-dir` is refused in
    // both builds.
    _offline: bool,
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    // Validate Google construction even under the stub; a malformed endpoint
    // or project must be refused on the same argument path as the live build.
    let record = match &config {
        ProviderConfig::OpenAi { model, base_url } => serde_json::json!({
            "provider": "openai",
            "model": model.0.as_str(),
            "base_url": base_url.as_ref().map(Url::as_str),
        }),
        ProviderConfig::Google(google) => {
            let (surface, project, location) = match &google.surface {
                GoogleSurface::Developer => ("developer", None, None),
                GoogleSurface::Vertex { project, location } => {
                    let location = match location {
                        VertexLocation::Global => "global",
                        VertexLocation::Us => "us",
                        VertexLocation::Eu => "eu",
                    };
                    ("vertex", Some(project.as_str()), Some(location))
                }
                GoogleSurface::VertexExpress => ("vertex-express", None, None),
            };
            let thinking = google.thinking_level.as_ref().map(|level| match level {
                ThinkingLevel::Low => "low",
                ThinkingLevel::Medium => "medium",
                ThinkingLevel::High => "high",
            });
            serde_json::json!({
                "provider": "google",
                "surface": surface,
                "project": project,
                "location": location,
                "thinking": thinking,
                "model": google.model.0.as_str(),
                "base_url": google.base_url.as_ref().map(Url::as_str),
            })
        }
    };
    if let ProviderConfig::Google(google) = config {
        TransyncGoogle::offline(google).map_err(|e| e.to_string())?;
    }
    // Test-only side channel: record the resolved provider axes so subprocess
    // smoke tests can prove that flags and selected environment values reached
    // this seam. The record never contains a credential.
    if let Ok(path) = std::env::var("TRANSYNC_STUB_ECHO_PATH") {
        std::fs::write(
            &path,
            serde_json::to_vec_pretty(&record).unwrap_or_default(),
        )
        .map_err(|e| format!("could not write TRANSYNC_STUB_ECHO_PATH {path}: {e}"))?;
    }
    // `R0001-0086` in the removed `reviews/reviewed/0001.md`, extended by ti
    // `e62b59`: TRANSYNC_STUB_MODE swaps the echo stub for one that fails in
    // a *chosen* way, so every exit code that needs a provider failure can be
    // driven end-to-end — through the pipeline, through the CLI's
    // classification, out as a process exit status — without a live provider.
    if let Ok(mode) = std::env::var("TRANSYNC_STUB_MODE")
        && !mode.is_empty()
    {
        return stub_for_mode(&mode);
    }
    // DCR-0028 / OI-0017: TRANSYNC_STUB_DETECT=<label> makes the stub report
    // that language on every envelope, so the `--cache-dir` scenario can give
    // two runs *different* answers and prove the second one replayed the
    // first's detection instead of asking again.
    if let Ok(lang) = std::env::var("TRANSYNC_STUB_DETECT")
        && !lang.is_empty()
    {
        return Ok(Box::new(transync::test_stub::DetectingEchoTranslator {
            language: Some(lang),
        }) as Box<dyn transync::Translator + Send + Sync>);
    }
    // ti `dca5bf`: TRANSYNC_STUB_GLOSSARY=`src=tgt,src=tgt` makes the stub
    // answer the auto-glossary preflight with that harvest, so the
    // `--cache-dir` scenario can give two runs *different* answers and prove
    // the second one replayed the first's harvest instead of asking again.
    if let Ok(spec) = std::env::var("TRANSYNC_STUB_GLOSSARY")
        && !spec.is_empty()
    {
        let terms = spec
            .split(',')
            .filter_map(|pair| pair.split_once('='))
            .map(|(s, t)| (s.trim().to_string(), t.trim().to_string()))
            .collect();
        return Ok(
            Box::new(transync::test_stub::ExtractingEchoTranslator { terms })
                as Box<dyn transync::Translator + Send + Sync>,
        );
    }
    Ok(Box::new(transync::test_stub::EchoTranslator)
        as Box<dyn transync::Translator + Send + Sync>)
}

/// Resolve a `TRANSYNC_STUB_MODE` value to the translator that drives it.
///
/// Every mode but `fail` raises a **terminal** `TranslatorError`, which
/// ADR-0017 makes a whole-run abort, so each one lands on exactly one exit
/// code (`contracts.md` §6): `auth` / `provider-rejected` / `output-ceiling`
/// / `context-window` on `6`, `content-filtered` / `model-refused` on `7`,
/// and `unclassified` on the residual `5`. `fail` is the older mode and is
/// different in kind — it answers *successfully* with
/// `FailedNeedsFallback` for every unit, which is the exit-`3` path.
///
/// An unrecognized mode is an **error**, not a fall-through to the echo
/// stub. A test that misspells its mode would otherwise get a perfectly
/// green translation and assert an exit code against a run that never
/// failed.
///
/// TRACE: ti e62b59
#[cfg(feature = "test-stub-provider")]
fn stub_for_mode(mode: &str) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    use transync::TranslatorError;
    use transync::test_stub::{AlwaysFailsTranslator, TerminalErrorTranslator};

    fn terminal(
        make: fn() -> TranslatorError,
    ) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
        Ok(Box::new(TerminalErrorTranslator::new(make))
            as Box<dyn transync::Translator + Send + Sync>)
    }

    match mode {
        "fail" => {
            Ok(Box::new(AlwaysFailsTranslator) as Box<dyn transync::Translator + Send + Sync>)
        }
        "auth" => terminal(|| TranslatorError::Authentication("stub: key rejected".into())),
        "provider-rejected" => terminal(|| TranslatorError::ProviderRejected {
            status: Some(404),
            message: "stub: the model does not exist".into(),
        }),
        "output-ceiling" => {
            terminal(|| TranslatorError::OutputCeilingExhausted("stub: answer was cut off".into()))
        }
        "context-window" => terminal(|| {
            TranslatorError::ContextWindowExceeded("stub: request did not fit the window".into())
        }),
        "content-filtered" => terminal(|| {
            TranslatorError::ContentFiltered("stub: content policy stopped generation".into())
        }),
        "model-refused" => {
            terminal(|| TranslatorError::ModelRefused("stub: the model declined".into()))
        }
        "unclassified" => {
            terminal(|| TranslatorError::Other("stub: unclassified provider failure".into()))
        }
        other => Err(format!(
            "unknown TRANSYNC_STUB_MODE {other:?}; expected one of fail, auth, \
             provider-rejected, output-ceiling, context-window, content-filtered, \
             model-refused, unclassified"
        )),
    }
}

/// TRACE: SCN-12
#[cfg(not(feature = "test-stub-provider"))]
pub(crate) fn translator_for_run(
    config: ProviderConfig,
    offline: bool,
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    match config {
        ProviderConfig::OpenAi { model, base_url } => {
            // Offline and credentialed instances must have the same cache
            // fingerprint; the adapter constructors own that invariant.
            let openai = if offline {
                transync_openai::TransyncOpenAI::offline(model, base_url)
            } else {
                let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
                    "OPENAI_API_KEY not set; set it, pass --offline to run from cache alone, \
                     or build --features test-stub-provider"
                        .to_string()
                })?;
                transync_openai::TransyncOpenAI::try_new(
                    SecretString::new(key.into()),
                    model,
                    base_url,
                )
            }
            .map_err(|e| e.to_string())?;
            Ok(Box::new(openai))
        }
        ProviderConfig::Google(config) => {
            let google = if offline {
                TransyncGoogle::offline(config)
            } else {
                let credentials = match &config.surface {
                    GoogleSurface::Developer => {
                        let key = std::env::var("GEMINI_API_KEY").map_err(|_| {
                            "GEMINI_API_KEY not set for Google Developer API".to_string()
                        })?;
                        GoogleCredentials::DeveloperApiKey(SecretString::new(key.into()))
                    }
                    GoogleSurface::Vertex { .. } => GoogleCredentials::VertexAdc,
                    GoogleSurface::VertexExpress => {
                        let key = std::env::var("GOOGLE_API_KEY").map_err(|_| {
                            "GOOGLE_API_KEY not set for Vertex AI express mode".to_string()
                        })?;
                        GoogleCredentials::VertexApiKey(SecretString::new(key.into()))
                    }
                };
                TransyncGoogle::try_new(config, credentials)
            }
            .map_err(|e| e.to_string())?;
            Ok(Box::new(google))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// R0010-0029: the argument-side refusals are the same refusals in both
    /// builds — this module's cfg split picks a transport, and must not be able
    /// to change the answer to a question about argv. Every case here passes an
    /// explicit `--base-url`, so nothing reads the ambient environment.
    #[test]
    fn the_argument_side_configuration_is_validated_in_both_builds() {
        assert!(provider_config("gpt-5", Some("https://api.example/v1")).is_ok());
        assert!(
            provider_config("gpt-5", Some("not a url")).is_err(),
            "a --base-url that is not a URL is refused before a transport is chosen"
        );
        assert!(
            provider_config("", Some("https://api.example/v1")).is_err(),
            "R0002-0037: an identifier that names no model is the configuration's fault"
        );
        assert!(
            provider_config(" gpt-5 ", Some("https://api.example/v1")).is_err(),
            "R0003-0009: a padded identifier is two cache namespaces for one model"
        );
    }
}
