# Does active-block selection overrun the scroll-frame budget? — measured 2026-09-02

Discharges the prerequisite in ti `dd21ad` / OI-0016, whose Required Actions
begin *"If profiling shows jank"*. Nobody had profiled it. Reproduce with:

```bash
node profile.mjs --sizes 50,500,2000,5000 --frames 150
```

## Answer: no overrun, and the mechanism is exactly as OI-0016 described

| blocks | sweep/frame | of budget | deep/frame | of budget | busy/frame | of budget |
|---:|---:|---:|---:|---:|---:|---:|
| 50 | 0.022 ms | 0.13 % | 0.034 ms | 0.20 % | 0.158 ms | 0.95 % |
| 500 | 0.126 ms | 0.76 % | 0.238 ms | 1.43 % | 0.354 ms | 2.12 % |
| 2000 | 0.500 ms | 3.00 % | 0.856 ms | 5.14 % | 1.327 ms | 7.96 % |
| 5000 | 1.053 ms | 6.32 % | 1.946 ms | 11.68 % | 2.439 ms | 14.63 % |

Self-time attributed to `activeBlockWithProgress` by the V8 sampler, inside the
real engine handling real scroll events. Frame budget 16.67 ms at 60 Hz.

- **sweep** — scrolling forward across the whole document. The average a reader
  pays, because the loop stops at the block holding the reference line and that
  index grows with scroll depth.
- **deep** — parked at 97 % and jiggling. The worst case, where the loop walks
  nearly every block before it stops.
- **busy** — *all* main-thread JS during the deep pass, not just this function.
  The number the frame budget actually constrains.

The scan is linear in block count, as OI-0016 says. It is also **cheap**: at
5000 blocks — well past anything this repository contains, its 2386-paragraph
CHANGELOG included — the worst case leaves 85 % of the frame budget unused.

## What that means for the optimization

The sorted-offset cache plus binary search would work. It is not worth building
now, and this measurement is what says so rather than an opinion.

OI-0016 already priced it: the cache must be invalidated on resize, on reflow
and on any DOM mutation, and the engine must decide what to do when layout is
non-monotonic — when a later block's `offsetTop` is not greater than an earlier
one's, which absolutely-positioned or floated content can produce. That policy
question is the real work; the binary search is trivial. Both would be added to
a path that today has **neither** an invalidation surface nor a layout-order
policy, in exchange for reclaiming at most 12 % of one frame.

## The honest boundary

**This is one machine.** Apple Silicon, headless Chromium 1228. OI-0016's
concern is "large documents on slower machines", and this does not measure one.
Scaling the worst case linearly: a machine **5× slower** would put the
5000-block case near 10 ms of function time inside a 16.67 ms budget — tight but
not over. A machine **10× slower** would overrun.

So the finding is not "this can never jank". It is: **no realistic document
janks on hardware like this, and the headroom at 2000 blocks is roughly 12×.**
What would reopen it is a slow-device report, not a larger document.

## What is deliberately not measured

- **Wall-clock frame times.** An earlier draft reported them and counted frames
  over budget; it said 120/120 at every size, because headless Chromium drives
  `requestAnimationFrame` at its own cadence (~24 Hz here). That column measured
  the harness, not the engine. Main-thread busy time out of the profile is the
  honest comparison, and it is what the table carries.
- **A reimplementation of the loop.** Timing a copy in the page would have been
  a second version of the function; one that drifted would measure something the
  engine does not do. Cost is attributed by function name out of a V8 CPU
  profile instead.
- **Real reader input.** Scrolling is driven programmatically, one step per
  animation frame. A trackpad fling coalesces differently; this is the
  one-event-per-frame case the engine is written for.
