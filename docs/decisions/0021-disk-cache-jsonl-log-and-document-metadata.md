---
type: ADR
title: The disk cache is a self-contained JSON-lines log replayed at open, and document-level metadata rides the cache
description: The disk-backed Cache backend commissioned by f12b8b is a single-directory append-only JSON-lines log with an in-memory index — no embedded database, no per-entry files, no new digest, no format migration — and the Cache seam itself carries document-level metadata (detected source language) so a fully-cache-hit run keeps reporting what a live run observed. Designed by DCR-0028.
tags: [decision, ADR-0021]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-09T00:00:00Z
status: stable
---

# ADR: The disk cache is a self-contained JSON-lines log replayed at open, and document-level metadata rides the cache

## Context and Problem Statement

Ticket `f12b8b` (owner decision 2026-08-06) commissions the disk-backed cache
the `Cache` trait was built for. Two decisions rise to ADR level because they
are principles later work must not silently reverse:

1. **The backing store.** SQLite, an embedded KV store, per-entry files, or a
   log — the choice fixes the failure modes, the dependency set, and what a
   "corrupt cache" can even mean.
2. **Where document-level facts live.** OI-0017's last item: a fully-cache-hit
   `--source-language auto` run reports `detected_source_language: None`
   because the per-unit cache stores no document-scoped state. Something must
   own that state across runs, and the choice of *what* owns it is a
   principle, not an implementation detail.

The precedent named by `mvp-scope.md` is resp-translator ADR-0002, which
weighed the same backing-store options and chose in-memory-only — **because
that system's translations are not durably valuable** (the originals live in
session state; a restart costs one API call). transync's premise is the
opposite: a cache entry is validated provider output over an operator's
document, §5b's cancellation design tells consumers "the paid-for progress
lives in the cache", and a large document's cache is exactly the artifact a
crashed or cancelled run wants back.

## Decision Drivers

* A clean checkout must keep building with no system dependencies and no C
  compile (workspace precedent; resp-translator ADR-0002's driver holds here
  even though its conclusion does not).
* A cache must never fail or corrupt a run: every failure mode has to degrade
  to re-translation (the shipped `CacheError` degrade policy, ADR-0015's hit
  revalidation).
* ADR-0020 fixes the threat model: accidental collision only — but every
  encoding must stay injective, and adding *new* collision surfaces to carry
  old identity would be a defect.
* The 0.3.0 cache-identity contract (prompt-bytes principle, ti `c02f69`) is
  input, not open for re-litigation; the store must transport it exactly.
* Cache state is written by one operator's runs on one machine; cross-machine
  sharing is out of scope by ticket.
* The store must be inspectable by a human with `less` — this repo's
  operational debugging is file-based.

## Considered Options

1. **SQLite (`rusqlite`, bundled)** — ACID and queryable; adds a C compile to
   every clean build for a table the pipeline reads once at open.
2. **Embedded pure-Rust KV (`sled`, `redb`)** — a new dependency whose crash
   recovery becomes part of transync's own trust surface (`sled` additionally
   in maintenance mode; both rejected by the precedent ADR on this ground).
3. **One file per entry** — filenames cannot carry a 12-axis key, so identity
   would need a new digest-to-filename scheme: a brand-new collision surface
   for nothing.
4. **Single append-only JSON-lines log, in-memory index, replay at open,
   compaction by rewrite** — zero new dependencies (serde/serde_json/std::fs),
   self-describing records, every failure mode is a line- or file-granular
   skip.
5. For document metadata: **a separate metadata store** (side file owned by
   the pipeline, or a per-backend downcast) rather than the `Cache` seam.

## Decision Outcome

**Options 4 for the store, and the `Cache` seam — not option 5 — for document
metadata.** Recorded design: DCR-0028 (format grammar, recovery rules,
capacity policy, slices SL-110..SL-114).

**The store principle.** One operator-chosen directory holding one
`transync-cache.jsonl`: a header record carrying `format`, then self-contained
entry/evict/doc_meta records serialized by serde from the real public types.
Opened by full replay into an in-memory index; appended during a run with
flush-per-record and **no fsync contract**; compacted (and capacity-trimmed,
oldest-written first) at open by temp-file-plus-atomic-rename. Three
subordinate rules carry the principle's weight:

- **No new digest, ever.** The on-disk identity of an entry is its full
  serialized axis set, compared by full-key equality after deserialization.
  The store never keys by a hash of the key, so it adds zero collision surface
  beyond the u64 axes `CacheKey` already carries — the disk format is a
  transport of the identity contract, not a party to it. (Injectivity comes
  free: JSON field names are the presence markers and JSON string framing the
  length prefixes that ADR-0020's discipline requires.)
- **Records are self-contained by decision, not convenience.** Interning
  run-scoped strings behind a namespace record (R0003-0051's suggestion) was
  rejected: a truncated or lost indirection record would orphan every entry
  referencing it, while self-contained lines fail independently — the ~200
  bytes of per-line duplication is the price of line-granular recovery and is
  reclaimed by compaction.
- **No format migration.** An unknown or unreadable format is rotated aside
  and the cache starts empty, loudly. A single-operator cache re-translates;
  it does not accumulate migration code paths that run once per format bump
  and rot in between.

**The metadata principle.** Document-scoped facts that must survive a
fully-elided run live **in the cache, behind the `Cache` trait**, as typed
records (`DocumentMetaKey` → `DocumentMeta`) with defaulted trait methods so
every existing implementation keeps compiling with today's behavior. The cache
is the only artifact that outlives a run; a side store or downcast would make
OI-0017's fix a property of one backend instead of the seam, and would leave a
consumer-owned `Cache` silently unable to carry it. Replay is compensation,
never override: metadata is consulted only by runs that made zero provider
calls, and written only from a live qualifying envelope. The metadata key
carries the namespace axes, the language labels and the whole document's
bytes — deliberately not the prompt-identity axes, which are batch-scoped
since DCR-0027 and belong to unit-content replay; DCR-0028 records that
scoping argument in full.

Status: Accepted 2026-08-09; implementation is DCR-0028's slices
SL-110..SL-114.

## Consequences

* Good, because a clean checkout still builds with zero new dependencies, and
  the entire failure surface of persistence is "a line or file was skipped,
  loudly" — every path degrades to re-translation and none can corrupt output
  (ADR-0015's hit revalidation catches a damaged-but-parseable payload like
  any stale entry).
* Good, because the cache identity contract crosses the process boundary
  byte-exactly — every u64 axis is SipHash-1-3 with fixed keys, deterministic
  across processes and platforms — and no new identity scheme exists to
  drift from it.
* Good, because `--source-language auto` stops being the one run whose report
  degrades with a warm cache, for any `Cache` implementation, including
  consumer-owned ones.
* Bad, because open cost is O(log size): the whole file is replayed before the
  first lookup. Accepted for a cache whose budget defaults to 1 GiB and whose
  alternative (a database) buys open speed with a dependency and a darker
  failure surface. Compaction bounds the growth.
* Bad, because durability is best-effort below process exit: no fsync means an
  OS crash can lose the flushed-but-unsynced tail. Accepted: the recovery is a
  re-translation, and the alternative taxes every unit with a disk barrier.
* Bad, because concurrent processes on one cache directory are unsupported
  (documented; cost bounded to lost entries). A future multi-process design
  would supersede this ADR's single-writer rule explicitly rather than
  discover it.

## The metadata principle takes a second record kind — 2026-08-10 (appended note)

Ticket `dca5bf` cached the auto-glossary extraction preflight, the one provider
call a fully-cache-hit run still paid. It **applies** the metadata principle
above rather than amending it: the harvest is a document-scoped fact that must
survive a fully-elided run, so it lives in the cache behind the `Cache` trait,
as a typed record (`GlossaryExtractionKey` → `GlossaryExtraction`) reached by
two more defaulted methods. On disk it is a fourth record kind, `glossary`,
which is the additive-kind tolerance the format decision above already
provides — `CACHE_DISK_FORMAT_VERSION` does not move, and no migration path is
introduced.

One sentence of the principle is scoped rather than extended by it. "Replay is
compensation, never override" is a rule about a **detection** — an advisory
observation a live provider could still contradict, which is why it is read
only by a run that made zero provider calls. A harvest is content the model
produced for a question its key names in full (a digest of the assembled
extraction prompt bytes), so it is read on every enabled run and replaying it
is the ordinary cache trade, identical in kind to replaying a unit entry. The
two records therefore share a home and a discipline, not a read rule. DCR-0028's
2026-08-10 appended note carries the full argument, including why the harvest
is not a `DocumentMeta` field.
