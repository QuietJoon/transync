---
type: DCR
title: Gemini 3.8 Flash through the Google Developer and Vertex APIs
description: Add transync-google as a sibling Translator, expose explicit Google surface selection in the CLI, and make Vertex the Google default so a Google Cloud trial can fund eligible translation calls.
tags: [provider, google, gemini, DCR-0058]
generated:
  by: codex/gpt-6
  at: 2026-09-23T00:00:00Z
status: stable
---

# DCR-0058: `transync-google` for Gemini 3.8 Flash

- **Date:** 2026-09-23.
- **Source:** owner request for a Gemini adapter and use of a 90-day, $300
  Google Cloud trial credit.
- **Decision:** add one Google provider crate with two explicit API surfaces and
  connect it to `transync translate`. The existing OpenAI CLI path remains the
  default. `--provider google` selects Vertex AI unless
  `--google-surface developer` is requested.
- **Model:** `gemini-3.8-flash` by default. The similarly named
  `gemini-3.8-live-extended-thinking` is a Live API model without structured
  output support, so it is not the translation default.

## Why two surfaces are explicit

The Google Developer API uses `GEMINI_API_KEY`; Vertex AI uses Google Cloud
Application Default Credentials (ADC), a project and a location. A key found in
the environment never selects a surface. The CLI's `--provider` choice and its
Google surface determine which credential and endpoint are used. This keeps
cache identity and billing destination observable, following contracts §8's
rejection of implicit provider selection by available key.
Vertex forwards the ADC SDK's authentication headers, including
`x-goog-user-project`. The selected Vertex project is the default quota
project; `GOOGLE_CLOUD_QUOTA_PROJECT` overrides it when set, and the ADC
principal needs `serviceusage.services.use` on that quota project.

The [Google Cloud Free Trial](https://docs.cloud.google.com/free/docs/free-cloud-features)
provides up to $300 over 90 days for eligible Cloud services but excludes the
Gemini API in AI Studio. The [Gemini API billing guide](https://ai.google.dev/gemini-api/docs/billing)
also says Welcome credit does not pay Developer API charges. Thus the CLI's
Google default is **Vertex AI**, while the Developer API remains available as
an explicit alternative. Vertex credit eligibility follows as an inference
from the published exclusion, not as an account-specific guarantee. Verify
project billing and trial-credit applicability before a paid run; remaining
credit and quota belong to the account and project.

## Interface and budget

- `transync-google` implements the existing `Translator` trait and uses the
  shared prompt, schema and result parsers. The facade does not re-export it.
  Google request shaping, transport and response classification stay inside the
  provider crate; no generic provider capability descriptor is introduced.
- CLI flags are `--provider openai|google`, `--google-surface
  vertex|developer`, `--google-project`, `--google-location global|us|eu` and
  `--google-thinking low|medium|high`. The Google defaults are Vertex,
  `gemini-3.8-flash`, location `global` and thinking `low`. Explicit
  Google-only flags with OpenAI, and Vertex-only flags with the Developer
  surface, are argument errors. `--model` and `--base-url` retain their generic
  names but resolve against only the selected provider and surface.
- The corresponding environment configuration is `TRANSYNC_PROVIDER`,
  `TRANSYNC_GOOGLE_SURFACE`, `TRANSYNC_GOOGLE_MODEL`,
  `TRANSYNC_GOOGLE_THINKING`, `TRANSYNC_GOOGLE_VERTEX_BASE_URL`,
  `TRANSYNC_GOOGLE_DEVELOPER_BASE_URL`, `GOOGLE_CLOUD_PROJECT` and
  `GOOGLE_CLOUD_LOCATION`; Developer calls require `GEMINI_API_KEY`.
  Each flag wins over its environment value. The selected provider's
  environment is read without consulting the other provider's settings.
- For a Google CLI run with no explicit batching override, the effective
  input target is 20,000 tokens, the output ceiling is 65,536 tokens, the
  output expansion factor is 2.0 and the hard unit cap is 32. Input and output remain separate limits; output includes
  thinking tokens, so an output-ceiling stop names the output knob.
  An explicit CLI or profile batching value wins over these Google defaults.
- Provider fingerprinting distinguishes surface, model, effective endpoint,
  thinking and, for Vertex, project and location. Secrets are never included.
  `--offline --cache-dir` constructs the same non-secret identity without
  reading a key or ADC; a warm cache can replay and a miss returns the existing
  `provider_unavailable` cause and exit 6.

The [Gemini 3.8 Flash model card](https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash)
lists a 1,048,576-token input limit, a 65,536-token output limit, structured
outputs and `low`/`medium`/`high` thinking. The CLI's 20,000-token input
target is a batching default, not a claim that the model's input limit is
20,000.

**Price snapshot, 2026-09-23:** the [Google Cloud model pricing table](https://cloud.google.com/gemini-enterprise-agent-platform/generative-ai/pricing)
lists introductory Vertex global Standard rates through 2026-12-31 of
$0.75 per million input tokens and $3.75 per million output tokens, including
thinking. Its `us`/`eu` non-global rates are $0.825 and $4.125. The
[Developer API pricing table](https://ai.google.dev/gemini-api/docs/pricing)
lists its own paid Standard rates of $0.75 and $3.75 through that date.
These are dated reference prices, not an enforced spending limit; verify
current account pricing before a paid run.

## Verification and records

Offline tests cover both request/response shapes, credentials, error mapping,
construction identity, cancellation and a full translation against loopback
responses. CLI tests pin provider/surface selection, provider-specific
configuration precedence and credential-free warm-cache replay. Live tests
are ignored by default and require an explicit opt-in for each Google surface;
the release gate names each surface separately so a missing credential cannot
silently count as a pass. The workspace publication roster, rustdoc gate and
release checklist include the new library crate. Living documentation describes
the resulting behavior; this record preserves the decision.
