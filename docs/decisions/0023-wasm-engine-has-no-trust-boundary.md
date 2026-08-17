---
type: ADR
title: The wasm engine has no trust boundary — its caller supplies both sides
description: transync-wasm is a stateless function whose caller provides the source and the map together, so it validates neither; the gates live in the demo, and this record exists because three consecutive review rounds have argued otherwise.
tags: [decision, ADR-0023]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-13T00:00:00Z
status: stable
---

# ADR: The wasm engine has no trust boundary — its caller supplies both sides

## Context and Problem Statement

Found, rejected, and found again:

| Round | Findings | Where the rejection was recorded |
|---|---|---|
| Review 0002 (review archived and removed) | R0002-0009, R0002-0012, R0002-0013 | DCR-0022, "ADR-0019's posture held" |
| Review 0003 (review archived and removed) | R0003-0061, R0003-0062, R0003-0079, R0003-0080 | DCR-0025 |
| Review 0004 (review archived and removed) | R0004-0091 | this record |

Location: `crates/transync-wasm/src/engine.rs` (`render_pair_impl`,
`rebuild_impl`).

Eight findings across three independent rounds have made the same argument in
different words: the engine deserializes an `AlignmentMap` and acts on it
without checking schema version, document identity, row coverage, row ordering,
duplicate ids, or payload/status agreement — so a stale, foreign or crafted map
reaches the renderer.

Each round the mechanism was confirmed and the conclusion rejected. The
rejection was recorded both times, but in **DCRs about other changes**, where a
reviewer looking for the engine's contract does not find it. That is why the
finding keeps arriving: the answer existed and was not where the question gets
asked.

## Decision Drivers

* **What a trust boundary is.** It is a point where data crosses from a party
  with one authority to a party with another. `render_pair_impl` takes the
  source Markdown *and* the alignment map from the same caller, in the same
  call, in the same address space. There is no second party.
* **What the engine is.** ADR-0019 and DCR-0020 made `transync-wasm` a
  stateless wrapper over `transync-syntax`, deliberately thin, with a
  JSON-string boundary and no state between calls. Validation inside it would
  be validation of the caller against itself.
* **Where the gates actually are.** `web/js/wasm-demo.js` validates before it
  calls: schema range, row shape, duplicate anchors, non-array `blocks`,
  ID-identity conformance (ticket `d3acc3`). The demo is the component with a
  boundary — it receives a bundle it did not produce.
* **What it would cost.** Every check moved into the engine is paid by every
  caller on every call, in a wasm module under a size budget
  (`scripts/build-wasm.sh`), to defend a caller against data it chose itself.

## Considered Options

1. **State the posture as an ADR and keep it** — the engine validates nothing;
   the demo gates; findings against this resolve to the record.
2. **Validate in the engine** — the recommendation all eight findings make.
   Duplicates the demo's checks, grows the module, and defends nobody from
   anybody.
3. **Validate in `transync-syntax`** — pushes the cost into the shared renderer
   and into the CLI path, which has no untrusted map at all: the CLI produces
   the map it renders.

## Decision Outcome

**Option 1.** The wasm engine has **no trust boundary**. Its caller supplies
both the source and the map, so it validates neither; the gates live in the
demo, which is the component that receives artifacts it did not produce.

This is not a new decision — it is ADR-0019 and DCR-0020's posture, restated
where the question is asked. A finding that the engine trusts its input is,
from this record forward, a correct description of a deliberate design.

Status: Implemented (the decision is the record; no code changed).

### Implementation

No code change.

The line this record does **not** cross, because two rounds found real defects
just past it:

- **The renderer is different, and it was fixed.** `transync-syntax`'s renderer
  *does* promise per-row anchor survival (contracts.md §4a), so R0002-0010 and
  R0002-0011 — duplicate rows silently overwriting, missing rows silently
  omitting blocks — were accepted, and renderer construction became fallible
  (DCR-0022). A promise the code makes is enforceable; trust the caller places
  in itself is not.
- **A gate that lies is a defect.** R0003-0014 found
  `check_top_level_structure` comparing only block counts while its diagnostic
  promised same-kind structure, and that was fixed (DCR-0025). The engine may
  decline to validate; it may not claim to have validated.

So the test for a future finding in this area is not "does the engine check
this?" but "does the engine or its documentation *say* it checks this?" If yes,
it is a defect. If no, it is this record.

## Consequences

* Good, because the answer now lives where the question is asked. Three rounds
  have each spent a verification pass re-deriving it from two DCRs about other
  subjects.
* Good, because it keeps the wasm module small — a real constraint with a
  measured budget, not a preference.
* Bad, because a direct library consumer of `transync-wasm` that is *not* the
  demo inherits the demo's validation duty and has no help doing it. That is
  the honest cost, and if such a consumer appears the right answer is a
  documented validation helper, not validation inside the engine.
* Bad, because "validates nothing" reads as carelessness out of context. The
  two limits in *Implementation* are the context and should travel with any
  restatement.
