//! R0013-0009: scanning a raw-text region allocates nothing per `<`.
//!
//! Inside `<script>`, `<style>` or an RCDATA element every `<` byte is a
//! candidate end tag, so the scanner probes for the element's own closer at
//! each one. The probe used to build the `</name` prefix as a fresh `String`
//! on every candidate, which made a script holding `a < b` comparisons pay one
//! heap allocation per comparison, on each of the several whole-document scans
//! the pipeline makes (intake, walk, balance, inventory, the layer-6 rescan).
//!
//! The measurement is the allocation COUNT on this thread while `scan_tags`
//! runs, at two region sizes. A scan whose allocations grow with the number of
//! `<` bytes in a raw-text region fails; one that allocates a constant amount
//! (the token vector, the frame stack, the names) passes.
//!
//! Its own test binary because it installs a global allocator, which would
//! count every other test's allocations too if it shared a binary with them.
//!
//! TRACE: R0013-0009

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

/// Counts allocations made on the current thread and delegates the memory
/// to the system allocator.
struct Counting;

thread_local! {
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

fn bump() {
    // `try_with`, not `with`: the allocator runs during thread teardown too,
    // after the thread-local is gone, and must not panic there.
    let _ = ALLOCS.try_with(|n| n.set(n.get() + 1));
}

// SAFETY: every method forwards to `System` with the caller's own arguments,
// so the `GlobalAlloc` contract is `System`'s; the counter touches no memory
// the allocator hands out.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: forwarded unchanged; the caller upholds `alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        bump();
        // SAFETY: as above.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        bump();
        // SAFETY: as above; `ptr` came from this allocator, i.e. from `System`.
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: as above.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn allocations() -> usize {
    ALLOCS.with(Cell::get)
}

/// Allocations `scan_tags` makes over `<tag>` + `n` copies of `a<b;` +
/// `</tag>`. The input is built before the count starts.
fn allocs_for(tag: &str, n: usize) -> usize {
    let html = format!("<{tag}>{}</{tag}>", "a<b;".repeat(n));
    let before = allocations();
    let tokens = transync_html::scan_tags(&html);
    let after = allocations();
    assert_eq!(tokens.len(), 2, "one open and one close tag: {tokens:?}");
    after - before
}

#[test]
fn a_raw_text_region_costs_the_same_allocations_however_many_angle_brackets_it_holds() {
    // A warm-up scan, so a lazily initialised static (none today) could not
    // read as growth.
    let _ = transync_html::scan_tags("<script>a<b</script>");
    for tag in ["script", "style", "textarea", "title", "xmp"] {
        let small = allocs_for(tag, 10);
        let large = allocs_for(tag, 10_000);
        assert_eq!(
            small, large,
            "`<{tag}>` with 10 `<` bytes cost {small} allocations and with 10,000 cost \
             {large}: the raw-text close probe allocates per candidate `<` again"
        );
    }
}
