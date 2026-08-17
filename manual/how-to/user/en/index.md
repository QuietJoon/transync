# How-to guides — user

* [How to translate a Markdown document with the CLI](translate-a-document-with-the-cli.md) - Run `transync translate` end-to-end to produce a translated document and a browsable dual-pane demo bundle, then open it with `transync serve`.
* [How to write a Profile TOML for a translation style](write-a-translation-profile.md) - Adapt the Profile Cookbook's technical-docs recipe into a working profile.toml, scope a glossary term to one section, and point transync translate at it with --profile.
* [How to diagnose a translation run](diagnose-a-translation-run.md) - Read the exit code, the stderr line, and validation-report.json to find out why a run failed, refused, fell back, or looks wrong — and verify a profile's compiled prompt before spending a real run on it.
* [How to reuse a warm cache across runs](reuse-a-warm-cache-across-runs.md) - Point `transync translate` at a `--cache-dir` so a second run over the same document re-dispatches only what actually changed, and operate that directory safely.
