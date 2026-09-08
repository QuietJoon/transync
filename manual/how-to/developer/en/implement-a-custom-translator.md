---
type: How-To Guide
title: How to implement a custom Translator provider
description: Write a sibling crate that implements transync's `Translator` trait against HEAD — cancellation parameter included — map its failures onto the error taxonomy, and hand it to the pipeline.
tags: [providers, llm, extensibility, caching, cancellation, ADR-0002, ADR-0009, DCR-0024, DCR-0029]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: core-llm, resource: crates/transync-core/src/llm.rs }
  - { id: core-lib, resource: crates/transync-core/src/lib.rs }
  - { id: core-error, resource: crates/transync-core/src/error.rs }
  - { id: core-cache, resource: crates/transync-core/src/cache.rs }
  - { id: core-dispatch, resource: crates/transync-core/src/pipeline/dispatch.rs }
  - { id: facade, resource: crates/transync/src/lib.rs }
  - { id: openai-lib, resource: crates/transync-openai/src/lib.rs }
  - { id: openai-client, resource: crates/transync-openai/src/client.rs }
  - { id: openai-error, resource: crates/transync-openai/src/error.rs }
  - { id: openai-classify, resource: crates/transync-openai/src/client/classify.rs }
  - { id: openai-chat, resource: crates/transync-openai/src/client/chat.rs }
  - { id: openai-responses, resource: crates/transync-openai/src/client/responses.rs }
  - { id: openai-cargo, resource: crates/transync-openai/Cargo.toml }
  - { id: anthropic-lib, resource: crates/transync-anthropic/src/lib.rs }
  - { id: anthropic-error, resource: crates/transync-anthropic/src/error.rs }
  - { id: anthropic-messages, resource: crates/transync-anthropic/src/client/messages.rs }
  - { id: anthropic-cargo, resource: crates/transync-anthropic/Cargo.toml }
  - { id: cli-provider, resource: crates/transync-cli/src/translate_cmd/provider.rs }
  - { id: workspace-manifest, resource: Cargo.toml }
  - { id: changelog, resource: CHANGELOG.md }
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: dev-guide, resource: docs/Developer_Guide.md }
synced_hash: 0ba7506d2a500c2b6d27ff0cfaae96f541aa30545e5b4bf22168547ed0b4811f
---

# How to implement a custom Translator provider

Use this when you need transync to dispatch translation batches to an LLM
endpoint it does not already speak to — a gateway, a local model server, a
mock for tests — while keeping the rest of the pipeline (batching,
validation, retry/fallback, caching, alignment-map emission) unchanged.

This assumes you are comfortable with async Rust and already know why the
pipeline is shaped the way it is (block IDs, layered validation,
retry-then-fallback). For that background read `docs/Developer_Guide.md`
and `docs/architecture/contracts.md` §1 — this page is the recipe, not the
explanation.

## Read a reference implementation first

Two provider crates in this workspace implement the trait, and they are
deliberately different in shape:

- `crates/transync-openai` — the default adapter, two HTTP surfaces (Chat
  Completions and Responses) with model-name dispatch between them.
- `crates/transync-anthropic` — the second adapter (DCR-0029), one HTTP
  surface, a **required** output ceiling, and a schema-profile pass that
  renders the shared Structured Output schema into a narrower dialect.

They share no code — only the trait and the discipline — which is exactly
why reading both is worth the time: what appears in one and not the other
is provider-specific, and what appears in both is the contract.

**Neither the second adapter nor yours is reachable from the CLI.**
`transync-cli` has no dependency on `transync-anthropic`, and
`translate_cmd/provider.rs` has exactly two build configurations: the live
path, which builds `TransyncOpenAI::try_new` from `OPENAI_API_KEY` — or the
credential-free `TransyncOpenAI::offline` under `--offline` — and an
in-process stub behind the `test-stub-provider` feature. There is no
provider flag and no plugin
lookup. A custom `Translator` is consumed by a host program that constructs
it and calls `transync::translate` itself — the same status
`transync-anthropic` has.

## Preconditions

- A Rust crate (new or existing) that can depend on the `transync` facade
  crate — either as a workspace member here (mirroring
  `crates/transync-openai`) or as an external consumer pinning the
  published `transync` crate.
- `async-trait` — the trait's `async fn` requires it.
- `tokio` — the trait signature hands you a cancellation token, and the
  honoring pattern below is a `tokio::select!`.

### About the cancellation token's type

`CancellationToken` in the trait signature is
`tokio_util::sync::CancellationToken`. It is the one foreign type the
facade admits into its curated surface, and it reaches you as
`transync::CancellationToken`.

Take it through that re-export and you need **no** `tokio-util` dependency
of your own. If you do depend on `tokio-util` directly — to build a token,
to derive a `child_token`, to hold a `DropGuard` — the two requirements must
unify, or your token and transync's are different types and the compiler
says so, naming both crates. The workspace pins `tokio-util = { version =
"0.7", default-features = false }`.

## 1. Scaffold the crate

```bash
cargo new --lib transync-myprovider
```

Depend on the facade crate only — never on `transync-core` or
`transync-syntax` directly; those are engine internals outside the semver
firewall (`docs/architecture/contracts.md` §0, tier (c)). Both in-tree
adapters declare the same dependency edge list, which is a reasonable
starting point:

```toml
[dependencies]
transync    = { path = "../transync" }   # or a crates.io version pin
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "1"
async-trait = "0.1"
tokio       = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
# plus whatever your provider's transport needs (reqwest, secrecy, url, …)
```

If you are adding the crate as an in-tree workspace member, also add its
path to `members` in the workspace root `Cargo.toml`.

## 2. Implement the trait

Verify signatures against `crates/transync-core/src/llm.rs` — it is ground
truth, not the prose describing it. `translate_batch` is the only required
method; `fingerprint`, `tokenizer_hint` and `extract_glossary` are
defaulted.

Both async methods take a trailing `cancel: &CancellationToken`. This
compiles against HEAD:

```rust
use async_trait::async_trait;
use transync::CancellationToken;
use transync::llm::{
    OutputKind, TranslationBatch, TranslationBatchResult, Translator, TranslatorError,
    UnitResult,
};

pub struct MyProvider {
    // api key, model id, http client, …
}

impl MyProvider {
    /// One provider round-trip. Kept off the trait so the trait method is
    /// only the race below, and so this future is what gets dropped when
    /// the race is lost.
    async fn round_trip(
        &self,
        batch: &TranslationBatch,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        // 1. Render `batch.units` into your provider's request shape.
        //    Each unit's `block_kind` + `input_mode` tell you the
        //    structural shape; `constraints` are the invariants you must
        //    preserve; `context` is optional but improves quality.
        // 2. Send the request over any transport you like — transync
        //    never sees network retries you do internally.
        // 3. Parse the response into one `UnitResult` per requested unit.
        // 4. Return `TranslationBatchResult` with the same `batch_id`.
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id.clone(),
            detected_source_language: None,
            units: batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: String::new(), // the model's translation
                    warnings: vec![],
                })
                .collect(),
        })
    }
}

#[async_trait]
impl Translator for MyProvider {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TranslatorError::Cancelled),
            r = self.round_trip(&batch) => r,
        }
    }
}
```

`biased;` is load-bearing, not decoration: it polls the token first, so an
already-cancelled run never issues the request at all.
`CancellationToken::run_until_cancelled` is biased the other way — it polls
the inner future first — and would send it. Both in-tree adapters use
exactly this shape, on both async methods.

Never cancel the token you are handed. It is the caller's, and it is shared
across every call of the run. The full set of cancellation obligations, and
the case for honoring the token when future-drop would already cancel you,
is in
[how to cancel a running translation](./cancel-a-running-translation.md).

A unit whose `block_kind` is `html` needs different handling: its
`source_payload` is a string holding a JSON array of text segments, not raw
text — see the [`Translator` trait
reference](../../../reference/developer/en/translator-trait.md) and
`contracts.md` §1 ("HTML-segment units") before writing that branch. A unit
whose `input_mode` is `table_row_window` needs **no** new code: its payload
is a complete GFM table and is translated exactly like a whole one.

## 3. Map provider failures onto `TranslatorError`

`TranslatorError` is `#[non_exhaustive]` and has **fourteen** variants at
HEAD — `Network`, `Authentication`, `RateLimited { retry_after }`,
`MalformedResponse`, `Unsupported`, `ContentFiltered`,
`OutputCeilingExhausted`, `ContextWindowExceeded`, `ModelRefused`,
`ResponseTooLarge`, `ProviderRejected { status, message }`,
`NoProviderAvailable`, `Cancelled`, `Other`. Match it with a wildcard arm; the full table, with each variant's
stable code, is in the
[reference](../../../reference/developer/en/translator-trait.md).

Two rules govern which one you reach for:

1. **The pipeline retries only `Network` and `RateLimited`**
   (`transync-core::pipeline::dispatch::translate_with_provider_retries`).
   Every other variant is terminal and aborts the run once encountered
   (ADR-0017; there is no per-batch fallback rung, only per-unit fallback
   via the validation branch).
2. **The line you are drawing is "could a verbatim resubmission plausibly
   come back different"** — not severity. An exhausted output ceiling is
   terminal because the ceiling rode in the request. A provider-side
   `failed` status is transient because the request was accepted and the
   provider gave up on its own side.

Both adapters keep a crate-private `ProviderError`, classify at the site
that reads the signal, and let the boundary function only rename. Here is
`transync-openai/src/error.rs`'s map, all ten arms:

```rust
match err {
    ProviderError::Transport(s) => TranslatorError::Network(s),
    ProviderError::Auth(s) => TranslatorError::Authentication(s),
    ProviderError::RateLimitedAfter(retry_after) => {
        TranslatorError::RateLimited { retry_after }
    }
    ProviderError::Malformed(s) => TranslatorError::MalformedResponse(s),
    ProviderError::ContentFiltered(s) => TranslatorError::ContentFiltered(s),
    ProviderError::OutputCeilingExhausted(s) => TranslatorError::OutputCeilingExhausted(s),
    ProviderError::ModelRefused(s) => TranslatorError::ModelRefused(s),
    ProviderError::ResponseTooLarge(s) => TranslatorError::ResponseTooLarge(s),
    ProviderError::Rejected { status, message } => {
        TranslatorError::ProviderRejected { status, message }
    }
    ProviderError::Other(s) => TranslatorError::Other(s),
}
```

`transync-anthropic`'s map is those ten plus one: it adds
`ContextWindowExceeded → ContextWindowExceeded`, because that provider says
*this specifically* while OpenAI has no signal that maps to it. Neither
adapter keeps a bare `RateLimited` — one rate-limit variant on purpose, and
`RateLimitedAfter` carries whatever the header parsed to, absence included.
Which variants your own private enum needs is decided the same way — by what
your provider actually distinguishes, not by copying either list.

### HTTP status → variant

`crates/transync-openai/src/client/classify.rs` is the worked example.
Reuse the table's *shape* rather than reinventing which 4xx statuses are
worth retrying:

| Status | Provider error | Retryable? |
|---|---|---|
| 401, 403 | `Auth` (bare body excerpt, no `HTTP <status>:` prefix) | No |
| 429 | `RateLimitedAfter(parsed Retry-After)` | Yes |
| 408, 409, 425 | `Transport` — commonly retryable, not obvious from the code alone | Yes |
| 300–399 | `Rejected { status: Some(code), message }` — no redirect is followed; point `base_url` at the endpoint that answers directly | No |
| any other 400–499 | `Rejected { status: Some(code), message }` | No |
| everything else (1xx, 5xx and beyond) | `Transport` | Yes |

**Do not collapse the other 4xx into `Other`.** They carry the status as a
typed `u16`, and that is the whole point: `Some(404)` is a model name that
does not exist — an operator fault — which a consumer must be able to tell
from a 400 without parsing prose. Sending the family through the
unclassified catch-all instead is precisely the defect
`ProviderRejected.status` was introduced to remove, because `Other` reads
downstream as *unknown* and the honest handling of unknown is retry. If
your provider has no HTTP status to give, `ProviderRejected` still fits —
`status` is an `Option<u16>` precisely so `None` is expressible.

`transync-anthropic` adds one status to the transient set by name: 529, its
`overloaded_error`, which sits outside the 500–599 band.

Failures raised before any status is seen classify the same way in both
adapters: a `reqwest` timeout and a connect failure become `Transport`, a
decode failure becomes `Malformed`, anything else `Transport`. A body that
blows past the response-size cap is a terminal `ResponseTooLarge` naming the
cap.

### Envelope signals → variant

A 200 response is not a success. Read the stop/finish signal **before** the
message body, and map it:

| Provider signal | Variant |
|---|---|
| Chat `finish_reason: "length"` | `OutputCeilingExhausted` |
| Chat `finish_reason: "content_filter"` | `ContentFiltered` |
| Chat `refusal` field (arrives under `finish_reason: "stop"`) | `ModelRefused` |
| Responses `incomplete` + `incomplete_details.reason: "max_output_tokens"` | `OutputCeilingExhausted` |
| Responses `incomplete` + `reason: "content_filter"` | `ContentFiltered` |
| Responses `incomplete` + any other or absent reason | `Other` (deliberately not a borrowed name) |
| Responses `failed`, `cancelled`, `queued`, `in_progress` | `Transport` (retryable — the envelope-level analogue of a 5xx) |
| Messages `stop_reason: "max_tokens"` | `OutputCeilingExhausted` |
| Messages `stop_reason: "refusal"` **with** a `stop_details.category` | `ContentFiltered` |
| Messages `stop_reason: "refusal"` with no category | `ModelRefused` |
| Messages `stop_reason: "model_context_window_exceeded"` | `ContextWindowExceeded` |

A refusal **outranks** any text beside it. Walk the whole envelope, collect
refusal segments, and fail `ModelRefused` even when non-empty output text
also arrived — otherwise you translate the model's refusal prose as if it
were the document.

Cap every provider-controlled string before it leaves your crate. Both
adapters truncate refusal text, status vocabulary, `incomplete_details`
reasons and error bodies to a 512-byte char-safe excerpt with a
`… (N bytes total, truncated)` suffix, so a hostile or misbehaving gateway
cannot flood a host's stderr and logs with an echoed document. Classify on
the **raw** signal, not on the excerpt — a diagnostic detail must not decide
a variant.

## 4. Override `fingerprint()` if your instances are not interchangeable

The default derives the fingerprint from your type's name, which is correct
only when every instance of `MyProvider` produces identical output for
identical input. If your struct is configurable (model, endpoint, API
surface, reasoning effort, …), override it to cover every axis that can
change output — otherwise a shared `Cache` can replay one configuration's
translation for a different one:

```rust
fn fingerprint(&self) -> ProviderFingerprint {
    let base = self.base_url.as_ref().map(Url::as_str).unwrap_or(DEFAULT_BASE_URL);
    let effort = self.effort.map(Effort::as_str).unwrap_or("default");
    ProviderFingerprint::new("myprovider", &[&self.model.0, base, effort])
}
```

Over-distinguishing (an extra axis that never actually changes output) only
costs a redundant re-translation on a cache hit that could have happened.
Under-distinguishing lets a shared cache leak one config's output into
another's slot — the direction to avoid.

**Not every per-instance axis belongs in the fingerprint.** Both adapters
carry a `with_timeout(Duration)` builder and both deliberately exclude it: a
timeout changes only *whether* a response arrives in time, never *what* the
provider returns, so folding it in would split the cache namespace between
two hosts that merely tune their patience. `transync-anthropic` excludes one
more thing for a different reason — its `anthropic-version` header is a
crate-wide constant, not a per-instance value, so no two instances of one
build can disagree on it. If your own provider grows a knob in either
shape, leave it out and say so in a doc comment; a reader auditing the
fingerprint later has no other way to tell a deliberate omission from a
missed axis.

Also consider `tokenizer_hint()`: if your model names are not
OpenAI-shaped, override it rather than let the engine's name heuristic
guess wrong. Batch-budget estimation is a soft cap, so an approximation is
fine — an *undeclared* one is not. `transync-anthropic` returns a flat
`Some(TokenizerHint::Cl100kBase)` for every model, which is what a stated
approximation looks like.

`extract_glossary()` stays `Ok(None)` (unsupported) by default; implement
it only if your provider can usefully harvest candidate terminology in one
preflight call. If you do, remember it takes `cancel` too, and that
`Ok(Some(vec![]))` ("supported, found nothing") is a different answer from
`Ok(None)`.

## 5. Satisfy the behavior contract

The pipeline assumes these hold for every `translate_batch` call
(`contracts.md` §1's "Behavior contract"); a violation either corrupts
output or wastes a retry/fallback slot the pipeline allocated in good
faith:

- [ ] Exactly one `UnitResult` per unit in `batch.units` — no duplicates,
      no extras, none missing.
- [ ] `unit_id` on each result is the corresponding request unit's
      `unit_id`, byte-for-byte — no normalization, no re-casing.
- [ ] Units you cannot translate get `output_kind: FailedNeedsFallback`,
      not a faked or partial translation dressed as `Translated`.
- [ ] `translated_payload` never leaks a `RetryContext.reason` string —
      that field is a side channel *about* the payload, never content
      *inside* it.
- [ ] A unit whose `retry` field is `Some(_)` is a re-dispatch: its
      `source_payload` is byte-identical to the original attempt, and you
      SHOULD use `retry.rejected_by` / `retry.reason` to correct the named
      structural failure (you MAY ignore it, but you MUST NOT echo
      `reason` into the output).
- [ ] Errors are mapped to the variant the pipeline's retry policy expects
      (§3 above) — an error you mis-map as terminal aborts a run that a
      correct `Network`/`RateLimited` classification would have survived,
      and one you mis-map as transient burns the budget and then aborts
      anyway.

And, on the `cancel` argument of both async methods:

- [ ] You accept and observe the token, and you never cancel it.
- [ ] The race is `biased;`, so an already-cancelled run issues no request.
- [ ] If your work is **not** drop-cancellable — an inner `tokio::spawn`, a
      `spawn_blocking` client, an internal queue — you honor the token
      yourself. The pipeline's only fallback is dropping your future, and
      that reaches none of those.

Returning `TranslatorError::Cancelled` when the token did *not* fire is
legal — you may hold a cancellation source of your own — and it is
terminal, never re-dispatched.

## 6. Wire it in

No registration step — `transync::translate` takes any `Translator` by
generic bound (`T: Translator + ?Sized`, so a `&dyn Translator` works too),
and the core library has no compile-time knowledge of any provider crate:

```rust
let translator = MyProvider::new(/* … */);
let mut opts = transync::TranslateOptions::default();
opts.target_language = "ko".into();
let output = transync::translate(&source, &opts, &translator).await?;
```

`TranslateOptions` is `#[non_exhaustive]`, which is why that is
default-then-assign rather than a struct literal.

To make the run cancellable, put a token on the options —
`opts.cancel = Some(token)`; the default is `None`, which means the run
cannot be cancelled and behaves exactly as it did before the field existed.
Two consequences worth knowing before you build on it:

- A cancelled run returns `Err(TransyncError::Cancelled)` and **never** a
  partial `TranslationOutput`. Unreached blocks are not marked
  `fallback_source` — that marker means "attempted and could not be
  translated", which is a different fact.
- The progress it paid for lives in the **cache**, not in the return value.
  Swap in
  `transync::translate_with_cache(&source, &opts, &translator, &cache)`
  with a cache you own and a cancelled run costs one round of in-flight
  batches on the retry; plain `translate` builds a throwaway cache per call
  and keeps nothing.

The recipe for both sides of that is
[how to cancel a running translation](./cancel-a-running-translation.md).

## Read the reference implementations

`crates/transync-openai/src/client.rs` and
`crates/transync-anthropic/src/client.rs` are both *flow only* — read them
alongside their submodules for the parts that differ per provider:

| File | What it owns |
|---|---|
| `openai/client/dispatch.rs` | model name → which HTTP surface |
| `openai/client/chat.rs` | Chat Completions DTOs, body builders, envelope reader |
| `openai/client/responses.rs` | Responses API DTOs, body builders, envelope reader |
| `openai/client/endpoint.rs` | endpoint paths, base-URL normalization, path joining |
| `openai/client/transport.rs` | bearer-authenticated POST with a response-size cap |
| `openai/client/classify.rs` | HTTP status / `reqwest` failure → provider error (the table in §3) |
| `anthropic/client/messages.rs` | Messages DTOs, both body builders, the single envelope reader |
| `anthropic/client/schema.rs` | rendering the shared schema object into a narrower dialect |
| `anthropic/client/endpoint.rs` | the single Messages path, base-URL normalization |
| `anthropic/client/transport.rs` | `x-api-key` POST with the body caps |
| `anthropic/client/classify.rs` | the same status table, plus 529 by name |

What both `client.rs` files show worth copying: one shared `round_trip()`
helper that resolves the endpoint, POSTs, and hands the raw bytes to an
envelope reader — so transport and malformed-envelope handling exist
exactly once, whether the crate supports one HTTP surface or two.

Their constructors are worth copying too. Both crates offer an unchecked
`new` (validates nothing), a checked `try_new`, and a `from_env` that *is*
`try_new` once the variables are read — so the environment is never the
weaker door. The OpenAI crate adds a fourth, `offline`, which validates the
same configuration with no credential at all, for a run that intends to be
served entirely from cache. A padded model id is refused rather than
trimmed, because the
stored string is simultaneously the wire value and a `fingerprint()` axis.

## What is already done for you, and the one thing that is not

The workspace is at `0.5.0`. The v0.4.0 and v0.5.0 breaking windows are both
used and closed, so everything below is shipped rather than staged. Starting
fresh today, these are simply the shape of the contracts and cost you no
migration:

- `fingerprint()`, `tokenizer_hint()` and `extract_glossary()` are all
  **defaulted** methods. You write an override only where step 4 applies.
- The `Cache` trait has **seven** methods, and you probably implement none
  of them: `get` / `put` / `evict` are required and already fallible
  (`Result<_, CacheError>`), and `get_document_meta` / `put_document_meta` /
  `get_glossary_extraction` / `put_glossary_extraction` are defaulted to
  `Ok(None)` / `Ok(())`. Two backends ship and are re-exported from the
  facade — `InMemoryCache` and the disk-backed `DiskCache` — so a provider
  crate normally consumes the trait rather than implementing it.
- `TranslationUnit` is `#[non_exhaustive]` with the `retry` field already
  present; construct it (in tests, say) via `TranslationUnit::new(unit_id,
  block_kind, input_mode, source_payload, source_hash)` plus the `with_*`
  builders, never a struct literal.
- Every `TranslatorError` variant already carries a `stable_code()`. Pick
  the right variant and the wire code a downstream consumer keys on is
  correct for free.

The one thing that is **not** free: `translate_batch` and
`extract_glossary` gained the `cancel: &CancellationToken` parameter in the
v0.4.0 window (DCR-0024), released 2026-08-20. That is a signature change to a
required method, so no default can absorb it — an implementation written
against 0.3.0 does not compile until the parameter is added.

## Reference: the full trait and error set

For the austere, complete listing — every method signature, every error
variant with its stable code, the field-by-field behavior contract — see
[the `Translator` trait reference](../../../reference/developer/en/translator-trait.md)
rather than re-deriving it from this recipe. Why terminal-rather-than-degrade
is the pipeline's answer at all is in
[why layered validation and bounded retry/fallback](../../../explanation/developer/en/validation-retry-fallback-model.md).
What an operator sees when your mapping reaches the CLI is in
[how to diagnose a translation run](../../user/en/diagnose-a-translation-run.md).

## Verify

```bash
cargo build -p transync-myprovider
cargo test -p transync-myprovider
```

Then run a real batch through `transync::translate` against a small sample
document and confirm `output.validation_summary` shows the units you expect
as `translated` rather than `fallback_source` — a provider that technically
compiles but violates the behavior contract in §5 usually shows up there
first, as unexplained fallbacks or a pipeline-aborting `Err`.
