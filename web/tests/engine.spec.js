// sync.js's mount contract — headless, driven directly rather than through
// a shell.
//
// scn13.spec.js and wasm.spec.js exercise the engine the way the shipped
// shells do: one mount, over panes a bundle produced. The refusal contract
// needs the other angle — a SECOND mount over panes that are already wired,
// maps a shell can never fetch, and `fetchOk` against a server that never
// answers. So these tests import the served `sync.js` module and call it,
// over a rig of two real scroll containers built in the page.
//
// The bundle page is still the host (same origin, same server, same module
// bytes); its own mount is allowed to finish before the rig replaces the
// body, so nothing here races the shell's boot.
//
// TRACE: contracts.md §3
// TRACE: SCN-13
// TRACE: ADR-0001

import { test, expect } from "@playwright/test";
import {
  advanceFrames,
  collectConsole,
  collectPageErrors,
  offsetTopOf,
  scrollTopOf,
  setScrollTop,
  waitForMounted,
  waitForScrollNear,
} from "./support/harness.js";

const SRC = "#eng-source";
const TGT = "#eng-target";

// Five equal blocks per pane, 150 px each in a 200 px pane: enough travel
// for the follow loop to be measurable, few enough to reason about exactly.
const BLOCK_IDS = ["e-0001", "e-0002", "e-0003", "e-0004", "e-0005"];
const BLOCK_PX = 150;
const PANE_PX = 200;

/** A minimal map the engine accepts: one anchor row per rig block. */
function mapOf(ids, overrides = {}) {
  return {
    schema_version: "1.2.0",
    source_language: "en",
    target_language: "ko",
    blocks: ids.map((id, i) => ({
      source_block_id: id,
      target_block_id: id,
      block_kind: "paragraph",
      sync_role: "anchor",
      order: i,
      fallback_status: "translated",
    })),
    ...overrides,
  };
}

/**
 * Replace the shell's DOM with two bare panes carrying `ids` as anchors.
 *
 * The panes are `position: relative` scroll containers — contracts.md §4a's
 * precondition, and the reason the engine's offsetTop math means anything.
 */
async function installRig(page, ids) {
  await page.goto("/");
  // Let the shell's own mount finish before its panes are removed; clearing
  // the body mid-boot would throw inside the shell rather than in a test.
  await waitForMounted(page);
  await page.evaluate(
    ([ids, blockPx, panePx]) => {
      document.body.innerHTML = "";
      document.body.style.cssText = "display:block;margin:0;padding:0";
      for (const paneId of ["eng-source", "eng-target"]) {
        const pane = document.createElement("div");
        pane.id = paneId;
        pane.style.cssText = `position:relative;height:${panePx}px;overflow:auto;margin:0;padding:0`;
        for (const id of ids) {
          const block = document.createElement("p");
          block.dataset.syncId = id;
          block.style.cssText = `height:${blockPx}px;margin:0;padding:0`;
          block.textContent = id;
          pane.appendChild(block);
        }
        document.body.appendChild(pane);
      }
    },
    [ids, BLOCK_PX, PANE_PX]
  );
}

/** Call `mountSync` over the rig; "controller" or null, never the object. */
function mount(page, map) {
  return page.evaluate(async (map) => {
    const { mountSync } = await import("/sync.js");
    const controller = mountSync(
      document.getElementById("eng-source"),
      document.getElementById("eng-target"),
      map
    );
    return controller && typeof controller.destroy === "function"
      ? "controller"
      : controller;
  }, map);
}

/** Is either rig pane currently tagged with a controller? */
function rigTagged(page) {
  return page.evaluate(
    () =>
      !!document.getElementById("eng-source").__transyncController ||
      !!document.getElementById("eng-target").__transyncController
  );
}

/** Set a rig pane's height — the reflow a `ResizeObserver` reports. */
function setPaneHeight(page, paneId, px) {
  return page.evaluate(
    ([paneId, px]) => {
      document.getElementById(paneId).style.height = `${px}px`;
    },
    [paneId, px]
  );
}

/** Append one more anchor to BOTH rig panes, after the engine is mounted. */
function appendBlock(page, id, px) {
  return page.evaluate(
    ([id, px]) => {
      for (const paneId of ["eng-source", "eng-target"]) {
        const block = document.createElement("p");
        block.dataset.syncId = id;
        block.style.cssText = `height:${px}px;margin:0;padding:0`;
        block.textContent = id;
        document.getElementById(paneId).appendChild(block);
      }
    },
    [id, px]
  );
}

/** Plant one anchor carrying `id` at the top or the bottom of a rig pane. */
function plantAnchor(page, paneId, id, px, where) {
  return page.evaluate(
    ([paneId, id, px, where]) => {
      const pane = document.getElementById(paneId);
      const el = document.createElement("p");
      el.dataset.syncId = id;
      el.style.cssText = `height:${px}px;margin:0;padding:0`;
      el.textContent = id;
      if (where === "top") pane.insertBefore(el, pane.firstChild);
      else pane.appendChild(el);
    },
    [paneId, id, px, where]
  );
}

test.describe("sync.js mount contract", () => {
  test("a — a refused re-mount returns null and takes the live engine with it", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // A good map mounts, and the mount is real: the target follows the
    // source to the third block.
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(await rigTagged(page)).toBe(true);
    const third = BLOCK_PX * 2;
    await setScrollTop(page, SRC, third);
    await waitForScrollNear(page, TGT, third, 4);

    // R0002-0016: the same call REFUSES an unknown major, and says so by
    // returning null rather than an inert controller no caller can tell
    // apart from a working one.
    expect(await mount(page, mapOf(BLOCK_IDS, { schema_version: "2.0.0" }))).toBeNull();
    expect(
      logs.some((m) => m.includes("rejecting alignment map") && m.includes("2.0.0"))
    ).toBe(true);

    // R0002-0015: and the refusal took the PREVIOUS mount down with it. The
    // tag is the documented "am I mounted?" read, so a stale one answers the
    // question wrongly...
    expect(await rigTagged(page)).toBe(false);

    // ...and the listeners behind it are gone too. Before the fix the old
    // controller stayed wired: scrolling the source here dragged the target
    // back to the top, driven by a map the engine had just refused.
    await page.waitForTimeout(160); // let the follower's 90 ms lock decay
    await setScrollTop(page, SRC, 0);
    await advanceFrames(page, 20);
    expect(await scrollTopOf(page, TGT)).toBeGreaterThan(250);

    expect(errors).toEqual([]);
  });

  test("b — a map with no synchronizable row cannot drive anchored panes", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // R0002-0047. `blocks: []` has no malformed row for the shape gate to
    // name, and used to mount — pairing whatever `data-sync-id` the DOM
    // happened to carry, which is the degradation that gate exists to
    // refuse. Same for a map whose every row is `non-sync`: it promises no
    // anchor at all.
    for (const [name, map] of [
      ["empty blocks", mapOf([])],
      [
        "all rows non-sync",
        mapOf(BLOCK_IDS, {
          blocks: mapOf(BLOCK_IDS).blocks.map((row) => ({
            ...row,
            sync_role: "non-sync",
          })),
        }),
      ],
    ]) {
      expect(await mount(page, map), name).toBeNull();
      expect(await rigTagged(page), name).toBe(false);
    }
    // The refusal message names what the PANES carry, and that number is the
    // pin. R0002-0047 asks "do these panes carry anchors at all", which is a
    // question about the DOM — so `mountSync` must keep answering it with an
    // ungated read. An anchor count taken from `collectAnchors`' output would
    // be 0 here once the OI-0035 row gate lands, because a map with no
    // synchronizable row lists no ids and the rig's ten anchors are then all
    // unlisted; `anchorCount > 0 && rowCount === 0` would become
    // unsatisfiable and this refusal would quietly turn into a vacuous mount.
    expect(
      logs.some(
        (m) =>
          m.includes("describes no synchronizable block") &&
          m.includes(`the panes carry ${BLOCK_IDS.length * 2} anchors`)
      )
    ).toBe(true);

    // The bound on that refusal: an empty document legitimately renders
    // empty panes, and an empty map over them is vacuous, not mismatched.
    await installRig(page, []);
    expect(await mount(page, mapOf([]))).toBe("controller");
    expect(await rigTagged(page)).toBe(true);

    expect(errors).toEqual([]);
  });

  test("c — an unbounded semver component is refused, not read as forward drift", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // R0002-0045: `Number("9".repeat(400))` is Infinity, and Infinity
    // compares as newer than this engine forever — so the map was accepted
    // with the forward-compat warning instead of being refused as the
    // malformed version string it is.
    const absurd = `1.${"9".repeat(400)}.0`;
    expect(await mount(page, mapOf(BLOCK_IDS, { schema_version: absurd }))).toBeNull();
    expect(await rigTagged(page)).toBe(false);
    expect(logs.some((m) => m.includes("rejecting alignment map"))).toBe(true);
    expect(logs.some((m) => m.includes("is newer than this engine"))).toBe(false);

    // The bound is on the digit count, not on the version being newer: a
    // real forward-minor map still drifts forward and still mounts.
    expect(await mount(page, mapOf(BLOCK_IDS, { schema_version: "1.3.0" }))).toBe(
      "controller"
    );
    expect(
      logs.some((m) => m.includes("is newer than this engine") && m.includes("1.3.0"))
    ).toBe(true);

    expect(errors).toEqual([]);
  });

  test("d — fetchOk bounds a stalled request and honors a caller's abort", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // A route that never answers — the case a plain `fetch` waits out
    // forever, leaving a boot in its loading state with nothing on the
    // console to say why.
    await page.route("**/stall-*.txt", () => {});

    // R0002-0052: bounded, and the diagnostic names the timeout.
    const timedOut = await page.evaluate(async () => {
      const { fetchOk } = await import("/sync.js");
      try {
        await fetchOk("stall-1.txt", (r) => r.text(), { timeoutMs: 250 });
        return "resolved";
      } catch (err) {
        return err.message;
      }
    });
    expect(timedOut).toMatch(/timed out loading stall-1\.txt after 250 ms/);

    // R0002-0053: a caller-owned controller cancels the requests still in
    // flight, which is what lets a failed multi-artifact boot stop paying
    // for the siblings of the fetch that already failed.
    const cancelled = await page.evaluate(async () => {
      const { fetchOk } = await import("/sync.js");
      const artifacts = new AbortController();
      const sibling = fetchOk("stall-2.txt", (r) => r.text(), {
        signal: artifacts.signal,
        timeoutMs: 20000,
      });
      setTimeout(() => artifacts.abort(), 50);
      try {
        await sibling;
        return "resolved";
      } catch (err) {
        return err.message;
      }
    });
    expect(cancelled).toMatch(/cancelled loading stall-2\.txt/);

    await page.unroute("**/stall-*.txt");
    expect(errors).toEqual([]);
  });

  test("e — one element passed as both panes is refused, and costs the live mount nothing", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // A working engine, so the refusal below has something to damage.
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(await rigTagged(page)).toBe(true);

    // R0003-0071. No shipped shell does this, which is exactly why it went
    // unchecked: the node would have been torn down twice, wired twice with
    // scroll listeners and toggle mirrors driving each other, and tagged
    // with one `__transyncController` answering for both directions.
    const verdict = await page.evaluate(async (map) => {
      const { mountSync } = await import("/sync.js");
      const pane = document.getElementById("eng-source");
      const controller = mountSync(pane, pane, map);
      return controller && typeof controller.destroy === "function"
        ? "controller"
        : controller;
    }, mapOf(BLOCK_IDS));
    expect(verdict).toBeNull();
    expect(logs.some((m) => m.includes("requires two distinct panes"))).toBe(true);

    // The guard runs BEFORE the teardown that R0002-0015 put first, so a
    // caller error does not take the live engine with it: the panes are
    // still tagged, and the source still drives the target.
    expect(await rigTagged(page)).toBe(true);
    const third = BLOCK_PX * 2;
    await setScrollTop(page, SRC, third);
    await waitForScrollNear(page, TGT, third, 4);

    expect(errors).toEqual([]);
  });

  test("f — a pane that is not its blocks' offsetParent is called out at mount", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // R0003-0076. contracts.md §4a's non-static `position` is what makes
    // `offsetTop` pane-relative; drop it and the mount still succeeds while
    // every offset is silently measured against `<body>` instead — the
    // contract's own "silent layout regression". The engine reads one
    // anchor's `offsetParent` per pane at mount and names the element the
    // offsets are actually coming from.
    await page.evaluate(() => {
      document.getElementById("eng-target").style.position = "static";
    });
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(
      logs.some(
        (m) =>
          m.includes("the target pane is not the offsetParent") && m.includes("<body>")
      )
    ).toBe(true);
    // Only the pane that violates it. The source pane is still relative, and
    // a check that fired for both would be noise nobody could act on.
    expect(logs.some((m) => m.includes("the source pane is not the offsetParent"))).toBe(
      false
    );

    // A warning, not a refusal: the geometry is wrong but the panes still
    // pair by block id, and the caller may simply be mid-layout.
    expect(await rigTagged(page)).toBe(true);

    expect(errors).toEqual([]);
  });

  test("g — a map that contradicts ID identity is refused, not mounted-but-unsyncable", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // A live engine first, so the refusal below has something to stop.
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");

    // R0003-0002 / ticket d3acc3. ID-identity pairing is normative for
    // schema 1.x (ADR-0001, contracts.md §3): the partner of a block is the
    // element carrying the SAME `data-sync-id`. A row that re-points the
    // target side therefore describes a pairing this engine will not
    // perform. It used to mount: `warnMapDomDrift` resolved
    // `target_block_id` while `handleScroll` looked the source id up
    // directly, so the two disagreed and the panes rendered but never
    // synced. It is now refused with the id pair named.
    const divergent = mapOf(BLOCK_IDS);
    divergent.blocks[2].target_block_id = "t-0003";
    expect(await mount(page, divergent)).toBeNull();
    expect(await rigTagged(page)).toBe(false);
    expect(
      logs.some(
        (m) =>
          m.includes("pairs source_block_id") &&
          m.includes("e-0003") &&
          m.includes("t-0003")
      )
    ).toBe(true);

    // The forward-minor exemption, read exactly like the `sync_role` one:
    // contracts.md §3 requires a newer minor to be accepted, so the same
    // row there is warned about and the engine keeps pairing by the source
    // id — which is all it knows how to do.
    const future = mapOf(BLOCK_IDS, { schema_version: "1.3.0" });
    future.blocks[2].target_block_id = "t-0003";
    expect(await mount(page, future)).toBe("controller");
    expect(logs.some((m) => m.includes("pairing by the source id anyway"))).toBe(true);

    // And the drift warning agrees with the lookup rather than contradicting
    // it: the target pane really does carry `e-0003`, so nothing is missing.
    // The old code probed the target side for `t-0003` and reported a
    // missing anchor for a block that is right there.
    expect(logs.some((m) => m.includes("no DOM anchor"))).toBe(false);

    // The pairing it warned about is the pairing it performs.
    const third = BLOCK_PX * 2;
    await setScrollTop(page, SRC, third);
    await waitForScrollNear(page, TGT, third, 4);

    expect(errors).toEqual([]);
  });

  test("h — a pane resize re-drives the follower and refreshes the anchor cache", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    const logs = collectConsole(page);
    await installRig(page, BLOCK_IDS);

    // OI-0024 item 1. The map promises a sixth block the DOM does not carry
    // yet — a real integration shape (the pane fills in late), and the
    // mount-time drift warning says so.
    const LATE_ID = "e-0006";
    const LATE_PX = 400;

    // A tall follower: 750 px of content in a 500 px pane can only scroll
    // 250 px, so the source's drive lands CLAMPED. That clamp is what makes
    // the resize observable — nothing about the block offsets changes.
    await setPaneHeight(page, "eng-target", 500);
    expect(await mount(page, mapOf([...BLOCK_IDS, LATE_ID]))).toBe("controller");
    expect(logs.some((m) => m.includes("no DOM anchor") && m.includes(LATE_ID))).toBe(
      true
    );

    // Drive the source to its own maximum. The follower wants ~550 and can
    // only reach 250.
    await setScrollTop(page, SRC, BLOCK_PX * BLOCK_IDS.length - PANE_PX);
    await waitForScrollNear(page, TGT, 250, 6);

    // Shrink the follower back. Its reachable range grows to 550, and the
    // place the reader is actually at becomes reachable — but only if
    // something re-drives, because no scroll event fires here. Before the
    // reflow hooks the pane sat at 250 until the reader scrolled again, and
    // the docstring told the caller to destroy and re-mount.
    await setPaneHeight(page, "eng-target", PANE_PX);
    await waitForScrollNear(page, TGT, 550, 12);

    // Second half of the same recompute: the anchor sets are re-collected,
    // not merely re-measured. `e-0006` arrives after mount, so the cached
    // block list has never seen it; without the refresh the source's active
    // scan finds nothing at all below the old last block and the follower
    // never moves.
    await appendBlock(page, LATE_ID, LATE_PX);
    await setPaneHeight(page, "eng-target", 260);
    await setScrollTop(
      page,
      SRC,
      BLOCK_PX * BLOCK_IDS.length + LATE_PX - PANE_PX
    );
    await waitForScrollNear(page, TGT, 890, 15);
    expect(await rigTagged(page)).toBe(true);

    expect(errors).toEqual([]);
  });

  test("i — an image that finishes loading re-drives the follower", async ({ page }) => {
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");

    // Park the reader on the third block; the panes agree.
    const third = BLOCK_PX * 2;
    await setScrollTop(page, SRC, third);
    await waitForScrollNear(page, TGT, third, 4);

    // An image lands inside the follower's FIRST block and doubles its
    // height, so every later block in that pane moves down 200 px — the
    // "image that finally arrived" reflow. The pane box never changes, so
    // `ResizeObserver` cannot see this one; the capture-phase `load`
    // listener on the pane is what reports it.
    await page.evaluate(() => {
      const pane = document.getElementById("eng-target");
      const first = pane.querySelector('[data-sync-id="e-0001"]');
      first.style.height = "auto";
      first.textContent = "";
      const img = document.createElement("img");
      img.style.cssText = "display:block";
      img.width = 10;
      img.height = 350;
      // 1×1 transparent GIF — no network, and `load` still fires.
      img.src =
        "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
      first.appendChild(img);
    });

    // The third block has moved down in the follower. The reader's own pane
    // never scrolled, so only the image-load recompute can put that block
    // back under the reference line — the engine's own drive target for
    // this position is exactly the block's offsetTop.
    const moved = await offsetTopOf(page, TGT, "e-0003");
    expect(moved).toBeGreaterThan(third + 100);
    await waitForScrollNear(page, TGT, moved, 12);

    expect(errors).toEqual([]);
  });

  test("j — a <details> toggle re-drives the follower, not just its twin", async ({
    page,
  }) => {
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // R0004-0087. The engine mirrors a disclosure across the panes (spec
    // 2026-08-03 §5, decision 9) and used to stop there. That mirror is the
    // one CONTENT reflow the engine causes itself, and no observer reports
    // it: the pane box never changes, so `ResizeObserver` is silent, and
    // there is no image and no font involved. The follower therefore stayed
    // wherever the pre-toggle geometry had left it until the reader happened
    // to scroll again.
    //
    // The two panes' disclosures rarely grow by the same number of pixels —
    // translated body text wraps differently — and equal growth needs no
    // correction at all, so the rig makes the asymmetry total: the driver's
    // first block is height-pinned with `overflow:hidden`, so opening its
    // disclosure moves NOTHING on the reader's side, while the follower's
    // grows by the body's full height. Whatever the follower does next is
    // therefore the recompute and nothing else.
    const BODY_PX = 400;
    await page.evaluate(
      ([bodyPx, blockPx]) => {
        for (const paneId of ["eng-source", "eng-target"]) {
          const first = document.querySelector(`#${paneId} [data-sync-id="e-0001"]`);
          first.textContent = "";
          if (paneId === "eng-source") {
            first.style.cssText = `height:${blockPx}px;margin:0;padding:0;overflow:hidden`;
          } else {
            first.style.cssText = `min-height:${blockPx}px;height:auto;margin:0;padding:0`;
          }
          const details = document.createElement("details");
          details.style.cssText = "margin:0;padding:0";
          const summary = document.createElement("summary");
          summary.style.cssText = "margin:0;padding:0";
          summary.textContent = "extras";
          const body = document.createElement("div");
          body.style.cssText = `height:${bodyPx}px;margin:0;padding:0`;
          details.append(summary, body);
          first.appendChild(details);
        }
      },
      [BODY_PX, BLOCK_PX]
    );

    // Built before the mount, so the engine's cached geometry is the settled
    // closed one and the toggle below is the only reflow in the test.
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(await offsetTopOf(page, SRC, "e-0003")).toBe(
      await offsetTopOf(page, TGT, "e-0003")
    );

    // Park the reader on the third block; the panes agree.
    const third = BLOCK_PX * 2;
    await setScrollTop(page, SRC, third);
    await waitForScrollNear(page, TGT, third, 4);

    // Open the DRIVER's disclosure. The mirror opens the follower's twin,
    // which pushes every later block in that pane down by the body height.
    await page.evaluate(() => {
      document.querySelector('#eng-source [data-sync-id="e-0001"] details').open = true;
    });
    await advanceFrames(page, 4);

    // The mirror landed (otherwise the follower has nothing to re-align to)…
    expect(
      await page.evaluate(
        () =>
          document.querySelector('#eng-target [data-sync-id="e-0001"] details').open
      )
    ).toBe(true);
    // …and the reader's own pane did not move a pixel, so no scroll event
    // fired and nothing but the recompute can drive the follower.
    expect(Math.round(await scrollTopOf(page, SRC))).toBe(third);
    expect(await offsetTopOf(page, SRC, "e-0003")).toBe(third);

    // The third block has moved down in the follower; the engine's drive
    // target for this reading position is exactly its new offsetTop. Before
    // this fix the pane simply stayed at `third`.
    const moved = await offsetTopOf(page, TGT, "e-0003");
    expect(moved).toBeGreaterThan(third + 200);
    await waitForScrollNear(page, TGT, moved, 12);

    expect(errors).toEqual([]);
  });

  test("k — an anchor no alignment row claims cannot drive the follower", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // OI-0035 route (c), engine half. Source content is untrusted data
    // (architectural invariant 7), raw HTML is translatable
    // structurally-owned content, and DOMPurify's default keeps `data-*` — so
    // a `data-sync-id` an author writes reaches BOTH panes. DOM membership
    // used to decide what could drive scroll while map membership decided
    // nothing at all; the validated rows are the authority now.
    //
    // The impostor sits at the TOP of the driver and the BOTTOM of the
    // follower, so if it were collected it would answer for the reader's very
    // first block and fling the follower to the other end of the document.
    await plantAnchor(page, "eng-source", "x-9999", BLOCK_PX, "top");
    await plantAnchor(page, "eng-target", "x-9999", BLOCK_PX, "bottom");

    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    for (const pane of ["source", "target"]) {
      expect(
        logs.some(
          (m) =>
            m.includes('ignoring anchor "x-9999"') &&
            m.includes(`${pane} pane`) &&
            m.includes("no alignment row claims it")
        ),
        pane
      ).toBe(true);
    }
    // Two policies, kept distinct: an unlisted anchor is not a duplicate, and
    // reporting it as one would send a reader hunting for a producer defect
    // that is not there.
    expect(logs.some((m) => m.includes("duplicate data-sync-id"))).toBe(false);

    // Park the reader mid-document, so "the follower came home" is a claim
    // about a pane that had somewhere else to be.
    const third = await offsetTopOf(page, TGT, "e-0003");
    await setScrollTop(page, SRC, await offsetTopOf(page, SRC, "e-0003"));
    await waitForScrollNear(page, TGT, third, 6);
    await page.waitForTimeout(160); // let the follower's 90 ms lock decay

    // Back to the top, where the impostor straddles the driver's reference
    // line. Collected, it pairs with the follower's copy 750 px down and the
    // follower runs to its maximum; ignored, the scan falls through to
    // `e-0001` and the follower comes home.
    await setScrollTop(page, SRC, 0);
    await waitForScrollNear(page, TGT, 0, 4);

    expect(errors).toEqual([]);
  });

  test("l — a duplicate the map does claim keeps its first occurrence, and an anchor that arrives after mount stays inert", async ({
    page,
  }) => {
    const logs = collectConsole(page);
    const errors = collectPageErrors(page);
    await installRig(page, BLOCK_IDS);

    // First half. The row gate runs BEFORE the duplicate policy, so the policy
    // has to be shown intact (R0001-0044): a SECOND `e-0001` at the bottom of
    // the follower is a producer defect, warned about, and the FIRST
    // occurrence is what both the scan array and the partner lookup name.
    await plantAnchor(page, "eng-target", "e-0001", BLOCK_PX, "bottom");
    expect(await mount(page, mapOf(BLOCK_IDS))).toBe("controller");
    expect(
      logs.some(
        (m) =>
          m.includes("duplicate data-sync-id") &&
          m.includes("e-0001") &&
          m.includes("keeping the first occurrence")
      )
    ).toBe(true);

    const third = await offsetTopOf(page, TGT, "e-0003");
    await setScrollTop(page, SRC, await offsetTopOf(page, SRC, "e-0003"));
    await waitForScrollNear(page, TGT, third, 6);
    await page.waitForTimeout(160);
    await setScrollTop(page, SRC, 0);
    // The first e-0001 at offsetTop 0, never the copy 750 px down.
    await waitForScrollNear(page, TGT, 0, 4);

    // Second half — the 2026-08-09 correction's residual, closed. A reflow
    // recompute ACTIVATES an anchor that entered the DOM after mount, with the
    // duplicate audit deliberately suppressed, so before this gate there was a
    // way into the live anchor set with no audit at any point. Now the row list
    // decides: a late arrival is inert on the same terms as one that was there
    // all along, and the recompute stays quiet about it.
    // 400 px, not BLOCK_PX: the driver's reachable scroll range is
    // `content - 200`, so a 150 px tail would sit entirely below the deepest
    // reference line the pane admits and the case could never be exercised.
    // At 400 px the band the impostor occupies is genuinely reachable.
    await appendBlock(page, "x-8888", 400);
    await setPaneHeight(page, "eng-target", 260);
    await advanceFrames(page, 6);
    expect(logs.some((m) => m.includes("x-8888"))).toBe(false);
    await page.waitForTimeout(160);

    // Drive the reader onto the late anchor's band. Collected, it pairs with
    // the follower's copy at 900 px and drags the pane there; inert, nothing
    // straddles the line, `handleScroll` returns early and the follower stays
    // home at 0.
    await setScrollTop(page, SRC, (await offsetTopOf(page, SRC, "x-8888")) + 4);
    await advanceFrames(page, 45);
    expect(await scrollTopOf(page, TGT)).toBeLessThan(50);

    expect(errors).toEqual([]);
  });
});
