---
type: DCR
title: v0.2 provider/cache boundary — fingerprint, fallible Cache with evict, RetryContext
description: The one sanctioned v0.2 breaking change — Translator::fingerprint(), Cache trait v2 (fallible get/put/evict), CacheKey provider/schema-version fields, targeted pipeline eviction, and the non-content RetryContext channel ADR-0009 anticipated.
tags: [change, project-control, DCR-0009]
status: active
---

# DCR-0009: v0.2 provider/cache boundary

- **Date:** 2026-07-13
- **Source:** External review 2026-07 (EXT-2026-07 P1-4 + the RetryContext half of P0-2); resolves OI-0017 items 1–3; design record at the wave's design doc (temp), decisions summarized here
- **Affected ADRs:** docs/decisions/0002-http-free-core-with-translator-trait.md (trait grew a defaulted method — v0.2 break), docs/decisions/0009-reject-retry-policy-changes.md (amended: the anticipated side channel shipped), docs/decisions/0015-cache-poison-and-invalid-hit-policy.md (unchanged semantics, fallible signatures)

## What Changed

Workspace version 0.1.0 → 0.2.0; all breaks land in this one window.

- **Provider identity:** `Translator` gained `fingerprint() -> ProviderFingerprint`
  with a `type_name`-based default; implementors whose instances can differ
  (model/endpoint/API surface) must override — `TransyncOpenAI` covers
  model + effective base URL + resolved API surface.
- **CacheKey:** gained `provider_fingerprint` and `validation_schema_version`
  (`validate::VALIDATION_SCHEMA_VERSION`, bump discipline documented on the
  const — bumps orphan, never corrupt).
- **Cache trait v2:** `get`/`put`/`evict`, all returning
  `Result<_, CacheError>` (`#[non_exhaustive]`); `evict` is required, not
  defaulted. The pipeline degrades on every cache error (warn + miss/skip) —
  a cache failure can never fail a run. `InMemoryCache` stays infallible
  internally and keeps the ADR-0015 clear-once poison policy.
- **Targeted eviction:** after the full-reparse cascade, exactly the
  downgraded keys (`full_reparse_fallbacks`) are evicted; on
  `FullReparseFailure::Hard`, exactly the implicated divergent blocks are
  evicted before the error returns. Never a blanket clear — OI-0011's
  keep-progress-on-abort behavior is preserved.
- **RetryContext:** re-dispatched units carry
  `TranslationUnit.retry: Option<RetryContext>` (attempt number, rejecting
  `ValidationLayer`, 512-byte-truncated reason); the OpenAI client
  serializes it as a data-framed `retry` prompt field with injection
  framing. Payload resubmission stays verbatim (ADR-0009's budget and
  rules unchanged) — this is exactly the non-content channel that ADR
  named as the sanctioned future feature.
- **Provisional vs final validity** documented as contracts.md §5a:
  per-unit acceptance is provisional (cached immediately); the
  full-document reparse is the final gate; disqualification evicts.

## Why

A shared cache without provider identity can replay another provider's
output (R0008-0002); results cached before the document-level gate
deterministically replay collectively-bad translations (R0008-0003/0004);
and verbatim retries relied on provider stochasticity alone. One coherent
boundary change fixes all of these where isolated patches could not.

## Affected Areas

- `crates/transync-core/src/{llm.rs,cache.rs,validate.rs,pipeline.rs,pipeline/retry.rs,lib.rs}`
- `crates/transync-openai/src/{lib.rs,client.rs}`
- `docs/architecture/contracts.md` (§1, §5, new §5a), `docs/architecture/rough-schema.md` (§14)
- Migration for external implementors: add `retry: None` to `TranslationUnit`
  literals; wrap `Cache` impl bodies in `Ok` and implement `evict`; override
  `fingerprint()` when one type serves multiple configs.

## Migration / Follow-up

- Deferred with YAGNI rationale: deadline/cancellation on `Translator`
  (revisit: long-running service consumer), capability set (revisit: second
  in-tree provider or row-window shipping), detected-source-language caching
  (OI-0017's remaining item; revisit: disk-backed-cache design).

## Note (2026-08-08) — the cancellation deferral's revisit condition fired

*Appended, not a rewrite. Everything above stands; this note records that one
line of the follow-up list has been superseded.*

The first YAGNI entry above — **deadline/cancellation on `Translator`, revisit:
long-running service consumer** — is **superseded by
[DCR-0024](DCR-0024-run-cancellation.md)** (2026-08-08, ticket `43331a`). Its
stated condition was met: `dynwebserver` is a tokio daemon holding transync
behind an HTTP endpoint, running translations as background jobs for readers
who navigate away mid-run. The deferral was therefore re-decided deliberately
rather than expiring by default, which is exactly what naming a revisit
condition was for.

What replaced it: `TranslateOptions.cancel: Option<CancellationToken>`, a
`cancel` parameter on both provider-facing `Translator` methods, and
`TransyncError::Cancelled` as the answer a cancelled run gives. See DCR-0024
for the shape and contracts.md §5b for the contract. The other two YAGNI
entries — capability set, detected-source-language caching — are untouched and
their conditions remain unmet.
