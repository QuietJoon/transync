//! Translation cache — the [`Cache`] trait and its two backends.
//!
//! Stable from `0.1.0`. The trait was built as the seam for a disk-backed
//! backend from the start; [`DiskCache`] (`cache::disk`, DCR-0028 / ADR-0021)
//! is that backend, and [`InMemoryCache`] remains the run-scoped default.
//!
//! **[`InMemoryCache`]'s lifetime is one run or one session (R0003-0048).**
//! It is deliberately **unbounded**: [`crate::translate`] builds one per call
//! and drops it with the call, and the CLI builds one per run, so the bound on
//! a shipped path is the document being translated — a capacity knob on this
//! map would be dead weight in every path that exists. A consumer that wants
//! one cache to outlive a run, or to span many documents indefinitely, is
//! asking for a different lifetime, and that lifetime is what a disk-backed
//! backend built for it is for (DCR-0028 §4). Holding an `InMemoryCache`
//! across an unbounded document stream is therefore a scoping mistake, not a
//! missing eviction policy.
//!
//! **Representation (R0003-0049, R0003-0051).** The map stores
//! `Arc<UnitResult>`, so [`Cache::get`] clones a pointer under the lock and
//! materializes the owned [`UnitResult`] the trait returns *after* the guard
//! drops — a payload can be megabytes of translated Markdown, and copying it
//! under the global mutex serialized every concurrent batch's lookups behind
//! one memcpy. The `Arc` is an internal representation choice and stays one:
//! returning it from the trait would put a backend's storage decision into the
//! public contract for no consumer request. [`CacheKey`]'s own fields stay
//! plain `String`s for the same kind of reason in the other direction — a few
//! short per-entry clones against an LLM round trip is not a cost lever, and
//! `Arc<str>` axes would be breaking churn on an exhaustive-by-policy public
//! key with no measured need behind it.
//!
//! **The identity principle** (owner decision 2026-08-06, ti c02f69): a
//! [`CacheKey`]'s content axes cover *exactly what landed in the prompt bytes
//! for that unit* — no more, no less. Everything the model was shown is an
//! axis; everything it was not shown is not, however much it shaped the run.
//! That is why the packing budget, the tokenizer hint and the output ceiling
//! stay out — none of them is a byte the model read — why the unit's context
//! hints and its batch's assembled instruction are in, and why co-batching
//! that leaves both of those untouched leaves the key untouched.
//! [`CacheKey`]'s own docs carry the field-by-field reading and the two
//! deliberate exceptions.
//!
//! TRACE: SCN-10
//! TRACE: ADR-0002

// The disk backend's implementation. Only [`DiskCache`] is public surface, and
// it is re-exported here, so `cache::disk` is not a path a consumer names —
// contracts.md §0 tabulates items, and an undocumented public module path would
// be surface the table does not cover.
mod disk;

pub use disk::{DiskCache, DiskCacheOptions};

use crate::id::BlockId;
use crate::llm::{GlossaryEntry, ProviderFingerprint, UnitResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Composite key for cache lookups.
///
/// Bumping any of these fields invalidates every entry in scope. The serde
/// shape is a plain JSON object over the field names below — [`DiskCache`]'s
/// on-disk transport (DCR-0028 §2). Nothing about an axis is re-encoded,
/// concatenated or re-hashed on the way to disk: an entry's stored identity is
/// its **full** serialized axis set, and a lookup compares full deserialized
/// `CacheKey` equality, so the store adds no collision surface of its own.
///
/// # What identity means here (ti c02f69)
///
/// The fields fall into two groups, and only the second one has a rule:
///
/// - **Namespace axes** — `provider_fingerprint`, `model_id`, and
///   `validation_schema_version`. They do not describe what the model was
///   shown; they say *who* produced the entry and *under which contract*, so
///   a shared cache cannot replay one provider's output for another's request
///   or carry an entry across a prompt/payload-contract generation.
/// - **Content axes** — everything else. Their rule is exact: **if it changed
///   the prompt bytes the model saw for this unit, it is identity; if it did
///   not, it is not** (owner decision 2026-08-06). `source_hash` +
///   `block_kind` + [`Self::input_mode`] cover the unit's own body, the
///   structural hints derived from it, and the two labels the prompt spells
///   beside it, `source_lang` / `target_lang` the language labels,
///   `profile_version` + `profile_prompt_hash` + `glossary_hash` the system
///   prompt, [`Self::context_hash`] the unit's serialized context hints, and
///   [`Self::instruction_hash`] the user-message instruction assembled for the
///   batch it was packed into.
///
/// Two deliberate exceptions to the content rule, both stated rather than
/// left to be inferred:
///
/// - The unit's **`BlockId`** is not an axis, so two blocks with identical
///   source bytes and identical context share one entry. That is the
///   cross-block dedup the cache exists for; the pipeline rewrites the cached
///   result's id to the requesting unit's (see `pipeline::dispatch`).
/// - The ADR-0009 **retry hint** is not an axis, although a re-dispatched
///   unit's prompt carries it. It is a correction channel over an unchanged
///   task — `source_payload` on a retry is byte-identical by contract — so
///   keying on it would file every retry-produced translation under a key no
///   first dispatch ever looks up, i.e. cache the work and never reuse it.
///
/// One axis is deliberately **absent**: the profile's
/// `[batching].target_output_tokens`, which leaves the engine as the
/// provider's enforced output ceiling (`max_completion_tokens` /
/// `max_output_tokens`). It is a *stop*, not a content parameter — it can
/// only cut a response off, and a cut-off response never reaches this map
/// because only output that passed the per-unit validation layers is ever
/// written (contracts.md §5a). Entries are therefore shared across ceilings
/// on purpose; `pipeline::run_level_tests::output_ceiling_does_not_change_cache_identity`
/// pins that, and contracts.md §1 *Output ceiling and cache identity* carries
/// the full argument and the provider obligation it rests on (R0001-0004).
///
/// TRACE: SCN-10
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct CacheKey {
    /// EXT-2026-07 P1-4 (R0008-0002): namespace of the translator
    /// instance that produced/consumes this entry. Without it, a shared
    /// cache could replay one provider's output for another's request.
    pub provider_fingerprint: ProviderFingerprint,
    /// EXT-2026-07 P1-4: core validation/prompt-contract generation;
    /// see `validate::VALIDATION_SCHEMA_VERSION` for bump discipline.
    pub validation_schema_version: u32,
    pub source_hash: u64,
    pub source_lang: String,
    pub target_lang: String,
    pub profile_version: String,
    pub profile_prompt_hash: u64,
    pub glossary_hash: u64,
    pub model_id: String,
    pub block_kind: String,
    /// The unit's [`InputMode`](crate::llm::InputMode) **as the prompt spells
    /// it** — the exact wire label `llm::prompt`'s user-message body puts in
    /// this unit's `input_mode` field (ti 5f7942).
    ///
    /// `block_kind` does not cover it, and neither does `source_hash`. The
    /// kind→mode map is one-to-one for every kind but one: a `table` unit
    /// reads `full_table_markdown` when it is a whole block and
    /// `table_row_window` when the DCR-0026 splitter made it one window of an
    /// oversize table. A window's payload is a *complete* table shaped exactly
    /// like a whole table's — same header, same delimiter, no trailing newline
    /// — so a small table and the opening window of a big one that starts with
    /// the same rows hash the same bytes. That is reachable rather than
    /// theoretical: when the two also share a section and a batch, every other
    /// axis agrees and the two prompts share one entry
    /// (`pipeline::run_level_tests::a_row_window_and_a_whole_table_are_not_one_entry`).
    ///
    /// The axis is the **label**, not the variant. `parent_block_id`,
    /// `window_index`, `window_count` and `language_info` never reach the
    /// model, so keying on them would over-discriminate and orphan the case
    /// the cache exists for — two byte-identical windows of one table must
    /// still share an entry, exactly as two identical blocks do under the
    /// `BlockId` exception below.
    pub input_mode: String,
    /// Hash of the unit's provider-visible context hints (document title,
    /// section path + heading levels, neighbor kinds + summaries) — exactly
    /// the set `llm::prompt`'s `ContextHints` serializes. R0008-0001: without
    /// it, a shared cache could return a translation produced under a
    /// different heading path or neighboring prose, silently changing
    /// meaning.
    pub context_hash: u64,
    /// Hash of the user-message **instruction** assembled for the batch this
    /// unit was packed into — the one part of a unit's prompt that its
    /// co-batched peers can move (ti c02f69, Review-0001 finding R0001-0005).
    ///
    /// The instruction carries optional clauses: two decided by the profile,
    /// and two decided by batch membership — the html-segment contract, which
    /// rides on a batch holding at least one raw-HTML unit, and the DCR-0026
    /// row-window contract, which rides on a batch holding at least one table
    /// row-window unit. So the same unit, packed beside different peers, can
    /// be translated under materially different instructions — and before this
    /// axis existed, the two shared one entry.
    ///
    /// It is a digest of the assembled bytes, not of the clause flags, which
    /// makes it exact in both directions: cohort membership that leaves the
    /// instruction identical (any reordering, any peer swap that keeps both
    /// membership verdicts) leaves the key identical, and a change to the
    /// instruction *wording* — not a contract, and free to move in any
    /// release (contracts.md §0 tier b) — moves the key rather than silently
    /// replaying pre-change translations.
    ///
    /// Its resolution is the batch **as the packer built it**, which is the
    /// population every dispatch for this unit is drawn from; contracts.md
    /// §5a records the one residual (a round dispatching a strict subset can
    /// assemble a shorter instruction) and why keying on the round instead
    /// was rejected.
    pub instruction_hash: u64,
}

/// Identity of one **document-level** cache record (DCR-0028 §3).
///
/// Unit entries answer "what did the model say about this block"; this key
/// answers "what did a provider observe about this whole document". The two
/// spaces are deliberately separate — a document record has no block, no
/// payload, and no prompt of its own.
///
/// # What is in the key, and what is deliberately not
///
/// It carries the three **namespace** axes ([`Self::provider_fingerprint`],
/// [`Self::model_id`], [`Self::validation_schema_version`]), the two language
/// labels, and a hash of the whole document's source bytes. It does **not**
/// carry the prompt-identity axes `CacheKey` uses (`profile_prompt_hash`,
/// `glossary_hash`, `context_hash`, `instruction_hash`), and that is a scoping
/// statement about the §5a identity principle rather than an exception to it:
/// that rule governs replay of **unit translation content**, where the prompt
/// bytes are the product's provenance. A detection is a different kind of
/// record — an advisory *envelope observation about the document*, bounded and
/// truncated — and since DCR-0027 made the prompt axes batch-scoped (one
/// cohort per section), a run has no single prompt identity a document-level
/// key could honestly name. Folding every cohort's digest in was considered and
/// rejected: unit entries already miss when the profile changes, so a full-hit
/// replay under a changed profile cannot occur within one cache generation.
///
/// `source_lang` is a label this layer does not interpret — the `auto` sentinel
/// is a CLI-boundary concern (ADR-0013), and this record needs no knowledge of
/// it: it persists and replays whatever detection was observed, under whatever
/// labels the run carried.
///
/// **Exhaustive by policy**, on `CacheKey`'s own ground: a disk-backed backend
/// has to serialize and reconstruct every field, so a field addition is
/// breaking-by-policy and rides a version bump. The field set is welded from
/// outside the crate by `crates/transync/tests/public_surface.rs`.
///
/// TRACE: DCR-0028
/// TRACE: OI-0017
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DocumentMetaKey {
    /// Namespace of the translator instance that observed this document.
    pub provider_fingerprint: ProviderFingerprint,
    /// Core validation/prompt-contract generation, as on [`CacheKey`].
    pub validation_schema_version: u32,
    pub model_id: String,
    pub source_lang: String,
    pub target_lang: String,
    /// `id::source_hash_bytes` over the exact source string handed to
    /// `translate` — before parsing, with no normalization.
    pub doc_source_hash: u64,
}

/// Document-level facts a run observed and a later run may replay
/// (DCR-0028 §3).
///
/// Advisory by construction: every field is optional, and a backend that
/// stores nothing degrades a replayed run to reporting less, never to
/// reporting something wrong.
///
/// TRACE: DCR-0028
/// TRACE: OI-0017
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DocumentMeta {
    /// The language a live provider envelope reported for this document,
    /// already bounded by `validate::truncate_diagnostic` before it reached
    /// the run's latch (R0003-0038).
    pub detected_source_language: Option<String>,
}

/// Identity of one cached **glossary-extraction preflight** (ti `dca5bf`, the
/// second record in DCR-0028's document-scoped family).
///
/// The preflight is the one provider call a fully-cache-hit run still paid:
/// `Translator::extract_glossary` runs before any batch is built, so no unit
/// entry can elide it. This key is what lets a second run replay the harvest
/// instead of buying it again.
///
/// # The content axis *is* the prompt bytes
///
/// Unlike [`DocumentMetaKey`], this record replays **content a model produced**,
/// so §5a's identity rule applies to it in full: fold exactly what changed the
/// bytes the extractor was shown, and nothing else. Rather than re-listing the
/// request's fields — a second reading of the prompt builder that could drift
/// from it — [`Self::request_hash`] digests the assembled bytes themselves, the
/// same discipline `pipeline::InstructionDigest` uses for a batch's
/// instruction. One axis, derived from the source of truth, covering every
/// input that reaches the model: the excerpt (and therefore the truncation
/// ceiling that produced it), both language labels, the static glossary's
/// source terms as `existing_terms`, the term cap, and the wording of both the
/// system prompt and the instruction.
///
/// Two consequences worth stating, because they are decisions and not
/// accidents:
///
/// - `GlossaryExtractionRequest::source_truncated` is **not** in the key. It
///   never reaches the model — `llm::prompt::build_extraction_user_prompt` does
///   not spell it — and it exists to let a report say the harvest was partial.
///   Two documents sharing a byte-identical excerpt were shown the same input
///   and honestly share one record.
/// - No `doc_source_hash`. A truncated run's extraction is about the excerpt,
///   not about the bytes past the ceiling that the model never saw; hashing the
///   whole document would file two identical questions under two identities.
///
/// The three namespace axes are [`DocumentMetaKey`]'s and [`CacheKey`]'s, and
/// carry the same meaning: who answered, and under which core contract.
/// `profile_version` is deliberately absent — the extraction prompt carries no
/// profile prompt body, and the one profile input it *does* carry (the static
/// glossary's source terms) is already inside `request_hash`.
///
/// **Exhaustive by policy**, on [`CacheKey`]'s ground: a disk backend has to
/// serialize and reconstruct every field, so a field addition is
/// breaking-by-policy. The field set is welded from outside the crate by
/// `crates/transync/tests/public_surface.rs`.
///
/// TRACE: DCR-0028
/// TRACE: OI-0026
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GlossaryExtractionKey {
    /// Namespace of the translator instance that produced the harvest.
    pub provider_fingerprint: ProviderFingerprint,
    /// Core validation/prompt-contract generation, as on [`CacheKey`].
    pub validation_schema_version: u32,
    pub model_id: String,
    /// `id::source_hash_bytes` over the length-framed pair
    /// (`llm::prompt::EXTRACTION_SYSTEM_PROMPT`, the assembled extraction user
    /// message) — exactly the bytes the extractor was shown. Framed, not
    /// concatenated, so no arrangement of one prompt's contents can reproduce
    /// another arrangement of the two (ADR-0020's injectivity discipline).
    pub request_hash: u64,
}

/// A provider's glossary harvest, stored **raw** (ti `dca5bf`).
///
/// These are the entries `Translator::extract_glossary` returned, *before*
/// [`crate::profile::merge_auto_glossary`] ran. Storing the pre-merge answer is
/// what makes replay honest: the merge folds in the static glossary's target
/// terms and notes, which `GlossaryExtractionKey` does not cover (only the
/// source terms reach the prompt, as `existing_terms`), so a replayed run
/// re-runs the merge against its own profile rather than replaying a merge
/// decided under a different one.
///
/// Only a successful `Ok(Some(_))` extraction is ever stored. `Ok(None)`
/// ("this translator does not support extraction") is a property of the
/// translator that costs no provider call to rediscover, and `Err(_)` is a
/// transient failure that must not be latched into a permanent one.
///
/// Storing the raw answer stores **untrusted provider output** (invariant 7),
/// and that is safe for the same reason a `UnitResult` payload is: nothing
/// consumes it directly. Every replay goes through `merge_auto_glossary`, which
/// is where the term cap and the 80/200/200 sanitize bounds — OI-0026's
/// prompt-stuffing guard — are applied. A record cannot smuggle past a gate a
/// live extraction would have hit, because it meets the same gate.
///
/// No `PartialEq`: [`GlossaryEntry`] is a third-party wire type that does not
/// carry one, and adding equality to it for a cache record's convenience would
/// be surface movement this record does not need — a backend stores and returns
/// the value, it never compares two of them.
///
/// TRACE: DCR-0028
/// TRACE: OI-0026
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GlossaryExtraction {
    /// The harvested entries in provider order. An empty vector is a real
    /// record — "supported, nothing salient found" — and replays as such.
    pub terms: Vec<GlossaryEntry>,
}

/// Error surface for cache backends. The in-memory cache never
/// returns errors; the variants exist for the disk-backed impls this
/// trait was built for (module doc). The pipeline treats every
/// `CacheError` as a degraded-but-alive condition — see the pipeline
/// helpers — a cache error can never fail a translation run.
///
/// EXT-2026-07 P1-4
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CacheError {
    #[error("cache io: {0}")]
    Io(String),
    #[error("cache serialization: {0}")]
    Serialization(String),
    #[error("cache backend: {0}")]
    Backend(String),
}

/// Cache trait v2. The pipeline consults a `&dyn Cache` per unit
/// before dispatching a batch, writes provisionally-valid results
/// after per-unit validation, and evicts keys whose results the
/// full-document reparse later disqualified (contracts.md §5a).
///
/// All methods are fallible: the trait's purpose is future disk-backed
/// backends, which have unavoidable I/O + (de)serialization failure
/// modes. The pipeline centralizes the degrade policy so a `CacheError`
/// degrades the run (miss / not-persisted / stale-entry-may-replay) and
/// never aborts it.
///
/// TRACE: ADR-0002
/// EXT-2026-07 P1-4
pub trait Cache: Send + Sync {
    fn get(&self, key: &CacheKey) -> Result<Option<UnitResult>, CacheError>;
    fn put(&self, key: CacheKey, value: UnitResult) -> Result<(), CacheError>;
    /// Remove one entry. Absent keys are a successful no-op.
    fn evict(&self, key: &CacheKey) -> Result<(), CacheError>;

    /// Read this document's stored metadata record (DCR-0028 §3).
    ///
    /// The pipeline consults it on **one** kind of run: one that dispatched
    /// zero provider batches, so no envelope ever existed to observe the
    /// document with. A run that made even one provider call never asks —
    /// replay compensates for the calls the cache elided, it never overrides
    /// what a live provider said or declined to say.
    ///
    /// The default answers `Ok(None)`, which is exactly pre-v0.4.0 behavior: a
    /// backend that ignores metadata makes a fully-cache-hit `auto` run report
    /// no detection. Degraded, never wrong.
    fn get_document_meta(
        &self,
        _key: &DocumentMetaKey,
    ) -> Result<Option<DocumentMeta>, CacheError> {
        Ok(None)
    }

    /// Store this document's metadata record (DCR-0028 §3).
    ///
    /// Written after the batch fan-out settles, and only when the run's
    /// detection latch holds a value from a **live qualifying envelope**
    /// (R0001-0007's rule: the earliest envelope whose batch carried no
    /// `BatchFault`). Last write wins; there is no metadata eviction, because
    /// the record is advisory and superseding it is the only maintenance it
    /// needs.
    ///
    /// The default discards the record, preserving pre-v0.4.0 behavior.
    fn put_document_meta(
        &self,
        _key: DocumentMetaKey,
        _meta: DocumentMeta,
    ) -> Result<(), CacheError> {
        Ok(())
    }

    /// Read this document's stored glossary harvest (ti `dca5bf`).
    ///
    /// Consulted on **every** run with the auto-glossary preflight enabled,
    /// before any batch is built — unlike [`Self::get_document_meta`], which
    /// only compensates for a run that dispatched nothing. The difference is
    /// what the two records are: a detection is an advisory observation a live
    /// envelope may still contradict, while a harvest is content the model
    /// produced for a question [`GlossaryExtractionKey`] names in full. Replay
    /// here is the ordinary cache trade every unit entry already makes.
    ///
    /// The default answers `Ok(None)`, which is exactly pre-v0.4.0 behavior: a
    /// backend that stores no harvest makes every enabled run pay the preflight
    /// call, as every enabled run did before this method existed.
    fn get_glossary_extraction(
        &self,
        _key: &GlossaryExtractionKey,
    ) -> Result<Option<GlossaryExtraction>, CacheError> {
        Ok(None)
    }

    /// Store a glossary harvest (ti `dca5bf`).
    ///
    /// Written only after a **live** `Ok(Some(_))` extraction — never after a
    /// replayed one, never for the unsupported or failed outcomes. Last write
    /// wins, and there is no harvest eviction: like a metadata record, it is
    /// superseded in place.
    ///
    /// The default discards the record, preserving pre-v0.4.0 behavior.
    fn put_glossary_extraction(
        &self,
        _key: GlossaryExtractionKey,
        _value: GlossaryExtraction,
    ) -> Result<(), CacheError> {
        Ok(())
    }
}

/// Default in-memory implementation. Backed by a `Mutex<HashMap>` so it is
/// `Send + Sync` without requiring `&mut self` on the trait methods.
///
/// Values are held as `Arc<UnitResult>` so a hit's deep copy happens outside
/// the lock (module docs, R0003-0049); the scope this cache is built for — one
/// run or one session — is the module docs' R0003-0048 answer.
///
/// TRACE: SCN-10
#[derive(Default)]
pub struct InMemoryCache {
    store: Mutex<HashMap<CacheKey, Arc<UnitResult>>>,
    /// Document-level records (DCR-0028 §3), keyed independently of unit
    /// entries and behind their own lock: they are written once per run at a
    /// point where no unit lookup is in flight, so sharing the hot map's mutex
    /// would buy nothing and couple two unrelated poison domains.
    meta: Mutex<HashMap<DocumentMetaKey, DocumentMeta>>,
    /// Glossary harvests (ti `dca5bf`), the family's second record kind — its
    /// own map for the same two reasons, plus a third: it is read and written
    /// at the *other* end of a run (before batching, where `meta` is written
    /// after the fan-out settles), so the two never contend at all.
    glossary: Mutex<HashMap<GlossaryExtractionKey, GlossaryExtraction>>,
}

impl InMemoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Lock the unit store, recovering from a poisoned mutex with a
    /// clear-once-and-resume policy (EXT-2026-07 P2-8).
    fn locked(&self) -> std::sync::MutexGuard<'_, HashMap<CacheKey, Arc<UnitResult>>> {
        lock_clearing_poison(&self.store, "cache")
    }

    /// The same recovery over the document-metadata map (DCR-0028 §5). One
    /// policy, two maps: a panic under either lock loses that map's contents
    /// and nothing else.
    fn meta_locked(&self) -> std::sync::MutexGuard<'_, HashMap<DocumentMetaKey, DocumentMeta>> {
        lock_clearing_poison(&self.meta, "cache document metadata")
    }

    /// The same recovery over the glossary-harvest map (ti `dca5bf`). One
    /// policy, three maps.
    fn glossary_locked(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<GlossaryExtractionKey, GlossaryExtraction>> {
        lock_clearing_poison(&self.glossary, "cache glossary extractions")
    }

    /// Number of entries currently held. Useful for partial-resume tests.
    ///
    /// Unit entries only — the document-scoped records (metadata, glossary
    /// harvests) are separate spaces and are not counted here, so a test that
    /// asserts on resume progress is not perturbed by a run having latched a
    /// detection or filed a harvest.
    ///
    /// TRACE: SCN-10
    pub fn len(&self) -> usize {
        self.locked().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// The clear-once-and-resume poison policy (ADR-0015, EXT-2026-07 P2-8),
/// shared by [`InMemoryCache`]'s two maps so neither can drift from it.
///
/// A poisoned lock means a previous holder panicked mid-mutation, so the map's
/// contents are untrusted. Recover the guard via `PoisonError::into_inner`,
/// drop the (untrusted) contents, clear the poison flag so subsequent locks
/// succeed normally, and emit a single `warn`. The returned guard is over an
/// empty, functional map: reads miss until repopulated, and writes land and
/// are readable.
fn lock_clearing_poison<'a, K, V>(
    lock: &'a Mutex<HashMap<K, V>>,
    what: &'static str,
) -> std::sync::MutexGuard<'a, HashMap<K, V>> {
    match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            guard.clear();
            lock.clear_poison();
            tracing::warn!(
                target: "transync::cache",
                "{what} poisoned; cleared and resumed empty"
            );
            guard
        }
    }
}

impl Cache for InMemoryCache {
    /// On a poisoned lock the shared recovery path clears the untrusted map,
    /// so this is a miss until entries are re-populated (EXT-2026-07 P2-8).
    /// The in-memory backend never fails — poison is an internal recovery
    /// event (wipe + warn), never a `CacheError`: the map after clearing is
    /// a valid empty cache, not a failed backend. Always `Ok` (EXT-2026-07
    /// P1-4).
    ///
    /// R0003-0049: only the `Arc` clone happens under the lock. The
    /// statement's temporaries — the guard included — drop at its `;`, so the
    /// deep copy of the payload on the next line runs with the mutex free and
    /// a concurrent batch's lookup is not queued behind one memcpy.
    ///
    /// TRACE: SCN-10
    fn get(&self, key: &CacheKey) -> Result<Option<UnitResult>, CacheError> {
        let hit: Option<Arc<UnitResult>> = self.locked().get(key).cloned();
        Ok(hit.map(|shared| (*shared).clone()))
    }

    /// After a poison-recovery the write lands in the cleared, functional map
    /// and is readable by later gets — no dead writes (EXT-2026-07 P2-8).
    /// Always `Ok` (EXT-2026-07 P1-4).
    ///
    /// The `Arc` is allocated *before* the lock is taken, for the same reason
    /// `get` materializes after it drops (R0003-0049).
    ///
    /// TRACE: SCN-10
    fn put(&self, key: CacheKey, value: UnitResult) -> Result<(), CacheError> {
        let shared = Arc::new(value);
        self.locked().insert(key, shared);
        Ok(())
    }

    /// Remove one entry (contracts.md §5a eviction). Same clear-once poison
    /// recovery as `get`/`put`; removing an absent key is a successful
    /// no-op. Always `Ok` (EXT-2026-07 P1-4).
    ///
    /// TRACE: SCN-10
    fn evict(&self, key: &CacheKey) -> Result<(), CacheError> {
        self.locked().remove(key);
        Ok(())
    }

    /// DCR-0028 §3: the in-memory backend really stores document metadata
    /// rather than taking the trait's discarding default, so the OI-0017 fix
    /// works for any consumer that passes a cache to
    /// [`crate::translate_with_cache`] — durability across processes is the
    /// disk backend's addition, not this fix's precondition.
    ///
    /// TRACE: OI-0017
    fn get_document_meta(&self, key: &DocumentMetaKey) -> Result<Option<DocumentMeta>, CacheError> {
        Ok(self.meta_locked().get(key).cloned())
    }

    /// Last write wins; there is no metadata eviction (DCR-0028 §3).
    ///
    /// TRACE: OI-0017
    fn put_document_meta(
        &self,
        key: DocumentMetaKey,
        meta: DocumentMeta,
    ) -> Result<(), CacheError> {
        self.meta_locked().insert(key, meta);
        Ok(())
    }

    /// ti `dca5bf`: the harvest store, overridden here for DCR-0028 §3's
    /// reason — the fix has to work for any consumer that passes a cache to
    /// [`crate::translate_with_cache`], not only for the disk backend.
    ///
    /// Within one process this already earns its keep: a caller translating
    /// several documents through one cache pays one preflight per *distinct*
    /// extraction request rather than one per call.
    ///
    /// TRACE: OI-0026
    fn get_glossary_extraction(
        &self,
        key: &GlossaryExtractionKey,
    ) -> Result<Option<GlossaryExtraction>, CacheError> {
        Ok(self.glossary_locked().get(key).cloned())
    }

    /// Last write wins; there is no harvest eviction (ti `dca5bf`).
    ///
    /// TRACE: OI-0026
    fn put_glossary_extraction(
        &self,
        key: GlossaryExtractionKey,
        value: GlossaryExtraction,
    ) -> Result<(), CacheError> {
        self.glossary_locked().insert(key, value);
        Ok(())
    }
}

#[allow(dead_code)]
fn _block_id_typing(_id: BlockId) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::OutputKind;
    use std::sync::Arc;

    fn key(hash: u64) -> CacheKey {
        CacheKey {
            provider_fingerprint: crate::llm::ProviderFingerprint::from_type_name("test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            source_hash: hash,
            source_lang: "en".to_string(),
            target_lang: "ko".to_string(),
            profile_version: "v1".to_string(),
            profile_prompt_hash: 0,
            glossary_hash: 0,
            model_id: "test-model".to_string(),
            block_kind: "paragraph".to_string(),
            input_mode: "text_fragment".to_string(),
            context_hash: 0,
            instruction_hash: 0,
        }
    }

    fn result(id: &str) -> UnitResult {
        UnitResult {
            unit_id: BlockId(id.to_string()),
            output_kind: OutputKind::Translated,
            translated_payload: format!("payload:{id}"),
            warnings: Vec::new(),
        }
    }

    // EXT-2026-07 P2-8: poisoning the lock must clear-once-and-resume, not
    // leave the cache permanently miss-on-get with dead writes. Adapted to
    // the fallible trait v2 (EXT-2026-07 P1-4): the in-memory backend still
    // never returns `Err`, so recovery is observed through `Ok(None)`.
    #[test]
    fn poison_clears_once_and_resumes() {
        let cache = Arc::new(InMemoryCache::new());

        // Pre-poison entry that recovery must drop.
        cache.put(key(1), result("pre")).unwrap();
        assert_eq!(cache.len(), 1);

        // Poison the mutex: hold the guard and panic while holding it.
        let poisoner = Arc::clone(&cache);
        let handle = std::thread::spawn(move || {
            let _guard = poisoner.store.lock().unwrap();
            panic!("poison the cache lock");
        });
        assert!(handle.join().is_err());

        // (b) recovery emptied the cache: the pre-poison entry is gone and
        // the first lock after poison reports an empty, functional map.
        assert_eq!(cache.len(), 0, "recovery should have cleared the map");
        assert!(
            cache.get(&key(1)).unwrap().is_none(),
            "pre-poison entry must not survive recovery"
        );

        // (a) put+get roundtrip works after recovery — writes are readable.
        cache.put(key(2), result("post")).unwrap();
        let got = cache
            .get(&key(2))
            .unwrap()
            .expect("post-recovery put must be readable");
        assert_eq!(got.translated_payload, "payload:post");
        assert_eq!(cache.len(), 1);
    }

    // EXT-2026-07 P1-4: get→put→get round-trip through the fallible trait,
    // and `evict` removes exactly the named key.
    #[test]
    fn get_put_get_round_trip_and_evict() {
        let cache = InMemoryCache::new();

        assert!(cache.get(&key(1)).unwrap().is_none(), "cold miss");
        cache.put(key(1), result("one")).unwrap();
        cache.put(key(2), result("two")).unwrap();
        assert_eq!(cache.len(), 2);
        assert_eq!(
            cache.get(&key(1)).unwrap().unwrap().translated_payload,
            "payload:one"
        );

        // Evict exactly key(1); key(2) survives and len drops by one.
        cache.evict(&key(1)).unwrap();
        assert_eq!(cache.len(), 1);
        assert!(
            cache.get(&key(1)).unwrap().is_none(),
            "evicted key must miss"
        );
        assert!(
            cache.get(&key(2)).unwrap().is_some(),
            "sibling key must survive eviction"
        );

        // Evicting an absent key is a successful no-op.
        cache.evict(&key(1)).unwrap();
        assert_eq!(cache.len(), 1);
    }

    fn meta_key(hash: u64) -> DocumentMetaKey {
        DocumentMetaKey {
            provider_fingerprint: crate::llm::ProviderFingerprint::from_type_name("test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            model_id: "test-model".to_string(),
            source_lang: "auto".to_string(),
            target_lang: "ko".to_string(),
            doc_source_hash: hash,
        }
    }

    // DCR-0028 §3: the in-memory backend overrides both metadata methods
    // rather than inheriting the trait's discarding defaults, and the record
    // is superseded in place rather than evicted.
    #[test]
    fn document_metadata_round_trips_and_the_last_write_wins() {
        let cache = InMemoryCache::new();

        assert_eq!(cache.get_document_meta(&meta_key(1)).unwrap(), None);
        cache
            .put_document_meta(
                meta_key(1),
                DocumentMeta {
                    detected_source_language: Some("en".to_string()),
                },
            )
            .unwrap();
        assert_eq!(
            cache
                .get_document_meta(&meta_key(1))
                .unwrap()
                .unwrap()
                .detected_source_language
                .as_deref(),
            Some("en")
        );

        // Superseding is the only maintenance the record needs.
        cache
            .put_document_meta(
                meta_key(1),
                DocumentMeta {
                    detected_source_language: Some("fr".to_string()),
                },
            )
            .unwrap();
        assert_eq!(
            cache
                .get_document_meta(&meta_key(1))
                .unwrap()
                .unwrap()
                .detected_source_language
                .as_deref(),
            Some("fr")
        );
        // A different document is a different record.
        assert_eq!(cache.get_document_meta(&meta_key(2)).unwrap(), None);
    }

    // The two spaces are independent: `len()` counts unit entries only, so a
    // stored detection cannot perturb a partial-resume assertion, and a unit
    // eviction cannot reach the metadata record.
    #[test]
    fn the_two_spaces_do_not_reach_into_each_other() {
        let cache = InMemoryCache::new();
        cache
            .put_document_meta(
                meta_key(1),
                DocumentMeta {
                    detected_source_language: Some("en".to_string()),
                },
            )
            .unwrap();
        assert!(cache.is_empty(), "len() counts unit entries only");

        cache.put(key(1), result("one")).unwrap();
        assert_eq!(cache.len(), 1);
        cache.evict(&key(1)).unwrap();
        assert!(cache.is_empty());
        assert!(
            cache.get_document_meta(&meta_key(1)).unwrap().is_some(),
            "evicting a unit entry must not touch the document record"
        );
    }

    fn glossary_key(request_hash: u64) -> GlossaryExtractionKey {
        GlossaryExtractionKey {
            provider_fingerprint: crate::llm::ProviderFingerprint::from_type_name("test"),
            validation_schema_version: crate::validate::VALIDATION_SCHEMA_VERSION,
            model_id: "test-model".to_string(),
            request_hash,
        }
    }

    fn harvest(term: &str) -> GlossaryExtraction {
        GlossaryExtraction {
            terms: vec![GlossaryEntry {
                source_term: term.to_string(),
                target_term: format!("{term}-ko"),
                note: None,
                scope: crate::llm::GlossaryScope::GlobalAcrossDocument,
                sections: Vec::new(),
            }],
        }
    }

    /// `GlossaryExtraction` carries no `PartialEq` (its entries are a wire type
    /// that has none), so assertions compare the term pairs.
    fn terms_of(extraction: Option<GlossaryExtraction>) -> Vec<(String, String)> {
        extraction
            .expect("a record was stored")
            .terms
            .into_iter()
            .map(|e| (e.source_term, e.target_term))
            .collect()
    }

    // ti `dca5bf`: the harvest store is a third independent space with the same
    // rules — overridden rather than defaulted, superseded rather than evicted,
    // and untouched by anything that happens to the other two.
    #[test]
    fn a_glossary_harvest_round_trips_in_its_own_space() {
        let cache = InMemoryCache::new();

        assert!(
            cache
                .get_glossary_extraction(&glossary_key(1))
                .unwrap()
                .is_none()
        );
        cache
            .put_glossary_extraction(glossary_key(1), harvest("tensor"))
            .unwrap();
        assert_eq!(
            terms_of(cache.get_glossary_extraction(&glossary_key(1)).unwrap()),
            vec![("tensor".to_string(), "tensor-ko".to_string())]
        );

        // Superseding is the only maintenance it needs, exactly as for a
        // metadata record.
        cache
            .put_glossary_extraction(glossary_key(1), harvest("agent"))
            .unwrap();
        assert_eq!(
            terms_of(cache.get_glossary_extraction(&glossary_key(1)).unwrap()),
            vec![("agent".to_string(), "agent-ko".to_string())]
        );
        // A different extraction request is a different record.
        assert!(
            cache
                .get_glossary_extraction(&glossary_key(2))
                .unwrap()
                .is_none()
        );

        // And it is invisible to the unit store, in both directions.
        assert!(cache.is_empty(), "len() counts unit entries only");
        cache.put(key(1), result("one")).unwrap();
        cache.evict(&key(1)).unwrap();
        assert_eq!(
            terms_of(cache.get_glossary_extraction(&glossary_key(1)).unwrap()),
            vec![("agent".to_string(), "agent-ko".to_string())],
            "a unit eviction must not reach the harvest"
        );
    }

    // DCR-0028 §5: the metadata map adopts the same clear-once-and-resume
    // poison policy, in its own domain — a panic under the metadata lock loses
    // the metadata and nothing else.
    #[test]
    fn a_poisoned_metadata_map_clears_once_and_resumes() {
        let cache = Arc::new(InMemoryCache::new());
        cache.put(key(1), result("one")).unwrap();
        cache
            .put_document_meta(
                meta_key(1),
                DocumentMeta {
                    detected_source_language: Some("en".to_string()),
                },
            )
            .unwrap();

        let poisoner = Arc::clone(&cache);
        let handle = std::thread::spawn(move || {
            let _guard = poisoner.meta.lock().unwrap();
            panic!("poison the metadata lock");
        });
        assert!(handle.join().is_err());

        assert_eq!(
            cache.get_document_meta(&meta_key(1)).unwrap(),
            None,
            "the untrusted record does not survive recovery"
        );
        assert_eq!(
            cache.len(),
            1,
            "the unit store has its own lock and is untouched"
        );

        // Writes after recovery land and are readable.
        cache
            .put_document_meta(
                meta_key(1),
                DocumentMeta {
                    detected_source_language: Some("fr".to_string()),
                },
            )
            .unwrap();
        assert_eq!(
            cache
                .get_document_meta(&meta_key(1))
                .unwrap()
                .unwrap()
                .detected_source_language
                .as_deref(),
            Some("fr")
        );
    }

    // R0003-0051 / DCR-0028 SL-110: the store holds ONE allocation per entry
    // and hands out copies of it, so the payload is not re-cloned into the map
    // on every write-through and is not duplicated per reader. Reaching into
    // `store` is the point: the `Arc` is an internal representation choice
    // (module docs), so this is the only place that can pin it, and it pins it
    // as a compile fact as much as an assertion.
    #[test]
    fn the_store_holds_one_shared_allocation_per_entry() {
        let cache = InMemoryCache::new();
        cache.put(key(1), result("one")).unwrap();

        let held: Arc<UnitResult> = {
            let guard = cache.store.lock().expect("uncontended");
            guard.get(&key(1)).cloned().expect("just written")
        };
        // The map's own handle plus the one taken above — no third copy was
        // made by writing, and none is made by reading.
        assert_eq!(
            Arc::strong_count(&held),
            2,
            "the entry must be shared, not re-cloned per handle"
        );

        // Two gets do not each add a handle: `get` materializes an owned
        // `UnitResult` and drops its `Arc` before returning.
        let a = cache.get(&key(1)).unwrap().expect("hit");
        let b = cache.get(&key(1)).unwrap().expect("hit");
        assert_eq!(Arc::strong_count(&held), 2, "gets must not retain handles");
        assert_eq!(a.translated_payload, b.translated_payload);
    }

    // The contract the Arc must not change: `get` returns an OWNED result, so
    // a caller mutating it cannot reach the stored entry. This is what stops a
    // future "return the Arc instead" shortcut from silently sharing mutable
    // state with the cache — the pipeline rewrites `unit_id` on every hit
    // (`pipeline::dispatch`), which would otherwise rewrite the cached entry.
    #[test]
    fn a_hit_is_owned_and_detached_from_the_store() {
        let cache = InMemoryCache::new();
        cache.put(key(1), result("one")).unwrap();

        let mut hit = cache.get(&key(1)).unwrap().expect("hit");
        hit.unit_id = BlockId("rewritten".to_string());
        hit.translated_payload.push_str(":mutated");

        let second = cache.get(&key(1)).unwrap().expect("hit");
        assert_eq!(second.unit_id, BlockId("one".to_string()));
        assert_eq!(second.translated_payload, "payload:one");
    }

    // Spec 2026-08-03 §6 Cache row / §7 Cache bullet: HTML-content
    // translation bumped `VALIDATION_SCHEMA_VERSION` to 2 (new
    // `html_segments` payload semantics + the always-on inline raw-HTML tag
    // guard), and the key carries that version — so no pre-feature entry can
    // replay into a post-bump run. The literal assertion is the part that
    // makes a silent revert of the bump fail here rather than silently
    // re-enable pre-guard replay.
    #[test]
    fn pre_bump_validation_schema_version_never_replays() {
        assert_eq!(
            crate::validate::VALIDATION_SCHEMA_VERSION,
            2,
            "the HTML-content bump must not be silently reverted",
        );

        let cache = InMemoryCache::new();
        let current = key(9);
        let mut pre_bump = current.clone();
        pre_bump.validation_schema_version = 1;
        assert_ne!(
            pre_bump, current,
            "the version field must participate in key identity",
        );

        cache.put(pre_bump.clone(), result("pre-feature")).unwrap();
        assert!(
            cache.get(&current).unwrap().is_none(),
            "a pre-bump entry must not satisfy a post-bump lookup",
        );
        // The miss is the version field, not a broken store: the pre-bump key
        // itself still resolves to what was written under it.
        assert_eq!(
            cache
                .get(&pre_bump)
                .unwrap()
                .expect("pre-bump key still resolves")
                .translated_payload,
            "payload:pre-feature",
        );
    }
}
