---
type: How-To Guide
title: How to cancel a running translation
description: Hand a `CancellationToken` to `translate_with_cache`, keep the progress a cancelled run paid for, and make your own `Translator` honour the token instead of relying on future-drop.
tags: [providers, llm, cancellation, DCR-0024, ADR-0017]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: core-lib, resource: crates/transync-core/src/lib.rs }
  - { id: core-llm, resource: crates/transync-core/src/llm.rs }
  - { id: core-error, resource: crates/transync-core/src/error.rs }
  - { id: core-pipeline, resource: crates/transync-core/src/pipeline.rs }
  - { id: core-dispatch, resource: crates/transync-core/src/pipeline/dispatch.rs }
  - { id: facade, resource: crates/transync/src/lib.rs }
  - { id: openai-lib, resource: crates/transync-openai/src/lib.rs }
  - { id: cancellation-tests, resource: crates/transync/tests/cancellation.rs }
  - { id: contracts, resource: docs/architecture/contracts.md }
synced_hash: 8456009607f5d8721f227fc3b0e9392f26e6d53356ade3747519d76fa2a7f652
---

# How to cancel a running translation

Use this when your own code drives `transync::translate` or
`transync::translate_with_cache` and something else decides the run should
stop — a shutdown signal, a client that disconnected, a job the operator
withdrew.

This is a library-side capability. The `transync` CLI exposes no cancel
flag; a CLI run stops when you kill the process.

## Prerequisites

- A Tokio runtime and a crate that depends on the `transync` facade.
- A `Translator` you either own or can configure. If you wrote it, step 4
  is the part that applies to you.

## 1. Get a token

`transync::CancellationToken` is a re-export of
`tokio_util::sync::CancellationToken` — the real ecosystem type, not a
facade-local wrapper. Use it through the re-export and you need no direct
`tokio-util` dependency of your own. If you do depend on `tokio-util`
directly, its version must unify with the one the facade pins; when it
does not, you get a compile error naming both crates rather than a silent
misbehaviour.

```rust
use transync::CancellationToken;

let token = CancellationToken::new();
```

In a daemon, derive the run's handle from the process-wide one instead of
making a fresh root token, so either signal stops the job:

```rust
let token = shutdown.child_token();
```

## 2. Put it on the run's options

```rust
use transync::{InMemoryCache, TranslateOptions, TransyncError};

let mut opts = TranslateOptions::default();
opts.target_language = "ko".into();
opts.cancel = Some(token.clone());
```

`TranslateOptions::cancel` is `Option<CancellationToken>` and defaults to
`None`, which means "this run cannot be cancelled" and behaves exactly as
it did before the field existed.

Set it **per run**. `TranslateOptions` is `Clone`, so a long-lived options
template carrying a token would hand every job the same one, and
cancelling a single job would cancel all of them — which is the reason the
field is an `Option` and not a bare token.

## 3. Run it with a cache you own, and read the cancelled answer

```rust
let cache = InMemoryCache::new();

match transync::translate_with_cache(&source, &opts, &translator, &cache).await {
    Ok(output) => { /* … */ }
    Err(TransyncError::Cancelled) => { /* stopped on your token */ }
    Err(e) => return Err(e),
}

// From anywhere holding a clone of the token — a signal handler, a
// request handler, a `DropGuard`:
token.cancel();
```

Two facts decide how you write the code around that `match`.

**A cancelled run answers `Err(TransyncError::Cancelled)` and never a
partial `TranslationOutput`.** Blocks the run never reached are *not*
marked `fallback_source`: that marker means "this block was attempted and
could not be translated", and borrowing it would make an abandoned run
indistinguishable from a degraded one in the alignment map. The stable
code for this error is `cancelled`.

**The progress you paid for lives in the cache, not in the return value.**
Every unit accepted before the token fired was written to the `Cache` as
it was accepted. So the entry point to use is `translate_with_cache` with
a cache you own: a re-run against the same cache re-dispatches only the
remainder. `translate` builds a throwaway in-memory cache per call and
drops it with the call, so a cancelled `translate` discards everything it
bought. For progress that survives the process, hand
`translate_with_cache` a `transync::DiskCache::open(dir)?` instead of an
`InMemoryCache`.

Where the token is observed, in order: run entry (before the parse — an
already-cancelled run issues no provider request at all), immediately
after the auto-glossary preflight, at each batch's entry, at the head of
each dispatch round, around every provider call, and around every
transport-backoff sleep. The run checks once more after the batch fan-out
settles, and **that check is authoritative**: a cancelled run reports
`Cancelled` even when a sibling batch also failed for its own reason.

Past that point only regeneration, alignment and rendering remain — pure
CPU, no I/O — and they are not interruptible. A token that fires there is
not observed, and the run returns `Ok`.

## 4. Honour the token in your `Translator`

Both trait methods take the run's token as a trailing argument. For the
exact signatures, see
[the `Translator` trait reference](../../../reference/developer/en/translator-trait.md);
the recipe here is what to do with the argument.

The obligation has three parts:

- **You must not cancel it.** The token is the caller's and is shared by
  every call of the run. Observe it only.
- **You should race your round-trip against it** and answer
  `TranslatorError::Cancelled` when it wins.
- **`biased;` is load-bearing.** It polls the token first, so an
  already-cancelled run never issues the request.
  `CancellationToken::run_until_cancelled` is biased the other way — it
  polls the inner future first — and would send it.

```rust
use async_trait::async_trait;
use transync::CancellationToken;
use transync::llm::{TranslationBatch, TranslationBatchResult, Translator, TranslatorError};

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

Apply the same shape to `extract_glossary` if you implement it. The
bundled OpenAI adapter does exactly this on both methods, which is what
lets a direct driver get a typed answer without waiting out that adapter's
per-request budget (120 seconds by default).

Two consequences of that pattern that are easy to get wrong:

- Returning `Cancelled` when the run's token did **not** fire is legal —
  an implementation may hold a cancellation source of its own — and it is
  **terminal**: the pipeline never re-dispatches it. It reaches a
  `translate` caller as `Translator(Cancelled)`, whose stable code is
  `provider_cancelled`, deliberately distinct from the engine's
  `cancelled`.
- The glossary preflight is asymmetric. An `extract_glossary` *error*
  degrades the run to the static glossary and is reported rather than
  fatal; an `extract_glossary` *cancellation* does not degrade. The
  pipeline re-reads the token after the preflight returns and aborts with
  `TransyncError::Cancelled` whatever your method answered, so a cancelled
  preflight can never resolve into "proceeded on the static glossary".

## 5. Decide whether ignoring the token is acceptable

Honouring it is a SHOULD, not a MUST, because the run ends either way: the
pipeline races every provider call against the same token and **drops** the
loser, and dropping a `reqwest`-shaped future aborts the in-flight request
by construction. Ignore the argument entirely and the run still returns
`Err(TransyncError::Cancelled)`.

What you give up by ignoring it:

- **The typed answer.** Your future is dropped mid-poll instead of
  returning `Cancelled`, so anything you drive directly — outside
  `translate` / `translate_with_cache` — learns nothing.
- **Work that drop does not stop.** An inner `tokio::spawn`, a
  `spawn_blocking` client, or a request already handed to an internal
  queue keeps running after your future is dropped. If your implementation
  has any of these, honouring the token is the only thing that stops the
  work, and the SHOULD is effectively a MUST for you.
- **Promptness on a direct call.** Without the select, a cancelled caller
  waits out whatever timeout your transport enforces.

## Verify

- [ ] `opts.cancel` is set from a per-run token, not from a shared options
      template.
- [ ] The run goes through `translate_with_cache` with a cache you own if
      you intend to resume; a cancelled `translate` keeps nothing.
- [ ] Your caller treats `TransyncError::Cancelled` as "stopped on
      request", not as a failure to report to a user.
- [ ] Your `Translator` observes the token and never calls `cancel()` on
      it.
- [ ] Your select is `biased;`, with the token arm first.
- [ ] Any non-drop-cancellable work inside your provider is stopped by the
      token, not by the pipeline dropping your future.

The behaviours this page relies on are pinned by the integration suite at
`crates/transync/tests/cancellation.rs` — including the already-cancelled
run that issues no request, the interrupted transport backoff, the
error-rather-than-partial-document answer, progress surviving in a
caller-owned cache, and a token-ignoring translator being cancelled by
being dropped.

## Related

- Every method signature and error variant, in full:
  [the `Translator` trait reference](../../../reference/developer/en/translator-trait.md).
- Writing the provider itself:
  [how to implement a custom Translator provider](./implement-a-custom-translator.md).
- Why a stopped run refuses to hand back a half-translated document:
  [the validation, retry and fallback model](../../../explanation/developer/en/validation-retry-fallback-model.md).
