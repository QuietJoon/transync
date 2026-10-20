---
type: ADR
title: The source-language gate is a pre-flight question, not a second detection
description: transync-lang answers "should a translation run start?" for a caller before any run; the alignment map's detected_source_language answers "what language was this, per the model that translated it". They must never converge, and the gate's type is built so it cannot.
tags: [decision, ADR-0028]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-02T00:00:00Z
status: stable
---

# ADR: The source-language gate is a pre-flight question, not a second detection

## Context and Problem Statement

A consumer (`resp-translator`) captures an agent's final answer and wants to
skip translation when the agent already replied in the target language. It
cannot ask transync that today without running the whole pipeline and paying
for a provider call (ti `eb1d89`).

The owner ruled that a language-detection dependency must not enter the facade,
but that a separate workspace member — the shape `transync-openai` and
`transync-anthropic` already prove — is right. That settles *where the code
lives*. It does not settle the harder question, which is what the new code is
**allowed to mean**.

Because transync already answers a question that sounds identical. The
alignment map carries `detected_source_language`, authored by the **provider**
under `--source-language auto`, admitted through one door in the pipeline and
cached per document. It reaches the wire, the cache, and the rendered page's
`lang` attribute.

Two answers to one question that can disagree is the defect this repository
spent 2026-09-01 removing, four times over: two opinions about where a tag name
ends (ti `415cdb`, ti `e20490`), two about where foreign content begins
(ti `2e2453`), two about which blocks anchor (ti `18b9c3`). Each was cheap to
introduce and expensive to find, and each was found only because a consequence
went wrong somewhere else. A fifth instance, one layer up and on the wire,
would be worse than the four: `detected_source_language` is a published field,
so a local guess leaking into it would be a contract violation and not merely
an inconsistency.

## Decision Drivers

* The two questions genuinely are different, and the difference is what makes
  the new crate legitimate rather than redundant.
* Prose separation is not enforcement. The four 2026-09-01 defects each had a
  comment explaining the intended arrangement.
* A gate's expensive failure is asymmetric: skipping a translation that was
  needed emits the wrong language, while running one that was not costs money.
  The type should make the cheap failure the easy one.

## Considered Options

1. **One language answer, shared.** Let the pipeline consult the local detector
   for `--source-language auto` and let the consumer read the result.
2. **Two answers, separated by documentation.** Write down that they are
   different questions and rely on review.
3. **Two answers, separated by construction.** Make the gate structurally
   incapable of producing the pipeline's answer.

## Decision Outcome

**Option 3.** The gate answers a different question, for a different party, at
a different time — and three properties of the code enforce that rather than
asking for it:

| | |
|---|---|
| `transync-lang` | *"Should a run start at all?"* — pre-flight, caller-side, before any provider exists |
| `detected_source_language` | *"What language was this, per the model that translated it"* — pipeline, on the wire |

1. **The verdict carries no language name.** `Verdict` is
   `AlreadyTarget(Basis) | Translate(Reason)`; neither variant holds a
   `Language`. There is nothing in it to copy into the alignment map.
2. **It derives no serialization.** No `Serialize`, so it cannot reach a wire
   format even by accident.
3. **The crate depends on no workspace member.** No engine crate can call it
   without someone adding an edge on purpose, in a diff a reviewer sees.

Option 1 was rejected because the pipeline's answer is *evidence from the model
that did the work* — an observation about text actually translated — while the
gate's is a guess about text nobody has looked at yet. Substituting one for the
other would degrade a published field from evidence to estimate. Option 2 was
rejected on this repository's own record: it is exactly what the four
2026-09-01 defects had.

**A corollary the type also encodes.** `Reason::Undecided` lives *inside*
`Translate`, not in a third variant. The only way to skip a translation is to
name `AlreadyTarget` explicitly, so "I could not tell" cannot be misread as "no
translation needed". A flat three-variant enum would put that mistake one
careless match arm away.

**The input domain is the modelled languages** (DCR-0063). Both halves of the
gate are closed-set: the script test knows the scripts `Language`'s sixteen
write, and the backend can name only those sixteen. A verdict is meaningful
for text in one of them; text in another language can come back
`AlreadyTarget`, which is the direction that cancels a run. Cyrillic, the
Arabic script and Devanagari are exclusive only within that set, so for a
Russian, Arabic or Hindi target any letter the target's orthography never uses
(Ukrainian `ї`, Persian `ی`, Marathi `ळ`) answers `Translate` with
`Reason::ForeignOrthography`, before both the script shortcut and the backend
hand-off. The screen narrows the exposure without closing it: Bulgarian reads
as Russian, and Nepali, Sanskrit and most Marathi as Hindi. On every
target the backend decides — each Latin one and Chinese — a language it does
not model can read as its nearest modelled neighbour (Catalan as Spanish,
Norwegian as Swedish, Cantonese as Chinese). Hangul and Kana are
near-exclusive, not exclusive: Jeju, Ainu and the Ryukyuan languages read as
Korean or Japanese. The crate doc and `Gate::verdict` carry the same statement.

## Consequences

* Good, because the separation survives people who have not read this record —
  it is checked by the compiler and by `containment`, not by memory.
  `containment` parses every module under `src/` and holds `whichlang` to
  function bodies in `backend.rs` (R0012-0021, DCR-0063).
* Bad, because a caller whose input can fall outside the modelled languages
  cannot treat `AlreadyTarget` as proof. The gate cannot tell it what language
  its input is in; only the domain statement above warns it.
* Good, because the gate can be published, versioned and reasoned about without
  touching the pipeline's contract at all.
* Bad, because a caller that wants *both* answers must call two things and
  understand why. That cost is deliberate: collapsing them is the failure this
  record exists to prevent.
* Neutral, because the gate is unusable by the CLI today. Nothing in the
  workspace depends on `transync-lang`, so it ships as a library for consumers
  and the roster's dependency-order publication puts it first.

## Notes

Amends **ADR-0003** (Cargo workspace with provider crates): the member roster
gains a ninth crate, and the first that is neither an engine layer nor a
provider. The rule ADR-0003 states — explicit member list, no globs, one
workspace — is unchanged and is what made the addition mechanical.

The backend behind the gate was chosen by measurement rather than reputation;
`benchmark/lang-detect/RESULTS.md` records the comparison, including that a
dependency-free codepoint test ties the winner on real fixtures, which is why
the library runs second rather than first.

TRACE: ti e4f4b0
TRACE: DCR-0045
