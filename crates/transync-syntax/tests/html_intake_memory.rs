//! R0013-0002: an HTML document's intake holds memory linear in its nesting.
//!
//! The defect this pins: an open pass-through container that keeps a full
//! copy of its ancestor path makes `N` nested `<div>`s around one paragraph
//! hold `N(N+1)/2` ordinals at once. At twenty thousand levels that is a
//! 220 KB input asking for about 1.6 GB, and an allocation failure aborts the
//! process rather than returning an error. A container keeps its own ordinal
//! only, and a block's path is built from the open stack when the block is
//! emitted.
//!
//! The measurement is the PEAK of live heap bytes on this thread while
//! `intake::html::parse` runs, at `N = 1,000` and `N = 4,000`. Two bounds:
//!
//! - the ratio. Linear growth gives about 4 for four times the input; the
//!   quadratic retention gave about 16. The bound is 8. Both sizes sit just
//!   under a power-of-two `Vec` and hash-table capacity, so the capacity
//!   doubling that every growing collection pays lands the same way at both
//!   and cannot carry a linear ratio past it.
//! - the absolute peak at `N = 4,000`, under 256 bytes per input byte (about
//!   11 MB here, where the quadratic retention reached about 128 MB).
//!
//! Sizes are counted from `Layout::size()`, so the figure is deterministic and
//! independent of the system allocator's rounding.
//!
//! The paragraph-per-level shape (`<div><p>x</p>` repeated) is NOT closed by
//! this alone: each emitted block's own path is as long as its nesting. That
//! shape is bounded by the HTML nesting ceiling the pipeline checks before it
//! parses (`intake::html::check_nesting_depth`), which this binary does not
//! exercise.
//!
//! Its own test binary with one test, because it installs a global allocator,
//! which would count every other test's allocations if it shared a binary with
//! them.
//!
//! TRACE: R0013-0002

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Tracks live and peak heap bytes for the current thread and delegates the
/// memory to the system allocator.
struct Peak;

thread_local! {
    static LIVE: Cell<usize> = const { Cell::new(0) };
    static HIGH: Cell<usize> = const { Cell::new(0) };
}

fn grow(bytes: usize) {
    // `try_with`, not `with`: the allocator runs during thread teardown too,
    // after the thread-locals are gone, and must not panic there.
    let _ = LIVE.try_with(|live| {
        let now = live.get() + bytes;
        live.set(now);
        let _ = HIGH.try_with(|high| high.set(high.get().max(now)));
    });
}

fn shrink(bytes: usize) {
    // Saturating: memory allocated on another thread and freed on this one
    // must not wrap the counter.
    let _ = LIVE.try_with(|live| live.set(live.get().saturating_sub(bytes)));
}

// SAFETY: every method forwards to `System` with the caller's own arguments,
// so the `GlobalAlloc` contract is `System`'s; the counters touch no memory
// the allocator hands out.
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: forwarded unchanged; the caller upholds `alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        grow(layout.size());
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // A realloc may move, so both blocks can be live at once: count the
        // new one before releasing the old.
        grow(new_size);
        shrink(layout.size());
        // SAFETY: as above; `ptr` came from this allocator, i.e. from `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        shrink(layout.size());
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Peak = Peak;

/// Peak heap bytes above the starting level while `parse` runs over `N`
/// nested `<div>`s around one paragraph. The input is built before the
/// measurement starts and the document is dropped after it ends.
fn peak_for(n: usize) -> (usize, usize) {
    let html = format!("{}<p>x</p>{}", "<div>".repeat(n), "</div>".repeat(n));
    let base = LIVE.with(Cell::get);
    HIGH.with(|high| high.set(base));
    let doc = transync_syntax::intake::html::parse(&html);
    let peak = HIGH.with(Cell::get) - base;
    assert_eq!(doc.blocks.len(), 1, "one paragraph, whatever the nesting");
    assert_eq!(doc.blocks[0].ast_path.0.len(), n + 1);
    drop(doc);
    (peak, html.len())
}

#[test]
fn a_deeply_nested_document_costs_memory_linear_in_its_depth() {
    // A warm-up parse, so a lazily initialised static could not read as
    // growth at the first size.
    let _ = transync_syntax::intake::html::parse("<div><p>x</p></div>");
    let (small, _) = peak_for(1_000);
    let (large, large_len) = peak_for(4_000);
    assert!(
        large < small * 8,
        "four times the nesting cost {large} bytes against {small}: growth is \
         quadratic, so open containers are retaining their ancestor paths again"
    );
    assert!(
        large < 256 * large_len,
        "a {large_len}-byte input peaked at {large} bytes, over 256 per input byte"
    );
}
