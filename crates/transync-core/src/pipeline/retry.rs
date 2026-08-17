//! Retry / fallback state machine. Pure logic — owns no I/O, never sleeps.
//!
//! TRACE: SCN-07
//! TRACE: SCN-08
//! TRACE: contracts.md §5

use crate::llm::{RetryContext, TranslationUnit};
use crate::validate::ValidationLayer;

/// How many bytes of the original reason survive truncation. Mirrors
/// client-side `truncate_diagnostic` (R0008-0034): reasons can embed payload
/// excerpts; unbounded reasons bloat prompts.
///
/// R0003-0046: this bounds the retained **prefix**, not the emitted string —
/// [`truncate_reason`] appends a fixed `"… (truncated)"` marker after the cut,
/// so a truncated reason runs to this many bytes plus that marker. Anything
/// sizing a budget off a retry hint must price the emitted string; the packer
/// does, by encoding the wire JSON itself
/// (`crate::llm::prompt::advisory_hint_json`) rather than this constant.
const MAX_RETRY_REASON_PREFIX_BYTES: usize = 512;

/// Attach the non-content retry channel to a re-dispatched unit.
///
/// `unit` must be the PRISTINE original (`retry: None`) so contexts
/// never stack; `attempt` is the upcoming attempt's 1-based number
/// (the first retry carries 2). The payload, constraints, and context
/// are untouched — resubmission stays verbatim per ADR-0009; only the
/// side channel (`retry`) is added, carried outside `source_payload`.
///
/// This is the "non-content retry channel" ADR-0009's Considered-Option
/// 3 anticipated. An empty `failure_reason` yields `reason: None`.
///
/// TRACE: SCN-07
/// EXT-2026-07 P0-2
pub fn retry_validation_unit(
    mut unit: TranslationUnit,
    attempt: u32,
    rejected_by: Option<ValidationLayer>,
    failure_reason: &str,
) -> TranslationUnit {
    unit.retry = Some(RetryContext {
        attempt,
        rejected_by,
        reason: (!failure_reason.is_empty()).then(|| truncate_reason(failure_reason)),
    });
    unit
}

/// Char-boundary-safe cut at [`MAX_RETRY_REASON_PREFIX_BYTES`] with a
/// `"… (truncated)"` suffix — mirrors the shape of `transync-openai`'s
/// `truncate_diagnostic`. The result is therefore the prefix *plus* that
/// marker, never bounded by the constant alone (R0003-0046).
///
/// EXT-2026-07 P0-2
fn truncate_reason(s: &str) -> String {
    if s.len() <= MAX_RETRY_REASON_PREFIX_BYTES {
        return s.to_string();
    }
    let mut end = MAX_RETRY_REASON_PREFIX_BYTES;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}… (truncated)", &s[..end])
}

// R0006-0053: the former `provider_retry` hook was deleted — the live
// transient-retry loop is `pipeline::dispatch::translate_with_provider_retries`,
// which owns its own budget/backoff and never consulted this helper.
//
// OI-0027: three more no-op hooks went the same way once `pipeline` became
// `pub(crate)` and the compiler could finally see they had no callers at
// all. None of them carried logic; each merely named a path that lives
// elsewhere, so keeping them would have meant `#[allow(dead_code)]` over
// doc comments describing behavior the bodies did not implement:
//
// - `retry_validation(batch, reason) -> batch` — claimed to re-batch a unit
//   with stricter prompt scope; returned its argument unchanged. The real
//   per-unit re-dispatch is `retry_validation_unit` above (ADR-0009).
// - `oversize_split(batch) -> vec![batch]` — the row-window splitting
//   extension point for a provider that signals oversize via
//   `TranslatorError::Unsupported`. The deferral this bullet recorded is
//   over, resolved in opposite directions for its two halves.
//   Row-window splitting SHIPPED with DCR-0026 — but as *packing*,
//   not retrying: `unit::split::split_oversize_tables` replaces an oversize
//   table with header-carrying windows exactly once, before round one, and
//   `pipeline::merge` reassembles one table afterwards. SCN-03 therefore
//   rides that path under the shipped default profile
//   (`default_table_strategy = "row-window-first"`), and the whole-block
//   path only when a profile or `--table-strategy` asks for it. The
//   *reactive* form this hook named — re-scoping a unit after a provider
//   signal — is REJECTED, not deferred (`unit::split`'s module doc,
//   contracts.md §5): nothing re-scopes a unit after packing, which is what
//   leaves ADR-0009's verbatim-resubmission rule untouched rather than
//   amended. If consumer-side oversize *handling* is ever wanted, it belongs
//   to OI-0008's retry/fallback redesign, not to a resurrected empty hook
//   (stub-manifest.md STUB-029). OI-0008: `transync-openai`'s mirror-image
//   `pagination::split_oversize` no-op was deleted alongside it.
// - `fallback_to_source(unit_id)` — empty; the SCN-08 fallback path is in
//   `pipeline::run_pipeline`, whose `accepted` map records FallbackSource
//   units directly.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::{BlockId, BlockKind};
    use crate::llm::{BatchId, BlockConstraints, BlockContext, InputMode};

    fn pristine_unit() -> TranslationUnit {
        TranslationUnit {
            unit_id: BlockId("p-0001".to_string()),
            block_kind: BlockKind::Paragraph,
            input_mode: InputMode::TextFragment,
            source_payload: "source".to_string(),
            context: BlockContext::default(),
            constraints: BlockConstraints::default(),
            source_hash: 0,
            batch_id: BatchId::new(1),
            retry: None,
        }
    }

    // EXT-2026-07 P0-2: a >512-byte reason with multibyte chars straddling
    // the boundary is cut char-safely (no panic, valid UTF-8) with a suffix.
    #[test]
    fn reason_truncates_char_safely() {
        // Each '가' is 3 bytes; 300 of them = 900 bytes, past the 512 cap,
        // and 512 lands mid-char so the cut must back up to a boundary.
        let reason = "가".repeat(300);
        let u = retry_validation_unit(
            pristine_unit(),
            2,
            Some(ValidationLayer::PerKindShape),
            &reason,
        );
        let ctx = u.retry.expect("retry attached");
        assert_eq!(ctx.attempt, 2);
        assert_eq!(ctx.rejected_by, Some(ValidationLayer::PerKindShape));
        let r = ctx.reason.expect("reason present");
        assert!(r.ends_with("… (truncated)"), "got: {r}");
        // R0003-0046: the constant bounds the retained prefix; the emitted
        // string is that prefix plus the marker, and this is where that is
        // written down as a bound rather than left to be misread.
        assert!(
            r.len() <= MAX_RETRY_REASON_PREFIX_BYTES + "… (truncated)".len(),
            "truncated reason too long: {} bytes",
            r.len()
        );
        // Original resubmission stays verbatim (ADR-0009).
        assert_eq!(u.source_payload, "source");
    }

    // A short reason is carried untruncated; an empty one becomes `None`.
    #[test]
    fn short_reason_untruncated_empty_is_none() {
        let u = retry_validation_unit(pristine_unit(), 3, None, "too many columns");
        assert_eq!(
            u.retry.as_ref().unwrap().reason.as_deref(),
            Some("too many columns")
        );

        let u = retry_validation_unit(pristine_unit(), 2, None, "");
        assert_eq!(u.retry.unwrap().reason, None);
    }
}
