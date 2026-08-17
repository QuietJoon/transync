---
type: Reference
title: Profile TOML schema
description: Every key a Profile TOML accepts — type, default, valid range, how a CLI flag overlays it, and what the loader warns about.
tags: [profile, cli, glossary, batching, DCR-0026, DCR-0027]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: profile-rs, resource: crates/transync-core/src/profile.rs }
  - { id: llm-rs, resource: crates/transync-core/src/llm.rs }
  - { id: llm-prompt-rs, resource: crates/transync-core/src/llm/prompt.rs }
  - { id: unit-rs, resource: crates/transync-core/src/unit.rs }
  - { id: unit-section-rs, resource: crates/transync-core/src/unit/section.rs }
  - { id: unit-budget-rs, resource: crates/transync-core/src/unit/budget.rs }
  - { id: unit-split-rs, resource: crates/transync-core/src/unit/split.rs }
  - { id: batch-rs, resource: crates/transync-core/src/batch.rs }
  - { id: validate-inline-rs, resource: crates/transync-core/src/validate/inline.rs }
  - { id: cache-rs, resource: crates/transync-core/src/cache.rs }
  - { id: core-lib-rs, resource: crates/transync-core/src/lib.rs }
  - { id: default-profile, resource: crates/transync-core/profiles/default.toml }
  - { id: cli-translate-args, resource: crates/transync-cli/src/translate_cmd/args.rs }
  - { id: cli-translate, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: cli-logging, resource: crates/transync-cli/src/logging.rs }
  - { id: openai-responses, resource: crates/transync-openai/src/client/responses.rs }
  - { id: profile-cookbook, resource: docs/Profile_Cookbook.md }
synced_hash: e3ceeb1a904c309f8e5fb987ab3296dd3923d7f99f16e379565bd4e8462f6332
---

# Profile TOML schema

The full schema in one file — every key at once, not a narrowed example.

A profile passed with `--profile <path>` is loaded **on its own**. The
shipped default profile
(`crates/transync-core/profiles/default.toml`) is not a layer underneath
it: the loader never merges a custom profile over the embedded one, so a
key absent from your file resolves to the built-in default in the tables
below, which is not always the value the shipped profile sets. The two
places where the shipped profile and the built-in default differ are
[`default_table_strategy`](#constraints) and the `[batching]` numbers.

```toml
# Required
slug    = "default"
version = "1.0.0"

# Optional
auto_glossary = false

# Required
[system]
prompt = """
…
Translate from {{source_language}} to {{target_language}}.
…
"""

# Optional
[constraints]
preserve_code_identifiers = true
preserve_urls             = true
default_table_strategy    = "row-window-first"   # or "whole-block"

# Optional
[render]
target_direction = "auto"                        # or "ltr" / "rtl"

# Optional
[batching]
target_output_tokens          = 8000
output_expansion_factor       = 2.0
target_input_tokens_per_batch = 6000
max_units_per_batch           = 8

# Optional, zero or more
[[glossary]]
source = "agent"
target = "에이전트"
note   = "Standard term for AI agents"
scope  = "global"

[[glossary]]
source   = "cell"
target   = "감방"
scope    = "section"
sections = ["Prisons"]
```

## Top level

| Key | Type | Required | Default | Notes |
|---|---|---|---|---|
| `slug` | string | yes | — | Missing is a hard load error. |
| `version` | string | yes | — | Opaque cache-invalidation token, never parsed as semver. It is a `CacheKey` axis, so any change to it invalidates cache entries keyed on this profile. |
| `auto_glossary` | bool | no | `false` | Opts into the auto-extracted candidate-glossary preflight. Precedence: `--auto-glossary`/`--no-auto-glossary` flag > this key > built-in `false`. |

Unknown top-level sections, and unknown keys inside known sections, are
ignored with a path-qualified warning (`transync::profile` tracing target;
reaches CLI stderr once, unless `--quiet`). Adding a new top-level section
to a future profile is non-breaking; removing or repurposing a key
requires bumping the shipped default profile's `version` so downstream
caches invalidate.

Only two load failures exist — a missing required field and malformed
TOML. Every other finding on a profile is a warning, and the run
continues with the value the warning names.

## `[system]`

| Key | Type | Required | Notes |
|---|---|---|---|
| `prompt` | string | yes | The system prompt template. Participates in cache identity via the compiled prompt hash. |

Template variables substituted at compile time: `{{source_language}}`,
`{{target_language}}`. Any other unnamespaced `{{name}}` is reported as a
warning and reaches the model literally.

## `[constraints]`

| Key | Type | Resolved when absent | Notes |
|---|---|---|---|
| `preserve_code_identifiers` | bool | unset | Renders as an explicit prompt policy line. The inline-protection layer's code-span identity check is enforced **only** when this is `true`. |
| `preserve_urls` | bool | unset (behaves as `true`) | Renders as an explicit prompt policy line. The inline-protection layer's link/image-destination identity check is enforced unless this is `false`. |
| `default_table_strategy` | `"whole-block"` \| `"row-window-first"` | `"whole-block"` | Effective since v0.4.0 (DCR-0026). Under `"row-window-first"` a table whose estimated response exceeds the effective output ceiling is split at packing time into header-carrying row windows — each window a complete GFM table — and reassembled into one table before regeneration. Under `"whole-block"` every table ships as one unit whatever its size; an oversize one is named by the output-budget preflight and then aborts the run at the provider. Both values load silently. An unrecognized value raises a load warning and resolves to `"whole-block"`. `--table-strategy` overlays this key. Never rendered into the prompt — it is application policy, not a model instruction. |

The split is inert in two further cases regardless of the strategy: when
`[batching].target_output_tokens` is unset (there is no ceiling to be
over), and for any table that fits the ceiling, is not a single parsed
table, or has fewer than two body rows to distribute. Those tables ship
whole.

**The absent-key resolution and the shipped default disagree**, and the
disagreement is silent. `crates/transync-core/profiles/default.toml` sets
`default_table_strategy = "row-window-first"`, so a run with no
`--profile` splits oversize tables. A hand-written profile that omits the
key resolves to `"whole-block"` — with no load warning, because an absent
key is not an error — so passing it with `--profile` disables the split
and restores the whole-run provider abort on an oversize table. Writing
the key into the profile, or passing `--table-strategy row-window-first`,
is what turns it back on.

## `[render]`

| Key | Type | Default | Notes |
|---|---|---|---|
| `target_direction` | `"auto"` \| `"ltr"` \| `"rtl"` | `"auto"` | Presentation-only: consumed only by the HTML-bundle emitters. Never enters the system prompt or cache identity. `--target-direction` overrides it. `"auto"` applies a best-effort RTL primary-subtag table to the target-language label. Unknown values warn and normalize to `"auto"`. |

## `[batching]`

| Key | Type | Range | Built-in default when unset | Notes |
|---|---|---|---|---|
| `target_output_tokens` | integer | `>= 1`, and above the 64-token response-envelope reserve | no ceiling | Sent to the provider as `max_completion_tokens` (Chat Completions) / `max_output_tokens` (Responses). Also drives output-aware packing, and the row-window table split, when set. `0` — and any value at or below the 64-token envelope reserve — is rejected with a warning and normalized to unset; no literal `0` is ever sent. |
| `output_expansion_factor` | float | finite, `> 0` | `2.0` | Estimated output/source token ratio used to size batches against `target_output_tokens`. Values below `1.0` are legal. Only has effect when `target_output_tokens` is set. |
| `target_input_tokens_per_batch` | integer | `>= 1` | `6000` | Per-batch input-token budget. |
| `max_units_per_batch` | integer | `>= 1` | `32` | Hard cap on units per batch. |

The shipped default profile sets `target_output_tokens = 8000`,
`max_units_per_batch = 8` and `output_expansion_factor = 2.0`, and leaves
`target_input_tokens_per_batch` unset — so a run with no `--profile` packs
to at most 8 units per batch, not 32, and takes the built-in `6000` input
budget.

Every sizing knob above treats `0` the same way: rejected with a
path-qualified load warning, normalized to unset, and the key then
resolves exactly as if it had been absent — never floored to `1`.

`[batching].max_split_retries` was **removed in v0.4.0** (DCR-0026). It is
no longer a field, no longer in the loader's key allowlist, and no longer
travels on the compiled profile. A profile still carrying it gets the
ordinary unknown-key load warning naming `batching.max_split_retries` and
is otherwise unaffected.

There is no key for merging sections into one batch. `coalesce_sections`
does not exist at any level — profile key, CLI flag, or library option —
and writing it produces an unknown-key warning and no behavior.

### These budgets bind within one section

Since v0.4.0 (DCR-0027) the unit list is partitioned at every heading of
any level before anything is packed, and each section is packed on its
own. Every batch's units therefore come from exactly one section, and no
batch straddles a `##` / `###` boundary. The budgets above cannot merge
two sections: on a heading-rich document the run dispatches at least one
batch per section however large the budgets are, so they no longer
predict batch count from document size alone. What they still control is
how a single section is divided — a section that exceeds a budget is the
only thing that splits one section across several batches, all of them
still confined to that section. A document with no headings is one
section and packs exactly as it did before.

Batch ids remain a single document-order sequence `1..N` across sections.

See
[why batches stop at section boundaries](../../../explanation/user/en/why-batches-stop-at-section-boundaries.md)
for what the rule buys and what it costs.

### Overlays

A caller-set `TranslateOptions` value that differs from the built-in
default wins over the profile value for `target_input_tokens_per_batch`
and `max_units_per_batch` (a caller passing the built-in default is
indistinguishable from "unset"). The CLI's four batching flags
(`--target-output-tokens`, `--output-expansion-factor`,
`--target-input-tokens-per-batch`, `--max-units-per-batch`) overlay the
profile for one run this same way, as does `--table-strategy` — applied
in the same place, though it lands on a `[constraints]` key rather than a
`[batching]` one. `--max-concurrent-batches` has no profile home at all —
see [CLI flags](./cli.md).

## `[[glossary]]` — zero or more entries

| Key | Type | Required | Notes |
|---|---|---|---|
| `source` | string | yes | Non-whitespace. Compared trimmed and case-folded. How often one term may be claimed depends on scope — see [Claiming a term](#claiming-a-term). |
| `target` | string | yes | Non-whitespace. |
| `note` | string | no | Free text. A note that is not the empty string **is rendered**, appended to the entry's bullet after an em dash, so it reaches the model as prompt text. |
| `scope` | `"global"` \| `"section"` | no, default `"global"` | `"global"` (alias `"global_across_document"`) renders into every batch's compiled prompt. `"section"` (alias `"conditional_on_section"`) renders into the prompt of batches from the sections `sections` names, and into no other; effective since v0.4.0 (DCR-0027). |
| `sections` | array of strings | required non-empty when `scope = "section"`; meaningless under `"global"` | Heading-text selectors naming where a section-scoped entry applies. |

### Section selectors

A section's identity is its **heading stack**: the plain text of every
heading enclosing the section's body blocks, outermost first, including
the heading that opens it. A section-scoped entry applies to a section
when any heading on that stack equals any of the entry's selectors,
compared trimmed and lowercased.

- Heading **levels** are never compared. A selector names a section by
  what it is called, not by how deep it sits.
- Matching the stack rather than the innermost heading gives subsection
  inheritance: `sections = ["Installation"]` also applies inside
  `### Windows` beneath `## Installation`.
- Selectors are literal heading text only. Path expressions, globs and
  regular expressions are not supported.
- The preamble — every block before the document's first heading — has an
  empty heading stack, so no section-scoped entry can ever apply there.
  Only global entries steer the preamble.

### Normalization

Four `scope`/`sections` combinations cannot mean what they say. Each is
normalized with a warning; none is an error.

| Written | Outcome |
|---|---|
| `sections` on a `scope = "global"` entry | The list is named in a warning and cleared. The entry still applies to the whole document. |
| An empty or whitespace-only selector | That selector is named and dropped. |
| A selector repeating an earlier one in the same entry | The duplicate is named and dropped; the first spelling survives. |
| A `scope = "section"` entry whose selector list is empty after the above | The **whole entry** is named and dropped — it would apply nowhere. |

An entry with an empty or whitespace-only `source` or `target` is dropped
and named in a warning rather than sent.

### Claiming a term

The rule is "claimed once per place the claims can meet", settled at load
time because selector overlap is a property of the profile alone:

- **Two `global` entries on one term** — the first in profile order wins,
  the later one is dropped with a warning.
- **Two `section` entries on one term** — legal while their `sections`
  selector sets are disjoint (selectors compared trimmed and
  case-insensitively). A later entry sharing a selector with an earlier
  one loses under the same first-wins rule and is dropped with a warning.
- **A `global` and a `section` entry on one term** — both load. This is
  the intended override pattern, and which one applies is decided per
  section.

### Which entries a batch carries

The effective glossary of a section is every `global` entry plus every
applicable `section` entry, resolved per case-folded source term:

- With no applicable section-scoped claim on a term, every entry claiming
  it passes through unchanged. A profile with no section-scoped entry at
  all therefore compiles byte-identically to one written before section
  scope existed, and produces identical cache keys.
- An applicable section-scoped entry beats the global entry for the same
  term, **silently** — the override is what the scope is for, not a
  mistake.
- Among several applicable section-scoped entries on one term — which
  nested headings can produce even from disjoint selector sets — the
  first in profile order wins and each loser raises a shadowing warning.

Sections resolving to the same entry list share one compiled prompt.
`profile_prompt_hash` and `glossary_hash` are consequently per batch, not
per run: two sections with identical effective glossaries can share cache
entries, and two units differing only in which section they fall in stop
sharing.

For the reasoning behind these rules, and how they interact with the
auto-glossary preflight, see
[how glossary entries are resolved](../../../explanation/user/en/how-glossary-entries-are-resolved.md).

### Diagnostics

Three warnings are raised while batches are built rather than at load,
all on the `transync::profile` tracing target (CLI stderr, unless
`--quiet`):

| Warning | When | How often |
|---|---|---|
| `glossary[i] is shadowed in section "…": glossary[j] already maps "…" there and comes first in the profile …` | Two applicable section-scoped entries claim one term in some section | Once per (shadowed, winner) pair per run, naming the first section it was observed in |
| `glossary[i] is scoped to sections […], none of which this document has; the entry reaches no prompt in this run …` | A section-scoped entry matched no section of the document being translated | Once per run, per entry |
| `glossary[i].<field> contains the control character U+XXXX; it is escaped in the rendered prompt …` | A control character sits in `source`, `target` or `note` | Once per run, per field |

None of the three drops an entry. The second is advisory and never a
gate: a profile is document-independent, and an entry may be meant for a
sibling document. The third is advisory too — the entry is kept, and the
escaping it describes happens unconditionally.

All three are raised over the glossary the batches actually carry rather
than over the file, so the control-character advisory also names a
character in a term the auto-glossary preflight supplied. The other two
never can: an extracted entry is forced to `scope = "global"`.

### Rendering

Each entry the batch carries renders as one bullet in the compiled system
prompt — `- "source" → "target"`, plus ` — note` when a non-empty note is
present. `scope` and `sections` are never rendered in any form: scope is
expressed by filtering the list handed to the renderer, never by
annotating a bullet. Rendering a profile directly renders its **whole**
glossary, because the section filter belongs to the batcher rather than
to the renderer.

Backslashes, double quotes and control characters in `source`, `target`
or `note` are escaped at render time (`\n`/`\r`/`\t`, else `\u{XXXX}`)
rather than dropped, so a term cannot end its own bullet and open a line
of its own inside the prompt.

The embedded shipped default ships with an **empty** active glossary by
design — one glossary entry names a target *form* with no target
*language*, so a shipped entry would apply a Korean rendering to a run
into Japanese just as readily as one into Korean.

## Related

- [How to write a Profile TOML](../../../how-to/user/en/write-a-translation-profile.md) —
  the task-oriented recipe this reference complements.
- [`docs/Profile_Cookbook.md`](../../../../docs/Profile_Cookbook.md) —
  five ready-to-adapt recipes (technical, literary, marketing, code-heavy,
  strict-preserve). All five predate section scope: every entry is
  `scope = "global"`, and every recipe writes
  `default_table_strategy = "whole-block"`.
