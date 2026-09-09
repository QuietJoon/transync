//! Proof — by execution, not by reading — of the sync-engine claims that
//! `docs/backlog.md`'s Type 2 entry `browser-sync-ux-polish` has been resting
//! on since it was owner-deferred on **2026-08-06**.
//!
//! A parked entry is not re-read. That is the whole hazard: the entry keeps
//! describing a system that has moved, and the next person to pick it up plans
//! work against a premise that expired. One of the three claims below has in
//! fact expired, and the point of this file is that the expiry is now a failing
//! assertion away from being noticed rather than a month of nobody looking.
//!
//! # Why a Rust test asserts on JavaScript
//!
//! `crates/transync-cli/tests/sync_js_drift.rs` established the pattern: the
//! engine is vanilla JS with no test runner of its own in the Rust suite, so
//! the durable guard is a Rust test that reads the source and asserts on it.
//! The browser suite (`web/tests/engine.spec.js`) proves *behaviour*; this file
//! proves the *shape of the claim*, which is what a backlog entry asserts.
//!
//! Two rules follow from that pattern, and both are load-bearing here:
//!
//! - **Assert on named constants and signatures, never on prose.** A substring
//!   assertion over a comment rots into a false pass the first time someone
//!   rewords the comment, which is the failure mode a drift guard exists to
//!   prevent.
//! - **Assert against the workspace copy** (`web/js/sync.js`). It is sufficient
//!   because `sync_js_drift.rs` already welds it byte-for-byte to the embedded
//!   copy at `crates/transync-cli/web/sync.js`; if that weld ever breaks, that
//!   test fails, not this one. This file re-checks the weld anyway — one
//!   `assert_eq!` — so a reader here does not have to take the coupling on
//!   faith.
//!
//! TRACE: docs/backlog.md Type 2 `browser-sync-ux-polish`
//! TRACE: R0011-0094 (the claim that expired)

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root resolves from CARGO_MANIFEST_DIR/../..")
}

fn sync_js() -> String {
    let p = repo_root().join("web/js/sync.js");
    std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("{} should be readable: {e}", p.display()))
}

/// The coupling this file's single-copy assertions rest on, restated as a test
/// so it is not an assumption. If the two copies ever diverge, every other
/// assertion here is only half a proof.
#[test]
fn the_two_sync_js_copies_are_still_one_file() {
    let root = repo_root();
    let workspace = std::fs::read_to_string(root.join("web/js/sync.js")).expect("workspace copy");
    let embedded = std::fs::read_to_string(root.join("crates/transync-cli/web/sync.js"))
        .expect("CLI-embedded copy");
    assert_eq!(
        workspace, embedded,
        "web/js/sync.js and crates/transync-cli/web/sync.js have drifted, so an \
         assertion against one of them no longer says anything about the other. \
         sync_js_drift.rs owns this weld; fix it there."
    );
}

/// **Claim (`browser-sync-ux-polish`, first of three):** "no hysteresis on
/// active-block selection (rapid flicks may briefly pick a neighbor)".
///
/// VERDICT: holds. `activeBlockWithProgress` is a pure function of the pane's
/// current geometry — it re-walks every block on each call and keeps no memory
/// of which block it chose last, so there is nothing for hysteresis to be made
/// of. This test pins that statelessness structurally rather than by reading
/// the prose: the selector takes exactly `(pane, blocks)` and no prior-choice
/// argument, and the module declares no damping or stickiness constant.
///
/// Deliberately narrow. "Rapid flicks may briefly pick a neighbor" is a claim
/// about perception under motion, which a source assertion cannot reach; what
/// it *can* reach is the absence of the mechanism that would prevent it.
#[test]
fn active_block_selection_still_carries_no_hysteresis_state() {
    let js = sync_js();

    assert!(
        js.contains("function activeBlockWithProgress(pane, blocks) {"),
        "the selector's signature changed; if it now takes a previous choice or \
         a tolerance, hysteresis may have been added and \
         `browser-sync-ux-polish`'s first claim is stale"
    );

    for absent in [
        "HYSTERESIS",
        "STICKY",
        "DEADBAND",
        "SELECTION_TOLERANCE",
        "MIN_SWITCH",
    ] {
        assert!(
            !js.contains(absent),
            "sync.js declares {absent:?} — a damping mechanism appears to exist, \
             so the claim that active-block selection has no hysteresis is stale"
        );
    }
}

/// **Claim (`browser-sync-ux-polish`, second of three):** "a single fixed
/// partner-pane easing policy (**20%/frame lerp**)".
///
/// VERDICT: **half stale, and this is the finding.** The policy is still single
/// and still fixed — no caller can vary it — but it has not been a *per-frame*
/// lerp since R0011-0094 (`e125a3a`, 2026-09-06). The factor is now applied per
/// elapsed **millisecond**: `SMOOTHING_FACTOR` is quoted against
/// `NOMINAL_FRAME_MS` and raised to the power `elapsed / NOMINAL_FRAME_MS`,
/// with `MAX_FRAME_STEP_MS` capping the arrears one frame may claim. So a
/// gesture settles in the same wall-clock time at 120 Hz as at 60 Hz — which is
/// precisely the defect the words "20%/frame" describe.
///
/// The entry was written 2026-08-06 and the change landed 2026-09-06, a month
/// later. Nobody re-read it, which is why this test exists: the stale half is
/// now pinned by an assertion instead of by nobody looking.
#[test]
fn the_easing_policy_is_time_based_not_per_frame() {
    let js = sync_js();

    // The three constants the time-based form is built from. Their VALUES are
    // asserted, not merely their presence, because "a constant named
    // NOMINAL_FRAME_MS exists" would pass against a per-frame implementation
    // that happened to keep the name.
    assert!(
        js.contains("const SMOOTHING_FACTOR = 0.2;"),
        "SMOOTHING_FACTOR is no longer 0.2; the easing policy moved and the \
         backlog entry's description needs re-deriving"
    );
    assert!(
        js.contains("const NOMINAL_FRAME_MS = 1000 / 60;"),
        "NOMINAL_FRAME_MS is gone or changed — it is the frame duration \
         SMOOTHING_FACTOR is quoted against, and without it the factor has no \
         time meaning"
    );
    assert!(
        js.contains("const MAX_FRAME_STEP_MS = 100;"),
        "MAX_FRAME_STEP_MS is gone or changed — it is the ceiling on the arrears \
         a single frame may claim, and without it one long stall would close the \
         whole remaining gap at once"
    );

    // The exponent is the mechanism: per-elapsed-time, not per-callback.
    assert!(
        js.contains("elapsed / NOMINAL_FRAME_MS"),
        "the lerp no longer raises the smoothing factor to `elapsed / \
         NOMINAL_FRAME_MS`. If it has gone back to one application per \
         animation-frame callback, then `browser-sync-ux-polish`'s \
         \"20%/frame\" wording is accurate again — and R0011-0094 has been \
         reverted, which is the more likely reading and the more serious one."
    );
}

/// **Claim (`browser-sync-ux-polish`, third of three):** "`SMOOTHING_FACTOR`
/// hardcoded rather than a `mountSync` option".
///
/// VERDICT: holds, in both halves. It is a module-level `const`, and
/// `mountSync` takes exactly three positional parameters with no options
/// object, so there is no channel through which a caller could override it.
///
/// Both halves are asserted because either alone is weak: a `const` could still
/// be shadowed by an option, and an options-free signature says nothing about
/// whether the value is fixed.
#[test]
fn smoothing_factor_is_a_module_constant_with_no_mount_time_override() {
    let js = sync_js();

    assert!(
        js.contains("const SMOOTHING_FACTOR = 0.2;"),
        "SMOOTHING_FACTOR is no longer a module-level const"
    );
    assert!(
        js.contains("export function mountSync(sourcePane, targetPane, alignmentMap) {"),
        "mountSync's signature changed. If it grew a fourth parameter, a caller \
         may now be able to supply easing options and the third claim of \
         `browser-sync-ux-polish` is stale."
    );
}

/// **Claim (`intra-block-progress-indicator`), the engine half:** "Within very
/// long blocks (large tables, long code blocks) sync feels coarse; an optional
/// intra-block progress indicator is the drafted mitigation."
///
/// VERDICT: the entry's premise is **more subtle than it reads**, and saying so
/// precisely is worth more than a bare verdict.
///
/// The engine already *computes* intra-block progress: `activeBlockWithProgress`
/// is named for it and returns a `progress` value, which the partner-pane
/// scroll uses to land at the proportional offset **within** the active block
/// rather than at its top. So the coarseness the entry describes is not a
/// missing computation — it is a missing *display*. What does not exist is any
/// surfacing of that value to the reader.
///
/// That distinction changes the work: the drafted mitigation is a UI addition
/// over a number the engine already has, not a new measurement. This test pins
/// both halves so the distinction survives.
#[test]
fn intra_block_progress_is_computed_by_the_engine_and_shown_to_nobody() {
    let js = sync_js();

    // Computed: the selector is named for it and the scroll consumes it.
    assert!(
        js.contains("function activeBlockWithProgress(pane, blocks) {"),
        "the engine no longer has a progress-bearing active-block selector, so \
         the premise that progress is computed-but-unshown needs re-deriving"
    );
    assert!(
        js.contains("progress"),
        "no progress value appears in the engine at all"
    );

    // Not shown: no indicator element, class or attribute is emitted for it.
    for absent in [
        "progress-indicator",
        "data-progress",
        "progressBar",
        "intraBlock",
    ] {
        assert!(
            !js.contains(absent),
            "sync.js references {absent:?} — an intra-block progress indicator \
             may now exist, which would make `intra-block-progress-indicator` \
             resolved rather than deferred"
        );
    }
}
