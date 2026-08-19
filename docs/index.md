# transync — Documentation Index

Single entry point for everything under `docs/`. Update this file whenever a new doc lands so nothing becomes orphaned — `crates/transync/tests/docs_index_drift.rs` enforces both halves of that rule: every Markdown document under `docs/` must be linked from here, and every link from here must resolve.

Three things sit outside the rule on purpose, because they are not this index's to carry, and the test excludes exactly them: `*.ko.md` Korean siblings (generated translations, git-ignored); git-ignored generated bundles under `docs/` — today `docs/investigation/`, regenerated from the code and untracked since 2026-08-08 — which the test reads out of `.gitignore` rather than hardcoding, so a re-tracked bundle is required again the day its ignore line goes; and this file, which needs no self-link. Non-Markdown files are not *required*, which is why `project/phase-state.yaml` appears below because a reader wants it rather than because the test demands it.

## For developers (start here)

- [Quick Start](Quick_Start.md) — clone, build, test, first translate in 5 minutes.
- [Developer Guide](Developer_Guide.md) — using transync as a library, full CLI reference, custom system prompts, profile TOML, custom `Translator` impls, demo CSS theme customization, JS sync engine tunables, common pitfalls.
- [Profile Cookbook](Profile_Cookbook.md) — drop-in TOML recipes for technical, literary, marketing, code-heavy and strict-preserve documents, plus a section-scoped glossary recipe for a term that means two different things in two chapters, with authoring tips.
- [Performance Tuning](Performance.md) — the three knobs that move wall-clock, sized as recipes for small / medium / large / code-heavy documents, plus how to diagnose provider-floor latency and a coarse peak-memory measurement across document sizes.
- [Troubleshooting](Troubleshooting.md) — symptom-keyed reference for the issues that surface during real runs (silent CLI exits, `model_not_found`, schema refusals, all-fallback runs, sync freezes, slow translation, glossary not landing).

## Architecture

- [Architecture Overview](architecture/README.md) — components, data flow, and pointers to deeper docs.
- [MVP Scope](architecture/mvp-scope.md) — what is in and out of scope for the first usable version.
- [Scenario Matrix](architecture/scenario-matrix.md) — concrete scenarios driving design and validation.
- [Source-of-Truth Table](architecture/source-of-truth-table.md) — ownership of every concern.
- [Rough Schema](architecture/rough-schema.md) — every wire / durable type.
- [Persistence and Files](architecture/persistence-and-files.md) — file lifecycle + atomic-write contract.
- [Contracts](architecture/contracts.md) — `Translator`, profile TOML, alignment-map JSON, HTML attrs, retry policy, CLI args.
- [Settled Questions](architecture/settled-questions.md) — reviewer-facing register of recurring review findings the record already answers, with the authoritative citation for each.

## Implementation

- [Module Map](implementation/module-map.md) — scenario → module/file/integration coverage.

## Decisions (ADRs)

- [ADR 0001 — Block-level alignment as sync currency](decisions/0001-block-level-alignment-as-sync-currency.md)
- [ADR 0002 — HTTP-free core with `Translator` trait](decisions/0002-http-free-core-with-translator-trait.md)
- [ADR 0003 — Cargo workspace with provider crates](decisions/0003-cargo-workspace-with-provider-crates.md)
- [ADR 0004 — Comrak as GFM parser](decisions/0004-comrak-as-gfm-parser.md)
- [ADR 0005 — BlockId format `<kind>-<NNNN>`](decisions/0005-block-id-format.md)
- [ADR 0006 — Renderer output shape (two pane fragments + shell)](decisions/0006-renderer-output-shape.md)
- [ADR 0007 — Renderer wraps `<table>` and `<pre>` in a transparent `<div>`](decisions/0007-renderer-wrapper-div-for-tables-and-code.md)
- [ADR 0008 — Reject making `warnings` optional in the strict output schema](decisions/0008-reject-optional-warnings-in-strict-schema.md)
- [ADR 0009 — Reject changes to the bounded retry policy](decisions/0009-reject-retry-policy-changes.md)
- [ADR 0010 — Reject replacing the tokenizer-initialization panic](decisions/0010-reject-tokenizer-panic-fallback.md)
- [ADR 0011 — Keep the LAN bind in the convenience wrappers](decisions/0011-lan-bind-in-convenience-wrappers.md)
- [ADR 0012 — Inline content is LLM-owned; inline constraints are advisory](decisions/0012-inline-content-llm-owned-advisory-constraints.md)
- [ADR 0013 — Language labels are opaque caller-supplied strings](decisions/0013-opaque-language-labels.md)
- [ADR 0014 — `conditional-on-section` glossary scope: the record of the rejection era](decisions/archive/0014-section-scoped-glossary-renders-globally.md) *(archived — superseded 2026-08-09 by DCR-0027, which re-admitted the scope)*
- [ADR 0015 — Cache poison & invalid-hit policy](decisions/0015-cache-poison-and-invalid-hit-policy.md)
- [ADR 0016 — Whole-document-in-memory is inherent](decisions/0016-whole-document-in-memory.md)
- [ADR 0017 — Batch-terminal provider failures abort the whole run](decisions/0017-batch-terminal-failures-abort-the-run.md)
- [ADR 0018 — HTML content is translatable via app-owned segment extraction](decisions/0018-html-content-translation-via-segment-extraction.md)
- [ADR 0019 — The browser gets the Rust renderer as a wasm demo layer, not the CLI bundle](decisions/0019-wasm-demo-layer.md)
- [ADR 0020 — Cache identity is not a security boundary; adversarial hash collision is out of the threat model](decisions/0020-cache-identity-excludes-adversarial-collision.md)
- [ADR 0021 — The disk cache is a self-contained JSON-lines log replayed at open; document-level metadata rides the cache](decisions/0021-disk-cache-jsonl-log-and-document-metadata.md)
- [ADR 0022 — A local user with write access to transync's own paths is outside the threat model](decisions/0022-local-user-threat-model.md)
- [ADR 0023 — The wasm engine has no trust boundary; its caller supplies both sides](decisions/0023-wasm-engine-has-no-trust-boundary.md)
- [ADR 0024 — The output publication protocol: a kernel lock, a surviving marker, and own-pid-only reclamation](decisions/0024-output-publication-protocol.md)

## Design change records (DCRs)

- [DCR-0001 — Renderer wrapper-div for tables and code blocks](project/design-change-records/archive/DCR-0001-renderer-wrapper-div.md) *(archived)*
- [DCR-0002 — Post-regen full-reparse failure becomes recoverable](project/design-change-records/archive/DCR-0002-recoverable-full-reparse-failure.md) *(archived)*
- [DCR-0003 — Annotated-HTML wrapper positioning contract](project/design-change-records/archive/DCR-0003-wrapper-positioning-contract.md) *(archived)*
- [DCR-0004 — `FallbackPerBlock` widens to neighbors and auto-escalates](project/design-change-records/archive/DCR-0004-fallback-per-block-widens-and-escalates.md) *(archived)*
- [DCR-0005 — Facade crate + `transync-core` split](project/design-change-records/DCR-0005-facade-and-core-crate-split.md)
- [DCR-0006 — CLI outputs commit all-or-nothing](project/design-change-records/archive/DCR-0006-all-or-nothing-output-staging.md) *(archived)*
- [DCR-0007 — Grouped list rendering with `<li>` sync anchors](project/design-change-records/DCR-0007-grouped-list-rendering.md)
- [DCR-0008 — Scroll-listener sync engine replaces the IntersectionObserver design](project/design-change-records/DCR-0008-scroll-listener-sync-engine.md)
- [DCR-0009 — v0.2 provider/cache boundary (fingerprint, fallible Cache + evict, RetryContext)](project/design-change-records/DCR-0009-v02-provider-cache-boundary.md)
- [DCR-0010 — Inline-protection validation layer (destinations + pledged code spans)](project/design-change-records/archive/DCR-0010-inline-protection-layer.md) *(archived)*
- [DCR-0011 — Staged fileset commit, `--out-dir` publish, CLI admission limits](project/design-change-records/archive/DCR-0011-staged-commit-out-dir-and-limits.md) *(archived)*
- [DCR-0012 — Output-aware batch packing + CLI batching knobs](project/design-change-records/DCR-0012-output-aware-batching-and-cli-knobs.md)
- [DCR-0013 — Reader-honesty placeholders + reference-link (refmap) resolution](project/design-change-records/archive/DCR-0013-reader-honesty-placeholders-and-refmap.md) *(archived)*
- [DCR-0014 — Auto-glossary preflight + per-batch schema-fault fairness](project/design-change-records/DCR-0014-auto-glossary-and-batch-fault-fairness.md)
- [DCR-0015 — Prompt lift into core, tokenizer hint, RTL pane direction, gated live smoke](project/design-change-records/archive/DCR-0015-prompt-lift-tokenizer-hint-rtl-live-smoke.md) *(archived)*
- [DCR-0016 — HTML-content translation via segment extraction (alignment schema 1.2.0)](project/design-change-records/DCR-0016-html-content-translation.md)
- [DCR-0017 — `transync-syntax` crate split + AST-direct renderer (wasm32 gate)](project/design-change-records/archive/DCR-0017-transync-syntax-split.md) *(archived)*
- [DCR-0018 — Public-surface curation (explicit facade, compiler-enforced tiers, `#[non_exhaustive]`)](project/design-change-records/archive/DCR-0018-public-surface-curation.md) *(archived)*
- [DCR-0019 — OI-0008 + OI-0033 internal-quality wave (CR-aware line table, pure policy module, six concern splits)](project/design-change-records/archive/DCR-0019-internal-quality-wave.md) *(archived)*
- [DCR-0020 — Track C wasm render+edit demo (`transync-wasm`, build pipeline, browser demo, parity gates)](project/design-change-records/DCR-0020-track-c-wasm-demo.md)
- [DCR-0021 — Publication lock over output directories, honest `--out-dir` replacement wording, non-reclaimed staging temps](project/design-change-records/DCR-0021-publication-lock-and-honest-replacement.md)
- [DCR-0022 — Review 0002 hardening (the render seam becomes fallible, and three guards stop trusting their input)](project/design-change-records/DCR-0022-review-0002-hardening.md)
- [DCR-0023 — Provider error taxonomy (five terminal causes named, `stable_code()` reaches them)](project/design-change-records/DCR-0023-provider-error-taxonomy.md)
- [DCR-0024 — Run cancellation (supersedes DCR-0009's YAGNI deferral; a cancelled run answers `Err`)](project/design-change-records/DCR-0024-run-cancellation.md)
- [DCR-0025 — Review 0003 hardening (content loss closed at four seams; a refusal outranks the text beside it)](project/design-change-records/DCR-0025-review-0003-content-loss-and-refusal-precedence.md)
- [DCR-0026 — Oversize tables split into header-carrying row windows at packing time (STUB-017 commissioned; slices SL-100..SL-105)](project/design-change-records/DCR-0026-oversize-table-row-window-split.md)
- [DCR-0027 — Batches become section-coherent, and the glossary section scope becomes real (ticket 43cfb4; slices SL-106..SL-109)](project/design-change-records/DCR-0027-section-coherent-batching-and-glossary-section-scope.md)
- [DCR-0028 — Disk-backed cache + document-level metadata (ticket f12b8b commissioned; slices SL-110..SL-114)](project/design-change-records/DCR-0028-disk-backed-cache-and-document-metadata.md)
- [DCR-0029 — `transync-anthropic`, the second provider crate (ticket bda471; slices SL-115..SL-119)](project/design-change-records/DCR-0029-transync-anthropic-second-provider.md)
- [DCR-0030 — Review 0004 hardening: a credential stops crossing origins, a server names the authority it answers for, and section identity becomes canonical](project/design-change-records/DCR-0030-review-0004-credential-and-origin-hardening.md)
- [DCR-0031 — An indented code block is normalized on translate: re-fenced for the wire, re-emitted fenced in the output (ticket 457e51)](project/design-change-records/DCR-0031-indented-code-normalize-on-translate.md)

## Project state

- [Status](project/status.md) — current phase and open actions.
- [Backlog](backlog.md) — the cross-source index of WIP / deferred / blocked items, maintained by the `/reopen` sweep; `status.md` names it as the index of open items, and the authoritative detail for `OI-`numbered entries stays in `open-issues.md`.
- [Phase state](project/phase-state.yaml) — machine-readable phase tracker.
- [Git history loss (2026-08-17)](project/git-history-loss-2026-08-17.md) — why this repository's commit graph begins on 2026-08-17: the cause named with physical evidence (a file-sync client writing conflict copies inside `.git`) and excluded — written, not yet proven — the recovery runbook that got 17 of 33 objects back, the 16 that were lost, where the archived object store lives, and what to do when a commit hash quoted in a ticket, ADR, DCR or CHANGELOG entry does not resolve.
- [Git history loss (2026-08-10)](project/git-history-loss-2026-08-10.md) — the *first* restart: what the 2026-08-10 loss of 30 objects took, what survived it, and where its archives live. Superseded as "where this history begins" by the 2026-08-17 record above, and still the only record of that event.
- [Release Checklist](project/release-checklist.md) — the tagged-release ritual: preflight, standing gates, the live-endpoint gate, CHANGELOG promotion, version bump, annotated tag.
- [Intake](project/intake.md) — initial intake notes.
- [Skeleton Plan](project/skeleton-plan.md) — Phase-3 plan governing Phase-4 generation.
- [Implementation Slice Checklists](project/implementation-slice-checklists.md) — SL-00..SL-14 done-gate criteria.
- [Stub Manifest](project/stub-manifest.md) — every `STUB` placeholder + closing slice.
- [Design Baseline `BL-2026-07-B`](project/design-baseline-2026-07.md) — **active** baseline: v0.2.0 steady state, authority hierarchy, live contract inventory.
- [Design Baseline `BL-2026-05-01-A`](project/design-baseline.md) — *(historical)* the 2026-05-01 implementation-ready handoff, superseded by `BL-2026-07-B`.
- [Open Issues](project/open-issues.md) — issues accepted but pending verification or larger architectural changes.
- [Open Issues — Archive](project/open-issues-archive.md) — resolved/lapsed issue records kept for audit.
- [Implementation Impact Report](project/implementation-impact-report.md) — implementation impact analysis.

## Brainstorming / specs

- [Transync Design Brainstorm (2026-05-01)](superpowers/specs/2026-05-01-transync-design.md) — *(historical)* the original brainstorming narrative; superseded where it disagrees with the active baseline.
- [Smooth Scroll Sync (2026-05-03)](superpowers/specs/2026-05-03-smooth-scroll-sync.md) — design for the per-frame lerp in the JS sync engine.
- [HTML-Content Translation (2026-08-03)](superpowers/specs/2026-08-03-html-content-translation-design.md) — approved v2 design spec for the shipped HTML-content translation feature; the decision is ADR-0018, the implementation record DCR-0016.
- [HTML-Content Translation — implementation plan (2026-08-03)](superpowers/plans/2026-08-03-html-content-translation.md) — the 16-task plan the feature shipped against.
- [Transync-Syntax Split (2026-08-04)](superpowers/specs/2026-08-04-transync-syntax-split-design.md) — design spec for extracting the parse/regen/render syntax layer into the `transync-syntax` crate behind a standing `wasm32` gate.
- [Transync-Syntax Split — implementation plan (2026-08-04)](superpowers/plans/2026-08-04-transync-syntax-split.md) — the 9-task plan the split is being executed against.
- [OI-0027 Public-Surface Hardening (2026-08-04)](superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md) — approved design spec for the facade curation, `#[non_exhaustive]` set, and surface-drift gate; the implementation record is DCR-0018.
- [OI-0027 Public-Surface Hardening — implementation plan (2026-08-04)](superpowers/plans/2026-08-04-oi0027-public-surface-hardening.md) — the 13-task plan the curation shipped against (amended in flight).
- [OI-0008 + OI-0033 Internal-Quality Wave (2026-08-05)](superpowers/specs/2026-08-05-oi0008-0033-internal-quality-design.md) — approved design spec for the lone-CR Support posture, the A1 retry/fallback policy module, and the six concern splits; the implementation record is DCR-0019.
- [OI-0008 + OI-0033 Internal-Quality Wave — implementation plan (2026-08-05)](superpowers/plans/2026-08-05-oi0008-0033-internal-quality.md) — the 12-task plan the wave shipped against.
- [Track C — In-Browser WASM Rendering + Edit Demo (2026-08-05)](superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md) — approved design spec for the `transync-wasm` crate, the JSON-string boundary, the build pipeline and size budget, and the web-only scope; the decision is ADR-0019, the implementation record DCR-0020.
- [Track C — WASM Render Demo — implementation plan (2026-08-05)](superpowers/plans/2026-08-05-track-c-wasm-render-demo.md) — the 6-task plan the wave shipped against.

## Browser demo

- [Manual SCN-13 smoke checklist](../web/SMOKE.md) — verify partner-pane sync visually in any browser.
- **WASM render+edit demo** — `web/demo-wasm.html`, built with `./scripts/build-wasm.sh` (needs wasm-pack and binaryen ≥ 121). Renders both panes locally through the same Rust renderer and edits translated blocks live; the decision is ADR-0019, the implementation record DCR-0020.
