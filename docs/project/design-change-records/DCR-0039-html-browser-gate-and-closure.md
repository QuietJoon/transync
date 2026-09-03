---
type: DCR
title: The browser gate closes the HTML→HTML feature, and the ledger says what stayed open
description: Wave 7 of ti 490d97 — the last. web/tests/scn16.spec.js drives the HTML-run bundle through the shipped shell (bidirectional block-id sync, the title as chrome rather than pane content, a console clean of engine warnings), falsified against four mutated copies of the real bundle; the docs-drift weld grows to four spec files; and the closure ledger dispositions every deferred item from waves 0–6 with the owed spec amendments batched for the controller.
tags: [change, project-control, DCR-0039]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-04T00:00:00Z
status: stable
---

# DCR-0039: The browser gate closes the HTML→HTML feature

- **Date:** 2026-09-04
- **Source:** ti `490d97`, wave 7 of 8 — **the last** — spec `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12 wave 7, with §8, §11, §13, §14 and §15 binding
- **Affected contracts:** `docs/architecture/contracts.md` §6 (one sentence — the fidelity door)
- **Affected DCRs:** none amended; this record **dispositions** the hand-forwards of DCR-0032 through DCR-0038

**Numbering note.** DCR-0039 was reserved by this wave from the start and was still
free at execution (verified). DCR-0040–0048 were taken by unrelated work in the
meantime, which is exactly the collision the wave-2 plan warned about and the reason
the reservation held.

## What changed

**`web/tests/scn16.spec.js`** — three tests over the wave-6 bundle leg's `/scn16/`
shell: fetch, DOMPurify fail-closed mount, `sync.js`, the way an operator meets an
HTML run's `--html-out`. `engine.spec.js`'s test `n` drives the same pane files
*without* the shell; this is the shell-driven half wave 6 deliberately left here.

- **a** — the served map is the 1.3.0 HTML shape: `input_format: "html"`, exactly one
  `title` row and it is `non-sync` and `html`-spelled and not a fallback, every row's
  `source_format` declared.
- **b** — the mount, and the title as chrome. **Every negative rides a positive**
  (the plan's Deviation 5): the backbone is *each pane's anchor list `toEqual` the
  map's anchor-role row ids, in order*. That one assertion catches an empty pane, a
  wrong selector, a renamed id, a leaked non-sync row (the title **or** the fixture's
  `<hr>`), a dropped block and a mis-ordered pane — as a surplus or a hole. The title
  negatives (id absent, text absent from both panes) sit on that base, and the title's
  text is asserted **present** in the one place D5 puts it: `page.title()`. Plus the
  D9 structural pair (`main > li` is 0; `main > ul > li[data-sync-id]` is ≥ 2) and the
  2026-08-21 ruling's targeted assertion — the `<x-note>` block's anchor is a `DIV`.
- **c** — bidirectional sync by block id over HTML-derived anchors: forward on the
  last `h2-` anchor, reverse to the first, both driver ids derived from the map rather
  than hard-coded, with loud guards for pane overflow and target reachability.

**Zero console warnings, scoped honestly** (Deviation 4). `harness.collectConsole`
records message *text*; the acceptance needs the *type*, because browser resource
noise arrives as an `error`-typed network message while everything the engine says
arrives through `console.warn`. The collector is inline in the spec, registered
**before** `page.goto` so mount-time warnings are captured, and the asserted set is:
zero `warning`-typed messages, zero uncaught page errors, zero `error`-typed messages
carrying the engine's `transync:` prefix.

**The weld grew to four spec files, in one commit with the three documents that must
name them.** `SPEC_FILES`, `SUITE_MENTIONS` and the panic message's advice all moved
together, and `docs/implementation/module-map.md`, `docs/Developer_Guide.md` and
`web/SMOKE.md` name `scn16.spec.js` in the same commit — ticket `729ec8`'s rule
applied whole, because a `SPEC_FILES` entry no `MUST_NAME_SPECS` document names is an
unpinned file. No document states a count (ti `e9481b`).

**The scenario matrix** gained SCN-16's row (the SCN-15 combined-type precedent), the
`title` and HTML-document block-kind rows, and one out-of-scope bullet recording what
ADR-0025 deliberately leaves out. **No strike-through**: HTML documents were never on
that list — the CLI's preamble-sniff refusal, not an out-of-scope entry, is what had
kept them out, so imitating the "Raw HTML left this list on 2026-08-04" precedent
would have annotated an item that never existed.

**The serve door** (§9) is documented in `docs/Developer_Guide.md` and `contracts.md`
§6: an `--input-format html` run's `--out-dir` holds `out.html` — full head, scripts,
styles, anchor-free — beside the sync bundle, and serving that directory is how you
see the real page. The panes are a sync surface, never a fidelity preview (D6).

**The living-document honesty sweep** (Deviation 2, plus the review-found rows 28–31):
README's feature bullet and block-ID flow line, README's pipeline diagram (the intake
edge, the regen box, and a `schema 1.2.0` label the wire left at wave 6), Quick_Start's
HTML-run variant, `docs/Troubleshooting.md`'s marker-less recovery rule (which stated
the four-name set wave 6's Deviation 7 replaced with a predicate, and did not know
`out.html`), and the Developer Guide's bundle-title chain (whose middle rung is the
source `<title>` on an HTML run).

**The backlog** gained cross-references on five entries whose ground truth the feature
changed, and four new entries for what wave 7 cannot close.

## Falsification evidence — the gate can fail

A characterization gate over a landed feature is green on its first honest run, so
what replaces red-first is the falsification protocol: each key assertion driven red
against a **mutated copy of the real bundle**, in the temp fixture dir, zero repo
edits. Captures: `/Volumes/Temp/claude/ti490d97-wave7/gate/probe{1,2,3,4}.txt`, each
`PW_EXIT=1`; the probe fixture was deleted after.

| Probe | Mutation | Assertion driven red |
|---|---|---|
| **P1** | a title-id anchor spliced into both pane files | test **b**'s anchor-list equality — `Received +1`, the title's id as a surplus at the tail. Behind it, the wave-1 unlisted-anchor mount warning fired verbatim: `transync: ignoring anchor "title-0001" in source pane — no alignment row claims it`, so `engineNoise` is a live channel and not a dead assertion |
| **P2** | the target pane's `<ul>` wrapper dissolved, leaving bare `<li>` as direct `<main>` children | test **b**'s D9 count for `#target` — `Expected: 0`, `Received: 2`. **This is also the measurement behind amendment I**: the ungrouped pane reached the assertion *intact*, which is direct evidence that DOMPurify 3.2.6 neither relocates nor repairs a bare `<li>` |
| **P3** | the map's title row re-labelled `sync_role: "anchor"` | test **a** first and deterministically — `Expected: "non-sync"`, `Received: "anchor"`; tests **b** and **c** red behind it, the layered net a D5 regression cannot slip |
| **P4** | the panes regressed to the **pre-ruling** presentation: the anchor self-injected into `<x-note>`'s own open tag | test **b**'s anchor-list equality — a **hole** where `html-0013` should be, both panes. DOMPurify removed the unknown element with its anchor and kept only the text |

**P4 is the probe no earlier wave could run.** Every prior browser proof deliberately
bypassed the sanitizer — wave 6's test `n` is documented "no shell, no fetch, no
DOMPurify" — so the pre-ruling defect passed every gate that existed before this one.
The 2026-08-21 ruling was made on a measurement; this is the first time the hazard it
closed has been demonstrated **through the shipped shell**.

## The deferred-item ledger

Every hand-forward, owed amendment and §14/§15 item from waves 0–6 and the spec, with
what the landed tree showed. This is the shape addition to the house DCR form
(Deviation 7): no DCR-0032…0038 carried a total ledger, only its own forwards.

| # | Item | Recorded where | Verdict at execution |
|---|---|---|---|
| 1 | `web/tests/scn16.spec.js`, shell-driven, wired | spec §12; DCR-0038 | **DISCHARGED** — `0578272`. Wiring is testDir discovery + wave 6's bundle leg; `test-browser.sh` moved by comments only, verified by a diff showing only `#`-prefixed lines |
| 2 | Scenario-matrix SCN-16 + block-kind rows + out-of-scope clause | spec §10, §12 | **DISCHARGED** — `7f95835` |
| 3 | Serve-door documentation | spec §9, §12 | **DISCHARGED** — `210447f` (Developer_Guide + contracts §6) |
| 4 | Backlog / open-issue cross-references | spec §12 | **DISCHARGED** — `3fb2ff8`. See the finding below: the OI-0035 entry had already been fixed by the 2026-08-24 `reopen` sweep, so only the Type-2 **preamble** half was still owed |
| 5 | CHANGELOG / status / phase-state | spec §12 | **DISCHARGED** — this commit |
| 6 | "The remaining contracts sections" (§10's list) | spec §12; DCR-0038 | **DISCHARGED, nothing owed** — twelve presence probes all ≥ 1, then §0/§1/§3/§4/§4a/§5a/§6 read clause by clause against §10. Every clause present. Capture: `gate/t6-contracts-sweep.txt`. The wave's only contracts edit is row 3's fidelity sentence |
| 7 | The docs-index link for this spec | spec §12 | **Already satisfied** (one link, verified). Its **annotation** was stale in a new direction and is **fixed here** — see "The index step, taken rather than handed off" |
| 8 | §14.1 — the PHRASING widening's sentence splitting, owner-mandated re-confirmation | spec §14.1, §13.5; D4 | **STAYS OPEN, evidence staged and it NARROWS the question** — ti `84bf37`, backlog `phrasing-custom-element-mid-sentence-review`. See the §14.1 section below |
| 9 | Amendment A — §7 check-3's "no token in `scan_tags`" | DCR-0032, repeated by DCR-0036 | **STAYS OPEN — batched for the controller** (item A) |
| 10 | Amendment B — §4's `<!DOCTYPE html>` MEASURED clause | DCR-0032 | Same batch (item B) |
| 11 | Amendment C — §4's img-exception sentence | DCR-0035 | Same batch (item C) |
| 12 | Amendment D — §12's wave-4 "parallel with 3" | DCR-0036 | Same batch (item D) |
| 13 | Amendment E — §6's context-projection hazard, stated backwards | DCR-0037 | Same batch (item E) |
| 14 | Amendment F — §7's over-broad "walk must never be called" | DCR-0038 | Same batch (item F) |
| 15 | Amendment G — §8 steps 3/4, both halves | DCR-0038 + the 2026-08-21 ruling | Same batch (item G) |
| 16 | Amendment H — §11's "only sanctioned diff" is three parts | DCR-0038 | Same batch (item H) |
| 17 | D11 — Markdown-island reclassification | ADR-0025 D11; spec §13.9 | **STAYS OPEN with a home** — new Type-3 backlog entry `markdown-island-reclassification`, blocked by a schema-2.x-shaped window |
| 18 | §15.4 oversize HTML leaf blocks (+ §15.5's splice watch item) | spec §15.4/§15.5; DCR-0034 | **STAYS OPEN with a home** — new Type-3 entry `html-oversize-leaf-block-split`, §15.5 folded in as its watch note |
| 19 | The `parser` → `intake::markdown` rename | DCR-0035 | **STAYS OPEN with a home** — new Type-1 entry `parser-intake-markdown-rename` |
| 20 | §15.3 DOMPurify `ALLOW_DATA_ATTR` verification | spec §15.3 | **Already discharged by wave 1** — verified at the STOP gate (`scn13.spec.js`, 2 hits) |
| 21 | §15.1 pane module home / §15.2 `ElementExtent` shape / §15.6 `prompt_html` body | spec §15 | **Already discharged** by waves 6 / 3 / 5 — verified via DCR-0038 / DCR-0035 / DCR-0037 |
| 22 | DCR-0036's Hard-arm prefix residual | DCR-0036 | **Already discharged by wave 5** — re-pinned in `an_html_hard_failure_speaks_the_twins_vocabulary` |
| 23 | Wave 5's empty panes; wave 0's second strip call site; wave 2's `Document.format` render/pane guard; wave 5's sniff-pin boundary note | DCR-0038 | **Already discharged by wave 6** |
| 24 | Living-doc honesty no wave assigned (CLAUDE.md, README, Quick_Start) | this plan's sweep | **DISCHARGED** — `210447f`, with the tracking finding below |
| 25 | `docs_index_drift` standing allowance | wave 6 | **Not needed** — the allowance existed for a controller hand-off that does not exist in this session, so the index moved with the record and the gate stays green |
| 26 | ti `490d97` itself | TicGit | **Not closed by the implementer** — the feature-complete verdict and the close ride the owner after acceptance |
| 27 | Amendment I — §8 step 6's relocation mechanism is measured-false | this plan's 2026-08-21 review | Same batch (item I) — **and now measured a second time**, by probe P2 |
| 28 | `docs/Troubleshooting.md`'s out-dir rule | this plan's review sweep | **DISCHARGED** — `210447f` |
| 29 | The Developer Guide's bundle-title chain | this plan's review sweep | **DISCHARGED** — `210447f` |
| 30 | `docs/backlog.md`'s Type-2 preamble framing OI-0035 as open | this plan's review sweep | **DISCHARGED** — `3fb2ff8`. This was the half of the record gap still owed |
| 31 | README's pipeline diagram | this plan's review sweep | **DISCHARGED** — `210447f` (intake edge, regen box, and the 1.2.0 label) |

**One hand-forward the plan could not have listed, and it is discharged too.**
DCR-0037 filed ti `c887bc` — the zero-width erasure gap it found while looking for a
layer-6 test lever — as an owner decision, deliberately unfixed "in a wave that adds
no validator". It was **resolved earlier the same day as this wave** (DCR-0048,
`039ae3c`/`585d6e6`): the finding turned out to be far wider than filed (all six
Markdown kinds, both HTML paths, and the layer-6 twin's net a singleton), and closing
it forced wave 5's routing-proof lever to change — `DissolvesRun` now works from a
ZWSP-only source run, so **layer 2 owns "the words are gone" and layer 6 owns "the
block is gone."** Wave 5's proof stands and still speaks the twin's vocabulary.

**Ledger totality check** (acceptance 6): the hand-forward sections of DCR-0032
through DCR-0038 were collected mechanically into `gate/acceptance-ledger.txt` and
every named item appears above. One item is older than this feature and has its own
home rather than a row here: DCR-0032's `transync-html` file-as-module split, which
is carried by `module-size-versus-inline-tests` (OI-0043).

## The owed spec amendments — batched for the controller, not applied

The spec file is **byte-untouched** by this wave (Deviation 3): every wave that owed
an amendment recorded it §14-style rather than editing the spec, and wave 5 explicitly
handed its amendment "to the controller alongside the index line". The last wave keeps
that convention rather than being the one that breaks the arc's record discipline.

Each item is *location → the sentence to replace → the replacement wording*.

- **A — §7, check-3's rationale.** Replace "`<!DOCTYPE>` and comments produce no token
  in `scan_tags`, so a dropped doctype or comment is ledger-invisible" with
  "`<!DOCTYPE>` and comments leave no **ledger entry** — since DCR-0032 they produce
  `TagToken::Skip`, and `tag_inventory` filters `Skip` — so a dropped doctype or
  comment is ledger-invisible."
- **B — §4, rule T's measured clause.** Amend to "**MEASURED (pre-wave-0 scanner):**
  `<!DOCTYPE html>` produced no token and no skip before DCR-0032's bogus-comment
  state; the sentence records the rationale for adding `Skip`, not the shipped
  scanner."
- **C — §4, the img exception.** Replace with the implemented reading:
  "**Exception:** a run that is textless after excluding absorbed phrasing markup and
  `Skip` spans — no naked byte outside those spans — and that contains one or more
  `img` elements becomes **one** `Image` block (`<a><img></a>`,
  `<picture><img></picture>`, and two adjacent `<img>` tags are each ONE block, never
  two and never gap), so images keep a sync anchor instead of dissolving into gap
  (DCR-0035; pinned by `a_linked_image_keeps_its_anchor_as_one_image_block`)."
- **D — §12, wave 4.** Append "*(Amended: 'parallel with 3' held for checks 1, 3, 4
  and the finalize branch; check 2 consumes `intake::html::parse`, and in the executed
  ordering wave 4 followed wave 3 — DCR-0036.)*"
- **E — §6, context projection.** Replace "an HTML `<h1>` would put raw tags into the
  heading stacks and `context_hash`" with "comrak sees an Html-spelled heading as one
  leaf `HtmlBlock` with no inline children, so it projected to **empty prose** —
  collapsed heading stacks and degraded `context_hash` identity, not markup leakage
  (measured by wave 2's pin; DCR-0037). The extract-projection remedy is unchanged."
- **F — §7, the walk sentence.** Replace "`walk::*` takes no HTML arm either: it is
  the *Markdown* layer-6/renderer pairing and must simply never be called on an HTML
  document — enforced by the format branch in `finalize`, not by adding arms" with
  "the comrak-typed consumers of `walk` (`reparse_full`, `render_fragment` — the
  Markdown layer-6/renderer pairing) must never run on an HTML document, enforced by
  the format branch in `finalize`, not by adding arms; `walk::normalize_top_level`
  itself is comrak-free IR projection and serves §8 step 6's `li` grouping on HTML
  documents (DCR-0038)."
- **G — §8, steps 3/4, both halves.** Append "A void element records no extent in the
  §5 walk, so an img-run block — the lone `<img>` included — presents no outermost
  element and takes step 4's transparent wrapper: the instrument's verdict, not a
  special case (DCR-0038). And a block whose outermost element is **unknown to §4's
  tables** — a custom element, a future element — takes the transparent wrapper even
  when it presents a complete extent: the shipped shell mounts panes through
  DOMPurify's untouched fail-closed config, which removes an unknown element and every
  attribute riding it, so a self-injected anchor would die in the mount (measured
  against the vendored 3.2.6, and demonstrated end to end by wave 7's probe P4). The
  rule is keyed on §4's tables, never on the sanitizer's allowlist — duplicating that
  allowlist in Rust would be a second opinion about what the sanitizer accepts, the
  same class of sin as a second Markdown parser; any element §4 does not know fails
  safe into the wrapper (the 2026-08-21 ruling; DCR-0038)."
- **H — §11.** The review-gate row's "these two additive fields as the *only*
  sanctioned alignment-output diff" becomes "the `schema_version` string plus these two
  additive fields — the three-part sanctioned alignment-output diff"; and the cheap
  pin's sentence is re-worded to §3's definition (`input_format == "markdown"`; every
  row's `source_format` equals `block_kind == "html" ? "html" : "markdown"`) —
  DCR-0038.
- **I — §8, step 6's mechanism (erratum).** The step's ground sentence — "a bare `<li>`
  as a direct `<main>` child gets relocated by DOMPurify and the anchor moves with it"
  — is **measured-false**: DOMPurify 3.2.6 (the vendored `web/vendor/purify.min.js`)
  passes a bare `<li data-sync-id=…>` through **unchanged, in place**; relocation is a
  table-family parser behaviour (`<tr>` without `<table>`), not `<li>`'s. Replace the
  mechanism with the true ground: an ungrouped pane is structural drift the suite's
  per-pane count/equality assertions catch, and the grouping is mandated by D9's
  model — item anchors inside one reconstructed group — not by any sanitizer
  behaviour. The step's instruction (group consecutive `li` blocks under one shared
  `<ul>`/`<ol>`) does not change. Measured twice: the 2026-08-21 review probe, and
  wave 7's probe P2, which drove the ungrouped pane red on exactly the structural
  assertion.

## §14.1 — staged, and the measurement narrows the question

The owner required the PHRASING widening's sentence splitting be re-confirmed against
real output. Wave 7 staged that and **cannot** take the ruling. Artifacts:
`/Volumes/Temp/claude/ti490d97-wave7/gate/phrasing-evidence/` (inputs, maps, exit
captures) and `gate/phrasing-sequences.txt`. Ticket: **`84bf37`**.

The finding contradicts the plan's own expectation, and informatively:

- **Wrapped in `<p>` — the split does NOT happen.** Three sentences each carrying a
  mid-sentence custom element (`price-tag`, `fa-icon`, `router-link`) yielded
  `title heading-1 paragraph paragraph paragraph html` — one `paragraph` per sentence,
  not three blocks each. The `<p>` is the element block and the custom element lives
  inside its extent, so the sentence never crosses units. Only the block-level
  `<my-callout>` became its own `html` block, which is intended.
- **Naked prose — the split fires, exactly as §14.1 describes.** The same sentence
  directly inside a `<div>` yielded `p-0002 html-0003 p-0004`: run, stopped element,
  run. One sentence, three translation units.
- **Known phrasing is absorbed either way** — `<em>` keeps its run whole.

For context, the in-tree fixtures show no mid-sentence split:
`scn-16-html-document.html` → 14 blocks, `html-corpus/marketing-page.html` → 14 blocks
with paragraphs intact and `html` blocks interleaved at block level.

**So the hazard is confined to prose that is not wrapped in an element**, which real
pages rarely are — and §14.1's own example (`Price: <my-price/> today`) is written in
the naked form without saying that the form is what makes it split. The decision is
therefore narrower than "accept sentence splitting": it is whether *naked*
mid-sentence custom elements are worth a lever. The ticket carries three options
(amend §14.1 to state the wrapping condition; add a per-run phrasing-extension list;
leave it) and one prohibition (do not make unknown elements PHRASING by default — the
sanitizer deletes them, which is why the wrapper ruling exists).

## Findings — where the landed tree disagreed with the plan

The plan was written 2026-08-20 and executed 2026-09-04; five premises moved, and the
landed tree won each time.

1. **`CLAUDE.md` is not tracked here.** It is matched by a *global* gitignore
   (`/Volumes/Common/git/gitignore_global`), so the plan's `git add CLAUDE.md` could
   not have worked. The edit was made anyway — a stale `CLAUDE.md` misinforms every
   agent, which is wave 0's own rationale — and is local only.
2. **`docs/investigation/` is gitignored too**, and it held the sweep's only stale
   claim: `constraints-and-debt.md`'s "HTML document intake is preparatory, not
   implemented", written 2026-08-24 between wave 2 and wave 3. Annotated in place
   (struck through, resolved, with the reason it is kept as a worked example of debt
   being paid off) rather than rewritten, since it is a generated register. The useful
   consequence: **no tracked living document calls HTML→HTML translation
   unimplemented.**
3. **`open-issues.md` holds ten more OPEN entries than the plan expected** — OI-0039
   through OI-0048, from review waves that landed after the plan was written. The
   plan's substantive claim survives: none of the ten is an HTML→HTML item. Recorded,
   not edited, per the plan's own instruction for this case.
4. **Half of the OI-0035 record gap had already been fixed.** The `reopen` sweep of
   2026-08-24 added a resolved note to the backlog entry, so Deviation 6's first half
   was overtaken. The **Type-2 preamble** half was genuinely still owed and is what
   wave 7 fixed. The entry itself gained only the one fact neither the note nor the
   description carried (the accepted residual), because the backlog's own header
   forbids it becoming a restatement of another document.
5. **`template-webcomponent-extraction` is a Type-3 entry now**, not Type 2, and the
   three telemetry-gated html entries carry `reopen` deferral-audit notes the plan did
   not know about. Annotations were appended in the landed house shape.

## What did NOT move

- **The engine: zero bytes.** `web/js/sync.js`, `crates/transync-cli/web/sync.js` and
  `web/js/wasm-demo.js` are untouched — the acceptance diff over `crates/` and
  `web/js/` names exactly one file, `crates/transync/tests/docs_browser_suite_drift.rs`
  (8 insertions, 2 deletions), and the diff over fixtures, `web/js` and
  `web/tests/support` is **empty**. Capture: `gate/acceptance-surface.txt`.
- Every crate's `src/`; every fixture; every `Cargo.toml` and `Cargo.lock`; the three
  pre-existing browser specs and the shared harness; `playwright.config.js`.
- The spec file (the batch above is the controller's).
- No exit code, no schema motion, no cache axis, no prompt bytes.

## The index step, taken rather than handed off

The plan reserved `docs/index.md` for a controller and expected `docs_index_drift`
**red** in the interim, naming DCR-0039's unlinked file. That separation assumed a
second agent. There was none: one session executed the wave, and leaving a workspace
gate red on purpose would mean reporting a red gate as acceptable. So the two
controller items were taken here, and named as such:

1. **DCR-0039's index line** was added, in the same shape every DCR line uses.
2. **The spec's own annotation** was stale in a new direction and is corrected. It
   read "**Waves 3–7 are unstarted** … and wave 4's layer-6 twin is a hard
   precondition for any HTML translation run" — true when the controller last
   corrected it, false the moment wave 3 landed. It now reads that waves 3–7 all
   landed 2026-09-03/04 with their DCR numbers, that the feature is implemented and
   complete, and that the v0.5.0 window it opened is still open with the release cut
   pending.

Nothing else in `docs/index.md` moved. The spec file itself is still byte-untouched —
the A–I batch above remains the owner's to apply.

## Verification status — what is proven, and the one gate that is not

**Proven, with captures under `/Volumes/Temp/claude/ti490d97-wave7/gate/`:**

- `scripts/test-browser.sh` — **42 passed, `SUITE_EXIT=0`**, `[test-browser] OK`, with
  the three `scn16.spec.js` tests listed as passed (`t2-suite.txt`; the baseline before
  this wave was 39). This is §12's acceptance line, measured.
- The four falsification probes, each `PW_EXIT` non-zero on its named assertion
  (`probe1..4.txt`), probe fixture deleted after.
- `docs_browser_suite_drift` green around every commit that touched a GUARDED document
  — `t3-red.txt` (the predicted three-line red), `t3-green.txt`, `t4-weld.txt`,
  `t5-weld.txt`.
- The §10 contracts sweep (`t6-contracts-sweep.txt`) and the open-issue verification
  (`t7-issues.txt`).
- The code surface: the diff over `crates/` and `web/js/` names exactly one file, and
  the diff over fixtures, `web/js` and `web/tests/support` is empty
  (`acceptance-surface.txt`).
- The baselines this wave started from, all green (`baseline-*.txt`).

**NOT proven, and deliberately not claimed: the closing full `cargo test --workspace`
and `cargo test -p transync-cli --features test-stub-provider` runs.** Both suites
contain tests that **spawn the built `transync` binary**, and on this machine macOS
Gatekeeper/`syspolicyd` intermittently parks a spawned binary before it executes —
measured repeatedly during this wave: `0.0% CPU, 0:00.00 total CPU time` over 5+
minutes of elapsed time on a binary whose bytes had not changed since a run that
succeeded half an hour earlier. It cleared spontaneously twice (once after ~11 minutes,
once after 3 seconds) and then recurred. The captures for those two runs have **no exit
marker**, which per this repository's own rule is *lost evidence, never green*, so no
number from them is recorded here.

What this does and does not mean: every document edit in this wave is inert to the Rust
suites except through the docs-drift welds, and those were run and captured green around
each commit that touched a guarded document. The remaining exposure is the welds this
wave did not individually re-run (`docs_gate_claims_drift`, `docs_ownership_drift`,
`reader_honesty`, `docs_cli_flags_drift`, `docs_index_drift`, `exit_code_docs_drift`).
The full-suite run should be repeated once the machine condition is cleared — the
documented remedy is a reboot — and the result appended here.

## Feature status, and what closure does not claim

**All eight waves of ti `490d97` (0 through 7) have landed.** The HTML→HTML document
translation feature is complete: a second intake, a layer-6 twin for it, units and
context and prompt over HTML documents, the alignment wire at schema 1.3.0, both panes
derived, a CLI flag, and a browser gate that proves the bundle syncs in a real browser
through the shipped shell.

This record closes the **feature**, not the release. The sanctioned v0.5.0 breaking
window is still open, and **cutting v0.5.0 is a separate owner decision** through
`docs/project/release-checklist.md`. The ti `490d97` close is the owner's after
acceptance.
