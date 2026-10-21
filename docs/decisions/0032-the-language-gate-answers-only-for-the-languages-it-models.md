---
type: ADR
title: The language gate answers only for the languages it models
description: transync-lang's script shortcut keeps Cyrillic, the Arabic script and Devanagari, but one letter the Russian, Arabic or Hindi orthography never writes answers Translate before either half of the gate decides; and a verdict is declared meaningful only for text in the sixteen modelled languages, with the misses that remain named rather than removed.
tags: [decision, ADR-0032]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-21T00:00:00Z
status: stable
---

# ADR: The language gate answers only for the languages it models

## Context and Problem Statement

Found in Review 0012 (Issue R0012-0014, Severity: MEDIUM).
Location: `crates/transync-lang/src/script.rs` (`ExclusiveScript`, `read`) and
`crates/transync-lang/src/lib.rs` (`Gate::verdict`).

The gate's free half treats a script as proof of a language when no other
language the backend models writes it. That is why Russian, Arabic and Hindi
have a script shortcut: within the backend's sixteen languages, Cyrillic, the
Arabic script and Devanagari each belong to one. Nothing confines the gate's
*input* to those sixteen. Ukrainian, Persian or Marathi text sent to a gate built
for Russian, Arabic or Hindi carried at least a fifth of its letters in the
target's script and came back `AlreadyTarget(ExclusiveScript)`. Diluted below a
fifth, it went to `whichlang` 0.1.1, which outside ASCII sees only a histogram
of 128-codepoint block buckets and can name only its sixteen languages, and it
came back `AlreadyTarget(Backend)`.

`AlreadyTarget` is the one answer that cancels a run: the caller ships the text
untranslated and nothing reports it. A live consumer reaches it —
`resp-translator`'s `source_gate::gate_for` builds a gate from any profile's
target, pinned to the v0.5.0 tag. The same closed-set exposure exists on the
backend path for every Latin and Han target, where a language the backend does
not model reads as its nearest modelled neighbour.

## Decision Drivers

* ADR-0028's asymmetry: skipping a needed translation emits the wrong language,
  while running an unneeded one costs a call. The type already makes the cheap
  failure the easy one; the fix must not reopen the expensive one.
* Neither half of the gate can see the difference on its own. The count treats
  every letter in a script block alike, and the backend's features for these
  scripts are block buckets. Sending more text to the backend is not a fix.
* The shortcut earns its place. The crate's own measurement
  (`benchmark/lang-detect/RESULTS.md`) is why it runs first, and a false
  `Translate` is what the crate calls the expensive direction: a provider call
  that emits same-language output.
* DCR-0045 chose the backend by measurement and is a write-once record.
  Replacing the backend reopens that choice, and it is the only change that
  removes the closed-set exposure on Latin and Han targets.
* The domain statement is owed whatever happens to ru/ar/hi: the crate doc and
  `Gate::verdict` named no input domain, and the Latin and Han exposure survives
  every option but the last.

## Considered Options

These are the options put to the owner.

A. **Never skip for ru, ar or hi.** Drop Cyrillic, the Arabic script and
   Devanagari from the shortcut and add a never-skip branch covering both the
   dominant and the minority reading, since the backend would answer the same.
   Fully safe in ADR-0028's terms, but it disables the gate for three targets:
   every answer already in Russian, Arabic or Hindi pays a call.
B. **Keep the shortcut and screen letters the target never writes.** Any letter
   of the target's script outside the target's orthography is positive evidence
   of another language and answers `Translate`, ahead of both the shortcut and
   the backend hand-off. It catches every orthography that has such a letter in
   the text and none that does not.
C. **Document the input domain only.** State on the crate doc, `Gate::verdict`
   and ADR-0028 that a verdict is meaningful only for text in the modelled
   languages, and change no behaviour; callers carry the risk.
D. **Re-benchmark against an open-set backend** for the shared-script targets,
   and possibly the Latin ones. The largest option; it revisits DCR-0045's
   measured choice.

Also raised, as a refinement of B: require a positive marker for Russian (`ы`,
`э`), which would also catch Bulgarian at a recall cost on short Russian text.

## Decision Outcome

ACCEPT: option B for behaviour plus this record for the domain.

B removes the defect for the large class it was found in — every orthography
that writes a letter the target never uses — without giving up the shortcut for
genuine Russian, Arabic and Hindi, which keep both the free script test and the
backend band. C is the floor under B rather than an alternative to it: the
misses B cannot reach, and the Latin and Han exposure no screen touches, are
stated where a caller reads the API, in the crate doc, in `Gate::verdict`'s
`# Input domain` section and in ADR-0028's decision body, and recorded here
with the reasons. A was rejected because it trades a narrow skip defect for
switching the gate off on three targets. D stays open as a possible later
decision; it is the only route to removing the closed-set exposure. The
positive-marker refinement was not adopted.

Status: Implemented (`a0de551`).

### Implementation

- `crates/transync-lang/src/script.rs` — `ExclusiveScript::foreign(c)`, asked
  only of a letter `contains` has placed in the target's script.
  - Cyrillic (U+0400–U+052F): every letter outside the 33-letter Russian
    alphabet (U+0410–U+044F, U+0401, U+0451).
  - Arabic (U+0600–U+06FF, U+0750–U+077F): U+0620, U+063B–U+063F,
    U+0672–U+06D3, U+06D5, U+06EE–U+06EF, U+06FA–U+06FC, U+06FF, and all of
    Arabic Supplement.
  - Devanagari (U+0900–U+097F): U+0904, U+090C, U+090E, U+0912, U+0929, U+0931,
    U+0933 (ळ), U+0934, U+0960, U+0961 and U+0972–U+097F.
  - Hangul and Kana: none.

  Each set is a superset of the letters put to the owner. `read` returns the new
  `Reading::Foreign` on the first such letter, before the ratio is looked at.
- `crates/transync-lang/src/lib.rs` — `Gate::verdict` maps `Reading::Foreign`
  to `Verdict::Translate(Reason::ForeignOrthography)` before both
  `Reading::Dominant` and the `Reading::Minority` hand-off. The new variant is
  additive, `Reason` being `#[non_exhaustive]`. `Basis::ExclusiveScript` now
  also means no foreign letter was seen. The crate doc and `Gate::verdict`'s
  `# Input domain` section state the domain and its misses.
- `crates/transync-lang/tests/gate_behaviour.rs` —
  `another_language_in_the_targets_script_is_not_the_target` and
  `a_foreign_letter_is_decisive_before_the_backend_is_asked` (both red before the
  fix), `the_shared_script_targets_still_recognize_their_own_language`, and
  `the_documented_residual_misses_are_still_misses`, which pins Bulgarian, Nepali
  and a Marathi sentence with no screened letter as the misses this record names.
  `script.rs`'s unit tests pin the letter sets.
- `docs/decisions/0028-the-language-gate-is-a-pre-flight-question.md`,
  `docs/architecture/README.md`, `docs/implementation/module-map.md` and the
  CHANGELOG carry the domain; DCR-0063 §12 records the contract delta.

## Consequences

* Good, because text carrying a screened letter no longer cancels a run for a
  Russian, Arabic or Hindi target. That covers Ukrainian, Belarusian, Serbian,
  Macedonian, Kazakh, Kyrgyz, Tajik and Mongolian; Persian, Urdu, Pashto,
  Kurdish, Uyghur, Sindhi and Jawi; and Marathi carrying ळ, ॲ or ऱ.
* Good, because genuine Russian, Arabic and Hindi keep the free shortcut and the
  backend band, and the screen adds `Translate` outcomes only: it can never
  produce a new `AlreadyTarget`.
* Good, because a caller can now read, on the API itself, when `AlreadyTarget`
  is evidence and when it is not.
* Bad, because target-language text quoting one such word costs a provider call.
  The supersets reach some genuine target text too — ॲ in Hindi loanword
  spellings, ऌ ॠ ॡ in Hindi alphabet lessons, Maghrebi ڤ ڥ ڨ in otherwise Modern
  Standard Arabic. Each costs one call, in the safe direction.
* Bad, because the exposure is narrowed, not closed. These still read as the
  target:
  - Bulgarian against Russian;
  - Nepali, and Sanskrit and Marathi that carry no screened letter, against
    Hindi (Sanskrit with its vocalic ऌ ॠ ॡ is caught);
  - on every target the backend decides — each Latin one and Chinese — a
    language it does not model (Catalan as Spanish, Norwegian as Swedish,
    Cantonese as Chinese);
  - Jeju for Korean, and Ainu and the Ryukyuan languages for Japanese, because
    Hangul and Kana are near-exclusive, not exclusive.

  Short text in a screened language that happens to carry no screened letter is
  the same miss.
* Bad, because the screen reads precomposed letters in the blocks `contains`
  covers, and nothing else. A decomposed spelling — `у` + U+0306 for `ў`, `र` +
  U+093C for `ऱ` — is not screened; NFC text is, because NFC composes both. So is
  a letter in the scripts' extended blocks (Arabic Extended-A and -B, Cyrillic
  Extended-B and -C), which counts as another letter, and a dependent vowel sign
  or mark of another orthography (Devanagari ॆ ॊ ॢ ॣ, the Arabic block's
  U+0657–U+065F marks). All of these fall in the same direction as the named
  misses, inside the domain statement.
* Neutral, because the change is visible to a library consumer upgrading from
  v0.5.0 — such text returns `Translate(Reason::ForeignOrthography)` where it
  returned `AlreadyTarget` — and `resp-translator`, which pins that tag, is to be
  told, along with the domain statement.

## Notes

Extends DCR-0045's "Voidness of the shortcut", which limited exclusivity to "the
backend's set" without saying what happens to input outside it; DCR-0045 stays
as written. Amends nothing in ADR-0028's three structural properties: the verdict
still carries no language, derives no serialization, and the crate still
depends on no workspace member.

TRACE: R0012-0014
TRACE: ti e522c774
TRACE: DCR-0063
TRACE: ADR-0028
TRACE: DCR-0045
