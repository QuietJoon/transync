// transync — vanilla ESM block-level sync engine.
//
// Public API: mountSync(sourcePane, targetPane, alignmentMap).
//
// Strategy: each pane runs a scroll listener (RAF-coalesced). On every
// scroll frame the engine looks up the block whose bounds straddle a
// small "reference line" inside the pane (4 px below the top), measures
// the user's progress *within* that block, and stores the corresponding
// scrollTop value the partner pane should reach. A separate per-pane
// animation loop (also RAF) lerps the partner's scrollTop toward that
// target so the partner glides instead of snapping.
//
// Per-pane programmatic-scroll locks absorb the cascade scroll event
// that fires on a pane when *we* programmatically set its scrollTop —
// without blocking the user's continued scrolling on the OTHER pane.
//
// Packaging: ONE file, deliberately (OI-0015, re-affirmed 2026-08-06).
// The engine, the map gate, the fetch helper and the storage helper live
// together because this module is mirrored byte-for-byte into the CLI
// bundle (`crates/transync-cli/web/sync.js`, welded by
// `crates/transync-cli/tests/sync_js_drift.rs`) and `web/` is a no-build,
// framework-free tree. Splitting the module would multiply both the
// mirror and the assets every consumer has to ship, to buy a source
// boundary neither of them needs.
//
// TRACE: SCN-13
// TRACE: contracts.md §3
// TRACE: contracts.md §4
// TRACE: ADR-0001
// TRACE: ADR-0006
// TRACE: DCR-0008

// Window during which scroll events on a pane are ignored after WE
// programmatically set its scrollTop. Long enough to absorb the follow-
// up scroll event; short enough that real user input on that pane
// resumes responsiveness once our animation halts.
const PROGRAMMATIC_SCROLL_LOCK_MS = 90;

// Reference line offset inside each pane (px from the top). Anything at
// or below this line is considered "currently being read".
const REFERENCE_OFFSET_PX = 4;

// Fraction of the remaining gap to close per animation frame. 0.2 ≈
// 20 % per frame at 60 fps; the partner reaches ~95 % of the target in
// ~250 ms after the user stops scrolling. Lower = softer trail; higher
// = snappier follow.
const SMOOTHING_FACTOR = 0.2;

// When |target - current| drops below this many pixels, snap to the
// target exactly and end the animation loop.
const SETTLE_THRESHOLD_PX = 0.5;

// How far a pane may sit from the value the smooth loop last assigned it
// before the loop concludes that something else moved it. Read-back, not
// assignment, is the baseline (see `ensureSmoothLoop`), so this absorbs only
// the browser's own quantizing of a fractional scrollTop — a genuine scroll
// is orders of magnitude larger. Nothing here sets `scroll-behavior: smooth`,
// so an assignment lands synchronously and the read-back is meaningful.
//
// ti 7936c2: without this the loop could drag a pane back to a stale
// destination, overriding a real scroll and — because it re-arms the lock on
// every frame — swallowing that scroll instead of letting the pane drive.
const EXTERNAL_SCROLL_TOLERANCE_PX = 2;

/**
 * Mount the sync engine over two pane DOM elements.
 *
 * Caches each pane's `[data-sync-id]` element list — the elements whose
 * ids the validated rows claim (OI-0035) — and a partner-side
 * `id → element` map at mount time, so per-frame scroll handling
 * doesn't repeat a `querySelectorAll` + attribute-selector lookup.
 *
 * **Reflow refreshes those caches, and re-aligns the panes (OI-0024 item
 * 1).** The engine observes four reflow signals — a `ResizeObserver` on
 * both panes, `document.fonts.ready`, `load`/`error` on images inside
 * either pane, and a `<details>` toggle inside either pane (R0004-0087) —
 * and on any of them re-collects both anchor sets and
 * re-drives the follower from the pane that last drove. Common reflows
 * (a resized window, a late webfont, an image that finally arrived, a
 * disclosure the reader opened) no
 * longer need the caller-driven destroy-and-remount the docstring used to
 * demand. A caller that *replaces* a pane's HTML still must
 * `controller.destroy()` and re-mount: a replacement is not a reflow, it
 * need not resize anything, and the outgoing elements are gone.
 *
 * **Pairing is by identical `data-sync-id`, and that is normative
 * (schema 1.x — ADR-0001, contracts.md §3).** The partner of the block
 * under the reference line is the element carrying the *same* id in the
 * other pane; the alignment map's `source_block_id` / `target_block_id`
 * indirection is not routed through, and a map whose row contradicts the
 * identity is refused rather than mounted-but-unsyncable (R0003-0002 —
 * see `validateRows`).
 *
 * **The anchor set comes from the validated rows, not from the DOM
 * (OI-0035).** Every element whose `data-sync-id` is absent from the map's
 * synchronizable rows is skipped — at mount, where the skip is announced, and
 * at every reflow recompute, where it is silent — so a `data-sync-id` carried
 * in by document content is inert forever, including one inserted after
 * mount. The residual this cannot reach is an impostor with a LISTED id
 * placed ahead of the genuine anchor, which first-occurrence-wins cannot tell
 * apart; the renderer closes that one by stripping the reserved namespace out
 * of raw-HTML blocks (`contracts.md` §4).
 *
 * The "no synchronizable row for panes that carry anchors" refusal below is
 * NOT routed through that set. Whether the panes carry anchors is read
 * straight off the DOM, before the anchor sets are collected, because it asks
 * what the panes hold rather than what the engine may drive — and a map with
 * no synchronizable row lists no ids, so a gated count would be zero for
 * exactly the maps that refusal exists to refuse.
 *
 * **Refusal is a real outcome, and it is signalled (R0002-0016).** A map this
 * engine cannot drive — unknown major `schema_version`, rows failing the
 * shape gate below, or no synchronizable row at all for panes that carry
 * anchors — leaves the panes unwired and returns `null`, with the reason on
 * the console. A caller that reports such a mount as a success leaves panes
 * that render but never scroll together, so `null` MUST be treated as "not
 * mounted"; the pane's `__transyncController` tag is the same verdict read
 * off the DOM, as the headless suite does.
 *
 * **Refusal tears down first (R0002-0015).** Whatever controller a previous
 * mount left on either pane is destroyed BEFORE this map is judged. The
 * alternative — judge, then tear down — left a refused re-mount with the old
 * controller still listening on panes whose DOM the caller had already
 * replaced, still tagged on them, and therefore still reporting "mounted".
 *
 * **Layout precondition.** Each pane element MUST have
 * `position: relative` (or any non-static value) in its CSS, so the
 * pane is the offsetParent of every rendered block. Without that,
 * `block.offsetTop` walks past the pane to the nearest positioned
 * ancestor — typically `<body>` — and the engine's per-block scroll
 * math reads the wrong offsets. See `docs/architecture/contracts.md`
 * §4a.
 *
 * @param {HTMLElement} sourcePane
 * @param {HTMLElement} targetPane
 * @param {Object} alignmentMap  // schema_version "1.3.0"
 * @returns {{ destroy: () => void, refresh: () => void } | null}  null when the map is refused
 */
export function mountSync(sourcePane, targetPane, alignmentMap) {
  if (!sourcePane || !targetPane) {
    console.warn("transync: mountSync requires both panes");
    return null;
  }
  // R0003-0071: two roles, two elements. One node passed twice would be torn
  // down twice, wired twice — two competing scroll listeners and two toggle
  // mirrors on the same node, each driving the other — and tagged with a
  // single `__transyncController` that answers for both directions. There is
  // no dual-pane arrangement this serves, so it is a caller error, refused
  // the same way a missing pane is.
  if (sourcePane === targetPane) {
    console.warn(
      "transync: mountSync requires two distinct panes; refusing to mount one element as both source and target"
    );
    return null;
  }

  // R0008-0048: a second mountSync() on a pane already mounted by this
  // module would double its listeners and run competing animation loops.
  // Tear down any controller previously installed on either pane first, so
  // a re-mount is idempotent rather than additive.
  //
  // R0002-0015: this runs BEFORE the map is judged, so it covers the refused
  // re-mount too. The other order left the previous controller live — with
  // its anchor caches pointing into DOM the caller had already replaced, and
  // its tag still on the panes claiming a mount that this call refused.
  destroyExisting(sourcePane);
  destroyExisting(targetPane);

  if (!loadAlignment(alignmentMap)) {
    return null;
  }

  const state = {
    // Per-pane lock — only blocks scroll events on the pane being
    // programmatically animated. This is the critical change versus
    // a shared lock: the source-pane handler must keep updating the
    // target's target throughout a continuous user scroll, even while
    // the target's smooth loop is running.
    lockUntil: { source: 0, target: 0 },
    rafScheduled: { source: false, target: false },
    scrollRafId: { source: null, target: null },
    targets: { source: null, target: null },
    // The scrollTop each pane's smooth loop last assigned it, read back after
    // the write, and the scrollHeight the pane had at that moment. `null`
    // whenever no loop owns the pane. Together they tell our own animation
    // apart from an external scroll — the height is what keeps a CONTENT
    // reflow out of that category (ti 7936c2).
    lastAssigned: { source: null, target: null },
    lastAssignedHeight: { source: null, target: null },
    smoothLoopActive: { source: false, target: false },
    smoothLoopRafId: { source: null, target: null },
    // Which pane last drove the other — the pane a reflow recompute
    // re-reads to put the follower back where the reader left it. Null
    // until the first drive, when the panes are still both at the top and
    // a reflow has nothing to correct.
    lastDriver: null,
  };

  // OI-0035: the anchor set comes from the validated rows, never from whatever
  // the DOM happens to carry. Built after `loadAlignment` because it is only
  // meaningful over rows the shape gate has already accepted.
  const rowIds = synchronizableRowIds(alignmentMap);

  // R0002-0047: a map whose rows describe no synchronizable block — `blocks:
  // []`, or every row `non-sync` — has no malformed row for the shape gate to
  // name, and would then wire whatever `data-sync-id` anchors the panes
  // happen to carry. That is precisely the "pair whatever the DOM has"
  // degradation the gate above exists to refuse, so it is refused here
  // instead. The refusal is conditioned on the panes because an empty
  // document legitimately mounts an empty map over empty panes: nothing to
  // pair on either side is a vacuous mount, not a mismatched one.
  //
  // The count is read straight off the DOM, and this refusal runs BEFORE the
  // collection below — both deliberate, and both load-bearing since the
  // OI-0035 gate (ti `490d97` wave 1). Counting collected anchors instead
  // would make the condition unsatisfiable: the maps refused here are exactly
  // the maps whose `rowIds` is empty, a gated collection returns nothing for
  // them, so `anchorCount` would always be 0 and a title-only map over
  // anchored panes would mount vacuously instead of being refused. This read
  // is not a hole in the gate — nothing it sees is retained, paired, driven,
  // or handed to a context; it answers only "do these panes carry anchors at
  // all", which is the question R0002-0047 asks and the one question the
  // validated rows cannot answer.
  //
  // The row side of the condition reads `rowIds` — the set built just above —
  // rather than deriving it a second time (R0009-0026). That is the same
  // single-derivation property this comment's OI-0035 note is about, one step
  // further: the ids the anchors are gated on and the count this refusal turns
  // on are now literally one value, not two agreeing computations.
  const anchorCount =
    sourcePane.querySelectorAll("[data-sync-id]").length +
    targetPane.querySelectorAll("[data-sync-id]").length;
  if (anchorCount > 0 && rowIds.size === 0) {
    console.warn(
      `transync: rejecting alignment map — it describes no synchronizable ` +
        `block, but the panes carry ${anchorCount} anchors`
    );
    return null;
  }

  const sourceAnchors = collectAnchors(sourcePane, "source", rowIds);
  const targetAnchors = collectAnchors(targetPane, "target", rowIds);

  warnMapDomDrift(alignmentMap, sourceAnchors.byId, targetAnchors.byId);
  warnOffsetParentDrift(sourcePane, sourceAnchors.blocks, "source");
  warnOffsetParentDrift(targetPane, targetAnchors.blocks, "target");

  // The two per-pane contexts are MUTABLE and long-lived: `handleScroll`
  // and the toggle mirror read `blocks` / `partnerById` off them at call
  // time, so a reflow recompute can swap in freshly collected anchor sets
  // without re-wiring a single listener.
  const sourceCtx = {
    pane: sourcePane,
    partner: targetPane,
    label: "source",
    partnerLabel: "target",
    state,
    blocks: sourceAnchors.blocks,
    partnerById: targetAnchors.byId,
  };
  const targetCtx = {
    pane: targetPane,
    partner: sourcePane,
    label: "target",
    partnerLabel: "source",
    state,
    blocks: targetAnchors.blocks,
    partnerById: sourceAnchors.byId,
  };
  const sourceController = wirePane(sourceCtx);
  const targetController = wirePane(targetCtx);

  const reflow = wireReflowRecompute({
    sourcePane,
    targetPane,
    sourceCtx,
    targetCtx,
    state,
    rowIds,
  });

  // Spec 2026-08-03 §5 (decision 9): mirror <details> toggle state across
  // panes so anchor geometry stays congruent and in-block progress mapping
  // keeps meaning. Interaction ownership, not structure. The toggle event
  // does not bubble — listen in the capture phase.
  function makeToggleMirror(ctx) {
    const pane = ctx.pane;
    return (event) => {
      const details = event.target;
      if (!details || details.tagName !== "DETAILS") return;
      const anchor = details.closest("[data-sync-id]");
      if (!anchor || !pane.contains(anchor)) return;
      // R0004-0087: a disclosure opening or closing is a CONTENT reflow —
      // the pane box does not change, so the `ResizeObserver` below never
      // reports it, and until this line the follower stayed where the old
      // geometry had put it until the reader happened to scroll again. It is
      // also the one content reflow the engine causes itself (the mirror
      // assignment below reflows the partner), and the two panes' disclosures
      // rarely grow by the same number of pixels — translated body text wraps
      // differently — so the correction is real rather than cosmetic. Ask for
      // the same recompute `refresh` exposes. Scheduled BEFORE the mirror and
      // for every recognised toggle, including one whose partner anchor is
      // missing: this pane reflowed either way. Coalesced into one animation
      // frame, so the mirrored toggle's own handler adds nothing.
      reflow.schedule();
      // Read the partner map off the ctx rather than closing over it, so a
      // reflow recompute's freshly collected map is the one consulted.
      const partnerAnchor = ctx.partnerById.get(anchor.dataset.syncId);
      if (!partnerAnchor) return;
      const own = anchor.querySelectorAll("details");
      const twins = partnerAnchor.querySelectorAll("details");
      const index = Array.prototype.indexOf.call(own, details);
      const twin = index >= 0 ? twins[index] : undefined;
      if (twin && twin.open !== details.open) {
        // Assignment fires the partner's toggle; the equality guard above
        // makes the mirrored event a no-op — no ping-pong.
        twin.open = details.open;
      }
    };
  }
  const sourceToggle = makeToggleMirror(sourceCtx);
  const targetToggle = makeToggleMirror(targetCtx);
  sourcePane.addEventListener("toggle", sourceToggle, true);
  targetPane.addEventListener("toggle", targetToggle, true);

  const controller = {
    destroy() {
      reflow.destroy();
      sourceController.destroy();
      targetController.destroy();
      sourcePane.removeEventListener("toggle", sourceToggle, true);
      targetPane.removeEventListener("toggle", targetToggle, true);
      if (sourcePane.__transyncController === controller) {
        delete sourcePane.__transyncController;
      }
      if (targetPane.__transyncController === controller) {
        delete targetPane.__transyncController;
      }
    },
    // The same recompute the engine's reflow signals trigger, exposed so a
    // caller that changed layout in a way no observer reports (a CSS class
    // swap on an ancestor, a stylesheet swapped at runtime) can ask for the
    // re-alignment without tearing the engine down. A `<details>` opened by
    // script no longer needs it — that fires a `toggle`, which the mirror
    // above now schedules on (R0004-0087). Coalesced into one
    // animation frame like every other trigger, so calling it in a loop
    // costs one recompute.
    refresh: reflow.schedule,
  };
  // Tag both panes so a later mountSync() can find and tear down this
  // controller (R0008-0048).
  sourcePane.__transyncController = controller;
  targetPane.__transyncController = controller;
  return controller;
}

/**
 * Reflow recompute (OI-0024 item 1, ticket `d3acc3`).
 *
 * Before this, the engine only ever reacted to *scroll*. Geometry is read
 * per frame, so a reflow never produced stale offsets — but it did leave
 * the two panes describing different places, and nothing brought them
 * back together until the reader happened to scroll again. The docstring's
 * answer was "destroy and re-mount", which a caller can only do by
 * guessing when a reflow happened. Four signals now say it directly:
 *
 * - **`ResizeObserver` on both panes** — the pane box changing is the
 *   common case (a resized window, a collapsed sidebar, a device rotation)
 *   and it moves the reachable scroll range as well as the layout.
 * - **`document.fonts.ready`** — a webfont swapping in after boot re-lays
 *   out every block in both panes at once.
 * - **`load` / `error` on images inside either pane**, in the capture
 *   phase because neither event bubbles. Both directions matter: an image
 *   that arrives grows its block, and one that fails collapses to alt
 *   text. Filtered to `<img>` so an unrelated subresource cannot pump the
 *   recompute.
 * - **A `<details>` toggle inside either pane** (R0004-0087), raised by the
 *   toggle mirror above. This one is a CONTENT reflow: the pane box is
 *   unchanged, so the `ResizeObserver` says nothing, and it is the only
 *   reflow the engine itself causes — mirroring the toggle reflows the
 *   partner pane.
 *
 * One recompute does two things. It re-collects both anchor sets, so a
 * block list that went stale is replaced rather than merely re-measured;
 * and it re-runs the last driving pane's scroll handler, which is what
 * actually puts the follower back under the reader. `state.lastDriver` is
 * null until someone drives, and then a reflow has nothing to correct.
 *
 * Every signal is coalesced into a single animation frame: a window drag
 * delivers `ResizeObserver` entries at frame rate, and one recompute per
 * frame is the most that can be observed anyway.
 */
function wireReflowRecompute({ sourcePane, targetPane, sourceCtx, targetCtx, state, rowIds }) {
  let torn = false;
  let rafId = null;

  const recompute = () => {
    rafId = null;
    if (torn) return;
    // Quiet re-collection: mount is the audit point for duplicate ids, and
    // a resize drag would otherwise replay the same warning every frame. The
    // row gate is NOT quiet in the same sense — it still applies here, and
    // that is what makes an anchor inserted after mount inert forever rather
    // than merely inert until the next reflow (OI-0035).
    const source = collectAnchors(sourcePane, "source", rowIds, true);
    const target = collectAnchors(targetPane, "target", rowIds, true);
    sourceCtx.blocks = source.blocks;
    sourceCtx.partnerById = target.byId;
    targetCtx.blocks = target.blocks;
    targetCtx.partnerById = source.byId;
    if (state.lastDriver === "source") handleScroll(sourceCtx);
    else if (state.lastDriver === "target") handleScroll(targetCtx);
  };

  const schedule = () => {
    if (torn || rafId != null) return;
    rafId = requestAnimationFrame(recompute);
  };

  let observer = null;
  if (typeof ResizeObserver === "function") {
    observer = new ResizeObserver(schedule);
    observer.observe(sourcePane);
    observer.observe(targetPane);
  }

  const onMediaSettled = (event) => {
    const el = event.target;
    if (el && el.tagName === "IMG") schedule();
  };
  for (const pane of [sourcePane, targetPane]) {
    pane.addEventListener("load", onMediaSettled, true);
    pane.addEventListener("error", onMediaSettled, true);
  }

  // Resolved already on most boots, in which case this lands as a
  // microtask over panes nobody has driven yet — a no-op, by design.
  if (typeof document !== "undefined" && document.fonts && document.fonts.ready) {
    document.fonts.ready.then(schedule, () => {});
  }

  return {
    schedule,
    destroy() {
      torn = true;
      if (observer) observer.disconnect();
      for (const pane of [sourcePane, targetPane]) {
        pane.removeEventListener("load", onMediaSettled, true);
        pane.removeEventListener("error", onMediaSettled, true);
      }
      if (rafId != null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
    },
  };
}

// Tear down whichever controller this module last mounted on `pane`, if any.
// R0008-0048.
function destroyExisting(pane) {
  const existing = pane && pane.__transyncController;
  if (existing && typeof existing.destroy === "function") {
    existing.destroy();
  }
}

/**
 * Collect a pane's `[data-sync-id]` anchors into the two structures the
 * engine runs on — the document-order array the active-block scan walks,
 * and the `id → element` map the partner lookup answers from — under ONE
 * duplicate policy (R0001-0044).
 *
 * `contracts.md` §4a guarantees one element per anchored row, so a repeated
 * id is a producer defect. It used to be absorbed two incompatible ways at
 * once: the lookup kept the LAST occurrence while the scan array kept every
 * one of them. Active-block detection could therefore settle on an earlier
 * duplicate while the partner lookup answered with the last, sending the
 * follower pane to a block the user was nowhere near — with nothing tying
 * the jump to the duplicate.
 *
 * Now the FIRST occurrence in document order wins in both structures: later
 * duplicates are dropped from the scan array as well as from the map, so the
 * element the scan can select is always the element the lookup returns.
 * Each drop is still warned about (first five, then a tally) — one policy,
 * applied twice, is what makes the outcome explainable.
 *
 * `quiet` suppresses those warnings for the reflow recompute, which re-runs
 * this on every observed reflow: mount is the audit point for a producer
 * defect, and a window-resize drag would otherwise turn one duplicate into
 * a console message per frame.
 *
 * **`rowIds` is the anchor authority, not the DOM (OI-0035).** Source content
 * can carry a `data-sync-id` — source Markdown is untrusted data
 * (architectural invariant 7), raw HTML blocks are translatable content that
 * reaches the pane, and DOMPurify's default keeps `data-*` attributes — and an
 * anchor the alignment map never claimed used to be indistinguishable from one
 * the renderer emitted. Every element whose id is absent from the validated
 * rows is now skipped, at mount and at every reflow recompute alike, so it is
 * inert forever: an anchor inserted AFTER mount can no longer be folded into
 * the live set by the next quiet recompute. Skips are announced under the same
 * first-five-then-a-tally policy as duplicates, and for the same reason they
 * are silent on reflow.
 *
 * **What this cannot do, stated rather than implied.** An impostor carrying a
 * LISTED id that precedes the genuine anchor in document order still wins,
 * because first-occurrence-wins has no DOM-visible discriminator to prefer one
 * over the other. That residual is why OI-0035 was closed at two layers: the
 * renderer strips the reserved namespace out of raw-HTML blocks
 * (`contracts.md` §4), so panes transync produces never contain one, and this
 * gate makes every UNLISTED id inert in any pane, whoever produced it. The
 * one case neither layer reaches is a listed impostor in a pane transync did
 * not produce — a third-party producer can still hand the engine one, and the
 * engine will drive from it.
 *
 * What this deliberately does NOT govern is the anchor count R0002-0047 asks
 * `mountSync` for. "Do these panes carry anchors at all" is a question about
 * the DOM, answered there by an ungated `querySelectorAll` before this
 * function runs; routing it through this gate would make that refusal
 * unsatisfiable, because the maps it refuses are exactly the maps for which
 * `rowIds` is empty.
 */
function collectAnchors(pane, label, rowIds, quiet) {
  const blocks = [];
  const byId = new Map();
  let duplicates = 0;
  let unlisted = 0;
  for (const el of pane.querySelectorAll("[data-sync-id]")) {
    const id = el.dataset.syncId;
    // The map gate runs FIRST, so an unlisted anchor is never reported as a
    // duplicate: the two are different defects with different owners — one is
    // a producer emitting a repeated row, the other is content claiming an
    // anchor it was never given.
    if (!rowIds.has(id)) {
      unlisted += 1;
      if (!quiet && unlisted <= 5) {
        console.warn(
          `transync: ignoring anchor "${id}" in ${label} pane — no alignment row claims it`
        );
      }
      continue;
    }
    if (byId.has(id)) {
      duplicates += 1;
      if (!quiet && duplicates <= 5) {
        console.warn(
          `transync: duplicate data-sync-id "${id}" in ${label} pane; keeping the first occurrence and ignoring this one`
        );
      }
      continue;
    }
    byId.set(id, el);
    blocks.push(el);
  }
  if (!quiet && unlisted > 5) {
    console.warn(
      `transync: ${unlisted - 5} more unlisted-anchor warnings suppressed (${label} pane)`
    );
  }
  if (!quiet && duplicates > 5) {
    console.warn(
      `transync: ${duplicates - 5} more duplicate data-sync-id warnings suppressed (${label} pane)`
    );
  }
  return { blocks, byId };
}

// The ids of every row that claims a DOM anchor. Every `sync_role` other than
// "non-sync" anchors scroll (contracts.md §3), including the ones a
// newer-minor map may add — `validateRows` has already decided that an
// unclassifiable role is either refused or treated as an anchor, so taking
// everything-but-"non-sync" here agrees with what the engine will wire.
//
// OI-0035: this set is the engine's anchor authority. `collectAnchors` skips
// any DOM element whose `data-sync-id` is not in it, so a `data-sync-id` that
// rode in on document content — source Markdown is untrusted data, raw HTML is
// translatable content, and DOMPurify's default keeps `data-*` — cannot become
// a scroll driver or a scroll target.
//
// It is also the count R0002-0047's refusal turns on, read as `.size` off the
// one set `mountSync` already built (R0009-0026). There used to be a
// `synchronizableRowCount` wrapper calling this a second time; the two always
// agreed, but "agreed" was the weaker claim and the second full scan of the
// map bought nothing. The size is the row count and not merely a distinct-id
// count because `validateRows` refuses a duplicate `source_block_id` before
// either is asked.
function synchronizableRowIds(alignmentMap) {
  const rows = Array.isArray(alignmentMap && alignmentMap.blocks)
    ? alignmentMap.blocks
    : [];
  const ids = new Set();
  for (const row of rows) {
    if (row && row.sync_role !== "non-sync") ids.add(row.source_block_id);
  }
  return ids;
}

// loadAlignment gates the schema version and the row shape; this surfaces
// map-vs-DOM drift (an anchor the map promises that a pane lacks) so an
// integration mistake is visible instead of silently degrading sync. Rows
// whose sync_role is "non-sync" (e.g. thematic breaks) carry no DOM anchor
// by design and are skipped.
function warnMapDomDrift(alignmentMap, sourceById, targetById) {
  const rows = Array.isArray(alignmentMap && alignmentMap.blocks)
    ? alignmentMap.blocks
    : [];
  let missingCount = 0;
  for (const row of rows) {
    if (!row || row.sync_role === "non-sync" || !row.source_block_id) continue;
    // Both panes are probed for the SAME id, because that is the pairing the
    // engine performs (ID identity, normative for schema 1.x — ADR-0001,
    // contracts.md §3). This used to resolve the target side through
    // `target_block_id` while `handleScroll` looked the source id up
    // directly, so on a non-identity map the two disagreed: the warning
    // reported anchors present that the engine could never reach
    // (R0003-0002). `validateRows` now refuses such a map outright, and
    // this reads the one id both code paths key on.
    const missing = [];
    if (!sourceById.has(row.source_block_id)) missing.push("source");
    if (!targetById.has(row.source_block_id)) missing.push("target");
    if (missing.length === 0) continue;
    missingCount += 1;
    if (missingCount <= 5) {
      console.warn(
        `transync: alignment map block "${row.source_block_id}" has no DOM anchor in ${missing.join(" + ")} pane`
      );
    }
  }
  if (missingCount > 5) {
    console.warn(
      `transync: ${missingCount - 5} more missing-anchor warnings suppressed`
    );
  }
}

/**
 * Probe the layout precondition instead of only documenting it (R0003-0076).
 *
 * `contracts.md` §4a requires each pane to be the `offsetParent` of the
 * blocks it renders — i.e. a non-static `position` on the pane — and every
 * scroll frame reads `offsetTop` / `offsetHeight` on that assumption. A CSS
 * refactor that drops the rule breaks nothing loudly: the mount succeeds and
 * the anchors' coordinate space silently moves to the nearest positioned
 * ancestor (typically `<body>`), after which the engine drives the partner
 * pane to the wrong place. The contract itself names that failure mode "a
 * silent layout regression"; this makes it audible.
 *
 * The property being checked is the PANE's computed position, not a fact
 * about any individual block — so it is read off the pane, and the cost stays
 * one property read per pane at mount, never one per frame (contracts.md §4a
 * budgets exactly that).
 *
 * **It used to be read off `blocks[0]` instead (R0009-0024), which is a
 * different question.** `offsetParent` names the nearest positioned ancestor,
 * so a consumer who wraps one block in a positioned element gets that wrapper
 * back for that block while the pane is configured perfectly — the old probe
 * called a correct pane broken, on evidence about a single block, and would
 * have said the same about `blocks[0]` alone however the blocks after it were
 * wrapped. Asking the pane what its `position` is answers the precondition
 * itself and cannot be fooled either way.
 *
 * The `offsetParent` read survives on the failure path only, where naming the
 * element the offsets are actually coming from is what makes the warning
 * actionable. A null one is deliberately not treated as a violation: it means
 * the anchor is `display: none`, inside a `position: fixed` subtree, or
 * detached, none of which is the misconfiguration this guards, and all of
 * which have no offset geometry to be wrong about. Nor is an `offsetParent`
 * that is the pane anyway — a `<td>`/`<th>`/`<table>` pane is one without any
 * `position` at all — since then the offsets are already pane-relative.
 */
function warnOffsetParentDrift(pane, blocks, label) {
  const probe = blocks[0];
  if (!probe) return;
  const view = pane.ownerDocument && pane.ownerDocument.defaultView;
  if (!view || typeof view.getComputedStyle !== "function") return;
  if (view.getComputedStyle(pane).position !== "static") return;
  const parent = probe.offsetParent;
  if (parent === null || parent === pane) return;
  const where = parent.tagName ? parent.tagName.toLowerCase() : String(parent);
  console.warn(
    `transync: the ${label} pane is not the offsetParent of its blocks — ` +
      `contracts.md §4a wants a non-static \`position\` on the pane, but block ` +
      `offsets are being measured against <${where}>, so scroll sync will be ` +
      `misaligned`
  );
}

// The schema version this engine was written against. A map with the same
// major but a newer minor/patch is forward-compat drift: still accepted, but
// surfaced with a console.warn (OI-0024) instead of the silent console.debug.
const KNOWN_SCHEMA = { major: 1, minor: 3, patch: 0 };

// The `sync_role` values this engine understands (contracts.md §3). It only
// ever acts on "non-sync" — every other role means "this row anchors
// scroll" — but a role outside this set is a row this engine cannot
// classify, so it is not quietly assumed to be an anchor. Kept in lockstep
// with `SyncRole` in transync-syntax::align by
// crates/transync-cli/tests/sync_js_drift.rs.
const KNOWN_SYNC_ROLES = new Set(["anchor", "container", "child-only", "non-sync"]);

// TRACE: contracts.md §3
function loadAlignment(alignmentMap) {
  if (!alignmentMap || typeof alignmentMap !== "object") {
    console.warn("transync: alignment map missing or malformed");
    return false;
  }
  const v = String(alignmentMap.schema_version || "");
  // R0008-0045: validate the full x.y.z semver form before trusting the
  // major, so "1", "1garbage", "1.foo" are rejected rather than accepted.
  //
  // R0002-0045: each component is digit-BOUNDED. Unbounded, a 400-digit
  // minor converted to Infinity, and Infinity is not a version this engine
  // can reason about — it compares as "newer than KNOWN_SCHEMA" forever, so
  // the map was accepted with the forward-drift warning instead of being
  // refused as the malformed input it is. Nine digits is orders of magnitude
  // past any schema this project will publish and keeps every component a
  // safe integer by construction, so `Number` below cannot lose precision.
  const match = /^(\d{1,9})\.(\d{1,9})\.(\d{1,9})$/.exec(v);
  if (!match || match[1] !== "1") {
    console.warn(
      `transync: rejecting alignment map with unknown major schema_version=${v}`
    );
    return false;
  }
  // OI-0024: a newer minor/patch than KNOWN_SCHEMA is forward-compat drift —
  // accept it, but warn visibly rather than logging at debug level.
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  const forwardDrift =
    minor > KNOWN_SCHEMA.minor ||
    (minor === KNOWN_SCHEMA.minor && patch > KNOWN_SCHEMA.patch);
  // R0001-0043: the version says the map SPEAKS this schema; the rows say
  // whether it can actually drive synchronization. Both gates run before
  // the panes are wired.
  if (!validateRows(alignmentMap.blocks, forwardDrift)) {
    return false;
  }
  if (forwardDrift) {
    console.warn(
      `transync: alignment map schema_version=${v} is newer than this engine ` +
        `(${KNOWN_SCHEMA.major}.${KNOWN_SCHEMA.minor}.${KNOWN_SCHEMA.patch}); ` +
        `proceeding, but sync may be incomplete`
    );
    return true;
  }
  console.debug(`transync: alignment map loaded (schema_version=${v})`);
  return true;
}

/**
 * Minimal row-shape gate (R0001-0043).
 *
 * Before it, a map that passed the version check was trusted whole. One
 * with no `blocks` array — or rows missing the id every consumer keys on —
 * then produced an empty row list at each call site: `warnMapDomDrift`
 * iterated nothing, and the panes were paired purely by whatever
 * `data-sync-id` values the DOM happened to carry. That degradation is
 * indistinguishable from working sync until the two panes disagree, so an
 * unusable map is now refused with a console message naming the offending
 * row and the reason.
 *
 * Only what synchronization actually needs is required: `blocks` an array,
 * every row a non-empty string `source_block_id`, unique across the map,
 * a `target_block_id` (when present) equal to it, plus a `sync_role` this
 * engine can classify. Ranges, orders, kinds and statuses belong to the
 * renderer and to the demo's edit model — they are deliberately not
 * policed here, so a map this engine accepts stays a superset of the maps
 * it can drive.
 *
 * **ID identity is one of those requirements, not an assumption
 * (R0003-0002).** The engine pairs anchors by identical `data-sync-id`;
 * schema 1.x guarantees that (ADR-0001 — ids survive translation), and
 * the pairing is normative rather than incidental. A row whose
 * `target_block_id` differs from its `source_block_id` therefore describes
 * a pairing this engine will not perform, and the honest outcome is the
 * same one every other undrivable map gets: refusal with a reason, not a
 * mount that renders and never syncs. Reserving the indirection for a
 * future divergence revision is what makes refusing it today correct —
 * the field stays in the wire format, and the revision that gives it
 * meaning arrives as a schema bump this engine will not silently
 * misread.
 *
 * `forwardDrift` (a same-major map newer than `KNOWN_SCHEMA`) relaxes
 * exactly two rules, both for the same reason. contracts.md §3 lets a
 * minor bump ADD enumerated values and requires consumers to accept newer
 * minors, so an unrecognized `sync_role` there is a value from the future:
 * warn and treat the row as a scroll anchor. A non-identity
 * `target_block_id` in a newer-minor map is read the same way — warn, and
 * keep pairing by the source id, which is all this engine knows how to do.
 * Either value in an in-band map is corruption, and is refused.
 */
function validateRows(blocks, forwardDrift) {
  if (!Array.isArray(blocks)) {
    console.warn("transync: rejecting alignment map — `blocks` is not an array");
    return false;
  }
  const seen = new Set();
  let unknownRoles = 0;
  let nonIdentity = 0;
  for (let i = 0; i < blocks.length; i += 1) {
    const row = blocks[i];
    const at = `block #${i}`;
    if (!row || typeof row !== "object") {
      console.warn(
        `transync: rejecting alignment map — ${at} is not an object`
      );
      return false;
    }
    const id = row.source_block_id;
    if (typeof id !== "string" || id === "") {
      console.warn(
        `transync: rejecting alignment map — ${at} has no string source_block_id`
      );
      return false;
    }
    if (seen.has(id)) {
      console.warn(
        `transync: rejecting alignment map — duplicate source_block_id "${id}" at ${at}`
      );
      return false;
    }
    seen.add(id);
    // ID identity (R0003-0002). An absent, null or empty `target_block_id`
    // is not a violation: the field is optional in a map, and omitting it
    // says nothing that contradicts the identity.
    const targetId = row.target_block_id;
    // R0009-0022: anything else present is not a block id at all. The
    // identity check below is `typeof targetId === "string"`-gated, so a
    // `42` or a `{}` slipped past the very refusal the string "42" would
    // have triggered — silently, and then failed later through implicit
    // string coercion at a lookup, which is the hard-to-diagnose partial
    // sync this gate exists to prevent. Refused whatever the schema minor
    // says, exactly like the non-string `sync_role` below: contracts.md §3
    // lets a newer minor add enumerated VALUES, never change a field's JSON
    // type, so there is no forward-compat reading of this.
    if (
      targetId !== undefined &&
      targetId !== null &&
      typeof targetId !== "string"
    ) {
      console.warn(
        `transync: rejecting alignment map — ${at} ("${id}") has a ` +
          `non-string target_block_id`
      );
      return false;
    }
    if (
      typeof targetId === "string" &&
      targetId !== "" &&
      targetId !== id
    ) {
      if (!forwardDrift) {
        console.warn(
          `transync: rejecting alignment map — ${at} pairs source_block_id ` +
            `"${id}" with target_block_id "${targetId}"; schema 1.x pairs ` +
            `anchors by identical id, so this map cannot be synchronized`
        );
        return false;
      }
      nonIdentity += 1;
      if (nonIdentity <= 5) {
        console.warn(
          `transync: alignment map ${at} pairs "${id}" with target_block_id ` +
            `"${targetId}"; pairing by the source id anyway (forward-compat drift)`
        );
      }
    }
    const role = row.sync_role;
    if (typeof role !== "string") {
      console.warn(
        `transync: rejecting alignment map — ${at} ("${id}") has a non-string sync_role`
      );
      return false;
    }
    if (!KNOWN_SYNC_ROLES.has(role)) {
      if (!forwardDrift) {
        console.warn(
          `transync: rejecting alignment map — ${at} ("${id}") has unknown sync_role "${role}"`
        );
        return false;
      }
      unknownRoles += 1;
      if (unknownRoles <= 5) {
        console.warn(
          `transync: alignment map ${at} ("${id}") has unknown sync_role "${role}"; ` +
            `treating it as a scroll anchor (forward-compat drift)`
        );
      }
    }
  }
  if (unknownRoles > 5) {
    console.warn(
      `transync: ${unknownRoles - 5} more unknown-sync_role warnings suppressed`
    );
  }
  if (nonIdentity > 5) {
    console.warn(
      `transync: ${nonIdentity - 5} more non-identity target_block_id warnings suppressed`
    );
  }
  return true;
}

function wirePane(ctx) {
  const { pane, label, state } = ctx;
  const onScroll = () => {
    if (state.rafScheduled[label]) return;
    state.rafScheduled[label] = true;
    state.scrollRafId[label] = requestAnimationFrame(() => {
      state.scrollRafId[label] = null;
      state.rafScheduled[label] = false;
      handleScroll(ctx);
    });
  };
  // The programmatic-scroll lock would otherwise silence real
  // wheel / touch / pointer input on the auto-followed pane, so a
  // user trying to take over mid-animation gets ignored and the
  // smooth loop snaps the pane back. On a real user gesture we
  // release the lock here and let this pane immediately become
  // the next driver.
  const releaseDriverLock = () => {
    // A real gesture on this pane makes it the driver — which is also what
    // a later reflow recompute must re-read, or it would drag this pane
    // back to wherever the other one points. Written ahead of the fast
    // path below because the takeover is true whether or not there was a
    // follow in flight to cancel.
    state.lastDriver = label;
    // Fast path: wheel can fire 60+ Hz during continuous trackpad
    // scrolling. Skip the writes when nothing is currently locked
    // or queued for this pane.
    if (
      state.smoothLoopRafId[label] == null
      && state.targets[label] == null
      && state.lockUntil[label] === 0
      && !state.smoothLoopActive[label]
    ) {
      return;
    }
    if (state.smoothLoopRafId[label] != null) {
      cancelAnimationFrame(state.smoothLoopRafId[label]);
      state.smoothLoopRafId[label] = null;
    }
    state.smoothLoopActive[label] = false;
    state.targets[label] = null;
    state.lastAssigned[label] = null;
    state.lastAssignedHeight[label] = null;
    state.lockUntil[label] = 0;
  };
  pane.addEventListener("scroll", onScroll, { passive: true });
  pane.addEventListener("wheel", releaseDriverLock, { passive: true });
  pane.addEventListener("touchstart", releaseDriverLock, { passive: true });
  pane.addEventListener("pointerdown", releaseDriverLock, { passive: true });
  pane.addEventListener("keydown", releaseDriverLock);
  return {
    destroy() {
      pane.removeEventListener("scroll", onScroll);
      pane.removeEventListener("wheel", releaseDriverLock);
      pane.removeEventListener("touchstart", releaseDriverLock);
      pane.removeEventListener("pointerdown", releaseDriverLock);
      pane.removeEventListener("keydown", releaseDriverLock);
      if (state.scrollRafId[label] != null) {
        cancelAnimationFrame(state.scrollRafId[label]);
        state.scrollRafId[label] = null;
        state.rafScheduled[label] = false;
      }
      // Cancels the in-flight smooth-scroll RAF and clears state for
      // this pane so a queued step doesn't fire after destroy().
      releaseDriverLock();
    },
  };
}

/**
 * Has something other than our own smooth loop moved `pane`?
 *
 * True when the pane sits further than [`EXTERNAL_SCROLL_TOLERANCE_PX`] from
 * the value that loop last read back. `null` means no loop has ever written
 * this pane, so there is nothing to be away from.
 *
 * TRACE: ti 7936c2
 */
/**
 * Has something other than our own smooth loop **scrolled** `pane`?
 *
 * Two conditions, and the second is the one that took a regression to find.
 * The position must have moved away from what the loop last read back — and
 * the pane's `scrollHeight` must be **unchanged**, because a content reflow
 * moves blocks under a fixed scrollTop (and can make the browser adjust
 * scrollTop itself to preserve anchoring) without the reader having scrolled
 * anything. The engine already owns that case: the disclosure mirror and the
 * ResizeObserver recompute re-drive the follower deliberately. Treating a
 * reflow as a takeover flipped that recompute's direction and moved the
 * reader's own pane — `engine.spec.js` leg j catches it, and did.
 *
 * The baseline outlives the loop that set it, which is what covers the 90 ms
 * post-settle lock window — the window `scn13.spec.js` leg k lands in, where
 * a scroll would otherwise be absorbed with no loop running and no baseline to
 * notice it. Keeping it is only safe *because* of the height condition: the
 * two rules were adopted together, and each without the other turns one of the
 * two browser legs red (measured, not assumed — leg j without the height
 * check, leg k without the persistence).
 *
 * TRACE: ti 7936c2
 */
function movedExternally(pane, label, state) {
  const assigned = state.lastAssigned[label];
  if (assigned == null) return false;
  if (state.lastAssignedHeight[label] !== pane.scrollHeight) return false;
  return Math.abs(pane.scrollTop - assigned) > EXTERNAL_SCROLL_TOLERANCE_PX;
}

/**
 * Drop this pane's in-flight follow and its lock, and make it the driver.
 *
 * Called when the pane has been scrolled by someone else: the destination it
 * was animating toward describes where the reader *was*, and the lock exists
 * only to absorb our own cascade events. Keeping either would fight the
 * scroll that just arrived.
 *
 * **Deliberately does not touch `state.lastDriver`.** `handleScroll` sets
 * that once it has actually decided to drive, and `releaseDriverLock` sets it
 * on a real gesture. Writing it here instead made a *follower's* content
 * reflow — a `<details>` mirror growing the pane — read as a takeover and
 * flipped the recompute's direction, which `engine.spec.js` leg j catches.
 *
 * TRACE: ti 7936c2
 */
function abandonFollow(label, state) {
  if (state.smoothLoopRafId[label] != null) {
    cancelAnimationFrame(state.smoothLoopRafId[label]);
    state.smoothLoopRafId[label] = null;
  }
  state.smoothLoopActive[label] = false;
  state.targets[label] = null;
  state.lastAssigned[label] = null;
  state.lastAssignedHeight[label] = null;
  state.lockUntil[label] = 0;
}

function handleScroll(ctx) {
  const { pane, partner, label, partnerLabel, state, blocks, partnerById } = ctx;
  // ti 7936c2: an external scroll is detectable HERE, where its own event is
  // the signal — the pane is no longer where our loop last put it. Checked
  // BEFORE the lock, because the lock is the thing that would swallow it:
  // `releaseDriverLock` covers a gesture takeover, but it listens for
  // wheel/touchstart/pointerdown/keydown and a scroll can arrive with none of
  // them (`scrollIntoView`, find-in-page, scroll restoration, an embedder, a
  // test harness). Before this, such a scroll was lerped back to a stale
  // destination AND suppressed, so it neither survived nor drove the partner.
  if (movedExternally(pane, label, state)) {
    abandonFollow(label, state);
  }
  // Only ignore scroll events on THIS pane that come from our own
  // programmatic scrollTop assignments. User-driven scrolls on the
  // OTHER pane (which is the input side here) must not be blocked.
  if (performance.now() < state.lockUntil[label]) {
    return;
  }

  const active = activeBlockWithProgress(pane, blocks);
  if (!active) return;

  // ID-identity pairing, normative for schema 1.x (ADR-0001, contracts.md
  // §3): the partner is the element carrying the SAME id. The map's
  // source/target indirection is deliberately not routed through here —
  // `validateRows` has already refused any row that contradicts the
  // identity, so the map and this lookup cannot disagree (R0003-0002).
  const partnerEl = partnerById.get(active.id);
  if (!partnerEl) return;

  // This pane is now the driver, and stays recorded as such so a reflow
  // recompute knows which side to re-read.
  state.lastDriver = label;

  // offsetTop / offsetHeight are pane-relative because the pane element
  // is `position: relative`, making it the offsetParent of every block.
  // This is a precondition for `mountSync` consumers — see
  // `docs/architecture/contracts.md` §4a. Reading offsetTop/offsetHeight
  // avoids the two `getBoundingClientRect` calls per RAF that the older
  // engine paid (each one can force a layout flush).
  const partnerBlockTop = partnerEl.offsetTop;
  const partnerBlockHeight = Math.max(1, partnerEl.offsetHeight);
  const desiredScrollTop =
    partnerBlockTop +
    active.progress * partnerBlockHeight -
    REFERENCE_OFFSET_PX;

  state.targets[partnerLabel] = desiredScrollTop;
  ensureSmoothLoop(partner, partnerLabel, state);
}

/**
 * Per-pane animation loop. Idempotent — kicking it twice while it is
 * already running is a no-op. Reads `state.targets[label]` each frame
 * so the source-side scroll handler can keep updating the destination
 * mid-animation; the loop just keeps closing the gap toward the
 * latest value.
 *
 * Arms `state.lockUntil[label]` (the LOCAL pane lock) on every frame
 * that actually moves the pane, so the cascade scroll event fired by
 * `pane.scrollTop = ...` does not bounce back into `handleScroll(pane)`
 * and try to drive the OTHER pane.
 *
 * **Abandons itself when something else moves the pane** (ti `7936c2`):
 * each moving frame records the scrollTop it read back, and a pane found
 * more than `EXTERNAL_SCROLL_TOLERANCE_PX` away from that value has been
 * scrolled by someone else, so the loop drops its destination and its
 * lock. `releaseDriverLock` covers the gesture-driven takeover, but it is
 * bound to wheel/touchstart/pointerdown/keydown and a scroll can arrive
 * with none of them; before this check such a scroll was lerped away AND
 * suppressed, because the per-frame lock arming also silences
 * `handleScroll`.
 *
 * TRACE: SCN-13
 */
function ensureSmoothLoop(pane, label, state) {
  if (state.smoothLoopActive[label]) return;
  state.smoothLoopActive[label] = true;

  // A fresh loop owns nothing yet: any position the pane is in right now is
  // legitimate, and comparing against a previous loop's last write would
  // abandon this one on its first frame.
  state.lastAssigned[label] = null;
  state.lastAssignedHeight[label] = null;

  const step = () => {
    state.smoothLoopRafId[label] = null;
    const target = state.targets[label];
    if (target == null) {
      state.smoothLoopActive[label] = false;
      state.lastAssigned[label] = null;
      state.lastAssignedHeight[label] = null;
      return;
    }
    const current = pane.scrollTop;
    // ti 7936c2: has anything other than this loop moved the pane since our
    // last write? `releaseDriverLock` handles the gesture-driven case, but it
    // listens for wheel/touchstart/pointerdown/keydown, and a scroll can
    // arrive with none of them — `scrollIntoView`, find-in-page, the
    // browser's scroll restoration, an embedder setting scrollTop, a test
    // harness. Left unchecked the loop lerps such a pane back to a
    // destination the reader has already left, and because it re-arms
    // `lockUntil` on every moving frame it also suppresses `handleScroll`,
    // so the scroll neither survives nor drives the partner. Abandoning is
    // the whole repair: the destination is stale by definition, and the pane
    // that was just scrolled is by definition the new driver.
    if (movedExternally(pane, label, state)) {
      abandonFollow(label, state);
      return;
    }
    // Browsers silently clamp scrollTop assignments to
    // [0, scrollHeight - clientHeight]; clamp the target ourselves so
    // an unreachable target cannot spin this loop forever.
    const maxScroll = Math.max(0, pane.scrollHeight - pane.clientHeight);
    const clamped = Math.min(Math.max(target, 0), maxScroll);
    const delta = clamped - current;
    if (Math.abs(delta) < SETTLE_THRESHOLD_PX) {
      // Settle exactly and stop the loop. Local lock decays 90 ms
      // later, after which user input on this pane is honored again.
      state.lockUntil[label] = performance.now() + PROGRAMMATIC_SCROLL_LOCK_MS;
      pane.scrollTop = clamped;
      // KEPT past settle, deliberately. This is still where we last put the
      // pane, and it is the only baseline that makes a scroll arriving inside
      // the 90 ms lock window detectable rather than silently absorbed — the
      // exact window `scn13.spec.js` leg k lands in. Safe to keep only
      // because `movedExternally` also requires an unchanged scrollHeight, so
      // a later content reflow does not read as a scroll.
      state.lastAssigned[label] = pane.scrollTop;
      state.lastAssignedHeight[label] = pane.scrollHeight;
      state.targets[label] = null;
      state.smoothLoopActive[label] = false;
      return;
    }
    state.lockUntil[label] = performance.now() + PROGRAMMATIC_SCROLL_LOCK_MS;
    pane.scrollTop = current + delta * SMOOTHING_FACTOR;
    if (pane.scrollTop === current) {
      // Integer-quantizing browsers can swallow sub-pixel lerp steps;
      // snap to the clamped target and end rather than spin.
      pane.scrollTop = clamped;
      state.lastAssigned[label] = pane.scrollTop;
      state.lastAssignedHeight[label] = pane.scrollHeight;
      state.targets[label] = null;
      state.smoothLoopActive[label] = false;
      return;
    }
    // Read back rather than trusting the assignment: a browser that
    // quantizes a fractional scrollTop would otherwise look like external
    // interference on the very next frame.
    state.lastAssigned[label] = pane.scrollTop;
    state.lastAssignedHeight[label] = pane.scrollHeight;
    state.smoothLoopRafId[label] = requestAnimationFrame(step);
  };

  state.smoothLoopRafId[label] = requestAnimationFrame(step);
}

/**
 * Return `{ id, progress }` for the block whose bounds straddle the
 * pane's reference line. `progress` is the 0..1 fraction of the block
 * that the user has scrolled past.
 *
 * If no block straddles the line (e.g. between blocks during a fast
 * fling), fall back to the topmost-visible block with progress = 0.
 *
 * TRACE: SCN-13
 */
function activeBlockWithProgress(pane, blocks) {
  if (blocks.length === 0) return null;
  // All coordinates here are in the pane's scroll-coord space:
  // offsetTop is pane-relative (because the pane is `position: relative` —
  // see contracts.md §4a), and pane.scrollTop is the current scroll offset.
  // The reference line sits REFERENCE_OFFSET_PX below the visible top.
  const scrollTop = pane.scrollTop;
  const visibleBottom = scrollTop + pane.clientHeight;
  const ref = scrollTop + REFERENCE_OFFSET_PX;

  let topmostVisible = null;
  let topmostVisibleTop = Infinity;

  // Walk every block, not until the first DOM-order overshoot —
  // multi-column or nested-wrapper layouts can produce DOM order that
  // doesn't match geometric order, and breaking early would skip
  // valid candidates.
  for (const el of blocks) {
    const top = el.offsetTop;
    const height = el.offsetHeight;
    const bottom = top + height;
    if (top > visibleBottom) continue;
    if (bottom < scrollTop) continue;

    if (top <= ref && bottom >= ref) {
      const progress = (ref - top) / Math.max(1, height);
      return {
        id: el.dataset.syncId,
        progress: Math.max(0, Math.min(1, progress)),
      };
    }

    if (top > ref && top < topmostVisibleTop) {
      topmostVisibleTop = top;
      topmostVisible = el;
    }
  }

  if (topmostVisible) {
    return { id: topmostVisible.dataset.syncId, progress: 0 };
  }
  return null;
}

/**
 * Wrapper around `localStorage.getItem` / `setItem` that returns `null`
 * (or no-ops on set) when access throws. localStorage can throw a
 * SecurityError in sandboxed iframes, private-mode browsers with
 * restricted storage, and security-policy contexts.
 */
export const safeStorage = {
  get(key) {
    try {
      return localStorage.getItem(key);
    } catch (_) {
      return null;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(key, value);
    } catch (_) {
      /* ignore */
    }
  },
};

// Upper bound on one artifact fetch — connection, response and body read
// together (R0002-0052). `fetch` has no timeout of its own, so a server that
// accepts the connection and then stalls used to hang a boot forever, in the
// "loading" state, with nothing on the console to say why. Generous enough
// that a slow link loading a large pane fragment is never cut off.
const FETCH_TIMEOUT_MS = 20000;

/**
 * `fetch(path)` that throws on non-2xx responses before reading the
 * body. The default `fetch` resolves on any HTTP status, so an error
 * page would otherwise be mounted as the pane's HTML content.
 *
 * The request is bounded (R0002-0052): it aborts after `timeoutMs` and says
 * so by name rather than hanging. The bound covers the body read as well as
 * the response, because a stalled body is the same hang seen one step later.
 *
 * Pass `signal` to join a caller-owned `AbortController` (R0002-0053): a boot
 * that fetches several artifacts at once can then cancel the siblings the
 * moment one of them fails, instead of leaving them running against a load
 * that is already lost.
 *
 * @param {string} path
 * @param {(r: Response) => Promise<unknown>} parser  e.g. `r => r.text()` or `r => r.json()`
 * @param {{ signal?: AbortSignal, timeoutMs?: number }} [options]
 */
export async function fetchOk(path, parser, options) {
  const { signal, timeoutMs = FETCH_TIMEOUT_MS } = options || {};
  // One controller per call, so the timeout aborts THIS request only; the
  // caller's signal (if any) is relayed into it rather than passed through,
  // which keeps both cancellation reasons distinguishable below.
  const control = new AbortController();
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    control.abort();
  }, timeoutMs);
  const relay = () => control.abort();
  if (signal) {
    if (signal.aborted) control.abort();
    else signal.addEventListener("abort", relay);
  }
  try {
    const r = await fetch(path, { signal: control.signal });
    if (!r.ok) {
      throw new Error(`failed to load ${path}: HTTP ${r.status}`);
    }
    // Awaited inside the try so a body read that throws — or is aborted by
    // the timer still armed above — is reported the same way as the request.
    return await parser(r);
  } catch (err) {
    if (timedOut) {
      throw new Error(`timed out loading ${path} after ${timeoutMs} ms`);
    }
    if (signal && signal.aborted) {
      throw new Error(`cancelled loading ${path}`);
    }
    throw err;
  } finally {
    clearTimeout(timer);
    if (signal) signal.removeEventListener("abort", relay);
  }
}
