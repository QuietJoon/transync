---
type: ADR
title: Reject replacing the tokenizer-initialization panic with a fallback
description: Both tiktoken vocabularies are compile-time bundled assets; failure to load them is a broken-build condition with no useful runtime recovery.
tags: [decision, ADR-0010]
status: active
---

# ADR: Reject replacing the tokenizer-initialization panic with a fallback

## Context and Problem Statement

Found in Review 0006 (Issue R0006-0028, Severity: Medium) (review archived and removed).
Location: `crates/transync-core/src/batch.rs` (`encoder_for`)

The reviewer flagged that `encoder_for` panics in library code when both
bundled tiktoken encoders fail to load.

## Decision Drivers

* The vocabularies are compile-time bundled assets of `tiktoken-rs`; a
  load failure means the build artifact itself is broken.
* A prior "one-byte-per-token" pseudo-estimator fallback was removed
  deliberately (commit 49d19ee, the day before Review 0006): it silently
  produced absurd batch sizes, which is worse than failing loudly.
* Batching cannot proceed without a tokenizer; there is no degraded mode
  that preserves the token-budget contract.

## Considered Options

1. Return `Result` from `encoder_for` and thread errors through batching
   (rejected — every caller can only abort anyway; adds error plumbing for
   an unreachable-in-practice condition).
2. Fall back to a fake estimator (rejected — previously removed on purpose).
3. Keep the documented panic with a clear "build asset is broken" message.

## Decision Outcome

REJECT: We keep option 3, reaffirming the 49d19ee decision.

Status: No change required.

### Implementation

None. The panic message already names the condition and the remedy.

## Consequences

* Good, because a corrupt build fails loudly at first use instead of
  mis-batching silently.
* Bad, because a library consumer cannot intercept this specific failure —
  acceptable for a broken-artifact condition.
