---
type: ADR
title: The browser gets the Rust renderer as a wasm demo layer, not the CLI bundle
description: A sixth workspace member, `transync-wasm`, wraps `transync-syntax` for the browser — syntax-only dependency (charter plus the getrandom trap), a JSON-string boundary, and both view and edit modes. wasm-bindgen is pinned exactly at `=0.2.126` instead of committing a lockfile (OI-0020 stays open, with a sharpened risk); binaryen >= 121 becomes a documented host prerequisite with an explicit `wasm-opt` pass; the module is budgeted at 1,950,000 B raw / 810,000 B gzip against a measured 1,739,791 / 718,136. The CLI `--html-out` bundle deliberately carries no wasm. Owner-settled 2026-08-05.
tags: [decision, ADR-0019]
status: active
---

# ADR: The browser gets the Rust renderer as a wasm demo layer, not the CLI bundle

> **Status: accepted.** Owner-settled 2026-08-05 (design spec
> `docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md`,
> approach A1, sections S1–S8); implemented and landed the same window in six
> commits, `94d7e39`..`1d04135`. This ADR records **Track C proper** — the
> browser integration OI-0028 / DCR-0017 deliberately deferred after
> delivering only the compile path and its standing gate. It is **additive**:
> the curated facade surface, the alignment schema (`1.2.0`), the ADR-0006
> six-file bundle contract, both shipped shells, and `sync.js` are all
> untouched.

## Context and Problem Statement

CLAUDE.md's tech-stack layering has reserved a WASM path since the beginning,
for one reason: **JS must never parse Markdown**. Two parsers means two
opinions about block boundaries, and block-ID correspondence — the only sync
currency (ADR-0001, invariant 1) — dies the moment they disagree. So a browser
that needs to render or re-render Markdown locally must get *our* parser, not
its own.

DCR-0017 made that reachable but not real. `crates/transync-syntax` compiles
for `wasm32-unknown-unknown` under a standing gate, and nothing more: there are
no `#[wasm_bindgen]` entry points, no JS boundary, no demo, and no build
pipeline. OI-0028 was explicitly scoped to the *compile path* and closed on
that basis; the integration was left as post-0.2.0 work. DCR-0019 then shrank
the path the browser would carry — deleting `Document::hierarchy` (a whole
extra parse pass) and `Block::parent_id` — precisely so this wave would ship a
lean module.

What remained undecided is everything a shipped browser module actually needs:
which crate wraps the syntax layer and what it may depend on, what crosses the
JS boundary, how the module is built and by whom, how large it is allowed to
be, and — the question with the widest blast radius — **whether the CLI's
`--html-out` bundle should carry the module too**, since that bundle is the
only artifact real readers ever open.

## Decision Drivers

* **One parser, still.** Anything that puts Markdown parsing in JS is
  disqualified before it is evaluated. The renderer crosses to the browser or
  the browser does without.
* **The wasm dependency set is a cliff, not a slope.** `transync-core` cannot
  compile for `wasm32` — tokio's `rt-multi-thread` is a `compile_error!` there,
  and tiktoken-rs is pulled for a single backoff path. Worse, both routes reach
  `getrandom`, which on `wasm32-unknown-unknown` needs an explicit backend or
  fails to link. A single careless dependency edge turns a working module into
  an unbuildable one.
* **Reproducibility without a lockfile.** `Cargo.lock` is uncommitted here
  (OI-0020, blocked by a machine-global gitignore). wasm-bindgen requires
  **exact** crate↔CLI version identity: a resolution drift does not degrade,
  it breaks the build outright, and the repair path (`cargo install
  wasm-bindgen-cli@<v>` from source) needs a network this machine measured as
  restricted.
* **Prerequisites must fail loudly.** The wasm gate's own precedent (DCR-0017:
  the hook fails with a `rustup target add` hint rather than skipping) applies
  to every new host tool. A silently-skipped build step is a gate that has
  quietly stopped being real.
* **Payload honesty.** The entire shipped JS payload is ~42 KB. Anything that
  multiplies that by an order of magnitude has to earn it with capability a
  reader actually gets.
* **Editing must not approach the ID-stability wall.** The document is assumed
  static (invariant 8). Any in-browser editing design that lets the user change
  the *block structure* walks straight into a non-goal.
* **Everything existing stays byte-stable.** This wave may add files and gate
  lines. It may not touch the shells, `sync.js`, the bundle contract, or the
  curated surface.

## Considered Options

1. **A dedicated `transync-wasm` member depending on `transync-syntax` only,
   web-demo scope.** Bindings live in their own crate; the CLI bundle is
   unchanged.
2. **`#[wasm_bindgen]` entry points inside `transync-syntax` itself.** No new
   member; the base crate grows a `wasm` feature.
3. **A `transync-wasm` member, *plus* shipping the module inside the CLI's
   `--html-out` bundle**, so every generated bundle can re-render locally.

## Decision Outcome

**ACCEPTED: option 1.** Option 2 is rejected outright — it requires a
`[features]` table on `transync-syntax`, which is exactly the
feature-unification trap OI-0028 Option B chose the crate split to avoid, and
which the standing charter forbids by name. Option 3 is rejected on payload and
redundancy grounds, argued below.

The owner settled five points, all binding.

### 1. Crate shape — `transync-syntax` and nothing else

`crates/transync-wasm` is the workspace's **sixth** member
(`crate-type = ["cdylib", "rlib"]`, `publish = false`), and among workspace
crates it depends on **`transync-syntax` alone**. The restriction carries a
double rationale, and both halves are load-bearing:

* **The charter.** `transync-syntax` exists to be the wasm-clean layer. A
  bindings crate that reached past it into `transync-core` would make the
  crate boundary decorative.
* **The `getrandom` trap.** `transync-core`'s `wasm32` dependency tree reaches
  `getrandom` through tokio/tiktoken; `transync-syntax`'s does not. This is not
  a stylistic preference — it is the difference between a module that links and
  one that does not, and it would be discovered at link time, late, by whoever
  widened the dependency.

Logic lives in `engine` — plain Rust, zero wasm-bindgen — with only thin
`#[wasm_bindgen]` wrappers in `lib`. That split is what makes the whole surface
**host-testable**: `cargo test -p transync-wasm` exercises the real call
sequences, and the wrappers stay obviously correct by inspection. `cdylib` +
`rlib` also means the ordinary `cargo build/test/clippy --workspace` sweeps the
new member with no special-casing (bindgen exports are inert on the host).

`transync-syntax`'s own manifest is **untouched**: still no `[features]`, still
no `transync-core` dependency. The standing gate simply widens to
`cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
in both pre-commit hook copies and `scripts/smoke.sh`.

### 2. wasm-bindgen is pinned exactly; OI-0020 stays open

`wasm-bindgen = "=0.2.126"` in `[workspace.dependencies]` — an exact pin, not a
caret range. Given no committed lockfile, the pin is the only defense against
clean-checkout skew on a dependency whose version mismatch is fatal rather than
degrading.

**The escalation this wave produced, recorded honestly so the owner can
revisit.** The pre-implementation analysis expected the binding constraint to
be `slug → comrak`, i.e. a wasm32-side edge sharing a resolution with a
dependency we already had. That is **wrong**, and the correction matters:

* `slug`/`comrak` declares only a caret `"0.2"` and never constrained anything.
* The binding edge is **host-side**: `reqwest → js-sys` inside
  `transync-openai`, where `js-sys 0.3.97` **exact-pins**
  `wasm-bindgen = "=0.2.120"`. Because cargo resolves one wasm-bindgen for the
  whole workspace lock, a host-side exact pin was holding the *wasm32* tree
  down.
* Unification therefore needed a **seven-package** update
  (`wasm-bindgen` + macro/macro-support/shared, `js-sys`, `web-sys`,
  `wasm-bindgen-futures`), not the one-liner the plan assumed. `reqwest` itself
  did not move, and all of `transync-openai`'s tests stayed green, so the
  js-sys/web-sys bump is behaviorally inert here.
* **The sharpened risk:** that seven-package resolution exists **only in the
  gitignored `Cargo.lock`**. A future wasm-bindgen release moves fresh `js-sys`
  resolutions, and a clean checkout can then land on a `js-sys` whose own exact
  pin **conflicts with our `=0.2.126`** — a hard resolver failure originating
  in a `reqwest` bump that looks entirely unrelated to wasm.

**Owner decision 2026-08-05: pin over lock.** OI-0020 stays OPEN and
un-retracted; the pin is the mitigation, this paragraph is the warning, and the
recovery is a matching `-p js-sys` update (or, finally, committing the lock).

### 3. binaryen is a host prerequisite, and `wasm-opt` is invoked explicitly

**The trap, reproduced before deciding:** wasm-pack 0.15 carries a cached
binaryen **117**, and binaryen 117 *rejects* current rustc output —
`Bulk memory operations require bulk memory [--enable-bulk-memory]`. Its
optimization step is therefore not merely suboptimal, it is blocking. A second
trap sits behind it: wasm-pack only honours
`[package.metadata.wasm-pack.profile.{dev,release,profiling}]` and **silently
ignores** the key for a user-defined profile, so the metadata flag-list route
cannot fix it under `wasm-release`.

`scripts/build-wasm.sh` therefore builds with `--no-opt` and runs `wasm-opt`
itself, resolved **once by absolute path** via `command -v` so the invocation
can never drift back onto the cached 117, with the five proposal flags spelled
out (`--enable-bulk-memory`, `--enable-reference-types`,
`--enable-nontrapping-float-to-int`, `--enable-sign-ext`,
`--enable-mutable-globals`). Anything below binaryen **121** is rejected with a
named path and an install hint. Missing wasm-pack, missing `wasm-opt`, an old
`wasm-opt`, and an unparseable `--version` all fail **loudly** — never a silent
skip, per the DCR-0017 gate precedent.

**Install-channel record (2026-08-05).** The spec's decision 2 says "brew
binaryen (≥131)". On the implementing host `brew install binaryen` was
**impossible**: `/opt/homebrew` is owned by another uid, verified as a real
filesystem permission rather than a sandbox artifact, and Homebrew's own remedy
(`sudo chown -R` over the whole prefix) is a far larger host mutation than the
one approved. What was installed instead is the **same version brew ships**:
the upstream release `binaryen-version_131-arm64-macos.tar.gz`, SHA-256
verified (`e441b48dc22163d209b4f05e44dc7210909b01237642b6c9ae48fd710a3ef83e`;
the chain was re-verified independently at review), unpacked to
`/Volumes/Common/local/opt/binaryen-131/` with `bin/wasm-opt` symlinked into
`/Volumes/Common/local/bin/` — this machine's existing convention for
user-prefix tools — ahead of `/opt/homebrew/bin` on PATH.

**This deviation is host-local by construction, not a fork of the decision.**
The script never names brew, a prefix, or a path: it requires *a* `wasm-opt`
≥ 121 on PATH and resolves whatever it finds. `brew install binaryen` remains
correct advice for a machine with a writable brew prefix, which is why the
failure hints still say exactly that. The prerequisite is "binaryen ≥ 121 on
PATH", and the channel is the host's business.

### 4. Size budget — 1,950,000 B raw / 810,000 B gzip

`scripts/build-wasm.sh` asserts the ceilings and fails the build on a breach,
so the number is a gate rather than a note. Measured today: **1,739,791 B raw /
718,136 B gzip** (1.66 MiB / 701 KiB), byte-reproducible across clean rebuilds
on the implementing host.

The spec's original ceilings (1,400,000 / 500,000, from a 1,237,349 B probe)
were **factually wrong input, corrected by measurement** — the exploration
probe compiled **view mode only**. The full attribution chain is recorded
durably in **DCR-0020**; the short form is that edit mode's Markdown
*reserialization* (`regen::regenerate` + `outcome::html_outcomes`) costs
+435,770 B that a render-only probe could not see. The corrected budget was
owner-approved 2026-08-05, and the spec and plan carry matching dated
corrections.

Two decision-relevant readings of that table:

* **The JSON-string boundary is cheap** — +42,127 B — so approach A1's
  "no `serde-wasm-bindgen`, no new dependency, one `JSON.parse` per call"
  choice is vindicated on size grounds, not merely on dependency grounds.
* **A breach is a design signal**, not a number to bump reflexively. The
  ceilings sit ~210 KB raw / ~92 KB gzip above the measurement; the one known
  recoverable slice (+113,844 B of fat-LTO scope lost to the `rlib` crate-type
  sitting beside the `cdylib`) is ticketed as `92ac61b9` rather than spent.

*(Dated note, 2026-08-05 — appended, nothing above rewritten. Ticket
`92ac61b9` was resolved by **spending** that slice: `crate-type` is now
`["cdylib"]` alone, so decision 1's manifest line and this bullet's "rather
than spent" both read as history. Re-measured A/B at the crate's then-current
size, the drop returns **101,228 B raw / 46,758 B gzip**, and the enforced
ceilings came down with it to **1,840,000 / 760,000** — the same ~12 %
headroom. Decision 1's host-testability claim is unaffected and was re-verified
rather than assumed: `cargo test` compiles the lib target into its own test
harness whatever the crate-type, so `cargo build/test/clippy --workspace` still
sweeps the member with no special-casing. DCR-0020's attribution table carries
the numbers.)*

### 5. Web-only — the CLI bundle deliberately carries no wasm

The module lives in the `web/` workspace demo (`web/demo-wasm.html` +
`web/js/wasm-demo.js`, artifacts in the gitignored `web/wasm/`) and its
Playwright suite. `HTML_BUNDLE_ENTRIES` is unchanged and the ADR-0006 six-file
contract is untouched. Two independent arguments, either of which suffices:

* **Payload.** The module is ~41× the entire current 42 KB JS payload. (The
  spec argued 30× from the view-only probe; the shipped measurement makes the
  same argument *stronger*, not weaker — the bundle would have carried
  ~1.74 MB, not ~1.2 MB.) Shipping it would also inflate the CLI binary by that
  much, since bundle assets are embedded.
* **Redundancy.** A `--html-out` bundle already ships **rendered HTML**. The
  wasm module's view mode reproduces those exact fragments — that is its parity
  test — so the bundle would pay 1.74 MB to recompute what it already contains.

The capability that is *not* redundant — local editing — belongs to a demo, not
to a published reader artifact.

### Editing scope, and why it does not touch invariant 8

Edit mode is deliberately **per-block payload** editing of the *translated*
side only: the user replaces one block's Markdown, and `rebuild` regenerates,
re-aligns, and re-renders. The block **set** is structurally preserved, because
block identity comes from parsing the *source* document, which the demo never
edits. The ID-stability wall (documents are static; ID survival across edits is
a non-goal) is therefore never approached rather than merely avoided.

Edit mode runs the **whole** loop — regen → alignment map → render — instead of
reusing the fetched `alignment.json`. Re-using a stale map after an edit
silently mis-slices html and skipped blocks, whose render arms read
`target_range` **bytes** that the edit has shifted (probe-confirmed hazard, not
a theoretical one). Rebuilding per edit eliminates the stale-range trap by
construction.

## Consequences

* Good, because the standing invariant survives contact with the browser: the
  demo renders both panes with the *same* Rust renderer, byte-identically to
  the CLI's fragments, and JS still owns only interaction. The alternative that
  was always available — a JS Markdown parser — stays unbuilt.
* Good, because the wasm-clean boundary is now enforced **twice**: by the crate
  charter (no `[features]`, no core dependency) and by a gate line that names
  both crates. A widening edge fails at `cargo check`, not at link time in
  someone's browser.
* Good, because the browser gets a capability the bundle genuinely lacks —
  editing a translated block and seeing the panes re-render, re-align, and
  re-sync locally — rather than a heavier way to do what the bundle already
  does.
* Good, because every new host prerequisite fails loudly with a named path and
  an install hint, and the size ceiling is a build failure rather than a
  comment.
* Bad, because **`transync-wasm` cannot be built from a clean checkout without
  luck**. Its build needs wasm-pack, binaryen ≥ 121, the `wasm32-unknown-unknown`
  target, *and* a dependency resolution that satisfies `=0.2.126` — and the
  last of those lives only in an uncommitted lockfile. This is OI-0020's cost,
  now concentrated on a critical path instead of spread thin, and it is the
  strongest argument yet for committing the lock.
* Bad, because the demo is **not** a supported package. It is not bundled, not
  published, not versioned separately, and its artifacts are gitignored build
  output; a consumer who wants in-browser rendering must build it themselves
  from this repo. Track D's "JS sync ships as an unpolished demo" posture now
  covers a second, heavier demo.
* Bad, because a second schema mirror exists. `web/js/wasm-demo.js` carries its
  own `KNOWN_SCHEMA = "1.2.0"`, joining both `sync.js` copies. Mitigated rather
  than removed: it is checked **twice** at boot — against the wasm module's own
  `schema_version()` (build-time drift: a blob from a different commit) and
  against the fetched map (data drift) — and both checks fail closed. A
  host-side test pins `schema_version()` to `ALIGNMENT_SCHEMA_VERSION`, so the
  blob can never silently disagree with the crate it was built from.
* Bad, because html blocks are **read-only** in the demo. Their payload is an
  extracted text-segment array (ADR-0018), not Markdown, so a Markdown slice
  could not be spliced back; the demo declines the edit rather than offering
  one it cannot round-trip.

  **2026-08-05 — the consequence runs one step further than "read-only".**
  Read-only is not free: the segment array is unrecoverable from the rendered
  bundle (the demo only ever sees the spliced result), so an html block cannot
  be *carried* through a rebuild either. `regen` falls it back to its **source**
  bytes, and any *other* block's edit therefore reverts every translated html
  block in the document. That degrade is accepted — but it must be visible, so
  `buildEditModel` omits html rows from the edit model's `statuses` map as well
  as its `payloads`. `align::build_alignment_map` then synthesizes
  `fallback_source` for unit-backed html rows, and the renderer presents them
  as the escaped `data-skipped="html-block"` placeholder under the fallback
  tint, in **both** panes. Preserving the map's original `translated` instead
  would have left the row — and the tint legend — asserting a translation the
  pane no longer shows. Pinned host-side in `transync-wasm`
  (`omitted_unit_backed_html_row_is_fallback_source_not_translated`) and in
  browser test `c`.
* Neutral, because `comrak` moved to `default-features = false` workspace-wide
  to shrink the module (the `cli`/`syntect` extras are used nowhere,
  grep-verified). It is a manifest-only change measured as behavior-identical
  across every test binary, and it benefits the host builds too.
* Neutral, because `panic = "abort"` in `[profile.wasm-release]` is safe here:
  the syntax API returns `Result` rather than using panics as control flow, and
  `transync-cli`'s stock `release` profile is untouched.

## Amendment (2026-08-07) — the lock was committed after all; the pin stays

Decision 2 above chose "pin over lock" and recorded that OI-0020 "stays OPEN and
un-retracted". **The owner reversed that one day later, on 2026-08-06, in commit
`6cf4164`:** `.gitignore` gained a repo-local `!Cargo.lock` un-ignore overriding
the machine-global rule, and `Cargo.lock` is now tracked. The seven-package
resolution decision 2 says "exists **only in the gitignored `Cargo.lock`**" is
therefore a tracked file, and the clean-checkout failure mode recorded there — a
future wasm-bindgen release moving fresh `js-sys` resolutions into conflict with
`=0.2.126` — no longer sits in front of a first build. **OI-0020 is RESOLVED
(2026-08-06)**; `docs/project/open-issues-archive.md` carries the resolution and names
the two things that survive it: the `CLAUDE.md` half of the same gitignore
concern, declined rather than fixed, and the absence of any `--locked` gate over
the committed lock.

**What is unchanged: the exact pin.** `wasm-bindgen = "=0.2.126"` stays in
`[workspace.dependencies]` and must not be loosened. Decision 2's reasoning for
an exact pin survives the lock intact, because a lockfile binds only the
resolves that read it — `cargo update`, a regenerated lockfile, and any build
after the lock is deleted all re-resolve from the manifests — and because
wasm-bindgen's requirement is crate↔**CLI** identity, of which no lockfile can
pin the CLI half at all (`wasm-bindgen-cli` is a host tool installed outside the
workspace). What changed is the pin's standing, not its necessity: it was the
*only* defense against clean-checkout skew and is now the half that covers the
lock-less resolves, with the lock covering the checkout. The root manifest's
comment beside the pin and `docs/project/release-checklist.md` step 18 both say
this. The front-matter description, the "Reproducibility without a lockfile"
driver and decision 2's heading and text are left exactly as written — they
record the 2026-08-05 state, and this amendment is the correction.

## Related

- **DCR-0020** — the implementing record for this decision: files, gate lines, the boundary contract, the edit model, the durable size-attribution table, and the parity evidence
- DCR-0017 / **OI-0028** — delivered the compile path and the standing gate, and deferred exactly this work; OI-0028 stays RESOLVED and gains a dated "Track C proper landed" note
- DCR-0019 — shrank the path this module ships (`Document::hierarchy` and `Block::parent_id` deleted) specifically so Track C would carry less; its IR freeze rule held, and this wave is where the "serde-vs-DTO for the JS boundary" question it deferred is answered — **neither**: JSON strings cross the boundary and the IR is not reshaped
- **OI-0020** — no committed `Cargo.lock`; **stays OPEN** by owner decision 2026-08-05, with the sharpened clean-checkout risk recorded in decision 2 above — **superseded 2026-08-06** (`6cf4164`): the lock is committed and OI-0020 is RESOLVED, while the exact pin stays; see the 2026-08-07 amendment above
- ADR-0003 — the workspace decision, **amended 2026-08-05** by this ADR: six members, `transync-wasm → transync-syntax`
- ADR-0006 — the renderer's output shape and the six-file bundle contract: **unchanged**, and decision 5 is the reason it is unchanged
- ADR-0001 — block-level alignment as the sync currency: the reason a second Markdown parser is not an option and the renderer crosses instead
- ADR-0018 — HTML content via segment extraction: why html blocks are read-only in the demo (their payload is a segment array, not Markdown)
- ADR-0004 — comrak as the GFM parser: unchanged; `default-features = false` drops `cli`/`syntect` only
- CLAUDE.md tech-stack layering (the WASM path) and invariants 1, 7, 8
- `docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md` — the approved design spec (its decision-2 "brew binaryen" wording carries a dated pointer to the install-channel record above)
