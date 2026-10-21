---
type: DCR
title: Reviews 0012 and 0013's fix pass, and the contract each of its thirty-eight findings moved
description: "The decision gate over independent reviews 0012 and 0013 accepted all thirty-eight findings and fixed them in twelve commits. This record carries every contract delta they made: the HTML scanner's script-escape states, template extents and first-attribute rule; a 512-element HTML nesting ceiling; a per-pane author-id namespace; reference definitions resolved document-wide through comrak; prompt, glossary, table, packing and dispatch corrections; filesystem-identity checks on output destinations; the language gate's foreign-letter screen and input domain (ADR-0032); a read-side workdir guard and a snapshot-checking pre-commit hook; narrower promises about coalesced requests and cache integrity; and the retirement of docs/investigation/. The write-once records it narrows or extends stay as written."
tags: [review-gate, R0012, R0013, DCR-0063, ADR-0032, DCR-0013, DCR-0014, DCR-0026, DCR-0028, DCR-0045, DCR-0047, DCR-0048, DCR-0051, DCR-0057, DCR-0062]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-21T00:00:00Z
status: stable
---

# DCR-0063: reviews 0012 and 0013's fix pass, and the contract each finding moved

- **Date:** 2026-10-21.
- **Source:** the `indy-review-gate` run over independent reviews **0012**
  (27 findings, 4 high) and **0013** (11 findings, 2 high), tracked by umbrella
  ticket `56dd1140` (duplicate `45ca7e2f`). Every finding was accepted and
  fixed, in commits `2c9ff70`..`3f2e5cc`. Thirty took the fix the gate
  proposed; the owner chose the route for eight in Phase 2: R0012-0014 (fix B, record C — ADR-0032),
  R0012-0015 (route a), R0012-0016 (route A), R0013-0002 (Stage A + B1),
  R0013-0003 (a + b + c), and R0012-0022 / R0012-0023 / R0013-0011 (retire
  `docs/investigation/`).
- **Narrows or supersedes in part, all unedited:** DCR-0013 Part B (§5),
  DCR-0014's request shape (§6), DCR-0026 §4 (§7), DCR-0057 (§9) and DCR-0028
  §2 (§10). **Extends:** DCR-0047 and DCR-0051 (§1), DCR-0016's template
  exclusion (§2), DCR-0048 (§8) and DCR-0045 (§12).
- **Decision records:** ADR-0032 is new (§12). The living ADR bodies these
  commits edited are ADR-0006, ADR-0012, ADR-0015, ADR-0016, ADR-0021,
  ADR-0024, ADR-0025 and ADR-0028. ADR-0029 was read against R0012-0020 and
  needs no edit.
- **Behaviour changed:** yes, in most sections; *Behaviour changed* below lists
  what a caller, an operator, a pane consumer or a cache sees. Nothing is
  breaking; *Semver* says why.
- **Version:** the manifest stays `0.6.0-dev`.

**Numbering note.** Next free number after DCR-0062.

## 1. The HTML scanner: script escapes, `</template>`, and the first attribute

- **R0012-0004, with R0013-0009.** *Was:* in `script`, the first `</script`
  plus a delimiter closed the element, so in the legacy
  `<script><!-- document.write('<script></script>') //--></script>` idiom the
  inner closer ended it and the outer one was an orphan the balancer dropped,
  leaving a browser inside the script. *Now:* the one scanner
  (`scan_tags_with_state`) models script data's **escaped** and
  **double-escaped** states for `script` in HTML content. `<!--` enters escaped
  with its two dashes counted, so `<!-->` and `<!--->` return at once;
  `<script` plus a delimiter enters double-escaped, where `</script` only steps
  back to escaped and closes nothing; `>` after two or more dashes returns to
  script data from either state. The other seven raw-text and RCDATA names keep
  the first-delimited-closer rule. `balance_fragment` closes a script left
  double-escaped at EOF with `-->` before its closers. `element_extents`,
  `tag_inventory`, the strip, `intake::html` and layer 6 all see the outer
  closer. *Why:* DCR-0047's "terminate where a browser terminates", one state
  earlier; repairing the region was chosen over deleting it. The close probe
  now compares source bytes in place and allocates nothing per `<` (R0013-0009,
  no contract change). Divergence ledger row 20, measured in Chromium 149.
- **R0012-0008, the scanner half.** *Was* (DCR-0051's end-tag model):
  `</template>` took the any-other-end-tag verdict, which stops at a special
  element such as `<p>` or a table cell. *Now:* it takes "in head"'s rule, which
  every insertion mode defers to: with an HTML template open it pops through
  whatever is open above the nearest one; with none, the old verdict stands. No
  other end tag crosses an open template, because `template` is special.
  `contracts.md` §4b's "What is modelled" carries it. *Why:* the balancer
  appended `</p></template></div>` to `<div><template><p>a</template>b</div>`,
  which in Chromium minted an empty `<p>` and closed the sync wrapper (§4a).
- **R0012-0009.** *Was:* `tag_attr_value` let a later duplicate override a
  valueless first attribute. *Now:* it returns the **first** attribute whose
  name matches ASCII-case-insensitively, decoded; a valueless one is `Some("")`
  and `None` means absent. `<annotation-xml encoding encoding="text/html">` is
  therefore MathML content, not an integration point. *Why:* HTML drops a later
  duplicate, and the old reading hid a live `data-sync-id` after a MathML
  `<style>` from the strip. Ledger row 22.
- **R0012-0007 (no contract text change).** HTML-source pane injection takes its
  insertion point from `TagToken::Open.self_closing`, never from a `/>` suffix,
  so `<a href=/>` keeps its anchor outside the attribute value. The dangling
  `=` shape (`<p class=>`) is ticket `4b8827c4`.
- **The browser oracle (ti `ec235f`) is re-blessed on a larger alphabet.**
  `ATOMS` gains `<!--<script>`, `<script><!--` and
  `<annotation-xml encoding encoding="text/html">`; `PHRASES` gains the class
  `script-escape:double-escaped`, and the coverage test requires it and
  `integration-point:duplicate-encoding`. Census: 8,814 of 10,000 agree, 4,336
  abstentions, 10 mechanisms. On this alphabet the §4a/§4b residual is **9 of
  10,000**. The fixed crate's corpus over the old alphabet is byte-identical to
  the pre-fix one, so the movement is the new draws, not the crate changing its
  mind, and ti `525bef`'s "rising on an unchanged alphabet" re-trigger has not
  fired.
- **The divergence ledger** grows from nineteen rows to twenty-three: rows 20–22
  here and in §2, row 23 in §3.

## 2. Template content stays out of the payload (R0012-0008)

*Was:* the extraction walk (`scan_chunks`) tracked template depth on lol_html's
selector stack and asked the syntactic `is_self_closing`. So `<template/>`'s text
was extracted, and a template ended wherever that stack popped it — at an
ancestor's end tag — so in `<div><template>a</div>b` the `b` was translated.

*Now:* each HTML-namespace template's extent comes from **the** walk, from the
start tag's end to the walk's `content_end`, and a text node is template content
when its start offset falls inside one of those ranges; the walk is skipped when
the block holds no `<template` start tag. A label is pushed unless
`!can_have_content()` or (HTML namespace and void), so `<p/>Hello</p>` is
labelled `p`. Spec §9 / DCR-0016's exclusion of template content therefore holds
for `<template/>` and for a template that outlives an ancestor's ignored end tag,
which runs to its own `</template>` or to EOF; an unclosed template keeps the
rest of its block inert, as a browser does. A `template` inside `<svg>` is an
ordinary element whose text is extracted, and inside a MathML text integration
point it is an HTML template again. In an HTML document a template block holding
only template content is `PreservedZeroSegment`, and the template leaf ends at
its own `</template>` instead of absorbing the blocks after it, so block ids
after it renumber.

*Why:* a browser never renders template content, and the selector stack cannot
see a later `</template>`. Ledger row 21. Labels inside exotic foreign content
can still follow lol_html's namespace model; they are hints, not structure.

## 3. HTML nesting has a ceiling (R0013-0002)

- **Stage A, behaviour-invisible.** *Was:* every open pass-through container in
  `intake::html` held a copy of its ancestor path, so `N` nested `<div>`s held
  `N(N+1)/2` ordinals — about 1.6 GB from a 220 KB file, and an allocation
  failure aborts the process. *Now:* a container holds its own ordinal, and a
  block's `ast_path` is built from the open containers' ordinals plus its own.
  Spec 2026-08-20 §4's child-index convention is unchanged; peak intake memory
  for `<div>`×N is linear (`transync-syntax/tests/html_intake_memory.rs`).
- **B1, the ceiling.** `transync_html::MAX_NESTING_DEPTH = 512` and
  `nesting_ceiling_breach(&str) -> Option<usize>` are the one scanner with an
  early exit: the byte offset of the first start tag that would open the 513th
  simultaneously open element, counted on the scanner's open-element stack after
  implied end tags. Voids, self-closed foreign elements, raw text and comments
  open nothing. Basis: Blink's parser depth cap,
  `kMaximumHTMLParserDOMTreeDepth` = 512 (Chrome 155 measures 513 with `html`
  counted), and Nokogiri's Gumbo default of 400.
- **HTML documents are refused.** New
  `ParseError::HtmlTooDeeplyNested { offset, limit }` — the Markdown
  `TooDeeplyNested` message did not fit — and
  `transync_syntax::intake::html::check_nesting_depth`, with
  `intake::html::MAX_NESTING_DEPTH` a re-export. `intake::html::parse` stays
  infallible and accepts any depth. The pipeline's HTML arm runs the check
  before parse: `TransyncError::Parse`, `stable_code()` `parse_failed`, CLI exit
  2, no override flag. `full_rescan_html` needs no guard, because a regenerated
  HTML document is exactly as deep as its source. `offset` indexes the caller's
  bytes, before any NUL substitution.
- **Markdown raw HTML and wasm fall back instead.** `transync_html::extract` and
  `splice` refuse a block past the ceiling before the lol_html rewriter runs
  (divergence 23), so such a block is `HtmlOutcome::ExtractionFailed`: no unit,
  a `fallback_source` row, source bytes in `out.md`, a skipped-node note.
  `ExtractionFailed` now has two causes, and its message names which. Both pane
  renderers present any html fragment whose own bytes breach the ceiling as
  `<pre data-skipped="html-block">` with escaped bytes and the full
  sync-attribute set, whatever the row or the caller's outcome map says, which
  covers the wasm view and edit modes. `scan_tags`, `element_extents`,
  `balance_fragment`, `strip_reserved_sync_attrs` and `tag_inventory` still
  accept any depth.
- **What the count is.** The scanner's open-element stack, because that is the
  stack whose depth the bounded cost scales with. It models implied end tags and
  in-body ignoring of stray table-part start tags. Among the rules it does not
  model: the adoption agency closing an open `<a>` or `<nobr>`; `<button>`
  closing an open button; a heading start tag popping a heading; `<table>`
  outside a cell closing a table; `<rb>`/`<rtc>` inside `<ruby>`; a second
  `<form>`, `<body>` or `<html>`; `<select>` inside a select; a repeated
  `<caption>` or `<colgroup><col>`; `<head>` or `<frameset>` in body; and
  `<noscript>` read as an element. A run of such unclosed tags counts a level
  per tag, so a document a browser builds a few levels deep can be refused
  (measured in Chrome 155 at 520 repetitions; pinned by
  `nesting_tests::the_count_is_the_scanners_stack_not_a_browsers`). Teaching the
  scanner those rules is a separate decision under DCR-0051's one stack.
- **Residual.** Under the ceiling a block's `ast_path` is at most 513 words
  (about 4 KiB), so tiny blocks nested near the cap can still hold about a
  thousand times their size in paths (`Performance.md`, ADR-0016's decision
  drivers). Route B2 — a constant-size path plus incremental scope indexes —
  was not taken.

## 4. Each pane names the author's ids in its own namespace (R0012-0015)

*Was:* both panes carried the author's ids verbatim, and the shells mount both
panes into one document, so an id both carried was one id on two elements and
every same-document lookup from the target pane found the source pane's element.

*Now* — `contracts.md` §4, "The author-id rule's pane half": the source pane
carries the author's ids under `transync-src--`, the target pane under
`transync-tgt--`, and the same-pane references that name them move with them,
in both formats. The additive
`transync_html::namespace_author_ids(html, prefix) -> Cow<str>`, built on the
shared tag reader with no new dependency and no `[features]`, runs on every live
html-bearing pane block after the reserved-namespace strip and before injection
and balancing. Inside open tags
only, it prefixes `id`; `name` on `a` and `map`; the single-ID references (`for`
on `label`, `form`, `list`, `popovertarget`, `commandfor`,
`aria-activedescendant`); every token of the ID-list references (`headers`,
`itemref`, `for` on `output`, and the ARIA list attributes); a fragment-only
`href` / `xlink:href`; `usemap`; and `url(#…)` in `style` and the SVG
presentation attributes. `for` on any other element is never rewritten. The
roster is the HTML Living Standard's attribute index and the WAI-ARIA editor's
draft. Markdown `#` links are prefixed in the comrak AST, reference links
included. Element and attribute names, text, the tag stream and every `data-*`
attribute are untouched, so `data-sync-id` stays the only sync currency.

Pane-only: `out.md` and `out.html` keep the author's ids byte for byte. A pane
file embedded alone keeps its in-pane links, labels and header cells working,
and a deep link or CSS/JS keyed on an author id addresses the prefixed id.
Documented residuals: `#id` selectors in a `<style>` a Markdown block carries,
CSS escapes and `var()` inside `url()`, SVG animation timing references, radio
groups and exclusive `<details>` sharing a `name` across panes, a `#top` link in
a document with no `top` id, and ids on HTML-run pass-through containers, which
never reach a pane. `web/js/wasm-demo.js` resolves its own panes, editor and
strips once at boot, so mounted content cannot take them over. Options not taken:
stripping ids, shadow DOM, an accepted limitation, mount-time JS namespacing.
ADR-0006 and ADR-0025's D6 carry the decision.

## 5. Reference-style links resolve against the whole document (R0013-0003)

*Was* (DCR-0013 Part B): the inline layer appended `Document.ref_defs` — the
definitions found between top-level blocks — to both payloads. A definition
inside a blockquote or list item (CommonMark 0.31.2 example 218), or on a
paragraph's own first lines, was never in that pool, so a reference to it stayed
bracket text on both sides and a dropped or relabelled reference compared
equally empty; a payload could also rewrite or inject a definition unchecked.
Part B's "no false accept" claim fails for every such definition.

*Now:*

- **One map per run.** `transync-core::refmap::ReferenceMap`, built once per
  Markdown run and empty for HTML, holds the document's effective definitions as
  comrak resolves them: a definition counts wherever it sits, and the first one
  wins. Candidates are every `[...]:` the source spells; comrak keys each
  spelling and resolves it, and a label continued across lines inside a
  container is read back exactly as comrak's block parser holds it, through a
  chain of code spans, so no construct a neighbouring definition opens can hide
  one. The map is not a `Document` field, and the wasm render path computes
  nothing new.
- **Per unit.** Each payload is parsed alone, and a label it does not define
  resolves through comrak's `broken_link_callback` against the map, so a dropped
  or relabelled reference is a retryable `inline` rejection. The definitions a
  payload carries must match its source's, label for label and destination for
  destination (titles are translatable): `reference definition [label] changed:
  source defines …, translated defines …`. Gated by `preserve_urls`, like
  destinations. Known limit: a payload that defines and uses a label shadowed by
  an earlier duplicate is judged here against the shadowed one, and scan 4
  catches the effect.
- **Full document, scan 4.** When `preserve_urls` is unset or true, the Markdown
  reparse compares each top-level block's link and image destinations as comrak
  resolves them over the whole source and the whole regenerated document (list
  items item by item), and falls back the moved blocks together with every block
  whose own bytes carry a changed definition. Under `FullReparseFailure::Hard` a
  run that passed can now fail. Residual: a translated title inside a container
  definition charges comrak's reference-expansion budget differently on the two
  sides, which in a document that exhausts the budget can widen the cascade.
- **Discharged.** OI-0040's R0009-0035 leg: with no append there is no
  per-unit pool parse, so the `[`-prefilter and the `unit_count × |ref_defs|`
  warning are removed, and the OI-0040 seam deferral has no self-firing
  re-trigger left.
- **Surface.** The additive
  `transync_syntax::intake::markdown::comrak_options_with_broken_link_callback`.
  `Document.ref_defs` keeps the gap text and no layer reads it. Heading context
  resolves through the map, so a heading citing a container definition reads as
  its link text and its `context_hash` moves once. `VALIDATION_SCHEMA_VERSION`
  does not move, because cache hits are re-validated (ADR-0015).

ADR-0012's protected inline structure, its retry paragraph and its consequences
carry the decision.

## 6. Prompts and the auto-glossary

- **R0012-0006.** *Was:* `--system-prompt` and `--system-prompt-file` replaced
  `[system].prompt` only, while an HTML run compiles `[system].prompt_html`, so
  the shipped HTML prompt was sent. *Now:* both flags replace both bodies — flag
  beats profile, including a profile's own `prompt_html`. The
  missing-`prompt_html` advisory never fires on a run with an override, the
  template-warning scan reads the override and reports a typo under
  `[system].prompt_html:`, and `profile_prompt_hash` moves for such runs, which
  miss the cache once. Markdown runs are byte-identical; the core API and the
  cache-key shape are unchanged.
- **R0012-0012.** *Was:* `parse_extraction_output` read an envelope without
  `terms` as "found nothing"; the row said `Extracted` and the empty harvest was
  filed and replayed. *Now:* `terms` is required, and a reply without it is
  `TranslatorError::MalformedResponse`, so the row is `Failed`, the run uses the
  static glossary only, and nothing is filed under `GlossaryExtractionKey`.
  `{"terms": []}` stays the one spelling of "nothing salient". A harvest filed
  before the fix replays until the cache is cleared.
- **R0012-0013 — narrows DCR-0014's request shape.** *Was* (DCR-0014, the
  record that defines the field): `GlossaryExtractionRequest::existing_terms`
  were "the static glossary's source terms", every one of them, so a compliant
  extractor withheld the global entry DCR-0027 G6's merge keeps beside a
  section-scoped one. *Now:* it lists only the `GlobalAcrossDocument` entries'
  terms, in profile order, through the predicate the merge's conflict set uses,
  so a listed term is exactly a merge conflict. The field's type is unchanged;
  its meaning narrows, and DCR-0014's reason for it — no term slot spent on
  pinned vocabulary — now holds for the global entries only. A profile with
  section-scoped entries gets a new `GlossaryExtractionKey` and one fresh call.
- **R0012-0025.** *Was:* a chain of `str::replace` let the second pass rewrite a
  label the first had inserted. *Now:* `render_prompt_body` fills
  `{{source_language}}` and `{{target_language}}` in one left-to-right pass that
  never rescans an inserted label, as ADR-0006's `fill_template` does, so a label
  reaches the system prompt verbatim, as ADR-0013 and `contracts.md` §2 promise.
  Only the exact tokens match. Residual: a library caller's pre-compiled profile
  can still be re-substituted when a cohort recompiles it (ticket `1e902c74`);
  the CLI never takes that path.

## 7. Split tables merge back through blank lines and lone CRs, in source order

- **R0012-0010 — supersedes DCR-0026 §4's reassembly algorithm.** *Was:* windows
  were spliced as plain text, window 0 verbatim and each later window minus its
  first two lines, "because every input already passed fragment reparse as a
  single table" — but fragment reparse accepts blank lines around a table.
  *Now:* `regen::regenerate_table` (signature unchanged) slices every window with
  `split_table_rows`, window 0 included, discards the blank lines around each,
  and emits window 0's header and delimiter followed by every window's body
  rows, with no trailing line break. One window comes back unchanged; when any
  window of two or more does not slice as exactly one table the result is `""`,
  which the merged-shape check sends down the DCR-0016/DCR-0026 engine-fault
  fallback. Visible: no line break after the last window reaches `out.md`, and
  a window 0 carrying a non-table line faults the parent instead of being
  spliced in.
  `contracts.md` §5's merge paragraph states the algorithm.
- **R0012-0018.** *Was:* `split_table_rows` cut lines on `\n` only, so a lone-CR
  table was refused and an oversize one shipped whole. *Now:* it cuts on
  CommonMark's rule through `LineOffsets`, the crate's one statement of it, and
  each line keeps its terminator's `\r`. A lone-CR table slices and splits into
  row windows; mixed endings slice on each line's own. `TableRows` is unchanged
  (gate ruling: no `line_break` field), so a lone-CR table's windows and merged
  rows are `\r\n`-terminated: comrak reads the same rows, the bytes differ. LF
  and CRLF tables slice byte-identically. The row-window planner's per-row price
  sum, added after v0.5.0 by `314d43f`, cut each piece just before the `\n` and
  under o200k could undercount rows that open with `/`. It is now `RowPrices`,
  which prices each piece at both places a body row's pre-token can begin (after
  the `\n`, and after an opening `/` run) and keeps the larger, so it never
  undercounts under either encoder, and the planner again reaches the plan
  v0.5.0's exact encode reached. For every table v0.5.0 could already slice (LF
  and CRLF), no split point, payload, `source_hash` or cache key moves relative
  to v0.5.0. An oversize lone-CR table, which v0.5.0 shipped whole, now gets row
  windows, so its split points, payloads and cache keys are new.
- **R0012-0024.** *Was:* `per_unit` and `full_reparse_fallbacks` ordered windows
  by id string, so `.w100` sat between `.w10` and `.w11`. *Now:* one key — the
  document index of the block or of the window's parent, the parent id, the
  numeric window ordinal, the full id — with `unit::split::window_parts` the one
  reader of the `<parent>.wNN` convention. `VALIDATION_REPORT_SCHEMA_VERSION`
  does not move, because `contracts.md` §3a already promised document order.

## 8. Packing, dispatch and the visible-text layer

- **R0012-0011, with R0013-0008.** *Was:* the per-unit input estimate encoded
  `source_payload` raw, though the request carries it JSON-escaped and an
  `HtmlSegments` array escaped twice. *Now* (`contracts.md` §2,
  `target_input_tokens_per_batch`): the input side encodes the escaped body,
  through the one escape definition labels also use; the output side still
  reads the raw payload, so DCR-0026's splitter, packer and preflight agree and
  window `source_hash` values do not move. Escape-heavy documents pack into more,
  smaller batches, and a unit whose batch now holds a different mix of raw-HTML
  or row-window units assembles a different instruction, so its `CacheKey`
  misses once, `DiskCache` included. The output-only estimate no longer computes
  the input side (R0013-0008, no contract change).
- **R0013-0005.** *Was:* a cache hit that failed re-validation was re-dispatched
  after every miss, and retry rounds kept that order. *Now* (`contracts.md` §5,
  "Packing is section-coherent"): nothing is reordered, at packing or at
  dispatch. A rejected hit is re-dispatched at its packed position, and retry
  rounds inherit round 1's order. ADR-0015's Implementation paragraph says so;
  its evict-and-re-dispatch rule is otherwise unchanged.
- **R0013-0004.** `validate::text_presence`'s invisible set is maintained
  against Unicode 16.0 `Default_Ignorable_Code_Point` plus U+2800 and
  U+FFF9..U+FFFB, and a test pins it codepoint by codepoint. That adds
  U+1BCA0..U+1BCA3 and U+FFF0..U+FFF8 to the set DCR-0048 enumerates: an
  under-inclusion fix under DCR-0048's own rule that over-inclusion is free, not
  a new decision. Such a payload is retried, then falls back.
- **No contract change:** R0013-0007 (the synthetic cache-validation batch names
  its fields instead of cloning every original unit) and R0012-0019 (a neighbour
  snippet borrows from the source; `context_hash` output is unchanged).

## 9. A coalesced request is not the request its sections would have sent (R0012-0026)

Narrows DCR-0057. It said "a coalesced batch's request is byte-identical to the
one its sections would have received alone", and called coalescing
output-invisible. That holds per unit and fails per request. *Now*
(`contracts.md` §5): under either `coalesce_sections` setting every unit keeps
the same compiled system prompt, the same assembled instruction and the same
`CacheKey` — the cohort pins `profile_prompt_hash` and `glossary_hash`,
`DocumentFacts` pins `instruction_hash`, and every other axis is per unit — so
no cache entry is orphaned. But a coalesced request's `units` array carries the
neighbouring sections' units, and the Structured Output schema's `minItems` /
`maxItems` follow the batch's unit count, so a live provider's output is not
guaranteed equal across the two settings. The decision itself — coalescing on
by default, cache axes unchanged — stands. Pinned by
`pipeline::run_level_tests::coalescing_changes_the_request_not_a_units_prompt`
beside `coalescing_moves_no_cache_key`.

## 10. Hit revalidation is not an integrity check (R0013-0010)

Narrows DCR-0028 §2, which said "a flipped bit in a payload is a
re-translation, not a corruption of output" and that the cache would "never
corrupt output (hit revalidation)". *Now* (`contracts.md` §1, "Recovery is loud,
safe and forward-only"): the log carries no checksum, and ADR-0015's hit
revalidation evicts and re-dispatches a parseable payload only when it fails a
per-unit layer — for `preserved`, the byte-equality proof against the source.
Damage that leaves a `translated` payload valid and same-shaped (one flipped bit
turning `100` into `900`) is not detected and is replayed into the output. A
still-parseable glossary record meets only `merge_auto_glossary`'s term-shape
filter, and a `doc_meta` record no check. No behaviour changes; pinned by
`cache::disk::tests::same_shape_payload_damage_replays_from_the_log`. Whether
the log should carry a payload digest is an open owner question (ticket
`f9a94236`); a digest would not breach ADR-0021's "no new digest, ever", which
concerns identity keying. ADR-0015's decision body and ADR-0021's decision
drivers and consequences carry the narrowed wording, and `settled-questions.md`
says the question is open, not settled.

## 11. CLI output publication: names, identity and staging

Every refusal here exits 4 (`contracts.md` §6).

- **R0012-0001, a clarification that brings the code to the existing rule "an
  `html` directory".** An `html` entry in an `--out-dir` target that is not a
  directory — a regular file, or a symlink judged as the link — is refused like
  any other misshapen entry, in a marked tree too, in the pre-provider preflight
  and in both guard passes of `publish_out_dir`. It is content, never "no output
  at all"; `--force` replaces it.
- **R0012-0005.** A destination or `--out-dir` target with
  `.transync-publish.lock` as **any** path component, in any ASCII letter case,
  is refused in either mode before the provider call and again at publication,
  and `--force` does not waive it. The name check lives in
  `destination_file_name`, so files mode, `--html-out`, `--validation-report`,
  `--out-dir` and staging share it. Backstop, Unix only: a path or any ancestor
  level that is, by `lstat` `dev`+`ino`, the lock marker of the level holding it
  is refused, which catches spellings the volume folds beyond ASCII and a hard
  link to the marker. It is asked before the provider when the marker exists,
  again after the run creates its levels, and again under the lock, so in a
  directory never published into it refuses after the provider call.
  `publish_out_dir` resolves the target's name before creating any level.
  ADR-0024 gains "The marker's name is reserved".
- **R0012-0020, operator-visible text.** The `.transync-out-dir` marker body
  (`OUT_DIR_MARKER_BODY`) says that deleting the marker does not protect a
  directory holding a complete set, and to move or rename the directory instead.
  Recognition is unchanged: name and file type (ADR-0029).
- **R0012-0002.** *Was:* whether two paths named one file was decided by
  spelling. *Now:* by directory-entry identity
  (`output::destination::EntryIdentity`). The parent is resolved as far as it
  exists and held by the `dev`+`ino` of its deepest existing level plus the
  missing tail. The final component is the name its directory stores
  (`read_dir`, then an exact match or an `lstat` `dev`+`ino` match, else the
  typed name), never followed when it is a symlink and never lowercased; a hard
  link is its own entry. Two names that do not exist yet compare under ASCII
  letter case only where a read-only probe shows the volume folds case.
  Containment asks whether the outer entry is among the inner path's directory
  levels, and the `--out-dir` read-side check uses it too.
  `write_fileset_atomic` runs `vet_destinations` again after creating parents,
  so a new-name pair the volume folds another way is refused at publication,
  before any rename. The duplicate refusal reads
  `duplicate output destination: <path> names the same file as <other>`.
- **R0013-0001.** The files-mode staging name has one derivation,
  `<final component>.tmp.<pid>`, and `vet_destinations` reserves that namespace
  for this run's pid — in the CLI preflight and in both of
  `write_fileset_atomic`'s passes — refusing a destination equal to, or inside,
  another destination's staging name, in either order. After staging and before
  the first rename, a destination whose `dev`+`ino` equals a temp staged for
  another destination is refused (Unix only), through the phase-1 rollback.
  ADR-0024 gains "The staging names are reserved too" beside the marker's
  reservation.
- **R0013-0006.** A read-only probe `lstat`s the derived staging name —
  `<name>.tmp.<pid>`, or under `--out-dir` the sibling
  `.<name>.staging.<pid>.<16-char token>` — in the deepest existing level of its
  directory, and refuses, when the volume answers `InvalidFilename`, before
  anything is created, quoting the OS error; files mode asks again in the levels
  it creates. The naming scheme is unchanged. "refusing existing temp" now means
  an occupied name only.

## 12. The language gate's input domain (R0012-0014, R0012-0021)

- **R0012-0014 — ADR-0032.** *Was:* DCR-0045's script shortcut treated Cyrillic,
  the Arabic script and Devanagari as exclusive to Russian, Arabic and Hindi,
  which holds only among the backend's sixteen languages. Ukrainian, Persian or
  ळ-bearing Marathi text took it to `AlreadyTarget(ExclusiveScript)` for those
  targets, and diluted below a fifth it reached a backend that sees only
  128-codepoint block buckets there and answered the same. *Now:* for a Russian,
  Arabic or Hindi target, one letter of the target's script that the target's
  orthography never uses (`ExclusiveScript::foreign`: Cyrillic outside the
  33-letter Russian alphabet; Arabic-block letters outside the Arabic alphabet,
  plus all of Arabic Supplement; the Devanagari letters Hindi never writes, ळ
  among them) answers `Translate(Reason::ForeignOrthography)`. `script::read`
  decides it as `Reading::Foreign` before both the `Dominant` shortcut and the
  `Minority` hand-off, whatever the ratio. The variant is additive, `Reason`
  being `#[non_exhaustive]`; `Basis::ExclusiveScript` now also means no foreign
  letter was seen; Hangul and Kana get no screen, and the shortcut stays for all
  five exclusive scripts. **The input domain** — stated in the crate doc,
  `Gate::verdict`'s `# Input domain` and ADR-0028's body: a verdict is
  meaningful only for text in the sixteen modelled languages. The residual
  misses are Bulgarian against Russian; Nepali, Sanskrit and most Marathi
  against Hindi; any language the backend does not model on a Latin or Han
  target; and Hangul and Kana, which are near-exclusive, not exclusive. This
  extends DCR-0045's "Voidness of the shortcut" to out-of-set input and
  supersedes nothing in it. ADR-0032 records the options and the residuals in
  full.
- **R0012-0021, tests and manifests only.** The containment weld
  (`src/containment.rs`, `cfg(test)`) parses every `*.rs` under `src/`, at any
  depth, with `syn` instead of scraping lines. `whichlang` may be named only in
  `src/backend.rs`, and there only inside function bodies; `#[path]`, directly
  or through `cfg_attr`, and `include!` are refused, an unparseable file is a
  violation, and a flat-token backstop fails any occurrence the walk misses.
  `syn` 2 and `proc-macro2` 1, without default features, join
  `[workspace.dependencies]` and are consumed only as `transync-lang`
  dev-dependencies; `Cargo.lock` gains two edges and no package. The manifest
  rule reads "ONE normal dependency, NO workspace member in `[dependencies]` or
  `[dev-dependencies]`". Documented limit: a macro invoked inside a body that
  expands into a global impl is beyond a syntactic check. ADR-0028's
  consequences say what the weld now checks.

## 13. Scripts and the pre-commit hook

- **R0012-0003.** `scripts/lib/workdir-guard.sh` gains the read-side rule beside
  the disposability rule: a workdir about to be `rm -rf`'d must hold nothing the
  run reads. `transync_guard_read_side <log-tag> <env-var-name> <workdir>
  <read-path>...` resolves each read path component by component, links expanded
  where met and `..` taken physically, compares directories by identity (`-ef`),
  and puts at stake what the `rm` spelling would delete: only the entry for a
  symlink workdir named without a trailing slash, the whole resolved tree
  otherwise, nothing for a workdir that does not exist. It exits the script
  with status 1. Callers: `smoke-live.sh` (repository root, input, profile,
  prompt file), and `smoke.sh` and `test-browser.sh` (repository root);
  `build-wasm.sh` never wipes its staging root and is not one. The trailing-slash
  hole in the disposability rule is ticket `9a675d87`.
- **R0012-0016, owner route A.** The hook's first step sources
  `scripts/lib/commit-snapshot-guard.sh`, and a missing library refuses the
  commit. *Refused* — `NOT CHECKABLE: the index and the working tree disagree
  on gate inputs`, each path named — a gate input the commit changes whose
  working copy differs from the index, and an untracked gate input git does not
  ignore. *Warned* at the end of the run — `THIS VERDICT COVERS A SUPERSET OF
  THE COMMIT` — a gate input the commit does not change whose working copy
  differs. Gate inputs, by path: `crates/**`, root `Cargo.*`, `rust-toolchain*`,
  `.cargo/**`, the rustfmt and clippy configs, the biome, package and tsconfig
  files, `scripts/hooks/**`, `scripts/lib/**`, and the tracked JS corpus.
  `GIT_INDEX_FILE` is honoured, so `git commit -a` and `git commit <paths>` are
  judged on their temporary index. Accepted cost: two sessions editing one
  gate-input file take turns on it. Known limits: `--amend` is judged against
  `HEAD`, and an intent-to-add entry is warned about rather than refused.
- **R0012-0017.** `scripts/smoke-live-gate.sh` names every leg's live test
  exactly — `chat`, `responses`, `anthropic`, `google-vertex`,
  `google-vertex-express`, `google-developer`, and `all` as all six by name.
  Before the first billable run every selected leg's test is looked up with
  `--ignored --exact --list`, whose captured output must carry its `<name>:
  test` line, or the gate refuses, exit 1, naming the test and crate. Runs use `--exact`, and the `PASS` line is
  followed by the legs that ran.
- **R0012-0027.** The model and base-URL shape checks (R0011-0057/0058) run only
  for the selected legs that read each variable, still before any compile;
  `all` checks all six.

## 14. `docs/investigation/` is retired (R0012-0022, R0012-0023, R0013-0011)

*Was* — the 2026-08-08 state recorded in `backlog-archive.md`'s
`untracked-analysis-bundles`: an untracked analysis bundle, regenerated from the
code and read as a source. Three of these findings were that bundle disagreeing
with the code: a web inventory describing the retired format-only JS gate
(R0012-0022), a WASM inventory promising validation ADR-0023 deliberately omits
(R0012-0023), and API inventories that no longer matched exported paths
(R0013-0011). *Now:* it is not produced, not read as a source and not tracked,
and it is absent from disk. graphify's `graphify-out/` at the repository root is
the owner's codebase map in its place: untracked, built only when the owner runs
graphify, and as fresh as that build. `.gitignore` keeps the
`docs/investigation/` line, so a leftover copy is never committed and
`docs_index_drift`, which reads its exclusions from `.gitignore`, never requires
one. *Why:* the owner's route — one map, rebuilt on demand, instead of a second
inventory that drifts unless someone regenerates it.

## Behaviour changed

What a caller, an operator, a pane consumer or a cache sees after upgrading.
Every finding with a visible effect is in at least one of these lists; the last
list names the findings without one.

- **Cache entries that miss once, and re-translate rather than mis-serve:** HTML
  documents whose block ids renumber after a commented-out script's tail or a
  template (§1, §2); units whose payload gains or loses template text, or whose
  segment labels change (§2); headings that cite a container definition (§5);
  HTML runs given a prompt override, runs whose language label spells a
  template variable, because the compiled prompt and its `profile_prompt_hash`
  move, and extraction harvests for profiles with section-scoped entries (§6);
  an oversize lone-CR table, which now goes out as row windows (§7); units
  whose batch mix changes under escape-aware packing (§8).
- **Requests:** an HTML document no longer sends the Paragraph a script's tail
  minted (§1); a raw-HTML or HTML-document payload no longer carries the text of
  `<template/>` or of a template that outlives an ancestor's end tag, does carry
  an SVG `<template>`'s text, and labels `<p/>` as `p` (§2); a heading's context
  reads a container-defined reference as its link text (§5); an HTML run sends
  its override prompt, the system prompt carries a language label as written,
  and the extraction request's `existing_terms` lists only global entries'
  terms (§6); an oversize lone-CR table is sent as row windows (§7);
  escape-heavy documents pack into more, smaller batches, and a rejected hit is
  re-dispatched in packed order (§8).
- **Output bytes.** *Panes:* author ids carry the pane's prefix (§4); a
  commented-out script keeps its outer `</script>`, and one left double-escaped
  at EOF is closed with `-->` before its closers; an anchor injected into a root
  whose unquoted attribute value ends in `/` lands after the value instead of
  inside it; `<div><template><p>a</template>b</div>` gets no
  `</p></template></div>` appended; a reserved `data-sync-id` after
  `<annotation-xml encoding encoding="text/html">` and a MathML `<style>` is
  stripped (§1); an html fragment past the nesting ceiling is a
  `<pre data-skipped="html-block">` placeholder (§3). *Translated document:*
  in an HTML document a commented-out script's tail stays script instead of
  coming back as a translated Paragraph (§1); template content stays source and
  an SVG `<template>`'s text is translated (§2); a merged table ends at its last row, and a lone-CR table's rows are
  `\r\n`-terminated (§7). *Validation report:* rows of a table split into 100
  or more windows list in numeric window order, `.w99` before `.w100`, with no
  schema version change (§7). *Files:* the `.transync-out-dir` marker body
  (§11).
- **New refusals:** exit 2 for an HTML document nested past 512 open elements
  (§3); exit 4 for a non-directory `html`, the lock marker's name, an alias of an
  input or another output by entry identity, a staging-name collision and a
  staging name the volume refuses (§11); exit 1 from the smoke scripts'
  read-side guard, from the hook's `NOT CHECKABLE`, and from the live gate when
  a selected leg's test is missing, renamed or no longer ignored, before any
  paid call (§13).
- **Refusals removed:** the live gate no longer refuses a model or base-URL
  override that only an unselected leg reads (§13).
- **New validation outcomes:** `inline` rejections for a dropped reference or a
  changed definition, scan-4 fallbacks, and a `Hard` run that can now fail (§5);
  a window 0 carrying a non-table line faults its parent into the engine-fault
  fallback (§7); a payload of shorthand format controls retried, then fallen
  back (§8); a Markdown raw-HTML block nested past the ceiling kept as source
  (§3).
- **Messages:** the duplicate-destination refusal names both paths (§11); the
  hook warns when its verdict covers more than the commit, and the live gate
  names the legs that ran after its `PASS` line (§13).
- **Library behaviour:** `parse_extraction_output` rejects an envelope without
  `terms` (§6); `transync_lang::Gate::verdict` returns
  `Translate(Reason::ForeignOrthography)` where it returned `AlreadyTarget`, and
  target-language text quoting one such word is now translated (§12);
  `transync_html`'s scanners, strip and balancer follow §1, and `extract` /
  `splice` follow §2 and §3.
- **No behaviour change:** R0012-0019, R0012-0021, R0012-0022, R0012-0023,
  R0012-0026, R0013-0007, R0013-0008, R0013-0009, R0013-0010, R0013-0011, and
  §3's Stage A.

## Semver

Nothing here is breaking by this project's policy, so none of it needs the
v0.6.0 window's permission (DCR-0062) or counts toward it. It ships in v0.6.0
because that is the next release.

- **Additive public items:** `transync_html::MAX_NESTING_DEPTH`,
  `nesting_ceiling_breach` and `namespace_author_ids`;
  `transync_syntax::intake::html::check_nesting_depth` and its
  `MAX_NESTING_DEPTH` re-export;
  `transync_syntax::intake::markdown::comrak_options_with_broken_link_callback`.
- **Additive variants on `#[non_exhaustive]` enums:**
  `ParseError::HtmlTooDeeplyNested`, which reaches the facade as
  `transync::ParseError`, and `transync_lang::Reason::ForeignOrthography`. No
  exhaustive match can break.
- **Unchanged signatures whose behaviour a library consumer sees:**
  `transync_lang::Gate::verdict` (§12) — `resp-translator`'s
  `source_gate::gate_for` pins `transync-lang` at v0.5.0 and meets it on
  upgrade; `transync::llm::prompt::parse_extraction_output`, tier (b) (§6), for a
  custom `Translator` that hands it a reply without `terms`;
  `transync_html::scan_tags`, `element_extents`, `tag_inventory`,
  `strip_reserved_sync_attrs` and `balance_fragment`, which follow §1's
  script-escape states, `</template>` rule and first-attribute rule;
  `transync_html::extract` and `splice`, which keep template content out (§2)
  and return `Err` for a block past the ceiling (§3);
  `regen::regenerate_table` and `split_table_rows` (§7). Pane
  HTML's prefixed ids reach anything that embeds a pane file or keys CSS or
  script on an author id (§4).
- **Internal only:** the `transync-core` functions that now take a
  `ReferenceMap` are crate-private, and `unit::build_batches` keeps its public
  signature by building its own map. The removed `[`-prefilter and append
  warning were crate-private; the warning was visible only as a
  `transync::pipeline` `tracing` line.
- **Dependencies:** `syn` and `proc-macro2` join `[workspace.dependencies]` as
  `transync-lang` dev-dependencies only, and no package enters `Cargo.lock`.
  `transync-html` and `transync-syntax` gain no dependency and no `[features]`.

## Affected Areas

- `crates/transync-html` — the scanner's script-escape states and `</template>`
  rule, `tag_attr_value`, `nesting_ceiling_breach` / `MAX_NESTING_DEPTH`,
  `namespace_author_ids`, and `extract` / `splice` (template extents, labels,
  the depth refusal).
- `crates/transync-syntax` — `intake::html` (one ordinal per container,
  `check_nesting_depth`, the template leaf), `error`, `outcome`, `regen`,
  `render` and `render::html_pane` (the namespace step, the nested-fragment
  placeholder), `intake::markdown`'s options and `refdefs`, `align`.
- `crates/transync-core` — the new `refmap`; `validate::{inline, full_reparse,
  text_presence}`; `pipeline` and its `dispatch`, `finalize`, `merge` and
  `report`; `batch`; `unit::{split, context}`; `llm::prompt`; `profile`;
  `cache::disk`'s documentation.
- `crates/transync-cli` — `output::{destination, fileset, lock, preflight,
  publish}`, `translate_cmd`, `error`; `tests/dev_scripts.rs` for the scripts.
- `crates/transync-lang` — `script`, `lib`, `containment`, `backend`, its
  manifest and `tests/gate_behaviour.rs`. `crates/transync-wasm` — one engine
  test.
- `scripts/` — `lib/workdir-guard.sh`, the new `lib/commit-snapshot-guard.sh`,
  `hooks/pre-commit`, `smoke-live-gate.sh`, `smoke-live.sh`, `smoke.sh`,
  `test-browser.sh`.
- `web/` — `js/wasm-demo.js`; `tests/html-oracle.spec.js` (re-blessed) and the
  pane-id legs in `scn13`, `scn16` and `wasm`.
- Documents — `contracts.md` §§1, 2, 3, 3a, 4, 4a, 4b, 5 and 6;
  `persistence-and-files.md`, `rough-schema.md`, `settled-questions.md`,
  `source-of-truth-table.md`, `transync-html-divergences.md` and
  `architecture/README.md`; `Developer_Guide.md`, `Performance.md`,
  `Profile_Cookbook.md`, `Troubleshooting.md`, `module-map.md`,
  `release-checklist.md` and the English manual pages; the eight ADR bodies named
  above, and ADR-0032.

## Gates

At `3f2e5cc`: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`
and the workspace suite at two test threads — 61 targets, 1,884 passed, 0
failed, 11 ignored — all exit 0. Each commit names its own tests and which of
them failed before its fix. The two-package `wasm32` gate is the tracked
pre-commit hook's own leg and was not re-run separately.

Over the same code: `scripts/build-wasm.sh` exits 0 with the module at
`raw=1677407B gzip=690505B` against its `raw<=1840000B gzip<=760000B` budget,
and the unmodified `scripts/test-browser.sh` exits 0 end to end, its one
`cargo test` call (the corpus emitter) held to two threads by a wrapper — 52
Playwright tests passed, with the oracle at the census pinned above (8,814 of
10,000 agree, 4,336 abstentions, 10 mechanisms, `sec4a:break-nested` 9).

## Migration / Follow-up

- **Consumers.** Anything that embeds a pane file re-points deep links and
  author-id selectors at `transync-src--…` / `transync-tgt--…` (§4). A direct
  `transync-syntax` caller of `intake::html::parse` runs `check_nesting_depth`
  first (§3). A `transync-lang` consumer keeps a wildcard arm on `Reason`, which
  `#[non_exhaustive]` already requires, and does not treat `AlreadyTarget` as
  proof for input outside the modelled languages; `resp-translator`, pinned to
  v0.5.0, is to be told all three (§12, ADR-0032).
- **Operators.** Stage a gate input whole before committing (§13). Expect the
  one-time cache misses listed above. A disk-cache harvest an earlier build filed
  from an extraction reply without `terms` replays until the cache is cleared
  (§6).
- **Follow-up tickets the gate filed:** `9a675d87` (a symlinked workdir spelled
  with a trailing slash passes `transync_guard_workdir`), `4b8827c4` (a dangling
  `=` before `>` swallows the injected anchor), `f9a94236` (needs decision: a
  payload digest in the disk cache), `1e902c74` (a cohort recompile can
  re-substitute an inserted language label for a pre-compiled library profile)
  and `8d73916b` (test and script scratch roots outside
  `/Volumes/Temp/claude/transync/`). Further follow-ups are filed as TicGit
  tickets tagged `follow-up,indy-review`.
- **Owner questions this record leaves open:** teaching the scanner the
  start-tag rules §3 lists (a DCR-0051 decision); route B2's constant-size path
  (§3); a byte-identical lone-CR merge, which needs a `TableRows` change (§7);
  and, for the language gate, an open-set backend or a positive marker for
  Russian (ADR-0032).

## What this does not do

It edits no record: DCR-0013, DCR-0014, DCR-0016, DCR-0026, DCR-0027,
DCR-0028, DCR-0045, DCR-0047, DCR-0048, DCR-0051 and DCR-0057 stand as written,
and this record carries what moved. It opens and closes no window and changes no version: the
manifest stays `0.6.0-dev`, and the owner's condition for `0.6.0` — complete and
verified — is unchanged. It adds no payload digest, teaches the scanner no
adoption agency, and gives the language gate no open-set backend.
