---
type: Reference
title: Settled questions — the register of review findings the record already answers
description: A reviewer-facing index of the eight questions that three independent review rounds (0002, 0003, 0004) kept re-raising after they were already decided and recorded, with the exact prior finding ids and the authoritative record to cite; an index only — the linked record governs on any conflict.
tags: [architecture, review, settled-questions]
generated:
  by: claude-code/claude-fable-5
  at: 2026-08-13T00:00:00Z
status: stable
---

# Settled questions — a register for reviewers

This file is an **index, not an authority**. Document authority in this
repository is governed by the hierarchy in
[`design-baseline-2026-07.md`](../project/design-baseline-2026-07.md)
(`BL-2026-07-B`): contracts and test suites define intended behavior,
architecture docs define structure, ADRs carry rationale, DCRs carry deltas,
code is evidence. This register sits below all of them. Every entry here is a
pointer plus a summary; **on any conflict between a summary here and the record
it links, the linked record governs and this file is what gets fixed.** Nothing
here is a second source of truth, and nothing here supersedes, amends, or
re-argues a linked record.

What it is for: three independent review rounds have now paid, repeatedly, to
re-derive answers the repository already held. This file lets the next round —
reviewer or verification gate — answer a recurring finding **by citation**
instead of by re-derivation.

## Which rounds these finding ids belong to

Every `R0002-####`, `R0003-####` and `R0004-####` id in this file names a
**2026-08 gated round**:

| Round | Completed | Findings |
|---|---|---|
| 0002 | 2026-08-07 | 86 |
| 0003 | 2026-08-07 | 90 |
| 0004 | 2026-08-11 | 100 |

The registry `reviews/README.md` also lists 2026-05 rounds numbered `0002`
(92 findings), `0003` (86) and `0004` (1 — an external `resp-translator`
consumer report), so the numbers really do collide.

**That is settled now and this file no longer needs a local rule** (ti
`bdf8d981`, 2026-09-03). Registry rule 4 used to assert the rounds did not
collide, which was false, and this section carried a scoped workaround —
"read every bare id **in this file** as the 2026-08 round" — until it was
fixed. Rule 4 now says the same thing repository-wide: a bare
`R0002`/`R0003`/`R0004` id means the 2026-08 gated round, and a 2026-05 one is
marked. The table above is kept because it is the shortest statement of these
three rounds' shape, and rule 4's range table was verified against it.

The three source review files were archived and removed after their findings
were mined into the records this file points at. Their text is **not** in this
repository's git history — no commit here ever held them — but it survives as
unreferenced blobs in the salvaged stores, and the registry's round table
carries the exact `cat-file` command for each.

## The measurement behind this file

Reviews 0002 (86 findings), 0003 (90) and 0004 (100) — 276 findings in total —
were each adjudicated by a verification gate. Of the 276:

- **76 findings (28%) were rejected as contradicting a recorded decision or as
  false** — they argued against something the project had already decided and
  written down.
- Measured by where the rejecting evidence lived: **45 of the 76 (59%)** were
  answered by a `docs/` record (ADR, DCR, `contracts.md`, or a spec);
  **32 (42%)** were answered by a rustdoc or inline comment **directly above
  the flagged code**; **12 (16%)** were self-evident from reading the code;
  14 had the answer in both code and docs. Exactly one (R0002-0021) was
  answered only by a TicGit ticket, and even that has since been promoted into
  `contracts.md` §7 and provider rustdoc — so at HEAD, **zero** recurring
  findings lack a repo-readable answer.

Two honest conclusions from that, stated because they bound what this file can
achieve:

1. **The fix for recurrence is not "write more records".** 42% of the rejected
   findings were already answered by a comment adjacent to the very line being
   flagged. The answer existed at the point of maximum visibility and was not
   read.
2. **Writing "do not re-raise this" does not stop re-raising.**
   [ADR-0008](../decisions/0008-reject-optional-warnings-in-strict-schema.md)'s
   own text says it exists "so future reviews do not re-raise the asymmetry" —
   and Review 0003 re-raised it twice anyway (R0003-0035, R0003-0036).

So this register attacks the one step it can: it puts the recurring questions
in **one** place, phrased the way reviewers phrase them, so a filing-time check
against a single file replaces a search the previous rounds demonstrably did
not perform.

## How to use this before filing a finding

1. **Read the doc comment on the item you are flagging, and its module
   header.** By the measurement above, that alone would have answered 42% of
   the rejected findings.
2. **Scan the questions below.** If your finding is one of them, cite the
   governing record (and, if useful, the prior finding ids) instead of filing
   it again.
3. **If you believe the settled answer is wrong, file against the record, not
   the code**: name the ADR/DCR/contract section your finding contradicts and
   argue why it should be superseded. A finding that names the record it
   disputes is a design challenge and gets adjudicated as one; a finding that
   ignores the record gets rejected by citation to it.
4. A question **not** in this file is not thereby fresh — this register only
   indexes what has recurred across rounds. The authority hierarchy and the
   records themselves are still the ground truth.

---

## 1. Output publication: the lock, the marker, the leftovers, the two-rename replace

**As reviewers phrase it:** "the publish lock can wait forever / has no
timeout"; "the `.transync-publish.lock` file is never deleted"; "stale
`*.tmp.<pid>` / staging leftovers from other runs are never cleaned up";
"`--out-dir` replacement is not atomic and a reader can see a missing target";
"a failed directory fsync is reported but the run still succeeds".

**Settled answer:** every one of those is a recorded decision, not an
oversight. Publication serializes on a kernel file lock (released on process
death, so no TTL, no staleness rule, no steal protocol); contention waits by
design; the marker deliberately survives the run because unlinking a lock file
reintroduces the fresh-inode race it exists to close; only staging temps
carrying the running process's own pid are reclaimed, because a foreign pid is
not evidence of death; replacing an existing target is crash-safe but not
atomic, and the contract says so rather than the code pretending otherwise; a
failed directory flush is advisory because the payload bytes are already
fsynced.

**Prior findings:** R0002-0024, R0002-0027, R0002-0030; R0004-0004,
R0004-0053, R0004-0055, R0004-0058, R0004-0060, R0004-0061, R0004-0062,
R0004-0064, R0004-0065.

**Governing record:**
[ADR-0024](../decisions/0024-output-publication-protocol.md) (the reasoning),
[`contracts.md` §6](contracts.md) (the observable contract),
[DCR-0021](../project/design-change-records/DCR-0021-publication-lock-and-honest-replacement.md)
(the change history and its appended notes).

## 2. The wasm engine validates neither the map nor the source

**As reviewers phrase it:** "`transync-wasm` deserializes an `AlignmentMap`
and acts on it without checking schema version / document identity / row
coverage / duplicate ids — a stale, foreign or crafted map is rendered without
complaint."

**Settled answer:** the engine has **no trust boundary to defend**: it is a
stateless function whose caller supplies the source and the map *together*, so
there is no seam at which one side is trusted and the other is not. The
consistency gates live in the demo shell that assembles the pair, not in the
engine. Findings premised on the engine defending its caller from its caller
argue against this record.

**Prior findings:** R0002-0009, R0002-0012, R0002-0013, R0002-0055,
R0002-0057, R0002-0058; R0003-0061, R0003-0062, R0003-0079, R0003-0080;
R0004-0091.

**Governing record:**
[ADR-0023](../decisions/0023-wasm-engine-has-no-trust-boundary.md).

## 3. The disk cache: durability, bounds, eviction, poison

**As reviewers phrase it:** "the cache log is not fsynced per write";
"`InMemoryCache` is unbounded"; "document metadata is never evicted"; "the
poison policy clears the whole cache once instead of failing"; "two processes
sharing a cache directory corrupt it".

**Settled answer:** the cache is an **accelerator whose failure degrades to
re-translation, never to wrong output** — that stance decides each of these.
Durability per append is deliberately not promised (a torn tail is discarded
at open); one writer per directory is documented as unsupported with lost
entries, never corrupt output, as the cost; and poisoned or invalid state is
cleared and resumed because every cache hit is re-validated through the full
per-unit layer stack before reuse, so a bad entry is evicted rather than
trusted.

**Prior findings:** R0004-0027, R0004-0029, R0004-0033, R0004-0035,
R0004-0036, R0004-0037, R0004-0038, R0004-0039, R0004-0040.

**Governing record:**
[DCR-0028](../project/design-change-records/DCR-0028-disk-backed-cache-and-document-metadata.md)
(the design),
[ADR-0021](../decisions/0021-disk-cache-jsonl-log-and-document-metadata.md)
(the principles), and — for the poison/invalid-hit rule specifically —
[ADR-0015](../decisions/0015-cache-poison-and-invalid-hit-policy.md).

## 4. Which provider failures retry and which abort

**As reviewers phrase it:** "error X should be retried instead of aborting the
run" / "error Y is retried but is actually permanent" — for some HTTP status,
finish reason, or provider error shape.

**Settled answer:** the terminal/transient split is a recorded classification,
not an accident of implementation. Only `Network` and `RateLimited` are
transient, retried on a bounded per-batch budget with backoff; every other
cause is terminal because a verbatim resubmission is the same request, and a
terminal provider error aborts the run rather than degrading to fallback
output (ADR-0017). The status-by-status and finish-reason-by-finish-reason
reasoning is written where the classification happens.

**Prior findings:** R0002-0021, R0002-0022, R0002-0040; R0003-0017;
R0004-0019.

**Governing record:** [`contracts.md` §7](contracts.md) (and §5 for the retry
budgets), plus the rustdoc of the classifying modules in
`crates/transync-openai/src/client/` (`chat.rs`, `responses.rs`,
`classify.rs`).

## 5. What `transync serve` does and does not defend against

**As reviewers phrase it:** "the serve command does not defend against a
hostile file in the served tree / a local attacker who can write into the
bundle / symlinks planted under `--rendered`".

**Settled answer:** `transync serve` is a loopback static file server for the
operator's own bundle, and a local user (or process) with write access to
transync's own paths — the served tree included — is **outside the threat
model by recorded decision**. Findings that begin "an attacker who can write
into the served directory…" rest on a premise the project has explicitly
declined to defend against. The server's real boundaries (loopback default
bind, the `Host` rule, the non-loopback warning) are contractual and
documented.

**Prior findings:** R0004-0003, R0004-0049, R0004-0050, R0004-0052.

**Governing record:**
[ADR-0022](../decisions/0022-local-user-threat-model.md) (the threat-model
premise) and the `transync serve` note in [`contracts.md` §6](contracts.md)
(the observable behavior; ticket `b791d6`).

## 6. `target_block_id` always equals `source_block_id`

**As reviewers phrase it:** "the alignment map carries two id fields that are
always identical — either the pairing is broken, the second field is dead
weight, or renamed target ids were meant to be supported".

**Settled answer:** identity pairing is **normative for schema 1.x**: the same
block id flows from source IR through translation to the regenerated document,
so `target_block_id == source_block_id` in every row is the contract, not a
degenerate case. The two-field shape is deliberately kept as the schema-2.x
evolution path; a consumer that pairs by identity is correct today, and a 1.x
producer that emits differing ids is out of contract.

**Prior findings:** R0002-0046; R0003-0085; R0004-0086.

**Governing record:** [`contracts.md` §3](contracts.md) (normative, ticket
`d3acc3`) and
[ADR-0001](../decisions/0001-block-level-alignment-as-sync-currency.md) (why
block id is the only sync currency).

## 7. Cache-identity hashes are not collision-resistant

**As reviewers phrase it:** "`CacheKey` / `ProviderFingerprint` / the source
hash use a 64-bit non-cryptographic hash with public keys, so an attacker who
controls input bytes can construct a collision and poison the cache".

**Settled answer:** correct as a statement about the hash, and **out of the
threat model by explicit owner decision**: cache identity defends against
accidental collision only and is not a security boundary. transync is a
single-operator tool, not a multi-tenant service sharing one cache between
mutually distrusting parties. The distinction the record itself draws still
holds: framing ambiguity (a non-injective encoding *before* hashing) remains a
defect; hash strength against an adversary does not.

**Prior findings:** R0002-0019; R0003-0054; R0004-0005.

**Governing record:**
[ADR-0020](../decisions/0020-cache-identity-excludes-adversarial-collision.md).

## 8. Strict schema out, lenient parse in

**As reviewers phrase it:** "the output schema declares `warnings` required
while the local parser accepts it missing — the schema and the parser
disagree".

**Settled answer:** the asymmetry is deliberate and forced. OpenAI strict
Structured Outputs requires every declared property to be `required`, so the
wire schema cannot make the field optional; the local parser stays lenient so
that compliant-but-lenient surfaces (proxies, other providers) are not
rejected for omitting what they may legitimately omit. Postel-style: strict in
what is requested, tolerant in what is accepted. Both halves now live side by
side in `transync-core::llm::prompt`.

**Prior findings:** R0003-0035, R0003-0036 (originally R0006-0020).

**Governing record:**
[ADR-0008](../decisions/0008-reject-optional-warnings-in-strict-schema.md).
