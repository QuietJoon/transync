---
type: ADR
title: Cache identity is not a security boundary — adversarial hash collision is out of the threat model
description: The 64-bit non-cryptographic digests behind CacheKey, ProviderFingerprint and the glossary hash defend against accidental collision only; an attacker who can aim a collision at a cache entry is explicitly not defended against, by owner decision, and this record is what future reviews of that question resolve to.
tags: [decision, ADR-0020]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-08T00:00:00Z
status: stable
---

# ADR: Cache identity is not a security boundary — adversarial hash collision is out of the threat model

## Context and Problem Statement

Found in Review 0002 (Issue R0002-0006, reviewer severity High; verified
severity at HEAD: Low) (review archived and removed). Location: `crates/transync-syntax/src/id.rs`
(`source_hash_bytes`), `crates/transync-core/src/cache.rs` (`CacheKey`),
`crates/transync-core/src/llm.rs` (`ProviderFingerprint`),
`crates/transync-core/src/pipeline.rs` (`context_hash`, the glossary hash).

Every semantic axis of cache identity is stored as a `u64` produced by
SipHash-1-3 with public zero keys. The reviewer's point is narrow and
correct: SipHash with published zero keys is a *fast* hash, not a
collision-resistant one, so an attacker who can choose input bytes can
construct two inputs that share a digest. Combined with architectural
invariant 7 — *source Markdown is untrusted data* — that describes a path
where a crafted document collides with another document's cache entry and is
served a translation produced for different content.

Two things make this a decision rather than a bug report:

1. **Nothing in the repository had ever analyzed it.** The retired
   2026-05-02 review round raised the same question and it was never
   dispositioned. So this was not a decision being re-litigated; it was a
   decision that had never been made, and it would have kept resurfacing at
   every review until it was written down.
2. **The blast radius is a function of cache scope, which is about to
   change.** Today only `InMemoryCache` ships: a cache lives and dies inside
   one process, and an attacker who can put a document into that process can
   already read its output. Commissioned ticket `f12b8b` designs a
   disk-backed cache, which is the first design where a cache could outlive a
   run or be shared between them.

## Decision Drivers

* Who transync is for: it translates documents its operator chose to
  translate, with an API key its operator supplied. It is not a multi-tenant
  service accepting documents from mutually distrusting parties.
* Accidental collision at 64 bits over any realistic corpus is not a concern;
  only a deliberately constructed one is.
* The cost of the alternative is not the hash — it is the versioning,
  invalidation and key-size consequences that ripple through `CacheKey`,
  the provider fingerprint, and any on-disk format.
* Silence is the expensive option: an unanswered question in a security-shaped
  area gets re-raised by every future reviewer, and each re-raising costs a
  verification pass.

## Considered Options

1. **Declare adversarial collision out of scope and record it** — keep the
   64-bit non-cryptographic digests, state the threat model explicitly, and
   make future occurrences of this finding resolve to this record.
2. **Widen or key the digest now** — move to a 128-bit or keyed hash and
   version the key. Invalidates every cached entry once, and pre-empts a
   design pass (`f12b8b`) that may want to choose differently with the
   on-disk format in view.
3. **Store canonical bytes in the key** — the reviewer's own recommendation.
   Removes collision entirely and removes the point of hashing with it: the
   key becomes as large as the content it identifies.

## Decision Outcome

**REJECT the finding, option 1.** Owner decision, 2026-08-07, stated
verbatim: *"이 프로그램에서는 적대적 충돌은 전혀 고려하지 않는 것을 명확한
전제로 합니다"* — adversarial collision is explicitly and entirely outside
what this program considers.

Cache identity in transync defends against **accidental** collision only. It
is not a security boundary, and no part of the system should be described as
if it were. A finding that an identity digest can be collided by an attacker
who controls the input is, from this record forward, a known and accepted
property — not a defect.

Status: Implemented (the decision is the record; no code changed).

### Implementation

No code change. The digests, key shapes and framing stay as they are.

Note what this decision does **not** cover, because the distinction is easy
to lose: *framing ambiguity* is still a defect. R0002-0007 and R0002-0008
found that `ProviderFingerprint` joined its fields with `U+001F` and the
glossary hash separated its fields with NUL, both unescaped and without
length prefixes, so two distinct inputs could produce byte-identical buffers
*before* hashing. That is not a hash-strength question — it is an encoding
that loses information — and it was fixed under this same review, adopting
the per-field presence marker plus fixed-width length prefix that
`context_hash` already uses (ticket `53d495`, commit `de7d6d3`). An encoding
must remain injective; the hash over it need only resist accident.

## Consequences

* Good, because the question is answered in one place. Reviews 0003+ and the
  `f12b8b` disk-cache design pass inherit this premise instead of re-deriving
  it, and a reviewer who raises it again is answered by citation rather than
  by another verification pass.
* Good, because it keeps `CacheKey` small and the cache fast, and leaves the
  on-disk format free to choose its own identity scheme when `f12b8b` designs
  one.
* Bad, because it is a real limitation stated plainly: transync must not be
  deployed as a shared translation service that accepts documents from
  mutually distrusting parties while sharing one cache between them. If that
  deployment ever becomes a goal, this ADR is what has to be superseded, and
  `f12b8b` is the natural place to do it.
* Bad, because "not collision-resistant" now appears in the record where a
  casual reader may take it as an unqualified weakness. The qualification —
  *by decision, for a single-operator tool* — travels with it here and should
  travel with any restatement.

#### Appended 2026-08-09 (DCR-0028, ticket f12b8b)

The `f12b8b` design pass this record anticipated has run, and it resolved the
question the way the *Consequences* above left open: the on-disk format
**chose no identity scheme of its own**. An entry's persisted identity is its
full serialized axis set (serde JSON over the public types), and lookups
compare full-key equality after deserialization — the store never keys by a
hash of the key, so persistence adds **zero** collision surface beyond the
u64 axes this record already dispositions. The injectivity discipline the
Implementation section preserves is satisfied by construction there: JSON
field names are the per-field presence markers and JSON string framing the
length prefixes, so no hand-rolled framing exists to get wrong. The premise
itself — accidental collision only, not a security boundary, single-operator
deployment — was inherited as stated, not re-derived. Design: DCR-0028;
principles: ADR-0021.
