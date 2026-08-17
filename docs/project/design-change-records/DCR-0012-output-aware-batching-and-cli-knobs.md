---
type: DCR
title: Output-aware batch packing, at-risk preflight, and five CLI batching flags
description: The batch packer gains a second cap — estimated output tokens — alongside the existing input cap, so a full batch cannot outgrow the provider output ceiling and trigger a whole-run abort (ADR-0017). A profile expansion-factor knob, an at-risk preflight warning, and five CLI batching flags ship with it. Breaking surface changes ride the still-open 0.2.0 window.
tags: [change, project-control, DCR-0012]
status: active
---

# DCR-0012: Output-aware batch packing + CLI batching knobs

- **Date:** 2026-07-27
- **Source:** DR-2026-07 design review (design item D1); owner decisions 2026-07-24
- **Affected ADRs:** docs/decisions/0017-batch-terminal-failures-abort-the-run.md (this is the prevention-side mitigation ADR-0017 relies on); no ADR reversed

## What Changed

Before D1, `batch::group_by_token_budget` packed batches by **input** tokens
only; `[batching].target_output_tokens` was a live *provider* ceiling
(`max_completion_tokens` / `max_output_tokens`) but had **no** effect on how
the batcher packed. A legitimately-full input batch translating into a
token-expensive target (KO/JA ≈ 1.3–2.0× source in o200k, plus the JSON
result envelope) could exceed the output ceiling with nobody misbehaving,
truncating the response and terminally aborting the whole run (ADR-0017).

- **Dual-cap packing.** `group_by_token_budget` now takes a `BatchBudget`
  (input target, unit cap, optional output ceiling, expansion factor, model,
  system prompt) and enforces **two** caps: the existing input-token budget
  *and* an estimated-output budget. A batch breaks before the next unit as
  soon as adding it would push **either** running total past its target
  (whichever binds first), or the unit cap is hit. The per-unit output
  estimate is:

  ```
  est_out(unit) = ceil(tokens(source_payload) × output_expansion_factor)
                + tokens(unit_id)                    // echoed byte-for-byte
                + PER_UNIT_OUTPUT_OVERHEAD_TOKENS     // = 40
  ```

  measured against `target_output_tokens − OUTPUT_ENVELOPE_RESERVE_TOKENS`
  (reserve `= 64` for the top-level result framing, floored at 1). Input and
  output estimates share one encoder pass per unit. Context hints and
  constraints are input-only (never echoed) and excluded from `est_out`.

- **Expansion factor + input-budget knob (profile).** `ProfileBatching`
  gains `output_expansion_factor: Option<f64>` (default
  `DEFAULT_OUTPUT_EXPANSION_FACTOR = 2.0`, a `pub const` in `batch.rs`) and
  `target_input_tokens_per_batch: Option<u32>` (the last packing knob the
  profile could not tune). Both are added to the `[batching]` known-keys list;
  a non-finite / non-positive factor gets a load warning and falls back to the
  default. `ProfileBatching` **drops `Eq`** (keeps `PartialEq`) because
  `Option<f64>` is not `Eq` — verified unused outside `profile.rs` and absent
  from `CacheKey`, so packing shape never affects content identity.

- **Unset ceiling disables everything, bit-identically.** When
  `[batching].target_output_tokens` is `None` (a custom profile with no
  `[batching]` section, or the CLI disable sentinel below), output-aware
  packing **and** the preflight are inert and packing is byte-for-byte the
  pre-D1 input-only behavior. This mirrors the provider wiring, which already
  omits the ceiling field when unset, and is pinned by an in-module regression
  oracle (`unset_ceiling_matches_input_only_packing`) that reproduces the old
  grouping exactly.

- **At-risk preflight (warning, never an abort).** After output-aware packing
  the only remaining over-ceiling shape is a single unit whose own `est_out`
  exceeds the target (it gets its own batch and may terminally abort at the
  provider). `batch::output_budget_warnings` flags every such unit as a typed
  `OutputBudgetWarning { unit_id, estimated_output_tokens, ceiling_tokens }`.
  `run_pipeline` computes these before dispatch, emits each via
  `tracing::warn!`, and records them on the new
  `ValidationReport.output_budget_warnings` field (serialized into
  `validation-report.json`). Because that success-path channel is unreachable
  on the very abort it predicts, `run_pipeline` also **annotates the terminal
  error**: when the aborting batch was flagged, it appends
  `"; preflight: <diagnosis>"` to the inner message of the terminal
  `TranslatorError` (variant and `stable_code()` preserved — enrichment, not
  reshaping), so the CLI's `translation failed: …` line names the block id,
  estimate vs ceiling, and remediation.

- **Five CLI batching flags** on `transync translate`, so tuning any batching
  knob no longer requires authoring a full profile TOML:

  | Flag | Effect |
  |---|---|
  | `--target-output-tokens <N>` | overlay `[batching].target_output_tokens`; **`0` = disable** (no ceiling → output-aware packing + preflight off) |
  | `--output-expansion-factor <F>` | overlay `[batching].output_expansion_factor` (finite, `> 0`) |
  | `--target-input-tokens-per-batch <N>` | overlay `[batching].target_input_tokens_per_batch` (`≥ 1`) |
  | `--max-units-per-batch <N>` | overlay `[batching].max_units_per_batch` (`≥ 1`) |
  | `--max-concurrent-batches <N>` | set `TranslateOptions::max_concurrent_batches` (`≥ 1`) |

  Precedence is **flag > profile > built-in default**, implemented as a single
  profile overlay (`apply_batching_overrides`) so there is exactly one
  resolution mechanism downstream. **All four profile-homed flags overlay onto
  the resolved profile** — including `--target-input-tokens-per-batch`, which
  the D1 design had originally routed onto `TranslateOptions`. Overlaying it
  too closes the *explicit-flag-equals-built-in-default sentinel hole*: because
  `build_batches` resolves a caller `TranslateOptions` value that equals the
  built-in default as indistinguishable from "unset," a flag whose value
  happened to equal the default would otherwise have silently lost to the
  profile. Only `--max-concurrent-batches` writes onto `TranslateOptions`:
  concurrency is a deployment/runtime property (rate limits, machine), not part
  of a shareable translation contract, so it deliberately gets no profile home.

- **`--max-split-retries` excluded.** `[batching].max_split_retries` is
  advisory for the not-yet-landed provider-side splitter (STUB-017); a flag for
  a knob with zero live effect would lie in `--help`.

## Why

ADR-0017 keeps whole-run abort as the terminal behavior for batch-terminal
provider failures and rejects both a per-batch fallback rung and a policy
flag. That decision is only defensible if the dominant terminal cause —
output-token truncation on a legitimately-full batch — is *prevented* rather
than merely caught. Modeling response size as a second packing cap does
exactly that; the preflight and terminal-error annotation make the residual
single-oversize-unit case self-diagnosing instead of an opaque provider abort.
The CLI flags close the motivating complaint that tuning any batching knob
required authoring a whole profile TOML.

The expansion-factor default is `2.0` because the failure mode is asymmetric
under abort-all semantics: **underestimating splits too little and aborts the
whole run**, while overestimating only produces more, smaller batches (linear
request overhead, zero quality loss). The default therefore covers the worst
*legitimate* case (KO/JA ≈ 2.0×). Latin-target users who find the resulting
~1.5× batch-count increase wasteful set `--output-expansion-factor 1.2` (or a
profile value) and recover the pre-D1 shape.

## Affected Areas

- `crates/transync-core/src/batch.rs` — `BatchBudget`, dual-cap
  `group_by_token_budget`, `DEFAULT_OUTPUT_EXPANSION_FACTOR` (pub) +
  `PER_UNIT_OUTPUT_OVERHEAD_TOKENS` + `OUTPUT_ENVELOPE_RESERVE_TOKENS`,
  `estimate_unit_output_tokens`, `OutputBudgetWarning`,
  `output_budget_warnings`, `resolve_expansion_factor`
- `crates/transync-core/src/unit.rs` — `build_batches` resolves both budgets
  from the profile and constructs `BatchBudget`
- `crates/transync-core/src/profile.rs` — two new `[batching]` fields, `Eq`
  dropped, known-keys + load-time factor sanity check
- `crates/transync-core/src/pipeline.rs` — preflight computation,
  `tracing::warn!`, report population, terminal-error annotation
- `crates/transync-core/src/validate.rs` — `ValidationReport.output_budget_warnings`
- `crates/transync-cli/src/translate_cmd.rs` — five `#[arg]` fields,
  `apply_batching_overrides`, direct `--max-concurrent-batches` assignment,
  success-path warning loop with the remediation-flag suffix
- `crates/{transync-core,transync-cli}/profiles/default.toml` — explicit
  `output_expansion_factor = 2.0`
- `docs/architecture/contracts.md` §2 (new `[batching]` keys), §5 (dual-cap
  note), §6 (five flags + 0-sentinel)

## Migration / Follow-up

Breaking surface changes ride the **still-open 0.2.0 window** (only 0.1.0 is
released):

- `group_by_token_budget` takes a `BatchBudget` instead of positional args
  (sole caller is internal).
- `ProfileBatching` gains two `Option` fields (TOML-additive via
  `#[serde(default)]`; Rust-literal construction breaks) and **loses `Eq`**.
- `ValidationReport` gains one `Serialize`-only field (constructed via
  `Default`).

`TranslateOptions` is deliberately **unchanged** — the most-constructed public
struct keeps its shape. Default batch shapes change for every run using the
default profile (the output cap binds before the 6000-token input cap at
factor 2.0): more, smaller provider requests. Any test asserting an exact
batch count must be re-checked; lower-bound assertions (`scn_10`) still pass.

Deferred refinement (noted, out of scope): per-kind expansion factors (code
blocks are usually `preserved` ≈ 1.0×). One global factor keeps the knob count
at one; revisit only with evidence that code-heavy docs over-split painfully.

### Note (2026-08-06) — one of the two profile paths above no longer exists

*Affected Areas* lists `crates/{transync-core,transync-cli}/profiles/default.toml`
as the place the explicit `output_expansion_factor = 2.0` landed. Only the
`transync-core` half was ever `include_str!`d, and the CLI-side file — which no
source, test or script referenced — was deleted on 2026-08-06 (Review-0001
finding R0001-0041) after its comments had already drifted from the core copy.
The key this record added is unchanged and still lives in the surviving file;
read `crates/transync-core/profiles/default.toml` for it.

## Amendment (2026-08-08) — a ceiling that cannot hold the envelope is now refused at the loader

Review-0003 finding **R0003-0034**. This record's packing formula measures each
unit against `target_output_tokens − OUTPUT_ENVELOPE_RESERVE_TOKENS`, "floored
at 1". That floor turned out to be doing more than keeping the arithmetic
non-zero: a nonzero ceiling at or below the 64-token reserve produced an
effective per-batch output target of exactly one token, so every unit was
flagged by the preflight and the request went out carrying a
`max_completion_tokens` / `max_output_tokens` too small to hold even the
result framing. The run was loudly diagnosable but failed at the provider
rather than at the profile.

`profile::normalize_batching` now gives such a ceiling the same warn-and-ignore
treatment `Some(0)` has had since R0001-0016: the value is dropped with a
path-qualified warning naming the 64-token floor, and the key resolves as if it
had been omitted. The supported range of `[batching].target_output_tokens` is
therefore `>= 65`, not `>= 1` (`contracts.md` §2 and the Profile Cookbook were
corrected in the same commit). Nothing about the packing formula, the reserve,
or the preflight changes; the CLI's `--target-output-tokens 0` disable sentinel
is unaffected, since it reaches the profile as `None` before this gate.
