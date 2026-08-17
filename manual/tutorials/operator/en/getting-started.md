---
type: Tutorial
title: Getting started with transync
description: Build transync from a clean checkout, run its tests, translate a sample document with no API key, and watch the two rendered panes scroll in sync in a browser.
tags: [getting-started, cli, serve, SCN-12, SCN-13]
audience: operator
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: quick-start, resource: docs/Quick_Start.md }
  - { id: smoke-sh, resource: scripts/smoke.sh }
  - { id: workspace-manifest, resource: Cargo.toml }
  - { id: cli-manifest, resource: crates/transync-cli/Cargo.toml }
  - { id: translate-cmd, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: translate-report, resource: crates/transync-cli/src/translate_cmd/report.rs }
  - { id: translate-provider, resource: crates/transync-cli/src/translate_cmd/provider.rs }
  - { id: serve-cmd, resource: crates/transync-cli/src/serve_cmd.rs }
  - { id: cli-logging, resource: crates/transync-cli/src/logging.rs }
  - { id: cli-output, resource: crates/transync-cli/src/output.rs }
  - { id: cli-smoke-test, resource: crates/transync-cli/tests/cli_smoke.rs }
  - { id: pipeline, resource: crates/transync-core/src/pipeline.rs }
  - { id: default-profile, resource: crates/transync-core/profiles/default.toml }
  - { id: echo-translator, resource: crates/transync-core/src/test_stub.rs }
  - { id: bundle-shell, resource: crates/transync-cli/web/index.html.tpl }
  - { id: sync-js, resource: crates/transync-cli/web/sync.js }
  - { id: fixture, resource: crates/transync/tests/fixtures/scn-14-full.md }
  - { id: browser-harness, resource: web/tests/support/harness.js }
synced_hash: b5cc09afa125e22010227bf69bd5663a223a3f78eb26ee46f10ff645a641ed3f
---

# Getting started with transync

By the end of this tutorial you will have built transync from source, run its
tests, translated a sample document, and watched the two rendered panes —
source and translation — scroll in sync in a browser. All of it runs offline:
no API key, no provider account, no extra tooling.

## Before you start

- **Rust 1.89 or newer** (`rustc --version`). The library crates build on 1.88,
  but `transync-cli` takes an OS file lock when it publishes its output, and
  that API is stable from 1.89.
- **git**, and a terminal.

Linux and macOS are tested; Windows should work but is unverified. Nothing else
is required — transync builds, translates, and serves the result on its own.

## 1. Clone and build

```bash
git clone https://github.com/QuietJoon/transync
cd transync
cargo build --workspace
```

A cold build takes a minute or two, most of it `comrak` (the Markdown parser)
and `reqwest`/`rustls` (the HTTP stack). It ends with a `Finished` line and no
errors.

Every command after this runs from the repository root.

## 2. Run the tests

```bash
cargo test --workspace
```

This builds and runs every crate's tests, including the end-to-end scenario
suite under `crates/transync/tests/scenarios/` — headings, tables, code blocks,
nested lists, validation, retry, fallback, alignment — driven by an in-process
mock translator, and the static server's own suite. Nothing calls a remote
provider. Each test binary finishes with a line like:

```
test result: ok. …
```

One suite is deliberately outside that run. The CLI's own end-to-end suite,
`crates/transync-cli/tests/cli_smoke.rs`, is compiled only when the
`test-stub-provider` feature is enabled, and that feature is off by default —
so `--workspace` alone never touches the binary's translate/serve paths. Run it
too:

```bash
cargo test -p transync-cli --features test-stub-provider
```

`scripts/smoke.sh`, the repository's hard gate, runs both commands as separate
steps for exactly this reason. This second one also leaves the binary that the
next step needs already built.

## 3. Translate a sample document

The `test-stub-provider` feature swaps the live OpenAI client for an in-process
echo translator, so this run needs no API key. The echo hands each unit's
source text back unchanged: what you are exercising is the pipeline — parse,
unit construction, batching, validation, regeneration, render — not the prose.

```bash
mkdir -p /tmp/transync-demo
cargo run -p transync-cli --features test-stub-provider --quiet -- translate \
  --input crates/transync/tests/fixtures/scn-14-full.md \
  --output /tmp/transync-demo/out.md \
  --map /tmp/transync-demo/out.json \
  --html-out /tmp/transync-demo/html \
  --target-language ko
```

The run exits `0` and prints one line on stderr:

```
transync: note: html block html-0017 contains no translatable text: it is preserved verbatim and live-rendered (possibly visually empty)
```

That is a success-time note, not a failure. The fixture ends with a bare
`</details>` line, which parses as a raw HTML block with no text in it;
transync reports what it preserved rather than dropping it silently. The
`--quiet` in the command above belongs to `cargo` and only hides the build log
— transync has a `--quiet` of its own, which would go after the `--`, and that
is the flag that would suppress notes like this one. The
[diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md)
guide reads the rest of that vocabulary.

Now look at what landed:

```bash
ls /tmp/transync-demo/html
```

```
alignment.json  index.html  purify.min.js  source.html  sync.js  target.html
```

Those six files are the browsable demo bundle. Beside them,
`/tmp/transync-demo/out.md` is the regenerated Markdown and
`/tmp/transync-demo/out.json` is the alignment map that ties the two documents
together block by block.

## 4. Serve the bundle and watch the panes sync

The bundle's `index.html` fetches `source.html`, `target.html` and
`alignment.json` from beside itself, so it needs an HTTP origin — opening the
file straight from disk will not work. transync ships the server:

```bash
cargo run -p transync-cli --features test-stub-provider --quiet -- serve \
  --rendered /tmp/transync-demo/html
```

The `--features` flag is repeated only so cargo reuses the binary it already
built; `serve` itself does not use the feature. The server binds
`127.0.0.1:7470` — loopback, so it is reachable from this machine only — and
prints:

```
transync serve: listening on http://127.0.0.1:7470/ — serving …/transync-demo/html
transync serve: press Ctrl-C to stop.
```

The tail of the first line is your bundle directory, resolved to its real path.

Open `http://127.0.0.1:7470/`. Two panes appear side by side. **Scroll either
one** — the other follows, settling on the matching block in about a quarter of
a second. It is following a block ID, not a scroll percentage: that
correspondence is computed once in Rust and never re-derived in the browser,
which is the thing this project exists to do reliably. The
[architecture overview](../../../explanation/developer/en/architecture-overview.md)
explains why that distinction is the whole design.

Press Ctrl-C in the terminal when you are done. The server stops accepting,
gives in-flight connections two seconds to finish, and exits `0`.

You have now built, tested, translated, served, and seen transync's core
guarantee hold, from a clean checkout.

## Where to go next

- **Translate your own document with a live model:**
  [how to translate a document with the CLI](../../../how-to/user/en/translate-a-document-with-the-cli.md)
  — that path needs an `OPENAI_API_KEY`.
- **Shape the translation with a profile:**
  [how to write a translation profile](../../../how-to/user/en/write-a-translation-profile.md).
- **Serve a bundle on another port, or to another machine:**
  [how to serve the demo bundle](../../../how-to/operator/en/serve-the-demo-bundle.md).
- **Every flag and exit code:**
  [CLI reference](../../../reference/user/en/cli.md).
