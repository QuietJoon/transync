---
type: How-To Guide
title: How to serve a translated demo bundle with transync serve
description: Serve an --html-out or --out-dir demo bundle over loopback HTTP with `transync serve`, confirm the two panes sync, and stop it cleanly.
tags: [cli, serve, sync, SCN-13]
audience: operator
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: serve-cmd, resource: crates/transync-cli/src/serve_cmd.rs }
  - { id: serve-route, resource: crates/transync-cli/src/serve_cmd/route.rs }
  - { id: serve-mime, resource: crates/transync-cli/src/serve_cmd/mime.rs }
  - { id: serve-conn, resource: crates/transync-cli/src/serve_cmd/conn.rs }
  - { id: serve-tests, resource: crates/transync-cli/tests/serve_static.rs }
  - { id: exit-codes, resource: crates/transync-cli/src/error.rs }
  - { id: cli-main, resource: crates/transync-cli/src/main.rs }
  - { id: bundle-entries, resource: crates/transync-cli/src/output.rs }
  - { id: bundle-shell, resource: crates/transync-cli/web/index.html.tpl }
  - { id: sync-engine, resource: web/js/sync.js }
  - { id: align-version, resource: crates/transync-syntax/src/align.rs }
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: persistence-files, resource: docs/architecture/persistence-and-files.md }
  - { id: web-smoke, resource: web/SMOKE.md }
  - { id: browser-suite, resource: web/playwright.config.js }
  - { id: test-browser, resource: scripts/test-browser.sh }
synced_hash: fcd9c21d0f4db65d208f6a2888d733be00a5148dd68e891f62f26a02463d14df
---

# How to serve a translated demo bundle with `transync serve`

Get an already-generated demo bundle (from `transync translate --html-out <dir>`
or `--out-dir <dir>`) open and scroll-syncing in a browser, using the server
`transync` ships.

## Before you start

- You have a bundle from `transync translate --html-out <dir>` or
  `--out-dir <dir>`. See
  [how to translate a document with the CLI](../../user/en/translate-a-document-with-the-cli.md)
  if you do not.
- You need a server at all because the bundle's shell (`index.html`) pulls
  `source.html`, `target.html` and `alignment.json` with `fetch()` before it
  mounts anything. Opened as a `file://` URL those fetches are blocked or
  opaque-origin in most browsers, so the panes never appear.

## Locate the bundle directory

- A `--html-out <dir>` run: the six bundle files (`index.html`,
  `source.html`, `target.html`, `alignment.json`, `sync.js`,
  `purify.min.js` — the set `crates/transync-cli/src/output.rs` calls
  `HTML_BUNDLE_ENTRIES`) sit directly in `<dir>`. Serve `<dir>`.
- An `--out-dir <dir>` run: those same six files live one level down, in
  `<dir>/html/`, alongside top-level siblings `out.md`, `alignment.json` and
  `validation-report.json` at `<dir>` itself. The browser needs the copy inside
  `html/` — **point `--rendered` at `<dir>/html`, not `<dir>`**. Serving the
  parent is the most common mistake: `/` resolves to a directory with no
  `index.html`, so the very first request is a 404.

## Serve it

```
transync serve --rendered <bundle-dir>        # or <out-dir>/html
```

The directory is an argument, so no `cd` is needed, and `--rendered` may itself
be a symlink — the root is canonicalized once at startup and every request is
measured against the link's target.

Two lines arrive on stderr:

```
transync serve: listening on http://127.0.0.1:7470/ — serving /abs/path/to/bundle
transync serve: press Ctrl-C to stop.
```

`127.0.0.1` and `7470` are the flag defaults (`--bind`, `--port`). Useful
variations:

- `--port 0` asks the OS for a free port. The `listening on` line reports the
  address the kernel actually bound, so that line is the only place the chosen
  port appears.
- `--bind 0.0.0.0` (or any non-loopback address) works, and prints a warning
  first: `transync serve: WARNING: 0.0.0.0 is not a loopback address —
  everything under <root> is reachable from other machines on this network.`
- Everything `serve` prints goes to **stderr**; stdout stays empty. A script
  that needs the bound port reads stderr. `serve` has no `--quiet` or
  `--verbose` flag — `crates/transync-cli/src/main.rs` pins it to the default
  verbosity floor.

Full flag, status, header and MIME tables live in
[the CLI reference](../../../reference/user/en/cli.md); this page does not
repeat them.

## Open it and check the panes sync

Open `http://127.0.0.1:7470/` (or whatever the `listening on` line printed).
`/` and any target ending in `/` resolve to `index.html` in that directory, so
the bare root just works. Scrolling one pane should carry the other along by
block ID; the follower closes about 95 % of the gap within roughly 250 ms of
your stopping.

Three checks, cheapest first:

- **Mount verdict.** On success `mountSync` tags *both* pane elements with
  `__transyncController`; on refusal it returns `null` and tags nothing. In a
  CLI bundle the panes are `#source` and `#target`, so
  `document.getElementById('source').__transyncController` being present is the
  authoritative answer to "did it mount" — it is what the headless suite reads.
- **Anchors.** `document.querySelectorAll("[data-sync-id]")` returns the same
  non-empty id set in both panes.
- **Console.** For a map this engine's version matches, `web/js/sync.js` prints
  `transync: alignment map loaded (schema_version=<the map's own version>)` via
  `console.debug` — some browsers hide `debug` lines behind a "Verbose" level
  toggle. `1.2.0` is the current wire version
  (`ALIGNMENT_SCHEMA_VERSION` in `crates/transync-syntax/src/align.rs`); see
  [the alignment-map schema reference](../../../reference/developer/en/alignment-map-schema.md).
  Read this line carefully rather than as a pass/fail: a same-major map *newer*
  than 1.2.0 replaces the debug line with a `console.warn` (`… is newer than
  this engine (1.2.0); proceeding, but sync may be incomplete`) and still
  syncs, so neither the absence of a debug line nor the presence of a warning
  is by itself a failure.

Resizing the window needs no follow-up scroll. The engine re-aligns itself:
a `ResizeObserver` on both panes, `document.fonts.ready`, and capture-phase
`load`/`error` on `<img>` inside either pane each schedule one coalesced
recompute that re-collects both anchor sets and re-drives the follower. Only
*replacing* a pane's HTML still requires `controller.destroy()` and a fresh
`mountSync`.

## Stop it

Ctrl-C, or `SIGTERM` from a supervisor or a shell `kill`. `transync serve:
shutting down.` is printed, the listener is dropped so nothing new arrives,
in-flight connections get a 2-second grace period, and anything still open
after that is aborted with a `… connection(s) still open after 2s; closing
anyway.` line. The process exits **0**, so a script chaining off `transync
serve` reads a deliberate stop as success rather than as a failure.

## What it will and will not serve

Worth knowing before pointing `--rendered` at anything other than a bundle:

- **`GET` and `HEAD` only.** Every other method is 405 with
  `Allow: GET, HEAD`. There is no upload, no execution and no proxying.
- **No directory listing, in any spelling.** A directory named *without* a
  trailing slash resolves to a non-regular file and is 404; with a trailing
  slash it resolves to that directory's `index.html`.
- **Two independent confinement layers.** The request target is split on `/`
  *before* any percent-decoding and each segment is decoded exactly once; a
  decoded `.` or `..` is 403, and a decoded separator or NUL is 400. Then the
  candidate is canonicalized and checked component-wise against the canonical
  root, which is the layer that turns a symlink pointing out of the bundle into
  a 403. Refusals **reject, never clamp** — a 403 in the log is a real attempt,
  not a normalized request.
- **A symlink cannot relabel a file.** `Content-Type` is computed from the
  canonical path, so `page.html -> data.json` is served as JSON.
- **A fixed extension table, never sniffing**, with
  `application/octet-stream` for anything the table does not name, plus
  `X-Content-Type-Options: nosniff` on every response.
- **`Cache-Control: no-store` on every response.** A regenerated bundle can
  never lose to a cached `sync.js`; you do not need a hard refresh after
  re-running `transync translate`.
- **No keep-alive**, by design: exactly one request per connection.
- **Demo-scale limits.** At most 128 connections in flight, a 15-second
  request-head timeout, an 8 KiB request-head cap. No HTTP Range support (so no
  media seeking) and no TLS — both named out of scope when the server shipped.
- **One documented residual race.** Another local process could replace an
  entry between the `canonicalize` and the `open`. The window is one syscall
  wide and needs local write access inside the served directory; closing it
  entirely (an `openat`/`O_NOFOLLOW` walk) was judged out of scope for a
  loopback demo server and is recorded in
  `crates/transync-cli/src/serve_cmd/conn.rs` rather than left to be
  rediscovered.

## Troubleshooting

| What you see | What it means | What to do |
|---|---|---|
| Exit `2`, `transync serve: … is not a directory; --rendered takes the bundle directory.` (or `cannot serve <path>: …`) | `--rendered` does not exist, is unreadable, or names a file | Point it at the bundle *directory* — `<dir>` for `--html-out`, `<dir>/html` for `--out-dir` |
| Exit `5`, `transync serve: cannot bind <addr>: …` | The address/port could not be bound — usually already in use, or an address this host does not own | Pick another `--port`, or `--port 0` for an OS-assigned one |
| Exit `1` with a clap usage error | `--rendered` was omitted, or `--bind` was not parseable as an IP address | Supply `--rendered`; give `--bind` a literal address, not a hostname |
| `404` on `/` | The served directory has no `index.html` — usually `--rendered <out-dir>` instead of `<out-dir>/html` | Re-point `--rendered` one level down |
| `404` on a path that exists | The target names a directory without a trailing slash, or something that is not a regular file | Add the trailing slash, or name the file |
| `403` | A `.`/`..` segment, or a path that resolved out of the root through a symlink | Expected refusal; move the real file inside the served root |
| `405` | Something sent a method other than `GET`/`HEAD` | Expected refusal; nothing to fix on the server |
| `transync: DOMPurify missing — refusing to mount unsanitized HTML` in the left pane | `purify.min.js` did not load | Almost always the wrong directory — see the `--out-dir`/`html/` distinction above |
| The page hangs, then `transync: timed out loading <path> after 20000 ms` in the left pane | A server accepted the request and stalled (a hung proxy in front, or a wedged process) | `fetchOk` bounds every artifact at 20 s; restart the server, and check nothing else is bound to that port |
| Console warns `the … pane is not the offsetParent of its blocks` | The pane lost its non-static `position` — restyled CSS | Fix the CSS. This is the one warning that means sync will be **misaligned** rather than absent |
| Console warns that a map row's ids disagree, and nothing syncs | Anchors pair by *identical* `data-sync-id`. In an in-band map (same major, not newer than 1.2.0) a row whose `target_block_id` differs from its `source_block_id` is corruption, and the whole map is refused | Regenerate the map, or fix the third-party generator to write the source id into both fields. Only a *forward-minor* map gets the lenient treatment — a warning, then pairing by the source id |

The repeating warning families — duplicate `data-sync-id`, map rows with no DOM
anchor, unknown `sync_role`, non-identity `target_block_id` — cap at five
occurrences each plus a suppressed tally, so a large broken bundle does not
flood the console.

## If you would rather use another static server

Any static file server works — `npx serve`, `nginx`, a CI artifact server —
because the bundle carries no server-side logic (see
`docs/architecture/persistence-and-files.md`). You give up two things
`transync serve` provides: `Cache-Control: no-store`, and the
`application/wasm` content type the
[wasm demo](./build-the-wasm-demo.md) needs.

## Related

- [How to build and run the wasm render+edit demo](./build-the-wasm-demo.md) —
  a separate in-browser rendering path (`web/demo-wasm.html`) that this same
  server can host.
- [Getting started with transync](../../../tutorials/operator/en/getting-started.md) —
  the end-to-end first run this recipe is the last step of.
- [How to diagnose a translation run](../../user/en/diagnose-a-translation-run.md) —
  when the bundle itself looks wrong rather than the serving of it.
- `web/SMOKE.md` — the manual smoke checklist whose prep step is this recipe.
  Its automated successor, `scripts/test-browser.sh`, drives this same server:
  it hands the built binary over as `TRANSYNC_SERVE_BIN`, and
  `web/playwright.config.js` runs
  `serve --rendered <fixture> --bind 127.0.0.1 --port 4319`, falling back to a
  small Node stand-in only when no binary is supplied.
- `docs/architecture/contracts.md` §6 — the authoritative serve contract.
