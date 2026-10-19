---
okf_version: "0.2"
---
# transync — Documentation Index

The OKF v0.2 bundle-root index: the machine-facing navigation of everything under `docs/`. Update this file whenever a new doc lands so nothing becomes orphaned — `crates/transync/tests/docs_index_drift.rs` enforces both halves of that rule: every Markdown document under `docs/` must be linked from here, and every link from here must resolve.

The hand-curated human entry point is a separate document, [`project-index.md`](project-index.md); this one is regenerable from frontmatter and never folds that one in.

Four things sit outside the rule on purpose, because they are not this index's to carry, and the test excludes exactly them: `*.ko.md` Korean siblings (generated translations, git-ignored); git-ignored generated bundles under `docs/` — today `docs/investigation/`, regenerated from the code and untracked — which the test reads out of `.gitignore` rather than hardcoding, so a re-tracked bundle is required again the day its ignore line goes; and the two OKF reserved files, this one and `log.md`, which are navigation and journal rather than concepts. Non-Markdown files are not *required*, which is why `project/phase-state.yaml` appears below because a reader wants it rather than because the test demands it.

## Project control

- [Project Documentation Index](project-index.md) — reading order, authority order, and what is deliberately not written down.
- `log.md` — the bundle change journal (OKF reserved; carries no frontmatter).

## For developers (start here)

- [Quick Start](Quick_Start.md) — clone, build, test, first translate in 5 minutes.
- [Developer Guide](Developer_Guide.md) — library use, full CLI reference, system prompts, profile TOML, `Translator` impls, CSS/JS hooks.
- [Profile Cookbook](Profile_Cookbook.md) — drop-in TOML recipes per document style, plus a section-scoped glossary recipe.
- [Performance Tuning](Performance.md) — the three knobs that move wall-clock; provider-floor latency; peak memory.
- [Troubleshooting](Troubleshooting.md) — symptom-keyed: silent exits, `model_not_found`, schema refusals, all-fallback runs, sync freezes.

## Architecture

- [Architecture Overview](architecture/README.md) — components, data flow, and pointers to deeper docs.
- [MVP Scope](architecture/mvp-scope.md) — what is in and out of scope for the first usable version.
- [Scenario Matrix](architecture/scenario-matrix.md) — concrete scenarios driving design and validation.
- [Source-of-Truth Table](architecture/source-of-truth-table.md) — ownership of every concern.
- [Rough Schema](architecture/rough-schema.md) — every wire / durable type.
- [Persistence and Files](architecture/persistence-and-files.md) — file lifecycle + atomic-write contract.
- [Contracts](architecture/contracts.md) — `Translator`, profile TOML, alignment-map JSON, HTML attrs, retry policy, CLI args.
- [Settled Questions](architecture/settled-questions.md) — recurring review findings the record already answers, each with its citation.
- [transync-html divergences](architecture/transync-html-divergences.md) — the twenty-two deliberate divergences from the retired `htmlseg`, with date, ticket, record.

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
- ADR 0014 is archived, superseded by DCR-0027 — see [Archive](#archive).
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
- [ADR 0025 — HTML documents are a second intake into one pipeline, and semantic kind is orthogonal to source spelling](decisions/0025-html-to-html-document-translation.md)
- [ADR 0026 — The browser suite is Chromium-only, and that is accepted rather than unnoticed](decisions/0026-browser-matrix-is-chromium-only.md) — the three conditions that reopen it; the cross-browser subset pre-scoped (R0009-0020).
- [ADR 0027 — `mountSync` refuses a map no anchor can drive, but not panes no row can drive](decisions/0027-mount-refusal-is-asymmetric-by-design.md) — an unusable map is terminal; unmatched panes are transient (R0009-0023).
- [ADR 0028 — The source-language gate is a pre-flight question, not a second detection](decisions/0028-the-language-gate-is-a-pre-flight-question.md) — no language in the verdict, no serialization, no workspace-member dependency (ticket `e4f4b0`).
- [ADR 0029 — The out-dir ownership marker is a name, not a credential](decisions/0029-the-out-dir-marker-is-a-name-not-a-credential.md) — filename and file type decide; its body is documentation and is never compared (R0011-0003).
- [ADR 0030 — The WASM publish lock fails closed, and the remedy is a human](decisions/0030-the-wasm-publish-lock-fails-closed.md) — no automatic stale-owner recovery: only `SIGKILL` can strand the lock (R0011-0016).
- [ADR 0031 — A provider request budget is taken verbatim](decisions/0031-a-provider-request-budget-is-taken-verbatim.md) — no floor or clamp; `Duration::ZERO` fails every request (R0011-0075).

## Design change records (DCRs)

Archived records — DCR-0001–0004, 0006, 0007, 0010, 0011, 0013–0015, 0017–0019, 0021–0023, 0030–0034, 0036–0038 and 0052 — are under [Archive](#archive).

- [DCR-0005 — Facade crate + `transync-core` split](project/design-change-records/DCR-0005-facade-and-core-crate-split.md)
- [DCR-0008 — Scroll-listener sync engine replaces the IntersectionObserver design](project/design-change-records/DCR-0008-scroll-listener-sync-engine.md)
- [DCR-0009 — v0.2 provider/cache boundary (fingerprint, fallible Cache + evict, RetryContext)](project/design-change-records/DCR-0009-v02-provider-cache-boundary.md)
- [DCR-0012 — Output-aware batch packing + CLI batching knobs](project/design-change-records/DCR-0012-output-aware-batching-and-cli-knobs.md)
- [DCR-0016 — HTML-content translation via segment extraction (alignment schema 1.2.0)](project/design-change-records/DCR-0016-html-content-translation.md)
- [DCR-0020 — Track C wasm render+edit demo (`transync-wasm`, build pipeline, browser demo, parity gates)](project/design-change-records/DCR-0020-track-c-wasm-demo.md)
- [DCR-0024 — Run cancellation (supersedes DCR-0009's YAGNI deferral; a cancelled run answers `Err`)](project/design-change-records/DCR-0024-run-cancellation.md)
- [DCR-0025 — Review 0003 hardening (content loss closed at four seams; a refusal outranks the text beside it)](project/design-change-records/DCR-0025-review-0003-content-loss-and-refusal-precedence.md)
- [DCR-0026 — Oversize tables split into header-carrying row windows at packing time (STUB-017 commissioned; slices SL-100..SL-105)](project/design-change-records/DCR-0026-oversize-table-row-window-split.md)
- [DCR-0027 — Batches become section-coherent, and the glossary section scope becomes real (ticket 43cfb4; slices SL-106..SL-109)](project/design-change-records/DCR-0027-section-coherent-batching-and-glossary-section-scope.md)
- [DCR-0028 — Disk-backed cache + document-level metadata (ticket f12b8b commissioned; slices SL-110..SL-114)](project/design-change-records/DCR-0028-disk-backed-cache-and-document-metadata.md)
- [DCR-0029 — `transync-anthropic`, the second provider crate (ticket bda471; slices SL-115..SL-119)](project/design-change-records/DCR-0029-transync-anthropic-second-provider.md)
- [DCR-0035 — HTML intake + identity round-trip (ti 490d97 wave 3): `transync-syntax::intake::html` lands beside `parser`, and `regenerate(parse, &empty)` is byte-identical](project/design-change-records/DCR-0035-html-intake-identity-round-trip.md) — five-class classification, default STOP; its deferred `parser` rename is paid by DCR-0053.
- [DCR-0039 — the browser gate closes the HTML→HTML feature, and the ledger says what stayed open (2026-09-04)](project/design-change-records/DCR-0039-html-browser-gate-and-closure.md) — ti `490d97` wave 7, the last: `scn16.spec.js`, proven able to fail, and the deferred-item ledger.
- [DCR-0040 — Review 0009's fix pass changes three shipped behaviours, one of them a breaking API removal](project/design-change-records/DCR-0040-review-0009-fix-pass.md) — the splice's agreement check becomes an error, `serve` gains a deadline, `RateLimited` is removed.
- [DCR-0041 — The element walk models HTML integration points and foreign-content breakout](project/design-change-records/DCR-0041-foreign-content-islands-and-breakout.md) — closes a live `contracts.md` §4a break reachable from untrusted Markdown (ticket `e77173`).
- [DCR-0042 — Raw text and RCDATA are HTML-content states, and the scanner owns the content-mode model](project/design-change-records/DCR-0042-raw-text-is-an-html-content-state.md) — inside `svg`/`math`, raw-text/RCDATA names are ordinary foreign elements (ticket `2e2453`).
- [DCR-0043 — Voidness is an HTML-content rule, and the breakout model already retired the hazard that kept it global](project/design-change-records/DCR-0043-voidness-is-an-html-content-rule.md) — safe because of DCR-0041's breakout model (ticket `48f3c6`).
- [DCR-0044 — The alignment row decides whether a block gets a DOM anchor, and `sync_role_for` names every `BlockKind` variant](project/design-change-records/DCR-0044-the-row-decides-the-anchor.md) — keyed on the row's own `sync_role`; no `_ => Anchor` catch-all (tickets `18b9c3`, `9ffb97`).
- [DCR-0045 — `transync-lang` joins the workspace as the source-language gate](project/design-change-records/DCR-0045-transync-lang-source-language-gate.md) — script test first, `whichlang` second; depends on no workspace member (ticket `e4f4b0`).
- [DCR-0046 — An offline run against a warm cache is supported behind `--offline`](project/design-change-records/DCR-0046-offline-runs-from-a-warm-cache.md) — byte-identical fingerprint, since `CacheKey` namespaces on it (ticket `30a744`, OI-0038).
- [DCR-0047 — `balance_fragment` repairs a fragment that ends inside an unterminated region](project/design-change-records/DCR-0047-the-balancer-repairs-an-unterminated-trailing-region.md) — `TagToken::Skip` carries the region's kind and terminated-ness (ticket `c1f9a8`).
- [DCR-0048 — Visible-text presence becomes a per-unit validation layer](project/design-change-records/DCR-0048-visible-text-presence-is-a-validation-layer.md) — source had visible text, translation has none: rejected, for both intakes (ticket `c887bc`).
- [DCR-0049 — The v0.5.0 blocker clearance, and why the arc does not reach a tag](project/design-change-records/DCR-0049-v050-blocker-clearance.md) — OI-0039..OI-0048 dispositioned; biome **format-only** (its lint half: DCR-0056).
- [DCR-0050 — Four `transync-html` tokenizer divergences close, and PLAINTEXT gets the arm ti `c1f9a8`'s rule never had](project/design-change-records/DCR-0050-four-tokenizer-divergences-and-the-plaintext-decision.md) — PLAINTEXT is deleted, start tag and all (ticket `bebebe`; R0010-0031/-0032/-0034/-0049).
- [DCR-0051 — One open-element stack: `transync-html`'s tree construction becomes browser-faithful](project/design-change-records/DCR-0051-one-open-element-stack.md) — adoption agency deliberately absent; residual in `contracts.md` §4b, owned by ti `525bef`.
- [DCR-0053 — The format seam becomes symmetric: the Markdown intake is `intake::markdown`](project/design-change-records/DCR-0053-the-format-seam-becomes-symmetric.md) — pays DCR-0035's deferred rename, no behaviour change; collision left to OI-0034 (`b7e4cc70`).
- [DCR-0054 — Four document kinds: a document's kind decides how it changes](project/design-change-records/DCR-0054-four-document-kinds.md) — living, register, record, archive; records become write-once, ADR decision bodies become living.
- [DCR-0055 — The docs bundle adopts OKF v0.2](project/design-change-records/DCR-0055-docs-becomes-an-okf-bundle.md) — write-once governs a Record's words, not its metadata header; `index.md` becomes the bundle root.
- [DCR-0056 — The JavaScript lint half is adopted](project/design-change-records/DCR-0056-javascript-lint-half-adopted.md) — the successor to DCR-0049's still-open half: the linter goes on with `--error-on-warnings`.
- [DCR-0057 — Adjacent sections coalesce into one batch](project/design-change-records/DCR-0057-sections-coalesce-into-batches.md) — settles DCR-0027's OQ-A as option (b), on by default; `coalesce_sections = false` restores P2.
- [DCR-0058 — Gemini 3.8 Flash through the Google Developer and Vertex APIs](project/design-change-records/DCR-0058-transync-google-gemini-provider.md) — one `transync-google` crate, explicit CLI provider/surface selection, Vertex the Google default.
- [DCR-0059 — A model's generation is a floor, not a roster](project/design-change-records/DCR-0059-model-generation-is-a-floor.md) — any `o<N>`, and `gpt-<N>` from 5 up, reach Responses and `o200k_base` unlisted.
- [DCR-0060 — Vertex AI express mode is a third Google surface](project/design-change-records/DCR-0060-vertex-express-mode.md) — `--google-auth api-key` with `GOOGLE_API_KEY`; an ambient key never selects it.
- [DCR-0062 — The v0.6.0 breaking window is open](project/design-change-records/DCR-0062-v060-breaking-window-opens.md) — the two parked changes may ship; the three queued items are eligible, not required.
- [DCR-0061 — Five corrections to the Gemini adapter, and what the first live call settled](project/design-change-records/DCR-0061-gemini-review-fix-pass.md) — an impossible ceiling is exit 2, the `MAX_TOKENS` advice stops being circular, `[::1]` is loopback again, and express mode's header form is live-verified.

## Project state

- [Status](project/status.md) — current phase and open actions.
- [Backlog](backlog.md) — open-items index (`OI-` detail: `open-issues.md`); header: **owner's v0.5.0 release condition**.
- [Phase state](project/phase-state.yaml) — machine-readable phase tracker.
- [Superseded citations](project/superseded-citations.md) — forwarding addresses for names and claims in write-once records that no longer resolve.
- [Git history loss (2026-08-17)](project/git-history-loss-2026-08-17.md) — why history begins here: cause, recovery, the authoritative `.git` is GitHub, unresolvable hashes.
- [Git history loss (2026-08-10)](project/git-history-loss-2026-08-10.md) — the *first* restart (30 objects lost), still the only record of that event.
- [Release Checklist](project/release-checklist.md) — the tagged-release ritual: preflight with step 2a's window-closing condition, the gates, the tag.
- [Intake](project/intake.md)
- [Skeleton Plan](project/skeleton-plan.md) — Phase-3 plan governing Phase-4 generation.
- [Implementation Slice Checklists](project/implementation-slice-checklists.md) — SL-00..SL-14 done-gate criteria.
- [Stub Manifest](project/stub-manifest.md) — every `STUB` placeholder + closing slice.
- [Design Baseline `BL-2026-07-B`](project/design-baseline-2026-07.md) — **active** baseline: v0.2.0 steady state, authority hierarchy, live contract inventory.
- [Design Baseline `BL-2026-05-01-A`](project/design-baseline.md) — *(historical)* the 2026-05-01 implementation-ready handoff, superseded by `BL-2026-07-B`.
- [Open Issues](project/open-issues.md) — issues accepted but pending verification or larger architectural changes.
- [Implementation Impact Report](project/implementation-impact-report.md)

## Brainstorming / specs

- [Transync Design Brainstorm (2026-05-01)](superpowers/specs/2026-05-01-transync-design.md) — *(historical)* the original brainstorm; superseded where it disagrees with the active baseline.
- [Smooth Scroll Sync (2026-05-03)](superpowers/specs/2026-05-03-smooth-scroll-sync.md) — design for the per-frame lerp in the JS sync engine.
- [HTML-Content Translation (2026-08-03)](superpowers/specs/2026-08-03-html-content-translation-design.md) — approved v2 spec; decision ADR-0018, implementation record DCR-0016.
- [Transync-Syntax Split (2026-08-04)](superpowers/specs/2026-08-04-transync-syntax-split-design.md) — the parse/regen/render layer into `transync-syntax`, behind a standing `wasm32` gate.
- [OI-0027 Public-Surface Hardening (2026-08-04)](superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md) — approved spec; implementation record DCR-0018.
- [OI-0008 + OI-0033 Internal-Quality Wave (2026-08-05)](superpowers/specs/2026-08-05-oi0008-0033-internal-quality-design.md) — approved spec; implementation record DCR-0019.
- [Track C — In-Browser WASM Rendering + Edit Demo (2026-08-05)](superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md) — approved spec; decision ADR-0019, implementation record DCR-0020.
- [HTML→HTML Document Translation (2026-08-20)](superpowers/specs/2026-08-20-html-to-html-translation-design.md) — ADR-0025, DCR-0032..0039. **The feature is implemented and complete**; its v0.5.0 window is closed.
- [Documentation Compaction (2026-09-13)](superpowers/specs/2026-09-13-docs-compaction-design.md) — owner-ratified: four document kinds, write-once records; implementation record DCR-0054.

## Browser demo

- [Manual SCN-13 smoke checklist](../web/SMOKE.md) — verify partner-pane sync visually in any browser.
- **WASM render+edit demo** — `web/demo-wasm.html`, built by `./scripts/build-wasm.sh` (wasm-pack, binaryen ≥ 121); ADR-0019.

## Archive

Verbatim and append-only; kept linked so nothing is orphaned, never read for current truth.

- [ADR 0014 — `conditional-on-section` glossary scope: the record of the rejection era](decisions/archive/0014-section-scoped-glossary-renders-globally.md) *(archived)* — superseded by DCR-0027, which re-admitted the scope.
- [DCR-0001 — Renderer wrapper-div for tables and code blocks](project/design-change-records/archive/DCR-0001-renderer-wrapper-div.md) *(archived)*
- [DCR-0002 — Post-regen full-reparse failure becomes recoverable](project/design-change-records/archive/DCR-0002-recoverable-full-reparse-failure.md) *(archived)*
- [DCR-0003 — Annotated-HTML wrapper positioning contract](project/design-change-records/archive/DCR-0003-wrapper-positioning-contract.md) *(archived)*
- [DCR-0004 — `FallbackPerBlock` widens to neighbors and auto-escalates](project/design-change-records/archive/DCR-0004-fallback-per-block-widens-and-escalates.md) *(archived)*
- [DCR-0006 — CLI outputs commit all-or-nothing](project/design-change-records/archive/DCR-0006-all-or-nothing-output-staging.md) *(archived)*
- [DCR-0007 — Grouped list rendering with `<li>` sync anchors](project/design-change-records/archive/DCR-0007-grouped-list-rendering.md) *(archived)*
- [DCR-0010 — Inline-protection validation layer (destinations + pledged code spans)](project/design-change-records/archive/DCR-0010-inline-protection-layer.md) *(archived)*
- [DCR-0011 — Staged fileset commit, `--out-dir` publish, CLI admission limits](project/design-change-records/archive/DCR-0011-staged-commit-out-dir-and-limits.md) *(archived)*
- [DCR-0013 — Reader-honesty placeholders + reference-link (refmap) resolution](project/design-change-records/archive/DCR-0013-reader-honesty-placeholders-and-refmap.md) *(archived)*
- [DCR-0014 — Auto-glossary preflight + per-batch schema-fault fairness](project/design-change-records/archive/DCR-0014-auto-glossary-and-batch-fault-fairness.md) *(archived)*
- [DCR-0015 — Prompt lift into core, tokenizer hint, RTL pane direction, gated live smoke](project/design-change-records/archive/DCR-0015-prompt-lift-tokenizer-hint-rtl-live-smoke.md) *(archived)*
- [DCR-0017 — `transync-syntax` crate split + AST-direct renderer (wasm32 gate)](project/design-change-records/archive/DCR-0017-transync-syntax-split.md) *(archived)*
- [DCR-0018 — Public-surface curation (explicit facade, compiler-enforced tiers, `#[non_exhaustive]`)](project/design-change-records/archive/DCR-0018-public-surface-curation.md) *(archived)*
- [DCR-0019 — OI-0008 + OI-0033 internal-quality wave (CR-aware line table, pure policy module, six concern splits)](project/design-change-records/archive/DCR-0019-internal-quality-wave.md) *(archived)*
- [DCR-0021 — Publication lock over output directories, honest `--out-dir` replacement wording, non-reclaimed staging temps](project/design-change-records/archive/DCR-0021-publication-lock-and-honest-replacement.md) *(archived)*
- [DCR-0022 — Review 0002 hardening (the render seam becomes fallible, and three guards stop trusting their input)](project/design-change-records/archive/DCR-0022-review-0002-hardening.md) *(archived)*
- [DCR-0023 — Provider error taxonomy (five terminal causes named, `stable_code()` reaches them)](project/design-change-records/archive/DCR-0023-provider-error-taxonomy.md) *(archived)*
- [DCR-0030 — Review 0004 hardening: a credential stops crossing origins, a server names the authority it answers for, and section identity becomes canonical](project/design-change-records/archive/DCR-0030-review-0004-credential-and-origin-hardening.md) *(archived)*
- [DCR-0031 — An indented code block is normalized on translate: re-fenced for the wire, re-emitted fenced in the output (ticket 457e51)](project/design-change-records/archive/DCR-0031-indented-code-normalize-on-translate.md) *(archived)*
- [DCR-0032 — The HTML mechanics become a workspace member, and the record names what stopped being called htmlseg (ticket 490d97)](project/design-change-records/archive/DCR-0032-transync-html-crate-extraction.md) *(archived)*
- [DCR-0033 — An anchor is trusted because the map claims it, and because we put it there: OI-0035 closed at both layers (ti 490d97 wave 1)](project/design-change-records/archive/DCR-0033-oi0035-anchor-trust-both-layers.md) *(archived)*
- [DCR-0034 — Semantic kind and source spelling become separate axes in the block IR](project/design-change-records/archive/DCR-0034-ir-semantic-kind-and-spelling-split.md) *(archived)*
- [DCR-0036 — The HTML layer-6 twin (ti 490d97 wave 4): `full_rescan_html` gates HTML regen output, and `finalize` dispatches on `Document.format`](project/design-change-records/archive/DCR-0036-html-layer6-twin.md) *(archived)*
- [DCR-0037 — HTML→HTML works at the library surface (ti 490d97 wave 5): `TranslateOptions.input_format`, `[system].prompt_html`, the run-level instruction clause, extract-projected context, and the no-new-cache-axis verdict](project/design-change-records/archive/DCR-0037-html-translation-entry-units-context-prompt.md) *(archived)*
- [DCR-0038 — HTML panes, alignment wire 1.3.0, and the CLI surface (2026-09-03)](project/design-change-records/archive/DCR-0038-html-panes-wire-cli.md) *(archived)*
- [DCR-0052 — The bottom clamp is the contract; a browser test that asserted the dead zone is corrected, not the engine](project/design-change-records/archive/DCR-0052-the-bottom-clamp-is-the-contract.md) *(archived)*
- [Status archive](project/status-archive.md) — *(archive)* the wave log and every completed action.
- [Phase-state run log](project/phase-state-notes.md) — *(archive)* the narrated history of every wave and release.
- [Backlog — Archive](backlog-archive.md) — *(archive)* entries moved out of `backlog.md` by the commit that closed them, verbatim.
- [Open Issues — Archive](project/open-issues-archive.md) — resolved/lapsed issue records kept for audit.
- [Release checklist archive](project/release-checklist-archive.md) — *(archive)* the v0.4.0 untagged-then-tagged decisions and the step notes they made spent.
- [Developer Guide — Archive](Developer_Guide-archive.md) — *(archive)* v0.2/v0.4 migration notes and dated history moved out of the guide.
- [Troubleshooting — Archive](Troubleshooting-archive.md) — *(archive)* passages moved out of `Troubleshooting.md`, verbatim, under the section they left.
- [ADR amendments](decisions/archive/adr-amendments.md) — *(archive)* every ADR amendment section, verbatim, in ADR-number order.
- [Contracts history](architecture/archive/contracts-history.md) — *(archive)* text moved out of `contracts.md`, verbatim, by section and record id.
