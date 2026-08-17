//! Provider construction for `transync translate` — the one seam with two
//! cfg-gated implementations (in-process stub vs. live OpenAI client).
//!
//! TRACE: SCN-12

#[cfg(not(feature = "test-stub-provider"))]
use secrecy::SecretString;
#[cfg(not(feature = "test-stub-provider"))]
use transync_openai::ModelId;
#[cfg(not(feature = "test-stub-provider"))]
use url::Url;

/// Pick the right `Translator` for the current build configuration.
///
/// With the `test-stub-provider` feature, an in-process echo translator
/// is used (no network, no API key). Without it, the live OpenAI client
/// is constructed via `from_env()`.
///
/// TRACE: SCN-12
#[cfg(feature = "test-stub-provider")]
pub(crate) fn translator_for_run(
    model: &str,
    base_url: Option<&str>,
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
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
) -> Result<Box<dyn transync::Translator + Send + Sync>, String> {
    let key = std::env::var("OPENAI_API_KEY").map_err(|_| {
        "OPENAI_API_KEY not set; set it or build --features test-stub-provider".to_string()
    })?;
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
    let openai = transync_openai::TransyncOpenAI::try_new(
        SecretString::new(key.into()),
        ModelId::new(model),
        base_url,
    )
    .map_err(|e| e.to_string())?;
    Ok(Box::new(openai) as Box<dyn transync::Translator + Send + Sync>)
}
