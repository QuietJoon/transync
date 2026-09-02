---
type: DCR
title: An offline run against a warm cache is supported behind --offline, and the credential-free provider shares the cache namespace by construction
description: The provider was built before the cache was consulted, so a run whose every unit was a hit still demanded a key. --offline builds a credential-free TransyncOpenAI whose fingerprint is byte-identical to the credentialed one, because the cache key namespaces on that fingerprint.
tags: [change, project-control, DCR-0046]
generated:
  by: claude-code/claude-opus-5
  at: 2026-09-02T00:00:00Z
status: stable
---

# DCR-0046: An offline run from a warm cache

- **Date:** 2026-09-02
- **Source:** ticket `30a744`, OI-0038 (from Review 0004, R0004-0069)
- **Affected DCRs:** `DCR-0028` (the disk cache — this makes its central promise reachable offline), `DCR-0023` (its classification discipline decided the error variant)
- **Affected contracts:** `contracts.md` §1 (the `stable_code()` vocabulary gains a code) and §6 (the CLI argument contract gains a flag)

**Numbering note.** DCR-0035–0039 remain reserved by the unrun HTML→HTML waves
3–7. This is the next free number after DCR-0045.

## The product question, and the answer

OI-0038 was tracked rather than fixed for three weeks for a good reason: the
records never said whether an offline run against a fully warm cache was a
*supported scenario*, and the two possible answers produce different work in
different files. The owner answered on 2026-09-02: **supported, behind an
explicit flag.**

## What was wrong

`translate_cmd` constructed the provider — and therefore demanded
`OPENAI_API_KEY` — **before** the cache was consulted. A run in which every
unit was a cache hit, and which would make zero provider calls, still refused
to start. DCR-0028's own tests prove such a run exists: a second identical run
dispatches zero provider batches. So the disk cache's central promise — a
document is paid for once — was unavailable exactly where it is most obviously
wanted: re-rendering on a machine with no key, or with no network.

## The decision, and where it departs from the ticket

`transync translate --offline` builds a **credential-free** provider. A fully
warm run completes with no key at all; a run that misses stops **at the miss**
with `no provider available for this run` and exit 6.

The ticket's acceptance criteria proposed deferring translator construction
**unconditionally**. That was not taken. An unconditional defer moves every
credential error later and into a different place, so a typo'd key against a
cold cache would surface mid-run rather than at startup — a diagnostic
regression paid by every ordinary run to serve an occasional one. The flag
makes the capability real and leaves the common path's diagnostics alone.

`--offline` **requires** `--cache-dir`, refused at argument time. Without one
the run gets a fresh in-memory cache, so the first unit misses by construction
and the flag would have exactly one reachable outcome; refusing early lets the
message name the flag to add.

## The part that would have shipped broken

The first design was a separate `OfflineTranslator` in the CLI. It would have
been silently useless.

`CacheKey` carries `provider_fingerprint` as a **namespace** axis. A stand-in
translator with its own fingerprint looks in a namespace nothing ever wrote, so
**every** lookup misses — and the failure presents as "the cache is empty",
which an operator reads as a corrupt cache rather than a misconfigured run. The
feature would have appeared to work and done nothing.

Reproducing the right fingerprint in the CLI was the other trap: the OpenAI
composition covers four parts — model, base URL, the API-surface heuristic, and
reasoning effort — so a copy would have been a second opinion about the cache
namespace, and a drifting one fails in exactly the invisible direction. That is
the defect class ti `415cdb`, ti `e20490` and ti `2e2453` each were.

**So the credential moved instead of the type.** `api_key` became
`Option<SecretString>`, and `TransyncOpenAI::offline(model, base_url)` builds a
real instance that resolves every other field exactly as `try_new` does.
`fingerprint()` reads none of the key, so an offline instance namespaces the
cache **byte-identically** to the run that warmed it — by construction, not by
agreement. `an_offline_instance_fingerprints_identically_to_a_credentialed_one`
pins it across three configurations, including a gateway base URL.

`offline` also runs `try_new`'s other two checks — the model id and the base
URL — because both are namespace-bearing. Accepting a configuration the warming
run rejected would point the lookup at a namespace nothing wrote and report a
cache miss for what is a configuration fault. The first draft skipped them
while its doc comment claimed otherwise; the doc was not the thing that was
wrong.

## The error variant, and why not `Unsupported`

`TranslatorError::NoProviderAvailable` → `provider_unavailable` → exit **6**
(`ConfigurationRejected`).

Reusing `Unsupported` would have inherited exit 5, whose own comment states
that the code exists for cases where the CLI **cannot tell** a configuration
fault from a document one. An offline run that missed is unambiguously the
caller's configuration, and the remediation is theirs — drop the flag, or warm
the cache. Naming 5 would have discarded information the CLI had.

The compiler then demanded a decision at `annotate_with_preflight`, and the
answer there is **not** to annotate. That diagnosis names the output-budget
knobs, and a run that failed for want of a provider did not fail over an output
budget; appending a batching remediation would name a knob that cannot help,
which is precisely the actively-harmful classification DCR-0023 exists to
remove.

## Consequences

* Good, because DCR-0028's promise is now reachable in the case that motivated
  it, and the cache namespace correctness is a compile-time property of one
  type rather than an agreement between two.
* Good, because ordinary runs are untouched: same startup checks, same
  diagnostics, same exit codes.
* Neutral, because the stub build accepts and ignores the flag — it needs no
  credential, so it is already what `--offline` asks the live build to become,
  and refusing there would break the smoke suites that exercise the translating
  path. The argument guard still runs in both builds.
* Bad, because `--offline` is OpenAI-shaped: `TransyncOpenAI::offline` is the
  only credential-free constructor, so a future provider wanting the flag has
  to add its own. That is honest for a binary DCR-0029 scoped to one provider.

## Evidence

- `cargo test --workspace --no-fail-fast -- --test-threads=4`: 44/44 binaries,
  **1175 passed, 0 failed**, `CARGO_EXIT=0`.
- The argument guard's test asserts the **message**, not only the code. Its
  first draft tripped on the output-target requirement instead — a different
  exit-1 path — which is the test discriminating rather than agreeing.
