---
type: DCR
title: Review 0004 hardening — a credential stops crossing origins, a server names the authority it answers for, and section identity becomes canonical
description: Both provider crates refuse redirects and userinfo, the static server requires a Host it recognises, section and glossary identity are NFC-normalised (re-keying the cache once), and compaction earns DCR-0028's never-a-hybrid promise.
tags: [change, project-control, DCR-0030]
generated:
  by: claude-code/claude-opus-5[1m]
  at: 2026-08-13T00:00:00Z
status: stable
---

# DCR-0030: Review 0004 hardening — a credential stops crossing origins, a server names the authority it answers for, and section identity becomes canonical

- **Date:** 2026-08-13
- **Source:** Review 0004, issues R0004-0001, R0004-0002, R0004-0022, R0004-0023, R0004-0028, R0004-0080 (review archived and removed)
- **Affected ADRs:** `docs/decisions/0022-local-user-threat-model.md` (new in this session), `docs/decisions/0023-wasm-engine-has-no-trust-boundary.md` (new in this session); `DCR-0028` §4's durability claim is made true by the compaction change below, and `DCR-0029`'s provider contract gains the redirect and userinfo rules

## What Changed

**A provider credential no longer crosses an origin.** Both adapters now build
their authenticated client with `redirect::Policy::none()`. The defect was an
**asymmetry**, not a single crate's bug: `transync-openai` sends the credential
via `bearer_auth`, which sets `Authorization` — one of the four headers reqwest
strips on a cross-origin redirect — while `transync-anthropic` sends a custom
`x-api-key`, which reqwest does not strip. A compromised or misconfigured
endpoint answering a 3xx could therefore receive the Anthropic key together with
document content. The fix was applied to both crates so the two postures agree
by construction rather than by coincidence.

**The terminal/transient classification widened, then narrowed again.** With
redirects refused, *every* 3xx now reaches the status classifier rather than
only the unfollowable ones, and all of them were being treated as retryable
`Network`; they are now terminal. Builder and redirect failures joined them. One
arm was then **removed** on evidence: `is_request()` had been added to the
terminal set, but reqwest stamps `Kind::Request` on every hyper error, making it
a union of transient faults (incomplete message, reset after send, h2 GOAWAY) —
so in-flight socket failures returned to the retryable catch-all, pinned by a
loopback server that drains a request and drops the socket.

**A base URL carrying userinfo is refused** by both crates' checked
constructors. Breaking; the unchecked `new()` keeps its documented
validates-nothing contract.

**The static server requires a `Host` it answers for.** `transync serve`
ignored `Host` entirely, so a hostile page could reach the loopback-bound server
by DNS rebinding and read the served bundle. A missing `Host`, duplicate `Host`
headers, and a `Host` naming another authority are now all refused. Ticket
`b791d6` scoped out "non-loopback deployment hardening"; rebinding is
specifically a *loopback-server* attack, so that exclusion never covered it.

**Section and glossary identity are NFC-normalised.** Identity was derived with
trim plus lowercase, so canonically-equivalent strings differing only in
composition were treated as different sections or different terms. This feeds
cohort keys and compiled prompts, and therefore **cache identity**: affected
entries re-key once, exactly as the `input_mode` axis addition did.

**Compaction earns DCR-0028 §4's promise.** That record states the log is never
left a hybrid of old and new records; without a sync at compaction the claim was
not true under power loss. One `sync_all` plus a parent-directory fsync now runs
at compaction only — which does not disturb §5's per-write no-fsync decision,
whose argument is per-unit write cost.

## Why

Five of the six are the same shape: a promise the system already made — a
credential stays with its origin, a classification is deterministic, a log is
never a hybrid, an identity is canonical — that the code did not keep. The
sixth, `Host` validation, closes the one path in this review that an attacker
reaches without local access.

## Affected Areas

- `crates/transync-anthropic/src/lib.rs`, `client/classify.rs`, `messages.rs`
- `crates/transync-openai/src/lib.rs`, `client/classify.rs`
- `crates/transync-cli/src/serve/` (host policy, connection handling)
- `crates/transync-core` section/glossary identity and the input-budget preflight
- `crates/transync-core/src/cache/disk.rs`
- `docs/architecture/contracts.md` §6, §7, §8

## Migration / Follow-up

- Breaking, and rides the open **v0.4.0** window: a base URL with userinfo now
  fails at construction, and a provider endpoint that answers with a redirect is
  no longer followed. Neither sibling consumer
  (`resp-translator`, `dynwebserver`) configures either, and both compile
  against this tree unchanged.
- **The cache re-keys once** for documents whose section or glossary identity
  contained non-NFC text. Recorded in `CHANGELOG.md` under `[Unreleased]`.
- Two ADRs were written alongside this record and are its companions rather
  than its consequences: **ADR-0022** states the local-user threat model that
  decided about a dozen of this review's findings, and **ADR-0023** states the
  wasm engine's absence of a trust boundary, which three consecutive rounds have
  now argued against. Both existed as reasoning; neither existed where the
  question gets asked.
- One finding was routed to tracking rather than fixed: `OI-0038`, provider
  credentials are demanded before the cache is consulted, so a fully-warm run
  that would make zero provider calls cannot start offline.
