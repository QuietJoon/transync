---
okf_version: "0.2"
---
# Manual

## Tutorials
* [Getting started with transync](tutorials/operator/en/getting-started.md) - Build transync from a clean checkout, run its tests, translate a sample document with no API key, and watch the two rendered panes scroll in sync in a browser. (operator)
    * [ko](tutorials/operator/ko/getting-started.md)

## How-to guides
* [How to translate a Markdown document with the CLI](how-to/user/en/translate-a-document-with-the-cli.md) - Run `transync translate` end-to-end to produce a translated document and a browsable dual-pane demo bundle, then open it with `transync serve`. (user)
    * [ko](how-to/user/ko/translate-a-document-with-the-cli.md)
* [How to write a Profile TOML for a translation style](how-to/user/en/write-a-translation-profile.md) - Adapt the Profile Cookbook's technical-docs recipe into a working profile.toml, scope a glossary term to one section, and point transync translate at it with --profile. (user)
    * [ko](how-to/user/ko/write-a-translation-profile.md)
* [How to diagnose a translation run](how-to/user/en/diagnose-a-translation-run.md) - Read the exit code, the stderr line, and validation-report.json to find out why a run failed, refused, fell back, or looks wrong — and verify a profile's compiled prompt before spending a real run on it. (user)
    * [ko](how-to/user/ko/diagnose-a-translation-run.md)
* [How to reuse a warm cache across runs](how-to/user/en/reuse-a-warm-cache-across-runs.md) - Point `transync translate` at a `--cache-dir` so a second run over the same document re-dispatches only what actually changed, and operate that directory safely. (user)
    * [ko](how-to/user/ko/reuse-a-warm-cache-across-runs.md)
* [How to serve a translated demo bundle with transync serve](how-to/operator/en/serve-the-demo-bundle.md) - Serve an --html-out or --out-dir demo bundle over loopback HTTP with `transync serve`, confirm the two panes sync, and stop it cleanly. (operator)
    * [ko](how-to/operator/ko/serve-the-demo-bundle.md)
* [How to build and run the wasm render+edit demo](how-to/operator/en/build-the-wasm-demo.md) - Build the transync-wasm module with scripts/build-wasm.sh, assemble the demo directory beside a translated document, serve it with `transync serve`, and read the demo's own boot verdict. (operator)
    * [ko](how-to/operator/ko/build-the-wasm-demo.md)
* [How to implement a custom Translator provider](how-to/developer/en/implement-a-custom-translator.md) - Write a sibling crate that implements transync's `Translator` trait against HEAD — cancellation parameter included — map its failures onto the error taxonomy, and hand it to the pipeline. (developer)
    * [ko](how-to/developer/ko/implement-a-custom-translator.md)
* [How to cancel a running translation](how-to/developer/en/cancel-a-running-translation.md) - Hand a `CancellationToken` to `translate_with_cache`, keep the progress a cancelled run paid for, and make your own `Translator` honour the token instead of relying on future-drop. (developer)
    * [ko](how-to/developer/ko/cancel-a-running-translation.md)

## Reference
* [CLI reference](reference/user/en/cli.md) - Every flag, default, limit, exit code, environment variable, and stderr line for `transync translate` and `transync serve`. (user)
    * [ko](reference/user/ko/cli.md)
* [Profile TOML schema](reference/user/en/profile-toml-schema.md) - Every key a Profile TOML accepts — type, default, valid range, how a CLI flag overlays it, and what the loader warns about. (user)
    * [ko](reference/user/ko/profile-toml-schema.md)
* [The `Translator` trait](reference/developer/en/translator-trait.md) - Method signatures, the complete `TranslatorError` variant set with its stable codes, and the behavior contract every implementation must satisfy. (developer)
    * [ko](reference/developer/ko/translator-trait.md)
* [Alignment-map JSON schema](reference/developer/en/alignment-map-schema.md) - The durable wire shape of the alignment map — top-level keys, every block-level field, the schema-version policy, and the normative pairing rule. (developer)
    * [ko](reference/developer/ko/alignment-map-schema.md)

## Explanation
* [Why block-ID sync, not scroll-percentage](explanation/developer/en/architecture-overview.md) - The reasoning behind transync's core design choice — one stable block ID as the only sync currency, end to end — how that choice shapes every other layer, and why a table split for the model is still one block for the reader. (developer)
    * [ko](explanation/developer/ko/architecture-overview.md)
* [Why layered validation and bounded retry/fallback](explanation/developer/en/validation-retry-fallback-model.md) - Why the pipeline checks a translation through seven independent layers instead of trusting the schema, why the one layer that reads content twice had to be widened before it was right, why retries resubmit verbatim instead of coaching the model, which seams are allowed to refuse rather than degrade, and where the fallback paper trail is thinner than it looks. (developer)
    * [ko](explanation/developer/ko/validation-retry-fallback-model.md)
* [How glossary entries are resolved](explanation/user/en/how-glossary-entries-are-resolved.md) - Why a glossary term can come from your profile or from the auto-glossary preflight, how a section-scoped entry is matched to a heading, and which entry wins when several could apply to the same term. (user)
    * [ko](explanation/user/ko/how-glossary-entries-are-resolved.md)
* [Why batches stop at section boundaries](explanation/user/en/why-batches-stop-at-section-boundaries.md) - Why no request ever mixes units from two headings, what that buys in glossary exactness and context coherence, and what it costs in request count on heading-rich documents. (user)
    * [ko](explanation/user/ko/why-batches-stop-at-section-boundaries.md)
