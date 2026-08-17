---
type: DCR
title: Provider-neutral prompt assembly lifted into core, tokenizer hint, RTL pane direction, gated live smoke
description: Three surface/infra resolutions. (A) The provider-neutral prompt, schema-object, and response-parsing assembly moves from transync-openai into transync-core's new llm::prompt module, byte-identity pinned by goldens generated from the pre-lift client; a defaulted Translator::tokenizer_hint replaces the OpenAI-model-name guess, and model identity is documented as fingerprint-authoritative with TranslateOptions::model_id demoted to advisory. (B) The HTML bundle stamps per-pane dir="rtl" from a flag, a profile key, or a best-effort primary-subtag table; LTR emits nothing, so existing bundles stay byte-identical. (C) Live-endpoint evidence becomes one double-gated command with machine assertions over both API surfaces.
tags: [change, project-control, DCR-0015]
status: deprecated
---

# DCR-0015: Prompt lift + tokenizer hint + RTL pane direction + gated live smoke

- **Date:** 2026-08-03
- **Source:** OI-resolution wave **OI-2026-08**; design D2 (`Surface & Infra: OI-0029 / OI-0032 / OI-0030`); resolves OI-0029, OI-0030 and OI-0032. The "act only when provider #2 lands" deferral recorded in OI-0029 was explicitly overridden by the owner.
- **Affected ADRs:** `docs/decisions/0013-opaque-language-labels.md` (dated amendment — a presentation-layer direction *hint* is added over labels that stay opaque for every engine consumer); `docs/decisions/0002-http-free-core-with-translator-trait.md` and `docs/decisions/0003-cargo-workspace-with-provider-crates.md` (upheld — the lift moves code *toward* the HTTP-free core and needs no new crate in the dependency DAG); `docs/decisions/0010-reject-tokenizer-panic-fallback.md` (upheld — the new encoder paths keep the same "bundled asset is broken" panic contract)

## What Changed

### Part A — OI-0029: the provider-neutral assembly moves to `transync-core`

Roughly 800 lines of provider-*neutral* prompt/hint assembly lived inside
`transync-openai`. The judgment that decided the split line: the assembly
encodes the **validator's** contract, not OpenAI's. `build_user_prompt`'s
instruction text mirrors `validate::inline`'s gates and ADR-0009's retry
framing; the Structured Output *schema object* describes
`TranslationBatchResult`, the envelope `validate::schema` and the pipeline
consume. A provider that words those differently silently desynchronizes from
validation. The Chat/Responses dispatch and both request wrappers, by
contrast, are genuinely OpenAI-shaped.

- **New module `transync-core::llm::prompt`** (`src/llm/prompt.rs`, a
  file-as-module child of `llm`; `pub mod prompt;` in `src/llm.rs` — no
  `mod.rs`). Public surface for the translation contract: `SCHEMA_NAME`,
  `schema_object_for(Option<usize>)`, `build_user_prompt(&TranslationBatch)`,
  and `parse_batch_output(&str, &BatchId)`. The hint/envelope structs
  (`UserPromptPayload`, `UserPromptUnit`, `ConstraintHints`,
  `ListTopologyHint`, `ContextHints`, `RetryHint`, `ApiBatchOutput`,
  `ApiUnitResult`, `ApiOutputKind`) moved **private** — a second provider
  consumes the functions, not the intermediate structs, and widening
  visibility later is non-breaking. A separate `provider-common` crate was
  rejected: the code depends only on core types, and ADR-0003's DAG
  (`transync-openai → transync → transync-core`) already delivers it to every
  provider through the facade. Request-side assembly and response-envelope
  parsing live in one module because they are the two halves of one wire
  contract.
- **Error identity preserved.** Core constructs `TranslatorError` directly
  with the same variants and message strings the old
  `ProviderError → map_provider_error` path produced: serialization failure →
  `Other("user-prompt serialization failed: …")`, JSON parse failure →
  `MalformedResponse("model output_text was not valid JSON for the response
  schema: …")`.
- **`transync-openai` after the lift** keeps dispatch, transport, and both
  surface envelopes: `ModelId`, `ReasoningEffort`, `api_for_model` /
  `api_from_env_or_model`, the `call_*` functions (**signatures untouched** —
  they still carry `reasoning_effort: Option<ReasoningEffort>`), the
  `ChatRequest` / `ResponsesRequest` wrappers (which now embed
  `prompt::schema_object_for(...)` under `prompt::SCHEMA_NAME`), the response
  extractors with their refusal/`incomplete` handling, `post_json`'s 32 MiB
  ceiling, and endpoint building. `translate_batch`, `fingerprint`,
  `with_reasoning_effort`, and `from_env` are unchanged.

**Byte-identity pinning (the gate for the whole lift).** The lift is a
refactor, so the prompts must not move a byte — and a same-wave move cannot be
diffed against a released artifact. Four goldens were therefore generated
from the **pre-lift** `client.rs` as the implementation's first action, before
any code moved, and now live at
`crates/transync-core/src/llm/prompt/golden/` (`user_prompt_first_dispatch.json`,
`user_prompt_retry.json`, `schema_two_units.json`, `schema_unbounded.json`),
consumed via `include_str!`:

- Fixture A is the provider's `fixture_batch()` **extended** with a populated
  `BlockContext` (document title, section-path heading, both neighbor
  summaries) and a profile carrying a glossary entry, so every hint branch
  serializes; fixture B adds `RetryContext { attempt: 2, rejected_by:
  Some(PerKindShape), reason: Some(...) }`.
- `golden_user_prompt_first_dispatch`, `golden_user_prompt_retry`, and
  `golden_schema_objects` assert byte equality, and stay in the tree as the
  permanent regression pin: an intentional prompt change must now consciously
  regenerate a golden.
- Two serialization-order hazards the pin converts from silent to loud:
  `Serialize` struct **field declaration order** had to be preserved verbatim
  through the move (serde emits in declaration order), and
  `schema_object_for`'s output relies on `serde_json`'s default sorted-map
  ordering — enabling `preserve_order` anywhere in the workspace would change
  the schema bytes.
- The independent wave review rebuilt the pre-lift assembly from the original
  `client.rs` and confirmed the lifted functions' output is byte-identical to
  it, so the goldens are corroborated rather than self-certified.

**Shape issue 1 — tokenizer selection.** `batch::encoder_for` guessed the
tiktoken encoder from an OpenAI-shaped model name, which is meaningless for a
non-OpenAI provider. New `#[non_exhaustive] llm::TokenizerHint { O200kBase,
Cl100kBase }` plus a **defaulted** `Translator::tokenizer_hint() ->
Option<TokenizerHint>` (default `None`). `batch::encoder_for_hint(hint)` loads
the named encoder and `batch::resolve_encoder(hint, model_id)` holds the
single "hint beats heuristic" rule, so the packer and the output-budget
preflight can never disagree. `encoder_for` stays as the documented **legacy**
fallback (unknown names → `cl100k_base`, adequate because batch budgets are
soft estimate caps, not enforcement). `TransyncOpenAI` overrides the method
via `client::tokenizer_hint_for_model`, mirroring core's rules exactly, so CLI
runs — which hand the same resolved model string to both `opts.model_id` and
the provider — pack **identically** to before; stubs keep the default `None`
and therefore the legacy path. The hint is deliberately **excluded** from
`fingerprint()` / `CacheKey`: batching shape never participates in content
identity (same precedent as `ProfileBatching`).

**Shape issue 2 — model-identity dual-homing, resolved by documentation.**
`opts.model_id` and the provider's own `ModelId` were two homes for one fact.
Removing `TranslateOptions::model_id` / `CacheKey::model_id` was considered
and **rejected**: `encoder_for`'s fallback needs *some* model string for
translators that declare no hint; reshaping `CacheKey` belongs to the
disk-backed-cache design that owns OI-0017's remaining scope; and safety is
already guaranteed, because `CacheKey.provider_fingerprint` embeds the
provider's own model, so a desynced `opts.model_id` can only
**over-distinguish** — the safe direction. The resolution is a single
documented authority: **the `Translator` instance is authoritative** (its
`fingerprint()` covers the provider's real model, endpoint, and surface), and
`TranslateOptions::model_id` is **advisory** — a tokenizer fallback label and
one cache-key axis. A mismatch is safe but wasteful. Recorded in the rustdoc
of both `TranslateOptions::model_id` and `transync_openai::ModelId`, and in
contracts.md §1.

### Part B — OI-0032: RTL direction for the rendered bundle

- **Pane-level `dir`, not per-block.** `dir` inherits, and per-block direction
  would need per-block language metadata that does not exist (the alignment
  map is deliberately presentation-free). The sync engine is unaffected: it
  syncs on `offsetTop`/`offsetHeight`, which is direction-agnostic — **no
  `sync.js` change, both byte-mirrored copies untouched.**
- **Precedence: `--target-direction` flag > profile `[render].target_direction`
  > `auto`.** New `ProfileRender { target_direction: Option<String> }` on
  `ProfileMetadata.render` (serde-defaulted, so old TOML profiles and old
  JSON dumps load unchanged); unknown values raise a load warning and
  normalize to `None` (= auto), matching `default_table_strategy`'s style.
  `"render"` joins the `TOP` known-key list with its own `RENDER` key set.
  The new CLI arg has **no clap default**, so "flag absent" stays
  distinguishable from an explicit `auto` — the lesson from DCR-0012's
  batching flags.
- **`auto` is a best-effort primary-subtag table.** The label's first
  `-`/`_`-separated token, ASCII-lowercased, is matched exactly against 15
  entries: `ar arc ckb dv fa he iw ji nqo ps sd syr ug ur yi`. The
  BCP-47 grandfathered legacy codes `iw` (→ he) and `ji` (→ yi) are included;
  `ku` is **deliberately excluded** — Kurmanji is Latin-script and Sorani is
  already covered by `ckb`.
- **Emission rule: RTL stamps ` dir="rtl"`, LTR emits *nothing*.** LTR is the
  HTML default, and emitting nothing is what keeps today's ko/ja/en bundles
  byte-identical. An explicit `--target-direction ltr` on an `ar` run
  therefore renders LTR by *absence* of the attribute — deliberate, and what
  the override test asserts. The attribute is a fixed literal, never caller
  text, so it needs no escaping.
- **The source pane auto-resolves too**, from the already-resolved source
  label (explicit `--source-language`, else the detected language, else empty
  → no attribute). There is no `--source-direction` flag in this wave: the
  source label is machine-resolved and the table covers it. A trivial
  extension point if it is ever needed.
- **Bundle only.** `out.md` is untouched (Markdown has no `dir`) and the
  alignment map is untouched — direction is presentation-layer, so the
  **schema stays `1.1.0`**. `<html>` keeps only the target `lang`: stamping
  `dir` there would flip the themer and legend chrome, and the panes are the
  content.
- **Plumbing.** New CLI `direction.rs` (file-as-module) owns
  `DirectionMode { Auto, Ltr, Rtl }`, the subtag table, `dir_attr(mode,
  label)`, and `resolve_mode(flag, profile_value)`.
  `output::html_bundle_files` gained `source_dir_attr` / `target_dir_attr`
  parameters after the two language parameters, feeding two new
  `{{TRANSYNC_SOURCE_DIR_ATTR}}` / `{{TRANSYNC_TARGET_DIR_ATTR}}` placeholders
  on the pane divs in `index.html.tpl`. The `.tpl` is not part of the
  `sync.js` / `purify.min.js` byte-mirror set, so a template-only change is
  drift-safe.
- **Byte-identity fix from the wave review.** The first implementation also
  added an explanatory HTML comment to `index.html.tpl`. That comment ships
  *inside every generated `index.html`*, so it broke the guarantee this design
  rests on — that an LTR (ko/ja/en) bundle is byte-identical to the pre-wave
  one. The review caught it and the comment was removed; the template's only
  change is the two placeholder insertions, and the byte-stability control run
  in `cli_rtl_target_language_stamps_target_pane` now passes for the right
  reason. The equivalent explanation lives in the workspace dev shell
  (`web/index.html`) instead, which is a hand-edited harness with no language
  flow and no substitution machinery — RTL preview there stays a manual
  `dir="rtl"` edit.

### Part C — OI-0030: automated-but-gated live-endpoint evidence

The repo has no CI, so the resolution is **one command, machine-asserted,
double-gated** — replacing "run a script by hand and eyeball the browser".
*Scheduled* CI live smoke remains explicitly out of scope; revisit if CI ever
lands.

- **`crates/transync-openai/tests/live_smoke.rs`** carries two `#[ignore]`d
  `tokio` round-trips, one per API surface:
  `live_chat_surface_round_trip` (`TRANSYNC_LIVE_SMOKE_CHAT_MODEL`, default
  `gpt-4o-mini` → Chat Completions) and `live_responses_surface_round_trip`
  (`TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL`, default `gpt-5-mini` → Responses).
  `TRANSYNC_OPENAI_BASE_URL` is honored, but the provider is constructed
  explicitly rather than via `from_env` so the *cheap* default model is used
  instead of `from_env`'s `gpt-5-chat-latest`.
- **Two independent gates, both required.** The `#[ignore]` attribute keeps
  them out of `cargo test --workspace`; a self-skip guard requires
  `TRANSYNC_LIVE_SMOKE=1` **and** a non-empty `OPENAI_API_KEY`, so even a
  blanket `--include-ignored` run without the opt-in passes as a no-op skip
  instead of failing or touching the network. The decision is factored into a
  pure `decide_gate(opt_in, api_key)` predicate that is unit-tested **offline**
  (`gate_requires_both_opt_in_and_api_key`). No `std::env::set_var` anywhere
  (unsafe under edition 2024 and racy across test threads) — surface selection
  is done by model choice, not by the `TRANSYNC_OPENAI_API` override.
- **Assertions are structural, not textual**, so they survive model
  nondeterminism: `translate(...)` returns `Ok` (any transport/schema/auth
  drift fails here, with every validation layer as the assertion);
  `validation_summary.total_units` equals the fixture's pinned unit count and
  the alignment-map block pairs match the parsed source block count;
  `fallback_source < total_units` (at least one unit genuinely translated —
  asserting **zero** fallbacks is deliberately avoided, since a single unit
  degrading to honest source is within contract and must not fail the gate);
  and `translated_markdown` is non-empty and differs from the source.
- **Fixture-count correction (recorded).** D2 described the fixture
  (`# Smoke` heading + one paragraph + a two-item bullet list) as **3** units.
  It is **4**: the IR has no list-*container* kind — each `- item` is its own
  `list-item` unit (DCR-0007's leaf-block model). The constant is `4`, and
  `live_source_fixture_has_the_expected_shape` pins the heading + paragraph +
  two list items **offline** so a fixture edit fails locally instead of
  mid-flight against a paid endpoint.
- **`scripts/smoke-live-gate.sh`** (mode 0755) is the human opt-in wrapper: it
  sets `TRANSYNC_LIVE_SMOKE=1` for its own child only, takes
  `chat|responses|all` (default `all`, ~2 tiny mini-model calls), runs with
  `--ignored --test-threads=1 --nocapture` (sequential for politeness and
  deterministic logs; the workspace `--test-threads=4` rule is a *cap*, so 1
  complies), refuses to run without `OPENAI_API_KEY`, and cross-references
  `scripts/smoke-live.sh` for the interactive browser check. The existing
  `smoke-live.sh` / `smoke-live-long.sh` are untouched and remain the
  interactive path.
- **No live call was made in this wave.** The artifacts are verified by
  `cargo test -p transync-openai --no-run` (the gated binary compiles), the
  normal suite (both tests listed as `ignored`, zero network), and
  `bash -n scripts/smoke-live-gate.sh`.

## Why

**Part A** removes the only structural obstacle to a second provider: a
provider crate that has to re-derive the validator's wording is a provider
crate that will drift from it, and the drift is silent — validation rejects
output the provider was never told to produce. Doing the lift *before*
provider #2 (the owner's override of OI-0029's deferral) means the boundary is
proven by a golden pin against a known-good artifact, rather than negotiated
against a half-written second implementation. The two shape issues rode along
because they are the same seam: the engine was guessing a provider's tokenizer
and holding two copies of its model identity.

**Part B** closes a rendering defect that made the demo bundle wrong for whole
language families, without breaching ADR-0013. The label stays opaque
everywhere it matters; only the bundle emitter — the one place that is already
a presentation layer — consults a hint table, and the hint is override-able
and invisible to prompt, cache, and alignment identity.

**Part C** converts "someone should run the script" into a single command
whose failure is a machine assertion over the full validation stack. It cannot
run by accident, it cannot spend tokens without an explicit opt-in, and its
one fragile input (the fixture's unit count) is pinned offline.

## Affected Areas

- `crates/transync-core/src/llm.rs` — `pub mod prompt;`, `TokenizerHint`,
  defaulted `Translator::tokenizer_hint`
- `crates/transync-core/src/llm/prompt.rs` (new) +
  `crates/transync-core/src/llm/prompt/golden/*.json` (new) — moved assembly,
  schema object, envelope parsing, migrated tests, golden pins
- `crates/transync-core/src/batch.rs` — `encoder_for_hint`, `resolve_encoder`,
  `BatchBudget.tokenizer_hint`, `output_budget_warnings` hint parameter,
  `encoder_for` legacy-heuristic docs
- `crates/transync-core/src/unit.rs` — `build_batches` tokenizer-hint
  parameter forwarded into `BatchBudget`
- `crates/transync-core/src/pipeline.rs` — `run_pipeline` queries
  `translator.tokenizer_hint()` once and forwards it to both the packer and
  the preflight
- `crates/transync-core/src/lib.rs` — `TokenizerHint` re-export, `model_id`
  demotion rustdoc
- `crates/transync-core/src/profile.rs` — `ProfileRender`,
  `ProfileMetadata.render`, `[render]` load validation + unknown-key warnings
- `crates/transync-openai/src/client.rs` — moved block deleted, wrappers now
  embed `prompt::*`, `tokenizer_hint_for_model`
- `crates/transync-openai/src/lib.rs` — `tokenizer_hint()` override,
  `ModelId` authority note
- `crates/transync-openai/tests/live_smoke.rs` (new) +
  `scripts/smoke-live-gate.sh` (new, 0755)
- `crates/transync-cli/src/direction.rs` (new),
  `crates/transync-cli/src/main.rs` (module registration),
  `crates/transync-cli/src/translate_cmd.rs` (`--target-direction`, mode
  resolution), `crates/transync-cli/src/output.rs` (`html_bundle_files` +
  2 params), `crates/transync-cli/web/index.html.tpl` (two placeholders)
- `crates/transync-core/profiles/default.toml`,
  `crates/transync-cli/profiles/default.toml` — commented `[render]` section
- `web/index.html` — dev-shell comment documenting manual RTL preview
- `docs/architecture/contracts.md` §1 / §2 / §6;
  `docs/architecture/mvp-scope.md` (RTL limitation → shipped behavior)

### Discriminating tests

Lift/identity: `golden_user_prompt_first_dispatch`, `golden_user_prompt_retry`,
`golden_schema_objects`, plus the migrated
`user_prompt_serializes_hints_and_injection_guard`,
`user_prompt_serializes_retry_hint`,
`user_prompt_gains_destination_rule_by_default`,
`user_prompt_omits_destination_rule_when_urls_localizable`,
`schema_object_stamps_min_max_items`,
`schema_object_without_count_has_no_item_bounds`,
`schema_unit_required_lists_all_properties`, `parse_batch_output_happy_path`,
`parse_batch_output_rejects_invalid_json`. Provider-side request-body,
endpoint, and envelope-extraction suites stayed put and stayed green.

Tokenizer: `encoder_for_hint_maps_both_variants`,
`resolve_encoder_prefers_the_hint_over_the_model_name`,
`resolve_encoder_falls_back_to_the_model_heuristic`,
`tokenizer_hint_matches_dispatch_generations`.

RTL: `auto_rtl_table`, `expressive_labels_stay_ltr`, `explicit_mode_wins`,
`resolve_mode_precedence`, `render_target_direction_loads_and_normalizes`, and
the CLI subprocess trio `cli_rtl_target_language_stamps_target_pane` (which
also carries the ko byte-stability control),
`cli_target_direction_flag_overrides_table`,
`cli_profile_render_direction_applies_and_flag_beats_it`.

Live smoke: `gate_requires_both_opt_in_and_api_key` and
`live_source_fixture_has_the_expected_shape` run offline in the normal suite;
`live_chat_surface_round_trip` and `live_responses_surface_round_trip` are
`ignored` until double-gated.

## Migration / Follow-up

Breaking / wire changes ride the **still-open 0.2.0 window**:

- `unit::build_batches(doc, opts)` → `build_batches(doc, opts,
  tokenizer_hint: Option<TokenizerHint>)`; in-tree test call sites pass
  `None`.
- `batch::BatchBudget` gains `pub tokenizer_hint: Option<TokenizerHint>`
  (struct-literal constructors updated).
- `batch::output_budget_warnings(batches, model)` → `(batches, model,
  tokenizer_hint)`; `run_pipeline` forwards the same hint it gave the packer.
- `prompt::parse_batch_output` is now **public** and takes `&BatchId` instead
  of `&TranslationBatch` (it only ever read `batch.batch_id`). It was private
  to the provider before, so this is new-API shape rather than a break of
  shipped API.
- `ProfileMetadata` gains `render: ProfileRender` — serde-defaulted, but its
  serialized JSON now carries a `"render"` object.
- `transync-cli::output::html_bundle_files` gains two trailing `&str`
  parameters (`source_dir_attr`, `target_dir_attr`).
- Additive and non-breaking for external implementors: the defaulted
  `Translator::tokenizer_hint`, the new `transync::llm::prompt` module, and
  the `TokenizerHint` re-export.
- `CacheKey`, `ProviderFingerprint`, `VALIDATION_SCHEMA_VERSION`, and the
  alignment-map schema (`1.1.0`) are all **unchanged**. Existing ko/ja/en
  bundles are byte-identical.

Follow-ups deliberately left open:

- **Release gate.** Before tagging a release that touches `transync-openai`,
  `llm::prompt`, the output schema, or batching, run
  `scripts/smoke-live-gate.sh` and record the date + models used in the
  CHANGELOG entry. Recorded in the Developer Guide's smoke-script table and in
  `status.md`'s next actions; there is no dedicated release-checklist document
  to host it yet.
- **Live-smoke model defaults** (`gpt-4o-mini` / `gpt-5-mini`) encode today's
  cheap-model guesses. If either identifier is retired the gate fails loudly
  at the provider; the env overrides are the fix, and they are documented in
  the script header so a retired model is not misread as a code regression.
- **Hint visibility.** `llm::prompt`'s hint/envelope structs stay private
  until provider #2 demonstrates a need; widening is non-breaking.

### Note (2026-08-06) — the release-checklist gap is closed

The **Release gate** follow-up above ends with "there is no dedicated
release-checklist document to host it yet". There is now:
`docs/project/release-checklist.md` (backlog item `release-checklist-document`),
a living document under `BL-2026-07-B` that owns the whole tagged-release
sequence — preflight and CHANGELOG-completeness audit, the standing gates
(`scripts/smoke.sh` including the rustdoc gate and the wasm size budget, the
Playwright suite, the `resp-translator` path-dependency check), this gate as
steps 10–12, the CHANGELOG promotion, the single-line workspace version bump,
the record updates, and the annotated tag.

The gate rule itself is **unchanged** — same trigger set (`transync-openai`,
`llm::prompt`, the output schema, batching), same script, same requirement to
record date + models in the release's CHANGELOG entry. What changed is where
it is written down: it was hosted informally in the Developer Guide's
smoke-script table and in `status.md`'s next actions, both of which now point
at the checklist instead. Two things the v0.2.0 release proved belong on a
written list rather than in memory are on it: the **CHANGELOG reference-link
stanza** (v0.2.0 shipped with `[Unreleased]` still comparing `v0.1.0...HEAD`
and `[0.2.0]` undefined, so the version heading rendered as literal text until
a follow-up commit fixed it) and the **completeness audit** against
`git log v<prev>..HEAD` (which restored 19 missing entries at v0.2.0 prep).

### Note (2026-08-06) — "batching shape never participates" is narrowed

This record's *Shape issue 1* justifies excluding `TokenizerHint` from
`fingerprint()` / `CacheKey` with the blanket claim that **batching shape never
participates in content identity**. Review-0001 finding R0001-0004 showed the
blanket does not fit: `[batching].target_output_tokens` is the one
`ProfileBatching` field that leaves the engine as a real request parameter
(`max_completion_tokens` / `max_output_tokens`), so the soft-estimate-cap
reasoning quoted here cannot carry it.

The exclusion of the hint itself is **unchanged and still correct** — the
soft-cap argument was always the right one for an estimation-only field. What
changed is the scope of the sentence: `docs/architecture/contracts.md` §1 now
reads "does not participate", confines the soft-cap justification to the
estimation-only fields, and adds *Output ceiling and cache identity*, which
exempts the ceiling on a narrower argument (it is a stop, not a content
parameter) plus a provider obligation. Read the contract, not this sentence,
for the current rule.

### Note (2026-08-06) — one of the two profile paths above no longer exists

*Affected Areas* lists both `crates/transync-core/profiles/default.toml` and
`crates/transync-cli/profiles/default.toml` as carrying the commented
`[render]` section. Only the `transync-core` half was ever `include_str!`d, and
the CLI-side file — referenced by no source, test or script — was deleted on
2026-08-06 (Review-0001 finding R0001-0041) after its comments had already
drifted from the core copy. The commented section this record added is
unchanged and still lives in the surviving file.

### Note (2026-08-06) — the user-prompt goldens moved once, deliberately

*Byte-identity pinning* describes fixture A as carrying "both neighbor
summaries", which was the whole of what the wire said about a neighbor at the
time. Review-0001 finding `R0001-0024` (ticket `53d4956f`) put the rest of the
typed context on the wire: a `section_path` entry became `{level, text}` and
each neighbor gained a `preceding_kind` / `following_kind` label. Both
`user_prompt_*.json` goldens were regenerated through the record's own
`regen_prompt_goldens` helper and differ from the pre-lift bytes **only** in the
`context` object — the instruction text, the schema goldens, and every other
unit field are untouched.

The pin itself works exactly as this record intended: the change had to be
declared by regenerating, not absorbed silently.
