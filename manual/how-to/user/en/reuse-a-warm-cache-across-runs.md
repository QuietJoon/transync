---
type: How-To Guide
title: How to reuse a warm cache across runs
description: Point `transync translate` at a `--cache-dir` so a second run over the same document re-dispatches only what actually changed, and operate that directory safely.
tags: [caching, cli, cost, DCR-0028, ADR-0021, SCN-10]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: translate-cmd, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: cli-logging, resource: crates/transync-cli/src/logging.rs }
  - { id: core-cache, resource: crates/transync-core/src/cache.rs }
  - { id: core-cache-disk, resource: crates/transync-core/src/cache/disk.rs }
  - { id: core-pipeline, resource: crates/transync-core/src/pipeline.rs }
  - { id: core-dispatch, resource: crates/transync-core/src/pipeline/dispatch.rs }
  - { id: quick-start, resource: docs/Quick_Start.md }
  - { id: troubleshooting, resource: docs/Troubleshooting.md }
  - { id: contracts, resource: docs/architecture/contracts.md }
synced_hash: 91ff6f9676b8d1467c143a5ff750de2bb7bc7a3c48d5a6d2af7251df22dead74
---

# How to reuse a warm cache across runs

You already paid a provider to translate a document. Use this when you
want the next run over that document to cost only what actually changed —
after an edit, after a failed run, or after raising an output ceiling and
retrying.

## Prerequisites

- A `transync translate` invocation that already works end to end. If you
  do not have one yet, start at
  [how to translate a Markdown document with the CLI](./translate-a-document-with-the-cli.md).
- A directory you can dedicate to the cache. It does not have to exist
  yet, and nothing else may write to it while a run is using it (see
  step 5).
- Nothing else. No service, no environment variable, no profile setting.

## 1. Add `--cache-dir` to the first run

```bash
transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko \
  --cache-dir .transync-cache
```

`--cache-dir` takes one path to a directory, which transync creates —
along with any missing parents — the first time it opens it. The flag has
no default, no environment-variable fallback, and no profile counterpart:
where a cache lives is a per-invocation decision, so this flag is the whole
control. Without it, each run builds a throwaway in-memory cache and drops
it on exit, which is why an uncached re-run costs full price.

The directory then holds one file, `transync-cache.jsonl` — an append-only
log, one JSON object per line, replayed into memory when a later run opens
it. Every accepted unit is appended and flushed as it is accepted, so a run
you interrupt with Ctrl-C still leaves behind everything it had already
validated.

The flag's row in the argument table lives in
[CLI flags](../../../reference/user/en/cli.md); this page is the recipe,
not the listing.

## 2. Re-run against the same directory

Edit the document (or change nothing at all) and run the identical command
again, `--cache-dir` included:

```bash
transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko \
  --cache-dir .transync-cache
```

Units whose identity is unchanged are served out of the log and never
reach the provider. Only the rest is dispatched. Nothing about the output
set changes: the translated Markdown, the alignment map, the validation
report and the demo bundle are written exactly as on a cold run.

Two other things a warm directory carries, both worth knowing because they
remove provider calls a warm run used to pay anyway:

- **The detected source language.** A `--source-language auto` run stores
  what the provider reported for the document, so a fully cache-hit re-run
  can still report a language in the alignment map and in the demo bundle's
  `lang` attribute instead of reporting nothing.
- **The auto-glossary harvest**, if you run with `--auto-glossary`. The
  preflight is consulted in the cache before the call is made, so a second
  run replays the first run's harvest rather than buying a fresh — and
  possibly different — one.

## 3. Know what invalidates an entry

An entry is identified by everything that landed in the prompt bytes the
model saw for that one block, plus who answered and under which contract.
Change any of it and that unit is re-dispatched.

| Change this | Effect on the cache |
|---|---|
| The block's own source text | That unit misses, and often its neighbours too — see the blast-radius note below. |
| A heading, or a block's position in the section tree | Every unit whose section path or heading levels moved misses. |
| The profile — its version string, its prompt body, or its glossary | Every unit under it misses. |
| `--model` (or `TRANSYNC_OPENAI_MODEL`) | Everything misses. |
| The provider identity: `--base-url`, the resolved API surface, reasoning effort | Everything misses. |
| `--source-language` or `--target-language` labels | Everything misses. Labels are compared as written, so `ko` and `Korean` are two namespaces. |
| Upgrading transync across a validation-schema bump | Everything misses. |

And the changes that do **not** cost you a re-dispatch, because none of
them is a byte the model read:

| Change this | Effect on the cache |
|---|---|
| `--target-output-tokens` (including raising it after an abort) | Nothing invalidated. |
| `--max-concurrent-batches` | Nothing invalidated. |
| Output paths — `--out-dir`, `--output`, `--map`, `--html-out`, `--validation-report` | Nothing invalidated. |
| `--quiet` / `--verbose` / `--force` | Nothing invalidated. |

`--max-units-per-batch` sits between the two. The packing budget itself is
not part of an entry's identity, but the instruction a batch assembles is,
and re-packing can move a unit into or out of a batch containing raw-HTML
units or oversize-table row windows — which is the one way a batching knob
changes an instruction and therefore a key.

**Expect a blast radius around an edit, not a single unit.** A unit's
identity includes the context hints the model was shown alongside it — the
document title, the section path and heading levels, and the kinds and
summaries of its neighbours. So editing one paragraph can also miss on the
paragraphs beside it, and editing a heading can miss on the units beneath
it. This is correct rather than wasteful: a translation produced under a
different heading path was produced for a different question.

## 4. Confirm the reuse

Pass `--validation-report report.json` (or read the copy `--out-dir`
always writes) and look at `per_unit[*].attempts`. A unit served from the
cache is recorded with `attempt_number: 0` — cache hits are not network
attempts, so a fully warm run shows no attempt above `0` at all.

A cached unit is not trusted blindly: every hit is re-validated through
the same per-unit layers a live response goes through, and a hit that
fails is evicted and re-dispatched. A damaged or stale cache therefore
costs you a re-translation, never a wrong document.

One result that is not a cache failure: a **blank detected language after
a partially warm re-run**. The stored detection is replayed only by a run
that dispatched zero provider batches. A run that made even one provider
call reports what that provider said — including its silence — and does
not consult the store, because replay exists to compensate for calls the
cache elided, not to override a live answer. See
[how to diagnose a translation run](./diagnose-a-translation-run.md) if
the rest of the report also looks wrong.

## 5. Operate the directory

**One writer at a time.** Two runs sharing one cache directory
concurrently are unsupported: there is no lock file, and none is planned.
Use one directory per person and keep it on local disk rather than on a
shared network drive. The cost of breaking this rule is bounded — the
reader drops interleaved or torn lines, and a compaction race loses the
other process's recent appends — so what you lose is entries, meaning a
re-translation, never corrupt output and never a failed run.

**The log has a ceiling, checked when it is opened.** The default budget
is 1 GiB, measured as the size a compacted log would occupy rather than
the current file size. It is enforced at open and never during a run, so a
single run can overshoot and the next one trims. Trimming drops the
**oldest-written** live entries first — this is deliberately not
least-recently-used, because persisting read recency would turn every
lookup into a disk write. A trim announces itself as a `WARN` record on
the `transync::cache` target, which reaches stderr at the default
verbosity and is silenced by `--quiet`:

```
cache log .transync-cache/transync-cache.jsonl exceeded its budget; dropped 12 oldest-written entr(ies), 400 kept
```

**Three filenames can appear, and only one is ever read back.**

| File | What it is |
|---|---|
| `transync-cache.jsonl` | The live log. The only file a run reads. |
| `transync-cache.jsonl.unreadable-<unix-seconds>` | A log whose header was missing or of an unknown format. It is rotated aside rather than deleted, and the cache starts empty. Safe to delete once you no longer want the evidence. |
| `transync-cache.jsonl.compact-<pid>-<nanos>` | A compaction temp file left behind by a crash before the rename. Inert, never read, and never cleaned up by transync. Safe to delete. |

Because nothing reclaims the last two, a cache directory can hold more
bytes than the budget describes; only you delete them.

**An unopenable directory degrades, it does not fail.** If the path cannot
be opened — it is an existing regular file, the parent is read-only, the
volume is gone — the run says so once and continues on a fresh in-memory
cache:

```
transync: could not open --cache-dir <path>: <error>; continuing with a fresh in-memory cache (this run will not reuse or persist anything)
```

That is a warning, not a failure: the exit code is unaffected and the
translation proceeds at full price. `--quiet` suppresses the line, so do
not use `--quiet` on the run where you are checking that caching works.

**Deleting the directory is always safe.** The next run re-translates from
scratch and rebuilds it.

## Limits worth knowing

- **The capacity policy is not reachable from the CLI.** Every CLI run
  gets the default 1 GiB byte budget and no entry cap. There is no flag,
  environment variable, or profile table to change them; the byte and
  entry budgets are library-only settings, available to code that
  constructs the cache itself.
- **A crash during compaction leaves a temp file behind.** Nothing removes
  it. See the file table above.
- **Writes are flushed, not `fsync`ed.** Records survive process exit,
  including a Ctrl-C. An operating-system crash may lose the tail of the
  log — which the next open discards as a torn record, making it a
  re-translation rather than a corruption.
- **A cache directory is portable between machines running the same
  transync build**, because nothing in a key is machine- or run-salted.
  That is a property, not an invitation to share one directory between two
  concurrent runs.

## Related

- The flag row, its defaults, and every other argument:
  [CLI flags](../../../reference/user/en/cli.md).
- What the profile contributes to an entry's identity:
  [how to write a Profile TOML](./write-a-translation-profile.md).
- Reading a run that went wrong:
  [how to diagnose a translation run](./diagnose-a-translation-run.md).
- Where the harvested glossary comes from and which entry wins:
  [how glossary entries are resolved](../../../explanation/user/en/how-glossary-entries-are-resolved.md).
- Why co-batching is bounded by headings, which is what keeps a batch's
  assembled instruction stable across runs:
  [why batches stop at section boundaries](../../../explanation/user/en/why-batches-stop-at-section-boundaries.md).
