# Developer Guide

How to **use**, **extend**, and **customize** transync. This is the
practical companion to `docs/architecture/README.md` (which explains
*why* the system is shaped this way). For a 5-minute "first run"
walkthrough see `docs/Quick_Start.md`.

## The two features

transync delivers two related capabilities. Most consumers want both;
some only want one.

### Feature 1 — Markdown translation pipeline

Turn a GFM Markdown document into a translated GFM Markdown document
**without breaking its structure**, regardless of which LLM you point
at it.

- **Block-aware parsing.** Comrak walks the source AST and produces an
  intermediate representation where every sync-relevant block carries a
  stable `BlockId` (`<kind>-<NNNN>`), a byte range into the source, and
  a SipHash-1-3 source hash. List items and blockquotes are themselves
  leaf translation units — the walker never recurses, so the IR carries no
  parent linkage at all, and the alignment map's `parent_id` is a reserved,
  always-`null` field held for a future nested-anchor revision
  (OI-0005 [archived]).
- **Per-block translation units.** Each translatable block becomes a
  `TranslationUnit` with `source_payload`, surrounding context (section
  path, neighbor snippets, document title), and per-kind structural
  constraints (`must_preserve_table_columns`, `expected_list_topology`,
  `expected_blockquote_children`, `must_preserve_code_fence_info`,
  `must_preserve_heading_level`, …).
- **Pluggable provider via the `Translator` trait.** HTTP-free,
  async, object-safe. Default impl in `transync-openai` does
  model-driven dispatch to the OpenAI Chat Completions and Responses
  APIs with strict Structured Outputs; sibling crates can target
  Anthropic, OpenRouter, local llama, or any LLM endpoint.
- **Layered validation.** Every batch result is checked at seven
  layers: schema, ID-set equality, per-kind shape (table column count,
  list topology, code-fence info, heading level, blockquote children),
  fragment reparse, visible-text presence (a translation that erases every
  visible character of a source that had some is rejected — the one content
  question the structural layers cannot ask; ti `c887bc`), inline
  protection (link/image destinations; policy-gated code spans), and a
  final full-document reparse. Any
  failure triggers a bounded verbatim resubmission of the failed unit —
  same payload, same scope — carrying a non-content `RetryContext` side
  channel (attempt, rejecting layer, reason) alongside, never inside,
  the payload (ADR-0009, DCR-0009); persistent failure falls back to
  source bytes with `fallback_status: fallback_source`. The pipeline
  never silently corrupts output.
- **Profile TOML.** System prompt template (with
  `{{source_language}}` / `{{target_language}}` substitution), per-call
  glossary, batching hints. Override via `--profile`,
  `--system-prompt`, or `--system-prompt-file`.
- **Long-document batching + cache.** The batcher partitions the units
  by section first (DCR-0027: a batch never straddles a heading) and
  packs each section under a token budget — a 6000-token soft input
  target per batch, plus a hard unit cap that is **8**, not the `32` on
  `TranslateOptions::default()`, on any run that does not set the field
  itself: the shipped default profile carries
  `[batching].max_units_per_batch = 8`, and a caller value equal to the
  built-in default reads as unset (contracts.md §2). An in-memory
  `Cache` keyed on `{provider_fingerprint,
  validation_schema_version, source_hash, source_lang, target_lang,
  profile_version, profile_prompt_hash, glossary_hash, model_id,
  block_kind, input_mode, context_hash, instruction_hash}` makes partial-resume runs
  byte-identical to the original — `detected_source_language` included
  since v0.4.0: the `Cache` also holds one **document-level** record per
  document, written from a live run's qualifying envelope and replayed by
  a run that dispatched zero provider batches, so a fully-cached resume
  reports the detection the live run made rather than `null`
  (contracts.md §5a). The last two key axes are the parts of the prompt
  that are not the unit's own text: the context hints it was sent with,
  and the instruction the batch it was packed into assembled
  (contracts.md §5a).
- **Staged fileset commit.** The CLI stages every output through
  `.tmp.<pid>` → `fsync` → `rename`, committing the whole set together:
  a failure while content is being staged touches no output, and only a
  crash or I/O error during the final rename pass can leave a mixed set.
  A *second run* cannot: every directory a publication writes into is held
  under an exclusive OS file lock (a zero-byte `.transync-publish.lock`
  marker, deliberately left behind) for the whole staging-and-rename span,
  so concurrent runs take turns (DCR-0021). Exit codes 0..7 distinguish
  success / argument error / read failure / all-fallback / write failure /
  other / configuration rejected / document refused.

If you only want this half, your code path is:

```
transync::translate(&source, &opts, &translator)
    -> TranslationOutput { translated_document, alignment_map, ... }
```

You can ignore the rendered HTML fields entirely.

### Feature 2 — Block-level scroll sync

A vanilla-JS engine that keeps two browser panes — source on the left,
translation on the right — in lockstep by **block ID**, not by scroll
percentage. Critical because:

- Translated text often grows or shrinks the document; a 50%
  source-scroll position does not correspond to 50% in the translated
  pane.
- Tables can wrap differently across languages (Korean cells double
  the byte width but stay one visual line; that breaks any height-based
  sync).
- Code blocks stay nearly the same height while their neighboring
  prose grows; percentage sync drifts within a single section.

The mechanism:

- The same `BlockId` flows from the parser IR through the LLM
  contract, into the regenerated Markdown, into the alignment-map
  JSON, and finally onto the rendered DOM as `data-sync-id`
  attributes. The JS engine reads only those attributes — it never
  parses Markdown independently (per ADR-0001).
- **Smooth proportional in-block following.** On every scroll frame,
  the active pane's reference line (4 px below the top) determines
  the active block and the user's `progress ∈ [0, 1]` within it. The
  partner pane's `scrollTop` is set so the matching block displays the
  same proportional offset. A per-frame lerp closes
  `SMOOTHING_FACTOR = 0.2` of the gap each RAF tick — the partner
  glides into place over ~250 ms.
- **No oscillation.** A per-pane programmatic-scroll lock (90 ms)
  absorbs the cascade scroll event each pane fires when *we* set its
  `scrollTop`, without blocking the other pane's user input.
- **Runtime CSS themes.** Three ship today (Default / Document /
  Book) and you can drop in more via additional
  `body[data-theme="…"] .pane …` rules. Choice persists in
  `localStorage["transync.theme"]`.
- **Schema-versioned alignment map** (`schema_version: "1.3.0"`).
  Consumers MUST reject unknown majors; minor/patch additions are
  backward-compatible.

If you only want this half — say, you have your own translation
pipeline and just want the dual-pane sync — your inputs are:

1. The annotated HTML pane fragments (`source.html`, `target.html`)
   with `data-sync-id` on every sync-relevant block (non-sync rows such
   as thematic breaks omit it — see DCR-0007 / contracts §4).
2. An alignment map JSON conforming to `schema_version 1.3.0`.

Then `mountSync(sourcePane, targetPane, alignmentMap)` from
`web/js/sync.js`. It returns a controller, or **`null`** when it refuses
the map (unknown major, an unusable row, or no synchronizable row for
panes that carry anchors) — treat `null` as *not mounted* rather than as
a mount you can ignore the result of. The full attribute and JSON
contracts are in `docs/architecture/contracts.md` §3 and §4.

### How the two features connect

```
   .md  ──▶  parse  ──▶  Document IR  ──▶  build batches  ──▶  Translator
                                                                    │
                                                                    ▼
                                                          per-batch result
                                                                    │
                              regenerated MD  ◀── regen ◀── validate (7 layers)
                                       │
                          ┌────────────┴────────────┐
                          ▼                         ▼
                    AlignmentMap                 source.html
                    (schema 1.3.0)               target.html
                          │                         │
                          └─────────┬───────────────┘
                                    ▼
                          web/js/sync.js (block-level scroll sync)
                                    ▼
                              browser demo
```

The translation feature is everything to the left of the alignment
map; the scroll-sync feature is everything to the right.

---

## Workspace at a glance

```
transync/
├── crates/
│   ├── transync-lang/        source-language gate — "should a translation run start at all?"; depends on no workspace member, carries no serialization (ADR-0028 / DCR-0045)
│   ├── transync-html/        HTML mechanics: tag scan, element extents, segment extract/splice, fragment balancing; lol_html + htmlize only (DCR-0032)
│   ├── transync-syntax/      syntax layer (parser, id, regen, render, align, outcome, walk); compiles for wasm32 under a standing gate
│   ├── transync-core/        pipeline on top (unit, batch, llm, validate, cache, profile, pipeline); no HTTP, no LLM dep
│   ├── transync/             curated facade (semver firewall) — an EXPLICIT re-export list, not a glob; contracts.md §0 is its table of contents
│   ├── transync-openai/      Translator impl; model-driven dispatch to Chat Completions + Responses
│   ├── transync-anthropic/   Translator impl; Anthropic Messages API. In-tree but on NO run path — transync-cli depends only on transync + transync-openai (ADR-0002 / DCR-0029)
│   ├── transync-cli/         binary; embeds the demo shell + sync engine
│   └── transync-wasm/        browser (wasm-bindgen) surface over transync-syntax; publish = false
├── web/                      vanilla-JS demo source-of-truth
│   ├── index.html            standalone shell
│   ├── demo-wasm.html        WASM render+edit demo shell (ADR-0019)
│   ├── js/sync.js            block-level scroll sync engine
│   ├── js/wasm-demo.js       WASM demo controller (local render + edit loop)
│   ├── wasm/                 build output of scripts/build-wasm.sh — GITIGNORED
│   └── SMOKE.md              manual smoke checklist
├── samples/                  hand-edited example inputs
└── scripts/
    ├── smoke.sh              full hard gate: build + wasm gate + both test suites + rustdoc gate + wasm build + dry-stub end-to-end
    ├── build-wasm.sh         wasm-pack + explicit binaryen wasm-opt + size budget
    ├── test-browser.sh       headless Playwright: SCN-13 + SCN-16 + the wasm demo
    ├── smoke-live.sh         live OpenAI smoke + transync serve
    └── test.sh               local convenience wrapper for smoke-live
```

## Build, test, lint

```bash
cargo build --workspace
cargo test --workspace                                 # unit + integration
cargo test -p transync-cli --features test-stub-provider  # CLI smoke
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all
cargo check -p transync-syntax -p transync-wasm \
  --target wasm32-unknown-unknown                       # standing wasm gate
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps \
  -p transync-html -p transync-syntax -p transync-core -p transync \
  -p transync-openai -p transync-anthropic -p transync-wasm   # standing rustdoc gate
./scripts/build-wasm.sh                                # demo module + size budget
```

The pre-commit hook runs `cargo fmt --check`, `cargo clippy
-D warnings`, and both standing gates above — wasm and rustdoc; any of
them failing blocks the commit. Run `cargo fmt --all` before staging if
rustfmt complains. The hook that actually runs is the tracked
`scripts/hooks/pre-commit`, wired in by `scripts/install-hooks.sh` setting
`core.hooksPath=scripts/hooks` — run that script once per clone.

**It is never bypassed.** `git commit --no-verify` is not an option here:
there is no CI behind the hook, so the four gates above run nowhere else
by themselves and a skipped hook is an unchecked commit until someone
runs `scripts/smoke.sh`. `docs/project/release-checklist.md` says the
same for the release-prep commit, `docs/Troubleshooting.md` says what to
run when a gate blocks you, and
`crates/transync/tests/docs_gate_claims_drift.rs` keeps all three from
drifting apart again.

The installer will not take a hooks configuration away from you. If
`core.hooksPath` already names some other directory — yours, or one
inherited from your global config — it prints what it found, lists the
three ways forward (chain the two hooks, take over, or clear the value)
and exits non-zero having changed nothing. `--force` takes over anyway:
a local value is saved to `transync.replacedHooksPath` before being
overwritten, an inherited one is left alone and merely shadowed by the
local value, and either way the exact restore command is printed. Once
the value is already `scripts/hooks`, re-running is a one-line no-op.

**The rustdoc gate** (DCR-0018, widened to `transync-openai` on
2026-08-07) keeps `cargo doc` warning-free for **every workspace member
that has a library target** — `transync-lang`, `transync-html`, `transync-syntax`,
`transync-core`, `transync`, `transync-openai`, `transync-anthropic`, `transync-wasm`. `transync-cli` is the one
member outside it, and deliberately: it is bin-only, with no public API
to document. Its usual failure is an intra-doc link from a public item to
a private sibling: either widen the linked item or de-link the prose to
plain backticks, whichever the curation calls for.

Two things run it, and neither owns the crate list. The list lives in
`scripts/lib/rustdoc-gate.sh`, sourced by `scripts/smoke.sh` — which runs
the gate after the test suites — and by `scripts/hooks/pre-commit`, which
runs it on **every commit** (since 2026-08-07; a warm re-document of the
eight crates costs about a second, next to the `--all-targets
--all-features` clippy run already there). Add a new library member to
that one file and both callers pick it up. Keep the list total — a member
left out is a member whose docs rot unobserved, which is exactly how
`transync-openai`'s public `client` module doc came to carry eight links
to private items — and `smoke.sh` enforces that for you: it walks
`crates/*/` and fails the run when a member with a `src/lib.rs` is not
named in the list.

**The wasm gate** (DCR-0017, widened by DCR-0020) keeps `transync-syntax`
— the parser / regen / render / align layer — and `transync-wasm` — the
browser bindings over it — compilable for the browser. It needs the
target installed once:

```bash
rustup target add wasm32-unknown-unknown
```

The hook **fails loudly** with that hint if the target is missing rather
than skipping the check, so the gate cannot quietly stop being real.
`scripts/smoke.sh` runs it unconditionally. Three rules keep it working:
`transync-syntax` declares **no `[features]`** (a feature there
reintroduces the feature-unification trap the crate split exists to
avoid) and **no dependency on `transync-core`**, not even a
dev-dependency (that would force the gate down to `--lib` and lose
test-target coverage); and **`transync-wasm` depends on
`transync-syntax` alone** among workspace crates — `transync-core`'s
`wasm32` tree reaches `getrandom` through tokio/tiktoken, so widening
that edge breaks the link, not merely the charter.

**The wasm demo module** (ADR-0019) is a build artifact, not a
checked-in one: `./scripts/build-wasm.sh` emits
`web/wasm/transync_wasm.js` + `transync_wasm_bg.wasm` (gitignored) and
enforces a size budget. It needs two **host prerequisites** beyond the
rustup target:

```bash
brew install wasm-pack
brew install binaryen     # >= 121; the script rejects anything older
```

wasm-pack's own **cached binaryen is not a substitute** — it is version
117, and 117 *rejects* current rustc output
(`Bulk memory operations require bulk memory`). The script therefore
builds with `--no-opt` and invokes the `wasm-opt` it finds on PATH
itself, resolved once by absolute path so it can never drift back onto
the cached copy. Any missing or too-old prerequisite fails **loudly**;
none of them is ever skipped. Any `wasm-opt` ≥ 121 on PATH works —
the install channel is the host's business.

The `test-stub-provider` Cargo feature on `transync-cli` swaps the
live OpenAI client for an in-process `EchoTranslator` so smoke runs
work without `OPENAI_API_KEY`.

**Repository integrity is not assumed here, it is checked.** This clone
lost a git object once (2026-08-07, ticket `7a7feb`: the `docs/` subtree
of one commit disappeared from `.git/objects`, and everything crossing
that commit failed with `fatal: unable to read tree`). The cause was a
file-sync client re-materializing files under `.git/` while git wrote
them, on the external volume the worktree lives on; it is parked rather
than removed. Two consequences for anyone working here:

- **Run `git fsck --no-progress --connectivity-only` before you cut a
  release** — it is step 0 of `docs/project/release-checklist.md` and
  costs a fraction of a second at this size. `missing` and `broken link`
  lines are the fault; `dangling` lines are normal. It is deliberately
  *not* part of `scripts/smoke.sh`: object-store health is a property of
  the clone, not of the change under test, and a gate that can fail for
  reasons unrelated to the diff sends people hunting the wrong bug.
- **A worktree shared by two concurrent sessions wants `git config
  --local gc.auto 0`** (set here), so no automatic gc runs inside one
  session's commit while the other is writing objects. Explicit `git gc`
  at a quiet point is unaffected and is the intended replacement.

`docs/Troubleshooting.md` carries the diagnosis and the hash-verified
tree rebuild that recovered the lost object.

---

## Using transync as a library

The smallest end-to-end usage from Rust:

```rust
use transync::{translate, TranslateOptions};
use transync_openai::TransyncOpenAI;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string("doc.md")?;
    let translator = TransyncOpenAI::from_env()?;   // reads OPENAI_API_KEY
    // `TranslateOptions` is `#[non_exhaustive]`: construct it by
    // default-then-assign. Struct literals and `..Default::default()` are
    // not available outside the crate.
    let mut opts = TranslateOptions::default();
    opts.source_language = "auto".into();
    opts.target_language = "ko".into();
    let output = translate(&source, &opts, &translator).await?;

    std::fs::write("doc.ko.md", &output.translated_document)?;
    let json = serde_json::to_vec_pretty(&output.alignment_map)?;
    std::fs::write("alignment.json", &json)?;
    println!("translated {} blocks", output.alignment_map.blocks.len());
    Ok(())
}
```

Key types you'll touch:

| Type                               | Purpose |
|------------------------------------|---------|
| `TranslateOptions`                 | Per-call knobs: source/target lang, model id, retry budgets, optional profile. |
| `TranslationOutput`                | Returns translated MD, the alignment map, source + target annotated HTML, validation report, and detected language. |
| `Translator` (trait)               | The LLM boundary. Either bring your own or use `transync-openai`. |
| `Cache` (trait) + `InMemoryCache`  | Skip the cache to use the default one inside `translate()`, or call `translate_with_cache()` to share a cache across calls. |
| `AlignmentMap`                     | The serializable output the JS sync engine consumes (`schema_version: "1.3.0"`). |
| `BlockId`, `BlockKind`             | Stable per-block identifier and the block-kind enum. |

Reusing a cache across calls (partial-resume across runs):

```rust
use transync::{InMemoryCache, TranslateOptions, translate_with_cache};

let cache = InMemoryCache::new();   // share across calls
let mut opts = TranslateOptions::default();
opts.target_language = "ko".into();  // required; an empty value fails fast
let output1 = translate_with_cache(&source1, &opts, &translator, &cache).await?;
let output2 = translate_with_cache(&source2, &opts, &translator, &cache).await?;
```

Successful translations land in the cache keyed on
`{provider_fingerprint, validation_schema_version, source_hash,
source_lang, target_lang, profile_version, profile_prompt_hash,
glossary_hash, model_id, block_kind, input_mode, context_hash,
instruction_hash}`. A
repeat run against an identical source (and same opts/profile/model)
skips every translator call.

`CacheKey` is exhaustive by policy, so a field addition is a breaking
change — `instruction_hash` is the 0.3.0 one, `input_mode` the 0.4.0 one.
Build a key with a struct
literal naming every field, and destructure one the same way; the
supported set is the one `crates/transync/tests/public_surface.rs`
pins.

---

## CLI reference

Every flag the two subcommands accept, in `--help` order. The one-liners
here are pointers; `contracts.md` §6 is the authority for what each flag
means, and `crates/transync-cli/tests/docs_cli_flags_drift.rs` fails when
this block, that one, and clap stop naming the same set — or when an entry
stops stating the default `--help` prints for its flag. The one-liners
themselves stay prose, unchecked.

```
transync translate
  --input <path>                 (required) source Markdown
  [--max-input-bytes <n>]        refuse a larger --input before parsing;
                                 default 67108864 (64 MiB), exit 2
  [--allow-html-input]           translate an --input whose preamble declares
                                 an HTML document as Markdown anyway; without
                                 it, that input is refused, exit 2. The refusal
                                 is not about a crash: such a run exits 0, with
                                 its prose re-read under Markdown inline rules,
                                 its title and section context silently empty,
                                 and every four-space-indented run re-emitted as
                                 a fenced block (ti d990b6 / 457e51; all three
                                 harms are silent — contracts.md 6). Markdown that
                                 opens with an HTML island; conflicts with
                                 --input-format html
  [--input-format <markdown|html>]    (default: markdown) which intake parses
                                 --input; html is the HTML→HTML path — flag-only
                                 routing, the sniff is never a router
  --output <path>                the translated document; needs --map, unless --out-dir
  --map <path>                   alignment-map JSON; needs --output, unless --out-dir
  --out-dir <dir>                publish the whole output set into one directory;
                                 mutually exclusive with --output, --map, --html-out
  [--html-out <dir>]             six-file demo bundle directory
  [--strict-csp]                 harden the emitted bundle with a CSP meta
  [--title <text>]               bundle <title>; beats the document's first H1
  [--validation-report <path>]   per-unit attempt log + rejection reasons,
                                 as JSON; --out-dir writes its own copy
  --target-language <label>      (required) opaque non-empty label
  [--source-language <label>|auto]             default: auto
  [--target-direction <rtl|ltr|auto>]
                                 dir for the bundle's target pane; default auto
  [--profile <path>]             custom Profile TOML
  [--system-prompt <text>]       overrides profile [system].prompt
  [--system-prompt-file <path>]  same, read from a file
  [--model <id>]                 default: $TRANSYNC_OPENAI_MODEL, else
                                 gpt-5-chat-latest
  [--base-url <url>]             default: $TRANSYNC_OPENAI_BASE_URL, else
                                 https://api.openai.com
  [--cache-dir <path>]           disk-backed translation cache in this
                                 directory (created if absent); absent, the
                                 run uses a fresh in-memory cache. One
                                 writer at a time; an unopenable path is
                                 warned about and the run continues
  [--offline]                    run with NO provider credentials, serving every
                                 unit from the cache. Requires --cache-dir. A
                                 fully warm run completes with no key; one that
                                 misses stops AT the miss with
                                 `no provider available for this run` and exit 6
  [--force]                      overwrite an --html-out / --out-dir holding
                                 foreign files; a previous bundle there is
                                 overwritten without it
  [--target-output-tokens <n>]   per-batch output ceiling; n >= 65, 0 disables it
  [--output-expansion-factor <f>]
                                 assumed output/source token ratio; > 0
  [--target-input-tokens-per-batch <n>]
                                 per-batch input budget; n >= 1
  [--max-units-per-batch <n>]    hard unit cap per batch; n >= 1
  [--table-strategy <whole-block|row-window-first>]
                                 what an oversize table does: split into
                                 header-carrying row windows (default) or
                                 ship whole and abort
  [--max-concurrent-batches <n>] in-flight provider requests; n >= 1
  [--auto-glossary]              run the glossary-extraction preflight
  [--no-auto-glossary]           skip it, overriding a profile that asks
  [--quiet|--verbose]

transync serve                   (loopback static server for a bundle)
  --rendered <dir>               (required) bundle dir
  [--port <u16>]                 default: 7470; 0 asks the OS for a free
                                 one, which is then printed
  [--bind <addr>]                default: 127.0.0.1 (loopback); any other
                                 address is warned about
  [--allow-host <authority>]     answer for this authority too, as `host` or
                                 `host:port`; repeatable
```

A run names exactly one output target: either `--out-dir`, or `--output` and
`--map` together. Neither of the latter two is required on its own — clap makes
`--out-dir` conflict with `--output`, `--map` and `--html-out`, and the
remaining combinations (neither, or one without the other) are argument errors,
exit 1. `--out-dir`'s fixed layout is `contracts.md` §6 "`--out-dir` semantics".

`--force` is about **foreign** files, not about a non-empty directory. Writing
into an `--html-out` that already holds a bundle overwrites the bundle's own
six filenames in place, and republishing over a directory that is a prior
`--out-dir` replaces it — the common case, and neither asks for a flag. What
the preflight refuses is a directory holding an entry transync did not write
(and, for `--out-dir`, a target that is not a directory at all, or one transync
did not publish); `--force` waives that refusal. Three file classes are
transync's own rather than foreign, and every guard skips all three — the two
markers only when the entry wearing the name is a *regular file*, since those
are the two names a guard accepts without looking inside: the
`.transync-publish.lock` marker, the `.transync-out-dir` ownership marker, and
`*.tmp.<pid>` staging leftovers — the last of these now including an `--out-dir`
target's top level, which used to be the one place the allow-list had no room
for them (ti `66339b`; `Troubleshooting.md` has the operator's view). `--out-dir`
also asks one question the bundle guard does not: whether transync *published*
the target, answered by the ownership marker or by the complete output set being
present, so a user directory whose only entry happens to be called `out.md` is no
longer replaced without a flag (OI-0036). The check runs before any output is
written, so a refusal — exit 4 — never strands a half-written output set.
`contracts.md` §6 lists the entries each guard allows.

Both language flags take an **opaque label, not a validated tag**. transync
trims surrounding whitespace once, at the argument boundary, and every
consumer downstream reads that one value: the compiled system prompt, the
cache key, the alignment map's `source_language` / `target_language`, and the
bundle's `lang` attribute. Beyond that trim the label is never validated as a
tag, never case-folded and never rewritten, so BCP-47 codes (`ko`, `ja-JP`,
`zh-Hant`) are the recommended form rather than a requirement, and
`--target-language 'Korean (formal, 존댓말)'` is a legal label that reaches the
model intact — saying register and style in the label is an intended use, not
a loophole (ADR-0013). The only rejected value is an empty or whitespace-only
label, an argument error, exit 1. `auto` is reserved as the
`--source-language` sentinel for model-side detection, and is its default; the
detected value comes back on the alignment map's `detected_source_language`.
Being reserved, it is the one value that *is* case-insensitive: `AUTO` and
`Auto` ask for detection and are stored as `auto`, so however you spell the
sentinel it is one run everywhere — same prompt, same cache identity, same
alignment metadata. Nothing is reserved on the target side, so
`--target-language AUTO` is an ordinary label and keeps its case.
Full rules: `contracts.md` §6 "Language labels at the CLI boundary".

`--system-prompt` and `--system-prompt-file` are mutually exclusive.
Template variables (`{{source_language}}`, `{{target_language}}`) in
the prompt body are substituted per call.

Four flags overlay the profile's `[batching]` section for one run —
`--target-output-tokens`, `--output-expansion-factor`,
`--target-input-tokens-per-batch` and `--max-units-per-batch` —
`--table-strategy` overlays `[constraints].default_table_strategy` under the
same rule, and `--max-concurrent-batches` sets the one runtime knob that has
no profile home. "Batching + concurrency knobs" below is where all six are
explained, including how a `0` resolves. `--auto-glossary` and `--no-auto-glossary` are mutually
exclusive and decide the extraction preflight over whatever the profile
says ("Glossary handling"). `--validation-report <path>` writes the
per-unit attempt log — every rejection reason and provider warning behind
a fallback — as JSON; an `--out-dir` run always writes
`validation-report.json` into its directory, so the flag is for the
`--output` / `--map` shape and is inert alongside `--out-dir` rather than
an argument error. `--target-direction` is bundle-only, like `--title`:
`contracts.md` §6 has its resolution order.

`transync serve --rendered <dir>` serves that directory over HTTP on
`127.0.0.1:7470` until you stop it with Ctrl-C. It prints the address the
kernel actually bound — so `--port 0` is a usable way to get a free port — and
then serves, and nothing else: `GET` and `HEAD` only, no directory listing, no
upload, no execution. A request either names a regular file inside the served
directory or it is refused, and "inside" is decided after the path is
canonicalized, so a symlink pointing out of the bundle is a `403` rather than a
file. Content types come from a fixed extension table rather than from
sniffing the bytes. `--bind` takes any address, but the default is loopback and
anything else prints a warning naming what it just published.

Every request also has to say *who it is for*: its `Host` must name an
authority this server answers for. For a bind that names an address, that is
the address literal, plus `localhost` when the address is a loopback one, plus
whatever `--allow-host` adds. A wildcard bind (`0.0.0.0`, `::`) is the
exception in both directions: it names no interface, so it answers for the
loopback authorities it is also listening on — `127.0.0.1`, `[::1]` and
`localhost` — and for nothing derived from the wildcard itself, so
`Host: 0.0.0.0:7470` is refused like any other unknown name. Anything
else is a `421` (or a `400`, when the request states no `Host` or two of them),
and the server prints the list it answers for at startup so a refusal is not a
mystery. That check is what stands between a loopback bundle and DNS rebinding:
a bind decides which network can open the socket, not which page may read the
answer, and a page on `attacker.example` that rebinds its own name to
`127.0.0.1` is reading the served document under its own origin unless the
authority is checked. Serving a bundle to other machines with `--bind 0.0.0.0`
therefore takes an `--allow-host <the address they reach it at>` as well.

Since the HTML→HTML wave (ADR-0025, ti `490d97`), serve is also the
**fidelity door**: an `--input-format html` run's `--out-dir` holds the
translated page itself — `out.html`, full head, scripts, styles,
anchor-free — beside the sync bundle in `html/`. The bundle's panes
deliberately carry no page fidelity (they are a sync surface — no
layout, no styles, no scripts), so to see the real translated page,
serve the out-dir and open `out.html`:
`transync serve --rendered <out-dir>`.

This retires the deferred stub the command shipped as (STUB-061) from SL-13
until 2026-08-09, which printed an address and exited `5`. Any other static
server still works — the bundle is self-contained — but `scripts/test-browser.sh`,
`scripts/smoke-live.sh` and `web/SMOKE.md` all use this one now. `contracts.md`
§6 has the refusal rules and the exit codes in full.

### What the bundle calls the document (`--title`)

The emitted `index.html` titles itself with the first of these that
exists: `--title <text>`, the source document's **first level-1
heading** (a Markdown run) — or, on an `--input-format html` run, the
source document's `<title>` text, the same extraction the provider saw
as `document_title` (ti `0f26b5`'s chain, middle rung re-seated;
DCR-0038) — or the literal `transync`. The heading is used as prose —
markers, emphasis and code-span backticks are gone, because the parser
consumed them — and it is the same value the model was told the document
is called, so the rendered page and the translation agree. A heading
that renders to nothing falls through to the literal; a blank `--title`
is an argument error rather than a silent fallback.

`<html lang>` carries the run's `--target-language` (the language of the
document the bundle shows), while each pane carries its own `lang`.
`--title` is presentation only: it names the bundle's `<title>` and
reaches neither `out.md`, the alignment map, nor the provider — the
model is told the source document's own first H1, never this flag. The
language label is not: `<html lang>` is its last stop, not its only
one, because the same trimmed value already reached the system prompt,
the cache key and the alignment map's `source_language` /
`target_language` (see the CLI reference above).

### Hardening a bundle you did not write (`--strict-csp`)

Source Markdown is untrusted data (invariant 7), and a remote image URL
in it becomes a tracking pixel: opening the generated bundle fetches it
and tells its host when — and from which IP — the document was read. By
default transync accepts that (documents are usually already-local
copies whose remote images are meant to render), so an unflagged bundle
is byte-identical to what earlier versions wrote.

`--strict-csp` opts a single run out of it by stamping a
Content-Security-Policy `<meta>` into the bundle's `index.html`:

```
default-src 'self'; img-src 'self' data:; script-src 'self' 'unsafe-inline';
style-src 'self' 'unsafe-inline'; connect-src 'self'; object-src 'none'; base-uri 'none'
```

Everything the bundle needs stays legal — its own inline style block and
module script, the two sibling scripts, the three same-origin fetches,
`data:` images — and everything remote stops loading. `'self'` is scoped to the
serving *origin*, not to the bundle directory: it blocks other hosts, not
sibling paths under the document root the bundle sits in. The visible cost is
that legitimately-remote images no longer render; that is the tradeoff,
not a failure. The flag hardens whichever bundle the run emits
(`--html-out` or `--out-dir`'s `html/`); with neither, no bundle exists
and the run says the flag did nothing. Serve the bundle over HTTP
(`transync serve --rendered <dir>`) as usual — `'self'` has no meaning
under `file://`, where the shell's fetches already fail.

### Exit codes

| Code | Meaning                                                                |
|-----:|------------------------------------------------------------------------|
| 0    | success                                                                |
| 1    | argument error (clap rejected the args, or `--profile` failed to parse) |
| 2    | input read failure (`--input` does not exist or is unreadable)          |
| 3    | every translatable unit fell back to source (outputs are still written) |
| 4    | write failure (atomic-write or HTML-bundle write errored)               |
| 5    | other (an unclassified provider failure, JSON serialization failure, …) |
| 6    | the provider refused the run because of how it was configured — fix it and re-run |
| 7    | the provider refused this document's content — no configuration change helps |

Every non-zero exit prints a `transync: <reason>` line on stderr unless
`--quiet` was passed. `--verbose` adds a one-line validation tally on
success.

`6` and `7` are the two a script actually branches on, and they exist
because a script cannot branch on prose. `6` collects the causes with an
operator remedy — a key the provider would not accept, a `--model` it does
not have, an output ceiling or a batch budget too small for the work — so
the wrapper's answer is "change the configuration and run me again". `7`
collects the causes with none: the provider's content policy stopped
generation, or the model declined. Transync's retry is a *verbatim*
resubmission (ADR-0009), so the same document fails the same way, and the
only useful automated response is to skip it. Both abort before publishing
anything (ADR-0017), which is what separates `7` from `3` — `3` writes a
full output set in which every block fell back to source.

Codes are append-only: `6` and `7` were added in 0.4.0 and nothing in
`0`–`5` changed meaning. If you are matching on `5` today to catch provider
failures, note that four of the causes that used to land there now land on
`6` and two on `7`; `5` keeps the ones the taxonomy does not classify.
Library callers get the finer split without exit codes at all, from
`TransyncError::stable_code()` (`contracts.md` §1).

### Environment variables (CLI)

| Variable                    | Default                       | Effect |
|-----------------------------|-------------------------------|--------|
| `OPENAI_API_KEY`            | —                             | Required for the live provider. |
| `TRANSYNC_OPENAI_MODEL`     | `gpt-5-chat-latest`           | Override default model. |
| `TRANSYNC_OPENAI_BASE_URL`  | `https://api.openai.com`      | Point at OpenRouter, Azure, or a local proxy. |
| `TRANSYNC_OPENAI_API`       | model-driven (see below)      | `chat` or `responses` — explicit override for the dispatch heuristic. Read once, when the adapter is constructed. |

Keys are read from the environment and never written anywhere
(`persistence-and-files.md`). If you keep yours in a file, the repo's
`.gitignore` already covers the usual homes — `.env`, the whole `.env.*`
family, `.envrc`, `.direnv/`, `*.api_key` — so a local key file cannot be
staged by accident. Only `.env.example` / `.env.sample` / `.env.*.example`
are un-ignored; extend that allow-list rather than narrowing `.env.*`.

### Batching + concurrency knobs

`TranslateOptions` exposes three tunables that together control how
fast a long document gets translated:

| Field                            | Default | What it does |
|----------------------------------|---------|--------------|
| `target_input_tokens_per_batch`  | 6000    | Soft cap on per-batch input tokens. The packer adds units to a batch until the next one would tip the running total over this number, then starts a new batch. Counted with the model's tiktoken encoding (`o200k_base` for gpt-4o / gpt-5 / o-series, `cl100k_base` otherwise). |
| `max_units_per_batch`            | 32      | Hard cap on units per batch even if they're tiny. Keeps retry granularity reasonable on documents with many short blocks. **The number that governs a default run is 8, not 32** — see below. |
| `max_concurrent_batches`         | 6       | How many batches the pipeline dispatches in parallel. Set to 1 for fully-sequential dispatch. |

The `Default` column is `TranslateOptions::default()`, and for the first
two fields that is not the same thing as what a run packs under. Both
resolve caller-then-profile-then-built-in-default, and "caller" means *a
value that differs from the built-in default* — `TranslateOptions` has no
`Option` in which to say "unset", so a caller explicitly passing `32` is
indistinguishable from a caller who never touched the field, and the
profile wins in both cases (`unit::budget::caller_wins`, contracts.md
§2). The shipped default profile
(`crates/transync-core/profiles/default.toml`) sets
`[batching].max_units_per_batch = 8` and leaves
`target_input_tokens_per_batch` unset. So on a bare `transync translate`
— and on any library run that leaves `TranslateOptions::profile` at
`None` — the packer runs at **6000 input tokens and 8 units**. To get 32
back from the library, ask for it in the profile you pass, or set some
*other* value on `TranslateOptions`. The CLI flags have no such blind
spot: `--max-units-per-batch` and `--target-input-tokens-per-batch`
overlay the resolved profile rather than `TranslateOptions`, precisely so
that an explicit `32` on the command line still beats a profile
(`translate_cmd::args::apply_batching_overrides`).

All three accept `>= 1`. A `0` is not a usable setting for any of them:
it is reported on `tracing::warn` and then resolves as if you had never
set the field — the profile's `[batching]` value, else the built-in
default above. It is **not** floored to 1, so a stray zero can no longer
put every block in its own batch, or turn a run sequential, without
saying so. The profile-side keys of the first two behave the same way
(contracts.md §2), and the profile's `target_output_tokens` follows the
identical rule so no provider request can carry a zero output ceiling.
`max_concurrent_batches` differs only in where the fallback comes from:
it has no profile home, so a zero there resolves straight to the
built-in `6`.

At the CLI the three are `--target-input-tokens-per-batch`,
`--max-units-per-batch` and `--max-concurrent-batches`, and all three
carry a `range(1..)` value parser, so a `0` there is rejected outright —
argument error, exit 1. The warn-then-ignore rule above is the library and
profile path. Two more flags overlay `[batching]` without
a row in this table: `--target-output-tokens`, the per-batch output
ceiling, where `0` is a real setting that disables the ceiling (and the
output-aware packing and at-risk preflight with it) rather than an error;
and `--output-expansion-factor`, the output-to-source token ratio the
packer estimates each response with, which must be finite and positive.
A nonzero output ceiling must also clear the 64-token response envelope
the packer reserves off it — anything at or below that leaves no room for
a single translated unit, so it is warned about and ignored on the same
warn-then-ignore rule, which disables the ceiling exactly as `0` does
(R0003-0034).

The input cap is a budget for the *units*: the compiled system prompt
(glossary included), the assembled instruction envelope and both
language labels ship once per batch and are measured off the target
before any unit is packed. A large profile can therefore eat the whole
budget — and when the reserve meets or exceeds
`target_input_tokens_per_batch`, the remainder is floored at one token,
which packs every unit into a batch of its own and sends every request
over the target. The run says so once, on `tracing::warn`, naming this
knob; it is a warning rather than a refusal, because this cap is soft
and a single over-budget unit deliberately still ships. Either raise the
target or shorten the profile's prompt and glossary.

Effect on a typical long document, measured on `samples/demo-complex.md`
(64 blocks → 63 translation units — the one thematic break carries no
text — and 11 headings): under
the shipped defaults it packs into **12** batches. The token target is
not what shapes that number — no section in it comes anywhere near 6000
tokens. Section partitioning does: 11 headings and no preamble means 11
sections, hence 11 batches at minimum however the budget is set, and the
one nine-unit section splits under the profile's 8-unit cap to make 12.
Raising the cap to 32 merges that split back and gives 11, not the 3–4 a
purely token-driven packer would. What the concurrency cap buys is
unaffected by any of that: with 6-way dispatch the wall-clock on a
`gpt-5-chat-latest` round-trip drops from ~30 s to ~5 s.

### Dual API dispatch

`transync-openai` picks one of two endpoints per request:

| Surface           | Endpoint                                | Used for |
|-------------------|-----------------------------------------|----------|
| Chat Completions  | `POST {base_url}/v1/chat/completions`   | Default; required for `*-chat-*` aliases (e.g. `gpt-5-chat-latest`). |
| Responses         | `POST {base_url}/v1/responses`          | O-series (`o1*`, `o3*`, `o4*`) and non-chat GPT-5 snapshots. |

Routing rule (`client::api_for_model`):

1. Model name contains `chat` → **Chat Completions**.
2. Model starts with `o1` / `o3` / `o4` → **Responses**.
3. Model starts with `gpt-5` and rule 1 didn't match → **Responses**.
4. Anything else → **Chat Completions**.

Override with `TRANSYNC_OPENAI_API=chat` or `TRANSYNC_OPENAI_API=responses`
when the heuristic gets it wrong (fine-tuned model with an unusual
name, third-party proxy that only implements one surface, etc.). The
strict JSON schema and the user-message shape are identical between
both surfaces — only the request envelope differs
(`response_format.json_schema` for chat vs. `text.format` for
responses).

The override is read **once, when `TransyncOpenAI` is constructed**, and the
answer is stored on the adapter; `TransyncOpenAI::api()` reports it. For the
CLI that is invisible (it builds the provider from the environment at
startup), but a long-lived library host that mutates the variable mid-process
no longer risks a run whose cache namespace names one surface while its
requests go to the other — set the variable before constructing, or build a
second adapter to change surface.

A library host does not have to reach for the variable at all. It is
process-global, so it cannot put two live adapters on two different surfaces
— say a proxy that implements only `/v1/chat/completions` alongside a real
OpenAI endpoint for an o-series model — and the only lever it leaves is
`std::env::set_var` between the two constructions, which is `unsafe` under
edition 2024 and racy against every other thread.
`TransyncOpenAI::with_api(Api::ChatCompletions)` pins the surface for **that
instance**, overriding whatever was resolved:

```rust
use transync_openai::{Api, ModelId, TransyncOpenAI};

let via_proxy = TransyncOpenAI::try_new(key.clone(), ModelId::new("o3-mini"), Some(proxy_url))?
    .with_api(Api::ChatCompletions);   // heuristic would have said Responses
let direct = TransyncOpenAI::try_new(key, ModelId::new("o3-mini"), None)?;
```

The surface is one stored field read by both `fingerprint()` and every
request path, so the pin carries the cache namespace with it: those two
adapters cannot share entries. `Api` is exported at the crate root and
implements `FromStr` over the same tokens the environment variable accepts —
`chat` (aliases `chat_completions`, `chatcompletions`) and `responses` — so a
`--api`-style flag in a host application parses a value exactly the way
`TRANSYNC_OPENAI_API` would.

### Smoke scripts

| Script                        | Use case                                                       |
|-------------------------------|----------------------------------------------------------------|
| `./scripts/smoke.sh`          | Build + tests + CLI dry path with the in-process echo provider. No network. **Host prerequisites:** the `wasm32-unknown-unknown` rustup target, `wasm-pack`, and **binaryen ≥ 121** (`brew install binaryen`) — it runs the wasm gate and `build-wasm.sh`, both of which fail loudly rather than skipping. |
| `./scripts/build-wasm.sh`     | Builds the Track C demo module into `web/wasm/` (gitignored) and enforces its size budget (raw ≤ 1,840,000 B, gzip ≤ 760,000 B — the script's own comment carries the derivation). Publication is **staged**: the build, the `wasm-opt` pass and both budget checks all run in a per-run staging directory, and `web/wasm` is replaced as a whole by rename, so a build that fails a gate leaves the previous module in place and two concurrent builds do not collide. Same wasm-pack + binaryen prerequisites; wasm-pack's cached binaryen 117 is **not** a substitute. |
| `./scripts/test-browser.sh`   | Headless Playwright suite: SCN-13 dual-pane sync (`web/tests/scn13.spec.js`), the SCN-16 HTML-run bundle driven through the shipped shell (`web/tests/scn16.spec.js`), `sync.js`'s mount contract driven directly over a bare two-pane rig (`web/tests/engine.spec.js`), and the wasm render+edit demo (`web/tests/wasm.spec.js`). The runner is the authority on coverage — `playwright.config.js` sets `testDir: "./tests"` and the script ends in a bare `pnpm exec playwright test`, so it runs every spec under `web/tests/`. Regenerates the CLI fixture and runs `build-wasm.sh`, so it inherits the same prerequisites plus Chromium. **A bare `pnpm exec playwright test` now refuses a bundle older than its inputs** (ti `ed2e73`): the script ends in that bare command, so running it directly skips the regeneration and would green a bundle that predates your edits. `playwright.config.js` compares the fixture's newest file against `crates/`, `web/js`, `web/vendor`, `web/wasm` and the two shell HTML files — deliberately **not** `web/tests`, so the documented spec-editing inner loop stays green. `TRANSYNC_ALLOW_STALE_FIXTURE=1` downgrades it to a warning. Two limits stated at the check: the input list is hand-maintained and will drift when a build leg is added, and mtime cannot see a `git checkout` that leaves sources older than the fixture. |
| `./scripts/smoke-live.sh`     | Live OpenAI run, then `transync serve` for browsing the demo. Knobs documented at the top of the file. |
| `./scripts/smoke-live-gate.sh`| **Machine-asserted** live round-trip against every provider surface (`chat` \| `responses` \| `anthropic` \| `all`, default `all`; ~3 tiny cheap-model calls). Wraps the double-gated tests in `crates/transync-openai/tests/live_smoke.rs` and `crates/transync-anthropic/tests/live_smoke.rs` — all are `#[ignore]`d *and* require `TRANSYNC_LIVE_SMOKE=1` plus the leg's own key, so they can never run by accident. Each leg demands its key **up front**, so `all` without one of them is refused rather than half-run. Model/endpoint overrides: `TRANSYNC_LIVE_SMOKE_CHAT_MODEL`, `TRANSYNC_LIVE_SMOKE_RESPONSES_MODEL`, `TRANSYNC_LIVE_SMOKE_ANTHROPIC_MODEL`, `TRANSYNC_OPENAI_BASE_URL`, `TRANSYNC_ANTHROPIC_BASE_URL`. |
| `./scripts/test.sh`           | Local one-shot wrapper for `smoke-live.sh` with a project-default system prompt. Edit to taste. |

**Release gate (OI-0030 / DCR-0015; second provider added by DCR-0029).**
Before tagging a release that touches `transync-openai`,
`transync-anthropic`, `llm::prompt`, the output schema, or batching, run
`./scripts/smoke-live-gate.sh` and record the date and the models used in the
CHANGELOG entry for that release. There is no `ANTHROPIC_API_KEY` in the
development environment yet, so the `anthropic` leg has never been executed —
run it as `./scripts/smoke-live-gate.sh anthropic` once a key exists, and
until then read that crate's evidence off its offline end-to-end test, which
drives the whole stack over a loopback socket. The assertions are structural (validation
summary, anchor counts, at least one genuinely translated unit), so a failure
means real drift — transport, auth, schema, or model behavior — not model
nondeterminism. If a default model identifier has been retired, the failure
surfaces at the provider with a clear message; use the env overrides above
rather than reading it as a code regression.

**The gate is one step of a larger ritual.** The full release sequence —
preflight, the standing gates in this table, the live gate above, the
CHANGELOG promotion *and its reference-link stanza*, the single-line
workspace version bump, the record updates, and the annotated tag — lives in
`docs/project/release-checklist.md`. That document is authoritative for the
steps and their order; this section only describes the scripts.

---

## Customizing the system prompt

Two paths, in order of convenience:

1. **Inline string per run** — `--system-prompt 'Translate from {{source_language}} to {{target_language}}, preserve markdown structure.'`
2. **External file** — `--system-prompt-file path/to/prompt.txt`

The text overrides the active profile's `[system].prompt` body before
template substitution. `{{source_language}}` and `{{target_language}}`
are substituted per call. Any *other* unnamespaced `{{placeholder}}` is a
typo that reaches the model literally, and it is reported on stderr
(`WARN transync::profile: unknown template variable …`) whichever way the
body arrived — profile TOML, `--system-prompt`, or `--system-prompt-file`.
The report is made against the body the run actually sends, so an override
that replaces a typo'd profile template reports nothing.

For repeatable per-document configuration (system prompt + glossary +
batching hints), use a Profile TOML.

---

## Custom Profile TOML

Default lives at `crates/transync-core/profiles/default.toml` and is
embedded into the binary. Pass `--profile path/to/yours.toml` to
override.

The shipped default carries **no glossary entries**: one default serves
every language pair, and a `[[glossary]]` entry names a target *form*
with no target *language*, so an entry there would instruct a Korean
rendering on a run into Japanese. The file keeps the `agent` /
`tool use` pair shown below as commented-out examples — uncomment them
in a profile of your own, or write your own entries.

Schema (every section optional except `slug` + `version`):

```toml
slug    = "literary-ko"
version = "1.0.0"

[system]
prompt = """
You are a literary translator.
Translate from {{source_language}} to {{target_language}}.
Preserve markdown structure and footnote markers byte-for-byte.
"""

[constraints]
preserve_code_identifiers = true
preserve_urls             = true
default_table_strategy    = "whole-block"

[batching]
target_output_tokens = 8000
max_units_per_batch  = 8

[[glossary]]
source = "agent"
target = "에이전트"
note   = "AI agent (not insurance agent)"
scope  = "global"

[[glossary]]
source = "tool use"
target = "도구 사용"
```

`slug` is purely informational. `version` is one of `CacheKey`'s profile
axes, but it is not the one doing the invalidating: `profile_prompt_hash`
and `glossary_hash` sit beside it, so editing the prompt or the glossary
already invalidates every entry that depended on it whether or not you
touch `version`. Bump it as release discipline — so a human reading a
report or a bug thread can tell two profile revisions apart — rather than
as cache hygiene. `docs/Profile_Cookbook.md` says the same under "Bump
`version` when you ship a prompt change".

The full schema is in `docs/architecture/contracts.md` §2.

#### Template state vs compiled prompt

`load_profile` (and `default_profile`) hand the profile back as a
**template**: `prompt_body` is your `[system].prompt` verbatim, with
`{{source_language}}` / `{{target_language}}` still in it and neither the
policy section nor the glossary section appended. `profile::render_prompt_body`
compiles it, and the pipeline does that for you at batch-build time — what a
`Translator` sees on `TranslationBatch::profile` is always the compiled form.

One type carries both states, so compiling is **idempotent**: handing a
compiled profile back to `translate` returns the same bytes rather than
stacking a second copy of the policy and glossary sections onto every batch's
system prompt, and the run's cache identity is the same either way.

Compile from the profile `load_profile` returned rather than mutating a
compiled one. A profile edited after compiling is neither state and cannot be
made into one: the sections a compile appended are identified by the fields
that rendered them, so changing `glossary` or `constraints` under a compiled
`prompt_body` leaves the previous compile's sections in the body and the next
compile appends the current ones in addition. (Nothing can recover the old
sections after the fact — finding them by their header text would eat a
template that happens to use the same words.) Two guards stand where they can:

- The **two boundary stages that change a glossary on your profile** — the gate
  that drops entries which cannot mean what they say, and the auto-glossary
  preflight that merges a harvest in — rewind the body to the template it was
  compiled from before they do. Passing a compiled profile is therefore safe
  for both, and the rewind changes nothing on the wire when the glossary comes
  out unchanged.
- **Batch building** counts the section headers in the prompt it has just
  compiled — the prompt every batch will carry — and warns on
  `transync::profile` when one appears more than once, so a doubled prompt is
  reported rather than sent in silence. That is the door every run passes
  through, so the warning reaches you whether you called `translate` or reached
  the batcher directly, and a full run raises it once rather than twice.

#### Glossary handling

`[[glossary]]` entries are loaded into `ProfileMetadata.glossary` and
also rendered as a bullet section appended to the system prompt the
provider sees, of the shape:

```
…your prompt body…

Glossary (when the source term appears, prefer the target form below; omit otherwise):
- "agent" → "에이전트" — AI agent (not insurance agent)
- "tool use" → "도구 사용"
```

The list a run ships is not always the list the profile wrote. The
auto-glossary preflight (OI-0026) spends one extra provider call before
batching to harvest recurring source terminology and merge it into that
same bullet section, so every batch pins the same target renderings —
once: the harvest is cached like any other provider answer, so a repeat
run over the same document, profile, languages and provider replays it
and makes no call (ti `dca5bf`; across processes with `--cache-dir`). It
is off unless something asks for it — `auto_glossary = true` in the
profile, or `--auto-glossary` on the command line, with
`--no-auto-glossary` turning it back off over a profile that enables it.
Static entries win every conflict, so nothing the extractor returns can
displace a rendering the profile author chose.

The structured `Vec<GlossaryEntry>` stays accessible on the returned
`ProfileMetadata` so downstream tools (custom Translator impls,
reviewers, exports) can read the entries programmatically without
re-parsing the prompt body. `scope = "global"` is applied
unconditionally; `scope = "section"` is **honored per section** since
v0.4.0 (DCR-0027; it was rejected with a now-removed
`ProfileError::Unsupported` until section-coherent batching made
per-section filtering possible). A section-scoped entry names the
sections it applies to with `sections = ["…"]`, a list of heading-text
selectors matched — trimmed, NFC-normalized and case-folded, levels
ignored — against
every heading enclosing a section, the one that opens it included. So a
term selected by `"Installation"` also applies inside `### Windows`
beneath `## Installation`. Batches never straddle a section boundary, so
every batch has one answer to "which entries apply here": its section's
effective glossary, which is every global entry plus every applicable
section-scoped one, with an applicable section-scoped entry **beating**
the global rendering of the same term. The selectors are never rendered
— filtering, not annotation — and are not part of the cache identity,
which keys on the prompt bytes the model saw.

Entries are content-checked before they can reach that bullet list:

- `source` and `target` must both carry non-whitespace text. An empty
  `source` would read as "this rule applies to every term", an empty
  `target` as "delete this term"; either one drops the entry with a
  warning naming its index.
- A source term may be claimed once **per place the claims can meet**.
  Terms are compared trimmed, NFC-normalized and case-folded, so
  `agent`, ` Agent ` and `AGENT` are one term — and so are the two
  Unicode encodings of an accented term, the composed one an editor
  types and the decomposed one a macOS-originating file carries. Two `global` entries on one term: the first wins
  and the later one is dropped, with a warning that says whether the
  loser was a byte-identical repeat or a conflicting rendering. Two
  `section` entries on one term: legal while their `sections` selectors
  are disjoint, and a later one sharing a selector loses the same way. A
  `global` and a `section` entry on one term: **both kept** — that pair
  is the override the scope exists for.
- A `sections` list on a `global` entry is cleared with a warning (it
  cannot narrow an entry that says it applies everywhere), and a
  `section` entry left with no usable selector is dropped (it would
  apply nowhere).
Both checks run at three gates: `load_profile` for a profile read from
TOML (warnings land on `ProfileMetadata.load_warnings`), the translate
boundary for a `ProfileMetadata` you built or mutated in code (warnings
go to `tracing`), and `unit::build_batches` for a caller who batches
without crossing that boundary (warnings go to `tracing`). Running the same
check three times reports nothing twice, because every warning it emits
names an entry it removed: the gate that acts is the only gate that can
speak. Dropping an entry changes the compiled prompt, and therefore the
run's cache identity — so every gate past the loader rewinds an
already-compiled `prompt_body` to its template before it drops
anything.

One more field check is *not* a gate, for exactly that reason:

- Control characters (and U+2028 / U+2029) in any field are escaped
  when the bullet is rendered — `\n` / `\r` / `\t`, else `\u{XXXX}` —
  so a term can never end its bullet and open a line of its own inside
  the system prompt. That escape is unconditional and needs no gate.
  The entry survives, and the character is named in an advisory that is
  raised **once**, at `unit::build_batches` — the one door every
  compiled prompt passes through, and the only place the scan sees the
  entries that actually ship (the auto-glossary harvest is merged in
  after the translate boundary). `load_profile` records the same line
  on `ProfileMetadata.load_warnings` without emitting it, the same way
  it treats the unknown-`{{placeholder}}` scan.

---

## Implementing a custom Translator

`transync` is HTTP-free and provider-agnostic. To add a provider, write
a sibling crate that implements the `Translator` trait:

```rust
use async_trait::async_trait;
use transync::CancellationToken;
use transync::llm::{
    Translator, TranslationBatch, TranslationBatchResult,
    TranslatorError, UnitResult, OutputKind,
};

pub struct MyProvider { /* api key, model id, http client, ... */ }

#[async_trait]
impl Translator for MyProvider {
    async fn translate_batch(
        &self,
        batch: TranslationBatch,
        cancel: &CancellationToken,
    ) -> Result<TranslationBatchResult, TranslatorError> {
        // 1. Render `batch.units` into your provider's request shape
        // 2. Send the request (use any HTTP client you like), raced against
        //    the run's cancellation token
        // 3. Parse the response into one UnitResult per requested unit
        // 4. Return TranslationBatchResult with the same batch_id
        let response = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(TranslatorError::Cancelled),
            r = self.send(&batch) => r?,
        };
        Ok(TranslationBatchResult {
            batch_id: batch.batch_id,
            detected_source_language: None,
            units: batch.units.iter().map(|u| UnitResult {
                unit_id: u.unit_id.clone(),
                output_kind: OutputKind::Translated,
                translated_payload: /* the model's translation */ String::new(),
                warnings: vec![],
            }).collect(),
        })
    }
}
```

### The cancellation token

`cancel` is the run's token (DCR-0024, contracts.md §5b). It is live and
shared across every call of the run: **observe it, never cancel it.**

Honoring it is a SHOULD, not a MUST. The pipeline races your future against
the same token and drops the loser, so an implementation that names the
parameter `_cancel` and ignores it is still cancelled — for a `reqwest`-shaped
client, dropping the future is what aborts the in-flight request. Three things
make honoring it worth the four lines above:

- your caller gets a typed `TranslatorError::Cancelled` instead of a future
  that silently disappeared;
- an implementation whose work is *not* drop-cancellable — an inner
  `tokio::spawn`, a `spawn_blocking` client, an internal queue — is otherwise
  not cancelled at all, only detached;
- an adapter driven directly, rather than through `transync::translate`, has no
  pipeline racing on its behalf.

`biased;` is deliberate: it polls the token first, so an already-cancelled run
never issues the request.

The same parameter is on the optional `extract_glossary` preflight, on the same
terms — with one asymmetry worth knowing: an extraction *error* only degrades
the run (static glossary, a report row), but a *cancellation* during it aborts
the run outright.

### Behavior contract (must obey)

- One `UnitResult` per requested `TranslationUnit`. No duplicates, no
  extras, no missing.
- `unit_id` round-trips byte-for-byte (no normalization).
- Set `output_kind = FailedNeedsFallback` for units the provider can't
  handle. Don't fake a translation.
- Map provider errors to the right `TranslatorError` variant
  (`Network`, `Authentication`, `RateLimited`, `MalformedResponse`,
  `Unsupported`, `ContentFiltered`, `OutputCeilingExhausted`,
  `ModelRefused`, `ResponseTooLarge`, `ProviderRejected`, `Cancelled`,
  `Other`). The pipeline only retries on `Network`/`RateLimited`; every
  other variant is terminal on first occurrence. Reach for `Other` when
  nothing fits — a forced ill-fitting name is worse than an honest
  catch-all (contracts.md §1).

### Reading the unit shape

Each `TranslationUnit` carries:

| Field             | What to send to the model |
|-------------------|---------------------------|
| `source_payload`  | The raw block markdown — translate it. |
| `block_kind`      | Wire-form string (heading-1, paragraph, table, code-block, list-item, blockquote, …). Use it to pick the right system instructions per kind. |
| `input_mode`      | `TextFragment`, `FullTableMarkdown`, `TableRowWindow`, `FullCodeBlock`, `ListItemContent`, `BlockquoteContent`, `HtmlSegments`. Tells you the structural shape. All seven — `HtmlSegments` is the one that changes what a payload *is*; see below. |
| `context`         | Surrounding section path + neighbor snippets. Optional but improves quality. Heading snippets are plain prose (the parsed heading's inline text — no `#` markers, no inline syntax); neighbor snippets are verbatim source excerpts of any block kind, paired with that block's `kind`. |
| `constraints`     | Per-kind structural invariants you must preserve (column count, info string, list topology, heading level, …). |
| `unit_id`         | Echo back unchanged. |

**`HtmlSegments` is the one variant where `source_payload` is not markdown.**
For a unit whose `block_kind` is `html`, `source_payload` is a **string
containing a JSON array of strings** — the ordered, entity-decoded text
segments extracted from the raw HTML block — and `translated_payload` MUST be
the same shape with the **same element count, in the same order**. Markup never
reaches you: tags, attributes, comments and `script`/`style` content are not in
the payload and are spliced back by the application, so an implementor cannot
affect markup and must not try. Preserve a segment by echoing it unchanged (an
echoed segment splices byte-identically, entity forms included).
`partially_translated` is not offered for html units; `failed_needs_fallback`
is legal and rides the normal fallback path. Returning a different element
count, or anything that is not a JSON array of strings, is rejected by the
validator. `contracts.md` §1's html-segment-units paragraph is the normative
statement of all of this (ADR-0018 / DCR-0016).

`transync-openai/src/client.rs` is the reference implementation — worth
reading before writing your own. It is flow only: the two API surfaces live
in `client/chat.rs` and `client/responses.rs`, transport in
`client/transport.rs`, and the HTTP-status → error classification your retry
behavior depends on in `client/classify.rs`.

### v0.2 migration notes (custom `Translator` / `Cache` implementors)

v0.2 moved three provider/cache-boundary surfaces (DCR-0009). If you
carry a custom impl across the bump:

- **`Translator::fingerprint()`** now namespaces every cache entry by the
  producing translator, so a shared cache cannot replay one provider's
  output for another's request. The default derives the fingerprint from
  the implementing type name — correct only when every instance of your
  type is interchangeable. If your instances can differ (configurable
  model, endpoint, prompt template), **override `fingerprint()`** to
  cover every output-affecting axis. Over-distinguishing only costs a
  redundant re-translation; under-distinguishing lets a shared cache
  replay foreign output.
- **The `Cache` trait is now fallible and gained `evict`.** `get` /
  `put` / `evict` all return `Result<_, CacheError>` (the pipeline
  degrades on a cache error — miss / not-persisted / stale — and never
  aborts). `evict` (remove exactly one key; an absent key is `Ok(())`)
  is **required** — it is the mechanism behind document-level
  disqualification eviction (contracts §5a).
- **`TranslationUnit` gained `retry: Option<RetryContext>`,** and became
  `#[non_exhaustive]` in the OI-0027 curation. Construct it with
  `TranslationUnit::new(unit_id, block_kind, input_mode, source_payload,
  source_hash)` — which defaults `retry` to `None` for a first dispatch —
  plus `with_context` / `with_constraints` / `with_batch_id` /
  `with_retry` as needed. Struct literals are in-crate only, so field
  additions no longer cost you a migration.
- **The error enums are `#[non_exhaustive]`.** Matches over
  `TranslatorError`, `TransyncError`, and `ParseError` need a wildcard arm;
  variant additions are non-breaking. `TransyncError::stable_code()` and
  `TranslatorError::stable_code()` give you a stable machine string per
  variant if you need to branch across a process boundary — the former
  delegates to the latter for provider failures, so one call answers either
  way (contracts.md §1 holds the nineteen-code vocabulary).
- **`TranslatorError` states a cause, not a policy.** Each variant names why
  the provider stopped — a content-policy stop, an exhausted output ceiling,
  a model refusal, an oversize response, a rejected request — and none of them
  says whether to retry. That is yours to decide; note that of the causes
  above, none survives a verbatim resubmission, which is why the pipeline
  re-dispatches only `Network` and `RateLimited`. A wildcard arm is mandatory
  but is the *wrong* home for those five: they are exactly the failures a
  reader must be told not to retry. Reach for `Other` in your own
  `Translator` impl when nothing fits — it is a permanent catch-all, not a
  deprecated bucket.

### v0.4 migration notes (custom `Translator` implementors)

One signature change, and it is not optional — a defaulted method was
considered and rejected precisely because it could be missed (DCR-0024,
contracts.md §5b).

- **`translate_batch` and `extract_glossary` take `cancel: &CancellationToken`.**
  Add the parameter. If you do not intend to use it, name it `_cancel` and you
  are done: the pipeline races your future against the same token and drops it,
  which aborts an in-flight `reqwest`-style request. If your work is *not*
  drop-cancellable — an inner `tokio::spawn`, a `spawn_blocking` client, an
  internal queue — or if callers drive your adapter directly, race it yourself
  and return `TranslatorError::Cancelled`. See *The cancellation token* above.
- **`TranslatorError::Cancelled` is new** (stable code `provider_cancelled`),
  as is **`TransyncError::Cancelled`** (stable code `cancelled`). Both enums
  are `#[non_exhaustive]`, so your matches keep compiling — but a cancelled run
  is not a failure to report to a user, and a wildcard arm will treat it as
  one.
- **Cancelling a run is opt-in from the caller's side:**
  `opts.cancel = Some(token)`. Leaving it `None` reproduces the pre-0.4
  behavior exactly. If you may cancel, use `translate_with_cache` with a cache
  you own — the units already accepted are in it, and that is the only place a
  cancelled run's paid-for work survives.

### Wiring it into the pipeline

```rust
let translator = MyProvider::new(/* … */);
let output = transync::translate(&source, &opts, &translator).await?;
```

That's it. The core library never knew about `transync-openai` either.

---

## Customizing demo CSS themes

The demo shell at `crates/transync-cli/web/index.html.tpl` ships three
themes runtime-switchable via a `<select>` in the page header:

| Theme    | Look |
|----------|------|
| Default  | System sans-serif, dense layout, neutral. |
| Document | Tables get borders + zebra rows, code/pre gets a bordered grey background, blockquotes get a left rule. |
| Book     | Serif body, narrow content column, justified prose with hyphens, drop-cap on the first H1, off-white background. |

Choice persists in `localStorage["transync.theme"]`.

### Adding your own theme

1. Open `crates/transync-cli/web/index.html.tpl`.
2. Add CSS rules scoped under `body[data-theme="yourname"] .pane …`
   inside the `<style>` block. The layout baseline (grid columns,
   pane scrolling, fallback highlight) lives outside the per-theme
   selectors.
3. Add `<option value="yourname">Your Name</option>` to the
   `<select id="transync-theme">`.
4. `cargo build -p transync-cli` to embed the new template into the
   binary.

If you also want the workspace standalone shell (`web/index.html`) to
have your theme, mirror the same edits there.

### Available CSS hooks per block

Every sync-relevant block carries (non-sync rows such as thematic
breaks omit `data-sync-id`, per DCR-0007 / contracts §4):

```html
<wrapper data-sync-id="…" data-block-kind="…" data-order="…"
         data-fallback="translated|preserved|partially_translated|fallback_source">
  …Comrak's HTML for this block…
</wrapper>
```

`data-parent-id` is **never emitted**. List items and blockquotes are leaf
units today, so every alignment row carries `parent_id: null`; the attribute
stays in the contract (`contracts.md` §4a) as reserved for a future
nested-anchor revision — OI-0005.

Style by `data-block-kind` for kind-specific look:

```css
[data-block-kind="code-block"] { /* … */ }
[data-block-kind="heading-1"]  { /* … */ }
```

Or by fallback status:

```css
[data-fallback="fallback_source"] {
  background: rgba(255, 220, 0, 0.18);
}
```

The full attribute contract is in `docs/architecture/contracts.md` §4.

---

## The alignment map (for custom renderers)

If you skip transync's HTML renderer and write your own, you'll consume
the alignment map JSON directly:

```json
{
  "schema_version": "1.3.0",
  "document_id": "a91f2c0d2e1bbb40",
  "source_language": "en",
  "target_language": "ko",
  "detected_source_language": "en",
  "generator": { "name": "transync", "version": "0.2.0" },
  "blocks": [
    {
      "source_block_id": "h1-0001",
      "target_block_id": "h1-0001",
      "block_kind": "heading-1",
      "source_order": 0,
      "target_order": 0,
      "source_range": { "start": 0,  "end": 18 },
      "target_range": { "start": 0,  "end": 22 },
      "sync_role": "anchor",
      "fallback_status": "translated",
      "parent_id": null
    }
  ],
  "validation_summary": {
    "total_units": 1,
    "translated": 1,
    "preserved": 0,
    "partially_translated": 0,
    "fallback_source": 0,
    "retried_units": 0
  }
}
```

Stability rules: `schema_version` is semver. Patch bumps add fields;
minor bumps may rename optional fields with aliases; major bumps are
breaking. Consumers MUST reject unknown major versions and MAY accept
unknown minor/patch versions with a warning.

The full schema, including `sync_role` semantics and the wire form of
`block_kind`, is in `docs/architecture/contracts.md` §3.

The JS sync engine at `web/js/sync.js` is the reference consumer.

---

## Customizing the JS sync engine

Tunables at the top of `web/js/sync.js`:

| Constant                          | Default | What it controls |
|-----------------------------------|--------:|------------------|
| `PROGRAMMATIC_SCROLL_LOCK_MS`     | 90      | How long after our `pane.scrollTop = …` assignment we ignore that pane's own scroll events. Stops feedback loops. |
| `REFERENCE_OFFSET_PX`             | 4       | "Currently being read" line, measured from the top of the pane. |
| `SMOOTHING_FACTOR`                | 0.2     | Fraction of the gap closed per RAF frame. Lower = softer trail; higher = snappier follow. |
| `SETTLE_THRESHOLD_PX`             | 0.5     | When `|target − current|` drops below this, snap and end the animation loop. |

After editing, mirror the change to `crates/transync-cli/web/sync.js`
(the CLI's embedded copy) and rebuild.

The full algorithm — per-pane locks, RAF-coalesced scroll events,
proportional in-block tracking — is described in
`docs/superpowers/specs/2026-05-03-smooth-scroll-sync.md`.

---

## Common pitfalls

| Symptom                                          | Likely cause                                                                          |
|--------------------------------------------------|---------------------------------------------------------------------------------------|
| `Project does not have access to model gpt-…`    | Set `TRANSYNC_OPENAI_MODEL=gpt-4o-2024-08-06` (or any model your project has access to). |
| `translation failed: malformed response: …`      | The model returned JSON that didn't match the strict schema. Try a Structured-Outputs-capable model. |
| `every translatable unit fell back to source`    | Persistent validation rejection or provider failure. Inspect `validation_report.per_unit` for `rejected_by`. |
| `translator error: content filtered: …` / `model refused: …` (exit 7) | The provider refused this document's content. Retries are verbatim (ADR-0009), so re-running cannot help — skip the document. See Troubleshooting. |
| `parse error: block nesting too deep`            | The source nests blockquotes/list levels past `parser::MAX_BLOCK_NESTING_DEPTH` (128). Deliberate and size-independent — a container costs one byte per level, and a recursive walk that deep aborts the process. See Troubleshooting. |
| Browser demo loads but doesn't sync              | Check the DevTools console — schema-version mismatch or missing `data-sync-id` attributes. |
| `CARGO_TARGET_DIR` errors after a workspace move | Reverify that the configured target dir is still mounted. transync respects whatever `CARGO_TARGET_DIR` you have. |

---

## Where each contract is documented

- **Translator trait + retry policy** — `docs/architecture/contracts.md` §1, §5.
- **Profile TOML schema** — `docs/architecture/contracts.md` §2.
- **AlignmentMap JSON** — `docs/architecture/contracts.md` §3.
- **HTML attribute contract** — `docs/architecture/contracts.md` §4.
- **CLI argument contract + exit codes** — `docs/architecture/contracts.md` §6.
- **`transync-openai` constructor + env vars** — `docs/architecture/contracts.md` §7.
- **All ADRs** — `docs/decisions/`.
- **Active design baseline + authority hierarchy (BL-2026-07-B)** — `docs/project/design-baseline-2026-07.md` (the superseded `BL-2026-05-01-A` record is preserved in `docs/project/design-baseline.md`).
