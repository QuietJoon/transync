//! Standing gate on the provider error taxonomy and the `stable_code()`
//! vocabulary (ti `1a85f3`).
//!
//! `TranslatorError` names *why* a provider stopped, and both error enums
//! answer with a stable machine string that leaves the process. Three
//! artifacts have to agree, and this test welds them:
//!
//! 1. **Distinctness** — every cause a consumer must react to differently
//!    has its own variant *and* its own code. A taxonomy where two causes
//!    share a code is the defect this ticket was filed about, one level
//!    down.
//! 2. **Delegation** — `TransyncError::stable_code()` returns exactly what
//!    `TranslatorError::stable_code()` returns for the `Translator` arm, so
//!    a consumer that only ever sees the string is not stuck one level above
//!    where it needs to be.
//! 3. **Documentation** — the code set is scraped out of contracts.md §1 and
//!    diffed against the codes the two methods actually return, in both
//!    directions. A code that lands without a row, or a row naming a code
//!    nothing returns, is a red test.
//!
//! What this cannot cover is the *adapter* half — which envelope produces
//! which variant. That is pinned where the envelopes are, in
//! `transync-openai`'s `client::{chat, responses, classify, transport}`
//! tests, because a provider crate is not on this crate's dependency path.
//!
//! TRACE: contracts.md §1
//! TRACE: ti 1a85f3

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use transync::{TranslatorError, TransyncError};

/// Repo root, derived from this crate's manifest dir (`crates/transync`).
/// Same idiom as `public_surface.rs` and `docs_index_drift.rs`.
fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One instance of every `TranslatorError` variant, paired with the variant
/// name contracts.md §1 spells.
///
/// Written out longhand rather than derived: this list is the thing the
/// exhaustive in-crate `stable_code()` match is checked *against*, so
/// generating it from the same source would make the check circular. A new
/// variant makes `stable_code()` fail to compile in `transync-core`; adding
/// it here is the second, deliberate step.
fn every_translator_error() -> Vec<(&'static str, TranslatorError)> {
    vec![
        ("Network", TranslatorError::Network("t".into())),
        (
            "Authentication",
            TranslatorError::Authentication("a".into()),
        ),
        (
            "RateLimited",
            TranslatorError::RateLimited {
                retry_after: Some(Duration::from_secs(1)),
            },
        ),
        (
            "MalformedResponse",
            TranslatorError::MalformedResponse("m".into()),
        ),
        ("Unsupported", TranslatorError::Unsupported("u".into())),
        (
            "ContentFiltered",
            TranslatorError::ContentFiltered("filtered".into()),
        ),
        (
            "OutputCeilingExhausted",
            TranslatorError::OutputCeilingExhausted("ceiling".into()),
        ),
        (
            "ContextWindowExceeded",
            TranslatorError::ContextWindowExceeded("context window".into()),
        ),
        (
            "ModelRefused",
            TranslatorError::ModelRefused("refused".into()),
        ),
        (
            "ResponseTooLarge",
            TranslatorError::ResponseTooLarge("too large".into()),
        ),
        (
            "ProviderRejected",
            TranslatorError::ProviderRejected {
                status: Some(404),
                message: "HTTP 404: no such model".into(),
            },
        ),
        ("Cancelled", TranslatorError::Cancelled),
        ("Other", TranslatorError::Other("o".into())),
    ]
}

/// One instance of every non-`Translator` `TransyncError` variant.
fn every_engine_error() -> Vec<TransyncError> {
    vec![
        TransyncError::Parse(transync::ParseError::Comrak("boom".into())),
        TransyncError::Validation("v".into()),
        TransyncError::Regen("r".into()),
        TransyncError::Profile(transync::ProfileError::Malformed("p".into())),
        TransyncError::Alignment(serde_json::from_str::<u8>("x").expect_err("bad json")),
        TransyncError::Cancelled,
        TransyncError::Internal("i".into()),
    ]
}

/// Every code contracts.md §1 lists, scraped out of the vocabulary section.
///
/// The section runs from the `stable_code()` vocabulary marker to the
/// `Rules:` paragraph that closes it, and each entry is a bullet of the form
/// ``- `code` — `Variant` ``. Both halves are captured so a row that renames
/// a variant without renaming its code (or the reverse) is visible.
fn documented_codes() -> BTreeSet<(String, String)> {
    let text = fs::read_to_string(repo_root().join("docs/architecture/contracts.md"))
        .expect("contracts.md must exist");
    let start = text
        .find("**`stable_code()` vocabulary")
        .expect("contracts.md §1 must carry the stable_code() vocabulary section");
    let rest = &text[start..];
    let end = rest
        .find("\nRules: a code string")
        .expect("the vocabulary section must close with its Rules paragraph");

    let rows: BTreeSet<(String, String)> = rest[..end]
        .lines()
        .filter_map(|line| {
            let entry = line.strip_prefix("- `")?;
            let (code, tail) = entry.split_once("` — `")?;
            let variant = tail.strip_suffix('`')?;
            Some((code.to_string(), variant.to_string()))
        })
        .collect();

    assert!(
        !rows.is_empty(),
        "the vocabulary scrape found nothing — the section's bullet shape \
         changed and this gate is passing vacuously"
    );
    rows
}

/// Every cause a consumer must react to differently answers with a distinct
/// code. Two variants sharing one code would reproduce, one level down,
/// exactly the collapse this taxonomy was built to end.
#[test]
fn every_translator_variant_carries_its_own_stable_code() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    for (name, err) in every_translator_error() {
        let code = err.stable_code();
        if let Some((other, _)) = seen.iter().find(|(_, c)| *c == code) {
            panic!("{name} and {other} share the stable code {code:?}");
        }
        assert!(
            !code.is_empty(),
            "{name} must carry a non-empty stable code"
        );
        seen.push((name, code));
    }
    assert_eq!(
        seen.len(),
        every_translator_error().len(),
        "every variant must be represented exactly once"
    );
}

/// `Other` keeps `provider_error` — the code an existing consumer already
/// keys on — and it keeps it for the *unclassified* case only. Both halves
/// matter: the string survives, its meaning narrows, and contracts.md §1
/// records that narrowing as the sanctioned 0.4.0 exception to append-only.
#[test]
fn the_catch_all_keeps_the_code_the_whole_family_used_to_share() {
    assert_eq!(
        TranslatorError::Other("anything".into()).stable_code(),
        "provider_error"
    );
    for (name, err) in every_translator_error() {
        if name == "Other" {
            continue;
        }
        assert_ne!(
            err.stable_code(),
            "provider_error",
            "{name} is classified, so it must not answer with the \
             unclassified code"
        );
    }
}

/// The five causes the ticket named are the five a consumer branches on, and
/// each one is reachable as its own variant with its own code. Pinned by
/// name rather than by count so a rename cannot slip through.
#[test]
fn the_five_terminal_causes_are_named_and_separable() {
    let cases = [
        (
            TranslatorError::ContentFiltered("policy".into()),
            "provider_content_filtered",
        ),
        (
            TranslatorError::OutputCeilingExhausted("ceiling".into()),
            "provider_output_ceiling_exhausted",
        ),
        (
            TranslatorError::ModelRefused("no".into()),
            "provider_model_refused",
        ),
        (
            TranslatorError::ResponseTooLarge("big".into()),
            "provider_response_too_large",
        ),
        (
            TranslatorError::ProviderRejected {
                status: Some(404),
                message: "HTTP 404: no such model".into(),
            },
            "provider_rejected",
        ),
    ];
    for (err, expected) in cases {
        assert_eq!(err.stable_code(), expected, "for {err}");
    }
}

/// DCR-0029: the two token-shaped terminal causes are separable **by code**,
/// not by reading prose. They are the pair an operator can most easily
/// confuse — both terminal, both about tokens — and they name opposite knobs,
/// so a consumer that cannot tell them apart cannot give the right advice.
#[test]
fn the_two_token_causes_name_opposite_knobs_under_distinct_codes() {
    let ceiling = TranslatorError::OutputCeilingExhausted("cut off".into());
    let window = TranslatorError::ContextWindowExceeded("did not fit".into());
    assert_eq!(ceiling.stable_code(), "provider_output_ceiling_exhausted");
    assert_eq!(window.stable_code(), "provider_context_window_exceeded");
    assert_ne!(ceiling.stable_code(), window.stable_code());
    // …and neither collapses into the unclassified catch-all on the way
    // through `TransyncError`, which is the level a process-boundary consumer
    // actually reads.
    for err in [ceiling, window] {
        let expected = err.stable_code();
        assert_ne!(expected, "provider_error");
        assert_eq!(TransyncError::from(err).stable_code(), expected);
    }
}

/// A rejected request carries its HTTP status as a number, so the operator
/// fault (a mistyped model name → 404) is separable from a malformed request
/// (400) without parsing the message.
#[test]
fn a_rejected_request_carries_a_typed_status() {
    let err = TranslatorError::ProviderRejected {
        status: Some(404),
        message: "HTTP 404: model not found".into(),
    };
    let TranslatorError::ProviderRejected { status, message } = &err else {
        panic!("constructed variant must match itself");
    };
    assert_eq!(*status, Some(404));
    assert!(message.contains("model not found"));
    // A provider with no HTTP status to give is still a rejection.
    assert_eq!(
        TranslatorError::ProviderRejected {
            status: None,
            message: "rejected".into(),
        }
        .stable_code(),
        "provider_rejected"
    );
}

/// `TransyncError::stable_code()` reaches the provider level instead of
/// stopping at `provider_error`. This is the whole point for a consumer that
/// only ever sees the string — across a process boundary, the code *is* the
/// error.
#[test]
fn transync_error_delegates_the_provider_code() {
    for (name, err) in every_translator_error() {
        let expected = err.stable_code();
        let wrapped = TransyncError::from(err);
        assert_eq!(
            wrapped.stable_code(),
            expected,
            "{name} must not be flattened on its way through TransyncError"
        );
    }
}

/// The engine-side codes are untouched by the delegation above — only the
/// `Translator` arm moved.
#[test]
fn the_engine_side_codes_are_unchanged() {
    let codes: Vec<&str> = every_engine_error()
        .iter()
        .map(TransyncError::stable_code)
        .collect();
    assert_eq!(
        codes,
        vec![
            "parse_failed",
            "validation_failed",
            "regen_failed",
            "profile_failed",
            "alignment_failed",
            "cancelled",
            "internal",
        ]
    );
}

/// contracts.md §1 and the code agree on the vocabulary, in both directions.
#[test]
fn the_documented_vocabulary_matches_the_codes_the_library_returns() {
    let documented = documented_codes();

    let mut live: BTreeSet<(String, String)> = every_translator_error()
        .into_iter()
        .map(|(name, err)| (err.stable_code().to_string(), name.to_string()))
        .collect();
    for (variant, code) in [
        ("Parse", "parse_failed"),
        ("Validation", "validation_failed"),
        ("Regen", "regen_failed"),
        ("Profile", "profile_failed"),
        ("Alignment", "alignment_failed"),
        ("Cancelled", "cancelled"),
        ("Internal", "internal"),
    ] {
        live.insert((code.to_string(), variant.to_string()));
    }

    let undocumented: Vec<_> = live.difference(&documented).collect();
    let orphaned: Vec<_> = documented.difference(&live).collect();
    assert!(
        undocumented.is_empty() && orphaned.is_empty(),
        "stable_code() vocabulary drift — returned but not in contracts.md \
         §1: {undocumented:?}; documented but not returned: {orphaned:?}"
    );
}
