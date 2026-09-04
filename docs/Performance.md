# Performance Tuning

How to make transync faster — or, when faster isn't possible, how
to confirm you're already at the floor for your provider.

The defaults in `TranslateOptions::default()` are tuned for documents
of 50–500 blocks against `gpt-5-chat-latest` and similar models.
This guide is the dial-by-dial reference for departing from those
defaults.

## The latency model

Wall-clock time for a translate run is approximately:

```
total ≈ parse + (⌈batches ÷ max_concurrent_batches⌉ ⋅ slowest_round_trip)
      + validate + regen + render
```

Batches dispatch concurrently up to `max_concurrent_batches`, so the
round-trips cost you *rounds*, not batches: raising the cap lowers the
middle term rather than raising it. For most non-trivial documents that
term dominates. Three levers move it:

1. **Number of batches** — controlled by
   `target_input_tokens_per_batch` and `max_units_per_batch`. Fewer,
   bigger batches → fewer round-trips.
2. **Concurrency cap** — `max_concurrent_batches`. More in-flight
   requests → batches run in parallel, not series. Limited by
   provider rate limits and your account's TPM/RPM tier.
3. **Per-call latency** — driven by model choice and network. Some
   models have markedly higher per-call latency floors regardless
   of payload size.

Parse, validation, regeneration and rendering together are typically
<5 % of a live run's total time. Don't optimize them.

## The defaults

| Field | Default | Effect of increasing |
|---|---|---|
| `max_concurrent_batches` | `6` | More parallelism → faster, until rate-limited |
| `target_input_tokens_per_batch` | `6000` | Bigger batches → fewer round-trips, more risk per failure |
| `max_units_per_batch` | `32` (profile: `8`) | Hard cap; bigger → less granular retries |
| `max_per_unit_validation_retries` | `2` | More forgiving on flaky models; slower on bad runs |
| `max_per_batch_schema_retries` | `2` | Survives a provider that drops or duplicates result rows; slower when it does |
| `max_per_batch_provider_retries` | `1` | Survives transient 5xx; doesn't help on permanent errors |

Each batching dial in this table is also a CLI flag
(`--max-concurrent-batches`, `--target-input-tokens-per-batch`,
`--max-units-per-batch`), as are the two output-side knobs that have no
`TranslateOptions` field at all and live only in the profile's
`[batching]` section (`--target-output-tokens`,
`--output-expansion-factor`). The three retry budgets are library-only:
set them on `TranslateOptions`.

**`max_units_per_batch` is 8 on a default run, not 32.** The `Default`
column is `TranslateOptions::default()`; what the packer runs with is
resolved caller-then-profile-then-that-default, and "caller" means a
value *differing* from the built-in one — there is no `Option` in which
to say "unset", so passing `32` is read as leaving the field alone. The
shipped default profile sets `[batching].max_units_per_batch = 8`
(alongside `target_output_tokens = 8000`) and wins on every run that
leaves the field alone. Every Rust recipe below that wants bigger
batches therefore has to name a value other than 32 — the ones that
raise the cap already do. The CLI flags are exempt: they overlay the
resolved profile rather than `TranslateOptions`, so
`--max-units-per-batch 32` really does mean 32.

Two things bound the batch count that no dial in this table touches.
Batches never straddle a heading (DCR-0027: the units are partitioned by
section and each section packed on its own), so a document with *N*
headings costs at least *N* batches — *N*+1 when it opens with prose
before the first heading. The fastest way to fewer round-trips is often
a flatter document, not a bigger budget. And `target_output_tokens`,
which lives only in the profile, breaks a batch on the estimated
*response* size when the input target has not bound yet.

`TranslateOptions` is `#[non_exhaustive]`, so every Rust recipe below
constructs it by default-then-assign. A struct literal — even with
`..Default::default()` — does not compile outside the engine crates.

## Recipe — Small documents (≤50 blocks)

The defaults are already overkill on the *token* axis: 50 short blocks
are nowhere near one 6000-token batch. It is the other two bounds that
set the batch count — the profile's 8-unit cap, and one batch per
section. Measured on synthetic 50-block documents under the shipped
defaults: 50 headingless paragraphs pack into **7** batches
(`8×6 + 2`), and the same 50 blocks arranged as ten `##` sections of
five pack into **10**, one per section. Concurrency=6 is not wasted on
either shape; it is what makes them cheap.

If you want to minimize latency further, raise the unit cap as well.
The token target alone cannot merge batches the unit cap is splitting —
adding only the `target_input_tokens_per_batch` line below leaves the
headingless document at the same 7 batches:

```rust
let mut opts = TranslateOptions::default();
opts.max_concurrent_batches = 2;           // enough for a one- or two-batch doc
opts.target_input_tokens_per_batch = 8000; // room for the whole doc, token-wise
opts.max_units_per_batch = 64;             // must differ from 32, or the profile's 8 stands
```

All three together collapse the headingless document to a single batch.
Nothing collapses the sectioned one below one batch per heading: it
stays at 10.

## Recipe — Medium documents (50–500 blocks)

Defaults are tuned here. If wall-clock disappoints, your model
choice is usually the lever, not the batching.

`gpt-4o-mini` is roughly 2–3× faster than `gpt-5-chat-latest` for
small batches, with a quality tradeoff:

```bash
TRANSYNC_OPENAI_MODEL=gpt-4o-mini ./scripts/test.sh
```

That is the live path: `scripts/test.sh` needs `OPENAI_API_KEY`, and
when the run finishes it `exec`s a static server on the bundle instead
of exiting — Ctrl-C once you have your timing.

If you're staying on `gpt-5*` and still want more parallelism:

```rust
let mut opts = TranslateOptions::default();
opts.max_concurrent_batches = 12;          // requires Tier-3+ OpenAI account
```

## Recipe — Large documents (>500 blocks, >100 KB)

Your bottleneck is provider rate limits, not transync. Two patterns:

1. **Crank concurrency to your tier limit.** OpenAI Tier-1 caps at
   ~500 RPM for `gpt-4o`; Tier-3 at ~5000. Set
   `max_concurrent_batches` to whatever doesn't trip 429s.
2. **Bigger batches.** Push `target_input_tokens_per_batch` to
   10000 — on a model whose input context has room for it. The target
   is a soft ceiling on the batch's *whole* input: the packer reserves
   the compiled system prompt (rendered glossary included), the
   instruction envelope and the two language labels off it once per
   batch, then spends what is left on unit payloads plus a fixed
   per-unit JSON overhead. The default 6000 is sized to leave the
   model's own output room inside an 8K input context; 10000 is not a
   value that fits one. Reduces total round-trips.

```rust
let mut opts = TranslateOptions::default();
opts.max_concurrent_batches = 20;
opts.target_input_tokens_per_batch = 10000;
opts.max_units_per_batch = 64;
```

Bigger input batches also mean bigger responses. When the profile sets
`[batching].target_output_tokens` (CLI: `--target-output-tokens`), the
packer breaks a batch on whichever ceiling binds first and the
validation report's `output_budget_warnings` names the units it
estimates over the ceiling; with no ceiling set, an over-long response
truncates and aborts the run instead — see `docs/Troubleshooting.md`,
"`incomplete (reason: max_output_tokens)`".

Rate limiting is not a value in any artifact: `RateLimited` is a
`TranslatorError` variant, so a rate-limited run reports itself on
stderr, not in the report's fields. At the default verbosity the
pipeline logs `provider attempt N failed transiently: rate limited
(retry_after = …); retrying in …` for each transient retry it absorbs,
and the validation report's `provider_retries` counts them. Once a
batch's `max_per_batch_provider_retries` budget is spent the next
transient error is terminal: the run exits with `transync: translation
failed: translator error: rate limited …` and writes no outputs at all.
Drop concurrency rather than raising that budget — the budget is
charged per input batch, so raising it just multiplies the requests the
provider is refusing.

## Recipe — Stopping a run you no longer want

The most expensive time a long-running host spends is time on a run nobody
is waiting for: a reader navigated away, or the process is draining. Set
`TranslateOptions.cancel` and the run stops issuing requests, drops the
in-flight one, and does not sit out a capped 30 s `Retry-After` backoff —
per batch, per retry.

```rust
let job = shutdown.child_token();   // cancels on the daemon's signal too
let mut opts = TranslateOptions::default();
opts.target_language = "ko".into();
opts.cancel = Some(job.clone());

// elsewhere: job.cancel();
match transync::translate_with_cache(src, &opts, &provider, &cache).await {
    Err(TransyncError::Cancelled) => { /* expected; not a failure */ }
    other => other?,
}
```

Two things decide whether this is cheap or expensive:

- **Use `translate_with_cache` with a cache you own.** A cancelled run
  returns `Err(TransyncError::Cancelled)` and no document — but every unit it
  had already accepted is in that cache, so the next attempt re-dispatches
  only the remainder. `translate` builds a throwaway cache per call, so a run
  cancelled through it discards everything it paid for.
- **Concurrency sets the worst case.** Cancellation stops *queued* batches
  immediately; the ones already in flight are dropped mid-request. So the
  work a cancellation can waste is bounded by `max_concurrent_batches`, not
  by the document.

Cancellation is not observed during regeneration and rendering (pure CPU, no
I/O), so a run whose provider work already finished returns `Ok`. Full
contract: `docs/architecture/contracts.md` §5b.

## Recipe — Code-heavy documents

Code blocks tokenize denser than prose (more punctuation per
"meaning"). The `target_input_tokens_per_batch` heuristic accounts
for this via tiktoken, but if you see batches running short, it's
because units rarely fit together cleanly:

```rust
let mut opts = TranslateOptions::default();
opts.target_input_tokens_per_batch = 4000; // smaller, fits more reliably
opts.max_units_per_batch = 16;
```

Smaller batches + more of them + higher concurrency often nets out
faster than fewer big batches that hit per-unit retries.

## Diagnosing actual time spent

The per-unit attempt log is the **validation report**, and a default
run never writes it. The alignment map (whatever `--map` names, or
`alignment.json` under `--out-dir`) carries only the
`validation_summary` counts, and `--verbose` adds one tally line on
stderr. Ask for the report explicitly — `--out-dir` always writes it as
`validation-report.json`, and every other run only writes it when
`--validation-report <path>` says where:

```bash
cargo run -p transync-cli -- translate \
  --input doc.md --target-language ko \
  --output doc.ko.md --map align.json \
  --validation-report report.json \
  --verbose
```

That run writes all three files it names — `doc.ko.md`, `align.json`,
`report.json` — and prints the tally
(`total_units=… translated=… preserved=… partial=… fallback=… retried=…`,
which is the alignment map's `validation_summary` on one line) on
stderr.

`report.json` lists every unit and its attempts. The paths below are
into that file; a Rust caller reads the same values off
`TranslationOutput.validation_report` without writing anything, and
there the layer labels are `ValidationLayer` variants
(`PerKindShape`) rather than their JSON spelling
(`"per_kind_shape"`).

| Symptom in `report.json` | Diagnosis |
|---|---|
| Every `per_unit[*].attempts` holds one row, `attempt_number: 1` | Healthy run; latency is provider-bound. |
| Many units hold several rows with `rejected_by: "per_kind_shape"` | Model is mangling tables/lists. Switch to a Structured-Outputs-strict model. |
| Units hold a row with `attempt_number: 0` | A cache hit, not a round-trip. Without `--cache-dir` it can only be a hit *within* this run (a block whose source and context match one already translated), since that process starts with an empty cache; with `--cache-dir` it may also be a hit replayed from a previous run's disk cache. |
| `total_retries` high, `total_fallbacks` low | Validation pass rate is low but recovery works. Can be acceptable; tune the prompt for higher first-pass yield. |
| `total_fallbacks` high | The model genuinely cannot translate. Different prompt or different model. |
| `provider_retries` above zero | Transient transport failures (network, 429) the pipeline absorbed. Each one cost a backoff sleep, so they show up as wall-clock with nothing to blame in the attempt rows. |

`total_retries` counts re-dispatches of a unit's own content only:
batch-envelope faults are `batch_schema_faults`, transport retries are
`provider_retries`, and the `attempt_number: 0` cache row is not a
retry at all.

## Concurrency vs. cache interplay

Batches are built first; within each batch the cache is then
consulted per unit, and cached units are dropped from the dispatch
set (a fully cached batch dispatches nothing). What that is worth
depends entirely on who owns the cache:

- **The CLI owns one only if you ask for it.** `translate()` constructs a
  fresh `InMemoryCache` per call and drops it when the call returns, so a
  `transync translate` run *without* `--cache-dir` starts cold and
  re-translates every unit; its only hits are within the one run — two
  blocks whose source bytes and context agree reuse one entry, and the
  second appears as an `attempt_number: 0` row. **With `--cache-dir` the
  run opens a disk-backed cache** (`DiskCache`, DCR-0028 / ADR-0021) that
  outlives the process, so a re-run of the same document against the same
  configuration can be served entirely from it — which is what
  `--offline` (DCR-0046) depends on.
- **A caller that keeps one hits it.** Hand your own cache to
  `translate_with_cache(source, &opts, &translator, &cache)` — the
  facade exports both the `Cache` trait and `InMemoryCache` — and a
  second pass over an unchanged document does **zero** round-trips.
  The floor is then parse + validate + regen + render: about 0.1 s for
  `samples/demo-complex.md` (63 units) from a release build.
- A partially-edited document only round-trips for the units that
  moved, but "moved" is wider than the edited block. `CacheKey` carries
  every axis that can change a translation — provider fingerprint,
  validation-schema version, the unit's source hash, both language
  labels, profile version and prompt hash, glossary hash, model id,
  block kind, a hash of the unit's context hints, and a digest of the
  instruction its batch assembled — and any difference forces
  re-translation. Because the context hints cover the section path and
  the neighbors' summaries, editing one paragraph also invalidates its
  neighbors; changing the profile, the glossary or the model
  invalidates the whole document.
- `--auto-glossary` costs one extra provider call the **first** time a
  given document, profile, language pair and provider are run together.
  The harvest is then cached in its own document-scoped record — keyed
  on the extraction prompt's bytes, so a changed excerpt, term cap,
  language label or static glossary is a different question and buys a
  fresh call — and a fully warm run replays it and calls nothing at all.
  The replay also removes what used to be this feature's real cost on
  repeat runs: a nondeterministic extractor no longer lowers the hit
  rate of the runs after it, because the runs after it do not ask again.
  With `--cache-dir` that holds across processes; without it, only
  within one process's cache.

## Provider-side ceiling

If you've cranked every transync dial and wall-clock won't drop,
you're at the provider floor. Verify with a baseline:

```bash
# How long does ONE request to your provider take?
time curl -s https://api.openai.com/v1/chat/completions \
  -H "Authorization: Bearer $OPENAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"gpt-5-chat-latest","messages":[{"role":"user","content":"hi"}]}' \
  > /dev/null
```

If a trivial request takes 3 s, transync cannot be faster than
`3 s × ceil(units / batch_size / concurrency)`. The bottleneck is
upstream.

## When to use the test-stub provider

For scripted runs — this is how `scripts/smoke.sh` and
`scripts/test-browser.sh` produce their bundles — or for perf-shape
testing without burning API budget:

```bash
cargo run -p transync-cli --features test-stub-provider -- translate \
  --input doc.md --target-language ko \
  --output doc.ko.md --map align.json
```

The stub returns content unchanged with zero network latency. This
isolates parse + batch + validate + regen + render time and lets you
benchmark transync's structural overhead. Build with `--release`
before reading the numbers: a 1000-block synthetic document measured
~0.17 s end-to-end from a release binary and ~1.5 s from the debug
binary `cargo run` builds — that order of magnitude is the build
profile, not the document.

## Memory

None of the dials above move memory. The pipeline is whole-document by
design (ADR-0016): one Comrak AST over the entire source, exact source
byte ranges spliced during regeneration, and a reparse of the *complete*
regenerated document in validation layer 5. Peak memory therefore tracks
document size, and the `--max-input-bytes` admission cap is boundary
hygiene — it rejects absurd inputs before parse, it does not lower the
peak for anything it admits.

What that costs, measured coarsely on 2026-08-07:

| Document | Source bytes | Units | Peak RSS |
|---|---|---|---|
| `samples/error-3.md` | 543 | 4 | 62 MiB |
| `samples/demo-complex.md` | 13,844 | 63 | 63 MiB |
| synthetic, 20 sections | 11,854 | 121 | 63 MiB |
| synthetic, 200 sections | 121,275 | 1,201 | 67 MiB |
| synthetic, 2,000 sections | 1,244,276 | 12,001 | 131–134 MiB |
| synthetic, 8,000 sections | 5,030,276 | 48,001 | 391 MiB |

**Exactly how those were taken.** A release build of the CLI with the
stub provider (`cargo build --release -p transync-cli --features
test-stub-provider`), then, per document:

```bash
/usr/bin/time -l target/release/transync translate \
  --input doc.md --target-language ko \
  --output doc.ko.md --map align.json
```

reading the `maximum resident set size` line, which macOS reports in
bytes. Each document was run twice; where the two runs disagreed the
table shows the range, otherwise the rounded single value. Machine:
macOS 26.6 on arm64, 64 GB, rustc 1.97.1. The synthetic fixtures are N
sections of heading + paragraph + nested list + fenced Rust block +
three-row table, each block carrying its own index so that no two units
share source bytes — without that the in-run cache collapses the
duplicates and the measurement flatters itself. Unit counts are
`validation_summary.total_units` from each run's own alignment map.

**Read that table as six points, not as a curve.** Max-RSS from one
process on one machine with one allocator is a coarse instrument, and
these numbers are worth exactly two observations. First, a run pays a
fixed cost of about 60 MiB that even a 543-byte document pays in full;
`transync --version` alone peaks at 3 MiB, so it is the translate run
that pays it and not process startup — what inside the run pays it is
not something this measurement breaks down. Second, the two largest
points sit well clear of that floor (about 2× it at 1.24 MB of source,
about 6× it at 5 MB), so at those sizes it is the document that moves
the number. Nothing here predicts a size that was not measured; in
particular nothing here says where an input at the 64 MiB
`--max-input-bytes` default would land, or where the true OOM ceiling
is.

The stub-provider figures are also the *floor* of the live case: the
stub holds no provider response buffers and has no round-trip in flight,
so a live run at `max_concurrent_batches = 6` carries that much more on
top. Raising concurrency or batch size raises the in-flight bytes with
it.

## Cross-reference

- Default values: `crates/transync-core/src/lib.rs::TranslateOptions::default`.
- Token estimation: `crates/transync-core/src/batch.rs::estimate_unit_tokens`;
  the once-per-batch reserve is `batch.rs::envelope_input_tokens`.
- Budget resolution (caller value > profile > built-in default):
  `crates/transync-core/src/unit/budget.rs::resolve`.
- Concurrent dispatch: `crates/transync-core/src/pipeline.rs::run_pipeline`
  (uses `futures::stream::buffer_unordered`).
- Cache key construction: `crates/transync-core/src/pipeline.rs::CacheKeyContext`
  — `for_run` composes the run-invariant axes, `key_for` adds the
  per-unit and per-batch ones.
- Shared-cache entry point:
  `crates/transync-core/src/lib.rs::translate_with_cache`.
- Slow-translation symptoms: `docs/Troubleshooting.md` "Translation
  is slow".
- Why memory is whole-document, and what the input cap does and does not
  buy: `docs/decisions/0016-whole-document-in-memory.md`.
