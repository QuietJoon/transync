// SCN-16 — the HTML-run bundle through the shipped shell, headless.
//
// Wave 7 of ti 490d97 (spec §12): the browser gate. scn13.spec.js drives
// the SCN-14 Markdown bundle; this file drives the SCN-16 HTML-run bundle
// that scripts/test-browser.sh publishes at <fixture>/scn16/ (the wave-6
// leg) through the SAME shipped shell — fetch, DOMPurify fail-closed
// mount, sync.js — the way an operator meets an HTML run's --html-out.
// engine.spec.js test n drives these pane files WITHOUT the shell; this
// spec is the shell-driven half that plan deliberately left to wave 7.
//
// The acceptance line, verbatim: bidirectional sync by block id, the
// title absent from panes, zero console warnings.
//
// One presentation this spec uniquely proves: the fixture's custom
// element (<x-note>) is unknown to spec §4's tables, so wave 6's
// 2026-08-21 ruling puts its anchor on a transparent <div> wrapper —
// self-injection would die in DOMPurify, which removes an unknown
// element and every attribute riding it. Every earlier browser proof
// deliberately bypassed the sanitizer (engine.spec.js test n: "no
// shell, no fetch, no DOMPurify"), so THIS suite is the first to show
// that anchor surviving the real mount; probe P4 drives the pre-ruling
// presentation red against the same vendored sanitizer.
//
// TRACE: SCN-16
// TRACE: ti 490d97 wave 7 (spec 2026-08-20 §8, §11, §12)

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
  waitForMounted,
  waitForScrollNear,
} from "./support/harness.js";

// The shell page of the SCN-16 bundle leg (scripts/test-browser.sh).
const SHELL = "/scn16/index.html";

// The source document's <title> text, decoded — from the wave-3 fixture
// crates/transync/tests/fixtures/scn-16-html-document.html
// (`<title>Transync &amp; the two-pane page</title>`). Unique on
// purpose: no body block carries this string, so "absent from the panes"
// below is a claim about THE TITLE, not about text the panes never had.
// The stub provider echoes, so source and translated title agree.
const TITLE_TEXT = "Transync & the two-pane page";

// Typed console capture. harness.collectConsole records text only; the
// zero-warnings acceptance needs the TYPE: browser resource noise (a
// favicon 404) arrives as an error-typed network message, while
// everything the engine says — map/DOM drift, the unlisted-anchor gate,
// forward drift, duplicate anchors — arrives through console.warn.
// MUST be called before page.goto so mount-time warnings are captured.
function collectConsoleTyped(page) {
  const entries = [];
  page.on("console", (msg) => entries.push({ type: msg.type(), text: msg.text() }));
  return entries;
}

// The acceptance's channel, plus the two ways an engine failure could
// dodge it: uncaught exceptions are collected separately, and an
// error-typed message carrying the engine's own prefix counts too.
function engineNoise(entries) {
  return entries.filter(
    (m) => m.type === "warning" || (m.type === "error" && m.text.includes("transync:")),
  );
}

function anchorRows(map) {
  return map.blocks.filter((r) => r.sync_role !== "non-sync");
}

test.describe("SCN-16 HTML-run bundle through the shipped shell", () => {
  test("a — the served map is the 1.3.0 HTML shape and the title row is honest", async () => {
    // Wire assertions on the served artifact — the positive base tests b
    // and c lean on, asserted here once so their failures read as wire
    // regressions rather than as mysterious pane emptiness.
    const map = JSON.parse(readFixture("scn16/alignment.json"));
    expect(map.schema_version).toBe("1.3.0");
    expect(map.input_format).toBe("html");

    // D5: exactly one title row, translated-but-non-sync, html-spelled.
    const titles = map.blocks.filter((r) => r.block_kind === "title");
    expect(titles.length).toBe(1);
    expect(titles[0].sync_role).toBe("non-sync");
    expect(titles[0].source_format).toBe("html");
    // The echo stub accepts every unit; either accepted status is honest
    // here — what must NOT appear is a fallback.
    expect(["translated", "preserved"]).toContain(titles[0].fallback_status);

    // Every row of an HTML-intake map declares the html spelling (§3's
    // format == Html invariant), and enough anchors exist to sync at all.
    for (const row of map.blocks) {
      expect(row.source_format, row.source_block_id).toBe("html");
    }
    expect(anchorRows(map).length).toBeGreaterThanOrEqual(5);
  });

  test("b — the shell mounts the panes; the title is chrome, never content", async ({ page }) => {
    const consoleLog = collectConsoleTyped(page);
    const errors = collectPageErrors(page);
    await page.goto(SHELL);
    await waitForMounted(page);

    const map = JSON.parse(readFixture("scn16/alignment.json"));
    const expected = anchorRows(map).map((r) => r.source_block_id);
    const titleId = map.blocks.find((r) => r.block_kind === "title").source_block_id;

    // POSITIVE BACKBONE: each pane carries exactly the anchors the map
    // promises — same ids, same order. This is what keeps the negatives
    // below falsifiable: an empty pane, a wrong selector, a renamed id,
    // a leaked non-sync block (title OR the fixture's <hr>) all fail
    // HERE, as a surplus or a hole in this list, instead of passing
    // vacuously there.
    for (const sel of ["#source", "#target"]) {
      const ids = await page.evaluate(
        (s) =>
          Array.from(document.querySelectorAll(`${s} [data-sync-id]`), (el) => el.dataset.syncId),
        sel,
      );
      expect(ids, sel).toEqual(expected);
    }

    // The wave-6 2026-08-21 ruling is what makes the equality above
    // satisfiable at all: the fixture's custom element is unknown to spec
    // §4's tables, so its anchor rides a transparent <div> wrapper —
    // DOMPurify removes an unknown element and every attribute on it, so
    // a self-injected <x-note> anchor would die in this very mount (probe
    // P4 drives that red). Positive: the html-kind row's anchor exists in
    // both panes and IS the wrapper.
    const htmlRow = map.blocks.find((r) => r.block_kind === "html" && r.sync_role !== "non-sync");
    expect(htmlRow, "the fixture's <x-note> mints an anchor-role html row").toBeTruthy();
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate(
          ([s, id]) => document.querySelector(`${s} [data-sync-id="${id}"]`).tagName,
          [sel, htmlRow.source_block_id],
        ),
        sel,
      ).toBe("DIV");
    }

    // NEGATIVE (D5), on that base: nothing in either pane claims the
    // title's id, and the title's text is not pane content...
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate(
          ([s, id]) => document.querySelectorAll(`${s} [data-sync-id="${id}"]`).length,
          [sel, titleId],
        ),
        sel,
      ).toBe(0);
      expect((await page.locator(sel).innerText()).includes(TITLE_TEXT), sel).toBe(false);
    }
    // ...while the SAME text IS the page's chrome: the §9 bundle-title
    // chain (flag > source <title> > "transync") put it on the browser
    // tab. Translated, aligned, rendered by chrome — never by the pane.
    expect(await page.title()).toBe(TITLE_TEXT);

    // D9: item anchors live under a reconstructed list group — never a
    // bare <li> as a direct <main> child. Measured (the vendored DOMPurify
    // 3.2.6): the sanitizer passes a bare <li> through unchanged, in
    // place — no relocation — so the mount will neither repair nor betray
    // an ungrouped pane; these structural assertions are what catch it,
    // and the grouping is mandated by D9's model, not by any sanitizer
    // behaviour (amendment I corrects §8 step 6's claim). Positive twin:
    // the group really exists and carries anchored items.
    for (const sel of ["#source", "#target"]) {
      expect(
        await page.evaluate((s) => document.querySelectorAll(`${s} main > li`).length, sel),
        sel,
      ).toBe(0);
      expect(
        await page.evaluate(
          (s) => document.querySelectorAll(`${s} main > ul > li[data-sync-id]`).length,
          sel,
        ),
        sel,
      ).toBeGreaterThanOrEqual(2);
    }

    // Zero console warnings — the acceptance's third clause. Everything
    // the engine can complain about (a map row with no DOM anchor, an
    // anchor no row claims, schema drift, duplicates) lands in this set.
    expect(errors).toEqual([]);
    expect(engineNoise(consoleLog)).toEqual([]);
  });

  test("c — bidirectional sync by block id over HTML-derived anchors", async ({ page }) => {
    const consoleLog = collectConsoleTyped(page);
    const errors = collectPageErrors(page);
    await page.goto(SHELL);
    await waitForMounted(page);
    await constrainPanes(page, 160);
    // Cold-start warm-up before the first timed wait (scn13 test a's
    // rationale — harness plumbing, no engine effect).
    await advanceFrames(page, 12);

    const map = JSON.parse(readFixture("scn16/alignment.json"));
    const anchors = anchorRows(map).map((r) => r.source_block_id);
    // Forward driver: the LAST h2-prefixed anchor — mid-document, real
    // travel, reachable (scn13's DRIVER_BLOCK rationale). Reverse target:
    // the document's first anchor row (the <h1>). Derived from the map,
    // not hard-coded, so an id-assignment change upstream moves the test
    // with it instead of silently hollowing it.
    const driver = anchors.filter((id) => id.startsWith("h2-")).pop();
    const top = anchors[0];
    expect(driver).toBeTruthy();

    // The panes must genuinely overflow, or sync-by-scroll is untestable
    // — fail loudly rather than pass vacuously (scn13's guard; the
    // SCN-16 document is smaller than SCN-14, hence 160 px panes and a
    // 60 px floor).
    expect(await maxScrollOf(page, "#source")).toBeGreaterThan(60);
    expect(await maxScrollOf(page, "#target")).toBeGreaterThan(60);

    // The follow target must be reachable, or the settle wait would time
    // out against the pane's scroll ceiling rather than the engine. If
    // THIS guard trips, shrink the pane height above — do not widen the
    // tolerance below.
    const tgtOffset = await offsetTopOf(page, "#target", driver);
    expect(tgtOffset).toBeLessThanOrEqual(await maxScrollOf(page, "#target"));

    // Forward: drive the source; the target follows to the same block.
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", driver));
    await waitForScrollNear(page, "#target", tgtOffset, 8);
    expect(await activeSyncId(page, "#target")).toBe(driver);

    await page.waitForTimeout(160); // programmatic-scroll lock decay

    // Reverse: drive the target back to the top block; the source follows.
    const srcOffset = await offsetTopOf(page, "#source", top);
    await setScrollTop(page, "#target", await offsetTopOf(page, "#target", top));
    await waitForScrollNear(page, "#source", srcOffset, 8);
    expect(await activeSyncId(page, "#source")).toBe(top);

    expect(errors).toEqual([]);
    expect(engineNoise(consoleLog)).toEqual([]);
  });
});
