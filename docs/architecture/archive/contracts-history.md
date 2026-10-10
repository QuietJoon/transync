---
type: archive
title: Contracts history — text moved out of contracts.md, by section and record id
description: The historical half of docs/architecture/contracts.md, moved out by the wave-4 documentation compaction (DCR-0054). It holds the "It read, until that date" recitals, provenance narratives, incident stories, rejected-alternative litigation and the measurements behind normative claims, each verbatim under a heading naming its section, record id and date. It is an archive — verbatim, append-only, never read for current truth. The live contract is docs/architecture/contracts.md.
tags: [archive, contracts]
generated:
  by: claude-code/claude-opus-5-5
  at: 2026-10-06T00:00:00Z
status: deprecated
---

# Contracts history

This is an **archive** (DCR-0054). `docs/architecture/contracts.md` is a living
document: it says what the contract is now. Everything below was in it once and
is kept verbatim, filed by the section it left and the record that changed it.

Read this to answer *what did `contracts.md` say about this before a record
changed it*. For the current contract, read `docs/architecture/contracts.md`;
for the decision itself, read the record named in each heading.

## §0 — Public Rust surface

### `lib.rs` weld — the gap it closed (backlog `public-surface-widening-gate`, 2026-08-06)

*Source: contracts.md §0, the paragraph “`lib.rs` is welded in as a fourth artifact” — “there” is `crates/transync/src/lib.rs`. The sentence below opened that paragraph's body. Current rule: that paragraph.*

A `pub use` added there *alone* used to widen the real surface without failing anything; `lib_rs_exports_nothing_the_documented_list_omits` closes that direction.

### `ListTopologyEntry::depth` widened from `u8` to `u32` (ticket `0d827781`, owner decision 2026-08-06)

*Source: contracts.md §0, the first paragraph of the `ListTopologyEntry::depth` recorded surface decision. Its second paragraph (the consumer break and the untrusted-side rationale) stays in §0, which states the current rule.*

**Recorded surface decision — `ListTopologyEntry::depth` is `u32` (owner decision 2026-08-06, ticket `0d827781`, one of the sanctioned 0.3.0 surface-window changes; taken under owner delegation, with the owner's veto open until the v0.3.0 tag).** The field shipped as a `u8` at v0.2.0. Because the curated surface is frozen between releases, the R0001-0025 overflow fix (ticket `d7658be4`) could only make `structure::inspect_list_topology` count in `u32` and *saturate* into the byte, capped by a `MAX_FINGERPRINTED_LIST_DEPTH` constant and announced on `tracing::warn`. That was sound but lossy: at or past 255 levels the depth field stopped telling items apart, and the list fingerprint's comparison fell back to entry count, entry order, `child_kinds` and the marker facts alone. This window widens the field to `u32`, and the saturating helper, the constant and the ceiling warning are **deleted** rather than deprecated — nothing projects onto a narrower type any more, so there is no ceiling left to document.

### `translated_markdown` renamed to `translated_document` — and the sibling field not taken (ticket `490d97`, owner decision 2026-08-13)

*Source: contracts.md §0, the first paragraph of the `translated_document` recorded surface decision. Its second paragraph (the field-level break §0's table cannot see) stays in §0, which states the current rule.*

**Recorded surface decision — `TranslationOutput.translated_markdown` is renamed to `translated_document` (owner decision 2026-08-13, ticket `490d97`, one of the sanctioned v0.4.0 surface-window changes; decided by the owner directly in session rather than under delegation, so there is no veto window attached to it).** The field is a `String` either way, in the same position, carrying the same value: no behavior changed, no output bytes changed, and no `schema_version` moved. The old name described the *input* format rather than the field — every shipped input path is GFM Markdown, so `translated_markdown` was accurate by coincidence — and the HTML→HTML path tracked under the same ticket hands the pipeline an HTML document and returns HTML in this field, at which point a name promising Markdown is wrong on exactly the runs a consumer most needs to trust it. The alternative the ticket's own *API impact* section assumed — adding `translated_document` as a **sibling field** and leaving the old name in place — would have landed additively, at the price of two permanent names for one value; the owner chose the rename instead, and the sanctioned window is what makes it cheap, since renaming a public field after the v0.4.0 tag would cost a second major bump for a change with no behavior in it.

### `BlockKind::CodeBlock` gains `fenced` — the two indented-code failure modes (ti `457e51`, DCR-0031, 2026-08-16)

*Source: contracts.md §0, the first paragraph of the `fenced: bool` recorded surface decision. Its second paragraph (the break and the no-`#[serde(default)]` rule) stays in §0; the behavior is DCR-0031 and architectural invariant 4.*

**Recorded surface decision — `BlockKind::CodeBlock` gains a `fenced: bool` field (ti `457e51`, one of the sanctioned v0.4.0 surface-window changes).** The variant was `CodeBlock { info: Option<String> }` and is now `CodeBlock { info: Option<String>, fenced: bool }`, where `fenced` records whether the *source* spelled the block with a fence or with four columns of indentation. The parser had been discarding comrak's flag, which is what made an indented block's unit payload its dedented body — and that failed two different ways. A **four-space** block's payload could only reparse as a paragraph, so `validate::fragment_reparse` rejected every attempt and the block burned three provider calls and fell back untranslated: loud, and self-reporting. A block indented **eight columns or more** left a payload that was itself a valid indented code block, so it passed every per-unit layer, was accepted on attempt one, and was then regenerated into an unclosed fence that swallowed the following paragraph — contained only by `validate::full_reparse` and the DCR-0004 cascade, and silent everywhere else. Nothing downstream could tell the two spellings apart, so the flag is the fix rather than an ornament on it; DCR-0031 records the decision it enables (an accepted indented block is re-emitted **fenced**).

### Block IR split — the `Spelling` / `SourceFormat` §0-row forecast (ti `490d97` wave 2, DCR-0034, 2026-09-01)

*Source: contracts.md §0, the last sentence of the block-IR-split recorded surface decision. The forecast was fulfilled differently from its wording: `SourceFormat` crossed the facade with the `TranslateOptions.input_format` option (ti `490d97` wave 5, DCR-0037, commit `6c3c9a9`, 2026-09-03), and its row landed in the same commit. Current rule: that paragraph and §1's `BlockKind` stability entry.*

`Spelling` and `SourceFormat` carry **no §0 row in this window** — the facade does not export them yet; `SourceFormat` reaches it with the alignment map's `input_format` field, and the row lands in the same commit as the export, exactly as §0 requires of every row.

## §1 — `Translator` trait (Rust API contract)

### `DiskCache` failed-append rule — dated lead (OI-0044 / R0009-0081, 2026-09-05)

*Source: contracts.md §1, the `DiskCache` bullet's bold lead. Only the date left the lead; the rule's body is unchanged in §1.*

- **A failed append closes the log for the rest of the process, and that is consumer-visible (OI-0044 / R0009-0081, 2026-09-05).**

### Output types made `#[non_exhaustive]` (owner decision 2026-08-04, pre-tag)

*Source: contracts.md §1, Stability, the read-only output types bullet, as it stood before the wave-4 compaction took the decision date and the open-window tense out of it. Current rule: that bullet.*

- The read-only **output types** are `#[non_exhaustive]` too (owner decision 2026-08-04, pre-tag): `TranslationOutput`, `ValidationReport`, `AlignmentMap`, `AlignmentBlock`, `ValidationSummary`, and `GeneratorMeta`. They are produced by the engine and only *read* downstream, so the attribute costs consumers nothing and makes output-side **field additions non-breaking**; the pattern-side `..` rule below applies to them as well. Field **removals** and renames stay breaking here too, and the 0.4.0 window carries exactly one of them: `TranslationOutput.translated_markdown` is renamed to `translated_document` (ti `490d97` — §0 records the decision). `#[non_exhaustive]` does not soften a rename, because a rest pattern covers the fields a consumer did not name, never a renamed binding. On the JSON wire the alignment-map types stay governed by `schema_version` (§3) — the Rust attribute and the wire policy are independent layers.

### `BlockKind` stability entry — the v0.4.0 and v0.5.0 window narrative (ti `457e51`, ti `490d97` waves 2 and 5, DCR-0034, DCR-0037, 2026-09-03)

*Source: contracts.md §1, Stability, the `BlockKind` bullet, as it stood before the wave-4 compaction rewrote it in the past tense and absorbed `SourceFormat`'s tier-(a) row into it. Current rule: that bullet.*

- `BlockKind` is an **exhaustive** enum, and so are its variants' field sets: adding a variant, removing one, or adding/removing a field on one is breaking-by-policy and rides a version bump. The 0.4.0 window carried one — `CodeBlock` gains `fenced: bool` (ti `457e51`; §0 records the decision, DCR-0031 the behavior it enables). A consumer's `BlockKind::CodeBlock { info }` pattern becomes `BlockKind::CodeBlock { info, .. }`, and the `wire_str()` vocabulary the alignment map and `CacheKey` are keyed on is unchanged — `code-block` still names both spellings, so no `schema_version` moves. **The v0.5.0 window carries two more** (ti `490d97` wave 2, DCR-0034): `Html { block_type: u8 }` narrows to the unit variant `Html`, and `Title` is added. The narrowing breaks any pattern or construction that named the field; the addition breaks **every exhaustive match** a consumer wrote, which is the cost the exhaustive-by-policy rule exists to make visible rather than silent. `wire_str()` is unchanged for every pre-existing kind and `Title` takes the fresh label `"title"`, so again no `schema_version` moves in this window. The two axis types the split introduces — `Spelling` (per block: how the source wrote it) and `SourceFormat` (per document: which intake produced it) — are **exhaustive by policy on `BlockKind`'s own ground**, since a consumer matching a spelling has to be told when a third one appears; they are **tier (c)** engine types today, reached only through a direct `transync-syntax` dependency, and each gets its §0 row in the window that exports it. `SourceFormat`'s row landed in the same window (ti `490d97` wave 5, DCR-0037), with the `TranslateOptions.input_format` option that made it nameable — an additive field on a `#[non_exhaustive]` struct, so no §0 prose paragraph is owed; `Spelling` remains tier (c).

### `provider_unavailable` appended to the `stable_code()` vocabulary (ti `30a744`, DCR-0046, 2026-09-02)

*Source: contracts.md §1, the provider-side `stable_code()` vocabulary list. Only the append date left the row; `crates/transync/tests/error_taxonomy.rs` scrapes that list.*

- `provider_unavailable` — `NoProviderAvailable`; appended 2026-09-02 (ti `30a744`). The run was configured with no provider — the CLI's `--offline` — and then reached work only a provider could do. Distinct from `provider_unsupported` on purpose: that code deliberately admits it cannot tell a configuration fault from a document one, while this one is unambiguously the caller's configuration, which is why it exits **6** (`ConfigurationRejected`) rather than 5.

## §2 — Profile TOML (file-on-disk contract)

### `[render].target_direction`: the ADR-0013 amendment date (OI-0032 / DCR-0015; ADR-0013, 2026-08-03)

Source: `docs/architecture/contracts.md` §2; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). The head keeps the bullet and cites `(ADR-0013)`; the amendment date left it. The bullet in full, before wave 4:

- `[render].target_direction` (OI-0032 / DCR-0015) is **presentation-only**: `"auto"` (default), `"ltr"`, or `"rtl"`, consumed exclusively by the CLI's HTML-bundle emitters. It is never rendered into the system prompt and never part of cache identity, and it does not touch `out.md` or the alignment map at all (it drove no schema bump). Unknown values raise a load warning and normalize to `"auto"`. `--target-direction` overrides it (§6). `"auto"` applies a best-effort RTL primary-subtag table to the target-language label — a presentation hint only; labels stay opaque everywhere else (ADR-0013, amended 2026-08-03).

### `[batching].target_input_tokens_per_batch`: the retired 256-token instruction constant, and what the R0004-0076 warning supplied (ticket `aa92d64f`, 2026-08-06; R0004-0076, 2026-08-11)

Source: `docs/architecture/contracts.md` §2; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). Two clauses left the head: the history of the 256-token constant the measured instruction replaced ("its unconditional part alone outgrew the 256-token constant that used to stand for it"), and the closing narrative of the R0004-0076 warning ("the diagnosis is what was missing, not an abort"). Every rule in the bullet stays in the head. The bullet in full, before wave 4:

- `[batching].target_input_tokens_per_batch` (DCR-0012) tunes the per-batch input budget from the profile; resolution mirrors `max_units_per_batch` (a caller-set `TranslateOptions` value differing from the built-in default wins, else the profile, else the built-in `6000`). Range `>= 1`. **What the budget covers.** Reserved off this target once per batch: the *encoded* constant user-message envelope — the assembled instruction plus the JSON framing `llm::prompt` wraps it in — the *encoded* compiled system prompt (glossary included, R0008-0029) and the *encoded* `source_language` / `target_language` labels. The remainder is filled with per-unit estimates that encode the payload, id, kind, context hints, and the `constraints` + `retry` hint JSON exactly as `llm::prompt` serializes it. **Nothing in that once-per-batch reserve is an allowance for a string the crate can produce** (ticket `aa92d64f`): the language labels are measured because ADR-0013 makes them opaque strings of any length (R0001-0013), and the instruction is measured because it is assembled, not fixed — its unconditional part alone outgrew the 256-token constant that used to stand for it, and five conditional clauses stack on top. Two of those clauses follow the profile (`[constraints].preserve_urls` unset/`true` adds the link/image-destination sentence; `preserve_code_identifiers = true` adds the code-span sentence). The other two — the raw-HTML segment contract and the DCR-0026 row-window contract — follow *batch membership*, which is what packing decides, so each is reserved on the document-level upper bound instead: **whenever the document holds at least one html unit, every batch of that document reserves the html clause; whenever packing produced at least one row-window unit, every batch of that document reserves the row-window clause**. A document with neither kind of unit pays nothing for either; a document that has one over-reserves for the batches that do not carry it rather than under-reserving for the ones that do. The fifth follows the **run**, not the batch or the profile: an `input_format = html` run adds the HTML-document clause to every batch it dispatches (ti `490d97` wave 5, DCR-0037), which is a document-level verdict for the same reason the two above are. What remains a tuned constant is fixed-shape framing whose size does not follow the run: the per-unit *input* JSON framing (`{"unit_id": …, "block_kind": …}` and its separators — this crate does serialize that, in `llm::prompt::build_user_prompt`, but identically for every unit, so it is kept as a per-unit estimate rather than re-serialized per unit) and the response-side envelope, which only the model writes. A containment test pins that the per-batch reserve plus the per-unit estimates still bound the whole assembled request. **The same budget packs every round**, first dispatch and retry alike (R0001-0012) — which is why both document-level membership verdicts are handed down to the per-batch retry packer, as one value, rather than re-derived from the one batch it can see — see §5. **A reserve that has eaten the whole target is named (R0004-0076).** The packer derives the payload target as `target − reserve` floored at `1`, so a profile whose rendered prompt alone meets or exceeds `target_input_tokens_per_batch` does not fail: it packs one unit per batch and sends every request over the target the reserve exists to defend. `unit::build_batches` raises one `tracing::warn` on the `transync::profile` channel for that state — once per run per distinct cohort reserve, not once per section — naming this knob as the one that moves it. A warning and not a refusal, because this target is a documented **soft** cap and a single over-budget unit deliberately still ships (§5); the diagnosis is what was missing, not an abort. This is the input-side counterpart of the output axis's `output_budget_warnings` preflight, which names units instead and rides the validation report.

### `[[glossary]].scope`: the `"section"` deferral and its three refusal gates (DCR-0027; ADR-0014, 2026-07-13; EXT-2026-07 P0-3; R0001-0006, ti 0ed6eb)

Source: `docs/architecture/contracts.md` §2; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). Until v0.4.0 a `scope = "section"` entry was refused. The head now states only that earlier versions treated it as a time-boxed deferral, what makes it effective, and that no gate refuses it; the reasoning for the deferral and the story of the three gates' retirement left it. The bullet in full, before wave 4:

- `[[glossary]].scope` wire values are `"global"` (the default) and `"section"`; the long forms `"global_across_document"` / `"conditional_on_section"` are accepted as deserialization aliases. `"global"` entries render into the compiled system prompt of every batch. **`"section"` is effective since v0.4.0 (DCR-0027)**: it renders into the compiled prompt of the batches belonging to its own sections and into no other. It was a **time-boxed deferral** until then — rejected at three gates with a `ProfileError::Unsupported` that is now removed, because a section-blind packer left no batch that belonged to one section and honoring the scope would have meant steering the whole document with one section's term (ADR-0014, amended 2026-07-13; EXT-2026-07 P0-3). What re-admitted it is §5's packing policy plus the `sections` selector below; the three gates retired together with the fallibility of `unit::build_batches`, which existed only to raise this refusal (R0001-0006, ti 0ed6eb).

### `[[glossary]]` entry content: the reserved-scope refusal DCR-0027 retired (R0001-0018 / R0001-0019, 2026-07-15; DCR-0027, 2026-08-09)

Source: `docs/architecture/contracts.md` §2; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). Three provenance phrases left the head — "DCR-0027 relaxed this from once per document, which predated any scope that could say *where*" with its back-reference "keep the old rule" (now the citation `(DCR-0027)` and a present-tense statement that two `global` entries on one term meet in every section), "since DCR-0027 retired the reserved-scope refusal" (now the citation `(DCR-0027)` after the bold claim), and "now" in "which is why batching now has one". The bullet in full, before wave 4:

- **`[[glossary]]` entry content (R0001-0018 / R0001-0019).** Every entry is rendered verbatim into every batch's system prompt as one `- "source" → "target"` bullet, so an entry that cannot mean what it says is dropped and named rather than sent. `source` and `target` must both carry non-whitespace text: an empty `source` would read as "this rule applies to every term" and an empty `target` as "delete this term". A source term may be claimed **once per place the claims can meet** (DCR-0027 relaxed this from once per document, which predated any scope that could say *where*): two `global` entries on one term keep the old rule — terms are compared trimmed, NFC-normalized and case-folded (the same identity the auto-glossary merge uses for static-vs-extracted conflicts, and the same one section selectors have — normalization is what makes it an identity rather than a spelling: two encodings of one accented term, the composed one an editor types and the decomposed one a macOS-originating file carries, are one term, R0004-0080), the **first** entry wins, and the later one is dropped with a warning that distinguishes a byte-identical repeat from a conflicting rendering; two `section` entries on one term are legal while their `sections` selectors are **disjoint**, and a later one sharing a selector loses under the same first-wins rule (selector overlap is document-independent, so the loader settles it); and a `global` and a `section` entry on one term **both survive**, which is the override pattern the scope exists for — which of the two reaches a given section's prompt is decided per section, not per document. Rejections are path-qualified load warnings (`glossary[i].source …`), not errors: the profile stays loadable and simply translates without the entry. **Three gates** — `profile::load_profile`; the translate boundary, where it runs *ahead of* the auto-glossary preflight so the merge's static-wins rule keys on the effective entries; and `unit::build_batches`, the `#[doc(hidden)]` batching entry point, which compiles a prompt body of its own without crossing that boundary (ti 5f6664). All three are warn-only, like the `[batching]` knobs; since DCR-0027 retired the reserved-scope refusal, **no profile check anywhere raises an error but the loader's own `MissingField` / `Malformed`**. Three gates can run the same check without repeating a diagnosis because **every warning the check emits names an entry it removed**: the gate that acts is the only gate that can speak, so the exactly-once rule (R0001-0032) holds by construction rather than by the gates agreeing to stay quiet. The renderer is the gate-independent half of the guarantee but only half of it: the escape below is unconditional and an entry with an empty term is skipped, so those two survive a `ProfileMetadata` that crossed no gate at all — the claimed-once rule is not a rendering property and has to be enforced at a gate, which is why batching now has one. Note that dropping an entry changes the compiled prompt and therefore the cache identity of the run; every gate past the loader rewinds an already-compiled `prompt_body` to its template first, so a drop can never leave a stale glossary section beside the effective one.

## §3 — AlignmentMap JSON (durable wire — `schema_version 1.3.0`)

### Anchors pair by identical id: the ticket date and the R0003-0002 drift-warning narrative (ticket `d3acc3`, 2026-08-09; R0003-0002)

Source: `docs/architecture/contracts.md` §3; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). The bold lead lost its date, "now" left the invariant sentence, and the closing sentence that narrated the R0003-0002 fix (the drift warning used to resolve `target_block_id` while the scroll handler resolved `source_block_id`) became a present-tense statement that both look up the row's `source_block_id`. The bullet in full, before wave 4:

- **Anchors pair by IDENTICAL id, normatively (schema 1.x, ticket `d3acc3` 2026-08-09).** `source_block_id` and `target_block_id` are equal in every row of every map this project emits — ADR-0001 decided that ids survive translation, and `build_alignment_map` writes the same `BlockId` into both fields — and that equality is now a **stated invariant of schema 1.x rather than an incidental property of the emitter**. A consumer pairs the two panes by looking the *same* `data-sync-id` up on the other side; the source/target indirection is **reserved**, in the same sense as `parent_id` and the `child-only` `sync_role`, for a future revision in which the two documents' block sets may genuinely diverge. Until that revision exists there is nothing for the indirection to express, and routing today's pairing through it would be a second way of saying "identity" that can silently disagree with the first. The reference consumer enforces its half: a row whose `target_block_id` is a non-empty string different from its `source_block_id` describes a pairing the engine will not perform, so an in-band map carrying one is **refused** (a mount that renders and never syncs is the degradation §3's gates exist to prevent), while the forward-minor policy above applies unchanged — the same row in a newer-minor map is warned about and paired by the source id. This is the R0003-0002 resolution: the drift warning and the scroll handler now read the one id both of them key on, instead of the warning resolving `target_block_id` while the handler resolved `source_block_id`.

## §3a — ValidationReport JSON (durable wire — `schema_version 1.1.0`)

### `VALIDATION_REPORT_SCHEMA_VERSION` on the §0 surface: the 0.3.0 surface window (owner decision 2026-08-06)

Source: `docs/architecture/contracts.md` §3a; the original bullet below is verbatim as on `origin/master` at `755d7ba`, moved by wave 4 (DCR-0054). The provenance clause ("added in the sanctioned 0.3.0 surface window — owner decision 2026-08-06", "restoring the symmetry") shortened to "added in 0.3.0" and "symmetric with". The bullet in full, before wave 4:

- **The constant behind `schema_version` is on the §0 surface**, as `transync::VALIDATION_REPORT_SCHEMA_VERSION` (tier a, added in the sanctioned 0.3.0 surface window — owner decision 2026-08-06), restoring the symmetry with §3's `transync::ALIGNMENT_SCHEMA_VERSION`. The two say different things and both are needed: the **constant is the version the linked build emits**, so a Rust caller can pin or branch on it at compile time without a run; the **serialized field is the version a given file declares**, which is the only honest discriminator for a report read off disk, since it may have been written by another version. So branch on `report.schema_version` when interpreting an artifact, and read the constant when asserting what *this* build produces — never the reverse.

## §4 — Annotated HTML attribute contract

Verbatim text that left `docs/architecture/contracts.md` §4, §4a and §4b when the section was compacted in place (DCR-0054, wave 4). This is history, not current truth: the current rule is in `contracts.md` under the same section number. Each entry is headed by the subsection, the topic and the original record ids and date; its paragraphs are the removed sentences, in source order.

### §4 wrapper rules — the `non-sync` attribute rule before the row decided it (ti `18b9c3`, DCR-0044, 2026-09-01)

**A `non-sync` row carries `data-block-kind` and nothing else (ti `18b9c3`, 2026-09-01).**

Until this date it was a literal `BlockKind::ThematicBreak` test in `render_block`: correct for the only kind that could reach it, and silently wrong for the next one. `BlockKind::Title` is `non-sync` by decision D5, so it would have rendered with a real `data-sync-id` while its own row denied it — the row and the DOM disagreeing about one block, which is the thing every anchor rule in this section rests on not happening. Nothing minted a `Title` when this landed; the HTML intake (ADR-0025) does now, and the decision was discharged ahead of it rather than at it.

Thematic break: rendered as a bare `<hr data-block-kind="thematic-break">` — byte-identical to what it was before the decision moved.

What the pane emits in its place has since been settled by the HTML render path: an HTML-source pane emits **no element at all** for a non-sync row (see *HTML-source panes* below).

### §4 html block — "orphan" before §4b narrowed it (ti `9b4d66` / `307283` / `895fb7` / `da6bb5` / `bb961a`, DCR-0051, 2026-09-05)

(**"orphan" narrowed 2026-09-05, ti `9b4d66` / `307283` / `895fb7` / `da6bb5` / `bb961a`, DCR-0051 — see §4b**)

### §4 html block — the withheld closer and its fuzz measurement (ti `95f55b`, 2026-09-01)

(ti `95f55b`, 2026-09-01 — the balancer re-scans its own append and keeps it only where the scanner reads it back as markup, because a swallowed closer buys nothing and feeds unbounded re-balance growth: 15,726 violations across 200,000 fuzz iterations before the fix).

### §4 html block — an unterminated trailing region before the repair (ti `c1f9a8`, DCR-0047, 2026-09-03; `--!>` R0010-0049 / DCR-0050)

**A fragment that ENDS inside an unterminated region is repaired too, as of ti `c1f9a8` (2026-09-03).**

It used to pass through untouched — `95f55b` stopped the balancer appending a closer the region would swallow, which bought idempotence and left the fragment unrepaired.

The region is now **terminated** where a browser terminates it (a comment at `-->` **or `--!>`** — HTML's comment-end-bang state closes on the second, and reading only the first made `<!-- a --!><div>y<!-- b -->` one comment with the `div` hidden inside it, R0010-0049 / DCR-0050 — a CDATA section at `]]>`, a bogus comment at `>`) or **deleted** where a browser abandons it or will not leave it.

The harm was larger than the trailing bytes: an unterminated COMMENT swallows everything after it and so does a `<plaintext>` region, while a bogus comment, an HTML-content CDATA and a cut tag each end at the first `>` — which in a mounted pane is the wrapper's own `</div>`, so the wrapper closed inside the region, stayed open, and every following block nested inside this one's wrapper.

### §4a foreign-content carve-out (stated in §4's html-block rule) — what the contract said before DCR-0041 closed it (ti `e77173`, DCR-0041, 2026-09-01)

**The foreign-content carve-out is closed (ti `e77173`, DCR-0041, 2026-09-01; refined by DCR-0042 and DCR-0043).**

It read, until that date: HTML's breakout tags and integration points are not modelled, so `<div class="wrap"><svg><div>x</svg></div>` left the inner `div` open in a browser while the walk closed it at `</svg>`, and the next block's anchor mounted inside the wrapper.

### §4 html block — the self-closing slash read the XML way ("Opened", ti `490d97` wave 1, 2026-08-23)

**"Opened" means what a browser means by it (ti `490d97` wave 1, 2026-08-23).**

Until that date the balancer's walk read the slash the way XML means it, and this guarantee was **false** for exactly those tags: the walk never pushed one, the author's `</div>` was deleted as an orphan, and the fragment reached the pane still open — so the wrapper's own `</div>` closed the fragment, the wrapper stayed open, and the next block's anchor mounted inside it, violating §4a's direct-child rule. Reachable from a plain author-written `<div/>` with no strip involved, and measured on a real `--html-out` bundle on both sides of the fix (`web/tests/scn13.spec.js`, test n).

### §4 reserved-namespace strip — how DOMPurify's `data-*` default was measured (OI-0035, ti `490d97` wave 1, commit `eedc9e3`, 2026-08-22)

Source Markdown is untrusted data (invariant 7) and DOMPurify's default keeps `data-*` attributes (**measured** 2026-08-22 against the vendored build, `web/tests/scn13.spec.js` test `m`, commit `eedc9e3` — the date was written as 2026-08-20 from the plan's own writing date and corrected 2026-08-23 by `git log -S`; spec §15 item 3 had flagged the `ALLOW_DATA_ATTR` default as *inferred*, and this is the measurement that retired the inference), so without the strip an author's `data-sync-id` reached the DOM as an anchor the engine could not tell from the renderer's own.

### §4 reserved-namespace strip — the seam rule's motivating case and the "attributes do not paint" overclaim (ti `490d97` wave 1, 2026-08-23)

Before that rule, `<div data-sync-id="a b="c">x` stripped to `<divc">x` and the inventory moved from `["div"]` to `["divc"]`.

And "it never changes rendered appearance — attributes do not paint" was an overclaim: the shipped shells' CSS — `crates/transync-cli/web/index.html.tpl`, which is what the `--html-out` bundle is generated from (`include_str!` in `crates/transync-cli/src/output/bundle.rs`), and the `web/index.html` demo shell alike — styles `[data-fallback="fallback_source"]`, `[data-fallback="partially_translated"]` and `pre[data-skipped]`, so stripping an author's copy of a reserved attribute does change its tint — rightly, because that presentation is engine-owned.

### §4 reserved-namespace strip — the stray-`=` over-deletion, closed (ti `415cdb`, ti `549b20`, DCR-0041, 2026-09-01)

What the strip guarantees is narrower: only reserved names, only inside element open tags, and no element name and no other attribute moves — and **the one measured exception is closed (ti `415cdb`, 2026-09-01)**. It read: across a stray `=` the strip over-deleted, so `strip_reserved_sync_attrs("<div =data-sync-id=\"x\">y")` returned `<div =>y` where a browser folds those bytes into **one junk attribute named `=data-sync-id`** — nothing reserved present, yet the cut ran and a non-reserved attribute moved. The direction was always safe (over-deletion, never under-deletion: a 51-case browser equivalence probe found 50/51 equal, that one over-deletion, and **zero** under-deletions), so the impostor-anchor property was never at risk. A stray `=` where an attribute name would start now begins that name, the way HTML's before-attribute-name state reads it, so the input above comes back unchanged (`a_stray_equals_begins_an_attribute_name_instead_of_being_skipped`). The cause was two attribute walks where there should be one: ti `549b20` aligned `scan_tags`' stray-`=` handling with the browser while `collect_reserved_attr_spans` kept its own. DCR-0041 collapsed the strip's walk — with `tag_has_any_attr` and `tag_attr_value` — onto the single shared `walk_attrs`, and `415cdb` aligned that walk with the browser, so the scanner and the strip can no longer disagree about where an attribute begins.

### §4 rendering mechanism — the DCR-0017 rework (DCR-0017, 2026-08-04)

**Rendering mechanism (DCR-0017, 2026-08-04) — not part of the contract, stated so the contract's guarantees are checkable.**

It does **not** reparse blocks individually and does **not** post-process Comrak's HTML with string surgery; the retired mechanism is why the tightness clause above changed.

Attribute names, attribute order, wrapper elements, and the one-block-per-line output shape are unchanged by that rework.

### §4a map checking — what a malformed map rendered before the refusal (R0002-0010, R0002-0011, R0003-0060; predates the 2026-08-17 root commit)

The guarantee above is per alignment *row*, so it says nothing about a map that repeats a row, omits one, or gives one a byte range that is not a slice of the pane it indexes — and all three used to render: a repeated `source_block_id` let whichever row the renderer indexed last decide that id's attributes and byte ranges, a block with no row was skipped outright, and a range that ran backwards, past the end, or into the middle of a UTF-8 character was clamped, reordered and snapped until it sliced *something* — an empty, truncated or character-short block under the row's own correct anchor.

### §4a layout probe — what it read, and the mount-only budget (R0003-0076; R0009-0024, commit `69701cd`, 2026-08-26; OI-0047, 2026-09-04)

A precondition whose violation is "a silent layout regression" cannot be left to documentation alone, so `mountSync` probes it once per pane at mount time and `console.warn`s on violation — naming the element the offsets are actually being measured against.

**What it reads changed 2026-08-26 (R0009-0024, commit `69701cd`):** the probe used to read `offsetParent` on one representative anchor, which answers a question about *that anchor* rather than the precondition. The precondition is a fact about the **pane** — that it is positioned — so the probe now reads `getComputedStyle(pane).position` and returns when it is non-static. `offsetParent` survives on the failure path only, where naming the ancestor the offsets are really measured against is exactly what makes the warning actionable.

**The budget widened 2026-09-04 (OI-0047), and the widening is the contract now:** the probe runs at mount *and* on every coalesced reflow recompute, so it is one property read per pane **per recompute frame** rather than one per pane at mount. The reason is that the old budget bought its cheapness by going silent exactly when geometry changes — a pane that becomes unpositioned *after* mount, or a map row whose anchor disappears on reflow, produced no diagnostic at all.

### §4a reflow — the docstring's earlier answer (ticket `d3acc3`, OI-0024 item 1; predates the 2026-08-17 root commit)

The docstring's answer used to be "destroy and re-mount", which a consumer can only act on by guessing when a reflow happened.

### §4a bottom clamp — the dead zone before the clamp (OI-0047, 2026-09-04; ti `8cd7ba`, DCR-0052, 2026-09-05)

**Past the last claimed anchor the follower clamps rather than freezes (OI-0047, 2026-09-04; stated here 2026-09-05 after ti `8cd7ba`).**

Until that date it answered "no active block" and the follower simply stopped, so a pane carrying more than a viewport of unanchored content below its last anchor — inside the scroll box — left the follower wherever it happened to be. The change is **consumer-visible**: a reader scrolling into that tail now MOVES the follower, to the partner's copy of that same anchor. It is not a route by which unclaimed content drives sync, and the distinction is worth stating because it has already been misread once.

### §4b two open-element stacks before one (DCR-0051, 2026-09-05)

**Added 2026-09-05 (DCR-0051).** §4's balancing guarantee rests on one question — "which element does this closer close, and what is still open at the end?" — and `transync-html` answers it by hand. Until this date it answered it **twice**: `scan_tags` kept a content-mode stack and `walk_elements` kept an open-element stack, and the two popped differently from each other and from a browser. That is the single root cause behind five tickets, two of which put an attacker-chosen `data-sync-id` into a live DOM and thereby defeated the render-side strip §4a's OI-0035 closure depends on. There is now **one** stack: `scan_tags_with_state` keeps it, every mutation is reported on the token, and `walk_elements` replays it and adds only extents, close spans and orphan spans.

### §4b table scope — the crate's earlier pop (ti `307283`, R0010-0039, DCR-0051, 2026-09-05)

Table scope is what makes `<div><table>a</div>` leave the table open: `table` terminates the search, a browser ignores the closer, and the crate used to pop the table and let the pane's own `</div>` be ignored the same way.

### §4b browser-oracle census at DCR-0051 (ti `ec235f`, ti `525bef`, DCR-0050 / DCR-0051, 2026-09-05)

Measured residual: 8 of 10,000 generated fragments still break §4a this way, down from 1,358.

On the alphabet DCR-0050 blessed and DCR-0051 did not change: 8,619 of 10,000 generated fragments in full agreement, 3,863 browser-declared abstentions, 9 divergence mechanisms, `sec4a:break-nested` 8, `reserved:dom-only` **0**.

### §4b orphan rule — the lead as it read at DCR-0051 (DCR-0051, 2026-09-05)

**"Orphan close tags are dropped" is narrower than it was, and the line is the pane's.**

## §5 — Retry / fallback policy

Text that left `contracts.md` §5, §5a and §5b in the wave-4 compaction (DCR-0054), quoted verbatim. Each quote shows a sentence or paragraph as it stood in the section and paragraph named. That paragraph in `contracts.md` holds the current rule; nothing here is current truth.

### Retry rounds before re-packing (R0001-0012, 2026-07-15)

Source: `docs/architecture/contracts.md` §5, paragraph **A round is a pass, not a request (R0001-0012).** The sentence was removed; the paragraph states the current re-packing rule.

> Before this, a retry round shipped as one batch of any size, since the packer's only call site was the initial grouping.

### The retry side channel as a v0.2 addition (ADR-0009, DCR-0009, 2026-07-13)

Source: `docs/architecture/contracts.md` §5, paragraph *Validation retry keeps the payload and scope verbatim (ADR-0009)*. The sentence was reworded in place to drop the version lead ("What v0.2 adds", "now carries"); the rule is unchanged.

> What v0.2 adds is a **non-content** side channel: the re-dispatched unit now carries a machine-readable `retry` hint (`attempt`, rejecting `ValidationLayer`, truncated `reason`) alongside — never inside — the payload, so a provider can correct the structural failure without the guidance leaking into the output (ADR-0009's Considered-Option 3).

### Row-window merge status: the OQ-1 resolution date (DCR-0026 OQ-1, 2026-08-09)

Source: `docs/architecture/contracts.md` §5, the row-window merge paragraph after **A row-window split is packing, not retrying (DCR-0026).** The sentence was kept in place without the resolution date.

> Merged status: unanimity carries (all-translated is `translated`, all-preserved is `preserved`, all-failed is `fallback_source` with the source table emitted), and **anything mixed is `partially_translated`** — a terminally-failed window contributes its own **source rows** and costs its rows rather than the table, with the failed windows named on the report's warnings channel (DCR-0026 OQ-1, resolved (b) on 2026-08-09).

## §5a — Unit validity: provisional vs final

### Visible-text presence: the provenance lead (ti c887bc, DCR-0048, 2026-09-04)

Source: `docs/architecture/contracts.md` §5a, tier 1 **Provisional (per-unit):**. The provenance lead "since ti `c887bc`" became a `(ti c887bc)` citation; the rejection clause is unchanged.

> The last is the only per-unit layer that reads content rather than structure: since ti `c887bc` a unit is rejected when the source payload carried visible text and the translation carries none — empty, whitespace-only, or zero-width/format-character-only.

### Per-unit-attributed schema tier: the provenance lead (OI-0031, DCR-0014, 2026-08-03)

Source: `docs/architecture/contracts.md` §5a, tier 1 **Provisional (per-unit):**. The provenance lead "Since OI-0031" became an `(OI-0031)` citation.

> Since OI-0031 the schema tier is **per-unit-attributed**: a batch-envelope fault rejects only the units it implicates, so an innocent batch-mate can reach provisional validity — and the cache — in a round where the batch as a whole faulted.

### Format-dispatched document gate: the provenance lead (ti 490d97 wave 4, DCR-0036, 2026-09-03)

Source: `docs/architecture/contracts.md` §5a, tier 2 **Final (document-level):**. The provenance lead became a `(ti 490d97 wave 4, DCR-0036)` citation.

> Since ti `490d97` wave 4 (DCR-0036) the gate dispatches on `Document.format`: a Markdown run takes the comrak reparse (`validate::full_reparse`), an HTML run takes the scanner rescan (`validate::full_rescan_html` — ordered tag ledger, fresh segmentation per D9, gap byte-identity including preamble and tail, boundary sanity).

### Cache consequences as a v0.2 rule (DCR-0009, 2026-07-13)

Source: `docs/architecture/contracts.md` §5a, the *Cache consequences* paragraph. The `(v0.2)` tag was dropped; the rule is unchanged.

> Cache consequences (v0.2): the cache stores **provisional** results.

### Cache identity: the rule that replaced case-by-case settlement (R0001-0005, ti c02f69, owner decision 2026-08-06)

Source: `docs/architecture/contracts.md` §5a, paragraph **Cache identity: the prompt bytes, and nothing else (owner decision 2026-08-06; R0001-0005).** "what used to be settled case by case" became "every candidate axis"; the list of axes in and out is unchanged.

> That single rule settles what used to be settled case by case: the packing budget, the tokenizer hint and the output ceiling stay out because none of them is a byte the model read — the ceiling does reach the wire, as a request parameter rather than as prompt text, and *Output ceiling and cache identity* (§1) reaches the same conclusion the long way, on the narrower ground that a truncated response never gets cached; the unit's context hints are in (`context_hash` covers exactly the `ContextHints` the user prompt serializes — document title, section path with heading levels, neighbor kinds and summaries); the two labels the prompt spells beside the payload are in, as `block_kind` and `input_mode`; and the **instruction the unit's batch assembled** is in, as `instruction_hash`.

### Injective encoding: the latent glossary and fingerprint collisions (R0002-0007, R0002-0008, 2026-08-07)

Source: `docs/architecture/contracts.md` §5a, paragraph **Injective encoding is the rule for every composed axis, not a `context_hash` local (R0002-0007, R0002-0008).** and the paragraph after its list. Before the fix, `glossary_hash` terminated its fields with `0` bytes. The head keeps the framing rule, the reason for it, and the principle that an axis may not rely on another axis.

> The other two composed axes now hold to it as well, on the same discipline — length-prefix every value, mark every optional one:

> - `glossary_hash` frames each entry's source term, target term and note. It used to terminate them with `0` bytes, while `profile::normalize_glossary` deliberately *keeps* control characters (it warns and moves on), so a NUL inside a value could impersonate a field boundary — `{source: "a", target: "b", note: "c\0d"}` and `{source: "a", target: "b\0c", note: "d"}` hashed the same buffer.

> Both were latent rather than live: every glossary pair that collided was separated by `profile_prompt_hash` — the rendered body escapes control characters (R0001-0015), so it tells the two apart — and the bundled adapter's own fingerprint parts are 0x1F-free by construction (a parsed URL percent-encodes C0 controls; the api and effort labels are closed sets). An axis that is only correct because *another* axis happens to be watching is not an axis, which is why both were fixed rather than documented as safe.

### Batch-scoped prompt axes: the move out of run scope and its migration (DCR-0027, 2026-08-09)

Source: `docs/architecture/contracts.md` §5a, paragraph **Which axes are run-scoped and which are batch-scoped (DCR-0027).** The scope statement was restated in the present tense, and the migration note was cut to its standing consequence: a profile with no section-scoped entry has one cohort.

> `provider_fingerprint`, `validation_schema_version`, `profile_version`, the two language labels and `model_id` are computed once per run; `source_hash`, `block_kind`, `input_mode` and `context_hash` are the unit's; and **three** are the *batch's* — `instruction_hash` (below) plus `profile_prompt_hash` and `glossary_hash`, which moved out of run scope when section-coherent packing gave each batch its sections' own compiled prompt and effective glossary. The rule forced the move rather than permitting it: per-section filtering makes the prompt bytes vary within one run, and the rule keys on the bytes the model saw *for this unit*.

> Migration is zero by construction: a profile with no section-scoped entry has one cohort and produces the digests the run-level computation produced, and a profile *with* one could not previously load at all.

### The instruction axis: the collision before `instruction_hash` (R0001-0005, ti c02f69, 2026-08-07)

Source: `docs/architecture/contracts.md` §5a, paragraph *The **instruction** axis is what the rule adds.* The sentence was removed; the paragraph keeps the rule that co-batching changes every unit's prompt.

> Before `instruction_hash`, one paragraph translated beside an HTML block and the same paragraph translated among plain prose shared a cache entry.

### The mode axis: its v0.4.0 introduction and the collision it closed (ti 5f7942, 2026-08-09)

Source: `docs/architecture/contracts.md` §5a, paragraph **The mode axis, and the one kind whose mode its `block_kind` does not imply (ti 5f7942).** The bold lead lost its version tag, and the history clauses were restated in the present tense. The welds (`Pinned by …`) are unchanged.

> **The mode axis, and the one kind whose mode its `block_kind` does not imply (ti 5f7942, v0.4.0).**

> It was absent until v0.4.0 on an assumption that stopped holding: for every block kind but one, the kind determines the mode, so `block_kind` covered it transitively.

> The collision is therefore reachable from ordinary source Markdown rather than adversarial input (ADR-0020's threat model is not in question here — nothing is being hashed harder; an axis was missing), it already bit `InMemoryCache` **within one run**, and `DiskCache` would have carried it between runs.

### Document-level metadata: the batch-scoped prompt axes and the rejected cohort-digest fold (DCR-0028, DCR-0027, 2026-08-09)

Source: `docs/architecture/contracts.md` §5a, paragraph **Document-level metadata: a second, deliberately narrower identity (DCR-0028; OI-0017 item 4).** The scoping sentence was restated in the present tense, and the rejected alternative was removed; the head still says the record carries **no** prompt-identity axis.

> Since the two prompt axes became batch-scoped (above), a run has no single prompt identity a document-level key could honestly name — first batch's? a fold over every cohort? — so any choice would be an invention.

> Folding all cohort digests in was considered and rejected: unit entries already miss when the profile changes, so a full-hit replay under a changed profile cannot occur within one cache generation, and the one case it would distinguish (alternating profiles across runs re-labeling an advisory field with a detection the same provider and model made on the same document bytes) is not a defect worth an invented axis.

## §5b — Run cancellation (DCR-0024)

### Cancellation off by default: the pre-v0.4.0 behaviour clause (DCR-0024, 2026-08-08)

Source: `docs/architecture/contracts.md` §5b, opening paragraph. The clause "and the run behaves as it did before v0.4.0" was dropped; "nothing below is observable" is unchanged.

> `TranslateOptions.cancel: Option<CancellationToken>` — `None` by default, in which case nothing below is observable and the run behaves as it did before v0.4.0.

### Why the token is a trait parameter: the `TranslationBatch`-field and deadline alternatives (DCR-0024, 2026-08-08)

Source: `docs/architecture/contracts.md` §5b, paragraph **Why the trait parameter, when a defaulted method would have been additive.** The paragraph keeps the first alternative (a defaulted `translate_batch_cancellable`), which `manual/reference/developer/en/translator-trait.md` cites §5b for. The two sentences below moved here, and a pointer to DCR-0024's *Alternatives Considered* replaced them.

> Putting the token on `TranslationBatch` instead was rejected for a different reason: that type is wire-shaped data with public fields, so the field addition breaks every struct literal anyway (the same cost as a parameter) while muddling control state into a data type. A deadline (`Instant` / `Duration`) instead of a token was rejected as strictly weaker — it cannot express "the reader closed the tab", the filing consumer's primary case, whereas a deadline is derivable from a token.

## §6 — CLI argument contract

Text that left `contracts.md` §6 in the wave-4 compaction (DCR-0054), quoted verbatim as it stood on `origin/master` at `755d7ba`. Each entry names the §6 paragraph it came from; that paragraph in `contracts.md` holds the current rule, and nothing here is current truth.

### Language labels: the ADR-0013 amendment date (ti fd5aa8, ADR-0013, 2026-08-07; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, the paragraph after **Language labels at the CLI boundary (R0001-0021, ti fd5aa8).** The head now cites `(ADR-0013)`, which states the sentinel rule; only the amendment date left the head. The sentence as it stood:

> The label stays opaque otherwise (ADR-0013, amended 2026-08-07): nothing in a *non-sentinel* label's interior is touched, its case is not folded, and no tag is parsed, so `"Korean (formal, 존댓말)"` survives intact and `--source-language DE` reaches the prompt, the key and the map as `DE`.

### Bundle Content-Security-Policy: the posture-amendment and `form-action` dates (OI-0018, 2026-08-07; R0011-0033, 2026-09-06; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, paragraph **Bundle Content-Security-Policy — opt-in**. The bold lead lost its amendment date and the `form-action 'none'` sentence lost the date the directive was added; both ids stay in the head and the policy is unchanged. The two sentences as they stood:

> **Bundle Content-Security-Policy — opt-in (`--strict-csp`; OI-0018, posture amended 2026-08-07).** By default the emitted bundle declares **no** CSP: source documents are usually already-local copies whose legitimately-remote images should keep rendering, and that accepted tradeoff is unchanged — a run without the flag writes a bundle byte-identical to what pre-flag transync wrote.

> `form-action 'none'` was added on 2026-09-06 (R0011-0033) and is the one directive here that guards a *navigation* rather than a load: `default-src` does not cover it, the bundle has no form of its own, and a `<form>` the sanitizer kept from untrusted source content would otherwise be a live submit target that carries a reader's typed input off-origin.

### Publication-lock depth: the settlement date and the rejected claim shapes (ti cbbc4e, DCR-0021, 2026-08-10; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, **Publication locking (R0001-0034; DCR-0021).**, bullet *Depth, and what bounds it*. The bullet keeps the rule that no pair of claims serializes every depth and none is added, the two orderings that bound the gap, and the accepted residuals; it now points here for why the alternative claim shapes were rejected. The opening sentence and the rejected-alternatives sentence as they stood:

> Depth, and what bounds it (ti `cbbc4e`, settled 2026-08-10).

> Claiming upward from the destination has no principled root short of `/` (and would serialize unrelated runs, and write markers into directories nobody asked transync to touch); claiming downward from the target is unbounded under `--force`, litters a foreign tree the guard may then refuse, and is a TOCTOU besides; and "no lock file beneath the target" is not "no publisher beneath the target".

### `--out-dir` replaceability: the sparse-subset incident (OI-0036, 2026-08-08; ti 66339b, 2026-08-10; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, **`--out-dir` semantics (staged directory publish; EXT-2026-07 P1-6).**, the "did transync publish this directory?" question. The head now states the rule in the present tense — a directory whose only entry is `out.md` does not qualify — and keeps the OI-0036 citation. The sentence as it stood:

> Holding *some* allow-listed name is not an answer: a user directory whose only entry happened to be called `out.md` used to qualify, and was moved aside and its backup recursively removed with no `--force` and no prompt (OI-0036).

### Diagnostics channel: profile load warnings once re-printed by the CLI (R0001-0032, 2026-07-15; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, **Diagnostics channel and verbosity (R0001-0032).**, the paragraph "Where a diagnostic exists on both channels, only one prints it." The head now says the CLI does not re-print `load_warnings`. The sentence as it stood:

> Profile load warnings are the `tracing` channel's (the CLI no longer re-prints `load_warnings`), and the per-unit output-ceiling sentences are too — the CLI adds a single line naming the count and the flags that move the ceiling, whatever the number of flagged units.

### The failed auto-glossary notice: keeping both channels, the rejected third option (ticket 33e178, R0001-0032, 2026-08-06; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, **The one diagnostic that is the CLI's instead (ticket `33e178`).** The paragraph keeps the pick-one-channel rule and why the channel is the CLI's (OI-0026); the rejected option of printing on both channels left the head. The sentence as it stood:

> Keeping both was the third option and it printed the same sentence twice on one stderr for every default-verbosity run, the duplication R0001-0032 removed everywhere else.

### Inline nesting: the ticket date and the delimiter-total measurement (ticket f69e83, 2026-08-07; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, **Inline nesting has no such ceiling, on purpose (ticket `f69e83`).** The bold lead lost its date. The measurement that makes a delimiter-total score unusable as a bound moved here; the head states its result and points here. The same figures are in `crates/transync-syntax/src/intake/markdown/depth.rs`'s module docs. The two sentences as they stood:

> **Inline nesting has no such ceiling, on purpose (ticket `f69e83`, 2026-08-07).**

> Scoring totals *is* a bound (depth ≤ delimiters / 2) but an unusable one: measured 2026-08-07 over the 123 Markdown files tracked here (`.ko.md` excluded), the deepest inline AST any of them parses to is **3** while the densest carries **3,860** delimiter characters.

### CLI usage-block drift check and `transync serve` scope: two asides (ti 57be8c, 2026-08-07; `755d7ba`)

Source: `docs/architecture/contracts.md` §6, paragraphs **The flag list and the defaults are welded to clap (ti 57be8c, ti fc0b18).** and *Out of scope by decision, not oversight*. The first lost its parenthetical about the guide's drift and the second lost the word "now"; both rules are unchanged. The two lines as they stood:

> shipped flag either block leaves out (the guide had drifted ten behind), a

> Out of scope by decision, not oversight: TLS, HTTP ranges, conditional requests and caching sophistication, and any non-loopback deployment hardening. Serving a bundle from another static server still works — the bundle is self-contained — but `transync serve` is what the smoke checklists and `scripts/test-browser.sh` now use.

## §7 — `transync-openai` constructor

Text that left `contracts.md` §7 in the wave-4 compaction (DCR-0054), quoted verbatim as it stands on `origin/master` `755d7ba`, original line breaks kept. Each entry names the paragraph it left; that paragraph in `contracts.md` states the current rule, and nothing here is current truth. A date written `≤2026-08-17` means the text was already present at this repository's root commit `59ce8df` (2026-08-17), so its own date is not recoverable from this history; any other date is the creation date of the ticket or review round named beside it.

### Padded model identifiers: the rejection lands (R0003-0009, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **The model axis has the same two constructors (`R0002-0037`).** The sentence was removed; the paragraph now states the rule it reported as a present-tense clause — a padded identifier is `ConfigError::PaddedModel` (`R0003-0009`).

**That rejection is now implemented** (`R0003-0009`): a
padded identifier used to be stored as given, so it rode out to a remote 400
and fingerprinted as a cache namespace of its own, while the type's own
contract said it should have been refused.

### Per-instance request budget: why `with_timeout` was added (ticket 7065fdb6, 2026-08-07, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **Per-instance request budget (ticket `7065fdb6`).** The whole second paragraph under that lead was removed; the first paragraph states the current rule.

It was added because the budget was the one axis a host could not express: a
library consumer with its own timeout knob had no way to honor it, so a
configured 600s waited on a request this crate killed at 120s, and the host's
own watchdog documented a premise ("the provider's timeout fires first") that
was false for every value above the default. Additive — no existing caller
changes behavior, and no breaking window was needed.

### Two HTTP timeouts: the single-budget past, and "Neither value is configurable" (R0002-0042, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **Two HTTP timeouts, not one (`R0002-0042`).** The paragraph was rewritten in the present tense. Its last sentence predates `with_timeout` (ticket `7065fdb6`, the same section), which made the request budget a per-instance setting; the head now says the connect timeout is fixed and the 120 s request timeout is the default `with_timeout` overrides. The whole original paragraph follows.

**Two HTTP timeouts, not one (`R0002-0042`).** The shared `reqwest::Client`
every adapter instance holds carries a **10 s connect timeout** (DNS + TCP +
TLS) inside a **120 s total request timeout**. Only the second existed before,
so a connect that was never going to complete — a firewall dropping SYNs, a
stale DNS record — spent the whole request budget before a byte was sent, and
spent it again on each bounded provider retry. Nothing was unbounded either
way; the inner budget only makes an unreachable endpoint fail in seconds
instead of minutes. Neither value is configurable.

### Responses `status`: the abort message, with its "now" (R0001-0030, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **Responses `status` (R0001-0030).** Only the word "now" was removed in the head.

A transient status is bounded by
`max_per_batch_provider_retries` like any other transport failure; when that
budget is spent the run aborts, now naming the provider's status instead of a
generic extraction failure.

### Chat `finish_reason: "length"`: how it was diagnosed before (ticket 3c9741bf, 2026-08-06, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, the `length` bullet under **Chat `finish_reason` (tickets `3c9741bf`, `0583a75a`).** The sentence was removed; the bullet states the current classification.

Before this the field was not
  read at all and the truncated body reached `parse_batch_output`, which called
  it `MalformedResponse` — loud, but blaming the provider for what was an
  over-tight `[batching].target_output_tokens`.

### Chat `finish_reason: "content_filter"`: how it was diagnosed before (ticket 0583a75a, 2026-08-06, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, the `content_filter` bullet under **Chat `finish_reason` (tickets `3c9741bf`, `0583a75a`).** The head keeps "Such a reply carries a `message.content` that is absent, null, or cut off where the filter fired. It is reported as a content policy stop (`ContentFiltered`), …" — the clause from "so before ticket" to "policy stop" and the word "now" were removed.

Such a
  reply carries a `message.content` that is absent, null, or cut off where the
  filter fired, so before ticket `0583a75a` it landed either on the
  missing-body malformed-envelope error or on `parse_batch_output`'s
  `MalformedResponse` — both blaming the shape of the response for what the
  provider had already labelled a policy stop. It is now reported as a content
  policy stop (`ContentFiltered`), in wording that denies the transport and
  schema readings, and carries **no** remediation clause: no request parameter
  this adapter sends moves it.

### Chat refusal over both shapes, with its "now" (R0002-0041, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **A refusal is read wherever it arrives, and it is capped (`R0002-0041`, `R0002-0039`, `R0003-0006`).** Only the word "now" was removed in the head.

On the Chat surface a refusal outranks
content, and that now holds for both shapes it can take: the `message.refusal`
field, and a `refusal`-typed part inside a content array.

### Responses refusal with no wording: the asymmetric reader (ti 594a7f, 2026-08-08, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **A refusal is read wherever it arrives, and it is capped (`R0002-0041`, `R0002-0039`, `R0003-0006`).** The sentence was removed; the paragraph's preceding sentences state the shared stand-in rule (ti `594a7f`) that replaced the behavior it describes.

The
Responses reader was the asymmetric one: an empty `refusal` segment printed
`model refused to translate: ` and stopped there, and a segment with the field
absent entirely failed the envelope parse, so an envelope that plainly said the
model declined came back as a malformed one — a misclassification, not only a
thin diagnostic.

### Refusal text cap: the uncapped string before it (R0002-0039, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **A refusal is read wherever it arrives, and it is capped (`R0002-0041`, `R0002-0039`, `R0003-0006`).** The head sentence now ends at "(R0008-0034)."

**On both surfaces** the refusal text is
provider-controlled and is therefore capped at the same 512-byte diagnostic
excerpt as an error body and a Responses `error` object (R0008-0034) — before
this it was the one provider-controlled string that could expand to whatever
the 32 MiB response cap allowed, on its way into stderr, the logs, and the
run's report artifacts.

### Responses refusal precedence: the two orders that let text through (R0003-0006, R0003-0019, ≤2026-08-17, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §7, paragraph **The Responses surface applies the same precedence (`R0003-0006`, `R0003-0019`).** The head keeps the first sentence without "now"; the last two sentences quoted here were removed.

Its envelope reader now reads the *whole* envelope before
accepting any of it: the nested `output` items are walked first, and a refusal
found anywhere in them outranks text found anywhere — the top-level
`output_text` shortcut included. Both orders that let text through are closed.
The shortcut used to return before the nested items were looked at at all, and
the concatenated nested text used to be returned before the refusals collected
in the same walk were checked, so a mixed envelope was translated as an
ordinary answer on a surface whose sibling had already ruled the other way.

## §8 — `transync-anthropic` constructor

Text that left `contracts.md` §8 in the wave-4 compaction (DCR-0054), quoted verbatim as it stands on `origin/master` `755d7ba`, under the same conventions as §7 above. §9 (`transync-google`, DCR-0058) gave up no text.

### Empty text blocks: what select-then-filter did (R0004-0018, 2026-08-11, origin/master 755d7ba)

Source: `docs/architecture/contracts.md` §8, paragraph **`stop_reason` is read before content, and that ordering is a rule.** The head sentence now ends at "carries its answer in the second block."

Emptiness is part of *finding* the answer rather than a verdict
on the block already chosen (`R0004-0018`): a reply shaped
`[{text: ""}, {text: "{…}"}]` carries its answer in the second block, and
selecting first and filtering after reported it as missing text — a valid
response turned into a content retry and, on repeat, a fallback to source.
