---
type: DCR
title: Five corrections to the Gemini adapter, and what the first live call settled
description: "A review of the transync-google code read five defects. An impossible output ceiling raised Unsupported and landed on the exit code that means 'I cannot tell a configuration fault from a document one', when it is unambiguously the former. The MAX_TOKENS message named a knob this adapter refuses to let the operator turn. The cleartext-loopback test compared host strings, so http://[::1] was refused as remote and 127.0.0.0/8 was one address wide. A test's name stated the opposite of the precedence it tested. And both argument refusals DCR-0058 mandated had no test. The same pass records what the 2026-10-13 live call answered: Vertex express accepts the x-goog-api-key header."
tags: [provider, google, vertex, error-taxonomy, DCR-0061, DCR-0058, DCR-0060, DCR-0030, DCR-0023]
generated:
  by: claude-code/claude-opus-5
  at: 2026-10-13T00:00:00Z
status: stable
---

# DCR-0061: five corrections to the Gemini adapter, and what the first live call settled

- **Date:** 2026-10-13.
- **Source:** an owner-requested review of the `transync-google` code, which
  returned five findings; the owner's instruction was to fix the obvious ones
  and bring back anything needing a decision. All five were obvious. The one
  thing that did need a decision — whether to run the live gate with real keys
  — was taken separately and is what produced the verification section below.
- **Extends:** DCR-0058 and DCR-0060, both Records and both **unedited**. This
  record carries the corrections to what they shipped.
- **Behaviour changed:** yes, in three of the five. One error variant changes
  on a documented path, one error message changes, and one validator accepts
  base URLs it used to refuse. None is breaking; §Semver says why.

**Numbering note.** Next free number after DCR-0060.

## 1. An impossible output ceiling is the operator's configuration, not an ambiguity

`validate_output_ceiling` refuses a `[batching].target_output_tokens` above
`gemini-3.8-flash`'s published 65,536-token maximum. It raised
`TranslatorError::Unsupported`, which the CLI maps to exit **5**, and
`crates/transync-cli/src/error.rs` states precisely why that mapping exists:

> `Unsupported` is genuinely ambiguous — a different model might accept the
> batch, or the document might carry a construct nothing accepts — and a code
> that guesses is worse than one that admits it does not know.

That reasoning is correct about the *variant* and wrong about *this call site*.
Nothing here has looked at the document. This is a pre-flight comparison of one
number from the operator's profile against one published model limit, made
before a request is built, so there is exactly one possible fault and exactly
one remediation, and both belong to the caller. Saying "I cannot tell" about a
question with one answer is the actively-harmful classification DCR-0023 exists
to remove — the same argument `NoProviderAvailable` was split out of
`Unsupported` on.

It now raises `ProviderRejected { status: None, .. }`, which the CLI maps to
`ConfigurationRejected`. The variant fits: the request was *refused* rather
than failed, and `status: None` is the documented spelling for a refusal with
no HTTP status behind it — here because no provider was asked at all. The
message gained the rejected value and the knob that set it, so the operator
reads what to change rather than only what is wrong.

Only the default model is checked, unchanged. Another model's ceiling is not
this crate's to know, so it rides to the provider, which does know.

## 2. The `MAX_TOKENS` remediation was circular at the ceiling

When Gemini stops at `MAX_TOKENS`, the adapter raises `OutputCeilingExhausted`,
whose own contract says the operator knob is
`[batching].target_output_tokens`. For a run that is already at
`gemini-3.8-flash`'s maximum — which is the common case, since 65,536 is the
CLI batching default — that advice points at a knob **§1's validator refuses to
let the operator turn**. The two rules were written separately and contradict
each other: one says raise it, the other refuses the raise.

The message is now computed by `max_tokens_remediation`, which offers the raise
only while there is headroom to raise into. At the cap it names the knobs that
still move: batch size, which decides how much output one request asks for, and
thinking level, whose tokens come out of this same ceiling. The old message's
one genuinely useful sentence — that thinking tokens consume the limit — is
kept in both branches.

## 3. Cleartext loopback was matched on host strings, so `[::1]` was never loopback

`validate_config` allows `http` only to a loopback host. It tested
`host_str() == "::1"`, and `Url::host_str` serializes an IPv6 host **with its
brackets**, so that comparison can never be true: `http://[::1]:8080/` — a
gateway on this very machine — was refused as remote. The IPv4 arm had the
narrower form of the same defect, testing `== "127.0.0.1"` when the whole of
`127.0.0.0/8` is loopback, so `127.0.0.2` was refused too. Both fail closed, so
neither is a security defect; both turn away legitimate local gateways.

It now matches on the parsed `url::Host` and asks the address type, with RFC
6761's `localhost` / `*.localhost` handled by name. This is a deliberate twin
of `transync_openai`'s `is_cleartext_to_remote_host` / `is_loopback_name` pair
(R0002-0005) — the two provider crates share no dependency that could hold the
rule, and it belongs to neither. The code says so, so the next person changes
both. `contracts.md` §7 and §8 already documented this rule in the correct
form for the other two adapters; §9 now matches them, which is the point: the
Google adapter was the odd one out, and a difference nobody chose is a defect.

## 4. A test's name stated the opposite of the precedence it tested

`finish_reason_precedes_partial_text_and_policy_block` asserted that
`finishReason` precedes a policy block. `parse_generation_response` reads
`promptFeedback.blockReason` **first**. Nothing caught the contradiction
because no case in the test carried both signals, so the name was the only
statement of an ordering in the file and it was wrong.

The implemented order is the right one and now has a test that pins it: a
blocked `promptFeedback` says the **input** was refused, which is why there is
no candidate worth reading, while `finishReason` describes a candidate that was
at least attempted. Reporting the attempt would name the wrong cause. The test
is renamed to
`a_blocked_prompt_outranks_finish_reason_which_outranks_partial_text`, gains
the both-present case, and gains one more that was equally unpinned:
`BLOCK_REASON_UNSPECIFIED` is the absence of a block and must not be read as
one.

## 5. Both argument refusals DCR-0058 mandated had no test

DCR-0058 mandates two refusals in the CLI: a `--google-*` flag without
`--provider google` is an argument error, and `--google-project` /
`--google-location` do not apply to the Developer surface. Neither had a test.
DCR-0060's `--google-auth` arrived with tests for *itself* under both rules,
which is how the gap stayed invisible — the first rule's guard could have been
narrowed to `--google-auth` alone and the suite would have stayed green.

Both are now covered, each flag driven separately rather than together, because
one working `||` arm holding the whole condition up reads identically to five
working arms. The Developer-surface test is deliberately distinct from the
existing `developer_surface_uses_only_its_own_environment`, which sets
`GOOGLE_CLOUD_LOCATION` in the environment and asserts the run *succeeds* with
a null location. That is the opposite rule and also correct: ambient
environment is not a request, and only an explicitly passed flag is refused.

## What the first live call settled

DCR-0060 shipped with a **Not yet verified** section, because the adapter had
to choose between two spellings of the express-mode credential and could not
test the choice. It sends `x-goog-api-key` as an HTTP header; Google's own
published examples use a `?key=` query parameter. The header was chosen on
principle — DCR-0030 keeps credentials out of URLs, where they reach logs,
proxies and error messages — and the record said plainly that no live call had
confirmed it.

**It is confirmed.** On 2026-10-13 `./scripts/smoke-live-gate.sh
google-vertex-express` completed a round-trip against `gemini-3.8-flash`, so
express mode accepts the header form, the documented query form is one spelling
of this key rather than the only one, and no `?key=` fallback is needed. This
is also `transync-google`'s first live evidence of any kind. Ticket `c5b1431a`
closes on it, and the note in `scripts/smoke-live-gate.sh` that asked for the
first run's outcome now records the answer instead of the question.

## Semver

Nothing here is breaking, so none of it needs a window.

- The error-variant change in §1 is a behaviour change on a documented path,
  not a type change: `TranslatorError` is unchanged and both variants already
  existed. A caller matching on `Unsupported` for this specific refusal sees a
  different arm, which is why it is recorded here rather than treated as a
  bug fix — but `TranslatorError` is `#[non_exhaustive]` and no exhaustive
  match can break.
- §2 changes an error *string*. No test outside this crate asserts it.
- §3 **widens** what is accepted. A base URL that worked still works.
- §4 and §5 are tests.

## Gates

`cargo test -p transync-google --lib` 15 passed / 0 failed, up from 12.
`cargo test -p transync-cli --features test-stub-provider --test
google_provider` 11 passed / 0 failed, up from 9. The workspace suite, clippy
and the two-package `wasm32` gate ran through the tracked pre-commit hook on
the commit carrying this record.

## What this does not do

It does not touch DCR-0058's or DCR-0060's text; both are Records. It does not
change any flag, default, environment variable or fingerprint. It does not
revisit which Google surfaces exist. And it does not give `transync-google`
live evidence for its other two surfaces: the Developer API and ADC Vertex
remain **UNVERIFIED**, recorded as such in the README's provider-evidence
table, and the ADC leg is deferred to its own track because the credentials
live on another account.
