---
type: How-To Guide
title: How to diagnose a translation run
description: Read the exit code, the stderr line, and validation-report.json to find out why a run failed, refused, fell back, or looks wrong — and verify a profile's compiled prompt before spending a real run on it.
tags: [cli, troubleshooting, validation, SCN-07]
audience: user
language: en
generated:
  by: claude-code/claude-opus-5
  at: "2026-08-10T08:40:09Z"
sources:
  - { id: troubleshooting, resource: docs/Troubleshooting.md }
  - { id: contracts, resource: docs/architecture/contracts.md }
  - { id: exit-codes, resource: crates/transync-cli/src/error.rs }
  - { id: translate-cmd, resource: crates/transync-cli/src/translate_cmd.rs }
  - { id: output-rs, resource: crates/transync-cli/src/output.rs }
  - { id: validate-rs, resource: crates/transync-core/src/validate.rs }
  - { id: dispatch-rs, resource: crates/transync-core/src/pipeline/dispatch.rs }
  - { id: llm-rs, resource: crates/transync-core/src/llm.rs }
  - { id: profile-rs, resource: crates/transync-core/src/profile.rs }
synced_hash: 0f250857af96237961b827254c35a3bf416e6667b72ab6ab7c4265ac8efcb278
---

# How to diagnose a translation run

Use this when a `transync translate` run exited non-zero, or exited `0` but
the output looks wrong — unexpected fallbacks, a glossary term that never
shows up, a table that came back half-translated.

## 1. Read the exit code first

| Exit | Read this as |
|---:|---|
| `0` | Success. Note that a successful run can still print advisory `transync: note:` lines — see step 4. |
| `1` | Argument error — nothing ran. Also covers the CLI's own refusals: a blank language label, `--output` without `--map`, a malformed `--base-url`, a missing API key. |
| `2` | Input could not be read. Also covers an oversize `--input`, non-UTF-8 input, an oversize `--profile` or `--system-prompt-file`, and the parse-time block-nesting refusal. |
| `3` | Every translatable unit fell back to source. Outputs *were* written — go to step 2. |
| `4` | Write failure — an atomic write or bundle write errored. The filesystem, not the translation. |
| `5` | The residual: an unclassified translator error, a malformed provider response, or — for `transync serve` — an address that could not be bound. |
| `6` | The provider refused over **how the run was configured**: the credential, a `--model` that does not exist, an output ceiling too small, or the context window exceeded. Nothing was published. Fix the named knob and re-run. |
| `7` | The provider refused **this document's content** (content filtered, or the model declined). Nothing was published, and re-running unchanged cannot help — retry is a verbatim resubmission, so it sends identical content. |

The stderr line is the fastest diagnosis for most of these, but which prefix
you get depends on who refused:

- `transync translate` prints `transync: <reason>` — suppressed by `--quiet`.
- A bad flag is caught by the argument parser, which prints its own usage
  error that `--quiet` cannot suppress (parsing happens first).
- `transync serve` prints `transync serve: <reason>` and has **no** `--quiet`
  flag at all.

If you see no output whatever, check whether you ran under `cargo run
--quiet`, which also swallows cargo's own compile errors.

## 2. Get the per-unit report

Re-run with `--validation-report report.json`. An `--out-dir` run always
writes `validation-report.json` into its target directory, no flag needed.

Each `per_unit[*]` entry carries every attempt:

```json
{
  "unit_id": "t-0006",
  "attempts": [
    { "attempt_number": 1, "rejected_by": "per_kind_shape", "rejection_reason": "table column count 3 != 4" },
    { "attempt_number": 2, "rejected_by": "schema", "rejection_reason": "unit missing from batch response", "batch_fault": true },
    { "attempt_number": 3, "rejected_by": null, "rejection_reason": null }
  ],
  "final_status": "translated",
  "warnings": []
}
```

`rejected_by: null` on the last attempt means that attempt passed;
`final_status` is the outcome that matters. A unit whose `final_status` is
`fallback_source` exhausted its retries, and its last attempt's `rejected_by`
names the layer that kept rejecting it.

**`batch_fault` is the field that changes the diagnosis.** It appears only
when `true`, and it means this unit failed because the provider dropped or
duplicated its row in a batch response — not because of anything about this
unit's own content. Those attempts are charged to the per-batch schema budget
instead of the unit's content-retry budget. A `rejected_by: "schema"` with
`batch_fault` is a provider-shape problem; the same value *without* it is
about this unit.

### What `rejected_by` can say

| Value | Layer | Likely cause |
|---|---|---|
| `schema` | Structured-output shape | The response did not carry this unit's row as the schema requires — often a refusal, or a dropped/duplicated row (check `batch_fault`). |
| `per_kind_shape` | Per-block-kind shape | The model changed table column count, list depth, code-fence info string, or heading level. |
| `fragment_reparse` | Fragment reparse | The returned text does not reparse as the same block kind under GFM. |
| `inline` | Inline protection | A link or image destination changed, or a policy-gated code-span check failed. This is the layer that catches silently-corrupted links. |
| `provider` | Provider | The provider itself rejected or failed this unit rather than a validator finding a defect. |

Two further values exist in the wire vocabulary — `id_set` and
`full_reparse` — but no code path assigns them to a per-unit attempt. They
guard batch- and document-level invariants and surface as run failures, not
as a unit's `rejected_by`. Do not go hunting for them in this file.

### Two things that will mislead you here

**A cache hit is not a rejection.** An accepted hit is recorded as
`attempt_number: 0` with `rejected_by: null` and `rejection_reason: null`.
Older guidance told readers to look for a `"cache hit"` sentinel in
`rejection_reason`; that sentinel was deliberately removed precisely so
consumers stop reading a hit as a failure. A *rejected* cache hit keeps its
real rejection reason.

**A `unit_id` may name a row window, not a block.** When an oversize table is
split at packing time, each window is its own translation unit with its own
id, and the windows are reassembled into one table before regeneration. So a
failing `unit_id` can name a slice of a table rather than a block you can
find in the rendered output — and a table whose windows partly failed reports
`partially_translated` at the block level while *which* window fell back
appears only here, in the report.

`total_retries`, `total_fallbacks`, `provider_retries` and
`batch_schema_faults` are four separate counters against different budgets.
Do not sum them into one "retry count".

## 3. Match the stderr line to a known cause

| Symptom | Cause | Fix |
|---|---|---|
| `authentication: … does not have access to model …` (exit `6`) | The account tier cannot reach the configured model. | Set a model your tier has, or correct the dispatch surface if the model/endpoint pairing is wrong. |
| `output ceiling exhausted: …` (exit `6`), whose detail names `finish_reason: length` or `reason: max_output_tokens` | The batch's output ceiling was too small for what the model needed to write. Terminal by design — re-running unchanged reproduces it. | Raise `--target-output-tokens`; raise (never lower) `--output-expansion-factor` if you tuned it down. For a table, `--table-strategy row-window-first` splits it into header-carrying windows instead — this is already the shipped default, so check whether a custom profile turned it off. |
| `context window exceeded: …` (exit `6`) | The request exceeded the model's input window. | Lower `--target-input-tokens-per-batch` or `--max-units-per-batch`, or use a larger-window model. |
| `content filtered: …` / `model refused: …` (exit `7`) | The provider would not translate this content. Not a configuration fault — no flag changes the answer. | Skip the document, or isolate and reword the block being refused. Do not re-run unchanged. |
| `model output_text was not valid JSON` (exit `5`) | The model does not support strict structured outputs, wrote prose instead of the schema, or a proxy stripped the response format. | Use a structured-outputs-capable model; test against the provider directly to rule out a proxy. |
| `every translatable unit fell back to source (N/N)` (exit `3`) | Every unit exhausted validation retries. | Go to step 2 — this message alone does not say why. |

The full symptom-keyed list, including sync-freeze and slow-run diagnosis, is
in the project's `docs/Troubleshooting.md`.

## 4. Notice the advisory lines on a successful run

A run that exits `0` can still print `transync: note:` lines on stderr. They
report things publication decided or worked around — not failures. Read them
as information; a run is not broken because they appeared.

`--verbose` adds more: per-stage pipeline diagnostics, a validation tally,
and the auto-glossary outcome when that preflight ran. `--verbose` and
`--quiet` are mutually exclusive and conflict at parse time.

## 5. Verify a profile's compiled prompt out-of-band

If a glossary term or a constraint does not seem to be reaching the model,
render the prompt the run will actually send before spending an API call:

```rust
use transync::profile::{load_profile, render_prompt_body};

let profile = load_profile("profile.toml")?;
let compiled = render_prompt_body(&profile, "en", "ko");
println!("{}", compiled.prompt_body);
```

`render_prompt_body` is idempotent over its own output, so calling it twice
never doubles the appended sections — what you print is what a real run
compiles.

One limit worth knowing: this renders the profile's **whole** glossary. If
your entries are section-scoped, the per-section filtering happens later, in
the batcher, so what you print here is a superset of what any single batch
actually carries. See
[How glossary entries are resolved](../../../explanation/user/en/how-glossary-entries-are-resolved.md).

## Related

- [How to translate a document with the CLI](./translate-a-document-with-the-cli.md)
- [How to write a Profile TOML](./write-a-translation-profile.md)
- [How to reuse a warm cache across runs](./reuse-a-warm-cache-across-runs.md)
- [CLI flags](../../../reference/user/en/cli.md) — every flag, exit code, and environment variable
- [The `Translator` trait](../../../reference/developer/en/translator-trait.md) — the provider error variants and their stable codes
- [Why layered validation and bounded retry/fallback](../../../explanation/developer/en/validation-retry-fallback-model.md)
