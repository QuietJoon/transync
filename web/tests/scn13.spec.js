// SCN-13 — block-level dual-pane scroll sync, headless.
//
// This is the automated evidence for SCN-13, the product's defining
// browser scenario (advances OI-0023). Each test drives the REAL bundle
// emitted by the stub CLI and asserts against the actual engine in
// web/js/sync.js. The manual checklist in web/SMOKE.md is the fallback.
//
// The stub bundle is a pass-through (target HTML == source HTML), so for
// an un-reflowed mount the follower pane converges to the same scrollTop
// as the driver; the reflow test deliberately breaks that symmetry to
// prove the engine reads geometry live rather than caching offsets.
//
// TRACE: SCN-13
// TRACE: OI-0023

import { test, expect } from "@playwright/test";
import {
  activeSyncId,
  advanceFrames,
  collectConsole,
  collectPageErrors,
  constrainPanes,
  driveThenSampleAfterFrames,
  isMounted,
  maxScrollOf,
  offsetTopOf,
  paneCenter,
  range,
  readFixture,
  sampleScroll,
  scrollTopOf,
  setScrollTop,
  waitForAnchors,
  waitForMounted,
  waitForScrollNear,
  waitInPagePumped,
} from "./support/harness.js";

// A mid-document heading (~2/3 down) — far enough to force real scroll
// travel, not so far it can't reach the pane top.
const DRIVER_BLOCK = "h2-0010";
// A near-top paragraph, for the reverse direction.
const REVERSE_BLOCK = "p-0004";

test.describe("SCN-13 dual-pane sync", () => {
  test("a — bidirectional sync follows the partner anchor", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);
    // Cold-start warm-up: prime Chromium's rAF scheduling and let the
    // just-added height style reflow settle before the first *timed* wait,
    // so a slow opening frame on a fresh browser can't time out
    // waitForScrollNear (the follower converges in ~2 s once warm). This is
    // the first timed wait in the suite; harness-only, no engine effect.
    await advanceFrames(page, 12);

    // Sanity: the panes must actually overflow, or "sync by scrolling"
    // is untestable. Fail loudly rather than pass vacuously.
    expect(await maxScrollOf(page, "#source")).toBeGreaterThan(100);
    expect(await maxScrollOf(page, "#target")).toBeGreaterThan(100);

    // Forward: scroll source so DRIVER_BLOCK sits at the reference line;
    // the target should glide until the same block sits at its top.
    const tgtOffset = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    await waitForScrollNear(page, "#target", tgtOffset, 2);
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);

    // Let the target's programmatic-scroll lock (90 ms) decay so the
    // reverse drive is honored rather than absorbed.
    await page.waitForTimeout(160);

    // Reverse: now drive from the target pane; source must follow.
    const srcOffset = await offsetTopOf(page, "#source", REVERSE_BLOCK);
    await setScrollTop(page, "#target", await offsetTopOf(page, "#target", REVERSE_BLOCK));
    await waitForScrollNear(page, "#source", srcOffset, 2);
    expect(await activeSyncId(page, "#source")).toBe(REVERSE_BLOCK);

    expect(errors).toEqual([]);
  });

  test("b — no feedback oscillation after settle", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);

    const tgtOffset = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    // Wait for FULL settle (not just within tolerance) so the follow loop
    // has snapped and stopped before we sample — otherwise residual lerp
    // motion would masquerade as oscillation.
    await waitForScrollNear(page, "#target", tgtOffset, 1);
    await advanceFrames(page, 8); // let the final snap + loop-stop happen

    // Record both panes over ~20 rAF frames. A ping-pong feedback loop
    // would show one pane nudging the other back and forth; a clean
    // settle shows both effectively frozen.
    const { source, target } = await sampleScroll(page, 20);
    expect(range(source)).toBeLessThan(1.5);
    expect(range(target)).toBeLessThan(1.5);
    expect(errors).toEqual([]);
  });

  test("c — user takeover cancels the smooth follow (no snap-back)", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);

    // Park both panes near the bottom, then send the source to the top so
    // the target begins a long downward-to-0 smooth-follow (many frames).
    const maxS = await maxScrollOf(page, "#source");
    await setScrollTop(page, "#source", maxS);
    await waitForScrollNear(page, "#target", maxS, 12);
    await page.waitForTimeout(160); // let the follower's 90 ms lock decay

    // Send the source to the top — the target now animates maxScroll -> 0 —
    // and sample the follower after exactly five frames. Drive, advance and
    // sample are one evaluate on purpose: the follow's own rAF keeps frames
    // coming, so any round trip in between is unbounded extra lerping and
    // the number this guard reads stops being the number it advanced to
    // (ticket `e88db0`). Five frames leave ~33 % of the journey, so the
    // 150 px floor has room to be a real claim about being mid-flight.
    const beforeWheel = await driveThenSampleAfterFrames(page, "#source", 0, "#target", 5);
    expect(beforeWheel).toBeGreaterThan(150); // guard: really mid-flight

    // The user grabs the target with the wheel, pushing DOWN — opposite
    // the 0-ward pull. releaseDriverLock() must cancel the follow so the
    // pane obeys the user rather than snapping back toward 0.
    const c = await paneCenter(page, "#target");
    await page.mouse.move(c.x, c.y);
    await page.mouse.wheel(0, 150);

    // Observe (rAF-pumped) for ~45 frames. Had the follow won, the target
    // would have been dragged to ~0 within this window; because it
    // yielded, the target stays well down-page and holds steady.
    const { target } = await sampleScroll(page, 45);
    const afterWheel = target[target.length - 1];
    expect(afterWheel).toBeGreaterThan(150);
    expect(range(target.slice(-15))).toBeLessThan(3); // settled, no snap-back

    // Still wired, never threw.
    expect(await isMounted(page)).toBe(true);
    expect(errors).toEqual([]);
  });

  test("d — major reflow: sync uses live geometry, lands on right anchor", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);

    // Baseline drive works.
    const base = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    await waitForScrollNear(page, "#target", base, 8);
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);

    // Drastic post-mount reflow of the FOLLOWER pane: blow up its font
    // and line-height and add padding. Every target offset the engine
    // could have cached at mount is now wrong; because it reads
    // offsetTop/offsetHeight per frame, the follow must still land the
    // partner anchor. (The source is left intact so it stays a reliable
    // driver — source-side live geometry is already covered by test a.)
    await page.evaluate(() => {
      const t = document.getElementById("target");
      t.style.fontSize = "32px";
      t.style.lineHeight = "2";
      t.style.padding = "40px";
    });
    await page.waitForTimeout(160); // let lock decay + layout settle

    // Re-read live geometry after reflow and drive again. The baseline
    // already parked the source at this block's offset, so bounce it to
    // the top first — re-assigning the same scrollTop fires no scroll
    // event and the follow would never re-run.
    const tgtOffset = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    const srcOffset = await offsetTopOf(page, "#source", DRIVER_BLOCK);
    await setScrollTop(page, "#source", 0);
    await advanceFrames(page, 3);
    await setScrollTop(page, "#source", srcOffset);
    // Target layout now differs from source, so we can't assert equal
    // scrollTops — assert the correct anchor is what sits at the target's
    // reference line, and that it is roughly at the pane top.
    await waitInPagePumped(
      page,
      `(id) => {
        const pane = document.getElementById("target");
        const el = pane.querySelector('[data-sync-id="' + id + '"]');
        return Math.abs(el.offsetTop - pane.scrollTop) <= 30;
      }`,
      DRIVER_BLOCK
    );
    expect(tgtOffset).toBeGreaterThan(0);
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);
    expect(errors).toEqual([]);
  });

  test("e — missing target anchor: warns and degrades gracefully", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // Serve a target fragment with one sync anchor removed. The alignment
    // map still promises it, so warnMapDomDrift must flag the drift and
    // the engine must not throw when asked to follow the absent block.
    const missingId = DRIVER_BLOCK;
    const original = readFixture("target.html");
    const mutated = original.replace(
      new RegExp(`^.*data-sync-id="${missingId}".*$\\n?`, "m"),
      ""
    );
    expect(mutated).not.toContain(`data-sync-id="${missingId}"`);
    expect(mutated.length).toBeLessThan(original.length);

    await page.route("**/target.html", (route) =>
      route.fulfill({ contentType: "text/html", body: mutated })
    );

    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);

    // Mount-time drift warning fired for the missing target anchor.
    expect(
      logs.some((m) => m.includes("no DOM anchor") && m.includes(missingId))
    ).toBe(true);

    // Scrolling the source to the now-orphaned block: the engine's scroll
    // handler runs (frames advanced), looks up the partner, finds none,
    // and returns early — the target stays put.
    const before = await scrollTopOf(page, "#target");
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", missingId));
    await advanceFrames(page, 10);
    const after = await scrollTopOf(page, "#target");
    expect(Math.abs(after - before)).toBeLessThan(4);

    // Degraded, not broken: no uncaught exception.
    expect(errors).toEqual([]);
  });

  test("f — alignment-schema drift is refused (no sync)", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // Both an unknown MAJOR ("2.0.0") and a malformed semver ("1.foo")
    // must be rejected by loadAlignment (R0008-0045): the engine warns
    // and never wires the panes.
    for (const badVersion of ["2.0.0", "1.foo"]) {
      const map = JSON.parse(readFixture("alignment.json"));
      map.schema_version = badVersion;
      await page.route("**/alignment.json", (route) =>
        route.fulfill({
          contentType: "application/json",
          body: JSON.stringify(map),
        })
      );

      await page.goto("/");
      await waitForAnchors(page); // HTML mounts, but the engine must refuse
      await constrainPanes(page);

      expect(
        logs.some(
          (m) =>
            m.includes("rejecting alignment map") && m.includes(badVersion)
        )
      ).toBe(true);
      expect(await isMounted(page)).toBe(false);

      // No engine wired => scrolling the source cannot move the target.
      const before = await scrollTopOf(page, "#target");
      await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
      await advanceFrames(page, 10);
      expect(Math.abs((await scrollTopOf(page, "#target")) - before)).toBeLessThan(4);

      await page.unroute("**/alignment.json");
    }
    expect(errors).toEqual([]);
  });

  test("g — newer minor/patch schema drifts forward (warns, still syncs)", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // A same-major map with a newer minor ("1.3.0") is forward-compat drift
    // (OI-0024): loadAlignment accepts it and wires the panes, but surfaces a
    // visible console.warn rather than the silent console.debug.
    const map = JSON.parse(readFixture("alignment.json"));
    map.schema_version = "1.3.0";
    await page.route("**/alignment.json", (route) =>
      route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(map),
      })
    );

    await page.goto("/");
    await waitForMounted(page); // the engine must accept and wire the panes
    await constrainPanes(page);

    // The forward-drift warning fired...
    expect(
      logs.some(
        (m) => m.includes("is newer than this engine") && m.includes("1.3.0")
      )
    ).toBe(true);
    // ...yet the engine mounted and syncs normally.
    expect(await isMounted(page)).toBe(true);

    const tgtOffset = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    await waitForScrollNear(page, "#target", tgtOffset, 2);
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);

    await page.unroute("**/alignment.json");
    expect(errors).toEqual([]);
  });

  test("h — html blocks live-render, never nest wrappers, and mirror details toggles", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);

    // Live render: a real <details> element inside its anchor, in BOTH panes,
    // and no escaped placeholder for it (spec §5).
    for (const sel of ["#source", "#target"]) {
      const details = page.locator(`${sel} [data-sync-id="html-0015"] details`);
      await expect(details).toHaveCount(1);
      await expect(page.locator(`${sel} [data-sync-id="html-0015"][data-skipped]`)).toHaveCount(0);
    }

    // Spec §3.4: auto-balancing means no sync wrapper is ever swallowed
    // into another after the innerHTML mount — the fixture's unclosed
    // <details> fragment (html-0015, which the source leaves open across
    // the following blocks) is exactly the shape that would do it; the
    // hero <div> (html-0018) is the closed control case.
    const nested = await page.evaluate(
      () => document.querySelectorAll("[data-sync-id] [data-sync-id]").length
    );
    expect(nested).toBe(0);

    // Decision 9: toggling the source details mirrors to the target.
    await page.locator('#source [data-sync-id="html-0015"] summary').click();
    await advanceFrames(page, 4);
    const targetOpen = await page.evaluate(() => {
      const d = document.querySelector('#target [data-sync-id="html-0015"] details');
      return d ? d.open : null;
    });
    const sourceOpen = await page.evaluate(() => {
      const d = document.querySelector('#source [data-sync-id="html-0015"] details');
      return d ? d.open : null;
    });
    // The fixture's <details> ships closed, so a click that registered must
    // have opened it. Asserting `true` (not merely source/target equality)
    // keeps the mirror check from passing vacuously if the click never landed.
    expect(sourceOpen).toBe(true);
    expect(targetOpen).toBe(sourceOpen);

    expect(errors).toEqual([]);
  });

  test("i — unusable alignment rows are refused (no sync)", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // R0001-0043: every map here passes the version gate — it really does
    // speak schema 1.x — and is still unusable, because the rows the engine
    // keys on are missing, mistyped, unclassifiable, or ambiguous. Each must
    // be refused BY NAME rather than degraded into "pair whatever
    // data-sync-id the DOM happens to carry", which is indistinguishable
    // from working sync until the panes disagree.
    //
    // The expected fragments are distinct per case, so the shared console
    // log cannot let one iteration satisfy another's assertion.
    const cases = [
      ["no blocks array", (m) => delete m.blocks, "`blocks` is not an array"],
      [
        "non-string id",
        (m) => {
          m.blocks[1].source_block_id = 42;
        },
        "has no string source_block_id",
      ],
      [
        "unknown sync_role",
        (m) => {
          m.blocks[1].sync_role = "sideways";
        },
        'unknown sync_role "sideways"',
      ],
      [
        "duplicate row",
        (m) => m.blocks.push({ ...m.blocks[1] }),
        "duplicate source_block_id",
      ],
    ];

    for (const [name, mutate, expected] of cases) {
      const map = JSON.parse(readFixture("alignment.json"));
      mutate(map);
      await page.route("**/alignment.json", (route) =>
        route.fulfill({
          contentType: "application/json",
          body: JSON.stringify(map),
        })
      );

      await page.goto("/");
      await waitForAnchors(page); // HTML mounts, but the engine must refuse
      await constrainPanes(page);

      expect(
        logs.some(
          (m) => m.includes("rejecting alignment map") && m.includes(expected)
        ),
        name
      ).toBe(true);
      expect(await isMounted(page), name).toBe(false);

      // Refused, not merely quiet: with no engine wired the source cannot
      // drive the target.
      const before = await scrollTopOf(page, "#target");
      await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
      await advanceFrames(page, 10);
      expect(Math.abs((await scrollTopOf(page, "#target")) - before), name).toBeLessThan(4);

      await page.unroute("**/alignment.json");
    }
    expect(errors).toEqual([]);
  });

  test("j — a newer-minor map may carry an unknown sync_role (warns, still syncs)", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // The one relaxation in the row gate, and the reason it exists:
    // contracts.md §3 lets a minor bump ADD enumerated values and obliges
    // consumers to accept newer minors. A role this engine cannot classify
    // is corruption in an in-band map (test i) and a value from the future
    // in a 1.3.0 one — warned about, treated as a scroll anchor, still
    // synced.
    const map = JSON.parse(readFixture("alignment.json"));
    map.schema_version = "1.3.0";
    map.blocks[1].sync_role = "gutter";
    await page.route("**/alignment.json", (route) =>
      route.fulfill({
        contentType: "application/json",
        body: JSON.stringify(map),
      })
    );

    await page.goto("/");
    await waitForMounted(page); // accepted, unlike the in-band case
    await constrainPanes(page);

    expect(
      logs.some(
        (m) =>
          m.includes('unknown sync_role "gutter"') &&
          m.includes("forward-compat drift")
      )
    ).toBe(true);
    expect(
      logs.some((m) => m.includes("rejecting alignment map"))
    ).toBe(false);

    const tgtOffset = await offsetTopOf(page, "#target", DRIVER_BLOCK);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", DRIVER_BLOCK));
    await waitForScrollNear(page, "#target", tgtOffset, 2);
    expect(await activeSyncId(page, "#target")).toBe(DRIVER_BLOCK);

    await page.unroute("**/alignment.json");
    expect(errors).toEqual([]);
  });

  test("k — a duplicate anchor is dropped from the scan as well as the lookup", async ({ page }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // R0001-0044: give the target pane a SECOND copy of a near-top block,
    // parked directly above the mid-document heading. The old split policy
    // — lookup keeps the LAST occurrence, scan array keeps them ALL — is
    // wrong in both directions, and this fixture catches both:
    //   forward: driving the source to REVERSE_BLOCK would land the target
    //            on the copy two-thirds down instead of the block itself;
    //   reverse: scrolling the target onto the copy would report the
    //            near-top id and fling the source back to the top.
    // First-occurrence-wins, applied to both structures, makes the scan and
    // the lookup name the same element, so neither can happen.
    const dupId = REVERSE_BLOCK;
    const original = readFixture("target.html");
    const lineOf = (id) =>
      original.match(new RegExp(`^.*data-sync-id="${id}".*$`, "m"))[0];
    const dupLine = lineOf(dupId);
    const anchorLine = lineOf(DRIVER_BLOCK);
    // Function replacer: the fixture text is arbitrary content, and `$&`
    // and friends in a string replacement would rewrite it.
    const mutated = original.replace(anchorLine, () => `${dupLine}\n${anchorLine}`);
    expect(mutated.split(`data-sync-id="${dupId}"`).length - 1).toBe(2);

    await page.route("**/target.html", (route) =>
      route.fulfill({ contentType: "text/html", body: mutated })
    );

    await page.goto("/");
    await waitForMounted(page);
    await constrainPanes(page);

    // The drop is announced, and it names the policy.
    expect(
      logs.some(
        (m) =>
          m.includes("duplicate data-sync-id") &&
          m.includes(dupId) &&
          m.includes("keeping the first occurrence")
      )
    ).toBe(true);

    // Forward: the lookup answers with the FIRST occurrence, near the top —
    // not the copy the old policy would have kept.
    const firstTop = await offsetTopOf(page, "#target", dupId);
    await setScrollTop(page, "#source", await offsetTopOf(page, "#source", dupId));
    await waitForScrollNear(page, "#target", firstTop, 6);

    await page.waitForTimeout(160); // let the target's 90 ms lock decay

    // Reverse: park the target's reference line on the DROPPED copy. It is
    // no longer a scan candidate, so the pane reports the block that really
    // starts there (the mid-document heading) and the source follows to it,
    // instead of being flung back to the near-top block the copy claims.
    const copyTop = await page.evaluate(
      (id) =>
        document.querySelectorAll(`#target [data-sync-id="${id}"]`)[1].offsetTop,
      dupId
    );
    const dupSrcTop = await offsetTopOf(page, "#source", dupId);
    const driverSrcTop = await offsetTopOf(page, "#source", DRIVER_BLOCK);
    expect(copyTop).toBeGreaterThan(firstTop + 100); // guard: really far apart
    await setScrollTop(page, "#target", copyTop);
    await waitForScrollNear(page, "#source", driverSrcTop, 12);
    expect(await scrollTopOf(page, "#source")).toBeGreaterThan(dupSrcTop + 100);

    await page.unroute("**/target.html");
    expect(errors).toEqual([]);
  });

  test("l — a failed artifact fetch cancels the siblings still in flight", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);

    // R0002-0053, shell half: the bundle's boot fetches three artifacts with
    // `Promise.all`, which rejects on the FIRST failure and leaves the other
    // two downloading for a page that has already given up. One shared
    // AbortController, aborted in the catch, is what stops that.
    //
    // Rig: target.html fails immediately, alignment.json never answers. The
    // observable is the aborted sibling request, not the message — the
    // error text is identical with or without the cancellation.
    const failed = [];
    page.on("requestfailed", (req) =>
      failed.push(`${req.url()} ${req.failure()?.errorText ?? ""}`)
    );

    await page.route("**/target.html", (route) =>
      route.fulfill({ status: 500, contentType: "text/html", body: "" })
    );
    await page.route("**/alignment.json", () => {});

    await page.goto("/");

    // The boot reports the failure it actually hit.
    await expect
      .poll(() => page.locator("#source").textContent())
      .toMatch(/failed to load target\.html: HTTP 500/);

    // …and the stalled sibling is cancelled rather than left to run out its
    // 20 s fetchOk budget on a boot nobody will read.
    await expect
      .poll(() => failed.filter((f) => f.includes("alignment.json")))
      .toEqual([expect.stringMatching(/net::ERR_ABORTED/)]);

    await page.unroute("**/alignment.json");
    await page.unroute("**/target.html");
    expect(errors).toEqual([]);
  });

  test("m — an impostor data-sync-id written into source HTML never reaches a pane", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);

    // OI-0035 route (c), render half, end to end. The fixture's raw-HTML block
    // claims `p-0003` — the id of the paragraph that FOLLOWS it — so before the
    // strip the pane held two elements claiming p-0003 and the impostor, being
    // first in document order, won the engine's first-occurrence lookup and
    // became that paragraph's scroll driver. The bundle is a second
    // `--html-out` run inside the fixture dir (scripts/test-browser.sh, the
    // OI-0035 leg).
    const sourceHtml = readFixture("oi0035/source.html");
    const targetHtml = readFixture("oi0035/target.html");

    // Guards first: the assertions below are only meaningful while the block
    // takes the LIVE-render arm. On the escaped-placeholder arm the impostor's
    // quotes become `&quot;` and every "does not contain" would pass vacuously.
    expect(sourceHtml).toContain('class="impostor-marker"');
    expect(sourceHtml).not.toContain('data-skipped="html-block"');
    // The target pane carries the guard too: if the stub's translation of the
    // html block ever fell back, the target block would be the escaped
    // placeholder — its count of 1 below would then hold without the strip
    // ever running on that pane. Same vacuous pass, seen from the other side.
    expect(targetHtml).not.toContain('data-skipped="html-block"');

    expect(sourceHtml.split('data-sync-id="p-0003"').length - 1).toBe(1);
    expect(targetHtml.split('data-sync-id="p-0003"').length - 1).toBe(1);
    expect(sourceHtml).not.toContain('data-fallback="translated" class=');

    // And in the DOM the bundle actually mounts.
    await page.goto("/oi0035/");
    await waitForMounted(page);

    expect(
      await page.evaluate(
        () => document.querySelectorAll('#source [data-sync-id="p-0003"]').length
      )
    ).toBe(1);
    expect(
      await page.evaluate(
        () => document.querySelector('#source [data-sync-id="p-0003"]').tagName
      )
    ).toBe("P");
    // A nested anchor is the same defect seen from the other side: the impostor
    // sat inside the html block's own wrapper (contracts.md §4a wants every
    // anchor a direct child of <main>, list items excepted).
    expect(
      await page.evaluate(
        () => document.querySelectorAll("[data-sync-id] [data-sync-id]").length
      )
    ).toBe(0);
    expect(logs.some((m) => m.includes("duplicate data-sync-id"))).toBe(false);

    // Spec §15 item 3: DOMPurify's `ALLOW_DATA_ATTR` default was INFERRED when
    // route (c) was chosen, never measured. Measure it here, against the
    // vendored build the bundle actually ships, because the render half's
    // necessity rests on it — a sanitizer that dropped `data-*` would already
    // have closed this route. The DCR's mechanism sentence quotes this.
    expect(
      await page.evaluate(() =>
        window.DOMPurify.sanitize('<div data-sync-id="p-0003">x</div>')
      )
    ).toContain('data-sync-id="p-0003"');

    expect(errors).toEqual([]);
  });
});
