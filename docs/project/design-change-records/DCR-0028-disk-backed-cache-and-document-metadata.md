---
type: DCR
title: The cache learns to outlive the process — a disk-backed backend, owned eviction semantics, and document-level metadata
description: DiskCache joins InMemoryCache behind the unchanged lookup contract — a self-contained JSON-lines log replayed at open, versioned, capacity-bounded, and holding to the injective-encoding discipline. The Cache trait gains two defaulted document-metadata methods so a fully-cache-hit --source-language auto run stops reporting detected_source_language None (OI-0017's last item). All four routed Review-0003 cache findings are answered. Design-first record for ticket f12b8b; slices SL-110..SL-114.
tags: [change, project-control, DCR-0028]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-09T00:00:00Z
status: stable
---

# DCR-0028: The cache learns to outlive the process — a disk-backed backend, owned eviction semantics, and document-level metadata

- **Date:** 2026-08-09
- **Source:** ticket `f12b8b` — owner decision 2026-08-06 to commission the
  disk-backed cache design pass, scheduled fifth of the six post-0.3.0 roadmap
  items. Routed into it by the Review-0003 gate: R0003-0048, R0003-0049 and
  R0003-0051 (this pass owns eviction and representation semantics), with
  R0003-0050 already a direct duplicate of the commission.
- **Design-first.** This record precedes the code; its slices
  (SL-110..SL-114) are the implementation plan. Living documents
  (`contracts.md`, `scenario-matrix.md`, `mvp-scope.md`, `open-issues.md`,
  module docs) keep describing shipped behavior and are edited by the slice
  that ships each change, not by this record.
- **Paired records (the pairing rule):** **new ADR-0021**
  (`docs/decisions/0021-disk-cache-jsonl-log-and-document-metadata.md`) carries
  the two principles this DCR introduces — the backing-store choice and
  *document-level metadata rides the cache*. Dated notes appended 2026-08-09 to
  **ADR-0015** (its recovery and hit-revalidation policies extend to the disk
  backend's analogs) and **ADR-0020** (this design inherits its premise and
  introduces no new digest). The resp-translator ADR-0002 precedent is cited
  as input, not edited — it belongs to a different repository and its "results
  are not durably valuable" driver is exactly what differs here.
- **Affected contracts (edited by the slices, listed here as the map):**
  `contracts.md` §0 (new facade rows: `DiskCache`, `DiskCacheOptions`,
  `DocumentMetaKey`, `DocumentMeta`; the field-set weld grows one type), §1
  (`Cache` trait gains the two defaulted metadata methods; the disk backend's
  durability/concurrency contract), §5a (metadata identity + replay rule), §6
  (`--cache-dir`). Plus `scenario-matrix.md` SCN-10 (persistence across
  processes becomes live-verifiable), `mvp-scope.md` (the "Disk-backed
  translation cache" deferred row resolves), `open-issues.md` OI-0017 (item 4,
  the last one, closes).
- **Surface-moving, under the open v0.4.0 window.** The `Cache` trait change is
  additive (defaulted methods — existing impls keep compiling), but it is a
  *contract* change and is treated as window work: every §0 edit lands with
  `crates/transync/tests/public_surface.rs` in the same commit. No version
  bump, no tag; CHANGELOG under `[Unreleased]`.
- **`VALIDATION_SCHEMA_VERSION` does not bump.** Nothing here changes prompt
  framing or payload semantics; every existing in-memory-era key stays honest.
  The disk format gets its **own** version number (below), on its own axis.

## The problem being solved

Only `InMemoryCache` ships. The `Cache` trait was built as the seam for a
disk-backed backend from v0.1.0 (`cache.rs` module docs; `CacheError`'s Io and
Serialization variants exist for no other reason; `mvp-scope.md` defers it
explicitly), and §5b's cancellation design already tells consumers "the
paid-for progress lives in the cache" — a promise that today dies with the
process holding the map. Meanwhile OI-0017's one remaining item is a real
correctness gap in reporting: the per-unit cache stores no document-level
metadata, so a fully-cache-hit `--source-language auto` run reports
`detected_source_language: None` — the detection was made, paid for, latched
from a live provider envelope, and then thrown away with the run.

Three Review-0003 findings were deliberately routed here rather than fixed
standalone, because capacity, lookup cost and key representation are decisions
about *what a cache entry is*, and this pass owns that question. This record
answers all four (§4).

**Two premises are inputs, not open questions.** The `CacheKey` identity
contract from the 0.3.0 window — cache identity is *exactly the prompt bytes
the model saw*, plus the three namespace axes (owner decision 2026-08-06, ti
`c02f69`, R0001-0005) — is the design's input; nothing below adds, removes or
re-weighs an axis. And ADR-0020's threat model governs: adversarial hash
collision is outside this program's consideration, by owner decision; the
design defends against accidental collision only, while holding to the
injectivity discipline that same ADR preserves (framing ambiguity is a defect
regardless of hash strength).

## What changes

### 1. The backend: `DiskCache`, a core module behind the unchanged lookup seam

`DiskCache` lives in `crates/transync-core/src/cache/disk.rs` (file-as-module
child of `cache.rs`; never `mod.rs`). Not a new crate: it adds **zero
dependencies** — `serde`/`serde_json` are already core dependencies and the I/O
is `std::fs` — and ADR-0003's ground for separate crates (provider HTTP stacks)
does not apply. Not `transync-syntax`: the cache is pipeline infrastructure,
and core is the host-only layer. The wasm gate is untouched by construction —
nothing here is reachable from `transync-syntax` or `transync-wasm`.

```rust
// crates/transync-core/src/cache/disk.rs
pub struct DiskCache { /* Mutex<index> + append writer + dir */ }

impl DiskCache {
    /// Open (creating the directory if absent) with default options.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, CacheError>;
    pub fn open_with(dir: impl AsRef<Path>, opts: DiskCacheOptions) -> Result<Self, CacheError>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl Cache for DiskCache { /* get / put / evict + the §3 metadata methods */ }

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct DiskCacheOptions {
    /// Byte budget for the log file, enforced at open (§4). Default 1 GiB.
    pub max_bytes: Option<u64>,
    /// Entry-count budget, same enforcement point. Default None.
    pub max_entries: Option<u64>,
}
```

Construction is fallible and **typed** (`CacheError::Io` / `Serialization`) —
the degrade decision belongs to the caller. Once constructed, the existing
pipeline degrade policy already covers every mid-run failure: `cache_get` /
`cache_put` / `cache_evict` in `pipeline.rs` warn and degrade (miss /
not-persisted), and a `CacheError` can never fail a translation run. That
policy predates this DCR and is not touched.

At runtime the backend is an **in-memory index over an append-only log**: `get`
is a map lookup (no disk read), `put`/`evict` append one line to a buffered
writer and flush it. This shape is load-bearing for §4's R0003-0050 answer.

Facade rows added (each in the same commit as its `contracts.md` §0 row and
`public_surface.rs` edit): `transync::DiskCache`, `transync::DiskCacheOptions`
(SL-112/SL-113), `transync::DocumentMetaKey`, `transync::DocumentMeta`
(SL-111). All tier (a).

### 2. On-disk format v1: one self-describing JSON-lines log

One operator-chosen directory; inside it, one file: `transync-cache.jsonl`.
Append-only during a run; rewritten only by compaction (§4). Line grammar,
`CACHE_DISK_FORMAT_VERSION = 1`:

```jsonl
{"t":"header","format":1}
{"t":"entry","k":{ …CacheKey, every axis by name… },"v":{ …UnitResult… }}
{"t":"evict","k":{ …CacheKey… }}
{"t":"doc_meta","k":{ …DocumentMetaKey… },"v":{ …DocumentMeta… }}
```

- **Replay-on-open.** The file is read once at `open`; records apply in order
  (later wins; `evict` removes) into the in-memory index. After replay the
  writer appends. There is no read path during a run.
- **Serialization is serde JSON over the real types.** SL-111/SL-112 add
  `Serialize`/`Deserialize` derives to `CacheKey`, `ProviderFingerprint`
  (`serde(transparent)` over its already-injective framed string), `UnitResult`
  and `OutputKind` (`rename_all = "snake_case"`); `BlockId` already has them.
  Derive additions are non-breaking.
- **The injectivity discipline is satisfied by construction, not by a new
  scheme.** ADR-0020's rule — an encoding must remain injective; per-field
  presence markers and length prefixes — is exactly what a JSON object over
  named fields provides: field names are the presence markers, JSON string
  framing is the length delimiter, and absent-vs-empty cannot alias. No axis is
  re-encoded, concatenated or re-hashed on the way to disk.
- **No new digest exists anywhere in the design.** The on-disk identity of an
  entry is its **full serialized axis set**, and a lookup compares full
  deserialized-`CacheKey` equality — the store is *not* keyed by any hash of
  the key. The only collision surfaces are therefore the u64 axes `CacheKey`
  already carries, which ADR-0020 already dispositions. This is the property
  that makes the disk format a pure transport of the 0.3.0 identity contract
  rather than a re-litigation of it.
- **Cross-process determinism is verified, not assumed.** Every u64 axis
  (`source_hash`, `profile_prompt_hash`, `glossary_hash`, `context_hash`,
  `instruction_hash`, and §3's `doc_source_hash`) routes through
  `id::source_hash_bytes` — SipHasher13 with fixed zero keys — so the same
  inputs produce the same axis values in every process on every platform. A
  key written by one run is byte-reproducible by the next. (Had any axis used
  std's randomized `DefaultHasher`, this design would be impossible; none
  does, and SL-112 pins it with a cross-open round-trip test.)

**Recovery, in ADR-0015's shape (loud, safe, forward progress):**

- **Torn tail** (crash or lost buffer mid-append): the final unparseable line
  is discarded and the file truncated to the last good record — one
  `tracing::warn`. Everything before it is intact because every line is
  self-contained (§4, R0003-0051).
- **Unreadable line elsewhere / unknown `"t"`:** skip the line, warn once per
  open. Unknown record types are how format 1 stays forward-tolerant to
  additive record kinds; incompatible changes bump `format` instead.
- **Missing/unreadable header or unknown `format`:** the file is rotated aside
  (`transync-cache.jsonl.unreadable-<unix-ts>`) and the cache starts empty —
  warn, never a hard error, never a migration. A single-operator tool
  re-translates; it does not carry format-migration code (ADR-0021).

**What keeps a bad disk entry out of the output:** nothing new — ADR-0015's
hit revalidation. Every cache hit already re-runs the per-unit validation
layers on reuse, and a failing hit is evicted and re-dispatched. A corrupted-
but-parseable payload is caught by the same gate that catches a stale one.
That existing gate is why the disk backend needs no checksums: a flipped bit
in a payload is a re-translation, not a corruption of output.

### 3. Document-level metadata: the OI-0017 resolution

**The seam.** The `Cache` trait gains two methods, **with default bodies**, so
every existing implementation (in-tree and the two sibling consumers') keeps
compiling and keeps exactly its pre-DCR behavior:

```rust
pub trait Cache: Send + Sync {
    fn get(&self, key: &CacheKey) -> Result<Option<UnitResult>, CacheError>;
    fn put(&self, key: CacheKey, value: UnitResult) -> Result<(), CacheError>;
    fn evict(&self, key: &CacheKey) -> Result<(), CacheError>;

    /// Document-level metadata, keyed independently of unit entries.
    /// Defaults preserve pre-v0.4.0 behavior: a backend that ignores
    /// metadata makes fully-cache-hit runs report no detection — degraded,
    /// never wrong.
    fn get_document_meta(&self, _key: &DocumentMetaKey) -> Result<Option<DocumentMeta>, CacheError> {
        Ok(None)
    }
    fn put_document_meta(&self, _key: DocumentMetaKey, _meta: DocumentMeta) -> Result<(), CacheError> {
        Ok(())
    }
}
```

Both in-tree backends override both methods. The trait change rides the open
window because it moves a §0-documented contract, and its justification is
structural, not incidental: the cache is the **only** artifact that outlives a
run (§5b already names it the keep-progress channel), so document-scoped facts
that must survive a fully-elided run have exactly one honest home. A side
channel (downcast, parallel store, reserved sentinel key) would bypass
consumer-owned `Cache` impls and make the fix a property of one backend
instead of the seam.

**The types** (in `cache.rs`, exhaustive-by-policy on `CacheKey`'s own ground —
a disk backend must serialize and reconstruct every field, so a field addition
is breaking-by-policy and rides a version bump; the §0 field-set weld test
grows a `DocumentMetaKey` destructure alongside the `CacheKey` one):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DocumentMetaKey {
    pub provider_fingerprint: ProviderFingerprint,
    pub validation_schema_version: u32,
    pub model_id: String,
    pub source_lang: String,
    pub target_lang: String,
    /// `id::source_hash_bytes` over the exact source string handed to
    /// `translate` — before parsing, no normalization.
    pub doc_source_hash: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentMeta {
    /// Already bounded by `truncate_diagnostic` before it reaches the latch.
    pub detected_source_language: Option<String>,
}
```

**The identity decision, stated rather than implied.** The meta key carries the
three namespace axes, the two language labels, and the whole document's bytes.
It deliberately does **not** carry the prompt-identity axes
(`profile_prompt_hash`, `glossary_hash`, `context_hash`, `instruction_hash`),
and this is a scoping statement about the R0001-0005 principle, not an
exception to it: that principle governs replay of **unit translation content**,
where the prompt bytes are the product's provenance. A detection is a different
kind of record — an *envelope observation about the document*, advisory,
bounded, and truncated — and since DCR-0027 made the prompt axes batch-scoped
(per section cohort), a run has no single prompt identity a document-level key
could honestly name; any choice (first batch's? a fold over all cohorts?) would
be an invention. The considered alternative — folding every cohort digest into
the key — was rejected: it adds no correctness for the scenario that exists
(unit entries already miss when the profile changes, so a full-hit replay
under a changed profile cannot occur within one cache generation), and the one
case it would distinguish (alternating profiles across runs re-labeling an
advisory field with a detection the same provider+model made on the same
document bytes) is not a defect worth an invented axis. `source_lang` stays a
label core does not interpret — the `auto` sentinel is a CLI-boundary concern
(ADR-0013, ti `fd5aa8`), and this design needs no knowledge of it: it
persists and replays whatever detection was observed, under whatever labels
the run carried.

**Pipeline wiring, both directions gated exactly:**

- **Write:** after the batch fan-out settles, if the run's
  `detected_source_language` latch holds a value **from a live qualifying
  envelope** (the existing R0001-0007 rule: earliest envelope whose batch had
  no `BatchFault`), the pipeline calls `put_document_meta`. Last write wins;
  there is no metadata eviction — the record is advisory and superseding it is
  the only maintenance it needs.
- **Read:** only when the run dispatched **zero provider batches** — every
  unit of every batch was served from cache, so no envelope ever existed. Then
  the pipeline consults `get_document_meta` and reports the stored detection
  in `TranslationOutput` / the alignment map (whose
  `detected_source_language` field is already optional — **no schema
  version moves**). A run that made even one provider call never consults the
  store: replay compensates for the calls the cache elided, it never overrides
  what a live provider said or declined to say. A `CacheError` on either call
  degrades (warn; report `None`) under the existing helper policy.

This resolves OI-0017's item 4 **at the seam** in SL-111 — it works for any
`Cache` a consumer passes to `translate_with_cache`, `InMemoryCache` included
— and becomes durable across processes when SL-112 lands the disk backend.
OI-0017 closes when SL-114 makes the end-to-end CLI scenario real.

### 4. Eviction and representation, owned end to end

This pass owns eviction semantics. Three distinct mechanisms, none conflated:

- **Semantic eviction (unchanged).** §5a's targeted, `BlockId`-keyed eviction
  of reparse-disqualified and `Hard`-implicated entries is untouched; on disk
  it becomes durable as `evict` records, replayed like any other. Never a
  blanket clear.
- **Capacity eviction (new, disk only).** Enforced at **open**: after replay,
  if the live set exceeds `max_bytes` / `max_entries`, oldest-written live
  entries are dropped first until within budget, and the log is compacted.

  **Amendment, 2026-09-03 (OI-0044 / R0009-0082, ti `0a3fca`).** "Until within
  budget" was never quite true and is now scoped: the trim evicts **entries**,
  and the header plus the document-scoped records (`DocumentMeta`, the glossary
  harvest) are exempt from it — so a `max_bytes` at or below their combined
  size named a total no set of entries could reach, the empty set included.
  Unguarded, that made every open drop every entry, still read over budget, and
  do it again next time: a cache that discarded its whole contents on every run
  while announcing a trim. `trim_to_budget` now recognizes an unreachable byte
  budget, logs the arithmetic (requested value, floor, and the floor's two
  components) and applies `max_entries` alone — clamped rather than refused,
  per this record's own "an accelerator must not kill a run" posture. The
  exemption is now stated on the public `max_bytes` doc, which is where an
  operator reads it.
  Compaction also triggers when dead records (superseded/evicted) have caught
  up with the live ones — at least as many dead as live, and at least one
  dead, with the document-scoped records counted among the live. Compaction
  writes a fresh log to a temp file in the same directory and atomically
  renames it over the old — a crash mid-compaction leaves one of the two
  intact files, never a hybrid. Not enforced per `put`: a single run may
  overshoot the budget by at most its own writes (bounded by the document
  being translated), and the next open trims. **Not LRU by
  decision:** true LRU would make every `get` a disk write (persisted read
  recency), inverting the backend's cost model; write order is the recency
  signal. `doc_meta` records are exempt from capacity trim — they are tiny
  and superseded in place.
- **Corruption recovery (§2).** The disk analogs of ADR-0015's
  clear-once-and-resume, applied at the smallest sound granularity: line-skip,
  tail-truncate, whole-file rotate-aside — each loud, each degrading to
  re-translation, never to corrupt output (hit revalidation, §2).

**The four routed findings, answered:**

| Finding | Answer |
|---|---|
| R0003-0048 — `InMemoryCache` has no capacity bound | **Scoping, plus the bounded alternative.** `InMemoryCache` stays unbounded by design: its intended scope is one run / one session (`translate` builds one per call; the CLI one per run), where the bound is the document itself. SL-110 states that scope in its module docs — the finding's own "or clearly scope the cache per run" arm. A consumer holding one cache across many documents indefinitely now has the backend built for that lifetime: `DiskCache` with a default 1 GiB budget (§4). No capacity knob is added to `InMemoryCache` — two eviction policies on the run-scoped map would be dead weight in every shipped path. |
| R0003-0049 — hits deep-clone under the global mutex | **Fixed in SL-110.** The store becomes `HashMap<CacheKey, Arc<UnitResult>>`; `get` clones the `Arc` under the lock (pointer-sized) and materializes the owned `UnitResult` after the guard drops. Trait signature unchanged — returning `Arc` from the trait would move a representation choice into the public contract for no consumer request. |
| R0003-0050 — scalar-only trait forces one lock/backend op per unit | **Rejected, with the grounds recorded here.** The premise (each op pays a backend round trip) does not hold for either shipped backend: both are memory-index lookups, and disk writes are buffered appends. Bulk methods would be speculative API for a network backend that is explicitly out of scope — and *defaulted* bulk methods are additive, addable in any future release without a breaking window, so declining now forecloses nothing. The finding's own text concedes it is "an extension simplification, not a disk-cache implementation request". |
| R0003-0051 — `CacheKey` duplicates run-level strings per entry | **Representation stands, on measurement and on recovery grounds.** In memory: a few short `String` clones per unit against an LLM round trip is not a cost lever, and rewriting the public exhaustive-by-policy key's field types to `Arc<str>` would be breaking churn with no measured need. On disk: entries are deliberately **self-contained** (full axes per line) rather than interned against a namespace record — an indirection record lost to tail truncation would orphan every entry referencing it, while self-contained lines survive independently; the duplication cost (~200 bytes/line) is noise against translated payloads and is reclaimed by compaction anyway. |

### 5. Durability and concurrency, stated as contract

- **Flush per record, no fsync.** Every `put`/`evict`/`put_document_meta`
  appends and flushes the buffered writer, so entries survive process exit in
  the normal case (the §5b cancellation promise). `fsync` is explicitly out of
  contract: an OS crash may lose the tail, and §2's torn-tail recovery makes
  that a re-translation, not a failure. A cache that made every unit a
  synchronous disk barrier would tax the common case to harden the rare one.
- **Single writer per cache directory.** Concurrent processes sharing one
  directory are **unsupported and documented as such** — no lock file, no new
  dependency. The violation cost is bounded by construction: interleaved or
  torn lines are dropped by the tolerant reader, a compaction race loses the
  other process's recent appends — always lost *entries* (re-translation),
  never corrupt output (hit revalidation) and never a crashed run (degrade
  policy). Cross-machine sharing is out of scope by ticket.
- **Poison policy.** `DiskCache`'s in-memory index lives behind a `Mutex` with
  the same clear-once-and-resume recovery `InMemoryCache` uses (ADR-0015);
  recovery clears the *index*, not the file — the next open replays the log.
  `InMemoryCache`'s metadata map (SL-111) adopts the same policy.

### 6. The CLI: `--cache-dir`

`transync translate` gains `--cache-dir <path>`: construct
`DiskCache::open(path)` and call `translate_with_cache`; absent, behavior is
byte-identical to today (fresh per-run cache). On open failure the CLI warns
once and falls back to a fresh `InMemoryCache` — at the CLI boundary a cache
is an accelerator, and an unwritable directory must not kill a translation the
user asked for. No profile `[cache]` table: where the operator's cache lives
is an invocation concern, not a translation-behavior concern, and profiles
travel between machines. §6 documents the flag, the fallback, and the
single-writer rule. This is the slice that makes OI-0017's scenario literal:
run twice with `--cache-dir` and `--source-language auto`; the second run
makes zero provider calls and still reports the detected language.

## Rules the implementer must not violate

1. **The identity contract is transport, never re-litigation.** No `CacheKey`
   axis is added, removed, re-encoded or re-hashed by any slice. The §0
   field-set weld (`cache_key_field_set_is_the_documented_one`) must never be
   loosened; SL-111 extends it to `DocumentMetaKey`.
2. **ADR-0020 governs.** No cryptographic hashes, no checksums-for-attackers,
   no collision hardening beyond the accidental. But every encoding stays
   injective — serde JSON over named fields satisfies this; any hand-rolled
   framing anywhere in the backend is a defect.
3. **A cache can never fail or corrupt a run.** Construction errors are typed
   and the caller decides; every mid-run `CacheError` degrades through the
   existing `pipeline.rs` helpers; every recovery path is warn-and-degrade;
   every disk hit passes the same revalidation gate as a memory hit
   (ADR-0015). No new abort path exists.
4. **Metadata replay only compensates, never overrides.** `get_document_meta`
   is consulted solely on zero-provider-call runs; `put_document_meta` fires
   solely from a live qualifying envelope (R0001-0007's rule). Core continues
   to treat language labels as opaque (ADR-0013) — no sentinel knowledge
   enters the library layer.
5. **Semantic eviction stays targeted** — `BlockId`-derived keys, never a
   blanket clear (§5a); capacity trim and compaction run at open, never
   mid-run behind a `get`.
6. **§0 weld:** every commit that adds a facade row edits `contracts.md` §0,
   `lib.rs`, and `public_surface.rs` together. The weld is never deleted.
7. **`transync-syntax` takes nothing from this DCR** — no `[features]`, no
   core dependency; the wasm gate stays green on every slice.
8. No version bumps or tags; CHANGELOG under `[Unreleased]`. Every
   `cargo test` invocation carries `-- --test-threads=4`, and results are
   read from cargo's own exit code.

## Out of scope

- **Streaming** and **cross-machine cache sharing** (ticket).
- Multi-process concurrent access to one cache directory (documented
  unsupported, §5) and any lock-file mechanism for it.
- Bulk `Cache` trait methods (R0003-0050 — rejected above; addable additively
  whenever a backend with real per-op cost exists).
- Cache-key or identity-principle changes of any kind; `VALIDATION_SCHEMA_VERSION`
  does not move.
- On-disk format migration between format versions (rotate-aside is the
  policy, ADR-0021).
- Caching the auto-glossary extraction preflight (a fully-cache-hit run still
  re-pays that one call when enabled). Real, observed, and **deliberately
  deferred**: it is a second document-level record with its own identity
  questions (the extraction prompt has its own axes), and OI-0017 does not
  name it. Filed as ticket `dca5bf` rather than scope-crept here.
- `InMemoryCache` capacity knobs (R0003-0048 — answered by scoping, §4).

## Implementation slices

Each slice is independently landable and testable, lands with its own tests
plus the living-doc edits for what it ships, and runs the full standing gates
(fmt, clippy, wasm gate, rustdoc gate, workspace tests with
`-- --test-threads=4`, exit codes read from cargo).

- **SL-110 — in-memory representation hardening.** `InMemoryCache` stores
  `Arc<UnitResult>`; the deep clone moves outside the lock (R0003-0049).
  Module docs state the run/session scoping answer (R0003-0048) and the
  representation decision (R0003-0051). No surface movement, no behavior
  change; poison tests keep passing unmodified. Independent of every other
  slice.
- **SL-111 — the document-metadata seam.** `DocumentMetaKey` / `DocumentMeta`
  (+ serde derives on both and on `ProviderFingerprint`); the two defaulted
  `Cache` methods; `InMemoryCache` overrides (own map, same poison policy);
  pipeline wiring (latch→put on live envelope; get on zero-provider-call runs;
  degrade on error); `doc_source_hash` from the pre-parse source string. Same
  commit: §0 rows + `public_surface.rs` + the `DocumentMetaKey` weld
  destructure; contracts §1/§5a. Tests: a fully-cache-hit `auto` run through
  `translate_with_cache` reports the first run's detection; a run with any
  live provider call never consults the store; a faulted-envelope-only run
  stores nothing; explicit-label runs replay under their own labels; the
  degraded (erroring cache) path reports `None` and completes.
- **SL-112 — `DiskCache` v1.** `cache/disk.rs`: format-1 log (header, entry,
  evict, doc_meta), serde derives on `CacheKey`/`UnitResult`/`OutputKind`,
  replay-on-open with last-wins and evict semantics, torn-tail truncation,
  unreadable-line skip, rotate-aside on unknown format, flush-per-record,
  poison policy on the index, `open`/`open_with`. Same commit: §0 rows
  (`DiskCache`) + `public_surface.rs`; contracts §1 durability/concurrency
  contract. Tests: two sequential opens on one directory round-trip entries,
  evictions and metadata (the cross-process determinism pin); a truncated
  tail drops exactly the last record; a foreign-format file is rotated aside
  and the run proceeds; a full pipeline run against `DiskCache` behaves
  identically to `InMemoryCache` including targeted §5a eviction.
- **SL-113 — capacity and compaction.** `DiskCacheOptions` (+ §0 row) with
  the 1 GiB default `max_bytes`; open-time trim (oldest-written first,
  `doc_meta` exempt); dead≥live compaction; temp-file + atomic-rename
  rewrite. Tests: over-budget logs trim to budget preserving newest entries;
  compaction drops superseded/evicted records and survives a simulated
  mid-compaction crash (both files parse); budgets `None` never trim.
- **SL-114 — the CLI and the end-to-end scenario.** `--cache-dir` on
  `translate` (open → `translate_with_cache`; warn-and-fallback to
  `InMemoryCache` on open failure); contracts §6; SCN-10 extension in
  `scenario-matrix.md` + a CLI scenario test: two runs with `--cache-dir` and
  `--source-language auto` against the mock provider — the second makes zero
  provider calls, reports the detected language, and produces a byte-identical
  bundle. Living-doc closures ride here: `mvp-scope.md` deferred row,
  `open-issues.md` OI-0017 (item 4 → RESOLVED, entry closes), Quick_Start /
  Troubleshooting touches, CHANGELOG under `[Unreleased]`.

Dependency order: SL-110 anywhere; SL-111 → SL-112 → SL-113 → SL-114.

## One key axis was added after all — `input_mode`, 2026-08-10 (appended note)

This record's §2 states that the disk format is "a pure transport of the 0.3.0
identity contract rather than a re-litigation of it", and the CHANGELOG entry
for `f12b8b` says no cache key axis is added, removed or re-encoded. Both were
true of *this* design and remain the right description of it. Ticket `5f7942`
then added one axis — `CacheKey.input_mode` — for a reason outside this record:
DCR-0026's row windows made a `table` unit's mode stop being implied by its
`block_kind`, so the label became a prompt byte no axis covered (contracts.md
§5a, *The mode axis*).

What that costs a `DiskCache` log written by an earlier build: its `entry`
records carry no `input_mode` field, so `parse_line` cannot deserialize them
and replay **skips them with the usual warning**. That is this record's own
designed behavior for a record a build cannot read, and it is why the change
needs no migration step — the log is not corrupt, `open` does not fail, and the
first run after the upgrade re-dispatches those units and re-files them.
`CACHE_DISK_FORMAT_VERSION` does **not** move: the log grammar is untouched,
and §2's rule that the format version moves only when the grammar does is
exactly what keeps it still.

The transport property itself is unaffected — a record still carries the whole
key, a lookup still compares full deserialized `CacheKey` equality, and the new
axis is a plain string rather than another digest, so it adds no collision
surface at all.

## The deferred glossary preflight came back to this seam — 2026-08-10 (appended note)

*Out of scope* above defers "caching the auto-glossary extraction preflight" to
ticket `dca5bf`, on the ground that it is a second document-level record with
its own identity questions. It shipped on 2026-08-10, and this note records
what it decided, because the answers extend this record's family rather than
replacing anything in it. No slice above changes.

**It rides this seam; it does not open a second one.** The `Cache` trait gains
two more defaulted methods — `get_glossary_extraction` /
`put_glossary_extraction` — keyed by `GlossaryExtractionKey` and carrying
`GlossaryExtraction`, both new §0 rows with the field-set weld §0 already
applies to `CacheKey` and `DocumentMetaKey`. Same trait, same defaulting
discipline (the defaults store nothing, so a consumer's `Cache` keeps exactly
its pre-v0.4.0 behavior), same last-write-wins-and-never-evict rule, same
degrade-on-`CacheError` policy, and the same rejection of side channels §3
records. On disk it is a fourth record kind, `glossary` — which is precisely
the additive-kind forward tolerance §2 designed for, so
`CACHE_DISK_FORMAT_VERSION` stands still and an older build skips it with the
usual warning. It is exempt from §4's capacity trim on `doc_meta`'s grounds.

**It is not a `DocumentMeta` field, and the reason is the identity question
this record deferred.** A detection is an advisory envelope observation, which
is why §3 could honestly narrow `DocumentMetaKey` to the namespace axes, the
language labels and the document bytes. A harvest is *content a model
produced*, so §5a's full rule applies to it: `GlossaryExtractionKey` folds the
three namespace axes plus one content axis, `request_hash` — a digest of the
length-framed (`EXTRACTION_SYSTEM_PROMPT`, assembled extraction user message)
pair, taken from `llm::prompt`'s builder rather than re-listed from the
request's fields, the discipline `InstructionDigest` already uses. Folding the
extraction's axes into `DocumentMetaKey` instead would have made a detection's
identity depend on glossary configuration that has nothing to do with it, and
the two records are written at opposite ends of a run (the harvest before
batching, the detection after the fan-out settles) and read under opposite
rules — the harvest on every enabled run, the detection only on a
zero-dispatch one. One value type holding both would have needed a
merge-on-write across those two moments.

**What that costs the compensate-never-override principle: nothing, because
the principle is about the detection.** Rule 4 above governs a record that a
live provider could still contradict. A harvest's key names the whole question
the extractor was asked, so a hit is the answer to *that* question, and
replaying it is the ordinary trade every unit entry already makes. Only a live
`Ok(Some(_))` is stored — `Ok(None)` costs no call to rediscover, `Err(_)`
must not latch a transient failure into a permanent one, and a replayed
harvest is not re-filed. The stored value is the provider's answer *before*
`merge_auto_glossary`, so a replayed run re-merges against its own profile and
produces the report row a live run produced.

**One consequence worth naming.** Two enabled runs over one cache can no longer
disagree about the harvest, so `auto_glossary_changes_cache_identity` — which
staged its "different effective glossary" by having two stub instances answer
the same question differently — now stages it by running with the harvest and
then without it. The claim it pins is unchanged; the mechanism it used to
produce two glossaries is exactly the nondeterminism this ticket collapses.

## The compaction trigger learns to weigh, and replay stops slurping — 2026-08-12 (appended note)

Review 0004 (R0004-0026, R0004-0024) found two consequences of one assumption
this record left implicit: that records are interchangeable in size. §4 states
the compaction trigger as "dead records (superseded/evicted) have caught up
with the live ones", a pure **count**, and the byte budget it sits beside is
deliberately "measured against what a compacted log would occupy" — so nothing
in the design ever looked at the file. Neither half is wrong; together they
leave a shape uncovered.

**The shape.** Dead weight concentrated in a *few very large* records — one
enormous superseded entry, a handful of unreadable lines somebody appended, a
big entry written and evicted — never catches up with a larger number of small
live records. The count test says the log is healthy, the budget test measures
a compacted size that is genuinely small, and the file on disk stays hundreds
of megabytes across every open. The fix keeps both existing tests and adds
their byte twin: compaction also runs when the bytes a compaction would reclaim
have caught up with the bytes it would keep (`dead_bytes >= compacted_bytes` —
the same "at least half dead weight" rule the count states, applied to the
measure an operator actually sees). It needs no budget to be set, so it holds
for `max_bytes: None` too, and it cannot churn: a compacted file has no dead
bytes.

**Replay reads the file a line at a time.** §2's "the file is read once at
`open`" is a statement about *how often*, and the implementation read it with
one `std::fs::read` — so the peak allocation was the index plus a byte-for-byte
copy of every dead record the index was about to discard. That is the same
assumption from the other side: an append-only log can be arbitrarily larger
than the live set it encodes, which is exactly why compaction exists. Replay
now streams the file through a `BufReader`, folding one record at a time, so
the peak is the live index it is building. Nothing observable changes — the
torn-tail, unreadable-line and rotate-aside paths keep their exact semantics —
and this is **not** the "streaming" the ticket scoped out: no API streams, no
record leaves the process lazily, and the log is still read exactly once at
open. One deliberate refinement: a file that is about to be **rotated aside**
is no longer truncated first. Rotation exists to hand the operator every byte
that was there, and truncating a fragment we are only preserving destroyed
evidence for no benefit.

## Compaction earns §4's never-a-hybrid claim, and cleans up after itself — 2026-08-12 (appended note)

Review 0004 (R0004-0028, R0004-0030, R0004-0031) reached the same function
three ways. §4 states that compaction's temp-file-plus-`rename` means "a crash
mid-compaction leaves one of the two intact files, never a hybrid". That is
true of a **process** crash — the test `a_crash_mid_compaction_leaves_the_old_log_whole`
pins it — and it was not true of a **power** loss: nothing synced the temp file,
so the rename could become durable before the bytes it named, leaving a renamed
log that is empty or half-written.

**This is not a walk-back of §5, and the distinction is the cost model.**
§5's no-`fsync` rule argues from *per-unit* write cost: a barrier per translated
block would tax every run to harden a rare one, and torn-tail recovery already
turns the loss into a re-translation. Compaction is the opposite shape — at most
once per open, rewriting the whole file — so one `sync_all` on the temp plus a
best-effort `fsync` of the directory after the rename costs nothing measurable
and is what makes §4's sentence true rather than aspirational. The append path
is untouched: it still flushes per record and still never syncs. The directory
sync is best-effort because not every platform will open a directory as a file,
and §6's "a cache is an accelerator" rule says an open must not fail over a
hardening step.

Two smaller repairs land with it. The temp file is now owned by a guard that
**removes it on drop unless the rename succeeded** — every error exit used to
leave one behind, and the names carry a pid and a nanosecond stamp, so a
misbehaving disk accumulated distinct files in the operator's directory. (A
crash still leaves one; nothing can run then, and §4 already records it as inert
and operator-deletable.) And compaction stopped flushing **per record**: the
flush in the shared writer exists for §5's survive-process-exit promise on the
live append path, and a private temp file that is renamed or discarded whole has
no such promise to make, so it was one `write(2)` per record defeating its own
`BufWriter`. It now buffers, flushes once, and syncs — which is where the
barrier above belongs anyway.
