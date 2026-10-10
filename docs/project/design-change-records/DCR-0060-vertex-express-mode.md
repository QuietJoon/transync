---
type: DCR
title: Vertex AI express mode is a third Google surface
description: "DCR-0058 reached Vertex AI only with Application Default Credentials, a project and a location. Express mode reaches the same service with an API key and addresses the publisher model directly, so transync-google gains GoogleSurface::VertexExpress and GoogleCredentials::VertexApiKey, and the CLI selects it only on an explicit --google-auth api-key with the Vertex surface, reading GOOGLE_API_KEY. An API key in the environment never selects it."
tags: [provider, google, vertex, DCR-0060, DCR-0058, DCR-0030]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-10T00:00:00Z
status: stable
---

# DCR-0060: Vertex AI express mode is a third Google surface

- **Date:** 2026-10-10.
- **Source:** a working-tree change to `transync-google` and the CLI, one of
  whose tests cites this number with no record behind it, found by
  `/compact-docs` on 2026-10-06 (ticket `1cc0c538`). This record states what
  that change ships; the code lands with this record.
- **Extends:** DCR-0058, which is a Record and is **not** edited. Its two
  surfaces and its defaults stay as they were; this adds a third surface.
- **Behaviour changed:** a new opt-in path only. No existing flag, default,
  environment reading or fingerprint changes.

**Numbering note.** Next free number after DCR-0059.

## Why a variant, and not a credential flag on `Vertex`

Express mode is the same Vertex service and the same models, reached with an
API key instead of Application Default Credentials (ADC). Its request URL is
different: it carries no project and no location, and addresses the
publisher model directly —
`https://aiplatform.googleapis.com/v1/publishers/google/models/{model}:generateContent`.

`TransyncGoogle::offline` promises the same fingerprint as a live instance,
and the fingerprint follows the effective request URL, so the URL's shape has
to be derivable from the configuration alone, not from which credential was
handed to `try_new`. That is why express mode is a surface,
`GoogleSurface::VertexExpress`, beside `Developer` and
`Vertex { project, location }`, with its own credential,
`GoogleCredentials::VertexApiKey`.

`TransyncGoogle::try_new` accepts `VertexExpress` only with `VertexApiKey`,
refuses a blank key (`EmptyApiKey`), and refuses every other pairing
(`CredentialSurfaceMismatch`) rather than sending it.

## Why the CLI makes it explicit

A key found in the environment never selects a surface (DCR-0058, after
contracts §8's rejection of implicit provider selection by available key).
`GOOGLE_API_KEY` is a generic name, and an operator may have exported it for
some other Google service; that key must not silently move a Vertex run's
billing path. The CLI therefore takes express mode only on an explicit
`--google-auth api-key`, and only when the selected surface is Vertex, which
is the Google default. `--google-auth adc`, the default, is DCR-0058's ADC
path. No environment variable stands in for the flag.

Argument rules, all answered before an input file or a credential is read:

- `--google-auth` with `--google-surface developer` is an argument error; the
  Developer API always authenticates with `GEMINI_API_KEY`.
- `--google-auth api-key` with `--google-project` or `--google-location` is an
  argument error. Express mode's URL has nowhere to put either, and an
  explicitly passed one would otherwise be dropped silently. An ambient
  `GOOGLE_CLOUD_PROJECT` is ignored rather than refused: it is environment,
  not a request.
- `--google-auth` with `--provider openai` is an argument error, like every
  other Google-only flag (DCR-0058).

The rest of the path:

- A live express run reads `GOOGLE_API_KEY`. `--offline --cache-dir` builds
  the same identity without reading it.
- `--base-url`, or `TRANSYNC_GOOGLE_VERTEX_BASE_URL` when the flag is absent,
  overrides the origin exactly as it does for ADC Vertex. DCR-0058's Google
  batching defaults apply unchanged.
- Under `--verbose`, the run logs one line saying it is in express mode.

## Wire shape and identity

- **Credential:** the key rides an `x-goog-api-key` header, never the `?key=`
  query parameter Google's own curl examples use: a URL carries into logs,
  proxies and error messages. DCR-0030 set the same posture for provider
  credentials (no redirect is followed, and a base URL carrying userinfo is
  refused). The Developer surface already sends its key this way.
- **Request body:** Vertex's `responseSchema` dialect, as for ADC Vertex.
- **Fingerprint:** provider `google`, surface `vertex-express`, an empty
  project and location, the model, the effective request URL and the thinking
  level. It differs from ADC Vertex's for the same model, so the two never
  share a cache entry. No secret is in it.
- **Tokenizer hint:** `cl100k_base`, unchanged.

## Not yet verified

- **The header form has not been confirmed by a live call.**
  `scripts/smoke-live-gate.sh google-vertex-express` is the leg that settles
  it, and the script's header, which asks for the first run's outcome,
  records none. If that leg returns 401 or 403 while the same key works
  against the documented `?key=` URL, the header form is the cause.
- **Trial-credit eligibility is not assessed.** DCR-0058's credit argument
  compared Vertex AI under ADC with the Developer API and says nothing about
  express mode; neither does this record. Check billing before a paid run.

## Semver

`GoogleSurface` and `GoogleCredentials` are exhaustive enums, so a new
variant stops an exhaustive `match` over either from compiling.
`transync-google` is in no released tag — it first appears after `v0.5.0` —
so these variants break no released surface.

## Gates

- `crates/transync-google/src/tests.rs`:
  `vertex_express_addresses_the_publisher_model_without_project_or_location`,
  `vertex_express_pairs_only_with_an_api_key` (every refused pairing of the
  express surface or the express key) and
  `vertex_express_and_adc_vertex_do_not_share_a_fingerprint`.
- `crates/transync-cli/tests/google_provider.rs`:
  `google_auth_api_key_selects_vertex_express_without_project_or_location`,
  `google_auth_api_key_refuses_a_project_or_location`,
  `google_auth_applies_to_the_vertex_surface_only`,
  `google_auth_requires_the_google_provider` and
  `an_ambient_google_api_key_never_selects_express_mode`.
- `crates/transync-google/tests/live_smoke.rs`: `vertex_express_live_smoke`,
  `#[ignore]`d and double-gated like the other legs, run by
  `scripts/smoke-live-gate.sh google-vertex-express` and by its `all`.

## What this does not do

- It does not change DCR-0058's default: `--provider google` still means ADC
  Vertex.
- It adds no environment variable that selects express mode.
- It does not change how the Developer surface authenticates.
