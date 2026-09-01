---
type: DCR
title: The HTML mechanics become a workspace member, and the tag scanner grows the two things an HTML intake needs
description: htmlseg.rs moved verbatim out of transync-syntax into transync-html with no re-export alias; splice's CommonMark u8 became a BlankLinePolicy; TagToken::Open gained the span it already computed and TagToken::Skip named the regions the scanner passed over silently; element_extents was lifted out from under balance_fragment rather than copied; strip_reserved_sync_attrs landed in its final home ahead of its caller.
tags: [change, project-control, DCR-0032]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-20T00:00:00Z
status: stable
---

# DCR-0032: The HTML mechanics become a workspace member

- **Date:** 2026-08-20.
- **Source:** ticket `490d97`, **wave 0** of the eight in
  `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md` §12.
  Wave 0 is the base of that dependency order (0 → 2 → {3, 4} → 5 → 6 → 7,
  with wave 1 parallel to 2–5); every later wave sits on the crate this one
  creates.
- **Post-implementation**: the record follows the code it describes and lands
  as the wave's **closing commit** — not inside the last code commit, which is
  `272be80`. Nine commits
  carry the wave — `1d6f19d` (the extraction, 28 paths), `e52ea44` (the
  token-stream pin), `240868d` (`BlankLinePolicy`), `61adf66` (the golden
  hatch's env-var interlock), `ac2e0bd` (`Open.span` + `TagToken::Skip`),
  `86961cf` (the policy matched exhaustively), `3f3c2f0` (`element_extents`
  lifted out), `9d39e43` (`ElementExtent`'s doc), `272be80`
  (`strip_reserved_sync_attrs`).
- **No ADR in this wave.** ADR-0025 records the spec's twelve decisions as one
  architectural commitment and lands in wave 2 (spec §12). Until it exists the
  name-continuity note the crate name owes lives in the crate doc — see *The
  name, and the two notes it owes* below.
- **Affected ADRs:** none amended. ADR-0003 (Cargo workspace with provider
  crates) is the shape this follows rather than changes; ADR-0018 (HTML
  content is translatable via app-owned segment extraction) is untouched — the
  engine it sanctions moved house, it did not change its mind.
- **Affected contracts:** `docs/architecture/contracts.md` §0 tier-(c) prose
  only — the crate is named there together with the statement that the facade
  re-exports **nothing** from it. The §0 **table** does not move, which is why
  `crates/transync/tests/public_surface.rs` gains only a *forbidden* path
  segment and no row.

## Not breaking, and it ships regardless

Nothing on the `transync` facade's §0 surface moves. The HTML mechanics were
always tier (c) — engine internals reached through the pipeline, never
re-exported — so relocating them across a crate boundary is invisible to every
consumer of `transync`. The version does not move; no schema constant moves;
`public_surface.rs` gains one name to the *hidden* list and nothing to the
documented one.

That matters beyond bookkeeping. Wave 0's value does **not** depend on the
rest of the HTML→HTML feature landing: the mechanics layer is a coherent
crate on its own terms, the scanner is more honest than it was, and the
`6 | 7` CommonMark rule now has exactly one home instead of being inlined in a
function that will soon serve a non-CommonMark host. If waves 1–7 were
cancelled tomorrow this refactor would still be worth having, and it would not
need reverting.

## What moved, and why there is no alias

`crates/transync-syntax/src/htmlseg.rs` became `crates/transync-html/src/lib.rs`
as a `git mv`, so the diff reads as a rename: 52 insertions and 14 deletions
over a **1278**-line file, landing at 1316 — all of them crate-doc rewording and
the visibility
changes listed below. Not one line of the extract/splice algorithm, the
tokenizer, or the balancer changed in that commit. `transync-syntax` dropped
`lol_html` and `htmlize` and gained `transync-html`; `transync-core` gained it
too; both crates' call sites moved from `crate::htmlseg::…` /
`transync_syntax::htmlseg::…` to `transync_html::…`.

**There is deliberately no `pub use transync_html as htmlseg;`.** A hidden
alias looks like kindness and is not:

- It is what spec §5 calls *"the 'misleadingly-named thin wrapper' D3
  rejects"* — a module in `transync-syntax` still called `htmlseg` that is not
  the HTML mechanics but a signpost to them, so a reader who opens it learns
  nothing about the code and a reader who greps it lands in the wrong crate.
  (D3 itself is the layer-axis-not-format-axis decision; the wrapper phrase is
  §5's application of it to this seam.)
- Every old path keeps compiling, so the migration never finishes. The call
  sites this wave repointed — spec §5's complete grep inventory named five
  modules (`outcome`, `regen`, `render`, `unit/payload`, `validate`), and
  `transync_html::` appears fourteen times across `transync-syntax/src` and
  `transync-core/src` today counting the tests and doc comments that name it —
  would have stayed unrepointed, and wave 3's intake would have been written
  against whichever spelling its author reached for first.

The cost of no alias is that **no module path `htmlseg` exists anywhere in the
tree**, while the shipped dated records that name it — the ones spec §10
enumerates, and a good many more besides — go on naming it. (Two scoping notes,
because the loose version of this sentence invites a reader to disprove it and
stop trusting the rest: the *string* `htmlseg` does survive in-tree, in
`public_surface.rs`'s forbidden list, in `docs_ownership_drift.rs`'s comment,
and in this crate's own doc — deliberately, since that doc is where a reader who
greps the old name is meant to land. And the record count is larger than spec
§10's enumeration, which strengthens the name-continuity note rather than
weakening it.) That cost is paid in the next section rather than avoided.

## The name, and the two notes it owes

`transync-html`, not `transync-htmlseg` (decision **D10**). "Segment"
under-describes a crate that also owns tokenization (`scan_tags`), the pairing
discipline (`is_void`, `is_raw_text`, `implicitly_closes`), element extents,
fragment balancing and the reserved-attribute strip; segment extract/splice is
one of its six capabilities, not its subject.

The rejected name was argued partly on grep continuity, and D10 has to buy
that back. Two notes do it, and they run in **both** directions:

- **Forward:** structural intake — classification, `BlockKind` assignment,
  block ids, `ast_path` — is `transync-syntax`'s job, not this crate's. The
  crate is named for the capability, not for a claim to decide what an HTML
  block is.
- **Backward:** the module formerly called `htmlseg` inside `transync-syntax`
  **is** this crate. ADR-0018, ADR-0003, `docs/project/stub-manifest.md`,
  `docs/project/status.md`, `docs/project/open-issues-archive.md`,
  `docs/project/implementation-slice-checklists.md` and the investigation
  bundle all name `htmlseg`, and they are **dated records, which this project
  does not rewrite**. A reader who greps the old name and finds nothing needs
  somewhere to land.

Both live in `crates/transync-html/src/lib.rs`'s crate doc today, because
ADR-0025 — where the spec's §10 puts the same sentence — is wave 2's. Both
also live in `CLAUDE.md`'s new `crates/transync-html` layout entry, which is
the file an agent reads before the records.

## The four non-mechanical changes

Everything else in the wave is a move or a rename. These four are not, and
each replaced something specific.

**1. `splice`'s third parameter became a `BlankLinePolicy`.** It was
`block_type: u8`, and `splice` computed `matches!(block_type, 6 | 7)` inline —
a CommonMark fact living inside a function whose subject is HTML. Now the
parameter is `blank_lines: BlankLinePolicy` (`Collapse` | `Keep`), and
`BlankLinePolicy::from_commonmark_html_block_type` is the **one** surviving
home of the `6 | 7` rule; its domain is CommonMark's 1–7 plus the `0` a caller
with no CommonMark context has, and anything outside it resolves to `Keep`
because the safe arm is the one that touches nothing. The Markdown call sites
(`regen`'s `BlockKind::Html` splice and `validate`'s layer 3) pass through the
constructor and behave identically. The point is what comes next: **every
splice in an HTML *document* will run `Keep`**, because nothing ever Markdown-
reparses that output, and with a `u8` the only way to say so would have been
to invent a fake block type.

`86961cf` closed the follow-up the same signature opened. The wave's standing
constraint — every enum this crate publishes stays exhaustive, no
`#[non_exhaustive]`, no `_ =>` arm — was written for `TagToken` and read as
covering only it, so a `matches!(blank_lines, BlankLinePolicy::Collapse)`
passed review as compliant. `matches!` **is** a catch-all: it expands to a
match with an implicit `_ => false`, so a third policy variant would silently
take the `Keep` branch instead of stopping the compiler. The call is now an
exhaustive `match` with both arms named, and the constraint says so.

**2. `TagToken::Open` carries the span it already computed.** The scanner
knew each start tag's `<`-through-`>` byte range and threw it away at push
time; `Close` had carried its span since the balancer needed it to delete
orphans. This is a pure widening of a value already in hand — no new scan, no
new state — and it is what lets a consumer say *"the element around these
bytes"* without re-tokenizing.

**3. `TagToken::Skip { span }`, and the bogus-comment state it required.**
This one is a real tokenizer change, not a new label on old behaviour. The
scanner already stepped over `<!-- … -->` and `<![CDATA[ … ]]>` silently;
`Skip` names those regions so an intake can trim the anonymous runs between
elements instead of guessing where they are. But HTML has a third case the
scanner did not model: `<!…>` and `<?…>` open the **bogus-comment** state,
which ends at the first `>`, and tag-shaped bytes inside one are **not**
markup. Before this wave `<! <div> >` tokenized the `<div>` as a real open
tag, and `<!doctype html>` produced **no token and no skip at all** — the `!`
failed the ascii-alphabetic first-byte test, so the bytes were ordinary text
to the scanner. Both are now `Skip` spans.

The change is browser-correct, and it is inert over everything the shipped
consumers see. That is measured, not assumed: the only `<!`-shaped bytes in
`crates/transync/tests/fixtures` are a single `<!--` comment in
`scn-15-html-blocks.md`, and the pin's twelve edge cases include
`doctype-upper` (`<!DOCTYPE html>\n<p>after</p>`), `doctype-lower`,
`cdata-foreign` and `cdata-html` — whose `Open`/`Close` streams, tag
inventories and balanced output are byte-identical across the commit that
introduced `Skip`. It is not a no-op in general: `<! <div> >` no longer
tokenizes the `<div>`. The Task 2 pin exists precisely to make that difference
visible the day it stops being inert.

**4. The visibility opening.** Six items went from private or `pub(crate)` to
`pub`: `is_void`, `is_raw_text`, `TagToken`, `scan_tags`, `implicitly_closes`
(all private inside the old module), and `balance_fragment` (`pub(crate)`).
They are the same code; what changed is that a sibling crate can now reach
them, which is the whole reason a crate boundary was drawn here. Everything
that stayed in — `scan`, `scan_chunks`, `scan_with_memory_cap`, `NodeRecord`,
`collapse_blank_lines`, `rewriter_settings`, `rewriter_err`,
`wanted_text_type`, `is_foreign_root`, `AttrState`, `VOID_ELEMENTS`,
`RAW_TEXT_ELEMENTS`, `DEFAULT_MAX_MEMORY_BYTES` — stayed `pub(crate)` or
private.

## Two things added, one of them with no caller yet

**`element_extents` / `ElementExtent` was lifted out from under
`balance_fragment`, not copied.** The balancer already walked the token stream
with an open-element stack, tracking depth, implicit closes, orphan closes and
what was still open at EOF. There is now exactly **one** such walk — a private
`Walk` returning `extents`, `orphan_closes` and `unclosed` — which
`element_extents` reads for structure and `balance_fragment` reads for
repairs. A second copy was the obvious cheap move and it is the one thing this
change exists to prevent: two walks that disagree about HTML is the same class
of defect as two Markdown parsers that disagree about a block.

`9d39e43` wrote down what the walk already enforced rather than what a reader
might assume: a self-closing tag outside raw-text/RCDATA is never pushed, so
it produces **no** `ElementExtent` at all. A consumer needing "the element
around these bytes" must handle the empty case — a block whose only element is
a lone `<img>` has zero extents.

**`strip_reserved_sync_attrs` landed in its final home with no call site.**
It removes the six reserved sync-attribute names (`data-sync-id`,
`data-block-kind`, `data-order`, `data-fallback`, `data-parent-id`,
`data-skipped`) from element open tags, case-insensitively, taking each one's
leading whitespace with it, and borrows when nothing matched. This is OI-0035
route (c)'s render half. It ships here rather than in wave 1 for one reason:
wave 1 would otherwise write it in the old crate and move it a commit later,
which is the same file churning twice for no gain. Its callers are wave 1's
and wave 6's.

## Evidence

Every claim below is a measurement taken against this tree, not an assurance.

- **The full workspace suite green with zero fixture or expectation edits.**
  `cargo test --workspace -- --test-threads=4` reports **1086 passed, 0
  failed, 6 ignored** at the close of the wave, against **1066 / 0 / 5** in
  the baseline capture taken at Task 1 Step 1 — twenty more passing and one
  more ignored, all of them coverage this wave wrote (the ignored one is the
  pin's regeneration hatch), and not one existing expectation moved. That last
  half is the measurement that matters:
  `git diff --stat a96589b..HEAD -- 'crates/*/tests/fixtures' 'crates/*/tests/scenarios' web/tests`
  prints **nothing** (`a96589b` is the wave's baseline commit, recorded at
  Task 1 Step 1). That emptiness is the whole acceptance argument for a pure
  refactor: the suite that was green before is green after, over inputs nobody
  touched.
- **The two-package wasm gate exit 0, command string unchanged.**
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  is the same string it was; `transync-html` is covered **transitively**
  because `transync-syntax` depends on it, which is why the new crate carries
  the same no-`[features]` prohibition and adds a no-workspace-member-dependency
  one.
- **`workspace_publication.rs` green on a seven-member roster in dependency
  order:** `transync-html`, `transync-syntax`, `transync-core`, `transync`,
  `transync-openai`, `transync-anthropic`, `transync-cli`. `transync-wasm` is
  still the single `publish = false` member.
- **The token-stream pin, generated before the token change and byte-identical
  after it.** `crates/transync-html/tests/goldens/` was written by `e52ea44`
  and has been touched by no commit since — `git log -- <goldens>` names
  exactly one commit, and `git diff --stat e52ea44..HEAD -- <goldens>` is
  empty. `e52ea44` precedes `ac2e0bd`, the commit that added `Skip`, so the
  goldens are genuinely pre-change evidence rather than a re-blessing. The
  pin's regeneration hatch is `#[ignore]`d **and** interlocked behind an
  environment variable (`61adf66`), so it cannot be run accidentally to make a
  red pin green.

## Welds that moved with it

The refactor's real surface area is the set of tests and scripts that assert
facts about the workspace's shape. All of them rode `1d6f19d`:

- **The forbidden path segment** — `crates/transync/tests/public_surface.rs`'s
  `forbidden` list gains `"transync_html"` beside the existing `"htmlseg"`, so
  contracts.md §0 can never document a path through the new crate.
- **The publication roster** — `crates/transync/tests/workspace_publication.rs`'s
  `PUBLISHED_MEMBERS`, six → seven, `transync-html` first because the order is
  a dependency order.
- **The workspace dependency entry and its two unwelded comments** — root
  `Cargo.toml` gains `transync-html = { version = "0.4.0", path = … }`, and
  the two prose comments no test reads were corrected by hand: the
  publication-set comment (six → seven members, and the dependency-order list
  it spells out) and the versioning comment (bumping `[workspace.package]
  version` means bumping four → **five** requirements in the same edit).
- **The ownership-drift roots, and their new per-root floor** —
  `crates/transync/tests/docs_ownership_drift.rs`'s `CRATE_ROOTS` became a
  three-tuple carrying each root's minimum module count: 5 for
  `transync-syntax`, 5 for `transync-core`, **0** for `transync-html`. The
  floor is per root because `transync-html` is a single file; a shared floor
  of 5 would assert a file-as-module split nobody has decided on yet, and the
  two roots above it are what keep the anti-vacuity check meaningful.
- **The rustdoc-gate crate list** — `scripts/lib/rustdoc-gate.sh`'s
  `RUSTDOC_GATE_CRATES` gains `transync-html`. This one is only exercised by
  `./scripts/smoke.sh`, never by `cargo test --workspace`, because the gate
  has a completeness check that fails the run when a member is missing from
  the list.
- **Seven documents** — `docs/architecture/contracts.md`,
  `docs/architecture/source-of-truth-table.md`, `docs/architecture/README.md`,
  `docs/implementation/module-map.md`, `docs/project/release-checklist.md`,
  `docs/Developer_Guide.md`, `docs/backlog.md`.

## Two spec sentences this wave made stale, recorded as owed rather than edited

The spec file is **not touched by this wave**, following the same convention
wave 3's plan states as its deviation 5: an implementation wave records what
it owes the spec, and the amendment lands with the record or the wave that
next opens the file. These are §14-style post-implementation items.

Task 4 gave `<!…>` / `<?…>` a `TagToken::Skip`, so `<!DOCTYPE html>` now
*does* produce a scanner token. **Both sentences' conclusions survive** —
`tag_inventory` filters `Skip` (`TagToken::Skip { .. } => None`), so those
regions remain invisible to the ledger — but the mechanism each states is now
wrong, and both are load-bearing where they sit.

1. **§7, the layer-6 twin's check-3 rationale.** It reads: "`<!DOCTYPE>` and
   comments produce no token in `scan_tags`, so a dropped doctype or comment
   is ledger-invisible". The correct form is *no **ledger entry** —
   `tag_inventory` filters `Skip`*. This is the amendment that matters most:
   the sentence is the stated justification for the one check in the HTML twin
   that has no Markdown counterpart (gap byte-identity), so a reader who
   verifies the mechanism and finds it false has reason to doubt the check
   itself. Wave 4's plan carries the corrected wording into its own doc
   comments and repeats this amendment as owed.
2. **§4, rule T.** It reads: "**MEASURED: `<!DOCTYPE html>` produces no token
   *and no skip* in today's scanner**". This is a *rationale for adding
   `Skip`*, not a live claim — "today's scanner" meant the pre-wave-0 scanner,
   and the sentence is correct about what it describes. It is listed anyway
   because the phrase reads as present-tense to anyone arriving after this
   wave. The amendment is a clarification ("in the pre-wave-0 scanner"), not a
   correction.

## Handed forward

- **The file-as-module split** the spec permits "later". `transync-html` is one
  `lib.rs` today — 1316 lines as moved, 1952 at the end of the wave — and the
  ownership-drift floor of 0 records that as a state nobody has decided against
  rather than a shape anyone chose.
- **`ElementExtent`'s exact field shape** (spec §15 item 2) — the seam is
  agreed, the fields are refined jointly when wave 3's consumer exists. The
  commitment that cannot move: the walk lives in `transync-html`, next to the
  balancer, and there is exactly one of it.
- **`strip_reserved_sync_attrs`' call sites** — wave 1's (OI-0035 closure) and
  wave 6's (the HTML panes). It is `pub` with no in-tree caller until then,
  which is deliberate and is the reason its own unit tests are the only thing
  exercising it.

## Affected areas

- `crates/transync-html/` (new member: `Cargo.toml`, `src/lib.rs`,
  `tests/token_stream_pin.rs`, `tests/goldens/`)
- `Cargo.toml` (root), `Cargo.lock`
- `crates/transync-syntax/` — `Cargo.toml`, `src/lib.rs`, `src/outcome.rs`,
  `src/regen.rs`, `src/render.rs`, `src/parser.rs`
- `crates/transync-core/` — `Cargo.toml`, `src/unit/payload.rs`,
  `src/validate.rs`, `src/validate/per_kind.rs`
- `crates/transync/tests/` — `public_surface.rs`, `workspace_publication.rs`,
  `docs_ownership_drift.rs`
- `scripts/lib/rustdoc-gate.sh`
- The seven documents listed under *Welds that moved with it*, plus
  `docs/index.md`, `CHANGELOG.md`, `docs/project/status.md` and
  `docs/project/phase-state.yaml`
- `CLAUDE.md` — four statements this wave made false (the workspace version and
  member count, the member enumeration, the publication-roster and `wasm32`
  counts, and the `transync-syntax` module split that still listed `htmlseg`).
  It is edited **on disk only**: `CLAUDE.md` is untracked in this repository by
  owner decision, recorded in `.gitignore` beside the `Cargo.lock` carve-out
  ("CLAUDE.md stays ignored by the same owner decision") and enforced by the
  machine-global ignore, so it appears in no commit — including this one.

## Migration / follow-up

- **In-tree consumers** use `transync_html::…`. There is no `htmlseg` path and
  no alias; a stale path is a compile error by design.
- **Out-of-tree consumers are unaffected**: nothing here was ever on the
  `transync` facade's surface.
- **`transync-html` gets no `[features]` table and no workspace-member
  dependency, ever.** `transync-syntax` sits on top of it and must keep
  passing the two-package `wasm32` gate; `transync-syntax` additionally keeps
  its own no-`[features]` and no-`transync-core`-dependency rules, including
  dev-dependencies.
- **Waves 1–7 are unstarted.** Wave 2 — the IR split — is the one that needs a
  sanctioned breaking window, carries the version to `0.5.0-dev`, and blocks
  every wave after it.

### Amendment 2026-08-21 (ti 549b20) — the scanner aligns with the browser it was measured against, and an unterminated tag becomes a named region

*Appended, not a rewrite. Everything above stands as written.*

Wave 0 task 6's adversarial review broke `strip_reserved_sync_attrs` on a
constructed input, the owner ruled on the fix, and task 8 landed it —
plus one sibling divergence in the same scanner state, measured into
scope during the task. Three tokenizer divergences, all in
`AttrState::Outside`:

- **A bare quote opened a phantom value.** HTML's tokenizer makes a quote
  not preceded by `=` part of the attribute **name**; the scanner opened
  a quoted value instead. One stray quote desynchronized its quote state
  from every browser's.
- **An unterminated tag aborted the whole scan.** The post-attribute-loop
  `break` left the *document* loop, so every byte after the desync point
  was invisible to the scanner — and to the strip. Measured through the
  vendored DOMPurify 3.2.6: `<div "> <p data-sync-id="v">x</p>` kept a
  live `P#v` anchor the strip never saw, while the well-formed control
  was stripped correctly. That contrast was the finding.
- **A stray `=` opened a value the browser never opens.** HTML's
  *before attribute name* state makes `=` a parse error that **starts an
  attribute named `=`**, and a following quote joins that name. Measured
  in headless Chromium: `<div ="> data-sync-id="v">x</div>` parses as one
  attribute named `="` with the tag ending at the **first** `>` and
  ` data-sync-id="v">x` painted as text (`live_sync_ids` empty — not an
  anchor bypass). A scanner that opens a value there swallows that `>`,
  and the strip then deletes text a browser paints.

The third fix is inside the ruling, not an expansion of it: the chosen
option is "align `AttrState::Outside` with HTML", the `=` transition is
that same arm, and the task's midpoint capture
(`t8-mid-red.txt`, preserved with the wave's gate evidence) shows what
stopping at the two previewed lines produces — the strip turning the
bypass into a *deletion* of painted text, the content-mutation class
invariant 6 forbids. Fixing one divergence in the arm while knowingly
leaving a sibling would also have falsified this amendment's own
rationale sentence, and the golden blast-radius window this task opens
(corpus commit, re-bless, reviewed diff) would have had to open twice.

The fix as landed: a bare quote in `Outside` is an ordinary name byte; a
new `has_attr_name` bit distinguishes HTML's *before attribute name*
state (stray `=` starts an attribute named `=`, no value state) from
*attribute name* / *after attribute name* (`=` after a consumed name
opens the value, the ordinary shape, unaffected); and a tag with no `>`
before EOF is pushed as `TagToken::Skip { span: (start, len) }` before
the scan stops — the passed-over region is named instead of silently
dropped. Blast radius, per consumer: `tag_inventory` filters `Skip` by
construction, so the layer-3 ledger is unaffected; `balance_fragment`
now closes what a browser closes on stray-quote input (`<div "> …` gains
the same `</div>` the sanitized DOM has) and is pinned unchanged on the
other two constructions, whose divs close explicitly; `element_extents`
mints the extents the old scanner never saw, and a truncated tag mints
none — a browser abandons it; the strip reaches both planted attributes
and no longer cuts bytes a browser paints.

**The token-stream pin moved, deliberately, twice.** Task 8 first taught
the corpus the blind spot — three `stray-*` entries blessed from the
broken scanner, which emits zero tokens for all three — then fixed the
scanner and re-blessed through the `TRANSYNC_REGEN_GOLDENS=1` hatch with
the diff reviewed: exactly the three new entries moved in
`token-stream.txt`, exactly one balanced golden moved
(`stray-quote-bare`), and the fifteen pre-existing goldens are
byte-identical. The corpus learning a gap and the fix are separate
commits, so the diff between the two blessings is itself the record of
how tokenization changed.

**Two remainder divergences outlive this amendment, filed together
rather than fixed** (ti `e20490`; the owner's ruling comment on
ti 549b20 names both as deliberately unfixed). First, HTML's *tag name*
state consumes `=` and quote bytes into the element name until the first
whitespace, where this scanner ends the name at the first byte outside
`[A-Za-z0-9:-]` — so an element a browser names `divq"x="` can carry a
real attribute the scanner files inside a phantom value. Second, HTML's
*end-tag-open* state makes `</` before a non-letter a parse error that
opens a bogus comment consuming to the first `>`, where this scanner's
name fall-through treats the bytes as plain text and keeps tokenizing
markup a browser never mints.

  **Both measured while landing this task, so the record states them
  rather than defers them.** Parsed raw, `<divq"x=" data-sync-id="v">z`
  yields an element `DIVQ"X="` carrying a **real** `data-sync-id` — a
  live anchor in an unsanitized DOM. Through the vendored DOMPurify the
  whole element is gone (`after_sanitize: "z"`, zero live ids), because
  its name can never match an allowlisted tag. So no live-anchor
  construction through the pane path is known — **and the safety is the
  sanitizer dropping an unknown element**, the same mechanism behind the
  custom-element collision measured in wave 6's pane plan (an anchor
  self-injected into an unknown element dies in the DOMPurify mount),
  here working in our favour. That remainder is therefore safe *only
  while every mount sanitizes*, which panes do, fail-closed by design.
  And in headless Chromium `</1 <div>x` parses to a comment `1 <div`
  plus text `x` — zero elements — while the scanner emits `Open{div}`:
  phantom structure (the R0003-0066 class), not a bypass, fail-safe in
  the strip direction because a plant there is comment interior a
  browser mints nothing from. The ticket states both dependencies
  rather than implying either divergence is harmless in itself.

The section *Two things added, one of them with no caller yet* above
describes `strip_reserved_sync_attrs`' guarantee as it stood at wave-0
landing; this amendment is the record that the guarantee was false for
stray-markup constructions until 2026-08-21, and of what made it true.

### Amendment 2026-08-23 (ti 490d97 wave 1) — the walk aligns with the browser too, and the strip's cut stops welding bytes

*Appended, not a rewrite. Everything above stands as written.*

The 2026-08-21 amendment aligned the **tokenizer** with the browser it was
measured against. Two defects one layer up outlived it, both found by wave 1's
adversarial review and both measured in headless Chromium before anything was
changed.

**The walk modelled the self-closing flag the way XML means it.** The section
*Two things added, one of them with no caller yet* says `9d39e43` "wrote down
what the walk already enforced": that a self-closing tag outside
raw-text/RCDATA is never pushed. That sentence recorded a real invariant of the
code and a wrong reading of HTML. A browser honours the flag in exactly **two**
places — inside foreign content, and on the `<svg>`/`<math>` start tags that
enter it — and everywhere else it is a parse error the parser **ignores**: the
element opens. Measured: `<div/>y</div>` is a div containing `y`;
`<div/ >y</div>` is the same tree, because a space after the slash defeats the
flag too; `<svg><rect/><circle/></svg>` really does make rect and circle empty
siblings; `<svg/>after` really is an empty svg with `after` outside it.

Because `walk_elements` never pushed a flagged non-void tag, the author's own
end tag matched nothing on the stack, became an orphan, and `balance_fragment`
**deleted** it. On the pane path — `render.rs`'s
`balance_fragment(&strip_reserved_sync_attrs(md))`, wave 1's first and only
call site — the fragment then reached the wrapper still open, the wrapper's own
`</div>` closed the fragment instead of the wrapper, and the **next block's
anchor mounted inside the html block's wrapper**: the contracts.md §4a
direct-child violation, which §4's own "a fragment can never consume the
wrapper's own `</div>`" sentence promised could not happen. Measured on a real
`--html-out` bundle on both sides of the fix: before it, `source.html` carried
one `</div>` after the block where it now carries two.

**The defect predates wave 1 entirely.** The strip's residue
(`<div/data-sync-id="…">` → `<div/>`) is merely the first *generated* input
that trips it; a plain author-written `<div/>` — a JSX habit — trips it from
the day the walk existed. That is why the fix is in the walk and not in the
strip: repairing only the residue would have left the authored hole open and
left `element_extents` wrong for every flagged non-void tag, which wave 3's
intake would have inherited.

**The strip's cut lost token separation.** `collect_reserved_attr_spans` takes
each removed attribute's leading whitespace with it, unconditionally. Where the
next surviving byte is a name byte — reachable in malformed markup, after a
quoted value whose closing quote the author misplaced — the deletion removes
the only separator and the following bytes weld onto the tag name. Measured:
`<div data-sync-id="a b="c">x` stripped to `<divc">x`, moving `tag_inventory`
from `["div"]` to `["divc"]` and, in a browser, renaming the element to
`divc"`. (The one-byte difference between `divc` and `divc"` is the tag-name
state remainder this DCR's 2026-08-21 amendment already filed as ti `e20490`,
not a new divergence. Moving off `["div"]` at all is the defect.) The sibling
case: where the byte before the cut is `/` and the byte after is `>`, deleting
**creates** a `/>` adjacency the input never had — it *sets* a flag the
author's tag did not carry, which on a foreign root changes the tree
(`<svg/>y</svg>` is an empty svg with `y` outside; `<svg/ >y</svg>` is an open
svg containing it).

So contracts.md §4's "it never moves the tag inventory, **because attributes
are not structure**" was false until this fix, and is now true **because of the
seam rule**: adjacent cuts are coalesced into one run first — judging per-cut
reads bytes inside a neighbouring cut and turns
`<div/data-sync-id="a" data-order="b">x` into the wrong `<div/>x` — and the run
is replaced by one U+0020 rather than deleted when the next byte is a name
byte, `=` or a quote, or when the previous byte is `/` and the next is `>`.
Nowhere else, so `<div data-sync-id="x">` still strips to `<div>` byte-exact
and every pre-existing strip expectation is unmoved.

**The walk now tracks foreign context**, as a bool per open-stack entry: an
entry is foreign iff it is an `<svg>`/`<math>` root or its parent entry was,
and the bit is read after the implied-close pops. It is deliberately **not**
`scan_tags`' `foreign_depth`, which is a saturating counter with no stack
scoping, adequate only for choosing a CDATA terminator. Without the bit,
pushing flagged tags would have nested `circle` inside `rect` and appended a
phantom `</svg>` after `<svg/>`.

**What it deliberately does not model**, each measured here and each left as
filed follow-up rather than silently accepted:

- **HTML integration points and breakout tags.** `<svg><foreignObject><div/>x`
  leaves that div **open** in a browser; our model treats it self-closing.
  Divergent before and after this amendment, in the same direction, and
  **untouched by it** — the balancer's output for these inputs is byte-identical
  old-versus-new.

  *Corrected 2026-08-23, hours after this bullet was written, by the wave's own
  review.* The bullet first read "bytes stay safe, extents diverge". **"Bytes
  stay safe" is false**, and it is the same overclaim this amendment was written
  to retire, made one paragraph after retiring it. Measured through the shipped
  DOMPurify mount: `<div class="wrap"><svg><div/>x</svg></div>` followed by a
  block mounts that block's anchor with parent `DIV.wrap`, not `<main>` —
  `contracts.md` §4a's direct-child rule, broken. The sharper form is that the
  self-closing flag is **not the mechanism at all**: the unflagged
  `<svg><div>x</svg>` does the same thing, because `div` is on HTML's
  foreign-content *breakout* list, so a browser pops the `<svg>` at the `<div>`
  and leaves the div open in HTML content, while our walk closes it at
  `</svg>` and passes the fragment through. `element_extents` having no in-tree
  callers is the wrong consumer to reason from: the same `walk_elements` feeds
  `balance_fragment`, whose caller is the shipped pane path. Reachable from
  untrusted source Markdown through a type-6 html block. Still a follow-up
  rather than this wave's work — it is pre-existing and orthogonal to the slash
  — but it is a **live §4a break**, not a benign extent divergence, and ticket
  `e77173` is re-ranked accordingly.

  *Closed 2026-09-01 by ticket `e77173`, recorded in **DCR-0041**.* Both rules
  are modelled now: the walk carries a three-valued content mode per open
  element instead of one inherited `in_foreign` bool, so `foreignObject` and
  `desc` return their children to HTML content and the breakout set pops
  foreign elements at the tag that breaks out. The §4a scenario above is a
  regression test in **both** spellings, flagged and unflagged, because this
  bullet's own correction is what established that the slash is not the
  mechanism. SVG `<title>` was still not claimed at that point, because the
  tokenizer ate its interior; the raw-text bullet below is what closed that.
- **Foreign raw-text/RCDATA.** `scan_tags` enters raw-text state for
  `script`/`style`/`textarea`/`title` even inside svg/math, where a browser
  does not: `<svg><title>a<b>c</b></title>` mints a real `b` element, and
  `<svg><script/>x` self-closes the script. Scanner-level and pre-existing
  (R0002-0020 made the entry unconditional); untouched here.

  *Closed 2026-09-01 by ticket `2e2453`, recorded in **DCR-0042**.* The state
  is entered only in HTML content now, so both measured cases match the
  browser, SVG `<title>` joins `is_svg_html_integration_point`, and the
  content-mode stack moved from `walk_elements` into `scan_tags` so the two
  layers read one verdict rather than two. R0002-0020 is refined, not
  reverted: its subject was RCDATA text in HTML content throughout.
- **Void names used as real foreign elements.** `is_void` wins globally, so a
  genuine `<svg><link>…</link>`'s closer is still dropped as an orphan. The
  trade is deliberate: an appended `</br>` would be turned back into a fresh
  `<br>` by HTML's end-tag-`br` rule, so the balancer would *mint* structure
  instead of repairing it — worse than dropping a closer.

**`VOID_ELEMENTS` gained four names** — `basefont`, `bgsound`, `frame`,
`keygen` — each measured never-open in Chromium (`<keygen>x` leaves `x` a
sibling). They are elements the spec dropped while the tree-construction rule
that makes them void survived; `param`, already on the list, is the same
category, and the earlier claim that the current syntax list has only thirteen
entries is why it was there. Without the four, the new push rule would have
newly appended junk closes for their flagged spellings. `image` is deliberately
excluded: `svg:image` is a real, closable foreign element, and calling it void
would delete an author's `</image>` closers.

**The pin moved, deliberately, twice — and this time it was a WALK change.**
The corpus commit added five `selfclose-*` entries blessed from the broken
behaviour (`selfclose-div` losing its final `</div>`, `selfclose-span` and
`selfclose-p` gaining no appended close, `selfclose-svg` and
`selfclose-svg-root` passing through as no-regression guards over the two real
carve-outs); the fix commit re-blessed through the same `TRANSYNC_REGEN_GOLDENS=1`
hatch. Exactly **three** balanced goldens moved between the blessings —
`selfclose-div`, `selfclose-span`, `selfclose-p`, all three added by the corpus
commit — and **`token-stream.txt` did not move at all**, because `scan_tags` is
untouched. That is the record that this was a walk change: a green token-stream
pin proved nothing here, which is why the never-re-bless doctrine was widened
from "a deliberate, reviewed **tokenizer** change" to "tokenizer **or walk**"
in the pin's module doc, its regeneration hatch doc and its token-stream
assertion, and why the `balance_fragment` assertion — the message a walk change
reaches **first** — gained the doctrine sentence it had never carried.

The corpus could not have caught either defect before: it held **zero**
`:true` tokens, i.e. not one self-closing spelling of any tag.

## Amendment (2026-08-24) — the wave's date is a range, and this record's `Date:` is its first day

*Appended, not a rewrite. The `**Date:** 2026-08-20.` line above stands as
written; this says what it means.*

Wave 0 did not land on 2026-08-20. `git log --date=short` over its own commits:

| commit | date | what |
|---|---|---|
| `1d6f19d` | 2026-08-20 | Task 1 — the crate move |
| `796a97b` | 2026-08-21 | Task 7 — this record |
| `d2dac83` | 2026-08-21 | Task 8 added to the plan |
| `37aa0ee` | 2026-08-22 | Task 8's fix |
| `1299280` | 2026-08-22 | the wave's close |

So the wave ran **2026-08-20 → 2026-08-22**, and the single date above is its
first day, inherited from the plan's filename. `status.md` and
`phase-state.yaml` were corrected in place on 2026-08-24 to carry the range;
this record keeps its original line because a dated record's value is that it
says what was believed when it was written.

The rule, now recorded three times in this repository and worth stating once
more plainly: **a date belongs to the work, not to the document that describes
it**, and `git log --date=short` is the check. DCR-0033 line 29 states it for
wave 1; the wave 2, 3, 5 and 7 plans carry execution-date placeholders since
2026-08-23 for the same reason.

