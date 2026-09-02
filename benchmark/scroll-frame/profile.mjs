#!/usr/bin/env node
// Does `activeBlockWithProgress` overrun the scroll-frame budget? — ti `dd21ad`
//
// OI-0016 records the shape: the function walks EVERY `[data-sync-id]` block on
// every RAF-coalesced scroll frame, reading `offsetTop`/`offsetHeight` on each,
// and a sorted-offset cache with binary search would be O(log n) instead of
// O(n). The entry's own Required Actions begin "If profiling shows jank", so
// the prerequisite is a measurement nobody had taken. This is that measurement.
//
// ## What it measures, and why this way
//
// The cost is attributed BY FUNCTION NAME out of a real V8 CPU profile
// (CDP `Profiler`), taken while the real engine handles real scrolling. The
// obvious alternative — reimplementing the loop in the page and timing it —
// would have been a second copy of the function, and a copy that drifted would
// measure something the engine does not do.
//
// It also records per-frame wall time, because the function's own cost is only
// interesting against the budget it has to fit in: 16.7 ms at 60 Hz.
//
// ## Why it serves `sync.js` itself
//
// `web/js/sync.js` has no imports, so a 40-line static server is enough. That
// sidesteps `scripts/test-browser.sh`'s bundle rebuild AND ti `ed2e73`'s
// stale-bundle hazard by construction: the module bytes come straight from the
// repo, so there is no bundle to be stale.
//
// Usage: node profile.mjs [--frames N] [--sizes 50,200,...]

import http from "node:http";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import pw from "../../web/node_modules/@playwright/test/index.js";

const { chromium } = pw;
const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, "../..");
const SYNC_JS = path.join(REPO, "web/js/sync.js");

const args = process.argv.slice(2);
const argOf = (name, fallback) => {
  const i = args.indexOf(name);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const SIZES = argOf("--sizes", "50,200,500,2000,5000").split(",").map(Number);
const FRAMES = Number(argOf("--frames", "180"));
const BLOCK_PX = 120;
const PANE_PX = 600;
const FRAME_BUDGET_MS = 1000 / 60;

// ---------------------------------------------------------------- rig page ---

const RIG = (n) => `<!doctype html><html><head><meta charset="utf-8"><style>
  html,body{margin:0;padding:0}
  .pane{position:relative;height:${PANE_PX}px;overflow:auto;margin:0;padding:0;width:48%;float:left}
  .pane p{margin:0;padding:0;height:${BLOCK_PX}px}
</style></head><body>
<div id="src" class="pane"></div><div id="tgt" class="pane"></div>
<script type="module">
  import { mountSync } from "/sync.js";
  const N = ${n};
  const ids = Array.from({length: N}, (_, i) => "p-" + String(i + 1).padStart(5, "0"));
  for (const paneId of ["src", "tgt"]) {
    const pane = document.getElementById(paneId);
    const frag = document.createDocumentFragment();
    for (const id of ids) {
      const el = document.createElement("p");
      el.dataset.syncId = id;
      el.textContent = id;
      frag.appendChild(el);
    }
    pane.appendChild(frag);
  }
  const map = {
    schema_version: "1.2.0", source_language: "en", target_language: "ko",
    blocks: ids.map((id, i) => ({
      source_block_id: id, target_block_id: id, block_kind: "paragraph",
      sync_role: "anchor", order: i, fallback_status: "translated",
    })),
  };
  window.__ctl = mountSync(document.getElementById("src"), document.getElementById("tgt"), map);
  window.__ready = !!window.__ctl;
</script></body></html>`;

// ------------------------------------------------------------------ server ---

function serve() {
  const server = http.createServer((req, res) => {
    const url = (req.url || "/").split("?")[0];
    if (url === "/sync.js") {
      res.writeHead(200, { "content-type": "text/javascript; charset=utf-8" });
      res.end(fs.readFileSync(SYNC_JS));
      return;
    }
    const m = /^\/rig\/(\d+)$/.exec(url);
    if (m) {
      res.writeHead(200, { "content-type": "text/html; charset=utf-8" });
      res.end(RIG(Number(m[1])));
      return;
    }
    res.writeHead(404).end("no");
  });
  return new Promise((resolve) => {
    // Loopback only. Nothing here binds a public interface.
    server.listen(0, "127.0.0.1", () => resolve({ server, port: server.address().port }));
  });
}

// ------------------------------------------------------------- attribution ---

/** Self-time per function name, in ms, from a V8 CPU profile. */
function selfTimeByName(profile) {
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const hits = new Map();
  // `timeDeltas` are per-sample; sum the delta that FOLLOWS each sample id,
  // which is how V8 attributes elapsed time to the sample that preceded it.
  for (let i = 0; i < profile.samples.length; i++) {
    const node = byId.get(profile.samples[i]);
    if (!node) continue;
    const name = node.callFrame.functionName || "(anonymous)";
    const dt = (profile.timeDeltas[i] ?? 0) / 1000; // µs -> ms
    hits.set(name, (hits.get(name) ?? 0) + dt);
  }
  return hits;
}

// --------------------------------------------------------------------- run ---

const { server, port } = await serve();
const browser = await chromium.launch();
const rows = [];

for (const n of SIZES) {
  const page = await browser.newPage({ viewport: { width: 1200, height: PANE_PX + 40 } });
  await page.goto(`http://127.0.0.1:${port}/rig/${n}`);
  await page.waitForFunction(() => window.__ready === true, null, { timeout: 30_000 });

  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Profiler.enable");
  // 100 µs: fine enough to resolve a sub-millisecond function, coarse enough
  // not to distort the thing being measured.
  await cdp.send("Profiler.setSamplingInterval", { interval: 100 });
  await cdp.send("Profiler.start");

  // Drive the scroll the way a reader does: forward across the whole
  // document, one step per animation frame, so every frame carries one
  // scroll event for the engine to coalesce.
  //
  // NOTE ON WHAT IS NOT MEASURED HERE. An earlier draft also reported
  // per-frame WALL time and counted frames over 16.67 ms. It reported
  // 120/120 over budget at every size — because headless Chromium drives
  // requestAnimationFrame at its own cadence (~24 Hz here), so that column
  // measured the harness, not the engine. Main-thread BUSY time is the
  // honest comparison against a frame budget, and it comes out of the
  // profile below rather than out of a clock in the page.
  const drive = await page.evaluate(async (frames) => {
    const pane = document.getElementById("src");
    const max = pane.scrollHeight - pane.clientHeight;
    const step = Math.max(1, Math.floor(max / frames));
    for (let i = 0; i < frames; i++) {
      pane.scrollTop = Math.min(max, i * step);
      await new Promise((r) => requestAnimationFrame(() => r()));
    }
    return { frames, maxScroll: max };
  }, FRAMES);

  const { profile } = await cdp.send("Profiler.stop");
  const sweep = selfTimeByName(profile);
  const sweepWall = (profile.endTime - profile.startTime) / 1000;

  // WORST CASE. The loop returns as soon as it finds the block holding the
  // reference line, so its cost tracks that block's INDEX, not the document
  // size — a reader at the top pays almost nothing and a reader at the bottom
  // pays the full walk. The sweep above averages the two. This pass parks near
  // the end and jiggles, which is where a large document actually hurts.
  await cdp.send("Profiler.start");
  await page.evaluate(async (frames) => {
    const pane = document.getElementById("src");
    const max = pane.scrollHeight - pane.clientHeight;
    const deep = Math.floor(max * 0.97);
    for (let i = 0; i < frames; i++) {
      pane.scrollTop = deep + (i % 2 === 0 ? 0 : 3);
      await new Promise((r) => requestAnimationFrame(() => r()));
    }
  }, FRAMES);
  const { profile: deepProfile } = await cdp.send("Profiler.stop");
  const deep = selfTimeByName(deepProfile);
  const deepWall = (deepProfile.endTime - deepProfile.startTime) / 1000;

  const busy = (hits, wall) => wall - (hits.get("(idle)") ?? 0);
  rows.push({
    n,
    frames: drive.frames,
    sweepActive: sweep.get("activeBlockWithProgress") ?? 0,
    sweepBusy: busy(sweep, sweepWall),
    deepActive: deep.get("activeBlockWithProgress") ?? 0,
    deepBusy: busy(deep, deepWall),
  });
  await page.close();
}

await browser.close();
server.close();

// ------------------------------------------------------------------ report ---

const fmt = (x, d = 3) => x.toFixed(d);
const pct = (ms) => `${((ms / FRAME_BUDGET_MS) * 100).toFixed(2)} %`;

console.log("\nscroll-frame profile — ti `dd21ad` / OI-0016");
console.log(`frames driven per pass: ${FRAMES}    frame budget: ${fmt(FRAME_BUDGET_MS, 2)} ms (60 Hz)`);
console.log("self-time attributed to `activeBlockWithProgress` by the V8 sampler,");
console.log("inside the real engine handling real scroll events.\n");

const head = ["blocks", "sweep/frame", "of budget", "deep/frame", "of budget", "busy/frame", "of budget"];
console.log(head.map((h, i) => (i === 0 ? h.padStart(7) : h.padStart(13))).join(""));
for (const r of rows) {
  const sweepPF = r.sweepActive / Math.max(1, r.frames);
  const deepPF = r.deepActive / Math.max(1, r.frames);
  const busyPF = r.deepBusy / Math.max(1, r.frames);
  console.log(
    [
      String(r.n).padStart(7),
      `${fmt(sweepPF)} ms`.padStart(13),
      pct(sweepPF).padStart(13),
      `${fmt(deepPF)} ms`.padStart(13),
      pct(deepPF).padStart(13),
      `${fmt(busyPF)} ms`.padStart(13),
      pct(busyPF).padStart(13),
    ].join("")
  );
}

console.log(`
  sweep/frame  scrolling forward across the whole document — the average a
               reader pays, since the loop stops at the block holding the
               reference line and that index grows with scroll depth.
  deep/frame   parked at 97 % and jiggling: the worst case, where the loop
               walks nearly every block before it stops.
  busy/frame   ALL main-thread JS during the deep pass, not just this
               function — the number the frame budget actually constrains.

Wall-clock frame times are deliberately absent: headless Chromium runs
requestAnimationFrame at its own cadence, so a frame-time column here would
measure the harness. Busy time is the honest comparison.
`);
