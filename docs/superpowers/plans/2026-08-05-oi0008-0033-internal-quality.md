# OI-0008 + OI-0033 Internal-Quality Wave Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the lone-CR line-table desync (OI-0033, Support posture) and retire OI-0008's whole refactor-debt cluster — pure retry/fallback policy module, run_pipeline phase split, unit/client/CLI concern splits, full parser walker rework — with a behavior-change budget of exactly two.

**Architecture:** Approved spec: `docs/superpowers/specs/2026-08-05-oi0008-0033-internal-quality-design.md` (each task cites its section). Line-precise structure maps from the exploration live in `/Volumes/Temp/claude/oi0008-0033-context/{retryPolicy,concernMixing,oi0033}.md` — implementers SHOULD read the digest named in their task; it is the map, the code is the territory. Order: bug fix first (T1–T2), then per-site splits, each site's tasks adjacent (T3–T4 pipeline, T5–T6 unit, T7–T8 client, T9 CLI, T10–T11 parser), records last (T12). The tree is green at every commit.

**Tech Stack:** Rust workspace (5 crates), comrak 0.27, file-as-module layout (never `mod.rs`).

## Global Constraints

- **Behavior-change budget is exactly two** (spec §0): lone-CR support and the `total_retries` semantics fix. Everything else is behavior-preserving motion — if a task finds it must change observable behavior, STOP and report instead.
- The facade surface is FROZEN: `crates/transync/tests/public_surface.rs` must stay green **unmodified**; `contracts.md` §0, alignment schema 1.2.0, `VALIDATION_SCHEMA_VERSION` 2, `CacheKey`, both `sync.js` copies untouched.
- `crates/transync-syntax`: no `[features]` table, no `transync-core` dependency (dev-dependencies included) — the standing wasm gate.
- IR freeze rule (spec §7): beyond deleting `Document::hierarchy` and `Block::parent_id`, the shapes of `Document`/`Block`/`Section` do not change.
- Zero test removals — with ONE sanctioned exception, Task 10's hierarchy re-key test swap (equivalent-strength replacement, named in that task). Inline `#[cfg(test)]` tests move WITH their code. The suite may only grow (baseline 340 passed / 0 failed / 3 ignored).
- `unit::build_batches` and `unit::html_outcomes` keep their exact paths (transync-openai dev-dep test).
- Run tests as `cargo test --workspace -- --test-threads=4`; never raise the cap. NEVER set/override `CARGO_TARGET_DIR` or pass `--target-dir`. Temp files under `/Volumes/Temp/claude/` only.
- No pure-formatting edits; `cargo fmt` before each commit. No line-number references in long-lived docs.
- Each task ends: `cargo fmt` → `cargo clippy --all-targets -- -D warnings` → `cargo test --workspace -- --test-threads=4` → commit (the pre-commit hook re-runs gates + the wasm check; never `--no-verify`).
- Commit messages end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

---

### Task 1: CR-aware `LineOffsets` + shared cap helper (OI-0033 core fix)

Spec §1. Digest: `/Volumes/Temp/claude/oi0008-0033-context/oi0033.md` (§1, §5 carry the exact algorithm and probe evidence). TDD.

**Files:**
- Modify: `crates/transync-syntax/src/parser/ranges.rs`

**Interfaces:**
- Produces: CR-aware `LineOffsets::new`; a private terminator-aware cap helper used by both `pos_to_byte` and `byte_range_for`. No signature changes. Task 2 builds integration tests on this fix.

- [ ] **Step 1: Write the failing tests** — new `#[cfg(test)] mod tests` at the bottom of `ranges.rs` (the file has none). Inline `&str` fixtures ONLY (a checked-in CR file can be silently rewritten by editors/autocrlf — spec §1):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const LONE_CR: &str = "# Title\r\rpara one\r\r- a\r- b\r";
    const CRLF: &str = "# Title\r\n\r\npara one\r\n\r\n- a\r\n- b\r\n";
    const LF: &str = "# Title\n\npara one\n\n- a\n- b\n";

    #[test]
    fn lone_cr_lines_are_boundaries() {
        let lo = LineOffsets::new(LONE_CR);
        // comrak sees 6 content lines; the table must have an entry per line
        // (probe: offsets [0, 8, 9, 18, 19, 23, 27]).
        assert_eq!(lo.pos_to_byte(3, 1), 9, "line 3 starts at 'para one'");
        assert_eq!(&LONE_CR[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1], "para one");
        assert_eq!(&LONE_CR[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 3) + 1], "- a");
        assert_eq!(&LONE_CR[lo.pos_to_byte(6, 1)..lo.pos_to_byte(6, 3) + 1], "- b");
    }

    #[test]
    fn crlf_offsets_are_unchanged_by_the_cr_arm() {
        // CRLF is one terminator: the \r arm must NOT fire before a \n.
        let lo = LineOffsets::new(CRLF);
        assert_eq!(&CRLF[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1], "para one");
        assert_eq!(&CRLF[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 3) + 1], "- a");
    }

    #[test]
    fn lf_offsets_are_unchanged() {
        let lo = LineOffsets::new(LF);
        assert_eq!(&LF[lo.pos_to_byte(3, 1)..lo.pos_to_byte(3, 8) + 1], "para one");
    }

    #[test]
    fn mixed_cr_lf_does_not_swallow_the_next_block() {
        let src = "# Title\n\npara one\r\rsecond para\n\n- a\n- b\n";
        let lo = LineOffsets::new(src);
        // Pre-fix, line 5 resolved into '- a' (the item-swallow). Post-fix it
        // must resolve to 'second para' (probe: CR-aware range 19..30).
        assert_eq!(&src[lo.pos_to_byte(5, 1)..lo.pos_to_byte(5, 11) + 1], "second para");
    }

    #[test]
    fn lone_cr_at_eof_is_a_boundary() {
        let lo = LineOffsets::new("just one line\r");
        assert_eq!(lo.pos_to_byte(1, 1), 0);
        assert_eq!(lo.pos_to_byte(2, 1), 14, "line 2 starts at EOF");
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test -p transync-syntax ranges -- --test-threads=4`. Expected: `lone_cr_lines_are_boundaries`, `mixed_cr_lf_does_not_swallow_the_next_block`, `lone_cr_at_eof_is_a_boundary` FAIL (wrong offsets); the LF/CRLF controls PASS.

- [ ] **Step 3: Implement the CR arm** in `LineOffsets::new` — comrak's rule and NOTHING more (not `str::lines()`, not Unicode line-break classes — probe-verified: VT/FF/NEL/U+2028/U+2029/NUL are not boundaries):

```rust
for (i, b) in bytes.iter().enumerate() {
    match *b {
        b'\n' => offsets.push(i + 1),
        b'\r' if bytes.get(i + 1) != Some(&b'\n') => offsets.push(i + 1),
        _ => {}
    }
}
```

Update the struct's doc comment to name the authority: comrak `strings::is_line_end_char` (`\n` or `\r`; `\r\n` consumed as one terminator).

- [ ] **Step 4: Extract the shared, terminator-aware cap helper.** `pos_to_byte` and `byte_range_for` currently derive `line_end_cap` independently (`next_start - 1` — assumes a 1-byte terminator, leaving a CRLF's `\r` inside the cap). Add one private helper on `LineOffsets` that returns the content-end (offset of the terminator's FIRST byte) for a line — for a line ending `\r\n` subtract 2, else 1, clamped for the final unterminated line — and make both call sites use it. This needs the source bytes; pass `&str`/`&[u8]` where the current code paths already have it (both call sites do). Behavior on LF/lone-CR lines is identical to today; the CRLF cap tightens by one currently-unreachable byte (probe: zero block-level over-column hits — still, add one direct unit test for the helper on a CRLF line).

- [ ] **Step 5: Run to green** — same command; all new tests PASS.

- [ ] **Step 6: Gates + commit**

```bash
git add crates/transync-syntax/src/parser/ranges.rs
git commit -m "fix: LineOffsets counts lone CR as a line boundary, matching comrak (OI-0033)"
```

---

### Task 2: OI-0033 integration — parse assertion, list-item guard, Guard-1 closure, doc amendments

Spec §1 (rest). Digest: `oi0033.md` §3, §6, §7.

**Files:**
- Modify: `crates/transync-syntax/src/parser.rs` (parse-level test + emit-time guard)
- Modify: `crates/transync-core/src/unit.rs` (Guard-1 closure test)
- Modify: `docs/project/design-change-records/archive/DCR-0017-transync-syntax-split.md` (residual note), `crates/transync-core/src/validate/full_reparse.rs` (comment)

**Interfaces:**
- Consumes: Task 1's fix.
- Produces: a parse warning `"list item has an empty source range at a non-degenerate position"`-class message (exact wording implementer's choice, must name the block id); the Guard-1 closure test that Task 6 must carry along when it moves `constraints_for`.

- [ ] **Step 1: Parse-level assertion** — in `parser.rs`'s existing inline-test style, add a test module (or extend one) asserting that parsing `"intro para\r\r- alpha\r- bravo\r- charlie\r\rtail para\r"` yields blocks whose `source_range`s are all non-empty, non-overlapping, and in increasing order, and whose payload slices equal `"intro para"`, `"- alpha"`, `"- bravo"`, `"- charlie"`, `"tail para"` respectively (probe `oi0033.md` §2 "CR-only list only" carries the expected offsets).

- [ ] **Step 2: Emit-time defensive guard** — in the parser's emit path: when the block being emitted is a list item (`BlockKind::ListItem { .. }`) AND its comrak sourcepos is non-degenerate (start != end) AND the computed `source_range` is empty, push a warning onto the document's warnings naming the block id. Scope STRICTLY to list items — SCN-15 legitimately produces an empty html-block range and must not warn (add a test: parsing the SCN-15-style comment fixture produces no new warning). Add a positive test that the guard fires by feeding a deliberately corrupted `LineOffsets`… if that is not constructible without test-only seams, assert the negative cases only and document the guard inline (do NOT add test-only public seams for it).

- [ ] **Step 3: Guard-1 residual (a) closure test** — in `crates/transync-core/src/unit.rs`'s test module:

```rust
#[test]
fn lone_cr_list_items_carry_list_topology_constraints() {
    // OI-0033 / DCR-0017 Guard-1 residual (a): the ONLY known route to
    // expected_list_topology == None was a mis-sliced (empty) payload from
    // the lone-CR desync. With CR-aware offsets the route must be closed.
    let src = "intro para\r\r- alpha\r- bravo\r- charlie\r\rtail para\r";
    let mut doc = crate::parser::parse(src).expect("parses");
    crate::id::assign_block_ids(&mut doc);
    let outcomes = crate::unit::html_outcomes(&doc);
    let opts = crate::TranslateOptions::default();
    let batches = crate::unit::build_batches(&doc, &opts, None, &outcomes);
    let li_units: Vec<_> = batches
        .iter()
        .flat_map(|b| &b.units)
        .filter(|u| matches!(u.block_kind, crate::id::BlockKind::ListItem { .. }))
        .collect();
    assert!(!li_units.is_empty(), "fixture must yield list-item units");
    for u in &li_units {
        assert!(!u.source_payload.is_empty(), "{} payload must be non-empty", u.unit_id);
        assert!(
            u.constraints.expected_list_topology.is_some(),
            "{} must carry Some(list topology) — residual (a) is void",
            u.unit_id
        );
    }
}
```

(Adjust paths/field names to the real ones in the file — `BlockKind::ListItem`'s actual variant shape, the real `assign_block_ids` flow that `build_batches` expects; mirror how the file's existing tests construct a parsed document.)

- [ ] **Step 4: Amend the two residual notes** — after Step 3 is green: DCR-0017's residual paragraph (the one ending "…the only route that could revive it is the lone-CR sourcepos defect filed as OI-0033") gains a dated note that OI-0033 is fixed (Support) and residual (a) is now void unqualified; the mirroring comment block in `validate/full_reparse.rs` is updated the same way. No line-number references in the DCR.

- [ ] **Step 5: Gates + commit**

```bash
git add crates/transync-syntax/src/parser.rs crates/transync-core/src/unit.rs crates/transync-core/src/validate/full_reparse.rs docs/project/design-change-records/archive/DCR-0017-transync-syntax-split.md
git commit -m "test: lone-CR parse pins + list-item empty-payload guard; Guard-1 residual (a) closed (OI-0033)"
```

---

### Task 3: Pure retry/fallback policy module + `total_retries` fix

Spec §2 (approach A1). Digest: `retryPolicy.md` — READ IT FIRST; its decision-point table (D1–D5l) is the extraction map.

**Files:**
- Create: `crates/transync-core/src/pipeline/policy.rs`
- Modify: `crates/transync-core/src/pipeline.rs` (`process_one_batch` rewired; `build_validation_report` semantics fix; `pub(crate) mod policy;` declaration)

**Interfaces:**
- Produces (Task 4 moves these with the code, names must hold):

```rust
pub(crate) struct BatchPolicy { /* owns: unit_fault_counter, batch_fault_rounds,
    provider_attempts, budgets copied from &TranslateOptions */ }

pub(crate) enum UnitDisposition {
    Accept,                 // cache-put eligibility decided by the caller from final_status
    RetryUnit { attempt: u32 },
    RedispatchOffender { attempt: u32 },
    FinalizeFallback,
}

impl BatchPolicy {
    pub(crate) fn new(opts: &TranslateOptions) -> Self;
    /// D5e: charge a batch-fault round (once per round); true = redispatch offenders.
    pub(crate) fn admit_batch_fault_round(&mut self, has_offenders: bool) -> bool;
    /// D5h/D5i/D5j collapsed: given (is_offender, rejected_by.is_some()), decide.
    pub(crate) fn unit_disposition(&mut self, unit_id: &BlockId, is_offender: bool, validation_failed: bool) -> UnitDisposition;
    /// D5a: transient provider error -> Some(backoff) while budget remains, None = give up.
    pub(crate) fn on_transient_error(&mut self, retry_after: Option<Duration>) -> Option<Duration>;
    /// Documented termination bound (kept as a debug assertion hook + doc anchor).
    pub(crate) fn max_rounds(&self, unit_count: usize) -> u32;
}

/// Pure backoff: Retry-After capped at 30s, else 200ms << min(attempts-1, 5) capped 5s.
pub(crate) fn backoff_delay(attempts: u32, retry_after: Option<Duration>) -> Duration;
```

- [ ] **Step 1: Write the policy unit tests FIRST** (in `policy.rs`'s own `#[cfg(test)]`), pinning: per-unit budget exhaustion (default 2 retries → 3rd fault = FinalizeFallback); batch-fault rounds charged once per round and capped by `max_per_batch_schema_retries`; an offender's disposition does not consume the unit's content budget; `backoff_delay(1, None) == 200ms`, `backoff_delay(6, None)` capped at 5 s, `backoff_delay(_, Some(60s))` capped at 30 s; `on_transient_error` returns None once `max_per_batch_provider_retries` is exhausted. Run: FAIL (module absent).

- [ ] **Step 2: Implement `policy.rs`** by LIFTING the logic out of `process_one_batch` (the digest's table names each source region: `unit_fault_counter`, `batch_fault_rounds` admission, the D5h/D5i/D5j arms, `translate_with_provider_retries`'s backoff math). The behavior must be identical — this is motion + encapsulation, not redesign. Run Step 1's tests to green.

- [ ] **Step 3: Rewire `process_one_batch`** to consult `BatchPolicy` at every decision point, deleting the now-lifted inline counters/branches. I/O (provider call, cache get/put/evict, sleep on the returned `Duration`, attempt-row emission, logging) stays in the orchestrator. `retry_validation_unit` keeps being called for both retry shapes with the attempt number the policy returns.

- [ ] **Step 4: `total_retries` semantics fix** (behavior change #2 of the budget). In `build_validation_report`, replace the `attempts.len() - 1` computation with a count of CONTENT retries only: attempt rows that (a) are not the unit's first dispatch, (b) are not batch-fault redispatches (`batch_fault: true` rows), and (c) are not the cache-hit re-validation row (`attempt_number: 0`). Add report-level tests pinning: a run with one batch-fault round and zero content retries reports `total_retries == 0` and `batch_schema_faults == 1`; a rejected-cache-hit re-run reports `total_retries == 0`; one genuine per-unit retry reports `total_retries == 1`. (Follow the existing in-file test style for constructing attempt rows / running the mock pipeline — `pipeline.rs`'s test module has precedents.)

- [ ] **Step 5: Full-suite sanity** — the ONLY tests allowed to change expectations are ones that pinned the old conflated `total_retries` (the digest says none exist; if one surfaces, report it in your notes rather than silently editing). Run the workspace suite.

- [ ] **Step 6: Commit**

```bash
git add crates/transync-core/src/pipeline.rs crates/transync-core/src/pipeline/policy.rs
git commit -m "refactor: retry/fallback policy extracted to a pure module; total_retries counts content retries only (OI-0008 R0001-0057)"
```

---

### Task 4: `run_pipeline` phase split (pure motion)

Spec §3. Digest: `retryPolicy.md` §1 (the run-level map names each phase's span).

**Files:**
- Create: `crates/transync-core/src/pipeline/dispatch.rs`, `crates/transync-core/src/pipeline/finalize.rs`, `crates/transync-core/src/pipeline/report.rs`
- Modify: `crates/transync-core/src/pipeline.rs` (shrinks to `run_pipeline` orchestration + preflights + module declarations)

**Interfaces:**
- Consumes: Task 3's `policy.rs` (moves under `dispatch`'s consumption unchanged).
- Produces: `pub(crate)` phase functions with the SAME signatures the moved code has today (`process_one_batch` → `dispatch.rs`; `finalize_regen_with_reparse_policy`, `widen_to_neighbors`, `downgrade_units`, `fall_back_all` → `finalize.rs`; `build_validation_report` + alignment assembly → `report.rs`). `run_pipeline`'s own signature and behavior byte-stable.

- [ ] **Step 1: Move dispatch** — `process_one_batch` + its direct private helpers (cache partition/re-validation, batch-id identity check, retry-batch construction, `translate_with_provider_retries`) into `dispatch.rs` with their `#[cfg(test)]` tests. `pipeline.rs` gains `pub(crate) mod dispatch;` and calls through. Compile + focused tests green.
- [ ] **Step 2: Move finalize** — the DCR-0004 cascade functions + their tests into `finalize.rs`. Green.
- [ ] **Step 3: Move report** — `build_validation_report` (with Task 3's new semantics + tests) + alignment-map assembly into `report.rs`. Green.
- [ ] **Step 4: Verify pure motion** — `git diff --stat` shows pipeline.rs shrinking by ≈ what the three new files gain (± mod/imports); grep confirms zero behavior keywords changed: no test expectation edits anywhere in the diff. Full suite green; count unchanged from Task 3's.
- [ ] **Step 5: Commit**

```bash
git add crates/transync-core/src/pipeline.rs crates/transync-core/src/pipeline/
git commit -m "refactor: run_pipeline split into dispatch/finalize/report phases (OI-0008 R0001-0076)"
```

---

### Task 5: Structure-oracle move out of `unit.rs`

Spec §4 (first bullet). Digest: `concernMixing.md` §2.

**Files:**
- Create: `crates/transync-core/src/structure.rs`, `crates/transync-core/src/structure/labels.rs`
- Modify: `crates/transync-core/src/unit.rs`, `crates/transync-core/src/validate/per_kind.rs` (imports), `crates/transync-core/src/lib.rs` (`pub(crate) mod structure;`)

**Interfaces:**
- Produces: `crate::structure::{ListFacts, inspect_list_topology, inspect_blockquote_children, inspect_table}` and `crate::structure::labels::{block_node_kind_label, item_child_kinds, blockquote_child_label}` — same signatures as today's `unit.rs` items. Task 6 depends on `unit.rs` already being free of them.

- [ ] **Step 1: Move** the fingerprint subsystem (≈255 lines: `ListFacts` + the three `inspect_*` fns + the three label fns) byte-preserving into the two new files; label fns into `labels.rs`.
- [ ] **Step 2: Repoint importers** — `unit.rs` (constraints_for) and `validate/per_kind.rs` (both import sites, src + tests) to `crate::structure::…`. The validator no longer imports from the unit builder.
- [ ] **Step 3: Tests travel** — any moved fn's tests move to the new files; per_kind's own tests stay put. Full suite green, count unchanged.
- [ ] **Step 4: Commit**

```bash
git add crates/transync-core/src/unit.rs crates/transync-core/src/structure.rs crates/transync-core/src/structure/labels.rs crates/transync-core/src/validate/per_kind.rs crates/transync-core/src/lib.rs
git commit -m "refactor: structural-fingerprint oracle moves to crate::structure (OI-0008 R0001-0077)"
```

---

### Task 6: `unit.rs` payload/budget split

Spec §4 (rest). Digest: `concernMixing.md` §2.

**Files:**
- Create: `crates/transync-core/src/unit/payload.rs`, `crates/transync-core/src/unit/budget.rs`
- Modify: `crates/transync-core/src/unit.rs`

**Interfaces:**
- Produces: `payload::assemble(doc, block, block_index, outcomes) -> (String, InputMode, BlockConstraints)` (absorbing `input_mode_for`, `constraints_for`, and the html arm — exact signature may add what the html arm needs; keep it minimal); `budget::resolve(opts, profile) -> BatchBudget` with ONE `caller_wins(caller_value, default_value, profile_value)` sentinel helper replacing the three inline copies.
- Constraints: `unit::build_batches` / `unit::html_outcomes` paths PRESERVED (dev-dep test); the empty-document early return stays BEFORE profile/budget resolution, now with a comment naming that ordering deliberate (a malformed profile must not surface on an empty document). Task 2's Guard-1 closure test and all existing unit tests move only if their subject moved.

- [ ] **Step 1: Extract `payload.rs`** (the two-arm branch + `input_mode_for` + `constraints_for`), moving the html-arm tests along. Green.
- [ ] **Step 2: Extract `budget.rs`** with the single sentinel helper; add one direct unit test per resolution rule (input tokens, unit cap, output cap) exercising caller-wins / profile-wins / default. Green — including the two batch-emptiness↔predicate agreement tests and the profile-side pins in `profile.rs` and `translate_cmd.rs`, all untouched.
- [ ] **Step 3: `build_batches` shrinks to orchestration** (~90 lines target). Full suite green.
- [ ] **Step 4: Commit**

```bash
git add crates/transync-core/src/unit.rs crates/transync-core/src/unit/payload.rs crates/transync-core/src/unit/budget.rs
git commit -m "refactor: unit construction split into payload strategy + budget resolution (OI-0008 R0001-0077)"
```

---

### Task 7: Client classify tests-first + transport/classify/endpoint/tokenizer split + dead-code deletion

Spec §5 (first half). Digest: `concernMixing.md` §3 — note its correction: the client owns CLASSIFICATION, not retry policy; the table must not change behavior (Task 3's policy consumes it).

**Files:**
- Create: `crates/transync-openai/src/client/transport.rs`, `crates/transync-openai/src/client/classify.rs`, `crates/transync-openai/src/client/endpoint.rs`, `crates/transync-openai/src/tokenizer.rs`
- Modify: `crates/transync-openai/src/client.rs`, `crates/transync-openai/src/lib.rs` (tokenizer import site)
- Delete: `crates/transync-openai/src/pagination.rs` (+ its `pub mod` line), the `_unused_for_typing` stub

**Interfaces:**
- Produces: `classify::provider_error_for_status(status, body_excerpt) -> ProviderError` (or the equivalent shape the current inline code has — extract, don't redesign), `classify::parse_retry_after`, `classify::truncate_diagnostic`, `classify::oversize_response_error`; `transport::post_json` reduced to send + size-cap + accumulate, taking the classifier as a plain function call; `endpoint::build_endpoint`; `tokenizer::tokenizer_hint_for_model`. Task 8 builds on this layout.

- [ ] **Step 1: TESTS FIRST for the classification table** (currently unpinned — the one uncovered piece): in `client.rs`'s test module, pin 408/409/425 → the retryable Transport variant, 429 (+`Retry-After` header parse) → `RateLimitedAfter`, 401/403 → auth, other 4xx → terminal `Other`, oversize → terminal; plus `parse_retry_after` on seconds-form, malformed, and absent inputs. Construct via the smallest seam the current code allows (if the classifier is only reachable through `post_json`, extract the pure classification fn FIRST with no other change, then test it — that extraction is Step 1's enabling move, allowed because behavior is pinned by the very tests being added). Green.
- [ ] **Step 2: Split** transport/classify/endpoint into the new files; move `tokenizer_hint_for_model` to `tokenizer.rs` (fix the `lib.rs` call site). Tests travel with their code (the 20 existing offline tests + Step 1's new ones).
- [ ] **Step 3: Delete dead surface** — `pagination.rs` (zero workspace callers; spec §5 records the crate-surface rationale) and `_unused_for_typing` (keep `BatchId` in the import set only if still genuinely used; otherwise drop the import too).
- [ ] **Step 4: Full suite green; commit**

```bash
git add crates/transync-openai/src/
git commit -m "refactor: client transport/classify/endpoint/tokenizer split; classification table pinned; dead pagination stub deleted (OI-0008 R0001-0079)"
```

---

### Task 8: Client surface unification (chat/responses + one flow)

Spec §5 (second half). Digest: `concernMixing.md` §3.

**Files:**
- Create: `crates/transync-openai/src/client/dispatch.rs`, `crates/transync-openai/src/client/chat.rs`, `crates/transync-openai/src/client/responses.rs`
- Modify: `crates/transync-openai/src/client.rs`

**Interfaces:**
- Consumes: Task 7's transport/classify/endpoint layout.
- Produces: `dispatch::{Api, api_for_model, api_from_env_or_model}` moved intact; per-surface modules each owning their DTOs + `translation_body(..)` + `extraction_body(..)` + `extract_output(..)`; `client.rs` keeps `call_api` and `call_glossary_extraction` as TWO thin flows over ONE shared shape (match on `Api`, call the surface module's three fns, one shared transport/error path) — the extraction flow's current inline duplication of endpoint+transport+error-mapping disappears.

- [ ] **Step 1: Move** the Chat block (DTOs, both body builders, `extract_chat_output`) to `chat.rs`; the Responses block to `responses.rs`; the `Api` dispatch trio to `dispatch.rs`. Tests travel. Green.
- [ ] **Step 2: Unify the flows** — rewrite `call_api` and `call_glossary_extraction` to share one request-shape: resolve surface → build body via the surface module → `transport::post_json` → surface `extract_output` / extraction parse. The ~30 duplicated lines in the extraction `match` die. All 20+ request-shape/envelope tests must pass UNCHANGED — they are the byte-level pin that unification didn't alter any wire body.
- [ ] **Step 3: Full suite green; commit**

```bash
git add crates/transync-openai/src/
git commit -m "refactor: one surface abstraction for chat/responses across translation + extraction (OI-0008 R0001-0079)"
```

---

### Task 9: CLI `translate_cmd` split + `execute()` seam

Spec §6. Digest: `concernMixing.md` §4 (the seam map + the note that `output.rs` is out of bounds).

**Files:**
- Create: `crates/transync-cli/src/translate_cmd/args.rs`, `translate_cmd/input.rs`, `translate_cmd/provider.rs`, `translate_cmd/publish.rs`, `translate_cmd/report.rs`
- Modify: `crates/transync-cli/src/translate_cmd.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) struct CliFailure { pub(crate) code: crate::error::ExitCode, pub(crate) message: String }
// translate_cmd.rs: pub(crate) async fn execute(args: &TranslateArgs) -> Result<RunSummary, CliFailure>
// outer `run` = match execute(): Ok -> summary printing + exit 0 path; Err -> report(message) + code as i32
```

`RunSummary` carries what the current tail of `run` prints (verbose tallies, all-fell-back detection input). The ~15 scattered `return ExitCode::… as i32` sites collapse into `CliFailure` constructions inside `execute`.

- [ ] **Step 1: Move the pure helpers** into `args.rs` / `input.rs` / `provider.rs` / `report.rs` per the digest's grouping (they are already well-factored functions — this is grouping, not rewriting). The 7 in-file tests move with their subjects; the `Harness` clap test helper goes to `args.rs`. Green.
- [ ] **Step 2: Collapse the two OutputTarget arms** into `publish.rs`: one file-set builder taking the outputs + serialized report + bundle decision, with the commit function (`publish_out_dir` vs `write_fileset_atomic`) chosen by the target. The html preflight stays with the Dir arm's path. `output.rs` untouched.
- [ ] **Step 3: Introduce `execute()`** — mechanical conversion of each early-return site to `return Err(CliFailure { code, message })`; the outer `run` becomes report-then-exit. NO exit-code value changes, NO message-text changes (cli_smoke pins both).
- [ ] **Step 4: Full suite incl. the 19 `cli_smoke.rs` subprocess tests green, byte-stable expectations; commit**

```bash
git add crates/transync-cli/src/translate_cmd.rs crates/transync-cli/src/translate_cmd/
git commit -m "refactor: translate_cmd split; execute() seam collapses exit-code policy into one table (OI-0008 R0001-0080)"
```

---

### Task 10: Parser subtractive slice

Spec §7 (first half). Digest: `concernMixing.md` §1 + §5 ranking notes.

**Files:**
- Modify: `crates/transync-syntax/src/parser.rs`, `crates/transync-syntax/src/id.rs` (re-key pass), `crates/transync-syntax/src/parser/refdefs.rs`, `crates/transync-syntax/src/regen.rs`, `crates/transync-syntax/src/render.rs`, `crates/transync-syntax/src/render/attrs.rs`, `crates/transync-syntax/src/align.rs` (parent_id filter sites), `crates/transync-core/src/unit/context.rs` (heading_level dedupe)

**Interfaces:**
- Produces: `Document` WITHOUT `hierarchy`; `Block` WITHOUT `parent_id`; one exported heading-level mapping in `transync-syntax` (e.g. `pub(crate)`… NOTE: core consumes it cross-crate, so it must be `pub` — place it with the existing kind helpers in `id.rs` or the parser and mark `#[doc(hidden)]` if it is not curated API) consumed by core's `unit/context.rs` instead of its byte-copy.
- CRITICAL wire rule: `AlignmentBlock.parent_id` (schema 1.2.0) STAYS, emitted as `None` always — exactly today's bytes. The `align.rs` change is only the removal of now-tautological `Block.parent_id` reads.

- [ ] **Step 1: Delete `Document::hierarchy`** + `build_hierarchy` + the `assign_block_ids` re-key pass. The re-key test in `id.rs` loses its subject with the feature — this is the plan's ONE sanctioned test swap: replace it with an equivalent-strength assertion on id assignment itself (same fixture, asserting the assigned id sequence), and name the swap in the commit body. Green.
- [ ] **Step 2: Delete `Block::parent_id`** (provably always `None`) and simplify the five tautological filter sites (`refdefs`, `regen`, `render`, `render/attrs`, `align`) — each currently filters `parent_id.is_none()`, which is `true` for every block; the filter disappears, behavior identical. `align.rs` keeps emitting `parent_id: None` into `AlignmentBlock` literally. Green + scenario suite byte-stable.
- [ ] **Step 3: Unify `emit`** — one free function replacing the closure + the List arm's inline re-implementation (the digest flags the `ast_path`/`parent_id` divergence between the two copies; with `parent_id` gone, reconcile `ast_path` handling explicitly and pin it with a test asserting list-item `ast_path`s are unchanged from today — capture today's values FIRST).
- [ ] **Step 4: One section-scope algorithm** — with `build_hierarchy` deleted, `visit`'s heading stack is the only copy; extract nothing yet (Task 11 will home it), just verify singleness.
- [ ] **Step 5: `heading_level` single home** — export the survivor from `transync-syntax`; `unit/context.rs` deletes its byte-copy and imports it. Green.
- [ ] **Step 6: Full suite + wasm gate green; commit**

```bash
git add crates/transync-syntax/src/ crates/transync-core/src/unit/context.rs
git commit -m "refactor: parser subtractive slice — dead hierarchy + vestigial parent_id deleted, emit/heading_level unified (OI-0008 R0001-0078)"
```

---

### Task 11: Parser `visit` restructure

Spec §7 (second half). Digest: `concernMixing.md` §1 target structure.

**Files:**
- Create: `crates/transync-syntax/src/parser/options.rs`, `parser/classify.rs`, `parser/emit.rs`, `parser/sections.rs`
- Modify: `crates/transync-syntax/src/parser.rs`

**Interfaces:**
- Produces: `parser.rs` = IR types + thin `parse`; `options.rs` = `gfm_options`/`comrak_options` (public path `parser::comrak_options` PRESERVED — it is `pub` today); `classify.rs` = NodeValue→`BlockKind` incl. image-only promotion + both label tables; `emit.rs` = Task 10's unified emit; `sections.rs` = the heading-scope stack; a `WalkState<'a>` struct (blocks, counter, section stack, ast_path, warnings) replacing the 9-parameter signatures — both `#[allow(clippy::too_many_arguments)]` lines die.
- IR FREEZE: `Document`/`Block`/`Section` shapes exactly as Task 10 left them. No serde additions, no field changes.

- [ ] **Step 1: Move config + labels + classification** into `options.rs`/`classify.rs`; inline test modules travel with their subjects. Green.
- [ ] **Step 2: Introduce `WalkState`** and convert `walk_children`/`visit`; move emit to `emit.rs`, the section stack to `sections.rs`. The match arms' logic is UNCHANGED — this is signature/namespace surgery. Both clippy allows removed.
- [ ] **Step 3: Behavioral pin** — the whole scenario suite + render/walk/align/regen/refdefs test packs green unchanged; wasm gate green (new modules are feature-free by construction).
- [ ] **Step 4: Commit**

```bash
git add crates/transync-syntax/src/parser.rs crates/transync-syntax/src/parser/
git commit -m "refactor: parser walker restructured — classify/emit/sections modules, WalkState replaces 9-arg signatures (OI-0008 R0001-0078)"
```

---

### Task 12: Records + final gates

Spec §9. Write records against `git log` reality.

**Files:**
- Create: `docs/project/design-change-records/archive/DCR-0019-internal-quality-wave.md`
- Modify: `docs/project/open-issues.md` (OI-0008 → RESOLVED, OI-0033 → RESOLVED, NEW OI for the NUL column desync), `docs/project/status.md`, `docs/project/phase-state.yaml`, `CHANGELOG.md` (`[Unreleased]`)

**Interfaces:** consumes everything; cite actual commit hashes.

- [ ] **Step 1: DCR-0019** (DCR-0017/0018 structure): the two behavior changes (lone-CR support with probe evidence; `total_retries` content-only semantics), the policy-module architecture (A1), all six split maps (before → after module layouts), the `transync-syntax` field deletions (`hierarchy`, `parent_id`) with the wire-invariance note, the `pagination.rs` deletion rationale (spec §5), the IR freeze rule honored, zero test removals + the one test-swap from Task 10 Step 1. No line-number references.
- [ ] **Step 2: OI-0008 → RESOLVED** (each bullet group mapped to its commit), **OI-0033 → RESOLVED** (posture, probe, Guard-1 closure), **new OI filed**: comrak NUL→U+FFFD substitution desyncs byte COLUMNS (a 1-byte NUL counted as 3), non-fatal today via clamping — same house format as OI-0033's entry.
- [ ] **Step 3: status.md / phase-state.yaml** — latest-wave bullets + notes paragraph + last_updated; CHANGELOG `[Unreleased]` gains the wave's entries (the lone-CR fix under a Fixed heading; `total_retries` under Changed; the refactor cluster summarized under Changed/Internal).
- [ ] **Step 4: Final full gate run** — fmt, clippy, workspace suite (report the authoritative summed count; expect ≥ 340 + this wave's new tests, zero failures), wasm gate, `scripts/smoke.sh` (incl. rustdoc gate — the moved modules' docs must stay warning-free), `scripts/test-browser.sh` (Playwright 8/8), `public_surface.rs` green UNMODIFIED (verify with `git diff --stat v0.2.0..HEAD -- crates/transync/tests/public_surface.rs` = empty), resp-translator `cargo check --workspace` green.
- [ ] **Step 5: Commit**

```bash
git add docs/ CHANGELOG.md
git commit -m "docs: DCR-0019 internal-quality wave — OI-0008 + OI-0033 RESOLVED, NUL column desync filed"
```

---

## Baseline numbers (for reviewers)

- Suite baseline at plan time: **340 passed / 0 failed / 3 ignored** (summed `test result:` lines). This wave only adds tests (T1 +6, T2 +3-ish, T3 policy + report pins, T6 budget pins, T7 classify pins, T10 ast_path pin); one test SWAP in T10 Step 1 (hierarchy re-key → id-assignment equivalent), zero removals otherwise.
- `public_surface.rs`: must be untouched from tag `v0.2.0` through this wave's HEAD.
- Two behavior changes only; every other task's review question is "is this pure motion?"

---

## Dated note

*(2026-08-07 — appended, nothing above rewritten. Ticket `81b28a00`. **Four of
this plan's literal commit-message strings carry a swapped review id, and the
commits were made with those strings, so both stay as written.** Against the
retired 2026-05-02 Review 0001's own titles (`git show
bb93b68^:reviews/reviewed/0001.md`; indexed in `reviews/README.md`),
`R0001-0077` is "Parser traversal has too many mutable cross-cutting
parameters" and `R0001-0078` is "Unit construction mixes payload extraction,
profile rendering, batching, and structural inspection". The unit-construction
pair — `f10b070` "structural-fingerprint oracle moves to crate::structure" and
`8b339f3` "unit construction split into payload strategy + budget resolution"
— says `R0001-0077` where the finding is `R0001-0078`; the parser pair —
`094c1c4` "parser subtractive slice" and `73fd331` "parser walker
restructured" — says `R0001-0078` where the finding is `R0001-0077`. The work
each task specifies is unaffected. `docs/project/open-issues.md` and
`docs/project/status.md`, the living records, carry the corrected pairing; the
design spec beside this plan carries the same note.)*
