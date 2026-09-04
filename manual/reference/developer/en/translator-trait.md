---
type: Reference
title: The `Translator` trait
description: Method signatures, the complete `TranslatorError` variant set with its stable codes, and the behavior contract every implementation must satisfy.
tags: [providers, llm, reference, cancellation, DCR-0024, DCR-0029]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: core-llm, resource: crates/transync-core/src/llm.rs }
  - { id: core-error, resource: crates/transync-core/src/error.rs }
  - { id: core-lib, resource: crates/transync-core/src/lib.rs }
  - { id: core-dispatch, resource: crates/transync-core/src/pipeline/dispatch.rs }
  - { id: core-policy, resource: crates/transync-core/src/pipeline/policy.rs }
  - { id: facade, resource: crates/transync/src/lib.rs }
  - { id: openai-lib, resource: crates/transync-openai/src/lib.rs }
  - { id: openai-classify, resource: crates/transync-openai/src/client/classify.rs }
  - { id: anthropic-lib, resource: crates/transync-anthropic/src/lib.rs }
  - { id: anthropic-messages, resource: crates/transync-anthropic/src/client/messages.rs }
  - { id: anthropic-classify, resource: crates/transync-anthropic/src/client/classify.rs }
  - { id: workspace-manifest, resource: Cargo.toml }
  - { id: taxonomy-tests, resource: crates/transync/tests/error_taxonomy.rs }
  - { id: contracts, resource: docs/architecture/contracts.md }
synced_hash: 20fb31f5f9b7a54be78be13756ec373e8c27b7d3520110ad1deb01d325fbe77d
---

# The `Translator` trait

`transync::llm::Translator`, declared in `transync-core::llm` and
re-exported through the facade. It has been the provider boundary since
`0.1.0` and its evolution is governed rather than frozen — see
[Trait stability](#trait-stability). The trait is `Send + Sync`, and
`translate` / `translate_with_cache` take `T: Translator + ?Sized`, so a
`&dyn Translator` and a concrete type are both accepted.

Two implementations ship in this workspace: `transync-openai`
(`TransyncOpenAI`) and `transync-anthropic` (`TransyncAnthropic`). Both are
libraries; only the first is reachable from the bundled CLI.

## Methods

```rust
#[async_trait::async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError>;

    fn fingerprint(&self) -> ProviderFingerprint {
        ProviderFingerprint::from_type_name(std::any::type_name::<Self>())
    }

    fn tokenizer_hint(&self) -> Option<TokenizerHint> {
        None
    }

    async fn extract_glossary(
        &self,
        req: &GlossaryExtractionRequest,
        cancel: &CancellationToken,
    ) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> {
        let _ = (req, cancel);
        Ok(None)
    }
}
```

| Method | Required | Default behavior |
|---|---|---|
| `translate_batch` | yes | — |
| `fingerprint` | no | Derives from `std::any::type_name::<Self>()` — correct only if every instance of the type is interchangeable. |
| `tokenizer_hint` | no | `None` — falls back to an OpenAI-model-name heuristic over `TranslateOptions::model_id`. |
| `extract_glossary` | no | `Ok(None)` — "not supported." |

## The `cancel` parameter

`CancellationToken` is `tokio_util::sync::CancellationToken`, re-exported
as `transync::CancellationToken`. It is the one foreign type the facade
admits into its curated surface, deliberately: naming it also pins the
`tokio-util` version a consumer's own dependency must unify with
(`0.7`, `default-features = false`, in the workspace manifest).

The token is the **run's**, not the call's — one token per
`translate` / `translate_with_cache` call, shared by every batch and handed
to every provider call. Its origin is `TranslateOptions::cancel:
Option<CancellationToken>`, whose default is `None`; a run with `None`
cannot be cancelled and behaves as it did before the field existed.

The pipeline races every provider call against the same token
(`pipeline::dispatch::translate_with_provider_retries`, a `biased`
`tokio::select!`) and drops the loser, so an implementation that ignores the
argument is still cancelled by future-drop. What honoring the argument adds
is a typed answer, correctness for work that is not drop-cancellable, and
correctness when the implementation is driven directly rather than through
the pipeline. The obligations are listed under
[Behavior contract](#behavior-contract).

The token is not a cache-identity axis: it cannot change what a completed
provider call says, only whether one happens.

## Trait stability

Variant and field additions are governed by `#[non_exhaustive]` (see the
error table below and `contracts.md` §0). Method-level evolution is
narrower, and two risk classes have now both occurred:

- **Additive.** A new method that carries a default keeps every existing
  implementor compiling and gives it the documented fallback behavior.
  `fingerprint()` (the sanctioned v0.2 break, DCR-0009), `tokenizer_hint()`
  and `extract_glossary()` (v0.2, DCR-0014 / DCR-0015) each landed this way.
- **Hard break.** A change to the *signature* of an existing method cannot
  be absorbed by a default. The `cancel` parameter added to
  `translate_batch` and `extract_glossary` is the sanctioned v0.4 break
  (DCR-0024): every implementation must add the parameter before it
  compiles again. `contracts.md` §5b records why the alternative that would
  have been additive — a defaulted `translate_batch_cancellable` delegating
  to the old method — was rejected: it leaves two methods for one job, and
  an implementor that overrides only the old one is silently uncancellable.

At the time of writing the workspace version is `0.3.0` and the `cancel`
parameter sits in the unreleased window `contracts.md` calls the v0.4.0
break.

## `fingerprint()` — cache-namespace identity

`ProviderFingerprint::new(family, parts)` composes injectively: every
part (family label included) is framed as its byte length, a `\u{1F}`
separator, then the bytes, so no arrangement of part *contents* can
collide with a different arrangement of *parts*. `from_type_name(name)`
is exactly `new("type", &[name])`.

Rule: **every axis that can change what the provider returns** — model,
endpoint, API surface, prompt template — MUST be covered.
Over-distinguishing (an axis that never actually changes output) costs a
redundant re-translation on a would-be cache hit. Under-distinguishing
lets a shared `Cache` replay one configuration's output for a different
one. An axis that only bounds *latency* (a timeout) rather than content
is correctly excluded.

## `tokenizer_hint()` — batch-budget estimation

`TokenizerHint` is `#[non_exhaustive]` with two variants today:
`O200kBase`, `Cl100kBase`. Not a cache-identity axis — batching shape is
estimation-only. A provider whose model names aren't OpenAI-shaped
SHOULD override this rather than let the engine guess wrong from a name
it doesn't recognize.

## `extract_glossary()` — optional candidate-glossary preflight

Called at most once per run, only when the caller or profile opts in.
`req.source_text` is untrusted document data and MUST be framed as data,
never instructions. Returned `GlossaryEntry.scope` values are ignored —
the merge forces `GlobalAcrossDocument`. `Ok(None)` (unsupported) and
`Ok(Some(vec![]))` (supported, nothing found) are deliberately distinct.

Two outcomes of this call are treated differently, and the asymmetry is
part of the contract:

- An **error** is not run-terminal. It is recorded on
  `ValidationReport.auto_glossary` and the run proceeds on the static
  glossary alone. No batch was dispatched, so output coverage is
  unaffected and the degraded state equals the opted-out baseline. The
  report is the only channel for it — an `Err(_)` here gets no `tracing`
  record.
- A **cancellation** is not degraded away. The pipeline re-reads the run's
  token after this call returns and aborts with
  `TransyncError::Cancelled` whatever the method answered. Without that
  re-read, cancelling during the one call a run makes before batching
  would resolve into "proceeded on the static glossary" and the whole
  document would still be dispatched.

## `TranslatorError` — `#[non_exhaustive]`, fourteen variants

Each variant names **why the provider stopped**, never what the caller
should do about it. Retryability is a policy over the taxonomy, not part
of it; the pipeline's policy is one column of this table and an
application's user-facing policy is its own.

| Variant | Meaning | Retried by the pipeline? | Stable code |
|---|---|---|---|
| `Network(String)` | Transport-level failure. | Yes — bounded transient-transport budget. | `provider_network` |
| `Authentication(String)` | Credential failure. | No — aborts the run. | `provider_auth` |
| `RateLimited { retry_after: Option<Duration> }` | Provider rate limit. | Yes — same budget; honors `retry_after` when present, capped at 30s, else exponential backoff (200ms doubling, capped at 5s). | `provider_rate_limited` |
| `MalformedResponse(String)` | Provider returned data the implementation could not parse. | No — aborts the run. | `provider_malformed_response` |
| `Unsupported(String)` | Provider does not support a constraint in the batch. | No — aborts the run. | `provider_unsupported` |
| `ContentFiltered(String)` | The provider's own content policy ended generation. Not the model declining, and not a transport fault; nothing in the request moves it, so there is no remediation to offer. | No — aborts the run. | `provider_content_filtered` |
| `OutputCeilingExhausted(String)` | The output-token ceiling was exhausted before the answer was complete. The ceiling rides in the request, so `[batching].target_output_tokens` is the remediation. | No — aborts the run. | `provider_output_ceiling_exhausted` |
| `ContextWindowExceeded(String)` | The request did not fit the model's context window — input plus requested output, not output alone. The remediation is the batching configuration (the `[batching]` token budget and `max_units_per_batch`), never the output ceiling. Added in v0.4.0 (DCR-0029). | No — aborts the run. | `provider_context_window_exceeded` |
| `ModelRefused(String)` | The model declined and said so; carries the provider's refusal text, length-capped by the adapter, or a fixed stand-in when the refusal arrived carrying none. | No — aborts the run. | `provider_model_refused` |
| `ResponseTooLarge(String)` | The answer body exceeded the adapter's size cap and was not read. | No — aborts the run. | `provider_response_too_large` |
| `ProviderRejected { status: Option<u16>, message: String }` | The provider rejected the *request* rather than failing to answer it. `status` is the HTTP status where the provider speaks HTTP, `None` for one that has no such code. | No — aborts the run. | `provider_rejected` |
| `NoProviderAvailable(String)` | This `Translator` has no provider to call, so the work it was handed cannot be done by it at all — raised by an `--offline` run when a unit misses the cache (ti `30a744`, DCR-0046). | No — aborts the run; the remediation is the caller's configuration. | `provider_unavailable` |
| `Cancelled` | The call stopped because it was cancelled: the run's token fired, or the implementation holds a cancellation source of its own. Carries no message on purpose. | No — never re-dispatched; a verbatim resubmission is exactly the work the caller asked to stop. | `provider_cancelled` |
| `Other(String)` | The standing catch-all for a provider failure this taxonomy does not name. | No — aborts the run. | `provider_error` |

`OutputCeilingExhausted` and `ContextWindowExceeded` are the pair an
operator must not confuse: both are terminal, both are about tokens, and
they name **opposite knobs**. They are separate variants for exactly that
reason. Not every provider raises `ContextWindowExceeded` as its own
signal — one that answers an over-long request with an HTTP 400 keeps the
status-based `ProviderRejected` classification, which is honest there. Of
the two bundled adapters, only `transync-anthropic` produces it, off the
provider's `model_context_window_exceeded` stop reason;
`transync-openai` has no signal that maps to it.

`RateLimited.retry_after` is `Option` because a hint is optional: `None`
means the provider offered none the implementation could use, and the
pipeline then falls back to its own exponential schedule. Both bundled
adapters parse the header the same way — delta-seconds, plus all three
RFC 7231 date forms — and resolve a deadline already at or behind the
system clock to `None` rather than to a zero delay, so a skewed clock
cannot turn into an immediate re-dispatch. What an out-of-tree
implementation puts in the field is its own decision; the 30-second cap
above applies to whatever arrives.

`Other` has narrowed in meaning. Through v0.3.0 it carried five distinct
terminal causes — content filter, exhausted output ceiling, model refusal,
oversize body, rejected request — separable only by matching a message
string that is explicitly not an interface. Those five now have names, and
`Other` means the *unclassified* provider failure only. It is kept
deliberately, so a new provider quirk can land as a message rather than
wait for a variant.

`#[non_exhaustive]`: variant additions are non-breaking; consumers MUST
keep a wildcard (`_`) match arm and cannot rely on exhaustiveness to
catch a new variant at compile time. Variant removals and renames remain
breaking.

Every non-retried variant is **terminal**: once encountered (or once the
transient-retry budget for `Network`/`RateLimited` is exhausted, default
`max_per_batch_provider_retries = 1`, charged per batch across its whole
retry ladder), the whole pipeline run aborts with `Err` — it does not
degrade to a `fallback_source` output. See
[why layered validation and bounded retry/fallback](../../../explanation/developer/en/validation-retry-fallback-model.md)
for why that's a deliberate design choice, not an omission.

## `stable_code()` — the wire vocabulary

`TranslatorError::stable_code(&self) -> &'static str` returns the
machine-readable name of the failure cause. These strings leave the
process — they are what a JSON-consuming or shell-consuming caller keys on
— so they are a wire vocabulary, not a debug convenience. Both
`stable_code` matches in the library are exhaustive with **no wildcard
arm**, so a new variant cannot land without being assigned a code.

`TransyncError::stable_code()` shares the same namespace and answers for
every failure the library can produce. Its `Translator` arm **delegates**
to `TranslatorError::stable_code()` rather than flattening the whole
provider family to one string. The complete set is twenty-one codes
(7 engine-side + 14 provider-side).

Engine-side, from `TransyncError`'s own variants:

| Code | Variant |
|---|---|
| `parse_failed` | `Parse` |
| `validation_failed` | `Validation` |
| `regen_failed` | `Regen` |
| `profile_failed` | `Profile` |
| `alignment_failed` | `Alignment` |
| `cancelled` | `Cancelled` |
| `internal` | `Internal` |

Provider-side: the fourteen `TranslatorError` codes in the variant table
above, returned both by `TranslatorError::stable_code()` and, through the
`Translator` variant, by `TransyncError::stable_code()`.

Rules governing the vocabulary:

- A code string never changes meaning, and is never renamed or
  repurposed. The mapping is **append-only**.
- A new variant on either enum requires a **new** code rather than reuse
  of an existing one.
- Consumers MUST tolerate an unrecognized code as an opaque failure, since
  variant additions are non-breaking.

One narrowing is the single sanctioned exception to append-only, and it
rides the v0.4.0 window: `provider_error` still means "a provider
failure", but now only the *unclassified* one. Code that read
`provider_error` as "any provider failure" now sees twelve sibling codes it
must treat as unrecognized-but-opaque.

`cancelled` and `provider_cancelled` are deliberately two codes, not one.
`cancelled` is the engine's answer — the *run* stopped because the caller's
token fired — and it is the only thing `translate` /
`translate_with_cache` return for a token-cancelled run, because the
pipeline decides cancellation from the token before a provider code can
surface. `provider_cancelled` says a `Translator` call stopped, and it
reaches a pipeline caller by exactly one route: an implementation answers
`TranslatorError::Cancelled` off a cancellation source of its own while the
run's token never fired. That is a terminal provider error like any other,
surfaced as `Translator(Cancelled)`. Driving the trait directly is the
other way to see it.

The vocabulary table in `docs/architecture/contracts.md` §1 is welded to
the code by `crates/transync/tests/error_taxonomy.rs`, which scrapes that
section and diffs it against the codes the two methods actually return.
That test reads the contract document, not this manual page.

## Behavior contract

Every `translate_batch` implementation MUST satisfy all of the following.
The pipeline assumes them; a violation either corrupts output or wastes a
retry/fallback slot the pipeline allocated in good faith.

- SHOULD attempt every unit in `batch.units` exactly once per call.
- MAY return `UnitResult { output_kind: FailedNeedsFallback }` for any
  unit it cannot handle — the core pipeline retries or falls back.
- MUST NOT return units not present in `batch.units`.
- MUST NOT return duplicate `unit_id`s.
- MUST preserve `unit_id` byte-for-byte — no normalization, no re-casing.
- MAY observe network retries internally — `transync` never sees them.
- A unit carrying `retry: Some(_)` is a re-dispatch: a previous attempt
  was rejected by the named validation layer. The implementation SHOULD
  use it to correct the structural failure, MAY ignore it, and MUST NOT
  reproduce `retry.reason` content in `translated_payload`.
  `source_payload` on a retry is byte-identical to the original dispatch.

Cancellation clauses, on the `cancel` argument of both `translate_batch`
and `extract_glossary`:

- MUST NOT cancel the token. It is the caller's, and it is shared across
  every call of the run.
- SHOULD observe it, by racing the provider round-trip against it and
  answering `TranslatorError::Cancelled`. `biased;` in the
  `tokio::select!` is load-bearing — it polls the token first, so an
  already-cancelled run never issues the request at all.
  `CancellationToken::run_until_cancelled` is biased the other way, polling
  the inner future first, and would send it.
- MAY ignore it. The pipeline races the call against the same token and
  drops the losing future, which aborts an in-flight `reqwest`-style
  request by construction. An implementation whose work is **not**
  drop-cancellable — an inner `tokio::spawn`, a `spawn_blocking` client,
  an internal queue — has no such safety net and must honor the token
  itself.
- MAY return `Cancelled` when the token did **not** fire, from a
  cancellation source of its own. That is legal and is terminal.

## HTML-segment units

A unit whose `block_kind` is `html` carries `input_mode:
"html_segments"`. `source_payload` is a **string containing a JSON array
of strings** — ordered, entity-decoded text segments extracted from the
block. `translated_payload` MUST be the same shape, same element count,
same order. Tags, attributes, comments, and `script`/`style` content
never appear in the payload and are spliced back by the application.
Preserve a segment by echoing it unchanged; `partially_translated` is not
offered for html units; `failed_needs_fallback` is legal.

## Related

- [How to implement a custom Translator provider](../../../how-to/developer/en/implement-a-custom-translator.md) —
  the task-oriented recipe: scaffolding, error mapping, wiring it in.
- [How to cancel a running translation](../../../how-to/developer/en/cancel-a-running-translation.md) —
  the caller-side and implementor-side cancellation recipe.
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md) —
  reading a failed run's exit code, report, and stderr line.
- `docs/architecture/contracts.md` §1 — the complete, authoritative
  contract this page summarizes, including `CacheKey` composition, the
  full retry decision flow (§5), and run cancellation (§5b).
