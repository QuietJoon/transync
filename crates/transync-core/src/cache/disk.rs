//! Disk-backed translation cache — format 1 (DCR-0028 / ADR-0021).
//!
//! The whole contract — log grammar, replay, recovery, durability and the
//! single-writer rule — is on [`DiskCache`] itself, where a consumer reads it.
//! This module is the implementation; `cache.rs` re-exports the one public
//! type, so `cache::disk` is not a path anybody outside the crate names.
//!
//! TRACE: DCR-0028
//! TRACE: ADR-0021
//! TRACE: ADR-0015
//! TRACE: ADR-0020
//! TRACE: SCN-10

use super::{
    Cache, CacheError, CacheKey, DocumentMeta, DocumentMetaKey, GlossaryExtraction,
    GlossaryExtractionKey,
};
use crate::llm::UnitResult;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// On-disk format generation. Its own axis: it moves when the *log grammar*
/// changes, never when prompt framing or payload semantics do — those are
/// `validate::VALIDATION_SCHEMA_VERSION`'s, and a `CacheKey` already carries
/// that one.
const CACHE_DISK_FORMAT_VERSION: u32 = 1;

/// The single file a cache directory holds.
const LOG_FILE_NAME: &str = "transync-cache.jsonl";

/// What a compaction temp file's name puts between the log's name and the
/// `<pid>-<nanos>` that makes it unique. Named once because two places read it:
/// [`compaction_temp_name`] writes the file and [`compaction_temp_pid`] decides
/// at open whether a leftover is this process's to reclaim.
const COMPACT_TEMP_INFIX: &str = ".compact-";

/// Default byte budget for the log: 1 GiB.
///
/// Large enough that no single-document workflow meets it, small enough that an
/// unattended long-lived cache has a ceiling instead of a growth curve.
const DEFAULT_MAX_LOG_BYTES: u64 = 1024 * 1024 * 1024;

/// Capacity policy for a [`DiskCache`] (DCR-0028 §4).
///
/// Budgets are enforced at **open**, never per `put`. A single run may overshoot
/// by at most its own writes — bounded by the document it is translating — and
/// the next open trims. Enforcing per write would put a rewrite in the middle of
/// a translation for no benefit the next open does not deliver.
///
/// **Not LRU, by decision.** True LRU would have to persist read recency, making
/// every `get` a disk write and inverting the backend's cost model (a `get` is a
/// map lookup and nothing else). Write order is the recency signal: the oldest
/// *written* live entries are dropped first.
///
/// `#[non_exhaustive]`: construct default-then-assign
/// (`let mut o = DiskCacheOptions::default(); o.max_bytes = None;`).
///
/// TRACE: DCR-0028
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskCacheOptions {
    /// Byte budget for the log file, measured against what a compacted log
    /// would occupy. `None` disables the budget. Default: 1 GiB.
    ///
    /// **Library-only, and that is settled** (ti `650bbb`, 2026-09-03). The
    /// CLI's `--cache-dir` always opens with [`Default`], so a `transync
    /// translate` run gets 1 GiB and no entry cap and has no flag to change
    /// either. DCR-0028 §6 already ruled out a profile `[cache]` table — a
    /// cache is an invocation concern, not a document one — and a flag was
    /// left unbuilt rather than declined; this is the declining.
    ///
    /// Two reasons, and the second is the load-bearing one. A single-operator
    /// cache is unlikely to reach 1 GiB of JSONL entries, so the lever has no
    /// demonstrated user. And exposing it is what would make **OI-0044**
    /// reachable: `trim_to_budget` drops only `entries` while `meta_bytes`
    /// counts toward the total, so a `max_bytes` set below header+meta discards
    /// every entry on every open and never converges. At the 1 GiB default that
    /// is unreachable; behind a flag it is one typo away. A CLI lever therefore
    /// needs OI-0044's floor first, and shipping the flag before it would hand
    /// users a documented way to build a cache that throws everything away.
    ///
    /// **The trigger for revisiting**, so this is falsifiable rather than
    /// permanent: an operator reporting a real 1 GiB cache, or a `--offline`
    /// run (DCR-0046) failing at a miss for entries an open-time trim had
    /// evicted. Either one makes the lever worth its precondition.
    pub max_bytes: Option<u64>,
    /// Live-entry-count budget, enforced at the same point. `None` (the
    /// default) disables it — the byte budget is the one an operator can reason
    /// about without knowing how large a translated block is.
    pub max_entries: Option<u64>,
}

impl Default for DiskCacheOptions {
    fn default() -> Self {
        Self {
            max_bytes: Some(DEFAULT_MAX_LOG_BYTES),
            max_entries: None,
        }
    }
}

/// One line of the log. Internally tagged on `"t"`, so an unknown tag and a
/// malformed line are one failure mode with one handler (skip + warn) — which
/// is exactly the forward-tolerance rule format 1 promises.
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
enum Record {
    Header {
        format: u32,
    },
    Entry {
        k: CacheKey,
        v: UnitResult,
    },
    Evict {
        k: CacheKey,
    },
    DocMeta {
        k: DocumentMetaKey,
        v: DocumentMeta,
    },
    /// The family's second document-scoped record (ti `dca5bf`), added to
    /// format 1 without moving `CACHE_DISK_FORMAT_VERSION`: the grammar is
    /// unchanged, and an older build skips a record kind it does not know with
    /// the usual warning — which is exactly the forward tolerance format 1
    /// promises for additive kinds.
    Glossary {
        k: GlossaryExtractionKey,
        v: GlossaryExtraction,
    },
}

/// The replayed index plus the append writer, behind one lock.
///
/// They share a mutex because they must move together: an appended record and
/// the index change it describes are one logical write, and a second lock
/// would only create an order to get wrong.
struct DiskState {
    entries: HashMap<CacheKey, Arc<UnitResult>>,
    meta: HashMap<DocumentMetaKey, DocumentMeta>,
    glossary: HashMap<GlossaryExtractionKey, GlossaryExtraction>,
    writer: BufWriter<File>,
}

/// Disk-backed [`Cache`]: an in-memory index over an append-only log
/// (DCR-0028 / ADR-0021).
///
/// One operator-chosen directory holds one file, `transync-cache.jsonl`,
/// replayed once at [`DiskCache::open`] into an in-memory index and appended to
/// for the rest of the process's life. There is no read path during a run —
/// [`Cache::get`] is a map lookup, and `put` / `evict` are one buffered append
/// plus a flush.
///
/// ```text
/// {"t":"header","format":1}
/// {"t":"entry","k":{ …CacheKey, every axis by name… },"v":{ …UnitResult… }}
/// {"t":"evict","k":{ …CacheKey… }}
/// {"t":"doc_meta","k":{ …DocumentMetaKey… },"v":{ …DocumentMeta… }}
/// {"t":"glossary","k":{ …GlossaryExtractionKey… },"v":{ …GlossaryExtraction… }}
/// ```
///
/// Records apply in order and later wins; an `evict` removes.
///
/// # Identity is transport, never re-litigation
///
/// Records carry the *whole* key, and a lookup compares full [`CacheKey`]
/// equality — the store is not keyed by any digest of the key, so it adds no
/// collision surface beyond the `u64` axes the key already carries (ADR-0020
/// disposes of those, and adversarial collision is outside this program's
/// threat model). ADR-0020's injectivity discipline is satisfied *by
/// construction* rather than by a new scheme: JSON field names are the presence
/// markers, JSON string framing is the length delimiter, and absent-versus-empty
/// cannot alias. Any hand-rolled framing anywhere in this backend is a defect.
///
/// Every `u64` axis routes through `id::source_hash_bytes` — SipHasher13 with
/// fixed zero keys — so a key written by one run is byte-reproducible by the
/// next, in another process on another machine. That is the property the whole
/// design rests on, and it is pinned by a cross-open round-trip test.
///
/// # Recovery: loud, safe, always forward (ADR-0015's shape)
///
/// - **Torn tail** — a crash mid-append leaves bytes after the last newline.
///   They are discarded and the file truncated to the last complete record,
///   with one warning. Everything before it is intact because every line is
///   self-contained. A file whose *only* bytes are a torn record has no header
///   at all, so it takes the rotate-aside path below instead and is preserved
///   whole; its warning names that rotation, never a truncation.
/// - **Unreadable line elsewhere, or an unknown `"t"`** — the line is skipped,
///   with one warning per open. Unknown record types are how format 1 stays
///   forward-tolerant to additive record kinds; incompatible changes bump
///   `format` instead.
/// - **Missing or unreadable header, or an unknown `format`** — the file is
///   rotated aside to `transync-cache.jsonl.unreadable-<unix-ts>` (preserved,
///   never deleted) and the cache starts empty. Never a hard error, never a
///   migration: a single-operator tool re-translates rather than carrying
///   format-migration code (ADR-0021).
///
/// None of this can put a bad entry into the output. Every cache hit is
/// re-validated through the per-unit layers on reuse (ADR-0015), and a failing
/// hit is evicted and re-dispatched — so a flipped bit in a payload is a
/// re-translation, not a corrupted document. That existing gate is why this
/// format carries no checksums.
///
/// # Capacity and compaction (DCR-0028 §4)
///
/// Three eviction mechanisms exist and are not conflated. **Semantic eviction**
/// is the pipeline's, targeted by `BlockId` per contracts.md §5a, and on disk it
/// simply becomes a durable `evict` record. **Capacity eviction** is
/// [`DiskCacheOptions`]', enforced at open: after replay, live entries over
/// budget are dropped oldest-*written* first (not LRU — see the options' docs)
/// and the log is compacted. Compaction also runs when the dead weight
/// (superseded and evicted records, and lines this build cannot read) has
/// caught up with the live set — measured **both** as a record count and as
/// bytes, because a handful of very large dead records outweighs a much larger
/// number of small live ones and would otherwise keep the file physically over
/// its budget open after open. It rewrites the log through a temp file in the
/// same directory plus one `rename`, so a crash leaves one of the two complete
/// files and never a hybrid — plus, in that case, the abandoned temp itself
/// (`transync-cache.jsonl.compact-<pid>-<nanos>`). Open sweeps those: a leftover
/// carrying **this** process's id is removed, because it can only be a crashed
/// predecessor whose pid the OS handed back; one carrying **another** pid is
/// left in place and named in a warning, because a foreign pid is not evidence
/// of death (ADR-0024) and deleting a peer's temp would cost that run its cache
/// to save this one a file. Such a file is inert either way — nothing but
/// `transync-cache.jsonl` is ever read — and an operator may delete it.
/// **Corruption recovery** is the third and is the section above. The
/// document-scoped records — `doc_meta` and `glossary` — are exempt from the
/// capacity trim: one small record per document each, superseded in place
/// rather than accumulating.
///
/// # Durability and concurrency (contracts.md §1)
///
/// Every write appends and **flushes**, so entries survive process exit in the
/// normal case. `fsync` is deliberately *out* of contract: an OS crash may lose
/// the tail, which the torn-tail recovery makes a re-translation, and a cache
/// that made every unit a synchronous disk barrier would tax the common case to
/// harden the rare one.
///
/// **One writer per cache directory.** Concurrent processes sharing one
/// directory are unsupported — no lock file, no new dependency. The violation
/// cost is bounded by construction: interleaved or torn lines are dropped by the
/// tolerant reader and a compaction race loses the other process's recent
/// appends, so the loss is always *entries* (a re-translation), never corrupt
/// output and never a failed run.
///
/// Construction is fallible and typed — the degrade decision belongs to the
/// caller (the CLI warns once and falls back to an in-memory cache). Once
/// constructed, the pipeline's existing policy covers every mid-run failure: a
/// [`CacheError`] degrades the run and can never abort it.
///
/// TRACE: DCR-0028
/// TRACE: ADR-0021
/// TRACE: SCN-10
pub struct DiskCache {
    dir: PathBuf,
    state: Mutex<DiskState>,
}

impl DiskCache {
    /// Open the cache in `dir` with the default capacity policy
    /// ([`DiskCacheOptions::default`] — a 1 GiB byte budget, no entry cap),
    /// creating the directory and the log if absent and replaying an existing
    /// log into the index.
    ///
    /// Never fails on a *damaged* log — damage is recovered per this type's
    /// docs. It fails only when the directory or the file cannot be used at all
    /// (`CacheError::Io`).
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, CacheError> {
        Self::open_with(dir, DiskCacheOptions::default())
    }

    /// As [`DiskCache::open`], with an explicit capacity policy (DCR-0028 §4).
    ///
    /// Open is where capacity is enforced, and it is the only place the log is
    /// ever rewritten: after replay, entries over budget are dropped
    /// oldest-written-first, and the log is compacted when a trim happened or
    /// when the dead weight has caught up with the live set — by record count
    /// or by bytes, whichever notices first. The document-scoped records —
    /// `doc_meta` and `glossary` — are exempt from the trim: they are tiny and
    /// superseded in place.
    ///
    /// It is also where the directory's own housekeeping happens: a compaction
    /// temp carrying **this** pid, and not held by a compaction running right
    /// now, is reclaimed; one carrying another pid is named in a warning rather
    /// than deleted, which is the ordinary case after a crash and leaves the
    /// file for the operator. See this module's `sweep_compaction_temps` for
    /// why the two cases differ and what the warning is for (a private item, so
    /// it is named here rather than linked).
    pub fn open_with(dir: impl AsRef<Path>, opts: DiskCacheOptions) -> Result<Self, CacheError> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)
            .map_err(|e| CacheError::Io(format!("create cache dir {}: {e}", dir.display())))?;
        // Before anything reads or writes: clear what a crashed predecessor of
        // this pid left behind, and name what belongs to a pid we cannot judge.
        // Ahead of the compaction below, so this open's own temp is never in
        // the set being swept.
        sweep_compaction_temps(&dir);
        let path = dir.join(LOG_FILE_NAME);

        let mut replayed = replay_log(&path)?;
        let trimmed = trim_to_budget(&mut replayed, &opts, &path);
        // A fresh (or rotated-aside) log has nothing to compact and no header
        // yet; an existing one is rewritten when a trim dropped entries or when
        // the log is at least half dead weight — measured **both** ways, because
        // records are not interchangeable in size. A file whose dead weight is a
        // few very large records (superseded huge entries, or unreadable lines
        // somebody appended) never satisfies the count test against a larger
        // number of small live ones, and would stay physically over its budget
        // open after open.
        let compact = !replayed.write_header
            && (trimmed
                || (replayed.dead_records > 0 && replayed.dead_records >= replayed.live())
                || replayed.dead_bytes() >= replayed.compacted_bytes());
        if compact {
            compact_log(&dir, &path, &replayed)?;
        }

        let mut writer = BufWriter::new(open_for_append(&path)?);
        if replayed.write_header {
            write_record(
                &mut writer,
                &Record::Header {
                    format: CACHE_DISK_FORMAT_VERSION,
                },
            )?;
        }

        Ok(Self {
            dir,
            state: Mutex::new(DiskState {
                entries: replayed
                    .entries
                    .into_iter()
                    .map(|(k, e)| (k, e.value))
                    .collect(),
                meta: replayed.meta,
                glossary: replayed.glossary,
                writer,
            }),
        })
    }

    /// The directory this cache lives in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Number of live unit entries in the index. Document-metadata records are
    /// a separate space and are not counted, exactly as on `InMemoryCache`.
    pub fn len(&self) -> usize {
        self.locked().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Lock the state with the clear-once-and-resume poison policy
    /// (ADR-0015), applied to the **index** rather than to the file: recovery
    /// drops the untrusted maps and leaves the log alone, so the next open
    /// replays everything the panicking run had written. Within this process
    /// the effect is a cold cache — misses until repopulated, writes that land
    /// and are readable.
    fn locked(&self) -> std::sync::MutexGuard<'_, DiskState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                guard.entries.clear();
                guard.meta.clear();
                guard.glossary.clear();
                self.state.clear_poison();
                tracing::warn!(
                    target: "transync::cache",
                    "disk cache index poisoned; cleared and resumed empty (the log is untouched \
                     and replays on the next open)"
                );
                guard
            }
        }
    }
}

/// Append one record and flush it, so the write survives process exit
/// (DCR-0028 §5; no `fsync` — see the module docs).
fn write_record(writer: &mut BufWriter<File>, record: &Record) -> Result<(), CacheError> {
    buffer_record(writer, record)?;
    writer
        .flush()
        .map_err(|e| CacheError::Io(format!("append cache record: {e}")))
}

/// Append one record **without** flushing.
///
/// The flush in [`write_record`] buys a specific promise: a record a live run
/// wrote is on its way to the file even if the process exits next. Compaction
/// has no such promise to make — it writes a private temp file that is either
/// renamed into place whole or thrown away, and nobody may read it in between —
/// so flushing per record there only defeats the `BufWriter` and turns one
/// rewrite into one `write(2)` per record. Compaction flushes once, at the end,
/// and then syncs.
fn buffer_record(writer: &mut BufWriter<File>, record: &Record) -> Result<(), CacheError> {
    let line = serde_json::to_string(record)
        .map_err(|e| CacheError::Serialization(format!("encode cache record: {e}")))?;
    writer
        .write_all(line.as_bytes())
        .and_then(|()| writer.write_all(b"\n"))
        .map_err(|e| CacheError::Io(format!("append cache record: {e}")))
}

fn open_for_append(path: &Path) -> Result<File, CacheError> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| CacheError::Io(format!("open cache log {}: {e}", path.display())))
}

/// One live entry, with the bookkeeping the open-time capacity pass needs.
struct LiveEntry {
    value: Arc<UnitResult>,
    /// Position of the record that produced this value, in log order. A
    /// superseding write moves the entry to the back — write order *is* the
    /// recency signal (DCR-0028 §4: not LRU).
    seq: u64,
    /// Serialized size of that record, newline included, so the byte budget can
    /// be measured against what a compacted log would occupy rather than
    /// against the current file (which may be mostly dead weight).
    bytes: u64,
}

/// What a replay produced.
struct Replayed {
    entries: HashMap<CacheKey, LiveEntry>,
    meta: HashMap<DocumentMetaKey, DocumentMeta>,
    glossary: HashMap<GlossaryExtractionKey, GlossaryExtraction>,
    /// Serialized size of the live document-scoped records (`doc_meta` plus
    /// `glossary`), newlines included.
    meta_bytes: u64,
    /// Records read that the live set does not include: superseded entries,
    /// `evict` records and the entries they removed, superseded metadata. The
    /// compaction trigger compares this against the live count.
    dead_records: u64,
    /// Bytes the file occupies after replay's own repairs — every complete
    /// line, the header included, with a truncated torn tail already excluded.
    /// The compaction trigger compares this against [`Replayed::compacted_bytes`].
    physical_bytes: u64,
    /// True when the file is new or was rotated aside, so the writer must lay
    /// down a header before any record.
    write_header: bool,
}

impl Replayed {
    fn live(&self) -> u64 {
        (self.entries.len() + self.meta.len() + self.glossary.len()) as u64
    }

    /// Bytes a compacted log would occupy: the header plus every live record.
    fn compacted_bytes(&self) -> u64 {
        header_bytes() + self.meta_bytes + self.entries.values().map(|e| e.bytes).sum::<u64>()
    }

    /// Bytes the file carries that a compaction would reclaim: superseded and
    /// evicted records, unreadable lines, and anything a capacity trim has just
    /// dropped. The byte half of the compaction trigger — one 500 MB dead
    /// record is invisible to a record *count* and is the whole file to an
    /// operator looking at the disk.
    fn dead_bytes(&self) -> u64 {
        self.physical_bytes.saturating_sub(self.compacted_bytes())
    }
}

/// Serialized size of the header line, newline included. Constant per build;
/// computed rather than typed so it cannot drift from the record it describes.
fn header_bytes() -> u64 {
    serde_json::to_string(&Record::Header {
        format: CACHE_DISK_FORMAT_VERSION,
    })
    .map(|s| s.len() as u64 + 1)
    .unwrap_or(0)
}

/// Drop live entries until the log fits its budgets, oldest-written first
/// (DCR-0028 §4). Returns whether anything was dropped.
///
/// The document-scoped records — `doc_meta` and `glossary` — are **exempt**:
/// each is one small record per document, superseded in place rather than
/// accumulating, and trimming them would trade a negligible number of bytes for
/// the properties they exist to provide (OI-0017's replayed detection, ti
/// `dca5bf`'s elided preflight). They still count toward the measured size,
/// because they really do occupy the file.
fn trim_to_budget(replayed: &mut Replayed, opts: &DiskCacheOptions, path: &Path) -> bool {
    if opts.max_bytes.is_none() && opts.max_entries.is_none() {
        return false;
    }

    let mut total = replayed.compacted_bytes();
    let mut count = replayed.entries.len() as u64;
    let over = |total: u64, count: u64| {
        opts.max_bytes.is_some_and(|m| total > m) || opts.max_entries.is_some_and(|m| count > m)
    };
    if !over(total, count) {
        return false;
    }

    let mut oldest_first: Vec<(CacheKey, u64, u64)> = replayed
        .entries
        .iter()
        .map(|(k, e)| (k.clone(), e.seq, e.bytes))
        .collect();
    oldest_first.sort_by_key(|(_, seq, _)| *seq);

    let mut dropped = 0u64;
    for (key, _, bytes) in oldest_first {
        if !over(total, count) {
            break;
        }
        replayed.entries.remove(&key);
        total = total.saturating_sub(bytes);
        count -= 1;
        dropped += 1;
    }

    if dropped > 0 {
        tracing::warn!(
            target: "transync::cache",
            "cache log {} exceeded its budget; dropped {dropped} oldest-written entr(ies), \
             {count} kept",
            path.display()
        );
        // Every dropped entry's record is now dead weight in the file, which is
        // why a trim always forces the compaction that removes it.
        replayed.dead_records = replayed.dead_records.saturating_add(dropped);
    }
    dropped > 0
}

/// Total order over a [`DocumentMetaKey`], for compaction's deterministic
/// output. **Every** field is in it — a partial order would leave two keys
/// that differ only in the omitted field tied, and a tie is exactly the
/// `HashMap` nondeterminism this exists to remove.
fn meta_key_order(k: &DocumentMetaKey) -> (&str, u32, &str, &str, &str, u64) {
    (
        k.provider_fingerprint.as_str(),
        k.validation_schema_version,
        k.model_id.as_str(),
        k.source_lang.as_str(),
        k.target_lang.as_str(),
        k.doc_source_hash,
    )
}

/// Total order over a [`GlossaryExtractionKey`], on the same rule as
/// [`meta_key_order`]: every field, so no two distinct keys tie.
fn glossary_key_order(k: &GlossaryExtractionKey) -> (&str, u32, &str, u64) {
    (
        k.provider_fingerprint.as_str(),
        k.validation_schema_version,
        k.model_id.as_str(),
        k.request_hash,
    )
}

/// Rewrite the log as exactly its live set: a header, then every live unit
/// entry in the order it was originally written, then the document-scoped
/// records in key order. Both orders are *deterministic* — that is the point:
/// two caches holding the same live set compact to the same bytes, so a
/// compacted log can be diffed and compared across runs.
///
/// Written to a temp file in the **same directory** and moved into place with
/// one `rename`, so a crash leaves either the old complete log or the new
/// complete log — never a hybrid. Two things make that promise hold rather than
/// merely describe the happy path:
///
/// - **The temp file is synced before the rename**, and the directory after it.
///   A rename can otherwise become durable before the bytes it names do, so a
///   power loss (not a process crash) could leave a renamed log that is empty
///   or half-written — exactly the hybrid the design excludes. This is *not* a
///   walk-back of DCR-0028 §5's no-`fsync` rule: that rule is about the per-unit
///   append path, where a synchronous barrier per translated block would tax
///   every run to harden a rare one. Compaction runs at most once per open and
///   rewrites the whole file, so one barrier costs nothing measurable and is
///   what makes §4's claim true. The directory sync is best-effort — not every
///   platform lets a directory be opened as a file, and a cache must never fail
///   an open over a hardening step.
/// - **The temp file removes itself unless it is renamed.** Every error exit
///   here used to leave one behind, and the names carry a pid and a timestamp,
///   so repeated failures accumulated distinct files in the operator's cache
///   directory. A crash still leaves one — nothing runs in a killed process —
///   and that leftover is [`sweep_compaction_temps`]' business at the next
///   open, which after a real crash usually means *named in a warning* rather
///   than removed: the restarted run has a new pid. Either way it is inert,
///   because nothing but `transync-cache.jsonl` is ever read.
fn compact_log(dir: &Path, path: &Path, replayed: &Replayed) -> Result<(), CacheError> {
    let temp = TempLog::at(
        dir,
        compaction_temp_name(
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
        ),
    );

    let file = File::create(temp.path()).map_err(|e| {
        CacheError::Io(format!(
            "create compaction temp {}: {e}",
            temp.path().display()
        ))
    })?;
    let mut writer = BufWriter::new(file);
    buffer_record(
        &mut writer,
        &Record::Header {
            format: CACHE_DISK_FORMAT_VERSION,
        },
    )?;
    let mut ordered: Vec<(&CacheKey, &LiveEntry)> = replayed.entries.iter().collect();
    ordered.sort_by_key(|(_, e)| e.seq);
    for (key, entry) in ordered {
        buffer_record(
            &mut writer,
            &Record::Entry {
                k: key.clone(),
                v: (*entry.value).clone(),
            },
        )?;
    }
    // The two document-scoped maps have no `seq` to ride, and `HashMap`
    // iteration order varies between processes, so writing them as they come
    // out would make two byte-identical logical caches compact to two
    // different files (R0009-0083). Nothing functional depends on the order —
    // replay is last-wins per key and the size accounting is order-independent
    // — but a reproducible artifact is worth two sorts on a path that runs at
    // most once per open.
    let mut ordered_meta: Vec<(&DocumentMetaKey, &DocumentMeta)> = replayed.meta.iter().collect();
    ordered_meta.sort_by(|(a, _), (b, _)| meta_key_order(a).cmp(&meta_key_order(b)));
    for (key, meta) in ordered_meta {
        buffer_record(
            &mut writer,
            &Record::DocMeta {
                k: key.clone(),
                v: meta.clone(),
            },
        )?;
    }
    let mut ordered_glossary: Vec<(&GlossaryExtractionKey, &GlossaryExtraction)> =
        replayed.glossary.iter().collect();
    ordered_glossary.sort_by(|(a, _), (b, _)| glossary_key_order(a).cmp(&glossary_key_order(b)));
    for (key, extraction) in ordered_glossary {
        buffer_record(
            &mut writer,
            &Record::Glossary {
                k: key.clone(),
                v: extraction.clone(),
            },
        )?;
    }
    writer.flush().map_err(|e| {
        CacheError::Io(format!(
            "flush compaction temp {}: {e}",
            temp.path().display()
        ))
    })?;
    writer.get_ref().sync_all().map_err(|e| {
        CacheError::Io(format!(
            "sync compaction temp {}: {e}",
            temp.path().display()
        ))
    })?;
    drop(writer);

    if let Err(e) = std::fs::rename(temp.path(), path) {
        // `NotFound` here means the *source* is gone: something outside this
        // process unlinked the temp between the create above and this call.
        // The only thing that does that is another transync sweeping this
        // directory and reading our pid as its own — the pid-namespace case
        // `sweep_compaction_temps` cannot decide (two containers over one
        // cache volume, both pid 1). Compaction is an optimization, and the
        // old log is still complete and still the log, so this must not cost
        // that run its cache: skip the rewrite, say so, and open normally.
        // contracts.md §1 bounds a single-writer violation to lost entries,
        // never a failed run, and this is what keeps that true.
        if e.kind() == std::io::ErrorKind::NotFound {
            tracing::warn!(
                target: "transync::cache",
                "compaction temp {} disappeared before it could replace {}; \
                 leaving the existing log in place and skipping this compaction \
                 (another process is writing this cache directory)",
                temp.path().display(),
                path.display()
            );
            return Ok(());
        }
        return Err(CacheError::Io(format!(
            "move compacted cache log into place ({} -> {}): {e}",
            temp.path().display(),
            path.display()
        )));
    }
    temp.renamed();
    sync_dir(dir);
    tracing::info!(
        target: "transync::cache",
        "compacted cache log {} to {} live record(s)",
        path.display(),
        replayed.live()
    );
    Ok(())
}

/// The compaction temp names this process is holding **right now** — one
/// entry per live [`TempLog`], counted rather than set-valued so two guards
/// that somehow chose one name cannot deregister each other.
///
/// [`sweep_compaction_temps`] consults it before deleting an own-pid leftover.
/// Without it the sweep's own-pid rule was unsound within a single process: a
/// second `DiskCache::open_with` on the same directory from another thread
/// sees a sibling's in-flight temp, reads back its own pid, and unlinks a file
/// whose writer is still using it (found in adversarial review of ti `d51ed7`).
/// Registration happens in [`TempLog::at`], which runs *before* the file is
/// created, so there is no window where the path exists and is unguarded.
///
/// Keyed on the file **name**, not the full path: two `DiskCache` handles on
/// one directory can spell that directory differently (relative, symlinked,
/// trailing separator), and a key that compares unequal for the same file
/// would be a guard that silently is not one. A name carries this process's
/// pid and a nanosecond stamp, so the only cost of the coarser key is that a
/// leftover in *another* directory sharing an in-flight name is left unswept —
/// a file that is not reclaimed, never a file that is wrongly deleted.
static LIVE_COMPACTION_TEMPS: std::sync::LazyLock<Mutex<HashMap<String, usize>>> =
    std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));

/// The live-temp registry, with the poison policy every lock in this module
/// uses: a panicking holder leaves a map, not a dead cache.
fn live_compaction_temps() -> std::sync::MutexGuard<'static, HashMap<String, usize>> {
    LIVE_COMPACTION_TEMPS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A compaction temp file that deletes itself unless [`TempLog::renamed`] says
/// it became the log.
///
/// Every exit between "temp created" and "rename succeeded" leaves the old log
/// in place, so the temp is garbage the moment it happens — and the only
/// process that knows its name is this one. Most of those exits also report an
/// error; the one that does not is a `rename` whose source has vanished, which
/// `compact_log` downgrades to a skipped compaction.
///
/// The guard also publishes the name in [`LIVE_COMPACTION_TEMPS`] for as long
/// as it lives, which is what keeps a concurrent open in this process from
/// sweeping a compaction that is still running.
struct TempLog {
    path: PathBuf,
    name: String,
    renamed: bool,
}

impl TempLog {
    /// Claim `dir/name`. The name is registered as in-flight *first*, so the
    /// file never exists unguarded.
    fn at(dir: &Path, name: String) -> Self {
        *live_compaction_temps().entry(name.clone()).or_insert(0) += 1;
        Self {
            path: dir.join(&name),
            name,
            renamed: false,
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// The file is the log now; there is nothing at this path to remove.
    fn renamed(mut self) {
        self.renamed = true;
    }
}

impl Drop for TempLog {
    fn drop(&mut self) {
        {
            let mut live = live_compaction_temps();
            if let std::collections::hash_map::Entry::Occupied(mut slot) =
                live.entry(std::mem::take(&mut self.name))
            {
                *slot.get_mut() -= 1;
                if *slot.get() == 0 {
                    slot.remove();
                }
            }
        }
        if !self.renamed {
            // Best-effort: this runs on an error path that already has a
            // failure to report, and on an unwind, where a second failure must
            // not abort the process.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The name a compaction temp file takes in the cache directory: the log's own
/// name, then `.compact-<pid>-<nanos>`.
///
/// Both halves are load-bearing. The **pid** is what [`sweep_compaction_temps`]
/// reads to decide whether a leftover is this process's to reclaim; the
/// **nanosecond stamp** keeps two compactions from ever writing one path, which
/// is what makes "either the old complete log or the new one" true rather than
/// "whichever of two interleaved writers finished last".
fn compaction_temp_name(pid: u32, nanos: u128) -> String {
    format!("{LOG_FILE_NAME}{COMPACT_TEMP_INFIX}{pid}-{nanos}")
}

/// The pid a compaction temp file's name carries, or `None` when the name is
/// not one this module wrote.
///
/// Deliberately strict: anything that does not parse — a rotated-aside
/// `.unreadable-<ts>` log, an operator's own note, a name whose pid field is not
/// a number — is not ours to reason about, and the sweep neither reclaims it nor
/// counts it.
fn compaction_temp_pid(name: &str) -> Option<u32> {
    let rest = name
        .strip_prefix(LOG_FILE_NAME)?
        .strip_prefix(COMPACT_TEMP_INFIX)?;
    let (pid, nanos) = rest.split_once('-')?;
    if nanos.is_empty() {
        return None;
    }
    pid.parse().ok()
}

/// Reclaim the compaction temps this process can prove are abandoned, and name
/// the rest for the operator.
///
/// **What it does not do.** This is not a general reclamation of the leak ti
/// `d51ed7` was filed about, and the entry below it in the byte budget is not
/// the measure of it. A crash stamps its temp with the **dead** process's pid;
/// the next run has a different pid, takes the foreign branch, and the file
/// stays. On that path — crash, restart, new pid, which is the ordinary one —
/// this function removes nothing and emits a warning naming the files and the
/// glob to delete. **Operators clean foreign-pid temps by hand**, which is one
/// of the two outcomes ti `d51ed7`'s *Expected* asked for; the sweep supplies
/// the other, plus the recovery instruction at the moment it is true.
///
/// The reclaiming branch fires only when a leftover carries the running
/// process's own pid, which means either a crashed predecessor whose pid the OS
/// handed back to us, or a live peer in another pid namespace. That is a narrow
/// window on purpose, for the reason ADR-0024 records for the CLI's publication
/// temps: **a foreign pid is not evidence of death**, and this backend has no
/// lock at all, so declining to touch a foreign name is the only protection a
/// concurrent peer gets. Deleting a peer's temp mid-compaction is a worse
/// outcome than an inert file — an inert file is inert (replay opens
/// `transync-cache.jsonl` by name, the byte budget measures the live set, and
/// nothing else in the program ever reads a cache directory's other files).
///
/// So, the rule:
///
/// - **Own pid, and no live [`TempLog`] in this process holds the name:
///   reclaimed.** The registry check is what makes the branch sound within one
///   process. Without it, a second `DiskCache::open_with` on this directory
///   from another thread would read its own pid off a sibling's in-flight temp
///   and unlink a file that sibling is still writing (adversarial review of ti
///   `d51ed7`). [`LIVE_COMPACTION_TEMPS`] closes that exactly: registration
///   precedes file creation, so an in-flight name is never a candidate here.
/// - **Own pid, name held: skipped, silently.** It is a compaction in flight,
///   not a leftover.
/// - **Foreign pid: left in place, and named once.** Per ADR-0024's rule above.
///   The warning carries the count, the bytes and the glob, because the file is
///   inert and deleting it is the operator's call.
///
/// **The residue this cannot decide.** Two peers in different pid namespaces
/// (two containers over one shared cache volume, both pid 1) look identical to
/// one process's crashed predecessor, and `std` offers nothing portable that
/// tells them apart — no boot id, no process start time, no file identity
/// (`file_index` is documented as not guaranteed; ADR-0024 declined the same
/// path for the same reason). So that case is not guarded here. What it costs
/// is bounded instead, at the other end: `compact_log` treats a `NotFound` on
/// its `rename` as "someone took my temp", leaves the existing log alone and
/// returns `Ok`. The peer therefore skips one compaction rather than losing its
/// cache to a [`CacheError`], which keeps contracts.md §1's bound — a
/// single-writer violation costs entries, never a failed run — true.
///
/// Best-effort throughout: a directory that cannot be read, or a file that
/// cannot be removed, must never fail an open over housekeeping.
fn sweep_compaction_temps(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let own_pid = std::process::id();
    let mut reclaimed = 0u64;
    let mut foreign = 0u64;
    let mut foreign_bytes = 0u64;

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(compaction_temp_pid) else {
            continue;
        };
        if pid == own_pid {
            // A name a live `TempLog` in this process is holding is a
            // compaction in flight, not a leftover. Unlinking it would fail
            // that compaction's rename.
            if name
                .to_str()
                .is_some_and(|n| live_compaction_temps().contains_key(n))
            {
                continue;
            }
            if std::fs::remove_file(entry.path()).is_ok() {
                reclaimed += 1;
            }
        } else {
            foreign += 1;
            foreign_bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }

    if reclaimed > 0 {
        tracing::info!(
            target: "transync::cache",
            "cache dir {}: removed {reclaimed} abandoned compaction temp file(s) left under \
             this process id",
            dir.display()
        );
    }
    if foreign > 0 {
        tracing::warn!(
            target: "transync::cache",
            "cache dir {}: {foreign} compaction temp file(s) ({foreign_bytes} bytes) under other \
             process id(s) — a run killed mid-compaction leaves one, and another process's pid is \
             no evidence that it died, so nothing here removes them and they will not go away on \
             their own. They are inert (only {LOG_FILE_NAME} is ever read, and they do not count \
             against the byte budget); delete them by hand when no run is active: \
             {LOG_FILE_NAME}{COMPACT_TEMP_INFIX}*",
            dir.display()
        );
    }
}

/// Make a rename in `dir` durable, best-effort.
///
/// The syscall is `fsync` on a directory handle, which is the portable-enough
/// way to say "the name change itself, not just the bytes it points at". Some
/// platforms and filesystems refuse to open a directory as a file at all; the
/// compaction has already succeeded by then, and a cache that failed an open
/// over a hardening step would be worse than one that is merely as durable as
/// the platform allows.
fn sync_dir(dir: &Path) {
    if let Ok(handle) = File::open(dir) {
        let _ = handle.sync_all();
    }
}

/// Read `path` once and fold its records into an index (DCR-0028 §2).
///
/// Records apply in order and **later wins**; an `evict` removes. The three
/// recovery paths (torn tail, unreadable line, unusable header) are decided
/// here because each one is a property of the file as a whole; [`scan_log`]
/// reads the bytes and this function acts on what it found, so every repair
/// runs after the read handle is closed.
fn replay_log(path: &Path) -> Result<Replayed, CacheError> {
    let empty = |write_header| Replayed {
        entries: HashMap::new(),
        meta: HashMap::new(),
        glossary: HashMap::new(),
        meta_bytes: 0,
        dead_records: 0,
        physical_bytes: 0,
        write_header,
    };

    let file = match File::open(path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(empty(true)),
        Err(e) => {
            return Err(CacheError::Io(format!(
                "read cache log {}: {e}",
                path.display()
            )));
        }
    };
    let scan = scan_log(file, path)?;

    // Torn tail: a crash mid-append leaves bytes with no terminating newline.
    // They are discarded — every line is self-contained, so nothing before the
    // tear is affected — and the file is truncated to the last complete record
    // *if it stays in place*. A file about to be rotated aside is not
    // truncated: rotation exists to hand the operator every byte that was
    // there. So the tear is reported below, by whichever repair actually runs;
    // a single warning ahead of the header decision could only describe one of
    // the two and would misname the other.
    match scan.header {
        // No non-empty line at all. A **zero-byte** file is the ordinary
        // "created but never written" case and starts clean where it is,
        // silently. Any other file that gets here carries bytes and still has
        // no header — nothing but blank lines, or nothing but a torn record —
        // so it is preserved like any other file this build cannot read,
        // rather than being written past. Blank lines are the case that only a
        // byte count separates from the empty one, and the same rule covers
        // both: bytes with no header rotate (contracts.md §1).
        Header::Missing => {
            if scan.complete_bytes == 0 && scan.torn_bytes == 0 {
                return Ok(empty(true));
            }
            // One warning, naming the rotation that is about to happen. A file
            // whose only bytes are a torn record gets the tear named too, but
            // never as a truncation: it is preserved whole, tail included.
            if scan.torn_bytes > 0 {
                tracing::warn!(
                    target: "transync::cache",
                    "cache log {} holds no complete format-{CACHE_DISK_FORMAT_VERSION} record, only \
                     {} byte(s) of a torn one; rotating it aside whole and starting empty",
                    path.display(),
                    scan.torn_bytes
                );
            } else {
                tracing::warn!(
                    target: "transync::cache",
                    "cache log {} has no readable format-{CACHE_DISK_FORMAT_VERSION} header; rotating \
                     it aside and starting empty",
                    path.display()
                );
            }
            rotate_aside(path)?;
            return Ok(empty(true));
        }
        // The header must be the first record. Anything else — a missing
        // header, a foreign file, a format this build does not know — rotates
        // the file aside and starts empty. Never a migration (ADR-0021).
        Header::Unusable => {
            tracing::warn!(
                target: "transync::cache",
                "cache log {} has no readable format-{CACHE_DISK_FORMAT_VERSION} header; rotating \
                 it aside and starting empty",
                path.display()
            );
            rotate_aside(path)?;
            return Ok(empty(true));
        }
        Header::Ok => {}
    }
    // The file stays in place, so here — and only here — the tear really is
    // repaired by a truncation.
    if scan.torn_bytes > 0 {
        tracing::warn!(
            target: "transync::cache",
            "cache log {} ends mid-record ({} trailing byte(s)); the partial record is \
             discarded and the file truncated to the last complete one",
            path.display(),
            scan.torn_bytes
        );
        truncate_to(path, scan.complete_bytes)?;
    }

    if scan.skipped > 0 {
        tracing::warn!(
            target: "transync::cache",
            "cache log {}: skipped {} unreadable or unknown record(s); the rest replayed",
            path.display(),
            scan.skipped
        );
    }

    let live = (scan.entries.len() + scan.meta.len() + scan.glossary.len()) as u64;
    Ok(Replayed {
        entries: scan.entries,
        meta: scan.meta,
        glossary: scan.glossary,
        meta_bytes: scan.meta_bytes,
        dead_records: scan.total_records.saturating_sub(live),
        physical_bytes: scan.complete_bytes,
        write_header: false,
    })
}

/// What the first non-empty line of a log turned out to be.
enum Header {
    /// A format-1 header: every non-empty line after it is a record.
    Ok,
    /// Something else — a foreign file, a future format, a headerless one.
    Unusable,
    /// The file held no non-empty line at all.
    Missing,
}

/// One streaming pass over the log file.
///
/// The file is read **a line at a time** through a `BufReader` rather than
/// slurped into one `Vec`, so replay's peak memory is the index it is building
/// plus one record — never the index *plus* a byte-for-byte copy of every dead
/// record the file still carries. That distinction is the whole point of an
/// append-only log: between compactions the file may be many times the size of
/// the live set it encodes, and a cache is an accelerator that must not be the
/// largest allocation in the process (R0004-0024). Reading it once at open is
/// unchanged (DCR-0028 §2) — this is how the read is performed, not how often.
///
/// The handle is consumed and dropped here, so the caller can truncate or
/// rename the file without a reader still open on it.
fn scan_log(file: File, path: &Path) -> Result<Scan, CacheError> {
    let mut reader = BufReader::new(file);
    let mut line: Vec<u8> = Vec::new();

    let mut entries: HashMap<CacheKey, LiveEntry> = HashMap::new();
    let mut meta: HashMap<DocumentMetaKey, DocumentMeta> = HashMap::new();
    let mut meta_bytes: HashMap<DocumentMetaKey, u64> = HashMap::new();
    let mut glossary: HashMap<GlossaryExtractionKey, GlossaryExtraction> = HashMap::new();
    let mut glossary_bytes: HashMap<GlossaryExtractionKey, u64> = HashMap::new();
    let mut header = Header::Missing;
    let mut skipped: usize = 0;
    // Every record after the header, counted so the compaction trigger can ask
    // how much of the file is no longer live.
    let mut total_records: u64 = 0;
    let mut complete_bytes: u64 = 0;
    let mut torn_bytes: u64 = 0;

    loop {
        line.clear();
        let read = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| CacheError::Io(format!("read cache log {}: {e}", path.display())))?;
        if read == 0 {
            break;
        }
        if line.last() != Some(&b'\n') {
            // Bytes with no terminating newline, which can only be the last of
            // the file: the torn tail.
            torn_bytes = read as u64;
            break;
        }
        complete_bytes += read as u64;
        // The record's own footprint in the file, newline included.
        let bytes = read as u64;
        let record = &line[..read - 1];
        if record.is_empty() {
            continue;
        }
        if matches!(header, Header::Missing) {
            match parse_line(record) {
                Some(Record::Header { format }) if format == CACHE_DISK_FORMAT_VERSION => {
                    header = Header::Ok;
                    continue;
                }
                // Nothing after an unusable header is ours to read, so the scan
                // stops here rather than parsing a foreign file to its end.
                _ => {
                    header = Header::Unusable;
                    break;
                }
            }
        }
        total_records += 1;
        let seq = total_records;
        match parse_line(record) {
            Some(Record::Entry { k, v }) => {
                entries.insert(
                    k,
                    LiveEntry {
                        value: Arc::new(v),
                        seq,
                        bytes,
                    },
                );
            }
            Some(Record::Evict { k }) => {
                entries.remove(&k);
            }
            Some(Record::DocMeta { k, v }) => {
                meta_bytes.insert(k.clone(), bytes);
                meta.insert(k, v);
            }
            Some(Record::Glossary { k, v }) => {
                glossary_bytes.insert(k.clone(), bytes);
                glossary.insert(k, v);
            }
            // A second header is not a record the grammar places here; treat it
            // like any other line this build cannot use.
            Some(Record::Header { .. }) | None => skipped += 1,
        }
    }

    Ok(Scan {
        entries,
        meta,
        glossary,
        meta_bytes: meta_bytes.values().sum::<u64>() + glossary_bytes.values().sum::<u64>(),
        header,
        skipped,
        total_records,
        complete_bytes,
        torn_bytes,
    })
}

/// What one pass over the file read, before any repair decision is taken.
struct Scan {
    entries: HashMap<CacheKey, LiveEntry>,
    meta: HashMap<DocumentMetaKey, DocumentMeta>,
    glossary: HashMap<GlossaryExtractionKey, GlossaryExtraction>,
    /// Serialized size of the live document-scoped records, newlines included.
    meta_bytes: u64,
    header: Header,
    skipped: usize,
    total_records: u64,
    /// Bytes belonging to complete, newline-terminated lines.
    complete_bytes: u64,
    /// Bytes after the last newline: a crash mid-append, or nothing.
    torn_bytes: u64,
}

fn parse_line(line: &[u8]) -> Option<Record> {
    serde_json::from_slice::<Record>(line).ok()
}

fn truncate_to(path: &Path, len: u64) -> Result<(), CacheError> {
    let file = OpenOptions::new().write(true).open(path).map_err(|e| {
        CacheError::Io(format!(
            "open cache log {} to truncate: {e}",
            path.display()
        ))
    })?;
    file.set_len(len)
        .map_err(|e| CacheError::Io(format!("truncate cache log {}: {e}", path.display())))
}

/// Move an unusable log out of the way, preserving it for an operator rather
/// than deleting it, and leave the directory ready for a fresh log.
///
/// The timestamp suffix can repeat within one second, so a taken name is
/// disambiguated rather than silently overwritten — the whole point of rotating
/// instead of deleting is that the bytes survive.
fn rotate_aside(path: &Path) -> Result<(), CacheError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let base = format!("{LOG_FILE_NAME}.unreadable-{stamp}");
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut candidate = dir.join(&base);
    let mut n = 1u32;
    while candidate.exists() {
        candidate = dir.join(format!("{base}.{n}"));
        n += 1;
    }
    std::fs::rename(path, &candidate).map_err(|e| {
        CacheError::Io(format!(
            "rotate unreadable cache log {} aside: {e}",
            path.display()
        ))
    })
}

impl Cache for DiskCache {
    /// A map lookup, never a disk read. The `Arc` clone happens under the
    /// lock and the payload copy after it drops — the same R0003-0049
    /// discipline `InMemoryCache` uses.
    fn get(&self, key: &CacheKey) -> Result<Option<UnitResult>, CacheError> {
        let hit: Option<Arc<UnitResult>> = self.locked().entries.get(key).cloned();
        Ok(hit.map(|shared| (*shared).clone()))
    }

    /// Append, flush, then index. Ordering is deliberate: if the append fails
    /// the entry is not durable, and indexing it anyway would let this process
    /// serve a result the next open cannot see.
    fn put(&self, key: CacheKey, value: UnitResult) -> Result<(), CacheError> {
        let shared = Arc::new(value);
        let mut state = self.locked();
        write_record(
            &mut state.writer,
            &Record::Entry {
                k: key.clone(),
                v: (*shared).clone(),
            },
        )?;
        state.entries.insert(key, shared);
        Ok(())
    }

    /// Drop from the index first, then record the eviction.
    ///
    /// The reverse of `put`'s order, on purpose: eviction exists because a
    /// result was *disqualified* (contracts.md §5a), so the safe direction
    /// under a failing append is a process that has already stopped serving it.
    /// A failed append is reported and leaves the disqualified entry in the log
    /// — the pipeline's `evict` helper says exactly that ("stale entry may
    /// replay"), and the replayed entry is re-validated on the run that reads
    /// it.
    fn evict(&self, key: &CacheKey) -> Result<(), CacheError> {
        let mut state = self.locked();
        state.entries.remove(key);
        write_record(&mut state.writer, &Record::Evict { k: key.clone() })
    }

    fn get_document_meta(&self, key: &DocumentMetaKey) -> Result<Option<DocumentMeta>, CacheError> {
        Ok(self.locked().meta.get(key).cloned())
    }

    /// Last write wins, on disk as in memory (DCR-0028 §3): a superseding
    /// record shadows its predecessor on replay, and there is no metadata
    /// eviction.
    fn put_document_meta(
        &self,
        key: DocumentMetaKey,
        meta: DocumentMeta,
    ) -> Result<(), CacheError> {
        let mut state = self.locked();
        write_record(
            &mut state.writer,
            &Record::DocMeta {
                k: key.clone(),
                v: meta.clone(),
            },
        )?;
        state.meta.insert(key, meta);
        Ok(())
    }

    fn get_glossary_extraction(
        &self,
        key: &GlossaryExtractionKey,
    ) -> Result<Option<GlossaryExtraction>, CacheError> {
        Ok(self.locked().glossary.get(key).cloned())
    }

    /// The harvest becomes durable here (ti `dca5bf`): a second *process* over
    /// the same document and profile replays it and makes no provider call at
    /// all. Same append-then-index order and same last-write-wins rule as
    /// `put_document_meta`, and no eviction.
    fn put_glossary_extraction(
        &self,
        key: GlossaryExtractionKey,
        value: GlossaryExtraction,
    ) -> Result<(), CacheError> {
        let mut state = self.locked();
        write_record(
            &mut state.writer,
            &Record::Glossary {
                k: key.clone(),
                v: value.clone(),
            },
        )?;
        state.glossary.insert(key, value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::InMemoryCache;
    use crate::id::BlockId;
    use crate::llm::{OutputKind, ProviderFingerprint};

    /// A unique scratch directory under `/Volumes/Temp/claude` when it exists
    /// (per this repo's convention — same helper shape as the CLI smokes),
    /// falling back to the platform temp dir elsewhere.
    ///
    /// It removes itself on drop rather than at the end of each test: a test
    /// that panics never reaches its last statement, so an end-of-test
    /// `remove_dir_all` cleans up exactly the runs that did not need it and
    /// leaks exactly the ones that did — and the scratch volume filling up
    /// lands on whoever runs the suite next, reading like a regression in the
    /// code under test (ticket `2fef6a`). Removal is best-effort so it can
    /// never turn a passing test red or abort an unwind.
    ///
    /// The name carries a process-wide counter as well as the clock, because
    /// two threads under `--test-threads=N` can read the same nanosecond and a
    /// self-removing guard must never own a directory another guard also owns.
    struct ScratchDir {
        path: PathBuf,
    }

    static NEXT_SCRATCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn scratch(prefix: &str) -> ScratchDir {
        let preferred = Path::new("/Volumes/Temp/claude");
        let root = if preferred.exists() {
            preferred.to_path_buf()
        } else {
            std::env::temp_dir()
        };
        let unique = format!(
            "{prefix}-{pid}-{nanos}-{n}",
            pid = std::process::id(),
            nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            n = NEXT_SCRATCH.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        );
        let path = root.join(unique);
        std::fs::create_dir_all(&path).expect("scratch dir should be creatable");
        ScratchDir { path }
    }

    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }

    impl std::ops::Deref for ScratchDir {
        type Target = Path;

        fn deref(&self) -> &Path {
            &self.path
        }
    }

    impl AsRef<Path> for ScratchDir {
        fn as_ref(&self) -> &Path {
            &self.path
        }
    }

    fn key(hash: u64) -> CacheKey {
        CacheKey {
            provider_fingerprint: ProviderFingerprint::from_type_name("disk-test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            source_hash: hash,
            source_lang: "en".to_string(),
            target_lang: "ko".to_string(),
            profile_version: "v1".to_string(),
            profile_prompt_hash: 11,
            glossary_hash: 22,
            model_id: "test-model".to_string(),
            block_kind: "paragraph".to_string(),
            input_mode: "text_fragment".to_string(),
            context_hash: 33,
            instruction_hash: 44,
        }
    }

    fn result(id: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId(id.to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: format!("payload:{id}"),
            warnings: vec![format!("warn:{id}")],
        }
    }

    fn meta_key(hash: u64) -> DocumentMetaKey {
        DocumentMetaKey {
            provider_fingerprint: ProviderFingerprint::from_type_name("disk-test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            model_id: "test-model".to_string(),
            source_lang: "auto".to_string(),
            target_lang: "ko".to_string(),
            doc_source_hash: hash,
        }
    }

    fn glossary_key(request_hash: u64) -> GlossaryExtractionKey {
        GlossaryExtractionKey {
            provider_fingerprint: ProviderFingerprint::from_type_name("disk-test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            model_id: "test-model".to_string(),
            request_hash,
        }
    }

    fn extraction(term: &str) -> GlossaryExtraction {
        GlossaryExtraction {
            terms: vec![crate::llm::GlossaryEntry {
                source_term: term.to_string(),
                target_term: format!("{term}-ko"),
                note: None,
                scope: crate::llm::GlossaryScope::GlobalAcrossDocument,
                sections: Vec::new(),
            }],
        }
    }

    /// `GlossaryExtraction` carries no `PartialEq` (its entries are a wire type
    /// that has none), so assertions compare the source terms.
    fn terms_of(extraction: Option<GlossaryExtraction>) -> Vec<String> {
        extraction
            .expect("a record was stored")
            .terms
            .into_iter()
            .map(|e| e.source_term)
            .collect()
    }

    fn log_path(dir: &Path) -> PathBuf {
        dir.join(LOG_FILE_NAME)
    }

    /// Names of the logs `rotate_aside` has preserved in `dir`.
    fn rotated_names(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".unreadable-"))
            .collect()
    }

    /// Names in `dir` that look like a compaction temp file, whoever wrote them.
    fn compaction_temp_names(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".compact-"))
            .collect()
    }

    /// A process id this process cannot have, for the leftovers the sweep must
    /// leave alone. Derived rather than typed: a hard-coded low number is a pid
    /// a container can really be running under, and the whole assertion below
    /// would then invert.
    fn foreign_pid() -> u32 {
        std::process::id().wrapping_add(1)
    }

    /// The cross-process determinism pin: a second `open` over the same
    /// directory reconstructs exactly what the first one wrote — entries by
    /// full `CacheKey` equality, an eviction as an absence, and the
    /// document-level record — with no shared in-memory state between them.
    ///
    /// This is the property the whole design rests on. Had any `CacheKey` axis
    /// used std's randomized `DefaultHasher`, a key written by one run would
    /// not be reproducible by the next and a disk cache would be impossible;
    /// every axis routes through `id::source_hash_bytes` instead.
    #[test]
    fn two_opens_round_trip_entries_evictions_and_metadata() {
        let dir = scratch("transync-diskcache-roundtrip");

        {
            let cache = DiskCache::open(&dir).expect("opens");
            cache.put(key(1), result("one")).unwrap();
            cache.put(key(2), result("two")).unwrap();
            cache.put(key(3), result("three")).unwrap();
            cache.evict(&key(2)).unwrap();
            cache
                .put_document_meta(
                    meta_key(9),
                    DocumentMeta {
                        detected_source_language: Some("en".to_string()),
                    },
                )
                .unwrap();
            assert_eq!(cache.len(), 2);
        }

        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(reopened.len(), 2, "the evicted entry does not come back");
        let one = reopened
            .get(&key(1))
            .unwrap()
            .expect("a key written by the first open resolves in the second");
        assert_eq!(one.unit_id, BlockId("one".to_string()));
        assert_eq!(one.translated_payload, "payload:one");
        assert_eq!(one.output_kind, OutputKind::Translated);
        assert_eq!(one.warnings, vec!["warn:one".to_string()]);
        assert!(reopened.get(&key(2)).unwrap().is_none(), "evict is durable");
        assert!(reopened.get(&key(3)).unwrap().is_some());
        assert_eq!(
            reopened
                .get_document_meta(&meta_key(9))
                .unwrap()
                .expect("the document record survives the process")
                .detected_source_language
                .as_deref(),
            Some("en")
        );
        assert_eq!(
            reopened.get_document_meta(&meta_key(8)).unwrap(),
            None,
            "a different document is a different record"
        );
    }

    /// Last write wins on replay, for both spaces.
    #[test]
    fn a_superseding_record_shadows_its_predecessor() {
        let dir = scratch("transync-diskcache-supersede");
        {
            let cache = DiskCache::open(&dir).expect("opens");
            cache.put(key(1), result("first")).unwrap();
            cache.put(key(1), result("second")).unwrap();
            cache
                .put_document_meta(
                    meta_key(9),
                    DocumentMeta {
                        detected_source_language: Some("en".to_string()),
                    },
                )
                .unwrap();
            cache
                .put_document_meta(
                    meta_key(9),
                    DocumentMeta {
                        detected_source_language: Some("fr".to_string()),
                    },
                )
                .unwrap();
        }
        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(
            reopened.get(&key(1)).unwrap().unwrap().translated_payload,
            "payload:second"
        );
        assert_eq!(
            reopened
                .get_document_meta(&meta_key(9))
                .unwrap()
                .unwrap()
                .detected_source_language
                .as_deref(),
            Some("fr")
        );
    }

    /// ti `dca5bf`: the glossary harvest is durable across processes too, and
    /// under the same rules as a metadata record — last write wins, a
    /// different key is a different record, and nothing evicts it.
    ///
    /// This is the property that makes a second *process* over one document
    /// make zero provider calls: without it the preflight would be re-bought
    /// on every fresh run, whatever the unit entries did.
    #[test]
    fn a_glossary_harvest_survives_the_process() {
        let dir = scratch("transync-diskcache-glossary");
        {
            let cache = DiskCache::open(&dir).expect("opens");
            cache
                .put_glossary_extraction(glossary_key(7), extraction("tensor"))
                .unwrap();
            cache
                .put_glossary_extraction(glossary_key(7), extraction("agent"))
                .unwrap();
            cache
                .put_glossary_extraction(glossary_key(8), GlossaryExtraction { terms: Vec::new() })
                .unwrap();
            assert_eq!(cache.len(), 0, "harvests are not unit entries");
        }

        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(
            terms_of(reopened.get_glossary_extraction(&glossary_key(7)).unwrap()),
            vec!["agent".to_string()],
            "the harvest survives the process, and last write wins"
        );
        assert!(
            terms_of(reopened.get_glossary_extraction(&glossary_key(8)).unwrap()).is_empty(),
            "\"supported, found nothing\" replays as itself, not as a miss"
        );
        assert!(
            reopened
                .get_glossary_extraction(&glossary_key(9))
                .unwrap()
                .is_none(),
            "a different extraction request is a different record"
        );
    }

    /// A crash mid-append leaves a partial final line. The torn record is
    /// dropped, the file is truncated to the last complete one, and every
    /// earlier record survives — the property that makes self-contained lines
    /// worth their duplication (R0003-0051).
    #[test]
    fn a_torn_tail_drops_exactly_the_last_record() {
        let dir = scratch("transync-diskcache-torn");
        {
            let cache = DiskCache::open(&dir).expect("opens");
            cache.put(key(1), result("one")).unwrap();
            cache.put(key(2), result("two")).unwrap();
        }

        // Simulate the crash: cut the file mid-final-record.
        let path = log_path(&dir);
        let bytes = std::fs::read(&path).unwrap();
        let last_nl = bytes[..bytes.len() - 1]
            .iter()
            .rposition(|b| *b == b'\n')
            .expect("three records were written");
        let cut = last_nl + 1 + (bytes.len() - last_nl) / 2;
        truncate_to(&path, cut as u64).unwrap();

        let reopened = DiskCache::open(&dir).expect("a torn log opens, it never errors");
        assert_eq!(reopened.len(), 1, "exactly the torn record is gone");
        assert!(reopened.get(&key(1)).unwrap().is_some());
        assert!(reopened.get(&key(2)).unwrap().is_none());

        // The file itself was repaired, so the next open is clean and the
        // appends that follow the tear are readable.
        reopened.put(key(3), result("three")).unwrap();
        drop(reopened);
        let third = DiskCache::open(&dir).expect("reopens");
        assert_eq!(third.len(), 2);
        assert!(third.get(&key(3)).unwrap().is_some());
    }

    /// A record this build cannot read is skipped, not fatal — and the records
    /// around it still replay. This is the forward-tolerance format 1 promises
    /// for additive record kinds.
    #[test]
    fn an_unreadable_line_is_skipped_and_its_neighbours_replay() {
        let dir = scratch("transync-diskcache-skip");
        {
            let cache = DiskCache::open(&dir).expect("opens");
            cache.put(key(1), result("one")).unwrap();
        }
        let path = log_path(&dir);
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push_str("{\"t\":\"from_the_future\",\"whatever\":1}\n");
        text.push_str("this is not JSON at all\n");
        std::fs::write(&path, &text).unwrap();
        {
            let cache = DiskCache::open(&dir).expect("opens despite the noise");
            assert_eq!(cache.len(), 1, "the readable entry replayed");
            cache.put(key(2), result("two")).unwrap();
        }
        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(
            reopened.len(),
            2,
            "an append after unreadable lines is itself readable"
        );
    }

    /// A file that is not a format-1 log — a foreign file, or one written by a
    /// future format — is rotated aside and the run proceeds against an empty
    /// cache. Never a hard error, never a migration, and never a deletion: the
    /// operator's bytes are still on disk under the rotated name.
    #[test]
    fn a_foreign_format_is_rotated_aside_and_the_cache_starts_empty() {
        let dir = scratch("transync-diskcache-foreign");
        let path = log_path(&dir);
        std::fs::write(
            &path,
            "{\"t\":\"header\",\"format\":99}\n{\"t\":\"entry\",\"k\":{},\"v\":{}}\n",
        )
        .unwrap();

        let cache = DiskCache::open(&dir).expect("an unknown format opens empty, it never errors");
        assert!(cache.is_empty());
        cache.put(key(1), result("one")).unwrap();
        drop(cache);

        let rotated: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".unreadable-"))
            .collect();
        assert_eq!(rotated.len(), 1, "the old file was preserved, not deleted");
        let preserved = std::fs::read_to_string(dir.join(&rotated[0])).unwrap();
        assert!(preserved.starts_with("{\"t\":\"header\",\"format\":99}"));

        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(reopened.len(), 1, "the fresh log is a normal format-1 log");
    }

    /// A header-less file gets the same treatment as a foreign one: the header
    /// is the only thing that identifies the grammar, so its absence means the
    /// file is not one of ours.
    #[test]
    fn a_headerless_file_is_rotated_aside() {
        let dir = scratch("transync-diskcache-headerless");
        std::fs::write(log_path(&dir), "not a cache log\n").unwrap();
        let cache = DiskCache::open(&dir).expect("opens empty");
        assert!(cache.is_empty());
    }

    /// A file of nothing but blank lines carries no header either, so it takes
    /// the same path as any other log this build cannot read: rotated aside and
    /// preserved, never written past. The **zero-byte** file is the single
    /// exception — "created but never written" is the ordinary case and starts
    /// clean where it is. Both halves are asserted together because they are
    /// two sides of one branch, and only the byte count separates them.
    #[test]
    fn a_blank_lines_only_file_is_rotated_aside_but_a_zero_byte_one_is_not() {
        let dir = scratch("transync-diskcache-blank");
        let path = log_path(&dir);
        std::fs::write(&path, "\n\n\n").unwrap();
        {
            let cache = DiskCache::open(&dir).expect("opens empty, it never errors");
            assert!(cache.is_empty());
            cache.put(key(1), result("one")).unwrap();
        }

        let rotated = rotated_names(&dir);
        assert_eq!(
            rotated.len(),
            1,
            "the blank file was preserved, not written past"
        );
        assert_eq!(
            std::fs::read_to_string(dir.join(&rotated[0])).unwrap(),
            "\n\n\n",
            "rotation hands the operator every byte that was there"
        );
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .starts_with("{\"t\":\"header\""),
            "the log left in place is a fresh one, header first"
        );
        assert_eq!(
            DiskCache::open(&dir).expect("reopens").len(),
            1,
            "and it replays as a normal format-1 log"
        );

        let zero = scratch("transync-diskcache-zero-byte");
        std::fs::write(log_path(&zero), "").unwrap();
        {
            let cache = DiskCache::open(&zero).expect("opens");
            assert!(cache.is_empty());
            cache.put(key(1), result("one")).unwrap();
        }
        assert!(
            rotated_names(&zero).is_empty(),
            "a never-written log is not a corrupt one"
        );
        assert_eq!(DiskCache::open(&zero).expect("reopens").len(), 1);
    }

    /// Every `tracing` message a closure emits on this thread, in order.
    ///
    /// The recovery paths differ in what they tell the operator as much as in
    /// what they do to the file, and a message that names the wrong repair is
    /// exactly as misleading as a wrong repair would be. Capturing needs a
    /// `Subscriber`, and the library installs none (only the CLI does), so the
    /// test builds the smallest one that records a message field — no
    /// `tracing-subscriber` dependency, and thread-local, so a parallel suite
    /// is unaffected.
    fn warnings_from(f: impl FnOnce()) -> Vec<String> {
        use std::sync::{Arc as StdArc, Mutex};

        struct Sink(StdArc<Mutex<Vec<String>>>);
        struct Message(String);

        impl tracing::field::Visit for Message {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.0 = format!("{value:?}");
                }
            }
        }

        impl tracing::Subscriber for Sink {
            fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }

            fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

            fn event(&self, event: &tracing::Event<'_>) {
                let mut message = Message(String::new());
                event.record(&mut message);
                self.0.lock().unwrap().push(message.0);
            }

            fn enter(&self, _: &tracing::span::Id) {}

            fn exit(&self, _: &tracing::span::Id) {}
        }

        let captured = StdArc::new(Mutex::new(Vec::new()));
        tracing::subscriber::with_default(Sink(StdArc::clone(&captured)), f);
        captured.lock().unwrap().clone()
    }

    /// A file whose only bytes are a torn record is **rotated aside whole** —
    /// it has no header, so it cannot stay in place — and the operator must be
    /// told that, not told the file was truncated to its last complete record.
    /// Only the readable log below is repaired by a truncation, and each path
    /// now names the repair that actually ran.
    #[test]
    fn a_torn_only_log_is_reported_as_rotated_and_a_readable_one_as_truncated() {
        let torn_only = scratch("transync-diskcache-torn-only");
        let partial = "{\"t\":\"header\",\"format\":1";
        std::fs::write(log_path(&torn_only), partial).unwrap();
        let messages = warnings_from(|| {
            assert!(
                DiskCache::open(&torn_only)
                    .expect("a torn-only log opens, it never errors")
                    .is_empty()
            );
        });

        assert!(
            messages.iter().any(|m| m.contains("rotating it aside")),
            "the operator hears about the rotation that ran: {messages:?}"
        );
        assert!(
            !messages.iter().any(|m| m.contains("truncat")),
            "nothing may claim a truncation on a file that was preserved whole: {messages:?}"
        );
        let rotated = rotated_names(&torn_only);
        assert_eq!(rotated.len(), 1, "and the file really was rotated");
        assert_eq!(
            std::fs::read_to_string(torn_only.join(&rotated[0])).unwrap(),
            partial,
            "preserved whole, torn tail included — which is why no message may say truncated"
        );

        // The other side of the same branch: a readable log with a torn tail
        // stays in place and *is* truncated, so that message is still earned.
        let readable = scratch("transync-diskcache-torn-readable");
        {
            let cache = DiskCache::open(&readable).expect("opens");
            cache.put(key(1), result("one")).unwrap();
        }
        let path = log_path(&readable);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(b"{\"t\":\"entry\",\"k\":{\"prov");
        std::fs::write(&path, &bytes).unwrap();
        let messages = warnings_from(|| {
            assert_eq!(DiskCache::open(&readable).expect("reopens").len(), 1);
        });
        assert!(
            messages
                .iter()
                .any(|m| m.contains("truncated to the last complete one")),
            "the in-place repair is a truncation and says so: {messages:?}"
        );
        assert!(
            rotated_names(&readable).is_empty(),
            "and a readable log is never rotated"
        );
    }

    /// Options with both budgets off — the shape every recovery and round-trip
    /// test above wants, so a byte budget cannot silently perturb them.
    fn unbounded() -> DiskCacheOptions {
        DiskCacheOptions {
            max_bytes: None,
            ..DiskCacheOptions::default()
        }
    }

    /// The shipped default is a byte budget and no entry cap (DCR-0028 §4).
    #[test]
    fn the_default_policy_is_a_byte_budget_and_no_entry_cap() {
        let o = DiskCacheOptions::default();
        assert_eq!(o.max_bytes, Some(1024 * 1024 * 1024), "1 GiB");
        assert_eq!(o.max_entries, None);
    }

    /// Over-budget logs trim to budget, **oldest-written first** — so what
    /// survives is what a run wrote most recently, which under an append log is
    /// the recency signal that costs nothing to maintain.
    #[test]
    fn an_over_budget_log_keeps_the_newest_entries() {
        let dir = scratch("transync-diskcache-trim");
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            for i in 1..=6 {
                cache.put(key(i), result(&format!("r{i}"))).unwrap();
            }
            cache
                .put_document_meta(
                    meta_key(9),
                    DocumentMeta {
                        detected_source_language: Some("en".to_string()),
                    },
                )
                .unwrap();
            cache
                .put_glossary_extraction(glossary_key(9), extraction("tensor"))
                .unwrap();
        }

        let opts = DiskCacheOptions {
            max_entries: Some(2),
            ..DiskCacheOptions::default()
        };
        let trimmed = DiskCache::open_with(&dir, opts).expect("reopens");
        assert_eq!(trimmed.len(), 2, "trimmed down to the entry budget");
        for i in 1..=4 {
            assert!(
                trimmed.get(&key(i)).unwrap().is_none(),
                "entry {i} was written earliest and must be the first to go"
            );
        }
        assert!(trimmed.get(&key(5)).unwrap().is_some());
        assert!(trimmed.get(&key(6)).unwrap().is_some());
        assert!(
            trimmed.get_document_meta(&meta_key(9)).unwrap().is_some(),
            "document-metadata records are exempt from the capacity trim"
        );
        assert!(
            trimmed
                .get_glossary_extraction(&glossary_key(9))
                .unwrap()
                .is_some(),
            "so is the glossary harvest — the other document-scoped record"
        );
        drop(trimmed);

        // The trim is durable: it rewrote the log rather than only the index.
        let reopened = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(reopened.len(), 2);
        assert!(reopened.get(&key(1)).unwrap().is_none());
        assert!(reopened.get_document_meta(&meta_key(9)).unwrap().is_some());
        assert!(
            reopened
                .get_glossary_extraction(&glossary_key(9))
                .unwrap()
                .is_some(),
            "the compaction the trim forced carried the harvest across"
        );
    }

    /// Budgets of `None` never trim, however long the log grows.
    #[test]
    fn budgets_of_none_never_trim() {
        let dir = scratch("transync-diskcache-nobudget");
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            for i in 1..=20 {
                cache.put(key(i), result(&format!("r{i}"))).unwrap();
            }
        }
        let reopened = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(reopened.len(), 20, "nothing was dropped");
    }

    /// Dead weight concentrated in a *few large* records compacts too. The
    /// record-count trigger cannot see this shape — two dead records against
    /// ten live ones is nowhere near "caught up" — and yet the file is almost
    /// entirely the two, so a count-only rule leaves it physically enormous
    /// open after open.
    #[test]
    fn a_log_that_is_mostly_dead_bytes_compacts_even_when_few_records_are_dead() {
        let dir = scratch("transync-diskcache-deadbytes");
        let path = log_path(&dir);
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            for i in 1..=10 {
                cache.put(key(i), result(&format!("r{i}"))).unwrap();
            }
            // One enormous entry, written and then evicted: two dead records
            // against ten live ones, and ~200 KB of the file.
            let mut huge = result("huge");
            huge.translated_payload = "x".repeat(200_000);
            cache.put(key(99), huge).unwrap();
            cache.evict(&key(99)).unwrap();
        }
        let before = std::fs::metadata(&path).unwrap().len();
        assert!(
            before > 200_000,
            "the dead weight really is in the file: {before} bytes"
        );

        let reopened = DiskCache::open_with(&dir, unbounded()).expect("reopens and compacts");
        assert_eq!(reopened.len(), 10, "every live entry survived the rewrite");
        assert!(reopened.get(&key(99)).unwrap().is_none());
        for i in 1..=10 {
            assert!(reopened.get(&key(i)).unwrap().is_some(), "entry {i}");
        }
        drop(reopened);

        let after = std::fs::metadata(&path).unwrap().len();
        assert!(
            after < 10_000,
            "the log was rewritten to its live set; {after} bytes remain of {before}"
        );
        assert_eq!(
            std::fs::read_to_string(&path).unwrap().lines().count(),
            11,
            "a header and the ten live records"
        );
    }

    /// A log whose dead weight is a *minority* of the bytes is left alone —
    /// the byte trigger is "at least half dead", not "any dead weight at all",
    /// so an ordinary run does not rewrite its whole cache on every open.
    #[test]
    fn a_log_that_is_mostly_live_bytes_is_left_in_place() {
        let dir = scratch("transync-diskcache-mostlylive");
        let path = log_path(&dir);
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            for i in 1..=10 {
                cache.put(key(i), result(&format!("r{i}"))).unwrap();
            }
            // One superseded record: dead, but a tenth of the file.
            cache.put(key(1), result("r1-again")).unwrap();
        }
        let before = std::fs::read_to_string(&path).unwrap();

        let reopened = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(reopened.len(), 10);
        drop(reopened);

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            before,
            "nothing was rewritten"
        );
    }

    /// Replay reads the file a line at a time, and the accounting that comes
    /// out of that pass is what the compaction trigger reads: every complete
    /// line counts toward the physical size — blank lines and unreadable ones
    /// included, because they really do occupy the file — while a torn tail is
    /// truncated away and counts for nothing.
    #[test]
    fn a_streamed_replay_accounts_for_every_complete_line_and_no_torn_one() {
        let dir = scratch("transync-diskcache-scan");
        let path = log_path(&dir);
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            cache.put(key(1), result("one")).unwrap();
        }
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push('\n'); // a blank line between records
        text.push_str("this is not JSON at all\n");
        let complete = text.len() as u64;
        text.push_str("{\"t\":\"entry\",\"k\":{\"prov"); // torn mid-append
        std::fs::write(&path, &text).unwrap();

        let replayed = replay_log(&path).expect("a damaged log replays, it never errors");
        assert_eq!(replayed.entries.len(), 1, "the one readable entry");
        assert_eq!(
            replayed.physical_bytes, complete,
            "the blank line and the unreadable line are in the file and are counted; \
             the torn tail is not"
        );
        assert!(
            replayed.dead_bytes() > 0 && replayed.dead_bytes() < replayed.compacted_bytes(),
            "dead weight, but a minority of it: {} dead of {} live",
            replayed.dead_bytes(),
            replayed.compacted_bytes()
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().len(),
            complete,
            "the file itself was truncated to the last complete record"
        );
    }

    /// Compaction drops superseded and evicted records: the log shrinks to
    /// exactly its live set, and the live set is unchanged by the rewrite.
    #[test]
    fn compaction_drops_superseded_and_evicted_records() {
        let dir = scratch("transync-diskcache-compact");
        let path = log_path(&dir);
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            // One live entry, and a pile of dead weight around it: five
            // supersedes of the same key, plus a written-then-evicted key.
            for i in 0..6 {
                cache.put(key(1), result(&format!("v{i}"))).unwrap();
            }
            cache.put(key(2), result("doomed")).unwrap();
            cache.evict(&key(2)).unwrap();
        }
        let before = std::fs::read_to_string(&path).unwrap().lines().count();
        assert_eq!(before, 1 + 6 + 2, "header + six puts + a put and an evict");

        let compacted = DiskCache::open_with(&dir, unbounded()).expect("reopens and compacts");
        assert_eq!(compacted.len(), 1);
        assert_eq!(
            compacted.get(&key(1)).unwrap().unwrap().translated_payload,
            "payload:v5",
            "the surviving value is the last one written"
        );
        assert!(compacted.get(&key(2)).unwrap().is_none());
        drop(compacted);

        let after = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            after.lines().count(),
            2,
            "the log is now a header and one live record: {after}"
        );
        assert!(after.starts_with("{\"t\":\"header\",\"format\":1}"));

        // And the compacted file is a normal log: it reopens and appends.
        let reopened = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(reopened.len(), 1);
        reopened.put(key(3), result("after")).unwrap();
        drop(reopened);
        assert_eq!(
            DiskCache::open_with(&dir, unbounded()).unwrap().len(),
            2,
            "an append after compaction is readable"
        );
    }

    /// R0009-0083: compaction is *reproducible*. Two caches holding the same
    /// live set — written in opposite orders, into two directories — compact
    /// to the same bytes. The unit entries always rode their replay sequence;
    /// the two document-scoped maps are `HashMap`s, and before the sort they
    /// came out in whatever order a separately-seeded table happened to
    /// produce, so one logical cache could compact two ways.
    #[test]
    fn compaction_is_byte_reproducible_for_the_same_live_set() {
        fn compacted_log(dir: &Path, hashes: &[u64]) -> String {
            {
                let cache = DiskCache::open_with(dir, unbounded()).expect("opens");
                for h in hashes {
                    cache
                        .put_document_meta(
                            meta_key(*h),
                            DocumentMeta {
                                detected_source_language: Some(format!("l{h}")),
                            },
                        )
                        .unwrap();
                    cache
                        .put_glossary_extraction(glossary_key(*h), extraction(&format!("t{h}")))
                        .unwrap();
                }
                // Dead weight, so the reopen below actually compacts: the
                // trigger wants at least as many dead records as live ones.
                for i in 0..16 {
                    cache.put(key(1), result(&format!("v{i}"))).unwrap();
                }
            }
            drop(DiskCache::open_with(dir, unbounded()).expect("reopens and compacts"));
            std::fs::read_to_string(log_path(dir)).expect("the compacted log is readable")
        }

        let forward = scratch("transync-diskcache-repro-fwd");
        let reverse = scratch("transync-diskcache-repro-rev");
        let a = compacted_log(&forward, &[1, 2, 3, 4, 5, 6]);
        let b = compacted_log(&reverse, &[6, 5, 4, 3, 2, 1]);
        assert_eq!(
            a.lines().count(),
            1 + 1 + 6 + 6,
            "header, the one live entry, and the twelve document-scoped records: {a}"
        );
        assert_eq!(
            a, b,
            "the same live set must compact to the same bytes\n--- forward ---\n{a}\n--- reverse ---\n{b}"
        );
    }

    /// A compaction that *fails* — as opposed to one interrupted by a crash —
    /// takes its temp file with it. The names carry a pid and a nanosecond
    /// stamp, so leaving them behind means one more file in the operator's
    /// cache directory every time the disk misbehaves, and the only process
    /// that ever knew those names is the one that gave up on them.
    #[test]
    fn a_failed_compaction_removes_its_temp_file() {
        let dir = scratch("transync-diskcache-tempclean");
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            cache.put(key(1), result("one")).unwrap();
            cache.put(key(2), result("two")).unwrap();
        }
        let replayed = replay_log(&log_path(&dir)).expect("replays");

        // A destination no rename can take: renaming a file onto a directory
        // fails on every platform this runs on.
        let blocked = dir.join("occupied");
        std::fs::create_dir(&blocked).unwrap();

        let err = compact_log(&dir, &blocked, &replayed)
            .expect_err("a compaction that cannot be moved into place is an error");
        assert!(matches!(err, CacheError::Io(_)), "{err:?}");

        let leftovers: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".compact-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "the temp file went with the failure: {leftovers:?}"
        );
    }

    /// A crash *during* compaction leaves the old log complete and a partial
    /// temp file beside it — never a hybrid. The next open reads the old log in
    /// full and ignores the temp, because nothing but `transync-cache.jsonl` is
    /// ever read; both files are intact on disk and each parses as what it is.
    ///
    /// The leftover here carries a **foreign** pid, so the open-time sweep
    /// leaves it alone: a pid that is not ours is not evidence that its process
    /// is dead (ADR-0024), and deleting a peer's temp mid-compaction would cost
    /// that run its cache to save this one a file.
    #[test]
    fn a_crash_mid_compaction_leaves_the_old_log_whole() {
        let dir = scratch("transync-diskcache-crash");
        let path = log_path(&dir);
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            cache.put(key(1), result("one")).unwrap();
            cache.put(key(2), result("two")).unwrap();
        }
        let original = std::fs::read_to_string(&path).unwrap();

        // What a crash between "temp created" and "rename" leaves behind.
        let temp = dir.join(format!(
            "{LOG_FILE_NAME}.compact-{pid}-1",
            pid = foreign_pid()
        ));
        let mut partial = String::from("{\"t\":\"header\",\"format\":1}\n");
        partial.push_str(original.lines().nth(1).unwrap());
        partial.push_str("{ truncated mid-record");
        std::fs::write(&temp, &partial).unwrap();

        let cache = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(cache.len(), 2, "the pre-crash log replayed in full");
        assert!(cache.get(&key(1)).unwrap().is_some());
        assert!(cache.get(&key(2)).unwrap().is_some());
        drop(cache);

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original,
            "the live log was neither truncated nor merged with the temp"
        );
        assert_eq!(
            std::fs::read_to_string(&temp).unwrap(),
            partial,
            "the abandoned temp is inert, not consumed and not deleted"
        );
    }

    /// The crash window the RAII guard cannot cover: nothing runs in a killed
    /// process, so the temp file it was holding survives it. `open` reclaims
    /// the leftovers carrying **this** process's id, the one case where a
    /// leftover can only be a dead predecessor whose pid the OS handed back
    /// (ADR-0024's rule, applied here). Without that pass every crash adds one
    /// more file — the names carry a nanosecond stamp as well as a pid, so they
    /// accumulate rather than overwrite — and nothing in the program ever looks
    /// at, replays, counts or removes them again.
    #[test]
    fn abandoned_compaction_temps_from_this_pid_are_reclaimed_at_open() {
        let dir = scratch("transync-diskcache-tempsweep");
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            cache.put(key(1), result("one")).unwrap();
        }

        // Two crashes' worth of leftovers, as the pid-plus-nanos naming leaves
        // them.
        let mine: Vec<PathBuf> = [1u128, 2]
            .iter()
            .map(|nanos| {
                dir.join(format!(
                    "{LOG_FILE_NAME}.compact-{pid}-{nanos}",
                    pid = std::process::id()
                ))
            })
            .collect();
        for path in &mine {
            std::fs::write(path, "{\"t\":\"header\",\"format\":1}\n").unwrap();
        }

        let cache = DiskCache::open_with(&dir, unbounded()).expect("reopens");
        assert_eq!(cache.len(), 1, "the live log replayed exactly as before");
        assert_eq!(
            compaction_temp_names(&dir),
            Vec::<String>::new(),
            "the leftovers of a crashed run under this pid are gone"
        );
        drop(cache);

        // And the log itself was neither swept nor disturbed by the pass.
        assert!(log_path(&dir).exists());
    }

    /// The own-pid branch must not unlink a compaction that is still running.
    ///
    /// Adversarial review of ti `d51ed7`: two `DiskCache` handles on one
    /// directory from two threads of one process both report the same pid, so
    /// the sweep read a sibling's in-flight temp as its own leftover and
    /// deleted it — the sibling's `rename` then failed and its open returned a
    /// `CacheError`. `LIVE_COMPACTION_TEMPS` decides it instead of the pid, and
    /// the two halves are asserted in one test because only their *difference*
    /// says the registry is what did the work: same directory, same name, same
    /// pid, and the only thing that changes between the two sweeps is whether a
    /// guard is alive.
    #[test]
    fn a_sweep_leaves_a_compaction_temp_this_process_is_still_holding() {
        let dir = scratch("transync-diskcache-liveguard");
        let name = compaction_temp_name(std::process::id(), 424_242);

        let held = TempLog::at(&dir, name.clone());
        std::fs::write(held.path(), "half a log").unwrap();

        sweep_compaction_temps(&dir);
        assert!(
            dir.join(&name).exists(),
            "a temp a live TempLog holds is a compaction in flight, not a leftover"
        );

        // `renamed` retires the guard without deleting the file, which is what
        // leaves a leftover of exactly the shape a crash leaves.
        held.renamed();
        sweep_compaction_temps(&dir);
        assert!(
            !dir.join(&name).exists(),
            "once nothing holds the name, the same sweep reclaims it"
        );
    }

    /// A compaction whose temp is taken out from under it costs this run its
    /// compaction, never its cache.
    ///
    /// The pid-namespace case the sweep cannot decide — two containers over one
    /// cache volume, both pid 1 — ends with a peer unlinking our temp, and
    /// `rename` reports that as `NotFound`. `compact_log` treats that kind as
    /// "someone took my temp": the existing log is complete and untouched, so
    /// it returns `Ok` and the open proceeds. Anything else keeps failing, which
    /// `a_failed_compaction_removes_its_temp_file` pins.
    ///
    /// Driven here through a destination whose parent does not exist, because
    /// that is the one deterministic way to make this `rename` answer
    /// `NotFound` from a test — in production the destination is always the
    /// log beside the temp, so a `NotFound` can only be the vanished source.
    #[test]
    fn a_compaction_whose_temp_vanished_leaves_the_log_and_does_not_fail() {
        let dir = scratch("transync-diskcache-tempstolen");
        {
            let cache = DiskCache::open_with(&dir, unbounded()).expect("opens");
            cache.put(key(1), result("one")).unwrap();
        }
        let path = log_path(&dir);
        let original = std::fs::read_to_string(&path).unwrap();
        let replayed = replay_log(&path).expect("replays");

        compact_log(
            &dir,
            &dir.join("no-such-dir").join(LOG_FILE_NAME),
            &replayed,
        )
        .expect("a temp that is gone is not this run's failure to report");

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            original,
            "the existing log is untouched, so skipping the compaction costs nothing but space"
        );
        assert_eq!(
            compaction_temp_names(&dir),
            Vec::<String>::new(),
            "and the guard still took its own temp with it"
        );
        assert!(
            DiskCache::open_with(&dir, unbounded())
                .expect("the cache still opens")
                .get(&key(1))
                .unwrap()
                .is_some(),
            "the entries survive: contracts.md §1 bounds this to lost entries, never a failed run"
        );
    }

    /// The writer and the reader of the temp name are welded: `compact_log`
    /// builds the name, the sweep parses the pid back out of it, and a drift
    /// between them would silently turn the sweep into a no-op that no
    /// directory-level test could tell from a directory with nothing to sweep.
    #[test]
    fn a_compaction_temp_name_carries_a_pid_the_sweep_reads_back() {
        assert_eq!(
            compaction_temp_pid(&compaction_temp_name(4242, 7)),
            Some(4242)
        );
        assert_eq!(
            compaction_temp_pid(&compaction_temp_name(u32::MAX, 0)),
            Some(u32::MAX)
        );
    }

    /// The sweep reclaims compaction temps and nothing else. A rotated-aside
    /// log is preserved *by decision* and a stranger's file is not ours at all,
    /// so a name that does not parse as `<log>.compact-<pid>-<nanos>` is neither
    /// removed nor counted — including one whose pid field is not a number.
    #[test]
    fn the_open_time_sweep_touches_only_compaction_temps() {
        let dir = scratch("transync-diskcache-sweepscope");
        let bystanders = [
            format!("{LOG_FILE_NAME}.unreadable-1"),
            format!("{LOG_FILE_NAME}{COMPACT_TEMP_INFIX}not-a-pid-1"),
            format!("{LOG_FILE_NAME}{COMPACT_TEMP_INFIX}{}", std::process::id()),
            "operator-notes.txt".to_string(),
        ];
        for name in &bystanders {
            std::fs::write(dir.join(name), "kept").unwrap();
        }
        let foreign = dir.join(format!(
            "{LOG_FILE_NAME}{COMPACT_TEMP_INFIX}{pid}-9",
            pid = foreign_pid()
        ));
        std::fs::write(&foreign, "kept").unwrap();

        drop(DiskCache::open_with(&dir, unbounded()).expect("opens"));

        for name in &bystanders {
            assert!(
                dir.join(name).exists(),
                "{name} is not a compaction temp and was left alone"
            );
        }
        assert!(
            foreign.exists(),
            "a foreign pid's temp is left in place: a pid that is not ours is not \
             evidence that its process is dead (ADR-0024)"
        );
    }

    /// A whole pipeline run against `DiskCache` behaves exactly as one against
    /// `InMemoryCache`: same output, same entry count, and a second process
    /// over the same directory resumes without dispatching a batch. The run
    /// here is clean — nothing is disqualified, so nothing is evicted; §5a's
    /// targeted eviction under a pipeline run is the sibling test below.
    #[tokio::test]
    async fn a_pipeline_run_against_disk_matches_one_against_memory() {
        use crate::TranslateOptions;

        let src = "alpha paragraph\n\nbravo paragraph\n";
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };

        let memory = InMemoryCache::new();
        let mem_dyn: &dyn Cache = &memory;
        let mem_out = crate::pipeline::run_pipeline(src, &opts, &EchoTranslator, mem_dyn)
            .await
            .expect("memory run completes");

        let dir = scratch("transync-diskcache-pipeline");
        let disk_out = {
            let disk = DiskCache::open(&dir).expect("opens");
            let disk_dyn: &dyn Cache = &disk;
            let out = crate::pipeline::run_pipeline(src, &opts, &EchoTranslator, disk_dyn)
                .await
                .expect("disk run completes");
            assert_eq!(disk.len(), memory.len(), "same entries written");
            out
        };
        assert_eq!(mem_out.translated_document, disk_out.translated_document);
        assert_eq!(
            serde_json::to_string(&mem_out.alignment_map).unwrap(),
            serde_json::to_string(&disk_out.alignment_map).unwrap()
        );

        // A second process over the same directory resumes from the log.
        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(reopened.len(), mem_out.alignment_map.blocks.len());
        let resumed_dyn: &dyn Cache = &reopened;
        let resumed = crate::pipeline::run_pipeline(src, &opts, &PanicTranslator, resumed_dyn)
            .await
            .expect("the resumed run is served entirely from the log");
        assert_eq!(resumed.translated_document, disk_out.translated_document);
    }

    /// DCR-0028 §4, semantic eviction on the disk backend: §5a's targeted,
    /// `BlockId`-keyed eviction of a reparse-disqualified unit has to reach
    /// `DiskCache` through a real pipeline run — and, unlike the in-memory
    /// backend, it has to still be gone once the process that made it is over.
    /// The round trips above pin the `evict` record from direct trait calls;
    /// this pins the pipeline path that emits it.
    ///
    /// The fixture is the report tests': the second list item comes back
    /// indented by two spaces. That payload is a valid one-item list on its own
    /// — per-kind and the fragment reparse both accept it, so it *is* cached —
    /// but spliced after the first item it becomes a SUB-list, so only the
    /// post-regen full-document reparse catches it. Which keys the eviction
    /// removed is then read the only way a cache's contents are observable
    /// through the trait: a second run over a REOPENED cache reports the units
    /// it had to dispatch, and that set must be exactly the disqualified set —
    /// no wider, or "targeted" is a word rather than a behavior.
    #[tokio::test]
    async fn a_reparse_disqualified_unit_is_evicted_from_disk_and_stays_evicted() {
        use crate::TranslateOptions;

        let src = "# Alpha\n\nfirst paragraph\n\n- item one\n- item two\n\nsecond paragraph\n";
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..Default::default()
        };

        let memory = InMemoryCache::new();
        let mem_dyn: &dyn Cache = &memory;
        let mem_out = crate::pipeline::run_pipeline(src, &opts, &IndentSecondItem, mem_dyn)
            .await
            .expect("FallbackPerBlock degrades, never aborts");

        let dir = scratch("transync-diskcache-evict-pipeline");
        let (disk_out, live_after_run) = {
            let disk = DiskCache::open(&dir).expect("opens");
            let disk_dyn: &dyn Cache = &disk;
            let out = crate::pipeline::run_pipeline(src, &opts, &IndentSecondItem, disk_dyn)
                .await
                .expect("FallbackPerBlock degrades, never aborts");
            let live = disk.len();
            assert_eq!(
                live,
                memory.len(),
                "the eviction left the two backends holding the same entries"
            );
            (out, live)
        };
        assert_eq!(mem_out.translated_document, disk_out.translated_document);

        let disqualified: Vec<String> = disk_out
            .validation_report
            .full_reparse_fallbacks
            .iter()
            .map(|id| id.0.clone())
            .collect();
        let total_units = disk_out.validation_report.per_unit.len();
        assert!(
            !disqualified.is_empty(),
            "the fixture must actually trip the full-document reparse gate"
        );
        assert!(
            disqualified.len() < total_units,
            "and must leave healthy siblings behind, or 'targeted' proves \
             nothing: {disqualified:?} of {total_units} units"
        );
        assert_eq!(
            live_after_run,
            total_units - disqualified.len(),
            "exactly the disqualified units left the log"
        );

        // A new process replays the log: the eviction is a fact on disk, not an
        // in-memory bookkeeping detail that dies with the handle that made it.
        let reopened = DiskCache::open(&dir).expect("reopens");
        assert_eq!(reopened.len(), live_after_run, "the eviction replayed");

        let dispatched: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let recorder = RecordingEcho {
            dispatched: Arc::clone(&dispatched),
        };
        let reopened_dyn: &dyn Cache = &reopened;
        let second = crate::pipeline::run_pipeline(src, &opts, &recorder, reopened_dyn)
            .await
            .expect("the second run completes");

        let mut dispatched_ids = dispatched.lock().expect("no panic held the lock").clone();
        dispatched_ids.sort();
        let mut expected = disqualified.clone();
        expected.sort();
        assert_eq!(
            dispatched_ids, expected,
            "the reopened run re-dispatched exactly the evicted units and \
             replayed every healthy sibling from the log"
        );
        assert!(
            second.validation_report.full_reparse_fallbacks.is_empty(),
            "with a clean payload the document reparses, so nothing is \
             disqualified a second time"
        );
    }

    /// Translates by echoing the source payload — enough to populate a cache
    /// with real, validation-passing entries.
    struct EchoTranslator;

    #[async_trait::async_trait]
    impl crate::llm::Translator for EchoTranslator {
        async fn translate_batch(
            &self,
            batch: crate::llm::TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<crate::llm::TranslationBatchResult, crate::llm::TranslatorError> {
            let units = batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: u.source_payload.clone(),
                    warnings: Vec::new(),
                })
                .collect();
            Ok(crate::llm::TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
        fn fingerprint(&self) -> ProviderFingerprint {
            ProviderFingerprint::new("disk-pipeline-stub", &["shared"])
        }
    }

    /// Echoes every unit except the second list item, which comes back
    /// indented by two spaces — a payload that passes every per-unit layer and
    /// is therefore cached, and fails only the post-regen full-document
    /// reparse.
    struct IndentSecondItem;

    #[async_trait::async_trait]
    impl crate::llm::Translator for IndentSecondItem {
        async fn translate_batch(
            &self,
            batch: crate::llm::TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<crate::llm::TranslationBatchResult, crate::llm::TranslatorError> {
            let units = batch
                .units
                .iter()
                .map(|u| UnitResult {
                    unit_id: u.unit_id.clone(),
                    output_kind: OutputKind::Translated,
                    translated_payload: if u.source_payload.contains("item two") {
                        format!("  {}", u.source_payload)
                    } else {
                        u.source_payload.clone()
                    },
                    warnings: Vec::new(),
                })
                .collect();
            Ok(crate::llm::TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
        fn fingerprint(&self) -> ProviderFingerprint {
            ProviderFingerprint::new("disk-evict-stub", &["shared"])
        }
    }

    /// Shares `IndentSecondItem`'s namespace — so the second run computes the
    /// same keys — and echoes cleanly, recording which units it was actually
    /// handed. That record is how "served from the log" and "re-dispatched"
    /// are told apart without reaching into the cache's private index.
    struct RecordingEcho {
        dispatched: Arc<Mutex<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl crate::llm::Translator for RecordingEcho {
        async fn translate_batch(
            &self,
            batch: crate::llm::TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<crate::llm::TranslationBatchResult, crate::llm::TranslatorError> {
            let mut seen = self.dispatched.lock().expect("no panic held the lock");
            let units = batch
                .units
                .iter()
                .map(|u| {
                    seen.push(u.unit_id.0.clone());
                    UnitResult {
                        unit_id: u.unit_id.clone(),
                        output_kind: OutputKind::Translated,
                        translated_payload: u.source_payload.clone(),
                        warnings: Vec::new(),
                    }
                })
                .collect();
            Ok(crate::llm::TranslationBatchResult {
                batch_id: batch.batch_id,
                detected_source_language: None,
                units,
            })
        }
        fn fingerprint(&self) -> ProviderFingerprint {
            ProviderFingerprint::new("disk-evict-stub", &["shared"])
        }
    }

    /// Shares `EchoTranslator`'s namespace but panics if dispatched, so "the
    /// reopened run made no provider call" is structural.
    struct PanicTranslator;

    #[async_trait::async_trait]
    impl crate::llm::Translator for PanicTranslator {
        async fn translate_batch(
            &self,
            _batch: crate::llm::TranslationBatch,
            _cancel: &crate::CancellationToken,
        ) -> Result<crate::llm::TranslationBatchResult, crate::llm::TranslatorError> {
            panic!("a run resumed from the log must dispatch no batch");
        }
        fn fingerprint(&self) -> ProviderFingerprint {
            ProviderFingerprint::new("disk-pipeline-stub", &["shared"])
        }
    }
}
