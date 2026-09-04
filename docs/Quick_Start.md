# Quick Start

Get transync running locally and produce your first translated demo
bundle in about five minutes.

## Prerequisites

| Tool      | Minimum | Used for                                          |
|-----------|---------|---------------------------------------------------|
| Rust      | 1.89    | building the workspace (see below)                |
| OpenAI key| —       | only for the live API path; tests use an echo stub|
| `wasm32-unknown-unknown` rustup target | — | **not** needed for steps 1–5; needed by `./scripts/smoke.sh` |
| `wasm-pack` + binaryen `wasm-opt` | 121 | **not** needed for steps 1–5; needed by `./scripts/smoke.sh` and `./scripts/build-wasm.sh` |

1.89 is what a **whole-workspace** build needs, because `transync-cli`
takes an OS file lock when it publishes a bundle (DCR-0021). Depending on
the library crates alone — `transync`, `transync-core`,
`transync-syntax`, `transync-openai` — needs **1.88**, the workspace
floor: edition 2024 plus `if let … && …` let-chains.

No numbered step below needs the last two rows. They are listed because
`./scripts/smoke.sh` — which a newcomer reasonably reads as "the
getting-started script" — does need them, and finding that out from a
failure mid-run is worse than reading it here. See *What
`./scripts/smoke.sh` is* at the end of step 3.

Linux and macOS are tested; Windows should work but is unverified.

## 1. Clone and build

```bash
git clone https://github.com/QuietJoon/transync
cd transync
cargo build --workspace
```

Cold compile is ~1 minute on a developer laptop (mostly comrak +
reqwest/rustls).

## 2. Run the test suite

```bash
cargo test --workspace
```

14 scenario tests must pass. They drive the full pipeline through an
in-process `MockTranslator`; no network access is required.

## 3. First end-to-end run, no API key

The `test-stub-provider` Cargo feature swaps an in-process echo
translator for the OpenAI client, so the whole pipeline — parse, unit
construction, batching, validation, regeneration, render — runs with no
network and no key. Drive it against the SCN-14 fixture, into any
directory you like:

```bash
mkdir -p /tmp/transync-first-run
cargo run -p transync-cli --features test-stub-provider -- translate \
  --input crates/transync/tests/fixtures/scn-14-full.md \
  --output /tmp/transync-first-run/out.md \
  --map /tmp/transync-first-run/align.json \
  --html-out /tmp/transync-first-run/html \
  --target-language ko
```

That writes `out.md` (the regenerated document), `align.json` (the
alignment map), and the six-file demo bundle in `html/` —
`index.html`, `source.html`, `target.html`, `alignment.json`,
`sync.js`, `purify.min.js`. The one `note:` line on stderr about an
html block with no translatable text is expected: the fixture contains
one.

An HTML document goes through the same pipeline — declare the format
(routing is flag-only; the sniff never guesses):

```bash
transync translate --input page.html --input-format html \
  --out-dir out/ --target-language ko
```

That writes `out.html` (the translated page, anchor-free) beside the
same `html/` sync bundle; `transync serve --rendered out/` shows the
real page.

To look at that bundle in a browser, serve it:

```bash
cargo run -p transync-cli --features test-stub-provider --quiet -- serve \
  --rendered /tmp/transync-first-run/html
```

That binds `127.0.0.1:7470` — loopback only unless you pass `--bind` —
and serves until Ctrl-C. Open `http://127.0.0.1:7470/`.

This is the whole of the no-API-key path. It needs nothing beyond
Rust.

### What `./scripts/smoke.sh` is

Not this. It is the repository's **hard gate**: there is no CI here, so
this script is the run a human does instead. It is also where the last
two Prerequisites rows come from. In order, it runs:

1. `cargo build --workspace`
2. `cargo check -p transync-syntax -p transync-wasm --target
   wasm32-unknown-unknown` — the standing wasm gate
3. `cargo test --workspace -- --test-threads=4`
4. `cargo test -p transync-cli --features test-stub-provider --
   --test-threads=4`
5. a completeness check that every library member is named in the
   rustdoc gate's crate list, then `RUSTDOCFLAGS="-D warnings" cargo
   doc --no-deps` over those crates
6. `scripts/build-wasm.sh` — wasm-pack plus an explicitly resolved
   binaryen `wasm-opt`, against a size budget
7. *only then* the CLI dry run of step 3, whose eight output files must
   all be non-empty

So it is a full workspace build and two test suites before it reaches
the part step 3 does, and the two wasm prerequisites are load-bearing
rather than optional: sub-step 2 fails if the `wasm32-unknown-unknown`
rustup target is missing, and sub-step 6 exits immediately with an
install hint if `wasm-pack` or a `wasm-opt` ≥ 121 is not on `PATH`.
Both fail loudly instead of skipping, on purpose — the wasm module is a
shipped artifact, not an optional linter. Install them with `rustup
target add wasm32-unknown-unknown` and, on macOS, `brew install
wasm-pack binaryen`; wasm-pack's own cached binaryen 117 is **not** a
substitute.

Run it when you want the repository checked — before a release, or
after a change big enough that the pre-commit hook's fmt / clippy /
wasm / rustdoc gates are not enough. `docs/project/release-checklist.md`
step 5 is where it is mandatory. It is not the way to see transync
work; step 3 is.

## 4. First live run with an API key

Drop your key into the environment, then drive the live OpenAI API and
serve the demo bundle for browsing:

```bash
export OPENAI_API_KEY=sk-...
./scripts/test.sh
```

The script builds, then calls the OpenAI API against
`samples/demo-complex.md` (64 blocks → 63 translation units — the one
thematic break carries no text — packed into 12 batches). The endpoint is chosen per model name
(model-driven dispatch): the
default model `gpt-5-chat-latest` routes to Chat Completions
(`/v1/chat/completions`), while o-series and non-chat GPT-5 models
route to Responses (`/v1/responses`); set
`TRANSYNC_OPENAI_API=chat|responses` to override. The script then
verifies all output files exist and `exec`s `transync serve --rendered
<bundle> --bind 0.0.0.0 --port 7470`.

**Open it at `http://localhost:7470/`.** The wildcard bind decides which
*network* can open the socket; it does not decide which authorities the
server answers for. Those are derived separately, and a wildcard bind
names no interface — so `serve` answers only for the loopback authorities
of the socket's own family. The default `0.0.0.0` bind is IPv4-only, so that
is `127.0.0.1:7470` and `localhost:7470`; a `--bind ::` run is dual-stack and
adds `[::1]:7470`. A browser on another machine on the
LAN types the dev box's address instead — say `http://10.0.0.2:7470/` —
so its request carries `Host: 10.0.0.2:7470`, which is not on that list,
and it gets a `421` rather than the demo. To browse from another
machine, name the authority it types before starting the script:

```bash
TRANSYNC_LIVE_ALLOW_HOST=10.0.0.2:7470 \
OPENAI_API_KEY=sk-... \
./scripts/test.sh
```

`scripts/smoke-live.sh` forwards that as `--allow-host`. The warning
`serve` prints about the non-loopback bind is about network reach, not
about this check; the list of authorities it answers for is printed
beside it at startup. Ctrl-C in the script terminal stops the server.

To translate your own document, override the input:

```bash
TRANSYNC_LIVE_INPUT=/path/to/your.md \
TRANSYNC_LIVE_TARGET_LANG=ko \
OPENAI_API_KEY=sk-... \
./scripts/test.sh
```

`scripts/test.sh` documents every override at the top of the file.

**Pay for a document once.** Add `--cache-dir <path>` to a `transync
translate` invocation and the run keeps its translated units in a log in
that directory instead of dropping them on exit. Re-run the same document
against the same directory and the untouched parts are served from the
cache, including the language a `--source-language auto` run detected. Use
one directory per person, not one shared over a network drive: a cache
directory has one writer at a time.

**Expect the second run to re-dispatch more than you edited.** A cache
entry is keyed on everything the model was shown for that block, not on the
block's own text alone, so an edit invalidates a neighbourhood rather than a
block. Each unit's prompt carries the kind and a 120-character leading
excerpt of the blocks immediately before and after it, so editing one
paragraph also re-dispatches the block above it and the block below it. Each
prompt likewise carries the enclosing heading path — text *and* level — so
editing a heading re-dispatches everything beneath it, subsections included,
and editing the document's first `#` heading, which every unit is told as
the document title, re-dispatches the whole document. Batches are packed one
section at a time, and a batch's instruction gains clauses when it holds a
raw-HTML block or a table big enough to be split into row windows, so adding
or removing one of those re-keys every unit packed beside it. A different
profile, model, or language label invalidates everything, by design.

What does *not* widen it is renumbering. Block IDs are not part of the key,
so inserting a block does not re-dispatch the ones after it, and two blocks
with identical text in identical context share one entry. A second run
costing more than the diff suggests is the cache working as designed, not
the cache failing.

## 5. Manual sync verification

Open the demo page in any modern browser. Scroll either pane —
the other follows smoothly within ~250 ms. The detailed manual smoke
checklist is at `web/SMOKE.md`.

## Where to go next

- **Use transync as a library or extend it:** `docs/Developer_Guide.md`.
- **Architecture overview:** `docs/architecture/README.md`.
- **Why each design decision was made:** `docs/decisions/` (ADRs).
- **The active design baseline + document authority order:** `docs/project/design-baseline-2026-07.md` (`BL-2026-07-B`); the superseded `BL-2026-05-01-A` record is preserved in `docs/project/design-baseline.md`.
- **Full CLI argument contract:** `docs/architecture/contracts.md` §6.
