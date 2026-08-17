# HTML-Content Translation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Text inside block-level raw HTML is translated via app-owned segment extraction; markup is preserved by construction; successful blocks live-render in both panes; the block-ID sync chain extends over them unchanged.

**Architecture:** A new `BlockKind::Html { block_type }` is emitted by the parser; a new `transync-core::htmlseg` module (lol_html-based) extracts decoded text segments at unit-construction time and splices translations back at regen time; validation gains a per-kind JSON-shape check, a post-splice structural check, and an always-on inline raw-HTML tag guard; the renderer live-renders auto-balanced fragments (placeholder only on fallback); the alignment schema bumps 1.1.0 → 1.2.0 with the full lockstep drill.

**Tech Stack:** Rust (comrak 0.27, lol_html 2, htmlize 1), vanilla JS (`web/js/sync.js`), Playwright.

**Authoritative spec:** `docs/superpowers/specs/2026-08-03-html-content-translation-design.md` (v2, approved). Where this plan and the spec disagree, the spec wins — stop and flag it.

> **Note added 2026-08-07 — review-round citation.** The `R0001-0022` this
> plan names (the `align.rs` counting comment) is from the **2026-05-02**
> Review 0001, removed in `bb93b68` (`reviews/reviewed/0001.md`) — "alignment
> summary counts blocks, not translatable units". It is not `reviews/0001.md`'s
> `R0001-0022`, which is about heading context. See `reviews/README.md`.

## Global Constraints

- Temp files ONLY under `/Volumes/Temp/claude/` (never `/tmp` directly, never `/private/tmp`, never the harness scratchpad).
- NEVER set/override `CARGO_TARGET_DIR` and never pass `--target-dir`. If the target volume is unreachable, stop and ask.
- Every test run: `cargo test --workspace -- --test-threads=4` (also for single crates: append `-- --test-threads=4`).
- CLI stub tests are feature-gated and NOT compiled by the workspace run — run both: `cargo test --workspace -- --test-threads=4` AND `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`.
- Lint gate: `cargo fmt --all` then `cargo clippy --all-targets --all-features -- -D warnings` (the pre-commit hook enforces both).
- No pure-formatting edits; let `cargo fmt` own style.
- Korean `*.ko.md` files are out of scope — never read, edit, or cite them (fixtures have `.ko.md` siblings; leave them alone).
- JS package manager is `pnpm`. Browser suite: `scripts/test-browser.sh` (builds a stub bundle under `/Volumes/Temp/claude/transync-browser-fixture` and runs Playwright).
- Commits: small, logically coherent, `git add <files> && git commit -m "..."`; end every commit message with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.
- transync-core has NO `tests/` dir — new core tests go in inline `#[cfg(test)] mod <concern>_tests` modules at the bottom of the file they cover, named by concern, with a leading comment citing the governing spec section. Integration tests go in `crates/transync/tests/`.
- The two `sync.js` copies (`web/js/sync.js` and `crates/transync-cli/web/sync.js`) must stay byte-identical (`sync_js_drift.rs` pins this). Edit one, copy to the other, verify with the drift test.
- `web/index.html` and `crates/transync-cli/web/index.html.tpl` intentionally differ (no drift test); apply equivalent JS changes to both by hand.
- Spec limits that must hold at the end (spec §9): no attribute-text translation; no sub-block anchors; no `<template>` extraction; no fold reproduction; no native-array wire field.

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `Cargo.toml` (workspace) | modify | add `lol_html`, `htmlize` to `[workspace.dependencies]` |
| `crates/transync-core/Cargo.toml` | modify | add the two deps |
| `crates/transync-core/src/htmlseg.rs` | **create** | the whole lol_html engine: pinned `Settings`, `scan` (per-text-node records), `extract`, `splice` (identity-skip + type-conditional blank-line collapse), `balance_fragment`, `tag_inventory` |
| `crates/transync-core/src/id.rs` | modify | `BlockKind::Html { block_type: u8 }` + `id_code`/`wire_str` arms |
| `crates/transync-core/src/parser.rs` | modify | explicit `NodeValue::HtmlBlock` arm; remove the now-dead html arms from `skipped_label`/`node_kind_label`; test updates |
| `crates/transync-core/src/unit.rs` | modify | `HtmlOutcome` + `pub fn html_outcomes`; per-block translatability; html payload/constraints/input-mode; `build_batches`/`has_translatable_blocks` signature updates |
| `crates/transync-core/src/llm.rs` | modify | `HtmlSegmentConstraints`, `BlockConstraints.html`, `InputMode::HtmlSegments` |
| `crates/transync-core/src/llm/prompt.rs` | modify | `input_mode_label` arm, `ConstraintHints` html fields, conditional html instruction, unconditional inline-tag instruction, golden regen helper |
| `crates/transync-core/src/batch.rs` | modify | estimate: html label tokens |
| `crates/transync-core/src/validate.rs` | modify | splice-check step (direct fallback), `VALIDATION_SCHEMA_VERSION` 1→2 |
| `crates/transync-core/src/validate/per_kind.rs` | modify | `check_html` |
| `crates/transync-core/src/validate/fragment_reparse.rs` | modify | Html skip arm |
| `crates/transync-core/src/validate/inline.rs` | modify | always-on raw inline-HTML tag guard (hoisted above policy gates) |
| `crates/transync-core/src/validate/full_reparse.rs` | modify | `html` label arms on BOTH sides |
| `crates/transync-core/src/regen.rs` | modify | Html splice dispatch arm |
| `crates/transync-core/src/align.rs` | modify | schema 1.2.0; per-block translatable predicate via outcomes; `build_alignment_map` signature |
| `crates/transync-core/src/render.rs` | modify | Html live/placeholder arm; wrapper arm |
| `crates/transync-core/src/pipeline.rs` | modify | compute/thread `html_outcomes`; report warnings for zero-segment/extraction-failed blocks |
| `web/js/sync.js` + `crates/transync-cli/web/sync.js` | modify | `KNOWN_SCHEMA` 1.2.0; details-toggle mirroring (byte-identical pair) |
| `web/tests/scn13.spec.js` | modify | test g probe → 1.3.0; new test h (live render, no nested wrappers, toggle mirror) |
| `web/tests/support/harness.js` | modify | extend `SYNC_IDS` |
| `crates/transync/tests/fixtures/scn-14-full.md` | modify | append html specimens (end of file only — ids before stay stable) |
| `crates/transync/tests/fixtures/scn-15-html-blocks.md` | **create** | rich html fixture per spec §7 |
| `crates/transync/tests/scenarios/scn_15_html_blocks.rs` | **create** | end-to-end scenario |
| `crates/transync/tests/scenarios.rs` | modify | register scn_15 |
| `crates/transync/tests/scenarios/scn_14_full.rs` | modify | new block totals |
| `crates/transync/tests/reader_honesty.rs` | modify | html is now live-rendered — rewrite the assertion |
| 13 scenario files + `crates/transync-cli/tests/cli_smoke.rs` | modify | `"1.1.0"` pins → `"1.2.0"` |
| `crates/transync-openai/tests/live_smoke.rs` | modify | `build_batches` call-site update |
| docs (ADR-0018, DCR-0016, CLAUDE.md, contracts.md, mvp-scope, index, status, phase-state, CHANGELOG, open-issues) | modify/create | records per spec §8 |

Signature changes ripple to every caller of `build_batches` and `build_alignment_map`; the affected call sites are enumerated in Tasks 5 and 6.

---

### Task 1: Dependencies + WASM canary + Settings pin

**Files:**
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]`)
- Modify: `crates/transync-core/Cargo.toml` (`[dependencies]`)

**Interfaces:**
- Consumes: nothing.
- Produces: `lol_html` and `htmlize` resolvable from `transync-core`; a recorded wasm32 verdict for the OI-0028 note (Task 16).

- [ ] **Step 1: Add the dependencies**

In the workspace `Cargo.toml`, extend `[workspace.dependencies]` (keep the existing alignment style):

```toml
lol_html    = "2"
htmlize     = { version = "1", features = ["unescape"] }
```

In `crates/transync-core/Cargo.toml` `[dependencies]`, after `tiktoken-rs`:

```toml
# HTML-content translation (spec 2026-08-03): segment extraction/splice
# rides lol_html's streaming rewriter; htmlize supplies the full
# named-entity decode table (lol_html has no decoder).
lol_html    = { workspace = true }
htmlize     = { workspace = true }
```

- [ ] **Step 2: Verify the workspace still builds**

Run: `cargo check --workspace`
Expected: clean. If `lol_html = "2"` fails to resolve, check `cargo search lol_html` and pin the newest 1.x/2.x that resolves; record the chosen version in the commit message.

- [ ] **Step 3: WASM canary (spec §10 first task)**

```bash
rustup target list --installed | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
mkdir -p /Volumes/Temp/claude/lolhtml-wasm-probe
cd /Volumes/Temp/claude/lolhtml-wasm-probe
cargo init --name lolhtml_wasm_probe 2>/dev/null || true
```

Set the probe's `Cargo.toml` dependencies to exactly the versions the workspace resolved (read them from `/Volumes/Common/QJoon/transync/Cargo.lock`):

```toml
[dependencies]
lol_html = "<resolved version>"
htmlize  = { version = "<resolved version>", features = ["unescape"] }
```

and `src/main.rs`:

```rust
fn main() {
    let _ = lol_html::Settings::new();
    let _ = htmlize::unescape("&amp;");
}
```

Run: `cargo check --target wasm32-unknown-unknown` (from the probe dir; do NOT touch CARGO_TARGET_DIR — the shared target dir is fine).
Expected: PASS. **Record the verdict** (pass/fail + versions) — Task 16 writes the OI-0028 note ONLY if this passed. If it fails, the feature still proceeds (nothing else depends on wasm); flag the failure to the owner in the task report and skip the OI-0028 note.

- [ ] **Step 4: Commit**

```bash
cd /Volumes/Common/QJoon/transync
git add Cargo.toml Cargo.lock crates/transync-core/Cargo.toml
git commit -m "deps: lol_html + htmlize for HTML-content translation

wasm32-unknown-unknown canary: <PASS/FAIL> at lol_html <ver>, htmlize <ver>.

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `htmlseg` — scan + extract

**Files:**
- Create: `crates/transync-core/src/htmlseg.rs`
- Modify: `crates/transync-core/src/lib.rs` (add `pub(crate) mod htmlseg;` next to the other module decls — find the `mod` block and match its ordering style; the module stays crate-private)

**Interfaces:**
- Consumes: `lol_html`, `htmlize`.
- Produces (all `pub(crate)` unless noted):
  - `struct NodeRecord { decoded: String, label: String, kept: bool }`
  - `struct HtmlSegments { texts: Vec<String>, labels: Vec<String> }`
  - `fn scan(block: &str) -> Result<Vec<NodeRecord>, String>` — one record per text node, in document order (dropped nodes included with `kept: false`)
  - `fn extract(block: &str) -> Result<HtmlSegments, String>` — the kept records only
  - `fn scan_with_memory_cap(block: &str, max_bytes: usize) -> Result<Vec<NodeRecord>, String>` — test hook for the error path

**Design notes the implementation must honor (spec §3.2, ordered algorithm):**
1. ONE rewriter configuration builder (`fn rewriter_settings(...)`) used by every pass — the extract pass and the splice pass (Task 3) must never diverge on Settings.
2. Document-level text handler (`doc_text!`) with `TextType` filtering: collect `TextType::Data` and `TextType::RCData` (covers `<textarea>`); never `ScriptData`/`RawText` (script/style) / CDATA. Comments never reach text handlers.
3. Coalesce chunks per text node via `last_in_text_node()` — decode entities only AFTER coalescing (entities split across chunk boundaries).
4. Decode with `htmlize::unescape` (full named-entity table).
5. `kept = !decoded.chars().all(char::is_whitespace)` — the drop decision is made on the DECODED form (an `&nbsp;`-only node decodes to whitespace and is dropped).
6. Parent label = top of an open-element stack maintained by an `element!("*")` handler (+ end-tag pop); text outside any element gets label `"fragment"`. Do not push void elements (`area base br col embed hr img input link meta param source track wbr`) or self-closing tags onto the stack; pop by name (scan the stack top-down for the name) so mismatched markup cannot corrupt later labels.

- [ ] **Step 1: Write the failing tests**

At the bottom of the new `htmlseg.rs`:

```rust
// Spec 2026-08-03 §3.2: ordered extraction algorithm — coalesce per text
// node, decode after coalescing, drop whitespace-only on the decoded form.
#[cfg(test)]
mod extract_tests {
    use super::*;

    #[test]
    fn segments_are_collected_in_document_order_with_parent_labels() {
        let block = "<details><summary>Click me</summary><p>Body &amp; soul</p></details>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["Click me".to_string(), "Body & soul".to_string()]);
        assert_eq!(segs.labels, vec!["summary".to_string(), "p".to_string()]);
    }

    #[test]
    fn script_style_and_comment_content_is_never_collected() {
        let block = "<div><script>var x = 'no';</script><style>.a{}</style><!-- hidden -->visible</div>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["visible".to_string()]);
        assert_eq!(segs.labels, vec!["div".to_string()]);
    }

    #[test]
    fn nbsp_only_text_node_is_dropped_on_the_decoded_form() {
        let block = "<p>&nbsp;</p><p>real</p>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["real".to_string()]);
    }

    #[test]
    fn entities_are_decoded_after_coalescing() {
        // A large text node forces lol_html to deliver multiple chunks in
        // some configurations; correctness must not depend on chunking, so
        // scan() must coalesce before decoding either way.
        let block = "<p>a &lt;tag&gt; and &copy; sign</p>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["a <tag> and © sign".to_string()]);
    }

    #[test]
    fn top_level_text_outside_any_element_is_captured_as_fragment() {
        let block = "</div>\norphan tail prose";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["\norphan tail prose".to_string()]);
        assert_eq!(segs.labels, vec!["fragment".to_string()]);
    }

    #[test]
    fn pre_content_is_extracted_with_pre_label() {
        let block = "<pre>line one\n\nline two</pre>";
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts, vec!["line one\n\nline two".to_string()]);
        assert_eq!(segs.labels, vec!["pre".to_string()]);
    }

    #[test]
    fn zero_segment_block_yields_empty_extraction() {
        let block = "<img src=\"a.png\"><hr><!-- badges -->";
        let segs = extract(block).expect("extracts");
        assert!(segs.texts.is_empty());
    }

    #[test]
    fn scan_reports_dropped_nodes_and_agrees_with_extract() {
        let block = "<p>&nbsp;</p><p>kept</p>";
        let records = scan(block).expect("scans");
        assert_eq!(records.len(), 2);
        assert!(!records[0].kept);
        assert!(records[1].kept);
        let segs = extract(block).expect("extracts");
        assert_eq!(segs.texts.len(), records.iter().filter(|r| r.kept).count());
    }

    #[test]
    fn rewriter_error_surfaces_as_err() {
        // The memory cap is the one deterministic way to make the rewriter
        // fail; the production degrade path (spec §3.2) rides this Err.
        let big = format!("<p>{}</p>", "x".repeat(64 * 1024));
        assert!(scan_with_memory_cap(&big, 16).is_err());
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p transync-core htmlseg -- --test-threads=4`
Expected: FAIL — module does not exist / functions unresolved.

- [ ] **Step 3: Implement**

Skeleton (adapt handler-registration syntax to the pinned lol_html version — check with `cargo doc -p lol_html --no-deps` if the macros differ; the load-bearing semantics are fixed by the spec, the API spelling is not):

```rust
//! HTML segment extraction / splice engine (lol_html).
//!
//! Spec: docs/superpowers/specs/2026-08-03-html-content-translation-design.md
//! §3.2–§3.4. One pinned Settings source (`rewriter_settings`) feeds every
//! pass so the extract and splice passes can never disagree on coalescing
//! or drop decisions.

use lol_html::html_content::{ContentType, TextType};
use lol_html::{HtmlRewriter, MemorySettings, Settings, doc_text, element};
use std::cell::RefCell;
use std::rc::Rc;

const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

#[derive(Debug, Clone)]
pub(crate) struct NodeRecord {
    pub decoded: String,
    pub label: String,
    pub kept: bool,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct HtmlSegments {
    pub texts: Vec<String>,
    pub labels: Vec<String>,
}

fn is_void(tag: &str) -> bool {
    VOID_ELEMENTS.contains(&tag)
}

fn wanted_text_type(t: TextType) -> bool {
    matches!(t, TextType::Data | TextType::RCData)
}

pub(crate) fn scan(block: &str) -> Result<Vec<NodeRecord>, String> {
    scan_with_memory_cap(block, DEFAULT_MAX_MEMORY_BYTES)
}

/// Pinned memory ceiling for the rewriter (spec §3.2 "pinned Settings").
/// Generous for real README blocks; the test hook lowers it to force the
/// error path deterministically.
const DEFAULT_MAX_MEMORY_BYTES: usize = 4 * 1024 * 1024;

pub(crate) fn scan_with_memory_cap(block: &str, max_bytes: usize) -> Result<Vec<NodeRecord>, String> {
    let records: Rc<RefCell<Vec<NodeRecord>>> = Rc::new(RefCell::new(Vec::new()));
    let stack: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let pending: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));

    let records_t = records.clone();
    let stack_t = stack.clone();
    let pending_t = pending.clone();
    let stack_e = stack.clone();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("*", move |el| {
                let name = el.tag_name().to_ascii_lowercase();
                // spec §3.2 step 6: never push void/self-closing tags —
                // they get no end tag, and would poison later labels.
                if !is_void(&name) && !el.is_self_closing() {
                    stack_e.borrow_mut().push(name.clone());
                    let stack_end = stack_e.clone();
                    el.end_tag_handlers().push(Box::new(move |_end| {
                        // Pop by name so mismatched markup cannot corrupt
                        // labels of later text nodes.
                        let mut s = stack_end.borrow_mut();
                        if let Some(pos) = s.iter().rposition(|t| *t == name) {
                            s.truncate(pos);
                        }
                        Ok(())
                    }));
                }
                Ok(())
            })],
            document_content_handlers: vec![doc_text!(move |t| {
                if wanted_text_type(t.text_type()) {
                    pending_t.borrow_mut().push_str(t.as_str());
                    if t.last_in_text_node() {
                        let raw = std::mem::take(&mut *pending_t.borrow_mut());
                        let decoded = htmlize::unescape(&raw).into_owned();
                        let kept = !decoded.chars().all(char::is_whitespace);
                        let label = stack_t
                            .borrow()
                            .last()
                            .cloned()
                            .unwrap_or_else(|| "fragment".to_string());
                        records_t.borrow_mut().push(NodeRecord { decoded, label, kept });
                    }
                }
                Ok(())
            })],
            memory_settings: MemorySettings {
                max_allowed_memory_usage: max_bytes,
                ..MemorySettings::default()
            },
            ..Settings::new()
        },
        |_chunk: &[u8]| {},
    );

    rewriter
        .write(block.as_bytes())
        .and_then(|_| rewriter.end())
        .map_err(|e| format!("html rewriter failed: {e}"))?;

    Ok(Rc::try_unwrap(records)
        .map(RefCell::into_inner)
        .unwrap_or_else(|rc| rc.borrow().clone()))
}

pub(crate) fn extract(block: &str) -> Result<HtmlSegments, String> {
    let records = scan(block)?;
    let mut out = HtmlSegments::default();
    for r in records.into_iter().filter(|r| r.kept) {
        out.texts.push(r.decoded);
        out.labels.push(r.label);
    }
    Ok(out)
}
```

Implementation caveats:
- If the pinned lol_html version exposes `on_end_tag`/a different end-tag hook name, use it — the pop-by-name semantics stay.
- If `rewriter.end()` consumes `self` in the pinned version, restructure to `let r = rewriter.write(...); drop-based end` per its docs; keep the single `String` error surface.
- The borrow in the `last_in_text_node` branch must not overlap the earlier `push_str` borrow — the `std::mem::take` line above is written to avoid that; keep it.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p transync-core htmlseg -- --test-threads=4`
Expected: all `extract_tests` PASS. If `top_level_text_outside_any_element_is_captured_as_fragment` fails on the exact leading-`\n` expectation, inspect what lol_html actually delivers for that input and adjust the *expected string* (not the filter) — the load-bearing assertion is that orphan-tail prose is captured with label `"fragment"`, not the exact whitespace.

- [ ] **Step 5: Commit**

```bash
git add crates/transync-core/src/htmlseg.rs crates/transync-core/src/lib.rs
git commit -m "core: htmlseg scan/extract — ordered segment extraction (spec §3.2)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: `htmlseg` — splice, balance, tag inventory

**Files:**
- Modify: `crates/transync-core/src/htmlseg.rs`

**Interfaces:**
- Consumes: `scan` (Task 2).
- Produces (`pub(crate)`):
  - `fn splice(block: &str, translated: &[String], block_type: u8) -> Result<String, String>` — positional per-text-node replacement; identity-skip; blank-line collapse only for `block_type` 6/7
  - `fn balance_fragment(html: &str) -> String` — render-path auto-balancing (spec §3.4): close unclosed tags at fragment end, DROP orphan close tags
  - `fn tag_inventory(html: &str) -> Vec<String>` — lowercase tag-name sequence (open and close), for the layer-3 belt-and-braces check
  - `fn collapse_blank_lines(text: &str) -> String` — interior whitespace-only lines removed (CommonMark blank-line definition)

- [ ] **Step 1: Write the failing tests**

Append to `htmlseg.rs`:

```rust
// Spec §3.3: identity-skip splice, entity-drift acceptance, type-conditional
// blank-line collapse. Spec §3.4: render-side auto-balancing.
#[cfg(test)]
mod splice_tests {
    use super::*;

    #[test]
    fn identity_segments_splice_byte_identical_including_entities() {
        // &nbsp; and &copy; would NOT survive a naive decode/re-escape
        // round-trip; the identity skip preserves them byte-exactly.
        let block = "<p>Caf&eacute; &copy; 2026&nbsp;&mdash; <b>bold &amp; true</b></p>";
        let segs = extract(block).expect("extracts");
        let out = splice(block, &segs.texts, 6).expect("splices");
        assert_eq!(out, block, "identity splice must be byte-identical");
    }

    #[test]
    fn translated_segment_is_replaced_and_specials_are_escaped() {
        let block = "<summary>Click to expand</summary>";
        let out = splice(block, &["펼치기 <&>".to_string()], 6).expect("splices");
        assert_eq!(out, "<summary>펼치기 &lt;&amp;&gt;</summary>");
    }

    #[test]
    fn entity_drift_for_translated_segments_is_pinned() {
        // Spec §6 entities row: genuinely translated segments re-escape only
        // < > & — a translated segment loses named-entity forms.
        let block = "<p>one&nbsp;two</p>";
        let out = splice(block, &["eins\u{a0}zwei".to_string()], 6).expect("splices");
        assert_eq!(out, "<p>eins\u{a0}zwei</p>");
    }

    #[test]
    fn blank_lines_collapse_for_type_6_but_not_type_1() {
        let block = "<div>text</div>";
        let translated = vec!["line1\n\nline2".to_string()];
        let out6 = splice(block, &translated, 6).expect("splices");
        assert!(!out6.contains("\n\n"), "type 6 must collapse blank lines: {out6}");

        let pre = "<pre>a\n\nb</pre>";
        let pre_segs = extract(pre).expect("extracts");
        let out1 = splice(pre, &pre_segs.texts, 1).expect("splices");
        assert_eq!(out1, pre, "type 1 keeps interior blank lines");
    }

    #[test]
    fn whitespace_only_line_counts_as_blank_for_collapse() {
        let block = "<div>text</div>";
        let out = splice(block, &["a\n \nb".to_string()], 6).expect("splices");
        assert!(!out.contains("\n \n"), "whitespace-only line must collapse: {out:?}");
    }

    #[test]
    fn wrong_segment_count_is_an_error() {
        let block = "<p>one</p><p>two</p>";
        let err = splice(block, &["only-one".to_string()], 6).unwrap_err();
        assert!(err.contains("segment count"), "got: {err}");
    }

    #[test]
    fn dropped_nodes_agree_between_extract_and_splice_passes() {
        // The &nbsp;-only node is dropped by BOTH passes (same scan()), so
        // one translated segment maps to the kept node — never off-by-one.
        let block = "<p>&nbsp;</p><p>kept</p>";
        let out = splice(block, &["유지".to_string()], 6).expect("splices");
        assert_eq!(out, "<p>&nbsp;</p><p>유지</p>");
    }
}

// Spec §3.4: auto-balancing exists only on the render path.
#[cfg(test)]
mod balance_tests {
    use super::*;

    #[test]
    fn unclosed_div_is_closed_at_fragment_end() {
        assert_eq!(
            balance_fragment("<div align=\"center\">\n<b>Hero</b>"),
            "<div align=\"center\">\n<b>Hero</b></div>"
        );
    }

    #[test]
    fn orphan_close_tag_is_dropped() {
        assert_eq!(balance_fragment("</details>"), "");
        assert_eq!(balance_fragment("tail</div>text"), "tailtext");
    }

    #[test]
    fn balanced_fragment_is_untouched() {
        let html = "<details><summary>ok</summary></details>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn void_elements_do_not_accumulate_open_tags() {
        let html = "<p>a<br>b<img src=\"x.png\"></p>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn comments_and_script_content_are_not_scanned_for_tags() {
        let html = "<!-- <div> --><script>if (a < b) { s = \"</div>\"; }</script>";
        assert_eq!(balance_fragment(html), html);
    }

    #[test]
    fn nested_unclosed_tags_close_in_reverse_order() {
        assert_eq!(
            balance_fragment("<div><span>x"),
            "<div><span>x</span></div>"
        );
    }
}

#[cfg(test)]
mod inventory_tests {
    use super::*;

    #[test]
    fn tag_inventory_lists_open_and_close_tags_in_order() {
        assert_eq!(
            tag_inventory("<div><b>x</b></div>"),
            vec!["div", "b", "/b", "/div"]
        );
    }

    #[test]
    fn identity_splice_preserves_tag_inventory() {
        let block = "<details><summary>s</summary><p>p</p></details>";
        let segs = extract(block).expect("extracts");
        let out = splice(block, &segs.texts, 6).expect("splices");
        assert_eq!(tag_inventory(&out), tag_inventory(block));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p transync-core htmlseg -- --test-threads=4`
Expected: FAIL — `splice`, `balance_fragment`, `tag_inventory` unresolved.

- [ ] **Step 3: Implement `collapse_blank_lines` + `splice`**

```rust
/// Remove interior blank lines (CommonMark definition: a line containing
/// only spaces/tabs). Only splice() for block types 6/7 calls this —
/// type-1 blocks (<pre>, <textarea>) keep their blank lines (spec §3.3).
pub(crate) fn collapse_blank_lines(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let n = lines.len();
    let kept: Vec<&str> = lines
        .iter()
        .enumerate()
        .filter(|(i, l)| {
            let blank = l.chars().all(|c| c == ' ' || c == '\t');
            !(blank && *i > 0 && *i + 1 < n)
        })
        .map(|(_, l)| *l)
        .collect();
    kept.join("\n")
}

/// Splice translated segments back into the block positionally
/// (spec §3.3). Phase A re-runs scan() for the node-level plan (kept/
/// dropped + decoded source text); phase B streams the block through the
/// same rewriter Settings, replacing kept nodes whose translation differs
/// from the decoded source (identity-skip) and passing everything else
/// through byte-verbatim.
pub(crate) fn splice(block: &str, translated: &[String], block_type: u8) -> Result<String, String> {
    let plan = scan(block)?;
    let kept_count = plan.iter().filter(|r| r.kept).count();
    if kept_count != translated.len() {
        return Err(format!(
            "html segment count mismatch at splice: block has {kept_count} kept segments, got {} translations",
            translated.len()
        ));
    }

    // Node-level actions, in text-node order: None = leave untouched
    // (dropped node OR identity translation); Some(text) = replace.
    let collapse = matches!(block_type, 6 | 7);
    let mut ti = 0usize;
    let actions: Vec<Option<String>> = plan
        .iter()
        .map(|r| {
            if !r.kept {
                return None;
            }
            let t = &translated[ti];
            ti += 1;
            if *t == r.decoded {
                None // identity-skip: source bytes (entities included) pass through
            } else if collapse {
                Some(collapse_blank_lines(t))
            } else {
                Some(t.clone())
            }
        })
        .collect();

    let output: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::with_capacity(block.len())));
    let node_index: Rc<RefCell<usize>> = Rc::new(RefCell::new(0));
    let replaced_current: Rc<RefCell<bool>> = Rc::new(RefCell::new(false));
    let actions = Rc::new(actions);

    let out_sink = output.clone();
    let node_index_t = node_index.clone();
    let replaced_t = replaced_current.clone();
    let actions_t = actions.clone();

    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![],
            document_content_handlers: vec![doc_text!(move |t| {
                if wanted_text_type(t.text_type()) {
                    let idx = *node_index_t.borrow();
                    if let Some(Some(replacement)) = actions_t.get(idx) {
                        if !*replaced_t.borrow() {
                            // First chunk of a replaced node carries the
                            // whole translation; ContentType::Text escapes
                            // < > & (spec §3.3).
                            t.replace(replacement, ContentType::Text);
                            *replaced_t.borrow_mut() = true;
                        } else {
                            t.remove();
                        }
                    }
                    if t.last_in_text_node() {
                        *node_index_t.borrow_mut() += 1;
                        *replaced_t.borrow_mut() = false;
                    }
                }
                Ok(())
            })],
            memory_settings: MemorySettings {
                max_allowed_memory_usage: DEFAULT_MAX_MEMORY_BYTES,
                ..MemorySettings::default()
            },
            ..Settings::new()
        },
        move |chunk: &[u8]| out_sink.borrow_mut().extend_from_slice(chunk),
    );

    rewriter
        .write(block.as_bytes())
        .and_then(|_| rewriter.end())
        .map_err(|e| format!("html splice rewriter failed: {e}"))?;

    let bytes = Rc::try_unwrap(output)
        .map(RefCell::into_inner)
        .unwrap_or_else(|rc| rc.borrow().clone());
    String::from_utf8(bytes).map_err(|e| format!("spliced html is not utf-8: {e}"))
}
```

Caveat: the phase-B counter counts text nodes the same way phase A does (same Settings, same `wanted_text_type` filter, `last_in_text_node` boundary) — that shared-routine property is exactly what `dropped_nodes_agree_between_extract_and_splice_passes` pins. Never add a filter to one pass without the other.

- [ ] **Step 4: Implement the tag scanner (`balance_fragment` + `tag_inventory`)**

Hand-rolled tokenizer — lol_html cannot drop unmatched close tags. Both fns share one tokenizer:

```rust
#[derive(Debug, Clone)]
enum TagToken {
    Open { name: String, span: (usize, usize), self_closing: bool },
    Close { name: String, span: (usize, usize) },
}

/// Minimal tag tokenizer for balancing: understands comments, script/style
/// raw-text state, and quoted attribute values. NOT a general HTML parser —
/// only the render-path balancer (spec §3.4) and the layer-3 inventory
/// check use it.
fn scan_tags(html: &str) -> Vec<TagToken> {
    let bytes = html.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0usize;
    let mut raw_until: Option<String> = None; // inside <script>/<style>

    while i < bytes.len() {
        if bytes[i] != b'<' {
            i += 1;
            continue;
        }
        if let Some(close_name) = &raw_until {
            // Only the matching close tag ends raw-text state.
            let rest = &html[i..];
            let want = format!("</{close_name}");
            if rest.len() >= want.len() && rest[..want.len()].eq_ignore_ascii_case(&want) {
                raw_until = None; // fall through: tokenize the close tag
            } else {
                i += 1;
                continue;
            }
        }
        if html[i..].starts_with("<!--") {
            i = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(html.len());
            continue;
        }
        let start = i;
        let mut j = i + 1;
        let closing = j < bytes.len() && bytes[j] == b'/';
        if closing {
            j += 1;
        }
        let name_start = j;
        while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
            j += 1;
        }
        if j == name_start {
            i += 1; // "<" not followed by a tag name — plain text
            continue;
        }
        let name = html[name_start..j].to_ascii_lowercase();
        // Scan to the closing '>' honoring quoted attribute values.
        let mut quote: Option<u8> = None;
        while j < bytes.len() {
            match (quote, bytes[j]) {
                (None, b'"') | (None, b'\'') => quote = Some(bytes[j]),
                (Some(q), c) if c == q => quote = None,
                (None, b'>') => break,
                _ => {}
            }
            j += 1;
        }
        if j >= bytes.len() {
            break; // unterminated tag: leave as-is
        }
        let self_closing = j > start && bytes[j - 1] == b'/';
        let span = (start, j + 1);
        if closing {
            tokens.push(TagToken::Close { name, span });
        } else {
            if matches!(name.as_str(), "script" | "style") {
                raw_until = Some(name.clone());
            }
            tokens.push(TagToken::Open { name, span, self_closing });
        }
        i = j + 1;
    }
    tokens
}

pub(crate) fn tag_inventory(html: &str) -> Vec<String> {
    scan_tags(html)
        .into_iter()
        .map(|t| match t {
            TagToken::Open { name, .. } => name,
            TagToken::Close { name, .. } => format!("/{name}"),
        })
        .collect()
}

pub(crate) fn balance_fragment(html: &str) -> String {
    let tokens = scan_tags(html);
    let mut open_stack: Vec<String> = Vec::new();
    let mut drop_spans: Vec<(usize, usize)> = Vec::new();

    for tok in &tokens {
        match tok {
            TagToken::Open { name, self_closing, .. } => {
                if !is_void(name) && !self_closing {
                    open_stack.push(name.clone());
                }
            }
            TagToken::Close { name, span } => {
                if let Some(pos) = open_stack.iter().rposition(|t| t == name) {
                    open_stack.truncate(pos);
                } else {
                    // Orphan close tag: dropping it is what keeps the sync
                    // wrapper's own </div> safe (spec §3.4).
                    drop_spans.push(*span);
                }
            }
        }
    }

    let mut out = String::with_capacity(html.len());
    let mut cursor = 0usize;
    for (s, e) in drop_spans {
        out.push_str(&html[cursor..s]);
        cursor = e;
    }
    out.push_str(&html[cursor..]);
    for name in open_stack.iter().rev() {
        out.push_str(&format!("</{name}>"));
    }
    out
}
```

- [ ] **Step 5: Run all htmlseg tests**

Run: `cargo test -p transync-core htmlseg -- --test-threads=4`
Expected: all PASS. If `identity_segments_splice_byte_identical_including_entities` fails, the two passes disagree — fix the shared routine, never the test.

- [ ] **Step 6: Lint + commit**

Run: `cargo fmt --all && cargo clippy -p transync-core --all-targets -- -D warnings`

```bash
git add crates/transync-core/src/htmlseg.rs
git commit -m "core: htmlseg splice/balance/inventory — identity-skip + render balancing (spec §3.3–§3.4)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: `BlockKind::Html` through the type system (temporarily non-translatable)

This task adds the variant and patches EVERY exhaustive match so the workspace compiles and every html block parses, anchors, and **live-renders as preserved content** — but is not yet batched (that flips in Task 5). Intermediate behavior after this task: html blocks = today's zero-segment end state.

**Files:**
- Modify: `crates/transync-core/src/id.rs` (variant + `id_code` + `wire_str`)
- Modify: `crates/transync-core/src/parser.rs` (explicit arm; dead-label cleanup; tests)
- Modify: `crates/transync-core/src/unit.rs` (`is_translatable` temporarily excludes Html; test-table row comment)
- Modify: `crates/transync-core/src/validate/per_kind.rs` (arm — permissive until Task 8)
- Modify: `crates/transync-core/src/validate/full_reparse.rs` (`html` label arms on BOTH sides — the v2 spec §4.2(4) correction)
- Modify: `crates/transync-core/src/render.rs` (wrapper arm + live/placeholder arm)
- Modify: `crates/transync-core/src/align.rs` (temporary: Html in the never-batched set with `Preserved`)
- Modify: `crates/transync/tests/reader_honesty.rs` (html is live now — rewrite)

**Interfaces:**
- Consumes: `htmlseg::balance_fragment` (Task 3).
- Produces: `BlockKind::Html { block_type: u8 }` with `id_code() == "html"`, `wire_str() == "html"`; render contract: live `<div{attrs}>…balanced…</div>` vs fallback `<pre{attrs} data-skipped="html-block">…escaped…</pre>`.

- [ ] **Step 1: Write the failing parser/id tests**

Replace the body of `parser.rs`'s `skipped_node_tests::top_level_raw_html_block_is_recorded_as_a_warning` with a new test in a new module (delete the old test — its premise is superseded; keep the module for the other two tests, whose specimens change too, see Step 4):

```rust
// Spec 2026-08-03 §3.1: raw HTML blocks are a first-class translatable
// kind with the CommonMark block type recorded for splice normalization.
#[cfg(test)]
mod html_block_tests {
    use super::*;

    #[test]
    fn top_level_html_block_becomes_block_kind_html_with_type() {
        let doc = parse("<div class=\"note\">side note</div>\n\nreal paragraph\n").expect("parses");
        let html = &doc.blocks[0];
        assert!(
            matches!(&html.kind, BlockKind::Html { block_type } if *block_type == 6),
            "expected Html type 6, got {:?}",
            html.kind
        );
        assert_eq!(html.block_id.0, "html-0001", "html id prefix");
        assert_eq!(doc.blocks[1].block_id.0, "p-0002");
        assert!(
            doc.warnings.is_empty(),
            "html blocks are translatable — no skip warning: {:?}",
            doc.warnings
        );
    }

    #[test]
    fn pre_block_records_type_1() {
        let doc = parse("<pre>\nascii art\n</pre>\n").expect("parses");
        assert!(
            matches!(&doc.blocks[0].kind, BlockKind::Html { block_type } if *block_type == 1),
            "got {:?}",
            doc.blocks[0].kind
        );
    }

    #[test]
    fn html_block_ids_round_trip_assign_block_ids() {
        let mut doc = parse("<div>x</div>\n\npara\n").expect("parses");
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "html-0001");
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "html-0001", "idempotent");
    }

    #[test]
    fn interleaved_details_region_parses_as_multiple_fragments() {
        // Spec §1 "fragment reality": blank lines split type-6 regions.
        let src = "<details>\n<summary>More</summary>\n\nBody paragraph.\n\n</details>\n";
        let doc = parse(src).expect("parses");
        let kinds: Vec<&str> = doc.blocks.iter().map(|b| b.kind.wire_str()).collect();
        assert_eq!(kinds, vec!["html", "paragraph", "html"]);
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p transync-core html_block -- --test-threads=4`
Expected: FAIL — no `Html` variant.

- [ ] **Step 3: Add the variant + all exhaustive-match arms**

`id.rs` — variant (after `Image`, before `Skipped`, with doc comment):

```rust
    /// Block-level raw HTML (spec 2026-08-03). Translatable via segment
    /// extraction; `block_type` is the CommonMark HTML block type (1–7)
    /// and drives type-conditional splice normalization.
    Html {
        block_type: u8,
    },
```

`id_code()`: `BlockKind::Html { .. } => "html",` (comment: no collision with h1..h6, p, t, c, li, q, hr, img, x). `wire_str()`: `BlockKind::Html { .. } => "html",`.

`parser.rs` — new arm in `visit`'s `match value`, placed just above the catch-all `_` arm:

```rust
        NodeValue::HtmlBlock(h) => {
            // Spec 2026-08-03 §3.1: block-level raw HTML is translatable.
            // Segment extraction happens at unit construction, not here.
            let sp = current_section_path(section_stack);
            emit(
                BlockKind::Html { block_type: h.block_type },
                counter,
                sp,
                blocks,
            );
        }
```

Remove the now-unreachable `NodeValue::HtmlBlock(_)` arms from `skipped_label` and `node_kind_label` (the catch-all no longer receives html blocks).

`per_kind.rs` — extend the never-rejects arm for now (Task 8 gives Html its real check):

```rust
        // Html payload validation lands with the segment pipeline (Task 8);
        // until then every html result is accepted here.
        BlockKind::Html { .. } => Ok(()),
```

`full_reparse.rs` — `label_for` gains `BlockKind::Html { .. } => "html",`; the reparse-side match gains, above the `_ => "skipped"` catch-all:

```rust
            // Spec §4.2(4): a spliced html block must label "html" on both
            // sides — the old catch-all bucketed it as "skipped", which
            // would fail-cascade every successfully translated block.
            NodeValue::HtmlBlock(_) => "html",
```

`render.rs` — `wrapper_element_for` gains `BlockKind::Html { .. } => "div",` (comment: matches the table/code-block/blockquote transparent-div pattern). In `render_block`, insert the Html arm between the ThematicBreak early-return and the Skipped early-return:

```rust
    if let BlockKind::Html { .. } = kind {
        // Spec §5: live render is a translated/preserved privilege; fallback
        // (and extraction-failure, which align marks fallback_source) keeps
        // the DCR-0013 escaped placeholder. Live output is auto-balanced
        // (spec §3.4) so an unbalanced fragment can never swallow sibling
        // sync wrappers after the DOMPurify innerHTML mount.
        let attrs = attrs::write_attrs(align_block);
        if matches!(align_block.fallback_status, FallbackStatus::FallbackSource) {
            let _ = writeln!(
                out,
                "<pre{attrs} data-skipped=\"html-block\">{}</pre>",
                html_escape(md),
            );
        } else {
            let _ = writeln!(
                out,
                "<div{attrs}>{}</div>",
                crate::htmlseg::balance_fragment(md),
            );
        }
        return;
    }
```

`unit.rs` — TEMPORARY: add `BlockKind::Html { .. }` to the `is_translatable` exclusion `matches!` with a `// Task 5 flips this` comment. `align.rs` — TEMPORARY: add `BlockKind::Html { .. }` to the `translatable` exclusion `matches!` in `build_alignment_map` (same comment). This keeps the unit/align/`has_translatable_blocks` triple in agreement through the intermediate state.

- [ ] **Step 4: Repair the tests the new kind breaks**

Run: `cargo test --workspace -- --test-threads=4` and fix in this order:

1. `parser.rs::skipped_node_tests` — the two surviving tests used raw HTML as the Skipped specimen. With the current comrak options nothing else parses to Skipped, so convert them to synthetic coverage: keep `ordinary_document_has_no_skip_warnings` as-is (still valid); rewrite `skipped_block_id_round_trips_assign_block_ids` to construct the block directly:

```rust
    #[test]
    fn skipped_block_id_round_trips_assign_block_ids() {
        // No node maps to Skipped under the current comrak options (html
        // became a real kind); pin the re-key path synthetically.
        let mut doc = parse("real paragraph\n").expect("parses");
        doc.blocks.insert(
            0,
            Block {
                block_id: BlockId::new("x", 99),
                kind: BlockKind::Skipped { label: "unsupported".to_string() },
                source_range: ByteRange { start: 0, end: 0 },
                source_hash: 0,
                parent_id: None,
                section_path: Vec::new(),
                ast_path: AstPath(Vec::new()),
            },
        );
        crate::id::assign_block_ids(&mut doc);
        assert_eq!(doc.blocks[0].block_id.0, "x-0001");
        assert_eq!(doc.blocks[1].block_id.0, "p-0002");
    }
```

2. `unit.rs::skipped_unit_tests::skipped_node_yields_no_translation_unit` — same treatment: parse `"real paragraph\n"`, insert a synthetic `Skipped` block (as above), assert no `x-` unit id is produced. The `oi_0026_tests` table row `("<div class=\"note\">n</div>\n", false)` stays `false` in this task (Html still excluded) — annotate it `// flips to true in Task 5`.
3. `render.rs::skipped_render_tests::raw_html_block_renders_as_escaped_placeholder` — the html specimen is now LIVE. Rewrite as `html_render_tests`:

```rust
// Spec §5: preserved/translated html blocks live-render inside the div
// wrapper; the escaped placeholder is fallback-only.
#[cfg(test)]
mod html_render_tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::align::build_alignment_map;
    use crate::{id, parser};

    fn render(src: &str) -> String {
        let mut doc = parser::parse(src).expect("parses");
        id::assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        let map = build_alignment_map(
            &doc,
            &[],
            &crate::regen::BlockOffsets::default(),
            &opts,
            None,
        );
        render_source(&doc, &map)
    }

    #[test]
    fn preserved_html_block_live_renders_unescaped_and_balanced() {
        let html = render("<div class=\"note\">side note</div>\n\npara\n");
        assert!(
            html.contains("data-sync-id=\"html-0001\""),
            "html anchor present:\n{html}"
        );
        assert!(
            html.contains("<div class=\"note\">side note</div>"),
            "content is live, not escaped:\n{html}"
        );
        assert!(!html.contains("data-skipped=\"html-block\""), "no placeholder:\n{html}");
    }

    #[test]
    fn unclosed_html_fragment_is_balanced_in_the_pane() {
        let html = render("<div align=\"center\">\n<b>Hero</b>\n\npara\n");
        let open = html.matches("<div").count();
        let close = html.matches("</div>").count();
        assert_eq!(open, close, "balanced output:\n{html}");
    }
}
```

(Note: `build_alignment_map`'s signature changes in Task 6 — this helper gets a `&HashMap` argument added then; write it against the CURRENT signature now.)
4. `align.rs::skipped_row_tests::skipped_row_is_preserved_anchor_and_uncounted` — specimen was html; convert to the synthetic-Skipped construction (same pattern as step 1) or switch the fixture to an image-only paragraph; keep the assertions (Preserved, Anchor, uncounted). Add a twin test `html_row_is_preserved_anchor_and_uncounted_until_task5` asserting the html row currently reports `block_kind == "html"`, `fallback_status == Preserved`, `sync_role == Anchor`, `total_units == 0` — Task 6 will rename/extend it.
5. `full_reparse.rs` inline tests — if any used raw html as a "skipped" specimen, update the expected label to `"html"`.
6. `crates/transync/tests/reader_honesty.rs::raw_html_block_round_trips_as_escaped_anchored_placeholder` — rewrite to the new posture (rename to `raw_html_block_round_trips_live_rendered_and_anchored`): translate the reader-honesty fixture with `MockTranslator::passthrough()`; assert (a) the html block's bytes appear VERBATIM in `translated_markdown`, (b) both `annotated_*_html` contain `data-sync-id="html-…"` and do NOT contain `data-skipped="html-block"`, (c) the alignment row for it has `block_kind == "html"`. Read the existing file first and keep its fixture + structure; only the html-expectations flip.
7. `scn_14_full.rs` / other scenarios — should be untouched (their fixtures contain no raw html yet). If a scenario fails, read the failure before editing anything.

- [ ] **Step 5: Full workspace green**

Run: `cargo test --workspace -- --test-threads=4` then `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4`
Expected: PASS everywhere.

- [ ] **Step 6: Lint + commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add -A crates/transync-core/src crates/transync/tests
git commit -m "core: BlockKind::Html through parser/render/align — live-rendered, not yet batched

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Unit construction — outcomes, payload, constraints, input mode

**Files:**
- Modify: `crates/transync-core/src/llm.rs` (`HtmlSegmentConstraints`, `BlockConstraints.html`, `InputMode::HtmlSegments`)
- Modify: `crates/transync-core/src/unit.rs` (`HtmlOutcome`, `html_outcomes`, per-block filter, payload/constraints/input-mode arms, signature changes)
- Modify: `crates/transync-core/src/batch.rs` (estimate: label tokens)

**Interfaces:**
- Consumes: `htmlseg::extract` (Task 2).
- Produces:
  - `llm.rs`: `pub struct HtmlSegmentConstraints { pub segment_count: u32, pub segment_labels: Vec<String>, pub source_bytes: String, pub block_type: u8 }`; `BlockConstraints` gains `pub html: Option<HtmlSegmentConstraints>`; `InputMode` gains `HtmlSegments` (unit variant, no fields).
  - `unit.rs`: `#[derive(Debug, Clone, PartialEq, Eq)] pub enum HtmlOutcome { Unit, PreservedZeroSegment, ExtractionFailed(String) }`; `pub fn html_outcomes(doc: &Document) -> std::collections::HashMap<BlockId, HtmlOutcome>`; `pub fn build_batches(doc: &Document, opts: &TranslateOptions, tokenizer_hint: Option<TokenizerHint>, html_outcomes: &std::collections::HashMap<BlockId, HtmlOutcome>) -> Vec<TranslationBatch>`; `pub(crate) fn has_translatable_blocks(doc: &Document, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> bool`.
  - Unit payload for html: `serde_json::to_string(&texts)` (a compact JSON array of decoded segment strings); `unit.constraints.html = Some(…)`; `input_mode = InputMode::HtmlSegments`.

- [ ] **Step 1: Write the failing tests**

New module at the bottom of `unit.rs`:

```rust
// Spec §3.2–§3.3: per-block translatability, JSON segment payload, and the
// zero-segment / extraction-failure outcomes.
#[cfg(test)]
mod html_unit_tests {
    use super::*;
    use crate::id::assign_block_ids;
    use crate::parser::parse;

    fn opts() -> TranslateOptions {
        TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        }
    }

    #[test]
    fn html_block_with_text_yields_a_json_segment_unit() {
        let mut doc = parse("<details><summary>Click &amp; go</summary></details>\n").expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(outcomes.get(&doc.blocks[0].block_id), Some(&HtmlOutcome::Unit));

        let batches = build_batches(&doc, &opts(), None, &outcomes);
        let unit = &batches[0].units[0];
        assert_eq!(unit.unit_id.0, "html-0001");
        assert!(matches!(unit.input_mode, InputMode::HtmlSegments));
        assert_eq!(unit.source_payload, "[\"Click & go\"]", "decoded, compact JSON");
        let h = unit.constraints.html.as_ref().expect("html constraints");
        assert_eq!(h.segment_count, 1);
        assert_eq!(h.segment_labels, vec!["summary".to_string()]);
        assert_eq!(h.block_type, 6);
        assert!(h.source_bytes.contains("&amp;"), "raw bytes, not decoded");
    }

    #[test]
    fn zero_segment_html_block_builds_no_unit() {
        let mut doc = parse("<!-- just a comment -->\n\nreal paragraph\n").expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(
            outcomes.get(&doc.blocks[0].block_id),
            Some(&HtmlOutcome::PreservedZeroSegment)
        );
        let batches = build_batches(&doc, &opts(), None, &outcomes);
        assert!(
            batches.iter().flat_map(|b| &b.units).all(|u| !u.unit_id.0.starts_with("html-")),
            "no unit for a zero-segment block"
        );
    }

    #[test]
    fn has_translatable_blocks_matches_batch_emptiness_for_html() {
        // The doc-comment contract: exactly the predicate deciding batch
        // emptiness — now per-block for html.
        for (src, expected) in [
            ("<!-- only a comment -->\n", false),
            ("<div class=\"note\">n</div>\n", true),
            ("<img src=\"a.png\">\n", false), // html img wall: zero segments
        ] {
            let mut doc = parse(src).expect("parses");
            assign_block_ids(&mut doc);
            let outcomes = html_outcomes(&doc);
            assert_eq!(
                has_translatable_blocks(&doc, &outcomes),
                expected,
                "predicate for {src:?}"
            );
            assert_eq!(
                !build_batches(&doc, &opts(), None, &outcomes).is_empty(),
                expected,
                "batches for {src:?}"
            );
        }
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p transync-core html_unit -- --test-threads=4`
Expected: FAIL — `HtmlOutcome`, 4-arg `build_batches` unresolved.

- [ ] **Step 3: Implement**

`llm.rs` — after `BlockConstraints`'s existing fields:

```rust
    /// Html-kind constraints (spec §3.2/§4.2): segment count + parent labels
    /// feed the prompt hints and the per-kind count check; `source_bytes`
    /// and `block_type` are validator-side inputs for the splice check and
    /// are NEVER serialized into the prompt.
    pub html: Option<HtmlSegmentConstraints>,
```

with the struct (near `ListTopologyEntry`):

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlSegmentConstraints {
    pub segment_count: u32,
    pub segment_labels: Vec<String>,
    /// Raw source bytes of the block — the splice-check input. Never sent
    /// to the model.
    pub source_bytes: String,
    /// CommonMark HTML block type (1–7); types 6/7 get blank-line
    /// normalization at splice, type 1 is exempt (spec §3.3).
    pub block_type: u8,
}
```

`InputMode` gains:

```rust
    /// Block-level raw HTML: `source_payload` is a compact JSON array of
    /// decoded text segments (spec §4.1); markup never reaches the model.
    HtmlSegments,
```

`unit.rs`:

```rust
/// Per-block extraction outcome for `BlockKind::Html` blocks (spec §3.2).
/// Computed once per run by [`html_outcomes`] and threaded to batching,
/// alignment, and the report so the three stay in agreement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlOutcome {
    /// Extraction produced ≥1 segment — a normal translation unit.
    Unit,
    /// Zero translatable segments — preserved, live-rendered, uncounted.
    PreservedZeroSegment,
    /// The rewriter errored — placeholder presentation, uncounted.
    ExtractionFailed(String),
}

pub fn html_outcomes(doc: &Document) -> HashMap<BlockId, HtmlOutcome> {
    let mut out = HashMap::new();
    for block in &doc.blocks {
        if !matches!(block.kind, BlockKind::Html { .. }) {
            continue;
        }
        let payload = block_payload(doc, block);
        let outcome = match crate::htmlseg::extract(&payload) {
            Ok(segs) if segs.texts.is_empty() => HtmlOutcome::PreservedZeroSegment,
            Ok(_) => HtmlOutcome::Unit,
            Err(e) => HtmlOutcome::ExtractionFailed(e),
        };
        out.insert(block.block_id.clone(), outcome);
    }
    out
}
```

(Imports: `use crate::id::BlockId;` and `use std::collections::HashMap;` — add to the existing `use` block.)

`is_translatable` reverts to kind-level (drop the Task-4 Html exclusion) and a per-block wrapper appears:

```rust
fn is_translatable(kind: &BlockKind) -> bool {
    !matches!(
        kind,
        BlockKind::ThematicBreak | BlockKind::Image | BlockKind::Skipped { .. }
    )
}

/// Per-block translatability (spec §3.2): kind-level for every kind except
/// Html, where the extraction outcome decides. MUST stay in exact agreement
/// with `build_batches` and `align::build_alignment_map`.
pub(crate) fn is_translatable_block(
    block: &Block,
    html_outcomes: &HashMap<BlockId, HtmlOutcome>,
) -> bool {
    if !is_translatable(&block.kind) {
        return false;
    }
    if matches!(block.kind, BlockKind::Html { .. }) {
        return matches!(html_outcomes.get(&block.block_id), Some(HtmlOutcome::Unit));
    }
    true
}
```

`build_batches` gains the 4th param and its filter becomes `if !is_translatable_block(block, html_outcomes) { continue; }`. `has_translatable_blocks(doc, html_outcomes)` becomes `doc.blocks.iter().any(|b| is_translatable_block(b, html_outcomes))` (update its doc-comment: still "exactly the predicate…").

Payload + constraints + input-mode arms (inside the per-block loop, replacing the plain `block_payload` call for html):

```rust
        let (payload, input_mode, constraints) = if matches!(block.kind, BlockKind::Html { .. }) {
            let raw = block_payload(doc, block);
            let segs = crate::htmlseg::extract(&raw)
                .expect("outcome said Unit — extract cannot fail here (same input, same routine)");
            let payload = serde_json::to_string(&segs.texts)
                .expect("Vec<String> JSON serialization is infallible");
            let block_type = match block.kind {
                BlockKind::Html { block_type } => block_type,
                _ => unreachable!(),
            };
            let mut c = BlockConstraints::default();
            c.html = Some(crate::llm::HtmlSegmentConstraints {
                segment_count: segs.texts.len() as u32,
                segment_labels: segs.labels,
                source_bytes: raw,
                block_type,
            });
            (payload, InputMode::HtmlSegments, c)
        } else {
            let payload = block_payload(doc, block);
            let constraints = constraints_for(&block.kind, &payload);
            (payload.clone(), input_mode_for(&block.kind), constraints)
        };
```

(Adapt to the actual loop shape — today it computes `payload`/`constraints`/`input_mode` as three separate lets; fold them into this tuple. `constraints_for` and `input_mode_for` keep their `_ => {}` / `_ => TextFragment` defaults and are simply not called for html.)

`align.rs` — REMOVE the Task-4 temporary Html exclusion (Task 6 threads outcomes through; until then align compiles against the old predicate — to keep the tree green within this task, leave align's temporary exclusion in place and note it; Task 6 removes it).

`batch.rs::estimate_unit` — add to `constraint_tokens`:

```rust
        + c.html.as_ref().map_or(0, |h| h.segment_labels.len() * 3 + 4)
```

- [ ] **Step 4: Fix the callers the signature broke**

`cargo check --workspace` will name them all. Known list:
- `crates/transync-core/src/pipeline.rs` — `build_batches(&doc, opts, tokenizer_hint)` call and the `has_translatable_blocks` call: compute `let html_outcomes = crate::unit::html_outcomes(&doc);` right after `assign_block_ids` and pass `&html_outcomes` (full pipeline wiring is Task 6; here just make it compile).
- `crates/transync-core/src/unit.rs` inline tests (`skipped_unit_tests`, `oi_0026_tests`) — pass `&html_outcomes(&doc)`; flip the `("<div class=\"note\">n</div>\n", …)` row to `true` and remove the Task-4 comment.
- `crates/transync-openai/tests/live_smoke.rs` — the `build_batches` call gains `&transync::unit::html_outcomes(&doc)` (check the module path it already uses).
- Any other caller `cargo check` reports.

- [ ] **Step 5: Run the suite**

Run: `cargo test --workspace -- --test-threads=4`
Expected: PASS (html blocks now batch; scenario fixtures contain no raw html yet, so pipeline behavior is unchanged for them).

- [ ] **Step 6: Lint + commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add -A crates/transync-core/src crates/transync-openai/tests
git commit -m "core: html segment units — outcomes, JSON payload, HtmlSegmentConstraints (spec §3.2–§3.3)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Pipeline + alignment plumbing (outcomes → align, report warnings)

**Files:**
- Modify: `crates/transync-core/src/align.rs` (`build_alignment_map` signature + per-block predicate + status mapping)
- Modify: `crates/transync-core/src/pipeline.rs` (thread outcomes; report warnings)
- Modify: `crates/transync-core/src/render.rs` (test helper call-site)

**Interfaces:**
- Consumes: `unit::{HtmlOutcome, html_outcomes, is_translatable_block}` (Task 5).
- Produces: `pub fn build_alignment_map(doc: &Document, validated: &[ValidatedBatch], offsets: &BlockOffsets, opts: &TranslateOptions, detected_source_language: Option<String>, html_outcomes: &HashMap<BlockId, HtmlOutcome>) -> AlignmentMap`. Counting rule (spec §5): unit-backed html blocks counted in `validation_summary`; zero-segment and extraction-failed html blocks NOT counted; extraction-failed rows carry `fallback_status: fallback_source`, zero-segment rows `preserved`; both `block_kind: "html"`, `sync_role: anchor`.

- [ ] **Step 1: Write the failing align tests**

Extend/replace the Task-4 module in `align.rs`:

```rust
// Spec §3.2/§5: per-block html accounting — Unit counted; ZeroSegment
// preserved+uncounted; ExtractionFailed fallback_source+uncounted.
#[cfg(test)]
mod html_row_tests {
    use super::*;
    use crate::TranslateOptions;
    use crate::id::assign_block_ids;
    use crate::parser::parse;
    use crate::unit::{HtmlOutcome, html_outcomes};
    use std::collections::HashMap;

    fn map_for(src: &str, outcomes: &HashMap<crate::id::BlockId, HtmlOutcome>) -> AlignmentMap {
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let opts = TranslateOptions {
            target_language: "ko".to_string(),
            ..TranslateOptions::default()
        };
        build_alignment_map(
            &doc,
            &[],
            &crate::regen::BlockOffsets::default(),
            &opts,
            None,
            outcomes,
        )
    }

    #[test]
    fn zero_segment_html_row_is_preserved_anchor_and_uncounted() {
        let src = "<!-- note -->\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        let map = map_for(src, &outcomes);
        let row = &map.blocks[0];
        assert_eq!(row.block_kind, "html");
        assert_eq!(row.fallback_status, FallbackStatus::Preserved);
        assert_eq!(row.sync_role, SyncRole::Anchor);
        assert_eq!(map.validation_summary.total_units, 0, "not a unit — uncounted");
    }

    #[test]
    fn extraction_failed_html_row_is_fallback_source_and_uncounted() {
        let src = "<div>x</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let mut outcomes = HashMap::new();
        outcomes.insert(
            doc.blocks[0].block_id.clone(),
            HtmlOutcome::ExtractionFailed("boom".to_string()),
        );
        let map = map_for(src, &outcomes);
        let row = &map.blocks[0];
        assert_eq!(row.block_kind, "html");
        assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
        assert_eq!(map.validation_summary.total_units, 0, "uncounted");
    }

    #[test]
    fn unit_backed_html_block_missing_from_results_is_fallback_and_counted() {
        // The existing internal-bug rule extends to html Unit blocks.
        let src = "<div>real text</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let outcomes = html_outcomes(&doc);
        assert_eq!(outcomes.values().next(), Some(&HtmlOutcome::Unit));
        let map = map_for(src, &outcomes);
        assert_eq!(map.blocks[0].fallback_status, FallbackStatus::FallbackSource);
        assert_eq!(map.validation_summary.total_units, 1, "Unit blocks count");
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-core html_row -- --test-threads=4` → FAIL (5-arg signature).

- [ ] **Step 3: Implement**

`align.rs` — signature gains `html_outcomes: &HashMap<BlockId, HtmlOutcome>` (import from `crate::unit`); remove the Task-4 temporary exclusion; the predicate becomes:

```rust
        let translatable = crate::unit::is_translatable_block(block, html_outcomes);
```

and the non-translatable status arm becomes:

```rust
        } else {
            match (&block.kind, html_outcomes.get(&block.block_id)) {
                // Spec §3.2: the rewriter errored — placeholder presentation,
                // honest fallback status, uncounted.
                (BlockKind::Html { .. }, Some(HtmlOutcome::ExtractionFailed(_))) => {
                    FallbackStatus::FallbackSource
                }
                _ => FallbackStatus::Preserved,
            }
        };
```

(Keep the surrounding comments; extend the R0001-0022 counting comment with one line: html Unit blocks count, zero-segment/extraction-failed html blocks do not — spec §5.)

`pipeline.rs` — after `assign_block_ids`: `let html_outcomes = crate::unit::html_outcomes(&doc);` feeding `has_translatable_blocks`, `build_batches`, and `build_alignment_map`. After the `skipped_source_nodes` mirror line, append the outcome warnings (deterministic order):

```rust
    // Spec §3.2: zero-segment and extraction-failed html blocks surface on
    // the same reader-honesty channel as parser skips — visible, not silent.
    let mut html_notes: Vec<String> = html_outcomes
        .iter()
        .filter_map(|(id, o)| match o {
            crate::unit::HtmlOutcome::Unit => None,
            crate::unit::HtmlOutcome::PreservedZeroSegment => Some(format!(
                "html block {id} contains no translatable text: it is preserved verbatim and live-rendered (possibly visually empty)"
            )),
            crate::unit::HtmlOutcome::ExtractionFailed(e) => Some(format!(
                "html block {id}: segment extraction failed ({e}); it is preserved verbatim and rendered as an inert escaped placeholder"
            )),
        })
        .collect();
    html_notes.sort();
    validation_report.skipped_source_nodes.extend(html_notes);
```

- [ ] **Step 4: Fix callers** — `cargo check --workspace` names them: `render.rs` test helper(s) (`html_render_tests`, `list_grouping_tests`, `refmap_render_tests` — add `&crate::unit::html_outcomes(&doc)` or an empty map via `&Default::default()` where the fixture has no html), `align.rs` other tests, any pipeline test.

For non-html fixtures `&HashMap::new()` is equivalent and fine — but prefer `&html_outcomes(&doc)` so helpers stay copy-paste-safe.

- [ ] **Step 5: Run + zero-segment warning test**

Add one pipeline-level test to `pipeline.rs` (module `html_outcome_report_tests`) that runs `translate` (with the crate's test stub or a minimal inline `Translator` impl — copy the pattern from an existing pipeline inline test) over `"<!-- note -->\n\npara\n"` and asserts `validation_report.skipped_source_nodes.iter().any(|w| w.contains("no translatable text"))`.

Run: `cargo test --workspace -- --test-threads=4` → PASS.

- [ ] **Step 6: Lint + commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add -A crates/transync-core/src
git commit -m "core: thread html outcomes through pipeline/align + reader-honesty warnings (spec §3.2, §5)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Prompt — html instruction, hints, input-mode label

**Files:**
- Modify: `crates/transync-core/src/llm/prompt.rs`

**Interfaces:**
- Consumes: `InputMode::HtmlSegments`, `BlockConstraints.html` (Task 5).
- Produces: `input_mode_label` → `"html_segments"`; `ConstraintHints` gains `html_segment_count: Option<u32>` + `html_segment_labels: Option<Vec<String>>` (raw `source_bytes`/`block_type` NEVER serialized); a conditional instruction paragraph appended only when the batch contains an html unit (goldens unchanged — their fixture has no html unit).

- [ ] **Step 1: Write the failing tests**

In `prompt.rs`'s `mod tests`:

```rust
    // Spec §4.1: html wire framing — conditional instruction + hints.
    #[test]
    fn html_unit_gains_segment_instruction_and_hints() {
        let mut batch = fixture_batch();
        let mut unit = batch.units[0].clone();
        unit.unit_id = crate::id::BlockId::new("html", 9);
        unit.block_kind = crate::id::BlockKind::Html { block_type: 6 };
        unit.input_mode = InputMode::HtmlSegments;
        unit.source_payload = "[\"Click\",\"here\"]".to_string();
        unit.constraints = BlockConstraints {
            html: Some(crate::llm::HtmlSegmentConstraints {
                segment_count: 2,
                segment_labels: vec!["summary".to_string(), "a".to_string()],
                source_bytes: "<summary>Click<a>here</a></summary>".to_string(),
                block_type: 6,
            }),
            ..BlockConstraints::default()
        };
        batch.units.push(unit);

        let prompt = build_user_prompt(&batch).expect("builds");
        assert!(prompt.contains("html_segments"), "input mode label:\n{prompt}");
        assert!(prompt.contains("SAME"), "count instruction present:\n{prompt}");
        assert!(prompt.contains("\"html_segment_count\":2") || prompt.contains("\"html_segment_count\": 2"), "count hint:\n{prompt}");
        assert!(prompt.contains("summary"), "labels hint:\n{prompt}");
        assert!(
            !prompt.contains("<summary>Click<a>here</a></summary>"),
            "raw source bytes must NEVER reach the prompt:\n{prompt}"
        );
    }

    #[test]
    fn batch_without_html_units_keeps_the_legacy_instruction() {
        let prompt = build_user_prompt(&fixture_batch()).expect("builds");
        assert!(!prompt.contains("html_segments"), "no html leak:\n{prompt}");
    }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-core prompt -- --test-threads=4` → FAIL (`HtmlSegments` label match non-exhaustive or hints missing).

- [ ] **Step 3: Implement**

- `input_mode_label`: add `InputMode::HtmlSegments => "html_segments",`.
- `ConstraintHints`: add the two fields (serde skip-if-none like the others), extend the builder (`hint_constraints`) with:

```rust
        html_segment_count: c.html.as_ref().map(|h| h.segment_count),
        html_segment_labels: c
            .html
            .as_ref()
            .map(|h| h.segment_labels.clone())
            .filter(|l| !l.is_empty()),
```

and extend `is_empty()` with both fields (the golden canary `golden_schema_objects` doesn't touch ConstraintHints; the two user-prompt goldens have `html: None` so their bytes are unchanged — the golden tests MUST still pass untouched in this task).
- Conditional instruction, appended after the existing `preserve_code_identifiers` gate:

```rust
    if batch
        .units
        .iter()
        .any(|u| matches!(u.block_kind, crate::id::BlockKind::Html { .. }))
    {
        instruction.push_str(
            " Units with input_mode \"html_segments\" carry a JSON array of text segments \
             extracted from a raw-HTML block; markup never appears in the payload. Return \
             translated_payload as a JSON array string with the SAME element count and order \
             (example: source payload [\"Click\",\"here\"] -> translated_payload \
             \"[\\\"클릭\\\",\\\"여기\\\"]\"). Segments sharing a parent element are pieces of \
             one sentence: translate each so the concatenation reads naturally. Echo a segment \
             unchanged to preserve it. Never merge, split, drop, or add segments.",
        );
    }
```

- [ ] **Step 4: Run** — `cargo test -p transync-core prompt -- --test-threads=4` and the golden tests: `cargo test -p transync-core golden -- --test-threads=4`. Expected: ALL PASS with goldens byte-untouched.

- [ ] **Step 5: Lint + commit**

```bash
cargo fmt --all && cargo clippy -p transync-core --all-targets -- -D warnings
git add crates/transync-core/src/llm/prompt.rs
git commit -m "core: html_segments prompt framing — conditional instruction + label/count hints (spec §4.1)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Validation — per-kind check, fragment skip, splice check (direct fallback)

**Files:**
- Modify: `crates/transync-core/src/validate/per_kind.rs` (`check_html` replaces the Task-4 permissive arm)
- Modify: `crates/transync-core/src/validate/fragment_reparse.rs` (Html skip)
- Modify: `crates/transync-core/src/validate.rs` (splice-check step in `validate_unit`)

**Interfaces:**
- Consumes: `htmlseg::{splice, tag_inventory}` (Task 3), `BlockConstraints.html` (Task 5).
- Produces: layer behavior per spec §4.2 —
  - per-kind (retryable): JSON parse / count / non-empty
  - fragment reparse: skipped for Html (payload is JSON, not Markdown)
  - splice check (layer 3): runs the real splice; rewriter error or inventory divergence → **direct fallback** (`final_status: FallbackSource`, `accepted_payload: None`, `rejected_by: None` — no retry burn, spec §3.3), with the reason in `warnings`
  - the existing `Preserved`-claims-byte-equality check applies to the JSON payload encoding unchanged.

- [ ] **Step 1: Write the failing per-kind tests**

In `per_kind.rs`'s test module (match its helper style — `fn unit_result(payload: &str) -> UnitResult`):

```rust
    // Spec §4.2 layer 2: html payload shape — all retryable.
    fn html_constraints(count: u32) -> BlockConstraints {
        BlockConstraints {
            html: Some(crate::llm::HtmlSegmentConstraints {
                segment_count: count,
                segment_labels: vec!["p".to_string(); count as usize],
                source_bytes: String::new(),
                block_type: 6,
            }),
            ..BlockConstraints::default()
        }
    }

    #[test]
    fn html_payload_that_is_not_json_is_rejected() {
        let err = check(
            &html_constraints(1),
            &BlockKind::Html { block_type: 6 },
            &unit_result("not json"),
        )
        .unwrap_err();
        assert!(err.contains("JSON array"), "got: {err}");
    }

    #[test]
    fn html_segment_count_change_is_rejected() {
        let err = check(
            &html_constraints(2),
            &BlockKind::Html { block_type: 6 },
            &unit_result("[\"only one\"]"),
        )
        .unwrap_err();
        assert!(err.contains("segment count"), "got: {err}");
    }

    #[test]
    fn empty_html_segment_is_rejected() {
        let err = check(
            &html_constraints(2),
            &BlockKind::Html { block_type: 6 },
            &unit_result("[\"ok\",\"\"]"),
        )
        .unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn well_shaped_html_payload_passes() {
        assert!(
            check(
                &html_constraints(2),
                &BlockKind::Html { block_type: 6 },
                &unit_result("[\"하나\",\"둘\"]"),
            )
            .is_ok()
        );
    }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-core per_kind -- --test-threads=4` → FAIL (permissive arm accepts everything).

- [ ] **Step 3: Implement `check_html` + fragment skip**

`per_kind.rs` — replace the Task-4 arm with `BlockKind::Html { .. } => check_html(constraints, result),` and:

```rust
/// Spec §4.2 layer 2 (html): JSON array of strings, element count equals
/// the source segment count, no empty element (source segments are
/// non-empty by construction — the extraction drop step). All retryable.
pub fn check_html(constraints: &BlockConstraints, result: &UnitResult) -> Result<(), String> {
    let Some(h) = &constraints.html else {
        return Ok(());
    };
    let segs: Vec<String> = serde_json::from_str(&result.translated_payload)
        .map_err(|e| format!("html payload is not a JSON array of strings: {e}"))?;
    if segs.len() != h.segment_count as usize {
        return Err(format!(
            "html segment count changed: expected {}, got {}",
            h.segment_count,
            segs.len()
        ));
    }
    if let Some(i) = segs.iter().position(|s| s.is_empty()) {
        return Err(format!(
            "html segment {i} is empty; echo the source segment to preserve it"
        ));
    }
    Ok(())
}
```

`fragment_reparse.rs` — at the top of its per-kind entry (read the file; the function `validate_unit` calls is `reparse_fragment(&unit.block_kind, result)`), add:

```rust
    if matches!(kind, BlockKind::Html { .. }) {
        // Spec §4.2: html payloads are JSON, not Markdown — structure is
        // owned by the splice check (layer 3), not a comrak reparse.
        return Ok(());
    }
```

- [ ] **Step 4: Write the failing splice-check test, then wire it into `validate_unit`**

Test in `validate.rs` (new module `html_splice_layer_tests`; copy the batch/result construction style from `preserved_tests` — build a one-unit `TranslationBatch` with an Html unit whose `constraints.html.source_bytes` is a real block, and a `TranslationBatchResult` echoing a VALID JSON payload):

```rust
    // Spec §4.2 layer 3 + §3.3: a splice-engine failure is not a model
    // fault — direct fallback, rejected_by None (no verbatim retry burn).
    #[test]
    fn splice_engine_failure_falls_back_directly_without_retry_marker() {
        // Force the engine to fail by giving constraints whose source_bytes
        // disagree with the payload segment count (splice's own count guard).
        let (batch, result) = html_batch_and_result(
            "<p>one</p><p>two</p>", // 2 kept segments
            "[\"하나\"]",            // 1 translation → splice count mismatch
        );
        // per-kind must not reject first: claim segment_count = 1 so layer 2
        // passes and the mismatch surfaces inside the splice engine.
        let vb = validate_batch(&batch_with_count(batch, 1), &result, "");
        let vu = &vb.units[0];
        assert_eq!(vu.final_status, FallbackStatus::FallbackSource);
        assert!(vu.accepted_payload.is_none());
        assert!(vu.rejected_by.is_none(), "direct fallback — not retryable");
        assert!(
            vu.warnings.iter().any(|w| w.contains("splice")),
            "cause recorded: {:?}",
            vu.warnings
        );
    }

    #[test]
    fn clean_html_splice_is_accepted_with_wire_payload() {
        let (batch, result) = html_batch_and_result("<p>one</p>", "[\"하나\"]");
        let vb = validate_batch(&batch, &result, "");
        let vu = &vb.units[0];
        assert_eq!(vu.final_status, FallbackStatus::Translated);
        assert_eq!(
            vu.accepted_payload.as_deref(),
            Some("[\"하나\"]"),
            "accepted_payload stays the WIRE payload — regen re-splices (spec §3.3); \
             this is also what keeps cached entries wire-shaped"
        );
    }
```

Write the two helpers (`html_batch_and_result`, `batch_with_count`) concretely in the test module: a `TranslationUnit` with `unit_id: BlockId::new("html", 1)`, `block_kind: BlockKind::Html { block_type: 6 }`, `input_mode: InputMode::HtmlSegments`, `source_payload` = the JSON of the extracted texts (call `crate::htmlseg::extract` on the source bytes to build it), `constraints.html = Some(HtmlSegmentConstraints { segment_count, segment_labels, source_bytes, block_type: 6 })`, defaulted context/hash/batch fields; a matching one-row `TranslationBatchResult` with `OutputKind::Translated`. `batch_with_count` clones the batch and overwrites `constraints.html.segment_count`.

Implementation in `validate_unit`, after the `inline::check_inline` step and before the final accepted `ValidatedUnit` construction:

```rust
    if let BlockKind::Html { block_type } = &unit.block_kind {
        if let Some(h) = &unit.constraints.html {
            // Layer 3 (spec §4.2): run the real splice as a check. Errors
            // here are engine faults, not model faults — direct fallback,
            // never the retry loop (spec §3.3).
            let direct_fallback = |msg: String| ValidatedUnit {
                unit_id: unit.unit_id.clone(),
                final_status: FallbackStatus::FallbackSource,
                accepted_payload: None,
                rejected_by: None,
                rejection_reason: None,
                warnings: vec![msg],
            };
            match serde_json::from_str::<Vec<String>>(&result.translated_payload) {
                Err(e) => {
                    // per_kind already rejected malformed JSON; defensive.
                    return direct_fallback(format!("html splice precheck: payload unparsable: {e}"));
                }
                Ok(segs) => match crate::htmlseg::splice(&h.source_bytes, &segs, *block_type) {
                    Err(e) => return direct_fallback(format!("html splice failed: {e}")),
                    Ok(spliced) => {
                        if crate::htmlseg::tag_inventory(&spliced)
                            != crate::htmlseg::tag_inventory(&h.source_bytes)
                        {
                            return direct_fallback(
                                "html splice check: tag inventory diverged from source (engine fault)"
                                    .to_string(),
                            );
                        }
                    }
                },
            }
        }
    }
```

(Adapt the exact return shape to `validate_unit`'s real structure — it returns `ValidatedUnit` directly; the rejection arms above it are the pattern to mirror.)

- [ ] **Step 5: Run** — `cargo test -p transync-core validate -- --test-threads=4` then the workspace. Expected: PASS. Also confirm the existing `preserved_tests` still pass (the Preserved byte-check needs no change — it compares against `unit.source_payload`, which IS the JSON encoding for html units).

- [ ] **Step 6: Lint + commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add crates/transync-core/src/validate.rs crates/transync-core/src/validate
git commit -m "core: html validation — per-kind shape, fragment skip, splice check with direct fallback (spec §4.2)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Always-on inline raw-HTML tag guard + VALIDATION_SCHEMA_VERSION bump

**Files:**
- Modify: `crates/transync-core/src/validate/inline.rs`
- Modify: `crates/transync-core/src/llm/prompt.rs` (unconditional inline-tag instruction + golden regeneration)
- Modify: `crates/transync-core/src/validate.rs` (`VALIDATION_SCHEMA_VERSION` 1 → 2)

**Interfaces:**
- Consumes: comrak `NodeValue::HtmlInline`.
- Produces: ordered verbatim tag-token identity for every inline-bearing unit, enforced regardless of profile policy (spec §4.3); prompt mirror ("never punished for a rule it was not told"); cache generation 2.

- [ ] **Step 1: Write the failing guard tests**

In `inline.rs`'s `mod tests` (reuse its `unit`/`unit_result`/`policy` helpers):

```rust
    // Spec §4.3: always-on ordered raw-inline-HTML tag identity.
    #[test]
    fn dropped_inline_tag_is_rejected() {
        let u = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd> now");
        let err = check_inline(&policy(None, None), &u, &unit_result("press Ctrl now"), "")
            .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn mangled_inline_tag_is_rejected() {
        let u = unit(BlockKind::Paragraph, "a <b>bold</b> word");
        let err = check_inline(
            &policy(None, None),
            &u,
            &unit_result("a <strong>bold</strong> word"),
            "",
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn reordered_inline_tags_are_rejected() {
        let u = unit(BlockKind::Paragraph, "<sup>a</sup> then <sub>b</sub>");
        let err = check_inline(
            &policy(None, None),
            &u,
            &unit_result("<sub>b</sub> then <sup>a</sup>"),
            "",
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn tag_guard_survives_permissive_policy() {
        // The old early-return (preserve_urls=false, no code pledge) must
        // NOT disable the guard — hoisted above the policy gates.
        let u = unit(BlockKind::Paragraph, "x <br> y");
        let err = check_inline(
            &policy(Some(false), None),
            &u,
            &unit_result("x y"),
            "",
        )
        .unwrap_err();
        assert!(err.contains("raw inline HTML tag"), "got: {err}");
    }

    #[test]
    fn text_only_change_around_tags_passes() {
        let u = unit(BlockKind::Paragraph, "press <kbd>Ctrl</kbd> now");
        assert!(
            check_inline(
                &policy(None, None),
                &u,
                &unit_result("지금 <kbd>Ctrl</kbd> 누르세요"),
                ""
            )
            .is_ok()
        );
    }

    #[test]
    fn html_units_skip_the_inline_layer() {
        let u = unit(BlockKind::Html { block_type: 6 }, "[\"seg\"]");
        assert!(check_inline(&policy(None, None), &u, &unit_result("[\"번역\"]"), "").is_ok());
    }

    // Spec §4.3 known false-reject, pinned as accepted behavior: a tag
    // legally moved to start a line reclassifies the remainder as an HTML
    // block on the translated side only — the tokens vanish and the unit
    // rejects (verbatim retry usually recovers in the pipeline).
    #[test]
    fn line_initial_inline_tag_reclassification_false_rejects() {
        let u = unit(BlockKind::Paragraph, "text <b>bold</b> tail");
        let moved = "text tail\n<b>bold</b>";
        assert!(check_inline(&policy(None, None), &u, &unit_result(moved), "").is_err());
    }
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-core inline -- --test-threads=4` → the new tests FAIL (no guard yet).

- [ ] **Step 3: Implement the guard**

`inline.rs`:
1. `InlineInventory` gains `html_tokens: Vec<String>`; `inline_inventory`'s match gains `NodeValue::HtmlInline(literal) => inv.html_tokens.push(literal.clone()),`.
2. Restructure `check_inline`'s head — kind skips first, then UNCONDITIONAL parse + tag guard, then the policy gates:

```rust
    if matches!(
        unit.block_kind,
        BlockKind::CodeBlock { .. } | BlockKind::Html { .. }
    ) {
        // Fences carry no inline nodes; html payloads are JSON (the splice
        // check owns their structure). Skip both parses.
        return Ok(());
    }

    let enforce_dest = policy.preserve_urls != Some(false);
    let enforce_code = policy.preserve_code_identifiers == Some(true);

    let source = inline_inventory(&unit.source_payload, ref_defs);
    let translated = inline_inventory(&result.translated_payload, ref_defs);

    // Spec §4.3: always-on ordered raw-inline-HTML tag identity — hoisted
    // ABOVE the policy gates so no profile can disable it. A model has no
    // legitimate reason to alter a tag.
    let (s, t) = (&source.html_tokens, &translated.html_tokens);
    if s.len() != t.len() {
        return Err(format!(
            "raw inline HTML tag count changed: source has {}, translated has {}",
            s.len(),
            t.len()
        ));
    }
    for (i, (a, b)) in s.iter().zip(t.iter()).enumerate() {
        if a != b {
            return Err(format!(
                "raw inline HTML tag {i} changed: source {}, translated {}",
                clip(a),
                clip(b)
            ));
        }
    }

    if !enforce_dest && !enforce_code {
        return Ok(());
    }
```

(The rest of the function is unchanged. The old `!enforce_dest && !enforce_code` early-return moves BELOW the guard — delete the original.)

- [ ] **Step 4: Prompt mirror + golden regeneration**

`prompt.rs` — append to the BASE instruction (unconditional; before the policy-gated appends):

```rust
    instruction.push_str(
        " Raw inline HTML tags inside the text (e.g. <kbd>, <br>, <sup>) are operational \
         markup: reproduce every tag byte-for-byte, in its original order; translate only \
         the text around them.",
    );
```

This intentionally changes the two user-prompt goldens. Add a gated regen helper to `mod tests`:

```rust
    /// Regenerate the golden files. Run explicitly ONLY when a prompt
    /// change is intentional:
    /// `TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-core regen_prompt_goldens -- --ignored --test-threads=1`
    #[test]
    #[ignore]
    fn regen_prompt_goldens() {
        if std::env::var("TRANSYNC_REGEN_GOLDENS").as_deref() != Ok("1") {
            panic!("set TRANSYNC_REGEN_GOLDENS=1 to confirm intentional regeneration");
        }
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/llm/prompt/golden");
        std::fs::write(
            dir.join("user_prompt_first_dispatch.json"),
            build_user_prompt(&golden_fixture_batch()).unwrap(),
        )
        .unwrap();
        let mut retry_batch = golden_fixture_batch();
        retry_batch.units[0].retry = Some(RetryContext {
            attempt: 2,
            rejected_by: Some(ValidationLayer::PerKindShape),
            reason: Some("table column count changed".to_string()),
        });
        std::fs::write(
            dir.join("user_prompt_retry.json"),
            build_user_prompt(&retry_batch).unwrap(),
        )
        .unwrap();
    }
```

(Check `golden_fixture_batch`'s retry construction in the existing `golden_user_prompt_retry` test and mirror it EXACTLY so the regenerated retry golden matches what that test builds.)

Run the regen: `TRANSYNC_REGEN_GOLDENS=1 cargo test -p transync-core regen_prompt_goldens -- --ignored --test-threads=1`, then `cargo test -p transync-core golden -- --test-threads=4` → PASS. Inspect `git diff` of the two goldens: the ONLY change must be the new instruction sentence.

- [ ] **Step 5: Bump `VALIDATION_SCHEMA_VERSION`**

`validate.rs`: `pub const VALIDATION_SCHEMA_VERSION: u32 = 2;` and append to its doc-comment:

```rust
/// History:
/// - 2 (2026-08): HTML-content translation — new `html_segments` payload
///   semantics, the html prompt instruction, and the always-on inline
///   raw-HTML tag guard (spec 2026-08-03 §6 Cache row). html entries could
///   not pre-exist; the bump guards paragraph-family units against
///   pre-guard cached results. Currently costless: the shipped cache is
///   in-memory and hits are re-validated.
```

- [ ] **Step 6: Run everything** — `cargo test --workspace -- --test-threads=4`. The `wire_str_tests` array is untouched (no new ValidationLayer variant). Expected: PASS.

- [ ] **Step 7: Lint + commit**

```bash
cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings
git add crates/transync-core/src/validate/inline.rs crates/transync-core/src/validate.rs crates/transync-core/src/llm/prompt.rs crates/transync-core/src/llm/prompt/golden
git commit -m "core: always-on inline raw-HTML tag guard + prompt mirror + validation schema v2 (spec §4.3, §6)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: Regen — splice the accepted payload into out.md

**Files:**
- Modify: `crates/transync-core/src/regen.rs`

**Interfaces:**
- Consumes: `htmlseg::splice` (Task 3); accepted html payloads are wire-shaped JSON (Task 8).
- Produces: `regenerate` splices html blocks whose unit was accepted; fallback html blocks keep byte-verbatim source (existing None-arm). Layer 3 already proved the splice succeeds — a residual failure here degrades to source bytes (honest, never panics).

- [ ] **Step 1: Write the failing tests**

`regen.rs` has no test module — create one:

```rust
// Spec §3.3: regen splices the translated segments into the original
// markup; fallback html blocks stay byte-verbatim.
#[cfg(test)]
mod html_regen_tests {
    use super::*;
    use crate::FallbackStatus;
    use crate::id::assign_block_ids;
    use crate::parser::parse;
    use crate::validate::{ValidatedBatch, ValidatedUnit};

    fn validated(unit_id: &str, payload: Option<&str>, status: FallbackStatus) -> Vec<ValidatedBatch> {
        vec![ValidatedBatch {
            batch_id_str: "test".to_string(),
            units: vec![ValidatedUnit {
                unit_id: BlockId(unit_id.to_string()),
                final_status: status,
                accepted_payload: payload.map(str::to_string),
                rejected_by: None,
                rejection_reason: None,
                warnings: Vec::new(),
            }],
            batch_fault: None,
        }]
    }

    #[test]
    fn translated_html_block_is_spliced_into_out_md() {
        let src = "<details><summary>Click</summary></details>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let vb = validated("html-0001", Some("[\"펼치기\"]"), FallbackStatus::Translated);
        let (md, offsets) = regenerate(&doc, &vb);
        assert_eq!(md, "<details><summary>펼치기</summary></details>\n");
        assert!(offsets.0.contains_key(&BlockId("html-0001".to_string())));
    }

    #[test]
    fn preserved_html_block_round_trips_byte_identical() {
        let src = "<p>Caf&eacute;&nbsp;&copy;</p>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        // Preserved: the wire payload echoes the source segments — the
        // identity skip keeps entity forms byte-exact (spec §3.3).
        let segs = crate::htmlseg::extract("<p>Caf&eacute;&nbsp;&copy;</p>").expect("extracts");
        let payload = serde_json::to_string(&segs.texts).unwrap();
        let vb = validated("html-0001", Some(&payload), FallbackStatus::Preserved);
        let (md, _) = regenerate(&doc, &vb);
        assert_eq!(md, src, "byte-identical incl. entities");
    }

    #[test]
    fn fallback_html_block_splices_source_bytes_verbatim() {
        let src = "<div>original</div>\n";
        let mut doc = parse(src).expect("parses");
        assign_block_ids(&mut doc);
        let vb = validated("html-0001", None, FallbackStatus::FallbackSource);
        let (md, _) = regenerate(&doc, &vb);
        assert_eq!(md, src);
    }
}
```

(Check whether the html block's `source_range` includes the trailing newline — if the first test's expected string mismatches only on the trailing `\n` placement, the inter-block copy owns that byte; adjust the expected string to what the range math produces, keeping the load-bearing assertion: the summary text is replaced, the markup is untouched.)

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-core html_regen -- --test-threads=4` → FAIL (splice not wired; the JSON payload is spliced literally).

- [ ] **Step 3: Implement the dispatch arm**

In `regenerate`'s per-kind `match` (currently only `CodeBlock` is special):

```rust
        let to_write = match &block.kind {
            BlockKind::CodeBlock { info } if translated => {
                regenerate_code_block(&payload, info.as_deref())
            }
            BlockKind::Html { block_type } if translated => {
                let source_bytes = &doc.source_text[bstart..bend];
                // Validation layer 3 already proved this splice succeeds
                // (same routine, same inputs). If it still fails, splice
                // the source bytes — out.md stays honest, never corrupt.
                serde_json::from_str::<Vec<String>>(&payload)
                    .ok()
                    .and_then(|segs| {
                        crate::htmlseg::splice(source_bytes, &segs, *block_type).ok()
                    })
                    .unwrap_or_else(|| source_bytes.to_string())
            }
            _ => payload,
        };
```

- [ ] **Step 4: Run** — `cargo test -p transync-core html_regen -- --test-threads=4`, then the workspace. Expected: PASS (the full-reparse label arms from Task 4 make the spliced block label `"html"` on both sides).

- [ ] **Step 5: Lint + commit**

```bash
cargo fmt --all && cargo clippy -p transync-core --all-targets -- -D warnings
git add crates/transync-core/src/regen.rs
git commit -m "core: regen splices html units into out.md; fallback stays byte-verbatim (spec §3.3)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 11: Alignment schema 1.1.0 → 1.2.0 — the full lockstep drill

**Files:**
- Modify: `crates/transync-core/src/align.rs` (const + doc)
- Modify: `web/js/sync.js` AND `crates/transync-cli/web/sync.js` (`KNOWN_SCHEMA`, JSDoc `"1.1.0"` mention)
- Modify: 13 scenario files under `crates/transync/tests/scenarios/` (the `"1.1.0"` pins)
- Modify: `crates/transync-cli/tests/cli_smoke.rs` (the `"1.1.0"` pin)
- Modify: `web/tests/scn13.spec.js` (test g probe `"1.2.0"` → `"1.3.0"`, comments)

**Interfaces:**
- Produces: `ALIGNMENT_SCHEMA_VERSION = "1.2.0"`; `KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 }` in BOTH byte-identical sync.js copies.

- [ ] **Step 1: Bump the const**

`align.rs`: value → `"1.2.0"`; extend the doc-comment: `/// Bumped 1.1.0 → 1.2.0 for the additive "html" block_kind value (HTML-content translation, spec 2026-08-03 §5).`

- [ ] **Step 2: Sweep the pins**

```bash
grep -rn '"1\.1\.0"' crates/ web/
```

Update every hit that is a schema pin (scenario `assert_eq!(…schema_version, "1.1.0")` lines, `cli_smoke.rs`, sync.js `KNOWN_SCHEMA` + its comment). Do NOT touch historical mentions in docs/ (Task 16 owns docs). In BOTH sync.js copies: `const KNOWN_SCHEMA = { major: 1, minor: 2, patch: 0 };` and the JSDoc `// schema_version "1.2.0"`. Keep the two files byte-identical (edit one, `cp web/js/sync.js crates/transync-cli/web/sync.js`).

- [ ] **Step 3: Move the Playwright forward-drift probe**

`web/tests/scn13.spec.js` test g: `map.schema_version = "1.2.0"` → `"1.3.0"`, and the two assertions/comment mentions of `1.2.0` → `1.3.0` (the probe must stay NEWER than the engine).

- [ ] **Step 4: Run the gates**

```bash
cargo test --workspace -- --test-threads=4
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4
./scripts/test-browser.sh
```

Expected: all green, Playwright 7/7 (test h arrives in Task 14).

- [ ] **Step 5: Commit**

```bash
git add crates/transync-core/src/align.rs crates/transync/tests crates/transync-cli/tests/cli_smoke.rs web/js/sync.js crates/transync-cli/web/sync.js web/tests/scn13.spec.js
git commit -m "schema: alignment 1.2.0 — additive html block_kind, full lockstep (spec §5)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 12: sync.js — `<details>` toggle mirroring

**Files:**
- Modify: `web/js/sync.js` AND `crates/transync-cli/web/sync.js` (byte-identical pair)

**Interfaces:**
- Consumes: existing `mountSync` internals (`sourceById`/`targetById` maps, `destroy`).
- Produces (spec §5, decision 9): a `toggle` listener per pane (capture phase — the event does not bubble) that mirrors `open` onto the partner anchor's same-index `<details>`; unwired on `destroy()`.

- [ ] **Step 1: Implement (JS has no unit harness — Playwright covers it in Task 14; write the code, then eyeball with the manual demo if desired)**

Inside `mountSync`, after the pane wiring and before the controller/`destroy` assembly, add:

```js
  // Spec 2026-08-03 §5 (decision 9): mirror <details> toggle state across
  // panes so anchor geometry stays congruent and in-block progress mapping
  // keeps meaning. Interaction ownership, not structure. The toggle event
  // does not bubble — listen in the capture phase.
  function makeToggleMirror(pane, partnerById) {
    return (event) => {
      const details = event.target;
      if (!details || details.tagName !== "DETAILS") return;
      const anchor = details.closest("[data-sync-id]");
      if (!anchor || !pane.contains(anchor)) return;
      const partnerAnchor = partnerById.get(anchor.dataset.syncId);
      if (!partnerAnchor) return;
      const own = anchor.querySelectorAll("details");
      const twins = partnerAnchor.querySelectorAll("details");
      const index = Array.prototype.indexOf.call(own, details);
      const twin = index >= 0 ? twins[index] : undefined;
      if (twin && twin.open !== details.open) {
        // Assignment fires the partner's toggle; the equality guard above
        // makes the mirrored event a no-op — no ping-pong.
        twin.open = details.open;
      }
    };
  }
  const sourceToggle = makeToggleMirror(sourcePane, targetById);
  const targetToggle = makeToggleMirror(targetPane, sourceById);
  sourcePane.addEventListener("toggle", sourceToggle, true);
  targetPane.addEventListener("toggle", targetToggle, true);
```

and extend the existing `destroy` (find where scroll listeners are removed) with:

```js
    sourcePane.removeEventListener("toggle", sourceToggle, true);
    targetPane.removeEventListener("toggle", targetToggle, true);
```

(Read `mountSync` first: `sourceById`/`targetById` are the `Map<id, el>` built at mount — reuse them; if `destroy` lives in a returned object literal, capture the two handler refs in the enclosing scope.)

- [ ] **Step 2: Mirror the copies + drift test**

```bash
cp web/js/sync.js crates/transync-cli/web/sync.js
cargo test -p transync-cli sync_js -- --test-threads=4
```

Expected: drift test PASS.

- [ ] **Step 3: Browser suite still green** — `./scripts/test-browser.sh` → 7/7 (mirroring is additive; the fixture has no details until Task 13's fixture append lands — order within this plan already guarantees that; if Task 13 ran first, expect 8/8 wording instead).

- [ ] **Step 4: Commit**

```bash
git add web/js/sync.js crates/transync-cli/web/sync.js
git commit -m "web: mirror <details> toggle state across panes (spec §5, decision 9)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 13: End-to-end scenario SCN-15 + cache round-trip

**Files:**
- Create: `crates/transync/tests/fixtures/scn-15-html-blocks.md`
- Create: `crates/transync/tests/scenarios/scn_15_html_blocks.rs`
- Modify: `crates/transync/tests/scenarios.rs` (register the module)

**Interfaces:**
- Consumes: the whole pipeline via `transync::{translate, translate_with_cache}` + `MockTranslator` (`passthrough`, `tamper_payload`, `always_fails_unit`, `recording`).
- Produces: the spec §7 end-to-end coverage.

- [ ] **Step 1: Write the fixture** (`scn-15-html-blocks.md` — the REAL shapes from spec §7):

```markdown
# HTML blocks

Intro paragraph with inline <kbd>Ctrl</kbd> markup.

<details>
<summary>Click to expand</summary>

Hidden **markdown** body.

</details>

<div align="center">
<b>Hero banner</b>
</div>

<table>
  <tr><th>Name</th><th>Role</th></tr>
  <tr><td>Ada</td><td>Engineer</td></tr>
</table>

<!-- maintainer note: invisible -->

<pre>
line one

line three
</pre>

Closing paragraph.
```

Expected block inventory (verify in Step 3, adjust ids if the parse differs — the STRUCTURE below is the assertion, the exact ordinals follow the single counter): `h1-0001`, `p-0002` (kbd paragraph), `html-0003` (details open fragment, type 6), `p-0004` (hidden body), `html-0005` (`</details>` orphan-close fragment, zero segments), `html-0006` (center hero, type 6), `html-0007` (table, type 6), `html-0008` (comment, type 2, zero segments), `html-0009` (`<pre>`, type 1), `p-0010`.

- [ ] **Step 2: Write the scenario** (`scn_15_html_blocks.rs`):

```rust
//! SCN-15 — HTML-content translation end-to-end (spec 2026-08-03 §7).
//!
//! TRACE: SCN-15

use crate::common::mock_translator::MockTranslator;
use transync::{FallbackStatus, TranslateOptions, translate, translate_with_cache};

fn opts() -> TranslateOptions {
    TranslateOptions {
        target_language: "ko".to_string(),
        ..TranslateOptions::default()
    }
}

const SRC: &str = include_str!("../fixtures/scn-15-html-blocks.md");

#[tokio::test]
async fn smoke_scn_15() {
    let translator = MockTranslator::passthrough();
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");

    assert_eq!(output.alignment_map.schema_version, "1.2.0");

    // Every html-kind row is an anchor with block_kind "html".
    let html_rows: Vec<_> = output
        .alignment_map
        .blocks
        .iter()
        .filter(|r| r.block_kind == "html")
        .collect();
    assert!(html_rows.len() >= 5, "details-open, orphan-close, hero, table, comment, pre: {html_rows:?}");

    // Counting rule (spec §5): zero-segment rows preserved+uncounted.
    let zero_rows: Vec<_> = html_rows
        .iter()
        .filter(|r| r.fallback_status == FallbackStatus::Preserved)
        .collect();
    assert!(!zero_rows.is_empty(), "comment + orphan-close are zero-segment");
    let unit_rows = html_rows.len() - zero_rows.len();
    let s = &output.alignment_map.validation_summary;
    let non_html_units = output
        .alignment_map
        .blocks
        .iter()
        .filter(|r| r.block_kind != "html" && r.block_kind != "thematic-break" && r.block_kind != "image")
        .count();
    assert_eq!(
        s.total_units as usize,
        non_html_units + unit_rows,
        "unit-backed html blocks count; zero-segment ones do not"
    );

    // Passthrough == identity: out.md must be byte-identical (identity skip
    // keeps entity/markup bytes; spec §3.3).
    assert_eq!(output.translated_markdown, SRC, "identity round-trip");

    // Live render: real <details> in both panes, no data-skipped for it.
    for html in [&output.annotated_source_html, &output.annotated_target_html] {
        assert!(html.contains("<details>"), "live details:\n{html}");
        assert!(html.contains("<summary>Click to expand</summary>"), "live summary");
        assert!(!html.contains("data-skipped=\"html-block\""), "no placeholder on success path");
    }

    // Reader-honesty warnings for the zero-segment blocks.
    assert!(
        output
            .validation_report
            .skipped_source_nodes
            .iter()
            .any(|w| w.contains("no translatable text")),
        "zero-segment warning present: {:?}",
        output.validation_report.skipped_source_nodes
    );
}

#[tokio::test]
async fn translated_segments_are_spliced_and_markup_survives() {
    let translator = MockTranslator::tamper_payload("Click to expand", "펼치기");
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");
    assert!(
        output.translated_markdown.contains("<summary>펼치기</summary>"),
        "segment translated inside intact markup:\n{}",
        output.translated_markdown
    );
    assert!(
        output.translated_markdown.contains("<div align=\"center\">"),
        "markup preserved by construction"
    );
}

#[tokio::test]
async fn failed_html_unit_falls_back_to_escaped_placeholder() {
    // Find the details-open fragment's id first (structure-derived).
    let probe = MockTranslator::passthrough();
    let probe_out = translate(SRC, &opts(), &probe).await.expect("Ok");
    let details_id = probe_out
        .alignment_map
        .blocks
        .iter()
        .find(|r| r.block_kind == "html" && r.fallback_status == FallbackStatus::Translated)
        .map(|r| r.source_block_id.clone())
        .expect("a unit-backed html row exists");

    let translator = MockTranslator::always_fails_unit(details_id.clone());
    let output = translate(SRC, &opts(), &translator).await.expect("Ok");
    let row = output
        .alignment_map
        .blocks
        .iter()
        .find(|r| r.source_block_id == details_id)
        .expect("row");
    assert_eq!(row.fallback_status, FallbackStatus::FallbackSource);
    let needle = format!("data-sync-id=\"{details_id}\"");
    let target = &output.annotated_target_html;
    let pos = target.find(&needle).expect("anchor present");
    let window = &target[pos.saturating_sub(120)..(pos + 200).min(target.len())];
    assert!(
        window.contains("data-skipped=\"html-block\""),
        "fallback html renders the placeholder:\n{window}"
    );
    // out.md keeps the source bytes verbatim for the failed block.
    assert!(output.translated_markdown.contains("<summary>Click to expand</summary>"));
}

#[tokio::test]
async fn html_units_cache_and_replay_without_provider_calls() {
    // Spec §6 cache row: wire-shaped payloads cache; hits re-validate and
    // re-splice.
    let cache = transync::cache::InMemoryCache::default();
    let t1 = MockTranslator::recording();
    let first = translate_with_cache(SRC, &opts(), &t1, &cache).await.expect("Ok");
    let calls_first = t1.call_count();
    assert!(calls_first > 0);

    let t2 = MockTranslator::recording();
    let second = translate_with_cache(SRC, &opts(), &t2, &cache).await.expect("Ok");
    assert_eq!(t2.call_count(), 0, "full cache replay — html units included");
    assert_eq!(first.translated_markdown, second.translated_markdown);
}
```

(Check `InMemoryCache`'s constructor — if it is `new()` rather than `Default`, use that; check the facade path `transync::cache::InMemoryCache` via the re-export. If `MockTranslator::recording()`'s fingerprint differs per instance — `ProviderFingerprint::from_type_name` is type-based, so two instances share a fingerprint — the replay test is sound.)

- [ ] **Step 3: Register + run**

`scenarios.rs`: add `#[path = "scenarios/scn_15_html_blocks.rs"] mod scn_15_html_blocks;` in numeric order.

Run: `cargo test -p transync scn_15 -- --test-threads=4`
Expected: PASS. If the block-inventory expectation in Step 1 was off (e.g. comrak folds the `</details>` line differently), print the actual `blocks` (`dbg!`) once, fix the STRUCTURE comments and any id-dependent assertion, and re-run — the counting-rule and byte-identity assertions are the contract, not the ordinal guesses.

- [ ] **Step 4: Full workspace + commit**

```bash
cargo test --workspace -- --test-threads=4
git add crates/transync/tests
git commit -m "test: SCN-15 html-blocks end-to-end — splice, counting rule, fallback placeholder, cache replay (spec §7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 14: Browser coverage — fixture append, harness, Playwright test h

**Files:**
- Modify: `crates/transync/tests/fixtures/scn-14-full.md` (append at END only)
- Modify: `crates/transync/tests/scenarios/scn_14_full.rs` (new totals)
- Modify: `web/tests/support/harness.js` (`SYNC_IDS`)
- Modify: `web/tests/scn13.spec.js` (new test h)

**Interfaces:**
- Consumes: the CLI stub bundle built by `scripts/test-browser.sh` from `scn-14-full.md`; toggle mirroring (Task 12).
- Produces: Playwright coverage per spec §7 — live render, **no nested sync wrappers**, toggle mirroring.

- [ ] **Step 1: Append to `scn-14-full.md`** (END of file, so every existing id — `h1-0001` … `img-0014`, `DRIVER_BLOCK h2-0010` — stays stable):

```markdown

<details>
<summary>Bundled extras</summary>

Extra bundled paragraph.

</details>

<div align="center">
<b>Hero banner</b>
</div>
```

New blocks: `html-0015` (details open fragment), `p-0016`, `html-0017` (orphan close, zero-segment), `html-0018` (unclosed hero — the wrapper-swallowing shape auto-balancing must defuse).

- [ ] **Step 2: Update `scn_14_full.rs`**

Run `cargo test -p transync scn_14 -- --test-threads=4`, read each failure, and update the pinned totals: row count +4; `total_units` +3 (`html-0015`, `p-0016`, `html-0018` — the orphan-close `html-0017` is zero-segment/uncounted); any kind-sequence pin gains `["html", "paragraph", "html", "html"]` at the tail. Keep every existing assertion's intent; only the numbers/sequences grow.

- [ ] **Step 3: Update `harness.js`**

`SYNC_IDS` += `"html-0015", "p-0016", "html-0017", "html-0018"` (comment: appended html specimens; `hr-0009` still absent — non-sync).

- [ ] **Step 4: Write Playwright test h**

Append to `scn13.spec.js` inside the describe block:

```js
  test("h — html blocks live-render, never nest wrappers, and mirror details toggles", async ({ page }) => {
    const errors = collectPageErrors(page);
    await page.goto("/");
    await waitForMounted(page);

    // Live render: a real <details> element inside its anchor, in BOTH panes,
    // and no escaped placeholder for it (spec §5).
    for (const sel of ["#source", "#target"]) {
      const details = page.locator(`${sel} [data-sync-id="html-0015"] details`);
      await expect(details).toHaveCount(1);
      await expect(page.locator(`${sel} [data-sync-id="html-0015"][data-skipped]`)).toHaveCount(0);
    }

    // Spec §3.4: auto-balancing means no sync wrapper is ever swallowed
    // into another after the innerHTML mount — the unclosed hero
    // (html-0018) is exactly the shape that would do it.
    const nested = await page.evaluate(
      () => document.querySelectorAll("[data-sync-id] [data-sync-id]").length
    );
    expect(nested).toBe(0);

    // Decision 9: toggling the source details mirrors to the target.
    await page.locator('#source [data-sync-id="html-0015"] summary').click();
    await advanceFrames(page, 4);
    const targetOpen = await page.evaluate(() => {
      const d = document.querySelector('#target [data-sync-id="html-0015"] details');
      return d ? d.open : null;
    });
    const sourceOpen = await page.evaluate(() => {
      const d = document.querySelector('#source [data-sync-id="html-0015"] details');
      return d ? d.open : null;
    });
    expect(sourceOpen).not.toBeNull();
    expect(targetOpen).toBe(sourceOpen);

    expect(errors).toEqual([]);
  });
```

(Import note: `expect` is already imported at the top of the spec; `advanceFrames` comes from the harness import — extend the existing import list if it isn't there.)

- [ ] **Step 5: Run the browser suite**

Run: `./scripts/test-browser.sh`
Expected: **8/8**, twice in a row (`./scripts/test-browser.sh && ./scripts/test-browser.sh`) to guard against flake. Also re-run `cargo test -p transync-cli --features test-stub-provider -- --test-threads=4` (the CLI smoke consumes scn-14).

- [ ] **Step 6: Commit**

```bash
git add crates/transync/tests/fixtures/scn-14-full.md crates/transync/tests/scenarios/scn_14_full.rs web/tests
git commit -m "test: browser coverage for html blocks — live render, wrapper integrity, toggle mirror (spec §7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 15: Records (spec §8)

**Files:**
- Create: `docs/decisions/0018-html-content-translation-via-segment-extraction.md`
- Create: `docs/project/design-change-records/DCR-0016.md` (follow the existing DCR file-naming in that dir — check `ls`)
- Modify: `CLAUDE.md` (invariant 7), `docs/architecture/contracts.md` (§3 alignment rows + §4 DOM attrs: the `html` kind), `docs/architecture/mvp-scope.md`, `docs/index.md` (link the two new records), `docs/project/status.md`, `docs/project/phase-state.yaml`, `CHANGELOG.md`, `docs/project/open-issues.md` (OI-0028 note — ONLY if Task 1's wasm canary passed)

**Interfaces:** none — documentation. Content requirements:

- [ ] **Step 1: ADR-0018** — records: HTML content translatable via app-owned segment extraction (owner decisions §2 of the spec, all nine); amends invariant 7; live render + fallback placeholder + render-side fragment auto-balancing; sequencing note (lands inside the 0.2.0 window). Follow the existing ADR template (`ls docs/decisions/` and copy the structure of 0017).
- [ ] **Step 2: DCR-0016** — supersedes DCR-0013's html-block posture (placeholder becomes failure-only); alignment schema 1.2.0; `VALIDATION_SCHEMA_VERSION` 2 with the spec §6 justification; the lockstep checklist actually performed (Task 11 list). Follow DCR-0015's structure.
- [ ] **Step 3: CLAUDE.md invariant 7** — replace the "Raw HTML in v1 is disabled / escaped / rejected" sentence with: "Raw HTML blocks are translatable, structurally-owned content: the application extracts text segments (lol_html), the LLM sees only text, splice-back preserves markup by construction, and live rendering rides the shells' DOMPurify fail-closed mount. The escaped placeholder is the *failure* presentation (extraction failure / fallback). Inline raw-HTML tags are guarded verbatim." Cite ADR-0018.
- [ ] **Step 4: contracts.md** — §3: `block_kind` value table gains `html` (counted when unit-backed; zero-segment/extraction-failed rows uncounted, statuses per spec §3.2); schema 1.2.0 row; §4: the html live wrapper (`div` + standard attr set) and the fallback `data-skipped="html-block"` reuse. §1: note the `html_segments` input mode + the always-on inline tag guard mirror.
- [ ] **Step 5: mvp-scope / status / phase-state / CHANGELOG** — mark the HTML-content translation feature landed (post-MVP feature wave), CHANGELOG `[Unreleased]` entry listing: BlockKind::Html, htmlseg engine, segment units, validation layers, schema 1.2.0, toggle mirroring, VALIDATION_SCHEMA_VERSION 2, new deps.
- [ ] **Step 6: OI-0028 note** — if (and only if) Task 1's canary PASSED: add the dated note to OI-0028's ledger entry: "lol_html <ver> + htmlize <ver> verified on wasm32-unknown-unknown (2026-08-03 canary) — the base-crate split's dependency set stays wasm-clean."
- [ ] **Step 7: Drift gates + commit**

```bash
cargo test -p transync docs_index_drift -- --test-threads=4
git add CLAUDE.md docs CHANGELOG.md
git commit -m "docs: HTML-content translation records — ADR-0018, DCR-0016, invariant 7 amendment (spec §8)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 16: Full gate

- [ ] **Step 1: The complete verification stack, in order**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --all-features -- -D warnings
cargo test --workspace -- --test-threads=4
cargo test -p transync-cli --features test-stub-provider -- --test-threads=4
./scripts/test-browser.sh && ./scripts/test-browser.sh
```

Expected: everything green, Playwright 8/8 twice.

- [ ] **Step 2: Spec-conformance sweep** — re-read the spec's §6 table and §7 list top to bottom; for each row/bullet name the test that covers it (they were all placed by Tasks 2–14). Any uncovered row is a missing test — add it now in the style of its task.

- [ ] **Step 3: Working-tree check** — `git status` clean except intentionally-untracked files; every commit passed the pre-commit hook.

- [ ] **Step 4: Report** — summarize: commits, verification results, the wasm-canary verdict, and any deviation from this plan (each deviation needs a one-line justification traceable to the spec).

---

## Execution notes

- Tasks are strictly ordered; do not parallelize file-overlapping tasks (Tasks 2/3 share `htmlseg.rs`; 4–10 ripple through core).
- If a step's expected state disagrees with reality (an API name, a line anchor, a fixture id), READ the file and adapt the mechanics — the spec's behavior contract is the invariant, not this plan's line guesses. Record every such adaptation in the task report.
- Fable-model subagents: max 2 concurrent (owner policy). Implementation subagents: model `opus` (owner memory).

