---
type: How-To Guide
title: How to build and run the wasm render+edit demo
description: Build the transync-wasm module with scripts/build-wasm.sh, assemble the demo directory beside a translated document, serve it with `transync serve`, and read the demo's own boot verdict.
tags: [wasm, demo, ADR-0019, DCR-0020]
audience: operator
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: adr-0019, resource: docs/decisions/0019-wasm-demo-layer.md }
  - { id: dcr-0020, resource: docs/project/design-change-records/DCR-0020-track-c-wasm-demo.md }
  - { id: build-wasm, resource: scripts/build-wasm.sh }
  - { id: workdir-guard, resource: scripts/lib/workdir-guard.sh }
  - { id: test-browser, resource: scripts/test-browser.sh }
  - { id: playwright-config, resource: web/playwright.config.js }
  - { id: demo-html, resource: web/demo-wasm.html }
  - { id: wasm-demo-js, resource: web/js/wasm-demo.js }
  - { id: sync-engine, resource: web/js/sync.js }
  - { id: wasm-engine, resource: crates/transync-wasm/src/engine.rs }
  - { id: renderer, resource: crates/transync-syntax/src/render.rs }
  - { id: align-version, resource: crates/transync-syntax/src/align.rs }
  - { id: serve-mime, resource: crates/transync-cli/src/serve_cmd/mime.rs }
  - { id: gitignore, resource: .gitignore }
synced_hash: 24b64d02ec987e4a351fd3e8c2c1998a8a53f07abd7f06db97500a56a7b1cb40
---

# How to build and run the wasm render+edit demo

`web/demo-wasm.html` renders both panes **in the browser**, using the same Rust
renderer as the CLI compiled to WebAssembly (ADR-0019), and lets you edit a
translated block and watch it re-render live. It is a separate demo from the
CLI's `--html-out` bundle — the CLI bundle deliberately ships no wasm — so it
needs its own build step and its own file layout.

## Preconditions

- `wasm-pack` on `PATH`.
- A `wasm-opt` (from [binaryen](https://github.com/WebAssembly/binaryen))
  **>= 121** on `PATH`. wasm-pack's own cached binaryen is **not** sufficient —
  it ships 117, which rejects current rustc's bulk-memory output. The script
  resolves `wasm-opt` once by absolute path precisely so the invocation cannot
  drift onto that cached copy.
- `rustup target add wasm32-unknown-unknown`, if you haven't already.
- A translated document — `out.md` and `alignment.json` from a prior
  `transync translate` run (see
  [how to translate a document with the CLI](../../user/en/translate-a-document-with-the-cli.md)) —
  plus the original source file that produced them.

Both tool prerequisites fail loudly with an install hint rather than silently
skipping. One failure mode is worth knowing in advance, because it looks like a
version problem and is not: the check parses the binaryen banner by anchoring
on the `version <n>` token (a vendor banner leading with a year used to be
misread as the version). A **wrapped or vendored `wasm-opt`** whose `--version`
carries no such token is rejected — even when it is new enough — with

```
[build-wasm] could not parse a 'version <n>' from '<path> --version' — need binaryen >= 121. It printed:
```

followed by that tool's raw output, verbatim, so you can see what it actually
said. The fix is to put a stock binaryen `wasm-opt` first on `PATH`; there is no
override that skips the check.

## 1. Build the module

```bash
./scripts/build-wasm.sh
```

**Nothing is installed until it has passed every gate.** The wasm-pack build,
the `wasm-opt -Oz` pass, the size measurement and both budget checks all happen
in a per-run staging directory; `web/wasm/` is only then replaced.

What runs, in order:

1. `wasm-pack build crates/transync-wasm --target web --profile wasm-release
   --no-opt --no-pack --out-dir "$STAGING"`, where `$STAGING` is a
   `mktemp -d` subdirectory of the staging root. That root is
   `TRANSYNC_WASM_STAGING`, defaulting to
   `/Volumes/Temp/claude/transync-wasm-pkg` and falling back to
   `${TMPDIR:-/tmp}/transync-wasm-pkg` when the default's parent directory does
   not exist; an override is vetted by the shared deletion guard
   (`scripts/lib/workdir-guard.sh`). The per-run subdirectory is what lets two
   concurrent builds coexist. `--no-opt` is mandatory, not merely preferable:
   wasm-pack ignores its optimization key for a user-defined profile like
   `wasm-release`.
2. `wasm-opt -Oz` invoked directly, with the bulk-memory / reference-types /
   nontrapping-float-to-int / sign-ext / mutable-globals proposal flags current
   rustc output needs.
3. Size measurement against both ceilings — **1,840,000 B raw** and
   **760,000 B gzip** (owner-approved 2026-08-05, lowered when
   `transync-wasm`'s `rlib` crate-type was dropped). Every run echoes
   `[build-wasm] size: raw=<n>B gzip=<n>B (budget: raw<=1840000B
   gzip<=760000B)`. The script's own comment records the current measurement as
   1,639,520 B raw / 671,795 B gzip, with the ceilings sitting about 12 % (raw)
   and 13 % (gzip) above it. A breach is documented as a design signal, not a
   number to bump.
4. Publication, as two renames under a `mkdir` lock
   (`web/.wasm.publish.lock`): the verified pair is copied into a *sibling* of
   `web/wasm/` and re-checked there, the existing `web/wasm/` is moved aside to
   `web/.wasm.backup.*`, and that sibling becomes `web/wasm/`. The sibling is
   deliberately chmod'ed to the mode your umask would have given a normal
   directory, because it becomes the directory a static server reads the module
   out of.

Three consequences you can rely on:

- **A failed build leaves the previously good module in place.** Every failure
  path — wasm-pack emitting no glue or module, a raw or gzip breach, an empty
  staged file, an unresolvable or too-old `wasm-opt`, an unparseable binaryen
  banner, a publish lock held too long — exits non-zero *before* the swap, so
  `web/wasm/` still holds the last module that passed. The two size failures say
  so in the message: `… $OUT_DIR left untouched.`
- **Glue and module always land as one version-coupled pair.** No browser tab
  and no parallel Playwright run can observe a mixed pair; a reader that looks
  into the window between the two renames finds nothing at all.
- **`web/wasm/` afterwards holds exactly the set this build emitted** — an
  artifact a past build wrote under a name this one no longer produces does not
  linger.

On success: `[build-wasm] OK — module + glue landed in <repo>/web/wasm`, holding
`transync_wasm.js` (the glue) and `transync_wasm_bg.wasm` (the module). Both are
gitignored, as are the swap siblings, via `web/wasm/` and `web/.wasm.*`.

## 2. Assemble the demo directory

The page fetches three files by relative path — `source.md`, `out.md`,
`alignment.json` — sitting next to it, loads `vendor/purify.min.js` and
`./js/wasm-demo.js`, and that module imports the glue as
`../wasm/transync_wasm.js` and the sync engine as `./sync.js`. Lay a directory
out like this:

```
demo/
├── demo-wasm.html          # web/demo-wasm.html
├── source.md                # the ORIGINAL document you translated
├── out.md                   # the translated Markdown (--output, or --out-dir's out.md)
├── alignment.json           # the alignment map (--map, or --out-dir's alignment.json)
├── js/
│   ├── wasm-demo.js         # web/js/wasm-demo.js
│   └── sync.js              # web/js/sync.js
├── vendor/
│   └── purify.min.js        # web/vendor/purify.min.js
└── wasm/
    ├── transync_wasm.js      # web/wasm/transync_wasm.js  (from step 1)
    └── transync_wasm_bg.wasm # web/wasm/transync_wasm_bg.wasm (from step 1)
```

```bash
mkdir -p demo/js demo/vendor demo/wasm
cp web/demo-wasm.html demo/
cp web/js/wasm-demo.js web/js/sync.js demo/js/
cp web/vendor/purify.min.js demo/vendor/
cp web/wasm/transync_wasm.js web/wasm/transync_wasm_bg.wasm demo/wasm/
cp <your-source-document>.md demo/source.md
cp <your-translated-output>.md demo/out.md
cp <your-alignment-map>.json demo/alignment.json
```

The glue and module must stay siblings in one directory — the wasm-bindgen glue
resolves the `.wasm` file relative to its own URL. This is the same shape
`scripts/test-browser.sh` mirrors into its fixture directory.

## 3. Serve and open it

```bash
transync serve --rendered demo
```

Open `http://127.0.0.1:7470/demo-wasm.html`. Use `transync serve` rather than a
general-purpose static server: its content-type table carries
`application/wasm` for `.wasm` and `text/javascript` for `.js`/`.mjs`
specifically so it can host this leg, and `WebAssembly.instantiateStreaming`
refuses to compile a module served as anything else. See
[how to serve a translated demo bundle](./serve-the-demo-bundle.md) for the
server's flags, refusals and limits.

## 4. Read the boot verdict

The demo stamps its own state on `document.body.dataset.demoState`: `booting`
at entry, `ready` once both panes are mounted and the click handler is live,
`fatal` when any gate refuses. That attribute is the readiness signal the
headless suite uses, and the fastest way to tell "still loading the module"
from "refused". A fatal message also replaces the source pane's content, blanks
the target, fills the error strip, and is logged with `console.error`.

The gates run in this order, and each is fail-closed:

1. the page's `#source`, `#target` and `#editor` elements are present — checked
   *first*, before fetching a module of well over a megabyte that would have
   nowhere to render;
2. the wasm module initializes;
3. the module's schema version matches the demo's;
4. `window.DOMPurify` is present;
5. all three artifacts fetch within the time bound;
6. `alignment.json` parses as JSON;
7. the map's `schema_version` passes its own verdict;
8. `blocks` is an array if present at all;
9. no duplicate rows;
10. every editable row's `target_range` is sliceable;
11. `render_pair` succeeds;
12. DOMPurify does not throw while sanitizing;
13. the sync engine accepts the map.

The last two behave differently once a render already exists: at boot a
throwing DOMPurify or a refused map is fatal, but mid-session — during an edit
rebuild — both are reported on the error strip and the last good render stays
on screen.

Two of these are commonly misread, and one of the misreadings sends you to the
wrong file:

- **`schema mismatch: wasm <x> vs demo <y>`** compares the *module's*
  `schema_version()` against the demo JS's own compiled-in constant
  (`KNOWN_SCHEMA = "1.3.0"` in `web/js/wasm-demo.js`). **The fetched alignment
  map is not involved in this check at all**, so regenerating the map cannot
  fix it. The remedy is rebuilding the module (step 1) against the current
  tree, or updating `web/js/wasm-demo.js` — whichever of the two is behind.
- The **fetched map** is judged separately, by a different gate with different
  outcomes: a `schema_version` that is not major 1 is fatal (`… is not major 1
  (demo speaks 1.3.0) — refusing to mount`), while a same-major *newer* version
  is only a `console.warn` (`proceeding, but rendering may be incomplete`) and
  the demo continues.

Two more failures worth naming:

- `render failed: alignment map has an unusable target byte range: …` — the
  Rust renderer refuses a byte range it cannot slice
  (`RenderError::UnusableRange`) for **every** row it slices, including the
  `html` and skipped rows the demo's own editable-row gate never checks.
  Reversed, out-of-bounds and mid-UTF-8 ranges are refused, not clamped.
  Regenerate the alignment map.
- `timed out loading <path> after 20000 ms` — `fetchOk` bounds each artifact,
  and one failure aborts its two siblings through a shared `AbortController`.
  A server that accepts and then stalls can no longer pin the demo at
  `booting`.

## 5. Try editing

Click an editable block in the target (translated) pane. Its Markdown loads into
the editor panel at the bottom. Edit it — after a short debounce the whole loop
reruns in the browser (regenerate → re-align → re-render) and the pane updates.

Editing is scoped to the translated side's block *payloads* only; the block set
itself comes from parsing the source document, which the demo never edits, so no
edit can create or remove a sync anchor.

A **rejected** edit does not enter the model. The submission is a staged clone
of the payload map, and `state.payloads` only advances past a rebuild *and* the
mount that followed it; a failed rebuild reports on the non-fatal error strip
(`transync: rebuild failed: …`) and keeps the last good render on screen — the
session continues. Your typed text is kept in `state.drafts`, keyed by block id,
and re-primed into the editor when you click that block again, so clicking away
and back does not lose it.

One advisory case: if a rebuild succeeds but the regenerated document no longer
carries the source's top-level block count, the panes still mount and the same
error strip carries a `structure_warning`. The render you are looking at is the
degraded one — the reshaped block shows its raw Markdown, and blocks after it
can show shifted content under correct anchors. Undoing the change is the whole
recovery; the strip clears on the next clean rebuild.

## When the build fails

- **`$OUT_DIR left untouched`** on a size breach — investigate what grew;
  `web/wasm/` still holds the last good module, so the demo keeps working while
  you do.
- **The publish lock.** `web/.wasm.publish.lock` is a lock *directory* held only
  for the two renames — milliseconds. A build waits up to 30 s for it, then
  fails with an instruction to remove that directory by hand once no build is
  running. That is the script's only manual-recovery step, and it means a build
  was killed mid-swap.
- **Leftover siblings.** A `kill -9` can leave a `web/.wasm.staging.*` or
  `web/.wasm.backup.*` behind. They are gitignored under `web/.wasm.*` and safe
  to delete when no build is running. The EXIT trap normally restores a
  moved-aside module from `web/.wasm.backup.*` rather than leaving the demo with
  no module at all.

## Verify it automatically

`scripts/test-browser.sh` runs `scripts/build-wasm.sh` as part of its own run,
assembles this demo leg on top of a generated CLI bundle inside its scratch
fixture directory, and drives it headless. The runner is the authority on coverage — it runs
every spec under `web/tests/`: `web/tests/scn13.spec.js`,
`web/tests/engine.spec.js`, `web/tests/wasm.spec.js` and
`web/tests/scn16.spec.js`. Playwright serves the fixture on loopback
`127.0.0.1:4319` with `transync serve` when the script hands it a binary via
`TRANSYNC_SERVE_BIN`, and with a small Node stand-in otherwise.

## Related

- [How to translate a document with the CLI](../../user/en/translate-a-document-with-the-cli.md) —
  produces the `out.md` / `alignment.json` pair this demo needs.
- [How to serve a translated demo bundle](./serve-the-demo-bundle.md) —
  the CLI's own `--html-out` bundle, which needs no wasm build at all, and the
  server used in step 3.
- [Alignment-map schema reference](../../../reference/developer/en/alignment-map-schema.md) —
  what the demo's map gates are checking.
- `docs/decisions/0019-wasm-demo-layer.md` — why this is a separate web-only
  demo and not part of the CLI bundle.
