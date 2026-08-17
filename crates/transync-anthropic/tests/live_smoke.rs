//! Gated live-endpoint smoke (OI-0030 pattern) for the Anthropic adapter.
//!
//! One full `translate()` round-trip against the real Messages API, so
//! passing means every validation layer (schema, ID-set, per-kind, fragment
//! reparse, inline protection, full reparse) accepted a genuine provider
//! response — machine-asserted, no eyeballing.
//!
//! **These tests never run by accident.** Two independent gates must both
//! open:
//!
//! 1. `#[ignore]` — invisible to `cargo test` / `cargo test --workspace`.
//! 2. [`live_gate`] — `TRANSYNC_LIVE_SMOKE=1` *and* a non-empty
//!    `ANTHROPIC_API_KEY`. A blanket `cargo test -- --include-ignored`
//!    without the opt-in is a no-op skip, not a failure and not a call.
//!
//! Run them with `scripts/smoke-live-gate.sh anthropic`, which supplies gate
//! 1 via `--ignored` and gate 2 via the environment of its own child process.
//!
//! **No default-run test may pass because the key is absent.** The gate
//! predicate itself is unit-tested offline (`gate_requires_both_opt_in_and_api_key`)
//! so it cannot rot into vacuity, and the fixture's shape is pinned offline
//! (`live_source_fixture_has_the_expected_shape`) so the live assertions
//! cannot drift into asserting nothing. Absence of a key is a printed skip on
//! the ignored path only.
//!
//! Environment knobs:
//!
//! - `TRANSYNC_LIVE_SMOKE=1` — the opt-in (required).
//! - `ANTHROPIC_API_KEY` — credentials (required).
//! - `TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL` — default `claude-haiku-4-5`.
//! - `TRANSYNC_ANTHROPIC_BASE_URL` — honored like `TransyncAnthropic::from_env`.
//!
//! TRACE: OI-0030
//! TRACE: DCR-0029
//! TRACE: SCN-12

use secrecy::SecretString;
use transync::{TranslateOptions, translate};
use transync_anthropic::{ModelId, TransyncAnthropic};
use url::Url;

/// Heading + paragraph + a two-item list, in a single batch, well under 1k
/// tokens per run. Kept deliberately tiny: the point is endpoint +
/// validation-stack evidence, not translation quality.
const LIVE_SOURCE: &str =
    "# Smoke\n\nOne short paragraph about nothing.\n\n- first item\n- second item\n";

/// Translation units [`LIVE_SOURCE`] produces: `Heading1`, `Paragraph`, and
/// one `ListItem` per bullet — the IR has no list-container kind, so the
/// two-item list contributes two units, not one.
///
/// Pinned by [`live_source_fixture_has_the_expected_shape`], which runs
/// offline, so the live assertions below cannot silently drift into vacuity.
const LIVE_SOURCE_UNITS: u32 = 4;

/// The cheapest model that supports this provider's structured outputs. The
/// live round-trip is endpoint-plus-validation-stack evidence, not quality
/// evidence, so paying for a larger model buys nothing this test asserts.
const DEFAULT_LIVE_MODEL: &str = "claude-haiku-4-5";

/// What the two gate layers decided. Split out as a pure function of the two
/// environment values so the gate is unit-testable without touching the
/// process environment.
#[derive(Debug, PartialEq, Eq)]
enum Gate {
    Run(String),
    SkipNoOptIn,
    SkipNoKey,
}

/// The gate predicate: the opt-in must be exactly `"1"`, and the API key must
/// be present and non-empty. Anything else skips.
fn decide_gate(opt_in: Option<&str>, api_key: Option<&str>) -> Gate {
    if opt_in != Some("1") {
        return Gate::SkipNoOptIn;
    }
    match api_key {
        Some(key) if !key.is_empty() => Gate::Run(key.to_owned()),
        _ => Gate::SkipNoKey,
    }
}

/// Second gate layer. `None` = skip this test as a no-op (printed, not
/// failed); `Some(key)` = the caller explicitly opted in to spending real
/// tokens.
fn live_gate() -> Option<SecretString> {
    let opt_in = std::env::var("TRANSYNC_LIVE_SMOKE").ok();
    let api_key = std::env::var("ANTHROPIC_API_KEY").ok();
    match decide_gate(opt_in.as_deref(), api_key.as_deref()) {
        Gate::Run(key) => Some(SecretString::new(key.into())),
        Gate::SkipNoOptIn => {
            eprintln!("live_smoke: skipped (set TRANSYNC_LIVE_SMOKE=1 to opt in)");
            None
        }
        Gate::SkipNoKey => {
            eprintln!("live_smoke: skipped (ANTHROPIC_API_KEY not set)");
            None
        }
    }
}

/// Honor `TRANSYNC_ANTHROPIC_BASE_URL` the way `TransyncAnthropic::from_env`
/// does, while still constructing explicitly so the *cheap* default model is
/// used instead of `from_env`'s `claude-opus-5`.
fn base_url_from_env() -> Option<Url> {
    match std::env::var("TRANSYNC_ANTHROPIC_BASE_URL") {
        Ok(raw) if !raw.is_empty() => Some(
            Url::parse(&raw).expect("TRANSYNC_ANTHROPIC_BASE_URL must be a valid absolute URL"),
        ),
        _ => None,
    }
}

fn model_from_env(var: &str, default: &str) -> String {
    std::env::var(var)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// One full round-trip. Assertions are *structural*, so model nondeterminism
/// cannot make them flap.
async fn assert_round_trip(api_key: SecretString, model: &str) {
    eprintln!("live_smoke: calling model {model} (real API, real tokens)");

    let provider = TransyncAnthropic::try_new(api_key, ModelId::new(model), base_url_from_env())
        .expect("provider construction with a non-empty key and valid base URL");
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();
    opts.model_id = model.to_owned();

    // 1. Any transport / auth / schema / validation drift fails here.
    let output = translate(LIVE_SOURCE, &opts, &provider)
        .await
        .expect("live translate() round-trip must succeed");

    // 2. Every source block is paired, and the unit tally matches the
    //    offline-pinned fixture shape.
    let summary = &output.alignment_map.validation_summary;
    assert_eq!(
        summary.total_units, LIVE_SOURCE_UNITS,
        "live run must translate the fixture's {LIVE_SOURCE_UNITS} units"
    );
    let source_blocks = transync_syntax::parser::parse(LIVE_SOURCE)
        .expect("fixture parses")
        .blocks
        .len();
    assert_eq!(
        output.alignment_map.blocks.len(),
        source_blocks,
        "alignment map must carry one source/target anchor pair per source block"
    );

    // 3. At least one unit genuinely translated. Deliberately not
    //    `fallback_source == 0`: a single flaky unit degrading to honest
    //    source is within contract and must not fail the gate.
    assert!(
        summary.fallback_source < summary.total_units,
        "every unit fell back to source — no evidence of a live translation \
         (fallback_source={}, total_units={})",
        summary.fallback_source,
        summary.total_units
    );

    // 4. Output actually changed.
    assert!(
        !output.translated_document.is_empty(),
        "translated markdown must not be empty"
    );
    assert_ne!(
        output.translated_document, LIVE_SOURCE,
        "translated markdown must differ from the source"
    );
}

/// The Messages API, once. One surface means one live test — where the
/// sibling adapter needs two because it has two endpoints to prove.
#[tokio::test]
#[ignore = "live Anthropic call — set TRANSYNC_LIVE_SMOKE=1 and ANTHROPIC_API_KEY, then run with --ignored"]
async fn live_messages_api_round_trip() {
    let Some(api_key) = live_gate() else { return };
    let model = model_from_env("TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL", DEFAULT_LIVE_MODEL);
    assert_round_trip(api_key, &model).await;
}

// ---------------------------------------------------------------------------
// Offline tests — these DO run in the default suite and never touch the
// network. They protect the gate itself and the fixture constants, which is
// what stops the file above from being a test that passes because nothing
// happened.
// ---------------------------------------------------------------------------

/// Both gate layers are required, and only the exact opt-in value opens the
/// first one. Pure — no environment mutation, so it is safe under any
/// `--test-threads` value and under `--include-ignored`.
#[test]
fn gate_requires_both_opt_in_and_api_key() {
    // Layer 1: no opt-in, however the key looks.
    assert_eq!(decide_gate(None, Some("sk-ant-live")), Gate::SkipNoOptIn);
    assert_eq!(
        decide_gate(Some(""), Some("sk-ant-live")),
        Gate::SkipNoOptIn
    );
    assert_eq!(
        decide_gate(Some("0"), Some("sk-ant-live")),
        Gate::SkipNoOptIn
    );
    assert_eq!(
        decide_gate(Some("true"), Some("sk-ant-live")),
        Gate::SkipNoOptIn
    );
    assert_eq!(
        decide_gate(Some("1 "), Some("sk-ant-live")),
        Gate::SkipNoOptIn
    );

    // Layer 2: opted in, but no usable credentials.
    assert_eq!(decide_gate(Some("1"), None), Gate::SkipNoKey);
    assert_eq!(decide_gate(Some("1"), Some("")), Gate::SkipNoKey);

    // Both open.
    assert_eq!(
        decide_gate(Some("1"), Some("sk-ant-live")),
        Gate::Run("sk-ant-live".to_string())
    );
}

/// The model knob resolves to the cheap default when the variable names
/// nothing, and to whatever it names otherwise. Pinned because the default is
/// a cost decision: a run that silently picked the top-tier model would still
/// pass every assertion above and cost an order of magnitude more.
#[test]
fn the_live_model_knob_defaults_to_the_cheapest_capable_model() {
    assert_eq!(DEFAULT_LIVE_MODEL, "claude-haiku-4-5");
    // The resolver itself is exercised through a variable no test sets, so it
    // reports the default — which is the arm that matters.
    assert_eq!(
        model_from_env(
            "TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL_UNSET_BY_ANY_TEST",
            DEFAULT_LIVE_MODEL
        ),
        DEFAULT_LIVE_MODEL
    );
}

/// Pins the fixture's block/unit shape offline so the live assertions above
/// stay meaningful. If the parser or batcher ever changes what
/// [`LIVE_SOURCE`] yields, this fails here — not mid-flight against a paid
/// endpoint.
#[test]
fn live_source_fixture_has_the_expected_shape() {
    let doc = transync_syntax::parser::parse(LIVE_SOURCE).expect("fixture parses");
    let mut opts = TranslateOptions::default();
    opts.target_language = "ko".to_string();
    let batches = transync_core::unit::build_batches(
        &doc,
        &opts,
        None,
        &transync_core::unit::html_outcomes(&doc),
    );
    assert_eq!(batches.len(), 1, "fixture must fit in a single batch");
    assert_eq!(
        batches[0].units.len() as u32,
        LIVE_SOURCE_UNITS,
        "fixture must yield exactly {LIVE_SOURCE_UNITS} translation units"
    );
    assert!(
        !doc.blocks.is_empty(),
        "fixture must parse to at least one block"
    );
}
