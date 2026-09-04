# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

**The sanctioned v0.5.0 breaking window is OPEN.** It was opened by wave 2 of
the HTML→HTML feature (ti `490d97`, ADR-0025, DCR-0034), which splits semantic
kind from source spelling in the block IR. **Six** breaking-by-policy changes
ride the window. Wave 2 contributes four, batched, and none of the four is
user-visible: the corpus regenerates, renders and aligns byte-identically with
zero fixture edits. The fifth is not wave 2's — `transync-openai`'s unreachable
`ProviderError::RateLimited` was removed under this window by the review-0009
fix pass (`8d4efda`, DCR-0040 §3) — and neither is the sixth:
`transync_html::TagToken::Skip` gained `kind` and `terminated` under it
(ti `c1f9a8`, DCR-0047), listed first below. Additive changes land here as they come;
further breaking changes may ride this window until it is closed by the v0.5.0
release.

### Changed (BREAKING)

- **`transync_html::TagToken::Skip` gains `kind: SkipKind` and `terminated: bool`** (ti `c1f9a8`, DCR-0047), and `SkipKind` is a new `pub` enum. `TagToken` is exhaustive by policy, so a downstream pattern or construction naming `Skip { span }` needs `..` or the two new fields. The fields carry what the scanner knew and nothing downstream can recover — notably that `<![CDATA[` is a CDATA section in foreign content and a *bogus comment* in HTML content, which the balancer cannot tell without the scanner's content-mode stack. Having it re-derive them from the bytes would have been a second opinion about where a comment ends, which is the defect ti `415cdb`, ti `e20490` and ti `2e2453` each were.
- **`BlockKind::Html { block_type: u8 }` narrows to the unit variant `BlockKind::Html`** (ti `490d97` wave 2, ADR-0025 / DCR-0034). The variant now means one thing — "HTML content with no semantic equivalent" — instead of two. The CommonMark block type moved to the new `Spelling::Html`, where it was always spelling metadata. A pattern or construction naming `block_type` no longer compiles; a bare `BlockKind::Html { .. }` pattern still does, and is now misleading.
- **`BlockKind` gains a `Title` variant** for an HTML document's `<title>`: `id_code` `"title"`, `wire_str` `"title"`, `heading_level()` `None`, and an **explicit** `sync_role` of `non-sync` — a title is translated and aligned but has no DOM anchor, because the browser chrome renders it and the pane has nothing to anchor. Every exhaustive `match` over `BlockKind` in a consumer's tree needs the new arm.
- **`parser::Block` gains `spelling: Spelling`; `parser::Document` gains `format: SourceFormat`.** Engine-tier (§0 tier (c)) types, reached only by a consumer depending on `transync-syntax` directly. Per-block spelling is required rather than stylistic: one Markdown document interleaves HTML-spelled islands with Markdown-spelled blocks, so no document-level bit can carry the axis.

### Changed

- **`TranslatorError` gains `NoProviderAvailable(String)`**, stable code `provider_unavailable`, exiting **6** (ti `30a744`). Additive on a `#[non_exhaustive]` enum, so a consumer's `match` is unaffected. It is its own variant rather than a message inside `Unsupported` because that variant's exit code (5) deliberately means "the CLI cannot tell a configuration fault from a document one", and an offline run that missed is unambiguously the caller's configuration with the caller's remediation. The `stable_code()` vocabulary is append-only, so this is a new code and not a reinterpretation of `provider_unsupported`.
- **The HTML→HTML feature's closure paperwork** (ti `490d97` wave 7, DCR-0039): the browser-suite weld pins four spec files (`SPEC_FILES`, the mention list and the panic message's advice moving in one commit with all three documents that must name them — ticket `729ec8`'s rule); `scenario-matrix.md` gains SCN-16's row, the `title` and HTML-document block-kind rows, and one out-of-scope bullet recording what ADR-0025 leaves out; the **serve fidelity door** is documented (`transync serve` over an HTML run's out-dir is how you see the real page — the panes are a sync surface, never a fidelity preview); and `README.md` / `docs/Quick_Start.md` stop describing a Markdown-only library. **All eight waves have landed — the feature is complete.** Cutting v0.5.0 remains a separate owner decision via `docs/project/release-checklist.md`.
- **The HTML mechanics moved to their own workspace member, `transync-html`** (ti `490d97` wave 0, DCR-0032). `transync-syntax::htmlseg` is gone with no re-export alias; `transync-syntax` and `transync-core` depend on the new crate, and `transync-syntax` no longer depends on `lol_html` or `htmlize`. Nothing on the `transync` facade's surface moves — the mechanics were always tier (c) engine internals. That wave took the publication roster to seven members; `transync-lang` later made it eight — see the `transync-lang` entry below.
- `transync_html::splice` takes a `BlankLinePolicy` instead of a CommonMark block-type `u8`; `BlankLinePolicy::from_commonmark_html_block_type` is the one surviving home of the `6 | 7` rule.
- Every html dispatch site is re-keyed onto the axis it meant — `Spelling::Html` for IR questions, `InputMode::HtmlSegments` for unit questions, `constraints.html` for the layer-3 splice. Behaviour-preserving today, by three-way set identity; the point is what it will mean once an HTML document's `<p>` is a `Paragraph` that ships a segment array.
- The GFM row-window table splitter now excludes html-segments units **explicitly**, with a test. `inspect_table` would have refused one anyway; an accident is not a guard.
- Workspace version `0.5.0-dev`.
- The HTML-document refusal message now points at both escapes — `--input-format html` to translate it as the HTML document it is, or `--allow-html-input` to translate it as Markdown anyway — and no longer calls HTML→HTML translation unimplemented (ti `490d97` wave 6). `--allow-html-input`'s documentation narrows to the island case in the same commit; its behaviour is bit-identical. `--out-dir` now allow-lists `out.html`, and its marker-less complete-set recovery is a predicate rather than a name count, because a run writes one translated document and never both.

### Added

- **The SCN-16 browser gate — `web/tests/scn16.spec.js`** (ti `490d97` wave 7, DCR-0039). The HTML-run bundle that `scripts/test-browser.sh` publishes is now driven through the **shipped shell** — fetch, DOMPurify fail-closed mount, `sync.js` — the way an operator meets an HTML run's `--html-out`, and it holds the feature's acceptance line: bidirectional sync by block id over HTML-derived anchors, the `<title>` translated and aligned but **absent from both panes** (present exactly once, on the browser tab, per the bundle-title chain), and a console clean of the engine's warnings. Every negative rides a positive: each pane's anchor list must equal the map's anchor-role rows *in order*, so a leaked title, a rendered thematic break, a dropped block or an ungrouped list item all surface as list drift instead of passing vacuously. Falsified before it shipped, against mutated copies of the real bundle — a leaked title anchor, a dissolved `li` group, an anchor-role title row, and the pre-ruling self-injected custom-element anchor each drive their named assertion red. That last one is the first demonstration **through the real sanitizer** of the hazard wave 6's wrapper ruling closed: every earlier browser proof deliberately bypassed DOMPurify.
- **`transync translate --offline`** — run with no provider credentials, serving every unit from the cache (ti `30a744`, OI-0038, DCR-0046). A fully warm run completes with no key at all; one that misses stops **at the miss** with `no provider available for this run` and exit 6, because the configuration is what to change. Requires `--cache-dir`, refused at argument time: without a cache that outlives the process the first unit misses by construction, so the flag would have exactly one reachable outcome. This makes DCR-0028's central promise — a document is paid for once — reachable in the case that motivated it, re-rendering on a machine with no key or no network. Ordinary runs are untouched: the provider is still built up front and a missing key is still a startup error, because deferring that unconditionally would move every credential diagnostic later to serve an occasional run.
- **`TransyncOpenAI::offline(model, base_url)`** — a credential-free instance for the above. Every other field resolves exactly as `try_new` resolves it, so `fingerprint()` returns the **same bytes** as a credentialed instance built from the same configuration. That equality is load-bearing rather than incidental: `CacheKey` carries `provider_fingerprint` as a namespace axis, so an offline run that fingerprinted differently would miss every entry it was pointed at and present as a corrupt cache rather than a misconfigured run. Pinned by `an_offline_instance_fingerprints_identically_to_a_credentialed_one`.
- **A ninth workspace member, `transync-lang`** — the source-language *gate*: should a translation run start at all? A consumer that captures text (an agent's final answer) can skip the provider call when it is already in the target language, without running the pipeline to find out (ti `e4f4b0`, ADR-0028 / DCR-0045). Three public items: `Language`, `Gate`, and a two-variant `Verdict { AlreadyTarget(Basis), Translate(Reason) }`. The verdict shape is the safety mechanism — "I could not tell" is `Reason::Undecided` *inside* `Translate`, so the only way to skip is to name `AlreadyTarget` explicitly, and the expensive mistake (skipping a translation that was needed) cannot be reached by a careless match arm. It depends on **no workspace member** and carries no serialization, which is what keeps it from becoming the alignment map's provider-authored `detected_source_language` — two answers to one question is the defect class the 2026-09-01 fixes above were each an instance of. Backed by `whichlang`, chosen by measurement over `lingua` and `whatlang` in `benchmark/lang-detect/`, whose second round reversed its own first recommendation; a dependency-free codepoint test ties the winner on real fixtures, so the script test runs first and the library only sees what counting could not settle. **The publication roster is eight members.**
- `transync_html::element_extents` / `ElementExtent` — the balancer's stack walk lifted out from under it, for the HTML intake to come.
- `TagToken::Open` carries the byte span the scanner already computed; `TagToken::Skip { span }` names comments, CDATA sections and bogus comments, including the `<!doctype …>` region that previously produced no token at all.
- `transync_html::strip_reserved_sync_attrs` — removes the six reserved sync-attribute names from element open tags (OI-0035 route (c), render half; no call site yet).
- HTML document intake (`transync-syntax::intake::html`, ti `490d97` wave 3, spec §4): five-class element classification with default STOP, rule T anonymous text runs, whole-element byte ranges, emission-order ids, heading section scopes, `<title>` blocks, force-close warnings, and debug-asserted source-order/non-overlap invariants. `regen::regenerate(parse, &empty)` is byte-identical over the SCN-16 fixture and a real-page corpus. No LLM run, no pipeline change, no core change — the layer-6 HTML twin (wave 4) remains the gate before any HTML translation run. (DCR-0035)
- The HTML layer-6 twin (`transync-core::validate::full_rescan_html`, ti `490d97` wave 4, spec §7, DCR-0036): the post-regeneration gate for HTML documents — document-wide ordered tag ledger, fresh segmentation re-run through the intake (per-`<li>`, D9), gap byte-identity including preamble and tail (closing the ledger's measured `<!DOCTYPE>`/comment blind spots), and boundary sanity. `finalize` dispatches on `Document.format`; `ReparseFailure` is unchanged, so the three-stage fallback cascade is untouched. No HTML translation run is reachable yet — the entry point is wave 5, the CLI flag wave 6.
- **HTML→HTML translation works at the library surface** (ti `490d97` wave 5, spec §6/§12, DCR-0037): `TranslateOptions.input_format: SourceFormat` (additive; `transync::SourceFormat` exported with its §0 row) routes a declared-HTML source through the HTML intake, the same pipeline, and the scanner-side layer-6 gate; `translate()` returns translated HTML with a real alignment map. `[system].prompt_html` is a new additive profile key (shipped in the default profile; a profile without it falls back to `[system].prompt` with an advisory, never a synthesized merge); the user-message instruction gains a run-level HTML-document clause; Html-spelled heading context and the `<title>`-preferred document title project through `transync_html::extract`. No new cache axis — `profile_prompt_hash` and `instruction_hash` both move for an HTML run (contracts §5a records the verdict) — and no `VALIDATION_SCHEMA_VERSION` bump: the new checks are validator tightening, the bump rules' explicit "Do NOT bump" case. Markdown runs are byte-identical throughout: prompts, instructions, goldens, corpus. The `html_dominance_warning` tail now names the entry point instead of calling the feature absent, and declared-HTML runs never evaluate it. Panes for HTML runs are empty until wave 6 (the pane derivation, the CLI flag, `out.html` and alignment schema 1.3.0 are wave 6's).
- **The HTML→HTML operator surface** (ti `490d97` wave 6, spec §8/§9, DCR-0038): `transync translate --input-format html --input page.html --out-dir out/` writes `out.html` — the full page, head, scripts and doctype verbatim, **anchor-free** — plus a bundle whose panes mount and sync by block id. Routing is flag-only: the preamble sniff stays a refusal and is re-tripped even by an explicit `--input-format markdown`, there is no reverse sniff on the html arm, and `--input-format html` together with `--allow-html-input` is an argument error (exit 1). `--allow-html-input` is unchanged in behaviour and narrowed in documentation to its real case: Markdown that genuinely opens with an HTML island. An HTML run's two annotated panes are now the synthesized `<main>` fragments — strip-then-inject into each block's own element, transparent `<div>` for `html`-kind and element-less blocks, `li` blocks grouped, and head/gap/script bytes never present.
- **Alignment schema 1.3.0** (ti `490d97` wave 6, DCR-0038): three additive changes — per-row `source_format` (the block's spelling; a Markdown run's html-island row says `"html"`), map-level `input_format` (the run's intake), and the `"title"` `block_kind` value. Forward-minor by design: no new `sync_role` value, no new required row fact, both additions outside the facts the minimum-usable-row gate polices, and `"title"` rides the `non-sync` role shipped since 1.0 — so a 1.2.0-era engine reads a 1.3.0 map and still drives, which the browser suite proves rather than asserts. The sync engine's logic is untouched; the whole `sync.js` diff is the schema mirror.

### Fixed

- **A provider can no longer erase a block's visible text and have it ship as `translated`** (ti `c887bc`, DCR-0048). Every per-unit validation layer compared **structure** — heading level, `(cols, rows, alignments)`, fence info string, node-kind fingerprints, segment count, kind labels, destinations, tag ledgers, gap bytes — and none asked whether the words survived, so a payload that kept each kind's structural fact and none of its text was accepted with `fallback_status: translated` and no warning: `"##"` for a heading, blank cells for a table, a bare `rust` fence for a code block, `"- \u{200B}"` for a list item, `"&nbsp;"` or `"\u{2800}"` or `" \u{FEFF}"` for a paragraph. **All six Markdown block kinds and both HTML paths.** The one arm that did look at text — `per_kind::check_html`'s whitespace guard — was wrong twice: its predicate was `char::is_whitespace`, exactly the 25 Unicode `White_Space` codepoints, so U+200B / U+200C / U+200D / U+2060 / U+00AD / U+FEFF all passed; and it justified being unconditional with "source segments carry at least one non-whitespace char by construction — the extraction drop step", which is false, because `transync_html::extract`'s drop test is the *same* predicate, so a zero-width-only source segment is kept, sent, and had its faithful echo rejected. `validate::text_presence` now owns the question for both intakes: reject when the **source payload carried visible text and the translation carries none**. The scope is what makes it safe to be aggressive about which codepoints render nothing — a `<td>&#8203;</td>` shim, a ZWJ-only emoji fragment, an empty-bodied fence and a thematic break each have no visible source text either, so the check never fires on them. The two intakes read different things: the segment **strings** on the HTML path, because `splice` writes a translated segment as `ContentType::Text` and escapes `&`, so a provider's `"&nbsp;"` reaches the page as six visible characters; and comrak's **decoded text** on every Markdown path, because nothing re-escapes there and `&nbsp;` / `&#8203;` / `&#xFEFF;` are pure ASCII in the payload bytes. Rejection is retryable like every other per-unit layer and reported under the existing `per_kind_shape` layer, so no wire shape moves. Visible garbage is deliberately out of scope — `**` renders two asterisks, and grading a translation is the model's job (architectural invariant 2).
- **A raw-HTML block that ends inside an unterminated comment, CDATA section, bogus comment or tag no longer takes the next block's anchor with it** (ti `c1f9a8`, DCR-0047). `balance_fragment` now terminates such a region where a browser terminates it, or deletes it where a browser abandons it — a tag cut off at EOF mints no element, and no single terminator even works for one cut inside a quoted value. Only an unterminated *comment* swallows everything after it; the other three kinds end at the **first `>`**, which in a mounted pane is the sync wrapper's own `</div>` — so the wrapper closed inside the region, stayed open, and every following block nested inside this one's wrapper, which is `contracts.md` §4a's direct-child break. Reachable from ordinary source Markdown: a type-6 html block ends at a blank line, so `<div>x<!--` plus a blank line plus a paragraph is one html block and a real anchor. ti `95f55b` had stopped the balancer appending a closer the region would swallow, which bought idempotence and left the fragment unrepaired; that carve-out is now closed and its re-scan guard is kept as an invariant check.
- `scan_tags` tokenizes malformed attribute regions the way a browser does (ti `549b20`; DCR-0032 amendment 2026-08-21): a bare quote in attribute-name position is a name byte instead of opening a phantom quoted value; a stray `=` starts an attribute named `=` instead of opening a value, so the strip neither misses a plant behind one nor deletes text a browser paints; and a tag left unterminated at EOF becomes a `TagToken::Skip` spanning to end of input instead of a silent whole-suffix abort. One stray byte could previously desynchronize the scanner and hide everything after it from `tag_inventory`, `balance_fragment`, `element_extents` and `strip_reserved_sync_attrs` — including a planted `data-sync-id` that DOMPurify keeps as a live anchor.
- `balance_fragment` / `element_extents` read a start tag's self-closing `/` the way a browser does (ti `490d97` wave 1; DCR-0032 amendment 2026-08-23). HTML honours the flag in exactly two places — inside foreign content and on the `<svg>`/`<math>` start tags that enter it — and everywhere else it is a parse error the parser ignores, so a flagged non-void tag now OPENS and the author's matching end tag is its real closer. The walk never pushed such a tag, so that end tag was deleted as an orphan, and on the render path the still-open fragment consumed the sync wrapper's own `</div>` — nesting the next block's anchor inside the html block's wrapper, which contracts.md §4a forbids. Reachable from a plain author-written `<div/>`, with no HTML translation involved. `walk_elements` gained per-stack-entry foreign-content tracking so the two real carve-outs keep the XML reading, and `VOID_ELEMENTS` gained the four measured parser-voids `basefont`, `bgsound`, `frame`, `keygen`.
- `strip_reserved_sync_attrs` no longer welds bytes together at the cut (ti `490d97` wave 1). Removing an attribute took its leading whitespace unconditionally, so `<div data-sync-id="a b="c">x` became `<divc">x` — moving `tag_inventory` from `["div"]` to `["divc"]` — and a reserved attribute sitting behind a stray `/` stripped to `<div/>`, setting a self-closing flag the source never carried. Adjacent cuts are now coalesced into one run and the run is replaced by a single space exactly where its absence would change the parse; a well-formed strip is byte-identical to before.
- **A `data-sync-id` written into source content can no longer pre-claim a real block's anchor** (OI-0035, ti `490d97` wave 1, DCR-0033). Closed at both layers. The renderer strips the six reserved sync-attribute names from raw-HTML blocks on the way into a **pane** — `out.md` keeps the author's bytes — and `mountSync` takes its anchor set from the validated alignment rows, so an element the map does not claim is inert forever, including one inserted after mount. Both `web/js/sync.js` and its byte-identical CLI-embedded twin moved in one commit. Recorded residual: an impostor carrying a *listed* id ahead of the genuine anchor still wins the engine's first-occurrence lookup, which is why the render-side strip is not optional.
- **An `<svg>`-wrapped `<div>` no longer swallows the following block's anchor** (ti `e77173`, DCR-0041). `walk_elements` carried one inherited `in_foreign` bool per stack entry, which models foreign content as a region entered once and left once. HTML's is not one: it has **islands** where the parser is back in HTML content while still inside an `<svg>` (`foreignObject`, `desc`, MathML's text integration points, and `annotation-xml` at exactly two `encoding` values), and **exits** that no end tag marks — a breakout start tag pops open foreign elements at the tag itself. So `<div class="wrap"><svg><div>x</svg></div>` left that inner `div` open in a browser while the walk closed it at `</svg>`, and the next block's anchor mounted inside the wrapper. That is a live `contracts.md` §4a break, reachable from untrusted source Markdown through a type-6 html block with no self-closing slash involved. The walk now carries a three-valued content mode per open element.
- **Raw text and RCDATA are HTML-content states, and a sync attribute planted in an SVG `<title>` no longer survives the strip** (ti `2e2453`, DCR-0042). `scan_tags` entered raw-text/RCDATA for `script`, `style`, `textarea` and `title` unconditionally, inside `<svg>`/`<math>` too. A browser does not — those states are entered by the tree construction stage from the "in body" insertion mode, and foreign content has no such rule — so there all four are ordinary foreign elements whose contents are markup and whose self-closing `/` is honoured. Measured: `<svg><title>a<b>c</b></title>` mints a real `<b>`; `<svg><script/>x` leaves `x` a sibling. The consequence that matters is the strip's: in `<svg><title><div data-sync-id="p-0001">` a browser sees a **live** attribute on a real `div`, while the scanner saw RCDATA text and `strip_reserved_sync_attrs` walked past it, leaving a planted anchor standing in a pane whose whole premise is that ours are the only sync attributes in it. The content-mode stack moved out of `walk_elements` and into `scan_tags`, which needs the same answer to decide whether a raw-text run opens at all, so the two layers read one verdict instead of deriving two. SVG `<title>` is now the HTML integration point the spec makes it.
- **A void name used as a real foreign element keeps its end tag** (ti `48f3c6`, DCR-0043). `is_void` decided whether a start tag opens an element globally, foreign content included. HTML's void list is an HTML-content rule: in foreign content "any other start tag" inserts a foreign element and only the self-closing flag pops it. So `<svg><link>a</link>` is a genuine closable SVG element whose `</link>` was classified an orphan and **deleted** from the pane — the balancer editing what the reader sees. Thirteen void names are reachable that way. The `</br>` hazard that kept the rule global — HTML's end-tag-`br` rule turns an appended one back into a fresh `<br>` — cannot arise: `br` is a breakout tag, so `<svg><br>` leaves foreign content before the tag is processed, as do `embed`, `hr`, `img` and `meta`. That set is pinned by test.
- **The scanner's tag-name and end-tag-open states follow HTML, closing a second strip bypass** (ti `e20490`). HTML's tag-name state consumes every byte up to whitespace, `/` or `>` into the element name — quotes and `=` included — so `<divq"x=" data-sync-id="v">` is one element named `divq"x="` carrying a **real** `data-sync-id`. The scanner ended the name at `divq` and attribute-walked the remainder, burying that plant in a phantom quoted value the strip never examines as a name, so a live anchor survived; only the pane path's DOMPurify mount dropped it, for a reason a consumer that does not sanitize never inherits. Separately, `</` before a non-letter opens a bogus comment in HTML and mints nothing, where the scanner emitted an `Open` and the balancer then owed it a closer a browser reads as orphan junk. One `tag_name_end` now serves both the scanner and the strip, because fixing the tokenizer alone left the hole open — the strip held its own copy of the boundary rule.
- **`strip_reserved_sync_attrs` stops cutting a non-reserved attribute across a stray `=`** (ti `415cdb`). `strip_reserved_sync_attrs("<div =data-sync-id=\"x\">y")` returned `<div =>y`, where a browser folds those bytes into one junk attribute named `=data-sync-id` — nothing reserved is present, yet the cut ran and a non-reserved attribute moved. The direction was always safe (a 51-case browser equivalence probe found 50/51 equal, that one over-deletion, and **zero** under-deletions), so the impostor-anchor property was never at risk. The cause was two attribute walks where there should be one; DCR-0041 had collapsed them onto the shared `walk_attrs`, and this aligns that walk with HTML's before-attribute-name state.
- **`balance_fragment` is idempotent under its own re-scan** (ti `95f55b`). A fragment can end inside an unterminated comment, CDATA section, bogus comment, raw-text run or tag — an author may close an HTML block mid-`<!--`, and invariant 7 says that input is expected. The closers appended at the end then landed **inside** that region, where they are not markup at all: a re-scan hid them, the elements read unclosed again, and the next balance appended another copy without limit (15,726 violations across 200,000 fuzz iterations). The balancer now asks the scanner rather than classifying the region itself — if the appended bytes come back as close tokens they belong, and if the scanner cannot see them they are dropped. The append stays where it is load-bearing: `</textarea>` ends the raw-text run it would otherwise be buried in.
- **A provider payload meets the same nesting ceiling the source had to satisfy** (ti `148fcf`). Source Markdown reaches comrak through one guarded door (`parser::intake`, which refuses on nesting depth before parsing), but a **translated** payload returned by a provider reached the validation layers without passing it. Untrusted-input handling now applies to both directions of the wire at one door in `validate_unit`, before any layer parses, and `validate::full_reparse` identifies which blocks exceeded the ceiling so a rollback names them rather than rolling back nothing. The originally-feared impact — a stack-overflow abort — was **refuted** by verification: comrak 0.27's block parse is iterative and its walkers are heap-based, and `structure.rs` keeps a deliberate 1000-level tripwire test on a 256 KiB stack that would catch a regression in that dependency. The fix is a consistency and rejection-quality change, not a crash fix.
- **A block's DOM anchor is decided by its own alignment row** (ti `18b9c3` / `9ffb97`, DCR-0044). `render_block` enforced "no anchor" with a literal `BlockKind::ThematicBreak` test and never consulted `align::sync_role_for`, so a block whose row says `sync_role: non-sync` could still render a real `data-sync-id` — the row and the DOM disagreeing about the same block, which every anchor rule in `contracts.md` §4a rests on not happening. Not reachable today (nothing mints the one other non-sync kind, `BlockKind::Title`) and reachable as soon as the HTML intake does. The omission moved into `attrs::write_attrs`, keyed on `row.sync_role`: one value, written by `sync_role_for` and read at emission, cannot disagree with itself. `sync_role_for` also lost its `_ => SyncRole::Anchor` catch-all and now names every `BlockKind` variant, so a new variant is a compile error at the one place that decides its wire role rather than silently anchoring. Thematic-break output is byte-identical.
- `DiskCache` no longer discards its whole contents on every open when `max_bytes` is set below the bytes eviction cannot reclaim (OI-0044 / R0009-0082, ti `0a3fca`). The trim evicts entries; the header and the document-scoped records are exempt by design, so a budget at or below their combined size named a total no set of entries could reach — and the loop freed everything, still read over budget, and repeated it next open. It now logs the arithmetic (requested value, floor, and the floor's two components) and applies `max_entries` alone, so the entries survive and the cache converges. Reachable only by a program embedding the crate with a small budget: the CLI always opens with the 1 GiB default (ti `650bbb`), and this was that lever's named precondition.

## [0.4.0] - 2026-08-20

**This release closes the sanctioned 0.4.0 breaking window.** Three breaking
changes rode it: `TranslationOutput::translated_markdown` became
`translated_document`, `transync translate` began refusing an HTML document
instead of corrupting it, and `BlockKind::CodeBlock` gained a `fenced` field so
an indented code block could be translated at all. Everything else here is
additive. The next breaking change needs a new window.

**v0.4.0 carries no git tag** — owner decision 2026-08-13;
`docs/project/release-checklist.md`, under *When it applies*, records why: this
repository's history was restarted twice, and a release tag on a commit that
does not carry the release's history is worse than no tag. The release-prep
commit is this release's only anchor.

> **Correction (2026-08-20, owner decision — reversed the same day).** The
> paragraph above was true when this entry shipped and is no longer: **`v0.4.0`
> is tagged**, annotated, on the release-prep commit `6fa4e88`. The reversal is
> not a change of mind about the history — that reasoning still holds — but a
> use the earlier decision did not weigh: **consumers need a ref to depend on**,
> and a tag serves that whether or not the commit's ancestry is the development
> that produced it. The tag makes no claim about ancestry; it names which tree
> is v0.4.0. The `[0.4.0]` link below therefore resolves to the tag rather than
> to a bare commit URL, and `[Unreleased]` compares against `v0.4.0`.

**Release gate (OI-0030): `scripts/smoke-live-gate.sh` PASS on 2026-08-20** —
2/2 machine-asserted live round-trips against `gpt-4o-mini` (Chat Completions)
and `gpt-5-mini` (Responses). The gate was triggered by this range: it changes
`unit::payload` (an indented code block now reaches the model as an
engine-synthesized fenced block, so the wire payload itself moved),
`transync-openai`, and the batching surface. Each leg is one full `translate()`
round-trip, so a pass is evidence that the whole validation stack — schema,
ID-set equality, per-kind shape, fragment reparse, inline protection, full
reparse — accepted a genuine provider response. Run against a tree identical to
the release-prep commit in every file but this one: `git diff 6fa4e88..HEAD`
is the two CHANGELOG link lines and nothing else, so the result attests to the
released code.

This ends a two-release run of shipping with the gate triggered and unrun
(v0.3.0 was the first). The **`anthropic` leg remains unrun** — no
`ANTHROPIC_API_KEY` was supplied, and it has never been executed in this
repository; `transync-anthropic` is in the tree but on no CLI run path, so its
only standing evidence is still its offline suite.

Standing gates, all green against the release-prep commit: `./scripts/smoke.sh`
(workspace 1066 passed / 0 failed / 5 ignored; CLI stub 208 passed; rustdoc and
wasm32 gates clean; wasm `raw=1,648,009B gzip=675,725B` inside the
`1,840,000 / 760,000` budget), `./scripts/test-browser.sh` 33 passed,
`cargo fmt --check`, `cargo clippy --all-targets --all-features -D warnings`.
Sibling consumers: `dynwebserver` `cargo check --workspace` clean;
`resp-translator` fails in `copy-transfer-mcp` on the `translated_markdown`
rename — reported as ticket `e502b0`, not patched from this side.

### Docs — the object store is restarted a second time, and the cause is named and excluded (2026-08-17)

`git fsck` reported **33 missing objects** and 33 broken links. Seventeen were
recovered by hash from sibling object stores; the other sixteen — seven blobs,
eight trees and one commit — were not found anywhere on this machine, and the
missing commit severed the chain. Of the **23 commits still reachable**, exactly
**one** had a tree `git ls-tree -r` could walk to completion, so the repository
restarted from the verified working tree at root commit **`59ce8df`**. This is
the second restart (the first was 2026-08-10) and the third loss event in ten
days. **No code changed**: all 342 tracked files came through byte-identical,
and `cargo test --workspace -- --test-threads=4` reported **1066 passed / 0
failed / 5 ignored** with the CLI stub suite at **208 passed / 0 failed**,
measured identically on both sides of the restart.

- **The record is `docs/project/git-history-loss-2026-08-17.md`**, indexed from
  `docs/index.md`. It carries what the 2026-08-10 record could not: the cause
  with physical evidence, and a recovery procedure that has been run.
- **The cause is named and removed.** A cloud file-sync client was writing
  conflict copies inside `.git` — 23 duplicated object *fanout directories*
  (`0a (2)`, `6d (2)`, …), three duplicated object files, and a duplicated ref
  file `refs/heads/master (2)` holding a null sha1; 27 collision paths inside
  `.git` against **0** in the working tree. The directory form is why the
  earlier events looked causeless: its contents carry ordinary 38-hex names, so
  a `* (*` search misses them, and git never reads a fanout directory whose name
  is not two hex characters. `.git` was added to the client's ignore list, and
  **that did not work**: on 2026-08-19 the loss recurred with the line still in
  place and the client still running — 13 objects gone, `git ls-tree -r HEAD`
  aborting at 151 of 342 — and it left **no collision copies at all**, so the
  census this entry once offered as the check read 0 throughout. `git fsck` is
  the check. What repaired it in seconds was redundancy: all 13 objects came
  back, hash-verified, from the salvaged store under `/Volumes/Common/git-backup/`.
  `docs/Troubleshooting.md` and `docs/project/git-history-loss-2026-08-17.md`
  carry the corrected account.
- **The recovery is written down as a runbook**: sweep collision copies inside
  `.git` (directories as well as files, verifying each by hash), sweep the
  working tree, search sibling stores with `git cat-file -e` so packfiles count
  rather than walking `objects/`, re-import with `git cat-file | git hash-object
  -w` asserting the sha comes back unchanged, then re-measure. The record is
  honest that the inside-`.git` sweep yielded **nothing** here and is
  recommended on cost, not on demonstrated yield.
- **Three live instructions were repaired**, not rewritten: `release-checklist.md`
  named the 2026-08-10 restart commit in *When it applies* and in steps 3 and 10,
  and that commit no longer exists. The root commit is now resolved with
  `git rev-list --max-parents=0 HEAD` instead of quoted, and both audit steps say
  what to do now that a `git log <range>` spans **zero** commits — read the
  `[Unreleased]` entries against the tree, and run the live gate when in doubt.
  **The v0.4.0-ships-untagged decision is unchanged**, and its reason is the same
  one, now twice over. *(Correction, same day: the owner reversed it — `v0.4.0`
  is tagged. The reasoning above about history is unaffected; the reversal rests
  on consumers needing a ref. See the `[0.4.0]` entry's correction note.)*
- **Dated records were left as written.** Every commit hash quoted in
  `status.md`, `backlog.md`, the ADRs, the DCRs and this file names a commit that
  is not in this object store; `status.md` and `backlog.md` gained one dated note
  each saying so, rather than having their wave entries edited.

### Fixed — BREAKING: an indented code block is translated instead of mishandled, and comes back fenced (2026-08-16)

Ticket `457e51`. **Every indented (four-space or tab) code block in every
document was mishandled, in one of two ways.** The parser discarded comrak's
`fenced` flag, so `BlockKind::CodeBlock` carried no shape, and
`outcome::block_payload` sliced a range comrak reports *after* the columns of
block structure it consumed — the unit's payload was the block's **dedented**
first line. A four-space block's payload can only reparse as a paragraph, so
`fragment_reparse` rejected it every time with `expected code-block, got
paragraph`, three provider calls bought nothing, and the block settled as
`fallback_source` — expensive, but loud and self-reporting. A block indented
eight columns or more failed the other way: it was **accepted on attempt one**
and reshaped the output silently, which is the eight-space bullet below and the
more dangerous half of this entry. It is breaking because the fix moves the public
`BlockKind::CodeBlock` variant and the shape of `out.md`; both ride the open
v0.4.0 window. The decision the fix takes is recorded in
`docs/project/design-change-records/DCR-0031-indented-code-normalize-on-translate.md`.

- **`BlockKind::CodeBlock` gains `fenced: bool`.** `CodeBlock { info }` becomes
  `CodeBlock { info, fenced }`, recording which spelling the *source* used.
  Migration is one rest pattern at every match site
  (`BlockKind::CodeBlock { info, .. }`) and one field at every construction
  site. The field carries **no** `#[serde(default)]` on purpose: nothing
  shipped serializes the enum form — the alignment map and `CacheKey` both hold
  `wire_str()` strings, and neither `TranslationUnit` nor `parser::Block`
  derives `Serialize` — so there is no compatibility to buy, and a loud failure
  beats a silent wrong guess. `contracts.md` §0 records the surface decision in
  prose and §1 the stability entry; the §0 **table** and
  `crates/transync/tests/public_surface.rs` are untouched, because both pin
  public *paths* and `transync::BlockKind` is still exactly that path.
- **An indented block's range now covers its block-structure indent**, so the
  slice is the block's complete indented spelling rather than a first line
  missing four columns. The snap walks `start` back over the run of spaces and
  tabs ahead of it, bounded by the start of the line — never "start minus four
  bytes", because comrak counts consumed **columns**, which is four bytes for
  four spaces, one for a tab, and four of eight for a doubly-indented block
  whose other four columns are content. It is bounded by *indentation* rather
  than by column 1 because comrak's column is a byte offset that also counts a
  document-leading UTF-8 BOM (it reports `"\u{feff}    code\n"` at `1:8`): a
  blind column-1 snap would pull the BOM into the block's range and an accepted
  translation would delete it from `out.md`, while a BOM ahead of any other
  first block survives in the inter-block gap. `source_hash` is computed after
  the snap, so it covers the full spelling.
- **The unit is re-fenced by the engine before dispatch.** `unit::payload`
  hands `regen::regenerate_code_block` the slice and sends the synthesized
  fenced block, so fence-length safety against a backtick run in the body is
  inherited from the one fence synthesizer rather than reimplemented, and the
  CommonMark dedent is comrak's. The wire contract does not move: the label is
  still `full_code_block`, the payload is still a whole fenced block, and a
  returned info string is still rejected for a source fence that had none
  (R0003-0043).
- **Output shape moves, and that is the point.** A translated or preserved
  indented block is re-emitted **fenced at column 0** — bare fence, no info
  string, safe length. Only a **fallback** keeps the indented spelling, and it
  keeps it byte-verbatim, so invariant 6 is untouched. In the alignment map the
  *values* move for these rows (`source_range` widens by the indent,
  `fallback_status` flips from `fallback_source` to `translated`); the schema
  does not, and `ALIGNMENT_SCHEMA_VERSION`, `VALIDATION_SCHEMA_VERSION` and
  `VALIDATION_REPORT_SCHEMA_VERSION` all stay where they were.
- **The eight-space case was the wider symptom, and it closes here too.** Its
  dedented slice is *itself* a valid indented code block, so the echoed payload
  passed every per-unit layer and was accepted on attempt one — after which
  regeneration emitted a fence beneath four residual columns that sat outside
  the block's range, producing an indented block holding a fence marker plus an
  unclosed fence that swallowed the following paragraph. `full_reparse` caught
  the divergence and the DCR-0004 cascade downgraded both blocks. The range
  snap removes the residual columns; no validator changed.
- **No cache invalidation, and no bump to buy one.** Every indented block's
  `source_hash` moves with its widened range, so an entry written under the old
  identity is unreachable **by the block that wrote it**. It is not orphaned,
  though, and the precise claim matters: a pre-fix eight-space entry is keyed on
  `H("    body")`, and post-fix a **four-space** block whose complete spelling
  is exactly those bytes hashes identically — `context_hash` cannot separate
  them either, because `unit::context` builds neighbor summaries through
  `text_from_range`, which ends in `.trim()`. That hit is *harmless*, traced
  end to end rather than assumed: the cached payload is re-validated against
  the requesting unit, `check_code` compares info strings (`None` on both
  sides), fragment reparse accepts the indented payload as a code block, and
  `regen::regenerate_code_block` dedents it through comrak and re-fences —
  yielding exactly the content the four-space block carries. So a surviving
  entry is reachable only by a block for which it is a correct answer.
  What eviction does *not* cover is a run that never finishes. `cache_put` runs
  per accepted unit at the Accept disposition in `pipeline/dispatch`, round by
  round, *inside* the batch fan-out; all three eviction points —
  `merge_outcome.faulted_parents`, the FailStop `divergent_source_blocks` path,
  and `full_reparse_fallbacks` — live in `run_pipeline` **after** the fan-out
  settles. A run that reaches the end does evict the eight-space class (it is
  exactly `full_reparse_fallbacks`), but a run cancelled at the DCR-0024
  checkpoint, aborted by a terminal provider error in a sibling batch, or
  killed outright leaves a durable `Entry` in a `DiskCache` that no *targeted*
  eviction can ever name again — semantic eviction derives keys from post-fix
  identities, so the stranded entry is not in any set they compute. Only the
  open-time capacity trim can drop it, incidentally, once the log exceeds its
  budget (a 1 GiB default, oldest-written first). The reachability argument
  above is what makes that tolerable, not the eviction.
- **A justification the code no longer exhibits was corrected everywhere it had
  reached** — the ti `d990b6` lesson applied to itself. The HTML-document
  refusal (ti `13e145`) rested partly on "a four-space-indented run is not
  translated at all after three provider calls", which is now false. **Eight
  passages across five files.** Two are **runtime strings** an operator reads:
  the CLI's `--allow-html-input` refusal message and the library's
  html-dominance note, both of which told an operator to expect a
  `fallback_source` row that will not be there. The other six are prose and
  rustdoc: `contracts.md` §6's `--allow-html-input` usage passage, `contracts.md`
  §6's exit-code-`2` bullet, `Developer_Guide.md`'s `translate` flag block,
  `allow_html_input`'s rustdoc in `translate_cmd.rs`, the preamble-sniff rustdoc
  in `translate_cmd/input.rs`, and `html_dominance_warning`'s own rustdoc in
  `transync-core`'s `unit.rs` — that last one sits above the runtime string it
  builds and states the same claim in its own words, so correcting the string
  alone would have left the file self-contradicting. The refusal itself
  **stands**, and its rationale is *stronger*
  — the indented run used to be the one loud, self-reporting harm on that path,
  and it is now a silent reshaping like the other two, so nothing a run emits
  distinguishes that input from a correct one.

### Changed — BREAKING: `transync translate` refuses an HTML document instead of quietly corrupting it (2026-08-13)

Ticket `13e145`. **An `--input` whose preamble declares an HTML document —
`<!doctype …` or `<html …`, read past a BOM, leading blanks and any leading
comments — now exits `2` instead of translating.** `--allow-html-input`
restores the previous behavior byte for byte. This is a refusal where there was
none, so it is a behavior change and it lands in the v0.4.0 window on purpose:
the window is the sanctioned place for one, and shipping the guard in a patch
release would turn a script that had been silently producing broken output into
a script that suddenly fails, with no version boundary to explain it.

- **What it was doing before.** `read_capped` checked size and UTF-8;
  `parser::parse` refused only on nesting depth. Nothing looked at the format,
  so an HTML document went straight into comrak and the run exited `0`. The
  result was worse than garbage because it was deceptively close to correct:
  every blank-line-delimited run opening with `<` became a `BlockKind::Html`
  fragment and translated genuinely, spliced back byte-exact — while every run
  whose next line did *not* open with a tag re-entered as **Markdown**. What
  that costs was measured against the echo stub under ti `d990b6`, and it is
  three things. Prose came back re-read under Markdown inline rules: `*` and
  `_` consumed as emphasis delimiters, `[` opening a link, entities decoded.
  Sections and `document_title` come from Markdown headings, so an `<h1>` left
  both empty — every unit's `section_path` `[]`, the provider given no title at
  all, the HTML bundle falling through to the literal `transync` — and nothing
  said so. And a line indented four spaces became an indented code
  block whose unit payload is the *dedented* body, so whatever came back
  reparsed as a paragraph, the per-kind layer rejected all three attempts, and
  the block settled as `fallback_source`: three provider calls spent on bytes
  that were then not translated at all. Only that last one reports itself, and
  what it reports is invariant 6's **loud** fallback, not corruption —
  regeneration splices the source bytes back verbatim, so the run stays exactly
  as written. The artifact was still named `out.md`, and the exit code was
  still `0`.

  *(The `[Unreleased]` text here previously said the indented line came back
  **fenced**, with literal ``` fences injected into the output. It does not:
  `regen::regenerate_code_block` is never reached, because the payload is
  rejected before regeneration sees it. Corrected under ti `d990b6`, which also
  pins the real behavior in
  `cli_html_input_leaves_an_indented_run_untranslated_and_unfenced`. The
  refusal itself is unchanged — the harms above justify it.)*

  *(**And the third harm above is now history, 2026-08-16.** Ticket `457e51`
  fixed the indented-code defect itself, so an indented run in HTML input is
  translated and written back **fenced** — the shape the original text claimed
  and `d990b6` was right to deny of the code as it then stood. The paragraph
  above is kept as the record of what the refusal was decided against; what it
  describes is no longer what a run does. The test that pinned it is renamed
  and inverted to the new truth:
  `cli_html_input_re_emits_an_indented_run_as_a_fenced_block`. The refusal is
  again unchanged, and its rationale is **stronger**: the indented run was the
  one *loud*, self-reporting harm on this path, and it is now a silent
  reshaping like the other two, so nothing a run emits distinguishes this
  input from a correct one.)*
- **Exit `2`, from the existing taxonomy — no new code.** It joins the other
  admission-control refusals (`--max-input-bytes`, non-UTF-8, the block-nesting
  ceiling) under "input read / parse failure". Deliberately not `1`: the
  arguments were well-formed and the file was readable; what this build cannot
  translate is the *document*. The numbers stay append-only.
- **The message names the missing feature.** HTML→HTML translation is a
  separate, unimplemented feature (ticket `490d97`), and a refusal that does not
  say so leaves the operator guessing whether they mistyped a flag. The
  diagnostic names what it saw, what would have happened, the absent feature,
  and the override.
- **Added (library, additive): an html-dominance note on the reader-honesty
  channel.** `translate` now appends one `skipped_source_nodes` entry when at
  least 80 percent of a document's unit-backed translatable *bytes* are raw-HTML
  blocks (over a 512-byte floor, so a one-`<div>` fixture is not the subject).
  It is a **note, never a refusal**: a README that is mostly HTML islands is
  legitimate input under ADR-0018, and a hard library refusal would break
  existing callers. Mass rather than block count, so a badge wall cannot outvote
  the prose it decorates; blocks that are not units — a zero-segment html block
  — count on neither side. The report field is unchanged in name, shape and
  version; the ordering rule in `contracts.md` §3a now reads parser notes, then
  this document-level note, then the per-block html notes.

### Fixed — a compaction temp left by a crash is now reported, deletable on instruction, and reclaimed where that is provably safe (2026-08-13)

Ticket `d51ed7`. `DiskCache` compaction rewrites the log through
`transync-cache.jsonl.compact-<pid>-<nanos>` plus one `rename`. Every *failure*
path already removed that temp through an RAII guard; a **crash** could not,
because nothing runs in a killed process — and since the name carries a
nanosecond stamp, every crash left a distinct file that was never replayed,
never counted against the byte budget, and never mentioned by anything.
`DiskCache::open` now sweeps the directory before it reads the log.

- **What the sweep does not do — read this first.** It is not an unconditional
  reclamation, and the heading above is deliberately not "no longer leaves a
  file nothing removes". A crash stamps the temp with the **dead** process's
  pid; the restarted run has a different pid, takes the foreign branch below,
  and the file stays. Reclamation by the sweep therefore only fires when the OS
  happens to hand that same pid to a later `transync` run over the same
  directory. **On the ordinary path — crash, restart, new pid — the file is left
  for the operator**, and the run says so. That is the second of the two
  outcomes ti `d51ed7`'s *Expected* allowed ("a documented statement that
  operators may need to clean them by hand"), and it is chosen over deleting
  everything that looks abandoned because a foreign pid is not evidence of
  death.
- **Own pid: reclaimed. Foreign pid: named, not deleted.** A leftover carrying
  the running process's id is removed, unless a compaction running right now is
  holding that name — a process-wide registry of live temp names decides that,
  because two `DiskCache` handles on one directory from two threads report the
  same pid and the pid alone would have let one sweep unlink the other's
  in-flight temp. One carrying another id is left in place and reported once per
  open (count, bytes, and the glob to delete), per ADR-0024's own-pid-only rule,
  reached there for the CLI's publication temps and applying here for a stronger
  reason: this backend has no lock, so declining to touch a foreign name is the
  only protection a concurrent peer gets.
- **The peer no pid can identify is bounded rather than guessed at.** Two
  containers over one cache volume both start at pid 1, and `std` offers nothing
  portable that separates that peer from a crashed predecessor. So a compaction
  whose temp disappears under it — `rename` answering `NotFound` — now leaves
  the existing log alone, warns, and returns success instead of failing the
  open. The peer skips one compaction rather than losing its cache, which is
  what keeps `contracts.md` §1's bound (a single-writer violation costs entries,
  never a failed run) true.
- **Nothing about the cache's shape moved.** `get` is still a map lookup,
  `put`/`evict` are still one append, the byte budget still measures the live
  set, and no checksum or digest was added (DCR-0028 / ADR-0020). The sweep is
  open-time housekeeping in the same pass that already enforces capacity, and it
  is best-effort throughout: an unreadable directory or an undeletable file can
  never fail an open.
- **The recovery instruction now exists where an operator looks.**
  `contracts.md` §1 and §6, `persistence-and-files.md` (a new *Cache directory
  contents* section listing all three file classes), and `--cache-dir --help`
  each say what the file is and that deleting `transync-cache.jsonl.compact-*`
  by hand is safe when no run is active.

### Changed — BREAKING: the output field stops promising Markdown and says "document" (2026-08-13)

Ticket `490d97`. **`TranslationOutput.translated_markdown` is renamed to
`TranslationOutput.translated_document`.** Same type (`String`), same value,
same position in the struct: no behavior changed, no output bytes changed, and
no schema version moved. The `docs/architecture/contracts.md` §0 **table** and
`crates/transync/tests/public_surface.rs` are untouched on purpose — both pin
public *paths*, and `transync::TranslationOutput` is still exactly that type,
so the surface weld has nothing to re-weld. The break is field-level and
therefore invisible to that weld, which is why §0 records it in prose instead:
see its *Recorded surface decision* paragraph for this rename, and §1's
Stability list for the entry alongside the window's other breaks.

- **The old name described the input format, not the field.** Every shipped
  input path is GFM Markdown, so `translated_markdown` has been accurate by
  coincidence. The HTML→HTML translation path now tracked under the same ticket
  hands the pipeline an HTML document and returns HTML in this field, at which
  point a name promising Markdown is wrong on exactly the runs a consumer most
  needs to trust it. The format belongs to the document you handed in; the
  field just carries the translated one back.
- **It lands ahead of the feature it is for, because this is the window.** The
  HTML→HTML path is not shipped yet, and renaming a public field after v0.4.0
  would cost a second major bump for a change with no behavior in it. Doing it
  inside the sanctioned window makes the later feature purely additive.
- **Migration is one substitution at every read site.**
  `output.translated_markdown` → `output.translated_document`; nothing else
  about the type moves. `TranslationOutput` stays `#[non_exhaustive]`, but that
  buys pattern-matching consumers nothing here: a rest pattern covers the
  fields a consumer did *not* name, never a renamed binding, so
  `let TranslationOutput { translated_markdown, .. } = out;` stops compiling
  and takes the same substitution a field read does.

### Docs — four documents stop calling a shipped feature deferred, and the output ADR says what "atomic" buys (2026-08-12)

Review 0004, batch B7, findings `R0004-0092` / `R0004-0093` / `R0004-0094` /
`R0004-0097` / `R0004-0098` / `R0004-0099` / `R0004-0100`, tickets `79fa05bf`
/ `b96e0c4f` / `4f6cb4e0` / `b7a7cb9e` / `a6897485` / `62318a21` / `fec32d4e`.
Documentation only: no code, no public surface, no behavior. Every correction
was derived from the code, not from another document — three of the seven are
the house bug class, one document restating another and only one copy getting
re-derived when the feature landed.

- **ADR-0014's frontmatter still rejected what its own body records as
  shipped.** `status: active` plus a `description` asserting present tense
  ("`load_profile` rejects glossary entries with scope
  `conditional-on-section`") survived DCR-0027's SL-106..SL-109, which deleted
  `reject_reserved_glossary_scope`, `ProfileMetadata::ensure_supported` and
  `ProfileError::Unsupported` outright — while the body's own tail says "the
  rejection era is over and this ADR is a record of it". Frontmatter is now
  `status: superseded` with a past-tense description, and `docs/index.md`'s
  entry — a description-driven index, the second reader of the same claim —
  says so too. The body, including the History section, is untouched.
- **Row-window table splitting stopped being "deferred" in two places at
  once.** `docs/architecture/README.md` invariant 3 said "Row-window fallback
  is deferred" and the repo `CLAUDE.md` invariant 3 said "deferred (post-MVP)"
  — both stale since DCR-0026 shipped `unit::split`, `pipeline::merge` and
  `--table-strategy`, with `"row-window-first"` as the *shipped default
  profile's* value (`crates/transync-core/profiles/default.toml`). Both now
  describe the shipped strategy, keep the two clauses that stayed true (a
  window is a complete GFM table; isolated cell translation is rejected, not
  deferred), and name where the default and the unset/unrecognized fallback
  come from. Fixed in one pass on purpose: leaving either copy would have
  re-seeded the other — and the sweep that found the second copy found a
  third: `docs/Troubleshooting.md`'s "the one case raising the ceiling cannot
  fix" still told an operator to hand-split a 200-row table because "the
  provider-side row-window / block splitter that would fix it is deferred
  (STUB-017)". STUB-017 closed 2026-08-09; the remedy now names
  `--table-strategy` and keeps ADR-0017's semantics for every kind DCR-0026 §7
  excludes.
- **`docs/architecture/rough-schema.md` labelled `row-window-first`
  "(reserved)"** in the `ProfileConstraints` sketch — the same document that
  already cites DCR-0026 two sections earlier for
  `must_preserve_table_row_count`, which is exactly how a partial
  re-derivation leaves one marker behind.
- **`README.md` listed Anthropic as a hypothetical custom provider.**
  `transync-anthropic` is a first-party workspace member (DCR-0029,
  `contracts.md` §8). The feature bullet now names both in-tree adapters and
  their env vars, and keeps the nuance that is still true and easy to lose:
  the CLI's provider is a compile-time choice and it links OpenAI only
  (ADR-0002), and the facade re-exports no provider crate.
- **`README.md` still carried OI-0017's carve-out** — "metadata such as
  detected source language is not restored on a full cache hit" — which
  DCR-0028 / ADR-0021 removed: `pipeline::resolve_document_detection` replays
  the recorded language when a run dispatched **zero** provider batches, and
  `cli_cache_dir_persists_progress_and_detected_language` pins a byte-identical
  second run over one `--cache-dir`. Replaced with what the code does,
  including the two limits that survive (replay never overrides a live
  provider answer; the two `Cache` metadata methods are defaulted, so a
  consumer implementation that skips them keeps the old behavior).
- **`README.md`'s "~200 LOC at `web/js/sync.js`" is gone rather than
  updated** (the file is ~1,040 lines). Ticket `e9481b` settled this
  convention for the browser suite — name the artifact, never the count — and
  deleting the number is the weld: there is nothing left to go stale. The rest
  of the sentence (no framework, no build step, `data-sync-id` only, never
  parses Markdown) is the durable part and stays.
- **ADR-0006 read as a promise of transactional replacement.** "One atomic
  six-file bundle" / "the CLI ships them together as one atomic set" describe
  `output::write_fileset_atomic`, which is a *staged* fileset commit: phase-1
  failures touch no target, but a crash or I/O error mid-phase-2 can still
  leave a mixed set (exit `4`), and the publish lock makes that a crash window
  rather than a concurrency one. A dated amendment states the guarantee in
  those terms, hands authority to `contracts.md` §6, and points at `--out-dir`
  — which already publishes a whole tree by rename — as the mode that offers
  set-level atomicity. **No transactional replacement was implemented**, and
  none is being asked for.

### Fixed — the browser demos: a disclosure re-aligns, a stalled module gives up, and one id stops being special (2026-08-12)

Review 0004, batch B6, findings `R0004-0087` / `R0004-0090` / `R0004-0088`,
tickets `819c1e02` / `278793f5` / `3015de30`. All three are `web/js` scope;
none of them changes a Rust API.

- **A `<details>` toggle is now the engine's fourth reflow signal.**
  `mountSync` mirrors a disclosure's open/close across the panes (spec
  2026-08-03 §5, decision 9) and stopped there. That mirror is the one
  *content* reflow the engine causes itself, and none of the three signals
  built for OI-0024 item 1 reports it: the pane box never changes, so the
  `ResizeObserver` is silent, and no font and no image are involved. Since the
  two panes' disclosures rarely grow by the same number of pixels — translated
  body text wraps differently — the follower stayed where the pre-toggle
  geometry had left it until the reader happened to scroll again. The mirror
  now raises the same recompute `controller.refresh()` exposes, coalesced into
  one animation frame like every other signal. `contracts.md` §4a and
  `mvp-scope.md` say four signals; a scripted `<details>` open drops off
  `refresh()`'s list of things only a caller can report, because it fires a
  `toggle` like any other. `web/js/sync.js` and its byte-identical CLI twin
  moved together (`sync_js_drift` green).
- **The wasm demo's module fetch is bounded and cancellable.** `boot()` awaited
  a bare `init()`, whose wasm-bindgen glue fetches the ~1.7 MB module with no
  `AbortSignal` and no deadline — so a server that accepted the connection and
  then went quiet pinned the page at `data-demo-state="booting"` with nothing
  on screen and nothing on the console. That is the hang R0002-0052 closed for
  the three artifact fetches, one request wider. The module request is now
  handed to `init()` as a `Request` carrying the demo's own `AbortSignal`,
  bounded at 60 s and reported through the existing fatal panel. Passing a
  `Request` rather than pre-fetched bytes keeps `WebAssembly.instantiateStreaming`
  on the fast path — and with it the wrong-MIME fallback warning that the
  browser suite's clean-boot assertion uses to cover both servers' `.wasm` MIME
  entry.
- **The demo's edit model no longer treats `__proto__` differently from every
  other id.** `buildEditModel` keyed `payloads` / `statuses` off plain `{}`
  literals and `state.drafts` was one too, so a fetched map row whose
  `source_block_id` is `__proto__` hit `Object.prototype`'s accessor:
  the assignment was a silent no-op, the row vanished from the model, and
  `state.drafts[id] ?? state.payloads[id]` then read `Object.prototype` itself.
  Every *other* alien id lands in the model and is refused by name by the wasm
  engine's `reject_unknown_ids`, so this was the demo quietly disagreeing with
  itself. The three maps and `applyEdit`'s staged clone are null-prototype now.
  Rust emits `kind-NNNN`, so this is bundle-corruption robustness in the same
  class as the demo's other map gates, not a live hole.

Pinned by three new headless browser tests: `web/tests/engine.spec.js` `j`
(the toggle re-drives the follower), `web/tests/wasm.spec.js` `j` (a stalled
module fetch reaches the fatal panel) and `k` (a `__proto__` row is refused by
name, exactly as `zz-9999` already was).

### Fixed — a section is a section however its accents are encoded (2026-08-12)

Review 0004, batch B5, finding `R0004-0080`, ticket `ad8b54e4`.

- **Section and glossary identity are canonically normalized.** The one
  comparison form `profile::section_key` and `glossary_key` fold their input
  into was trimmed and case-folded, and `to_lowercase` is Unicode-aware but
  purely per-scalar: it settles case and leaves *composition* alone. So a
  heading whose `é` is written as `e` + U+0301 — the composition a
  macOS-originating file carries by default — never matched a selector whose
  `é` is the precomposed U+00E9, though the two are the same string under
  Unicode's own definition of canonical equivalence. The entry applied to no
  section and said nothing (DCR-0027 G7's run-level warning fires only when an
  entry matches *no* section of the document, so the partial miss was silent),
  and a glossary term in the same shape claimed nothing, so two contradictory
  bullets for one term could render into every prompt. Both keys now run
  through one private `canonical_key`: trim, case-fold, then **NFC**. NFC comes
  last because case mapping preserves canonical equivalence but not the
  composed form, and composing last makes the fold idempotent on its own
  output — which is what lets both sides of every comparison run through it.
- **Cache consequence: an affected entry re-keys once.** This identity decides
  which glossary entries a section's prompt carries, so it decides the run's
  cohorts, its compiled prompt bytes, and therefore `profile_prompt_hash` and
  `glossary_hash` (contracts.md §5a). A profile whose text is already NFC —
  every ASCII profile, and every editor-typed one — folds to exactly the keys
  it folded to before and keeps every cache entry; a profile this actually
  moves re-dispatches its units once and re-files them under the new key.
  Unlike the `input_mode` axis addition, **a `DiskCache` log written by an
  earlier build replays cleanly**: no record field is added or removed, the log
  grammar does not move, and every stored record still deserializes and indexes.
  The affected keys simply stop being asked for, and the entries under them age
  out by the ordinary eviction rules rather than being skipped at open.
  `CACHE_DISK_FORMAT_VERSION` and `VALIDATION_SCHEMA_VERSION` both stay put —
  no documented trigger of either fires.
- **One new dependency edge, no new crate.** `transync-core` now depends on
  `unicode-normalization`, which comrak already pulls into the graph through
  `caseless`. `transync-syntax` takes nothing: it compiles to `wasm32` and its
  dependency set is welded (ADR-0019).

### Fixed — a prompt that eats the whole input budget is named, not shipped in silence (2026-08-12)

Review 0004, batch B5, finding `R0004-0076`, ticket `8c6cbc8b`.

- **The input axis gets the preflight the output axis has had since D1.** The
  packer reserves the once-per-batch envelope — compiled system prompt,
  instruction envelope, both language labels — off the input target, and
  derives the payload target as `target - envelope`, floored at 1. When a
  profile's own rendered prompt meets or exceeds the whole input target, that
  floor turned the defense into its opposite: every unit packed alone, and
  every request went out over the target the reserve exists to defend, with
  nothing said. `batch::input_budget_warning` now answers the question the
  output side's `output_budget_warnings` already answered on its axis, and
  `unit::build_batches` raises it on the `transync::profile` channel beside the
  other profile diagnostics — once per run, not once per section — naming
  `[batching].target_input_tokens_per_batch` as the knob that moves it.
- **A warning, not a refusal**, because the input target is a documented soft
  cap and a single over-budget unit deliberately still ships; what was missing
  was the diagnosis. No public surface moves: the warning is a `pub(crate)`
  type in the crate-private `batch` module, and it is not a
  `ValidationReport` field —
  its output-side twin names *units* a report reader needs listed, while this
  one names the run's own configuration.

### Fixed — cache compaction earns the never-a-hybrid promise it makes, and cleans up after itself (2026-08-12)

Review 0004, batch B4. Three findings in one function, all of them at open only.

- **The compacted log is synced before it is renamed into place.** DCR-0028 §4
  promises that a crash mid-compaction leaves one of the two intact files and
  never a hybrid; that held for a process crash and not for a power loss, where
  the rename could become durable before the bytes it named. The temp file is
  now `sync_all`ed before the rename and the directory `fsync`ed after it. This
  does not move §5's no-`fsync` rule for the append path — that rule argues from
  per-unit write cost, and compaction runs at most once per open. The directory
  sync is best-effort, because a cache must never fail an open over a hardening
  step. `R0004-0028`.
- **A failed compaction no longer leaves its temp file behind.** The temp is
  owned by a guard that removes it on drop unless the rename succeeded; the
  names carry a pid and a nanosecond stamp, so a misbehaving disk used to
  accumulate distinct files in the operator's cache directory. A crash still
  leaves one, and it stays inert and operator-deletable as documented.
  `R0004-0030`.
- **Compaction stopped flushing once per record.** The per-record flush exists
  for the live append path's survive-process-exit promise; a private temp file
  that is renamed or discarded whole has no such promise to make, so it was one
  `write(2)` per record defeating its own `BufWriter`. It now buffers, flushes
  once and syncs. `R0004-0031`.

### Fixed — the cache log weighs its dead records as well as counting them, and replay stops reading the whole file into memory (2026-08-12)

Review 0004, batch B4. Both entries come from one assumption the disk cache's
design left implicit: that records are interchangeable in size.

- **Compaction now triggers on dead bytes as well as dead records.** The
  trigger asked whether superseded and evicted *records* had caught up with the
  live ones, and the byte budget beside it measures "what a compacted log would
  occupy" — so nothing in the backend ever looked at the file. Dead weight
  concentrated in a few very large records (one enormous superseded entry, a
  handful of unreadable lines) therefore passed both tests while the file
  stayed physically enormous, open after open. Compaction now also runs when
  the bytes a rewrite would reclaim have caught up with the bytes it would keep
  — the same "at least half dead weight" rule, applied to the measure an
  operator can see. It holds with `max_bytes: None` too, and it cannot churn: a
  compacted file has no dead bytes. `R0004-0026`.
- **Replay streams the log instead of slurping it.** `DiskCache::open` read the
  whole file with one `std::fs::read` before folding it, so the peak allocation
  was the in-memory index *plus* a byte-for-byte copy of every dead record the
  index was about to discard — on a log that is append-only between
  compactions, which is precisely the file that can be many times its own live
  set. It is now read a line at a time through a `BufReader`; the peak is the
  index being built. The log is still read exactly once at open and every
  recovery path (torn tail, unreadable line, rotate-aside) keeps its semantics,
  with one refinement: a file about to be rotated aside is no longer truncated
  first, because rotation exists to preserve every byte that was there — and
  the warning such a file produces names that rotation, rather than the
  truncation only the in-place repair performs. `R0004-0024`.

### Fixed — `transync serve` reads a file at the length it measured, not at what the file has since become (2026-08-12)

Review 0004, batch B4.

- **The in-memory branch of `send_file` caps its read.** The eight-megabyte
  split is decided from the open handle's metadata, and the read that followed
  trusted that same number instead of enforcing it: a writer appending between
  the two — an operator regenerating the bundle while `serve` runs is the
  innocent case — decided how large a buffer the server allocated. The read is
  now `Read::take`-bounded at the measured length, which is the discipline this
  repo already states for its own admission control (contracts.md §6, "file
  metadata is never trusted alone") and the one the streaming branch above the
  limit already applied. Both branches now answer a mid-request growth
  identically. `R0004-0045`.

### Fixed — every destination question is asked before the provider is paid, and every diagnostic is one line of text (2026-08-12)

Review 0004, batch B3. Five CLI fixes with one theme between the first three:
this repo's own R0002-0029 rule is that a question the argv and the filesystem
can answer is answered **before** the translation is bought, and three corners
were still answering after.

- **A destination set that nests one destination inside another is refused up
  front.** `--output x --map x/y` (or `--output x` beside `--html-out x`) names
  no file twice, so the duplicate check passed it — and then the publication
  needed `x` to be a regular file and a directory at once. It failed in
  `create_dir_all` when `x` already existed, and inside the phase-2 rename pass
  (the one that cannot be rolled back) when it did not. Containment is now
  compared on the same normalized identities as duplication, component-wise, so
  `xy` is still not inside `x`, and the refusal names both paths. `R0004-0067`.
- **`--force` no longer skips the `--html-out` shape check.** The waiver is
  consent to destroy foreign *content* (DCR-0021); it was never permission to
  skip a check that costs one `stat` and whose answer no flag changes — the
  bundle writes its six files *into* a directory, and a regular file is not
  one. `--force --html-out <regular file>` used to pass both preflights and die
  in `create_dir_all` after the whole document had been translated. A dangling
  symlink at the path failed the same way even *without* `--force`, because
  `exists()` follows links; the shape is now read with `symlink_metadata` and
  then resolved, so a link to a real directory still passes. The `--out-dir`
  case is unchanged and deliberately different: a regular file there **is**
  replaceable under `--force`. `R0004-0063`.
- **A destination whose final path component is not valid UTF-8 is refused in
  the preflight, by its real name.** Every staging name transync builds is that
  component plus a suffix, so such a run could never publish; it used to be
  told so by the staging code after the provider call, in a sentence blaming a
  missing file name the path did not lack. Both modes ask it now, and the late
  pass says the same sentence. `R0004-0066`.
- **Two silent cleanups became audible.** A `--out-dir` replace whose backup
  removal fails leaves the operator's *entire previous output* under a hidden
  dot-name — the publication itself succeeded, and this call was the only thing
  that could see it happen; and the opportunistic sweeps of this process's own
  crashed-predecessor residue dropped both their scan errors and their removal
  errors. Neither changes what was published, and neither is now
  indistinguishable from "there was nothing to clean up", which is the whole
  point of the `note:` discipline the rest of the module already followed.
  `R0004-0054`, `R0004-0056`.
- **Provider-authored text can no longer forge a `transync: ` line or reach the
  terminal as a control sequence.** Invariant 7 makes source Markdown
  untrusted, and most of what lands on stderr is derived from it or from a
  remote server: an HTTP error body, a model-authored warning or rejection
  reason. A raw newline in that text forged a diagnostic the run never emitted;
  a raw `ESC` drove the terminal. The whole C0/C1 range is escaped at the
  rendering boundary now — `\n` / `\r` / `\t` by name, everything else as
  `\u{XX}` — so nothing is dropped and nothing is truncated. This is the
  third, un-adopted ask of `R0002-0068`, which bounded the same text in count
  and in bytes but not in alphabet. `R0004-0071`.
- **Docs:** files-mode publication no longer claims whole-set atomicity. ADR-0006's
  "atomic" is per *file* — stage, fsync, rename, one destination at a time — so
  the guarantee is that a failure while content is being written leaves every
  destination untouched, and a crash inside the rename pass can still leave
  some destinations new and the rest as they were. `--out-dir` is the mode that
  commits a whole set at once. `R0004-0009`.

### Security — BREAKING: `transync serve` answers only for the authorities it was reached as (2026-08-12)

Review 0004, batch B2. The loopback demo server ignored `Host` entirely, so it
answered any request that reached the socket — which is the whole of a DNS
rebinding attack. A page on `attacker.example` whose name resolves, on a second
lookup, to `127.0.0.1` keeps its own origin while its `fetch` lands on this
server; the browser then treats the served bundle as same-origin with the
attacker's page and hands the user's translated document to script. Binding
loopback does not prevent it: a bind decides which *network* can open the
socket, and the rule it leans on for who may *read* the answer is the
same-origin policy that rebinding dissolves. `R0004-0002`.

- **Every request must name an authority this server answers for.** No `Host`,
  two of them, an obs-folded one, or one that is not an authority is `400`; a
  well-formed authority that is not this server's is `421` with a body naming
  the ones that are. The check runs on the head, ahead of the method and ahead
  of any filesystem work, so a request for another authority learns nothing —
  not even which paths exist. The rebound request is indistinguishable from a
  legitimate one *except* in `Host`, which mirrors the URL the page used and
  which the attacker cannot change without giving up the origin the attack is
  for.
- **The answered-for set comes from the bind**: the bound address literal, plus
  `localhost` when that address is a loopback one (RFC 6761 reserves the name
  for exactly that address, and it is what the smoke checklists type), plus —
  for a wildcard bind, which names no interface — the loopback authorities the
  socket is also listening on. The port is part of the authority, so
  `Host: 127.0.0.1` names port 80 and is not a server on `7470`.
- **`--allow-host <authority>` (new, repeatable)** names what a bind cannot: the
  address or name another machine reaches a `--bind 0.0.0.0` server at. It takes
  `host` or `host:port`, a bare host meaning the bound port; a value that is not
  an authority is an argument error, exit `1`. Deliberately *not* an escape
  hatch that turns the check off under `--bind`: a wildcard bind includes
  loopback, so that is the configuration where dropping the check would leave
  the hole open and reachable from off the machine. The server prints the
  authorities it answers for at startup, so a `421` has its explanation on
  screen already.
- **Breaking for one shape of caller**, inside the sanctioned v0.4.0 window: a
  client that sent no `Host`, or one naming an authority it did not reach the
  server at, now gets a refusal instead of the bundle. Browsers and `curl`
  always send the authority they used, and `scripts/test-browser.sh`,
  `web/playwright.config.js` and `scripts/smoke-live.sh` all reach the server at
  the address it bound. `scripts/smoke-live.sh` gains a
  `TRANSYNC_LIVE_ALLOW_HOST` knob for its non-loopback `TRANSYNC_LIVE_BIND`
  path. Not covered by ticket `b791d6`'s "non-loopback deployment hardening"
  exclusion: this is request validation, and the configuration it protects is
  the loopback default.

### Fixed — `transync serve` keeps the request-head cap it states, and a `HEAD` refusal announces the length a `GET` would have sent (2026-08-12)

Review 0004, batch B2. Two response-shape defects in the loopback server, each
visible only to something that measures what the server says about itself.

- **The 8 KiB request-head cap is the number it states** (`R0004-0041`). The
  read loop checked the buffer *before* each fixed 1024-byte read, so a head
  whose terminator arrived inside the final chunk was accepted at up to
  `MAX_HEAD_BYTES + 1023` — the documented cap was soft by 12.5% and moved
  with the client's write pattern. The loop now asks only for the room that is
  left, so a head of exactly the cap is served and one byte more is `431`.
  Nothing downstream trusted the cap, so the consequence was imprecision
  rather than exposure; memory was bounded either way.
- **A `HEAD` error response no longer reports `Content-Length: 0`**
  (`R0004-0047`). The status path suppressed the length along with the body,
  so a `HEAD /missing.html` announced a zero-length `404` that no `GET` of that
  URL produces. `Content-Length` on a `HEAD` response describes the
  representation a `GET` would have returned (RFC 9110 §8.6), so the field now
  carries the body's length under both methods and only the body itself is
  withheld — which is what `send_file` already did for `200`s.

### Fixed — the OpenAI adapter normalizes a zero output ceiling too, and both injection surfaces say who owns the redirect policy (2026-08-12)

Review 0004, batch B1, second pass: the two residuals the gate's own audit
found after the round above landed. Neither is a new defect; each is the half
of an already-closed finding that stopped at one crate.

- **A zero output ceiling is normalized on the OpenAI surfaces as well**
  (`R0004-0016`, the sibling). The Anthropic fix above closed the
  caller-built-`TranslationBatch` bypass at that adapter's boundary, but
  `transync-openai` still passed a `Some(0)` through raw — onto the wire as
  `max_completion_tokens: 0` (Chat Completions) and `max_output_tokens: 0`
  (Responses), a ceiling with nothing to answer in. Both surfaces now read the
  knob through one shared `translation_output_ceiling`, which maps zero to
  absent; absent on these surfaces means *omit the field*, so a zero resolves
  exactly the way core says it should ("ignored — exactly as if the key were
  absent"). Only zero is normalized: a nonzero-but-tiny ceiling still makes a
  valid request and a loud, terminal exhausted-ceiling diagnostic, which is
  the designed behavior. The rule now holds on both surfaces of both crates
  rather than on the crate the review happened to name.
- **Both injection surfaces document that an injected client brings its own
  redirect policy** (the doc residual of `R0004-0001`, on the terms
  `R0004-0020` set for the timeout half). `client::translate_on` /
  `extract_glossary_on` and `client::call_*` accept any `reqwest::Client`, and
  in `reqwest` the redirect policy — like the timeout budget — is settled at
  *client build* time, with no per-request override an adapter could apply on
  top. The `redirect::Policy::none()` above is therefore a guarantee about the
  clients these crates build, not about the injection contract: a caller whose
  client keeps the default follow-up-to-ten policy re-opens the hop for its
  own calls, carrying the `x-api-key` header on the Anthropic path and the
  document body on both. Said at the injection functions themselves and in
  each `client` module's docs, because a reader landing on one function's page
  never sees the adapter that avoids the question. Documentation only —
  rebuilding or refusing a caller's client is not what an injection API is
  for.

### Security — BREAKING: neither provider client follows a redirect, and neither accepts a base URL carrying userinfo (2026-08-12)

Review 0004, findings `R0004-0001` (the round's only HIGH), `R0004-0021`,
`R0004-0022` and `R0004-0023`. One subject in four parts: what the two
authenticated provider clients do at the network boundary, and why the two
crates now agree there by decision rather than by accident.

- **The Anthropic api key could cross to another origin on a redirect**
  (`R0004-0001`). `http_client_builder_with` set a connect timeout and a
  request timeout and nothing else, so `reqwest`'s default policy followed up
  to ten redirects — and this provider's credential rides in a **custom**
  `x-api-key` header. `reqwest` strips only its own fixed sensitive set
  (`Authorization`, `Cookie`, `Proxy-Authorization`, `Www-Authenticate`) on a
  cross-origin hop; a custom header is not in it. A compromised or
  misconfigured endpoint could answer the Messages POST with a `3xx` and
  receive the key together with the document body. `transync-openai` was
  never exposed — `bearer_auth` sets `Authorization`, which *is* stripped —
  and that asymmetry was the defect: one crate was safe by accident of which
  header its provider chose. Both clients now set
  `redirect::Policy::none()`. Nothing legitimate is lost: each adapter posts
  to one endpoint it builds itself, and `reqwest` would have rewritten a
  followed 301/302/303 into a bodyless `GET` in any case. The Anthropic
  crate's proof is a two-socket test — the endpoint answers `302` pointing at
  a second listener, and that listener must never see a request.
- **An unfollowed `3xx` is terminal, not retryable** (`R0004-0021`). With the
  policy above, a redirect arrives at `provider_error_for_status` as a
  *response*; the catch-all made it a retryable `Network` error, so the same
  POST earned the same `Location` on every attempt in the bounded budget
  before surfacing anyway. Both tables now classify `300..=399` as
  `ProviderRejected`, carrying the status and a message naming `base_url` as
  the thing to change.
- **Deterministic `reqwest` failures are terminal** (`R0004-0022`). ADR-0009's
  retry is a *verbatim* resubmission, so a failure decided entirely by the
  request itself reproduces exactly. `map_reqwest_error`'s catch-all made
  builder and redirect-policy errors retryable; timeout, connect and decode
  keep their existing classes, those two become terminal `Other`, and an
  unrecognized cause still takes the retryable catch-all rather than being
  declared permanent on a guess. The terminal set stops there on purpose:
  `reqwest`'s request *kind* looks like a third deterministic class and is
  not, because the async client stamps it on every in-flight failure its
  hyper service reports — a socket the server closed before answering, a
  reset after send, an `h2` GOAWAY — none of which carry a connect-phase or
  timeout source, and all of which are exactly what the retry budget is for.
  Both crates pin it with a loopback server that drains the request and drops
  the socket unanswered.
- **BREAKING: a base URL carrying userinfo is refused at the checked
  constructors** (`R0004-0023`). `https://user:pass@gateway/` passed the
  scheme and host checks, and the endpoint builder clears only the query, the
  fragment and the path — so userinfo survived onto the wire, where `reqwest`
  turns it into an `Authorization: Basic` header: a second unmanaged
  credential on the Anthropic path, and one that collides with the bearer on
  the OpenAI path. The URL is also the one piece of configuration that gets
  echoed into logs, so an embedded password leaked where the api key
  deliberately never does. `TransyncAnthropic::try_new` / `from_env` and
  `TransyncOpenAI::try_new` / `from_env` now return
  `ConfigError::MalformedUrl`; the message names the shape and never the
  value. The unchecked `new` constructors are unchanged — they validate
  nothing by contract. Not an ADR-0020 matter: that record scopes adversarial
  hash collision out of the threat model, and this is credential hygiene in
  the configuration an operator wrote.

### Fixed — an empty leading text block no longer masks an Anthropic answer, and a zero output ceiling is never sent (2026-08-12)

Review 0004, findings `R0004-0018` and `R0004-0016`. Both are in the
Anthropic request/response body rules, and both were narrow doors that turned
a workable request into a provider-visible failure.

- **`R0004-0018`** — `output_from_envelope` selected the first `text`-typed
  content block and *then* filtered emptiness, so a reply shaped
  `[{text: ""}, {text: "{…}"}]` was reported as
  `no non-empty text content block` even though the second block carried the
  answer. A valid response became a false missing-text error: it burned a
  content retry, and on repeat fell the unit back to source. Emptiness is now
  part of finding the answer rather than a verdict on the block already
  chosen — the first block that actually carries text is the answer, exactly
  as the docs always said.
- **`R0004-0016`** — `translation_body` mapped `target_output_tokens: Some(0)`
  to `max_tokens: 0`. Core's `profile::normalize_batching` already treats zero
  as "not an output budget; ignored … exactly as if the key were absent", at
  both `load_profile` and `unit::build_batches`, so the pipeline never hands
  one down; a caller assembling a `TranslationBatch` itself and calling the
  adapter directly bypassed both. On this API that is not merely a useless
  ceiling — `max_tokens: 0` alongside the `output_config.format` this adapter
  always sends is refused by the provider. Zero now takes the same road an
  absent value does (`Ceiling::Default`), because `max_tokens` is required and
  something must ride.

### Docs — the ownership table names the structural-fingerprint oracle, and its module list is welded shut (2026-08-10)

Ticket `2d3b16`, a row that was never written rather than one that went stale.
`transync-core::structure` is the shared "what shape is this block" oracle —
`unit::payload` records the *source* block's fingerprint into
`BlockConstraints`, `validate::per_kind` re-derives it from the *translated*
payload and compares — and its module doc gives the reason it is one module
("a second copy would drift and silently disarm the check") in almost the words
`source-of-truth-table.md` opens with. The table had no row for it, so a reader
asking who decides what shape a block is found `parser` (kind classification)
and `validate` (checking rules) and no hint of the third, separately-owned
thing they compare.

- Three rows added, each a single-owner oracle with consumers in more than one
  module: `transync-core::structure` (the fingerprint), `transync-syntax::walk`
  (top-level normalization + per-list item count, imported by the renderer and
  by `validate::full_reparse` from *different crates*), and
  `transync-syntax::outcome` (which raw-HTML blocks become units, read by
  batching, alignment and the pipeline report so a block's row, its counters
  and its presentation cannot disagree).
- The intro's crate-by-crate module enumeration read as complete while omitting
  `structure`, `walk`, `outcome` and `error`. It is now complete **and welded**:
  `docs_ownership_drift.rs` reads both crates' `lib.rs` and fails on a module
  the table never names, skipping `cfg`-gated test scaffolding by the attribute
  rather than by a deny-list. An omission is invisible in a way a wrong
  sentence is not — nothing about a paragraph advertises what is missing from
  it — which is why this one gets a test instead of a proofread.

### Docs — the manual's exit-code tables catch up to `6` and `7` (2026-08-10)

Ticket `6a034e`. Ticket `e62b59` added exit `6` (the provider refused the run
over **how it was configured**) and exit `7` (the provider refused **this
document's content**), moving six causes out of `5` and renumbering nothing.
The `docs/` bundle moved in that commit; `manual/` is derived one-way from
`docs/` and did not, so its tables described a failure set the CLI stopped
having. Corrected from the code (`ExitCode::for_pipeline_failure`) and
`contracts.md` §6:

- `manual/reference/user/en/cli.md` — the exit-code reference table stopped at
  `5`, and its `5` row still credited `transync serve`'s deferred-stub exit,
  which `serve` stopped having when it became a real server on 2026-08-09 (that
  code now means it could not bind). Gains rows for `6` and `7`, plus the
  append-only rule that explains why nothing below them moved.
- `manual/how-to/user/en/diagnose-a-translation-run.md` — "Read the exit code
  first" was built entirely on the old set. Its stderr-symptom table also
  labelled neither of the two rows that moved to `6` (`authentication: … does
  not have access to model …`, `output incomplete (reason: max_output_tokens)`)
  and carried no row at all for the `content filtered` / `model refused` pair
  that now exits `7` — the one failure whose remedy is "stop re-running this".

No code changed; `crates/transync-cli/tests/exit_code_docs_drift.rs` already
welds the enum to `contracts.md` §6 and the Developer Guide, and deliberately
does not reach into generated prose.

### Fixed — an `--out-dir` replace judges the tree it is about to rename away, not the one it saw a bundle-write earlier (2026-08-10)

Ticket `cbbc4e`, the residual DCR-0021 carried through two fix rounds: the
publication lock serializes an `--out-dir` replace against a files-mode publish
into the same directory and into its `html/`, but not against one *deeper* than
that. No claim scheme closes it — a replace claims a fixed, shallow set while a
publication claims a point that can be arbitrarily deep — so none is added. What
closes the reachable half is ordering, and two fixes make that ordering hold.

- **The replaceability guard runs twice.** It ran once, before the whole bundle
  was staged and fsynced, and the publish lock does not hold that verdict still:
  a bare `mkdir` takes no lock, and a files-mode peer creates its destination
  levels *before* it locks and then parks on the level the replace holds. It now
  runs again with the staged tree in hand, immediately before the swap. A target
  that grew an entry meanwhile is refused (exit 4) with `… (it changed while
  this run was staging its output)`, its previous contents left in place and the
  staged tree removed.
- **A directory wearing a transync name is no longer mistaken for one.** The
  six-file bundle allow-list and both `<name>.tmp.<pid>` staging-temp
  recognizers matched on the name alone, so a *directory* called `index.html`
  inside `html/`, or one called `out.md.tmp.<pid>` at the top level, was read as
  a bundle file or as transync's own residue — and its whole subtree went into
  the backup `remove_dir_all` of a replace that needed no `--force`. All three
  now require a regular file, which is the rule the top level has applied to
  `out.md` since ti `66339b` and to the two marker names since its review round.
  A symlink at one of those names is likewise judged as what it is, so a bundle
  assembled with symlinked files now needs `--force`.

Together these mean that, without `--force`, a peer cannot both create a level
inside an `--out-dir` target and publish into it: it either waits on the
replace's lock (its claim is read before it creates anything, so it lands on the
target or above) or it makes the replace refuse. What is accepted rather than
open — `--force`, which waives the guard by request, and the few syscalls
between the second guard pass and the rename — is stated in `contracts.md` §6
and in DCR-0021's third 2026-08-10 note.

### Docs — the ownership table stops calling the disk cache deferred, and a test keeps it that way (2026-08-10)

Ticket `d00367`. `DiskCache` shipped on 2026-08-09 (DCR-0028 / ADR-0021,
`transync translate --cache-dir`), and the living-doc closures that rode that
work reached `mvp-scope.md` and `open-issues.md` and stopped short of two
architecture documents. `source-of-truth-table.md`'s Translation cache row
still read "Process-lifetime | In-memory only per `mvp-scope.md`; disk-backed
cache is DEFERRED" — both halves false — and `persistence-and-files.md` opened
by calling the library "filesystem-blind" and listed the cache under what
transync does not persist, flat.

- **The cache row now describes two backends and their two lifetimes.**
  `InMemoryCache` (the default) dies with the process; `DiskCache` replays a
  versioned JSON-lines log at open. What a backend picks is *where* entries
  live — `CacheKey` still owns what makes two units the same unit, and the row
  says so, because that is the ownership claim the table exists to make.
  `DocumentMeta` and `GlossaryExtraction` are named as riding the same seam.
- **The section it sits in was called "In-process state"** and is now "Run
  state": a cache the caller opens on disk outlives the process, so the
  heading was false along with the row.
- **`persistence-and-files.md` gets the same correction** in two places — the
  opening claim and the "does not persist" list. The library's one filesystem
  door is `DiskCache::open(<dir>)` on a path the *caller* supplies; nothing is
  discovered, defaulted or guessed, and there is still no XDG presence.
- **Two adjacent rows corrected while the file was open.** API-key discovery
  is now "provider credential discovery (`OPENAI_API_KEY`,
  `ANTHROPIC_API_KEY`)" owned by whichever provider crate needs it — naming
  only `transync-openai` stopped being complete when `transync-anthropic`
  shipped in this same window (DCR-0029). The two *other* places the same two
  documents name a credential — the table's "does not own" bullet and
  `persistence-and-files.md`'s "API keys" bullet — were still OpenAI-only, so
  the files disagreed with themselves; both now name both variables and say
  the rule that generalizes them, one variable per provider crate.
- **New guard: `crates/transync/tests/docs_ownership_drift.rs`.** Both
  documents must name every `Cache` backend a consumer can construct, and the
  names come from the types via `std::any::type_name`, so renaming `DiskCache`
  fails the test until the documents follow and deleting a backend fails the
  build. A short list of retired absolute claims ("disk-backed cache is
  DEFERRED", "filesystem-blind", "process-lifetime in-memory only") is
  asserted absent. "In-memory only" is deliberately *not* on that list: "in
  memory only when no `--cache-dir` is given" is a true sentence, and a guard
  that forbids true sentences is one the next author deletes.
- **Two documents left alone on purpose:** `docs/project/intake.md` and
  `docs/superpowers/specs/2026-05-01-transync-design.md` still say the cache
  is in-memory only. Both are dated records of what was decided when they were
  written, and the index marks the second historical; the guard covers living
  documents only.
- **No library change:** documentation and one new test file. No
  `contracts.md` §0 change and no `public_surface.rs` change.

### Docs — the documentation index's own rule is what the drift test enforces (2026-08-10)

Ticket `1347b4`, routed by the owner to option (a) — index the missing
document *and* widen the guard. `docs/index.md`'s header has always claimed to
be the "single entry point for everything under `docs/`", and
`crates/transync/tests/docs_index_drift.rs` enforced only `docs/decisions/*.md`
and the DCRs while its module doc restated the header's broader claim. The
header, the test and the tree were three different things, and the gap was
real: `docs/backlog.md` — the cross-source index of open items that
`docs/project/status.md` points a reader at — was linked from neither the index
nor `docs/architecture/README.md`, so it was unreachable from the entry point
with nothing going red.

- **Two documents are now indexed.** `docs/backlog.md` (also added to
  `docs/architecture/README.md`'s "Where to look next", so it is reachable from
  both entry points) and `docs/project/git-history-loss-2026-08-10.md`, the
  record of the 2026-08-10 object-store loss that this run created — and that
  the widened check caught on its first run, which is the cheapest possible
  proof the widening works.
- **The check is now the header's rule:** every Markdown document under
  `docs/`, at any depth, must be linked from the index. The required set went
  from 46 documents (21 ADRs + 25 live DCRs) to 88.
- **Four stated exclusions, and no others.** `*.ko.md` Korean siblings;
  git-ignored bundles under `docs/` (today `docs/investigation/`, untracked
  since 2026-08-08 — an untracked bundle is not the index's to carry);
  `index.md` itself; non-Markdown files, so `project/phase-state.yaml` is
  listed because a reader wants it rather than because the test demands it.
  The git-ignored set is **read out of `.gitignore`** rather than hardcoded, so
  it cannot outlive the decision that created it — re-track
  `docs/investigation/` and its 29 files are required in the same edit. The
  exclusions are stated in `docs/index.md` for a reader and asserted by
  `the_stated_exclusions_are_the_only_exclusions`; a drift test that fires on
  files nobody intends to index is one the next person to trip it will weaken.
- **Test names moved.**
  `docs_index_drift::every_live_decision_and_dcr_is_linked_from_index` is now
  `every_doc_under_docs_is_linked_from_index` (earlier CHANGELOG entries name
  the old one; they are dated records and stand as written), and the file grew
  two guards — `the_required_set_is_the_whole_docs_tree_not_just_the_record_trees`
  and `the_stated_exclusions_are_the_only_exclusions`. Two tests became four.
- **No library change:** documentation and one test file. No `contracts.md` §0
  change and no `public_surface.rs` change.

### Added — CLI exit codes `6` and `7` split provider failures by what the operator must do next (2026-08-10)

Ticket `e62b59`, the CLI half of the DCR-0023 / DCR-0029 provider taxonomy.
Through v0.3.0 the CLI answered a wrong API key, a `--model` that does not
exist, a batch that overran the output ceiling and a document the provider's
content policy refused with the **same** exit code, `5`, so the only
discriminator was the stderr string — which is exactly the interface the
taxonomy work was filed to stop consumers from parsing. A script wrapping
transync could not tell "fix my configuration and retry" from "skip this
document and move on".

- **`6` — the provider refused the run because of how it was configured.**
  `TranslatorError::Authentication`, `ProviderRejected`,
  `OutputCeilingExhausted` and `ContextWindowExceeded`. Four causes naming
  four different knobs — the credential, the model name,
  `[batching].target_output_tokens`, the `[batching]` input budget — but one
  action, which is the granularity a process exit code can carry.
  Deliberately **not** `1`: that code means the CLI refused the arguments
  before anything ran, while `6` means they were accepted, the run started,
  and the provider refused it.
- **`7` — the provider refused this document's content.**
  `TranslatorError::ContentFiltered` and `ModelRefused`. No configuration
  change helps: transync's retry is a verbatim resubmission (ADR-0009), so
  the identical content that was refused is what a retry would send. Distinct
  from `3` because `3` still writes a full output set; `7` aborts before
  publishing anything (ADR-0017).
- **`5` narrows to the residual and every other code is untouched.**
  `Network` / `RateLimited` with the retry budget spent, `MalformedResponse`,
  `ResponseTooLarge`, `Unsupported`, `Cancelled` and `Other` still exit `5`,
  each for a stated reason rather than by omission — `Unsupported` in
  particular is genuinely ambiguous between a model swap and a document
  construct, and a code that guesses is worse than one that admits it does
  not know. **Nothing was renumbered or re-meant**: `0`–`5` mean exactly what
  they meant in 0.1.0, because a deployed `if [ $? -eq N ]` has no way to
  learn that a number moved.
- **Consumer impact:** a script that matched `5` as "any provider failure"
  now sees `6` and `7` for six of those causes. A script that only checks
  `!= 0` is unaffected, as is every library consumer — the exit codes are the
  CLI's policy over the taxonomy, and `TransyncError::stable_code()` is
  unchanged.
- **The library surface did not move.** No `contracts.md` §0 change and no
  `public_surface.rs` change: the classification is `transync-cli`'s
  `ExitCode::for_pipeline_failure`, and the new
  `test_stub::TerminalErrorTranslator` sits in the feature-gated
  `transync::test_stub` module §0 explicitly excludes.
- **Documents moved together:** `contracts.md` §1 and §6, `Developer_Guide.md`
  (table, the `0..N` claim, the pitfalls table), `Troubleshooting.md` (the
  output-ceiling section now says `6`; a new exit-`7` section), `README.md`,
  `design-baseline-2026-07.md`, `module-map.md`. The three historical
  snapshots that state `0..5` (`design-baseline.md`, `stub-manifest.md`,
  `implementation-slice-checklists.md`) keep it — it was true when written.
- **Regression guards:** `crates/transync-cli/tests/exit_code_docs_drift.rs`
  welds the enum to §6's list, the guide's table and every living `exit codes
  0..N` claim, so the next extension cannot be done halfway; four unit tests
  in `error.rs` pin the mapping including the residual; and
  `cli_smoke::{cli_exit_6_configuration_rejected, cli_exit_7_document_refused,
  cli_exit_5_stays_the_unclassified_residual, cli_unknown_stub_mode_is_an_error}`
  drive each code end-to-end through the binary on a new `TRANSYNC_STUB_MODE`
  vocabulary (`auth`, `provider-rejected`, `output-ceiling`,
  `context-window`, `content-filtered`, `model-refused`, `unclassified`
  beside the existing `fail`). An unrecognized mode is now an error rather
  than a silent fall-through to the echo stub.

### Fixed — an `--out-dir` replace and a files-mode publish into that same directory now serialize (2026-08-10)

Ticket `40e2a5`, closing the "Known boundary" DCR-0021 recorded. Run A
publishing `--out-dir X` locked the directory *holding* X (where its staging and
backup siblings live); run B publishing `--output X/out.md` locked X itself.
Different inodes, so nothing serialized them, and A's swap either took B's
committed files with the backup or left B writing into A's fresh tree.

- **A fileset commit also locks the deepest level that exists** on the way to a
  destination it has to create. For a destination that already exists that is
  the destination itself, so an ordinary publication locks exactly what it
  locked before; for one being created — the case an inode-keyed lock cannot
  reach, since a directory that does not exist has no marker — it is the level
  that holds it, which is the level the `--out-dir` publish holds. The claimed
  level is always one the run is creating into, so no publication starts needing
  permission it did not need.
- **An `--out-dir` publish also locks the directories inside its target** (the
  target and its `html/`), which is the whole shape a published out-dir has and
  therefore the whole set a files-mode peer can be holding. Locking a target
  that is about to be renamed away is sound because of the revalidation in the
  entry above.
- **Not a name-keyed lease in the parent**, the other candidate: that would have
  `transync translate --output out.md` in `~/project` write a lock marker into
  `~`, and fail where a parent is unwritable but the destination is not.
- **The inner set is re-read once the locks are held** (review fix round), and
  the whole set retaken until it stops changing — bounded at four passes, then
  an error. It is a filesystem read, so a replace that started before its target
  existed locked nothing inside it; a peer could create the target while the
  replace queued on the level above, and a run *started after that* claims the
  target itself and publishes into the tree being renamed away. The re-read runs
  before the replaceability guard, so guard, lock set and swap act on one tree.
- **Still uncovered:** a run publishing *deeper* inside an `--out-dir` target
  than `html/`, and a directory that appears inside the target after that last
  re-read, put there by something the replace is not serialized with (a bare
  `mkdir`, or a peer that creates its destination levels before parking on the
  replace's lock). Nothing transync produces the first shape; both are ticket
  `cbbc4e`, and `contracts.md` §6 states them under "Scope".
- **Regression guards:** `output::tests::{an_out_dir_replace_waits_for_a_files_mode_publish_into_its_target,
  a_files_mode_publish_into_a_fresh_directory_waits_for_the_run_that_holds_its_parent,
  a_replace_re_reads_what_is_inside_its_target_once_it_holds_the_lock,
  the_claim_anchor_is_the_deepest_level_that_exists}` — deterministic, real
  locks, no sleeps. DCR-0021 carries two dated appended notes.

### Fixed — a run granted the publish lock checks that the marker it locked is still the marker at that path (2026-08-10)

Ticket `8792b7`, the residual `92abf5` left open on purpose. No operator-visible
behavior in the ordinary case; what changes is which concurrent sequences can
produce two "exclusive" publishers of one directory.

- `PublishLock::acquire` now compares the `dev`+`ino` of the descriptor it holds
  against the `dev`+`ino` of `<dir>/.transync-publish.lock` once the lock is
  granted, and re-locks the current marker when they differ or the path names
  nothing. Bounded at 16 reacquisitions; exhausting it is an error, not a spin.
- The rollback's own probe (`hold_marker_for_removal`) gained the same check, so
  it cannot unlink a replacement in place of the file it probed.
- **Why the remover's probe was not enough:** advisory locks report no *waiters*,
  so a run queued inside a blocking `lock()` call is invisible to the run about
  to unlink the inode it is queued on. The waiter is the only party that can see
  it, and checking there makes every unlink safe rather than only the ones a
  probe can see — which is what let ticket `40e2a5` lock a directory an
  `--out-dir` publish is about to rename away.
- **Platform:** `dev`+`ino` is `std::os::unix::fs::MetadataExt`; Windows has no
  guaranteed file identity in `std`, so the check compiles to a no-op there and
  the lock behaves exactly as before — the shape `preserve_target_permissions`
  already uses.
- **Regression guards:** `output::lock::tests::{a_waiter_granted_an_unlinked_marker_locks_the_marker_at_the_path_instead,
  marker_identity_is_the_inode_not_the_path}`. DCR-0021 carries a dated appended
  note.

### Fixed — `--out-dir` decides "is this my bundle?" from a marker it wrote, not from the file names it finds (2026-08-10)

Tickets `66339b` and OI-0036, decided together because they are one question
approached from opposite ends: how strictly `ensure_out_dir_replaceable` should
answer "is this a transync out-dir?". Name-based judgment was wrong in both
directions — too strict about transync's own residue, too loose about a user
directory that happened to hold one allow-listed name.

- **Every `--out-dir` publication now writes a `.transync-out-dir` ownership
  marker** into the tree it publishes, staged with the rest of the set so the
  target carries it from the instant it exists. Operator-visible: a new
  dot-file in every `--out-dir` target, inert to serve and to ignore.
- **A target with no marker is replaceable only when the COMPLETE published
  set is present** (`out.md`, `alignment.json`, `validation-report.json` and an
  `html/` directory). A user directory whose only entry happened to be called
  `out.md` used to qualify as a prior out-dir — moved aside and its backup
  recursively removed, with no `--force` and no prompt (OI-0036). It now
  refuses with exit 4 and a sentence naming the marker. Bundles published by
  earlier builds keep republishing without a flag: the complete set is the
  evidence. A target holding **no output at all** is never asked the ownership
  question — `mkdir out && transync translate --out-dir out` keeps working, and
  so does a target holding only transync's own markers.
- **A `<name>.tmp.<pid>` staging leftover at an `--out-dir` target's top level
  is no longer foreign** (ticket `66339b`). It is transync's own residue from a
  crashed `--output <dir>/out.md` run into the same directory, and that top
  level was the one place no guard recognized it — so transync refused to
  replace a directory over its own leftovers, and `Troubleshooting.md` had to
  document the corner. Whose pid it carries changes nothing there: the level is
  replaced whole or left untouched, so nothing is swept and nothing is promised
  about it.
- **Both marker names are recognized only on a regular file** (review fix
  round). They are the two entries both guards skip *without looking inside*,
  so a **directory** wearing one carried arbitrary nested user data past the
  check that refuses a directory named `out.md` for exactly that reason — and
  into the backup `remove_dir_all` of a replace that needed no `--force`. The
  lock name had that exposure since DCR-0021; the ownership marker added a
  second one. A directory or symlink at either name is now judged as what it
  is, and the publish refuses (exit 4) until `--force`.
- **Regression guards:** `output::tests::{a_sparse_allow_listed_subset_is_not_a_transync_out_dir,
  a_top_level_staging_temp_is_transync_residue_not_a_foreign_file,
  a_published_out_dir_carries_its_ownership_marker_and_republishes,
  a_complete_prior_output_set_republishes_without_a_marker,
  an_owned_out_dir_still_refuses_content_transync_did_not_write,
  a_directory_wearing_a_marker_name_is_not_a_marker}` and
  `cli_smoke::cli_out_dir_tolerates_a_top_level_staging_temp`.
- `contracts.md` §6, `persistence-and-files.md`, `Developer_Guide.md` and
  `Troubleshooting.md` all carry the two-question rule; the last of them stops
  documenting the corner as permanent.

### Fixed — a Responses refusal with nothing to say stops printing a sentence that ends at the colon (2026-08-10)

Ticket `594a7f`, found while landing R0003-0006. `transync-openai`-internal:
no API change, no schema change, and the classification was already right on
every shape below — a consumer reading `stable_code()` sees exactly what it saw
before.

- **One stand-in, shared by both surfaces.** `NO_REFUSAL_DETAIL` ("no refusal
  detail") moves from `client::chat` to `client::classify`, beside
  `truncate_diagnostic`, and both envelope readers use it. It was private to
  Chat since R0002-0041, so the Responses reader had none: a `refusal` segment
  carrying an empty string produced `model refused to translate: ` and stopped
  there, and two of them produced the separator alone. A refusal now reaches
  the operator in the same words whichever surface it arrived on.
- **A refusal segment with the field absent is a refusal, not a malformed
  envelope.** `ContentItem::Refusal`'s `refusal` field required a string, so
  `{"type": "refusal"}` failed `serde` and the whole envelope came back as
  `MalformedResponse` — the response was blamed for its shape when it had
  plainly said the model declined. The field is now `Option`, defaulted, and
  the *segment* is what classifies; the wording is a detail. This is the only
  behavior change in the entry: `provider_malformed_response` becomes
  `provider_model_refused` for that one shape. Chat's part has been `Option`
  for the same reason all along.
- **A detail-less refusal beside a real one contributes nothing**, not a
  dangling `; ` — empty strings are dropped before the join rather than joined,
  which is what Chat's part path already did.
- **Regression guards:**
  `client::responses::tests::responses_refusal_without_detail_names_the_stand_in`
  (all four shapes: field absent, empty string, two empty segments, empty
  beside text — each asserted as the whole sentence, plus the
  `provider_model_refused` code), and
  `client::chat::tests::chat_refusal_content_parts_are_not_dropped` now asserts
  its sentence whole from the same constant, so "both surfaces read the same"
  is a property a test can lose. `contracts.md` §1 and §7 state the stand-in on
  both surfaces instead of on Chat alone.

### Added — BREAKING: the auto-glossary preflight becomes cacheable, so a warm run calls nothing (2026-08-10)

Ticket `dca5bf`, deferred out of **DCR-0028** and now riding that record's
document-scoped seam as its second record kind (DCR-0028 carries a dated
amendment note; ADR-0021's *document-level facts ride the cache* principle is
applied, not amended). Two §0 surface additions and one `Cache` trait change
ride the sanctioned 0.4.0 window; `VALIDATION_SCHEMA_VERSION` does **not** move,
`CACHE_DISK_FORMAT_VERSION` does **not** move, and no `CacheKey` axis is added,
removed or re-encoded.

- **The last provider call a warm run paid is gone.** With `auto_glossary`
  enabled, a fully-cache-hit run still bought one round trip: the
  `Translator::extract_glossary` preflight runs *before* any batch exists, so no
  unit entry could elide it. A second run over the same document, profile,
  languages and provider namespace now makes **zero** calls of either kind.
- **BREAKING (additive): the `Cache` trait gains two more defaulted methods**,
  `get_glossary_extraction` / `put_glossary_extraction`, keyed by the new
  `transync::GlossaryExtractionKey` and carrying `transync::GlossaryExtraction`.
  The defaults store nothing, so every existing implementation keeps compiling
  and keeps exactly its pre-v0.4.0 behavior — paying the preflight on every
  enabled run. Both in-tree backends override both.
- **The key is the prompt bytes, and its absences are decisions.** Three
  namespace axes (`provider_fingerprint`, `model_id`,
  `validation_schema_version`) plus exactly one content axis, `request_hash`: a
  digest of the length-framed pair (`EXTRACTION_SYSTEM_PROMPT`, the assembled
  extraction user message), taken from the prompt builder rather than re-listed
  from the request's fields, so it cannot drift from what the model is shown. It
  therefore folds the excerpt, both language labels, the static glossary's
  source terms as `existing_terms`, the term cap and both prompt halves'
  wording. It carries no `doc_source_hash` (a truncated run's harvest is about
  the excerpt), no `source_truncated` (never reaches the model) and no
  `profile_version` (the extraction prompt carries no profile prompt body) —
  contracts.md §5a records each.
- **Only a live success is stored, and the merge always re-runs.** `Ok(None)`
  (unsupported) costs no call to rediscover and `Err(_)` must not latch a
  transient failure, so neither is filed; a replayed harvest is not re-filed.
  The stored value is the provider's answer **before** `merge_auto_glossary`,
  so a replay merges against its own profile and produces the same
  `AutoGlossaryReport` a live run produced — same status, same counts, same
  terms. The replay is named on `tracing::info` alone.
- **`DiskCache` gains a fourth record kind, `glossary`.** The log grammar is
  unchanged, so the format version stands still and an older build skips the
  kind it does not know with the usual warning — exactly the forward tolerance
  format 1 promises. Like `doc_meta`, the record is exempt from the capacity
  trim and superseded in place, never evicted.
- **Regression guards:**
  `pipeline::auto_glossary_tests::a_warm_cache_replays_the_harvest_and_the_second_run_calls_nothing`
  (zero extraction calls, zero dispatches, identical document and report row),
  `…::a_changed_static_glossary_is_a_different_extraction` (the key really folds
  `existing_terms`), `cache::disk::tests::a_glossary_harvest_survives_the_process`,
  and the cross-process `cli_cache_dir_replays_the_auto_glossary_harvest`, whose
  two runs are given stubs that would harvest *different* renderings of one term
  — so the second run's report can only match the first's by replaying.

### Fixed — a rolled-back run stops pulling a directory out from under the peer that just locked it (2026-08-10)

Ticket `92abf5`, carrying Review-0003 finding R0003-0001. CLI-internal; no API
or on-disk format changes.

- **The empty-only rollback now asks the lock, not just the directory
  listing.** `write_fileset_atomic` removes the directory levels a failed
  staging created, and a level it created and left empty holds nothing but the
  `.transync-publish.lock` marker — so the rollback unlinks that marker to be
  able to remove the level. R0002-0002 already bounded that to sole occupancy,
  which answers *has a peer published here* but not *is a peer about to*: the
  rollback runs after this run releases its own lock, which is exactly when a
  peer blocked behind it acquires that lock having written nothing yet.
- **What that cost was worse than a lost run.** Unlinking the marker there
  leaves the peer locking an unlinked inode while the next run creates a fresh
  marker at the same path and locks *that* — two runs simultaneously "the
  exclusive publisher" of one directory, interleaving their renames, which is
  the race DCR-0021 exists to prevent. The milder half, a peer that stages into
  a directory that no longer exists and exits 4, is the same window.
- **The rollback now `try_lock`s the marker before unlinking it** and holds
  that lock across the removal. A marker a peer holds — or one the probe cannot
  decide about at all — leaves the whole level alone, marker and directory
  both, because an empty directory left behind is inert and a directory removed
  out from under a peer is not. The drop-then-roll-back ordering the portable
  unlink depends on is unchanged.
- **Regression guard:**
  `output::tests::a_rollback_leaves_a_level_whose_lock_a_peer_has_taken` builds
  the race deterministically with the real lock and no sleeps, on the harness
  R0002-0002 introduced, plus
  `output::lock::tests::the_removal_probe_refuses_a_held_marker_and_an_undecidable_one`
  for the probe's two refusals. `contracts.md` §6 and
  `persistence-and-files.md` stop claiming the marker is never unlinked and
  state the one exception; DCR-0021 carries a dated appended note.
- **Still open, and now on the record:** a peer *queued* inside a blocking
  `lock()` call is invisible to any probe, so a rollback that wins the race
  back to the lock can still unlink the marker that peer is waiting on. Closing
  it needs waiter-side revalidation of the marker's inode after the lock is
  granted — ticket `8792b7`.

### Fixed — BREAKING: a row window and a whole table stop sharing one cache entry (2026-08-10)

Ticket `5f7942`, raised from dynwebserver as a question — *is `InputMode` an
intentional non-axis of `CacheKey`?* — and answered as a defect. It was
theoretical when it was filed and is not any more: `fc0304` (DCR-0026) made
`InputMode::TableRowWindow` a variant the packer actually produces, and
`f12b8b` (DCR-0028) gave cache entries a life past the process.

- **`CacheKey` gains one axis, `input_mode`** — the wire label the user prompt
  writes into each unit's object, added in the sanctioned 0.4.0 window and
  welded in the same commit by `crates/transync/tests/public_surface.rs`.
  §5a's rule has always been *if it changed the prompt bytes the model saw for
  this unit, it is identity*; the mode label is such a byte, and it was missing
  because `block_kind` implied it for every kind. That stopped being true when
  DCR-0026 shipped: a `table` unit reads `full_table_markdown` as a whole block
  and `table_row_window` as one window of an oversize table.
- **The collision was reachable, and already live in `InMemoryCache`.**
  `source_hash` cannot separate the two, because a row window's payload is by
  design a *complete* table shaped exactly like a whole table's — so a small
  table and the opening window of a big one that starts with the same row are
  byte-identical. A window inherits its parent's context, so two tables flanked
  by neighbors with the same 120-character summaries agree on `context_hash`
  too, and a window packed beside that small table agrees on all three
  batch-scoped axes. `pipeline::run_level_tests::a_row_window_and_a_whole_table_are_not_one_entry`
  builds exactly that document; before this change the two units produced one
  key **inside a single run**, and `DiskCache` would have carried the wrong hit
  between runs.
- **The axis is the label, not the variant.** `parent_block_id`,
  `window_index`, `window_count` and `language_info` never reach the model, so
  keying on them would orphan the case the cache exists for — two
  byte-identical windows of one table must still share an entry, exactly as two
  identical blocks do under the `BlockId` exclusion.
- **Cache consequence: every existing entry re-keys once.** An `InMemoryCache`
  is run-scoped, so nothing is observable there. A `DiskCache` log written by
  an earlier build has `entry` records with no `input_mode` field; replay
  cannot deserialize them and **skips them with the usual warning** (the log is
  not corrupt and `open` does not fail), so the first run after upgrading
  re-dispatches those units and re-files them under the new key.
  `CACHE_DISK_FORMAT_VERSION` does **not** move — the log grammar is unchanged,
  only a key field set — and `VALIDATION_SCHEMA_VERSION` does not move either:
  none of its three documented triggers fires.
- Consumers implementing `Cache` construct and destructure `CacheKey`
  exhaustively by policy, so this is a compile break with a one-line fix.

### Added — BREAKING: a second provider crate, and the context window gets a name (2026-08-10)

Ticket `bda471`, the last of the six commissioned post-0.3.0 roadmap items,
designed by **DCR-0029** (with a dated ADR-0002 amendment) and shipped as
slices SL-115..SL-119. One `TranslatorError` variant rides the sanctioned
0.4.0 window; everything else is additive, and the core trait does not move —
which is the point of the exercise.

- **`transync-anthropic` — the second in-tree `Translator`**, over the
  Anthropic **Messages API** (`POST {base_url}/v1/messages`). It is the first
  exercise of ADR-0002's claim that a provider lands as a sibling crate without
  touching the core trait, and the claim held: the crate reuses the shared
  tier-(b) surface (`transync::llm::prompt`, `profile::render_prompt_body`)
  unchanged, and the only thing that reached the seam was one taxonomy variant.
  It **publishes** (sixth published member); `contracts.md` gains a new **§8**,
  while **§0 and `public_surface.rs` are untouched** — the facade does not
  re-export provider crates, so a consumer names this crate directly.
- **Construction-time configuration identity, one axis smaller than §7's.**
  `try_new` / `new` / `from_env` follow §7's rules verbatim (empty-or-whitespace
  key, blank-or-padded model, non-`http(s)`/host-less base URL; the
  one-per-process cleartext-`http` warning emitted from the single constructor
  the other two delegate to). What is absent is the surface axis: this provider
  has **one** endpoint, so there is no `Api` type, no surface environment
  variable, no `with_api`. The crate's entire environment-read surface is
  `ANTHROPIC_API_KEY`, `TRANSYNC_ANTHROPIC_MODEL` (default `claude-opus-5`) and
  `TRANSYNC_ANTHROPIC_BASE_URL`, all read inside `from_env` and nowhere else.
  `fingerprint()` is `("anthropic", [model, base, effort])`; the
  `anthropic-version` protocol header is deliberately **not** an axis, because
  it is a crate-wide constant rather than a per-instance value.
  `tokenizer_hint()` is a flat `Cl100kBase` — the stated approximation the
  trait docs ask a non-OpenAI provider for, rather than `None`, which would
  send core back to guessing from an advisory label.
- **The output ceiling is required, so one is always sent.** `max_tokens` has
  no omit-and-default path on this API. `[batching].target_output_tokens` rides
  out when set; `DEFAULT_MAX_OUTPUT_TOKENS` (16 384) rides out when it is not.
  The consequence to plan for: **`stop_reason: "max_tokens"` can fire on a run
  whose operator configured nothing**, so the diagnostic names the value that
  was sent, its origin, and the fact that on this API `max_tokens` caps
  thinking and answer *together* — a short batch can exhaust a ceiling for
  reasons unrelated to the answer's length. **No `thinking` parameter is ever
  sent**, in either request body: it is the only choice valid across the
  provider's whole current model family. `with_effort`
  (`output_config.effort`) is the sanctioned depth knob.
- **Structured outputs are unconditional, in the provider's narrower dialect.**
  Every request carries the shared schema object under `output_config.format`;
  there is no prompt-coaxed-JSON mode. A deterministic schema-profile pass
  *subtracts* the keyword class this dialect rejects (array/numeric/string
  count constraints) and reshapes nothing; the dropped `max_terms` cap is
  enforced **post-parse** instead, so the `GlossaryExtractionRequest` contract
  holds regardless of what the provider did with the keyword. Model support is
  the provider's authority: a model that rejects `output_config` answers HTTP
  400, which is already precise, and the adapter maintains no allowlist to rot.
- **BREAKING: `TranslatorError::ContextWindowExceeded`, stable code
  `provider_context_window_exceeded`.** DCR-0029 put this to the owner as an
  open fork and it resolved to a named variant.
  `model_context_window_exceeded` is terminal *and operator-actionable*, and
  its remediation is the **batching** configuration — so reporting it as
  `OutputCeilingExhausted` would name the wrong knob (the actively-harmful
  class DCR-0023 exists to remove) and leaving it in `Other` would name no knob
  at all. It is a **variant addition on a `#[non_exhaustive]` enum**, so a
  consumer keeping the mandated wildcard arm keeps compiling; what changes is
  the vocabulary it may now see, and a consumer with a stable-code table should
  add the row. The stable-code set goes from nineteen to **twenty**. The OpenAI
  adapter is deliberately unchanged — its context overflows arrive as HTTP 400
  and keep their status-based `ProviderRejected` classification, which is
  already honest there.
- **The stop-reason table is mapped on this provider's own evidence.**
  `stop_reason` is read **before** any content, because a refusal on this API
  can arrive with an *empty* `content` array. `stop_details` is read only under
  a refusal (the only stop reason that populates it), and its `category` splits
  the safety layer's `ContentFiltered` from the model's own `ModelRefused` — a
  stated heuristic, since both are terminal and a misdrawn boundary costs a
  name rather than a behavior. `stop_sequence` / `tool_use` / `pause_turn` take
  the sanctioned catch-all naming themselves, because a request this adapter
  shapes cannot elicit them and *unknown* is the honest answer. On the HTTP
  side, **529 `overloaded_error` is classified by name** — it sits outside the
  5xx band, so a range-only table would catch this provider's overload signal
  by luck. `retry-after` is honored in both RFC 7231 forms.
- **Cancellation (DCR-0024) is a biased race on both trait methods**, so an
  already-cancelled run never issues the request at all. **Bounded retry stays
  core's**: the adapter runs no retry loop of its own, because an inner loop
  would multiply core's budgets (ADR-0009).
- **The whole stack is proven keyless.** An offline end-to-end test drives
  `transync::translate` over the adapter against a loopback server that is a
  *translator, not a fixture* — it reads the request the adapter actually sent
  and answers one result per unit — so a drift in prompt assembly, schema
  profiling, or unit-id handling fails there instead of passing as a test about
  a stale constant.
- **The live half is written, gated, and never executed.** `tests/live_smoke.rs`
  is `#[ignore]`d *and* requires `TRANSYNC_LIVE_SMOKE=1` plus a non-empty
  `ANTHROPIC_API_KEY`, with the gate predicate unit-tested offline so it cannot
  rot into vacuity; `scripts/smoke-live-gate.sh` grows an `anthropic` leg
  (`chat|responses|anthropic|all`), each leg demanding its own key up front.
  **There is no `ANTHROPIC_API_KEY` in the development environment**, so no
  request has ever been made to the real endpoint by this repository — recorded
  as a standing gap in `status.md`, to be run and dated once a key exists.

### Added — BREAKING: the cache outlives the process, and learns document metadata (2026-08-09)

Ticket `f12b8b`, the fifth commissioned post-0.3.0 roadmap item, designed by
**DCR-0028** and **ADR-0021** and shipped as slices SL-110..SL-114. Two §0
surface additions and one `Cache` trait change ride the sanctioned 0.4.0
window; `VALIDATION_SCHEMA_VERSION` does **not** move, and no cache key axis is
added, removed or re-encoded — the 0.3.0 identity contract is this design's
input, not its subject.

- **`transync::DiskCache` — a cache that survives the process.** One
  operator-chosen directory holds one `transync-cache.jsonl`: a JSON-lines
  append log with a self-describing header and three record kinds (`entry`,
  `evict`, `doc_meta`), replayed once at `open` into an in-memory index and
  appended to thereafter. A `get` is still a map lookup and a `put` is one
  buffered append plus a flush, so nothing about the run's cost model changes.
  An entry's on-disk identity is its **full serialized axis set** — no digest
  of the key exists anywhere in the format, so the store adds no collision
  surface beyond the axes `CacheKey` already carries (ADR-0020 governs; the
  injectivity discipline holds by construction, since JSON field names are
  presence markers and string framing is the length delimiter).
- **Recovery is loud, safe and forward-only, and can never fail a run.** A torn
  tail is discarded and the file truncated to the last complete record; an
  unreadable line or unknown record type is skipped (which is how format 1
  stays forward-tolerant to additive record kinds); a missing or foreign header
  rotates the file aside as `transync-cache.jsonl.unreadable-<unix-ts>` —
  preserved, never deleted — and the cache starts empty. There is no migration
  path between format versions and no checksum: ADR-0015's hit revalidation
  already re-runs every per-unit layer on reuse, so a corrupted-but-parseable
  payload is a re-translation rather than a corrupted document.
- **`transync::DiskCacheOptions` — capacity and compaction, at open only.**
  `max_bytes` defaults to 1 GiB, `max_entries` to `None`. Over-budget logs drop
  entries **oldest-written-first** (not LRU: persisting read recency would make
  every `get` a disk write), and the log is compacted when a trim happened or
  when dead records have caught up with the live set. Compaction goes through a
  temp file plus one `rename`, so a crash leaves one complete file and never a
  hybrid. `doc_meta` records are exempt from the trim.
- **Durability and concurrency are contract (contracts.md §1).** Every write
  flushes, so entries survive process exit; `fsync` is explicitly out of
  contract. **One writer per cache directory** — concurrent processes sharing
  one are unsupported, with no lock file; the violation cost is bounded to lost
  entries, never corrupt output and never a failed run.
- **BREAKING (additive): the `Cache` trait gains two defaulted document-level
  methods**, `get_document_meta` / `put_document_meta`, keyed by the new
  `transync::DocumentMetaKey` and carrying `transync::DocumentMeta`. Every
  existing implementation keeps compiling and keeps exactly its pre-v0.4.0
  behavior, because the defaults store nothing. This closes **OI-0017's last
  item**: a fully-cache-hit `--source-language auto` run used to report
  `detected_source_language: null`, having made the detection, paid for it, and
  thrown it away with the run. The pipeline now writes the record from a live
  qualifying envelope and replays it **only** on a run that dispatched zero
  provider batches — replay compensates for the calls the cache elided, it
  never overrides what a live provider said or declined to say. The meta key
  deliberately carries no prompt-identity axis; contracts.md §5a records why.
  SCN-10's partial-resume guarantee lost its one carve-out with it: a resumed
  run's alignment map is byte-identical, detected language included.
- **`transync translate --cache-dir <path>`** opens that cache. Absent, the run
  behaves exactly as before. An unopenable directory is warned about once and
  the run continues on a fresh in-memory cache — at the CLI boundary a cache is
  an accelerator, and an unwritable path must not kill a translation the user
  asked for. There is no profile `[cache]` table: where the cache lives is an
  invocation concern, and profiles travel between machines.
- **Three Review-0003 cache findings are answered rather than deferred.**
  `InMemoryCache` stays unbounded **by scoping** — its lifetime is one run or
  one session, and the backend built for a longer one is `DiskCache`
  (R0003-0048); its map now stores `Arc<UnitResult>` so a hit's deep copy
  happens outside the lock instead of queueing every concurrent batch behind
  one memcpy (R0003-0049); and `CacheKey` keeps plain `String` axes while disk
  records stay **self-contained** rather than interned, because an indirection
  record lost to tail truncation would orphan every entry referencing it
  (R0003-0051). Bulk trait methods were considered and declined (R0003-0050):
  both shipped backends are memory-index lookups, and defaulted bulk methods
  remain addable in any future release without a breaking window.

### Added — BREAKING: `transync serve` binds a socket and serves the bundle (2026-08-09)

Ticket `b791d6`, the fourth commissioned post-0.3.0 roadmap item. No DCR: the
owner un-deferred `STUB-061` on 2026-08-06. Rust library API untouched — this
is `transync-cli` alone, and `contracts.md` §0's curated surface and the
`public_surface.rs` weld are both unchanged.

- **The deferred-stub contract is retired, which is the breaking part.** From
  SL-13 until now, `serve` parsed `--rendered` / `--port` / `--bind`, printed
  the address it *would* have bound, and exited `5` — a documented contract in
  `contracts.md` §6, restated in the Developer Guide's CLI reference and in the
  module doc. It binds now. A script that branched on exit `5` to detect the
  deferral is branching on a **bind failure** instead: `serve` uses four exit
  codes — `0` when a signal stopped a running server, `1` for an argument error
  (including a `--bind` value that is not an IP address, which is newly
  rejected by clap rather than accepted as a string), `2` when `--rendered` is
  not a readable directory, `5` when the address could not be bound.
- **Serving is all it does.** `GET` and `HEAD` only, `405` with `Allow:
  GET, HEAD` otherwise. No upload, no directory listing, no execution, no
  proxying. A target ending in `/` resolves to that directory's `index.html`;
  a directory without one is `404`, and the files inside it stay reachable by
  name.
- **Path confinement is two layers, because either alone is a known hole.**
  The request target is split on `/` and each segment percent-decoded *on its
  own*: a segment decoding to `.` or `..` is `403`, one decoding to a separator
  or a NUL (`%2f`, `%5c`, `%00`) is `400`, because that decode was inventing
  structure the split had already passed. The resolved path is then
  **canonicalized** and refused unless the canonical result is still the
  canonical root or beneath it — the only check that can see a symlink pointing
  out of the bundle. Refusals reject; nothing is clamped back inside.
- **Loopback by default.** `--bind` defaults to `127.0.0.1`; anything else
  takes the flag and prints a warning naming the address and the directory it
  just published. `--port 0` takes an OS-assigned port and the announced
  address comes from `local_addr()`, so it is the kernel's answer rather than
  an echo of the argument.
- **A fixed content-type table, never sniffing** — the bundle's file set plus
  what the wasm demo needs (`.wasm` is not optional: `instantiateStreaming`
  refuses any other type), with `application/octet-stream` for everything
  unnamed. Every response carries `X-Content-Type-Options: nosniff`,
  `Cache-Control: no-store` and `Connection: close`; one request per
  connection means no message boundary is ever inferred. Ctrl-C and `SIGTERM`
  stop the accept loop, drain for two seconds, and exit `0`.
- **No new dependency.** Four more tokio features (`net`, `io-util`, `fs`,
  `signal`) on `transync-cli` alone, so no library member acquires a socket or
  a signal handler; `skeleton-plan.md` §2's locked invariant — no HTTP crate in
  the library members — is untouched. The lockfile gains only
  `signal-hook-registry` and `errno`, tokio's own signal backend.
- **Every path that reached for `python3 -m http.server` now reaches for this
  server**, and Python leaves the Quick Start's prerequisites table:
  `web/SMOKE.md`, `scripts/smoke-live.sh` (whose Python-3 interpreter probe,
  R0008-0053, retired with the dependency it guarded) and
  `scripts/test-browser.sh`, which builds the CLI once and hands the binary to
  Playwright as the suite's `webServer` — so the 30-test SCN-13 + wasm browser
  suite is now evidence for the shipped server. `crates/transync-cli/tests/
  serve_static.rs` replaces `serve_deferred.rs` and pins the refusals over a
  raw `TcpStream`, because a convenience HTTP client normalizes `/../secret`
  before sending and would have measured itself.

### Changed — the browser sync engine reacts to reflow, and its pairing rule is stated (2026-08-09)

Ticket `d3acc3`, the third commissioned post-0.3.0 roadmap item. No DCR: the
owner settled its three decisions on 2026-08-06 under delegation. Rust API
untouched — this is `web/js/sync.js` and its byte-identical embedded CLI twin
(`crates/transync-cli/web/sync.js`), which move in one commit under
`sync_js_drift.rs`.

- **Reflow no longer needs the caller** (OI-0024 item 1). `mountSync` observes
  three signals — a `ResizeObserver` on **both** panes, `document.fonts.ready`,
  and `load`/`error` on `<img>` elements inside either pane (capture phase;
  neither event bubbles) — and on any of them re-collects both anchor sets and
  re-runs the *last driving* pane's scroll handler, coalesced into one
  animation frame. Geometry was always read per frame, so what this fixes is
  not stale offsets: it is the two panes describing different places, with
  nothing to reconcile them until the reader happened to scroll again. The
  docstring's old answer — destroy and re-mount — remains correct for an actual
  HTML **replacement**, which is not a reflow. `controller.refresh()` requests
  the same recompute for a layout change no observer reports.
- **Anchors pair by identical `data-sync-id`, normatively** (schema 1.x). The
  engine always did this; it is now a stated invariant (ADR-0001 amendment,
  `contracts.md` §3) rather than an incidental property of the emitter, with
  the alignment map's `source_block_id` / `target_block_id` indirection
  **reserved** for a future divergence revision. Consequence for a third-party
  map producer: a row whose `target_block_id` is a non-empty string different
  from its `source_block_id` is now **refused** with a console reason instead
  of mounting into panes that render and never sync — the forward-minor policy
  applies unchanged, so the same row in a newer-minor map is warned about and
  paired by the source id. Every map transync emits already satisfies this
  (`build_alignment_map` writes one `BlockId` into both fields), so no shipped
  path changes behavior.
- **Fixed (Review 0003, `R0003-0002`):** the map/DOM drift warning resolved
  `target_block_id` while the scroll handler resolved `source_block_id`, so a
  schema-valid map with distinct directional ids warned accurately and then
  could not synchronize in either direction. Both paths now read the id the
  engine actually keys on, and the gate above keeps the contradicting map from
  reaching them.
- **Single-file packaging is re-affirmed, not split** (OI-0015). The byte-twin
  discipline and the no-build, framework-free `web/` tree outweigh an
  engine-vs-helpers source boundary; the module's doc-comment carries the
  reasoning.
- Browser suite 27 → 30 (`web/tests/engine.spec.js` `g`, `h`, `i`).

### Added — BREAKING: batches become section-coherent, and the glossary section scope becomes real (2026-08-09)

Ticket `43cfb4`, designed in **DCR-0027** and shipped as slices SL-106..SL-109.
Two halves, one commission because the second was gated on the first: the
batcher packed units in document order until a budget bound, so batches
straddled `##` / `###` boundaries freely; and `[[glossary]].scope = "section"`
was rejected at load, because a section-scoped term rendered into every batch's
prompt would steer the whole document. ADR-0014's own revisit trigger — "a
reliable unit→section mapping at batch-assembly time" — fires here and both
carry a dated note (ADR-0014, and the EXT-2026-07 baseline record).

- **A batch never straddles a section boundary.** `unit::build_batches`
  partitions the unit list at every heading — a heading belongs to the section
  it *opens* — and packs each section on its own with the unchanged greedy
  dual-budget packer. The single sanctioned exception splits a section *across*
  batches and never merges two *into* one: a section too large for one batch
  yields several, all inside it. Nothing is reordered, batch ids stay one
  1..N document-order sequence, and a document with no heading packs
  bit-identically to before. Whole-section coalescing is deliberately not done
  (DCR-0027 OQ-A records the request-count trade for the owner).
- **`[[glossary]].sections`** names where a `scope = "section"` entry applies:
  heading-text selectors matched — trimmed, case-folded, levels ignored —
  against every heading enclosing a section, **including the one that opens
  it**, so a term scoped to `"Installation"` also applies inside
  `### Windows` beneath it. An empty or repeated selector is named and dropped;
  a `section` entry with no usable selector is dropped; a `sections` list on a
  `global` entry is named and cleared.
- **Claimed-once relaxes to once per place the claims can meet.** Two `global`
  entries on one term keep first-wins; two `section` entries are legal while
  their selectors are disjoint; a `global` and a `section` entry on one term
  both survive — that pair is the override the scope exists for, with the
  section-scoped one winning inside its sections, silently. The auto-glossary
  merge follows: an extracted term is dropped only against a static *global*
  claim.
- **One compiled prompt per cohort.** A distinct effective glossary is compiled
  once per run and shared by every section with it; each batch carries its
  section's compiled profile and entry list. Tier-(b) providers need no change
  — they already read `batch.profile.prompt_body`. Scope and selectors are
  never rendered: filtering replaces annotation, which is what ADR-0014's
  amendment demanded.
- **BREAKING: `GlossaryEntry` gains a public `sections: Vec<String>` field**, so
  struct literals must name it (`Default` is unchanged for deserialization: the
  key is `#[serde(default)]`).
- **BREAKING: `ProfileError::Unsupported` and `ProfileMetadata::ensure_supported`
  are removed, and `unit::build_batches` is infallible** (`Vec<TranslationBatch>`
  instead of `Result<_, ProfileError>`). The variant's only carrier was the
  retired scope rejection, and `build_batches` was fallible for that gate alone.
  The three gates retired together, as the backlog entry required; the loader's
  `MissingField` / `Malformed` are now the only profile errors.
- **Cache identity follows the prompt bytes.** `profile_prompt_hash` and
  `glossary_hash` move from run scope to **batch** scope, derived from what the
  batch carries rather than from a second run of the section filter. `CacheKey`
  does not change shape and `VALIDATION_SCHEMA_VERSION` does not move:
  a profile with no section-scoped entry has one cohort and produces the
  digests the run-level computation produced, so nothing is orphaned, and a
  profile *with* one could not previously load at all.
- No facade row moves — both changed items are §0 *type* rows, and neither a
  removed variant nor a removed method is a row — but `public_surface.rs` was
  re-run in the same commit as the §2 / §5 / §5a contract edits. SCN-09 and
  SCN-10 gain the two acceptance assertions; `docs/backlog.md` retires its
  Type-3 `glossary-section-scope` entry.

### Added — BREAKING: an oversize table splits into header-carrying row windows instead of aborting the run (2026-08-09)

Ticket `fc0304` (the STUB-017 cluster), designed in **DCR-0026** and shipped as
slices SL-100..SL-105. Output-aware packing (DCR-0012) can shrink a batch but
not a *unit*, so a single block whose estimated response exceeded the output
ceiling was packed alone, flagged by the preflight, and aborted the whole run
at the provider (ADR-0017) — with raising `--target-output-tokens` the only
remedy. For the one kind whose sub-structure the application already owns and
validates, that is over.

- **The split.** When `[constraints].default_table_strategy` resolves to
  `row-window-first` **and** the run has an output ceiling,
  `unit::build_batches` replaces every table unit whose estimated response
  exceeds the effective ceiling with **row windows**: each a complete GFM table
  carrying the source header, its delimiter row, and one contiguous run of body
  rows (architectural invariant 3's sanctioned shape — never an isolated cell,
  never a headerless fragment). It happens once, before round one,
  deterministically. Nothing splits without a ceiling, under `whole-block`, or
  for a table that fits.
- **A split is packing, not retrying.** It consumes no retry budget and changes
  no in-flight unit; from birth a window is an ordinary unit whose scope is
  fixed for the run, retried verbatim against its own
  `max_per_unit_validation_retries`. There is no re-split on any signal, so
  ADR-0009's verbatim-resubmission rule is untouched rather than amended, and
  ADR-0017's abort is unchanged for everything not split (both ADRs carry a
  dated note).
- **Reassembly.** `transync_syntax::regen::regenerate_table` stops being
  STUB-017's identity function: it takes `&[&str]` and splices window 0 whole
  plus every later window's rows below its delimiter, with `split_table_rows`
  as its comrak-verified source-side inverse. `pipeline::merge` runs between
  aggregation and regeneration, re-inspects the merged table against the
  **source** block's columns, alignment and row total, and replaces the window
  entries with one parent entry — so a window id never reaches
  `regen::regenerate`, the alignment map, or the DOM. The alignment map still
  carries exactly one row per source block and its schema version does not move.
- **A failed window costs its rows, not the table.** It contributes its own
  source rows and the block is marked `partially_translated`, with the failed
  windows named on the report's warnings channel; only an all-failed table is
  `fallback_source`. A merged table that lost the source's shape although every
  window passed its own layers is an engine fault and takes the DCR-0016
  direct-fallback shape (`rejected_by` and `rejection_reason` both `None`), with
  every window's cache entry evicted.
- **BREAKING: `InputMode::TableRowWindow`'s fields are redefined** to
  `{ parent_block_id, window_index, window_count }`. The reserved
  `header_markdown` / `rows_markdown` / `window_index` shape was never produced
  and described a payload split the design does not use — the payload IS the
  mini-table. A `Translator` needs no code for the variant beyond what it
  already does for a table; the wire label `"table_row_window"` is unchanged and
  the payload contract is now stated in `contracts.md` §1.
- **BREAKING: `[batching].max_split_retries` is removed** — the field, its
  `default.toml` line, its key allowlist entry and its contracts rows. Under a
  deterministic packing-time split there is no "split retry" to bound, and a
  knob whose name promises retry semantics must not silently become a window
  count. An old profile carrying it gets the ordinary unknown-key load warning.
- **The shipped default changes.** `[constraints].default_table_strategy` stops
  being advisory and ships as `"row-window-first"`; the "reserved" load warning
  is retired and the unknown-value warning stays. The only runs whose behavior
  changes are runs that today abort. `--table-strategy
  <whole-block|row-window-first>` joins the DCR-0012 batching flags under the
  same **flag > profile > built-in default** rule, and the output-budget
  preflight's remediation line names it when a flagged unit is a whole table.
- **`VALIDATION_REPORT_SCHEMA_VERSION` → `1.1.0`** (additive). Windows keep
  their own `per_unit` rows — real dispatches with real attempts — listed
  directly under their parent's, under ids `<parent-block-id>.wNN`. No key moved
  and every field keeps its meaning, but a `unit_id` that names no block is a
  new row shape: consumers must not assume every `unit_id` in the report has a
  row in the alignment map. `VALIDATION_SCHEMA_VERSION` (the cache axis) does
  **not** move — no existing payload's semantics change and whole-block entries
  stay valid.
- **SCN-03 is live-verified.** `scn_03_table_large.rs` sets a ceiling the
  200-row fixture cannot fit and asserts what was *dispatched* — two or more
  header-carrying windows — as well as the 200x4 round-trip in source order,
  plus a negative path in which one window exhausts its retries. The scenario
  matrix's "stub-verified-only" footnote and the `stub-manifest.md` STUB-017
  row are both closed.
- **One definition of "oversize".** `batch::effective_output_target`,
  `batch::estimate_payload_output_tokens` and `budget::output_ceiling` are
  shared by the packer, the at-risk preflight and the splitter, so the three
  cannot disagree about what does not fit.
- No facade row moves: `regen` is a tier (c) internal and §0 lists
  `ProfileBatching` as a type rather than by field, so `public_surface.rs` is
  unchanged. `contracts.md` §1, §2, §3a, §5 and §6 move with the code.

### Added — BREAKING: a run can be cancelled, and a cancelled run answers `Err` (2026-08-08)

Ticket `43331a`, filed by the dynwebserver maintainer. Recorded in
**DCR-0024**, which **supersedes DCR-0009's YAGNI deferral** of
deadline/cancellation on `Translator` — that deferral named its own revisit
condition ("a long-running service consumer appearing") and the condition
fired. `docs/decisions/0002-http-free-core-with-translator-trait.md` gained a
dated amendment; `docs/architecture/contracts.md` §0, §1 and a new **§5b** move
with the code, and `crates/transync/tests/public_surface.rs` moves in the same
commit as §0.

- **`TranslateOptions.cancel: Option<CancellationToken>`.** `None` is the
  default and reproduces the old behavior exactly. The token is
  `tokio_util::sync::CancellationToken`, re-exported as
  `transync::CancellationToken` — the only foreign type in the curated surface,
  taken deliberately so a daemon can derive a per-job token from a process-wide
  one (`shutdown.child_token()`) instead of converting between look-alikes.
- **A cancelled run returns `TransyncError::Cancelled`** (stable code
  `cancelled`), **never a partial `TranslationOutput`.** This is the design
  decision, and it is ADR-0017's reasoning applied unchanged: an untranslated
  island in a "successful" document is not visually distinguished in raw
  Markdown, so a consumer can ship it without noticing. `fallback_source` also
  *means* "attempted and could not be translated" — a cancelled run's unreached
  blocks were never attempted, and conflating the two would undo in the
  alignment map exactly what the previous entry undid in `TranslatorError`.
- **The paid-for progress lives in the cache, not the return value.** Units
  accepted before the cancellation were already written to the `Cache` as they
  were accepted, so `translate_with_cache` with a caller-owned cache
  re-dispatches only the remainder. `translate` builds a throwaway cache per
  call and keeps nothing; its rustdoc now says so.
- **BREAKING: `Translator::translate_batch` and `Translator::extract_glossary`
  take `cancel: &CancellationToken`.** Every implementor moves — in-tree that
  is the shipped adapter, both test stubs and ~40 test doubles. Honoring the
  token is a SHOULD; `TranslatorError::Cancelled` (stable code
  `provider_cancelled`) is the answer when an implementation does. A defaulted
  `translate_batch_cancellable` would have been additive and was rejected: an
  implementor overriding only the old method would be silently uncancellable.
- **The pipeline honors it at seven points** — run entry, after the
  auto-glossary preflight, each batch's entry, each dispatch round's head,
  around every provider call, around every transport-backoff sleep, and once
  more after the fan-out settles. Provider calls and backoff sleeps are
  `tokio::select!` with `biased;`, so an already-cancelled run issues no request
  at all and does not sit out a capped 30 s `Retry-After`.
- **A `Translator` that ignores the argument is still cancelled**, because the
  pipeline drops its future — which aborts an in-flight `reqwest` request. The
  parameter exists for implementations whose work is *not* drop-cancellable and
  for adapters driven directly.
- **The last checkpoint is the fan-out.** Regeneration, alignment and rendering
  are pure CPU with no I/O and are not interruptible, so a token that fires
  during them is not observed and the run returns `Ok`. Stated in §5b rather
  than left to be discovered.
- **The preflight asymmetry.** An `extract_glossary` error still only degrades
  the run; a *cancellation* during that same call aborts it. Without the
  re-check, cancelling during the one call a run makes before batching would
  have meant "proceed and translate the whole document".
- **The `stable_code()` vocabulary grows from seventeen to nineteen**:
  `cancelled` (engine) and `provider_cancelled` (a `Translator` call). Both are
  additions, so the append-only rule is unbroken.

### Changed — BREAKING: `TranslatorError` names five terminal causes, and `stable_code()` reaches them (2026-08-08)

Ticket `1a85f3`, filed by the dynwebserver maintainer after integrating 0.3.0.
Recorded in **DCR-0023**; `docs/decisions/0002-http-free-core-with-translator-trait.md`
gained a dated amendment; `docs/architecture/contracts.md` §1 and §7 move with
the code. The §0 table is **untouched** — no item was added or removed, only
variants and a method on types §0 already lists — so
`crates/transync/tests/public_surface.rs` needed no change.

- **Five causes stopped sharing `TranslatorError::Other(String)`.** A census of
  the shipped adapter found nine construction expressions, eight of them
  collapsing to five causes (the ninth is an unreachable internal case):
  `ContentFiltered`, `OutputCeilingExhausted`, `ModelRefused`,
  `ResponseTooLarge`, and `ProviderRejected { status: Option<u16>, message }`.
  Each is now its own variant. `Other` stays as the permanent catch-all — a
  closed enum here would make every future provider quirk a breaking change,
  and `transync-anthropic` is commissioned.
- **A refusal is one cause with three shapes.** `message.refusal`, a Chat
  `refusal` content part and a Responses `refusal` segment all reach
  `ModelRefused`, so a consumer never has to know which surface a run
  dispatched on.
- **Responses `incomplete` is classified by its `reason`.** Mapping the whole
  status onto the ceiling would have mislabeled
  `incomplete_details.reason: "content_filter"` — and the two remediations are
  opposite (raise `[batching].target_output_tokens` / no knob exists). A reason
  this adapter does not know, an absent one included, stays `Other`.
- **A rejected request carries its HTTP status as a number.** `Some(404)` is a
  model name that does not exist — an operator fault no retry or document edit
  fixes — and it was previously indistinguishable from a 400 without matching
  the message string.
- **`TranslatorError::stable_code()` is new, and `TransyncError::stable_code()`
  delegates to it.** The inter-process vocabulary grows from seven codes to
  **seventeen**: the six engine-side codes are unchanged, and eleven
  provider-side codes replace the single `provider_error` the whole family used
  to return. Both matches stay exhaustive with no wildcard arm, so a new
  variant cannot land without a code.
- **`provider_error` narrows** to `TranslatorError::Other` — the unclassified
  case — instead of meaning "any provider failure". This is the one sanctioned
  exception to contracts.md §1's append-only rule and the reason the change
  rides 0.4.0.
- **Why this was worth a breaking change.** Every one of the five fails
  identically on a verbatim resubmission, which is transync's own stated reason
  for calling them terminal. But `Other` is also a consumer's
  "I-don't-know-what-this-is" bucket, and the honest handling of unknown is
  *retry* — so the type was inviting readers into an unbounded loop of paid,
  guaranteed-identical failures. dynweb had flattened the bucket into a
  retryable "the translation service is unavailable — try again" for exactly
  that reason.
- **Migration.** Matches keep compiling (`#[non_exhaustive]` already required a
  wildcard), so this is a review task rather than a build error.
  **dynwebserver**'s `classify_translator` sends the five causes to its
  trailing `other => TranslateError::Local(…)` arm — a provider stop reported
  to a reader as a *local* fault — until it adds arms. **resp-translator**'s
  `scn12_provider_failure_maps_to_stable_code_and_arms_cooldown` asserts
  `provider_error` for a `TranslatorError::Network`, which now returns
  `provider_network`; its listener's hardcoded code table moves with it.
- **New gate.** `crates/transync/tests/error_taxonomy.rs` (7 tests) welds
  variant-to-code distinctness, the delegation, and the contracts.md §1
  vocabulary table — scraped and diffed in both directions, so a code without a
  documented row (or a row without a code) is a red test.

### Fixed — the wasm build gates the module before it installs it (2026-08-09)

Review 0003, batch B7 (`R0003-0007`, `R0003-0008`, `R0003-0081`, `R0003-0082`,
`R0003-0083`). `scripts/build-wasm.sh` copied its two artifacts into `web/wasm`
and *then* measured them, so the size budget it exists to enforce was checked
on a module that was already installed. Four of the five findings are one
change: publish through a staging directory. **Developer tooling only** — no
library, CLI, or browser source moves, `web/wasm/` is a gitignored build
artifact, and the curated surface is untouched.

- **Every gate now runs before anything is published (`R0003-0008`).** The
  wasm-pack build, the `wasm-opt` pass, the raw/gzip measurement and both
  budget checks all happen in staging. A breach exits 1 with the previously
  good module still in place and says so (`… — web/wasm left untouched`);
  before, the rejected module had already replaced it.
- **Glue and module land as one pair (`R0003-0007`).** They were two
  independent `cp`s into the live directory, so a browser tab or a parallel
  Playwright run could observe new JavaScript against an old binary. The staged
  tree is published by renaming a *directory*. Crash-safe rather than atomic,
  and for the reason `publish_out_dir` already documents for `--out-dir`:
  `rename` cannot replace a non-empty directory, so replacing an existing
  `web/wasm` is two renames with a window in which it is absent. A reader in
  that window finds nothing — never a mixed pair.
- **`web/wasm` holds exactly what this build emitted (`R0003-0081`).** The old
  `cp` updated two names and never replaced the directory as a set, so an
  artifact a previous build wrote under a name the current one no longer
  produces survived indefinitely and could be copied into the browser fixture.
- **Concurrent builds no longer collide (`R0003-0083`).** The staging directory
  was a fixed path wiped at start, so two runs — this repo does run concurrent
  agent sessions — deleted each other's half-built module. Each run now stages
  under a per-run `mktemp -d` name and reclaims only trees carrying its own
  pid, the rule `crates/transync-cli/src/output.rs` publishes by. The final
  swap is serialized on a `mkdir` lock, because `mv` moves a directory *into*
  an existing destination instead of replacing it: unserialized, two swaps
  could nest one run's staging inside the other's published `web/wasm`.
  `TRANSYNC_WASM_STAGING` now names the staging *root* rather than the staging
  directory; pointing it at a valuable directory is refused exactly as before.
- **The `wasm-opt` version check anchors on `version <n>` (`R0003-0082`).** It
  took the first digit sequence anywhere in `--version`, so a vendor banner
  leading with a year (`binaryen 2024 … version 117`) read as 2024 and passed
  the `>= 121` gate. It now reads 117 and rejects, and an output with no such
  token is echoed verbatim in the diagnostic. Diagnostics, not a shipping hole
  — a misfire was already caught downstream by `wasm-opt` rejecting the module
  or by the size gate.

### Fixed — the browser demo keeps the promises it makes about failing (2026-08-09)

Review 0003, batch B6 (`R0003-0003`, `R0003-0068`, `R0003-0069`, `R0003-0070`,
`R0003-0071`, `R0003-0076`, `R0003-0086`, `R0003-0090`). Six ways the demo and
the sync engine fell through their own fail-closed presentation — five of them
ending in an uncaught exception with the page pinned at
`data-demo-state="booting"`, which is the one failure the demo reported by
reporting nothing. **No curated-surface change**: everything here is
`web/js/*.js`, so contracts.md §0 and `crates/transync/tests/public_surface.rs`
are untouched. `docs/architecture/contracts.md` §3 and §4a move with the code
(the engine's refusal set and the layout precondition are consumer contracts).
`web/js/sync.js` and the CLI-embedded `crates/transync-cli/web/sync.js` move
byte-identically, as `sync_js_drift.rs` requires.

- **A rejected edit no longer poisons the edit model (`R0003-0003`).** The
  typed payload was written into `state.payloads` *before* the rebuild and was
  taken back out by neither failure path, and since every rebuild resubmits the
  whole payload map, one rejected payload then re-failed the rebuild of every
  later edit — of any block — until the user happened to return to the block
  that caused it. The panes stayed right and the model was poisoned, with
  nothing on screen to connect the two. `rebuild` is now handed a staged clone
  and `state.payloads` only advances past a mount that succeeded. Merely
  restoring the previous value was rejected as the fix: the editor is re-primed
  from the model, so that would have healed the model by silently discarding
  what the user wrote. The text lives in `state.drafts` instead, per block, and
  re-opening a rejected block still shows it.
- **A non-array `blocks` is refused instead of thrown over (`R0003-0068`).**
  `"blocks": 42` passed the schema-version gate and then hit `for…of` in the
  demo's own row loops — "is not iterable", uncaught, inside `boot()`. It is
  now a named fatal verdict, and every row loop in the module reads through one
  `Array.isArray` door.
- **Missing DOM nodes are a boot verdict, not a null dereference
  (`R0003-0069`).** `mountPanes` reads both panes before it writes either, so
  template drift in `demo-wasm.html` raised a TypeError past every catch. The
  required ids are checked once, first of all the gates — before the ~1.7 MB
  wasm module is fetched for a page that has nowhere to render it.
- **Both panes are sanitized before either is written (`R0003-0070`).**
  OI-0001's presence check answers for a DOMPurify that is *missing*; one that
  is present and *throws* (a broken build, a hooked instance) used to take the
  second `sanitize()` call with the first pane's markup already replaced — a
  half-swapped pair, and the exception escaping into `boot()` or a debounce
  timer. Staging both results makes the failure atomic: nothing is written, so
  the last good render needs no restore, and the presentation splits fatal (at
  boot) from the non-fatal strip (mid-session) like every other refusal.
- **`mountSync` refuses one element passed as both panes (`R0003-0071`).** It
  would be torn down twice, wired twice with competing scroll listeners and
  mutually-driving toggle mirrors, and tagged with a single
  `__transyncController` answering for both directions. Caller error, refused
  the way a missing pane is.
- **The `offsetTop` layout precondition is probed, not just documented
  (`R0003-0076`).** contracts.md §4a names its own failure mode "a silent
  layout regression"; a CSS refactor that drops `position: relative` on a pane
  still mounted successfully and drove the partner to the wrong place.
  `mountSync` now reads `offsetParent` on one representative anchor per pane at
  mount and warns, naming the element the offsets are actually measured
  against. A warning, not a refusal — the panes still pair by block id — and
  one property read per pane at mount, never per frame.
- **Four headless cases pin all of it (`R0003-0086`, `R0003-0090`).** The
  suite could not see the poisoned model at all: the DOM is identical either
  way and only the *next* edit tells the two apart, so the new case rejects an
  edit through the payload (an over-nested block payload the intake guard
  refuses), then edits a different block and requires that rebuild to land. The
  next two remove a pane element and make `sanitize` throw — the latter marking
  a live node in the source pane, so a half-swap is observed rather than
  inferred. The fourth serves `"blocks": 42` and requires the named fatal
  panel, so the boot verdict above is a gate a test actually walks through
  rather than one a refactor can reorder away in silence.

### Fixed — the renderer refuses a byte range it cannot slice, instead of clamping it into content (2026-08-09)

Review 0003, batch B5 (`R0003-0060`, `R0003-0078`). The residual the Review
0002 fix deliberately stopped short of: R0002-0054 drew the refuse-vs-degrade
line at alignment-map **shape**, and left range **values** to `block_text`,
which clamped them into the pane's length, reordered a reversed pair, and
snapped both ends to UTF-8 boundaries. **No curated-surface change**:
`transync-syntax::render` is contracts.md §0 tier (c) and stays on
`public_surface.rs`'s `forbidden` list, so the §0 table and its weld are
untouched.

- **`RenderError::UnusableRange { pane, ids }`** — a new `#[non_exhaustive]`
  variant. A range that runs backwards, ends past the pane's Markdown, or
  falls inside a UTF-8 character is now refused, named per row with its
  offsets and its fault, and bounded by the same sample the other refusals
  use. Coercion kept the renderer panic-free, which was its point, but it also
  kept the pane *rendering*: an empty block for a reversed range, a truncated
  one for an out-of-bounds range, a character-short one for a mid-character
  range — each under the row's own correct anchor, with `fallback_status`
  unchanged and no signal anywhere.
- **Checked once per pane, against what that pane slices.** `PaneCtx::new`
  measures `Block::source_range` against `doc.source_text` for the source
  pane and `AlignmentBlock::target_range` against the `translated_md` handed
  to `render_target` for the target pane, beside the two existing refusals.
  Every block is checked, not only the kinds that read raw bytes today,
  because Guard 2 can send any row down the byte-reading path.
- **An empty in-bounds range is still fine**, and a row naming a block the
  document does not have is still inert. `build_alignment_map` emits `0..0`
  for a block whose offsets it was not given, and warns (`R0003-0055`); that
  map still renders.
- **The browser demo needed no second gate (`R0003-0078`).**
  `web/js/wasm-demo.js` checks `target_range` values only for the rows it
  lets you *edit*, because its blast radius is the edit model — but html and
  skipped rows' ranges are read by the renderer's bypass arms, and nothing
  was checking those. The renderer now covers every row it reads, so a
  corrupt range on a non-editable row is a fatal, named refusal at the
  `render_pair` call instead of clamped bytes mounted as content. The demo's
  own gate stays where it is and keeps owning the earlier, edit-model
  message.
- `regen::regenerate` keeps `parser::ranges::clamped_char_bounds`
  (`R0003-0059`): it is infallible by design, so for it the snap is the safe
  answer and the alternative is a panic. The renderer has an error channel,
  which is why it can refuse.

### Fixed — a space is not a translation, and four more seams stop trusting what they were handed (2026-08-09)

Review 0003, batch B4 (`R0003-0042`, `R0003-0043`, `R0003-0066`,
`R0003-0067`, `R0003-0055`, `R0003-0057`, `R0003-0059`). Two content-loss
defects, both silent past every validation layer, plus three
programmatic-misuse signals. **No curated-surface change**: everything here
lives in `transync-syntax` and `transync-core`, which contracts.md §0 places
in tier (c) — engine internals, unreachable through the facade — so the §0
table and `crates/transync/tests/public_surface.rs` are untouched.
`transync-syntax` gains one dependency (`tracing`, already a workspace member
and `wasm32`-clean) and one new public function; ADR-0018, DCR-0016 and the
SCN-04 row move with the code.

- **A whitespace-only HTML segment erases visible text (`R0003-0042`).**
  `validate::per_kind::check_html` rejected only `String::is_empty`, while
  `htmlseg::scan` drops every *source* text node whose decoded form is all
  whitespace — so a source segment always carries visible text, and the one
  character class the check could not see was the one a provider needs to
  delete a sentence. `[" "]` for a real segment splices back as
  markup-preserving whitespace: the layer-3 tag inventory matches, the
  fragment reparse matches, the full-document reparse matches, and the block
  ships in `out.md` marked `translated` with its text gone. The predicate is
  now the same one extraction uses (`chars().all(char::is_whitespace)`), so
  the two ends of the segment contract state the same rule, and there is no
  false-positive case to trade against: a whitespace-only source segment
  cannot exist. The rejection is retryable and still names the correct answer
  ("echo the source segment").
- **An infoless code fence could gain an invented language (`R0003-0043`).**
  `check_code` returned `Ok` early whenever the source's info string was
  absent, and `regen::regenerate_code_block` takes the info string **from the
  translated payload** — so a model answering ` ```rust ` for a bare ` ``` `
  fence got `rust` written into `out.md`, end to end. That is the LLM deciding
  structure, which architectural invariant 2 reserves for the application
  ("must not alter … code fence/language metadata"), and no decision permits
  the absent→present direction. The check now compares the whole expectation
  in both directions; Comrak reports an infoless fence as `Some("")`, so empty
  and absent are one statement on both sides, and the `Some`→`Some` verbatim
  rule is unchanged. Nothing was added to the prompt: the compiled instruction
  already says "Preserve all structural Markdown markers (… code fences …)"
  and "never invent structure not in `source_payload`", so R0006-0035's rule —
  never punish the model for a rule it was not told — already held.
- **`<1>` is prose, not a tag (`R0003-0066`).** `htmlseg::scan_tags` accepted
  `is_ascii_alphanumeric` at the first name byte where HTML's tag-open state
  admits a letter only, so `<1>` and `<2026 rows>` tokenized as markup. The
  cost is R0002-0020's, exactly: `tag_inventory` recorded a phantom open for
  the **source** that the escaped spliced translation never reproduces, so the
  layer-3 post-splice check called an engine fault and dropped a correct
  translation to `fallback_source`; and on the render path `balance_fragment`
  appended a `</1>` — or **deleted** a written `</1>` as an orphan close,
  editing what the reader sees. Continuation bytes are unchanged, so `<h1>`
  and `<x-2>` are still tags and `<:x>` is still text.
- **A CDATA section is not markup (`R0003-0067`).** The scanner knew comments
  and raw text but had no `<![CDATA[` state, so tag-looking bytes inside an
  SVG/MathML section were inventoried and balanced as real tags — where a
  browser tokenizes none of them. The section is now skipped with the
  terminator HTML actually uses in each context: `]]>` inside foreign content,
  and the first `>` outside it, where the same bytes open a *bogus comment*.
  Foreign content is a depth count over `<svg>` / `<math>`, deliberately
  bounded for the same reason `implicitly_closes` is: a flat scanner cannot
  know the adjusted current node.
- **Incomplete `BlockOffsets` are named, not just survived (`R0003-0055`).**
  `align::build_alignment_map` filled a missing `target_range` with `0..0` and
  said nothing. The empty range stays — it is the *safe* answer, and the
  alternative (falling back to `block.source_range`) indexes the wrong string
  and used to panic the renderer on a char boundary — but the function now
  raises one `tracing::warn` per document naming the missed ids (bounded at a
  sample of eight plus a count). It stays **infallible** on purpose: the
  offsets `regen::regenerate` returns cover every block, so no shipped path
  can trip this, and a `Result` would tax every caller for a self-inflicted
  input. This is the one dependency `transync-syntax` gains.
- **`regen::regenerate_checked` reports accepted ids the document never had
  (`R0003-0057`).** `regenerate` reads the map from the document's side, so a
  key matching no block was never read and never mentioned — a caller with a
  mistyped `BlockId` got a document with the edit silently missing.
  `regenerate` is unchanged (this module is deliberately
  validation-agnostic); the companion returns the same Markdown and offsets
  plus the sorted unknown ids. `transync-wasm`'s own unknown-id check is left
  alone deliberately: it *refuses* rather than reports, and it covers
  `statuses` too.
- **A hand-built byte range can no longer panic regeneration
  (`R0003-0059`).** `Document` and `Block` are `pub` with `pub` fields, so a
  caller can build a `source_range` that splits a multi-byte char; regen
  clamped numerically and then sliced, which panics. `render::block_text` had
  grown a snapping loop for exactly this and regen never got one. Both now
  call one `parser::ranges::clamped_char_bounds`, so the two readers of raw
  offsets cannot drift apart again. Parsed documents are unaffected — comrak's
  positions are boundary-valid, so the snap is the identity for every range
  `parse` produces.

### Fixed — five things publication knew and never said (2026-08-09)

Review 0003, findings `R0003-0020` (an unflushed directory level),
`R0003-0021` (a directory that never opened), `R0003-0022` and `R0003-0024`
(cleanup residue), and `R0003-0023` (a mode that could not be carried over).
All in `crates/transync-cli`'s `output` module, all one cluster: a best-effort
step that failed and told nobody. Every one of them stays *advisory* — none
changes what a run returns or exits with, because none of them changes whether
the published bytes are correct. `docs/architecture/contracts.md` §6 and
`docs/architecture/persistence-and-files.md` move with the code; §0 and
`crates/transync/tests/public_surface.rs` are untouched — the facade's curated
surface is unchanged, and so is the `Notify` type these notes travel on.

- **Every created directory level is flushed, not only the deepest
  (`R0003-0020`).** `create_dir_all` can make a whole chain — `a`, `a/b`,
  `a/b/c` for one `a/b/c/out.md` — and the fileset commit flushed only each
  destination's own parent, while `--out-dir` staging recorded only
  `dest.parent()` for a nested payload and the chain leading to the `--out-dir`
  target itself was never recorded at all. The entry *naming* a level lives in
  the level above it, so a crash could leave a file whose contents and whose own
  directory entry were both durable, under an ancestor that never reached disk.
  All three paths now flush the whole chain, shallowest first and once per
  directory. Narrow in practice — durability here is advisory by policy and
  today's callers create at most one level under staging — so it is a wider
  flush, not a new failure mode.
- **A directory that could not be *opened* is as audible as one that could not
  be flushed (`R0003-0021`).** `fsync_dir` began `let Ok(f) = File::open(dir)
  else { return; }`, silencing every open failure on the strength of a rationale
  that is true only where directories have no handle at all. A Unix `EACCES` or
  `EIO` means exactly what a failed `sync_all` means — this directory was not
  flushed — and it suppressed the `R0002-0025` warning entirely. Silence is now
  reserved for the platform saying it has no such operation.
- **Cleanup that could not finish says so (`R0003-0022`, `R0003-0024`).** The
  staged-temp sweep and the created-directory rollback discarded every
  `remove_file`/`remove_dir` result, so a failed publication reported its
  original error while residue stayed on disk unmentioned. Each now emits one
  bounded note — a count and one example path, beside the run's real error and
  never in place of it. The rollback's *expected* refusals stay silent, because
  they are the design working: a level a peer published into
  (`DirectoryNotEmpty` — the empty-only rule of `R0002-0002`) and a level
  something else already removed (`NotFound`).
- **A mode that could not be carried over is reported (`R0003-0023`).**
  `commit_staged_renames` did `let _ = fs::set_permissions(…)`, so a failed
  `R0008-0042` preservation silently shipped the temp's fresh umask mode over a
  destination whose mode differed. It takes a `set_permissions` failure on a
  file this process just created, which is rare — hence a note, not a failed
  publication.

### Fixed — the packer prices what it actually sends, and four counters stop being able to wrap (2026-08-08)

Review 0003, findings `R0003-0031`, `R0003-0032`, `R0003-0046` (budget
fidelity), `R0003-0034` (an impossible output ceiling), `R0003-0038` (the last
uncapped provider string), and `R0003-0029`, `R0003-0030`, `R0003-0045`
(arithmetic). All in `crates/transync-core`. `docs/architecture/contracts.md`
§2, §5 and §6 move with the code; §0 and `crates/transync/tests/public_surface.rs`
are untouched — the facade's curated surface is unchanged.

- **The packer encodes the context hints from the wire JSON (`R0003-0031`).**
  `batch::estimate_unit` newline-joined the document title, section headings and
  the two neighbor summaries and encoded *that*, while `build_user_prompt` ships
  the same strings JSON-escaped inside a `context` object with field names. The
  module's own stated rule — "every one of them is *encoded*, not allowed for" —
  had one exception left, and it ran the unsafe way: a heading-rich document
  whose headings carry quotes, backslashes or newlines under-reserved, and the
  fixed 50-token per-unit framing allowance was never scoped to absorb the
  difference. It now encodes `llm::prompt::context_hint_json`, the same
  wire-form treatment `constraints` and the ADR-0009 `retry` channel have had
  since `R0001-0012`/`R0001-0013`. Ordinary prose costs the same as before —
  escaping is identity for text with nothing to escape.
- **A language label costs its escaped body (`R0003-0032`).** Same class, once
  per batch: the two ADR-0013 opaque labels were encoded raw although they land
  inside the user-prompt JSON. `envelope_input_tokens` now prices the escaped
  form. Again identity for an ordinary label.
- **The containment property is pinned for escaping-heavy input.** The existing
  "reserve + per-unit estimates bound the whole request" test used a quote-free
  fixture. A new one gives every context field and both labels
  quote/backslash/newline-heavy content, and fails without the two fixes above.
- **An output ceiling too small to answer in is refused at the loader
  (`R0003-0034`) — the one behavior change here an operator can notice.**
  `profile::normalize_batching` normalized `Some(0)`, but a nonzero ceiling at or
  below the fixed 64-token response-envelope reserve flowed straight through:
  `saturating_sub(64).max(1)` turned it into an effective output target of one
  token, every unit was flagged by the preflight, and the request went out with a
  `max_completion_tokens` / `max_output_tokens` too small to hold even the result
  framing — diagnosable, but failing at the provider instead of at the profile.
  Such a ceiling now gets the same warn-and-ignore treatment the zero has had
  since `R0001-0016`, with a warning naming the floor. The supported range of
  `[batching].target_output_tokens` is therefore `>= 65`, not `>= 1`;
  `contracts.md` §2/§6, the Profile Cookbook, the Developer Guide and **DCR-0012**
  (dated amendment) say so. `--target-output-tokens 0` is unaffected — it reaches
  the profile as `None` before this gate.
- **`detected_source_language` is bounded like every other provider string
  (`R0003-0038`).** The `R0002-0068` capping wave closed `warnings`,
  `rejection_reason`, the retry `reason` and the auto-glossary diagnostic; this
  envelope field was never in it. It is provider-authored free text that reaches
  `TranslationOutput`, the alignment map and the published bundle's `lang`
  attribute, so it now goes through the same 512-byte diagnostic cap at the one
  door it enters by — the dispatch latch. A hostile or broken provider gets to
  name a language, not to size the artifacts.
- **The output-axis estimate saturates instead of overflowing (`R0003-0029`,
  `R0003-0030`).** `resolve_expansion_factor` rejects only non-finite and
  non-positive factors, and a library caller sets
  `BatchBudget::output_expansion_factor` directly, so an absurd-but-finite value
  like `1e308` is a legal configuration. It saturated the documented float→int
  cast and then overflowed on the very next line — the addition of the echoed id
  and the per-unit framing — and again in the packer's running batch sum: a panic
  in any checked build. Both now saturate, which reads as "larger than any
  budget" and degrades into the packer's existing one-unit-per-batch behavior with
  an `OutputBudgetWarning` apiece. The input axis is unchanged: its sum is bounded
  by bytes that are simultaneously in memory, with no float amplifier.
- **Four run counters saturate (`R0003-0045`).** `provider_retries`,
  `batch_schema_faults`, `total_retries` and `total_fallbacks` used plain `+=` on
  `u32`. Every increment is one real round trip or one validated unit, so a wrap
  needs a run that cannot physically happen — but these are *reported* numbers,
  and a wrapped total reads as a clean run, which is the one failure mode
  telemetry must not have.
- **The 512-byte retry-reason cap says what it bounds (`R0003-0046`).**
  `MAX_RETRY_REASON_BYTES` is now `MAX_RETRY_REASON_PREFIX_BYTES`: it caps the
  retained prefix, and the emitted string is that prefix plus a fixed
  `"… (truncated)"` marker — which the module's own test had always asserted.
  The same correction lands on `validate::truncate_diagnostic`'s doc and on the
  four comments and two contract sentences that said "up to 512 bytes". No
  behavior change: nothing sizes a budget off the constant (the packer prices the
  actual wire JSON, marker included).

### Fixed — a Responses refusal outranks the text beside it, and the provider adapter's two configuration doors agree (2026-08-08)

Review 0003, findings `R0003-0006`, `R0003-0019`, `R0003-0087`, `R0003-0015`,
`R0003-0016` (the Responses envelope reader) and `R0003-0009`, `R0003-0010`,
`R0003-0011` (the constructors). All in `crates/transync-openai`;
`docs/architecture/contracts.md` §7 moves with the code, §0 and
`crates/transync/tests/public_surface.rs` are untouched — nothing in the
facade's curated surface changed.

- **A refusal outranks text on the Responses surface too (`R0003-0006`,
  `R0003-0019`).** The Chat surface has answered this way since `R0002-0041`
  ("a refusal outranks content, and that holds however the refusal arrives");
  the Responses surface was left asymmetric in two places. Its envelope reader
  returned the top-level `output_text` shortcut before it had looked at the
  nested items at all, and returned the concatenated nested text before it
  checked the refusals the same walk had just collected — so an envelope
  carrying both, which is the shape a compatible gateway produces, was
  translated as an ordinary answer on one surface and refused on the other.
  The whole envelope is now read before any of it is accepted. The
  classification is the one ticket `1a85f3` introduced —
  `TranslatorError::ModelRefused`, code `provider_model_refused` — not a new
  string-carrying `Other`. With nothing refusing, the shortcut still precedes
  the nested text, so envelopes from either OpenAI snapshot still land.
- **The regression pin (`R0003-0087`).** No test had ever put text and a
  refusal in one envelope. Four shapes now do — shortcut beside a nested
  refusal, both in one message, both in separate messages, and all of it on an
  explicit `completed` status — and they assert the stable code, not just the
  variant.
- **Two more provider-controlled strings are bounded (`R0003-0015`,
  `R0003-0016`).** The residual of `R0002-0039`'s batch:
  `incomplete_details.reason` in the ceiling diagnostic, and an unrecognized
  `status` in the "no output_text content" fall-through. Both are
  gateway-supplied and could carry up to the 32 MiB body cap into stderr, the
  logs and the run's report artifacts, while every sibling string in the same
  file already went through the shared 512-byte excerpt. Same helper, no
  second constant. The `incomplete` *classification* still reads the raw
  reason — matching on an excerpt would let a diagnostic detail decide a
  variant.
- **A padded model identifier is refused instead of stored (`R0003-0009`) —
  the one behavior change in this entry that an operator can notice, and the
  reason it rides the 0.4.0 window.** `ModelId::parse`'s own rustdoc already
  stated the contract ("surrounding whitespace is enough to reject on but not
  something to silently rewrite, because the stored string is simultaneously
  the wire value and a `fingerprint()` axis") and the code implemented neither
  half: `" gpt-5 "` was stored as given, rode out to a remote 400, and
  fingerprinted as a cache namespace of its own. It is now the new
  `ConfigError::PaddedModel`, which quotes the identifier so the invisible
  padding is visible. Interior whitespace is untouched — padding means the
  edges. `ConfigError` is the adapter's own enum, not a facade type, and
  nothing in the workspace matches it exhaustively.
- **A whitespace-only api key is refused (`R0003-0010`).** The class
  `R0002-0037` closed on the model axis, on the axis it did not cover:
  `try_new` tested `is_empty()`, so `"   "` passed and was found out as a 401
  one request later. The key is still never logged, echoed, or named in the
  error.
- **`from_env` stopped being the weaker door (`R0003-0011`).** It handed a
  padded `TRANSYNC_OPENAI_MODEL` to the *unchecked* `ModelId::new` and checked
  its key with `is_empty()` — its own copies of two rules that live on
  `ModelId::parse` and `try_new`. It now reads the three variables and
  delegates to `try_new`. Only the fallback stays its own: an unset or blank
  `TRANSYNC_OPENAI_MODEL` still takes the default rather than riding out as
  the model (`R0002-0037`), in a free function so the rule is testable without
  a process-global variable.

### Fixed — two records landed unlinked from the docs index (2026-08-08)

`docs/index.md` was missing rows for
`decisions/0020-cache-identity-excludes-adversarial-collision.md` and
`DCR-0022-review-0002-hardening.md`, both of which landed in `e5aa896`. The
standing `docs_index_drift::every_live_decision_and_dcr_is_linked_from_index`
gate had been red since that commit; it named both files, so this is the fix it
asked for and not a discovery. Found while adding DCR-0023's own row.

### Fixed — three living documents that described something other than the code (2026-08-08)

Tickets `e04a84`, `12f945`, `2d8a9a`. **Documentation only**: no Rust signature
moved, so the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched. Two of the three were
found while writing the Diátaxis manual, which had already routed around them.

- **`docs/Quick_Start.md` called `./scripts/smoke.sh` the no-API-key demo
  (`e04a84`).** It is the repository's hard gate: build, wasm32 gate, the
  workspace suite, the CLI stub suite, the rustdoc-gate completeness check plus
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`, then
  `scripts/build-wasm.sh`, and only then the CLI dry run the page described.
  A reader on the "about five minutes, no API key" path therefore hit a hard
  stop at a `wasm-pack` / binaryen ≥ 121 prerequisite the page never named —
  or earlier, at a missing `wasm32-unknown-unknown` rustup target. Step 3 now
  leads with the light path smoke.sh itself ends with (one `cargo run -p
  transync-cli --features test-stub-provider -- translate …`, which needs only
  Rust), and what smoke.sh really runs gets its own subsection. The same
  shortened framing lived in `README.md`'s quick-start block, in
  `docs/Developer_Guide.md`'s layout tree, and in the header comment of
  `scripts/smoke.sh` itself; all four move together, because a document copying
  a document is how this class of defect propagates here.
- **`docs/Troubleshooting.md` told readers to grep for values the JSON never
  contains (`12f945`).** Its `rejected_by` table listed the Rust variant names
  `Schema` / `PerKindShape` / `FragmentReparse`; `ValidationLayer` carries
  `#[serde(rename_all = "snake_case")]` and the file only ever holds `schema` /
  `per_kind_shape` / `fragment_reparse` (`wire_str()`, pinned to serde by
  `wire_str_matches_serde`). The table now shows the wire values, states the
  Rust-caller distinction the way `docs/Performance.md` already did, names the
  remaining values (`inline`, `provider`; `id_set` and `full_reparse` reserved
  and unused), and warns that `final_status: "fallback_source"` with a `null`
  `rejected_by` throughout is the html-splice path whose reason lives in that
  entry's `warnings`.
- **Release-checklist step 8 knew about one path consumer, and there are two
  (`2d8a9a`).** `/Volumes/Common/QJoon/dynwebserver` declares `transync` and
  `transync-openai` by path exactly as `resp-translator` does, so the ritual
  could finish green with a tag pushed while dynweb was broken. Step 8 is now a
  list, with the same `cargo check --workspace` instruction and per-consumer
  record for each, and an instruction to maintain the list rather than remember
  it. **No step was renumbered** — these numbers are cited from
  `workspace_publication.rs` failure messages, `module-map.md`, ADR-0019 and
  shipped CHANGELOG sections. `docs/project/status.md`'s summary of the
  standing gates is corrected to match.

### Changed — the investigation bundle is untracked again (2026-08-08)

Owner decision, reversing the 2026-08-06 one that committed it in `727d3bc`.
`docs/investigation/` joins `reviews/` in `.gitignore`; the files stay on disk
and are still read as sources by `docs/backlog.md`, but they are no longer
version-controlled. The reason is that the bundle is **regenerated from the
code** rather than authored, so every regeneration produced a review-shaped
diff that carried no decision. `Cargo.lock` stays committed (`6cf4164`,
oi-0020) — only the analysis-bundle half of that day's decision is reversed.

Side effect on an open ticket: `1347b4` (docs/index.md claims to index
everything under `docs/`) shrinks from fifteen unindexed tracked docs to one,
`docs/backlog.md` — an untracked bundle is not the index's to carry.

### Added — a host can set the adapter's request budget (2026-08-08)

Ticket `7065fdb6`. **Additive**: `TransyncOpenAI` gains one builder and one
getter; every existing caller keeps the 120s default and behaves identically.
The `transync` facade's contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched — `transync-openai` is
its own crate with its own §7 contract.

- **The request budget was the one per-instance axis a host could not
  express.** `REQUEST_TIMEOUT` was a private 120s const baked into the client
  builder, and `new` built the client unconditionally, while
  `with_reasoning_effort` and `with_api` already existed for the other axes. A
  library consumer that exposes its own timeout knob therefore could not honor
  it: resp-translator's `translation.request_timeout_ms` (default 600000) fed
  only its own watchdog (+60s), so the watchdog waited 660s for a request this
  crate killed at 120s — and the watchdog's stated premise, "the provider's own
  timeout fires first", was false for every configured value above the default.
- **`with_timeout(Duration)` sets the total per-request budget** — first byte
  sent to last byte read, spent again on every bounded provider retry — for one
  instance. `request_timeout()` reports what the instance holds, so a host can
  log or reconcile the budget it actually got rather than assume its
  configuration was honored.
- **`CONNECT_TIMEOUT` stays a separate budget** (10s, R0002-0042) and is
  deliberately not parameterized: it guards a connect that was never going to
  complete, which is orthogonal to how long a caller will wait for a response.
  A test pins that a 600s request budget leaves the connect budget at 10s.
- **The timeout is deliberately NOT a `fingerprint()` axis** — the only
  per-instance field that is not. Every other axis feeds the fingerprint
  because it changes *what* the provider returns; a timeout changes only
  *whether* a result arrives in time. Folding it in would split the cache
  namespace between hosts that merely tune their patience, so two runs with the
  same model, prompt and glossary would stop sharing work for no semantic
  reason. `the_timeout_is_not_part_of_the_cache_namespace` pins it.
- The two client-builder helpers collapsed into one
  (`http_client_builder_with`); the no-argument wrapper became test-only the
  moment the budget was parameterized, which `clippy -D warnings` catches as
  dead code.

### Changed — four internal seams stop keeping a second copy of the same fact (2026-08-08)

Review 0002, batch B8 (`R0002-0069`, `R0002-0081`, `R0002-0085`,
`R0002-0086`). **No curated-surface change and no behavior change**: no Rust
signature moved, so the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched, and every existing
test passes unchanged. Each of the four was a place where the compiler checked
that a *field* or an *arm* existed but could not check that its *value* still
agreed with its twin — the drift surface that survives an exhaustive match.

- **`regen::regenerate` borrows the payload it splices (`R0002-0069`).** It
  cloned every accepted payload and copied every fallback slice before writing
  either, so regenerating a document briefly held a second copy of the whole
  translated text. Both sources outlive the splice, so the common arm now
  borrows and only the two transforming arms (code-fence re-wrap, html splice)
  own a string. Output is byte-identical by construction — the arms did not
  move, only their ownership.
- **`validate::validate_unit` has one rejection shape (`R0002-0081`).** Seven
  layers each wrote out a full `ValidatedUnit` literal that differed only in
  `rejected_by` and `rejection_reason`; the field set was compiler-checked, the
  field *values* were not, so an eighth arm could have quietly kept a payload
  or claimed `Translated`. One `reject(layer, reason)` closure now states the
  shape once. The html splice check keeps its own constructor on purpose — it
  is the one arm that is not a rejection (no layer, no reason, so the retry
  loop skips it, §3.3).
- **`walk::label_for` delegates to `BlockKind::wire_str` (`R0002-0085`).** The
  normalized top-level label and the wire label were two hand-written tables
  agreeing on thirteen of fifteen kinds. They now agree by construction: only
  `ListItem` (one item of a `List` node that normalizes to one entry) and
  `Image` (a wire-level promotion Comrak reparses as a `Paragraph`) are written
  out. Because the labels are compared element-wise against `node_label`, whose
  values stay hand-written literals, a new `label_for_pins_every_kind` test
  pins all fifteen so a `wire_str` rename cannot move them out from under that
  comparison unnoticed.
- **`align::build_alignment_map` has one row constructor (`R0002-0086`).** The
  translatable and non-translatable branches pushed field-for-field identical
  `AlignmentBlock` literals — including the same
  `offsets.0.get(..).unwrap_or_default()` — and differed only in whether the
  row was counted into `validation_summary`. Translatability now decides only
  the counting; the row is built once.

### Docs — four documents stop promising a server that never ran (2026-08-08)

Review 0002, batch B7 (`R0002-0071`, `R0002-0072`, `R0002-0073`,
`R0002-0074`). **No curated-surface change**: no Rust signature moved, so the
contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched. `transync serve` shipped **DEFERRED (STUB-061)** — it parses
`--rendered` / `--bind` / `--port`, prints the bind address it would have used,
and exits `5` (`Other`) — and `contracts.md` §6, `mvp-scope.md`, ADR-0006 and
the Developer Guide have all said so for some time. Four documents had not
caught up, and each is a different kind of record, so each is corrected
differently.

- **`crates/transync-cli/src/serve_cmd.rs` is code, so it is fixed in place
  (`R0002-0071`).** Its module summary opened "Static HTTP server on
  `127.0.0.1` for the demo bundle" — the one sentence rustdoc puts at the top
  of the page, contradicted two screens down by `run`'s own doc, which had the
  deferral right all along. The summary now leads with the deferral, says what
  the command does instead, and points at `contracts.md` §6.
- **The three project records get dated appended corrections, not rewrites.**
  `skeleton-plan.md` §3 ("`transync serve` is a short-lived 127.0.0.1 server
  bound for local demo use", `R0002-0072`) is a Phase-3 snapshot whose own
  closing rule sends changes to a DCR rather than an in-place edit;
  `intake.md` ("Served by `transync serve` from RAM" in the File/Blob Lifecycle
  table, and a matching process-model bullet, `R0002-0073`) is a Phase-1
  transcription. Both now carry a `Correction (2026-08-08)` blockquote naming
  the sentence, stating what actually ships, and noting that the process model
  is *narrower* than planned rather than wider — no daemon, no persistent
  process, no IPC holds all the more firmly when nothing binds. The intake's
  correction also retires the same row's `include_dir!`: the shipped code uses
  three `include_str!` constants and never took that dependency.
- **The SL-13 row's residue is a misrecorded history (`R0002-0074`).**
  `implementation-slice-checklists.md` already declares every slice closed and
  its bullets deliberately not rewritten, so the row was not going to mislead
  new implementation work. What survived that defense is narrower and worth
  recording: `transync serve` was *already* STUB-061 when SL-13 closed, so
  "consumes the SL-12 output bundle from `transync serve --rendered <dir>`"
  never described how this slice was accepted either. SCN-13 was verified
  against `python3 -m http.server` per `web/SMOKE.md`, as `scripts/test-browser.sh`
  still is. An inline correction, in the file's own parenthetical style, says so.
- **The behavior is now pinned, which it never was.** The CLI suite exercised
  exit codes 0..4 and never invoked `serve` at all — the fact four documents now
  assert had no test behind it. `crates/transync-cli/tests/serve_deferred.rs`
  (2 tests, no feature gate, no provider) asserts that a satisfied `serve` exits
  `5` with the deferral, the served directory and the bind address on stderr and
  nothing on stdout, and that a missing `--rendered` is still clap's
  `ArgumentError` (1) — so "parses its flags and acts on none of them" stays
  distinguishable from "prints before clap runs". That the process *exits* is
  the proof it binds no socket; no port probe, and none of a port probe's
  flakiness.

All four corrections say **deferred**, not abandoned: the real loopback server
is commissioned as post-0.3.0 roadmap ticket `b791d6`.

### Fixed — the browser engine says no out loud, and the demo listens (2026-08-08)

Review 0002, batch B6 (`R0002-0015`, `R0002-0016`, `R0002-0017`,
`R0002-0045`, `R0002-0047`, `R0002-0049`, `R0002-0052`, `R0002-0053`). **No
curated-surface change**: `web/js/sync.js` is a reference consumer, not part
of the Rust facade, so the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched. Two JS signatures
move — `mountSync`'s return and `fetchOk`'s parameter list — and
`docs/architecture/contracts.md` §3 now states both. `web/js/sync.js` and its
byte-identical CLI twin `crates/transync-cli/web/sync.js` moved together, as
`sync_js_drift.rs` requires.

- **A refused mount is now distinguishable, and it is refused before it
  costs anything (`R0002-0016`, `R0002-0015`, `R0002-0049`).** `mountSync`
  returned an inert `{ destroy(){} }` for a map it would not drive, which no
  caller could tell from a working mount — so `web/js/wasm-demo.js` assigned
  `state.map`, returned `true`, and boot marked the demo `ready` over panes
  that rendered and never scrolled together. It now returns `null`, and the
  demo fails the mount on it. Two orderings behind that were wrong the same
  way. `mountSync` judged the map *before* tearing down the controller a
  previous mount had left on the panes, so a refused re-mount left the old
  engine live, still tagged on both panes — measured in the new suite: the
  follower was dragged from scrollTop 300 to 3 by a map the engine had just
  rejected, while `__transyncController` (the documented "am I mounted?"
  read) still said mounted. Teardown now runs first. And `mountPanes` wrote
  both panes' `innerHTML` before the engine ever saw the map, so a refusal
  destroyed the last good render to put an unsynchronized one in its place;
  the outgoing markup, scroll offsets and map are now kept until the mount is
  accepted, and a refusal restores them and re-mounts the last good map.
- **A map that describes nothing to synchronize is refused (`R0002-0047`).**
  `blocks: []` — or a map whose every row is `non-sync` — passed the row gate
  by having no malformed row to name, and the engine then paired whatever
  `data-sync-id` anchors the panes happened to carry: exactly the degradation
  R0001-0043's gate exists to refuse. Refused now, but only for panes that
  carry anchors: an empty document still mounts its empty map over its empty
  panes, because nothing to pair on either side is vacuous rather than
  mismatched.
- **`schema_version` components are digit-bounded (`R0002-0045`).** An
  unbounded run of digits converts to `Infinity`, and `Infinity` compares as
  newer than this engine forever — so `1.<400 nines>.0` was accepted with the
  forward-compat warning instead of refused as the malformed string it is.
  Nine digits per component is orders of magnitude past any schema this
  project will publish and keeps every component a safe integer, so the
  `Number` conversions cannot lose precision. Same bound in the demo's mirror
  of the gate.
- **Artifact fetches are bounded and cancellable (`R0002-0052`,
  `R0002-0053`).** `fetch` has no timeout of its own, so a server that
  accepted a connection and then stalled pinned the demo in `booting` and the
  shells in their loading state, with nothing on the console to say why.
  `fetchOk` now aborts after `timeoutMs` (default 20 s, covering the body read
  as well as the response) and reports the timeout by name; it also accepts a
  caller-owned `signal`. All three boots — the wasm demo, `web/index.html`
  and the CLI bundle's `index.html` — pass one `AbortController` to their
  three artifact fetches, so the first failure cancels the siblings still in
  flight instead of leaving them running against a load already lost.
- **The rebuild's nested map parse is inside the error boundary
  (`R0002-0017`).** `JSON.parse(out.alignment_json)` sat one line past the
  `try` that exists to turn a failed rebuild into a message on the non-fatal
  strip; a malformed nested payload would have thrown out of a timer callback
  instead. Defense in depth — `alignment_json` is serialized by the same
  `rebuild` call and a serialization failure already throws inside the
  boundary — but the boundary should cover the whole rebuild, not most of it.

### Fixed — the wasm demo's structure gate compares shape, not just how many blocks there are (2026-08-08)

Review 0002, batch B5 (`R0002-0014`). **No curated-surface change**:
`transync-wasm` is `publish = false` and the facade does not re-export it, so
the contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched. No signature moves — `structure_warning` fires in strictly more
cases and says more when it does.

- **A same-count kind substitution now warns (`R0002-0014`).**
  `check_top_level_structure` compared only how many normalized top-level
  entries each side had, so a payload that swapped one block for a *different
  kind* of block, one for one, passed silently — and the render it passed was
  the degraded one. Demonstrated on `> quoted text` + `second paragraph` with
  `"EDITED PLAIN TEXT"` typed over the blockquote: two top-level blocks
  before and after, no warning, and because `render_fragment`'s cursor does
  not advance past the paragraph node the edited blockquote produced, the
  following paragraph's anchor rendered the *edited* text while its own
  content left the pane entirely. The check now compares the whole normalized
  fingerprint — top-level count, label sequence, and each collapsed list's
  item count — which is the same three facts `validate::full_reparse`
  compares on the pipeline side. The per-list count catches the other
  same-count shape change: an item split renders one `<li>` short, because
  `render_list_group` pairs rows against the `List` node's direct items
  positionally. Both sides are projections of `walk::normalize_top_level`, so
  the normalization rule still has exactly one home and `transync-wasm` still
  depends on `transync-syntax` alone. The count message is unchanged; the two
  new cases name the offending 1-based block position. Still a warning and
  never a rejection: the panes in the same result are a real render and the
  block stays editable, so the user can undo the change.

### Fixed — the renderer checks the map it is handed, and the HTML balancer stops inventing tags (2026-08-08)

Review 0002, batch B4 (`R0002-0010`, `R0002-0011`, `R0002-0020`,
`R0002-0054`, `R0002-0059`, `R0002-0060`, `R0002-0061`). **No curated-surface
change**: `render` is engine-internal (contracts.md §0 tier (c), pinned by
`public_surface.rs::hidden_modules_are_not_documented_as_surface`), so the
§0 table and the weld are untouched. Two engine-internal signatures DO move —
see the first bullet — and `transync-wasm` grew `EngineError::Render`.

- **The renderer refuses an alignment map that does not describe the document
  (`R0002-0010`, `R0002-0011`).** `render_source` / `render_target` return
  `Result<String, RenderError>` instead of `String`. Two silences are gone: a
  repeated `source_block_id` let whichever row the row index happened to hold
  last decide that id's attributes, order and byte ranges; and a block with no
  row at all was skipped outright — no anchor, no placeholder, no warning —
  so an incomplete pane was indistinguishable from a complete one. Both are
  producer defects (`align::build_alignment_map` emits exactly one row per
  block, in document order) and both were reachable only through a
  caller-supplied map, i.e. `transync-wasm`'s view mode, where serde accepts
  either shape. Rows naming blocks the document does not have are still
  accepted: they are inert. The error names the offending ids, bounded at five
  plus a count. Callers thread it — the pipeline maps a refusal to
  `TransyncError::Internal` (its own map builder is the only producer there,
  so a refusal is an internal fault), and `transync-wasm` gained
  `EngineError::Render`. ADR-0006 and contracts.md §4a carry the amendment.
- **Both panes' Markdown meets the same intake (`R0002-0059`).** The target
  pane handed `translated_md` straight to `comrak::parse_document`, bypassing
  the nesting-depth guard and the NUL normalization `parser::parse` applies to
  a source — so a document the library refuses to parse was rendered anyway,
  and any future recursive walk over the target tree would have been
  unguarded. Both panes now go through the new `parser::intake`, which is also
  what `parse` itself calls, so the ceiling has one home. Only the parse
  consumes the normalized string: the bypass and Guard-2 arms still slice the
  caller's own bytes, which is what the byte ranges index. No pipeline
  behavior changes (a regenerated document has already been through `parse`
  by the time it renders); a wasm edit that types a document past the ceiling
  now reports a rebuild failure instead of rendering it.
- **`<textarea>` and `<title>` content is RCDATA, not markup (`R0002-0020`).**
  `htmlseg::scan_tags` entered raw-text state for `script` and `style` only,
  while the balancer already treated all four `RAW_TEXT_ELEMENTS` as raw-text
  — and the disagreement cost a translated block twice. `tag_inventory`
  reported a phantom `/p` for `<textarea>Use </p> to close</textarea>` on the
  **source** side only (lol_html reads RCDATA correctly and the splice escapes
  `<>&`), so the layer-3 post-splice check declared an engine fault and any
  translated `<textarea>`/`<title>` holding tag-like bytes fell back to
  source; and on the live render path the same phantom was **deleted** from
  the pane as an orphan close tag, editing what the reader sees. The
  tokenizer now enters the state for every entry in the set. The self-closing
  expectations DCR-0016 Part D recorded are unchanged.
- **Namespaced tag names are scanned whole (`R0002-0060`).** The name scan
  stopped at `:`, so `<div:x>` tokenized as a `div` and the balancer appended
  a `</div>` — which at DOMPurify parse time pops the sync wrapper's own
  `<div>` and spills the rest of the block outside its anchor, exactly the
  failure balancing exists to prevent. A colon now belongs to the name after
  at least one name byte; a leading colon is still not a tag name.
- **HTML's optional end tags are generated implicitly (`R0002-0061`).** Every
  non-void open tag sat on the stack until an explicit close, so `<p>a<p>b`
  earned two appended `</p>`s and a browser turned the surplus one into a
  phantom empty `<p></p>` — structure in the pane that is in no source
  document. A start tag now pops the optional-end-tag elements it closes
  (`p` by the block-level set, `li`, `dt`/`dd`, `tr`/`td`/`th`, the table
  sections, `option`/`optgroup`, `rp`/`rt`) before being pushed. The rule is
  bounded to elements at the top of the stack: HTML's real algorithm walks
  down through non-special elements, and guessing at that from a flat stack
  would close more than a browser does.
- **The wasm demo refuses an unusable `target_range` instead of coercing it
  (`R0002-0054`).** `sliceBytes` clamped and reordered, so a reversed range
  became an empty pre-edit payload and an out-of-range one a truncated
  payload. That is a document-wide fault, not a per-row one: `applyEdit`
  resubmits the whole `state.payloads` map on every rebuild, so one mis-sliced
  row was written into its block the first time the user edited *any* block.
  A third boot gate (`targetRangeVerdict`) now checks every editable row's
  range for integer offsets, ordering, bounds and UTF-8 character boundaries,
  and fails closed in the same fatal panel the schema and duplicate-row gates
  use. The editable-row predicate moved to one `isEditableRow` so the gate and
  the edit model cannot disagree about which rows are checked.

### Fixed — every composed cache axis frames its own fields, and the report bounds what the model writes into it (2026-08-08)

Review 0002, batch B3 (`R0002-0007`, `R0002-0008`, `R0002-0068`). **No API
change**: no type, field or signature moves, and the contracts.md §0 facade
table and `crates/transync/tests/public_surface.rs` are untouched.
**Cache identity moves for two axes** — see the last bullet.

- **`ProviderFingerprint` composes injectively (`R0002-0007`).** Parts were
  joined with a bare `\u{1F}`, so `new("p", &["a\u{1F}b", "c"])` and
  `new("p", &["a", "b\u{1F}c"])` were the *same* fingerprint — two provider
  configurations in one cache namespace, which is the under-distinguishing
  direction contracts.md §1 warns about. The doc comment claimed the
  opposite. Every part, the family label included, is now written as its byte
  length, a `\u{1F}`, then the part, so no arrangement of part contents can
  produce the string another arrangement of parts produces.
  `from_type_name(name)` is now literally `new("type", &[name])` — a reserved
  family, stated rather than assumed.
- **The glossary digest frames its fields (`R0002-0008`).** `glossary_hash`
  terminated each entry's source term, target term and note with a `0` byte,
  while glossary normalization deliberately *keeps* control characters (it
  warns and moves on), so a NUL inside a value could impersonate a field
  boundary: `{source: "a", target: "b", note: "c\0d"}` and
  `{source: "a", target: "b\0c", note: "d"}` hashed the identical buffer.
  Values are length-prefixed now and `note` carries a presence byte, the same
  discipline `pipeline::context_hash` adopted.
- **Both were latent, and are stated as such.** Neither produced a reachable
  cache confusion at HEAD: every colliding glossary pair was separated by
  `profile_prompt_hash` (the rendered body escapes control characters), and
  the bundled adapter's fingerprint parts are 0x1F-free by construction (a
  parsed URL percent-encodes C0 controls; the api and effort labels are closed
  sets). An axis that is only correct because another axis happens to be
  watching is not an axis — and `ProviderFingerprint::new` is public, so a
  third-party `Translator` composing from arbitrary configuration had no such
  second axis behind it. The tests pin the *encoding* (the two constructed
  colliding pairs now hash apart), not a symptom.
- **Provider warnings are bounded (`R0002-0068`).** `UnitResult.warnings` is
  a schema field the model fills in, forwarded verbatim by every validation
  arm into the validation-report JSON and into the cached result, with no cap
  of its own — while the sibling provider-authored strings have been capped
  since R0008-0034. It is now capped at **8 messages per unit, each ≤ 512
  bytes**, through the same helper (`truncate_diagnostic`, moved to
  `validate` so the two callers share one cap rather than restating it). The
  overflow is named — `"… (N further provider warnings dropped; M total)"` —
  and an engine-authored warning is appended after the cap, so provider volume
  cannot displace it.
- **Cache consequence: the two encodings re-key their entries once.** A run
  **with a glossary** misses once and retranslates (a run without one still
  hashes an empty buffer and keeps its entries); every entry produced under
  any provider fingerprint is in a new namespace and misses once. Nothing on
  the wire moves — prompt bytes, goldens and instruction text are
  byte-identical — and `VALIDATION_SCHEMA_VERSION` is deliberately not bumped:
  none of its three documented triggers fires, and re-keying these axes is
  what these two levers are for.

### Fixed — the provider adapter bounds what it trusts a gateway to send (2026-08-07)

Review 0002, batch B2 (`R0002-0005`, `R0002-0037`, `R0002-0038`,
`R0002-0039`, `R0002-0041`, `R0002-0042`). All of it is `transync-openai`.
The contracts.md §0 facade table and `crates/transync/tests/public_surface.rs`
are untouched: `transync-openai` is a provider crate, not part of the curated
`transync::` surface. `transync-openai`'s own documented surface (contracts.md
§7) does grow — `ModelId::parse` and `ConfigError::EmptyModel` — and §7 was
updated in the same commit.

- **A refusal is read wherever it arrives (`R0002-0041`).** On Chat
  Completions a `refusal`-typed content part was dropped by the text filter:
  a refusal beside text returned the text as ordinary output, and a
  refusal-only array degraded into the generic missing-content error rather
  than the refusal diagnosis `message.refusal` already earned. Both shapes
  now reach the same diagnosis, with the refusal outranking text beside it.
- **Refusal diagnostics are capped, on both surfaces (`R0002-0039`).** The
  refusal string was the one provider-controlled text that could expand to
  whatever the 32 MiB response cap allowed on its way into stderr, the logs
  and the run's report artifacts — while the same files already capped HTTP
  error bodies and the Responses `error` object at 512 bytes. It goes
  through the same `truncate_diagnostic` now.
- **A non-success body is read for its diagnostic, not to the answer cap
  (`R0002-0038`).** Error bodies are bounded at 64 KiB instead of 32 MiB; the
  read stops there and the transfer ends. Reaching the ceiling is not a
  failure — the status classifies the failure. Consequently the up-front
  `Content-Length` rejection is now confined to success bodies: **a 503 (or
  any non-success status) declaring more than 32 MiB now reaches the pipeline
  as its own retryable classification instead of a terminal "response
  exceeded 32 MiB cap".**
- **Cleartext base URLs are announced, not refused (`R0002-0005`).** An
  `http` base URL puts the api key (a bearer token on every request) and the
  document body on the network in the clear. Nothing is rejected — a
  plain-http ollama/litellm gateway on a trusted intranet is a real
  deployment — but a non-loopback `http` base URL now earns a
  one-per-process `tracing::warn` naming what travels unencrypted. Loopback
  stays silent. The warning comes from `new`, and so from *every* constructor:
  `try_new` and `from_env` both end in `new`, which leaves one call site for
  all three. (An earlier shape of this fix announced from the two checked
  constructors only, leaving `TransyncOpenAI::new` silent.)
- **A model identifier that names no model is refused at construction
  (`R0002-0037`).** `ModelId::parse` is the checked counterpart of
  `ModelId::new`, and `TransyncOpenAI::try_new` applies its rule; an empty or
  whitespace-only identifier is `ConfigError::EmptyModel` instead of a remote
  400 plus an API surface chosen out of a blank name. A whitespace-only
  `TRANSYNC_OPENAI_MODEL` takes the default rather than riding out as the
  model.
- **The HTTP client has a connect budget inside its request budget
  (`R0002-0042`).** `connect_timeout` 10 s inside the existing 120 s total
  timeout, so a black-holed connect fails in seconds instead of spending the
  whole request budget — and spending it again on each bounded retry.

### Fixed — publication deletes only what it created, and refuses before it charges you (2026-08-07)

Review 0002, batch B1 (`R0002-0001`, `R0002-0002`, `R0002-0004`,
`R0002-0023`, `R0002-0025`, `R0002-0029`, `R0002-0032`). All of it is
`transync-cli`'s publication layer; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched, because none of these
paths is on the library surface.

- **A rolled-back run could delete a peer's committed output (`R0002-0002`).**
  Two ordinary runs were enough: run A creates a missing shared directory and
  records it, run B blocks on A's publication lock, A's staging fails, A
  releases the lock — waking B — and then `remove_dir_all`s the directory B has
  since published into. Rollback is now **empty-only**: `fs::remove_dir` one
  level at a time, deepest first, after removing A's own lock marker and only
  while that marker is the directory's sole occupant. Anything a peer added
  makes the rollback a no-op instead of a loss, and R0008-0041's "a failed
  stage leaves no directory residue" still holds for the levels that really are
  empty. The regression test drives the real lock in two threads with no
  sleeps: it loses `peer.md` against the old code.
- **`--out-dir` siblings are now this run's alone (`R0002-0001`).** The staging
  and backup names carry a random per-run token
  (`.<name>.staging.<pid>.<token>`) and the staged directory is *created*
  rather than adopted, so every sibling the publish deletes is one it made — a
  reused pid or a planted name can no longer route cleanup at something else.
  Own-pid reclamation of a crashed predecessor survives with better evidence
  (full reserved name, directory type, under the lock) and is now confined to
  *staging* trees: a `.<name>.backup.<pid>` is the operator's previous output,
  possibly the only copy, and is never reclaimed by a later run.
- **A `--force` publish over a regular-file target left a hidden backup forever
  (`R0002-0023`).** The moved-aside file was cleaned up with `remove_dir_all`,
  which fails with `NotADirectory` and was best-effort, so the dot-name stayed.
  Cleanup now branches on `symlink_metadata`: a tree, a file, or a symlink's
  own entry.
- **`--out-dir` entries that escape the staged tree are refused
  (`R0002-0004`).** An absolute or `..`-bearing relative path would have joined
  its way out of the staging root before the publishing rename. Defense in
  depth — the only caller passes fixed names — now enforced rather than
  assumed.
- **Duplicate destinations are recognized through normalization
  (`R0002-0032`).** `a/../out.md` versus `out.md`, and paths through a
  symlinked parent, used to survive the preflight and fail halfway through
  staging with a confusing "refusing existing temp". They now collide up front
  with the sentence that names them. A destination that *is* a symlink stays a
  distinct destination: publication renames over the link entry, so those two
  commit as two independent files.
- **Destination guards run before the provider call, not after
  (`R0002-0029`).** A foreign `--html-out` directory, an `--out-dir` target
  that is not a prior out-dir, and a duplicated destination are all decided by
  the arguments and the filesystem alone; collecting the refusal after a paid
  translation run discarded the whole run (the cache is per-run and in-memory)
  for a fact that was true before it started. The early pass is advisory and
  silent; the authoritative pass still runs at publication time — under the
  publication lock for `--out-dir` — and both refuse with the same exit code
  and the same sentence.
- **A directory that fails to flush says so (`R0002-0025`).** `fsync` failures
  on a directory were discarded wholesale, so a run could report an
  unqualified success for a publication whose durability was not established.
  They are now reported as a `transync: note:` line naming the directory and
  scoping the damage — the published file *contents* are durable (every staged
  payload's `sync_all` propagates), the directory entries naming them may not
  survive a crash. Not fatal, and still a silent no-op where opening a
  directory for read is unsupported (Windows).

### Docs — ADR-0016 retracts a citation, and the performance guide earns the section it was citing (2026-08-07)

Ticket `038c54`. **Documentation only** — no code, no test, no gate, no change
to any artifact the CLI writes; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched.

- **The citation was never true.** ADR-0016's "Implementation" section ended
  with "`docs/Performance.md` describes memory scaling with document size for
  accepted inputs". It never did: `git log -S` over that file finds no commit
  that ever introduced `RSS`, `resident`, `peak`, or `memory` in the
  consumption sense — every `Memory` hit is `InMemoryCache` — and the guide has
  been about wall-clock since the commit that added it. ADR-0016 is a snapshot
  record, so the sentence stays and a dated **Correction (2026-08-07)**
  blockquote follows it: the claim was false, nothing in the decision rested on
  it (the ADR argues memory-tracks-size from the architecture itself, in its own
  Decision Drivers), and its third consequence — the admission cap is boundary
  hygiene, not a peak-memory guard — is unaffected either way.
- **`docs/Performance.md` grew a *measured* "Memory" section, not an estimated
  one.** Six peak-RSS points taken with `/usr/bin/time -l` over release
  stub-provider runs — 543 B, 11.9 KB, 13.8 KB, 121 KB, 1.24 MB and 5.03 MB of
  source, landing at 62, 63, 63, 67, 131–134 and 391 MiB — with the exact
  command, the machine, the two-run protocol and the synthetic fixtures' shape
  (per-block unique content, so the in-run cache cannot collapse units) stated
  inline. The section is labelled coarse and reports only what was measured: no
  curve is fitted, no unmeasured size is predicted, and it says outright that it
  locates neither the 64 MiB-input figure nor the true OOM ceiling. The ~60 MiB
  fixed floor is separated from document cost by a `transync --version` run that
  peaks at 3 MiB, so the floor is paid by the run and not by process startup.
- **`docs/index.md`'s one-line summary of the guide names the new section.** No
  other document repeated the retracted claim — `docs/Troubleshooting.md` points
  at `Performance.md` for the tuning model only.

### Tests — the usage blocks state the defaults clap prints (2026-08-07)

Ticket `fc0b18b2`. **One new test plus the two sentences that describe it** —
no code path, no manifest, no CLI behavior change, no new gate: every flag,
default and exit code is what it was before, and the contracts.md §0 facade
table and `crates/transync/tests/public_surface.rs` are untouched.

- **The flag weld checked names and nothing else, so a wrong default would
  have passed every gate.** `crates/transync-cli/tests/docs_cli_flags_drift.rs`
  (ti `57be8c`) pins the *set* of flag names in contracts.md §6 and the
  Developer Guide's "CLI reference" and deliberately reads no description —
  which is right for prose, but a default value is not prose. Where
  `transync <sub> --help` renders `[default: V]`, V is a literal an entry
  either states or does not, and the four in flight —
  `--max-input-bytes` (67108864), `--source-language` (auto), `--port` (7470)
  and `--bind` (127.0.0.1) — are exactly the kind of thing that rots in a hand
  copy without a reader noticing.
- **The new check derives its own subjects from the help text.** A flag clap
  gives no default contributes nothing, so the `Option<T>` flags and the
  `SetTrue` bools are never asked about, and `--model` / `--base-url` stay out
  by construction rather than by an exclusion list — their defaults resolve
  through `TRANSYNC_OPENAI_MODEL` / `TRANSYNC_OPENAI_BASE_URL` and then a
  literal in `args::resolve_model` / `provider::translator_for_run`, which is
  why the guide's lines name the environment variable first. A flag that gains
  or loses a default enters or leaves the check with no test edit.
- **It is scoped to the flag's own entry, and a syntax sketch does not answer
  for a stated default.** The check reads the entry a flag heads, not the whole
  block (`auto` appears in the block for `--target-direction` too), and drops
  the entry's `--flag` and `<metavar>` tokens first — without that,
  `[--source-language <label>|auto]` would have satisfied the `auto` default
  from its own placeholder. Mutating each of the four defaults on the document
  side, then on the clap side, was verified to fail the test in both
  directions; the current documents pass unmutated, so no default was wrong
  today.
- **The two sentences that advertise the weld's reach were updated to match**
  — contracts.md §6's "only the set of flag names is machine-verified" and the
  guide's "stop naming the same set". Both now say the defaults are checked and
  the rest of each description is not.

### Docs — the performance guide's timing recipe writes the file its table reads (2026-08-07)

Ticket `c1da11`. **Documentation only** — no code, no test, no gate, no change
to any artifact the CLI writes; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched. Every claim is derived
from the code that implements it — `translate_cmd::TranslateArgs` (clap),
`translate_cmd::publish::publish_outputs`, `validate::ValidationReport`,
`align::ValidationSummary`, `cache::CacheKey`, `pipeline::CacheKeyContext`,
`unit::budget::resolve` — and each corrected recipe was executed against
`samples/demo-complex.md` with the stub provider.

- **The diagnosis recipe read an `out.json` no run writes, and never asked for
  the file whose fields it quotes.** `docs/Performance.md` said "Run with
  `--verbose` and check `out.json`", then headed its symptom table with that
  file's `validation_summary` — while `attempts`, `attempt_number`,
  `rejected_by`, `total_retries` and `total_fallbacks` are all fields of the
  *validation report*, which is written only under `--out-dir` (as
  `validation-report.json`) or when `--validation-report <path>` names it. The
  section now separates the two artifacts, its example passes
  `--validation-report`, and the table's rows quote the report's real JSON
  paths and wire spellings (`rejected_by: "per_kind_shape"`, not
  `PerKindShape`, which is the Rust variant). Two rows were added for
  `provider_retries` and for what `total_retries` excludes.
- **None of the four Rust recipes compiled.** Each constructed
  `TranslateOptions` with a struct literal plus `..Default::default()`; the
  type is `#[non_exhaustive]`, so a consumer got `E0639` (reproduced from a
  scratch crate against the facade). They are default-then-assign now, and the
  defaults table says so once.
- **The advice for sharing a cache pointed at a private module.** "Call
  `pipeline::run_pipeline()` with your own `Arc<dyn Cache>`" — that module is
  `pub(crate)` in `transync-core` and unreachable from the facade, and the
  parameter is `&dyn Cache`. The entry point is `translate_with_cache`.
- **The cache section described a cross-run cache that does not exist.** The
  CLI's `translate()` builds a fresh `InMemoryCache` per call, so a second
  identical CLI run takes no hits at all (verified: 0 cache-hit rows) and
  `attempt_number: 0` never means "you're re-running" there — it means a
  within-run duplicate (verified: 51 of 60 identical paragraphs). The
  cross-run promise now belongs to the caller that keeps a cache across
  `translate_with_cache` passes (verified: 63/63 hits on the second pass).
  The six-axis cache-key tuple was replaced by `CacheKey`'s actual axes,
  including the context and instruction hashes that decide whether an edit
  invalidates a block's neighbors.
- **`RateLimited` was described as a value in the validation report.** It is a
  `TranslatorError` variant: a rate-limited run surfaces as the pipeline's
  transient-retry warning on stderr and a rising `provider_retries`, and once
  the per-batch budget is spent the run aborts and writes no outputs.
- **The latency formula multiplied by the concurrency cap** — reading as "more
  concurrency, more wall-clock" — where it divides by it, as the guide's own
  provider-ceiling section already said.
- **`target_input_tokens_per_batch = 10000` was annotated "still inside 8K
  context".** The target bounds the batch's whole input (the system prompt,
  instruction envelope and language labels are reserved off it before unit
  payloads are packed), so 10000 does not fit an 8K context; the interaction
  with the output ceiling (`--target-output-tokens`) is named too.
- **Timings replaced with measured ones.** "<100 ms even for 1000-block
  documents" measured ~0.17 s from a release binary and ~1.5 s from the debug
  binary `cargo run` builds; the warm-cache floor measured ~0.1 s on
  `samples/demo-complex.md`. Also fixed: the cross-reference to
  `pipeline.rs::cache_key_for` (no such symbol —
  `CacheKeyContext::for_run` / `key_for`), the missing
  `max_per_batch_schema_retries` default, and which dials are CLI flags versus
  library-only.

### Docs — `--force` is about foreign files, not about a non-empty directory (2026-08-07)

Ticket `3b90c1`. **Documentation only** — no code, no test, no gate, no change
to what the CLI accepts; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched. Every claim below is
derived from `crates/transync-cli/src/output.rs`
(`ensure_html_out_safe` / `scan_bundle_dir` / `ensure_out_dir_replaceable`),
`translate_cmd::TranslateArgs` and `serve_cmd`, with `contracts.md` §6 read
only as a cross-check.

- **`docs/Developer_Guide.md`'s CLI reference described a rule the CLI does not
  implement.** Its one-liner read `[--force] overwrite non-empty --html-out`,
  which tells the reader to pass a flag on the most common run there is —
  re-running a translate into the same `--html-out`. The preflight refuses only
  a *foreign* entry (a name outside the bundle's six, or outside a prior
  out-dir's own entries); a directory holding a previous bundle is overwritten
  in place with no flag. The one-liner now says what is refused and what is
  not, and a new paragraph under the block spells out the rule, says which of
  transync's own files each guard skips (`.transync-publish.lock` everywhere,
  `*.tmp.<pid>` in every bundle scan but not at an `--out-dir` target's top
  level), and records that the check runs before any output is written, so a
  refusal (exit 4) cannot strand a half-written output set.
- **`--model` and `--base-url` stopped hiding their environment defaults.** The
  block claimed `default: gpt-5-chat-latest` / `default: https://api.openai.com`
  while `args::resolve_model` and `provider::translator_for_run` consult
  `TRANSYNC_OPENAI_MODEL` / `TRANSYNC_OPENAI_BASE_URL` first. The two lines now
  name the variable ahead of the literal, agreeing with the guide's own
  environment table further down.
- **The guide no longer implies `transync serve` binds a socket.** It published
  the subcommand's three flags with their defaults and said nothing about the
  command being deferred (STUB-061), which `contracts.md` §6 states twice. The
  block header now carries `(deferred stub — nothing binds)` and a short
  paragraph says what it does instead (prints the address, exits `5`) and what
  to use in its place.
- **`docs/Troubleshooting.md`: two entries that had drifted the same way.** Its
  staging-temp entry promised that "the `--html-out` and `--out-dir` guards
  recognize them, so you do not need `--force`" — true except at the *top level
  of an `--out-dir` target*, where `ensure_out_dir_replaceable`'s allow-list
  admits only `out.md`, `alignment.json`, `validation-report.json`, `html` and
  the lock marker, so a `*.tmp.<pid>` left there by a crashed `--output` /
  `--map` run into that same directory does make the target foreign; the entry
  now names that corner and says to delete the leftover rather than force. And
  its performance checklist still said "the CLI doesn't expose
  `--max-concurrent-batches` today", sending readers to library code for a flag
  clap has accepted since DCR-0012; it now says to re-run with the flag.

### Docs — the troubleshooting entry stops offering `--no-verify`, and a test keeps CI claims honest (2026-08-07)

Ticket `8c5156`. **Documentation plus one new test** — no code path, no script
and no gate changed; the one manifest edit is a corrected *comment*
(`crates/transync-cli/Cargo.toml`), not a build change, and the contracts.md §0
facade table and `crates/transync/tests/public_surface.rs` are untouched.

- **`docs/Troubleshooting.md` no longer sanctions a bypass.** The entry for a
  blocked commit offered `git commit --no-verify` "only when you're certain",
  justified by "the hook catches things that would otherwise blow up in CI" —
  the exact opposite of `docs/project/release-checklist.md` steps 7 and 24
  ("never bypassed with `--no-verify`"), and resting on a CI that has never
  existed here (no workflow file in the tree, none anywhere in the history).
  The entry now lists the four Rust gates `scripts/hooks/pre-commit` runs —
  fmt, clippy, the wasm gate, the rustdoc gate — as the commands to re-run by
  hand (the rustdoc one by sourcing `scripts/lib/rustdoc-gate.sh`, whose crate
  list keeps its single home), points at the hook's `[pre-commit] …` echo as
  the way to tell which gate failed, and says why a bypass is not a shortcut:
  nothing else runs those gates by itself, so a skipped hook is an unchecked
  commit until a human runs `scripts/smoke.sh`.
- **`docs/Developer_Guide.md` states the rule where it describes the hook.**
  It listed the gates and said a failure blocks the commit, but said nothing
  about bypassing, which left the prohibition in the release checklist alone;
  it now says the hook is never bypassed, why (nothing else runs those gates
  by itself), and where the other two documents stand.
- **Three sibling documents stop citing a CI that is not there.**
  `docs/Quick_Start.md`'s prerequisite table said "CI uses an echo stub" (it
  is the *tests* that do); `docs/Performance.md` recommended the test-stub
  provider "for CI", now for the scripted runs that actually use it
  (`scripts/smoke.sh`, `scripts/test-browser.sh`); and
  `docs/implementation/module-map.md`'s integrations table promised a
  "`--integration-live` job [that] calls real OpenAI when `OPENAI_API_KEY` is
  present" one row above its own "no CI" answer — replaced by what exists,
  the `#[ignore]`d, doubly-gated `crates/transync-openai/tests/live_smoke.rs`
  behind `scripts/smoke-live-gate.sh`.
- **The last two "stub for CI" claims go too** (review of this ticket, same
  day). `docs/implementation/module-map.md`'s *scenario-coverage* table — the
  one above the integrations table corrected in the bullet before this — still
  read "OpenAI Responses API (when run live; stub for CI)" for SCN-12, so the
  file contradicted itself; it now names the two live HTTP surfaces the
  provider dispatches between and the `test-stub-provider` echo `Translator`
  that every automated run uses. `crates/transync-cli/Cargo.toml` documented
  that feature as "Used by smoke + CI" and now names the runs that exist
  (`tests/cli_smoke.rs`, `scripts/smoke.sh`, `scripts/test-browser.sh`).
- **New test: `crates/transync/tests/docs_gate_claims_drift.rs`** (6 tests).
  Every `--no-verify` in a guarded living document must sit in a paragraph
  that also says *never*, so the bypass can be named as the prohibition but
  never offered; no guarded living file may name `CI` outside a segment that
  also says "no CI", so the mention can be the denial but not the claim; and
  no CI configuration may exist in the tree, which is what
  makes "there is no CI" true — if CI ever lands, the test fails and names the
  living documents that then have to describe it. The CI check reads table
  **rows** rather than paragraphs, because a Markdown table has no blank line
  in it and one row would otherwise vouch for another — which is exactly how
  the scenario-coverage row above survived the first pass. Snapshot records
  are not guarded: they keep what was true when written.

### Docs — a lost git object gets found the day it happens (2026-08-07)

Ticket `7a7feb`. **Documentation only** — no code, no manifest, no script
and no gate changed; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched.

- **The release checklist opens with an object-store check.** New **step 0**
  of `docs/project/release-checklist.md`: `git fsck --no-progress
  --connectivity-only`, passing only with no `missing` and no `broken link`
  line (`dangling` lines are normal in any clone). It exists because this
  clone lost an object: on 2026-08-07 the `docs/` subtree of `494bc9c`
  disappeared from `.git/objects` minutes after that commit was written, and
  every `git show` or `git diff` crossing it failed with `fatal: unable to
  read tree`. A tag is the worst place to find that out — the released
  commit would name a history nobody can clone — and same-day detection is
  also what keeps the damage cheap, since a tree is only rebuildable while
  the surrounding commits still hold every blob it referenced.
- **It is step *0* on purpose.** The checklist's step numbers are cited from
  outside the file — the failure messages in
  `crates/transync/tests/workspace_publication.rs` (steps 17, 19),
  `docs/implementation/module-map.md` (step 6), `docs/project/open-issues.md`
  and the snapshot ADR-0019 (step 18). Renumbering would falsify a record
  that is never edited in place, so the file now states the rule: a new step
  takes a number that leaves the existing ones where they are.
- **`docs/Troubleshooting.md` gained the symptom entry** — `fatal: unable to
  read tree` / `git fsck` reporting a missing object: how to tell a fault
  line from a `dangling` line, the cause (a file-sync client
  re-materializing files under `.git/` while git writes them, on the
  external volume the worktree lives on; the tell is a `.pack` mtime newer
  than its own `.idx`), and the purely additive repair — rebuild the tree
  from a neighbouring commit's entries with `git mktree` and accept it only
  when the printed hash equals the missing one. A missing *blob* has no such
  trick, which is the whole argument for detection over recovery.
- **`docs/Developer_Guide.md` records both mitigations** in "Build, test,
  lint": the preflight check, and `gc.auto 0` on a worktree two concurrent
  sessions commit into (an automatic gc firing inside one session's commit
  is an independent second way to lose an object; an explicit `git gc` at a
  quiet point is unaffected and is what the setting assumes). It also
  records why the check is **not** in `scripts/smoke.sh`: object-store
  health is a property of the clone, not of the change under test, and a
  gate that fails for reasons unrelated to the diff sends people hunting the
  wrong bug.

### Docs — the guide's CLI reference names every flag, and a test keeps it that way (2026-08-07)

Ticket `57be8c1d`. **Documentation plus one new test** — no code path, no
manifest, no CLI behavior change: the flags `transync translate` accepts are
byte-for-byte the ones it accepted before, and the contracts.md §0 facade table
and `crates/transync/tests/public_surface.rs` are untouched.

- **`docs/Developer_Guide.md`'s "CLI reference" block was ten flags short and
  said nothing about it.** It listed 20 of the 30 flags the two subcommands
  declare, while `docs/index.md` advertises the guide as carrying the "full CLI
  reference" — so a reader had no signal that `--help` knew flags the guide did
  not. The missing ten: `--max-input-bytes`, `--validation-report`,
  `--target-direction`, `--target-output-tokens`, `--output-expansion-factor`,
  `--target-input-tokens-per-batch`, `--max-units-per-batch`,
  `--max-concurrent-batches`, `--auto-glossary` and `--no-auto-glossary`. Two of
  them were features the guide discussed in prose without ever naming the flag
  that turns them on. The block now lists all 30, in `--help` order, and says
  above itself that `contracts.md` §6 remains the authority for semantics.
- **The prose that discussed those features now names them.** The batching
  section names the five CLI overlays and records that clap rejects a `0` on the
  three `>= 1` knobs outright (argument error, exit 1) where the library and
  profile path warns and ignores; the glossary section names
  `--auto-glossary` / `--no-auto-glossary` and the `auto_glossary` profile key
  as the switch for the preflight it already described; and the new paragraph
  under the block says `--validation-report` is inert alongside `--out-dir`
  (which always writes `validation-report.json`) rather than an argument error.
- **The list is now welded to clap.** `crates/transync-cli/tests/docs_cli_flags_drift.rs`
  scrapes `transync translate --help` and `transync serve --help` — clap's own
  rendering of the argument structs, so it cannot lag the code the way a third
  document copying a second one does — and fails three ways: a shipped flag no
  block names, a block naming a flag clap does not accept, and the guide's block
  disagreeing with contracts.md §6's. Only the *set of flag names* is checked;
  the descriptions stay hand-written. This is the drift class three tickets in
  this arc (`c044d4`, `e9481b`, `14307f`) each traced back to by hand.

### Fixed — the source-language sentinel is one run however it is spelled (2026-08-07)

Ticket `fd5aa88c`, ADR-0013 amended. **No facade change** — the contracts.md §0
table and `crates/transync/tests/public_surface.rs` are untouched; the CLI's
flags, exit codes and output shapes are unchanged.

- **`--source-language AUTO` now reaches every consumer as `auto`.** Both
  sentinel tests in the tree folded ASCII case, so `AUTO` already compiled the
  *"the auto-detected source language"* prompt and already rendered the HTML
  bundle's source pane from the model's detected label — but the argument
  boundary only trimmed, so `CacheKey::source_lang`, the user-message payload
  and the alignment map's `source_language` carried the literal `AUTO`. One
  request had two identities: the prompt and the pane said "detection was
  requested", the key and the metadata said "a label spelled `AUTO`". A
  recognized sentinel is now stored in its canonical spelling at the same place
  the labels are trimmed, so every spelling produces a byte-identical
  `alignment.json`, `out.md` and bundle shell.
- **One recognizer, asked by both callers.**
  `translate_cmd::args::is_source_language_sentinel` is now the CLI's only
  definition of "is this the sentinel?"; the argument boundary canonicalizes
  with it and `publish::pane_source_language` asks it instead of answering the
  question again locally. Two independent copies of that test were what let the
  answers diverge.
- **Only the reserved literal is folded.** A value the recognizer rejects is
  stored byte-for-byte, case included (`DE`, `Auto-detect`,
  `"Korean (formal, 존댓말)"`), and nothing is reserved on the target side, so
  `--target-language AUTO` stays `AUTO`. Labels remain opaque (ADR-0013).
- **The library layer is deliberately unchanged**, on the same layering the
  2026-08-06 padding amendment stated: `TranslateOptions::source_language` is
  forwarded verbatim and only its sentinel *test* tolerates padding and case, so
  a caller bypassing the CLI with `"AUTO"` compiles the detection prompt but
  keys and stamps `AUTO`. That asymmetry is now pinned by a test rather than
  left to be discovered.
- **Cache note: a run spelled `AUTO` or `Auto` re-keys, once.** Its former
  entries are never looked up again, and it now shares identity with `auto`
  runs — correct rather than merely convenient, since the canonical label is
  what goes on the wire too, so two runs sharing a key send byte-identical
  requests.

### Docs — OI-0020 is closed by the lockfile that has been committed since 2026-08-06 (2026-08-07)

Ticket `d0cd9825`. **Documentation only** — the open-issues dashboard, the status
record and ADR-0019's amendment log; no code, no manifest, no test, no gate and
no behavior change; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched.

`docs/project/open-issues.md`'s OI-0020 was titled "No committed Cargo.lock
(blocked by machine-global gitignore)" and carried `Status: OPEN`, but
`git ls-files --error-unmatch Cargo.lock` has succeeded since `6cf4164`
(2026-08-06), where the owner reversed the previous day's pin-over-lock
deferral and added a repo-local `!Cargo.lock` un-ignore. The root manifest and
`docs/project/release-checklist.md` were corrected in the 0.3.0 window; the
dashboard was not, so the one document whose job is to say what is open was the
last one still saying it. **OI-0020 is now RESOLVED (2026-08-06)** with the
commit named, the entry retitled to a topic ("Committed Cargo.lock vs. floating
dependency resolution") that no longer asserts a state the tree contradicts, and
the filed title preserved verbatim in a `Filed as` line. Nothing dated was
rewritten: the 2026-07-11 problem statement and the 2026-08-05 pin-over-lock
note stand as written, and the reversal is an appended 2026-08-06 note.

- **The `=0.2.126` pin is not made redundant by the lock**, and the resolution
  says so instead of leaving it to be inferred. A lockfile binds only the
  resolves that read it — `cargo update`, a regenerated lockfile and any build
  after the lock is deleted all re-resolve from the manifests — and
  wasm-bindgen's requirement is crate↔**CLI** identity, of which no lockfile can
  pin the CLI half at all (`wasm-bindgen-cli` is a host tool installed outside
  the workspace). The pin's standing changed, not its necessity: it was the only
  defense against clean-checkout skew and is now the half covering the lock-less
  resolves. This is what the root `Cargo.toml` comment and release-checklist
  step 18 already said; the dashboard now agrees with them.
- **Two residuals are named rather than implied.** The `CLAUDE.md` half of the
  same gitignore concern was **declined, not fixed** (recorded in `6cf4164`'s
  message and the `.gitignore` comment), so this repo's `CLAUDE.md` is untracked
  by owner choice; and nothing gates the committed lock's freshness — no check
  runs `--locked`, so keeping it current is release-checklist step 18, a human
  step.
- **Siblings swept.** `docs/project/status.md` listed OI-0020 among the open
  issues and called the Review-0008 follow-ups "OI-0017..OI-0020" trigger-gated
  when three of those four are closed; both now match the dashboard, and the
  Track C wave bullet carries a dated note that its pin-over-lock decision was
  reversed. **ADR-0019** gained an `## Amendment (2026-08-07)` recording the
  supersession of decision 2's lock half while its pin half stands — the
  snapshot text, including the front-matter description, is left as written. The
  2026-08-03 dependency-set canary note inside OI-0028 gained a dated update
  (its "the manifests float" premise changed; its re-run rule did not).
- **Left alone on purpose.** `docs/investigation/architecture/constraints-and-debt.md`
  (a dated 2026-08-04 investigation snapshot), DCR-0017's and DCR-0020's
  lockfile sentences, the Track C plan and design spec, `phase-state.yaml`'s
  2026-08-05 wave block, and the `[0.2.0]` CHANGELOG entry's "ships no committed
  `Cargo.lock`" are all **true as history** and dated as such — the 0.3.0 window
  is where the record turns over, and `phase-state.yaml`'s current block already
  says the lock is committed.

### Docs — the language flags take an opaque label, not a BCP-47 tag (2026-08-07)

Ticket `c044d441`. **Documentation only** — guide prose, two doc comments and one
script comment; no code, no test, no gate and no behavior change; the
contracts.md §0 facade table and `crates/transync/tests/public_surface.rs` are
untouched.

`docs/Developer_Guide.md`'s CLI reference block called `--target-language`
"`(required) BCP-47 target`" and gave both language flags a `<code>` metavar.
Nothing validates a tag. `translate_cmd::args::resolve_language_and_model_args`
trims *surrounding* whitespace exactly once, rejects an empty or whitespace-only
label as an argument error (exit 1), and hands the rest on untouched — to the
compiled system prompt, to `CacheKey::{source_lang, target_lang}`, to the
alignment map, and to the bundle's `lang` attribute. That is a decision, not a
gap: ADR-0013 considered BCP-47 enforcement and rejected it *because*
`"Korean (formal, 존댓말)"` is a legitimately useful label a validator would
throw out. So the guide was wrong twice over — a reader would have expected that
invocation to be refused, and would never have learned that saying register and
style in the label is an intended use of the flag. `contracts.md` §6 ("Language
labels at the CLI boundary") already said all of this and remains the authority;
the guide now agrees with it, with a `<label>` metavar and a paragraph under the
block. **What the CLI accepts is unchanged** — the fix was the sentence, not the
behavior, and adding the validation the guide implied would have broken labels
ADR-0013 exists to protect.

- **The library's own field docs carried the same claim, and are corrected
  too.** `TranslateOptions::source_language` read "BCP-47 language code or the
  literal string `auto`" and `target_language` "BCP-47 target language code;
  required" — what a `cargo doc` reader is told, and what ADR-0013's 2026-08-06
  amendment flatly contradicts ("`TranslateOptions::source_language` remains
  opaque"). Both now describe the opaque label and name `translate_with_cache`'s
  non-emptiness check as the only rule. They also state the one thing that
  differs below the CLI: the trim belongs to the *argument boundary*, so a
  library caller who passes `" ko "` keeps the padding and keys its own cache
  entry.
- **`scripts/smoke-live.sh`** described `TRANSYNC_LIVE_TARGET_LANG` as a "target
  BCP-47". It is a label like any other; `ko` is its default, not its schema.
- **The `--title` section said the opposite, and is corrected too.** "Title and
  document language are presentation only: neither reaches `out.md`, the
  alignment map, or the provider" predates this wave and was false for its
  language half — it also contradicted the new paragraph under the CLI reference
  block. `--title` genuinely is presentation only: it names the bundle's
  `<title>`, and the `document_title` hint the model receives is built from the
  source document's own first H1, never from the flag. The language label is
  not: `profile::render_prompt_body` substitutes it into the system prompt and
  `pipeline::report::assemble_alignment_map` writes it to the map's
  `source_language` / `target_language`, so `<html lang>` is its last stop, not
  its only one. The sentence now says which flag is which.
- **Left alone on purpose.** The structured-output schema's
  `detected_source_language` description ("BCP-47 code of the detected source
  language, or null") is an instruction to the *model* about what to return, not
  a claim about what transync accepts — and it is pinned by the prompt goldens,
  so editing it would change the wire. `reviews/README.md`'s `R0001-0072` (the
  **retired** 2026-05-02 Review 0001, `git show bb93b68^:reviews/reviewed/0001.md`)
  is the finding as filed, and `crates/transync/tests/fixtures/scn-02-table-small.md`
  is fixture bytes a test hashes, not documentation.

### Docs — OI-0008's parser and unit-construction items wear their own review ids (2026-08-07)

Ticket `81b28a00`. **Documentation only** — prose in the project records; no
code, no test, no gate and no behavior change; the contracts.md §0 facade table
and `crates/transync/tests/public_surface.rs` are untouched.

Ticket `4af82bbc` settled *which round* a bare `R0001-` id names. This is the
next question — whether an id names the right finding inside its round — and
two of OI-0008's bullets did not. The retired 2026-05-02 Review 0001 titles
`R0001-0077` "Parser traversal has too many mutable cross-cutting parameters"
and `R0001-0078` "Unit construction mixes payload extraction, profile
rendering, batching, and structural inspection" (`git show
bb93b68^:reviews/reviewed/0001.md`; `reviews/README.md` indexes it). OI-0008
had the pair the other way round, and every document written afterwards copied
the pairing from there rather than from the review.

- **`open-issues.md` and `status.md` are corrected in place** — the two living
  records. The bullet describing the `structure{,/labels}` +
  `unit/{payload,budget}` split is `R0001-0078`; the one describing the parser
  subtractive slice and the `WalkState` restructure is `R0001-0077`. The two
  bullets also traded places, so the entry's `0076`..`0080` run reads in
  order — which is what makes a future re-swap visible on sight.
- **The parser bullet's summary now says what the finding said.** It read "the
  parser walker mixes multiple concerns" — the one paraphrase in that group
  that did not track its finding's title, and the reason the swap looked
  plausible. It names the mutable cross-cutting parameters instead.
- **Dated records keep their text and gained notes.** The wave's design spec
  (§4, §7, §9) and its implementation plan carry the same swap, and four
  commit messages already in history — `f10b070`, `8b339f3`, `094c1c4`,
  `73fd331` — carry it on the wire where nothing can edit it. Those are
  records of what was said at the time, so each document got a dated
  2026-08-07 note stating the correct pairing rather than a rewrite.
- **The rest of the round was re-checked, not assumed.** Every `R0001-`
  citation in the tree that resolves to the retired round — OI-0008's other
  six ids, `open-issues-archive.md`'s entries, `phase-state.yaml`, DCR-0008,
  DCR-0017, DCR-0018, ADR-0007, ADR-0013, this file's `[0.2.0]` fix-wave
  section, and the retired-round comments in living code — was compared
  against the recovered review text. `R0001-0077` and `R0001-0078` were the
  only pair attached to each other's work.

### Docs — review-finding citations say which review round they came from (2026-08-07)

Ticket `4af82bbc`. **Documentation only** — comments, doc comments and prose;
no code, no test, no gate and no behavior change; the contracts.md §0 facade
table and `crates/transync/tests/public_surface.rs` are untouched.

Two review rounds numbered themselves `0001`: the retired 2026-05-02 round
(104 findings, files removed in `bb93b68`) and the `reviews/0001.md` on disk
(2026-07-15, 52 findings). Finding numbers `0001`–`0052` therefore meant two
different things, and both meanings were in active use — `pipeline/report.rs`
cited `R0001-0025` as "`retried_units` was never populated" while `llm.rs`
cited the same id for the list-depth `u8` overflow. A reader chasing a bare id
opened the only `reviews/0001.md` there is and read an unrelated finding;
that had already produced one false "mis-cited id" report.

- **`reviews/README.md` is new**: the round registry (all nine rounds, their
  dates, and where each one's text is — on disk, recoverable with
  `git show bb93b68^:…`, or never tracked at all), the citation convention,
  and a finding index for the retired Review 0001 so its ids resolve without
  git archaeology.
- **The convention follows the shape `beb8481` and `llm.rs` already used**: a
  bare `R0001-NNNN` is the live `reviews/0001.md`; a citation of the retired
  round names its file. Finding numbers above `0052` exist only in the retired
  round and are self-identifying.
- **Dated records are not rewritten.** `CHANGELOG.md`'s `[0.2.0]` and
  `[0.3.0]` sections each gained a dated note naming the round they cite —
  `[0.2.0]` is retired-round throughout, `[0.3.0]` live-round throughout — and
  the one historical plan that cites an ambiguous id got the same treatment.
  `open-issues-archive.md` did too, on the backstop pass: most of its entries
  name their round on the `Source:` line in the older
  `(review archived and removed)` spelling, but OI-0014 cites `R0001-0010` /
  `0009` in its body alone, where no marker reached it.
- **Living code and docs were swept**: 26 retired-round citations across
  `transync-syntax`, `transync-core`, `transync-openai`, `transync-cli` and
  the scenario tests now name `reviews/reviewed/0001.md`, as do the
  `R0001-` ids in `open-issues.md`'s OI-0008 entry, `status.md` and
  `phase-state.yaml`.

### Docs — `--strict-csp` confines the bundle to an origin, not to a directory (2026-08-07)

Backstop over ticket `14307f84`. **Documentation only** — no code, no test, no
gate and no behavior change; the emitted policy is byte-for-byte the one
`343f9ee` shipped, and the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched. The `[0.3.0]` entry
for the flag needed no correction: it says "everything remote stops loading",
which is what the policy does.

The `--strict-csp` line in the `contracts.md` §6 CLI usage block said the flag
confines images, scripts, styles, frames and connections "to the bundle
directory". CSP `'self'` is scoped to the serving **origin**. A bundle
published under a document root that holds other content can still load that
content, because it is same-origin — so an operator reading only the usage
block could believe in a folder-level sandbox the flag does not provide. The
normative "Bundle Content-Security-Policy" paragraph below it was already
correct; the summary above it was not.

Both now say origin, and both say what that excludes: other hosts, not sibling
paths on the same host. `CSP_META`'s rustdoc in `transync-cli`'s `output`
module (`default-src 'self'` — previously "bundle directory or nothing") and
the Developer Guide's "Hardening a bundle you did not write" section carry the
same correction, so the flag is described identically wherever it is described.

### Docs — the Developer Guide's CLI reference admits `--out-dir` (2026-08-07)

Backstop over ticket `14307f84`. **Documentation only**; no flag, default or
conflict rule changes.

`docs/Developer_Guide.md`'s CLI reference block listed `--output` and `--map`
as unconditionally `(required)` and omitted `--out-dir` entirely — while the
"Hardening a bundle you did not write" section added by the same ticket named
`--out-dir` twice, as one of the two bundle modes `--strict-csp` applies to. A
reader following the reference could not find the flag it had just been told
about.

What clap actually declares: `output` and `map` are plain `Option<PathBuf>`
with no `required`, and `out_dir` carries
`conflicts_with_all = ["output", "map", "html_out"]`; the
either-or rule is enforced after parsing by
`translate_cmd::args::resolve_output_target`, which accepts `--out-dir` alone
or `--output` **and** `--map` together and rejects every other combination as
an argument error. The reference now lists `--out-dir` with its exclusions,
marks the other two as conditional rather than required, and states the
one-output-target rule under the block, pointing at `contracts.md` §6
"`--out-dir` semantics" for the published layout.

### Docs — the 0.3.0 notes stop contradicting themselves about `content_filter` (2026-08-07)

Backstop over ticket `3c9741bf`. **Documentation only** — no code, no test, no
gate and no API change; the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched.

Two entries inside the already-released `[0.3.0]` section disagreed. The
`3c9741bf` entry (*an exhausted output ceiling is named as one on Chat
Completions too*) listed `content_filter` among the stop reasons that "still
goes to extraction" — true when it was written, and made false later in the
same release window by `0583a75a` (*a Chat content-policy stop is reported as
one, not as a missing body*), which turned `content_filter` into an acted-on
terminal diagnosis. A reader of the 0.3.0 notes met both claims with no way to
tell which one described the shipped code.

The shipped code was never in doubt: `chat::finish_reason_error` acts on
`length` and `content_filter` and on nothing else, its rustdoc lists neither
among the reasons that reach extraction, `chat_other_finish_reasons_still_extract`
pins that list, and `docs/architecture/contracts.md` §7 has said the same since
`0583a75a` landed. Only the older changelog bullet was stale.

That bullet is corrected in place rather than rewritten silently: it now names
`stop`, `tool_calls`, an absent field and unknown gateway vocabulary, and
carries a dated **Corrected 2026-08-07** note pointing at the entry that
superseded it. Released text is not edited without saying so.

### Fixed — a kept-but-flagged glossary entry is named once, not once per gate (2026-08-07)

Backstop over ticket `5f6664d4`, correcting a regression the previous backstop
over the same ticket introduced. **No API change**: no type, field or signature
moves, and the contracts.md §0 table is untouched. This is
**diagnostics-only** — no entry changes, no prompt byte changes, no cache
identity changes.

`profile::normalize_glossary` had three findings: an empty term (drop), a
repeated source term (drop), and a control character in `source` / `target` /
`note` (**keep** — `escape_for_quoted` already neutralizes it, and the warning
exists so the author learns the term is not rendered literally). Because the
third one changed nothing, re-running the check re-produced it. The check runs
at three gates, so once batching became the third one, a run over a profile
whose note held a newline printed the identical

```
WARN transync::profile: glossary[0].note contains the control character U+000A; …
```

**three times** — `load_profile`, the translate boundary, and
`unit::build_batches` — the R0001-0032 double print the gate work cites as its
own rationale, and the defect class ticket `33e178` exists for.

- **The gate check is now a fixed point.** Every warning
  `normalize_glossary` returns names an entry it *removed*, so running it again
  on what it left behind is silent. Three gates can share one check without one
  of them re-diagnosing an earlier one's finding, by construction rather than
  by agreement. `profile::normalized_glossary_profile` therefore returns `None`
  — "keep the profile you hold" — for every already-normalized glossary,
  including one carrying a flagged entry.
- **The advisory moved to the door the prompt leaves by.**
  `profile::glossary_control_char_warnings` is emitted once, from
  `unit::build_batches`, beside the doubled-prompt count that moved there for
  the same reason (ti 28110f). That is the door both the pipeline and a direct
  batcher pass through, and it is the only place the scan sees the entries that
  *ship*: `pipeline::resolve_auto_glossary` merges the extracted harvest in
  after the translate boundary (the merge trims and length-caps extracted
  terms but does not strip control characters), so a control character in a
  **provider-supplied term** is named as well, and a run that replaced the
  loaded glossary wholesale no longer carries a claim about the one the file
  held.
- **`load_profile` records it without emitting it**, on
  `ProfileMetadata.load_warnings` — the record of what the file said — exactly
  as it treats the unknown-`{{placeholder}}` scan (ti ed8c57). The index is
  taken after normalization, so the record and the emitted line agree.
- **Behavior difference to expect**: one WARN line instead of three, and a
  document with nothing to translate no longer raises the advisory at all
  (`build_batches` returns before the gates for an empty unit list — the same
  cost every other warn-only check at that door already pays). The drop
  warnings are unchanged in text, count and channel.

`docs/architecture/contracts.md` §2, the Developer Guide, the Profile Cookbook
and `docs/Troubleshooting.md` said the control-character finding rides with the
three gates; all four now separate the unconditional escape and its
single-door advisory from the gates that drop.

### Fixed — a unit's cache identity tells apart every context its prompt tells apart (2026-08-07)

Backstop over ticket `53d4956f`. **No API change, no gate change**: no type,
field or signature moves and the contracts.md §0 table is untouched.
**Cache identity moves for every unit that carried context** — see the last
bullet.

`pipeline::context_hash` built its buffer by concatenating field values around
fixed slot markers, which made the buffer ambiguous in two ways the wire is
not:

- **An empty value read as an absent one.** A `document_title` of `None` and
  one of `Some("")` both contributed zero bytes, so they hashed identically —
  while `llm::prompt::ContextHints` skips the field only when it is `None`, so
  one prompt says `"document_title":""` and the other says nothing about a
  title at all. The state is reachable, not theoretical: a document whose first
  level-1 heading is a bare `#` has an empty title. Two units whose prompts
  differ shared a cache key, so a translation produced under one context could
  be replayed for the other — the wrong-output class the identity rule (owner
  decision 2026-08-06, ti `c02f69`) exists to close.
- **Content read as structure.** Section-path entries were terminated by `0`
  bytes, so one heading whose text spelled out `a\0\2\0b` produced the exact
  byte string of two headings `a` and `b`.

The encoding is now self-delimiting rather than delimiter-scanned. Each
optional value is introduced by a presence byte that is not a slot marker, and
every variable-length value — heading text, neighbor excerpt, and the neighbor
kind label — is length-prefixed, so no arrangement of field *contents* can
reproduce the byte string another arrangement of *fields* produces. A property
test walks a set of structurally distinct contexts, including the two
adversarial pairs above, and asserts every pair hashes differently.

- **`VALIDATION_SCHEMA_VERSION` is deliberately not bumped.** None of its three
  documented triggers fires: the instruction text, the `InputMode` payload
  semantics and the `UnitResult` semantics are all untouched. This is a
  cache-identity change, and `context_hash` is the lever for it.
- **A context-free unit keeps its entry.** The absent case is still zero bytes
  per slot, so a `BlockContext::default()` still hashes the three bare slot
  markers; the pinned value is unchanged. Its prompt did not move, so orphaning
  it would buy nothing.
- **Cache consequence.** Every cached unit that carried *any* context — a
  title, a section path, or a neighbor — misses once after this lands and is
  retranslated, because its buffer now carries presence bytes and length
  prefixes it did not before. Nothing on the wire changes: the prompt bytes,
  the goldens and the instruction text are all byte-identical.

### Fixed — the prompt-template warning now describes the body the run sends (2026-08-07)

Backstop over ticket `ed8c572f`. **No API change**: no type, field or signature
moves and the contracts.md §0 table is untouched. This is a **relocation** of
one `tracing` warning, plus the removal of one false line.

`profile::load_profile` scanned the `[system].prompt` it had just parsed for an
unknown `{{placeholder}}` and emitted the finding immediately. But the CLI's
`--system-prompt` / `--system-prompt-file` replace `prompt_body` *after* the
load, so a run that put a clean override over a typo'd profile still printed
`unknown template variable {{target_lang}} … will be sent to the model
literally` — a claim about a body that run was not going to send, and one
nothing downstream could retract. ti `ed8c572f` closed the dangerous direction
(a typo arriving through an override used to reach the model in silence); this
is its mirror image.

- **One emitter.** The translate boundary — the funnel `translate` /
  `translate_with_cache` both pass through — scans `prompt_body` and reports
  it, unfiltered. It is the only place that knows which of the three doors
  (profile TOML, either override flag, a direct assignment to the public field)
  last wrote the body.
- **The loader records without emitting.** `ProfileMetadata.load_warnings`
  still carries the finding, because that field is the record of what the
  *file* said. The boundary's report is a first print, not a second one, so the
  R0001-0032 no-double-print rule holds: a typo that survives to the wire is
  named exactly once whichever door installed it.
- **A removed typo removes the warning.** Replacing a typo'd profile body with
  a clean `--system-prompt` now prints nothing.

`docs/architecture/contracts.md` §2, the Developer Guide and Troubleshooting
described a loader that speaks first; all three now name the boundary.

### Fixed — the doubled-prompt diagnostic now stands at the batching door, not the translate boundary (2026-08-07)

Backstop over ticket `28110fc0`. **No API change**: nothing moves on the
curated facade, `unit` is `#[doc(hidden)]` and not re-exported, and the
contracts.md §0 table is untouched. This is a **behavior change on
`unit::build_batches`** for a caller who batches without crossing the translate
boundary, and a **relocation** of one `tracing` warning for everyone else.

A `ProfileMetadata` that is compiled and *then* edited is a state no rewind can
reach — the sections a compile appended are identified by the fields that
rendered them — so the library names it instead of correcting it. That
diagnostic ran at the translate boundary only. `unit::build_batches` is
`#[doc(hidden)] pub` and compiles a prompt body of its own, so a caller batching
directly shipped the doubled system prompt in every batch with **nothing said at
all** — the same asymmetry the reserved glossary scope (ti 0ed6eb), the
`[batching]` zeros, and the claimed-once glossary rule (ti 5f6664) each had to
close at this door.

- **The count moved to the batching door**, immediately after the compile that
  produces the body every `TranslationBatch` carries. That door is the one both
  callers pass through, so the direct batcher is covered and a full run raises
  the warning **once** rather than twice — a second copy at the boundary would
  be the R0001-0032 double print.
- **It is also the only place the count can be true.** `resolve_auto_glossary`
  runs between the boundary and batching and can change the glossary; only the
  body `build_batches` compiles is the body that ships.
- **One implementation.** The counter is now
  `profile::stacked_prompt_section_warnings`, beside the section headers it
  counts, so the message and the rendered sections cannot drift.
- **Nothing changes for a profile in either state the type is meant to hold**:
  a template and its own compiled prompt are silent, as is a compiled profile
  whose glossary the gate above rewinds.

`docs/architecture/contracts.md` §2 and the Developer Guide said the translate
boundary takes this count; both now name the batching gate and say why.

### Fixed — the glossary's claimed-once rule now holds at the batching door too (2026-08-07)

Backstop over ticket `5f6664d4`. **No API change**: no type, field or
signature moves, `unit` is `#[doc(hidden)]` and not re-exported by the facade,
and the contracts.md §0 table is untouched. This is a **behavior change on
`unit::build_batches`** for a caller who batches without crossing the translate
boundary.

`normalize_glossary` ran at two gates — `profile::load_profile` and the
translate boundary. `unit::build_batches` re-ran the *other* two profile checks
(the reserved `conditional-on-section` scope, and the `[batching]` zeros) on the
`ProfileMetadata` a caller built or mutated after loading, but not this one. The
renderer was standing in for it: `format_glossary` skips an entry with an empty
term and `escape_for_quoted` neutralizes control characters on every render
path. Neither knows anything about "a source term may be claimed once" — that is
not a rendering property — so a profile carrying two conflicting entries for one
term reached this door and shipped **both** contradictory bullets in every
batch's system prompt, and both entries on `TranslationBatch::glossary`, with no
warning.

- **Batching is the third gate.** The drop is warn-only there (`tracing`,
  target `transync::profile`), like the `[batching]` knobs and unlike the
  reserved scope, which is a refusal; it sits below the empty-document early
  return for the same reason, because it names entries only a document with
  something to translate would have been translated under.
- **One implementation, two callers.** The drop *and* the ti 28110f rewind of
  an already-compiled `prompt_body` now live together in
  `profile::normalized_glossary_profile`, which the translate boundary and the
  batching gate both call — so a dropped entry can never leave a stale glossary
  section beside the effective one, at either gate.
- **Nothing changes for a profile that came through the loader or the
  boundary**, auto-glossary merge included: the merge dedupes on the same
  case-folded key, so the second pass finds nothing to say and the compiled
  body rides out byte-identical. *(Correction, 2026-08-07: that held for
  every entry the check **drops**, but not for the one shape it **keeps** — a
  control character in a field was reported without changing anything, so the
  new gate re-reported it. Fixed by the entry below; the compiled body was
  byte-identical either way.)*

`docs/architecture/contracts.md` §2 said batching "does not re-run this
normalization, and does not need to — see the renderer clause below"; the
renderer clause covers only empty terms and control characters, so the
justification was overclaiming. §2 and the Developer Guide now name three gates
and say which half of the guarantee the renderer actually carries.

## [0.3.0] - 2026-08-07

> **Note added 2026-08-07 — which review round this section cites.** Every
> `R0001-NNNN` id below is from `reviews/0001.md`, the round on disk
> (2026-07-15, 52 findings). The retired 2026-05-02 round reused the same
> `0001`–`0052` numbers for different findings and is cited only in `[0.2.0]`
> and older; see `reviews/README.md`.

The review-0001 hardening release. One review (`reviews/0001.md`, 52
findings) was triaged into 20 tickets, and resolving them surfaced 26 more;
all 46 closed across seven sequential implement-review waves on
2026-08-06/07, every ticket carrying an independent review. The workspace
test count moved 408 → 583 (0 failed / 3 ignored) over the release range.

This release **uses and closes the sanctioned 0.3.0 breaking window** opened
after v0.2.0. Exactly three curated-surface changes rode it, each landed in
lockstep with contracts.md §0 and `public_surface.rs`: `CacheKey` gained the
`instruction_hash` field (cache identity covers the prompt bytes the unit was
translated under), `transync::VALIDATION_REPORT_SCHEMA_VERSION` joined the
facade (tier a), and `ListTopologyEntry.depth` widened `u8` → `u32`. The next
sanctioned breaking window is 0.4.0.

Release gate (OI-0030): **NOT RUN for v0.3.0.** The trigger condition was met
(the range touches `transync-openai` and batching), but no API key was
available in the release session; the owner's stated release condition for
v0.3.0 — waves complete plus `scripts/smoke.sh` green (2026-08-06) — governed
the cut. Run `OPENAI_API_KEY=… ./scripts/smoke-live-gate.sh` against this tag
when a key is available and record the outcome here as a dated note.

Standing gates on the release commit: `scripts/smoke.sh` exit 0 (workspace
583 passed / 0 failed / 3 ignored; CLI stub suite 77/0/0; rustdoc gate over
all five library members; wasm size raw 1,641,070 ≤ 1,840,000 B / gzip
672,629 ≤ 760,000 B; CLI dry run, eight non-empty output files),
`scripts/test-browser.sh` 16 passed, `cargo publish --dry-run --workspace`
green, and the sibling consumer `resp-translator` compiles clean with no
migration.

### Tests — inline nesting is pinned where it was only measured (2026-08-07)

Ticket `f69e83`, the follow-up `07844dad` left behind. **No API change and no
behavior change**: the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched, `ParseError` gains no
variant, and no document that parsed before parses differently now.

`parser::MAX_BLOCK_NESTING_DEPTH` (128) bounds **block containers** — one per
blockquote level, one per list level. It never bounded **inline** nesting, and
nothing in the repository said whether that was safe. It was; but it was safe
by measurement in a scratch binary, so a comrak upgrade could have taken it
away with nothing going red.

- **The decision is that inline nesting stays unbounded, and it is now a
  stated posture rather than a silence.** Two measurements settled it. No
  cheap byte score bounds inline depth: `*a *a *a … a* a* a*` and
  `![![![ … ](u)](u)](u)` each nest one level per repetition with no run
  longer than a single character — so scoring *runs* of `*` / `_` / `[` is not
  a bound at all, while it would refuse an ASCII rule or a `/****/` banner
  inside a fenced code block, which cannot nest anything because a fence's
  content is never inline-parsed. Scoring *totals* is a real bound (depth is
  at most half the delimiter count) and a useless one: over the 123 Markdown
  files tracked here (`.ko.md` excluded), the deepest inline AST any of them
  parses to is **3** while the densest carries **3,860** delimiter characters.
  And there is nothing to bound against — see below.
- **A regression pin replaces the scratch measurement.**
  `parser::depth::tests::inline_nesting_never_reaches_the_call_stack`
  re-executes the test binary once per probe shape and walks 50,000 levels of
  it on a 256 KiB stack **inside a child process**. A stack overflow is an
  abort, so an inline walk that started using the call stack takes the child
  down and the parent reads a non-zero exit status — the ticket's requirement
  that this go *red* rather than merely kill the test binary. Five shapes: the
  ticket's three (a `*` run, a `[` run, an image with nested alt text driven
  through `render::render_source`) plus the two spaced spellings above, which
  reach twice the depth per delimiter.
- **The pin cannot pass vacuously.** The child prints a line the parent
  requires, so a child that ran no probe fails; and it asserts the inline AST
  depth its shape actually reached (25,001 / 1 / 25,002 / 50,001 / 50,001), so
  a comrak that stopped nesting fails too. Verified by mutation: making the
  probe's own walk recursive turns the parent red with the child's
  `stack overflow, aborting` attached, and so does a stale filter name or an
  unmet depth floor.
- **`the_block_scan_never_scores_inline_punctuation`** pins the other half —
  a 200-character `*` rule, a separator line and a `/****/` banner inside code
  fences, and all five probe shapes still score **zero** block containers, so
  the block guard is not what stops them and never starts refusing them.

**The measurement, for the record.** On a 256 KiB stack — a quarter of the
1 MiB a wasm module gets — `parse` and `render::render_source` both survive an
inline AST **2,097,153** levels deep. Comrak 0.27 keeps inline delimiters on a
heap stack, walks both formatters on an explicit `Vec`, and drives
`descendants()` off tree edges; its four genuinely recursive walks
(`collect_text` and the three footnote passes) are unreachable under
`parser::comrak_options`, which enables no `header_ids`, no heading adapter
and no footnote extension. By contrast the leanest possible recursive walk —
one `fn(node, &mut usize)`, ~30 bytes of frame per level in release — aborts
at **8,536** levels on that same stack, which is what makes 50,000 a trap and
not a formality.

### Fixed — `out.md` never contains a NUL, on the target side too (2026-08-07)

Ticket `d06c43`, the residual **OI-0034** left behind (`743d27f0` / `bc74f49`).
**No facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched; no type, field or
signature moved, and `ValidationLayer` gains no variant.

Since OI-0034 the *source* side has been NUL-free: `parser::parse` substitutes
U+FFFD before comrak sees the document, because CommonMark §2.3 requires it.
The *target* side had no such gate. `regen::regenerate` splices the provider's
bytes verbatim, so a `U+0000` in a translated payload reached `out.md` intact
while the rendered target pane — which goes through comrak — showed U+FFFD for
the same byte. Two outputs of one run, disagreeing about a character the
document contains.

- **Behavior change: a payload carrying a NUL now fails the unit** instead of
  being spliced. It is rejected at the `schema` layer before any content layer
  runs, so the unit takes the ordinary retry-then-fallback path — resubmitted
  verbatim under ADR-0009 with the rejecting layer and reason in the `retry`
  hint, and finalized as `fallback_source` if the provider keeps sending it.
  A fallback splices the block's own source bytes, which are NUL-free by
  OI-0034's fix, so the run still produces a whole document. Nothing else
  changes for any payload that does not carry the byte.
- **Rejection, not normalization**, and that was the decision the ticket
  existed to make. Substituting inside validation would change what "the
  payload bytes the provider returned" means for the `Cache` and for the
  `ValidationReport` — both are supposed to be literal — which is exactly why
  `743d27f0` did not fold this into the source-side fix. Rejecting keeps them
  honest and is symmetric with the source-side posture: a NUL is never allowed
  to desync or to leak, by whichever mechanism that side has.
- **The seat is the `schema` layer, and `batch_fault` still means envelope.**
  A JSON string may legally hold a `U+0000` and `out.md` may not, so the rule
  is the part of "the response is a well-formed payload" that Structured
  Output has no way to carry — `validate::schema::check_payload_bytes`, beside
  the ID-set classifier. A report row therefore reads `rejected_by: "schema"`
  with **no** `batch_fault` flag; that flag, not the layer, is what
  distinguishes an envelope fault, and the per-unit content budget is what
  pays (never `max_per_batch_schema_retries`).
- **An html unit is judged on its decoded segments.** Its wire payload is a
  JSON array, so the byte arrives escaped and a scan of the payload *text*
  sees nothing — while regen splices the decoded segments. The reason names
  the offending segment.
- **A cache entry written before this fix cannot replay a NUL, and
  `VALIDATION_SCHEMA_VERSION` is deliberately *not* bumped.** A cache hit is
  not a bypass: `process_one_batch` runs every hit back through
  `validate_batch` on every run (ADR-0015), which is exactly why that
  constant's bump rule says not to bump for validator tightening. So a
  pre-fix entry carrying the byte is rejected on the hit, its key is
  **evicted**, and the pristine unit is re-dispatched — a poisoned cache
  self-heals on the next run instead of costing that unit a permanent
  fallback. None of the constant's three bump triggers fires (no `CacheKey`
  axis, no `InputMode` payload-meaning change, no `UnitResult` field
  semantics change), and bumping anyway would orphan every existing entry to
  buy nothing.
- **Nine tests, five of them RED without the check.** Four in
  `validate::schema::tests` pin the predicate (markdown payload, every clean
  kind, an escaped NUL inside an html segment, and a literal NUL in an
  undecodable html payload — the raw scan the decode arm cannot replace);
  three in `validate::nul_payload_tests` pin the seat through `validate_batch`
  (layer, dropped payload, no batch fault, and precedence over the `Preserved`
  proof, which would otherwise blame payload fidelity for the wrong reason);
  and two `run_pipeline` tests assert the whole-run statement end to end — one
  on a live provider, with a failure message that is the defect itself
  (`out.md must never carry a NUL`), and one on a pre-seeded pre-fix cache
  entry, which is what turns the "no bump" reasoning above from prose into a
  pin. Fixtures are inline `&str` for OI-0034's reason: a NUL is exactly what
  an editor or a filter drops on the way to a checked-in file.

`docs/architecture/contracts.md` §3 carries the resulting contract next to the
source-side sentence, and §5's per-unit budget row names the new rejection;
OI-0034's Residual section is struck through and closed; `docs/backlog.md`
drops the decision item.

### Fixed — a failed auto-glossary extraction is reported once, on the channel that cannot hide it (2026-08-07)

Ticket `33e178`, deferred from `7f922fa1` (R0001-0032) because it needed an
owner decision rather than an edit. **No facade change** — the contracts.md §0
table and `crates/transync/tests/public_surface.rs` are untouched; no type,
field or signature moved.

- **The duplicate is gone.** Since the reference binary installs a `tracing`
  subscriber, an auto-glossary extraction failure printed twice on stderr at
  default verbosity: `WARN transync::pipeline: glossary extraction failed (…);
  proceeding with static glossary only` from `transync-core`, and
  `transync: warning: auto-glossary: extraction failed (…); static glossary
  only` from the CLI. One fact, two near-identical sentences, one run — the
  same shape R0001-0032 removed for profile load warnings and the per-unit
  output-ceiling lines.
- **The CLI's line is the one that stayed**, and `transync-core`'s
  `tracing::warn!` for a failed extraction is **deleted**. That inverts the
  usual rule (the library owns the record, the CLI adds only what a log record
  cannot know) and it is the only degradation in `transync-core` with no
  `tracing` site. OI-0026 requires a degraded run of an *explicitly requested*
  feature to be visible without `--verbose`; the CLI line is unconditional
  where a log record follows the level filter, so dropping it instead would
  have hidden the notice from an operator running `RUST_LOG=error`.
- **Nothing structural moved.** `ValidationReport.auto_glossary` still carries
  `status: "failed"` and the 512-byte-truncated provider diagnostic, so library
  consumers — who read the report, not the log — see exactly what they saw
  before. `AutoGlossaryStatus::Unsupported` keeps its `tracing::info!` and its
  `--verbose`-only visibility: it is a static property of the caller's
  `Translator`, and no release build of the CLI can reach it (that build's only
  translator is `TransyncOpenAI`, which answers `Ok(Some(_))` even for an empty
  harvest).
- **Pinned, not narrated.** A new `transync-core` test installs a hand-rolled
  capturing subscriber (no new dependency — `tracing-subscriber` stays the
  binary's alone) and asserts a failed run raises no `transync::pipeline`
  record, with the `Unsupported` record captured in the same test as the
  positive control. contracts.md §6 and DCR-0014 record the exception.

### Added — the publication roster and the version-bump obligation are welded, not narrated (2026-08-07)

Ticket `0e7506`, deferred from `c447700f` (R0001-0040) because that ticket's
scope named the manifests only. **Behavior-neutral**: no crate source changed,
the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched, and every manifest
this reads is byte-identical to what it was. What changes is that two facts
which were held by prose now fail a test run when the manifests drift from
them.

- **`crates/transync/tests/workspace_publication.rs`, four tests.** They read
  the root `Cargo.toml` and all six member manifests and assert: every
  `[workspace.dependencies]` entry naming a member requires exactly
  `[workspace.package] version` and points at the directory the member is
  listed at, and every member still inherits that version through
  `version.workspace = true`; every internal dependency edge in every member —
  including `[dev-dependencies]`, `[build-dependencies]`, and the
  `[target.…]` forms — is `{ workspace = true }` with no `path` / `version` /
  `git` / `registry` beside it, and the internal entries in the root table are
  exactly the ones some member reaches; the members carrying `publish = false`
  are exactly `{transync-wasm}` (ADR-0019); and every directory under `crates/`
  with a manifest is a declared `[workspace] members` entry, since a crate that
  never reached that list is one the roster tests cannot classify and
  `cargo build --workspace` does not build.
- **The hole it closes is the patch bump.** `[workspace.dependencies]`
  `version` does not inherit from `[workspace.package]`, and a minor or major
  bump that forgets it fails the next resolve — but `^0.2.0` still accepts
  `0.2.1`, so a *patch* bump leaves `cargo build --workspace` green while
  publishing a manifest that understates what its sibling needs. Verified by
  probe: bumping `[workspace.package] version` to `0.2.1` alone builds clean
  and turns `every_internal_requirement_equals_the_workspace_version` red. Ten
  such probes were run — bare path, `path` beside `workspace = true`, a
  dropped `publish = false`, a registry allow-list, a self-pinned member
  version, a requirement with no `version`, a stale root entry, a new member
  with no decision at all, and a crate directory missing from
  `[workspace] members` — each failing the one test it should.
- **It fails closed, and the roster is two-valued on purpose.** A
  `[workspace] members` glob, a member with no `[package] name`, or a `publish`
  value that is neither absent nor `false` is a panic that says why, never a
  skipped member — the `lib_rs_exports_nothing_the_documented_list_omits`
  precedent. The `PUBLISHED_MEMBERS` / `PRIVATE_MEMBERS` lists are asserted to
  cover the workspace exactly, so a new member is red until someone records
  which side it is on: publishing is cargo's default, and a crate that ships
  because nobody said otherwise is the failure this exists for.
- **Until now the only mechanical check was the 41-minute dry run.**
  `cargo publish --dry-run --workspace` verify-builds the shared graph once per
  package, which makes it a release step; these tests ride
  `cargo test --workspace` (and therefore `scripts/smoke.sh`) instead.
  `docs/project/release-checklist.md` step 17 no longer ends "Check them, do
  not rely on the build to." — it names the test that checks them — and step
  19's roster table names its mechanical twin.
- **`toml` joins `transync`'s dev-dependencies**, adding one edge to
  `Cargo.lock` and no package to the tree (it is already a `transync-core`
  dependency). DCR-0018 kept the facade surface scrape dependency-free, but
  what it refused there was an install-dependent external tool that skips when
  absent. Manifest TOML is dotted keys (`version.workspace = true`) and inline
  tables in one file — the shapes a line scan misreads — and a misread here
  would fail open.

Suites after the change: workspace 571 passed / 0 failed / 3 ignored (567
before, plus the four new tests), CLI stub suite 77, browser suite unchanged.

### Changed — the declared MSRV is a version that can actually build the tree (2026-08-07)

Ticket `23e76a`. **Behavior-neutral**: no public item moved, no signature
changed, the contracts.md §0 facade table and
`crates/transync/tests/public_surface.rs` are untouched, and every toolchain
that could already build this workspace still can. What changes is what the
workspace *promises* — plus the seven nested `if`s a truthful floor lets clippy
finally complain about.

- **`[workspace.package] rust-version` moves 1.85 → 1.88.** 1.85 was the
  edition-2024 floor and nothing else; it was never a version these crates
  could be built on. `transync-syntax`, `transync-core` and `transync-openai`
  use `if let … && …` let-chains, stabilized in 1.88, and a 1.87 rustc rejects
  them outright (`error[E0658]: 'let' expressions in this position are
  unstable`). Measured, not reasoned: `cargo +1.88 check --workspace --exclude
  transync-cli --all-targets`, `cargo +1.88 test --doc` over the same set, and
  `cargo +1.88 check -p transync-syntax -p transync-wasm --target
  wasm32-unknown-unknown` are all clean, and clippy's `incompatible_msrv` finds
  no std API in the library members above even the old 1.85 — the language
  feature is the whole of the gap.
- **The floor is the dependency graph's too, not only ours.** `lol_html 2.9.0`
  declares `rust-version = "1.85"` and uses let-chains regardless, so
  `cargo +1.87 check --ignore-rust-version` fails inside that dependency before
  reaching this workspace's code. Even a future style change that removed every
  let-chain from these crates could not lower the key back on its own — the
  same mis-declaration this entry fixes here, one level down.
- **`transync-cli` keeps its own 1.89 pin instead of dragging the floor up.**
  It is the only member that needs it — `File::{lock, try_lock}` is still
  `feature(file_lock)` on 1.88 — and moving the workspace to 1.89 would charge
  every library consumer for a binary they do not depend on. The gap is one
  minor version, and the CLI manifest already records why it exists (DCR-0021).
- **Seven nested `if`s collapse into let-chains, because the floor is what was
  hiding them.** `clippy::collapsible_if` suppresses a suggestion the declared
  MSRV cannot compile, so at 1.85 it stayed quiet about every `if cond { if let
  … }` in the tree; at 1.88 it says so, and `-D warnings` makes that a build
  failure rather than a note. The rewrites are in `transync-syntax`'s `walk`,
  `transync-core`'s `pipeline::dispatch`, `pipeline::report`, `profile` and
  `structure`, and two table-shape helpers in the SCN-02/SCN-03 scenario
  tests. Each is the mechanical collapse clippy prints — same conditions, same
  short-circuit order, same bodies — and the suites confirm it: workspace 567
  passed / 0 failed / 3 ignored, CLI stub suite 77, browser suite 16, all
  unchanged from before the edit.
- **The number is corrected wherever a living document quotes it.**
  `docs/Quick_Start.md`'s prerequisite table now says 1.89, because the step
  right under it is `cargo build --workspace`, and names 1.88 as the
  library-only floor; `docs/Troubleshooting.md`'s toolchain-too-old entry names
  both floors and the message cargo actually prints; `docs/architecture/
  mvp-scope.md` and OI-0020's cross-reference follow the manifest. OI-0020's
  problem statement stops naming a number at all — the risk it describes is
  "drift past the declared MSRV", which was never about 1.85 specifically, and
  its `Related` line already carries the number and the date it moved. Dated
  records that quote 1.85 as it stood then — ADR-0015's 2026-07-13 amendment,
  DCR-0020, `skeleton-plan.md`, the archived investigation notes — are left as
  written.

Nobody on 1.85–1.87 could build this workspace before the change either; the
difference is that cargo now says so at resolve time instead of failing partway
through `transync-core`.

### Changed — the pre-commit hook runs the rustdoc gate, and one file owns its crate list (2026-08-07)

Ticket `08ccd46d`, the owner decision `000e5a` split out rather than made: that
ticket found the hook carried no rustdoc gate at all — only `cargo fmt
--check`, `cargo clippy --all-targets --all-features` and the wasm gate — and
declined to add one mid-wave, since blocking every contributor's commit on a
new condition is a workflow change, not a drive-by edit. **The answer is yes.**
Repo tooling and documentation only: no crate source moved, the contracts.md §0
facade table and `crates/transync/tests/public_surface.rs` are untouched, and
nothing about a build's output differs. Contributors get one more thing that
can fail a `git commit`.

- **`scripts/hooks/pre-commit` now runs `RUSTDOCFLAGS="-D warnings" cargo doc
  --no-deps`** over the five library members, after the wasm gate. It is cheap
  beside the `--all-targets --all-features` clippy run already there — a warm
  re-document of the five crates measures about a second — and it catches the
  failure class it exists for, a public item's intra-doc link to a private
  sibling, at the commit that writes it instead of at the next smoke run. That
  class fired twice this arc: the `normalize_batching` links `2f6214b`
  introduced and `d8b1cac` had to de-link, and `transync-openai`'s eight
  `client` module links at `6eb1e1d`.
- **The crate list moved to `scripts/lib/rustdoc-gate.sh`, sourced by both
  callers.** The list was written by hand at DCR-0018 and widened by hand at
  DCR-0020, missing `transync-openai` both times; a second copy inside the hook
  would have been a third chance to miss a member, and a hook that gates fewer
  crates than the smoke run is the worst of the two. Both scripts now source
  one array and one prebuilt `-p a -p b …` argument list, next to
  `scripts/lib/workdir-guard.sh` under the same rule — the definition lives in
  one place, the callers read it there.
- **`smoke.sh` keeps the completeness check, which now covers the hook too.**
  It still walks `crates/*/` and fails the run when a member with a
  `src/lib.rs` is not named in the list; because that list is the one the hook
  sources, adding a library member to the shared file is all either caller
  needs. Its failure message points at the new location.
- **The hook fails loudly when the shared file is missing** rather than
  invoking `cargo doc` with no packages: an empty package list is not a smaller
  gate, it is a different one — it would document the whole workspace,
  bin-only `transync-cli` included.

`docs/Developer_Guide.md` stated in two places that the hook does not run the
gate; both are corrected in place, together with the release checklist's
description of what the hook covers on the release-prep commit.

### Added — an OpenAI adapter's HTTP surface can be pinned per instance, not just per process (2026-08-07)

Ticket `42c8e6d3`, filed while resolving `a60f07` — not a defect in that fix, a
residual it made visible. **Additive on `transync-openai` only**: the
contracts.md §0 facade table, `crates/transync/tests/public_surface.rs` and
`crates/transync/src/lib.rs` are untouched, no existing signature moved, and a
caller that never mentions the new items sees the same behavior as before.

Since `a60f07` the adapter resolves its surface once, in `new` / `try_new` /
`from_env`, from `TRANSYNC_OPENAI_API` or the model-name heuristic — which
closed the fingerprint-vs-request divergence, and for the CLI is entirely
sufficient. But the only way to *choose* a surface was still the environment
variable, which is process-global: a library host wanting two adapters on two
surfaces at once (an OpenAI-compatible proxy that implements only
`/v1/chat/completions`, alongside a real endpoint for an o-series model) had to
mutate the variable between the two constructions — `std::env::set_var`, which
is `unsafe` under edition 2024 and racy against any other thread. The
surface-pinning `client::call_chat_completions_api` /
`call_responses_api` were no answer either: bare functions, so they carry no
`fingerprint()` and cannot be handed to `translate()` as a `Translator`.

- **`TransyncOpenAI::with_api(api) -> Self`**, mirroring
  `with_reasoning_effort`: it sets the stored surface directly, overriding
  whatever the constructor resolved. Surface selection becomes configuration,
  like `model` and `base_url`, instead of ambient process state.
- **The cache namespace follows the pin, and is pinned by a test.** The surface
  was already one field read by both `fingerprint()` and every request path, so
  nothing had to be re-plumbed: a pinned adapter neither reuses nor poisons
  entries written under the other surface, and a pin that merely restates what
  the heuristic already chose namespaces identically to it. `tests/api_surface_identity.rs`
  — the binary that owns its own process precisely so it can mutate the
  environment soundly — now also asserts the reverse direction: with
  `TRANSYNC_OPENAI_API=responses` live, both call paths of a `with_api`-pinned
  adapter reach `/v1/chat/completions`.
- **`Api` is re-exported at the crate root** (`transync_openai::Api`, still the
  same type as `transync_openai::client::Api`). Pinning a surface should not
  require naming the module that owns the model-name heuristic.
- **`Api` implements `FromStr`, over exactly the environment variable's
  vocabulary.** `chat` — with the long-standing `chat_completions` /
  `chatcompletions` aliases — and `responses`, case-insensitive. Both paths go
  through one private parser, so a host's `--api` flag and
  `TRANSYNC_OPENAI_API` cannot drift apart; the parse error is the new
  `ParseApiError`. `Api::as_str` / `Display` give the canonical name back, and
  it is the same token the provider fingerprint carries.
- **One behavior change, in the widening direction:** sharing the parser gives
  the environment variable the `FromStr` trim, so `TRANSYNC_OPENAI_API=" chat "`
  now takes effect where it used to warn and fall back to the heuristic. Every
  value that worked before still works, unchanged; an unrecognized value still
  warns and still falls back.

`docs/architecture/contracts.md` §7 (constructor block, configuration-identity
paragraph, dispatch override) and `docs/Developer_Guide.md` (dual-API dispatch)
are updated in place.

### Fixed — a Chat content-policy stop is reported as one, not as a missing body (2026-08-07)

Ticket `0583a75a`, filed while resolving `3c9741bf` (which gave
`finish_reason: "length"` its explicit diagnosis) and deliberately left out of
it, since that ticket's criteria named `length` alone. **No API change, no gate
change, no wire change, no cache change, no new dependency**: the contracts.md
§0 facade table and `crates/transync/tests/public_surface.rs` are untouched, the
request bodies are byte-identical, and the fix is read-side only.

`chat::finish_reason_error` acted on `length` alone, so `content_filter` — the
provider stating that its own filter ended generation — was left in exactly the
shape `length` had been in. Such a reply carries a `message.content` that is
absent, null, or cut off where the filter fired: the first landed on
`MalformedResponse("Chat-Completions response had no
choices[0].message.content")`, the second on `prompt::parse_batch_output`'s
`MalformedResponse`. Both blamed the shape of the response for what the provider
had already labelled a policy stop, and neither told the operator anything they
could act on.

- **`finish_reason: "content_filter"` is now the diagnosis.** It reads
  "Chat-Completions generation stopped by the provider's content filter
  (finish_reason: content_filter): the reply was cut short by a content policy,
  not by a transport fault or a schema mismatch …". Unlike the ceiling
  diagnosis it carries **no remediation clause** — no request parameter this
  adapter sends moves the outcome, and the retry ADR-0009 prescribes is
  verbatim, so the message names the content the provider flagged instead of a
  knob that would not have helped.
- **Terminal, and argued as such.** `ProviderError::Other` →
  `TranslatorError::Other`, on the ADR-0009 argument in its strongest form: the
  resubmission is verbatim and identical content is what the filter acted on,
  so every attempt stops the same way; retrying would spend
  `max_per_batch_provider_retries` to reach the same stop under a message about
  attempts rather than about policy. `MalformedResponse` was terminal too, so
  no run that used to finish now aborts and no run that used to abort now
  retries — the message is the whole behavior difference.
- **Distinct from the refusal check, and not a parity gap.** A `message.refusal`
  (`R0008-0035`) is the *model* declining: it arrives with
  `finish_reason: "stop"` and carries a refusal string, and it still reaches its
  own branch untouched. A filter stop is the *provider* cutting generation and
  carries no such string. The Responses surface needs nothing here — it
  delivers its filter stops as `refusal` content segments, which already
  surface. `tool_calls` and `function_call` stay unhandled on purpose: this
  adapter sends no tools, so a request it shaped cannot elicit them.

`docs/architecture/contracts.md` §7 *Provider signals the adapter honors* is
corrected in place — its `Chat finish_reason` bullet listed `content_filter`
among the reasons that proceed to extraction — and now states both acted-on
reasons, their shared terminal classification, and the refusal distinction.

### Fixed — `transync-openai` joins the rustdoc gate, and its `client` module doc stops dropping links (2026-08-07)

Ticket `000e5ab0`, found while self-reviewing `3c9741` and filed rather than
fixed there. **No facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched, no item's visibility
moved, and no runtime behavior differs. Rendered documentation and repo
tooling only.

- **The standing rustdoc gate named four crates when five have libraries.**
  `scripts/smoke.sh` ran `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` over
  `transync-syntax`, `transync-core`, `transync` and `transync-wasm` — a list
  written by hand at DCR-0018 and widened by hand at DCR-0020, with
  `transync-openai` left out both times. It is a published library member with
  a `pub mod client` and a `pub mod error`, and nothing was watching its docs.
  Run against it, the gate failed with **eight** errors.
- **All eight were `client`'s module doc linking to its own private
  submodules.** The "# Layout" list linked `dispatch`, `chat`, `responses`,
  `endpoint`, `transport` and `classify` — every one of them a `mod`, not a
  `pub mod` — and the paragraph under it linked the private `SurfaceRequest`
  and `round_trip`. Rustdoc renders such a link as bare text, so the list read
  as if it were navigable and was not. Following the `d8b1cac` precedent the
  prose now **names them in plain code spans** rather than widening eight
  internals to make the links resolve, and says in one line why they are
  unlinked. `[`Api`]` stays a link — it is `pub use`d.
- **The gate list is now checked for completeness, not just widened.** Adding
  one crate fixes today's hole and leaves tomorrow's: `smoke.sh` now walks
  `crates/*/`, and any member with a `src/lib.rs` that is not in
  `RUSTDOC_GATE_CRATES` **fails the run** with the names and where to add them.
  `transync-cli` is outside the gate deliberately — it is bin-only, with no
  public API to document — and, having no `src/lib.rs`, the check agrees.
- **The pre-commit hook does not run the rustdoc gate, and now says so.** The
  ticket assumed it did (it carries the *wasm* gate). A comment beside that
  gate records where the rustdoc one lives, so the next reader does not commit
  in the belief it was checked. `docs/Developer_Guide.md` — which had gone on
  calling this "the three library crates" after the fourth joined — and
  `docs/project/release-checklist.md` both name the five-crate set and the
  reason `transync-cli` is out.

### Changed — the list-topology fingerprint tells deep items apart again (2026-08-07)

Ticket `0d827781`, follow-up to `d7658be4` / Review-0001 `R0001-0025`. **This is
a facade change** — one of the sanctioned 0.3.0 surface-window changes, and the
only **breaking** one: `transync::ListTopologyEntry.depth` is now a **`u32`**
where it was a `u8`. The §0 row is unchanged (rows name items, not field types),
so the lockstep edit is contracts.md §0's prose — which now records the owner
decision and welds the new width — together with
`crates/transync/tests/public_surface.rs`, in one commit.

`d7658be4` fixed the overflow that let ~300 lines of indented `- item` panic a
checked build and wrap `255 → 0` in a release one, but it could not widen the
field: the curated surface was frozen. It saturated into the byte instead, at
`MAX_FINGERPRINTED_LIST_DEPTH`, which bought safety at the price of signal —
past 255 levels the depth field stopped discriminating and the comparison
leaned on entry count, entry order, `child_kinds` and the marker facts alone.

- **`ListTopologyEntry.depth: u8` → `u32`.** Depth is exact at any nesting.
  `structure::fingerprint_depth`, `MAX_FINGERPRINTED_LIST_DEPTH` and the
  `tracing::warn!(target: "transync::structure", …)` that announced the ceiling
  are **deleted**, not deprecated — the walker's `u32` counter now lands in the
  entry unchanged, so there is nothing left to project, cap or warn about.
- **Why it earns the slot.** `parser::MAX_BLOCK_NESTING_DEPTH` (128) refuses an
  over-nested *source* at the front door, but a **provider result** is
  fingerprinted by the same walker without passing through `parse`. The side
  the ceiling blinded was the untrusted one, in the function whose job is to
  catch a reshape.
- **Migration.** Anyone who named the type adjusts: `let d: u8 = entry.depth;`
  becomes `u32`, and integer-typed `match` arms or comparisons against
  `entry.depth` widen with it. Nothing else moves — field order, the remaining
  field types, `PartialEq`/`Eq`, and the outermost-list-is-depth-1 convention
  are all unchanged, and any depth an actual document reaches compares exactly
  as before.
- **No wire change.** The prompt hint's private `ListTopologyHint.depth`
  widened alongside it, but it serializes as a JSON number either way, so no
  request payload differs at any depth a real document reaches.

Tests: `structure::depth_ceiling_tests` now asserts the **whole** 1..=300
sequence at 300 nested levels instead of a saturation point, and the surface
gate constructs a `ListTopologyEntry` with `depth: 300` — a value that does not
fit a `u8` — so re-narrowing the field is a red compile.

### Added — the validation report's version is a constant a consumer can name (2026-08-07)

Ticket `8a4a3288`, owner decision 2026-08-06. **This is a facade change** — one
of the sanctioned 0.3.0 surface-window changes, and a purely **additive** one:
`transync::VALIDATION_REPORT_SCHEMA_VERSION` is a new §0 row (tier a), nothing
moved or was removed, and no existing consumer is affected. The lockstep edit
landed in one commit: contracts.md §0 and §3a, the facade's re-export list, and
`crates/transync/tests/public_surface.rs` (`DOCUMENTED` + `first_class`).

`ValidationReport` has carried its own artifact version since `R0001-0027`, but
the constant behind it was `pub(crate)` in `transync-core`, so the only way to
learn the version a build emits was to run a translation and read
`report.schema_version` — or to hard-code the string. The sibling
`ALIGNMENT_SCHEMA_VERSION` has been on the facade all along; this restores the
symmetry between the two durable artifacts.

- **`transync::VALIDATION_REPORT_SCHEMA_VERSION: &str`** (currently `"1.0.0"`)
  is the version *the linked build emits*. A consumer can pin or branch on it at
  compile time.
- **It does not replace the serialized field.** contracts.md §3a now states the
  division explicitly: the `schema_version` key discriminates *an artifact*
  (which may have been written by another version and is the only honest
  signal when reading off disk); the constant declares *this build's* output.
  Branch on the field when interpreting a file, read the constant when asserting
  what you produce — never the reverse.
- **Still not `VALIDATION_SCHEMA_VERSION`.** That one is a `CacheKey` axis over
  the prompt/payload contract; the two are independent and neither implies the
  other. §3a and both rustdoc entries keep saying so, now with the confusable
  pair sitting adjacent in the §0 table.

One test: a value-level pin in the surface gate that the exported constant
equals the `schema_version` the engine stamps into a report — asserted from
outside the defining crate, so the constant cannot describe a report shape
nothing writes. It deliberately does not pin the literal, so a version bump
stays one edit.

### Changed — a cached translation carries the instruction its batch was translated under (2026-08-07)

Ticket `c02f6938`, Review-0001 finding `R0001-0005`. **This is a facade change,
and a breaking one** — one of the sanctioned 0.3.0 surface-window changes.
`transync::CacheKey` is exhaustive by policy (contracts.md §1), so its new
`instruction_hash: u64` field breaks any code that builds a key with a struct
literal or destructures one without `..`. The lockstep edit landed in one
commit: contracts.md §0/§1/§5a, `crates/transync/tests/public_surface.rs`, and
the key itself. The facade's re-export list is unchanged — `CacheKey` was
already exported, and no §0 row moved.

**No shipped cache is invalidated.** `InMemoryCache` is the only implementation
in the workspace and it never outlives its process, so the new axis costs
nothing today; a future disk-backed cache will read entries written under the
old shape as absent, which is the safe direction (a miss, never a wrong hit).

- **A unit's identity now covers the instruction its batch assembled.** The
  user-message instruction carries an html-segment contract that rides only on
  a batch holding a raw-HTML unit, so the same paragraph translated beside an
  HTML block and translated among plain prose were two different requests
  sharing one cache entry. `instruction_hash` separates them.
- **Cohort membership is otherwise NOT identity.** The axis digests the
  assembled instruction *bytes*, not the batch, so reordering a batch, swapping
  a peer, or any co-batching that leaves the instruction identical leaves the
  key identical and the entry reusable. Hashing the cohort itself was rejected
  for exactly this reason: it would have thrown away most cache reuse to record
  something the model never saw.
- **The principle behind it is now written down** (owner decision 2026-08-06):
  a key's content axes cover *exactly what landed in the prompt bytes for that
  unit* — if it changed the prompt bytes the model saw, it is identity; if it
  did not, it is not. contracts.md §5a states it, names the three namespace
  axes it does not govern (`provider_fingerprint`, `model_id`,
  `validation_schema_version`), and records the two deliberate exclusions: the
  unit's `BlockId` (cross-block dedup is the point) and the ADR-0009 retry hint
  (keying on it would cache retry work under a key no first dispatch looks up).
- **One derivation, not two.** The digest hashes what
  `llm::prompt::instruction_text` returns — the same source of truth the packer
  prices (ti `aa92d64f`) — so the instruction the request embeds is the
  instruction the key names, by construction. Its resolution is the batch as
  the packer built it, for the whole run; §5a records why a round-level
  resolution is neither available (the cache lookup decides round membership)
  nor wanted (a key that moved mid-run would desynchronize the eviction map).
- **`CacheKey`'s field set is now a standing gate.** `public_surface.rs`
  constructs and exhaustively destructures the key from outside the defining
  crate, so the next axis added, removed or renamed is a red compile in the
  surface test rather than a silent break in a downstream disk-backed cache.

Four tests: the acceptance criterion in one test (an html-bearing cohort moves
the key, a reshuffled markdown cohort does not, and the instruction axis is the
only field that co-batching can reach), an eight-variant pin that the digest
moves exactly when the instruction bytes do, an end-to-end pin that a run's
cache entry is filed under the instruction its batch assembled and misses under
any other, and the new surface-gate destructure.

### Added — the HTML bundle carries the document's real title (2026-08-07)

Ticket `0f26b50a`, closing STUB-062/063. **No facade change**: the contracts.md
§0 table, `crates/transync/tests/public_surface.rs` and the facade's re-exports
are untouched. **No alignment-map schema change**: `schema_version` stays
`1.2.0` — a document title is presentation, and the bundle assembler reads it
from pipeline state instead of from the durable wire shape.

An emitted bundle's `<title>` was the literal `transync` on every document ever
translated, because no title flowed out of the pipeline. One did, though: the
provider has read the source document's first H1 as
`BlockContext.document_title` since SL-01. It is now returned to the caller too,
so the page a reader opens and the model that translated it name the document
identically.

- **`transync translate --title <text>`** sets the bundle's `<title>`.
  Precedence is **flag > the source document's first level-1 heading > the
  `transync` literal** (owner decision 2026-08-06). The middle level is
  parser-plain text — `` # The `transync` **Guide** # `` titles a bundle
  `The transync Guide` — not a trimmed source line, so a content `#` survives
  and a closing ATX marker does not. A heading that renders to nothing is not a
  title and falls through; a blank `--title` is an argument error (exit 1)
  rather than a silent fall-through, matching the posture an explicit empty
  `--model` gets, and it is rejected before the input is read.
- **`<html lang>`** carries the run's target language (unchanged behavior,
  now documented as contract rather than as a placeholder).
- **`TranslationOutput::document_title: Option<String>`** is the new field the
  title travels on. `TranslationOutput` is `#[non_exhaustive]` (contracts.md
  §1), so the addition is non-breaking; consumers read it or ignore it.
- **The title is untrusted content** (invariant 7), so it is HTML-escaped, and
  the shell's placeholders are now filled in a **single pass**: a substituted
  value is never rescanned, so a heading that reads like `{{TRANSYNC_…}}`
  cannot pull another slot's value into `<title>`. Bundles from documents whose
  first H1 is absent are byte-identical to before.
- **Bundle-only**, like `--target-direction`: the title reaches neither
  `out.md`, nor the alignment map, nor the provider.

Eleven tests: three e2e CLI runs (each precedence level; an untrusted heading
lands escaped; a blank flag exits 1), three over the assembler (the title and
document language reach the shell, an untrusted title is escaped and never
re-substituted, the filler's own semantics), three over the flag/precedence
resolution, one over the extraction rule (the *first* heading, and only level
1), and one pipeline test pinning that the caller's title and the provider's
are the same value from the same extraction. Contract:
`contracts.md` §6 "Bundle title and language"; ADR-0006 carries a dated
amendment for the placeholder inventory and the single-pass fill.

### Added — `--strict-csp` hardens an emitted HTML bundle against remote loads (2026-08-07)

Ticket `14307f84`. **No facade change**: the contracts.md §0 table,
`crates/transync/tests/public_surface.rs` and the facade's re-exports are
untouched — the flag lives entirely in `transync-cli`. **No default-output
change**: a run without the flag writes a bundle byte-identical to every prior
version's, which is the point of the posture below.

Source Markdown is untrusted (invariant 7), so a remote image URL in it is a
tracking pixel: opening the generated bundle fetches it and tells its host when
and from which IP the document was read. OI-0018 accepted that in 2026-07 —
source documents are usually already-local copies whose remote images are meant
to render — with an explicit re-open-if-the-posture-changes marker. The owner
amended the posture on 2026-08-06: the default stands, and users translating
sensitive documents get an escape hatch instead of a choice between the default
and editing their source.

- **`transync translate --strict-csp`** stamps a Content-Security-Policy
  `<meta>` into the emitted bundle's `index.html`, directly after the charset
  declaration:
  `default-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'`.
  That is the minimal policy under which the bundle still works — `'self'`
  covers `purify.min.js`, `sync.js` and the three fetched fragments,
  `'unsafe-inline'` covers the shell's own inline `<style>` and module
  `<script>` (no nonce or hash is emitted, so it is honored rather than
  ignored), `data:` keeps inline images rendering — and everything remote
  stops loading. The visible cost is that legitimately-remote images no longer
  render.
- **Both bundle modes**, `--html-out` and `--out-dir`'s `html/`, go through one
  shell assembler, so the two cannot drift into different postures.
- **With neither**, no bundle is emitted and the flag does nothing; the run
  says so on stderr rather than accepting a security flag silently.
- `frame-ancestors`, `sandbox` and `report-uri` are deliberately absent: a
  `<meta>`-delivered policy ignores them.
- The static demo shells under `web/` are untouched (dev demos over repo-local
  fixtures), and so is the embedded `sync.js` byte-twin.

Five tests: three CLI-smoke runs (the policy is present verbatim; the strict
shell minus that element **is** the default shell, byte for byte; `--out-dir`'s
bundle carries it too; plus the no-bundle notice) and two over the shell
assembler (the element is the only difference the flag makes; the strict shell
declares the policy inside `<head>`, carries every directive the bundle needs,
and leaves no template slot unsubstituted). Contract: `contracts.md` §6
"Bundle Content-Security-Policy"; `docs/project/open-issues.md` OI-0018 records
the posture update.

### Fixed — batching enforces the same profile gate the translate boundary does (2026-08-07)

Ticket `0ed6eb34`. **No facade change**: the contracts.md §0 table,
`crates/transync/tests/public_surface.rs` and the facade's re-exports are
untouched. The signature that changes belongs to
`transync_core::unit::build_batches`, which is `#[doc(hidden)]` and reachable
only by depending on `transync-core` directly — visible to hidden-API users
(currently `transync-openai`'s live-smoke test), which is why it is recorded
here.

The reserved `conditional-on-section` glossary scope is refused at
`profile::load_profile` and — since R0001-0006 — at the translate boundary,
because `ProfileMetadata` is `Deserialize` with public fields and a profile
built in code never crosses the loader. `build_batches` was the door neither
gate covered: it compiles a prompt body of its own, so a caller batching
directly still got the section-scoped entry rendered into every unit's system
prompt, which is a term meant for one section steering the whole document.

- **`unit::build_batches` now returns
  `Result<Vec<TranslationBatch>, ProfileError>`** and raises
  `ProfileMetadata::ensure_supported` on `opts.profile` — the same value, under
  the same `Some` guard, as `pipeline::run_pipeline` — so one profile defect
  has one error whichever door it came through.
- **The gate runs first**, ahead of unit construction and therefore ahead of
  the empty-document early return, because the boundary raises it before it has
  even parsed the source. A profile the boundary refuses is refused here for
  every document, including one with nothing to translate. The early return
  still precedes profile *rendering* and budget resolution, so the
  `[batching]` normalization warnings stay off documents that pack nothing.
- **Callers**: `run_pipeline` propagates with `?` (a formality — it cleared the
  same check at the top of the function); `transync-openai`'s
  `live_source_fixture_has_the_expected_shape` unwraps, its `opts` carrying no
  profile.

Three tests, in `unit::profile_gate_tests`: a programmatic section-scoped
profile refused by both doors with a byte-identical message and the
`profile_failed` stable code, the same pair on a document that produces no
batches at all, and a control in which the same entry with the shipped
`global` scope batches and still reaches the compiled prompt. The translator
in all of them panics on any call.

### Fixed — deeply nested Markdown is refused instead of aborting the process (2026-08-06)

Ticket `07844dad`. **No facade change**: the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched. `ParseError` gains a
variant, which is additive — the enum is `#[non_exhaustive]` (contracts.md §1).

A Markdown block container costs one byte per level to write, so `>>>>…` is
twenty thousand nested blockquotes in twenty kilobytes, and source Markdown is
untrusted by construction (invariant 7). A Rust stack overflow is an **abort**,
not a panic: nothing catches it, so a library host died with it.

- **`parser::parse` refuses a document nested deeper than
  `parser::MAX_BLOCK_NESTING_DEPTH` (128 block containers)** with the new
  `ParseError::TooDeeplyNested { depth, limit }`, from a linear,
  allocation-free pre-scan that runs on the caller's own bytes *before* comrak,
  `normalize_source`, or anything else touches them. comrak 0.27 was checked
  first and exposes no nesting or recursion knob — its one internal ceiling
  (`MAX_LIST_DEPTH = 100`) caps how many list containers a *single line* may
  open, so a list nested one level per line walks past it and blockquotes are
  unconstrained — so the bound had to be ours.
- **Behavior difference to expect**: a document whose block nesting exceeds 128
  levels now fails to parse rather than being translated (CLI exit code `2`).
  Nothing shallower changes: the pre-scan scores a line's `>` markers, its list
  markers, and `min(indent columns / 2, deepest score so far)`, so a line
  carrying **neither** a blockquote marker nor a list marker can never raise
  the score — deep indentation on its own (a wide code block, pretty-printed
  HTML, a spreadsheet pasted into a fence) is inert. Raising the bound takes
  129 marker lines each out-indenting the last, or one line with 129 `>` on it.
  A list marker is closed by a space, a tab, **or the end of the line** — a
  bare `-` on its own line is an empty CommonMark list item and opens a level
  like any other, so it is counted (Review-0002: it was not, and because a line
  with no counted marker cannot raise the running maximum, a document written
  entirely in empty items scored zero however deep it nested).
- **`structure::inspect_list_topology` now walks on an explicit stack.** This
  was the actual abort site, and it is reachable *without* passing through
  `parse`: the same walker fingerprints a provider's translated payload. The
  rewrite is order-preserving and the existing 300-level fingerprint tests pin
  it.

**A correction to the record.** The walker's R0001-0025 comment said comrak's
own parse "survives five thousand levels and exhausts the stack by ten
thousand". Measured on this workspace at a test thread's 2 MiB stack, comrak
0.27's block parse is iterative and survives **400,000** nested blockquotes and
5,000 nested list levels untouched; what aborted was the recursive walker on top
of it — two frames per level, ~950 bytes of them — at **2,193** levels on 2 MiB
and **1,100** on the 1 MiB a wasm module gets. 128 sits an order of magnitude
above any document written for a reader and an order of magnitude below the
shallowest measured abort, on the smallest stack in the system.

Twelve tests, including the ticket's own reproduction (20,000 blockquotes on one
line) asserting the typed rejection, a 128-level document that still parses, a
300-level document of empty items that does not, and a 1,000-level fingerprint
walked on a deliberately 256 KiB stack — where the recursive form aborted at
300.

### Fixed — a NUL byte in the source no longer desyncs the sourcepos→byte mapping (2026-08-06)

Ticket `743d27f0`, **OI-0034**. **No API change, no gate change**: no type, field
or signature moves, and the contracts.md §0 facade table is untouched.

CommonMark §2.3 requires a parser to replace `U+0000` with the REPLACEMENT
CHARACTER, and comrak does it before it records a single `Sourcepos` — so its
byte columns were counted over text in which each one-byte NUL had become a
three-byte U+FFFD, while `parser::ranges` mapped those columns onto the bytes
the caller passed. Two bytes of drift per preceding NUL on the line.

- **`parser::parse` now performs the substitution itself**, before comrak is
  handed the document (owner posture, 2026-08-06: normalize at intake). The
  normalized string is what becomes `Document::source_text`, so comrak and the
  range table count the same bytes by construction and the agreement no longer
  depends on a clamp that was never written for it.
- **Behavior difference to expect, on a source containing NUL only**: `out.md`
  and the alignment map's `source_range` offsets carry U+FFFD where the input
  had a NUL, and `document_id` (a hash of the source) moves with them. The
  rendered panes already showed U+FFFD, because they render through comrak — so
  this makes the Markdown output agree with the HTML output rather than
  changing what a reader sees. Every other source is byte-for-byte unaffected:
  a document with no NUL is passed through borrowed, untouched.
- **Six tests, four of them RED without the substitution.** The fixture is an
  inline `&str` — a NUL is exactly what an editor or a filter drops on the way
  to a checked-in file — and its trailing whitespace is load-bearing: a
  block-level `Sourcepos` always ends where its line's content ends, which is
  exactly where the clamp lands, so a NUL fixture without it would have passed
  before and after. The pins assert an interior column and prove the clamp did
  not produce it.

`docs/project/open-issues.md` marks OI-0034 RESOLVED with the posture and
corrects its "one extra byte" impact estimate; DCR-0019 gains a dated
amendment. One residual is ticketed and out of scope: a NUL arriving in a
*translated payload* still reaches `out.md` verbatim (`d06c4349`).

### Fixed — the embedded default profile no longer instructs Korean renderings (2026-08-06)

Ticket `e7ae214e`, Review-0001 finding `R0001-0003`. **No API change, no gate
change**: no type, field or signature moves, and the contracts.md §0 facade
table is untouched.

The embedded default profile shipped two `[[glossary]]` entries — `agent` →
`에이전트` and `tool use` → `도구 사용`, both `scope = "global"`. A
`[[glossary]]` entry names a target *form* and carries no target *language*,
and this one file is the default for **every** language pair, so a run
translating into Japanese or French was told to render those two terms in
Korean — in every batch's system prompt, without the operator opting into
anything.

- **The shipped default now carries zero active glossary entries** (owner
  decision 2026-08-06). The two examples stay in
  `crates/transync-core/profiles/default.toml` as comments, alongside the
  `auto_glossary` and `[render]` examples that were already commented out:
  documentation of the feature that cannot reach a prompt. Uncommenting them,
  or writing your own, in a profile passed with `--profile` works exactly as
  before.
- **Behavior difference to expect**: a run with no `--profile` no longer has a
  glossary section on its system prompt at all. That moves the profile's
  prompt hash and its glossary hash, so such a run re-fetches its cached units
  once; the profile `version` stays `1.0.0` because those two hashes are
  already cache-key axes and no schema key changed. Runs under a custom
  profile are unaffected. For en→ko documents the model is no longer told
  those two forms, so their rendering is now its own choice.
- **Both halves are pinned.** One test asserts the compiled-in default's
  active glossary is empty *and* that no glossary section reaches the compiled
  prompt for `ko`, `ja` or `fr`. A second uncomments every commented-out
  example in the file — the file's convention is that prose comments start
  `# ` and examples start `#key` — and asserts the result loads clean, yields
  the two entries at `scope = "global"`, and turns `auto_glossary` and
  `[render].target_direction` on. An example cannot rot into invalid TOML
  unnoticed.

`docs/architecture/contracts.md` §2, the Developer Guide and Troubleshooting
say the shipped default's glossary is empty and why.

### Fixed — a compiled profile is rewound before its glossary is changed (2026-08-06)

Ticket `28110fc0`, the follow-up to `574a1947` (Review-0001 `R0001-0014`).
**No API change, no gate change**: no type, field or signature moves, the
contracts.md §0 facade table is untouched, and the single-compile bytes are
unchanged — `profile::render_prompt_body` over a template still produces
exactly what it produced before, so no cache entry is invalidated.

`R0001-0014` made compiling idempotent for a profile's *own* compiled output.
A profile that is compiled and then **edited** is a third state: the sections
a compile appended can only be identified by the fields that rendered them, so
changing `glossary` or `constraints` under a compiled `prompt_body` leaves the
earlier compile's sections in the body and the next compile appends the
current ones in addition — the `[constraints]` policy block twice, and a
superseded glossary alongside the live one, in every batch's system prompt.

- **The library no longer produces that state.** Two boundary stages change a
  glossary under the caller's profile — `pipeline::normalize_profile_glossary`,
  which drops entries that cannot mean what they say (`R0001-0015` /
  `R0001-0018` / `R0001-0019`), and `pipeline::resolve_auto_glossary`, which
  merges the auto-glossary harvest in. Both now rewind `prompt_body` to the
  template the compile consumed *before* replacing the glossary — possible
  there and only there, because the fields that rendered the sections are still
  in hand — and hand the profile downstream in **template state**, so
  `build_batches` and `CacheKeyContext::for_run` each compile one copy of each
  section from the one body. **Behavior difference to expect**: only for a
  caller who passed an already-compiled profile *and* either enabled
  `auto_glossary` or carried a glossary entry the boundary drops. That run's
  system prompt loses the duplicated policy block and the superseded glossary
  section, which changes its prompt hash and re-fetches its cached units once —
  onto the identity a template-state run has always used. A rewind whose
  glossary comes out unchanged is a no-op on the bytes.
- **The state a caller can still hand in is named.** The translate boundary
  counts the lines of the prompt this run compiles that are exactly a section
  header and warns on the `transync::profile` `tracing` target when one appears
  more than once. It measures what will be sent rather than guessing at
  provenance — two copies of a section are wrong however they got there — so it
  cannot fire on either state the type is meant to hold, the shipped default
  included.
- **Rewinding is byte-exact, never a guess.** The rewind is the inverse of the
  append: what it returns re-compiles to the compiled bytes, and it refuses any
  body a compile could not have produced. Locating a previous section by its
  header text was rejected: it would eat an authored template that happens to
  use the same words. So a profile edited outside the library still stacks, and
  the rule stands — compile from the profile `load_profile` returned.

`docs/architecture/contracts.md` §2 and the Developer Guide carry the rule and
the two guards.

### Fixed — the per-batch instruction is measured, not allowed for (2026-08-06)

Ticket `aa92d64f`, the third residual of Review-0001 finding `R0001-0013`.
**No facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched, and the prompt
goldens are byte-identical: the instruction the model receives has not
changed at all. What changed is how many tokens the packer thinks it costs.

- **`batch::FIXED_ENVELOPE_TOKENS` is gone.** The constant stood for the whole
  assembled user-message instruction, at 256 tokens. Under the default profile
  the instruction actually encodes to **364 tokens** (o200k), and **471** once
  a batch carries a raw-HTML unit — so the reserve undercounted by 108–215
  tokens per batch, in the unsafe direction: the packer believed it had room it
  did not have, which is the whole of what `R0001-0013` was about. The reserve
  is now the *encoded* serialized envelope `llm::prompt` assembles, so it also
  covers the JSON framing and the escaping the instruction's own quotes pick up
  on the wire. Nothing in the once-per-batch reserve stands for a string this
  crate can produce any more; what stays constant is fixed-shape framing whose
  size does not follow the run — the per-unit *input* JSON keys and separators
  (serialized by this crate, but identically for every unit, so they stay a
  tuned per-unit estimate) and the response envelope, which only the model
  writes. A containment test pins that the reserve plus the per-unit estimates
  still bound the whole assembled request.
- **The instruction has one source of truth.** `llm::prompt::instruction_text`
  assembles it from an `InstructionVariant` — which of the three conditional
  clauses ride — and `build_user_prompt` embeds exactly what the packer prices.
  A test walks all variants (`preserve_urls` × `preserve_code_identifiers` ×
  html) and asserts the priced string and the shipped string are the same bytes.
- **The batch-membership-dependent clause has a stated policy.** The raw-HTML
  segment contract rides only on a batch holding an html unit, which is what
  packing decides — so it is reserved on a **document-level** upper bound
  (owner decision, 2026-08-06): whenever the document holds at least one html
  unit, every batch of that document reserves the clause. An html-free document
  pays nothing; an html-bearing one over-reserves for its Markdown-only batches
  rather than under-reserving for its html-bearing ones.
- **The retry round still packs under the same budget** (ADR-0009, R0001-0012).
  `pipeline::dispatch` sees one batch, not the document, so `run_pipeline`
  computes the html verdict once over the whole population and hands it down —
  deriving it per batch would have given an html-free batch of an html-bearing
  document a smaller reserve than the round that packed it.
- **Visible effect:** a run packed to its budget now produces slightly more,
  slightly smaller batches than before — the same overestimate-is-cheap
  asymmetry `batch.rs` documents for the output expansion factor. Requests that
  previously sat a few hundred tokens over the intended input target no longer
  do.

### Fixed — every workspace member now says whether it publishes, and the five that do can (2026-08-06)

Ticket `c447700f`, Review-0001 finding `R0001-0040`. **No facade change** —
the contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched, no Rust source changed, and `Cargo.lock` is byte-identical.
Manifests and the release ritual only.

- **Internal dependencies carry a registry version requirement.**
  `transync-core → transync-syntax`, `transync → transync-core`,
  `transync-openai → transync` (plus its two test-only reaches back into
  `transync-core` / `transync-syntax`), `transync-cli → {transync,
  transync-openai}` and `transync-wasm → transync-syntax` were declared with
  `path` alone. `cargo publish` refuses a path dependency that gives it no
  version to substitute once `path` is stripped, so **every member that has an
  internal dependency was unpublishable** — which is every member except
  `transync-syntax`. `cargo publish --dry-run -p transync-core` stopped at
  "all dependencies must have a version requirement specified when
  publishing". The four internal members are now declared once in the root
  `[workspace.dependencies]` table as `{ version = "0.2.0", path = "crates/…"
  }`, and all eight declaration sites reference them with `{ workspace = true
  }`: one place to bump, directly under the `[workspace.package]` version it
  has to track, instead of a literal in each site. Inside the workspace `path`
  still wins, so nothing about how this tree — or the path-depending sibling
  consumer — builds has changed.
- **The publication set is a decision on the record, not an omission.**
  `transync-syntax`, `transync-core`, `transync`, `transync-openai` and
  `transync-cli` publish, in that dependency order; `transync-wasm` remains
  the one private member (`publish = false`, ADR-0019). The root manifest
  names the set and `docs/project/release-checklist.md` carries the ritual,
  including the part that is not guessable: while the crates are unpublished
  the dry run only works as `cargo publish --dry-run --workspace` (cargo
  resolves the members against each other through a temporary local registry),
  because a single-package `-p` dry run of a dependent looks for its sibling
  on crates.io and does not find it.
- **Still nothing is published**, and adding a real registry publish stays a
  change to the checklist with its own record. The checklist's version-bump
  step gained the four requirements that do **not** inherit from
  `[workspace.package]`, and the two remaining places that still said
  `Cargo.lock` is uncommitted — that step and the `wasm-bindgen` pin's
  rationale in the root manifest — were corrected; it has been committed since
  `6cf4164`, and the exact pin is kept for the resolves that never read a
  lockfile.

### Fixed — installing the hooks never takes someone else's hooks away, and the whole `.env` family stays out of the repo (2026-08-06)

Ticket `097c20c5`, Review-0001 findings `R0001-0038` and `R0001-0039`.
**No facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched; nothing in any
crate changed. Repo tooling only.

- **`scripts/install-hooks.sh` no longer writes `core.hooksPath`
  unconditionally.** It wrote the value on every run, so a contributor who
  had pointed the setting at their own hooks directory — or inherited one
  from global config — lost those hooks, including security ones, without a
  word. The installer now compares the *resolved* directory (so
  `scripts/hooks`, `./scripts/hooks` and the absolute path are all
  recognized as ours) and branches: unset installs and says so; already ours
  is a one-line no-op; anything else **refuses, changes nothing, and exits
  1**, printing the current value with its config scope and the three ways
  forward (chain both hooks, take over, or clear the value where it is
  defined). `--force` takes over deliberately and says which it did: a local
  value is copied to `transync.replacedHooksPath` before being overwritten,
  an inherited global/system value is left in place and merely shadowed by
  the new local one, and the exact restore command is printed either way.
  `--help` and an unknown-argument exit (2) came with it. The inert
  `.git/hooks` copy removal (2026-08-04) is unchanged and still runs on the
  idempotent path.
- **`.gitignore` covers the files that habitually carry `OPENAI_API_KEY`.**
  It listed `.env`, `.env.local` and `*.api_key`, leaving `.env.production`,
  `.env.development`, `.env.test`, `.envrc` and `.direnv/` committable
  against the repository's own secret policy. The list is now the whole
  `.env.*` family plus the direnv pair, with `.env.example`, `.env.sample`
  and `.env.*.example` un-ignored so example files can still be committed,
  and a comment saying to extend that allow-list rather than narrow
  `.env.*`. Nothing tracked became ignored (`git ls-files -i -c
  --exclude-standard` is empty).

### Fixed — two runs publishing the same outputs take turns, and the writer says what it leaves behind (2026-08-06)

Ticket `f41f1652`, Review-0001 findings `R0001-0034`, `R0001-0035` and
`R0001-0036`, recorded as **DCR-0021**. **No facade change** — the
contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched; the change is confined to the reference binary.

- **Concurrent publications serialize instead of interleaving.** Staging to
  `<name>.tmp.<pid>` kept two runs from colliding while they *wrote*, but
  nothing covered the publication: two processes aimed at the same
  `--output` / `--map` / `--html-out` paths could interleave their rename
  passes and leave a bundle whose `out.md`, alignment map and `index.html`
  came from different translations. Every directory a publication writes into
  is now held under an exclusive OS file lock for the whole span from the
  foreign-file guard through the last rename. Neither run fails — the second
  waits, and says so (`transync: note: waiting for another transync run …`).
- **Operator-visible:** the lock lives on a zero-byte
  `.transync-publish.lock` marker in each output directory (for `--out-dir`,
  beside the target, next to the staging and backup siblings), and it is
  **never deleted** — unlinking it would let a later run lock a fresh inode at
  the same path while a waiting run still held the old one. Both directory
  guards treat it as a transync file, so a republish is unaffected; tooling
  that enumerates a bundle directory should ignore it the way it ignores
  `.DS_Store`. The kernel releases the lock when the process exits, `SIGKILL`
  included, so a crashed run cannot block later ones.
- **`--out-dir` no longer claims to replace an existing target atomically.**
  Onto a fresh target it is one atomic rename. Replacing an existing target
  is crash-safe but not atomic: `rename` cannot replace a non-empty directory
  in place, so the old tree is moved aside first, and a reader looking between
  the two renames finds no target at all. Other transync runs no longer land
  in that window; unrelated readers (a web server, a build step) still can.
  The `--out-dir` help text, contracts.md §6, `persistence-and-files.md` and
  the README all say this now.
- **Staging temps from other runs are documented as preserved, not
  reclaimed.** contracts.md §6 promised that "stale `*.tmp.<pid>` leftovers
  from an interrupted prior run are removed automatically"; the scanner only
  ever removed temps carrying its *own* pid, because a foreign pid may belong
  to a live run whose staging deleting it would corrupt. The contract now
  states the real rule with its reason, and a run that leaves such a file
  behind prints one line naming the count, an example and the manual remedy
  (`rm <dir>/*.tmp.*` with no transync run active).
- `transync-cli` declares `rust-version = "1.89"` for the std file-lock API
  (`std::fs::File::{lock, try_lock}`); the library members keep the workspace
  floor.

### Fixed — every prompt body is diagnosed, and `auto` is the sentinel however it is typed (2026-08-06)

Ticket `ed8c572f`, Review-0001 findings `R0001-0020` and `R0001-0021`. **No
facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched.

- **A template-variable typo now warns whichever door installed the prompt
  body.** `{{target_lang}}` is neither a substituted variable nor a namespaced
  placeholder, so it reaches the model literally — and it warned only when it
  arrived in a profile TOML, because `--system-prompt` / `--system-prompt-file`
  overwrite `prompt_body` *after* `load_profile` has scanned it (and
  `ProfileMetadata` has public fields, so a library caller could assign one
  too). The scan now also runs at the translate boundary, on the body the run
  will actually compile, and prints as
  `WARN transync::profile: unknown template variable …` on stderr. It suppresses
  anything the profile already carries on `load_warnings`, so a typo in a
  profile TOML is still named **once**, not twice. The run still succeeds — this
  is a warning, not a new failure mode.
- **`--source-language ' auto '` is the sentinel, not a label.** The CLI
  validated a trimmed value and then forwarded the untrimmed one, so a padded
  `auto` compiled a prompt naming the literal padded label instead of the
  auto-detection phrase, keyed its own cache entries, and was stamped on the
  alignment map — while the HTML bundle's source pane, which trimmed on its own,
  treated the same run as auto-detected. Both labels are now trimmed of
  *surrounding* whitespace exactly once, where they are validated, and every
  consumer reads that value: a padded run is byte-identical to a bare one.
  `profile::render_prompt_body` tolerates padding and case in its own sentinel
  test as well, so a library caller gets the same answer.
- **Labels stay opaque (ADR-0013, amended).** Nothing in a label's interior is
  touched, no case is folded, no tag is parsed — `"Korean (formal, 존댓말)"`
  survives byte-for-byte. A whitespace-only label is still an argument error
  (exit 1), and `--model` is deliberately not normalized: it is a provider
  identifier, not a label. Cache note: a run that previously used a padded label
  will re-key, once.

### Fixed — the reference CLI now has somewhere to put the library's diagnostics (2026-08-06)

Ticket `7f922fa1`, Review-0001 finding `R0001-0032`. **No facade change** — the
contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched; no library crate gained a dependency, and none installs a
subscriber.

- **`transync` (the binary) installs a stderr `tracing` subscriber.**
  `transync-core` and `transync-openai` carry sixteen `tracing::warn!` sites —
  cache failures degraded to misses, provider backoff, batch-schema-fault
  routing, full-reparse fallback cascades, conflicting per-batch language
  detection, profile load warnings, an unusable `TRANSYNC_OPENAI_API`
  override — and every one of them wrote to a no-op sink in the reference
  binary, because nothing had ever installed a subscriber. They now print as
  `WARN transync::<target>: <message>` on **stderr**; stdout stays empty, so
  piped output and the written artifacts are unaffected. The dependency
  (`tracing-subscriber`, `fmt` + `env-filter`, no default features) is on
  `transync-cli` **alone**: choosing where events go is an application
  decision, and a library that made it would take it away from its callers.
- **`--quiet` / `--verbose` govern the new channel and the old one together.**
  `--quiet` sets the filter to `off` (the run is silent, as before);
  no flag means `warn` and above; `--verbose` means `debug` and above, which
  is what newly surfaces the `info` records — among them the auto-glossary
  preflight reporting that a provider cannot extract at all.
- **`RUST_LOG` layers over the flag-derived level instead of replacing it**, so
  `RUST_LOG=transync::pipeline=trace` widens one target and leaves the rest at
  `warn`. An unparsable directive is reported and skipped rather than
  discarding the whole variable. `--quiet` does not consult `RUST_LOG`.
- **Two diagnostics stopped printing twice.** With a subscriber installed, the
  CLI's own re-print of `ProfileMetadata.load_warnings` became a second copy of
  a sentence `transync::profile` already emits, and the per-unit output-ceiling
  warnings became two copies each — thirty stderr lines carrying fifteen facts
  on the SCN-14 fixture at a small ceiling. Profile load warnings now reach
  stderr through the `tracing` channel only (they lose the `transync: ` prefix
  and gain a level and a target); the per-unit ceiling sentences do too, and
  the CLI contributes one line naming the count and the flags that move the
  ceiling — `--target-output-tokens`, `--output-expansion-factor` — which is
  what a log record cannot know. `--validation-report` still carries every
  flagged unit structurally, unchanged.

### Fixed — the browser engine refuses an unusable alignment map, and duplicate anchors get one policy (2026-08-06)

Ticket `4f8dde0c`, Review-0001 findings `R0001-0043` and `R0001-0044`. **No
facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched; this is the vanilla-JS
sync engine (`web/js/sync.js` and its byte-identical CLI-embedded twin), not
the Rust API.

- **`mountSync` now gates the map's rows, not just its `schema_version`.** A
  map with no `blocks` array, a row missing the `source_block_id` everything is
  keyed on, a row whose `sync_role` this engine cannot classify, or two rows
  claiming the same id is refused, with a console message naming the row and
  the reason. Previously any of those passed the version check and left the
  engine pairing panes by whatever `data-sync-id` values the DOM happened to
  carry — a degradation indistinguishable from working sync until the two panes
  disagreed. Only what synchronization needs is required; ranges, orders,
  block kinds and fallback statuses are still nobody's business here.
- **Forward-minor drift keeps its exemption** (contracts.md §3): in a map whose
  minor/patch is newer than the engine's `KNOWN_SCHEMA`, an unrecognized
  `sync_role` is an added enumerated value — warned about and treated as a
  scroll anchor — while the same value in an in-band map is corruption and is
  refused. `crates/transync-cli/tests/sync_js_drift.rs` now pins the JS role
  mirror to the Rust `SyncRole` enumeration, so adding a variant cannot leave
  the engine rejecting maps the emitter legitimately produces.
- **A duplicate `data-sync-id` is now dropped from the active-block scan as
  well as from the partner lookup.** The two structures previously disagreed —
  the lookup kept the LAST occurrence while the scan kept every one of them —
  so the scan could select an early duplicate while the lookup answered with a
  far-away one, scrolling the follower pane to a block the user was nowhere
  near. First occurrence in document order now wins in both, and each drop is
  still warned about.
- **Nothing changes for a well-formed map and duplicate-free panes**, which is
  every bundle the pipeline emits (contracts.md §4a guarantees one element per
  anchored row). The refusal path is the existing one: panes stay unwired and
  an inert controller comes back.

### Fixed — a rendered task list carries the GFM task-list classes again (2026-08-06)

Ticket `98f9ecfd`, Review-0001 finding `R0001-0026`. **No facade change** — the
contracts.md §0 table and `crates/transync/tests/public_surface.rs` are
untouched; this moves rendered HTML, not the Rust API.

`transync-syntax::render` reconstructs the shared `<ul>` / `<ol>` that groups a
run of list items (DCR-0007) instead of letting comrak write it. The
reconstruction dropped the GFM task-list markers: the group came out bare and
every row came out as a bare `<li>`, so a rendered task list was recognizable
only by the checkbox `<input>` inside it, and CSS or an integration keyed on
`.contains-task-list` / `.task-list-item` — the classes GitHub-rendered
Markdown carries — matched nothing.

- **A group holding at least one task row now renders
  `class="contains-task-list"`**, on `<ol>` as well as `<ul>`, written before
  `start` so the two presentational attributes have one fixed order
  (`<ol class="contains-task-list" start="3">`).
- **Each task row now renders `class="task-list-item"` on its `<li>`**, ahead
  of the sync attributes.
- **Both classes key off the same paired `TaskItem` node that produces the
  checkbox**, so a row can never carry the class without the checkbox or the
  reverse — in either pane, and including the Guard-2 degrade path, which
  emits neither.
- **Nothing else in the output moves.** The sync-attribute set is byte-identical
  (`data-sync-id`, `data-block-kind`, `data-order`, `data-fallback`), the
  alignment map is untouched, and a list with no task row renders exactly the
  bytes it rendered before. The `<input>` itself stays as the pinned comrak
  writes it — GFM's third class, `task-list-item-checkbox`, is deliberately not
  emitted.

Known gap, **weighed and accepted within this same unreleased window** (ticket
`cfeb5df5`, 2026-08-07): only the reconstructed top-level group is marked. A
task list nested inside a list item is emitted by comrak whole, and the pinned
comrak (0.27) writes neither class and offers no option to — bumping the
parser that every block ID and reparse guard depends on, for two CSS classes,
is not a trade this makes. The rendered shape is now stated in contracts.md
§4/§4a instead: a nested task list comes back as a bare `<ul>`/`<ol>` of bare
`<li>`s that still carry the checkbox `<input>` and, as before, no sync
attributes, so **CSS that must reach every task row keys on the checkbox
rather than on `.contains-task-list` / `.task-list-item`**. Scroll sync is
unaffected — nested rows were never anchors — and no shipped stylesheet keys
on the classes.

### Fixed — hostile nesting and hostile columns no longer overflow the arithmetic that guards them (2026-08-06)

Ticket `d7658be4`, Review-0001 findings `R0001-0025` and `R0001-0045`. **No
facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched, and
`ListTopologyEntry.depth` kept its `u8` type here (widening it is a
curated-surface change, deferred to an owner decision as ticket `0d827781`).
*Superseded within this same unreleased window:* that decision came back
**widen**, so the `u8` field, the saturation and its ceiling are all gone
before any of this ships — see "the list-topology fingerprint tells deep items
apart again" below. What survives from this entry is the overflow fix itself.

Two additions on untrusted input were unchecked, in the two places whose job is
to make untrusted input safe.

- **List-topology depth counts in `u32` and saturates into the fingerprint.**
  `structure::inspect_list_topology`'s walker advanced a `u8` with `depth + 1`,
  so about three hundred lines of indented `- item` — a ~90 KB document, well
  inside what comrak parses, since its ~99-level cap applies only to blocks
  opened on a *single* line — panicked a checked build with "attempt to add
  with overflow" and wrapped `255` to `0` in a release one. The walker builds
  *both* sides of list validation (the expected topology at unit-construction
  time and the actual topology re-derived from provider output), so the wrap
  handed deep items a shallow-looking depth in exactly the function that is
  supposed to catch a reshape. The counter is a `u32` with a saturating
  increment from here on, which is four billion levels out of reach. (This
  entry originally projected that counter back onto the `u8` field by
  saturating at a `MAX_FINGERPRINTED_LIST_DEPTH` ceiling; ticket `0d827781`
  widened the field instead, so the projection, the constant and the ceiling
  warning never shipped. Depth is exact at any nesting.)
- **`LineOffsets::pos_to_byte` saturates before it clamps.** It computed
  `line_start + col.saturating_sub(1)` and only then clamped the result to the
  line's content end. It is public API in a public module, so `col` is whatever
  an arbitrary caller passes rather than only comrak's own small sourcepos
  columns, and `offsets` is a `pub` field a caller can corrupt: `usize::MAX`
  panicked a checked build and, in a release build, wrapped to a small
  in-range offset that the clamp then waved through as a *valid-looking* byte
  position. The add is `saturating_add` now, so an oversized column lands where
  an oversized column has always landed — the line's content end. The `+ 1` in
  `byte_range_for` is saturating for the same reason; it could not overflow
  today, but only because of a clamp two calls away. **No clamp was removed** —
  the ones OI-0034's NUL column drift leans on are exactly as they were, and no
  offset produced for any non-adversarial input changes.

### Fixed — the OpenAI adapter's API surface is resolved once, at construction (2026-08-06)

Ticket `a60f0746`, Review-0001 findings `R0001-0031` and `R0001-0010`. **No
facade change** — the contracts.md §0 table and
`crates/transync/tests/public_surface.rs` are untouched; the only API movement
is one additive accessor on `transync-openai`, which is not part of the curated
`transync` surface.

`TRANSYNC_OPENAI_API` used to be read afresh on every call, by
`TransyncOpenAI::fingerprint()` and by each request path independently, and the
code documented the process-lifetime stability it needed as an *assumption*
about the host rather than enforcing it. A library process that mutated the
variable between building a cache key and issuing the request could therefore
name one surface in the provider fingerprint and call the other — a cache
entry attributed to a surface that never produced it.

- **The surface is a field now.** `new` / `try_new` / `from_env` resolve it
  once (`TRANSYNC_OPENAI_API` when set to a recognized value, the model-name
  heuristic otherwise) and store it; `fingerprint()`, `translate_batch`, and
  `extract_glossary` all read that one field. Model, base URL, and reasoning
  effort were already fields, so the adapter's whole output-affecting
  configuration is now fixed at construction.
- **New: `TransyncOpenAI::api() -> client::Api`** — reports the resolved
  surface, so a caller can log or assert it instead of re-deriving it and
  hoping the two derivations agree.
- **Cache consequence: none for a stable environment.** The fingerprint hashes
  the same four parts in the same order; what changed is *when* the surface
  part is computed. Any process that does not mutate `TRANSYNC_OPENAI_API`
  mid-run — every CLI run, and every library host observed so far — produces a
  byte-identical fingerprint, so warm caches are not invalidated. A process
  that *does* mutate it now keeps its existing adapters on their original
  surface (and namespace) instead of silently switching them.
- **Behavior difference to expect**: the variable must be set *before* the
  adapter is constructed. Setting it afterwards no longer affects that
  instance; build a second adapter to change surface.
- The standalone `client::call_api` / `client::call_glossary_extraction` free
  functions keep resolving per call — they have no instance to remember
  anything in — and now say so; `client::call_chat_completions_api` /
  `client::call_responses_api` remain the pinned-surface entry points.

`crates/transync-openai/tests/api_surface_identity.rs` is the pin: it stands up
a loopback origin, mutates `TRANSYNC_OPENAI_API` after construction, and
asserts the fingerprint, the translation request, and the glossary-preflight
request all still land on the construction-time surface — while a
freshly-built adapter does honor the new value, so the test cannot pass
vacuously.

**Docs — what `TranslateOptions.model_id` governs, stated exhaustively.**
`R0001-0010` (the caller-owned model label can disagree with the translator's
real model) is closed as documented rather than as removed: its cache-safety
half was already covered by `provider_fingerprint`, and its encoder half by
`Translator::tokenizer_hint`. The field's rustdoc now enumerates the three
things it governs (encoder fallback *only* when the translator declares no
hint, the output-budget preflight under the same rule, and one `CacheKey`
axis), states plainly that it governs neither the model called nor the endpoint
nor any request field, and records why identity stays off the `Translator`
trait. `docs/architecture/contracts.md` §7 gains the construction-time
resolution rule and lists `try_new` and `api()`; the Developer Guide's
environment table and dual-dispatch section say when the override is read.

### Fixed — an exhausted output ceiling is named as one on Chat Completions too (2026-08-06)

Ticket `3c9741bf`, filed while resolving Review-0001 `R0001-0004`. **No API
change, no gate change, no wire change, no cache change, no new dependency**:
the contracts.md §0 facade table is untouched, the request bodies are
byte-identical, and the fix is read-side only.

The Responses surface has reported an exhausted output ceiling explicitly since
`R0008-0036` (`incomplete (reason: max_output_tokens)`). Chat Completions never
read `finish_reason` at all, so a reply stopped at `max_completion_tokens`
arrived as the schema-shaped answer cut off mid-object and failed downstream in
`prompt::parse_batch_output` as `MalformedResponse("model output_text was not
valid JSON …")`. The run was never *unsafe* — the truncated attempt is
rejected, so nothing invalid is cached or emitted — but the operator was told
their provider returned malformed JSON when in fact their own
`[batching].target_output_tokens` was too small, and the remediation was not
discoverable from the message.

- **`finish_reason: "length"` is now the diagnosis.** `chat::Choice` carries
  the field and `output_from_envelope` surfaces it before the payload is handed
  on, naming both the request parameter the provider enforced
  (`max_completion_tokens`) and the knob that set it
  (`[batching].target_output_tokens` / `--target-output-tokens`). That
  remediation is stated "on a translation batch", because the one envelope
  reader also serves the candidate-glossary preflight, whose ceiling is fixed
  and does not move with that knob. It is read
  before the message, mirroring the Responses surface where `status` decides
  whether extraction happens at all: a reply the ceiling cut off has nothing
  complete to give, a truncated refusal string included.
- **Terminal, and unchanged as such.** The error is `TranslatorError::Other`,
  matching what Responses `incomplete` already does for the identical
  condition and for the identical reason — the ceiling rides in the request, so
  ADR-0009's verbatim resubmission would truncate the same way, and ADR-0017
  keeps that run-terminal. `MalformedResponse` was terminal too, so no run that
  used to finish now aborts and no run that used to abort now retries.
- **Behavior difference to expect**: the abort message. What read "model
  output_text was not valid JSON for the response schema: EOF while parsing …"
  now reads "Chat-Completions output incomplete (finish_reason: length): the
  max_completion_tokens output ceiling was exhausted …". Every other stop
  reason — `stop`, `tool_calls`, an absent field, and any vocabulary a
  compatible gateway invents — still goes to extraction, so a reply that
  carries a usable body is never rejected on a label this adapter does not
  model. (**Corrected 2026-08-07**: this bullet listed `content_filter` among
  those reasons, which it was when written. Ticket `0583a75a`, landed later in
  this same 0.3.0 window, gave `content_filter` its own terminal diagnosis —
  see *Fixed — a Chat content-policy stop is reported as one, not as a missing
  body* above. What 0.3.0 shipped acts on `length` **and** `content_filter`;
  the reasons listed here are the ones that remain.)

`docs/architecture/contracts.md` §7 records the new diagnosis under *Response
limits and the output ceiling* and its retry classification under *Provider
signals the adapter honors*, and §1's *Output ceiling and cache identity*
paragraph is corrected — both had documented the Chat overshoot as
`MalformedResponse`, which is what *Docs — the output ceiling's absence from
cache identity becomes a decision* (below) wrote earlier the same day.

### Fixed — the OpenAI transport stops discarding two provider signals (2026-08-06)

Ticket `77ccf109`, Review-0001 findings `R0001-0029` and `R0001-0030`. **No API
change, no gate change, no wire change, no cache change, no new dependency**: the
contracts.md §0 facade table is untouched, the request bodies are byte-identical,
and both fixes are read-side only. Retry semantics (ADR-0009 / ADR-0017) are
untouched as well — a honored `Retry-After` changes *when* a retry is dispatched,
never what it carries, and the resubmission stays verbatim.

- **`Retry-After` in the HTTP-date form is honored.** `parse_retry_after` read
  the delta-seconds form (`120`) only and mapped every date to "no hint", so a
  provider or intermediary sending `Wed, 21 Oct 2015 07:28:00 GMT` got the
  pipeline's 200 ms-doubling backoff instead of the pause it asked for — and
  could keep re-earning the 429. All three RFC 7231 §7.1.1.1 date forms
  (IMF-fixdate, the obsolete RFC 850 form, and asctime) now parse, resolved
  against the system clock, hand-rolled rather than bought with a `chrono` /
  `httpdate` dependency. A deadline already at or behind the clock, and anything
  unparseable, still degrade to "no hint" — deliberately not a zero-second wait,
  which a skewed clock would turn into a hot retry loop. The pipeline's 30 s cap
  applies on top, unchanged.
- **Every Responses status that cannot carry an answer says so.**
  `output_from_envelope` branched on `incomplete` alone, so a `failed` or
  `cancelled` response fell through to the generic "no output_text" malformed
  error and the provider's status and error text were lost. The envelope now
  models the `error` object, and the status classification is explicit — and *is*
  the retry decision: `incomplete` stays terminal (the ceiling that truncated the
  answer rides in the request, so a verbatim resubmission truncates identically),
  while `failed`, `cancelled`, and the unsettled `queued` / `in_progress` are
  transient, bounded by `max_per_batch_provider_retries` exactly like a 5xx.
- **Behavior difference to expect**: a run that died instantly on a `failed`
  Responses envelope now spends its (default: one) transport retry first, and if
  that is spent the abort names the provider's status and error code instead of
  "Responses-API envelope had no output_text content". A rate-limited run pacing
  off an HTTP-date `Retry-After` now waits as instructed, up to 30 s.
- Unknown statuses still go to extraction, so an OpenAI-compatible gateway with
  its own status vocabulary keeps working; when such an envelope has nothing to
  extract, the diagnostic now names the status and the error object.

`docs/architecture/contracts.md` §7 gains a "Provider signals the adapter honors"
subsection recording both classifications.

### Fixed — a zero dispatch concurrency is reported, not silently made sequential (2026-08-06)

Ticket `e6ef28d4`, the follow-up filed while resolving Review-0001 `R0001-0017`.
**No API change, no gate change, no wire change, no cache change**: no type,
field or signature moves, the contracts.md §0 facade table is untouched, and the
CLI is unaffected in every respect.

`pipeline::run_pipeline` resolved dispatch concurrency with
`opts.max_concurrent_batches.max(1)`. A library caller passing `0` therefore got
a fully sequential run with nothing said — the same "the configuration mistake
runs under different semantics than it asked for" defect `R0001-0017` named for
the two `[batching]` sizing knobs, on the one knob of that shape those fixes did
not reach.

- **`0` now resolves as "unset", with the field named.**
  `TranslateOptions::max_concurrent_batches` accepts `>= 1`; a zero is reported
  on `tracing::warn` (target `transync::pipeline`) and then resolves as the
  built-in default `6`, which is what leaving the field alone would have done.
  Unlike the two `[batching]` knobs there is no profile value to fall back to
  first: concurrency is a runtime property with no profile home.
- **Behavior difference to expect**: a run that previously dispatched one batch
  at a time off a `max_concurrent_batches = 0` now dispatches six at a time. A
  deliberate `1` is untouched and still means fully-sequential dispatch.
- **The CLI never reached the old floor**: `--max-concurrent-batches` carries
  `clap::value_parser!(u32).range(1..)`, so only the library path could hand a
  zero in.

`docs/architecture/contracts.md` §2 and §6 and the Developer Guide's batching +
concurrency table now state the supported range for this knob too.

### Fixed — the validation report declares its schema, and every block list in it follows the document (2026-08-06)

Review-0001 findings `R0001-0027` and `R0001-0028` (ticket `096e0e0e`). **The
durable `validation-report.json` artifact changes shape** — one added root key
and one reordered array. **No cache change, no gate change, no facade change**:
`VALIDATION_SCHEMA_VERSION` stays `2`, `CacheKey` is untouched, not one cache
entry is orphaned, and the contracts.md §0 curated table and
`crates/transync/tests/public_surface.rs` are byte-identical.

- **The report carries a `schema_version`.** `ValidationReport` is serialized
  as the *root object* of the file `--validation-report` writes and of
  `--out-dir`'s `validation-report.json`, and it had no version discriminator —
  the sibling `AlignmentMap` has carried one since `0.1.0`. It is now the first
  key of every report, currently `"1.0.0"`, with §3a of contracts.md stating
  the compatibility rules (semver, reject unknown majors, accept unknown
  minor/patch, tolerate unknown keys). A report with **no** `schema_version`
  key at all was produced before this and reads as pre-`1.0.0`.
- **Two versions, two axes, neither derived from the other.**
  `VALIDATION_SCHEMA_VERSION` is a `CacheKey` component versioning the
  prompt/payload contract; it bumps for prompt framing and payload semantics.
  None of its three documented bump triggers fires for a change to this JSON's
  shape, so it is deliberately **not** bumped here and no cache entry is
  invalidated — a run after this change replays every entry a run before it
  cached. The new constant is engine-internal on purpose: the discriminator a
  consumer reads is the serialized field.
- **`full_reparse_fallbacks` is in document order.** It was sorted with
  `a.0.cmp(&b.0)` — a lexical sort over the `BlockId` string, which groups
  `c-`, `h1-`, `li-`, `p-` by kind prefix instead of following the document.
  OI-0021 fixed exactly this for the sibling `per_unit` field and the fix was
  never carried across. Both lists now come out of **one** ordering pass in
  `pipeline/report.rs`, run after the run-level fields are stamped, so the next
  list added to the report has a single place to join. A consumer that keyed on
  the old lexical order sees a different sequence; the ids and their meaning are
  unchanged.
- **Ordering is now written down as a contract**, not left as an implementation
  detail two fields disagreed about: §3a states that every block list in the
  report is in document (source) order, and both `ValidationReport` field docs
  say so.

### Fixed — a retry round is packed by the budget, and the estimator measures the language labels (2026-08-06)

Review-0001 findings `R0001-0012` and `R0001-0013` (ticket `24fd28b4`). **No API
change, no gate change, no wire change, no cache change**: no type, field or
signature moves on the public facade, the contracts.md §0 table is untouched,
and not a byte of what is *sent* differs — only what the packer *counts* before
sending it, and how many requests one retry round is divided into.

- **A retry round is re-packed under the same budget the first round was.**
  `crate::batch::group_by_token_budget`'s only call site was the initial
  grouping in `unit::build_batches`; `pipeline::dispatch` rebuilt the next round
  straight from its `retry_units` vector, so a retry round shipped as **one**
  batch however large — even though every re-dispatched unit now carries an
  ADR-0009 `RetryContext` whose `reason` runs to 512 bytes that the original
  packing never accounted for. Retry units now go through the packer, and a
  round splits into several retry batches when the hints push it past the token
  target. A round that still fits stays one batch, exactly as before.
- **A dispatch round is a pass, not a request.** Both retry budgets stay charged
  per round: the batch-fault admission runs once over the round's union of
  offenders, and the per-batch transient-transport budget spans the whole ladder
  (ti `294dda`). The `1 + max_per_batch_schema_retries + U ×
  max_per_unit_validation_retries` round bound is unchanged.
- **ADR-0009 is untouched.** Re-dispatch is still a *verbatim* resubmission —
  same `source_payload`, same scope, guidance confined to the non-content
  `retry` side channel. Only the request boundaries around those payloads moved;
  a split regroups units, it never rewrites, merges, or re-scopes one.
- **Every retry batch stamps its own id.** R0008-0020 held for the single retry
  batch that used to exist; it now holds for the second and third one of a round
  too — each gets its own `BatchId` from the run's retry allocator and stamps it
  on the units it carries.
- **The language labels are measured, not allowed for.** `FIXED_ENVELOPE_TOKENS`
  (256) stood in for the constant user-message instruction *and* the
  source/target-language fields; ADR-0013 lets a caller pass an opaque label of
  any length, so a fixed allowance for them was unsafe by construction. The
  per-batch envelope reserve now encodes both labels; the constant covers only
  the instruction it was tuned for. Short labels (`en`, `ko`) cost a token
  apiece, so packing shape is effectively unchanged for ordinary runs.
- **Constraint hints are encoded, not approximated.** `estimate_unit` costed the
  constraint vectors with per-entry constants (`* 8`, `* 4`, `* 2`, `* 3 + 4`).
  It now encodes the exact `constraints` + `retry` JSON `llm::prompt` puts on
  the wire, so an added hint is costed the moment it is sent instead of when
  someone remembers to retune a constant. Structure-heavy units (lists, tables,
  fenced code, raw HTML) therefore estimate slightly higher and may pack into
  marginally smaller batches — the documented safe direction (more, smaller
  requests; zero quality loss).

### Fixed — the heading level and the neighbor block kind reach the model (2026-08-06)

Review-0001 finding `R0001-0024` (ticket `53d4956f`). **No API change, no gate
change**: no type, field or signature moves, and the contracts.md §0 facade
table is untouched. **Cache identity moves for every unit that has context** —
see the last bullet.

`BlockContext` carries typed context — `HeadingSnippet.level` and
`NeighborSnippet.kind` — but the wire struct every provider sends flattened it
away, so the public types advertised context the model never received: a
section path under an `h3` was indistinguishable from one under an `h1`, and a
preceding code block from a preceding paragraph.

- **A section-path entry is now `{level, text}`**, not a bare string. Position
  cannot stand in for the level: a path is not always contiguous (an `h1` may
  be followed directly by an `h3`).
- **Each neighbor snippet now carries its kind**, as `context.preceding_kind` /
  `context.following_kind` in the same wire vocabulary as a unit's own
  `block_kind` (`paragraph`, `code-block`, `table`, …), beside the existing
  `preceding_summary` / `following_summary`.
- **The instruction text did not move.** The data-framing sentence still names
  the four source-derived values (invariant 7); the level and the two kind
  labels are the parser's verdict about the document's structure, never the
  document's own words, so they are deliberately not in that enumeration. The
  two user-prompt goldens were regenerated and differ **only** in the `context`
  object.
- **A context-free unit is unchanged.** The hint object still disappears
  entirely when a `BlockContext` is empty — the new fields are `Option`s that
  ride only with the snippet they belong to.
- **Cache consequence.** `pipeline::context_hash` hashes the provider-visible
  hints, so the level and the kind join it: a unit whose prompt now says more
  than it did no longer matches its previously cached translation and is
  retranslated once. This is deliberately the only invalidation lever —
  `VALIDATION_SCHEMA_VERSION` is **not** bumped, because the change cannot
  alter what a valid result looks like and a bump would additionally orphan
  every context-free unit, whose hash is pinned unchanged by test.
- **Batch budgeting follows the wire.** `batch::estimate_unit` encodes the
  neighbor kind labels it now ships; the per-entry heading level stays inside
  the existing framing approximation.

### Fixed — heading context is the parsed heading's text, not a trimmed source line (2026-08-06)

Review-0001 findings `R0001-0022` and `R0001-0023` (ticket `974d199a`). **No API
change, no gate change**: no type, field or signature moves, and the
contracts.md §0 facade table is untouched. **Cache identity moves for documents
with the affected headings** — see the last bullet.

`BlockContext`'s heading snippets (`section_path[].text` and `document_title`)
were produced by dropping a setext underline with a string rule and then running
`trim_start_matches('#').trim()` over the heading's source slice. That is not
what "plain text" means, and the function's own doc comment promised prose:

- **A `#` that is content is no longer deleted.** A setext heading reading
  `#hashtag` over `========` carries no ATX marker at all, so the trim ate a
  content byte and the provider was told the section was called `hashtag`. The
  ATX spelling (`# #hashtag`) was never affected — the mandatory marker space
  stops the trim — which is why the finding's literal repro did not reproduce.
- **The closing ATX marker is gone.** `# Foo #` reached the provider as
  `Foo #`; the trim only ever looked at the front of the line.
- **Inline syntax is flattened instead of shipped raw.** Emphasis, links,
  images, strikethrough, code spans and inline raw-HTML tags all survived the
  trim verbatim, so `# **Bold** [Docs](https://example.com) and \`code\`` was
  sent as its own Markdown source. It is now `Bold Docs and code`. A
  reference-style link in a heading resolves through the document's
  link-reference pool (the same `Document::ref_defs` append `validate::inline`
  uses), so `[the docs][d]` becomes `the docs` rather than leaking brackets.
- **The mechanism is the parser, not a longer string rule.** The heading's
  source slice is reparsed under the canonical GFM options and its inline
  descendants are walked. Every marker an implementation could get wrong — ATX
  opener, ATX closer, setext underline, inline delimiters — is syntax Comrak
  consumes, and everything else is content that survives byte for byte. The
  setext-underline case the string rule already handled is now structural.
- **Snippets are computed once per document.** They live in the per-document
  `ContextIndex` next to the document title, so the added parse happens once per
  heading rather than once per section-path entry per unit.
- **Cache consequence.** `pipeline::context_hash` folds the document title and
  every section-path text into the cache key, so a document containing a heading
  whose plain text changed — a closing ATX marker, any inline syntax, or the
  setext leading-`#` case — no longer matches its previously cached units and
  retranslates them once. Documents whose headings are plain prose keep every
  cache hit. The prompt goldens are unchanged: they pin already-plain context
  values, and no instruction wording moved.

### Removed — the CLI's duplicate default profile (2026-08-06)

Review-0001 finding `R0001-0041` (ticket `c0d6fead`). **No runtime behavior
change, no API change, no gate change**: the deleted file was never read by
anything, so `default_profile()` returns exactly what it returned before, byte
for byte, and no cache entry is invalidated. Anyone unpacking the
`transync-cli` package will find one fewer file in it.

- **`crates/transync-cli/profiles/default.toml` is gone.** The only default
  profile the runtime has ever seen is `crates/transync-core/profiles/default.toml`,
  which `transync-core` pulls in with `include_str!`; the CLI resolves its
  default through `profile::default_profile()` and never opened a file of its
  own. The CLI-labelled copy was referenced by no `include_str!`, no source
  file, no test and no script — and it had already drifted, missing the
  glossary-scope comment block the core copy carries. A contributor editing it
  to change the shipped default would have changed nothing and seen no error.
- **It stays gone, mechanically.** `crates/transync-cli/tests/default_profile_single_source.rs`
  fails if the path reappears, and points the reader at the core copy. Byte-
  welding the two — the `sync_js_drift.rs` treatment — was the alternative; it
  was rejected because nothing needed a second copy in the first place, and the
  test says so, so a future genuine need can swap in the weld.
- Docs corrected in the same commit: `docs/architecture/persistence-and-files.md`
  had the embedded profile sourced from the CLI path and owned by
  `transync-cli`, `docs/architecture/source-of-truth-table.md` said the CLI
  "carries a bundled copy", and `docs/implementation/module-map.md` still drew
  a `profiles/` dir under the CLI crate. DCR-0012, DCR-0014 and DCR-0015 name
  the deleted path in their *Affected Areas* lists and each gained a dated note
  instead of an edit. The frozen historical records (`stub-manifest.md`,
  `skeleton-plan.md`, the superseded `design-baseline.md`) are left alone by
  their own stated policy — their paths are as-of-date by design.

### Fixed — compiling an already-compiled profile is now a no-op (2026-08-06)

Review-0001 finding `R0001-0014` (ticket `574a1947`). **No API change, no gate
change**: no type, field or signature moves, and the contracts.md §0 facade
table is untouched. A single-compile run is byte-identical on the wire, so no
cache entry is invalidated.

One `ProfileMetadata` is both states of a profile — the **template**
`profile::load_profile` returns (`prompt_body` = `[system].prompt` verbatim,
`{{source_language}}` / `{{target_language}}` unsubstituted, no sections
appended) and the **compiled prompt** `profile::render_prompt_body` returns
(variables substituted, then the `[constraints]` policy lines and the glossary
bullets appended). Nothing in the type said which state an instance held, and
`render_prompt_body` appended a fresh copy of both sections to whatever body it
was handed. A caller who compiled a profile — to inspect the prompt, say — and
then put that profile on `TranslateOptions::profile` shipped **two** copies of
both sections in the system prompt of every batch, and hashed that doubled
prompt into the run's cache identity.

- **Compiling is idempotent.** When `prompt_body` already ends with exactly the
  sections that profile compiles, it *is* that profile's compiled prompt and is
  returned untouched — substitution included, so a glossary term containing a
  literal `{{target_language}}` is not rewritten on the second pass.
  `render(render(p)) == render(p)`, byte for byte, and the
  `profile_prompt_hash` half of the cache key is the same whichever state the
  caller passed.
- **Recognition is byte-exact and anchored at the end**, so it can only match
  text a compilation wrote, never text it merely resembles. What it recognizes
  is that profile's *own* compiled output: a profile whose glossary or
  constraints were edited **after** compiling is neither state, and its current
  sections are still appended to whatever the body holds. Compile from the
  profile `load_profile` returned.
- **The idempotence route was chosen over a type split** on purpose: v0.2.0
  froze the curated public surface, and a `ProfileTemplate` / `CompiledProfile`
  pair would widen the contracts.md §0 table.
- **Behavior difference to expect**: only for a caller who was already
  double-compiling. That caller's system prompt loses the duplicated policy and
  glossary sections, which changes its prompt hash and re-fetches its cached
  units once — onto the same identity a single-compile run has always used.

`docs/architecture/contracts.md` §2 and the Developer Guide now name the two
states and the idempotence rule; the `ProfileMetadata` and `render_prompt_body`
doc comments say which boundary hands out which state.

### Fixed — a glossary entry that cannot mean what it says no longer reaches the prompt (2026-08-06)

Review-0001 findings `R0001-0015`, `R0001-0018` and `R0001-0019` (ticket
`5f6664d4`). **No API change, no gate change**: no type, field or signature
moves, and the contracts.md §0 facade table is untouched. This is a
**load-time and render-time behavior change** — profiles that used to load in
silence now load with a warning, and one class of glossary field is rendered
escaped instead of verbatim.

`[[glossary]]` was the last profile section with no content check. The loader
validated slug, version, prompt and the reserved scope, then handed every entry
straight to the renderer that writes `- "source" → "target"` into every batch's
system prompt. Three shapes of entry survived that trip without meaning what
they said: an empty `source` (which reads as "this rule applies to every
term"), an empty `target` ("delete this term"), and a second entry claiming a
term an earlier entry already claimed. A fourth was worse than meaningless —
`escape_for_quoted` escaped only `"` and `\`, so a newline in any field ended
the bullet and started a line of the author's choosing, inside the system
prompt.

- **Control characters are escaped, not passed through.** `\n` / `\r` / `\t`
  render as their short escapes and every other control character (plus
  U+2028 / U+2029) as `\u{XXXX}`, so one entry can only ever produce one
  bullet. This is unconditional and independent of any gate: it holds for a
  `ProfileMetadata` assembled in code as much as for one read from TOML.
- **Empty terms are dropped and named.** Both gates report
  `glossary[i].source`/`glossary[i].target` in the loader's path-qualified
  warning style, and the renderer skips an unusable entry too, so a profile
  that crossed neither gate still cannot emit a bullet meaning "every term".
- **A repeated source term keeps its first answer.** Terms are compared
  trimmed and case-folded — the same identity the auto-glossary merge already
  used for static-vs-extracted conflicts — so `agent` / ` Agent ` / `AGENT` are
  one term. The later entry is dropped with a warning that distinguishes a
  byte-identical repeat from a genuine conflict and names both renderings.
- **Two gates, not one.** `profile::load_profile` covers a profile read from
  TOML; the translate boundary (`translate` / `translate_with_cache`, ahead of
  the auto-glossary preflight so the merge's static-wins rule sees the
  effective entries) covers the `ProfileMetadata` a caller built or mutated
  afterwards. Same two-gate reasoning as the reserved glossary scope
  (`R0001-0006`) and the `[batching]` knobs (`R0001-0016`).
- **Behavior difference to expect**: a profile carrying one of these entries
  now translates without it and says so; because the entry no longer renders,
  the prompt hash changes and its cached units are re-fetched once. A
  well-formed glossary — including the shipped default profile's — is
  untouched, silent, and byte-identical on the wire.

`docs/architecture/contracts.md` §2, the Profile Cookbook and
`docs/Troubleshooting.md` now state the entry rules.

### Fixed — a zero `[batching]` knob is refused out loud instead of loading silently (2026-08-06)

Review-0001 findings `R0001-0016` and `R0001-0017` (ticket `3e972bb2`). **No API
change, no gate change**: no type, field or signature moves, and the contracts.md
§0 facade table is untouched. This is a **load-time behavior change** — profiles
that used to load in silence now load with a warning, and one packing shape
changes.

`[batching]` validated only `output_expansion_factor`. A `target_output_tokens =
0` therefore survived the loader and was forwarded verbatim as the provider's
`max_completion_tokens` / `max_output_tokens`: a request with no room to answer,
discovered as a truncated round-trip instead of as a local, actionable profile
message. Meanwhile `unit::budget::resolve` applied a bare `.max(1)` to
`max_units_per_batch` and `target_input_tokens_per_batch`, so a `0` from either
the profile or the caller ran the whole document one unit (or one token) per
batch without a word — a shape nobody asked for, and one the module's own test
recorded as intended.

- **`0` now resolves as "unset", with the key named.** Each of the three sizing
  knobs accepts `>= 1`; a zero is normalized to `None` and reported in a
  path-qualified warning, the same normalize-and-warn treatment
  `[render].target_direction` and `[constraints].default_table_strategy` already
  get. The profile stays loadable — the value simply resolves as if the key had
  been omitted (caller value, else the built-in default).
- **Two gates, not one.** `profile::load_profile` covers a profile read from
  TOML; `unit::build_batches` covers the `ProfileMetadata` a caller built or
  mutated afterwards, since that struct is `Deserialize` with public fields and
  is the one cloned onto every `TranslationBatch`. The second gate is what makes
  "no batch carries a zero ceiling" the same statement as "no request carries
  one" (same two-gate reasoning as the reserved glossary scope, R0001-0006).
- **The caller side is reported too.** `TranslateOptions` has no `Option` for its
  two knobs, so a zero there warns on `tracing` and then resolves as unset.
  Warnings from the loader also reach the CLI's stderr, as before.
- **Behavior difference to expect**: a run that previously packed one unit per
  batch off a `max_units_per_batch = 0` now packs at the resolved default, and a
  run that previously sent a zero ceiling now sends none. The CLI's
  `--target-output-tokens 0` "disable the ceiling" sentinel is unchanged, as are
  the `n >= 1` clap ranges on `--max-units-per-batch` /
  `--target-input-tokens-per-batch`.

`docs/architecture/contracts.md` §2 and the Profile Cookbook now state the
supported range for each knob.

### Fixed — a rejected provider round no longer decides the reported source language (2026-08-06)

Review-0001 finding `R0001-0007` (ticket `badc976c`). **No API change, no gate
change**: `TranslationOutput.detected_source_language` and the alignment map's
field of the same name keep their exact v0.2.0 shape, and the contracts.md §0
facade table is untouched. What changed is which dispatch round is allowed to
fill them in.

`process_one_batch` latched `detected_source_language` from the provider's
envelope *before* `validate_batch` ran, under an `is_none()` guard that nothing
revisited. A first round that was thrown away — rows dropped, duplicated, or
invented — therefore still decided the language the run published, while the
accepted retry's answer was discarded. The published metadata could name a
language produced by an attempt whose output never reached the document.

- **The latch moved after `validate_batch` and is gated on the envelope.** Only
  a round with no `BatchFault` may commit the language; the `is_none()` guard
  still keeps the earliest such round, which is also the best-sourced one
  (retry rounds only ever shrink the dispatched population).
- **The rule's other half is now stated rather than incidental.** A *per-unit*
  rejection does not disqualify a round. The per-kind, fragment-reparse and
  inline layers judge one block's content shape and say nothing about which
  language the provider read, so one stubborn table no longer gets to veto a
  sound observation — and a batch ending with a unit in fallback still reports
  its language.
- **Operator-visible consequence**: when *no* round's envelope survives, the
  run now reports no detected language instead of one lifted from a discarded
  envelope. `detected_source_language` is `Option` by construction, so
  "unknown" is an honest answer where a wrong language was not.

The cross-batch tie-break (lowest `input_index` wins, `pipeline/report.rs`) is
unchanged — this fix decides only which round within a batch may speak.

### Docs — the output ceiling's absence from cache identity becomes a decision (2026-08-06)

Review-0001 finding `R0001-0004` (ticket `47d183`). **No behavior change, no
API change, no gate change**: `CacheKey` keeps its exact v0.2.0 field list and
the contracts.md §0 facade table is untouched. What changed is that the absence
is now argued, tested, and operator-visible instead of merely true.

`[batching].target_output_tokens` is the one `ProfileBatching` field that
leaves the engine as a real request parameter (`max_completion_tokens` /
`max_output_tokens`), so contracts.md's blanket "batching shape never
participates in content identity" — justified by budgets being soft estimate
caps — did not actually cover it.

#### Docs

- **contracts.md §1 gains *Output ceiling and cache identity***, recording the
  decision that the ceiling stays out of `CacheKey` and the narrower argument
  that carries it: the ceiling is a *stop*, not a content parameter, and a
  stopped response never reaches the cache because only per-unit-validated
  output is ever written (§5a) — on Responses truncation is the explicit
  `incomplete (reason: max_output_tokens)` error, on Chat Completions the
  partial body fails `parse_batch_output` as `MalformedResponse`. The paragraph
  also states the provider obligation the argument rests on: an implementor
  whose provider *shapes* generation to fit the budget instead of being cut off
  by it MUST surface that as a `TranslatorError`, because no key axis — and not
  `fingerprint()` either, the ceiling being a per-run profile value — separates
  it.
- **The `TokenizerHint` paragraph stops over-claiming.** Its "never
  participates" sentence is scoped to the estimation-only fields and points at
  the new paragraph for the one field it does not cover. The same re-scoping is
  applied to the two other places that carried the blanket sentence: the live
  public rustdoc on `llm::TokenizerHint`, and DCR-0015 — which originated it —
  through a dated note. The hint's own exclusion is unchanged and still correct;
  only the breadth of the reason is.
- **contracts.md §7 gains the operator-facing form** — changing
  `target_output_tokens` does not invalidate a warm cache in either direction,
  and why (a tighter ceiling yields *fewer* entries, never worse ones) — plus
  the separate-cache-per-ceiling caveat for a budget-shaping provider. The same
  section now also names how a Chat-surface overshoot presents, which was
  documented for the Responses surface only.
- **`CacheKey`'s own doc comment names the absent axis**, so the next reader of
  the struct finds the decision where the field list would otherwise just look
  incomplete.

#### Tests

- **`output_ceiling_does_not_change_cache_identity`** (transync-core,
  `pipeline::run_level_tests`) pins the decision end-to-end: a warm run at
  `target_output_tokens = 8000`, then the same source against the same cache at
  `60` and at unset, each dispatching zero batches, with a
  different-target-language run as the negative control. The decision is
  invisible in `CacheKey`'s field list, so this is what makes adding the axis
  later a red test rather than a silent reversal.

### Docs — the living records stop calling six closed tickets open (2026-08-06)

Backlog item `stale-ticket-references-in-living-docs`. **No code, no behavior,
no gate change.** The 2026-08-05 ticket-resolution wave (`242f03a`..`eab1111`)
touched only the documents its own tickets named, so the cross-cutting project
records still described `0fba5082`, `69b9b3db`, `294ddabd`, `2a276cb4`,
`92ac61b9`, and `0c6a5c8b` as open work.

#### Docs

- **`status.md`'s Action line states current truth** — all six tickets resolved
  2026-08-05 — and stops enumerating open work: TicGit (`ti list`) is the
  ticket backlog and `docs/backlog.md` is the index of open items, so the line
  cannot go stale again the next time a ticket is filed or closed.
- **The dated narrative stays as history and gains resolution notes.** Both
  wave bullets in `status.md`, the OI-0008 cluster's "Related" line in
  `open-issues.md`, and four `phase-state.yaml` notes keep their as-of-landing
  sentences and carry a bracketed dated note naming the resolving commit —
  `7572c45`, `1436304`, `cc9fb8f`, `223535b`, `eab1111`. The sixth ticket,
  `0fba5082` (`242f03a`), is named on the Action line only: no wave narrative
  ever mentioned it.
- **DCR-0019's two remaining present-tense claims are annotated** — the "a
  fourth mapping survives" sentence in Part D and the `69b9b3db` follow-up
  bullet, the one ticket of the six whose fix commit was code-only — plus its
  affected-areas ledger row and a third superseded statement folded into the
  existing `294ddabd` note (the transient predicate is no longer "unpinned
  end-to-end"). ADR-0003, ADR-0009, ADR-0019, and DCR-0020 were already
  annotated by the wave and are unchanged; the specs and plans under
  `docs/superpowers/` stay verbatim, per those records' own
  "deliberately not rewritten" rule.

### Docs — the release ritual has a written checklist (2026-08-06)

Backlog item `release-checklist-document`, closing the process gap DCR-0015
left open. **No code, no behavior, no gate change** — every gate named below
already existed and already ran; what was missing was one place that says
which ones, in what order, and what to record.

#### Docs

- **New `docs/project/release-checklist.md`**, a living document under
  `BL-2026-07-B`: preflight (fix the commit, pick the version under
  contracts.md §0/§1, audit CHANGELOG completeness against
  `git log v<prev>..HEAD`, check the schema and identity set), the standing
  gates (`scripts/smoke.sh` incl. the rustdoc gate and the wasm size budget,
  the Playwright suite, fmt/clippy plus a check that the pre-commit hook is
  actually live, the `resp-translator` path-dependency check against the
  tree it compiles against), the DCR-0015 live-endpoint gate
  with its trigger test and CHANGELOG record line, the CHANGELOG promotion,
  the single-line workspace version bump, the record updates, and the
  annotated tag. Its preamble states its authority position: procedure, below
  every level of the baseline's hierarchy, fixed rather than followed when it
  disagrees with contracts.md or a gate's own ADR/DCR.
- **The reference-link stanza is a numbered step**, because v0.2.0 missed it:
  the release shipped with `[Unreleased]` still comparing `v0.1.0...HEAD` and
  `[0.2.0]` undefined, so the new version heading rendered as literal text
  until a follow-up commit fixed it. The invariant it now asserts — one link
  definition per version heading and vice versa — is the check that would
  have caught it.
- The rule stopped living in two informal homes: the Developer Guide's
  smoke-script section and `status.md`'s next actions now point at the
  checklist for the sequence and keep only what they own (the scripts, and
  the current action). DCR-0015's Migration/Follow-up gained a dated note
  recording the gap closed, and `docs/index.md` links the new document.

### Tests — the facade's `lib.rs` is welded into the public-surface gate (2026-08-06)

Backlog item `public-surface-widening-gate`. **No API change: not one export,
`DOCUMENTED` entry, or contracts.md §0 row moved.** What changed is that the
one drift direction OI-0027 left open is now machine-checked.

#### Tests

- **A `pub use` added to `crates/transync/src/lib.rs` alone used to widen the
  real surface silently.** The gate welded three artifacts — contracts.md §0,
  `DOCUMENTED`, and the `first_class` compile assertion — but nothing read the
  facade itself, so the widening direction rested on a manual pre-release
  read-through. `public_surface.rs::lib_rs_exports_nothing_the_documented_list_omits`
  scrapes `lib.rs`, resolves each re-export to the name it publishes (the `as`
  alias, else the last path segment), and asserts every published name carries
  a §0 row. Feature-gated exports are the one allowance, enumerated in a
  `FEATURE_GATED` constant (today `transync::test_stub`, which §0 already
  excluded in prose) and asserted not to overlap the table.
- **The scrape refuses to fail open.** Any top-level `pub` form it cannot read
  — `pub fn`, `pub struct`, an inline `pub mod NAME { … }` — panics with the
  offending line rather than being skipped, and the reverse inclusion is
  asserted too (every §0 root row must be a name the scrape found), so a scrape
  that stopped seeing anything cannot pass vacuously. Verified by four throwaway
  probes against a temporarily-widened `lib.rs`: an undocumented `pub use`, a
  `pub fn`, a `pub mod NAME;`, and a scrape pointed at the wrong file — each
  failed the expected assertion, none committed.
- Dependency-free by design: the OI-0027 spec had already rejected a
  `cargo-public-api` snapshot gate as install-dependent (the "silently skips"
  failure mode) and left the tool as a manual ritual step, never a test — so no
  new tool or toolchain enters here. Surface reaching consumers *without being
  named in `lib.rs`* — an engine type exposed through an exported signature —
  stays rustdoc-JSON territory and is stated as out of scope in §0.
- Suite: **408 passed / 0 failed / 3 ignored** at `--test-threads=4`.

#### Docs

- `contracts.md` §0's "what this does not cover" paragraph now describes the
  fourth check instead of the manual read-through ritual. DCR-0018 gained a
  dated addendum, and OI-0027's resolution record notes the direction closed.

### Tests — ordered-list `start` offset, pinned in both panes (2026-08-05)

Backlog item `ordered-list-start-offset`. **No behavior change: the fix already
shipped.** `<ol start="N">` has been emitted since the P2-9 wave (see 0.2.0
below, "Structural ownership completed"), and the renderer now reads the
ordinal straight off the paired Comrak `List` node — the DCR-0007 bullet that
called it a known limitation and proposed carrying `start` in the IR is
original-2026-07-10 text that its own later updates supersede. `pub struct
Block` did not need the field then and does not now.

#### Tests

- **The target pane had no pin.** Every start-offset assertion ran on the
  source pane, yet the target pane parses the *regenerated* Markdown, so the
  ordinal only reaches it if the accepted payload keeps its marker.
  `render::list_grouping_tests::ordered_start_reaches_the_target_pane_too`
  drives the real `regen` → `render_target` path with translated payloads and
  asserts `<ol start="3">` — and no bare `<ol>` — in **both** panes.
- Verified alongside it, no code needed: `regen` splices list-item payloads
  verbatim (no list reserialization step) and `validate::per_kind::check_list`
  rejects a renumbering provider, so translated and fallback output both keep
  the source marker; the vendored DOMPurify (3.2.6) keeps `start` under its
  default config, confirmed by sanitizing `<ol start="3">` in headless Chromium
  against `web/vendor/purify.min.js`; and the CLI `--html-out` bundle embeds
  `annotated_source_html` verbatim, so it inherits the attribute.
- Suite: **407 passed / 0 failed / 3 ignored** at `--test-threads=4`; browser
  suite 13/13.

#### Docs

- **`contracts.md` §4/§4a stopped contradicting the renderer.** Both said the
  shared `<ul>`/`<ol>` group carries *no attributes* / is *unattributed* — text
  the `<ol start="N">` fix had falsified and never corrected. They now say the
  group carries no **sync** attributes, and name `start` as the one
  presentational attribute it may carry. No rendered output changed; the
  contract text caught up to it.

### Engine internals — alignment-map assembly joins the report phase (2026-08-05)

Backlog item `alignment-map-assembly-lift`, closing the open seam DCR-0019 left
in its *Migration / Follow-up*. Nothing here is observable: no type, field,
signature, wire value, or curated facade row changes, and `pipeline` is
`pub(crate)`.

#### Internal

- **`run_pipeline` no longer assembles the alignment map inline.** The status
  projection, the completeness `debug_assert`, the `build_alignment_map` call
  and the `retried_units` patch move verbatim into
  `pipeline::report::assemble_alignment_map`, so the reporting phase owns both
  of its products and the orchestrator calls it the way it already calls
  `build_validation_report`. The extraction states one ordering fact the inline
  code left implicit: the report handed in must be the composed one, because
  `retried_units` is derived from its per-unit attempt log.
- Verified by the workspace suite plus a byte-diff of the CLI's full output
  tree (`out.md`, alignment map, `--html-out` bundle) regenerated by the
  pre-change and post-change binaries over all fourteen checked-in fixtures:
  **141 files, zero diff**.

#### Tests

- **`ValidationSummary.retried_units` is pinned at a non-zero value.** The
  field had no value assertion anywhere, so a derivation replaced by a
  hardcoded zero would have passed every test; SCN-07 — the one scenario that
  actually retries — now pins it at 2 of its 3 units.
- Suite: **406 passed / 0 failed / 3 ignored** at `--test-threads=4`.

### Engine internals — DCR-0019's five deferred minors (2026-08-05)

Backlog item `dcr-0019-minor-cleanups`. Nothing here is observable: no type,
field, signature, wire value, log-message shape, or curated facade row changes,
and every module involved is `pub(crate)`, module-private, or inside
`transync-syntax` (whose direct consumers accept the weaker stability promise).

#### Internal

- **`byte_range_for` clamps once instead of three times.** Two of its clamps
  were provable no-ops: `pos_to_byte` already clamps to the same line's content
  end, and a content end never exceeds the source length. The surviving clamp is
  the load-bearing one (the same one OI-0034 currently relies on).
- **`LineOffsets` no longer keeps a parallel per-line vector of content ends.**
  It borrows the source it describes and derives each line's content end from
  the next line start, subtracting the terminator's length — two bytes exactly
  for `\r\n`, the same rule `LineOffsets::new` already applies, so the lone-CR
  boundary (OI-0033) is still stated once. `source_len` folds into the borrow.
  Proven byte-identical by a differential probe over every block's id, kind,
  byte range, source hash, and sliced text: 13 synthetic documents (LF / CRLF /
  lone-CR / mixed / NUL / leading and missing terminators / empty) plus all 14
  checked-in fixtures in three line-ending variants each — 360 rows, zero diff.
- **The per-batch retry policy asserts its round protocol on both sides.**
  `unit_disposition` already debug-asserted that `record_dispatch` had run;
  `admit_batch_fault_round`'s once-per-round obligation — which is how the
  previous round's routing is cleared — was enforced by nothing. Both are now
  debug-asserted against the round ordinal, and the check was verified
  discriminating by making the admission conditional and watching the suite go
  red at it.
- **The schema-fault warn log reads its ceiling through the policy** that owns
  the budget (`max_batch_fault_rounds()`) instead of pairing the policy's
  counter with the orchestrator's own copy of `TranslateOptions`.
- **`transync-openai`'s endpoint paths and default base URL moved into
  `client/endpoint.rs`**, next to the `/v1` de-duplication rule that exists for
  them; `client.rs` keeps a `pub(crate)` re-export, so `lib.rs` is unchanged.
- Suite: **406 passed / 0 failed / 3 ignored** at `--test-threads=4` (one new
  unit test, pinning the four table edges the derived cap now has to get right
  on its own), `scripts/smoke.sh` green including the rustdoc gate and the
  wasm size budget.

### Profiles — the reserved glossary scope is rejected on every entry path (2026-08-05)

Backlog item `profile-section-scope-programmatic-bypass`, closing review finding
R0001-0006 (High). Behavior change confined to programmatic profiles that were
already in the misleading condition; no type, field, wire value, or serialized
key moves.

#### Fixed

- **A programmatically-built profile carrying `GlossaryScope::ConditionalOnSection`
  now fails the run instead of silently applying the entry globally.**
  `profile::load_profile` has rejected the reserved `"section"` scope since
  ADR-0014's amendment, but the loader was the *only* gate — and
  `ProfileMetadata` derives `Deserialize` with public fields. A profile built in
  code (`serde_json::from_str` over a JSON config, or `default_profile()`
  followed by `glossary.push(...)`) never crossed the loader, reached
  `format_glossary`, and had its section-scoped term rendered into every batch's
  system prompt — a term meant for one section steering the whole document,
  which is exactly what the loader gate exists to prevent. The rejection now
  lives in one helper called from two places: the loader as before, and the
  translate boundary (`translate` / `translate_with_cache`) on
  `TranslateOptions::profile`, before the parse and before any provider call —
  the auto-glossary preflight included.
  - **Observable:** only for callers already in that state. A run that used to
    succeed while quietly widening a section-scoped term now returns
    `TransyncError::Profile(ProfileError::Unsupported(..))` (stable code
    `profile_failed`) with the loader's own message, which names the supported
    `scope = "global"`. Profiles loaded from TOML are unaffected — they could
    never reach the pipeline with this scope. Profiles with a global scope, or
    with no glossary, are unaffected. The CLI is unaffected: it builds its
    profile through `load_profile`.
  - `contracts.md` §2 now names both gates, and **ADR-0014** carries a dated
    note recording that its *Implementation* section described the loader
    rather than the invariant.

### Scripts — a workdir is deletable by containment or ownership, never by name (2026-08-05)

Backlog item `smoke-script-rm-rf-basename-allowlist`, closing review finding
R0001-0002 (High). Developer tooling only: no library, CLI, or browser code is
touched, and no shipped artifact changes.

#### Fixed

- **The `rm -rf` guard in `scripts/smoke.sh`, `scripts/smoke-live.sh`,
  `scripts/test-browser.sh`, and `scripts/build-wasm.sh` no longer accepts a
  path because of its *name*.** The old rule wiped any path whose basename
  began with `transync-`, anywhere on disk — so
  `TRANSYNC_SMOKE_WORKDIR=/Users/x/transync-notes` deleted that directory. The
  rule is now containment or ownership: the path must resolve — symlinks on
  its parent included — to a strict descendant of an approved temp root
  (`/tmp`, `/private/tmp`, `/var/folders`, `/Volumes/Temp/claude`, `$TMPDIR`),
  or not exist yet, or carry the `.transync-workdir` marker file these scripts
  stamp immediately after `mkdir`. The `..` rejection (R0008-0008) is unchanged
  and still runs first, and a temp root itself is still never a valid target.
  - **Observable:** an override pointing outside every temp root now needs one
    bootstrap run — the first run creates the directory and marks it, later
    runs recognise the marker. An override aimed at an existing *unmarked*
    directory is refused, with a message naming all three acceptable forms.
    Defaults and any override under a temp root behave exactly as before.
  - The rule now has a single home, `scripts/lib/workdir-guard.sh`
    (`transync_guard_workdir` / `transync_mark_workdir`), sourced by all four
    scripts. It used to be four copies — the shape that let this hatch
    outlive the sibling fix (R0001-0001) which hardened the same guard's
    root-equality handling.

### Retry policy — the provider-retry budget is per batch (2026-08-05)

Ticket `294dda`, filed by the OI-0008 wave that surfaced the mismatch and left
it verbatim (its behavior-change budget was already spent). Behavior change,
library-internal: no type, field, or serialized key moves.

#### Fixed

- **`TranslateOptions::max_per_batch_provider_retries` is now charged to the
  whole input batch**, as its name and rustdoc always said. It used to be
  charged per *dispatch round*: `process_one_batch` calls the transport-retry
  loop once per round, and the round boundary reset the counter — so a batch
  going through the full retry ladder could spend up to
  `rounds × max_per_batch_provider_retries` transient retries while the knob
  promised `max_per_batch_provider_retries`. The policy's counter now carries
  across rounds, and the exponential backoff ordinal with it.
  - **Observable:** a run against a provider that is *both* flaky and
    structurally sloppy (transient `Network` / `RateLimited` failures spread
    over rounds that also need validation or schema retries) now aborts sooner
    — the transient error that the second round used to absorb on a refunded
    budget is terminal (ADR-0017: a spent transport budget aborts the run, it
    is never a fallback path). Runs whose transient failures all fall inside
    one dispatch round are unaffected, as are runs with no transient failure —
    which is every run in the test corpus and both smoke paths.
  - **Remedy if a run now aborts:** raise
    `max_per_batch_provider_retries` (default `1`), which is what the knob
    means for the first time.
  - `ValidationReport.provider_retries` is unchanged in meaning — the honest
    observed total — and is now bounded by
    `max_per_batch_provider_retries × input batch count`.
  - The transport path gained its first end-to-end tests: two scripted
    translators drive real `TranslatorError::RateLimited` failures across more
    than one dispatch round (`pipeline::dispatch::provider_retry_budget_tests`).
    The policy unit test that pinned the per-round reset pinned the bug; it now
    pins the per-batch budget. Recorded in **ADR-0009** (dated note) with the
    correction noted on **DCR-0019**, which had documented the old reset.

### Track C — in-browser WASM rendering + edit demo (2026-08-05)

Track C proper: the browser integration OI-0028 / DCR-0017 deferred after
delivering only the compile path. Recorded in **ADR-0019** (the decision) and
**DCR-0020** (the implementation); implements the owner-approved spec
`docs/superpowers/specs/2026-08-05-track-c-wasm-render-demo-design.md`. Six
commits, `94d7e39`..`1d04135`.

**Everything here is additive.** The curated facade surface, the alignment
schema (`1.2.0`), `VALIDATION_SCHEMA_VERSION` (`2`), `CacheKey`, both `sync.js`
copies, both shipped shells, and the ADR-0006 six-file `--html-out` bundle
contract are **untouched** — `git diff 94d7e39..HEAD` over that frozen set is
empty. No public API moved and no output byte differs.

#### Added

- **`crates/transync-wasm` — the browser (wasm-bindgen) surface over
  `transync-syntax`**, the workspace's sixth member (`cdylib` + `rlib`,
  `publish = false`). It depends on `transync-syntax` **alone** among workspace
  crates: `transync-core`'s `wasm32` tree reaches `getrandom` through
  tokio/tiktoken and `transync-syntax`'s does not, so the restriction is a link
  requirement as well as a charter rule. All logic lives in a plain-Rust
  `engine` module with zero wasm-bindgen types, so it is fully host-tested;
  `lib` holds three thin `#[wasm_bindgen]` wrappers.
  - **`render_pair(source_md, translated_md, alignment_json)`** — view mode.
    Reproduces the CLI's pane fragments **byte-identically**.
  - **`rebuild(source_md, payloads_json, statuses_json, source_lang,
    target_lang, detected?)`** — edit mode, running the whole local loop
    (regen → alignment map → render). It deliberately never reuses a fetched
    alignment map: the html and skipped render arms slice `target_range`
    **bytes** that an edit has shifted, which would mis-render silently.
  - **`schema_version()`** — the lockstep leg, pinned by a host test to
    `ALIGNMENT_SCHEMA_VERSION` so a built module can never disagree with the
    crate it came from.
  - The boundary is JSON strings in, one JSON string out — no
    `serde-wasm-bindgen`, no new dependency, one `JSON.parse` per call. Errors
    become thrown JS exceptions; unknown block ids are **rejected** rather than
    silently dropped.
- **`scripts/build-wasm.sh`** — builds the module into `web/wasm/` (gitignored)
  with wasm-pack plus an explicitly resolved binaryen `wasm-opt`, and
  **enforces a size budget** (raw ≤ 1,950,000 B, gzip ≤ 810,000 B; measured
  1,739,791 / 718,136). Prerequisites fail **loudly** — never a silent skip.
- **`web/demo-wasm.html` + `web/js/wasm-demo.js`** — a framework-free demo that
  renders both panes locally from `source.md` + `out.md` + `alignment.json` and
  lets you edit a translated block's Markdown and watch the panes re-render,
  re-align, and re-sync. It mounts through the same fail-closed DOMPurify path
  the shipped shells use, and refuses to boot on a missing DOMPurify or on
  either schema mismatch (module-vs-demo, or the fetched map).
- **Four Playwright tests** (`web/tests/wasm.spec.js`), taking the browser
  suite from 8/8 to **12/12** — including a standing **CLI-vs-browser parity
  gate** that compares the wasm-rendered panes against the CLI's own
  `source.html` / `target.html` block for block.

#### Changed

- **`comrak` now builds with `default-features = false`** workspace-wide. Its
  default `cli` and `syntect` features are used nowhere (grep-verified) and
  dropping them shrinks the wasm module. Verified behavior-identical across
  every test binary, target by target. *Consumer note:* a downstream crate that
  relied on transync's dependency to enable those features for it must now
  declare them itself.
- **`wasm-bindgen` is pinned exactly at `=0.2.126`.** wasm-bindgen requires
  crate↔CLI version identity, and this repository ships no committed
  `Cargo.lock` (OI-0020, which **stays open** by owner decision), so an exact
  pin is the only defense against clean-checkout skew.
- **The standing wasm gate now names both crates** —
  `cargo check -p transync-syntax -p transync-wasm --target wasm32-unknown-unknown`
  — in both pre-commit hook copies and `scripts/smoke.sh`. `scripts/smoke.sh`
  additionally runs `scripts/build-wasm.sh`, and its rustdoc gate covers
  `transync-wasm`.

#### Not done, deliberately

- **The CLI `--html-out` bundle carries no wasm.** The module is ~41× the
  entire 42 KB JS payload, and a bundle already ships rendered HTML — exactly
  what the module's view mode reproduces. `HTML_BUNDLE_ENTRIES` and the
  six-file contract are unchanged.
- **Source-pane and structure-changing edits are out of scope.** The demo edits
  per-block payloads of the translated side only, so the block set is
  structurally preserved and ID stability across edits stays a non-goal.
- **html blocks are read-only in the demo** — their payload is an extracted
  text-segment array (ADR-0018), not Markdown, so the demo declines an edit it
  could not round-trip.

#### Building it

The demo module is a build artifact, not a checked-in one. `./scripts/build-wasm.sh`
needs three host prerequisites: the `wasm32-unknown-unknown` rustup target,
`wasm-pack`, and **binaryen ≥ 121** (`brew install binaryen`). wasm-pack's own
cached binaryen is **not** a substitute — it is version 117 and rejects current
rustc output.

### Internal-quality wave — OI-0008 + OI-0033 (2026-08-05)

The first wave after v0.2.0 closed the breaking window, and it respects it:
the curated facade surface is **byte-identical to the tag**
(`crates/transync/tests/public_surface.rs` is unmodified and green), the
alignment schema stays `1.2.0`, `VALIDATION_SCHEMA_VERSION` stays `2`, and
`CacheKey` and both `sync.js` copies are untouched. Recorded in **DCR-0019**;
**resolves OI-0008** (the 2026-05-04 refactor/perf debt cluster, in full) and
**OI-0033** (lone-CR line-table desync), and **files OI-0034**. Implements the
owner-approved spec
`docs/superpowers/specs/2026-08-05-oi0008-0033-internal-quality-design.md`.
Eleven commits, `b1c3c7f`..`73fd331`.

**The behavior-change budget was exactly two items** — the `Fixed` and
`Changed` entries below. Everything under `Internal` is behavior-preserving
motion, each piece carrying its own proof.

#### Fixed

- **Lone-CR sources now slice correctly.** `LineOffsets` built its line table
  by scanning for `\n` only, so a document using a lone `\r` as its line
  terminator (classic Mac endings, or any mixed-ending file) produced a table
  with fewer entries than the document had lines — while comrak's `Sourcepos`
  counted those breaks. Every lookup past the first lone `\r` resolved against
  the wrong line, and the resulting block payloads were **silently** empty or
  swallowed the following block. `LineOffsets::new` now counts a `\r` whose
  next byte is not `\n` as a boundary, exactly as comrak's own
  `strings::is_line_end_char` does.
  - **`\r\n` and `\n` documents are entirely unaffected** — the new arm fires
    only where a lone `\r` exists, so the offsets vector is byte-identical for
    every LF and CRLF document. Nothing about today's inputs changes.
  - The two duplicated line-end-cap derivations collapsed into one
    terminator-aware helper (a CRLF line's cap now excludes its `\r` too), and
    a translatable **list item** with an empty computed range at a
    non-degenerate source position now raises a parse warning instead of
    silently disarming list-structure validation.

#### Changed

- **`ValidationReport.total_retries` counts content retries only.** It was
  computed as `attempts.len() - 1`, which also counted batch-schema-fault
  rounds and rejected cache-hit re-validations — contradicting the documented
  claim that `batch_schema_faults` is counted apart from `total_retries`. A
  `validation-report.json` from a run that took a batch-schema fault or a
  rejected cache hit will now report a **lower** `total_retries` than it did
  at v0.2.0. `batch_schema_faults` and `provider_retries` are unchanged, and
  the three counters now sum without double-counting. A run with neither
  reports the same number as before.
- **`transync_openai::pagination` is removed** (module and `split_oversize`).
  It was an explicit DEFERRED no-op with zero callers in this workspace and
  zero in the `resp-translator` sibling; its consumer-side counterpart was
  already deleted in v0.2.0. Taken deliberately outside the breaking window:
  the provider contract (`contracts.md` §7) never covered it, the workspace
  publishes to no registry, and a dead public stub kept alive for semver
  optics is the opposite of this wave's purpose. **The deferral itself
  stands** — the oversize row-window split remains tracked in
  `stub-manifest.md` and `contracts.md` §5.

#### Internal

Nothing below changes observable behavior; the engine modules involved are
`pub(crate)` or module-private since v0.2.0, so no name a curated consumer can
reach moved.

- **Retry/fallback policy is a pure decision core.** The budget state and
  every decision that used to be interleaved with provider calls, cache
  writes, sleeps, and log lines now live in `pipeline/policy.rs`, whose
  complete dependency set is `TranslateOptions`, `BlockId`, `AttemptOutcome`,
  `HashMap`, and `Duration` — no async, no I/O types. Twelve decision tests
  now run without a mock translator, and the backoff schedule plus the
  provider-retry budget got their first coverage at all.
- **Six concern splits**, each verified as motion rather than design:
  `run_pipeline` → `pipeline/{dispatch,finalize,report}` (pure-motion proof:
  a normalized line-multiset diff leaves zero production lines on either
  side); `unit.rs` → `structure` + `structure/labels` (which removes the
  inversion where the validator imported its structural oracle from the unit
  builder) and `unit/{payload,budget}`; `transync-openai`'s client →
  `client/{classify,transport,endpoint,dispatch,chat,responses}` plus
  `tokenizer`, behind one surface abstraction shared by translation and
  glossary extraction; `transync-cli`'s translate command →
  `translate_cmd/{args,input,provider,publish,report}` behind an `execute`
  seam that collapses fifteen scattered exit-code sites into one table
  (byte-identical output trees, stderr, and exit codes across seven
  invocations of the old and new binaries); and the parser walker onto a
  `WalkState`, across `parser/{options,classify,emit,sections}`.
- **The HTTP-status → provider-error classification table is pinned.** It is
  the input the pipeline's retry policy branches on and it had **no** test,
  because it was reachable only over a socket. Ten new tests were written
  against a module containing nothing but tests (29 unresolved-name compile
  errors as the RED step) before the table was moved character-for-character.
- **Dead IR deleted.** `Document::hierarchy` (a full extra pass with
  allocations, computed on every parse, read by nothing) and
  `Block::parent_id` (provably always `None`) are gone, along with six
  filter sites they made tautological, three
  `#[allow(clippy::too_many_arguments)]` attributes, and an `emit_here!`
  macro that existed only to hide nine positional arguments. **The wire is
  unchanged:** `AlignmentBlock.parent_id` remains in schema `1.2.0` as an
  always-null reserved field, proven by regenerating 112 CLI output files
  from the pre-change tree and diffing byte-for-byte.
- Suite: **385 passed / 0 failed / 3 ignored** at `--test-threads=4`, up from
  340. Zero test removals, with one sanctioned 1-for-1 swap whose replacement
  is stronger on the surviving contract.

## [0.2.0] - 2026-08-05

> **Note added 2026-08-07 — which review round this section cites.** Every
> `R0001-NNNN` id below is from the **2026-05-02** Review 0001, whose file
> was removed in `bb93b68` (`reviews/reviewed/0001.md`). It is *not* the
> `reviews/0001.md` on disk today, which numbers a different 52 findings over
> the same `0001`–`0052` range — `R0001-0025` here is
> "`ValidationSummary.retried_units` is never populated", not the live round's
> list-depth overflow. `reviews/README.md` carries the round registry and the
> retired round's finding index.
>
> **Corrected 2026-09-03 (ti `bdf8d981`).** This note used to end "the
> `R0002`–`R0008` ids below do not collide with anything." That was `reviews/`
> rule 4's claim and it was false: three gated rounds on 2026-08-07/07/11
> reused `0002`, `0003` and `0004`. Every `R0002`/`R0003`/`R0004` id in **this**
> section is from the **2026-05** round — the section predates the gated ones —
> while a bare id written after that date means the 2026-08 round. Most ids
> here need no marker anyway, because their numbers decide them
> (`reviews/README.md` rule 4's range table); this note covers the seven in the
> genuinely overlapping `R0002`/`R0003` `0001`–`0086` band. Nothing below is
> rewritten: rule 5 keeps a released section's words and appends a note like
> this one instead.

Closes the sanctioned breaking window opened after v0.1.0; every entry
below rode it. Release gate (OI-0030): `scripts/smoke-live-gate.sh`
**PASS** on 2026-08-05 — 2/2 machine-asserted live round-trips against
`gpt-4o-mini` (Chat Completions) and `gpt-5-mini` (Responses).

### Release prep (2026-08-04..05)

- **`#[non_exhaustive]` extended to the six read-only output types** —
  `TranslationOutput`, `ValidationReport`, `AlignmentMap`, `AlignmentBlock`,
  `ValidationSummary`, `GeneratorMeta` (owner decision at release prep,
  adopting the final review's observation; workspace attribute count
  11 → 17). Output-side field additions are now non-breaking too.
  `ByteRange` stays exhaustive by policy — a byte range is complete by
  definition. A construction census found zero cross-crate literals for
  all six, so no migration accompanied the attributes.
- Standing-gate polish: `scripts/smoke.sh`'s CLI test line gained the
  `--test-threads=4` cap, and `phase-state.yaml`'s drift-test part count
  and gate tally caught up with the final fix wave.
- **The reserved-glossary-scope rejection now names a value that parses.**
  Loading a profile with `GlossaryScope::ConditionalOnSection` told the
  operator to use `"global-across-document"` — neither the serde name nor
  an alias, so following the hint produced a second, different error. The
  message now names the reserved scope by its real wire values (`"section"`,
  alias `"conditional_on_section"`) and points at `scope = "global"`, or at
  deleting the key since `"global"` is the default. A regression assert pins
  that the hint never again names a value serde rejects. **ADR-0014** amended
  to match (the quoted message, the false wire-value list, and the
  consequences line that called the non-value fix obvious).

### Changed

- `transync-openai` now supports an optional typed reasoning effort across GPT-5 generations, emits the endpoint-specific Chat Completions or Responses request shape, keeps the field absent for existing callers, includes it in the provider fingerprint, and regression-tests GPT-5.0 through GPT-5.6+ model dispatch.

### Surface wave — public-surface curation, the 0.2.0 window-closing ritual (2026-08-04)

`crates/transync` stopped re-exporting the engine crates wholesale and became
an explicitly curated list, and the tier boundary became a compiler fact.
Recorded in **DCR-0018**; **resolves OI-0027**, the last item on the road to
0.2.0. **DCR-0005** gained a dated note (the firewall it created is now
curated) and **ADR-0018** a second dated path note. Implements the
owner-approved spec
`docs/superpowers/specs/2026-08-04-oi0027-public-surface-hardening-design.md`.
All changes ride the still-open 0.2.0 breaking window.

**Added:**

- **`contracts.md` §0 "Public Rust surface"** — a 78-row table that *is* the
  supported Rust surface, with the tier (a) first-class / tier (b)
  provider-SDK split and the deliberate absence of tier (c) engine internals.
- **`crates/transync/tests/public_surface.rs`** — a five-part standing gate:
  a compile-assertion module that re-exports every §0 row, a `DOCUMENTED`
  mirror, a **bidirectional** scrape-and-diff against §0, a segment-based
  negative test keeping engine module names out of §0, and a DCR-0017 **M6**
  guard pinning `AlignmentMap`'s `generator.version` to the facade crate's.
- **`TranslationUnit::new(unit_id, block_kind, input_mode, source_payload,
  source_hash)`** plus `with_context` / `with_constraints` / `with_batch_id` /
  `with_retry`. `Default` is deliberately not implemented — a zero
  `source_hash` would silently alias cache entries.
- **Root aliases:** `ParseError`, `GlossaryScope`, `ALIGNMENT_SCHEMA_VERSION`,
  `OutputBudgetWarning`.
- **Standing rustdoc gate** in `scripts/smoke.sh`:
  `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` for the three library
  crates. `cargo doc` is warning-free for all of them.

**Changed (breaking, inside the 0.2.0 window):**

- **The facade is an explicit re-export list.** `pub use transync_core::*;`
  and `pub use transync_syntax;` are **gone**, along with the facade's
  `transync-syntax` dependency. `transync::{parser, id, unit, batch, validate,
  regen, render, align, pipeline}` and `transync::transync_syntax` are no
  longer reachable. A consumer needing an engine internal depends on
  `transync-core` / `transync-syntax` **directly**.
- **`transync::pipeline::run_pipeline` → `transync::translate_with_cache`.**
  Same arguments and return type, plus a fail-fast preflight: an empty or
  whitespace-only `target_language` now returns
  `TransyncError::Internal` (`stable_code` `"internal"`) instead of
  proceeding.
- **`#[non_exhaustive]`** on `TranslatorError`, `TransyncError`, `ParseError`,
  `TranslateOptions`, `ProfileMetadata`, `ProfileConstraints`,
  `ProfileBatching`, `ProfileRender`, and `TranslationUnit`. Matches need a
  wildcard arm, destructures need a trailing `..`, and construction is
  default-then-assign — `..Default::default()` is illegal cross-crate on
  these types.
- **`OutputBudgetWarning` moved from `batch` to `validate`**, where
  `ValidationReport` lives. Type identity and serde shape unchanged; name it
  at the crate root.
- **`transync-core` engine modules are `pub(crate)`**: `batch`, `error`,
  `pipeline`, `validate`, and the `align`/`id`/`parser`/`regen`/`render`
  aliases. `unit` is `#[doc(hidden)] pub` solely so `transync-openai`'s
  offline live-smoke fixture test can reach it; it is not curated API.
- **Narrowed to `pub(crate)`** in `transync-syntax`:
  `htmlseg::balance_fragment`, `outcome::is_translatable`, `walk::label_for`.

**Removed:**

- `pipeline::retry::{retry_validation, oversize_split, fallback_to_source}` —
  three no-op hooks the module closure exposed as dead. The retry/fallback
  policy debt itself remains open under **OI-0008**.
- `batch::group_by_unit_count` (with its pre-existing `#[allow(dead_code)]`)
  and `ValidatedBatch::batch_id_str` — no callers anywhere.

**Unchanged:** `out.md`, the alignment map and its `1.2.0` schema,
`VALIDATION_SCHEMA_VERSION` (`2`), `CacheKey`, and both `sync.js` copies. No
test was removed anywhere in this wave.

### Structural wave — `transync-syntax` crate split + AST-direct renderer (2026-08-04)

The syntax layer became its own workspace member so it can compile for the
browser, and the renderer was rewritten to stop paying per-block parse costs
that a WASM build would have exported to the browser. Recorded in
**DCR-0017**; **resolves OI-0028** and **narrows OI-0008** (both renderer
items cleared). **ADR-0003** gained a dated amendment (five members;
`transync-core → transync-syntax`), **ADR-0006** and **ADR-0007** dated
mechanism amendments, **DCR-0005** and **DCR-0007** dated notes. Implements
the owner-approved spec
`docs/superpowers/specs/2026-08-04-transync-syntax-split-design.md` (v2). All
changes ride the still-open 0.2.0 breaking window.

**Added:**

- **`crates/transync-syntax`** — the workspace's fifth member and the new home
  of `parser` (+`ranges`/`refdefs`), `id`, `regen`, `render` (+`attrs`),
  `align`, `htmlseg`, `error::ParseError`, plus two modules the split created:
  `outcome` (the per-block `HtmlOutcome` closure) and `walk` (one shared
  top-level normalization + per-list item count, consumed by the renderer
  **and** by `validate::full_reparse`, so the two cannot drift).
  `transync-core` keeps `pipeline`/`unit`/`batch`/`llm`/`validate`/`cache`/`profile`.
  Deliberately **no `[features]`** and **no core-directed dev-dependency**.
- **Standing wasm gate.**
  `cargo check -p transync-syntax --target wasm32-unknown-unknown` in both
  pre-commit hook copies and `scripts/smoke.sh`. The hook **fails loudly**
  with a `rustup target add wasm32-unknown-unknown` hint when the target is
  missing rather than skipping. Dependency canary re-run for the full set
  (comrak 0.27.0, serde 1.0.228, serde_json 1.0.149, siphasher 1.0.2,
  thiserror 1.0.69, lol_html 2.9.0, htmlize 1.1.0) — compiles; compile-only,
  nothing executed on a WASM host.
- **`pub use transync_syntax;` on the facade**, so consumers can name the base
  crate directly.
- **Guard 1** — `validate::full_reparse` now checks each top-level list's
  direct `Item`/`TaskItem` count against the source's collapsed row count.
  Defense in depth behind `per_kind::check_list`; its real residual is
  cross-unit splice adjacency. Validator tightening only: **no
  `VALIDATION_SCHEMA_VERSION` bump**.
- **Guard 2** — a row the render zip cannot pair (unreachable by
  construction) degrades to escaped source bytes inside its normal wrapper
  without advancing the node cursor: never a panic, never a dropped anchor.

**Changed (breaking, inside the 0.2.0 window):**

- **`regen::regenerate(doc, accepted: &HashMap<BlockId, String>)`** — was
  `&[ValidatedBatch]`. **Absence in the map IS the fallback contract.**
  `regen::collect_translations` was deleted; the pipeline builds the map.
- **`align::build_alignment_map`** — arity 6 → 7; statuses arrive as
  `&HashMap<BlockId, FallbackStatus>` and languages as
  `source_language: &str, target_language: &str` instead of
  `&TranslateOptions`.
- **New public item** `regen::top_level_blocks`, plus the `walk` and `outcome`
  modules and the `#[doc(hidden)]` items the crate boundary forced public
  (inventory handed to OI-0027).
- **No module path changed.** `transync::{parser, id, regen, render, align}`,
  `transync::error::ParseError`, `transync::unit::html_outcomes`, and the root
  `BlockId`/`FallbackStatus` aliases all still resolve — core re-exports the
  moved modules.

**Changed (rendered output):** the renderer now does **one Comrak parse per
pane** — **2 parses per run** instead of `2 × (N_blocks + N_ol_runs)` fragment
reparses — and the per-fragment link-reference-definition append is gone.
`strip_outer_wrapper`, `block_inner_html`, and `ordered_list_start` were
**deleted**. Three byte shapes changed — a consumer diffing rendered HTML
against a stored golden must regenerate it:

- loose source lists now render **loose** (`<li><p>…</p></li>`) in **both**
  panes, matching CommonMark, where per-item reparse used to flatten them;
- loose task items carry Comrak's `<input …/>`-then-`<p>` shape verbatim;
- **bugfix:** 4-space-indented code blocks used to render as `<p>` because the
  pre-reparse `md.trim()` destroyed the indent that made them code blocks;
  they now render as `<pre><code>`.

Attribute names/order, wrapper elements, the one-block-per-line shape,
`out.md`, the alignment map, `ALIGNMENT_SCHEMA_VERSION` (`1.2.0`),
`VALIDATION_SCHEMA_VERSION` (`2`), `CacheKey`, and both `sync.js` copies are
**unchanged**.

Verified: fmt/clippy clean, wasm gate green, `cargo test --workspace` green —
`transync-syntax` 96, `transync-core` 165 (+1 ignored golden-regen helper), 21
scenario + 6 `boundary_v02` + 2 `error_fixtures` + 2 `reader_honesty` + 2
`docs_index_drift` in the facade crate, `transync-openai` 25 (+2 ignored live),
CLI 11 + 3 drift.

**Also opened:** **OI-0033** — `LineOffsets` counts only `\n`, so a lone-CR
source desyncs the sourcepos→byte mapping and can produce empty or
item-swallowing payload slices. Pre-existing and orthogonal to this wave;
found by the probe that **disproved** the spec's Guard-1 residual (a).

### Feature wave — HTML-content translation (2026-08-04)

Block-level raw HTML moved from "preserved, escaped placeholder" (DCR-0013) to a
**translatable kind**. Recorded in **ADR-0018** — which **amends architectural
invariant 7** in `CLAUDE.md` — and **DCR-0016**; **ADR-0012** gained a dated
amendment (raw inline-HTML tags become protected structure, with no policy gate)
and **DCR-0013** a superseded-in-part banner. Implements the owner-approved
design spec `docs/superpowers/specs/2026-08-03-html-content-translation-design.md`
(v2). Verified: fmt/clippy clean, `cargo test --workspace` green
(`transync-core` 235 + 1 ignored golden-regen helper, 21 scenario/integration
tests, `transync-openai` 25 + 2 ignored live, CLI 11 + 3 and 19 with
`--features test-stub-provider`), Playwright SCN-13 suite **8/8**. All changes
ride the still-open 0.2.0 breaking window.

**Shipped:**

- **`BlockKind::Html { block_type }`** — a **new explicit** `NodeValue::HtmlBlock`
  arm in the parser replaces the catch-all that used to send those nodes to
  `BlockKind::Skipped { label: "html-block" }`. ID code `html` (`html-0004`),
  wire form `html`. The comrak HTML block type (CommonMark 1–7) is recorded
  because splice-time normalization is type-conditional.
- **New segment engine `transync-core::htmlseg`** — one **pinned `lol_html`
  `Settings`** shared by the extract pass and the splice pass (so the two can
  never disagree on tokenization): scan → coalesce chunks per text node →
  decode entities → drop whitespace-only segments *judged on the decoded form*,
  each survivor carrying a positional index and a parent-element label
  (`summary`, `td`, `pre`, …). Splice is positional with an **identity skip** —
  a segment whose translation equals its decoded source is not replaced at all,
  so `preserved` and echoed blocks are byte-identical to source **by
  construction**. Blank-line collapse applies to block types **6/7 only**
  (CommonMark's definition, whitespace-only lines included); type-1 (`<pre>`,
  `<textarea>`) is exempt. Plus render-path `balance_fragment` and the
  `tag_inventory` used by the post-splice check.
- **Segment units on the wire** — new `InputMode::HtmlSegments`
  (`html_segments`); the payload is a **string containing a JSON array of
  strings**, one element per segment, in order. New public
  `HtmlSegmentConstraints` on `BlockConstraints.html` carries the segment count
  and parent labels as prompt-visible hints and the source bytes + block type as
  **validator-only** fields never serialized into a prompt. `unit::html_outcomes`
  computes a per-block `HtmlOutcome { Unit, PreservedZeroSegment,
  ExtractionFailed }` **once per run**, and every html-aware stage reads that one
  map, so a block's alignment row, its counters, and its presentation cannot
  disagree. **Counting rule:** unit-backed blocks count in `validation_summary`;
  zero-segment and extraction-failed blocks do **not** (same rule as Image rows)
  and each emits a reader-honesty warning row on
  `ValidationReport::skipped_source_nodes`.
- **Validation layers** — per-kind `check_html` (JSON array of strings, exact
  count, no empty element; all **retryable**); a layer-3 post-splice check whose
  *engine* faults take a **DIRECT fallback** (no retry burned, nothing cached,
  `rejected_by`/`rejection_reason` both `None` — consumers must join
  `final_status` with the warnings channel); the fragment-reparse layer
  **returns early** for `Html` after its shared empty-payload guard (the payload
  is a segment list, not Markdown); and `html` labels on **both** sides of the full-document reparse
  compare, without which every spliced block would label-mismatch the
  `"skipped"` catch-all and cascade to fallback.
- **Always-on inline raw-HTML tag guard** — the ordered sequence of comrak
  `HtmlInline` literals must survive byte-verbatim across every paragraph-family
  payload. **Hoisted above** `check_inline`'s policy early-return, so
  `preserve_urls = false` with no code pledge cannot disable it: this is a
  structural identity check, not a content policy (delimits ADR-0012). The user
  prompt carries the matching instruction **unconditionally**, which is a
  conscious regeneration of DCR-0015's byte-identity prompt goldens; the html
  segment instruction itself is conditional, so a batch with no html units keeps
  the legacy instruction byte-for-byte.
- **Live rendering + fallback presentation** — successful blocks render as real
  content inside the standard `<div>` sync wrapper (`data-block-kind="html"`),
  **auto-balanced on the render path** (unclosed tags closed at the fragment
  end, orphan close tags dropped, and a self-closing raw-text/RCDATA start tag —
  `<script/>`, `<style/>`, `<textarea/>`, `<title/>` — counted as **open**,
  because HTML ignores the self-closing flag there) so an unbalanced fragment
  can never consume the wrapper's own `</div>` and swallow later anchors; a
  Playwright assertion
  pins that no sync wrapper is nested inside another after mount. `out.md` keeps
  the true fragment bytes. Extraction-failed and fallback blocks reuse the
  DCR-0013 escaped `<pre data-skipped="html-block">` placeholder — now the
  **failure-only** presentation. Mount path is unchanged: the shells' DOMPurify
  fail-closed sanitize-before-mount.
- **`<details>` toggle mirroring** in both byte-mirrored `sync.js` copies —
  capture-phase listener (the `toggle` event does not bubble), mirror by index
  within the matching `data-sync-id` anchor, equality guard so the mirrored
  event is a no-op (no ping-pong), unwired on `destroy()`.
- **Alignment schema `1.1.0` → `1.2.0`** (additive `html` `block_kind`), full
  lockstep: `ALIGNMENT_SCHEMA_VERSION`, `KNOWN_SCHEMA` in both `sync.js`
  copies, the Playwright forward-drift probe moved to `1.3.0`, and the 12
  scenario pins + the CLI smoke pin.
- **`VALIDATION_SCHEMA_VERSION` `1` → `2`** — justified by the new
  `html_segments` payload semantics, the prompt instruction changes, and the new
  always-on guard (**not** a validator tightening of an unchanged contract).
  Currently **costless**: the shipped cache is in-memory and hits are
  re-validated (ADR-0015). `CacheKey`'s shape, the `Cache` trait, and
  `ProviderFingerprint` are unchanged.
- **New dependencies** — `lol_html` **2.9.0** (streaming rewriter; byte-exact
  preservation of untouched markup) and `htmlize` **1.1.0** (`unescape` — the
  full named-entity decode table `lol_html` lacks), both canaried as
  **compiling** for `wasm32-unknown-unknown` (compile-only, nothing executed)
  so the OI-0028 base-crate split's dependency set stays WASM-clean.
- **New scenario `SCN-15`** (`scn-15-html-blocks.md`: interleaved `<details>`
  region, `<div align="center">` hero, HTML `<table>`, comment-only block,
  `<pre>` with interior blank lines, inline `<kbd>`) plus Playwright test **h**;
  `scn-14-full.md` gained an appended html tail (`html-0015`..`html-0018`).

**Accepted limits (documented, not defects — full list in DCR-0016 Part D):**
the `<details>` fold is not reproduced (an interleaved region renders a live
translated `<summary>` above an always-visible body); translation quality is
capped at text-node granularity; `out.md` gains entity-**form** drift for
genuinely translated segments (identity-skipped ones stay byte-exact); the tag
guard has a known false-reject when a model moves a type-6 tag such as `<div>`
to line-start (verbatim retry usually recovers — type-7 tags like `<b>`/`<kbd>`
cannot interrupt a paragraph, comrak-verified and pinned); attribute text
(`alt`/`title`/`aria-label`) and `<template>` content are never translated.

### OI-resolution wave — OI-2026-08 (2026-08-03)

Resolved five of the seven open issues the DR-2026-07 design review opened
(OI-0026, OI-0029, OI-0030, OI-0031, OI-0032). Recorded in **DCR-0014**
(pipeline quality) and **DCR-0015** (surface & infra), with dated amendments on
ADR-0009 and ADR-0013. Verified: fmt/clippy clean, `transync-core` 164 tests,
17 scenarios, `transync-openai` 25 (+2 ignored live), CLI stub 11 + 19 + 2. All
surface changes ride the still-open 0.2.0 breaking window; the alignment-map
schema stays **1.1.0** and `CacheKey` / the `Cache` trait /
`VALIDATION_SCHEMA_VERSION` are **unchanged**. OI-0027 (public-surface
hardening) and OI-0028 (WASM compile path) remain open.

**Shipped:**

- **Auto-extracted candidate glossary (OI-0026, DCR-0014 Part A).** New
  defaulted `Translator::extract_glossary` preflight (`Ok(None)` =
  unsupported, `Ok(Some(vec![]))` = supported-but-empty) runs once per run
  after ID assignment; `profile::merge_auto_glossary` folds the harvest into
  the run's glossary with **static entries winning**, scope forced
  `GlobalAcrossDocument` (ADR-0014), a 24-term cap, and length bounds that act
  as the invariant-7 prompt-stuffing guard. A `Cow<TranslateOptions>` shadow in
  `run_pipeline` makes prompts, the batch wire field, **and** cache identity
  derive from one effective profile, so the merged terms enter the key twice
  through the existing `glossary_hash` + `profile_prompt_hash` — **no
  `CacheKey` change**. Extraction failure / unsupported / empty-document all
  degrade to static-only and are key-identical to an opted-out run; this is
  explicitly **not** an ADR-0017 terminal event. Default **OFF**: flag >
  profile `auto_glossary` > `false`; CLI `--auto-glossary` /
  `--no-auto-glossary`; outcome on
  `ValidationReport.auto_glossary: Option<AutoGlossaryReport>` (absent when
  off, so opted-out report JSON is byte-identical). First-occurrence pinning
  was evaluated and rejected — undefined under concurrent dispatch, corrupts
  cache identity, breaks ADR-0009's verbatim-retry comparability.
- **Per-batch schema-fault fairness (OI-0031, DCR-0014 Part B).**
  `validate/schema.rs` gains `SchemaClassification` + `classify()`;
  `validate_batch` is now **total over the requested units** and salvages
  innocents — a unit with exactly one result row is validated and accepted in
  round 1 even when the batch faulted. Missing / duplicated ids become
  `Schema`-layer offender rows, re-dispatched verbatim with an ADR-0009
  `RetryContext` and charged **once per round** to the new
  `TranslateOptions.max_per_batch_schema_retries` (default 2, library-only —
  symmetric with `max_per_unit_validation_retries`). Bookkeeping splits into
  `dispatch_counter` / `unit_fault_counter` / `batch_fault_rounds`, so the two
  budgets drain independently and pre-wave histories are numerically
  unchanged. Foreign ids are discarded **uncharged**; duplicate ids in the
  *request* are now a loud terminal `TransyncError::Validation` preflight
  instead of a silent all-fallback. Rounds are bounded at
  `1 + batch budget + U × unit budget`. Amends ADR-0009 additively; ADR-0017
  untouched.
- **Provider-neutral assembly lifted into core (OI-0029, DCR-0015 Part A).**
  New `transync-core::llm::prompt` exposes `SCHEMA_NAME`, `build_user_prompt`,
  `schema_object_for`, and `parse_batch_output(text, &BatchId)`; the
  hint/envelope structs moved private. `transync-openai` keeps dispatch,
  transport, and both surface wrappers, with the `reasoning-effort` `call_*`
  signatures **untouched**. Byte-identity is pinned by four goldens under
  `llm/prompt/golden/`, generated from the **pre-lift** `client.rs` before any
  code moved; the independent wave review rebuilt the pre-lift assembly and
  confirmed byte-identity. New `#[non_exhaustive] llm::TokenizerHint`
  (`O200kBase` / `Cl100kBase`) + defaulted `Translator::tokenizer_hint()`
  (`None` = legacy model-name heuristic) replace the engine's guess; the hint
  is deliberately excluded from `fingerprint()` / `CacheKey`. Model-identity
  dual-homing is resolved **by documentation**: the `Translator` instance is
  the authority and `TranslateOptions::model_id` is demoted to an advisory
  tokenizer-fallback + cache-key label (a mismatch only over-distinguishes —
  the safe direction).
- **RTL pane direction (OI-0032, DCR-0015 Part B).** Precedence
  `--target-direction` > profile `[render].target_direction` (new additive
  `ProfileRender` on `ProfileMetadata`; unknown values warn and normalize) >
  `auto`, where `auto` matches the label's ASCII-lowercased first `-`/`_`
  token against a 15-entry table (`ar arc ckb dv fa he iw ji nqo ps sd syr ug
  ur yi`; `ku` deliberately excluded). RTL stamps ` dir="rtl"` on the pane and
  **LTR emits nothing**, so ko/ja/en bundles stay byte-identical (a review
  finding that the template had gained a shipped-in-every-bundle comment was
  fixed by removing it). The source pane auto-resolves from the resolved
  source label. **Bundle-only:** `out.md` and the alignment map are untouched,
  the schema stays `1.1.0`, and `sync.js` (direction-agnostic) plus both
  byte-mirrored copies are unchanged. Amends ADR-0013: a presentation-layer
  hint over labels that stay opaque for prompt, cache, map, and validation,
  with expressive labels documented as staying LTR.
- **Double-gated live smoke (OI-0030, DCR-0015 Part C).**
  `crates/transync-openai/tests/live_smoke.rs` adds two `#[ignore]`d `tokio`
  round-trips — `gpt-4o-mini` → Chat Completions and `gpt-5-mini` →
  Responses, both env-overridable — gated by **both** the `#[ignore]` and a
  self-skip guard requiring `TRANSYNC_LIVE_SMOKE=1` *and* `OPENAI_API_KEY`,
  with the decision factored into a pure `decide_gate` predicate unit-tested
  offline. Assertions are structural (unit count, matching anchor counts,
  `fallback_source < total_units`, output differs from source) to survive model
  nondeterminism, and the fixture shape is pinned offline at **4** units —
  heading + paragraph + two list items; the D2 design's "3" was a miscount,
  since the IR has no list-*container* kind. New `scripts/smoke-live-gate.sh`
  (0755) is the human opt-in wrapper. Scheduled CI live smoke stays out of
  scope (no CI in this repo). A live-smoke release-gate step is documented in
  the Developer Guide and tracked in `status.md`.

**Signature / surface changes (0.2.0 window):**

- `transync::llm::prompt` — **new module**; `parse_batch_output` is now public
  and takes `&BatchId` (was private, took `&TranslationBatch`).
- `unit::build_batches` gained a trailing `tokenizer_hint: Option<TokenizerHint>`
  parameter.
- `batch::BatchBudget` gained a `tokenizer_hint: Option<TokenizerHint>` field.
- `batch::output_budget_warnings` gained a trailing `tokenizer_hint` parameter.
- `TranslateOptions` gained `auto_glossary: Option<bool>` and
  `max_per_batch_schema_retries: u32`.
- `ValidatedBatch` gained `batch_fault: Option<BatchFault>`.
- `AttemptOutcome` gained `batch_fault: bool` (skip-serialized when false —
  wire-additive; report JSON is unchanged until a batch fault occurs).
- `ValidationReport` gained `batch_schema_faults: u32` and
  `auto_glossary: Option<AutoGlossaryReport>`.
- `ProfileMetadata` gained `render: ProfileRender` — serde-defaulted, but the
  serialized JSON now carries a `"render"` object.
- `transync-cli::output::html_bundle_files` gained two parameters
  (`source_dir_attr`, `target_dir_attr`).
- New re-exports: `TokenizerHint`, `BatchFault` (plus
  `GlossaryExtractionRequest`, `MergedGlossary`, `merge_auto_glossary`,
  `AutoGlossaryReport`, `AutoGlossaryStatus`,
  `DEFAULT_MAX_AUTO_GLOSSARY_TERMS`, `MAX_EXTRACTION_SOURCE_BYTES`).
- `Translator` gained **two defaulted methods** (`tokenizer_hint`,
  `extract_glossary`) — additive for external implementors.
- Behavior change for direct `process_one_batch` / `validate_batch` callers: a
  request batch with duplicate `unit_id`s now aborts loudly instead of
  silently falling everything back (unreachable via `run_pipeline`).

### Design-review wave — DR-2026-07 (2026-07-27)

The 2026-07 design/architecture fitness review produced owner decisions
(2026-07-24) and this implementation wave (verified: fmt/clippy clean,
workspace tests green, CLI stub green, Playwright SCN-13 7/7). All surface
changes ride the still-open 0.2.0 breaking window.

**Owner decisions:**

- **Abort-all kept (ADR-0017).** Batch-terminal provider failures (output
  truncation/incomplete, refusal, malformed JSON, exhausted transport retries)
  keep whole-run abort semantics. A per-batch fallback rung and a policy flag
  were both considered and **rejected** by the owner; mitigation is
  prevention-side (below), not a fallback rung.
- **CLI disk cache / `--resume` stays deferred.**

**Shipped:**

- **Output-aware batch packing + five CLI batching flags (DCR-0012).**
  `group_by_token_budget` now enforces a second cap — estimated output tokens
  (`ceil(source × output_expansion_factor)` + echoed id + per-unit envelope,
  against `target_output_tokens − 64` reserve) — alongside the input cap;
  either binding first breaks the batch. `ProfileBatching` gains
  `output_expansion_factor` (default 2.0) and `target_input_tokens_per_batch`.
  An unset `target_output_tokens` disables output-aware packing **and** the
  preflight, bit-identical to pre-wave packing (pinned by a regression oracle).
  A typed `OutputBudgetWarning` preflight names any single oversize unit
  (`ValidationReport.output_budget_warnings` + `tracing::warn!` +
  terminal-error annotation on the abort it predicts). New flags:
  `--target-output-tokens` (0 = disable ceiling), `--output-expansion-factor`,
  `--target-input-tokens-per-batch`, `--max-units-per-batch`,
  `--max-concurrent-batches`; precedence flag > profile > default.
- **Reader-honesty rendering (DCR-0013).** `BlockKind::Skipped { label }`
  gives unmodeled top-level nodes (raw HTML, front matter, …) an anchored,
  HTML-escaped `<pre data-skipped>` placeholder in **both** panes (never live
  HTML, invariant 7) plus an honest alignment row (preserved, anchored,
  uncounted in `validation_summary`) instead of silently vanishing.
  Document-level link-reference definitions are recovered from inter-block gaps
  (`parser/refdefs.rs` → `Document.ref_defs`) and appended to fragments in
  render and inline validation, so `[text][ref]` resolves to a real link in
  both panes and a mistranslated reference label is now a retryable `Inline`
  rejection. Alignment-map schema **1.0.0 → 1.1.0** (additive `skipped` kind +
  `data-skipped` attribute; `KNOWN_SCHEMA` bumped in both byte-mirrored
  `sync.js` copies; Playwright forward-drift test moved to 1.2.0). Web shells
  gained a tint legend (`pointer-events: none`) + `partially_translated`
  styling + `pre[data-skipped]` styling.
- **Cache twin-block fix.** A cached `UnitResult.unit_id` is rewritten to the
  requesting unit's id at hit time, so a byte-identical twin block is no longer
  stranded into a silent `FallbackSource` (cross-block dedup preserved).
- **Flag-sentinel fix.** `--target-input-tokens-per-batch` now overlays onto
  the resolved profile like the other profile-homed flags, closing the hole
  where a flag value equal to the built-in default silently lost to the
  profile. Only `--max-concurrent-batches` writes onto `TranslateOptions`.

**Breaking changes (0.2.0 window):**

- `validate_batch` gained a trailing `ref_defs: &str` parameter.
- `group_by_token_budget` takes a `BatchBudget` instead of positional args.
- `ProfileBatching` lost `Eq` (keeps `PartialEq`).
- Alignment-map schema is now `1.1.0` (`Document` also gains a `pub ref_defs`
  field; `ValidationReport` gains `output_budget_warnings`).

### External-review wave — v0.2.0 (2026-07-13)

Executed all ten recommendations of the 2026-07 external architecture
review (EXT-2026-07 P0-1..P2-10) plus its decision-process cleanup.
Workspace version bumped 0.1.0 → 0.2.0 (one sanctioned breaking window).

- **Packaging: `crates/transync` → `crates/transync-core`, and `transync`
  became a thin facade (breaking for path/workspace consumers).** Every
  engine source file moved verbatim to a crate renamed `transync-core`, and
  the `transync` name was re-created as a facade whose entire body is
  `pub use transync_core::*` plus a `test-stub` feature forwarded to
  `transync-core/test-stub`. This is the commit pair that added the fourth
  workspace member and carried the 0.1.0 → 0.2.0 version bump, and it is
  what v0.1.0's "Core library — `transync`" now means: the engine ships
  under a different crate name. Because the facade re-exported wholesale,
  `transync::…` item paths kept resolving; what breaks is anything naming
  the engine by crate or path (a `path = "crates/transync"` dependency, a
  vendored checkout, a manifest expecting the engine's own `[features]`),
  which must retarget to `transync-core`. `transync-core` also took `tokio`
  as a regular dependency — the `time` primitive only, for provider-retry
  backoff sleeps; the core stays HTTP-free per ADR-0002. Completes
  **DCR-0005**'s semver-firewall reframing. The wholesale re-export is not
  the end state: the 2026-08-04 surface wave above replaced it with an
  explicit curated list and removed the engine module paths.
- **Fetched pane fragments are sanitized, and mounting fails closed.**
  DOMPurify 3.2.6 is vendored at `web/vendor/purify.min.js`; both demo
  shells sanitize every fetched fragment before mount and refuse to mount
  at all when DOMPurify is unavailable. The CLI mirrors the asset, so the
  `--html-out` bundle now writes **six** files — `index.html`,
  `source.html`, `target.html`, `alignment.json`, `sync.js`,
  `purify.min.js` — not the five documented under v0.1.0, with the
  embedded copy byte-equality-checked against the workspace copy.
- **v0.2 provider/cache boundary (P1-4, breaking):** `Translator` gains
  `fingerprint()` (defaulted; `TransyncOpenAI` covers model + base URL +
  API surface); `Cache` trait v2 is fallible with required `evict`;
  `CacheKey` gains `provider_fingerprint` + `validation_schema_version`;
  the pipeline evicts reparse-cascade-downgraded keys and
  `Hard`-implicated keys (targeted — OI-0011 keep-progress preserved);
  cache errors degrade, never abort. Resolves OI-0017 items 1–3.
- **RetryContext side channel (P0-2):** re-dispatched units carry
  `TranslationUnit.retry: Option<RetryContext>` (attempt, rejecting
  layer, truncated reason) serialized as a data-framed prompt field —
  the non-content channel ADR-0009 anticipated; payload resubmission
  stays verbatim.
- **Resource limits (P1-7, amends ADR-0016):** CLI `--max-input-bytes`
  (64 MiB default, limit+1 reads), 4 MiB profile/prompt caps, 32 MiB
  provider response ceiling, live `target_output_tokens` output ceiling
  (`max_completion_tokens`/`max_output_tokens`). Resolves OI-0019.
- **Staged fileset commit + `--out-dir` (P1-6):** the output contract is
  now honestly named (phase-2 rename exposure documented); new
  `--out-dir` publishes the complete output set via a single directory
  rename with backup/rollback.
- **Cache poison policy fixed (P2-8, amends ADR-0015):** clear-once-and-
  resume via `Mutex::clear_poison` — the old miss-forever/dead-write
  asymmetry is gone.
- **Structural ownership completed (P2-9):** list marker facts
  (`start`/`delimiter`/`tight`) fingerprinted and `<ol start>` preserved
  in rendering (closes DCR-0007's limitation); blockquote fingerprints
  recurse one level; `Preserved` results must byte-match the source.
  Resolves OI-0022.
- **SCN-13 automated (P2-10):** headless Playwright suite
  (`web/tests/scn13.spec.js`, 6 scenarios) via `scripts/test-browser.sh`
  is now the primary browser-sync evidence.
- **Reserved settings stop lying (P0-3, amends ADR-0014):** glossary
  scope `conditional-on-section` is rejected at load until section-aware
  batching lands; "section-aware batching" and browser reflow-hook
  claims corrected to shipped/deferred labels.
- **One architectural baseline (P0-1):** BL-2026-07-B issued with an
  explicit documentation-authority hierarchy; the 2026-05 baseline and
  brainstorming spec are marked historical; the pipeline diagram now
  models the real state machine (full reparse after regeneration, cache
  write/evict points, policy branches).
- **Decision hygiene:** ADRs 0001–0007 gained frontmatter (0007's title
  typo fixed); stale stricter-prompt/row-window claims corrected;
  source-of-truth table single-owner violation fixed; DCR-0005 reframed
  (facade = semver firewall, dialect trait deferred); new
  `docs_index_drift` test pins ADR/DCR ↔ index coverage.

### Tracked open-issue cleanup — v0.2.0 (2026-07-13..14)

Closed the actionable (unblocked, non-deferred) entries in the open-issue
ledger, each with a discriminating regression test that fails under the old
behavior:

- **Retry BatchId no longer collides at scale (OI-0025).** The retry ordinal
  sequence is seeded from the input batch count
  (`AtomicU32::new(batches.len() as u32 + 1)`) instead of a fixed `10_000`, so
  retry IDs are provably disjoint from the `1..=N` input IDs for any document
  size. Test: `retry_batch_ids_never_collide_with_input_batch_ids`.
- **Validation report is in document order (OI-0021 item 2).**
  `build_validation_report` sorts `per_unit` by the block's index in
  `doc.blocks` (source order) with a stable tiebreak, instead of
  lexicographically by unit-ID (which grouped by kind prefix `c-`/`h1-`/`li-`).
  Test: `validation_report_per_unit_is_in_document_order`. (Item 1, cache-hit
  labeling, was fixed earlier in the wave.)
- **Test-coverage gaps closed (OI-0023 items 1–3).**
  `cli_html_out_force_overwrites_foreign_file` (refusal vs `--force` overwrite
  of a foreign `--html-out` dir; the per-file overwrite leaves the foreign file
  intact), `cli_model_and_base_url_reach_provider` (flag→provider propagation
  via a feature-gated `TRANSYNC_STUB_ECHO_PATH` echo hook), and
  `abort_keeps_cached_progress` in SCN-10 (a mid-run non-transient
  `TranslatorError` aborts the run yet keeps the pre-abort cache entries — the
  OI-0011 contract; the assertion fails under a blanket-clear). Item 4
  (browser) was closed earlier by the Playwright suite.
- **Forward-compat schema drift is visible (OI-0024 item 2, 2026-07-14).**
  `loadAlignment` in `web/js/sync.js` now emits a `console.warn` when a
  same-major alignment map carries a newer minor/patch than the engine's
  known `1.0.0` schema (still accepted); previously silent at `console.debug`.
  Byte-mirrored to the CLI bundle copy (`sync_js_drift` green) and pinned by
  Playwright test `g`. (Item 1, reflow-recalculation hooks, stays deferred
  into OI-0015.)

### Added

- **Inline-protection validation layer (EXT-2026-07 P1-5, ADR-0012
  amendment):** link/image destinations are compared source-vs-translated
  (ordered, autolinks included) unless the profile sets
  `preserve_urls = false`; `preserve_code_identifiers = true` now enforces
  inline code-span identity as a multiset. Mismatches are retryable
  rejections surfaced as the new `"inline"` layer in validation reports;
  user prompts gain matching policy-gated instructions.
- **Glossary as a first-class profile feature.** Profile TOML now
  loads `[[glossary]]` entries (typed: `source` / `target` / `note` /
  `scope`); `profile::render_prompt_body` substitutes language
  variables AND appends a rendered glossary bullet section to the
  system prompt. Entries stay accessible programmatically on
  `ProfileMetadata.glossary` so custom `Translator` impls can read
  them without re-parsing the prompt body. Default profile ships
  example entries (`agent → 에이전트`, `tool use → 도구 사용`) for
  discoverability.
- **Token-budget batching via `tiktoken-rs`.** `TranslateOptions`
  gains `target_input_tokens_per_batch` (default 6000) as a soft
  cap; `max_units_per_batch` (default 32) becomes a hard cap.
  `batch::group_by_token_budget` packs units up to the soft cap
  using the model's tiktoken encoding (`o200k_base` for gpt-4o /
  gpt-5 / o-series, `cl100k_base` otherwise) plus a small per-unit
  JSON overhead. Replaces the prior count-only batching.
- **Concurrent batch dispatch.** Pipeline dispatches batches via
  `futures::stream::buffer_unordered` up to
  `TranslateOptions::max_concurrent_batches` (default 6). Total
  wall-clock now scales with the slowest concurrent batch rather
  than the sum of all batches; on `samples/demo-complex.md` this
  cuts a typical run from ~30 s to ~5 s. Set to 1 to force
  sequential dispatch.
- **Dual-API dispatch in `transync-openai`.** Provider now picks
  between `/v1/chat/completions` and `/v1/responses` per request
  based on the model name: `*-chat-*` aliases route to Chat
  Completions; `o1` / `o3` / `o4` / `gpt-5*` route to Responses;
  others default to Chat Completions. Override with
  `TRANSYNC_OPENAI_API=chat|responses`. The same strict
  Structured-Outputs JSON schema is sent to both surfaces.
- **Three-theme browser demo.** `index.html.tpl` ships scoped CSS
  under `body[data-theme="default|document|book"]` with a top-right
  `<select>` picker; selection persists in `localStorage` under
  `"transync.theme"`. Default keeps the v0.1.0 look; document
  emphasizes table borders and zebra rows; book uses serif
  typography with generous leading for long-form prose.
- **Documentation suite.** New developer-facing guides:
  `docs/Quick_Start.md`, `docs/Developer_Guide.md`,
  `docs/Troubleshooting.md`, `docs/Profile_Cookbook.md`,
  `docs/Performance.md`. README and `docs/index.md` updated to lead
  with the two feature pillars (translation + scroll sync) and link
  the new guides.
- **Documented annotated-HTML wrapper positioning contract (R0005-0001).**
  `contracts.md` §4a now explicitly describes the `<main>` wrapper
  structure (single outer element; blocks as direct children in source
  order) and names the layout precondition for `offsetTop`-based scroll
  math: the consumer's scrollable pane element MUST be
  `position: relative`. The renderer deliberately does NOT stamp
  positioning on `<main>` (doing so would silently shift `offsetTop`
  by any pane padding when the consumer already styled their pane
  correctly). Reference `web/js/sync.js` `mountSync` doc-block and
  inline scroll-math comments now reference §4a instead of the demo
  shells. See DCR-0003.
- **`FallbackPerBlock` widens fallback radius and auto-escalates (R0006).**
  When the first per-block fallback's re-reparse still fails — a real
  failure mode where boundary contamination from a list-item's
  translation lands the hallucinated reparsed block inside the
  *neighboring* source block's byte range, so byte-offset attribution
  flags the wrong block — the policy now widens to fall back both
  preceding and following top-level neighbors of every flagged block.
  If the widened reparse also fails, it auto-escalates to source bytes
  for every block (always succeeds structurally). `FullReparseFailure::
  FallbackPerBlock` no longer returns `Err`; callers who relied on the
  Err signal must switch to `Hard`. Each stage is visible via
  `tracing::warn`. See DCR-0004.
- **Recoverable post-regen full-reparse failures (R0004-0001).**
  When the post-regeneration full-document reparse rejects the output,
  the pipeline now identifies the divergent source block(s) via byte-
  offset attribution, marks them as `FallbackStatus::FallbackSource`,
  and re-runs regen+reparse once. Behavior is selected by the new
  `TranslateOptions.full_reparse_failure: FullReparseFailure` knob
  (`Hard` = today's `Err`, `FallbackPerBlock` = new default,
  `FallbackAll` = always source bytes). Failure messages now name the
  source-side `BlockId`s flanking the divergence point so consumers
  can log/display the regression locus. New
  `TransyncError::stable_code() -> &'static str` lets downstream wire
  formats map errors without enumerating variants.

### Review 0008 wave (2026-07-11)

65 accepted findings from the external Review 0008 audit (91 findings:
65 fixed, 4 tracked as OI-0017..0020, 1 recorded as ADR-0011, 13
rejected as settled/not-a-bug, 8 dropped by routing).

- **Fixed — validation (core):** cache key now hashes provider-visible
  context hints (R0008-0001); duplicate *requested* unit IDs rejected
  (R0008-0022); list topology fingerprints each item's direct child
  kinds (R0008-0014); blockquote child fingerprint labels unknown nodes
  instead of dropping them (R0008-0015); retry units carry the retry
  batch's `batch_id` (R0008-0020); token budgeting reserves the system
  prompt + glossary + fixed envelope per batch (R0008-0029); skipped
  top-level source nodes (raw HTML blocks etc.) surface as
  `Document::warnings` → `ValidationReport.skipped_source_nodes` → CLI
  stderr notes instead of vanishing silently (R0008-0013).
- **Fixed — CLI output staging:** duplicate output destinations
  rejected before I/O (R0008-0005); staging temps open with
  `create_new` so a planted symlink cannot truncate another file
  (R0008-0006); stale-temp preflight cleans only temps stamped with the
  CURRENT pid, so a concurrent run's staging files are left alone
  (R0008-0007); phase-1 failures remove directories this call created
  (R0008-0041); renames preserve an existing target's permissions
  (R0008-0042); whitespace-only profile slug/version rejected
  (R0008-0024); `--source-language`/`--model` validated non-empty
  (R0008-0039); unknown `{{template}}` variables in the system prompt
  warn (R0008-0027).
- **Fixed — provider (`transync-openai`):** 408/409/425 classified as
  transient (retryable) instead of terminal (R0008-0032); non-http(s)
  or hostless base URLs rejected at construction (R0008-0033); error
  diagnostics capped at 512 bytes (R0008-0034); Chat Completions parser
  accepts content-part arrays and surfaces refusals explicitly
  (R0008-0035); Responses parser reports `incomplete` status/reason,
  concatenates all output_text segments, and surfaces refusal segments
  (R0008-0036); unused direct deps (serde, thiserror, tracing, toml)
  removed from the CLI manifest (R0008-0060).
- **Fixed — browser/demo:** alignment-map `schema_version` must be full
  `x.y.z` semver (R0008-0045); target-pane drift check keys on
  `target_block_id` (R0008-0046); `mountSync` tears down a previously
  installed controller instead of double-mounting (R0008-0048); panes
  are keyboard-focusable (`role="region" tabindex="0"`, R0008-0049);
  the generated bundle stamps `<html lang>` and per-pane `lang`
  attributes from the run's target/detected-source languages
  (R0008-0050).
- **Fixed — scripts:** deletion guards reject `..` paths (R0008-0008);
  smoke checks verify the sixth bundle asset `purify.min.js`
  (R0008-0052); Python 3 + `http.server` probed before any deletion
  (R0008-0053); `smoke-live-long.sh` mirrors the portable workdir
  fallback (R0008-0054); inline `--system-prompt` values redacted from
  logs (R0008-0055); the pre-commit hook is now tracked at
  `scripts/hooks/pre-commit` with `scripts/install-hooks.sh`
  (R0008-0058).
- **Docs realigned** (~35 findings, R0008-0023/0057/0061..0090):
  README status/cache/retry wording, Developer_Guide crate split + leaf
  model, Troubleshooting `--validation-report` flow, contracts §2
  opaque profile version + live retry/CLI/serve behavior,
  architecture/mvp-scope/module-map/rough-schema brought to the live
  tree, status.md + phase-state.yaml to steady-state, stub-manifest
  re-audited, ADR-0001/0003/0006 implementation notes refreshed.
- **Records:** ADR-0011 (deliberate LAN bind in convenience wrappers);
  OI-0017 (cache identity/eviction design), OI-0018 (CSP/remote-image
  posture), OI-0019 (provider resource bounds), OI-0020 (Cargo.lock vs
  global gitignore).

### Review 0001/0002/0003 fix wave (2026-05-04)

Three internal audits landed back-to-back the day after v0.1.0 — Review 0001
(the large one), then Reviews 0002 and 0003 — as roughly a dozen fix commits
sitting between the feature adds under **Added** and the 2026-05-05
full-reparse recovery work (R0004-0001 / R0005-0001 / R0006, also under
**Added**). Everything here rides the sanctioned 0.2.0 breaking window.

**Changed (breaking, inside the 0.2.0 window):**

- **Structural drift after regeneration became fatal (R0001-0002).** A failed
  post-regeneration full-document reparse was a `tracing::warn` followed by
  `return Ok` — a structurally drifted document was handed back to the caller
  as a successful translation. It now returns `TransyncError::Validation`.
  (The 2026-05-05 `FullReparseFailure` knob under **Added** later made this
  recoverable per block; the behavior introduced here is what that knob calls
  `Hard`.)
- **The parser stopped emitting nested children of list items and blockquotes
  (R0001-0003/0020/0032/0033).** List items, task items and blockquotes are
  leaf translation units — their `source_range` already covers the nested
  content. So `AlignmentMap.blocks` now contains only top-level units, nested
  child IDs (a `p-NNNN` inside a list item) are no longer present, and
  `ValidationReport.per_unit` / `ValidationSummary.total_units` count only
  the units the provider actually translated. Regeneration had always
  iterated top-level blocks only, so those child translations were being
  requested, validated, cached — and dropped on the floor.
- **`InputMode::CodeContent` → `InputMode::FullCodeBlock` (R0001-0026).** The
  old name claimed the unit carried body content with the regenerator owning
  the fences, while `block_payload` had always sliced the whole fenced block
  and `check_code` had always expected a full fenced envelope back. The
  `language_info` field is unchanged, but an external `Translator` matching
  on `InputMode` must update the variant name.
- **`CacheKey` gained three fields, invalidating every pre-existing entry.**
  `source_lang`, so swapping `auto` for a concrete language no longer reuses
  entries produced under a different source-language contract (R0001-0012);
  `profile_prompt_hash`, a SipHash-1-3 of the *rendered* prompt body, so
  `--system-prompt` / `--system-prompt-file` edits invalidate even on an
  unchanged profile version (R0001-0011); and `glossary_hash`, so editing
  `[[glossary]]` entries without bumping the version can no longer reuse
  stale translations (R0002-0003). `profile_version` is now read off the
  rendered profile rather than a `"1.0.0"` fallback.
- **`source_hash_block` returns `Option<u64>` (R0001-0099).** It used to
  return `0` for an out-of-range block index — indistinguishable from a
  legitimate zero hash.
- **`build_user_prompt` returns `Result<String, TranslatorError>`
  (R0001-0052).** A serde failure used to be swallowed into an empty user
  message; it now surfaces as `ProviderError::Other`.
- **Profiles without a usable `[system].prompt` are rejected (R0001-0014,
  R0001-0015).** `load_profile` raises `MissingField` when the key is missing
  or empty, so a profile that loaded under v0.1.0 without one now fails; and a
  parse failure on the *embedded* default profile panics with a "build asset
  is broken" message instead of degrading to an empty-prompt fallback that
  silently disabled the safety text.
- **A translatable block missing from the validation lookup is now
  `FallbackSource` (R0001-0021).** It used to default to `Translated` —
  untranslated output reported as translated. Non-translatable kinds
  (thematic break, image) keep the `Translated` no-op default.
- **New `ValidationLayer::Provider` variant (R0001-0006).** A provider-side
  `FailedNeedsFallback` is now a Provider-layer rejection, so the per-unit
  retry budget applies to it instead of being bypassed.
- **Two CLI exit codes changed.** `transync serve` — still the deferred
  placeholder — returns `ExitCode::Other` instead of `0`, so a script cannot
  mistake the deferred notice for a successful bind (R0001-0028); and
  `TransyncError::Parse` maps to `ExitCode::InputReadFailure` (2), the code
  `contracts.md` §6 documents, instead of collapsing into `Other` (5)
  (R0001-0059). Scripts keyed on the old codes change behavior.
- **`--quiet` and `--verbose` became mutually exclusive (R0003-0045).** clap
  `conflicts_with` fails the invocation at arg-parse time; passing both used
  to be accepted.
- **`translate_with_cache` is the public partial-resume seam (R0001-0071,
  R0001-0075).** Callers can run passes against an externally-managed
  `&dyn Cache`, and `translate` delegates to it. An empty or whitespace-only
  `target_language` returns `TransyncError::Internal` before any provider
  call.
- **`ValidatedUnit` carries the provider's `UnitResult.warnings` forward
  (R0001-0073).** They used to be dropped at the validation boundary, so a
  caller could not surface them in its own logs.
- **The renderer resolves blocks by ID (R0001-0070).** `render_source` /
  `render_target` look `doc.blocks` up by `source_block_id` through a
  `HashMap` instead of trusting alignment-row order, so a reordered or
  filtered alignment map can no longer render the wrong block under a sync
  ID.

**Changed — what the provider actually receives:**

- **The user prompt gained structural hints (R0001-0004).** Each unit now
  ships an `input_mode` label plus two advisory blocks: `constraints`
  (heading level, table columns and row count, code-fence info, list
  topology depth/count, blockquote child kinds) and `context` (document
  title, section path, preceding and following summaries). Both serialize as
  omitted when entirely empty, and the instruction text marks them advisory —
  match when possible, never invent structure not in `source_payload`. The
  strict response schema is unchanged, but the request bytes, and therefore
  model output, differ from v0.1.0. These are the hint structs the 2026-08-04
  prompt work later froze into goldens.
- **The Structured-Outputs schema constrains unit count (R0001-0051).** The
  `units` array carries per-batch `minItems`/`maxItems`, so strict Structured
  Outputs forces the model to answer with exactly the batch size.
- **`Retry-After` is honored (R0001-0049).** A 429 carrying the header now
  surfaces as `TranslatorError::RateLimited { retry_after: Some(_) }` through
  a new `ProviderError::RateLimitedAfter` variant, so the bounded retry can
  pace to the provider's own guidance.
- **Provider retries actually run (R0001-0007).**
  `max_per_batch_provider_retries` was configured and never read — every
  `TranslatorError` surfaced immediately. A `translate_with_provider_retries`
  wrapper now re-dispatches `Network` and `RateLimited` failures up to the
  configured budget; auth, malformed and unsupported still surface at once,
  because retrying them only burns quota.
- **Table alignment is enforced (R0001-0018).** `check_table` compares
  `constraints.must_preserve_table_alignment` against the reparsed alignment
  vector; swapped `:---` / `:---:` / `---:` markers were silently accepted
  before, and are now a rejection reason.
- **Glossary bullets are escaped and honest (R0002-0014, R0002-0017).**
  Entries are `\"`-escaped before being spliced into the `"<src>" → "<tgt>"`
  prompt list, so a `"` or `\` inside an entry cannot break out of its quoted
  segment; section-scoped entries render with a "currently advisory only"
  suffix, because the renderer treats them as global today.
- **Retry stopped polluting the payload.** `retry_validation_unit` appended
  `[Validation rejected the previous attempt; reason: …]` onto
  `source_payload` before re-dispatch; a faithful provider treats
  `source_payload` as content, translated the note, and echoed it back as an
  extra block in the regenerated document — which the now-fatal full reparse
  surfaced immediately. The append is gone and the structural retry mechanic
  is unchanged; the hint returned as the non-content ADR-0009 `RetryContext`
  side channel in the 2026-07-13 wave above.

**Fixed:**

- **CLI flags that were parsed and ignored started working (R0001-0005,
  -0008, -0009, -0010, -0027, -0062).** `--model` builds the live provider,
  where `from_env()` used to ignore it — so the cache and `TranslateOptions`
  claimed one model while the provider used another; `--base-url` is plumbed
  through `TransyncOpenAI::try_new` with a `TRANSYNC_OPENAI_BASE_URL`
  fallback; a missing `OPENAI_API_KEY` returns a documented exit code with a
  stderr diagnostic instead of panicking on `.expect()`; `--force` gates the
  write for real — `ensure_html_out_safe` refuses a non-empty `--html-out`
  directory holding files outside the bundle's own names unless it is set,
  where the bundle used to overwrite silently; and the emitted `index.html`
  no longer ships literal `{{TRANSYNC_DOC_LANG}}` / `{{TRANSYNC_TITLE}}`
  placeholders for the browser to render.
- **`--base-url` values ending in `/v1` no longer double up (R0003-0070).**
  `build_endpoint` strips a trailing `/v1` before joining, so an
  OpenAI-compatible proxy URL stops producing
  `https://proxy/v1/v1/chat/completions` — the URL actually called changes.
- **`ValidationSummary.retried_units` is populated (R0001-0025).** It read
  `0` in every validation report and every `--validation-report` output; it
  now counts units with an attempt beyond the first (cache hits, at attempt
  0, do not count).
- **`block_payload` snaps to UTF-8 char boundaries (R0001-0029).** A
  surprising offset could panic the slice on multi-byte source.
- **Byte ranges stop leaking into the next block (R0001-0034, R0001-0035).**
  `byte_range_for` clamps its `+1` end byte to the owning source line, and
  `pos_to_byte` clamps `col` against that line's own byte span, so a range
  can no longer pull in the following block's leading byte.
- **Rendered attribute values are HTML-escaped (R0001-0068).** The attribute
  writer escapes `&`, `"`, `<` and `>` on every value at the boundary.
- **Ordered task lists inherit the parent list's `ordered` flag
  (R0001-0031).** A `TaskItem` AST node carries no list-type marker of its
  own — it lives on the owning `List` — so the walker now iterates
  `NodeValue::List` children directly. Ordered task lists change the IDs and
  markers they emit.
- **Heading context reaches the model without its `#` markers (R0001-0041,
  R0001-0098), and `<hr />` is emitted in canonical void-element form
  `<hr …>` (R0001-0097)** — the latter a byte change for anyone diffing
  rendered HTML against a v0.1.0 golden.
- **A poisoned cache mutex recovers instead of disabling the cache
  (R0001-0100).** `get` / `put` recover through `into_inner` and emit a
  `tracing::warn`, rather than treating every subsequent access as a silent
  miss.
- **Atomic writes clean up and land durably (R0001-0060, R0001-0061).** The
  failure path best-effort removes the `.tmp.<pid>` file so a half-written
  temp cannot leak, and the parent directory is fsynced after the rename
  (POSIX-only; a silent no-op where opening a directory fails).
- **Exit code 3 became reachable (R0001-0022).** The alignment summary counts
  only translatable units — excluding thematic breaks and images — so the
  CLI's all-fallback check fires when every translatable unit fell back.
  Without it, exit 3 was unreachable on any document containing a thematic
  break.
- **Section-path context lookup is linear (R0001-0042).** `build_context`
  builds one id→block `HashMap` per call instead of rescanning `doc.blocks`
  for every hop.

**Browser demo (`web/js/sync.js`, kept byte-identical with the CLI-embedded
copy):**

- **Real user input takes back the driver lock.** The smooth-follow loop
  re-armed the per-pane programmatic-scroll lock on every animation frame,
  so wheel / touch / pointer input on the auto-scrolled pane hit the same
  lock and was ignored while the loop yanked the pane back to its old target
  — "scroll pane A, then try pane B, and it moves a little and snaps back".
  `wheel`, `touchstart` and `pointerdown` now cancel that pane's pending
  smooth-follow RAF, clear its target and reset its lock, so the pane's next
  scroll event wins driver ownership immediately. `destroy()` reuses the same
  release path and cancels the in-flight RAF for both panes (R0003-0085).
- **Scroll math measures with `offsetTop` / `offsetHeight`.** The per-RAF
  `getBoundingClientRect` calls (two in `handleScroll`, one plus one per
  block in `activeBlockWithProgress`) are gone in favor of pane-relative,
  layout-cached reads against `pane.scrollTop` / `pane.clientHeight`; on a
  100-block document the worst case drops from ~101 forced layouts per frame
  to none. This is the change that created the pane `position: relative`
  precondition documented under **Added** as R0005-0001.
- **`[data-sync-id]` lookups are cached at mount.** `mountSync` builds
  per-pane element arrays and id→element `Map`s once, instead of running
  `querySelectorAll` on every scroll frame; the hand-rolled `cssEscape` went
  with them. The caches live for the controller's lifetime, so a re-render
  means `destroy()` and re-mount — now stated in `mountSync`'s docstring.
- **`safeStorage` and `fetchOk` became named exports of `sync.js`.** Both
  shells carried byte-identical inline copies and now
  `import { mountSync, safeStorage, fetchOk }` instead. A public addition to
  the shipped JS module surface: `safeStorage` wraps `localStorage` in
  try/catch so sandboxed iframes and private-mode browsers do not break the
  demo on a thrown `SecurityError` (R0003-0042), and `fetchOk` throws on a
  non-2xx response before the body is read.
- **HTTP errors surface as a controlled error pane (R0001-0063)** — fetches
  are ok-checked before the body is read, instead of an error page being
  mounted as the pane's HTML — **and the active-block scan no longer breaks
  early** on the first block below the viewport (R0001-0066), which
  multi-column and nested-wrapper layouts can order non-monotonically in the
  DOM.

## [0.1.0] - 2026-05-03

First public release. Implements every mandatory MVP scenario
(SCN-01..SCN-14) end-to-end against the live OpenAI Responses API,
plus a vanilla-JS browser demo with smooth block-level scroll sync.

### Design baseline

- Design baseline `BL-2026-05-01-A` issued via `/design-first-architecture`.
- Brainstorming spec `docs/superpowers/specs/2026-05-01-transync-design.md` and follow-up spec `docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`.
- Six accepted ADRs:
  - ADR-0001 — block-level alignment as the only sync currency.
  - ADR-0002 — HTTP-free core with `Translator` trait + provider crates.
  - ADR-0003 — Cargo workspace with provider crates as separate workspace members.
  - ADR-0004 — Comrak as the GFM parser.
  - ADR-0005 — `BlockId` format `<kind>-<NNNN>` with separate `source_hash: u64`.
  - ADR-0006 — renderer emits two pane fragments + a templated `index.html` shell.

### Core library — `transync`

- GFM parser via Comrak with stable kind-prefixed sequential `BlockId`s and SipHash-1-3 source hashing (SL-00).
- Pipeline orchestrator: parse → unit-build → batch → translate → validate (schema → per-kind → fragment-reparse → full-reparse) → regenerate → align → render.
- Per-kind validators with structural fingerprints captured at unit-build time:
  - heading level (SL-01)
  - table column count + alignment + row count (SL-02)
  - code-block info string + safe fence sizing (SL-04)
  - nested-list `(depth, ordered, task)` topology (SL-05)
  - blockquote child-kind sequence (SL-06)
- Retry + fallback state machine: per-unit validation retry (max 2), provider-side retry hook, fallback to source on persistent failure (SL-07, SL-08, contracts.md §5).
- Profile TOML loader + embedded default profile with `{{source_language}}` / `{{target_language}}` template substitution per call (SL-09).
- `source-language="auto"` propagated end-to-end via `detected_source_language` on the alignment map and the `TranslationOutput` (SL-11).
- Long-doc batching with in-memory cache (`Mutex<HashMap>`) keyed on `{source_hash, target_lang, profile_version, model_id, block_kind}`. Partial-resume across runs that reuse the same cache produces byte-identical output (SL-10).
- 200-row table fixture round-trips through the whole-block path (SL-03; row-window splitter is a documented post-MVP refinement).
- Full-document reparse: `validate::full_reparse::reparse_full` asserts top-level kind sequence + count parity between source IR and regenerated MD (SL-14).
- HTML renderer: `<main>` fragments per ADR-0006 with `data-sync-id` / `data-block-kind` / `data-order` / `data-fallback` / `data-parent-id` attributes; nested blocks live in the alignment map as anchors but are not double-emitted in the rendered DOM.

### Provider — `transync-openai`

- Live OpenAI Responses API integration (`POST {base_url}/v1/responses`) with strict Structured-Outputs JSON schema for `TranslationBatchResult`.
- HTTP via `reqwest` (rustls-tls), 120 s timeout. Status-code mapping: 401/403 → `Authentication`, 429 → `RateLimited`, 4xx → `Other`, 5xx / network → `Network`. Decode failures → `MalformedResponse`.
- `TransyncOpenAI::from_env` honors `OPENAI_API_KEY`, `TRANSYNC_OPENAI_MODEL`, `TRANSYNC_OPENAI_BASE_URL` per `contracts.md` §7. Default model: `gpt-5-chat-latest`.
- `TransyncOpenAI::try_new` validates non-empty api key.
- `EchoTranslator` test-stub provider behind the `transync/test-stub` feature for CI without an API key.

### CLI — `transync-cli`

- `transync translate` subcommand with `--input`, `--output`, `--map`, `--html-out`, `--target-language`, `--source-language`, `--profile`, `--system-prompt`, `--system-prompt-file`, `--model`, `--base-url`, `--force`, `--quiet`, `--verbose`.
- `--profile` loads a Profile TOML from disk; `--system-prompt` / `--system-prompt-file` (mutually exclusive) override the active profile's `[system].prompt` body before template substitution.
- Atomic writes (`.tmp.<pid>` → fsync → rename) for all output paths.
- 5-file `--html-out` bundle per ADR-0006: `index.html`, `source.html`, `target.html`, `alignment.json`, `sync.js`.
- Exit codes 0..5 per `contracts.md` §6: success / argument error / input read failure / all-fallback / write failure / other. Diagnostic stderr on every error path.
- `transync serve` placeholder subcommand (deferred — see `web/SMOKE.md` for the recommended `python3 -m http.server` replacement).
- `test-stub-provider` Cargo feature swaps the live OpenAI provider for `EchoTranslator` so the CLI runs from a clean checkout without an API key.

### Browser demo — `web/js/sync.js`

- Block-level scroll sync — no scroll percentage, no DOM measurement assumptions across panes.
- Smooth proportional in-block following: each scroll frame computes the active block's progress (0..1) from a 4 px reference line, projects that onto the partner block's matching offset, and lerps `partner.scrollTop` toward the target at `SMOOTHING_FACTOR=0.2` per RAF tick. Settles within 5 % of the target in ~250 ms after user input stops.
- Per-pane programmatic-scroll lock (`PROGRAMMATIC_SCROLL_LOCK_MS=90`) absorbs the cascade scroll event from each pane's own `scrollTop` assignment without blocking the other pane's user input.
- Schema-version validation on the loaded alignment map (rejects unknown majors with a clear console warning).
- `mountSync(sourcePane, targetPane, alignmentMap)` returns a `{ destroy }` controller.
- Manual smoke checklist: `web/SMOKE.md`.

### Test + smoke infrastructure

- 13 scenario integration tests in `crates/transync/tests/scenarios.rs` covering SCN-01..SCN-12, SCN-14, plus a partial-resume cache-equivalence check.
- 4 CLI integration tests in `crates/transync-cli/tests/cli_smoke.rs` covering exit codes 0/1/2/4 (gated on `test-stub-provider`).
- `MockTranslator` modes: passthrough, rejects-then-accepts, always-fails-unit, always-fails-all, recording, echoes-detected-language.
- Programmatic fixture generators (`tests/common/fixture_gen::generate_scn_03/10`) for the oversized-table and long-document variants so the repo doesn't bloat with hand-typed dumps.
- 12 hand-edited fixtures + 1 long-form sample (`samples/demo-complex.md`, 92 sync-relevant blocks).
- `scripts/smoke.sh` — Phase-4 hard-gate driver (build + test + CLI dry path with `EchoTranslator`).
- `scripts/smoke-live.sh` — live OpenAI smoke, then exec's `python3 -m http.server` so the demo is reachable from a peer machine on the LAN. Knobs: `TRANSYNC_OPENAI_MODEL`, `TRANSYNC_OPENAI_BASE_URL`, `TRANSYNC_LIVE_INPUT`, `TRANSYNC_LIVE_TARGET_LANG`, `TRANSYNC_LIVE_WORKDIR`, `TRANSYNC_LIVE_PORT`, `TRANSYNC_LIVE_BIND`, `TRANSYNC_LIVE_PROFILE`, `TRANSYNC_LIVE_SYSTEM_PROMPT`, `TRANSYNC_LIVE_SYSTEM_PROMPT_FILE`.

### Bugs fixed during the cycle

- UTF-8 char-boundary panic in the renderer when the translated string was wider than the source (multi-byte CJK). `align::build_alignment_map` now anchors nested blocks to their parent's target range; `render::block_text` snaps slice indices to char boundaries.
- Duplicate list-item paragraphs in the rendered DOM. The renderer now skips child blocks (`parent_id != None`); list-item and blockquote wrappers switched from `<li>` / `<blockquote>` to `<div>` to avoid double-nesting Comrak's HTML.
- Sync freeze when the smooth animation loop re-armed a shared `lockUntil`. Lock split into per-pane numbers (`{ source, target }`) so the partner pane's animation does not block the input pane's handler.
- Silent CLI failures: every error path in `translate_cmd::run` now prints a `transync: <reason>` diagnostic on stderr unless `--quiet` is passed. `--verbose` adds a validation-tally summary on success.

### Tracking artifacts

- `docs/project/design-baseline.md` — implementation-ready handoff spec.
- `docs/project/stub-manifest.md` — every `STUB`/`DEFERRED` placeholder accounted for. MVP-required code paths carry zero `STUB` markers; 11 post-MVP placeholders remain as `DEFERRED` with written justification (live row-window splitter, `tiktoken-rs` token estimation, real static HTTP `serve`, HTML bundle template substitution, etc.).
- `docs/project/implementation-slice-checklists.md` — SL-00..SL-14 done-gate criteria.
- `docs/project/phase-state.yaml` — final state: `design.status: handoff_complete`, `implementation.status: mvp_complete`.

### Verification (replayable from a clean checkout)

```
cargo build --workspace                                            # clean
cargo test --workspace                                             # 13 scenario tests pass
cargo test -p transync-cli --features test-stub-provider           # 4 CLI tests pass
cargo clippy --workspace --all-targets -- -D warnings              # clean
./scripts/smoke.sh                                                 # exit 0
OPENAI_API_KEY=... ./scripts/smoke-live.sh                         # live API + browser demo
```

[Unreleased]: https://github.com/QuietJoon/transync/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/QuietJoon/transync/releases/tag/v0.4.0
[0.3.0]: https://github.com/QuietJoon/transync/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/QuietJoon/transync/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/QuietJoon/transync/releases/tag/v0.1.0
