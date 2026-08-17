---
type: ADR
title: Cache poison policy is miss-on-get / recover-on-put; invalid cache hits re-dispatch
description: A poisoned cache mutex is cleared once and resumed empty (amended 2026-07-13, external review P2-8; superseding the original miss-on-get / recover-on-put split), and a cached result that fails validation on reuse is rejected and re-translated.
tags: [decision, ADR-0015]
status: active
---

# ADR: Cache poison & invalid-hit policy

## Context and Problem Statement

Found in Review 0006 (Issue R0006-0002, Severity: High) (review archived and removed), re-raised in
Review 0008 (Issue R0008-0019, Severity: Low). Location:
`crates/transync-core/src/cache.rs` (`InMemoryCache::{get, put}`),
`crates/transync-core/src/pipeline.rs` (cache-hit validation).

Review 0006 found that cached translations were accepted on reuse
without revalidation, and that mutex poisoning was unhandled. Review
0008 re-raised the residual asymmetry: after a poisoning panic, `put`
recovers via `into_inner` and keeps writing, but every subsequent `get`
stays a miss for the cache's lifetime.

## Decision Drivers

* A poisoned mutex means a panic occurred *while mutating the map* — the
  map's contents can no longer be trusted for reads. Serving a possibly
  half-written entry to skip one provider call is the wrong trade for a
  translation pipeline whose whole design is "never silently corrupt
  output."
* Treating reads as misses is always safe (worst case: re-translate) and
  the `tracing::warn` on every poisoned `get` deliberately keeps the
  underlying bug loud instead of self-healing it into silence.
* Writes recovering via `into_inner` preserves forward progress — new
  results from the current healthy run still land, so a long run isn't
  punished for one earlier panic.
* Independent of poisoning, a cache hit is revalidated before
  acceptance; a hit that fails validation is discarded and the unit
  re-dispatched (the R0006-0002 fix).

## Considered Options

1. Recover reads too (`into_inner` on `get`), fully self-healing.
2. Replace/clear the cache instance on first poison.
3. Miss-on-get (loud), recover-on-put, revalidate every hit.

## Decision Outcome

ACCEPT R0006-0002 (fixed: hit revalidation + poison handling shipped);
REJECT R0008-0019 (the read/write asymmetry is the intended shape):
option 3. Option 1 trusts data a panic interrupted; option 2 throws
away provably-good progress written after the poison.

Status: Implemented (`cache.rs` doc comments trace the rationale;
`pipeline.rs` rejects-and-re-dispatches invalid hits).

> **Superseded** — see *Amended 2026-07-13* below.

#### Amended 2026-07-13 (external review P2-8)

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

### Implementation

`get` → miss + `tracing::warn` on poison; `put` → recover + write;
pipeline revalidates hits before acceptance.

#### Amended 2026-07-13 (external review P2-8)

`InMemoryCache` now routes `get`, `put`, and `len` through one private
`locked()` helper. On a healthy lock it returns the guard unchanged; on
poison it recovers via `into_inner`, clears the map, calls
`Mutex::clear_poison` (std, stable since 1.77; within MSRV 1.85), emits
one `warn`, and returns a guard over the now-empty, functional map. The
pipeline's hit-revalidation behaviour is unchanged.

## Consequences

* Good, because a panic can never launder a half-written entry into the
  output, and the warn stream surfaces the real bug.
* Bad, because one poisoning event makes the rest of the run cache-cold
  on reads (bounded cost: re-translation of not-yet-dispatched units).

#### Amended 2026-07-13 (external review P2-8)

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

#### Appended 2026-08-09 (DCR-0028, ticket f12b8b)

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
