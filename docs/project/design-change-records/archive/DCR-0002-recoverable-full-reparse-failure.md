# DCR-0002: Post-regen full-reparse failure becomes recoverable

> Archived 2026-07-11. Reason: change fully absorbed — affected ADRs/contracts in sync, migration/follow-up landed; kept for audit.

- **Date:** 2026-05-04
- **Source:** Review 0004, Issue R0004-0001 (external consumer report from `resp-translator`) (review archived and removed)
- **Affected ADRs:** none directly; SCN-14 implementation slice (in `docs/project/implementation-slice-checklists.md`) is materially affected. `docs/architecture/architecture_investigation.md` and `docs/architecture/codebase_investigation.md` reference the prior "hard" behavior and need wording updates.

## What Changed

Before this change, `pipeline::run_pipeline` returned `TransyncError::Validation(_)` whenever the post-regeneration full-document reparse rejected the output (kind mismatch or top-level block count drift). The whole document failed; consumers had no per-block diagnostic and retries against a deterministic LLM produced byte-identical output → the same failure → same dead end.

The pipeline now performs **byte-offset attribution** of each reparsed top-level block back to a source block (via `regen::BlockOffsets`), identifies the source block(s) that own ≠ 1 reparsed top-level block (lost or over-produced), and applies the caller-selected policy:

- `FullReparseFailure::Hard` — original behavior; surface `Validation` error.
- `FullReparseFailure::FallbackPerBlock` — **new default**. Mark divergent source blocks as `FallbackStatus::FallbackSource`, re-regenerate, re-reparse once. Only return `Err` if the second reparse also fails.
- `FullReparseFailure::FallbackAll` — fall back every block to source bytes, skip the second reparse (the result is structurally identical to the source by construction).

Failure messages now include the source-side `BlockId`s flanking the divergence point so consumers can log/display the regression locus. A new `TransyncError::stable_code() -> &'static str` helper lets downstream code map errors to wire formats without enumerating variants.

## Why

The fail-hard behavior was defensible as a strict structural guard during MVP development but proved brittle in real consumer use. Per-unit fallback infrastructure (`FallbackStatus::FallbackSource`) already existed for in-pipeline rejections; extending it to the post-regen reparse closes the recovery gap without weakening the guard — the layered validators (schema → IDs → per-kind → fragment reparse) still gate translations going in, and the second reparse still gates the final document.

The knob preserves the old behavior under `Hard` for callers that prefer to surface failures rather than degrade silently.

## Affected Areas

- `crates/transync/src/lib.rs` — new `FullReparseFailure` enum; new `TranslateOptions.full_reparse_failure` field; default `FallbackPerBlock`.
- `crates/transync/src/pipeline.rs::finalize_regen_with_reparse_policy` — new helper encapsulating the regen + reparse + policy-driven fallback retry loop.
- `crates/transync/src/validate/full_reparse.rs::reparse_full` — signature now takes `&BlockOffsets`; returns `Result<(), ReparseFailure>` (was `Result<(), String>`); attributes divergence to specific source blocks via byte ranges; emits diagnostic `BlockId`s in the `reason` field.
- `crates/transync/src/error.rs::TransyncError::stable_code` — new helper for downstream consumers.
- `docs/architecture/architecture_investigation.md` (line ~18) — wording mentions "hard full-document reparse"; to update.
- `docs/architecture/codebase_investigation.md` (line ~56) — `run_pipeline` description mentions "hard full-document reparse"; to update.

## Migration / Follow-up

- Existing callers that relied on `Err(TransyncError::Validation(_))` to detect reparse failures will now see a `TranslationOutput` with the offending block(s) carrying `FallbackStatus::FallbackSource` instead. To preserve the old behavior set `opts.full_reparse_failure = FullReparseFailure::Hard`.
- The two architecture docs above need their "hard reparse" wording softened. Tracked here until the prose lands.
- The reviewer's original recommendation #3 (caller-controlled severity) is fully implemented; recommendation #1 (per-block fallback) is the new default; recommendation #2 (diagnostic detail) is the `BlockId`s in the failure reason and the `stable_code()` helper.
