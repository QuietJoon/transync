---
type: DCR
title: HTML→HTML translation works at the library surface, and the records say what deliberately did not move
description: Wave 5 of the HTML→HTML feature adds TranslateOptions.input_format, [system].prompt_html, the run-level HTML-document instruction clause, and extract-projected heading context — so translate() on an HTML document returns translated HTML through the wave-4 gate. No new cache axis, no schema bump, no edit to the gate, and Markdown runs byte-identical throughout.
tags: [change, project-control, DCR-0037]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-03T00:00:00Z
status: stable
---

# DCR-0037: HTML→HTML translation works at the library surface, and the records say what deliberately did not move

- **Date:** 2026-09-03 — this record, and the code, which landed the same day
  across eight commits: `0fa8a08` (the §6 unit-layer pins through the real
  intake), `ea7f41e` (the context projection), `2a93b4a` (`[system].prompt_html`),
  `7417130` (the run-level clause and the cache separation), `a2ff4d8` (the
  entry point and the four dispatches), `6c3c9a9` (`SourceFormat` crosses the
  facade), `a6125c0` (the dominance re-text), `b0c5c72` (SCN-16). Read from
  `git log --date=short`, **not** from the plan's filename: the plan is dated
  2026-08-20, its writing date — the rule DCR-0033 wrote down.
- **Source:** `docs/superpowers/specs/2026-08-20-html-to-html-translation-design.md`
  (ratified design spec), ticket `490d97`, **wave 5** of the eight in its §12.
  §6 is this wave in full. Plan:
  `docs/superpowers/plans/2026-08-20-html-wave5-units-context-prompt.md`.
- **Affected ADRs:** `docs/decisions/0025-html-to-html-document-translation.md`
  (D3, D4, D5, D8's library half, and the prompt/cache decisions as landed).
- **Affected DCRs:** `DCR-0032` (`extract`, consumed unmodified);
  `DCR-0034` (`Spelling` / `SourceFormat`, the axes this wave dispatches on);
  `DCR-0035` (the intake, called at one new site); `DCR-0036` (the gate, whose
  Hard-arm hand-forward this record discharges); `DCR-0027` (the cohort
  re-compile, which `prompt_html` selection must precede); `DCR-0026` (whose
  fourth instruction clause set the pattern the fifth follows).
- **Numbering note.** This record takes **DCR-0037** because wave 5 reserved
  it. `DCR-0038`–`DCR-0039` remain reserved for waves 6 and 7.

## What changed

- **`TranslateOptions.input_format: SourceFormat`** — additive on a
  `#[non_exhaustive]` struct, defaulting to `Markdown`. D8's library half:
  routing is **explicit**, the library never sniffs, and there is no reverse
  sniff on HTML runs because a body fragment is legitimately accepted HTML.
  `transync::SourceFormat` is exported with its §0 row in the same commit.
- **Four exhaustive format dispatches in `run_pipeline`**, which are the whole
  pipeline change: the intake branch (`transync_syntax::intake::html::parse`,
  named directly at the one call site); dominance suppression (the Html arm
  never evaluates the predicate); render suppression (empty pane fields until
  wave 6); and the boundary template scan, extended per format.
- **`heading_plain_text` branches on spelling** through `transync_html::extract`
  — one function, and because it is the function `build_index`'s headings map
  is built from, the map, `section_path` and `document_title` cannot hold
  divergent projections.
- **`[system].prompt_html`** — an additive profile key with a shipped default
  body, selected by `select_prompt_for_format` at `unit::build_batches`' profile
  door **before the first compile**, so the initial body, every DCR-0027 cohort
  re-compile and the template rewind all inherit it.
- **`InstructionVariant.html_document`** — the run-level clause, derived in one
  place (`DocumentFacts::of`, off the `block_type == 0` sentinel) and welded to
  the input format by a `build_batches` `debug_assert_eq!`.
- **The `html_dominance_warning` tail**, re-texted in library vocabulary.
- **`Mode::AppendsExtraSegment` and the SCN-16 scenario** — three legs:
  identity, invariant 6 through a real rejection, and cache disjointness.

## Why

Wave 5 is the feature working: `translate()` on an HTML document returns
translated HTML — **through** the gate wave 4 built, never around it.

## What deliberately did NOT change, which is the §6 evidence

This is the record's spine, because on a wave this size the non-changes are
the load-bearing claims:

- **No cache axis.** `CacheKey`'s field set is untouched. contracts §5a now
  carries the verdict: the document's format reaches the model only through
  prompt bytes, and both routes are already axes — `profile_prompt_hash` moves
  (an HTML run compiles `prompt_html`) and `instruction_hash` moves (the
  run-level clause), either alone separating the identities. Adding the axis
  would buy a distinction the key already makes at the price of a red compile
  in an out-of-crate exhaustive destructure plus a breaking change to a §0
  tier-(a) field set.
- **No `VALIDATION_SCHEMA_VERSION` bump.** The bump rules' own words apply:
  *"Do NOT bump for validator tightening alone — cache hits are re-validated
  through `validate_batch` on every run (ADR-0015), so per-kind/schema
  tightening already rejects stale entries."* The `html_segments` payload
  semantics are unchanged, and the shipped `pre_bump_validation_schema_version_never_replays`
  pin holds the constant at 2.
- **No alignment schema bump** — still 1.2.0; row `source_format` and map
  `input_format` are wave 6's 1.3.0.
- **No edit anywhere under `validate/` or in `pipeline/finalize.rs`.** The
  wave-4 gate is consumed, never re-implemented, and the acceptance
  containment diff proves it rather than asserting it.
- **Markdown prompts, instructions and goldens byte-identical.** The three
  `golden_*` tests pass against the **unregenerated** files, and `git status`
  over the golden directory prints nothing. `TRANSYNC_REGEN_GOLDENS` was never
  set: on this wave a red golden is a finding, never a regeneration errand.
- **Neighbor snippets stay verbatim source excerpts for both formats.**
  Projecting them through `extract` was proposed and rejected (§6 final
  confirmation ii); the HTML-side test is a tombstone so nobody re-proposes it.
- **Nothing outside `crates/transync-core`, `crates/transync` and the named
  docs.** No CLI file, no `web/` file, no manifest, no `Cargo.lock`.

## The judgement calls, each with its ground

**1. The `html_document` flag is derived from the sentinel, not passed in.**
Spec §6 says the flag is "set run-level from the input format". The run-level
*value* is exactly that — but the derivation site cannot be a parameter,
because two consumers have frozen signatures: `build_user_prompt(batch)` is §0
tier-(b) surface a provider calls with nothing but the batch, and
`InstructionDigest::for_batch(batch)` must hash the very instruction that call
assembles. `TranslationBatch` is an exhaustive §0 struct, so it cannot carry a
new field without a breaking change this feature's sanctioned set does not
include. The one honest carrier a bare batch holds is the sentinel §6 itself
defines — "0 = a block of an HTML document, where no CommonMark type applies"
— a documented **record** of precisely this fact. §6's "record, not a switch"
sentence is scoped to the *splice policy*, which this does not touch.

Three welds keep it honest: the derivation lives in exactly one expression;
`build_batches` `debug_assert_eq!`s the derived value against `doc.format`
(debug builds only — the plan says so rather than selling the assert as a
release-time check); and a pin proves an island unit never raises it.

**2. An HTML run's two annotated-pane fields are empty until wave 6.** The
alternative — letting `render_source`/`render_target` run — would put comrak
and `walk` over an HTML document: the render-path twin of the exact hazard the
layer-6 dispatch exists to prevent. Empty is the honest value, documented on
the fields and pinned by test.

**3. The `prompt_html`-absence advisory is emitted at the selection door, not
recorded by the loader.** The loader cannot know the run's format, and a
warning printed to a Markdown-only operator about an HTML-only gap is a
warning everybody learns to skip. The loader's contribution is the part that
is file-truth whatever the run: parsing the key, warning on unknown `[system]`
keys, and *recording* the new body's placeholder scan — `prompt`'s own ed8c57
shape. Emission belongs to the boundary, which also closes a door the loader
can never see: a caller-built profile, handed to `translate()` without ever
being loaded, whose `prompt_html` typo would otherwise compile unmentioned.

**4. The Hard-arm prefix is re-pinned, not re-worded.** `"full reparse failed:
{reason}"` now prefixes the twin's reasons too, and "reparse" is loose
vocabulary for a scanner rescan. Re-wording would edit an operator-visible
error plus a shipped out-of-crate expectation — priced in exactly the currency
a wave whose Markdown-side claim is *byte-identical, zero expectation edits*
must not spend. The prefix is the historic name of the document-level layer-6
gate for both formats; the honest vocabulary is the twin's reason after the
colon, and `an_html_hard_failure_speaks_the_twins_vocabulary` pins the
combined form.

## Two inter-plan reconciliations, resolved toward the spec

- **Wave 2's plan mis-assigned the dominance re-text to wave 6** ("its
  suppression for declared-HTML runs and its re-texted tail are wave 6's,
  landing with the entry point"). The entry point lands in *this* wave, and
  spec §12 assigns wave 5 "dominance-warning suppression + re-text". The spec
  is the binding authority (`BL-2026-07-B`), and the aside's own premise —
  "with the entry point" — is honored: the two commits are back to back, and
  the interval where the entry point is live while the tail still said "not
  implemented" is named in the re-text commit rather than left as an accident.
- **Wave 2's deviation 2 deferred `SourceFormat`'s §0 row to wave 6.** The
  option that makes the type nameable lands here, so the row lands here —
  under exactly the rule wave 2 cited (a row lands with the export).
  `AlignmentMap.input_format` stays wave 6's.

## Discharged hand-forwards

- **DCR-0036's Hard-arm item** — re-pinned by decision, recorded above, with a
  live test of the combined form.
- **Wave 3's "wave 5 decides how core reaches the intake"** — decided: named
  directly at the one new call site, exactly as `full_rescan_html` already
  names it. No `pub(crate) use`, no facade export, no new path for
  `public_surface.rs` to police.
- **Wave 2's `a_real_title_element_projects_to_empty_prose_until_wave_5`** —
  retired as its own comment instructed; the expectation is now the projected
  text and the test's name went with it.

## One verification note

DCR-0036's hand-forward paragraph names the real out-of-crate pin,
`boundary_v02::hard_failure_maps_error_and_evicts_implicated_keys`, and that
was confirmed in the tree rather than taken from prose: the test exists under
that name, `finalize.rs`'s comment names it, the prefix lives in exactly one
source string (`pipeline.rs`'s Hard arm), and exactly one test pins it. The
stale name `hard_policy_maps_error_and_evicts` survives only in the two plan
files' own prose, where it is self-describing history. Nothing was handed back.

## One spec erratum, owed as a §14 review item

Spec §6's context-projection bullet says the pre-wave comrak projection "would
put raw tags into the heading stacks and `context_hash`". **The true mechanism
is the opposite failure, and the wave's own red observed it:** comrak sees one
leaf `HtmlBlock` with no inline children, so an Html-spelled heading projected
to **empty prose** — `left: [(1, "")]`, not `[(1, "<h1 …>")]`. The hazard is
collapse and degraded context identity (distinct headings deduping to the same
empty entry, the section story going silent), not markup leakage. The
extract-projection decision is unaffected — it fixes the real defect too — and
the shipped source carries the true mechanism. **Amend §6's sentence.** The
plan predicted this exact red and instructed the implementer to stop if raw
tags appeared instead; they did not.

## One finding, filed rather than fixed

The plan built its layer-6 routing proof on a translator returning `[" "]`,
asserting every per-unit layer passes it. **It does not:** `per_kind` rejects
any segment whose chars are `all char::is_whitespace`, a check written for
exactly that attack. Defense in depth, working — but the lever that *does*
reach layer 6 exposes a gap worth naming: **U+FEFF is not
`char::is_whitespace` in Rust while rule T explicitly strips it**, so a
segment of only U+FEFF passes every per-unit layer and dissolves the block.

On an HTML-document run the layer-6 twin catches it, which is what makes it
the honest lever for the routing proof. On a **raw-HTML island in a Markdown
run** layer 6 is `reparse_full`, which sees one `HtmlBlock` with an unchanged
label and cannot see it — a reachable content-erasure path with no defense.
Filed as ticket `c887bc` with three options and the recommendation, and
deliberately not fixed here: this wave adds no validator, and the predicate's
exact membership (legitimate zero-width-joiner-only text exists) is an owner
decision.

## Evidence

The gate files under `/Volumes/Temp/claude/ti490d97-wave5/gate/`:
`t2-pins.txt` (4 green on arrival — the §6 invariant chain survives the trip
from real intake to real batcher); `t3-red.txt` (the projection red on values
— empty collapse, not raw tags); `t4-red.txt` (the selection reds —
behavioural; the module compiles clean); `t5-red-clause.txt` /
`t5-red-key.txt` (the wiring reds); `t5-goldens.txt` (goldens green against
unregenerated files, empty `git status` over the golden dir);
`t6-red-compile.txt` (`E0560` on `input_format`); `t6-boundary.txt`
(`boundary_v02` 7 green, unedited); `t8-red.txt` (the tail's lie printed in
the failure); `t9-scenarios.txt` (30 green — SCN-16 beside SCN-01..15 with
zero edits); plus the wave's acceptance captures.

## Hand-forwards to wave 6

The pane derivation fills the two empty fields; alignment schema 1.3.0 (row
`source_format`, map `input_format`); the CLI's `--input-format`, the
preamble-sniff refusal rewrite (including its own `(ti 490d97)` clause and its
`cli_smoke.rs` pins), `--allow-html-input`'s narrowing and the exit-1
conflict; `out.html` / `Destinations.document`; the bundle-title chain. The
scenario-matrix SCN-16 row and the browser gate are wave 7's.

Noted for completeness: the document-scoped cache records (`DocumentMetaKey`,
`GlossaryExtractionKey`) deliberately stay **format-blind** on §5a's own
narrowing logic — a detection is an observation about the document's bytes and
an extraction request spells no format, so two formats asking one question
honestly share one answer. No new axis there either.
