Filed at `/Volumes/Common/QJoon/transync/reviews/0005.md` (claim token `113506F6-…-BF22258FADE6`).

**One medium-severity finding**: transync's annotated-HTML wrapper structure isn't documented, and the wrapper element it emits (`<main>`) carries no positioning context. Consumers that mount this HTML into a scrollable pane and use `data-sync-id` for scroll math discover the hard way that `block.offsetTop` walks past the wrapper and the consumer's pane to whatever ancestor is positioned (often `<body>`). Result: scroll-sync engines over-scroll the partner pane to absolute coordinates instead of pane-relative ones.

Three recommendations, in priority order:

1. **Document the wrapper in `contracts.md`** — spell out that there's exactly one outer wrapper element (`<main>`), blocks are its direct children, transync emits no positioning rules, the consumer owns positioning context.
2. **Stamp `position: relative` on the wrapper inline.** ~5-line renderer change; consumers can use `offsetTop`-based math without CSS guesswork. Smallest behavioral fix.
3. **Optionally drop the `<main>` wrapper.** Largest simplification; only if it's not load-bearing for accessibility/structure.

Plus one **low-severity ride-along**: transync's reference `sync.js` uses `getBoundingClientRect` everywhere it could plausibly use `offsetTop` and doesn't say why — adding a two-line comment would propagate the lesson without forcing every consumer to read `contracts.md`.

Bug context: reproducible on any annotated HTML mounted in a default-CSS pane; resp-translator's `web/js/sync.js` hit it on a 32-block document and reverted to `getBoundingClientRect` (commit `0229ed6`) as the consumer-side workaround.
