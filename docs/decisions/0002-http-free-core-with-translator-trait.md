---
type: ADR
title: HTTP-free core with `Translator` trait + provider crates
description: The core crate owns no HTTP; consumers supply a `Translator` implementation (or use the default `transync-openai` sibling), keeping the core network-free and unit-testable.
tags: [decision, ADR-0002]
status: active
---

# ADR 0002: HTTP-free core with `Translator` trait + provider crates

## Context and Problem Statement

The library calls an LLM to translate units. The LLM call needs auth, retries, rate-limit handling, observability, and (eventually) provider portability. We must decide whether the core `transync` crate owns these concerns or whether it delegates them.

Two reference projects show two different choices:
- `LLM-API/LLM-Trans` builds the OpenAI client *into* the backend, with a thin internal `Provider` trait that exists for testability.
- `resp-translator` ships a `translation-provider` crate with an explicit trait used by the binary.

Neither makes the LLM call truly the *consumer's* concern, because both are end-user binaries. `transync` is a library; its consumers might want to bring their own client (with their own retry policy, their own tracing, their own rate-limit middleware).

## Decision Drivers

- The core library should be reusable in long-running applications that already have an HTTP stack and an opinionated retry / observability story.
- The MVP track is D: a thin CLI must work out of the box. Without *some* default provider, the CLI cannot run from a fresh checkout.
- Adding new providers (Anthropic, local llama, …) should be a pure-additive change, never a breaking change to the core crate.
- Testing the core pipeline must not require network access or fake HTTP servers.

## Considered Options

1. **Bring-your-own-provider via trait, with a sibling `transync-openai` default impl.** Core crate is HTTP-free. Consumer implements `Translator` (or uses `transync-openai`) and passes it in. CLI depends on `transync-openai`.
2. **Built-in OpenAI-compatible provider, hidden trait.** Core crate ships an internal `Provider` trait with an OpenAI Responses-API impl behind a default Cargo feature. Consumer normally just sets `api_key + model + base_url`.
3. **Built-in OpenAI only, no abstraction.** Hardcoded against OpenAI. Less code; harder to test deterministically; harder to add Anthropic/local-llama later.

## Decision Outcome

We chose **option 1**. The user selected it during brainstorming Q3.

The core `transync` crate exposes:

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
    ) -> Result<TranslationBatchResult, TranslatorError>;
}
```

Consumer owns the LLM transport. The default `transync-openai` sibling crate implements `Translator` against the OpenAI Responses API + Structured Outputs and ships in the same workspace so the CLI works out of the box. Future providers are sibling crates: `transync-anthropic`, `transync-local-llama`, …

Status: Decided 2026-05-01 (brainstorming Q3). Shipped — `transync-core` is HTTP-free; the `Translator` trait seam is implemented by `transync-openai` (model-driven Chat Completions + Responses dispatch).

### Implementation

- `crates/transync` declares the `Translator` trait, `TranslationBatch`, `TranslationBatchResult`, `TranslatorError`. **It does not depend on `reqwest`, `async-openai`, `tokio`'s networking features beyond the runtime, or any HTTP client.**
- `crates/transync-openai` depends on `transync` and on `async-openai` (or `reqwest` + handcrafted types). It re-exports its concrete type so consumers can construct it with `TransyncOpenAI::new(api_key, model, base_url)`.
- `crates/transync-cli` depends on both `transync` and `transync-openai`. The default profile expects `OPENAI_API_KEY`; the CLI surfaces a clear error when the key is missing.
- Future provider crates follow the same pattern. They never modify the core trait.

## Consequences

- **Good:** Core crate is unit-testable with a `MockTranslator` implementing the trait. SCN-07 / SCN-08 / SCN-09 / SCN-10 are verified without network.
- **Good:** Consumers with bespoke HTTP stacks can adopt the library without adopting our HTTP choices.
- **Good:** Adding a provider is a sibling crate. Zero churn in the core API.
- **Bad:** The CLI must depend on `transync-openai` directly, which means changing the default provider is technically a breaking change at the CLI level. Acceptable — providers are stable choices, not hot-swappable.
- **Bad:** Slightly more workspace ceremony than a single crate. Acceptable.

## Amendment (2026-08-06) — where the boundary is declared, not what it is

*Appended, not a rewrite. Every decision above stands: the core owns no HTTP,
the consumer supplies a `Translator`, and `transync-openai` is the default
sibling implementation. Only the "Implementation" map below the decision is
corrected — Review-0001 `R0001-0049` found it naming the wrong crate.*

- **Declaration site.** `Translator`, `TranslationBatch`,
  `TranslationBatchResult`, `TranslationUnit` and `TranslatorError` are
  declared in **`transync-core`'s `llm` module**. `crates/transync` is the
  facade — the semver firewall — and only *re-exports* them, which is why they
  appear in `contracts.md` §0 as `transync::…` paths. A new boundary type is
  therefore added to `transync-core::llm` and then curated into the facade;
  adding one to `crates/transync` itself is not even possible, as its `lib.rs`
  is nothing but `pub use` lines and declares no item of its own.
- **Which crates are HTTP-free.** The property the decision buys belongs to
  `transync-core` and `transync-syntax`, and to the facade above them: no
  member except `transync-openai` depends on `reqwest` or any other HTTP
  client. That is checkable in one grep of the workspace's manifests.
- **The provider crate's transport.** The either/or this ADR left open —
  "`async-openai` (or `reqwest` + handcrafted types)" — resolved to the second:
  `transync-openai` speaks to the API through `reqwest` with hand-written
  request/response types and no OpenAI SDK. It depends on the facade
  `transync`, as written here (plus test-only dev-dependencies on
  `transync-core` / `transync-syntax` for engine internals the facade stops
  re-exporting — OI-0027).
- **The constructor named here** — `TransyncOpenAI::new(api_key, model,
  base_url)` — is the *unchecked* one. `try_new` is canonical; it refuses an
  empty key or an unusable base URL at construction (R0001-0033,
  `contracts.md` §7).
- **The trait's shape** has grown three defaulted methods since this ADR
  (`fingerprint`, `tokenizer_hint`, `extract_glossary` — DCR-0009 / DCR-0014 /
  DCR-0015), so the single-method snippet above is the 2026-05-01 shape, not
  today's. `contracts.md` §1 carries the live one and its stability rules.

## Amendment (2026-08-08) — `TranslatorError` states a cause, not a policy

*Appended, not a rewrite. The decision stands unchanged: the core owns no
HTTP, the consumer supplies a `Translator`, and the trait's error type is the
seam. What is added is the shape of that error type, which this ADR named
(`TranslatorError`) without saying what it must carry. DCR-0023 / ticket
`1a85f3`; breaking, carried by v0.4.0.*

The trait is the whole boundary, so `TranslatorError` is the **only** channel
through which a provider tells the application what went wrong. Through v0.3.0
one variant, `Other(String)`, carried five distinct terminal causes — a
content-policy stop, an exhausted output ceiling, a model refusal, an oversize
response, and every non-transient 4xx — and the only way to separate them was
to match a message string that is explicitly not an interface. That is a
failure of *this* decision, not of the adapter: option 1 was chosen so that a
consumer could bring its own transport, its own retry policy and its own
observability, and none of the three is expressible over an error that will not
say what happened.

Three rules now govern the type, and a new provider crate inherits them:

- **A variant names why the provider stopped, never what the caller should do.**
  Retryability is deliberately absent. The pipeline's rule (re-dispatch
  `Network` and `RateLimited`, surface everything else) is one policy over this
  taxonomy, and a consuming application's user-facing policy is another. A
  predicate baked into the type would have frozen the first and pre-empted the
  second.
- **The catch-all is permanent.** `Other(String)` stays, and a `Translator`
  implementation SHOULD reach for it rather than force a quirk into a name that
  does not fit. Without it, "adding a provider is a sibling crate, zero churn in
  the core API" — this ADR's third decision driver — would fail the first time a
  provider had a failure mode the enum had not anticipated. `transync-anthropic`
  (ticket `bda471`) is the near-term test of that.
- **Every variant carries a stable machine code.** `TranslatorError::stable_code()`
  mirrors `TransyncError::stable_code()`, and `TransyncError` delegates to it, so
  the taxonomy survives the process boundary a `Translator`'s caller may sit
  behind. `contracts.md` §1 holds the vocabulary; a scrape test welds the two.

The live variant list is `contracts.md` §1's, not this file's — a decision
record should not become a second declaration site for a set that grows.

## Amendment (2026-08-08) — the boundary carries the caller's stop signal

*Appended, not a rewrite. The decision stands unchanged: the core owns no HTTP,
the consumer supplies a `Translator`, and the trait is the whole boundary. What
is added is that the boundary must also cross in the other direction — from the
application into the provider — which the 2026-05-01 single-method shape had no
place for. DCR-0024 / ticket `43331a`; breaking, carried by v0.4.0.*

`Translator::translate_batch` and `Translator::extract_glossary` now take a
`cancel: &CancellationToken` alongside their payload.

This follows from a decision driver already on the list above: *"the core
library should be reusable in long-running applications that already have an
HTTP stack and an opinionated retry / observability story."* A long-running
application also has an opinionated **lifecycle** story — a request that is
abandoned, a process that is draining — and the trait as declared gave it
nowhere to say so. The only stop signal available was dropping the future,
which is a property of async Rust rather than a promise this boundary made, and
which a consumer could only discover by reading transync's source.

Two rules govern the parameter, and a new provider crate inherits them:

- **Honoring it is a SHOULD, and the guarantee does not depend on it.** The
  pipeline races every provider call against the same token and drops the
  loser, so an implementation that ignores the argument is still cancelled.
  What honoring it buys is a typed answer (`TranslatorError::Cancelled`)
  instead of a silently dropped future, correctness for an implementation whose
  work is not drop-cancellable, and correctness when the implementation is
  driven directly rather than through the pipeline.
- **The token is the caller's, not the implementation's.** It is live and
  shared across every call of the run; an implementation observes it and must
  never cancel it.

That the token type is `tokio_util`'s rather than a transync-owned one is not a
retreat from "the core owns no HTTP" — a cancellation token is a
synchronization primitive, not transport — but it is the first foreign type in
the curated surface, and `contracts.md` §0 records the cost that carries.

The trait's live shape is `contracts.md` §1's, not this file's.

## Amendment (2026-08-10) — the second sibling is commissioned, and the seam is what it must hold

*Appended, not a rewrite. The decision stands unchanged: the core owns no HTTP,
the consumer supplies a `Translator`, and future providers are sibling crates.
What is added is that the future tense above ("`transync-anthropic`,
`transync-local-llama`, …") stops being hypothetical for the first name on the
list. DCR-0029 / ticket `bda471`; owner decision 2026-08-06 (Anthropic first,
local-llama later). Additive — the crate itself breaks nothing.*

`transync-anthropic` is commissioned as the second in-tree provider, designed
in DCR-0029 against the post-DCR-0023/0024 trait — the typed terminal causes
and the cancellation parameter — deliberately *after* both landed, so the
second implementation never existed against an interface about to move.

The commissioning is also this ADR's first real test, and three of its claims
are what the DCR holds the implementation to:

- **"Adding a provider is a sibling crate, zero churn in the core API."** The
  crate touches no core type. The one candidate exception is put to the owner
  as DCR-0029's open fork — Anthropic's `model_context_window_exceeded` stop
  reason has no honest variant in the taxonomy, and *naming a new cause* is
  vocabulary growth the 2026-08-08 amendment anticipated, not trait churn.
  Everything else the provider needed, the seam already carried.
- **The provider-neutral half is genuinely neutral.** The second adapter calls
  the same tier-(b) surface (`transync::llm::prompt`,
  `profile::render_prompt_body`) unmodified — prompt assembly, schema object,
  response parsing — under a schema transport OpenAI does not use
  (`output_config.format`). Where the provider's schema dialect is narrower,
  the adaptation lives in the adapter (DCR-0029's schema-profile pass), never
  in the shared surface.
- **The catch-all earns its keep.** The 2026-08-08 amendment named
  `transync-anthropic` as the near-term test of `Other(String)`; the answer:
  the second provider's stop-reason vocabulary maps onto named variants
  everywhere a cause fits, `Other` absorbs exactly the stop reasons the
  adapter cannot elicit by construction (`stop_sequence`, `tool_use`,
  `pause_turn`), and the one genuinely new cause goes to the owner as a fork
  instead of being flattened into it.

What this amendment does **not** do is restate the adapter's design —
constructors, terminal-cause table, packaging, slices are DCR-0029's and
`contracts.md` §8's. The CLI remains OpenAI-backed; this ADR's recorded "Bad"
consequence (the CLI's provider is a compile-time choice) is unchanged, and a
CLI provider axis is explicitly out of DCR-0029's scope.
