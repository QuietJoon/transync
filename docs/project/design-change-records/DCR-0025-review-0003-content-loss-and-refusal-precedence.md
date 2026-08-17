---
type: DCR
title: Review 0003 hardening — content loss closed at four seams, and a refusal outranks the text beside it
description: Four paths that could silently lose or invent document content were closed, the Responses surface learned the refusal-precedence rule its Chat sibling already had, and the renderer now refuses a byte range it cannot slice.
tags: [change, project-control, DCR-0025]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-09T00:00:00Z
status: stable
---

# DCR-0025: Review 0003 hardening — content loss closed at four seams, and a refusal outranks the text beside it

- **Date:** 2026-08-09
- **Source:** Review 0003, issues R0003-0042, R0003-0043, R0003-0060, R0003-0078, R0003-0006, R0003-0019 (review archived and removed)
- **Affected ADRs:** `docs/decisions/0006-renderer-output-shape.md` (updated), `docs/decisions/0018-html-content-translation-via-segment-extraction.md` (updated); `docs/project/design-change-records/DCR-0016-html-content-translation.md`, `DCR-0020-track-c-wasm-demo.md` and `DCR-0012-output-aware-batching-and-cli-knobs.md` each carry a dated note from the same pass

## What Changed

**Whitespace was not emptiness.** The raw-HTML segment engine skipped a
segment it judged empty, but the emptiness test missed whitespace-only
content, so a segment carrying *visible* text could be dropped past every
validation layer — the one path in this review that silently loses document
content the user can see. The test now treats whitespace-only as non-empty.
The counter-case that would have made this risky (a whitespace-only *source*
segment) cannot occur by construction, so the fix carries no false-positive
cost.

**A code fence could gain an info string it never had.** Regeneration could
turn an infoless fence into one carrying an invented language tag — the
`None -> Some` direction. Architectural invariant 2 says the LLM must not
alter code fence or language metadata, and no record permitted that
direction; the `Some -> Some` verbatim rule was already correct and is
unchanged.

**The renderer refuses a byte range it cannot slice.** `R0002-0054` drew the
refuse-vs-degrade line at map *shape*; a corrupt or reversed `target_range`
was still coerced into a silently wrong slice. That coercion is gone. This
extends the direction DCR-0022 recorded when renderer construction became
fallible, and it closed the wasm demo's shadow of the same defect for free.

**A refusal outranks the text beside it.** The Responses surface accepted
output text appearing *before* a refusal in the same envelope — the unfixed
mirror of the Chat-side rule that landed as R0002-0041. The envelope is now
read whole, and any refusal is terminal before text is accepted from either
the top-level shortcut or the nested segments. Classification goes through
the typed `TranslatorError` variants introduced by ticket `1a85f3`, so this
did not reintroduce a string-carrying `Other`.

**Two balancer states the HTML tokenizer did not have.** Digit-leading tag
names and CDATA sections were mis-scanned; the first could, through the
layer-3 inventory, cause a *good* translation to be dropped — the same
phantom-failure pattern R0002-0020 had.

## Why

Every item above is one seam quietly disagreeing with a promise the design
already made: that anchors survive, that structure is preserved verbatim,
that a guard which cannot honor its input says so. None of them needed a new
principle — they needed the existing one applied where it had not been.

## Affected Areas

- `crates/transync-syntax/src/htmlseg.rs`, `regen.rs`, `render.rs`
- `crates/transync-openai/src/client/responses.rs`
- `crates/transync-wasm/src/engine.rs`, `web/js/wasm-demo.js`
- `crates/transync-core` batching and report capping
- `scripts/build-wasm.sh` (staged publish — artifacts are judged before they are installed)

## Migration / Follow-up

- Breaking, and rides the open **v0.4.0** window alongside the provider error
  taxonomy (DCR-0023) and run cancellation (DCR-0024). One operator-visible
  break in this pass: a padded `--model` / `TRANSYNC_OPENAI_MODEL` now fails
  at construction instead of reaching the provider.
- Four findings were routed to tracking rather than fixed and are registered
  in `docs/project/open-issues.md`: provider-returned payloads bypass
  `parser::intake`'s depth guard at five call sites (OI-0037, with its test
  gap), and the in-memory cache has no capacity bound (folded into ticket
  `f12b8b`'s commissioned design).
- `reviews/0003.patch` was deliberately **not** applied wholesale: its
  `R0003-0002` half implements the alignment map's source/target indirection
  that ticket `d3acc3` reserved for a future divergence revision, so applying
  it would have reversed a recorded decision.
