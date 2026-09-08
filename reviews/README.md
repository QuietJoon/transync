# Review rounds — registry and citation convention

Independent-review findings are cited all over this repository — in code
comments, rustdoc, ADRs, DCRs, `CHANGELOG.md` and the project docs — as
`R<round>-<finding>`, e.g. `R0001-0025`. This file says which round each id
belongs to and how to write a citation that stays resolvable.

It exists because **two different review rounds numbered themselves `0001`**,
so for finding numbers `0001`–`0052` a bare `R0001-NNNN` had two meanings and
a reader chasing one landed in the wrong finding. That produced at least one
false "mis-cited id" report before it was noticed.

## The convention

1. **A bare `R0001-NNNN` means `reviews/reviewed/0001.md`** — the round that is on
   disk. Nothing else may be spelled bare.
2. **A citation of the retired 2026-05-02 round names its file**, the shape
   `crates/transync-core/src/unit/context.rs`,
   `crates/transync-core/src/validate/full_reparse.rs` and
   `crates/transync-core/src/pipeline/report.rs` already use:

   > `R0001-0026` in the removed `reviews/reviewed/0001.md`

3. **A finding number above `0052` is a retired-round id wherever it
   appears.** `reviews/reviewed/0001.md` has exactly 52 findings, so `R0001-0079` can
   only be the 2026-05-02 round; the table below resolves it.
4. **`R0002`, `R0003` and `R0004` were each used TWICE, and a bare id means
   the 2026-08 gated round.** The 2026-05 rounds are the marked side, exactly
   as rule 2 already marks the retired `R0001` — one convention, applied again,
   rather than a second one.

   > `R0002-0031` in the 2026-05 round

   *(Decided 2026-09-03, ti `bdf8d981`. Rule 4 used to assert these rounds did
   not collide. They do: three gated rounds on 2026-08-07/07/11 reused the
   numbers, and the rule was false for as long as it stood there.)*

   The marker goes on the 2026-05 side because that is the small side and the
   resolvable one is the other. Roughly forty to sixty 2026-05 citations sit in
   about eight places, most already self-marking — the `CHANGELOG`'s `[0.2.0]`
   section, and the several records that say "external consumer report from
   `resp-translator`" — against roughly a thousand 2026-08 citations that
   already read bare across `contracts.md`, both provider crates, DCR-0022,
   DCR-0025, DCR-0030, DCR-0046 and ADR-0022/0023/0024. Default-bare belongs
   where the lookup actually works: the 2026-08 texts resolve exactly (the
   table gives the command), and most of the 2026-05 ones do not.

   **Numbers settle most of it without any marker**, the way rule 3 does for
   `R0001`. Counts verified 2026-09-03 by reading the recovered blobs:

   | id range | round | why |
   |---|---|---|
   | `R0002-0087` … `0092` | 2026-05 | the 2026-08 round has 86 findings |
   | `R0003-0087` … `0090` | 2026-08 | the 2026-05 round has 86 |
   | `R0004-0001` | **2026-05** | the 2026-05 round has exactly one finding, and 12 `TRACE: R0004-0001` comments in `transync-core`/`-syntax` plus archived DCR-0002 and DCR-0004 all mean that external consumer report |
   | `R0004-0002` … `0100` | 2026-08 | the 2026-05 round stops at `0001` |

   So `R0004` needs **no marker at all** — every id in it is decided by its
   number. Only `R0002-0001`…`0086` and `R0003-0001`…`0086` genuinely overlap,
   and those are what the bare-means-2026-08 default is for.

5. **Dated records are not rewritten to comply.** An ADR, a DCR, a released
   `CHANGELOG` section or an archived issue keeps the words it was written
   with; where its ids need a round, a dated note is appended saying so. The
   older spelling `(review archived and removed)`, which several ADRs and
   `docs/project/open-issues-archive.md` carry, means the same thing as rule 2
   and is left in place.

## The rounds

| Round | Date | Findings | Where the text is |
|---|---|---|---|
| `R0001` (live) | 2026-07-15 | 52 (`0001`–`0052`) | `reviews/reviewed/0001.md`, with `reviews/reviewed/0001-dispositions.md` and the inert `reviews/reviewed/0001.patch` |
| `R0001` (retired) | 2026-05-02 | 104 (`0001`–`0104`) | removed in `bb93b68`; `git show bb93b68^:reviews/reviewed/0001.md`. Titles are indexed below. |
| `R0002` (2026-05) | 2026-05-03 | 92 (`0001`–`0092`) | removed in `bb93b68`; `git show bb93b68^:reviews/reviewed/0002.md` |
| `R0002` (gated, **bare**) | 2026-08-07 | 86 (`0001`–`0086`) | never tracked in this history. `git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260817 cat-file -p 5b187df5b9e94e51` (also in the `20260810` store) |
| `R0003` (2026-05) | 2026-05-03 | 86 (`0001`–`0086`) | removed in `bb93b68`; `git show bb93b68^:reviews/reviewed/0003.md` |
| `R0003` (gated, **bare**) | 2026-08-07 | 90 (`0001`–`0090`) | never tracked. `git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260817 cat-file -p fa12b94b6c743d53` (also in the `20260810` store) |
| `R0004` (2026-05) | 2026-05-04 | 1 (`R0004-0001`, external `resp-translator` consumer report) | removed in `bb93b68`; `git show bb93b68^:reviews/reviewed/0004.md` |
| `R0004` (gated, **bare** from `0002`) | 2026-08-11 | 100 (`0001`–`0100`) | never tracked. `git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260817 cat-file -p e809dce2fdda1632` — **this store only**; the `20260810` store predates it |
| `R0005` | 2026-05-04 | 1 (`R0005-0001`) | removed in `bb93b68`; `git show bb93b68^:reviews/reviewed/0005.md` |
| `R0006` | 2026-05 (ADR-0010 places it one day after commit `49d19ee`) | — | **never tracked.** No file, no git object. The ids survive only in the ADRs, DCRs and `CHANGELOG` entries that quote the finding. |
| `R0007` | by 2026-07-10 (DCR-0005 records it) | — | **never tracked**, as `R0006`. |
| `R0008` | 2026-07-11 | 91 (65 accepted) | **never tracked**, as `R0006`. `CHANGELOG.md`'s "Review 0008 wave" entry summarizes the round. |
| `R0009` | 2026-08-24 | 91 (6 high, 43 medium, 42 low) | `reviews/reviewed/0009.md`, with `reviews/reviewed/0009.patch`. **Claimed as `0002` and renumbered before any citation existed** — see the note in the file's header and rule 4 above, with its 2026-09-03 note. |
| `R0010` | 2026-09-04 | 93 (1 critical, 9 high, 65 medium, 18 low) | `reviews/reviewed/0010.md`, with `reviews/reviewed/0010.patch`. Gated and archived 2026-09-06. |
| `R0011` | 2026-09-05 | 95 (6 high, 42 medium, 47 low) | `reviews/reviewed/0011.md`, with `reviews/reviewed/0011.patch`. Gated and archived 2026-09-06 in the same round as `R0010`. |

`bb93b68` ("reviews: remove archived reviews 0001-0005 (findings preserved)")
is the commit that took rounds `0001`–`0005` off disk; `bb93b68^` is therefore
the last tree that holds them. The live round landed later, in `727d3bc`.

**Note (2026-08-17) — the `git show` forms in the table need a `--git-dir`
now.** This repository's object store has been restarted twice, on 2026-08-10
and 2026-08-17 (`docs/project/git-history-loss-2026-08-17.md`), and neither
`bb93b68` nor `727d3bc` is in this clone — `git show bb93b68^:…` answers
`fatal: Not a valid object name`. The text is still readable, from the object
store archived by the **2026-08-10** restart:

```bash
git --git-dir=/Volumes/Common/git-backup/transync-broken-git-20260810 \
    show bb93b68^:reviews/reviewed/0001.md
```

Verified on 2026-08-17 for `0001` and re-verified on 2026-08-29 at the path
above; substitute `0002`–`0005` for the others. *(Both archives were originally
written under `/Volumes/Temp/claude/`, which is scratch and has since been
wiped. The durable copies under `/Volumes/Common/git-backup/` are the ones this
recipe now names, and are the same stores `status.md` and the loss record cite.)*
Reach for the **2026-08-10** archive specifically, not the newer one:
`bb93b68` predates the first restart, so the store archived on 2026-08-17
(`/Volumes/Common/git-backup/transync-broken-git-20260817`) never held it either —
that store begins at the 2026-08-10 root commit. The table rows above are left
as written: they record which commit removed the text, which is still true.

**Not a citation:** `samples/demo-long.md` is a translation fixture — a
decision summary imported from an unrelated project — and it carries about
ninety `R0001-…` ids of its own, numbered up to `0191`, plus one stray
`R0002-0007`. A tree-wide grep for review ids will hit them. They belong to no
transync round.

## Retired Review 0001 — finding index (2026-05-02)

The round's own file is gone, so this table is what makes its ids resolvable
without git archaeology. Both rounds are frozen, so it cannot drift.

| Id | Severity | Finding |
|---|---|---|
| `R0001-0001` | Critical | Demo shell mounts fetched HTML with unsanitized `innerHTML` |
| `R0001-0002` | Critical | Full-document reparse failures are only logged |
| `R0001-0003` | Critical | Nested block translations are requested but not regenerated |
| `R0001-0004` | Critical | Live OpenAI prompts omit constraints, context, input mode, and glossary |
| `R0001-0005` | Critical | CLI cache metadata can diverge from the live provider actually used |
| `R0001-0006` | High | Provider-declared `FailedNeedsFallback` bypasses retry policy |
| `R0001-0007` | High | Per-batch provider retry option is unused |
| `R0001-0008` | High | Missing API key panics instead of returning a documented CLI exit |
| `R0001-0009` | High | `--base-url` is parsed but never used |
| `R0001-0010` | High | `--model` is ignored by the live provider |
| `R0001-0011` | High | Cache key omits the rendered system prompt |
| `R0001-0012` | High | Cache key omits source language |
| `R0001-0013` | High | Profile TOML parser ignores glossary, constraints, and batching |
| `R0001-0014` | High | Profiles without `[system].prompt` silently disable core safety text |
| `R0001-0015` | High | Embedded default profile parse failures are swallowed |
| `R0001-0016` | High | Glossary is always empty in provider batches |
| `R0001-0017` | High | Profile batch size is hard-coded to eight |
| `R0001-0018` | High | Table alignment constraints are collected but never enforced |
| `R0001-0019` | High | Fragment reparse accepts extra blocks after a valid first block |
| `R0001-0020` | High | Full reparse ignores nested structure |
| `R0001-0021` | High | Missing validation rows are marked translated |
| `R0001-0022` | High | Alignment summary counts blocks, not translatable units |
| `R0001-0023` | High | Target ranges for nested blocks fall back to source offsets |
| `R0001-0024` | High | Table and code block sync attributes are placed on wrapper `<div>` elements |
| `R0001-0025` | High | `ValidationSummary.retried_units` is never populated |
| `R0001-0026` | High | Code-block provider contract is internally inconsistent |
| `R0001-0027` | High | `--force` and `--html-out` path safety are not implemented |
| `R0001-0028` | High | `transync serve` exits success without serving |
| `R0001-0029` | High | Byte-range slicing can panic on non-character boundaries |
| `R0001-0030` | High | Image blocks are modeled but never parsed as image anchors |
| `R0001-0031` | Medium | Ordered task-list items are always marked unordered |
| `R0001-0032` | Medium | Parser emits list items and their child paragraphs as separate units |
| `R0001-0033` | Medium | Headings inside containers mutate the global section stack |
| `R0001-0034` | Medium | Source-position end handling can include neighboring bytes |
| `R0001-0035` | Medium | `LineOffsets::pos_to_byte` does not clamp columns to line length |
| `R0001-0036` | Medium | `assign_block_ids` leaves relationship fields stale |
| `R0001-0037` | Medium | Deterministic zero-key SipHash is used as document identity |
| `R0001-0038` | Medium | `ast_path` is stored but not consumed |
| `R0001-0039` | Medium | Heading hierarchy is computed but not used |
| `R0001-0040` | Medium | Section hierarchy only attaches blocks to the nearest heading |
| `R0001-0041` | Medium | Context heading snippets include Markdown markers |
| `R0001-0042` | Medium | Context lookup is O(n^2) over section paths |
| `R0001-0043` | Medium | Neighbor snippets use flat block adjacency |
| `R0001-0044` | Medium | List topology depth is documented as 1-based but starts at 0 |
| `R0001-0045` | Medium | Blockquote validator checks only direct child kind names |
| `R0001-0046` | Medium | `forbid_block_breaks_in_inline` is set but never enforced |
| `R0001-0047` | Medium | `InputMode::TableRowWindow` is public but never produced |
| `R0001-0048` | Medium | OpenAI client creates a new HTTP client per batch |
| `R0001-0049` | Medium | Rate-limit responses discard `Retry-After` |
| `R0001-0050` | Medium | Nested empty `output_text` can be treated as a valid response |
| `R0001-0051` | Medium | JSON schema does not constrain unit count |
| `R0001-0052` | Medium | `serde_json::to_string` failure becomes an empty prompt |
| `R0001-0053` | Medium | `TransyncOpenAI::new` documentation claims validation it does not do |
| `R0001-0054` | Medium | Token estimation always returns zero |
| `R0001-0055` | Medium | Provider-side oversize split returns the original batch |
| `R0001-0056` | Medium | Core oversize split hook returns the original batch |
| `R0001-0057` | Medium | Retry/fallback policy is split across no-op helpers and inline pipeline logic |
| `R0001-0058` | Medium | CLI `--verbose` has no effect |
| `R0001-0059` | Medium | Pipeline errors all map to CLI exit code 5 |
| `R0001-0060` | Medium | Atomic write docs promise tmp cleanup that code does not do |
| `R0001-0061` | Medium | Atomic writes do not fsync the parent directory |
| `R0001-0062` | Medium | Template placeholders remain visible in generated HTML |
| `R0001-0063` | Medium | Fetch errors are mounted as content |
| `R0001-0064` | Medium | JS sync implementation diverges from ADR-0001 algorithm |
| `R0001-0065` | Medium | JS sync ignores `sync_role` |
| `R0001-0066` | Medium | JS active-block loop assumes DOM order matches vertical layout |
| `R0001-0067` | Medium | Workspace and embedded JS copies can drift |
| `R0001-0068` | Medium | Attribute writer assumes future IDs cannot need HTML escaping |
| `R0001-0069` | Medium | Renderer reparses every block individually |
| `R0001-0070` | Medium | Renderer depends on alignment order matching document order |
| `R0001-0071` | Medium | Library API allows empty target language |
| `R0001-0072` | Medium | BCP-47 language values are not validated |
| `R0001-0073` | Medium | Validation warnings from providers disappear |
| `R0001-0074` | Medium | `ValidationLayer::IdSet` is unused |
| `R0001-0075` | Medium | `translate()` hides cache reuse behind a fresh cache |
| `R0001-0076` | Medium | `run_pipeline` is a large multi-responsibility function |
| `R0001-0077` | Medium | Parser traversal has too many mutable cross-cutting parameters |
| `R0001-0078` | Medium | Unit construction mixes payload extraction, profile rendering, batching, and structural inspection |
| `R0001-0079` | Medium | OpenAI client mixes HTTP, prompt construction, schema generation, and response parsing |
| `R0001-0080` | Medium | CLI translate command mixes IO, profile resolution, provider construction, pipeline invocation, and output policy |
| `R0001-0081` | Medium | Live OpenAI request body is untested |
| `R0001-0082` | Medium | Profile parsing behavior is under-tested |
| `R0001-0083` | Medium | Full-reparse negative path is not tested through the pipeline |
| `R0001-0084` | Medium | CLI smoke does not validate JSON schema or sync attributes |
| `R0001-0085` | Medium | SCN-05 test duplicates production topology logic |
| `R0001-0086` | Medium | CLI exit-code 3 is not tested in `cli_smoke.rs` |
| `R0001-0087` | Low | `codebase_investigation.md` still describes many closed stubs as active |
| `R0001-0088` | Low | `architecture_investigation.md` describes current outputs as empty |
| `R0001-0089` | Low | README says live OpenAI integration is still a post-MVP stub |
| `R0001-0090` | Low | Changelog pending section contradicts completed implementation slices |
| `R0001-0091` | Low | Persistence docs say `transync serve` is a pinned local server |
| `R0001-0092` | Low | Persistence docs claim symlink/canonicalization safety that does not exist |
| `R0001-0093` | Low | Stub manifest contains contradictory deferred-marker language |
| `R0001-0094` | Low | Stub manifest top table still labels closed/deferred rows as `STUB rows` |
| `R0001-0095` | Low | `web/SMOKE.md` accepts no DOMPurify despite architecture requiring it |
| `R0001-0096` | Low | `render::strip_outer_wrapper` is brittle string parsing |
| `R0001-0097` | Low | Thematic breaks are emitted as self-closing HTML |
| `R0001-0098` | Low | `document_title` context uses raw heading Markdown |
| `R0001-0099` | Low | `source_hash_block` returns zero for invalid indexes |
| `R0001-0100` | Low | `InMemoryCache` silently disables itself after mutex poisoning |
| `R0001-0101` | Low | `CacheKey` lacks a schema/version field for its own layout |
| `R0001-0102` | Low | `call_responses_api` module docs reference an outdated response path |
| `R0001-0103` | Low | `scripts/smoke-live.sh` default workdir uses a project-specific temp path |
| `R0001-0104` | Low | `cargo test --workspace` does not run CLI smoke tests |

## Where the collision still shows

Some places keep an unqualified retired-round id on purpose, because rule 5
protects them and rules 1–4 already resolve them:

- `CHANGELOG.md`'s `[0.2.0]` section cites the retired round throughout, and
  `[0.3.0]` cites the live one throughout. Each carries a dated note saying so.
- The older ADRs mark their sources `(review archived and removed)`.
  `docs/project/open-issues-archive.md` does too, and carries a dated note
  covering the few retired-round ids that appear only in an entry's body
  rather than on its `Source:` line.
- Historical plans and specs under `docs/superpowers/` predate the live round
  wherever they are dated before 2026-07-15, so their `R0001-` ids can only be
  retired-round ids.
