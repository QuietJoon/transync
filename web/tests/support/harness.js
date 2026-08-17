// SCN-13 Playwright harness — shared page-side helpers.
//
// The suite drives a REAL bundle produced by the stub CLI
// (`--html-out`), served over HTTP by Playwright's webServer. Every
// assertion is aligned with the actual engine in `web/js/sync.js`:
//   - active block = the one straddling a reference line 4 px below the
//     pane top (REFERENCE_OFFSET_PX), progress-weighted;
//   - the partner pane is driven by a per-frame lerp (SMOOTHING_FACTOR
//     0.2) toward the computed scrollTop, settling under 0.5 px;
//   - both panes are tagged `__transyncController` once mountSync wires
//     them (absent when mount is refused).
//
// TRACE: SCN-13
// TRACE: OI-0023

import fs from "node:fs";
import path from "node:path";

// The bundle the webServer serves. scripts/test-browser.sh regenerates
// it before every run; the default matches playwright.config.js.
export const FIXTURE_DIR =
  process.env.TRANSYNC_FIXTURE_DIR ||
  "/Volumes/Temp/claude/transync-browser-fixture/html";

// REFERENCE_OFFSET_PX in sync.js — the "currently reading" line.
export const REFERENCE_OFFSET_PX = 4;

// Sync-relevant block ids present in the SCN-14 stub bundle, in document
// order. The lone thematic break (hr-0009) carries no data-sync-id
// (sync_role "non-sync"), so it is intentionally absent here.
//
// The html-* / p-0016 tail is the appended raw-HTML specimen set: an
// unclosed <details> fragment (html-0015), the paragraph the fragment
// would swallow without render-side balancing (p-0016), the orphan
// </details> close that carries no translatable segment (html-0017), and
// a self-contained <div> hero (html-0018). All four are "anchor" blocks,
// so all four carry data-sync-id.
export const SYNC_IDS = [
  "h1-0001", "p-0002", "h2-0003", "p-0004", "c-0005",
  "li-0006", "li-0007", "li-0008", "h2-0010", "p-0011",
  "t-0012", "q-0013", "img-0014", "html-0015", "p-0016",
  "html-0017", "html-0018",
];

// Read a file straight from the served bundle so route-interception
// tests can mutate a real payload rather than a hand-authored stand-in.
export function readFixture(name) {
  return fs.readFileSync(path.join(FIXTURE_DIR, name), "utf8");
}

// Collect every console message (log/info/warn/debug/error) emitted by
// the page. MUST be called before page.goto so mount-time warnings —
// warnMapDomDrift, the schema-version rejection — are captured.
export function collectConsole(page) {
  const messages = [];
  page.on("console", (msg) => messages.push(msg.text()));
  return messages;
}

// Collect uncaught page exceptions so "degrades gracefully / no
// exception" can be asserted rather than assumed.
export function collectPageErrors(page) {
  const errors = [];
  page.on("pageerror", (err) => errors.push(err.message));
  return errors;
}

// Wait until mountSync has wired the panes (tags __transyncController on
// both). Rejects via Playwright timeout if mount never completes.
export function waitForMounted(page) {
  return page.waitForFunction(
    () => !!document.getElementById("source")?.__transyncController
  );
}

// Wait until the panes' HTML has been injected (data-sync-id anchors
// present) WITHOUT requiring a controller — used when mount is expected
// to be refused (schema drift), where innerHTML is still set but no
// engine is wired.
export function waitForAnchors(page) {
  return page.waitForFunction(
    () => !!document.querySelector("#source [data-sync-id]")
  );
}

// True iff the engine wired this run (both panes carry the controller).
export function isMounted(page) {
  return page.evaluate(
    () => !!document.getElementById("source")?.__transyncController
  );
}

// offsetTop of the anchor with `id` inside pane `sel` (#source/#target).
// Pane-relative because the pane is position:relative (contracts §4a) —
// the same coordinate space the engine reads.
export function offsetTopOf(page, sel, id) {
  return page.evaluate(
    ([sel, id]) =>
      document.querySelector(`${sel} [data-sync-id="${id}"]`).offsetTop,
    [sel, id]
  );
}

// Force both panes to a fixed height so they are genuine scroll
// containers under a headless fixed viewport. The demo shell's grid
// otherwise stretches each pane to its content height (the WINDOW
// scrolls, panes do not) — which would leave pane.scrollTop pinned at 0
// and make block-level sync untestable. An explicit height overrides
// grid `align-self: stretch`. This is harness plumbing only: the engine
// reads pane geometry live regardless of where the height comes from.
export function constrainPanes(page, h = 260) {
  return page.addStyleTag({
    content: `#source, #target { height: ${h}px; }`,
  });
}

// Maximum scrollTop the pane admits (scrollHeight - clientHeight).
export function maxScrollOf(page, sel) {
  return page.evaluate((sel) => {
    const p = document.querySelector(sel);
    return Math.max(0, p.scrollHeight - p.clientHeight);
  }, sel);
}

export function scrollTopOf(page, sel) {
  return page.evaluate(
    (sel) => document.querySelector(sel).scrollTop,
    sel
  );
}

// Programmatically scroll a pane; fires the native scroll event the
// engine listens for (RAF-coalesced), exactly as a user drag would.
export function setScrollTop(page, sel, y) {
  return page.evaluate(
    ([sel, y]) => {
      document.querySelector(sel).scrollTop = y;
    },
    [sel, y]
  );
}

// Wait for the follower pane's smooth loop to bring scrollTop within
// `tol` px of `expected`. The lerp is monotone toward the target, so a
// one-shot threshold is a sound settle test.
//
// IMPORTANT: this runs its own in-page requestAnimationFrame loop rather
// than Playwright's waitForFunction polling. The engine's smooth-follow
// (ensureSmoothLoop) self-schedules via rAF; in headless Chromium those
// frames only advance while something in-page is actively pumping rAF.
// A continuous in-page loop guarantees the engine animates to settle
// (verified: the follower converges to 0 px error), whereas external
// polling can leave the loop starved and the pane frozen mid-follow.
//
// The default timeout carries cold-start headroom: on the very first timed
// wait of a fresh browser, JIT + first-layout jank can delay the follower's
// opening frames well past a warm run's ~2 s convergence (OI-0024 flake fix).
export function waitForScrollNear(page, sel, expected, tol = 8, timeout = 8000) {
  return page.evaluate(
    ([sel, expected, tol, timeout]) => {
      const el = document.querySelector(sel);
      const start = performance.now();
      return new Promise((resolve, reject) => {
        const tick = () => {
          if (Math.abs(el.scrollTop - expected) <= tol) {
            resolve(el.scrollTop);
            return;
          }
          if (performance.now() - start > timeout) {
            reject(
              new Error(
                `waitForScrollNear: ${sel} at ${Math.round(el.scrollTop)}, expected ~${expected}±${tol}`
              )
            );
            return;
          }
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
    },
    [sel, expected, tol, timeout]
  );
}

// Pump rAF in-page until `predicate` (a browser-context function body,
// stringified) returns true, or reject on timeout. Same rationale as
// waitForScrollNear: keep the engine's rAF loop fed in headless.
export function waitInPagePumped(page, predicateSource, args, timeout = 4000) {
  return page.evaluate(
    ([predicateSource, args, timeout]) => {
      // eslint-disable-next-line no-new-func
      const predicate = new Function("args", `return (${predicateSource})(args);`);
      const start = performance.now();
      return new Promise((resolve, reject) => {
        const tick = () => {
          if (predicate(args)) {
            resolve(true);
            return;
          }
          if (performance.now() - start > timeout) {
            reject(new Error("waitInPagePumped: timeout"));
            return;
          }
          requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
    },
    [predicateSource, args, timeout]
  );
}

// Replicate the engine's reference-line active-block pick so a test can
// assert which anchor a pane "lands on" independent of sub-pixel
// scrollTop. Mirrors activeBlockWithProgress() in sync.js.
export function activeSyncId(page, sel) {
  return page.evaluate(
    ([sel, ref]) => {
      const pane = document.querySelector(sel);
      const blocks = Array.from(pane.querySelectorAll("[data-sync-id]"));
      if (!blocks.length) return null;
      const scrollTop = pane.scrollTop;
      const visibleBottom = scrollTop + pane.clientHeight;
      const line = scrollTop + ref;
      let topmost = null;
      let topmostTop = Infinity;
      for (const el of blocks) {
        const top = el.offsetTop;
        const bottom = top + el.offsetHeight;
        if (top > visibleBottom) continue;
        if (bottom < scrollTop) continue;
        if (top <= line && bottom >= line) return el.dataset.syncId;
        if (top > line && top < topmostTop) {
          topmostTop = top;
          topmost = el;
        }
      }
      return topmost ? topmost.dataset.syncId : null;
    },
    [sel, REFERENCE_OFFSET_PX]
  );
}

// Sample both panes' scrollTop across `frames` requestAnimationFrame
// ticks. Used to prove the panes settle without ping-pong.
export function sampleScroll(page, frames = 20) {
  return page.evaluate((frames) => {
    const src = document.getElementById("source");
    const tgt = document.getElementById("target");
    return new Promise((resolve) => {
      const source = [];
      const target = [];
      let n = 0;
      const tick = () => {
        source.push(src.scrollTop);
        target.push(tgt.scrollTop);
        if (++n >= frames) resolve({ source, target });
        else requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
  }, frames);
}

// Drive `driverSel` to `y`, pump exactly `frames` requestAnimationFrame
// ticks, and read `followerSel`'s scrollTop — all inside ONE evaluate,
// returning that scrollTop.
//
// Three separate calls cannot express this. The engine's smooth-follow
// loop self-schedules rAF, so while a follow is in flight the page keeps
// its own frame pump alive: every round trip between the test runner and
// the browser is an unbounded number of extra frames, and a follower
// sampled after one has lerped further than the test asked it to. Each
// frame closes 20 % of the remaining distance (SMOOTHING_FACTOR), so the
// gap compounds — five extra frames are a third of the journey.
//
// That is ticket `e88db0`: scn13 case `c` advanced five frames toward 0
// from ~1280 px (~420 px left) and then read 137 px, failing a `> 150`
// floor the engine had not actually crossed at the frame the test named.
// Keeping the drive, the advance and the sample on the page side makes the
// sample a property of the frame count and nothing else.
export function driveThenSampleAfterFrames(page, driverSel, y, followerSel, frames) {
  return page.evaluate(
    ([driverSel, y, followerSel, frames]) => {
      const driver = document.querySelector(driverSel);
      const follower = document.querySelector(followerSel);
      driver.scrollTop = y;
      return new Promise((resolve) => {
        let i = 0;
        const tick = () => {
          if (++i >= frames) resolve(follower.scrollTop);
          else requestAnimationFrame(tick);
        };
        requestAnimationFrame(tick);
      });
    },
    [driverSel, y, followerSel, frames]
  );
}

// Pump exactly `n` requestAnimationFrame ticks in-page. Advances the
// engine's smooth-follow loop (and its scroll-handler rAF) a
// deterministic number of steps in headless, where frames otherwise
// stall unless something in-page keeps rAF alive.
//
// Sound for "let things settle" and for "prove nothing moved". For a
// reading taken WHILE a follow is animating, use
// `driveThenSampleAfterFrames` instead — see its note.
export function advanceFrames(page, n = 6) {
  return page.evaluate((n) => {
    return new Promise((resolve) => {
      let i = 0;
      const tick = () => {
        if (++i >= n) resolve(i);
        else requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
  }, n);
}

export function range(values) {
  return Math.max(...values) - Math.min(...values);
}

// Centre point of a pane's bounding box, for aiming mouse.wheel.
export function paneCenter(page, sel) {
  return page.evaluate((sel) => {
    const r = document.querySelector(sel).getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  }, sel);
}
