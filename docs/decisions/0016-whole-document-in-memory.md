---
type: ADR
title: Whole-document-in-memory is inherent; CLI reads are admission-capped at the boundary
description: The pipeline is architecturally whole-document (parse → full AST → full-document reparse), so the whole-document-in-memory stance stands for ACCEPTED inputs. A limit+1 admission cap ships at the CLI boundary to reject absurd inputs before parse; it is boundary hygiene, not a peak-memory guard.
tags: [decision, ADR-0016]
status: active
---

# ADR: Whole-document-in-memory is inherent

## Context and Problem Statement

Found in Review 0008 (Issue R0008-0040, Severity: Medium) (review archived and removed). Location:
`crates/transync-cli/src/translate_cmd.rs` (`run`, `resolve_profile` —
`std::fs::read_to_string` on `--input`, `--profile`,
`--system-prompt-file`).

The reviewer flagged that the CLI reads input, profile, and prompt
files without metadata checks or streaming caps, so an accidental huge
file exhausts memory before batching or provider limits apply.

## Decision Drivers

* The pipeline is whole-document by design: intake builds the IR for the
  entire source in one pass (a full Comrak AST for Markdown, the
  `intake::html` scanner walk for HTML), regeneration splices exact source
  byte ranges, and validation layer 6 re-checks the *complete* regenerated
  document (`full_reparse`, or `full_rescan_html` for an HTML run).
  Streaming is not an option the architecture supports; a read-side cap
  would only relabel "OOM" as "file too large," not reduce peak memory
  for accepted inputs.
* That memory tracks document size *linearly* is a property each intake
  has to keep, not a consequence of being whole-document. The HTML intake
  keeps it under a nesting ceiling (DCR-0063): every emitted block's
  `ast_path` is as long as its nesting, so depth multiplies memory, and an
  `--input-format html` document holding more than 512 elements open at
  once on the scanner's stack is refused before the intake runs. Under
  the ceiling a block's path is at most 513 words, which bounds that
  factor without removing it.
* This is a local CLI reading files the operator chose on their own
  machine — there is no untrusted-uploader boundary. The failure mode
  is self-inflicted and immediately diagnosable.
* Every real document that should translate must fit in memory several
  times over (source + AST + batches + regenerated output); a cap tuned
  low enough to protect anything would reject legitimate large docs.

## Considered Options

1. Add a configurable max-input-size with a clear error.
2. Stream/chunk the input (architecture change).
3. Accept unbounded reads of operator-chosen local files; document that
   memory scales with document size.

## Decision Outcome

Adopt option 3 **for accepted inputs**: the pipeline holds the whole
document in memory by design, and no cap is tuned low enough to reduce peak
memory for documents that legitimately translate. Option 2 (streaming)
contradicts the full-document reparse contract that anchors correctness and
stays rejected.

Option 1 is accepted in a *narrow* form: a boundary admission cap that
rejects absurd inputs before parse, with the in-memory stance itself
unchanged for everything the cap admits. The reviewer's `limit + 1`
admission-control argument holds without changing the architecture: for any
input the cap admits, the pipeline still holds the whole document in memory
as **Decision Drivers** describes, so the cap is boundary hygiene, not a
peak-memory guard. This is a **scoped decision, not a settled absolute**.
Revisit trigger: transync running as a service or multi-tenant deployment
ingesting third-party uploads — that deployment needs request-level limits
*outside* the library anyway, and the CLI-boundary cap is not a substitute
for them.

### Implementation

`--max-input-bytes` (default 64 MiB) caps `--input`; a fixed 4 MiB constant
caps `--profile` / `--system-prompt-file`. Each is enforced by reading
`limit + 1` bytes via `Read::take` — metadata length is never trusted alone
— and exceeding a cap exits 2 (InputReadFailure) with a message naming the
limit (and, for input, the flag). Reading one byte past the limit and
erroring on the extra byte is the correct primitive because it never trusts
`metadata().len()` (which can lie, race, or grow). See `read_capped` and
`resolve_profile` in `crates/transync-cli/src/translate_cmd/input.rs`
(`translate_cmd::execute` applies the `--input` cap) and contracts.md §6.

`docs/Performance.md`'s "Memory" section reports coarse peak-RSS readings
(`/usr/bin/time -l` over stub-provider release runs, one machine) as
measured points and explicitly not as a scaling law. It does not predict
unmeasured sizes, does not say where an input at the `--max-input-bytes`
default would land, and does not locate the true OOM ceiling, so the third
consequence below stands as written. Read narrowly, that section is what
discharges the documentation half of option 3. The reason peak memory
tracks document size is argued in **Decision Drivers** from the architecture
itself, and that argument is self-contained: rely on it, not on the
measurements.

## Consequences

* Good, because no artificial ceiling rejects legitimate large documents:
  the default cap sits well above any realistic document, so the in-memory
  contract is untouched for real work.
* Good, because an absurd `--input` (a multi-GB file, a wrong-file
  mistake) now fails fast with a friendly, flag-named error instead of
  OOM-ing the process.
* Bad, because a document between the cap and the true OOM ceiling can
  still exhaust memory — the cap is boundary hygiene, not a peak-memory
  guard, and a service deployment still needs its own limits.

## History

The full text of each replaced passage is in
[`archive/adr-amendments.md`](archive/adr-amendments.md), under ADR-0016.

- 2026-07-11 (R0008-0040): option 3 adopted and recorded as "no change required" — CLI reads of operator-chosen files stayed uncapped.
- 2026-07-13 (EXT-2026-07 P1-7, DCR-0011): `limit + 1` admission caps shipped for `--input` and the auxiliary texts; the core stance held.
- 2026-08-05 (DCR-0019): `resolve_profile` and the capped reads moved to `translate_cmd/input.rs`; `run`'s body became `translate_cmd::execute`.
- 2026-08-07 (ti 038c54): the Performance.md memory-scaling citation was retracted as never true; Performance.md gained a measured "Memory" section.
