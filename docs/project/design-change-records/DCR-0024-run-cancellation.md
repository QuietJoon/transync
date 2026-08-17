---
type: DCR
title: Run cancellation — DCR-0009's YAGNI deferral is superseded, and a cancelled run answers Err
description: A CancellationToken on TranslateOptions, threaded to a new cancel parameter on Translator::translate_batch and extract_glossary; a cancelled run returns TransyncError::Cancelled rather than a partial document, and leaves its paid-for progress in the caller's cache.
tags: [change, project-control, DCR-0024]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-08T00:00:00Z
status: stable
---

# DCR-0024: Run cancellation

- **Date:** 2026-08-08
- **Source:** ticket `43331a`, filed by the dynwebserver maintainer; owner decided 2026-08-07 to do it now rather than leave it commissioned
- **Supersedes:** **DCR-0009's Migration/Follow-up YAGNI entry** — *"Deferred with YAGNI rationale: deadline/cancellation on `Translator` (revisit: long-running service consumer)"*. Its stated revisit condition has fired; the deferral is replaced by this record, not overturned.
- **Affected ADRs:** `docs/decisions/0002-http-free-core-with-translator-trait.md` (updated — 2026-08-08 amendment; it is where `Translator` is declared). `docs/decisions/0017-batch-terminal-failures-abort-the-run.md` is the precedent this record's central decision rests on, and is **unchanged**.
- **Affected contracts:** `docs/architecture/contracts.md` §0 (one new row, `transync::CancellationToken`), §1 (trait signature, stability rule, `TranslatorError::Cancelled`, two new `stable_code()` entries), new **§5b**
- **Breaking.** Carried by **v0.4.0**, under the sanctioned 0.x breaking window.

## What Changed

**A run can be cancelled.** `TranslateOptions` gained
`cancel: Option<CancellationToken>` — `tokio_util`'s token, re-exported as
`transync::CancellationToken`. `None` is the default and reproduces the
pre-0.4.0 behavior exactly.

**A cancelled run returns `Err(TransyncError::Cancelled)`** — stable code
`cancelled`, a new variant on a `#[non_exhaustive]` enum. It never returns a
partial `TranslationOutput`.

**`Translator` grew a parameter, not a method.** Both provider-facing methods
now take the run's token:

```rust
async fn translate_batch(
    &self,
    batch: TranslationBatch,
    cancel: &CancellationToken,
) -> Result<TranslationBatchResult, TranslatorError>;

async fn extract_glossary(
    &self,
    req: &GlossaryExtractionRequest,
    cancel: &CancellationToken,
) -> Result<Option<Vec<GlossaryEntry>>, TranslatorError> { … }
```

Honoring it is a **SHOULD**. `TranslatorError::Cancelled` (stable code
`provider_cancelled`) is the answer an implementation returns when it does.

**The pipeline honors cancellation at seven points**, not merely accepts a
token: run entry, after the auto-glossary preflight, each batch's entry, each
dispatch round's head, around every provider call, around every
transport-backoff sleep, and once more after the batch fan-out settles. Every
provider call and every backoff sleep is a `tokio::select!` with `biased;`, so
an already-cancelled token wins deterministically and the request is never
issued.

**The post-fan-out check is authoritative.** A cancelled run reports
`Cancelled` even when a sibling batch also failed for its own reason.

**Past the fan-out there is no checkpoint.** Regeneration, alignment and
rendering are pure CPU with no I/O; a token that fires during them is not
observed and the run returns `Ok`. That boundary is stated in §5b rather than
left to be discovered.

**Progress survives in the caller's cache.** Nothing new was built for this:
each unit is written to the `Cache` as it is accepted (§5a provisional
validity), so the OI-0011 keep-progress mechanism a failing sibling batch
already relied on is what makes a cancelled run resumable. `translate_with_cache`
is therefore the entry point a cancelling consumer should use; `translate`'s
throwaway cache keeps nothing, and its rustdoc now says so.

## Why

DCR-0009's YAGNI call was correct when it was made, and this record does not
say otherwise. It named the condition that would reverse it — *a long-running
service consumer appearing* — and that consumer now exists. `dynwebserver` is a
tokio daemon holding transync behind an HTTP endpoint, running translations as
background jobs, serving readers who navigate away mid-run.

The filer measured the cost. dynweb runs one translation job at a time by
default (`translate.max_concurrent = 1`). A run it can no longer use — reader
closed the tab, daemon took SIGTERM — could not be told to stop:

- the only available cancellation was dropping the future, which the consumer
  had to reason out from transync's source because nothing stated it;
- a stall multiplies by batches **and** by retries, and the longest single term
  is a capped 30 s `Retry-After` backoff for a job nobody wants;
- all paid progress was discarded, because a `translate` call's in-memory cache
  dies with the future;
- at shutdown dynweb bounds teardown at 3 s and simply orphans whatever has not
  finished.

Three of those four are now contract rather than inference, and the fourth —
discarded progress — is answered by pointing the consumer at
`translate_with_cache`, which already had the mechanism.

## The central decision: what a cancelled run returns

This was the design question, and the alternative was real: return a
`TranslationOutput` whose unreached blocks are marked `fallback_source`, so a
consumer keeps whatever was translated.

**Rejected, on ADR-0017's reasoning applied unchanged.** That ADR settled — at
owner level, against two proposed alternatives — that a run which cannot
complete answers `Err` and writes nothing, because *a translated document with
an untranslated island is worse than no document: the island is not visually
distinguished in raw Markdown, so a downstream reader can ship it without
noticing.* Cancellation does not weaken any part of that.

It adds a second objection of its own. `fallback_source` is a **statement about
an attempt**: this block was tried, its retries were spent, and the source was
emitted instead (invariant 6, ADR-0009). A cancelled run's unreached blocks
were never tried. Reusing the marker would make "we tried and failed" and "we
never got there" indistinguishable in the alignment map and in
`ValidationReport`, which is precisely the kind of silent conflation DCR-0023
had just finished removing from `TranslatorError::Other`.

The affordability question — "so I lose everything?" — is answered by the cache
rather than by weakening the artifact contract. That answer already existed
(OI-0011); this record only makes it load-bearing and documents it.

## Alternatives Considered

- **A defaulted `translate_batch_cancellable`, delegating to
  `translate_batch`.** The only shape that would have been additive rather than
  breaking. Rejected: two methods for one job, the pipeline must call the new
  one, and an implementor who overrides only the old one is silently
  uncancellable — a correctness bug no compiler can see. The owner's standing
  position that backward compatibility is not a constraint (transync has never
  been released; both consumers are path-dependent siblings that recompile)
  removed the only argument for it.
- **The token as a field on `TranslationBatch`.** Would have needed no
  signature change — but `TranslationBatch` has public fields and is
  exhaustive-by-policy (§1), so a field addition breaks every struct literal
  anyway: the same migration cost, plus live control state smuggled into a
  wire-shaped data type.
- **A deadline (`Instant` / `Duration`) instead of a token**, the ticket's
  other named option. Rejected as strictly weaker: a deadline cannot express
  "the reader closed the tab", which is the filer's primary case, whereas a
  deadline is derivable from a token in three lines. Not a genuine fork — one
  option dominates.
- **Batch-boundary cancellation only** (the ticket's cheaper option 3, "check a
  flag between batches"). It removes the multiply-by-batches term but leaves
  the single longest one — an in-flight provider call plus its backoff. The
  `select!` that fixes that is smaller than the flag plumbing would have been,
  so the cheap option was not cheaper.
- **Pipeline-level racing only, with no trait change.** Genuinely tempting: the
  `select!` alone delivers in-flight abort for any drop-cancellable client, and
  `reqwest` is one. Rejected because it silently excludes implementations whose
  work is *not* drop-cancellable (an inner `tokio::spawn`, a `spawn_blocking`
  client, an internal queue), leaves a directly-driven adapter with no
  cancellation at all, and would have required a *second* breaking change later
  — the expensive-to-undo outcome. Both halves shipped instead: the race is the
  guarantee that does not depend on the implementation, the parameter is what
  lets a cooperating implementation be correct.
- **A transync-local token type**, to keep a foreign crate out of §0. Rejected:
  it would cut the run token off from the composition the ecosystem already has
  (`child_token`, `DropGuard`, graceful-shutdown crates) — deriving a per-job
  token from a process-wide one is the entire use case — and force every
  consumer to convert between two types with identical semantics. The cost is
  named in §0: transync's public API now moves if `tokio-util` ever breaks
  `CancellationToken`.
- **A `deadline` knob *and* a token.** Rejected as two mechanisms for one job,
  with a murky answer to "which error does a deadline produce".

## Consequences

- **Good:** A long-running host can stop a run it no longer wants, and both the
  in-flight request and the queued batches stop.
- **Good:** The cancelled outcome is a distinct, machine-readable code
  (`cancelled`) that a consumer will not confuse with a provider failure, and
  the artifact contract is unchanged — there is still exactly one shape of
  successful output.
- **Good:** "Cancel then resume" is cheap for a consumer that owns its cache,
  and the cost of *not* owning one is now stated on `translate` itself.
- **Bad (accepted):** Every `Translator` implementation moves. In-tree that is
  the shipped adapter, both test stubs and ~40 test doubles; out of tree it is
  the two sibling consumers' test doubles. The change is mechanical (add a
  parameter, name it `_cancel` if unused) and the compiler finds every site.
- **Bad (accepted):** §0 now carries one foreign type, and with it a
  `tokio-util 0.7` compatibility promise no other row exposes.
- **Bad (accepted):** Cancellation is not observed during regeneration and
  rendering. A run whose provider work finished returns `Ok` even if the token
  fired a millisecond later. Adding a checkpoint there would discard a run that
  has already paid for every provider call, to save a few milliseconds of CPU.

## Migration

Both sibling consumers implement `Translator` only in **test** code, so neither
has production code to change beyond the call site.

**dynwebserver** (`crates/dynweb-core/src/translate/engine.rs`) — four test
doubles (`ProfileSpy`, `RequestSpy`, `PreservingTranslator`,
`DetectingTranslator`) each add one parameter:

```rust
async fn translate_batch(
    &self,
    batch: transync::TranslationBatch,
    _cancel: &transync::CancellationToken,
) -> Result<transync::TranslationBatchResult, transync::TranslatorError> { … }
```

The engine itself gains the feature it filed for: build a per-job token from
the daemon-wide shutdown token, hand it to the options, and treat
`TransyncError::Cancelled` as its own outcome class rather than a failure:

```rust
let job = shutdown.child_token();           // cancels on SIGTERM *or* on
opts.cancel = Some(job.clone());            // job.cancel() when the tab closes
match transync::translate_with_cache(src, &opts, &provider, &cache).await {
    Err(TransyncError::Cancelled) => { /* not an error to report to a reader */ }
    other => other?,
}
```

Using `translate_with_cache` with a cache that outlives the job is what turns
the 3 s bounded teardown from "orphan the work" into "keep the work" — the
units already accepted stay in the cache and the next attempt starts from them.
`classify_translator` should also grow a `TranslatorError::Cancelled` arm; it
is terminal and is not a provider fault.

**resp-translator** (`bins/copy-transfer-mcp/tests/common/mod.rs`) — two test
doubles (`StubProvider`, `FailingProvider`) take the same one-parameter edit.
Its listener hardcodes stable-code strings; `cancelled` and
`provider_cancelled` are new members of that vocabulary, but nothing it
currently produces returns either.

## Verification

- `cargo test --workspace -- --test-threads=4`
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, the wasm32
  gate and the rustdoc gate (all four are the tracked pre-commit hook)
- `crates/transync/tests/cancellation.rs` — 10 pins: no request from an
  already-cancelled run; the mid-run stop returns `Cancelled` rather than a
  document; the backoff sleep is interrupted; progress survives in a
  caller-owned cache and does **not** survive `translate`'s throwaway one; a
  token-ignoring `Translator` is cancelled by being dropped; a cancelled
  preflight aborts while a failed one still only degrades; and the two
  cancellation codes are distinct. Two of the ten are non-vacuity pins on the
  same fixtures.
- `crates/transync/tests/error_taxonomy.rs` — the vocabulary scrape now covers
  nineteen codes in both directions
- `crates/transync/tests/public_surface.rs` — the four-artifact weld carries
  the new `transync::CancellationToken` row

## Note (2026-08-08, review 0003) — `provider_cancelled` is reachable through `translate`

Review 0003 caught an overclaim this record's own trait rustdoc contradicted.
contracts.md §1, `TranslatorError::stable_code`'s comment, and the last pin in
`crates/transync/tests/cancellation.rs` all said `provider_cancelled` was
reachable *only* by driving the trait directly, because `run_pipeline` decides
a cancelled run from the token before a provider code can surface. That is true
of a run the caller cancelled, and it is not the whole set of cases: the trait
deliberately permits an implementation to answer `TranslatorError::Cancelled`
off a cancellation source of its **own** while the run's token never fires.
Then the verdict checkpoint does not trigger, the lowest-index-error selection
runs, and `translate` / `translate_with_cache` return
`TransyncError::Translator(Cancelled)` — stable code `provider_cancelled`. The
`annotate_with_preflight` comment already conceded that path; the three
consumer-facing statements had not.

Three one-sentence doc corrections, no behavior change. The pin count in
*Verification* above goes from ten to eleven: the new
`a_providers_own_cancellation_surfaces_as_a_provider_error` drives that route
end to end through `translate` with an unfired token, asserting the engine-side
`Cancelled` is *not* what comes back, that the code is `provider_cancelled`,
and that the retry ladder dispatched exactly once — a cancelled call is
terminal whoever's decision stopped it.
