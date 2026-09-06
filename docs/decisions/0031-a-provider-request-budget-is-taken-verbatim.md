---
type: ADR
title: A provider request budget is taken verbatim
description: with_timeout stores whatever Duration the host passes, including zero; the adapters report the budget back rather than validating a domain, because the builder is infallible by design and a caller error surfaces immediately and legibly on the first request.
tags: [decision, ADR-0031]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-06T00:00:00Z
status: stable
---

# ADR: A provider request budget is taken verbatim

## Context and Problem Statement

Found in Review 0011 (Issue R0011-0075, Severity: LOW).
Location: `crates/transync-anthropic/src/lib.rs` and its OpenAI sibling
(`with_timeout` / `request_timeout`).

`with_timeout(d)` sets the total per-request budget and rebuilds the HTTP client
around it. It accepts any `Duration`, `Duration::ZERO` included. The contracts
document's per-instance request budget section (ticket `7065fdb6`) defines what the
budget *means* but says nothing about its domain, so a zero or absurdly small
value is accepted at construction and fails every request afterwards.

## Decision Drivers

* The builder is infallible on purpose: `with_timeout` returns `Self`, not
  `Result<Self>`. Adding a domain check means either a panic in a builder — which
  this codebase does not do for caller configuration — or a signature change, which
  is a breaking change to a published surface and needs a sanctioned window.
* The whole reason the knob exists is that a library consumer with its own timeout
  had no way to make transync honor it; the adapter's stated stance is that the
  host's configured budget is *honored*, not second-guessed. A validation floor is
  transync deciding it knows the host's deployment better than the host does.
* The error is not silent. A zero budget fails the very first request with a
  timeout error, immediately, per request, and `request_timeout()` reports exactly
  the value that was set — which is the field's documented purpose ("so a host can
  log or reconcile the budget it actually got instead of assuming its own
  configuration was honored").
* Any floor would be arbitrary. There is no value that is wrong for every
  deployment: a fake or in-process provider behind the same trait can legitimately
  answer in microseconds, and a fault-injection harness may *want* a budget that
  always expires.

## Considered Options

1. Reject a zero (or below-floor) duration at construction, adding a fallible
   builder or a `ConfigError` variant.
2. Silently clamp a below-floor value up to a minimum.
3. Take the value verbatim and document that it is taken verbatim, including what
   `Duration::ZERO` does.

## Decision Outcome

REJECT: we decided for option 3. Option 1 breaks an infallible builder to catch a
mistake that already announces itself on the first request, and would consume a
breaking window for it. Option 2 is worse than either: clamping means
`request_timeout()` would report a budget the host did not choose, which
contradicts the reason that accessor exists.

Status: Implemented (no code change; `contracts.md`'s per-instance request budget
section now states the domain).

### Implementation

No behavioural change. The contracts section covering `with_timeout` /
`request_timeout` gained a sentence recording that the value is stored verbatim,
that no floor is applied, and that `Duration::ZERO` therefore means every request
fails immediately with a timeout error while `request_timeout()` still reports zero.

## Consequences

* Good, because the builder stays infallible and the accessor keeps its one
  guarantee: it reports what the host set, not what transync would have preferred.
* Good, because a test or fake provider can deliberately configure a budget that
  always expires.
* Bad, because a host that computes its timeout from configuration and gets zero by
  accident learns about it from a request failure rather than from construction.
  The record and the contracts sentence are what make that a documented outcome
  instead of a surprise.
