# Review 0001 — finding dispositions

**Date:** 2026-08-06. **Source:** `reviews/reviewed/0001.md` (52 findings,
completed 2026-07-15). **Status of this file:** committed and archived.

> **Corrected 2026-08-29.** This header said "untracked, like its source — the
> decision on whether to commit the `reviews/` bundle is still open with the
> owner". Both halves were stale. The owner settled it on **2026-08-08**, two
> days after this file was written: `docs/investigation/` went back to
> untracked, but `reviews/` was **not** reversed with it — round 0001, its
> dispositions, its patch and `README.md` stay committed as the dated record
> they were made, "and a later round joins them when the gate archives it"
> (`docs/backlog.md`, `untracked-analysis-bundles`). This file has been tracked
> since `59ce8df`. The sentence outlived its decision by three weeks and was
> read as current at least twice.

## What this is

`reviews/0001.md` never got a systematic disposition pass, so genuinely-fixed and
genuinely-open findings were indistinguishable without re-verification
(`docs/backlog.md` Type 1: `review-0001-residual-findings-triage`). This table is
that pass. Every finding id in `reviews/0001.md` appears exactly once, with:

- **archived-resolved / resolved-by-later-work** — fixed before this wave; the
  mechanism is named.
- **fixed-this-wave** — fixed by the 2026-08-05/06 backlog wave; the commit is named.
- **backlog-tracked** — already carried in `docs/backlog.md` (usually because it
  needs an owner decision or is gated); no ticket, the backlog entry is the tracker.
- **ticketed** — confirmed still open; a TicGit ticket now exists (id given). Related
  findings share a ticket when they share a fix.
- **not-a-defect** — the premise no longer exists.

Nothing in this table was fixed by the triage itself: verification was read-only
except for the two fixes noted as landing in this wave, which landed before it.

## Verification note — an id collision that produced false "resolved" claims

`docs/project/open-issues-archive.md` cites `R0001-nnnn` ids from an **older Review
0001** that was "archived and removed" (dated 2026-05-04). Those ids collide
numerically with the current `reviews/0001.md` (2026-07-15) but mean different
things — archived `R0001-0030`, for example, is the image-block parsing issue, not
the Responses-status issue this review's `R0001-0030` describes.

`docs/backlog.md`'s claim that "only nine are archived as resolved
(R0001-0001/0004/0010/0013/0030/0036/0038/0039/0040)" inherits that collision. Each
was re-checked against current code for this pass: **only R0001-0001 is genuinely
resolved.** The other eight are open and are ticketed below. The backlog sentence
should be corrected when the controller next edits `docs/backlog.md`.

> **Note added 2026-08-07 (ticket `4af82bbc`).** The collision this section
> found reached further than `open-issues-archive.md`: it ran through code
> comments, rustdoc, the ADRs/DCRs and `CHANGELOG.md`. It is now swept, and
> `reviews/README.md` carries the round registry, the citation convention (a
> bare `R0001-NNNN` is *this* round; a retired-round id names
> `reviews/reviewed/0001.md`) and a finding index for the retired round.
> `docs/backlog.md`'s nine-resolved sentence no longer exists — the backlog was
> rewritten at the v0.3.0 release. Two further corrections to this file's own
> preamble: it and `reviews/0001.md` are **tracked** as of `727d3bc`, not
> untracked; and the retired round is dated 2026-05-02, not 2026-05-04 (the
> `(review archived and removed)` markers in `open-issues-archive.md` carry the
> issue's own date, not the review's).

## Dispositions

| ID | Title | Disposition | Mechanism / ticket |
|----|-------|-------------|--------------------|
| R0001-0001 | Temp-root values pass destructive `rm -rf` guards | resolved-by-later-work | `scripts/lib/workdir-guard.sh` — one shared guard requiring a **strict** descendant of an approved temp root (a root itself never matches), `..` rejected outright; the file's header names R0001-0001 as the root-equality fix it absorbed |
| R0001-0002 | Destructive guards trust an arbitrary `transync-*` basename | fixed-this-wave | commit `0f137ed` — deletion is authorized by containment or a marker file, never by name |
| R0001-0003 | Default profile forces Korean glossary terms | backlog-tracked | `docs/backlog.md` Type 2 `default-profile-korean-glossary` (owner decision: neutralize / comment out / keep) |
| R0001-0004 | Cache identity omits the enforced provider output ceiling | **ticketed** | `47d183d1` — `CacheKey` carries no output-budget axis while `target_output_tokens` is the enforced request ceiling (EXT-2026-07 P1-7). Backlog's "archived" claim is the id collision; re-verified open |
| R0001-0005 | Cache identity omits co-batched peers | backlog-tracked | `docs/backlog.md` Type 2 `cache-identity-batch-cohort` (trades away cross-run hits; needs a design decision) |
| R0001-0006 | Programmatic profiles bypass the section-scope invariant | fixed-this-wave | commit `c570d35` — the reserved scope is rejected on every entry path |
| R0001-0007 | Rejected provider attempts can determine the reported source language | **ticketed** | `badc976c` — the latch is written before `validate_batch` and never revisited |
| R0001-0008 | Deep blockquote structure can change without rejection | backlog-tracked | `docs/backlog.md` Type 3 `deep-blockquote-fingerprint-depth` (partially addressed by OI-0022; blocked on its observed-drift gate) |
| R0001-0009 | Inline raw HTML bypasses the raw-HTML invariant | resolved-by-later-work | `validate/inline.rs` collects every `HtmlInline` token and enforces an always-on, non-policy-gated tag-identity check (EXT-2026-07 P1-5) |
| R0001-0010 | `TranslateOptions.model_id` can disagree with the translator model | **ticketed** | `a60f0746` — still open; the cache-correctness half is mitigated (the OpenAI `fingerprint()` covers the real model), the tokenizer/label half is not |
| R0001-0011 | Cache-revalidation retries counted inconsistently | resolved-by-later-work | OI-0008 unified `policy::content_retry_count` and `report.rs`'s `retried_units`; pinned by `a_rejected_cache_hit_is_not_a_content_retry` |
| R0001-0012 | Validation retry hints are never re-budgeted | **ticketed** | `24fd28b4` — `group_by_token_budget`'s only call site is still the initial grouping |
| R0001-0013 | Token budgeting undercounts provider-visible input | **ticketed** | `24fd28b4` (same ticket) — largely addressed by R0008-0029 and the tokenized context/constraint estimates; residual: language labels folded into a fixed 256-token allowance (ADR-0013 permits arbitrarily long labels) and per-entry constraint constants |
| R0001-0014 | `ProfileMetadata` conflates template and compiled states | **ticketed** | `574a1947` — one type for both states; `render_prompt_body` re-appends its sections on every call |
| R0001-0015 | Glossary values can break the prompt's line structure | **ticketed** | `5f6664d4` — `escape_for_quoted` still escapes only `\` and `"` |
| R0001-0016 | A zero output-token ceiling is accepted and forwarded | **ticketed** | `3e972bb2` — no positivity check; `Some(0)` reaches `max_completion_tokens` / `max_output_tokens` |
| R0001-0017 | A zero unit cap is silently changed to one | **ticketed** | `3e972bb2` (same ticket) — `.max(1)` in `unit/budget.rs::resolve`, documented by its own test as intended |
| R0001-0018 | Empty glossary source/target terms are accepted | **ticketed** | `5f6664d4` (same ticket) — `load_profile` performs no content check on static entries |
| R0001-0019 | Conflicting glossary entries accepted without warning | **ticketed** | `5f6664d4` (same ticket) — `merge_auto_glossary`'s static-wins rule never compares two static entries |
| R0001-0020 | CLI prompt overrides bypass template-variable diagnostics | **ticketed** | `ed8c572f` — `resolve_profile` overwrites `prompt_body` after `load_warnings` is computed |
| R0001-0021 | Whitespace prevents the `auto` language sentinel from working | **ticketed** | `ed8c572f` (same ticket) — the sentinel check does not trim; `publish.rs` trims, the prompt path does not |
| R0001-0022 | Heading context deletes literal leading `#` content | **ticketed** | `974d199a` — mechanism confirmed with a corrected repro: ATX is protected by the mandatory marker space, **setext** headings whose text starts with `#` genuinely lose it |
| R0001-0023 | "Plain" heading context still contains Markdown syntax | **ticketed** | `974d199a` (same ticket) — closing ATX marker and inline syntax both survive the trim |
| R0001-0024 | The OpenAI adapter drops typed context metadata | **ticketed** | `53d4956f` — now broader than OpenAI: `ContextHints` is the shared builder in `llm/prompt.rs` and carries no level or kind |
| R0001-0025 | List nesting depth overflows `u8` | **ticketed** | `d7658be4` — unchecked `depth + 1` on `u8` in `structure.rs` |
| R0001-0026 | Reconstructed task lists lose Comrak's semantic CSS classes | **ticketed** | `98f9ecfd` — `render_list_group` emits no class on either the group tag or the item |
| R0001-0027 | Validation-report JSON has no version discriminator | **ticketed** | `096e0e0e` — `ValidationReport` is serialized as the JSON root with no `schema_version` |
| R0001-0028 | Full-reparse fallback IDs ordered by kind, not document | **ticketed** | `096e0e0e` (same ticket) — OI-0021's `doc_order` fix was applied to `per_unit` only |
| R0001-0029 | Standard HTTP-date `Retry-After` values are ignored | **ticketed** | `77ccf109` — delta-seconds only, pinned by a test that asserts the rejection |
| R0001-0030 | Responses terminal statuses other than `incomplete` lose their reason | **ticketed** | `77ccf109` (same ticket) — the skip list's "archived" is the id collision; `output_from_envelope` still branches only on `incomplete`, and the envelope models no `error` |
| R0001-0031 | API-surface env changes desynchronize fingerprint and request | **ticketed** | `a60f0746` (same ticket) — `api_from_env_or_model` is read fresh by `fingerprint()` and by the request path |
| R0001-0032 | CLI discards operational `tracing` warnings | **ticketed** | `7f922fa1` — no `tracing` dep, no subscriber; the warn-site count has grown 13 → 16 |
| R0001-0033 | Documented OpenAI constructor surface omits the checked constructor | **ticketed** | `156ff586` — contracts §7 still lists only `new`/`from_env`; the CLI's own use of `try_new` mitigates runtime risk but not the contract |
| R0001-0034 | Concurrent individual-output commits can publish a mixed run | **ticketed** | `f41f1652` — no lock, lease or generation marker over the rename phase |
| R0001-0035 | Existing `--out-dir` replacement has a visible missing-target window | **ticketed** | `f41f1652` (same ticket) — two sequential renames; contracts.md now describes it honestly, the `--out-dir` help text still says "atomically" |
| R0001-0036 | Stale bundle temps from old PIDs are never cleaned | **ticketed** | `f41f1652` (same ticket) — foreign-pid temps are deliberately skipped (R0008-0007) while contracts §6 still promises automatic removal; skip-list "archived" is the id collision |
| R0001-0037 | The installed pre-commit hook never checks the actual web package | resolved-by-later-work | commit `bda8c1a` — the hook searches `.` and `web` for `package.json` and runs the JS checks from there |
| R0001-0038 | Hook installation overwrites an existing hooks configuration | **ticketed** | `097c20c5` — `git config core.hooksPath` is still written unconditionally; skip-list "archived" is the id collision |
| R0001-0039 | Common API-key environment files remain trackable | **ticketed** | `097c20c5` (same ticket) — `.gitignore` still covers only `.env`, `.env.local`, `*.api_key` |
| R0001-0040 | Internal path-only dependencies block registry publication | **ticketed** | `c447700f` — path deps carry no `version`; only `transync-wasm` declares `publish = false` |
| R0001-0041 | Two default-profile files have no drift enforcement | **ticketed** | `c0d6fead` — the CLI copy is referenced by nothing and has drifted further |
| R0001-0042 | Browser synchronization ignores `target_block_id` | backlog-tracked | `docs/backlog.md` Type 2 `oi-0015-sync-module-split-and-id-identity` — the ID-identity pairing question is exactly this finding and needs the owner's normative decision; confirmed still present in `sync.js` |
| R0001-0043 | Alignment loading validates only the version string | **ticketed** | `4f8dde0c` — `loadAlignment` never inspects `blocks` |
| R0001-0044 | Duplicate DOM anchors remain active after only a warning | **ticketed** | `4f8dde0c` (same ticket) — `indexById` keeps the last, the scan arrays keep them all |
| R0001-0045 | Public range conversion can overflow before clamping | **ticketed** | `d7658be4` (same ticket) — `pos_to_byte` adds before it clamps, on public API |
| R0001-0046 | Module map publishes the obsolete cache v1 signature | resolved-by-later-work | `docs/implementation/module-map.md` now reproduces the three-method fallible `Cache` trait and the `Mutex<HashMap>` impl verbatim |
| R0001-0047 | Module map claims browser automation is still deferred | **ticketed** | `156ff586` (same ticket) — the SCN-13 rows still say manual-only, contradicting mvp-scope.md and the shipped Playwright suite |
| R0001-0048 | Module map labels a no-op pagination stub as split logic | not-a-defect | Premise gone: `crates/transync-openai/src/pagination.rs` was deleted 2026-08-05 (DCR-0019) and the module map no longer mentions pagination anywhere; the deferral itself stays tracked in `stub-manifest.md` (STUB-045) |
| R0001-0049 | ADR-0002 points the provider boundary at the wrong crate | **ticketed** | `156ff586` (same ticket) — the trait is declared in `transync-core::llm`, only re-exported by the facade |
| R0001-0050 | ADR-0004 describes a serializer-based regenerator that does not exist | **ticketed** | `156ff586` (same ticket) — `regen.rs` is source-range splicing plus fence re-wrap, no serializer path |
| R0001-0051 | ADR-0006 names removed output APIs and placeholders | **ticketed** | `156ff586` (same ticket) — `write_html_bundle` → `html_bundle_files` (12545e3); no `ALIGNMENT_MAP_REL` in the template |
| R0001-0052 | Rust compatibility rules in the locked contract are incorrect | resolved-by-later-work | `7f62cb9` made `TranslatorError` `#[non_exhaustive]`; `507ee43` rewrote contracts.md to the correct variant-additions-non-breaking / fields-breaking-by-policy rules |

## Totals

| Disposition | Count | IDs |
|---|---|---|
| resolved before this wave | 6 | 0001, 0009, 0011, 0037, 0046, 0052 |
| fixed in the 2026-08-05/06 wave | 2 | 0002, 0006 |
| backlog-tracked (no ticket) | 4 | 0003, 0005, 0008, 0042 |
| not-a-defect | 1 | 0048 |
| ticketed | 39 | everything else, in 21 tickets |

## Tickets created (21)

| Ticket | Findings | Area |
|---|---|---|
| `badc976c` | 0007 | core-pipeline |
| `24fd28b4` | 0012, 0013 | core-batch |
| `574a1947` | 0014 | core-profile |
| `5f6664d4` | 0015, 0018, 0019 | core-profile |
| `3e972bb2` | 0016, 0017 | core-profile |
| `47d183d1` | 0004 | cache |
| `ed8c572f` | 0020, 0021 | cli |
| `974d199a` | 0022, 0023 | core-pipeline |
| `53d4956f` | 0024 | core-prompt |
| `d7658be4` | 0025, 0045 | core-syntax |
| `98f9ecfd` | 0026 | syntax-render |
| `096e0e0e` | 0027, 0028 | core-pipeline |
| `77ccf109` | 0029, 0030 | provider-openai |
| `a60f0746` | 0010, 0031 | provider-openai |
| `7f922fa1` | 0032 | cli |
| `156ff586` | 0033, 0047, 0049, 0050, 0051 | docs |
| `f41f1652` | 0034, 0035, 0036 | cli |
| `c0d6fead` | 0041 | cli |
| `4f8dde0c` | 0043, 0044 | web-js |
| `097c20c5` | 0038, 0039 | scripts |
| `c447700f` | 0040 | packaging |

All carry the `review-0001` tag plus an area tag. Priorities follow the review's
severity (High → 2, Medium → 3, Low → 4).

## Verdicts overturned during synthesis

Eight findings the triage inputs carried as already-dispositioned were re-checked
against current code and found open; all eight trace to the archive id collision
described above.

| ID | Carried as | Re-verified as | Evidence |
|---|---|---|---|
| R0001-0004 | archived-resolved | open | `CacheKey` has no output-budget field; `CacheKeyContext::for_run` adds none |
| R0001-0010 | archived-resolved | open (partly mitigated) | `TranslateOptions.model_id` still caller-owned; `Translator` exposes no model identity; the OpenAI `fingerprint()` covers cache correctness only |
| R0001-0013 | archived-resolved | open (largely addressed) | `FIXED_ENVELOPE_TOKENS = 256` still stands in for the language labels; constraint vectors costed by per-entry constants |
| R0001-0030 | archived-resolved | open | `output_from_envelope` branches only on `incomplete`; `Envelope` models no `error` |
| R0001-0036 | archived-resolved | open | `scan_bundle_dir` removes own-pid temps only; contracts §6 still says leftovers are removed automatically |
| R0001-0038 | archived-resolved | open | `install-hooks.sh` writes `core.hooksPath` unconditionally |
| R0001-0039 | archived-resolved | open | `.gitignore` covers `.env`, `.env.local`, `*.api_key` only |
| R0001-0040 | archived-resolved | open | path deps carry no `version`; no `publish = false` outside `transync-wasm` |

Two further adjustments, neither an evidence overturn:

- **R0001-0022** — the review's literal repro (`# #hashtag`) does not reproduce: the
  mandatory ATX marker space stops the trim. The defect is real via **setext**
  headings whose body starts with `#`. The ticket carries the corrected repro.
- **R0001-0042** — verified still present, but reclassified from "ticket" to
  "backlog-tracked": `docs/backlog.md`'s `oi-0015-sync-module-split-and-id-identity`
  already carries exactly this question and gates it on an owner decision. Filing a
  ticket would have duplicated a tracked decision item.

## Follow-ups for the controller

- `docs/backlog.md`'s `review-0001-residual-findings-triage` entry: the "only nine
  are archived as resolved" sentence is wrong (eight of the nine are id collisions
  with the removed 2026-05-04 review). Correct it when closing the item, and point at
  this file.
- `docs/project/open-issues-archive.md` cites `R0001-nnnn` ids from the removed
  review with no disambiguation. A one-line note at the top of that file ("R0001 ids
  here refer to the 2026-05-04 review, archived and removed; the current
  `reviews/0001.md` re-uses the same numbering") would prevent the next agent from
  repeating this mistake.
