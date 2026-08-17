//! Shared `#[cfg(test)]` fixtures used across the crate's unit and
//! integration-style tests. Keep this module small — only fixtures that
//! more than one test site needs belong here.

use crate::FallbackStatus;
use crate::id::BlockId;
use crate::parser::Document;
use crate::validate::ValidatedUnit;
use std::cell::RefCell;
use std::sync::{Arc, Mutex, Once};

/// Captures every `tracing` event raised on the thread it is attached to, so
/// a test can assert which channel named a diagnostic. Hand-rolled because
/// `tracing-subscriber` is the reference binary's dependency alone
/// (R0001-0032), and a test is no reason to widen that.
///
/// Attach with [`record_events`] and hold the guard for the scope under test.
#[derive(Default)]
pub(crate) struct EventLog {
    events: Mutex<Vec<(String, String)>>,
}

impl EventLog {
    /// The messages recorded on one `tracing` target, in emission order.
    pub(crate) fn messages_on(&self, target: &str) -> Vec<String> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|(t, _)| t == target)
            .map(|(_, m)| m.clone())
            .collect()
    }
}

thread_local! {
    /// The log the calling thread is recording into, if any. This is what
    /// keeps one test's events out of another's, and it is the *only* thing
    /// that is per-thread here — see [`record_events`] for why the subscriber
    /// itself is not.
    static ATTACHED: RefCell<Option<Arc<EventLog>>> = const { RefCell::new(None) };
}

/// The one `tracing` subscriber this crate's tests install, process-wide, for
/// the life of the test binary. It owns no log: it routes each event to the
/// [`EventLog`] the emitting thread has attached, and drops events from
/// threads that have none.
struct RoutingSubscriber;

impl tracing::Subscriber for RoutingSubscriber {
    fn enabled(&self, _meta: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _attrs: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _id: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _id: &tracing::span::Id, _follows: &tracing::span::Id) {}
    fn event(&self, event: &tracing::Event<'_>) {
        // Clone the handle out before touching the log, so the `RefCell`
        // borrow cannot outlive this line. `try_with` because a thread tearing
        // down has already destroyed its thread-locals and may still emit.
        let attached = match ATTACHED.try_with(|a| a.borrow().clone()) {
            Ok(Some(log)) => log,
            _ => return,
        };
        let mut message = MessageVisitor(String::new());
        event.record(&mut message);
        attached
            .events
            .lock()
            .unwrap()
            .push((event.metadata().target().to_string(), message.0));
    }
    fn enter(&self, _id: &tracing::span::Id) {}
    fn exit(&self, _id: &tracing::span::Id) {}
}

/// Record every `tracing` event this thread raises into `log`, until the
/// returned guard drops.
///
/// # Why one process-wide subscriber and not `set_default` per test
///
/// `tracing` caches each callsite's `Interest` **globally**, and — while only
/// one dispatcher has ever been registered — computes it from whatever
/// subscriber the thread that *first reaches that callsite* happens to have.
/// With a scoped `tracing::subscriber::set_default` per test, that thread is a
/// coin flip. A concurrently running test with no subscriber installed answers
/// `Interest::never()` for a callsite it is the first to touch; the answer is
/// cached for the rest of the process; and every later event at that callsite
/// is discarded before any subscriber sees it — including the one the
/// recording test is about to assert on.
///
/// That is exactly the shape ticket `9601d3` reported: the log comes back
/// **empty**, not wrong, and only under a narrowed filter that happens to
/// co-schedule the recording test with a plain one. Measured before this
/// change, `cargo test -p transync-core --lib profile:: -- --test-threads=4`
/// failed 18 runs out of 30 while the full-suite run stayed green — a test
/// that passes only under one scheduling is a test that will lie later. The
/// empty log is also what would make a "nothing was warned" assertion pass
/// vacuously, so the mechanism is worth making deterministic rather than
/// living with.
///
/// A process-wide subscriber removes the coin flip: every thread's default
/// dispatcher is this one, so a callsite registers as `Interest::always()`
/// whichever thread reaches it first. Which *log* an event lands in stays
/// thread-local, so the isolation between tests is exactly what it was.
///
/// The `rebuild_interest_cache` below closes the one remaining window — a
/// callsite first reached *before* this subscriber was installed keeps the
/// `never` it was given. Rebuilding here, before the code under test runs,
/// makes the observing thread's view of every callsite deterministic.
pub(crate) fn record_events(log: Arc<EventLog>) -> RecordingGuard {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        tracing::subscriber::set_global_default(RoutingSubscriber)
            .expect("nothing else in this crate's tests installs a global tracing subscriber");
    });
    tracing::callsite::rebuild_interest_cache();

    let previous = ATTACHED.with(|attached| attached.borrow_mut().replace(log));
    RecordingGuard { previous }
}

/// Detaches the log [`record_events`] attached, restoring whatever the thread
/// had before it.
pub(crate) struct RecordingGuard {
    previous: Option<Arc<EventLog>>,
}

impl Drop for RecordingGuard {
    fn drop(&mut self) {
        let restored = self.previous.take();
        let _ = ATTACHED.try_with(|attached| *attached.borrow_mut() = restored);
    }
}

struct MessageVisitor(String);

impl tracing::field::Visit for MessageVisitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

/// Build a fixture set of `ValidatedUnit`s for `doc`, where the first
/// top-level paragraph carries a payload that — once spliced in by
/// regen — reparses as two top-level paragraphs instead of one.
///
/// Used by both the pipeline-level reparse-policy tests and the
/// validator-level byte-offset attribution test, which exercise the
/// same divergence scenario from different angles.
///
/// TRACE: R0004-0001
pub(crate) fn evil_units(doc: &Document) -> Vec<ValidatedUnit> {
    let mut units = Vec::new();
    let mut first = true;
    for block in &doc.blocks {
        let payload = if std::mem::take(&mut first) {
            EVIL_FIRST_PAYLOAD.to_string()
        } else {
            "translated".to_string()
        };
        units.push(ValidatedUnit {
            unit_id: block.block_id.clone(),
            final_status: FallbackStatus::Translated,
            accepted_payload: Some(payload),
            rejected_by: None,
            rejection_reason: None,
            warnings: Vec::new(),
        });
    }
    units
}

/// Helper: same shape as [`evil_units`] but keyed by `BlockId` for
/// consumers that work with the pipeline's `accepted` map.
pub(crate) fn evil_accepted(doc: &Document) -> std::collections::HashMap<BlockId, ValidatedUnit> {
    evil_units(doc)
        .into_iter()
        .map(|u| (u.unit_id.clone(), u))
        .collect()
}

/// Source markdown that pairs with [`evil_units`] / [`evil_accepted`].
pub(crate) const EVIL_SOURCE_MD: &str = "para 1\n\npara 2\n";

/// First-paragraph payload that, once spliced, reparses as two
/// top-level paragraphs.
pub(crate) const EVIL_FIRST_PAYLOAD: &str = "para 1 translated\n\nuninvited extra paragraph";

/// The recording fixture's own contract (ticket `9601d3`). Every `EventLog`
/// assertion in this crate rests on it, so it is pinned here rather than
/// inferred from the tests that use it.
mod mechanism_tests {
    use super::{EventLog, record_events};
    use std::sync::Arc;

    /// A target nothing else in this crate emits on, so a message under it can
    /// only have come from this module.
    const PROBE_TARGET: &str = "transync::test_fixtures::probe";

    /// One callsite, reachable from two threads. `#[inline(never)]` is not
    /// what makes it one callsite — a `tracing` callsite is a static per macro
    /// *invocation site*, and there is exactly one here — but it keeps the
    /// generated code as obvious as the intent.
    #[inline(never)]
    fn fire(marker: &str) {
        tracing::warn!(target: "transync::test_fixtures::probe", "probe {marker}");
    }

    /// The regression the ticket describes, made deterministic.
    ///
    /// A thread with no log attached reaches the callsite *first*, after the
    /// observing thread has already started recording. Under the previous
    /// fixture — a scoped `set_default` per test — that thread's default
    /// dispatcher was `NoSubscriber`, whose answer for a freshly registered
    /// callsite is `Interest::never()`, and the answer is cached process-wide:
    /// the event fired below would never reach any subscriber and the log
    /// would come back empty. With one process-wide subscriber there is no
    /// thread whose answer is `never`.
    #[test]
    fn a_thread_with_no_log_cannot_silence_a_callsite_for_a_thread_that_has_one() {
        let log = Arc::new(EventLog::default());
        let guard = record_events(Arc::clone(&log));

        std::thread::spawn(|| fire("from an unattached thread"))
            .join()
            .expect("the unattached thread should not panic");
        fire("from the recording thread");

        drop(guard);

        let said = log.messages_on(PROBE_TARGET);
        assert_eq!(
            said.len(),
            1,
            "the recording thread's own event must survive another thread \
             touching the callsite first: {said:?}"
        );
        assert!(
            said[0].contains("from the recording thread"),
            "and it must be that thread's event, not the other's: {said:?}"
        );
    }

    /// The isolation half: attaching is per-thread, so a thread that attached
    /// nothing contributes nothing, and the guard detaches on the way out.
    #[test]
    fn only_the_attached_thread_records_and_only_while_it_is_attached() {
        let log = Arc::new(EventLog::default());

        fire("before");
        {
            let _guard = record_events(Arc::clone(&log));
            fire("during");
            let elsewhere = Arc::clone(&log);
            std::thread::spawn(move || {
                fire("from a thread that attached nothing");
                assert!(
                    elsewhere.messages_on(PROBE_TARGET).len() <= 1,
                    "an unattached thread must not write into another's log"
                );
            })
            .join()
            .expect("the unattached thread should not panic");
        }
        fire("after");

        let said = log.messages_on(PROBE_TARGET);
        assert_eq!(said.len(), 1, "only the attached scope records: {said:?}");
        assert!(said[0].contains("during"), "{said:?}");
    }
}
