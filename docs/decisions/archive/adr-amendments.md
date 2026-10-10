---
type: archive
title: ADR amendments — every amendment section moved out of the ADR decision bodies
description: The historical half of the ADRs under docs/decisions/, moved out by the wave-5 documentation compaction (DCR-0054). Each ADR's amendment sections, corrections, numbering notes and dated appendices are kept verbatim under a heading naming the ADR, in ADR-number order. It is an archive — verbatim, append-only, never read for current truth. Each ADR's decision body states its current rule, and its `## History` list points here.
tags: [archive, decisions]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-06T00:00:00Z
status: deprecated
---

# ADR amendments

This is an **archive** (DCR-0054). An ADR's decision body is a living document:
it states the current rule, with a `## History` list of one line per event.
Every amendment section below was in an ADR once and is kept verbatim, under
the ADR it left.

Read this to answer *what did an ADR say before an amendment changed it, and
why*. For the current rule, read the ADR itself.

## ADR-0001 — Block-level alignment as the only sync currency

### Decision Outcome — the status line, verbatim before the DCR-0054 compaction

Status: Decided 2026-05-01 (brainstorming Q2). Shipped — block IDs are the sole sync currency end-to-end (Rust IR → LLM contract → alignment map → `data-sync-id` DOM anchors → JS sync engine); the sync engine was later amended by DCR-0008 (see Implementation).

### Implementation — the JS sync engine bullet, as amended by DCR-0008 (2026-07-10, OI-0006)

- The JS sync engine works against `[data-sync-id]` anchors only. No sentence-level lookups. *(Amended by DCR-0008: the originally-specified `IntersectionObserver` + intersection-ratio + viewport-center + hysteresis design was replaced by the shipped scroll-listener engine — RAF-coalesced scroll events, a reference-line active-block pick with intra-block progress fraction, per-frame lerp toward the partner target, and a per-pane programmatic-scroll lock. The authoritative algorithm description is `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`. The block-ID-as-only-currency principle this ADR decides is unchanged.)*

### Amendment (2026-08-09, ticket `d3acc3`) — ID identity is normative, not incidental

The Implementation note above records that `source_block_id` and
`target_block_id` are "identical for static documents". That was written as an
observation about the emitter. It is now a **stated invariant of alignment
schema 1.x**, and the consumer side is written against it:

- The reference JS engine pairs the two panes by **identical `data-sync-id`**.
  It always did (`handleScroll` looks the active block's own id up in the
  partner map); what changed is that this is the contract rather than an
  implementation detail one could read either way.
- The map's `source_block_id` / `target_block_id` indirection is therefore
  **RESERVED** — in the same sense as `parent_id` and the `child-only`
  `sync_role` — for a future revision in which the two documents' block sets
  may genuinely diverge. It stays in the wire format; nothing routes through
  it today, because under this ADR there is nothing for it to express.
- A schema-1.x map whose row contradicts the identity is refused by the engine
  rather than mounted. Review 0003's `R0003-0002` found the two consumer code
  paths disagreeing about it — the map/DOM drift warning resolved
  `target_block_id` while the scroll handler resolved `source_block_id`, so
  such a map warned correctly and then could not sync in either direction.
  Making the identity normative is what makes one of those two readings the
  right one; refusing the map is what keeps the other from being reached.

The block-ID-as-only-currency decision itself is unchanged; this names which
id, on which side, is that currency. The consumer contract lives in
`docs/architecture/contracts.md` §3.

## ADR-0002 — HTTP-free core with `Translator` trait + provider crates

Source: `docs/decisions/0002-http-free-core-with-translator-trait.md`. Moved verbatim when the
decision body was consolidated under DCR-0054; the body's `## History` list indexes these events.

### Decision Outcome — the wording the consolidation replaced

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

### Implementation — the wording the consolidation replaced

- `crates/transync` declares the `Translator` trait, `TranslationBatch`, `TranslationBatchResult`, `TranslatorError`. **It does not depend on `reqwest`, `async-openai`, `tokio`'s networking features beyond the runtime, or any HTTP client.**
- `crates/transync-openai` depends on `transync` and on `async-openai` (or `reqwest` + handcrafted types). It re-exports its concrete type so consumers can construct it with `TransyncOpenAI::new(api_key, model, base_url)`.
- `crates/transync-cli` depends on both `transync` and `transync-openai`. The default profile expects `OPENAI_API_KEY`; the CLI surfaces a clear error when the key is missing.
- Future provider crates follow the same pattern. They never modify the core trait.

### Amendment (2026-08-06) — where the boundary is declared, not what it is

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

### Amendment (2026-08-08) — `TranslatorError` states a cause, not a policy

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

### Amendment (2026-08-08) — the boundary carries the caller's stop signal

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

### Amendment (2026-08-10) — the second sibling is commissioned, and the seam is what it must hold

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

### Amendment (2026-08-26, DCR-0040) — an adapter may not name an error its transport cannot produce

*Appended, not a rewrite. Everything above, and the 2026-08-08 amendment that
DCR-0023 added, stand as written.*

DCR-0023 gave the provider adapters a shared error taxonomy. It did not say what
happens when an adapter carries a variant **nothing constructs**, and one did:
`transync-openai`'s `ProviderError::RateLimited` was unreachable, while the
later `transync-anthropic` was written without it.

Review 0009 (R0009-0085) found it; `8d4efda` removed it. `ProviderError` is
`pub` in a published crate with no `#[non_exhaustive]`, so that is a breaking
change — permitted **only** because `ff788f7` had opened the sanctioned v0.5.0
window hours earlier. With the window closed the correct outcome would have been
to leave the variant in place and record it.

The rule this leaves is narrower than "keep the adapters' taxonomies identical",
which would contradict this ADR's whole premise that each adapter owns its own
mapping: **an adapter may name any error its transport can actually produce, and
may not name one it cannot.** A variant nothing constructs is not a capability
claim — it is a promise to consumers that a condition is distinguishable when it
is not, and it survives review precisely because dead code compiles.

## ADR-0003 — Cargo workspace with provider crates as separate workspace members

Source: `docs/decisions/0003-cargo-workspace-with-provider-crates.md`. Moved verbatim when the ADR's decision body was rewritten to state the current rule (DCR-0054, compaction wave 5). The ADR body is current truth; nothing below is.

### Decision Outcome — workspace shape and status paragraph, as they read before compaction

Workspace shape (as amended by DCR-0005, DCR-0017, and DCR-0020):

```
transync/
├── Cargo.toml             # [workspace] members (explicit list)
├── crates/
│   ├── transync-syntax/   # wasm32-compilable base: parser, IR + IDs, regen, render, align, htmlseg
│   ├── transync-core/     # pipeline on top: unit, batch, llm, validate, cache, profile, pipeline
│   ├── transync/          # public facade: re-exports transync-core + transync-syntax
│   ├── transync-cli/
│   ├── transync-openai/
│   └── transync-wasm/     # browser (wasm-bindgen) surface over transync-syntax; publish = false
```

Status: Decided 2026-05-01 (brainstorming Q3). Amended 2026-07-10 by
DCR-0005: the structural core was extracted to `crates/transync-core/`;
`crates/transync/` remains the public dependency target as a re-export
facade (and will select a markdown/html dialect front-end once the
dialect split lands). The integration-test suite stays in the facade
crate. The dependency DAG gains one edge: `transync → transync-core`;
the drivers above (one-command build, compile-time wire compatibility,
localized provider addition) are unchanged. Shipped: the four-member
workspace (`transync-core`, `transync`, `transync-openai`,
`transync-cli`) builds from a clean checkout with one
`cargo build --workspace`.

### Amendment 2026-08-04 (DCR-0017) — fifth member, and Core does have an internal dependency

*Appended, not a rewrite. The original text above stands as the record of
what was decided in 2026-05-01 and amended in 2026-07-10.*

The workspace is now **five members**. `crates/transync-syntax/` was
extracted from `transync-core` as the wasm32-compilable base crate (OI-0028
Option B): it owns `parser`, `id`, `regen`, `render`, `align`, `htmlseg`,
`outcome`, `walk`, and `error::ParseError`; `transync-core` keeps `pipeline`,
`unit`, `batch`, `llm`, `validate`, `cache`, `profile`, and
`error::TransyncError`. See the tree above.

Two statements in the *Implementation* section below are superseded by this
amendment:

- **"Core has no internal dependencies" is no longer true.** The DAG gains
  one edge: `transync-core → transync-syntax`. The full DAG is now
  `transync-cli → {transync, transync-openai}`, `transync-openai → transync`,
  `transync → {transync-core, transync-syntax}`,
  `transync-core → transync-syntax`, and `transync-syntax` is the sole
  dependency-free member. It is still a strict DAG.
- **"Three `Cargo.toml` files instead of one"** (the *Bad* tradeoff line) is
  now **five**. Still acceptable, and for a new reason: the extra manifest is
  what makes
  `cargo check -p transync-syntax --target wasm32-unknown-unknown` a
  compiler-enforced gate instead of a documented intention. `transync-syntax`
  deliberately declares **no `[features]`** — a feature there would
  reintroduce the feature-unification trap the crate split exists to avoid.

The explicit-`members`-list rule is unchanged and now load-bearing for the
gate: `transync-syntax` is listed by name, never via a `crates/*` glob. The
original decision drivers (one-command build, compile-time wire
compatibility, localized provider addition, no `CARGO_TARGET_DIR` override)
are all unchanged.

### Amendment 2026-08-05 (ADR-0019 / DCR-0020) — sixth member, and the first one that is not published

*Appended, not a rewrite. Everything above stands as written.*

The workspace is now **six members**. `crates/transync-wasm/` is the browser
(wasm-bindgen) surface over `transync-syntax` — Track C proper. It differs
from every existing member in three ways worth naming:

- **`publish = false`, and `crate-type = ["cdylib", "rlib"]`.** It is a build
  target for a demo, not a library anyone depends on. Nothing in the workspace
  consumes it. *(2026-08-05, later the same day — ticket `92ac61b9`: the `rlib`
  is gone, `crate-type = ["cdylib"]`. The reasoning above is why it could go —
  nothing consumed the artifact — and it was worth 101,228 B of fat-LTO scope.
  `cargo test`/`clippy --workspace` still sweep the member; the lib target is
  compiled into its own test harness regardless of crate-type.)*
- **It depends on `transync-syntax` alone** among workspace crates, and the
  restriction is a decision rather than a coincidence: `transync-core`'s
  `wasm32` dependency tree reaches `getrandom` through tokio/tiktoken and
  `transync-syntax`'s does not, so a widening edge would break the link rather
  than merely the charter. The DAG gains `transync-wasm → transync-syntax` and
  stays strict; `transync-syntax` remains the sole dependency-free member.
- **It extends the gate rather than adding one.** The standing check is now
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  in both pre-commit hook copies and `scripts/smoke.sh`. `transync-syntax`'s
  own manifest is untouched — still no `[features]`, still no core dependency.

The *Bad* tradeoff line below ("Three `Cargo.toml` files", amended to five in
2026-08-04) is now **six**, and the newest one pays for itself the same way the
fifth did: it is what makes the browser surface a compiler-checked target
instead of a script that happens to work.

### Amendment 2026-09-02 (DCR-0029 / DCR-0032 / ADR-0028 / DCR-0045) — the seventh, eighth and ninth members

*Appended, not a rewrite. Everything above stands as written. The note is
late, and covers three members that joined on three separate dates — the last
of them, `crates/transync-lang`, is one that DCR-0045 already cites as an amendment
to this ADR.*

The workspace is now **nine members**, in the `members` list's own order:
`transync-html`, `transync-lang`, `transync-syntax`, `transync-core`,
`transync`, `transync-openai`, `transync-anthropic`, `transync-cli`,
`transync-wasm`. The three that arrived after the sixth:

- **`crates/transync-anthropic/`** — the second provider, added 2026-08-10
  (DCR-0029). It is the shape this ADR predicted: future provider crates land
  at `crates/transync-anthropic/`. The DAG gains `transync-anthropic →
  transync`, exactly the edge `transync-openai` has, and nothing else moves —
  no workspace member depends on the adapter, because the CLI is OpenAI-backed
  by DCR-0029's own scope.
- **`crates/transync-html/`** — the HTML mechanics, added 2026-08-20
  (DCR-0032), lifted out of `transync-syntax`'s `htmlseg` module (the name the
  tree diagram above still carries). DCR-0032 recorded **no amendment** to
  this ADR deliberately, and that judgement stands: the extraction follows
  this ADR's shape rather than changing it. What moves is the count, and one
  sentence of the 2026-08-04 amendment — `transync-syntax` is no longer
  dependency-free. `transync-syntax → transync-html` is the new edge, it is
  still a strict DAG, and `transync-html` takes over as the engine stack's
  dependency-free base. The new
  member carries the same two prohibitions for the same reason — **no
  `[features]`**, and no dependency on a workspace member — because it now
  sits under the `wasm32-unknown-unknown` gate transitively. The gate's
  command string is unchanged and must stay unchanged: it names
  `transync-syntax` and `transync-wasm`, and reaches this crate through the
  first.
- **`crates/transync-lang/`** — the source-language gate, added 2026-09-02
  (ADR-0028 / DCR-0045), and the first member that is neither an engine layer
  nor a provider. It **depends on no workspace member**, and that absence is
  load-bearing rather than incidental (ADR-0028): a second local answer to
  "what language is this" is precisely what the gate must not become. So it is
  the second member with no internal dependency, and it answers a pre-flight
  question for the caller rather than a question inside the pipeline.

Two statements below move with this note:

- The *Implementation* section's parenthetical members list stops at
  `crates/transync-wasm` (2026-08-05); read `crates/transync-anthropic`
  (2026-08-10), `crates/transync-html` (2026-08-20) and `crates/transync-lang`
  (2026-09-02) after it. The explicit-`members`-list rule is unchanged and now
  load-bearing twice over: `transync-wasm`'s `publish = false` makes the
  publication roster the other **eight**, and a first publication has to walk
  those eight in dependency order.
- The *Bad* tradeoff line's manifest count — three, then five, then six — is
  now **nine**.

The original decision drivers (one-command build, compile-time wire
compatibility, localized provider addition, no `CARGO_TARGET_DIR` override)
are all unchanged.

### Implementation — the members-list and dependency-DAG bullets, as they read before compaction

- Workspace root `Cargo.toml` declares an explicit `members` list (`crates/transync-core`, `crates/transync`, `crates/transync-openai`, `crates/transync-cli`; `crates/transync-syntax` added 2026-08-04; `crates/transync-wasm` added 2026-08-05) — not a `crates/*` glob — and a `[workspace.package]` block for shared metadata (license, edition, repository).
- The dependency graph is a strict DAG: `transync-cli → {transync, transync-openai}`, `transync-openai → transync`. Core has no internal dependencies. *(Superseded 2026-08-04 by DCR-0017 — see the amendment above: `transync-core → transync-syntax`.)*

### Consequences — the manifest-count line, as it read before compaction

- **Bad:** Three `Cargo.toml` files instead of one — **five** since 2026-08-04 (DCR-0017), **six** since 2026-08-05 (DCR-0020). Acceptable; see the amendments above for why the fifth and sixth pay for themselves.

## ADR-0004 — Comrak as the GFM parser

From `docs/decisions/0004-comrak-as-gfm-parser.md` (ADR-0004), moved verbatim under DCR-0054 when the decision body was restated as the current rule. The first four sections below are the text the ADR carried from its 2026-05-01 decision until then — its *Context and Problem Statement*, *Decision Drivers*, the *Implementation* subsection of its *Decision Outcome*, and *Consequences*; the fifth is its one amendment, dated 2026-08-06 (Review-0001 `R0001-0050`). The ADR's *Considered Options* and the two opening paragraphs of its *Decision Outcome* were not changed and are not repeated here. Read the ADR, not this section, for the current rule.

### Context and Problem Statement

The library needs a Rust GFM parser that:
- Produces an AST or equivalent IR with stable structural access (block path, parent, children).
- Supports GFM tables, fenced code blocks with info strings, task list markers, strikethrough, autolinks.
- Lets us record source ranges for each block (byte or line/column).
- Reserializes back to Markdown for the regeneration step.

`references/draft.md` §4.4 names two candidates: Comrak and pulldown-cmark.

### Decision Drivers

- We need block-level identity that survives parse → mutate → reserialize. An AST-shaped parser supports this naturally; an event stream forces us to bookkeep the AST ourselves.
- We need to reserialize translated Markdown back. Comrak ships an HTML and a CommonMark/GFM serializer; pulldown-cmark only emits HTML.
- We need to render annotated HTML with our own attributes. Both parsers can do this — Comrak via AST visitors, pulldown-cmark via event interception.
- Performance is not a primary driver at MVP scale (single-document translation, not bulk indexing).

### Implementation

- `crates/transync` depends on `comrak` (latest stable) with default features plus `arena_tree` for AST manipulation.
- The parser module wraps Comrak's `parse_document` to produce a `Document` IR that adds:
  - `block_id` per sync-relevant block (assigned in source order; format finalized Phase 2).
  - `source_range` per block (byte offsets recovered from Comrak's `start_line` / `start_column` / source slicing).
  - `source_hash` per block (for cache keys + integrity checks).
- The regenerator uses Comrak's GFM serializer for non-mutated subtrees and a custom path for mutated table cells / code-block bodies (where translated content replaces source content).
- The renderer extends Comrak's HTML renderer to emit `data-sync-id`, `data-block-kind`, `data-order`, `data-fallback` attributes on the `<section>` / `<table>` / `<pre>` / `<blockquote>` / `<li>` wrapper for each sync-relevant block.

### Consequences

- **Good:** One parser handles parse → mutate → reserialize → render → reparse. SCN-14 (full-document reparse) is straightforward.
- **Good:** AST visitors cleanly localize the ID assignment, the renderer attribute injection, and the validator's per-kind checks.
- **Good:** Comrak handles GFM tables, task lists, strikethrough, autolinks out of the box. No custom GFM extensions to maintain.
- **Bad:** Comrak's AST is more allocation-heavy than pulldown-cmark's event stream. Not a concern at MVP scale.
- **Bad:** Comrak's serializer emits *its* canonical Markdown formatting, not the source's exact whitespace. Acceptable per draft NG1 (semantic preservation, not byte-for-byte).

### Amendment (2026-08-06) — regeneration splices source bytes; no Markdown serializer is used

*Appended, not a rewrite. The decision stands: Comrak is the sole GFM parser,
and one shared `comrak_options()` serves parse, per-kind validation, fragment
and full reparse, and render. What is corrected is the "Implementation"
bullet claiming the regenerator reserializes through Comrak's GFM serializer —
Review-0001 `R0001-0050`. It never has.*

- **How regeneration actually works.** `regen::regenerate` walks the
  **top-level** blocks in source order, copies the text *between* blocks
  verbatim, and at each block's recorded source range splices either the
  accepted translated payload or — when the block is absent from the accepted
  map — that block's own source bytes. Comrak's CommonMark/GFM serializer
  (`format_commonmark`) is called **nowhere** in the workspace; the only
  Comrak serializer in use is `format_html`, in `render`. Comrak still does
  the *reading* on this path: the code-block extractor reparses a fragment to
  recover its body and info string, and validation reparses both the fragment
  and the full regenerated document.
- **The one content-aware path** is the fenced code block (invariant 4): the
  payload's body is extracted, then re-wrapped by `regenerate_code_fence` with
  a fence at least one character longer than the longest run of that fence
  character inside the body, switching backticks to tildes when the info
  string itself contains a backtick. `regenerate_table` exists but is the
  identity function — the row-window splitter it reserves room for is
  deferred, and whole-block tables need no reassembly.
- **What that buys, and what it costs.** The "Bad" consequence above about
  canonical formatting does not bite: everything the translation did not touch
  is copied byte-for-byte, so a fully-fallback document is byte-identical to
  its source — the property DCR-0002 / DCR-0004 rely on to skip the verifying
  reparse. The cost is the other side of the same coin: splicing is top-level
  only, so a nested block (a paragraph inside a blockquote, an item inside a
  list) is not replaced individually — its whole top-level container is the
  unit. `regen`'s module docs record the AST-splicing follow-up.
- **Crate home and dependency.** Since DCR-0017 `parser`, `regen` and `render`
  live in **`crates/transync-syntax`**, not `crates/transync` (which declares
  nothing and only re-exports); `validate`, which drives the fragment and
  full-document reparses, stays in `transync-core` and calls into the parser
  from there. The dependency is declared once in the workspace root as
  `comrak = { version = "0.27", default-features = false }` — no extra feature
  is enabled; the AST arena is Comrak's own, reached through
  `comrak::Arena` + `parse_document`, not a separate `arena_tree` feature.

## ADR-0006 — Renderer output shape — two pane files plus shell

Source: `docs/decisions/0006-renderer-output-shape.md`. Moved verbatim when the
ADR's decision body was rewritten to state the current rule (DCR-0054,
compaction wave 5, 2026-10-06). This is history, never current truth: the
ADR's own decision body is the rule. The first five sections below are the
decision-body passages as they read just before that rewrite; the rest are
the eight dated amendment sections, in the order the ADR carried them.

### Frontmatter `description:` — as it read before compaction

```yaml
description: The renderer emits fragment-shaped source.html and target.html; the CLI adds the demo shell, alignment JSON, sync.js, and vendored DOMPurify, committed as one staged six-file fileset — atomic per file, and see the 2026-08-12 amendment for what "one atomic set" does and does not guarantee.
```

### Decision Drivers — the Renderer/JS separation driver, as it read before compaction

- **Renderer/JS separation.** `transync::render` should emit just the annotated block-tree HTML; layout, mounting, and demo chrome are CLI concerns. Coupling layout into the renderer makes the WASM-track-C swap harder.

### Decision Outcome — the replaced passages, as they read before compaction

```
<--html-out>/
├── index.html        # demo shell — fetches source.html, target.html, alignment.json; loads sync.js
├── source.html       # annotated source block tree (no shell, no head, just the block-tree fragment in a single <main>)
├── target.html       # annotated target block tree (same shape as source.html)
├── alignment.json    # AlignmentMap JSON, schema_version 1.0.0
├── sync.js           # vanilla ESM sync engine (copied from web/js/sync.js at compile time)
└── purify.min.js     # vendored DOMPurify (OI-0001) — shells fail closed if absent
```

`source.html` and `target.html` are **fragment-shaped** — each contains a single `<main>` element wrapping the block tree, with no `<html>`, `<head>`, or `<body>`. The demo shell `fetch()`es each fragment, sanitizes it with `DOMPurify.sanitize()`, and assigns the result to the slot's `innerHTML` (OI-0001) — failing closed and refusing to mount when `DOMPurify` is unavailable. This matches `persistence-and-files.md` §output-directory-layout.

`transync::render` returns these fragment strings — it never templates the shell. The shell is templated inside `transync-cli::output` against an embedded `index.html.tpl`.

Status: Decided 2026-05-01 (Phase 3). Shipped — the six-file bundle is emitted atomically by `transync-cli::output` and mounted by the vendored shell; amended 2026-07-10 by OI-0001 to add `purify.min.js` and sanitize fetched fragments before mount.

### Implementation seams — as the list read before compaction

- `transync::render::render_source(doc, alignment) -> String` — fragment only.
- `transync::render::render_target(doc, translated_md, alignment) -> String` — fragment only.
- `transync-cli::output::write_html_bundle(dir, source_fragment, target_fragment, alignment_json) -> io::Result<()>` — commits all six files as a single staged fileset — `index.html`, `source.html`, `target.html`, `sync.js`, `purify.min.js`, and `alignment.json` (the last from the `--map` argument when `--html-out` is also set).
- `transync-cli/web/index.html.tpl` is `include_str!`'d at compile time. The template has named placeholders (e.g. `{{TRANSYNC_TITLE}}`, `{{ALIGNMENT_MAP_REL}}`) but no per-document content beyond title.

### Consequences — the Bad: six files bullet and its Correction (2026-08-16, ti `1c79f8`)

- **Bad:** Six files instead of one. Mitigated by the fact that the CLI ships them together as one atomic set; they are served by an external static server (e.g. `python3 -m http.server`), since `transync serve` is a deferred placeholder (STUB-061).

  > **Correction (2026-08-16, ti `1c79f8`).** The bullet's *cost* stands — six
  > files are still six files — but both halves of its mitigation have expired,
  > and both are corrected in appended amendments below rather than here. A
  > reader skimming this list for current operational guidance should not act on
  > the sentence as written:
  >
  > - **`transync serve` is not a placeholder and STUB-061 is closed.** It binds
  >   `127.0.0.1:7470` and serves the `--rendered` directory, path-confined,
  >   with a content-type table covering exactly this bundle — so the external
  >   static server is an option, not the step it reads as. See *Amendment
  >   (2026-08-09)*; `contracts.md` §6 carries the contract and
  >   `docs/project/stub-manifest.md` records the closure.
  > - **"One atomic set" is a staging guarantee, not a rollback promise.** The
  >   files mode can leave a mixed set if the rename pass fails partway; the
  >   set-level property is `--out-dir`'s. See *Amendment (2026-08-12)*, which
  >   names this bullet by title.

### Amendment 2026-08-04 (DCR-0017) — mechanism, not decision

*Appended, not a rewrite. Every decision above stands: two fragment-shaped
pane files plus a templated shell, `render_source` / `render_target`
signatures unchanged, renderer never templates the shell.*

Two statements about *how* the renderer works are now stale:

- **Crate home.** `render` (and `render/attrs`) live in
  **`crates/transync-syntax/src/`**, not `crates/transync-core/src/`. The
  paths `transync::render::render_source` / `render_target` are unchanged —
  core re-exports the module and the facade re-exports core.
- **Rendering mechanism.** The renderer used to reparse each block's Markdown
  slice as an isolated fragment and then string-strip Comrak's outer wrapper.
  It now does **one Comrak parse per pane** and zips the top-level AST node
  sequence against the alignment rows, formatting the paired node (its
  children for the strip-kinds, the whole node for table / code block /
  blockquote). The emitted attribute set, wrapper elements, and
  one-line-per-block shape are unchanged; three rendered byte shapes changed
  (loose lists, loose task items, and a fixed bug where 4-space-indented code
  blocks rendered as `<p>`) — see DCR-0017.

The consequence line *"Renderer stays a pure function from IR + alignment →
block-tree HTML. WASM track-C reuses it byte-for-byte"* is the one this
amendment makes literally true: the renderer's crate now compiles for
`wasm32-unknown-unknown` under a standing gate, which is what the 2026-05-01
decision was reserving room for.

### Amendment (2026-08-06) — the seams have been renamed, split, and narrowed

*Appended, not a rewrite. Every decision above stands: six files, two
fragment-shaped pane files, a templated shell the renderer never writes.
Corrected here is the "Implementation seams" list, which still names an API
and a template placeholder that no longer exist — Review-0001 `R0001-0051`.*

- **`write_html_bundle` is gone.** It was renamed and split on 2026-07-13
  (`12545e3`) into two seams with different jobs. `output::html_bundle_files`
  is a **pure** function: it fills the template and returns the six
  `(path, bytes)` pairs, deciding names and contents but writing nothing. It
  takes more than the four arguments listed above — the language labels and
  the two direction attributes are template inputs too (next bullet).
  `output::write_fileset_atomic` then performs the **staged fileset commit**:
  every payload is staged as `<path>.tmp.<pid>` and fsynced, and only when all
  of them are staged does a second pass rename each into place, under a
  publish lock over the destination directories. It is generic over any
  fileset, so `out.md`, the alignment map, and the validation report are
  committed by the same call — the six-file bundle is one caller of it, not
  its definition.
- **The template's placeholders are all `TRANSYNC_*`.** `{{ALIGNMENT_MAP_REL}}`
  no longer exists; the shell's inline module fetches `source.html`,
  `target.html` and `alignment.json` by fixed relative name, so the bundle's
  own file names are not parameterized. What *is* substituted:
  `{{TRANSYNC_TITLE}}`, `{{TRANSYNC_DOC_LANG}}`, and the four per-pane
  attribute slots `{{TRANSYNC_SOURCE_LANG_ATTR}}` /
  `{{TRANSYNC_TARGET_LANG_ATTR}}` / `{{TRANSYNC_SOURCE_DIR_ATTR}}` /
  `{{TRANSYNC_TARGET_DIR_ATTR}}`. So "no per-document content beyond title" is
  no longer true either: the resolved language labels reach the shell
  (attribute-escaped, since they are caller text), and RTL targets get a
  `dir` attribute on the panes — never on `<html>`, which would flip the demo
  chrome.
- **The render seam is not public.** `render_source` / `render_target` keep
  the signatures and the semantics recorded above, but they are
  `transync-syntax::render`'s and are called by `transync-core`'s pipeline,
  which hands the two fragments to the CLI on `TranslationOutput`. Since
  DCR-0018 the facade re-exports no renderer, so the 2026-08-04 amendment's
  `transync::render::…` paths no longer resolve; a consumer that wants to
  render for itself depends on `transync-syntax` directly (`contracts.md` §0
  tier (c)) — which is exactly what `transync-wasm` does.
- **The map in the bundle is `schema_version` 1.2.0**, not the 1.0.0 the file
  tree above records; `contracts.md` §3 owns that number, and the bundle
  simply writes the bytes the run produced.

### Amendment (2026-08-07) — a seventh placeholder: the opt-in bundle CSP

*Appended. The decision above is unchanged: six files, a templated shell, no
per-document content the run did not resolve. What changes is the placeholder
inventory the 2026-08-06 amendment lists.*

- **`{{TRANSYNC_CSP_META}}`** is the seventh slot, and unlike the other six it
  is not caller text at all: `--strict-csp` (OI-0018, posture amended
  2026-08-07) selects between a fixed `Content-Security-Policy` `<meta>`
  literal and the empty string, so the shell cannot be injected into through
  it. `html_bundle_files` therefore gained a `bool`, not a ninth `&str`.
- **The slot sits at the end of the charset line**, so the default (empty)
  substitution leaves `index.html` byte-identical to the pre-flag output —
  which is the whole posture: the CSP is opt-in, and the accepted default
  tradeoff (remote images render) stands. `contracts.md` §6 "Bundle
  Content-Security-Policy" owns the policy text and its derivation.

### Amendment (2026-08-07) — the title slot carries document content, so the fill changed shape

*Appended. The decision above is unchanged: six files, two fragment-shaped pane
files, a templated shell the renderer never writes. What changes is one
sentence of the original "Implementation seams" list — "no per-document content
beyond title" — which the 2026-08-06 amendment already corrected for the
language labels and which is now wrong in the other direction too: the title
itself is per-document content (ti 0f26b5, closing STUB-062/063).*

- **`{{TRANSYNC_TITLE}}` is resolved, not a literal.** `--title` when given,
  else the source document's **first level-1 heading** as parser-plain text,
  else the `transync` literal the slot always carried. The middle level is not
  a second extraction: it is `TranslationOutput::document_title`, the same
  value the provider already read as `BlockContext.document_title`, so the
  document is named identically to the model and to the reader.
  `contracts.md` §6 "Bundle title and language" owns the precedence.
- **The alignment map was deliberately not extended.** A document title is
  presentation, and the map is a durable inter-process wire shape (§3); making
  the bundle's title a schema field would have moved `schema_version` for
  something no sync consumer reads. Bundle assembly reads pipeline state
  instead — which is also why `--title` never reaches `out.md`, the map, or the
  provider, exactly like `--target-direction`.
- **The fill became a single pass** (`output::fill_template`) instead of a
  chain of `String::replace`. With a slot fed by untrusted source content
  (invariant 7), a replace chain lets a heading that reads `{{TRANSYNC_…}}` be
  re-substituted by a later slot's value; scanning once and never rescanning a
  substituted value makes that structural rather than an ordering the next edit
  can revoke. The title is HTML-escaped by the same escaper the language labels
  use, which is a superset of what a `<title>` text node needs.

### Amendment (2026-08-08) — the render seam is fallible

*Appended, not a rewrite. Every decision above stands: two fragment-shaped
pane files plus a templated shell, the renderer never templates the shell,
the seam stays out of the curated facade (`contracts.md` §0 tier (c)). One
statement in the "Implementation seams" list — and its restatement in the
2026-08-04 amendment, "`render_source` / `render_target` keep the signatures"
— is now stale: both return a `Result`.*

Review 0002 (R0002-0010, R0002-0011) found the renderer treating an
alignment map as trustworthy in two ways that failed silently:

- **A repeated `source_block_id`** decided an id's attributes and byte ranges
  by whichever row the `HashMap` happened to index last.
- **A block with no row** was skipped entirely — no anchor, no placeholder,
  no warning — so an incomplete pane was indistinguishable from a complete
  one, against §4a's per-row anchor-survival guarantee.

Both are producer defects (`align::build_alignment_map` emits exactly one
row per block, in document order), and both were reachable only through a
caller-supplied map — `transync-wasm`'s view mode. Since transync has never
shipped to a consumer, the honest fix was taken over the compatible one:

- `render_source(doc, alignment) -> Result<String, RenderError>`
- `render_target(doc, translated_md, alignment) -> Result<String, RenderError>`

`RenderError` is `transync_syntax::render`'s, `#[non_exhaustive]`, and names
the offending ids (bounded). Rows naming blocks the document does not have
are NOT refused — they are inert. Byte-range *sanity* is still the caller's
to keep: `block_text` continues to clamp and snap to char boundaries, the
panic-free posture recorded in that function.

The seam is not in the facade, so `contracts.md` §0 and the
`public_surface.rs` weld are untouched — `render` is on that test's
`forbidden` list and stays there. Callers thread the error: the pipeline maps
it to `TransyncError::Internal` (its own map builder is the only producer, so
a refusal there is an internal fault), and `transync-wasm` grew
`EngineError::Render`.

A third change rides the same signature (R0002-0059): both panes' Markdown
now reaches Comrak through `parser::intake` — the nesting-depth guard plus
NUL normalization `parser::parse` applies — instead of the target pane
calling `comrak::parse_document` directly. A document the library refuses as
a source is no longer accepted as a target. Only the parse consumes the
normalized string; the bypass and degrade arms still slice the caller's own
bytes, which is what the byte ranges index.

### Amendment (2026-08-08) — the refusal reaches range *values*, not just map shape

*Appended, not a rewrite. Everything above stands, including the whole
2026-08-08 amendment; one sentence of it is superseded and is marked here
rather than edited there: "Byte-range **sanity** is still the caller's to
keep: `block_text` continues to clamp and snap to char boundaries." It no
longer does either.*

Review 0003 (R0003-0060, and R0003-0078 as its browser-side shadow) found
the residual the 0002 fix deliberately stopped short of. That fix drew the
refuse-vs-degrade line at map **shape** — a repeated row, a missing row —
and left range **values** to `block_text`, which clamped them into the
pane's length, reordered a reversed pair, and snapped both ends to UTF-8
boundaries. That kept the renderer panic-free, which was the goal, but it
also meant a row pointing at the wrong bytes still rendered: an empty block
for a reversed range, a truncated one for an out-of-bounds range, a block
missing the character it split for a mid-character range — each under the
row's own correct anchor, with its `fallback_status` unchanged and no signal
anywhere. A coercion that keeps rendering is not a guard; it is a silent
wrong answer with a defensive shape.

The values are now checked once per pane, in `PaneCtx::new`, beside the two
0002 refusals, and the renderer refuses with `RenderError::UnusableRange {
pane, ids }` — naming which pane's Markdown the ranges were measured against
and, per offending row, the offsets and the fault, bounded by the same
`MAX_NAMED_IDS` the other refusals use. Each pane checks only what it reads:
`Block::source_range` against `doc.source_text` for the source pane,
`AlignmentBlock::target_range` against the handed `translated_md` for the
target pane. Every block is checked, not only the kinds that read raw bytes
today, because Guard 2 can send any row down the byte-reading path.

Three properties of the check are decisions rather than details:

- **An empty in-bounds range is not a fault.** `build_alignment_map` emits
  `0..0` for a block its `BlockOffsets` did not cover, and warns about it
  (R0003-0055). That map still renders.
- **Inert rows stay inert.** Rows naming blocks the document does not have
  are still accepted and still never looked up, so their ranges are never
  measured either.
- **`block_text` slices instead of coercing.** It borrows `source.get(..)`
  behind a `debug_assert`, so the only residue of the old posture is a
  wrong-but-not-panicking answer for a future caller that builds a `PaneCtx`
  some other way. `regen::regenerate` keeps `clamped_char_bounds`
  (R0003-0059): it is infallible by design and has no error channel to
  refuse through.

R0003-0078 closes with it. `web/js/wasm-demo.js` checks `target_range`
values only for the rows it lets you edit, because its own blast radius is
the edit model (R0002-0054) — but html and skipped rows' ranges are read by
the renderer's bypass arms, and nothing was checking those. The renderer now
does, for every row it reads, so the demo needs no second gate; its gate
stays where it is and keeps owning the earlier, edit-model-specific message.

The seam is still not in the facade: `render` is `contracts.md` §0 tier (c)
and on `public_surface.rs`'s `forbidden` list, so the §0 table and the weld
are untouched. `RenderError` is `#[non_exhaustive]`, so the new variant is
additive for every caller that matches on it; the pipeline's blanket mapping
to `TransyncError::Internal` and `transync-wasm`'s `EngineError::Render`
carry it unchanged.

### Amendment (2026-08-09) — the "external static server" mitigation is now transync's own

*Appended, not a rewrite. The decision above stands whole: six files, two
fragment-shaped pane files plus a templated shell, committed as one atomic
set.*

The **Bad: six files instead of one** consequence names its mitigation as
"they are served by an external static server (e.g. `python3 -m
http.server`), since `transync serve` is a deferred placeholder
(STUB-061)". Half of that sentence has expired. `transync serve` stopped
being a placeholder on 2026-08-09 (ticket `b791d6`): it binds
`127.0.0.1:7470` and serves the `--rendered` directory, path-confined to
it, with a content-type table that covers exactly this bundle's file set.

The consequence itself is unchanged and, if anything, cheaper: six files
are still six files, but the server that turns them back into one page is
now the same binary that wrote them, so "bring your own static server" is
no longer a step a reader has to supply. An external server still works —
the bundle is self-contained, which is the property that made the
mitigation valid in the first place and is what the whole rejected
**Three-file no-shell** option was measured against. `contracts.md` §6
carries the server's contract; `docs/project/stub-manifest.md` records the
STUB-061 closure.

### Amendment (2026-08-12) — "one atomic set" is a staging guarantee, not a rollback promise

*Appended, not a rewrite. The decision above stands whole: six files, two
fragment-shaped pane files plus a templated shell the renderer never writes.
What is corrected is one word this ADR leans on — "atomic" — as it appears in
the Status line ("emitted atomically by `transync-cli::output`") and in the
**Bad: six files instead of one** consequence ("the CLI ships them together as
one atomic set"). Both read as a promise of transactional replacement, which
the files mode does not make and never did (Review 0004 `R0004-0100`).*

`output::write_fileset_atomic` is a **staged fileset commit**, and it is
atomic in one direction only:

- **Phase 1** stages every payload as `<path>.tmp.<pid>` and fsyncs it. A
  failure here — the phase that does the real I/O — removes every staged temp
  and leaves every target untouched. That much this ADR's consequence line
  claims correctly, and it is what "one file = one rename; the shell never
  partially overlays a previous run" was reaching for.
- **Phase 2** renames each staged file over its target. A crash or I/O error
  partway through this pass **can leave a mixed set** — some outputs from this
  run beside some from the previous one. Nothing rolls that back: the run says
  so on stderr and exits `4`. What the staging buys is a smaller exposure, not
  its absence — "seconds of content writing" shrink to a few metadata renames
  — and the publish lock over every destination directory (DCR-0021) confines
  the window to a crash/IO-error one rather than a concurrency one, because a
  *second run* cannot interleave its renames with this one.

`contracts.md` §6 states both of those precisely (exit code `4`, and
"Publication locking"), and it is the authority; this ADR's prose is a summary
of it and should be read that way wherever the two are not word-for-word.

Set-level atomicity — the property where no reader ever sees a mixed set — is
a **different publication mode that already ships**, not missing work.
`--out-dir` stages the whole tree in a sibling directory and renames it into
place: onto a fresh target that is one atomic rename, and replacing an
existing target is crash-safe but *not* atomic (the old tree is moved aside
first, so a reader between the two renames finds no target at all).
`contracts.md` §6 owns both descriptions. Nothing here asks for the files mode
to grow a transaction; the choice between the two exposures is the one the two
flags already offer, and this amendment exists so the ADR stops implying the
files mode already made it.

(The **Bad** line's other half — "since `transync serve` is a deferred
placeholder (STUB-061)" — expired separately and is corrected by the
2026-08-09 amendment directly above.)

## ADR-0007 — Renderer wraps `<table>` and `<pre>` in a transparent `<div>`

### Context and Problem Statement (decision-body wording before consolidation)

Found in Review 0001 (Issue R0001-0024, Severity: High) (review archived and removed).
Location: `crates/transync-core/src/render.rs::wrapper_element_for`
(now `crates/transync-syntax/src/render.rs` — see the 2026-08-04 amendment
at the end of this record)

`docs/architecture/contracts.md` §4 (HTML attributes) specifies that the
sync-id attributes (`data-sync-id`, `data-block-kind`, `data-order`,
`data-fallback`, `data-parent-id`) live on the *semantic outer element*
of each block. For tables that means `<table data-sync-id="t-0001">…`;
for code blocks that means `<pre data-sync-id="c-0001"><code>…`.

The shipping renderer instead wraps tables and code blocks in a
transparent `<div data-sync-id="t-0001"><table>…</table></div>` /
`<div data-sync-id="c-0001"><pre><code>…</code></pre></div>`. The
inline comment on `wrapper_element_for` documents the choice:

> For block kinds whose Comrak HTML already includes the semantic
> element (`<table>`, `<pre>`, `<ul><li>...`, `<blockquote>`), we use
> a transparent `<div>` so the sync-id wrapper does not nest a second
> copy of the same element.

This is a real conflict between the contract and the implementation.

### Implementation (decision-body wording before consolidation)

No code change required — the renderer behavior is being formalized,
not modified. The action items are documentation:
- Update `contracts.md` §4 to describe the wrapper-div pattern as the
  intended shape.
- Cross-reference this ADR + DCR-0001.
- Keep the inline comment on `wrapper_element_for` as the
  authoritative description of the choice.

### Consequences, second bullet (decision-body wording before consolidation)

* Good, because the implementation can use Comrak's HTML output
  unmodified — no AST surgery required for tables and code blocks.

### Amendment 2026-08-04 (DCR-0017) — mechanism, not decision

*Appended, not a rewrite. The decision stands unchanged: sync-id attributes
live on a transparent `<div>` wrapping Comrak's own `<table>` / `<pre>` /
`<blockquote>`, and `contracts.md` §4 continues to document that as the
intended shape.*

Two mechanism statements above are now stale:

- **Location.** `wrapper_element_for` lives in
  `crates/transync-syntax/src/render.rs` (the renderer moved to the new
  wasm32-compilable base crate). Its inline comment remains the authoritative
  description of the wrapper choice, per the *Implementation* section's third
  action item.
- **"No AST surgery required" is now satisfied differently — and better.**
  The 2026-07 renderer avoided AST surgery by reparsing each block as an
  isolated fragment and string-stripping Comrak's outer element
  (`strip_outer_wrapper`). That mechanism is **deleted**. The renderer now
  parses each pane once and hands the paired AST node straight to
  `comrak::format_html`: for table / code block / blockquote it formats the
  **whole node**, so Comrak still emits its own `<table>` / `<pre><code>` /
  `<blockquote>` inside the transparent `<div>` — identical output, reached by
  reading the AST instead of by rewriting Comrak's HTML.

The *Decision Drivers* bullet "Injecting attributes into Comrak's emitted
outer element requires either AST-level node manipulation … or post-process
string surgery (brittle)" is unchanged as a rejection of option 2 — the
renderer still injects nothing into Comrak's element. What changed is that the
alternative it chose is no longer string surgery either, which retires the
R0001-0096 brittleness that OI-0008 tracked.

## ADR-0009 — Reject changes to the bounded retry policy

Source: `docs/decisions/0009-reject-retry-policy-changes.md` (ADR-0009). Its six
amendment sections moved here verbatim when the decision body absorbed them
(DCR-0054); the ADR's `## History` list carries one line per event. The last
section keeps, verbatim, the decision-body lines that absorption rewrote.

### Amendment (2026-07)

The anticipated non-content retry channel shipped as `TranslationUnit.retry`
(`RetryContext { attempt, rejected_by, reason }`) — see DCR-0009.
Payload verbatim-resubmission and the bounded budget are unchanged;
only the side channel was added, exactly as Considered-Option 3 anticipated.
The `transync-openai` client serializes it as a data-framed `retry` object in
the user-prompt unit (never into `source_payload`) and extends the
injection-guard framing to the source-derived `reason` field.

### Amendment (2026-08-03)

A **second** bounded retry budget is added *beside* the per-unit one, for
batch-envelope faults only — see DCR-0014 (OI-0031). Nothing in this ADR's
decision is reversed:

- `max_per_unit_validation_retries` keeps its meaning, its default (2), and
  its arithmetic (total content attempts per unit = retries + 1). Resubmission
  is still **verbatim** — same payload, same scope — with guidance confined to
  the non-content `RetryContext` side channel.
- What changed is *attribution*, not policy. A batch-level schema fault (a
  requested unit's result row dropped, or returned more than once) is no longer
  charged to every batch-mate's per-unit budget. Batch-mates whose rows arrived
  intact are validated and accepted in that same round; the implicated units
  are re-dispatched verbatim on the new
  `TranslateOptions.max_per_batch_schema_retries` (default 2, charged once per
  round, library-configurable in the same spirit as the per-unit budget), and
  finalize as honest `fallback_source` once it is spent.
- Rationale for a separate budget rather than a shared one: a dropped unit's
  *content* was never judged, so spending its content budget on the provider's
  formatting failure would leave a twice-dropped unit with fewer real
  translation attempts than a merely-mistranslated one — the opposite of the
  bounded-fairness this ADR defends.
- Retries remain provably bounded: total rounds per input batch are
  `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`.

ADR-0017 is untouched: schema faults are validation-layer events with a
fallback path, not provider/transport-terminal errors.

### Amendment (2026-08-05) — the transient budget's scope is per batch

The third bounded budget, `max_per_batch_provider_retries` (transient
`Network` / `RateLimited` transport failures only), was **charged per dispatch
round** rather than per batch: `process_one_batch` calls the transport-retry
loop once per round of the ladder above, and the round boundary reset the
counter. A batch that walked its whole ladder could therefore spend up to
`rounds × max_per_batch_provider_retries` transient retries, while the knob's
name and its rustdoc both promised per batch (`contracts.md` §5's pseudocode
was the one document that matched the code, by restarting its `attempt`
counter at each validation-retry round). Ticket `294dda` fixed the code to
match the name: the counter now carries across dispatch rounds, so one batch
spends at most `max_per_batch_provider_retries` transient retries in total,
and the backoff ordinal keeps climbing with it. §5's table and pseudocode were
updated to state the per-batch scope explicitly.

Nothing else in this ADR moves. The transient budget is not a content budget:
it is charged only for errors that never produced a payload, so it neither
touches `max_per_unit_validation_retries` nor participates in the verbatim
resubmission rule, and it has no fallback rung — ADR-0017 still owns what
happens when it is spent (whole-run abort, not `fallback_source`). The round
bound `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
is unchanged; what changed is only how many *provider calls* one round may
contain across a batch's lifetime, which the bound never counted.

Consequence, stated plainly: a run against a provider that is both flaky and
structurally sloppy now aborts earlier than it did before 2026-08-05. That is
the documented contract taking effect, and the escape hatch is the same
caller-configurable knob (default 1).

### Amendment (2026-08-06) — a retry round is packed, and the payload still is not

Review-0001 `R0001-0012` observed that the retry side channel this ADR
authorized was never *budgeted*: `pipeline::dispatch` rebuilt the next round
straight from its `retry_units` vector, so a retry round shipped as one batch
however large — even though every re-dispatched unit carries a `RetryContext`
whose `reason` runs to 512 bytes that the original token packing never counted.
Ticket `24fd28` runs retry units through `batch::group_by_token_budget` under
the same budget the first round used, and teaches the estimator to encode the
`retry` hint.

**Nothing in this ADR's decision moves, and the verbatim rule is preserved
exactly.** The distinction is between the *payload* and the *request boundary*
around it:

- Resubmission is still verbatim — same `source_payload`, same scope, the
  pristine original with `retry: None` re-fetched from the request map, and
  guidance confined to the non-content `RetryContext`. `retry_validation_unit`
  is untouched. Re-packing only decides **which request** a unit rides in; it
  never rewrites, merges, splits, or re-scopes one, and the tests assert the
  re-dispatched payloads are byte-identical to the first round's.
- Every budget keeps its meaning and its arithmetic. `max_per_unit_validation_retries`
  and `max_per_batch_schema_retries` are charged **per round**, and a round is a
  pass over the outstanding population however many batches it is divided into:
  the batch-fault admission runs once over the round's union of offenders, so a
  split round costs exactly what an unsplit one would. The per-batch transient
  budget still spans the whole ladder (2026-08-05 amendment).
- The round bound `≤ 1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
  is unchanged. As in the 2026-08-05 amendment, what changed is only how many
  *provider calls* one round may contain — a quantity the bound never counted.

Consequence, stated plainly: a retry round that used to be one over-budget
request is now several in-budget ones. That is more requests for the same work,
which is the same asymmetry `batch.rs` already documents for the output
expansion factor — overestimating costs linear request overhead, underestimating
aborts the run at the provider.

### Amendment (2026-08-06) — the budget that packs both rounds now prices the instruction

Ticket `aa92d64f`, a follow-up to the amendment above. The 2026-08-06 rule that
**the same budget packs every round** is unchanged; what changed is what that
budget reserves. The per-batch envelope used to open with a flat 256-token
allowance for the constant user-message instruction; the instruction is longer
than that, so both packers believed they had room they did not have. It is now
encoded from the string `llm::prompt` assembles.

One consequence lands on this ADR's territory, and is settled here rather than
left implicit:

- The instruction's raw-HTML segment clause rides only on a batch that holds a
  raw-HTML unit, so its cost depends on batch membership — which is what packing
  decides. The owner's resolution (2026-08-06) is a **document-level** upper
  bound: reserve the clause whenever the *document* holds at least one html unit.
- `pipeline::dispatch` can see one batch, not the document. Deriving the verdict
  there would give an html-free batch of an html-bearing document a *smaller*
  reserve than the round that packed it, and "the same budget packs every round"
  would quietly stop being true. So the verdict is computed once by
  `run_pipeline`, where the whole population is still in hand, and handed to
  every batch. Both packers price the same instruction, byte for byte.

Nothing about the payload moves, again: resubmission stays verbatim, guidance
stays confined to the non-content `RetryContext`, and every budget keeps its
meaning and arithmetic. The reserve is larger than it was, so a round packed to
its budget may divide into slightly more requests than before — the same
overestimate-is-cheap asymmetry the previous amendment states.

### Amendment (2026-08-09) — a packing-time split is not a retry, and windows are units

DCR-0026 commissions the oversize table row-window splitter (ticket `fc0304`,
STUB-017 cluster). Because "a split changes the scope" sounds adjacent to the
resubmission rule this ADR defends, the boundary is drawn here explicitly:

- **The split happens before round one, once, deterministically.** It is an act
  of the same kind as `group_by_token_budget` deciding batch boundaries —
  packing, not retrying. It consumes no retry budget and touches no in-flight
  unit.
- **A window is an ordinary unit from birth.** Its payload and scope are fixed
  for the whole run. A validation failure re-dispatches *that window* verbatim
  — same payload, same scope, guidance confined to the non-content
  `RetryContext` — charged to `max_per_unit_validation_retries` per window.
  The batch-fault and transient budgets apply to windows unchanged, and the
  round bound `1 + max_per_batch_schema_retries + U × max_per_unit_validation_retries`
  holds with `U` counting windows.
- **No re-split exists.** Nothing — no failure, no provider signal, no retry
  round — changes any unit's scope after packing. The verbatim-resubmission
  rule is therefore untouched rather than amended; this note exists so the next
  reviewer does not have to re-derive that.

The relationship to ADR-0017 is unchanged as well: a window that cannot fit
the ceiling even alone is flagged by the preflight and its terminal provider
error aborts the run — the split adds prevention ahead of this ADR's machinery,
never a rung inside it.

### Considered Options and Decision Outcome — lines as written before absorption

Option 3 of *Considered Options*, then the *Decision Outcome* status line and its
*Implementation* subsection, as they stood in the ADR before the amendments
above were absorbed into the body:

```text
3. Keep the bounded uniform retry; build the retry-hint side channel as a
   future feature if retry efficacy becomes a measured problem.

Status: No change required.

### Implementation

None. The retry-hint side channel remains a candidate future feature.
```

## ADR-0011 — Keep the LAN bind in the convenience wrappers

Source: `docs/decisions/0011-lan-bind-in-convenience-wrappers.md`. Moved verbatim under DCR-0054
(docs compaction, wave 5); the ADR's decision body states the current rule.

### Consequences — as written for the Review 0008 gate (R0008-0010, 2026-07-11)

* Good, because the owner's cross-machine demo workflow keeps working
  with zero flags.
* Bad, because anyone copying the wrappers verbatim onto an untrusted
  network exposes their translation output; the wrappers' comments and
  this ADR carry that warning.

### Amendment (2026-08-16) — the bind is no longer sufficient on its own

The decision stands; one consequence above has expired. Since the `Host`
check landed (R0004-0002, 2026-08-12), a bind decides which *network* can
open the socket and no longer decides which authorities the server
answers for. `serve_cmd::host::HostPolicy::new` derives nothing from a
wildcard bind but the loopback authorities it is also listening on —
`127.0.0.1`, `[::1]`, `localhost` — so a browser on the second machine
typing the dev box's LAN address sends a `Host` naming that address,
gets `Verdict::Elsewhere`, and is answered with a `421`.

So "keeps working with zero flags" now reads: keeps working with zero
flags **from this machine**, at `localhost`. Cross-machine browsing
additionally needs the authority the other browser types, named through
`TRANSYNC_LIVE_ALLOW_HOST=<host:port>`, which `scripts/smoke-live.sh`
forwards as `--allow-host`. The wrappers cannot set it themselves — the
address is the operator's to state — so their comments say so instead.

The "bad" consequence is correspondingly narrower: a wrapper copied onto
an untrusted network still publishes the socket, but a page reaching it
under a name the policy does not answer for is refused before the served
root is consulted.

## ADR-0012 — Inline content is LLM-owned; inline-structure constraints are advisory

Moved verbatim from `docs/decisions/0012-inline-content-llm-owned-advisory-constraints.md`
(git blob `093341fc805ef1541259462f0ab4f6b1ccbbc29c`) when its three amendment sections were
folded into the decision body (DCR-0054, compaction wave 5). This is history, never current
truth: the ADR's own decision body is the rule. The first three sections below are the body
passages that the rewrite replaced, in their original wording; the last three are the amendment
sections in the order the ADR carried them, with only each heading's level changed (`##` → `###`).

### Decision Outcome — wording before the amendments were folded in (Review 0008 disposition of R0001-0046 / R0008-0011 / R0008-0016)

REJECT (all three findings): option 3. Inline validation contradicts
invariant 2 for translatable text, and the fragment/full reparse layers
already catch inline damage severe enough to break block structure.
`forbid_block_breaks_in_inline` remains a documented advisory hook
(`R0001-0046` trace comments at the constraint and hint sites).

Status: No change required. Raised three times across two independent
reviews. This decision rests on the assumption that scroll-sync and
regeneration depend only on the *outer* block structure, never on inline
markup. Revisit trigger: if the sync/regen contract ever starts depending on
inline structure (e.g. an anchor scheme keyed to inline spans), the
inline-fingerprint option is back on the table.

### Implementation — wording before the amendments were folded in (Review 0008 disposition of R0001-0046 / R0008-0011 / R0008-0016)

None. The hint plumbing (R0006-0036) ships; the validator hook stays
open for a future opt-in profile policy.

### Consequences — wording before the amendments were folded in (Review 0008 disposition of R0001-0046 / R0008-0011 / R0008-0016)

* Good, because correct translations are never rejected for legitimate
  inline reshaping, and the model still receives the structural hints.
* Bad, because a misbehaving provider can alter link destinations or
  inject inline content within a block without tripping validation —
  bounded by the outer-structure checks and DOMPurify at render time.

### Amendment (2026-07, external review P1-5): destinations and pledged code spans are protected structure

The rejection above treated *all* inline markup as LLM-owned. This
amendment narrows it: link and image **destinations**, and — when the
profile pledges `preserve_code_identifiers = true` — inline **code
spans**, are reclassified as protected structure, enforced by the
`Inline` validation layer (`validate/inline.rs`) after fragment
reparse. Destinations are operational metadata, not prose: a swapped
URL is invisible in the rendered pane, navigation breaks silently, and
DOMPurify does not defend against a phishing destination. The original
"bounded by outer-structure checks" consequence was wrong for exactly
this surface, and the check is cheap — both sides reuse the comrak
parse the validators already run.

Policy gates (profile `[constraints]`):

* `preserve_urls` unset or `true` → destination identity enforced
  (ordered, kind-tagged link/image sequence; autolinks included,
  autolink ↔ explicit-link form changes with the same destination
  pass). `false` → the shipped "URLs may be localized" policy; no
  destination check.
* `preserve_code_identifiers = true` → code-span identity enforced as
  an unordered multiset (spans legitimately move with target-language
  word order). Unset or `false` → advisory only, as before.

Mismatches — including added/dropped links — are retryable validation
rejections (`ValidationLayer::Inline`) following the standard
verbatim-retry → fallback-source path (ADR-0009, invariant 6). There
is no repair pass: the pipeline never rewrites provider output.

What did **not** change: inline TEXT remains LLM-owned. Link text,
image alt text, link titles, emphasis boundaries, and table-cell prose
are never fingerprinted; `forbid_block_breaks_in_inline` remains an
advisory hint; reference-link labels are not protected (fragment
parses resolve no refmap — a symmetric no-op, documented gap). *(This
reference-label gap was closed 2026-07-27 — see the DR-2026-07 amendment
below.)* The original rejection rationale above stands as history for those
surfaces.

### Amendment (2026-07-27, DR-2026-07): reference destinations protected; two residuals recorded

The 2026-07 design review (design-review wave DR-2026-07) revisited the inline
surface and produced one narrowing plus two explicitly-accepted residuals.

**Reference-style destinations are now protected (DCR-0013).** The previous
amendment's documented gap — "reference-link labels are not protected
(fragment parses resolve no refmap)" — is **closed**. A document-level
link-reference-definition pool is now recovered from the inter-block gaps at
parse time (`parser/refdefs.rs` → `Document.ref_defs`) and appended to each
fragment before both the render reparse and the inline-inventory parse. A
`[text][ref]` link therefore resolves to a real destination on both sides, so
a provider that mistranslates a reference **label** changes the resolved
destination inventory and trips the same `ValidationLayer::Inline` rejection as
an explicit-link destination change. Reference-link **text** remains LLM-owned,
exactly like explicit-link text. See DCR-0013 for the mechanism, including the
narrow render-only duplicate-label ordering limitation (validation stays
symmetric, so no false accept/reject).

**Accepted residuals (within the LLM-owns-content doctrine).** Two inline-level
exposures are acknowledged and accepted, not fixed — both follow directly from
invariant 2:

1. **Same-kind payload transposition is structurally undetectable.** If a
   provider swaps the translated payloads of two units of the *same* block kind
   (paragraph A's translation returned under paragraph B's id and vice-versa),
   every structural and inline check still passes on each unit in isolation —
   the ID↔content binding ultimately rests on **provider honesty**, not on any
   validator. The MUST-preserve-`unit_id` contract (§1) and per-unit validation
   catch id corruption and shape drift, but not an honest-looking same-kind
   content swap.
2. **Math / LaTeX content is unprotected.** Inline and block math is opaque
   paragraph text to the pipeline (math extensions are off); it is neither
   fingerprinted nor destination-checked, so a provider may alter a formula as
   freely as prose. This is the same posture as inline text generally.

Both residuals sit inside the "LLM owns content decisions" doctrine and are
recorded for honesty, not slated for a fix at this scope.

### Amendment (2026-08-04, ADR-0018 / DCR-0016): raw inline-HTML tags are protected structure, unconditionally

Raw inline HTML (`<kbd>`, `<br>`, `<sup>`, `<b>`, …) was covered by "other
inline markup belongs to the model". It no longer is. The ordered sequence of
comrak `HtmlInline` literals in a paragraph-family payload must survive
**byte-verbatim**, in order; only the text around the tags is translatable. A
mismatch is a retryable `ValidationLayer::Inline` rejection on the standard
verbatim-retry → fallback path, exactly like a destination change.

Two things distinguish this from the amendments above:

* **No policy gate.** Unlike `preserve_urls` / `preserve_code_identifiers`, the
  guard is **always on** and is deliberately *hoisted above* `check_inline`'s
  policy early-return, so a profile with `preserve_urls = false` and no code
  pledge cannot switch it off. The rationale is that a tag is not a content
  decision the profile could reasonably delegate: a model has no legitimate
  reason to alter markup it was told to reproduce, and the HTML-block feature
  (ADR-0018) made raw markup a first-class structural concern rather than an
  incidental one. The user prompt carries the matching instruction on **every**
  dispatch, and both halves live in `transync::llm::prompt` so they cannot
  drift apart.
* **An accepted known false-reject.** A model that legally moves a **type-6**
  tag such as `<div>` to line-start makes the fragment reparse reclassify the
  paragraph remainder as an HTML block, so the `HtmlInline` tokens vanish on the
  translated side only and the guard rejects. Verbatim retry usually recovers;
  the behavior is pinned by test. Type-7 tags (`<b>`, `<kbd>`, …) cannot
  interrupt a paragraph (comrak-verified, also pinned), so the exposure is
  confined to the type-6 set. Model normalization of `<br>` ⇄ `<br/>` is a live
  rejection source to watch — the comparison is byte-verbatim by design.

What still did **not** change: inline **text**, including the text between and
around tags, remains LLM-owned; attribute text inside inline tags is preserved
only as a side effect of the tag being reproduced verbatim, and is never
translated (ADR-0018). Both residuals recorded in the DR-2026-07 amendment
above stand.

## ADR-0013 — Language labels are opaque caller-supplied strings

From `docs/decisions/0013-opaque-language-labels.md`, where these five sections followed *Consequences*. Moved verbatim; only each heading's level changed (`##` → `###`).

### Amendment (2026-08-03) — presentation-layer direction hint

RTL pane direction shipped in the OI-2026-08 wave (DCR-0015, OI-0032). This
ADR is **amended, not violated**: labels remain fully opaque for every *engine*
consumer — the system prompt, the cache key, the alignment map, and every
validation layer still forward them verbatim, unparsed, with only the
non-emptiness check.

What is added is confined to the **presentation layer** — the CLI's HTML
bundle emitter, which is already the one component whose job is rendering:

- When direction resolves to `auto` (the default), the emitter applies a
  best-effort **hint**: the label's first `-`/`_`-separated token,
  ASCII-lowercased, is matched against a 15-entry RTL primary-subtag table
  (`ar arc ckb dv fa he iw ji nqo ps sd syr ug ur yi`); a match stamps
  ` dir="rtl"` on the pane, everything else emits nothing (LTR is the HTML
  default).
- The hint is **explicitly override-able** and the override is the
  authoritative path: `--target-direction {rtl|ltr|auto}` beats the profile's
  `[render].target_direction`, which beats `auto`.
- The hint is **best-effort by design and documented as such.** An expressive
  label — exactly the kind this ADR exists to protect — is not tag-shaped, so
  it resolves LTR: `"Korean (formal, 존댓말)"` → subtag `"korean"` → no match;
  `"العربية"` → no match. That is a *documented limitation*, pinned by the test
  `expressive_labels_stay_ltr`, not a parsing attempt. A caller who wants RTL
  for an expressive label uses the flag or the profile key.
- The hint is **invisible to identity**: it never enters the prompt, the cache
  key, the alignment map (schema stays `1.1.0`), or `out.md`. Two runs
  differing only in pane direction are cache-identical.

Revisit trigger for this amendment: if direction ever needs to reach the
alignment map or the prompt, the opacity contract must be re-examined first —
that would make the label load-bearing for the engine, which this ADR
forbids.

### Amendment (2026-08-06) — the batch budget measures the label instead of assuming a size

Review-0001 `R0001-0013` pointed out a consequence of this ADR that the batch
packer had not absorbed: `batch::FIXED_ENVELOPE_TOKENS` (256) stood in for the
constant user-message instruction **and** the two language labels. An allowance
is a size assumption, and this ADR is precisely the decision that labels have no
bounded size — `"Middle High German as written in the Rhine Franconian scribal
tradition"` is exactly the expressive label this ADR exists to protect, and it
alone outweighs the whole allowance.

Ticket `24fd28` encodes `source_language` and `target_language` into the
per-batch envelope reserve; the constant now covers only the instruction it was
tuned for. This is **not** a parsing attempt and does not make the label
load-bearing for the engine: the labels are still forwarded verbatim and
unparsed to every consumer this ADR names. Counting a string's tokens is the
same operation the packer already performs on the system prompt and on every
unit's payload — it reads the bytes, it does not interpret them.

Consequence: a caller who passes a very long label now gets more, smaller
batches rather than requests that quietly exceed the token target. Ordinary
labels (`en`, `ko`, `한국어`) cost a token or two, so packing shape is
effectively unchanged for them.

### Amendment (2026-08-06) — surrounding whitespace comes off once, at the CLI boundary

Review-0001 `R0001-0021`. This ADR reserves exactly one literal: `auto`, the
source-language sentinel that compiles to *"the auto-detected source language"*
instead of being substituted. A reserved literal has to be recognizable, and
`--source-language ' auto '` was not recognized: the CLI validated
`trim().is_empty()` and then forwarded the untrimmed value, so the padded label
compiled a prompt naming ` auto ` verbatim, keyed its own cache entries, and was
stamped on the alignment map — while the HTML bundle's pane label, which had a
`trim()` of its own, treated the same run as auto-detected. One run, two
answers about whether the sentinel had been typed.

Ticket `ed8c57` trims *surrounding* whitespace once, where the labels are
validated (`translate_cmd::args::resolve_language_and_model_args`), and every
CLI consumer reads that one normalized value. The library's sentinel test
(`profile::render_prompt_body`) tolerates padding and case as well, so a caller
that never crosses the CLI gets the same answer.

This is **not** validation and does not narrow what counts as a label:

- Nothing in a label's interior is touched, no case is folded, and no tag is
  parsed. `"Korean (formal, 존댓말)"` survives byte-for-byte, spaces included.
- Non-emptiness is still the only rule (R0008-0039); a whitespace-only label is
  blank, which it already was under the old `trim().is_empty()` check.
- `--model` is deliberately **not** normalized: it is a provider identifier,
  not a label this ADR governs.
- The library layer still substitutes a non-sentinel label verbatim. Padding is
  removed by the *argument boundary*, which is the layer that knows a value was
  typed on a command line; `TranslateOptions::source_language` remains opaque.

Consequence: a padded run is now indistinguishable from a bare one — same
compiled prompt, same cache identity, same alignment metadata. A caller who
genuinely wants a label with a leading space cannot express it through the CLI;
that is the intended trade, since such a label differs from its trimmed twin in
no way anyone can see except cache identity.

### Amendment (2026-08-06) — the constant the labels were carved out of is gone entirely

The 2026-08-06 amendment above ended with `batch::FIXED_ENVELOPE_TOKENS` covering
"only the instruction it was tuned for". Ticket `aa92d64f` found that it never
covered even that: the assembled instruction outgrew 256 tokens on its own, and
the constant no longer exists. The per-batch reserve is now four encoded strings
— the instruction envelope, the compiled system prompt, and the two labels —
with no fixed allowance left in it.

This ADR's rule is unchanged and is now applied uniformly: **the packer measures
what it will send and assumes nothing about the size of anything.** The labels
keep the treatment R0001-0013 gave them, and they are still forwarded verbatim
and unparsed to every consumer this ADR names.

### Amendment (2026-08-07) — the reserved literal has one spelling at the CLI boundary

Ticket `fd5aa8`. The 2026-08-06 whitespace amendment made the reserved literal
recognizable through padding; **case** was the half it left. Both sentinel tests
in the tree folded ASCII case — `profile::render_prompt_body` and
`translate_cmd::publish::pane_source_language` — while
`translate_cmd::args::resolve_language_and_model_args` only trimmed. So
`--source-language AUTO` compiled *"the auto-detected source language"* and
rendered the bundle's source pane from the model's *detected* label, yet
`CacheKey::source_lang`, the user-message payload's `source_language` and the
alignment map's `source_language` all carried the literal `AUTO`. One request,
two identities: the prompt and the pane said "detection was requested", the key
and the metadata said "a label spelled `AUTO`". That is the R0001-0021 split
again, one notch weaker — the two answers no longer contradict each other, they
just fail to be the same answer.

The CLI now stores a recognized sentinel in its **canonical spelling**, at the
same place and for the same reason it trims:

- **One recognizer, two callers.** `args::is_source_language_sentinel` is the
  only place the CLI decides whether a value is the sentinel; the argument
  boundary canonicalizes with it and the bundle emitter's
  `pane_source_language` asks it. Two copies of the question were what let the
  answers diverge, so there is now one.
- **Only the reserved literal is folded.** A value the recognizer rejects is
  stored byte-for-byte, case included — `DE` stays `DE`, `Auto-detect` stays
  `Auto-detect`, `"Korean (formal, 존댓말)"` stays intact. This is not case
  folding of *labels*; it is a reserved word being spelled the one way it is
  defined.
- **The target side is untouched.** No literal is reserved there, so
  `--target-language AUTO` is an ordinary label.
- **The library layer is deliberately unchanged**, on the same layering the
  padding amendment stated: `TranslateOptions::source_language` stays opaque,
  and only its sentinel *test* tolerates padding and case. A caller that
  bypasses the CLI with `"AUTO"` therefore compiles the detection prompt but
  keys and stamps `AUTO` — pinned by
  `scn_11_the_library_keeps_the_sentinels_spelling`, so the asymmetry is a
  recorded decision rather than an oversight.

Consequence for the cache, stated plainly: **a run spelled `AUTO` (or `Auto`)
re-keys, once.** Entries it wrote under the old literal are still in the cache
and are never looked up again; from now on it shares identity with the `auto`
runs. Sharing is correct and not merely convenient, because the canonical label
is also what goes on the wire: two runs that now share a key send byte-identical
requests. Nothing about a non-sentinel label's identity moves.

### Implementation — as written before the amendments were folded in (R0008-0039)

None beyond the shipped non-emptiness checks (R0008-0039).

## ADR-0015 — Cache poison & invalid-hit policy

Source: `docs/decisions/0015-cache-poison-and-invalid-hit-policy.md`. Moved verbatim under DCR-0054
(docs compaction, wave 5); the ADR's decision body states the current rule.

### Decision Drivers — second and third bullets, as written for the Review 0008 gate (R0008-0019, 2026-07-11)

* Treating reads as misses is always safe (worst case: re-translate) and
  the `tracing::warn` on every poisoned `get` deliberately keeps the
  underlying bug loud instead of self-healing it into silence.
* Writes recovering via `into_inner` preserves forward progress — new
  results from the current healthy run still land, so a long run isn't
  punished for one earlier panic.

### Decision Outcome — as written for the Review 0008 gate (R0006-0002, R0008-0019, 2026-07-11), with its superseded marker

ACCEPT R0006-0002 (fixed: hit revalidation + poison handling shipped);
REJECT R0008-0019 (the read/write asymmetry is the intended shape):
option 3. Option 1 trusts data a panic interrupted; option 2 throws
away provably-good progress written after the poison.

Status: Implemented (`cache.rs` doc comments trace the rationale;
`pipeline.rs` rejects-and-re-dispatches invalid hits).

> **Superseded** — see *Amended 2026-07-13* below.

### Amended 2026-07-13 (external review P2-8) — under Decision Outcome

The original outcome (option 3: miss-on-get, recover-on-put) is
**superseded**. It was internally inconsistent: `std::sync::Mutex`
poison never clears on its own, so after one panic *every* `get`
returned a miss for the cache's whole lifetime, while `put` kept
"recovering" via `into_inner` into a map no `get` would ever read —
**dead writes**. The two halves of option 3 could not coexist as
described; the recover-on-put branch was unreachable-by-readers.

New outcome: **clear-once-and-resume** (a refinement of option 2 that
does not discard a live instance). On the first poisoned lock — from
`get`, `put`, or `len` — the guard is recovered via
`PoisonError::into_inner`, the map is **cleared** (its contents were
mutated mid-panic and are untrusted), `Mutex::clear_poison` is called
so subsequent locks succeed normally, and a single `tracing::warn`
("cache poisoned; cleared and resumed empty") is emitted. The cache is
then a normal empty cache: gets miss until repopulated, and puts land
and *are readable*.

### Implementation — as written for the Review 0008 gate (R0008-0019, 2026-07-11)

`get` → miss + `tracing::warn` on poison; `put` → recover + write;
pipeline revalidates hits before acceptance.

### Amended 2026-07-13 (external review P2-8) — under Implementation

`InMemoryCache` now routes `get`, `put`, and `len` through one private
`locked()` helper. On a healthy lock it returns the guard unchanged; on
poison it recovers via `into_inner`, clears the map, calls
`Mutex::clear_poison` (std, stable since 1.77; within MSRV 1.85), emits
one `warn`, and returns a guard over the now-empty, functional map. The
pipeline's hit-revalidation behaviour is unchanged.

### Consequences — as written for the Review 0008 gate (R0008-0019, 2026-07-11)

* Good, because a panic can never launder a half-written entry into the
  output, and the warn stream surfaces the real bug.
* Bad, because one poisoning event makes the rest of the run cache-cold
  on reads (bounded cost: re-translation of not-yet-dispatched units).

### Amended 2026-07-13 (external review P2-8) — under Consequences

The "cache-cold on reads for the rest of the run" consequence above no
longer holds; it is **superseded** by clear-once-and-resume.

* Good, because after recovery the cache is fully functional again:
  writes are no longer dead, and a run is not punished for the rest of
  its life by a single earlier panic.
* Good, because clearing on poison is safe by construction — the
  discarded entries are exactly those a panic interrupted mid-mutation,
  which the original policy already deemed untrusted.
* Cost is bounded to entries present at the moment of the panic: they
  are dropped and re-translated on next demand. Everything written after
  recovery is cached normally.
* The single `warn` per poison event still keeps the underlying bug
  loud rather than self-healing it into silence.

### Appended 2026-08-09 (DCR-0028, ticket f12b8b)

The disk-backed cache design extends both halves of this policy rather than
inventing parallel ones, at the granularity each failure actually has:

* **Recovery.** `DiskCache`'s persisted state recovers in this ADR's shape —
  loud, safe, forward progress — but line- and file-granular instead of
  map-granular: an unreadable line is skipped with a `warn`, a torn tail is
  truncated to the last good record, and an unreadable or unknown-format file
  is rotated aside so the cache resumes empty. Its in-memory *index* mutex
  uses this ADR's clear-once-and-resume directly (clearing the index, not the
  file — the next open replays the log), as does the document-metadata map
  `InMemoryCache` gains.
* **Hit revalidation is what makes disk entries trustworthy.** Every cache
  hit already re-runs the per-unit validation layers on reuse and a failing
  hit is evicted and re-dispatched — that existing gate, not checksums, is
  why a damaged-but-parseable disk payload degrades to a re-translation and
  can never reach the output. The decision above did that work in advance;
  DCR-0028 leans on it and adds nothing to it.

## ADR-0016 — Whole-document-in-memory is inherent

Source: `docs/decisions/0016-whole-document-in-memory.md`. Moved verbatim under DCR-0054
(docs compaction, wave 5); the ADR's decision body states the current rule.

### Amended 2026-07-13 (external review P1-7) — the banner under the title

> **Amended 2026-07-13 (external review P1-7).** The original framing —
> "CLI input reads are not size-capped" — is superseded at the boundary: a
> `limit + 1` admission cap now ships (`--max-input-bytes`, default 64 MiB,
> for `--input`; a fixed 4 MiB cap for `--profile` / `--system-prompt-file`).
> This is a scoped decision, not a settled absolute — see **Amendment** below.
> The whole-document-in-memory core stance still holds for ACCEPTED inputs.

### Context and Problem Statement — Path note (2026-08-05, DCR-0019)

> **Path note (2026-08-05, DCR-0019).** The decision is unchanged; the code
> moved. `resolve_profile` and the capped reads now live in
> `crates/transync-cli/src/translate_cmd/input.rs`, and `run`'s body became
> `translate_cmd::execute`. The caps this ADR sets are enforced in the same
> place they always were, one module deeper.

### Decision Outcome — second paragraph, as it read before DCR-0054 (2026-10-06)

Originally recorded as "no change required" — that framing is now scoped by
the 2026-07-13 amendment: option 1 is accepted in a *narrow* form (a
boundary admission cap that rejects absurd inputs before parse), while the
in-memory stance itself is unchanged for everything the cap admits. This is
a **scoped decision, not a settled absolute**. Revisit trigger: transync
running as a service or multi-tenant deployment ingesting third-party
uploads — that deployment needs request-level limits *outside* the library
anyway, and the CLI-boundary cap is not a substitute for them.

### Implementation — the paragraph the 2026-08-07 Correction (ti `038c54`) refers to

`--max-input-bytes` (default 64 MiB) caps `--input`; a fixed 4 MiB constant
caps `--profile` / `--system-prompt-file`. Each is enforced by reading
`limit + 1` bytes via `Read::take` — metadata length is never trusted alone
— and exceeding a cap exits 2 (InputReadFailure) with a message naming the
limit (and, for input, the flag). See `crates/transync-cli/src/translate_cmd.rs`
(`read_capped`) and contracts.md §6. `docs/Performance.md` describes memory
scaling with document size for accepted inputs.

### Implementation — Correction (2026-08-07, ti `038c54`)

> **Correction (2026-08-07, ti `038c54`).** The last sentence above was never
> true. `docs/Performance.md` has been a wall-clock tuning guide since it was
> written and said nothing about memory consumption at any point in its history,
> so a reader sent there for the scaling curve found batching dials and provider
> latency instead. Nothing in this ADR rests on that forward reference: the
> reason peak memory tracks document size is argued in **Decision Drivers**
> above from the architecture itself — full Comrak AST, exact-source-range
> splicing, full-document reparse in validation layer 5 — and that argument is
> self-contained. Rely on it, not on the citation.
>
> As of this note `docs/Performance.md` does carry a "Memory" section, so the
> reference now resolves — but it resolves to something narrower than the
> original sentence promised. It is six coarse peak-RSS readings
> (`/usr/bin/time -l` over a stub-provider release run, one machine), reported
> as measured points and explicitly not as a scaling law. It does not predict
> unmeasured sizes, does not say where an input at the `--max-input-bytes`
> default would land, and does not locate the true OOM ceiling. The third
> consequence below therefore stands exactly as written. Read narrowly, that
> section is also the first thing to discharge the documentation half of
> option 3 under **Considered Options** — the half this sentence had been
> claiming as already done.

### Amendment (2026-07-13, external review P1-7)

The reviewer's `limit + 1` admission-control argument was **accepted**:
reading one byte past the limit and erroring on the extra byte is the
correct primitive because it never trusts `metadata().len()` (which can lie,
race, or grow). CLI-boundary caps shipped accordingly (`--max-input-bytes`
for `--input`, a fixed 4 MiB cap for the auxiliary text inputs). What did
**not** change is the architecture: for any input the cap admits, the
pipeline still parses a full Comrak AST, splices exact source byte ranges,
and reparses the *complete* regenerated document. The record is therefore
reframed from "settled: no cap" to "scoped: boundary caps ship, core
stance holds for accepted inputs, revisit on service/multi-tenant
deployment."

## ADR-0017 — Batch-terminal provider failures abort the whole run

Source: `docs/decisions/0017-batch-terminal-failures-abort-the-run.md`. Moved verbatim under DCR-0054
(docs compaction, wave 5); the ADR's decision body states the current rule.

### Consequences — the "boundary document" bullet, original wording (before the 2026-08-09 note's narrowing)

* Bad, because a **boundary document** — one whose largest translatable unit
  is a single block bigger than the output ceiling can hold (a giant table or
  a wall-of-text paragraph) — is *untranslatable* under a too-low ceiling:
  output-aware packing cannot split a single unit, so the run aborts until the
  operator raises `--target-output-tokens` (or the deferred oversize-split
  lands — STUB-017, the provider-side row-window / block splitter). The
  preflight names the block and the remediation flags rather than letting the
  provider truncate silently.

### Related — the STUB-017 entry, original wording (before the 2026-08-09 note)

- STUB-017 — deferred provider-side oversize / row-window splitter

### Note (2026-08-08) — cancellation adopts this shape, and is not this decision

*Appended, not a rewrite. Nothing above changes; this note records that a later
decision leaned on the reasoning here, and marks the boundary between them.*

DCR-0024 added run cancellation (`TranslateOptions.cancel`), and settled its
central question — what a cancelled run returns — by applying this ADR's
reasoning unchanged: a cancelled run answers `Err(TransyncError::Cancelled)`
and writes no output, rather than a `TranslationOutput` whose unreached blocks
are marked `fallback_source`. The driver that carried it is the second one
listed above: *loud failure beats a quietly-degraded document*, because an
untranslated island is not visually distinguished in raw Markdown and a
downstream reader can ship it without noticing.

Cancellation added a second objection of its own, which is why it is a separate
record rather than an extension of this one: `fallback_source` is a statement
that a block *was attempted* and its retries were spent (ADR-0009, invariant
6), and a cancelled run's unreached blocks were never attempted. Borrowing the
marker would make the two indistinguishable in the alignment map.

The scope boundary: **this ADR governs provider/transport-terminal batch
failures only.** A cancellation is not a failure at all — it is the caller's
own instruction — so it does not consume this ADR's rejected alternatives, and
a future proposal to add a per-batch fallback rung would still be arguing
against the decision above, not against DCR-0024.

### Note (2026-08-09) — the row-window splitter is commissioned; the boundary-document cost narrows to non-table kinds

*Appended, not a rewrite. Nothing in the decision above moves.*

DCR-0026 (ticket `fc0304`, owner decision 2026-08-06) commissions the deferred
oversize split this ADR's first "Bad" consequence pointed at (STUB-017), for
**tables only**, and it lands entirely on this ADR's prevention side: an
oversize table is split into header-carrying row-window units **at packing
time, before the first provider call**, deterministically — the same kind of
act as batch packing, not a runtime rung that catches a provider failure.

What this ADR settled stays settled, on every point:

- **Abort remains the terminal behavior.** No per-batch fallback rung and no
  policy flag were added. A provider/transport-terminal error still aborts the
  whole run, windows included; a provider-side oversize signal after packing is
  still terminal. DCR-0026 explicitly rejected a reactive re-split for exactly
  the reasons recorded here.
- **The "boundary document" consequence narrows rather than disappears.** A
  document whose over-ceiling block is a **table** stops being untranslatable
  under a too-low ceiling once DCR-0026's slices ship. Every other kind —
  paragraph, code block (invariant 4), list item, blockquote, html — keeps this
  ADR's semantics exactly, each excluded by a recorded per-kind decision in
  DCR-0026 §7, with the preflight naming the block and the ceiling flags (and
  now the table-strategy knob, where applicable) as the remedy.
- The preflight and its terminal-error annotation are unchanged; a window that
  cannot fit even alone (a single giant row) is flagged and aborts exactly as a
  whole oversize unit does today.

## ADR-0018 — HTML content is translatable via app-owned segment extraction

Moved verbatim from `docs/decisions/0018-html-content-translation-via-segment-extraction.md`
(git blob `fe068ac4fb3ecf9016fa31282fa28a6623e5b2fa`) when its two dated path notes and its
amendment section were folded into the decision body (DCR-0054, compaction wave 5). This is
history, never current truth: the ADR's own decision body is the rule. The first two sections
below are the *Implementation* passages that the rewrite replaced, in their original wording —
the lead with the two path notes that followed it, then the **Validation** bullet; the last is
the amendment section, with only its heading's level changed (`##` → `###`).

### Implementation — the lead and its two dated path notes, wording before they were folded in

Recorded in full in **DCR-0016**; the decision-relevant shape:

*Path note (2026-08-04, DCR-0017): the module paths below are as of this
record's date. `htmlseg`, the parser, and `HtmlOutcome` moved to
`crates/transync-syntax/src/` in the crate split — `htmlseg.rs` and
`parser.rs` at the same filenames, `HtmlOutcome`/`html_outcomes` into
`outcome.rs` (still reachable as `transync::unit::html_outcomes`). The
decision is unaffected: one parser owns block boundaries, `lol_html` works
inside a block, and both now live in the same crate.*

*Path note 2 (2026-08-04, DCR-0018): the `transync::unit::html_outcomes`
path cited just above is no longer reachable. The OI-0027 curation made
`transync-core`'s `unit` module `#[doc(hidden)] pub` and dropped it from the
facade's curated re-export list, so `html_outcomes` is named as
`transync_core::unit::html_outcomes` (or `transync_syntax::outcome::html_outcomes`)
by code that depends on an engine crate directly — which is what
`transync-openai`'s live-smoke test now does, via dev-dependencies. `htmlseg`
was never on the facade and is additionally narrowed: `balance_fragment` is
`pub(crate)`. The decision is again unaffected — this changes who may name
the machinery, not what it does.*

### Implementation — the Validation bullet, wording before the 2026-08-09 amendment was folded in

* **Validation** — per-kind `check_html` (JSON shape, count, non-empty;
  retryable); a layer-3 post-splice check whose *engine faults* go to **direct
  fallback** (no retry burn, no cache put) because a splice failure after
  acceptance is not a model fault; `html` labels on both sides of the
  full-document reparse compare; and an **always-on** inline raw-HTML tag
  guard hoisted above the inline layer's policy gates.

### Amendment (2026-08-09) — "non-empty" is "non-blank" (R0003-0042)

*Appended, not a rewrite.* The Decision Outcome's **Validation** bullet above
says `check_html` enforces "JSON shape, count, non-empty". The last word is now
**non-blank**: a translated segment consisting only of whitespace is rejected
the same way an empty one is, and for the same reason. Extraction drops every
source text node whose decoded form is all whitespace, so a segment the model
sees always carries visible text — a whitespace-only answer is content erasure
with no legitimate case behind it, and it used to pass layer 2 and every layer
after it (structure is unchanged, so the splice, the fragment reparse and the
full reparse all agree) and ship as `translated`. The rejection is retryable,
like the rest of the bullet. See DCR-0016's 2026-08-09 amendment.

## ADR-0019 — The browser gets the Rust renderer as a wasm demo layer, not the CLI bundle

Source: `docs/decisions/0019-wasm-demo-layer.md` (git blob
`f78ac60aa5fb5fe0bbe13be33c836a7e81d43d76`). Moved verbatim when the ADR's decision
body was rewritten to state the current rule (DCR-0054, compaction wave 5,
2026-10-06). This is history, never current truth: the ADR's own decision body is
the rule. The sections below are the passages the rewrite replaced, in their
original wording and in document order. The 2026-08-05 dated note under decision 4
and the 2026-08-05 note under the html read-only consequence travel inside their
sections; the `## Amendment (2026-08-07)` section follows with only its heading
level changed (`##` → `###`). Italic lines directly under a heading are
navigation added at the move, not part of the original text.

### Frontmatter `description:` — as it read before compaction (2026-08-05 state)

description: A sixth workspace member, `transync-wasm`, wraps `transync-syntax` for the browser — syntax-only dependency (charter plus the getrandom trap), a JSON-string boundary, and both view and edit modes. wasm-bindgen is pinned exactly at `=0.2.126` instead of committing a lockfile (OI-0020 stays open, with a sharpened risk); binaryen >= 121 becomes a documented host prerequisite with an explicit `wasm-opt` pass; the module is budgeted at 1,950,000 B raw / 810,000 B gzip against a measured 1,739,791 / 718,136. The CLI `--html-out` bundle deliberately carries no wasm. Owner-settled 2026-08-05.

### Status — the status blockquote, as it read before compaction

*The status blockquote named the alignment schema by its then-current value; the body now points at `ALIGNMENT_SCHEMA_VERSION`.*

> **Status: accepted.** Owner-settled 2026-08-05 (design spec
> `docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md`,
> approach A1, sections S1–S8); implemented and landed the same window in six
> commits, `94d7e39`..`1d04135`. This ADR records **Track C proper** — the
> browser integration OI-0028 / DCR-0017 deliberately deferred after
> delivering only the compile path and its standing gate. It is **additive**:
> the curated facade surface, the alignment schema (`1.2.0`), the ADR-0006
> six-file bundle contract, both shipped shells, and `sync.js` are all
> untouched.

### Decision Drivers — "Reproducibility without a lockfile", as it read before compaction

* **Reproducibility without a lockfile.** `Cargo.lock` is uncommitted here
  (OI-0020, blocked by a machine-global gitignore). wasm-bindgen requires
  **exact** crate↔CLI version identity: a resolution drift does not degrade,
  it breaks the build outright, and the repair path (`cargo install
  wasm-bindgen-cli@<v>` from source) needs a network this machine measured as
  restricted.

### 1. Crate shape — `transync-syntax` and nothing else — the two crate-type passages, as they read before compaction

`crates/transync-wasm` is the workspace's **sixth** member
(`crate-type = ["cdylib", "rlib"]`, `publish = false`), and among workspace
crates it depends on **`transync-syntax` alone**. The restriction carries a
double rationale, and both halves are load-bearing:

Logic lives in `engine` — plain Rust, zero wasm-bindgen — with only thin
`#[wasm_bindgen]` wrappers in `lib`. That split is what makes the whole surface
**host-testable**: `cargo test -p transync-wasm` exercises the real call
sequences, and the wrappers stay obviously correct by inspection. `cdylib` +
`rlib` also means the ordinary `cargo build/test/clippy --workspace` sweeps the
new member with no special-casing (bindgen exports are inert on the host).

### 2. wasm-bindgen is pinned exactly; OI-0020 stays open

*Decision 2 as written on 2026-08-05 (owner decision: pin over lock), reversed 2026-08-06 by commit `6cf4164`; see the 2026-08-07 amendment below.*

`wasm-bindgen = "=0.2.126"` in `[workspace.dependencies]` — an exact pin, not a
caret range. Given no committed lockfile, the pin is the only defense against
clean-checkout skew on a dependency whose version mismatch is fatal rather than
degrading.

**The escalation this wave produced, recorded honestly so the owner can
revisit.** The pre-implementation analysis expected the binding constraint to
be `slug → comrak`, i.e. a wasm32-side edge sharing a resolution with a
dependency we already had. That is **wrong**, and the correction matters:

* `slug`/`comrak` declares only a caret `"0.2"` and never constrained anything.
* The binding edge is **host-side**: `reqwest → js-sys` inside
  `transync-openai`, where `js-sys 0.3.97` **exact-pins**
  `wasm-bindgen = "=0.2.120"`. Because cargo resolves one wasm-bindgen for the
  whole workspace lock, a host-side exact pin was holding the *wasm32* tree
  down.
* Unification therefore needed a **seven-package** update
  (`wasm-bindgen` + macro/macro-support/shared, `js-sys`, `web-sys`,
  `wasm-bindgen-futures`), not the one-liner the plan assumed. `reqwest` itself
  did not move, and all of `transync-openai`'s tests stayed green, so the
  js-sys/web-sys bump is behaviorally inert here.
* **The sharpened risk:** that seven-package resolution exists **only in the
  gitignored `Cargo.lock`**. A future wasm-bindgen release moves fresh `js-sys`
  resolutions, and a clean checkout can then land on a `js-sys` whose own exact
  pin **conflicts with our `=0.2.126`** — a hard resolver failure originating
  in a `reqwest` bump that looks entirely unrelated to wasm.

**Owner decision 2026-08-05: pin over lock.** OI-0020 stays OPEN and
un-retracted; the pin is the mitigation, this paragraph is the warning, and the
recovery is a matching `-p js-sys` update (or, finally, committing the lock).

### 4. Size budget — 1,950,000 B raw / 810,000 B gzip

*Decision 4 as written on 2026-08-05, including its appended dated note (ticket `92ac61b9`, the `rlib` drop and the lowered ceilings).*

`scripts/build-wasm.sh` asserts the ceilings and fails the build on a breach,
so the number is a gate rather than a note. Measured today: **1,739,791 B raw /
718,136 B gzip** (1.66 MiB / 701 KiB), byte-reproducible across clean rebuilds
on the implementing host.

The spec's original ceilings (1,400,000 / 500,000, from a 1,237,349 B probe)
were **factually wrong input, corrected by measurement** — the exploration
probe compiled **view mode only**. The full attribution chain is recorded
durably in **DCR-0020**; the short form is that edit mode's Markdown
*reserialization* (`regen::regenerate` + `outcome::html_outcomes`) costs
+435,770 B that a render-only probe could not see. The corrected budget was
owner-approved 2026-08-05, and the spec and plan carry matching dated
corrections.

Two decision-relevant readings of that table:

* **The JSON-string boundary is cheap** — +42,127 B — so approach A1's
  "no `serde-wasm-bindgen`, no new dependency, one `JSON.parse` per call"
  choice is vindicated on size grounds, not merely on dependency grounds.
* **A breach is a design signal**, not a number to bump reflexively. The
  ceilings sit ~210 KB raw / ~92 KB gzip above the measurement; the one known
  recoverable slice (+113,844 B of fat-LTO scope lost to the `rlib` crate-type
  sitting beside the `cdylib`) is ticketed as `92ac61b9` rather than spent.

*(Dated note, 2026-08-05 — appended, nothing above rewritten. Ticket
`92ac61b9` was resolved by **spending** that slice: `crate-type` is now
`["cdylib"]` alone, so decision 1's manifest line and this bullet's "rather
than spent" both read as history. Re-measured A/B at the crate's then-current
size, the drop returns **101,228 B raw / 46,758 B gzip**, and the enforced
ceilings came down with it to **1,840,000 / 760,000** — the same ~12 %
headroom. Decision 1's host-testability claim is unaffected and was re-verified
rather than assumed: `cargo test` compiles the lib target into its own test
harness whatever the crate-type, so `cargo build/test/clippy --workspace` still
sweeps the member with no special-casing. DCR-0020's attribution table carries
the numbers.)*

### Consequences — the clean-checkout and schema-mirror bullets, as they read before compaction

* Bad, because **`transync-wasm` cannot be built from a clean checkout without
  luck**. Its build needs wasm-pack, binaryen ≥ 121, the `wasm32-unknown-unknown`
  target, *and* a dependency resolution that satisfies `=0.2.126` — and the
  last of those lives only in an uncommitted lockfile. This is OI-0020's cost,
  now concentrated on a critical path instead of spread thin, and it is the
  strongest argument yet for committing the lock.
* Bad, because a second schema mirror exists. `web/js/wasm-demo.js` carries its
  own `KNOWN_SCHEMA = "1.2.0"`, joining both `sync.js` copies. Mitigated rather
  than removed: it is checked **twice** at boot — against the wasm module's own
  `schema_version()` (build-time drift: a blob from a different commit) and
  against the fetched map (data drift) — and both checks fail closed. A
  host-side test pins `schema_version()` to `ALIGNMENT_SCHEMA_VERSION`, so the
  blob can never silently disagree with the crate it was built from.

### Consequences — the html read-only bullet and its 2026-08-05 note ("the consequence runs one step further")

* Bad, because html blocks are **read-only** in the demo. Their payload is an
  extracted text-segment array (ADR-0018), not Markdown, so a Markdown slice
  could not be spliced back; the demo declines the edit rather than offering
  one it cannot round-trip.

  **2026-08-05 — the consequence runs one step further than "read-only".**
  Read-only is not free: the segment array is unrecoverable from the rendered
  bundle (the demo only ever sees the spliced result), so an html block cannot
  be *carried* through a rebuild either. `regen` falls it back to its **source**
  bytes, and any *other* block's edit therefore reverts every translated html
  block in the document. That degrade is accepted — but it must be visible, so
  `buildEditModel` omits html rows from the edit model's `statuses` map as well
  as its `payloads`. `align::build_alignment_map` then synthesizes
  `fallback_source` for unit-backed html rows, and the renderer presents them
  as the escaped `data-skipped="html-block"` placeholder under the fallback
  tint, in **both** panes. Preserving the map's original `translated` instead
  would have left the row — and the tint legend — asserting a translation the
  pane no longer shows. Pinned host-side in `transync-wasm`
  (`omitted_unit_backed_html_row_is_fallback_source_not_translated`) and in
  browser test `c`.

### Amendment (2026-08-07) — the lock was committed after all; the pin stays

Decision 2 above chose "pin over lock" and recorded that OI-0020 "stays OPEN and
un-retracted". **The owner reversed that one day later, on 2026-08-06, in commit
`6cf4164`:** `.gitignore` gained a repo-local `!Cargo.lock` un-ignore overriding
the machine-global rule, and `Cargo.lock` is now tracked. The seven-package
resolution decision 2 says "exists **only in the gitignored `Cargo.lock`**" is
therefore a tracked file, and the clean-checkout failure mode recorded there — a
future wasm-bindgen release moving fresh `js-sys` resolutions into conflict with
`=0.2.126` — no longer sits in front of a first build. **OI-0020 is RESOLVED
(2026-08-06)**; `docs/project/open-issues-archive.md` carries the resolution and names
the two things that survive it: the `CLAUDE.md` half of the same gitignore
concern, declined rather than fixed, and the absence of any `--locked` gate over
the committed lock.

**What is unchanged: the exact pin.** `wasm-bindgen = "=0.2.126"` stays in
`[workspace.dependencies]` and must not be loosened. Decision 2's reasoning for
an exact pin survives the lock intact, because a lockfile binds only the
resolves that read it — `cargo update`, a regenerated lockfile, and any build
after the lock is deleted all re-resolve from the manifests — and because
wasm-bindgen's requirement is crate↔**CLI** identity, of which no lockfile can
pin the CLI half at all (`wasm-bindgen-cli` is a host tool installed outside the
workspace). What changed is the pin's standing, not its necessity: it was the
*only* defense against clean-checkout skew and is now the half that covers the
lock-less resolves, with the lock covering the checkout. The root manifest's
comment beside the pin and `docs/project/release-checklist.md` step 18 both say
this. The front-matter description, the "Reproducibility without a lockfile"
driver and decision 2's heading and text are left exactly as written — they
record the 2026-08-05 state, and this amendment is the correction.

### Related — the OI-0020 and ADR-0003 bullets, as they read before compaction

- **OI-0020** — no committed `Cargo.lock`; **stays OPEN** by owner decision 2026-08-05, with the sharpened clean-checkout risk recorded in decision 2 above — **superseded 2026-08-06** (`6cf4164`): the lock is committed and OI-0020 is RESOLVED, while the exact pin stays; see the 2026-08-07 amendment above
- ADR-0003 — the workspace decision, **amended 2026-08-05** by this ADR: six members, `transync-wasm → transync-syntax`
