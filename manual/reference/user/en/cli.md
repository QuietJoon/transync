---
type: Reference
title: CLI reference
description: Every flag, default, limit, exit code, environment variable, and stderr line for `transync translate` and `transync serve`.
tags: [cli, reference, flags, exit-codes, serve, SCN-12, SCN-13, DCR-0026, DCR-0028]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: cli-main, resource: crates/transync-cli/src/main.rs }
  - { id: cli-error, resource: crates/transync-cli/src/error.rs }
  - { id: cli-logging, resource: crates/transync-cli/src/logging.rs }
  - { id: cli-direction, resource: crates/transync-cli/src/direction.rs }
  - { id: cli-translate, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: cli-translate-args, resource: crates/transync-cli/src/translate_cmd/args.rs }
  - { id: cli-translate-input, resource: crates/transync-cli/src/translate_cmd/input.rs }
  - { id: cli-translate-provider, resource: crates/transync-cli/src/translate_cmd/provider.rs }
  - { id: cli-translate-publish, resource: crates/transync-cli/src/translate_cmd/publish.rs }
  - { id: cli-translate-report, resource: crates/transync-cli/src/translate_cmd/report.rs }
  - { id: cli-serve, resource: crates/transync-cli/src/serve_cmd.rs }
  - { id: cli-serve-conn, resource: crates/transync-cli/src/serve_cmd/conn.rs }
  - { id: cli-serve-route, resource: crates/transync-cli/src/serve_cmd/route.rs }
  - { id: cli-serve-mime, resource: crates/transync-cli/src/serve_cmd/mime.rs }
  - { id: cli-output, resource: crates/transync-cli/src/output.rs }
  - { id: cli-output-lock, resource: crates/transync-cli/src/output/lock.rs }
  - { id: cli-drift-test, resource: crates/transync-cli/tests/docs_cli_flags_drift.rs }
  - { id: cli-manifest, resource: crates/transync-cli/Cargo.toml }
  - { id: core-lib, resource: crates/transync-core/src/lib.rs }
  - { id: core-profile, resource: crates/transync-core/src/profile.rs }
  - { id: core-batch, resource: crates/transync-core/src/batch.rs }
  - { id: core-budget, resource: crates/transync-core/src/unit/budget.rs }
  - { id: core-cache-disk, resource: crates/transync-core/src/cache/disk.rs }
  - { id: default-profile, resource: crates/transync-core/profiles/default.toml }
  - { id: openai-lib, resource: crates/transync-openai/src/lib.rs }
  - { id: openai-endpoint, resource: crates/transync-openai/src/client/endpoint.rs }
  - { id: syntax-depth, resource: crates/transync-syntax/src/parser/depth.rs }
synced_hash: 50930da09894bf4d8e13034bb6b8a27587e976be4d3904eb09a7822841d3964f
---

# CLI reference

The `transync` binary has two subcommands: `transync translate`, which
translates a Markdown document and publishes the output set, and
`transync serve`, which serves a rendered bundle directory over HTTP on
loopback. They share an exit-code space and a stderr stream and nothing else:
`serve` has no provider, no profile, and no verbosity flags.

`--help` and `--version` are handled by the argument parser and exit `0`.

## `transync translate`

| Flag | Required | Default | Meaning |
|---|---|---|---|
| `--input <path>` | yes | — | Source Markdown file. |
| `--max-input-bytes <n>` | no | `67108864` (64 MiB) | Refuse a larger `--input` before parsing (exit `2`). The read takes `n + 1` bytes rather than trusting file metadata. Does not govern `--profile` / `--system-prompt-file`, which are capped at a fixed 4 MiB. |
| `--output <path>` | with `--map`, unless `--out-dir` | — | Destination for the translated Markdown. |
| `--map <path>` | with `--output`, unless `--out-dir` | — | Destination for the alignment-map JSON. |
| `--out-dir <dir>` | mutually exclusive with `--output` / `--map` / `--html-out` | — | Publish the whole output set into one directory. See [Output destinations](#output-destinations). |
| `--html-out <dir>` | no | — | Write the six-file browsable demo bundle into this directory. |
| `--strict-csp` | no | off | Stamp a `Content-Security-Policy` `<meta>` into the emitted bundle's `index.html`. See [`--strict-csp` policy](#--strict-csp-policy). |
| `--title <text>` | no | source document's first H1, else `transync` | Bundle `<title>`. Trimmed once; a blank or whitespace-only value is exit `1` rather than a fall-through. Bundle-only — never reaches `out.md`, the alignment map, or the provider. |
| `--validation-report <path>` | no | — | Write the per-unit validation report as its own JSON file. No effect under `--out-dir`, which always writes `validation-report.json`. |
| `--target-language <label>` | yes | — | Opaque, non-empty label. Not validated as a BCP-47 tag. |
| `--source-language <label>\|auto` | no | `auto` | `auto` is the one reserved sentinel for model-side detection: recognized case-insensitively and stored in canonical lowercase. Any other value is an opaque label with its case preserved. |
| `--target-direction <rtl\|ltr\|auto>` | no | profile `[render].target_direction`, else `auto` | Presentation-only: the bundle's target-pane text direction. `ltr` emits no `dir` attribute at all. Never affects `out.md` or the alignment map. |
| `--profile <path>` | no | the embedded default profile | Custom Profile TOML. **Replaces** the embedded profile; it is not merged onto it. See [Profile TOML schema](./profile-toml-schema.md). |
| `--system-prompt <text>` | mutually exclusive with `--system-prompt-file` | — | Replaces the active profile's `[system].prompt` body. Template variables are still substituted. |
| `--system-prompt-file <path>` | mutually exclusive with `--system-prompt` | — | The same, read from a file (4 MiB cap). |
| `--model <id>` | no | `$TRANSYNC_OPENAI_MODEL`, else `gpt-5-chat-latest` | Model ID passed to the provider. No parser default, because the environment variable resolves first. An explicitly blank value is exit `1`. |
| `--base-url <url>` | no | `$TRANSYNC_OPENAI_BASE_URL`, else `https://api.openai.com` | Provider endpoint. No parser default, for the same reason. An unparsable URL is exit `1`. |
| `--cache-dir <path>` | no | none — a fresh in-memory cache per run | Open (creating if absent) a disk-backed translation cache in this directory, logged to `transync-cache.jsonl`. A later run over the same document under the same profile, model and languages re-dispatches only what changed, and a fully-cache-hit `--source-language auto` run still reports the first run's detected language. One writer at a time. A directory that cannot be opened warns once and the run continues on a fresh in-memory cache. |
| `--force` | no | off | Waive the foreign-file refusal when writing into an `--html-out` directory or replacing an `--out-dir` target that holds files transync did not write. Not needed to overwrite transync's own prior bundle or output set. |
| `--target-output-tokens <n>` | no | `8000` (from the embedded default profile) | Per-batch output-token ceiling: sent to the provider as the response cap and used as the budget the batcher packs against. Overlays `[batching].target_output_tokens`. `0` disables the ceiling. See [Batching and concurrency](#batching-and-concurrency). |
| `--output-expansion-factor <f>` | no | `2.0` | Assumed output-to-source token ratio used to size batches. Must be a finite number greater than zero; values below `1.0` are legal. |
| `--target-input-tokens-per-batch <n>` | no | `6000` | Soft per-batch input-token budget; `n >= 1`. |
| `--max-units-per-batch <n>` | no | `8` (from the embedded default profile) | Hard cap on units per batch; `n >= 1`. |
| `--table-strategy <whole-block\|row-window-first>` | no | `row-window-first` (from the embedded default profile) | What happens to a table whose estimated response exceeds the output ceiling. `row-window-first` splits it into header-carrying row windows at packing time and reassembles one table afterwards; `whole-block` ships it whole. Overlays `[constraints].default_table_strategy`. Any other value is exit `1`. Inert when there is no ceiling. |
| `--max-concurrent-batches <n>` | no | `6` | In-flight provider requests; `n >= 1`. The one batching knob with no profile home. |
| `--auto-glossary` | mutually exclusive with `--no-auto-glossary` | off | Run the auto-extracted candidate-glossary preflight for this run — one extra provider call before batching. |
| `--no-auto-glossary` | mutually exclusive with `--auto-glossary` | off | Skip it, overriding a profile that opts in. |
| `--quiet` | mutually exclusive with `--verbose` | off | Suppress every `transync: ` line and set the tracing floor to `off`. |
| `--verbose` | mutually exclusive with `--quiet` | off | Print a validation tally on success and set the tracing floor to `debug`. |

There is no flag for the retry-policy knobs (`max_per_unit_validation_retries`,
`max_per_batch_schema_retries`, `max_per_batch_provider_retries`); their
built-in values are `2`, `2` and `1`. There is no flag for cancellation either:
`TranslateOptions::cancel` is a library capability and the CLI never sets it, so
Ctrl-C during a `translate` run terminates the process rather than producing a
clean cancellation.

### Output destinations

`--out-dir <dir>` publishes a fixed layout:

```
<dir>/
├── out.md                   translated Markdown
├── alignment.json           alignment map
├── validation-report.json   always written in this mode
├── html/                    the six-file demo bundle
└── .transync-out-dir        ownership marker
```

The six bundle filenames are `index.html`, `source.html`, `target.html`,
`alignment.json`, `sync.js`, `purify.min.js`. Any other regular file in an
`--html-out` directory makes it foreign and requires `--force`; `*.tmp.<pid>`
staging leftovers and the `.transync-publish.lock` marker are transync's own and
never count as foreign.

A fresh `--out-dir` target appears with one atomic rename. Replacing an existing
target is crash-safe but **not** atomic: the old tree is renamed aside first, so
an unrelated reader looking between the two renames finds no target at all.

Without `--out-dir`, both `--output` and `--map` are required. Neither alone is a
legal invocation.

Destination guards run twice: once before the provider call, silently, so a
refusal costs no paid translation; and again at publication time, under the
publication lock for `--out-dir`. Both passes produce the same exit code and the
same sentence.

### Argument-boundary refusals

These are the CLI's own refusals, all exit `1`, each printed as
`transync: <message>`:

- `--target-language must not be empty`
- `--source-language must not be empty (use "auto" to detect)`
- `--model must not be empty`
- `--title must not be empty`
- `one of --out-dir, or both --output and --map, is required`
- `--output requires --map (or use --out-dir)`
- `--map requires --output (or use --out-dir)`
- `invalid --base-url: {e}`
- `invalid TRANSYNC_OPENAI_BASE_URL: {e}`
- `OPENAI_API_KEY not set; set it or build --features test-stub-provider`
- `invalid profile TOML: {e}`
- `failed to read --profile {path}: {e}` / `failed to read --system-prompt-file {path}: {e}`
- `--profile {path} is not valid UTF-8` / `--system-prompt-file {path} is not valid UTF-8`

The size-cap sibling of those last two is exit `2`, not `1`:
`--profile {path} exceeds the 4194304-byte read limit`.

Refusals decided by the argument parser itself — an unknown flag, a conflicting
pair, a value outside a declared range — also exit `1`, but print the parser's
own usage error rather than a `transync: ` line, and `--quiet` cannot suppress
them.

### Parser-enforced constraints

- Mutually exclusive pairs: `--out-dir` against each of `--output`, `--map`,
  `--html-out`; `--system-prompt` against `--system-prompt-file`;
  `--auto-glossary` against `--no-auto-glossary`; `--quiet` against `--verbose`.
- Range `n >= 1`: `--target-input-tokens-per-batch`, `--max-units-per-batch`,
  `--max-concurrent-batches`. `--target-output-tokens` carries no range check.
- Fixed value list: `--table-strategy` accepts `whole-block` and
  `row-window-first` and nothing else.
- `--output-expansion-factor` is rejected at parse time for anything
  non-numeric, non-finite, or at or below zero:
  `` `{s}` is not a number `` or
  `` expansion factor must be a positive, finite number, got `{s}` ``.

## Defaults: built-in versus the embedded default profile

A run with no `--profile` uses the profile embedded in the binary, and that
profile overrides several library built-ins. Where the two differ, the profile
wins.

| Knob | Library built-in | Embedded default profile | Effective with no `--profile` |
|---|---|---|---|
| `target_output_tokens` | unset — no ceiling | `8000` | `8000` |
| `output_expansion_factor` | `2.0` | `2.0` | `2.0` |
| `target_input_tokens_per_batch` | `6000` | not set | `6000` |
| `max_units_per_batch` | `32` | `8` | `8` |
| `max_concurrent_batches` | `6` | no profile key exists | `6` |
| `default_table_strategy` | `whole-block` (an absent key resolves here) | `row-window-first` | `row-window-first` |

`--profile <path>` replaces the embedded profile rather than merging with it, so
a custom profile that omits a key falls to the **built-in** column, not to the
middle one. A custom profile with no `[constraints].default_table_strategy` and
no `--table-strategy` therefore does not split oversize tables.

## Batching and concurrency

Five knobs are applied as a single overlay onto the resolved profile, under
**flag > profile > built-in default**. Because the overlay is unconditional, a
flag value that happens to equal a built-in default still beats the profile.

| Flag | Profile key |
|---|---|
| `--target-output-tokens` | `[batching].target_output_tokens` |
| `--output-expansion-factor` | `[batching].output_expansion_factor` |
| `--target-input-tokens-per-batch` | `[batching].target_input_tokens_per_batch` |
| `--max-units-per-batch` | `[batching].max_units_per_batch` |
| `--table-strategy` | `[constraints].default_table_strategy` |

`--max-concurrent-batches` is the exception: it has no profile home and is
written directly onto the run's options.

**`--target-output-tokens` and zero.** The flag carries no range validator, so
`0` parses and is the documented sentinel for "no ceiling": no cap is sent to
the provider, output-aware packing is off, and the at-risk preflight is off.
Because the embedded default profile always sets `8000`, `--target-output-tokens 0`
is the only way a run taking that profile can express "no ceiling"; a custom
`--profile` that omits the key has no ceiling to begin with. Separately, any
nonzero value at or below `64` — the fixed response-envelope reserve — leaves no
room for a single translated unit; the profile loader warns about it and
normalizes it to unset, which disables the ceiling as well. The three sibling
flags with `n >= 1` ranges reject `0` at parse time instead, as exit `1`.

A table splits only when a ceiling exists and the unit's estimated response
exceeds `ceiling - 64`, measured with the same encoder and the same expansion
factor the packer uses. Raising `--target-output-tokens` or lowering
`--output-expansion-factor` moves the split threshold and the window boundaries
together.

## Language-label rules

Both `--source-language` and `--target-language` take an opaque label, not a
validated tag. Exactly two normalizations happen, once, at the argument
boundary:

- Surrounding whitespace is trimmed from both labels.
- A recognized source sentinel is stored in its canonical spelling. `auto` is
  the one reserved literal, recognized through padding and ASCII case, so
  `--source-language ' AUTO '` is the sentinel and is stored as `auto`.

Nothing else is folded. Interior text, case, and non-ASCII characters survive,
so `--target-language 'Korean (formal, 존댓말)'` reaches the model intact and
`--target-language AUTO` is an ordinary label. An empty or whitespace-only label
is exit `1`.

The normalized values are what the compiled system prompt, the request payload,
the cache key, the alignment map, and the bundle's `lang` and `dir` attributes
all read.

## `--strict-csp` policy

The stamped policy, verbatim:

```
default-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline';
style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'
```

`'self'` is scoped to the serving origin, so it blocks other hosts, not sibling
paths under the same document root. Remote images stop rendering under it;
everything the bundle needs itself — inline style, the module script, the three
same-origin fetches, `data:` images — stays legal. It applies to whichever
bundle the run emits (`--html-out`, or `--out-dir`'s `html/`). With neither, the
flag has no effect and the run says so on stderr.

`frame-ancestors`, `sandbox` and `report-uri` are absent because a
`<meta>`-delivered policy ignores them.

## `transync serve`

A loopback static file server for a rendered bundle directory. It serves and
does nothing else: no upload, no directory listing, no execution, no proxying.

| Flag | Required | Default | Meaning |
|---|---|---|---|
| `--rendered <dir>` | yes | — | Directory to serve, normally an `--html-out` bundle. Canonicalized once at startup, so a symlinked directory is served as the directory it points at. |
| `--port <u16>` | no | `7470` | TCP port. `0` asks the OS for a free one, which is then printed with the bound address. |
| `--bind <addr>` | no | `127.0.0.1` | Address to bind, parsed as an IP address. A value that is not one is exit `1`. Any non-loopback address prints a warning. |

`serve` has no `--quiet` and no `--verbose`. It always runs at the default
tracing floor (`warn`), and its own diagnostics are prefixed
`transync serve: `, not `transync: `.

### Startup and shutdown lines

On stderr, in order:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/dir
transync serve: WARNING: <ip> is not a loopback address — everything under <root> is reachable from other machines on this network.
transync serve: press Ctrl-C to stop.
```

The warning line appears only when the bound address is not a loopback address.
Ctrl-C stops the accept loop, as does `SIGTERM` on platforms that have one
(a supervisor that stops the server sends the latter); the server then prints
`transync serve: shutting down.`, drains in-flight connections for two seconds,
and exits `0`. If connections remain after the grace period it prints
`transync serve: N connection(s) still open after 2s; closing anyway.` and
aborts them.

An accept error prints `transync serve: accept failed: {err}` and is not fatal;
the loop backs off 50 ms and continues.

### HTTP contract

`GET` and `HEAD` only. A `HEAD` response carries the head and no body.

| Status | Cause |
|---|---|
| `200 OK` | A regular file inside the served root. |
| `400 Bad Request` | The request line is not three tokens ending in an `HTTP/` version; the target is not origin-form; a percent escape is truncated or non-hex; a decoded segment is not valid UTF-8; a segment decodes to something carrying `/`, `\` or NUL. |
| `403 Forbidden` | A segment decodes to `.` or `..`; or the canonicalized path resolves outside the served root, which is what catches a symlink escape. |
| `404 Not Found` | Nothing is at that path, or what is there is not a regular file — a directory with no `index.html`, a device, a socket. |
| `405 Method Not Allowed` | Any method other than `GET` or `HEAD`. The response carries `Allow: GET, HEAD`. |
| `408 Request Timeout` | The request head did not arrive within the head timeout. |
| `431 Request Header Fields Too Large` | The request head exceeded the head cap. |

Refusals reject; they never clamp a traversal back into the root. A refused
request is an HTTP status on the connection, never a process exit — the server
keeps serving.

Every response carries these headers:

| Header | Value |
|---|---|
| `Content-Type` | From the extension table below. |
| `Content-Length` | The body length actually written (bodies at or below the in-memory limit) or the open handle's length. |
| `Connection` | `close` |
| `X-Content-Type-Options` | `nosniff` |
| `Cache-Control` | `no-store` |

There is no keep-alive: exactly one request is read per connection.

### Content types

By filename extension only, never by content sniffing. The extension is matched
case-insensitively, and the type is taken from the **canonical** path, so a
symlink cannot relabel a file.

| Extension | `Content-Type` |
|---|---|
| `.html`, `.htm` | `text/html; charset=utf-8` |
| `.js`, `.mjs` | `text/javascript; charset=utf-8` |
| `.json`, `.map` | `application/json; charset=utf-8` |
| `.css` | `text/css; charset=utf-8` |
| `.md` | `text/markdown; charset=utf-8` |
| `.txt` | `text/plain; charset=utf-8` |
| `.svg` | `image/svg+xml` |
| `.png` | `image/png` |
| `.wasm` | `application/wasm` |
| anything else, or no extension at all | `application/octet-stream` |

### Path resolution

The request target is split into `/` segments **before** anything is decoded,
and each segment is percent-decoded on its own. The bare `/`, and any target
ending in `/`, resolve to `index.html` in the directory named. The resolved
path is then canonicalized and checked against the canonical root; only then is
it opened.

### Limits

| Limit | Value |
|---|---|
| Request-head size | 8 KiB (`431` beyond it) |
| Request-head arrival | 15 s (`408` beyond it) |
| In-flight connections | 128; the accept loop waits for one to finish |
| Accept-error backoff | 50 ms |
| Response read into memory | at or below 8 MiB; larger files stream |
| Shutdown grace period | 2 s |

### What it does not do

- **No directory listing**, in either spelling. A directory without an
  `index.html` is `404`; files inside it stay reachable by name.
- **No TLS, no HTTP ranges, no conditional requests**, and no non-loopback
  deployment hardening. These are out of scope by decision.
- **One documented residual race.** The path is canonicalized, checked, and then
  opened. Another local process could replace an entry between the check and the
  open. The window is one syscall wide and the path opened is the one just
  verified to contain no links, so exploiting it requires local write access
  inside the served directory. Closing it entirely would need an
  `openat`/`O_NOFOLLOW` walk, which this server does not do.

## Exit codes

| Code | `transync translate` | `transync serve` |
|---:|---|---|
| 0 | Success. Also `--help` and `--version`. | A shutdown signal stopped a running server after draining. |
| 1 | The argument parser rejected argv, `--profile` failed to load, or one of the CLI's own boundary refusals fired. | The argument parser rejected argv, including a `--bind` value that is not an IP address. |
| 2 | Input read or parse failure: `--input` missing, unreadable, not valid UTF-8, or larger than `--max-input-bytes`; a `--profile` or `--system-prompt-file` past the fixed 4 MiB cap; any parse error, including the block-nesting refusal at depth 128. | `--rendered` cannot be canonicalized, is not a directory, or is unreadable. |
| 3 | Every translatable unit fell back to source. All outputs are still written. Fires only when the run had at least one unit. | Not reachable. |
| 4 | Write failure — `could not write --html-out bundle to {dir}: {source}` or `could not write outputs: {e}`. | Not reachable. |
| 5 | The residual: a translator or pipeline failure this build cannot classify further, or an alignment-map / validation-report serialization failure. | The address could not be bound (port in use, address not assigned locally), or the bound address could not be read back. |
| 6 | The provider refused the run over **how it was configured** — a rejected credential, a request the provider refused outright, an exhausted output ceiling, or an exceeded context window. | Not reachable: `serve` has no provider. |
| 7 | The provider refused **this document's content** — its content policy stopped generation, or the model declined and said so. | Not reachable. |

`6` and `7` are the two a script can branch on, because they differ in what the
caller must do: `6` has a configuration remedy, `7` has none — a retry
resubmits the identical payload, so the same content is refused again. Both
abort before anything is published, which is what separates them from `3`,
where outputs are written.

Codes are append-only. `6` and `7` were added by moving causes out of `5`; every
code `0`–`5` still means what it always meant.

**What prints alongside a non-zero exit.** A `translate` run that fails after
argument parsing prints one `transync: <reason>` line on stderr, suppressed by
`--quiet`. A parser-level failure prints the parser's own error instead, in its
own format, and no flag suppresses it. `serve` has no `--quiet`, and its failure
lines are prefixed `transync serve: ` — for example
`transync serve: cannot bind {addr}: {err}` and
`transync serve: cannot serve {path}: {err}`.

## Environment variables

| Variable | Default | Effect |
|---|---|---|
| `OPENAI_API_KEY` | — | Required for the live provider. Read from the environment and never written anywhere. |
| `TRANSYNC_OPENAI_MODEL` | `gpt-5-chat-latest` | Model ID when `--model` is absent. A blank or whitespace-only value is ignored and the default applies. |
| `TRANSYNC_OPENAI_BASE_URL` | `https://api.openai.com` | Provider endpoint when `--base-url` is absent — an OpenAI-compatible proxy, gateway, or local server. An empty value is ignored. An unparsable one is exit `1`. |
| `TRANSYNC_OPENAI_API` | model-driven | `chat` or `responses`; an explicit override of the endpoint-dispatch heuristic. Read once, at adapter construction. |
| `RUST_LOG` | unset | Layered **over** the flag-derived tracing floor rather than replacing it, so `RUST_LOG=transync::pipeline=trace` widens that one target and leaves everything else at its floor. Floors: `--quiet` is `off` and does not consult `RUST_LOG` at all; no flag is `warn`; `--verbose` is `debug`. An unparsable directive prints `transync: ignoring unparsable RUST_LOG directive {directive}: {e}` and is skipped; the rest of the variable still applies. |

Library `tracing` records print as `LEVEL target: message` with no timestamp,
on the same stderr as the command's own lines.

A second provider crate, `transync-anthropic`, exists in the workspace and reads
its own `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` and
`TRANSYNC_ANTHROPIC_BASE_URL` — but no CLI flag selects a provider and the
binary depends only on the OpenAI adapter, so none of those three variables
affects a `transync` invocation.

Four further variables exist only in a build carrying the `test-stub-provider`
cargo feature and are absent from a release binary: `TRANSYNC_STUB_ECHO_PATH`,
`TRANSYNC_STUB_MODE`, `TRANSYNC_STUB_DETECT` and `TRANSYNC_STUB_GLOSSARY`.

## Advisory stderr lines on a successful run

A run that exits `0` can still print `transync: note: …` lines. Every one of
them is advisory: none changes the exit code, and none changes whether the
published bytes are correct. `--quiet` suppresses them all.

| Line | Meaning |
|---|---|
| `note: <skipped source node>` | A top-level source node that survives into the translated Markdown but is not ordinary translated content — front matter, a footnote definition, an html block with nothing translatable, or an html block whose segment extraction failed. |
| `note: waiting for another transync run to finish publishing into {dir}` | Another run holds the publication lock on that directory. This run waits; neither fails. |
| `note: {dir} holds {n} staging temp(s) from other transync runs (e.g. {first}) — kept in case a run is still staging into them; delete them by hand once no transync run is active` | Staging leftovers carrying another process's pid are never reclaimed automatically. |
| `note: could not flush the directory {dir} to disk ({e}); the published file CONTENTS are durable, but the directory entries naming them may not survive a crash — re-run the publication if the machine goes down before the filesystem catches up` | The directory could not be flushed, or could not even be opened to try. |
| `note: could not clean up {n} {what} after a failed publication (e.g. {path}: {e}); the residue is inert — delete it by hand once no transync run is active` | Best-effort cleanup after a *failed* publication left residue. `{what}` is either `staged temp file(s)` or `directory level(s) this run created`. Printed once however many entries failed, and printed **beside** the run's real error, not instead of it. |
| `note: could not carry the existing permissions of {path} over to its replacement ({e}); the published file keeps this run's default mode instead` | Unix only. |
| `note: --strict-csp has no effect without --html-out or --out-dir (this run emits no HTML bundle)` | The flag was passed but no bundle was emitted. |
| `could not open --cache-dir {dir}: {e}; continuing with a fresh in-memory cache (this run will not reuse or persist anything)` | The disk cache could not be opened. The run continues and translates everything. |

Two further lines are warnings rather than notes, and also leave the exit code
alone:

- `warning: auto-glossary: extraction failed ({diagnostic}); static glossary only`
  — printed unconditionally except under `--quiet`, so a `RUST_LOG` directive
  cannot hide it.
- `warning: {n} unit(s) estimated over the per-batch output ceiling (each named on
  the transync::pipeline warning channel and in --validation-report) — raise
  --target-output-tokens, lower --output-expansion-factor, or split the source
  block` — with `, --table-strategy row-window-first (tables only)` inserted
  before "or split" when at least one flagged unit is a table block. Exactly one
  such line per run, however many units were flagged.

`--verbose` adds a tally line,
`transync: total_units=N translated=N preserved=N partial=N fallback=N retried=N`,
and — when the auto-glossary preflight ran — one `transync: auto-glossary: …`
summary line.

Profile load warnings are not re-printed by the CLI. They reach stderr only
through the `transync::profile` tracing target, at the default `warn` floor.

## Related pages

- [How to translate a document with the CLI](../../../how-to/user/en/translate-a-document-with-the-cli.md)
- [How to reuse a warm cache across runs](../../../how-to/user/en/reuse-a-warm-cache-across-runs.md)
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md)
- [How to write a translation profile](../../../how-to/user/en/write-a-translation-profile.md)
- [How to serve the demo bundle](../../../how-to/operator/en/serve-the-demo-bundle.md)
- [How to build the wasm demo](../../../how-to/operator/en/build-the-wasm-demo.md)
- [Profile TOML schema](./profile-toml-schema.md)
- [Alignment map schema](../../developer/en/alignment-map-schema.md)
- [Why batches stop at section boundaries](../../../explanation/user/en/why-batches-stop-at-section-boundaries.md)
