---
type: ADR
title: Language labels are opaque caller-supplied strings (no BCP-47 validation)
description: Source/target language values are forwarded verbatim to prompts, cache keys, and the alignment map; validation is limited to non-emptiness, and label-based prompt injection is out of scope because the label author is the prompt author.
tags: [decision, ADR-0013]
status: active
---

# ADR: Language labels are opaque caller-supplied strings

## Context and Problem Statement

Found in Review 0001 (Issue R0001-0072, Severity: Medium) (review archived and removed), re-raised in
Review 0008 (Issue R0008-0028, Severity: Medium). Location:
`crates/transync-cli/src/translate_cmd.rs` (`--target-language` /
`--source-language`), `crates/transync-core/src/profile.rs`
(`render_prompt_body`).

Review 0001 flagged that language values are not validated as BCP-47.
Review 0008 escalated the consequence: because labels are substituted
into the system-prompt template, a label containing newlines or
instruction text modifies the trusted prompt outside the
source-content-as-data framing.

## Decision Drivers

* The label reaches three consumers — the model prompt, the cache key,
  and the alignment map — none of which needs a *parsed* language tag;
  they need a stable, caller-meaningful string. "Korean (formal, 존댓말)"
  is a legitimately useful label a BCP-47 validator would reject.
* There is no cross-principal boundary to defend: the person supplying
  `--target-language` is the same person who controls `--system-prompt`,
  the profile TOML, and the environment. A "label injection" is the
  operator instructing their own model run.
* Non-emptiness IS enforced (R0008-0039): blank labels are rejected at
  the CLI with exit 1, and `translate_with_cache` rejects an empty
  target language.

## Considered Options

1. Validate labels as BCP-47 and reject everything else.
2. Quote/encode labels as data inside the prompt template.
3. Keep labels opaque; enforce non-emptiness only; document the
   recommendation (BCP-47 form) without enforcing it.

## Decision Outcome

REJECT (both findings): option 3. BCP-47 enforcement breaks legitimate
expressive labels for zero security gain; encoding labels as "data"
misrepresents them — they are operator instructions, exactly like the
system prompt they land in. `contracts.md` §6 and the CLI arg docs
state the opaque-label contract explicitly.

Labels are fully opaque for every *engine* consumer: the system prompt, the
cache key, the alignment map, and every validation layer forward them
verbatim, unparsed, with only the non-emptiness check (R0008-0039). What they
forward is the label the caller passed or, on a CLI run, the one value the
argument boundary stored for it (below). The rest of this section is the one
reserved literal and every place that reads a label beyond forwarding it; none
of them parses a tag, and none makes the label load-bearing for the engine.

**One reserved literal.** This ADR reserves exactly one literal: `auto`, the
source-language sentinel that compiles to *"the auto-detected source
language"* instead of being substituted. No literal is reserved on the target
side, so `--target-language AUTO` is an ordinary label.

**The CLI normalizes once, at its argument boundary.** The labels are
validated in `translate_cmd::args::resolve_language_and_model_args`, and every
CLI consumer reads the one normalized value it stores. This is **not**
validation and does not narrow what counts as a label:

- *Surrounding* whitespace comes off both labels. Nothing in a label's
  interior is touched, no case is folded, and no tag is parsed.
  `"Korean (formal, 존댓말)"` survives byte-for-byte, spaces included. A
  whitespace-only label is blank, and blank is an error.
- A recognized sentinel is stored in its **canonical spelling**, `auto`.
  **Only the reserved literal is folded.** A value the recognizer rejects is
  stored byte-for-byte, case included — `DE` stays `DE`, `Auto-detect` stays
  `Auto-detect`. This is not case folding of *labels*; it is a reserved word
  being spelled the one way it is defined.
- **One recognizer, two callers.** `args::is_source_language_sentinel` is the
  only place the CLI decides whether a value is the sentinel; the argument
  boundary canonicalizes with it and the bundle emitter's
  `pane_source_language` asks it. Two copies of the question are what let
  answers diverge, so there is one.
- `--model` is deliberately **not** normalized: it is a provider identifier,
  not a label this ADR governs.

**The library layer deliberately does not normalize.**
`TranslateOptions::source_language` stays opaque, and the library substitutes
a non-sentinel label verbatim; only its sentinel *test*
(`profile::render_prompt_body`) tolerates padding and case. Padding is removed
by the *argument boundary*, which is the layer that knows a value was typed on
a command line. A caller that bypasses the CLI with `"AUTO"` therefore
compiles the detection prompt but keys and stamps `AUTO` — pinned by
`scn_11_the_library_keeps_the_sentinels_spelling`, so the asymmetry is a
recorded decision rather than an oversight.

**The batch packer measures the labels.** This ADR is the decision that labels
have no bounded size — `"Middle High German as written in the Rhine Franconian
scribal tradition"` is exactly the expressive label it exists to protect — so
the per-batch reserve is four encoded strings: the instruction envelope, the
compiled system prompt, and the two labels, with no fixed allowance in it.
**The packer measures what it will send and assumes nothing about the size of
anything.** Counting a string's tokens is the same operation the packer
performs on the system prompt and on every unit's payload — it reads the
bytes, it does not interpret them.

**The bundle's pane direction is a presentation hint.** It is confined to the
**presentation layer** — the CLI's HTML bundle emitter, the one component
whose job is rendering:

- When direction resolves to `auto` (the default), the emitter applies a
  best-effort **hint**: the label's first `-`/`_`-separated token,
  ASCII-lowercased, is matched against a 15-entry RTL primary-subtag table
  (`ar arc ckb dv fa he iw ji nqo ps sd syr ug ur yi`); a match stamps
  ` dir="rtl"` on the pane, everything else emits nothing (LTR is the HTML
  default).
- The hint is **explicitly override-able** and the override is the
  authoritative path: `--target-direction {rtl|ltr|auto}` beats the profile's
  `[render].target_direction`, which beats `auto`.
- The hint is **best-effort by design and documented as such.** An expressive
  label — exactly the kind this ADR exists to protect — is not tag-shaped, so
  it resolves LTR: `"Korean (formal, 존댓말)"` → subtag `"korean"` → no match;
  `"العربية"` → no match. That is a *documented limitation*, pinned by the test
  `expressive_labels_stay_ltr`, not a parsing attempt. A caller who wants RTL
  for an expressive label uses the flag or the profile key.
- The hint is **invisible to identity**: it never enters the prompt, the cache
  key, the alignment map, or `out.md`, and it drove no alignment-map schema
  bump. Two runs differing only in pane direction are cache-identical.

Status: No change required. Raised twice. This decision rests on the
assumption that the label author and the prompt author are the same
principal — the single operator of a local CLI run. Revisit trigger: a
multi-tenant or service deployment that separates the label author from the
prompt author; that deployment would need its own boundary review before
relying on opaque labels.

A second revisit trigger guards the direction hint: if direction ever needs to
reach the alignment map or the prompt, the opacity contract must be
re-examined first — that would make the label load-bearing for the engine,
which this ADR forbids.

### Implementation

None for the rejection beyond the shipped non-emptiness checks (R0008-0039).
The rules above that read a label ship under their own records: the CLI
normalization under R0001-0021 (ti `ed8c57`) and ti `fd5aa8`, the measured
batch reserve under R0001-0013 (ti `24fd28`, `aa92d64f`), and the direction
hint under DCR-0015 / OI-0032.

## Consequences

* Good, because expressive labels ("한국어 — 기술문서체") flow through to
  the model unmangled, and cache identity keys on exactly what the
  model saw.
* Good, because at the CLI a padded label is indistinguishable from a bare
  one, and `AUTO` or `Auto` from `auto` — same compiled prompt, same cache
  identity, same alignment metadata. Sharing the key is correct and not merely
  convenient, because the canonical label is also what goes on the wire: two
  runs that share a key send byte-identical requests. No non-sentinel label's
  identity is touched by the folding.
* Good, because a caller who passes a very long label gets more, smaller
  batches rather than requests that quietly exceed the token target. Ordinary
  labels (`en`, `ko`, `한국어`) cost a token or two, so packing shape is
  effectively the same for them.
* Bad, because a typo'd label silently produces a differently-keyed,
  differently-prompted run instead of a validation error.
* Bad, because a caller who genuinely wants a label with a leading space
  cannot express it through the CLI. That is the intended trade, since such a
  label differs from its trimmed twin in no way anyone can see except cache
  identity.
* Bad, because the direction hint misses expressive labels; such a run renders
  LTR unless the caller sets `--target-direction` or
  `[render].target_direction`.

## History

The full text of each event is in
[`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0013.

- 2026-08-03 (DCR-0015, OI-0032): the HTML bundle gained a best-effort RTL pane-direction hint; engine consumers keep the label opaque.
- 2026-08-06 (R0001-0013, ti 24fd28): the batch reserve began encoding both labels instead of covering them with a fixed 256-token allowance.
- 2026-08-06 (R0001-0021, ti ed8c57): the CLI began trimming surrounding whitespace off both labels once, so a padded `auto` is the sentinel.
- 2026-08-06 (ti aa92d64f): `batch::FIXED_ENVELOPE_TOKENS` was removed; the per-batch reserve became four encoded strings.
- 2026-08-07 (ti fd5aa8): the CLI began storing a recognized sentinel as `auto`; a run spelled `AUTO` re-keyed once, then shared `auto`'s identity.
