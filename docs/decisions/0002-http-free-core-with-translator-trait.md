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

The core owns no HTTP. Its provider boundary is the `Translator` trait, declared in `transync-core`'s `llm` module and re-exported by the `transync` facade. The one method an implementation must write is:

```rust
#[async_trait]
pub trait Translator: Send + Sync {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError>;

    // …and three defaulted methods, named below.
}
```

Besides that method the trait carries three defaulted ones (`fingerprint`, `tokenizer_hint`, `extract_glossary` — DCR-0009 / DCR-0014 / DCR-0015). The trait's live shape is `contracts.md` §1's, not this file's, and §1 carries its stability rules.

Consumer owns the LLM transport. The default `transync-openai` sibling crate implements `Translator` against the OpenAI API — model-driven Chat Completions + Responses dispatch, with Structured Outputs — and ships in the same workspace so the CLI works out of the box. Future providers are sibling crates. The first one named here, `transync-anthropic`, is built: it is the second in-tree provider, and its design is DCR-0029's and `contracts.md` §8's, not this file's. `transync-local-llama` stays deferred — the owner ordered it after Anthropic (ticket `bda471`), and ticket `3887f0` holds the open decision on what happens to it. The provider crates in the tree, and which of them the CLI carries, are `docs/implementation/module-map.md`'s to list.

Status: Decided 2026-05-01 (brainstorming Q3). Shipped — `transync-core` is HTTP-free; the `Translator` trait seam is implemented by `transync-openai` (model-driven Chat Completions + Responses dispatch) and by every further provider crate in the tree.

### Implementation

- **Declaration site.** `Translator`, `TranslationBatch`, `TranslationBatchResult`, `TranslationUnit` and `TranslatorError` are declared in **`transync-core`'s `llm` module**. `crates/transync` is the facade — the semver firewall — and only *re-exports* them, which is why they appear in `contracts.md` §0 as `transync::…` paths. A new boundary type is therefore added to `transync-core::llm` and then curated into the facade; adding one to `crates/transync` itself is not even possible, as its `lib.rs` is nothing but `pub use` lines and declares no item of its own.
- **Which crates are HTTP-free.** The property the decision buys belongs to `transync-core` and `transync-syntax`, and to the facade above them. **None of them depends on `reqwest`, `async-openai`, `tokio`'s networking features beyond the runtime, or any HTTP client.** An HTTP client is a provider crate's dependency and no other member's. That is checkable in one grep of the workspace's manifests.
- **The provider crate's transport.** `transync-openai` speaks to the API through `reqwest` with hand-written request/response types and no OpenAI SDK. It depends on the facade `transync` (plus test-only dev-dependencies on `transync-core` / `transync-syntax` for engine internals the facade stops re-exporting — OI-0027), and re-exports its concrete type, `TransyncOpenAI`.
- **The constructor.** `TransyncOpenAI::try_new(api_key, model, base_url)` is canonical; it refuses an empty key or an unusable base URL at construction (R0001-0033, `contracts.md` §7). `TransyncOpenAI::new(api_key, model, base_url)` is the *unchecked* one.
- **The CLI.** `crates/transync-cli` is the reference binary over the facade and the provider crates it carries. Which crates those are is `docs/implementation/module-map.md`'s; which provider a run selects, which one is the default, and the credential each provider path reads are `contracts.md` §6's. This record restates neither. On the OpenAI path the CLI expects `OPENAI_API_KEY` and surfaces a clear error when the key is missing.
- Future provider crates follow the same pattern. They never modify the core trait. A provider crate uses the provider-neutral tier-(b) surface (`transync::llm::prompt`, `profile::render_prompt_body`) unmodified — prompt assembly, schema object, response parsing. Where the provider's schema dialect is narrower, the adaptation lives in the adapter (DCR-0029's schema-profile pass), never in the shared surface. `transync-anthropic` holds to this under a schema transport OpenAI does not use (`output_config.format`).

### `TranslatorError` states a cause, not a policy

The trait is the whole boundary, so `TranslatorError` is the **only** channel
through which a provider tells the application what went wrong. An error that
cannot say what happened is a failure of *this* decision, not of the adapter:
option 1 was chosen so that a consumer could bring its own transport, its own
retry policy and its own observability, and none of the three is expressible
over an error that will not say what happened (DCR-0023, ticket `1a85f3`).

Three rules govern the type, and a new provider crate inherits them:

- **A variant names why the provider stopped, never what the caller should do.**
  Retryability is deliberately absent. The pipeline's rule (re-dispatch
  `Network` and `RateLimited`, surface everything else) is one policy over this
  taxonomy, and a consuming application's user-facing policy is another. A
  predicate baked into the type would have frozen the first and pre-empted the
  second.
- **The catch-all is permanent.** `Other(String)` stays, and a `Translator`
  implementation SHOULD reach for it rather than force a quirk into a name that
  does not fit. Without it, "adding a provider is a sibling crate, zero churn in
  the core API" would fail the first time a provider had a failure mode the
  enum had not anticipated. A genuinely new cause is named rather than
  flattened into `Other` — vocabulary growth, not trait churn — which is how
  Anthropic's context-window overflow became `ContextWindowExceeded`
  (DCR-0029). `Other` absorbs what fits no name: for that adapter, exactly the
  stop reasons it cannot elicit by construction (`stop_sequence`, `tool_use`,
  `pause_turn`).
- **Every variant carries a stable machine code.** `TranslatorError::stable_code()`
  mirrors `TransyncError::stable_code()`, and `TransyncError` delegates to it, so
  the taxonomy survives the process boundary a `Translator`'s caller may sit
  behind. `contracts.md` §1 holds the vocabulary; a scrape test welds the two
  (`crates/transync/tests/error_taxonomy.rs`).

The live variant list is `contracts.md` §1's, not this file's — a decision
record should not become a second declaration site for a set that grows.

### The boundary carries the caller's stop signal

`Translator::translate_batch` and `Translator::extract_glossary` take a
`cancel: &CancellationToken` alongside their payload (DCR-0024, ticket
`43331a`). That follows from the first decision driver: a long-running
application also has an opinionated **lifecycle** story — a request that is
abandoned, a process that is draining — and dropping the future is a property
of async Rust, not a promise this boundary makes.

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

### An adapter may not name an error its transport cannot produce

Each adapter owns its own mapping onto `TranslatorError`, and may keep its own
error type in front of it. The rule that governs such a type is narrower than
"keep the adapters' taxonomies identical", which would contradict this ADR's
whole premise that each adapter owns its own mapping: **an adapter may name any
error its transport can actually produce, and may not name one it cannot.** A
variant nothing constructs is not a capability claim — it is a promise to
consumers that a condition is distinguishable when it is not, and it survives
review precisely because dead code compiles (DCR-0040, R0009-0085).

Removing such a variant is not free. An adapter error type that is `pub` in a
published crate with no `#[non_exhaustive]` — `transync-openai`'s
`ProviderError` is one — loses a variant only as a breaking change, which needs
a sanctioned breaking window. With no window open, the correct outcome is to
leave the variant in place and record it.

## Consequences

- **Good:** Core crate is unit-testable with a `MockTranslator` implementing the trait. SCN-07 / SCN-08 / SCN-09 / SCN-10 are verified without network.
- **Good:** Consumers with bespoke HTTP stacks can adopt the library without adopting our HTTP choices.
- **Good:** Adding a provider is a sibling crate. Zero churn in the core API. `transync-anthropic` was the first test of it and touched no core type beyond one new `TranslatorError` variant — vocabulary growth, not trait churn (DCR-0029).
- **Bad:** The CLI must depend on `transync-openai` directly, which means changing the default provider is technically a breaking change at the CLI level. Acceptable — providers are stable choices, not hot-swappable. Which provider crates the binary carries is `docs/implementation/module-map.md`'s to list, and how a run selects among them is `contracts.md` §6's.
- **Bad:** Slightly more workspace ceremony than a single crate. Acceptable.

## History

The full text of each event is in [`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0002.

- 2026-08-06 (R0001-0049): Implementation map corrected — types declared in `transync-core::llm`; `transync-openai` on `reqwest`; `try_new` canonical.
- 2026-08-08 (DCR-0023, ti `1a85f3`): `TranslatorError` gained named causes, a stable code per variant and three rules; breaking, v0.4.0.
- 2026-08-08 (DCR-0024, ti `43331a`): `translate_batch` and `extract_glossary` gained `cancel: &CancellationToken`; breaking, v0.4.0.
- 2026-08-10 (DCR-0029, ti `bda471`): `transync-anthropic` commissioned as the second sibling provider, against the post-DCR-0023/0024 trait; additive.
- 2026-08-26 (DCR-0040, R0009-0085): an adapter may not name an error its transport cannot produce; `ProviderError::RateLimited` removed in the v0.5.0 window.
- 2026-09-23 (DCR-0058): the CLI gained `--provider openai|google`; the 2026-08-10 "CLI remains OpenAI-backed" gloss no longer holds.
