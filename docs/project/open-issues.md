# Open Issues

Issues accepted but pending verification or requiring larger architectural changes.
Remove entries once fully resolved; resolved entries with audit value move to `open-issues-archive.md`.

***

## OI-0016: Active-block selection scans every block on every scroll frame

- **Source:** R0006-0090 (Review 0006) (review archived and removed)
- **Date:** 2026-07-10
- **Decision:** ACCEPT (track — user routing in the Review 0006 gate)
- **Status:** OPEN

### Problem

`activeBlockWithProgress` linearly scans all blocks per RAF-coalesced scroll frame. A sorted-offset cache + binary search would be O(log n), but the code comment deliberately declines the geometric-monotonicity assumption (multi-column / nested-wrapper layouts), and a cache needs invalidation on resize/reflow.

### Impact

No observed jank; cost grows with document size. Optimize only when profiling shows scroll-frame overruns on large documents.

### Required Actions

1. If profiling shows jank: design the offset cache (invalidation on resize/reflow/mutation) and decide the non-monotonic-layout policy. *(2026-08-09, ticket `d3acc3`: the invalidation half got cheaper — the engine now observes `ResizeObserver` on both panes, `document.fonts.ready` and image `load`/`error`, and already re-collects its anchor sets on each. An offset cache would hang off that same recompute rather than needing hooks of its own. The non-monotonic-layout policy is untouched, and this issue stays gated on profiling.)*

***

## OI-NNNN: <Title>

- **Source:** RNNNN-#### (Review NNNN)
- **Date:** YYYY-MM-DD
- **Decision:** ACCEPT
- **Status:** OPEN | RESOLVED (YYYY-MM-DD)
- **Resolution:** <RNNNN-#### that resolved it, if applicable>

### Problem

<Description of the issue - what was found and why it matters>

### Impact

<What could go wrong if not addressed>

### Required Actions

1. <Specific action item>
2. <Specific action item>

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- <Links to related decisions or ignored issues>

***

## OI-0035: A `data-sync-id` injected through raw HTML can pre-claim a real block's anchor

- **Source:** R0002-0018 (Review 0002) (review archived and removed)
- **Date:** 2026-08-08
- **Decision:** ACCEPT (track — the mechanism is confirmed, but which layer should close it is a design choice)
- **Status:** OPEN
- **Resolution:** —

### Problem

The browser sync engine builds its anchor sets by collecting **every**
`data-sync-id` in each pane's DOM. Alignment rows are used for validation and
warnings, but they are not the source of the anchor set, so membership in the
map does not gate what can become a scroll driver or target.

Half of that is a recorded decision and is not in question: ID-identity
pairing (the runtime pairs anchors by identical `data-sync-id` rather than
routing through the map's source/target indirection) is normative under
`d3acc3` / OI-0015. The unrecorded half is what this entry is for. Source
Markdown is untrusted data (architectural invariant 7), and raw HTML blocks
are translatable, structurally-owned content that reaches the rendered pane.
A `data-sync-id` attribute written into a source document therefore survives
the default DOMPurify configuration and lands in the DOM as an anchor that
the engine cannot distinguish from one the renderer emitted — so it can
**pre-claim the id of a real block** and become that block's scroll driver.

### Impact

Scroll synchronization can be steered by document content rather than by the
alignment map: the wrong pane region tracks the reader, or a genuine block's
anchor is shadowed. It is a correctness-of-presentation failure, not a data
loss or code-execution one — DOMPurify still bounds what markup renders.
Reachability requires a source document containing crafted raw HTML, which
invariant 7 says to expect rather than to rule out.

### Required Actions

1. Decide the layer, which is the reason this is tracked rather than fixed:
   (a) strip or namespace `data-*` attributes at sanitize time so injected
   anchors never reach the DOM; (b) build the anchor sets from validated
   alignment rows so DOM anchors outside the map are inert; or (c) both, if
   defense in depth is wanted at the render boundary and the engine boundary.
2. Implement the chosen layer in `web/js/sync.js` **and** its byte-identical
   embedded CLI twin in the same commit; the drift tests weld the pair.
3. Extend the browser suite (`scripts/test-browser.sh`) with a document whose
   raw HTML carries a `data-sync-id` colliding with a real block id.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- `d3acc3` / OI-0015 — the ID-identity pairing decision this does **not**
  reopen; its packaging half is settled. **(Landed 2026-08-09; it changes
  nothing here on purpose.)** The anchor sets are still built by
  `pane.querySelectorAll("[data-sync-id]")`, so DOM membership still decides
  what can drive scroll and map membership still does not — every one of
  actions (a), (b) and (c) below is exactly as open as it was. Two facts for
  whoever routes it. The pass made route **(b)** slightly *cheaper*: reflow
  recompute re-collects anchors, so `collectAnchors` is now the single choke
  point every anchor set on either pane passes through, at mount and on every
  reflow, and it already takes a per-call policy argument — a map-membership
  gate lands there once and covers both. And the pass touched the existing
  duplicate-id warning that is today's only signal of a shadowing attempt: the
  *recompute's* re-collection is deliberately quiet (a resize drag would
  otherwise replay one warning per frame), while the **mount-time** warning —
  the one an injected anchor in the initially rendered document actually trips
  — is unchanged, as is the first-occurrence-wins policy that decides which of
  two same-id anchors survives. No reflow signal can introduce an anchor.
  **Correction (2026-08-09), same day, from this pass's own review:** the
  sentence that stood here — *"so nothing became reachable that was not
  reachable before"* — overstated that. Reflow inserts no anchor, but the quiet
  recompute **activates** one that entered the DOM after mount. Before this
  pass such an anchor stayed inert until a `destroy()` + re-mount, which
  re-runs the duplicate-id audit; now the next reflow signal — an `<img>` load
  is enough, and `controller.refresh()` schedules the same quiet recompute —
  folds it into the live anchor set with the duplicate warning suppressed and
  no audit at any point (`engine.spec.js` `h` phase 2 drives a post-mount
  `e-0006` with no re-mount). Under **this entry's** threat model — injection
  carried by the rendered source document, therefore present at mount — the
  mount-time audit still fires and exposure is unchanged. What is weaker is the
  doctrine the layer choice leans on: activation no longer implies an audited
  mount, so "mount is the audit point" is now an argument **for** route (b),
  not a substitute for it.
- ADR-0018 — raw HTML as translatable, structurally-owned content.
- DCR-0022 — the Review 0002 hardening pass that routed this to tracking.

***

## OI-0037: Provider-returned payloads bypass the parser's nesting intake guard

- **Source:** R0003-0004, R0003-0005, R0003-0088 (Review 0003) (review archived and removed)
- **Date:** 2026-08-09
- **Decision:** ACCEPT (track — defense in depth; no reachable failure today)
- **Status:** OPEN
- **Resolution:** —

### Problem

Source Markdown enters through `parser::intake`, which carries the depth
pre-scan ticket `07844d` added. Provider-returned payloads do not: they call
`comrak::parse_document` directly at five sites —
`validate/fragment_reparse.rs`, `validate/inline.rs`, `validate/per_kind.rs`,
`structure.rs` and `validate/full_reparse.rs`. So the ceiling that exists
because stack exhaustion is an **uncatchable abort** governs what the user
wrote but not what the model returned.

The impact the reviewer claimed — a malicious or defective `Translator`
exhausting the stack — was **refuted** by verification, and that refutation is
the reason this is tracked rather than fixed. The measured, regression-pinned
record in `parser/depth.rs` establishes that comrak 0.27's block parse is
iterative, and every untrusted-tree walker in this workspace was made
iterative for exactly this provider-payload path; memory is linear in the
32 MiB response cap regardless of nesting.

### Impact

None today. The value of closing it is that today's safety rests on **comrak
internals**, not on anything this repository asserts: a dependency upgrade
could reintroduce a recursive block parse and nothing here would go red. The
principle worth holding is that a translated payload should not bypass a
safety ceiling the source must satisfy.

### Required Actions

1. Route provider-payload parsing through one guarded entry point that
   applies the same depth and size policy `parser::intake` applies, rather
   than five direct `comrak::parse_document` calls.
2. Convert a refusal into `ReparseFailure` with attributed fallback ids, so
   the existing retry-then-fallback machinery handles it rather than a new
   error path.
3. R0003-0088 is this work's test gap and cannot land before it: a test for a
   guard cannot exist until the guard does.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- Ticket `07844d` — the source-side depth ceiling and the measurement that
  refutes the abort claim here.
- ADR-0009 — the retry/fallback contract a refusal must ride.
- DCR-0025 — the Review 0003 pass that routed this to tracking.

***

## OI-0038: A fully-warm run cannot start offline, because credentials are demanded before the cache is consulted

- **Source:** R0004-0069 (Review 0004) (review archived and removed)
- **Date:** 2026-08-13
- **Decision:** ACCEPT (track — whether offline warm-cache rerun is a supported scenario is a product call that decides the fix's shape)
- **Status:** OPEN
- **Resolution:** —

### Problem

The pipeline constructs its `Translator` — and therefore demands an API key —
**before** the cache is consulted. So a run in which every unit is a cache hit,
which would make zero provider calls, still cannot start without credentials.

`DCR-0028`'s own tests prove such a run exists: they assert that a second
identical run dispatches **zero** provider calls. What the design never
recorded is whether that run is expected to be possible **offline**, with no
key present. Nothing in the records answers it either way, which is why this is
tracked rather than fixed.

### Impact

Bounded and non-destructive: the run fails to start with a credential error
rather than doing anything wrong. But it makes the disk cache's most valuable
property — a document is paid for once — unavailable in the situation where it
is most obviously wanted: re-rendering a translated document on a machine with
no key, or offline.

### Required Actions

1. **Decide the product question first:** is an offline run against a fully
   warm cache a supported scenario? Everything below depends on the answer, and
   answering it in the record is worth as much as the code.
2. If yes, choose the mechanism: construct the `Translator` lazily, or resolve
   it at first dispatch. Both are more than a few lines.
3. Define the semantics of the case that only exists once the fix lands — a run
   that started without credentials and then takes a cache **miss**. Fail at
   that point? Fall back to source and mark it? That choice belongs with the
   ADR-0009 retry/fallback contract rather than being invented at the call site.
4. Whatever is decided, record it: `DCR-0028` should say what a warm run
   requires, since its own tests are what make the gap visible.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0028 — the disk-backed cache design, whose zero-provider-call tests
  surface this.
- ADR-0009 — the retry/fallback contract that owns the miss-without-credentials
  semantics.
- DCR-0030 — the Review 0004 pass that routed this to tracking.

***

## Open Issues Summary

| Issue ID | Title                                                  | Status   | Severity |
|----------|--------------------------------------------------------|----------|----------|
| OI-0016  | Active-block selection scans per scroll frame            | OPEN   | Low      |
| OI-0035  | Injected `data-sync-id` can pre-claim a real block's anchor | OPEN (2026-08-08) | Low |
| OI-0037  | Provider payloads bypass the parser's nesting intake guard | OPEN (2026-08-09) | Low |
| OI-0038  | A fully-warm run cannot start offline — credentials precede the cache | OPEN (2026-08-13) | Low |
