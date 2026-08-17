---
type: ADR
title: Reject making `warnings` optional in the strict output schema
description: OpenAI strict Structured Outputs requires every property in `required`; the lenient serde default on the local parser is deliberate Postel-style asymmetry.
tags: [decision, ADR-0008]
status: active
---

# ADR: Reject making `warnings` optional in the strict output schema

## Context and Problem Statement

Found in Review 0006 (Issue R0006-0020, Severity: Medium) (review archived and removed).
Location: `crates/transync-openai/src/client.rs` (`schema_object_for` / result parsing)

> **Path note (2026-08-05, DCR-0019).** The decision is unchanged; both halves
> of that Location line moved, in two separate steps worth distinguishing.
>
> - **The `schema_object_for` half drifted earlier**, in the DCR-0015 prompt
>   lift (OI-0029, 2026-08-03): the strict-schema builder — including the
>   `required` array this ADR is about — left the provider crate for
>   `crates/transync-core/src/llm/prompt.rs`, and `transync-openai` has only
>   *called* it since. This wave moved those call sites one module deeper,
>   into `crates/transync-openai/src/client/chat.rs` and
>   `client/responses.rs`, when each API surface took ownership of its own
>   request body.
> - **The result-parsing half moved in this wave.** At the `v0.2.0` tag the
>   provider-side envelope readers were `extract_chat_output` and
>   `extract_responses_output` in `crates/transync-openai/src/client.rs`; the
>   client split gave each surface its own `extract_output` in
>   `client/chat.rs` and `client/responses.rs`, reached through the shared
>   `SurfaceRequest::extract_output`. Those readers pull the model's output
>   text out of the HTTP envelope; the JSON → `UnitResult` parse that actually
>   applies the lenient `#[serde(default)]` on `warnings` — the leniency this
>   ADR upholds — sits with the schema in `transync-core`'s `llm/prompt.rs`,
>   having moved there in the same DCR-0015 lift.
>
> Net: the asymmetry this record defends is now expressed in **one** place,
> `transync-core::llm::prompt` — strict schema and lenient parser side by
> side — rather than split across the provider crate. Nothing about the
> decision changed.

The reviewer flagged an asymmetry: the strict Structured Outputs schema
lists `warnings` as required, while the local serde parser accepts a
missing `warnings` field via `#[serde(default)]`.

## Decision Drivers

* OpenAI strict Structured Outputs **mandates** that every declared
  property appear in the `required` array — optionality is expressed only
  through union-with-null types, which complicates the schema for zero gain.
* The local parser must also accept output from non-strict surfaces
  (proxies, future providers) where the field may legitimately be absent.

## Considered Options

1. Make `warnings` optional in the schema (rejected — strict mode forbids it).
2. Make the local parser reject missing `warnings` (rejected — needless
   brittleness toward compliant-but-lenient providers).
3. Keep the deliberate asymmetry: strict on the wire out, lenient parsing in.

## Decision Outcome

REJECT: We keep option 3. The asymmetry is forced by the Structured
Outputs contract and the leniency is deliberate Postel-style robustness.

Status: No change required.

### Implementation

None. This record exists so future reviews do not re-raise the asymmetry.

## Consequences

* Good, because the schema stays valid under strict mode and parsing stays
  robust across provider surfaces.
* Bad, because the asymmetry looks accidental without this record.
