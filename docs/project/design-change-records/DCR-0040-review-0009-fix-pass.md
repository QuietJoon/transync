---
type: DCR
title: Review 0009's fix pass changes three shipped behaviours, one of them a breaking API removal
description: The splice's two-pass agreement becomes a real error instead of a debug assertion, the serve path gains a response deadline, and OpenAI's unreachable RateLimited variant is removed from a published enum under the open v0.5.0 window.
tags: [change, project-control, DCR-0040]
generated:
  by: claude-code/claude-opus-5
  at: 2026-08-26T15:30:00Z
status: stable
---

# DCR-0040: Review 0009's fix pass changes three shipped behaviours

- **Date:** 2026-08-26
- **Source:** Review 0009 (`reviews/reviewed/0009.md`), findings R0009-0066, R0009-0085, R0009-0001
- **Affected ADRs:** `docs/decisions/0002-http-free-core-with-translator-trait.md` (updated — it is where `TranslatorError` is named, per DCR-0023's own pairing; the type is declared in `transync-core`'s `llm` module)
- **Affected DCRs:** `docs/project/design-change-records/DCR-0023-provider-error-taxonomy.md` (the record this narrows)
- **Commits:** `8d4efda` (`fix!`), `69701cd`, `88964df`

**Numbering note.** This record takes **DCR-0040**, not the next free number.
`DCR-0032`–`DCR-0039` are **reserved** by the eight HTML→HTML waves
(`docs/superpowers/plans/2026-08-20-html-wave*.md`), several of which have not
run. Taking "the next free number" would have collided with an unrun wave's
paperwork — the race the wave-2 plan warns about in its own Task 8.

## What Changed

### 1. A debug-only invariant became a real error (R0009-0066)

`transync_html::splice` runs two passes over a fragment's text nodes: phase A
collects them, phase B replaces them. That the two passes see the **same** nodes
was asserted with `debug_assert`.

In release the assertion is compiled out, and the failure mode is **silent**:
`actions.get(idx)` answers `None` past the end, so a short phase B drops the
tail of a translation and a long one shifts every later replacement onto the
wrong node — both returning `Ok`. A user would see a partially-translated or
scrambled HTML block with no error anywhere in the run.

It is now a real `Err`, riding the splice-failure degrade path the segment-count
mismatch already used: `validate` falls the block back to source and `regen`
splices the source bytes. **Invariant 6 (retry then fallback) already covered
this shape** — the defect was that one path into it was unreachable in the
profile users actually run.

### 2. `serve`'s response half gained a deadline (R0009-0001)

Only the head read was bounded. Every response write awaited the socket bare, so
a peer that completed its head and stopped reading held one of `MAX_IN_FLIGHT`
(128) connection slots for the process's lifetime.

`serve` now splits into a head phase and a `respond` phase, the second under
`RESPONSE_TIMEOUT = 60s`, with the 408/431 refusals moved inside it — they are
responses like any other. A **total** deadline was chosen over a write-idle one
because a write-idle deadline is defeated by a peer reading one byte per
interval.

**The trade is recorded at the constant:** a legitimate transfer slower than 60s
is now cut off. That is only reachable through a non-loopback `--bind`, which
`contracts.md` already scopes as a trusted-network convenience.

### 3. BREAKING: `transync-openai`'s `ProviderError::RateLimited` removed (R0009-0085)

The variant was unreachable — no code path constructed it — and the sibling
Anthropic adapter already shows the intended end state. `ProviderError` is `pub`
in a published crate with **no `#[non_exhaustive]`**, so removing a variant is a
breaking change to a published API.

**This is permitted because the v0.5.0 window is open**, not because the variant
was dead. `ff788f7` set the workspace to `0.5.0-dev` and opened the sanctioned
window; `8d4efda` is marked `fix!` accordingly. Had the window been closed, the
correct outcome would have been to leave the variant and record it.

## Why

All three are the same shape: **a guarantee that existed in one configuration
and not in the one that ships.** A `debug_assert` that vanishes in release, a
timeout on one half of a connection, and an error variant that documents a
capability nothing can produce. Review 0009 rated the first Medium and the third
Low; the verification pass found the first to be the most consequential fix in
the round.

## Affected Areas

- `crates/transync-html/src/lib.rs` — `splice`'s phase agreement
- `crates/transync-cli/src/serve_cmd/conn.rs` — the head/respond split
- `crates/transync-openai/src/error.rs` — the removed variant
- `CHANGELOG.md` — owes the breaking-change entry under the v0.5.0 window

## Migration / Follow-up

- **Consumers matching `ProviderError` exhaustively** must drop the
  `RateLimited` arm. There is no behavioural replacement: nothing produced it.
- A splice-agreement failure now surfaces as a per-block fallback with
  `fallback_status: fallback_source` on the alignment map, where it previously
  produced silent corruption. Consumers reading that field see one more reason
  it can be set.
- `contracts.md` §4a's probe description was corrected in the same pass
  (R0009-0024): `mountSync` now reads `getComputedStyle(pane).position` rather
  than `offsetParent` on a representative anchor. The recorded residual — a
  positioned wrapper around a *later* anchor while the pane is correct — stays
  open deliberately; closing it would break the section's own "one property read
  per pane at mount, never per frame" budget and is therefore a contract change.
- ADR-0002 gains a line recording the rule the removal leaves: **an adapter may
  name any error its transport can actually produce, and may not name one it
  cannot.** That is deliberately *narrower* than "keep the adapters' taxonomies
  identical" — the identical-taxonomies reading would contradict the ADR's own
  premise that each adapter owns its mapping. `RateLimited` went because
  `transync-openai` constructed it nowhere, not because `transync-anthropic`
  lacked it.
