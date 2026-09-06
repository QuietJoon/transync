/**
 * transync — WASM render+edit demo controller.
 *
 * The browser-side twin of the CLI's `--html-out` shell: instead of
 * fetching pre-rendered `source.html` / `target.html`, it fetches the two
 * Markdown payloads plus the alignment map and renders both panes with the
 * *same* Rust renderer through `transync-wasm` (ADR-0019). JS still owns
 * only interaction — no second Markdown parser enters the system.
 *
 * Import layout (Track C task 4 decision). The wasm artifacts live in
 * `web/wasm/` (where `scripts/build-wasm.sh` emits them) and this module
 * lives in `web/js/`, so the glue is imported as `../wasm/transync_wasm.js`.
 * `scripts/test-browser.sh` mirrors exactly that shape into the Playwright
 * fixture dir (`js/wasm-demo.js` + `wasm/transync_wasm*`), so the same
 * relative specifiers resolve in the repo tree and in the fixture. The
 * wasm-bindgen glue resolves its own `.wasm` via
 * `new URL('transync_wasm_bg.wasm', import.meta.url)`, i.e. as a sibling of
 * the glue — which is why the two artifacts must stay in one directory.
 *
 * Fail-closed posture (invariant 7 / OI-0001): rendered HTML is still
 * untrusted-source content, so every `innerHTML` write goes through
 * `window.DOMPurify` and refuses to mount when DOMPurify is absent. The
 * demo additionally refuses to boot when the wasm module's
 * `schema_version()` or the fetched map's `schema_version` disagrees with
 * the constant mirrored below.
 *
 * TRACE: SCN-13
 * TRACE: ADR-0019
 */

import init, { render_pair, rebuild, schema_version } from "../wasm/transync_wasm.js";
import { mountSync, fetchOk } from "./sync.js";

/**
 * The alignment-map schema this demo was written against — the JS mirror of
 * `transync_syntax::align::ALIGNMENT_SCHEMA_VERSION`. It is checked twice:
 * against the wasm module at init (build-time drift: a blob built from a
 * different commit) and against the fetched map before mounting (data
 * drift). Both checks fail closed.
 */
const KNOWN_SCHEMA = "1.3.0";

/** Debounce between the last keystroke and the wasm rebuild. */
const DEBOUNCE_MS = 300;

/**
 * Where the wasm module lives, resolved exactly the way the wasm-bindgen
 * glue resolves it for itself — `new URL('transync_wasm_bg.wasm',
 * import.meta.url)` from `web/wasm/transync_wasm.js`, i.e. a sibling of the
 * glue. Spelled out here because `boot()` hands `init()` a `Request` of its
 * own (see `WASM_INIT_TIMEOUT_MS`) instead of letting the glue build one,
 * and `import.meta.url` is *this* module's URL, one directory over. The
 * repo tree and `scripts/test-browser.sh`'s fixture mirror the same shape,
 * so the one specifier resolves in both; drift in the emitted artifact name
 * would surface as a 404 at the fatal panel rather than silently.
 */
const WASM_MODULE_URL = new URL("../wasm/transync_wasm_bg.wasm", import.meta.url);

/**
 * Upper bound on fetching the wasm module (R0004-0090).
 *
 * `init()` left to itself fetches `transync_wasm_bg.wasm` with no
 * `AbortSignal` and no deadline, so a server that accepted the connection
 * and then went quiet pinned the page at `data-demo-state="booting"` with
 * nothing on screen and nothing on the console — the same hang R0002-0052
 * closed for the three artifact fetches, one request wider. The module is
 * an order of magnitude larger than those artifacts (~1.7 MB against a few
 * kB), so it gets its own, more generous bound rather than `sync.js`'s
 * `FETCH_TIMEOUT_MS`.
 */
const WASM_INIT_TIMEOUT_MS = 60000;

/**
 * Live demo state. `payloads` / `statuses` are the edit model handed to
 * `rebuild`; `map` is whichever alignment map the panes are currently
 * mounted against.
 *
 * `payloads` is the LAST ACCEPTED model and nothing else (R0003-0003): it is
 * only ever replaced by a payload set that survived a whole rebuild AND the
 * mount that followed it. Text the user has typed but that has not got that
 * far lives in `drafts`, keyed by block id — see `applyEdit`.
 */
const state = {
  sourceMd: "",
  translatedMd: "",
  map: null,
  payloads: null,
  drafts: null,
  statuses: null,
  editable: new Set(),
  sourceLang: "",
  targetLang: "",
  detected: undefined,
  editingId: null,
  debounceTimer: null,
};

function paneEl(id) {
  return document.getElementById(id);
}

/**
 * The element ids this module dereferences without a null check: the two
 * panes (`mountPanes` reads their `innerHTML` and `scrollTop` before it
 * writes) and the editor (`openEditor` writes its `value`). Missing any of
 * them is template drift in `demo-wasm.html`, and it used to surface as an
 * uncaught TypeError out of `boot()` or out of a debounce timer, leaving the
 * page stuck at `data-demo-state="booting"` — the one failure the demo
 * presents by not presenting anything (R0003-0069).
 *
 * The strips (`error-strip`, `status-strip`, `editor-label`) are absent from
 * this list on purpose: every writer already null-guards them, because a
 * demo that cannot show its message must still take its decision.
 */
const REQUIRED_ELEMENT_IDS = ["source", "target", "editor"];

function missingElementIds() {
  return REQUIRED_ELEMENT_IDS.filter((id) => !document.getElementById(id));
}

/* ------------------------------------------------------------------ *
 * Presentation: fatal panel, non-fatal error strip, status strip.
 * ------------------------------------------------------------------ */

/**
 * Unrecoverable: replace the source pane's content with the message and
 * blank the target, mirroring the shipped shells' fail-closed style.
 */
function showFatal(message) {
  const text = `transync: ${message}`;
  const source = paneEl("source");
  const target = paneEl("target");
  if (source) source.textContent = text;
  if (target) target.textContent = "";
  showError(text);
  document.body.dataset.demoState = "fatal";
  console.error(text);
}

/** Recoverable (e.g. a rejected edit): strip only, panes untouched. */
function showError(message) {
  const strip = document.getElementById("error-strip");
  if (!strip) return;
  strip.textContent = message;
  strip.hidden = false;
}

function clearError() {
  const strip = document.getElementById("error-strip");
  if (!strip) return;
  strip.textContent = "";
  strip.hidden = true;
}

function setStatus(message) {
  const strip = document.getElementById("status-strip");
  if (strip) strip.textContent = message;
}

/* ------------------------------------------------------------------ *
 * Schema gates.
 * ------------------------------------------------------------------ */

function parseSemver(value) {
  // R0002-0045, and the same bound `sync.js`'s gate uses: an unbounded run of
  // digits converts to Infinity, and Infinity in the minor slot compares as
  // "newer than this demo" forever — accepted with a forward-drift warning
  // instead of refused. Nine digits keeps every component a safe integer.
  const match = /^(\d{1,9})\.(\d{1,9})\.(\d{1,9})$/.exec(String(value || ""));
  if (!match) return null;
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

/**
 * Mirror of `sync.js`'s `loadAlignment` version gate, promoted from a
 * `console.warn` + silent no-mount to a visible fatal panel: an unknown
 * major means the rows this demo slices `out.md` with are not the rows the
 * map actually carries.
 *
 * A newer minor/patch with the same major is forward-compat drift — warn
 * and proceed, exactly as the engine does (OI-0024).
 */
function alignmentSchemaVerdict(map) {
  const known = parseSemver(KNOWN_SCHEMA);
  const raw = map && map.schema_version;
  const found = parseSemver(raw);
  if (!found || found.major !== known.major) {
    return {
      ok: false,
      message:
        `alignment map schema_version=${raw === undefined ? "(missing)" : raw} ` +
        `is not major ${known.major} (demo speaks ${KNOWN_SCHEMA}) — refusing to mount`,
    };
  }
  if (found.minor > known.minor || (found.minor === known.minor && found.patch > known.patch)) {
    console.warn(
      `transync: alignment map schema_version=${raw} is newer than this demo ` +
        `(${KNOWN_SCHEMA}); proceeding, but rendering may be incomplete`,
    );
  }
  return { ok: true };
}

/**
 * The map's rows, or none — the single door every row loop in this module
 * goes through (R0003-0068).
 *
 * `render_pair` deserializes `blocks` into a `Vec<AlignmentBlock>` and
 * refuses anything else, but it runs LAST: three gates and the edit model
 * iterate these rows first. `for (const row of map.blocks || [])` over a
 * truthy non-array (`"blocks": 42`) throws "is not iterable" inside `boot()`,
 * which has no try/catch around its gates and no top-level rejection
 * handler — so a malformed map left the page pinned at
 * `data-demo-state="booting"` with an uncaught error instead of the fatal
 * panel it promises. `blocksVerdict` names that shape; this keeps every
 * later loop honest even if the order of the gates ever changes.
 */
function mapRows(map) {
  const rows = map && map.blocks;
  return Array.isArray(rows) ? rows : [];
}

/**
 * `blocks` must be an array if it is present at all. Absent (or null) is
 * left to `render_pair`, whose serde error names the field better than this
 * could; a present non-array is refused here, because `mapRows` would
 * otherwise silently read it as an empty document and boot a pair of panes
 * with an empty edit model.
 */
function blocksVerdict(map) {
  const rows = map && map.blocks;
  if (rows === undefined || rows === null || Array.isArray(rows)) return { ok: true };
  return {
    ok: false,
    message:
      `alignment map "blocks" is ${typeof rows}, not an array of rows ` + `— refusing to mount`,
  };
}

/**
 * The demo's row gate.
 *
 * `render_pair` deserializes the fetched map into `AlignmentMap` before
 * anything mounts, so a missing `blocks`, a row with no `source_block_id`, a
 * mistyped field, or an unknown `sync_role` all fail there, fatally and by
 * name. This demo therefore needs no JS mirror of the `sync.js` row gate
 * (R0001-0043) — `sync.js` needs one because nothing Rust sits between it
 * and the map it is handed.
 *
 * A REPEATED `source_block_id` is the one serde waves through. Since
 * R0002-0010 the Rust renderer refuses it too, so this gate is no longer the
 * only thing standing between a repeated id and a mounted pane — it stays
 * because it runs FIRST, before `buildEditModel` keys `payloads` /
 * `statuses` off the last such row, and because it names the offending id in
 * the demo's own fatal panel instead of a stringified engine error.
 */
function duplicateRowVerdict(map) {
  const seen = new Set();
  for (const row of mapRows(map)) {
    const id = row && row.source_block_id;
    // Anything else about the row is `render_pair`'s verdict to give.
    if (typeof id !== "string" || id === "") continue;
    if (seen.has(id)) {
      return {
        ok: false,
        message: `alignment map repeats source_block_id "${id}" — refusing to mount`,
      };
    }
    seen.add(id);
  }
  return { ok: true };
}

/* ------------------------------------------------------------------ *
 * Edit model.
 * ------------------------------------------------------------------ */

const UTF8_ENCODER = new TextEncoder();
const UTF8_DECODER = new TextDecoder();

/**
 * Slice by BYTE offsets. `source_range` / `target_range` are byte ranges
 * into the UTF-8 Markdown; JS string indices are UTF-16 code units, so
 * `String.prototype.slice` would mis-cut the moment the document contains
 * a non-ASCII character (the SCN-14 fixture's em dashes already do).
 *
 * The range must already have passed `targetRangeVerdict`; this function
 * neither clamps nor reorders. It used to (R0002-0054), and the coercion
 * was worse than it looked: a reversed range became an EMPTY payload, an
 * out-of-range one a truncated payload, and since every `rebuild` resubmits
 * `state.payloads` in full, one such row rewrote its block with that wrong
 * content the first time the user edited any *other* block.
 */
function sliceBytes(bytes, range) {
  return UTF8_DECODER.decode(bytes.subarray(range.start, range.end));
}

/**
 * Which rows the demo will let you edit (spec §4.4: "non-html, non-skipped,
 * sync-relevant"). One home, because `targetRangeVerdict` and
 * `buildEditModel` must agree on it exactly: gating a row the model then
 * edits would leave the coercion in place for precisely the rows that
 * matter, and gating one it omits would refuse to boot over a range nothing
 * reads.
 */
function isEditableRow(row) {
  const id = row && row.source_block_id;
  if (typeof id !== "string" || id === "") return false;
  return row.block_kind !== "html" && row.block_kind !== "skipped" && row.sync_role !== "non-sync";
}

/** Is `index` the start of a UTF-8 sequence (or the end of the buffer)? */
function isCharBoundary(bytes, index) {
  if (index === 0 || index === bytes.length) return true;
  // Continuation bytes are 0b10xxxxxx; anything else starts a character.
  return (bytes[index] & 0xc0) !== 0x80;
}

/**
 * The demo's third boot gate: every editable row's `target_range` must be a
 * real slice of `out.md`.
 *
 * `render_pair` deserializes the same map into Rust `ByteRange { start:
 * usize, end: usize }` with no serde defaults, so a missing, negative,
 * fractional or mistyped offset already fails fatally there. *Value* sanity
 * — reversed (`end < start`), past the end of the document, or landing
 * mid-character — used to be nobody's job: the Rust renderer clamped and
 * snapped to char boundaries, which kept the pane panic-free and told the
 * edit model nothing.
 *
 * Since R0003-0060 the renderer refuses those instead, for **every** row it
 * slices — which is what closes the hole this gate could not see
 * (R0003-0078): html and skipped rows are not editable, so they are not
 * checked here, and their ranges are exactly the ones the renderer's bypass
 * arms read. That refusal lands at the `render_pair` call below and is
 * fatal, so no coerced byte reaches a pane through either path.
 *
 * This gate stays, scoped to editable rows, because it answers a different
 * question at a different time: the edit model is built BEFORE the render
 * (`buildEditModel` slices these same ranges), and its blast radius is the
 * whole document rather than one row — `applyEdit` submits the entire
 * `state.payloads` map on every rebuild, so a single mis-sliced pre-edit
 * payload is written into its block the first time the user edits any block
 * at all (R0002-0054). Refusing here names the row and the edit model;
 * the renderer's later refusal names the pane and the byte offsets.
 */
function targetRangeVerdict(bytes, map) {
  for (const row of mapRows(map)) {
    if (!isEditableRow(row)) continue;
    const id = row.source_block_id;
    const range = row.target_range;
    const start = range && range.start;
    const end = range && range.end;
    const refuse = (why) => ({
      ok: false,
      message:
        `alignment row "${id}" has an unusable target_range ` +
        `(${JSON.stringify(range)}): ${why} — refusing to mount`,
    });
    if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end)) {
      return refuse("offsets must be integers");
    }
    if (start < 0 || end < start) {
      return refuse("start must be non-negative and no greater than end");
    }
    if (end > bytes.length) {
      return refuse(`end is past the ${bytes.length}-byte out.md`);
    }
    if (!isCharBoundary(bytes, start) || !isCharBoundary(bytes, end)) {
      return refuse("offsets must fall on UTF-8 character boundaries");
    }
  }
  return { ok: true };
}

/**
 * Build the edit model from the *pre-edit* triple. `bytes` is `out.md`
 * UTF-8-encoded, already checked by `targetRangeVerdict`.
 *
 * `target_range` indexes those bytes and is only valid before the first edit
 * — which is exactly when this runs. Afterwards `payloads` is the authority
 * and is never re-derived from a regenerated document.
 *
 * The model covers exactly the rows `isEditableRow` admits (spec §4.4:
 * "non-html, non-skipped, sync-relevant"). Every other row is omitted from
 * BOTH maps, which is the honest choice rather than merely the tidy one:
 *
 * - **html rows.** `regen` expects an html block's payload to be its
 *   extracted text-segment array (ADR-0018), not Markdown, and that array is
 *   unrecoverable from the rendered bundle — the demo only ever sees the
 *   spliced result. A Markdown slice therefore fails `regen`'s parse and the
 *   block falls back to its *source* bytes: after any edit the html block
 *   shows source content. Carrying the original `translated` status across
 *   that would make the row (and the tint legend) claim a translation the
 *   pane no longer shows. Omitted instead, `build_alignment_map` synthesizes
 *   `fallback_source` for unit-backed html rows, so the pane presents the
 *   escaped `data-skipped="html-block"` placeholder under the fallback tint —
 *   pinned host-side by `transync-wasm`'s
 *   `omitted_unit_backed_html_row_is_fallback_source_not_translated`. html
 *   blocks stay out of `editable`; the demo never offers an edit it cannot
 *   round-trip.
 * - **skipped and non-sync rows** (thematic breaks). Never translation units,
 *   so their target bytes already *are* their source bytes and omission
 *   changes nothing: `regen` re-emits the same bytes and
 *   `build_alignment_map` synthesizes the same `preserved` the map carried.
 *
 * Editable rows keep their status verbatim: it records *LLM* provenance, and
 * a human demo edit is not an LLM decision.
 */
function buildEditModel(bytes, map) {
  // R0004-0088: keyed by ids that come off the FETCHED map, so they are data.
  // A plain `{}` inherits `Object.prototype`, on which `__proto__` is an
  // accessor: `payloads["__proto__"] = "…"` is a silent no-op, the row's
  // payload and status vanish from the model, and `state.drafts[id] ??
  // state.payloads[id]` then reads `Object.prototype` itself — a truthy
  // object the editor renders as "[object Object]". Every OTHER alien id
  // lands in the model and is named by the engine's `reject_unknown_ids`, so
  // the plain object made exactly one id behave differently from the rest.
  // Null-prototype maps have no inherited accessor to hit and no inherited
  // key to read, which is the same fail-closed line `duplicateRowVerdict`'s
  // `Set` and the rest of the demo's map gates hold. `state.drafts` and
  // `applyEdit`'s staged clone are built the same way — a null prototype the
  // first assignment restores to `Object.prototype` would guard nothing.
  const payloads = Object.create(null);
  const statuses = Object.create(null);
  const editable = new Set();
  for (const row of mapRows(map)) {
    if (!isEditableRow(row)) continue;
    const id = row.source_block_id;
    editable.add(id);
    payloads[id] = sliceBytes(bytes, row.target_range);
    // The wire (snake_case) form, verbatim — `FallbackStatus` deserializes
    // exactly these strings and rejects anything else.
    statuses[id] = row.fallback_status;
  }
  return { payloads, statuses, editable };
}

/* ------------------------------------------------------------------ *
 * Pane metadata.
 * ------------------------------------------------------------------ */

/**
 * The one reserved source-language label (ADR-0013). `source_language`
 * carries the run's REQUESTED label verbatim, so a run that asked the model
 * to detect the language leaves the literal `auto` in the map rather than a
 * tag. Compared case-insensitively, mirroring the CLI's single recognizer
 * (`translate_cmd::args::is_source_language_sentinel`).
 */
const SOURCE_LANGUAGE_SENTINEL = "auto";

/**
 * A map language label, trimmed — and empty for anything that is not a
 * string. `render_pair` refuses a non-string label by name, but it runs
 * AFTER this, and a `TypeError` thrown out of `boot()` is the one failure
 * this demo presents by not presenting anything (R0003-0069).
 */
function languageLabel(value) {
  return typeof value === "string" ? value.trim() : "";
}

/**
 * Stamp `lang` on the two panes from the map's labels (R0011-0038). Without
 * it both panes inherit the page's `lang="en"`, and a screen reader, a
 * spellchecker, and the font/hyphenation machinery all read the translated
 * pane as English.
 *
 * The resolution is the CLI bundle's rather than a second one: the source
 * pane is `publish::pane_source_language` — the sentinel hands the pane over
 * to the model's detection, and an absent detection leaves it empty — and an
 * empty label stamps no attribute at all, which is `bundle.rs::lang_attr`.
 * Labels stay opaque (ADR-0013): they are set as attribute VALUES, never
 * parsed, canonicalized, or matched against a table.
 *
 * Once, at boot, is enough. `mountPanes` replaces each pane's CONTENT
 * through `innerHTML`, which leaves the pane element and its attributes
 * standing, and `rebuild` cannot change a run's language labels.
 */
function applyPaneLanguages() {
  const requested = languageLabel(state.sourceLang);
  const wantsDetection = requested.toLowerCase() === SOURCE_LANGUAGE_SENTINEL;
  const source = wantsDetection ? languageLabel(state.detected) : requested;
  const target = languageLabel(state.targetLang);
  if (source) paneEl("source").lang = source;
  if (target) paneEl("target").lang = target;
}

/* ------------------------------------------------------------------ *
 * Mounting.
 * ------------------------------------------------------------------ */

/**
 * Sanitize, mount, sync. `mountSync` tears down any controller it
 * previously installed on either pane, so calling this again after a
 * re-render is the documented idempotent re-mount (R0008-0048) — no
 * explicit `destroy()` needed.
 *
 * `carry` (re-render only) is `{ details, scroll }` captured from the
 * outgoing DOM. Both are re-applied *between* the `innerHTML` writes and
 * `mountSync`, deliberately: an `innerHTML` swap resets `scrollTop` to 0,
 * and assigning it back once the engine's scroll listeners are live reads
 * as a user scroll, so each pane would drive the other away from the
 * position being restored. `<details>` goes first within that window —
 * re-opening a collapsed block changes the pane's `scrollHeight`, and a
 * `scrollTop` assigned against the shorter layout would be clamped before
 * the height came back.
 *
 * **A refused map is a failed mount (R0002-0016, R0002-0049).** `mountSync`
 * returns `null` for a map it cannot drive, and this used to be ignored:
 * boot then marked the demo `ready` and the edit loop kept running over
 * panes that rendered but never scrolled together — with the previous render
 * already destroyed by the `innerHTML` writes, which run before the engine
 * gets to judge the map. Both halves are answered here: the outgoing markup
 * is kept until the mount is accepted, and a refusal restores it, re-mounts
 * the last good map over it, and reports `false`.
 */
function mountPanes(srcHtml, tgtHtml, map, carry) {
  const source = paneEl("source");
  const target = paneEl("target");
  // OI-0001: re-checked on every mount, not just at boot — a mount without
  // DOMPurify must never happen, however we got here.
  if (!window.DOMPurify) {
    showFatal("DOMPurify missing — refusing to mount unsanitized HTML");
    return false;
  }
  // Everything the `innerHTML` writes below are about to destroy. Captured
  // even at boot, where it is empty and the restore is a no-op.
  const previous = {
    sourceHtml: source.innerHTML,
    targetHtml: target.innerHTML,
    sourceScroll: source.scrollTop,
    targetScroll: target.scrollTop,
    map: state.map,
  };
  // R0003-0070: both panes are sanitized BEFORE either is written. The
  // presence check above answers for a DOMPurify that is missing; this
  // answers for one that is present and throws (a broken build, a hooked or
  // misconfigured instance). Sanitizing inline meant the second call could
  // throw with the first pane's markup already replaced — a half-swapped
  // pair, the source showing the new render and the target the old one, and
  // the exception escaping into `boot()` or a debounce timer. Staging first
  // makes the failure atomic: nothing was written, so the last good render
  // is still on screen and needs no restore.
  let cleanSource;
  let cleanTarget;
  try {
    cleanSource = window.DOMPurify.sanitize(srcHtml);
    cleanTarget = window.DOMPurify.sanitize(tgtHtml);
  } catch (err) {
    const why = `sanitizing the rendered HTML failed: ${err && err.message ? err.message : err}`;
    // Same presentation split as the refused-map branch below: with a render
    // to keep, this is a strip; at boot there is nothing to keep.
    if (previous.map) showError(`transync: ${why}, keeping the last good render`);
    else showFatal(why);
    return false;
  }
  source.innerHTML = cleanSource;
  target.innerHTML = cleanTarget;
  if (carry) {
    restoreDetails(source, carry.details.source);
    restoreDetails(target, carry.details.target);
    // Read scrollHeight first: it forces the pending layout, so the
    // assignment below is clamped against the real content height rather
    // than the not-yet-laid-out pane's zero.
    void source.scrollHeight;
    void target.scrollHeight;
    source.scrollTop = carry.scroll.source;
    target.scrollTop = carry.scroll.target;
  }
  state.map = map;
  if (!mountSync(source, target, map)) {
    restorePanes(source, target, previous);
    const why = "sync engine refused the alignment map — see the console";
    // With a render to fall back to this is the non-fatal presentation: the
    // panes still show the last good pair and the session continues. At boot
    // there is no such render, so the refusal is the end of the demo.
    if (previous.map) showError(`transync: ${why}, keeping the last good render`);
    else showFatal(why);
    return false;
  }
  markEditable();
  markEditing();
  return true;
}

/**
 * Put back what `mountPanes` displaced when the engine refused the new map.
 *
 * The restored markup is *new* DOM — `innerHTML` re-parses — and `mountSync`
 * destroyed the controller that was driving the outgoing nodes before it
 * judged the incoming map (R0002-0015). So the last good map has to be
 * mounted again over the restored panes; without that the demo would show
 * the right picture with no engine behind it. That re-mount cannot cascade:
 * the map it re-uses is the one that was accepted for these exact panes.
 */
function restorePanes(source, target, previous) {
  state.map = previous.map;
  source.innerHTML = previous.sourceHtml;
  target.innerHTML = previous.targetHtml;
  void source.scrollHeight;
  void target.scrollHeight;
  source.scrollTop = previous.sourceScroll;
  target.scrollTop = previous.targetScroll;
  if (previous.map) mountSync(source, target, previous.map);
}

/**
 * Give each EDITABLE block in the target pane a tab stop (R0011-0042), so the
 * demo's one human-edit workflow can be reached without a pointer. Re-applied
 * after every mount for the reason `markEditing` is: `innerHTML` builds new
 * nodes, and the renderer emits none of this — the anchor markup is shared
 * with the CLI bundle, where there is no editor to open.
 *
 * Editable rows only: a tab stop on a block the click path answers with "is
 * not editable" is a stop that leads nowhere. No `role`/`aria-label` rides
 * along — naming a block of translated content would replace the content it
 * names for a screen reader; the boot line on the `role="status"` strip is
 * where the keyboard path is announced.
 */
function markEditable() {
  const target = paneEl("target");
  if (!target) return;
  for (const anchor of target.querySelectorAll("[data-sync-id]")) {
    if (state.editable.has(anchor.dataset.syncId)) anchor.tabIndex = 0;
  }
}

/** Re-apply the selection outline; the class dies with each innerHTML swap. */
function markEditing() {
  for (const pane of [paneEl("source"), paneEl("target")]) {
    if (!pane) continue;
    for (const el of pane.querySelectorAll(".is-editing")) {
      el.classList.remove("is-editing");
    }
    if (!state.editingId) continue;
    const el = pane.querySelector(`[data-sync-id="${CSS.escape(state.editingId)}"]`);
    if (el) el.classList.add("is-editing");
  }
}

/**
 * Capture `<details open>` per anchor. The state lives in the DOM and dies
 * with `innerHTML`, and `<details>` elements carry no `data-sync-id` of
 * their own (the renderer attributes the *wrapper*), so the key is
 * `syncId` + the details' index inside that anchor — the same pairing
 * `sync.js`'s toggle mirror uses.
 */
function captureDetails(pane) {
  const snapshot = new Map();
  if (!pane) return snapshot;
  for (const anchor of pane.querySelectorAll("[data-sync-id]")) {
    const list = anchor.querySelectorAll("details");
    for (let i = 0; i < list.length; i += 1) {
      snapshot.set(`${anchor.dataset.syncId}#${i}`, list[i].open);
    }
  }
  return snapshot;
}

function restoreDetails(pane, snapshot) {
  if (!pane || !snapshot || snapshot.size === 0) return;
  for (const anchor of pane.querySelectorAll("[data-sync-id]")) {
    const list = anchor.querySelectorAll("details");
    for (let i = 0; i < list.length; i += 1) {
      const key = `${anchor.dataset.syncId}#${i}`;
      if (snapshot.has(key)) list[i].open = snapshot.get(key);
    }
  }
}

/* ------------------------------------------------------------------ *
 * Editing.
 * ------------------------------------------------------------------ */

/**
 * Descendants that own their own activation (R0011-0041). A translated block
 * can contain a link, a form control, or the `<summary>` of a `<details>`
 * whose open state `sync.js` mirrors across the panes — and a click on one of
 * those is that control's click, not a request to edit the block around it.
 * The pane handler resolves the nearest anchor for EVERY descendant click, so
 * without this both happened at once: the control acted and the editor
 * opened over it.
 */
const INTERACTIVE_SELECTOR = "a, button, summary, input, select, textarea, label";

function wireEditing() {
  const target = paneEl("target");
  const editor = document.getElementById("editor");
  if (!target || !editor) return;

  target.addEventListener("click", (event) => {
    const node = event.target;
    // Scoped with `contains` for the reason the anchor lookup below is:
    // `closest` walks past the pane, so an ancestor of the whole demo could
    // otherwise answer for a click inside it.
    const control = node && node.closest ? node.closest(INTERACTIVE_SELECTOR) : null;
    if (control && target.contains(control)) return;
    const anchor = node && node.closest ? node.closest("[data-sync-id]") : null;
    if (!anchor || !target.contains(anchor)) return;
    const id = anchor.dataset.syncId;
    if (!state.editable.has(id)) {
      setStatus(`${id} (${anchor.dataset.blockKind || "?"}) is not editable`);
      return;
    }
    openEditor(id);
  });

  // The same selection without a pointer (R0011-0042): `markEditable` gives
  // each editable block a tab stop, and Enter/Space open the block that HAS
  // focus. Keyed off `event.target` — the focused element itself, never its
  // nearest anchor — so a focusable control inside a block keeps its own
  // Enter/Space, which is the line the click guard above draws.
  target.addEventListener("keydown", (event) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    const node = event.target;
    const id = node && node.dataset ? node.dataset.syncId : undefined;
    if (id === undefined || !state.editable.has(id)) return;
    // Space scrolls the pane by default, which would carry the block that is
    // about to open out from under the reader.
    event.preventDefault();
    openEditor(id);
  });

  editor.addEventListener("input", () => {
    if (!state.editingId) return;
    window.clearTimeout(state.debounceTimer);
    state.debounceTimer = window.setTimeout(applyEdit, DEBOUNCE_MS);
  });
}

/**
 * Run a debounce that has not fired yet, right now.
 *
 * The timer is bound to whatever `state.editingId` / `editor.value` hold
 * when it fires, not to what they held when it was armed — so anything that
 * is about to change either one must flush first or silently retarget the
 * pending edit.
 */
function flushPendingEdit() {
  if (state.debounceTimer === null) return;
  window.clearTimeout(state.debounceTimer);
  applyEdit();
}

function openEditor(id) {
  // Switching blocks within the debounce window would otherwise drop the
  // outgoing block's last keystrokes: the timer survives the swap below and
  // then writes the INCOMING block's freshly loaded value into the outgoing
  // block's payload. Land the pending edit before the swap instead.
  flushPendingEdit();
  const editor = document.getElementById("editor");
  state.editingId = id;
  // A draft outranks the accepted payload: it is what the user typed and it
  // is the only copy left (R0003-0003). Re-priming from `state.payloads`
  // alone would silently throw away the text of a rejected edit the moment
  // the user clicked away and back — the model would be honest and the
  // user's work would be gone.
  editor.value = state.drafts[id] ?? state.payloads[id] ?? "";
  editor.disabled = false;
  const label = document.getElementById("editor-label");
  if (label) label.textContent = `editing ${id}`;
  setStatus(`editing ${id} — changes re-render after ${DEBOUNCE_MS} ms`);
  markEditing();
  editor.focus();
}

/**
 * The debounced edit loop: payload in, whole document back out.
 *
 * `rebuild` regenerates the translated Markdown AND a matching alignment
 * map; the stale map must never be reused, because the html/skipped render
 * arms slice raw `target_range` bytes out of the regenerated document.
 * `alignment_json` is nested JSON, hence the second parse.
 *
 * It also reports `structure_warning`, which is announced on the non-fatal
 * strip below — see there for why it never aborts the mount.
 *
 * **A rejected edit does not enter the model (R0003-0003).** The edit used
 * to be written straight into `state.payloads` before the attempt, and
 * neither failure path took it back out: since every rebuild resubmits the
 * whole payload map, one rejected payload then re-failed the rebuild of
 * every LATER edit, of any block, until the user happened to fix the
 * original block. The panes were right and the model was poisoned. So the
 * submission is a staged clone and `state.payloads` only advances past a
 * mount that succeeded, while the typed text is kept in `state.drafts` —
 * restoring the old payload alone would have healed the model by discarding
 * what the user wrote.
 */
function applyEdit() {
  // Cleared first, whichever way we got here (timer fired, or
  // `flushPendingEdit` beat it to it), so the flag means exactly "a rebuild
  // is still owed" and a later flush cannot re-run a landed edit.
  state.debounceTimer = null;
  const editor = document.getElementById("editor");
  if (!state.editingId) return;
  const editingId = state.editingId;
  // Kept whatever the outcome: this is the user's text, and after a
  // rejection it is the only copy of it that survives.
  state.drafts[editingId] = editor.value;
  // Submitted, not committed. Null-prototype like the model it clones
  // (R0004-0088): `state.payloads = staged` below makes this object the
  // model, so a `{ ... }` spread here would quietly restore
  // `Object.prototype` after the first edit and undo `buildEditModel`'s
  // guard for every edit that followed.
  const staged = Object.assign(Object.create(null), state.payloads, {
    [editingId]: editor.value,
  });

  let out;
  let freshMap;
  try {
    out = JSON.parse(
      rebuild(
        state.sourceMd,
        JSON.stringify(staged),
        JSON.stringify(state.statuses),
        state.sourceLang,
        state.targetLang,
        state.detected,
      ),
    );
    // R0002-0017: inside the boundary, not after it. `alignment_json` is
    // nested JSON from the same call, so a blob that returned a malformed
    // one would have thrown out of a timer callback — an uncaught exception
    // past the visible recovery path this catch exists to be.
    freshMap = JSON.parse(out.alignment_json);
  } catch (err) {
    // Non-fatal by construction: nothing was mounted, so the previous
    // panes are still the last good render.
    showError(`transync: rebuild failed: ${err && err.message ? err.message : err}`);
    return;
  }

  const source = paneEl("source");
  const target = paneEl("target");
  // Captured before the swap: both live only in the DOM and die with it.
  const carry = {
    details: { source: captureDetails(source), target: captureDetails(target) },
    scroll: { source: source.scrollTop, target: target.scrollTop },
  };

  if (!mountPanes(out.source_html, out.target_html, freshMap, carry)) return;

  // Accepted — and only here. The staged clone is the model now, and the
  // draft has become indistinguishable from it, so it stops being a draft.
  state.payloads = staged;
  delete state.drafts[editingId];
  state.translatedMd = out.translated_md;
  // `structure_warning` is a warning, not a rejection, so the panes are
  // already mounted above and the block stays editable — undoing the change
  // is the whole recovery. `rebuild` sets it when the regenerated document
  // no longer carries the source's top-level block count, which means the
  // render just mounted is the DEGRADED one: the reshaped block shows its
  // raw Markdown and blocks after it can show shifted content under correct
  // anchors. Silence there would let that pass for a clean re-render, so it
  // takes the same non-fatal strip a rejected rebuild uses. The strip is
  // cleared on every clean rebuild, so a fixed payload clears the notice.
  if (out.structure_warning) {
    showError(`transync: ${out.structure_warning}`);
  } else {
    clearError();
  }
  setStatus(`re-rendered ${freshMap.blocks.length} blocks after editing ${state.editingId}`);
}

/* ------------------------------------------------------------------ *
 * Boot.
 * ------------------------------------------------------------------ */

async function boot() {
  document.body.dataset.demoState = "booting";
  // R0003-0069, first of all the gates: the page this module drives is
  // checked into the repo, so a missing pane or editor is drift rather than
  // data — and it costs nothing to say so before fetching a ~1.7 MB wasm
  // module that has nowhere to render.
  const missing = missingElementIds();
  if (missing.length > 0) {
    showFatal(
      `the page is missing required element${missing.length > 1 ? "s" : ""} ` +
        `#${missing.join(", #")} — refusing to mount`,
    );
    return;
  }
  // R0004-0090: bounded, and cancelled when the bound expires. `init()` with
  // no argument builds its own unabortable `fetch`, so the module request is
  // handed in as a `Request` carrying our signal instead — the glue passes it
  // straight to `fetch`, which keeps `WebAssembly.instantiateStreaming` on the
  // fast path (and keeps the wrong-MIME fallback warning the browser suite's
  // clean-boot assertion watches for). Reported through the same fatal panel
  // every other boot refusal uses; `timedOut` distinguishes our own abort from
  // a network error, exactly as `fetchOk` does.
  const wasmInit = new AbortController();
  let wasmTimedOut = false;
  const wasmTimer = window.setTimeout(() => {
    wasmTimedOut = true;
    wasmInit.abort();
  }, WASM_INIT_TIMEOUT_MS);
  try {
    await init({
      module_or_path: new Request(WASM_MODULE_URL, { signal: wasmInit.signal }),
    });
  } catch (err) {
    if (wasmTimedOut) {
      showFatal(
        `wasm init failed: timed out loading ${WASM_MODULE_URL.pathname} ` +
          `after ${WASM_INIT_TIMEOUT_MS} ms`,
      );
    } else {
      showFatal(`wasm init failed: ${err && err.message ? err.message : err}`);
    }
    return;
  } finally {
    window.clearTimeout(wasmTimer);
  }

  const wasmSchema = schema_version();
  if (wasmSchema !== KNOWN_SCHEMA) {
    showFatal(`schema mismatch: wasm ${wasmSchema} vs demo ${KNOWN_SCHEMA}`);
    return;
  }
  if (!window.DOMPurify) {
    showFatal("DOMPurify missing — refusing to mount (fail-closed)");
    return;
  }

  let sourceMd;
  let outMd;
  let alignmentJson;
  // R0002-0053: one controller for all three artifacts. `Promise.all` rejects
  // on the first failure but leaves its siblings running, and this boot is
  // already lost by then — aborting cancels the requests still in flight
  // rather than paying for downloads nothing will read. Each fetch is also
  // independently time-bounded inside `fetchOk` (R0002-0052), so a server
  // that accepts and stalls can no longer pin the demo in "booting".
  const artifacts = new AbortController();
  try {
    [sourceMd, outMd, alignmentJson] = await Promise.all([
      fetchOk("source.md", (r) => r.text(), { signal: artifacts.signal }),
      fetchOk("out.md", (r) => r.text(), { signal: artifacts.signal }),
      fetchOk("alignment.json", (r) => r.text(), { signal: artifacts.signal }),
    ]);
  } catch (err) {
    artifacts.abort();
    showFatal(err && err.message ? err.message : String(err));
    return;
  }

  let map;
  try {
    map = JSON.parse(alignmentJson);
  } catch (err) {
    showFatal(`alignment map is not JSON: ${err && err.message ? err.message : err}`);
    return;
  }

  const verdict = alignmentSchemaVerdict(map);
  if (!verdict.ok) {
    showFatal(verdict.message);
    return;
  }

  const shape = blocksVerdict(map);
  if (!shape.ok) {
    showFatal(shape.message);
    return;
  }

  const rows = duplicateRowVerdict(map);
  if (!rows.ok) {
    showFatal(rows.message);
    return;
  }

  const outBytes = UTF8_ENCODER.encode(outMd);
  const ranges = targetRangeVerdict(outBytes, map);
  if (!ranges.ok) {
    showFatal(ranges.message);
    return;
  }

  const model = buildEditModel(outBytes, map);
  state.sourceMd = sourceMd;
  state.translatedMd = outMd;
  state.payloads = model.payloads;
  // Nothing is typed yet, so nothing is unaccepted yet. Null-prototype for
  // the reason `buildEditModel` gives (R0004-0088): this map is READ with
  // `state.drafts[id] ?? …`, so an inherited key would answer for a block
  // that has no draft at all.
  state.drafts = Object.create(null);
  state.statuses = model.statuses;
  state.editable = model.editable;
  state.sourceLang = map.source_language || "";
  state.targetLang = map.target_language || "";
  // wasm-bindgen's `Option<String>`: null and undefined both arrive as None.
  state.detected = map.detected_source_language ?? undefined;
  applyPaneLanguages();

  let pair;
  try {
    pair = JSON.parse(render_pair(sourceMd, outMd, alignmentJson));
  } catch (err) {
    showFatal(`render failed: ${err && err.message ? err.message : err}`);
    return;
  }

  if (!mountPanes(pair.source_html, pair.target_html, map)) return;
  wireEditing();
  setStatus(
    `${map.blocks.length} blocks rendered locally — ` +
      `${state.editable.size} editable; click one in the right pane, ` +
      `or tab to it and press Enter`,
  );
  // Readiness signal for the headless suite: the panes are mounted and the
  // click handler is live.
  document.body.dataset.demoState = "ready";
}

boot();
