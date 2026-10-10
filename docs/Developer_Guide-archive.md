---
type: archive
title: Developer Guide archive — migration notes and history moved out of the guide
description: The historical half of docs/Developer_Guide.md, moved out in the wave-3 documentation compaction. It holds the v0.2 and v0.4 migration notes for custom Translator and Cache implementors, and every dated or "since" / "used to" sentence the living guide no longer carries, each verbatim under a heading naming the guide section it came from. It is an archive — verbatim, append-only, never read for current truth. The live guide is docs/Developer_Guide.md.
tags: [archive, reference]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-06T00:00:00Z
status: deprecated
---

# Developer Guide archive

This is an **archive** (DCR-0054). `docs/Developer_Guide.md` is a living
document: it says what is true now. Everything below was true once and is kept
verbatim, with its dates, version numbers, ticket hashes and record ids intact.

Read this to answer *what changed for a custom `Translator` or `Cache` at v0.2
or v0.4*, or *when and why a guide statement took its current form*. Read
`docs/Developer_Guide.md` for the current rules and
`docs/architecture/contracts.md` for the contract itself.

## Migration notes — from "Implementing a custom Translator"

Moved from `docs/Developer_Guide.md`, section *Implementing a custom
Translator*, where they stood as two subsections between *Reading the unit
shape* and *Wiring it into the pipeline*. Records: DCR-0009 (v0.2), DCR-0024
(v0.4). The rules they carry that still bind an implementor are stated in the
guide's *Cache identity, unit construction, errors and cancellation*.

### v0.2 migration notes (custom `Translator` / `Cache` implementors)

v0.2 moved three provider/cache-boundary surfaces (DCR-0009). If you
carry a custom impl across the bump:

- **`Translator::fingerprint()`** now namespaces every cache entry by the
  producing translator, so a shared cache cannot replay one provider's
  output for another's request. The default derives the fingerprint from
  the implementing type name — correct only when every instance of your
  type is interchangeable. If your instances can differ (configurable
  model, endpoint, prompt template), **override `fingerprint()`** to
  cover every output-affecting axis. Over-distinguishing only costs a
  redundant re-translation; under-distinguishing lets a shared cache
  replay foreign output.
- **The `Cache` trait is now fallible and gained `evict`.** `get` /
  `put` / `evict` all return `Result<_, CacheError>` (the pipeline
  degrades on a cache error — miss / not-persisted / stale — and never
  aborts). `evict` (remove exactly one key; an absent key is `Ok(())`)
  is **required** — it is the mechanism behind document-level
  disqualification eviction (contracts §5a).
- **`TranslationUnit` gained `retry: Option<RetryContext>`,** and became
  `#[non_exhaustive]` in the OI-0027 curation. Construct it with
  `TranslationUnit::new(unit_id, block_kind, input_mode, source_payload,
  source_hash)` — which defaults `retry` to `None` for a first dispatch —
  plus `with_context` / `with_constraints` / `with_batch_id` /
  `with_retry` as needed. Struct literals are in-crate only, so field
  additions no longer cost you a migration.
- **The error enums are `#[non_exhaustive]`.** Matches over
  `TranslatorError`, `TransyncError`, and `ParseError` need a wildcard arm;
  variant additions are non-breaking. `TransyncError::stable_code()` and
  `TranslatorError::stable_code()` give you a stable machine string per
  variant if you need to branch across a process boundary — the former
  delegates to the latter for provider failures, so one call answers either
  way (contracts.md §1 holds the twenty-two-code vocabulary — 8 engine-side
  plus 14 provider-side).
- **`TranslatorError` states a cause, not a policy.** Each variant names why
  the provider stopped — a content-policy stop, an exhausted output ceiling,
  a model refusal, an oversize response, a rejected request — and none of them
  says whether to retry. That is yours to decide; note that of the causes
  above, none survives a verbatim resubmission, which is why the pipeline
  re-dispatches only `Network` and `RateLimited`. A wildcard arm is mandatory
  but is the *wrong* home for those five: they are exactly the failures a
  reader must be told not to retry. Reach for `Other` in your own
  `Translator` impl when nothing fits — it is a permanent catch-all, not a
  deprecated bucket.

### v0.4 migration notes (custom `Translator` implementors)

One signature change, and it is not optional — a defaulted method was
considered and rejected precisely because it could be missed (DCR-0024,
contracts.md §5b).

- **`translate_batch` and `extract_glossary` take `cancel: &CancellationToken`.**
  Add the parameter. If you do not intend to use it, name it `_cancel` and you
  are done: the pipeline races your future against the same token and drops it,
  which aborts an in-flight `reqwest`-style request. If your work is *not*
  drop-cancellable — an inner `tokio::spawn`, a `spawn_blocking` client, an
  internal queue — or if callers drive your adapter directly, race it yourself
  and return `TranslatorError::Cancelled`. See *The cancellation token* above.
- **`TranslatorError::Cancelled` is new** (stable code `provider_cancelled`),
  as is **`TransyncError::Cancelled`** (stable code `cancelled`). Both enums
  are `#[non_exhaustive]`, so your matches keep compiling — but a cancelled run
  is not a failure to report to a user, and a wildcard arm will treat it as
  one.
- **Cancelling a run is opt-in from the caller's side:**
  `opts.cancel = Some(token)`. Leaving it `None` reproduces the pre-0.4
  behavior exactly. If you may cancel, use `translate_with_cache` with a cache
  you own — the units already accepted are in it, and that is the only place a
  cancelled run's paid-for work survives.

## Sentences rewritten in place — dated or "since" / "used to" history

Each entry is the sentence or paragraph exactly as `docs/Developer_Guide.md`
carried it before its history framing was removed. The heading names the guide
section it came from, its topic, and the date, release or record it carried;
the guide states the current rule in the same place, in the present tense.

### Feature 1 › Long-document batching + cache — the document-level detection record (v0.4.0)

An in-memory
  `Cache` keyed on `{provider_fingerprint,
  validation_schema_version, source_hash, source_lang, target_lang,
  profile_version, profile_prompt_hash, glossary_hash, model_id,
  block_kind, input_mode, context_hash, instruction_hash}` makes partial-resume runs
  byte-identical to the original — `detected_source_language` included
  since v0.4.0: the `Cache` also holds one **document-level** record per
  document, written from a live run's qualifying envelope and replayed by
  a run that dispatched zero provider batches, so a fully-cached resume
  reports the detection the live run made rather than `null`
  (contracts.md §5a).

### Build, test, lint › the pre-commit hook — the rustdoc bin-only leg (2026-09-06)

The pre-commit hook runs `cargo fmt --check`, `cargo clippy
-D warnings`, and both standing gates above — wasm, and rustdoc in its
**two** legs (the library one and, since 2026-09-06, the bin-only one);
any of them failing blocks the commit.

### Build, test, lint › the rustdoc gate — widened to `transync-openai` (DCR-0018, 2026-08-07)

**The rustdoc gate** (DCR-0018, widened to `transync-openai` on
2026-08-07) keeps `cargo doc` warning-free for **every workspace member
that has a library target** — `transync-lang`, `transync-html`, `transync-syntax`,
`transync-core`, `transync`, `transync-openai`, `transync-anthropic`, `transync-google`, `transync-wasm`.

### Build, test, lint › `transync-cli` joins the rustdoc gate — the ten broken links (2026-09-06, OI-0043)

**`transync-cli` is inside it too, since 2026-09-06, on its own leg.** It
was excluded on the ground that it is bin-only with no public API to
document — true, and beside the point: `--document-private-items` is how a
developer reads this crate, and that reader is who the links are for. With
nothing checking them, nine broke silently, four of them in
`crates/transync-cli/src/output/lock.rs` — a file OI-0043's `output.rs`
split never opened, whose `super::X` references simply became sibling
references when the concerns moved out — plus a tenth latent behind
`#[cfg(not(unix))]`. It is a **second** `cargo doc` invocation rather than
two more `-p` entries on the first, because `--document-private-items`
documents the private items and so stops `rustdoc::private_intra_doc_links`
from firing, which is exactly the failure the library leg exists to catch.
Its list is `RUSTDOC_GATE_BIN_CRATES`, beside the other one.

### Build, test, lint › the rustdoc gate on every commit (2026-08-07)

The list lives in
`scripts/lib/rustdoc-gate.sh`, sourced by `scripts/smoke.sh` — which runs
the gate after the test suites — and by `scripts/hooks/pre-commit`, which
runs it on **every commit** (since 2026-08-07; a warm re-document of the
nine crates costs about a second, next to the `--all-targets
--all-features` clippy run already there).

### Build, test, lint › repository integrity — the first object loss (2026-08-07, ticket `7a7feb`)

**Repository integrity is not assumed here, it is checked.** This clone
lost a git object once (2026-08-07, ticket `7a7feb`: the `docs/` subtree
of one commit disappeared from `.git/objects`, and everything crossing
that commit failed with `fatal: unable to read tree`). The cause was a
file-sync client re-materializing files under `.git/` while git wrote
them, on the external volume the worktree lives on; it is parked rather
than removed. Two consequences for anyone working here:

### Build, test, lint › repository integrity — the pointer to the tree rebuild (ticket `7a7feb`)

`docs/Troubleshooting.md` carries the diagnosis and the hash-verified
tree rebuild that recovered the lost object.

### Using transync as a library › `CacheKey` fields by release (0.3.0, 0.4.0)

`CacheKey` is exhaustive by policy, so a field addition is a breaking
change — `instruction_hash` is the 0.3.0 one, `input_mode` the 0.4.0 one.

### CLI reference › `--force` — staging leftovers at an `--out-dir` top level (ti `66339b`)

Three file classes are
transync's own rather than foreign, and every guard skips all three — the two
markers only when the entry wearing the name is a *regular file*, since those
are the two names a guard accepts without looking inside: the
`.transync-publish.lock` marker, the `.transync-out-dir` ownership marker, and
`*.tmp.<pid>` staging leftovers — the last of these now including an `--out-dir`
target's top level, which used to be the one place the allow-list had no room
for them (ti `66339b`; `Troubleshooting.md` has the operator's view).

### CLI reference › `--force` — the `--out-dir` ownership question (OI-0036)

`--out-dir`
also asks one question the bundle guard does not: whether transync *published*
the target, answered by the ownership marker or by the complete output set being
present, so a user directory whose only entry happens to be called `out.md` is no
longer replaced without a flag (OI-0036).

### CLI reference › `transync serve` — the fidelity door (ADR-0025, ti `490d97`)

Since the HTML→HTML wave (ADR-0025, ti `490d97`), serve is also the
**fidelity door**: an `--input-format html` run's `--out-dir` holds the
translated page itself — `out.html`, full head, scripts, styles,
anchor-free — beside the sync bundle in `html/`.

### CLI reference › `transync serve` — the deferred stub it retired (STUB-061, SL-13, 2026-08-09)

This retires the deferred stub the command shipped as (STUB-061) from SL-13
until 2026-08-09, which printed an address and exited `5`. Any other static
server still works — the bundle is self-contained — but `scripts/test-browser.sh`,
`scripts/smoke-live.sh` and `web/SMOKE.md` all use this one now. `contracts.md`
§6 has the refusal rules and the exit codes in full.

### CLI reference › Exit codes — `6` and `7` added in 0.4.0

Codes are append-only: `6` and `7` were added in 0.4.0 and nothing in
`0`–`5` changed meaning. If you are matching on `5` today to catch provider
failures, note that four of the causes that used to land there now land on
`6` and two on `7`; `5` keeps the ones the taxonomy does not classify.
Library callers get the finer split without exit codes at all, from
`TransyncError::stable_code()` (`contracts.md` §1).

### CLI reference › Batching + concurrency knobs — a zero is no longer floored to 1

It is **not** floored to 1, so a stray zero can no longer
put every block in its own batch, or turn a run sequential, without
saying so.

### CLI reference › Dual API dispatch — `TRANSYNC_OPENAI_API` read once at construction

For the
CLI that is invisible (it builds the provider from the environment at
startup), but a long-lived library host that mutates the variable mid-process
no longer risks a run whose cache namespace names one surface while its
requests go to the other — set the variable before constructing, or build a
second adapter to change surface.

### CLI reference › Smoke scripts — `test-browser.sh` stale-bundle refusal (ti `ed2e73`)

**A bare `pnpm exec playwright test` now refuses a bundle older than its inputs** (ti `ed2e73`): the script ends in that bare command, so running it directly skips the regeneration and would green a bundle that predates your edits.

### Custom Profile TOML › Glossary handling — `scope = "section"` honored (v0.4.0, DCR-0027)

`scope = "global"` is applied
unconditionally; `scope = "section"` is **honored per section** since
v0.4.0 (DCR-0027; it was rejected with a now-removed
`ProfileError::Unsupported` until section-coherent batching made
per-section filtering possible).
