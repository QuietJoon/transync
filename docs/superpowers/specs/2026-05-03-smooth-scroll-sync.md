# Smooth scroll sync — `web/js/sync.js`

Per-frame lerping so the partner pane glides toward the active block's
position instead of snapping to it.

## Problem

`web/js/sync.js::handleScroll` currently does, every frame the source
pane fires a scroll event:

```js
partner.scrollTop = desiredScrollTop;
```

Two effects the user wants gone:

- **A. No easing.** The partner snaps in fixed steps every RAF tick;
  there is no soft trail / damped motion.
- **B. Within-block jitter.** `getBoundingClientRect()` measurements
  shift by sub-pixel amounts between frames; that jitter passes straight
  through to the partner, producing visible micro-twitch while the user
  is reading inside one long paragraph.

Block-to-block transitions feel fine already (the user explicitly said
C is OK).

## Approach: per-frame lerp toward a stored target

Decouple "compute target" (driven by user scroll events on the active
pane) from "move partner" (driven by an animation loop on the partner
pane).

### State

Add a `targets` map to `mountSync`'s state:

```js
state.targets = { source: null, target: null };
state.smoothLoopActive = { source: false, target: false };
```

`targets[label]` is the desired `scrollTop` for the pane named `label`.
`null` means no animation is in flight.

### `handleScroll` change

Replace the direct assignment with a target write + loop kick:

```js
state.targets[partnerLabel] = desiredScrollTop;
ensureSmoothLoop(partner, partnerLabel, state);
```

`ensureSmoothLoop` is idempotent — if the loop is already running for
this partner, it's a no-op.

### The smooth loop

```js
function ensureSmoothLoop(pane, label, state) {
  if (state.smoothLoopActive[label]) return;
  state.smoothLoopActive[label] = true;

  const step = () => {
    const target = state.targets[label];
    if (target == null) {
      state.smoothLoopActive[label] = false;
      return;
    }
    const current = pane.scrollTop;
    const delta = target - current;
    if (Math.abs(delta) < 0.5) {
      pane.scrollTop = target;            // settle exactly
      state.targets[label] = null;
      state.smoothLoopActive[label] = false;
      return;
    }
    state.lockUntil = performance.now() + PROGRAMMATIC_SCROLL_LOCK_MS;
    pane.scrollTop = current + delta * SMOOTHING_FACTOR;
    requestAnimationFrame(step);
  };

  requestAnimationFrame(step);
}
```

`SMOOTHING_FACTOR = 0.2` — closes 20% of the gap per frame. After
~250 ms (15 frames at 60 fps), the partner is within 4% of the target.
The 0.5 px threshold prevents perpetual sub-pixel jitter from keeping
the loop alive forever.

### Lock interaction

Each smooth step re-arms `state.lockUntil = now + 90 ms`. While the
smooth loop is animating, the partner's own scroll-event handler will
see `now < state.lockUntil` and bail out — no feedback loop.

When the loop terminates (delta < 0.5 px), no further `lockUntil`
re-arm happens; the lock decays after 90 ms; user input on the partner
side is honored from then on.

### Why this fixes both A and B

- **A**: motion is now `delta * 0.2` per frame, not `delta` per frame.
  That's exponential ease-out — the canonical "soft trail" feel.
- **B**: per-frame target jitter passes through a 0.2 multiplier, so a
  ±1 px wobble in the computed target produces ±0.2 px movement, below
  the visual threshold for noticing twitch.

## Non-goals (deliberately deferred)

- **User-takeover detection.** If the user starts scrolling the partner
  pane while a smooth animation is in flight, the partner's scroll
  events will be ignored for up to 90 ms (the lock). They can scroll
  freely once the animation halts; for typical reading flows this is
  invisible. A heuristic that detects "user input vs. our own
  animation step" by comparing actual delta to expected lerp delta is
  possible but not needed for MVP-smooth.
- **Configurable smoothing.** `SMOOTHING_FACTOR` is a const for now.
  If the chosen value feels off after testing, we tune it inline. A
  future enhancement could expose it via `mountSync` options.
- **Spring physics / cubic-bezier easing.** Linear lerp gives a
  "trailing" feel that is close enough to spring damping for this use
  case, without the extra state (velocity, mass).

## Files touched

1. `web/js/sync.js` — workspace-level source-of-truth.
2. `crates/transync-cli/web/sync.js` — build-time embedded copy.

Both files must stay in sync until SL-13's optional build script
automates the copy.

## Verification

- Manual: run `./scripts/test.sh`, scroll either pane on
  `samples/demo-complex.md`. Both panes glide; no within-block twitch;
  no oscillation; no perceptible feedback loop. Acceptable trail is
  ~200 ms.
- Existing scenario tests are unaffected — `cargo test --workspace`
  must still pass 13 + 4 (transync-cli with `test-stub-provider`).

## Out of scope

- Renderer changes (the duplicate-list-item fix is already in
  `1e3936d`).
- Live OpenAI integration tweaks.
- Block-boundary algorithm (C is already OK per user).
