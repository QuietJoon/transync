---
type: ADR
title: Whole-document-in-memory is inherent; CLI reads are admission-capped at the boundary
description: The pipeline is architecturally whole-document (parse → full AST → full-document reparse), so the whole-document-in-memory stance stands for ACCEPTED inputs. Amended 2026-07-13 (external review P1-7) — a limit+1 admission cap ships at the CLI boundary to reject absurd inputs before parse; it is boundary hygiene, not a peak-memory guard.
tags: [decision, ADR-0016]
status: active
---

# ADR: Whole-document-in-memory is inherent

> **Amended 2026-07-13 (external review P1-7).** The original framing —
> "CLI input reads are not size-capped" — is superseded at the boundary: a
> `limit + 1` admission cap now ships (`--max-input-bytes`, default 64 MiB,
> for `--input`; a fixed 4 MiB cap for `--profile` / `--system-prompt-file`).
> This is a scoped decision, not a settled absolute — see **Amendment** below.
> The whole-document-in-memory core stance still holds for ACCEPTED inputs.

## Context and Problem Statement

Found in Review 0008 (Issue R0008-0040, Severity: Medium) (review archived and removed). Location:
`crates/transync-cli/src/translate_cmd.rs` (`run`, `resolve_profile` —
`std::fs::read_to_string` on `--input`, `--profile`,
`--system-prompt-file`).

> **Path note (2026-08-05, DCR-0019).** The decision is unchanged; the code
> moved. `resolve_profile` and the capped reads now live in
> `crates/transync-cli/src/translate_cmd/input.rs`, and `run`'s body became
> `translate_cmd::execute`. The caps this ADR sets are enforced in the same
> place they always were, one module deeper.

The reviewer flagged that the CLI reads input, profile, and prompt
files without metadata checks or streaming caps, so an accidental huge
file exhausts memory before batching or provider limits apply.

## Decision Drivers

* The pipeline is whole-document by design: `parse()` builds a full
  Comrak AST over the entire source, regeneration splices exact source
  byte ranges, and validation layer 5 reparses the *complete* document.
  Streaming is not an option the architecture supports; a read-side cap
  would only relabel "OOM" as "file too large," not reduce peak memory
  for accepted inputs.
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

Originally recorded as "no change required" — that framing is now scoped by
the 2026-07-13 amendment: option 1 is accepted in a *narrow* form (a
boundary admission cap that rejects absurd inputs before parse), while the
in-memory stance itself is unchanged for everything the cap admits. This is
a **scoped decision, not a settled absolute**. Revisit trigger: transync
running as a service or multi-tenant deployment ingesting third-party
uploads — that deployment needs request-level limits *outside* the library
anyway, and the CLI-boundary cap is not a substitute for them.

### Implementation

`--max-input-bytes` (default 64 MiB) caps `--input`; a fixed 4 MiB constant
caps `--profile` / `--system-prompt-file`. Each is enforced by reading
`limit + 1` bytes via `Read::take` — metadata length is never trusted alone
— and exceeding a cap exits 2 (InputReadFailure) with a message naming the
limit (and, for input, the flag). See `crates/transync-cli/src/translate_cmd.rs`
(`read_capped`) and contracts.md §6. `docs/Performance.md` describes memory
scaling with document size for accepted inputs.

> **Correction (2026-08-07, ti `038c54`).** The last sentence above was never
> true. `docs/Performance.md` has been a wall-clock tuning guide since it was
> written and said nothing about memory consumption at any point in its history,
> so a reader sent there for the scaling curve found batching dials and provider
> latency instead. Nothing in this ADR rests on that forward reference: the
> reason peak memory tracks document size is argued in **Decision Drivers**
> above from the architecture itself — full Comrak AST, exact-source-range
> splicing, full-document reparse in validation layer 5 — and that argument is
> self-contained. Rely on it, not on the citation.
>
> As of this note `docs/Performance.md` does carry a "Memory" section, so the
> reference now resolves — but it resolves to something narrower than the
> original sentence promised. It is six coarse peak-RSS readings
> (`/usr/bin/time -l` over a stub-provider release run, one machine), reported
> as measured points and explicitly not as a scaling law. It does not predict
> unmeasured sizes, does not say where an input at the `--max-input-bytes`
> default would land, and does not locate the true OOM ceiling. The third
> consequence below therefore stands exactly as written. Read narrowly, that
> section is also the first thing to discharge the documentation half of
> option 3 under **Considered Options** — the half this sentence had been
> claiming as already done.

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

## Amendment (2026-07-13, external review P1-7)

The reviewer's `limit + 1` admission-control argument was **accepted**:
reading one byte past the limit and erroring on the extra byte is the
correct primitive because it never trusts `metadata().len()` (which can lie,
race, or grow). CLI-boundary caps shipped accordingly (`--max-input-bytes`
for `--input`, a fixed 4 MiB cap for the auxiliary text inputs). What did
**not** change is the architecture: for any input the cap admits, the
pipeline still parses a full Comrak AST, splices exact source byte ranges,
and reparses the *complete* regenerated document. The record is therefore
reframed from "settled: no cap" to "scoped: boundary caps ship, core
stance holds for accepted inputs, revisit on service/multi-tenant
deployment."
