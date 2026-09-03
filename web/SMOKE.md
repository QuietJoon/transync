# SCN-13 — JS demo smoke

## Primary evidence: automated headless suite

The browser-side sync engine is verified by an automated, headless
Playwright suite — the primary SCN-13 evidence (advances OI-0023). Run
it from a clean checkout with:

```
scripts/test-browser.sh
```

That builds the stub CLI, regenerates a real HTML bundle from the SCN-14
fixture, serves it with `transync serve` on loopback, builds and
assembles the wasm demo beside it, and drives the lot in headless
Chromium. The runner is the authority on what is covered: it runs every
spec under `web/tests/`, each taking a different angle on the same
browser layer.

- `web/tests/scn13.spec.js` — the shipped CLI bundle driven through
  `web/index.html`, the way an operator meets it: dual-pane sync, the
  reflow and takeover behaviours, and the alignment `schema_version`
  gate.
- `web/tests/engine.spec.js` — `sync.js`'s mount contract driven
  directly over a bare two-pane rig: the refusals and the reflow
  recompute that a bundle page cannot stage.
- `web/tests/scn16.spec.js` — the SCN-16 HTML-run bundle (an
  `--input-format html` run over the SCN-16 fixture, published by the
  runner at `scn16/` inside the served dir) driven through the shipped
  shell: bidirectional block-id sync over HTML-derived anchors, the
  `<title>` translated but rendered by browser chrome rather than either
  pane, and a console clean of warnings.
- `web/tests/wasm.spec.js` — `web/demo-wasm.html`, whose panes the Rust
  renderer compiled to wasm produces in the browser (ADR-0019).

Read the spec files themselves for the current coverage. This page names
them and states no count on purpose: a count rots the next time a spec
gains a case, and it is the spec files, not their number, that tell a
reader where to look.

Suite plumbing lives under `web/` (`package.json`, `playwright.config.js`,
`tests/`); `pnpm` is the package manager (workspace convention).

Run the suite through `scripts/test-browser.sh`, not through a bare
`pnpm exec playwright test`. The script regenerates the bundle first; the bare
command validates whatever the last run left behind. Since ti `ed2e73` the
config refuses a bundle older than `crates/`, `web/js`, `web/vendor`,
`web/wasm` or the shell HTML — `web/tests` is excluded, so editing a spec and
re-running bare still works, which is what that loop is for. Set
`TRANSYNC_ALLOW_STALE_FIXTURE=1` to downgrade the refusal to a warning.

## Fallback: manual smoke checklist

The manual run below remains a fallback when a browser-capable
environment for the automated suite is unavailable.

## Prep

1. From a clean checkout: `cargo run -p transync-cli --features test-stub-provider --quiet -- translate \
     --input crates/transync/tests/fixtures/scn-14-full.md \
     --output /tmp/transync/out.md \
     --map /tmp/transync/out.json \
     --html-out /tmp/transync/html \
     --target-language ko`
2. `cargo run -p transync-cli --features test-stub-provider --quiet -- serve --rendered /tmp/transync/html`
   (loopback `127.0.0.1:7470`; Ctrl-C stops it. The feature flag only
   keeps step 1's build warm — `serve` makes no provider call. Any other
   static server pointed at the same directory works too; the bundle is
   self-contained.)
3. Open `http://127.0.0.1:7470/` in any modern browser. The page should mount two
   panes side-by-side with the SCN-14 fixture rendered in both.

## Checklist

- [ ] **Anchor presence.** `document.querySelectorAll("[data-sync-id]")`
      returns the same set in both panes (one per sync-relevant block).
- [ ] **Schema-version log.** With the DevTools console showing
      *Verbose* messages (the success line is a `console.debug`, hidden
      at the default level), the page logs
      `transync: alignment map loaded (schema_version=<x.y.z>)` and the
      panes sync. What the engine actually gates on is the **major**
      version, so do not compare the whole string: any `1.y.z` mounts.
      A map whose minor/patch is newer than the engine's own logs
      `... is newer than this engine ...` as a warning and still syncs —
      also a pass. The only failure is a
      `rejecting alignment map with unknown major schema_version=...`
      warning, with no sync at all. The version the bundle writes is
      `ALIGNMENT_SCHEMA_VERSION` (`crates/transync-syntax/src/align.rs`);
      the version the engine speaks is `KNOWN_SCHEMA` (`web/js/sync.js`);
      the two are kept in lockstep by
      `crates/transync-cli/tests/sync_js_drift.rs`.
- [ ] **Forward sync.** Scroll the source pane down to a heading and
      observe the target pane glide to the matching heading (the smooth
      follow settles within ~250 ms).
- [ ] **Reverse sync.** Same as above with the panes swapped.
- [ ] **No oscillation.** A continuous user scroll over a few seconds in
      one pane does not cause the other pane to jitter back into the
      first; the partner-pane scroll is one-directional during the
      programmatic-scroll lock window.
- [ ] **Resize reflow.** Scroll one pane to a mid-document heading first,
      so the engine has a driving pane, then resize the browser window
      and **touch neither pane's scroll afterwards**. The follower
      re-aligns on that heading by itself, with the same smooth glide as
      a normal follow but no scroll of yours behind it: the
      `ResizeObserver` in `web/js/sync.js` re-collects both anchor sets
      and re-runs the last driving pane's scroll handler. If the panes
      only come back together once you scroll again, the step **fails** —
      recovering on the next scroll is the pre-reflow behaviour, not the
      shipped one.
- [ ] **Fallback styling.** Any block carrying
      `data-fallback="fallback_source"` is highlighted (the demo shell's
      CSS rule applies — visible as a yellow tint).
- [ ] **No console errors.** No uncaught exceptions during the smoke.

## Known limitations (post-MVP)

- The active-block heuristic picks the block whose bounds straddle a
  reference line 4 px below the pane top (tracking the user's scroll
  progress within that block) and falls back to the topmost visible
  block; there is no hysteresis state machine, so rapid flicks across
  multiple blocks may briefly select a neighboring block.
- Partner-pane scrolling is a per-frame lerp toward the computed
  scrollTop (proportional offset within the active block): each frame
  closes ~20 % of the remaining gap and arms a 90 ms
  programmatic-scroll lock, and the loop snaps and stops once the gap
  drops below the 0.5 px settle threshold. Alternative easing /
  alignment policies are deferred.
- DOMPurify 3.2.6 is vendored (`web/vendor/purify.min.js`; CLI bundles
  ship their own copy) and both shells sanitize fetched fragments before
  mounting, failing closed with an on-page error if DOMPurify is absent
  (OI-0001). Checklist: after loading either demo, confirm the panes
  render (sanitizer present) and that `data-sync-id` attributes survive
  in the mounted DOM (DOMPurify keeps `data-*` by default).
