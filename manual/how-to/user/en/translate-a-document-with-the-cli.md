---
type: How-To Guide
title: How to translate a Markdown document with the CLI
description: Run `transync translate` end-to-end to produce a translated document and a browsable dual-pane demo bundle, then open it with `transync serve`.
tags: [cli, translation, serve, caching, SCN-12, SCN-13]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: translate-cmd, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: translate-args, resource: crates/transync-cli/src/translate_cmd/args.rs }
  - { id: translate-publish, resource: crates/transync-cli/src/translate_cmd/publish.rs }
  - { id: translate-report, resource: crates/transync-cli/src/translate_cmd/report.rs }
  - { id: output-rs, resource: crates/transync-cli/src/output.rs }
  - { id: output-lock, resource: crates/transync-cli/src/output/lock.rs }
  - { id: serve-cmd, resource: crates/transync-cli/src/serve_cmd.rs }
  - { id: exit-codes, resource: crates/transync-cli/src/error.rs }
  - { id: cache-disk, resource: crates/transync-core/src/cache/disk.rs }
  - { id: contracts, resource: docs/architecture/contracts.md }
synced_hash: ec6631d09bba1c0b4d6f008b809f614bcc3f7ab5c976d3a6109a0c8ef03e5cec
---

# How to translate a Markdown document with the CLI

Translate a GFM Markdown file with `transync translate` and view the
result as a scroll-synced source/target demo page in a browser.

## Prerequisites

- The `transync` binary available on your `PATH`, or built from a
  checkout with `cargo build --workspace`.
- `OPENAI_API_KEY` exported for the live OpenAI provider (or a
  compatible `--base-url` pointed at a proxy that accepts the same key).
- The path to your source Markdown file and a target-language label
  (e.g. `ko`, `ja-JP`) in hand.

Nothing else. `transync serve` serves the demo bundle, so no separate
static HTTP server is required.

## 1. Pick an output layout

`transync translate` writes its output set in one of two mutually
exclusive layouts — passing `--out-dir` together with `--output`,
`--map`, or `--html-out` is a `clap` argument error (exit code `1`):

- **Discrete paths** — `--output` and `--map` (required together) plus
  an optional `--html-out` directory for the browsable demo bundle.
- **`--out-dir`** — one directory that always gets the translated
  Markdown, the alignment map, the validation report, and the demo
  bundle together.

Use discrete paths when you want the translated Markdown or alignment
map at specific locations (e.g. committed into a docs tree). Use
`--out-dir` when you just want "give me everything, in one place."

## 2. Translate with discrete paths and a demo bundle

```bash
export OPENAI_API_KEY=sk-...

transync translate \
  --input README.md \
  --output README.ko.md \
  --map README.alignment.json \
  --html-out dist/demo \
  --target-language ko
```

- `--output` and `--map` are required together in this form.
- `--html-out dist/demo` is optional but is what produces the
  browsable bundle: `index.html`, `source.html`, `target.html`,
  `alignment.json`, `sync.js`, and `purify.min.js`.
- The bundle's own six files are overwritten in place on a repeat run;
  add `--force` only if `dist/demo` also holds unrelated ("foreign")
  files you want to write alongside. That refusal is checked before the
  provider is called, so a foreign-file abort costs you nothing.
- Add `--validation-report path/to/report.json` if you want the
  per-unit attempt log (rejection reasons, provider warnings) as its
  own file; it has no effect under `--out-dir` (see step 4).

A non-zero exit means something needs attention before you have a
result to open — check the printed `transync: <reason>` line. (Exit
code `3` is the exception: outputs are still written even when every
unit fell back to source — see
[how to diagnose a translation run](./diagnose-a-translation-run.md)
if you land there.)

A run that exits `0` can still print advisory `transync: note: …`
lines — waiting behind another run's publication lock, staging temps
left by other runs, a directory whose flush to disk failed. None of
them changes the exit code or the bytes that were published.

## 3. View the bundle with `transync serve`

```bash
transync serve --rendered dist/demo
```

It binds `127.0.0.1:7470` by default and prints, on stderr:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/dist/demo
transync serve: answering for 127.0.0.1:7470, localhost:7470 — a request naming another authority is refused (--allow-host adds one).
transync serve: press Ctrl-C to stop.
```

Open that URL and confirm the two panes render and stay in sync as you
scroll either one. Ctrl-C stops the server and exits `0`. If the port
is taken, `--port 0` asks the OS for a free one and the `listening on`
line reports the port it got.

A server is genuinely required: the bundle shell fetches
`source.html`, `target.html` and `alignment.json` from beside itself,
which a `file://` URL will not do. For the non-loopback `--bind` case,
checking that the panes actually mounted, and the full status and MIME
behaviour, see
[how to serve the demo bundle](../../operator/en/serve-the-demo-bundle.md).

## 4. Alternative: one-shot with `--out-dir`

```bash
export OPENAI_API_KEY=sk-...

transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko
```

This publishes:

```
dist/out/
├── out.md                   # translated Markdown
├── alignment.json           # alignment map
├── validation-report.json   # always written here, regardless of --validation-report
└── html/                    # the same six-file demo bundle as --html-out
```

Point `serve` at the `html/` subdirectory, not at `dist/out` — the
top level has no `index.html`:

```bash
transync serve --rendered dist/out/html
```

An existing `dist/out` from a prior transync run is republished in
place; if the directory holds anything outside
`{out.md, alignment.json, validation-report.json, html}`, pass
`--force` to replace it.

Two honest limits on that republication:

- **A fresh target appears atomically; replacing an existing one does
  not.** The old tree is renamed aside before the new one is renamed
  into place, so the publication is crash-safe but not atomic: an
  unrelated reader looking between the two renames finds no
  `dist/out` at all. Do not point a live consumer at a directory you
  are republishing into.
- **A `.transync-publish.lock` marker stays behind.** transync locks
  that zero-byte file in every directory it publishes into and never
  unlinks it, deliberately. Leave it; it is not residue from a failure.

## 5. Make the next run cheap

Nothing survives a run by default — each invocation builds a fresh
in-memory cache, so translating the same document twice pays the
provider twice. Add `--cache-dir <dir>` and the run keeps a
`transync-cache.jsonl` log there, so a second run over the same
document under the same profile, model and languages re-dispatches
only what actually changed:

```bash
transync translate \
  --input README.md \
  --out-dir dist/out \
  --target-language ko \
  --cache-dir .transync-cache
```

The recipe for operating that directory — what invalidates entries,
what the size budget does, why only one run may write it at a time —
is [how to reuse a warm cache across runs](./reuse-a-warm-cache-across-runs.md).

## Next steps

- Full flag list, defaults, environment variables, and exit codes:
  [CLI reference](../../../reference/user/en/cli.md).
- A run that exits non-zero, or one whose output looks wrong:
  [how to diagnose a translation run](./diagnose-a-translation-run.md).
- Steering translation style, preserved terms, and structural policy:
  [how to write a Profile TOML](./write-a-translation-profile.md).
