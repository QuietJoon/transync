---
type: Explanation
title: Why layered validation and bounded retry/fallback
description: Why the pipeline checks a translation through six independent layers instead of trusting the schema, why retries resubmit verbatim instead of coaching the model, which seams are allowed to refuse rather than degrade, and where the fallback paper trail is thinner than it looks.
tags: [architecture, validation, ADR-0009, ADR-0017, ADR-0018, DCR-0025, DCR-0026]
audience: developer
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: adr-0009, resource: docs/decisions/0009-reject-retry-policy-changes.md }
  - { id: adr-0017, resource: docs/decisions/0017-batch-terminal-failures-abort-the-run.md }
  - { id: adr-0018, resource: docs/decisions/0018-html-content-translation-via-segment-extraction.md }
  - { id: dcr-0025, resource: docs/project/design-change-records/DCR-0025-review-0003-content-loss-and-refusal-precedence.md }
  - { id: dcr-0026, resource: docs/project/design-change-records/DCR-0026-oversize-table-row-window-split.md }
  - { id: stub-manifest, resource: docs/project/stub-manifest.md }
  - { id: validate, resource: crates/transync-core/src/validate.rs }
  - { id: validate-per-kind, resource: crates/transync-core/src/validate/per_kind.rs }
  - { id: pipeline-merge, resource: crates/transync-core/src/pipeline/merge.rs }
  - { id: pipeline-policy, resource: crates/transync-core/src/pipeline/policy.rs }
  - { id: unit-split, resource: crates/transync-core/src/unit/split.rs }
  - { id: htmlseg, resource: crates/transync-syntax/src/htmlseg.rs }
  - { id: render, resource: crates/transync-syntax/src/render.rs }
  - { id: parser-ranges, resource: crates/transync-syntax/src/parser/ranges.rs }
  - { id: align, resource: crates/transync-syntax/src/align.rs }
synced_hash: 16d55a9db2369c25017d0ead6a08b21f6c2d206c4b21014f6d7c76724b7e401e
---

# Why layered validation and bounded retry/fallback

A `Translator` implementation is asked for structured JSON and, on a
Structured-Outputs-capable model, will usually return exactly the shape it
was asked for. It would be tempting to stop checking there. transync
doesn't, and the reasons compound into a shape worth understanding as a
whole rather than layer by layer.

## Schema compliance is not content correctness

A model can return perfectly valid JSON — the right keys, the right
types — and still have quietly broken the one thing that isn't expressed
in a JSON Schema: that a Markdown table still has the same column count,
that a code fence's language tag survived, that a link's destination
wasn't paraphrased along with the surrounding prose. None of that is a
schema violation. It's a *structural* violation, and structural integrity
is the thing the application — not the model — is supposed to own
(invariant 2: the LLM owns content, the application owns structure). So
after the schema layer — which is also where a payload-byte rule no JSON
Schema can express lives, and where a batch-envelope fault is attributed —
the pipeline checks the ID set (nothing missing, nothing extra, no
duplicates), then per-block-kind shape (table columns, list depth,
blockquote children, code-fence metadata, heading level, raw-HTML segment
count), then re-parses the translated fragment under GFM to confirm it's
still the same *kind* of block, then checks inline protection (link and
image destinations, raw inline-HTML tag identity, code-span identity where
policy asks for it). Each layer catches a failure mode the ones before it
cannot see.

The final layer is different in kind from the rest: after every unit is
individually valid, the whole regenerated document is reparsed and the
anchor count and order are checked against the source. This is what
catches drift that only exists at the seams — a splice that leaves the
document syntactically different from what any per-unit check could
detect in isolation. It's why unit validity is explicitly two-tier
(*provisional* per-unit, *final* only after the whole-document reparse):
a unit can pass every per-unit gate and still be downgraded later if the
document it became part of doesn't hold together.

## Layering is not a safety net, and it cuts both ways

The comfortable reading of a six-layer stack is that a hole in one layer
is caught by the next. That reading is wrong, and the project has a
concrete counter-example rather than a theoretical worry.

The raw-HTML layer used to reject an *empty* translated segment. A
segment of a single space is not empty. It splices back as
markup-preserving whitespace, so the splice succeeds, the tag inventory
matches, the fragment reparse matches, and the full-document reparse
matches — every later layer agrees, because nothing *structural* changed.
The block shipped marked `translated` with its visible sentence gone. No
layer after the second one was ever going to see it, because they all
check structure and the structure was fine. The fix (DCR-0025, amending
ADR-0018) was to make the earliest layer state the rule the extractor
already enforced: the segment scanner keeps a source text node only if
its *decoded* form carries something other than whitespace, so a
whitespace-only source segment cannot exist and a whitespace-only answer
has no legitimate meaning. Rejecting it is therefore free of
false positives, and the rejection is retryable like the rest of that
layer — the message even names the correct provider answer, which is to
echo the source segment back.

The code-fence rule closed in the same pass has the same shape. Fence
metadata was compared only when the *source* fence carried an info
string, so an infoless fence whose translation came back as a
Rust-tagged fence sailed through — and regeneration reads the info
string off the *translated* payload, so the invented language tag reached
the output file. That is the `None → Some` direction, and no decision ever
permitted it: choosing a language for a code block is a structure
decision, which invariant 2 reserves for the application. The
`Some → Some` verbatim rule was already right and did not change; what
changed is that an absent info string is now stated as a rule instead of
being an early return.

The symmetric risk is easy to forget and costs just as much output.
Validation that *invents* a fault corrupts a document as surely as
validation that misses one. The raw-HTML tokenizer used to admit
digit-leading tag names, so ordinary prose like `Rows <2026 total> here`
registered as markup. The engine proves a translated HTML block preserved
its markup by comparing the tag inventory of the spliced result against
the source's; a phantom entry made that comparison disagree and dropped a
perfectly good translation to source content, and on the rebuild path the
same mis-scan could delete a written `</1>` from the pane outright.
Because that comparison is one of the engine-fault paths described below,
the drop was also immediate — no layer named, no retry attempted. A guard
is not automatically on the safe side of a trade just because it is
strict.

## Where a guard is allowed to refuse

"Refuse, never silently degrade" is this page's governing idea, and it is
worth being honest that it is not unconditional. The codebase now contains
the same question answered three different ways, on purpose, and the
deciding factor each time is whether the seam owns an error channel.

The renderer refuses. A byte range that is reversed, past the end of the
pane's Markdown, or landing inside a UTF-8 character is not a slice of
that pane, and since DCR-0025 the pane refuses to render at all, naming
the offending block ids with their offsets and the reason. It used to
clamp and snap such a range into whatever bytes survived — which still
renders, so a block whose alignment row pointed at the wrong bytes
presented as truncated, empty, or unrelated content underneath a
correct-looking anchor. Two details are worth stating precisely, because
they are easy to get backwards: no call became newly fallible here.
Renderer construction became fallible earlier, when a duplicated or
missing alignment row started being refused (DCR-0022); what DCR-0025
added is a new *refusal reason* on that existing, deliberately
non-exhaustive error type. And because the CLI bundle and the
WebAssembly demo are the same renderer, both inherit the refusal rather
than one of them keeping the old coercion.

Regeneration does not refuse. It is infallible by design — the caller has
nowhere to put an error — so for it the clamp-and-snap is the *safe*
answer and the alternative is a panic. The shared helper that does the
snapping now has exactly one caller for that reason, and its own
documentation records the split.

Alignment-map construction warns. When it is handed incomplete offsets it
still emits an empty range for the uncovered blocks, because the empty
range is genuinely the safe value there, and it names the blocks on a
`tracing` warning instead of forcing every caller to handle a `Result`
for an input no shipped path can produce. That is a real, named limit: an
embedder that installs no `tracing` subscriber sees nothing at all.

The reusable rule is not "always refuse". It is: a seam that can say no
must say no; a seam that cannot must pick the value that fails loudly
somewhere else rather than the value that looks like content.

## Retry: bounded, verbatim, and deliberately not smarter

When a unit fails validation, the obvious next move looks like "tell the
model what it got wrong and ask again." transync's retry does something
narrower: it resubmits the *exact same request* — same payload, same
scope — and relies on the model's own stochastic variation to eventually
produce a compliant shape, with a bounded budget (default: 2 retries, so
3 total content attempts) rather than an unbounded coaching loop.

This was reviewed and challenged (ADR-0009) — twice, independently — and
kept both times, for a reason that's easy to miss: **the fix that looks
obviously better is unsafe.** Putting corrective guidance into the
payload the model sees was tried. A faithful model, doing exactly what a
well-behaved translator should do, translates the guidance text along
with everything else and echoes it back as an extra block — which the
full-document reparse then correctly rejects, because there's genuinely
one more block than there should be. The fix broke the thing it was
trying to improve. What shipped instead, once it was needed, is a
non-content side channel (`RetryContext`: attempt number, which layer
rejected the previous attempt, a short reason) carried *beside* the
payload — in the system prompt framing, never inside `source_payload` —
so a model can use the hint without being able to leak it into output.
The verbatim-resubmission rule itself never changed; only the channel for
optional guidance was added on top of it.

Row-window splitting is the interesting stress test of that rule, and it
passes without amending it. An oversize table is broken into
header-carrying windows at *packing* time, before the first provider call,
deterministically — the same kind of act as deciding batch boundaries.
Nothing after packing ever re-scopes a unit: there is no reactive
re-split, no failure or provider signal shrinks a window, and a window is
an ordinary unit from birth whose scope is fixed for the whole run. So a
window that fails validation is resubmitted verbatim, exactly like any
other unit, charged to its own content budget. The documented worst-case
round bound still holds with the unit count now counting windows rather
than source tables.

The same conservatism shows up in how the retry budget is charged. A
provider that dropped one unit's result row and a provider that
mistranslated a sibling unit are different failures, and as of DCR-0014
they're charged to different budgets — a *batch-envelope* fault (rows
missing or duplicated) spends a separate, once-per-round budget, so a
unit whose content was never actually judged doesn't end up with fewer
real translation attempts than a unit that was merely wrong. Fairness
between failure modes turned out to need its own accounting, not a
shared counter. A third budget, for transient transport errors,
deliberately does *not* reset when a batch enters another dispatch round:
it spans the whole ladder, so a batch cannot re-earn network attempts by
failing validation.

## Whose budget pays: model faults, provider faults, engine faults

Two of those three categories are the ones most designs anticipate. The
third is the one worth naming, because it changes what a report means.

An *engine* fault is a failure the model did not cause, discovered after
its payload was already accepted. There are two today. One is a raw-HTML
block whose segments came back fine but whose splice then failed — or
whose spliced result no longer carries the source's markup. The other arrived
with row-window splitting: after every window has passed its own layers,
the windows are merged back into one table and the merged table is
re-inspected against the *source* block's column count, per-column
alignment, and body-row count. If that check fails, no model output was
at fault — the engine's own reassembly was.

Both take the same shape, and it is deliberately not a retry: the block
falls back to source content directly, with no rejecting layer and no
rejection reason recorded, no retry budget spent, and — for the merge
case — every window's cache entry evicted, so a re-run against a shared
cache cannot deterministically replay the same broken state. Spending the
model's budget on the engine's mistake would be dishonest accounting, and
caching the result would make a transient engine fault permanent.

The consequence for anyone auditing a run is concrete: `rejected_by`
alone does not account for every `fallback_source`. Two of them have no
rejecting layer, and the only place their cause is written down is the
report's warnings channel.

## Fallback is a last resort with a paper trail — with one hole in it

When a unit's retries are exhausted, it falls back to source content —
untranslated, in the target document, explicitly marked
`fallback_status` in the alignment map. This is the one place the
pipeline is allowed to ship something other than a real translation, and
it's deliberately visible rather than smoothed over: every block's final
status is in the map, and the validation report names which layer
rejected each failed attempt.

Row-window splitting weakened that guarantee, and the weakening is worth
stating plainly rather than dropping the claim. A split table can
finalize as `partially_translated`: the windows that succeeded contribute
their translated rows and a terminally-failed window contributes its own
*source* rows, so one table can ship half in each language. The alignment
map records `partially_translated` on that block — but only on that
block. **Which** rows fell back is not in the map and not in the DOM,
because window ids never reach either; they live on the wire and in
`validation-report.json` only, as rows whose unit id is the parent block
id plus a window ordinal. The failed windows are named on the report's
warnings channel and nowhere else, and in the rendered output a
source-language row inside a translated table looks exactly like a row
the model chose to preserve.

So the honest form of the guarantee is narrower than "every fallback is
findable in the alignment map." It is: every *block*'s status is in the
alignment map, and the reason — plus any sub-block detail — is in the
validation report. A tool that audits only the map will see that a table
is partial; it will not see which part. A consumer reading the report
must also stop assuming that every unit id in it names a block: since the
report's schema minor bump, some of them name row windows that exist
nowhere else.

## Why a batch-terminal provider failure aborts the whole run

Fallback exists for *validation* failures — the model tried, and what it
returned didn't hold up. It does not exist for *provider*-level
failures — truncated output, a refusal, transport retries exhausted. When
one of those happens to a batch, the whole run aborts with no output
written, and this was deliberately re-examined and kept (ADR-0017) against
two proposals that both sound reasonable on their own: degrade just that
batch to source content and keep going, or make abort-vs-degrade a policy
flag the caller chooses.

Both were rejected for the same underlying reason. The CLI already
commits every output file as one atomic, all-or-nothing staged set
(DCR-0006 / DCR-0011) specifically so a consumer never has to reason
about a half-written result. A per-batch fallback rung would quietly
reintroduce exactly the ambiguity that staging exists to remove: a run
that exits `0` — success — while silently carrying an untranslated
island somewhere in the middle, indistinguishable in raw Markdown from a
paragraph that was *supposed* to stay in the source language. A loud
abort with a diagnostic is self-correcting, because the failure is
visible the moment it happens; a document with a hidden gap is not,
because nothing about it looks wrong until a bilingual reader happens
to reach that paragraph. The policy-flag option was rejected on a
related ground: "sometimes abort, sometimes silently degrade" doubles
the states every consumer of the output has to handle, in service of a
mode whose only honest use is "I'm willing to ship a mixed-language
document" — an appetite the design does not want to make a first-class,
supported shape.

The trade this accepts is real, named, and — since the row-window
splitter shipped — narrower than it was. A **boundary document**, where a
single block's translation genuinely cannot fit under any output ceiling
a request can carry, used to be untranslatable until an operator raised
the ceiling or split the source by hand. For tables that is over: an
oversize table is now split into header-carrying row windows at packing
time and reassembled afterwards. But note *where* the splitter sits. It
is on the prevention side of ADR-0017, alongside output-aware packing and
the preflight that warns about and names any unit already over budget
before a single request goes out; it is not a fallback rung that catches
a provider failure at runtime. A provider-side oversize signal *after*
packing is still terminal, exactly as ADR-0017 decided, and a single
table row so large that it cannot fit even in a window of its own still
aborts the run.

Every other block kind keeps ADR-0017's semantics unchanged — paragraph,
code block (invariant 4 makes a fence indivisible), list item,
blockquote, raw HTML — each excluded from splitting by a recorded
per-kind decision rather than by omission. And whether the splitter is
even active for your run is a profile question, not a property of the
engine: the embedded default profile turns it on, but a hand-written
profile that omits the key resolves to whole-block silently, which is the
pre-splitter behaviour with no warning that you chose it. The
[profile schema reference](../../../reference/user/en/profile-toml-schema.md)
documents that trap in the form an operator needs.

## Related

- [Why block-ID sync, not scroll-percentage](./architecture-overview.md) —
  the ID contract this validation model is built to protect, and why a
  split table is still one block to everything downstream of the merge.
- [How to diagnose a translation run](../../../how-to/user/en/diagnose-a-translation-run.md) —
  the task-oriented counterpart: reading `validation-report.json` when one
  of these layers actually fires.
- [Alignment-map JSON schema](../../../reference/developer/en/alignment-map-schema.md) —
  the `fallback_status` vocabulary this page reasons about, stated
  normatively.
- `docs/architecture/contracts.md` §5 — the full retry/fallback decision
  flow, the three retry budgets' exact semantics, the row-window merge
  rules, and the two fallbacks that name no rejecting layer.
