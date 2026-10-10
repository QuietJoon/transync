---
type: DCR
title: A model's generation is a floor, not a roster
description: "transync-openai chose the Responses surface and the o200k_base encoder by matching model-name prefixes (gpt-5, o1, o3, o4), so every later family fell back to Chat Completions and cl100k_base. Surface dispatch and both encoder guesses now read the generation number a model name declares — any o<N>, and gpt-<N> from generation 5 up, reach Responses and o200k_base without being listed. A chat snapshot stays on Chat Completions in every generation, and gpt-4o stays on o200k_base by name."
tags: [provider, openai, batching, DCR-0059, OI-0029, SCN-10, SCN-12]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T00:00:00Z
status: stable
---

# DCR-0059: A model's generation is a floor, not a roster

- **Date:** 2026-10-10.
- **Source:** a working-tree change whose decision was never written down,
  found by `/reconcile-docs` on 2026-10-06 (ticket `e17432c0`: six citations of
  DCR-0059, three of them `TRACE:` anchors, and no record behind them), and
  committed on 2026-10-07 as `c6df400` (`transync-openai`) and `d2a947c`
  (`transync-core`'s twin). This record states the generation-floor rule those
  commits ship. The core twin's test and the `transync-openai` side's
  cross-reference, both named below, land with this record.
- **Affected documents:** `docs/architecture/contracts.md` §7 (*Dual-API
  dispatch*) and the Developer Guide's dispatch and batching tables state the
  rule.
- **Behaviour changed:** yes, for the model names the old prefixes did not
  match. A `gpt-<N>` with N ≥ 6, and an `o<N>` the prefixes `o1` / `o3` /
  `o4` did not cover (`o2`, `o5`, `o6-pro`, …), is now counted with
  `o200k_base` instead of `cl100k_base` and — unless its name contains `chat`
  — dispatches to Responses instead of Chat Completions. Every name the old
  rule sent to Responses or to `o200k_base` still goes there. See *Identity*
  for the cache consequence.

**Numbering note.** Next free number after DCR-0058.

## What was wrong

Two functions in `transync-openai` read a model name for unrelated reasons,
and both recognised families by prefix:

- `client::api_for_model` chose the HTTP surface: a name containing `chat` →
  Chat Completions; `o1`, `o3` or `o4` → Responses; `gpt-5` → Responses;
  anything else → Chat Completions.
- `tokenizer::tokenizer_hint_for_model` chose the encoder batch packing counts
  with: `gpt-4o`, `gpt-5`, `o1`, `o3` or `o4` → `o200k_base`; anything else →
  `cl100k_base`.

`transync-core`'s `batch::encoder_for`, the fallback for a provider that
declares no tokenizer hint, carried the same five prefixes.

A list of shipped families is out of date the day a new one ships. `gpt-6`
matched none of the prefixes, so it reached Chat Completions — the surface a
family past GPT-5 is least likely to accept — and was counted with the older
vocabulary. Neither failure announces itself. The request is well-formed, so
the operator sees a provider-side rejection rather than a dispatch error; and
a wrong token count is not an error at all, because batch packing trusts it.

## The decision

Read the generation number, and treat it as a floor.

`generation_after(model, prefix)` returns the leading digit run that follows
`prefix` in the lower-cased name: `gpt-` in `gpt-5.6-terra` yields 5, `o` in
`o4-mini` yields 4, and `gpt-` in `gpt-oss-120b` yields nothing. Only the
leading integer is read, so a point release rides its major generation. A
digit run too long for `u32` saturates to `u32::MAX` instead of failing to
parse: an absurd generation is still a later one, and a parse failure would
fall back toward the older surface and encoder, which is the wrong direction.

Surface dispatch (`api_for_model`), in order:

1. A name containing `chat` → Chat Completions. Checked first, so it holds in
   every generation: `gpt-6-chat-latest` is a chat snapshot exactly as
   `gpt-5-chat-latest` is.
2. Any `o<N>` → Responses.
3. `gpt-<N>` with N ≥ `FIRST_RESPONSES_GPT_GENERATION` (5) → Responses.
4. Anything else → Chat Completions: `gpt-4o` (generation 4), `gpt-4.1`,
   `gpt-3.5-turbo`, `gpt-oss-120b` (no generation).

Encoder (`tokenizer_hint_for_model`): `o200k_base` for `gpt-4o` — named,
because its `o` is a product suffix rather than a generation — for any
`o<N>`, and for `gpt-<N>` with N ≥ `FIRST_O200K_GPT_GENERATION` (5);
`cl100k_base` otherwise. The `chat` rule is deliberately not repeated here: a
`*-chat-*` alias rides Chat Completions but is still an o200k-era model.

The surface and the encoder share one `generation_after` (re-exported
crate-privately from `client`), so the two readings of a name cannot disagree
about which generation it declares. They stay two functions, because they
answer two questions.

`TRANSYNC_OPENAI_API` and `TransyncOpenAI::with_api` still override the
surface for a model this rule reads wrongly. Nothing about the override
changes.

## The core twin

`transync-core` depends on no provider crate, and that absence is the
layering, so `batch::encoder_for` keeps a private copy of `generation_after`
and the same o200k rule rather than importing either. Each side's doc names
the other — the `transync-openai` side's sentence lands with this record —
and a change to one is made to both. If they ever disagree, the provider's
answer wins at runtime: `batch::resolve_encoder` consults a declared
`TokenizerHint` first and calls `encoder_for` only when there is none. All
three shipped adapters declare one — `transync-openai` through
`tokenizer_hint_for_model`, `transync-anthropic` and `transync-google` as
`cl100k_base` — so the twin governs only a `Translator` that declares no hint.

## Identity

`TransyncOpenAI`'s provider fingerprint names the API surface it calls
(`fingerprint_covers_model_base_url_and_api`). A model whose surface moves
under this rule therefore lands in a new provider namespace: its cached
entries miss once and are translated again. The encoder is not a fingerprint
axis; it moves where batches are cut for these model names, not which
namespace their entries live in. No schema version moves —
`ALIGNMENT_SCHEMA_VERSION`, `VALIDATION_SCHEMA_VERSION`,
`VALIDATION_REPORT_SCHEMA_VERSION` and `CACHE_DISK_FORMAT_VERSION` are
untouched.

## Gates

- `crates/transync-openai/src/client/dispatch.rs`:
  `api_dispatch_heuristic_covers_gpt5_generations`,
  `a_generation_past_gpt5_rides_responses_without_being_listed` (`gpt-6`
  through `gpt-12.0-something`, `o1` through `o6-pro`),
  `a_chat_snapshot_stays_on_chat_completions_in_every_generation`,
  `older_and_numberless_families_stay_on_chat_completions` and
  `an_absurd_generation_saturates_forward_rather_than_falling_back`.
- `crates/transync-openai/src/tokenizer.rs`:
  `a_generation_past_gpt5_is_o200k_without_being_listed` and
  `older_and_numberless_families_stay_cl100k`.
- `crates/transync-core/src/batch.rs`: `encoder_for_reads_the_generation_floor`,
  which lands with this record, holds the core twin to the same answers.

## What this does not do

- It adds no model registry and no list of known families. A family is
  recognised by the number it declares, or not at all.
- It changes no provider selection, no batching default and no override.
- It does not touch the hint mechanism itself, which OI-0029 introduced
  (archived DCR-0015 Part A); it changes only what the OpenAI hint answers.
