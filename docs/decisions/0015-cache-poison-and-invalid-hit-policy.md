---
type: ADR
title: Cache poison policy is clear-once-and-resume; invalid cache hits re-dispatch
description: A poisoned cache mutex is cleared once and resumed empty through one locked() helper, and a cached result that fails validation on reuse is rejected and re-translated.
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

External review P2-8 (EXT-2026-07) showed what that asymmetry costs:
`std::sync::Mutex` poison never clears on its own, so `put` kept
"recovering" via `into_inner` into a map no `get` would ever read —
**dead writes**.

## Decision Drivers

* A poisoned mutex means a panic occurred *while mutating the map* — the
  map's contents can no longer be trusted for reads. Serving a possibly
  half-written entry to skip one provider call is the wrong trade for a
  translation pipeline whose whole design is "never silently corrupt
  output."
* Dropping untrusted contents is always safe (worst case: re-translate),
  and one `tracing::warn` per poison event deliberately keeps the
  underlying bug loud instead of self-healing it into silence.
* Recovery preserves forward progress — new results from the current
  healthy run still land *and are readable*, so a long run isn't
  punished for one earlier panic.
* Independent of poisoning, a cache hit is revalidated before
  acceptance; a hit that fails validation is discarded and the unit
  re-dispatched (the R0006-0002 fix).

## Considered Options

1. Recover reads too (`into_inner` on `get`), fully self-healing.
2. Replace/clear the cache instance on first poison.
3. Miss-on-get (loud), recover-on-put, revalidate every hit.
4. Clear-once-and-resume: on the first poisoned lock, recover the guard,
   clear the map and the poison flag, warn once, and carry on as a
   normal empty cache; revalidate every hit.

## Decision Outcome

Option 4, **clear-once-and-resume** (a refinement of option 2 that
does not discard a live instance), with every cache hit revalidated.
ACCEPT R0006-0002 (fixed: hit revalidation + poison handling shipped).
Option 1 trusts data a panic interrupted; option 2 throws away
provably-good progress written after the poison. Option 3 is
internally inconsistent: its two halves cannot coexist as described,
and its recover-on-put branch is unreachable-by-readers (Context). The
Review 0008 gate rejected R0008-0019 on the ground that option 3's
read/write asymmetry was the intended shape; external review P2-8
superseded option 3, and no asymmetry remains.

**Poison.** On the first poisoned lock — from `get`, `put`, `evict` or
`len` — the guard is recovered via `PoisonError::into_inner`, the map is
**cleared** (its contents were mutated mid-panic and are untrusted),
`Mutex::clear_poison` is called so subsequent locks succeed normally,
and a single `tracing::warn` ("cache poisoned; cleared and resumed
empty") is emitted. The cache is then a normal empty cache: gets miss
until repopulated, and puts land and *are readable*.

**Invalid hits.** Every cache hit re-runs the per-unit validation
layers on reuse before it is accepted. A hit that fails is evicted and
the unit re-dispatched, so a cached translation that no longer
validates is re-translated rather than trusted.

The disk-backed cache (DCR-0028, ticket `f12b8b`) extends both halves of
this policy rather than inventing parallel ones, at the granularity each
failure actually has:

* **Recovery.** `DiskCache`'s persisted state recovers in this ADR's
  shape — loud, safe, forward progress — but line- and file-granular
  instead of map-granular: an unreadable line is skipped with a `warn`,
  a torn tail is truncated to the last good record, and an unreadable or
  unknown-format file is rotated aside so the cache resumes empty. Its
  in-memory *index* mutex uses this ADR's clear-once-and-resume directly
  (clearing the index, not the file — the next open replays the log), as
  do `InMemoryCache`'s document-metadata map (DCR-0028) and
  glossary-harvest map (ti `dca5bf`), each behind its own lock.
* **Hit revalidation is what makes disk entries trustworthy.** That
  gate, not checksums, is why a damaged-but-parseable disk payload
  degrades to a re-translation and can never reach the output. DCR-0028
  leans on it and adds nothing to it.

Status: Implemented (`cache.rs` doc comments trace the rationale;
`pipeline.rs` rejects-and-re-dispatches invalid hits).

### Implementation

`InMemoryCache` routes `get`, `put`, `evict` and `len` through one
private `locked()` helper: a healthy lock's guard is returned unchanged,
and a poisoned one gets the recovery above and comes back as a guard
over the now-empty, functional map. `Mutex::clear_poison` is std,
stabilized in Rust 1.77 and inside the workspace MSRV (the root
`Cargo.toml`'s `rust-version` owns that number). `DiskCache`'s own
`locked()` (`cache/disk.rs`) clears its index maps and leaves the log
untouched. The pipeline revalidates hits before acceptance: the
cache-hit arm in `pipeline/dispatch.rs` runs the cached rows through
`validate_batch`, evicts a rejected hit's key, and re-queues the
pristine unit at its packed position, so the re-dispatch reaches the
provider in the order the packer built (R0013-0005).

## Consequences

* Good, because a panic can never launder a half-written entry into the
  output, and the warn stream surfaces the real bug.
* Good, because after recovery the cache is fully functional: writes
  land and are readable, and a run is not punished for the rest of its
  life by a single earlier panic.
* Good, because clearing on poison is safe by construction — the
  discarded entries are exactly those a panic interrupted mid-mutation,
  which this ADR's first driver already deems untrusted.
* Good, because the single `warn` per poison event keeps the underlying
  bug loud rather than self-healing it into silence.
* Bad, but bounded: the entries present at the moment of the panic are
  dropped and re-translated on next demand. Everything written after
  recovery is cached normally.

## History

The full text of each replaced passage is in
[`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0015.

- 2026-07-11 (R0006-0002, R0008-0019): recorded at the Review 0008 gate as option 3 — miss-on-get, recover-on-put; R0008-0019 rejected.
- 2026-07-13 (external review P2-8): option 3 superseded by clear-once-and-resume through one `locked()` helper; the cache-cold-reads cost is gone.
- 2026-08-09 (DCR-0028, ti f12b8b): the disk cache extends both halves — line/file-granular recovery, index clear-once, hit revalidation over checksums.
