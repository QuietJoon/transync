// WASM render+edit demo — headless browser evidence (Track C, ADR-0019).
//
// The sibling of scn13.spec.js. Where that suite drives the CLI's
// pre-rendered bundle through `web/index.html`, this one drives
// `web/demo-wasm.html`: the panes are rendered IN THE BROWSER by the Rust
// renderer compiled to wasm, from `source.md` + `out.md` +
// `alignment.json`. Same fixture, same webServer, same worker — the only
// new server fact is the `.wasm` MIME entry, which both servers carry
// (`transync serve`'s `serve_cmd::mime` table and the Node stand-in's).
//
// `scripts/test-browser.sh` assembles the demo's `js/` + `wasm/` +
// `vendor/` layout beside the bundle before the run, so these tests need
// no second server and no second port.
//
// Readiness is `body[data-demo-state="ready"]`, set by wasm-demo.js only
// after both panes are mounted AND the click handler is live — the panes'
// anchors alone would be a premature signal here, because mounting
// precedes wiring.
//
// TRACE: SCN-13
// TRACE: ADR-0019

import { test, expect } from "@playwright/test";
import {
  activeSyncId,
  advanceFrames,
  collectPageErrors,
  constrainPanes,
  maxScrollOf,
  offsetTopOf,
  readFixture,
  setScrollTop,
  SYNC_IDS,
  waitInPagePumped,
} from "./support/harness.js";

const DEMO = "/demo-wasm.html";

// A mid-document heading, as in scn13 — far enough down to force real
// scroll travel in a constrained pane.
const DRIVER_BLOCK = "h2-0010";

// The SCN-14 fixture's block-level image points at `./logo.png`, which the
// bundle deliberately does not ship (the CLI emits HTML, not assets). The
// resulting 404 is a fixture fact, not a demo defect, so it is the one
// console error the clean-load test forgives — matched on the resource URL
// rather than the message text, which carries no URL of its own.
const EXPECTED_MISSING_ASSET = "/logo.png";

/**
 * Console collector that keeps the message *severity* and origin, which
 * harness.js's text-only `collectConsole` drops. The clean-load test needs
 * both: severity to select warnings/errors, origin to forgive the one
 * known-missing fixture asset.
 *
 * Must be attached before `page.goto` — boot-time messages (the wasm-bindgen
 * MIME fallback warning above all) fire during the first navigation.
 */
function collectTypedConsole(page) {
  const messages = [];
  page.on("console", (msg) =>
    messages.push({ type: msg.type(), text: msg.text(), url: msg.location().url }),
  );
  return messages;
}

/** Everything at warn/error severity except the known-missing fixture asset. */
function complaints(messages) {
  return messages.filter(
    (m) => (m.type === "warning" || m.type === "error") && !m.url.endsWith(EXPECTED_MISSING_ASSET),
  );
}

/** Boot the demo and wait for wasm init + render + mount + wiring. */
async function bootDemo(page) {
  await page.goto(DEMO);
  // Generous vs. the config's 5 s expect timeout: this waits on a ~1.7 MB
  // wasm module being fetched, compiled and instantiated on a cold run.
  await page.waitForSelector('body[data-demo-state="ready"]', { timeout: 20_000 });
}

/** The sync ids currently mounted in a pane, in document order. */
function mountedIds(page, sel) {
  return page.evaluate(
    (sel) =>
      Array.from(document.querySelectorAll(`${sel} [data-sync-id]`)).map((el) => el.dataset.syncId),
    sel,
  );
}

/** Count of anchors across BOTH panes — 0 is the fail-closed assertion. */
function anchorCount(page) {
  return page.evaluate(
    () => document.querySelectorAll("#source [data-sync-id], #target [data-sync-id]").length,
  );
}

/** Extract the renderer's `<main>…</main>` fragment from a bundle HTML file. */
function mainFragment(html) {
  const match = /<main[\s\S]*<\/main>/i.exec(html);
  return match ? match[0] : null;
}

test.describe("WASM render+edit demo", () => {
  test("a — boots clean: both panes render locally, no console complaints", async ({ page }) => {
    const messages = collectTypedConsole(page);
    const errors = collectPageErrors(page);

    await bootDemo(page);

    // Both panes carry the full anchor set, rendered by the wasm renderer
    // rather than fetched pre-rendered.
    expect(await mountedIds(page, "#source")).toEqual(SYNC_IDS);
    expect(await mountedIds(page, "#target")).toEqual(SYNC_IDS);

    // Zero warnings and zero errors. This is the standing guard for the
    // whole loading path, and it is deliberately absolute:
    //   - a `.wasm` served with the wrong MIME makes wasm-bindgen's glue
    //     log the `instantiateStreaming` fallback warning (both servers map
    //     `.wasm` -> application/wasm precisely to prevent it, which is how
    //     this assertion also covers `transync serve`'s MIME table);
    //   - any bindgen/module-resolution complaint surfaces here too;
    //   - showFatal() logs console.error, so a silent fail-closed boot
    //     cannot masquerade as a clean one.
    expect(complaints(messages)).toEqual([]);
    expect(errors).toEqual([]);
  });

  test("b — parity: the browser render matches the CLI render block for block", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await bootDemo(page);

    // The reference is the SAME fixture's CLI-rendered bundle, fetched over
    // the same server. Both sides come from one Rust renderer, so the panes
    // must agree with it block for block.
    //
    // The comparison is structural (id sequence, data-block-kind, per-block
    // textContent) rather than an innerHTML byte diff: the demo mounts
    // through DOMPurify, which may normalize markup, and the browser
    // re-serializes what it parsed. Byte-parity of the PRE-sanitize HTML is
    // proven on the host instead — transync-wasm's engine tests assert
    // render_pair/rebuild agree byte-for-byte on the same inputs.
    for (const [paneSel, refFile] of [
      ["#source", "source.html"],
      ["#target", "target.html"],
    ]) {
      const response = await page.request.get(`/${refFile}`);
      expect(response.ok()).toBe(true);
      const fragment = mainFragment(await response.text());
      expect(fragment).not.toBeNull();

      const { reference, live } = await page.evaluate(
        ([fragment, paneSel]) => {
          const digest = (root, sel) =>
            Array.from(root.querySelectorAll(sel)).map((el) => ({
              id: el.dataset.syncId,
              kind: el.dataset.blockKind || null,
              text: el.textContent,
            }));
          const doc = new DOMParser().parseFromString(fragment, "text/html");
          return {
            reference: digest(doc, "[data-sync-id]"),
            live: digest(document, `${paneSel} [data-sync-id]`),
          };
        },
        [fragment, paneSel],
      );

      // Guard against a vacuous pass on two empty lists.
      expect(reference.map((b) => b.id)).toEqual(SYNC_IDS);
      expect(live).toEqual(reference);
    }

    expect(errors).toEqual([]);
  });

  test("c — edit loop: html blocks refuse then degrade honestly, a paragraph re-renders, sync survives", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await bootDemo(page);

    const editor = page.locator("#editor");
    const status = page.locator("#status-strip");

    // html blocks are excluded from the edit model by construction (their
    // payload is an extracted text-segment array, not Markdown, so `rebuild`
    // could not splice an edited one). Clicking one must therefore say so
    // and leave the editor shut — never open an edit that cannot round-trip.
    const htmlRow = page.locator('#target [data-sync-id="html-0018"]');
    await htmlRow.click();
    await expect(status).toHaveText(/html-0018 \(html\) is not editable/);
    await expect(editor).toBeDisabled();

    // Pre-edit it is a live-rendered, translated html block — the state the
    // assertion after the rebuild is measured against.
    await expect(htmlRow).toHaveAttribute("data-fallback", "translated");
    await expect(htmlRow).toHaveJSProperty("tagName", "DIV");

    // A paragraph is editable: the editor opens preloaded with the block's
    // Markdown payload, sliced out of out.md by byte range.
    const editedId = "p-0004";
    const sourceBefore = await page.locator(`#source [data-sync-id="${editedId}"]`).textContent();
    await page.locator(`#target [data-sync-id="${editedId}"]`).click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${editedId}`);
    await expect(editor).toBeEnabled();
    await expect(editor).not.toHaveValue("");

    // Typing drives the debounced wasm rebuild: payloads in, a whole
    // regenerated document + a fresh alignment map back out, both panes
    // re-mounted. Waiting on the status strip's re-render line is the
    // deterministic "past the debounce" signal.
    const replacement = "Edited in the browser through the wasm rebuild path.";
    await editor.fill(replacement);
    await expect(status).toHaveText(/re-rendered \d+ blocks after editing p-0004/);

    // The edit landed in the target...
    await expect(page.locator(`#target [data-sync-id="${editedId}"]`)).toHaveText(replacement);
    // ...and nowhere else: the source pane is regenerated from source.md and
    // must be untouched by a target-side payload edit.
    expect(await page.locator(`#source [data-sync-id="${editedId}"]`).textContent()).toBe(
      sourceBefore,
    );
    // Non-fatal error strip stayed shut — the rebuild was accepted.
    await expect(page.locator("#error-strip")).toBeHidden();

    // Status honesty for html rows after a rebuild. An html block's payload
    // is a text-segment array that the demo cannot reconstruct from the
    // rendered bundle, so the regenerated document carries the block's
    // SOURCE bytes — and the row must say so. `buildEditModel` omits html
    // rows from both maps precisely so `build_alignment_map` synthesizes
    // `fallback_source` here instead of the map's original `translated`
    // riding along over reverted content, unlit by the tint legend.
    // Presentation follows the status: the escaped `<pre>` placeholder.
    await expect(htmlRow).toHaveAttribute("data-fallback", "fallback_source");
    await expect(htmlRow).toHaveAttribute("data-skipped", "html-block");
    await expect(htmlRow).toHaveJSProperty("tagName", "PRE");
    // The row is one row: the source pane presents the same verdict. That is
    // the accepted cost of the honest status, not a second defect.
    await expect(page.locator('#source [data-sync-id="html-0018"]')).toHaveAttribute(
      "data-fallback",
      "fallback_source",
    );

    // Invariant 1: the block-ID correspondence survives a rebuild. Same ids,
    // same order, both panes — this is the sync currency, and an edit that
    // renumbered it would silently break every anchor.
    expect(await mountedIds(page, "#source")).toEqual(SYNC_IDS);
    expect(await mountedIds(page, "#target")).toEqual(SYNC_IDS);

    // And the re-mounted engine still syncs. Panes are constrained only now
    // (the clicks above needed unconstrained, fully visible blocks).
    await constrainPanes(page);
    await advanceFrames(page, 12);
    expect(await maxScrollOf(page, "#source")).toBeGreaterThan(100);
    expect(await maxScrollOf(page, "#target")).toBeGreaterThan(100);

    // Bounce to the top first: re-assigning a scrollTop a pane already holds
    // fires no scroll event, and the follow would never run. Then let the
    // clicks' programmatic-scroll lock (90 ms) decay.
    await setScrollTop(page, "#source", 0);
    await advanceFrames(page, 3);
    await page.waitForTimeout(160);

    // Block-level assertion, not an exact scrollTop one: the edited target
    // no longer has the source's line-for-line layout, so the two panes'
    // scroll offsets legitimately differ. What must hold is that the partner
    // anchor lands at the target's reference line.
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    await waitInPagePumped(
      page,
      `(id) => {
        const pane = document.getElementById("target");
        const el = pane.querySelector('[data-sync-id="' + id + '"]');
        return !!el && Math.abs(el.offsetTop - pane.scrollTop) <= 30;
      }`,
      DRIVER_BLOCK,
      8000,
    );
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);

    expect(errors).toEqual([]);
  });

  test("d — fail-closed: no DOMPurify, an alien map, a non-array blocks, a repeated row and a reversed range on either side of the edit gate all refuse to mount", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // (a) OI-0001 / invariant 7. DOMPurify arrives as a classic script tag
    // before the module, so serving an empty body leaves window.DOMPurify
    // undefined. Rendered HTML is untrusted-source content; with no
    // sanitizer the demo must mount NOTHING rather than mount raw.
    await page.route("**/vendor/purify.min.js", (route) =>
      route.fulfill({ contentType: "text/javascript; charset=utf-8", body: "" }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(/DOMPurify missing/);
    // The gate is reached only after wasm init and the module-vs-demo schema
    // check pass, so this message also witnesses a healthy wasm boot.
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/vendor/purify.min.js");

    // (b) Data-side schema drift. The wasm module's own schema_version()
    // cannot be forged from JS (it is compiled in), so the testable half of
    // the pair is the FETCHED map: an unknown major means the byte ranges
    // this demo slices out.md with are not the ranges the map describes.
    const map = JSON.parse(readFixture("alignment.json"));
    map.schema_version = "9.9.9";
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(map) }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toHaveText(
      /schema_version=9\.9\.9 is not major 1 .* refusing to mount/,
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/alignment.json");

    // (c) R0003-0068. `blocks` is the field every gate below and the edit
    // model iterate, and all of them run BEFORE `render_pair` deserializes it
    // into a `Vec<AlignmentBlock>` and gives the shape its Rust verdict. A
    // truthy non-array (`42`) passes the schema gate above — the only field
    // that one reads is `schema_version` — and then `for (const row of
    // map.blocks || [])` threw "is not iterable" inside `boot()`, which has no
    // try/catch around its gates and no top-level rejection handler. The page
    // sat at `data-demo-state="booting"`: the failure the demo reported by
    // reporting nothing. Refusing by name also stops the other half, where a
    // later `Array.isArray` door reads the same value as ZERO rows and boots a
    // pair of panes with an empty edit model.
    const shaped = JSON.parse(readFixture("alignment.json"));
    shaped.blocks = 42;
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(shaped) }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(
      /alignment map "blocks" is number, not an array of rows . refusing to mount/,
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/alignment.json");

    // (d) R0001-0043/0044, demo side. A repeated `source_block_id` is a
    // shape `AlignmentMap` deserializes fine, and it is exactly the shape
    // that would key the edit model off one row while the DOM carries two
    // anchors and `sync.js` refuses to wire sync. The demo must refuse it
    // whole rather than serve a pair that renders, invites edits, and never
    // scrolls together. (Since R0002-0010 the Rust renderer refuses it too;
    // this gate still runs first, and owns the message.)
    const dup = JSON.parse(readFixture("alignment.json"));
    dup.blocks.push({ ...dup.blocks[1] });
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(dup) }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toHaveText(
      new RegExp(`repeats source_block_id "${dup.blocks[1].source_block_id}" . refusing to mount`),
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/alignment.json");

    // (e) R0002-0054. A well-typed but REVERSED `target_range` survives
    // serde at the Rust boundary — `ByteRange` takes two `usize`s and says
    // nothing about their order. The edit model used to clamp it, handing
    // that row an empty pre-edit payload; since `applyEdit` resubmits every
    // payload on every rebuild, the first edit of ANY block would then blank
    // this one. Refuse the map instead of editing from a coerced slice.
    // (Since R0003-0060 the renderer refuses this row too; this gate still
    // runs first, and owns the edit-model message.)
    const reversed = JSON.parse(readFixture("alignment.json"));
    const victim = reversed.blocks.find(
      (b) =>
        b.block_kind !== "html" &&
        b.block_kind !== "skipped" &&
        b.sync_role !== "non-sync" &&
        b.target_range.end > b.target_range.start,
    );
    expect(victim, "fixture has an editable row with a non-empty range").toBeTruthy();
    victim.target_range = {
      start: victim.target_range.end,
      end: victim.target_range.start,
    };
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(reversed) }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toHaveText(
      new RegExp(`alignment row "${victim.source_block_id}" has an unusable target_range`),
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/alignment.json");

    // (f) R0003-0078. The same corruption on a row this demo does NOT gate.
    // `targetRangeVerdict` checks editable rows only — html rows are not
    // editable — but the renderer's html bypass arm slices their
    // `target_range` all the same, so before R0003-0060 this row mounted as
    // clamped bytes: an empty block under a correct anchor, no refusal
    // anywhere in the stack. The refusal now comes from the renderer, so the
    // message is the Rust one, reaching the strip through the `render_pair`
    // catch rather than through a boot gate.
    const bypass = JSON.parse(readFixture("alignment.json"));
    const htmlRow = bypass.blocks.find(
      (b) => b.block_kind === "html" && b.target_range.end > b.target_range.start,
    );
    expect(htmlRow, "fixture has an html row with a non-empty range").toBeTruthy();
    htmlRow.target_range = {
      start: htmlRow.target_range.end,
      end: htmlRow.target_range.start,
    };
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(bypass) }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toHaveText(
      new RegExp(`render failed:.*unusable target byte range: ${htmlRow.source_block_id} `),
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/alignment.json");

    // Refused, not crashed: all six paths are handled failures, not
    // exceptions.
    expect(errors).toEqual([]);
  });

  test("e — non-fatal: a rejected rebuild reports on the strip and keeps the last good render", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // The demo has two failure presentations and test `d` covers only the
    // fatal one. This is the other: `showError` into `#error-strip`, panes
    // untouched, `data-demo-state` still "ready" — where a thrown `rebuild`
    // lands, i.e. the only way a wasm engine error reaches a user
    // mid-session.
    //
    // The rejection is planted in the DATA, not in the demo. `buildEditModel`
    // keys the edit model by each row's `source_block_id`, so an EXTRA row
    // carrying an id the source document does not have puts a ghost into
    // `payloads` — and `engine::rebuild_impl` rejects the whole call rather
    // than dropping an unknown id on the floor.
    //
    // The extra row is a copy of a real one, so the map still covers every
    // block exactly once and boots normally: the renderer refuses repeated
    // and MISSING rows (R0002-0010, R0002-0011) but accepts a row that names
    // no block, because nothing ever looks it up. This test used to RENAME a
    // row instead, which left its block uncovered — silently absent from both
    // panes back then, a fatal render refusal now, and either way not the
    // mid-session rebuild failure this test is about.
    const GHOST_ID = "zz-9999";
    const map = JSON.parse(readFixture("alignment.json"));
    const donor = map.blocks.find((b) => b.source_block_id === "p-0011");
    expect(donor).toBeTruthy();
    map.blocks.push({ ...donor, source_block_id: GHOST_ID, target_block_id: GHOST_ID });
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(map) }),
    );

    await bootDemo(page);

    // Boot is entirely normal: the ghost row renders nothing (the renderer
    // walks the document's blocks, not the map's rows), so both panes carry
    // the full anchor set. Spelling the baseline out as an id list stops the
    // "unchanged" assertions below from passing on a pane that silently
    // emptied.
    const expectedIds = SYNC_IDS;
    expect(await mountedIds(page, "#source")).toEqual(expectedIds);
    expect(await mountedIds(page, "#target")).toEqual(expectedIds);
    const anchorsBefore = await anchorCount(page);
    expect(anchorsBefore).toBe(expectedIds.length * 2);

    // Edit a block that IS editable — the ghost travels in the model, not in
    // the block under the cursor, so any editable row triggers the rejection.
    const editedId = "p-0004";
    const editedBlock = page.locator(`#target [data-sync-id="${editedId}"]`);
    const textBefore = await editedBlock.textContent();
    await editedBlock.click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${editedId}`);
    const editor = page.locator("#editor");
    await expect(editor).toBeEnabled();

    // A same-topology payload: one paragraph stays one paragraph, so this
    // edit cannot trip `rebuild`'s structure warning — which shares this
    // strip but is announced AFTER a successful rebuild has already
    // re-mounted the panes. Whatever the strip says next is the rejection.
    await editor.fill("This edit never reaches a pane: the model carries a ghost id.");

    const strip = page.locator("#error-strip");
    // This is also the "past the 300 ms debounce" wait, and a deterministic
    // one: the strip is written inside `applyEdit`'s catch, so its appearance
    // IS the rebuild having run and thrown.
    await expect(strip).toBeVisible();
    await expect(strip).toHaveText(
      new RegExp(`rebuild failed:.*absent from the source document: ${GHOST_ID}`),
    );
    // And it is the rejection rather than the post-mount structure warning.
    await expect(strip).not.toHaveText(/changed the document structure/);

    // The failed rebuild mounted nothing: same state, same anchors, same
    // pre-edit text in the block that was edited.
    await expect(page.locator("body")).toHaveAttribute("data-demo-state", "ready");
    expect(await mountedIds(page, "#source")).toEqual(expectedIds);
    expect(await mountedIds(page, "#target")).toEqual(expectedIds);
    expect(await anchorCount(page)).toBe(anchorsBefore);
    expect(await editedBlock.textContent()).toBe(textBefore);
    // The status strip still reads the pre-edit line — a rebuild that landed
    // (warning or not) would have overwritten it with "re-rendered N blocks".
    const statusText = await page.locator("#status-strip").textContent();
    expect(statusText).toMatch(new RegExp(`^editing ${editedId}\\b`));
    expect(statusText).not.toMatch(/re-rendered/);

    await page.unroute("**/alignment.json");

    // Reported, not crashed: a rejected edit is a handled failure.
    expect(errors).toEqual([]);
  });

  test("f — a map the sync engine refuses fails the mount instead of reporting ready", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0002-0016 / R0002-0049. The demo's own gates and `render_pair`'s serde
    // catch nearly everything, but not everything: a row whose
    // `source_block_id` is the EMPTY STRING deserializes fine, is skipped by
    // `duplicateRowVerdict` and by the edit model (both require a non-empty
    // string), and names no block, so the renderer treats it as inert. It is
    // `sync.js`'s row gate that refuses it — after `mountPanes` has already
    // written both panes' innerHTML.
    //
    // `mountSync`'s refusal used to be discarded: the panes showed a real
    // render, boot marked the demo `ready`, and nothing scrolled together.
    // The mount must fail instead, and the last-good render — here, nothing —
    // must not have been traded for it.
    const map = JSON.parse(readFixture("alignment.json"));
    const donor = map.blocks.find((b) => b.source_block_id === "p-0011");
    expect(donor).toBeTruthy();
    map.blocks.push({ ...donor, source_block_id: "", target_block_id: "" });
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(map) }),
    );

    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(/sync engine refused the alignment map/);
    expect(await anchorCount(page)).toBe(0);

    await page.unroute("**/alignment.json");

    // Refused, not crashed.
    expect(errors).toEqual([]);
  });

  test("g — template drift: a missing pane element fails visibly instead of throwing", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0003-0069 / R0003-0090. `mountPanes` reads both panes' `innerHTML` and
    // `scrollTop` before it writes either, so a page missing one of them
    // raised a TypeError on a null reference — out of `boot()` at load, or
    // out of a debounce timer mid-session — with no catch anywhere above it.
    // The page then sat at `data-demo-state="booting"` forever: the one
    // failure this demo presented by presenting nothing.
    //
    // The page is the checked-in `demo-wasm.html`, so this is drift rather
    // than data — which is why the fixture's own markup is what gets broken
    // here, rather than a hand-authored stand-in.
    const drifted = readFixture("demo-wasm.html").replace(
      '<div id="target"',
      "<div data-drifted-target",
    );
    expect(drifted).not.toContain('id="target"');
    await page.route("**/demo-wasm.html", (route) =>
      route.fulfill({ contentType: "text/html; charset=utf-8", body: drifted }),
    );

    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(
      /missing required element #target . refusing to mount/,
    );
    expect(await anchorCount(page)).toBe(0);

    await page.unroute("**/demo-wasm.html");

    // Refused, not crashed — the whole point of a gate that runs before the
    // module is even fetched.
    expect(errors).toEqual([]);
  });

  test("h — a throwing DOMPurify refuses the mount whole: fatal at boot, atomic mid-session", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0003-0070 / R0003-0090. OI-0001's presence check answers for a
    // DOMPurify that is ABSENT (test `d`, case a); this is the other half —
    // one that is present and throws, as a broken build or a hooked instance
    // would. Both `sanitize()` calls used to run inline with their own
    // `innerHTML` write, so a throw on the second left the pair half-swapped
    // and the exception escaping into `boot()` or a debounce timer.

    // (a) At boot there is no last good render to keep, so a sanitizer that
    // cannot sanitize is fatal — the same verdict as one that is missing.
    await page.route("**/vendor/purify.min.js", (route) =>
      route.fulfill({
        contentType: "text/javascript; charset=utf-8",
        body:
          'window.DOMPurify = { version: "throwing-stub", ' +
          'sanitize() { throw new Error("sanitize exploded"); } };',
      }),
    );
    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(
      /sanitizing the rendered HTML failed: sanitize exploded/,
    );
    expect(await anchorCount(page)).toBe(0);
    await page.unroute("**/vendor/purify.min.js");

    // (b) Mid-session the panes hold a render worth keeping, so the same
    // failure takes the non-fatal strip — and it must be ATOMIC. The REAL
    // DOMPurify is served, wrapped so it works for the two calls boot needs
    // and throws on the next mount's first call: exactly the ordering that
    // used to replace the source pane and then abandon the target.
    await page.route("**/vendor/purify.min.js", async (route) => {
      const upstream = await route.fetch();
      const body = await upstream.text();
      await route.fulfill({
        contentType: "text/javascript; charset=utf-8",
        body:
          body +
          "\n;(() => {" +
          "  const real = window.DOMPurify.sanitize.bind(window.DOMPurify);" +
          "  let calls = 0;" +
          "  window.DOMPurify.sanitize = (html) => {" +
          "    calls += 1;" +
          "    if (calls > 2) throw new Error('sanitize exploded');" +
          "    return real(html);" +
          "  };" +
          "})();",
      });
    });

    await bootDemo(page);

    const editedId = "p-0004";
    // A marker on a LIVE node inside the source pane. An `innerHTML` write
    // re-parses the pane and the marker dies with the old DOM, so its
    // survival is the partial replacement observed directly rather than
    // inferred from a pane whose text would look the same either way.
    await page.evaluate((id) => {
      document.querySelector(`#source [data-sync-id="${id}"]`).dataset.probe = "kept";
    }, editedId);

    const editedBlock = page.locator(`#target [data-sync-id="${editedId}"]`);
    const textBefore = await editedBlock.textContent();
    await editedBlock.click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${editedId}`);
    await page.locator("#editor").fill("A rebuild whose sanitizer explodes must change nothing.");

    const strip = page.locator("#error-strip");
    await expect(strip).toBeVisible();
    await expect(strip).toHaveText(
      /sanitizing the rendered HTML failed: sanitize exploded, keeping the last good render/,
    );

    // Nothing was written: the marker survived, both panes still carry the
    // whole anchor set, the edited block still shows its pre-edit text, and
    // the session is still live.
    expect(
      await page.evaluate(
        (id) => document.querySelector(`#source [data-sync-id="${id}"]`)?.dataset.probe ?? null,
        editedId,
      ),
    ).toBe("kept");
    expect(await mountedIds(page, "#source")).toEqual(SYNC_IDS);
    expect(await mountedIds(page, "#target")).toEqual(SYNC_IDS);
    expect(await editedBlock.textContent()).toBe(textBefore);
    await expect(page.locator("body")).toHaveAttribute("data-demo-state", "ready");

    await page.unroute("**/vendor/purify.min.js");

    // Reported, not crashed — on both paths.
    expect(errors).toEqual([]);
  });

  test("i — a rejected edit stays out of the model: a later edit of another block still lands", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await bootDemo(page);

    const editor = page.locator("#editor");
    const status = page.locator("#status-strip");
    const strip = page.locator("#error-strip");

    // R0003-0003 / R0003-0086. Every rebuild resubmits the WHOLE payload map,
    // so a rejected payload that entered the model re-failed the rebuild of
    // every later edit — of any block — until the user happened to return to
    // the block that caused it. Test `e` proves the panes survive a rejection;
    // this proves the model does, which nothing observed before: the DOM is
    // identical either way and only the NEXT edit tells the two apart.
    //
    // The rejection is planted in the PAYLOAD rather than in the map (test `e`
    // does the map), because that is the only injection a later edit can
    // outlive: `parser::intake` refuses the regenerated target pane past
    // `MAX_BLOCK_NESTING_DEPTH`, so a payload of deeper blockquotes is a hard
    // `rebuild` error caused purely by what was typed.
    const REJECTED = `${"> ".repeat(200)}too deep for the intake guard`;
    const firstId = "p-0004";
    const firstBlock = page.locator(`#target [data-sync-id="${firstId}"]`);
    const firstTextBefore = await firstBlock.textContent();
    await firstBlock.click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${firstId}`);
    await editor.fill(REJECTED);

    await expect(strip).toBeVisible();
    await expect(strip).toHaveText(/rebuild failed:.*block nesting too deep/);
    expect(await firstBlock.textContent()).toBe(firstTextBefore);

    // Now edit a DIFFERENT block with an unimpeachable payload. With the
    // rejected payload still in the model this rebuild fails too, and the
    // status strip never advances past the click.
    const secondId = "p-0011";
    const ACCEPTED = "A later edit of another block must still land.";
    await page.locator(`#target [data-sync-id="${secondId}"]`).click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${secondId}`);
    await editor.fill(ACCEPTED);

    await expect(status).toHaveText(
      new RegExp(`re-rendered \\d+ blocks after editing ${secondId}`),
    );
    await expect(page.locator(`#target [data-sync-id="${secondId}"]`)).toHaveText(ACCEPTED);
    // And the rejected payload did not travel with it: its block re-rendered
    // from the last ACCEPTED model, not from what was refused.
    expect(await page.locator(`#target [data-sync-id="${firstId}"]`).textContent()).toBe(
      firstTextBefore,
    );
    // A clean rebuild clears the strip, so the earlier rejection is over.
    await expect(strip).toBeHidden();

    // The other half of the contract. Keeping the model clean must not cost
    // the user their text: re-opening the rejected block re-primes the editor
    // with what they typed, not with the payload that was kept.
    await page.locator(`#target [data-sync-id="${firstId}"]`).click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${firstId}`);
    await expect(editor).toHaveValue(REJECTED);

    expect(errors).toEqual([]);
  });

  test("j — a stalled wasm module fetch is bounded and says so, instead of booting forever", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0004-0090. `init()` left to build its own request fetched the ~1.7 MB
    // module with no `AbortSignal` and no deadline, so a server that accepted
    // the connection and then went quiet pinned the page at
    // `data-demo-state="booting"` — nothing on screen, nothing on the console.
    // That is the same hang R0002-0052 closed for the three artifact fetches,
    // one request wider, and test `g` above is the proof that "stuck at
    // booting" is the one failure this demo used to present by presenting
    // nothing.
    //
    // The bound is a whole minute in production, which no suite should sit
    // through, so the SERVED module source is rewritten to a short one — the
    // same route-the-real-file technique test `g` uses on `demo-wasm.html`.
    // The replacement is asserted to have landed, so a renamed constant fails
    // this test rather than quietly disarming it.
    const SHORT_MS = 400;
    const impatient = readFixture("js/wasm-demo.js").replace(
      "const WASM_INIT_TIMEOUT_MS = 60000;",
      `const WASM_INIT_TIMEOUT_MS = ${SHORT_MS};`,
    );
    expect(impatient).toContain(`const WASM_INIT_TIMEOUT_MS = ${SHORT_MS};`);
    await page.route("**/js/wasm-demo.js", (route) =>
      route.fulfill({
        contentType: "text/javascript; charset=utf-8",
        body: impatient,
      }),
    );

    // A route that never answers — the server accepted and went silent.
    await page.route("**/transync_wasm_bg.wasm", () => {});

    await page.goto(DEMO);
    await page.waitForSelector('body[data-demo-state="fatal"]', { timeout: 20_000 });
    await expect(page.locator("#error-strip")).toBeVisible();
    await expect(page.locator("#error-strip")).toHaveText(
      new RegExp(
        `wasm init failed: timed out loading .*transync_wasm_bg\\.wasm after ${SHORT_MS} ms`,
      ),
    );
    // Fail-closed: nothing rendered, because nothing could be.
    expect(await anchorCount(page)).toBe(0);

    await page.unroute("**/transync_wasm_bg.wasm");
    await page.unroute("**/js/wasm-demo.js");

    // Refused, not crashed — and the abort that ends the fetch is caught by
    // the same handler, so it never escapes as an unhandled rejection.
    expect(errors).toEqual([]);
  });

  test("k — a map row keyed `__proto__` enters the edit model like any other alien id", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0004-0088. This is test `e` with one character class changed: the same
    // ghost row, planted the same way, differing only in the id it carries.
    // `buildEditModel` used to key `payloads` / `statuses` off plain `{}`
    // literals, and `__proto__` is an ACCESSOR on `Object.prototype` — so
    // `payloads["__proto__"] = "…"` was a silent no-op and this one id, alone
    // among every id a fetched map can carry, vanished from the model instead
    // of reaching the engine's `reject_unknown_ids` gate. The rebuild below
    // then SUCCEEDED, which is the demo quietly disagreeing with itself: every
    // other alien id (test `e`'s `zz-9999`) is refused by name.
    //
    // Rust emits `kind-NNNN`, so this is bundle-corruption robustness rather
    // than a live hole — the same class as the demo's other map gates, and
    // pinned here for the same reason they are.
    const GHOST_ID = "__proto__";
    const map = JSON.parse(readFixture("alignment.json"));
    const donor = map.blocks.find((b) => b.source_block_id === "p-0011");
    expect(donor).toBeTruthy();
    map.blocks.push({ ...donor, source_block_id: GHOST_ID, target_block_id: GHOST_ID });
    await page.route("**/alignment.json", (route) =>
      route.fulfill({ contentType: "application/json", body: JSON.stringify(map) }),
    );

    await bootDemo(page);

    // Boot is normal: the ghost row names no block, so the renderer never
    // looks it up (render.rs `PaneCtx::new` — rows naming absent blocks are
    // inert) and both panes carry the full anchor set.
    expect(await mountedIds(page, "#source")).toEqual(SYNC_IDS);
    expect(await mountedIds(page, "#target")).toEqual(SYNC_IDS);

    const editedId = "p-0004";
    const editedBlock = page.locator(`#target [data-sync-id="${editedId}"]`);
    const textBefore = await editedBlock.textContent();
    await editedBlock.click();
    await expect(page.locator("#editor-label")).toHaveText(`editing ${editedId}`);
    await page
      .locator("#editor")
      .fill("A same-shape paragraph, so only the ghost id can reject this.");

    const strip = page.locator("#error-strip");
    await expect(strip).toBeVisible();
    await expect(strip).toHaveText(
      new RegExp(`rebuild failed:.*absent from the source document: ${GHOST_ID}`),
    );
    // The rejection, not the post-mount structure warning that shares this
    // strip — and nothing mounted, so the edited block still reads as it did.
    await expect(strip).not.toHaveText(/changed the document structure/);
    expect(await editedBlock.textContent()).toBe(textBefore);
    const statusText = await page.locator("#status-strip").textContent();
    expect(statusText).not.toMatch(/re-rendered/);

    await page.unroute("**/alignment.json");
    expect(errors).toEqual([]);
  });

  test("l — a control inside an editable block keeps its own click instead of opening the editor", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await bootDemo(page);

    const editor = page.locator("#editor");
    const label = page.locator("#editor-label");
    const editedId = "p-0004";
    const row = page.locator(`#target [data-sync-id="${editedId}"]`);

    // R0011-0041. The pane's click handler resolved the nearest anchor for
    // EVERY descendant click, so a click on a control inside a translated
    // block did two things at once: the control acted, and the editor opened
    // over it. A translated block can legitimately carry a link, a form
    // control, or the `<summary>` of a `<details>` whose open state sync.js
    // mirrors — so the control is planted here rather than fished out of the
    // fixture, which pins the guard rather than one document's markup.
    await expect(editor).toBeDisabled();
    const clicked = await row.evaluate((el) => {
      const button = el.ownerDocument.createElement("button");
      button.type = "button";
      button.id = "planted-control";
      button.textContent = "act";
      button.addEventListener("click", () => {
        button.dataset.acted = "yes";
      });
      el.appendChild(button);
      return el.dataset.syncId;
    });
    expect(clicked).toBe(editedId);

    await page.locator("#planted-control").click();
    await expect(page.locator("#planted-control")).toHaveAttribute("data-acted", "yes");
    await expect(editor).toBeDisabled();
    await expect(label).not.toHaveText(`editing ${editedId}`);

    // And the guard is a guard, not a refusal: the block around the control
    // still opens when the click is the block's own.
    await row.click();
    await expect(label).toHaveText(`editing ${editedId}`);
    await expect(editor).toBeEnabled();

    expect(errors).toEqual([]);
  });

  test("m — the editor is reachable by keyboard, and only editable blocks are tab stops", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await bootDemo(page);

    const editor = page.locator("#editor");
    const label = page.locator("#editor-label");
    const editedId = "p-0004";

    // R0011-0042. `openEditor` used to be reachable from `click` alone, so
    // the demo's one human-edit workflow had no keyboard path at all.
    const row = page.locator(`#target [data-sync-id="${editedId}"]`);
    await expect(row).toHaveAttribute("tabindex", "0");
    await row.focus();
    await page.keyboard.press("Enter");
    await expect(label).toHaveText(`editing ${editedId}`);
    await expect(editor).toBeEnabled();
    await expect(editor).not.toHaveValue("");

    // A tab stop on a block the click path answers with "is not editable"
    // would lead nowhere, so the html block is deliberately not one.
    await expect(page.locator('#target [data-sync-id="html-0018"]')).not.toHaveAttribute(
      "tabindex",
      "0",
    );

    expect(errors).toEqual([]);
  });
});
