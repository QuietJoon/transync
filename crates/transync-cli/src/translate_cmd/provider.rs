//! Provider construction for `transync translate` — the one seam with two
//! cfg-gated implementations (in-process stub vs. live OpenAI client).
//!
//! TRACE: SCN-12

#[cfg(not(feature = "test-stub-provider"))]
use secrecy::SecretString;
use transync_openai::ModelId;
use url::Url;

/// The provider configuration both builds resolve, before either one picks a
/// transport (R0010-0029).
///
/// The `test-stub-provider` feature exists so the smoke suites can drive **this
/// CLI**; a stub arm that answers before the arguments are parsed tests a CLI
/// with a different preflight from the shipped one. A `--base-url` that is not
/// a URL, or a `--model` [`ModelId::parse`] refuses, used to reach the echo
/// translator and exit 0 under the feature while the live build refused the
/// same argv at exit 1. So everything that depends on the *arguments* rather
/// than on the transport is answered here, where both arms run it — the stub
/// discards the product, the live build passes it on.
///
/// ti `30a744`: the base URL is resolved BEFORE the credential is demanded,
/// because an offline run needs the former and must not be asked for the
/// latter. The order is the whole fix — it used to be reversed, so a run that
/// was going to make no provider call still could not start.
///
/// What stays with the live arm is what needs a live adapter to answer:
/// `TransyncOpenAI::{try_new, offline}` re-run [`ModelId::parse`] (idempotent)
/// and additionally validate the URL's scheme, host and userinfo through a
/// rule this crate cannot reach.
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

/// Pick the right `Translator` for the current build configuration.
///
/// With the `test-stub-provider` feature, an in-process echo translator
/// is used (no network, no API key). Without it, `provider_config` resolves
/// the model and base URL first, then this builds either
/// `TransyncOpenAI::offline` (under `--offline`, credential-free) or
/// `TransyncOpenAI::try_new` from `OPENAI_API_KEY` — not `from_env()`, which
/// would re-read the environment this function has already resolved
/// (ti `30a744`).
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
pub(crate) fn translator_for_run(
    model: &str,
    base_url: Option<&str>,
    // ti `30a744`: accepted and unused. The stub needs no credential, so it is
    // already what `--offline` asks the live build to become; honouring the
    // flag by refusing to translate would make the stub useless for the smoke
    // suites that exist to exercise the translating path. The flag's argument
    // guard still runs, so `--offline` without `--cache-dir` is refused in
    // both builds.
    _offline: bool,
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    // R0010-0029: the argument-side half of provider construction runs here
    // too, and its product is discarded — the stub configures no transport,
    // but a `--model` or `--base-url` the shipped binary refuses must not go
    // green under the feature that exists to test the shipped binary.
    provider_config(model, base_url)?;
    // OI-0023 item 2: with no live provider to inspect, record the model +
    // base-url this stub provider was handed — the same pair the live build
    // forwards to `TransyncOpenAI::try_new` — to the path in
    // `TRANSYNC_STUB_ECHO_PATH`, so a subprocess smoke test can observe that
    // the `--model` / `--base-url` CLI flags reached the provider. Test-only
    // side channel, gated behind the `test-stub-provider` feature.
    if let Ok(path) = std::env::var("TRANSYNC_STUB_ECHO_PATH") {
        let record = serde_json::json!({ "model": model, "base_url": base_url });
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
    model: &str,
    base_url: Option<&str>,
    offline: bool,
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    // R0010-0029: the base URL and the model id are parsed by the function
    // both builds share, which is also where ti `30a744`'s "base URL before
    // credential" ordering now lives.
    let (model, base_url) = provider_config(model, base_url)?;
    // The two instances differ in exactly one field, and deliberately not in
    // any field `fingerprint()` reads: an offline run has to look in the
    // namespace the warming run wrote, or it misses everything and reads as a
    // corrupt cache. `TransyncOpenAI::offline` owns that guarantee and
    // `an_offline_instance_fingerprints_identically_to_a_credentialed_one`
    // pins it, so this seam cannot re-derive the composition and drift.
    let openai = if offline {
        transync_openai::TransyncOpenAI::offline(model, base_url)
    } else {
        let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
            "OPENAI_API_KEY not set; set it, pass --offline to run from cache alone, \
             or build --features test-stub-provider"
                .to_string()
        })?;
        transync_openai::TransyncOpenAI::try_new(SecretString::new(key.into()), model, base_url)
    }
    .map_err(|e| e.to_string())?;
    Ok(Box::new(openai) as Box<dyn transync::Translator + Send + Sync>)
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
