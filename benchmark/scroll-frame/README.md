# `scroll-frame`

Does `sync.js`'s `activeBlockWithProgress` overrun the scroll-frame budget?
See `RESULTS.md` for the answer and `profile.mjs` for how it is measured.

```bash
node profile.mjs                                  # 50,200,500,2000,5000 × 180 frames
node profile.mjs --sizes 2000 --frames 300        # one size, longer drive
```

Needs the Playwright install under `web/node_modules` and its Chromium
(`cd web && pnpm install && pnpm exec playwright install chromium`). It resolves
Playwright from there rather than carrying its own copy.

**It serves `web/js/sync.js` itself**, from a ~40-line static server in the
script. That is deliberate: the module has no imports, so no bundle is needed —
which sidesteps `scripts/test-browser.sh`'s rebuild *and* ti `ed2e73`'s
stale-bundle hazard by construction. There is no bundle here to be stale.
