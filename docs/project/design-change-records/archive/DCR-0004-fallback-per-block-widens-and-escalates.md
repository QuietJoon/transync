# DCR-0004: `FallbackPerBlock` widens to neighbors and auto-escalates

> Archived 2026-07-11. Reason: change fully absorbed — affected ADRs/contracts in sync, migration/follow-up landed; kept for audit.

- **Date:** 2026-05-05
- **Source:** External consumer report from `resp-translator`, follow-up to R0004-0001 (review archived and removed)
- **Affected ADRs:** none. Extends DCR-0002.

## What Changed

`FullReparseFailure::FallbackPerBlock` was a two-stage policy: try byte-offset attribution → mark divergent block(s) as fallback → re-regen + re-reparse → return `Err` if the second reparse still fails.

It is now a **three-stage** policy:

1. **Stage 1 — minimal.** Same as before: fall back exactly the block(s) byte-offset attribution flagged.
2. **Stage 2 — widen radius (NEW).** If stage 1's reparse still fails, also fall back the immediate top-level neighbors (preceding + following block) of every previously-flagged block. Re-regen + re-reparse.
3. **Stage 3 — auto-escalate (NEW).** If stage 2's reparse also fails, fall back every block to source bytes. By construction the regenerated document is the source, so the structural shape always matches. Returns `Ok`.

Each stage emits a `tracing::warn` at `transync::pipeline` so consumers can see which stage recovered or escalated.

## Why

`resp-translator` reproduced four distinct failures, three of which sat at a list-item → paragraph boundary. The pattern: an LLM translates a list item with content that, when re-emitted into the document, makes the reparser see a fenced/indented code block at the seam. Byte-offset attribution maps the hallucinated reparsed block to whichever source block owns its starting byte — typically the *paragraph after the list*, not the list item that produced the contamination. Stage 1 falls back the wrong block, leaves the actual culprit's translation intact, and the second reparse repeats the failure.

Widening to immediate neighbors covers both sides of every flagged seam, catching boundary-contamination cases. Auto-escalation removes the "translation totally failed, nothing the consumer can do" dead-end the original R0004-0001 report described — `FallbackPerBlock` now never returns `Err` for documents that parse cleanly. Consumers who want to surface failures explicitly stay on `FullReparseFailure::Hard`.

## Affected Areas

- `crates/transync/src/pipeline.rs` — `finalize_regen_with_reparse_policy`'s `FallbackPerBlock` arm runs the three-stage cascade; new `widen_to_neighbors` helper.
- DCR-0002 still describes the *intent* (recoverable full-reparse failure); this DCR captures the *implementation refinement* that made it actually recover the cases R0004-0001 said it should.

## Migration / Follow-up

- `FullReparseFailure::FallbackPerBlock` no longer returns `Err`. Callers who treated `Err(Validation(_))` as a "translation totally failed" signal must switch to `Hard` to preserve that signal, or check `validation_summary.fallback_source` against `total_units` to detect the auto-escalation case.
- The escalation log line (`escalating to FallbackAll (stage 3)`) is observable via `tracing` for consumers that want metric-style visibility.
