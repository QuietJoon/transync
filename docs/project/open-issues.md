# Open Issues

Issues accepted but pending verification or requiring larger architectural changes.
Remove entries once fully resolved; resolved entries with audit value move to `open-issues-archive.md`.

***

## OI-0040: The same bytes are parsed or spliced repeatedly on the accepted translation path

- **Source:** R0009-0031, R0009-0034, R0009-0035, R0009-0037, R0009-0043,
  R0009-0065 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** RESOLVED 2026-09-04
- **Not archived — residual obligation (ti `26238261`).** Required Action 4's divergence note is present at the validation splice site but absent at the regen splice site, and Actions 3, 4 and 5 are disposed only by DCR-0049's blanket deferral rather than by name.
- **Resolution:** The `[`-prefilter landed at both `inline_inventory` sites and the run-level `unit_count × |ref_defs|` tripwire landed in `build_batches`; the shared-AST **seam** is deferred with that tripwire as its own re-trigger, so the deferral reports itself when it starts to matter. Three corrections to this entry as written: the parse count is **6/5/4** per attempt (heading-table-list-blockquote / paragraph / code), not "up to four" — `validate/text_presence.rs` added two on 2026-09-04; Required Action 2's routing of R0009-0034 / R0009-0037 onto **OI-0037 is dead** (OI-0037 resolved 2026-09-01 the other way — it shared a *guard*, not an artifact), so those two are re-homed here rather than closed; and the measured cost is +1,756 ms on a 1,289 ms baseline at 5,000 units × 20 KB pool, of which the prefilter recovers 51.8% at this corpus's bracket density and 0% at link-reference house style.

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

*(Filled in 2026-09-09, ticket `13fcede1` — resolved in the 2026-09-02/04 review-0009 wave with the boxes left unticked.)*

- [x] Code change applied — **partially, by decision.** The `[`-prefilter landed at
      both `inline_inventory` sites and the run-level `unit_count × |ref_defs|`
      tripwire landed in `build_batches`. The shared-AST **seam** is deferred, with
      that tripwire as its own re-trigger, so the deferral reports itself when it
      starts to matter rather than waiting to be remembered.
- [x] Tests pass (if applicable) — the standing workspace gate is green (2026-09-09: 47 targets, 1,359 passed, 0 failed, 8 ignored).
- [x] No regressions observed — the prefilter is a fast path in front of unchanged
      logic, and the tripwire only warns.

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
- **Status:** RESOLVED 2026-09-04 (partial — the rest declined or deferred, with evidence)
- **Not archived — residual obligation (ti `b3e8d80c`).** Required Action 1's `greedy_plan` per-row re-encode hoist is neither done nor recorded as declined or deferred anywhere.
- **Resolution:** `unit/split.rs` now plans in one pass and carries the sliced parent with the plan: `windows_of` runs once per unit instead of once per unit plus once more up to the first splitting table, a whole comrak `split_table_rows` per splitting table is gone, and **a panic site is deleted** — the invariant rides the signature instead of an `.expect`. `impl Default for ProfileMetadata` gained a `# Panics` section: the panic is real but unreachable for any caller of a built crate (the input is an `include_str!` compile-time constant, and 85 in-tree call sites plus three content assertions fail first), and all three structural fixes are worse than the disease — a `build.rs` duplicates `load_profile`'s rules, Rust literals violate the single-source rule `default_profile_single_source.rs` enforces, and a `Default` that stops equalling `default_profile()` hands out `prompt_body: ""`, converting an unreachable panic into a reachable silent-empty-prompt run. Six of the eight filed redundancies are **declined**, on this entry's own verdict that they were filed above their verified severity. R0009-0049 / R0009-0050 are **deferred**: their prescribed fix needs a new field on `GlossaryEntry`, a tier-(a) public type without `#[non_exhaustive]` whose serde shape is operator-edited profile TOML — a breaking wire change for a loop this entry itself records as dwarfed by the tiktoken encode beside it. Re-trigger: the next breaking window that is already moving `GlossaryEntry`.

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

*(Filled in 2026-09-09, ticket `13fcede1` — resolved in the 2026-09-02/04 review-0009 wave with the boxes left unticked.)*

- [x] Code change applied — **partially, by decision**, as the Status line says.
      `unit/split.rs` now plans in one pass and carries the sliced parent with the
      plan, so `windows_of` runs once per unit instead of once per unit plus once
      more up to the first splitting table, and a whole comrak `split_table_rows`
      per splitting table is gone. **A panic site was deleted** — the invariant
      rides the signature instead of an `.expect`. The remainder is declined or
      deferred with the evidence recorded above.
- [x] Tests pass (if applicable) — the standing workspace gate is green (2026-09-09: 47 targets, 1,359 passed, 0 failed, 8 ignored).
- [x] No regressions observed — the one-pass plan produces the same windows; the
      deleted `.expect` was the only behavioural difference, and it removed a panic
      rather than adding one.

### Related

- DCR-0026 — the row-window split whose planning path R0009-0044/0045/0073
  sit in.
- DCR-0027 — obligation G7, which is why R0009-0051's two passes are not
  interchangeable.

***

## OI-0046: Three holes in the checking apparatus itself

- **Source:** R0009-0015, R0009-0018, R0009-0067 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** RESOLVED 2026-09-04
- **Not archived — residual obligation (ti `a736f3fd`).** Required Action 2's second half is undone: `scripts/test-browser.sh` still defaults to one fixed fixture workdir and deletes it per run, with no per-run derivation and no recorded decision to decline one.
- **Resolution:** All three sub-items. (1) The rustdoc **completeness** loop moved from `scripts/smoke.sh` into the shared `scripts/lib/rustdoc-gate.sh` that both smoke and the hook already source, so the hook gained it — ~+13 net lines and 15 ms of hook time. Its only candid deferral trigger would have been "someone adds a library member without gating it", the very event it exists to catch, and **that trigger had already fired**: `Developer_Guide.md`'s documented gate command omitted `transync-lang`, added 2026-09-02. (2) `web/playwright.config.js`'s hard-coded port gained an env fallback, so two concurrent `test-browser.sh` runs stop colliding. (3) **The demonstration failed, and adopting the target was right.** `crates/transync-html/tests/generative_properties.rs` — 840 lines, a nine-line `splitmix64` generator over four pinned seeds, `std` only, **no dependency, no `cargo-fuzz`, no `#[ignore]`** — because the only automatic gate here is the pre-commit hook and a coverage-guided harness would run at no venue, which this repository has two measured records of (`benchmark/lang-detect/`'s single never-re-run `RESULTS.md`, and `smoke-live-gate.sh anthropic`, written and never executed). It found a **live defect on its first execution** — `balance_fragment`'s orphan-deletion pass welds a literal `<` onto its new neighbour, minting markup from text, with four harms including a `strip_reserved_sync_attrs` bypass that puts an attacker-chosen `data-sync-id` on a real element in a pane (ticket `fdd989`). **Why 8/8 historical divergences being pinned did not cover it, measured:** not one of the 38 `EDGE_CASES` entries produces a single orphan close tag — zero — so the entire orphan-deletion pass, the first thing `balance_fragment` does, had no corpus coverage at all; six entries carry a literal `<` and none carries one alongside an orphan closer. The two ingredients existed separately and never together, which is the combinatorial gap a hand-picked corpus structurally cannot close. One correction to this entry's own framing: the anchor-safety invariant (`walk_elements(&balanced).unclosed.is_empty()`) was asserted over **2** inputs, not the 213 literals the decision brief reported; the 41 goldens pin bytes, never that blessed bytes are still anchor-safe.

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

*(Filled in 2026-09-09, ticket `13fcede1` — resolved in the 2026-09-02/04 review-0009 wave with the boxes left unticked.)*

- [x] Code change applied — all three sub-items. The rustdoc **completeness** loop
      moved out of `scripts/smoke.sh` into the shared `scripts/lib/rustdoc-gate.sh`
      that both smoke and the hook already source, so the hook gained it for about
      thirteen net lines and 15 ms. Its only candid deferral trigger would have
      been "someone adds a library member without gating it" — the very event it
      exists to catch — and that trigger **had already fired**: the documented gate
      command in `Developer_Guide.md` omitted `transync-lang`.
- [x] Tests pass (if applicable) — the standing workspace gate is green (2026-09-09: 47 targets, 1,359 passed, 0 failed, 8 ignored); the moved completeness check now runs on
      every commit through the hook.
- [x] No regressions observed — the checking apparatus is strictly wider: the hook
      fails on a library *or* bin-only member left outside the rustdoc gate.

### Related

- Ticket `95f55b` (`docs/backlog.md`, `transync-html-wave-0-1-findings`) — the
  ad hoc fuzz run whose harness was not kept.
- Commit `88964df` — R0009-0017's fix, which R0009-0018's work should build
  on rather than duplicate.

***

## OI-0048: Four boundary checks are weaker than the contract a reader would infer

- **Source:** R0009-0052, R0009-0053, R0009-0075, R0009-0078 (Review 0009)
- **Date:** 2026-08-26
- **Decision:** ACCEPT (track — user routing in the Review 0009 gate)
- **Status:** RESOLVED 2026-09-04 (R0009-0052 deferred)
- **Not archived — the deferral's premise is contradicted by the code (ti `830de884`).** R0009-0052 was deferred because `#[non_exhaustive]` was said to stop a caller-built `ProfileConstraints`; it does not, and `crates/transync-cli/src/translate_cmd/args.rs` assigns `constraints.default_table_strategy` on a `ProfileMetadata` the facade re-exports. The recorded re-trigger is already met.
- **Resolution:** Policy adopted: **the library refuses at a boundary a real caller can reach, and records — with a named trigger — where no caller can reach it yet**, reachability measured against the consumer roster in `release-checklist.md`. R0009-0053 landed: `TransyncError::InvalidOptions(String)` with the new stable code `invalid_options`, so an empty `TranslateOptions.target_language` stops reporting `internal` — a code that told a consumer "transync has a bug" about a value the consumer passed in. **The reachability argument was corrected in flight and it inverted:** both roster consumers guard the field, which first read as "unreachable, do not build it", but the guards exist *because* transync mis-reported the cause — downstream compensation for an upstream defect, not evidence the fix was unneeded — and naming the cause is what lets those guards be retired. §1's own Rules paragraph independently forbids the cheaper route: "a new variant requires a **new** stable code rather than reuse of an existing one". R0009-0075 landed: `report.rs`'s `debug_assert!` is a collected missing-id list behind one bounded `tracing::warn!`. R0009-0078 landed as its **doc half only** — §4a now states that the source pane measures the *Block's* `source_range` and never the row's copy, which is equal by construction — with `render.rs` untouched. R0009-0052 is **deferred**: `ProfileConstraints` is `#[non_exhaustive]`, so no caller outside the crate can build the unrecognized value that would reach it. Re-trigger: a caller-built `ProfileConstraints` becomes constructible, or a consumer reports a silently-whole-block table.

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

*(Filled in 2026-09-09, ticket `13fcede1` — resolved in the 2026-09-02/04 review-0009 wave with the boxes left unticked.)*

- [x] Code change applied — **partially, under an adopted policy**, as the Status
      line says: the library refuses at a boundary a real caller can reach, and
      records with a named trigger where no caller can reach it yet, reachability
      measured against the consumer roster in `release-checklist.md`. R0009-0053
      landed — `TransyncError::InvalidOptions(String)` with the new stable code
      `invalid_options`, so an empty `TranslateOptions.target_language` stops
      reporting `internal`, a code that told a consumer "transync has a bug" about
      a value the consumer passed in. **R0009-0052 is deferred** under that policy.
- [x] Tests pass (if applicable) — the standing workspace gate is green (2026-09-09: 47 targets, 1,359 passed, 0 failed, 8 ignored); the stable-code vocabulary is welded by
      `crates/transync/tests/error_taxonomy.rs`.
- [x] No regressions observed — the new variant is additive, and it rode the
      sanctioned v0.5.0 breaking window.

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

## OI-0049: Review 0010's non-blocking survivors — sixteen verified findings, explicitly deferred past v0.5.0

- **Source:** Review 0010 (2026-09-04), triaged and adversarially verified 2026-09-05
- **Date:** 2026-09-05
- **Decision:** ACCEPT (real) — **DEFERRED past v0.5.0, with a re-trigger**
- **Status:** DEFERRED 2026-09-05

### Why this entry exists

The v0.5.0 release condition is the union of the TicGit queue and this
register, **less explicitly deferred items**. These sixteen findings are real
and verified, and none of them blocks. Leaving them unregistered would fail the
condition by omission; filing them as OPEN would fail it by arithmetic. So they
are recorded here, once, as a deferral with a stated reason — the same route
OI-0039, OI-0040, OI-0044 and OI-0048 took for their own residuals.

### How they were established

Review 0010's 93 findings were triaged against the recorded decisions, and then
audited in **both** directions — every ACCEPT faced an adversarial refutation
pass instructed to default to refuted, and every REJECT was challenged for
false negatives. Of 27 ACCEPTs, 19 survived; of 44 rejections, 39 stood and 5
were overturned. The sixteen below are the survivors that do not block.

The audit mattered: three of the five overturned rejections DID block, and each
had been rejected by citing a real record that ruled on an adjacent question.
That is the failure mode a "check the register first" method introduces, and it
is why the rejections were audited rather than trusted.

### The deferred set

| Finding | Severity | Title |
|---|---|---|
| R0010-0015 | LOW | Invalid output parents are discovered after translation |
| R0010-0016 | MEDIUM | Dangling out-directory symlinks bypass early existence checks |
| R0010-0037 | LOW | A new heading does not close an open heading |
| R0010-0044 | MEDIUM | Head mode is not implicitly closed by ordinary body content |
| R0010-0045 | LOW | Later head tags can re-enter suppression mode |
| R0010-0054 | LOW | HTML-document sniffing does not recognize lone-CR blank preambles |
| R0010-0057 | MEDIUM | An XML declaration prevents XHTML detection |
| R0010-0061 | LOW | Spaced known placeholders evade warnings but are not substituted |
| R0010-0072 | LOW | WASM demo startup has no terminal catch |
| R0010-0079 | LOW | Kana coverage omits supplementary and halfwidth ranges |
| R0010-0080 | LOW | Arabic coverage omits extended and presentation-form letters |
| R0010-0087 | LOW | HTML crate and package descriptions still call HTML intake future work |
| R0010-0088 | LOW | Trimming HTML runs can become quadratic in skip count |
| R0010-0089 | LOW | Naked-character detection rescans all tag spans per character |
| R0010-0014 | LOW | A plain `--output <dir>/out.md` destination gets no staging-temp sweep |
| R0010-0062 | LOW | An unclosed `{{` folds with the next placeholder, so later typos go unwarned |

### Why none of them blocks

None breaches invariant 1 (block ID is the only sync currency) or invariant 7
(source Markdown is untrusted) from untrusted input. The set is operator
filesystem state (0015, 0016, 0014), diagnostics that under-report (0061,
0062), detection-coverage gaps in `transync-lang` for scripts the shipped
corpus does not measure (0079, 0080), HTML-document sniffing edges behind an
explicit `--input-format` (0054, 0057), head/suppression-mode edges in the HTML
intake (0037, 0044, 0045), a browser-demo startup path (0072), documentation
drift (0087), and two performance shapes with no measurement attached (0088,
0089).

### The re-trigger — objective and self-firing

Any ONE of these reopens the set as blocking work:

1. **A `sec4a` or anchor-loss route is measured** for any of them by the ti
   `ec235f` Chromium oracle. Three of the four HTML-intake entries (0037, 0044,
   0045) are in the family the oracle keeps finding routes in, and it has
   already produced two live impostor-anchor routes nobody hypothesised.
2. **`transync-lang` gains a measured non-Hangul target.** 0079 and 0080 are
   coverage gaps in Kana and Arabic; today `benchmark/lang-detect/RESULTS.md`
   measures Korean only, so they are unmeasurable rather than acceptable. A
   second measured target makes them ordinary correctness bugs — and ti
   `15acc0` already showed this crate can return `AlreadyTarget` wrongly, which
   cancels a run rather than degrading one.
3. **0088 or 0089 is measured over a real document** and shows a cost of the
   order OI-0042's did (26 ms / 245 ms / 1.9 s at 5,000 / 20,000 / 50,000
   blocks). Both are called quadratic by inspection and neither has a number;
   OI-0042 is the precedent for taking that seriously once measured, and
   OI-0016 is the precedent for closing it when measurement shows no overrun.

### Related

- Fixed rather than deferred: R0010-0031/0032/0034/0049 (ti `bebebe`,
  DCR-0050) and R0010-0066 (ti `15acc0`).
- Still blocking, ticketed: R0010-0033 (ti `a7e625`, spec-traced and awaiting
  oracle measurement); R0010-0039 and R0010-0043 folded into ti `307283`.

  *All three RESOLVED 2026-09-05 by DCR-0051.* R0010-0033 was **confirmed** by
  the oracle rather than accepted from its spec trace — Chromium parses
  `<math><mi><mglyph><script><div>` as `<math><mi><mglyph><script></script>
  </mglyph><div></div></mi></math>`, and `malignmark` measures identically —
  and R0010-0039's table scope and R0010-0043's special-element guard both
  landed in the one open-element stack that record introduces.
- 0061 and 0062 are the same function in `profile.rs`; one fix covers both.
- 0015 and 0016 are the same preflight-vs-publish seam in
  `crates/transync-cli/src/output/`.

---

## OI-0051: Every translation batch carries a cloned profile and a second copy of its glossary

- **Source:** R0010-0091, R0010-0092 (Review 0010)
- **Date:** 2026-09-06
- **Decision:** ACCEPT — user-routed **track** at the indy-review-gate Phase 2 round
- **Status:** OPEN (blocked on a breaking window)

### Problem

`TranslationBatch` carries **both** `glossary: Vec<GlossaryEntry>` **and**
`profile: ProfileMetadata`, and `ProfileMetadata` has a `glossary` of its own.
The single construction site in `unit::build_batches` fills both from the same
cohort (`glossary: cohort.glossary.clone(), profile: cohort`), so the two are
byte-identical by construction — a test pins exactly that equality. Separately,
`profile` is an owned value per batch whose largest member is `prompt_body`, the
entire compiled system prompt: DCR-0027's cohort memo compiles it once, but every
batch in a cohort still holds its own full clone.

### Impact

The allocation cost is real but modest. The reason to fix it is the **two-places-
one-opinion** class: `pipeline` builds the glossary-sensitive `CacheKey` from
`batch.glossary`, while the text the provider actually sees is
`batch.profile.prompt_body`, compiled from `profile.glossary`. Today they cannot
disagree. The moment any code writes one field without the other, the cache key
describes a glossary that is not in the prompt — a silent wrong-cache-hit, not a
crash. Removing the field makes that state unrepresentable.

The reviewer's stronger claim — that cohorts themselves are recomputed per batch —
is **wrong**; DCR-0027 §5 memoizes them. Only the per-batch clone is real.

### Required Actions

1. Remove `TranslationBatch::glossary`; readers take `profile.glossary` (add an
   accessor if the call sites read better for it).
2. Change `profile` to `Arc<ProfileMetadata>` so a cohort's compiled prompt is
   shared rather than cloned per batch.
3. Update the equality test to assert the field is gone rather than that two
   copies agree.

### Verification

- [ ] Code change applied
- [ ] Cache keys unchanged for an unchanged glossary (byte-level)
- [ ] No regressions observed

### Blocked by

**A breaking window.** Both actions change `TranslationBatch`'s public field set,
which is exhaustive-by-policy. The v0.5.0 window closed at the `v0.5.0` tag
(2026-09-05); a further breaking change needs a new window and the owner decision
that opens one.

### Related

- DCR-0027 §5 (cohort memoization) — the part of the finding that is already done.
- OI-0052 — the other finding waiting on the same window.

---

## OI-0052: The checked provider constructors accept header values that cannot become headers

- **Source:** R0011-0022 (Review 0011)
- **Date:** 2026-09-06
- **Decision:** ACCEPT — user-routed **track**, deferred until the v0.6.0 window opens
- **Status:** OPEN (blocked on a breaking window)

### Problem

`try_new` validates only that the API key is non-empty. Conversion to a
`HeaderValue` is deferred to request time, so a key containing a newline, a NUL,
or any other byte `HeaderValue` refuses is accepted by the constructor whose whole
purpose is to reject bad configuration, and then fails on **every** request
afterwards.

### Impact

The failure is loud but late and repeated: a host that builds an adapter at
startup learns its credential is unusable only when the first translation call is
made, and gets a per-request error rather than one construction error. For a
long-lived service that is a startup check that does not check the thing it exists
to check.

### Required Actions

1. Attempt the `HeaderValue` conversion inside `try_new` and surface the failure
   as a `ConfigError` variant.
2. Apply it to both adapters so the two constructors do not diverge.

### Verification

- [ ] Code change applied
- [ ] A key with an embedded newline fails at construction, not at request time
- [ ] No regressions observed

### Blocked by

**A breaking window.** The clean fix adds a `ConfigError` variant to an enum that
is exhaustive by policy. The v0.5.0 window closed at the `v0.5.0` tag (2026-09-05).
The owner has recorded that this may be deferred again when the v0.6.0 window opens.

### Related

- OI-0051 — the other finding waiting on the same window.
- ADR-0031 — the neighbouring `with_timeout` question, decided the other way: a
  budget is taken verbatim because its error is immediate and legible, whereas an
  unusable header is a *permanent* per-request failure.

---

## OI-0053: The bundle and demo shells have no small-screen layout, and their panes have no visible headings

- **Source:** R0011-0083, R0011-0084, R0011-0087 (Review 0011)
- **Date:** 2026-09-06
- **Decision:** ACCEPT — user-routed **track**, deferred until the v0.6.0 window opens
- **Status:** OPEN (needs a scope decision)

### Problem

Every shell transync ships lays its two panes out as `grid-template-columns: 1fr
1fr` with no media query and no container query, while emitting a
`width=device-width` viewport meta that promises the opposite. On a narrow
viewport the two panes are squeezed side by side rather than stacked. Separately,
the panes carry `aria-label` (R0008-0049 / R0008-0050) but no **visible** caption,
so a sighted reader has no on-screen statement of which pane is source and which
is target.

### Impact

The bundle is the artifact an operator hands to a reader, and the reader's device
is not the operator's. Today a phone gets two unreadable columns. The missing
visible headings are the same gap in the other direction: the accessible name
exists, the visible one does not, so the two audiences get different information.

### Why one entry and not three

These are one product question — *does the bundle shell target narrow viewports at
all?* — not three defects. Answering it decides all three findings; splitting them
invites the shells to drift apart, which is the failure mode `sync_js_drift.rs`
exists to prevent for the engine.

### Required Actions

1. Decide whether narrow-viewport support is in scope for the shipped shells.
2. If yes: one stacking breakpoint applied identically to the bundle shell
   template and the demo shell, plus visible pane headings.
3. If no: record the decision and drop the `width=device-width` promise, or state
   in the manual that the bundle targets desktop widths.

### Verification

- [ ] Decision recorded
- [ ] Change applied to every shell, or the non-support decision documented
- [ ] No regressions observed

### Blocked by

An owner scope decision, deferred to the v0.6.0 window. The browser suite is
Desktop-Chrome-only (ADR-0026), so a narrow-viewport commitment also implies a
viewport dimension in the test matrix — which ADR-0026's re-open conditions
should be checked against before the work starts.

### Related

- ADR-0026 (the browser suite is Chromium-only) — its conditions govern what any
  new viewport coverage would cost.

---

## OI-0054: The Markdown NUL/nesting guard is reachable as `intake::markdown::intake`

- **Source:** the 2026-09-08 documentation drift audit; created by DCR-0053
- **Date:** 2026-09-08
- **Decision:** ACCEPT — track; the rename itself needs a breaking window
- **Status:** OPEN (needs a decision)

### Problem

The Markdown NUL/nesting guard **function** is reachable as
`crate::intake::markdown::intake`. The path says "intake" twice, and the module
and the function share the word for two different things: the module is the
format seam, the function is the guard that substitutes U+FFFD for NUL and
refuses over-nested source before comrak sees it.

DCR-0053 (2026-09-07, `18c1ab4`) created the collision by renaming `parser` to
`intake::markdown`, and deliberately left the function alone — renaming a
public function is a surface change rather than a move, and folding it in would
have broken that change's "the diff must be only the rename" acceptance
condition.

### Why this entry exists

The collision was recorded in three places as "owned by OI-0034", and **that
attribution is wrong**. OI-0034 is *comrak's NUL→U+FFFD substitution desyncs
byte columns*, RESOLVED 2026-08-06 and archived. It owns the NUL normalization
the guard performs — which is what every `TRACE: OI-0034` in the code correctly
cites — but it has never owned a naming decision, and being resolved and
archived it cannot acquire one. So the collision had no live owner. This entry
and ticket `cdadffea` are that owner.

### Impact

Readability only; nothing is incorrect at runtime. The cost is that
`intake::markdown::intake` reads as a mistake to every new reader of the seam,
which is why `intake.rs`'s module doc has had to carry a "not to be confused
with" note since DCR-0035.

### Why it is not a free rename

`intake::markdown` is a `pub mod` of `transync-syntax`, a published crate, so
renaming the function is **breaking** for a consumer depending on it directly.
v0.5.0 closed the sanctioned window, so this sits in the same unopened window as
`LineOffsets::offsets` going private (R0010-0023) and the DCR-0053 rename —
all three invisible through the `transync` facade, all three breaking only for a
direct `transync-syntax` consumer.

### Required Actions

1. Check first whether any external caller needs the function at all. If not,
   making it crate-private removes the surface question and may make the rename
   non-breaking.
2. Otherwise decide between renaming it (`guarded_intake`, `normalize_and_guard`,
   or folding it into `parse`) in the next breaking window, and keeping the name
   with its wrinkle note on the grounds that it is ugly rather than wrong.
3. Correct any remaining "owned by OI-0034" attribution as it is found.

### Verification

- [ ] Decision recorded
- [ ] Change applied, or the keep-the-name decision documented
- [ ] `intake.rs`'s wrinkle note updated to match the outcome

### Blocked by

A sanctioned breaking window for option (a), unless step 1 shows the function
can be made crate-private — in which case nothing blocks it.

### Related

- DCR-0053 (created the collision, and records why it was left alone)
- DCR-0035 (first recorded the wrinkle, when the path was `parser::intake`)
- OI-0034 in `open-issues-archive.md` — the NUL normalization, **not** this

---

## Open Issues Summary

| Issue ID | Title                                                  | Status   | Severity |
|----------|--------------------------------------------------------|----------|----------|
| OI-0016  | Active-block selection scans per scroll frame            | RESOLVED (2026-09-02) | Low      |
| OI-0035  | Injected `data-sync-id` can pre-claim a real block's anchor | RESOLVED (2026-08-23) — archived | Low |
| OI-0037  | Provider payloads bypass the parser's nesting intake guard | RESOLVED (2026-09-01) | Low |
| OI-0038  | A fully-warm run cannot start offline — credentials precede the cache | RESOLVED (2026-09-02) | Low |
| OI-0039  | JS lint gate exits 0 while validating nothing            | RESOLVED (2026-09-04) — format half adopted; lint deferred | Low |
| OI-0040  | Same bytes parsed/spliced repeatedly on the accepted path | RESOLVED (2026-09-04) — prefilter + tripwire; seam deferred | Low (R0009-0035 Medium) |
| OI-0041  | Pre-network setup recomputes derived values              | RESOLVED (2026-09-04) — partial; rest declined/deferred | Low |
| OI-0042  | Two worse-than-linear scans in the degraded regen cascade | RESOLVED (2026-09-04) — all three sites, twin included | Low |
| OI-0043  | Four modules called oversized; one is a real concern bundle | RESOLVED (2026-09-04) — output.rs split, proven pure | Low |
| OI-0044  | Disk cache log: unbounded read, poisonable write, non-converging trim | RESOLVED (2026-09-05) — trim member 2026-09-03; read and write members 2026-09-05 | Low |
| OI-0045  | `serve` under-delivers a truncated body, over-advertises authorities | RESOLVED (2026-09-04) | Low |
| OI-0046  | Three holes in the checking apparatus itself             | RESOLVED (2026-09-04) — all three; (3) found a live defect | Low |
| OI-0047  | Sync engine freezes past its last anchor; silent drift on reflow | RESOLVED (2026-09-04) | Low |
| OI-0048  | Four boundary checks weaker than the inferred contract   | RESOLVED (2026-09-04) — R0009-0052 deferred | Low |
| OI-0049  | Review 0010's non-blocking survivors (16 findings)       | DEFERRED (2026-09-05) — re-trigger recorded | Low |
| OI-0050  | A panicking `serve` connection task is discarded silently | RESOLVED (2026-09-06) — fixed the day it was filed | Low |
| OI-0051  | Every batch clones its profile and duplicates its glossary | OPEN (2026-09-06) — blocked on a breaking window | Low |
| OI-0052  | Checked provider constructors accept invalid header values | OPEN (2026-09-06) — blocked on a breaking window | Medium |
| OI-0053  | Shells have no small-screen layout and no visible pane headings | OPEN (2026-09-06) — needs a scope decision | Low |
| OI-0054  | The NUL/nesting guard is reachable as `intake::markdown::intake` | OPEN (2026-09-08) — needs a decision; rename needs a window | Low |

***

## Entry template — not an issue

Copy this shape when adding an entry above. It lived between OI-0016 and OI-0037
until 2026-09-09 with a `## OI-NNNN:` heading, where it read as a live entry and,
because `NNNN` is not digits, defeated every parser that splits this file on
`## OI-\d{4}` — such a parser folded it into the preceding entry instead. Kept
here, under a heading that cannot be mistaken for an issue id, so the shape stays
available without being counted (ticket `13fcede1`). Every line inside the
fence is indented two spaces so a line-based scan cannot match `^## ` either —
which is the form of parser the original stub actually broke. Dedent on copy.

```markdown
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
```
