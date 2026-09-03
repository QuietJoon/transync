# Troubleshooting

Symptom-keyed reference. If a CLI run, a library call, or the browser
demo misbehaves, the table at the bottom of `docs/Developer_Guide.md`
is the short list; this doc is the long list with diagnosis steps.

## CLI exits silently with no output

Almost always: the binary failed and was invoked with `cargo run --quiet`,
which suppresses cargo's compile output too.

```bash
# Drop --quiet to see what's happening
cargo run -p transync-cli -- translate ... --target-language ko
```

`translate_cmd::run` prints a `transync: <reason>` line on stderr for
every error path *unless* `--quiet` was passed. Combined with `cargo
--quiet` you can lose both layers of diagnostic at once.

## `model_not_found` from the OpenAI API

```
transync: translation failed: translator error: authentication: {
  "error": { "message": "Project … does not have access to model gpt-…" }
}
```

Your project's tier doesn't have access to the configured model. The run
exits **6** — the code for "the provider refused the run over how it was
configured", which is exactly what this is. So does the neighboring case
where the model name simply does not exist (`provider rejected the
request`, HTTP 404). Override per-run:

```bash
TRANSYNC_OPENAI_MODEL=gpt-4o-2024-08-06 ./scripts/test.sh
# or
TRANSYNC_OPENAI_MODEL=gpt-4o-mini ./scripts/test.sh
```

Routing rule: the model name decides which endpoint transync hits.
If your model isn't on Chat Completions but the heuristic sent it
there (or vice versa), force the surface with
`TRANSYNC_OPENAI_API=chat` or `TRANSYNC_OPENAI_API=responses`.

See `docs/architecture/contracts.md` §7 and the "Dual API dispatch"
table in `docs/Developer_Guide.md`.

## `MalformedResponse` / "model output_text was not valid JSON"

The model returned text that didn't conform to the strict
`TranslationBatchResult` schema. Three common causes:

1. **The model doesn't support strict Structured Outputs.** Older
   snapshots (e.g. very early `gpt-4` releases) only honor
   `response_format: { type: "json_object" }`, not `json_schema` with
   `strict: true`. Switch to a Structured-Outputs-capable model
   (`gpt-4o-2024-08-06` and later, `gpt-5*`).
2. **The model wrote a refusal instead of the JSON.** A refusal the
   provider delivers in its *own* refusal field — Chat
   `message.refusal`, a Responses `refusal` content segment — is
   reported as `model refused: model refused to translate: …` and never
   reaches this error. What lands here is a refusal written as
   ordinary prose in the content field, which is not valid JSON for
   the schema. Reword the system prompt (`--system-prompt-file`) to be
   less ambiguous about the task.
3. **A 3rd-party proxy stripped the response_format.** Some
   OpenAI-compatible proxies (Azure, OpenRouter older endpoints) drop
   the `response_format` field. Test against `https://api.openai.com`
   first to isolate.

What is **not** on this list: a response the provider's output ceiling
cut off. That used to arrive here as invalid JSON on Chat Completions;
both surfaces name it themselves now — see "`incomplete (reason:
max_output_tokens)` / `output incomplete (finish_reason: length)`".

A `MalformedResponse` is a translator error, so it aborts the run and
writes no `out.json`; the stderr line is the whole diagnosis. The exit
code is **5**, the residual, and deliberately not one of the two the
0.4.0 window added: the provider neither refused the run over its
configuration (`6`) nor refused this document's content (`7`) — it
answered, badly. An `out.json` whose blocks are all `fallback_status:
fallback_source` is a *different* failure — the run reached validation
— and "Translation succeeds but every block fell back to source" covers
it.

## `incomplete (reason: max_output_tokens)` / `output incomplete (finish_reason: length)`

The run aborted with **exit code 6** and one of these on stderr — the
first from the Responses API, the second from Chat Completions:

```
transync: translation failed: translator error: output ceiling exhausted:
Responses output incomplete (reason: max_output_tokens)
```

```
transync: translation failed: translator error: output ceiling exhausted:
Chat-Completions output incomplete (finish_reason: length): the
max_completion_tokens output ceiling was exhausted before the answer
was complete — on a translation batch, raise
[batching].target_output_tokens (CLI --target-output-tokens)
```

Both say the same thing on the two surfaces: the provider stopped
generating because the output ceiling *in the request* was reached
before the answer was finished. That ceiling is
`[batching].target_output_tokens`, sent as `max_output_tokens` on the
Responses API and `max_completion_tokens` on Chat Completions, and it
was too small for what the batch asked the model to write. Nothing
half-translated reached your files: the run aborts before publishing
anything, so a previous bundle in the output directory is untouched.

Exit `6` is the whole diagnosis in one number: *the run cannot succeed
until the configuration changes*. A wrapper script can act on it without
reading the sentence above — and must, because `6` is also what a
rejected credential, a `--model` the provider does not have, and a
request too large for the context window return. Different knobs, one
action.

**It is terminal on purpose, and re-running changes nothing.** The
ceiling travels in the request, so the verbatim resubmission the retry
policy prescribes (ADR-0009) would be cut off at the same token. Both
surfaces therefore classify it as a non-transient translator error,
which aborts the run (ADR-0017) instead of spending the transport retry
budget on an identical truncation. A second run with the same flags
reproduces it exactly. A library consumer sees the same verdict as a
type rather than as a sentence: `TranslatorError::OutputCeilingExhausted`,
stable code `provider_output_ceiling_exhausted`.

Fix it by making the ceiling fit the answer, or the answer fit the
ceiling:

1. **Raise the ceiling.** `--target-output-tokens <n>` overlays
   `[batching].target_output_tokens` for one run (flag beats profile
   beats built-in default); set the profile key to make it stick.
   `0` is the documented "no ceiling" sentinel — it omits the parameter
   from the request and turns output-aware packing and the preflight
   below off with it.
2. **Raise `--output-expansion-factor` — do not lower it.** The factor
   is how much larger than its source the batcher assumes a translation
   will be (default `2.0`, sized for the KO/JA worst case). It never
   changes the ceiling that is sent; it decides how full a batch is
   packed *against* that ceiling. Too low a factor packs a batch the
   ceiling cannot hold, which is exactly this failure — so if you set
   it down to cut request count (`1.2` is the documented Latin-target
   tuning), that is the first thing to undo.
3. **Split the oversized block in the source.** Splitting the
   *document* into several files buys nothing that batching did not
   already do — packing is per batch, not per document — but splitting
   a single huge block does: a 200-row table into two tables, a
   wall-of-text paragraph into several.

**The one case raising the ceiling cannot fix.** If a *single block*
translates to more tokens than any ceiling the model will honor, no
value of `--target-output-tokens` helps: output-aware packing cannot
split a unit, and a model caps its own maximum output whatever the
request asks for. ADR-0017 records this as the *boundary document* and
accepts it. One kind is exempt since DCR-0026 (STUB-017, closed
2026-08-09): an oversize **table** with at least two body rows is split
at packing time into header-carrying row windows and merged back into
one block afterwards, whenever the effective
`[constraints].default_table_strategy` is `"row-window-first"` — the
shipped default profile's value, and `--table-strategy` overrides it for
one run. Every other kind — paragraph, code block, list item,
blockquote, html — keeps the behavior above, each excluded by a recorded
per-kind decision in DCR-0026 §7, and for those remedy 3 is the only
remedy: the fix is editorial, in the source.

**The prevention half: the preflight.** Before dispatching anything the
pipeline estimates each unit's response size and warns about every one
that already exceeds the ceiling:

```
WARN transync::pipeline: unit t-0037 estimated output ~4200 tokens
exceeds the per-batch output ceiling of 2000 (profile [batching]
target_output_tokens); the provider may truncate the response and abort
the run
```

That line is visible at the default verbosity and silenced by
`--quiet`. On a run that succeeds anyway it also lands in the
`--validation-report` file as `output_budget_warnings`. And if the
batch carrying a flagged unit is the one that aborts, the same
diagnosis is appended to the terminal error above, after `; preflight:`
— so the abort names the culprit block and its estimate, not just the
ceiling. The estimate is a model of the response, not a measurement, so
a run can abort with no warning at all (the estimate was under) or warn
and then succeed (it was over).

**A warm cache survives the fix.** `target_output_tokens` is not a
cache-key axis, so raising it does not invalidate anything, and a
truncated unit is rejected before it could be written — every entry the
aborted run left behind is a complete, validated translation. Re-run
without clearing the cache. That is only worth anything if the cache
outlived the aborted run, which without `--cache-dir` it does not: each
invocation builds its own in-memory cache and drops it on exit. Pass
`--cache-dir <path>` on both runs and the retry re-dispatches only the
batch that aborted. See `docs/architecture/contracts.md` §7
*Response limits and the output ceiling* for the contract behind all of
the above, and §7 *Cache reuse across ceilings* for the cache argument.

## `content filtered` / `model refused` / exit code 7

```
transync: translation failed: translator error: content filtered:
Responses run ended with an incomplete content_filter reason
```

```
transync: translation failed: translator error: model refused:
model refused to translate: I can't help with that.
```

The provider would not translate **this document**. The two lines are
different actors saying so: `content filtered` is the provider's own
content policy ending generation, and `model refused` is the model
declining and writing a refusal into its refusal channel. Both abort
the run before anything is published (ADR-0017), and both exit **7**.

**Re-running changes nothing, and neither does changing a flag.**
Transync's retry is a *verbatim* resubmission (ADR-0009) — same payload,
same scope — so the identical content that was refused is exactly what a
retry would send. That is why the exit code is its own number rather
than sharing `6` with the configuration faults: `6` means "fix something
and run me again", and `7` means "there is nothing to fix here". A batch
script that treats them the same either loops forever on `7` or gives up
on a `6` that one environment variable would have cleared.

What to do about it, in order:

1. **Confirm which one it is.** A library caller has this as a type
   rather than a sentence: `TranslatorError::ContentFiltered` versus
   `TranslatorError::ModelRefused`, stable codes
   `provider_content_filtered` and `provider_model_refused`
   (`contracts.md` §1).
2. **For a refusal, look at the prompt before the document.** A system
   prompt that reads as an instruction to *act on* the content rather
   than translate it draws refusals on material that is otherwise
   unremarkable. `--system-prompt-file` is the knob; a refusal that
   survives a plainly-worded translation instruction is about the
   content.
3. **For a content-policy stop, the document is the subject.** No
   request parameter moves it — that is the difference between
   `ContentFiltered` and everything under exit `6`. Either translate the
   document with a provider whose policy admits it, or skip it.
4. **Do not confuse it with a refusal written as prose.** A model that
   declines in the *content* field instead of the refusal field produces
   invalid JSON for the strict schema and lands on `MalformedResponse`
   (exit 5) — see "`MalformedResponse` / \"model output_text was not
   valid JSON\"" above.

## Translation succeeds but every block fell back to source

Exit code 3 (`AllUnitsFellBack`) and stderr says:

```
transync: every translatable unit fell back to source (N/N)
```

The pipeline ran, but every per-unit validation rejected. `out.json`
carries only the `validation_summary` counts; the per-unit detail is
`TranslationOutput.validation_report.per_unit[*]` (Rust API). From the
CLI, re-run with `--validation-report report.json` to write that
report to a separate file — each `per_unit[*]` entry carries
`attempts[*].rejected_by` and `attempts[*].rejection_reason` so you can
see which validation layer fired.

Common patterns. These are the values as they appear **in the JSON
file**, which is what you grep — a Rust caller reading
`TranslationOutput.validation_report` matches `ValidationLayer`
variants (`PerKindShape`) instead, and the file never contains that
spelling:

| `rejected_by`         | Likely cause                                                          |
|-----------------------|-----------------------------------------------------------------------|
| `"schema"`            | Model returned the wrong unit-id set (most often a refusal).           |
| `"per_kind_shape"`    | Model changed table column count, list depth, code-fence info, or heading level. |
| `"fragment_reparse"`  | Model emitted text that doesn't reparse as the same kind (rare on Structured-Outputs models). |

The other values the field can hold are `"inline"` (a link/image
destination or a policy-gated code span changed) and `"provider"` (the
provider opted the unit out); `"id_set"` and `"full_reparse"` are
reserved and no attempt carries them today. `null` means the attempt
was accepted — and a unit can reach `final_status: "fallback_source"`
with `rejected_by` null on every attempt, which is the html-splice
failure path: read that entry's own `warnings` array to see it
(`docs/architecture/contracts.md` §3a).

For a noisy run, drop `--quiet`, add `--verbose`, and re-run; you'll
get a one-line validation tally on success.

## Sync freezes after the first scroll

The browser demo only re-syncs when the active block changes is the
**old** behavior. The smooth-scroll engine in v0.1.0+ continuously
follows. If you see freezing, you're looking at:

1. **A stale build.** Rebuild and restart the server:
   ```bash
   pkill -f "transync serve"
   ./scripts/test.sh
   ```
2. **Browser cache.** Hard-reload (Shift-Reload / ⌘-Shift-R) — the
   browser may have cached the old `sync.js`. `transync serve` sends
   `Cache-Control: no-store`, so this is only a suspect when something
   else served the bundle.
3. **A schema-version mismatch.** DevTools console will print
   `transync: rejecting alignment map with unknown major
   schema_version=…`. Regenerate the bundle with the matching
   transync version.

## "byte index N is not a char boundary" panic

Pre-`e4182f6` panic when the translation widens the document
(multi-byte CJK). Fixed in v0.1.0.

`e4182f6` predates the 2026-08-17 history restart
(`docs/project/git-history-loss-2026-08-17.md`), so it cannot appear in
this clone's `git log` at all — the check this section used to give could
never succeed, on any current checkout. Every revision reachable from the
2026-08-17 root `59ce8df` already carries the fix, so if you see this
panic on a current checkout it is a **new** defect: file a ticket rather
than pulling.

## `block nesting too deep` / exit code 2 on a small file

```
transync: translation failed: parse error: block nesting too deep: a
pre-scan bounds this source at 20000 nested block containers, over the
128-level maximum
```

`parser::parse` refuses a document whose block-container nesting exceeds
`parser::MAX_BLOCK_NESTING_DEPTH` (128) — one level per blockquote,
one per list level — *before* comrak parses it. This is deliberate and
size-independent: a container costs one byte per level, so a single
20 KB line of `>>>>…` is twenty thousand nested blockquotes, and a
recursive walk over a tree that deep **aborts the process** rather than
returning an error. Refusing is the only outcome a host can handle.

The number in the message is the pre-scan's upper bound, not a measured
depth — the document was never built. Find what produced it:

```bash
# deepest run of blockquote markers at the start of a line
grep -n '^>\{20,\}' input.md | head

# lines indented past ~256 columns AND carrying a list marker
grep -nE '^ {256,}[-*+] ' input.md | head
```

A line carrying **neither** a blockquote marker nor a list marker can
never raise the bound, so wide code blocks, pretty-printed HTML, and
spreadsheets pasted into a fence are not the cause however far they are
indented. If a hand-written document trips this, it is almost certainly
generated or corrupted — flatten the nesting or split the file. There is
no flag to raise the limit; raising it would trade a refusal for an
abort.

**Inline nesting never causes this.** The limit counts *block
containers* only. Nested emphasis, long runs of `*` or `_` (an ASCII
rule, a `/****/` banner in a code fence), deep brackets, and images with
nested alt text are all unbounded and none of them can raise the score —
which is deliberate, not an oversight. Every walk over inline nesting in
the library runs on the heap rather than the call stack, measured to an
inline tree over two million levels deep on a stack a quarter the size of
the smallest one the library ever runs on, and a regression pin in
`parser::depth` fails if that ever stops being true (ticket `f69e83`).

## Translation is slow (~30 s on a small document)

Almost always: sequential dispatch + tiny batches. v0.1.0+ uses
token-budget batching with default `target_input_tokens_per_batch=6000`
and `max_concurrent_batches=6`. On a fresh checkout this should give
~5 s on `samples/demo-complex.md`.

If it's slow on a recent build:

1. **Verify concurrency is enabled.** Re-run with `--validation-report
   report.json` and check its `per_unit[*].attempts[0].attempt_number`
   — every unit should have `attempt_number=1` (one round-trip per
   unit). Multiple attempts means the model is failing validation and
   retrying.
2. **Check the model.** Some `gpt-5*` models have higher per-call
   latency. `gpt-4o-mini` is markedly faster for small batches.
3. **Network.** TLS handshake to `api.openai.com` over a slow link
   adds 2-5 s per round-trip. Test with `curl
   https://api.openai.com/v1/models` to baseline.
4. **Drop concurrency.** Re-run with `--max-concurrent-batches 1` to
   confirm parallelism is helping; the flag sets
   `TranslateOptions.max_concurrent_batches` directly, so library
   callers set that field instead. If wall-clock is identical at
   concurrency=1 vs. 6, you're rate-limited at the provider, not at
   transync.

See `docs/Performance.md` for the tuning model.

## Cache hits report `attempt_number = 0`

That's intentional. Cache hits don't count as a network attempt; in
the `--validation-report` file they appear in `per_unit[*].attempts`
with `attempt_number = 0`. An **accepted** hit is not a rejection, so
the rest of that row is empty: `"rejected_by": null` and
`"rejection_reason": null`. There is no `"cache hit"` sentinel — an
earlier version wrote that string into `rejection_reason`, and it was
removed precisely because a non-null reason reads as a failure to every
consumer. Detect hits by `attempt_number == 0`, never by the reason
string.

A cached unit is re-validated before it is used, so a hit can also be
**rejected**. That row still has `attempt_number = 0`, but it carries
the real `rejected_by` layer and `rejection_reason` validation produced;
the stale entry is evicted and the unit is dispatched fresh. So a
zero-numbered attempt with a reason is a genuine validation failure, not
a mislabelled hit.

A run served entirely from accepted hits has zero `total_retries` and
`total_fallbacks` — the zero-numbered row charges neither budget.

## "OPENAI_API_KEY not set" expect-panic

```
OPENAI_API_KEY not set; set it, pass --offline to run from cache alone, or build --features test-stub-provider
```

You ran `cargo run -p transync-cli -- translate …` without an API
key and without the `test-stub-provider` feature. Either:

```bash
# Live mode
export OPENAI_API_KEY=sk-...
cargo run -p transync-cli -- translate ...

# Stub mode (no key, no network)
cargo run -p transync-cli --features test-stub-provider -- translate ...
```

## `cargo build` fails with "is not supported by the following package"

Your toolchain is older than the floor the package declares. Update with
`rustup update stable`. There are two floors, and cargo's message names
whichever one you tripped:

- The workspace pins `rust-version = "1.88"`. Edition 2024 by itself
  would only need 1.85, but the library members use `if let … && …`
  let-chains, stable since 1.88 — a 1.87 toolchain rejects them
  outright.
- `transync-cli` pins **1.89** on top of that, for the std file-lock API
  publication uses (DCR-0021). It is the only member that does, so
  `cargo build --workspace` needs 1.89 while depending on the library
  crates needs 1.88.

## `.transync-out-dir` / `.transync-publish.lock` / `*.tmp.<pid>` files in an output directory

All three are transync's own **as regular files**, and none of them breaks
a republish — every guard recognizes them, so you do not need `--force`
because of them. (A *directory* or a symlink wearing one of the two marker
names is not transync's and is not skipped: those are the two names the
guards accept without looking inside, so a directory there would carry
whatever it held into the replace. Transync refuses such a target until
`--force`.) That
now includes a `*.tmp.<pid>` at the **top level of an `--out-dir` target**,
left there by a crashed `--output` / `--map` run that wrote into that same
directory: it used to be the one place the allow-list had no room for, so
the target read as foreign and the publish refused with exit 4 (ti
`66339b`). It no longer does, and the leftover goes with the tree the
publish replaces.

- `.transync-out-dir` is the **ownership marker** a `--out-dir`
  publication writes into the tree it publishes. It is what tells the next
  run that transync produced this directory, rather than guessing from the
  file names in it. An **empty** target is never asked the question, so
  `mkdir out && transync translate --out-dir out` needs nothing. Two
  things follow for a target that holds output. **Deleting it** makes the next
  `--out-dir` publish refuse (exit 4) unless the complete published set —
  `out.md`, `alignment.json`, `validation-report.json` and `html/` — is
  still there, in which case the set itself is evidence enough; recreate
  the file (any content) or pass `--force` once, and the publish writes a
  fresh marker. **Copying a bundle with a glob** (`cp <dir>/* elsewhere/`)
  silently drops it along with every other dot-file, so prefer `cp -r
  <dir> elsewhere/`; the copy is still republishable while its set is
  complete.
- `.transync-publish.lock` is a **zero-byte publication lock marker**,
  one per directory transync publishes into. It is created once and
  never deleted by design: a run that unlinked it on release would let
  the next run lock a fresh inode at the same path while a waiting run
  still held the old one. (A run that *is* handed such a stale marker
  notices and re-locks the current one, so deleting this file while a
  run waits on it costs that run a retry, not correctness.) Ignore it
  the way you ignore `.DS_Store`; it does not need to be copied when you
  publish a bundle to a web server. Deleting it by hand is safe only
  when no transync run is active, and it will come back on the next run.
- `*.tmp.<pid>` is a **staging temp**. A run sweeps up leftovers
  carrying its *own* pid; one carrying a different pid is deliberately
  preserved, because it may belong to a live run whose staging deleting
  it would corrupt. So leftovers from crashed runs accumulate, and the
  run tells you when it finds them. Clear them yourself, with no
  transync run active: `rm <dir>/*.tmp.*`.

## `transync: note: waiting for another transync run to finish publishing into …`

Another `transync` process is publishing into one of your output
directories, and this run is queued behind it rather than interleaving
its renames (which is how a bundle ends up mixing two translations).
Publication is a handful of renames, so the wait is short. If it does
not end, the other process is stuck — the lock is released the moment
that process exits, `SIGKILL` included, so ending it unblocks this run.

## `cargo fmt --all -- --check` blocks the commit

The pre-commit hook enforces `cargo fmt`. Run `cargo fmt --all` and
re-stage. Formatting is one of four Rust gates in the tracked
`scripts/hooks/pre-commit`, and any of them failing blocks the commit:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo check -p transync-syntax -p transync-wasm \
  --target wasm32-unknown-unknown
. scripts/lib/rustdoc-gate.sh   # the gate's crate list has one home
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps "${RUSTDOC_GATE_ARGS[@]}"
```

The hook echoes each command as a `[pre-commit] …` line before running
it, so the last such line before the failure names the gate to re-run by
hand — run that one from the repo root for the unabridged error, fix it,
and commit again. The whole hook runs outside a commit too:
`bash scripts/hooks/pre-commit`.

**Never bypass it with `git commit --no-verify`.** The hook is the only
gate that runs by itself: this repository has no CI — no workflow file,
and none anywhere in its history — so a skipped hook is not a deferred
failure, it is an unchecked commit that nobody notices until a human
runs `scripts/smoke.sh`. `docs/project/release-checklist.md` states the
same rule for the release-prep commit (steps 7 and 24). Fix the failing
gate instead; a gate that is wrong rather than you is worth a ticket,
not a bypass.

## `.ko.md` files keep appearing in `git status`

The repo has external translation tooling that materializes Korean
sidecar files for every `.md`. The tracked `.gitignore` carries a
`*.ko.md` rule, so they shouldn't show up as untracked. If they do, your
`.gitignore` is stale:

```bash
git check-ignore -v scripts/test.sh.ko.md  # should print a hit
```

If not, sync `master` or add `*.ko.md` to your local `.gitignore`.

## `fatal: unable to read tree` / `git fsck` reports a missing object

An object vanished from `.git/objects`. It has happened three times here —
2026-08-07 (one tree, repaired), 2026-08-10 (30 objects, history restarted:
`docs/project/git-history-loss-2026-08-10.md`) and 2026-08-17 (33 objects,
history restarted again: `docs/project/git-history-loss-2026-08-17.md`, which
carries the full recovery runbook). The worked example below is the first one,
because it is the only one that ended in a repair rather than a restart: the
`docs/` subtree of commit `494bc9c` was gone minutes after the commit was
written, so every `git show`, `git diff` and `git log -p` crossing that commit
died with

```
fatal: unable to read tree (c8daa7a8aa53cbaed7e805c6db86023a8e052e70)
```

Establish what is actually missing before touching anything:

```bash
git fsck --no-progress --connectivity-only
```

`dangling` lines are noise — every clone has some. The fault lines are
`missing <type> <sha>` and `broken link from <sha> to <sha>`; the second
one names the parent that still points at the hole, which is how you find
out *what* was lost.

**Cause — confirmed 2026-08-17.** A file-sync client (Insync/Google Drive,
here) re-materializing files under `.git/` while git is writing them — this
worktree lives on a synced external volume. For the first two events this was
an inference; on 2026-08-17 the client's conflict copies were found *inside*
`.git`: 23 duplicated object **fanout directories** named `0a (2)`, `6d (2)`
and so on, three duplicated object files, and a duplicated ref file
`refs/heads/master (2)` holding a null sha1 — 27 collision paths inside `.git`
against **0** in the working tree.

The duplicated *directory* is the form that hides: the object files inside it
carry ordinary 38-hex names, so a `find .git -name '* (*'` sweep walks past
them, and git never opens the directory either, because a fanout name that is
not exactly two hex characters is not part of the loose-object layout. Search
for the directories, not only the files:

```bash
find .git -name '* (*'                                     # files AND directories
find .git/objects -type d -name '* (*' -exec find {} -type f \;
```

Two older tells still apply: a pack whose `.pack` mtime is newer than its own
`.idx` (git never rewrites a finished pack), and the fact that nothing in this
repository prunes objects and no `gc` had run.

**Repair, when the missing object is a tree.** Recovery is purely
additive: never `reset`, `gc`, `prune` or rewrite history to make the
error go away — that converts a recoverable hole into a lost commit.
A tree is derivable when a neighbouring commit still carries the same
entries, and the object's own hash is the proof you rebuilt the right
thing:

```bash
git ls-tree <neighbour-commit>:<path>   # the entries, in mktree's input format
git mktree                              # feed them in; it prints the hash it wrote
```

Rebuild the deepest missing tree first, then each parent tree with the
child's new hash substituted in. If the hash `git mktree` prints equals
the hash git said was missing, the object is the original byte for byte —
nothing else can produce that hash. The 2026-08-07 repair rebuilt three
nested trees this way, after which `git fsck` was clean and `git show
--stat 494bc9c` worked again.

A missing **blob** has no equivalent trick — its content is not derivable
from anything else in the store — which is why detection is worth more
here than recovery technique.

**Mitigations in place.**

- `git config --local gc.auto 0` on this worktree. Two agent sessions
  commit into it concurrently, and an automatic gc firing inside one
  session's commit is a second, independent way to lose an object. An
  explicit `git gc` at a quiet point is unaffected, and is what the
  setting assumes you will run instead.
- Step 0 of `docs/project/release-checklist.md` runs `git fsck
  --no-progress --connectivity-only` as release preflight, so a loss
  surfaces the day it happens rather than at the next clone — while the
  neighbouring objects that make a tree rebuildable are still there.
- **The cause is excluded from the sync client, since 2026-08-17.** `.git` is
  now in the client's ignore list (`/Volumes/Common/GDrive/InsyncIgnore.txt`),
  which had previously excluded only rebuildable directories like `target` and
  `node_modules`. This **reverses** the earlier posture — excluding the
  repository had been weighed and declined under ticket `7a7feb`, leaving
  detection as the standing answer, and two restarts followed.

  **The exclusion did not work.** On 2026-08-19 the loss recurred with the line
  still in the file and the client still running: 13 objects missing,
  `git ls-tree -r HEAD` aborting at 151 of 342 paths. It also left **no
  collision copies** — the census read 0 while objects were vanishing — so a
  clean census is not evidence of anything. `git fsck` is the check.

  What repaired it was **redundancy**: every one of the 13 objects was found in
  the salvaged store at `/Volumes/Common/git-backup/transync-broken-git-20260817`
  and re-imported hash-verified.

  **There is now a mirror to push to.** The remote is named **`backup`**, not
  `origin` — deliberately, because it is a mirror of record and not an upstream
  anyone develops against. It is `/Volumes/Common/git-backup/transync.git`, a
  bare clone created 2026-08-19, and `master` tracks `backup/master`. Push
  after every commit (`git push backup master`) — it is the cheapest thing
  on this page and the only one that has ever turned a loss into a repair. Its
  own integrity is checkable the same way: `git --git-dir=/Volumes/Common/git-backup/transync.git fsck --full`.

  Be honest about its limit: it lives on the same synced volume, so it can be
  damaged by the same cause. It is redundancy, not immunity — a copy on a
  volume the sync client does not touch would be strictly better. And treat any
  sync-client setting as unproven until an `fsck` after a stretch of normal
  work says otherwise; the 2026-08-19 recurrence is what that rule is made of.

The commit hashes in this section — `494bc9c` above among them — name commits
that are **not in this object store**: the 2026-08-10 and 2026-08-17 restarts
replaced the graph twice. They are kept because the story is what makes the
repair legible, not because they can be checked out. See the two records named
at the top of this section for the archived stores that may still hold them.

## Glossary entries don't appear to take effect

Glossary entries are baked into the system prompt as a bullet list
appended to the prompt body. They're guidance, not enforcement —
a model can still ignore them. They also only exist if you supplied
them: the embedded default profile ships **zero** active entries (its
`[[glossary]]` examples are commented out, because one default serves
every target language and a target form does not), so a run without
`--profile` has no glossary section at all. To verify the pipeline is
*sending* yours:

```rust
use transync::profile::{load_profile, render_prompt_body};
let toml = std::fs::read_to_string("profile.toml")?;
let p = render_prompt_body(&load_profile(&toml)?, "en", "ko");
println!("{}", p.prompt_body);
// Should end with "Glossary (when the source term appears, …):"
// followed by one bullet per entry.
```

If the bullets are missing, check that the TOML's `[[glossary]]` keys
match the documented `source` / `target` / `note` / `scope` schema
(`source` and `target` required; `note` and `scope` optional). Wrong
key names don't fail silently: an unknown or misspelled *optional* key
is ignored with a warning surfaced on `ProfileMetadata.load_warnings`,
via `tracing::warn`, and on the CLI's stderr — so check that output. A
misspelled *required* key (`source`/`target`) is worse: the entry
can't deserialize and the whole profile load fails with a hard TOML
error. (See `docs/architecture/contracts.md` §2.)

If exactly *one* entry is missing, the loader dropped it and said so on
the same warning channel. Read the warning: it names the entry by index
(`glossary[2] …`). An entry is dropped when `source` or `target` is
empty or whitespace-only, or when an earlier entry already claimed the
same source term — terms are compared trimmed, NFC-normalized and
case-insensitively, so `agent`, ` Agent ` and `AGENT` are one term (and
so are the composed and decomposed spellings of an accented term) and
the first one wins.
A term containing a newline or other control character is *kept*, but
the character is escaped in the rendered bullet (`\n`, or `\u{XXXX}`),
so it can never open a second line inside the system prompt. That too
is reported, naming the field and the codepoint — but only when the run
actually compiles a prompt, and then exactly once:

```
WARN transync::profile: glossary[0].note contains the control character U+000A;
it is escaped in the rendered prompt, so it cannot open a line of its own in the
glossary bullet list
```

Because the entry is kept, that line is an advisory rather than a
report of something the run changed, so it is raised at the single
point where the glossary becomes prompt text. A `--profile` whose
entries are never batched (an empty input document) will not print it,
though `ProfileMetadata.load_warnings` still records it.

## A `{{placeholder}}` reaches the model literally

Only `{{source_language}}` and `{{target_language}}` are substituted.
Anything else unnamespaced — `{{target_lang}}`, `{{lang}}` — is sent
verbatim, and the model reads the braces as text. The run says so:

```
WARN transync::profile: unknown template variable `{{target_lang}}` in system
prompt is not substituted and will be sent to the model literally
```

That line appears whichever way the prompt body arrived — a `--profile`
TOML, `--system-prompt`, or `--system-prompt-file` — so if you don't see
it, the placeholder is spelled right. It describes the body *this run*
sends and nothing else: if a `--system-prompt` / `--system-prompt-file`
override replaces a profile whose template had the typo, the warning goes
away with the typo. It is a warning, not an error: the
run continues. `--quiet` silences it along with everything else. Names
carrying a dot (`{{ctx.section_path}}`) are reserved namespaces and are
deliberately left alone, so they neither substitute nor warn.

## The source pane says "auto" instead of a language

`--source-language auto` is a sentinel, not a label: it asks the model to
detect the language, and the detected value comes back on the alignment
map's `detected_source_language`. Two things follow. First, the compiled
prompt reads "Translate from the auto-detected source language to …",
never the word `auto`. Second, the HTML bundle's source pane is stamped
with the *detected* label, or with nothing at all when the provider
volunteered no detection — so an empty `lang` on the source pane means
the model never told us.

Since v0.4.0 a **fully cached re-run keeps the detection** rather than
losing it. Before, a second run over an unchanged document served every
unit from cache, made no provider call, and therefore had no envelope to
report a language from, so `detected_source_language` came back `null` on
a run that had already paid for the answer. The `Cache` now holds one
document-level record beside the per-unit entries, written from the live
run's envelope and replayed only by a run that dispatched *nothing* — so
a run that really did call the provider still reports what the provider
said, including its silence. With `--cache-dir` the record survives the
process, so the re-run can be a different invocation on a different day.
A blank `lang` after a cached re-run therefore means the first run's
provider volunteered no detection either.

Neither whitespace nor case defeats the sentinel: `--source-language
' auto '` and `--source-language AUTO` are both normalized to `auto` at
the argument boundary and behave exactly like `--source-language auto`,
down to the cache key and the `"source_language"` the alignment map
records. `auto` is the only value that gets that treatment, because it is
the only reserved one — every other label is left alone, interior text
and case included, so `--target-language 'Korean (formal, 존댓말)'` and
`--source-language DE` reach the model and the alignment map
byte-for-byte (ADR-0013). Nothing is reserved on the *target* side:
`--target-language AUTO` is just a label spelled `AUTO`.

## When all else fails

1. `cargo test --workspace` — confirms the library is healthy.
2. `cargo run -p transync-cli --features test-stub-provider --
   translate …` — confirms the pipeline shape works end-to-end with
   no network.
3. If 1 + 2 pass but live mode fails, the issue is provider-side
   (model access, rate limit, account state) — not transync.
