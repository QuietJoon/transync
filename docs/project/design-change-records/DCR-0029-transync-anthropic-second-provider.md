---
type: DCR
title: transync-anthropic — the second provider crate, on the interface the last two days finished moving
description: A sibling Translator implementation over the Anthropic Messages API, mirroring transync-openai's construction-time config identity, transport bounds and stub/live test split, and differing exactly where the provider does — one HTTP surface, x-api-key auth, structured outputs via output_config.format, a required output ceiling, and a stop-reason vocabulary mapped honestly onto DCR-0023's taxonomy. Design-first record for ticket bda471; slices SL-115..SL-119. One open fork — the model_context_window_exceeded terminal cause — is put to the owner rather than decided here.
tags: [change, project-control, DCR-0029]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-10T00:00:00Z
status: stable
---

# DCR-0029: `transync-anthropic` — the second provider crate

- **Date:** 2026-08-10
- **Source:** ticket `bda471` — owner decision 2026-08-06 (**Anthropic first;
  local-llama later**), deliberately scheduled **last** of the six post-0.3.0
  roadmap items: the crate implements `Translator` and returns
  `TranslatorError`, and both moved in the two days before this record —
  DCR-0023 gave the error its typed terminal causes, DCR-0024 added the
  `cancel: &CancellationToken` parameter to both trait methods. This crate is
  designed against the post-0023/0024 interface and against nothing older.
- **Design-first.** This record precedes the code; its slices
  (SL-115..SL-119) are the implementation plan. Living documents
  (`contracts.md`, `open-issues.md`, `backlog.md`, module docs) keep
  describing shipped behavior and are edited by the slice that ships each
  change, not by this record.
- **Paired record (the pairing rule):** a dated amendment appended 2026-08-10
  to **ADR-0002** (`docs/decisions/0002-http-free-core-with-translator-trait.md`),
  which is where the provider-crate pattern — and `transync-anthropic` by
  name — was decided. No new principle is introduced, so no new ADR: this
  record is the first exercise of ADR-0002's "future providers are sibling
  crates, purely additive" promise, and the amendment records what that
  exercise demands of the seam. ADR-0003 (workspace with provider crates) and
  ADR-0020 (cache identity excludes adversarial collision) are load-bearing
  and **unchanged**.
- **Affected contracts:** `docs/architecture/contracts.md` gains a new **§8**
  (`transync-anthropic` constructor + provider signals), mirroring §7's
  structure. **§0 is untouched** — the facade does not re-export provider
  crates, so `crates/transync/tests/public_surface.rs` does not move. §1
  moves **only if** the open fork below resolves to a new variant.
- **Not breaking by itself.** The crate is additive (ADR-0002). The one
  candidate breaking change — a new `TranslatorError` variant — is the open
  fork, and if taken it rides the already-open **v0.4.0** window.

## What Changes

A new workspace member, `crates/transync-anthropic`, implementing
`transync::Translator` over the **Anthropic Messages API**
(`POST {base_url}/v1/messages`). It is the second in-tree provider and the
first proof of ADR-0002's claim that a provider lands without touching the
core trait. The concrete type is `TransyncAnthropic`.

The crate **mirrors `transync-openai` deliberately** — same module split
(config root, `client` flow, private `transport`/`classify`/envelope modules,
`error` mapping), same construction-time configuration identity (commit
`b507aba`, ticket `a60f07`, R0001-0031), same transport bounds (32 MiB answer
cap, 64 KiB error-body cap, 512-byte diagnostics, 10 s connect / 120 s
request budgets, `with_timeout` override), same offline-test/live-smoke
split (OI-0030's two-gate pattern), and the same reliance on the shared
tier-(b) surface (`transync::llm::prompt`, `profile::render_prompt_body`)
for every provider-neutral byte — prompt assembly, schema object, response
parsing. **The two adapters never share code across the crate boundary**;
what they share is the tier-(b) surface and the discipline. The transport and
classification modules are duplicated-with-intent, not extracted — see
*Alternatives*.

### Where it mirrors, and where it must differ

| axis | `transync-openai` | `transync-anthropic` |
|---|---|---|
| HTTP surface | two (`/v1/chat/completions`, `/v1/responses`), model-name heuristic, `TRANSYNC_OPENAI_API`, `with_api` pin | **one** (`/v1/messages`). No `Api` type, no surface env var, no `with_api`, no `api()` accessor — the whole dual-dispatch layer has no counterpart |
| auth + protocol headers | `Authorization: Bearer` | `x-api-key` header, plus `anthropic-version: 2023-06-01` on every request (a crate constant — see *Config identity*) |
| schema enforcement | Structured Outputs: `response_format.json_schema` (chat) / `text.format` (responses), `strict: true` | Structured outputs: **`output_config: {format: {type: "json_schema", schema}}`** — see *Schema enforcement* |
| system prompt carrier | `messages[0]` (`system` / `developer` role) | top-level `system` request field; `messages` holds the single user turn |
| output ceiling | optional; omitted when `[batching].target_output_tokens` unset | **`max_tokens` is a required request field** — see *The required ceiling* |
| reasoning knob | `reasoning_effort` / `reasoning.effort`, `with_reasoning_effort` | `output_config: {effort}`, `with_effort`; **no `thinking` parameter is ever sent** — see *Thinking* |
| extraction retention | Responses body sends `store: false` | no equivalent parameter exists on the Messages API; nothing is sent, and §8 records the asymmetry rather than letting it look forgotten |
| env-read surface | `OPENAI_API_KEY`, `TRANSYNC_OPENAI_MODEL` (default `gpt-5-chat-latest`), `TRANSYNC_OPENAI_BASE_URL`, `TRANSYNC_OPENAI_API` | `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` (default `claude-opus-5`), `TRANSYNC_ANTHROPIC_BASE_URL` (default `https://api.anthropic.com`). **Three variables, not four** — no surface to select |
| `fingerprint()` | `("openai", [model, base, api, effort])` | `("anthropic", [model, base, effort])` — one axis fewer, same injective composition |
| `tokenizer_hint()` | per-generation tiktoken map | `Some(TokenizerHint::Cl100kBase)`, flat — the *deliberate, documented approximation* the `TokenizerHint` docs name for non-OpenAI providers. Overriding to a constant beats `None` (the model-name heuristic over the advisory `model_id`) because it is a decision this crate states rather than a guess core makes about a tokenizer it does not know |

### Schema enforcement — the same strict contract, this provider's spelling

**Every request carries the schema. There is no prompt-coaxed-JSON mode.**
Both call paths send the shared schema object under
`output_config.format = {type: "json_schema", schema: …}` — translation with
`prompt::schema_object_for(...)`, extraction with
`prompt::extraction_schema_object(...)` — so architectural invariant 5's
first layer (JSON/Structured-Output schema) holds on this provider exactly
as it does on the shipped adapter, and the downstream validators see the
same shapes either way. The old top-level `output_format` parameter is
deprecated on this API and must not be used.

Two provider facts shape the implementation:

1. **Anthropic's structured outputs accept a narrower JSON-Schema dialect.**
   `additionalProperties: false` and `required` are demanded (the shared
   schema already satisfies both — OpenAI strict mode demanded the same),
   but numerical constraints, string-length constraints and array-count
   constraints are **not** part of the accepted dialect. The one place the
   shared schema uses such a keyword today is the extraction schema's
   `maxItems` on the terms array (stamped from `req.max_terms`). The adapter
   therefore passes the shared object through a small, deterministic,
   crate-private **schema-profile pass** that removes keywords the provider
   does not accept — nothing else is rewritten, and the pass is pinned by an
   offline test so it cannot silently widen. The dropped cap is then
   **enforced post-parse**: `extract_glossary` truncates the parsed list to
   `req.max_terms` before returning, so the `GlossaryExtractionRequest`
   contract holds regardless of what the provider did with the keyword.
2. **Structured-output support is model-gated** (current generations and
   Haiku 4.5; older snapshots reject it). The adapter does not maintain a
   model allowlist — a model that rejects `output_config` answers HTTP 400,
   which classifies as `ProviderRejected {status: Some(400)}` with the
   provider's own message in the capped diagnostic, and §8 names the
   constraint so the operator remediation (pick a supporting model) is
   discoverable. An allowlist would rot with every model launch; the 400 is
   already precise. This costs what the commissioning note predicted — a
   misconfigured model costs an error per batch, not silent schema drift —
   and the error is terminal, so nothing retries it.

Structured outputs are incompatible with citations and prefilling on this
API; the adapter uses neither, so nothing to avoid — recorded so nobody
adds one later.

### The required ceiling, and thinking

`max_tokens` is a **required** field on the Messages API — there is no
"omit it and take the provider default" path, which is how the OpenAI
adapter treats an unset `[batching].target_output_tokens`. The rule:

- `[batching].target_output_tokens` set → sent as `max_tokens`, unchanged
  semantics (§7's *output ceiling* reasoning carries over: terminal on
  exhaustion, not a cache axis).
- unset → the crate constant `DEFAULT_MAX_OUTPUT_TOKENS = 16_384` is sent.
  16 K is the non-streaming guidance ceiling for this API (large enough that
  a real batch answer fits, small enough that a non-streaming request stays
  inside HTTP timeouts); the adapter does not stream.
- The extraction preflight keeps its own fixed small cap (the
  `EXTRACTION_MAX_OUTPUT_TOKENS`-class constant, 4096), exactly as on the
  OpenAI side — `target_output_tokens` never moves it.

The consequence worth stating: **`stop_reason: "max_tokens"` (→
`OutputCeilingExhausted`) can now fire on a run whose operator set
nothing**, because the ceiling always rides in the request. The diagnostic
must therefore name the value that was sent, whether it came from the knob
or the default, and the knob (`[batching].target_output_tokens` /
`--target-output-tokens`) as the remediation — the DCR-0023 rule that a
ceiling stop names its knob applies with the default counted as a ceiling.

**No `thinking` parameter is ever sent.** This is the only choice valid
across the provider's whole current model family: the legacy
budget-token form is rejected (400) on current models, and the explicit
`disabled` form is rejected on the top-tier model unconditionally and on the
default model at higher efforts. Model defaults therefore apply (thinking on
by default on current models), and because `max_tokens` caps **thinking plus
answer together** on this API, the ceiling diagnostic also states that
reasoning shares the budget — an operator seeing an exhausted ceiling on a
short batch should raise the knob, not suspect truncation logic. The
`with_effort` builder (sending `output_config.effort`, omitted when
unconfigured) is the sanctioned depth knob and is a fingerprint axis, as
reasoning effort is on the OpenAI side.

### Terminal-cause mapping — the honest table

DCR-0023's variants name *why the provider stopped*. Anthropic's stop
reasons and error shapes are not OpenAI's set; each maps on its own
evidence, none by analogy. The classification below **is** the adapter's
entire retry contribution: core's pipeline re-dispatches exactly `Network`
and `RateLimited` as bounded verbatim resubmissions (ADR-0009, §5), so
picking a variant here is declaring a failure retryable or terminal, and
the adapter runs **no retry loop of its own** — an inner loop would
multiply core's budgets.

| Anthropic signal | `TranslatorError` | notes |
|---|---|---|
| HTTP 401 (`authentication_error`), 403 (`permission_error`) | `Authentication` | both are credential/permission faults; the key is never echoed |
| HTTP 429 (`rate_limit_error`) | `RateLimited { retry_after }` | `retry-after` parsed in both RFC 7231 forms, same parser rules as §7 (unparseable / stale date → `None`); core's 30 s cap still governs |
| HTTP 408, 409, 425 | `Network` | commonly retryable, as on the OpenAI side (R0008-0032) |
| HTTP 5xx — including **529 `overloaded_error`** | `Network` | 529 is this provider's overload signal and is named in the classify table and its test, not left to fall through the `>499` arm silently |
| any other 4xx — 400 `invalid_request_error`, 404 `not_found_error`, 413 `request_too_large`, … | `ProviderRejected { status: Some(code) }` | the status is the discriminator, per DCR-0023; 404 is a model name to fix. **The 400 arm also carries this provider's context-window overflows when they surface at the HTTP layer** (an over-long prompt can be rejected as an invalid request rather than answered) — same fact as the stop-reason fork below, different door, and the status-based classification is already honest for it |
| `reqwest` timeout / connect | `Network` | |
| `reqwest` decode | `MalformedResponse` | |
| answer body over 32 MiB | `ResponseTooLarge` | declared-length up-front or streaming, as in §7 |
| `stop_reason: "max_tokens"` | `OutputCeilingExhausted` | diagnostic names the sent value, its origin (knob or `DEFAULT_MAX_OUTPUT_TOKENS`), and that thinking shares the budget |
| `stop_reason: "refusal"` with `stop_details.category: Some(_)` | `ContentFiltered` | the provider's safety layer named a policy category — the "provider's own content policy" cause; category + explanation ride the capped diagnostic |
| `stop_reason: "refusal"` with no category | `ModelRefused` | the model declined without a policy label. The split is a stated heuristic: this API reports both declines under one stop reason and `stop_details.category` is the only evidence of which layer acted. Both are terminal with no remediation, so a misdrawn boundary costs a name, never a behavior — but the two causes stay distinct rather than flattened, per DCR-0023 |
| `stop_reason: "model_context_window_exceeded"` | **OPEN FORK** | see below — the taxonomy has no honest variant for it |
| `stop_reason: "stop_sequence"` / `"tool_use"` / `"pause_turn"` | `Other`, naming the stop reason | unreachable by construction — the adapter sends no stop sequences and declares no tools — so if one arrives, *unknown* is the honest answer. §1's rule ("reach for `Other` rather than force an ill-fitting name") applies; in particular `pause_turn` is a *resumable* state the taxonomy deliberately cannot express, and expressing it is a server-tools feature this crate does not have |
| missing/empty text output, unparseable JSON | `MalformedResponse` | via the shared `prompt::parse_*` parsers |
| run token fired / adapter's own stop | `Cancelled` | DCR-0024 terms, below |

**Envelope reading order is a rule, not a habit:** `stop_reason` is checked
**before** any content is read — a refusal can arrive with an empty
`content` array, so code that reaches for the first block unconditionally is
wrong on this API. The model's JSON rides in the first `text`-typed content
block (thinking-summary blocks, when a model emits them, precede it and are
skipped, never parsed, never logged). `stop_details` is populated only on
refusal and is guarded accordingly. The error envelope's `request_id` is
included in the capped diagnostic when present — it is the provider's own
correlation handle and costs nothing.

### Open fork — the context-window terminal cause

`stop_reason: "model_context_window_exceeded"` means the request — input
plus requested output — did not fit the model's context window. It is
terminal (a verbatim resubmission fails identically) and it is
**operator-actionable**: the remediation is the batching configuration
(`[batching]` token budget / `max_units_per_batch`), not the output knob.
Mapping it onto `OutputCeilingExhausted` would name the wrong knob — the
actively-harmful classification class DCR-0023 was written to remove — so
that mapping is off the table. Two defensible shapes remain, with materially
different consequences, and this record does not pick silently:

- **Option A — a new variant.** `TranslatorError::ContextWindowExceeded(String)`
  (stable code `provider_context_window_exceeded`), carried by the open
  v0.4.0 window. Costs: a §1 vocabulary row, two `error_taxonomy.rs` weld
  rows, and a migration note for the two sibling consumers' stable-code
  tables. Buys: the remediation (batching knobs) has a place to live, and
  DCR-0023's own criterion — *a distinct cause with a distinct remediation
  gets a name* — is satisfied rather than deferred. The OpenAI adapter is
  **not** obliged to follow: its context overflows arrive as HTTP 400 and
  keep their status-based `ProviderRejected` classification, which is
  already honest there.
- **Option B — the sanctioned catch-all.** `TranslatorError::Other` with the
  stop reason named in the message. Post-DCR-0023, both sibling consumers
  treat `Other` as a terminal provider stop, so there is no retry-loop harm;
  the cost is the lost machine-readable remediation signal and the near
  certainty of re-litigating this exact paragraph when a consumer files the
  DCR-0023 complaint about it.

**Recommendation: Option A** — the window is open now and will not be later.
The decision gates SL-117 only; SL-115 and SL-116 proceed either way.

### Config identity — the `a60f07` pattern, applied

Everything §7 records for the OpenAI adapter's construction discipline
applies, with the surface axis deleted:

- **Constructors:** `try_new(api_key, model, base_url)` canonical
  (fail-fast: empty/whitespace key, empty/padded model via the crate's own
  `ModelId::parse` with §7's exact rules, non-`http(s)`/host-less base URL);
  `new` unchecked, validates nothing, and is where the one-per-process
  cleartext-`http`-to-non-loopback `tracing::warn` (target
  `transync::anthropic`) is emitted; `from_env` **is** `try_new` once the
  three variables are read, with the same fallback rules (a blank
  `TRANSYNC_ANTHROPIC_MODEL` takes the default; a padded one is refused).
- **Env reads happen at construction, never at request time.** There is no
  surface variable to resolve, so the invariant is simpler here: the three
  variables above are the entire env-read surface of the crate, §8 lists
  them exhaustively, and nothing reads the environment after `from_env`
  returns.
- **`model_id` is caller-owned.** `ModelId` is this crate's authoritative
  model identity — the wire value and a fingerprint axis, stored verbatim —
  and `transync::TranslateOptions::model_id` remains the advisory label it
  is today. The crate ships no model registry and validates no model names
  beyond §7's blank/padding rules: which models exist is the provider's
  authority, and a wrong name is a 404 → `ProviderRejected {status: Some(404)}`.
- **`fingerprint()`** = `ProviderFingerprint::new("anthropic", &[model,
  effective_base_url, effort_or_default])` — every output-affecting axis the
  instance owns, nothing else. The request timeout stays out (it changes
  *whether*, never *what* — §7's reasoning verbatim). The
  `anthropic-version` header value stays out too, deliberately: it is a
  crate-wide constant, not a per-instance axis — two instances of one crate
  build cannot disagree on it, and a crate release that changes it can state
  the cache consequence in its changelog, exactly as prompt-text revisions
  (also version-carried, also not fingerprinted) already do. ADR-0020
  governs: this identity defends against accidental collision only; no
  adversarial hardening is added or owed.
- **Builders:** `with_effort`, `with_timeout` (+ `request_timeout()`
  accessor), each setting one independent field, order-insensitive.

### Cancellation (DCR-0024)

Both trait methods race their round-trip against the run's token with
`tokio::select! { biased; … }` — the token polled first, so an
already-cancelled run never issues the request — returning
`TranslatorError::Cancelled` on loss, exactly the trait-doc shape. The
extraction path re-checks nothing afterwards (the pipeline owns the
post-call re-check). The token is observed, never cancelled.

### The stub/live split (OI-0030 pattern)

- **Everything that runs by default runs with no key and no network:**
  request-body pins (`serde_json::to_value` over the builders), envelope
  reader over fixture bytes (every stop reason, the refusal split, the
  empty-content refusal), the classify table (including 529 and both
  `retry-after` forms), the schema-profile pass, transport bounds against
  the one-shot loopback server, config/fingerprint tests, and one
  end-to-end offline test driving the whole adapter against a canned
  Messages envelope served from loopback.
- **The live half never runs by accident:** `tests/live_smoke.rs` with the
  two independent gates — `#[ignore]`, plus a `decide_gate`-style pure
  predicate requiring `TRANSYNC_LIVE_SMOKE=1` *and* a non-empty
  `ANTHROPIC_API_KEY` — where the predicate itself is unit-tested offline so
  the gate cannot rot into vacuity. **No default-run test may pass because
  the key is absent**; absence is a printed skip on the ignored path only.
  Model knob `TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL`, default
  `claude-haiku-4-5` (cheapest structured-outputs-capable model — the live
  round-trip is endpoint-plus-validation-stack evidence, not quality
  evidence). `scripts/smoke-live-gate.sh` grows an `anthropic` leg in its
  surface vocabulary (`chat|responses|anthropic|all`), each leg demanding
  its own key up front as the script already does; there is no Anthropic
  key in the development environment, which is precisely why the offline
  suite must carry the whole correctness burden.

### Packaging (the `c44770` outcome)

`crates/transync-anthropic` **publishes**. Per the weld in
`crates/transync/tests/workspace_publication.rs`, adding the member turns
that test red until, in the **same commit**: the crate joins
`PUBLISHED_MEMBERS`, `docs/project/release-checklist.md` step 19 names it
(after `transync`, beside `transync-openai` — it depends on `transync`
only), the root `[workspace.dependencies]` gains
`transync-anthropic = { version = <workspace version>, path = "crates/transync-anthropic" }`,
and the member manifest declares `version.workspace = true` with
`publish` absent. That red-then-green is the weld working as designed, not
an obstacle. Dependency set mirrors `transync-openai`'s exactly —
`transync`, `serde`, `serde_json`, `thiserror`, `tracing`, `async-trait`,
`secrecy`, `url`, `reqwest`, `tokio` (all `workspace = true`); dev-deps
`transync-core` + `transync-syntax` for the live-smoke fixture-shape pin,
declared with the same OI-0027 honesty note. **No edge touches
`transync-syntax` or `transync-wasm` outside that dev-dep**, so the wasm
gate is untouched by construction.

## Why

The owner decided 2026-08-06; this record decides *how*. One sentence of
substance: `dynwebserver` and `resp-translator` consume transync through
the `Translator` seam, and a second real provider is both the feature
(Anthropic-backed translation) and the proof that ADR-0002's seam holds —
the shared prompt/schema/parse surface is reused unchanged, the taxonomy
absorbs a second provider's vocabulary without flattening, and the core
trait does not move.

## Alternatives Considered

- **A shared HTTP/transport crate (or hoisting transport into core).**
  Rejected. Core is HTTP-free by ADR-0002 — `reqwest` must not enter it,
  dev-dependencies included. A `transync-provider-http` crate would add a
  publication-roster member and a semver surface to carry ~300 lines whose
  halves genuinely differ per provider (auth header, protocol header, error
  envelope, overload signal). Duplication-with-intent is cheaper than a
  wrong abstraction between two data points; `transync-local-llama` (out of
  scope) is the natural third data point to revisit on.
- **Forced tool-use as the schema mechanism.** Rejected. This API's
  structured outputs (`output_config.format`) are the same contract shape
  the OpenAI adapter already relies on — schema in, guaranteed-conformant
  JSON text out. Tool-use would wrap the answer in `tool_use` blocks,
  change the stop-reason vocabulary mid-flight (`tool_use` becomes a
  *success* signal), and buy no additional strictness.
- **Sending `thinking: {type: "disabled"}` to reclaim the token budget.**
  Rejected: rejected-with-400 on part of the current model family and
  effort-gated on another part, so it would make the adapter's validity a
  function of the model name — a per-model capability table this crate
  otherwise refuses to carry. Not sending the parameter is valid everywhere.
- **A model-capability allowlist (structured-outputs support, effort
  support).** Rejected — rots with every model launch; the provider's own
  400 is precise, arrives once (terminal), and names the real authority.
- **`tokenizer_hint() → None`.** Rejected: `None` means core guesses from
  the advisory `model_id` string; a flat `Cl100kBase` is the same numeric
  outcome for `claude-*` names but as this crate's *stated* approximation,
  which is what the trait docs ask a non-OpenAI provider to do. Budgets are
  soft caps; a deliberate approximation is sufficient and honest.
- **Folding the fork into this record.** Rejected — two defensible shapes
  with materially different consequences (a vocabulary the two sibling
  consumers must absorb vs. a deferred re-litigation) is exactly what the
  owner-fork channel exists for.

## Rules the implementer must not violate

1. **No adapter-side retry.** Classification is the adapter's whole retry
   contribution; ADR-0009/§5's bounded verbatim resubmission lives in core
   and must not be doubled.
2. **Schema enforcement is unconditional.** Every request carries
   `output_config.format` with the shared schema object; no prompt-only
   JSON mode exists, even as a fallback.
3. **Tier-(b) discipline.** Prompt assembly, schema objects and response
   parsing come from `transync::llm::prompt` / `profile::render_prompt_body`
   — called, never reproduced; prompt *text* never asserted. The adapter
   adds no instruction text of its own around unit payloads: the
   untrusted-data framing (invariant 7) is the shared prompt's job.
4. **Structure stays the application's** (invariant 2): the adapter never
   rewrites unit ids, payloads, or constraint hints on either direction of
   the wire — validation of the model's structural obedience is core's, and
   the adapter's job ends at faithful transport plus honest classification.
5. **Config identity is fixed at construction.** All env reads in
   constructors only; every output-affecting axis a field read by both
   `fingerprint()` and the request path; the timeout and the version
   constant stay out of the fingerprint for the reasons recorded above.
6. **Core and the gates do not move** — except the fork's variant if Option
   A is chosen, nothing in `transync-core`, `transync-syntax`,
   `transync-wasm` or the facade changes; `transync-syntax` keeps no
   `[features]` and no core dep; §0 / `public_surface.rs` untouched.
7. **`stop_reason` before content**, `stop_details` only under refusal, and
   every provider-controlled string that can reach stderr/logs capped at
   the 512-byte diagnostic rule.
8. **ADR-0020 stands.** Cache identity defends against accidental collision
   only; do not design fingerprint or key handling around an attacker.
9. **No default-run test may depend on a key** — present or absent. The
   live path is double-gated; the gate predicate is offline-tested.
10. **Cancellation answers come from the biased race** — never check-then-
    send, and the shared run token is observed, never cancelled.

## Out of Scope

- `transync-local-llama` (owner: later).
- Provider capability descriptors (Type 3, evidence-gated) — but note: **this
  crate landing satisfies half of the `provider-capability-set` revisit
  condition** (`docs/backlog.md`; DCR-0009 YAGNI list). SL-119 records the
  landing there so the item wakes as designed.
- Streaming (both adapters are non-streaming by design; the 16 K default
  ceiling is chosen to keep that sound).
- CLI integration (`transync-cli` provider selection). The reference CLI
  stays OpenAI-backed; a `--provider` axis is real design work (env surface,
  key resolution, §6 argument contract) and is not smuggled in here. Wants
  its own ticket if the owner wants it.
- Anthropic-accurate token estimation (a new `TokenizerHint` variant plus a
  core estimator); the documented approximation stands until batching
  evidence says otherwise.
- Prompt caching, batch API, and any beta-headed feature of the provider —
  the adapter speaks the plain GA Messages surface only.

## Implementation Slices

Each slice lands independently, in order, gates green (`cargo test
--workspace -- --test-threads=4`, clippy `-D warnings`, fmt, the wasm gate,
the rustdoc gate — the tracked pre-commit hook runs all of them; test
results measured from cargo's own exit code).

- **SL-115 — crate skeleton, config identity, and the publication weld.**
  The member exists and publishes: manifest, roster
  (`workspace_publication.rs` `PUBLISHED_MEMBERS`), release-checklist step
  19, root `[workspace.dependencies]` entry — one commit, red-then-green on
  the weld. `TransyncAnthropic` with `new`/`try_new`/`from_env`, its own
  `ModelId` (§7 parse rules), `ConfigError`, `with_effort`, `with_timeout`
  + `request_timeout()`, the cleartext warning, `fingerprint()`,
  `tokenizer_hint()`. No `Translator` impl yet. contracts.md gains §8's
  constructor half. Tests mirror the OpenAI config/fingerprint suite.
- **SL-116 — the wire shape, offline-pinned.** The `client` flow plus its
  private per-concern modules: translation and extraction request builders
  over the tier-(b) functions (top-level `system`, single user turn,
  `output_config.format` + the schema-profile pass, required `max_tokens`
  with `DEFAULT_MAX_OUTPUT_TOKENS`, `output_config.effort` placement, the
  `anthropic-version` constant), and the envelope reader (stop-reason
  table minus the classification wiring, first-text-block extraction,
  `stop_details` guard, empty-content refusal). Everything pinned offline.
- **SL-117 — transport, classification, and the terminal-cause map.**
  `post_json` with `x-api-key` + version header, both body caps, the
  loopback-server tests; the classify table exactly as recorded above,
  529 named and tested, `retry-after` in both forms; the stop-reason
  causes wired to variants, including the fork's resolution as the owner
  decided it (Option A additionally lands the core variant, its stable
  code, the §1 row and the `error_taxonomy.rs` weld rows in this same
  slice — shared files, one commit). contracts.md gains §8's
  provider-signals half.
- **SL-118 — the `Translator` implementation.** `translate_batch` and
  `extract_glossary` wired through the shared parsers, the biased
  cancellation race on both, the post-parse `max_terms` truncation,
  `Ok(Some(_))`-even-when-empty extraction semantics; the end-to-end
  offline test drives `transync::translate` over the adapter against a
  canned envelope from loopback, proving the full stack keyless.
- **SL-119 — the live half and the records.** `tests/live_smoke.rs` with
  the two-gate pattern and the offline fixture-shape + gate-predicate
  pins; the `anthropic` leg in `scripts/smoke-live-gate.sh`; the
  `provider-capability-set` wake-up note in `docs/backlog.md`;
  `status.md` / `phase-state.yaml` close-out; CHANGELOG under
  `[Unreleased]`.

## Verification

- `cargo test --workspace -- --test-threads=4` — exit code read from cargo
  itself (baseline at commissioning: 828 passed / 0 failed; DCR-0028's
  slices have since raised it — re-baseline from the current HEAD before
  SL-115).
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, the
  wasm32 gate and the rustdoc gate (the tracked pre-commit hook).
- The offline suite listed per slice; `workspace_publication.rs` green with
  the new roster; `error_taxonomy.rs` green (and extended, under fork
  Option A).
- `scripts/smoke-live-gate.sh anthropic` — deferred until an
  `ANTHROPIC_API_KEY` exists; recorded as the standing gap in SL-119's
  close-out (the offline canned-envelope end-to-end is the shipped
  evidence until then).

---

## Implementation notes (appended 2026-08-10, SL-115)

**The root `[workspace.dependencies]` entry is NOT added, and the *Packaging*
section above is wrong to require it.** That paragraph lists four things the
`c44770` weld turns red until they land together; three are right (the
`PUBLISHED_MEMBERS` roster, release-checklist step 19, `version.workspace =
true` with `publish` absent), and the fourth inverts the weld it cites.
`crates/transync/tests/workspace_publication.rs`'s
`every_internal_edge_is_declared_through_the_workspace` asserts the root
table's internal entries and the edges members actually declare are the **same
set**, in both directions — so an entry for a member nothing depends on fails
as `declared but unused`. Nothing in the workspace depends on
`transync-anthropic` (CLI integration is out of scope, above), so adding the
entry would make the weld red rather than green.

The crate publishes anyway, because publishability is a property of a package's
**outgoing** edges: `transync-anthropic` depends on `transync` (plus
`transync-core` / `transync-syntax` as dev-only), each declared
`workspace = true` and each already carrying a `version` in the root table, so
`cargo publish` resolves every stripped `path` from the registry. The entry
lands the day a member depends on the crate, in that same edit — the root
manifest and release-checklist step 19 both say so in place.

**The schema-profile pass strips more than the extraction schema's
`maxItems`.** *Schema enforcement* names that keyword as "the one place the
shared schema uses such a keyword today"; `prompt::schema_object_for(Some(n))`
also stamps `minItems`/`maxItems` on the translation schema's `units` array,
which is the same array-count class the provider's dialect does not accept. The
mechanism the record designs is unaffected — a deterministic, crate-private
pass that removes a **named keyword set** recursively — and it covers both
schemas by construction; only the "one place" count was understated.

**The fork resolved to Option A** (`TranslatorError::ContextWindowExceeded`,
stable code `provider_context_window_exceeded`), landing in SL-117 with its §1
row and its `error_taxonomy.rs` weld rows, per the recommendation and the open
v0.4.0 window.
