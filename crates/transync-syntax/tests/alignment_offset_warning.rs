//! R0003-0055: `align::build_alignment_map` still answers a caller who hands
//! it `BlockOffsets` that do not cover the document — the `0..0` target range
//! is the safe answer and stays — but it no longer answers *silently*.
//!
//! # Why this lives in its own test binary
//!
//! `tracing` caches a callsite's `Interest` **globally, once**, the first time
//! that callsite is reached. The registration consults the current thread's
//! default subscriber (`Rebuilder::JustOne` → `dispatcher::get_default`), so if
//! the first thread to reach the `warn!` has no subscriber installed, the
//! callsite is cached as `Interest::never()` and every later emission — on any
//! thread, under any subscriber — is skipped by the macro's fast path.
//!
//! Several unit tests in `align` and `render` call `build_alignment_map` with
//! `BlockOffsets::default()` on purpose, so inside the lib test binary this
//! assertion is a coin flip decided by thread scheduling: it passed standalone
//! and failed under `cargo test --workspace`. An integration test is a separate
//! process with exactly one test in it, so this thread is provably the first to
//! reach the callsite. **Keep it that way** — a second test in this file brings
//! the race back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use transync_syntax::align::{ByteRange, build_alignment_map};
use transync_syntax::id::assign_block_ids;
use transync_syntax::intake::markdown::parse;
use transync_syntax::outcome::html_outcomes;
use transync_syntax::regen::{BlockOffsets, regenerate};

/// Records every `tracing` event field raised on the installing thread.
/// Hand-rolled: `tracing-subscriber` is the reference binary's dependency
/// alone, and a test is no reason to widen the wasm-compilable base crate's
/// dependency set (`transync-core::test_fixtures` records the same reasoning
/// for its copy).
#[derive(Default)]
struct EventLog(Mutex<Vec<String>>);

struct RecordingSubscriber(Arc<EventLog>);

impl tracing::Subscriber for RecordingSubscriber {
    fn enabled(&self, _meta: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _id: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _id: &tracing::span::Id, _follows: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        let mut v = FieldVisitor(String::new());
        event.record(&mut v);
        self.0.0.lock().unwrap().push(v.0);
    }
    fn enter(&self, _id: &tracing::span::Id) {}
    fn exit(&self, _id: &tracing::span::Id) {}
}

struct FieldVisitor(String);

impl tracing::field::Visit for FieldVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.0.push_str(&format!(" {}={value:?}", field.name()));
    }
}

#[test]
fn incomplete_block_offsets_are_named_once_on_the_warn_channel() {
    let src = "# Title\n\npara\n";
    let mut doc = parse(src).expect("parses");
    assign_block_ids(&mut doc);
    assert_eq!(doc.blocks.len(), 2);
    let outcomes = html_outcomes(&doc);

    let log = Arc::new(EventLog::default());
    let map = {
        let _guard = tracing::subscriber::set_default(RecordingSubscriber(Arc::clone(&log)));
        // Belt and braces for the hazard the module comment describes: if the
        // callsite were somehow already registered, this recomputes its
        // interest against the subscriber installed a line above.
        tracing::callsite::rebuild_interest_cache();

        // The offsets `regenerate` returned for THIS document cover every
        // block — the in-pipeline case. Nothing to report.
        let (_, offsets) = regenerate(&doc, &HashMap::new());
        build_alignment_map(
            &doc,
            &HashMap::new(),
            &offsets,
            "auto",
            "ko",
            None,
            &outcomes,
        );
        assert!(
            log.0.lock().unwrap().is_empty(),
            "a complete map warns about nothing: {:?}",
            log.0.lock().unwrap()
        );

        // A foreign / empty map misses every block.
        build_alignment_map(
            &doc,
            &HashMap::new(),
            &BlockOffsets::default(),
            "auto",
            "ko",
            None,
            &outcomes,
        )
    };

    let events = log.0.lock().unwrap().clone();
    assert_eq!(
        events.len(),
        1,
        "one line for the document, not one per block: {events:?}"
    );
    let e = &events[0];
    assert!(e.contains("missing=2") && e.contains("total=2"), "got: {e}");
    for block in &doc.blocks {
        assert!(
            e.contains(&block.block_id.0),
            "id {} unnamed in {e}",
            block.block_id.0
        );
    }

    // The row values are unchanged — the warning is the whole delta.
    for row in &map.blocks {
        assert_eq!(
            row.target_range,
            ByteRange::default(),
            "the safe empty range is still what ships"
        );
    }
}
