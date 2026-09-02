# Open Issues

Issues accepted but pending verification or requiring larger architectural changes.
Remove entries once fully resolved; resolved entries with audit value move to `open-issues-archive.md`.

***

## OI-0016: Active-block selection scans every block on every scroll frame

- **Source:** R0006-0090 (Review 0006) (review archived and removed)
- **Date:** 2026-07-10
- **Decision:** ACCEPT (track — user routing in the Review 0006 gate)
- **Status:** OPEN

### Problem

`activeBlockWithProgress` linearly scans all blocks per RAF-coalesced scroll frame. A sorted-offset cache + binary search would be O(log n), but the code comment deliberately declines the geometric-monotonicity assumption (multi-column / nested-wrapper layouts), and a cache needs invalidation on resize/reflow.

### Impact

No observed jank; cost grows with document size. Optimize only when profiling shows scroll-frame overruns on large documents.

### Required Actions

1. If profiling shows jank: design the offset cache (invalidation on resize/reflow/mutation) and decide the non-monotonic-layout policy. *(2026-08-09, ticket `d3acc3`: the invalidation half got cheaper — the engine now observes `ResizeObserver` on both panes, `document.fonts.ready` and image `load`/`error`, and already re-collects its anchor sets on each. An offset cache would hang off that same recompute rather than needing hooks of its own. The non-monotonic-layout policy is untouched, and this issue stays gated on profiling.)*

***

## OI-NNNN: <Title>

- **Source:** RNNNN-#### (Review NNNN)
- **Date:** YYYY-MM-DD
- **Decision:** ACCEPT
- **Status:** OPEN | RESOLVED (YYYY-MM-DD)
- **Resolution:** <RNNNN-#### that resolved it, if applicable>

### Problem

<Description of the issue - what was found and why it matters>

### Impact

<What could go wrong if not addressed>

### Required Actions

1. <Specific action item>
2. <Specific action item>

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- <Links to related decisions or ignored issues>

***

## OI-0037: Provider-returned payloads bypass the parser's nesting intake guard

- **Source:** R0003-0004, R0003-0005, R0003-0088 (Review 0003) (review archived and removed)
- **Date:** 2026-08-09
- **Decision:** ACCEPT (track — defense in depth; no reachable failure today)
- **Status:** RESOLVED 2026-09-01
- **Resolution:** ticket `148fcf`. Provider payloads are refused at one door
  (`validate::validate_unit`) before any layer parses them, and every remaining
  provider-path reparse goes through `parser::guarded_parse`, which applies the
  same ceiling `parser::intake` applies. `structure.rs`'s three walkers are
  **deliberately not routed** — see below.

### Problem

Source Markdown enters through `parser::intake`, which carries the depth
pre-scan ticket `07844d` added. Provider-returned payloads do not: they call
`comrak::parse_document` directly at five sites —
`validate/fragment_reparse.rs`, `validate/inline.rs`, `validate/per_kind.rs`,
`structure.rs` and `validate/full_reparse.rs`. So the ceiling that exists
because stack exhaustion is an **uncatchable abort** governs what the user
wrote but not what the model returned.

The impact the reviewer claimed — a malicious or defective `Translator`
exhausting the stack — was **refuted** by verification, and that refutation is
the reason this is tracked rather than fixed. The measured, regression-pinned
record in `parser/depth.rs` establishes that comrak 0.27's block parse is
iterative, and every untrusted-tree walker in this workspace was made
iterative for exactly this provider-payload path; memory is linear in the
32 MiB response cap regardless of nesting.

### Impact

None today. The value of closing it is that today's safety rests on **comrak
internals**, not on anything this repository asserts: a dependency upgrade
could reintroduce a recursive block parse and nothing here would go red. The
principle worth holding is that a translated payload should not bypass a
safety ceiling the source must satisfy.

### Required Actions

1. Route provider-payload parsing through one guarded entry point that
   applies the same depth and size policy `parser::intake` applies, rather
   than five direct `comrak::parse_document` calls.
2. Convert a refusal into `ReparseFailure` with attributed fallback ids, so
   the existing retry-then-fallback machinery handles it rather than a new
   error path.
3. R0003-0088 is this work's test gap and cannot land before it: a test for a
   guard cannot exist until the guard does.

### Resolution detail

Required Action 1 named "five direct `comrak::parse_document` calls". That is
five *files*; there are eight production call sites, and two further calls that
are not on this path at all (`unit/payload.rs`'s is inside `#[cfg(test)]`,
`unit/context.rs`'s takes source bytes that already passed `intake`).

Six sites are routed through `parser::guarded_parse`. **`structure.rs`'s three
are not, on purpose**, and the reason is this entry's own Impact paragraph.
The worry recorded there is that today's safety rests on comrak internals and
"a dependency upgrade could reintroduce a recursive block parse and nothing
here would go red". Something here does go red:
`structure::depth_ceiling_tests::deep_nesting_walks_on_the_heap_not_the_call_stack`
parses a thousand nesting levels on a 256 KiB stack. That test **is** this
repository's assertion about comrak's block parse — and applying the ceiling at
that site would refuse the input before comrak ever saw it, deleting the
tripwire in the name of the risk it exists to catch. Routing it also turns
"too deep" into "not a list", which is a weaker diagnostic that can mask a real
topology change.

So the ceiling is applied where payloads *enter*, and `structure` keeps the
hardening plus the tripwire. `pipeline::merge` fingerprints already-merged
windows without passing the door, and that path is what `structure`'s
heap-safety continues to cover.

### Verification

- [x] Code change applied — `parser::guarded_parse` beside `intake`; the door
      in `validate::validate_unit`; `full_reparse` refusing with per-block
      attribution through `BlockOffsets`.
- [x] Tests pass — three added (R0003-0088's gap, Required Action 3):
      a payload past the ceiling rejected at the `FragmentReparse` layer with a
      legal-depth control; a regenerated document past the ceiling refused; and
      per-block attribution naming only the block whose own bytes are too deep.
      Workspace 40/40 binaries, 1131 passed, 0 failed.
- [x] No regressions observed — the three `depth_ceiling_tests` are green and
      unmodified, which is what caught the `structure` misstep above.

### Related

- Ticket `07844d` — the source-side depth ceiling and the measurement that
  refutes the abort claim here.
- ADR-0009 — the retry/fallback contract a refusal must ride.
- DCR-0025 — the Review 0003 pass that routed this to tracking.

***

## OI-0038: A fully-warm run cannot start offline, because credentials are demanded before the cache is consulted

- **Source:** R0004-0069 (Review 0004) (review archived and removed)
- **Date:** 2026-08-13
- **Decision:** ACCEPT (track — whether offline warm-cache rerun is a supported scenario is a product call that decides the fix's shape)
- **Status:** RESOLVED 2026-09-02 (`cdc1e32`, DCR-0046, ticket `30a744`)
- **Resolution:** The product question is answered — **yes, supported**, behind an explicit `--offline` flag rather than by deferring translator construction unconditionally. See *Resolution detail* below.

### Problem

The pipeline constructs its `Translator` — and therefore demands an API key —
**before** the cache is consulted. So a run in which every unit is a cache hit,
which would make zero provider calls, still cannot start without credentials.

`DCR-0028`'s own tests prove such a run exists: they assert that a second
identical run dispatches **zero** provider calls. What the design never
recorded is whether that run is expected to be possible **offline**, with no
key present. Nothing in the records answers it either way, which is why this is
tracked rather than fixed.

### Impact

Bounded and non-destructive: the run fails to start with a credential error
rather than doing anything wrong. But it makes the disk cache's most valuable
property — a document is paid for once — unavailable in the situation where it
is most obviously wanted: re-rendering a translated document on a machine with
no key, or offline.

### Resolution detail (2026-09-02)

**The product answer: yes, and by opt-in.** `transync translate --offline` runs
with no provider credentials, serving every unit from the cache. A fully warm
run completes; one that misses stops **at the miss** with
`no provider available for this run` (`provider_unavailable`, §1) and exit 6.

The ticket's own acceptance criteria proposed deferring translator construction
unconditionally. That was **not** taken, and the deviation is deliberate: an
unconditional defer moves every credential error later and into a different
place, so a typo'd key against a cold cache would be reported mid-run instead
of at startup. The flag keeps the common path's diagnostics intact and makes
the capability real. `--offline` additionally **requires** `--cache-dir`,
refused at argument time, because a fresh in-memory cache misses its first
lookup by construction and the flag would otherwise have exactly one reachable
outcome.

**What the implementation nearly got wrong, recorded because it is the
interesting part.** The first design was a separate `OfflineTranslator` in the
CLI. `CacheKey` carries `provider_fingerprint` as a *namespace* axis, so a
stand-in with its own fingerprint would have missed **every** entry it was
pointed at — and an operator would have read that as a corrupt cache rather
than a misconfigured run. The OpenAI fingerprint covers four parts (model, base
URL, the API-surface heuristic, reasoning effort), so reproducing it outside
that type would also have been a second opinion about the cache namespace.

The credential moved instead: `api_key` became `Option`, and
`TransyncOpenAI::offline` builds a real instance that resolves every other
field exactly as `try_new` does. `fingerprint()` reads none of the key, so an
offline instance namespaces the cache **byte-identically** — pinned by
`an_offline_instance_fingerprints_identically_to_a_credentialed_one`.

### Required Actions

1. **Decide the product question first:** is an offline run against a fully
   warm cache a supported scenario? Everything below depends on the answer, and
   answering it in the record is worth as much as the code.
2. If yes, choose the mechanism: construct the `Translator` lazily, or resolve
   it at first dispatch. Both are more than a few lines.
3. Define the semantics of the case that only exists once the fix lands — a run
   that started without credentials and then takes a cache **miss**. Fail at
   that point? Fall back to source and mark it? That choice belongs with the
   ADR-0009 retry/fallback contract rather than being invented at the call site.
4. Whatever is decided, record it: `DCR-0028` should say what a warm run
   requires, since its own tests are what make the gap visible.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0028 — the disk-backed cache design, whose zero-provider-call tests
  surface this.
- ADR-0009 — the retry/fallback contract that owns the miss-without-credentials
  semantics.
- DCR-0030 — the Review 0004 pass that routed this to tracking.

***

## OI-0039: The JavaScript lint gate exits 0 while validating nothing

- **Source:** R0009-0014 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (routed `fix`; the visibility half landed in `88964df`,
  the exit-status half is a decision above a fix route and was deliberately
  refused there)
- **Status:** OPEN
- **Resolution:** —

### Problem

`scripts/hooks/pre-commit` — the only hook copy, the one `core.hooksPath`
points at via `install-hooks.sh` — runs a JavaScript/TypeScript leg over
`web/`. Its `run_js_tool` helper probes `web/node_modules/.bin/<tool>` and,
when the binary is absent, prints
`[pre-commit] SKIP: <tool> not installed in web — run: (cd web && pnpm add -D <tool>)`
and `return 0`.

`web/node_modules/.bin/` contains exactly one entry: `playwright`.
`web/package.json`'s only devDependency is `@playwright/test`. There is no
`web/tsconfig.json`, so the `tsc --noEmit` leg is skipped as well. **Every
probe therefore misses**, and the hook's exit status is decided entirely by
the Rust gates. Verified by running the hook against a scratch tree
mirroring `web/` with no `Cargo.toml`, so only the JS leg ran: the output was
`SKIP: prettier not installed in web`, `SKIP: eslint not installed in web`,
`HOOK_EXIT=0`.

The consequence is that `web/js/sync.js` — 52,996 bytes, the sync engine
itself — together with `web/js/wasm-demo.js`, `web/playwright.config.js` and
the three spec files under `web/tests/`, ships with **zero** format or lint
coverage.

Verification narrowed the reviewer's claim in two ways, and both matter. The
filed phrasing was that the hook behaves "as if the language gate ran": that
is wrong. Two SKIP lines carrying the exact `pnpm add -D` remediation have
always been printed, and skip-with-notice is the **recorded** design —
DCR-0018, and both 2026-08-20 wave plans call the SKIP lines "normal output
here, not a failure". And the severity was overstated: there is no
correctness exposure, because browser behaviour is covered by the Playwright
suite.

**What landed (`88964df`) and what did not.** The visibility half is done:
the hook now ends with a framed block reading `THE JAVASCRIPT/TYPESCRIPT GATE
DID NOT RUN.`, naming the missing tools and stating that "a successful exit
below covers the Rust gates ONLY", plus the command that would close it. The
**status** is still 0, and that is what remains open. The fixing agent
refused the substantive half on the record: making the exit status honest
means choosing and pinning a linter and bringing every existing JS file into
conformance, which is a decision, not a fix.

### Impact

A commit that breaks JS formatting or introduces a lint-visible defect passes
the hook, and the hook is the only automatic gate this repository has — there
is no CI behind it. Until the status is honest, "the pre-commit hook passed"
is a statement about Rust alone, and a reader has to have read the framed
skip block to know that. The bounded part is that behaviour is still covered:
Playwright exercises the sync engine, the WASM demo and SCN-13.

### Required Actions

1. **Choose between two routes.** Both are decisions, not fixes, and both
   have a named cost:
   - **(a) Adopt and pin a linter.** The hook already has a biome-first
     branch (`if [ -x "$js_dir/node_modules/.bin/biome" ]`), so
     `pnpm add -D @biomejs/biome` alone makes it fire. The cost is
     conforming roughly 190 KB of existing JavaScript under `web/` and
     `crates/transync-cli/web/` — including `web/js/sync.js`, whose
     **byte-identical twin** at `crates/transync-cli/web/sync.js` is pinned by
     `crates/transync-cli/tests/sync_js_drift.rs`: any reformat must land in
     both copies in the same commit or that test goes red. A rule set has to
     be chosen that does not fight the deliberately framework-free style.
   - **(b) Fail only when the commit stages JavaScript.** Turn the skip into
     a failure when a staged path under `web/` is a `.js` file. Narrower
     blast radius, but it blocks work in progress: the first developer to
     touch a JS file mid-feature is stopped until a linter is installed and
     that file conforms.
2. Whichever is chosen, record it — DCR-0018 is where skip-with-notice was
   decided, so it is where the successor decision belongs.
3. Do not close this by improving the message again. The message is already
   as loud as a message can be; what is unresolved is the status.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0018 — where skip-with-notice was decided.
- Commit `88964df` — the visibility half, and the recorded refusal of the
  substantive half.
- `crates/transync-cli/tests/sync_js_drift.rs` — the weld that makes a
  reformat of `sync.js` a two-file change.

***

## OI-0040: The same bytes are parsed or spliced repeatedly on the accepted translation path

- **Source:** R0009-0031, R0009-0034, R0009-0035, R0009-0037, R0009-0043,
  R0009-0065 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Six findings, one shape: **no seam in this workspace carries a parsed
artifact**, so every layer that needs an AST — or a spliced HTML fragment —
makes its own from the same bytes.

1. **R0009-0034 — one candidate, up to four comrak parses per attempt.**
   `validate/per_kind.rs`'s `check_heading` does
   `parse_document(&arena, &result.translated_payload, &opts)`;
   `validate/fragment_reparse.rs` parses the same string again;
   `validate/inline.rs`'s `check_inline` reaches `inline_inventory`, which
   parses it a third time; and for table, list and blockquote kinds
   `structure.rs` adds a fourth. None of the four accepts a pre-parsed AST.
   (`validate/full_reparse.rs` is the exception that proves the shape — it is
   per-document, not per-candidate.)
2. **R0009-0035 — the whole reference-definition pool is copied and reparsed
   twice per unit per attempt.** `validate/inline.rs` calls
   `inline_inventory(&unit.source_payload, ref_defs)` and
   `inline_inventory(&result.translated_payload, ref_defs)`, and
   `inline_inventory` builds `appended = format!("{payload}\n\n{ref_defs}")`
   and parses that. `ref_defs` is the **whole-document** pool
   (`pipeline.rs`: `let ref_defs: &str = &doc.ref_defs;`, built by
   `parser/refdefs.rs::extract`). The cost is therefore
   O(units × |ref_defs|) in both allocation and parse time — quadratic in
   document size for a reference-definition-heavy document. The only relief
   is the `ref_defs.is_empty()` fast path, so documents with no reference
   definitions pay nothing. **This is the one member of the six with real
   algorithmic teeth.**
3. **R0009-0037 — the source-side inventory is recomputed every retry
   round.** `pipeline/dispatch.rs` calls
   `crate::validate::validate_batch(current_batch, &result, ref_defs)` once
   per round inside the round loop, and nothing memoizes the source side:
   `TranslationUnit` carries `source_payload` and no precomputed inventory.
   Verification narrowed the saving — a fresh run and the cache-hit
   revalidation path each validate a unit exactly once, so the recomputation
   only costs on units that actually retry.
4. **R0009-0043 — an accepted HTML unit is spliced at least twice.**
   `validate.rs` runs
   `transync_html::splice(&h.source_bytes, &segs, BlankLinePolicy::from_commonmark_html_block_type(h.block_type))`,
   uses the result only for a `tag_inventory` comparison and then discards
   it; `regen.rs` splices again, its own comment saying "Validation layer 3
   already proved this splice succeeds". The repair ladder in
   `pipeline/finalize.rs` can re-run `regen_pass` up to three times, splicing
   again each time.
5. **R0009-0065 — `transync_html::splice` itself tokenizes twice.** `scan`
   → `scan_chunks` builds a full `HtmlRewriter` and writes the block through
   it; phase B then builds a **second** `HtmlRewriter` and re-writes the same
   `block.as_bytes()`. Two complete tokenizer passes per splice, plus an
   `htmlize::unescape` per text node in phase A. The redundancy is wider than
   the finding says: `extract` already scanned the same bytes at unit-build
   time.
6. **R0009-0031 — a WASM rebuild is four whole-document parses, not two.**
   `crates/transync-wasm/src/engine.rs`'s `rebuild_impl` calls
   `check_top_level_structure(&doc, &translated_md)`, which calls
   `parser::parse` (its own `comrak::Arena`, its own `parse_document`), and
   then calls `render::render_target(...)`, whose `render_fragment` does
   `parser::intake` followed by a second `comrak::parse_document` over the
   same bytes. The source side is parsed twice the same way —
   `parse_with_ids` then `render_source`.

### Impact

No correctness defect in any of the six; all are CPU and allocation.
Verification found five of the six overstated: they are microsecond-scale
work on one block inside a network-bound pipeline, invisible next to a single
provider round trip. R0009-0035 is the exception — its cost grows with the
product of unit count and reference-definition pool size, so it is the one
that can be *measured* on a real document.

Two caveats have to survive into any fix, because both point at work that
would make the code worse:

- **R0009-0043 must not be "fixed" by caching the validated splice.** The
  two calls take *different* inputs — validation uses
  `constraints.html.source_bytes` (built by `outcome::block_payload`), regen
  slices `doc.source_text` via `parser::ranges::clamped_char_bounds` — and
  `validate.rs` documents that divergence as load-bearing: it "cannot make
  this layer lie about regen's success". Carrying the validated result
  forward collapses a deliberate independent double-check to save local CPU.
- **R0009-0065's recommendation is not directly implementable.** lol_html's
  `TextChunk` exposes no source byte offsets, so "stage offsets during one
  scan" needs a custom offset tracker built first, not a refactor.

### Required Actions

1. **R0009-0035 on its own merit, first.** The cheapest sound fix is a
   per-side prefilter: skip the `ref_defs` append when that payload contains
   no `[`. Reference definitions emit no inline nodes, so a bracket-free
   payload provably cannot change its inventory, and the comparison stays
   symmetric. Reimplementing label matching, as the review recommends, is not
   needed.
2. **A per-attempt validation context** that parses the candidate once and
   lends the AST to `per_kind`, `fragment_reparse`, `inline` and `structure`
   covers R0009-0034 and R0009-0037 together. This is the **same refactor**
   OI-0037's Required Action 1 already mandates — funnelling those five
   `comrak::parse_document` sites through one guarded entry point — with a
   second, performance justification. Do it once, under OI-0037, rather than
   opening a competing thread.
3. **R0009-0031 needs an API decision before any code.** `parser::parse`
   owns its arena and returns owned IR, so threading a parsed AST into both
   `check_top_level_structure` and `render_target` pushes an arena lifetime
   through `transync-syntax`'s public render signature.
4. **R0009-0043:** the cheap, safe half is to record beside both call sites
   that the inputs differ and why. Any sharing is a decision about whether
   the independent double-check is still wanted.
5. **R0009-0065** needs a source-offset tracker over lol_html before the two
   passes can become one.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- OI-0037 — the same five `comrak::parse_document` call sites, reached from
  the safety side rather than the performance side.
- DCR-0004 — the reparse cascade that can run `regen_pass` up to three times.
- ADR-0009 — the retry contract that decides how often a unit is revalidated.

***

## OI-0041: Pre-network setup recomputes derived values it could compute once

- **Source:** R0009-0044, R0009-0045, R0009-0046, R0009-0047, R0009-0049,
  R0009-0050, R0009-0051, R0009-0073 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Eight findings in `transync-core`, all in the phase that runs **once per run,
before any provider call**: profile loading, glossary resolution, unit
building and oversize-table split planning. Each recomputes something it
already had. They are recorded together because that shared verdict — *this
is pre-network CPU, not a defect* — is the most useful thing the register can
say about all eight at once. They divide into two distinct edits.

**Split planning (`crates/transync-core/src/unit/split.rs`, plus
`pipeline/merge.rs`).**

- **R0009-0044** — `split_oversize_tables` does
  `if !units.iter().any(|u| windows_of(u, target, factor, &bpe).is_some()) { return; }`
  and then, immediately, `for unit in units.drain(..) { match windows_of(&unit, target, factor, &bpe) { … } }`.
  `windows_of` runs `estimate_unit_output_tokens`, then `split_table_rows`
  (a full comrak `parse_document`, in `transync-syntax/src/regen.rs`), then
  `greedy_plan`. Verification bounded it: `.any()` short-circuits at the
  **first** splitting unit, so only the prefix is priced twice and the common
  no-oversize-table document pays exactly one pass — the comment above the
  code already says so. It also found a **larger cost in the same path that
  the review missed**: `greedy_plan` rebuilds and re-encodes the whole growing
  window payload once per body row
  (`estimate_payload_output_tokens(&rows.window(&current), …)` inside the row
  loop). That one is quadratic in row count.
- **R0009-0045** — `window_units` re-parses the parent it was planned from:
  `let rows = split_table_rows(&parent.source_payload).expect("the plan was built from this payload's own slice");`,
  under a comment that begins `// Re-sliced rather than threaded through:`.
  Deliberate and stated; same edit as R0009-0044.
- **R0009-0073** — **the headline is refuted.** The review said "`SplitPlan`
  owns cloned `TranslationUnit` values". `pipeline/merge.rs` defines
  `struct Window { unit_id: BlockId, source_payload: String }` — not a
  `TranslationUnit` — and `of_batches` clones exactly those two fields, with
  a doc comment justifying it ("the row boundaries are the packer's and are
  not recoverable from the source alone"). What survives is smaller: it could
  be a borrow, since the batches outlive the plan. Verification found the
  larger avoidable clone the review walked past, in the same file:
  `window_records.push(vu.clone())` copies the whole `ValidatedUnit` —
  accepted payload included — immediately before the original is consumed
  field by field.

**Profile loading and glossary resolution (`crates/transync-core/src/profile.rs`,
plus `unit.rs`).**

- **R0009-0046** — `default_profile()` is
  `load_profile(DEFAULT_PROFILE_TOML).unwrap_or_else(|e| panic!(…))` with no
  `OnceLock`, and `impl Default for ProfileMetadata` calls it. Verification
  bounded the cost hard: the live path touches it at most twice per run
  (`pipeline.rs`'s `opts.profile.clone().unwrap_or_else(default_profile)` and
  `CacheKeyContext::for_run`, whose doc comment reads "Computed once per
  pipeline run") — never per unit or per batch. **The sharper smell the
  finding missed is not the parsing: it is a `Default` impl that parses TOML
  and can panic.**
- **R0009-0047** — `load_profile` decodes the same text twice:
  `toml::from_str(toml_text)` for the typed profile, and
  `toml_text.parse::<toml::Value>()` inside `collect_unknown_key_warnings`.
  The fix does not need `serde_ignored`, as the review suggests: parse
  `toml::Value` once and deserialize the typed profile from that value,
  keeping the warning scan on the same tree.
- **R0009-0049** — `entry_applies_to_section` canonicalizes on every
  comparison:
  `heading_stack.iter().any(|heading| { let heading = section_key(heading); entry.sections.iter().any(|selector| section_key(selector) == heading) })`,
  where `canonical_key` is `text.trim().to_lowercase()` plus an NFC fold for
  non-ASCII — a fresh `String` per comparison. Called per (section, entry)
  from `unit.rs` and again from `effective_glossary`, giving
  O(sections × entries × headings × selectors) allocations.
- **R0009-0050** — `effective_glossary` canonicalizes the same term twice per
  section: `section_winner.entry(glossary_key(&entries[i].source_term)).or_insert(i)`
  in the winner loop, then `section_winner.get(&glossary_key(&entry.source_term))`
  in the keep loop — on top of `normalize_glossary` and `merge_auto_glossary`.
- **R0009-0051** — `unit.rs` walks the glossary twice per section:
  `for (i, e) in raw_profile.glossary.iter().enumerate() { if matches!(e.scope, ConditionalOnSection) && crate::profile::entry_applies_to_section(e, &sec.heading_stack) { ever_applied.insert(i); } }`
  immediately followed by
  `crate::profile::effective_glossary(&raw_profile.glossary, &sec.heading_stack)`,
  whose first statement re-filters on `entry_applies_to_section`. **The two
  passes answer different questions**, and a naive merge would break one:
  `ever_applied` is DCR-0027's G7 obligation ("did this selector match
  anywhere") and must count entries that applied but were **shadowed**, which
  `cohort_key` excludes.

### Impact

None observable. Every item here runs before the first provider request and
is dwarfed by it; the glossary loop is dwarfed even locally by the tiktoken
encode of every unit in the same loop. Six of the eight were filed above
their verified severity. The reasons to do the work are that the split-side
re-parses are pure waste on exactly the documents that are already large,
and that `Default for ProfileMetadata` can panic — which is a defect of shape
even though nothing reaches it today.

### Required Actions

1. **Split planning, one edit:** thread a `TableRows` value and the plan
   through `split_oversize_tables` → `window_units` instead of re-deriving
   them, which closes R0009-0044 and R0009-0045 together. While in there,
   hoist the per-row re-encode out of `greedy_plan`'s row loop — the
   unnamed quadratic is the bigger win — and turn `Window`'s owned fields
   into borrows plus remove the `ValidatedUnit` clone in `merge.rs`
   (R0009-0073).
2. **Profile, one edit:** store a canonical source key on the normalized
   glossary entry at load time and canonicalize selectors and heading stacks
   once at partition time, which closes R0009-0049 and R0009-0050. Have
   `effective_glossary` also return the applicable set so `unit.rs` can drop
   its own scan (R0009-0051) — **not** by reusing `cohort_key`, which omits
   shadowed entries.
3. Parse the profile TOML once into a `toml::Value` and deserialize from it
   (R0009-0047); memoize `default_profile()` behind a `OnceLock`
   (R0009-0046).
4. Separately from the memoization, decide whether `impl Default for
   ProfileMetadata` should be able to panic at all.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0026 — the row-window split whose planning path R0009-0044/0045/0073
  sit in.
- DCR-0027 — obligation G7, which is why R0009-0051's two passes are not
  interchangeable.

***

## OI-0042: Two scans in the degraded regeneration cascade are worse than linear

- **Source:** R0009-0033, R0009-0079 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Two inefficiencies that the review filed as Medium hot-path problems and that
verification placed on a path which **only runs after regeneration has
already failed**. They are recorded together because that is the whole point
of the entry: someone re-reading the review should not re-raise them as
throughput issues.

**R0009-0033 — quadratic offender detection.**
`crates/transync-core/src/validate/full_reparse.rs`'s `attribute_offenders`
does
`for entry in normalized { … let owned = regen_top.iter().filter(|top| top.start >= start && top.start < end).count(); … }`
— a full scan of `regen_top` per entry, with both collections sized by the
document's top-level block count. Both call sites sit inside terminal error
branches of `reparse_full` (the kind-divergence scan and the count-drift
scan) that `return Err(...)` immediately, so it never runs on a passing
reparse: at most once per failing `reparse_full`, and up to three times per
run through the DCR-0004 cascade. **The review's suggested fix is
off-target** — containment here is range-based (`top.start` inside the
entry's offset span), not id-based, so the linear form is a sorted sweep or
a binary search over `regen_top`, not an ID lookup table.

**R0009-0079 — a linear string scan per seed.**
`crates/transync-core/src/pipeline/finalize.rs`'s `widen_to_neighbors` builds
`let top_level: Vec<&BlockId> = crate::regen::top_level_blocks(&doc.blocks).map(|b| &b.block_id).collect();`
and then, for each seed,
`let Some(i) = top_level.iter().position(|id| *id == seed) else { continue };`
— a linear comparison of `BlockId` (a `String`) per seed, with
`top_level_blocks` documented as "now the identity walk", so the inner bound
is the whole block list. Its single call site is stage 2 of the cascade in
`finalize_regen_with_reparse_policy`, reached only after `reparse_full`
failed **and** the stage-1 re-regeneration also failed.

### Impact

None observable, on either. Both run at most a handful of times per run and
only inside an already-degraded fallback path, where the pipeline is
salvaging a document rather than serving one at speed. The value of doing
the work is that both fixes are small and both make a rarely-exercised path
easier to reason about; the value of the record is that neither is a
throughput problem and should not be prioritized as one.

### Required Actions

1. R0009-0079: build one `HashMap<&BlockId, usize>` before the seed loop.
2. R0009-0033: replace the per-entry filter with a sorted sweep or a binary
   search over `regen_top` by `start`. Do not build an ID lookup table — the
   predicate is a range containment, not an identity match.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0004 — the reparse cascade that decides how often either scan runs.

***

## OI-0043: Four modules are called oversized; three are mostly inline test code, and one really is a concern bundle

- **Source:** R0009-0068, R0009-0069, R0009-0070, R0009-0071 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

The review filed four "oversized module" findings from raw `wc -l`.
Verification re-split each file on `#[cfg(test)]` with brace tracking, and
the raw number hides the real shape in three of the four cases:

| file | total | production | test | submodule dir |
|------|-------|-----------|------|---------------|
| `crates/transync-html/src/lib.rs` | 2,550 | 1,242 | 1,308 (8 modules) | none — single-file crate by DCR-0032 |
| `crates/transync-core/src/profile.rs` | 4,035 | 1,738 | 2,297 (8 modules) | **none** |
| `crates/transync-core/src/pipeline.rs` | 4,981 | 1,457 | 3,524 (7 modules) | `dispatch`, `finalize`, `merge`, `policy`, `report`, `retry` |
| `crates/transync-cli/src/output.rs` | 3,967 | 2,041 | 1,926 (2 modules) | `lock.rs` only |

Read against that table:

- **R0009-0071 (`output.rs`) is the strongest of the four** and the only one
  whose stated impact is intact. It is the single file where the *production*
  body is around 2,000 lines with just one concern (`lock.rs`) extracted, so
  target resolution, staging, bundle rendering and filesystem policy really
  do share one module.
- **R0009-0070 (`pipeline.rs`) is 71% test code**, and its recommendation is
  largely already discharged: retries, dispatch, finalization, merge, policy
  and reports each already live in their own file, leaving roughly 1,457
  production lines in the parent. "Move orchestration phases into focused
  files" is mostly done; what is actually left in the parent is 3,524 lines
  of tests.
- **R0009-0069 (`profile.rs`) is 57% test code**, and is the one of the four
  with **no submodule directory at all** — so the split is genuinely undone.
  But the cheapest and largest win is still moving the eight test modules
  out, not re-cutting the runtime concerns.
- **R0009-0068 (`transync-html`) is a re-raise of a known open item**, not a
  new finding. DCR-0032's "Handed forward" section already names the
  file-as-module split the spec permits "later", records the crate as "one
  `lib.rs` today — 1316 lines as moved, 1952 at the end of the wave", and
  encodes it as a `CRATE_ROOTS` floor of 0 in
  `crates/transync/tests/docs_ownership_drift.rs` — "a state nobody has
  decided against rather than a shape anyone chose". Its production body is
  1,242 lines, not 2,550.

### Impact

No behavioural defect; all four are maintainability. The concrete cost is
navigation and review surface: a reviewer opening `output.rs` to check one
filesystem policy reads past bundle rendering to reach it. The concrete risk
of acting on the raw line counts instead of this table is doing the wrong
work — re-cutting `pipeline.rs`'s orchestration, which is already cut.

### Required Actions

1. `output.rs` first: it is the only genuine concern bundle. Split target
   resolution, staging, bundle rendering and filesystem policy into
   `output/*.rs` under the repo's file-as-module convention (never `mod.rs`).
2. For `pipeline.rs` and `profile.rs`, narrow the work to moving the inline
   `#[cfg(test)]` modules out. Decide per module whether the destination is
   `tests/` (integration, loses access to private items) or `src/<name>/tests.rs`
   (keeps it) — several will need the latter.
3. `transync-html` rides DCR-0032's handed-forward item and its
   `CRATE_ROOTS` floor; do not open a second thread for it, and sequence it
   behind the HTML→HTML waves still landing in that crate rather than racing
   them.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0032 — the `transync-html` extraction, which already hands the
  file-as-module split forward.
- `crates/transync/tests/docs_ownership_drift.rs` — the `CRATE_ROOTS` floor
  of 0 that records the single-file state.

***

## OI-0044: The disk cache log has an unbounded read, a poisonable write and a trim that can never converge

- **Source:** R0009-0080, R0009-0081, R0009-0082 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Three findings, one file — `crates/transync-core/src/cache/disk.rs` — one on
each of its three paths.

**R0009-0080 — replay reads an unbounded physical line.** `scan_log(file: File, path: &Path)`
reads with `.read_until(b'\n', &mut line)` into a plain
`let mut line: Vec<u8> = Vec::new();`. There is no `Read::take`, no length
cap and no resync anywhere in the file — `grep -n 'take(' disk.rs` returns
only `std::mem::take`. The header decision happens **after** the read
(`if matches!(header, Header::Missing)`), so a foreign file with no newline
at all is materialized whole and only then rejected. That contradicts the
function's own doc claim that "peak memory is the index it is building plus
one record". Verification bounded it twice: a legitimately large record has
to be materialized anyway because it lands in the index, and a corrupt or
enlarged cache file requires a local writer to the cache path — which
ADR-0022 (`docs/decisions/0022-local-user-threat-model.md`) already placed
outside the threat model, **naming R0004-0024, this same memory concern, by
number**.

**R0009-0081 — a failed write leaves the writer installed.** `DiskState`
holds `writer: BufWriter<File>` and never replaces or disables it.
`buffer_record` does `write_all(line)` then `write_all(b"\n")` and returns
`Err` with that writer still in place; `Cache::put` and `evict` call
`write_record(&mut state.writer, …)` on it again on the next call. There is
no last-good-offset field, and `truncate_to` is called only from
`replay_log`, never from the write path. Verification narrowed the blast
radius sharply — it is **one welded line, not open-ended corruption**:
sub-capacity records self-heal, because `BufWriter::flush_buf` leaves the
unwritten remainder queued and the next flush completes the record. Damage
needs a record at or above the 8 KiB buffer, where `write_all` bypasses the
buffer into `File::write_all` and can partially write. The result is one
unparseable line that `scan_log` skips with a warning, and a truncated JSON
object concatenated with a whole one cannot deserialize — so **no wrong value
is ever served**. The cost is two lost cache entries, inside DCR-0028's
stated "degrading to re-translation" envelope. No test covers a failing
writer.

**R0009-0082 — the byte budget can remain exceeded forever.**
`trim_to_budget` measures `replayed.compacted_bytes()`, which is
`header_bytes() + self.meta_bytes` plus the entry bytes, but its drop loop
iterates only `replayed.entries`. When `oldest_first` is exhausted the loop
ends with `over(total, count)` still true. DCR-0028 §4 nevertheless states
that "oldest-written live entries are dropped first **until within budget**",
and the public field doc says only "Byte budget for the log file" with no
exemption named. Verification found it **worse in kind than filed**: if the
header plus `meta_bytes` alone exceeds `max_bytes`, every open drops *every*
unit entry and still returns `true`, forcing a compaction — the disk cache
then permanently retains nothing across opens. That is realistic only when an
operator sets `max_bytes` small relative to accumulated document-meta and
glossary records; the 1 GiB default is far away.

### Impact

Bounded, and none of the three can serve a wrong value. R0009-0080 and
R0009-0081 cost memory and cache entries respectively, both inside the
"degrade to re-translation" envelope DCR-0028 already claims. R0009-0082 is
the one with a user-visible failure mode: an operator who sets a small
`max_bytes` gets a cache that discards everything on every open and never
says why — the exemption exists in DCR-0028 §4 and in `trim_to_budget`'s own
comment, but not on the public knob the operator actually sets.

### Required Actions

1. **R0009-0082 first**, because it is the only one an operator can walk
   into: add a floor so the drop loop stops once dropping every entry cannot
   get under budget, and state on the public `max_bytes` doc that the header
   and the document-scoped meta records are not subject to it. Whether meta
   records should instead become evictable is a separate question and does
   not block the floor.
2. **R0009-0081:** record a last-good offset before each record and truncate
   to it on a write error, or drop the writer so the next call reopens.
   Either way, add the failing-writer test that does not exist.
3. **R0009-0080:** cap the physical line with `Read::take` before the header
   decision, or amend the doc claim to match the code. Note ADR-0022's
   posture in whichever is chosen — this is tidiness and doc accuracy, not a
   threat-model gap.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- DCR-0028 — the disk-backed cache design; §4 states the trim guarantee
  R0009-0082 contradicts, and the "degrading to re-translation" envelope both
  other members sit inside.
- ADR-0022 — the local-user threat model, which already answers R0009-0080's
  security framing by number.

***

## OI-0045: `transync serve` under-delivers a truncated body and over-advertises its authorities

- **Source:** R0009-0002, R0009-0008, R0009-0009 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Three residual serve findings after the Review 0009 fix pass, two in
`crates/transync-cli/src/serve_cmd/host.rs` and one in `.../conn.rs`.

**R0009-0002 — the streaming branch discards the byte count it announced.**
`conn.rs` does
`write_head(stream, 200, "OK", content_type, &[], len).await?;` then
`let mut limited = file.take(len); tokio::io::copy(&mut limited, stream).await.map(|_| ())`
— the copied count is thrown away by `.map(|_| ())`. The in-memory branch a
few lines above instead announces `body.len() as u64`, i.e. the bytes it
actually read. The doc comment on `read_capped` reasons only about a file
that **grows**; the shrink direction is unhandled in both the prose and the
code. So a served file past the 8 MiB in-memory limit that is truncated
mid-response sends fewer bytes than the announced `Content-Length`.
Verification put it at Low, not Medium: the reach is only the >8 MiB branch —
`crates/transync-cli/tests/serve_static.rs` says in its own comment "Nothing
in a bundle is this big" — and the client sees a short read on a
`Connection: close` socket (curl reports a failed transfer), not silent
corruption. The existing test
`a_body_past_the_in_memory_limit_streams_intact` pins the streaming branch
for a static file only.

**R0009-0008 — an IPv4 wildcard bind advertises an IPv6 authority it is not
listening on.** `host.rs` has one branch for both families and never consults
`local.ip()`'s family:
`if ip.is_unspecified() { answered.push((Host::Ip(IpAddr::V4(Ipv4Addr::LOCALHOST)), port)); answered.push((Host::Ip(IpAddr::V6(Ipv6Addr::LOCALHOST)), port)); … }`.
A `0.0.0.0` listener is IPv4-only, yet startup output and 421 bodies both
advertise `[::1]:port`. The test
`a_wildcard_bind_answers_for_loopback_and_for_what_was_allowed` pins
`[::1]:4319` as Answered for a `0.0.0.0:4319` bind, and
`docs/architecture/contracts.md` states the same thing in prose — so the code,
its test and the contract are consistently wrong together. The impact is
guidance accuracy plus one unreachable allowlist entry: a client that used
`http://[::1]:port/` never reaches this socket at all, so no verdict changes.

**R0009-0009 — derived and explicit authorities are not deduplicated.**
`host.rs` pushes the derived entries and then every `--allow-host` into the
same `answered: Vec<(Host, u16)>` with no normalization, and
`answered_authorities` maps and joins that vector verbatim — so
`--bind 127.0.0.1 --allow-host localhost` prints
`127.0.0.1:7470, localhost:7470, localhost:7470`. Cosmetic only: the lookup
is `self.answered.iter().any(...)`, so a duplicate cannot change a verdict,
and it surfaces only when the operator passed a redundant `--allow-host`,
where the repeat arguably tells them the flag was unnecessary.

### Impact

Low across all three. R0009-0002 is the only one that can affect a byte on
the wire, and only for a bundle file larger than 8 MiB that shrinks during
the response — the client detects it. R0009-0008 misleads an operator into
trying an authority that cannot work. R0009-0009 is display noise.

### Required Actions

1. R0009-0002: compare `tokio::io::copy`'s returned count against `len` and
   treat a short copy as an aborted response rather than a success. Extend
   `read_capped`'s doc comment to state the shrink direction it currently
   ignores.
2. R0009-0008: branch on `local.ip()`'s family so a `0.0.0.0` bind advertises
   only `127.0.0.1`. **Keep the `::` case exactly as it is** — a dual-stack
   `::` listener genuinely does answer at `127.0.0.1`. The fix is three
   changes, not one: the code, the test that pins the wrong expectation, and
   the `contracts.md` sentence that repeats it in prose.
3. R0009-0009: an order-preserving retain-first dedup. **Do not sort** — the
   display test `the_authorities_print_the_way_they_are_typed` pins ordering
   deliberately.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- `docs/architecture/contracts.md` — carries R0009-0008's claim in prose and
  must be corrected with the code.

***

## OI-0046: Three holes in the checking apparatus itself

- **Source:** R0009-0015, R0009-0018, R0009-0067 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Three findings about what this repository's checks cover, rather than about
shipped behaviour. None is a product defect; each is a place where a check is
narrower than a reader would assume.

**R0009-0015 — the rustdoc *completeness* check runs only in smoke, not in
the hook.** The discovery loop that catches a library member nobody added to
the gate — `for manifest in "$REPO_ROOT"/crates/*/Cargo.toml` …
`[[ -f "$member_dir/src/lib.rs" ]] || continue` …
`[smoke] FAIL: library member(s) outside the rustdoc gate` — exists only in
`scripts/smoke.sh`. `grep -c ungated scripts/hooks/pre-commit` returns 0; the
hook instead sources `scripts/lib/rustdoc-gate.sh` and runs `cargo doc` over
`RUSTDOC_GATE_ARGS`, failing only if that array is empty. So a new library
crate can be added and committed with the hook green while nothing documents
it, until someone runs smoke. Verification narrowed this twice: the split is
**documented and deliberate** (`rustdoc-gate.sh` says "What keeps it total is
the completeness check in `scripts/smoke.sh`"), and there is **no live gap
today** — all eight members carrying a `src/lib.rs` (`transync-lang`,
`transync-html`, `-syntax`, `-core`, `transync`, `-openai`, `-anthropic`,
`-wasm`) are in `RUSTDOC_GATE_CRATES`. What makes it worth closing is the price: the check is
a pure bash glob with no cargo cost, so moving it into `rustdoc-gate.sh`
closes the window at zero hook-time expense.

**R0009-0018 — the browser harness hard-codes a global port.**
`web/playwright.config.js` declares `const HOST = "127.0.0.1";` and
`const PORT = 4319;` as bare literals with no `process.env` fallback, and
both the `baseURL` and the `webServer` command consume them.
`scripts/test-browser.sh` passes only `TRANSYNC_SERVE_BIN` and
`TRANSYNC_FIXTURE_DIR` on its final `pnpm exec playwright test "$@"` line —
no port at all. Two concurrent runs therefore collide. The config's own
comment shows that only the *smoke server* collision was considered
("uncommon to avoid colliding with … `scripts/smoke-live.sh` uses 7470"), not
two runs of this suite. **The port is not the only shared resource:**
`scripts/test-browser.sh` also defaults `WORKDIR` to a single fixed path
(`TRANSYNC_FIXTURE_WORKDIR`, defaulting to a shared scratch directory) and
`rm -rf`s it before each run, so a port-only fix does not make concurrent
runs safe. Fix the port and the workdir together. R0009-0017, the sibling
finding about reusing a foreign server, was fixed in `88964df` — reuse is now
off unless `TRANSYNC_REUSE_SERVER=1`.

**R0009-0067 — the hand-rolled HTML tokenizer has no generative testing.**
`grep -rn 'proptest|quickcheck|arbitrary|cargo-fuzz|libfuzzer|afl' */Cargo.toml`
over the workspace returns nothing, and `find . -type d -name 'fuzz*'`
returns nothing: there is no committed fuzz, property or differential harness
anywhere in the repository. `crates/transync-html/tests/` holds only
`token_stream_pin.rs` — a golden pin over three checked-in fixtures — and
`goldens/`; the 79 `#[test]` items in `lib.rs` are handpicked cases. So a
tokenizer that architectural invariant 7 tells us to treat as sitting on
untrusted input is validated only by examples someone thought of. **The value
is already proven, and then thrown away:** this same backlog records "15,726
violations / 200k fuzz iterations" behind ticket `95f55b` — someone fuzzed
this crate ad hoc, found a real defect, and did not keep the harness. This is
distinct from the five `490d97`-wave tickets, which are specific defects
rather than the missing strategy.

### Impact

No shipped defect from any of the three. R0009-0015 is a window that is
currently empty. R0009-0018 costs a developer a confusing failure when two
runs overlap, and — because the workdir is also shared — can make one run
read a bundle the other deleted. R0009-0067 is the one with real expected
value: a generative harness against this tokenizer has already found a defect
once, on a code path the threat model says to assume hostile.

### Required Actions

1. R0009-0015: move the library-member discovery loop from `scripts/smoke.sh`
   into `scripts/lib/rustdoc-gate.sh`, so the hook and smoke enforce
   completeness from one definition. Keep smoke's failure message.
2. R0009-0018: give the config `process.env` fallbacks for host and port, and
   give `scripts/test-browser.sh` a per-run workdir. Decide how a second
   concurrent run gets both — an explicit env pair, or a derived
   run-identifier — before writing either half; a port-only change would
   leave the destructive resource shared.
3. R0009-0067: decide the harness shape first — property tests inside the
   crate, a `cargo-fuzz` target, or a differential oracle against a browser
   parse — and where it runs, given there is no CI and the pre-commit hook is
   the only automatic gate. Then rebuild what ticket `95f55b`'s ad hoc run
   already demonstrated.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- Ticket `95f55b` (`docs/backlog.md`, `transync-html-wave-0-1-findings`) — the
  ad hoc fuzz run whose harness was not kept.
- Commit `88964df` — R0009-0017's fix, which R0009-0018's work should build
  on rather than duplicate.

***

## OI-0047: The sync engine freezes past its last anchor and goes silent about drift on reflow

- **Source:** R0009-0021, R0009-0025 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Two findings in `web/js/sync.js`, whose **byte-identical twin** lives at
`crates/transync-cli/web/sync.js` and is pinned by
`crates/transync-cli/tests/sync_js_drift.rs` — both copies move in the same
commit or that test goes red. (The review named a third path,
`crates/transync-wasm/demo/sync.js`; it does not exist.)

**R0009-0021 — a dead zone after the last anchor freezes the follower.**
`activeBlockWithProgress` ends
`if (topmostVisible) { return { id: …, progress: 0 }; } return null;`, and
its reference line is `const ref = scrollTop + REFERENCE_OFFSET_PX;` with
`const REFERENCE_OFFSET_PX = 4;`. Executed against a stub DOM — three anchors
ending at y=900, 1200px of unanchored trailing content, `clientHeight` 600 —
it returns `{"id":"p-3","progress":1}` at `scrollTop=896`, `null` at
`scrollTop=897`, and `null` at `scrollTop=1500`. `handleScroll` then does
`const active = activeBlockWithProgress(pane, blocks); if (!active) return;`,
so the follower pane simply stops moving. The dead zone begins exactly at
`scrollTop > lastAnchorBottom - 4`.

Verification narrowed the reach hard, and this is the part worth carrying
forward: **neither shipped shell can scroll into it.** Reaching the zone
needs more than a viewport of unanchored content below the last anchor
*inside the scroll box*, and shipped panes get only the sanitized `<main>`
plus `.pane { padding: 12px }` (`web/index.html`,
`crates/transync-cli/web/index.html.tpl`), while the only non-anchoring block
kinds are `ThematicBreak` and `Title` (`transync-syntax/src/align.rs`,
`sync_role_for`). A third-party pane with a tall in-pane footer can. Also,
half of the review's recommendation is already implemented: a viewport above
the first anchor already returns that anchor at progress 0
(`scrollTop=0 → {"id":"p-1","progress":0}`). What is missing is the symmetric
clamp at the bottom — roughly three lines.

**R0009-0025 — reflow recollection suppresses drift diagnostics.**
`recompute` calls `collectAnchors(sourcePane, "source", rowIds, true)` and the
same for the target — the quiet flag set — and calls neither
`warnMapDomDrift` nor `warnOffsetParentDrift`, both of which run at mount.
Executed: a clean mount emits 0 warnings; after mutating the source pane to
hold a duplicate `p-1`, an unlisted `ghost`, and a `p-2` whose `offsetParent`
is a `<figure>`, `controller.refresh()` emitted **0** warnings while mounting
the same DOM fresh emitted **3**.

Two qualifications the verification adds. First, **what is lost is reporting,
not behaviour**: the functional containment still holds on reflow, because
the `rowIds` gate is not quiet — an unlisted anchor stays inert and a
duplicate keeps its first occurrence. Second, **the silence is partly
deliberate and recorded**: `contracts.md` §4a says of the skip warnings that
each "is named at mount … and is silent on reflow for the same reason". What
no record covers is the other two diagnostics: `warnMapDomDrift` and
`warnOffsetParentDrift` do not re-run at all, and the offsetParent probe is
the one with geometric consequence.

### Impact

Neither is reachable from a shipped shell today. R0009-0021 affects a
third-party consumer whose pane carries tall unanchored trailing content —
for them the follower silently stops tracking, which reads as a broken
product rather than a layout mistake. R0009-0025 costs a consumer the
diagnostic that would tell them their post-mount DOM edit or their pane
positioning is wrong; they get correct-but-inert behaviour and no
explanation.

### Required Actions

1. R0009-0021: clamp to the last block at progress 1 when the reference line
   is past the last anchor's bottom, mirroring the existing above-the-first
   clamp. Apply to both `sync.js` copies in one commit.
2. R0009-0025: decide the reflow diagnostic policy before coding. §4a states
   a mount-time budget — "One property read per pane at mount, never per
   frame" — so re-running the probes on every coalesced reflow frame is a
   change to a documented budget, not just an added call. Candidate shapes:
   re-run only on `controller.refresh()` (an explicit caller request, not a
   frame); or re-run once and latch, warning only when the verdict changes.
3. Whatever is chosen, amend §4a: today it explains the skip warnings'
   reflow silence and says nothing about the other two.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- `docs/architecture/contracts.md` §4a — the mount-time probe budget and the
  recorded reflow silence of the skip warnings.
- `crates/transync-cli/tests/sync_js_drift.rs` — the weld that makes either
  fix a two-file change.
- `docs/investigation/architecture/constraints-and-debt.md` — "DOM-only
  anchor mutation has no dedicated observer", a related but distinct row: it
  is about *detecting* the mutation, not about reporting it.

***

## OI-0048: Four boundary checks are weaker than the contract a reader would infer

- **Source:** R0009-0052, R0009-0053, R0009-0075, R0009-0078 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** OPEN
- **Resolution:** —

### Problem

Four places where the library enforces less than its record implies, each
visible only to a consumer that is not the CLI. Grouped because each fix is
partly a `docs/architecture/contracts.md` edit and partly code, and because
deciding them apart risks four inconsistent answers about how much a
programmatic caller is owed.

**R0009-0052 — an unrecognized table strategy silently becomes whole-block.**
`profile.rs` resolves with
`match constraints.default_table_strategy.as_deref() { Some("row-window-first") => RowWindowFirst, _ => WholeBlock }`,
and the only warning for an unrecognized value lives in `load_profile`.
`ProfileConstraints` is `#[non_exhaustive]` with
`pub default_table_strategy: Option<String>`, and contracts.md §1 documents
default-then-assign as the external construction route — so a programmatic
caller who writes `Some("rowwindow")` gets whole-block with no warning at
all. The CLI is safe: `translate_cmd/args.rs` notes "Clap has already
rejected any value that is not one of the two". Verification bounded the
impact and corrected the fix: the failure is **loud but mis-diagnosed**, not
silent end-to-end — an unsplit oversize table is still named by the live
preflight (`pipeline.rs`, `output_budget_warnings`) and aborts at the
provider under ADR-0017. And **the review's enum recommendation collides with
the TOML wire shape and the §0 frozen-field policy**; the cheap fix is to
repeat the loader's unknown-value warning at the translate boundary, exactly
as R0001-0020 did for prompt template variables.

**R0009-0053 — a caller input error is reported as an engine fault.**
`crates/transync-core/src/lib.rs` returns
`TransyncError::Internal("TranslateOptions.target_language must be non-empty".to_string())`
for an empty `target_language`, and `error.rs` maps
`TransyncError::Internal(_) => "internal"`. So a caller who passed a blank
string is told the engine broke. `TransyncError` is `#[non_exhaustive]`, so a
new variant is additive — but `stable_code()` is an inter-process vocabulary
pinned in contracts.md §1, which states "The complete set is **twenty**
codes". The fix must add a row and change that number: small, but a contract
edit rather than a one-liner.

**R0009-0075 — a dropped unit status is indistinguishable from a rejection in
release builds.** `crates/transync-syntax/src/align.rs` synthesizes a status
when the map has none —
`let fallback_status = if let Some(s) = statuses.get(&block.block_id).copied() { s } else if translatable { FallbackStatus::FallbackSource } else { … }`
— with no `tracing::warn`, in contrast to the missing-offset path immediately
below it, which does warn. (The review's path,
`crates/transync-core/src/align.rs`, does not exist.) The only completeness
guard is debug-only, in `pipeline/report.rs`:
`debug_assert!(doc.blocks.iter().filter(|b| …is_translatable_block…).all(|b| statuses.contains_key(&b.block_id)), "every translatable block must carry a finalized status into build_alignment_map")`.
Verification cut the severity and, more usefully, cut off the review's
proposed fix. The artifact is **not** corrupted: regen splices the source
bytes, the row is labelled `fallback_source` — the *least*-integrity status —
and it is counted into `summary.fallback_source`. What is lost is only the
distinction between "validated and rejected" and "dropped by a bug". And
making `build_alignment_map` strict is **not available**:
`crates/transync-wasm/src/engine.rs` documents that "a block absent from
`statuses_json` gets the same synthesized status the pipeline would give it",
and the test `omitted_unit_backed_html_row_is_fallback_source_not_translated`
pins that as the demo's HTML carve-out. Any hardening belongs at the
`report.rs` seam.

**R0009-0078 — the source pane never validates the row's own `source_range`.**
`transync-syntax/src/render.rs`'s `PaneCtx::new` performs exactly three
checks — duplicate `source_block_id`, `UncoveredBlock`, and `range_fault` —
and never compares `row.block_kind`, `source_order`, `target_order`,
`target_block_id` or `sync_role` against the `Block`. `Pane::range` is
`match self { Pane::Source => block.source_range, Pane::Target(_) => row.target_range }`,
so the **row's** `source_range` is neither read nor bounds-checked, while row
fields do reach the DOM (`attrs::write_attrs(row)` writes `data-block-kind`
from `row.block_kind` and `data-order` from `row.source_order`).

Most of this finding describes a documented boundary rather than a broken
promise, and verification established that in five separate ways: §4a
enumerates precisely the three refusals the renderer promises; the CLI never
renders a foreign map (no `render_source`, `render_target` or
`from_str::<AlignmentMap>` anywhere in `crates/transync-cli` — `publish.rs`
serializes the map the same run built); the shipped JS gates two of the
listed fields (`validateRows` refuses a non-identity `target_block_id` and an
unknown `sync_role`, and `warnMapDomDrift` reports a promised anchor the pane
lacks); `data-order` has no consumer in `web/` at all; and the only live
foreign-map path is `transync-wasm`'s view mode, which ADR-0023 assigns to
the demo. **The one sub-claim no record covers** is the narrow residual this
member tracks: a third-party consumer of published `transync-syntax` that
slices the map's `source_range` can disagree with the pane it mounted. §4a's
own sentence — "Each pane is measured against what it slices: `source_range`
against the source text" — does not say *whose* `source_range`, and the code
measures the block's.

### Impact

None of the four is reachable from the CLI, which is why all four are tracked
rather than fixed. Each costs a non-CLI consumer the same thing in different
words: a wrong answer that looks like a right one. A programmatic caller
mis-spelling a strategy gets a provider abort blamed on size (R0009-0052); a
caller passing a blank language gets told the engine faulted (R0009-0053); a
pipeline bug that drops a status is presented as an ordinary validated
fallback (R0009-0075); a third-party renderer slicing `source_range` gets
bytes the pane does not hold (R0009-0078).

### Required Actions

1. R0009-0052: repeat the loader's unknown-value warning at the translate
   boundary, following R0001-0020's precedent. Do **not** convert the field
   to an enum — the TOML wire shape and §0's frozen-field policy both refuse
   it.
2. R0009-0053: add a caller-input variant to `TransyncError` (additive under
   `#[non_exhaustive]`) and its `stable_code()` row, and update contracts.md
   §1's count, which currently reads "twenty".
3. R0009-0075: promote `report.rs`'s `debug_assert` to a release-mode warn or
   error. Do **not** make `build_alignment_map` strict — the WASM view mode
   depends on the synthesized status, with a test pinning it.
4. R0009-0078: choose one — assert that `row.source_range` equals
   `block.source_range` in `PaneCtx::new`, or state in §4a that the source
   pane reads the block's range and the row's field is advisory. The second
   is cheaper and the first is stronger; either closes it, and leaving the
   sentence ambiguous does not.

### Verification

- [ ] Code change applied
- [ ] Tests pass (if applicable)
- [ ] No regressions observed

### Related

- `docs/architecture/contracts.md` §1 (the twenty-code `stable_code`
  vocabulary, the default-then-assign construction route) and §4a (the
  renderer's three promised refusals, and the `source_range` sentence).
- ADR-0017 — the provider-side abort that makes R0009-0052 loud but
  mis-diagnosed.
- ADR-0023 — assigns the only live foreign-map path to the WASM demo.
- R0001-0020 — the precedent for repeating a loader warning at the translate
  boundary.

***

## Open Issues Summary

| Issue ID | Title                                                  | Status   | Severity |
|----------|--------------------------------------------------------|----------|----------|
| OI-0016  | Active-block selection scans per scroll frame            | OPEN   | Low      |
| OI-0035  | Injected `data-sync-id` can pre-claim a real block's anchor | RESOLVED (2026-08-23) — archived | Low |
| OI-0037  | Provider payloads bypass the parser's nesting intake guard | RESOLVED (2026-09-01) | Low |
| OI-0038  | A fully-warm run cannot start offline — credentials precede the cache | RESOLVED (2026-09-02) | Low |
| OI-0039  | JS lint gate exits 0 while validating nothing            | OPEN (2026-08-26) | Low |
| OI-0040  | Same bytes parsed/spliced repeatedly on the accepted path | OPEN (2026-08-26) | Low (R0009-0035 Medium) |
| OI-0041  | Pre-network setup recomputes derived values              | OPEN (2026-08-26) | Low |
| OI-0042  | Two worse-than-linear scans in the degraded regen cascade | OPEN (2026-08-26) | Low |
| OI-0043  | Four modules called oversized; one is a real concern bundle | OPEN (2026-08-26) | Low |
| OI-0044  | Disk cache log: unbounded read, poisonable write, non-converging trim | OPEN (2026-08-26) | Low |
| OI-0045  | `serve` under-delivers a truncated body, over-advertises authorities | OPEN (2026-08-26) | Low |
| OI-0046  | Three holes in the checking apparatus itself             | OPEN (2026-08-26) | Low |
| OI-0047  | Sync engine freezes past its last anchor; silent drift on reflow | OPEN (2026-08-26) | Low |
| OI-0048  | Four boundary checks weaker than the inferred contract   | OPEN (2026-08-26) | Low |
