---
type: DCR
title: Provider error taxonomy — five terminal causes get names, and stable_code() reaches them
description: TranslatorError::Other stopped carrying five distinct terminal causes; each is now its own variant with its own stable code, and TransyncError::stable_code() delegates instead of flattening the provider family to provider_error.
tags: [change, project-control, DCR-0023]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-08T00:00:00Z
status: stable
---

# DCR-0023: Provider error taxonomy — five terminal causes get names, and `stable_code()` reaches them

- **Date:** 2026-08-08
- **Source:** ticket `1a85f3`, filed by the dynwebserver maintainer after integrating transync 0.3.0
- **Affected ADRs:** `docs/decisions/0002-http-free-core-with-translator-trait.md` (updated — 2026-08-08 amendment; it is where `TranslatorError` is declared)
- **Affected contracts:** `docs/architecture/contracts.md` §1 (variant list, `stable_code()` vocabulary), §7 (HTTP status, Responses `status`, Chat `finish_reason`, refusals, byte cap)
- **Breaking.** Carried by **v0.4.0**, under the sanctioned 0.x breaking window.

## What Changed

**Five terminal causes stopped sharing one variant.** Through v0.3.0
`TranslatorError::Other(String)` was the destination for every terminal
provider failure that was not auth, malformed or unsupported. A census of the
shipped adapter's construction sites — both surfaces, not the ticket's list
taken on faith — found **nine** `ProviderError::Other` expressions across four
modules. Eight collapse to **five** distinct causes; the ninth is a
genuinely-internal case that stays `Other`:

| cause | where it is produced today |
|---|---|
| `ContentFiltered` | Chat `finish_reason: "content_filter"`; Responses `incomplete` whose `reason` names the filter |
| `OutputCeilingExhausted` | Chat `finish_reason: "length"`; Responses `incomplete (reason: max_output_tokens)` |
| `ModelRefused` | Chat `message.refusal` **and** a `refusal`-typed content part; Responses `refusal` content segments |
| `ResponseTooLarge` | the 32 MiB answer cap, tripped up front (`Content-Length`) or mid-stream |
| `ProviderRejected { status, message }` | every non-transient 4xx (400/404/418/422/499 …) |

The ninth — `endpoint::build_endpoint`'s "malformed default base_url", a
`Url::parse` over a crate constant that cannot fail — stays `Other`, which is
the honest answer for it.

Two of those rows are not one-to-one with a construction site, and that is the
finding worth recording:

- **A refusal has three shapes and one cause.** `message.refusal`, a Chat
  `refusal` content part, and a Responses `refusal` segment all mean *the model
  declined*. They collapse to one variant, so a consumer never has to know
  which surface a run dispatched on.
- **Responses `incomplete` is one status over several causes.** Mapping it
  wholesale onto the ceiling would have been a lie for
  `incomplete_details.reason: "content_filter"` — and an actively harmful one,
  since the two remediations are opposite (raise a knob / no knob exists). The
  reason now picks the variant, and a reason this adapter does not know — an
  absent one included — stays `Other` rather than borrowing a name that would
  send an operator after the wrong knob.

**`stable_code()` reaches the provider level.** `TranslatorError` gained a
`stable_code()` mirroring `TransyncError`'s, and `TransyncError::stable_code()`
now **delegates** its `Translator` arm to it. The vocabulary grew from seven
codes to seventeen: six engine-side, unchanged, and eleven provider-side. Both
matches stay exhaustive with no wildcard arm, so a new variant on either enum
cannot land without a code.

**`provider_error` narrows.** It still means "a provider failure", but now only
the *unclassified* one (`TranslatorError::Other`). This is the one sanctioned
exception to contracts.md §1's append-only rule, and it rides the 0.4.0 window.

**A typed status.** `ProviderRejected` carries `status: Option<u16>` beside its
message. `Some(404)` is a model name that does not exist — an operator fault
that no retry and no document edit will fix — and it was previously
indistinguishable from a 400 without matching a string.

**A gate.** `crates/transync/tests/error_taxonomy.rs` welds three artifacts:
every variant has a distinct code, `TransyncError` delegates rather than
flattens, and the contracts.md §1 vocabulary table is scraped and diffed
against the codes the library actually returns, in both directions.

## Why

The type was hiding a decision the library had already made and could not
communicate. Every one of the five causes is *terminal for the same stated
reason* — ADR-0009's retry is a verbatim resubmission, and verbatim
resubmission of the content that tripped a filter, blew a ceiling, or drew a
refusal fails identically. The adapter's own rustdoc says exactly that. But
`Other` is also where a consumer's "I don't know what this is" bucket lands, so
the honest downstream reading of `Other` was *unknown*, and the honest handling
of unknown is *retry*.

The filer measured the cost. dynweb maps `TranslatorError` onto reader-facing
failure classes; unable to separate the five, it advertised the whole bucket as
a RETRYABLE "the translation service is unavailable — try again". A reader
following that advice enters an unbounded loop of paid, guaranteed-identical
failures. The type was inviting the behavior its own documentation forbade.

Naming the causes also gives the two remediations that exist somewhere to
live: an exhausted ceiling names `[batching].target_output_tokens`; a 404 names
the operator's `model`. A content-policy stop names nothing, deliberately —
and being able to *say* "nothing will help here" is itself the fix for the
retry loop.

## Alternatives Considered

- **A `retryable()` / `is_terminal()` predicate only** (the ticket's minimum,
  option 3). Rejected: it encodes *policy* in a type whose job is to state
  *cause*. The pipeline's rule (re-dispatch `Network` and `RateLimited`) is one
  policy over this taxonomy; a consumer's user-facing policy is another, and a
  third provider may want a third. With causes named, every such predicate is
  derivable; with only a predicate, no explanation is.
- **A closed enum, no catch-all.** Rejected: `transync-anthropic` is
  commissioned (ticket `bda471`), and a closed enum would make every future
  provider quirk a breaking change. `Other` stays, and §1 now tells a
  `Translator` implementor to reach for it rather than force an ill-fitting
  name.
- **Leaving `TransyncError::stable_code()` at `provider_error`.** Rejected: the
  code strings exist *for* the process boundary — a consumer reading JSON or
  parsing stderr has nothing else. Stopping the taxonomy one level above where
  it is consumed reproduces the original complaint in the one channel that
  cannot work around it.
- **Splitting the 4xx family into named variants** (`ModelNotFound`,
  `BadRequest`, …). Rejected: the HTTP status is already the precise, stable
  discriminator, and enumerating a transport's status codes into a
  provider-neutral enum would put HTTP into a type that must also serve a
  provider that has none. `status: Option<u16>` carries the fact without
  importing the vocabulary.

## Consequences

- **Good:** A consumer can explain the failure and can stop retrying what
  cannot succeed. The two operator-actionable causes (ceiling, 404) are
  separable from the two document-level ones (filter, refusal).
- **Good:** Both surfaces of the bundled adapter now agree on cause names, so
  a consumer's classification does not depend on which API the run used.
- **Good:** The gate makes the vocabulary un-driftable: a new code without a
  documented row, or a row without a code, is a red test.
- **Bad (accepted):** `provider_error` narrows, so a consumer keying on it as
  "any provider failure" sees ten codes it must treat as
  unrecognized-but-opaque. contracts.md §1 already required that tolerance.
- **Bad (accepted):** Every match over `TranslatorError` in a consumer keeps
  compiling — the enum is `#[non_exhaustive]`, so wildcards are mandatory — but
  a wildcard arm is now the *wrong* answer for the five named causes, and the
  compiler cannot say so. Migration is a review task, not a build error. Both
  sibling consumers are named in the migration note below.

## Migration

**dynwebserver** (`crates/dynweb-core/src/translate/engine.rs`,
`classify_translator`) — its `E::Other(detail) => TranslateError::ProviderStopped(detail)`
arm no longer sees the five causes, and its trailing
`other => TranslateError::Local(other.to_string())` now catches them: a
provider stop reported to a reader as a *local* fault. Before:

```rust
E::Other(detail) => TranslateError::ProviderStopped(detail),
other => TranslateError::Local(other.to_string()),
```

After:

```rust
E::ContentFiltered(d) | E::OutputCeilingExhausted(d)
| E::ModelRefused(d) | E::ResponseTooLarge(d) => TranslateError::ProviderStopped(d),
E::ProviderRejected { status: Some(404), message } => TranslateError::Refused(message),
E::ProviderRejected { message, .. } => TranslateError::ProviderStopped(message),
E::Other(detail) => TranslateError::ProviderStopped(detail),
other => TranslateError::Local(other.to_string()),
```

**resp-translator** (`bins/copy-transfer-mcp`) — its listener hardcodes the
code strings with no cargo edge, and
`tests/dcr0002_pipeline_parity.rs::scn12_provider_failure_maps_to_stable_code_and_arms_cooldown`
asserts `provider_error` for a `TranslatorError::Network`. That now returns
`provider_network`; the assertion and the listener's code table both move.

## Verification

- `cargo test --workspace -- --test-threads=4`
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, the wasm32
  gate and the rustdoc gate (all four are the tracked pre-commit hook)
- `crates/transync/tests/error_taxonomy.rs` — 7 pins on distinctness,
  delegation, the typed status, and the contracts.md §1 scrape
- `transync-openai`'s `client::{chat, responses, classify, transport}` tests —
  the envelope-to-variant half, including the new
  `responses_incomplete_names_its_cause_from_the_reason`
- `transync-core::pipeline::run_level_tests::a_classified_terminal_cause_survives_the_preflight_annotation`
  — end to end through the annotator that used to flatten fields
