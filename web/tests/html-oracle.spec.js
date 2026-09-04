// The browser oracle for `transync-html` — Chromium as an INDEPENDENT
// answer about HTML tree construction (ti `ec235f`).
//
// # Why this file exists at all
//
// `crates/transync-html/tests/generative_properties.rs` states the right
// properties over generated fragments — P1 idempotence through P6 anchor
// injection — and the four routes found by the adversarial verification of
// ti `fdd989` are routes those properties are about. They were still missed,
// and generating more input would not have found two of them, because every
// oracle in that file is built out of `scan_tags` / `walk_elements` /
// `balance_fragment`: the functions under test. Where this crate's stacks and
// a browser's tree construction disagree, BOTH sides of every comparison over
// there carry the same disagreement, so the disagreement is invisible by
// construction. That is the finding ti `ec235f` was filed for, and no amount
// of generated input fixes it.
//
// So the oracle is a real browser. The corpus arrives as a file — this crate's
// answers, written by the double-gated `emit_browser_oracle_corpus` in that
// same Rust file and published into the served fixture directory by
// `scripts/test-browser.sh` — and every Chromium answer below is computed
// HERE, from `innerHTML`, and never read out of the corpus.
//
// # Which side of each comparison is which
//
// Stated explicitly, because "independent oracle" is the entire claim.
//
//   (a)  What is still open at the end of the input?
//        CHROMIUM: `openAtEnd(c.input)`      CRATE: `c.crate_open_at_input`
//   (a') And at the end of the balanced output, where P2 says "nothing"?
//        CHROMIUM: `openAtEnd(c.balanced)`   CRATE: `c.crate_open_at_balanced`
//   (b)  §4a — is the FOLLOWING block's anchor still a direct child of the
//        pane's `<main>`, or did this block's fragment eat its own `</div>`?
//        CHROMIUM: `nextIsDirectChild(c.pane)`
//        CRATE:    `c.crate_next_is_direct_child`
//   (c)  Does the pane chain hand any element a reserved sync attribute?
//        ("ours are the only sync attributes in this DOM" is a construction,
//        not a scan — OI-0035 / DCR-0033 / invariant 7.)
//        CHROMIUM: `carriesReserved(c.chained)`
//        CRATE:    `c.crate_chained_carries_reserved`
//
// Nothing on the Chromium side is computed from a corpus field other than the
// raw bytes (`input`, `balanced`, `pane`, `chained`), and nothing on the crate
// side is computed here. If a future edit lets one column be derived from the
// other, this file stops being evidence and becomes decoration.
//
// (c) is asked because two of the four routes are not stack divergences at
// all. The crate's own P6 already reports ti `9b4d66`'s input as a violation
// — its gap was generator coverage, not oracle self-reference — and what a
// browser adds there is the other half of the sentence: that the reported
// violation really is a LIVE anchor in a real DOM rather than an over-strict
// property. An oracle that only ever confirms is as useful as one that only
// ever contradicts; this file records both, per input.
//
// # How Chromium is asked "what is still open?"
//
// A finished DOM has nothing "open" — the question is about the parser's stack
// at the moment the bytes run out. So a SENTINEL element is appended and the
// DOM is asked where it landed: its ancestor chain up to the mount container
// IS the stack of open elements at the end of the fragment, because HTML
// inserts at the current insertion point. It is also the operationally
// meaningful reading of the question — "what would enclose the next byte of
// content?" — which is exactly what `balance_fragment` uses its own answer
// for, so the two columns really are answering the same thing.
//
// Two honest properties of the sentinel, neither of which the crate gets a
// vote on:
//
//   1. **Chromium may abstain.** A fragment that ends inside an unterminated
//      comment, CDATA section, raw-text run or cut-off tag swallows the
//      sentinel, so no such element exists in the DOM and there is no
//      insertion point to report. That is a browser-declared abstention, not a
//      skip this file chose and not a question the crate answered — the
//      abstentions are counted, printed, and bounded below so a change that
//      made everything abstain cannot pass as agreement.
//   2. **The sentinel is a start tag, so HTML reconstructs the active
//      formatting elements before inserting it.** A reconstructed clone
//      therefore appears in the chain. That is not noise: it is precisely what
//      would happen to the next real element, and it is the mechanism behind
//      tickets `307283` / `895fb7`.
//
// # Which parse
//
// `innerHTML` on a detached `<div>` — the **fragment parsing algorithm with a
// `div` context**, which is exactly how the shipped shells mount a pane
// (`web/index.html` and `crates/transync-cli/web/index.html.tpl` both do
// `pane.innerHTML = DOMPurify.sanitize(...)`). DOMPurify is deliberately NOT
// in the loop: `balance_fragment` runs before any sanitizer, and §4a is a
// claim about the balancer's output, so putting a sanitizer between them would
// let it launder the very structure under test.
//
// DELIBERATELY NOT COVERED, recorded rather than dropped quietly: the
// `DOMParser`/document-parse path a reader takes by navigating straight to a
// bundle's `source.html`. It is a different algorithm (insertion modes
// "before html" … "after body", a real `<head>`), so it would roughly double
// the divergence census below while the contract this file is about — a
// block's balanced fragment inside its pane wrapper — is mounted by
// `innerHTML`. A second parse mode is a follow-up, not a silent gap.
//
// # There is no CI here
//
// `crates/transync/tests/docs_gate_claims_drift.rs::the_repository_still_has_
// no_ci` is green. This spec runs on demand through `scripts/test-browser.sh`
// like the rest of the browser suite, and the Rust emitter that feeds it is
// `#[ignore]`d and env-var interlocked, so `cargo test --workspace` neither
// needs a browser nor writes a file.
//
// TRACE: ti ec235f
// TRACE: contracts.md §4a
// TRACE: ADR-0018 (architectural invariant 7)

import fs from "node:fs";
import path from "node:path";

import { expect, test } from "@playwright/test";

import { FIXTURE_DIR } from "./support/harness.js";

// Published by scripts/test-browser.sh's browser-oracle leg, inside the served
// fixture directory like the wasm, OI-0035 and SCN-16 legs before it.
const CORPUS_PATH = path.join(FIXTURE_DIR, "html-oracle", "corpus.json");

// The element appended to read Chromium's insertion point off. No `ATOMS`
// entry and no `PHRASES` entry can produce this name, so finding it is
// unambiguous even for inputs whose whole point is to forge markup.
const SENTINEL = "transync-oracle-here";

// The reserved sync-attribute namespace, verbatim from `contracts.md` §4 and
// `strip_reserved_sync_attrs`. Restated here on purpose: a browser oracle that
// imported the crate's idea of "reserved" would be self-referential in exactly
// the way this file exists to avoid.
const RESERVED_ATTRS = [
  "data-sync-id",
  "data-block-kind",
  "data-order",
  "data-fallback",
  "data-parent-id",
  "data-skipped",
];

// 10_004 cases at four parses each is one `page.evaluate` payload of several
// megabytes if sent whole. Chunked so the bridge carries megabyte-scale
// strings rather than one multi-megabyte one; the chunk size is a transport
// detail and changes no answer.
const CHUNK = 1_000;

// 40_000 in-page parses plus the bridge. Generous, because a timeout here
// would be reported as a browser problem rather than as the volume it is.
const SUITE_TIMEOUT_MS = 300_000;

// ---------------------------------------------------------------------------
// The pins: what is wrong TODAY
// ---------------------------------------------------------------------------

// The four routes the ti `fdd989` verification found, each with the FULL
// observed answer recorded — both columns, all four questions. They are not
// this file's to fix (tickets `9b4d66`, `307283` and `895fb7` own them), and
// they are why this file can be trusted at all: an oracle that has never
// contradicted the code under test has not been shown able to.
//
// `crate*` fields come from the corpus the Rust emitter wrote; `dom*` fields
// are what Chromium said when the pin was blessed. Both are asserted, so an
// entry goes red when EITHER column moves — including when the pair becomes
// healthy, which is the good news this list exists to make visible. When that
// happens, delete the entry in the same commit that fixes the ticket; do not
// weaken the comparison to keep it quiet.
//
// `verdict` says what kind of record each one is:
//
//   * `divergent`     — the two columns contradict each other. Only a browser
//                       can see these, which is gap 2 of ti `ec235f`.
//   * `agreed-broken` — both columns agree the output is wrong. The crate's
//                       own property already reports it, so the gap was
//                       generator coverage (gap 1); the browser's contribution
//                       is confirming the harm is real in a live DOM.
//
// Blessed 2026-09-05 against the Chromium `pnpm-lock.yaml` pins.
const KNOWN_DIVERGENT = [
  {
    ticket: "9b4d66",
    verdict: "agreed-broken",
    harm:
      "the balancer deletes the orphan `</p>` that had taken the walk out of " +
      "foreign content, which puts `<title>` back inside `<svg>`, turns its " +
      "interior back into markup, and revives an impostor `data-sync-id` the " +
      "strip had correctly read as text. The crate's P6 is red here and " +
      "Chromium confirms a live element carries the plant.",
    input: '<p><ul><svg></p><title><div data-sync-id="p-0002">impostor</div></title></svg>',
    observed: {
      crateOpenAtInput: ["ul"],
      domOpenAtInput: ["ul"],
      crateOpenAtBalanced: [],
      domOpenAtBalanced: [],
      crateNextIsDirectChild: true,
      domNext: { ok: true, note: "ok" },
      crateChainedCarriesReserved: true,
      domChainedCarriesReserved: true,
      signature: "",
    },
  },
  {
    ticket: "307283/895fb7",
    verdict: "divergent",
    harm:
      "the adoption agency algorithm. HTML leaves a clone of `<b>` on the " +
      "stack inside the inner `<div>`, so the author's `</div>` closes the " +
      "INNER div and the outer one is still open at EOF; the walk closes the " +
      "nearest matching frame, reports nothing open, appends nothing, and the " +
      "pane's own `</div>` is consumed — §4a's direct-child break.",
    input: "<div><b><div></b></div>",
    observed: {
      crateOpenAtInput: [],
      domOpenAtInput: ["div"],
      crateOpenAtBalanced: [],
      domOpenAtBalanced: ["div"],
      crateNextIsDirectChild: true,
      domNext: { ok: false, note: "parent-div" },
      crateChainedCarriesReserved: false,
      domChainedCarriesReserved: false,
      signature: "open@input:dom-has-more open@balanced:dom-has-more sec4a:break-nested",
    },
  },
  {
    ticket: "307283/895fb7",
    verdict: "divergent",
    harm:
      "the same shape with the closer the author actually wrote: a browser " +
      "ends balanced, and the walk deletes that closer as an orphan.",
    input: "<div><b><div></b></div></div>",
    observed: {
      crateOpenAtInput: [],
      domOpenAtInput: [],
      crateOpenAtBalanced: [],
      domOpenAtBalanced: ["div"],
      crateNextIsDirectChild: true,
      domNext: { ok: false, note: "parent-div" },
      crateChainedCarriesReserved: false,
      domChainedCarriesReserved: false,
      signature: "open@balanced:dom-has-more sec4a:break-nested",
    },
  },
  {
    ticket: "895fb7",
    verdict: "divergent",
    harm:
      "foreign content left by a breakout END tag with an unterminated tag " +
      "behind it — the one route of the four that also breaks P1 in the Rust " +
      "properties. The browser oracle is what says WHICH of the two answers " +
      "is HTML's.",
    input: '<p/><ul><math></p><div ">',
    observed: {
      crateOpenAtInput: ["ul", "math", "div"],
      domOpenAtInput: ["ul", "div"],
      crateOpenAtBalanced: [],
      domOpenAtBalanced: [],
      crateNextIsDirectChild: true,
      domNext: { ok: true, note: "ok" },
      crateChainedCarriesReserved: false,
      domChainedCarriesReserved: false,
      signature: "open@input:crate-has-more",
    },
  },
];

// The divergence MECHANISMS the generated sweep still produces, with the case
// count each accounted for when this pin was blessed.
//
// Coarse on purpose. The first blessing of this file produced 738 distinct
// signatures when the label carried the element names, because the names are
// whatever the generator drew — `+b`, `+div`, `+desc,+div,+svg`. A pin with
// 738 entries records nothing a reader can act on and goes red on any
// generator edit. `signatureOf` therefore labels the MECHANISM (which question
// diverged, and in which direction) and the exemplar carries the names, so the
// pin stays short while the run log stays diagnostic.
//
// Exact counts, not a ceiling: the generator is a pinned-seed `splitmix64` and
// the corpus is a deterministic prefix of its stream, so these numbers are
// reproducible on every host — the same reasoning that makes
// `token_stream_pin.rs` byte-exact rather than approximate. A movement in
// either direction is red, and the failure message says which direction and
// what to do about it.
//
// Blessed 2026-09-05. **Re-blessed the same day, on a larger alphabet**, and
// the reason matters more than the numbers: ti `bebebe` (DCR-0050) fixed four
// tokenizer divergences whose states `ATOMS` could not reach — there was no
// `<plaintext>`, no `<xmp>`/`<iframe>`/`<noembed>`/`<noframes>`, no `--!>` and
// no entity-spelled attribute value in the generator — so the first blessing
// of this file measured a corpus that had nothing to say about any of them.
// Proof that the fixes alone moved NOTHING here: a corpus emitted by the fixed
// crate over the OLD alphabet is byte-identical to the one emitted at
// `14d6eda`. The movement below is the nine new atoms drawing a different
// deterministic prefix, not the crate changing its mind about an input.
//
// One entry is NEW rather than moved, and it was FILED before it was pinned,
// per this file's own instruction: see ti `bb961a` on the last one.
const DIVERGENCE_CENSUS = {
  "open@input:dom-has-more open@balanced:dom-has-more sec4a:break-nested": 841,
  //   e.g. "</1 ></foreignObject></i><br><svg><foreignObject><p></foreignObject></svg>"
  //        input +foreignobject,+p,+svg; balanced +foreignobject,+p,+svg; sec4a dom=parent-foreignobject crate=ok
  "open@balanced:dom-has-more sec4a:break-nested": 430,
  //   e.g. "<! <div> ><math/><svg><desc><div></desc><script><annotation-xml encoding=\"text&#47;html\"><annotation-xml encoding=\"text/html\"><a href=https://example.com/><foo><pé→"
  //        balanced +desc,+div,+svg; sec4a dom=parent-desc crate=ok
  "open@balanced:dom-has-more": 371,
  //   e.g. "a < b<ul><li><b></ul><iframe><!--<b><div></b></div>"
  //        balanced +b
  "open@input:crate-has-more": 235,
  //   e.g. "<p><mtext><textarea/><meta></textarea><!-- c --><!doctype html><font color=\"r\"><div /<ul>"
  //        input -mtext,-p
  "open@input:dom-has-more open@balanced:dom-has-more": 230,
  //   e.g. "  <embed></font><![CDATA[y</font></script><annotation-xml encoding=\"text&#47;html\"></script><p><table><tr><td>c"
  //        input +p,+tbody; balanced +annotation-xml,+p
  "open@input:both-ways open@balanced:dom-has-more sec4a:break-nested": 78,
  //   e.g. "</xmp><! <div> ><td>x</tr><image><keygen></1 ><math><mtext><b>q</mtext></math></script><span/>"
  //        input +b,+math,+mtext,-td; balanced +b,+math,+mtext; sec4a dom=parent-b crate=ok
  "open@input:dom-has-more": 57,
  //   e.g. "<div>data-sync-id=\"p-9\"><span/></g><p><table><tr><td>c<blockquote><div><span></blockquote><svg/>"
  //        input +p,+tbody
  "open@input:both-ways open@balanced:dom-has-more": 22,
  //   e.g. "<blockquote><div><span></blockquote><div/></mi><td>x</tr><div data-block-kind=\"paragraph\" class=\"k\">><hr><span><ul><li><b></ul>"
  //        input +b,-td; balanced +b
  "open@input:crate-has-more open@balanced:dom-has-more": 15,
  //   e.g. "< div><i><p>x</i></p><foo><tr><td>a<td>b<br>data-sync-id=\"p-9\"></image><foreignObject></font>"
  //        input -td,-tr; balanced +p
  "sec4a:break-absent": 11,
  //   e.g. "<svg><g><br></g></svg><embed><svg><desc><div></desc><script/><div</div><i>"
  //        sec4a dom=absent crate=ok
  "open@input:crate-has-more open@balanced:dom-has-more sec4a:break-nested": 8,
  //   e.g. "/div><p/><b><div></b></div><div><p><span></div><td>x</tr><![CDATA[<b>x</b>]]><div=x><![CDATA[<b>x</b>]]>"
  //        input -td; balanced +div; sec4a dom=parent-div crate=ok
  "open@input:both-ways": 4,
  //   e.g. "<p/><br/><annotation-xml><ul><p><table><tr><td>c<dl><dt>a<dd>b</dl><image>"
  //        input -annotation-xml,+tbody
  // ti `bb961a`, filed the moment this line was written and not before: the
  // only mechanism here that carries `reserved:dom-only`, i.e. a LIVE impostor
  // anchor in a real DOM that the strip believes it cleared. HTML ignores the
  // `</desc>` (in-body's "any other end tag" stops at the special `div`) and
  // stays in HTML content, where `<![CDATA[` is a bogus comment ending at the
  // first `>`; the walk pops the nearest name match, lands back in foreign
  // content, and reads one CDATA section to EOF — so everything after it is
  // text to the strip and markup to the browser. Same family as ti `9b4d66` /
  // `895fb7`, and it is pinned here rather than fixed because this file's
  // ticket does not own the open-element stacks.
  "open@input:dom-has-more open@balanced:dom-has-more sec4a:break-nested reserved:dom-only": 1,
  //   e.g. "<!doctype html><blockquote><div><span></blockquote><a href=\"x\"><div>y</a><div>z</a><svg><desc><div></desc><![CDATA[y<p><table><tr><td>c<?pi<!doctype html><div/data-sync-id=\"p-1\"></>"
  //        input +desc,+div,+div,+div,+table,+tbody,+td,+tr; balanced +desc,+div,+div,+div,+svg,+table,+tbody,+td,+tr; sec4a dom=parent-td crate=ok
};

// Bounds, so agreement cannot be reported vacuously. A change that made every
// case abstain, or that compared nothing at all, would otherwise be the
// greenest run this file has ever had. Blessed with the census above.
//
// Both rose with the alphabet (7_511 -> 7_697 agreeing, 2_941 -> 3_874
// abstentions), and the abstention rise is the expected shape rather than a
// worry: `<plaintext>` and the four raw-text names added by DCR-0050 are
// precisely the constructs that swallow the sentinel, so a corpus that now
// contains them declares more abstentions by construction. Agreement rose for
// the same reason — a fragment whose tail is text has less structure for the
// two parsers to disagree about.
const AGREEING_CASES = 7_697;
const ABSTENTIONS = 3_874;

// ---------------------------------------------------------------------------
// Falsification: cases where the CRATE COLUMN IS A LIE
// ---------------------------------------------------------------------------

// An oracle that has never gone red is not evidence, and "the pinned routes
// disagree" only shows it can contradict inputs somebody already knew about.
// These four cases are synthetic: the bytes are real and Chromium parses them
// for real, but the `crate_*` columns are HAND-WRITTEN and — for the first
// three — deliberately WRONG. Each one names the mechanism the comparison must
// report, so test `c` fails if `signatureOf` ever stops noticing.
//
// The fourth is the control. Three cases that go red prove sensitivity; a
// fourth that must stay silent proves the comparison is not simply reporting
// everything, which would make the other three pass for the wrong reason.
//
// `pane` is built by `paneOf` below rather than copied from the corpus,
// because a fabricated case has no corpus entry to copy from.
const FALSIFICATIONS = [
  {
    what: "a <div> left open, with the crate claiming nothing is open",
    input: "<div>x",
    balanced: "<div>x",
    chained: "<div>x",
    crate_open_at_input: [],
    crate_open_at_balanced: [],
    crate_next_is_direct_child: true,
    crate_next_note: "ok",
    crate_chained_carries_reserved: false,
    expected: "open@input:dom-has-more open@balanced:dom-has-more sec4a:break-nested",
  },
  {
    what: "a live impostor anchor, with the strip claiming it cleared the namespace",
    input: '<div data-sync-id="impostor">y</div>',
    balanced: '<div data-sync-id="impostor">y</div>',
    chained: '<div data-sync-id="impostor">y</div>',
    crate_open_at_input: [],
    crate_open_at_balanced: [],
    crate_next_is_direct_child: true,
    crate_next_note: "ok",
    crate_chained_carries_reserved: false,
    expected: "reserved:dom-only",
  },
  {
    what: "frames the crate invents out of plain text",
    input: "just text",
    balanced: "just text",
    chained: "just text",
    crate_open_at_input: ["b", "i"],
    crate_open_at_balanced: ["b", "i"],
    crate_next_is_direct_child: false,
    crate_next_note: "depth-2",
    crate_chained_carries_reserved: true,
    expected:
      "open@input:crate-has-more open@balanced:crate-has-more " +
      "sec4a:crate-pessimistic reserved:crate-only",
  },
  {
    what: "the control: a balanced fragment both columns describe correctly",
    input: "<div>x</div>",
    balanced: "<div>x</div>",
    chained: "<div>x</div>",
    crate_open_at_input: [],
    crate_open_at_balanced: [],
    crate_next_is_direct_child: true,
    crate_next_note: "ok",
    crate_chained_carries_reserved: false,
    expected: "",
  },
];

/**
 * The pane mount, mirroring `pane_mount` in the Rust emitter.
 *
 * A second spelling of one string is exactly the kind of duplication that
 * rots, so it exists for the fabricated cases above ONLY, and test `c` welds
 * it to the emitter's own spelling by comparing it against a real corpus
 * entry's `pane` before it trusts anything built with it.
 */
const paneOf = (balanced) =>
  `<main><div data-sync-id="p-0001">${balanced}</div>` +
  '<div data-sync-id="p-0002" data-oracle-next="1">next</div></main>';

// ---------------------------------------------------------------------------
// Reading the corpus
// ---------------------------------------------------------------------------

function readCorpus() {
  if (!fs.existsSync(CORPUS_PATH)) {
    throw new Error(
      [
        `No browser-oracle corpus at ${CORPUS_PATH}.`,
        "It is generated by the Rust emitter, not tracked:",
        "  ./scripts/test-browser.sh",
        "regenerates it into the served fixture directory before running this suite.",
      ].join("\n"),
    );
  }
  return JSON.parse(fs.readFileSync(CORPUS_PATH, "utf8"));
}

// ---------------------------------------------------------------------------
// Chromium's side
// ---------------------------------------------------------------------------

/**
 * Chromium's answers for one chunk of cases, computed in-page.
 *
 * The body runs in the browser, so it is self-contained by necessity — and by
 * design: it reads `input`, `balanced`, `pane` and `chained` off each case and
 * nothing else, so no crate answer can leak into a Chromium answer.
 */
function answerChunk([cases, sentinel, reserved]) {
  const container = document.createElement("div");

  // (a) The stack of open elements at the end of `html`, read off where a
  // sentinel start tag lands. `null` is Chromium abstaining: the fragment
  // swallowed the sentinel, so there is no insertion point to report.
  // `localName` is lowercased before it leaves the page. HTML's tree
  // construction ADJUSTS foreign tag names, so Chromium reports
  // `foreignObject` and `annotation-xml` with SVG's own capitalisation while
  // `ElementExtent::name` is documented as the lowercased name. That is a
  // naming convention, not a difference of opinion about the tree, and
  // leaving it in produced a `+foreignObject,-foreignobject` "divergence"
  // that says nothing about either parser.
  const openAtEnd = (html) => {
    container.innerHTML = `${html}<${sentinel}></${sentinel}>`;
    const probe = container.getElementsByTagName(sentinel)[0];
    if (!probe) return null;
    const chain = [];
    for (let el = probe.parentElement; el && el !== container; el = el.parentElement) {
      chain.push(el.localName.toLowerCase());
    }
    return chain.reverse();
  };

  // (b) contracts.md §4a: mount the pane and ask whether the FOLLOWING block's
  // anchor is still a direct child of the `<main>` — i.e. whether the first
  // block's own `</div>` closed the first block. `data-oracle-next` is used to
  // find it because a corpus input may forge a `data-sync-id`.
  const nextIsDirectChild = (pane) => {
    container.innerHTML = pane;
    const next = container.querySelector("[data-oracle-next]");
    if (!next) return { ok: false, note: "absent" };
    const main = container.querySelector("main");
    if (main && next.parentElement === main) return { ok: true, note: "ok" };
    const parent = next.parentElement;
    return { ok: false, note: `parent-${parent ? parent.localName.toLowerCase() : "none"}` };
  };

  // (c) Does any element in the mounted pane chain carry a reserved sync
  // attribute? Asked of the live DOM, element by element — the strip is not
  // consulted, which is the point. Names are lowercased because attribute
  // names keep their case on foreign-content elements.
  const carriesReserved = (chained) => {
    container.innerHTML = chained;
    for (const el of container.querySelectorAll("*")) {
      for (const name of el.getAttributeNames()) {
        if (reserved.includes(name.toLowerCase())) return true;
      }
    }
    return false;
  };

  return cases.map((c) => ({
    openAtInput: openAtEnd(c.input),
    openAtBalanced: openAtEnd(c.balanced),
    next: nextIsDirectChild(c.pane),
    reserved: carriesReserved(c.chained),
  }));
}

async function askChromium(page, cases) {
  const answers = [];
  for (let i = 0; i < cases.length; i += CHUNK) {
    const slice = cases.slice(i, i + CHUNK).map((c) => ({
      input: c.input,
      balanced: c.balanced,
      pane: c.pane,
      chained: c.chained,
    }));
    answers.push(...(await page.evaluate(answerChunk, [slice, SENTINEL, RESERVED_ATTRS])));
  }
  return answers;
}

// ---------------------------------------------------------------------------
// Comparing the two sides
// ---------------------------------------------------------------------------

const sameSeq = (a, b) => a.length === b.length && a.every((x, i) => x === b[i]);

/** The per-name multiset delta: positive where Chromium has more. */
function delta(dom, crat) {
  const counts = new Map();
  for (const n of dom) counts.set(n, (counts.get(n) ?? 0) + 1);
  for (const n of crat) counts.set(n, (counts.get(n) ?? 0) - 1);
  return counts;
}

/**
 * Which DIRECTION two stacks differ in — the mechanism, not the names.
 *
 * `dom-has-more` is Chromium holding frames the walk does not (the adoption
 * agency clone, the unpopped breakout root); `crate-has-more` is the walk
 * holding frames a browser has already discarded (a `<td>` outside a table);
 * `order` is the same multiset nested differently.
 *
 * Counted as a MULTISET, deliberately. A set-membership version of this read
 * `["div","div"]` against `["div"]` as neither direction, and reported the
 * pair under a third label that named no mechanism at all.
 */
function direction(dom, crat) {
  let extra = 0;
  let missing = 0;
  for (const d of delta(dom, crat).values()) {
    if (d > 0) extra += d;
    else if (d < 0) missing -= d;
  }
  if (extra > 0 && missing > 0) return "both-ways";
  if (extra > 0) return "dom-has-more";
  if (missing > 0) return "crate-has-more";
  return "order";
}

/** The names behind a direction, for the exemplar line in the run log. */
function detail(dom, crat) {
  const parts = [];
  for (const [name, d] of [...delta(dom, crat).entries()].sort((x, y) => (x[0] < y[0] ? -1 : 1))) {
    for (let i = 0; i < Math.abs(d); i += 1) parts.push(`${d > 0 ? "+" : "-"}${name}`);
  }
  return parts.length > 0 ? parts.join(",") : `order(${dom.join(">")} vs ${crat.join(">")})`;
}

/**
 * The divergence signature for one case: the empty string when Chromium and
 * the crate agree about everything they both answered.
 *
 * Coarse enough to name a class of mechanism (so the pinned census is a short
 * list a reader can act on) and specific enough that a new mechanism is a new
 * entry. `sec4a` and `reserved` carry a direction too, because which way round
 * they fail is the difference between a §4a break and mere pessimism.
 */
function signatureOf(c, dom) {
  const parts = [];
  if (dom.openAtInput !== null && !sameSeq(dom.openAtInput, c.crate_open_at_input)) {
    parts.push(`open@input:${direction(dom.openAtInput, c.crate_open_at_input)}`);
  }
  if (dom.openAtBalanced !== null && !sameSeq(dom.openAtBalanced, c.crate_open_at_balanced)) {
    parts.push(`open@balanced:${direction(dom.openAtBalanced, c.crate_open_at_balanced)}`);
  }
  if (dom.next.ok !== c.crate_next_is_direct_child) {
    // The crate believing the sibling survives while Chromium nests or loses
    // it IS §4a's direct-child break. The reverse is pessimism: the balanced
    // fragment is fine in a browser and the walk thinks it is not.
    parts.push(
      c.crate_next_is_direct_child
        ? `sec4a:break-${dom.next.note.startsWith("parent-") ? "nested" : dom.next.note}`
        : "sec4a:crate-pessimistic",
    );
  }
  if (dom.reserved !== c.crate_chained_carries_reserved) {
    // `dom-only` is the OI-0035 harm: an element in a real DOM carries a
    // reserved attribute that the strip believes it cleared. `crate-only` is
    // the strip finding bytes that never become an attribute in a browser.
    parts.push(dom.reserved ? "reserved:dom-only" : "reserved:crate-only");
  }
  return parts.join(" ");
}

/** How many of a case's questions Chromium abstained from. */
const abstentionsIn = (dom) =>
  (dom.openAtInput === null ? 1 : 0) + (dom.openAtBalanced === null ? 1 : 0);

/** The census, in a shape that can be pasted straight back into the pin. */
function pasteable(census) {
  const lines = Object.entries(census)
    .sort((a, b) => b[1].count - a[1].count)
    .map(
      ([sig, v]) =>
        `  ${JSON.stringify(sig)}: ${v.count},\n` +
        `  //   e.g. ${JSON.stringify(v.exemplar)}\n` +
        `  //        ${v.detail}`,
    );
  return `const DIVERGENCE_CENSUS = {\n${lines.join("\n")}\n};`;
}

/**
 * Stable serialization, so a pin's key ORDER cannot decide a comparison.
 * `JSON.stringify` is insertion-ordered, which would make re-typing a pinned
 * record in a different order a spurious red — the one failure mode a pin
 * whose whole job is precision must not have.
 */
function canon(v) {
  if (Array.isArray(v)) return `[${v.map(canon).join(",")}]`;
  if (v && typeof v === "object") {
    const body = Object.keys(v)
      .sort()
      .map((k) => `${JSON.stringify(k)}:${canon(v[k])}`)
      .join(",");
    return `{${body}}`;
  }
  return JSON.stringify(v);
}

/** Everything both columns said about one case, as the pin records it. */
function record(c, dom) {
  return {
    crateOpenAtInput: c.crate_open_at_input,
    domOpenAtInput: dom.openAtInput,
    crateOpenAtBalanced: c.crate_open_at_balanced,
    domOpenAtBalanced: dom.openAtBalanced,
    crateNextIsDirectChild: c.crate_next_is_direct_child,
    domNext: dom.next,
    crateChainedCarriesReserved: c.crate_chained_carries_reserved,
    domChainedCarriesReserved: dom.reserved,
    signature: signatureOf(c, dom),
  };
}

// ---------------------------------------------------------------------------

test.describe("transync-html vs Chromium — the browser oracle", () => {
  test.beforeEach(async ({ page }) => {
    test.setTimeout(SUITE_TIMEOUT_MS);
    // No bundle page: nothing here needs the shell, the engine or the
    // sanitizer, and a blank document keeps 40_000 parses off a page that
    // would otherwise be mounting and syncing panes at the same time.
    await page.goto("about:blank");
  });

  test("a — the four known-bad routes are still exactly as bad as recorded", async ({ page }) => {
    const corpus = readCorpus();
    expect(
      corpus.known_divergent.map((c) => c.input),
      "the Rust emitter's KNOWN_DIVERGENT list and this spec's must name the " +
        "same inputs in the same order — they are two halves of one record",
    ).toEqual(KNOWN_DIVERGENT.map((p) => p.input));

    const dom = await askChromium(page, corpus.known_divergent);

    const problems = [];
    const observed = [];
    for (const [i, pin] of KNOWN_DIVERGENT.entries()) {
      const now = record(corpus.known_divergent[i], dom[i]);
      observed.push(
        `  ti ${pin.ticket} (${pin.verdict})  ${JSON.stringify(pin.input)}\n` +
          `    ${JSON.stringify(now)}`,
      );
      const healthy =
        now.signature === "" &&
        now.crateChainedCarriesReserved === false &&
        now.domChainedCarriesReserved === false &&
        now.crateNextIsDirectChild === true &&
        now.domNext.ok === true;
      if (healthy) {
        problems.push(
          `ti ${pin.ticket} — ${JSON.stringify(pin.input)} is now HEALTHY: the ` +
            "two columns agree and neither reports a harm. That is good news, " +
            "and it makes this entry a lie. Delete it from KNOWN_DIVERGENT " +
            "here and from the Rust emitter's own list, in the commit that " +
            "fixed the ticket.",
        );
      } else if (canon(now) !== canon(pin.observed)) {
        problems.push(
          `ti ${pin.ticket} — ${JSON.stringify(pin.input)} behaves in a NEW ` +
            `way.\n  recorded: ${JSON.stringify(pin.observed)}\n` +
            `  observed: ${JSON.stringify(now)}\n` +
            "  Either transync-html moved or Chromium did (check " +
            "pnpm-lock.yaml). Find out which before re-blessing.",
        );
      }
    }
    console.log(`known-bad routes:\n${observed.join("\n")}`);
    expect(problems, problems.join("\n\n")).toEqual([]);
  });

  test("b — Chromium agrees with the crate over the generated sweep, apart from the pinned mechanisms", async ({
    page,
  }) => {
    const corpus = readCorpus();
    const g = corpus.generator;
    // The bound, logged rather than applied silently: a cap nobody prints
    // reads as "covered everything".
    console.log(
      `browser oracle: ${corpus.generated.length} generated cases ` +
        `(${g.browser_per_seed} of the ${g.bounded_per_seed} inputs each of the ` +
        `${g.seeds.length} pinned seeds feeds the Rust bounded run; ` +
        `${g.seeds.length * (g.bounded_per_seed - g.browser_per_seed)} generated ` +
        `inputs are NOT browser-checked), drawn from ${g.atoms} atoms and ` +
        `${g.phrases} stale-frame phrases at 1-in-${g.phrase_odds}`,
    );

    const dom = await askChromium(page, corpus.generated);
    expect(dom.length).toBe(corpus.generated.length);

    const census = {};
    let agreeing = 0;
    let abstentions = 0;
    for (const [i, c] of corpus.generated.entries()) {
      abstentions += abstentionsIn(dom[i]);
      const sig = signatureOf(c, dom[i]);
      if (sig === "") {
        agreeing += 1;
        continue;
      }
      if (!census[sig]) {
        const bits = [];
        if (dom[i].openAtInput !== null && !sameSeq(dom[i].openAtInput, c.crate_open_at_input)) {
          bits.push(`input ${detail(dom[i].openAtInput, c.crate_open_at_input)}`);
        }
        if (
          dom[i].openAtBalanced !== null &&
          !sameSeq(dom[i].openAtBalanced, c.crate_open_at_balanced)
        ) {
          bits.push(`balanced ${detail(dom[i].openAtBalanced, c.crate_open_at_balanced)}`);
        }
        if (dom[i].next.ok !== c.crate_next_is_direct_child) {
          bits.push(`sec4a dom=${dom[i].next.note} crate=${c.crate_next_note}`);
        }
        census[sig] = { count: 0, exemplar: c.input, detail: bits.join("; ") };
      }
      census[sig].count += 1;
    }

    console.log(
      `agreement: ${agreeing}/${corpus.generated.length} cases fully agree; ` +
        `${abstentions} browser-declared abstentions; ` +
        `${Object.keys(census).length} divergence mechanisms\n${pasteable(census)}`,
    );

    // Anti-vacuity first: a run where nothing was comparable would otherwise
    // satisfy every assertion below it. Exact, like the census — the corpus is
    // deterministic, so "roughly this many agreed" would only hide movement.
    expect(
      agreeing,
      "a different number of cases agree than when this was blessed. The " +
        "corpus is deterministic, so this is real movement — and if it fell, " +
        "check the abstention count before anything else: a run that compares " +
        "nothing agrees about nothing",
    ).toBe(AGREEING_CASES);
    expect(
      abstentions,
      "Chromium abstained on a different number of questions than recorded. " +
        "An abstention is the browser saying the fragment swallowed the " +
        "sentinel, so a rise means more fragments end inside an unterminated " +
        "region — either a balancer regression (ti c1f9a8) or a generator " +
        "change",
    ).toBe(ABSTENTIONS);

    const unpinned = Object.keys(census).filter((sig) => !(sig in DIVERGENCE_CENSUS));
    const fixed = Object.keys(DIVERGENCE_CENSUS).filter((sig) => !(sig in census));
    const moved = Object.entries(census)
      .filter(([sig, v]) => sig in DIVERGENCE_CENSUS && DIVERGENCE_CENSUS[sig] !== v.count)
      .map(([sig, v]) => `${JSON.stringify(sig)}: ${DIVERGENCE_CENSUS[sig]} -> ${v.count}`);

    expect(
      unpinned.map(
        (sig) =>
          `${JSON.stringify(sig)} (e.g. ${JSON.stringify(census[sig].exemplar)} — ${census[sig].detail})`,
      ),
      "Chromium and this crate disagree by a mechanism nothing has recorded. " +
        "This is a FINDING: the exact input is named above, the generator is a " +
        "pinned-seed splitmix64 so it reproduces on every host, and §4a or a " +
        "stale frame is at stake. File it before pinning it.",
    ).toEqual([]);
    expect(
      fixed,
      "a pinned divergence mechanism no longer occurs — which is good news, " +
        "and makes the pin a lie. Delete the entry in the commit that fixed " +
        "it, the way the KNOWN_DIVERGENT entries above are meant to go.",
    ).toEqual([]);
    expect(
      moved,
      "a pinned divergence mechanism changed size. The corpus is " +
        "deterministic, so this is not noise: either transync-html moved, the " +
        "generator moved, or Chromium moved (check pnpm-lock.yaml). Re-bless " +
        "from the pasteable census printed above only once you know which.",
    ).toEqual([]);
  });

  test("c — the comparison can go red: a lying crate column is caught", async ({ page }) => {
    const corpus = readCorpus();
    // Weld `paneOf` to the Rust emitter's own `pane_mount` before anything is
    // built with it. Two spellings of one string are how a §4a mount quietly
    // stops being the §4a mount.
    const real = corpus.generated[0];
    expect(
      paneOf(real.balanced),
      "paneOf and the Rust emitter's pane_mount have drifted apart, so the " +
        "fabricated cases below no longer mount the way §4a is measured",
    ).toBe(real.pane);

    const cases = FALSIFICATIONS.map((f) => ({ ...f, pane: paneOf(f.balanced) }));
    const dom = await askChromium(page, cases);

    const observed = cases.map((c, i) => signatureOf(c, dom[i]));
    for (const [i, f] of FALSIFICATIONS.entries()) {
      console.log(`falsification ${i} (${f.what}) -> ${JSON.stringify(observed[i])}`);
    }
    expect(
      observed,
      "the comparison no longer reports a fabricated crate answer. Every " +
        "assertion in tests a and b is worth exactly as much as this one: an " +
        "oracle that cannot go red is not evidence, it is decoration.",
    ).toEqual(FALSIFICATIONS.map((f) => f.expected));
  });
});
