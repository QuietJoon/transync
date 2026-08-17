//! The CLI test suite's scratch directories are the suite's to remove.
//!
//! `cli_smoke.rs` and `serve_static.rs` between them create a scratch
//! directory per scratch-using test, every run, forever. Before ticket
//! `2fef6a` nothing removed them: `serve_static.rs` called `remove_dir_all`
//! at the end of some tests — which is precisely the set of runs that did not
//! need it, since a panicking test never reaches its last statement — and
//! `cli_smoke.rs` called nothing at all. They accumulated until the scratch
//! volume filled and an unrelated test failed with `StorageFull`, which reads
//! like a regression in the code under test and is not one.
//!
//! So the guard is the contract, and this file pins it. It is deliberately
//! not feature-gated: the leak was in the ungated file too, and a promise the
//! plain `cargo test --workspace` does not check is a promise that rots.
//!
//! TRACE: ticket 2fef6a

mod common;

use common::ScratchDir;
use std::path::PathBuf;

/// The ordinary case: the directory exists while the guard does, and is gone
/// once the guard drops.
#[test]
fn a_scratch_dir_exists_while_its_guard_does_and_not_after() {
    let recorded: PathBuf;
    {
        let scratch = ScratchDir::new("transync-scratch-lifetime");
        recorded = scratch.path().to_path_buf();
        assert!(
            recorded.is_dir(),
            "the guard should have created {recorded:?}"
        );
        std::fs::write(scratch.join("payload.txt"), b"contents").expect("scratch is writable");
    }
    assert!(
        !recorded.exists(),
        "the guard must remove {recorded:?}, contents and all"
    );
}

/// The case the end-of-test `remove_dir_all` calls could not cover, and the
/// reason this is a `Drop` guard: a test that panics still cleans up, because
/// the unwind runs the destructor that the abandoned statements never reach.
#[test]
fn a_panicking_test_still_leaves_no_scratch_dir() {
    let recorded = std::sync::Arc::new(std::sync::Mutex::new(None::<PathBuf>));
    let seen = std::sync::Arc::clone(&recorded);

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let scratch = ScratchDir::new("transync-scratch-panic");
        *seen.lock().unwrap() = Some(scratch.path().to_path_buf());
        std::fs::write(scratch.join("payload.txt"), b"contents").expect("scratch is writable");
        panic!("a test failing the way tests fail");
    }));

    assert!(
        outcome.is_err(),
        "the closure under test must have panicked"
    );
    let recorded = recorded.lock().unwrap().clone().expect("path was recorded");
    assert!(
        !recorded.exists(),
        "an unwinding test must still leave {recorded:?} removed"
    );
}

/// Two guards never name the same directory — otherwise one test's cleanup
/// would delete another test's files mid-run. The clock alone does not settle
/// this: `--test-threads=4` puts four of these calls in the same instant.
#[test]
fn concurrent_guards_never_share_a_path() {
    let mut handles = Vec::new();
    for _ in 0..4 {
        handles.push(std::thread::spawn(|| {
            (0..64)
                .map(|_| ScratchDir::new("transync-scratch-unique"))
                .collect::<Vec<_>>()
        }));
    }
    let dirs: Vec<ScratchDir> = handles
        .into_iter()
        .flat_map(|h| h.join().expect("scratch thread should not panic"))
        .collect();

    let mut paths: Vec<PathBuf> = dirs.iter().map(|d| d.path().to_path_buf()).collect();
    let created = paths.len();
    paths.sort();
    paths.dedup();
    assert_eq!(
        paths.len(),
        created,
        "two guards claimed the same directory"
    );
}
