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

Status: No change required. Raised twice. This decision rests on the
assumption that the label author and the prompt author are the same
principal — the single operator of a local CLI run. Revisit trigger: a
multi-tenant or service deployment that separates the label author from the
prompt author; that deployment would need its own boundary review before
relying on opaque labels.

### Implementation

None beyond the shipped non-emptiness checks (R0008-0039).

## Consequences

* Good, because expressive labels ("한국어 — 기술문서체") flow through to
  the model unmangled, and cache identity keys on exactly what the
  model saw.
* Bad, because a typo'd label silently produces a differently-keyed,
  differently-prompted run instead of a validation error.

## Amendment (2026-08-03) — presentation-layer direction hint

RTL pane direction shipped in the OI-2026-08 wave (DCR-0015, OI-0032). This
ADR is **amended, not violated**: labels remain fully opaque for every *engine*
consumer — the system prompt, the cache key, the alignment map, and every
validation layer still forward them verbatim, unparsed, with only the
non-emptiness check.

What is added is confined to the **presentation layer** — the CLI's HTML
bundle emitter, which is already the one component whose job is rendering:

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
  key, the alignment map (schema stays `1.1.0`), or `out.md`. Two runs
  differing only in pane direction are cache-identical.

Revisit trigger for this amendment: if direction ever needs to reach the
alignment map or the prompt, the opacity contract must be re-examined first —
that would make the label load-bearing for the engine, which this ADR
forbids.

## Amendment (2026-08-06) — the batch budget measures the label instead of assuming a size

Review-0001 `R0001-0013` pointed out a consequence of this ADR that the batch
packer had not absorbed: `batch::FIXED_ENVELOPE_TOKENS` (256) stood in for the
constant user-message instruction **and** the two language labels. An allowance
is a size assumption, and this ADR is precisely the decision that labels have no
bounded size — `"Middle High German as written in the Rhine Franconian scribal
tradition"` is exactly the expressive label this ADR exists to protect, and it
alone outweighs the whole allowance.

Ticket `24fd28` encodes `source_language` and `target_language` into the
per-batch envelope reserve; the constant now covers only the instruction it was
tuned for. This is **not** a parsing attempt and does not make the label
load-bearing for the engine: the labels are still forwarded verbatim and
unparsed to every consumer this ADR names. Counting a string's tokens is the
same operation the packer already performs on the system prompt and on every
unit's payload — it reads the bytes, it does not interpret them.

Consequence: a caller who passes a very long label now gets more, smaller
batches rather than requests that quietly exceed the token target. Ordinary
labels (`en`, `ko`, `한국어`) cost a token or two, so packing shape is
effectively unchanged for them.

## Amendment (2026-08-06) — surrounding whitespace comes off once, at the CLI boundary

Review-0001 `R0001-0021`. This ADR reserves exactly one literal: `auto`, the
source-language sentinel that compiles to *"the auto-detected source language"*
instead of being substituted. A reserved literal has to be recognizable, and
`--source-language ' auto '` was not recognized: the CLI validated
`trim().is_empty()` and then forwarded the untrimmed value, so the padded label
compiled a prompt naming ` auto ` verbatim, keyed its own cache entries, and was
stamped on the alignment map — while the HTML bundle's pane label, which had a
`trim()` of its own, treated the same run as auto-detected. One run, two
answers about whether the sentinel had been typed.

Ticket `ed8c57` trims *surrounding* whitespace once, where the labels are
validated (`translate_cmd::args::resolve_language_and_model_args`), and every
CLI consumer reads that one normalized value. The library's sentinel test
(`profile::render_prompt_body`) tolerates padding and case as well, so a caller
that never crosses the CLI gets the same answer.

This is **not** validation and does not narrow what counts as a label:

- Nothing in a label's interior is touched, no case is folded, and no tag is
  parsed. `"Korean (formal, 존댓말)"` survives byte-for-byte, spaces included.
- Non-emptiness is still the only rule (R0008-0039); a whitespace-only label is
  blank, which it already was under the old `trim().is_empty()` check.
- `--model` is deliberately **not** normalized: it is a provider identifier,
  not a label this ADR governs.
- The library layer still substitutes a non-sentinel label verbatim. Padding is
  removed by the *argument boundary*, which is the layer that knows a value was
  typed on a command line; `TranslateOptions::source_language` remains opaque.

Consequence: a padded run is now indistinguishable from a bare one — same
compiled prompt, same cache identity, same alignment metadata. A caller who
genuinely wants a label with a leading space cannot express it through the CLI;
that is the intended trade, since such a label differs from its trimmed twin in
no way anyone can see except cache identity.

## Amendment (2026-08-06) — the constant the labels were carved out of is gone entirely

The 2026-08-06 amendment above ended with `batch::FIXED_ENVELOPE_TOKENS` covering
"only the instruction it was tuned for". Ticket `aa92d64f` found that it never
covered even that: the assembled instruction outgrew 256 tokens on its own, and
the constant no longer exists. The per-batch reserve is now four encoded strings
— the instruction envelope, the compiled system prompt, and the two labels —
with no fixed allowance left in it.

This ADR's rule is unchanged and is now applied uniformly: **the packer measures
what it will send and assumes nothing about the size of anything.** The labels
keep the treatment R0001-0013 gave them, and they are still forwarded verbatim
and unparsed to every consumer this ADR names.

## Amendment (2026-08-07) — the reserved literal has one spelling at the CLI boundary

Ticket `fd5aa8`. The 2026-08-06 whitespace amendment made the reserved literal
recognizable through padding; **case** was the half it left. Both sentinel tests
in the tree folded ASCII case — `profile::render_prompt_body` and
`translate_cmd::publish::pane_source_language` — while
`translate_cmd::args::resolve_language_and_model_args` only trimmed. So
`--source-language AUTO` compiled *"the auto-detected source language"* and
rendered the bundle's source pane from the model's *detected* label, yet
`CacheKey::source_lang`, the user-message payload's `source_language` and the
alignment map's `source_language` all carried the literal `AUTO`. One request,
two identities: the prompt and the pane said "detection was requested", the key
and the metadata said "a label spelled `AUTO`". That is the R0001-0021 split
again, one notch weaker — the two answers no longer contradict each other, they
just fail to be the same answer.

The CLI now stores a recognized sentinel in its **canonical spelling**, at the
same place and for the same reason it trims:

- **One recognizer, two callers.** `args::is_source_language_sentinel` is the
  only place the CLI decides whether a value is the sentinel; the argument
  boundary canonicalizes with it and the bundle emitter's
  `pane_source_language` asks it. Two copies of the question were what let the
  answers diverge, so there is now one.
- **Only the reserved literal is folded.** A value the recognizer rejects is
  stored byte-for-byte, case included — `DE` stays `DE`, `Auto-detect` stays
  `Auto-detect`, `"Korean (formal, 존댓말)"` stays intact. This is not case
  folding of *labels*; it is a reserved word being spelled the one way it is
  defined.
- **The target side is untouched.** No literal is reserved there, so
  `--target-language AUTO` is an ordinary label.
- **The library layer is deliberately unchanged**, on the same layering the
  padding amendment stated: `TranslateOptions::source_language` stays opaque,
  and only its sentinel *test* tolerates padding and case. A caller that
  bypasses the CLI with `"AUTO"` therefore compiles the detection prompt but
  keys and stamps `AUTO` — pinned by
  `scn_11_the_library_keeps_the_sentinels_spelling`, so the asymmetry is a
  recorded decision rather than an oversight.

Consequence for the cache, stated plainly: **a run spelled `AUTO` (or `Auto`)
re-keys, once.** Entries it wrote under the old literal are still in the cache
and are never looked up again; from now on it shares identity with the `auto`
runs. Sharing is correct and not merely convenient, because the canonical label
is also what goes on the wire: two runs that now share a key send byte-identical
requests. Nothing about a non-sentinel label's identity moves.
