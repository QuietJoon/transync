---
type: DCR
title: transync-lang joins the workspace as the source-language gate, backed by a measurement that reversed its own first recommendation
description: A ninth member answering whether a translation run should start at all — script test first, whichlang second, depending on no workspace member so it cannot become the pipeline's provider-authored language answer.
tags: [change, project-control, DCR-0045]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-02T00:00:00Z
status: stable
---

# DCR-0045: `transync-lang`, the source-language gate

- **Date:** 2026-09-02
- **Source:** ticket `e4f4b0`, following the owner's ruling on ti `eb1d89`
- **Affected ADRs:** **ADR-0028** (new — the principle this member introduces), `ADR-0003` (amended — a ninth member, the first that is neither engine layer nor provider)
- **Evidence:** `benchmark/lang-detect/RESULTS.md`

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This is the next free number after DCR-0044.

## What changed

A ninth workspace member, `crates/transync-lang`, holding one question:
**should a translation run start at all?** A consumer captures an agent's final
answer and skips the provider call when it is already in the target language.

Three public items and no error type: a `#[non_exhaustive] Language` naming the
sixteen languages the backend recognizes, a two-variant `Verdict`, and a `Gate`
built once around a target.

## Why a DCR *and* an ADR

DCR-0029 added `transync-anthropic` with a DCR alone, and by its own stated
test that was right: a second provider introduced no new principle, only a
second instance of ADR-0002's shape. This member does introduce one — that the
gate is a *pre-flight question and not a second detection* — so the principle
is **ADR-0028** and this record is the change that carries it.

## The measurement, and that it overturned its own author

`benchmark/lang-detect/` is committed with the crate, reproducible, and it took
two rounds:

- **Round 1**, on authored Korean prose, scored every candidate 15/15. Hangul
  against Latin is trivially separable, so the benchmark had no discriminating
  power and decided nothing. Reporting "all equivalent, take the cheapest"
  would have been wrong.
- **Round 2**, on `resp-translator`'s real agent-response fixtures, separated
  them — and reversed the recommendation already given. `lingua` had been
  proposed on its documented strength with short text; against real input it
  flips **earliest** of the three, at 30–40 % English dilution. That is the
  expensive direction: calling a Korean answer non-Korean costs a provider call
  *and* emits ko→ko.

| | real fixtures | flip point | throughput | licence |
|---|---|---|---|---|
| **`whichlang`** | 14/14 | 70 % en | 0.042 ms | MIT |
| `script` (no dependency) | 14/14 | 60 % en | 0.004 ms | — |
| `lingua` (ko+en) | 14/14 | 30–40 % en | 0.179 ms | Apache-2.0 |
| `whatlang` | 10/14 | unusable | 0.008 ms | MIT |

**The uncomfortable part is recorded rather than smoothed:** a Hangul codepoint
test with no dependency ties the winner and is ten times faster. `whichlang`
earns its place on two things the control cannot do — it names Japanese and
Chinese where the control abstains, and it generalises past the two scripts
this gate needs today. So the composition is **script first, library second**,
and the library only sees text counting could not settle.

## What the design settled

**Voidness of the shortcut.** The script test only runs where a script is
exclusive *within the backend's language set*: Korean, Japanese (kana only —
Han is shared with Chinese and cannot discriminate), Russian, Arabic, Hindi.
Every Latin target, and Chinese, goes straight to the backend rather than
counting a script that cannot answer. Only Korean's threshold is measured; the
other four take the same rule by construction and the code says so.

**The threshold is a constant, not a knob.** One letter in five. Across the
measured sweep every multiplier from 1 to 9 gives identical verdicts, so a
caller-tunable ratio would have no reachable effect except to contradict the
evidence.

**Semver containment.** `whichlang` is 0.1.x, where every release may break. It
is named in exactly one file, the mapping to our own `Language` is
wildcard-free — so a new backend variant is a compile error here rather than a
silent "not the target" — and `containment` proves the containment by reading
the crate's own sources.

## Welds that moved

Five welded artifacts go red until edited, and all five were: the root
`members` list and its publication-order comment (seven → eight packages);
`PUBLISHED_MEMBERS` (the new member is **first** — it has no internal edge);
`RUSTDOC_GATE_CRATES`; `docs_ownership_drift.rs`'s `CRATE_ROOTS` with the
source-of-truth table naming both new modules; and `docs/index.md` for these
records.

Two welds were **added**: `public_surface.rs`'s forbidden list gains
`transync_lang`, so the no-facade-dependency ruling fails loudly if code ever
reverses it; and the `containment` scrape above.

One entry was deliberately **not** added: a `[workspace.dependencies]
transync-lang` row. Nothing depends on the crate, so the edge test would fail
it as declared-but-unused — the `transync-anthropic` precedent exactly. It
lands the day a member takes the edge.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 43/43 binaries,
  **1170 passed, 0 failed**, `CARGO_EXIT=0` (1150 before, plus twenty).
- The `containment` weld was **verified to fire**: a `whichlang::Lang` planted
  in `script.rs` fails it, and removing it passes. Its first draft fired on the
  crate's own prose, so it now reads what the compiler reads — comment lines
  stripped, and the checker exempt by name rather than by a cleverly-spelled
  needle.
- `tests/gate_behaviour.rs` pins the composed gate against
  `benchmark/lang-detect/corpus/` **read in place**. Copying it would be two
  places holding one corpus. `corpus-real/` is deliberately unread: it is
  gitignored, so a test depending on it would pass or skip by who ran it.

## What this does not do

- **No consumer can use it yet.** `resp-translator` pins transync by
  `tag = "v0.4.0"`, so the crate is invisible to it until v0.5.0 is tagged —
  and v0.5.0 requires every registered task resolved, `e4f4b0` now among them.
- **Romanized Korean is not handled.** Every candidate failed it, and the gate
  fails toward translating rather than claiming it.
- **It is not wired into the CLI.** No `--skip-if-source-is-target` flag exists;
  that would be its own decision, and would bring the first internal edge.
